//! Exact production GEO on one explicitly declared physical population (BD-062, GB-27).
//! Determinism: Exact (inherited colour and UU metadata retain their floating-point bits).

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use fmine_claim::{AdditivityClass, Claim, Confidentiality, Representation, SpatialSupport};
use fmine_core::{Encoder, ObjectId, blake3_hash};
use fmine_evidence::color::validate_color_payload;
use fmine_evidence::{Color, EpistemicStatus, EvidenceClass, KnowledgeState, Refusal, RefusalCode};
use fmine_units::{Currency, ExactQty, UnitsError};

use crate::guidance::{Ownership, Period};

pub mod worksheet;

pub const INPUT_METRIC: &str = "production.geo.input/1";
pub const OUTPUT_METRIC: &str = "production.geo.comparison/1";
pub const NO_CLAIM: &str = "Exact GEO arithmetic conditional on one declared physical production population and named price decks. Omitted metals are outside that population, not independently verified zero. Deck differences measure price-ratio basis only; no physical production change, source audit, calibrated forecast, reserve classification or disclosure authority is established.";
pub const MAX_DECKS: usize = 64;
type Result<T> = std::result::Result<T, Refusal>;
const ZERO: ExactQty = ExactQty::from_integer(0);
const ONE: ExactQty = ExactQty::from_integer(1);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Metal {
    Gold,
    Copper,
    Silver,
}

impl Metal {
    pub const ALL: [Self; 3] = [Self::Gold, Self::Copper, Self::Silver];

    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Gold => "gold",
            Self::Copper => "copper",
            Self::Silver => "silver",
        }
    }

    #[must_use]
    pub const fn unit(self) -> &'static str {
        match self {
            Self::Gold | Self::Silver => "oz",
            Self::Copper => "lb",
        }
    }
}

