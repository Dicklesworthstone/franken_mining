//! Geometry kernels for Strata (`WP-11`, plan §27): exact geometric predicates first (`BD-193`).
//! Determinism: BitwisePortable (straight-line IEEE `+ − ×` and `mul_add`; no transcendental).
//!
//! - `exact`: the adaptive-precision `orient2d`, `orient3d`, `incircle` and `insphere` predicates and their
//!   Simulation-of-Simplicity forms, lifted from `frankensim` fs-ivl and fs-math (registries/lifts.toml). The
//!   sign each returns is exact; a non-finite coordinate or an intermediate overflow is refused, never guessed.
//!
//! Nothing in the workspace calls this crate yet. VdbGrid (`BD-191`), meshes and block grids are later slices.

pub mod exact;
