//! Exact, decision-support-only recovery bounds with all admitted source metadata retained.
//! Determinism: Exact

use std::fmt::Write as _;

use crate::card::render::{claim_json, js, rq};

use super::{
    BoundCard, BoundSummary, Interval, NO_CLAIM, RecoveryBoundModel, TestCondition, WeightBasis,
};

fn interval(value: Interval) -> String {
    format!(
        "{{\"lower\":{},\"upper\":{}}}",
        rq(value.lower),
        rq(value.upper)
    )
}

fn weights(value: &WeightBasis) -> String {
    match value {
        WeightBasis::ContainedMetal { untested_fraction } => format!(
            "{{\"basis\":\"contained_metal\",\"untested_fraction\":{}}}",
            interval(*untested_fraction)
        ),
        WeightBasis::DryTonnesAndGrade {
            tested_tonnes,
            tested_grade_g_per_t,
            untested_tonnes,
            untested_grade_g_per_t,
        } => format!(
            "{{\"basis\":\"dry_tonnes_and_grade\",\"tested_dry_metric_tonnes\":{},\"tested_grade_g_per_t\":{},\"untested_dry_metric_tonnes\":{},\"untested_grade_g_per_t\":{}}}",
            interval(*tested_tonnes),
            interval(*tested_grade_g_per_t),
            interval(*untested_tonnes),
            interval(*untested_grade_g_per_t)
        ),
    }
}

