//! DESIGN: component arithmetic and the existing decision statistics share one receipt.
//! Determinism: DeterministicPerIsa

use super::*;
use std::fmt::Write as _;

pub(super) fn js(value: &str) -> String {
    let mut out = String::from("\"");
    for c in value.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            c if c.is_control() => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}
pub(super) fn md(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('|', "&#124;")
        .replace('`', "&#96;")
        .replace(['\n', '\r'], " ")
}
fn rat(value: Rat) -> String {
    format!(
        "{{\"numerator\":\"{}\",\"denominator\":\"{}\"}}",
        value.num(),
        value.den()
    )
}
pub(super) fn shown(value: Rat) -> String {
    if value.den() == 1 {
        value.num().to_string()
    } else {
        format!("{}/{}", value.num(), value.den())
    }
}
pub(super) fn monetary(value: Money) -> String {
    let n = value.minor.unsigned_abs();
    format!(
        "{}{}.{:04}",
        if value.minor < 0 { "-" } else { "" },
        n / 10_000,
        n % 10_000
    )
}
fn anchor(source: &SourceAnchor) -> String {
    format!(
        "{{\"path\":{},\"sha256\":{},\"locator\":{}}}",
        js(&source.artifact_path),
        js(&fmine_core::Digest32(source.artifact_sha256).to_hex()),
        js(&source.locator)
    )
}
pub(super) fn claim<T>(claim: &Claim<T>) -> String {
    let body = claim
        .canonical_body()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    let parents = claim
        .parents
        .iter()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .map(|p| js(&p.to_string()))
        .collect::<Vec<_>>()
        .join(",");
    let sources = claim
        .anchors
        .iter()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .map(anchor)
        .collect::<Vec<_>>()
        .join(",");
    let uu = claim.uu_terms.iter().map(|u| format!(
        "{{\"id\":{},\"description\":{},\"trigger\":{},\"downside_usd_m\":{},\"downside_bits\":\"{:016x}\"}}",
        js(&u.scenario_id), js(&u.description), js(&u.trigger_indicator),
        u.downside_exposure_usd_m, u.downside_exposure_usd_m.to_bits(),
    )).collect::<Vec<_>>().join(",");
    format!(
        concat!(
            "{{\"id\":{},\"metric\":{},\"value_tag\":{},\"canonical_body_hex\":{},",
            "\"evidence\":{},\"epistemic\":{},\"knowledge\":{},\"confidentiality\":{},",
            "\"representation\":{},\"support\":{},\"additivity\":{},\"stale\":{},\"calibration_failed\":{},",
            "\"parents\":[{}],\"sources\":[{}],\"uu_terms\":[{}],\"supersedes\":{},\"color\":{}}}"
        ),
        js(&claim.claim_id.to_string()),
        js(&claim.metric_key),
        js(claim.value_tag()),
        js(&body),
        js(&format!("{:?}", claim.evidence)),
        js(&format!("{:?}", claim.epistemic)),
        js(&format!("{:?}", claim.state)),
        js(&claim.confidentiality.to_string()),
        js(&format!("{:?}", claim.representation)),
        js(&format!("{:?}", claim.support)),
        js(&format!("{:?}", claim.additivity)),
        claim.stale,
        claim.calibration_failed,
        parents,
        sources,
        uu,
        claim
            .supersedes
            .map_or_else(|| "null".into(), |id| js(&id.to_string())),
        js(&format!("{:?}", claim.color()))
    )
}

