//! Metric definitions as versioned data (`BD-060`, plan §21.6): `registries/metrics.toml`.
//!
//! This is deliberately outside `Registries::REQUIRED_FILES`: those files feed the manifest digest, and adding a
//! definition should not move a state root. `xtask registry-check` validates it instead.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use crate::RegistryError;
use crate::toml_subset::{TomlRow, TomlValue, parse_subset};

/// Schema id `registries/metrics.toml` must declare.
pub const METRICS_SCHEMA: &str = "fmine.metrics.v1";

const NAME: &str = "metrics";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MetricFamily {
    AllInCost,
    Geo,
    ReserveReplacement,
}

/// How a cost definition treats the other metals.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CreditBasis {
    /// Credits reduce the cost.
    ByProduct,
    /// Credits are left out and the cost is allocated by the issuer's stated share.
    CoProduct,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineKind {
    Cost,
    Credit,
}

/// One line a cost definition counts. `letter` is its label in the source's table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MetricLine {
    pub item: String,
    pub letter: String,
    pub kind: LineKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MetricDefinition {
    /// `key@version`.
    pub id: String,
    pub family: MetricFamily,
    pub unit: String,
    pub formula: String,
    pub source: String,
    pub evidence: String,
    pub credit_basis: CreditBasis,
    pub extends: Vec<String>,
    pub supersedes: Option<String>,
}

/// The sustaining versus non-sustaining test, thresholds as data (WGC FAQ 7 and 10).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectTest {
    pub definition: String,
    pub material_uplift_percent: String,
    pub net_cash_months: i64,
    pub extension_months: i64,
    pub ore_phase_years: i64,
}

/// One year's fixed prices for a gold-equivalent definition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeoDeck {
    pub definition: String,
    pub year: i64,
    pub gold_usd_per_oz: String,
    pub copper_usd_per_lb: String,
}

/// The parsed, cross-checked registry.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MetricRegistry {
    pub definitions: Vec<MetricDefinition>,
    pub project_tests: Vec<ProjectTest>,
    pub geo_decks: Vec<GeoDeck>,
    lines: BTreeMap<String, Vec<MetricLine>>,
    excluded: BTreeMap<String, Vec<String>>,
}

fn schema_err(detail: String) -> RegistryError {
    RegistryError::SchemaViolation {
        registry: NAME.to_string(),
        detail,
    }
}

fn text<'a>(row: &'a TomlRow, key: &str) -> Result<&'a str, RegistryError> {
    match row.get(key).map(|f| &f.value) {
        Some(TomlValue::Str(s)) if !s.trim().is_empty() => Ok(s),
        _ => Err(schema_err(format!(
            "line {}: needs the text field {key}",
            row.line
        ))),
    }
}

fn int(row: &TomlRow, key: &str) -> Result<i64, RegistryError> {
    match row.get(key).map(|f| &f.value) {
        Some(TomlValue::Int(n)) => Ok(*n),
        _ => Err(schema_err(format!(
            "line {}: needs the integer field {key}",
            row.line
        ))),
    }
}

fn decimal(row: &TomlRow, key: &str) -> Result<String, RegistryError> {
    let s = text(row, key)?;
    if s.split('.').count() <= 2 && s.chars().all(|c| c.is_ascii_digit() || c == '.') {
        Ok(s.to_string())
    } else {
        Err(schema_err(format!(
            "line {}: {key} is not a plain decimal",
            row.line
        )))
    }
}

