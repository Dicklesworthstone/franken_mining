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

    #[test]
    fn test_seeded_broken_refclass_registry_fails() {
        let tmp =
            std::env::temp_dir().join(format!("fmine_xtask_refclass_neg_{}", std::process::id()));
        let reg_dir = tmp.join("registries");
        fs::create_dir_all(&reg_dir).expect("mkdir");

        // Case 1: baseline GB-32 row downgraded to unverified must fail check_refclass.
        let bad_unverified = r#"
schema_version = 1
[[refclass]]
id = "REFCLASS-CAPEX-GREENFIELD"
domain = "capex_greenfield"
population = "p"
n = 63
period = "1980-2001"
inclusion_rule = "r"
similarity_justification = "j"
status = "unverified"
epsilon_bps = 2500
ref_mean_multiplier_bps = 12800
ref_std_multiplier_bps = 3500
source_document = "Bertisen & Davis"
source_e_ids = "E001,GB-32"
"#;
        fs::write(reg_dir.join("refclass.toml"), bad_unverified).expect("write");
        let errs = check_refclass(&tmp).expect_err("unverified GB-32 row must fail");
        assert!(
            errs.iter().any(|e| e.contains("REFCLASS-CAPEX-GREENFIELD")),
            "{errs:?}"
        );

        // Case 2: unknown key in registries/refclass.toml must fail check_refclass.
        let bad_key = bad_unverified.replace("status = \"unverified\"", "status = \"verified\"")
            + "extra_typo = \"bad\"\n";
        fs::write(reg_dir.join("refclass.toml"), bad_key).expect("write");
        let errs2 = check_refclass(&tmp).expect_err("unknown field must fail");
        assert!(errs2.iter().any(|e| e.contains("unknown")), "{errs2:?}");

        let _ = fs::remove_dir_all(&tmp);
    }
}
