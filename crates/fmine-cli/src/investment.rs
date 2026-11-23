//! DESIGN (BD-225/229): source-derived operating cash becomes a declared-period investment comparison.
//! Determinism: DeterministicPerIsa

use crate::{Envelope, NextAction, json_string, worksheet_input};
use fmine_evidence::Refusal;
use fmine_metrics::valuation::{investment, worksheet};
use std::path::Path;

pub(super) const USAGE: &str = "decide investment --manifest <investment.toml>";
pub(super) const ROBOT_DOCS_JSON: &str = r#"{"schema":"fmine.production-investment.v1","command":"decide investment --manifest <path>","fields":["schema","model_key","question","shared_scenario_basis","discount_basis","cashflow_basis","currency","valuation_time_ns","boundary","circuit","material_domain","dry_basis","element","commercial_support","monetary_rounding"],"period_fields":["id","start_ns","end_ns","discount_factor"],"alternative_fields":["id","label","initial_capex_usd"],"bundle_fields":["alternative","period","input","grinding","recovery","terms"],"uu_fields":["id","description","trigger","downside_usd_m"],"downside_limit":{"section":"Optional single [[downside_limit]]","fields":["floor_usd","max_breach_probability"],"event":"Final rounded discounted incremental value after initial capex strictly below floor_usd","selection":"Maximum probability is inclusive. Rank eligible investments and do-nothing by nominal expected value. Retain every tie and an empty eligible set."},"basis":"USD only; cashflow_basis=incremental_operating_cash_excludes_initial_capex; monetary_rounding=nearest_ties_even_0_0001_usd","calculation":"Sum each complete scenario path's unrounded operating cash minus its same-period unrounded do-nothing cash times the explicit period discount factor, then subtract the nonnegative initial investment once. Round only the final investment outcome; do-nothing has zero incremental value","support":"One through eight mutually exclusive investments, one through sixteen nonoverlapping periods, valuation time no later than every period start, every investment-period source bundle, at most 65536 bundle/state rows. Same exact state ids/weights and commercial bridge across time; baseline cash must agree across alternatives in each period before rounding. Positive discount factors are explicit multipliers, never inferred annual rates","sources":"All source paths relative to workspace root. Every physical/commercial bundle is recomputed and retained with original worksheets and SHA-256. The 64-KiB manifest requires at least one positive named UU assumption","scope":"Only declared periods; no inferred tax, financing, ownership, terminal value, omitted cash, project lifetime, NAV, calibrated probability or authorization","example":"tests/decisions/production_investment.toml"}"#;

pub(super) fn execute(path: &Path, root: &Path) -> Envelope {
    let run = || -> Result<Envelope, Refusal> {
        let (text, source) = worksheet_input::read(
            &root.join(path),
            "production investment manifest",
            investment::worksheet::MAX_BYTES,
        )?;
        let manifest = investment::worksheet::parse(&text, &source)?;
        let mut bundles = Vec::with_capacity(manifest.sources().len());
        let mut snapshots = Vec::with_capacity(manifest.sources().len());
        for b in manifest.sources() {
            let production = crate::plant::production::load(
                Path::new(&b.input),
                Path::new(&b.grinding),
                Path::new(&b.recovery),
                root,
            )?;
            let (terms, terms_source) = worksheet_input::read(
                &root.join(&b.terms),
                "production valuation terms",
                worksheet::MAX_BYTES,
            )?;
            let valuation = worksheet::evaluate(&terms, &terms_source, production.receipt())?;
            snapshots.push(format!(r#"{{"alternative_id":{},"period_id":{},"production":{},"terms":{{"source":{},"submitted_worksheet":{}}}}}"#,
                json_string(&b.alternative), json_string(&b.period), production.to_json(), worksheet_input::source_json(&terms_source, terms.len()), json_string(&terms)));
            bundles.push(investment::PeriodValuation {
                alternative_id: b.alternative.clone(),
                period_id: b.period.clone(),
                valuation,
            });
        }
        let receipt = manifest.evaluate(&bundles)?;
        let payload = format!(
            r#"{{"schema":"fmine.production-investment-command.v1","mode":"declared_periods_investment_cash","manifest":{{"source":{},"submitted_worksheet":{}}},"sources":[{}],"investment":{}}}"#,
            worksheet_input::source_json(&source, text.len()),
            json_string(&text),
            snapshots.join(","),
            receipt.to_json()
        );
        let mut envelope = Envelope::ok("decide investment", payload, vec![]);
        envelope.rendered_markdown = Some(receipt.to_markdown());
        Ok(envelope)
    };
    let mut envelope = run().unwrap_or_else(|r| {
        Envelope::refused(
            "decide investment",
            r,
            vec![NextAction::new(
                "fmine robot-docs",
                "inspect investment period, scenario and source requirements",
            )],
        )
    });
    envelope.no_claim = Some(investment::NO_CLAIM);
    envelope
}
