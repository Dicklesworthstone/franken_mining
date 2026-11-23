//! DESIGN: IO only; raw likelihood and conserved extensive states belong to plant.
//! Determinism: DeterministicPerIsa
use super::*;
use fmine_plant::raw_measurements::balance;

pub(super) fn execute(input: &Path, root: &Path) -> Envelope {
    let run = || -> Result<Envelope, Refusal> {
        let (text, source) = worksheet_input::read(
            &root.join(input),
            "plant raw-measurement balance worksheet",
            balance::MAX_BYTES,
        )?;
        let result = balance::evaluate(&text, &source)?;
        let mut envelope = match result.refusal() {
            Some(refusal) => Envelope::refused("plant reconcile-raw", refusal.clone(), actions()),
            None => Envelope::ok("plant reconcile-raw", String::new(), vec![]),
        };
        envelope.payload_json = result.to_json();
        envelope.rendered_markdown = Some(result.to_markdown());
        Ok(envelope)
    };
    let mut envelope = run()
        .unwrap_or_else(|refusal| Envelope::refused("plant reconcile-raw", refusal, actions()));
    envelope.no_claim = Some(balance::NO_CLAIM);
    envelope
}
