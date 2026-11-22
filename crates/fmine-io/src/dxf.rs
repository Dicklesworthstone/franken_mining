//! Determinism: BitwisePortable
//! Pure-Rust bounded ASCII DXF subset reader (`BD-195`, plan §32.1 & §39 WP-11).
//!
//! Supported sections and entities:
//! - `HEADER`: `$ACADVER`, `$INSUNITS` (Autodesk unit codes; explicit unit reporting, never silent
//!   feet-to-metres conversion);
//! - `ENTITIES`: `3DFACE` (triangles and quads split consistently into two triangles), `LINE`,
//!   `POINT`, `LWPOLYLINE` (2D + elevation, open or closed; non-zero bulge explicitly refused so
//!   curved arcs are never silently chord-approximated), and `POLYLINE` / `VERTEX` / `SEQEND`
//!   (both 3D polylines and `PFACE` polyface meshes).
//!
//! Security & honesty invariants:
//! - Hard byte (`MAX_DXF_BYTES`), entity (`MAX_DXF_ENTITIES`), and vertex (`MAX_DXF_VERTICES`) caps
//!   prevent unbounded allocation on adversarial inputs (`PO-PORT-1`, ready for `BD-197` fuzzing).
//! - Binary DXF (`AutoCAD Binary DXF`) is explicitly refused rather than misparsed.
//! - Every parsed document binds its raw bytes via `source_sha256` and carries an explicit caller
//!   [`CrsId`] (`G-CRS-1`).
//! - Direct bridges to `fmine_geom::winding::Soup` and `fmine_geom::volcert::ClosedMesh::certify`.

use std::collections::BTreeSet;
use std::fmt;

use fmine_core::hash::{Digest32, sha256};
use fmine_crs::CrsId;
use fmine_evidence::{Refusal, RefusalCode};
use fmine_geom::volcert::ClosedMesh;
use fmine_geom::winding::Soup;

/// Maximum admitted ASCII DXF byte length (32 MiB, `PO-PORT-1`).
pub const MAX_DXF_BYTES: usize = 32 * 1024 * 1024;
/// Maximum admitted entity count per DXF document.
pub const MAX_DXF_ENTITIES: usize = 500_000;
/// Maximum admitted vertices across all polylines/meshes in one DXF document.
pub const MAX_DXF_VERTICES: usize = 2_000_000;

/// Autodesk DXF `$INSUNITS` header code (plan §25 unit hygiene: never silently convert feet to metres).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DxfUnits {
    Unitless,
    Inches,
    Feet,
    Miles,
    Millimetres,
    Centimetres,
    Metres,
    Kilometres,
    Decimetres,
    UsSurveyFeet,
    Other(i32),
}

impl DxfUnits {
    /// Decode Autodesk DXF `$INSUNITS` integer code.
    #[must_use]
    pub const fn from_code(code: i32) -> Self {
        match code {
            0 => Self::Unitless,
            1 => Self::Inches,
            2 => Self::Feet,
            3 => Self::Miles,
            4 => Self::Millimetres,
            5 => Self::Centimetres,
            6 => Self::Metres,
            7 => Self::Kilometres,
            14 => Self::Decimetres,
            21 => Self::UsSurveyFeet,
            other => Self::Other(other),
        }
    }

    /// Canonical unit label.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Unitless => "unitless",
            Self::Inches => "inches",
            Self::Feet => "feet",
            Self::Miles => "miles",
            Self::Millimetres => "mm",
            Self::Centimetres => "cm",
            Self::Metres => "m",
            Self::Kilometres => "km",
            Self::Decimetres => "dm",
            Self::UsSurveyFeet => "us_survey_feet",
            Self::Other(_) => "unregistered",
        }
    }
}

/// Typed errors raised by the bounded ASCII DXF reader.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DxfError {
    ByteBudgetExceeded { actual: usize, limit: usize },
    EntityBudgetExceeded { limit: usize },
    VertexBudgetExceeded { limit: usize },
    BinaryDxfUnsupported,
    InvalidUtf8,
    MalformedGroupPair { line: usize, detail: String },
    NonFiniteCoordinate { line: usize, detail: String },
    UnsupportedBulge { line: usize, layer: String },
    InvalidPolyfaceIndex { line: usize, detail: String },
    UndeclaredOrNonMetricUnits { declared: Option<DxfUnits> },
    EmptySurface,
    GeometryRefusal(String),
}

