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
