//! Determinism: BitwisePortable
//! Five-class exploration absence semantics and cover-vs-DOI detection thinning (`BD-231`,
//! plan §11.3–§11.4).
//!
//! Cover enters detection `q_j(cover, DOI)` and NEVER existence `λ`: soil geochemistry over
//! 60 m of transported cover classifies as `A1LookedInsensitive` (`NotObservable`), never as
//! evidence that covered ground is barren. Unrecorded historical effort classifies as
//! `A4EffortSuspectedUnrecorded` (`Unknown`) carrying explicit Manski bounds `[0, π_era_max]`.

use std::collections::BTreeMap;

use fmine_core::id::ObjectId;
use fmine_evidence::KnowledgeState;
use fmine_units::ExactQty;

use crate::ExploreError;
use crate::model::{CoverageConfidence, DepositRecord, EffortRecord, GeoCoord};

/// All five §11.3 exploration absence classes (extending R7 §1.7 with `A4`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AbsenceClass {
    /// **A0 Not looked:** no `EffortRecord` intersects location `s`. Enters likelihood as nothing.
    A0NotLooked,
    /// **A1 Looked, insensitive:** intersecting effort has `q_j(cover, DOI) <= q_min` (e.g. soil
    /// geochemistry over deep transported cover). Logged, enters likelihood as ~0.
    A1LookedInsensitive { max_q: ExactQty },
    /// **A2 Looked, sensitive, nothing:** intersecting effort has cumulative hazard `H > 0`.
    /// Enters likelihood as `-∫λπ` penalty scaled by `q`.
    A2LookedSensitiveNothing { cumulative_hazard_h: ExactQty },
    /// **A3 Found sub-economic:** known occurrence with a censored mark (`0.3 g/t over 20 m`).
    A3FoundSubEconomic {
        deposit_id: ObjectId,
        censored_grade_g_per_t: ExactQty,
    },
    /// **A4 Effort suspected, unrecorded:** `coverage ∈ {Low, Unknown}` (e.g. 1987 report citing
    /// RAB drilling without collars). Enters as Manski bounds `π ∈ [0, manski_pi_max]`.
    A4EffortSuspectedUnrecorded { manski_pi_max: ExactQty },
}

impl AbsenceClass {
    /// Exact §11.3 table projection onto [`fmine_evidence::KnowledgeState`].
    #[must_use]
    pub const fn knowledge_state(&self) -> KnowledgeState {
        match self {
            Self::A0NotLooked | Self::A1LookedInsensitive { .. } => KnowledgeState::NotObservable,
            Self::A2LookedSensitiveNothing { .. } => KnowledgeState::Estimated,
            Self::A3FoundSubEconomic { .. } => KnowledgeState::Known,
            Self::A4EffortSuspectedUnrecorded { .. } => KnowledgeState::Unknown,
        }
    }
}

/// Complete absence assessment for a query location `s` (`BD-231`, plan §11.3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AbsenceAssessment {
    pub class: AbsenceClass,
    pub knowledge_state: KnowledgeState,
    pub intersecting_effort_count: usize,
    /// Explicit Manski detection probability bounds `[pi_lo, pi_hi]`.
    /// Tight (`[0, 0]`) for `A0`/`A1`, positive lower bound for `A2`, wide `[0, π_era_max]` for `A4`.
    pub manski_pi_bounds: (ExactQty, ExactQty),
}

