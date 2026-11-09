//! DESIGN: source-bound worksheet IO for conditional Bond grinding throughput.
//! Determinism: DeterministicPerIsa

use super::*;
use fmine_plant::grinding::worksheet;

pub(super) fn execute(input: &Path, root: &Path) -> Envelope {
    let run = || -> Result<Envelope, Refusal> {
        let (text, source) = worksheet_input::read(
            &root.join(input),
            "plant grinding worksheet",
            worksheet::MAX_BYTES,
        )?;
        let (mut envelope, result) = match worksheet::evaluate(&text, &source) {
            Ok(result) => {
                let mut envelope = Envelope::ok("plant grinding", String::new(), vec![]);
                envelope.rendered_markdown = Some(result.to_markdown());
                (envelope, result.to_json())
            }
            Err(refusal) => (
                Envelope::refused("plant grinding", refusal, actions()),
                format!(
                    "{{\"schema\":\"fmine.plant-grinding-result.v1\",\"submitted_worksheet\":{},\"model\":null,\"scenarios\":[],\"summary\":null,\"admitted_claim\":null,\"receipt_digest\":null}}",
                    quoted(&text),
                ),
            ),
        };
        envelope.payload_json = format!(
            "{{\"source\":{},\"grinding\":{result}}}",
            worksheet_input::source_json(&source, text.len()),
        );
        Ok(envelope)
    };
    let mut envelope =
        run().unwrap_or_else(|refusal| Envelope::refused("plant grinding", refusal, actions()));
    envelope.no_claim = Some(worksheet::NO_CLAIM);
    envelope.warnings.push(
        "Submitted grinding model, validity domains and complete joint scenarios are Assumed, Internal and uncalibrated; source-byte binding does not establish plant applicability.".into(),
    );
    envelope
}
