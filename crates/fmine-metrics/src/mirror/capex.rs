//! DESIGN (BD-346.S4): capital-line mirrors and a separately declared component-sum assessment.
//! The joint result never substitutes an independently rounded total for a sum of printed components.
//! Determinism: Exact

use super::*;

const SUM_FAMILY: &str = "mirror_capex_component_sum";

/// All three independent single-line checks, plus their separately declared joint construction.
#[derive(Debug, Clone, PartialEq)]
pub struct CapexReport {
    /// Each line uses its own printed half-step box. These are not the joint component-sum verdict.
    pub lines: Vec<MirrorReport>,
    pub joint: Option<CapexJointReport>,
    /// Absence is never an implicit successful joint assessment.
    pub joint_unavailable_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CapexJointReport {
    /// Source indices in the verdict refer to this stable reporter/metric/Claim ordering.
    pub inputs: Vec<Claim<MirrorReading>>,
    pub normalized: Vec<Claim<MirrorReading>>,
    pub cells: Vec<xf::Cell>,
    pub identities: Vec<xf::Identity>,
    pub rules: xf::Rules,
    pub construction: xf::Construction,
    pub verdict: MirrorVerdict,
}

fn owner_key(ownership: Ownership) -> String {
    match ownership {
        Ownership::Full => "full".into(),
        Ownership::Attributable { share } => {
            format!("attributable:{}/{}", share.num(), share.den())
        }
    }
}

/// Compare sustaining, project and total USD-million readings. Every line must have the same reporters,
/// asset, period, dated operating perimeter, timing and lease treatment. No basis adjustment is inferred.
/// A joint assessment additionally requires each reporter's complete unambiguous component triple on at
/// least one ownership basis. Printed totals are component sums by declaration, never by per-row selection.
pub fn compare_capex(inputs: &[Claim<MirrorReading>]) -> Result<CapexReport> {
    let mut by_scope = BTreeMap::<CapexScope, Vec<Claim<MirrorReading>>>::new();
    for input in inputs {
        validate(input)?;
        let MirrorMetric::Capex(scope) = input.value.quantity.kind else {
            return refuse(
                RefusalCode::BasisMismatch105,
                "joint capex accepts capital amounts only",
            );
        };
        by_scope.entry(scope).or_default().push(input.clone());
    }
    let mut lines = Vec::new();
    for scope in CapexScope::ALL {
        let rows = by_scope.get(&scope).ok_or_else(|| {
            Refusal::new(
                RefusalCode::NoClaimInDomain201,
                format!("joint capex lacks the {} line", scope.as_str()),
            )
        })?;
        lines.push(compare(rows)?);
    }
    let mut inputs = inputs.to_vec();
    inputs.sort_by(|a, b| {
        (&a.value.reporter, a.value.quantity.kind, a.claim_id).cmp(&(
            &b.value.reporter,
            b.value.quantity.kind,
            b.claim_id,
        ))
    });
    let first = &inputs[0].value;
    let reporters: BTreeSet<_> = inputs.iter().map(|i| i.value.reporter.as_str()).collect();
    let mut operations = first.perimeter.operations.clone();
    operations.sort();
    let mut shares = BTreeMap::new();
    let mut source_rows = BTreeSet::new();
    for input in &inputs {
        let r = &input.value;
        let mut members = r.perimeter.operations.clone();
        members.sort();
        if r.quantity.subject != first.quantity.subject
            || r.quantity.period != first.quantity.period
        {
            return refuse(
                RefusalCode::SupportMismatch102,
                "capital lines must name one asset and period",
            );
        }
        if r.perimeter.id != first.perimeter.id || members != operations {
            return refuse(
                RefusalCode::NonIdentifiableAttribution104,
                "capital lines name different operating perimeters",
            );
        }
        let basis = r.quantity.capex_basis;
        let initial = first.quantity.capex_basis;
        if basis.map(|b| (b.timing, b.leases)) != initial.map(|b| (b.timing, b.leases)) {
            return refuse(
                RefusalCode::NonIdentifiableAttribution104,
                "capital lines need the same stated timing and lease treatment",
            );
        }
        if !source_rows.insert((&r.reporter, &r.source)) {
            return refuse(
                RefusalCode::NonIdentifiableAttribution104,
                "one capital source row cannot supply multiple line identities",
            );
        }
        if let Ownership::Attributable { share } = r.quantity.ownership
            && shares
                .insert(&r.reporter, share)
                .is_some_and(|old| old != share)
        {
            return refuse(
                RefusalCode::BasisMismatch105,
                "one reporter's capital lines have incompatible ownership shares",
            );
        }
    }
    for line in &lines {
        let line_reporters: BTreeSet<_> = line
            .inputs
            .iter()
            .map(|i| i.value.reporter.as_str())
            .collect();
        if line_reporters != reporters {
            return refuse(
                RefusalCode::NoClaimInDomain201,
                "every capital line requires the same reporters",
            );
        }
    }
    let normalized = inputs.iter().map(normalize).collect::<Result<Vec<_>>>()?;
    let cells = inputs.iter().map(proof_cell).collect::<Result<Vec<_>>>()?;
    let mut identities = Vec::new();
    for scope in CapexScope::ALL {
        let members: Vec<_> = inputs
            .iter()
            .enumerate()
            .filter(|(_, i)| i.value.quantity.kind == MirrorMetric::Capex(scope))
            .map(|(i, _)| i)
            .collect();
        let first = members[0];
        for &i in &members[1..] {
            identities.push(xf::Identity {
                id: format!("capex-scale-{}-{i}", scope.as_str()),
                family: "mirror_ownership".into(),
                closure: xf::Closure::Definitional,
                kind: xf::IdentityKind::Scale {
                    target: cells[i].locator.clone(),
                    source: cells[first].locator.clone(),
                    factor: checked(
                        share(&inputs[i].value.quantity)
                            .checked_div(share(&inputs[first].value.quantity)),
                    )?,
                },
            });
        }
    }
    let mut triples = BTreeMap::<(String, String), BTreeMap<CapexScope, Vec<usize>>>::new();
    for (i, input) in inputs.iter().enumerate() {
        let MirrorMetric::Capex(scope) = input.value.quantity.kind else {
            return refuse(
                RefusalCode::BasisMismatch105,
                "non-capital input in a capital triple",
            );
        };
        triples
            .entry((
                input.value.reporter.clone(),
                owner_key(input.value.quantity.ownership),
            ))
            .or_default()
            .entry(scope)
            .or_default()
            .push(i);
    }
    let mut complete_reporters = BTreeSet::new();
    for ((reporter, owner), scopes) in &triples {
        if scopes.values().any(|rows| rows.len() > 1) {
            return Ok(CapexReport {
                lines,
                joint: None,
                joint_unavailable_reason: Some(format!(
                    "ambiguous capital line readings for {reporter} on the {owner} ownership basis; no component triple was selected"
                )),
            });
        }
        if CapexScope::ALL
            .iter()
            .any(|s| scopes.get(s).is_none_or(|v| v.len() != 1))
        {
            continue;
        }
        let sustaining = scopes[&CapexScope::Sustaining][0];
        let project = scopes[&CapexScope::Project][0];
        let total = scopes[&CapexScope::Total][0];
        identities.push(xf::Identity {
            id: format!("capex-sum-{reporter}-{owner}"),
            family: SUM_FAMILY.into(),
            closure: xf::Closure::Definitional,
            kind: xf::IdentityKind::Sum {
                total: cells[total].locator.clone(),
                lines: vec![
                    cells[sustaining].locator.clone(),
                    cells[project].locator.clone(),
                ],
                signs: vec![1, 1],
            },
        });
        complete_reporters.insert(reporter.as_str());
    }
    if complete_reporters != reporters {
        let missing = reporters
            .difference(&complete_reporters)
            .copied()
            .collect::<Vec<_>>()
            .join(", ");
        return Ok(CapexReport {
            lines,
            joint: None,
            joint_unavailable_reason: Some(format!(
                "no complete unambiguous sustaining/project/total triple on one reporting ownership basis for: {missing}"
            )),
        });
    }
    let construction = xf::Construction::ComponentSum;
    let rules = xf::Rules::new(xf::Convention::Closed).with(SUM_FAMILY, construction);
    let components = checked(xf::solve(&cells, &identities, &rules))?;
    let [component] = components.as_slice() else {
        return refuse(
            RefusalCode::NumericalCertificateFailed207,
            "joint capital identities must form one connected component",
        );
    };
    let verdict = match &component.outcome {
        xf::Outcome::Feasible { witness } => {
            checked(xf::check_witness(&cells, &identities, &rules, witness))?;
            MirrorVerdict::Consistent {
                witness: witness.clone(),
            }
        }
        xf::Outcome::Infeasible { certificate } => {
            checked(xf::check_certificate(
                &cells,
                &identities,
                &rules,
                certificate,
            ))?;
            let certificate_digest = checked(xf::certificate_digest(
                &cells,
                &identities,
                &rules,
                certificate,
            ))?;
            let blame = checked(xf::blame(&cells, &identities, &rules))?;
            let [blame] = blame.as_slice() else {
                return refuse(
                    RefusalCode::NumericalCertificateFailed207,
                    "joint capital blame must name one component",
                );
            };
            let mut minimal_source_sets = Vec::new();
            if let xf::BlameOutcome::Blamed(b) = &blame.outcome {
                for set in &b.conflicts {
                    // The kernel's deletion runs check minimality. Re-check each resulting certificate;
                    // a joint sum conflict can involve three or more cells, unlike a single scale line.
                    checked(xf::check_conflict_set(&cells, &identities, &rules, set))?;
                    let mut indices = Vec::new();
                    for at in &set.cells {
                        let Some(i) = cells.iter().position(|c| c.locator == *at) else {
                            return refuse(
                                RefusalCode::NumericalCertificateFailed207,
                                "joint blame names an unknown source cell",
                            );
                        };
                        indices.push(i);
                    }
                    indices.sort();
                    minimal_source_sets.push(indices);
                }
            }
            minimal_source_sets.sort();
            minimal_source_sets.dedup();
            MirrorVerdict::Conflict {
                certificate: certificate.clone(),
                certificate_digest,
                blame: blame.outcome.clone(),
                minimal_source_sets,
            }
        }
        xf::Outcome::Indeterminate { reason } => MirrorVerdict::Indeterminate {
            reason: reason.clone(),
        },
        xf::Outcome::NotRefuted => MirrorVerdict::Indeterminate {
            reason: "a relaxation is not a checked capital witness".into(),
        },
    };
    Ok(CapexReport {
        lines,
        joint: Some(CapexJointReport {
            inputs,
            normalized,
            cells,
            identities,
            rules,
            construction,
            verdict,
        }),
        joint_unavailable_reason: None,
    })
}

#[cfg(test)]
mod tests {
    use super::super::tests::{reading, reseal};
    use super::*;

