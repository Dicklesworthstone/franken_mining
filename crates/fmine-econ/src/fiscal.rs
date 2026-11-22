//! Determinism: BitwisePortable
//! Exact-`Money` Net Smelter Return (NSR), flat & price-stepped royalties, corporate income tax,
//! super-profits tax, and exact discounted NPV (`BD-228`, plan §15.4, §23.4).
//!
//! Zero bare `f64` parameters in any public signature (L-6): all monetary amounts use exact
//! `fmine_units::Money` and all ratios/grades/recoveries/discount factors use `ExactQty`.

use fmine_units::{ExactQty, Money};

use crate::price_deck::{Commodity, PriceDeck, PriceUnit};
use crate::{EconError, money_mul_exact};

/// Royalty structure: either flat ad-valorem or stepped by metal price (e.g. Mali Loi n°2023-040).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoyaltySchedule {
    /// Constant fraction of gross revenue (e.g., `57 / 1000` = `5.7%` for Kibali).
    FlatAdValorem { rate: ExactQty },
    /// Price-stepped royalty:
    /// `rate(P) = fixed_surcharges + base_rate + steps(P) * step_increment`,
    /// where `steps(P) = ceil(max(0, P - base_ceiling) / step_width)` when `P > base_ceiling`,
    /// or `0` when `P <= base_ceiling` (Mali Loi n°2023-040: `7.85%` fixed + `7.5%` TAV up to
    /// `$2,500/oz` + `0.5%` per `$400/oz` above; FACT E414, E422–E424).
    PriceStepped {
        base_rate: ExactQty,
        base_ceiling: Money,
        step_increment: ExactQty,
        step_width: Money,
        fixed_surcharges: ExactQty,
    },
}

/// Evaluate the exact effective royalty rate at `metal_price`.
pub fn compute_royalty_rate(
    schedule: RoyaltySchedule,
    metal_price: Money,
) -> Result<ExactQty, EconError> {
    if metal_price.minor < 0 {
        return Err(EconError::InvalidDomain {
            field: "metal_price",
            detail: "metal price must be non-negative".to_string(),
        });
    }
    match schedule {
        RoyaltySchedule::FlatAdValorem { rate } => {
            if rate.signum() < 0
                || rate.checked_cmp(ExactQty::from_integer(1))? == core::cmp::Ordering::Greater
            {
                return Err(EconError::InvalidDomain {
                    field: "royalty.rate",
                    detail: "flat royalty rate must lie in [0, 1]".to_string(),
                });
            }
            Ok(rate)
        }
        RoyaltySchedule::PriceStepped {
            base_rate,
            base_ceiling,
            step_increment,
            step_width,
            fixed_surcharges,
        } => {
            if metal_price.ccy != base_ceiling.ccy {
                return Err(EconError::CurrencyMismatch {
                    left: metal_price.ccy,
                    right: base_ceiling.ccy,
                });
            }
            if metal_price.ccy != step_width.ccy {
                return Err(EconError::CurrencyMismatch {
                    left: metal_price.ccy,
                    right: step_width.ccy,
                });
            }
            if base_ceiling.minor < 0
                || step_width.minor <= 0
                || base_rate.signum() < 0
                || step_increment.signum() < 0
                || fixed_surcharges.signum() < 0
            {
                return Err(EconError::InvalidDomain {
                    field: "price_stepped_royalty",
                    detail: "base_ceiling >= 0, step_width > 0, and non-negative rates required"
                        .to_string(),
                });
            }
            let base_total = base_rate.checked_add(fixed_surcharges)?;
            if base_total.checked_cmp(ExactQty::from_integer(1))? == core::cmp::Ordering::Greater {
                return Err(EconError::InvalidDomain {
                    field: "effective_royalty_rate",
                    detail: "base stepped royalty rate exceeded 100%".to_string(),
                });
            }
            if metal_price.minor <= base_ceiling.minor {
                return Ok(base_total);
            }
            let excess = metal_price
                .minor
                .checked_sub(base_ceiling.minor)
                .ok_or(EconError::Overflow)?;
            // Exact ceiling division: ceil(excess / step_width).
            let steps = excess
                .checked_add(step_width.minor - 1)
                .ok_or(EconError::Overflow)?
                / step_width.minor;
            let step_add = step_increment.checked_mul(ExactQty::from_integer(steps))?;
            let total = base_total.checked_add(step_add)?;
            if total.checked_cmp(ExactQty::from_integer(1))? == core::cmp::Ordering::Greater {
                return Err(EconError::InvalidDomain {
                    field: "effective_royalty_rate",
                    detail: "stepped royalty rate exceeded 100%".to_string(),
                });
            }
            Ok(total)
        }
    }
}

