//! Touchstone reconciliation engine (`fmine-recon`; plan §20, `BD-149`, `BD-150`, `BD-151`).
//! Determinism: DeterministicPerIsa (delegates linear, partial, and nonlinear LDLᵀ solves to
//! `fmine_numerics::reconciliation`; deterministic special functions via `fmine_core::det` and
//! canonical `BTreeMap`/`BTreeSet` variable ordering).
//!
//! - [`error`]: typed `ReconError` mapped to registered `FMINE-REF-*` refusal codes.
//! - [`linear`]: R1 linear WLS reconciliation over `fmine_flow::FlowGraph` with full covariance,
//!   observable unmeasured stream inference, guards `G-TOUCH-1`/`G-TOUCH-2`/`G-TOUCH-3`, and
//!   Bayesian bias identifiability analysis (`GB-24`, `GB-25`, `BD-149`).
//! - [`gross_error`]: global chi-square and per-measurement normalized adjustment tests with
//!   commit gating (`FMINE-REF-101`), exact analytical detection power curves, and single-period
//!   / quarterly Minimum Detectable Bias (`BD-150`).
//! - [`bilinear`]: bilinear tonnes × grade metallurgical reconciliation over strictly extensive
//!   `[dry_mass, elemental_metal]` state vectors with post-reconciliation derived grades and
//!   recoveries (`BD-151`).

pub mod bilinear;
pub mod error;
pub mod gross_error;
pub mod linear;
mod stats;

pub use error::ReconError;