fn definition_from(row: &TomlRow) -> Result<MetricDefinition, RegistryError> {
    let id = text(row, "id")?.to_string();
    if id.split('@').filter(|p| !p.is_empty()).count() != 2 {
        return Err(schema_err(format!(
            "line {}: id {id} is not key@version",
            row.line
        )));
    }
    let family = match text(row, "family")? {
        "all_in_cost" => MetricFamily::AllInCost,
        "geo" => MetricFamily::Geo,
        "reserve_replacement" => MetricFamily::ReserveReplacement,
        other => return Err(schema_err(format!("{id}: unknown family {other}"))),
    };
    let evidence = text(row, "evidence")?.to_string();
    let digits = evidence.strip_prefix('E').unwrap_or("");
    if digits.is_empty() || !digits.chars().all(|c| c.is_ascii_digit()) {
        return Err(schema_err(format!(
            "{id}: evidence {evidence} is not an E id"
        )));
    }
    let credit_basis = match row.get("credit_basis") {
        None => CreditBasis::ByProduct,
        Some(_) => match text(row, "credit_basis")? {
            "by_product" => CreditBasis::ByProduct,
            "co_product" => CreditBasis::CoProduct,
            other => return Err(schema_err(format!("{id}: unknown credit_basis {other}"))),
        },
    };
    Ok(MetricDefinition {
        family,
        unit: text(row, "unit")?.to_string(),
        formula: text(row, "formula")?.to_string(),
        source: text(row, "source")?.to_string(),
        evidence,
        credit_basis,
        extends: match row.get("extends") {
            None => Vec::new(),
            Some(_) => text(row, "extends")?
                .split_whitespace()
                .map(str::to_string)
                .collect(),
        },
        supersedes: match row.get("supersedes") {
            None => None,
            Some(_) => Some(text(row, "supersedes")?.to_string()),
        },
        id,
    })
}

impl MetricRegistry {
    /// Parses the registry and checks it: schema, one row per id, parents and superseded ids registered with no
    /// cycle, every line and exclusion on a registered definition, and no item both counted and excluded on one
    /// definition.
    pub fn parse(toml: &str) -> Result<Self, RegistryError> {
        let doc = parse_subset(toml).map_err(|e| schema_err(e.to_string()))?;
        match doc.top_get("schema").map(|f| &f.value) {
            Some(TomlValue::Str(s)) if s == METRICS_SCHEMA => {}
            _ => return Err(schema_err(format!("schema must be {METRICS_SCHEMA}"))),
        }
        let mut reg = Self::default();
        let mut ids = BTreeSet::new();
        for row in doc.rows_of("definition") {
            let def = definition_from(row)?;
            if !ids.insert(def.id.clone()) {
                return Err(RegistryError::DuplicateId {
                    registry: NAME.to_string(),
                    id: def.id,
                });
            }
            reg.definitions.push(def);
        }
        let known = |row: &TomlRow, key: &str| -> Result<String, RegistryError> {
            let id = text(row, key)?;
            if ids.contains(id) {
                Ok(id.to_string())
            } else {
                Err(schema_err(format!(
                    "line {}: unregistered {key} {id}",
                    row.line
                )))
            }
        };
        for row in doc.rows_of("line") {
            let kind = match row.get("kind") {
                None => LineKind::Cost,
                Some(_) => match text(row, "kind")? {
                    "cost" => LineKind::Cost,
                    "credit" => LineKind::Credit,
                    other => {
                        return Err(schema_err(format!(
                            "line {}: unknown kind {other}",
                            row.line
                        )));
                    }
                },
            };
            reg.lines
                .entry(known(row, "definition")?)
                .or_default()
                .push(MetricLine {
                    item: text(row, "item")?.to_string(),
                    letter: text(row, "letter")?.to_string(),
                    kind,
                });
        }
        for row in doc.rows_of("excluded") {
            reg.excluded
                .entry(known(row, "definition")?)
                .or_default()
                .push(text(row, "item")?.to_string());
        }
        for row in doc.rows_of("project_test") {
            reg.project_tests.push(ProjectTest {
                definition: known(row, "definition")?,
                material_uplift_percent: decimal(row, "material_uplift_percent")?,
                net_cash_months: int(row, "net_cash_months")?,
                extension_months: int(row, "extension_months")?,
                ore_phase_years: int(row, "ore_phase_years")?,
            });
        }
        for row in doc.rows_of("geo_deck") {
            reg.geo_decks.push(GeoDeck {
                definition: known(row, "definition")?,
                year: int(row, "year")?,
                gold_usd_per_oz: decimal(row, "gold_usd_per_oz")?,
                copper_usd_per_lb: decimal(row, "copper_usd_per_lb")?,
            });
        }
        reg.check_links(&ids)?;
        Ok(reg)
    }

