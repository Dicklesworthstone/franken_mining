//! DESIGN (bd-0ln4). Compare one review period from operating and care starting states.
//! Determinism: Exact

use super::{
    Card, Result, date, dollars, fraction, get, headroom, invalid, js, qty, row_fields, table,
    tables,
};
use fmine_registry::toml_subset::TomlDoc;
use std::collections::BTreeMap;

struct Terms {
    tonnes: i128,
    upfront: i128,
    period_cost: i128,
    residual: i128,
    shortfall: i128,
}

impl Terms {
    fn cash(&self, price: i128) -> i128 {
        self.tonnes * price - self.period_cost - self.upfront
    }

    fn value(&self, price: i128) -> i128 {
        self.cash(price) + self.residual
    }
}

enum Route<'a> {
    Priced(Terms),
    Unavailable(&'a str),
}

// The first alternative produces metal. Both other alternatives have fixed cash terms.
const OPERATING: [&str; 3] = ["continue", "suspend", "close_from_operating"];
const CARE: [&str; 3] = ["restart", "remain_on_care", "close_from_care"];

fn names(items: &[&str]) -> String {
    format!(
        "[{}]",
        items.iter().map(|s| js(s)).collect::<Vec<_>>().join(",")
    )
}

fn readable(items: &[&str]) -> String {
    items
        .iter()
        .map(|name| action(name))
        .collect::<Vec<_>>()
        .join(" and ")
}

fn action(name: &str) -> &str {
    match name {
        "continue" => "continuing operations",
        "suspend" => "suspending operations",
        "restart" => "restarting",
        "remain_on_care" => "staying on care",
        "close_from_operating" | "close_from_care" => "permanent closure",
        other => other,
    }
}

fn number(v: Option<i128>) -> String {
    v.map_or_else(|| "null".into(), |n| js(&n.to_string()))
}

/// Only the upfront quote changes. At zero upfront, every extra dollar of price paid
/// reduces both window cash and comparable value by one dollar.
fn restart_quote(
    t: &Terms,
    price: i128,
    other: Option<i128>,
    room: i128,
    card: &mut Card,
) -> String {
    let economic_room = other.map(|value| t.value(price) + t.upfront - value);
    let limit = economic_room.map_or(room, |v| v.min(room));
    let maximum = (limit >= 0).then_some(limit);
    let cut = maximum.map(|v| (t.upfront - v).max(0));
    let mut message = match maximum {
        None if room < 0 => "Even a free restart leaves opening cash below the floor.".into(),
        None => "Even a free restart trails the best funded nonproducing choice at the submitted metal price.".into(),
        Some(max) if other.is_none() => format!("No nonproducing choice meets the upfront floor. Treasury could pay up to {} upfront to restart.", dollars(max)),
        Some(max) => format!("At today's price, the highest upfront restart quote that meets the floor and matches or beats the best nonproducing choice is {}. The submitted quote is {}.", dollars(max), dollars(t.upfront)),
    };
    if let Some(reduction) = cut.filter(|v| *v > 0) {
        message.push_str(&format!(
            " The required reduction is {}.",
            dollars(reduction)
        ));
    }
    card.findings.push(message);
    format!(
        "{{\"quoted_upfront_usd\":\"{}\",\"cash_above_floor_usd\":\"{room}\",\"advantage_before_upfront_usd\":{},\"max_upfront_usd\":{},\"required_reduction_usd\":{}}}",
        t.upfront,
        number(economic_room),
        number(maximum),
        number(cut)
    )
}

fn leaders<'a>(ranked: &[(&'a str, &Terms)], quote: i128) -> Vec<&'a str> {
    let Some((_, best)) = ranked.first() else {
        return Vec::new();
    };
    ranked
        .iter()
        .take_while(|(_, t)| t.value(quote) == best.value(quote))
        .map(|(name, _)| *name)
        .collect()
}

#[derive(Debug, PartialEq, Eq)]
enum Threshold {
    Unavailable,
    Unfunded,
    OnlyChoice,
    Behind,
    Tied,
    Ahead,
    Crossing { first: i128, tie: String },
}

impl Threshold {
    fn first(&self) -> Option<i128> {
        match self {
            Self::Crossing { first, .. } => Some(*first),
            Self::OnlyChoice | Self::Ahead | Self::Tied => Some(0),
            Self::Unavailable | Self::Unfunded | Self::Behind => None,
        }
    }
}

