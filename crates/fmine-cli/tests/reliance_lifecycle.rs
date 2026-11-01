//! Cargo supplies the real `fmine` binary path to this CLI test target.
//! The unchanged cross-crate lifecycle matrix lives in T and executes once through this harness.

#[cfg(target_os = "linux")]
#[path = "../../xtask/testdata/reliance_lifecycle.rs"]
mod lifecycle;
