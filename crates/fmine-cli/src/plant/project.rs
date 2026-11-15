//! DESIGN: explicit whole-material worksheets into exact nominal elemental kg (BD-177/336).
//! Submitted source values and support declarations remain Assumed and Internal.
//! Determinism: Exact

use std::path::Path;

use crate::{Envelope, NextAction, worksheet_input};
use fmine_evidence::Refusal;
use fmine_plant::measurements::worksheet;

pub(super) const ROBOT_DOCS_JSON: &str = worksheet::ROBOT_DOCS_JSON;

fn actions() -> Vec<NextAction> {
    vec![NextAction::new(
        "fmine robot-docs",
        "inspect the explicit plant material, moisture and total-element assay bases",
    )]
}

pub(super) fn execute(input: &Path, root: &Path) -> Envelope {
    let run = || -> Result<Envelope, Refusal> {
        let (text, source) = worksheet_input::read(
            &root.join(input),
            "plant material worksheet",
            worksheet::MAX_BYTES,
        )?;
        let result = worksheet::evaluate(&text, &source)?;
        let mut envelope = match result.refusal() {
            Some(refusal) => Envelope::refused("plant project", refusal.clone(), actions()),
            None => Envelope::ok("plant project", String::new(), vec![]),
        };
        envelope.payload_json = result.to_json().into();
        Ok(envelope)
    };
    let mut envelope =
        run().unwrap_or_else(|refusal| Envelope::refused("plant project", refusal, actions()));
    envelope.no_claim = Some(worksheet::NO_CLAIM);
    envelope.warnings.push("All worksheet values, total-assay interpretation and whole-material support are Assumed, Internal and uncalibrated; byte binding does not verify measurement accuracy or sample representativeness.".into());
    envelope
}
