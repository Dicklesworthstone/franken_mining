//! DESIGN: bounded source IO for a complete coupled plant calculation (BD-176/181).
//! All model admission and numerical work stay with the existing plant owners.

use std::path::Path;

use fmine_core::anchor::SourceAnchor;
use fmine_evidence::{Refusal, RefusalCode};
use fmine_plant::{
    cards, grinding,
    production::{ProductionReceipt, worksheet},
};

use crate::{Envelope, json_string as quoted, worksheet_input};

pub(crate) struct LoadedProduction {
    result: worksheet::WorksheetResult,
    grinding_text: String,
    grinding_source: SourceAnchor,
    recovery_text: String,
    recovery_source: SourceAnchor,
}

impl LoadedProduction {
    pub(crate) fn receipt(&self) -> &ProductionReceipt {
        self.result.receipt()
    }

    /// Exact source snapshots plus the admitted joined receipt. Parent digests and
    /// Claims in the result bind the computations without repeating every source row.
    pub(crate) fn to_json(&self) -> String {
        let source = |text: &str, anchor: &SourceAnchor| {
            format!(
                "{{\"source\":{},\"locator\":{},\"submitted_worksheet\":{}}}",
                worksheet_input::source_json(anchor, text.len()),
                quoted(&anchor.locator),
                quoted(text),
            )
        };
        format!(
            "{{\"schema\":\"fmine.plant-production-command.v1\",\"sources\":{{\"grinding\":{},\"recovery\":{}}},\"production\":{}}}",
            source(&self.grinding_text, &self.grinding_source),
            source(&self.recovery_text, &self.recovery_source),
            self.result.to_json(),
        )
    }

    pub(crate) fn to_markdown(&self) -> String {
        self.result.to_markdown()
    }
}

pub(crate) fn load(
    input: &Path,
    grinding: &Path,
    recovery: &Path,
    root: &Path,
) -> Result<LoadedProduction, Refusal> {
    let (grinding_text, grinding_source) = worksheet_input::read(
        &root.join(grinding),
        "plant grinding worksheet",
        grinding::worksheet::MAX_BYTES,
    )?;
    let grinding_result = grinding::worksheet::evaluate(&grinding_text, &grinding_source)?;
    let (recovery_text, recovery_source) = worksheet_input::read(
        &root.join(recovery),
        "plant recovery worksheet",
        cards::worksheet::MAX_BYTES,
    )?;
    let recovery_result = cards::worksheet::evaluate(&recovery_text, &recovery_source)?;
    if let Some(refusal) = recovery_result.refusal() {
        return Err(refusal.clone());
    }
    let ensemble = recovery_result.ensemble_receipt().ok_or_else(|| {
        Refusal::new(
            RefusalCode::NoClaimInDomain201,
            "plant production requires a complete declared recovery scenario population",
        )
    })?;
    let (text, source) = worksheet_input::read(
        &root.join(input),
        "plant production worksheet",
        worksheet::MAX_BYTES,
    )?;
    let result = worksheet::evaluate(&text, &source, grinding_result.receipt(), ensemble)?;
    Ok(LoadedProduction {
        result,
        grinding_text,
        grinding_source,
        recovery_text,
        recovery_source,
    })
}

pub(super) fn execute(input: &Path, grinding: &Path, recovery: &Path, root: &Path) -> Envelope {
    let mut envelope = match load(input, grinding, recovery, root) {
        Ok(result) => {
            let mut envelope = Envelope::ok("plant production", result.to_json(), vec![]);
            envelope.rendered_markdown = Some(result.to_markdown());
            envelope
        }
        Err(refusal) => Envelope::refused("plant production", refusal, super::actions()),
    };
    envelope.no_claim = Some(worksheet::NO_CLAIM);
    envelope
}
