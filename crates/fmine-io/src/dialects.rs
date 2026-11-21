//! Determinism: BitwisePortable
//!
//! `registries/io_dialects.toml` parser and column-semantics resolver (`BD-186`, `PO-PORT-3`, §32.1).
//!
//! Every imported column declares its `unit`, `basis`, `support`, and `crs`. Any column missing
//! from the dialect or referencing an unregistered unit or CRS stays explicitly `UNREGISTERED`
//! (`UNREGISTERED_UNIT` / `FMINE-REF-107`, `UnregisteredCrs` / `FMINE-REF-106`), never guessed
//! from header text.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fs;
use std::path::Path;

use fmine_evidence::{Refusal, RefusalCode};
use fmine_registry::toml_subset::{TomlValue, parse_subset};
use fmine_units::{ExactQty, ImportedValue, UNREGISTERED_UNIT};

/// Schema identifier for `registries/io_dialects.toml`.
pub const IO_DIALECTS_SCHEMA: &str = "fmine.io_dialects.v1";

/// Mass/metal measurement basis declared on a dialect column.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColumnBasis {
    Dry,
    Wet,
    AsReceived,
    Contained,
    Recovered,
    None,
}

impl ColumnBasis {
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "dry" => Some(Self::Dry),
            "wet" => Some(Self::Wet),
            "as_received" => Some(Self::AsReceived),
            "contained" => Some(Self::Contained),
            "recovered" => Some(Self::Recovered),
            "none" => Some(Self::None),
            _ => None,
        }
    }
}

/// Spatial or temporal support declared on a dialect column (`INV-014`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColumnSupport {
    PointCore,
    Composite,
    SmuBlock,
    MiningPolygon,
    PlantLot,
    PeriodAggregate,
    None,
}

impl ColumnSupport {
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "point_core" => Some(Self::PointCore),
            "composite" => Some(Self::Composite),
            "smu_block" => Some(Self::SmuBlock),
            "mining_polygon" => Some(Self::MiningPolygon),
            "plant_lot" => Some(Self::PlantLot),
            "period_aggregate" => Some(Self::PeriodAggregate),
            "none" => Some(Self::None),
            _ => None,
        }
    }
}

/// Privacy classification of an imported column (`BD-292`, §31.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PiiClass {
    None,
    PersonalRedact,
}

impl PiiClass {
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "none" => Some(Self::None),
            "personal_redact" => Some(Self::PersonalRedact),
            _ => None,
        }
    }
}

/// Resolved unit state of an imported column (`PO-PORT-3`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolvedUnit {
    /// Non-physical identifier / text column (`unit = "none"`).
    None,
    /// Unit present in `registries/units.toml` (or canonical length `"m"`).
    Registered(String),
    /// Undeclared or unknown unit — stored as [`UNREGISTERED_UNIT`]; arithmetic refused (`FMINE-REF-107`).
    Unregistered { declared: String },
}

/// Resolved CRS state of an imported column (`PO-PORT-3`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolvedCrs {
    None,
    Registered(String),
    Unregistered { declared: String },
}

/// One dialect header row in `registries/io_dialects.toml`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DialectHeader {
    pub id: String,
    pub vendor: String,
    pub format: String,
    pub description: String,
}

/// One column specification inside a dialect.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DialectColumn {
    pub dialect: String,
    pub column: String,
    pub declared_unit: String,
    pub basis: ColumnBasis,
    pub support: ColumnSupport,
    pub declared_crs: String,
    pub pii_class: PiiClass,
}

/// Effective resolved semantics of an actual file column header against a dialect & registries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedColumn {
    pub column_name: String,
    pub unit: ResolvedUnit,
    pub basis: Option<ColumnBasis>,
    pub support: Option<ColumnSupport>,
    pub crs: ResolvedCrs,
    pub pii_class: PiiClass,
    /// `true` if the column had no row in the dialect registry (`PO-PORT-3` undeclared).
    pub undeclared_in_dialect: bool,
}

