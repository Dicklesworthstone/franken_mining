//! Render exact stream totals alongside the split implied by a two-outlet dry balance.
//! Determinism: Exact for JSON; human percentages are rounded for display.

use std::fmt::Write as _;
use std::path::Path;

use super::actions;
use crate::{
    Envelope,
    worksheet_input::{markdown_cell, read},
};
use fmine_evidence::Refusal;
use fmine_plant::measurements::{
    two_product::{self, Receipt},
    worksheet::MAX_BYTES,
};
use fmine_units::ExactQty;

fn percent(q: Option<ExactQty>) -> String {
    q.map_or_else(|| "—".into(), |v| format!("{:.2}%", v.to_f64() * 100.0))
}

fn exact_text(q: ExactQty) -> String {
    if q.den() == 1 {
        q.num().to_string()
    } else {
        format!("{}/{}", q.num(), q.den())
    }
}

fn render(receipt: &Receipt) -> Envelope {
    let b = &receipt.claim().value;
    let scope = &receipt.streams()[0].summary().scope;
    let mut md = format!(
        "# Two-product plant balance\n\n{} · interval {} to {} ns\n\nDry matter residual (feed minus outlets): {} kg. Concentrate/feed dry mass: {}.\n\n",
        markdown_cell(&scope.boundary),
        scope.physical_time.start_ns,
        scope.physical_time.end_ns,
        exact_text(b.dry_residual_kg),
        percent(b.concentrate_over_feed_dry_mass)
    );
    for (role, stream) in two_product::ROLES.into_iter().zip(receipt.streams()) {
        let summary = stream.summary();
        let _ = writeln!(
            md,
            "- {role}: {} ({} dry kg)",
            markdown_cell(&summary.scope.material),
            exact_text(summary.amounts.dry_solids_kg)
        );
    }
    let check = b.assay_pull_check();
    let _ = writeln!(
        md,
        "\nAt the submitted precision, {} element assays identify a split, {} are individually infeasible, and {} are unresolved.",
        check.solved_assays, check.infeasible_assays, check.unresolved_assays
    );
    if let Some(reference) = check.reference_assay {
        if let Some(other) = check.conflicting_assay {
            let _ = writeln!(
                md,
                "The solved assays imply different splits. {} / {} gives {} and {} / {} gives {}.",
                markdown_cell(&reference.element.element),
                markdown_cell(&reference.element.measurand),
                exact_text(reference.mass_pull),
                markdown_cell(&other.element.element),
                markdown_cell(&other.element.measurand),
                exact_text(other.mass_pull)
            );
        } else if check.solved_assays > 1 {
            let _ = writeln!(
                md,
                "The solved assays agree on a dry mass pull of {}.",
                exact_text(reference.mass_pull)
            );
        }
    }
    md.push_str("\n| Element / measurand | Metal residual (kg) | Concentrate/feed metal | Assay mass pull | Assay recovery | Assay result | Comparison with submitted dry pull |\n|---|---:|---:|---:|---:|---|---|\n");
    for row in &b.elements {
        let (status, pull, recovery) = row.assay_split.report();
        let _ = writeln!(
            md,
            "| {} / {} | {} | {} | {} | {} | {} | {} |",
            markdown_cell(&row.element.element),
            markdown_cell(&row.element.measurand),
            row.residual_kg.map_or_else(|| "—".into(), exact_text),
            percent(row.concentrate_over_feed_metal),
            percent(pull),
            percent(recovery),
            status.replace('_', " "),
            row.assay_split
                .submitted_dry_pull_comparison(b.concentrate_over_feed_dry_mass)
        );
    }
    md.push_str("\nA dash marks an unavailable value.\n\n");
    md.push_str(two_product::NO_CLAIM);
    let mut envelope = Envelope::ok("plant balance", receipt.to_json(), vec![]);
    envelope.rendered_markdown = Some(md);
    envelope
}

pub(super) fn execute(root: &Path, paths: [&Path; 3]) -> Envelope {
    let run = || -> Result<Envelope, Refusal> {
        let [feed, concentrate, tailings] = paths.map(|path| {
            read(
                &root.join(path),
                "two-product material worksheet",
                MAX_BYTES,
            )
        });
        let inputs = [feed?, concentrate?, tailings?];
        let receipt =
            two_product::evaluate(inputs.each_ref().map(|(s, source)| (s.as_str(), source)))?;
        Ok(render(&receipt))
    };
    let mut envelope = run().unwrap_or_else(|e| Envelope::refused("plant balance", e, actions()));
    envelope.no_claim = Some(two_product::NO_CLAIM);
    envelope
}
