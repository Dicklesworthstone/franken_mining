#![cfg(test)]

//! BD-341: source-to-score CLI checks on invented workbooks and two synthetic reader records.
//! The PDF stubs are hash-bound bytes, never evidence that a real document or human reading was checked.

#![allow(clippy::expect_used, clippy::unwrap_used)]

// Keep Cargo's executable path and the original test names while the cross-crate matrix lives in T.
include!("../../xtask/testdata/track_record_contract.rs");
