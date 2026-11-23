//! DESIGN (BD-225, plan §§17.8, 22.3, 23.4): compare operating alternatives
//! without transcribing their source-bound production and cash calculations.
//! Matching state labels is conditional on an explicit shared-scenario declaration.
//! Determinism: DeterministicPerIsa (inherits the physical calculation).

use super::*;
use fmine_decide::card::downside::DownsideLimit;
use fmine_decide::card::{InformationOption, model::INFORMATION_BASIS};
use std::fmt::Write as _;

#[cfg(test)]
mod tests;
pub mod worksheet;

pub const INPUT_METRIC: &str = "production.comparison.declaration/1";
pub const MAX_ALTERNATIVES: usize = 16;
pub const NO_CLAIM: &str = "Conditional comparison of declared operating alternatives over one complete shared scenario population and physical/commercial support. Common state meaning is an explicit assumption, not established by matching names. Every alternative retains its original production and commercial calculations, including losing alternatives; do-nothing cash is identical before rounding. No physical feasibility, counterfactual identification, joint action or portfolio, calibration, project NPV, safety assessment or execution authorization is supplied.";
pub const INFORMATION_NO_CLAIM: &str = "Conditional comparison of declared operating alternatives and at most one source-declared information option over the same complete scenario population. Common state meaning, likelihoods and continued validity of each action payoff after the test delay are explicit assumptions. The test is observed before acting; test and delay costs are USD in the same immediate cash-margin basis, charged once in every signal outcome, including do nothing. Delay days do not shift physical periods or infer lost production, discounting or additional costs. Exact EVSI, signal policies and policy risk are conditional on these declarations; no calibrated test accuracy, sequential tests, physical feasibility, causal identification, joint action portfolio, project NPV or execution authorization is supplied. Every original source and losing alternative remains retained.";

/// The counterfactual population and comparison support are declared separately
/// from each alternative's original one-action valuation question.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComparisonSpec {
    pub model_key: String,
    pub question: String,
    pub shared_scenario_basis: String,
    pub boundary: String,
    pub circuit: String,
    pub material_domain: String,
    pub dry_basis: String,
    pub element: String,
    pub start_ns: i64,
    pub end_ns: i64,
    pub commercial_support: String,
    pub action_ids: Vec<String>,
    pub downside_limit: Option<DownsideLimit>,
    pub information_basis: Option<String>,
    pub information_options: Vec<InformationOption>,
}

impl ComparisonSpec {
    pub fn canonical_tag(&self) -> String {
        let mut enc = Encoder::new_frame(*b"PVCS", 1);
        for value in [
            &self.model_key,
            &self.question,
            &self.shared_scenario_basis,
            &self.boundary,
            &self.circuit,
            &self.material_domain,
            &self.dry_basis,
            &self.element,
            &self.commercial_support,
        ] {
            enc.put_str(value);
        }
        enc.put_i64(self.start_ns);
        enc.put_i64(self.end_ns);
        let mut ids = self.action_ids.iter().collect::<Vec<_>>();
        ids.sort();
        enc.put_u64(ids.len() as u64);
        for id in ids {
            enc.put_str(id);
        }
        // Existing declarations retain their canonical bytes when no limit is declared.
        if let Some(limit) = &self.downside_limit {
            enc.put_str(&limit.canonical_tag());
        }
        if self.information_basis.is_some() || !self.information_options.is_empty() {
            enc.put_str("source-bound-information/v1");
            enc.put_str(self.information_basis.as_deref().unwrap_or(""));
            let mut options = self.information_options.iter().collect::<Vec<_>>();
            options.sort_by(|a, b| a.option_id.cmp(&b.option_id));
            enc.put_u64(options.len() as u64);
            for option in options {
                enc.put_str(&option.canonical_tag());
            }
        }
        format!("production-comparison/v1:{}", sha256(&enc.finish()))
    }
}

/// All source calculations remain available; the joint card recomputes regret
/// against the complete alternative population rather than reusing pairwise regret.
#[derive(Debug, Clone, PartialEq)]
pub struct ComparisonReceipt {
    declaration: Claim<ComparisonSpec>,
    alternatives: Vec<ValuationReceipt>,
    decision: EvaluatedCard,
    digest: String,
    submitted_manifest: Option<String>,
}

