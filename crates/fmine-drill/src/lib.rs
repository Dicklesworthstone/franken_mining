//! Corebox drillhole records on the authority ledger (`BD-163`, plan §12.1) and their desurvey (`BD-164`, §12.2).
//! Determinism: BitwisePortable (integer, byte and ordered-map operations; desurvey uses IEEE-754 arithmetic and
//! `fmine_core::det` trigonometry only).
//!
//! [`records`] defines the thirteen record types (collar, survey, interval, sample, dispatch, assay result, QC
//! insertion, lab certificate, sample loss, twin pair, twin acceptance, sample discard, batch flag), the checks
//! each must pass alone and their canonical bytes. [`store`] writes them to a ledger cell, where a correction
//! supersedes the version before it and every version stays readable, and runs the commit-time law that refuses a
//! record naming one that is not recorded. [`desurvey`] turns a collar and its survey stations into a
//! minimum-curvature trace, and [`survey_error`] propagates their stated accuracies under declared error terms to a
//! positional covariance at any depth. [`join`] finds the overlapping from/to spans of two interval sets, hole by
//! hole. [`composite`] cuts domain runs into length- or mass-weighted composites that conserve their inputs.
//! [`lab`] folds the cell's batches into `OB-LAB` obligations: every dispatched sample resolves to a received
//! result or a declared loss. [`twin`] measures a twin pair on the desurveyed traces, compares its matched grades,
//! guards twin-derived corrections (`G-TWIN`) and runs the RC smearing monitor. [`checks`] runs the campaign data
//! checks: recovery–grade bias, data-generation comparison, in/out survey runs, collar against DTM, `qaqc_regime`
//! per campaign, and the selection of `CommittedFlagged` results with their re-assay state.
//! [`qaqc`] evaluates fixed CRM batches on declared SD bases, CANMET and a conditional bias interval.

pub mod checks;
pub mod composite;
pub mod desurvey;
pub mod join;
pub mod lab;
pub mod qaqc;
pub mod records;
mod stats;
pub mod store;
pub mod survey_error;
pub mod twin;

#[cfg(test)]
#[path = "../tests/common/fixtures.rs"]
mod fixtures;
