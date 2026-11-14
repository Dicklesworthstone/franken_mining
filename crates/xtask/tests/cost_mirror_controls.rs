//! Run the exact cost controls through the production public API, without the metrics lib-test suite.

#![cfg(test)]

use fmine_claim::Claim;
use fmine_evidence::EvidenceClass;
use fmine_metrics::guidance::Period;
use fmine_metrics::mirror::cost::*;
use fmine_numerics::exact_feas::Rat;
use fmine_registry::sources::DocumentRow;
use fmine_registry::transcriptions::{Accepted, Locator};

mod controls {
    include!("../testdata/metrics_mirror_cost_controls.rs");
}
