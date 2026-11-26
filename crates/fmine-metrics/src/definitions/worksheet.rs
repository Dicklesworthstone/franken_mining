//! Strict declared cost worksheets and their exact, evidence-preserving receipts (BD-061).
//! Determinism: Exact (monetary inputs never pass through floating point).

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use fmine_claim::UuTerm;
use fmine_core::{Digest32, anchor::SourceAnchor, hash::sha256};
use fmine_evidence::{Refusal, RefusalCode};
use fmine_registry::metrics::MetricRegistry;
use fmine_registry::toml_subset::parse_subset;
use fmine_units::{Currency, ExactQty, Money};

use super::CostUnit;
use super::claims::{self, CostInput, CostReceipt, CostValue, DeclaredDriver, Scope};
use super::declared::{
    Fields, claim_json, decimal_parts, fields, join_json, js, qty, source_claim,
};
use crate::guidance::{Ownership, Period};

pub const SCHEMA: &str = "fmine.cost-worksheet.v1";
const MAX_ROWS: usize = 4096;
/// Bounded population of separately fitted, explicitly named cost pools in one plan.
pub const MAX_FITTED_DRIVERS: usize = 64;

fn bad(line: usize, message: impl AsRef<str>) -> Refusal {
    Refusal::new(
        RefusalCode::InputMissingOrUnreadable506,
        format!("cost worksheet line {line}: {}", message.as_ref()),
    )
}

fn get<'a>(fields: &Fields<'a>, key: &str) -> Result<(&'a str, usize), Refusal> {
    fields
        .get(key)
        .copied()
        .ok_or_else(|| bad(1, format!("missing {key}")))
}

fn quantity(value: &str, line: usize) -> Result<ExactQty, Refusal> {
    super::declared::quantity(value).ok_or_else(|| {
        bad(
            line,
            "expected an exact decimal or n/d with a positive denominator",
        )
    })
}

pub(super) fn money(value: &str, line: usize) -> Result<Money, Refusal> {
    let fail = || {
        bad(
            line,
            "USD must be a plain decimal with at most four fractional digits that fits Money",
        )
    };
    let (negative, whole, fraction) = decimal_parts(value).ok_or_else(fail)?;
    if fraction.len() > 4 {
        return Err(fail());
    }
    let mut minor = 0_i128;
    for digit in whole
        .bytes()
        .chain(fraction.bytes())
        .chain(std::iter::repeat_n(b'0', 4 - fraction.len()))
    {
        let digit = i128::from(digit - b'0');
        minor = minor
            .checked_mul(10)
            .and_then(|n| {
                if negative {
                    n.checked_sub(digit)
                } else {
                    n.checked_add(digit)
                }
            })
            .ok_or_else(fail)?;
    }
    Ok(Money::from_minor(Currency::USD, minor))
}

fn period(value: &str, line: usize) -> Result<Period, Refusal> {
    super::declared::period(value)
        .ok_or_else(|| bad(line, "period must be YYYYFY, YYYYQ1..Q4 or YYYYH1..H2"))
}

fn anchored(source: &SourceAnchor, locator: impl Into<String>) -> SourceAnchor {
    SourceAnchor::new(&source.artifact_path, source.artifact_sha256, locator)
}

/// Parse authenticated worksheet and registry bytes, then evaluate their selected registered rule.
/// All declared inputs stay Assumed and Internal. `complete = "true"` is an explicit assumption,
/// never independent evidence that an omitted registered component is zero.
pub fn evaluate(
    text: &str,
    source: SourceAnchor,
    metrics_text: &str,
    metrics_source: SourceAnchor,
    definition: &str,
) -> Result<CostReceipt, Refusal> {
    evaluate_inner(text, source, metrics_text, metrics_source, definition, &[])
}

