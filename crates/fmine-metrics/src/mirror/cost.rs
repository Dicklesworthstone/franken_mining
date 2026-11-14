//! DESIGN (BD-346.S5): exact dollar-cost mirrors with explicitly cited, signed definition bridges.
//! These compare extensive USD-million amounts, never implied per-ounce rates. Every applied bridge
//! has a reading from each reporter; missing definitions or adjustment amounts are not inferred.
//! Determinism: Exact

use super::*;
use fmine_registry::sources::DocumentRow;
use fmine_registry::transcriptions::{Accepted, Locator};

pub mod bind;

pub const SOURCE_METRIC: &str = "mirror.cost_source";
pub const NO_CLAIM: &str = "Conditional compatibility of supplied USD-million cost assertions and signed, source-cited adjustments on one dated asset perimeter. No per-ounce rate, physical truth, accounting reconciliation, bridge transfer, disclosure permission or issuer affiliation is established.";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum CostMetric {
    CashCosts,
    Aisc,
    CostOfSales,
}

impl CostMetric {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::CashCosts => "cash_costs",
            Self::Aisc => "aisc",
            Self::CostOfSales => "cost_of_sales",
        }
    }
}

/// Each dimension is explicit; dollar adjustments cannot bridge denominator or allocation policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum DefinitionField {
    Metal,
    Denominator,
    Allocation,
    Credits,
    Inventory,
    Ga,
    Reclamation,
    Accounting,
}

