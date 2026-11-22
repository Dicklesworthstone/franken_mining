//! Determinism: BitwisePortable
//! Deterministic glTF 2.0 JSON (embedded data URI) and aligned binary GLB 2.0 scene exporter
//! (`BD-256`, plan §28.3, `INV-002`, `G-CRS-1`).

use std::fmt::Write as _;

use fmine_core::Digest32;
use fmine_core::hash::sha256;
use fmine_crs::CrsId;

use crate::voxel_mesh::SceneMesh;
use crate::{VIEW_NO_CLAIM, ViewError};

/// GLB 2.0 binary magic (`"glTF"` little-endian).
pub const GLB_MAGIC: u32 = 0x4654_6C67;

/// GLB 2.0 version number.
pub const GLB_VERSION: u32 = 2;

/// GLB JSON chunk type (`"JSON"` little-endian).
pub const GLB_CHUNK_JSON: u32 = 0x4E4F_534A;

/// GLB binary buffer chunk type (`"BIN\0"` little-endian).
pub const GLB_CHUNK_BIN: u32 = 0x004E_4942;

/// Multi-layer 3-D scene export input.
#[derive(Debug, Clone, PartialEq)]
pub struct GltfSceneInput {
    pub scene_name: String,
    pub meshes: Vec<SceneMesh>,
}

/// Exported glTF 2.0 JSON + aligned binary GLB 2.0 artifact with canonical SHA-256 digest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GltfExport {
    pub scene_name: String,
    pub crs_id: CrsId,
    /// Standalone glTF 2.0 JSON with embedded `data:application/octet-stream;base64,...` buffer.
    pub gltf_json: String,
    /// Aligned binary GLB 2.0 container bytes.
    pub glb_bytes: Vec<u8>,
    /// Raw little-endian geometry buffer bytes.
    pub bin_bytes: Vec<u8>,
    pub vertex_count: usize,
    pub triangle_count: usize,
    /// Canonical SHA-256 digest over `glb_bytes` (bit-identical across runs).
    pub scene_sha256: Digest32,
}

/// Parsed GLB 2.0 container header summary for deterministic verification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GlbHeaderInfo {
    pub version: u32,
    pub total_length: u32,
    pub json_chunk_len: u32,
    pub bin_chunk_len: u32,
}

fn json_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn fmt_f32_json(v: f32) -> String {
    let normalized = if v == 0.0 { 0.0_f32 } else { v };
    format!("{normalized:.6}")
}

pub(crate) fn crs_label(crs: CrsId) -> String {
    match crs {
        CrsId::Epsg4326 => "EPSG:4326".to_string(),
        CrsId::Wgs84Utm { zone, north } => {
            let epsg = if north {
                32600 + u32::from(zone)
            } else {
                32700 + u32::from(zone)
            };
            format!("EPSG:{epsg}")
        }
        CrsId::MineGrid { grid_code } => format!("MINEGRID:{grid_code}"),
    }
}

fn encode_base64(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b0 = u32::from(chunk[0]);
        let b1 = if chunk.len() > 1 {
            u32::from(chunk[1])
        } else {
            0
        };
        let b2 = if chunk.len() > 2 {
            u32::from(chunk[2])
        } else {
            0
        };
        let triple = (b0 << 16) | (b1 << 8) | b2;
        out.push(TABLE[((triple >> 18) & 0x3F) as usize] as char);
        out.push(TABLE[((triple >> 12) & 0x3F) as usize] as char);
        if chunk.len() > 1 {
            out.push(TABLE[((triple >> 6) & 0x3F) as usize] as char);
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            out.push(TABLE[(triple & 0x3F) as usize] as char);
        } else {
            out.push('=');
        }
    }
    out
}

struct BufferSliceMeta {
    pos_view: usize,
    pos_offset: usize,
    pos_len: usize,
    norm_view: usize,
    norm_offset: usize,
    norm_len: usize,
    idx_view: usize,
    idx_offset: usize,
    idx_len: usize,
    pos_accessor: usize,
    norm_accessor: usize,
    idx_accessor: usize,
}

