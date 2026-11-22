//! Determinism: BitwisePortable
//! Preg-robbing card expressed as a gold-adsorption spike-test percentage with source-specific
//! feed caps (`< 40%` and `< 60%`) and optional Raman `carbon_type` covariate (`BD-288`, plan
//! §17.3, `FACT E173`, `FACT E175`, `G-CRUC-1`).
//!
//! All physical quantities use exact `fmine_units::ExactQty` (zero bare `f64` parameters; L-6).

use fmine_evidence::{Refusal, RefusalCode};
use fmine_units::{ExactQty, UnitsError};

use crate::cards::InputUnit;

/// Raman-confirmed organic carbon reactivity covariate (`FACT E175`: the same organic carbon mass
/// percent yields `0–40%` or `75–100%` preg-robbing depending on carbon structure/source).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CarbonType {
    /// Uncharacterized organic carbon (conservative cap applies).
    Unspecified,
    /// Low-activity carbon confirmed by Raman spectroscopy (0–40% spike-test regime).
    RamanLowActivity,
    /// High-activity graphitic/amorphous preg-robbing carbon confirmed by Raman (75–100% regime;
    /// requires roasting, POX, or blanking agent rather than direct CIL).
    RamanHighActivity,
}

impl CarbonType {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Unspecified => "unspecified",
            Self::RamanLowActivity => "raman_low_activity",
            Self::RamanHighActivity => "raman_high_activity",
        }
    }
}

/// Source-specific preg-robbing validity profile and spike-test feed cap (`E173`, `E175`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PregRobCard {
    pub card_id: String,
    pub version: String,
    pub source_name: String,
    /// Strict upper feed cap on gold-adsorption spike-test percentage (e.g. `< 40%` or `< 60%`).
    pub max_spike_test_pct: ExactQty,
    /// Optional Raman `carbon_type` covariate restricting which carbon species this card covers.
    pub carbon_type: Option<CarbonType>,
    /// Baseline CIL recovery fraction at 0% preg-rob spike test (in `(0, 1]`).
    pub base_cil_recovery_fraction: ExactQty,
    /// Linear recovery loss per 1% spike-test preg-rob (fraction per percentage point, `>= 0`).
    pub penalty_fraction_per_spike_pct: ExactQty,
    /// Optional annual feed tonnage cap for refractory/carbonaceous sources (e.g. Sage mill `< 120,000 t/y`).
    pub max_annual_feed_tonnes: Option<ExactQty>,
    /// Evidence register citations (e.g. `E173`, `E175`).
    pub source_e_ids: Vec<String>,
}

/// Feed blend observation evaluated against a [`PregRobCard`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PregRobInput {
    /// Must be [`InputUnit::SpikeTestPercent`]; dry-mass-% substitution is refused (`FMINE-REF-107`).
    pub unit: InputUnit,
    /// Gold-adsorption spike-test percentage in `[0, 100]`.
    pub spike_test_pct: ExactQty,
    /// Optional Raman-identified carbon species for the feed blend.
    pub carbon_type: Option<CarbonType>,
    /// Optional planned annual feed tonnes from this source.
    pub annual_feed_tonnes: Option<ExactQty>,
}

/// Result of evaluating a valid feed blend against [`PregRobCard`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PregRobVerdict {
    pub card_id: String,
    pub source_name: String,
    pub spike_test_pct: ExactQty,
    pub max_spike_test_pct: ExactQty,
    pub headroom_pct: ExactQty,
    pub carbon_type: Option<CarbonType>,
    pub recovery_fraction: ExactQty,
}

fn map_err(e: UnitsError) -> Refusal {
    Refusal::new(
        RefusalCode::OutsidePhysicalDomain109,
        format!("exact preg-rob arithmetic error: {e}"),
    )
}

fn fmt_pct(q: ExactQty) -> String {
    if q.den() == 1 {
        format!("{}%", q.num())
    } else if let Some(scaled) = q.num().checked_mul(100) {
        let hundredths = scaled / q.den();
        let whole = hundredths / 100;
        let frac = (hundredths % 100).abs();
        format!("{whole}.{frac:02}%")
    } else {
        format!("{}/{}%", q.num(), q.den())
    }
}