    fn rat(n: i128, d: i128) -> Rat {
        Rat::new(n, d).unwrap()
    }

    fn capital(
        reporter: &str,
        scope: CapexScope,
        value: i128,
        ownership: Ownership,
    ) -> Claim<MirrorReading> {
        let mut claim = reading(reporter, scope.as_str(), value, ownership);
        let source = &claim.value.source;
        let basis_source = SourceAnchor::new(
            source.artifact_path.clone(),
            source.artifact_sha256,
            format!("{}:p3:Invented capital definition", source.artifact_path),
        );
        claim.anchors.push(basis_source.clone());
        claim.value.basis_source = Some(basis_source);
        claim.value.quantity.kind = MirrorMetric::Capex(scope);
        claim.value.quantity.capex_basis = Some(CapexBasis {
            scope,
            timing: CapexTiming::Cash,
            leases: LeaseTreatment::Included,
        });
        reseal(&mut claim);
        claim
    }

    fn pair(left: [i128; 3], right: [i128; 3]) -> Vec<Claim<MirrorReading>> {
        [("left", left), ("right", right)]
            .into_iter()
            .flat_map(|(reporter, values)| {
                CapexScope::ALL
                    .into_iter()
                    .zip(values)
                    .map(move |(scope, value)| capital(reporter, scope, value, Ownership::Full))
            })
            .collect()
    }

