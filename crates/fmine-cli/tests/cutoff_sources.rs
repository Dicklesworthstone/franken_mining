#![cfg(test)]

//! BD-274: actual CLI contracts on invented technical-report bytes and synthetic reader records.
//! These fixtures test binding and arithmetic; they are not readings of the seven real corpus cut-offs.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

// Preserve Cargo's executable path while the cross-crate scenarios live in T.
include!("../../xtask/testdata/cutoff_sources_contract.rs");
