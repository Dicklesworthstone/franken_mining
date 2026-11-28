//! Touchstone reconciliation engine (`fmine-recon`, Layer `K2`; plan §20, `BD-149`–`BD-153`, `BD-156`).
//! Determinism: DeterministicPerIsa (reconciliation solves delegate to `fmine-numerics::reconciliation`;
//! canonical `BTreeMap` ordering, `PhiloxStream` sampling, and SHA-256 / BLAKE3 digests).
//!
//! Modules:
//! - [`linear`]: R1 linear WLS reconciliation over `fmine_flow::FlowGraph` with full covariance,
//!   observable unmeasured flow inference, and guards `G-TOUCH-1`/`G-TOUCH-2`/`G-TOUCH-3` (`BD-149`).
//! - [`gross_error`]: Global chi-square and per-measurement normalized-adjustment gross-error tests,
//!   commit gate (`INV-005` / `FMINE-REF-101`), exact theoretical detection power curves, and
//!   quarterly Minimum Detectable Bias publication (`BD-150`).
//! - [`bilinear`]: Bilinear tonnes × grade metallurgical reconciliation over extensive
//!   `[dry_mass, elemental_metal]` states (`G-TOUCH-1`) with derived grades and recoveries (`BD-151`).
//! - [`bias`]: Hierarchical per-node / per-source bias posteriors with partial pooling, exact
//!   multivariate Gaussian sampling for SBC, `REFCLASS-` linkage, and `G-TOUCH-5` safeguard (`BD-152`).
//! - [`identifiability`]: Observation-operator rank & nullspace decomposition, correlation /
//!   prior-dominance refusal (`FMINE-REF-104`), and Measurement VOI ranking (§20.6, `BD-153`).
//! - [`certificate`]: Canonical `ReconciliationCertificate` (`FMNE` + `RCRT` + `FMT-CERT-1` bridge,
//!   `G-TOUCH-4`, `INV-002`, `PO-TOUCH-6`; §20.9, `BD-156`).
//! - [`error`]: Typed refusal errors mapped to registered `FMINE-REF-*` codes.

pub mod bias;
pub mod bilinear;
pub mod certificate;
pub mod error;
pub mod gross_error;
pub mod identifiability;
pub mod linear;
mod stats;
pub mod stockpile;

pub use error::ReconError;
