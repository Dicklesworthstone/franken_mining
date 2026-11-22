//! Determinism: BitwisePortable
//! `CountryRegime` registry parser, `Restricted` / `NOT_PUBLISHED` operational states, and
//! price-contingent fiscal evaluation (`BD-296`, plan §23.4, `RISK-26`, `FACT E242–E246, E410–E449`).

use std::collections::BTreeSet;

use fmine_units::{Currency, ExactQty, Money};

use crate::EconError;
use crate::fiscal::{
    CorporateTaxTerms, FiscalBreakdown, RoyaltySchedule, SuperProfitsBase, SuperProfitsTax,
    compute_fiscal_breakdown,
};
use crate::price_deck::{Commodity, PriceDeck};

/// Cause of an operation-level `Restricted` state (distinct from full `Suspension`; §23.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RestrictionCause {
    Reagent,
    Water,
    StockpileOnly,
}

/// Country and operation-level fiscal/political state (plan §23.4, `RISK-26`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegimeState {
    Stable,
    FiscalShock,
    SecurityDisruption,
    Suspension,
    Restricted { cause: RestrictionCause },
    NotPublished { reason: String },
}

impl RegimeState {
    fn parse(raw: &str) -> Result<Self, EconError> {
        match raw {
            "Stable" => Ok(Self::Stable),
            "FiscalShock" => Ok(Self::FiscalShock),
            "SecurityDisruption" => Ok(Self::SecurityDisruption),
            "Suspension" => Ok(Self::Suspension),
            other => {
                if let Some(sub) = other.strip_prefix("Restricted:") {
                    let cause = match sub {
                        "reagent" => RestrictionCause::Reagent,
                        "water" => RestrictionCause::Water,
                        "stockpile-only" => RestrictionCause::StockpileOnly,
                        _ => {
                            return Err(EconError::RegistryParse {
                                detail: format!("unknown Restricted cause {sub:?}"),
                            });
                        }
                    };
                    Ok(Self::Restricted { cause })
                } else if let Some(reason) = other.strip_prefix("NotPublished:") {
                    if reason.is_empty() {
                        return Err(EconError::RegistryParse {
                            detail: "NotPublished state requires a non-empty reason".to_string(),
                        });
                    }
                    Ok(Self::NotPublished {
                        reason: reason.to_string(),
                    })
                } else {
                    Err(EconError::RegistryParse {
                        detail: format!("unknown RegimeState {other:?}"),
                    })
                }
            }
        }
    }
}

/// One registered `CountryRegime` row with statutory/contractual instrument and as-of date.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CountryRegimeRow {
    pub id: String,
    pub country: String,
    pub asset: String,
    pub instrument: String,
    pub as_of_date: String,
    pub superseded_on: Option<String>,
    pub state: RegimeState,
    pub royalty: RoyaltySchedule,
    pub tax_terms: CorporateTaxTerms,
    pub super_profits: Option<SuperProfitsTax>,
    pub state_equity_fraction: ExactQty,
    pub renewal_equity_step_fraction: ExactQty,
    pub source_e_ids: Vec<String>,
}

/// Result of evaluating a `CountryRegimeRow` against a versioned [`PriceDeck`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegimeFiscalEvaluation {
    pub regime_id: String,
    pub instrument: String,
    pub as_of_date: String,
    pub state: RegimeState,
    pub deck_id: String,
    pub deck_version: u32,
    pub super_profits_triggered: bool,
    pub breakdown: FiscalBreakdown,
}

