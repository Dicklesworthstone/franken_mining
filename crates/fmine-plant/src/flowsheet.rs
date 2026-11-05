//! DESIGN: nominal accounting for a declared conservative plant flowsheet
//! (plan §§17.2, 20.1–20.2; BD-175/177). Each internal edge owns one observation,
//! used at both endpoints; recycle is not new feed across the outer boundary.
//! Elemental closure is owned by `inventory::account_interval`. Dry matter and
//! water are conserved carriers here: reaction, dissolution/precipitation that
//! changes the declared carrier basis, and implicit process prediction are absent.
//! Determinism: Exact

use std::collections::{BTreeMap, BTreeSet};

use fmine_claim::{AdditivityClass, Claim, Confidentiality, Representation, SpatialSupport};
use fmine_core::{codec::Encoder, hash::blake3_hash};
use fmine_evidence::{EpistemicStatus, EvidenceClass, KnowledgeState, Refusal, RefusalCode};
use fmine_time::ValidTime;
use fmine_units::{ExactQty, UnitsError};

use crate::inventory::{self, BalanceScope, BalanceValue, MetalBalanceInput, MetalBalanceReceipt};

pub mod reconciliation;

type Result<T> = std::result::Result<T, Refusal>;
const ZERO: ExactQty = ExactQty::from_integer(0);
pub const INPUT_METRIC: &str = "plant.flowsheet.input/1";
pub const OUTPUT_METRIC: &str = "plant.flowsheet.closed_interval/1";
pub const NO_CLAIM: &str = "Exact nominal accounting for a declared complete conservative flowsheet. No reaction, process prediction, uncertainty reconciliation, source audit or fitted recovery is supplied.";

/// Typed identity of one physical unit boundary in a flowsheet version.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct UnitId(String);

