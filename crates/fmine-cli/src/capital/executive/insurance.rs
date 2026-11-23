//! DESIGN (bd-3ikw): conditional interruption indemnity for explicitly synthetic policy terms.
//! Determinism: Exact

use super::{
    Card, Currency, Fields, Money, Result, TomlDoc, date, dollars, get, invalid, js, qty,
    row_fields, table, tables,
};
use std::collections::BTreeMap;

#[derive(Clone, Copy)]
struct Bounds<T> {
    low: T,
    high: T,
}

fn bounds(f: &Fields<'_>, low: &str, high: &str) -> Result<Bounds<i128>> {
    let pair = Bounds {
        low: qty(f, low)?,
        high: qty(f, high)?,
    };
    if pair.low > pair.high {
        return Err(invalid(format!("{low} must be at most {high}")));
    }
    Ok(pair)
}

#[derive(Clone, Copy)]
enum Adjustment {
    Deductible,
    Limit,
}

impl Adjustment {
    fn name(self) -> &'static str {
        match self {
            Self::Deductible => "deductible",
            Self::Limit => "limit",
        }
    }
}

#[derive(Clone, Copy)]
struct Terms {
    waiting_days: i128,
    indemnity_days: i128,
    deductible: Money,
    limit: Money,
    order: [Adjustment; 2],
}

struct Bridge {
    economic: Money,
    eligible_days: i128,
    eligible: Money,
    adjusted: [Money; 2],
}

impl Terms {
    fn bridge(self, days: i128, daily_loss: Money) -> Result<Bridge> {
        let loss = |days| {
            daily_loss
                .minor
                .checked_mul(days)
                .map(|minor| Money::from_minor(daily_loss.ccy, minor))
                .ok_or_else(|| invalid("interruption loss exceeds the exact monetary range"))
        };
        let economic = loss(days)?;
        // In this interpretation both periods run from occurrence. The floor at
        // zero is a policy term for a loss that ends inside its waiting period.
        let eligible_days = (days.min(self.indemnity_days) - self.waiting_days).max(0);
        let eligible = loss(eligible_days)?;
        let mut adjusted = [eligible; 2];
        let mut amount = eligible;
        for (slot, action) in adjusted.iter_mut().zip(self.order) {
            amount = match action {
                Adjustment::Deductible => amount
                    .checked_sub(Money::from_minor(
                        self.deductible.ccy,
                        self.deductible.minor.min(amount.minor),
                    ))
                    .map_err(|e| invalid(e.to_string()))?,
                Adjustment::Limit => {
                    Money::from_minor(amount.ccy, amount.minor.min(self.limit.minor))
                }
            };
            *slot = amount;
        }
        Ok(Bridge {
            economic,
            eligible_days,
            eligible,
            adjusted,
        })
    }
}

fn whole(money: Money) -> i128 {
    money.minor / Money::SCALE
}

fn usd(low: Money, high: Money) -> String {
    if low == high {
        dollars(whole(low))
    } else {
        format!("{} to {}", dollars(whole(low)), dollars(whole(high)))
    }
}

