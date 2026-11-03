//! Geometry kernels for Strata (`WP-11`, plan §27): exact geometric predicates (`BD-193`), the sparse voxel
//! tree (`BD-191`) and generalized winding numbers (`BD-192`).
//! Determinism: BitwisePortable (straight-line IEEE `+ − × ÷`, `sqrt` and `mul_add`; the one transcendental,
//! `atan2` in `winding`, is fmine-core's `det::atan2`).
//!
//! - `exact`: the adaptive-precision `orient2d`, `orient3d`, `incircle` and `insphere` predicates and their
//!   Simulation-of-Simplicity forms, lifted from `frankensim` fs-ivl and fs-math (registries/lifts.toml). The
//!   sign each returns is exact; a non-finite coordinate or an intermediate overflow is refused, never guessed.
//! - `vdb`: `VdbGrid`, the root map → 32³ internal node → 8³ leaf tree over `i32` voxel indices, lifted from
//!   `frankensim` fs-rep-sdf, with deterministic `iter_active` order (GATE-BM-01).
//! - `winding`: `winding_exact` and the dipole `WindingOctree` over a triangle `Soup`, lifted from `frankensim`
//!   fs-rep-mesh with `det::atan2` in place of std's. Inside/outside holds up on soups with holes, flips and
//!   duplicates; a differential corpus checks it against upstream's own output.
//!
//! Nothing in the workspace calls this crate yet. Meshes, block grids and columnar attribute chunks are later
//! slices.

pub mod exact;
pub mod vdb;
pub mod volcert;
pub mod winding;