impl PregRobCard {
    /// Validate card bounds and parameters.
    pub fn validate(&self) -> Result<(), Refusal> {
        if self.card_id.trim().is_empty()
            || self.version.trim().is_empty()
            || self.source_name.trim().is_empty()
        {
            return Err(Refusal::new(
                RefusalCode::UndeclaredUnitBasisOrTz107,
                "PregRobCard requires non-empty card_id, version, and source_name",
            ));
        }
        let zero = ExactQty::from_integer(0);
        let one = ExactQty::from_integer(1);
        let hundred = ExactQty::from_integer(100);
        if self
            .max_spike_test_pct
            .checked_cmp(zero)
            .map_err(map_err)?
            .is_le()
            || self
                .max_spike_test_pct
                .checked_cmp(hundred)
                .map_err(map_err)?
                .is_gt()
        {
            return Err(Refusal::new(
                RefusalCode::OutsidePhysicalDomain109,
                "max_spike_test_pct must lie in (0, 100]",
            ));
        }
        if self
            .base_cil_recovery_fraction
            .checked_cmp(zero)
            .map_err(map_err)?
            .is_le()
            || self
                .base_cil_recovery_fraction
                .checked_cmp(one)
                .map_err(map_err)?
                .is_gt()
        {
            return Err(Refusal::new(
                RefusalCode::OutsidePhysicalDomain109,
                "base_cil_recovery_fraction must lie in (0, 1]",
            ));
        }
        if self
            .penalty_fraction_per_spike_pct
            .checked_cmp(zero)
            .map_err(map_err)?
            .is_lt()
        {
            return Err(Refusal::new(
                RefusalCode::OutsidePhysicalDomain109,
                "penalty_fraction_per_spike_pct must be >= 0",
            ));
        }
        if let Some(cap_t) = self.max_annual_feed_tonnes
            && cap_t.signum() <= 0
        {
            return Err(Refusal::new(
                RefusalCode::OutsidePhysicalDomain109,
                "max_annual_feed_tonnes must be strictly positive",
            ));
        }
        Ok(())
    }

    /// Canonical NGM `< 40%` spike-test feed cap profile (`FACT E173`, `E175`).
    pub fn ngm_40_pct_cap(source_name: impl Into<String>) -> Result<Self, Refusal> {
        let card = Self {
            card_id: "preg_rob.ngm_cap_40".to_string(),
            version: "v1-E173-E175".to_string(),
            source_name: source_name.into(),
            max_spike_test_pct: ExactQty::from_integer(40),
            carbon_type: Some(CarbonType::RamanLowActivity),
            base_cil_recovery_fraction: ExactQty::new(90, 100).map_err(map_err)?,
            penalty_fraction_per_spike_pct: ExactQty::new(3, 1000).map_err(map_err)?,
            max_annual_feed_tonnes: Some(ExactQty::from_integer(120_000)),
            source_e_ids: vec!["E173".to_string(), "E175".to_string()],
        };
        card.validate()?;
        Ok(card)
    }

    /// Canonical NGM `< 60%` spike-test feed cap profile (`FACT E173`, `E175`).
    pub fn ngm_60_pct_cap(source_name: impl Into<String>) -> Result<Self, Refusal> {
        let card = Self {
            card_id: "preg_rob.ngm_cap_60".to_string(),
            version: "v1-E173-E175".to_string(),
            source_name: source_name.into(),
            max_spike_test_pct: ExactQty::from_integer(60),
            carbon_type: Some(CarbonType::RamanLowActivity),
            base_cil_recovery_fraction: ExactQty::new(88, 100).map_err(map_err)?,
            penalty_fraction_per_spike_pct: ExactQty::new(4, 1000).map_err(map_err)?,
            max_annual_feed_tonnes: None,
            source_e_ids: vec!["E173".to_string(), "E175".to_string()],
        };
        card.validate()?;
        Ok(card)
    }
}

