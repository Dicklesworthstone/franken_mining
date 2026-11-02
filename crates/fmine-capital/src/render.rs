//! Canonical source-complete capital reports. No render-time arithmetic changes a selection.
//! Determinism: Exact (colour and UU metadata use their stored IEEE bits).

use std::fmt::Write as _;

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
    format!(
        "{{\"policy\":{},\"weights\":{},\"candidates\":[{candidates}]}}",
        claim_json(&inputs.policy, &policy_json(&inputs.policy.value)),
        claim_json(&inputs.weights, &weights_json(&inputs.weights.value))
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
    }
}

pub(super) fn summary_json(s: &ProgramSummary) -> String {
    let choices = s.choices.iter().map(|r| format!("{{\"bundle_id\":{},\"selected\":{},\"spend\":{},\"budget_slack\":{},\"structural_rejections\":[{}],\"risk_rejected\":{},\"tail_loss\":{},\"statistics\":{}}}", js(&r.bundle_id), ids(&r.selected), rq(r.spend.major), rq(r.budget_slack.major), r.structural_rejections.iter().map(rejection_json).collect::<Vec<_>>().join(","), r.risk_rejected, r.tail_loss.map_or_else(|| "null".into(), |v| rq(v.major)), r.statistics.as_ref().map_or_else(|| "null".into(), statistics_json))).collect::<Vec<_>>().join(",");
    format!(
        "{{\"model_key\":{},\"question\":{},\"currency\":{},\"monetary_scale\":\"major_currency_units\",\"status\":{},\"policy\":{},\"choices\":[{choices}],\"unconstrained_winners\":{},\"feasible_winners\":{},\"expected_value_cost_of_risk_limit\":{},\"selection_frequency\":null,\"selection_frequency_status\":\"unavailable_static_declared_model\",\"regret_reference\":\"all_structurally_feasible_subsets_before_risk_limit\"}}",
        js(&s.model_key),
        js(&s.question),
        currency(s.policy.currency),
        js(if s.feasible_winners.is_empty() {
            "no_risk_feasible_program"
        } else {
            "ok"
        }),
        policy_json(&s.policy),
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
            js(NO_CLAIM)
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
            NO_CLAIM,
            md(&s.policy.valuation_basis),
            md(&s.policy.reference_class_basis),
            mq(s.policy.budget).trim_matches('"'),
            ratio(s.policy.tail_mass),
            s.policy.max_tail_loss.map_or_else(
                || "unbounded".into(),
                |v| mq(v).trim_matches('"').to_string()
            ),
            md(&s.unconstrained_winners.join(", ")),
            if s.feasible_winners.is_empty() {
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