/// Add an admitted fitted cost line to the explicitly complete population. A duplicated
/// item, mismatched period, basis or ownership is refused by the same definition owner.
pub fn evaluate_with_fitted_driver(
    text: &str,
    source: SourceAnchor,
    metrics_text: &str,
    metrics_source: SourceAnchor,
    definition: &str,
    driver: &super::driver::DriverReceipt,
) -> Result<CostReceipt, Refusal> {
    evaluate_with_fitted_drivers(
        text,
        source,
        metrics_text,
        metrics_source,
        definition,
        std::slice::from_ref(driver),
    )
}

/// Include one fitted line per explicitly named cost pool in the same complete plan.
/// Every card must be used by the selected definition exactly once. Direct lines and declared
/// drivers cannot duplicate a fitted pool. The cards retain their individual error models;
/// summing their point predictions does not assert independent errors or a joint variance.
pub fn evaluate_with_fitted_drivers(
    text: &str,
    source: SourceAnchor,
    metrics_text: &str,
    metrics_source: SourceAnchor,
    definition: &str,
    drivers: &[super::driver::DriverReceipt],
) -> Result<CostReceipt, Refusal> {
    if drivers.is_empty() || drivers.len() > MAX_FITTED_DRIVERS {
        return Err(bad(
            1,
            format!("supply 1..{MAX_FITTED_DRIVERS} fitted cost-pool cards"),
        ));
    }
    evaluate_inner(
        text,
        source,
        metrics_text,
        metrics_source,
        definition,
        drivers,
    )
}

