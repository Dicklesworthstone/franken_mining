//! DESIGN (BD-060/061): conditional project classification under the selected registered cost rule.
//! The worksheet declares project facts; classification neither authenticates them nor classifies accounts.
//! Determinism: Exact

use std::collections::BTreeSet;

use fmine_core::{Encoder, ObjectId, blake3_hash};
use fmine_evidence::{Refusal, RefusalCode};
use fmine_registry::metrics::{MetricRegistry, ProjectTest};
use fmine_units::{Currency, ExactQty, Money};

use super::declared::{js, qty};
use super::{Project, is_non_sustaining};

type Result<T> = std::result::Result<T, Refusal>;

/// The four project cost categories that have registered sustaining and non-sustaining WGC lines.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CostKind {
    ExplorationAndStudy,
    CapitalExploration,
    StrippingAndDevelopment,
    CapitalExpenditure,
}

impl CostKind {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "exploration_and_study" => Some(Self::ExplorationAndStudy),
            "capital_exploration" => Some(Self::CapitalExploration),
            "capitalised_stripping_and_underground_development" => {
                Some(Self::StrippingAndDevelopment)
            }
            "capital_expenditure" => Some(Self::CapitalExpenditure),
            _ => None,
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ExplorationAndStudy => "exploration_and_study",
            Self::CapitalExploration => "capital_exploration",
            Self::StrippingAndDevelopment => "capitalised_stripping_and_underground_development",
            Self::CapitalExpenditure => "capital_expenditure",
        }
    }

    pub const fn item(self, non_sustaining: bool) -> &'static str {
        match (self, non_sustaining) {
            (Self::ExplorationAndStudy, false) => "exploration_and_study_sustaining",
            (Self::ExplorationAndStudy, true) => "exploration_and_study_non_sustaining",
            (Self::CapitalExploration, false) => "capital_exploration_sustaining",
            (Self::CapitalExploration, true) => "capital_exploration_non_sustaining",
            (Self::StrippingAndDevelopment, false) => {
                "capitalised_stripping_and_underground_development_sustaining"
            }
            (Self::StrippingAndDevelopment, true) => {
                "capitalised_stripping_and_underground_development_non_sustaining"
            }
            (Self::CapitalExpenditure, false) => "capital_expenditure_sustaining",
            (Self::CapitalExpenditure, true) => "capital_expenditure_non_sustaining",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeclaredProject {
    pub id: String,
    pub cost_kind: CostKind,
    pub amount: Money,
    pub classification_rule: String,
    pub rule_tag: String,
    pub assumptions: Project,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Classification {
    pub input_claim_id: ObjectId,
    pub project: DeclaredProject,
    pub thresholds: ProjectTest,
    /// Existing operation: material uplift. Extension: minimum duration.
    pub first_condition: bool,
    /// Existing operation: cash later than the window or never. Extension: ore phase strictly exceeds threshold.
    pub second_condition: bool,
    pub non_sustaining: bool,
    pub item: &'static str,
}

fn bad(code: RefusalCode, text: &str) -> Refusal {
    Refusal::new(code, format!("cost project: {text}"))
}

fn test<'a>(registry: &'a MetricRegistry, id: &str) -> Result<&'a ProjectTest> {
    let mut matches = registry.project_tests.iter().filter(|t| t.definition == id);
    let result = matches.next().ok_or_else(|| {
        bad(
            RefusalCode::NoClaimInDomain201,
            "classification rule is not registered",
        )
    })?;
    if matches.next().is_some() {
        return Err(bad(
            RefusalCode::BasisMismatch105,
            "classification rule is ambiguous",
        ));
    }
    let uplift = ExactQty::from_decimal_str(&result.material_uplift_percent)
        .map_err(|_| bad(RefusalCode::BasisMismatch105, "invalid uplift threshold"))?;
    if uplift.signum() <= 0
        || result.net_cash_months < 0
        || result.extension_months < 0
        || result.ore_phase_years <= 0
    {
        return Err(bad(
            RefusalCode::BasisMismatch105,
            "classification thresholds are outside their domain",
        ));
    }
    Ok(result)
}

