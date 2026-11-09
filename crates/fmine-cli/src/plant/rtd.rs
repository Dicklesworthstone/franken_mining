//! DESIGN: bounded worksheet IO and CLI envelopes for passive RTD transport.
//! Determinism: DeterministicPerIsa

use super::*;
use fmine_plant::rtd::worksheet;

pub(super) fn execute(input: &Path, root: &Path) -> Envelope {
    let run = || -> Result<Envelope, Refusal> {
        let (text, source) = worksheet_input::read(
            &root.join(input),
            "plant RTD worksheet",
            worksheet::MAX_BYTES,
        )?;
        let (mut envelope, result) = match worksheet::evaluate(&text, &source) {
            Ok(result) => {
                let mut envelope = Envelope::ok("plant rtd", String::new(), vec![]);
                envelope.rendered_markdown = Some(result.to_markdown());
                (envelope, result.to_json())
            }
            Err(refusal) => (
                Envelope::refused("plant rtd", refusal, actions()),
                format!(
                    "{{\"schema\":\"fmine.plant-rtd-result.v1\",\"submitted_worksheet\":{},\"admitted_claim\":null,\"receipt_digest\":null,\"result\":null,\"recovery\":null}}",
                    quoted(&text),
                ),
            ),
        };
        envelope.payload_json = format!(
            "{{\"source\":{{\"path\":{},\"sha256\":{},\"bytes\":{}}},\"transport\":{result}}}",
            quoted(&source.artifact_path),
            quoted(&Digest32(source.artifact_sha256).to_hex()),
            text.len(),
        );
        Ok(envelope)
    };
    let mut envelope =
        run().unwrap_or_else(|refusal| Envelope::refused("plant rtd", refusal, actions()));
    envelope.no_claim = Some(worksheet::NO_CLAIM);
    envelope.warnings.push(
        "Submitted RTD, inventories, feeds and any recovery assumptions are Assumed, Internal and uncalibrated; source-byte binding does not establish plant applicability.".into(),
    );
    envelope
}
