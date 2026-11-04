//! DESIGN (BD-331.S4): an implied company-minus-regions rate is a descriptive residual, never a proven
//! corporate or JV cost allocation and never a scored identity. All arithmetic uses displayed rounding boxes.

use std::fmt::Write as _;

use fmine_core::{Encoder, blake3_hash, hash::Digest32};

use super::{
    BTreeMap, BTreeSet, FLOW_FIELDS, Group, Interval, LedgerFile, LedgerRules, Mark, Operand,
    Period, RATE_FIELDS, Rat, RegionMemberRow, Slot, SlotKey, mean, rat_text,
};
use crate::js;

const RULES: &str = "company-unallocated-cost-layer/1";
const LIMITATION: &str = "Total Gold rate minus rates weighted by regional attributable gold sold. The implied difference identifies no corporate G&A, JV or other cost component, and does not reconcile the company's sales denominator. The missing cost breakdown remains unresolved, including when zero is within the interval. These are rounding bounds, not confidence intervals. This separately digested projection is outside scored identity counts and the M0 identity certificate.";

/// A displayed source operand, retaining nil and unavailable states instead of assigning them numeric values.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CostLayerValue {
    Number { value: Rat, step: Rat },
    Nil,
    Unavailable { reason: &'static str },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CostLayerOperand {
    pub at: Option<String>,
    pub value: CostLayerValue,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CostLayerRegion {
    pub region: String,
    /// A printed registered region with no active membership is retained, but cannot enter a partial mean.
    pub mapped: bool,
    pub rate: CostLayerOperand,
    pub sold: CostLayerOperand,
}

/// Rates and differences are USD per attributable gold ounce sold. Intervals are closed exact rounding boxes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CostLayerAmounts {
    pub weighted_point: Rat,
    pub weighted_interval: Interval,
    pub point: Rat,
    pub interval: Interval,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CostLayerUnavailable {
    pub reason: &'static str,
    pub at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompanyCostLayer {
    pub release: String,
    pub sheet: String,
    pub period: Period,
    pub group: Group,
    pub field: String,
    /// Every source registered at this release; more than one distinct source makes the result unavailable.
    pub sources: Vec<(String, Digest32)>,
    /// Active membership declarations, sorted by region, member and applicability. Comparatives use the
    /// release's map, just as the summary identities do, rather than a map inferred from the value period.
    pub region_map: Vec<RegionMemberRow>,
    pub company: CostLayerOperand,
    pub regions: Vec<CostLayerRegion>,
    pub result: Result<CostLayerAmounts, CostLayerUnavailable>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CompanyCostLayers {
    pub rows: Vec<CompanyCostLayer>,
}

impl CostLayerOperand {
    fn from_slot(slot: Option<&Slot>) -> Self {
        Self {
            at: slot.map(|s| s.locator.clone()),
            value: match slot.map(|s| &s.operand) {
                Some(Operand::Number { value, step }) => CostLayerValue::Number {
                    value: *value,
                    step: *step,
                },
                Some(Operand::Placeholder(Mark::Nil)) => CostLayerValue::Nil,
                other => CostLayerValue::Unavailable {
                    reason: match other {
                        None => "operand_row_missing",
                        Some(Operand::Blank) => "blank_operand",
                        Some(Operand::Placeholder(_)) => "placeholder_operand",
                        _ => "operand_unreadable",
                    },
                },
            },
        }
    }

    fn number(&self) -> Result<(Rat, Interval), CostLayerUnavailable> {
        match self.value {
            CostLayerValue::Number { value, step } => {
                let bounds = || {
                    let half = step.checked_div(Rat::from_integer(2)).ok()?;
                    Some((value.checked_sub(half).ok()?, value.checked_add(half).ok()?))
                };
                bounds()
                    .map(|b| (value, b))
                    .ok_or_else(|| self.failure("exact_arithmetic_unavailable"))
            }
            CostLayerValue::Nil => Err(self.failure("placeholder_operand")),
            CostLayerValue::Unavailable { reason } => Err(self.failure(reason)),
        }
    }

    fn failure(&self, reason: &'static str) -> CostLayerUnavailable {
        CostLayerUnavailable {
            reason,
            at: self.at.clone(),
        }
    }

    fn json(&self) -> String {
        let value = match self.value {
            CostLayerValue::Number { value, step } => format!(
                "\"kind\":\"number\",\"displayed\":{},\"step\":{}",
                js(&rat_text(value)),
                js(&rat_text(step))
            ),
            CostLayerValue::Nil => {
                "\"kind\":\"nil\",\"interpretation\":\"zero sold only; a nil rate is not zero\""
                    .into()
            }
            CostLayerValue::Unavailable { reason } => {
                format!("\"kind\":\"unavailable\",\"reason\":{}", js(reason))
            }
        };
        format!(
            "{{\"at\":{},{value}}}",
            self.at.as_deref().map_or_else(|| "null".into(), js)
        )
    }

    fn shown(&self) -> String {
        let value = match self.value {
            CostLayerValue::Number { value, step } => {
                format!("{} (step {})", rat_text(value), rat_text(step))
            }
            CostLayerValue::Nil => "nil".into(),
            CostLayerValue::Unavailable { reason } => reason.into(),
        };
        format!("{value} at {}", self.at.as_deref().unwrap_or("missing row"))
    }
}

pub(super) fn amounts(
    company: &CostLayerOperand,
    regions: &[CostLayerRegion],
) -> Result<CostLayerAmounts, CostLayerUnavailable> {
    let (company_point, company_box) = company.number()?;
    let (mut rates, mut weights, mut parts) = (Vec::new(), Vec::new(), Vec::new());
    for region in regions {
        // A nil sold cell means exactly no sales. Its rate is unused, but both source operands remain visible.
        if region.sold.value == CostLayerValue::Nil {
            continue;
        }
        let (weight, weight_box) = region.sold.number()?;
        if weight_box.0.signum() <= 0 {
            return Err(region.sold.failure("weight_not_positive"));
        }
        let (rate, rate_box) = region.rate.number()?;
        rates.push(rate);
        weights.push(weight);
        parts.push((rate_box, weight_box));
    }
    if parts.is_empty() {
        return Err(CostLayerUnavailable {
            reason: "weights_all_nil",
            at: None,
        });
    }
    let calculate = || {
        let weighted_point = mean(&rates, &weights)?;
        let weighted_interval = weighted_mean_bounds(&parts)?;
        Some(CostLayerAmounts {
            weighted_point,
            weighted_interval,
            point: company_point.checked_sub(weighted_point).ok()?,
            interval: (
                company_box.0.checked_sub(weighted_interval.1).ok()?,
                company_box.1.checked_sub(weighted_interval.0).ok()?,
            ),
        })
    };
    calculate().ok_or(CostLayerUnavailable {
        reason: "exact_arithmetic_unavailable",
        at: None,
    })
}

/// Fallible merge sort keeps rational-comparison overflow an unavailable result, never an invented ordering.
fn sort_rates(rows: &mut [(Rat, Interval)], descending: bool) -> Option<()> {
    if rows.len() < 2 {
        return Some(());
    }
    let mid = rows.len() / 2;
    let (left, right) = rows.split_at_mut(mid);
    sort_rates(left, descending)?;
    sort_rates(right, descending)?;
    let mut sorted = Vec::with_capacity(left.len() + right.len());
    let (mut a, mut b) = (0, 0);
    while a < left.len() && b < right.len() {
        let cmp = left[a].0.checked_cmp(right[b].0).ok()?;
        if if descending {
            !cmp.is_lt()
        } else {
            !cmp.is_gt()
        } {
            sorted.push(left[a]);
            a += 1;
        } else {
            sorted.push(right[b]);
            b += 1;
        }
    }
    sorted.extend_from_slice(&left[a..]);
    sorted.extend_from_slice(&right[b..]);
    rows.copy_from_slice(&sorted);
    Some(())
}

/// At a minimum, a rate below the optimum takes its high weight and one above it takes its low weight.
/// Therefore an optimum is among the prefixes of rates sorted ascending (descending for a maximum).
/// Sweeping those prefixes visits n+1 vertices in O(n log n) total work, rather than all 2^n weight vertices.
pub(super) fn weighted_mean_bounds(parts: &[(Interval, Interval)]) -> Option<Interval> {
    if parts.is_empty() {
        return None;
    }
    for &((a, b), (c, d)) in parts {
        if c.signum() <= 0 || a.checked_cmp(b).ok()?.is_gt() || c.checked_cmp(d).ok()?.is_gt() {
            return None;
        }
    }
    let extreme = |upper: bool| -> Option<Rat> {
        let mut rows = parts
            .iter()
            .map(|((lo, hi), weight)| (if upper { *hi } else { *lo }, *weight))
            .collect::<Vec<_>>();
        sort_rates(&mut rows, upper)?;
        let (mut numerator, mut denominator) = (Rat::from_integer(0), Rat::from_integer(0));
        for &(rate, (low, _)) in &rows {
            numerator = numerator.checked_add(rate.checked_mul(low).ok()?).ok()?;
            denominator = denominator.checked_add(low).ok()?;
        }
        let mut best = numerator.checked_div(denominator).ok()?;
        for (rate, (low, high)) in rows {
            let delta = high.checked_sub(low).ok()?;
            numerator = numerator.checked_add(rate.checked_mul(delta).ok()?).ok()?;
            denominator = denominator.checked_add(delta).ok()?;
            let candidate = numerator.checked_div(denominator).ok()?;
            let cmp = candidate.checked_cmp(best).ok()?;
            if if upper { cmp.is_gt() } else { cmp.is_lt() } {
                best = candidate;
            }
        }
        Some(best)
    };
    Some((extreme(false)?, extreme(true)?))
}

pub(super) fn assess(
    files: &[LedgerFile<'_>],
    rules: &LedgerRules<'_>,
    slots: &BTreeMap<SlotKey, Slot>,
) -> CompanyCostLayers {
    let mut sources_by_release: BTreeMap<&str, Vec<(String, Digest32)>> = BTreeMap::new();
    for file in files {
        sources_by_release.entry(file.quarter).or_default().push((
            file.workbook.artifact_path.clone(),
            Digest32(file.workbook.sha256),
        ));
    }
    for sources in sources_by_release.values_mut() {
        sources.sort();
        sources.dedup();
    }
    let registered: BTreeSet<&str> = rules.regions.iter().map(String::as_str).collect();
    let mut printed: BTreeMap<(&str, &str, Group), BTreeSet<&str>> = BTreeMap::new();
    for (release, sheet, _, group, subject) in slots.keys() {
        if registered.contains(subject.as_str()) {
            printed
                .entry((release, sheet, *group))
                .or_default()
                .insert(subject);
        }
    }
    let mut rows = Vec::new();
    for ((release, sheet, field, group, subject), total) in slots {
        if subject != "Total Gold" || !RATE_FIELDS.contains(&field.as_str()) {
            continue;
        }
        let sources = sources_by_release
            .get(release.as_str())
            .cloned()
            .unwrap_or_default();
        let mut region_map = rules
            .region_members
            .iter()
            .filter(|r| r.holds_for(release))
            .cloned()
            .collect::<Vec<_>>();
        region_map.sort_by(|a, b| {
            (&a.region, &a.member, &a.first, &a.last)
                .cmp(&(&b.region, &b.member, &b.first, &b.last))
        });
        let mapped = region_map
            .iter()
            .map(|r| r.region.as_str())
            .collect::<BTreeSet<_>>();
        let mut regions = mapped.clone();
        // A still-printed registered region cannot disappear from the comparison merely because its entire
        // membership was removed. Retain its operands and name the incomplete map instead of using a subtotal.
        if let Some(found) = printed.get(&(release.as_str(), sheet.as_str(), *group)) {
            regions.extend(found.iter().copied());
        }
        let mut mismatched_period = None;
        let mut inputs = Vec::new();
        for region in regions {
            let slot = |metric: &str| {
                slots.get(&(
                    release.clone(),
                    sheet.clone(),
                    metric.into(),
                    *group,
                    region.into(),
                ))
            };
            let (rate, sold) = (slot(field), slot(FLOW_FIELDS[0][2]));
            for input in [rate, sold].into_iter().flatten() {
                if input.period != total.period && mismatched_period.is_none() {
                    mismatched_period = Some(input.locator.clone());
                }
            }
            inputs.push(CostLayerRegion {
                region: region.into(),
                mapped: mapped.contains(region),
                rate: CostLayerOperand::from_slot(rate),
                sold: CostLayerOperand::from_slot(sold),
            });
        }
        let company = CostLayerOperand::from_slot(Some(total));
        let result = if sources.len() != 1 {
            Err(company.failure("release_source_ambiguous"))
        } else if let Some(region) = inputs.iter().find(|r| !r.mapped) {
            Err(region.rate.failure("region_not_in_map"))
        } else if inputs.is_empty() {
            Err(company.failure("region_not_in_map"))
        } else if let Some(at) = mismatched_period {
            Err(CostLayerUnavailable {
                reason: "period_mismatch",
                at: Some(at),
            })
        } else {
            amounts(&company, &inputs)
        };
        rows.push(CompanyCostLayer {
            release: release.clone(),
            sheet: sheet.clone(),
            period: total.period,
            group: *group,
            field: field.clone(),
            sources,
            region_map,
            company,
            regions: inputs,
            result,
        });
    }
    CompanyCostLayers { rows }
}

fn interval_json((lo, hi): Interval) -> String {
    format!("[{},{}]", js(&rat_text(lo)), js(&rat_text(hi)))
}

impl CompanyCostLayer {
    #[must_use]
    pub fn region_map_digest(&self) -> Digest32 {
        let mut enc = Encoder::new_frame(*b"CLMP", 1);
        for r in &self.region_map {
            for field in [&r.region, &r.member, &r.first] {
                enc.put_str(field);
            }
            enc.put_u8(u8::from(r.last.is_some()));
            if let Some(last) = &r.last {
                enc.put_str(last);
            }
        }
        blake3_hash(&enc.finish())
    }

    fn json(&self) -> String {
        let sources = self
            .sources
            .iter()
            .map(|(path, sha)| {
                format!("{{\"path\":{},\"sha256\":{}}}", js(path), js(&sha.to_hex()))
            })
            .collect::<Vec<_>>()
            .join(",");
        let map = self
            .region_map
            .iter()
            .map(|r| {
                format!(
                    "{{\"region\":{},\"member\":{},\"first\":{},\"last\":{}}}",
                    js(&r.region),
                    js(&r.member),
                    js(&r.first),
                    r.last.as_deref().map_or_else(|| "null".into(), js)
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        let regions = self
            .regions
            .iter()
            .map(|r| {
                format!(
                    "{{\"region\":{},\"mapped\":{},\"rate\":{},\"sold\":{},\"excluded_nil_sold\":{}}}",
                    js(&r.region),
                    r.mapped,
                    r.rate.json(),
                    r.sold.json(),
                    r.sold.value == CostLayerValue::Nil
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        let result = match &self.result {
            Ok(a) => format!(
                "\"status\":\"unresolved\",\"weighted_regional_rate\":{},\"weighted_regional_interval\":{},\"implied_layer\":{},\"implied_layer_interval\":{}",
                js(&rat_text(a.weighted_point)),
                interval_json(a.weighted_interval),
                js(&rat_text(a.point)),
                interval_json(a.interval)
            ),
            Err(e) => format!(
                "\"status\":\"unavailable\",\"reason\":{},\"at\":{}",
                js(e.reason),
                e.at.as_deref().map_or_else(|| "null".into(), js)
            ),
        };
        format!(
            "{{\"release\":{},\"sheet\":{},\"period\":{},\"group\":{},\"field\":{},\"sources\":[{sources}],\"region_map_digest\":{},\"region_map\":[{map}],\"company_rate\":{},\"regions\":[{regions}],{result}}}",
            js(&self.release),
            js(&self.sheet),
            js(&self.period.code()),
            js(self.group.as_str()),
            js(&self.field),
            js(&self.region_map_digest().to_hex()),
            self.company.json()
        )
    }
}

impl CompanyCostLayers {
    fn body(&self) -> String {
        let rows = self
            .rows
            .iter()
            .map(CompanyCostLayer::json)
            .collect::<Vec<_>>()
            .join(",");
        format!(
            "{{\"schema\":\"fmine.company-cost-layers.v1\",\"rules_version\":{},\"unit\":\"usd_per_attributable_gold_ounce_sold\",\"weight_unit\":\"attributable_gold_koz_sold\",\"rounding\":\"closed_half_displayed_step\",\"limitations\":{},\"rows\":[{rows}]}}",
            js(RULES),
            js(LIMITATION)
        )
    }

    /// Binds source hashes, source operands, active membership, results and scope; separate from the M0 root.
    #[must_use]
    pub fn digest(&self) -> Digest32 {
        blake3_hash(self.body().as_bytes())
    }

    #[must_use]
    pub fn to_json(&self) -> String {
        let mut body = self.body();
        let digest = blake3_hash(body.as_bytes()).to_hex();
        body.pop();
        format!("{body},\"digest\":{}}}", js(&digest))
    }

    #[must_use]
    pub fn to_markdown(&self) -> String {
        let mut out = format!(
            "\n## Implied unallocated company cost layers\n\n{LIMITATION}\n\nProjection digest: `{}`.\n\n| Release / period / field | Regional rate and rounding bounds (USD/oz) | Implied difference and rounding bounds (USD/oz) | Status | Source evidence |\n|---|---|---|---|---|\n",
            self.digest().to_hex()
        );
        for r in &self.rows {
            let (weighted, layer, status) = match &r.result {
                Ok(a) => (
                    format!(
                        "{} {}",
                        rat_text(a.weighted_point),
                        interval_json(a.weighted_interval)
                    ),
                    format!("{} {}", rat_text(a.point), interval_json(a.interval)),
                    "unresolved breakdown".into(),
                ),
                Err(e) => (
                    "—".into(),
                    "—".into(),
                    format!(
                        "unavailable: {} at {}",
                        e.reason,
                        e.at.as_deref().unwrap_or("no cell")
                    ),
                ),
            };
            let mut evidence = vec![format!("Total Gold: {}", r.company.shown())];
            evidence.extend(r.regions.iter().map(|v| {
                format!(
                    "{} (mapped: {}) rate {}; sold {}",
                    v.region,
                    v.mapped,
                    v.rate.shown(),
                    v.sold.shown()
                )
            }));
            evidence.push(format!("region map {}", r.region_map_digest().to_hex()));
            evidence.extend(
                r.sources
                    .iter()
                    .map(|(p, s)| format!("{p} SHA-256 {}", s.to_hex())),
            );
            let cells = [
                format!(
                    "{} / {} / {} / {}",
                    r.release,
                    r.period.code(),
                    r.group.as_str(),
                    r.field
                ),
                weighted,
                layer,
                status,
                evidence.join("; "),
            ];
            let cells = cells
                .iter()
                .map(|s| markdown_text(s))
                .collect::<Vec<_>>()
                .join(" | ");
            let _ = writeln!(out, "| {cells} |");
        }
        out
    }
}

fn markdown_text(value: &str) -> String {
    let mut out = String::new();
    for c in value.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '\n' | '\r' => out.push(' '),
            '\\' | '`' | '*' | '_' | '[' | ']' | '(' | ')' | '|' | '#' => {
                out.push('\\');
                out.push(c);
            }
            c if c.is_control() => out.push(' '),
            _ => out.push(c),
        }
    }
    out
}