fn evaluate_inner(
    text: &str,
    source: SourceAnchor,
    metrics_text: &str,
    metrics_source: SourceAnchor,
    definition: &str,
    drivers: &[super::driver::DriverReceipt],
) -> Result<CostReceipt, Refusal> {
    for (bytes, anchor) in [(text, &source), (metrics_text, &metrics_source)] {
        if sha256(bytes.as_bytes()).0 != anchor.artifact_sha256 {
            return Err(Refusal::new(
                RefusalCode::DigestMismatch505,
                "cost worksheet or metric registry bytes differ from their source SHA-256",
            ));
        }
    }
    let registry =
        MetricRegistry::parse(metrics_text).map_err(|e| bad(1, format!("metric registry: {e}")))?;
    let definition_tag = claims::definition_tag(&registry, definition)?;
    let doc = parse_subset(text).map_err(|e| bad(e.line, e.reason))?;
    if doc.rows.len() > MAX_ROWS {
        return Err(bad(
            1,
            "worksheet permits at most 4096 cost/driver/project/UU rows",
        ));
    }
    let top = fields(
        &doc.top,
        &[
            "schema",
            "issuer",
            "subject",
            "period",
            "ownership",
            "basis",
            "currency",
            "complete",
        ],
        &[
            "ownership_share",
            "ounces_sold",
            "pounds_sold",
            "gold_share",
            "copper_share",
        ],
        1,
        bad,
    )?;
    if get(&top, "schema")?.0 != SCHEMA {
        return Err(bad(1, format!("schema must be {SCHEMA}")));
    }
    if get(&top, "currency")?.0 != "USD" {
        return Err(bad(
            get(&top, "currency")?.1,
            "currency must be USD; no implicit FX",
        ));
    }
    let (complete, complete_line) = get(&top, "complete")?;
    if complete != "true" {
        return Err(bad(complete_line, "complete must explicitly be \"true\""));
    }
    let ownership = match (get(&top, "ownership")?.0, top.get("ownership_share")) {
        ("full", None) => Ownership::Full,
        ("attributable", Some((share, line))) => Ownership::Attributable {
            share: quantity(share, *line)?,
        },
        _ => {
            return Err(bad(
                get(&top, "ownership")?.1,
                "ownership requires full without ownership_share, or attributable with an exact ownership_share",
            ));
        }
    };
    let (period_text, period_line) = get(&top, "period")?;
    let scope = Scope {
        issuer: get(&top, "issuer")?.0.into(),
        subject: get(&top, "subject")?.0.into(),
        period: period(period_text, period_line)?,
        ownership,
        basis: get(&top, "basis")?.0.into(),
    };
    let mut values = Vec::new();
    match (top.get("ounces_sold"), top.get("pounds_sold")) {
        (Some((sold, line)), None) => {
            values.push((CostValue::SoldOunces(quantity(sold, *line)?), *line));
        }
        (None, Some((sold, line))) => {
            values.push((CostValue::SoldPounds(quantity(sold, *line)?), *line));
        }
        _ => {
            return Err(bad(
                1,
                "declare exactly one of ounces_sold (gold) or pounds_sold (copper)",
            ));
        }
    }
    if let Some((share, line)) = top.get("gold_share") {
        values.push((CostValue::GoldShare(quantity(share, *line)?), *line));
    }
    if let Some((share, line)) = top.get("copper_share") {
        values.push((CostValue::CopperShare(quantity(share, *line)?), *line));
    }
    let mut unknowns = BTreeMap::new();
    let mut row_anchors = BTreeMap::new();
    for row in &doc.rows {
        match row.table.as_str() {
            "cost" => {
                let f = fields(&row.fields, &["item", "usd"], &[], row.line, bad)?;
                let (usd, line) = get(&f, "usd")?;
                values.push((
                    CostValue::Line {
                        item: get(&f, "item")?.0.into(),
                        amount: money(usd, line)?,
                    },
                    row.line,
                ));
            }
            "driver_cost" => {
                let f = fields(
                    &row.fields,
                    &["item", "driver", "unit", "quantity", "usd_per_unit"],
                    &["fixed_usd"],
                    row.line,
                    bad,
                )?;
                let fixed_usd = f.get("fixed_usd").map_or_else(
                    || Ok(Money::from_minor(Currency::USD, 0)),
                    |(value, line)| money(value, *line),
                )?;
                let parse_quantity = |key| {
                    let (value, line) = get(&f, key)?;
                    quantity(value, line)
                };
                values.push((
                    CostValue::DriverLine {
                        item: get(&f, "item")?.0.into(),
                        driver: DeclaredDriver {
                            kind: get(&f, "driver")?.0.into(),
                            unit: get(&f, "unit")?.0.into(),
                            quantity: parse_quantity("quantity")?,
                            usd_per_unit: parse_quantity("usd_per_unit")?,
                            fixed_usd,
                        },
                    },
                    row.line,
                ));
                let mut anchors: Vec<_> = f
                    .iter()
                    .map(|(key, (_, line))| {
                        anchored(&source, format!("line {line}: driver_cost.{key}"))
                    })
                    .collect();
                if !f.contains_key("fixed_usd") {
                    anchors.push(anchored(
                        &source,
                        format!("line {}: driver_cost.fixed_usd defaults to 0 USD", row.line),
                    ));
                }
                row_anchors.insert(row.line, anchors);
            }
            "priced_sale" => {
                let f = fields(
                    &row.fields,
                    &["metal", "unit", "quantity_sold", "usd_per_unit"],
                    &["royalty_fraction", "by_product_credit_fraction"],
                    row.line,
                    bad,
                )?;
                let (metal, line) = get(&f, "metal")?;
                let metal = super::geo::Metal::ALL
                    .into_iter()
                    .find(|m| m.name() == metal)
                    .ok_or_else(|| bad(line, "priced sale metal must be gold, copper or silver"))?;
                let (unit, line) = get(&f, "unit")?;
                if unit != metal.unit() {
                    return Err(bad(
                        line,
                        format!("{} sales require {} units", metal.name(), metal.unit()),
                    ));
                }
                let number = |key| {
                    let (v, line) = get(&f, key)?;
                    quantity(v, line)
                };
                let optional = |key: &str| f.get(key).map(|(v, line)| quantity(v, *line)).transpose();
                values.push((
                    CostValue::PricedSale(claims::priced_sale::PricedSale {
                        metal,
                        quantity_sold: number("quantity_sold")?,
                        usd_per_unit: number("usd_per_unit")?,
                        royalty_fraction: optional("royalty_fraction")?,
                        by_product_credit_fraction: optional("by_product_credit_fraction")?,
                    }),
                    row.line,
                ));
                row_anchors.insert(
                    row.line,
                    f.iter()
                        .map(|(key, (_, line))| {
                            anchored(&source, format!("line {line}: priced_sale.{key}"))
                        })
                        .collect(),
                );
            }
            "project_cost" => {
                let f = fields(
                    &row.fields,
                    &[
                        "id",
                        "cost_kind",
                        "usd",
                        "classification_rule",
                        "project_kind",
                    ],
                    &[
                        "uplift_percent",
                        "months_to_net_cash",
                        "duration_months",
                        "ore_phase_years",
                    ],
                    row.line,
                    bad,
                )?;
                let whole_months = |key| -> Result<i64, Refusal> {
                    let (value, line) = get(&f, key)?;
                    value
                        .parse::<i64>()
                        .ok()
                        .filter(|n| *n >= 0)
                        .ok_or_else(|| bad(line, "months must be a nonnegative whole number"))
                };
                let assumptions = match get(&f, "project_kind")?.0 {
                    "new_operation"
                        if ![
                            "uplift_percent",
                            "months_to_net_cash",
                            "duration_months",
                            "ore_phase_years",
                        ]
                        .iter()
                        .any(|key| f.contains_key(key)) =>
                    {
                        super::Project::NewOperation
                    }
                    "existing_operation"
                        if !f.contains_key("duration_months")
                            && !f.contains_key("ore_phase_years") =>
                    {
                        let (uplift, line) = get(&f, "uplift_percent")?;
                        let months_to_net_cash = match get(&f, "months_to_net_cash")?.0 {
                            "never" => None,
                            _ => Some(whole_months("months_to_net_cash")?),
                        };
                        super::Project::AtExistingOperation {
                            uplift_percent: quantity(uplift, line)?,
                            months_to_net_cash,
                        }
                    }
                    "extension"
                        if !f.contains_key("uplift_percent")
                            && !f.contains_key("months_to_net_cash") =>
                    {
                        let (years, line) = get(&f, "ore_phase_years")?;
                        super::Project::Extension {
                            duration_months: whole_months("duration_months")?,
                            ore_phase_years: quantity(years, line)?,
                        }
                    }
                    _ => {
                        return Err(bad(
                            row.line,
                            "project_kind requires new_operation without threshold fields, existing_operation with uplift_percent and months_to_net_cash, or extension with duration_months and ore_phase_years",
                        ));
                    }
                };
                let (usd, line) = get(&f, "usd")?;
                let (kind, kind_line) = get(&f, "cost_kind")?;
                let classification_rule = get(&f, "classification_rule")?.0;
                values.push((
                    CostValue::ProjectLine(super::project::DeclaredProject {
                        id: get(&f, "id")?.0.into(),
                        cost_kind: super::project::CostKind::parse(kind)
                            .ok_or_else(|| bad(kind_line, "unsupported project cost_kind"))?,
                        amount: money(usd, line)?,
                        classification_rule: classification_rule.into(),
                        rule_tag: super::project::rule_tag(&registry, classification_rule)?,
                        assumptions,
                    }),
                    row.line,
                ));
                let mut anchors: Vec<_> = f
                    .iter()
                    .map(|(key, (_, line))| {
                        anchored(&source, format!("line {line}: project_cost.{key}"))
                    })
                    .collect();
                anchors.push(anchored(
                    &metrics_source,
                    format!("project_test {classification_rule}: exact classification thresholds"),
                ));
                row_anchors.insert(row.line, anchors);
            }
            "uu" => {
                let f = fields(
                    &row.fields,
                    &["id", "description", "trigger", "downside_usd_m"],
                    &[],
                    row.line,
                    bad,
                )?;
                let (exposure, line) = get(&f, "downside_usd_m")?;
                decimal_parts(exposure).ok_or_else(|| {
                    bad(
                        line,
                        "UU exposure must be a positive finite decimal in USD millions",
                    )
                })?;
                let exposure: f64 = exposure
                    .parse()
                    .map_err(|_| bad(line, "invalid UU exposure"))?;
                if !exposure.is_finite() || exposure <= 0.0 {
                    return Err(bad(line, "UU exposure must be positive and finite"));
                }
                let id = get(&f, "id")?.0;
                if unknowns
                    .insert(
                        id,
                        UuTerm {
                            scenario_id: id.into(),
                            description: get(&f, "description")?.0.into(),
                            trigger_indicator: get(&f, "trigger")?.0.into(),
                            downside_exposure_usd_m: exposure,
                        },
                    )
                    .is_some()
                {
                    return Err(bad(row.line, format!("duplicate UU scenario {id}")));
                }
            }
            other => return Err(bad(row.line, format!("unknown table {other}"))),
        }
    }
    values.push((
        CostValue::Rule {
            definition: definition.into(),
            definition_tag,
            complete: true,
        },
        complete_line,
    ));
    let uu: Vec<_> = unknowns.into_values().collect();
    let mut inputs = values
        .into_iter()
        .map(|(value, line)| {
            let mut anchors = vec![anchored(
                &source,
                format!("line {line}: declared cost worksheet input"),
            )];
            anchors.extend(row_anchors.remove(&line).unwrap_or_default());
            let rule = matches!(value, CostValue::Rule { .. });
            if rule {
                anchors.push(anchored(
                    &metrics_source,
                    format!("definition {definition} and inherited lines/exclusions"),
                ));
            }
            let input = CostInput {
                scope: scope.clone(),
                value,
            };
            let tag = input.canonical_tag();
            let additivity = input.additivity();
            source_claim(
                claims::INPUT_METRIC,
                input,
                &tag,
                additivity,
                // Scope-wide unknowns belong to the completeness declaration and propagate once.
                // Copying the whole budget onto each line would multiply input/output size quadratically.
                if rule { uu.clone() } else { Vec::new() },
                anchors,
            )
        })
        .collect::<Vec<_>>();
    let mut fitted_items = BTreeSet::new();
    for driver in drivers {
        let item = driver.summary().item.as_str();
        if !fitted_items.insert(item) {
            return Err(Refusal::new(
                RefusalCode::BasisMismatch105,
                format!("cost worksheet: more than one fitted card maps to cost pool {item}"),
            ));
        }
        if inputs.iter().any(|input| match &input.value.value {
            CostValue::Line { item: existing, .. }
            | CostValue::DriverLine { item: existing, .. } => existing == item,
            _ => false,
        }) {
            return Err(Refusal::new(
                RefusalCode::BasisMismatch105,
                format!(
                    "cost worksheet: fitted cost pool {item} is already supplied by another line"
                ),
            ));
        }
        inputs.push(driver.predicted_line());
    }
    let receipt = claims::evaluate(&registry, &inputs)?;
    for contribution in &receipt.summary().contributions {
        if fitted_items.contains(contribution.item.as_str())
            && contribution.treatment == claims::Treatment::Excluded
        {
            return Err(Refusal::new(
                RefusalCode::BasisMismatch105,
                format!(
                    "cost worksheet: fitted cost pool {} is unused by definition {definition}; select a definition that includes it or remove the card",
                    contribution.item
                ),
            ));
        }
    }
    Ok(receipt)
}

