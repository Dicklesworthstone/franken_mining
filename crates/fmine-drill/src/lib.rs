//! Corebox drillhole records on the authority ledger (`BD-163`, plan §12.1) and their desurvey (`BD-164`, §12.2).
//! Determinism: BitwisePortable (integer, byte and ordered-map operations; desurvey uses IEEE-754 arithmetic and
//! `fmine_core::det` trigonometry only).
//!
//! [`records`] defines the eight record types (collar, survey, interval, sample, dispatch, assay result, QC
//! insertion, lab certificate), the checks each must pass alone and their canonical bytes. [`store`] writes them
//! to a ledger cell, where a correction supersedes the version before it and every version stays readable, and
//! runs the commit-time law that refuses a record naming one that is not recorded. [`desurvey`] turns a collar and
//! its survey stations into a minimum-curvature trace. [`join`] finds the overlapping from/to spans of two interval
//! sets, hole by hole. [`composite`] cuts domain runs into length- or mass-weighted composites that conserve their
//! inputs. [`lab`] folds the cell's batches into `OB-LAB` obligations: every dispatched sample resolves to a
//! received result or a declared loss.

pub mod composite;
pub mod desurvey;
pub mod join;
pub mod lab;
pub mod records;
pub mod store;

#[cfg(test)]
#[path = "../tests/common/fixtures.rs"]
mod fixtures;
