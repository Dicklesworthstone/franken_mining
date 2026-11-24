//! DESIGN (bd-9bsy): a synthetic approval clock and early-works stage decision.
//! Determinism: Exact

use super::{
    Card, Result, TomlDoc, date, dollars, flag, fraction, get, headroom, invalid, js, qty,
    row_fields, table, tables,
};
use crate::capital::investment::number;
use fmine_capital::permit::{
    DeemedRule, Disposition, GateState, Pause, Record, Review, Terms, review,
};

struct PermitGate<'a> {
    application: &'a str,
    terms: Terms,
    record: Record,
    review: Review,
}

fn gate<'a>(doc: &'a TomlDoc, as_of: &str, project: &str) -> Result<PermitGate<'a>> {
    let permit = row_fields(
        table(doc, "permit")?,
        &[
            "project",
            "authority",
            "application",
            "filed_on",
            "clock_start",
            "clock_days",
            "deemed_rule",
            "reopen_at_pct",
            "change_basis",
        ],
        as_of,
    )?;
    let status = row_fields(
        table(doc, "permit_status")?,
        &[
            "application",
            "status",
            "effective_on",
            "scope_change_pct",
            "change_basis",
        ],
        as_of,
    )?;
    let application = get(&permit, "application")?;
    if get(&permit, "project")? != project || get(&status, "application")? != application {
        return Err(invalid(
            "permit, evidence and schedule require matching project and application",
        ));
    }
    let terms = Terms {
        authority: get(&permit, "authority")?.into(),
        filed_on: get(&permit, "filed_on")?.into(),
        clock_start: get(&permit, "clock_start")?.into(),
        clock_days: u64::try_from(qty(&permit, "clock_days")?)
            .map_err(|_| invalid("clock_days requires a nonnegative whole number"))?,
        deemed_rule: DeemedRule::parse(get(&permit, "deemed_rule")?)?,
        reopen_at_pct: number(get(&permit, "reopen_at_pct")?)?,
        change_basis: get(&permit, "change_basis")?.into(),
        source: get(&permit, "source")?.into(),
        known_on: get(&permit, "known_on")?.into(),
    };
    let record = Record {
        disposition: Disposition::parse(get(&status, "status")?)?,
        effective_on: get(&status, "effective_on")?.into(),
        scope_change_pct: number(get(&status, "scope_change_pct")?)?,
        change_basis: get(&status, "change_basis")?.into(),
        source: get(&status, "source")?.into(),
        known_on: get(&status, "known_on")?.into(),
    };
    let mut pauses = Vec::new();
    for row in doc.rows_of("pause") {
        let f = row_fields(row, &["application", "from", "resumed_on"], as_of)?;
        if get(&f, "application")? != application {
            return Err(invalid("pauses must refer to this application"));
        }
        let resumed = get(&f, "resumed_on")?;
        pauses.push(Pause {
            from: get(&f, "from")?.into(),
            resumed_on: (resumed != "open").then(|| resumed.into()),
            source: get(&f, "source")?.into(),
            known_on: get(&f, "known_on")?.into(),
        });
    }
    let review = review(&terms, &record, &pauses, as_of)?;
    Ok(PermitGate {
        application,
        terms,
        record,
        review,
    })
}