pub(super) fn scope_json(scope: &Scope) -> String {
    let ownership = super::declared::ownership_json(scope.ownership);
    format!(
        "{{\"issuer\":{},\"subject\":{},\"period\":{{\"year\":{},\"first_month\":{},\"last_month\":{}}},\"ownership\":{ownership},\"basis\":{}}}",
        js(&scope.issuer),
        js(&scope.subject),
        scope.period.year,
        scope.period.first_month,
        scope.period.last_month,
        js(&scope.basis)
    )
}

fn input_json(input: &CostInput) -> String {
    let value = match &input.value {
        CostValue::Line { item, amount } => format!(
            "{{\"kind\":\"cost\",\"item\":{},\"currency\":\"USD\",\"minor\":{},\"scale\":10000}}",
            js(item),
            js(&amount.minor.to_string())
        ),
        CostValue::DriverLine { item, driver } => format!(
            "{{\"kind\":\"driver_cost\",\"item\":{},{}}}",
            js(item),
            driver.json_fields()
        ),
        CostValue::PricedSale(sale) => {
            format!("{{\"kind\":\"priced_sale\",{}}}", sale.json_fields())
        }
        CostValue::ProjectLine(project) => {
            format!("{{\"kind\":\"project_cost\",{}}}", project.json_fields())
        }
        CostValue::SoldOunces(q) => {
            format!("{{\"kind\":\"ounces_sold\",\"value\":{}}}", js(&qty(*q)))
        }
        CostValue::GoldShare(q) => {
            format!("{{\"kind\":\"gold_share\",\"value\":{}}}", js(&qty(*q)))
        }
        CostValue::SoldPounds(q) => {
            format!("{{\"kind\":\"pounds_sold\",\"value\":{}}}", js(&qty(*q)))
        }
        CostValue::CopperShare(q) => {
            format!("{{\"kind\":\"copper_share\",\"value\":{}}}", js(&qty(*q)))
        }
        CostValue::Rule {
            definition,
            definition_tag,
            complete,
        } => format!(
            "{{\"kind\":\"rule\",\"definition\":{},\"definition_tag\":{},\"complete\":{complete}}}",
            js(definition),
            js(definition_tag)
        ),
    };
    format!(
        "{{\"scope\":{},\"value\":{value}}}",
        scope_json(&input.scope)
    )
}