/// Exact piecewise-linear cover attenuation `q_j(cover, DOI)`:
/// - If `cover_depth_m >= doi_m`: returns `0` (method cannot see through cover thicker than its DOI).
/// - Otherwise: returns `baseline_q * (1 - cover_depth_m / doi_m)`.
///
/// Cover strictly attenuates detection `q_j`, never underlying mineral existence `λ` (§11.4).
pub fn effective_detection_q(
    baseline_q: ExactQty,
    cover_depth_m: ExactQty,
    doi_m: ExactQty,
) -> Result<ExactQty, ExploreError> {
    let zero = ExactQty::from_integer(0);
    let one = ExactQty::from_integer(1);
    if baseline_q.signum() < 0
        || baseline_q.checked_cmp(one)? == core::cmp::Ordering::Greater
        || cover_depth_m.signum() < 0
        || doi_m.signum() <= 0
    {
        return Err(ExploreError::InvalidRecord {
            field: "detection_q",
            detail: "baseline_q in [0, 1], cover_depth_m >= 0, doi_m > 0 required".to_string(),
        });
    }
    if cover_depth_m.checked_cmp(doi_m)? != core::cmp::Ordering::Less {
        return Ok(zero);
    }
    let ratio = cover_depth_m.checked_div(doi_m)?;
    let atten = one.checked_sub(ratio)?;
    Ok(baseline_q.checked_mul(atten)?)
}