pub(super) fn evaluate(doc: &TomlDoc, as_of: &str) -> Result<Card> {
    tables(
        doc,
        &[
            "permit",
            "permit_status",
            "pause",
            "early_works",
            "schedule",
            "cash",
        ],
    )?;
    let works = row_fields(
        table(doc, "early_works")?,
        &[
            "project",
            "entity",
            "instrument",
            "separate_permission",
            "current_scope_eligible",
            "available_through",
            "spend_usd",
            "credit_on_proceed_usd",
            "recovery_if_no_proceed_usd",
        ],
        as_of,
    )?;
    let schedule = row_fields(
        table(doc, "schedule")?,
        &[
            "project",
            "approval_needed_by",
            "critical_days_saved",
            "critical_day_value_usd",
        ],
        as_of,
    )?;
    let cash = row_fields(
        table(doc, "cash")?,
        &["entity", "cash_usd", "floor_usd"],
        as_of,
    )?;
    let project = get(&works, "project")?;
    if get(&schedule, "project")? != project || get(&works, "entity")? != get(&cash, "entity")? {
        return Err(invalid(
            "schedule and cash require the early-work project and paying entity",
        ));
    }
    let gate = gate(doc, as_of, project)?;
    let needed = date(&schedule, "approval_needed_by")?;
    let valid_through = date(&works, "available_through")?;
    let instrument = get(&works, "instrument")?;
    let separate = flag(&works, "separate_permission")?;
    let scope_eligible = flag(&works, "current_scope_eligible")?;
    let has_early_permission = separate && scope_eligible && as_of <= valid_through;
    let spend = qty(&works, "spend_usd")?;
    let credit = qty(&works, "credit_on_proceed_usd")?;
    let recovery = qty(&works, "recovery_if_no_proceed_usd")?;
    if credit > spend || recovery > spend {
        return Err(invalid(
            "the credit and recoverable value each must fit the early spend",
        ));
    }
    let balance = qty(&cash, "cash_usd")?;
    let floor = qty(&cash, "floor_usd")?;
    let slack = headroom(balance, spend, floor)?;
    let max_spend = headroom(balance, 0, floor)?;
    let schedule_value =
        qty(&schedule, "critical_days_saved")? * qty(&schedule, "critical_day_value_usd")?;
    let gain = schedule_value - (spend - credit);
    let loss = spend - recovery;
    let main_admitted = gate.review.state == GateState::PermissionDocumented;
    let blocker = if main_admitted {
        Some("Main permission is already documented. The approval-dependent comparison has resolved.".into())
    } else if gate.review.state == GateState::Refused {
        Some("The documented refusal rules out this early-work comparison.".into())
    } else if as_of >= needed {
        Some(format!(
            "The required approval date {needed} has arrived or passed."
        ))
    } else if !has_early_permission {
        Some(if as_of > valid_through {
            format!("The early-work terms expired on {valid_through}.")
        } else {
            "The worksheet lacks separate early-work permission for the current scope.".into()
        })
    } else if slack < 0 {
        Some(format!(
            "The proposed payment would leave cash {} below the floor.",
            dollars(-slack)
        ))
    } else {
        None
    };
    let eligible = blocker.is_none();
    let threshold = if eligible && gain > 0 {
        Some(fraction(loss, gain + loss)?)
    } else {
        None
    };

    let mut card = Card::new(&["Early works compared with waiting", "Value", "Basis"]);
    let clock_deadline = gate.review.clock_deadline.as_deref();
    let clock_summary = clock_deadline.map_or(
        "Clock paused with no recorded restart date".to_owned(),
        |d| format!("Declared clock deadline {d}"),
    );
    card.findings.push(format!(
        "{clock_summary}. Application {} to {} was filed {}. The clock started {} and has {} paused days as of this worksheet.",
        gate.application, gate.terms.authority, gate.terms.filed_on, gate.terms.clock_start, gate.review.paused_days,
    ));
    card.findings.push(match gate.review.state {
        GateState::PermissionDocumented => "The worksheet records main permission for the declared scope.".into(),
        GateState::Refused => "The main application is refused under the stated record.".into(),
        GateState::RequiresReopenReview => "The declared scope change meets the threshold for reopening. Main works remain blocked pending review.".into(),
        GateState::Elapsed if gate.review.deeming_review_due => "The clock has elapsed. Deeming requires a separate confirmation, so main works remain on hold.".into(),
        GateState::Elapsed => "The clock has elapsed. The main decision is still pending.".into(),
        _ => "The main decision is still pending.".into(),
    });
    card.findings.push(format!(
        "Separate early-work terms {instrument} are {} for this scope as of the decision date.",
        if has_early_permission {
            "documented"
        } else {
            "unavailable"
        },
    ));
    if let Some(value) = &threshold {
        // Loss is at most the quoted i64 spend, so centi-percent scaling fits in i128.
        let total = gain + loss;
        let pct = (loss * 10_000 + total / 2) / total;
        card.findings.push(format!(
            "Break-even probability of proceeding with permission by {needed} is {value} ({}.{:02}%). Approval odds are undeclared.",
            pct / 100, pct % 100,
        ));
    } else {
        card.findings.push(blocker.unwrap_or_else(|| {
            "The supplied outcomes give early works no positive advantage over waiting.".into()
        }));
    }
    card.rows = vec![
        vec![
            "Upfront early-work quote".into(),
            dollars(spend),
            format!("Terms available through {valid_through}"),
        ],
        vec![
            "Cash above floor after payment".into(),
            dollars(slack),
            format!("{} unrestricted cash", get(&cash, "entity")?),
        ],
        vec![
            "Incremental value if proceeding with permission in time".into(),
            dollars(gain),
            "Critical-path value less unreused early spend".into(),
        ],
        vec![
            "Incremental value if the project cannot proceed in time".into(),
            dollars(-loss),
            "Recoverable value less early spend".into(),
        ],
    ];
    let string_or_null = |value: Option<&str>| value.map_or_else(|| "null".into(), js);
    let change = fraction(
        gate.record.scope_change_pct.num(),
        gate.record.scope_change_pct.den(),
    )?;
    let reopen_at = fraction(
        gate.terms.reopen_at_pct.num(),
        gate.terms.reopen_at_pct.den(),
    )?;
    let max_spend = if max_spend >= 0 {
        js(&max_spend.to_string())
    } else {
        "null".into()
    };
    card.detail = Some((
        "permit_gate",
        format!(
            r#"{{"project":{},"application":{},"authority":{},"filed_on":{},"clock_start":{},"clock_days":"{}","clock_deadline":{},"paused_days":"{}","recorded_status":{},"status_effective_on":{},"status":{},"main_terms_admitted":{},"deemed_rule":{},"deeming_review_due":{},"scope_change_pct":{},"reopen_at_pct":{},"change_basis":{},"scope_change_requires_review":{},"early_works":{{"instrument":{},"available_through":{},"documented_for_scope":{has_early_permission},"comparison_eligible":{eligible},"approval_needed_by":{},"spend_usd":"{spend}","maximum_spend_preserving_floor_usd":{max_spend},"cash_slack_usd":"{slack}","schedule_value_usd":"{schedule_value}","gain_if_proceed_usd":"{gain}","loss_if_no_proceed_usd":"{loss}","break_even_proceed_probability":{}}}}}"#,
            js(project),
            js(gate.application),
            js(&gate.terms.authority),
            js(&gate.terms.filed_on),
            js(&gate.terms.clock_start),
            gate.terms.clock_days,
            string_or_null(clock_deadline),
            gate.review.paused_days,
            js(gate.record.disposition.label()),
            js(&gate.record.effective_on),
            js(gate.review.state.label()),
            main_admitted,
            js(gate.terms.deemed_rule.label()),
            gate.review.deeming_review_due,
            js(&change),
            js(&reopen_at),
            js(&gate.terms.change_basis),
            gate.review.scope_reopens,
            js(instrument),
            js(valid_through),
            js(needed),
            string_or_null(threshold.as_deref()),
        ),
    ));
    Ok(card)
}