/// Evaluate exact royalty `Money` due on `gross_revenue` at `metal_price`.
pub fn compute_royalty_amount(
    schedule: RoyaltySchedule,
    metal_price: Money,
    gross_revenue: Money,
) -> Result<(ExactQty, Money), EconError> {
    if metal_price.ccy != gross_revenue.ccy {
        return Err(EconError::CurrencyMismatch {
            left: metal_price.ccy,
            right: gross_revenue.ccy,
        });
    }
    let rate = compute_royalty_rate(schedule, metal_price)?;
    let amount = money_mul_exact(gross_revenue, rate)?;
    Ok((rate, amount))
}

/// One payable metal stream contributing to per-tonne ore Net Smelter Return (plan §15.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MetalStreamSpec {
    pub commodity: Commodity,
    /// Grade expressed directly in pricing units per dry tonne (e.g. `oz t / dry t` for Au,
    /// `lb / dry t` for Cu) so unit alignment is explicit and exact.
    pub pricing_units_per_dry_tonne: ExactQty,
    pub price_unit: PriceUnit,
    /// Metallurgical recovery fraction in `[0, 1]`.
    pub recovery: ExactQty,
    /// Smelter/refinery payable fraction in `[0, 1]`.
    pub payability: ExactQty,
    /// Refining / realization deduction per payable pricing unit.
    pub refining_charge_per_unit: Money,
    /// Treatment / penalty charge per dry ore tonne.
    pub tc_and_penalties_per_tonne: Money,
    pub royalty: RoyaltySchedule,
}

/// Exact per-tonne NSR breakdown for one metal stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NsrBreakdown {
    pub commodity: Commodity,
    pub payable_units_per_tonne: ExactQty,
    pub gross_value_per_tonne: Money,
    pub refining_deduction_per_tonne: Money,
    pub tc_and_penalties_per_tonne: Money,
    pub effective_royalty_rate: ExactQty,
    pub royalty_per_tonne: Money,
    pub net_smelter_return_per_tonne: Money,
}

/// Compute exact Net Smelter Return per dry tonne of ore from a versioned [`PriceDeck`] (`BD-228`).
pub fn compute_metal_nsr(
    deck: &PriceDeck,
    spec: &MetalStreamSpec,
) -> Result<NsrBreakdown, EconError> {
    let pt = deck.price(spec.commodity)?;
    if pt.unit != spec.price_unit {
        return Err(EconError::UnitMismatch {
            detail: format!(
                "deck price unit {:?} != stream unit {:?} for {}",
                pt.unit,
                spec.price_unit,
                spec.commodity.symbol()
            ),
        });
    }
    if pt.price.ccy != spec.refining_charge_per_unit.ccy {
        return Err(EconError::CurrencyMismatch {
            left: pt.price.ccy,
            right: spec.refining_charge_per_unit.ccy,
        });
    }
    if pt.price.ccy != spec.tc_and_penalties_per_tonne.ccy {
        return Err(EconError::CurrencyMismatch {
            left: pt.price.ccy,
            right: spec.tc_and_penalties_per_tonne.ccy,
        });
    }
    let zero = ExactQty::from_integer(0);
    let one = ExactQty::from_integer(1);
    if spec.pricing_units_per_dry_tonne.signum() < 0
        || spec.recovery.checked_cmp(zero)? == core::cmp::Ordering::Less
        || spec.recovery.checked_cmp(one)? == core::cmp::Ordering::Greater
        || spec.payability.checked_cmp(zero)? == core::cmp::Ordering::Less
        || spec.payability.checked_cmp(one)? == core::cmp::Ordering::Greater
        || spec.refining_charge_per_unit.minor < 0
        || spec.tc_and_penalties_per_tonne.minor < 0
    {
        return Err(EconError::InvalidDomain {
            field: "metal_stream_spec",
            detail: "grade >= 0, recovery/payability in [0, 1], charges >= 0 required".to_string(),
        });
    }

    let payable_units = spec
        .pricing_units_per_dry_tonne
        .checked_mul(spec.recovery)?
        .checked_mul(spec.payability)?;
    let gross = money_mul_exact(pt.price, payable_units)?;
    let refining = money_mul_exact(spec.refining_charge_per_unit, payable_units)?;
    let (royalty_rate, royalty_amt) = compute_royalty_amount(spec.royalty, pt.price, gross)?;
    let nsr = gross
        .checked_sub(refining)?
        .checked_sub(spec.tc_and_penalties_per_tonne)?
        .checked_sub(royalty_amt)?;

    Ok(NsrBreakdown {
        commodity: spec.commodity,
        payable_units_per_tonne: payable_units,
        gross_value_per_tonne: gross,
        refining_deduction_per_tonne: refining,
        tc_and_penalties_per_tonne: spec.tc_and_penalties_per_tonne,
        effective_royalty_rate: royalty_rate,
        royalty_per_tonne: royalty_amt,
        net_smelter_return_per_tonne: nsr,
    })
}

