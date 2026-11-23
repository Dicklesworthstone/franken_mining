//! DESIGN (bd-ftw5). Cash calls and credits are submitted as settlement amounts for one entity.
//! Determinism: Exact

use super::{Card, Result, date, day, get, headroom, invalid, js, qty, row_fields, table, tables};
use fmine_registry::toml_subset::TomlDoc;
use std::collections::BTreeMap;

#[derive(Clone, Copy)]
struct Point<'a> {
    date: &'a str,
    phase: &'static str,
    broker_flow: i128,
    other_flow: i128,
    cash_without_shipment: i128,
    line: i128,
}

impl Point<'_> {
    fn gap(&self, amount: i128) -> String {
        format!(
            "{{\"date\":{},\"phase\":{},\"additional_usd\":\"{amount}\"}}",
            js(self.date),
            js(self.phase)
        )
    }
}

struct Outcome<'a> {
    peak_draw: i128,
    extra: i128,
    first_gap: Option<(Point<'a>, i128)>,
    worst_gap: Option<Point<'a>>,
    closing_cash: i128,
}

impl Outcome<'_> {
    fn json(&self, settlement: &str) -> String {
        format!(
            "{{\"collection_date\":{},\"peak_draw_needed_usd\":\"{}\",\"additional_line_usd\":\"{}\",\"first_shortfall\":{},\"peak_shortfall\":{},\"closing_cash_before_financing_usd\":\"{}\"}}",
            js(settlement),
            self.peak_draw,
            self.extra,
            self.first_gap
                .map_or_else(|| "null".into(), |(p, n)| p.gap(n)),
            self.worst_gap
                .map_or_else(|| "null".into(), |p| p.gap(self.extra)),
            self.closing_cash
        )
    }
}

struct Window<'a> {
    points: Vec<Point<'a>>,
    floor: i128,
    receipt: i128,
    receipt_phase: &'static str,
}

impl<'a> Window<'a> {
    fn arrival(&self, date: &str) -> usize {
        self.points
            .iter()
            .position(|p| p.date == date && p.phase == self.receipt_phase)
            .unwrap_or(self.points.len())
    }

    /// Unfinanced cash is the same in each case except for one collection. Borrowing
    /// fills only the floor deficit; the unused facility is never counted as a receipt.
    fn assess(&self, arrival: usize, mut ledger: Option<(&str, &mut Card)>) -> Result<Outcome<'a>> {
        let mut result = Outcome {
            peak_draw: 0,
            extra: 0,
            first_gap: None,
            worst_gap: None,
            closing_cash: 0,
        };
        for (i, p) in self.points.iter().enumerate() {
            let cash = p.cash_without_shipment + if i >= arrival { self.receipt } else { 0 };
            let draw = (-headroom(cash, 0, self.floor)?).max(0);
            let extra = (draw - p.line).max(0);
            result.peak_draw = result.peak_draw.max(draw);
            if extra > result.extra {
                result.first_gap.get_or_insert((*p, extra));
                result.worst_gap = Some(*p);
                result.extra = extra;
            }
            result.closing_cash = cash;
            if let Some((case, card)) = &mut ledger {
                card.rows.push(vec![
                    (*case).into(),
                    p.date.into(),
                    p.phase.into(),
                    p.broker_flow.to_string(),
                    p.other_flow.to_string(),
                    (if i == arrival { self.receipt } else { 0 }).to_string(),
                    cash.to_string(),
                    p.line.to_string(),
                    draw.to_string(),
                    extra.to_string(),
                ]);
            }
        }
        Ok(result)
    }

    fn show_case(&self, case: &str, date: &str, card: &mut Card) -> Result<Outcome<'a>> {
        let outcome = self.assess(self.arrival(date), Some((case, card)))?;
        let funding = match (outcome.first_gap, outcome.worst_gap) {
            (Some((first, amount)), Some(worst)) => format!(
                "First shortfall of {amount} USD on {} at {}. Peak additional funding of {} USD is needed on {} at {}.",
                first.date, first.phase, outcome.extra, worst.date, worst.phase
            ),
            _ => "The declared cash and committed line cover every checkpoint.".into(),
        };
        card.findings.push(format!("For {case} collection on {date}, the peak draw needed to keep the cash floor is {} USD. {funding}", outcome.peak_draw));
        Ok(outcome)
    }
}