impl ResolvedColumn {
    /// Converts a parsed numeric value into an [`ImportedValue`], tagging undeclared or unknown
    /// units with [`UNREGISTERED_UNIT`] so arithmetic refuses with `FMINE-REF-107`.
    #[must_use]
    pub fn wrap_numeric(&self, qty: ExactQty, registered_units: &[String]) -> ImportedValue {
        match &self.unit {
            ResolvedUnit::Registered(u) => ImportedValue::import(qty, u, registered_units),
            ResolvedUnit::None => ImportedValue::import(qty, "", registered_units),
            ResolvedUnit::Unregistered { declared } => {
                ImportedValue::import(qty, declared, registered_units)
            }
        }
    }

    /// Returns a typed refusal if this column is unregistered for numeric/coordinate operations.
    #[must_use]
    pub fn registration_refusal(&self) -> Option<Refusal> {
        if let ResolvedCrs::Unregistered { declared } = &self.crs {
            return Some(Refusal::new(
                RefusalCode::CrsUnregisteredOrIncompatible106,
                format!(
                    "column '{}' declares unregistered CRS '{declared}'",
                    self.column_name
                ),
            ));
        }
        if let ResolvedUnit::Unregistered { declared } = &self.unit {
            return Some(Refusal::new(
                RefusalCode::UndeclaredUnitBasisOrTz107,
                format!(
                    "column '{}' unit resolved to {UNREGISTERED_UNIT} (declared: '{declared}')",
                    self.column_name
                ),
            ));
        }
        None
    }
}

/// Errors raised when loading or parsing `registries/io_dialects.toml`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DialectError {
    Io { path: String, detail: String },
    Parse { line: usize, detail: String },
    UnknownDialect { id: String },
}

impl fmt::Display for DialectError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { path, detail } => write!(f, "failed to read {path}: {detail}"),
            Self::Parse { line, detail } => write!(f, "io_dialects.toml line {line}: {detail}"),
            Self::UnknownDialect { id } => write!(f, "unknown import dialect '{id}'"),
        }
    }
}

impl std::error::Error for DialectError {}

/// Parsed `registries/io_dialects.toml` registry (`BD-186`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IoDialectRegistry {
    pub dialects: BTreeMap<String, DialectHeader>,
    columns: BTreeMap<(String, String), DialectColumn>,
}

impl IoDialectRegistry {
    /// Loads `<workspace_root>/registries/io_dialects.toml`.
    pub fn load_from_root(workspace_root: &Path) -> Result<Self, DialectError> {
        let path = workspace_root.join("registries/io_dialects.toml");
        let text = fs::read_to_string(&path).map_err(|e| DialectError::Io {
            path: path.display().to_string(),
            detail: e.to_string(),
        })?;
        Self::parse(&text)
    }

