//! Pushback optimisation kernels (`WP-19`, plan §15.4): pit limits first (`BD-250`).
//! Determinism: BitwisePortable (integer arithmetic only; no floating point).
//!
//! - `pitflow`: the ultimate pit as a maximum-weight closure, by Hochbaum's pseudoflow on an `i64` CSR precedence
//!   graph. Each pit comes with a pseudoflow certificate that `pitflow::verify` checks without the solver.
//! - `pitflow::shells`: nested pit shells across exact revenue factors, each one checked against the certified
//!   optimum at its factor (PO-PUSH-3, `bd-nzwd`).
//!
//! Nothing in the workspace calls this crate yet. Slope-template arcs, Bienstock–Zuckerberg, TopoSort and the
//! stochastic layer are later slices.

pub mod pitflow;
