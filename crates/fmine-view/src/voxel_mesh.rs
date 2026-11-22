//! Determinism: BitwisePortable
//! Exposed boundary-face mesh extraction from sparse `fmine_geom::vdb::VdbGrid<u8>` volumes
//! (`BD-256`, `G-CRS-1`).

use fmine_crs::CrsId;
use fmine_geom::vdb::VdbGrid;

use crate::ViewError;

/// Local/world grid frame mapping integer `VdbGrid` indices `[i, j, k]` to metres in `crs_id`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VoxelGridFrame {
    pub origin_m: [f64; 3],
    pub voxel_size_m: [f64; 3],
    pub crs_id: CrsId,
}

impl VoxelGridFrame {
    /// Validate finite origin and strictly positive finite voxel sizes.
    pub fn validate(&self) -> Result<(), ViewError> {
        for (axis, &v) in self.origin_m.iter().enumerate() {
            if !v.is_finite() {
                return Err(ViewError::InvalidGeometry {
                    detail: format!("origin_m[{axis}] is non-finite"),
                });
            }
        }
        for (axis, &v) in self.voxel_size_m.iter().enumerate() {
            if !v.is_finite() || v <= 0.0 {
                return Err(ViewError::InvalidGeometry {
                    detail: format!("voxel_size_m[{axis}] must be finite and > 0, got {v}"),
                });
            }
        }
        Ok(())
    }
}

/// Deterministic indexed triangle mesh for one scene layer (outward right-handed CCW winding).
#[derive(Debug, Clone, PartialEq)]
pub struct SceneMesh {
    pub name: String,
    pub crs_id: CrsId,
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub indices: Vec<u32>,
    pub color_rgba: [f32; 4],
    pub min_pos: [f32; 3],
    pub max_pos: [f32; 3],
}

/// Face definition for one exposed boundary of a unit voxel.
struct VoxelFace {
    dir: [i32; 3],
    normal: [f32; 3],
    corners: [[f64; 3]; 4],
}