impl ComparisonReceipt {
    pub fn declaration(&self) -> &Claim<ComparisonSpec> {
        &self.declaration
    }
    pub fn alternatives(&self) -> &[ValuationReceipt] {
        &self.alternatives
    }
    pub fn decision(&self) -> &EvaluatedCard {
        &self.decision
    }
    pub fn digest(&self) -> &str {
        &self.digest
    }
}

/// Compare already admitted immutable valuations. No input is dropped and no
/// marginal populations, state weights or alternative payoffs are synthesized.
pub fn evaluate(
    declaration: &Claim<ComparisonSpec>,
    alternatives: &[ValuationReceipt],
) -> Result<ComparisonReceipt> {
    check_claim(declaration)?;
    let spec = &declaration.value;
    if declaration.metric_key != INPUT_METRIC || declaration.value_tag() != spec.canonical_tag() {
        return Err(bad(
            RefusalCode::FrozenRecordMutation507,
            "comparison declaration differs from its Claim",
        ));
    }
    if declaration.representation != Representation::Deterministic
        || declaration.additivity != AdditivityClass::NonAdditive
        || spec.start_ns >= spec.end_ns
        || [
            &spec.model_key,
            &spec.question,
            &spec.shared_scenario_basis,
            &spec.boundary,
            &spec.circuit,
            &spec.material_domain,
            &spec.dry_basis,
            &spec.element,
            &spec.commercial_support,
        ]
        .iter()
        .any(|s| !named(s))
    {
        return Err(bad(
            RefusalCode::SupportMismatch102,
            "explicit shared-scenario interpretation and complete common support required",
        ));
    }
    if spec
        .downside_limit
        .as_ref()
        .is_some_and(|limit| limit.policy_optimization.is_some())
    {
        return Err(bad(
            RefusalCode::NoClaimInDomain201,
            "production comparisons screen act-now and declared fixed one-test policies; policy optimization is not supported",
        ));
    }
    match (
        spec.information_basis.as_deref(),
        spec.information_options.is_empty(),
    ) {
        (None, true) | (Some(INFORMATION_BASIS), false) => {}
        _ => {
            return Err(bad(
                RefusalCode::BasisMismatch105,
                "information options require before_action_same_payoffs_explicit_delay_cost; the basis alone is not an information option",
            ));
        }
    }
    if !(2..=MAX_ALTERNATIVES).contains(&alternatives.len())
        || alternatives.len() != spec.action_ids.len()
    {
        return Err(bad(
            RefusalCode::NoClaimInDomain201,
            "comparison requires two to sixteen complete declared alternatives",
        ));
    }
    let expected = spec
        .action_ids
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    if expected.len() != spec.action_ids.len()
        || expected.iter().any(|id| !named(id) || *id == DO_NOTHING)
    {
        return Err(bad(
            RefusalCode::BasisMismatch105,
            "distinct named action ids, separate from do-nothing, are required",
        ));
    }
    let mut ordered = BTreeMap::new();
    let mut all_meta = Metadata::new();
    all_meta.include(declaration)?;
    for receipt in alternatives {
        let terms = &receipt.terms.value;
        if !expected.contains(terms.action_id.as_str())
            || ordered.insert(terms.action_id.as_str(), receipt).is_some()
        {
            return Err(bad(
                RefusalCode::BasisMismatch105,
                "duplicate, missing or undeclared production alternative",
            ));
        }
        if terms.boundary != spec.boundary
            || terms.circuit != spec.circuit
            || terms.material_domain != spec.material_domain
            || terms.dry_basis != spec.dry_basis
            || terms.element != spec.element
            || terms.start_ns != spec.start_ns
            || terms.end_ns != spec.end_ns
            || terms.commercial_support != spec.commercial_support
            || terms.valuation_basis != VALUATION_BASIS
        {
            return Err(bad(
                RefusalCode::SupportMismatch102,
                format!(
                    "alternative {} does not match the declared comparison support",
                    terms.action_id
                ),
            ));
        }
        check_claim(receipt.decision.claim())?;
        all_meta.include(receipt.decision.claim())?;
    }
    let first = ordered.values().next().ok_or_else(|| {
        bad(
            RefusalCode::NoClaimInDomain201,
            "no production alternatives",
        )
    })?;
    for receipt in ordered.values() {
        if receipt.points.len() != first.points.len()
            || receipt.terms.value.physical_to_commercial_basis
                != first.terms.value.physical_to_commercial_basis
            || receipt.terms.value.monetary_rounding != first.terms.value.monetary_rounding
            || receipt.points.iter().zip(&first.points).any(|(a, b)| {
                a.scenario_id != b.scenario_id
                    || a.weight != b.weight
                    || a.unrounded_do_nothing_usd != b.unrounded_do_nothing_usd
            })
        {
            return Err(bad(
                RefusalCode::BasisMismatch105,
                "all alternatives require the same complete states, exact weights, commercial bridge and unrounded do-nothing cash",
            ));
        }
    }
    let weights = WeightSet {
        model_key: spec.model_key.clone(),
        question: spec.question.clone(),
        states: first
            .points
            .iter()
            .map(|p| (p.scenario_id.clone(), p.weight))
            .collect(),
    };
    let tag = weights.canonical_tag();
    let weights = all_meta.claim(
        "production.comparison.weights/1",
        weights,
        &tag,
        AdditivityClass::NonAdditive,
    );
    let mut actions = Vec::with_capacity(ordered.len() + 1);
    for receipt in ordered.values() {
        let mut meta = Metadata::new();
        meta.include(declaration)?;
        meta.include(receipt.decision.claim())?;
        let action = ActionPayoffs {
            model_key: spec.model_key.clone(),
            action_id: receipt.terms.value.action_id.clone(),
            label: receipt.terms.value.action_label.clone(),
            role: ActionRole::Action,
            currency: Currency::USD,
            valuation_basis: VALUATION_BASIS.into(),
            values: receipt
                .points
                .iter()
                .map(|p| (p.scenario_id.clone(), p.margin))
                .collect(),
        };
        let tag = action.canonical_tag();
        actions.push(meta.claim(
            "production.comparison.action/1",
            action,
            &tag,
            AdditivityClass::ExtensiveAdditive,
        ));
    }
    let hold = ActionPayoffs {
        model_key: spec.model_key.clone(),
        action_id: DO_NOTHING.into(),
        label: "Do nothing (common declared immediate cash)".into(),
        role: ActionRole::DoNothing,
        currency: Currency::USD,
        valuation_basis: VALUATION_BASIS.into(),
        values: first
            .points
            .iter()
            .map(|p| (p.scenario_id.clone(), p.do_nothing))
            .collect(),
    };
    let tag = hold.canonical_tag();
    actions.push(all_meta.claim(
        "production.comparison.action/1",
        hold,
        &tag,
        AdditivityClass::ExtensiveAdditive,
    ));
    let downside_limit = spec.downside_limit.as_ref().map(|limit| {
        all_meta.claim(
            "production.comparison.downside_limit/1",
            limit.clone(),
            &limit.canonical_tag(),
            AdditivityClass::NonAdditive,
        )
    });
    let information_options = spec
        .information_options
        .iter()
        .map(|option| {
            all_meta.claim(
                "production.comparison.information/1",
                option.clone(),
                &option.canonical_tag(),
                AdditivityClass::NonAdditive,
            )
        })
        .collect();
    let decision = card::evaluate(&DecisionInputs {
        weights,
        actions,
        information_options,
        robustness: None,
        downside_limit,
    })?;
    let mut enc = Encoder::new_frame(*b"PVCR", 1);
    enc.put_str(&render::claim(declaration));
    enc.put_u64(ordered.len() as u64);
    for receipt in ordered.values() {
        enc.put_str(receipt.digest());
    }
    enc.put_str(&decision.digest());
    Ok(ComparisonReceipt {
        declaration: declaration.clone(),
        alternatives: ordered.into_values().cloned().collect(),
        decision,
        digest: sha256(&enc.finish()).to_hex(),
        submitted_manifest: None,
    })
}

