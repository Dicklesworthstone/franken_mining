//! DESIGN: bounded source IO and CLI envelopes for declared recovery cards (BD-176/287).
//! Admission, Claim construction and conditional evaluation belong to the plant owner.
//! Determinism: DeterministicPerIsa

use std::path::Path;

use fmine_evidence::Refusal;
use fmine_plant::cards::worksheet;

use crate::{Envelope, NextAction, worksheet_input};

pub(super) const ROBOT_DOCS_JSON: &str = worksheet::ROBOT_DOCS_JSON;

fn actions() -> Vec<NextAction> {
    vec![NextAction::new(
        "fmine robot-docs",
        "inspect declared recovery-card units, validity bounds and versioned layers",
    )]
}

pub(super) fn execute(input: &Path, root: &Path) -> Envelope {
    let run = || -> Result<Envelope, Refusal> {
        let (text, source) = worksheet_input::read(
            &root.join(input),
            "plant recovery worksheet",
            worksheet::MAX_BYTES,
        )?;
        let result = worksheet::evaluate(&text, &source)?;
        let mut envelope = match result.refusal() {
            Some(refusal) => Envelope::refused("plant recovery", refusal.clone(), actions()),
            None => Envelope::ok("plant recovery", String::new(), vec![]),
        };
        envelope.payload_json = result.to_json();
        envelope.no_claim = Some(result.no_claim());
        Ok(envelope)
    };
    let mut envelope =
        run().unwrap_or_else(|refusal| Envelope::refused("plant recovery", refusal, actions()));
    envelope.no_claim.get_or_insert(worksheet::NO_CLAIM);
    envelope.warnings.push("Formula, applicability bounds, versioned layers, route and processing-support inputs are Assumed, Internal and uncalibrated; source-byte binding does not establish plant applicability or measurement accuracy.".into());
    envelope
}
