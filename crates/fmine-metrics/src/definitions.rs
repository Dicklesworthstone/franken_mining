//! Cost per ounce and project classification read from the metric registry (`BD-060`, plan §21.6).
//! Determinism: DeterministicPerIsa (exact rationals throughout; `PriceDeck` hands floats on to `compute_geo`).
//!
//! `registries/metrics.toml` decides what counts: a line it does not name is refused, so a new kind of cost cannot
//! fall out of a metric unseen. Every result names the definition it used, and whether that definition is superseded.
//! The registry says what the rules are, not that any amount is right: these functions take the amounts as given.

use std::fmt;

use fmine_registry::RegistryError;
use fmine_registry::metrics::{CreditBasis, LineKind, MetricFamily, MetricRegistry, ProjectTest};
use fmine_units::{ExactQty, UnitsError};

/// The registry as committed.
const METRICS_TOML: &str = include_str!("../../../registries/metrics.toml");

/// Parses the registry as committed.
pub fn bundled_registry() -> Result<MetricRegistry, RegistryError> {
    MetricRegistry::parse(METRICS_TOML)
}

/// One cost amount, under a registered item name. A credit is given as a positive amount.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CostLine {
    pub item: String,
    pub usd: ExactQty,
}

/// Why a metric could not be computed.
#[derive(Debug, Clone, PartialEq)]
pub enum MetricFailure {
    UnknownDefinition(String),
    /// The definition exists and is not a cost per ounce.
    NotACostDefinition(String),
    /// The line is neither counted nor excluded by this definition.
    UnregisteredItem {
        definition: String,
        item: String,
    },
    NonPositiveOunces,
    /// A co-product definition needs the issuer's stated gold share, in (0, 1].
    AllocationRequired {
        definition: String,
    },
    /// A share was given to a definition that allocates nothing.
    AllocationNotApplicable {
        definition: String,
    },
    Exact(UnitsError),
}

impl fmt::Display for MetricFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownDefinition(id) => write!(f, "unregistered metric definition {id}"),
            Self::NotACostDefinition(id) => write!(f, "{id} is not a cost per ounce"),
            Self::UnregisteredItem { definition, item } => {
                write!(
                    f,
                    "{definition} neither counts nor excludes the line {item}"
                )
            }
            Self::NonPositiveOunces => write!(f, "ounces sold must be positive"),
            Self::AllocationRequired { definition } => {
                write!(
                    f,
                    "{definition} needs the issuer's stated gold share, in (0, 1]"
                )
            }
            Self::AllocationNotApplicable { definition } => {
                write!(f, "{definition} allocates nothing, so it takes no share")
            }
            Self::Exact(e) => write!(f, "exact arithmetic failed: {e:?}"),
        }
    }
}

impl std::error::Error for MetricFailure {}

impl From<UnitsError> for MetricFailure {
    fn from(e: UnitsError) -> Self {
        Self::Exact(e)
    }
}

/// A cost per gold ounce and how it was reached.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AllInCost {
    pub definition: String,
    /// The definition that replaces this one, if any (`INV-013`).
    pub superseded_by: Option<String>,
    pub usd_per_oz: ExactQty,
    /// Items counted, in input order.
    pub counted: Vec<String>,
    /// Items the definition left out, in input order.
    pub left_out: Vec<String>,
}

/// `Σ registered lines / ounces sold` under one registered definition.
///
/// `gold_share` is the issuer's stated allocation, and only a co-product definition takes it: there the credit lines
/// are left out and the total is multiplied by the share (WGC FAQ 13).
pub fn all_in_cost_per_oz(
    reg: &MetricRegistry,
    id: &str,
    lines: &[CostLine],
    ounces_sold: ExactQty,
    gold_share: Option<ExactQty>,
) -> Result<AllInCost, MetricFailure> {
    let def = reg
        .definition(id)
        .ok_or_else(|| MetricFailure::UnknownDefinition(id.to_string()))?;
    if def.family != MetricFamily::AllInCost {
        return Err(MetricFailure::NotACostDefinition(id.to_string()));
    }
    if ounces_sold.signum() <= 0 {
        return Err(MetricFailure::NonPositiveOunces);
    }
    let co_product = def.credit_basis == CreditBasis::CoProduct;
    let share = match (co_product, gold_share) {
        (true, Some(s)) if s.signum() > 0 && s.checked_cmp(ExactQty::from_integer(1))?.is_le() => s,
        (true, _) => {
            return Err(MetricFailure::AllocationRequired {
                definition: id.to_string(),
            });
        }
        (false, Some(_)) => {
            return Err(MetricFailure::AllocationNotApplicable {
                definition: id.to_string(),
            });
        }
        (false, None) => ExactQty::from_integer(1),
    };
    let (registered, excluded) = (reg.lines_of(id), reg.excluded_of(id));
    let mut total = ExactQty::from_integer(0);
    let (mut counted, mut left_out) = (Vec::new(), Vec::new());
    for line in lines {
        match registered.get(line.item.as_str()).map(|l| l.kind) {
            Some(LineKind::Cost) => {
                total = total.checked_add(line.usd)?;
                counted.push(line.item.clone());
            }
            Some(LineKind::Credit) if co_product => left_out.push(line.item.clone()),
            Some(LineKind::Credit) => {
                total = total.checked_sub(line.usd)?;
                counted.push(line.item.clone());
            }
            None if excluded.contains(line.item.as_str()) => left_out.push(line.item.clone()),
            None => {
                return Err(MetricFailure::UnregisteredItem {
                    definition: id.to_string(),
                    item: line.item.clone(),
                });
            }
        }
    }
    Ok(AllInCost {
        definition: id.to_string(),
        superseded_by: reg.superseded_by(id).map(str::to_string),
        usd_per_oz: total.checked_mul(share)?.checked_div(ounces_sold)?,
        counted,
        left_out,
    })
}

