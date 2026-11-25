//! Exact, decision-support-only recovery bounds with all admitted source metadata retained.
//! Determinism: Exact

use std::fmt::Write as _;

use super::parcels::{FeedCase, PartialHoldback};
use crate::card::render::{claim_json, js, rq, short_decimal};

use super::{
    BoundCard, BoundSummary, Interval, NO_CLAIM, RecoveryBoundModel, ShareRange, TestCondition,
    WeightBasis,
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
        WeightBasis::Parcels(lots) => {
            let rows = lots
                .iter()
                .map(|(id, lot)| {
                    format!(
                        r#"{{"id":{},"role":{},"dry_tonnes":{},"grade_g_per_t":{}}}"#,
                        js(id),
                        js(lot.testwork.as_str()),
                        interval(lot.dry_tonnes),
                        interval(lot.grade_g_per_t)
                    )
                })
                .collect::<Vec<_>>()
                .join(",");
            format!(r#"{{"basis":"parcels","lots":[{rows}]}}"#)
        }
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

fn feed_fields(case: &FeedCase) -> String {
    let assessment = match &case.recovery {
        Ok((r, s)) => format!(
            r#""recovery":{},"stability":{},"refusal":null"#,
            interval(*r),
            js(s.as_str())
        ),
        Err(e) => format!(
            r#""recovery":null,"stability":null,"refusal":{{"code":{},"what":{},"fix":{}}}"#,
            js(e.code),
            js(&e.what),
            js(e.fix)
        ),
    };
    format!(
        r#""dry_tonnes":{},"contained_grams":{},"recovered_grams":{},"forgone_recovered_grams":{},{assessment}"#,
        interval(case.dry_tonnes),
        interval(case.contained_grams),
        interval(case.recovered_grams),
        interval(case.forgone_recovered_grams)
    )
}

fn case_json(case: &FeedCase) -> String {
    let held = case.held_back.as_deref().map_or_else(|| "null".into(), js);
    format!(r#"{{"held_back":{held},{}}}"#, feed_fields(case))
}

fn partial_holdback_json(value: &PartialHoldback) -> String {
    let lot = js(&value.lot);
    match &value.minimum {
        Ok(minimum) => format!(
            r#"{{"lot":{lot},"status":"minimum","minimum_dry_tonne_fraction":{},"held_dry_tonnes":{},"retained_feed":{{{}}}}}"#,
            rq(minimum.fraction),
            interval(minimum.held_dry_tonnes),
            feed_fields(&minimum.retained_feed)
        ),
        Err(limit) => format!(
            r#"{{"lot":{lot},"status":"impossible","reason":{},"what":{}}}"#,
            js(limit.as_str()),
            js(limit.description())
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
    let partial = value
        .partial_holdback
        .as_ref()
        .map_or_else(String::new, |lot| {
            format!(
                r#","partial_holdback":{{"lot":{},"basis":"proportional_same_grade"}}"#,
                js(lot)
            )
        });
    format!(
        "{{\"model_key\":{},\"question\":{},\"route\":{},\"metal\":{},\"additivity_basis\":{},\"untested_bound_basis\":{},\"weights\":{},\"tested_recovery\":{},\"untested_recovery\":{},\"threshold\":{},\"rule\":\"proceed_if_at_least\",\"interactions\":[{interactions}]{partial}}}",
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
    let cases = if value.parcel_cases.is_empty() {
        String::new()
    } else {
        let rows = value
            .parcel_cases
            .iter()
            .map(case_json)
            .collect::<Vec<_>>()
            .join(",");
        format!(r#","parcel_cases":[{rows}]"#)
    };
    let partial = value
        .partial_holdback
        .as_ref()
        .map_or_else(String::new, |holdback| {
            format!(r#","partial_holdback":{}"#, partial_holdback_json(holdback))
        });
    format!(
        "{{\"untested_metal_fraction\":{},\"recovery\":{},\"threshold\":{},\"stability\":{},\"proceed_condition\":{},\"stop_condition\":{},\"vertices\":[{vertices}]{cases}{partial}}}",
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

fn lot_range(bounds: Interval, format: impl Fn(super::Rat) -> String) -> String {
    let lower = format(bounds.lower);
    if bounds.lower == bounds.upper {
        lower
    } else {
        format!("{lower}–{}", format(bounds.upper))
    }
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

fn windows_json(rule: &str, windows: super::Result<super::ShareWindows>) -> String {
    let range_json = |range: Option<ShareRange>| match range {
        None => "null".into(),
        Some(r) => format!(
            "{{\"lower\":{},\"upper\":{},\"lower_inclusive\":{},\"upper_inclusive\":{}}}",
            rq(r.interval.lower),
            rq(r.interval.upper),
            r.lower_inclusive,
            r.upper_inclusive
        ),
    };
    match windows {
        Ok((proceed, stop)) => format!(
            "{{\"rule\":{},\"status\":\"ok\",\"proceed\":{},\"stop\":{}}}",
            js(rule),
            range_json(proceed),
            range_json(stop)
        ),
        Err(e) => format!(
            "{{\"rule\":{},\"status\":\"refused\",\"code\":{},\"what\":{},\"fix\":{}}}",
            js(rule),
            js(e.code),
            js(&e.what),
            js(e.fix)
        ),
    }
}

impl BoundCard {
    /// Deterministic exact report; the digest binds full input metadata and the checked summary.
    #[must_use]
    pub fn to_json(&self) -> String {
        let shares = windows_json("additive-contained-metal-share/v1", self.share_ranges());
        let dry = self.dry_share_ranges().transpose().map_or_else(
            || "null".into(),
            |windows| windows_json("additive-dry-tonne-share/v1", windows),
        );
        format!(
            "{{\"schema\":\"fmine.recovery-bounds-card.v1\",\"digest\":{},\"calibration\":\"not_established\",\"disclosure_eligible\":false,\"no_claim\":{},\"inputs\":{},\"result\":{},\"guaranteed_share_ranges\":{shares},\"guaranteed_dry_tonne_share_ranges\":{dry}}}",
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
        out.push_str("## Untested share of contained metal\n\nWithin the declared share range, the following choices hold for every recovery in the stated bands. Shares outside both windows need more evidence.\n\n");
        let write_windows =
            |out: &mut String, windows: super::Result<super::ShareWindows>| match windows {
                Ok((proceed, stop)) => {
                    for (action, window) in [("Proceed", proceed), ("Stop", stop)] {
                        let description = window.map_or_else(
                            || "no admitted share".to_owned(),
                            |r| {
                                format!(
                                    "{}{}, {}{}",
                                    if r.lower_inclusive { "[" } else { "(" },
                                    ratio(r.interval.lower),
                                    ratio(r.interval.upper),
                                    if r.upper_inclusive { "]" } else { ")" }
                                )
                            },
                        );
                        let _ = writeln!(out, "- {action} — {description}");
                    }
                }
                Err(e) => {
                    let _ = writeln!(
                        out,
                        "Share calculation refused ({}). {} {}",
                        e.code,
                        md(&e.what),
                        md(e.fix)
                    );
                }
            };
        write_windows(&mut out, self.share_ranges());
        if !summary.parcel_cases.is_empty() {
            out.push_str("\n## Whole-lot holdbacks\n\nEach row uses the remaining feed without replacement. Forgone metal is the recovery lost this period with the same feed and recovery assumptions. Recovery percentages are rounded for display. Verdicts use the exact bounds in the JSON card. Record the selected row with `decide journal record --action all-feed` or `--action hold-back:<lot-id>`.\n\n| Feed scenario | Dry tonnes | Contained metal (g) | Recovered metal (g) | Forgone metal (g) | Recovery (%) | Target verdict |\n|---|---:|---:|---:|---:|---:|---|\n");
            for case in &summary.parcel_cases {
                let label = case
                    .held_back
                    .as_ref()
                    .map_or_else(|| "All feed".into(), |id| format!("Hold back {}", md(id)));
                let (recovery, status) = match &case.recovery {
                    Ok((r, s)) => (
                        lot_range(*r, |v| format!("{:.2}", v.to_f64() * 100.0)),
                        s.as_str().replace('_', " "),
                    ),
                    Err(e) => ("—".into(), format!("{} · {}", e.code, md(&e.what))),
                };
                let _ = writeln!(
                    out,
                    "| {label} | {} | {} | {} | {} | {recovery} | {status} |",
                    lot_range(case.dry_tonnes, short_decimal),
                    lot_range(case.contained_grams, short_decimal),
                    lot_range(case.recovered_grams, short_decimal),
                    lot_range(case.forgone_recovered_grams, short_decimal)
                );
            }
        }
        if let Some(holdback) = &summary.partial_holdback {
            let lot = md(&holdback.lot);
            out.push_str("\n## Minimum proportional holdback\n\n");
            out.push_str(
                "The split scales dry mass and contained metal at each admitted grade.\n\n",
            );
            match &holdback.minimum {
                Ok(minimum) => {
                    let feed = &minimum.retained_feed;
                    let action = if minimum.fraction == super::ZERO {
                        "All feed already meets the target for every admitted input.".into()
                    } else {
                        format!(
                            "Hold back **{} of the dry mass of {lot}** at the stated grade. This holds back {} dry tonnes and meets the target across all admitted inputs.",
                            ratio(minimum.fraction),
                            lot_range(minimum.held_dry_tonnes, short_decimal)
                        )
                    };
                    let _ = writeln!(
                        out,
                        "{action} Retained dry tonnes {}. Recovered metal {} g. Forgone recovered metal {} g.",
                        lot_range(feed.dry_tonnes, short_decimal),
                        lot_range(feed.recovered_grams, short_decimal),
                        lot_range(feed.forgone_recovered_grams, short_decimal),
                    );
                }
                Err(limit) => {
                    let _ = writeln!(out, "{lot}. {}", limit.description());
                }
            }
            if let Some(action) = holdback.journal_action() {
                let _ = writeln!(
                    out,
                    "\nRecord this split with `decide journal record --action {action}`."
                );
            }
        }
        if let Some(windows) = self.dry_share_ranges().transpose() {
            out.push_str("\n## Untested share of dry feed tonnes\n\nThese windows use the available dry tonnes and hold across both declared grade and recovery ranges.\n\n");
            write_windows(&mut out, windows);
        }
        out.push('\n');
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