fn json(low: i128, high: i128) -> String {
    format!(r#"{{"lower":"{low}","upper":"{high}"}}"#)
}

fn money_json(low: Money, high: Money) -> String {
    json(whole(low), whole(high))
}

pub(super) fn evaluate(doc: &TomlDoc, as_of: &str) -> Result<Card> {
    tables(doc, &["interruption", "loss_rate", "policy", "exclusion"])?;
    let event = row_fields(
        table(doc, "interruption")?,
        &["entity", "perimeter", "occurrence", "days_low", "days_high"],
        as_of,
    )?;
    let loss = row_fields(
        table(doc, "loss_rate")?,
        &[
            "entity",
            "perimeter",
            "currency",
            "usd_per_day_low",
            "usd_per_day_high",
        ],
        as_of,
    )?;
    let policy = row_fields(
        table(doc, "policy")?,
        &[
            "entity",
            "perimeter",
            "currency",
            "version",
            "interpretation_role",
            "day_basis",
            "waiting_days",
            "waiting_clause",
            "indemnity_days",
            "indemnity_clause",
            "deductible_usd",
            "deductible_clause",
            "limit_usd",
            "limit_clause",
            "order",
            "exclusions",
        ],
        as_of,
    )?;
    let entity = get(&event, "entity")?;
    let perimeter = get(&event, "perimeter")?;
    for f in [&loss, &policy] {
        if get(f, "entity")? != entity
            || get(f, "perimeter")? != perimeter
            || get(f, "currency")? != "USD"
        {
            return Err(invalid(
                "the interruption, loss and policy require the same insured entity and perimeter and a USD basis",
            ));
        }
    }
    let occurrence = date(&event, "occurrence")?;
    if occurrence > as_of || get(&policy, "day_basis")? != "both_from_occurrence" {
        return Err(invalid(
            "interruption must have occurred by the decision date, and both policy periods must run from occurrence",
        ));
    }
    let order = get(&policy, "order")?;
    let adjustments = match order {
        "deductible_then_limit" => [Adjustment::Deductible, Adjustment::Limit],
        "limit_then_deductible" => [Adjustment::Limit, Adjustment::Deductible],
        _ => {
            return Err(invalid(
                "declare deductible_then_limit or limit_then_deductible",
            ));
        }
    };
    let days = bounds(&event, "days_low", "days_high")?;
    let loss_rate = bounds(&loss, "usd_per_day_low", "usd_per_day_high")?;
    // qty() admits nonnegative i64 values widened to i128. Whole USD convert
    // exactly to Money before multiplication by the independently bounded days.
    let money = |usd| Money::from_major(Currency::USD, usd);
    let terms = Terms {
        waiting_days: qty(&policy, "waiting_days")?,
        indemnity_days: qty(&policy, "indemnity_days")?,
        deductible: money(qty(&policy, "deductible_usd")?),
        limit: money(qty(&policy, "limit_usd")?),
        order: adjustments,
    };
    let mut exclusions = BTreeMap::new();
    for row in doc.rows_of("exclusion") {
        let f = row_fields(row, &["id", "status"], as_of)?;
        let status = get(&f, "status")?;
        if !matches!(status, "applies" | "does_not_apply" | "unresolved")
            || exclusions
                .insert(get(&f, "id")?, (status, row.line))
                .is_some()
        {
            return Err(invalid(
                "exclusions require distinct IDs and status applies, does_not_apply or unresolved",
            ));
        }
    }
    if !matches!(
        (get(&policy, "exclusions")?, exclusions.is_empty()),
        ("none", true) | ("listed", false)
    ) {
        return Err(invalid(
            "policy exclusions must be declared as none or supported by listed rows",
        ));
    }
    let blocked = exclusions.values().any(|(status, _)| *status == "applies");
    let unresolved = exclusions
        .values()
        .any(|(status, _)| *status == "unresolved");
    let version = get(&policy, "version")?;

    let low = terms.bridge(days.low, money(loss_rate.low))?;
    let high = terms.bridge(days.high, money(loss_rate.high))?;
    let later_low = terms.bridge(days.high, money(loss_rate.low))?.adjusted[1];
    let sooner_high = terms.bridge(days.low, money(loss_rate.high))?.adjusted[1];
    let (adjusted_low, adjusted_high) = (low.adjusted[1], high.adjusted[1]);
    let duration_matters = !blocked && (later_low != adjusted_low || sooner_high != adjusted_high);
    let rate_matters = !blocked && (sooner_high != adjusted_low || later_low != adjusted_high);
    let exclusion_matters = !blocked && unresolved && adjusted_high.minor > 0;

    let mut card = Card::new(&["Amount", "Declared bounds", "Basis"]);
    let mut branches = Vec::new();
    let mut add_branch = |name, label: &str, lo, hi| {
        branches.push(format!(
            r#"{{"condition":{},"usd":{}}}"#,
            js(name),
            money_json(lo, hi)
        ));
        (usd(lo, hi), label.to_owned())
    };
    let eligible_days = if low.eligible_days == high.eligible_days {
        low.eligible_days.to_string()
    } else {
        format!("{}–{}", low.eligible_days, high.eligible_days)
    };
    card.rows = vec![
        vec![
            "Economic loss".into(),
            usd(low.economic, high.economic),
            "Outage days × submitted daily loss".into(),
        ],
        vec![
            "Policy-eligible loss".into(),
            usd(low.eligible, high.eligible),
            format!("{eligible_days} eligible days after the day cap and wait"),
        ],
    ];
    if !blocked {
        let (amount, note) = add_branch(
            "no_exclusion_applies",
            "No listed exclusion applies",
            adjusted_low,
            adjusted_high,
        );
        card.rows
            .push(vec!["Conditional recovery".into(), amount, note]);
    }
    if blocked || unresolved {
        let (amount, note) = add_branch(
            "exclusion_applies",
            "A listed exclusion applies",
            money(0),
            money(0),
        );
        card.rows
            .push(vec!["Conditional recovery".into(), amount, note]);
    }

    let deductible = format!("{} deductible", dollars(whole(terms.deductible)));
    let limit = format!("{} limit", dollars(whole(terms.limit)));
    let order_label = if order == "deductible_then_limit" {
        format!("{deductible} before {limit}")
    } else {
        format!("{limit} before {deductible}")
    };
    card.findings.push(format!(
        "{entity} · {perimeter} · occurrence {occurrence} · synthetic policy {version}."
    ));
    card.findings.push(format!(
        "Apply the {}-day indemnity window and {}-day wait from occurrence, then the {order_label}.",
        terms.indemnity_days, terms.waiting_days
    ));
    let mut drivers = Vec::new();
    if duration_matters {
        drivers.push("the interruption chronology");
    }
    if rate_matters {
        drivers.push("the daily loss estimate");
    }
    if exclusion_matters {
        drivers.push("whether any unresolved exclusion applies");
    }
    if drivers.is_empty() {
        card.findings.push("The conditional recovery bounds hold across the stated duration, daily loss and exclusion alternatives.".into());
    } else {
        card.findings.push(format!(
            "The recovery bounds can narrow by confirming {}.",
            drivers.join(", ")
        ));
    }
    card.findings.push(
        "Treasury cash planning needs a separately confirmed payment amount and date.".into(),
    );

    let adjustment_rows = adjustments
        .iter()
        .enumerate()
        .map(|(i, op)| {
            let name = op.name();
            let clause = get(&policy, &format!("{name}_clause"))?;
            let parameter = match op {
                Adjustment::Deductible => terms.deductible,
                Adjustment::Limit => terms.limit,
            };
            Ok(format!(
                r#"{{"term":{},"clause":{},"parameter_usd":"{}","remaining_usd":{}}}"#,
                js(name),
                js(clause),
                whole(parameter),
                money_json(low.adjusted[i], high.adjusted[i]),
            ))
        })
        .collect::<Result<Vec<_>>>()?;
    let exclusion_rows = exclusions
        .iter()
        .map(|(id, (status, line))| {
            format!(
                r#"{{"id":{},"status":{},"source_line":{line}}}"#,
                js(id),
                js(status)
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    card.detail = Some((
        "indemnity",
        format!(
            r#"{{"entity":{},"perimeter":{},"currency":"USD","occurrence":{},"terms_version":{},"interpretation_role":{},"order":{},"outage_days":{},"daily_loss_usd":{},"economic_loss_usd":{},"policy_eligible_loss_usd":{},"eligible_days":{},"day_terms":{{"basis":"both_from_occurrence","indemnity_days":"{}","indemnity_clause":{},"waiting_days":"{}","waiting_clause":{}}},"monetary_steps":[{}],"conditional_recovery":[{}],"exclusions":[{exclusion_rows}],"bound_drivers":{{"duration":{duration_matters},"daily_loss":{rate_matters},"exclusions":{exclusion_matters}}}}}"#,
            js(entity),
            js(perimeter),
            js(occurrence),
            js(version),
            js(get(&policy, "interpretation_role")?),
            js(order),
            json(days.low, days.high),
            json(loss_rate.low, loss_rate.high),
            money_json(low.economic, high.economic),
            money_json(low.eligible, high.eligible),
            json(low.eligible_days, high.eligible_days),
            terms.indemnity_days,
            js(get(&policy, "indemnity_clause")?),
            terms.waiting_days,
            js(get(&policy, "waiting_clause")?),
            adjustment_rows.join(","),
            branches.join(","),
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

    const EVENT: &str = include_str!("../../../../../tests/capital/executive_interruption.toml");

    fn run(text: &str) -> Result<Json> {
        let doc = parse_subset(text).expect("valid synthetic TOML");
        let anchor = SourceAnchor::new("interrupt.toml", sha256(text.as_bytes()).0, "worksheet");
        let result = super::super::worksheet(&doc, text, &anchor, Path::new("."), false)?;
        Ok(parse(result.payload_json.as_bytes()).expect("valid JSON result"))
    }

    fn at<'a>(value: &'a Json, keys: &[&str]) -> &'a Json {
        keys.iter().fold(value, |v, k| v.get(k).expect(k))
    }

    fn upper_recovery(payload: &Json) -> &str {
        let rows = at(payload, &["indemnity", "conditional_recovery"])
            .as_array()
            .expect("branch rows");
        at(&rows[0], &["usd", "upper"]).as_str().expect("amount")
    }

    #[test]
    fn order_and_indemnity_cap_change_the_recovery_as_in_the_known_answers() {
        let normal = run(EVENT).expect("original policy");
        let reversed = run(&EVENT.replace("deductible_then_limit", "limit_then_deductible"))
            .expect("reversed terms");
        let capped = run(&EVENT.replace("indemnity_days = \"10\"", "indemnity_days = \"5\""))
            .expect("day cap");
        assert_eq!(upper_recovery(&normal), "3000000");
        assert_eq!(upper_recovery(&reversed), "2500000");
        assert_eq!(upper_recovery(&capped), "2500000");
        assert_eq!(
            at(&capped, &["indemnity", "policy_eligible_loss_usd", "upper"]).as_str(),
            Some("3000000")
        );
        assert_ne!(
            at(&normal, &["source", "sha256"]),
            at(&reversed, &["source", "sha256"])
        );
        let below_wait = run(&EVENT.replace("waiting_days = \"2\"", "waiting_days = \"7\""))
            .expect("waiting period");
        assert_eq!(upper_recovery(&below_wait), "0");
        let no_deductible = run(&EVENT
            .replace("deductible_usd = \"500000\"", "deductible_usd = \"0\"")
            .replace("limit_usd = \"3000000\"", "limit_usd = \"5000000\""))
        .expect("zero deductible");
        assert_eq!(upper_recovery(&no_deductible), "4000000");
    }

    #[test]
    fn bounds_identify_when_chronology_loss_and_exclusions_change_the_answer() {
        let uncertain = EVENT
            .replace("days_low = \"6\"", "days_low = \"3\"")
            .replace("days_high = \"6\"", "days_high = \"7\"")
            .replace(
                "usd_per_day_low = \"1000000\"",
                "usd_per_day_low = \"750000\"",
            );
        let result = run(&uncertain).expect("uncertain loss");
        let indemnity = at(&result, &["indemnity"]);
        for driver in ["duration", "daily_loss", "exclusions"] {
            assert_eq!(at(indemnity, &["bound_drivers", driver]), &Json::Bool(true));
        }
        assert_eq!(
            at(indemnity, &["economic_loss_usd", "lower"]).as_str(),
            Some("2250000")
        );
        let branches = at(indemnity, &["conditional_recovery"])
            .as_array()
            .expect("two branches");
        assert_eq!(at(&branches[0], &["usd", "lower"]).as_str(), Some("250000"));
        assert_eq!(at(&branches[1], &["usd", "upper"]).as_str(), Some("0"));

        let all_capped = EVENT
            .replace("days_high = \"6\"", "days_high = \"8\"")
            .replace(
                "usd_per_day_high = \"1000000\"",
                "usd_per_day_high = \"2000000\"",
            )
            .replace("status = \"unresolved\"", "status = \"does_not_apply\"");
        let result = run(&all_capped).expect("binding limit");
        assert_eq!(upper_recovery(&result), "3000000");
        for driver in ["duration", "daily_loss", "exclusions"] {
            assert_eq!(
                at(&result, &["indemnity", "bound_drivers", driver]),
                &Json::Bool(false)
            );
        }

        let excluded = run(&uncertain.replace("status = \"unresolved\"", "status = \"applies\""))
            .expect("stated exclusion");
        assert_eq!(upper_recovery(&excluded), "0");
        assert_eq!(
            at(&excluded, &["indemnity", "conditional_recovery"])
                .as_array()
                .expect("one branch")
                .len(),
            1
        );
        assert_eq!(
            at(
                &excluded,
                &["indemnity", "policy_eligible_loss_usd", "upper"]
            )
            .as_str(),
            Some("5000000")
        );
        let none = EVENT
            .split_once("[[exclusion]]")
            .expect("exclusion")
            .0
            .replace("exclusions = \"listed\"", "exclusions = \"none\"");
        let admitted = run(&none).expect("explicitly no exclusions");
        assert_eq!(upper_recovery(&admitted), "3000000");
        assert_eq!(
            at(&admitted, &["indemnity", "conditional_recovery"])
                .as_array()
                .expect("one branch")
                .len(),
            1
        );
    }

    #[test]
    fn inconsistent_missing_future_overflowing_or_live_inputs_refuse() {
        let overflow = EVENT
            .replace("days_high = \"6\"", "days_high = \"9223372036854775807\"")
            .replace(
                "usd_per_day_high = \"1000000\"",
                "usd_per_day_high = \"9223372036854775807\"",
            );
        for (input, expected) in [
            (
                EVENT.replacen(
                    "entity = \"North Range Copper\"",
                    "entity = \"Other entity\"",
                    1,
                ),
                "same insured entity",
            ),
            (
                EVENT.replacen("currency = \"USD\"", "currency = \"CAD\"", 1),
                "USD basis",
            ),
            (
                EVENT.replace("days_low = \"6\"", "days_low = \"7\""),
                "must be at most",
            ),
            (
                EVENT.replace("known_on = \"2030-06-09\"", "known_on = \"2030-06-11\""),
                "unavailable",
            ),
            (
                EVENT.replace("occurrence = \"2030-06-01\"", "occurrence = \"2030-06-12\""),
                "occurred by",
            ),
            (
                EVENT.replace(
                    "evidence = \"synthetic\"",
                    "evidence = \"operator_declared\"",
                ),
                "synthetic evidence",
            ),
            (
                EVENT
                    .split_once("[[exclusion]]")
                    .expect("exclusion")
                    .0
                    .to_owned(),
                "supported by listed rows",
            ),
            (overflow, "exact monetary range"),
        ] {
            let refusal = run(&input).expect_err("incompatible terms");
            assert!(refusal.what.contains(expected), "{}", refusal.what);
        }
    }

    #[test]
    fn each_order_is_monotone_and_bounded_by_eligible_loss_and_limit() {
        let money = |value| Money::from_major(Currency::USD, value);
        for order in [
            [Adjustment::Deductible, Adjustment::Limit],
            [Adjustment::Limit, Adjustment::Deductible],
        ] {
            for waiting_days in 0..=5 {
                for indemnity_days in 0..=5 {
                    for deductible in 0..=5 {
                        for limit in 0..=5 {
                            let terms = Terms {
                                waiting_days,
                                indemnity_days,
                                deductible: money(deductible),
                                limit: money(limit),
                                order,
                            };
                            let mut previous = 0;
                            for days in 0..=8 {
                                let result = terms.bridge(days, money(3)).expect("bounded domain");
                                let amount = result.adjusted[1].minor;
                                assert!(
                                    amount >= previous
                                        && amount <= terms.limit.minor
                                        && amount <= result.eligible.minor
                                );
                                assert!(result.eligible.minor <= result.economic.minor);
                                previous = amount;
                            }
                        }
                    }
                }
            }
        }
    }
}