    #[test]
    fn same_basis_capex_retains_units_citations_and_signed_amounts() {
        for (value, lo, hi) in [
            (100, rat(199, 2), rat(201, 2)),
            (0, rat(-1, 2), rat(1, 2)),
            (-10, rat(-21, 2), rat(-19, 2)),
        ] {
            let inputs = [
                capital("left", CapexScope::Sustaining, value, Ownership::Full),
                capital("right", CapexScope::Sustaining, value, Ownership::Full),
            ];
            let report = compare(&inputs).unwrap();
            assert!(!report.physical_nonnegative);
            let common = report.common.unwrap();
            assert_eq!(common.value.quantity.kind.unit(), "USD_m");
            assert_eq!(common.value.quantity.interval.lo, lo);
            assert_eq!(common.value.quantity.interval.hi, hi);
            assert_eq!(common.evidence, EvidenceClass::Assumed);
            assert_eq!(common.parents.len(), 2);
            for input in &inputs {
                assert!(
                    common
                        .anchors
                        .contains(input.value.basis_source.as_ref().unwrap())
                );
            }
            let MirrorVerdict::Consistent { witness } = report.verdict else {
                panic!("expected a signed-capital witness")
            };
            xf::check_witness(&report.cells, &report.identities, &report.rules, &witness).unwrap();
        }
        let report = compare(&[
            capital("left", CapexScope::Total, 1000, Ownership::Full),
            capital(
                "right",
                CapexScope::Total,
                615,
                Ownership::Attributable {
                    share: rat(123, 200),
                },
            ),
        ])
        .unwrap();
        let common = report.common.unwrap();
        assert_eq!(common.value.quantity.interval.lo, rat(1999, 2));
        assert_eq!(common.value.quantity.interval.hi, rat(2001, 2));
    }

