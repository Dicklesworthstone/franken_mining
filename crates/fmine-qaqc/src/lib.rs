//! Corebox QA/QC crate (`fmine-qaqc`; plan §12.1–§12.4, `BD-167`, `BD-168`, `BD-169`, `BD-282`,
//! `BD-283`, `BD-284`).
//! Determinism: BitwisePortable (IEEE-754 arithmetic, `fmine_core::det` primitives, and ordered
//! `BTreeMap`/`BTreeSet` aggregations; zero external crates, zero I/O, zero ambient clocks).
//!
//! - [`error`]: typed `QaqcError` mapped to registered `FMINE-REF-*` refusal codes.
//! - [`crm`]: CRM evaluation with named SD basis (`σ_C`, `σ_L`, `S_W`), CANMET batch-mean test,
//!   mandatory Student-t bias confidence intervals, multi-basis comparison, and versioned site
//!   rule texts (Kibali, Loulo, NGM).
//! - [`insertion`]: QC insertion-rate accounting with explicit denominators and `PolicyDefault`
//!   vs `QpPolicy` targets, versioned blank contamination rules (`> k·DL`, Carlin `"and"`,
//!   Turquoise Ridge `"either/or"`), per-lab disaggregation, and failed-batch cascade (`PO-CORE-4`).
//! - [`monitors`]: sequential anytime-valid PIT betting e-process drift detection (`fmine-numerics`)
//!   alongside multi-rule pattern monitors and the dispersion-collapse tampering monitor
//!   (`AlarmScope::Integrity`, `GATE-CORE-02`).
//! - [`duplicates`]: canonical `CV_AVR%` precision with derived `HARD`/`AMPD`, Thompson–Howarth
//!   (`N >= 50` with optimistic caveat), `E374` mass matching, `FieldDuplicateAbsent`, Gy's FSE
//!   (general and calibrated with `K`/`α` provenance and `OutsideValidity`), and nugget split to
//!   Helm protocol-vs-infill information options.
//! - [`censored`]: Type-I multi-detection-limit left-censored lognormal MLE and robust ROS with
//!   guard `G-CENS` refusing DL/2 substitution.

pub mod censored;
pub mod crm;
pub mod duplicates;
pub mod error;
pub mod insertion;
pub mod monitors;
mod stats;
pub mod verification_index;

pub use error::QaqcError;
