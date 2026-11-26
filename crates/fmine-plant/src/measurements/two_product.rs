//! DESIGN (BD-137). Compare submitted stream totals with the two-product assay identity.
//! The identity assumes two dry outlets, a common interval and no change of circuit inventory.
//! Determinism: Exact

use super::{MaterialProjectionReceipt, ONE, ZERO, bad, exact, worksheet};
use crate::flowsheet::ElementKey;
use fmine_claim::{AdditivityClass, Claim, Confidentiality, Representation, SpatialSupport};
use fmine_core::anchor::SourceAnchor;
use fmine_evidence::{EpistemicStatus, EvidenceClass, KnowledgeState, Refusal, RefusalCode};
use fmine_units::ExactQty;
use std::collections::BTreeSet;

pub const NO_CLAIM: &str = "Worksheet values are Assumed, Internal and uncalibrated. The assay split assumes feed, concentrate and tailings are the only dry streams and circuit inventory is unchanged. Residuals are feed minus outlets. This diagnostic supplies no independent sampling, inventory or period-close attestation.";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssaySplit {
    Solved {
        mass_pull: ExactQty,
        recovery: Option<ExactQty>,
    },
    MissingAssay,
    EmptyDryStream,
    IndistinguishableProducts,
    FeedOutsideProductGrades,
}

impl AssaySplit {
    pub fn report(self) -> (&'static str, Option<ExactQty>, Option<ExactQty>) {
        match self {
            Self::Solved {
                mass_pull,
                recovery: Some(r),
            } => ("solved", Some(mass_pull), Some(r)),
            Self::Solved {
                mass_pull,
                recovery: None,
            } => ("mass_pull_only_zero_feed_grade", Some(mass_pull), None),
            Self::MissingAssay => ("missing_assay", None, None),
            Self::EmptyDryStream => ("empty_dry_stream", None, None),
            Self::IndistinguishableProducts => ("indistinguishable_product_grades", None, None),
            Self::FeedOutsideProductGrades => ("feed_outside_product_grades", None, None),
        }
    }

    pub fn submitted_dry_pull_comparison(self, submitted: Option<ExactQty>) -> &'static str {
        match (self, submitted) {
            (Self::Solved { mass_pull, .. }, Some(dry_pull)) if mass_pull == dry_pull => "matches",
            (Self::Solved { .. }, Some(_)) => "differs",
            _ => "unavailable",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ElementBalance {
    pub element: ElementKey,
    /// Feed, concentrate and tailings; a missing assay is distinct from zero kg.
    pub stream_metal_kg: [Option<ExactQty>; 3],
    pub residual_kg: Option<ExactQty>,
    pub concentrate_over_feed_metal: Option<ExactQty>,
    pub assay_split: AssaySplit,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Balance {
    pub dry_residual_kg: ExactQty,
    pub concentrate_over_feed_dry_mass: Option<ExactQty>,
    pub elements: Vec<ElementBalance>,
}

#[derive(Clone, Copy)]
pub struct AssayPullWitness<'a> {
    pub element: &'a ElementKey,
    pub mass_pull: ExactQty,
}

#[derive(Default)]
pub struct AssayPullCheck<'a> {
    pub solved_assays: usize,
    pub infeasible_assays: usize,
    pub unresolved_assays: usize,
    pub reference_assay: Option<AssayPullWitness<'a>>,
    pub conflicting_assay: Option<AssayPullWitness<'a>>,
}

impl AssayPullCheck<'_> {
    pub fn status(&self) -> &'static str {
        match (self.solved_assays, self.conflicting_assay) {
            (0, _) => "unidentified",
            (1, _) => "one_solved_assay",
            (_, Some(_)) => "conflicting_solved_assays",
            _ => "matching_solved_assays",
        }
    }
}

