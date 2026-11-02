//! Canonical source-complete capital reports. No render-time arithmetic changes a selection.
//! Determinism: Exact (colour and UU metadata use their stored IEEE bits).

use std::{collections::BTreeMap, fmt::Write as _};

use fmine_evidence::{AoaStatus, AssertionSource, Color};

use super::*;

fn js(value: &str) -> String {
    let mut out = String::from("\"");
    for c in value.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c.is_control() => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}
fn ratio(value: Rat) -> String {
    format!("{}/{}", value.num(), value.den())
}
fn rq(value: Rat) -> String {
    js(&ratio(value))
}
fn mq(value: Money) -> String {
    let (mut a, mut b) = (value.minor.unsigned_abs(), Money::SCALE as u128);
    while b != 0 {
        (a, b) = (b, a % b);
    }
    // gcd divides the fixed 10,000-unit scale, so this cast always fits signed i128.
    let divisor = a as i128;
    js(&format!(
        "{}/{}",
        value.minor / divisor,
        Money::SCALE / divisor
    ))
}
fn ids(values: &[String]) -> String {
    format!(
        "[{}]",
        values.iter().map(|s| js(s)).collect::<Vec<_>>().join(",")
    )
}
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
fn currency(value: Currency) -> String {
    js(&format!("{value:?}"))
}

pub(super) fn value_tag(kind: &str, value: &str) -> String {
    let mut enc = Encoder::new_frame(*b"CAPI", 1);
    enc.put_str(kind);
    enc.put_str(value);
    format!("{kind}:{}", sha256(&enc.finish()))
}

pub(super) fn policy_json(p: &ProgramPolicy) -> String {
    format!(
        "{{\"model_key\":{},\"currency\":{},\"valuation_basis\":{},\"budget\":{},\"budget_currency\":{},\"tail_mass\":{},\"max_tail_loss\":{},\"max_tail_loss_currency\":{},\"payoff_convention\":{},\"reference_class_basis\":{}}}",
        js(&p.model_key),
        currency(p.currency),
        js(&p.valuation_basis),
        mq(p.budget),
        currency(p.budget.ccy),
        rq(p.tail_mass),
        p.max_tail_loss.map_or_else(|| "null".into(), mq),
        p.max_tail_loss
            .map_or_else(|| "null".into(), |v| currency(v.ccy)),
        js(&p.payoff_convention),
        js(&p.reference_class_basis)
    )
}