fn terms(t: &ValuationTerms) -> String {
    let rows = t
        .scenarios
        .iter()
        .map(|s| (s.id.as_str(), s))
        .collect::<BTreeMap<_, _>>();
    let rows = rows.values().map(|s| format!(concat!(
        "{{\"id\":{},\"weight\":{},\"payable_fraction\":{},\"usd_per_kg\":{},",
        "\"variable_usd_per_dry_t\":{},\"fixed_usd\":{},\"royalty_fraction\":{},\"do_nothing_usd\":{}}}"
    ), js(&s.id), rat(s.weight), rat(s.payable_fraction), rat(s.usd_per_kg),
        rat(s.variable_usd_per_dry_t), rat(s.fixed_usd), rat(s.royalty_fraction), rat(s.do_nothing_usd)
    )).collect::<Vec<_>>().join(",");
    format!(
        concat!(
            "{{\"model_key\":{},\"question\":{},\"valuation_basis\":{},\"action_id\":{},\"action_label\":{},",
            "\"boundary\":{},\"circuit\":{},\"material_domain\":{},\"dry_basis\":{},\"element\":{},",
            "\"start_ns\":\"{}\",\"end_ns\":\"{}\",\"commercial_support\":{},",
            "\"physical_to_commercial_basis\":{},\"monetary_rounding\":{},\"scenarios\":[{}]}}"
        ),
        js(&t.model_key),
        js(&t.question),
        js(&t.valuation_basis),
        js(&t.action_id),
        js(&t.action_label),
        js(&t.boundary),
        js(&t.circuit),
        js(&t.material_domain),
        js(&t.dry_basis),
        js(&t.element),
        t.start_ns,
        t.end_ns,
        js(&t.commercial_support),
        js(&t.physical_to_commercial_basis),
        js(t.monetary_rounding.as_str()),
        rows
    )
}

fn point(p: &ValuationPoint) -> String {
    format!(
        concat!(
            "{{\"scenario_id\":{},\"weight\":{},\"represented_dry_tonnes\":{},\"represented_recovered_kg\":{},",
            "\"payable_kg\":{},\"gross_revenue_usd\":{},\"royalty_usd\":{},\"variable_cost_usd\":{},",
            "\"fixed_cost_usd\":{},\"unrounded_margin_usd\":{},\"rounded_margin_usd\":{},",
            "\"margin_minor\":\"{}\",\"margin_rounding_delta_usd\":{},\"unrounded_do_nothing_usd\":{},",
            "\"rounded_do_nothing_usd\":{},\"do_nothing_minor\":\"{}\",\"do_nothing_rounding_delta_usd\":{}}}"
        ),
        js(&p.scenario_id),
        rat(p.weight),
        rat(p.represented_dry_tonnes),
        rat(p.represented_recovered_kg),
        rat(p.payable_kg),
        rat(p.gross_revenue_usd),
        rat(p.royalty_usd),
        rat(p.variable_cost_usd),
        rat(p.fixed_cost_usd),
        rat(p.unrounded_margin_usd),
        js(&monetary(p.margin)),
        p.margin.minor,
        rat(p.margin_rounding_delta_usd),
        rat(p.unrounded_do_nothing_usd),
        js(&monetary(p.do_nothing)),
        p.do_nothing.minor,
        rat(p.do_nothing_rounding_delta_usd)
    )
}

impl ValuationReceipt {
    pub fn to_json(&self) -> String {
        let points = self.points.iter().map(point).collect::<Vec<_>>().join(",");
        let production = format!(
            "{{\"digest\":{},\"claim\":{},\"input_claim\":{}}}",
            js(self.production.digest()),
            claim(self.production.claim()),
            claim(self.production.inputs())
        );
        format!(
            concat!(
                "{{\"schema\":\"fmine.production-valuation.receipt.v1\",\"gate_eligible\":false,",
                "\"calibration\":\"uncalibrated\",\"physical_arithmetic\":\"exact represented binary64 quantities\",",
                "\"monetary_scale\":10000,\"rounding_delta_definition\":\"rounded minus unrounded USD\",",
                "\"terms\":{},\"terms_claim\":{},\"submitted_worksheet\":{},\"production\":{},",
                "\"points\":[{}],\"decision\":{},\"digest\":{},\"no_claim\":{}}}"
            ),
            terms(&self.terms.value),
            claim(&self.terms),
            self.submitted_worksheet
                .as_deref()
                .map_or_else(|| "null".into(), js),
            production,
            points,
            self.decision.to_json(),
            js(&self.digest),
            js(NO_CLAIM)
        )
    }