impl Balance {
    /// A two-product boundary has one concentrate/feed dry mass ratio for all elements.
    /// Compare the exact assay-implied ratios at the submitted precision, keeping individually
    /// infeasible and unresolved elements separate from the identifying assays.
    pub fn assay_pull_check(&self) -> AssayPullCheck<'_> {
        let mut check = AssayPullCheck::default();
        for row in &self.elements {
            match row.assay_split {
                AssaySplit::Solved { mass_pull, .. } => {
                    check.solved_assays += 1;
                    let witness = AssayPullWitness {
                        element: &row.element,
                        mass_pull,
                    };
                    let first = check.reference_assay.get_or_insert(witness);
                    if mass_pull != first.mass_pull && check.conflicting_assay.is_none() {
                        check.conflicting_assay = Some(witness);
                    }
                }
                AssaySplit::FeedOutsideProductGrades => check.infeasible_assays += 1,
                AssaySplit::MissingAssay
                | AssaySplit::EmptyDryStream
                | AssaySplit::IndistinguishableProducts => {
                    check.unresolved_assays += 1;
                }
            }
        }
        check
    }
}

pub struct Receipt {
    streams: [MaterialProjectionReceipt; 3],
    claim: Claim<Balance>,
}

pub const ROLES: [&str; 3] = ["feed", "concentrate", "tailings"];

impl Receipt {
    pub fn streams(&self) -> &[MaterialProjectionReceipt; 3] {
        &self.streams
    }

    pub fn claim(&self) -> &Claim<Balance> {
        &self.claim
    }

    pub fn to_json(&self) -> String {
        use crate::inventory::worksheet::{claim_json, exact, quoted};
        let optional = |q: Option<ExactQty>| q.map_or_else(|| "null".into(), exact);
        let b = &self.claim.value;
        let scope = &self.streams[0].summary().scope;
        let streams = ROLES.into_iter().zip(&self.streams).map(|(role, stream)| {
            let summary = stream.summary();
            format!(r#"{{"role":{},"material":{},"dry_kg":{},"projection_digest":{},"claim_id":{}}}"#,
                quoted(role), quoted(&summary.scope.material), exact(summary.amounts.dry_solids_kg),
                quoted(stream.digest()), quoted(&stream.claim().claim_id.to_string()))
        }).collect::<Vec<_>>().join(",");
        let elements = b.elements.iter().map(|row| {
            let (status, pull, recovery) = row.assay_split.report();
            let comparison = row.assay_split.submitted_dry_pull_comparison(b.concentrate_over_feed_dry_mass);
            let [f, c, t] = row.stream_metal_kg.map(optional);
            format!(r#"{{"element":{},"measurand":{},"feed_kg":{f},"concentrate_kg":{c},"tailings_kg":{t},"residual_kg":{},"concentrate_over_feed_metal":{},"assay_status":{},"assay_mass_pull":{},"assay_recovery":{},"submitted_dry_pull_comparison":{}}}"#,
                quoted(&row.element.element), quoted(&row.element.measurand), optional(row.residual_kg),
                optional(row.concentrate_over_feed_metal), quoted(status), optional(pull), optional(recovery), quoted(comparison))
        }).collect::<Vec<_>>().join(",");
        let check = b.assay_pull_check();
        let witness = |w: Option<AssayPullWitness<'_>>| {
            w.map_or_else(
                || "null".into(),
                |v| {
                    format!(
                        r#"{{"element":{},"measurand":{},"mass_pull":{}}}"#,
                        quoted(&v.element.element),
                        quoted(&v.element.measurand),
                        exact(v.mass_pull)
                    )
                },
            )
        };
        let pull_check = format!(
            r#"{{"status":{},"solved_assays":{},"infeasible_assays":{},"unresolved_assays":{},"reference_assay":{},"conflicting_assay":{}}}"#,
            quoted(check.status()),
            check.solved_assays,
            check.infeasible_assays,
            check.unresolved_assays,
            witness(check.reference_assay),
            witness(check.conflicting_assay)
        );
        format!(
            r#"{{"schema":"fmine.two-product-screen.v1","boundary":{},"start_ns":"{}","end_ns":"{}","streams":[{streams}],"dry_residual_kg":{},"concentrate_over_feed_dry_mass":{},"elements":[{elements}],"claim":{},"assay_pull_check":{pull_check}}}"#,
            quoted(&scope.boundary),
            scope.physical_time.start_ns,
            scope.physical_time.end_ns,
            exact(b.dry_residual_kg),
            optional(b.concentrate_over_feed_dry_mass),
            claim_json(&self.claim)
        )
    }
}