    fn check_links(&self, ids: &BTreeSet<String>) -> Result<(), RegistryError> {
        for def in &self.definitions {
            for parent in def.extends.iter().chain(&def.supersedes) {
                if !ids.contains(parent) {
                    return Err(schema_err(format!(
                        "{}: unregistered parent {parent}",
                        def.id
                    )));
                }
            }
            let mut seen = BTreeSet::new();
            let mut stack = def.extends.clone();
            while let Some(id) = stack.pop() {
                if id == def.id {
                    return Err(schema_err(format!("{}: extends itself", def.id)));
                }
                if seen.insert(id.clone())
                    && let Some(parent) = self.definition(&id)
                {
                    stack.extend(parent.extends.iter().cloned());
                }
            }
            let mut items = BTreeSet::new();
            let own = self.lines.get(&def.id).into_iter().flatten();
            for line in own {
                if !items.insert(line.item.as_str()) {
                    return Err(schema_err(format!("{}: line {} twice", def.id, line.item)));
                }
            }
            for item in self.excluded.get(&def.id).into_iter().flatten() {
                if items.contains(item.as_str()) {
                    return Err(schema_err(format!(
                        "{}: {item} both counted and excluded",
                        def.id
                    )));
                }
            }
            if def.family == MetricFamily::AllInCost && self.lines_of(&def.id).is_empty() {
                return Err(schema_err(format!(
                    "{}: a cost definition with no lines",
                    def.id
                )));
            }
        }
        Ok(())
    }

    /// Reads `registries/metrics.toml` under `workspace_root`.
    pub fn load_from_root(workspace_root: &Path) -> Result<Self, RegistryError> {
        let path = workspace_root.join("registries").join("metrics.toml");
        let text = fs::read_to_string(&path).map_err(|e| RegistryError::Io {
            path: path.display().to_string(),
            detail: e.to_string(),
        })?;
        Self::parse(&text)
    }

    /// The definition with this id.
    #[must_use]
    pub fn definition(&self, id: &str) -> Option<&MetricDefinition> {
        self.definitions.iter().find(|d| d.id == id)
    }

    /// The definition that replaces `id`, if one does: a Claim on `id` is then stale (`INV-013`).
    #[must_use]
    pub fn superseded_by(&self, id: &str) -> Option<&str> {
        self.definitions
            .iter()
            .find(|d| d.supersedes.as_deref() == Some(id))
            .map(|d| d.id.as_str())
    }

    fn ancestry<'a>(&'a self, id: &'a str) -> Vec<&'a str> {
        let mut seen: Vec<&str> = Vec::new();
        let mut stack = vec![id];
        while let Some(at) = stack.pop() {
            if seen.contains(&at) {
                continue;
            }
            seen.push(at);
            if let Some(def) = self.definition(at) {
                stack.extend(def.extends.iter().map(String::as_str));
            }
        }
        seen
    }

    /// Every line `id` counts, its parents' included, by item.
    #[must_use]
    pub fn lines_of(&self, id: &str) -> BTreeMap<&str, &MetricLine> {
        self.ancestry(id)
            .into_iter()
            .filter_map(|at| self.lines.get(at))
            .flatten()
            .map(|l| (l.item.as_str(), l))
            .collect()
    }

