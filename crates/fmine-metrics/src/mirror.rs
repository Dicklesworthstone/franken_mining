//! Exact production, sales and capital-expenditure mirrors of a declared common public perimeter (BD-346).
//! Determinism: Exact
//!
//! DESIGN: source intervals and fixed ownership shares define Scale identities in the existing exact
//! feasibility kernel. Agreement means a common quantity can explain the supplied assertions; it does
//! not establish physical truth, independence of the reporters, or a cost-definition bridge. The caller
//! authenticates source bytes, availability, readership and the stated ownership/perimeter context.

mod capex;
pub mod transcriptions;

pub use capex::{CapexJointReport, CapexReport, compare_capex};

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Debug;

use fmine_claim::{AdditivityClass, Claim, Confidentiality, Representation, SpatialSupport};
use fmine_core::anchor::SourceAnchor;
use fmine_core::{Digest32, Encoder, blake3_hash};
use fmine_evidence::{EpistemicStatus, EvidenceClass, KnowledgeState, Refusal, RefusalCode};
use fmine_numerics::exact_feas::{self as xf, Interval, Rat};
use fmine_time::{CivilDate, days_in_month};

use crate::guidance::{Ownership, Period};

pub const READING_METRIC: &str = "mirror.source_reading";
pub const NO_CLAIM: &str = "Consistency of supplied public assertions under stated ownership and perimeter; no physical-truth, cost-comparability or affiliation claim.";
type Result<T> = std::result::Result<T, Refusal>;
const ZERO: Rat = Rat::from_integer(0);
const ONE: Rat = Rat::from_integer(1);

/// A capital line's declared classification. No reclassification between reporters is inferred.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum CapexScope {
    Sustaining,
    Project,
    Total,
}

impl CapexScope {
    pub const ALL: [Self; 3] = [Self::Sustaining, Self::Project, Self::Total];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Sustaining => "sustaining",
            Self::Project => "project",
            Self::Total => "total",
        }
    }
}

/// Cash paid and accrual additions are different measures, even when their printed values agree.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapexTiming {
    Cash,
    Accrual,
    Unstated,
}

impl CapexTiming {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Cash => "cash",
            Self::Accrual => "accrual",
            Self::Unstated => "unstated",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LeaseTreatment {
    Included,
    Excluded,
    Unstated,
}

impl LeaseTreatment {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Included => "included",
            Self::Excluded => "excluded",
            Self::Unstated => "unstated",
        }
    }
}

/// The separately cited capital definition; every field must be stated before comparison.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CapexBasis {
    pub scope: CapexScope,
    pub timing: CapexTiming,
    pub leases: LeaseTreatment,
}

/// Mirror-local units and quantity kinds. Capital never masquerades as AISC spending or gold ounces.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum MirrorMetric {
    ProductionKoz,
    SoldKoz,
    Capex(CapexScope),
}

impl MirrorMetric {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ProductionKoz => "gold_produced",
            Self::SoldKoz => "gold_sold",
            Self::Capex(CapexScope::Sustaining) => "capex_sustaining",
            Self::Capex(CapexScope::Project) => "capex_project",
            Self::Capex(CapexScope::Total) => "capex_total",
        }
    }

    #[must_use]
    pub const fn unit(self) -> &'static str {
        match self {
            Self::ProductionKoz | Self::SoldKoz => "koz",
            Self::Capex(_) => "USD_m",
        }
    }

    #[must_use]
    pub const fn is_capex(self) -> bool {
        matches!(self, Self::Capex(_))
    }
}

/// An exact extensive amount on explicit temporal, ownership and reporting bases.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MirrorQuantity {
    pub subject: String,
    pub period: Period,
    pub ownership: Ownership,
    pub kind: MirrorMetric,
    pub capex_basis: Option<CapexBasis>,
    pub interval: Interval,
}