/// Bind the exact thresholds independently of the selected metric's included/excluded line rules.
pub fn rule_tag(registry: &MetricRegistry, id: &str) -> Result<String> {
    let t = test(registry, id)?;
    let mut e = Encoder::new_frame(*b"CPRJ", 1);
    e.put_str(&super::claims::definition_tag(registry, id)?);
    e.put_str(&t.definition);
    e.put_str(&t.material_uplift_percent);
    for n in [t.net_cash_months, t.extension_months, t.ore_phase_years] {
        e.put_i128(i128::from(n));
    }
    Ok(blake3_hash(&e.finish()).to_hex())
}

/// Recompute the classification at admission; neither a supplied item nor a cached verdict is trusted.
pub fn classify(
    registry: &MetricRegistry,
    definition: &str,
    project: &DeclaredProject,
    input_claim_id: ObjectId,
) -> Result<Classification> {
    if project.id.is_empty()
        || project.id.len() > 128
        || !project
            .id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b))
    {
        return Err(bad(
            RefusalCode::BasisMismatch105,
            "project id must be a unique bounded identifier",
        ));
    }
    if project.amount.ccy != Currency::USD || project.amount.minor < 0 {
        return Err(bad(
            RefusalCode::OutsidePhysicalDomain109,
            "project expenditure must be nonnegative USD; adjustments require a separate direct line",
        ));
    }
    let mut ancestors = BTreeSet::new();
    let mut pending = vec![definition];
    while let Some(id) = pending.pop() {
        if ancestors.insert(id) {
            let d = registry
                .definition(id)
                .ok_or_else(|| bad(RefusalCode::NoClaimInDomain201, "missing cost definition"))?;
            pending.extend(d.extends.iter().map(String::as_str));
        }
    }
    if !ancestors.contains(project.classification_rule.as_str()) {
        return Err(bad(
            RefusalCode::BasisMismatch105,
            "selected cost definition does not inherit the declared classification rule",
        ));
    }
    let thresholds = test(registry, &project.classification_rule)?;
    if rule_tag(registry, &project.classification_rule)? != project.rule_tag {
        return Err(bad(
            RefusalCode::FrozenRecordMutation507,
            "classification thresholds differ from the bound project Claim",
        ));
    }
    let arithmetic = || {
        bad(
            RefusalCode::NumericalCertificateFailed207,
            "project comparison overflow",
        )
    };
    let (first_condition, second_condition) = match project.assumptions {
        Project::AtExistingOperation {
            uplift_percent,
            months_to_net_cash,
        } => {
            if months_to_net_cash.is_some_and(|months| months < 0) {
                return Err(bad(
                    RefusalCode::OutsidePhysicalDomain109,
                    "months to incremental net cash must be nonnegative or explicitly never",
                ));
            }
            let threshold = ExactQty::from_decimal_str(&thresholds.material_uplift_percent)
                .map_err(|_| arithmetic())?;
            (
                uplift_percent
                    .checked_cmp(threshold)
                    .map_err(|_| arithmetic())?
                    .is_ge(),
                months_to_net_cash.is_none_or(|months| months > thresholds.net_cash_months),
            )
        }
        Project::Extension {
            duration_months,
            ore_phase_years,
        } => {
            if duration_months < 0 || ore_phase_years.signum() < 0 {
                return Err(bad(
                    RefusalCode::OutsidePhysicalDomain109,
                    "extension duration and ore phase must be nonnegative",
                ));
            }
            (
                duration_months >= thresholds.extension_months,
                ore_phase_years
                    .checked_cmp(ExactQty::from_integer(i128::from(
                        thresholds.ore_phase_years,
                    )))
                    .map_err(|_| arithmetic())?
                    .is_gt(),
            )
        }
    };
    let non_sustaining =
        is_non_sustaining(thresholds, project.assumptions).map_err(|_| arithmetic())?;
    if non_sustaining != (first_condition && second_condition) {
        return Err(arithmetic());
    }
    Ok(Classification {
        input_claim_id,
        project: project.clone(),
        thresholds: thresholds.clone(),
        first_condition,
        second_condition,
        non_sustaining,
        item: project.cost_kind.item(non_sustaining),
    })
}