    /// Every item `id` leaves out, its parents' included. A line in the family wins over an exclusion.
    #[must_use]
    pub fn excluded_of(&self, id: &str) -> BTreeSet<&str> {
        let lines = self.lines_of(id);
        self.ancestry(id)
            .into_iter()
            .filter_map(|at| self.excluded.get(at))
            .flatten()
            .map(String::as_str)
            .filter(|item| !lines.contains_key(item))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const REGISTRY: &str = include_str!("../../../registries/metrics.toml");
    const AISC_2013: &str = "aisc@wgc-2013";
    const AISC_2018: &str = "aisc@wgc-2018";
    const AIC_2013: &str = "aic@wgc-2013";
    const AIC_2018: &str = "aic@wgc-2018";

    fn registry() -> MetricRegistry {
        MetricRegistry::parse(REGISTRY).expect("registry parses")
    }

    /// `BD-060` done-when. The 2013 table has 17 lines to AISC and 7 more to AIC, (a) to (z) without the subtotal (l)
    /// and AISC (s); the 2018 FAQ adds leases. Every line is counted once, and each family is registered.
    #[test]
    fn test_metric_definition_registry_check() {
        let reg = registry();
        assert_eq!(reg.lines_of(AISC_2013).len(), 17);
        assert_eq!(reg.lines_of(AIC_2013).len(), 24);
        assert_eq!(reg.lines_of(AISC_2018).len(), 18);
        assert_eq!(reg.lines_of(AIC_2018).len(), 25);
        let letters: String = {
            let mut l: Vec<&str> = reg
                .lines_of(AIC_2013)
                .values()
                .map(|l| l.letter.as_str())
                .collect();
            l.sort_unstable();
            l.concat()
        };
        assert_eq!(letters, "abcdefghijkmnopqrtuvwxyz");
        assert_eq!(
            reg.lines_of(AISC_2013)["by_product_credits"].kind,
            LineKind::Credit
        );
        for id in [
            "aisc.co_product@wgc-2018",
            "geo.reserve_deck@barrick-2026-circular",
            "reserve_replacement.gold_oz@barrick-fy2025",
            "reserve_replacement.geo@barrick-2026-circular",
        ] {
            assert!(reg.definition(id).is_some(), "{id}");
        }
        assert_eq!(reg.geo_decks.len(), 3);
        assert_eq!(reg.project_tests.len(), 1);
    }

    /// The 2013 note left finance leases out and FAQ 19 brings them in, so one item changes side between versions.
    #[test]
    fn test_a_line_in_a_later_version_overrides_an_earlier_exclusion() {
        let reg = registry();
        assert!(reg.excluded_of(AISC_2013).contains("lease_payments"));
        assert!(!reg.lines_of(AISC_2013).contains_key("lease_payments"));
        assert!(!reg.excluded_of(AISC_2018).contains("lease_payments"));
        assert!(reg.lines_of(AIC_2018).contains_key("lease_payments"));
        assert!(!reg.excluded_of(AIC_2018).contains("lease_payments"));
        assert!(reg.excluded_of(AIC_2018).contains("income_tax"));
    }

    #[test]
    fn test_superseded_definitions_name_their_successor() {
        let reg = registry();
        assert_eq!(reg.superseded_by(AISC_2013), Some(AISC_2018));
        assert_eq!(reg.superseded_by(AIC_2013), Some(AIC_2018));
        assert_eq!(reg.superseded_by(AISC_2018), None);
    }

    fn with(extra: &str) -> Result<MetricRegistry, RegistryError> {
        MetricRegistry::parse(&format!("{REGISTRY}\n{extra}"))
    }

    fn violation(result: Result<MetricRegistry, RegistryError>, needle: &str) {
        match result {
            Err(RegistryError::SchemaViolation { detail, .. }) => {
                assert!(detail.contains(needle), "{detail}");
            }
            other => panic!("expected a schema violation on {needle}, got {other:?}"),
        }
    }

    const ROW: &str = "[[definition]]\nid = \"x@1\"\nfamily = \"geo\"\nunit = \"oz\"\nformula = \"f\"\n\
                       source = \"s\"\nevidence = \"E1\"\n";

    #[test]
    fn test_registry_refusals() {
        assert!(with(ROW).is_ok());
        assert!(matches!(
            with(&format!("{ROW}{ROW}")),
            Err(RegistryError::DuplicateId { .. })
        ));
        violation(MetricRegistry::parse(ROW), "schema must be");
        violation(with(&ROW.replace("x@1", "x")), "key@version");
        violation(with(&ROW.replace("E1", "e1")), "not an E id");
        violation(with(&ROW.replace("geo", "ratio")), "unknown family");
        violation(
            with(&format!("{ROW}extends = \"nope@1\"\n")),
            "unregistered parent",
        );
        violation(
            with(&format!("{ROW}supersedes = \"nope@1\"\n")),
            "unregistered parent",
        );
        violation(with(&format!("{ROW}extends = \"x@1\"\n")), "extends itself");
        violation(
            with("[[line]]\ndefinition = \"nope@1\"\nletter = \"a\"\nitem = \"i\"\n"),
            "unregistered definition",
        );
        violation(
            with(
                "[[line]]\ndefinition = \"aisc@wgc-2018\"\nletter = \"a\"\nitem = \"i\"\nkind = \"debit\"\n",
            ),
            "unknown kind",
        );
        violation(
            with(
                "[[line]]\ndefinition = \"aisc@wgc-2013\"\nletter = \"z\"\nitem = \"on_site_mining_costs\"\n",
            ),
            "twice",
        );
        violation(
            with("[[excluded]]\ndefinition = \"aisc@wgc-2013\"\nitem = \"on_site_mining_costs\"\n"),
            "both counted and excluded",
        );
        violation(with(&ROW.replace("\"geo\"", "\"all_in_cost\"")), "no lines");
    }
}
