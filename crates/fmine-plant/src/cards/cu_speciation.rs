//! Determinism: BitwisePortable
//! Speciation-weighted copper heap/dump recovery transfer card (`BD-182`, plan §17.3,
//! `FACT E289–E290` Zaldívar `F1/F2/F3 = 96/78/40`, 55% insoluble-Cu cap, `G-CRUC-1`, `G-CRUC-2`).
//!
//! Computes `TCu_rec = F1 · (ASCu/TCu) + F2 · (CNCu/TCu) + F3_eff · (SCu/TCu)` in exact rational
//! `ExactQty` arithmetic (zero bare `f64` parameters; L-6), enforcing processing-support validity
//! (`G-CRUC-2`) and explicit validity-box bounds on residence time, head grade, and speciation
//! ratios (`G-CRUC-1`, `FMINE-REF-203`).

use fmine_claim::{SpatialSupport, UuTerm};
use fmine_evidence::{KnowledgeState, Refusal, RefusalCode};
use fmine_units::{ExactQty, UnitsError};

/// Closed exact validity interval `[min, max]` for one card coordinate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExactValidityRange {
    pub min: ExactQty,
    pub max: ExactQty,
}

impl ExactValidityRange {
    /// Construct a validated `[min, max]` range (`min <= max`).
    pub fn new(min: ExactQty, max: ExactQty) -> Result<Self, Refusal> {
        let ord = min.checked_cmp(max).map_err(map_err)?;
        if ord.is_gt() {
            return Err(Refusal::new(
                RefusalCode::OutsidePhysicalDomain109,
                "validity range min exceeds max",
            ));
        }
        Ok(Self { min, max })
    }

    /// Returns `true` iff `min <= x <= max`.
    pub fn contains(&self, x: ExactQty) -> Result<bool, Refusal> {
        let ge_min = x.checked_cmp(self.min).map_err(map_err)?.is_ge();
        let le_max = x.checked_cmp(self.max).map_err(map_err)?.is_le();
        Ok(ge_min && le_max)
    }
}

/// Speciation-weighted Cu heap/dump transfer card definition (plan §17.3, `E289–E290`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CuSpeciationCard {
    pub card_id: String,
    pub version: String,
    pub domain_id: String,
    /// Recovery factor `F1` on acid-soluble Cu (`ASCu/TCu`), fraction in `[0, 1]` (Zaldívar: `96/100`).
    pub f1_acid_soluble: ExactQty,
    /// Recovery factor `F2` on cyanide-soluble Cu (`CNCu/TCu`), fraction in `[0, 1]` (Zaldívar: `78/100`).
    pub f2_cyanide_soluble: ExactQty,
    /// Raw recovery factor `F3` on primary/insoluble sulphide Cu (`SCu/TCu`), fraction in `[0, 1]`
    /// (Zaldívar: `40/100`).
    pub f3_sulphide: ExactQty,
    /// Optional explicit upper cap on insoluble/sulphide Cu recovery (Zaldívar `E290`: `55/100`).
    pub insoluble_cu_cap: Option<ExactQty>,
    /// Tested residence time validity box (days).
    pub residence_time_days_box: ExactValidityRange,
    /// Tested total copper grade validity box (% Cu).
    pub total_cu_pct_box: ExactValidityRange,
    /// Tested `ASCu/TCu` ratio validity box.
    pub as_cu_ratio_box: ExactValidityRange,
}

/// Processing-support feed blend inputs to [`CuSpeciationCard`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CuSpeciationInput {
    pub support: SpatialSupport,
    /// Acid-soluble copper share `ASCu / TCu` in `[0, 1]`.
    pub as_cu_ratio: ExactQty,
    /// Cyanide-soluble copper share `CNCu / TCu` in `[0, 1]`.
    pub cn_cu_ratio: ExactQty,
    /// Primary/insoluble sulphide copper share `SCu / TCu` in `[0, 1]`.
    pub s_cu_ratio: ExactQty,
    /// Total copper head grade (% Cu, strictly positive).
    pub total_cu_pct: ExactQty,
    /// Planned heap/dump leach residence time (days, strictly positive).
    pub residence_time_days: ExactQty,
}

