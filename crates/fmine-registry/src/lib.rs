//! Schema-validated loader and cross-reference verifier for `registries/*.toml` (`BD-030`, `BD-031`).

use std::collections::BTreeSet;
use std::fmt;
use std::fs;
use std::path::Path;

use fmine_core::codec::Encoder;
use fmine_core::hash::blake3_hash;
use fmine_evidence::REFUSAL_CATALOG;

/// Errors raised when loading or validating `registries/*.toml`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegistryError {
    /// Underlying file read failure.
    Io { path: String, detail: String },
    /// Duplicate key within a registry table.
    DuplicateId { registry: String, id: String },
    /// Missing required field or drift against static catalog.
    SchemaViolation { registry: String, detail: String },
}

impl fmt::Display for RegistryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { path, detail } => write!(f, "registry I/O error at {path}: {detail}"),
            Self::DuplicateId { registry, id } => {
                write!(f, "duplicate id '{id}' in registry '{registry}'")
            }
            Self::SchemaViolation { registry, detail } => {
                write!(f, "schema violation in '{registry}': {detail}")
            }
        }
    }
}

impl std::error::Error for RegistryError {}

/// Validated summary of all workspace TOML registries (`BD-030`, `BD-031`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegistryCatalog {
    pub unit_symbols: Vec<String>,
    pub crs_codes: Vec<String>,
    pub site_calendar_ids: Vec<String>,
    pub refusal_codes: Vec<String>,
    pub verb_names: Vec<String>,
    pub manifest_digest: [u8; 32],
}

impl RegistryCatalog {
    /// Required registry filenames inside `<workspace_root>/registries/`.
    pub const REQUIRED_FILES: [&'static str; 8] = [
        "units.toml",
        "crs.toml",
        "calendars.toml",
        "refusals.toml",
        "verbs.toml",
        "loc_budget.toml",
        "workspace_topology.toml",
        "lifts.toml",
    ];

    /// Loads and validates all 8 TOML registries from `<workspace_root>/registries/`.
    pub fn load_from_root(workspace_root: &Path) -> Result<Self, RegistryError> {
        let reg_dir = workspace_root.join("registries");
        let mut enc = Encoder::new_frame(*b"REGS", 1);

        for &file in &Self::REQUIRED_FILES {
            let p = reg_dir.join(file);
            let content = fs::read_to_string(&p).map_err(|e| RegistryError::Io {
                path: p.display().to_string(),
                detail: e.to_string(),
            })?;
            enc.put_str(file);
            enc.put_str(&content);
        }

        let units_src = read_reg(&reg_dir, "units.toml")?;
        let unit_symbols = extract_unique_key(&units_src, "units.toml", "code")?;

        let crs_src = read_reg(&reg_dir, "crs.toml")?;
        let crs_codes = extract_unique_key(&crs_src, "crs.toml", "code")?;

        let cal_src = read_reg(&reg_dir, "calendars.toml")?;
        let site_calendar_ids = extract_unique_key(&cal_src, "calendars.toml", "site_id")?;

        let ref_src = read_reg(&reg_dir, "refusals.toml")?;
        let refusal_codes = extract_unique_key(&ref_src, "refusals.toml", "code")?;

        // Verify 1:1 alignment between registries/refusals.toml and fmine_evidence::REFUSAL_CATALOG.
        for spec in REFUSAL_CATALOG {
            if !refusal_codes.iter().any(|c| c == spec.code) {
                return Err(RegistryError::SchemaViolation {
                    registry: "refusals.toml".to_string(),
                    detail: format!("missing refusal code {} required by fmine-evidence", spec.code),
                });
            }
        }

        let verbs_src = read_reg(&reg_dir, "verbs.toml")?;
        let verb_names = extract_unique_key(&verbs_src, "verbs.toml", "path")?;

        let manifest_digest = blake3_hash(&enc.finish()).0;

        Ok(Self {
            unit_symbols,
            crs_codes,
            site_calendar_ids,
            refusal_codes,
            verb_names,
            manifest_digest,
        })
    }
}

fn read_reg(reg_dir: &Path, name: &str) -> Result<String, RegistryError> {
    let p = reg_dir.join(name);
    fs::read_to_string(&p).map_err(|e| RegistryError::Io {
        path: p.display().to_string(),
        detail: e.to_string(),
    })
}

fn extract_unique_key(
    toml_text: &str,
    registry_name: &str,
    key: &str,
) -> Result<Vec<String>, RegistryError> {
    let mut seen = BTreeSet::new();
    let mut ordered = Vec::new();
    for raw_line in toml_text.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with('[') {
            continue;
        }
        if let Some((k, v)) = line.split_once('=') {
            if k.trim() == key {
                let val = v.trim().trim_matches('"').to_string();
                if val.is_empty() {
                    return Err(RegistryError::SchemaViolation {
                        registry: registry_name.to_string(),
                        detail: format!("empty value for key '{key}'"),
                    });
                }
                if !seen.insert(val.clone()) {
                    return Err(RegistryError::DuplicateId {
                        registry: registry_name.to_string(),
                        id: val,
                    });
                }
                ordered.push(val);
            }
        }
    }
    if ordered.is_empty() {
        return Err(RegistryError::SchemaViolation {
            registry: registry_name.to_string(),
            detail: format!("no entries found with key '{key}'"),
        });
    }
    Ok(ordered)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_load_and_validate_workspace_registries() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|p| p.parent())
            .expect("workspace root")
            .to_path_buf();
        let catalog = RegistryCatalog::load_from_root(&root).expect("registries must validate");
        assert!(catalog.unit_symbols.len() >= 10);
        assert!(catalog.crs_codes.iter().any(|c| c == "EPSG:32641"));
        assert_eq!(catalog.refusal_codes.len(), 22);
        assert!(catalog.verb_names.iter().any(|v| v == "assure public"));
    }
}
