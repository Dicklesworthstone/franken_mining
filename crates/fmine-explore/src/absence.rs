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
    /// Applies to the supplied survey effort. A catalogue occurrence does not establish survey sensitivity.
    pub manski_pi_bounds: (ExactQty, ExactQty),
}

/// A catalogue presence is a positive observation outside the five absence classes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SiteAssessment {
    KnownOccurrences {
        /// Matching uncensored catalogue identities in canonical order.
        deposit_ids: Vec<ObjectId>,
        intersecting_effort_count: usize,
    },
    Absence(AbsenceAssessment),
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
    match assess_site(
        site,
        deposits,
        efforts,
        occurrence_match_radius_m,
        q_min,
        manski_era_pi_max,
    )? {
        SiteAssessment::Absence(result) => Ok(result),
        SiteAssessment::KnownOccurrences { deposit_ids, .. } => {
            Err(ExploreError::KnownOccurrences(deposit_ids))
        }
    }
}

/// Match catalogue observations before interpreting a lack of detection (§11.3).
pub fn assess_site(
    site: &GeoCoord,
    deposits: &[DepositRecord],
    efforts: &[EffortRecord],
    occurrence_match_radius_m: ExactQty,
    q_min: ExactQty,
    manski_era_pi_max: ExactQty,
) -> Result<SiteAssessment, ExploreError> {
    GeoCoord::require_metric_crs(site.crs)?;
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
    let mut catalogue = BTreeMap::new();
    for dep in deposits {
        if dep.coord.crs != site.crs {
            return Err(ExploreError::CrsMismatch {
                left: site.crs,
                right: dep.coord.crs,
            });
        }
        if let Some(previous) = catalogue.insert(dep.id, dep)
            && previous != dep
        {
            return Err(ExploreError::ConflictingDeposit(dep.id));
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
    let mut positives = Vec::new();
    let mut nearest: Option<(ExactQty, &DepositRecord)> = None;
    for dep in catalogue.into_values() {
        let d_sq = site.planar_dist_sq(&dep.coord)?;
        if d_sq.checked_cmp(occ_r_sq)?.is_gt() {
            continue;
        }
        if !dep.marks.censored_sub_economic {
            positives.push(dep.id);
        } else if match nearest {
            None => true,
            Some((best, _)) => d_sq.checked_cmp(best)?.is_lt(),
        } {
            // Equal distances retain the first (lowest canonical ID).
            nearest = Some((d_sq, dep));
        }
    }

    let mut intersecting = 0usize;
    let mut saw_unrecorded = false;
    let mut max_q = zero;
    let mut cumulative_h = zero;
    let mut recorded_h = zero;

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
                continue;
            }
            if !positives.is_empty() {
                continue;
            }
            let q = effective_detection_q(
                eff.spec.baseline_q,
                site.cover_depth_m,
                eff.spec.depth_of_investigation_m,
            )?;
            if q.checked_cmp(max_q)? == core::cmp::Ordering::Greater {
                max_q = q;
            }
            let h_j = eff.spec.effort_intensity.checked_mul(q)?;
            recorded_h = recorded_h.checked_add(h_j)?;
            if q.checked_cmp(q_min)?.is_gt() {
                cumulative_h = cumulative_h.checked_add(h_j)?;
            }
        }
    }

    if !positives.is_empty() {
        return Ok(SiteAssessment::KnownOccurrences {
            deposit_ids: positives,
            intersecting_effort_count: intersecting,
        });
    }
    // H/(1+H) <= 1-exp(-H) <= min(1,H). Weak looks contribute only to the upper
    // bound. The union bound admits additional historical effort without assuming independence.
    let pi_lb = cumulative_h.checked_div(one.checked_add(cumulative_h)?)?;
    let unrecorded_upper = if saw_unrecorded {
        manski_era_pi_max
    } else {
        zero
    };
    let pi_ub = if recorded_h
        .checked_cmp(one.checked_sub(unrecorded_upper)?)?
        .is_lt()
    {
        recorded_h.checked_add(unrecorded_upper)?
    } else {
        one
    };
    let class = if let Some((_, dep)) = nearest {
        AbsenceClass::A3FoundSubEconomic {
            deposit_id: dep.id,
            censored_grade_g_per_t: dep.marks.grade_g_per_t,
        }
    } else if intersecting == 0 {
        AbsenceClass::A0NotLooked
    } else if saw_unrecorded {
        AbsenceClass::A4EffortSuspectedUnrecorded {
            manski_pi_max: manski_era_pi_max,
        }
    } else if cumulative_h.signum() == 0 {
        AbsenceClass::A1LookedInsensitive { max_q }
    } else {
        AbsenceClass::A2LookedSensitiveNothing {
            cumulative_hazard_h: cumulative_h,
        }
    };
    Ok(SiteAssessment::Absence(AbsenceAssessment {
        knowledge_state: class.knowledge_state(),
        class,
        intersecting_effort_count: intersecting,
        manski_pi_bounds: (pi_lb, pi_ub),
    }))
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
                site,
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
    fn angular_coordinates_and_invalid_zones_cannot_match_a_metre_radius() {
        let base = coord(1, 2, 0);
        let zero = ExactQty::from_integer(0);
        let one = ExactQty::from_integer(1);
        for crs in [
            CrsId::Epsg4326,
            CrsId::Wgs84Utm {
                zone: 0,
                north: true,
            },
            CrsId::Wgs84Utm {
                zone: 61,
                north: false,
            },
        ] {
            assert_eq!(
                GeoCoord::new(crs, one, one, zero, zero)
                    .unwrap_err()
                    .to_refusal()
                    .code,
                "FMINE-REF-106"
            );
            let invalid = GeoCoord { crs, ..base };
            assert_eq!(
                invalid
                    .planar_dist_sq(&invalid)
                    .unwrap_err()
                    .to_refusal()
                    .code,
                "FMINE-REF-106"
            );
            assert_eq!(
                assess_site(&invalid, &[], &[], one, zero, one)
                    .unwrap_err()
                    .to_refusal()
                    .code,
                "FMINE-REF-106"
            );
        }
        for crs in [
            CrsId::Wgs84Utm {
                zone: 1,
                north: true,
            },
            CrsId::Wgs84Utm {
                zone: 60,
                north: false,
            },
            CrsId::MineGrid { grid_code: 1 },
        ] {
            let point = GeoCoord::new(crs, one, one, zero, zero).unwrap();
            assert_eq!(point.planar_dist_sq(&point).unwrap(), zero);
        }
    }

    #[test]
    fn observed_occurrences_preclude_false_absences_and_survive_reordering() {
        let site = coord(0, 0, 0);
        let one = ExactQty::from_integer(1);
        let occurrence = |tag: &str, e, censored| {
            DepositRecord::build(
                ObjectId::from_canonical_bytes(PrefixKind::Claim, tag.as_bytes()),
                tag,
                coord(e, 0, 0),
                DepositTypeProbs::new(
                    BTreeMap::from([("vein".into(), one)]),
                    ExactQty::from_integer(0),
                )
                .unwrap(),
                MarkRecord::new(one, one, one, "vein", censored).unwrap(),
                None,
                "one-collar",
                vec![anchor(tag)],
            )
            .unwrap()
        };
        let survey = EffortRecord::build(
            ObjectId::from_canonical_bytes(PrefixKind::Claim, b"survey"),
            ExploreMethod::AircoreDrilling,
            site,
            2024,
            2024,
            MethodSpec::new(one, one, one, one, one).unwrap(),
            anchor("survey"),
            CoverageConfidence::High,
        )
        .unwrap();
        let surveys = [survey.clone(), survey];
        let zero = ExactQty::from_integer(0);
        let radius = ExactQty::from_integer(2);
        let assess = |deps: &[DepositRecord]| assess_site(&site, deps, &surveys, radius, zero, one);
        let near = occurrence("near", 1, true);
        let tie = occurrence("tie", -1, true);
        let far = occurrence("far", 2, true);
        let expected = near.id.min(tie.id);
        for deps in [
            [far.clone(), near.clone(), tie.clone()],
            [tie, near.clone(), far.clone()],
        ] {
            let SiteAssessment::Absence(a) = assess(&deps).unwrap() else {
                panic!("expected a censored occurrence")
            };
            assert!(
                matches!(a.class, AbsenceClass::A3FoundSubEconomic { deposit_id, .. } if deposit_id == expected)
            );
            assert_eq!(a.intersecting_effort_count, 1);
            assert_eq!(a.manski_pi_bounds, (ExactQty::new(1, 2).unwrap(), one));
        }
        let found = occurrence("found", 2, false);
        let outside = occurrence("outside", 3, false);
        let another = occurrence("another", 0, false);
        let mut ids = vec![found.id, another.id];
        ids.sort();
        let records = vec![outside, near.clone(), found.clone(), another, found];
        let mut reversed = records.clone();
        reversed.reverse();
        for deps in [&records, &reversed] {
            assert_eq!(
                assess(deps).unwrap(),
                SiteAssessment::KnownOccurrences {
                    deposit_ids: ids.clone(),
                    intersecting_effort_count: 1,
                }
            );
            let err = classify_site_absence(&site, deps, &[], radius, zero, one).unwrap_err();
            assert_eq!(err.to_refusal().code, "FMINE-REF-203");
            assert_eq!(err, ExploreError::KnownOccurrences(ids.clone()));
        }
        let mut conflicting = near.clone();
        conflicting.marks.censored_sub_economic = false;
        assert_eq!(
            assess(&[near, conflicting]).unwrap_err().to_refusal().code,
            "FMINE-REF-507"
        );
    }

    #[test]
    fn weak_recorded_effort_and_unrecorded_effort_enclose_detection() {
        let site = coord(0, 0, 0);
        let one = ExactQty::from_integer(1);
        let zero = ExactQty::from_integer(0);
        let q = |n, d| ExactQty::new(n, d).unwrap();
        let survey = |tag: &str, sensitivity, intensity, coverage| {
            EffortRecord::build(
                ObjectId::from_canonical_bytes(PrefixKind::Claim, tag.as_bytes()),
                ExploreMethod::AircoreDrilling,
                site,
                2024,
                2024,
                MethodSpec::new(one, one, one, sensitivity, intensity).unwrap(),
                anchor(tag),
                coverage,
            )
            .unwrap()
        };
        let assess = |records: &[EffortRecord]| {
            classify_site_absence(&site, &[], records, one, q(1, 10), q(1, 20)).unwrap()
        };
        let weak = survey(
            "weak",
            q(1, 10),
            ExactQty::from_integer(2),
            CoverageConfidence::Medium,
        );
        let weak_result = assess(std::slice::from_ref(&weak));
        assert!(matches!(
            weak_result.class,
            AbsenceClass::A1LookedInsensitive { .. }
        ));
        assert_eq!(weak_result.manski_pi_bounds, (zero, q(1, 5)));
        let supported = survey("supported", q(1, 2), one, CoverageConfidence::High);
        let unknown = survey(
            "historic",
            one,
            ExactQty::from_integer(50),
            CoverageConfidence::Low,
        );
        let mixed = assess(&[unknown, supported, weak]);
        assert!(matches!(
            mixed.class,
            AbsenceClass::A4EffortSuspectedUnrecorded { .. }
        ));
        assert_eq!(mixed.intersecting_effort_count, 3);
        assert_eq!(mixed.manski_pi_bounds, (q(1, 3), q(3, 4)));
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
