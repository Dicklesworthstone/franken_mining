//! `PO-ADIT-4` (`BD-112`): every invocation exits with a code from the plan §30.1 dictionary (0, 2–9), and 1 is
//! never emitted, so a crash can never be mistaken for a typed outcome.

// Keep the binary-owning package's executable path; cross-crate scenarios live in T.
include!("../../xtask/testdata/exit_codes_contract.rs");