    pub fn to_markdown(&self) -> String {
        let t = &self.terms.value;
        let s = self.decision.summary();
        let mut out = format!(
            "# Production cash-margin comparison\n\n{}\n\n{NO_CLAIM}\n\n**Model:** {}. **Basis:** {}. **Physical interval:** [{}, {}). **Element:** {}.\n\n**Commercial support:** {}. **Declared bridge:** {}.\n\n**Monetary rule:** {}. **Evidence:** {:?}. **Confidentiality:** {}.\n\n",
            md(&t.question),
            md(&t.model_key),
            md(&t.valuation_basis),
            t.start_ns,
            t.end_ns,
            md(&t.element),
            md(&t.commercial_support),
            md(&t.physical_to_commercial_basis),
            t.monetary_rounding.as_str(),
            self.decision.claim().evidence,
            md(&self.decision.claim().confidentiality.to_string()),
        );
        out.push_str("## Paired physical and cash outcomes\n\nAll component amounts are exact rational USD over the represented physical values. Rounding deltas are rounded minus unrounded; only final margins are quantized.\n\n| State | Weight | Dry tonnes | Recovered kg | Payable kg | Revenue USD | Royalty USD | Variable USD | Fixed USD | Margin USD | Rounding delta USD | Do-nothing USD | Do-nothing delta USD |\n|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|\n");
        for p in &self.points {
            let _ = writeln!(
                out,
                "| {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} |",
                md(&p.scenario_id),
                shown(p.weight),
                shown(p.represented_dry_tonnes),
                shown(p.represented_recovered_kg),
                shown(p.payable_kg),
                shown(p.gross_revenue_usd),
                shown(p.royalty_usd),
                shown(p.variable_cost_usd),
                shown(p.fixed_cost_usd),
                monetary(p.margin),
                shown(p.margin_rounding_delta_usd),
                monetary(p.do_nothing),
                shown(p.do_nothing_rounding_delta_usd)
            );
        }
        out.push_str("\n## Decision under the declared population\n\nThe existing decision evaluator uses the rounded scenario payoffs. Quantiles use the inverse CDF. CVaR95 integrates the largest 5% of signed loss = minus immediate cash margin. Its value may be negative when all states gain.\n\n| Action | Expected USD | P10 / P50 / P90 USD | CVaR95 loss USD | Mean regret USD | Maximum regret USD |\n|---|---:|---:|---:|---:|---:|\n");
        for a in &s.alternatives {
            let _ = writeln!(
                out,
                "| {} ({}) | {} | {} / {} / {} | {} | {} | {} |",
                md(&a.label),
                md(&a.action_id),
                shown(a.mean.major),
                shown(a.p10_p50_p90[0].major),
                shown(a.p10_p50_p90[1].major),
                shown(a.p10_p50_p90[2].major),
                shown(a.cvar95_loss.major),
                shown(a.mean_regret.major),
                shown(a.max_regret.major)
            );
        }
        let _ = write!(
            out,
            "\n**Expected-value co-winners:** {}. **Minimax-regret co-winners:** {}.\n\n{}\n\n## Source ancestry\n\nProduction Claim: `{}`. Terms Claim: `{}`. Decision Claim: `{}`.\n\n",
            md(&s.expected_value_winners.join(", ")),
            md(&s.minimax_regret_winners.join(", ")),
            card::TIE_RULE,
            self.production.claim().claim_id,
            self.terms.claim_id,
            self.decision.claim().claim_id
        );
        let mut sources = BTreeSet::new();
        sources.extend(self.production.claim().anchors.iter());
        sources.extend(self.terms.anchors.iter());
        for source in sources {
            let _ = writeln!(
                out,
                "- {} — {} — SHA-256 `{}`",
                md(&source.artifact_path),
                md(&source.locator),
                fmine_core::Digest32(source.artifact_sha256).to_hex()
            );
        }
        let _ = writeln!(out, "\nReceipt digest: `{}`.", self.digest);
        out
    }
}
