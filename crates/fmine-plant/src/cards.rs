//! DESIGN: source-bound nominal recovery cards (plan §17.3; BD-176/287).
//! Empirical formulas consume numeric coordinates in declared source units.
//! Four source/version layers remain separate: test work, ordered operational
//! factors, explicit correction/cap, and the assumed processing route.
//! This evaluates a declared card and optional declared joint input population;
//! it does not fit parameters, transfer sample support, manufacture uncertainty,
//! calibrate a forecast or authorize plant control.
//! Determinism: DeterministicPerIsa

use crate::{inventory, reconciliation::Metadata};
use fmine_claim::{AdditivityClass, Claim, ClaimId};
use fmine_core::{codec::Encoder, hash::blake3_hash};
use fmine_evidence::{Refusal, RefusalCode};
use fmine_time::ValidTime;
use fmine_units::{ExactQty, Qty};
use std::collections::{BTreeMap, BTreeSet};

pub mod ensemble;
mod formula;
#[cfg(test)]
mod tests;
pub mod worksheet;

type Result<T> = std::result::Result<T, Refusal>;
pub const TEST_WORK_METRIC: &str = "plant.recovery.test_work_card/1";
pub const OPERATION_METRIC: &str = "plant.recovery.operational_factor/1";
pub const CORRECTION_METRIC: &str = "plant.recovery.correction/1";
pub const ROUTE_METRIC: &str = "plant.recovery.route/1";
pub const INPUT_METRIC: &str = "plant.recovery.inputs/1";
pub const OUTPUT_METRIC: &str = "plant.recovery.nominal_evaluation/1";
pub const MAX_INPUTS: usize = 32;
pub const MAX_OPERATIONAL_FACTORS: usize = 8;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RecoveryFraction;
pub type Fraction = Qty<RecoveryFraction>;