type Result<T> = std::result::Result<T, Refusal>;

fn ratio(n: ExactQty, d: ExactQty) -> Result<Option<ExactQty>> {
    if d == ZERO {
        Ok(None)
    } else {
        exact(n.checked_div(d)).map(Some)
    }
}

fn residual([feed, concentrate, tailings]: [ExactQty; 3]) -> Result<ExactQty> {
    exact(exact(feed.checked_sub(concentrate))?.checked_sub(tailings))
}

fn assay_split(dry: [ExactQty; 3], metal: [ExactQty; 3]) -> Result<AssaySplit> {
    if dry.contains(&ZERO) {
        return Ok(AssaySplit::EmptyDryStream);
    }
    let [f, c, t] = [
        exact(metal[0].checked_div(dry[0]))?,
        exact(metal[1].checked_div(dry[1]))?,
        exact(metal[2].checked_div(dry[2]))?,
    ];
    if c == t {
        return Ok(if f == t {
            AssaySplit::IndistinguishableProducts
        } else {
            AssaySplit::FeedOutsideProductGrades
        });
    }
    // f = p*c + (1-p)*t. Depleted elements can report preferentially to tailings.
    let pull = exact(exact(f.checked_sub(t))?.checked_div(exact(c.checked_sub(t))?))?;
    if pull.signum() < 0 || exact(pull.checked_cmp(ONE))?.is_gt() {
        return Ok(AssaySplit::FeedOutsideProductGrades);
    }
    Ok(AssaySplit::Solved {
        mass_pull: pull,
        recovery: ratio(exact(pull.checked_mul(c))?, f)?,
    })
}

/// The worksheet admission fixes all three source envelopes as internal, assumed and uncalibrated.
/// Stream role order is part of the derived claim identity.
pub fn evaluate(sources: [(&str, &SourceAnchor); 3]) -> Result<Receipt> {
    let [feed, concentrate, tailings] =
        sources.map(|(text, source)| worksheet::receipt(text, source));
    let streams = [feed?, concentrate?, tailings?];
    let projections = streams.each_ref().map(MaterialProjectionReceipt::summary);
    let scope = &projections[0].scope;
    if scope.physical_time.start_ns >= scope.physical_time.end_ns
        || projections.iter().any(|p| {
            p.scope.boundary != scope.boundary || p.scope.physical_time != scope.physical_time
        })
        || projections
            .iter()
            .map(|p| &p.scope.material)
            .collect::<BTreeSet<_>>()
            .len()
            != 3
    {
        return Err(bad(
            RefusalCode::SupportMismatch102,
            "three distinct stream identities must cover the same boundary and positive-duration interval",
        ));
    }
    let dry = projections.map(|p| p.amounts.dry_solids_kg);
    let keys: BTreeSet<_> = projections
        .iter()
        .flat_map(|p| p.amounts.elements_kg.keys())
        .collect();
    let mut elements = Vec::with_capacity(keys.len());
    for key in keys {
        let metal = projections.map(|p| p.amounts.elements_kg.get(key).copied());
        let direct = match (metal[0], metal[1]) {
            (Some(f), Some(c)) => ratio(c, f)?,
            _ => None,
        };
        let (residual_kg, assay_split) = match metal {
            [Some(f), Some(c), Some(t)] => {
                (Some(residual([f, c, t])?), assay_split(dry, [f, c, t])?)
            }
            _ => (None, AssaySplit::MissingAssay),
        };
        elements.push(ElementBalance {
            element: key.clone(),
            stream_metal_kg: metal,
            residual_kg,
            concentrate_over_feed_metal: direct,
            assay_split,
        });
    }
    let balance = Balance {
        dry_residual_kg: residual(dry)?,
        concentrate_over_feed_dry_mass: ratio(dry[1], dry[0])?,
        elements,
    };
    let tag = format!(
        "two-product/v1/{}/{}/{}",
        streams[0].digest(),
        streams[1].digest(),
        streams[2].digest()
    );
    let claim = Claim::new(
        "plant.measurement.two-product/1",
        balance,
        &tag,
        Representation::Deterministic,
        SpatialSupport::PeriodAggregate,
        AdditivityClass::NonAdditive,
        EvidenceClass::Assumed,
        EpistemicStatus::KnownKnown,
        KnowledgeState::Known,
        Confidentiality::internal(),
        vec![],
        streams.iter().map(|r| r.claim().claim_id).collect(),
        streams
            .iter()
            .flat_map(|r| r.claim().anchors.iter().cloned())
            .collect(),
    );
    Ok(Receipt { streams, claim })
}