fn build_gltf_json(
    scene_name: &str,
    crs: CrsId,
    meshes: &[SceneMesh],
    slices: &[BufferSliceMeta],
    bin_len: usize,
    buffer_uri: Option<&str>,
) -> String {
    let mut j = String::new();
    let _ = write!(
        j,
        "{{\"asset\":{{\"version\":\"2.0\",\"generator\":\"fmine-view (read-only)\",\
         \"extras\":{{\"read_only\":true,\"execution_authorized\":false,\
         \"reserve_disclosure_claim\":false,\"geotechnical_signoff\":false,\
         \"regulatory_compliance_claim\":false,\"crs_id\":{},\"no_claim\":{}}}}},\
         \"scene\":0,\"scenes\":[{{\"name\":{},\"nodes\":[",
        json_escape(&crs_label(crs)),
        json_escape(VIEW_NO_CLAIM),
        json_escape(scene_name),
    );
    for i in 0..meshes.len() {
        if i > 0 {
            j.push(',');
        }
        let _ = write!(j, "{i}");
    }
    j.push_str("]}],\"nodes\":[");
    for (i, m) in meshes.iter().enumerate() {
        if i > 0 {
            j.push(',');
        }
        let _ = write!(j, "{{\"name\":{},\"mesh\":{i}}}", json_escape(&m.name));
    }
    j.push_str("],\"materials\":[");
    for (i, m) in meshes.iter().enumerate() {
        if i > 0 {
            j.push(',');
        }
        let _ = write!(
            j,
            "{{\"name\":{},\"pbrMetallicRoughness\":{{\"baseColorFactor\":[{},{},{},{}],\
             \"metallicFactor\":0.0,\"roughnessFactor\":0.85}}}}",
            json_escape(&format!("{}_mat", m.name)),
            fmt_f32_json(m.color_rgba[0]),
            fmt_f32_json(m.color_rgba[1]),
            fmt_f32_json(m.color_rgba[2]),
            fmt_f32_json(m.color_rgba[3]),
        );
    }
    j.push_str("],\"meshes\":[");
    for (i, (m, sl)) in meshes.iter().zip(slices.iter()).enumerate() {
        if i > 0 {
            j.push(',');
        }
        let _ = write!(
            j,
            "{{\"name\":{},\"primitives\":[{{\"attributes\":{{\"POSITION\":{},\"NORMAL\":{}}},\
             \"indices\":{},\"material\":{i},\"mode\":4}}]}}",
            json_escape(&m.name),
            sl.pos_accessor,
            sl.norm_accessor,
            sl.idx_accessor,
        );
    }
    j.push_str("],\"accessors\":[");
    for (i, (m, sl)) in meshes.iter().zip(slices.iter()).enumerate() {
        if i > 0 {
            j.push(',');
        }
        let _ = write!(
            j,
            "{{\"bufferView\":{},\"componentType\":5126,\"count\":{},\"type\":\"VEC3\",\
             \"min\":[{},{},{}],\"max\":[{},{},{}]}},\
             {{\"bufferView\":{},\"componentType\":5126,\"count\":{},\"type\":\"VEC3\"}},\
             {{\"bufferView\":{},\"componentType\":5125,\"count\":{},\"type\":\"SCALAR\"}}",
            sl.pos_view,
            m.positions.len(),
            fmt_f32_json(m.min_pos[0]),
            fmt_f32_json(m.min_pos[1]),
            fmt_f32_json(m.min_pos[2]),
            fmt_f32_json(m.max_pos[0]),
            fmt_f32_json(m.max_pos[1]),
            fmt_f32_json(m.max_pos[2]),
            sl.norm_view,
            m.normals.len(),
            sl.idx_view,
            m.indices.len(),
        );
    }
    j.push_str("],\"bufferViews\":[");
    for (i, sl) in slices.iter().enumerate() {
        if i > 0 {
            j.push(',');
        }
        let _ = write!(
            j,
            "{{\"buffer\":0,\"byteOffset\":{},\"byteLength\":{},\"target\":34962}},\
             {{\"buffer\":0,\"byteOffset\":{},\"byteLength\":{},\"target\":34962}},\
             {{\"buffer\":0,\"byteOffset\":{},\"byteLength\":{},\"target\":34963}}",
            sl.pos_offset, sl.pos_len, sl.norm_offset, sl.norm_len, sl.idx_offset, sl.idx_len,
        );
    }
    j.push_str("],\"buffers\":[{\"byteLength\":");
    let _ = write!(j, "{bin_len}");
    if let Some(uri) = buffer_uri {
        j.push_str(",\"uri\":");
        j.push_str(&json_escape(uri));
    }
    j.push_str("}]}");
    j
}