/// Evaluate a feed blend against [`PregRobCard`], refusing wrong unit (`FMINE-REF-107`),
/// spike-test `>= max_spike_test_pct` (`FMINE-REF-203`), mismatched Raman `carbon_type`
/// (`FMINE-REF-203`), or excess annual feed tonnage (`FMINE-REF-203`).
pub fn evaluate_preg_rob_card(
    card: &PregRobCard,
    input: &PregRobInput,
) -> Result<PregRobVerdict, Refusal> {
    card.validate()?;

    if input.unit != InputUnit::SpikeTestPercent {
        return Err(Refusal::new(
            RefusalCode::UndeclaredUnitBasisOrTz107,
            format!(
                "preg-rob card requires InputUnit::SpikeTestPercent, got {:?}; \
                 never substitute dry-mass TOC% for a gold-adsorption spike test",
                input.unit
            ),
        ));
    }

    let zero = ExactQty::from_integer(0);
    let hundred = ExactQty::from_integer(100);
    if input
        .spike_test_pct
        .checked_cmp(zero)
        .map_err(map_err)?
        .is_lt()
        || input
            .spike_test_pct
            .checked_cmp(hundred)
            .map_err(map_err)?
            .is_gt()
    {
        return Err(Refusal::new(
            RefusalCode::OutsidePhysicalDomain109,
            "spike_test_pct must lie in [0, 100]",
        ));
    }

    if let Some(required_ct) = card.carbon_type {
        match input.carbon_type {
            Some(actual_ct) if actual_ct == required_ct => {}
            Some(actual_ct) => {
                return Err(Refusal::new(
                    RefusalCode::OutsideAoa203,
                    format!(
                        "G-CRUC-1: card '{}' calibrated for carbon_type '{}', refused '{}'; \
                         high-activity carbonaceous ore requires roasting/POX/blanking route",
                        card.card_id,
                        required_ct.label(),
                        actual_ct.label()
                    ),
                ));
            }
            None => {
                return Err(Refusal::new(
                    RefusalCode::OutsideAoa203,
                    format!(
                        "G-CRUC-1: card '{}' requires Raman carbon_type '{}' (E175)",
                        card.card_id,
                        required_ct.label()
                    ),
                ));
            }
        }
    }

    // Strict `<` feed cap (e.g. `< 40%` or `< 60%`).
    if input
        .spike_test_pct
        .checked_cmp(card.max_spike_test_pct)
        .map_err(map_err)?
        .is_ge()
    {
        return Err(Refusal::new(
            RefusalCode::OutsideAoa203,
            format!(
                "G-CRUC-1: source '{}' spike-test preg-rob {} violates feed cap < {} (E173/E175)",
                card.source_name,
                fmt_pct(input.spike_test_pct),
                fmt_pct(card.max_spike_test_pct)
            ),
        ));
    }

    if let Some(cap_t) = card.max_annual_feed_tonnes {
        let planned_t = input.annual_feed_tonnes.ok_or_else(|| {
            Refusal::new(
                RefusalCode::OutsideAoa203,
                format!(
                    "G-CRUC-1: source '{}' requires annual_feed_tonnes to verify cap < {}/{} t/y",
                    card.source_name,
                    cap_t.num(),
                    cap_t.den()
                ),
            )
        })?;
        if planned_t.signum() < 0 {
            return Err(Refusal::new(
                RefusalCode::OutsidePhysicalDomain109,
                "annual_feed_tonnes must be >= 0",
            ));
        }
        if planned_t.checked_cmp(cap_t).map_err(map_err)?.is_ge() {
            return Err(Refusal::new(
                RefusalCode::OutsideAoa203,
                format!(
                    "G-CRUC-1: source '{}' planned feed {}/{} t/y violates annual cap < {}/{} t/y",
                    card.source_name,
                    planned_t.num(),
                    planned_t.den(),
                    cap_t.num(),
                    cap_t.den()
                ),
            ));
        }
    } else if let Some(planned_t) = input.annual_feed_tonnes
        && planned_t.signum() < 0
    {
        return Err(Refusal::new(
            RefusalCode::OutsidePhysicalDomain109,
            "annual_feed_tonnes must be >= 0",
        ));
    }

    let penalty = card
        .penalty_fraction_per_spike_pct
        .checked_mul(input.spike_test_pct)
        .map_err(map_err)?;
    let recovery_fraction = card
        .base_cil_recovery_fraction
        .checked_sub(penalty)
        .map_err(map_err)?;
    if recovery_fraction.signum() < 0 {
        return Err(Refusal::new(
            RefusalCode::OutsidePhysicalDomain109,
            "preg-rob penalty reduced recovery below 0; refuse rather than clamp",
        ));
    }
    let headroom_pct = card
        .max_spike_test_pct
        .checked_sub(input.spike_test_pct)
        .map_err(map_err)?;

    Ok(PregRobVerdict {
        card_id: card.card_id.clone(),
        source_name: card.source_name.clone(),
        spike_test_pct: input.spike_test_pct,
        max_spike_test_pct: card.max_spike_test_pct,
        headroom_pct,
        carbon_type: input.carbon_type,
        recovery_fraction,
    })
}

