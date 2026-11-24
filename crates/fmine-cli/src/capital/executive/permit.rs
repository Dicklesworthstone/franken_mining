//! DESIGN (bd-9bsy): a synthetic approval clock and early-works stage decision.
//! Determinism: Exact

use super::{
    Card, CivilDate, ExactQty, Result, TomlDoc, date, day, dollars, flag, fraction, get, headroom,
    invalid, js, qty, row_fields, table, tables,
};
use crate::capital::investment::number;

struct Clock {
    deadline: Option<i64>,
    paused_days: i128,
}

fn deadline(value: i128) -> Result<i64> {
    if value > i128::from(day("9999-12-31")?) {
        return Err(invalid("clock deadline exceeds the worksheet calendar"));
    }
    i64::try_from(value).map_err(|_| invalid("clock deadline is unrepresentable"))
}

fn date_label(epoch_day: i64) -> String {
    let d = CivilDate::from_epoch_days(epoch_day);
    format!("{:04}-{:02}-{:02}", d.year, d.month, d.day)
}

fn clock(
    doc: &TomlDoc,
    as_of: &str,
    application: &str,
    start: i64,
    length: i128,
    resolved: Option<i64>,
) -> Result<Clock> {
    if length == 0 {
        return Err(invalid("the declared clock must have at least one day"));
    }
    let through = resolved.unwrap_or(day(as_of)?);
    let mut periods = Vec::new();
    for row in doc.rows_of("pause") {
        let f = row_fields(row, &["application", "from", "resumed_on"], as_of)?;
        let from = day(date(&f, "from")?)?;
        let known = day(date(&f, "known_on")?)?;
        let end = match get(&f, "resumed_on")? {
            "open" => None,
            _ => Some(day(date(&f, "resumed_on")?)?),
        };
        if get(&f, "application")? != application
            || from > known
            || end.is_some_and(|d| d <= from || d > known)
        {
            return Err(invalid(
                "pauses must refer to this application and use observed, correctly ordered dates",
            ));
        }
        if resolved.is_some_and(|d| from >= d || end.is_some_and(|resume| resume > d)) {
            return Err(invalid(
                "the statutory clock cannot pause or restart after disposition",
            ));
        }
        periods.push((from, end));
    }
    periods.sort_unstable();
    let mut boundary = deadline(i128::from(start) + length)?;
    let mut previous_end = start;
    let mut paused_days = 0;
    for (index, (from, resume)) in periods.iter().copied().enumerate() {
        if from < previous_end || from >= boundary {
            return Err(invalid(
                "pauses require a running, unexpired clock and cannot overlap",
            ));
        }
        let Some(resume) = resume else {
            if index + 1 != periods.len() {
                return Err(invalid("an open pause must be the last pause"));
            }
            return Ok(Clock {
                deadline: None,
                paused_days: paused_days + i128::from(through - from),
            });
        };
        let paused = i128::from(resume - from);
        paused_days += paused;
        boundary = deadline(i128::from(boundary) + paused)?;
        previous_end = resume;
    }
    Ok(Clock {
        deadline: Some(boundary),
        paused_days,
    })
}