#[cfg(test)]
mod tests {
    use super::*;
    use fmine_core::anchor::SourceAnchor;
    use fmine_core::hash::sha256;
    use fmine_io::omf::json::{Json, parse};
    use fmine_registry::toml_subset::parse_subset;
    use std::path::Path;

    const CASE: &str = include_str!("../../../../../tests/capital/executive_permit.toml");

    fn run(text: &str) -> Result<Json> {
        let doc = parse_subset(text).expect("synthetic TOML");
        let source = SourceAnchor::new("permit.toml", sha256(text.as_bytes()).0, "worksheet");
        let report = super::super::worksheet(&doc, text, &source, Path::new("."), false)?;
        let payload = parse(report.payload_json.as_bytes()).expect("valid JSON result");
        Ok(payload.get("permit_gate").expect("permit result").clone())
    }

    #[test]
    fn elapsed_is_pending_and_only_timely_confirmation_admits_unchanged_main_scope() {
        let elapsed = run(CASE).expect("elapsed");
        assert_eq!(
            elapsed.get("clock_deadline").and_then(Json::as_str),
            Some("2030-04-01")
        );
        assert_eq!(
            elapsed.get("paused_days").and_then(Json::as_str),
            Some("10")
        );
        assert_eq!(elapsed.get("deeming_review_due"), Some(&Json::Bool(true)));
        assert_eq!(elapsed.get("main_terms_admitted"), Some(&Json::Bool(false)));

        let confirmed = CASE.replace("status = \"pending\"", "status = \"deeming_confirmed\"");
        let on_deadline = run(&confirmed).expect("confirmation on day of expiry");
        assert_eq!(
            on_deadline.get("main_terms_admitted"),
            Some(&Json::Bool(true))
        );
        for (input, expected) in [
            (
                confirmed.replace(
                    "effective_on = \"2030-04-01\"",
                    "effective_on = \"2030-03-31\"",
                ),
                "at or after",
            ),
            (
                confirmed.replace("confirmation_required", "none"),
                "declared rule",
            ),
            (
                confirmed.replace("resumed_on = \"2030-03-02\"", "resumed_on = \"open\""),
                "running clock",
            ),
        ] {
            assert!(
                run(&input)
                    .expect_err("no admissible confirmation")
                    .what
                    .contains(expected)
            );
        }
        let written = CASE.replace("status = \"pending\"", "status = \"written_grant\"");
        for (change, reopens) in [("9.9999", false), ("10", true), ("10.0001", true)] {
            let revised = run(&written.replace(
                "scope_change_pct = \"7\"",
                &format!("scope_change_pct = \"{change}\""),
            ))
            .expect("scope review");
            assert_eq!(
                revised.get("scope_change_requires_review"),
                Some(&Json::Bool(reopens))
            );
            assert_eq!(
                revised.get("main_terms_admitted"),
                Some(&Json::Bool(!reopens))
            );
        }
    }