impl MirrorQuantity {
    #[must_use]
    pub fn canonical_tag(&self) -> String {
        // DESIGN: existing physical Claims retain their original bytes and identities.
        if !self.kind.is_capex() && self.capex_basis.is_none() {
            return crate::guidance::Quantity {
                subject: self.subject.clone(),
                period: self.period,
                ownership: self.ownership,
                kind: if self.kind == MirrorMetric::ProductionKoz {
                    crate::guidance::QuantityKind::ProductionKoz
                } else {
                    crate::guidance::QuantityKind::SoldKoz
                },
                definition: String::new(),
                interval: self.interval,
            }
            .canonical_tag();
        }
        let mut enc = Encoder::new_frame(*b"MCQ1", 1);
        enc.put_str(&self.subject);
        enc.put_u16(self.period.year);
        enc.put_u8(self.period.first_month);
        enc.put_u8(self.period.last_month);
        match self.ownership {
            Ownership::Full => enc.put_u8(0),
            Ownership::Attributable { share } => {
                enc.put_u8(1);
                enc.put_i128(share.num());
                enc.put_i128(share.den());
            }
        }
        enc.put_str(self.kind.as_str());
        if let Some(basis) = self.capex_basis {
            enc.put_u8(1);
            enc.put_str(basis.scope.as_str());
            enc.put_str(basis.timing.as_str());
            enc.put_str(basis.leases.as_str());
        } else {
            enc.put_u8(0);
        }
        for bound in [self.interval.lo, self.interval.hi] {
            enc.put_i128(bound.num());
            enc.put_i128(bound.den());
        }
        enc.put_u8(u8::from(self.interval.lo_strict));
        enc.put_u8(u8::from(self.interval.hi_strict));
        blake3_hash(&enc.finish()).to_hex()
    }
}

/// Explicit operation membership and inclusive dates over which it and the reading's basis apply.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Perimeter {
    pub id: String,
    pub operations: Vec<String>,
    pub valid_from: CivilDate,
    pub valid_through: CivilDate,
}

impl Perimeter {
    fn encode(&self, enc: &mut Encoder) {
        enc.put_str(&self.id);
        let mut operations = self.operations.clone();
        operations.sort();
        enc.put_u64(operations.len() as u64);
        for operation in operations {
            enc.put_str(&operation);
        }
        for date in [self.valid_from, self.valid_through] {
            enc.put_i64(i64::from(date.year));
            enc.put_u8(date.month);
            enc.put_u8(date.day);
        }
    }
}

/// A source interpretation. `source` identifies the numeric row, separately from context anchors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MirrorReading {
    pub reporter: String,
    pub source: SourceAnchor,
    /// The reporter's own page defining capital scope, timing and lease treatment.
    pub basis_source: Option<SourceAnchor>,
    pub perimeter: Perimeter,
    pub quantity: MirrorQuantity,
}

impl MirrorReading {
    #[must_use]
    pub fn canonical_tag(&self) -> String {
        let mut enc = Encoder::new_frame(*b"MIRD", if self.basis_source.is_some() { 2 } else { 1 });
        enc.put_str(&self.reporter);
        enc.put_str(&self.source.artifact_path);
        enc.put_fixed(&self.source.artifact_sha256);
        enc.put_str(&self.source.locator);
        self.perimeter.encode(&mut enc);
        enc.put_str(&self.quantity.canonical_tag());
        if let Some(source) = &self.basis_source {
            enc.put_str(&source.artifact_path);
            enc.put_fixed(&source.artifact_sha256);
            enc.put_str(&source.locator);
        }
        blake3_hash(&enc.finish()).to_hex()
    }
}

/// The common full-perimeter quantity conditional on every supplied source interpretation.
/// Gold amounts are nonnegative; capital amounts retain their signed monetary domain.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MirrorCommon {
    pub reporters: Vec<String>,
    pub perimeter: Perimeter,
    pub quantity: MirrorQuantity,
}