    /// Parses and validates `io_dialects.toml` using `fmine_registry::toml_subset`.
    pub fn parse(text: &str) -> Result<Self, DialectError> {
        let doc = parse_subset(text).map_err(|e| DialectError::Parse {
            line: e.line,
            detail: e.reason,
        })?;
        match doc.top_get("schema").map(|f| &f.value) {
            Some(TomlValue::Str(s)) if s == IO_DIALECTS_SCHEMA => {}
            Some(TomlValue::Str(s)) => {
                return Err(DialectError::Parse {
                    line: 1,
                    detail: format!("expected schema {IO_DIALECTS_SCHEMA:?}, found {s:?}"),
                });
            }
            _ => {
                return Err(DialectError::Parse {
                    line: 1,
                    detail: "missing `schema` string".to_string(),
                });
            }
        }

        let mut dialects = BTreeMap::new();
        for row in doc.rows_of("dialect") {
            let id = req_str(row, "id")?;
            let vendor = req_str(row, "vendor")?;
            let format = req_str(row, "format")?;
            let description = req_str(row, "description")?;
            let header = DialectHeader {
                id: id.clone(),
                vendor,
                format,
                description,
            };
            if dialects.insert(id.clone(), header).is_some() {
                return Err(DialectError::Parse {
                    line: row.line,
                    detail: format!("duplicate dialect id '{id}'"),
                });
            }
        }

        let mut columns = BTreeMap::new();
        for row in doc.rows_of("column") {
            let dialect = req_str(row, "dialect")?;
            if !dialects.contains_key(&dialect) {
                return Err(DialectError::Parse {
                    line: row.line,
                    detail: format!("column references undeclared dialect '{dialect}'"),
                });
            }
            let column = req_str(row, "column")?;
            let declared_unit = req_str(row, "unit")?;
            let basis_str = req_str(row, "basis")?;
            let basis = ColumnBasis::parse(&basis_str).ok_or_else(|| DialectError::Parse {
                line: row.line,
                detail: format!("unknown basis '{basis_str}'"),
            })?;
            let support_str = req_str(row, "support")?;
            let support =
                ColumnSupport::parse(&support_str).ok_or_else(|| DialectError::Parse {
                    line: row.line,
                    detail: format!("unknown support '{support_str}'"),
                })?;
            let declared_crs = req_str(row, "crs")?;
            let pii_str = req_str(row, "pii_class")?;
            let pii_class = PiiClass::parse(&pii_str).ok_or_else(|| DialectError::Parse {
                line: row.line,
                detail: format!("unknown pii_class '{pii_str}'"),
            })?;
            let spec = DialectColumn {
                dialect: dialect.clone(),
                column: column.clone(),
                declared_unit,
                basis,
                support,
                declared_crs,
                pii_class,
            };
            if columns
                .insert((dialect.clone(), column.clone()), spec)
                .is_some()
            {
                return Err(DialectError::Parse {
                    line: row.line,
                    detail: format!("duplicate column '{column}' in dialect '{dialect}'"),
                });
            }
        }

        Ok(Self { dialects, columns })
    }

    /// Resolves an actual file's header list against `dialect_id` and the registered unit & CRS sets.
    ///
    /// Any column absent from the dialect, or naming an unregistered unit/CRS, resolves to
    /// `ResolvedUnit::Unregistered` / `ResolvedCrs::Unregistered` (`BD-186` done-when:
    /// `Undeclared -> UNREGISTERED`).
    pub fn resolve_headers(
        &self,
        dialect_id: &str,
        headers: &[String],
        registered_units: &BTreeSet<String>,
        registered_crs: &BTreeSet<String>,
    ) -> Result<Vec<ResolvedColumn>, DialectError> {
        if !self.dialects.contains_key(dialect_id) {
            return Err(DialectError::UnknownDialect {
                id: dialect_id.to_string(),
            });
        }
        let mut out = Vec::with_capacity(headers.len());
        for col_name in headers {
            if let Some(spec) = self
                .columns
                .get(&(dialect_id.to_string(), col_name.clone()))
            {
                let unit = if spec.declared_unit == "none" {
                    ResolvedUnit::None
                } else if registered_units.contains(&spec.declared_unit)
                    || spec.declared_unit == "m"
                {
                    ResolvedUnit::Registered(spec.declared_unit.clone())
                } else {
                    ResolvedUnit::Unregistered {
                        declared: spec.declared_unit.clone(),
                    }
                };
                let crs = if spec.declared_crs == "none" {
                    ResolvedCrs::None
                } else if registered_crs.contains(&spec.declared_crs) {
                    ResolvedCrs::Registered(spec.declared_crs.clone())
                } else {
                    ResolvedCrs::Unregistered {
                        declared: spec.declared_crs.clone(),
                    }
                };
                out.push(ResolvedColumn {
                    column_name: col_name.clone(),
                    unit,
                    basis: Some(spec.basis),
                    support: Some(spec.support),
                    crs,
                    pii_class: spec.pii_class,
                    undeclared_in_dialect: false,
                });
            } else {
                // PO-PORT-3 / BD-186: undeclared column NEVER guesses unit/basis/support/CRS!
                out.push(ResolvedColumn {
                    column_name: col_name.clone(),
                    unit: ResolvedUnit::Unregistered {
                        declared: "UNDECLARED".to_string(),
                    },
                    basis: None,
                    support: None,
                    crs: ResolvedCrs::None,
                    pii_class: PiiClass::None,
                    undeclared_in_dialect: true,
                });
            }
        }
        Ok(out)
    }
}