/// Base on which a price-contingent super-profits tax is assessed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SuperProfitsBase {
    /// Incremental gross operating surplus net of royalty delta above `trigger_price` (DRC Art. 251 bis).
    NetOfRoyaltySurplus,
    /// Gross realized price excess `max(0, price - trigger_price)` times payable units.
    GrossPriceExcess,
}

/// Price-contingent super-profits tax specification (e.g. DRC/Kibali Art. 251 bis: 50% above
/// `1.25 × BFS_price = $2,000/oz` on `$1,600/oz` BFS; FACT E242, E446).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SuperProfitsTax {
    pub bfs_price: Money,
    pub trigger_price: Money,
    pub rate: ExactQty,
    pub base: SuperProfitsBase,
}

/// Corporate income tax + optional minimum gross-revenue turnover floor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CorporateTaxTerms {
    pub cit_rate: ExactQty,
    pub min_turnover_tax_rate: Option<ExactQty>,
}

/// Complete exact period fiscal breakdown (`BD-228`, `BD-296`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FiscalBreakdown {
    pub gross_revenue: Money,
    pub effective_royalty_rate: ExactQty,
    pub royalty_paid: Money,
    pub operating_cost: Money,
    pub super_profits_tax_paid: Money,
    pub taxable_income: Money,
    pub corporate_income_tax_paid: Money,
    pub total_government_take: Money,
    pub after_tax_cashflow: Money,
}