#[cfg(test)]
mod tests {
    use super::*;
    use fmine_core::hash::sha256;

    const FEED: &str = include_str!("../../../../tests/plant/material_projection.toml");
    const CONC: &str = include_str!("../../../../tests/plant/two_product_concentrate.toml");
    const TAIL: &str = include_str!("../../../../tests/plant/two_product_tailings.toml");

    fn screen(f: &str, c: &str, t: &str) -> Result<Receipt> {
        let sources = [("feed", f), ("concentrate", c), ("tailings", t)]
            .map(|(role, text)| SourceAnchor::new(role, sha256(text.as_bytes()).0, "worksheet"));
        evaluate([(f, &sources[0]), (c, &sources[1]), (t, &sources[2])])
    }

    #[test]
    fn forward_product_masses_recover_the_known_split_for_enriched_and_depleted_elements() {
        let q = ExactQty::from_integer;
        for (conc_t, tail_t) in [(1, 1), (3, 7), (11, 3)] {
            for cg in [0, 1, 5] {
                for tg in [0, 2, 5] {
                    let dry = [q(10 * (conc_t + tail_t)), q(10 * conc_t), q(10 * tail_t)];
                    let feed = cg * conc_t + tg * tail_t;
                    let metal = [q(feed), q(cg * conc_t), q(tg * tail_t)];
                    let expected = if cg == tg {
                        AssaySplit::IndistinguishableProducts
                    } else {
                        AssaySplit::Solved {
                            mass_pull: ExactQty::new(conc_t, conc_t + tail_t).unwrap(),
                            recovery: Some(ExactQty::new(cg * conc_t, feed).unwrap()),
                        }
                    };
                    assert_eq!(assay_split(dry, metal).unwrap(), expected);
                }
            }
        }
        assert_eq!(
            assay_split([q(10); 3], [q(0), q(1), q(0)]).unwrap(),
            AssaySplit::Solved {
                mass_pull: ZERO,
                recovery: None
            }
        );
        assert_eq!(
            assay_split([q(10); 3], [q(1), q(2), q(2)]).unwrap(),
            AssaySplit::FeedOutsideProductGrades
        );
        assert_eq!(
            assay_split([q(10); 3], [q(5), q(2), q(3)]).unwrap(),
            AssaySplit::FeedOutsideProductGrades
        );
        assert_eq!(
            assay_split([q(10), ZERO, q(5)], [q(1), ZERO, q(1)]).unwrap(),
            AssaySplit::EmptyDryStream
        );
    }