/// Parse `registries/country_regimes.toml` (schema version 1).
pub fn parse_country_regime_registry(text: &str) -> Result<Vec<CountryRegimeRow>, EconError> {
    let mut saw_version = false;
    let mut rows = Vec::new();
    let mut seen_ids = BTreeSet::new();
    let mut cur: Option<Vec<(String, String)>> = None;

    let flush = |fields: Vec<(String, String)>,
                 seen: &mut BTreeSet<String>,
                 out: &mut Vec<CountryRegimeRow>|
     -> Result<(), EconError> {
        let get = |k: &str| -> Result<String, EconError> {
            fields
                .iter()
                .find(|(fk, _)| fk == k)
                .map(|(_, v)| v.clone())
                .ok_or_else(|| EconError::RegistryParse {
                    detail: format!("[[regime]] missing field '{k}'"),
                })
        };
        let get_u64 = |k: &str| -> Result<u64, EconError> {
            let v = get(k)?;
            v.parse::<u64>().map_err(|_| EconError::RegistryParse {
                detail: format!("invalid u64 for '{k}': {v:?}"),
            })
        };
        let bps = |val: u64| -> Result<ExactQty, EconError> {
            Ok(ExactQty::new(i128::from(val), 10_000)?)
        };

        let id = get("id")?;
        if id.is_empty() || !seen.insert(id.clone()) {
            return Err(EconError::RegistryParse {
                detail: format!("empty or duplicate regime id {id:?}"),
            });
        }
        let state = RegimeState::parse(&get("state")?)?;
        let sup_raw = get("superseded_on")?;
        let superseded_on = if sup_raw.is_empty() {
            None
        } else {
            Some(sup_raw)
        };

        let royalty_kind = get("royalty_kind")?;
        let royalty = match royalty_kind.as_str() {
            "flat" => RoyaltySchedule::FlatAdValorem {
                rate: bps(get_u64("royalty_rate_bps")?)?,
            },
            "stepped" => RoyaltySchedule::PriceStepped {
                base_rate: bps(get_u64("royalty_rate_bps")?)?,
                base_ceiling: Money::from_major(
                    Currency::USD,
                    i128::from(get_u64("royalty_ceiling_usd")?),
                ),
                step_increment: bps(get_u64("royalty_step_bps")?)?,
                step_width: Money::from_major(
                    Currency::USD,
                    i128::from(get_u64("royalty_step_width_usd")?),
                ),
                fixed_surcharges: bps(get_u64("royalty_fixed_surcharges_bps")?)?,
            },
            other => {
                return Err(EconError::RegistryParse {
                    detail: format!("regime '{id}': unknown royalty_kind {other:?}"),
                });
            }
        };

        let cit_rate = bps(get_u64("cit_rate_bps")?)?;
        let min_turn_bps = get_u64("min_turnover_tax_bps")?;
        let tax_terms = CorporateTaxTerms {
            cit_rate,
            min_turnover_tax_rate: if min_turn_bps > 0 {
                Some(bps(min_turn_bps)?)
            } else {
                None
            },
        };

        let spt_rate_bps = get_u64("super_profits_rate_bps")?;
        let super_profits = if spt_rate_bps > 0 {
            Some(SuperProfitsTax {
                bfs_price: Money::from_major(
                    Currency::USD,
                    i128::from(get_u64("super_profits_bfs_usd")?),
                ),
                trigger_price: Money::from_major(
                    Currency::USD,
                    i128::from(get_u64("super_profits_trigger_usd")?),
                ),
                rate: bps(spt_rate_bps)?,
                base: SuperProfitsBase::NetOfRoyaltySurplus,
            })
        } else {
            None
        };

        let source_e_ids: Vec<String> = get("source_e_ids")?
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(ToString::to_string)
            .collect();
        if source_e_ids.is_empty() {
            return Err(EconError::RegistryParse {
                detail: format!("regime '{id}' must cite at least one E id"),
            });
        }

        out.push(CountryRegimeRow {
            id,
            country: get("country")?,
            asset: get("asset")?,
            instrument: get("instrument")?,
            as_of_date: get("as_of_date")?,
            superseded_on,
            state,
            royalty,
            tax_terms,
            super_profits,
            state_equity_fraction: bps(get_u64("state_equity_bps")?)?,
            renewal_equity_step_fraction: bps(get_u64("renewal_equity_step_bps")?)?,
            source_e_ids,
        });
        Ok(())
    };

    for (idx, raw) in text.lines().enumerate() {
        let line = idx + 1;
        let s = raw.trim();
        if s.is_empty() || s.starts_with('#') {
            continue;
        }
        if s == "[[regime]]" {
            if !saw_version {
                return Err(EconError::RegistryParse {
                    detail: format!("line {line}: schema_version = 1 must precede [[regime]]"),
                });
            }
            if let Some(prev) = cur.take() {
                flush(prev, &mut seen_ids, &mut rows)?;
            }
            cur = Some(Vec::new());
            continue;
        }
        let Some((k, v)) = s.split_once('=') else {
            return Err(EconError::RegistryParse {
                detail: format!("line {line}: expected key = value"),
            });
        };
        let key = k.trim();
        let val = v.trim();
        match cur.as_mut() {
            None => {
                if key == "schema_version" && val == "1" {
                    saw_version = true;
                } else {
                    return Err(EconError::RegistryParse {
                        detail: format!("line {line}: expected schema_version = 1"),
                    });
                }
            }
            Some(fields) => {
                let parsed = val
                    .strip_prefix('"')
                    .and_then(|x| x.strip_suffix('"'))
                    .map_or_else(|| val.to_string(), ToString::to_string);
                fields.push((key.to_string(), parsed));
            }
        }
    }
    if let Some(last) = cur.take() {
        flush(last, &mut seen_ids, &mut rows)?;
    }
    if !saw_version || rows.is_empty() {
        return Err(EconError::RegistryParse {
            detail: "country_regimes.toml requires schema_version = 1 and >= 1 [[regime]]"
                .to_string(),
        });
    }
    Ok(rows)
}

