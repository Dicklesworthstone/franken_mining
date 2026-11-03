//! OMF 2 (Open Mining Format) reader (`BD-194`, plan §32.2, `KILL-6`, E858): block models, surfaces, line sets,
//! point sets, grid surfaces and composites, with their attributes, read without `serde` or `parquet` (both banned,
//! L-5).
//!
//! An OMF 2 file is a ZIP of stored members whose comment names the format: a gzipped JSON index
//! (`index.json.gz`), one Parquet file per array and PNG or JPEG images. `read` takes the whole file and returns
//! every element with every array decoded. It refuses, with `FMINE-REF-506`, anything it cannot read exactly:
//! - a file whose comment is not "Open Mining Format 2.x", a compressed member, a missing member;
//! - an array whose Parquet schema is not the one OMF gives its type, or whose length is not the one the index
//!   declares;
//! - the read-time checks of the vendor reader (omf-rust 64d3057): category indices, segment and triangle vertex
//!   indices, sub-block parents and corners, sub-block mode sizes, colormap boundaries in order, sizes above zero;
//! - the vendor validator's error-level checks: unit and orthogonal axes, finite origins and sizes, octree counts
//!   that are powers of two, attribute locations that apply to the element, attribute lengths equal to the
//!   location's.
//!
//! Not checked: the vendor validator's warnings (duplicate element names, for one), whether category names are
//! unique, and the content of images beyond their PNG or JPEG signature. Members the index never names are not
//! read.

use std::collections::BTreeMap;
use std::path::Path;

use fmine_evidence::{Refusal, RefusalCode};

use crate::zip_inflate::{archive_comment, read_stored_entries};

mod gzip;
pub mod json;
mod parquet;
mod proportions;

pub use json::Json;
use parquet::{Column, Logical, Table, Values};
pub use proportions::{parent_grid, proportions_in_surface, surface_mesh};

/// The largest member accepted.
pub const MAX_MEMBER_BYTES: usize = 1 << 30;

/// The largest total of members accepted.
pub const MAX_ARCHIVE_BYTES: usize = 1 << 31;

/// The largest index accepted, inflated.
pub const MAX_INDEX_BYTES: usize = 64 << 20;

const INDEX: &str = "index.json.gz";
const FORMAT: &str = "Open Mining Format ";

/// A project: the index's top level, with every element read.
#[derive(Debug, Clone, PartialEq)]
pub struct Project {
    /// The archive comment, such as "Open Mining Format 2.0".
    pub format: String,
    pub name: String,
    pub description: String,
    pub coordinate_reference_system: String,
    pub units: String,
    /// Added to every element's own origin.
    pub origin: [f64; 3],
    pub author: String,
    pub application: String,
    /// As written: RFC 3339 in UTC.
    pub date: String,
    pub metadata: Vec<(String, Json)>,
    pub elements: Vec<Element>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Element {
    pub name: String,
    pub description: String,
    pub color: Option<[u8; 4]>,
    pub metadata: Vec<(String, Json)>,
    pub attributes: Vec<Attribute>,
    pub geometry: Geometry,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Geometry {
    PointSet {
        origin: [f64; 3],
        vertices: Vec<[f64; 3]>,
    },
    LineSet {
        origin: [f64; 3],
        vertices: Vec<[f64; 3]>,
        segments: Vec<[u32; 2]>,
    },
    Surface {
        origin: [f64; 3],
        vertices: Vec<[f64; 3]>,
        triangles: Vec<[u32; 3]>,
    },
    GridSurface {
        orient: Orient2,
        grid: Grid2,
        heights: Option<Vec<f64>>,
    },
    BlockModel(BlockModel),
    Composite {
        elements: Vec<Element>,
    },
}

/// A plane: its origin and two orthogonal unit axes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Orient2 {
    pub origin: [f64; 3],
    pub u: [f64; 3],
    pub v: [f64; 3],
}

/// A frame: its origin and three orthogonal unit axes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Orient3 {
    pub origin: [f64; 3],
    pub u: [f64; 3],
    pub v: [f64; 3],
    pub w: [f64; 3],
}

#[derive(Debug, Clone, PartialEq)]
pub enum Grid2 {
    Regular { size: [f64; 2], count: [u32; 2] },
    Tensor { u: Vec<f64>, v: Vec<f64> },
}

impl Grid2 {
    #[must_use]
    pub fn count(&self) -> [u32; 2] {
        match self {
            Self::Regular { count, .. } => *count,
            // Each tensor axis was read from an array of at most `parquet::MAX_ROWS` rows.
            Self::Tensor { u, v } => [u.len() as u32, v.len() as u32],
        }
    }
}

/// A block model's parent grid: regular sizes, or one size per row, column and layer.
#[derive(Debug, Clone, PartialEq)]
pub enum Grid3 {
    Regular {
        size: [f64; 3],
        count: [u32; 3],
    },
    Tensor {
        u: Vec<f64>,
        v: Vec<f64>,
        w: Vec<f64>,
    },
}

impl Grid3 {
    /// Parent blocks along U, V and W.
    #[must_use]
    pub fn count(&self) -> [u32; 3] {
        match self {
            Self::Regular { count, .. } => *count,
            Self::Tensor { u, v, w } => [u.len() as u32, v.len() as u32, w.len() as u32],
        }
    }