const VOXEL_FACES: [VoxelFace; 6] = [
    // -X face
    VoxelFace {
        dir: [-1, 0, 0],
        normal: [-1.0, 0.0, 0.0],
        corners: [
            [0.0, 0.0, 0.0],
            [0.0, 0.0, 1.0],
            [0.0, 1.0, 1.0],
            [0.0, 1.0, 0.0],
        ],
    },
    // +X face
    VoxelFace {
        dir: [1, 0, 0],
        normal: [1.0, 0.0, 0.0],
        corners: [
            [1.0, 0.0, 0.0],
            [1.0, 1.0, 0.0],
            [1.0, 1.0, 1.0],
            [1.0, 0.0, 1.0],
        ],
    },
    // -Y face
    VoxelFace {
        dir: [0, -1, 0],
        normal: [0.0, -1.0, 0.0],
        corners: [
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [1.0, 0.0, 1.0],
            [0.0, 0.0, 1.0],
        ],
    },
    // +Y face
    VoxelFace {
        dir: [0, 1, 0],
        normal: [0.0, 1.0, 0.0],
        corners: [
            [0.0, 1.0, 0.0],
            [0.0, 1.0, 1.0],
            [1.0, 1.0, 1.0],
            [1.0, 1.0, 0.0],
        ],
    },
    // -Z face
    VoxelFace {
        dir: [0, 0, -1],
        normal: [0.0, 0.0, -1.0],
        corners: [
            [0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [1.0, 1.0, 0.0],
            [1.0, 0.0, 0.0],
        ],
    },
    // +Z face
    VoxelFace {
        dir: [0, 0, 1],
        normal: [0.0, 0.0, 1.0],
        corners: [
            [0.0, 0.0, 1.0],
            [1.0, 0.0, 1.0],
            [1.0, 1.0, 1.0],
            [0.0, 1.0, 1.0],
        ],
    },
];

fn validate_rgba(rgba: [f32; 4]) -> Result<(), ViewError> {
    for (i, &c) in rgba.iter().enumerate() {
        if !c.is_finite() || !(0.0..=1.0).contains(&c) {
            return Err(ViewError::InvalidGeometry {
                detail: format!("color_rgba[{i}] must be in [0, 1], got {c}"),
            });
        }
    }
    Ok(())
}

/// Extract exposed boundary faces of active voxels in `grid` (matching `filter_value`, or all
/// active voxels if `filter_value == None`) in canonical `VdbGrid::iter_active()` order.
pub fn extract_voxel_surface_mesh(
    name: &str,
    grid: &VdbGrid<u8>,
    frame: &VoxelGridFrame,
    color_rgba: [f32; 4],
    filter_value: Option<u8>,
) -> Result<SceneMesh, ViewError> {
    if name.trim().is_empty() {
        return Err(ViewError::InvalidGeometry {
            detail: "scene layer name must be non-empty".to_string(),
        });
    }
    frame.validate()?;
    validate_rgba(color_rgba)?;

    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut normals: Vec<[f32; 3]> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();
    let mut min_pos = [f32::INFINITY; 3];
    let mut max_pos = [f32::NEG_INFINITY; 3];

    let matches_voxel = |coord: [i32; 3]| -> bool {
        if !grid.is_active(coord) {
            return false;
        }
        match filter_value {
            Some(target) => grid.get(coord) == target,
            None => true,
        }
    };

    for (coord, val) in grid.iter_active() {
        if let Some(target) = filter_value
            && val != target
        {
            continue;
        }
        for face in &VOXEL_FACES {
            let dir = face.dir;
            let normal = face.normal;
            let corners = face.corners;
            let nb = [
                coord[0].checked_add(dir[0]),
                coord[1].checked_add(dir[1]),
                coord[2].checked_add(dir[2]),
            ];
            let neighbor_occupied = match (nb[0], nb[1], nb[2]) {
                (Some(nx), Some(ny), Some(nz)) => matches_voxel([nx, ny, nz]),
                _ => false,
            };
            if neighbor_occupied {
                continue;
            }

            let base_idx = u32::try_from(positions.len()).map_err(|_| ViewError::Overflow)?;
            for corner in corners {
                let x = (frame.origin_m[0]
                    + (f64::from(coord[0]) + corner[0]) * frame.voxel_size_m[0])
                    as f32;
                let y = (frame.origin_m[1]
                    + (f64::from(coord[1]) + corner[1]) * frame.voxel_size_m[1])
                    as f32;
                let z = (frame.origin_m[2]
                    + (f64::from(coord[2]) + corner[2]) * frame.voxel_size_m[2])
                    as f32;
                if !x.is_finite() || !y.is_finite() || !z.is_finite() {
                    return Err(ViewError::InvalidGeometry {
                        detail: "voxel vertex coordinate overflowed finite f32".to_string(),
                    });
                }
                let p = [x, y, z];
                for axis in 0..3 {
                    if p[axis] < min_pos[axis] {
                        min_pos[axis] = p[axis];
                    }
                    if p[axis] > max_pos[axis] {
                        max_pos[axis] = p[axis];
                    }
                }
                positions.push(p);
                normals.push(normal);
            }
            let i1 = base_idx.checked_add(1).ok_or(ViewError::Overflow)?;
            let i2 = base_idx.checked_add(2).ok_or(ViewError::Overflow)?;
            let i3 = base_idx.checked_add(3).ok_or(ViewError::Overflow)?;
            indices.extend_from_slice(&[base_idx, i1, i2, base_idx, i2, i3]);
        }
    }

    if indices.is_empty() {
        return Err(ViewError::EmptyScene {
            detail: format!("layer '{name}' has zero active matching voxels"),
        });
    }

    Ok(SceneMesh {
        name: name.to_string(),
        crs_id: frame.crs_id,
        positions,
        normals,
        indices,
        color_rgba,
        min_pos,
        max_pos,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use fmine_evidence::RefusalCode;

    #[test]
    fn test_vdb_voxel_surface_extraction_culls_internal_faces_and_enforces_crs() {
        let mut grid = VdbGrid::new(0u8);
        grid.set([0, 0, 0], 1);
        grid.set([1, 0, 0], 1); // Adjacent along +X: 2 shared internal faces culled!
        let frame = VoxelGridFrame {
            origin_m: [0.0, 0.0, 0.0],
            voxel_size_m: [10.0, 10.0, 5.0],
            crs_id: CrsId::REKO_DIQ_UTM41N,
        };

        let mesh = extract_voxel_surface_mesh(
            "ore_shell",
            &grid,
            &frame,
            [0.85, 0.65, 0.13, 1.0],
            Some(1),
        )
        .unwrap();
        // 2 cubes = 12 faces - 2 shared internal faces = 10 exposed faces -> 40 verts, 20 triangles (60 indices).
        assert_eq!(mesh.positions.len(), 40);
        assert_eq!(mesh.normals.len(), 40);
        assert_eq!(mesh.indices.len(), 60);
        assert_eq!(mesh.min_pos, [0.0, 0.0, 0.0]);
        assert_eq!(mesh.max_pos, [20.0, 10.0, 5.0]);

        // Empty filter refuses with FMINE-REF-201.
        assert_eq!(
            extract_voxel_surface_mesh("empty", &grid, &frame, [1.0, 0.0, 0.0, 1.0], Some(9))
                .unwrap_err()
                .to_refusal()
                .code,
            RefusalCode::NoClaimInDomain201.spec().code
        );

        // Non-positive voxel size refuses with FMINE-REF-109.
        let mut bad_frame = frame;
        bad_frame.voxel_size_m[2] = 0.0;
        assert_eq!(
            extract_voxel_surface_mesh("bad", &grid, &bad_frame, [1.0, 0.0, 0.0, 1.0], None)
                .unwrap_err()
                .to_refusal()
                .code,
            RefusalCode::OutsidePhysicalDomain109.spec().code
        );
    }
}