impl UnitId {
    pub fn new(name: impl Into<String>) -> Result<Self> {
        let name = name.into();
        identifier(&name)?;
        Ok(Self(name))
    }
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Typed identity of one directed transfer, independent of its observation order.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct StreamId(String);

impl StreamId {
    pub fn new(name: impl Into<String>) -> Result<Self> {
        let name = name.into();
        identifier(&name)?;
        Ok(Self(name))
    }
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Mechanical unit labels; none selects or fits an equipment model.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnitKind {
    Mill,
    Classifier,
    Separator,
    Mixer,
    Storage,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnitSpec {
    pub id: UnitId,
    pub kind: UnitKind,
}

/// Explicit source-unit accounting role for an internal outgoing stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutflowRole {
    Product,
    Other,
}

/// Physical endpoints and classification. An internal transfer is one edge,
/// including in a cycle; sender and receiver cannot supply different amounts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StreamRoute {
    Feed {
        to: UnitId,
    },
    Internal {
        from: UnitId,
        to: UnitId,
        source_role: OutflowRole,
    },
    Product {
        from: UnitId,
    },
    OtherOutflow {
        from: UnitId,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StreamSpec {
    pub id: StreamId,
    pub route: StreamRoute,
}

/// Total element in all phases under one explicitly named source measurand.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ElementKey {
    pub element: String,
    pub measurand: String,
}

/// Nominal extensive amounts in kg. Dry solids means all non-water dry matter
/// on this accounting basis, including dissolved dry matter. Tracked elements
/// overlap these carriers: do not add elemental masses to solids + water.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MaterialAmounts {
    pub dry_solids_kg: ExactQty,
    /// H2O mass, not total solution mass; dissolved non-water matter is dry matter.
    pub water_kg: ExactQty,
    /// Exactly the declared element set, including explicitly supplied zeros.
    pub elements_kg: BTreeMap<ElementKey, ExactQty>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlowsheetScope {
    /// Full physical plant boundary and version; no ownership allocation.
    pub boundary: String,
    pub interval: ValidTime,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum StocktakeKind {
    Opening,
    Closing,
}

/// One source declaration or observation. Every declared edge has exactly one
/// Transfer, and every declared unit has both stocktakes, even when zero.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FlowsheetValue {
    Declaration {
        units: Vec<UnitSpec>,
        streams: Vec<StreamSpec>,
        elements: Vec<ElementKey>,
        /// All physical boundaries, transfers and held inventory are included.
        complete: bool,
    },
    Transfer {
        stream: StreamId,
        amounts: MaterialAmounts,
    },
    Stocktake {
        unit: UnitId,
        kind: StocktakeKind,
        amounts: MaterialAmounts,
    },
}

impl FlowsheetValue {
    #[must_use]
    pub const fn additivity(&self) -> AdditivityClass {
        match self {
            Self::Declaration { .. } => AdditivityClass::NonAdditive,
            _ => AdditivityClass::ExtensiveAdditive,
        }
    }
    pub(crate) fn key(&self) -> (u8, &str, u8) {
        match self {
            Self::Declaration { .. } => (0, "", 0),
            Self::Transfer { stream, .. } => (1, stream.as_str(), 0),
            Self::Stocktake { unit, kind, .. } => (2, unit.as_str(), *kind as u8),
        }
    }
    pub(crate) fn amounts(&self) -> Option<&MaterialAmounts> {
        match self {
            Self::Declaration { .. } => None,
            Self::Transfer { amounts, .. } | Self::Stocktake { amounts, .. } => Some(amounts),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlowsheetInput {
    pub scope: FlowsheetScope,
    /// Exact full interval for transfers/declaration; endpoint instant for stocks.
    pub physical_time: ValidTime,
    pub value: FlowsheetValue,
}

impl FlowsheetInput {
    /// Binds the complete typed payload. Declaration ordering is immaterial;
    /// duplicate identities still refuse at admission.
    #[must_use]
    pub fn canonical_tag(&self) -> String {
        let mut enc = Encoder::new_frame(*b"FLWI", 1);
        enc.put_str(&self.scope.boundary);
        put_time(&mut enc, self.scope.interval);
        put_time(&mut enc, self.physical_time);
        enc.put_str("kg/dry-matter+water/total-elements/all-phases/full-boundary");
        enc.put_u8(self.value.key().0);
        match &self.value {
            FlowsheetValue::Declaration {
                units,
                streams,
                elements,
                complete,
            } => {
                enc.put_u8(u8::from(*complete));
                let mut units: Vec<_> = units.iter().collect();
                units.sort_by(|a, b| a.id.cmp(&b.id));
                enc.put_u64(units.len() as u64);
                for unit in units {
                    enc.put_str(unit.id.as_str());
                    enc.put_u8(unit.kind as u8);
                }
                let mut streams: Vec<_> = streams.iter().collect();
                streams.sort_by(|a, b| a.id.cmp(&b.id));
                enc.put_u64(streams.len() as u64);
                for stream in streams {
                    put_stream(&mut enc, stream);
                }
                let mut elements: Vec<_> = elements.iter().collect();
                elements.sort();
                enc.put_u64(elements.len() as u64);
                for element in elements {
                    put_element(&mut enc, element);
                }
            }
            FlowsheetValue::Transfer { stream, amounts } => {
                enc.put_str(stream.as_str());
                put_amounts(&mut enc, amounts);
            }
            FlowsheetValue::Stocktake {
                unit,
                kind,
                amounts,
            } => {
                enc.put_str(unit.as_str());
                enc.put_u8(*kind as u8);
                put_amounts(&mut enc, amounts);
            }
        }
        blake3_hash(&enc.finish()).to_hex()
    }
}

/// A carrier's exact unit or outer-boundary account. All fields are kilograms.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CarrierBalance {
    pub opening_kg: ExactQty,
    pub feed_kg: ExactQty,
    pub product_kg: ExactQty,
    pub other_outflow_kg: ExactQty,
    pub closing_kg: ExactQty,
    pub inventory_change_kg: ExactQty,
    pub residual_kg: ExactQty,
}

/// One boundary's carriers and elemental receipts from the existing owner.
#[derive(Debug, Clone, PartialEq)]
pub struct MaterialBalance {
    pub dry_solids: CarrierBalance,
    pub water: CarrierBalance,
    pub elements: BTreeMap<ElementKey, MetalBalanceReceipt>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FlowsheetSummary {
    pub scope: FlowsheetScope,
    pub units: BTreeMap<UnitId, MaterialBalance>,
    /// Only external feeds and outflows, plus all unit opening/closing stocks.
    pub boundary: MaterialBalance,
    /// Edges used at both endpoints and cancelled from outer-boundary flows.
    pub internal_streams: Vec<StreamId>,
}

/// Immutable original observations and a claim over every unit and the boundary.
#[derive(Debug, Clone, PartialEq)]
pub struct FlowsheetReceipt {
    inputs: Vec<Claim<FlowsheetInput>>,
    claim: Claim<FlowsheetSummary>,
    digest: String,
}

impl FlowsheetReceipt {
    #[must_use]
    pub fn inputs(&self) -> &[Claim<FlowsheetInput>] {
        &self.inputs
    }
    #[must_use]
    pub fn summary(&self) -> &FlowsheetSummary {
        &self.claim.value
    }
    #[must_use]
    pub fn claim(&self) -> &Claim<FlowsheetSummary> {
        &self.claim
    }
    #[must_use]
    pub fn digest(&self) -> &str {
        &self.digest
    }
}

fn bad(code: RefusalCode, detail: impl Into<String>) -> Refusal {
    Refusal::new(code, format!("plant flowsheet: {}", detail.into()))
}
fn exact<T>(result: std::result::Result<T, UnitsError>) -> Result<T> {
    result.map_err(|_| {
        bad(
            RefusalCode::NumericalCertificateFailed207,
            "exact mass arithmetic is unrepresentable",
        )
    })
}
fn named(text: &str) -> bool {
    !text.is_empty() && text.trim() == text && !text.chars().any(char::is_control)
}
fn identifier(text: &str) -> Result<()> {
    if text.is_empty()
        || !text
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'-' | b'_' | b'.'))
    {
        return Err(bad(
            RefusalCode::UndeclaredUnitBasisOrTz107,
            "unit/stream IDs require ASCII letters, digits, '.', '_' or '-'",
        ));
    }
    Ok(())
}
fn put_qty(enc: &mut Encoder, value: ExactQty) {
    enc.put_i128(value.num());
    enc.put_i128(value.den());
}
fn put_time(enc: &mut Encoder, time: ValidTime) {
    enc.put_i64(time.start_ns);
    enc.put_i64(time.end_ns);
}
fn put_element(enc: &mut Encoder, element: &ElementKey) {
    enc.put_str(&element.element);
    enc.put_str(&element.measurand);
}
fn put_amounts(enc: &mut Encoder, amounts: &MaterialAmounts) {
    put_qty(enc, amounts.dry_solids_kg);
    put_qty(enc, amounts.water_kg);
    enc.put_u64(amounts.elements_kg.len() as u64);
    for (element, amount) in &amounts.elements_kg {
        put_element(enc, element);
        put_qty(enc, *amount);
    }
}
fn put_stream(enc: &mut Encoder, stream: &StreamSpec) {
    enc.put_str(stream.id.as_str());
    match &stream.route {
        StreamRoute::Feed { to } => {
            enc.put_u8(0);
            enc.put_str(to.as_str());
        }
        StreamRoute::Internal {
            from,
            to,
            source_role,
        } => {
            enc.put_u8(1);
            enc.put_str(from.as_str());
            enc.put_str(to.as_str());
            enc.put_u8(*source_role as u8);
        }
        StreamRoute::Product { from } => {
            enc.put_u8(2);
            enc.put_str(from.as_str());
        }
        StreamRoute::OtherOutflow { from } => {
            enc.put_u8(3);
            enc.put_str(from.as_str());
        }
    }
}

fn admit(input: &Claim<FlowsheetInput>) -> Result<()> {
    inventory::admit_source(
        input,
        INPUT_METRIC,
        &input.value.canonical_tag(),
        input.value.value.additivity(),
    )?;
    let scope = &input.value.scope;
    if !named(&scope.boundary) || scope.interval.start_ns >= scope.interval.end_ns {
        return Err(bad(
            RefusalCode::OutsidePhysicalDomain109,
            "name a physical boundary/version and positive reporting interval",
        ));
    }
    let expected = match input.value.value {
        FlowsheetValue::Stocktake {
            kind: StocktakeKind::Opening,
            ..
        } => ValidTime::instant(scope.interval.start_ns),
        FlowsheetValue::Stocktake {
            kind: StocktakeKind::Closing,
            ..
        } => ValidTime::instant(scope.interval.end_ns),
        _ => scope.interval,
    };
    if input.value.physical_time != expected {
        return Err(bad(
            RefusalCode::SupportMismatch102,
            "transfer/declaration interval or stocktake endpoint does not match its role",
        ));
    }
    Ok(())
}

fn validate_amounts(amounts: &MaterialAmounts, elements: &BTreeSet<ElementKey>) -> Result<()> {
    if !amounts.elements_kg.keys().eq(elements.iter()) {
        return Err(bad(
            RefusalCode::BasisMismatch105,
            "supply exactly the declared elemental measurands, including explicit zeros",
        ));
    }
    if amounts.dry_solids_kg.signum() < 0
        || amounts.water_kg.signum() < 0
        || amounts.elements_kg.values().any(|q| q.signum() < 0)
    {
        return Err(bad(
            RefusalCode::OutsidePhysicalDomain109,
            "carrier and elemental stocks/flows must be nonnegative",
        ));
    }
    let bulk = exact(amounts.dry_solids_kg.checked_add(amounts.water_kg))?;
    let mut tracked = ZERO;
    let mut non_water_elements = ZERO;
    for (element, amount) in &amounts.elements_kg {
        tracked = exact(tracked.checked_add(*amount))?;
        if !matches!(element.element.as_str(), "H" | "O") {
            non_water_elements = exact(non_water_elements.checked_add(*amount))?;
        }
    }
    if exact(tracked.checked_cmp(bulk))?.is_gt() {
        return Err(bad(
            RefusalCode::OutsidePhysicalDomain109,
            "distinct tracked elements exceed total dry matter plus water mass",
        ));
    }
    if exact(non_water_elements.checked_cmp(amounts.dry_solids_kg))?.is_gt() {
        return Err(bad(
            RefusalCode::OutsidePhysicalDomain109,
            "non-water elements exceed dry matter; the water carrier contains only H and O",
        ));
    }
    Ok(())
}

// These are projections of already admitted original source claims. Metadata
// never improves, and even structural zeros retain the complete declaration.
fn derived<T>(
    sources: &[&Claim<FlowsheetInput>],
    metric: &str,
    value: T,
    tag: &str,
    additivity: AdditivityClass,
) -> Result<Claim<T>> {
    let mut evidence = EvidenceClass::Derived;
    let mut epistemic = EpistemicStatus::KnownKnown;
    let mut state = KnowledgeState::Known;
    let mut confidentiality = Confidentiality::public();
    let mut parents = BTreeSet::new();
    let mut anchors = BTreeSet::new();
    let mut uu = BTreeMap::new();
    for source in sources {
        evidence = evidence.meet(source.evidence);
        epistemic = epistemic.and(source.epistemic);
        state = state.and(source.state);
        confidentiality = confidentiality.join(&source.confidentiality);
        parents.insert(source.claim_id);
        anchors.extend(source.anchors.iter().cloned());
        for term in &source.uu_terms {
            if let Some(previous) = uu.insert(term.scenario_id.clone(), term.clone())
                && previous != *term
            {
                return Err(bad(
                    RefusalCode::UnbudgetedUnknowns204,
                    "conflicting definitions for a shared UU scenario",
                ));
            }
        }
    }
    Ok(Claim::new(
        metric,
        value,
        tag,
        Representation::Deterministic,
        SpatialSupport::PeriodAggregate,
        additivity,
        evidence,
        epistemic,
        state,
        confidentiality,
        uu.into_values().collect(),
        parents.into_iter().collect(),
        anchors.into_iter().collect(),
    ))
}

type Source<'a> = &'a Claim<FlowsheetInput>;
#[derive(Default)]
pub(crate) struct Rows<'a> {
    pub(crate) opening: Vec<Source<'a>>,
    pub(crate) feed: Vec<Source<'a>>,
    pub(crate) product: Vec<Source<'a>>,
    pub(crate) other: Vec<Source<'a>>,
    pub(crate) closing: Vec<Source<'a>>,
}

/// Admitted original observations shared by exact accounting and conditional
/// reconciliation. Admission checks the graph and sources, not balance closure.
pub(crate) struct AdmittedFlowsheet<'a> {
    pub(crate) inputs: Vec<Source<'a>>,
    pub(crate) declaration: Source<'a>,
    pub(crate) schema: BTreeSet<ElementKey>,
    pub(crate) per_unit: BTreeMap<UnitId, Rows<'a>>,
    pub(crate) outer: Rows<'a>,
    pub(crate) internal_streams: Vec<StreamId>,
}

fn aggregate(rows: &[Source<'_>], elements: &BTreeSet<ElementKey>) -> Result<MaterialAmounts> {
    let mut total = MaterialAmounts {
        dry_solids_kg: ZERO,
        water_kg: ZERO,
        elements_kg: elements.iter().cloned().map(|e| (e, ZERO)).collect(),
    };
    for source in rows {
        let amounts = source.value.value.amounts().ok_or_else(|| {
            bad(
                RefusalCode::NoClaimInDomain201,
                "a declaration cannot substitute for a measured stock or transfer",
            )
        })?;
        total.dry_solids_kg = exact(total.dry_solids_kg.checked_add(amounts.dry_solids_kg))?;
        total.water_kg = exact(total.water_kg.checked_add(amounts.water_kg))?;
        for (element, amount) in &amounts.elements_kg {
            let sum = total.elements_kg.get_mut(element).ok_or_else(|| {
                bad(
                    RefusalCode::BasisMismatch105,
                    "undeclared element in aggregation",
                )
            })?;
            *sum = exact(sum.checked_add(*amount))?;
        }
    }
    Ok(total)
}

fn carrier(boundary: &str, label: &str, q: [ExactQty; 5]) -> Result<CarrierBalance> {
    let [opening, feed, product, other, closing] = q;
    let residual = inventory::conservation_residual(
        opening,
        feed,
        exact(product.checked_add(other))?,
        closing,
    )?;
    if residual != ZERO {
        return Err(bad(
            RefusalCode::Conservation101,
            format!(
                "{boundary}: {label} residual {}/{} kg; no balancing stream inserted",
                residual.num(),
                residual.den()
            ),
        ));
    }
    Ok(CarrierBalance {
        opening_kg: opening,
        feed_kg: feed,
        product_kg: product,
        other_outflow_kg: other,
        closing_kg: closing,
        inventory_change_kg: exact(closing.checked_sub(opening))?,
        residual_kg: residual,
    })
}

fn balance(
    boundary: &str,
    declaration: Source<'_>,
    rows: &Rows<'_>,
    elements: &BTreeSet<ElementKey>,
) -> Result<MaterialBalance> {
    let groups = [
        &rows.opening,
        &rows.feed,
        &rows.product,
        &rows.other,
        &rows.closing,
    ];
    let mut totals = Vec::with_capacity(5);
    for group in groups {
        totals.push(aggregate(group, elements)?);
    }
    let dry_solids = carrier(
        boundary,
        "dry solids",
        std::array::from_fn(|i| totals[i].dry_solids_kg),
    )?;
    let water = carrier(
        boundary,
        "water",
        std::array::from_fn(|i| totals[i].water_kg),
    )?;
    let interval = declaration.value.scope.interval;
    let mut elemental = BTreeMap::new();
    for element in elements {
        let scope = BalanceScope {
            boundary: boundary.into(),
            interval,
            element: element.element.clone(),
            measurand: element.measurand.clone(),
        };
        let population = MetalBalanceInput {
            scope: scope.clone(),
            physical_time: interval,
            value: BalanceValue::Population { complete: true },
        };
        let tag = population.canonical_tag();
        let mut inputs = vec![derived(
            &[declaration],
            inventory::INPUT_METRIC,
            population,
            &tag,
            AdditivityClass::NonAdditive,
        )?];
        for (i, total) in totals.iter().enumerate() {
            let mass = *total
                .elements_kg
                .get(element)
                .ok_or_else(|| bad(RefusalCode::BasisMismatch105, "missing aggregate element"))?;
            let value = match i {
                0 => BalanceValue::OpeningInventory(mass),
                1 => BalanceValue::Feed(mass),
                2 => BalanceValue::Product(mass),
                3 => BalanceValue::OtherOutflow(mass),
                _ => BalanceValue::ClosingInventory(mass),
            };
            let physical_time = match i {
                0 => ValidTime::instant(interval.start_ns),
                4 => ValidTime::instant(interval.end_ns),
                _ => interval,
            };
            let input = MetalBalanceInput {
                scope: scope.clone(),
                physical_time,
                value,
            };
            let tag = input.canonical_tag();
            let mut sources = vec![declaration];
            sources.extend(groups[i].iter().copied());
            inputs.push(derived(
                &sources,
                inventory::INPUT_METRIC,
                input,
                &tag,
                AdditivityClass::ExtensiveAdditive,
            )?);
        }
        let receipt = inventory::account_interval(&inputs).map_err(|mut refusal| {
            refusal.what = format!("{boundary}: {}", refusal.what);
            refusal
        })?;
        elemental.insert(element.clone(), receipt);
    }
    Ok(MaterialBalance {
        dry_solids,
        water,
        elements: elemental,
    })
}

fn put_balance(enc: &mut Encoder, balance: &MaterialBalance) {
    for carrier in [&balance.dry_solids, &balance.water] {
        for q in [
            carrier.opening_kg,
            carrier.feed_kg,
            carrier.product_kg,
            carrier.other_outflow_kg,
            carrier.closing_kg,
            carrier.inventory_change_kg,
            carrier.residual_kg,
        ] {
            put_qty(enc, q);
        }
    }
    enc.put_u64(balance.elements.len() as u64);
    for (element, receipt) in &balance.elements {
        put_element(enc, element);
        enc.put_str(receipt.digest());
    }
}

pub(crate) fn admit_flowsheet(inputs: &[Claim<FlowsheetInput>]) -> Result<AdmittedFlowsheet<'_>> {
    admit_flowsheet_with_unmeasured(inputs, &BTreeSet::new())
}

/// Shared source/topology admission. Only the partial reconciliation owner may
/// omit explicitly named stream observations; stocktakes remain measured.
/// Rows then contain measured contributions, not complete nominal balances.
pub(crate) fn admit_flowsheet_with_unmeasured<'a>(
    inputs: &'a [Claim<FlowsheetInput>],
    unmeasured: &BTreeSet<StreamId>,
) -> Result<AdmittedFlowsheet<'a>> {
    let mut inputs: Vec<_> = inputs.iter().collect();
    inputs.sort_by(|a, b| a.value.value.key().cmp(&b.value.value.key()));
    let declaration = *inputs.first().ok_or_else(|| {
        bad(
            RefusalCode::NoClaimInDomain201,
            "missing complete flowsheet declaration",
        )
    })?;
    let FlowsheetValue::Declaration {
        units,
        streams,
        elements,
        complete: true,
    } = &declaration.value.value
    else {
        return Err(bad(
            RefusalCode::NoClaimInDomain201,
            "requires an explicit complete flowsheet declaration",
        ));
    };
    for &source in &inputs {
        admit(source)?;
        if source.value.scope != declaration.value.scope {
            return Err(bad(
                RefusalCode::SupportMismatch102,
                "inputs disagree on plant boundary/version or interval",
            ));
        }
    }
    if inputs
        .windows(2)
        .any(|p| p[0].value.value.key() == p[1].value.value.key())
    {
        return Err(bad(
            RefusalCode::BasisMismatch105,
            "duplicate declaration, transfer or unit stocktake role",
        ));
    }
    if units.is_empty() {
        return Err(bad(
            RefusalCode::NoClaimInDomain201,
            "declare at least one physical unit",
        ));
    }
    let mut unit_ids = BTreeSet::new();
    for unit in units {
        if !unit_ids.insert(unit.id.clone()) {
            return Err(bad(
                RefusalCode::BasisMismatch105,
                "duplicate unit identity",
            ));
        }
    }
    let mut schema = BTreeSet::new();
    let mut symbols = BTreeSet::new();
    for element in elements {
        let symbol = element.element.as_bytes();
        if !(symbol.len() == 1 || symbol.len() == 2)
            || !symbol[0].is_ascii_uppercase()
            || (symbol.len() == 2 && !symbol[1].is_ascii_lowercase())
            || !named(&element.measurand)
        {
            return Err(bad(
                RefusalCode::UndeclaredUnitBasisOrTz107,
                "declare a case-sensitive element and conserved measurand",
            ));
        }
        if !schema.insert(element.clone()) || !symbols.insert(element.element.clone()) {
            return Err(bad(
                RefusalCode::BasisMismatch105,
                "each distinct element needs exactly one measurand",
            ));
        }
    }
    let mut routes = BTreeMap::new();
    for stream in streams {
        if routes.insert(stream.id.clone(), &stream.route).is_some() {
            return Err(bad(
                RefusalCode::BasisMismatch105,
                "duplicate declared stream identity",
            ));
        }
        let valid = match &stream.route {
            StreamRoute::Feed { to } => unit_ids.contains(to),
            StreamRoute::Internal { from, to, .. } => {
                from != to && unit_ids.contains(from) && unit_ids.contains(to)
            }
            StreamRoute::Product { from } | StreamRoute::OtherOutflow { from } => {
                unit_ids.contains(from)
            }
        };
        if !valid {
            return Err(bad(
                RefusalCode::BasisMismatch105,
                "stream endpoint is undeclared or an internal self-loop",
            ));
        }
    }
    if unmeasured.iter().any(|stream| !routes.contains_key(stream)) {
        return Err(bad(
            RefusalCode::BasisMismatch105,
            "an explicitly unmeasured stream is absent from the declared topology",
        ));
    }
    let mut observations = BTreeMap::new();
    let mut stocks = BTreeMap::new();
    for &source in &inputs {
        if let Some(amounts) = source.value.value.amounts() {
            validate_amounts(amounts, &schema)?;
        }
        match &source.value.value {
            FlowsheetValue::Declaration { .. } => {}
            FlowsheetValue::Transfer { stream, .. } => {
                if unmeasured.contains(stream) {
                    return Err(bad(
                        RefusalCode::BasisMismatch105,
                        "a stream cannot be both measured and explicitly unmeasured",
                    ));
                }
                if !routes.contains_key(stream) {
                    return Err(bad(
                        RefusalCode::NoClaimInDomain201,
                        "transfer has no declared stream",
                    ));
                }
                observations.insert(stream.clone(), source);
            }
            FlowsheetValue::Stocktake { unit, kind, .. } => {
                if !unit_ids.contains(unit) {
                    return Err(bad(
                        RefusalCode::NoClaimInDomain201,
                        "stocktake has no declared unit",
                    ));
                }
                stocks.insert((unit.clone(), *kind), source);
            }
        }
    }
    let mut per_unit: BTreeMap<UnitId, Rows<'_>> = BTreeMap::new();
    let mut outer = Rows::default();
    for unit in &unit_ids {
        let opening = *stocks
            .get(&(unit.clone(), StocktakeKind::Opening))
            .ok_or_else(|| {
                bad(
                    RefusalCode::NoClaimInDomain201,
                    format!("{} missing opening stocktake", unit.as_str()),
                )
            })?;
        let closing = *stocks
            .get(&(unit.clone(), StocktakeKind::Closing))
            .ok_or_else(|| {
                bad(
                    RefusalCode::NoClaimInDomain201,
                    format!("{} missing closing stocktake", unit.as_str()),
                )
            })?;
        per_unit.insert(
            unit.clone(),
            Rows {
                opening: vec![opening],
                closing: vec![closing],
                ..Rows::default()
            },
        );
        outer.opening.push(opening);
        outer.closing.push(closing);
    }
    let mut internal_streams = Vec::new();
    for (stream, route) in &routes {
        if unmeasured.contains(stream) {
            if matches!(route, StreamRoute::Internal { .. }) {
                internal_streams.push(stream.clone());
            }
            continue;
        }
        let source = *observations.get(stream).ok_or_else(|| {
            bad(
                RefusalCode::NoClaimInDomain201,
                format!("{} missing transfer; no implicit zero", stream.as_str()),
            )
        })?;
        match route {
            StreamRoute::Feed { to } => {
                rows_mut(&mut per_unit, to)?.feed.push(source);
                outer.feed.push(source);
            }
            StreamRoute::Internal {
                from,
                to,
                source_role,
            } => {
                match source_role {
                    OutflowRole::Product => rows_mut(&mut per_unit, from)?.product.push(source),
                    OutflowRole::Other => rows_mut(&mut per_unit, from)?.other.push(source),
                }
                rows_mut(&mut per_unit, to)?.feed.push(source);
                internal_streams.push(stream.clone());
            }
            StreamRoute::Product { from } => {
                rows_mut(&mut per_unit, from)?.product.push(source);
                outer.product.push(source);
            }
            StreamRoute::OtherOutflow { from } => {
                rows_mut(&mut per_unit, from)?.other.push(source);
                outer.other.push(source);
            }
        }
    }
    Ok(AdmittedFlowsheet {
        inputs,
        declaration,
        schema,
        per_unit,
        outer,
        internal_streams,
    })
}