    /// Parent blocks in all.
    #[must_use]
    pub fn blocks(&self) -> u64 {
        self.count().iter().map(|&n| u64::from(n)).product()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct BlockModel {
    pub orient: Orient3,
    pub grid: Grid3,
    pub subblocks: Option<Subblocks>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubblockMode {
    /// Each parent splits in halves, recursively, down to the sub-block count.
    Octree,
    /// Each parent is whole or split to the full sub-block count.
    Full,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Subblocks {
    /// Sub-blocks on a `count` grid inside each parent.
    Regular {
        count: [u32; 3],
        mode: Option<SubblockMode>,
        blocks: Vec<RegularSubblock>,
    },
    /// Sub-blocks anywhere inside each parent, in its [0, 1] coordinates.
    Freeform { blocks: Vec<FreeformSubblock> },
}

impl Subblocks {
    #[must_use]
    pub fn len(&self) -> usize {
        match self {
            Self::Regular { blocks, .. } => blocks.len(),
            Self::Freeform { blocks } => blocks.len(),
        }
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// A sub-block: its parent's indices and its corners on the sub-block grid, `min < max <= count`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RegularSubblock {
    pub parent: [u32; 3],
    pub min: [u32; 3],
    pub max: [u32; 3],
}

/// A sub-block: its parent's indices and its corners in the parent's [0, 1] coordinates, `min < max`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FreeformSubblock {
    pub parent: [u32; 3],
    pub min: [f64; 3],
    pub max: [f64; 3],
}

/// What an attribute's values attach to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Location {
    Vertices,
    Primitives,
    Subblocks,
    Elements,
    Projected,
    Categories,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Attribute {
    pub name: String,
    pub description: String,
    pub units: String,
    pub metadata: Vec<(String, Json)>,
    pub location: Location,
    pub data: AttributeData,
}

/// Nullable numbers of one kind.
#[derive(Debug, Clone, PartialEq)]
pub enum Numbers {
    /// 32-bit values are widened, exactly.
    Float(Vec<Option<f64>>),
    Integer(Vec<Option<i64>>),
    /// Days since 1970-01-01.
    Date(Vec<Option<i32>>),
    /// Microseconds since 1970-01-01T00:00:00Z.
    DateTime(Vec<Option<i64>>),
}

impl Numbers {
    #[must_use]
    pub fn len(&self) -> usize {
        match self {
            Self::Float(v) => v.len(),
            Self::Integer(v) => v.len(),
            Self::Date(v) => v.len(),
            Self::DateTime(v) => v.len(),
        }
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// A discrete colormap's boundaries, each with whether it is inclusive, in non-decreasing order.
#[derive(Debug, Clone, PartialEq)]
pub enum Boundaries {
    Float(Vec<(f64, bool)>),
    Integer(Vec<(i64, bool)>),
    Date(Vec<(i32, bool)>),
    DateTime(Vec<(i64, bool)>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Colormap {
    /// The range as written: numbers, or date and date-time strings.
    Continuous {
        min: Json,
        max: Json,
        gradient: Vec<[u8; 4]>,
    },
    Discrete {
        boundaries: Boundaries,
        gradient: Vec<[u8; 4]>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum Vectors {
    Two(Vec<Option<[f64; 2]>>),
    Three(Vec<Option<[f64; 3]>>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum AttributeData {
    Number {
        values: Numbers,
        colormap: Option<Colormap>,
    },
    Vector {
        values: Vectors,
    },
    Text {
        values: Vec<Option<String>>,
    },
    Category {
        values: Vec<Option<u32>>,
        names: Vec<String>,
        gradient: Option<Vec<[u8; 4]>>,
        attributes: Vec<Attribute>,
    },
    Boolean {
        values: Vec<Option<bool>>,
    },
    Color {
        values: Vec<Option<[u8; 4]>>,
    },
    /// The image's bytes, not decoded.
    MappedTexture {
        image: Vec<u8>,
        texcoords: Vec<[f64; 2]>,
    },
    ProjectedTexture {
        image: Vec<u8>,
        orient: Orient2,
        width: f64,
        height: f64,
    },
}

impl AttributeData {
    /// The number of values, the number of texture coordinates, or `None` for a projected texture.
    #[must_use]
    pub fn item_count(&self) -> Option<usize> {
        Some(match self {
            Self::Number { values, .. } => values.len(),
            Self::Vector {
                values: Vectors::Two(v),
            } => v.len(),
            Self::Vector {
                values: Vectors::Three(v),
            } => v.len(),
            Self::Text { values } => values.len(),
            Self::Category { values, .. } => values.len(),
            Self::Boolean { values } => values.len(),
            Self::Color { values } => values.len(),
            Self::MappedTexture { texcoords, .. } => texcoords.len(),
            Self::ProjectedTexture { .. } => return None,
        })
    }
}

fn bad(detail: impl std::fmt::Display) -> Refusal {
    Refusal::new(
        RefusalCode::InputMissingOrUnreadable506,
        format!("OMF: {detail}"),
    )
}

/// Reads the OMF 2 file at `path`.
pub fn read_path(path: &Path) -> Result<Project, Refusal> {
    let bytes = std::fs::read(path).map_err(|e| bad(format!("{}: {e}", path.display())))?;
    read(&bytes)
}

/// Reads an OMF 2 file's bytes.
pub fn read(bytes: &[u8]) -> Result<Project, Refusal> {
    read_all(bytes).map_err(bad)
}

fn read_all(bytes: &[u8]) -> Result<Project, String> {
    let comment = archive_comment(bytes).map_err(|e| e.to_string())?;
    let format =
        String::from_utf8(comment.to_vec()).map_err(|_| "the archive comment is not UTF-8")?;
    let version = format
        .strip_prefix(FORMAT)
        .ok_or_else(|| format!("not an OMF file: the archive comment is {format:?}"))?;
    if !version.starts_with("2.") {
        return Err(format!("OMF version {version}; this reader reads 2.x"));
    }
    let members = read_stored_entries(bytes, MAX_MEMBER_BYTES, MAX_ARCHIVE_BYTES)
        .map_err(|e| e.to_string())?;
    let index = members
        .get(INDEX)
        .ok_or("the archive has no index.json.gz")?;
    let index = gzip::gunzip(index, MAX_INDEX_BYTES).map_err(|e| format!("{INDEX}: {e}"))?;
    let index = json::parse(&index).map_err(|e| format!("{INDEX}: {e}"))?;
    Reader { members: &members }.project(&index, format)
}

// --- Index fields ---

fn object<'j>(j: &'j Json, at: &str) -> Result<&'j [(String, Json)], String> {
    j.as_object()
        .ok_or_else(|| format!("{at} is not an object"))
}

fn field<'j>(j: &'j Json, key: &str, at: &str) -> Result<&'j Json, String> {
    j.get(key)
        .ok_or_else(|| format!("{at}: \"{key}\" is missing"))
}

/// A string member, "" when absent.
fn text(j: &Json, key: &str, at: &str) -> Result<String, String> {
    match j.get(key) {
        None => Ok(String::new()),
        Some(v) => v
            .as_str()
            .map(str::to_owned)
            .ok_or_else(|| format!("{at}: \"{key}\" is not a string")),
    }
}

fn required_text(j: &Json, key: &str, at: &str) -> Result<String, String> {
    field(j, key, at)?
        .as_str()
        .map(str::to_owned)
        .ok_or_else(|| format!("{at}: \"{key}\" is not a string"))
}

fn kind<'j>(j: &'j Json, at: &str) -> Result<&'j str, String> {
    field(j, "type", at)?
        .as_str()
        .ok_or_else(|| format!("{at}: \"type\" is not a string"))
}

fn metadata(j: &Json, at: &str) -> Result<Vec<(String, Json)>, String> {
    match j.get("metadata") {
        None => Ok(Vec::new()),
        Some(m) => object(m, &format!("{at}.metadata")).map(<[_]>::to_vec),
    }
}

fn floats<const N: usize>(j: &Json, at: &str) -> Result<[f64; N], String> {
    let items = j
        .as_array()
        .filter(|a| a.len() == N)
        .ok_or_else(|| format!("{at} is not {N} numbers"))?;
    let mut out = [0.0; N];
    for (o, v) in out.iter_mut().zip(items) {
        *o = v
            .f64()
            .ok_or_else(|| format!("{at} is not {N} finite numbers"))?;
    }
    Ok(out)
}

fn finite_floats<const N: usize>(j: &Json, at: &str) -> Result<[f64; N], String> {
    floats(j, at)
}

fn counts<const N: usize>(j: &Json, at: &str) -> Result<[u32; N], String> {
    let items = j
        .as_array()
        .filter(|a| a.len() == N)
        .ok_or_else(|| format!("{at} is not {N} counts"))?;
    let mut out = [0; N];
    for (o, v) in out.iter_mut().zip(items) {
        *o = v
            .u64()
            .and_then(|n| u32::try_from(n).ok())
            .ok_or_else(|| format!("{at} is not {N} whole numbers below 2^32"))?;
    }
    Ok(out)
}

fn rgba(j: &Json, at: &str) -> Result<[u8; 4], String> {
    let items = j
        .as_array()
        .filter(|a| a.len() == 4)
        .ok_or_else(|| format!("{at} is not 4 color channels"))?;
    let mut out = [0; 4];
    for (o, v) in out.iter_mut().zip(items) {
        *o = v
            .u64()
            .and_then(|n| u8::try_from(n).ok())
            .ok_or_else(|| format!("{at} is not 4 channels in 0..=255"))?;
    }
    Ok(out)
}

fn origin(j: &Json, at: &str) -> Result<[f64; 3], String> {
    match j.get("origin") {
        None => Ok([0.0; 3]),
        Some(o) => finite_floats(o, &format!("{at}.origin")),
    }
}

fn unit(v: [f64; 3], at: &str) -> Result<[f64; 3], String> {
    let m2 = v[0] * v[0] + v[1] * v[1] + v[2] * v[2];
    if (1.0 - m2).abs() < 1e-6 {
        Ok(v)
    } else {
        Err(format!("{at} {v:?} is not a unit vector"))
    }
}

fn orthogonal(a: [f64; 3], b: [f64; 3], at: &str) -> Result<(), String> {
    let n = |v: [f64; 3]| {
        let m = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
        [v[0] / m, v[1] / m, v[2] / m]
    };
    let (a1, b1) = (n(a), n(b));
    if (a1[0] * b1[0] + a1[1] * b1[1] + a1[2] * b1[2]).abs() < 1e-6 {
        Ok(())
    } else {
        Err(format!("{at}: axes {a:?} and {b:?} are not orthogonal"))
    }
}

fn axis(j: &Json, key: &str, default: [f64; 3], at: &str) -> Result<[f64; 3], String> {
    match j.get(key) {
        None => Ok(default),
        Some(v) => unit(floats(v, &format!("{at}.{key}"))?, &format!("{at}.{key}")),
    }
}

const I: [f64; 3] = [1.0, 0.0, 0.0];
const J: [f64; 3] = [0.0, 1.0, 0.0];
const K: [f64; 3] = [0.0, 0.0, 1.0];

fn orient2(j: &Json, at: &str) -> Result<Orient2, String> {
    let o = Orient2 {
        origin: finite_floats(field(j, "origin", at)?, &format!("{at}.origin"))?,
        u: axis(j, "u", I, at)?,
        v: axis(j, "v", J, at)?,
    };
    orthogonal(o.u, o.v, at)?;
    Ok(o)
}

fn orient3(j: &Json, at: &str) -> Result<Orient3, String> {
    let o = Orient3 {
        origin: finite_floats(field(j, "origin", at)?, &format!("{at}.origin"))?,
        u: axis(j, "u", I, at)?,
        v: axis(j, "v", J, at)?,
        w: axis(j, "w", K, at)?,
    };
    orthogonal(o.u, o.v, at)?;
    orthogonal(o.u, o.w, at)?;
    orthogonal(o.v, o.w, at)?;
    Ok(o)
}

fn positive(values: &[f64], at: &str) -> Result<(), String> {
    match values.iter().position(|x| !(x.is_finite() && *x > 0.0)) {
        None => Ok(()),
        Some(i) => Err(format!(
            "{at}[{i}] = {} is not a finite size above zero",
            values[i]
        )),
    }
}

fn nonzero(counts: &[u32], at: &str) -> Result<(), String> {
    if counts.contains(&0) {
        Err(format!("{at} {counts:?} has a zero count"))
    } else {
        Ok(())
    }
}

fn location(name: &str, at: &str) -> Result<Location, String> {
    Ok(match name {
        "Vertices" => Location::Vertices,
        "Primitives" => Location::Primitives,
        "Subblocks" => Location::Subblocks,
        "Elements" => Location::Elements,
        "Projected" => Location::Projected,
        "Categories" => Location::Categories,
        other => return Err(format!("{at}: location {other:?} is unknown")),
    })
}

/// How many values an attribute at `loc` needs on `g`, or `None` where the location does not apply.
fn location_len(g: &Geometry, loc: Location) -> Option<u64> {
    let corners = |c: &[u32]| c.iter().map(|&n| u64::from(n) + 1).product::<u64>();
    let cells = |c: &[u32]| c.iter().map(|&n| u64::from(n)).product::<u64>();
    match (g, loc) {
        (Geometry::PointSet { vertices, .. }, Location::Vertices)
        | (Geometry::LineSet { vertices, .. }, Location::Vertices)
        | (Geometry::Surface { vertices, .. }, Location::Vertices) => Some(vertices.len() as u64),
        (Geometry::LineSet { segments, .. }, Location::Primitives) => Some(segments.len() as u64),
        (Geometry::Surface { triangles, .. }, Location::Primitives) => Some(triangles.len() as u64),
        (Geometry::GridSurface { grid, .. }, Location::Vertices) => Some(corners(&grid.count())),
        (Geometry::GridSurface { grid, .. }, Location::Primitives) => Some(cells(&grid.count())),
        (Geometry::BlockModel(b), Location::Vertices) => Some(corners(&b.grid.count())),
        (Geometry::BlockModel(b), Location::Primitives) => Some(b.grid.blocks()),
        (Geometry::BlockModel(b), Location::Subblocks) => {
            b.subblocks.as_ref().map(|s| s.len() as u64)
        }
        (Geometry::Composite { elements }, Location::Elements) => Some(elements.len() as u64),
        _ => None,
    }
}

// --- Array columns ---

fn names_of(t: &Table) -> Vec<String> {
    t.columns.iter().map(|c| c.leaf.path.join(".")).collect()
}

fn columns<'t>(t: &'t Table, names: &[&str], at: &str) -> Result<&'t [Column], String> {
    let got = names_of(t);
    if got != names {
        return Err(format!("{at}: columns {got:?}, where OMF has {names:?}"));
    }
    Ok(&t.columns)
}

fn name(c: &Column) -> String {
    c.leaf.path.join(".")
}

fn required(c: &Column, at: &str) -> Result<(), String> {
    if c.leaf.optional {
        Err(format!(
            "{at}: column {} is optional where OMF requires it",
            name(c)
        ))
    } else {
        Ok(())
    }
}

fn presence<'c>(c: &'c Column, at: &str) -> Result<&'c [bool], String> {
    c.present.as_deref().ok_or_else(|| {
        format!(
            "{at}: column {} is required where OMF has it optional",
            name(c)
        )
    })
}

fn nullable<T>(present: &[bool], values: Vec<T>) -> Vec<Option<T>> {
    let mut it = values.into_iter();
    present
        .iter()
        .map(|&p| if p { it.next() } else { None })
        .collect()
}

fn float_values(c: &Column, at: &str) -> Result<Vec<f64>, String> {
    match (&c.values, c.leaf.logical) {
        (Values::Float(v), Logical::None) => Ok(v.iter().map(|&x| f64::from(x)).collect()),
        (Values::Double(v), Logical::None) => Ok(v.clone()),
        _ => Err(format!("{at}: column {} is not float or double", name(c))),
    }
}

fn unsigned(c: &Column, bits: u8, at: &str) -> Result<Vec<u32>, String> {
    match (&c.values, c.leaf.logical) {
        (
            Values::Int32(v),
            Logical::Integer {
                bits: b,
                signed: false,
            },
        ) if b == bits => v
            .iter()
            // The bits of an unsigned 32-bit integer, as Parquet stores it in INT32.
            .map(|&x| x as u32)
            .map(|u| {
                if bits == 8 && u > 255 {
                    Err(format!(
                        "{at}: column {} holds {u}, outside 8 bits",
                        name(c)
                    ))
                } else {
                    Ok(u)
                }
            })
            .collect(),
        _ => Err(format!(
            "{at}: column {} is not unsigned {bits}-bit",
            name(c)
        )),
    }
}

fn strings(c: &Column, at: &str) -> Result<Vec<String>, String> {
    match (&c.values, c.leaf.logical) {
        (Values::Bytes(v), Logical::String) => v
            .iter()
            .map(|b| {
                String::from_utf8(b.clone())
                    .map_err(|_| format!("{at}: column {} holds text that is not UTF-8", name(c)))
            })
            .collect(),
        _ => Err(format!("{at}: column {} is not a string", name(c))),
    }
}

fn booleans(c: &Column, at: &str) -> Result<Vec<bool>, String> {
    match (&c.values, c.leaf.logical) {
        (Values::Boolean(v), Logical::None) => Ok(v.clone()),
        _ => Err(format!("{at}: column {} is not boolean", name(c))),
    }
}

/// The columns' float width must agree, as each of OMF's float schemas has one width throughout.
fn one_width(cs: &[Column], at: &str) -> Result<(), String> {
    let first = cs.first().map(|c| c.leaf.physical);
    if cs.iter().all(|c| Some(c.leaf.physical) == first) {
        Ok(())
    } else {
        Err(format!("{at}: float columns of mixed widths"))
    }
}

fn zip3<T: Copy>(a: &[T], b: &[T], c: &[T]) -> Vec<[T; 3]> {
    a.iter()
        .zip(b)
        .zip(c)
        .map(|((x, y), z)| [*x, *y, *z])
        .collect()
}

/// Typed values of a number column, by its physical and logical type.
enum Typed {
    Float(Vec<f64>),
    Integer(Vec<i64>),
    Date(Vec<i32>),
    DateTime(Vec<i64>),
}

fn typed(c: &Column, at: &str) -> Result<Typed, String> {
    Ok(match (&c.values, c.leaf.logical) {
        (Values::Float(v), Logical::None) => {
            Typed::Float(v.iter().map(|&x| f64::from(x)).collect())
        }
        (Values::Double(v), Logical::None) => Typed::Float(v.clone()),
        (
            Values::Int64(v),
            Logical::None
            | Logical::Integer {
                bits: 64,
                signed: true,
            },
        ) => Typed::Integer(v.clone()),
        (Values::Int32(v), Logical::Date) => Typed::Date(v.clone()),
        (Values::Int64(v), Logical::TimestampMicrosUtc) => Typed::DateTime(v.clone()),
        _ => {
            return Err(format!(
                "{at}: column {} is not a float, int64, date or date-time",
                name(c)
            ));
        }
    })
}

struct Reader<'m> {
    members: &'m BTreeMap<String, Vec<u8>>,
}

impl Reader<'_> {
    fn member(&self, j: &Json, at: &str) -> Result<(&[u8], String, u64), String> {
        let file = required_text(j, "filename", at)?;
        let count = field(j, "item_count", at)?
            .u64()
            .ok_or_else(|| format!("{at}: \"item_count\" is not a whole number"))?;
        let bytes = self
            .members
            .get(&file)
            .ok_or_else(|| format!("{at} refers to non-existent archive member '{file}'"))?;
        Ok((bytes, file, count))
    }

