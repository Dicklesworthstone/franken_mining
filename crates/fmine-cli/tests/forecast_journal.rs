//! Real-binary acceptance for versioned forecast admission across process restarts.
//! Synthetic forecasts and the declared local clock bound are not external timestamp proof.

#![allow(clippy::expect_used, clippy::panic)]

// Keep Cargo's executable path and the original test names while the cross-crate matrix lives in T.
include!("../../xtask/testdata/forecast_journal_contract.rs");