impl DeclaredProject {
    pub(super) fn json_fields(&self) -> String {
        let assumptions = match self.assumptions {
            Project::AtExistingOperation {
                uplift_percent,
                months_to_net_cash,
            } => format!(
                "\"project_kind\":\"existing_operation\",\"uplift_percent\":{},\"months_to_net_cash\":{}",
                js(&qty(uplift_percent)),
                months_to_net_cash.map_or_else(|| js("never"), |n| js(&n.to_string()))
            ),
            Project::Extension {
                duration_months,
                ore_phase_years,
            } => format!(
                "\"project_kind\":\"extension\",\"duration_months\":{},\"ore_phase_years\":{}",
                js(&duration_months.to_string()),
                js(&qty(ore_phase_years))
            ),
        };
        format!(
            "\"id\":{},\"cost_kind\":{},\"currency\":\"{:?}\",\"amount_minor\":{},\"scale\":10000,\"classification_rule\":{},\"rule_tag\":{},{}",
            js(&self.id),
            js(self.cost_kind.as_str()),
            self.amount.ccy,
            js(&self.amount.minor.to_string()),
            js(&self.classification_rule),
            js(&self.rule_tag),
            assumptions
        )
    }
}

impl Classification {
    pub const fn condition_names(&self) -> (&'static str, &'static str) {
        match self.project.assumptions {
            Project::AtExistingOperation { .. } => (
                "material_uplift_at_least_threshold",
                "incremental_cash_after_window_or_never",
            ),
            Project::Extension { .. } => (
                "duration_at_least_threshold",
                "ore_phase_strictly_above_threshold",
            ),
        }
    }