impl CostReceipt {
    /// Complete admitted Claims and exact values, including excluded inputs and named unknowns.
    #[must_use]
    pub fn to_json(&self) -> String {
        let s = self.summary();
        let contributions = s.contributions.iter().map(|c| format!("{{\"item\":{},\"input_claim_id\":{},\"amount_minor\":{},\"treatment\":{},\"numerator_usd\":{}}}",
            js(&c.item), js(&c.input_claim_id.to_string()), js(&c.amount.minor.to_string()), js(c.treatment.as_str()), js(&qty(c.numerator_usd))))
            .collect::<Vec<_>>().join(",");
        let (quantity_key, share_key, rate_key) = match s.unit {
            CostUnit::GoldOunceSold => ("ounces_sold", "gold_share", "usd_per_oz"),
            CostUnit::CopperPoundSold => ("pounds_sold", "copper_share", "usd_per_lb"),
        };
        let projects = if s.project_classifications.is_empty() {
            String::new()
        } else {
            format!(
                ",\"project_classifications\":[{}]",
                join_json(&s.project_classifications, |p| p.to_json())
            )
        };
        let sales = if s.priced_sales.is_empty() {
            String::new()
        } else {
            format!(
                ",\"priced_sales\":[{}]",
                join_json(&s.priced_sales, |sale| sale.to_json())
            )
        };
        let summary = format!(
            "{{\"scope\":{},\"definition\":{},\"definition_tag\":{},\"superseded_by\":{},\"declared_complete\":{},\"currency\":\"USD\",\"numerator_usd\":{},\"{quantity_key}\":{},\"{share_key}\":{},\"{rate_key}\":{},\"contributions\":[{contributions}]{projects}{sales}}}",
            scope_json(&s.scope),
            js(&s.definition),
            js(&s.definition_tag),
            s.superseded_by
                .as_ref()
                .map_or_else(|| "null".into(), |id| js(id)),
            s.declared_complete,
            js(&qty(s.numerator_usd)),
            js(&qty(s.quantity_sold)),
            s.primary_metal_share
                .map_or_else(|| "null".into(), |q| js(&qty(q))),
            js(&qty(s.usd_per_unit))
        );
        format!(
            "{{\"digest\":{},\"summary\":{summary},\"claim\":{},\"inputs\":[{}]}}",
            js(self.digest()),
            claim_json(self.claim(), &summary, true),
            join_json(self.inputs(), |c| claim_json(
                c,
                &input_json(&c.value),
                true
            ))
        )
    }