impl DxfError {
    /// Map DXF parsing or validation failures to registered `FMINE-REF-*` codes.
    #[must_use]
    pub fn to_refusal(&self) -> Refusal {
        match self {
            Self::UndeclaredOrNonMetricUnits { .. } => {
                Refusal::new(RefusalCode::UndeclaredUnitBasisOrTz107, self.to_string())
            }
            Self::NonFiniteCoordinate { .. }
            | Self::InvalidPolyfaceIndex { .. }
            | Self::EmptySurface => {
                Refusal::new(RefusalCode::OutsidePhysicalDomain109, self.to_string())
            }
            Self::GeometryRefusal(_) => {
                Refusal::new(RefusalCode::NumericalCertificateFailed207, self.to_string())
            }
            _ => Refusal::new(RefusalCode::SourceCellUnusable208, self.to_string()),
        }
    }
}

impl fmt::Display for DxfError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ByteBudgetExceeded { actual, limit } => {
                write!(f, "DXF byte size {actual} exceeds limit {limit}")
            }
            Self::EntityBudgetExceeded { limit } => {
                write!(f, "DXF entity count exceeds limit {limit}")
            }
            Self::VertexBudgetExceeded { limit } => {
                write!(f, "DXF vertex count exceeds limit {limit}")
            }
            Self::BinaryDxfUnsupported => {
                write!(f, "binary DXF is unsupported; supply ASCII DXF")
            }
            Self::InvalidUtf8 => write!(f, "DXF payload is not valid UTF-8"),
            Self::MalformedGroupPair { line, detail } => {
                write!(f, "malformed DXF group pair at line {line}: {detail}")
            }
            Self::NonFiniteCoordinate { line, detail } => {
                write!(f, "non-finite DXF coordinate at line {line}: {detail}")
            }
            Self::UnsupportedBulge { line, layer } => {
                write!(
                    f,
                    "DXF LWPOLYLINE on layer '{layer}' at line {line} contains non-zero bulge arc; \
                     refusing silent chord approximation"
                )
            }
            Self::InvalidPolyfaceIndex { line, detail } => {
                write!(f, "invalid DXF PFACE face index at line {line}: {detail}")
            }
            Self::UndeclaredOrNonMetricUnits { declared } => {
                write!(
                    f,
                    "DXF $INSUNITS must explicitly declare Metres (code 6), got {declared:?}"
                )
            }
            Self::EmptySurface => write!(f, "DXF document contains no surface triangles"),
            Self::GeometryRefusal(msg) => write!(f, "DXF geometry certification failed: {msg}"),
        }
    }
}

impl std::error::Error for DxfError {}

/// One 3D point entity (`POINT`).
#[derive(Debug, Clone, PartialEq)]
pub struct DxfPoint {
    pub layer: String,
    pub position: [f64; 3],
}

/// One 3D line segment entity (`LINE`).
#[derive(Debug, Clone, PartialEq)]
pub struct DxfLine {
    pub layer: String,
    pub start: [f64; 3],
    pub end: [f64; 3],
}

/// One 3D polyline or lightweight contour (`LWPOLYLINE` / `POLYLINE`).
#[derive(Debug, Clone, PartialEq)]
pub struct DxfPolyline {
    pub layer: String,
    pub closed: bool,
    pub vertices: Vec<[f64; 3]>,
}

/// One triangulated 3D surface face extracted from `3DFACE` or `POLYLINE`/`PFACE`.
#[derive(Debug, Clone, PartialEq)]
pub struct DxfTriangle {
    pub layer: String,
    pub vertices: [[f64; 3]; 3],
}

/// Parsed bounded ASCII DXF document with provenance digest and explicit CRS tag (`BD-195`).
#[derive(Debug, Clone, PartialEq)]
pub struct DxfDocument {
    pub crs_id: CrsId,
    pub source_sha256: Digest32,
    pub acad_version: Option<String>,
    pub insunits: Option<DxfUnits>,
    pub points: Vec<DxfPoint>,
    pub lines: Vec<DxfLine>,
    pub polylines: Vec<DxfPolyline>,
    pub triangles: Vec<DxfTriangle>,
    pub skipped_entity_types: BTreeSet<String>,
}

impl DxfDocument {
    /// Require `$INSUNITS == Some(DxfUnits::Metres)` before converting coordinates into metric
    /// geometry (`G-UNIT-1`, never silently assume metres when feet or unitless is declared).
    pub fn require_metres(&self) -> Result<(), DxfError> {
        if self.insunits == Some(DxfUnits::Metres) {
            Ok(())
        } else {
            Err(DxfError::UndeclaredOrNonMetricUnits {
                declared: self.insunits,
            })
        }
    }