pub(super) fn candidate_json(c: &CapitalCandidate) -> String {
    let mut payoffs = c.payoffs.clone();
    payoffs.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.minor.cmp(&b.1.minor)));
    let payoffs = payoffs
        .iter()
        .map(|(id, v)| {
            format!(
                "{{\"state_id\":{},\"value\":{},\"currency\":{}}}",
                js(id),
                mq(*v),
                currency(v.ccy)
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let mut requires = c.requires.clone();
    requires.sort();
    format!(
        "{{\"model_key\":{},\"candidate_id\":{},\"label\":{},\"currency\":{},\"valuation_basis\":{},\"cost\":{},\"cost_currency\":{},\"requires\":{},\"exclusion_group\":{},\"reference_class_basis\":{},\"payoffs\":[{payoffs}]}}",
        js(&c.model_key),
        js(&c.candidate_id),
        js(&c.label),
        currency(c.currency),
        js(&c.valuation_basis),
        mq(c.cost),
        currency(c.cost.ccy),
        ids(&requires),
        c.exclusion_group
            .as_deref()
            .map_or_else(|| "null".into(), js),
        js(&c.reference_class_basis)
    )
}

pub(super) fn funding_json(funding: &ProgramFunding) -> String {
    let mut available = funding.available_cash.clone();
    available.sort_by(|a, b| a.0.cmp(&b.0));
    let available = available
        .iter()
        .map(|(id, value)| {
            format!(
                "{{\"state_id\":{},\"value\":{},\"currency\":{}}}",
                js(id),
                mq(*value),
                currency(value.ccy)
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let mut requirements = funding.requirements.clone();
    requirements
        .sort_by(|a, b| (&a.candidate_id, &a.state_id).cmp(&(&b.candidate_id, &b.state_id)));
    let requirements = requirements
        .iter()
        .map(|r| {
            format!(
                "{{\"candidate_id\":{},\"state_id\":{},\"value\":{},\"currency\":{}}}",
                js(&r.candidate_id),
                js(&r.state_id),
                mq(r.value),
                currency(r.value.ccy)
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{{\"model_key\":{},\"currency\":{},\"cash_period\":{},\"cash_basis\":{},\"minimum_cash\":{},\"minimum_cash_currency\":{},\"available_cash\":[{available}],\"requirements\":[{requirements}]}}",
        js(&funding.model_key),
        currency(funding.currency),
        js(&funding.cash_period),
        js(&funding.cash_basis),
        mq(funding.minimum_cash),
        currency(funding.minimum_cash.ccy)
    )
}

pub(super) fn funding_schedule_json(funding: &FundingSchedule) -> String {
    let mut periods: Vec<_> = funding.periods.iter().collect();
    periods.sort_by(|a, b| (&a.cash_period, &a.id).cmp(&(&b.cash_period, &b.id)));
    let period_order: BTreeMap<_, _> = periods
        .iter()
        .enumerate()
        .map(|(index, period)| (period.id.as_str(), index))
        .collect();
    let position = |id: &str| period_order.get(id).copied().unwrap_or(usize::MAX);
    let periods = periods
        .iter()
        .map(|p| {
            format!(
                "{{\"id\":{},\"cash_period\":{},\"minimum_cash\":{},\"minimum_cash_currency\":{}}}",
                js(&p.id),
                js(&p.cash_period),
                mq(p.minimum_cash),
                currency(p.minimum_cash.ccy)
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let mut opening: Vec<_> = funding.opening_cash.iter().collect();
    opening.sort_by(|a, b| a.0.cmp(&b.0));
    let opening = opening
        .iter()
        .map(|(id, value)| {
            format!(
                "{{\"state_id\":{},\"value\":{},\"currency\":{}}}",
                js(id),
                mq(*value),
                currency(value.ccy)
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let mut baseline: Vec<_> = funding.baseline_flows.iter().collect();
    baseline.sort_by(|a, b| {
        (position(&a.period_id), &a.period_id, &a.state_id).cmp(&(
            position(&b.period_id),
            &b.period_id,
            &b.state_id,
        ))
    });
    let baseline = baseline
        .iter()
        .map(|f| {
            format!(
                "{{\"period_id\":{},\"state_id\":{},\"value\":{},\"currency\":{}}}",
                js(&f.period_id),
                js(&f.state_id),
                mq(f.value),
                currency(f.value.ccy)
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let mut candidates: Vec<_> = funding.candidate_flows.iter().collect();
    candidates.sort_by(|a, b| {
        (
            position(&a.period_id),
            &a.period_id,
            &a.candidate_id,
            &a.state_id,
        )
            .cmp(&(
                position(&b.period_id),
                &b.period_id,
                &b.candidate_id,
                &b.state_id,
            ))
    });
    let candidates = candidates
        .iter()
        .map(|f| {
            format!(
                "{{\"period_id\":{},\"candidate_id\":{},\"state_id\":{},\"value\":{},\"currency\":{}}}",
                js(&f.period_id),
                js(&f.candidate_id),
                js(&f.state_id),
                mq(f.value),
                currency(f.value.ccy)
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{{\"model_key\":{},\"currency\":{},\"cash_basis\":{},\"opening_date\":{},\"opening_cash\":[{opening}],\"periods\":[{periods}],\"baseline_flows\":[{baseline}],\"candidate_flows\":[{candidates}]}}",
        js(&funding.model_key),
        currency(funding.currency),
        js(&funding.cash_basis),
        js(&funding.opening_date)
    )
}

fn weights_json(w: &WeightSet) -> String {
    let mut states = w.states.clone();
    states.sort_by(|a, b| a.0.cmp(&b.0));
    let states = states
        .iter()
        .map(|(id, weight)| format!("{{\"state_id\":{},\"weight\":{}}}", js(id), rq(*weight)))
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{{\"model_key\":{},\"question\":{},\"states\":[{states}]}}",
        js(&w.model_key),
        js(&w.question)
    )
}

fn evidence(value: EvidenceClass) -> &'static str {
    match value {
        EvidenceClass::Measured => "measured",
        EvidenceClass::Derived => "derived",
        EvidenceClass::ModelledCalibrated => "modelled_calibrated",
        EvidenceClass::ModelledUncalibrated => "modelled_uncalibrated",
        EvidenceClass::Assumed => "assumed",
        EvidenceClass::Elicited => "elicited",
        EvidenceClass::Transferred {
            aoa: AoaStatus::Inside,
        } => "transferred_inside",
        EvidenceClass::Transferred {
            aoa: AoaStatus::Outside,
        } => "transferred_outside",
        EvidenceClass::Transferred {
            aoa: AoaStatus::NotAssessed,
        } => "transferred_not_assessed",
        EvidenceClass::Assertion {
            source: AssertionSource::Issuer,
        } => "assertion_issuer",
        EvidenceClass::Assertion {
            source: AssertionSource::Agent,
        } => "assertion_agent",
        EvidenceClass::Assertion {
            source: AssertionSource::Extraction,
        } => "assertion_extraction",
        EvidenceClass::Assertion {
            source: AssertionSource::Secondary,
        } => "assertion_secondary",
    }
}
fn color_json(value: Color) -> String {
    match value {
        Color::Verified { lo, hi } => format!(
            "{{\"name\":\"verified\",\"lo_bits\":\"{:016x}\",\"hi_bits\":\"{:016x}\"}}",
            lo.to_bits(),
            hi.to_bits()
        ),
        Color::Estimated {
            estimator,
            dispersion,
        } => format!(
            "{{\"name\":\"estimated\",\"estimator\":{},\"dispersion_bits\":\"{:016x}\"}}",
            js(&estimator),
            dispersion.to_bits()
        ),
        Color::Validated { regime, dataset } => {
            let bounds = regime
                .bounds()
                .iter()
                .map(|(axis, (lo, hi))| {
                    format!(
                        "{{\"axis\":{},\"lo_bits\":\"{:016x}\",\"hi_bits\":\"{:016x}\"}}",
                        js(axis),
                        lo.to_bits(),
                        hi.to_bits()
                    )
                })
                .collect::<Vec<_>>()
                .join(",");
            format!(
                "{{\"name\":\"validated\",\"dataset\":{},\"regime\":[{bounds}]}}",
                js(&dataset)
            )
        }
    }
}

/// Bind/display metadata omitted by generic Claim identity, including the computed colour and exact UU bits.
fn claim_json<T>(claim: &Claim<T>, value: &str) -> String {
    let anchors = claim
        .anchors
        .iter()
        .map(|a| {
            format!(
                "{{\"artifact_path\":{},\"sha256\":{},\"locator\":{}}}",
                js(&a.artifact_path),
                js(&hex(&a.artifact_sha256)),
                js(&a.locator)
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let uu = claim.uu_terms.iter().map(|u| format!("{{\"scenario_id\":{},\"description\":{},\"downside_exposure_usd_m\":{},\"downside_bits\":\"{:016x}\",\"trigger_indicator\":{}}}", js(&u.scenario_id), js(&u.description), u.downside_exposure_usd_m, u.downside_exposure_usd_m.to_bits(), js(&u.trigger_indicator))).collect::<Vec<_>>().join(",");
    let color = claim
        .color()
        .map_or_else(|e| format!("{{\"refusal\":{}}}", js(&e.what)), color_json);
    let additivity = match claim.additivity {
        AdditivityClass::ExtensiveAdditive => "extensive_additive",
        AdditivityClass::IntensiveWeighted => "intensive_weighted",
        AdditivityClass::NonAdditive => "non_additive",
    };
    format!(
        "{{\"id\":{},\"metric_key\":{},\"value_tag\":{},\"value\":{value},\"canonical_body_hex\":{},\"representation\":\"deterministic\",\"support\":\"period_aggregate\",\"additivity\":{},\"evidence\":{},\"evidence_tag\":{},\"epistemic_tag\":{},\"state\":{},\"state_tag\":{},\"confidentiality\":{},\"uu_terms\":[{uu}],\"parents\":{},\"anchors\":[{anchors}],\"supersedes\":{},\"stale\":{},\"calibration_failed\":{},\"color\":{color}}}",
        js(&claim.claim_id.to_string()),
        js(&claim.metric_key),
        js(claim.value_tag()),
        js(&hex(&claim.canonical_body())),
        js(additivity),
        js(evidence(claim.evidence)),
        js(&hex(&claim.evidence.tag())),
        js(&hex(&claim.epistemic.tag())),
        js(if claim.state == KnowledgeState::Known {
            "known"
        } else {
            "estimated"
        }),
        claim.state.tag(),
        js(&claim.confidentiality.to_string()),
        ids(&claim
            .parents
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()),
        claim
            .supersedes
            .map_or_else(|| "null".into(), |id| js(&id.to_string())),
        claim.stale,
        claim.calibration_failed
    )
}

pub(super) fn inputs_json(inputs: &ProgramInputs) -> String {
    let candidates = inputs
        .candidates
        .iter()
        .map(|c| claim_json(c, &candidate_json(&c.value)))
        .collect::<Vec<_>>()
        .join(",");
    let schedule = inputs
        .funding_schedule
        .as_ref()
        .map_or_else(String::new, |claim| {
            format!(
                ",\"funding_schedule\":{}",
                claim_json(claim, &funding_schedule_json(&claim.value))
            )
        });
    format!(
        "{{\"policy\":{},\"weights\":{},\"candidates\":[{candidates}],\"funding\":{}{schedule}}}",
        claim_json(&inputs.policy, &policy_json(&inputs.policy.value)),
        claim_json(&inputs.weights, &weights_json(&inputs.weights.value)),
        inputs.funding.as_ref().map_or_else(
            || "null".into(),
            |claim| claim_json(claim, &funding_json(&claim.value))
        )
    )
}

fn statistics_json(s: &AlternativeStats) -> String {
    let states = s.states.iter().map(|r| format!("{{\"state_id\":{},\"weight\":{},\"payoff\":{},\"best_in_hindsight\":{},\"regret\":{}}}", js(&r.state_id), rq(r.weight), rq(r.payoff.major), rq(r.best_in_hindsight.major), rq(r.regret.major))).collect::<Vec<_>>().join(",");
    format!(
        "{{\"mean\":{},\"p10\":{},\"p50\":{},\"p90\":{},\"cvar95_loss\":{},\"mean_regret\":{},\"max_regret\":{},\"states\":[{states}]}}",
        rq(s.mean.major),
        rq(s.p10_p50_p90[0].major),
        rq(s.p10_p50_p90[1].major),
        rq(s.p10_p50_p90[2].major),
        rq(s.cvar95_loss.major),
        rq(s.mean_regret.major),
        rq(s.max_regret.major)
    )
}

fn rejection_json(r: &StructuralRejection) -> String {
    match r {
        StructuralRejection::BudgetExceeded => "{\"kind\":\"budget_exceeded\"}".into(),
        StructuralRejection::MissingPrerequisite {
            candidate,
            prerequisite,
        } => format!(
            "{{\"kind\":\"missing_prerequisite\",\"candidate\":{},\"prerequisite\":{}}}",
            js(candidate),
            js(prerequisite)
        ),
        StructuralRejection::MutualExclusion { group, candidates } => format!(
            "{{\"kind\":\"mutual_exclusion\",\"group\":{},\"candidates\":{}}}",
            js(group),
            ids(candidates)
        ),
        StructuralRejection::LiquidityShortfall {
            state_id,
            shortfall,
        } => format!(
            "{{\"kind\":\"liquidity_shortfall\",\"state_id\":{},\"shortfall\":{}}}",
            js(state_id),
            rq(shortfall.major)
        ),
        StructuralRejection::CashScheduleShortfall {
            period_id,
            checkpoint_date,
            state_id,
            shortfall,
        } => format!(
            "{{\"kind\":\"cash_schedule_shortfall\",\"period_id\":{},\"checkpoint_date\":{},\"state_id\":{},\"shortfall\":{}}}",
            period_id.as_deref().map_or_else(|| "null".into(), js),
            js(checkpoint_date),
            js(state_id),
            rq(shortfall.major)
        ),
    }
}

fn funding_assessment_json(funding: &FundingAssessment) -> String {
    let states = funding.states.iter().map(|s| format!(
        "{{\"state_id\":{},\"available_cash\":{},\"cash_required\":{},\"remaining_cash\":{},\"minimum_cash\":{},\"slack\":{}}}",
        js(&s.state_id), rq(s.available_cash.major), rq(s.cash_required.major), rq(s.remaining_cash.major), rq(s.minimum_cash.major), rq(s.slack.major)
    )).collect::<Vec<_>>().join(",");
    format!(
        "{{\"states\":[{states}],\"minimum_slack\":{}}}",
        rq(funding.minimum_slack.major)
    )
}

fn funding_schedule_assessment_json(funding: &FundingScheduleAssessment) -> String {
    let checkpoints = funding.checkpoints.iter().map(|c| format!(
        "{{\"period_id\":{},\"checkpoint_date\":{},\"state_id\":{},\"opening_cash\":{},\"baseline_cash_flow\":{},\"candidate_cash_flow\":{},\"closing_cash\":{},\"minimum_cash\":{},\"slack\":{}}}",
        c.period_id.as_deref().map_or_else(|| "null".into(), js),
        js(&c.checkpoint_date), js(&c.state_id), rq(c.opening_cash.major),
        rq(c.baseline_cash_flow.major), rq(c.candidate_cash_flow.major),
        rq(c.closing_cash.major), rq(c.minimum_cash.major), rq(c.slack.major)
    )).collect::<Vec<_>>().join(",");
    format!(
        "{{\"checkpoints\":[{checkpoints}],\"minimum_slack\":{}}}",
        rq(funding.minimum_slack.major)
    )
}

pub(super) fn summary_json(s: &ProgramSummary) -> String {
    let choices = s.choices.iter().map(|r| {
        let schedule = r.funding_schedule.as_ref().map_or_else(String::new, |funding| {
            format!(",\"funding_schedule\":{}", funding_schedule_assessment_json(funding))
        });
        format!("{{\"bundle_id\":{},\"selected\":{},\"spend\":{},\"budget_slack\":{},\"funding\":{}{schedule},\"structural_rejections\":[{}],\"risk_rejected\":{},\"tail_loss\":{},\"statistics\":{}}}", js(&r.bundle_id), ids(&r.selected), rq(r.spend.major), rq(r.budget_slack.major), r.funding.as_ref().map_or_else(|| "null".into(), funding_assessment_json), r.structural_rejections.iter().map(rejection_json).collect::<Vec<_>>().join(","), r.risk_rejected, r.tail_loss.map_or_else(|| "null".into(), |v| rq(v.major)), r.statistics.as_ref().map_or_else(|| "null".into(), statistics_json))
    }).collect::<Vec<_>>().join(",");
    let schedule = s
        .funding_schedule
        .as_ref()
        .map_or_else(String::new, |funding| {
            format!(",\"funding_schedule\":{}", funding_schedule_json(funding))
        });
    format!(
        "{{\"model_key\":{},\"question\":{},\"currency\":{},\"monetary_scale\":\"major_currency_units\",\"status\":{},\"policy\":{},\"funding\":{}{schedule},\"choices\":[{choices}],\"unconstrained_winners\":{},\"feasible_winners\":{},\"expected_value_cost_of_risk_limit\":{},\"selection_frequency\":null,\"selection_frequency_status\":\"unavailable_static_declared_model\",\"regret_reference\":\"all_structurally_feasible_subsets_before_risk_limit\"}}",
        js(&s.model_key),
        js(&s.question),
        currency(s.policy.currency),
        js(if s.unconstrained_winners.is_empty() {
            "no_structurally_feasible_program"
        } else if s.feasible_winners.is_empty() {
            "no_risk_feasible_program"
        } else {
            "ok"
        }),
        policy_json(&s.policy),
        s.funding
            .as_ref()
            .map_or_else(|| "null".into(), funding_json),
        ids(&s.unconstrained_winners),
        ids(&s.feasible_winners),
        s.expected_value_cost_of_risk_limit
            .map_or_else(|| "null".into(), |v| rq(v.major))
    )
}

fn md(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('|', "\\|")
        .replace('`', "\\`")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace(['\n', '\r'], " ")
}
fn shown(value: Option<ExactMoney>) -> String {
    value.map_or_else(|| "unavailable".into(), |v| ratio(v.major))
}

impl EvaluatedProgram {
    #[must_use]
    pub fn to_json(&self) -> String {
        let summary = summary_json(self.summary());
        format!(
            "{{\"schema\":\"fmine.capital-program.v1\",\"digest\":{},\"summary\":{summary},\"claim\":{},\"inputs\":{},\"no_claim\":{},\"reserve_disclosure_claim\":false,\"execution_authorized\":false,\"geotechnical_signoff\":false,\"regulatory_compliance_claim\":false}}",
            js(&self.digest),
            claim_json(&self.claim, &summary),
            inputs_json(&self.inputs),
            js(self.no_claim())
        )
    }

    #[must_use]
    pub fn to_markdown(&self) -> String {
        let s = self.summary();
        let mut out = format!(
            "# Static capital program comparison\n\n{}\n\n**Declared model:** `{}`. **Evidence:** {}. **Confidentiality:** {}. **Digest:** `{}`.\n\n{}\n\n**Valuation basis:** {}. **Reference-class basis:** {}.\n\nBudget: {} USD. Upper loss tail mass: {}. Maximum tail loss: {} USD. Candidate net delta-NAV already includes spending; the cost column constrains the budget only.\n\n**Expected-value co-winners before risk limit:** {}. **Risk-feasible co-winners:** {}. **Expected-value cost of the risk limit:** {} USD.\n\nSelection frequency is unavailable for this declared static model. Regret compares every structurally feasible subset before the tail-loss limit. All monetary values below are exact rational major USD.\n\n## Every declared subset\n\n| Bundle | Candidates | Spend | Budget slack | Mean net delta-NAV | Tail loss | CVaR95 loss | Mean regret | Max regret | Status |\n|---|---|---:|---:|---:|---:|---:|---:|---:|---|\n",
            md(&s.question),
            md(&s.model_key),
            evidence(self.claim.evidence),
            md(&self.claim.confidentiality.to_string()),
            self.digest,
            self.no_claim(),
            md(&s.policy.valuation_basis),
            md(&s.policy.reference_class_basis),
            mq(s.policy.budget).trim_matches('"'),
            ratio(s.policy.tail_mass),
            s.policy.max_tail_loss.map_or_else(
                || "unbounded".into(),
                |v| mq(v).trim_matches('"').to_string()
            ),
            if s.unconstrained_winners.is_empty() {
                "none: no structurally feasible program".into()
            } else {
                md(&s.unconstrained_winners.join(", "))
            },
            if s.unconstrained_winners.is_empty() {
                "none: no structurally feasible program".into()
            } else if s.feasible_winners.is_empty() {
                "none: no risk-feasible program".into()
            } else {
                md(&s.feasible_winners.join(", "))
            },
            shown(s.expected_value_cost_of_risk_limit)
        );
        for row in &s.choices {
            let status = if !row.structural_rejections.is_empty() {
                row.structural_rejections
                    .iter()
                    .map(|r| match r {
                        StructuralRejection::BudgetExceeded => "budget exceeded".into(),
                        StructuralRejection::MissingPrerequisite {
                            candidate,
                            prerequisite,
                        } => format!("{candidate} requires {prerequisite}"),
                        StructuralRejection::MutualExclusion { group, candidates } => {
                            format!("group {group}: {} are exclusive", candidates.join(", "))
                        }
                        StructuralRejection::LiquidityShortfall {
                            state_id,
                            shortfall,
                        } => {
                            format!(
                                "cash-floor shortfall in {state_id}: {} USD",
                                ratio(shortfall.major)
                            )
                        }
                        StructuralRejection::CashScheduleShortfall {
                            period_id,
                            checkpoint_date,
                            state_id,
                            shortfall,
                        } => format!(
                            "cash-floor shortfall in {state_id} at {} ({checkpoint_date}): {} USD",
                            period_id.as_deref().unwrap_or("opening"),
                            ratio(shortfall.major)
                        ),
                    })
                    .collect::<Vec<String>>()
                    .join("; ")
            } else if row.risk_rejected {
                "risk limit exceeded".into()
            } else {
                "feasible".into()
            };
            let stats = &row.statistics;
            let _ = writeln!(
                out,
                "| `{}` | {} | {} | {} | {} | {} | {} | {} | {} | {} |",
                row.bundle_id,
                if row.selected.is_empty() {
                    "Do nothing".into()
                } else {
                    md(&row.selected.join(" + "))
                },
                ratio(row.spend.major),
                ratio(row.budget_slack.major),
                shown(stats.as_ref().map(|v| v.mean)),
                shown(row.tail_loss),
                shown(stats.as_ref().map(|v| v.cvar95_loss)),
                shown(stats.as_ref().map(|v| v.mean_regret)),
                shown(stats.as_ref().map(|v| v.max_regret)),
                md(&status)
            );
        }
        if let Some(funding) = &self.inputs.funding {
            let f = &funding.value;
            let _ = writeln!(
                out,
                "\n## Funding over the declared cash window\n\n**Cash period:** {}. **Cash basis:** {}. **Minimum cash:** {} USD.\n\nEvery shared state must retain the declared floor after the program's cash requirements. This covers aggregate cash over the declared window; it does not establish every intra-period balance or a staged funding schedule. Cash requirements do not alter the supplied net delta-NAV payoffs.\n",
                md(&f.cash_period),
                md(&f.cash_basis),
                mq(f.minimum_cash).trim_matches('"')
            );
            claim_markdown(&mut out, funding);
            out.push_str("\n| Bundle | State | Available before program | Cash required | Cash remaining | Minimum cash | Slack |\n|---|---|---:|---:|---:|---:|---:|\n");
            for choice in &s.choices {
                if let Some(result) = &choice.funding {
                    for state in &result.states {
                        let _ = writeln!(
                            out,
                            "| `{}` | {} | {} | {} | {} | {} | {} |",
                            choice.bundle_id,
                            md(&state.state_id),
                            ratio(state.available_cash.major),
                            ratio(state.cash_required.major),
                            ratio(state.remaining_cash.major),
                            ratio(state.minimum_cash.major),
                            ratio(state.slack.major)
                        );
                    }
                }
            }
        } else if self.inputs.funding_schedule.is_none() {
            out.push_str("\nScenario funding: not declared.\n");
        }
        if let Some(funding) = &self.inputs.funding_schedule {
            funding_schedule_markdown(&mut out, funding, &s.choices);
        }
        out.push_str("\n## Candidate assumptions and source Claims\n\nEvery candidate remains an input, including candidates absent from all winning subsets.\n\n");
        for candidate in &self.inputs.candidates {
            let c = &candidate.value;
            let _ = writeln!(
                out,
                "### {} — {}\n\nSpending: {} USD. Reference-class basis: {}. Prerequisites: {}. Exclusion group: {}.\n",
                md(&c.candidate_id),
                md(&c.label),
                mq(c.cost).trim_matches('"'),
                md(&c.reference_class_basis),
                if c.requires.is_empty() {
                    "none".into()
                } else {
                    md(&c.requires.join(", "))
                },
                c.exclusion_group
                    .as_deref()
                    .map_or_else(|| "none".into(), md)
            );
            claim_markdown(&mut out, candidate);
            out.push_str("\n| State | Declared net delta-NAV (USD) |\n|---|---:|\n");
            for (state, value) in &c.payoffs {
                let _ = writeln!(out, "| {} | {} |", md(state), mq(*value).trim_matches('"'));
            }
        }
        out.push_str(
            "\n## Shared scenario weights and policy\n\n| State | Exact weight |\n|---|---:|\n",
        );
        for (id, weight) in &self.inputs.weights.value.states {
            let _ = writeln!(out, "| {} | {} |", md(id), ratio(*weight));
        }
        claim_markdown(&mut out, &self.inputs.weights);
        claim_markdown(&mut out, &self.inputs.policy);
        out.push_str("\n## Declared unknown unknowns\n\n");
        for u in &self.claim.uu_terms {
            let _ = writeln!(
                out,
                "- {}: {}. Downside exposure: {} USD millions. Trigger: {}.",
                md(&u.scenario_id),
                md(&u.description),
                u.downside_exposure_usd_m,
                md(&u.trigger_indicator)
            );
        }
        out.push_str("\nAll four authority flags are false: reserve disclosure, execution authorization, geotechnical sign-off and regulatory compliance.\n");
        out
    }
}

fn funding_schedule_markdown(
    out: &mut String,
    funding: &Claim<FundingSchedule>,
    choices: &[ProgramChoice],
) {
    let f = &funding.value;
    let _ = writeln!(
        out,
        "\n## Funding at declared cash checkpoints\n\n**Opening date:** {}. **Cash basis:** {}.\n\nPositive cash flows are receipts; negative cash flows are expenditures. Each shared state starts with its declared opening cash. Baseline flows and the selected candidates' flows then change that balance period by period. The opening balance must meet the first period's floor, and every period-end balance must meet that period's floor. An earlier breach remains a rejection even when later receipts restore cash.\n\nThese checks cover the declared opening and period-end dates. They do not establish every intra-period balance or a contingent funding strategy. Cash flows do not alter the supplied net delta-NAV payoffs. All monetary values are exact rational major USD.\n",
        md(&f.opening_date),
        md(&f.cash_basis)
    );
    claim_markdown(out, funding);
    let mut periods: Vec<_> = f.periods.iter().collect();
    periods.sort_by(|a, b| (&a.cash_period, &a.id).cmp(&(&b.cash_period, &b.id)));
    let period_order: BTreeMap<_, _> = periods
        .iter()
        .enumerate()
        .map(|(index, period)| (period.id.as_str(), index))
        .collect();
    let position = |id: &str| period_order.get(id).copied().unwrap_or(usize::MAX);
    out.push_str("\n### Declared cash periods and floors\n\n| Period | Inclusive cash period | Minimum cash (USD) |\n|---|---|---:|\n");
    for p in &periods {
        let _ = writeln!(
            out,
            "| {} | {} | {} |",
            md(&p.id),
            md(&p.cash_period),
            mq(p.minimum_cash).trim_matches('"')
        );
    }
    let mut opening: Vec<_> = f.opening_cash.iter().collect();
    opening.sort_by(|a, b| a.0.cmp(&b.0));
    out.push_str("\n### Declared opening cash\n\n| State | Opening cash (USD) |\n|---|---:|\n");
    for (state, value) in opening {
        let _ = writeln!(out, "| {} | {} |", md(state), mq(*value).trim_matches('"'));
    }
    let mut baseline: Vec<_> = f.baseline_flows.iter().collect();
    baseline.sort_by(|a, b| {
        (position(&a.period_id), &a.period_id, &a.state_id).cmp(&(
            position(&b.period_id),
            &b.period_id,
            &b.state_id,
        ))
    });
    out.push_str("\n### Declared baseline cash flows\n\n| Period | State | Net cash flow (USD) |\n|---|---|---:|\n");
    for flow in baseline {
        let _ = writeln!(
            out,
            "| {} | {} | {} |",
            md(&flow.period_id),
            md(&flow.state_id),
            mq(flow.value).trim_matches('"')
        );
    }
    let mut candidates: Vec<_> = f.candidate_flows.iter().collect();
    candidates.sort_by(|a, b| {
        (
            position(&a.period_id),
            &a.period_id,
            &a.candidate_id,
            &a.state_id,
        )
            .cmp(&(
                position(&b.period_id),
                &b.period_id,
                &b.candidate_id,
                &b.state_id,
            ))
    });
    out.push_str("\n### Declared candidate cash flows\n\nEvery candidate's signed flows remain visible, including candidates absent from all winning subsets. Explicit zero rows are retained.\n\n| Period | Candidate | State | Net cash flow (USD) |\n|---|---|---|---:|\n");
    for flow in candidates {
        let _ = writeln!(
            out,
            "| {} | {} | {} | {} |",
            md(&flow.period_id),
            md(&flow.candidate_id),
            md(&flow.state_id),
            mq(flow.value).trim_matches('"')
        );
    }
    for choice in choices {
        if let Some(result) = &choice.funding_schedule {
            let _ = writeln!(
                out,
                "\n### Cash timeline for `{}`\n\nMinimum cash-floor slack across every state and checkpoint: {} USD.\n\n| State | Period | Checkpoint date | Opening cash | Baseline net | Program net | Closing cash | Floor | Slack | Checkpoint result |\n|---|---|---|---:|---:|---:|---:|---:|---:|---|",
                md(&choice.bundle_id),
                ratio(result.minimum_slack.major)
            );
            let mut checkpoints: Vec<_> = result.checkpoints.iter().collect();
            checkpoints.sort_by(|a, b| {
                (&a.state_id, &a.checkpoint_date, &a.period_id).cmp(&(
                    &b.state_id,
                    &b.checkpoint_date,
                    &b.period_id,
                ))
            });
            for c in checkpoints {
                let _ = writeln!(
                    out,
                    "| {} | {} | {} | {} | {} | {} | {} | {} | {} | {} |",
                    md(&c.state_id),
                    c.period_id.as_deref().map_or_else(|| "Opening".into(), md),
                    md(&c.checkpoint_date),
                    ratio(c.opening_cash.major),
                    ratio(c.baseline_cash_flow.major),
                    ratio(c.candidate_cash_flow.major),
                    ratio(c.closing_cash.major),
                    ratio(c.minimum_cash.major),
                    ratio(c.slack.major),
                    if c.slack.major.num() < 0 {
                        "floor breach"
                    } else {
                        "floor met"
                    }
                );
            }
        }
    }
}

fn claim_markdown<T>(out: &mut String, claim: &Claim<T>) {
    let _ = writeln!(
        out,
        "\nClaim `{}` — {}; evidence {}; confidentiality {}.",
        claim.claim_id,
        md(&claim.metric_key),
        evidence(claim.evidence),
        md(&claim.confidentiality.to_string())
    );
    for a in &claim.anchors {
        let _ = writeln!(
            out,
            "- {} — {}; SHA-256 `{}`.",
            md(&a.artifact_path),
            md(&a.locator),
            hex(&a.artifact_sha256)
        );
    }
}