fn req_str(row: &fmine_registry::toml_subset::TomlRow, key: &str) -> Result<String, DialectError> {
    match row.get(key).map(|f| &f.value) {
        Some(TomlValue::Str(s)) => Ok(s.clone()),
        _ => Err(DialectError::Parse {
            line: row.line,
            detail: format!("missing required string field `{key}`"),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fmine_registry::RegistryCatalog;
    use std::path::PathBuf;

    fn workspace_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|p| p.parent())
            .expect("workspace root")
            .to_path_buf()
    }

    #[test]
    fn test_io_dialects_undeclared_column_and_unknown_unit_resolve_to_unregistered() {
        let root = workspace_root();
        let catalog = RegistryCatalog::load_from_root(&root).expect("load registries");
        let reg_units: BTreeSet<String> = catalog.unit_symbols.iter().cloned().collect();
        let reg_crs: BTreeSet<String> = catalog.crs_codes.iter().cloned().collect();

        let dialects =
            IoDialectRegistry::load_from_root(&root).expect("load registries/io_dialects.toml");
        assert!(dialects.dialects.contains_key("acquire_assay_v1"));
        assert!(dialects.dialects.contains_key("datamine_string_v1"));
        assert!(dialects.dialects.contains_key("surpac_string_v1"));
        assert!(dialects.dialects.contains_key("esia_stakeholder_v1"));

        let headers = vec![
            "HOLEID".to_string(),
            "AU_GPT".to_string(),
            "MYSTERY_COL".to_string(),
        ];
        let resolved = dialects
            .resolve_headers("acquire_assay_v1", &headers, &reg_units, &reg_crs)
            .expect("resolve");
        assert_eq!(resolved[0].unit, ResolvedUnit::None);
        assert_eq!(
            resolved[1].unit,
            ResolvedUnit::Registered("g_per_t".to_string())
        );
        assert_eq!(resolved[1].basis, Some(ColumnBasis::Dry));
        assert_eq!(resolved[1].support, Some(ColumnSupport::PointCore));
        assert!(resolved[1].registration_refusal().is_none());

        // Undeclared column resolves to UNREGISTERED_UNIT and emits FMINE-REF-107 on arithmetic.
        assert!(resolved[2].undeclared_in_dialect);
        assert!(matches!(
            resolved[2].unit,
            ResolvedUnit::Unregistered { .. }
        ));
        let r = resolved[2]
            .registration_refusal()
            .expect("undeclared refuses");
        assert_eq!((r.code, r.exit_code), ("FMINE-REF-107", 3));
        let wrapped =
            resolved[2].wrap_numeric(ExactQty::new(150, 2).expect("qty"), &catalog.unit_symbols);
        assert_eq!(wrapped.unit(), UNREGISTERED_UNIT);
        assert!(wrapped.checked_add(&wrapped).is_err());

        // Dialect row with unknown unit or unknown CRS also resolves to Unregistered!
        let bad_toml = r#"
schema = "fmine.io_dialects.v1"
[[dialect]]
id = "custom_v1"
vendor = "Custom"
format = "csv"
description = "custom"
[[column]]
dialect = "custom_v1"
column = "WEIRD_GRADE"
unit = "furlongs_per_fortnight"
basis = "dry"
support = "point_core"
crs = "EPSG:999999"
pii_class = "none"
"#;
        let custom = IoDialectRegistry::parse(bad_toml).expect("parse custom");
        let cols = custom
            .resolve_headers(
                "custom_v1",
                &["WEIRD_GRADE".to_string()],
                &reg_units,
                &reg_crs,
            )
            .expect("resolve custom");
        assert!(matches!(cols[0].unit, ResolvedUnit::Unregistered { .. }));
        assert!(matches!(cols[0].crs, ResolvedCrs::Unregistered { .. }));
        let r_crs = cols[0].registration_refusal().expect("crs refused");
        assert_eq!((r_crs.code, r_crs.exit_code), ("FMINE-REF-106", 3));
    }
}
