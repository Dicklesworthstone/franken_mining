//! DESIGN (BD-225/229): source-derived operating cash becomes a declared-period investment comparison.
//! Determinism: DeterministicPerIsa

use crate::{Envelope, NextAction, json_string, worksheet_input};
use fmine_evidence::Refusal;
use fmine_metrics::valuation::{investment, worksheet};
use std::path::Path;

pub(super) const USAGE: &str = "decide investment --manifest <investment.toml>";
pub(super) const ROBOT_DOCS_JSON: &str = r#"{"schema":"fmine.production-investment.v1","command":"decide investment --manifest <path>","fields":["schema","model_key","question","shared_scenario_basis","discount_basis","cashflow_basis","currency","valuation_time_ns","boundary","circuit","material_domain","dry_basis","element","commercial_support","monetary_rounding"],"period_fields":["id","start_ns","end_ns","discount_factor"],"alternative_fields":["id","label","initial_capex_usd"],"bundle_fields":["alternative","period","input","grinding","recovery","terms"],"uu_fields":["id","description","trigger","downside_usd_m"],"downside_limit_fields":["floor_usd","max_breach_probability"],"downside_limit":"Optional single table; floor exactly representable in 0.0001 USD and probability in [0,1]. Screens final rounded discounted incremental value after initial capex, strictly below floor; equality is not a breach and the probability cap is inclusive. Declared information policies use realized state/signal payoffs net of test and delay cost. All choices and nominal winners remain visible; no eligible choice has no fallback. No per-period liquidity, loss-severity bound or constrained-policy optimization is inferred","information":{"optional_top_field":"information_basis","required_basis":"before_action_same_payoffs_explicit_delay_cost","information_option_fields":["id","label","test_cost","delay_cost","delay_days"],"signal_fields":["option","id"],"likelihood_fields":["option","state","signal","probability"],"types":"Costs are quoted nonnegative exact USD decimals on the 0.0001 grid; delay_days is a nonnegative integer; probabilities are quoted exact n/d fractions","population":"Every option explicitly includes every admitted state/signal pair, including zero probabilities, and each conditional state row sums to one. No missing observation or state substitution","timing_and_cost_basis":"The result is assumed available before choosing one complete investment, with all original cash paths unchanged after the declared delay. Test and delay costs are already present-value USD at valuation_time_ns, charged once on every branch including do-nothing. Delay days do not shift periods, infer loss, discount costs again or change initial capital","output":"Exact finite EVSI, net VOI, posterior signal policies, realized net payoff risk and downside eligibility, with original Claims and sources; no Monte Carlo sampling or calibration claim","portfolio_boundary":"Information-bearing investment sources refuse when supplied as act-now portfolio candidates; no joint contingent portfolio policy is inferred","example":"tests/decisions/investment_testwork.toml"},"basis":"USD only; cashflow_basis=incremental_operating_cash_excludes_initial_capex; monetary_rounding=nearest_ties_even_0_0001_usd","calculation":"Sum each complete scenario path's unrounded operating cash minus its same-period unrounded do-nothing cash times the explicit period discount factor, then subtract the nonnegative initial investment once. Round only the final investment outcome; do-nothing has zero incremental value","support":"One through eight mutually exclusive investments, one through sixteen nonoverlapping periods, valuation time no later than every period start, every investment-period source bundle, at most 65536 bundle/state rows. Same exact state ids/weights and commercial bridge across time; baseline cash must agree across alternatives in each period before rounding. Positive discount factors are explicit multipliers, never inferred annual rates","sources":"All source paths relative to workspace root. Every physical/commercial bundle is recomputed and retained with original worksheets and SHA-256. The 64-KiB manifest requires at least one positive named UU assumption","scope":"Only declared periods; no inferred tax, financing, ownership, terminal value, omitted cash, project lifetime, NAV, calibrated probability or authorization","example":"tests/decisions/production_investment.toml"}"#;

/// The admitted investment and the complete source snapshots used to derive it.
pub(crate) struct LoadedInvestment {
    receipt: investment::InvestmentReceipt,
    payload: String,
}

impl LoadedInvestment {
    pub(crate) fn receipt(&self) -> &investment::InvestmentReceipt {
        &self.receipt
    }

    pub(crate) fn to_json(&self) -> String {
        self.payload.clone()
    }
}

/// Resolve every declared source against the same workspace root before admitting any result.
pub(crate) fn load(path: &Path, root: &Path) -> Result<LoadedInvestment, Refusal> {
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
    Ok(LoadedInvestment { receipt, payload })
}

pub(super) fn execute(path: &Path, root: &Path) -> Envelope {
    let run = || -> Result<Envelope, Refusal> {
        let loaded = load(path, root)?;
        let mut envelope = Envelope::ok("decide investment", loaded.to_json(), vec![]);
        envelope.rendered_markdown = Some(loaded.receipt().to_markdown());
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
