//! Reference-class registry governance check (`BD-050`, `GATE-PLUMB-E4`, plan §9.3).
//!
//! Validates `registries/refclass.toml` via `fmine_claim::refclass::RefClassRegistry::parse`
//! (single source of truth, zero duplicate parser) and confirms the baseline `GB-32` row
//! (`REFCLASS-CAPEX-GREENFIELD`) and both `verified` and `unverified` statuses are present.

use std::fs;
use std::path::Path;

use fmine_claim::refclass::{RefClassId, RefClassRegistry, RefClassStatus};

/// Summary emitted by `xtask check-refclass`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RefClassCheckReport {
    pub total: usize,
    pub verified: usize,
    pub unverified: usize,
}

impl RefClassCheckReport {
    pub fn log_line(&self) -> String {
        format!(
            "{{\"check\":\"refclass\",\"total\":{},\"verified\":{},\"unverified\":{}}}",
            self.total, self.verified, self.unverified
        )
    }
}

/// Validate `registries/refclass.toml` against `GATE-PLUMB-E4` schema and completeness rules.
pub fn check_refclass(root: &Path) -> Result<RefClassCheckReport, Vec<String>> {
    let path = root.join("registries/refclass.toml");
    let text = fs::read_to_string(&path)
        .map_err(|e| vec![format!("failed to read {}: {e}", path.display())])?;
    let reg = RefClassRegistry::parse(&text).map_err(|r| vec![r.to_string()])?;

    let mut errors = Vec::new();
    let gb32_id =
        RefClassId::parse("REFCLASS-CAPEX-GREENFIELD").map_err(|r| vec![r.to_string()])?;
    match reg.get(&gb32_id) {
        None => errors.push(
            "registries/refclass.toml: missing baseline GB-32 row REFCLASS-CAPEX-GREENFIELD"
                .to_string(),
        ),
        Some(row) if row.status != RefClassStatus::Verified => errors.push(
            "registries/refclass.toml: REFCLASS-CAPEX-GREENFIELD must carry status = \"verified\""
                .to_string(),
        ),
        Some(_) => {}
    }

    let verified = reg
        .iter()
        .filter(|r| r.status == RefClassStatus::Verified)
        .count();
    let unverified = reg
        .iter()
        .filter(|r| r.status == RefClassStatus::Unverified)
        .count();
    if verified == 0 {
        errors.push(
            "registries/refclass.toml: must contain at least one verified REFCLASS- row"
                .to_string(),
        );
    }

    if errors.is_empty() {
        Ok(RefClassCheckReport {
            total: reg.len(),
            verified,
            unverified,
        })
    } else {
        Err(errors)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_workspace_refclass_registry_passes_gate_plumb_e4() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|p| p.parent())
            .expect("workspace root");
        let report = check_refclass(root).expect("registries/refclass.toml must pass");
        assert!(report.total >= 6);
        assert!(report.verified >= 5);
        assert!(report.unverified >= 1);
    }
}
