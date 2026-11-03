//! An OMF wireframe against an OMF block model (bd-aqfl, plan §13.9): the proportion of each parent block inside a
//! closed surface, certified by `fmine_geom::volcert`.
//!
//! Both elements are taken in the project's frame without the project origin, which they share: a surface vertex
//! is its element origin plus the vertex, a block model starts at its frame origin. Leaving the project origin out
//! changes no proportion and saves one rounding on every coordinate.
//!
//! **Turned models** (bd-wn57). Proportions are computed in the block model's own frame, where its blocks are
//! axis-aligned from zero: a surface point `p` has grid coordinates `M⁻¹(p − origin)`, `M`'s columns being the
//! axes u, v and w and its inverse the adjugate over the determinant. For axes x, y and z that is one subtraction.
//! Otherwise each coordinate is rounded a few times more, and the certificate is for the surface so placed. The
//! reader holds the axes to unit length and right angles within 1e-6; using the inverse rather than the transpose
//! keeps the blocks exactly the parallelepipeds the file's axes span. A mirrored frame (negative determinant)
//! reverses each triangle, so an outward surface stays outward.

use fmine_evidence::{Refusal, RefusalCode};
use fmine_geom::volcert::{BlockGrid, ClosedMesh, Proportions, block_proportions};

use super::{BlockModel, Element, Geometry, Grid3, Orient3, Project};

pub(super) fn missing(detail: String) -> Refusal {
    Refusal::new(RefusalCode::InputMissingOrUnreadable506, detail)
}

/// The one element called `name`, top level or inside a composite.
pub(super) fn element<'a>(project: &'a Project, name: &str) -> Result<&'a Element, Refusal> {
    fn collect<'a>(elements: &'a [Element], name: &str, found: &mut Vec<&'a Element>) {
        for e in elements {
            if e.name == name {
                found.push(e);
            }
            if let Geometry::Composite { elements } = &e.geometry {
                collect(elements, name, found);
            }
        }
    }
    let mut found = Vec::new();
    collect(&project.elements, name, &mut found);
    match found.as_slice() {
        [one] => Ok(one),
        [] => Err(missing(format!("OMF: no element is called \"{name}\""))),
        many => Err(missing(format!(
            "OMF: {} elements are called \"{name}\"; rename them so one name picks one element",
            many.len()
        ))),
    }
}

pub(super) fn kind(geometry: &Geometry) -> &'static str {
    match geometry {
        Geometry::PointSet { .. } => "a PointSet",
        Geometry::LineSet { .. } => "a LineSet",
        Geometry::Surface { .. } => "a Surface",
        Geometry::GridSurface { .. } => "a GridSurface",
        Geometry::BlockModel(_) => "a BlockModel",
        Geometry::Composite { .. } => "a Composite",
    }
}

/// A surface's positions and triangles.
type Triangles<'a> = (Vec<[f64; 3]>, &'a [[u32; 3]]);

/// The Surface element `name`: its positions (element origin plus vertex) and triangles.
fn surface<'a>(project: &'a Project, name: &str) -> Result<Triangles<'a>, Refusal> {
    let e = element(project, name)?;
    let Geometry::Surface {
        origin,
        vertices,
        triangles,
    } = &e.geometry
    else {
        return Err(missing(format!(
            "OMF: element \"{name}\" is {}, not a Surface",
            kind(&e.geometry)
        )));
    };
    let positions = vertices
        .iter()
        .map(|v| std::array::from_fn(|a| origin[a] + v[a]))
        .collect();
    Ok((positions, triangles))
}

/// The Surface element `name` as a certified closed mesh, in the project's frame.
///
/// # Errors
/// `FMINE-REF-506` when no element, or more than one, is called `name`, or when it is not a Surface; then
/// `ClosedMesh::certify`'s refusals, for a surface that is not closed and outward-facing.
pub fn surface_mesh(project: &Project, name: &str) -> Result<ClosedMesh, Refusal> {
    let (positions, triangles) = surface(project, name)?;
    ClosedMesh::certify(positions, triangles.to_vec())
}