/// Units refer to numerical source coordinates, including the assay's basis.
/// No automatic conversion occurs at formula evaluation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputUnit {
    GramsPerDryMetricTonne,
    TroyOuncesPerDryShortTon,
    DryMassPercent,
    DryMassFraction,
    PartsPerMillionDryMass,
    SpikeTestPercent,
    Micrometres,
    Hours,
}
impl InputUnit {
    fn tag(self) -> u8 {
        match self {
            Self::GramsPerDryMetricTonne => 0,
            Self::TroyOuncesPerDryShortTon => 1,
            Self::DryMassPercent => 2,
            Self::DryMassFraction => 3,
            Self::PartsPerMillionDryMass => 4,
            Self::SpikeTestPercent => 5,
            Self::Micrometres => 6,
            Self::Hours => 7,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputScale {
    Fraction,
    Percent,
}

/// A version names a source's semantic revision, not a mutable display label.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CardVersion {
    pub id: String,
    pub revision: String,
}

/// Recovery of one named element at one circuit, domain and matched dry state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecoveryScope {
    pub circuit: String,
    pub material_domain: String,
    pub dry_basis: String,
    pub element: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InputRequirement {
    pub measurand: String,
    pub unit: InputUnit,
    /// Source-domain bounds in exactly this coordinate's unit.
    pub lower: ExactQty,
    pub upper: ExactQty,
    pub lower_inclusive: bool,
    pub upper_inclusive: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TestWorkCard {
    pub version: CardVersion,
    pub scope: RecoveryScope,
    pub valid_time: ValidTime,
    pub input_units: BTreeMap<String, InputRequirement>,
    /// ASCII + - * /, parentheses, exp/ln, and integer powers [-16,16].
    /// Coefficients are specific to input_units and output_scale.
    pub formula: String,
    pub output_scale: OutputScale,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OperationalFactor {
    pub version: CardVersion,
    pub test_work: ClaimId,
    /// Dimensionless nonnegative multiplier; it is not percentage-point addition.
    pub multiplier: ExactQty,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecoveryCorrection {
    pub version: CardVersion,
    pub test_work: ClaimId,
    /// Signed additive fraction; +0.03 means +3 percentage points.
    pub additive_fraction: ExactQty,
    /// Optional explicitly sourced upper policy cap in [0,1].
    pub upper_cap_fraction: Option<ExactQty>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RouteAssumption {
    pub version: CardVersion,
    pub test_work: ClaimId,
    pub route: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InputValue {
    pub measurand: String,
    pub unit: InputUnit,
    pub value: ExactQty,
}

/// Complete simultaneous processing-support inputs, already aggregated by the
/// caller's supported method. Block/sample predictions cannot substitute here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecoveryInputs {
    pub scope: RecoveryScope,
    pub interval: ValidTime,
    pub values: BTreeMap<String, InputValue>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RecoveryEvaluation {
    pub scope: RecoveryScope,
    pub interval: ValidTime,
    pub test_work_fraction: Fraction,
    /// One intermediate after each declared factor, preserving caller order.
    pub operational_fractions: Vec<Fraction>,
    pub corrected_before_cap: Fraction,
    pub cap_applied: bool,
    pub recovery_fraction: Fraction,
    pub route: String,
}

/// Parse-once compiled card, retaining all original source/version layers.
#[derive(Debug, Clone, PartialEq)]
pub struct RecoveryModel {
    test_work: Claim<TestWorkCard>,
    operations: Vec<Claim<OperationalFactor>>,
    correction: Claim<RecoveryCorrection>,
    route: Claim<RouteAssumption>,
    program: formula::Formula,
    names: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RecoveryReceipt {
    model: RecoveryModel,
    inputs: Claim<RecoveryInputs>,
    claim: Claim<RecoveryEvaluation>,
    digest: String,
}
impl RecoveryReceipt {
    pub fn model(&self) -> &RecoveryModel {
        &self.model
    }
    pub fn inputs(&self) -> &Claim<RecoveryInputs> {
        &self.inputs
    }
    pub fn claim(&self) -> &Claim<RecoveryEvaluation> {
        &self.claim
    }
    pub fn digest(&self) -> &str {
        &self.digest
    }
}

impl RecoveryModel {
    /// Compile and admit the four source layers once. Syntax, bound variables,
    /// work limits and exact card-version references are checked before use.
    pub fn compile(
        test_work: &Claim<TestWorkCard>,
        operations: &[Claim<OperationalFactor>],
        correction: &Claim<RecoveryCorrection>,
        route: &Claim<RouteAssumption>,
    ) -> Result<Self> {
        admit(
            test_work,
            TEST_WORK_METRIC,
            &test_work.value.canonical_tag(),
        )?;
        let card = &test_work.value;
        version(&card.version)?;
        scope(&card.scope)?;
        interval(card.valid_time)?;
        if card.input_units.len() > MAX_INPUTS || operations.len() > MAX_OPERATIONAL_FACTORS {
            return Err(fail(
                RefusalCode::NoClaimInDomain201,
                "bounded card input/layer count exceeded",
            ));
        }
        for (name, spec) in &card.input_units {
            if !identifier(name)
                || matches!(name.as_str(), "ln" | "LN" | "exp" | "EXP")
                || !named(&spec.measurand)
            {
                return Err(fail(
                    RefusalCode::UndeclaredUnitBasisOrTz107,
                    "declare unique coordinate names and measurands",
                ));
            }
            let ordering = compare(spec.lower, spec.upper)?;
            if ordering.is_gt()
                || (ordering.is_eq() && (!spec.lower_inclusive || !spec.upper_inclusive))
            {
                return Err(fail(
                    RefusalCode::OutsidePhysicalDomain109,
                    "empty or inverted card validity interval",
                ));
            }
            physical_coordinate(spec.lower, spec.unit)?;
            physical_coordinate(spec.upper, spec.unit)?;
            numeric(spec.lower)?;
            numeric(spec.upper)?;
        }
        let mut versions = BTreeSet::new();
        for factor in operations {
            admit(factor, OPERATION_METRIC, &factor.value.canonical_tag())?;
            target(
                &factor.value.version,
                factor.value.test_work,
                test_work.claim_id,
            )?;
            if !versions.insert((
                factor.value.version.id.clone(),
                factor.value.version.revision.clone(),
            )) || factor.value.multiplier.signum() < 0
            {
                return Err(fail(
                    RefusalCode::OutsidePhysicalDomain109,
                    "duplicate operational version or negative multiplier",
                ));
            }
            numeric(factor.value.multiplier)?;
        }
        admit(
            correction,
            CORRECTION_METRIC,
            &correction.value.canonical_tag(),
        )?;
        target(
            &correction.value.version,
            correction.value.test_work,
            test_work.claim_id,
        )?;
        numeric(correction.value.additive_fraction)?;
        if let Some(cap) = correction.value.upper_cap_fraction {
            if cap.signum() < 0 || compare(cap, ExactQty::from_integer(1))?.is_gt() {
                return Err(fail(
                    RefusalCode::OutsidePhysicalDomain109,
                    "explicit recovery cap must lie in [0,1]",
                ));
            }
            numeric(cap)?;
        }
        admit(route, ROUTE_METRIC, &route.value.canonical_tag())?;
        target(
            &route.value.version,
            route.value.test_work,
            test_work.claim_id,
        )?;
        if !named(&route.value.route) {
            return Err(fail(
                RefusalCode::NoClaimInDomain201,
                "processing route must be explicitly named",
            ));
        }
        let names: Vec<_> = card.input_units.keys().cloned().collect();
        let program = formula::Formula::compile(&card.formula, &names)?;
        Ok(Self {
            test_work: test_work.clone(),
            operations: operations.to_vec(),
            correction: correction.clone(),
            route: route.clone(),
            program,
            names,
        })
    }

    pub fn test_work(&self) -> &Claim<TestWorkCard> {
        &self.test_work
    }
    pub fn operations(&self) -> &[Claim<OperationalFactor>] {
        &self.operations
    }
    pub fn correction(&self) -> &Claim<RecoveryCorrection> {
        &self.correction
    }
    pub fn route(&self) -> &Claim<RouteAssumption> {
        &self.route
    }

    /// Evaluate a conditional nominal recovery. Exact bounds are checked before
    /// numerical conversion. No extrapolation, unit conversion or implicit cap.
    pub fn evaluate(&self, inputs: &Claim<RecoveryInputs>) -> Result<RecoveryReceipt> {
        admit(inputs, INPUT_METRIC, &inputs.value.canonical_tag())?;
        let query = &inputs.value;
        let card = &self.test_work.value;
        interval(query.interval)?;
        if query.scope != card.scope {
            return Err(fail(
                RefusalCode::SupportMismatch102,
                "card circuit/domain/dry basis/element differs from inputs",
            ));
        }
        if query.interval.start_ns < card.valid_time.start_ns
            || query.interval.end_ns > card.valid_time.end_ns
        {
            return Err(fail(
                RefusalCode::OutsideAoa203,
                "processing interval lies outside this card version's validity",
            ));
        }
        if query.values.len() != self.names.len() {
            return Err(fail(
                RefusalCode::NoClaimInDomain201,
                "supply exactly the declared card coordinates",
            ));
        }
        let mut coordinates = [0.0; MAX_INPUTS];
        for (i, name) in self.names.iter().enumerate() {
            let value = query.values.get(name).ok_or_else(|| {
                fail(
                    RefusalCode::NoClaimInDomain201,
                    "missing named card coordinate",
                )
            })?;
            let spec = &card.input_units[name];
            if value.unit != spec.unit || value.measurand != spec.measurand {
                return Err(fail(
                    RefusalCode::UndeclaredUnitBasisOrTz107,
                    format!("coordinate {name} has a different unit or measurand"),
                ));
            }
            let low = compare(value.value, spec.lower)?;
            let high = compare(value.value, spec.upper)?;
            if low.is_lt()
                || high.is_gt()
                || (low.is_eq() && !spec.lower_inclusive)
                || (high.is_eq() && !spec.upper_inclusive)
            {
                return Err(fail(
                    RefusalCode::OutsideAoa203,
                    format!("coordinate {name} lies outside its exact validity bounds"),
                ));
            }
            coordinates[i] = numeric(value.value)?;
        }
        let raw = self.program.evaluate(&coordinates[..self.names.len()])?;
        let initial = finite(match card.output_scale {
            OutputScale::Fraction => raw,
            OutputScale::Percent => raw / 100.0,
        })?;
        if raw != 0.0 && initial == 0.0 {
            return Err(fail(
                RefusalCode::NumericalCertificateFailed207,
                "output-scale conversion underflows",
            ));
        }
        if initial < 0.0 {
            return Err(fail(
                RefusalCode::OutsidePhysicalDomain109,
                "test-work formula produces negative recovery",
            ));
        }
        let mut result = initial;
        let mut operational_fractions = Vec::with_capacity(self.operations.len());
        for factor in &self.operations {
            let multiplier = numeric(factor.value.multiplier)?;
            let next = finite(result * multiplier)?;
            if result != 0.0 && multiplier != 0.0 && next == 0.0 {
                return Err(fail(
                    RefusalCode::NumericalCertificateFailed207,
                    "operational factor product underflows",
                ));
            }
            result = next;
            operational_fractions.push(Fraction::new(result));
        }
        let corrected = finite(result + numeric(self.correction.value.additive_fraction)?)?;
        let (final_value, cap_applied) = if let Some(cap) = self.correction.value.upper_cap_fraction
        {
            let cap = numeric(cap)?;
            (corrected.min(cap), corrected > cap)
        } else {
            (corrected, false)
        };
        if !(0.0..=1.0).contains(&final_value) {
            return Err(fail(
                RefusalCode::OutsidePhysicalDomain109,
                "declared layers produce recovery outside [0,1]",
            ));
        }
        let value = RecoveryEvaluation {
            scope: query.scope.clone(),
            interval: query.interval,
            test_work_fraction: Fraction::new(initial),
            operational_fractions,
            corrected_before_cap: Fraction::new(corrected),
            cap_applied,
            recovery_fraction: Fraction::new(final_value),
            route: self.route.value.route.clone(),
        };
        let mut metadata = Metadata::new();
        let mut enc = Encoder::new_frame(*b"RCER", 1);
        include(&mut metadata, &mut enc, &self.test_work, 0)?;
        enc.put_u64(self.operations.len() as u64);
        for factor in &self.operations {
            include(&mut metadata, &mut enc, factor, 1)?;
        }
        include(&mut metadata, &mut enc, &self.correction, 2)?;
        include(&mut metadata, &mut enc, &self.route, 3)?;
        include(&mut metadata, &mut enc, inputs, 4)?;
        enc.put_u64(initial.to_bits());
        for step in &value.operational_fractions {
            enc.put_u64(step.value().to_bits());
        }
        enc.put_u64(corrected.to_bits());
        enc.put_u8(u8::from(cap_applied));
        enc.put_u64(final_value.to_bits());
        let digest = blake3_hash(&enc.finish()).to_hex();
        let claim = metadata.into_claim(OUTPUT_METRIC, value, &digest);
        Ok(RecoveryReceipt {
            model: self.clone(),
            inputs: inputs.clone(),
            claim,
            digest,
        })
    }
}

impl TestWorkCard {
    pub fn canonical_tag(&self) -> String {
        let mut e = Encoder::new_frame(*b"RCTW", 1);
        put_version(&mut e, &self.version);
        put_scope(&mut e, &self.scope);
        put_time(&mut e, self.valid_time);
        e.put_u64(self.input_units.len() as u64);
        for (name, s) in &self.input_units {
            e.put_str(name);
            e.put_str(&s.measurand);
            e.put_u8(s.unit.tag());
            put_exact(&mut e, s.lower);
            put_exact(&mut e, s.upper);
            e.put_u8(u8::from(s.lower_inclusive));
            e.put_u8(u8::from(s.upper_inclusive));
        }
        e.put_str(&self.formula);
        e.put_u8(match self.output_scale {
            OutputScale::Fraction => 0,
            OutputScale::Percent => 1,
        });
        blake3_hash(&e.finish()).to_hex()
    }
}
impl OperationalFactor {
    pub fn canonical_tag(&self) -> String {
        let mut e = layer_encoder(*b"RCOP", &self.version, self.test_work);
        put_exact(&mut e, self.multiplier);
        blake3_hash(&e.finish()).to_hex()
    }
}
impl RecoveryCorrection {
    pub fn canonical_tag(&self) -> String {
        let mut e = layer_encoder(*b"RCCO", &self.version, self.test_work);
        put_exact(&mut e, self.additive_fraction);
        e.put_u8(u8::from(self.upper_cap_fraction.is_some()));
        if let Some(v) = self.upper_cap_fraction {
            put_exact(&mut e, v);
        }
        blake3_hash(&e.finish()).to_hex()
    }
}
impl RouteAssumption {
    pub fn canonical_tag(&self) -> String {
        let mut e = layer_encoder(*b"RCRT", &self.version, self.test_work);
        e.put_str(&self.route);
        blake3_hash(&e.finish()).to_hex()
    }
}
impl RecoveryInputs {
    pub fn canonical_tag(&self) -> String {
        let mut e = Encoder::new_frame(*b"RCIN", 1);
        put_scope(&mut e, &self.scope);
        put_time(&mut e, self.interval);
        e.put_u64(self.values.len() as u64);
        for (name, v) in &self.values {
            e.put_str(name);
            e.put_str(&v.measurand);
            e.put_u8(v.unit.tag());
            put_exact(&mut e, v.value);
        }
        blake3_hash(&e.finish()).to_hex()
    }
}

fn fail(code: RefusalCode, message: impl Into<String>) -> Refusal {
    Refusal::new(code, format!("recovery card: {}", message.into()))
}
fn finite(v: f64) -> Result<f64> {
    if v.is_finite() {
        Ok(v)
    } else {
        Err(fail(
            RefusalCode::NumericalCertificateFailed207,
            "non-finite card arithmetic",
        ))
    }
}
fn numeric(v: ExactQty) -> Result<f64> {
    let x = finite(v.to_f64())?;
    if x == 0.0 && v.signum() != 0 {
        return Err(fail(
            RefusalCode::NumericalCertificateFailed207,
            "exact coordinate loses numerical range",
        ));
    }
    Ok(x)
}
fn compare(a: ExactQty, b: ExactQty) -> Result<std::cmp::Ordering> {
    a.checked_cmp(b).map_err(|_| {
        fail(
            RefusalCode::NumericalCertificateFailed207,
            "exact validity comparison overflows",
        )
    })
}
fn physical_coordinate(value: ExactQty, unit: InputUnit) -> Result<()> {
    let upper = match unit {
        InputUnit::GramsPerDryMetricTonne | InputUnit::PartsPerMillionDryMass => {
            Some(ExactQty::from_integer(1_000_000))
        }
        InputUnit::TroyOuncesPerDryShortTon => Some(
            ExactQty::from_integer(1_000_000)
                .g_per_t_to_oz_per_st()
                .map_err(|_| {
                    fail(
                        RefusalCode::NumericalCertificateFailed207,
                        "statutory grade conversion failed",
                    )
                })?,
        ),
        InputUnit::DryMassPercent | InputUnit::SpikeTestPercent => {
            Some(ExactQty::from_integer(100))
        }
        InputUnit::DryMassFraction => Some(ExactQty::from_integer(1)),
        InputUnit::Micrometres | InputUnit::Hours => None,
    };
    let exceeds_upper = match upper {
        Some(upper) => compare(value, upper)?.is_gt(),
        None => false,
    };
    if value.signum() < 0
        || (unit == InputUnit::Micrometres && value.signum() == 0)
        || exceeds_upper
    {
        return Err(fail(
            RefusalCode::OutsidePhysicalDomain109,
            "validity bound violates the coordinate's physical domain",
        ));
    }
    Ok(())
}
fn named(s: &str) -> bool {
    !s.is_empty() && s.trim() == s && !s.chars().any(char::is_control)
}
fn identifier(s: &str) -> bool {
    s.len() <= 64
        && s.as_bytes()
            .first()
            .is_some_and(|b| b.is_ascii_alphabetic() || *b == b'_')
        && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
}
fn version(v: &CardVersion) -> Result<()> {
    if named(&v.id) && named(&v.revision) {
        Ok(())
    } else {
        Err(fail(
            RefusalCode::NoClaimInDomain201,
            "every layer needs a source version",
        ))
    }
}
fn scope(s: &RecoveryScope) -> Result<()> {
    if [&s.circuit, &s.material_domain, &s.dry_basis, &s.element]
        .into_iter()
        .all(|s| named(s))
    {
        Ok(())
    } else {
        Err(fail(
            RefusalCode::UndeclaredUnitBasisOrTz107,
            "declare circuit, domain, dry basis and element",
        ))
    }
}
fn interval(t: ValidTime) -> Result<()> {
    if t.start_ns < t.end_ns {
        Ok(())
    } else {
        Err(fail(
            RefusalCode::OutsidePhysicalDomain109,
            "positive physical interval required",
        ))
    }
}
fn target(v: &CardVersion, actual: ClaimId, expected: ClaimId) -> Result<()> {
    version(v)?;
    if actual == expected {
        Ok(())
    } else {
        Err(fail(
            RefusalCode::FrozenRecordMutation507,
            "layer targets a different test-work version",
        ))
    }
}
fn admit<T>(c: &Claim<T>, metric: &str, tag: &str) -> Result<()> {
    inventory::admit_source(c, metric, tag, AdditivityClass::NonAdditive)
}
fn include<T>(m: &mut Metadata, e: &mut Encoder, c: &Claim<T>, role: u8) -> Result<()> {
    m.include(c)?;
    inventory::put_source_envelope(e, c, role);
    Ok(())
}
fn put_exact(e: &mut Encoder, v: ExactQty) {
    e.put_i128(v.num());
    e.put_i128(v.den());
}
fn put_version(e: &mut Encoder, v: &CardVersion) {
    e.put_str(&v.id);
    e.put_str(&v.revision);
}
fn put_scope(e: &mut Encoder, s: &RecoveryScope) {
    e.put_str(&s.circuit);
    e.put_str(&s.material_domain);
    e.put_str(&s.dry_basis);
    e.put_str(&s.element);
}
fn put_time(e: &mut Encoder, t: ValidTime) {
    e.put_i64(t.start_ns);
    e.put_i64(t.end_ns);
}
fn layer_encoder(magic: [u8; 4], version: &CardVersion, target: ClaimId) -> Encoder {
    let mut e = Encoder::new_frame(magic, 1);
    put_version(&mut e, version);
    e.put_str(&target.to_string());
    e
}