/// What a project is, for the sustaining versus non-sustaining test.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Project {
    /// A project at an existing operation. `uplift_percent` is the largest rise it brings to annual or life-of-mine
    /// production, NPV or reserves at the operation level. `months_to_net_cash` is when it first brings incremental
    /// net cash inflows after commencing, and `None` means it never does.
    AtExistingOperation {
        uplift_percent: ExactQty,
        months_to_net_cash: Option<i64>,
    },
    /// A pushback, a layback or an extension of an underground footprint.
    Extension {
        duration_months: i64,
        ore_phase_years: ExactQty,
    },
}

/// The WGC test, thresholds from the registry (FAQ 7 and 10): non-sustaining at a material uplift with no net cash
/// inside the window, and an extension only when it runs at least that long and its ore phase exceeds the threshold.
/// A new operation is non-sustaining outright, so it needs no test.
pub fn is_non_sustaining(test: &ProjectTest, project: Project) -> Result<bool, MetricFailure> {
    Ok(match project {
        Project::AtExistingOperation {
            uplift_percent,
            months_to_net_cash,
        } => {
            let threshold = ExactQty::from_decimal_str(&test.material_uplift_percent)?;
            uplift_percent.checked_cmp(threshold)?.is_ge()
                && months_to_net_cash.is_none_or(|m| m > test.net_cash_months)
        }
        Project::Extension {
            duration_months,
            ore_phase_years,
        } => {
            let threshold = ExactQty::from_integer(i128::from(test.ore_phase_years));
            duration_months >= test.extension_months
                && ore_phase_years.checked_cmp(threshold)?.is_gt()
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{PriceDeck, compute_geo};

    const AISC: &str = "aisc@wgc-2018";

    fn usd(text: &str) -> ExactQty {
        ExactQty::from_decimal_str(text).expect("decimal")
    }

    fn line(item: &str, amount: &str) -> CostLine {
        CostLine {
            item: item.to_string(),
            usd: usd(amount),
        }
    }

    fn registry() -> MetricRegistry {
        bundled_registry().expect("the committed registry parses")
    }

    fn cost(id: &str, lines: &[CostLine]) -> Result<AllInCost, MetricFailure> {
        all_in_cost_per_oz(&registry(), id, lines, usd("100"), None)
    }

    /// Synthetic amounts, so the arithmetic can be read off: (800 + 300 + 100 − 200) / 100 oz.
    #[test]
    fn test_aisc_counts_its_lines_and_names_what_it_left_out() {
        let got = cost(
            AISC,
            &[
                line("on_site_mining_costs", "800"),
                line("corporate_general_and_administrative", "300"),
                line("lease_payments", "100"),
                line("by_product_credits", "200"),
                line("income_tax", "999"),
            ],
        )
        .expect("computes");
        assert_eq!(got.usd_per_oz, usd("10"));
        assert_eq!(got.left_out, vec!["income_tax"]);
        assert_eq!(got.counted.len(), 4);
        assert_eq!(got.definition, AISC);
        assert_eq!(got.superseded_by, None);
    }

    /// AIC is AISC plus the non-sustaining lines. The same line is refused under AISC, because AISC neither counts
    /// nor excludes it: growth capital cannot slip through either metric unseen.
    #[test]
    fn test_non_sustaining_capital_is_in_aic_and_refused_in_aisc() {
        let lines = [
            line("on_site_mining_costs", "800"),
            line("capital_expenditure_non_sustaining", "400"),
        ];
        let aic = cost("aic@wgc-2018", &lines).expect("computes");
        assert_eq!(aic.usd_per_oz, usd("12"));
        assert_eq!(
            cost(AISC, &lines),
            Err(MetricFailure::UnregisteredItem {
                definition: AISC.to_string(),
                item: "capital_expenditure_non_sustaining".to_string(),
            })
        );
    }

    /// One item, two versions: the same inputs differ by the lease line, and the older result says it is superseded.
    #[test]
    fn test_a_definition_change_moves_the_result_and_is_named() {
        let lines = [
            line("on_site_mining_costs", "800"),
            line("lease_payments", "100"),
        ];
        let older = cost("aisc@wgc-2013", &lines).expect("computes");
        let newer = cost(AISC, &lines).expect("computes");
        assert_eq!(older.usd_per_oz, usd("8"));
        assert_eq!(older.left_out, vec!["lease_payments"]);
        assert_eq!(older.superseded_by.as_deref(), Some(AISC));
        assert_eq!(newer.usd_per_oz, usd("9"));
    }

    #[test]
    fn test_co_product_needs_a_share_and_drops_credits() {
        let id = "aisc.co_product@wgc-2018";
        let lines = [
            line("on_site_mining_costs", "1000"),
            line("by_product_credits", "300"),
        ];
        let reg = registry();
        let got =
            all_in_cost_per_oz(&reg, id, &lines, usd("100"), Some(usd("0.75"))).expect("computes");
        assert_eq!(got.usd_per_oz, usd("7.5"));
        assert_eq!(got.left_out, vec!["by_product_credits"]);
        for share in [None, Some(usd("0")), Some(usd("1.01"))] {
            assert!(matches!(
                all_in_cost_per_oz(&reg, id, &lines, usd("100"), share),
                Err(MetricFailure::AllocationRequired { .. })
            ));
        }
        assert!(matches!(
            all_in_cost_per_oz(&reg, AISC, &lines, usd("100"), Some(usd("0.75"))),
            Err(MetricFailure::AllocationNotApplicable { .. })
        ));
    }

    #[test]
    fn test_refusals() {
        let lines = [line("on_site_mining_costs", "1")];
        assert_eq!(
            cost("aisc@wgc-1999", &lines),
            Err(MetricFailure::UnknownDefinition(
                "aisc@wgc-1999".to_string()
            ))
        );
        let rr = "reserve_replacement.geo@barrick-2026-circular";
        assert_eq!(
            cost(rr, &lines),
            Err(MetricFailure::NotACostDefinition(rr.to_string()))
        );
        assert_eq!(
            all_in_cost_per_oz(&registry(), AISC, &lines, usd("0"), None),
            Err(MetricFailure::NonPositiveOunces)
        );
    }

    fn major(uplift: &str, months: Option<i64>) -> bool {
        let reg = registry();
        is_non_sustaining(
            &reg.project_tests[0],
            Project::AtExistingOperation {
                uplift_percent: usd(uplift),
                months_to_net_cash: months,
            },
        )
        .expect("classifies")
    }

    fn extension(months: i64, years: &str) -> bool {
        let reg = registry();
        is_non_sustaining(
            &reg.project_tests[0],
            Project::Extension {
                duration_months: months,
                ore_phase_years: usd(years),
            },
        )
        .expect("classifies")
    }

    /// FAQ 7: "at least a 10% increase" and no net cash "within 12 months". Both conditions, and both edges.
    #[test]
    fn test_major_project_needs_both_conditions() {
        assert!(major("10", None));
        assert!(major("10", Some(13)));
        assert!(!major("10", Some(12)));
        assert!(!major("9.99", None));
        assert!(!major("50", Some(6)));
    }

    /// FAQ 10: sustaining unless at least 12 months and more than five years of ore.
    #[test]
    fn test_extension_needs_both_conditions() {
        assert!(extension(12, "5.1"));
        assert!(!extension(12, "5"));
        assert!(!extension(11, "10"));
        assert!(extension(36, "12"));
    }

    /// The Circular's decks come from the registry (E606), and a year it does not print stays unregistered.
    #[test]
    fn test_price_decks_read_from_the_registry() {
        let deck = PriceDeck::barrick_reserve(2025).expect("registered");
        assert_eq!(deck.deck_id, "BARRICK-RESERVE-2025");
        assert_eq!(
            (deck.gold_usd_per_oz, deck.copper_usd_per_lb),
            (1_500.0, 3.25)
        );
        assert!(PriceDeck::barrick_reserve(2022).is_none());
        let geo = compute_geo(1_000.0, 1.0, &deck);
        assert!((geo.copper_geo_koz - 2.166_666_666_666_667).abs() < 1e-12);
    }
}