/// The BlockModel element `name`'s frame and regular grid.
fn block_model<'a>(
    project: &'a Project,
    name: &str,
) -> Result<(&'a Orient3, [f64; 3], [u32; 3]), Refusal> {
    let e = element(project, name)?;
    let Geometry::BlockModel(BlockModel { orient, grid, .. }) = &e.geometry else {
        return Err(missing(format!(
            "OMF: element \"{name}\" is {}, not a BlockModel",
            kind(&e.geometry)
        )));
    };
    let Grid3::Regular { size, count } = grid else {
        return Err(Refusal::new(
            RefusalCode::OutsidePhysicalDomain109,
            format!(
                "OMF: block model \"{name}\" has a tensor grid; block proportions cover regular grids"
            ),
        ));
    };
    Ok((orient, *size, *count))
}

/// The parent grid of the BlockModel element `name`, in the model's own frame: block `(i, j, k)` spans
/// `size·(i, j, k)` to `size·(i + 1, j + 1, k + 1)` from zero along u, v and w. Sub-blocks are not considered.
///
/// # Errors
/// `FMINE-REF-506` when no element, or more than one, is called `name`, or when it is not a BlockModel.
/// `FMINE-REF-109` for a tensor grid: proportions cover regular grids. Then `BlockGrid::new`'s refusals.
pub fn parent_grid(project: &Project, name: &str) -> Result<BlockGrid, Refusal> {
    let (_, size, count) = block_model(project, name)?;
    BlockGrid::new([0.0; 3], size, count)
}

/// The Surface element `surface` as a certified closed mesh in the frame of the BlockModel element `block_model`
/// (see the module notes), where [`parent_grid`] places its blocks.
///
/// # Errors
/// Those of [`surface_mesh`] and [`parent_grid`], and `FMINE-REF-109` for axes whose determinant is not a finite
/// nonzero number.
pub fn surface_mesh_in(
    project: &Project,
    surface_name: &str,
    block_model_name: &str,
) -> Result<ClosedMesh, Refusal> {
    let (positions, triangles) = surface(project, surface_name)?;
    let (orient, _, _) = block_model(project, block_model_name)?;
    let cross = |a: [f64; 3], b: [f64; 3]| -> [f64; 3] {
        [
            a[1] * b[2] - a[2] * b[1],
            a[2] * b[0] - a[0] * b[2],
            a[0] * b[1] - a[1] * b[0],
        ]
    };
    let dot = |a: [f64; 3], b: [f64; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
    let (u, v, w, o) = (orient.u, orient.v, orient.w, orient.origin);
    let adjugate = [cross(v, w), cross(w, u), cross(u, v)];
    let det = dot(u, adjugate[0]);
    if !det.is_finite() || det == 0.0 {
        return Err(Refusal::new(
            RefusalCode::OutsidePhysicalDomain109,
            format!(
                "OMF: block model \"{block_model_name}\" has axes u {u:?}, v {v:?}, w {w:?} whose determinant is \
                 {det}; they span no volume"
            ),
        ));
    }
    let rows = adjugate.map(|r| r.map(|x| x / det));
    let local = positions
        .iter()
        .map(|p| {
            let d: [f64; 3] = std::array::from_fn(|a| p[a] - o[a]);
            rows.map(|r| dot(r, d))
        })
        .collect();
    let triangles = if det < 0.0 {
        triangles.iter().map(|&[a, b, c]| [a, c, b]).collect()
    } else {
        triangles.to_vec()
    };
    ClosedMesh::certify(local, triangles)
}

/// The proportion of each parent block of the BlockModel `block_model` inside the Surface `surface`, as sure and
/// band sub-cell counts at `depth` (see `fmine_geom::volcert::block_proportions`), in the model's frame.
///
/// # Errors
/// Those of [`surface_mesh_in`], [`parent_grid`] and `block_proportions`.
pub fn proportions_in_surface(
    project: &Project,
    surface: &str,
    block_model: &str,
    depth: u8,
) -> Result<Proportions, Refusal> {
    let mesh = surface_mesh_in(project, surface, block_model)?;
    let grid = parent_grid(project, block_model)?;
    block_proportions(&mesh, &grid, depth)
}