/// Export one or more same-CRS [`SceneMesh`] layers to deterministic glTF 2.0 JSON and binary GLB 2.0.
pub fn export_gltf_scene(input: &GltfSceneInput) -> Result<GltfExport, ViewError> {
    if input.scene_name.trim().is_empty() {
        return Err(ViewError::InvalidGeometry {
            detail: "scene_name must be non-empty".to_string(),
        });
    }
    let first = input.meshes.first().ok_or_else(|| ViewError::EmptyScene {
        detail: "scene must contain at least one mesh layer".to_string(),
    })?;
    let crs_id = first.crs_id;

    let mut bin_bytes: Vec<u8> = Vec::new();
    let mut slices = Vec::with_capacity(input.meshes.len());
    let mut vertex_count = 0_usize;
    let mut triangle_count = 0_usize;

    for (idx, mesh) in input.meshes.iter().enumerate() {
        if mesh.crs_id != crs_id {
            return Err(ViewError::CrsMismatch {
                expected: crs_id,
                got: mesh.crs_id,
            });
        }
        if mesh.name.trim().is_empty()
            || mesh.positions.is_empty()
            || mesh.positions.len() != mesh.normals.len()
            || mesh.indices.is_empty()
            || !mesh.indices.len().is_multiple_of(3)
        {
            return Err(ViewError::InvalidGeometry {
                detail: format!(
                    "mesh '{}' has empty or malformed vertex/index buffers",
                    mesh.name
                ),
            });
        }
        for (c_i, &c) in mesh.color_rgba.iter().enumerate() {
            if !c.is_finite() || !(0.0..=1.0).contains(&c) {
                return Err(ViewError::InvalidGeometry {
                    detail: format!("mesh '{}' color_rgba[{c_i}] outside [0, 1]: {c}", mesh.name),
                });
            }
        }
        for axis in 0..3 {
            if !mesh.min_pos[axis].is_finite()
                || !mesh.max_pos[axis].is_finite()
                || mesh.min_pos[axis] > mesh.max_pos[axis]
            {
                return Err(ViewError::InvalidGeometry {
                    detail: format!("mesh '{}' invalid bounding box on axis {axis}", mesh.name),
                });
            }
        }
        let max_v = u32::try_from(mesh.positions.len()).map_err(|_| ViewError::Overflow)?;
        for &v_idx in &mesh.indices {
            if v_idx >= max_v {
                return Err(ViewError::InvalidGeometry {
                    detail: format!(
                        "mesh '{}' index {v_idx} out of bounds for {} vertices",
                        mesh.name,
                        mesh.positions.len()
                    ),
                });
            }
        }

        let pos_offset = bin_bytes.len();
        let mut actual_min = [f32::INFINITY; 3];
        let mut actual_max = [f32::NEG_INFINITY; 3];
        for p in &mesh.positions {
            for (axis, &coord) in p.iter().enumerate() {
                if !coord.is_finite() {
                    return Err(ViewError::InvalidGeometry {
                        detail: format!("mesh '{}' contains non-finite position", mesh.name),
                    });
                }
                actual_min[axis] = actual_min[axis].min(coord);
                actual_max[axis] = actual_max[axis].max(coord);
                bin_bytes.extend_from_slice(&coord.to_le_bytes());
            }
        }
        // glTF 2.0 §3.7.2.2 requires POSITION accessor min/max to equal the exact vertex extrema.
        if actual_min != mesh.min_pos || actual_max != mesh.max_pos {
            return Err(ViewError::InvalidGeometry {
                detail: format!(
                    "mesh '{}' declared min_pos/max_pos ({:?}/{:?}) != actual vertex extrema ({:?}/{:?})",
                    mesh.name, mesh.min_pos, mesh.max_pos, actual_min, actual_max
                ),
            });
        }
        let pos_len = bin_bytes.len() - pos_offset;

        let norm_offset = bin_bytes.len();
        for n in &mesh.normals {
            for &coord in n {
                if !coord.is_finite() {
                    return Err(ViewError::InvalidGeometry {
                        detail: format!("mesh '{}' contains non-finite normal", mesh.name),
                    });
                }
                bin_bytes.extend_from_slice(&coord.to_le_bytes());
            }
            if n[0] == 0.0 && n[1] == 0.0 && n[2] == 0.0 {
                return Err(ViewError::InvalidGeometry {
                    detail: format!("mesh '{}' contains zero-length normal", mesh.name),
                });
            }
        }
        let norm_len = bin_bytes.len() - norm_offset;

        let idx_offset = bin_bytes.len();
        for &i in &mesh.indices {
            bin_bytes.extend_from_slice(&i.to_le_bytes());
        }
        let idx_len = bin_bytes.len() - idx_offset;

        vertex_count = vertex_count
            .checked_add(mesh.positions.len())
            .ok_or(ViewError::Overflow)?;
        triangle_count = triangle_count
            .checked_add(mesh.indices.len() / 3)
            .ok_or(ViewError::Overflow)?;

        slices.push(BufferSliceMeta {
            pos_view: idx * 3,
            pos_offset,
            pos_len,
            norm_view: idx * 3 + 1,
            norm_offset,
            norm_len,
            idx_view: idx * 3 + 2,
            idx_offset,
            idx_len,
            pos_accessor: idx * 3,
            norm_accessor: idx * 3 + 1,
            idx_accessor: idx * 3 + 2,
        });
    }

    // Pad BIN payload to 4-byte alignment with 0x00 per glTF 2.0 §4.4.3.3.
    let raw_bin_len = bin_bytes.len();
    let mut padded_bin = bin_bytes.clone();
    while !padded_bin.len().is_multiple_of(4) {
        padded_bin.push(0x00);
    }

    let uri = format!(
        "data:application/octet-stream;base64,{}",
        encode_base64(&bin_bytes)
    );
    let gltf_json = build_gltf_json(
        &input.scene_name,
        crs_id,
        &input.meshes,
        &slices,
        raw_bin_len,
        Some(&uri),
    );

    // GLB embedded JSON chunk omits URI (buffer 0 is the following BIN chunk).
    let mut glb_json_bytes = build_gltf_json(
        &input.scene_name,
        crs_id,
        &input.meshes,
        &slices,
        raw_bin_len,
        None,
    )
    .into_bytes();
    while !glb_json_bytes.len().is_multiple_of(4) {
        glb_json_bytes.push(b' ');
    }

    let json_chunk_len = u32::try_from(glb_json_bytes.len()).map_err(|_| ViewError::Overflow)?;
    let bin_chunk_len = u32::try_from(padded_bin.len()).map_err(|_| ViewError::Overflow)?;
    let total_length = 12_u32
        .checked_add(8)
        .and_then(|v| v.checked_add(json_chunk_len))
        .and_then(|v| v.checked_add(8))
        .and_then(|v| v.checked_add(bin_chunk_len))
        .ok_or(ViewError::Overflow)?;

    let mut glb_bytes = Vec::with_capacity(total_length as usize);
    glb_bytes.extend_from_slice(&GLB_MAGIC.to_le_bytes());
    glb_bytes.extend_from_slice(&GLB_VERSION.to_le_bytes());
    glb_bytes.extend_from_slice(&total_length.to_le_bytes());
    glb_bytes.extend_from_slice(&json_chunk_len.to_le_bytes());
    glb_bytes.extend_from_slice(&GLB_CHUNK_JSON.to_le_bytes());
    glb_bytes.extend_from_slice(&glb_json_bytes);
    glb_bytes.extend_from_slice(&bin_chunk_len.to_le_bytes());
    glb_bytes.extend_from_slice(&GLB_CHUNK_BIN.to_le_bytes());
    glb_bytes.extend_from_slice(&padded_bin);

    let scene_sha256 = sha256(&glb_bytes);

    Ok(GltfExport {
        scene_name: input.scene_name.clone(),
        crs_id,
        gltf_json,
        glb_bytes,
        bin_bytes,
        vertex_count,
        triangle_count,
        scene_sha256,
    })
}