/// Compute exact period royalties, price-contingent super-profits tax, CIT, and after-tax cashflow.
pub fn compute_fiscal_breakdown(
    payable_units: ExactQty,
    metal_price: Money,
    operating_cost: Money,
    royalty_schedule: RoyaltySchedule,
    tax_terms: CorporateTaxTerms,
    super_profits: Option<SuperProfitsTax>,
) -> Result<FiscalBreakdown, EconError> {
    if metal_price.ccy != operating_cost.ccy {
        return Err(EconError::CurrencyMismatch {
            left: metal_price.ccy,
            right: operating_cost.ccy,
        });
    }
    if payable_units.signum() < 0 || metal_price.minor < 0 || operating_cost.minor < 0 {
        return Err(EconError::InvalidDomain {
            field: "fiscal_inputs",
            detail: "payable_units, metal_price, and operating_cost must be non-negative"
                .to_string(),
        });
    }
    let one = ExactQty::from_integer(1);
    if tax_terms.cit_rate.signum() < 0
        || tax_terms.cit_rate.checked_cmp(one)? == core::cmp::Ordering::Greater
    {
        return Err(EconError::InvalidDomain {
            field: "tax_terms.cit_rate",
            detail: "CIT rate must lie in [0, 1]".to_string(),
        });
    }
    if let Some(min_rate) = tax_terms.min_turnover_tax_rate
        && (min_rate.signum() < 0 || min_rate.checked_cmp(one)? == core::cmp::Ordering::Greater)
    {
        return Err(EconError::InvalidDomain {
            field: "tax_terms.min_turnover_tax_rate",
            detail: "minimum turnover tax rate must lie in [0, 1]".to_string(),
        });
    }
    let zero_money = Money::from_minor(metal_price.ccy, 0);
    let gross_revenue = money_mul_exact(metal_price, payable_units)?;
    let (eff_roy_rate, royalty_paid) =
        compute_royalty_amount(royalty_schedule, metal_price, gross_revenue)?;

    let super_profits_tax_paid = match super_profits {
        None => zero_money,
        Some(spt) => {
            if spt.trigger_price.ccy != metal_price.ccy {
                return Err(EconError::CurrencyMismatch {
                    left: metal_price.ccy,
                    right: spt.trigger_price.ccy,
                });
            }
            if spt.bfs_price.ccy != metal_price.ccy {
                return Err(EconError::CurrencyMismatch {
                    left: metal_price.ccy,
                    right: spt.bfs_price.ccy,
                });
            }
            if spt.bfs_price.minor < 0
                || spt.trigger_price.minor < 0
                || spt.rate.signum() < 0
                || spt.rate.checked_cmp(one)? == core::cmp::Ordering::Greater
            {
                return Err(EconError::InvalidDomain {
                    field: "super_profits",
                    detail: "BFS/trigger prices must be >= 0 and rate must lie in [0, 1]"
                        .to_string(),
                });
            }
            if metal_price.minor <= spt.trigger_price.minor {
                zero_money
            } else {
                let price_excess = metal_price.checked_sub(spt.trigger_price)?;
                let gross_excess = money_mul_exact(price_excess, payable_units)?;
                let base_surplus = match spt.base {
                    SuperProfitsBase::GrossPriceExcess => gross_excess,
                    SuperProfitsBase::NetOfRoyaltySurplus => {
                        let rev_at_trigger = money_mul_exact(spt.trigger_price, payable_units)?;
                        let (_, roy_at_trigger) = compute_royalty_amount(
                            royalty_schedule,
                            spt.trigger_price,
                            rev_at_trigger,
                        )?;
                        let net_now = gross_revenue.checked_sub(royalty_paid)?;
                        let net_trigger = rev_at_trigger.checked_sub(roy_at_trigger)?;
                        let delta = net_now.checked_sub(net_trigger)?;
                        if delta.minor > 0 { delta } else { zero_money }
                    }
                };
                money_mul_exact(base_surplus, spt.rate)?
            }
        }
    };

    let ebt = gross_revenue
        .checked_sub(royalty_paid)?
        .checked_sub(operating_cost)?
        .checked_sub(super_profits_tax_paid)?;
    let taxable_income = if ebt.minor > 0 { ebt } else { zero_money };
    let cit_raw = money_mul_exact(taxable_income, tax_terms.cit_rate)?;
    let cit_paid = match tax_terms.min_turnover_tax_rate {
        Some(min_rate) => {
            let floor = money_mul_exact(gross_revenue, min_rate)?;
            if floor.minor > cit_raw.minor {
                floor
            } else {
                cit_raw
            }
        }
        None => cit_raw,
    };

    let total_gov_take = royalty_paid
        .checked_add(super_profits_tax_paid)?
        .checked_add(cit_paid)?;
    let after_tax_cashflow = gross_revenue
        .checked_sub(operating_cost)?
        .checked_sub(total_gov_take)?;

    Ok(FiscalBreakdown {
        gross_revenue,
        effective_royalty_rate: eff_roy_rate,
        royalty_paid,
        operating_cost,
        super_profits_tax_paid,
        taxable_income,
        corporate_income_tax_paid: cit_paid,
        total_government_take: total_gov_take,
        after_tax_cashflow,
    })
}