    /// Build a [`Soup`] over all extracted surface triangles (optionally filtered to one layer).
    pub fn to_triangle_soup(&self, layer_filter: Option<&str>) -> Result<Soup, DxfError> {
        let mut positions = Vec::new();
        let mut indices = Vec::new();
        for tri in &self.triangles {
            if let Some(target) = layer_filter
                && tri.layer != target
            {
                continue;
            }
            let base =
                u32::try_from(positions.len()).map_err(|_| DxfError::VertexBudgetExceeded {
                    limit: MAX_DXF_VERTICES,
                })?;
            positions.extend_from_slice(&tri.vertices);
            indices.push([base, base + 1, base + 2]);
        }
        if indices.is_empty() {
            return Err(DxfError::EmptySurface);
        }
        Soup::new(positions, indices).map_err(|r| DxfError::GeometryRefusal(r.to_string()))
    }

    /// Certify extracted surface triangles as a closed, consistently outward-oriented [`ClosedMesh`]
    /// with exact volume enclosure (`fmine_geom::volcert`, `PO-STRATA-8`).
    pub fn certify_closed_mesh(&self, layer_filter: Option<&str>) -> Result<ClosedMesh, DxfError> {
        let mut positions = Vec::new();
        let mut indices = Vec::new();
        for tri in &self.triangles {
            if let Some(target) = layer_filter
                && tri.layer != target
            {
                continue;
            }
            let base =
                u32::try_from(positions.len()).map_err(|_| DxfError::VertexBudgetExceeded {
                    limit: MAX_DXF_VERTICES,
                })?;
            positions.extend_from_slice(&tri.vertices);
            indices.push([base, base + 1, base + 2]);
        }
        if indices.is_empty() {
            return Err(DxfError::EmptySurface);
        }
        ClosedMesh::certify(positions, indices)
            .map_err(|r| DxfError::GeometryRefusal(r.to_string()))
    }
}

#[derive(Debug, Clone, Copy)]
struct GroupPair<'a> {
    code: i32,
    value: &'a str,
    line: usize,
}

fn parse_f64_finite(s: &str, line: usize) -> Result<f64, DxfError> {
    let v: f64 = s.trim().parse().map_err(|_| DxfError::MalformedGroupPair {
        line,
        detail: format!("expected float coordinate, got '{s}'"),
    })?;
    if !v.is_finite() {
        return Err(DxfError::NonFiniteCoordinate {
            line,
            detail: format!("non-finite coordinate '{s}'"),
        });
    }
    Ok(v)
}

fn parse_i32(s: &str, line: usize) -> Result<i32, DxfError> {
    s.trim().parse().map_err(|_| DxfError::MalformedGroupPair {
        line,
        detail: format!("expected integer value, got '{s}'"),
    })
}

fn same_point_bits(a: [f64; 3], b: [f64; 3]) -> bool {
    a[0] == b[0] && a[1] == b[1] && a[2] == b[2]
}

fn tokenize_ascii_dxf(bytes: &[u8]) -> Result<Vec<GroupPair<'_>>, DxfError> {
    if bytes.len() > MAX_DXF_BYTES {
        return Err(DxfError::ByteBudgetExceeded {
            actual: bytes.len(),
            limit: MAX_DXF_BYTES,
        });
    }
    if bytes.starts_with(b"AutoCAD Binary DXF") || bytes.contains(&0u8) {
        return Err(DxfError::BinaryDxfUnsupported);
    }
    let text = core::str::from_utf8(bytes).map_err(|_| DxfError::InvalidUtf8)?;
    let mut lines = text.lines().enumerate();
    let mut pairs = Vec::new();
    while let Some((idx, raw_code)) = lines.next() {
        let code_line = idx + 1;
        let trimmed = raw_code.trim();
        if trimmed.is_empty() && lines.clone().next().is_none() {
            break;
        }
        let code: i32 = trimmed.parse().map_err(|_| DxfError::MalformedGroupPair {
            line: code_line,
            detail: format!("expected integer group code, got '{trimmed}'"),
        })?;
        let Some((_, raw_val)) = lines.next() else {
            return Err(DxfError::MalformedGroupPair {
                line: code_line,
                detail: "dangling group code without value line".to_string(),
            });
        };
        pairs.push(GroupPair {
            code,
            value: raw_val.trim_end_matches('\r'),
            line: code_line,
        });
    }
    if pairs.is_empty() {
        return Err(DxfError::MalformedGroupPair {
            line: 1,
            detail: "empty DXF payload".to_string(),
        });
    }
    Ok(pairs)
}