/// Evaluation verdict from [`evaluate_cu_speciation_card`].
#[derive(Debug, Clone, PartialEq)]
pub struct CuSpeciationVerdict {
    pub card_id: String,
    pub version: String,
    pub domain_id: String,
    pub effective_f3_sulphide: ExactQty,
    pub insoluble_cap_applied: bool,
    pub tcu_recovery_fraction: ExactQty,
    pub extrapolated_outside_validity: bool,
    pub knowledge_state: KnowledgeState,
    pub extrapolation_uu: Option<UuTerm>,
}

fn map_err(e: UnitsError) -> Refusal {
    Refusal::new(
        RefusalCode::OutsidePhysicalDomain109,
        format!("exact Cu speciation arithmetic error: {e}"),
    )
}

fn check_unit_fraction(name: &str, v: ExactQty) -> Result<(), Refusal> {
    let zero = ExactQty::from_integer(0);
    let one = ExactQty::from_integer(1);
    if v.checked_cmp(zero).map_err(map_err)?.is_lt() || v.checked_cmp(one).map_err(map_err)?.is_gt()
    {
        return Err(Refusal::new(
            RefusalCode::OutsidePhysicalDomain109,
            format!("Cu speciation parameter '{name}' outside [0, 1]"),
        ));
    }
    Ok(())
}

impl CuSpeciationCard {
    /// Validate card factors and optional insoluble-Cu cap.
    pub fn validate(&self) -> Result<(), Refusal> {
        if self.card_id.trim().is_empty()
            || self.version.trim().is_empty()
            || self.domain_id.trim().is_empty()
        {
            return Err(Refusal::new(
                RefusalCode::UndeclaredUnitBasisOrTz107,
                "CuSpeciationCard requires non-empty card_id, version, and domain_id",
            ));
        }
        check_unit_fraction("f1_acid_soluble", self.f1_acid_soluble)?;
        check_unit_fraction("f2_cyanide_soluble", self.f2_cyanide_soluble)?;
        check_unit_fraction("f3_sulphide", self.f3_sulphide)?;
        if let Some(cap) = self.insoluble_cu_cap {
            check_unit_fraction("insoluble_cu_cap", cap)?;
        }
        let _ = ExactValidityRange::new(
            self.residence_time_days_box.min,
            self.residence_time_days_box.max,
        )?;
        let _ = ExactValidityRange::new(self.total_cu_pct_box.min, self.total_cu_pct_box.max)?;
        let _ = ExactValidityRange::new(self.as_cu_ratio_box.min, self.as_cu_ratio_box.max)?;
        Ok(())
    }

    /// Reference Zaldívar speciation card (`FACT E289`: `F1/F2/F3 = 96/78/40`, `FACT E290`: `55%`
    /// insoluble-Cu cap) with explicit validity box.
    pub fn zaldivar_reference(
        residence_time_days_box: ExactValidityRange,
        total_cu_pct_box: ExactValidityRange,
        as_cu_ratio_box: ExactValidityRange,
    ) -> Result<Self, Refusal> {
        let card = Self {
            card_id: "cu_speciation.zaldivar_heap".to_string(),
            version: "v1-E289-E290".to_string(),
            domain_id: "zaldivar_mixed_cu".to_string(),
            f1_acid_soluble: ExactQty::new(96, 100).map_err(map_err)?,
            f2_cyanide_soluble: ExactQty::new(78, 100).map_err(map_err)?,
            f3_sulphide: ExactQty::new(40, 100).map_err(map_err)?,
            insoluble_cu_cap: Some(ExactQty::new(55, 100).map_err(map_err)?),
            residence_time_days_box,
            total_cu_pct_box,
            as_cu_ratio_box,
        };
        card.validate()?;
        Ok(card)
    }
}