    #[test]
    fn capital_basis_is_required_cited_and_cannot_be_silently_bridged() {
        for case in 0..8 {
            let mut inputs = pair([60, 40, 100], [60, 40, 100]);
            let right = &mut inputs[3];
            let code = match case {
                0 => {
                    right.value.quantity.capex_basis = None;
                    "104"
                }
                1 => {
                    right.value.quantity.capex_basis.as_mut().unwrap().timing =
                        CapexTiming::Unstated;
                    "104"
                }
                2 => {
                    right.value.quantity.capex_basis.as_mut().unwrap().leases =
                        LeaseTreatment::Unstated;
                    "104"
                }
                3 => {
                    right.value.quantity.capex_basis.as_mut().unwrap().timing =
                        CapexTiming::Accrual;
                    "104"
                }
                4 => {
                    right.value.quantity.capex_basis.as_mut().unwrap().leases =
                        LeaseTreatment::Excluded;
                    "104"
                }
                5 => {
                    right.value.basis_source = None;
                    "104"
                }
                6 => {
                    right.value.quantity.capex_basis.as_mut().unwrap().scope = CapexScope::Project;
                    "105"
                }
                _ => {
                    let source = right.value.basis_source.as_mut().unwrap();
                    source.artifact_path = "OtherIssuer.pdf".into();
                    right.anchors.push(source.clone());
                    "104"
                }
            };
            reseal(right);
            assert_eq!(
                compare(&[inputs[0].clone(), inputs[3].clone()])
                    .unwrap_err()
                    .code,
                format!("FMINE-REF-{code}")
            );
        }
        let mut input = capital("left", CapexScope::Sustaining, 60, Ownership::Full);
        input.value.quantity.capex_basis.as_mut().unwrap().leases = LeaseTreatment::Excluded;
        assert_eq!(
            compare(&[
                input,
                capital("right", CapexScope::Sustaining, 60, Ownership::Full)
            ])
            .unwrap_err()
            .code,
            "FMINE-REF-507"
        );
    }

    #[test]
    fn capex_sum_identity_sustaining_plus_project_has_checked_witness() {
        let report = compare_capex(&pair([60, 40, 100], [60, 40, 100])).unwrap();
        assert_eq!(report.lines.len(), 3);
        assert!(report.joint_unavailable_reason.is_none());
        let joint = report.joint.unwrap();
        assert_eq!(joint.construction, xf::Construction::ComponentSum);
        assert_eq!(
            joint
                .identities
                .iter()
                .filter(|i| matches!(i.kind, xf::IdentityKind::Sum { .. }))
                .count(),
            2
        );
        let MirrorVerdict::Consistent { witness } = joint.verdict else {
            panic!("expected joint witness")
        };
        xf::check_witness(&joint.cells, &joint.identities, &joint.rules, &witness).unwrap();
        let mut corrupted = witness;
        *corrupted.values_mut().next().unwrap() = Rat::from_integer(999);
        assert!(
            xf::check_witness(&joint.cells, &joint.identities, &joint.rules, &corrupted).is_err()
        );
    }

    #[test]
    fn independent_line_agreement_cannot_hide_a_joint_footing_conflict() {
        let report = compare_capex(&pair([60, 40, 101], [60, 40, 101])).unwrap();
        assert!(
            report
                .lines
                .iter()
                .all(|r| matches!(r.verdict, MirrorVerdict::Consistent { .. }))
        );
        let joint = report.joint.unwrap();
        let MirrorVerdict::Conflict {
            certificate,
            minimal_source_sets,
            blame,
            ..
        } = joint.verdict
        else {
            panic!("101 != 60 + 40")
        };
        xf::check_certificate(&joint.cells, &joint.identities, &joint.rules, &certificate).unwrap();
        assert!(minimal_source_sets.iter().any(|set| set.len() == 3));
        let xf::BlameOutcome::Blamed(blame) = blame else {
            panic!("small synthetic conflict is attributable")
        };
        for set in &blame.conflicts {
            xf::check_conflict_set(&joint.cells, &joint.identities, &joint.rules, set).unwrap();
        }
    }

