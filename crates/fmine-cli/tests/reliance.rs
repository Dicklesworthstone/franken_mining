#![cfg(test)]
#![cfg(unix)]

//! BD-347: real-binary lifecycle and filesystem regressions on explicitly synthetic reports and keys.

// Keep Cargo's executable path and the original test names while the cross-crate matrix lives in T.
include!("../../xtask/testdata/reliance_contract.rs");
