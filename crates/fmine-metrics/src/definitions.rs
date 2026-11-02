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

pub mod claims;
pub mod geo;
pub mod worksheet;

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

include!("../../xtask/testdata/metrics_definitions_unit_tests.rs");