/// Parse an ASCII DXF byte slice under strict byte/entity/vertex caps (`BD-195`).
pub fn parse_ascii_dxf(bytes: &[u8], crs_id: CrsId) -> Result<DxfDocument, DxfError> {
    let pairs = tokenize_ascii_dxf(bytes)?;
    let source_sha256 = sha256(bytes);

    let mut acad_version = None;
    let mut insunits = None;
    let mut points = Vec::new();
    let mut lines = Vec::new();
    let mut polylines = Vec::new();
    let mut triangles = Vec::new();
    let mut skipped_entity_types = BTreeSet::new();

    let mut entity_count = 0_usize;
    let mut vertex_count = 0_usize;

    let mut i = 0_usize;
    while i < pairs.len() {
        if pairs[i].code == 0 && pairs[i].value.trim().eq_ignore_ascii_case("SECTION") {
            i += 1;
            if i >= pairs.len() || pairs[i].code != 2 {
                return Err(DxfError::MalformedGroupPair {
                    line: pairs.get(i).map_or(1, |p| p.line),
                    detail: "SECTION must be followed by group code 2 section name".to_string(),
                });
            }
            let sec_name = pairs[i].value.trim().to_ascii_uppercase();
            i += 1;
            if sec_name == "HEADER" {
                let mut current_var: Option<String> = None;
                while i < pairs.len() {
                    if pairs[i].code == 0 && pairs[i].value.trim().eq_ignore_ascii_case("ENDSEC") {
                        i += 1;
                        break;
                    }
                    if pairs[i].code == 9 {
                        current_var = Some(pairs[i].value.trim().to_ascii_uppercase());
                    } else if let Some(ref var) = current_var {
                        if var == "$ACADVER" && pairs[i].code == 1 {
                            acad_version = Some(pairs[i].value.trim().to_string());
                        } else if var == "$INSUNITS" && pairs[i].code == 70 {
                            let code = parse_i32(pairs[i].value, pairs[i].line)?;
                            insunits = Some(DxfUnits::from_code(code));
                        }
                    }
                    i += 1;
                }
            } else if sec_name == "ENTITIES" {
                while i < pairs.len() {
                    if pairs[i].code == 0 && pairs[i].value.trim().eq_ignore_ascii_case("ENDSEC") {
                        i += 1;
                        break;
                    }
                    if pairs[i].code != 0 {
                        i += 1;
                        continue;
                    }
                    entity_count += 1;
                    if entity_count > MAX_DXF_ENTITIES {
                        return Err(DxfError::EntityBudgetExceeded {
                            limit: MAX_DXF_ENTITIES,
                        });
                    }
                    let entity_type = pairs[i].value.trim().to_ascii_uppercase();
                    let entity_line = pairs[i].line;
                    i += 1;

                    match entity_type.as_str() {
                        "POINT" => {
                            let mut layer = "0".to_string();
                            let mut pos = [0.0_f64; 3];
                            while i < pairs.len() && pairs[i].code != 0 {
                                match pairs[i].code {
                                    8 => layer = pairs[i].value.trim().to_string(),
                                    10 => pos[0] = parse_f64_finite(pairs[i].value, pairs[i].line)?,
                                    20 => pos[1] = parse_f64_finite(pairs[i].value, pairs[i].line)?,
                                    30 => pos[2] = parse_f64_finite(pairs[i].value, pairs[i].line)?,
                                    _ => {}
                                }
                                i += 1;
                            }
                            vertex_count += 1;
                            if vertex_count > MAX_DXF_VERTICES {
                                return Err(DxfError::VertexBudgetExceeded {
                                    limit: MAX_DXF_VERTICES,
                                });
                            }
                            points.push(DxfPoint {
                                layer,
                                position: pos,
                            });
                        }
                        "LINE" => {
                            let mut layer = "0".to_string();
                            let mut start = [0.0_f64; 3];
                            let mut end = [0.0_f64; 3];
                            while i < pairs.len() && pairs[i].code != 0 {
                                match pairs[i].code {
                                    8 => layer = pairs[i].value.trim().to_string(),
                                    10 => {
                                        start[0] = parse_f64_finite(pairs[i].value, pairs[i].line)?
                                    }
                                    20 => {
                                        start[1] = parse_f64_finite(pairs[i].value, pairs[i].line)?
                                    }
                                    30 => {
                                        start[2] = parse_f64_finite(pairs[i].value, pairs[i].line)?
                                    }
                                    11 => end[0] = parse_f64_finite(pairs[i].value, pairs[i].line)?,
                                    21 => end[1] = parse_f64_finite(pairs[i].value, pairs[i].line)?,
                                    31 => end[2] = parse_f64_finite(pairs[i].value, pairs[i].line)?,
                                    _ => {}
                                }
                                i += 1;
                            }
                            vertex_count += 2;
                            if vertex_count > MAX_DXF_VERTICES {
                                return Err(DxfError::VertexBudgetExceeded {
                                    limit: MAX_DXF_VERTICES,
                                });
                            }
                            lines.push(DxfLine { layer, start, end });
                        }
                        "3DFACE" => {
                            let mut layer = "0".to_string();
                            let mut v = [[0.0_f64; 3]; 4];
                            let mut has_v3 = false;
                            while i < pairs.len() && pairs[i].code != 0 {
                                let p = pairs[i];
                                match p.code {
                                    8 => layer = p.value.trim().to_string(),
                                    10 => v[0][0] = parse_f64_finite(p.value, p.line)?,
                                    20 => v[0][1] = parse_f64_finite(p.value, p.line)?,
                                    30 => v[0][2] = parse_f64_finite(p.value, p.line)?,
                                    11 => v[1][0] = parse_f64_finite(p.value, p.line)?,
                                    21 => v[1][1] = parse_f64_finite(p.value, p.line)?,
                                    31 => v[1][2] = parse_f64_finite(p.value, p.line)?,
                                    12 => v[2][0] = parse_f64_finite(p.value, p.line)?,
                                    22 => v[2][1] = parse_f64_finite(p.value, p.line)?,
                                    32 => v[2][2] = parse_f64_finite(p.value, p.line)?,
                                    13 => {
                                        v[3][0] = parse_f64_finite(p.value, p.line)?;
                                        has_v3 = true;
                                    }
                                    23 => {
                                        v[3][1] = parse_f64_finite(p.value, p.line)?;
                                        has_v3 = true;
                                    }
                                    33 => {
                                        v[3][2] = parse_f64_finite(p.value, p.line)?;
                                        has_v3 = true;
                                    }
                                    _ => {}
                                }
                                i += 1;
                            }
                            vertex_count += 3;
                            if vertex_count > MAX_DXF_VERTICES {
                                return Err(DxfError::VertexBudgetExceeded {
                                    limit: MAX_DXF_VERTICES,
                                });
                            }
                            triangles.push(DxfTriangle {
                                layer: layer.clone(),
                                vertices: [v[0], v[1], v[2]],
                            });
                            // Quad 3DFACE (4th corner distinct from 3rd and 0th): emit second triangle (v0, v2, v3).
                            if has_v3
                                && !same_point_bits(v[3], v[2])
                                && !same_point_bits(v[3], v[0])
                            {
                                vertex_count += 3;
                                if vertex_count > MAX_DXF_VERTICES {
                                    return Err(DxfError::VertexBudgetExceeded {
                                        limit: MAX_DXF_VERTICES,
                                    });
                                }
                                triangles.push(DxfTriangle {
                                    layer,
                                    vertices: [v[0], v[2], v[3]],
                                });
                            }
                        }
                        "LWPOLYLINE" => {
                            let mut layer = "0".to_string();
                            let mut closed = false;
                            let mut elevation = 0.0_f64;
                            let mut xy: Vec<[f64; 2]> = Vec::new();
                            let mut cur_x: Option<f64> = None;
                            let mut cur_y: Option<f64> = None;
                            while i < pairs.len() && pairs[i].code != 0 {
                                let p = pairs[i];
                                match p.code {
                                    8 => layer = p.value.trim().to_string(),
                                    38 => elevation = parse_f64_finite(p.value, p.line)?,
                                    70 => {
                                        let flags = parse_i32(p.value, p.line)?;
                                        closed = (flags & 1) != 0;
                                    }
                                    10 => {
                                        if let (Some(x), Some(y)) = (cur_x.take(), cur_y.take()) {
                                            xy.push([x, y]);
                                        }
                                        cur_x = Some(parse_f64_finite(p.value, p.line)?);
                                    }
                                    20 => {
                                        cur_y = Some(parse_f64_finite(p.value, p.line)?);
                                    }
                                    42 => {
                                        let bulge = parse_f64_finite(p.value, p.line)?;
                                        if bulge != 0.0 {
                                            return Err(DxfError::UnsupportedBulge {
                                                line: p.line,
                                                layer,
                                            });
                                        }
                                    }
                                    _ => {}
                                }
                                i += 1;
                            }
                            if let (Some(x), Some(y)) = (cur_x, cur_y) {
                                xy.push([x, y]);
                            }
                            vertex_count = vertex_count.saturating_add(xy.len());
                            if vertex_count > MAX_DXF_VERTICES {
                                return Err(DxfError::VertexBudgetExceeded {
                                    limit: MAX_DXF_VERTICES,
                                });
                            }
                            let verts = xy.into_iter().map(|[x, y]| [x, y, elevation]).collect();
                            polylines.push(DxfPolyline {
                                layer,
                                closed,
                                vertices: verts,
                            });
                        }
                        "POLYLINE" => {
                            let mut layer = "0".to_string();
                            let mut poly_flags = 0_i32;
                            while i < pairs.len() && pairs[i].code != 0 {
                                match pairs[i].code {
                                    8 => layer = pairs[i].value.trim().to_string(),
                                    70 => poly_flags = parse_i32(pairs[i].value, pairs[i].line)?,
                                    _ => {}
                                }
                                i += 1;
                            }
                            let is_polyface = (poly_flags & 64) != 0;
                            let is_closed = (poly_flags & 1) != 0;
                            let mut mesh_verts: Vec<[f64; 3]> = Vec::new();
                            let mut poly_verts: Vec<[f64; 3]> = Vec::new();
                            let mut seen_seqend = false;

                            while i < pairs.len() {
                                if pairs[i].code != 0 {
                                    i += 1;
                                    continue;
                                }
                                let sub = pairs[i].value.trim().to_ascii_uppercase();
                                let sub_line = pairs[i].line;
                                i += 1;
                                if sub == "SEQEND" {
                                    while i < pairs.len() && pairs[i].code != 0 {
                                        i += 1;
                                    }
                                    seen_seqend = true;
                                    break;
                                }
                                if sub != "VERTEX" {
                                    return Err(DxfError::MalformedGroupPair {
                                        line: sub_line,
                                        detail: format!(
                                            "expected VERTEX or SEQEND inside POLYLINE, got '{sub}'"
                                        ),
                                    });
                                }
                                let mut vflags = 0_i32;
                                let mut pt = [0.0_f64; 3];
                                let mut fidx = [0_i32; 4];
                                while i < pairs.len() && pairs[i].code != 0 {
                                    let p = pairs[i];
                                    match p.code {
                                        10 => pt[0] = parse_f64_finite(p.value, p.line)?,
                                        20 => pt[1] = parse_f64_finite(p.value, p.line)?,
                                        30 => pt[2] = parse_f64_finite(p.value, p.line)?,
                                        70 => vflags = parse_i32(p.value, p.line)?,
                                        71 => fidx[0] = parse_i32(p.value, p.line)?,
                                        72 => fidx[1] = parse_i32(p.value, p.line)?,
                                        73 => fidx[2] = parse_i32(p.value, p.line)?,
                                        74 => fidx[3] = parse_f64_or_i32_idx(p.value, p.line)?,
                                        _ => {}
                                    }
                                    i += 1;
                                }
                                if is_polyface {
                                    // Polyface face record: bit 128 set and bit 64 clear.
                                    let is_face = (vflags & 128) != 0 && (vflags & 64) == 0;
                                    if is_face {
                                        let resolve = |raw: i32| -> Result<[f64; 3], DxfError> {
                                            let a = raw.unsigned_abs() as usize;
                                            if a == 0 || a > mesh_verts.len() {
                                                return Err(DxfError::InvalidPolyfaceIndex {
                                                    line: sub_line,
                                                    detail: format!(
                                                        "face index {raw} out of range 1..={}",
                                                        mesh_verts.len()
                                                    ),
                                                });
                                            }
                                            Ok(mesh_verts[a - 1])
                                        };
                                        let a = resolve(fidx[0])?;
                                        let b = resolve(fidx[1])?;
                                        let c = resolve(fidx[2])?;
                                        triangles.push(DxfTriangle {
                                            layer: layer.clone(),
                                            vertices: [a, b, c],
                                        });
                                        if fidx[3] != 0 {
                                            let d = resolve(fidx[3])?;
                                            if !same_point_bits(d, c) && !same_point_bits(d, a) {
                                                triangles.push(DxfTriangle {
                                                    layer: layer.clone(),
                                                    vertices: [a, c, d],
                                                });
                                            }
                                        }
                                    } else {
                                        vertex_count += 1;
                                        if vertex_count > MAX_DXF_VERTICES {
                                            return Err(DxfError::VertexBudgetExceeded {
                                                limit: MAX_DXF_VERTICES,
                                            });
                                        }
                                        mesh_verts.push(pt);
                                    }
                                } else {
                                    vertex_count += 1;
                                    if vertex_count > MAX_DXF_VERTICES {
                                        return Err(DxfError::VertexBudgetExceeded {
                                            limit: MAX_DXF_VERTICES,
                                        });
                                    }
                                    poly_verts.push(pt);
                                }
                            }
                            if !seen_seqend {
                                return Err(DxfError::MalformedGroupPair {
                                    line: entity_line,
                                    detail: "unterminated POLYLINE missing SEQEND".to_string(),
                                });
                            }
                            if !is_polyface {
                                polylines.push(DxfPolyline {
                                    layer,
                                    closed: is_closed,
                                    vertices: poly_verts,
                                });
                            }
                        }
                        other => {
                            skipped_entity_types.insert(other.to_string());
                            while i < pairs.len() && pairs[i].code != 0 {
                                i += 1;
                            }
                        }
                    }
                }
            } else {
                // Skip other sections (CLASSES, TABLES, BLOCKS, OBJECTS) cleanly until ENDSEC.
                while i < pairs.len() {
                    if pairs[i].code == 0 && pairs[i].value.trim().eq_ignore_ascii_case("ENDSEC") {
                        i += 1;
                        break;
                    }
                    i += 1;
                }
            }
        } else {
            i += 1;
        }
    }

    Ok(DxfDocument {
        crs_id,
        source_sha256,
        acad_version,
        insunits,
        points,
        lines,
        polylines,
        triangles,
        skipped_entity_types,
    })
}