/// Classify absence at query location `site` against `deposits` and `efforts` (`BD-231`, §11.3).
///
/// - Enforces `G-CRS-1` (`FMINE-REF-106`) if any deposit or effort carries a different `CrsId`.
/// - `occurrence_match_radius_m`: radius within which a sub-economic occurrence matches `site`.
/// - `q_min`: minimum effective detection probability separating `A1` from `A2`.
/// - `manski_era_pi_max`: upper detection-probability bound `π_era-typical ∈ (0, 1]` for `A4`.
pub fn classify_site_absence(
    site: &GeoCoord,
    deposits: &[DepositRecord],
    efforts: &[EffortRecord],
    occurrence_match_radius_m: ExactQty,
    q_min: ExactQty,
    manski_era_pi_max: ExactQty,
) -> Result<AbsenceAssessment, ExploreError> {
    let zero = ExactQty::from_integer(0);
    let one = ExactQty::from_integer(1);
    if site.cover_depth_m.signum() < 0
        || occurrence_match_radius_m.signum() < 0
        || q_min.signum() < 0
        || q_min.checked_cmp(one)? == core::cmp::Ordering::Greater
        || manski_era_pi_max.signum() <= 0
        || manski_era_pi_max.checked_cmp(one)? == core::cmp::Ordering::Greater
    {
        return Err(ExploreError::InvalidRecord {
            field: "classify_site_absence",
            detail:
                "cover >= 0, radius >= 0, q_min in [0, 1], manski_era_pi_max in (0, 1] required"
                    .to_string(),
        });
    }
    // Pre-pass: enforce G-CRS-1 (FMINE-REF-106) and positive MethodSpec bounds across all inputs
    // deterministically before any early A3 return.
    for dep in deposits {
        if dep.coord.crs != site.crs {
            return Err(ExploreError::CrsMismatch {
                left: site.crs,
                right: dep.coord.crs,
            });
        }
    }
    // A reimport is the same survey. Resolve identities before any classification, including A3.
    let mut surveys = BTreeMap::new();
    for eff in efforts {
        if eff.center.crs != site.crs {
            return Err(ExploreError::CrsMismatch {
                left: site.crs,
                right: eff.center.crs,
            });
        }
        if let Some(previous) = surveys.insert(eff.id, eff) {
            if previous != eff {
                return Err(ExploreError::ConflictingEffort(eff.id));
            }
            continue;
        }
        let _ = crate::model::MethodSpec::new(
            eff.spec.coverage_radius_m,
            eff.spec.spacing_m,
            eff.spec.depth_of_investigation_m,
            eff.spec.baseline_q,
            eff.spec.effort_intensity,
        )?;
    }

    let occ_r_sq = occurrence_match_radius_m.checked_mul(occurrence_match_radius_m)?;
    for dep in deposits {
        let d_sq = site.planar_dist_sq(&dep.coord)?;
        if d_sq.checked_cmp(occ_r_sq)? != core::cmp::Ordering::Greater
            && dep.marks.censored_sub_economic
        {
            let class = AbsenceClass::A3FoundSubEconomic {
                deposit_id: dep.id,
                censored_grade_g_per_t: dep.marks.grade_g_per_t,
            };
            let ks = class.knowledge_state();
            return Ok(AbsenceAssessment {
                class,
                knowledge_state: ks,
                intersecting_effort_count: 0,
                manski_pi_bounds: (one, one),
            });
        }
    }

    let mut intersecting = 0usize;
    let mut saw_unrecorded = false;
    let mut max_q = zero;
    let mut cumulative_h = zero;

    for eff in surveys.values() {
        let d_sq = site.planar_dist_sq(&eff.center)?;
        let r_sq = eff
            .spec
            .coverage_radius_m
            .checked_mul(eff.spec.coverage_radius_m)?;
        if d_sq.checked_cmp(r_sq)? != core::cmp::Ordering::Greater {
            intersecting += 1;
            if matches!(
                eff.coverage,
                CoverageConfidence::Low | CoverageConfidence::Unknown
            ) {
                saw_unrecorded = true;
            }
            let q = effective_detection_q(
                eff.spec.baseline_q,
                site.cover_depth_m,
                eff.spec.depth_of_investigation_m,
            )?;
            if q.checked_cmp(max_q)? == core::cmp::Ordering::Greater {
                max_q = q;
            }
            if q.checked_cmp(q_min)? == core::cmp::Ordering::Greater {
                let h_j = eff.spec.effort_intensity.checked_mul(q)?;
                cumulative_h = cumulative_h.checked_add(h_j)?;
            }
        }
    }

    if intersecting == 0 {
        let class = AbsenceClass::A0NotLooked;
        return Ok(AbsenceAssessment {
            knowledge_state: class.knowledge_state(),
            class,
            intersecting_effort_count: 0,
            manski_pi_bounds: (zero, zero),
        });
    }
    if saw_unrecorded {
        let class = AbsenceClass::A4EffortSuspectedUnrecorded {
            manski_pi_max: manski_era_pi_max,
        };
        return Ok(AbsenceAssessment {
            knowledge_state: class.knowledge_state(),
            class,
            intersecting_effort_count: intersecting,
            manski_pi_bounds: (zero, manski_era_pi_max),
        });
    }
    if cumulative_h.signum() <= 0 {
        let class = AbsenceClass::A1LookedInsensitive { max_q };
        return Ok(AbsenceAssessment {
            knowledge_state: class.knowledge_state(),
            class,
            intersecting_effort_count: intersecting,
            manski_pi_bounds: (zero, max_q),
        });
    }

    // Rational conservative lower bound on `1 - exp(-H)`: `H / (1 + H) <= 1 - exp(-H)` for all H > 0.
    let pi_lb = cumulative_h.checked_div(one.checked_add(cumulative_h)?)?;
    let class = AbsenceClass::A2LookedSensitiveNothing {
        cumulative_hazard_h: cumulative_h,
    };
    Ok(AbsenceAssessment {
        knowledge_state: class.knowledge_state(),
        class,
        intersecting_effort_count: intersecting,
        manski_pi_bounds: (pi_lb, one),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    use fmine_core::anchor::SourceAnchor;
    use fmine_core::hash::sha256;
    use fmine_core::id::PrefixKind;
    use fmine_crs::CrsId;

    use crate::model::{DepositTypeProbs, ExploreMethod, MarkRecord, MethodSpec};

    fn coord(e: i128, n: i128, cover_m: i128) -> GeoCoord {
        GeoCoord::new(
            CrsId::NGM_UTM11N,
            ExactQty::from_integer(e),
            ExactQty::from_integer(n),
            ExactQty::from_integer(1_500),
            ExactQty::from_integer(cover_m),
        )
        .unwrap()
    }

    fn anchor(tag: &str) -> SourceAnchor {
        SourceAnchor::new(
            format!("openfile/{tag}.pdf"),
            sha256(tag.as_bytes()).0,
            "p.1",
        )
    }

    #[test]
    fn repeated_effort_does_not_strengthen_absence_evidence() {
        let site = coord(500_000, 4_500_000, 0);
        let one = ExactQty::from_integer(1);
        let half = ExactQty::new(1, 2).unwrap();
        let survey = |tag: &str| {
            EffortRecord::build(
                ObjectId::from_canonical_bytes(PrefixKind::Claim, tag.as_bytes()),
                ExploreMethod::AircoreDrilling,
                site.clone(),
                2024,
                2024,
                MethodSpec::new(one, one, one, half, one).unwrap(),
                anchor(tag),
                CoverageConfidence::High,
            )
            .unwrap()
        };
        let assess = |records: &[EffortRecord]| {
            classify_site_absence(&site, &[], records, one, ExactQty::from_integer(0), one)
        };
        let first = survey("survey-one");
        let single = assess(std::slice::from_ref(&first)).unwrap();
        assert_eq!(single.manski_pi_bounds.0, ExactQty::new(1, 3).unwrap());
        let reimport = assess(&[first.clone(), first.clone()]).unwrap();
        assert_eq!(reimport, single);

        let second = survey("survey-two");
        let independent = assess(&[first.clone(), second.clone()]).unwrap();
        assert_eq!(independent.intersecting_effort_count, 2);
        assert_eq!(independent.manski_pi_bounds.0, half);
        assert_eq!(
            assess(&[second, first.clone(), first.clone()]).unwrap(),
            independent
        );

        let mut conflict = first.clone();
        conflict.spec.effort_intensity = ExactQty::from_integer(2);
        for records in [[first.clone(), conflict.clone()], [conflict, first]] {
            assert_eq!(
                assess(&records).unwrap_err().to_refusal().code,
                "FMINE-REF-507"
            );
        }
    }

    #[test]
    fn test_all_five_absence_classes_and_cover_thinning_law() {
        let q_min = ExactQty::new(5, 100).unwrap(); // 5% minimum sensitivity
        let manski_max = ExactQty::new(60, 100).unwrap(); // 60% era-typical max detection
        let occ_r = ExactQty::from_integer(100);

        // 1. A0 Not looked: unmapped tenement under 60m cover with zero intersecting effort -> NotObservable.
        let site_deep_cover = coord(500_000, 4_500_000, 60);
        let a0 = classify_site_absence(&site_deep_cover, &[], &[], occ_r, q_min, manski_max)
            .expect("A0");
        assert_eq!(a0.class, AbsenceClass::A0NotLooked);
        assert_eq!(a0.knowledge_state, KnowledgeState::NotObservable);

        // 2. A1 Looked, insensitive: soil geochemistry (DOI = 5 m) over 60 m transported cover!
        //    Cover attenuates detection q -> 0 (never teaches model that covered ground is barren).
        let soil_eff = EffortRecord::build(
            ObjectId::from_canonical_bytes(PrefixKind::Claim, b"eff-soil"),
            ExploreMethod::SoilGeochem,
            coord(500_000, 4_500_000, 60),
            2020,
            2020,
            MethodSpec::new(
                ExactQty::from_integer(1_000),
                ExactQty::from_integer(100),
                ExactQty::from_integer(5), // DOI = 5 m << 60 m cover
                ExactQty::new(80, 100).unwrap(),
                ExactQty::from_integer(1),
            )
            .unwrap(),
            anchor("soil-2020"),
            CoverageConfidence::High,
        )
        .unwrap();
        let a1 =
            classify_site_absence(&site_deep_cover, &[], &[soil_eff], occ_r, q_min, manski_max)
                .expect("A1");
        assert_eq!(
            a1.class,
            AbsenceClass::A1LookedInsensitive {
                max_q: ExactQty::from_integer(0)
            }
        );
        assert_eq!(a1.knowledge_state, KnowledgeState::NotObservable);

        // 3. A2 Looked, sensitive, nothing: close-spaced aircore (DOI = 100 m) through 20 m cover!
        //    q = 0.80 * (1 - 20/100) = 0.64; intensity = 2 -> H = 1.28 > 0 -> Estimated.
        let site_thin_cover = coord(500_000, 4_500_000, 20);
        let aircore_eff = EffortRecord::build(
            ObjectId::from_canonical_bytes(PrefixKind::Claim, b"eff-ac"),
            ExploreMethod::AircoreDrilling,
            coord(500_000, 4_500_000, 20),
            2024,
            2024,
            MethodSpec::new(
                ExactQty::from_integer(500),
                ExactQty::from_integer(50),
                ExactQty::from_integer(100),
                ExactQty::new(80, 100).unwrap(),
                ExactQty::from_integer(2),
            )
            .unwrap(),
            anchor("aircore-2024"),
            CoverageConfidence::High,
        )
        .unwrap();
        let a2 = classify_site_absence(
            &site_thin_cover,
            &[],
            &[aircore_eff],
            occ_r,
            q_min,
            manski_max,
        )
        .expect("A2");
        assert_eq!(
            a2.class,
            AbsenceClass::A2LookedSensitiveNothing {
                cumulative_hazard_h: ExactQty::new(128, 100).unwrap()
            }
        );
        assert_eq!(a2.knowledge_state, KnowledgeState::Estimated);
        assert!(a2.manski_pi_bounds.0.signum() > 0);

        // 4. A3 Found sub-economic: 0.3 g/t Au intercept with censored mark -> Known.
        let mut tmap = BTreeMap::new();
        tmap.insert("carlin_au".to_string(), ExactQty::new(9, 10).unwrap());
        let sub_occ = DepositRecord::build(
            ObjectId::from_canonical_bytes(PrefixKind::Claim, b"occ-sub"),
            "SubEcon-1",
            coord(500_020, 4_500_010, 20),
            DepositTypeProbs::new(tmap, ExactQty::new(1, 10).unwrap()).unwrap(),
            MarkRecord::new(
                ExactQty::from_integer(1),
                ExactQty::new(3, 10).unwrap(), // 0.3 g/t
                ExactQty::from_integer(40),
                "disseminated oxide",
                true,
            )
            .unwrap(),
            None,
            "USGS-2km-v1",
            vec![anchor("occ-sub")],
        )
        .unwrap();
        let a3 = classify_site_absence(&site_thin_cover, &[sub_occ], &[], occ_r, q_min, manski_max)
            .expect("A3");
        assert!(matches!(
            a3.class,
            AbsenceClass::A3FoundSubEconomic {
                censored_grade_g_per_t,
                ..
            } if censored_grade_g_per_t == ExactQty::new(3, 10).unwrap()
        ));
        assert_eq!(a3.knowledge_state, KnowledgeState::Known);

        // 5. A4 Effort suspected, unrecorded: 1987 report mentions extensive RAB drilling without collars -> Unknown + Manski bounds!
        let rab_1987 = EffortRecord::build(
            ObjectId::from_canonical_bytes(PrefixKind::Claim, b"eff-rab-1987"),
            ExploreMethod::RabDrilling,
            coord(500_000, 4_500_000, 20),
            1987,
            1987,
            MethodSpec::new(
                ExactQty::from_integer(2_000),
                ExactQty::from_integer(200),
                ExactQty::from_integer(50),
                ExactQty::new(50, 100).unwrap(),
                ExactQty::from_integer(1),
            )
            .unwrap(),
            anchor("rab-1987-report"),
            CoverageConfidence::Unknown,
        )
        .unwrap();
        let a4 =
            classify_site_absence(&site_thin_cover, &[], &[rab_1987], occ_r, q_min, manski_max)
                .expect("A4");
        assert_eq!(
            a4.class,
            AbsenceClass::A4EffortSuspectedUnrecorded {
                manski_pi_max: manski_max
            }
        );
        assert_eq!(a4.knowledge_state, KnowledgeState::Unknown);
        assert_eq!(a4.manski_pi_bounds, (ExactQty::from_integer(0), manski_max));
    }
}