impl MirrorCommon {
    #[must_use]
    pub fn canonical_tag(&self) -> String {
        let mut enc = Encoder::new_frame(*b"MIRC", 1);
        let mut reporters = self.reporters.clone();
        reporters.sort();
        enc.put_u64(reporters.len() as u64);
        for reporter in reporters {
            enc.put_str(&reporter);
        }
        self.perimeter.encode(&mut enc);
        enc.put_str(&self.quantity.canonical_tag());
        enc.put_u8(u8::from(!self.quantity.kind.is_capex()));
        blake3_hash(&enc.finish()).to_hex()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MirrorVerdict {
    Consistent {
        /// Values in each original reporting basis, keyed by the issuer-qualified proof cell.
        witness: BTreeMap<String, Rat>,
    },
    Conflict {
        certificate: xf::Certificate,
        certificate_digest: String,
        /// An indeterminate blame search does not erase an independently checked exclusion.
        blame: xf::BlameOutcome,
        /// Inclusion-minimal conflicting source sets; indices address `MirrorReport::inputs`.
        minimal_source_sets: Vec<Vec<usize>>,
    },
    Indeterminate {
        reason: String,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct MirrorReport {
    /// Sorted by reporter and Claim ID; normalized readings and proof cells use this same order.
    pub inputs: Vec<Claim<MirrorReading>>,
    /// Exact share division, retaining the original rounding interval even when it crosses zero.
    pub normalized: Vec<Claim<MirrorReading>>,
    pub common: Option<Claim<MirrorCommon>>,
    /// Original, unmodified source bounds represented as interpreted interval cells, not new readings.
    pub cells: Vec<xf::Cell>,
    pub identities: Vec<xf::Identity>,
    pub rules: xf::Rules,
    /// An explicit domain condition on the common quantity, in addition to the source Scale identities.
    pub physical_nonnegative: bool,
    pub verdict: MirrorVerdict,
}

fn refuse<T>(code: RefusalCode, detail: impl Into<String>) -> Result<T> {
    Err(Refusal::new(
        code,
        format!("public mirror: {}", detail.into()),
    ))
}

fn checked<T, E: Debug>(value: std::result::Result<T, E>) -> Result<T> {
    value.map_err(|error| {
        Refusal::new(
            RefusalCode::NumericalCertificateFailed207,
            format!("public mirror exact calculation/check failed: {error:?}"),
        )
    })
}

fn midpoint(interval: Interval) -> Result<Rat> {
    let half_width =
        checked(checked(interval.hi.checked_sub(interval.lo))?.checked_div(Rat::from_integer(2)))?;
    checked(interval.lo.checked_add(half_width))
}

fn share(quantity: &MirrorQuantity) -> Rat {
    match quantity.ownership {
        Ownership::Full => ONE,
        Ownership::Attributable { share } => share,
    }
}

fn validate(claim: &Claim<MirrorReading>) -> Result<()> {
    claim.check_id()?;
    if claim.value_tag() != claim.value.canonical_tag() {
        return refuse(
            RefusalCode::FrozenRecordMutation507,
            "reading differs from its canonical value tag",
        );
    }
    if claim.metric_key != READING_METRIC {
        return refuse(
            RefusalCode::BasisMismatch105,
            format!("input metric must be {READING_METRIC}"),
        );
    }
    if !claim.confidentiality.is_public() {
        return refuse(
            RefusalCode::MnpiWall301,
            "this mirror admits public inputs only",
        );
    }
    if claim.stale || claim.state == KnowledgeState::Stale {
        return refuse(RefusalCode::StaleAnchor502, "stale source interpretation");
    }
    if matches!(claim.representation, Representation::Placeholder { .. }) {
        return refuse(
            RefusalCode::SourceCellUnusable208,
            "a placeholder is not a numeric mirror reading",
        );
    }
    if !matches!(
        claim.state,
        KnowledgeState::Known | KnowledgeState::Estimated
    ) {
        return refuse(
            RefusalCode::NotObservable202,
            "source interpretation is neither Known nor Estimated",
        );
    }
    if claim.calibration_failed {
        return refuse(
            RefusalCode::CalibrationRequired205,
            "failed-calibration source",
        );
    }
    if claim.representation != Representation::Deterministic {
        return refuse(
            RefusalCode::OutsideAoa203,
            "the mirror requires deterministic closed source bounds",
        );
    }
    if claim.support != SpatialSupport::PeriodAggregate {
        return refuse(
            RefusalCode::SupportMismatch102,
            "a reading must be a period aggregate",
        );
    }
    if claim.additivity != AdditivityClass::ExtensiveAdditive {
        return refuse(
            RefusalCode::NonAdditiveAverage103,
            "mirror amounts are extensive quantities",
        );
    }
    let reading = &claim.value;
    if !claim.anchors.contains(&reading.source)
        || claim.anchors.iter().any(|a| {
            a.artifact_path.trim().is_empty()
                || a.locator.trim().is_empty()
                || a.artifact_sha256 == [0; 32]
        })
    {
        return refuse(
            RefusalCode::NoClaimInDomain201,
            "numeric source and every context anchor need a locator and digest",
        );
    }
    let q = &reading.quantity;
    match q.kind {
        MirrorMetric::ProductionKoz | MirrorMetric::SoldKoz => {
            if q.capex_basis.is_some() || reading.basis_source.is_some() {
                return refuse(
                    RefusalCode::BasisMismatch105,
                    "physical gold quantities cannot carry a capital definition",
                );
            }
        }
        MirrorMetric::Capex(scope) => {
            let Some(basis) = q.capex_basis else {
                return refuse(
                    RefusalCode::NonIdentifiableAttribution104,
                    "missing capital expenditure definition",
                );
            };
            if basis.scope != scope {
                return refuse(
                    RefusalCode::BasisMismatch105,
                    "capital scope differs from the requested metric",
                );
            }
            if basis.timing == CapexTiming::Unstated || basis.leases == LeaseTreatment::Unstated {
                return refuse(
                    RefusalCode::NonIdentifiableAttribution104,
                    if basis.timing == CapexTiming::Unstated {
                        "missing capex timing basis"
                    } else {
                        "missing capex lease treatment"
                    },
                );
            }
            if !reading.basis_source.as_ref().is_some_and(|source| {
                claim.anchors.contains(source)
                    && source.artifact_path == reading.source.artifact_path
                    && source.artifact_sha256 == reading.source.artifact_sha256
            }) {
                return refuse(
                    RefusalCode::NonIdentifiableAttribution104,
                    "capex definition needs a cited page in this reporter's source document",
                );
            }
        }
    }
    if reading.reporter.is_empty()
        || !reading
            .reporter
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_' || b == b'-')
        || q.subject.trim().is_empty()
        || q.subject != q.subject.trim()
    {
        return refuse(
            RefusalCode::UndeclaredUnitBasisOrTz107,
            "declare a canonical reporter key and asset",
        );
    }
    let p = q.period;
    if p.year == 0
        || p.year > 9999
        || p.first_month == 0
        || p.first_month > p.last_month
        || p.last_month > 12
    {
        return refuse(
            RefusalCode::SupportMismatch102,
            "invalid calendar-month support",
        );
    }
    let perimeter = &reading.perimeter;
    let members: BTreeSet<_> = perimeter.operations.iter().collect();
    if perimeter.id.trim().is_empty()
        || perimeter.id != perimeter.id.trim()
        || members.is_empty()
        || members.len() != perimeter.operations.len()
        || members
            .iter()
            .any(|s| s.trim().is_empty() || s.as_str() != s.trim())
    {
        return refuse(
            RefusalCode::NonIdentifiableAttribution104,
            "declare a unique, complete common-perimeter operation list and identifier",
        );
    }
    for date in [perimeter.valid_from, perimeter.valid_through] {
        if !(1..=9999).contains(&date.year)
            || CivilDate::new(date.year, date.month, date.day).is_err()
        {
            return refuse(
                RefusalCode::SupportMismatch102,
                "invalid perimeter/basis validity date",
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
    if perimeter.valid_from > start || perimeter.valid_through < end {
        return refuse(
            RefusalCode::NonIdentifiableAttribution104,
            "ownership/perimeter validity does not cover the whole quantity period; no straddle or interpolation",
        );
    }
    if share(q).signum() <= 0 || checked(share(q).checked_cmp(ONE))?.is_gt() {
        return refuse(
            RefusalCode::BasisMismatch105,
            "attributable ownership must be an exact fraction in (0,1]",
        );
    }
    if q.interval.lo_strict || q.interval.hi_strict {
        return refuse(
            RefusalCode::OutsideAoa203,
            "only explicitly Closed source-rounding bounds are supported",
        );
    }
    if checked(q.interval.lo.checked_cmp(q.interval.hi))?.is_gt()
        || (!q.kind.is_capex() && midpoint(q.interval)?.signum() < 0)
    {
        return refuse(
            RefusalCode::OutsidePhysicalDomain109,
            "reversed interval or negative reported physical quantity midpoint",
        );
    }
    Ok(())
}

fn derived<T>(metric: &str, value: T, tag: &str, sources: &[&Claim<MirrorReading>]) -> Claim<T> {
    let mut evidence = EvidenceClass::Derived;
    let mut epistemic = EpistemicStatus::KnownKnown;
    let mut state = KnowledgeState::Known;
    let mut confidentiality = Confidentiality::public();
    let mut uu_terms = Vec::new();
    let mut anchors = Vec::new();
    let mut parents = Vec::new();
    for source in sources {
        evidence = evidence.meet(source.evidence);
        epistemic = epistemic.and(source.epistemic);
        state = state.and(source.state);
        confidentiality = confidentiality.join(&source.confidentiality);
        for term in &source.uu_terms {
            if !uu_terms.contains(term) {
                uu_terms.push(term.clone());
            }
        }
        anchors.extend(source.anchors.iter().cloned());
        parents.push(source.claim_id);
    }
    anchors.sort();
    anchors.dedup();
    parents.sort();
    parents.dedup();
    Claim::new(
        metric,
        value,
        tag,
        Representation::Deterministic,
        SpatialSupport::PeriodAggregate,
        AdditivityClass::ExtensiveAdditive,
        evidence,
        epistemic,
        state,
        confidentiality,
        uu_terms,
        parents,
        anchors,
    )
}

fn normalize(input: &Claim<MirrorReading>) -> Result<Claim<MirrorReading>> {
    let r = &input.value;
    let mut value = r.clone();
    value.quantity.ownership = Ownership::Full;
    value.quantity.interval.lo = checked(r.quantity.interval.lo.checked_div(share(&r.quantity)))?;
    value.quantity.interval.hi = checked(r.quantity.interval.hi.checked_div(share(&r.quantity)))?;
    let tag = value.canonical_tag();
    Ok(derived("mirror.normalized_full", value, &tag, &[input]))
}

fn proof_cell(input: &Claim<MirrorReading>) -> Result<xf::Cell> {
    let r = &input.value;
    Ok(xf::Cell {
        locator: format!("{}:{}:{}", r.reporter, input.claim_id, r.source.locator),
        sha256: Digest32(r.source.artifact_sha256).to_hex(),
        token: format!("interpreted Claim {} closed interval", input.claim_id),
        printed: midpoint(r.quantity.interval)?,
        step: checked(r.quantity.interval.hi.checked_sub(r.quantity.interval.lo))?,
        scope: if r.source.locator.contains(":p") {
            xf::Scope::Pdf
        } else {
            xf::Scope::Visible
        },
    })
}

/// Compare two or more cited readings. A reporter may supply full and attributable figures, but at
/// least two distinct reporters are required. Their attributable shares need not sum to one.
pub fn compare(inputs: &[Claim<MirrorReading>]) -> Result<MirrorReport> {
    compare_with_caps(inputs, xf::MAX_VARIABLES, xf::MAX_PIVOTS)
}

fn compare_with_caps(
    inputs: &[Claim<MirrorReading>],
    max_variables: usize,
    max_pivots: usize,
) -> Result<MirrorReport> {
    let mut inputs = inputs.to_vec();
    inputs.sort_by(|a, b| (&a.value.reporter, a.claim_id).cmp(&(&b.value.reporter, b.claim_id)));
    for input in &inputs {
        validate(input)?;
    }
    let reporters: BTreeSet<_> = inputs.iter().map(|c| c.value.reporter.clone()).collect();
    if inputs.len() < 2 || reporters.len() < 2 {
        return refuse(
            RefusalCode::NoClaimInDomain201,
            "at least two distinct reporters with usable source readings are required",
        );
    }
    let first = &inputs[0].value;
    let mut perimeter = first.perimeter.clone();
    perimeter.operations.sort();
    let mut unique_sources = BTreeSet::new();
    let mut shares = BTreeMap::new();
    for input in &inputs {
        let r = &input.value;
        if !unique_sources.insert((&r.reporter, &r.source)) {
            return refuse(
                RefusalCode::NonIdentifiableAttribution104,
                "one reporter's numeric source row was supplied twice",
            );
        }
        if r.quantity.subject != first.quantity.subject || r.quantity.kind != first.quantity.kind {
            return refuse(
                RefusalCode::BasisMismatch105,
                "reporters must name the same asset and quantity kind",
            );
        }
        if r.quantity.period != first.quantity.period {
            return refuse(
                RefusalCode::SupportMismatch102,
                "reporters cover different periods; no annualization or calendar bridge",
            );
        }
        if r.quantity.capex_basis != first.quantity.capex_basis {
            return refuse(
                RefusalCode::NonIdentifiableAttribution104,
                "capex definitions differ: an explicit cited timing or lease bridge would be required",
            );
        }
        let mut operations = r.perimeter.operations.clone();
        operations.sort();
        if r.perimeter.id != perimeter.id || operations != perimeter.operations {
            return refuse(
                RefusalCode::NonIdentifiableAttribution104,
                "common perimeter is unresolved: both operation lists and identifiers must match",
            );
        }
        perimeter.valid_from = perimeter.valid_from.max(r.perimeter.valid_from);
        perimeter.valid_through = perimeter.valid_through.min(r.perimeter.valid_through);
        if let Ownership::Attributable { share } = r.quantity.ownership
            && shares
                .insert(&r.reporter, share)
                .is_some_and(|old| old != share)
        {
            return refuse(
                RefusalCode::BasisMismatch105,
                "one reporter supplies incompatible ownership shares over the same period/perimeter",
            );
        }
    }
    let mut normalized = Vec::new();
    let mut cells = Vec::new();
    for input in &inputs {
        normalized.push(normalize(input)?);
        cells.push(proof_cell(input)?);
    }
    let mut identities = Vec::new();
    for i in 1..inputs.len() {
        identities.push(xf::Identity {
            id: format!("mirror-scale-{i}"),
            family: "mirror_ownership".into(),
            closure: xf::Closure::Definitional,
            kind: xf::IdentityKind::Scale {
                target: cells[i].locator.clone(),
                source: cells[0].locator.clone(),
                factor: checked(
                    share(&inputs[i].value.quantity).checked_div(share(&first.quantity)),
                )?,
            },
        });
    }
    let rules = xf::Rules::new(xf::Convention::Closed);
    let components = checked(xf::solve_with_caps(
        &cells,
        &identities,
        &rules,
        max_variables,
        max_pivots,
    ))?;
    let [component] = components.as_slice() else {
        return refuse(
            RefusalCode::NumericalCertificateFailed207,
            "mirror must form exactly one connected component",
        );
    };
    let mut common = None;
    let verdict = match &component.outcome {
        xf::Outcome::Feasible { .. } => {
            let mut interval = normalized[0].value.quantity.interval;
            interval.lo = if !first.quantity.kind.is_capex() && interval.lo.signum() < 0 {
                ZERO
            } else {
                interval.lo
            };
            for input in &normalized[1..] {
                let next = input.value.quantity.interval;
                if checked(next.lo.checked_cmp(interval.lo))?.is_gt() {
                    interval.lo = next.lo;
                }
                if checked(next.hi.checked_cmp(interval.hi))?.is_lt() {
                    interval.hi = next.hi;
                }
            }
            if checked(interval.lo.checked_cmp(interval.hi))?.is_gt() {
                return refuse(
                    RefusalCode::NumericalCertificateFailed207,
                    "checked feasible source boxes have no common interval in the declared domain",
                );
            }
            let witness: BTreeMap<_, _> = cells
                .iter()
                .zip(&inputs)
                .map(|(cell, input)| {
                    Ok((
                        cell.locator.clone(),
                        checked(interval.lo.checked_mul(share(&input.value.quantity)))?,
                    ))
                })
                .collect::<Result<_>>()?;
            checked(xf::check_witness(&cells, &identities, &rules, &witness))?;
            if !first.quantity.kind.is_capex() && witness.values().any(|q| q.signum() < 0) {
                return refuse(
                    RefusalCode::NumericalCertificateFailed207,
                    "physical witness is negative",
                );
            }
            let mut quantity = first.quantity.clone();
            quantity.ownership = Ownership::Full;
            quantity.interval = interval;
            let value = MirrorCommon {
                reporters: reporters.into_iter().collect(),
                perimeter,
                quantity,
            };
            let tag = value.canonical_tag();
            common = Some(derived(
                "mirror.common_full",
                value,
                &tag,
                &inputs.iter().collect::<Vec<_>>(),
            ));
            MirrorVerdict::Consistent { witness }
        }
        xf::Outcome::Infeasible { certificate } => {
            checked(xf::check_certificate(
                &cells,
                &identities,
                &rules,
                certificate,
            ))?;
            let certificate_digest = checked(xf::certificate_digest(
                &cells,
                &identities,
                &rules,
                certificate,
            ))?;
            let blame = checked(xf::blame(&cells, &identities, &rules))?;
            let [blame] = blame.as_slice() else {
                return refuse(
                    RefusalCode::NumericalCertificateFailed207,
                    "conflict blame did not name exactly one component",
                );
            };
            let mut minimal_source_sets = Vec::new();
            if let xf::BlameOutcome::Blamed(b) = &blame.outcome {
                for set in &b.conflicts {
                    checked(xf::check_conflict_set(&cells, &identities, &rules, set))?;
                    let mut indices = Vec::new();
                    for locator in &set.cells {
                        let Some(index) = cells.iter().position(|c| c.locator == *locator) else {
                            return refuse(
                                RefusalCode::NumericalCertificateFailed207,
                                "blame names an unknown source cell",
                            );
                        };
                        indices.push(index);
                    }
                    // In one dimension, nonempty closed intervals have an inclusion-minimal conflict
                    // exactly when two of them are disjoint. Check minimality independently of deletion.
                    let [a, b] = indices.as_slice() else {
                        return refuse(
                            RefusalCode::NumericalCertificateFailed207,
                            "a minimal interval conflict must name exactly two source cells",
                        );
                    };
                    let (a, b) = (
                        normalized[*a].value.quantity.interval,
                        normalized[*b].value.quantity.interval,
                    );
                    if !checked(a.hi.checked_cmp(b.lo))?.is_lt()
                        && !checked(b.hi.checked_cmp(a.lo))?.is_lt()
                    {
                        return refuse(
                            RefusalCode::NumericalCertificateFailed207,
                            "claimed minimal source pair still overlaps",
                        );
                    }
                    indices.sort();
                    minimal_source_sets.push(indices);
                }
            }
            minimal_source_sets.sort();
            minimal_source_sets.dedup();
            MirrorVerdict::Conflict {
                certificate: certificate.clone(),
                certificate_digest,
                blame: blame.outcome.clone(),
                minimal_source_sets,
            }
        }
        xf::Outcome::Indeterminate { reason } => MirrorVerdict::Indeterminate {
            reason: reason.clone(),
        },
        xf::Outcome::NotRefuted => MirrorVerdict::Indeterminate {
            reason: "a relaxation supplies no checked mirror witness".into(),
        },
    };
    let physical_nonnegative = !first.quantity.kind.is_capex();
    Ok(MirrorReport {
        inputs,
        normalized,
        common,
        cells,
        identities,
        rules,
        physical_nonnegative,
        verdict,
    })
}

#[cfg(test)]
mod tests {
    include!("../../xtask/testdata/metrics_mirror_controls.rs");
}