fn parse_f64_or_i32_idx(s: &str, line: usize) -> Result<i32, DxfError> {
    parse_i32(s, line)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn emit_3dface(buf: &mut String, layer: &str, v: [[f64; 3]; 4]) {
        buf.push_str(&format!(
            "0\n3DFACE\n8\n{layer}\n\
             10\n{}\n20\n{}\n30\n{}\n\
             11\n{}\n21\n{}\n31\n{}\n\
             12\n{}\n22\n{}\n32\n{}\n\
             13\n{}\n23\n{}\n33\n{}\n",
            v[0][0],
            v[0][1],
            v[0][2],
            v[1][0],
            v[1][1],
            v[1][2],
            v[2][0],
            v[2][1],
            v[2][2],
            v[3][0],
            v[3][1],
            v[3][2],
        ));
    }

    #[test]
    fn test_dxf_3dface_closed_cube_certifies_exact_volume_and_units() {
        // 10m x 10m x 10m closed cube (6 outward quad 3DFACEs -> 12 triangles, exact volume = 1000 m^3).
        let mut dxf = String::from(
            "0\nSECTION\n2\nHEADER\n9\n$ACADVER\n1\nAC1032\n9\n$INSUNITS\n70\n6\n0\nENDSEC\n\
             0\nSECTION\n2\nENTITIES\n",
        );
        let p = |x, y, z| [x, y, z];
        // Bottom (-Z), Top (+Z), Front (-Y), Back (+Y), Left (-X), Right (+X), all outward CCW.
        emit_3dface(
            &mut dxf,
            "PIT_SHELL",
            [
                p(0.0, 0.0, 0.0),
                p(0.0, 10.0, 0.0),
                p(10.0, 10.0, 0.0),
                p(10.0, 0.0, 0.0),
            ],
        );
        emit_3dface(
            &mut dxf,
            "PIT_SHELL",
            [
                p(0.0, 0.0, 10.0),
                p(10.0, 0.0, 10.0),
                p(10.0, 10.0, 10.0),
                p(0.0, 10.0, 10.0),
            ],
        );
        emit_3dface(
            &mut dxf,
            "PIT_SHELL",
            [
                p(0.0, 0.0, 0.0),
                p(10.0, 0.0, 0.0),
                p(10.0, 0.0, 10.0),
                p(0.0, 0.0, 10.0),
            ],
        );
        emit_3dface(
            &mut dxf,
            "PIT_SHELL",
            [
                p(10.0, 10.0, 0.0),
                p(0.0, 10.0, 0.0),
                p(0.0, 10.0, 10.0),
                p(10.0, 10.0, 10.0),
            ],
        );
        emit_3dface(
            &mut dxf,
            "PIT_SHELL",
            [
                p(0.0, 10.0, 0.0),
                p(0.0, 0.0, 0.0),
                p(0.0, 0.0, 10.0),
                p(0.0, 10.0, 10.0),
            ],
        );
        emit_3dface(
            &mut dxf,
            "PIT_SHELL",
            [
                p(10.0, 0.0, 0.0),
                p(10.0, 10.0, 0.0),
                p(10.0, 10.0, 10.0),
                p(10.0, 0.0, 10.0),
            ],
        );
        dxf.push_str(
            "0\nLWPOLYLINE\n8\nCREST_500\n70\n1\n38\n500.0\n\
             10\n0.0\n20\n0.0\n10\n10.0\n20\n0.0\n10\n10.0\n20\n10.0\n10\n0.0\n20\n10.0\n\
             0\nPOINT\n8\nCOLLARS\n10\n5.0\n20\n5.0\n30\n10.0\n\
             0\nLINE\n8\nFAULT\n10\n0.0\n20\n0.0\n30\n0.0\n11\n10.0\n21\n10.0\n31\n10.0\n\
             0\nENDSEC\n0\nEOF\n",
        );

        let doc = parse_ascii_dxf(dxf.as_bytes(), CrsId::NGM_UTM11N).unwrap();
        doc.require_metres().unwrap();
        assert_eq!(doc.acad_version.as_deref(), Some("AC1032"));
        assert_eq!(doc.triangles.len(), 12);
        assert_eq!(doc.polylines.len(), 1);
        assert!(doc.polylines[0].closed);
        assert_eq!(doc.polylines[0].vertices[0], [0.0, 0.0, 500.0]);
        assert_eq!(doc.points.len(), 1);
        assert_eq!(doc.lines.len(), 1);

        let mesh = doc.certify_closed_mesh(Some("PIT_SHELL")).unwrap();
        let vol = mesh.volume();
        assert!(vol.lo() <= 1000.0 && 1000.0 <= vol.hi(), "vol={vol:?}");
        assert!((vol.hi() - vol.lo()) < 1e-9);
        let soup = doc.to_triangle_soup(Some("PIT_SHELL")).unwrap();
        assert_eq!(soup.triangles().len(), 12);
    }

    #[test]
    fn test_dxf_pface_polyface_mesh_and_adversarial_refusals() {
        // Right tetrahedron (0,0,0), (6,0,0), (0,6,0), (0,0,6) -> exact volume = 6*6*6 / 6 = 36 m^3.
        let pface = "\
0\nSECTION\n2\nHEADER\n9\n$INSUNITS\n70\n6\n0\nENDSEC\n\
0\nSECTION\n2\nENTITIES\n\
0\nPOLYLINE\n8\nSTOPE\n70\n64\n\
0\nVERTEX\n70\n192\n10\n0\n20\n0\n30\n0\n\
0\nVERTEX\n70\n192\n10\n6\n20\n0\n30\n0\n\
0\nVERTEX\n70\n192\n10\n0\n20\n6\n30\n0\n\
0\nVERTEX\n70\n192\n10\n0\n20\n0\n30\n6\n\
0\nVERTEX\n70\n128\n71\n1\n72\n3\n73\n2\n\
0\nVERTEX\n70\n128\n71\n1\n72\n2\n73\n4\n\
0\nVERTEX\n70\n128\n71\n1\n72\n4\n73\n3\n\
0\nVERTEX\n70\n128\n71\n2\n72\n-3\n73\n4\n\
0\nSEQEND\n\
0\nENDSEC\n0\nEOF\n";
        let doc = parse_ascii_dxf(pface.as_bytes(), CrsId::REKO_DIQ_UTM41N).unwrap();
        let mesh = doc.certify_closed_mesh(Some("STOPE")).unwrap();
        assert!(mesh.volume().lo() <= 36.0 && 36.0 <= mesh.volume().hi());

        // 1. Feet ($INSUNITS = 2) fails require_metres with FMINE-REF-107 (no silent conversion!).
        let feet_dxf = "0\nSECTION\n2\nHEADER\n9\n$INSUNITS\n70\n2\n0\nENDSEC\n0\nEOF\n";
        let feet_doc = parse_ascii_dxf(feet_dxf.as_bytes(), CrsId::NGM_UTM11N).unwrap();
        assert_eq!(feet_doc.insunits, Some(DxfUnits::Feet));
        assert_eq!(
            feet_doc.require_metres().unwrap_err().to_refusal().code,
            RefusalCode::UndeclaredUnitBasisOrTz107.spec().code
        );

        // 2. Binary DXF sentinel refused cleanly.
        assert_eq!(
            parse_ascii_dxf(b"AutoCAD Binary DXF\r\n\x1a\0", CrsId::NGM_UTM11N)
                .unwrap_err()
                .to_refusal()
                .code,
            RefusalCode::SourceCellUnusable208.spec().code
        );

        // 3. Non-zero bulge refused cleanly (never silently chord-approximated).
        let bulge_dxf = "0\nSECTION\n2\nENTITIES\n0\nLWPOLYLINE\n8\nARC\n10\n0\n20\n0\n42\n0.5\n0\nENDSEC\n0\nEOF\n";
        assert!(matches!(
            parse_ascii_dxf(bulge_dxf.as_bytes(), CrsId::NGM_UTM11N),
            Err(DxfError::UnsupportedBulge { .. })
        ));

        // 4. NaN coordinate refused with FMINE-REF-109.
        let nan_dxf =
            "0\nSECTION\n2\nENTITIES\n0\nPOINT\n8\nP\n10\nNaN\n20\n0\n30\n0\n0\nENDSEC\n0\nEOF\n";
        assert_eq!(
            parse_ascii_dxf(nan_dxf.as_bytes(), CrsId::NGM_UTM11N)
                .unwrap_err()
                .to_refusal()
                .code,
            RefusalCode::OutsidePhysicalDomain109.spec().code
        );
    }
}