/// Evaluate a [`CuSpeciationCard`] inside its validity box (`G-CRUC-1`, `G-CRUC-2`).
///
/// Refuses:
/// - `FMINE-REF-102` (`G-CRUC-2`) if `input.support` is not processing/period support;
/// - `FMINE-REF-109` if speciation shares fall outside `[0, 1]` or do not sum to `1` (`ASCu + CNCu + SCu != 1`);
/// - `FMINE-REF-203` (`G-CRUC-1`) if residence time, head grade, or `ASCu/TCu` lies outside the card's validity box.
pub fn evaluate_cu_speciation_card(
    card: &CuSpeciationCard,
    input: &CuSpeciationInput,
) -> Result<CuSpeciationVerdict, Refusal> {
    evaluate_cu_speciation_inner(card, input, None)
}

/// Explicit `G-CRUC-1` extrapolation mode: allows evaluating outside the validity box ONLY when the
/// caller attaches a valid priced [`UuTerm`] (`FMINE-REF-204` if empty/non-finite/negative), marking
/// the output `extrapolated_outside_validity = true` with `KnowledgeState::Estimated`.
pub fn evaluate_cu_speciation_with_extrapolation(
    card: &CuSpeciationCard,
    input: &CuSpeciationInput,
    extrapolation_uu: UuTerm,
) -> Result<CuSpeciationVerdict, Refusal> {
    if extrapolation_uu.scenario_id.trim().is_empty()
        || extrapolation_uu.description.trim().is_empty()
        || extrapolation_uu.trigger_indicator.trim().is_empty()
        || !extrapolation_uu.downside_exposure_usd_m.is_finite()
        || extrapolation_uu.downside_exposure_usd_m < 0.0
    {
        return Err(Refusal::new(
            RefusalCode::UnbudgetedUnknowns204,
            "G-CRUC-1 extrapolation requires a complete, finite, non-negative priced UuTerm",
        ));
    }
    evaluate_cu_speciation_inner(card, input, Some(extrapolation_uu))
}

