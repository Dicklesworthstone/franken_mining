//! DESIGN: IO only; trajectory parsing, inference and rendering belong to the plant owner.
//! Determinism: DeterministicPerIsa
use super::*;
use fmine_plant::trajectory::worksheet;

pub(super) fn execute(input: &Path, root: &Path) -> Envelope {
    let run = || -> Result<Envelope, Refusal> {
        let (text, source) = worksheet_input::read(
            &root.join(input),
            "plant trajectory worksheet",
            worksheet::MAX_BYTES,
        )?;
        let result = match worksheet::evaluate(&text, &source) {
            Ok(result) => result,
            Err(refusal) => {
                let payload = worksheet::refusal_json(&text, &source, &refusal);
                let mut envelope = Envelope::refused("plant trajectory", refusal, actions());
                envelope.payload_json = payload;
                return Ok(envelope);
            }
        };
        let mut envelope = match result.refusal() {
            Some(refusal) => Envelope::refused("plant trajectory", refusal.clone(), actions()),
            None => Envelope::ok("plant trajectory", String::new(), vec![]),
        };
        envelope.payload_json = result.to_json();
        envelope.rendered_markdown = Some(result.to_markdown());
        Ok(envelope)
    };
    let mut envelope =
        run().unwrap_or_else(|refusal| Envelope::refused("plant trajectory", refusal, actions()));
    envelope.no_claim = Some(worksheet::NO_CLAIM);
    envelope
}
