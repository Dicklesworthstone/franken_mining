//! Exact BD-355 controls through the public metrics API.
//! Determinism: Exact
#![allow(clippy::expect_used, clippy::unwrap_used)]

use fmine_claim::{AdditivityClass, Claim, Confidentiality, Representation, SpatialSupport};
use fmine_evidence::{EpistemicStatus, EvidenceClass, KnowledgeState};
use fmine_metrics::comparison::*;
use fmine_numerics::exact_feas::{Interval, Rat};
use fmine_registry::sources::DocumentRow;
use fmine_registry::transcriptions::{Accepted, Locator, parse_token};

include!("../testdata/metrics_comparison_controls.rs");