    pub fn to_json(&self) -> String {
        let t = &self.thresholds;
        let (first, second) = self.condition_names();
        format!(
            "{{\"input_claim_id\":{},\"project\":{{{}}},\"thresholds\":{{\"material_uplift_percent\":{},\"net_cash_months\":{},\"extension_months\":{},\"ore_phase_years\":{}}},\"conditions\":{{{}:{},{}:{}}},\"classification\":{},\"item\":{}}}",
            js(&self.input_claim_id.to_string()),
            self.project.json_fields(),
            js(&t.material_uplift_percent),
            t.net_cash_months,
            t.extension_months,
            t.ore_phase_years,
            js(first),
            self.first_condition,
            js(second),
            self.second_condition,
            js(if self.non_sustaining {
                "non_sustaining"
            } else {
                "sustaining"
            }),
            js(self.item)
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn project(assumptions: Project) -> (MetricRegistry, DeclaredProject, ObjectId) {
        let registry = super::super::bundled_registry().unwrap();
        let p = DeclaredProject {
            id: "declared-project".into(),
            cost_kind: CostKind::CapitalExpenditure,
            amount: Money::from_minor(Currency::USD, 4_000_000),
            classification_rule: "aisc@wgc-2018".into(),
            rule_tag: rule_tag(&registry, "aisc@wgc-2018").unwrap(),
            assumptions,
        };
        (
            registry,
            p,
            ObjectId::from_canonical_bytes(fmine_core::PrefixKind::Claim, b"invented-project"),
        )
    }

    #[test]
    fn registered_project_thresholds_preserve_inclusive_and_strict_boundaries() {
        for (uplift, months, expected) in [
            ("9.999", Some(13), false),
            ("10", Some(13), true),
            ("10.001", Some(12), false),
            ("10", None, true),
            ("-1", None, false),
            ("10", Some(0), false),
        ] {
            let (registry, p, id) = project(Project::AtExistingOperation {
                uplift_percent: ExactQty::from_decimal_str(uplift).unwrap(),
                months_to_net_cash: months,
            });
            let answer = classify(&registry, "aic@wgc-2018", &p, id).unwrap();
            assert_eq!(answer.non_sustaining, expected, "{uplift} {months:?}");
            assert_eq!(answer.first_condition && answer.second_condition, expected);
            assert_eq!(answer.project, p);
            assert_eq!(
                answer.item,
                if expected {
                    "capital_expenditure_non_sustaining"
                } else {
                    "capital_expenditure_sustaining"
                }
            );
        }
        for (duration, years, expected) in [
            (11, "6", false),
            (12, "5", false),
            (12, "5.0001", true),
            (13, "0", false),
        ] {
            let (registry, p, id) = project(Project::Extension {
                duration_months: duration,
                ore_phase_years: ExactQty::from_decimal_str(years).unwrap(),
            });
            let answer = classify(&registry, "aisc@wgc-2018", &p, id).unwrap();
            assert_eq!(answer.non_sustaining, expected, "{duration} {years}");
            assert_eq!(answer.thresholds.ore_phase_years, 5);
        }
    }

    #[test]
    fn changed_rules_invalid_domains_and_unrelated_definitions_refuse_classification() {
        let (mut registry, p, id) = project(Project::Extension {
            duration_months: 12,
            ore_phase_years: ExactQty::from_integer(6),
        });
        assert_eq!(
            classify(&registry, "aisc@wgc-2013", &p, id)
                .unwrap_err()
                .code,
            "FMINE-REF-105"
        );
        registry.project_tests[0].ore_phase_years = 7;
        assert_eq!(
            classify(&registry, "aisc@wgc-2018", &p, id)
                .unwrap_err()
                .code,
            "FMINE-REF-507"
        );
        registry.project_tests[0].ore_phase_years = 5;
        let mut changed = p.clone();
        changed.amount.minor = -1;
        assert_eq!(
            classify(&registry, "aisc@wgc-2018", &changed, id)
                .unwrap_err()
                .code,
            "FMINE-REF-109"
        );
        for assumptions in [
            Project::AtExistingOperation {
                uplift_percent: ExactQty::from_integer(10),
                months_to_net_cash: Some(-1),
            },
            Project::Extension {
                duration_months: -1,
                ore_phase_years: ExactQty::from_integer(6),
            },
            Project::Extension {
                duration_months: 12,
                ore_phase_years: ExactQty::from_integer(-1),
            },
        ] {
            changed = p.clone();
            changed.assumptions = assumptions;
            assert_eq!(
                classify(&registry, "aisc@wgc-2018", &changed, id)
                    .unwrap_err()
                    .code,
                "FMINE-REF-109"
            );
        }
        registry
            .project_tests
            .push(registry.project_tests[0].clone());
        assert_eq!(
            classify(&registry, "aisc@wgc-2018", &p, id)
                .unwrap_err()
                .code,
            "FMINE-REF-105"
        );
    }

    #[test]
    fn every_declared_project_cost_kind_has_an_explicit_aisc_exclusion_and_aic_inclusion() {
        let registry = super::super::bundled_registry().unwrap();
        for kind in [
            CostKind::ExplorationAndStudy,
            CostKind::CapitalExploration,
            CostKind::StrippingAndDevelopment,
            CostKind::CapitalExpenditure,
        ] {
            assert_eq!(CostKind::parse(kind.as_str()), Some(kind));
            assert!(
                registry
                    .lines_of("aisc@wgc-2018")
                    .contains_key(kind.item(false))
            );
            assert!(
                registry
                    .excluded_of("aisc@wgc-2018")
                    .contains(kind.item(true))
            );
            assert!(
                registry
                    .lines_of("aic@wgc-2018")
                    .contains_key(kind.item(true))
            );
            assert!(
                !registry
                    .excluded_of("aic@wgc-2018")
                    .contains(kind.item(true))
            );
        }
    }
}
