//! Pushback optimisation kernels (`WP-19`, plan §15.4): pit limits first (`BD-250`).
//! Determinism: BitwisePortable (integer arithmetic only; no floating point).
//!
//! - `pitflow`: the ultimate pit as a maximum-weight closure, by Hochbaum's pseudoflow on an `i64` CSR precedence
//!   graph. Each pit comes with a pseudoflow certificate that `pitflow::verify` checks without the solver.
//! - `pitflow::shells`: nested pit shells across exact revenue factors, each one checked against the certified
//!   optimum at its factor (PO-PUSH-3, `bd-nzwd`).
//! - `pitflow::slope`: precedence arcs from exact slope tangents per azimuth sector and geotechnical domain, with
//!   implied arcs dropped and the closure kept (`bd-t75z`).
//! - `pitflow::minelib` and `pitflow::candidate`: MineLib UPIT pairs and external candidate pits, read for
//!   `fmine plan pit` (`bd-ip99`, `bd-nq06`).
//! - `toposort`: multi-period schedules. A feasibility checker (PO-PUSH-5), TopoSort's greedy from a ranking of the
//!   blocks, and MineLib's CPIT reader (`bd-kcpj`).
//!
//! `fmine plan pit` calls `pitflow`; nothing calls `toposort` outside its tests yet. Bienstock–Zuckerberg, local
//! search and the stochastic layer are later slices.

pub mod pitflow;
pub mod toposort;