fn evaluate_cu_speciation_inner(
    card: &CuSpeciationCard,
    input: &CuSpeciationInput,
    extrapolation_uu: Option<UuTerm>,
) -> Result<CuSpeciationVerdict, Refusal> {
    card.validate()?;

    // G-CRUC-2: recovery cards apply at processing/period support, never at block or core support.
    if !matches!(
        input.support,
        SpatialSupport::PeriodAggregate | SpatialSupport::PlantLot
    ) {
        return Err(Refusal::new(
            RefusalCode::SupportMismatch102,
            format!(
                "G-CRUC-2: Cu speciation recovery card cannot be applied at support {:?}; \
                 aggregate to processing support first",
                input.support
            ),
        ));
    }

    check_unit_fraction("as_cu_ratio", input.as_cu_ratio)?;
    check_unit_fraction("cn_cu_ratio", input.cn_cu_ratio)?;
    check_unit_fraction("s_cu_ratio", input.s_cu_ratio)?;
    if input.total_cu_pct.signum() <= 0 || input.residence_time_days.signum() <= 0 {
        return Err(Refusal::new(
            RefusalCode::OutsidePhysicalDomain109,
            "total_cu_pct and residence_time_days must be strictly positive",
        ));
    }

    let one = ExactQty::from_integer(1);
    let ratio_sum = input
        .as_cu_ratio
        .checked_add(input.cn_cu_ratio)
        .map_err(map_err)?
        .checked_add(input.s_cu_ratio)
        .map_err(map_err)?;
    if ratio_sum.checked_cmp(one).map_err(map_err)?.is_ne() {
        return Err(Refusal::new(
            RefusalCode::OutsidePhysicalDomain109,
            format!(
                "Cu speciation partition ASCu/TCu + CNCu/TCu + SCu/TCu must equal 1, got {}/{}",
                ratio_sum.num(),
                ratio_sum.den()
            ),
        ));
    }

    let mut violations = Vec::new();
    if !card
        .residence_time_days_box
        .contains(input.residence_time_days)?
    {
        violations.push("residence_time_days");
    }
    if !card.total_cu_pct_box.contains(input.total_cu_pct)? {
        violations.push("total_cu_pct");
    }
    if !card.as_cu_ratio_box.contains(input.as_cu_ratio)? {
        violations.push("as_cu_ratio");
    }

    let extrapolated_outside_validity = !violations.is_empty();
    if extrapolated_outside_validity && extrapolation_uu.is_none() {
        return Err(Refusal::new(
            RefusalCode::OutsideAoa203,
            format!(
                "G-CRUC-1: Cu speciation card '{}@{}' evaluated outside validity box on [{}]",
                card.card_id,
                card.version,
                violations.join(", ")
            ),
        ));
    }

    let (effective_f3, insoluble_cap_applied) = match card.insoluble_cu_cap {
        Some(cap) if card.f3_sulphide.checked_cmp(cap).map_err(map_err)?.is_gt() => (cap, true),
        _ => (card.f3_sulphide, false),
    };

    let t1 = card
        .f1_acid_soluble
        .checked_mul(input.as_cu_ratio)
        .map_err(map_err)?;
    let t2 = card
        .f2_cyanide_soluble
        .checked_mul(input.cn_cu_ratio)
        .map_err(map_err)?;
    let t3 = effective_f3
        .checked_mul(input.s_cu_ratio)
        .map_err(map_err)?;
    let tcu_recovery_fraction = t1
        .checked_add(t2)
        .map_err(map_err)?
        .checked_add(t3)
        .map_err(map_err)?;

    Ok(CuSpeciationVerdict {
        card_id: card.card_id.clone(),
        version: card.version.clone(),
        domain_id: card.domain_id.clone(),
        effective_f3_sulphide: effective_f3,
        insoluble_cap_applied,
        tcu_recovery_fraction,
        extrapolated_outside_validity,
        knowledge_state: KnowledgeState::Estimated,
        extrapolation_uu: if extrapolated_outside_validity {
            extrapolation_uu
        } else {
            None
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn q(n: i128, d: i128) -> ExactQty {
        ExactQty::new(n, d).unwrap()
    }

    fn zaldivar_card() -> CuSpeciationCard {
        CuSpeciationCard::zaldivar_reference(
            ExactValidityRange::new(q(180, 1), q(540, 1)).unwrap(),
            ExactValidityRange::new(q(25, 100), q(120, 100)).unwrap(),
            ExactValidityRange::new(q(10, 100), q(80, 100)).unwrap(),
        )
        .unwrap()
    }

    #[test]
    fn test_zaldivar_cu_speciation_transfer_card_known_answers_and_insoluble_cap() {
        let card = zaldivar_card();

        // Blend: 50% ASCu + 30% CNCu + 20% SCu -> 0.5*0.96 + 0.3*0.78 + 0.2*0.40 = 0.48 + 0.234 + 0.08 = 0.794.
        let verdict = evaluate_cu_speciation_card(
            &card,
            &CuSpeciationInput {
                support: SpatialSupport::PeriodAggregate,
                as_cu_ratio: q(50, 100),
                cn_cu_ratio: q(30, 100),
                s_cu_ratio: q(20, 100),
                total_cu_pct: q(55, 100),
                residence_time_days: q(360, 1),
            },
        )
        .unwrap();
        assert_eq!(verdict.tcu_recovery_fraction, q(794, 1000));
        assert!(!verdict.insoluble_cap_applied);
        assert!(!verdict.extrapolated_outside_validity);
        assert_eq!(verdict.knowledge_state, KnowledgeState::Estimated);

        // E290 insoluble-Cu 55% cap: if raw testwork predicts F3 = 64% on 100% SCu-heavy blend (10% AS, 0% CN, 90% S),
        // cap trims F3 from 64% -> 55%, reducing recovery by 0.90 * 9% = 8.1 percentage points!
        let mut capped_card = card;
        capped_card.f3_sulphide = q(64, 100);
        let capped_v = evaluate_cu_speciation_card(
            &capped_card,
            &CuSpeciationInput {
                support: SpatialSupport::PeriodAggregate,
                as_cu_ratio: q(10, 100),
                cn_cu_ratio: q(0, 100),
                s_cu_ratio: q(90, 100),
                total_cu_pct: q(50, 100),
                residence_time_days: q(400, 1),
            },
        )
        .unwrap();
        assert!(capped_v.insoluble_cap_applied);
        assert_eq!(capped_v.effective_f3_sulphide, q(55, 100));
        // 0.10 * 0.96 + 0.90 * 0.55 = 0.096 + 0.495 = 0.591 (vs uncapped 0.672, delta = -8.1%).
        assert_eq!(capped_v.tcu_recovery_fraction, q(591, 1000));
    }

    #[test]
    fn test_cu_speciation_card_validity_box_and_support_refusals() {
        let card = zaldivar_card();
        let valid_in = CuSpeciationInput {
            support: SpatialSupport::PeriodAggregate,
            as_cu_ratio: q(50, 100),
            cn_cu_ratio: q(30, 100),
            s_cu_ratio: q(20, 100),
            total_cu_pct: q(55, 100),
            residence_time_days: q(360, 1),
        };

        // 1. Block support refused under G-CRUC-2 (FMINE-REF-102).
        let mut block_in = valid_in.clone();
        block_in.support = SpatialSupport::SmuBlock {
            dx_cm: 1000,
            dy_cm: 1000,
            dz_cm: 1000,
        };
        assert_eq!(
            evaluate_cu_speciation_card(&card, &block_in)
                .unwrap_err()
                .code,
            RefusalCode::SupportMismatch102.spec().code
        );

        // 2. Residence time outside [180, 540] days refused under G-CRUC-1 (FMINE-REF-203).
        let mut short_res = valid_in.clone();
        short_res.residence_time_days = q(120, 1);
        assert_eq!(
            evaluate_cu_speciation_card(&card, &short_res)
                .unwrap_err()
                .code,
            RefusalCode::OutsideAoa203.spec().code
        );

        // 3. Explicit G-CRUC-1 extrapolation succeeds ONLY with priced UU and marks Estimated + extrapolated.
        let extrap = evaluate_cu_speciation_with_extrapolation(
            &card,
            &short_res,
            UuTerm {
                scenario_id: "uu-heap-short-residence".to_string(),
                description: "residence time < 180 d cuts heap recovery below break-even"
                    .to_string(),
                downside_exposure_usd_m: 12.5,
                trigger_indicator: "leach column recovery < 70% at 120 d".to_string(),
            },
        )
        .unwrap();
        assert!(extrap.extrapolated_outside_validity);
        assert_eq!(extrap.knowledge_state, KnowledgeState::Estimated);
        assert!(extrap.extrapolation_uu.is_some());

        // Invalid/unpriced UU refused with FMINE-REF-204.
        assert_eq!(
            evaluate_cu_speciation_with_extrapolation(
                &card,
                &short_res,
                UuTerm {
                    scenario_id: "".to_string(),
                    description: "bad".to_string(),
                    downside_exposure_usd_m: -1.0,
                    trigger_indicator: "none".to_string(),
                },
            )
            .unwrap_err()
            .code,
            RefusalCode::UnbudgetedUnknowns204.spec().code
        );

        // 4. Speciation shares not summing to 1 refused with FMINE-REF-109.
        let mut bad_sum = valid_in;
        bad_sum.s_cu_ratio = q(30, 100); // sum = 1.10
        assert_eq!(
            evaluate_cu_speciation_card(&card, &bad_sum)
                .unwrap_err()
                .code,
            RefusalCode::OutsidePhysicalDomain109.spec().code
        );
    }
}
