//! DESIGN: bounded file input and CLI envelopes for the pure raw-transfer worksheet.
//! The plant owner binds sources, admits declarations, evaluates and renders results.
//! Determinism: Exact nominal quantities; DeterministicPerIsa covariance.

use std::path::Path;

use fmine_core::Digest32;
use fmine_evidence::Refusal;
use fmine_plant::raw_measurements::worksheet::{self, MAX_BYTES, NO_CLAIM};

use super::quoted;
use crate::{Envelope, NextAction, worksheet_input};

pub(super) use fmine_plant::raw_measurements::worksheet::ROBOT_DOCS_JSON;

const VERB: &str = "plant transfer";

fn actions() -> Vec<NextAction> {
    vec![NextAction::new(
        "fmine robot-docs",
        "inspect complete lot population, raw support, matched dry basis, uncertainties and every correlation pair",
    )]
}
fn finish(mut envelope: Envelope) -> Envelope {
    envelope.no_claim = Some(NO_CLAIM);
    envelope
}
fn failed_input(refusal: Refusal, source: Option<&str>) -> Envelope {
    let mut envelope = Envelope::refused(VERB, refusal, actions());
    envelope.payload_json = format!(
        "{{\"schema\":\"fmine.plant-transfer-result.v1\",\"source\":{},\"admitted_claim\":null,\"nominal_transfer\":null,\"receipt_digest\":null,\"result\":null}}",
        source.unwrap_or("null")
    );
    finish(envelope)
}

pub(super) fn execute(input: &Path, root: &Path) -> Envelope {
    let (text, source) =
        match worksheet_input::read(&root.join(input), "raw plant transfer worksheet", MAX_BYTES) {
            Ok(value) => value,
            Err(refusal) => return failed_input(refusal, None),
        };
    let source_json = format!(
        "{{\"path\":{},\"sha256\":{},\"bytes\":{}}}",
        quoted(&source.artifact_path),
        quoted(&Digest32(source.artifact_sha256).to_hex()),
        text.len()
    );
    let result = match worksheet::evaluate(&text, &source) {
        Ok(value) => value,
        Err(refusal) => return failed_input(refusal, Some(&source_json)),
    };
    let mut envelope = match result.refusal() {
        Some(refusal) => Envelope::refused(VERB, refusal.clone(), actions()),
        None => Envelope::ok(VERB, String::new(), vec![]),
    };
    envelope.payload_json = result.to_json().to_owned();
    envelope.no_claim = Some(result.no_claim());
    envelope
}