/// The nonproducing alternative's value is fixed. The first whole price includes a tie.
fn threshold(producing: Option<&Terms>, other: Option<i128>) -> Result<Threshold> {
    let Some(p) = producing else {
        return Ok(Threshold::Unavailable);
    };
    if p.shortfall > 0 {
        return Ok(Threshold::Unfunded);
    }
    let Some(other) = other else {
        return Ok(Threshold::OnlyChoice);
    };
    let gap = other - p.value(0);
    if p.tonnes == 0 {
        return Ok(match gap.cmp(&0) {
            std::cmp::Ordering::Greater => Threshold::Behind,
            std::cmp::Ordering::Equal => Threshold::Tied,
            std::cmp::Ordering::Less => Threshold::Ahead,
        });
    }
    if gap < 0 {
        return Ok(Threshold::Ahead);
    }
    Ok(Threshold::Crossing {
        first: gap / p.tonnes + i128::from(gap % p.tonnes != 0),
        tie: fraction(gap, p.tonnes)?,
    })
}

pub(super) fn compare(doc: &TomlDoc, as_of: &str) -> Result<Card> {
    tables(doc, &["review", "route", "unavailable"])?;
    let f = row_fields(
        table(doc, "review")?,
        &[
            "basis_id",
            "ownership_basis",
            "payable_product",
            "review_date",
            "starting_state",
            "net_usd_per_payable_tonne",
            "opening_cash_usd",
            "cash_floor_usd",
            "valuation_basis",
        ],
        as_of,
    )?;
    let basis = get(&f, "basis_id")?;
    let actual = get(&f, "starting_state")?;
    let through = date(&f, "review_date")?;
    if through <= as_of
        || !matches!(actual, "operating" | "care")
        || get(&f, "valuation_basis")? != "undiscounted_window_cash_plus_net_residual_at_review"
    {
        return Err(invalid(
            "require a future review date, operating or care starting state, and common undiscounted window cash plus net residual at review",
        ));
    }
    let quote = qty(&f, "net_usd_per_payable_tonne")?;
    let cash = qty(&f, "opening_cash_usd")?;
    let floor = qty(&f, "cash_floor_usd")?;
    let mut routes = BTreeMap::new();
    for row in doc.rows.iter().filter(|r| r.table != "review") {
        let unavailable = row.table == "unavailable";
        let required: &[&str] = if unavailable {
            &["route", "basis_id", "reason"]
        } else {
            &[
                "route",
                "basis_id",
                "payable_tonnes",
                "upfront_usd",
                "period_cost_usd",
                "net_residual_usd",
            ]
        };
        let r = row_fields(row, required, as_of)?;
        let name = get(&r, "route")?;
        if get(&r, "basis_id")? != basis || !(OPERATING.contains(&name) || CARE.contains(&name)) {
            return Err(invalid(
                "every route must name the same review basis and a supported transition",
            ));
        }
        let route = if unavailable {
            Route::Unavailable(get(&r, "reason")?)
        } else {
            let tonnes = qty(&r, "payable_tonnes")?;
            if tonnes != 0 && !matches!(name, "continue" | "restart") {
                return Err(invalid(
                    "care and closed alternatives require zero payable tonnes in this comparison",
                ));
            }
            let upfront = qty(&r, "upfront_usd")?;
            let residual = get(&r, "net_residual_usd")?
                .parse::<i64>()
                .map_err(|_| invalid("net_residual_usd requires signed whole USD within i64"))?;
            Route::Priced(Terms {
                tonnes,
                upfront,
                period_cost: qty(&r, "period_cost_usd")?,
                residual: i128::from(residual),
                shortfall: (-headroom(cash, upfront, floor)?).max(0),
            })
        };
        if routes.insert(name, route).is_some() {
            return Err(invalid(format!("duplicate status for route {name}")));
        }
    }
    let mut card = Card::new(&[
        "Starting state",
        "Choice",
        "Upfront USD",
        "Funding gap USD",
        "Cash through review USD",
        "Net residual USD",
        "Comparable value USD",
    ]);
    let mut summaries = Vec::new();
    let states = if actual == "operating" {
        [("operating", OPERATING), ("care", CARE)]
    } else {
        [("care", CARE), ("operating", OPERATING)]
    };
    for (state, choices) in states {
        let mut priced = Vec::new();
        for name in choices {
            let route = routes.get(name).ok_or_else(|| {
                invalid(format!(
                    "missing terms or explicit unavailability for {name}"
                ))
            })?;
            let label = name.replace('_', " ");
            match route {
                Route::Unavailable(reason) => {
                    card.rows.push(vec![
                        state.into(),
                        format!("{label} (unavailable: {reason})"),
                        "—".into(),
                        "—".into(),
                        "—".into(),
                        "—".into(),
                        "—".into(),
                    ]);
                }
                Route::Priced(t) => {
                    card.rows.push(vec![
                        state.into(),
                        label,
                        t.upfront.to_string(),
                        t.shortfall.to_string(),
                        t.cash(quote).to_string(),
                        t.residual.to_string(),
                        t.value(quote).to_string(),
                    ]);
                    priced.push((name, t));
                }
            }
        }
        let operating = choices[0];
        let producing = priced
            .iter()
            .find(|(name, _)| *name == operating)
            .map(|(_, t)| *t);
        priced.sort_by_key(|(_, t)| std::cmp::Reverse(t.value(quote)));
        let economic = leaders(&priced, quote);
        priced.retain(|(_, t)| t.shortfall == 0);
        let funded = leaders(&priced, quote);
        let advantage = match priced.as_slice() {
            [(_, best), (_, second), ..] => Some(best.value(quote) - second.value(quote)),
            _ => None,
        };
        let idle: Vec<_> = priced
            .iter()
            .copied()
            .filter(|(name, _)| *name != operating)
            .collect();
        let best_idle = idle.first().map(|(_, t)| t.value(0));
        let competing = leaders(&idle, 0);
        let limit = threshold(producing, best_idle)?;
        let setting = if state == actual {
            "Actual"
        } else {
            "Counterfactual"
        };
        let verdict = match priced.as_slice() {
            [] => "No available route meets the upfront cash floor.".into(),
            [(name, t), ..] if funded.len() == 1 => {
                let edge = advantage.map_or_else(String::new, |v| {
                    format!(
                        " Its advantage over the next eligible choice is {}.",
                        dollars(v)
                    )
                });
                format!(
                    "At {}/t, {} gives the highest value among choices meeting the upfront floor. Cash flow through review is {}.{}",
                    dollars(quote),
                    action(name),
                    dollars(t.cash(quote)),
                    edge
                )
            }
            _ => format!(
                "At {}/t, {} tie for highest value among choices meeting the upfront floor.",
                dollars(quote),
                readable(&funded)
            ),
        };
        card.findings
            .push(format!("{setting} starting state {state}. {verdict}"));
        if economic != funded {
            card.findings.push(format!(
                "Before the upfront funding screen, the highest-value choice from {state} is {}.",
                readable(&economic)
            ));
        }
        let operating = action(operating);
        let (status, tie, explanation) = match &limit {
            Threshold::Crossing { first, tie } => {
                let price = dollars(*first);
                let text = if tie.contains('/') {
                    format!(
                        "The tie between {operating} and {} is {tie} USD/t. The first whole payable price that favors or ties {operating} is {price}/t.",
                        readable(&competing)
                    )
                } else {
                    format!(
                        "At {price}/t, {operating} and {} have equal value.",
                        readable(&competing)
                    )
                };
                ("price_threshold", js(tie), text)
            }
            Threshold::OnlyChoice => (
                "only_funded_route",
                "null".into(),
                format!("From {state}, {operating} is the only choice meeting the upfront floor."),
            ),
            Threshold::Unavailable => (
                "operating_route_unavailable",
                "null".into(),
                format!("From {state}, {operating} is unavailable under the submitted terms."),
            ),
            Threshold::Unfunded => (
                "operating_route_unfunded",
                "null".into(),
                format!(
                    "From {state}, {operating} cannot meet the upfront cash floor at any sale price."
                ),
            ),
            Threshold::Behind => (
                "behind_at_every_price",
                "null".into(),
                format!(
                    "From {state}, {operating} delivers no payable tonnes by review and trails {} at every price.",
                    readable(&competing)
                ),
            ),
            Threshold::Tied => (
                "tied_at_every_price",
                "null".into(),
                format!(
                    "From {state}, {operating} ties {} at every price.",
                    readable(&competing)
                ),
            ),
            Threshold::Ahead => (
                "ahead_at_every_price",
                "null".into(),
                format!(
                    "From {state}, {operating} exceeds the funded nonproducing choices even at zero price."
                ),
            ),
        };
        card.findings.push(explanation);
        let terms = if state == "care" {
            let quote_limit = producing.map_or_else(
                || "null".into(),
                |t| restart_quote(t, quote, best_idle, cash - floor, &mut card),
            );
            format!(",\"restart_quote\":{quote_limit}")
        } else {
            String::new()
        };
        summaries.push(format!("{}:{{\"economic_best\":{},\"funded_best\":{},\"advantage_over_next_funded_usd\":{},\"funded_nonproducing_best\":{},\"threshold_status\":{},\"first_whole_usd_per_tonne\":{},\"tie_usd_per_tonne\":{tie}{terms}}}", js(state), names(&economic), names(&funded), number(advantage), names(&competing), js(status), number(limit.first())));
    }
    card.findings.push(format!("Review date {through}. Price applies to {}. Scope: {}. Both starts use {} in opening cash and a {} floor before receipts. Price thresholds hold tonnes, costs and residual values fixed.", get(&f,"payable_product")?,get(&f,"ownership_basis")?,dollars(cash),dollars(floor)));
    card.detail = Some((
        "operating_review",
        format!(
            "{{\"starting_state\":{},\"states\":{{{}}}}}",
            js(actual),
            summaries.join(",")
        ),
    ));
    Ok(card)
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use fmine_io::omf::json::{Json, parse};
    use fmine_registry::toml_subset::parse_subset;

    const NICKEL: &str = include_str!("../../../../../tests/capital/executive_care.toml");

    fn evaluate(input: &str) -> Result<Card> {
        let doc = parse_subset(input).expect("valid worksheet syntax");
        compare(&doc, "2030-03-31")
    }

    fn state(input: &str, starting: &str) -> Json {
        let (_, json) = evaluate(input)
            .expect("admissible terms")
            .detail
            .expect("details");
        parse(json.as_bytes())
            .expect("JSON")
            .get("states")
            .expect("states")
            .get(starting)
            .expect("starting state")
            .clone()
    }

    fn choices<'a>(state: &'a Json, field: &str) -> Vec<&'a str> {
        state
            .get(field)
            .and_then(Json::as_array)
            .expect("choice set")
            .iter()
            .map(|x| x.as_str().expect("choice"))
            .collect()
    }

    #[test]
    fn closure_competes_and_a_later_sale_cannot_fund_restart_today() {
        let cheap_close = NICKEL.replace("4500000", "0");
        let x = state(&cheap_close, "operating");
        assert_eq!(choices(&x, "funded_best"), ["close_from_operating"]);
        assert_eq!(
            x.get("tie_usd_per_tonne").and_then(Json::as_str),
            Some("13600")
        );
        let costly_restart = NICKEL
            .replace("11000", "17000")
            .replace("10000000", "7999999");
        let x = state(&costly_restart, "care");
        assert_eq!(choices(&x, "economic_best"), ["restart"]);
        assert_eq!(choices(&x, "funded_best"), ["remain_on_care"]);
        assert_eq!(
            x.get("threshold_status").and_then(Json::as_str),
            Some("operating_route_unfunded")
        );
        let at_floor = state(&costly_restart.replace("7999999", "8000000"), "care");
        assert_eq!(choices(&at_floor, "funded_best"), ["restart"]);
        let tie = state(&NICKEL.replace("11000", "13750"), "care");
        assert_eq!(choices(&tie, "funded_best"), ["restart", "remain_on_care"]);
        let no_cash = state(&NICKEL.replace("10000000", "4999999"), "operating");
        assert!(choices(&no_cash, "funded_best").is_empty());
    }

    #[test]
    fn maximum_restart_quote_matches_repriced_routes_at_the_exact_dollar() {
        let base = state(NICKEL, "care");
        let term = |key| {
            base.get("restart_quote")
                .and_then(|v| v.get(key))
                .and_then(Json::as_str)
        };
        assert_eq!(term("max_upfront_usd"), Some("800000"));
        assert_eq!(term("required_reduction_usd"), Some("2200000"));
        let reprice = |dollars: i128| {
            NICKEL.replace(
                "upfront_usd = \"3000000\"",
                &format!("upfront_usd = \"{dollars}\""),
            )
        };
        assert_eq!(
            choices(&state(&reprice(800000), "care"), "funded_best"),
            ["restart", "remain_on_care"]
        );
        assert_eq!(
            choices(&state(&reprice(800001), "care"), "funded_best"),
            ["remain_on_care"]
        );
        let funded_cap = state(&NICKEL.replace("10000000", "5500000"), "care");
        assert_eq!(
            funded_cap
                .get("restart_quote")
                .and_then(|v| v.get("max_upfront_usd"))
                .and_then(Json::as_str),
            Some("500000")
        );
        let free_loses = state(&NICKEL.replace("11000", "9999"), "care");
        assert_eq!(
            free_loses
                .get("restart_quote")
                .and_then(|v| v.get("max_upfront_usd")),
            Some(&Json::Null)
        );
        let free_ties = state(&NICKEL.replace("11000", "10000"), "care");
        assert_eq!(
            free_ties
                .get("restart_quote")
                .and_then(|v| v.get("max_upfront_usd"))
                .and_then(Json::as_str),
            Some("0")
        );
    }

    #[test]
    fn every_whole_price_threshold_matches_direct_cash_comparisons_including_flat_routes() {
        for tonnes in 0..=5 {
            for cost in 0..=7 {
                for terminal in -4..=4 {
                    for other in -6..=6 {
                        let t = Terms {
                            tonnes,
                            upfront: 2,
                            period_cost: cost,
                            residual: terminal,
                            shortfall: 0,
                        };
                        let first = threshold(Some(&t), Some(other))
                            .expect("exact threshold")
                            .first();
                        for price in 0..=24 {
                            assert_eq!(
                                first.is_some_and(|minimum| price >= minimum),
                                price * tonnes + terminal - 2 - cost >= other
                            );
                        }
                    }
                }
            }
        }
        let t = Terms {
            tonnes: 3,
            upfront: 0,
            period_cost: 5,
            residual: 0,
            shortfall: 0,
        };
        assert_eq!(
            threshold(Some(&t), Some(0)).expect("fractional tie"),
            Threshold::Crossing {
                first: 2,
                tie: "5/3".into()
            }
        );
    }

    #[test]
    fn missing_future_or_incompatible_terms_refuse_and_explicit_unavailability_stays_visible() {
        for input in [
            NICKEL.replace("2030-03-29", "2030-04-01"),
            NICKEL.replace("2030-06-30", "2030-03-31"),
            NICKEL.replacen("NB-NICKEL-Q2-PARENT", "JV", 1),
            NICKEL.replace("route = \"suspend\"", "route = \"restart\""),
            NICKEL.replacen("payable_tonnes = \"0\"", "payable_tonnes = \"1\"", 1),
            NICKEL.replace(
                "net_residual_usd = \"-1200000\"",
                "net_residual_usd = \"unknown\"",
            ),
            NICKEL.replace(
                "starting_state = \"operating\"",
                "starting_state = \"closed\"",
            ),
        ] {
            assert!(evaluate(&input).is_err());
        }
        let (prefix, _) = NICKEL.rsplit_once("[[route]]").expect("six priced routes");
        assert!(evaluate(prefix).is_err());
        let unavailable = format!(
            "{prefix}[[unavailable]]\nroute = \"close_from_care\"\nbasis_id = \"NB-NICKEL-Q2-PARENT\"\nsource = \"Synthetic terms\"\nknown_on = \"2030-03-30\"\nreason = \"Closure agreement outstanding\"\n"
        );
        let report = evaluate(&unavailable).expect("unavailable closure retained");
        assert_eq!(report.rows.len(), 6);
        assert!(
            report
                .rows
                .iter()
                .any(|row| row[1].contains("Closure agreement outstanding") && row[6] == "—")
        );
        let care_first = unavailable.replace(
            "starting_state = \"operating\"",
            "starting_state = \"care\"",
        );
        assert!(
            evaluate(&care_first).expect("care starts first").findings[0]
                .starts_with("Actual starting state care.")
        );
    }
}
