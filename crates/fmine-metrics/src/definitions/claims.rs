//! Claim-bound exact AISC/AIC worksheets (BD-061).
//! Determinism: Exact; inherited UU and colour metadata are bound by their exact floating-point bits.
//!
//! The rule Claim explicitly declares that supplied lines form the complete cost population for one
//! scope. That declaration is evidence, not an inferred zero for every omitted registered component.
//! Source authentication and classification of an issuer's accounts belong to the caller.

use std::collections::{BTreeMap, BTreeSet};

use fmine_claim::{AdditivityClass, Claim, Confidentiality, Representation, SpatialSupport};
use fmine_core::{Encoder, ObjectId, blake3_hash};
use fmine_evidence::color::validate_color_payload;
use fmine_evidence::{Color, EpistemicStatus, EvidenceClass, KnowledgeState, Refusal, RefusalCode};
use fmine_registry::metrics::{CreditBasis, LineKind, MetricFamily, MetricRegistry};
use fmine_units::{Currency, ExactQty, Money, UnitsError};

use super::{CostLine, MetricFailure, all_in_cost_per_oz};
use crate::guidance::{Ownership, Period};

type Result<T> = std::result::Result<T, Refusal>;
const ZERO: ExactQty = ExactQty::from_integer(0);
const ONE: ExactQty = ExactQty::from_integer(1);
pub const INPUT_METRIC: &str = "cost.worksheet.input/1";
pub const OUTPUT_METRIC: &str = "cost.worksheet.result/1";
pub const NO_CLAIM: &str = "Exact arithmetic conditional on the declared complete cost population and selected registered definition. No omitted component is independently verified zero; no source audit, account classification, forecast, disclosure eligibility or decision authorization is supplied.";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Scope {
    pub issuer: String,
    pub subject: String,
    pub period: Period,
    pub ownership: Ownership,
    /// Explicit shared accounting/valuation interpretation; no currency or ownership conversion is implicit.
    pub basis: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CostValue {
    Line {
        item: String,
        amount: Money,
    },
    /// Gold ounces sold, not produced ounces or thousands of ounces.
    SoldOunces(ExactQty),
    /// The issuer's separately stated co-product gold allocation; not the ownership share.
    GoldShare(ExactQty),
    Rule {
        definition: String,
        definition_tag: String,
        complete: bool,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CostInput {
    pub scope: Scope,
    pub value: CostValue,
}

fn put_qty(enc: &mut Encoder, value: ExactQty) {
    enc.put_i128(value.num());
    enc.put_i128(value.den());
}

fn put_scope(enc: &mut Encoder, scope: &Scope) {
    enc.put_str(&scope.issuer);
    enc.put_str(&scope.subject);
    enc.put_u16(scope.period.year);
    enc.put_u8(scope.period.first_month);
    enc.put_u8(scope.period.last_month);
    match scope.ownership {
        Ownership::Full => enc.put_u8(0),
        Ownership::Attributable { share } => {
            enc.put_u8(1);
            put_qty(enc, share);
        }
    }
    enc.put_str(&scope.basis);
}

impl CostInput {
    fn key(&self) -> (u8, &str) {
        match &self.value {
            CostValue::Rule { .. } => (0, ""),
            CostValue::SoldOunces(_) => (1, ""),
            CostValue::GoldShare(_) => (2, ""),
            CostValue::Line { item, .. } => (3, item),
        }
    }

    #[must_use]
    pub const fn additivity(&self) -> AdditivityClass {
        match self.value {
            CostValue::Line { .. } | CostValue::SoldOunces(_) => AdditivityClass::ExtensiveAdditive,
            CostValue::GoldShare(_) | CostValue::Rule { .. } => AdditivityClass::NonAdditive,
        }
    }

    #[must_use]
    pub fn canonical_tag(&self) -> String {
        let mut enc = Encoder::new_frame(*b"CSTI", 1);
        put_scope(&mut enc, &self.scope);
        enc.put_u8(self.key().0);
        match &self.value {
            CostValue::Line { item, amount } => {
                enc.put_str(item);
                enc.put_str(&format!("{:?}", amount.ccy));
                enc.put_i128(amount.minor);
            }
            CostValue::SoldOunces(value) | CostValue::GoldShare(value) => put_qty(&mut enc, *value),
            CostValue::Rule {
                definition,
                definition_tag,
                complete,
            } => {
                enc.put_str(definition);
                enc.put_str(definition_tag);
                enc.put_u8(u8::from(*complete));
            }
        }
        blake3_hash(&enc.finish()).to_hex()
    }
}

/// Binds the selected definition, inherited definition metadata, effective lines/exclusions and successor.
/// A definition edited under an unchanged name cannot reuse its old rule Claim.
pub fn definition_tag(registry: &MetricRegistry, id: &str) -> Result<String> {
    let selected = registry.definition(id).ok_or_else(|| {
        bad(
            RefusalCode::NoClaimInDomain201,
            "unregistered cost definition",
        )
    })?;
    if selected.family != MetricFamily::AllInCost || selected.unit != "USD/oz" {
        return Err(bad(
            RefusalCode::BasisMismatch105,
            "definition must calculate USD per gold ounce sold",
        ));
    }
    let mut definitions = BTreeMap::new();
    let mut pending = vec![id];
    while let Some(at) = pending.pop() {
        if definitions.contains_key(at) {
            continue;
        }
        let definition = registry.definition(at).ok_or_else(|| {
            bad(
                RefusalCode::NoClaimInDomain201,
                "missing inherited cost definition",
            )
        })?;
        if registry.definitions.iter().filter(|d| d.id == at).count() != 1 {
            return Err(bad(
                RefusalCode::BasisMismatch105,
                "ambiguous registered definition",
            ));
        }
        pending.extend(definition.extends.iter().map(String::as_str));
        definitions.insert(at, definition);
    }
    let mut enc = Encoder::new_frame(*b"CSTD", 1);
    enc.put_str(id);
    enc.put_u64(definitions.len() as u64);
    for (key, definition) in definitions {
        for text in [
            key,
            &definition.unit,
            &definition.formula,
            &definition.source,
            &definition.evidence,
        ] {
            enc.put_str(text);
        }
        enc.put_u8(match definition.family {
            MetricFamily::AllInCost => 0,
            MetricFamily::Geo => 1,
            MetricFamily::ReserveReplacement => 2,
        });
        enc.put_u8(u8::from(definition.credit_basis == CreditBasis::CoProduct));
        enc.put_u64(definition.extends.len() as u64);
        for parent in &definition.extends {
            enc.put_str(parent);
        }
        enc.put_str(definition.supersedes.as_deref().unwrap_or(""));
    }
    let lines = registry.lines_of(id);
    enc.put_u64(lines.len() as u64);
    for (item, line) in lines {
        enc.put_str(item);
        enc.put_str(&line.letter);
        enc.put_u8(u8::from(line.kind == LineKind::Credit));
    }
    let excluded = registry.excluded_of(id);
    enc.put_u64(excluded.len() as u64);
    for item in excluded {
        enc.put_str(item);
    }
    enc.put_str(registry.superseded_by(id).unwrap_or(""));
    Ok(blake3_hash(&enc.finish()).to_hex())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Treatment {
    Cost,
    Credit,
    Excluded,
}
impl Treatment {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Cost => "cost",
            Self::Credit => "credit",
            Self::Excluded => "excluded",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Contribution {
    pub item: String,
    pub input_claim_id: ObjectId,
    pub amount: Money,
    pub treatment: Treatment,
    /// Signed, allocated major USD contributing to the final numerator; zero for an excluded line.
    pub numerator_usd: ExactQty,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CostSummary {
    pub scope: Scope,
    pub definition: String,
    pub definition_tag: String,
    pub superseded_by: Option<String>,
    pub declared_complete: bool,
    pub numerator_usd: ExactQty,
    pub ounces_sold: ExactQty,
    pub gold_share: Option<ExactQty>,
    pub usd_per_oz: ExactQty,
    pub contributions: Vec<Contribution>,
}

/// Immutable admitted inputs and result. Cloning a returned Claim cannot alter this receipt.
#[derive(Debug, Clone, PartialEq)]
pub struct CostReceipt {
    inputs: Vec<Claim<CostInput>>,
    claim: Claim<CostSummary>,
    digest: String,
}
impl CostReceipt {
    #[must_use]
    pub fn inputs(&self) -> &[Claim<CostInput>] {
        &self.inputs
    }
    #[must_use]
    pub fn summary(&self) -> &CostSummary {
        &self.claim.value
    }
    #[must_use]
    pub fn claim(&self) -> &Claim<CostSummary> {
        &self.claim
    }
    #[must_use]
    pub fn digest(&self) -> &str {
        &self.digest
    }
}

fn bad(code: RefusalCode, text: &str) -> Refusal {
    Refusal::new(code, format!("cost worksheet: {text}"))
}
fn exact<T>(value: std::result::Result<T, UnitsError>) -> Result<T> {
    value.map_err(|_| {
        bad(
            RefusalCode::NumericalCertificateFailed207,
            "exact arithmetic overflow",
        )
    })
}
fn named(text: &str) -> bool {
    !text.is_empty() && text.trim() == text && !text.chars().any(char::is_control)
}

fn admit(claim: &Claim<CostInput>) -> Result<()> {
    claim.check_id()?;
    if claim.metric_key != INPUT_METRIC || claim.value_tag() != claim.value.canonical_tag() {
        return Err(bad(
            RefusalCode::FrozenRecordMutation507,
            "input value differs from its bound identity",
        ));
    }
    if claim.stale || claim.state == KnowledgeState::Stale {
        return Err(bad(RefusalCode::StaleAnchor502, "stale input"));
    }
    if !matches!(
        claim.state,
        KnowledgeState::Known | KnowledgeState::Estimated
    ) || claim.calibration_failed
    {
        return Err(bad(RefusalCode::NotObservable202, "unusable input state"));
    }
    if claim.representation != Representation::Deterministic
        || claim.support != SpatialSupport::PeriodAggregate
    {
        return Err(bad(
            RefusalCode::SupportMismatch102,
            "inputs require explicit deterministic period aggregates",
        ));
    }
    if claim.additivity != claim.value.additivity() {
        return Err(bad(
            RefusalCode::NonAdditiveAverage103,
            "costs and sold ounces must be extensive; rule and allocation are non-additive",
        ));
    }
    if claim.anchors.is_empty()
        || claim
            .anchors
            .iter()
            .any(|a| !named(&a.artifact_path) || !named(&a.locator) || a.artifact_sha256 == [0; 32])
    {
        return Err(bad(
            RefusalCode::NoClaimInDomain201,
            "every input needs source anchors with a digest and locator",
        ));
    }
    if claim.uu_terms.iter().any(|u| {
        !named(&u.scenario_id)
            || !named(&u.description)
            || !named(&u.trigger_indicator)
            || !u.downside_exposure_usd_m.is_finite()
            || u.downside_exposure_usd_m < 0.0
    }) {
        return Err(bad(
            RefusalCode::UnbudgetedUnknowns204,
            "invalid supplied UU metadata",
        ));
    }
    if let Ok(color) = claim.color() {
        validate_color_payload(&color).map_err(|_| {
            bad(
                RefusalCode::CalibrationRequired205,
                "invalid input colour metadata",
            )
        })?;
    }
    Ok(())
}

fn put_envelope(enc: &mut Encoder, claim: &Claim<CostInput>) {
    enc.put_bytes(&claim.canonical_body());
    // These fields are not all present in generic Claim canonical bodies; bind them explicitly.
    enc.put_str(&format!("{:?}", claim.representation));
    enc.put_str(&format!("{:?}", claim.support));
    enc.put_str(&format!("{:?}", claim.additivity));
    enc.put_u8(u8::from(claim.stale));
    enc.put_u8(u8::from(claim.calibration_failed));
    enc.put_str(
        &claim
            .supersedes
            .map_or_else(String::new, |id| id.to_string()),
    );
    enc.put_u64(claim.uu_terms.len() as u64);
    for term in &claim.uu_terms {
        enc.put_str(&term.scenario_id);
        enc.put_str(&term.description);
        enc.put_u64(term.downside_exposure_usd_m.to_bits());
        enc.put_str(&term.trigger_indicator);
    }
    match claim.color() {
        Err(refusal) => {
            enc.put_u8(0);
            enc.put_str(refusal.code);
        }
        Ok(Color::Verified { lo, hi }) => {
            enc.put_u8(1);
            enc.put_u64(lo.to_bits());
            enc.put_u64(hi.to_bits());
        }
        Ok(Color::Validated { regime, dataset }) => {
            enc.put_u8(2);
            enc.put_str(&dataset);
            enc.put_u64(regime.bounds().len() as u64);
            for (axis, (lo, hi)) in regime.bounds() {
                enc.put_str(axis);
                enc.put_u64(lo.to_bits());
                enc.put_u64(hi.to_bits());
            }
        }
        Ok(Color::Estimated {
            estimator,
            dispersion,
        }) => {
            enc.put_u8(3);
            enc.put_str(&estimator);
            enc.put_u64(dispersion.to_bits());
        }
    }
}

/// Computes one explicitly selected registered definition over a declared complete cost population.
/// Signed costs remain signed; credits are nonnegative amounts subtracted exactly once. All admitted
/// lines, including excluded ones, remain parents and affect the receipt identity and weakest evidence.
pub fn evaluate(registry: &MetricRegistry, inputs: &[Claim<CostInput>]) -> Result<CostReceipt> {
    let mut inputs = inputs.to_vec();
    inputs.sort_by(|a, b| a.value.key().cmp(&b.value.key()));
    let first = inputs
        .first()
        .ok_or_else(|| bad(RefusalCode::NoClaimInDomain201, "empty worksheet"))?;
    let scope = &first.value.scope;
    if !named(&scope.issuer)
        || !named(&scope.subject)
        || !named(&scope.basis)
        || scope.period.year == 0
        || scope.period.first_month == 0
        || scope.period.first_month > scope.period.last_month
        || scope.period.last_month > 12
    {
        return Err(bad(
            RefusalCode::UndeclaredUnitBasisOrTz107,
            "a named scope and complete calendar-month support are required",
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
                "inputs differ in issuer, subject, period, ownership or accounting basis",
            ));
        }
    }
    if inputs
        .windows(2)
        .any(|pair| pair[0].value.key() == pair[1].value.key())
    {
        return Err(bad(
            RefusalCode::BasisMismatch105,
            "duplicate rule, sold quantity, allocation or cost item",
        ));
    }
    let CostValue::Rule {
        definition,
        definition_tag: supplied_tag,
        complete: true,
    } = &first.value.value
    else {
        return Err(bad(
            RefusalCode::NoClaimInDomain201,
            "a rule Claim must explicitly declare the supplied cost population complete",
        ));
    };
    let rule_tag = definition_tag(registry, definition)?;
    if *supplied_tag != rule_tag {
        return Err(bad(
            RefusalCode::FrozenRecordMutation507,
            "registered definition differs from the bound rule Claim",
        ));
    }
    let sold = inputs
        .iter()
        .find_map(|c| match c.value.value {
            CostValue::SoldOunces(q) => Some(q),
            _ => None,
        })
        .ok_or_else(|| bad(RefusalCode::NoClaimInDomain201, "sold ounces are required"))?;
    let share = inputs.iter().find_map(|c| match c.value.value {
        CostValue::GoldShare(q) => Some(q),
        _ => None,
    });
    let mut lines = Vec::new();
    for input in &inputs {
        if let CostValue::Line { item, amount } = &input.value.value {
            if !named(item) || amount.ccy != Currency::USD {
                return Err(bad(
                    RefusalCode::BasisMismatch105,
                    "named USD cost items are required; no implicit FX",
                ));
            }
            lines.push(CostLine {
                item: item.clone(),
                usd: exact(ExactQty::new(amount.minor, Money::SCALE))?,
            });
        }
    }
    if lines.is_empty() {
        return Err(bad(
            RefusalCode::NoClaimInDomain201,
            "at least one explicit cost item is required",
        ));
    }
    let calculated =
        all_in_cost_per_oz(registry, definition, &lines, sold, share).map_err(|failure| {
            Refusal::new(
                match failure {
                    MetricFailure::Exact(_) => RefusalCode::NumericalCertificateFailed207,
                    MetricFailure::NonPositiveOunces => RefusalCode::OutsidePhysicalDomain109,
                    _ => RefusalCode::BasisMismatch105,
                },
                failure.to_string(),
            )
        })?;
    let registered = registry.lines_of(definition);
    let mut contributions = Vec::new();
    let mut numerator = ZERO;
    for input in &inputs {
        if let CostValue::Line { item, amount } = &input.value.value {
            let kind = registered.get(item.as_str()).map(|line| line.kind);
            if kind == Some(LineKind::Credit) && amount.minor < 0 {
                return Err(bad(
                    RefusalCode::OutsidePhysicalDomain109,
                    "credits must be nonnegative amounts before subtraction",
                ));
            }
            let treatment = if calculated.left_out.contains(item) {
                Treatment::Excluded
            } else if kind == Some(LineKind::Credit) {
                Treatment::Credit
            } else {
                Treatment::Cost
            };
            let amount_usd = exact(ExactQty::new(amount.minor, Money::SCALE))?;
            let signed = match treatment {
                Treatment::Excluded => ZERO,
                Treatment::Credit => exact(amount_usd.checked_neg())?,
                Treatment::Cost => amount_usd,
            };
            let contribution = exact(signed.checked_mul(share.unwrap_or(ONE)))?;
            numerator = exact(numerator.checked_add(contribution))?;
            contributions.push(Contribution {
                item: item.clone(),
                input_claim_id: input.claim_id,
                amount: *amount,
                treatment,
                numerator_usd: contribution,
            });
        }
    }
    if exact(numerator.checked_div(sold))? != calculated.usd_per_oz {
        return Err(bad(
            RefusalCode::NumericalCertificateFailed207,
            "contributions do not reconcile to the registered calculation",
        ));
    }
    let summary = CostSummary {
        scope: scope.clone(),
        definition: definition.clone(),
        definition_tag: rule_tag,
        superseded_by: calculated.superseded_by,
        declared_complete: true,
        numerator_usd: numerator,
        ounces_sold: sold,
        gold_share: share,
        usd_per_oz: calculated.usd_per_oz,
        contributions,
    };
    let mut enc = Encoder::new_frame(*b"CSTR", 1);
    enc.put_u64(inputs.len() as u64);
    let mut evidence = EvidenceClass::Derived;
    let mut epistemic = EpistemicStatus::KnownKnown;
    let mut state = KnowledgeState::Known;
    let mut confidentiality = Confidentiality::public();
    let mut uu = BTreeMap::new();
    let mut parents = BTreeSet::new();
    let mut anchors = BTreeSet::new();
    for input in &inputs {
        put_envelope(&mut enc, input);
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
    put_qty(&mut enc, summary.numerator_usd);
    put_qty(&mut enc, summary.usd_per_oz);
    enc.put_str(summary.superseded_by.as_deref().unwrap_or(""));
    for contribution in &summary.contributions {
        enc.put_str(&contribution.item);
        enc.put_str(contribution.treatment.as_str());
        put_qty(&mut enc, contribution.numerator_usd);
    }
    let digest = blake3_hash(&enc.finish()).to_hex();
    let claim = Claim::new(
        OUTPUT_METRIC,
        summary,
        &digest,
        Representation::Deterministic,
        SpatialSupport::PeriodAggregate,
        AdditivityClass::IntensiveWeighted,
        evidence,
        epistemic,
        state,
        confidentiality,
        uu.into_values().collect(),
        parents.into_iter().collect(),
        anchors.into_iter().collect(),
    );
    Ok(CostReceipt {
        inputs,
        claim,
        digest,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use fmine_claim::UuTerm;
    use fmine_core::anchor::SourceAnchor;

    fn leaf(value: CostValue) -> Claim<CostInput> {
        let input = CostInput {
            scope: Scope {
                issuer: "invented".into(),
                subject: "Site A".into(),
                period: Period {
                    year: 2030,
                    first_month: 1,
                    last_month: 12,
                },
                ownership: Ownership::Full,
                basis: "nominal USD accrual".into(),
            },
            value,
        };
        let tag = input.canonical_tag();
        let additivity = input.additivity();
        Claim::new(
            INPUT_METRIC,
            input,
            &tag,
            Representation::Deterministic,
            SpatialSupport::PeriodAggregate,
            additivity,
            EvidenceClass::Derived,
            EpistemicStatus::KnownKnown,
            KnowledgeState::Known,
            Confidentiality::internal(),
            vec![],
            vec![],
            vec![SourceAnchor {
                artifact_path: "invented-cost.toml".into(),
                artifact_sha256: [7; 32],
                locator: tag.clone(),
            }],
        )
    }
    fn line(item: &str, minor: i128) -> Claim<CostInput> {
        leaf(CostValue::Line {
            item: item.into(),
            amount: Money::from_minor(Currency::USD, minor),
        })
    }
    fn worksheet(
        id: &str,
        lines: &[(&str, i128)],
        sold: i128,
    ) -> (MetricRegistry, Vec<Claim<CostInput>>) {
        let reg = super::super::bundled_registry().unwrap();
        let mut inputs = vec![
            leaf(CostValue::Rule {
                definition: id.into(),
                definition_tag: definition_tag(&reg, id).unwrap(),
                complete: true,
            }),
            leaf(CostValue::SoldOunces(ExactQty::from_integer(sold))),
        ];
        inputs.extend(lines.iter().map(|(item, minor)| line(item, *minor)));
        (reg, inputs)
    }

    #[test]
    fn aic_exact_money_exposes_growth_credit_and_excluded_contributions() {
        // Invented known answer: (800.01 + 400 - 200) / 100 = 10.0001 USD/oz.
        let (reg, inputs) = worksheet(
            "aic@wgc-2018",
            &[
                ("on_site_mining_costs", 8_000_100),
                ("capital_expenditure_non_sustaining", 4_000_000),
                ("by_product_credits", 2_000_000),
                ("income_tax", 9_990_000),
            ],
            100,
        );
        let receipt = evaluate(&reg, &inputs).unwrap();
        assert_eq!(
            receipt.summary().numerator_usd,
            ExactQty::new(100001, 100).unwrap()
        );
        assert_eq!(
            receipt.summary().usd_per_oz,
            ExactQty::new(100001, 10000).unwrap()
        );
        assert_eq!(receipt.claim().parents.len(), 6);
        assert_eq!(receipt.claim().confidentiality, Confidentiality::internal());
        assert!(receipt.summary().declared_complete);
        let tax = receipt
            .summary()
            .contributions
            .iter()
            .find(|c| c.item == "income_tax")
            .unwrap();
        assert_eq!(
            (tax.treatment, tax.numerator_usd),
            (Treatment::Excluded, ZERO)
        );
        assert_eq!(
            receipt.claim().additivity,
            AdditivityClass::IntensiveWeighted
        );
        receipt.claim().check_id().unwrap();
        let (_, mut altered) = worksheet(
            "aic@wgc-2018",
            &[
                ("on_site_mining_costs", 8_000_000),
                ("by_product_credits", 10_000_000),
            ],
            100,
        );
        assert_eq!(
            evaluate(&reg, &altered).unwrap().summary().usd_per_oz,
            ExactQty::from_integer(-2)
        );
        altered[2] = line("on_site_mining_costs", -1_000_000);
        assert_eq!(
            evaluate(&reg, &altered).unwrap().summary().usd_per_oz,
            ExactQty::from_integer(-11)
        );
    }

    #[test]
    fn registered_versions_and_explicit_co_product_allocation_change_the_positive_result() {
        for (id, expected, successor) in [
            ("aisc@wgc-2013", 8, Some("aisc@wgc-2018")),
            ("aisc@wgc-2018", 9, None),
        ] {
            let (reg, inputs) = worksheet(
                id,
                &[
                    ("on_site_mining_costs", 8_000_000),
                    ("lease_payments", 1_000_000),
                ],
                100,
            );
            let receipt = evaluate(&reg, &inputs).unwrap();
            assert_eq!(
                receipt.summary().usd_per_oz,
                ExactQty::from_integer(expected)
            );
            assert_eq!(receipt.summary().superseded_by.as_deref(), successor);
        }
        let (reg, mut inputs) = worksheet(
            "aisc.co_product@wgc-2018",
            &[
                ("on_site_mining_costs", 10_000_000),
                ("by_product_credits", 3_000_000),
            ],
            100,
        );
        inputs.push(leaf(CostValue::GoldShare(ExactQty::new(3, 4).unwrap())));
        let receipt = evaluate(&reg, &inputs).unwrap();
        assert_eq!(receipt.summary().numerator_usd, ExactQty::from_integer(750));
        assert_eq!(receipt.summary().usd_per_oz, ExactQty::new(15, 2).unwrap());
        assert_eq!(
            receipt.summary().contributions[0].treatment,
            Treatment::Excluded
        );
    }

    #[test]
    fn input_order_is_irrelevant_and_minor_unit_changes_propagate_exactly() {
        let (reg, original) = worksheet(
            "aisc@wgc-2018",
            &[("on_site_mining_costs", 1_000_000), ("income_tax", 123)],
            3,
        );
        let base = evaluate(&reg, &original).unwrap();
        let mut reversed = original.clone();
        reversed.reverse();
        assert_eq!(evaluate(&reg, &reversed).unwrap(), base);
        for increment in [1, 100, 10001] {
            let mut changed = original.clone();
            changed[2] = line("on_site_mining_costs", 1_000_000 + increment);
            let got = evaluate(&reg, &changed).unwrap();
            assert_eq!(
                got.summary()
                    .usd_per_oz
                    .checked_sub(base.summary().usd_per_oz)
                    .unwrap(),
                ExactQty::new(increment, 30000).unwrap()
            );
            assert_ne!(got.claim().claim_id, base.claim().claim_id);
        }
        let mut changed_exclusion = original;
        changed_exclusion[3] = line("income_tax", 124);
        let got = evaluate(&reg, &changed_exclusion).unwrap();
        assert_eq!(got.summary().usd_per_oz, base.summary().usd_per_oz);
        assert_ne!(
            got.digest(),
            base.digest(),
            "excluded evidence remains admitted evidence"
        );
    }

    #[test]
    fn incomplete_duplicate_changed_basis_and_unregistered_inputs_refuse() {
        let (reg, base) = worksheet("aisc@wgc-2018", &[("on_site_mining_costs", 1_000_000)], 100);
        for mutation in [
            "duplicate",
            "incomplete",
            "growth",
            "currency",
            "zero_sold",
            "extra_share",
            "negative_credit",
            "raw_value",
            "stale",
            "missing_anchor",
            "nan_uu",
        ] {
            let mut inputs = base.clone();
            let expected = match mutation {
                "duplicate" => {
                    inputs.push(inputs[2].clone());
                    "FMINE-REF-105"
                }
                "incomplete" => {
                    inputs[0] = leaf(CostValue::Rule {
                        definition: "aisc@wgc-2018".into(),
                        definition_tag: definition_tag(&reg, "aisc@wgc-2018").unwrap(),
                        complete: false,
                    });
                    "FMINE-REF-201"
                }
                "growth" => {
                    inputs.push(line("capital_expenditure_non_sustaining", 100));
                    "FMINE-REF-105"
                }
                "currency" => {
                    inputs[2] = leaf(CostValue::Line {
                        item: "on_site_mining_costs".into(),
                        amount: Money::from_minor(Currency::CAD, 100),
                    });
                    "FMINE-REF-105"
                }
                "zero_sold" => {
                    inputs[1] = leaf(CostValue::SoldOunces(ZERO));
                    "FMINE-REF-109"
                }
                "extra_share" => {
                    inputs.push(leaf(CostValue::GoldShare(ONE)));
                    "FMINE-REF-105"
                }
                "negative_credit" => {
                    inputs.push(line("by_product_credits", -1));
                    "FMINE-REF-109"
                }
                "raw_value" => {
                    inputs[2].value.value = CostValue::Line {
                        item: "on_site_mining_costs".into(),
                        amount: Money::from_minor(Currency::USD, 5),
                    };
                    "FMINE-REF-507"
                }
                "stale" => {
                    inputs[2].stale = true;
                    "FMINE-REF-502"
                }
                "missing_anchor" => {
                    inputs[2].anchors.clear();
                    "FMINE-REF-507"
                }
                _ => {
                    inputs[2].uu_terms.push(unknown(f64::NAN));
                    "FMINE-REF-204"
                }
            };
            assert_eq!(
                evaluate(&reg, &inputs).unwrap_err().code,
                expected,
                "{mutation}"
            );
        }
        let mut altered_rules = reg.clone();
        altered_rules.definitions[0].source.push_str(" changed");
        assert_eq!(
            evaluate(&altered_rules, &base).unwrap_err().code,
            "FMINE-REF-507"
        );
    }

    fn unknown(exposure: f64) -> UuTerm {
        UuTerm {
            scenario_id: "cost-omission".into(),
            description: "Unmapped account".into(),
            downside_exposure_usd_m: exposure,
            trigger_indicator: "Ledger reconciliation".into(),
        }
    }

    #[test]
    fn excluded_evidence_and_unhashed_claim_metadata_remain_bound_and_non_laundered() {
        let (reg, mut inputs) = worksheet(
            "aisc@wgc-2018",
            &[("on_site_mining_costs", 1_000_000), ("income_tax", 100)],
            100,
        );
        let base = evaluate(&reg, &inputs).unwrap();
        inputs[3] = inputs[3].downgraded(EvidenceClass::Assumed);
        inputs[3].uu_terms.push(unknown(1.0));
        let assumed = evaluate(&reg, &inputs).unwrap();
        assert_eq!(assumed.claim().evidence, EvidenceClass::Assumed);
        assert_eq!(assumed.claim().uu_terms, vec![unknown(1.0)]);
        assert_eq!(assumed.summary().usd_per_oz, base.summary().usd_per_oz);
        let same_input_id = inputs[3].claim_id;
        inputs[3].uu_terms[0].downside_exposure_usd_m = 1.0f64.next_up();
        assert_eq!(same_input_id, inputs[3].claim_id);
        let changed = evaluate(&reg, &inputs).unwrap();
        assert_ne!(changed.claim().claim_id, assumed.claim().claim_id);
        assert_ne!(changed.digest(), assumed.digest());
        inputs[2].uu_terms.push(unknown(2.0));
        assert_eq!(evaluate(&reg, &inputs).unwrap_err().code, "FMINE-REF-204");
    }
}