    #[test]
    fn component_sum_uses_component_rounding_not_the_total_half_step() {
        // Independent totals 2 and 4 conflict, but both separately rounded component lines meet at 1.5.
        // Their true total is 3; choosing the total's own half-step box would reject this valid joint model.
        let report = compare_capex(&pair([1, 1, 2], [2, 2, 4])).unwrap();
        assert!(matches!(
            report.lines[2].verdict,
            MirrorVerdict::Conflict { .. }
        ));
        let joint = report.joint.unwrap();
        let MirrorVerdict::Consistent { witness } = joint.verdict else {
            panic!("component-sum tie is feasible")
        };
        xf::check_witness(&joint.cells, &joint.identities, &joint.rules, &witness).unwrap();
        for (input, cell) in joint.inputs.iter().zip(&joint.cells) {
            if input.value.quantity.kind == MirrorMetric::Capex(CapexScope::Total) {
                assert_eq!(witness[&cell.locator], Rat::from_integer(3));
            }
        }
    }

    #[test]
    fn incomplete_ownership_triple_is_explicit_and_input_order_is_immaterial() {
        let mut inputs = pair([60, 40, 100], [60, 40, 100]);
        let before = compare_capex(&inputs).unwrap();
        inputs.reverse();
        assert_eq!(compare_capex(&inputs).unwrap(), before);
        let project = inputs
            .iter_mut()
            .find(|i| {
                i.value.reporter == "left"
                    && i.value.quantity.kind == MirrorMetric::Capex(CapexScope::Project)
            })
            .unwrap();
        project.value.quantity.ownership = Ownership::Attributable { share: rat(1, 2) };
        project.value.quantity.interval.lo = rat(79, 4);
        project.value.quantity.interval.hi = rat(81, 4);
        reseal(project);
        let report = compare_capex(&inputs).unwrap();
        assert!(
            report
                .lines
                .iter()
                .all(|r| matches!(r.verdict, MirrorVerdict::Consistent { .. }))
        );
        assert!(report.joint.is_none());
        assert!(report.joint_unavailable_reason.unwrap().contains("left"));
    }

    #[test]
    fn independently_rounded_capital_components_preserve_known_truth() {
        // Two independently rounded attributable series, with displayed totals formed from the lines.
        // Every source box contains its separately computed rational truth, so every joint must be feasible.
        for n in 100_i128..164 {
            let mut inputs = Vec::new();
            for (reporter, numerator) in [("left", 3), ("right", 2)] {
                let a = (n * numerator + 25) / 50;
                let b = ((200 + n % 7) * numerator + 25) / 50;
                for (scope, value) in CapexScope::ALL.into_iter().zip([a, b, a + b]) {
                    inputs.push(capital(
                        reporter,
                        scope,
                        value,
                        Ownership::Attributable {
                            share: rat(numerator, 5),
                        },
                    ));
                }
            }
            let joint = compare_capex(&inputs).unwrap().joint.unwrap();
            let MirrorVerdict::Consistent { witness } = joint.verdict else {
                panic!("lost known capital truth for n={n}")
            };
            xf::check_witness(&joint.cells, &joint.identities, &joint.rules, &witness).unwrap();
        }
    }

    #[test]
    fn a_complete_attributable_triple_cannot_hide_ambiguous_full_basis_lines() {
        let mut inputs = pair([60, 40, 100], [60, 40, 100]);
        for (scope, value) in CapexScope::ALL.into_iter().zip([30, 20, 50]) {
            let mut input = capital(
                "left",
                scope,
                value,
                Ownership::Attributable { share: rat(1, 2) },
            );
            input.value.source.locator.push_str(" attributable");
            input.anchors.push(input.value.source.clone());
            reseal(&mut input);
            inputs.push(input);
        }
        let mut ambiguous = capital("left", CapexScope::Sustaining, 60, Ownership::Full);
        ambiguous
            .value
            .source
            .locator
            .push_str(" second full reading");
        ambiguous.anchors.push(ambiguous.value.source.clone());
        reseal(&mut ambiguous);
        inputs.push(ambiguous);
        let report = compare_capex(&inputs).unwrap();
        assert!(
            report
                .lines
                .iter()
                .all(|r| matches!(r.verdict, MirrorVerdict::Consistent { .. }))
        );
        assert!(report.joint.is_none());
        assert!(
            report
                .joint_unavailable_reason
                .unwrap()
                .contains("ambiguous")
        );
    }
}
