//! DESIGN: dated, source-bound alternatives for a single executive decision.
//! Determinism: Exact
//! Worksheet inputs are declarations, including their claimed availability.

use std::cmp::Reverse;
use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::path::Path;

use fmine_core::anchor::SourceAnchor;
use fmine_evidence::Refusal;
use fmine_registry::toml_subset::{TomlDoc, TomlRow};
use fmine_time::CivilDate;
use fmine_units::{Currency, ExactQty, Money};

use super::investment::{Fields, fields, get, invalid};
use crate::{Envelope, json_string as js, worksheet_input};
use worksheet_input::markdown_cell as cell;

mod care;
mod deck;
mod hedge;
mod insurance;
mod permit;

pub(super) const SCHEMA: &str = "fmine.executive-decision.v1";
const BOUNDARY: &str = "Conditional on the submitted worksheet, dated sources and completeness of the declared scope. No calibrated probability, authenticated historical availability, real-world savings, execution authority or sign-off is inferred.";
type Result<T> = std::result::Result<T, Refusal>;

/// Fields common to every decision row include the source locator and the date it became available.
fn row_fields<'a>(row: &'a TomlRow, names: &[&str], as_of: &str) -> Result<Fields<'a>> {
    row_fields_with_optional(row, names, &[], as_of)
}

fn row_fields_with_optional<'a>(
    row: &'a TomlRow,
    names: &[&str],
    optional: &[&str],
    as_of: &str,
) -> Result<Fields<'a>> {
    let mut all = vec!["source", "known_on"];
    all.extend_from_slice(names);
    let f = fields(&row.fields, &all, optional)?;
    if date(&f, "known_on")? > as_of {
        return Err(invalid(format!(
            "line {}: source was unavailable on the decision date",
            row.line
        )));
    }
    Ok(f)
}

fn date<'a>(f: &Fields<'a>, k: &str) -> Result<&'a str> {
    let s = get(f, k)?;
    if !fmine_registry::sources::is_date(s) {
        return Err(invalid(format!("{k} requires a calendar date YYYY-MM-DD")));
    }
    Ok(s)
}

fn day(s: &str) -> Result<i64> {
    let err = || invalid("invalid calendar day");
    // Callers first validate ASCII YYYY-MM-DD with date().
    CivilDate::new(
        s[..4].parse().map_err(|_| err())?,
        s[5..7].parse().map_err(|_| err())?,
        s[8..].parse().map_err(|_| err())?,
    )
    .map(CivilDate::to_epoch_days)
    .map_err(|_| err())
}

// Whole USD or whole declared physical units. At most 16 rows; widening each i64 leaves
// room in i128 for summed cash and for products of two nonnegative inputs.
fn qty(f: &Fields<'_>, k: &str) -> Result<i128> {
    get(f, k)?
        .parse::<i64>()
        .ok()
        .filter(|v| *v >= 0)
        .map(i128::from)
        .ok_or_else(|| {
            invalid(format!(
                "{k} requires a nonnegative whole quantity within i64"
            ))
        })
}

fn flag(f: &Fields<'_>, k: &str) -> Result<bool> {
    match get(f, k)? {
        "yes" => Ok(true),
        "no" => Ok(false),
        _ => Err(invalid(format!("{k} requires yes or no"))),
    }
}

fn table<'a>(doc: &'a TomlDoc, name: &'a str) -> Result<&'a TomlRow> {
    let mut rows = doc.rows_of(name);
    let row = rows
        .next()
        .ok_or_else(|| invalid(format!("missing {name}")))?;
    if rows.next().is_some() {
        return Err(invalid(format!("duplicate {name}")));
    }
    Ok(row)
}

fn tables(doc: &TomlDoc, allowed: &[&str]) -> Result<()> {
    if doc
        .rows
        .iter()
        .any(|r| !allowed.contains(&r.table.as_str()))
    {
        return Err(invalid("unexpected table for the decision type"));
    }
    Ok(())
}