    fn table(&self, j: &Json, at: &str) -> Result<Table, String> {
        let (bytes, file, count) = self.member(j, at)?;
        if !file.ends_with(".parquet") {
            return Err(format!(
                "{at}: array member '{file}' is not a .parquet file"
            ));
        }
        let t = parquet::read(bytes).map_err(|e| format!("{at} ('{file}'): {e}"))?;
        if t.rows as u64 != count {
            return Err(format!(
                "{at} ('{file}'): array length {} does not match the declared length {count}",
                t.rows
            ));
        }
        Ok(t)
    }

    fn image(&self, j: &Json, at: &str) -> Result<Vec<u8>, String> {
        let (bytes, file, _) = self.member(j, at)?;
        if bytes.starts_with(b"\x89PNG\r\n\x1a\n") || bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
            Ok(bytes.to_vec())
        } else {
            Err(format!("{at}: member '{file}' is neither PNG nor JPEG"))
        }
    }

    fn scalars(&self, j: &Json, at: &str) -> Result<Vec<f64>, String> {
        let t = self.table(j, at)?;
        let c = columns(&t, &["scalar"], at)?;
        required(&c[0], at)?;
        float_values(&c[0], at)
    }

    fn vertices(&self, j: &Json, at: &str) -> Result<Vec<[f64; 3]>, String> {
        let t = self.table(j, at)?;
        let c = columns(&t, &["x", "y", "z"], at)?;
        one_width(c, at)?;
        let mut v = Vec::new();
        for col in c {
            required(col, at)?;
            v.push(float_values(col, at)?);
        }
        Ok(zip3(&v[0], &v[1], &v[2]))
    }

    /// Vertex indices, each below `vertices`.
    fn indices<const N: usize>(
        &self,
        j: &Json,
        names: [&str; N],
        vertices: usize,
        at: &str,
    ) -> Result<Vec<[u32; N]>, String> {
        let t = self.table(j, at)?;
        let c = columns(&t, &names, at)?;
        let mut cols = Vec::new();
        for col in c {
            required(col, at)?;
            cols.push(unsigned(col, 32, at)?);
        }
        (0..t.rows)
            .map(|r| {
                let mut out = [0_u32; N];
                for (k, o) in out.iter_mut().enumerate() {
                    *o = cols[k][r];
                    if *o as usize >= vertices {
                        return Err(format!(
                            "{at}[{r}]: vertex index {o} is not below the {vertices} vertices"
                        ));
                    }
                }
                Ok(out)
            })
            .collect()
    }

    fn names(&self, j: &Json, at: &str) -> Result<Vec<String>, String> {
        let t = self.table(j, at)?;
        let c = columns(&t, &["name"], at)?;
        required(&c[0], at)?;
        strings(&c[0], at)
    }

    fn rgba_columns(&self, c: &[Column], at: &str) -> Result<Vec<[u8; 4]>, String> {
        let mut ch = Vec::new();
        for col in c {
            ch.push(unsigned(col, 8, at)?);
        }
        Ok((0..ch[0].len())
            .map(|r| {
                [
                    ch[0][r] as u8,
                    ch[1][r] as u8,
                    ch[2][r] as u8,
                    ch[3][r] as u8,
                ]
            })
            .collect())
    }

    fn gradient(&self, j: &Json, at: &str) -> Result<Vec<[u8; 4]>, String> {
        let t = self.table(j, at)?;
        let c = columns(&t, &["r", "g", "b", "a"], at)?;
        for col in c {
            required(col, at)?;
        }
        self.rgba_columns(c, at)
    }

    fn texcoords(&self, j: &Json, at: &str) -> Result<Vec<[f64; 2]>, String> {
        let t = self.table(j, at)?;
        let c = columns(&t, &["u", "v"], at)?;
        one_width(c, at)?;
        required(&c[0], at)?;
        required(&c[1], at)?;
        let (u, v) = (float_values(&c[0], at)?, float_values(&c[1], at)?);
        Ok(u.into_iter().zip(v).map(|(a, b)| [a, b]).collect())
    }

    fn boundaries(&self, j: &Json, at: &str) -> Result<Boundaries, String> {
        let t = self.table(j, at)?;
        let c = columns(&t, &["value", "inclusive"], at)?;
        required(&c[0], at)?;
        required(&c[1], at)?;
        let inclusive = booleans(&c[1], at)?;
        fn ordered<T: PartialOrd>(
            v: Vec<T>,
            inclusive: &[bool],
            at: &str,
        ) -> Result<Vec<(T, bool)>, String> {
            match v.windows(2).position(|w| w[0] > w[1]) {
                None => Ok(v.into_iter().zip(inclusive.iter().copied()).collect()),
                Some(i) => Err(format!("{at}: boundary {} decreases", i + 1)),
            }
        }
        Ok(match typed(&c[0], at)? {
            Typed::Float(v) => {
                // The vendor reader lets NaN through, as it compares as neither less nor more.
                if let Some(i) = v.iter().position(|x| x.is_nan()) {
                    return Err(format!("{at}: boundary {i} is not a number"));
                }
                Boundaries::Float(ordered(v, &inclusive, at)?)
            }
            Typed::Integer(v) => Boundaries::Integer(ordered(v, &inclusive, at)?),
            Typed::Date(v) => Boundaries::Date(ordered(v, &inclusive, at)?),
            Typed::DateTime(v) => Boundaries::DateTime(ordered(v, &inclusive, at)?),
        })
    }

    fn numbers(&self, j: &Json, at: &str) -> Result<Numbers, String> {
        let t = self.table(j, at)?;
        let c = columns(&t, &["number"], at)?;
        let p = presence(&c[0], at)?;
        Ok(match typed(&c[0], at)? {
            Typed::Float(v) => Numbers::Float(nullable(p, v)),
            Typed::Integer(v) => Numbers::Integer(nullable(p, v)),
            Typed::Date(v) => Numbers::Date(nullable(p, v)),
            Typed::DateTime(v) => Numbers::DateTime(nullable(p, v)),
        })
    }

    fn colormap(&self, j: &Json, at: &str) -> Result<Colormap, String> {
        let gradient = self.gradient(field(j, "gradient", at)?, &format!("{at}.gradient"))?;
        match kind(j, at)? {
            "Continuous" => {
                let range = field(j, "range", at)?;
                // Kept when the minimum is above the maximum: the vendor reader does not check the order.
                let (min, max) = (
                    field(range, "min", at)?.clone(),
                    field(range, "max", at)?.clone(),
                );
                Ok(Colormap::Continuous { min, max, gradient })
            }
            "Discrete" => Ok(Colormap::Discrete {
                boundaries: self
                    .boundaries(field(j, "boundaries", at)?, &format!("{at}.boundaries"))?,
                gradient,
            }),
            other => Err(format!("{at}: colormap type {other:?} is unknown")),
        }
    }

    fn vectors(&self, j: &Json, at: &str) -> Result<Vectors, String> {
        let t = self.table(j, at)?;
        let names = names_of(&t);
        let three = names == ["vector.x", "vector.y", "vector.z"];
        if !three && names != ["vector.x", "vector.y"] {
            return Err(format!(
                "{at}: columns {names:?}, where OMF has vector.x, vector.y and maybe vector.z"
            ));
        }
        one_width(&t.columns, at)?;
        let p = presence(&t.columns[0], at)?;
        let mut v = Vec::new();
        for c in &t.columns {
            if presence(c, at)? != p {
                return Err(format!(
                    "{at}: the vector's columns disagree on which rows are null"
                ));
            }
            v.push(float_values(c, at)?);
        }
        Ok(if three {
            Vectors::Three(nullable(p, zip3(&v[0], &v[1], &v[2])))
        } else {
            Vectors::Two(nullable(
                p,
                v[0].iter().zip(&v[1]).map(|(a, b)| [*a, *b]).collect(),
            ))
        })
    }

    fn texts(&self, j: &Json, at: &str) -> Result<Vec<Option<String>>, String> {
        let t = self.table(j, at)?;
        let c = columns(&t, &["text"], at)?;
        Ok(nullable(presence(&c[0], at)?, strings(&c[0], at)?))
    }

    fn bools(&self, j: &Json, at: &str) -> Result<Vec<Option<bool>>, String> {
        let t = self.table(j, at)?;
        let c = columns(&t, &["bool"], at)?;
        Ok(nullable(presence(&c[0], at)?, booleans(&c[0], at)?))
    }

    fn colors(&self, j: &Json, at: &str) -> Result<Vec<Option<[u8; 4]>>, String> {
        let t = self.table(j, at)?;
        let c = columns(&t, &["color.r", "color.g", "color.b", "color.a"], at)?;
        let p = presence(&c[0], at)?;
        for col in c {
            if presence(col, at)? != p {
                return Err(format!(
                    "{at}: the color's columns disagree on which rows are null"
                ));
            }
        }
        Ok(nullable(p, self.rgba_columns(c, at)?))
    }

    fn category(&self, j: &Json, at: &str) -> Result<AttributeData, String> {
        let names = self.names(field(j, "names", at)?, &format!("{at}.names"))?;
        let vat = format!("{at}.values");
        let t = self.table(field(j, "values", at)?, &vat)?;
        let c = columns(&t, &["index"], &vat)?;
        let p = presence(&c[0], &vat)?;
        let idx = unsigned(&c[0], 32, &vat)?;
        if let Some(bad) = idx.iter().find(|&&i| i as usize >= names.len()) {
            return Err(format!(
                "{vat}: category index {bad} is not below the {} names",
                names.len()
            ));
        }
        let gradient = match j.get("gradient") {
            None | Some(Json::Null) => None,
            Some(g) => Some(self.gradient(g, &format!("{at}.gradient"))?),
        };
        let n = names.len() as u64;
        let attributes =
            self.attributes(j, at, &|loc| (loc == Location::Categories).then_some(n))?;
        Ok(AttributeData::Category {
            values: nullable(p, idx),
            names,
            gradient,
            attributes,
        })
    }

    fn data(&self, j: &Json, at: &str) -> Result<AttributeData, String> {
        let values = |at: &str| field(j, "values", at);
        let vat = format!("{at}.values");
        Ok(match kind(j, at)? {
            "Number" => AttributeData::Number {
                values: self.numbers(values(at)?, &vat)?,
                colormap: match j.get("colormap") {
                    None | Some(Json::Null) => None,
                    Some(m) => Some(self.colormap(m, &format!("{at}.colormap"))?),
                },
            },
            "Vector" => AttributeData::Vector {
                values: self.vectors(values(at)?, &vat)?,
            },
            "Text" => AttributeData::Text {
                values: self.texts(values(at)?, &vat)?,
            },
            "Category" => self.category(j, at)?,
            "Boolean" => AttributeData::Boolean {
                values: self.bools(values(at)?, &vat)?,
            },
            "Color" => AttributeData::Color {
                values: self.colors(values(at)?, &vat)?,
            },
            "MappedTexture" => AttributeData::MappedTexture {
                image: self.image(field(j, "image", at)?, &format!("{at}.image"))?,
                texcoords: self
                    .texcoords(field(j, "texcoords", at)?, &format!("{at}.texcoords"))?,
            },
            "ProjectedTexture" => {
                let size = |key: &str| -> Result<f64, String> {
                    let x = field(j, key, at)?
                        .f64()
                        .ok_or_else(|| format!("{at}: \"{key}\" is not a number"))?;
                    positive(&[x], &format!("{at}.{key}"))?;
                    Ok(x)
                };
                AttributeData::ProjectedTexture {
                    image: self.image(field(j, "image", at)?, &format!("{at}.image"))?,
                    orient: orient2(field(j, "orient", at)?, &format!("{at}.orient"))?,
                    width: size("width")?,
                    height: size("height")?,
                }
            }
            other => return Err(format!("{at}: attribute type {other:?} is unknown")),
        })
    }

    /// The `attributes` of `j`, each checked against `len_of`, which gives a location's length or `None` where it
    /// does not apply.
    fn attributes(
        &self,
        j: &Json,
        at: &str,
        len_of: &dyn Fn(Location) -> Option<u64>,
    ) -> Result<Vec<Attribute>, String> {
        let Some(list) = j.get("attributes") else {
            return Ok(Vec::new());
        };
        let list = list
            .as_array()
            .ok_or_else(|| format!("{at}.attributes is not an array"))?;
        let mut out = Vec::new();
        for (i, a) in list.iter().enumerate() {
            let name = required_text(a, "name", &format!("{at}.attributes[{i}]"))?;
            let aat = format!("{at}.attributes[{i}] \"{name}\"");
            let location = location(&required_text(a, "location", &aat)?, &aat)?;
            let data = self.data(field(a, "data", &aat)?, &format!("{aat}.data"))?;
            match (data.item_count(), location) {
                (None, Location::Projected) => {}
                (None, _) | (Some(_), Location::Projected) => {
                    return Err(format!(
                        "{aat}: a projected texture goes with location Projected, and only it"
                    ));
                }
                (Some(n), loc) => {
                    let want = len_of(loc)
                        .ok_or_else(|| format!("{aat}: location {loc:?} does not apply here"))?;
                    if n as u64 != want {
                        return Err(format!(
                            "{aat}: {n} values where location {loc:?} has {want}"
                        ));
                    }
                }
            }
            out.push(Attribute {
                name,
                description: text(a, "description", &aat)?,
                units: text(a, "units", &aat)?,
                metadata: metadata(a, &aat)?,
                location,
                data,
            });
        }
        Ok(out)
    }

    fn regular_subblocks(
        &self,
        j: &Json,
        parents: [u32; 3],
        count: [u32; 3],
        mode: Option<SubblockMode>,
        at: &str,
    ) -> Result<Vec<RegularSubblock>, String> {
        const NAMES: [&str; 9] = [
            "parent_u",
            "parent_v",
            "parent_w",
            "corner_min_u",
            "corner_min_v",
            "corner_min_w",
            "corner_max_u",
            "corner_max_v",
            "corner_max_w",
        ];
        let t = self.table(j, at)?;
        let c = columns(&t, &NAMES, at)?;
        let mut cols = Vec::new();
        for col in c {
            required(col, at)?;
            cols.push(unsigned(col, 32, at)?);
        }
        // The sizes a sub-block may have in each mode (omf-rust's `valid_subblock_sizes`).
        let sizes: Option<(SubblockMode, Vec<[u32; 3]>)> = mode.map(|m| match m {
            SubblockMode::Octree => {
                let mut s = vec![count];
                while s.last() != Some(&[1, 1, 1]) {
                    let l = s[s.len() - 1];
                    s.push(l.map(|n| (n / 2).max(1)));
                }
                (m, s)
            }
            SubblockMode::Full => (m, vec![[1, 1, 1], count]),
        });
        (0..t.rows)
            .map(|r| {
                let b = RegularSubblock {
                    parent: [cols[0][r], cols[1][r], cols[2][r]],
                    min: [cols[3][r], cols[4][r], cols[5][r]],
                    max: [cols[6][r], cols[7][r], cols[8][r]],
                };
                let here = format!("{at}[{r}]");
                for i in 0..3 {
                    if b.parent[i] >= parents[i] {
                        return Err(format!(
                            "{here}: parent {:?} outside the {parents:?} parent blocks",
                            b.parent
                        ));
                    }
                    if b.min[i] >= b.max[i] {
                        return Err(format!(
                            "{here}: corners {:?} to {:?} have no size",
                            b.min, b.max
                        ));
                    }
                    if b.max[i] > count[i] {
                        return Err(format!(
                            "{here}: corner {:?} outside the {count:?} sub-blocks",
                            b.max
                        ));
                    }
                }
                if let Some((m, sizes)) = &sizes {
                    let size = [
                        b.max[0] - b.min[0],
                        b.max[1] - b.min[1],
                        b.max[2] - b.min[2],
                    ];
                    let aligned = *m != SubblockMode::Octree
                        || (0..3).all(|i| b.min[i].is_multiple_of(size[i]));
                    if !sizes.contains(&size) || !aligned {
                        return Err(format!(
                            "{here}: corners {:?} to {:?} are not a sub-block of mode {m:?}",
                            b.min, b.max
                        ));
                    }
                }
                Ok(b)
            })
            .collect()
    }

    fn freeform_subblocks(
        &self,
        j: &Json,
        parents: [u32; 3],
        at: &str,
    ) -> Result<Vec<FreeformSubblock>, String> {
        const NAMES: [&str; 9] = [
            "parent_u",
            "parent_v",
            "parent_w",
            "corner_min_u",
            "corner_min_v",
            "corner_min_w",
            "corner_max_u",
            "corner_max_v",
            "corner_max_w",
        ];
        let t = self.table(j, at)?;
        let c = columns(&t, &NAMES, at)?;
        let mut p = Vec::new();
        for col in &c[..3] {
            required(col, at)?;
            p.push(unsigned(col, 32, at)?);
        }
        one_width(&c[3..], at)?;
        let mut f = Vec::new();
        for col in &c[3..] {
            required(col, at)?;
            f.push(float_values(col, at)?);
        }
        (0..t.rows)
            .map(|r| {
                let b = FreeformSubblock {
                    parent: [p[0][r], p[1][r], p[2][r]],
                    min: [f[0][r], f[1][r], f[2][r]],
                    max: [f[3][r], f[4][r], f[5][r]],
                };
                let here = format!("{at}[{r}]");
                for i in 0..3 {
                    if b.parent[i] >= parents[i] {
                        return Err(format!(
                            "{here}: parent {:?} outside the {parents:?} parent blocks",
                            b.parent
                        ));
                    }
                    // NaN is outside [0, 1].
                    let unit = 0.0..=1.0;
                    if !(unit.contains(&b.min[i]) && unit.contains(&b.max[i])) {
                        return Err(format!(
                            "{here}: corners {:?} to {:?} outside the parent",
                            b.min, b.max
                        ));
                    }
                    if b.min[i] >= b.max[i] {
                        return Err(format!(
                            "{here}: corners {:?} to {:?} have no size",
                            b.min, b.max
                        ));
                    }
                }
                Ok(b)
            })
            .collect()
    }

    fn block_model(&self, j: &Json, at: &str) -> Result<BlockModel, String> {
        let orient = orient3(field(j, "orient", at)?, &format!("{at}.orient"))?;
        let gj = field(j, "grid", at)?;
        let gat = format!("{at}.grid");
        let grid = match kind(gj, &gat)? {
            "Regular" => {
                let size = floats::<3>(field(gj, "size", &gat)?, &format!("{gat}.size"))?;
                positive(&size, &format!("{gat}.size"))?;
                let count = counts::<3>(field(gj, "count", &gat)?, &format!("{gat}.count"))?;
                nonzero(&count, &format!("{gat}.count"))?;
                Grid3::Regular { size, count }
            }
            "Tensor" => {
                let axis = |k: &str| -> Result<Vec<f64>, String> {
                    let kat = format!("{gat}.{k}");
                    let v = self.scalars(field(gj, k, &gat)?, &kat)?;
                    positive(&v, &kat)?;
                    if v.is_empty() {
                        return Err(format!("{kat} is empty"));
                    }
                    Ok(v)
                };
                Grid3::Tensor {
                    u: axis("u")?,
                    v: axis("v")?,
                    w: axis("w")?,
                }
            }
            other => return Err(format!("{gat}: grid type {other:?} is unknown")),
        };
        let parents = grid.count();
        let subblocks = match j.get("subblocks") {
            None | Some(Json::Null) => None,
            Some(s) => {
                let sat = format!("{at}.subblocks");
                let arr = field(s, "subblocks", &sat)?;
                let aat = format!("{sat}.subblocks");
                Some(match kind(s, &sat)? {
                    "Regular" => {
                        let count = counts::<3>(field(s, "count", &sat)?, &format!("{sat}.count"))?;
                        nonzero(&count, &format!("{sat}.count"))?;
                        let mode = match s.get("mode") {
                            None | Some(Json::Null) => None,
                            Some(m) => match m.as_str() {
                                Some("Octree") => Some(SubblockMode::Octree),
                                Some("Full") => Some(SubblockMode::Full),
                                Some(other) => {
                                    return Err(format!(
                                        "{sat}: sub-block mode {other:?} is unknown"
                                    ));
                                }
                                None => {
                                    return Err(format!("{sat}: sub-block mode is not a string"));
                                }
                            },
                        };
                        if mode == Some(SubblockMode::Octree)
                            && !count.iter().all(|n| n.is_power_of_two())
                        {
                            return Err(format!(
                                "{sat}: octree sub-block count {count:?} is not a power of two on every axis"
                            ));
                        }
                        Subblocks::Regular {
                            count,
                            mode,
                            blocks: self.regular_subblocks(arr, parents, count, mode, &aat)?,
                        }
                    }
                    "Freeform" => Subblocks::Freeform {
                        blocks: self.freeform_subblocks(arr, parents, &aat)?,
                    },
                    other => return Err(format!("{sat}: sub-block type {other:?} is unknown")),
                })
            }
        };
        Ok(BlockModel {
            orient,
            grid,
            subblocks,
        })
    }

    fn geometry(&self, j: &Json, at: &str) -> Result<Geometry, String> {
        let vertices =
            |at: &str| self.vertices(field(j, "vertices", at)?, &format!("{at}.vertices"));
        Ok(match kind(j, at)? {
            "PointSet" => Geometry::PointSet {
                origin: origin(j, at)?,
                vertices: vertices(at)?,
            },
            "LineSet" => {
                let v = vertices(at)?;
                let segments = self.indices(
                    field(j, "segments", at)?,
                    ["a", "b"],
                    v.len(),
                    &format!("{at}.segments"),
                )?;
                Geometry::LineSet {
                    origin: origin(j, at)?,
                    vertices: v,
                    segments,
                }
            }
            "Surface" => {
                let v = vertices(at)?;
                let triangles = self.indices(
                    field(j, "triangles", at)?,
                    ["a", "b", "c"],
                    v.len(),
                    &format!("{at}.triangles"),
                )?;
                Geometry::Surface {
                    origin: origin(j, at)?,
                    vertices: v,
                    triangles,
                }
            }
            "GridSurface" => {
                let orient = orient2(field(j, "orient", at)?, &format!("{at}.orient"))?;
                let gj = field(j, "grid", at)?;
                let gat = format!("{at}.grid");
                let grid = match kind(gj, &gat)? {
                    "Regular" => {
                        let size = floats::<2>(field(gj, "size", &gat)?, &format!("{gat}.size"))?;
                        positive(&size, &format!("{gat}.size"))?;
                        let count =
                            counts::<2>(field(gj, "count", &gat)?, &format!("{gat}.count"))?;
                        nonzero(&count, &format!("{gat}.count"))?;
                        Grid2::Regular { size, count }
                    }
                    "Tensor" => {
                        let axis = |k: &str| -> Result<Vec<f64>, String> {
                            let kat = format!("{gat}.{k}");
                            let v = self.scalars(field(gj, k, &gat)?, &kat)?;
                            positive(&v, &kat)?;
                            if v.is_empty() {
                                return Err(format!("{kat} is empty"));
                            }
                            Ok(v)
                        };
                        Grid2::Tensor {
                            u: axis("u")?,
                            v: axis("v")?,
                        }
                    }
                    other => return Err(format!("{gat}: grid type {other:?} is unknown")),
                };
                let heights = match j.get("heights") {
                    None | Some(Json::Null) => None,
                    Some(h) => {
                        let hat = format!("{at}.heights");
                        let h = self.scalars(h, &hat)?;
                        let corners: u64 = grid.count().iter().map(|&n| u64::from(n) + 1).product();
                        if h.len() as u64 != corners {
                            return Err(format!(
                                "{hat}: {} heights for {corners} grid corners",
                                h.len()
                            ));
                        }
                        Some(h)
                    }
                };
                Geometry::GridSurface {
                    orient,
                    grid,
                    heights,
                }
            }
            "BlockModel" => Geometry::BlockModel(self.block_model(j, at)?),
            "Composite" => Geometry::Composite {
                elements: match j.get("elements") {
                    None => Vec::new(),
                    Some(e) => self.elements(e, &format!("{at}.elements"))?,
                },
            },
            other => return Err(format!("{at}: geometry type {other:?} is unknown")),
        })
    }

    fn element(&self, j: &Json, at: &str) -> Result<Element, String> {
        object(j, at)?;
        let name = required_text(j, "name", at)?;
        let at = format!("{at} \"{name}\"");
        let geometry = self.geometry(field(j, "geometry", &at)?, &format!("{at}.geometry"))?;
        let attributes = self.attributes(j, &at, &|loc| location_len(&geometry, loc))?;
        Ok(Element {
            name,
            description: text(j, "description", &at)?,
            color: match j.get("color") {
                None | Some(Json::Null) => None,
                Some(c) => Some(rgba(c, &format!("{at}.color"))?),
            },
            metadata: metadata(j, &at)?,
            attributes,
            geometry,
        })
    }

    fn elements(&self, j: &Json, at: &str) -> Result<Vec<Element>, String> {
        j.as_array()
            .ok_or_else(|| format!("{at} is not an array"))?
            .iter()
            .enumerate()
            .map(|(i, e)| self.element(e, &format!("{at}[{i}]")))
            .collect()
    }

    fn project(&self, j: &Json, format: String) -> Result<Project, String> {
        let at = "project";
        object(j, at)?;
        Ok(Project {
            format,
            name: text(j, "name", at)?,
            description: text(j, "description", at)?,
            coordinate_reference_system: text(j, "coordinate_reference_system", at)?,
            units: text(j, "units", at)?,
            origin: origin(j, at)?,
            author: text(j, "author", at)?,
            application: text(j, "application", at)?,
            date: required_text(j, "date", at)?,
            metadata: metadata(j, at)?,
            elements: match j.get("elements") {
                None => Vec::new(),
                Some(e) => self.elements(e, "elements")?,
            },
        })
    }
}

#[cfg(test)]
mod tests;
