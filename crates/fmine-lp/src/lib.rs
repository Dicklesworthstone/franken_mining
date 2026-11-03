//! Pushback linear programming (`WP-19`, plan §15.4).
//! Determinism: DeterministicPerIsa (re-exports only; see `pdlp`).
//!
//! - `pdlp` (`BD-251`): restarted primal–dual hybrid gradient for large sparse linear programs, with a dual bound
//!   certified by dual repair and outward rounding (PO-PUSH-4).
//! - `mps` (bd-j2ay): free-format MPS read into, and written from, `pdlp::LpProblem`.
//!
//! `fmine-io`'s OMF schedules (bd-djb7) call `pdlp`. MILP, QP and decomposition are later slices.

pub mod mps;
pub mod pdlp;
