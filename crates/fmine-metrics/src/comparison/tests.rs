//! Exact synthetic peer-comparison controls, also exercised by the small integration target.
//! Determinism: Exact
#![allow(clippy::expect_used, clippy::unwrap_used)]

use super::*;
use fmine_registry::sources::DocumentRow;
use fmine_registry::transcriptions::{Accepted, Locator, parse_token};

include!("../../../xtask/testdata/metrics_comparison_controls.rs");