impl ComparisonReceipt {
    pub fn no_claim(&self) -> &'static str {
        if self.declaration.value.information_options.is_empty() {
            NO_CLAIM
        } else {
            INFORMATION_NO_CLAIM
        }
    }

    pub fn to_json(&self) -> String {
        use render::js;
        let spec = &self.declaration.value;
        let mut ids = spec
            .action_ids
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>();
        ids.sort();
        let scope = format!(
            "{{\"boundary\":{},\"circuit\":{},\"material_domain\":{},\"dry_basis\":{},\"element\":{},\"start_ns\":\"{}\",\"end_ns\":\"{}\",\"commercial_support\":{}}}",
            js(&spec.boundary),
            js(&spec.circuit),
            js(&spec.material_domain),
            js(&spec.dry_basis),
            js(&spec.element),
            spec.start_ns,
            spec.end_ns,
            js(&spec.commercial_support),
        );
        let information = spec
            .information_basis
            .as_ref()
            .map_or_else(String::new, |basis| {
                format!(",\"information_basis\":{}", js(basis))
            });
        format!(
            "{{\"schema\":\"fmine.production-comparison.receipt.v1\",\"model_key\":{},\"question\":{},\"shared_scenario_basis\":{}{information},\"scope\":{},\"action_ids\":[{}],\"declaration\":{},\"submitted_manifest\":{},\"alternatives\":[{}],\"decision\":{},\"digest\":{},\"no_claim\":{}}}",
            js(&spec.model_key),
            js(&spec.question),
            js(&spec.shared_scenario_basis),
            scope,
            ids.into_iter().map(js).collect::<Vec<_>>().join(","),
            render::claim(&self.declaration),
            self.submitted_manifest
                .as_deref()
                .map_or_else(|| "null".into(), js),
            self.alternatives
                .iter()
                .map(ValuationReceipt::to_json)
                .collect::<Vec<_>>()
                .join(","),
            self.decision.to_json(),
            js(&self.digest),
            js(self.no_claim()),
        )
    }

    pub fn to_markdown(&self) -> String {
        use render::{md, monetary, shown};
        let spec = &self.declaration.value;
        let summary = self.decision.summary();
        let brief = summary
            .downside
            .as_ref()
            .map_or_else(String::new, |limit| limit.decision_brief());
        let no_claim = self.no_claim();
        let mut out = format!(
            "# Production alternatives\n\n{}\n\n{brief}{no_claim}\n\n**Shared scenario assumption:** {}.\n\n**Boundary / circuit:** {} / {}. **Physical interval:** [{}, {}). **Element:** {}.\n\n**Commercial support:** {}. **Evidence:** {:?}. **Confidentiality:** {}.\n\n",
            md(&spec.question),
            md(&spec.shared_scenario_basis),
            md(&spec.boundary),
            md(&spec.circuit),
            spec.start_ns,
            spec.end_ns,
            md(&spec.element),
            md(&spec.commercial_support),
            self.decision.claim().evidence,
            md(&self.decision.claim().confidentiality.to_string()),
        );
        out.push_str("## Comparison under the common population\n\nCVaR95 integrates the largest 5% of signed loss = minus immediate cash margin. Regret compares every alternative in the same state; all ties survive.\n\n| Action | Expected USD | P10 / P50 / P90 USD | CVaR95 loss USD | Mean regret USD | Maximum regret USD |\n|---|---:|---:|---:|---:|---:|\n");
        for action in &summary.alternatives {
            let _ = writeln!(
                out,
                "| {} ({}) | {} | {} / {} / {} | {} | {} | {} |",
                md(&action.label),
                md(&action.action_id),
                shown(action.mean.major),
                shown(action.p10_p50_p90[0].major),
                shown(action.p10_p50_p90[1].major),
                shown(action.p10_p50_p90[2].major),
                shown(action.cvar95_loss.major),
                shown(action.mean_regret.major),
                shown(action.max_regret.major)
            );
        }
        let _ = writeln!(
            out,
            "\n**Expected-value co-winners:** {}. **Minimax-regret co-winners:** {}.\n\n{}",
            md(&summary.expected_value_winners.join(", ")),
            md(&summary.minimax_regret_winners.join(", ")),
            card::TIE_RULE
        );
        if let Some(information) = self.decision.information_to_markdown() {
            let _ = writeln!(
                out,
                "\n**Information basis:** {INFORMATION_BASIS}. Test and delay costs use the same immediate USD cash-margin basis; unchanged post-delay action payoffs are declared, not recalculated.\n"
            );
            out.push_str(&information);
            if let Some(downside) = self.decision.downside_to_markdown() {
                out.push_str(&downside);
            }
        }
        if let Some(screen) = &summary.downside
            && summary.information.is_none()
        {
            let _ = writeln!(
                out,
                "\n## Downside probability limit\n\n**Breach:** rounded immediate cash margin < **{} USD**; equality is not a breach. **Maximum probability:** **{}**, inclusive. Probabilities remain conditional on the declared uncalibrated scenarios; the unconstrained ranking above is unchanged. This is not a safety limit or a bound on loss severity.\n\n| Action | Breach probability | Eligibility |\n|---|---:|---|",
                monetary(screen.limit.floor),
                shown(screen.limit.max_breach_probability),
            );
            for choice in &screen.choices {
                let _ = writeln!(
                    out,
                    "| {} | {} | {} |",
                    md(&choice.choice.id),
                    shown(choice.nominal_breach_probability),
                    if choice.nominal_eligible {
                        "eligible"
                    } else {
                        "rejected"
                    },
                );
            }
            if screen.nominal.winners.is_empty() {
                out.push_str("\n**No eligible choice.** Every declared action fails this probability limit; no fallback is selected.\n");
            } else {
                let winners = screen
                    .nominal
                    .winners
                    .iter()
                    .map(|choice| choice.id.as_str())
                    .collect::<Vec<_>>()
                    .join(", ");
                let _ = writeln!(
                    out,
                    "\n**Downside-eligible expected-value co-winners:** {}. **Expected-value cost of the limit:** {} USD. All exact ties remain.\n",
                    md(&winners),
                    screen
                        .nominal
                        .expected_value_cost_of_limit
                        .map_or_else(|| "unavailable".into(), shown),
                );
            }
        }
        out.push_str("\n## Retained alternative calculations\n\nMoney is rounded once to 0.0001 USD with ties to even; delta means rounded minus unrounded USD. All original worksheets and full unrounded component calculations remain in JSON.\n\n| Action | State | Weight | Dry tonnes | Recovered kg | Revenue USD | Royalty USD | Variable USD | Fixed USD | Margin USD | Rounding delta USD | Do-nothing USD |\n|---|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|\n");
        for receipt in &self.alternatives {
            for p in receipt.points() {
                let _ = writeln!(
                    out,
                    "| {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} |",
                    md(&receipt.terms.value.action_id),
                    md(&p.scenario_id),
                    shown(p.weight),
                    shown(p.represented_dry_tonnes),
                    shown(p.represented_recovered_kg),
                    shown(p.gross_revenue_usd),
                    shown(p.royalty_usd),
                    shown(p.variable_cost_usd),
                    shown(p.fixed_cost_usd),
                    monetary(p.margin),
                    shown(p.margin_rounding_delta_usd),
                    monetary(p.do_nothing)
                );
            }
        }
        out.push_str("\n## Source ancestry\n\n");
        let mut artifacts = BTreeSet::new();
        for source in &self.decision.claim().anchors {
            artifacts.insert((&source.artifact_path, source.artifact_sha256));
        }
        for (path, hash) in artifacts {
            let _ = writeln!(
                out,
                "- {} — SHA-256 `{}`",
                md(path),
                fmine_core::Digest32(hash).to_hex()
            );
        }
        for receipt in &self.alternatives {
            let _ = writeln!(
                out,
                "\nAlternative {}: production `{}`, valuation `{}`.",
                md(&receipt.terms.value.action_id),
                receipt.production.digest(),
                receipt.digest()
            );
        }
        let _ = writeln!(out, "\nComparison receipt digest: `{}`.", self.digest);
        out
    }
}
