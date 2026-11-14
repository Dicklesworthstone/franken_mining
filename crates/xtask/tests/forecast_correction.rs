//! DESIGN: run the identical correction known answers against the production library, without
//! generating code for every unrelated calibration unit test. No candidate oracle is generated.

#![cfg(test)]
#![allow(clippy::expect_used, clippy::panic)]

mod ledger {
    pub use fmine_calib::ledger::*;
}

use fmine_calib::ledger::correction::NO_CLAIM;
use fmine_calib::ledger::{ForecastLedger, ForecastMetadata, ForecastPayload, OutcomeSelector};
use fmine_core::{Digest32, ObjectId};

#[path = "../../fmine-calib/src/ledger/correction/tests.rs"]
mod tests;

#[path = "../../fmine-calib/src/ledger/correction/series/tests.rs"]
mod series_tests;