pub(super) fn model_json(value: &RecoveryBoundModel) -> String {
    let interactions = value
        .interactions
        .iter()
        .map(|v| js(v))
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{{\"model_key\":{},\"question\":{},\"route\":{},\"metal\":{},\"additivity_basis\":{},\"untested_bound_basis\":{},\"weights\":{},\"tested_recovery\":{},\"untested_recovery\":{},\"threshold\":{},\"rule\":\"proceed_if_at_least\",\"interactions\":[{interactions}]}}",
        js(&value.model_key),
        js(&value.question),
        js(&value.route),
        js(&value.metal),
        js(&value.additivity_basis),
        js(&value.untested_bound_basis),
        weights(&value.weights),
        interval(value.tested_recovery),
        interval(value.untested_recovery),
        rq(value.threshold)
    )
}

fn condition(value: &TestCondition) -> String {
    match value {
        TestCondition::AlreadyStable => "{\"kind\":\"already_stable\"}".into(),
        TestCondition::RecoveryLowerAtLeast(bound) => format!(
            "{{\"kind\":\"untested_lower_at_least\",\"comparison\":\">=\",\"value\":{}}}",
            rq(*bound)
        ),
        TestCondition::RecoveryUpperBelow(bound) => format!(
            "{{\"kind\":\"untested_upper_below\",\"comparison\":\"<\",\"value\":{}}}",
            rq(*bound)
        ),
        TestCondition::Impossible { reason } => {
            format!("{{\"kind\":\"impossible\",\"reason\":{}}}", js(reason))
        }
    }
}

pub(super) fn summary_json(value: &BoundSummary) -> String {
    let vertices = value.vertices.iter().map(|v| format!(
        "{{\"untested_metal_fraction\":{},\"tested_recovery\":{},\"untested_recovery\":{},\"aggregate_recovery\":{}}}",
        rq(v.untested_metal_fraction), rq(v.tested_recovery), rq(v.untested_recovery), rq(v.aggregate_recovery)
    )).collect::<Vec<_>>().join(",");
    format!(
        "{{\"untested_metal_fraction\":{},\"recovery\":{},\"threshold\":{},\"stability\":{},\"proceed_condition\":{},\"stop_condition\":{},\"vertices\":[{vertices}]}}",
        interval(value.untested_metal_fraction),
        interval(value.recovery),
        rq(value.threshold),
        js(value.stability.as_str()),
        condition(&value.proceed_condition),
        condition(&value.stop_condition)
    )
}

fn md(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('|', "\\|")
        .replace('*', "\\*")
        .replace('_', "\\_")
        .replace('[', "\\[")
        .replace(']', "\\]")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('`', "\\`")
}

fn ratio(value: super::Rat) -> String {
    format!("{}/{}", value.num(), value.den())
}

fn interval_text(value: Interval) -> String {
    format!("[{}, {}]", ratio(value.lower), ratio(value.upper))
}

fn condition_text(value: &TestCondition) -> String {
    match value {
        TestCondition::AlreadyStable => "Already stable under the admitted bounds.".into(),
        TestCondition::RecoveryLowerAtLeast(bound) => format!(
            "Untested recovery lower bound must be at least {} (inclusive).",
            ratio(*bound)
        ),
        TestCondition::RecoveryUpperBelow(bound) => format!(
            "Untested recovery upper bound must be below {} (strict; equality does not establish stopping).",
            ratio(*bound)
        ),
        TestCondition::Impossible { reason } => format!(
            "No admissible tightening can establish this choice: {}.",
            md(reason)
        ),
    }
}

impl BoundCard {
    /// Deterministic exact report; the digest binds full input metadata and the checked summary.
    #[must_use]
    pub fn to_json(&self) -> String {
        format!(
            "{{\"schema\":\"fmine.recovery-bounds-card.v1\",\"digest\":{},\"calibration\":\"not_established\",\"disclosure_eligible\":false,\"no_claim\":{},\"inputs\":{},\"result\":{}}}",
            js(self.digest()),
            js(NO_CLAIM),
            claim_json(self.inputs(), &model_json(&self.inputs().value)),
            claim_json(self.claim(), &summary_json(self.summary()))
        )
    }

    /// A private decision-support card. Recovery fractions are exact and are not probability intervals.
    #[must_use]
    pub fn to_markdown(&self) -> String {
        let model = &self.inputs().value;
        let summary = self.summary();
        let mut out = format!(
            "# Untested-ore recovery bounds\n\n{}\n\nModel: {}. Route: {}. Metal: {}.\n\n**Decision: {}.** Proceed when aggregate recovery is at least {}. All fractions below are exact.\n\n| Quantity | Closed bounds |\n|---|---|\n| Untested share of contained metal | {} |\n| Tested recovery | {} |\n| Untested recovery | {} |\n| Aggregate recovery | {} |\n\n",
            md(&model.question),
            md(&model.model_key),
            md(&model.route),
            md(&model.metal),
            summary.stability.as_str(),
            ratio(summary.threshold),
            interval_text(summary.untested_metal_fraction),
            interval_text(model.tested_recovery),
            interval_text(model.untested_recovery),
            interval_text(summary.recovery)
        );
        if let WeightBasis::DryTonnesAndGrade {
            tested_tonnes,
            tested_grade_g_per_t,
            untested_tonnes,
            untested_grade_g_per_t,
        } = model.weights
        {
            let _ = writeln!(
                out,
                "Metal weights use dry tonnes multiplied by grade, never tonne shares.\n\n| Stratum | Dry metric tonnes | Grade, g/t |\n|---|---|---|\n| Tested | {} | {} |\n| Untested | {} | {} |\n",
                interval_text(tested_tonnes),
                interval_text(tested_grade_g_per_t),
                interval_text(untested_tonnes),
                interval_text(untested_grade_g_per_t)
            );
        }
        let _ = writeln!(
            out,
            "## What would resolve the decision\n\n- Stable proceed: {}\n- Stable stop: {}\n\nThese conditions tighten the admitted untested recovery bounds while retaining the other assumptions. They assign no probability or monetary value to a test.\n\n## Declared validity\n\nAdditivity: {}\n\nUntested-bound basis: {}\n\nThe bound is conditional on these declarations. A declared blending interaction refuses aggregation.\n\n## Endpoint checks\n\n| Untested metal share | Tested recovery | Untested recovery | Aggregate recovery |\n|---|---|---|---|",
            condition_text(&summary.proceed_condition),
            condition_text(&summary.stop_condition),
            md(&model.additivity_basis),
            md(&model.untested_bound_basis)
        );
        for v in &summary.vertices {
            let _ = writeln!(
                out,
                "| {} | {} | {} | {} |",
                ratio(v.untested_metal_fraction),
                ratio(v.tested_recovery),
                ratio(v.untested_recovery),
                ratio(v.aggregate_recovery)
            );
        }
        let _ = writeln!(
            out,
            "\n## Evidence and residual uncertainty\n\nInput Claim: `{}`. Result Claim: `{}`. Receipt: `{}`.\n\nInput evidence: {:?}; output evidence: {:?}. Confidentiality: {}. Calibration is not established.\n",
            self.inputs().claim_id,
            self.claim().claim_id,
            self.digest(),
            self.inputs().evidence,
            self.claim().evidence,
            md(&self.claim().confidentiality.to_string())
        );
        for u in &self.claim().uu_terms {
            let _ = writeln!(
                out,
                "- {}: {}. Trigger: {}. Declared downside: {} USD million.",
                md(&u.scenario_id),
                md(&u.description),
                md(&u.trigger_indicator),
                u.downside_exposure_usd_m
            );
        }
        let _ = writeln!(
            out,
            "\n| Source | SHA-256 | Locator / original token |\n|---|---|---|"
        );
        for a in &self.inputs().anchors {
            let hash = a
                .artifact_sha256
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect::<String>();
            let _ = writeln!(
                out,
                "| {} | {} | {} |",
                md(&a.artifact_path),
                hash,
                md(&a.locator)
            );
        }
        let _ = writeln!(out, "\n{NO_CLAIM}");
        out
    }
}