pub(super) fn compare(doc: &TomlDoc, as_of: &str) -> Result<Card> {
    tables(doc, &["treasury", "shipment", "cash_day"])?;
    let t = row_fields(
        table(doc, "treasury")?,
        &[
            "entity",
            "through",
            "opening_cash_usd",
            "floor_usd",
            "committed_undrawn_usd",
            "line_from",
            "line_through",
            "cash_basis",
        ],
        as_of,
    )?;
    let s = row_fields(
        table(doc, "shipment")?,
        &[
            "entity",
            "cargo",
            "net_receipt_usd",
            "planned_collection",
            "delayed_collection",
            "collection_cutoff",
        ],
        as_of,
    )?;
    let entity = get(&t, "entity")?;
    if get(&s, "entity")? != entity
        || get(&t, "cash_basis")? != "unrestricted_excludes_undrawn_line_shipment_and_daily_flows"
    {
        return Err(invalid(
            "require one cash owner and unrestricted opening cash excluding the facility, shipment and daily flows",
        ));
    }
    let through = date(&t, "through")?;
    let planned = date(&s, "planned_collection")?;
    let delayed = date(&s, "delayed_collection")?;
    let from = date(&t, "line_from")?;
    let until = date(&t, "line_through")?;
    if through <= as_of || planned <= as_of || delayed < planned || until < from {
        return Err(invalid(
            "require a future cash horizon and original collection, a non-earlier delayed collection and an ordered facility window",
        ));
    }
    let cutoff = get(&s, "collection_cutoff")?;
    let receipt_phase = match cutoff {
        "before_calls" => "calls",
        "after_calls" => "close",
        _ => {
            return Err(invalid(
                "collection_cutoff requires before_calls or after_calls",
            ));
        }
    };
    let committed = qty(&t, "committed_undrawn_usd")?;
    let available_line = |d: &str| {
        if d >= from && d <= until {
            committed
        } else {
            0
        }
    };
    let mut days = BTreeMap::new();
    for row in doc.rows_of("cash_day") {
        let f = row_fields(
            row,
            &[
                "entity",
                "date",
                "broker_call_usd",
                "broker_credit_usd",
                "other_payment_usd",
                "other_receipt_usd",
            ],
            as_of,
        )?;
        let d = date(&f, "date")?;
        if get(&f, "entity")? != entity || d <= as_of || d > through {
            return Err(invalid(
                "each cash day must belong to the paying entity and fall inside the future window",
            ));
        }
        let flows = (
            qty(&f, "broker_call_usd")?,
            qty(&f, "broker_credit_usd")?,
            qty(&f, "other_payment_usd")?,
            qty(&f, "other_receipt_usd")?,
        );
        if days.insert(d, flows).is_some() {
            return Err(invalid("duplicate cash day"));
        }
    }
    if day(through)? - day(as_of)? != days.len() as i64 {
        return Err(invalid(
            "declare every calendar day after as_of through the horizon, including explicit zero-flow days",
        ));
    }

    let mut cash = qty(&t, "opening_cash_usd")?;
    let mut points = vec![Point {
        date: as_of,
        phase: "opening",
        broker_flow: 0,
        other_flow: 0,
        cash_without_shipment: cash,
        line: available_line(as_of),
    }];
    for (&date, &(call, credit, payment, receipt)) in &days {
        // The day's facility is effective at open. Debits settle before broker and other credits.
        for (phase, broker_flow, other_flow) in [
            ("open", 0, 0),
            ("calls", -call, -payment),
            ("close", credit, receipt),
        ] {
            cash += broker_flow + other_flow;
            points.push(Point {
                date,
                phase,
                broker_flow,
                other_flow,
                cash_without_shipment: cash,
                line: available_line(date),
            });
        }
    }
    let window = Window {
        points,
        floor: qty(&t, "floor_usd")?,
        receipt: qty(&s, "net_receipt_usd")?,
        receipt_phase,
    };
    let mut card = Card::new(&[
        "Case",
        "Date",
        "Checkpoint",
        "Broker flow USD",
        "Other flow USD",
        "Shipment received USD",
        "Cash before financing USD",
        "Committed line limit USD",
        "Draw needed for floor USD",
        "Additional line USD",
    ]);
    let planned_case = window.show_case("planned", planned, &mut card)?;
    let delayed_case = window.show_case("delayed", delayed, &mut card)?;
    let (status, last) = if window.assess(window.points.len(), None)?.extra == 0 {
        ("receipt_not_needed_within_window", None)
    } else {
        let mut latest = None;
        for &d in days.keys().rev() {
            if window.assess(window.arrival(d), None)?.extra == 0 {
                latest = Some(d);
                break;
            }
        }
        (
            if latest.is_some() {
                "latest_feasible_date"
            } else {
                "no_feasible_date_in_window"
            },
            latest,
        )
    };
    let deadline = match last {
        Some(d) => format!("To preserve the floor without extra funding, collect by {d} under the stated {} cutoff.", cutoff.replace('_', " ")),
        None if status == "receipt_not_needed_within_window" => format!("The floor holds through {through} even without the shipment receipt."),
        None => "Even collection on the first modeled day leaves a shortfall. Bringing this shipment forward within the window cannot fund the floor.".into(),
    };
    card.findings.push(deadline);
    // Before delayed settlement an advance adds exactly its amount. At and after
    // settlement the remaining invoice restores the original path. If the full receipt
    // on 'last' works, every uncovered checkpoint falls between the two receipts.
    let advance = if delayed_case.extra == 0 {
        format!(
            "{{\"status\":\"not_needed\",\"amount_usd\":\"0\",\"by\":null,\"remaining_on_delayed_date_usd\":\"{}\"}}",
            window.receipt
        )
    } else if let Some(by) = last {
        let amount = delayed_case.extra;
        let remaining = window.receipt - amount;
        card.findings.push(format!("Alternatively, collect at least {amount} USD of the invoice by {by} at the same cutoff and collect the remaining {remaining} USD on {delayed}. This covers the delayed case with the existing line."));
        format!(
            "{{\"status\":\"sufficient\",\"amount_usd\":\"{amount}\",\"by\":{},\"remaining_on_delayed_date_usd\":\"{remaining}\"}}",
            js(by)
        )
    } else {
        "{\"status\":\"insufficient_even_in_full\",\"amount_usd\":null,\"by\":null,\"remaining_on_delayed_date_usd\":null}".into()
    };
    card.findings.push(format!("{} expects {} USD from {}. The cash floor is {} USD. Opening precedes the day's transactions. Broker and other credits settle after calls.", entity, window.receipt, get(&s, "cargo")?, window.floor));
    card.detail = Some((
        "liquidity",
        format!(
            "{{\"planned\":{},\"delayed\":{},\"latest_collection_status\":{},\"latest_collection_date\":{},\"collection_cutoff\":{},\"advance_against_delayed_invoice\":{advance}}}",
            planned_case.json(planned),
            delayed_case.json(delayed),
            js(status),
            last.map_or_else(|| "null".into(), js),
            js(cutoff)
        ),
    ));
    Ok(card)
}