    #[test]
    fn pause_chronology_refuses_overlap_hindsight_and_time_after_disposition() {
        let paused = CASE.replace("resumed_on = \"2030-03-02\"", "resumed_on = \"open\"");
        let result = run(&paused).expect("ongoing pause");
        assert_eq!(result.get("clock_deadline"), Some(&Json::Null));
        assert_eq!(result.get("paused_days").and_then(Json::as_str), Some("41"));
        let late = CASE
            .replace("from = \"2030-02-20\"", "from = \"2030-03-22\"")
            .replace("resumed_on = \"2030-03-02\"", "resumed_on = \"2030-03-25\"")
            .replace("known_on = \"2030-03-02\"", "known_on = \"2030-03-25\"");
        let overlap = format!(
            "{CASE}\n[[pause]]\napplication = \"NRC-42\"\nfrom = \"2030-02-21\"\nresumed_on = \"2030-02-22\"\nsource = \"Synthetic overlap\"\nknown_on = \"2030-02-22\"\n"
        );
        for (input, expected) in [
            (late, "unexpired"),
            (overlap, "cannot overlap"),
            (
                CASE.replace("from = \"2030-02-20\"", "from = \"2030-03-03\""),
                "correctly ordered",
            ),
            (
                CASE.replacen("known_on = \"2030-03-02\"", "known_on = \"2030-04-03\"", 1),
                "unavailable",
            ),
            (
                CASE.replace("status = \"pending\"", "status = \"written_refusal\"")
                    .replace(
                        "effective_on = \"2030-04-01\"",
                        "effective_on = \"2030-02-19\"",
                    ),
                "after disposition",
            ),
            (
                CASE.replace(
                    "clock_days = \"40\"",
                    "clock_days = \"9223372036854775807\"",
                ),
                "exceeds the worksheet calendar",
            ),
            (
                CASE.replacen(
                    "evidence = \"synthetic\"",
                    "evidence = \"operator_declared\"",
                    1,
                ),
                "synthetic evidence",
            ),
        ] {
            let refusal = run(&input).expect_err("inconsistent evidence");
            assert!(refusal.what.contains(expected), "{}", refusal.what);
        }
    }

    #[test]
    fn approval_timing_current_scope_and_cash_gate_the_option_threshold() {
        for (input, eligible, threshold) in [
            (
                CASE.replace("cash_usd = \"22000000\"", "cash_usd = \"21000000\""),
                true,
                Some("5/13"),
            ),
            (
                CASE.replace("cash_usd = \"22000000\"", "cash_usd = \"20999999\""),
                false,
                None,
            ),
            (
                CASE.replace(
                    "available_through = \"2030-04-12\"",
                    "available_through = \"2030-04-02\"",
                ),
                true,
                Some("5/13"),
            ),
            (
                CASE.replace(
                    "available_through = \"2030-04-12\"",
                    "available_through = \"2030-04-01\"",
                ),
                false,
                None,
            ),
            (
                CASE.replace(
                    "current_scope_eligible = \"yes\"",
                    "current_scope_eligible = \"no\"",
                ),
                false,
                None,
            ),
            (
                CASE.replace(
                    "approval_needed_by = \"2030-05-10\"",
                    "approval_needed_by = \"2030-04-02\"",
                ),
                false,
                None,
            ),
            (
                CASE.replace(
                    "critical_days_saved = \"30\"",
                    "critical_days_saved = \"3\"",
                ),
                true,
                None,
            ),
            (
                CASE.replace(
                    "recovery_if_no_proceed_usd = \"1000000\"",
                    "recovery_if_no_proceed_usd = \"6000000\"",
                ),
                true,
                Some("0"),
            ),
        ] {
            let gate = run(&input).expect("compare supplied terms");
            let works = gate.get("early_works").expect("separate works");
            assert_eq!(
                works.get("comparison_eligible"),
                Some(&Json::Bool(eligible))
            );
            let result = works
                .get("break_even_proceed_probability")
                .expect("threshold field");
            assert_eq!(
                result,
                &threshold.map_or(Json::Null, |v| Json::String(v.into()))
            );
        }
        let overcredit = CASE.replace(
            "credit_on_proceed_usd = \"5000000\"",
            "credit_on_proceed_usd = \"6000001\"",
        );
        assert!(
            run(&overcredit)
                .expect_err("unfunded credit")
                .what
                .contains("fit the early spend")
        );
    }
}
