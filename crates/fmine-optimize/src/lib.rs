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
//! - `cutoff`: exact Lane limiting grades and a declared feed-grade screen (`BD-252`, plan §15.6).

pub mod cutoff;
pub mod pitflow;
pub mod plan_cert;
pub mod toposort;
