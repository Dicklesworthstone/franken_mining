//! Corebox drillhole records on the authority ledger (`BD-163`, plan §12.1).
//! Determinism: BitwisePortable (integer, byte and ordered-map operations only; no floating-point arithmetic).
//!
//! [`records`] defines the eight record types (collar, survey, interval, sample, dispatch, assay result, QC
//! insertion, lab certificate), the checks each must pass alone and their canonical bytes. [`store`] writes them
//! to a ledger cell, where a correction supersedes the version before it and every version stays readable, and
//! runs the commit-time law that refuses a record naming one that is not recorded.

pub mod records;
pub mod store;

#[cfg(test)]
#[path = "../tests/common/fixtures.rs"]
mod fixtures;