/// Inspect and validate the header and chunk structure of a binary GLB 2.0 byte buffer.
pub fn inspect_glb_header(bytes: &[u8]) -> Result<GlbHeaderInfo, ViewError> {
    if bytes.len() < 28 {
        return Err(ViewError::InvalidGeometry {
            detail: "GLB buffer shorter than 28-byte minimum header + chunk headers".to_string(),
        });
    }
    let magic = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
    let version = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
    let total_length = u32::from_le_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]);
    if magic != GLB_MAGIC || version != GLB_VERSION {
        return Err(ViewError::InvalidGeometry {
            detail: format!("invalid GLB magic/version: 0x{magic:08x} v{version}"),
        });
    }
    if total_length as usize != bytes.len() {
        return Err(ViewError::InvalidGeometry {
            detail: format!(
                "GLB total_length {total_length} != actual byte length {}",
                bytes.len()
            ),
        });
    }
    let json_chunk_len = u32::from_le_bytes([bytes[12], bytes[13], bytes[14], bytes[15]]);
    let json_chunk_type = u32::from_le_bytes([bytes[16], bytes[17], bytes[18], bytes[19]]);
    if json_chunk_type != GLB_CHUNK_JSON || !json_chunk_len.is_multiple_of(4) {
        return Err(ViewError::InvalidGeometry {
            detail: "invalid or unaligned GLB JSON chunk header".to_string(),
        });
    }
    let bin_hdr_start = 20_usize
        .checked_add(json_chunk_len as usize)
        .ok_or(ViewError::Overflow)?;
    if bin_hdr_start + 8 > bytes.len() {
        return Err(ViewError::InvalidGeometry {
            detail: "GLB truncated before BIN chunk header".to_string(),
        });
    }
    let bin_chunk_len = u32::from_le_bytes([
        bytes[bin_hdr_start],
        bytes[bin_hdr_start + 1],
        bytes[bin_hdr_start + 2],
        bytes[bin_hdr_start + 3],
    ]);
    let bin_chunk_type = u32::from_le_bytes([
        bytes[bin_hdr_start + 4],
        bytes[bin_hdr_start + 5],
        bytes[bin_hdr_start + 6],
        bytes[bin_hdr_start + 7],
    ]);
    if bin_chunk_type != GLB_CHUNK_BIN
        || !bin_chunk_len.is_multiple_of(4)
        || bin_hdr_start + 8 + (bin_chunk_len as usize) != bytes.len()
    {
        return Err(ViewError::InvalidGeometry {
            detail: "invalid or truncated GLB BIN chunk".to_string(),
        });
    }
    Ok(GlbHeaderInfo {
        version,
        total_length,
        json_chunk_len,
        bin_chunk_len,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::voxel_mesh::{VoxelGridFrame, extract_voxel_surface_mesh};
    use fmine_evidence::RefusalCode;
    use fmine_geom::vdb::VdbGrid;

    #[test]
    fn test_gltf_and_glb_golden_scene_determinism_and_structure() {
        // Golden synthetic open-pit bench + high-grade ore pod scene (`BD-256`).
        let mut grid = VdbGrid::new(0u8);
        for x in 0..3 {
            for y in 0..3 {
                grid.set([x, y, 0], 1); // bench waste floor
            }
        }
        grid.set([1, 1, 1], 2); // ore pod block on bench 1

        let frame = VoxelGridFrame {
            origin_m: [1000.0, 2000.0, 500.0],
            voxel_size_m: [10.0, 10.0, 10.0],
            crs_id: CrsId::REKO_DIQ_UTM41N,
        };
        let bench = extract_voxel_surface_mesh(
            "bench_0_waste",
            &grid,
            &frame,
            [0.55, 0.55, 0.58, 1.0],
            Some(1),
        )
        .unwrap();
        let ore = extract_voxel_surface_mesh(
            "bench_1_ore",
            &grid,
            &frame,
            [0.90, 0.72, 0.10, 1.0],
            Some(2),
        )
        .unwrap();

        let input = GltfSceneInput {
            scene_name: "golden_pit_bench_v1".to_string(),
            meshes: vec![bench.clone(), ore.clone()],
        };
        let export_a = export_gltf_scene(&input).unwrap();
        let export_b = export_gltf_scene(&input).unwrap();

        // Bit-identical determinism across repeated exports.
        assert_eq!(export_a.glb_bytes, export_b.glb_bytes);
        assert_eq!(export_a.gltf_json, export_b.gltf_json);
        assert_eq!(export_a.scene_sha256, export_b.scene_sha256);

        // Validate GLB 2.0 header and 4-byte aligned chunk framing.
        let hdr = inspect_glb_header(&export_a.glb_bytes).unwrap();
        assert_eq!(hdr.version, 2);
        assert_eq!(hdr.total_length as usize, export_a.glb_bytes.len());
        assert!(hdr.json_chunk_len.is_multiple_of(4));
        assert!(hdr.bin_chunk_len.is_multiple_of(4));

        // Verify INV-002 read-only flags, CRS label, and embedded base64 URI in glTF JSON.
        assert!(export_a.gltf_json.contains("\"version\":\"2.0\""));
        assert!(export_a.gltf_json.contains("\"read_only\":true"));
        assert!(
            export_a
                .gltf_json
                .contains("\"execution_authorized\":false")
        );
        assert!(
            export_a
                .gltf_json
                .contains("\"reserve_disclosure_claim\":false")
        );
        assert!(export_a.gltf_json.contains("\"crs_id\":\"EPSG:32641\""));
        assert!(
            export_a
                .gltf_json
                .contains("data:application/octet-stream;base64,")
        );

        // CRS mismatch across layers refuses with FMINE-REF-106.
        let mut wrong_crs_ore = ore.clone();
        wrong_crs_ore.crs_id = CrsId::Epsg4326;
        assert_eq!(
            export_gltf_scene(&GltfSceneInput {
                scene_name: "mixed_crs".to_string(),
                meshes: vec![bench, wrong_crs_ore],
            })
            .unwrap_err()
            .to_refusal()
            .code,
            RefusalCode::CrsUnregisteredOrIncompatible106.spec().code
        );

        // Fresh-eyes regressions: stale min_pos/max_pos or zero-length normal refuses with FMINE-REF-109.
        let mut stale_bbox = ore.clone();
        stale_bbox.max_pos[0] += 100.0;
        assert_eq!(
            export_gltf_scene(&GltfSceneInput {
                scene_name: "stale_bbox".to_string(),
                meshes: vec![stale_bbox],
            })
            .unwrap_err()
            .to_refusal()
            .code,
            RefusalCode::OutsidePhysicalDomain109.spec().code
        );
        let mut zero_norm = ore;
        zero_norm.normals[0] = [0.0, 0.0, 0.0];
        assert_eq!(
            export_gltf_scene(&GltfSceneInput {
                scene_name: "zero_norm".to_string(),
                meshes: vec![zero_norm],
            })
            .unwrap_err()
            .to_refusal()
            .code,
            RefusalCode::OutsidePhysicalDomain109.spec().code
        );
    }
}