struct PermitGate<'a> {
    authority: &'a str,
    application: &'a str,
    filed: &'a str,
    start: &'a str,
    clock_days: i128,
    approval_status: &'a str,
    effective: &'a str,
    clock: Clock,
    state: &'static str,
    scope_reopens: bool,
    main_admitted: bool,
    deeming_review_due: bool,
    threshold: ExactQty,
    change: ExactQty,
    change_basis: &'a str,
    deemed_rule: &'a str,
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
    let change_basis = get(&permit, "change_basis")?;
    if get(&permit, "project")? != project
        || get(&status, "application")? != application
        || get(&status, "change_basis")? != change_basis
    {
        return Err(invalid(
            "permit, evidence and schedule require matching project, application and scope-change basis",
        ));
    }
    let filed = date(&permit, "filed_on")?;
    let start = date(&permit, "clock_start")?;
    let effective = date(&status, "effective_on")?;
    if !(filed <= start
        && start <= date(&permit, "known_on")?
        && start <= effective
        && effective <= date(&status, "known_on")?)
    {
        return Err(invalid(
            "require filing before clock start, and both the start and effective status known by their source dates",
        ));
    }
    let approval_status = get(&status, "status")?;
    let effective_day = day(effective)?;
    let resolved = matches!(
        approval_status,
        "written_grant" | "written_refusal" | "deeming_confirmed"
    )
    .then_some(effective_day);
    let clock_days = qty(&permit, "clock_days")?;
    let time = clock(doc, as_of, application, day(start)?, clock_days, resolved)?;
    let deemed_rule = get(&permit, "deemed_rule")?;
    if !matches!(deemed_rule, "none" | "confirmation_required") {
        return Err(invalid("deemed_rule must be none or confirmation_required"));
    }
    if approval_status == "deeming_confirmed"
        && (deemed_rule != "confirmation_required"
            || time.deadline.is_none_or(|d| effective_day < d))
    {
        return Err(invalid(
            "confirmed deeming requires the declared rule and an effective date at or after the running clock's deadline",
        ));
    }
    let (change, threshold) = (
        number(get(&status, "scope_change_pct")?)?,
        number(get(&permit, "reopen_at_pct")?)?,
    );
    if change.signum() < 0 || threshold.signum() <= 0 {
        return Err(invalid(
            "scope change must be nonnegative and its reopening threshold positive",
        ));
    }
    let scope_reopens = !change
        .checked_cmp(threshold)
        .map_err(|e| invalid(e.to_string()))?
        .is_lt();
    let today = day(as_of)?;
    let state = match approval_status {
        "written_refusal" => "refused",
        "pending" | "written_grant" | "deeming_confirmed" if scope_reopens => {
            "requires_reopen_review"
        }
        "written_grant" | "deeming_confirmed" => "permission_documented",
        "pending" if time.deadline.is_none() => "clock_paused",
        "pending" if time.deadline.is_some_and(|d| today >= d) => "clock_elapsed",
        "pending" => "clock_running",
        _ => {
            return Err(invalid(
                "permit status requires pending, written_grant, written_refusal or deeming_confirmed",
            ));
        }
    };
    Ok(PermitGate {
        authority: get(&permit, "authority")?,
        application,
        filed,
        start,
        clock_days,
        approval_status,
        effective,
        clock: time,
        state,
        scope_reopens,
        main_admitted: state == "permission_documented",
        deeming_review_due: state == "clock_elapsed" && deemed_rule == "confirmation_required",
        threshold,
        change,
        change_basis,
        deemed_rule,
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
    let blocker = if gate.main_admitted {
        Some("Main permission is already documented. The approval-dependent comparison has resolved.".into())
    } else if gate.state == "refused" {
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
    let clock_deadline = gate.clock.deadline.map(date_label);
    let clock_summary = clock_deadline.as_deref().map_or(
        "Clock paused with no recorded restart date".to_owned(),
        |d| format!("Declared clock deadline {d}"),
    );
    card.findings.push(format!(
        "{clock_summary}. Application {} to {} was filed {}. The clock started {} and has {} paused days as of this worksheet.",
        gate.application, gate.authority, gate.filed, gate.start, gate.clock.paused_days,
    ));
    card.findings.push(match gate.state {
        "permission_documented" => "The worksheet records main permission for the declared scope.".into(),
        "refused" => "The main application is refused under the stated record.".into(),
        "requires_reopen_review" => "The declared scope change meets the threshold for reopening. Main works remain blocked pending review.".into(),
        "clock_elapsed" if gate.deeming_review_due => "The clock has elapsed. Deeming requires a separate confirmation, so main works remain on hold.".into(),
        "clock_elapsed" => "The clock has elapsed. The main decision is still pending.".into(),
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
    let change = fraction(gate.change.num(), gate.change.den())?;
    let reopen_at = fraction(gate.threshold.num(), gate.threshold.den())?;
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
            js(gate.authority),
            js(gate.filed),
            js(gate.start),
            gate.clock_days,
            string_or_null(clock_deadline.as_deref()),
            gate.clock.paused_days,
            js(gate.approval_status),
            js(gate.effective),
            js(gate.state),
            gate.main_admitted,
            js(gate.deemed_rule),
            gate.deeming_review_due,
            js(&change),
            js(&reopen_at),
            js(gate.change_basis),
            gate.scope_reopens,
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
