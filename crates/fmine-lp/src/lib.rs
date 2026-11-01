//! Pushback linear programming (`WP-19`, plan §15.4).
//! Determinism: DeterministicPerIsa (re-exports only; see `pdlp`).
//!
//! - `pdlp` (`BD-251`): restarted primal–dual hybrid gradient for large sparse linear programs, with a dual bound
//!   certified by dual repair and outward rounding (PO-PUSH-4).
//!
//! Nothing in the workspace calls this crate yet. MILP, QP, MPS I/O and decomposition are later slices.

pub mod pdlp;
