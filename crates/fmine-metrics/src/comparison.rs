//! Source-bound peer comparisons on identical declared definitions (BD-355).
//! Determinism: Exact
//!
//! Ranks use competition ranking: equal values share a rank and the next rank skips the tied
//! positions. Closed display-rounding intervals are not probability distributions. All declared
//! peers participate; missing evidence or a definition mismatch refuses the comparison.

use std::collections::{BTreeMap, BTreeSet};

use fmine_claim::{AdditivityClass, Claim, Confidentiality, Representation, SpatialSupport};
use fmine_core::{Encoder, ObjectId, anchor::SourceAnchor, blake3_hash};
use fmine_evidence::{EpistemicStatus, EvidenceClass, KnowledgeState, Refusal, RefusalCode};
use fmine_numerics::exact_feas::{Interval, Rat};
use fmine_registry::sources::is_date;

pub mod source;
#[cfg(test)]
mod tests;

pub const INPUT_METRIC: &str = "peer.comparison.source/1";
pub const OUTPUT_METRIC: &str = "peer.comparison.ranks/1";
pub const NO_CLAIM: &str = "Comparison of the complete selected peer set on matching source-cited definitions, from public filings; no affiliation implied. Rounding bounds are not confidence intervals. No transferred bridge, proprietary industry cost curve, disclosure permission or independently verified physical or accounting truth is supplied.";
pub const MAX_PEERS: usize = 64;
type Result<T> = std::result::Result<T, Refusal>;

fn fail(code: RefusalCode, reason: impl Into<String>) -> Refusal {
    Refusal::new(code, format!("peer comparison: {}", reason.into()))
}

fn checked<T>(value: std::result::Result<T, fmine_units::UnitsError>) -> Result<T> {
    value.map_err(|_| fail(RefusalCode::NotObservable202, "exact arithmetic overflow"))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Metric {
    Aisc,
    GoldProduction,
}

impl Metric {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Aisc => "aisc",
            Self::GoldProduction => "gold-production",
        }
    }

    #[must_use]
    pub const fn unit(self) -> &'static str {
        match self {
            Self::Aisc => "USD/oz",
            Self::GoldProduction => "koz",
        }
    }

    const fn additivity(self) -> AdditivityClass {
        match self {
            Self::Aisc => AdditivityClass::IntensiveWeighted,
            Self::GoldProduction => AdditivityClass::ExtensiveAdditive,
        }
    }
}

const COMMON_FIELDS: &[&str] = &[
    "metric",
    "scope",
    "ownership",
    "operations",
    "metal",
    "denominator",
];
const COST_FIELDS: &[&str] = &[
    "allocation",
    "credits",
    "inventory",
    "ga",
    "sustaining_leases",
    "minesite_exploration",
    "reclamation",
    "stripping",
    "accounting",
];

/// Every field is part of the agreed source interpretation, never a caller's restatement override.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Definition {
    pub fields: BTreeMap<String, String>,
}

impl Definition {
    pub fn validate(&self, metric: Metric) -> Result<()> {
        let required: Vec<_> = COMMON_FIELDS
            .iter()
            .copied()
            .chain(
                COST_FIELDS
                    .iter()
                    .copied()
                    .filter(|_| metric == Metric::Aisc),
            )
            .collect();
        for field in &required {
            let Some(value) = self.fields.get(*field) else {
                return Err(fail(
                    RefusalCode::NonIdentifiableAttribution104,
                    format!("missing definition field {field}"),
                ));
            };
            let admitted = match *field {
                "metric" => value == metric.as_str(),
                "scope" => value == "company",
                "ownership" => matches!(value.as_str(), "full" | "attributable"),
                "operations" => matches!(value.as_str(), "all" | "continuing"),
                "metal" => value == "gold",
                "denominator" => {
                    value
                        == if metric == Metric::Aisc {
                            "sold"
                        } else {
                            "produced"
                        }
                }
                "allocation" => matches!(value.as_str(), "by_product" | "co_product"),
                "credits" => matches!(
                    value.as_str(),
                    "separate_line" | "inside_selling_costs" | "excluded"
                ),
                "inventory" | "ga" | "sustaining_leases" | "minesite_exploration" | "stripping" => {
                    matches!(value.as_str(), "included" | "excluded")
                }
                "reclamation" => {
                    matches!(value.as_str(), "operating_only" | "all_sites" | "excluded")
                }
                "accounting" => matches!(
                    value.as_str(),
                    "as_reported" | "historical_cost" | "purchase_price_allocation"
                ),
                _ => false,
            };
            if !admitted {
                return Err(fail(
                    RefusalCode::NonIdentifiableAttribution104,
                    format!("unsupported definition {field}={value}"),
                ));
            }
        }
        if let Some(field) = self.fields.keys().find(|k| !required.contains(&k.as_str())) {
            return Err(fail(
                RefusalCode::NonIdentifiableAttribution104,
                format!("unknown definition field {field}"),
            ));
        }
        Ok(())
    }