impl DefinitionField {
    pub const ALL: [Self; 8] = [
        Self::Metal,
        Self::Denominator,
        Self::Allocation,
        Self::Credits,
        Self::Inventory,
        Self::Ga,
        Self::Reclamation,
        Self::Accounting,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Metal => "metal",
            Self::Denominator => "denominator",
            Self::Allocation => "allocation",
            Self::Credits => "credits",
            Self::Inventory => "inventory",
            Self::Ga => "ga",
            Self::Reclamation => "reclamation",
            Self::Accounting => "accounting",
        }
    }

    fn admits(self, value: &str) -> bool {
        match self {
            Self::Metal => matches!(value, "gold" | "copper" | "silver" | "tin"),
            Self::Denominator => matches!(value, "sold" | "produced"),
            Self::Allocation => matches!(value, "by_product" | "co_product"),
            Self::Credits => matches!(value, "separate_line" | "inside_selling_costs" | "excluded"),
            Self::Inventory | Self::Ga => matches!(value, "included" | "excluded"),
            Self::Reclamation => matches!(value, "operating_only" | "all_sites" | "excluded"),
            Self::Accounting => matches!(
                value,
                "as_reported" | "historical_cost" | "purchase_price_allocation"
            ),
        }
    }

    fn bridgeable(self, metric: CostMetric) -> bool {
        match self {
            Self::Metal | Self::Denominator | Self::Allocation => false,
            Self::Accounting => metric == CostMetric::CostOfSales,
            _ => true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CostDefinition {
    pub fields: BTreeMap<DefinitionField, String>,
}

impl CostDefinition {
    fn validate(&self) -> Result<()> {
        for field in DefinitionField::ALL {
            if !self.fields.get(&field).is_some_and(|v| field.admits(v)) {
                return refuse(
                    RefusalCode::NonIdentifiableAttribution104,
                    format!("missing or unsupported cost definition: {}", field.as_str()),
                );
            }
        }
        Ok(())
    }
}

/// The entire semantic support participates in the tag of both source and derived cost Claims.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CostValue {
    pub asset: String,
    pub period: Period,
    pub metric: CostMetric,
    pub perimeter: Perimeter,
    pub definition: CostDefinition,
    pub interval: xf::Interval,
}

impl CostValue {
    #[must_use]
    pub fn canonical_tag(&self) -> String {
        let mut e = Encoder::new_frame(*b"JVCD", 1);
        e.put_str(&self.asset);
        e.put_str(self.metric.as_str());
        e.put_u16(self.period.year);
        e.put_u8(self.period.first_month);
        e.put_u8(self.period.last_month);
        self.perimeter.encode(&mut e);
        for field in DefinitionField::ALL {
            e.put_str(
                self.definition
                    .fields
                    .get(&field)
                    .map_or("", String::as_str),
            );
        }
        for v in [self.interval.lo, self.interval.hi] {
            e.put_i128(v.num());
            e.put_i128(v.den());
        }
        e.put_u8(u8::from(self.interval.lo_strict));
        e.put_u8(u8::from(self.interval.hi_strict));
        blake3_hash(&e.finish()).to_hex()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CostLayer {
    pub field: DefinitionField,
    /// The common target treatment cited in this reporter's own reconciliation.
    pub target: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CostReading {
    pub reporter: String,
    pub source: SourceAnchor,
    pub definition_source: SourceAnchor,
    pub ownership: Ownership,
    pub value: CostValue,
    /// None is the original total; Some is a signed amount added to it on the same ownership basis.
    pub layer: Option<CostLayer>,
    /// Original reader-agreed interpretation and publication evidence, committed without normalization.
    pub basis: String,
    pub publication: String,
    pub raw_token: String,
    pub readers: Vec<String>,
}

impl CostReading {
    #[must_use]
    pub fn canonical_tag(&self) -> String {
        let mut e = Encoder::new_frame(*b"JVCS", 1);
        e.put_str(&self.reporter);
        e.put_str(&self.value.canonical_tag());
        for a in [&self.source, &self.definition_source] {
            e.put_str(&a.artifact_path);
            e.put_fixed(&a.artifact_sha256);
            e.put_str(&a.locator);
        }
        match self.ownership {
            Ownership::Full => e.put_u8(0),
            Ownership::Attributable { share } => {
                e.put_u8(1);
                e.put_i128(share.num());
                e.put_i128(share.den());
            }
        }
        if let Some(layer) = &self.layer {
            e.put_u8(1);
            e.put_str(layer.field.as_str());
            e.put_str(&layer.target);
        } else {
            e.put_u8(0);
        }
        e.put_str(&self.basis);
        e.put_str(&self.publication);
        e.put_str(&self.raw_token);
        let mut readers = self.readers.clone();
        readers.sort();
        e.put_u64(readers.len() as u64);
        for reader in readers {
            e.put_str(&reader);
        }
        blake3_hash(&e.finish()).to_hex()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct BoundCosts {
    pub ownership: Option<Claim<Rat>>,
    pub inputs: Vec<Claim<CostReading>>,
}

/// Exact separation certificate: the lower reporter's least possible cost exceeds the other's greatest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Separation {
    pub lower_reporter: String,
    pub lower: Rat,
    pub upper_reporter: String,
    pub upper: Rat,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CostReport {
    pub inputs: Vec<Claim<CostReading>>,
    /// Sorted reporter keys, each with its exact extensive amount on the agreed definition and 100% basis.
    pub aligned: Vec<(String, Claim<CostValue>)>,
    pub common: Option<Claim<CostValue>>,
    pub layers: Vec<DefinitionField>,
    pub separation: Option<Separation>,
}

impl CostReport {
    #[must_use]
    pub fn status(&self) -> &'static str {
        if self.separation.is_some() {
            "conflict"
        } else if self.layers.is_empty() {
            "consistent"
        } else {
            "explained"
        }
    }
}

fn validate(input: &Claim<CostReading>) -> Result<()> {
    input.check_id()?;
    let r = &input.value;
    let v = &r.value;
    if input.value_tag() != r.canonical_tag() {
        return refuse(
            RefusalCode::FrozenRecordMutation507,
            "cost reading differs from its canonical tag",
        );
    }
    if input.metric_key != SOURCE_METRIC
        || input.representation != Representation::Deterministic
        || input.support != SpatialSupport::PeriodAggregate
        || input.additivity != AdditivityClass::ExtensiveAdditive
    {
        return refuse(
            RefusalCode::SupportMismatch102,
            "cost source needs deterministic extensive period-aggregate support",
        );
    }
    if !input.confidentiality.is_public() {
        return refuse(
            RefusalCode::MnpiWall301,
            "public cost mirror cannot admit private inputs",
        );
    }
    if input.stale || input.state == KnowledgeState::Stale {
        return refuse(RefusalCode::StaleAnchor502, "stale cost assertion");
    }
    if input.calibration_failed
        || !matches!(
            input.state,
            KnowledgeState::Known | KnowledgeState::Estimated
        )
    {
        return refuse(RefusalCode::NotObservable202, "unusable cost source state");
    }
    if [&r.source, &r.definition_source]
        .into_iter()
        .any(|a| !input.anchors.contains(a))
        || input.anchors.iter().any(|a| {
            a.artifact_path.is_empty() || a.locator.is_empty() || a.artifact_sha256 == [0; 32]
        })
        || r.source.artifact_path != r.definition_source.artifact_path
        || r.source.artifact_sha256 != r.definition_source.artifact_sha256
    {
        return refuse(
            RefusalCode::NonIdentifiableAttribution104,
            "cost and definition need citations in this reporter's own document",
        );
    }
    if r.reporter.is_empty()
        || v.asset.trim().is_empty()
        || r.basis.is_empty()
        || r.publication.is_empty()
    {
        return refuse(
            RefusalCode::NonIdentifiableAttribution104,
            "missing cost reporter, asset or source interpretation",
        );
    }
    v.definition.validate()?;
    let readers: BTreeSet<_> = r.readers.iter().collect();
    if readers.len() < 2
        || readers.len() != r.readers.len()
        || readers.iter().any(|reader| reader.trim().is_empty())
    {
        return refuse(
            RefusalCode::NotObservable202,
            "cost token requires distinct named readers",
        );
    }
    let fmine_registry::transcriptions::Token::Point(decimal) =
        fmine_registry::transcriptions::parse_token(&r.raw_token)
            .map_err(|reason| Refusal::new(RefusalCode::SourceCellUnusable208, reason))?
    else {
        return refuse(
            RefusalCode::SourceCellUnusable208,
            "cost source token must be a printed point",
        );
    };
    let denominator = 10_i128.checked_pow(decimal.scale).ok_or_else(|| {
        Refusal::new(
            RefusalCode::NumericalCertificateFailed207,
            "cost token scale overflow",
        )
    })?;
    let point = checked(Rat::new(decimal.mantissa, denominator))?;
    let half = checked(checked(Rat::new(1, denominator))?.checked_div(Rat::from_integer(2)))?;
    if v.interval.lo != checked(point.checked_sub(half))?
        || v.interval.hi != checked(point.checked_add(half))?
    {
        return refuse(
            RefusalCode::FrozenRecordMutation507,
            "cost bounds do not match the original printed token",
        );
    }
    let p = v.period;
    if p.year == 0
        || p.year > 9999
        || p.first_month == 0
        || p.first_month > p.last_month
        || p.last_month > 12
    {
        return refuse(RefusalCode::SupportMismatch102, "invalid cost period");
    }
    let members: BTreeSet<_> = v.perimeter.operations.iter().collect();
    if members.is_empty()
        || members.len() != v.perimeter.operations.len()
        || v.perimeter.id.is_empty()
        || members.iter().any(|s| s.trim().is_empty())
    {
        return refuse(
            RefusalCode::NonIdentifiableAttribution104,
            "cost perimeter needs unique, nonempty operations",
        );
    }
    for date in [v.perimeter.valid_from, v.perimeter.valid_through] {
        if !(1..=9999).contains(&date.year)
            || CivilDate::new(date.year, date.month, date.day).is_err()
        {
            return refuse(
                RefusalCode::SupportMismatch102,
                "invalid cost perimeter date",
            );
        }
    }
    let start = CivilDate {
        year: i32::from(p.year),
        month: p.first_month,
        day: 1,
    };
    let end = CivilDate {
        year: i32::from(p.year),
        month: p.last_month,
        day: days_in_month(i32::from(p.year), p.last_month),
    };
    if v.perimeter.valid_from > start || v.perimeter.valid_through < end {
        return refuse(
            RefusalCode::NonIdentifiableAttribution104,
            "cost ownership/perimeter must cover the whole period",
        );
    }
    if let Ownership::Attributable { share } = r.ownership
        && (share.signum() <= 0 || checked(share.checked_cmp(ONE))?.is_gt())
    {
        return refuse(
            RefusalCode::BasisMismatch105,
            "cost ownership fraction must be in (0,1]",
        );
    }
    if v.interval.lo_strict
        || v.interval.hi_strict
        || checked(v.interval.lo.checked_cmp(v.interval.hi))?.is_gt()
    {
        return refuse(
            RefusalCode::OutsideAoa203,
            "costs require ordered closed rounding intervals",
        );
    }
    if let Some(layer) = &r.layer
        && (!layer.field.bridgeable(v.metric) || !layer.field.admits(&layer.target))
    {
        return refuse(
            RefusalCode::NonIdentifiableAttribution104,
            format!(
                "{} cannot use the {} adjustment to {}",
                v.metric.as_str(),
                layer.field.as_str(),
                layer.target
            ),
        );
    }
    Ok(())
}

/// Compare one original amount per reporter plus the explicitly supplied signed dollar adjustments.
/// All cost-definition dimensions must agree after those adjustments. Every layer requires both sides'
/// own cited amount, including an explicitly printed zero when a reporter needs no adjustment.
pub fn compare(inputs: &[Claim<CostReading>]) -> Result<CostReport> {
    let mut inputs = inputs.to_vec();
    inputs.sort_by(|a, b| {
        (
            &a.value.reporter,
            &a.value.layer.as_ref().map(|l| l.field),
            a.claim_id,
        )
            .cmp(&(
                &b.value.reporter,
                &b.value.layer.as_ref().map(|l| l.field),
                b.claim_id,
            ))
    });
    let Some(first) = inputs.first() else {
        return refuse(RefusalCode::NoClaimInDomain201, "no cost assertions");
    };
    let mut reporters = BTreeMap::<String, Vec<&Claim<CostReading>>>::new();
    let mut locators = BTreeSet::new();
    for input in &inputs {
        validate(input)?;
        let v = &input.value.value;
        let f = &first.value.value;
        if v.asset != f.asset
            || v.period != f.period
            || v.metric != f.metric
            || v.perimeter != f.perimeter
        {
            return refuse(
                RefusalCode::SupportMismatch102,
                "costs and bridges must share asset, period, metric and complete dated perimeter",
            );
        }
        if !locators.insert((
            &input.value.source.artifact_path,
            input.value.source.artifact_sha256,
            &input.value.source.locator,
        )) {
            return refuse(
                RefusalCode::NonIdentifiableAttribution104,
                "one printed source row cannot supply multiple cost terms",
            );
        }
        reporters
            .entry(input.value.reporter.clone())
            .or_default()
            .push(input);
    }
    if reporters.len() != 2 {
        return refuse(
            RefusalCode::NonIdentifiableAttribution104,
            "cost mirror requires two distinct reporters",
        );
    }
    let mut aligned = Vec::new();
    let mut common_layers = None;
    for (reporter, rows) in &reporters {
        let originals: Vec<_> = rows.iter().filter(|r| r.value.layer.is_none()).collect();
        let [original] = originals.as_slice() else {
            return refuse(
                RefusalCode::NonIdentifiableAttribution104,
                format!("{reporter} requires exactly one original cost assertion"),
            );
        };
        let source = &original.value;
        let mut value = source.value.clone();
        let mut layers = BTreeMap::new();
        for row in rows.iter().filter(|r| r.value.layer.is_some()) {
            let Some(layer) = &row.value.layer else {
                continue;
            };
            if row.value.ownership != source.ownership
                || row.value.value.definition != source.value.definition
                || row.value.source.artifact_path != source.source.artifact_path
                || row.value.source.artifact_sha256 != source.source.artifact_sha256
            {
                return refuse(
                    RefusalCode::BasisMismatch105,
                    "adjustments need the original amount's ownership, definition and source document",
                );
            }
            if layers.insert(layer.field, layer.target.clone()).is_some() {
                return refuse(
                    RefusalCode::NonIdentifiableAttribution104,
                    "duplicate cost adjustment field",
                );
            }
            value
                .definition
                .fields
                .insert(layer.field, layer.target.clone());
            value.interval.lo =
                checked(value.interval.lo.checked_add(row.value.value.interval.lo))?;
            value.interval.hi =
                checked(value.interval.hi.checked_add(row.value.value.interval.hi))?;
        }
        if common_layers.as_ref().is_some_and(|prior| prior != &layers) {
            return refuse(
                RefusalCode::NonIdentifiableAttribution104,
                "each cost bridge needs both reporters' own signed amount and identical target treatment",
            );
        }
        common_layers = Some(layers);
        if let Ownership::Attributable { share } = source.ownership {
            value.interval.lo = checked(value.interval.lo.checked_div(share))?;
            value.interval.hi = checked(value.interval.hi.checked_div(share))?;
        }
        if aligned
            .first()
            .is_some_and(|(_, prior): &(String, Claim<CostValue>)| {
                prior.value.definition != value.definition
            })
        {
            let fields = DefinitionField::ALL
                .iter()
                .filter(|f| {
                    aligned[0].1.value.definition.fields.get(f) != value.definition.fields.get(f)
                })
                .map(|f| f.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            return refuse(
                RefusalCode::NonIdentifiableAttribution104,
                format!("cost definitions differ without an admissible cited bridge: {fields}"),
            );
        }
        let tag = value.canonical_tag();
        aligned.push((
            reporter.clone(),
            derived("mirror.cost_aligned_full", value, &tag, rows),
        ));
    }
    let a = aligned[0].1.value.interval;
    let b = aligned[1].1.value.interval;
    let (lower_i, lower) = if checked(a.lo.checked_cmp(b.lo))?.is_ge() {
        (0, a.lo)
    } else {
        (1, b.lo)
    };
    let (upper_i, upper) = if checked(a.hi.checked_cmp(b.hi))?.is_le() {
        (0, a.hi)
    } else {
        (1, b.hi)
    };
    let (common, separation) = if checked(lower.checked_cmp(upper))?.is_gt() {
        (
            None,
            Some(Separation {
                lower_reporter: aligned[lower_i].0.clone(),
                lower,
                upper_reporter: aligned[upper_i].0.clone(),
                upper,
            }),
        )
    } else {
        let mut value = aligned[0].1.value.clone();
        value.interval = xf::Interval {
            lo: lower,
            hi: upper,
            lo_strict: false,
            hi_strict: false,
        };
        let tag = value.canonical_tag();
        (
            Some(derived(
                "mirror.cost_common_full",
                value,
                &tag,
                &[&aligned[0].1, &aligned[1].1],
            )),
            None,
        )
    };
    Ok(CostReport {
        inputs,
        aligned,
        common,
        layers: common_layers.unwrap_or_default().into_keys().collect(),
        separation,
    })
}

#[cfg(test)]
mod tests {
    include!("../../../xtask/testdata/metrics_mirror_cost_controls.rs");
}