/// Every quantity and deck is evaluated on this same physical production support.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Scope {
    pub issuer: String,
    pub subject: String,
    pub period: Period,
    pub ownership: Ownership,
    pub basis: String,
    pub population: String,
    pub currency: Currency,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct DeckKey {
    pub id: String,
    pub version: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PriceDeck {
    pub key: DeckKey,
    pub basis: String,
    pub currency: Currency,
    /// Major currency units per troy ounce of gold; needed even without gold production.
    pub gold_per_oz: ExactQty,
    pub copper_per_lb: Option<ExactQty>,
    pub silver_per_oz: Option<ExactQty>,
}

impl PriceDeck {
    fn price(&self, metal: Metal) -> Option<ExactQty> {
        match metal {
            Metal::Gold => Some(self.gold_per_oz),
            Metal::Copper => self.copper_per_lb,
            Metal::Silver => self.silver_per_oz,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GeoValue {
    Population {
        metals: Vec<Metal>,
        reference: DeckKey,
    },
    Quantity {
        metal: Metal,
        unit: String,
        amount: ExactQty,
    },
    Deck(PriceDeck),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeoInput {
    pub scope: Scope,
    pub value: GeoValue,
}

impl GeoInput {
    fn key(&self) -> (u8, &str, &str) {
        match &self.value {
            GeoValue::Population { .. } => (0, "", ""),
            GeoValue::Quantity { metal, .. } => (1, metal.name(), ""),
            GeoValue::Deck(deck) => (2, &deck.key.id, &deck.key.version),
        }
    }

    #[must_use]
    pub const fn additivity(&self) -> AdditivityClass {
        match self.value {
            GeoValue::Quantity { .. } => AdditivityClass::ExtensiveAdditive,
            _ => AdditivityClass::NonAdditive,
        }
    }

    #[must_use]
    pub fn canonical_tag(&self) -> String {
        let mut enc = Encoder::new_frame(*b"GEOI", 1);
        enc.put_str(&scope_json(&self.scope));
        enc.put_str(&value_json(&self.value));
        blake3_hash(&enc.finish()).to_hex()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Contribution {
    pub metal: Metal,
    pub quantity_claim_id: ObjectId,
    pub amount: ExactQty,
    pub price_per_unit: ExactQty,
    pub gold_oz_per_unit: ExactQty,
    pub geo_oz: ExactQty,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeckResult {
    pub deck: PriceDeck,
    pub deck_claim_id: ObjectId,
    pub contributions: Vec<Contribution>,
    pub total_geo_oz: ExactQty,
    pub difference_from_reference_oz: ExactQty,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeoSummary {
    pub scope: Scope,
    pub metals: Vec<Metal>,
    pub excluded_metals: Vec<Metal>,
    pub reference: DeckKey,
    pub decks: Vec<DeckResult>,
}

/// The receipt owns immutable admitted inputs and a derived comparison Claim.
#[derive(Debug, Clone, PartialEq)]
pub struct GeoReceipt {
    inputs: Vec<Claim<GeoInput>>,
    claim: Claim<GeoSummary>,
    digest: String,
}

impl GeoReceipt {
    #[must_use]
    pub fn inputs(&self) -> &[Claim<GeoInput>] {
        &self.inputs
    }
    #[must_use]
    pub fn summary(&self) -> &GeoSummary {
        &self.claim.value
    }
    #[must_use]
    pub fn claim(&self) -> &Claim<GeoSummary> {
        &self.claim
    }
    #[must_use]
    pub fn digest(&self) -> &str {
        &self.digest
    }
}

fn bad(code: RefusalCode, message: &str) -> Refusal {
    Refusal::new(code, format!("production GEO: {message}"))
}

fn exact<T>(value: std::result::Result<T, UnitsError>) -> Result<T> {
    value.map_err(|_| {
        bad(
            RefusalCode::NumericalCertificateFailed207,
            "exact arithmetic overflow",
        )
    })
}

fn named(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 4096
        && value.trim() == value
        && !value.chars().any(char::is_control)
}

fn admit(input: &Claim<GeoInput>) -> Result<()> {
    input.check_id()?;
    if input.metric_key != INPUT_METRIC || input.value_tag() != input.value.canonical_tag() {
        return Err(bad(
            RefusalCode::FrozenRecordMutation507,
            "input differs from its canonical value",
        ));
    }
    if input.stale || input.state == KnowledgeState::Stale {
        return Err(bad(RefusalCode::StaleAnchor502, "stale input"));
    }
    if !matches!(
        input.state,
        KnowledgeState::Known | KnowledgeState::Estimated
    ) || input.calibration_failed
    {
        return Err(bad(RefusalCode::NotObservable202, "unusable input state"));
    }
    if input.representation != Representation::Deterministic
        || input.support != SpatialSupport::PeriodAggregate
    {
        return Err(bad(
            RefusalCode::SupportMismatch102,
            "explicit deterministic period aggregates required",
        ));
    }
    if input.additivity != input.value.additivity() {
        return Err(bad(
            RefusalCode::NonAdditiveAverage103,
            "quantities are extensive; decks and population declarations are non-additive",
        ));
    }
    if input.anchors.is_empty()
        || input
            .anchors
            .iter()
            .any(|a| !named(&a.artifact_path) || !named(&a.locator) || a.artifact_sha256 == [0; 32])
    {
        return Err(bad(
            RefusalCode::NoClaimInDomain201,
            "every input requires a source hash and locator",
        ));
    }
    if input.uu_terms.iter().any(|u| {
        !named(&u.scenario_id)
            || !named(&u.description)
            || !named(&u.trigger_indicator)
            || !u.downside_exposure_usd_m.is_finite()
            || u.downside_exposure_usd_m < 0.0
    }) {
        return Err(bad(
            RefusalCode::UnbudgetedUnknowns204,
            "invalid UU metadata",
        ));
    }
    if let Ok(color) = input.color() {
        validate_color_payload(&color).map_err(|_| {
            bad(
                RefusalCode::CalibrationRequired205,
                "invalid colour metadata",
            )
        })?;
    }
    Ok(())
}

/// Evaluate every declared deck against identical produced quantities. Ownership is a shared label;
/// amounts are already on that basis and are never rescaled. Inputs may be supplied in any order.
pub fn evaluate(inputs: &[Claim<GeoInput>]) -> Result<GeoReceipt> {
    if inputs.len() > MAX_DECKS + 4 {
        return Err(bad(
            RefusalCode::OutsidePhysicalDomain109,
            "input population exceeds the finite limit",
        ));
    }
    let mut inputs = inputs.to_vec();
    inputs.sort_by(|a, b| a.value.key().cmp(&b.value.key()));
    let first = inputs
        .first()
        .ok_or_else(|| bad(RefusalCode::NoClaimInDomain201, "missing population"))?;
    let scope = &first.value.scope;
    if [
        &scope.issuer,
        &scope.subject,
        &scope.basis,
        &scope.population,
    ]
    .iter()
    .any(|v| !named(v))
        || scope.period.year == 0
        || scope.period.first_month == 0
        || scope.period.first_month > scope.period.last_month
        || scope.period.last_month > 12
    {
        return Err(bad(
            RefusalCode::SupportMismatch102,
            "named physical scope and valid calendar support required",
        ));
    }
    if let Ownership::Attributable { share } = scope.ownership
        && (share.signum() <= 0 || exact(share.checked_cmp(ONE))?.is_gt())
    {
        return Err(bad(
            RefusalCode::BasisMismatch105,
            "ownership share must lie in (0, 1]",
        ));
    }
    for input in &inputs {
        admit(input)?;
        if input.value.scope != *scope {
            return Err(bad(
                RefusalCode::BasisMismatch105,
                "issuer, subject, period, ownership, physical basis, population or currency differs",
            ));
        }
    }
    if inputs
        .windows(2)
        .any(|p| p[0].value.key() == p[1].value.key())
    {
        return Err(bad(
            RefusalCode::BasisMismatch105,
            "duplicate population, metal quantity or deck/version",
        ));
    }
    let GeoValue::Population { metals, reference } = &first.value.value else {
        return Err(bad(
            RefusalCode::NoClaimInDomain201,
            "an explicit metal population and reference deck are required",
        ));
    };
    let population: BTreeSet<_> = metals.iter().copied().collect();
    if population.is_empty()
        || population.len() != metals.len()
        || !named(&reference.id)
        || !named(&reference.version)
    {
        return Err(bad(
            RefusalCode::BasisMismatch105,
            "a nonempty unique metal population and named reference/version are required",
        ));
    }
    let mut quantities = BTreeMap::new();
    let mut decks = Vec::new();
    for input in &inputs {
        match &input.value.value {
            GeoValue::Quantity {
                metal,
                unit,
                amount,
            } => {
                if unit != metal.unit() {
                    return Err(bad(
                        RefusalCode::UndeclaredUnitBasisOrTz107,
                        "gold/silver require oz and copper requires lb; no conversion is implicit",
                    ));
                }
                if amount.signum() < 0 {
                    return Err(bad(
                        RefusalCode::OutsidePhysicalDomain109,
                        "production cannot be negative",
                    ));
                }
                quantities.insert(*metal, (input.claim_id, *amount));
            }
            GeoValue::Deck(deck) => {
                if !named(&deck.key.id)
                    || !named(&deck.key.version)
                    || !named(&deck.basis)
                    || deck.currency != scope.currency
                {
                    return Err(bad(
                        RefusalCode::BasisMismatch105,
                        "each deck needs a name, version, basis and matching currency",
                    ));
                }
                if std::iter::once(deck.gold_per_oz)
                    .chain(deck.copper_per_lb)
                    .chain(deck.silver_per_oz)
                    .any(|price| price.signum() <= 0)
                {
                    return Err(bad(
                        RefusalCode::OutsidePhysicalDomain109,
                        "all supplied prices must be positive",
                    ));
                }
                decks.push((input.claim_id, deck));
            }
            GeoValue::Population { .. } => {}
        }
    }
    if quantities.keys().copied().collect::<BTreeSet<_>>() != population {
        return Err(bad(
            RefusalCode::NoClaimInDomain201,
            "exactly one quantity is required for every included metal and no other metal",
        ));
    }
    if !(2..=MAX_DECKS).contains(&decks.len()) || !decks.iter().any(|(_, d)| d.key == *reference) {
        return Err(bad(
            RefusalCode::NoClaimInDomain201,
            "at least two decks, including the named reference/version, are required",
        ));
    }
    let mut results = Vec::with_capacity(decks.len());
    for (deck_claim_id, deck) in decks {
        let mut contributions = Vec::with_capacity(population.len());
        let mut total = ZERO;
        for (metal, (quantity_claim_id, amount)) in &quantities {
            let price = deck.price(*metal).ok_or_else(|| {
                bad(
                    RefusalCode::NoClaimInDomain201,
                    "a deck omits a price for an included metal",
                )
            })?;
            let ratio = exact(price.checked_div(deck.gold_per_oz))?;
            let geo_oz = exact(amount.checked_mul(ratio))?;
            total = exact(total.checked_add(geo_oz))?;
            contributions.push(Contribution {
                metal: *metal,
                quantity_claim_id: *quantity_claim_id,
                amount: *amount,
                price_per_unit: price,
                gold_oz_per_unit: ratio,
                geo_oz,
            });
        }
        results.push(DeckResult {
            deck: deck.clone(),
            deck_claim_id,
            contributions,
            total_geo_oz: total,
            difference_from_reference_oz: ZERO,
        });
    }
    let baseline = results
        .iter()
        .find(|r| r.deck.key == *reference)
        .ok_or_else(|| bad(RefusalCode::NoClaimInDomain201, "reference deck absent"))?
        .total_geo_oz;
    for result in &mut results {
        result.difference_from_reference_oz = exact(result.total_geo_oz.checked_sub(baseline))?;
    }
    let summary = GeoSummary {
        scope: scope.clone(),
        metals: population.iter().copied().collect(),
        excluded_metals: Metal::ALL
            .into_iter()
            .filter(|m| !population.contains(m))
            .collect(),
        reference: reference.clone(),
        decks: results,
    };
    let mut enc = Encoder::new_frame(*b"GEOR", 1);
    enc.put_u64(inputs.len() as u64);
    let mut evidence = EvidenceClass::Derived;
    let mut epistemic = EpistemicStatus::KnownKnown;
    let mut state = KnowledgeState::Known;
    let mut confidentiality = Confidentiality::public();
    let mut uu = BTreeMap::new();
    let mut parents = BTreeSet::new();
    let mut anchors = BTreeSet::new();
    for input in &inputs {
        enc.put_str(&claim_json(input, &input_json(&input.value)));
        evidence = evidence.meet(input.evidence);
        epistemic = epistemic.and(input.epistemic);
        state = state.and(input.state);
        confidentiality = confidentiality.join(&input.confidentiality);
        parents.insert(input.claim_id);
        anchors.extend(input.anchors.iter().cloned());
        for term in &input.uu_terms {
            if let Some(prior) = uu.insert(term.scenario_id.clone(), term.clone())
                && prior != *term
            {
                return Err(bad(
                    RefusalCode::UnbudgetedUnknowns204,
                    "conflicting definitions for a shared UU scenario",
                ));
            }
        }
    }
    enc.put_str(&summary_json(&summary));
    let digest = blake3_hash(&enc.finish()).to_hex();
    let claim = Claim::new(
        OUTPUT_METRIC,
        summary,
        &digest,
        Representation::Deterministic,
        SpatialSupport::PeriodAggregate,
        AdditivityClass::NonAdditive,
        evidence,
        epistemic,
        state,
        confidentiality,
        uu.into_values().collect(),
        parents.into_iter().collect(),
        anchors.into_iter().collect(),
    );
    Ok(GeoReceipt {
        inputs,
        claim,
        digest,
    })
}

fn js(value: &str) -> String {
    let mut out = String::from("\"");
    for c in value.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c.is_control() => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn qty(value: ExactQty) -> String {
    if value.den() == 1 {
        value.num().to_string()
    } else {
        format!("{}/{}", value.num(), value.den())
    }
}
fn names(metals: &[Metal]) -> String {
    metals
        .iter()
        .map(|m| js(m.name()))
        .collect::<Vec<_>>()
        .join(",")
}
fn key_json(key: &DeckKey) -> String {
    format!(
        "{{\"id\":{},\"version\":{}}}",
        js(&key.id),
        js(&key.version)
    )
}
fn scope_json(scope: &Scope) -> String {
    let ownership = match scope.ownership {
        Ownership::Full => "{\"kind\":\"full\"}".into(),
        Ownership::Attributable { share } => format!(
            "{{\"kind\":\"attributable\",\"share\":{}}}",
            js(&qty(share))
        ),
    };
    format!(
        "{{\"issuer\":{},\"subject\":{},\"period\":{{\"year\":{},\"first_month\":{},\"last_month\":{}}},\"ownership\":{ownership},\"basis\":{},\"population\":{},\"currency\":{}}}",
        js(&scope.issuer),
        js(&scope.subject),
        scope.period.year,
        scope.period.first_month,
        scope.period.last_month,
        js(&scope.basis),
        js(&scope.population),
        js(&format!("{:?}", scope.currency))
    )
}
fn deck_json(deck: &PriceDeck) -> String {
    let optional = |price: Option<ExactQty>| price.map_or_else(|| "null".into(), |v| js(&qty(v)));
    format!(
        "{{\"key\":{},\"basis\":{},\"currency\":{},\"gold_price_per_oz\":{},\"copper_price_per_lb\":{},\"silver_price_per_oz\":{}}}",
        key_json(&deck.key),
        js(&deck.basis),
        js(&format!("{:?}", deck.currency)),
        js(&qty(deck.gold_per_oz)),
        optional(deck.copper_per_lb),
        optional(deck.silver_per_oz)
    )
}
fn value_json(value: &GeoValue) -> String {
    match value {
        GeoValue::Population { metals, reference } => {
            let mut metals = metals.clone();
            metals.sort();
            format!(
                "{{\"kind\":\"population\",\"metals\":[{}],\"reference\":{}}}",
                names(&metals),
                key_json(reference)
            )
        }
        GeoValue::Quantity {
            metal,
            unit,
            amount,
        } => format!(
            "{{\"kind\":\"produced_quantity\",\"metal\":{},\"unit\":{},\"amount\":{}}}",
            js(metal.name()),
            js(unit),
            js(&qty(*amount))
        ),
        GeoValue::Deck(deck) => format!("{{\"kind\":\"price_deck\",\"deck\":{}}}", deck_json(deck)),
    }
}
fn input_json(input: &GeoInput) -> String {
    format!(
        "{{\"scope\":{},\"value\":{}}}",
        scope_json(&input.scope),
        value_json(&input.value)
    )
}
fn color_json(color: std::result::Result<Color, Refusal>) -> String {
    match color {
        Err(r) => format!("{{\"status\":\"refused\",\"code\":{}}}", js(r.code)),
        Ok(Color::Verified { lo, hi }) => format!(
            "{{\"kind\":\"verified\",\"lo_bits\":\"{:016x}\",\"hi_bits\":\"{:016x}\"}}",
            lo.to_bits(),
            hi.to_bits()
        ),
        Ok(Color::Estimated {
            estimator,
            dispersion,
        }) => format!(
            "{{\"kind\":\"estimated\",\"estimator\":{},\"dispersion_bits\":\"{:016x}\"}}",
            js(&estimator),
            dispersion.to_bits()
        ),
        Ok(Color::Validated { regime, dataset }) => {
            let axes = regime
                .bounds()
                .iter()
                .map(|(axis, (lo, hi))| {
                    format!(
                        "{{\"axis\":{},\"lo_bits\":\"{:016x}\",\"hi_bits\":\"{:016x}\"}}",
                        js(axis),
                        lo.to_bits(),
                        hi.to_bits()
                    )
                })
                .collect::<Vec<_>>()
                .join(",");
            format!(
                "{{\"kind\":\"validated\",\"dataset\":{},\"bounds\":[{axes}]}}",
                js(&dataset)
            )
        }
    }
}
fn claim_json<T>(claim: &Claim<T>, value: &str) -> String {
    let mut body = String::new();
    for byte in claim.canonical_body() {
        let _ = write!(body, "{byte:02x}");
    }
    let anchors = claim
        .anchors
        .iter()
        .map(|a| {
            format!(
                "{{\"artifact_path\":{},\"sha256\":{},\"locator\":{}}}",
                js(&a.artifact_path),
                js(&fmine_core::Digest32(a.artifact_sha256).to_hex()),
                js(&a.locator)
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let uu = claim.uu_terms.iter().map(|u| format!("{{\"id\":{},\"description\":{},\"downside_usd_m_bits\":\"{:016x}\",\"trigger\":{}}}", js(&u.scenario_id), js(&u.description), u.downside_exposure_usd_m.to_bits(), js(&u.trigger_indicator))).collect::<Vec<_>>().join(",");
    let parents = claim
        .parents
        .iter()
        .map(|id| js(&id.to_string()))
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{{\"claim_id\":{},\"metric_key\":{},\"value_tag\":{},\"canonical_body_hex\":{},\"value\":{value},\"evidence\":{},\"epistemic\":{},\"knowledge\":{},\"representation\":{},\"support\":{},\"additivity\":{},\"confidentiality\":{},\"stale\":{},\"calibration_failed\":{},\"supersedes\":{},\"parents\":[{parents}],\"anchors\":[{anchors}],\"uu_terms\":[{uu}],\"color\":{}}}",
        js(&claim.claim_id.to_string()),
        js(&claim.metric_key),
        js(claim.value_tag()),
        js(&body),
        js(&format!("{:?}", claim.evidence)),
        js(&format!("{:?}", claim.epistemic)),
        js(&format!("{:?}", claim.state)),
        js(&format!("{:?}", claim.representation)),
        js(&format!("{:?}", claim.support)),
        js(&format!("{:?}", claim.additivity)),
        js(&claim.confidentiality.to_string()),
        claim.stale,
        claim.calibration_failed,
        claim
            .supersedes
            .map_or_else(|| "null".into(), |id| js(&id.to_string())),
        color_json(claim.color())
    )
}
fn summary_json(summary: &GeoSummary) -> String {
    let decks = summary.decks.iter().map(|d| {
        let contributions = d.contributions.iter().map(|c| format!("{{\"metal\":{},\"quantity_claim_id\":{},\"amount\":{},\"unit\":{},\"price_per_unit\":{},\"gold_oz_per_unit\":{},\"geo_oz\":{}}}", js(c.metal.name()), js(&c.quantity_claim_id.to_string()), js(&qty(c.amount)), js(c.metal.unit()), js(&qty(c.price_per_unit)), js(&qty(c.gold_oz_per_unit)), js(&qty(c.geo_oz)))).collect::<Vec<_>>().join(",");
        format!("{{\"deck\":{},\"deck_claim_id\":{},\"total_geo_oz\":{},\"difference_from_reference_oz\":{},\"contributions\":[{contributions}]}}", deck_json(&d.deck), js(&d.deck_claim_id.to_string()), js(&qty(d.total_geo_oz)), js(&qty(d.difference_from_reference_oz)))
    }).collect::<Vec<_>>().join(",");
    format!(
        "{{\"scope\":{},\"included_metals\":[{}],\"metals_outside_declared_population\":[{}],\"reference\":{},\"decks\":[{decks}],\"difference_basis\":\"price ratios on unchanged physical quantities\",\"calibration\":\"not_established\"}}",
        scope_json(&summary.scope),
        names(&summary.metals),
        names(&summary.excluded_metals),
        key_json(&summary.reference)
    )
}

impl GeoReceipt {
    #[must_use]
    pub fn to_json(&self) -> String {
        let summary = summary_json(self.summary());
        let inputs = self
            .inputs
            .iter()
            .map(|c| claim_json(c, &input_json(&c.value)))
            .collect::<Vec<_>>()
            .join(",");
        format!(
            "{{\"schema\":{},\"digest\":{},\"summary\":{summary},\"claim\":{},\"inputs\":[{inputs}],\"no_claim\":{}}}",
            js(worksheet::SCHEMA),
            js(&self.digest),
            claim_json(&self.claim, &summary),
            js(NO_CLAIM)
        )
    }

    #[must_use]
    pub fn to_markdown(&self) -> String {
        let text = |v: &str| {
            v.replace('\\', "\\\\")
                .replace('|', "\\|")
                .replace('<', "&lt;")
                .replace('>', "&gt;")
        };
        let s = self.summary();
        let ownership = match s.scope.ownership {
            Ownership::Full => "100%".into(),
            Ownership::Attributable { share } => format!("attributable {}", qty(share)),
        };
        let mut out = format!(
            "# Production GEO comparison\n\n{NO_CLAIM}\n\n**Issuer / subject:** {} / {}. **Period:** {} months {}–{}. **Ownership:** {}.\n\n**Population:** {}. **Physical basis:** {}. **Currency:** {:?}. **Reference:** {} / {}.\n\n**Metals outside the declared population:** {}.\n\n**Evidence:** {:?}; confidentiality: {}. Arithmetic does not establish calibration.\n\n| Deck | Version | Price basis | GEO oz | Difference from reference, oz |\n|---|---|---|---:|---:|\n",
            text(&s.scope.issuer),
            text(&s.scope.subject),
            s.scope.period.year,
            s.scope.period.first_month,
            s.scope.period.last_month,
            ownership,
            text(&s.scope.population),
            text(&s.scope.basis),
            s.scope.currency,
            text(&s.reference.id),
            text(&s.reference.version),
            s.excluded_metals
                .iter()
                .map(|m| m.name())
                .collect::<Vec<_>>()
                .join(","),
            self.claim.evidence,
            self.claim.confidentiality
        );
        for d in &s.decks {
            let _ = writeln!(
                out,
                "| {} | {} | {} | {} | {} |",
                text(&d.deck.key.id),
                text(&d.deck.key.version),
                text(&d.deck.basis),
                qty(d.total_geo_oz),
                qty(d.difference_from_reference_oz)
            );
        }
        out.push_str("\nAll deck differences use the same physical quantities; they are price-ratio effects only. No ownership or mass conversion is implicit.\n\n| Deck / version | Metal | Produced quantity | Unit | Price per unit | Gold oz per unit | GEO oz |\n|---|---|---:|---|---:|---:|---:|\n");
        for d in &s.decks {
            for c in &d.contributions {
                let _ = writeln!(
                    out,
                    "| {} / {} | {} | {} | {} | {} | {} | {} |",
                    text(&d.deck.key.id),
                    text(&d.deck.key.version),
                    c.metal.name(),
                    qty(c.amount),
                    c.metal.unit(),
                    qty(c.price_per_unit),
                    qty(c.gold_oz_per_unit),
                    qty(c.geo_oz)
                );
            }
        }
        out.push_str("\n## Source Claims\n\n");
        for input in &self.inputs {
            let _ = writeln!(out, "- {} ({:?}):", input.claim_id, input.evidence);
            for a in &input.anchors {
                let _ = writeln!(
                    out,
                    "  - {} — {}; SHA-256 {}.",
                    text(&a.artifact_path),
                    text(&a.locator),
                    fmine_core::Digest32(a.artifact_sha256)
                );
            }
        }
        let _ = writeln!(
            out,
            "\nOutput Claim: {}. Digest: {}.",
            self.claim.claim_id, self.digest
        );
        out
    }
}