    /// Readable reconciliation with the same exact numbers and source identities as JSON.
    #[must_use]
    pub fn to_markdown(&self) -> String {
        let s = self.summary();
        let text = |v: &str| {
            v.replace('\\', "\\\\")
                .replace('|', "\\|")
                .replace('`', "\\`")
                .replace('<', "&lt;")
                .replace('>', "&gt;")
        };
        let ownership = match s.scope.ownership {
            Ownership::Full => "100%".into(),
            Ownership::Attributable { share } => format!("attributable {}", qty(share)),
        };
        let mut out = format!(
            "# Declared cost worksheet\n\n{} / {} — {} months {}–{}, {}.\n\nBasis: {}.\n\n**{} {} sold**, under `{}`.\n\nNumerator: {} USD; denominator: {} {}.\n\n",
            text(&s.scope.issuer),
            text(&s.scope.subject),
            s.scope.period.year,
            s.scope.period.first_month,
            s.scope.period.last_month,
            ownership,
            text(&s.scope.basis),
            qty(s.usd_per_unit),
            s.unit.rate_unit(),
            text(&s.definition),
            qty(s.numerator_usd),
            qty(s.quantity_sold),
            s.unit.quantity_label()
        );
        if let Some(share) = s.primary_metal_share {
            let _ = writeln!(
                out,
                "Declared co-product {} allocation: {}.\n",
                match s.unit {
                    CostUnit::GoldOunceSold => "gold",
                    CostUnit::CopperPoundSold => "copper",
                },
                qty(share)
            );
        }
        if let Some(next) = &s.superseded_by {
            let _ = writeln!(out, "This definition is superseded by `{}`.\n", text(next));
        }
        out.push_str("| Cost item | Supplied USD | Treatment | Contribution to numerator, USD | Input Claim |\n|---|---:|---|---:|---|\n");
        for c in &s.contributions {
            let units = c.amount.minor.unsigned_abs();
            let amount = format!(
                "{}{}.{:04}",
                if c.amount.minor < 0 { "-" } else { "" },
                units / 10_000,
                units % 10_000
            );
            let _ = writeln!(
                out,
                "| {} | {amount} | {} | {} | `{}` |",
                text(&c.item),
                c.treatment.as_str(),
                qty(c.numerator_usd),
                c.input_claim_id
            );
        }
        let drivers: Vec<_> = self
            .inputs()
            .iter()
            .filter_map(|input| {
                if let CostValue::DriverLine { item, driver } = &input.value.value {
                    Some(format!(
                        "| {} | {} | {} {} | {} | {}/10000 |",
                        text(item),
                        text(&driver.kind),
                        qty(driver.quantity),
                        text(&driver.unit),
                        qty(driver.usd_per_unit),
                        driver.fixed_usd.minor
                    ))
                } else {
                    None
                }
            })
            .collect();
        if !drivers.is_empty() {
            out.push_str("\n## Declared cost drivers\n\nCost = quantity × USD per stated unit + fixed USD; no fitted rates or unit conversion.\n\n| Cost item | Driver | Quantity and unit | USD per unit | Fixed USD |\n|---|---|---:|---:|---:|\n");
            out.push_str(&drivers.join("\n"));
            out.push('\n');
        }
        if !s.priced_sales.is_empty() {
            out.push_str("\n## Price-linked costs on declared gross sales\n\nEach fraction uses the same quantity sold and price. The reconciliation above shows the selected definition’s treatment. Gross sales cover only the listed metals.\n\n| Metal | Sold | USD per unit | Gross sales (USD) | Royalty fraction | By-product credit fraction | Input Claim |\n|---|---:|---:|---:|---:|---:|---|\n");
            for assessed in &s.priced_sales {
                let sale = &assessed.sale;
                let fraction = |v: Option<ExactQty>| v.map_or_else(|| "undeclared".into(), qty);
                let _ = writeln!(
                    out,
                    "| {} | {} {} | {} | {} | {} | {} | {} |",
                    sale.metal.name(),
                    qty(sale.quantity_sold),
                    sale.metal.unit(),
                    qty(sale.usd_per_unit),
                    qty(assessed.gross_sales_usd),
                    fraction(sale.royalty_fraction),
                    fraction(sale.by_product_credit_fraction),
                    assessed.input_claim_id
                );
            }
        }
        if !s.project_classifications.is_empty() {
            out.push_str("\n## Conditional project classifications\n\nThe registered rule is applied to declared project facts. Initial development of a new operation is non-sustaining. Existing-operation and extension rows must meet both of their stated conditions. Facts and account classification are supplied by the worksheet author. `never` explicitly declares no incremental net cash. Supply a timing or `never` for an existing operation.\n\n");
            for p in &s.project_classifications {
                let t = &p.thresholds;
                let assumptions = match p.project.assumptions {
                    super::Project::NewOperation => "initial development at a new operation".into(),
                    super::Project::AtExistingOperation {
                        uplift_percent,
                        months_to_net_cash,
                    } => format!(
                        "existing operation; largest declared uplift {}%; first incremental net cash {}",
                        qty(uplift_percent),
                        months_to_net_cash
                            .map_or_else(|| "never".into(), |n| format!("at month {n}"))
                    ),
                    super::Project::Extension {
                        duration_months,
                        ore_phase_years,
                    } => format!(
                        "extension; duration {duration_months} months; ore phase {} years",
                        qty(ore_phase_years)
                    ),
                };
                let rule = if let Some((first, second)) = p.condition_names() {
                    format!(
                        "Rule `{}`: uplift ≥ {}%; incremental net cash after {} months or never; extension duration ≥ {} months and ore phase > {} years.\n\n- `{first}`: **{}**.\n- `{second}`: **{}**.",
                        text(&t.definition),
                        t.material_uplift_percent,
                        t.net_cash_months,
                        t.extension_months,
                        t.ore_phase_years,
                        p.first_condition,
                        p.second_condition,
                    )
                } else {
                    format!(
                        "Rule `{}` treats initial development at a new operation as non-sustaining.",
                        text(&t.definition)
                    )
                };
                let _ = writeln!(
                    out,
                    "### {}\n\n{}; declared category `{}`.\n\n{rule}\n\n- Classification: **{}**; cost item `{}`.\n- Input Claim: `{}`; rule digest `{}`.\n",
                    text(&p.project.id),
                    assumptions,
                    p.project.cost_kind.as_str(),
                    if p.non_sustaining {
                        "non-sustaining"
                    } else {
                        "sustaining"
                    },
                    p.item,
                    p.input_claim_id,
                    p.project.rule_tag
                );
            }
        }
        let _ = write!(
            out,
            "\nResult Claim: `{}`. Receipt: `{}`.\n\nEvidence: {:?}; knowledge: {:?}; confidentiality: {}.\n\nDeclared complete: **yes**. {}\n\n## Source trace\n\n",
            self.claim().claim_id,
            self.digest(),
            self.claim().evidence,
            self.claim().state,
            self.claim().confidentiality,
            claims::NO_CLAIM
        );
        for input in self.inputs() {
            let _ = writeln!(out, "- `{}` — {:?}", input.claim_id, input.value.value);
            for a in &input.anchors {
                let _ = writeln!(
                    out,
                    "  - {}: {}; SHA-256 `{}`",
                    text(&a.artifact_path),
                    text(&a.locator),
                    Digest32(a.artifact_sha256).to_hex()
                );
            }
        }
        out.push_str("\n## Named unknowns\n\n");
        if self.claim().uu_terms.is_empty() {
            out.push_str("None supplied; this worksheet does not create an unknown-risk budget or authorize a decision.\n");
        }
        for u in &self.claim().uu_terms {
            let _ = writeln!(
                out,
                "- {}: {}; downside exposure {} USD million; trigger: {}.",
                text(&u.scenario_id),
                text(&u.description),
                u.downside_exposure_usd_m,
                text(&u.trigger_indicator)
            );
        }
        out
    }
}