/// Evaluate a `CountryRegimeRow` at `deck` for `payable_units` and `operating_cost`.
///
/// Refuses superseded rows (`FMINE-REF-502`) unless `allow_historical` is true, and refuses
/// `RegimeState::NotPublished` (`FMINE-REF-201`) so missing site-quarters are never silently filled.
pub fn evaluate_regime_fiscal_terms(
    row: &CountryRegimeRow,
    deck: &PriceDeck,
    commodity: Commodity,
    payable_units: ExactQty,
    operating_cost: Money,
    allow_historical: bool,
) -> Result<RegimeFiscalEvaluation, EconError> {
    if !allow_historical && let Some(ref sup) = row.superseded_on {
        return Err(EconError::StaleVersion {
            kind: "country_regime",
            id: row.id.clone(),
            superseded_by: format!("superseded_on:{sup}"),
        });
    }
    if let RegimeState::NotPublished { ref reason } = row.state {
        return Err(EconError::NotPublished {
            regime_id: row.id.clone(),
            reason: reason.clone(),
        });
    }
    let pt = if allow_historical {
        deck.price_historical(commodity)?
    } else {
        deck.price(commodity)?
    };
    let breakdown = compute_fiscal_breakdown(
        payable_units,
        pt.price,
        operating_cost,
        row.royalty,
        row.tax_terms,
        row.super_profits,
    )?;

    Ok(RegimeFiscalEvaluation {
        regime_id: row.id.clone(),
        instrument: row.instrument.clone(),
        as_of_date: row.as_of_date.clone(),
        state: row.state.clone(),
        deck_id: deck.deck_id.clone(),
        deck_version: deck.version,
        super_profits_triggered: breakdown.super_profits_tax_paid.minor > 0,
        breakdown,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    use crate::price_deck::{DeckMixAudit, PricePoint, PriceUnit, audit_royalty_deck_mix};

    fn gold_deck(id: &str, usd_per_oz: i128) -> PriceDeck {
        PriceDeck::build(
            id,
            1,
            "2026-08-10",
            Currency::USD,
            vec![
                PricePoint::new(
                    Commodity::Au,
                    PriceUnit::PerTroyOz,
                    Money::from_major(Currency::USD, usd_per_oz),
                )
                .unwrap(),
            ],
            None,
        )
        .expect("build gold deck")
    }

    #[test]
    fn test_country_regimes_kibali_loulo_restricted_not_published_and_deck_mix_flag() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|p| p.parent())
            .expect("workspace root")
            .to_path_buf();
        let text = std::fs::read_to_string(root.join("registries/country_regimes.toml"))
            .expect("read country_regimes.toml");
        let rows = parse_country_regime_registry(&text).expect("parse country_regimes.toml");
        assert!(rows.len() >= 7);

        let by_id = |id: &str| {
            rows.iter()
                .find(|r| r.id == id)
                .unwrap_or_else(|| panic!("missing regime row {id}"))
        };

        // 1. DRC / Kibali reproduction (FACT E242–E245, E443–E449):
        //    - At $1,500/oz reserve deck: 5.7% royalty = $85.50/oz; super-profits tax = $0 (below $2,000 trigger).
        //    - At $3,000/oz spot deck: 5.7% royalty = $171.00/oz; super-profits tax TRIGGERED =
        //      50% * ((3000 - 171) - (2000 - 114)) = 50% * $943 = $471.50/oz!
        let kibali = by_id("drc_kibali_2026");
        assert_eq!(kibali.state_equity_fraction, ExactQty::new(1, 10).unwrap());
        assert_eq!(
            kibali.renewal_equity_step_fraction,
            ExactQty::new(5, 100).unwrap()
        );
        let one_oz = ExactQty::from_integer(1);
        let opex_per_oz = Money::from_major(Currency::USD, 1_000);

        let k_1500 = evaluate_regime_fiscal_terms(
            kibali,
            &gold_deck("KIB-1500", 1_500),
            Commodity::Au,
            one_oz,
            opex_per_oz,
            false,
        )
        .expect("kibali at $1500");
        assert_eq!(k_1500.breakdown.royalty_paid.minor, 855_000); // $85.5000/oz
        assert!(!k_1500.super_profits_triggered);
        assert_eq!(k_1500.breakdown.super_profits_tax_paid.minor, 0);

        let k_3000 = evaluate_regime_fiscal_terms(
            kibali,
            &gold_deck("KIB-3000", 3_000),
            Commodity::Au,
            one_oz,
            opex_per_oz,
            false,
        )
        .expect("kibali at $3000");
        assert_eq!(k_3000.breakdown.royalty_paid.minor, 1_710_000); // $171.0000/oz
        assert!(k_3000.super_profits_triggered);
        assert_eq!(k_3000.breakdown.super_profits_tax_paid.minor, 4_715_000); // $471.5000/oz

        // 2. Flag the Kibali $85.5/oz deck mix (FACT E244):
        //    Kibali's COG table charges $85.50/oz royalty at a $2,000/oz price,
        //    which is 5.7% of $1,500/oz instead of 5.7% of $2,000/oz ($114.00/oz).
        let audit = audit_royalty_deck_mix(
            Money::from_major(Currency::USD, 2_000),
            ExactQty::new(57, 1_000).unwrap(),
            Money::from_minor(Currency::USD, 855_000), // $85.50/oz
            Money::from_minor(Currency::USD, 100),     // $0.01 tolerance
        )
        .expect("deck mix audit");
        assert_eq!(
            audit,
            DeckMixAudit::DeckMixFlagged {
                stated_price: Money::from_major(Currency::USD, 2_000),
                stated_rate: ExactQty::new(57, 1_000).unwrap(),
                expected_royalty: Money::from_major(Currency::USD, 114),
                quoted_royalty: Money::from_minor(Currency::USD, 855_000),
                discrepancy: Money::from_minor(Currency::USD, -285_000), // -$28.50/oz
                implied_deck_price: Money::from_major(Currency::USD, 1_500),
            }
        );

        // 3. Mali / Loulo-Gounkoto reproduction (FACT E202, E246, E410–E424):
        //    - 2023 1991-Code row is superseded on 2025-11-24 -> refuses live eval with FMINE-REF-502,
        //      and reproduces 6% flat royalty in explicit historical mode.
        //    - 2026 Loi n°2023-040 row at $4,500/oz planning price reproduces 17.85% (~18%) = $803.25/oz.
        let loulo_old = by_id("mali_loulo_2023_superseded");
        let stale_err = evaluate_regime_fiscal_terms(
            loulo_old,
            &gold_deck("LOULO-4500", 4_500),
            Commodity::Au,
            one_oz,
            opex_per_oz,
            false,
        )
        .unwrap_err();
        assert_eq!(stale_err.to_refusal().code, "FMINE-REF-502");
        let hist = evaluate_regime_fiscal_terms(
            loulo_old,
            &gold_deck("LOULO-2000", 2_000),
            Commodity::Au,
            one_oz,
            opex_per_oz,
            true,
        )
        .expect("historical 1991 code");
        assert_eq!(
            hist.breakdown.effective_royalty_rate,
            ExactQty::new(6, 100).unwrap()
        );
        assert_eq!(
            hist.breakdown.royalty_paid,
            Money::from_major(Currency::USD, 120)
        );

        let loulo_2026 = by_id("mali_loulo_2026");
        let l_4500 = evaluate_regime_fiscal_terms(
            loulo_2026,
            &gold_deck("LOULO-4500", 4_500),
            Commodity::Au,
            one_oz,
            opex_per_oz,
            false,
        )
        .expect("loulo 2026 at $4500");
        assert_eq!(
            l_4500.breakdown.effective_royalty_rate,
            ExactQty::new(1_785, 10_000).unwrap()
        ); // 17.85%
        assert_eq!(l_4500.breakdown.royalty_paid.minor, 8_032_500); // $803.2500/oz

        // 4. NOT_PUBLISHED refuses with FMINE-REF-201 (never silent zero fill; RISK-26).
        let loulo_np = by_id("mali_loulo_2025_not_published");
        let np_err = evaluate_regime_fiscal_terms(
            loulo_np,
            &gold_deck("LOULO-3000", 3_000),
            Commodity::Au,
            one_oz,
            opex_per_oz,
            false,
        )
        .unwrap_err();
        assert_eq!(np_err.to_refusal().code, "FMINE-REF-201");

        // 5. Restricted operational states preserve distinct causes (stockpile-only vs reagent).
        let cobre = by_id("panama_cobre_panama_2026");
        assert_eq!(
            cobre.state,
            RegimeState::Restricted {
                cause: RestrictionCause::StockpileOnly
            }
        );
        let veladero = by_id("argentina_veladero_2017_restricted");
        assert_eq!(
            veladero.state,
            RegimeState::Restricted {
                cause: RestrictionCause::Reagent
            }
        );
    }
}