    fn encode(&self, enc: &mut Encoder) {
        enc.put_u64(self.fields.len() as u64);
        for (key, value) in &self.fields {
            enc.put_str(key);
            enc.put_str(value);
        }
    }
}

/// One issuer's disclosed value and its own definition citation. No bridge is admitted in this slice.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PeerReading {
    pub issuer: String,
    pub metric: Metric,
    pub year: u16,
    pub definition: Definition,
    pub interval: Interval,
    pub source: SourceAnchor,
    pub definition_source: SourceAnchor,
    pub quantity: String,
    pub raw_token: String,
    pub basis: String,
    pub published_at: String,
    pub published_at_basis: String,
    pub readers: Vec<String>,
}

fn put_rat(enc: &mut Encoder, value: Rat) {
    enc.put_i128(value.num());
    enc.put_i128(value.den());
}

fn put_anchor(enc: &mut Encoder, source: &SourceAnchor) {
    enc.put_str(&source.artifact_path);
    enc.put_fixed(&source.artifact_sha256);
    enc.put_str(&source.locator);
}

impl PeerReading {
    #[must_use]
    pub fn canonical_tag(&self) -> String {
        let mut enc = Encoder::new_frame(*b"PCSR", 1);
        enc.put_str(&self.issuer);
        enc.put_str(self.metric.as_str());
        enc.put_u16(self.year);
        self.definition.encode(&mut enc);
        put_rat(&mut enc, self.interval.lo);
        put_rat(&mut enc, self.interval.hi);
        enc.put_u8(u8::from(self.interval.lo_strict));
        enc.put_u8(u8::from(self.interval.hi_strict));
        put_anchor(&mut enc, &self.source);
        put_anchor(&mut enc, &self.definition_source);
        for value in [
            &self.quantity,
            &self.raw_token,
            &self.basis,
            &self.published_at,
            &self.published_at_basis,
        ] {
            enc.put_str(value);
        }
        enc.put_u64(self.readers.len() as u64);
        for reader in &self.readers {
            enc.put_str(reader);
        }
        blake3_hash(&enc.finish()).to_hex()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RankOrder {
    Lowest,
    Highest,
}

impl RankOrder {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Lowest => "lowest",
            Self::Highest => "highest",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RankClaim {
    pub issuer: String,
    pub order: RankOrder,
    /// Competition rank at most this number; tied issuers all share the same rank.
    pub at_most: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request {
    pub metric: Metric,
    pub year: u16,
    pub as_of: String,
    pub issuers: Vec<String>,
    pub claim: Option<RankClaim>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RankBounds {
    pub min: usize,
    pub max: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PeerRank {
    pub issuer: String,
    pub source_claim: ObjectId,
    pub interval: Interval,
    pub ascending: RankBounds,
    pub descending: RankBounds,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    Supported,
    Unsupported,
    Indeterminate,
}

impl Verdict {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Supported => "supported",
            Self::Unsupported => "unsupported",
            Self::Indeterminate => "indeterminate",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Summary {
    pub request: Request,
    pub definition: Definition,
    pub peers: Vec<PeerRank>,
    pub verdict: Option<Verdict>,
}

impl Summary {
    #[must_use]
    pub fn canonical_tag(&self) -> String {
        let mut enc = Encoder::new_frame(*b"PCRK", 1);
        enc.put_str(self.request.metric.as_str());
        enc.put_u16(self.request.year);
        enc.put_str(&self.request.as_of);
        enc.put_u64(self.request.issuers.len() as u64);
        for issuer in &self.request.issuers {
            enc.put_str(issuer);
        }
        self.definition.encode(&mut enc);
        enc.put_u64(self.peers.len() as u64);
        for peer in &self.peers {
            enc.put_str(&peer.issuer);
            enc.put_str(&peer.source_claim.to_string());
            put_rat(&mut enc, peer.interval.lo);
            put_rat(&mut enc, peer.interval.hi);
            enc.put_u8(u8::from(peer.interval.lo_strict));
            enc.put_u8(u8::from(peer.interval.hi_strict));
            for rank in [peer.ascending, peer.descending] {
                enc.put_u64(rank.min as u64);
                enc.put_u64(rank.max as u64);
            }
        }
        if let Some(claim) = &self.request.claim {
            enc.put_u8(1);
            enc.put_str(&claim.issuer);
            enc.put_str(claim.order.as_str());
            enc.put_u64(claim.at_most as u64);
        } else {
            enc.put_u8(0);
        }
        enc.put_str(self.verdict.map_or("none", Verdict::as_str));
        blake3_hash(&enc.finish()).to_hex()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Report {
    pub inputs: Vec<Claim<PeerReading>>,
    pub claim: Claim<Summary>,
}

fn validate(input: &Claim<PeerReading>, request: &Request) -> Result<()> {
    input.check_id()?;
    let reading = &input.value;
    if input.value_tag() != reading.canonical_tag() {
        return Err(fail(
            RefusalCode::FrozenRecordMutation507,
            "source value differs from its bound canonical interpretation",
        ));
    }
    if input.metric_key != INPUT_METRIC || reading.metric != request.metric {
        return Err(fail(
            RefusalCode::BasisMismatch105,
            "source metric differs from the selected metric",
        ));
    }
    if reading.year != request.year || input.support != SpatialSupport::PeriodAggregate {
        return Err(fail(
            RefusalCode::SupportMismatch102,
            "source does not have the selected complete annual support",
        ));
    }
    if input.additivity != request.metric.additivity() {
        return Err(fail(
            RefusalCode::NonAdditiveAverage103,
            "source additivity differs from the metric",
        ));
    }
    input.verify_export_clearance(&Confidentiality::public())?;
    if input.stale || input.state == KnowledgeState::Stale {
        return Err(fail(
            RefusalCode::StaleAnchor502,
            "stale source interpretation",
        ));
    }
    if input.representation != Representation::Deterministic
        || !matches!(
            input.state,
            KnowledgeState::Known | KnowledgeState::Estimated
        )
        || input.calibration_failed
    {
        return Err(fail(
            RefusalCode::NotObservable202,
            "source is not an admitted deterministic reading",
        ));
    }
    if reading.interval.lo_strict
        || reading.interval.hi_strict
        || checked(reading.interval.lo.checked_cmp(reading.interval.hi))?.is_gt()
    {
        return Err(fail(
            RefusalCode::NotObservable202,
            "comparison requires nonempty closed bounds",
        ));
    }
    if request.metric == Metric::GoldProduction && reading.interval.hi.signum() < 0 {
        return Err(fail(
            RefusalCode::BasisMismatch105,
            "gold production cannot be wholly negative",
        ));
    }
    for anchor in [&reading.source, &reading.definition_source] {
        if !input.anchors.contains(anchor)
            || anchor.artifact_path.trim().is_empty()
            || anchor.locator.trim().is_empty()
            || anchor.artifact_sha256 == [0; 32]
        {
            return Err(fail(
                RefusalCode::NoClaimInDomain201,
                "value and definition need their own retained source anchors",
            ));
        }
    }
    if reading.source.artifact_path != reading.definition_source.artifact_path
        || reading.source.artifact_sha256 != reading.definition_source.artifact_sha256
    {
        return Err(fail(
            RefusalCode::NonIdentifiableAttribution104,
            "definition must cite this issuer's selected source document",
        ));
    }
    let readers: BTreeSet<_> = reading.readers.iter().collect();
    if readers.len() < 2
        || readers.len() != reading.readers.len()
        || readers.iter().any(|reader| reader.trim().is_empty())
        || [&reading.quantity, &reading.raw_token, &reading.basis]
            .iter()
            .any(|value| value.trim().is_empty())
    {
        return Err(fail(
            RefusalCode::NotObservable202,
            "source needs a named quantity, printed token, interpretation and two distinct readers",
        ));
    }
    if !is_date(&reading.published_at)
        || !reading.published_at_basis.starts_with("FACT: ")
        || reading.published_at_basis[6..].trim().is_empty()
    {
        return Err(fail(
            RefusalCode::NotObservable202,
            "actual source needs evidenced publication availability",
        ));
    }
    if reading.published_at < format!("{:04}-12-31", request.year) {
        return Err(fail(
            RefusalCode::SupportMismatch102,
            "complete annual actual was published before the year ended",
        ));
    }
    if reading.published_at > request.as_of {
        return Err(fail(
            RefusalCode::AssayLookahead108,
            "source was published after the selected evidence cut",
        ));
    }
    reading.definition.validate(request.metric)
}

/// Exact rank bounds for the entire declared population. No missing or incompatible peer is dropped.
pub fn compare(request: &Request, inputs: &[Claim<PeerReading>]) -> Result<Report> {
    let peers: BTreeSet<_> = request.issuers.iter().cloned().collect();
    if !(2..=MAX_PEERS).contains(&peers.len())
        || peers.len() != request.issuers.len()
        || peers.iter().any(|p| p.trim().is_empty())
    {
        return Err(fail(
            RefusalCode::NotObservable202,
            "declare 2..64 distinct nonempty issuers",
        ));
    }
    if !(1900..=9999).contains(&request.year) || !is_date(&request.as_of) {
        return Err(fail(
            RefusalCode::SupportMismatch102,
            "comparison needs a valid annual period and evidence cut",
        ));
    }
    let found: BTreeSet<_> = inputs
        .iter()
        .map(|input| input.value.issuer.clone())
        .collect();
    if inputs.len() != peers.len() || found != peers {
        let missing = peers
            .difference(&found)
            .cloned()
            .collect::<Vec<_>>()
            .join(",");
        let unexpected = found
            .difference(&peers)
            .cloned()
            .collect::<Vec<_>>()
            .join(",");
        return Err(fail(
            RefusalCode::NotObservable202,
            format!(
                "complete peer set requires exactly one reading each; missing=[{missing}], unexpected=[{unexpected}], readings={}",
                inputs.len()
            ),
        ));
    }
    if let Some(claim) = &request.claim
        && (!peers.contains(&claim.issuer) || !(1..=peers.len()).contains(&claim.at_most))
    {
        return Err(fail(
            RefusalCode::NotObservable202,
            "rank claim needs a selected issuer and rank threshold within the complete peer count",
        ));
    }
    let mut inputs = inputs.to_vec();
    inputs.sort_by(|a, b| a.value.issuer.cmp(&b.value.issuer));
    for input in &inputs {
        validate(input, request)?;
    }
    let first = &inputs[0].value;
    for input in &inputs[1..] {
        for (field, expected) in &first.definition.fields {
            let actual = input.value.definition.fields.get(field);
            if actual != Some(expected) {
                return Err(fail(
                    RefusalCode::BasisMismatch105,
                    format!(
                        "definition mismatch {field}: {}={expected}, {}={}; no cited bridge is admitted",
                        first.issuer,
                        input.value.issuer,
                        actual.map_or("missing", String::as_str)
                    ),
                ));
            }
        }
    }
    let mut ranks = Vec::new();
    for current in &inputs {
        let mut ascending = RankBounds { min: 1, max: 1 };
        let mut descending = RankBounds { min: 1, max: 1 };
        let q = current.value.interval;
        for other in &inputs {
            if current.value.issuer == other.value.issuer {
                continue;
            }
            let p = other.value.interval;
            ascending.min += usize::from(checked(p.hi.checked_cmp(q.lo))?.is_lt());
            ascending.max += usize::from(checked(p.lo.checked_cmp(q.hi))?.is_lt());
            descending.min += usize::from(checked(p.lo.checked_cmp(q.hi))?.is_gt());
            descending.max += usize::from(checked(p.hi.checked_cmp(q.lo))?.is_gt());
        }
        ranks.push(PeerRank {
            issuer: current.value.issuer.clone(),
            source_claim: current.claim_id,
            interval: q,
            ascending,
            descending,
        });
    }
    let verdict = request.claim.as_ref().and_then(|claim| {
        ranks
            .iter()
            .find(|rank| rank.issuer == claim.issuer)
            .map(|rank| {
                let bounds = match claim.order {
                    RankOrder::Lowest => rank.ascending,
                    RankOrder::Highest => rank.descending,
                };
                if bounds.max <= claim.at_most {
                    Verdict::Supported
                } else if bounds.min > claim.at_most {
                    Verdict::Unsupported
                } else {
                    Verdict::Indeterminate
                }
            })
    });
    let mut canonical_request = request.clone();
    canonical_request.issuers = peers.into_iter().collect();
    let summary = Summary {
        request: canonical_request,
        definition: first.definition.clone(),
        peers: ranks,
        verdict,
    };
    let mut evidence = EvidenceClass::Derived;
    let mut epistemic = EpistemicStatus::KnownKnown;
    let mut state = KnowledgeState::Known;
    let mut anchors = BTreeSet::new();
    let mut parents = BTreeSet::new();
    let mut uu = BTreeMap::new();
    for input in &inputs {
        evidence = evidence.meet(input.evidence);
        epistemic = epistemic.and(input.epistemic);
        state = state.and(input.state);
        anchors.extend(input.anchors.iter().cloned());
        parents.insert(input.claim_id);
        for term in &input.uu_terms {
            if let Some(prior) = uu.insert(term.scenario_id.clone(), term.clone())
                && prior != *term
            {
                return Err(fail(
                    RefusalCode::UnbudgetedUnknowns204,
                    "conflicting shared unknown-unknown scenario",
                ));
            }
        }
    }
    let tag = summary.canonical_tag();
    let claim = Claim::new(
        OUTPUT_METRIC,
        summary,
        &tag,
        Representation::Deterministic,
        SpatialSupport::PeriodAggregate,
        AdditivityClass::NonAdditive,
        evidence,
        epistemic,
        state,
        Confidentiality::public(),
        uu.into_values().collect(),
        parents.into_iter().collect(),
        anchors.into_iter().collect(),
    );
    Ok(Report { inputs, claim })
}