fn sanitize_md_cell(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '|' => out.push_str("\\|"),
            '\r' | '\n' => out.push(' '),
            c => out.push(c),
        }
    }
    out
}

/// Deterministic human-readable validity summary rendering source-specific `< 40%` / `< 60%`
/// spike-test feed caps and optional Raman `carbon_type` covariates (`BD-288`).
#[must_use]
pub fn render_preg_rob_validity_table(cards: &[PregRobCard]) -> String {
    let mut out = String::from(
        "| Card | Source | Spike-Test Feed Cap | Carbon Type | Annual Feed Cap | Evidence |\n\
         |---|---|---|---|---|---|\n",
    );
    for c in cards {
        let ct = c.carbon_type.map_or("any", CarbonType::label);
        let tpy = match c.max_annual_feed_tonnes {
            Some(t) if t.den() == 1 => format!("< {} t/y", t.num()),
            Some(t) => format!("< {}/{} t/y", t.num(), t.den()),
            None => "none".to_string(),
        };
        let eids = if c.source_e_ids.is_empty() {
            "-".to_string()
        } else {
            sanitize_md_cell(&c.source_e_ids.join(", "))
        };
        out.push_str(&format!(
            "| {}@{} | {} | < {} | {} | {} | {} |\n",
            sanitize_md_cell(&c.card_id),
            sanitize_md_cell(&c.version),
            sanitize_md_cell(&c.source_name),
            fmt_pct(c.max_spike_test_pct),
            ct,
            tpy,
            eids
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_preg_rob_card_40_and_60_caps_carbon_type_and_validity_render() {
        let cap40 = PregRobCard::ngm_40_pct_cap("ngm_source_a_sage_feed").unwrap();
        let cap60 = PregRobCard::ngm_60_pct_cap("ngm_source_b_cil_blend").unwrap();

        // Validity table explicitly renders both 40% and 60% spike-test feed caps and carbon_type.
        let rendered = render_preg_rob_validity_table(&[cap40.clone(), cap60.clone()]);
        assert!(
            rendered.contains("< 40%"),
            "must render 40% cap: {rendered}"
        );
        assert!(
            rendered.contains("< 60%"),
            "must render 60% cap: {rendered}"
        );
        assert!(rendered.contains("raman_low_activity"));
        assert!(rendered.contains("E173, E175"));
        assert!(rendered.contains("< 120000 t/y"));

        // Inside 40% cap (35%): 0.90 - 35 * 0.003 = 0.795, headroom = 5%.
        let ok40 = evaluate_preg_rob_card(
            &cap40,
            &PregRobInput {
                unit: InputUnit::SpikeTestPercent,
                spike_test_pct: ExactQty::from_integer(35),
                carbon_type: Some(CarbonType::RamanLowActivity),
                annual_feed_tonnes: Some(ExactQty::from_integer(100_000)),
            },
        )
        .unwrap();
        assert_eq!(ok40.recovery_fraction, ExactQty::new(795, 1000).unwrap());
        assert_eq!(ok40.headroom_pct, ExactQty::from_integer(5));

        // Exact boundary 40% refuses with FMINE-REF-203 (strict `< 40%`).
        assert_eq!(
            evaluate_preg_rob_card(
                &cap40,
                &PregRobInput {
                    unit: InputUnit::SpikeTestPercent,
                    spike_test_pct: ExactQty::from_integer(40),
                    carbon_type: Some(CarbonType::RamanLowActivity),
                    annual_feed_tonnes: Some(ExactQty::from_integer(100_000)),
                },
            )
            .unwrap_err()
            .code,
            RefusalCode::OutsideAoa203.spec().code
        );

        // Inside 60% cap (55%) passes; exact boundary 60% refuses with FMINE-REF-203.
        let ok60 = evaluate_preg_rob_card(
            &cap60,
            &PregRobInput {
                unit: InputUnit::SpikeTestPercent,
                spike_test_pct: ExactQty::from_integer(55),
                carbon_type: Some(CarbonType::RamanLowActivity),
                annual_feed_tonnes: None,
            },
        )
        .unwrap();
        // 0.88 - 55 * 0.004 = 0.88 - 0.22 = 0.66.
        assert_eq!(ok60.recovery_fraction, ExactQty::new(66, 100).unwrap());
        assert_eq!(
            evaluate_preg_rob_card(
                &cap60,
                &PregRobInput {
                    unit: InputUnit::SpikeTestPercent,
                    spike_test_pct: ExactQty::from_integer(60),
                    carbon_type: Some(CarbonType::RamanLowActivity),
                    annual_feed_tonnes: None,
                },
            )
            .unwrap_err()
            .code,
            RefusalCode::OutsideAoa203.spec().code
        );

        // High-activity Raman carbon refuses on low-activity card (E175) with FMINE-REF-203.
        assert_eq!(
            evaluate_preg_rob_card(
                &cap40,
                &PregRobInput {
                    unit: InputUnit::SpikeTestPercent,
                    spike_test_pct: ExactQty::from_integer(20),
                    carbon_type: Some(CarbonType::RamanHighActivity),
                    annual_feed_tonnes: Some(ExactQty::from_integer(100_000)),
                },
            )
            .unwrap_err()
            .code,
            RefusalCode::OutsideAoa203.spec().code
        );

        // Fresh-eyes regression: omitting annual_feed_tonnes cannot bypass an active annual cap!
        assert_eq!(
            evaluate_preg_rob_card(
                &cap40,
                &PregRobInput {
                    unit: InputUnit::SpikeTestPercent,
                    spike_test_pct: ExactQty::from_integer(20),
                    carbon_type: Some(CarbonType::RamanLowActivity),
                    annual_feed_tonnes: None,
                },
            )
            .unwrap_err()
            .code,
            RefusalCode::OutsideAoa203.spec().code
        );

        // Fresh-eyes regression: Markdown table escapes pipes and newlines in untrusted labels.
        let mut tricky = cap60;
        tricky.source_name = "bad|source\ninjected".to_string();
        let md = render_preg_rob_validity_table(&[tricky]);
        assert!(md.contains("bad\\|source injected"));
        assert_eq!(md.lines().count(), 3);

        // Dry-mass percent unit substitution refused with FMINE-REF-107.
        assert_eq!(
            evaluate_preg_rob_card(
                &cap40,
                &PregRobInput {
                    unit: InputUnit::DryMassPercent,
                    spike_test_pct: ExactQty::from_integer(20),
                    carbon_type: Some(CarbonType::RamanLowActivity),
                    annual_feed_tonnes: Some(ExactQty::from_integer(100_000)),
                },
            )
            .unwrap_err()
            .code,
            RefusalCode::UndeclaredUnitBasisOrTz107.spec().code
        );
    }
}