struct Card {
    findings: Vec<String>,
    columns: Vec<&'static str>,
    rows: Vec<Vec<String>>,
    /// Optional mode-specific JSON member, built alongside the human findings.
    detail: Option<(&'static str, String)>,
    calculation_digest: Option<String>,
    journal_actions: Vec<(&'static str, &'static str)>,
}
impl Card {
    fn new(columns: &[&'static str]) -> Self {
        Self {
            findings: Vec::new(),
            columns: columns.to_vec(),
            rows: Vec::new(),
            detail: None,
            calculation_digest: None,
            journal_actions: Vec::new(),
        }
    }
}

pub(super) fn worksheet(
    doc: &TomlDoc,
    text: &str,
    source: &SourceAnchor,
    root: &Path,
    brief: bool,
) -> Result<Envelope> {
    evaluate(doc, text, source, root, brief, None)
}

pub(crate) struct ExecutiveSnapshot {
    pub card: String,
    pub claim_id: String,
    pub not_before_ns: i128,
}

pub(super) fn journal_snapshot(
    doc: &TomlDoc,
    text: &str,
    source: &SourceAnchor,
    root: &Path,
    action: &str,
) -> Result<ExecutiveSnapshot> {
    use fmine_claim::{AdditivityClass, Claim, Confidentiality, Representation, SpatialSupport};
    use fmine_evidence::{EpistemicStatus, EvidenceClass, KnowledgeState, KuKind};
    use fmine_registry::toml_subset::TomlValue;

    let evaluated = evaluate(doc, text, source, root, false, Some(action))?;
    let Some(TomlValue::Str(as_of)) = doc.top_get("as_of").map(|f| &f.value) else {
        return Err(invalid("executive worksheet is missing its decision date"));
    };
    let (metric, domain) = match doc.top_get("decision").map(|f| &f.value) {
        // Existing procurement receipts retain their original identity domain.
        Some(TomlValue::Str(mode)) if mode == "purchase" => (
            "decision.procurement-worksheet/1",
            "procurement-worksheet/v1",
        ),
        Some(TomlValue::Str(mode)) if mode == "operating_review" => {
            ("decision.operating-review/1", "operating-review/v1")
        }
        Some(TomlValue::Str(mode)) if mode == "common_price_deck" => {
            ("decision.common-price-deck/1", "common-price-deck/v1")
        }
        _ => {
            return Err(invalid(
                "executive worksheet does not support a journal choice",
            ));
        }
    };
    let card = evaluated.to_json();
    let tag = format!("{domain}:{}", fmine_core::hash::sha256(card.as_bytes()));
    // This Claim identifies the retained worksheet calculation. The human choice and
    // rationale are separately supplied by the journal writer.
    let binding = Claim::new(
        metric,
        (),
        &tag,
        Representation::Deterministic,
        SpatialSupport::PeriodAggregate,
        AdditivityClass::NonAdditive,
        EvidenceClass::Assumed,
        EpistemicStatus::KnownUnknown(KuKind::Parametric),
        KnowledgeState::Estimated,
        Confidentiality::internal(),
        Vec::new(),
        Vec::new(),
        vec![source.clone()],
    );
    Ok(ExecutiveSnapshot {
        card,
        claim_id: binding.claim_id.to_string(),
        not_before_ns: i128::from(day(as_of)?) * 86_400_000_000_000,
    })
}

fn evaluate(
    doc: &TomlDoc,
    text: &str,
    source: &SourceAnchor,
    root: &Path,
    brief: bool,
    selected_action: Option<&str>,
) -> Result<Envelope> {
    if text.len() > 65536 || doc.rows.is_empty() || doc.rows.len() > 16 {
        return Err(invalid(
            "executive worksheets require 1–16 source rows within 64 KiB",
        ));
    }
    let f = fields(
        &doc.top,
        &[
            "schema",
            "decision",
            "question",
            "owner",
            "as_of",
            "scope",
            "evidence",
            "uncertainty",
            "revisit_if",
        ],
        &[],
    )?;
    let as_of = date(&f, "as_of")?;
    let evidence = get(&f, "evidence")?;
    if !matches!(evidence, "synthetic" | "operator_declared") {
        return Err(invalid("evidence must be synthetic or operator_declared"));
    }
    let mode = get(&f, "decision")?;
    if selected_action.is_some()
        && !matches!(mode, "purchase" | "operating_review" | "common_price_deck")
    {
        return Err(invalid(
            "only procurement, operating-review and common-price-deck executive worksheets can be recorded in the decision journal",
        ));
    }
    let card = match mode {
        "purchase" => purchase(doc, as_of)?,
        "common_price_deck" => deck::compare(doc, as_of, source, root, selected_action.is_none())?,
        "operating_review" => care::compare(doc, as_of)?,
        "hedge_liquidity" => hedge::compare(doc, as_of)?,
        "interruption_indemnity" if evidence == "synthetic" => insurance::evaluate(doc, as_of)?,
        "interruption_indemnity" => {
            return Err(invalid(
                "interruption indemnity requires synthetic evidence",
            ));
        }
        "permit_early_works" if evidence == "synthetic" => permit::evaluate(doc, as_of)?,
        "permit_early_works" => {
            return Err(invalid(
                "the early-works permit worksheet requires synthetic evidence",
            ));
        }
        "rehabilitation" => rehabilitation(doc, as_of)?,
        "tenure" => tenure(doc, as_of)?,
        "group_recovery" => group_recovery(doc, as_of)?,
        "project_estimates" => project_estimates(doc, as_of)?,
        _ => return Err(invalid("unknown executive decision type")),
    };
    if let Some(action) = selected_action {
        let id = if let Some(digest) = &card.calculation_digest {
            let (id, reviewed) = action.split_once('@').ok_or_else(|| {
                invalid("copy a digest-bound action from decide program --json journal_actions")
            })?;
            if reviewed != digest {
                return Err(Refusal::new(
                    fmine_evidence::RefusalCode::DigestMismatch505,
                    "common price-deck comparison differs from the reviewed action; evaluate the worksheet again before recording",
                ));
            }
            id
        } else {
            action
        };
        if !card
            .journal_actions
            .iter()
            .any(|(offered, _)| *offered == id)
        {
            return Err(invalid(format!(
                "{mode} action {action} is unavailable in the evaluated worksheet"
            )));
        }
    }
    let action_id = |id: &str| match &card.calculation_digest {
        Some(digest) => format!("{id}@{digest}"),
        None => id.to_owned(),
    };
    let array = |v: &[String]| {
        format!(
            "[{}]",
            v.iter().map(|s| js(s)).collect::<Vec<_>>().join(",")
        )
    };
    let columns: Vec<_> = card.columns.iter().map(|s| s.to_string()).collect();
    let mut md = format!(
        "# {}\n\n{} · {} · {} · {}\n\n",
        cell(get(&f, "question")?),
        cell(get(&f, "owner")?),
        as_of,
        cell(get(&f, "scope")?),
        evidence
    );
    for msg in &card.findings {
        let _ = writeln!(md, "- {}", cell(msg));
    }
    // Other worksheets need their alternative rows to convey the decision. The treasury
    // findings already give both cases, the binding dates and the invoice split.
    if !brief || mode != "hedge_liquidity" {
        let _ = writeln!(
            md,
            "\n| {} |\n|{}|",
            columns.join(" | "),
            vec!["---"; columns.len()].join("|")
        );
        for row in &card.rows {
            let _ = writeln!(
                md,
                "| {} |",
                row.iter().map(|s| cell(s)).collect::<Vec<_>>().join(" | ")
            );
        }
    }
    let _ = writeln!(
        md,
        "\nUncertainty: {}. Revisit if: {}.\n\nEvidence: {} (SHA-256 {}).",
        cell(get(&f, "uncertainty")?),
        cell(get(&f, "revisit_if")?),
        cell(&source.artifact_path),
        fmine_core::Digest32(source.artifact_sha256)
    );
    if let Some(digest) = &card.calculation_digest {
        let _ = writeln!(md, "\nInvestment comparison SHA-256: {digest}.");
    }
    if !brief && !card.journal_actions.is_empty() {
        let actions = card
            .journal_actions
            .iter()
            .map(|(id, label)| format!("{label} (`{}`)", action_id(id)))
            .collect::<Vec<_>>()
            .join(", ");
        let _ = writeln!(
            md,
            "\nChoices for `decide journal record --action` are {actions}."
        );
    }
    let mut sources = Vec::new();
    for r in &doc.rows {
        let text_field = |key| match r.get(key).map(|f| &f.value) {
            Some(fmine_registry::toml_subset::TomlValue::Str(value)) => Ok(value.as_str()),
            _ => Err(invalid(format!(
                "line {}: missing source evidence {key}",
                r.line
            ))),
        };
        let locator = text_field("source")?;
        let known_on = text_field("known_on")?;
        if !brief {
            let _ = writeln!(
                md,
                "- {} line {} (known {}): {}",
                r.table,
                r.line,
                known_on,
                cell(locator)
            );
        }
        sources.push(format!(
            "{{\"table\":{},\"line\":{},\"known_on\":{},\"locator\":{}}}",
            js(&r.table),
            r.line,
            js(known_on),
            js(locator)
        ));
    }
    let _ = writeln!(md, "\n{BOUNDARY}");
    let detail = card
        .detail
        .as_ref()
        .map_or_else(String::new, |(key, value)| format!(",{}:{value}", js(key)));
    let actions = if card.journal_actions.is_empty() {
        String::new()
    } else {
        let rows = card
            .journal_actions
            .iter()
            .map(|(id, label)| format!(r#"{{"id":{},"label":{}}}"#, js(&action_id(id)), js(label)))
            .collect::<Vec<_>>()
            .join(",");
        format!(",\"journal_actions\":[{rows}]")
    };
    let payload = format!(
        "{{\"mode\":{},\"evidence\":{},\"as_of\":{},\"question\":{},\"owner\":{},\"support\":{},\"evidence_class\":\"assumed\",\"calibrated\":false,\"uncertainty\":{},\"revisit_if\":{},\"source_rows\":[{}],\"findings\":{},\"columns\":{},\"rows\":[{}],\"source\":{},\"worksheet\":{}{detail}{actions}}}",
        js(mode),
        js(evidence),
        js(as_of),
        js(get(&f, "question")?),
        js(get(&f, "owner")?),
        js(get(&f, "scope")?),
        js(get(&f, "uncertainty")?),
        js(get(&f, "revisit_if")?),
        sources.join(","),
        array(&card.findings),
        array(&columns),
        card.rows
            .iter()
            .map(|r| array(r))
            .collect::<Vec<_>>()
            .join(","),
        worksheet_input::source_json(source, text.len()),
        js(text)
    );
    let mut envelope = Envelope::ok("decide program", payload, Vec::new());
    envelope.rendered_markdown = Some(md);
    envelope.no_claim = Some(BOUNDARY);
    Ok(envelope)
}

fn headroom(cash: i128, payment: i128, floor: i128) -> Result<i128> {
    let usd = |n| Money::from_major(Currency::USD, n);
    let (_, slack) = fmine_capital::cash_after_payment(usd(cash), usd(payment), usd(floor))?;
    Ok(slack.minor / Money::SCALE)
}

/// Whole USD are serialized as decimal strings, including zero. An impossible cash
/// limit and a missing live hold credit each remain absent.
struct PurchaseLimits {
    replacement_price: Option<i128>,
    latest_delivery: Option<String>,
    hold_credit: Option<i128>,
    hold_fee: Option<i128>,
    explicit_exercise_price: Option<i128>,
}

impl PurchaseLimits {
    fn json(&self) -> String {
        let usd = |n: Option<i128>| n.map_or_else(|| "null".into(), |v| js(&v.to_string()));
        let exercise_price = self
            .explicit_exercise_price
            .map_or_else(String::new, |price| {
                format!(",\"fixed_exercise_price_usd\":{}", js(&price.to_string()))
            });
        format!(
            "{{\"max_replacement_price_usd\":{},\"latest_on_time_delivery\":{},\"fixed_exercise_credit_usd\":{},\"max_hold_fee_usd\":{}{exercise_price}}}",
            usd(self.replacement_price),
            self.latest_delivery
                .as_deref()
                .map_or_else(|| "null".into(), js),
            usd(self.hold_credit),
            usd(self.hold_fee),
        )
    }
}

fn purchase(doc: &TomlDoc, as_of: &str) -> Result<Card> {
    tables(doc, &["order", "treasury", "schedule", "hold", "wait"])?;
    let order = row_fields_with_optional(
        table(doc, "order")?,
        &[
            "entity",
            "expires",
            "review",
            "price_usd",
            "deposit_usd",
            "total_cancel_usd",
        ],
        &["preserves_slot_through_expiry"],
        as_of,
    )?;
    let fund = row_fields(
        table(doc, "treasury")?,
        &[
            "entity",
            "decision_cash_usd",
            "review_cash_usd",
            "decision_floor_usd",
            "review_floor_usd",
            "other_project",
            "other_due_usd",
        ],
        as_of,
    )?;
    let schedule = row_fields(
        table(doc, "schedule")?,
        &[
            "reserved_delivery",
            "float_days",
            "delay_cost_usd_per_day",
            "milestone",
        ],
        as_of,
    )?;
    let review = date(&order, "review")?;
    let expiry = date(&order, "expires")?;
    let delivery = date(&schedule, "reserved_delivery")?;
    if get(&order, "entity")? != get(&fund, "entity")? || review <= as_of || delivery < review {
        return Err(invalid(
            "require matching paying entity, future review, and delivery on or after review",
        ));
    }
    let price = qty(&order, "price_usd")?;
    let deposit = qty(&order, "deposit_usd")?;
    let cancel = qty(&order, "total_cancel_usd")?;
    if deposit > price {
        return Err(invalid("credited deposit exceeds order price"));
    }
    let now = qty(&fund, "decision_cash_usd")?;
    let later = qty(&fund, "review_cash_usd")?;
    let now_floor = qty(&fund, "decision_floor_usd")?;
    let later_floor = qty(&fund, "review_floor_usd")?;
    let other = qty(&fund, "other_due_usd")?;
    let float = qty(&schedule, "float_days")?;
    let delay_rate = qty(&schedule, "delay_cost_usd_per_day")?;
    let decision_room = headroom(now, 0, now_floor)?;
    let review_room = headroom(later, other, later_floor)?;
    let last_day = i128::from(day(delivery)?) + float;
    // Worksheet dates have four-digit years. An unusually large float permits every
    // representable future delivery; do not wrap or invent a calendar deadline.
    let latest_delivery = if last_day <= i128::from(day("9999-12-31")?) {
        let d = CivilDate::from_epoch_days(
            i64::try_from(last_day).map_err(|_| invalid("delivery calendar overflow"))?,
        );
        Some(format!("{:04}-{:02}-{:02}", d.year, d.month, d.day))
    } else {
        None
    };
    let mut limits = PurchaseLimits {
        replacement_price: (decision_room >= 0 && review_room >= 0).then_some(review_room),
        latest_delivery,
        hold_credit: None,
        hold_fee: None,
        explicit_exercise_price: None,
    };
    let mut c = Card::new(&[
        "Choice / outcome",
        "Pay now USD",
        "Net due at review USD",
        "Total supplier USD",
        "Milestone slip days",
        "Delay exposure USD",
        "Cash slack now USD",
        "Cash slack at review USD",
        "Slack with competing project USD",
    ]);
    c.findings.push(format!("Order deadline {expiry}; engineering review {review}; milestone {}. Cash belongs to {}. Competing project {} needs {other} USD at review.",get(&schedule,"milestone")?,get(&fund,"entity")?,get(&fund,"other_project")?));
    let timing = limits.latest_delivery.as_ref().map_or_else(
        || format!("the {float} days of float extend beyond the worksheet calendar"),
        |date| format!("delivery must be on or before {date}"),
    );
    c.findings.push(match limits.replacement_price {
        Some(cap) => format!("Replacement quote: at most {cap} USD payable at review and {timing}. Both conditions must hold."),
        None => format!("Even a free replacement at review cannot preserve both treasury floors and the competing project. For the schedule, {timing}."),
    });
    let mut paths = Vec::new();
    if as_of <= expiry {
        c.journal_actions.push(("order-now", "Place the order"));
        paths.push(("Order / proceed", deposit, price, Some(0)));
        paths.push(("Order / cancel", deposit, cancel, None));
    } else {
        c.findings
            .push("Order unavailable: quoted order deadline passed.".into());
    }
    if order.contains_key("preserves_slot_through_expiry")
        && flag(&order, "preserves_slot_through_expiry")?
    {
        if review <= expiry {
            c.journal_actions.push((
                "defer-order",
                "Decide on the firm quote at engineering review",
            ));
            paths.push(("Defer order / proceed", 0, price, Some(0)));
            paths.push(("Defer order / cancel", 0, 0, None));
            c.findings.push(format!(
                "The supplier guarantees the quoted price and reserved delivery through {expiry}. At review on {review}, order for {price} USD or decline at zero supplier cost."
            ));
        } else {
            c.findings.push(
                "Deferred order unavailable: the guaranteed quote and delivery slot expire before engineering review."
                    .into(),
            );
        }
    }
    if doc.rows_of("hold").next().is_some() {
        let hold = row_fields_with_optional(
            table(doc, "hold")?,
            &[
                "accept_by",
                "valid_through",
                "fee_usd",
                "credit_on_purchase_usd",
            ],
            &["exercise_price_usd"],
            as_of,
        )?;
        let fee = qty(&hold, "fee_usd")?;
        let credit = qty(&hold, "credit_on_purchase_usd")?;
        let explicit_exercise_price = hold
            .contains_key("exercise_price_usd")
            .then(|| qty(&hold, "exercise_price_usd"))
            .transpose()?;
        // Original worksheets declare that the hold preserves the order price.
        let exercise_price = explicit_exercise_price.unwrap_or(price);
        let accept = date(&hold, "accept_by")?;
        let through = date(&hold, "valid_through")?;
        if credit > fee || credit > exercise_price || through < accept {
            return Err(invalid("invalid slot hold credit or date order"));
        }
        if accept >= as_of && through >= review {
            c.journal_actions
                .push(("hold-slot", "Pay to hold the slot"));
            // On exercise the fee and the net purchase price are due across the two dates.
            // Cancellation costs the fee alone. The net purchase price is nonnegative.
            let net_exercise_price = exercise_price - credit;
            let exercise_total = fee + net_exercise_price;
            let cap = decision_room.min(review_room - net_exercise_price);
            limits.explicit_exercise_price = explicit_exercise_price;
            limits.hold_credit = Some(credit);
            limits.hold_fee = (cap >= credit).then_some(cap);
            if let Some(exercise_price) = explicit_exercise_price {
                c.findings.push(format!(
                    "The hold fixes an exercise price of {exercise_price} USD before its {credit} USD credit."
                ));
            }
            c.findings.push(match limits.hold_fee {
                Some(max_fee) => format!("Maximum affordable hold fee: {max_fee} USD with the exercise credit fixed at {credit} USD. This preserves both treasury floors and the competing project on exercise or cancellation."),
                None => format!("No hold fee consistent with the fixed {credit} USD exercise credit preserves both treasury floors and the competing project."),
            });
            paths.push(("Hold / proceed", fee, exercise_total, Some(0)));
            paths.push(("Hold / cancel", fee, fee, None));
            c.findings.push(format!(
                "Accept hold by {accept}; exercise or release at {review}."
            ));
            if as_of <= expiry {
                let extra = exercise_total - price;
                let saving = cancel - fee;
                let comparison = if extra == 0 && saving == 0 {
                    "Equal supplier cost in both outcomes".into()
                } else if extra <= 0 && saving >= 0 {
                    "Holding costs no more in either outcome".into()
                } else if extra >= 0 && saving < 0 {
                    "Ordering costs no more in either outcome".into()
                } else {
                    let (numerator, denominator, direction) = if extra < 0 {
                        (-extra, -extra - saving, "below")
                    } else {
                        (extra, extra + saving, "above")
                    };
                    let p = ExactQty::new(numerator, denominator)
                        .map_err(|_| invalid("break-even overflow"))?;
                    format!(
                        "Break-even cancellation probability {}/{}; holding costs no more at or {direction} this threshold",
                        p.num(),
                        p.den()
                    )
                };
                c.findings.push(format!("{comparison}. Comparison uses undiscounted supplier costs before liquidity checks."));
            }
        } else {
            c.findings.push("Hold unavailable: acceptance deadline passed or the slot expires before the review.".into());
        }
    } else {
        c.findings
            .push("Hold unavailable: no documented supplier option.".into());
    }
    c.journal_actions.push(("let-lapse", "Let the quote lapse"));
    paths.push(("Lapse / cancel", 0, 0, None));
    if doc.rows_of("wait").next().is_some() {
        let wait = row_fields(
            table(doc, "wait")?,
            &["delivery", "price_usd", "valid_through"],
            as_of,
        )?;
        let arrival = date(&wait, "delivery")?;
        let replacement_price = qty(&wait, "price_usd")?;
        if arrival < review {
            return Err(invalid("unreserved delivery precedes review"));
        }
        if date(&wait, "valid_through")? >= review {
            let slip = (i128::from(day(arrival)?) - last_day).max(0);
            if let Some(cap) = limits.replacement_price {
                let cut = (replacement_price - cap).max(0);
                let mut concessions = Vec::new();
                if cut > 0 {
                    concessions.push(format!("reduce the price by at least {cut} USD"));
                }
                if slip > 0 {
                    let unit = if slip == 1 { "day" } else { "days" };
                    concessions.push(format!("bring delivery forward by at least {slip} {unit}"));
                }
                if !concessions.is_empty() {
                    c.findings.push(format!(
                        "For the current replacement offer to qualify, {}.",
                        concessions.join(" and ")
                    ));
                }
            }
            paths.push(("Lapse / proceed", 0, replacement_price, Some(slip)));
        } else {
            c.findings
                .push("Lapse / proceed unpriced: replacement quote expires before review.".into());
        }
    } else {
        c.findings
            .push("Lapse / proceed unpriced: replacement price or delivery is unknown.".into());
    }
    let mut qualified = Vec::new();
    for (proceed, cancel_label) in [
        ("Order / proceed", "Order / cancel"),
        ("Defer order / proceed", "Defer order / cancel"),
        ("Hold / proceed", "Hold / cancel"),
        ("Lapse / proceed", "Lapse / cancel"),
    ] {
        if let (Some(&(_, upfront, cost, Some(0))), Some(&(_, _, cancelled, None))) = (
            paths.iter().find(|p| p.0 == proceed),
            paths.iter().find(|p| p.0 == cancel_label),
        ) && headroom(now, upfront, now_floor)? >= 0
            && headroom(later, cost.max(cancelled), later_floor)? >= other
        {
            qualified.push((
                proceed.split_once(" / ").map_or(proceed, |v| v.0),
                upfront,
                cancelled,
            ));
        }
    }
    let first = match qualified.iter().map(|v|v.1).min() {
        Some(minimum) => format!("Least cash due today among on-time alternatives that preserve both cash floors and the competing project in both outcomes: {} ({minimum} USD).", qualified.iter().filter(|v|v.1==minimum).map(|v|v.0).collect::<Vec<_>>().join(", ")),
        None => "No fully priced on-time alternative preserves both floors and the competing project in both outcomes.".into(),
    };
    c.findings.insert(0, first);
    if let Some(loss) = qualified.iter().map(|v| v.2).min() {
        let choices = qualified
            .iter()
            .filter(|v| v.2 == loss)
            .map(|v| v.0)
            .collect::<Vec<_>>()
            .join(", ");
        c.findings.insert(1,format!("Smallest total loss if cancelled among the same eligible choices: {choices} ({loss} USD)."));
    }
    for (label, upfront, total, slip) in paths {
        let initial = headroom(now, upfront, now_floor)?;
        let closing = headroom(later, total, later_floor)?;
        let joint = closing - other;
        if initial >= 0 && closing >= 0 && other <= later - later_floor && joint < 0 {
            c.findings.push(format!(
                "{label} displaces {} at review by {} USD.",
                get(&fund, "other_project")?,
                -joint
            ));
        }
        c.rows.push(vec![
            label.into(),
            upfront.to_string(),
            (total - upfront).to_string(),
            total.to_string(),
            slip.map_or_else(|| "not applicable".into(), |days| days.to_string()),
            slip.map_or_else(
                || "not applicable".into(),
                |days| (days * delay_rate).to_string(),
            ),
            initial.to_string(),
            closing.to_string(),
            joint.to_string(),
        ]);
    }
    c.detail = Some(("procurement_limits", limits.json()));
    Ok(c)
}

fn distinct(seen: &mut BTreeSet<String>, id: &str) -> Result<()> {
    if !seen.insert(id.into()) {
        return Err(invalid(format!(
            "duplicate decision alternative or site {id}"
        )));
    }
    Ok(())
}

fn closure_cash(through: &str, mut flows: [Option<(&str, i128)>; 3]) -> (i128, i128) {
    // ISO dates sort in calendar order. At a common date reserve debits before receipts.
    flows.sort_unstable();
    let mut net = 0;
    let mut peak_needed = 0;
    for (_, change) in flows
        .into_iter()
        .flatten()
        .take_while(|(date, _)| *date <= through)
    {
        net += change;
        peak_needed = peak_needed.max(-net);
    }
    (net, peak_needed)
}

fn rehabilitation(doc: &TomlDoc, as_of: &str) -> Result<Card> {
    tables(doc, &["horizon", "package"])?;
    let h = row_fields_with_optional(
        table(doc, "horizon")?,
        &["entity", "through"],
        &["cash_headroom_usd"],
        as_of,
    )?;
    let through = date(&h, "through")?;
    if through < as_of {
        return Err(invalid("comparison horizon precedes decision"));
    }
    let cash_headroom = h
        .contains_key("cash_headroom_usd")
        .then(|| qty(&h, "cash_headroom_usd"))
        .transpose()?;
    let mut headers = vec![
        "Package",
        "Eligibility",
        "Work USD",
        "Earliest release",
        "Net cash USD by horizon",
        "Net credit line USD",
        "Net surety USD",
    ];
    if cash_headroom.is_some() {
        headers.extend(["Cash needed USD", "Funding through horizon"]);
    }
    let mut c = Card::new(&headers);
    let mut seen = BTreeSet::new();
    let mut best: Option<i128> = None;
    let mut leaders = Vec::new();
    for row in doc.rows_of("package") {
        let f = row_fields_with_optional(
            row,
            &[
                "id",
                "entity",
                "eligible",
                "acceptance_condition",
                "work_date",
                "release_date",
                "work_usd",
                "released_cash_usd",
                "reposted_cash_usd",
                "released_line_usd",
                "reposted_line_usd",
                "released_surety_usd",
                "reposted_surety_usd",
                "cash_fee_saving_usd",
            ],
            &["cash_repost_date"],
            as_of,
        )?;
        let id = get(&f, "id")?;
        distinct(&mut seen, id)?;
        let work = date(&f, "work_date")?;
        let release = get(&f, "release_date")?;
        if get(&f, "entity")? != get(&h, "entity")?
            || work < as_of
            || (release != "pending" && date(&f, "release_date")? < work)
        {
            return Err(invalid(
                "rehabilitation requires one entity and prospective work before release",
            ));
        }
        let cash_repost_date = f
            .contains_key("cash_repost_date")
            .then(|| date(&f, "cash_repost_date"))
            .transpose()?;
        if cash_repost_date.is_some_and(|posting| cash_headroom.is_none() || posting < as_of) {
            return Err(invalid(
                "cash_repost_date requires cash_headroom_usd and a date on or after the decision",
            ));
        }
        let cost = qty(&f, "work_usd")?;
        let repost = qty(&f, "reposted_cash_usd")?;
        let release_cash = qty(&f, "released_cash_usd")? + qty(&f, "cash_fee_saving_usd")?
            - if cash_repost_date.is_none() {
                repost
            } else {
                0
            };
        let line = qty(&f, "released_line_usd")? - qty(&f, "reposted_line_usd")?;
        let surety = qty(&f, "released_surety_usd")? - qty(&f, "reposted_surety_usd")?;
        let eligible = flag(&f, "eligible")?;
        let mut r = vec![
            id.into(),
            if eligible {
                "eligible".into()
            } else {
                "ineligible".into()
            },
            cost.to_string(),
            release.into(),
        ];
        let timed_release = (release != "pending").then_some(release);
        let quantified = eligible && (timed_release.is_some() || work > through);
        let released = i128::from(timed_release.is_some_and(|date| date <= through));
        let (net, peak_needed) = closure_cash(
            through,
            [
                Some((work, -cost)),
                cash_repost_date.map(|date| (date, -repost)),
                timed_release.map(|date| (date, release_cash)),
            ],
        );
        if quantified {
            r.extend([
                net.to_string(),
                (released * line).to_string(),
                (released * surety).to_string(),
            ]);
        } else {
            r.extend([
                "unresolved".into(),
                "unresolved".into(),
                "unresolved".into(),
            ]);
        }
        if let Some(headroom) = cash_headroom {
            let funding = if !eligible {
                "ineligible".into()
            } else if !quantified {
                "pending release".into()
            } else if peak_needed > headroom {
                format!("short by {} USD", peak_needed - headroom)
            } else {
                "funded".into()
            };
            r.extend([
                if eligible {
                    peak_needed.to_string()
                } else {
                    "unresolved".into()
                },
                funding,
            ]);
        }
        if quantified && cash_headroom.is_none_or(|headroom| peak_needed <= headroom) {
            if best.is_none_or(|b| net > b) {
                best = Some(net);
                leaders.clear();
            }
            if best == Some(net) {
                leaders.push(id.to_string());
            }
        }
        c.rows.push(r);
        c.findings.push(format!(
            "{id}: acceptance requires {}.",
            get(&f, "acceptance_condition")?
        ));
    }
    if seen.len() < 2 {
        return Err(invalid("compare at least two distinct packages"));
    }
    if let Some(headroom) = cash_headroom {
        c.findings.push(format!("Cash headroom above the floor: {headroom} USD. Reserve same-day debits before receipts. Cash needed for a pending package uses only dated debits and assumes no pending receipt."));
        c.findings.push(match best {
            Some(net) => format!("Highest quantified net cash among funded packages through {through}: {} ({net} USD).", leaders.join(", ")),
            None => format!("No eligible package with resolved cash timing fits the declared headroom through {through}."),
        });
    } else if let Some(net) = best {
        c.findings.push(format!("Highest quantified net cash through {through}: {} ({net} USD). Pending releases remain unresolved. Credit and surety are separate.",leaders.join(", ")));
    }
    Ok(c)
}

fn fraction(n: i128, d: i128) -> Result<String> {
    let v = ExactQty::new(n, d).map_err(|_| invalid("undefined or overflowing ratio"))?;
    Ok(if v.den() == 1 {
        v.num().to_string()
    } else {
        format!("{}/{}", v.num(), v.den())
    })
}

fn dollars(amount: i128) -> String {
    let digits = amount.unsigned_abs().to_string();
    let mut display = String::from(if amount < 0 { "-US$" } else { "US$" });
    for (i, digit) in digits.bytes().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            display.push(',');
        }
        display.push(char::from(digit));
    }
    display
}

fn tenure(doc: &TomlDoc, as_of: &str) -> Result<Card> {
    tables(doc, &["tenement", "alternative"])?;
    let t = row_fields(
        table(doc, "tenement")?,
        &[
            "expiry",
            "linked_rights",
            "option_low_usd",
            "option_high_usd",
            "survey_result",
            "baseline_detection_pct",
            "cover_metres",
            "investigation_metres",
            "sensitive_above_pct",
        ],
        as_of,
    )?;
    let expiry = date(&t, "expiry")?;
    if expiry < as_of {
        return Err(invalid("tenure renewal deadline has passed"));
    }
    let low = qty(&t, "option_low_usd")?;
    let high = qty(&t, "option_high_usd")?;
    let pct = |n| ExactQty::new(n, 100).map_err(|_| invalid("invalid percentage"));
    let threshold = qty(&t, "sensitive_above_pct")?;
    if low > high || threshold > 100 {
        return Err(invalid("invalid option bounds or sensitivity threshold"));
    }
    let q = fmine_explore::absence::effective_detection_q(
        pct(qty(&t, "baseline_detection_pct")?)?,
        ExactQty::from_integer(qty(&t, "cover_metres")?),
        ExactQty::from_integer(qty(&t, "investigation_metres")?),
    )
    .map_err(|_| invalid("invalid survey detection inputs"))?;
    let survey = match get(&t, "survey_result")? {
        "no_detection"
            if q.checked_cmp(pct(threshold)?)
                .map_err(|_| invalid("detection comparison overflow"))?
                .is_gt() =>
        {
            "Sensitive negative within the surveyed footprint"
        }
        "no_detection" => "Survey insufficiently sensitive in the declared cover",
        "unknown" => "Survey result unknown",
        "detection" => "Recorded detection; economic significance unresolved",
        _ => return Err(invalid("unknown survey result")),
    };
    let mut c = Card::new(&[
        "Alternative",
        "Availability",
        "Net value low USD",
        "Net value high USD",
        "Retained share %",
        "Break-even gross option USD including rights loss (low to high)",
    ]);
    c.findings.push(format!("Renewal deadline {expiry}. Linked rights: {}. {survey}; effective detection {}/{} under the declared model.",get(&t,"linked_rights")?,q.num(),q.den()));
    let mut seen = BTreeSet::new();
    let mut intervals = Vec::new();
    for row in doc.rows_of("alternative") {
        let f = row_fields(
            row,
            &[
                "action",
                "available",
                "terms_through",
                "cash_cost_usd",
                "cash_received_usd",
                "retained_pct",
                "linked_loss_low_usd",
                "linked_loss_high_usd",
                "terms",
            ],
            as_of,
        )?;
        let action = get(&f, "action")?;
        distinct(&mut seen, action)?;
        if !matches!(action, "renew" | "farm_out" | "surrender") {
            return Err(invalid("unknown tenure alternative"));
        }
        let cost = qty(&f, "cash_cost_usd")? - qty(&f, "cash_received_usd")?;
        let share = qty(&f, "retained_pct")?;
        let l = qty(&f, "linked_loss_low_usd")?;
        let u = qty(&f, "linked_loss_high_usd")?;
        if share > 100 || l > u || (action == "surrender" && share != 0) {
            return Err(invalid(
                "invalid remaining interest or linked-rights bounds",
            ));
        }
        let terms_through = date(&f, "terms_through")?;
        let available = flag(&f, "available")? && terms_through >= as_of;
        let a = low * share - 100 * (cost + u);
        let b = high * share - 100 * (cost + l);
        if available {
            intervals.push((action, a, b));
        }
        let mut r = vec![
            action.into(),
            if available {
                "available"
            } else {
                "unavailable"
            }
            .into(),
        ];
        if available {
            r.extend([
                fraction(a, 100)?,
                fraction(b, 100)?,
                share.to_string(),
                if share == 0 {
                    "no retained interest".into()
                } else {
                    format!(
                        "{} to {}",
                        fraction((cost + l) * 100, share)?,
                        fraction((cost + u) * 100, share)?
                    )
                },
            ]);
        } else {
            r.extend(["—".into(), "—".into(), "—".into(), "—".into()]);
        }
        c.rows.push(r);
        c.findings.push(format!(
            "{action}: {}. Terms through {}.",
            get(&f, "terms")?,
            date(&f, "terms_through")?
        ));
    }
    if seen.len() != 3 || intervals.is_empty() {
        return Err(invalid(
            "declare renew, farm_out and surrender, with at least one available",
        ));
    }
    for (candidate, _, upper) in &intervals {
        if let Some((better, _, _)) = intervals
            .iter()
            .find(|(id, lower, _)| id != candidate && lower > upper)
        {
            c.findings.push(format!(
                "{candidate} is dominated by {better} over the supplied economic ranges."
            ));
        }
    }
    let robust = intervals.iter().find(|(id, lo, _)| {
        intervals
            .iter()
            .all(|(other, _, hi)| id == other || lo > hi)
    });
    c.findings.push(robust.map_or_else(
        || "The available value ranges overlap; no uniquely separated choice.".into(),
        |(id, _, _)| {
            format!("Only feasible or strictly separated choice across declared ranges: {id}.")
        },
    ));
    Ok(c)
}

struct SiteUplift<'a> {
    id: &'a str,
    act_by: &'a str,
    interest_bp: i128,
    project_units: i128,
}

fn maximum_shared_uplift(
    prioritized_sites: &[SiteUplift<'_>],
    mut project_capacity: i128,
    excluded: Option<&str>,
) -> i128 {
    // Every incremental project unit uses one unit of capacity. Moving a unit
    // from a lower interest to an available higher interest cannot reduce the maximum.
    let mut attributable = 0;
    for site in prioritized_sites {
        if excluded == Some(site.id) {
            continue;
        }
        let admitted = site.project_units.min(project_capacity);
        project_capacity -= admitted;
        attributable += admitted * site.interest_bp;
    }
    attributable
}

fn group_recovery(doc: &TomlDoc, as_of: &str) -> Result<Card> {
    tables(doc, &["group", "site"])?;
    let g = row_fields_with_optional(
        table(doc, "group")?,
        &[
            "commodity",
            "unit",
            "through",
            "target",
            "common_capacity_basis",
        ],
        &["shared_incremental_project_capacity"],
        as_of,
    )?;
    let through = date(&g, "through")?;
    if through <= as_of {
        return Err(invalid("group recovery needs a remaining future interval"));
    }
    let target = qty(&g, "target")? * 10000;
    let shared_capacity = g
        .contains_key("shared_incremental_project_capacity")
        .then(|| qty(&g, "shared_incremental_project_capacity"))
        .transpose()?;
    let site_ceiling = if shared_capacity.is_some() {
        "Site maximum attributable before shared limit"
    } else {
        "Admissible maximum attributable"
    };
    let mut c = Card::new(&[
        "Site",
        "Interest bp",
        "Allocated target attributable",
        "Current plan attributable",
        site_ceiling,
        "Decision by",
        "Site constraint",
    ]);
    let (mut goals, mut plan) = (0, 0);
    let mut offsets = Vec::new();
    let mut seen = BTreeSet::new();
    for row in doc.rows_of("site") {
        let f = row_fields(
            row,
            &[
                "id",
                "commodity",
                "unit",
                "through",
                "basis",
                "interest_bp",
                "target",
                "plan",
                "maximum",
                "constraint",
                "act_by",
            ],
            as_of,
        )?;
        let id = get(&f, "id")?;
        distinct(&mut seen, id)?;
        let share = qty(&f, "interest_bp")?;
        if share == 0
            || share > 10000
            || get(&f, "basis")? != "100_percent_project"
            || get(&f, "commodity")? != get(&g, "commodity")?
            || get(&f, "unit")? != get(&g, "unit")?
            || date(&f, "through")? != through
        {
            return Err(invalid(
                "site ownership, commodity, unit and remaining period must match the declared scope",
            ));
        }
        let goal = qty(&f, "target")? * share;
        let base = qty(&f, "plan")?;
        let max = qty(&f, "maximum")?;
        let by = date(&f, "act_by")?;
        if max < base || by > through {
            return Err(invalid(
                "site maximum below plan or action deadline after the reporting interval",
            ));
        }
        let usable = if by < as_of { base } else { max };
        goals += goal;
        plan += base * share;
        offsets.push(SiteUplift {
            id,
            act_by: by,
            interest_bp: share,
            project_units: usable - base,
        });
        c.rows.push(vec![
            id.into(),
            share.to_string(),
            fraction(goal, 10000)?,
            fraction(base * share, 10000)?,
            fraction(usable * share, 10000)?,
            by.into(),
            get(&f, "constraint")?.into(),
        ]);
    }
    if seen.len() < 2 || target != goals {
        return Err(invalid(
            "group target must equal the attributable allocations across at least two distinct sites",
        ));
    }
    let additional = if let Some(limit) = shared_capacity {
        offsets.sort_unstable_by_key(|site| (Reverse(site.interest_bp), site.id));
        maximum_shared_uplift(&offsets, limit, None)
    } else {
        offsets
            .iter()
            .map(|site| site.project_units * site.interest_bp)
            .sum()
    };
    let capacity = plan + additional;
    c.findings.push(format!("{} {} attributable through {through}. Group target {}; current plan {}; remaining net increase needed {}; maximum additional output before deadlines {}.",get(&g,"commodity")?,get(&g,"unit")?,fraction(target,10000)?,fraction(plan,10000)?,fraction((target-plan).max(0),10000)?,fraction(capacity-plan,10000)?));
    if let Some(limit) = shared_capacity {
        c.findings.push(format!(
            "Shared limit above existing site plans: {limit} {} on a 100%-project basis.",
            get(&g, "unit")?
        ));
    }
    c.findings.push(if capacity<target {format!("Even all admitted offsets leave a group shortfall of {}.",fraction(target-capacity,10000)?)} else {"Declared site bounds allow enough aggregate output. This is conditional on the stated simultaneous capacity basis.".into()});
    if capacity >= target {
        for site in &offsets {
            let SiteUplift {
                id,
                act_by,
                interest_bp,
                project_units,
            } = site;
            if let Some(limit) = shared_capacity {
                let without = plan + maximum_shared_uplift(&offsets, limit, Some(id));
                if without < target {
                    c.findings.push(format!(
                        "{id} is required. Without its uplift, the attributable maximum is {} and the group is short by {}. Decision deadline {act_by}.",
                        fraction(without, 10000)?,
                        fraction(target - without, 10000)?
                    ));
                }
                continue;
            }
            let indispensable = target - (capacity - project_units * interest_bp);
            if indispensable > 0 {
                c.findings.push(format!("{id} must supply at least {} above its baseline even if all other sites reach their bounds. Decision deadline {act_by}.", fraction(indispensable,10000)?));
            }
        }
    }
    c.findings.push(format!(
        "Simultaneous capacity basis: {}.",
        get(&g, "common_capacity_basis")?
    ));
    Ok(c)
}

fn project_estimates(doc: &TomlDoc, as_of: &str) -> Result<Card> {
    tables(doc, &["cohort", "project"])?;
    let cohort = row_fields(
        table(doc, "cohort")?,
        &["holdout", "comparable_scope", "price_basis"],
        as_of,
    )?;
    let held = get(&cohort, "holdout")?;
    let mut seen = BTreeSet::new();
    let mut records = Vec::new();
    for row in doc.rows_of("project") {
        let f = row_fields(
            row,
            &[
                "id",
                "approved_on",
                "approved_usd",
                "status",
                "comparable_actual_usd",
                "scope_bridge",
            ],
            as_of,
        )?;
        let id = get(&f, "id")?;
        distinct(&mut seen, id)?;
        let estimate = qty(&f, "approved_usd")?;
        let approved = date(&f, "approved_on")?;
        let known = date(&f, "known_on")?;
        let status = get(&f, "status")?;
        if estimate == 0 || approved > known {
            return Err(invalid(
                "approval must have a positive baseline and precede source availability",
            ));
        }
        let actual = match status {
            "completed" if known > approved => Some(qty(&f, "comparable_actual_usd")?),
            "cancelled" | "scope_changed" | "unresolved"
                if get(&f, "comparable_actual_usd")? == "unknown" =>
            {
                None
            }
            _ => {
                return Err(invalid(
                    "completed outcomes need a subsequent comparable actual; cancelled, changed or unresolved records require unknown",
                ));
            }
        };
        records.push((
            id,
            approved,
            known,
            estimate,
            actual,
            status,
            get(&f, "scope_bridge")?,
        ));
    }
    let &(_, cut, _, estimate, observed, _, _) = records
        .iter()
        .find(|r| r.0 == held)
        .ok_or_else(|| invalid("holdout absent from cohort"))?;
    if date(&cohort, "known_on")? > cut {
        return Err(invalid(
            "cohort definition was unavailable at holdout approval",
        ));
    }
    if records.iter().any(|r| r.1 > cut) {
        return Err(invalid(
            "holdout must be a latest decision in the supplied cohort",
        ));
    }
    let mut c = Card::new(&[
        "Project",
        "Approved",
        "Outcome available",
        "Approved USD",
        "Comparable actual USD",
        "Status",
        "Use",
        "Scope bridge",
    ]);
    let mut ratios = Vec::new();
    for (id, approved, known, base, actual, status, bridge) in &records {
        let usage = if *id == held {
            "holdout"
        } else if *known >= cut {
            "unavailable before holdout"
        } else if let Some(a) = actual {
            ratios.push((*a, *base));
            "comparable completed"
        } else {
            status
        };
        c.rows.push(vec![
            id.to_string(),
            approved.to_string(),
            known.to_string(),
            base.to_string(),
            actual.map_or_else(|| "unknown".into(), |x| x.to_string()),
            status.to_string(),
            usage.to_string(),
            bridge.to_string(),
        ]);
    }
    c.findings.push(format!("{} approved projects; {} comparable outcomes available strictly before {cut}. Holdout {held}. Price basis {}. Scope {}.",records.len(),ratios.len(),get(&cohort,"price_basis")?,get(&cohort,"comparable_scope")?));
    if ratios.is_empty() {
        c.findings.push("Comparable adjustment unavailable.".into());
    } else {
        // Use the empirical lower median ratio for both odd and even sample sizes.
        ratios.sort_by(|(a, b), (x, y)| (a * y).cmp(&(x * b)));
        let (n, d) = ratios[(ratios.len() - 1) / 2];
        c.findings.push(format!(
            "Lower median actual / approval {}. Holdout original {estimate} USD; adjusted {} USD.",
            fraction(n, d)?,
            fraction(estimate * n, d)?
        ));
        if let Some(actual) = observed {
            let original = (estimate - actual).abs();
            let corrected = (estimate * n - actual * d).abs();
            c.findings.push(format!("Held-out absolute error: original {original} USD; adjusted {} USD. One comparison, uncalibrated.",fraction(corrected,d)?));
        } else {
            c.findings
                .push("Held-out realized error unavailable.".into());
        }
    }
    Ok(c)
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use fmine_registry::toml_subset::parse_subset;
    const PURCHASE: &str = include_str!("../../../../tests/capital/executive_purchase.toml");
    const LAND: &str = include_str!("../../../../tests/capital/executive_tenure.toml");
    const CLOSURE: &str = include_str!("../../../../tests/capital/executive_rehabilitation.toml");
    const GROUP: &str = include_str!("../../../../tests/capital/executive_group.toml");
    const ESTIMATES: &str = include_str!("../../../../tests/capital/executive_estimates.toml");
    const HEDGE: &str = include_str!("../../../../tests/capital/executive_hedge.toml");
    fn evaluate(text: &str) -> Result<Envelope> {
        let doc = parse_subset(text).expect("control uses valid TOML");
        let source = SourceAnchor::new(
            "synthetic.toml",
            fmine_core::hash::sha256(text.as_bytes()).0,
            "worksheet",
        );
        worksheet(&doc, text, &source, std::path::Path::new("."), false)
    }
    fn report(s: &str) -> String {
        evaluate(s)
            .expect("admitted")
            .rendered_markdown
            .expect("markdown")
    }

    #[test]
    fn shipment_deadline_moves_only_with_usable_cash_and_the_actual_collection_cutoff() {
        let data = |s: &str| evaluate(s).expect("valid cash window").payload_json;
        let early = HEDGE.replace("after_calls", "before_calls");
        assert!(data(&early).contains("\"latest_collection_date\":\"2030-03-06\""));
        let at_limit = HEDGE.replace(
            "committed_undrawn_usd = \"3000000\"",
            "committed_undrawn_usd = \"7000000\"",
        );
        assert!(
            data(&at_limit)
                .contains("\"latest_collection_status\":\"receipt_not_needed_within_window\"")
        );
        assert!(
            data(&at_limit.replace("7000000", "6999999")).contains("\"additional_line_usd\":\"1\"")
        );
        assert!(
            data(&HEDGE.replace("11000000", "0"))
                .contains("\"latest_collection_status\":\"no_feasible_date_in_window\"")
        );
        let outside = HEDGE.replace(
            "delayed_collection = \"2030-03-08\"",
            "delayed_collection = \"2030-03-19\"",
        );
        assert!(data(&outside).contains("\"closing_cash_before_financing_usd\":\"4000000\""));
        let expires = at_limit.replace("2030-03-31", "2030-03-06");
        assert!(data(&expires).contains("\"first_shortfall\":{\"date\":\"2030-03-07\",\"phase\":\"open\",\"additional_usd\":\"4000000\"}"));
        let opening = HEDGE.replace("12000000", "4000000");
        assert!(
            data(&opening)
                .contains("\"first_shortfall\":{\"date\":\"2030-03-04\",\"phase\":\"opening\"")
        );
        assert!(
            data(&opening).contains("\"latest_collection_status\":\"no_feasible_date_in_window\"")
        );
    }

    #[test]
    fn hedge_cash_windows_refuse_missing_days_unavailable_sources_and_mixed_entities() {
        for invalid in [
            HEDGE.replace("date = \"2030-03-06\"", "date = \"2030-03-05\""),
            HEDGE.replace("through = \"2030-03-08\"", "through = \"2030-03-09\""),
            HEDGE.replacen("entity = \"Parent\"", "entity = \"JV\"", 1),
            HEDGE.replace("known_on = \"2030-03-04\"", "known_on = \"2030-03-09\""),
            HEDGE.replace("after_calls", "unknown"),
            HEDGE.replace(
                "line_through = \"2030-03-31\"",
                "line_through = \"2030-02-28\"",
            ),
            HEDGE.replace(
                "delayed_collection = \"2030-03-08\"",
                "delayed_collection = \"2030-03-04\"",
            ),
            HEDGE.replace(
                "broker_call_usd = \"5000000\"",
                "broker_call_usd = \"-5000000\"",
            ),
        ] {
            assert!(evaluate(&invalid).is_err());
        }
    }

    #[test]
    fn advancing_part_of_the_delayed_invoice_is_sufficient_and_a_dollar_less_fails() {
        let split = |early: u32| {
            let text = HEDGE
                .replace("11000000", &(11_000_000 - early).to_string())
                .replacen(
                    "other_receipt_usd = \"0\"",
                    &format!("other_receipt_usd = \"{early}\""),
                    1,
                );
            let json = evaluate(&text).expect("valid split").payload_json;
            let parsed = fmine_io::omf::json::parse(json.as_bytes()).expect("JSON");
            parsed
                .get("liquidity")
                .expect("summary")
                .get("delayed")
                .expect("late case")
                .clone()
        };
        let covered = split(4_000_000);
        let short = split(3_999_999);
        for case in [&covered, &short] {
            assert_eq!(
                case.get("closing_cash_before_financing_usd")
                    .and_then(|v| v.as_str()),
                Some("15000000")
            );
        }
        assert_eq!(
            covered.get("additional_line_usd").and_then(|v| v.as_str()),
            Some("0")
        );
        assert_eq!(
            short.get("additional_line_usd").and_then(|v| v.as_str()),
            Some("1")
        );
        let large_line = HEDGE.replace(
            "committed_undrawn_usd = \"3000000\"",
            "committed_undrawn_usd = \"7000000\"",
        );
        assert!(
            evaluate(&large_line)
                .expect("covered")
                .payload_json
                .contains("\"status\":\"not_needed\",\"amount_usd\":\"0\"")
        );
        assert!(
            evaluate(&HEDGE.replace("11000000", "3999999"))
                .expect("undersized invoice")
                .payload_json
                .contains("\"status\":\"insufficient_even_in_full\"")
        );
    }

    #[test]
    fn purchase_requires_firm_terms_available_by_the_meeting() {
        let expired = PURCHASE.replace(
            "valid_through = \"2030-03-22\"",
            "valid_through = \"2030-03-20\"",
        );
        let shown = report(&expired);
        assert!(shown.contains("Hold unavailable") && shown.contains("Lapse / proceed unpriced"));
        assert!(!shown.contains("| Hold / proceed") && !shown.contains("probability 1/20"));
        let future = PURCHASE.replace("known_on = \"2030-03-07\"", "known_on = \"2030-03-10\"");
        assert!(evaluate(&future).is_err());
        let other = PURCHASE.replacen("entity = \"Parent\"", "entity = \"Ring-fenced JV\"", 1);
        assert!(evaluate(&other).is_err());
        for key in [
            "credit_on_purchase_usd = \"300000\"",
            "credit_on_purchase_usd = \"-1\"",
            "credit_on_purchase_usd = \"0.5\"",
        ] {
            assert!(evaluate(&PURCHASE.replace("credit_on_purchase_usd = \"0\"", key)).is_err());
        }
    }

    #[test]
    fn guaranteed_slot_through_review_can_avoid_both_deposit_and_hold_fee() {
        let expiry = r#"expires = "2030-03-22""#;
        let price_only = PURCHASE.replace(r#"expires = "2030-03-08""#, expiry);
        let firm = price_only.replace(
            expiry,
            &format!("{expiry}\npreserves_slot_through_expiry = \"yes\""),
        );
        let md = report(&firm);
        assert!(md.contains("Defer order (0 USD)"));
        assert!(md.contains(
            "Smallest total loss if cancelled among the same eligible choices: Defer order (0 USD)."
        ));
        assert!(md.contains("| Defer order / proceed | 0 | 20000000 | 20000000 | 0 | 0 | 1500000 | 7000000 | 500000 |"));
        assert!(md.contains("| Defer order / cancel | 0 | 0 | 0 | not applicable | not applicable | 1500000 | 27000000 | 20500000 |"));
        assert!(md.contains("| Lapse / proceed | 0 | 21500000 | 21500000 | 18 |"));

        for unguaranteed in [
            price_only,
            firm.replace("expiry = \"yes\"", "expiry = \"no\""),
            firm.replace(expiry, r#"expires = "2030-03-21""#),
        ] {
            let md = report(&unguaranteed);
            assert!(!md.contains("| Defer order /"));
            assert!(md.contains("Hold (250000 USD)"));
        }
        assert!(evaluate(&firm.replace("expiry = \"yes\"", "expiry = \"maybe\"")).is_err());

        let exactly_funded = firm.replace("45000000", "44500000");
        assert!(report(&exactly_funded).contains("Defer order (0 USD)"));
        let short = report(&exactly_funded.replace("44500000", "44499999"));
        assert!(short.contains("No fully priced on-time alternative"));
        assert!(short.contains(
            "Defer order / proceed displaces North Ridge substation at review by 1 USD."
        ));
        let below_opening_floor = report(&firm.replace("19500000", "17999999"));
        assert!(below_opening_floor.contains("No fully priced on-time alternative"));
    }

    #[test]
    fn purchase_accepts_absent_hold_and_only_charges_elapsed_critical_days() {
        let (start, rest) = PURCHASE.split_once("[[hold]]").expect("hold");
        let (_, next) = rest.split_once("[[wait]]").expect("wait");
        let shown = report(&format!("{start}[[wait]]{next}"));
        assert!(
            shown.contains("no documented supplier option") && !shown.contains("| Hold / proceed")
        );
        let credited = report(&PURCHASE.replace(
            "credit_on_purchase_usd = \"0\"",
            "credit_on_purchase_usd = \"250000\"",
        ));
        assert!(credited.contains("Holding costs no more in either outcome"));
        let refunded = report(&PURCHASE.replace(
            "total_cancel_usd = \"5000000\"",
            "total_cancel_usd = \"1000000\"",
        ));
        assert!(refunded.contains("| Order / cancel | 3000000 | -2000000 | 1000000 |"));
        let floored = report(&PURCHASE.replace("float_days = \"12\"", "float_days = \"40\""));
        assert!(floored.contains("| Lapse / proceed | 0 | 21500000 | 21500000 | 0 | 0 |"));
        assert!(evaluate(&PURCHASE.replace("2030-11-15", "2030-02-30")).is_err());
    }

    #[test]
    fn procurement_limits_match_the_forward_decision_at_and_across_each_boundary() {
        let at_review_cap = PURCHASE.replace("fee_usd = \"250000\"", "fee_usd = \"500000\"");
        assert!(report(&at_review_cap).contains("Hold (500000 USD)"));
        assert!(
            report(&at_review_cap.replace("fee_usd = \"500000\"", "fee_usd = \"500001\""))
                .contains("No fully priced on-time alternative")
        );

        let at_upfront_cap = at_review_cap.replace("19500000", "18500000");
        assert!(report(&at_upfront_cap).contains("Hold (500000 USD)"));
        assert!(
            report(&at_upfront_cap.replace("18500000", "18499999"))
                .contains("Maximum affordable hold fee: 499999 USD")
        );
        assert!(
            report(&at_upfront_cap.replace("18500000", "18499999"))
                .contains("No fully priced on-time alternative")
        );

        let wait = PURCHASE
            .replace("21500000", "20500000")
            .replace("2030-12-15", "2030-11-27");
        assert!(report(&wait).contains("Lapse (0 USD)"));
        for (change, needed) in [
            (
                wait.replace("20500000", "20500001"),
                "reduce the price by at least 1 USD.",
            ),
            (
                wait.replace("2030-11-27", "2030-11-28"),
                "bring delivery forward by at least 1 day.",
            ),
        ] {
            let md = report(&change);
            assert!(
                md.contains("Hold (250000 USD)") && md.contains(needed),
                "{md}"
            );
        }
    }

    #[test]
    fn separately_quoted_hold_price_controls_the_review_payment_and_funding_limit() {
        let at_limit = PURCHASE.replace(
            "credit_on_purchase_usd = \"0\"",
            "credit_on_purchase_usd = \"0\"\nexercise_price_usd = \"20250000\"",
        );
        let admitted = evaluate(&at_limit).expect("firm slot and exercise quote");
        assert!(
            admitted.payload_json.contains(
                "\"max_hold_fee_usd\":\"250000\",\"fixed_exercise_price_usd\":\"20250000\""
            )
        );
        let shown = admitted.rendered_markdown.expect("committee comparison");
        assert!(shown.contains("Hold (250000 USD)"));
        assert!(shown.contains(
            "| Hold / proceed | 250000 | 20250000 | 20500000 | 0 | 0 | 1250000 | 6500000 | 0 |"
        ));
        assert!(shown.contains("probability 2/21"));

        let above_limit = at_limit.replace("20250000", "20250001");
        let shown = report(&above_limit);
        assert!(shown.contains("No fully priced on-time alternative"));
        assert!(shown.contains("Maximum affordable hold fee: 249999 USD"));
        assert!(
            shown.contains("Hold / proceed displaces North Ridge substation at review by 1 USD.")
        );

        let expired =
            above_limit.replace("accept_by = \"2030-03-08\"", "accept_by = \"2030-03-07\"");
        let evaluated = evaluate(&expired).expect("expired terms remain visible");
        assert!(
            !evaluated
                .payload_json
                .contains("\"fixed_exercise_price_usd\"")
        );
        assert!(evaluated.payload_json.contains("Hold unavailable"));
        assert!(!evaluated.payload_json.contains("\"hold-slot\""));
    }

    #[test]
    fn cheaper_exercise_terms_can_reverse_the_break_even_or_dominate() {
        let discounted = PURCHASE.replace(
            "credit_on_purchase_usd = \"0\"",
            "credit_on_purchase_usd = \"0\"\nexercise_price_usd = \"19000000\"",
        );
        assert!(report(&discounted).contains("Holding costs no more in either outcome"));
        let full_refund =
            discounted.replace("total_cancel_usd = \"5000000\"", "total_cancel_usd = \"0\"");
        assert!(
            report(&full_refund).contains("probability 3/4; holding costs no more at or below")
        );

        let admissible_credit = discounted.replace(
            "credit_on_purchase_usd = \"0\"",
            "credit_on_purchase_usd = \"1\"",
        );
        assert!(evaluate(&admissible_credit).is_ok());
        assert!(evaluate(&admissible_credit.replace("19000000", "0")).is_err());
        for malformed in ["-1", "20250000.5", "9223372036854775808"] {
            assert!(evaluate(&discounted.replace("19000000", malformed)).is_err());
        }
    }

    #[test]
    fn procurement_limits_distinguish_impossible_free_and_unquoted_terms() {
        let payload = |s: &str| evaluate(s).expect("valid comparison").payload_json;
        let no_money = PURCHASE.replace("45000000", "24499999");
        assert!(payload(&no_money).contains("\"max_replacement_price_usd\":null"));
        assert!(
            payload(&no_money)
                .contains("\"fixed_exercise_credit_usd\":\"0\",\"max_hold_fee_usd\":null")
        );
        assert!(
            payload(&PURCHASE.replace("19500000", "17999999"))
                .contains("\"max_replacement_price_usd\":null")
        );
        assert!(
            payload(&PURCHASE.replace("45000000", "24500000"))
                .contains("\"max_replacement_price_usd\":\"0\"")
        );
        let credit_too_large = PURCHASE.replace("19500000", "18100000").replace(
            "credit_on_purchase_usd = \"0\"",
            "credit_on_purchase_usd = \"200000\"",
        );
        assert!(
            payload(&credit_too_large)
                .contains("\"fixed_exercise_credit_usd\":\"200000\",\"max_hold_fee_usd\":null")
        );
        let expired = PURCHASE.replace("accept_by = \"2030-03-08\"", "accept_by = \"2030-03-07\"");
        assert!(
            payload(&expired)
                .contains("\"fixed_exercise_credit_usd\":null,\"max_hold_fee_usd\":null")
        );
        let huge = PURCHASE.replace(
            "float_days = \"12\"",
            "float_days = \"9223372036854775807\"",
        );
        assert!(payload(&huge).contains("\"latest_on_time_delivery\":null"));
        let leap = PURCHASE.replace("2030-11-15", "2032-02-17");
        assert!(payload(&leap).contains("\"latest_on_time_delivery\":\"2032-02-29\""));
    }

    #[test]
    fn release_timing_and_eligibility_control_collateral_comparison() {
        let late = report(&CLOSURE.replace("2030-10-31", "2031-01-01"));
        assert!(late.contains("2031-01-01 | -900000 | 0 | 0"));
        let unknown = report(&CLOSURE.replace("2030-10-31", "pending"));
        assert!(unknown.contains("pending | unresolved") && !unknown.contains("(1510000 USD)"));
        let closed = report(&CLOSURE.replacen("eligible = \"yes\"", "eligible = \"no\"", 1));
        assert!(closed.contains("North waste-rock cover | ineligible"));
    }

    #[test]
    fn closure_ranks_fundable_packages_by_cash_without_spending_credit_or_surety() {
        let screened = CLOSURE.replace(
            r#"through = "2030-12-31""#,
            "through = \"2030-12-31\"\ncash_headroom_usd = \"750000\"",
        );
        let md = report(&screened);
        assert!(md.contains("North waste-rock cover | eligible | 900000 | 2030-10-31 | 1510000 | 0 | 0 | 900000 | short by 150000 USD"));
        assert!(md.contains("East stockpile rehabilitation | eligible | 600000 | 2030-11-30 | -575000 | 4200000 | 1000000 | 600000 | funded"));
        assert!(md.contains(
            "among funded packages through 2030-12-31: East stockpile rehabilitation (-575000 USD)"
        ));
        let at_east_floor = report(&screened.replace("750000", "600000"));
        assert!(at_east_floor.contains("East stockpile rehabilitation (-575000 USD)"));
        let short = report(&screened.replace("750000", "599999"));
        assert!(short.contains("| 4200000 | 1000000 | 600000 | short by 1 USD"));
        assert!(short.contains("No eligible package with resolved cash timing fits"));
    }

    #[test]
    fn closure_reserves_separately_dated_cash_reposts_before_release() {
        let screened = CLOSURE
            .replace(
                r#"through = "2030-12-31""#,
                "through = \"2030-12-31\"\ncash_headroom_usd = \"1300000\"",
            )
            .replace(
                r#"reposted_cash_usd = "400000""#,
                "reposted_cash_usd = \"400000\"\ncash_repost_date = \"2030-07-01\"",
            );
        let covered = report(&screened);
        assert!(covered.contains("| 1510000 | 0 | 0 | 1300000 | funded"));
        assert!(covered.contains("North waste-rock cover (1510000 USD)"));
        let short = report(&screened.replace(
            "cash_headroom_usd = \"1300000\"",
            "cash_headroom_usd = \"1299999\"",
        ));
        assert!(short.contains("| 1510000 | 0 | 0 | 1300000 | short by 1 USD"));
        assert!(short.contains("East stockpile rehabilitation (-575000 USD)"));
        let same_day = report(&screened.replace("2030-07-01", "2030-10-31"));
        assert!(same_day.contains("| 1510000 | 0 | 0 | 1300000 | funded"));
        let later_post = report(&screened.replace("2030-07-01", "2031-01-01"));
        assert!(later_post.contains("| 1910000 | 0 | 0 | 900000 | funded"));
        let late_release = report(&screened.replace("2030-10-31", "2031-01-01"));
        assert!(late_release.contains("| -1300000 | 0 | 0 | 1300000 | funded"));
        let pending = report(&screened.replace("2030-10-31", "pending"));
        assert!(pending.contains(
            "| pending | unresolved | unresolved | unresolved | 1300000 | pending release"
        ));
        assert!(pending.contains("East stockpile rehabilitation (-575000 USD)"));
        let ineligible = report(&screened.replacen("eligible = \"yes\"", "eligible = \"no\"", 1));
        assert!(
            ineligible.contains("| unresolved | unresolved | unresolved | unresolved | ineligible")
        );
        for invalid in [
            screened.replace("2030-07-01", "2030-03-07"),
            screened.replace("2030-07-01", "2030-06-31"),
            screened.replace("\ncash_headroom_usd = \"1300000\"", ""),
        ] {
            assert!(evaluate(&invalid).is_err());
        }
    }

    #[test]
    fn tenure_options_require_available_rights_and_admissible_detection() {
        let only = LAND
            .replace("available = \"yes\"", "available = \"no\"")
            .replacen("available = \"no\"", "available = \"yes\"", 1);
        assert!(report(&only).contains("across declared ranges: renew"));
        assert!(
            report(&LAND.replace("cover_metres = \"60\"", "cover_metres = \"0\""))
                .contains("Sensitive negative")
        );
        assert!(
            report(&LAND.replace("2030-03-15", "2030-03-07")).contains("farm\\_out | unavailable")
        );
        assert!(evaluate(&LAND.replace("retained_pct = \"0\"", "retained_pct = \"1\"")).is_err());
        assert!(
            evaluate(&LAND.replace(
                "baseline_detection_pct = \"80\"",
                "baseline_detection_pct = \"101\""
            ))
            .is_err()
        );
    }

    #[test]
    fn group_excludes_expired_interventions_and_rejects_incompatible_scopes() {
        assert!(
            report(&GROUP.replace("2030-10-04", "2030-09-28")).contains("group shortfall of 700")
        );
        for changed in [
            GROUP.replacen("commodity = \"copper\"", "commodity = \"nickel\"", 1),
            GROUP.replace("interest_bp = \"6000\"", "interest_bp = \"12000\""),
            GROUP.replace("100_percent_project", "attributable"),
            GROUP.replace("id = \"Mesa\"", "id = \"Valley\""),
            GROUP.replacen("target = \"13000\"", "target = \"13001\"", 1),
        ] {
            assert!(evaluate(&changed).is_err());
        }
    }

    fn group_with_shared_limit(limit: &str) -> String {
        GROUP
            .replace(
                "Separate processing and haul routes; simultaneous limits affirmed in Q4C",
                "One remaining shared copper capacity allowance above current site plans",
            )
            .replace(
                r#"target = "13000""#,
                &format!("target = \"13000\"\nshared_incremental_project_capacity = \"{limit}\""),
            )
    }

    #[test]
    fn shared_group_capacity_limits_physical_output_before_ownership() {
        let scarce = report(&group_with_shared_limit("900"));
        assert!(scarce.contains("current plan 12000"));
        assert!(scarce.contains("maximum additional output before deadlines 900."));
        assert!(scarce.contains("group shortfall of 100."));

        let fractional = report(&group_with_shared_limit("1101"));
        // Mesa takes 1,000 units. Valley's remaining 101 units on a 60% share yield 60.6.
        assert!(fractional.contains("maximum additional output before deadlines 5303/5."));
        assert!(fractional.contains("Site maximum attributable before shared limit"));
        assert!(fractional.contains("Valley | 6000 | 6000 | 4800 | 5100"));

        let equal = report(&group_with_shared_limit("1000"));
        assert!(equal.contains("allow enough aggregate output"));
        assert!(equal.contains("Mesa is required. Without its uplift, the attributable maximum is 12300 and the group is short by 700. Decision deadline 2030-10-04."));
        assert!(!equal.contains("Valley is required"));
        assert!(report(&group_with_shared_limit("999")).contains("group shortfall of 1."));
        assert!(report(&group_with_shared_limit("0")).contains("group shortfall of 1000."));
    }

    #[test]
    fn shared_group_capacity_reassigns_headroom_when_an_intervention_is_unavailable() {
        let shared =
            group_with_shared_limit("1100").replace(r#"maximum = "8200""#, r#"maximum = "8100""#);
        let both_needed = report(&shared);
        assert!(both_needed.contains("maximum additional output before deadlines 1020."));
        assert!(both_needed.contains("Valley is required. Without its uplift, the attributable maximum is 12900 and the group is short by 100."));
        assert!(both_needed.contains("Mesa is required. Without its uplift, the attributable maximum is 12300 and the group is short by 700."));
        let at_deadline = report(&shared.replace("2030-10-04", "2030-09-30"));
        assert!(at_deadline.contains("maximum additional output before deadlines 1020."));
        let expired = report(&shared.replace("2030-10-04", "2030-09-29"));
        assert!(expired.contains("maximum additional output before deadlines 300."));
        assert!(expired.contains("group shortfall of 700."));
        for limit in ["-1", "900.5", "9223372036854775808"] {
            assert!(evaluate(&group_with_shared_limit(limit)).is_err());
        }
    }

    #[test]
    fn estimate_replay_refuses_retrospectively_selected_cohorts_and_keeps_nulls() {
        assert!(evaluate(&ESTIMATES.replace("2025-01-03", "2028-07-02")).is_err());
        assert!(
            evaluate(&ESTIMATES.replace(
                "holdout = \"South circuit\"",
                "holdout = \"Western transformer\""
            ))
            .is_err()
        );
        assert!(
            evaluate(&ESTIMATES.replace(
                "comparable_actual_usd = \"unknown\"",
                "comparable_actual_usd = \"0\""
            ))
            .is_err()
        );
        assert!(
            report(&ESTIMATES.replace("2027-07-31", "2028-07-01"))
                .contains("Comparable adjustment unavailable")
        );
    }
}
