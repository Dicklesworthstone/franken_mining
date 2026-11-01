//! Exact geometric predicates (`BD-193`, plan §27 row "fs-ivl predicates/expansions, fs-math/eft.rs →
//! fmine-geom::exact").
//! Determinism: BitwisePortable (re-exports only).
//!
//! [`predicates`] is the public surface. The error-free transformations and the expansion arithmetic it stands on
//! stay crate-private: their bare-`f64` operands are arithmetic, not quantities, and no other crate needs them yet.

pub(crate) mod eft;
pub(crate) mod expansion;
pub mod predicates;

pub use predicates::{
    Sign, Stage, incircle, incircle_with_stage, insphere, insphere_with_stage, orient2d,
    orient2d_sos, orient2d_with_stage, orient3d, orient3d_sos, orient3d_with_stage,
};
