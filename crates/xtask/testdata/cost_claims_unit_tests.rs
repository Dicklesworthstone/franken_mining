#[cfg(test)]
mod tests {
    use super::*;
    use fmine_claim::{Confidentiality, Representation, SpatialSupport, UuTerm};
    use fmine_core::anchor::SourceAnchor;
    use fmine_evidence::{EpistemicStatus, EvidenceClass, KnowledgeState};

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
            receipt.summary().usd_per_unit,
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
            evaluate(&reg, &altered).unwrap().summary().usd_per_unit,
            ExactQty::from_integer(-2)
        );
        altered[2] = line("on_site_mining_costs", -1_000_000);
        assert_eq!(
            evaluate(&reg, &altered).unwrap().summary().usd_per_unit,
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
                receipt.summary().usd_per_unit,
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
        assert_eq!(
            receipt.summary().usd_per_unit,
            ExactQty::new(15, 2).unwrap()
        );
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
                    .usd_per_unit
                    .checked_sub(base.summary().usd_per_unit)
                    .unwrap(),
                ExactQty::new(increment, 30000).unwrap()
            );
            assert_ne!(got.claim().claim_id, base.claim().claim_id);
        }
        let mut changed_exclusion = original;
        changed_exclusion[3] = line("income_tax", 124);
        let got = evaluate(&reg, &changed_exclusion).unwrap();
        assert_eq!(got.summary().usd_per_unit, base.summary().usd_per_unit);
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
        altered_rules
            .definitions
            .iter_mut()
            .find(|definition| definition.id == "aisc@wgc-2013")
            .unwrap()
            .source
            .push_str(" changed");
        assert_eq!(
            evaluate(&altered_rules, &base).unwrap_err().code,
            "FMINE-REF-507"
        );
    }

    fn copper_worksheet(id: &str) -> (MetricRegistry, Vec<Claim<CostInput>>) {
        let (reg, mut inputs) = worksheet(id, &[], 400);
        inputs[1] = leaf(CostValue::SoldPounds(ExactQty::from_integer(400)));
        for (item, usd) in [
            ("cost_of_sales", 1000),
            ("depreciation_amortization_removed", 200),
            ("treatment_refining_charges", 100),
            ("equity_method_c1_cash_costs", 150),
            ("royalties_removed_from_cost_of_sales", 50),
            ("costs_allocated_to_by_products", 20),
            ("corporate_general_and_administrative", 40),
            ("reclamation_accretion_and_amortisation_operating_sites", 10),
            ("royalties_added", 50),
            ("exploration_and_evaluation_sustaining", 20),
            ("minesite_sustaining_capital_expenditures", 95),
            ("sustaining_leases", 5),
        ] {
            inputs.push(line(item, usd * Money::SCALE));
        }
        (reg, inputs)
    }

    #[test]
    fn copper_c1_and_aisc_share_one_population_with_explicit_deductions_and_sold_pounds() {
        // Independent invented arithmetic: C1 = 1000-200+100+150-50-20 = 980;
        // AISC adds 40+10+50+20+95+5 = 220. Divide each by 400 copper lb sold.
        for (id, numerator, rate) in [
            (
                "c1.copper@barrick-q2-2026",
                980,
                ExactQty::new(49, 20).unwrap(),
            ),
            (
                "aisc.copper@barrick-q2-2026",
                1200,
                ExactQty::from_integer(3),
            ),
        ] {
            let (reg, mut inputs) = copper_worksheet(id);
            let receipt = evaluate(&reg, &inputs).unwrap();
            assert_eq!(receipt.summary().unit, CostUnit::CopperPoundSold);
            assert_eq!(receipt.summary().quantity_sold, ExactQty::from_integer(400));
            assert_eq!(
                receipt.summary().numerator_usd,
                ExactQty::from_integer(numerator)
            );
            assert_eq!(receipt.summary().usd_per_unit, rate);
            assert_eq!(receipt.inputs().len(), 14);
            assert_eq!(receipt.claim().parents.len(), 14);
            let depreciation = receipt
                .summary()
                .contributions
                .iter()
                .find(|c| c.item == "depreciation_amortization_removed")
                .unwrap();
            assert_eq!(depreciation.treatment, Treatment::Deduction);
            assert_eq!(depreciation.numerator_usd, ExactQty::from_integer(-200));
            inputs.reverse();
            assert_eq!(evaluate(&reg, &inputs).unwrap(), receipt);
            let json = receipt.to_json();
            assert!(json.contains("\"pounds_sold\":\"400\""));
            assert!(!json.contains("ounces_sold"));
            assert!(!json.contains("usd_per_oz"));
            assert!(receipt.to_markdown().contains("copper pounds sold"));
        }
    }

    #[test]
    fn copper_units_allocation_and_deductions_cannot_be_silently_reinterpreted() {
        let id = "c1.copper@barrick-q2-2026";
        let (reg, original) = copper_worksheet(id);
        let pounds_id = original[1].claim_id;
        let ounces = leaf(CostValue::SoldOunces(ExactQty::from_integer(400)));
        assert_ne!(pounds_id, ounces.claim_id);
        let mut wrong_unit = original.clone();
        wrong_unit[1] = ounces.clone();
        assert_eq!(
            evaluate(&reg, &wrong_unit).unwrap_err().code,
            "FMINE-REF-105"
        );
        let mut ambiguous = original.clone();
        ambiguous.push(ounces);
        assert_eq!(
            evaluate(&reg, &ambiguous).unwrap_err().code,
            "FMINE-REF-105"
        );
        let mut wrong_share = original.clone();
        wrong_share.push(leaf(CostValue::GoldShare(ONE)));
        assert_eq!(
            evaluate(&reg, &wrong_share).unwrap_err().code,
            "FMINE-REF-105"
        );
        for item in [
            "depreciation_amortization_removed",
            "royalties_removed_from_cost_of_sales",
            "costs_allocated_to_by_products",
        ] {
            let mut negative = original.clone();
            let position = negative
                .iter()
                .position(|c| {
                    matches!(&c.value.value, CostValue::Line { item: found, .. } if found == item)
                })
                .unwrap();
            negative[position] = line(item, -Money::SCALE);
            assert_eq!(evaluate(&reg, &negative).unwrap_err().code, "FMINE-REF-109");
            let raw = [CostLine {
                item: item.into(),
                usd: ExactQty::from_integer(-1),
            }];
            assert!(matches!(
                cost_per_sold_unit(&reg, id, &raw, CostUnit::CopperPoundSold, ONE, None),
                Err(MetricFailure::NegativeDeduction(_))
            ));
        }
        let raw = [CostLine {
            item: "cost_of_sales".into(),
            usd: ONE,
        }];
        assert!(matches!(
            super::super::all_in_cost_per_oz(&reg, id, &raw, ONE, None),
            Err(MetricFailure::UnitMismatch { .. })
        ));
        // Unlisted inventory adjustments may already be in CoS and cannot be added a second time.
        let mut undeclared = original.clone();
        undeclared.push(line("inventory_write_down_added", Money::SCALE));
        assert_eq!(
            evaluate(&reg, &undeclared).unwrap_err().code,
            "FMINE-REF-105"
        );
    }

    #[test]
    fn coproduct_allocation_retains_non_byproduct_reconciliation_deductions() {
        let id = "c1.copper@barrick-q2-2026";
        let (mut reg, mut inputs) = copper_worksheet(id);
        let mut definition = reg.definition(id).unwrap().clone();
        definition.id = "copper.allocated@invented".into();
        definition.credit_basis = CreditBasis::CoProduct;
        definition.extends = vec![id.into()];
        reg.definitions.push(definition);
        inputs[0] = leaf(CostValue::Rule {
            definition: "copper.allocated@invented".into(),
            definition_tag: definition_tag(&reg, "copper.allocated@invented").unwrap(),
            complete: true,
        });
        inputs.push(leaf(CostValue::CopperShare(ExactQty::new(1, 2).unwrap())));
        let receipt = evaluate(&reg, &inputs).unwrap();
        // (1000-200+100+150-50) * 1/2 / 400 = 1.25. Only the 20 by-product credit is excluded.
        assert_eq!(receipt.summary().usd_per_unit, ExactQty::new(5, 4).unwrap());
        let contributions = &receipt.summary().contributions;
        assert_eq!(
            contributions
                .iter()
                .find(|c| c.item == "depreciation_amortization_removed")
                .unwrap()
                .numerator_usd,
            ExactQty::from_integer(-100)
        );
        assert_eq!(
            contributions
                .iter()
                .find(|c| c.item == "costs_allocated_to_by_products")
                .unwrap()
                .treatment,
            Treatment::Excluded
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
        assert_eq!(assumed.summary().usd_per_unit, base.summary().usd_per_unit);
        let same_input_id = inputs[3].claim_id;
        inputs[3].uu_terms[0].downside_exposure_usd_m = 1.0f64.next_up();
        assert_eq!(same_input_id, inputs[3].claim_id);
        let changed = evaluate(&reg, &inputs).unwrap();
        assert_ne!(changed.claim().claim_id, assumed.claim().claim_id);
        assert_ne!(changed.digest(), assumed.digest());
        inputs[2].uu_terms.push(unknown(2.0));
        assert_eq!(evaluate(&reg, &inputs).unwrap_err().code, "FMINE-REF-204");
    }

    fn driver(item: &str, quantity: ExactQty, rate: ExactQty, fixed_minor: i128) -> CostValue {
        CostValue::DriverLine {
            item: item.into(),
            driver: DeclaredDriver {
                kind: "tonnes_mined".into(),
                unit: "dry_t".into(),
                quantity,
                usd_per_unit: rate,
                fixed_usd: Money::from_minor(Currency::USD, fixed_minor),
            },
        }
    }

    #[test]
    fn declared_driver_costs_preserve_exact_fixed_variable_and_credit_components() {
        // Independent invented answer: 100.01 t * 7.5 USD/t + 49.935 USD = 800.01 USD.
        // (800.01 mining + 300 overhead + 100 leases - 40 * 5 credits) / 100 oz = 10.0001.
        let (reg, mut inputs) = worksheet(
            "aisc@wgc-2018",
            &[
                ("corporate_general_and_administrative", 3_000_000),
                ("lease_payments", 1_000_000),
            ],
            100,
        );
        inputs.push(
            leaf(driver(
                "on_site_mining_costs",
                ExactQty::new(10001, 100).unwrap(),
                ExactQty::new(15, 2).unwrap(),
                499350,
            ))
            .downgraded(EvidenceClass::Assumed),
        );
        inputs.push(leaf(driver(
            "by_product_credits",
            ExactQty::from_integer(40),
            ExactQty::from_integer(5),
            0,
        )));
        inputs.push(leaf(driver("income_tax", ZERO, ZERO, 9_990_000)));
        let receipt = evaluate(&reg, &inputs).unwrap();
        assert_eq!(
            receipt.summary().numerator_usd,
            ExactQty::new(100001, 100).unwrap()
        );
        assert_eq!(
            receipt.summary().usd_per_unit,
            ExactQty::new(100001, 10000).unwrap()
        );
        assert_eq!(receipt.claim().evidence, EvidenceClass::Assumed);
        assert_eq!(receipt.inputs().len(), inputs.len());
        assert_eq!(receipt.claim().parents.len(), inputs.len());
        let credit = receipt
            .summary()
            .contributions
            .iter()
            .find(|c| c.item == "by_product_credits")
            .unwrap();
        assert_eq!(credit.amount.minor, 2_000_000);
        assert_eq!(credit.numerator_usd, ExactQty::from_integer(-200));
        assert_eq!(credit.treatment, Treatment::Credit);
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
        let json = receipt.to_json();
        for operand in [
            "\"kind\":\"driver_cost\"",
            "\"quantity\":\"10001/100\"",
            "\"usd_per_unit\":\"15/2\"",
            "\"fixed_minor\":\"499350\"",
        ] {
            assert!(json.contains(operand), "{operand}: {json}");
        }
        assert!(receipt.to_markdown().contains("## Declared cost drivers"));
        inputs.reverse();
        assert_eq!(evaluate(&reg, &inputs).unwrap(), receipt);
    }

    #[test]
    fn driver_costs_refuse_ambiguous_units_nonphysical_values_and_inexact_money() {
        let (reg, mut inputs) = worksheet("aisc@wgc-2018", &[], 100);
        let base = driver("on_site_mining_costs", ONE, ONE, 0);
        for (name, unit) in [
            ("tonnes_mined", "dry_t"),
            ("tonnes_mined", "wet_t"),
            ("tonnes_milled", "dry_t"),
            ("tonnes_milled", "wet_t"),
            ("haulage", "dry_t_km"),
            ("haulage", "wet_t_km"),
            ("electricity", "kWh"),
            ("reagent", "kg"),
            ("labour", "hour"),
        ] {
            let mut value = base.clone();
            if let CostValue::DriverLine { driver, .. } = &mut value {
                driver.kind = name.into();
                driver.unit = unit.into();
                driver.quantity = ZERO;
                driver.fixed_usd.minor = 100_000;
            }
            inputs.truncate(2);
            inputs.push(leaf(value));
            assert_eq!(
                evaluate(&reg, &inputs).unwrap().summary().usd_per_unit,
                ExactQty::new(1, 10).unwrap()
            );
        }
        for (edit, code) in [
            ("unit", "FMINE-REF-105"),
            ("driver", "FMINE-REF-105"),
            ("currency", "FMINE-REF-105"),
            ("quantity", "FMINE-REF-109"),
            ("rate", "FMINE-REF-109"),
            ("fixed", "FMINE-REF-109"),
            ("precision", "FMINE-REF-207"),
            ("overflow", "FMINE-REF-207"),
        ] {
            let mut value = base.clone();
            if let CostValue::DriverLine { driver, .. } = &mut value {
                match edit {
                    "unit" => driver.unit = "t".into(),
                    "driver" => driver.kind = "invented-driver".into(),
                    "currency" => driver.fixed_usd.ccy = Currency::CAD,
                    "quantity" => driver.quantity = ExactQty::from_integer(-1),
                    "rate" => driver.usd_per_unit = ExactQty::from_integer(-1),
                    "fixed" => driver.fixed_usd.minor = -1,
                    "precision" => driver.usd_per_unit = ExactQty::new(1, 3).unwrap(),
                    "overflow" => driver.quantity = ExactQty::from_integer(i128::MAX),
                    _ => unreachable!(),
                }
            }
            inputs.truncate(2);
            inputs.push(leaf(value));
            assert_eq!(evaluate(&reg, &inputs).unwrap_err().code, code, "{edit}");
        }
        inputs.truncate(2);
        inputs.extend([leaf(base), line("on_site_mining_costs", 1)]);
        assert_eq!(evaluate(&reg, &inputs).unwrap_err().code, "FMINE-REF-105");
    }

    #[test]
    fn driver_operand_changes_bind_the_receipt_even_when_the_cost_stays_equal() {
        let (reg, mut inputs) = worksheet("aisc@wgc-2018", &[], 100);
        inputs.push(leaf(driver(
            "on_site_mining_costs",
            ExactQty::from_integer(10),
            ExactQty::from_integer(20),
            50_000,
        )));
        let original = evaluate(&reg, &inputs).unwrap();
        inputs[2] = leaf(driver(
            "on_site_mining_costs",
            ExactQty::from_integer(20),
            ExactQty::from_integer(10),
            50_000,
        ));
        let changed = evaluate(&reg, &inputs).unwrap();
        assert_eq!(
            original.summary().numerator_usd,
            ExactQty::from_integer(205)
        );
        assert_eq!(
            original.summary().usd_per_unit,
            changed.summary().usd_per_unit
        );
        assert_ne!(original.digest(), changed.digest());
        assert_ne!(original.claim().claim_id, changed.claim().claim_id);
        if let CostValue::DriverLine { driver, .. } = &mut inputs[2].value.value {
            driver.quantity = ExactQty::from_integer(10);
        }
        assert_eq!(evaluate(&reg, &inputs).unwrap_err().code, "FMINE-REF-507");
    }
}