    #[test]
    fn split_retains_unclosed_masses_missing_assays_and_source_identity() {
        let base = screen(FEED, CONC, TAIL).unwrap();
        let reversed = screen(FEED, TAIL, CONC).unwrap();
        assert_ne!(base.claim.claim_id, reversed.claim.claim_id);
        assert_eq!(
            reversed.claim.value.elements[1].assay_split,
            AssaySplit::Solved {
                mass_pull: ExactQty::new(9, 10).unwrap(),
                recovery: Some(ExactQty::new(1, 5).unwrap()),
            }
        );
        let doubled = screen(FEED, &CONC.replace("\"0.3\"", "\"0.6\""), TAIL).unwrap();
        assert_ne!(base.claim.claim_id, doubled.claim.claim_id);
        assert_eq!(
            doubled.claim.value.dry_residual_kg,
            ExactQty::from_integer(-250)
        );
        let cu = &doubled.claim.value.elements[1];
        assert_eq!(cu.residual_kg, Some(ExactQty::from_integer(-40)));
        assert_eq!(
            cu.concentrate_over_feed_metal,
            Some(ExactQty::new(8, 5).unwrap())
        );
        assert_eq!(cu.assay_split, base.claim.value.elements[1].assay_split);
        let missing = screen(FEED, CONC, TAIL.rsplit_once("[[assay]]").unwrap().0).unwrap();
        assert_eq!(
            missing.claim.value.elements[0],
            base.claim.value.elements[0]
        );
        let cu = &missing.claim.value.elements[1];
        assert_eq!(
            (cu.residual_kg, cu.assay_split),
            (None, AssaySplit::MissingAssay)
        );
        assert_eq!(
            cu.concentrate_over_feed_metal,
            Some(ExactQty::new(4, 5).unwrap())
        );
        for tail in [
            TAIL.replace("synthetic-plant/v1", "other"),
            TAIL.replace("end_ns = \"10\"", "end_ns = \"11\""),
            TAIL.replace("tailings/period-1", "feed/period-1"),
        ] {
            assert_eq!(
                screen(FEED, CONC, &tail).err().unwrap().code,
                "FMINE-REF-102"
            );
        }
        let instant = [FEED, CONC, TAIL].map(|s| s.replace("end_ns = \"10\"", "end_ns = \"0\""));
        assert_eq!(
            screen(&instant[0], &instant[1], &instant[2])
                .err()
                .unwrap()
                .code,
            "FMINE-REF-102"
        );
        let anchors = [FEED, CONC, TAIL]
            .map(|s| SourceAnchor::new("source", sha256(s.as_bytes()).0, "worksheet"));
        assert_eq!(
            evaluate([
                (FEED, &anchors[0]),
                (CONC, &anchors[1]),
                (TAIL, &anchors[0])
            ])
            .err()
            .unwrap()
            .code,
            "FMINE-REF-505"
        );
    }

    #[test]
    fn common_dry_split_counts_unresolved_and_infeasible_assays_separately() {
        let conc_without_cu = CONC.rsplit_once("[[assay]]").unwrap().0;
        let single = screen(FEED, conc_without_cu, TAIL).unwrap();
        let check = single.claim.value.assay_pull_check();
        assert_eq!(check.status(), "one_solved_assay");
        assert_eq!(
            (
                check.solved_assays,
                check.infeasible_assays,
                check.unresolved_assays
            ),
            (1, 0, 1)
        );
        assert!(check.conflicting_assay.is_none());
        assert_eq!(check.reference_assay.unwrap().element.element, "Au");
        assert_eq!(
            single.claim.value.elements[1]
                .assay_split
                .submitted_dry_pull_comparison(Some(ONE)),
            "unavailable"
        );

        let impossible =
            screen(&FEED.replace("value = \"2\"", "value = \"20\""), CONC, TAIL).unwrap();
        let check = impossible.claim.value.assay_pull_check();
        assert_eq!(check.status(), "one_solved_assay");
        assert_eq!(
            (
                check.solved_assays,
                check.infeasible_assays,
                check.unresolved_assays
            ),
            (1, 1, 0)
        );
        assert!(check.conflicting_assay.is_none());

        let flat_au_conc = CONC.replace("value = \"20\"", "value = \"4\"");
        let flat_au_tail = TAIL.replace("value = \"20/9\"", "value = \"4\"");
        let r = screen(FEED, &flat_au_conc, &flat_au_tail).unwrap();
        assert_eq!(
            r.claim.value.assay_pull_check().status(),
            "one_solved_assay"
        );
        assert_eq!(r.claim.value.assay_pull_check().unresolved_assays, 1);

        let both_flat = screen(
            FEED,
            &flat_au_conc.replace("value = \"16\"", "value = \"2\""),
            &flat_au_tail.replace("value = \"4/9\"", "value = \"2\""),
        )
        .unwrap();
        let check = both_flat.claim.value.assay_pull_check();
        assert_eq!(check.status(), "unidentified");
        assert_eq!(check.unresolved_assays, 2);
        assert!(check.reference_assay.is_none());
    }
}