/// Compute exact Net Present Value from paired `(cashflow, discount_factor)` terms with zero
/// implicit currency conversion (`BD-228`).
pub fn exact_npv(terms: &[(Money, ExactQty)]) -> Result<Money, EconError> {
    let Some(&(first, _)) = terms.first() else {
        return Err(EconError::InvalidDomain {
            field: "npv_terms",
            detail: "at least one cashflow term is required".to_string(),
        });
    };
    let mut acc = Money::from_minor(first.ccy, 0);
    for &(cf, df) in terms {
        if cf.ccy != first.ccy {
            return Err(EconError::CurrencyMismatch {
                left: first.ccy,
                right: cf.ccy,
            });
        }
        if df.signum() < 0 {
            return Err(EconError::InvalidDomain {
                field: "discount_factor",
                detail: "discount factor must be non-negative".to_string(),
            });
        }
        let pv = money_mul_exact(cf, df)?;
        acc = acc.checked_add(pv)?;
    }
    Ok(acc)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::price_deck::PricePoint;
    use fmine_units::Currency;

    #[test]
    fn test_versioned_price_decks_nsr_royalties_and_tax_known_answers() {
        let deck = PriceDeck::build(
            "DECK-2026-PLAN",
            1,
            "2026-02-15",
            Currency::USD,
            vec![
                PricePoint::new(
                    Commodity::Au,
                    PriceUnit::PerTroyOz,
                    Money::from_major(Currency::USD, 2_000),
                )
                .unwrap(),
                PricePoint::new(
                    Commodity::Cu,
                    PriceUnit::PerAvdpLb,
                    Money::from_minor(Currency::USD, 45_000), // $4.50/lb
                )
                .unwrap(),
            ],
            None,
        )
        .expect("build deck");

        // 1. Exact Au NSR known answer:
        //    grade = 0.10 oz/t, recovery = 90%, payability = 99.5% -> 0.08955 payable oz/t
        //    gross = 0.08955 * $2,000 = $179.10/t
        //    refining = 0.08955 * $5.00 = $0.44775 -> rounds to $0.4478/t (4478 minor units)
        //    TC/penalties = $1.00/t
        //    5% flat royalty on $179.10 = $8.9550/t
        //    NSR = $179.10 - $0.4478 - $1.00 - $8.9550 = $168.6972/t
        let nsr = compute_metal_nsr(
            &deck,
            &MetalStreamSpec {
                commodity: Commodity::Au,
                pricing_units_per_dry_tonne: ExactQty::new(1, 10).unwrap(),
                price_unit: PriceUnit::PerTroyOz,
                recovery: ExactQty::new(9, 10).unwrap(),
                payability: ExactQty::new(995, 1000).unwrap(),
                refining_charge_per_unit: Money::from_major(Currency::USD, 5),
                tc_and_penalties_per_tonne: Money::from_major(Currency::USD, 1),
                royalty: RoyaltySchedule::FlatAdValorem {
                    rate: ExactQty::new(5, 100).unwrap(),
                },
            },
        )
        .expect("compute Au NSR");
        assert_eq!(nsr.gross_value_per_tonne.minor, 1_791_000); // $179.1000
        assert_eq!(nsr.refining_deduction_per_tonne.minor, 4_478); // $0.4478
        assert_eq!(nsr.royalty_per_tonne.minor, 89_550); // $8.9550
        assert_eq!(nsr.net_smelter_return_per_tonne.minor, 1_686_972); // $168.6972

        // 2. Superseded deck refuses with FMINE-REF-502 (INV-013).
        let stale_deck = PriceDeck::build(
            "DECK-2025-OLD",
            1,
            "2025-01-01",
            Currency::USD,
            vec![
                PricePoint::new(
                    Commodity::Au,
                    PriceUnit::PerTroyOz,
                    Money::from_major(Currency::USD, 1_500),
                )
                .unwrap(),
            ],
            Some("DECK-2026-PLAN@v1".to_string()),
        )
        .expect("build stale deck");
        let err = stale_deck.price(Commodity::Au).unwrap_err();
        assert_eq!(err.to_refusal().code, "FMINE-REF-502");

        // 3. Mali Loi n°2023-040 stepped royalty exact check at $2,500 and $4,500/oz:
        //    ISCP 5% + funds 2.25% + stamp 0.6% = 7.85%, TAV 7.5% up to $2,500 + 0.5%/$400 above.
        //    At $4,500/oz -> 5 steps -> 7.85% + 10.0% = 17.85% (1785/10000).
        let mali_2023 = RoyaltySchedule::PriceStepped {
            base_rate: ExactQty::new(750, 10_000).unwrap(),
            base_ceiling: Money::from_major(Currency::USD, 2_500),
            step_increment: ExactQty::new(50, 10_000).unwrap(),
            step_width: Money::from_major(Currency::USD, 400),
            fixed_surcharges: ExactQty::new(785, 10_000).unwrap(),
        };
        let r_2500 = compute_royalty_rate(mali_2023, Money::from_major(Currency::USD, 2_500))
            .expect("rate at 2500");
        assert_eq!(r_2500, ExactQty::new(1_535, 10_000).unwrap());
        let r_4500 = compute_royalty_rate(mali_2023, Money::from_major(Currency::USD, 4_500))
            .expect("rate at 4500");
        assert_eq!(r_4500, ExactQty::new(1_785, 10_000).unwrap());

        // 4. Exact NPV known answer: -$100M t=0, +$60M at df=10/11, +$60.5M at df=100/121 -> +$4.5454545M -> wait:
        //    use clean exact factors 1, 1/2, 1/4: -100 + 120*(1/2) + 200*(1/4) = -100 + 60 + 50 = +10M exact.
        let npv = exact_npv(&[
            (
                Money::from_major(Currency::USD, -100_000_000),
                ExactQty::from_integer(1),
            ),
            (
                Money::from_major(Currency::USD, 120_000_000),
                ExactQty::new(1, 2).unwrap(),
            ),
            (
                Money::from_major(Currency::USD, 200_000_000),
                ExactQty::new(1, 4).unwrap(),
            ),
        ])
        .expect("exact npv");
        assert_eq!(npv, Money::from_major(Currency::USD, 10_000_000));
    }

    #[test]
    fn test_fiscal_audit_domain_and_currency_regressions() {
        // PriceStepped base_total > 100% must refuse even when metal_price <= base_ceiling.
        let bad_stepped = RoyaltySchedule::PriceStepped {
            base_rate: ExactQty::new(6, 10).unwrap(),
            base_ceiling: Money::from_major(Currency::USD, 2_500),
            step_increment: ExactQty::new(5, 100).unwrap(),
            step_width: Money::from_major(Currency::USD, 400),
            fixed_surcharges: ExactQty::new(5, 10).unwrap(), // 60% + 50% = 110%
        };
        assert!(matches!(
            compute_royalty_rate(bad_stepped, Money::from_major(Currency::USD, 1_000)),
            Err(EconError::InvalidDomain { .. })
        ));

        // SuperProfitsTax bfs_price currency mismatch must report bfs_price.ccy, not trigger_price.ccy.
        let err = compute_fiscal_breakdown(
            ExactQty::from_integer(1),
            Money::from_major(Currency::USD, 3_000),
            Money::from_major(Currency::USD, 1_000),
            RoyaltySchedule::FlatAdValorem {
                rate: ExactQty::new(5, 100).unwrap(),
            },
            CorporateTaxTerms {
                cit_rate: ExactQty::new(3, 10).unwrap(),
                min_turnover_tax_rate: None,
            },
            Some(SuperProfitsTax {
                bfs_price: Money::from_major(Currency::CAD, 1_600),
                trigger_price: Money::from_major(Currency::USD, 2_000),
                rate: ExactQty::new(5, 10).unwrap(),
                base: SuperProfitsBase::NetOfRoyaltySurplus,
            }),
        )
        .unwrap_err();
        assert_eq!(
            err,
            EconError::CurrencyMismatch {
                left: Currency::USD,
                right: Currency::CAD,
            }
        );

        // Out-of-bounds CIT rate (> 100%) must refuse.
        assert!(matches!(
            compute_fiscal_breakdown(
                ExactQty::from_integer(1),
                Money::from_major(Currency::USD, 2_000),
                Money::from_major(Currency::USD, 1_000),
                RoyaltySchedule::FlatAdValorem {
                    rate: ExactQty::new(5, 100).unwrap(),
                },
                CorporateTaxTerms {
                    cit_rate: ExactQty::new(12, 10).unwrap(),
                    min_turnover_tax_rate: None,
                },
                None,
            ),
            Err(EconError::InvalidDomain { .. })
        ));
    }
}
