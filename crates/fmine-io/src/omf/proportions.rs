//! An OMF wireframe against an OMF block model (bd-aqfl, plan §13.9): the proportion of each parent block inside a
//! closed surface, certified by `fmine_geom::volcert`.
//!
//! Both elements are taken in the project's frame without the project origin, which they share: a surface vertex
//! is its element origin plus the vertex, a block model starts at its frame origin. Leaving the project origin out
//! changes no proportion and saves one rounding on every coordinate.

use fmine_evidence::{Refusal, RefusalCode};
use fmine_geom::volcert::{BlockGrid, ClosedMesh, Proportions, block_proportions};

use super::{BlockModel, Element, Geometry, Grid3, Project};

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

/// The Surface element `name` as a certified closed mesh.
///
/// # Errors
/// `FMINE-REF-506` when no element, or more than one, is called `name`, or when it is not a Surface; then
/// `ClosedMesh::certify`'s refusals, for a surface that is not closed and outward-facing.
pub fn surface_mesh(project: &Project, name: &str) -> Result<ClosedMesh, Refusal> {
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
    ClosedMesh::certify(positions, triangles.clone())
}

/// The parent grid of the BlockModel element `name`. Sub-blocks are not considered.
///
/// # Errors
/// `FMINE-REF-506` when no element, or more than one, is called `name`, or when it is not a BlockModel.
/// `FMINE-REF-109` for a frame whose axes are not x, y and z or a tensor grid: proportions cover regular,
/// axis-aligned grids only. Then `BlockGrid::new`'s refusals.
pub fn parent_grid(project: &Project, name: &str) -> Result<BlockGrid, Refusal> {
    let e = element(project, name)?;
    let Geometry::BlockModel(BlockModel { orient, grid, .. }) = &e.geometry else {
        return Err(missing(format!(
            "OMF: element \"{name}\" is {}, not a BlockModel",
            kind(&e.geometry)
        )));
    };
    if orient.u != [1.0, 0.0, 0.0] || orient.v != [0.0, 1.0, 0.0] || orient.w != [0.0, 0.0, 1.0] {
        return Err(Refusal::new(
            RefusalCode::OutsidePhysicalDomain109,
            format!(
                "OMF: block model \"{name}\" is rotated (u {:?}, v {:?}, w {:?}); block proportions cover grids \
                 whose axes are x, y and z",
                orient.u, orient.v, orient.w
            ),
        ));
    }
    let Grid3::Regular { size, count } = grid else {
        return Err(Refusal::new(
            RefusalCode::OutsidePhysicalDomain109,
            format!(
                "OMF: block model \"{name}\" has a tensor grid; block proportions cover regular grids"
            ),
        ));
    };
    BlockGrid::new(orient.origin, *size, *count)
}

/// The proportion of each parent block of the BlockModel `block_model` inside the Surface `surface`, as sure and
/// band sub-cell counts at `depth` (see `fmine_geom::volcert::block_proportions`).
///
/// # Errors
/// Those of [`surface_mesh`], [`parent_grid`] and `block_proportions`.
pub fn proportions_in_surface(
    project: &Project,
    surface: &str,
    block_model: &str,
    depth: u8,
) -> Result<Proportions, Refusal> {
    let mesh = surface_mesh(project, surface)?;
    let grid = parent_grid(project, block_model)?;
    block_proportions(&mesh, &grid, depth)
}