/// Check every declared unit, then independently close the outer boundary.
/// Recycle cycles are allowed with explicitly supplied stream amounts; this
/// evaluates no process equation and does not solve unknown stream flows.
pub fn account_flowsheet(inputs: &[Claim<FlowsheetInput>]) -> Result<FlowsheetReceipt> {
    let admitted = admit_flowsheet(inputs)?;
    let AdmittedFlowsheet {
        inputs,
        declaration,
        schema,
        per_unit,
        outer,
        internal_streams,
    } = admitted;
    let scope = declaration.value.scope.clone();
    let mut unit_balances = BTreeMap::new();
    for (unit, rows) in &per_unit {
        unit_balances.insert(
            unit.clone(),
            balance(
                &format!("{}/unit/{}", scope.boundary, unit.as_str()),
                declaration,
                rows,
                &schema,
            )?,
        );
    }
    let boundary = balance(&scope.boundary, declaration, &outer, &schema)?;
    let summary = FlowsheetSummary {
        scope,
        units: unit_balances,
        boundary,
        internal_streams,
    };
    let mut enc = Encoder::new_frame(*b"FLWR", 1);
    enc.put_u64(inputs.len() as u64);
    for source in &inputs {
        inventory::put_source_envelope(&mut enc, source, source.value.value.key().0);
    }
    enc.put_u64(summary.units.len() as u64);
    for (unit, balance) in &summary.units {
        enc.put_str(unit.as_str());
        put_balance(&mut enc, balance);
    }
    put_balance(&mut enc, &summary.boundary);
    enc.put_u64(summary.internal_streams.len() as u64);
    for stream in &summary.internal_streams {
        enc.put_str(stream.as_str());
    }
    let digest = blake3_hash(&enc.finish()).to_hex();
    let claim = derived(
        &inputs,
        OUTPUT_METRIC,
        summary,
        &digest,
        AdditivityClass::NonAdditive,
    )?;
    Ok(FlowsheetReceipt {
        inputs: inputs.into_iter().cloned().collect(),
        claim,
        digest,
    })
}

fn rows_mut<'a, 's>(
    units: &'a mut BTreeMap<UnitId, Rows<'s>>,
    unit: &UnitId,
) -> Result<&'a mut Rows<'s>> {
    units
        .get_mut(unit)
        .ok_or_else(|| bad(RefusalCode::BasisMismatch105, "undeclared unit endpoint"))
}

#[cfg(test)]
mod tests;
