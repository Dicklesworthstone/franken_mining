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

fn kg(q: ExactQty) -> String {
    if q.den() == 1 {
        q.num().to_string()
    } else {
        format!("{}/{}", q.num(), q.den())
    }
}

fn render(receipt: &Receipt) -> Envelope {
    let b = &receipt.claim.value;
    let scope = &receipt.streams[0].summary().scope;
    let mut md = format!(
        "# Two-product plant balance\n\n{} · interval {} to {} ns\n\nDry matter residual (feed minus outlets): {} kg. Concentrate/feed dry mass: {}.\n\n",
        markdown_cell(&scope.boundary),
        scope.physical_time.start_ns,
        scope.physical_time.end_ns,
        kg(b.dry_residual_kg),
        percent(b.concentrate_over_feed_dry_mass)
    );
    for (role, stream) in two_product::ROLES.into_iter().zip(&receipt.streams) {
        let summary = stream.summary();
        let _ = writeln!(
            md,
            "- {role}: {} ({} dry kg)",
            markdown_cell(&summary.scope.material),
            kg(summary.amounts.dry_solids_kg)
        );
    }
    md.push_str("\n| Element / measurand | Metal residual (kg) | Concentrate/feed metal | Assay mass pull | Assay recovery | Assay result |\n|---|---:|---:|---:|---:|---|\n");
    for row in &b.elements {
        let (status, pull, recovery) = row.assay_split.report();
        let _ = writeln!(
            md,
            "| {} / {} | {} | {} | {} | {} | {} |",
            markdown_cell(&row.element.element),
            markdown_cell(&row.element.measurand),
            row.residual_kg.map_or_else(|| "—".into(), kg),
            percent(row.concentrate_over_feed_metal),
            percent(pull),
            percent(recovery),
            status.replace('_', " ")
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
