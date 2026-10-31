//! Original FrankenMining code (not lifted). Upstream analogue: frankensim crates/fs-evidence/src/lib.rs @ 20463525d (not copied).
//! The colour a class may carry: the last column of the `§7.1` table (`BD-037`, `BD-038`).

use crate::color::{Color, ColorRank};
use crate::epistemic::{AoaStatus, EvidenceClass};

/// Projects a class onto a colour, given the colour its anchor or inputs support.
///
/// `Assertion` gets none: it is refused as a colour leaf until reviewed. A class that §7.1 lets stand on an anchor keeps
/// `basis`. Every other class is `Estimated`, and a `basis` stronger than that is ignored, so a caller cannot raise
/// a colour by supplying one. With no `basis` there is nothing to stand on, so the spread is unbounded.
#[must_use]
pub fn project_color(evidence: EvidenceClass, basis: Option<&Color>) -> Option<Color> {
    let unbounded = |estimator: &str| {
        Some(Color::Estimated {
            estimator: estimator.to_string(),
            dispersion: f64::INFINITY,
        })
    };
    let anchored = matches!(
        evidence,
        EvidenceClass::Measured
            | EvidenceClass::Derived
            | EvidenceClass::ModelledCalibrated
            | EvidenceClass::Transferred {
                aoa: AoaStatus::Inside
            }
    );
    match (evidence, basis) {
        (EvidenceClass::Assertion { .. }, _) => None,
        (EvidenceClass::Assumed, _) => unbounded("class:assumed"),
        (_, Some(color)) if anchored || color.rank() == ColorRank::Estimated => Some(color.clone()),
        _ => unbounded("class:unanchored"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::color::ValidityDomain;
    use crate::epistemic::AssertionSource;

    fn validated() -> Color {
        Color::Validated {
            regime: ValidityDomain::unconstrained().with("feed_grade", 1.0, 3.0),
            dataset: "qaqc-2026".to_string(),
        }
    }

    fn estimated() -> Color {
        Color::Estimated {
            estimator: "kriging".to_string(),
            dispersion: 0.2,
        }
    }

    fn is_unbounded(color: Option<Color>) -> bool {
        matches!(color, Some(Color::Estimated { dispersion, .. }) if dispersion.is_infinite())
    }

    #[test]
    fn test_assertion_is_refused_as_a_colour_leaf_whatever_its_basis() {
        for source in [
            AssertionSource::Agent,
            AssertionSource::Extraction,
            AssertionSource::Issuer,
            AssertionSource::Secondary,
        ] {
            let class = EvidenceClass::Assertion { source };
            assert_eq!(project_color(class, None), None);
            assert_eq!(project_color(class, Some(&validated())), None);
        }
    }

    #[test]
    fn test_assumed_is_estimated_with_unbounded_dispersion() {
        assert!(is_unbounded(project_color(EvidenceClass::Assumed, None)));
        assert!(is_unbounded(project_color(
            EvidenceClass::Assumed,
            Some(&validated())
        )));
        assert!(is_unbounded(project_color(
            EvidenceClass::Assumed,
            Some(&estimated())
        )));
    }

    #[test]
    fn test_anchored_classes_keep_a_validated_basis_and_need_one() {
        for class in [
            EvidenceClass::Measured,
            EvidenceClass::Derived,
            EvidenceClass::ModelledCalibrated,
            EvidenceClass::Transferred {
                aoa: AoaStatus::Inside,
            },
        ] {
            assert_eq!(project_color(class, Some(&validated())), Some(validated()));
            assert!(is_unbounded(project_color(class, None)), "{class:?}");
        }
    }

    #[test]
    fn test_unanchored_classes_never_rise_above_estimated() {
        for class in [
            EvidenceClass::ModelledUncalibrated,
            EvidenceClass::Elicited,
            EvidenceClass::Transferred {
                aoa: AoaStatus::Outside,
            },
            EvidenceClass::Transferred {
                aoa: AoaStatus::NotAssessed,
            },
        ] {
            assert!(is_unbounded(project_color(class, Some(&validated()))));
            assert_eq!(project_color(class, Some(&estimated())), Some(estimated()));
            let verified = Color::Verified { lo: 1.0, hi: 2.0 };
            assert!(is_unbounded(project_color(class, Some(&verified))));
        }
    }
}
