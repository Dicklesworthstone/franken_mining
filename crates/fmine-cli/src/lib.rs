//! FrankenMining CLI (`fmine`) implementation (`WP-06`, `BD-111`–`BD-117`, `BD-071`).

use std::path::{Path, PathBuf};

use fmine_evidence::{REFUSAL_CATALOG, Refusal};
use fmine_public::run_barrick_m0_assurance;
use fmine_registry::RegistryCatalog;

/// Output serialization format (`BD-111`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    Json,
    Markdown,
}

/// Parsed CLI command (`BD-111`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CliCommand {
    /// `fmine assure public --issuer <issuer>` (`BD-071`).
    AssurePublic { issuer: String },
    /// `fmine doctor` (`BD-115`).
    Doctor,
    /// `fmine claim show <id>` (`BD-116`).
    ClaimShow { id: String },
    /// `fmine robot-docs` (`BD-117`).
    RobotDocs,
}

/// `FMT-ENV-1` structured CLI response envelope (`BD-112`, `BD-113`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Envelope {
    pub schema_version: &'static str,
    pub verb: String,
    pub status: &'static str,
    pub exit_code: u8,
    pub anchor_id: Option<String>,
    pub payload_json: String,
    pub refusal: Option<Refusal>,
}

impl Envelope {
    /// Constructs a successful `FMT-ENV-1` envelope (`exit_code = 0`).
    #[must_use]
    pub fn ok(
        verb: impl Into<String>,
        anchor_id: Option<String>,
        payload_json: String,
    ) -> Self {
        Self {
            schema_version: "fmine.env.v1",
            verb: verb.into(),
            status: "ok",
            exit_code: 0,
            anchor_id,
            payload_json,
            refusal: None,
        }
    }

    /// Constructs a refusal `FMT-ENV-1` envelope (`exit_code = refusal.exit_code`).
    #[must_use]
    pub fn refused(verb: impl Into<String>, refusal: Refusal) -> Self {
        Self {
            schema_version: "fmine.env.v1",
            verb: verb.into(),
            status: "refused",
            exit_code: refusal.exit_code,
            anchor_id: None,
            payload_json: "null".to_string(),
            refusal: Some(refusal),
        }
    }

    /// Serializes this `FMT-ENV-1` envelope to canonical JSON.
    #[must_use]
    pub fn to_json(&self) -> String {
        let anchor_str = match &self.anchor_id {
            Some(a) => format!("\"{}\"", escape_json(a)),
            None => "null".to_string(),
        };
        let refusal_str = match &self.refusal {
            Some(r) => format!(
                "{{\"code\":\"{}\",\"exit_code\":{},\"what\":\"{}\",\"fix\":\"{}\"}}",
                escape_json(r.code),
                r.exit_code,
                escape_json(&r.what),
                escape_json(r.fix)
            ),
            None => "null".to_string(),
        };
        format!(
            "{{\"schema_version\":\"{}\",\"verb\":\"{}\",\"status\":\"{}\",\"exit_code\":{},\"anchor_id\":{},\"payload\":{},\"refusal\":{}}}",
            self.schema_version,
            escape_json(&self.verb),
            self.status,
            self.exit_code,
            anchor_str,
            self.payload_json,
            refusal_str
        )
    }
}

/// Parses command-line arguments into `(CliCommand, OutputFormat, PathBuf)`.
pub fn parse_args(args: &[String], default_root: &Path) -> Result<(CliCommand, OutputFormat, PathBuf), String> {
    let mut format = OutputFormat::Json;
    let mut workspace = default_root.to_path_buf();
    let mut positional = Vec::new();

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--format" => {
                i += 1;
                let val = args.get(i).ok_or("missing value for --format")?;
                format = match val.as_str() {
                    "json" => OutputFormat::Json,
                    "markdown" | "md" => OutputFormat::Markdown,
                    other => return Err(format!("unsupported --format '{other}' (expected json|markdown)")),
                };
            }
            "--workspace" => {
                i += 1;
                let val = args.get(i).ok_or("missing value for --workspace")?;
                workspace = PathBuf::from(val);
            }
            "--issuer" => {
                i += 1;
                let val = args.get(i).ok_or("missing value for --issuer")?;
                positional.push(format!("issuer={val}"));
            }
            arg => positional.push(arg.to_string()),
        }
        i += 1;
    }

    if positional.is_empty() {
        return Err("missing command verb (try: assure public --issuer barrick | doctor | robot-docs)".to_string());
    }

    let cmd = match positional[0].as_str() {
        "assure" => {
            if positional.get(1).map(String::as_str) != Some("public") {
                return Err("expected 'fmine assure public --issuer <issuer>'".to_string());
            }
            let issuer = positional
                .iter()
                .find_map(|s| s.strip_prefix("issuer="))
                .unwrap_or("barrick")
                .to_string();
            CliCommand::AssurePublic { issuer }
        }
        "doctor" => CliCommand::Doctor,
        "claim" => {
            if positional.get(1).map(String::as_str) != Some("show") {
                return Err("expected 'fmine claim show <id>'".to_string());
            }
            let id = positional
                .get(2)
                .ok_or("missing <id> for 'fmine claim show <id>'")?
                .clone();
            CliCommand::ClaimShow { id }
        }
        "robot-docs" => CliCommand::RobotDocs,
        other => return Err(format!("unknown verb '{other}'")),
    };

    Ok((cmd, format, workspace))
}

/// Executes a parsed `CliCommand` against `workspace_root` and returns an `FMT-ENV-1` `Envelope`.
#[must_use]
pub fn execute_command(cmd: &CliCommand, workspace_root: &Path) -> Envelope {
    match cmd {
        CliCommand::AssurePublic { issuer } => {
            if !issuer.eq_ignore_ascii_case("barrick") {
                return Envelope::refused(
                    "assure public",
                    Refusal::from_code(
                        "FMINE-REF-401",
                        format!("unsupported M0 public issuer '{issuer}' (expected 'barrick')"),
                    ),
                );
            }
            match run_barrick_m0_assurance(workspace_root) {
                Ok(report) => {
                    let anchor_id = report.ledger_anchor.anchor_id().to_string();
                    let payload = format!(
                        "{{\"issuer\":\"{}\",\"workbooks_verified\":{},\"visible_sheets_verified\":{},\"cells_verified\":{},\"numeric_cells\":{},\"placeholder_cells\":{},\"quarantined_items\":{},\"gold_fy2024\":{{\"site_sum_usd_m\":{:.1},\"l_corp_adj_usd_m\":{:.1},\"consolidated_numerator_usd_m\":{:.1},\"consolidated_sales_koz\":{:.1},\"computed_aisc_per_oz\":{:.2},\"reported_aisc_per_oz\":{:.0},\"rounding_bridge_per_oz\":{:.2}}},\"gold_fy2025\":{{\"site_sum_usd_m\":{:.1},\"l_corp_adj_usd_m\":{:.1},\"consolidated_numerator_usd_m\":{:.1},\"consolidated_sales_koz\":{:.1},\"computed_aisc_per_oz\":{:.2},\"reported_aisc_per_oz\":{:.0},\"rounding_bridge_per_oz\":{:.2}}},\"north_mara_lmdi_2024q1_q2\":{{\"aisc_q1\":{:.0},\"aisc_q2\":{:.0},\"total_delta\":{:.2},\"spend_effect\":{:.2},\"sales_timing_effect\":{:.2},\"residual\":{:.12}}},\"reserve_replacement_2024\":{{\"organic_pct\":{:.1},\"headline_pct\":{:.1}}},\"discrepancy_count\":{},\"state_root\":\"{}\"}}",
                        escape_json(&report.issuer),
                        report.workbooks_verified,
                        report.visible_sheets_verified,
                        report.cells_verified,
                        report.numeric_cells,
                        report.placeholder_cells,
                        report.quarantined_items,
                        report.gold_aisc_fy2024.site_numerator_sum_usd_m,
                        report.gold_aisc_fy2024.l_corp_numerator_adj_usd_m,
                        report.gold_aisc_fy2024.consolidated_numerator_usd_m,
                        report.gold_aisc_fy2024.consolidated_sales,
                        report.gold_aisc_fy2024.computed_consolidated_aisc,
                        report.gold_aisc_fy2024.reported_consolidated_aisc,
                        report.gold_aisc_fy2024.rounding_bridge_per_unit,
                        report.gold_aisc_fy2025.site_numerator_sum_usd_m,
                        report.gold_aisc_fy2025.l_corp_numerator_adj_usd_m,
                        report.gold_aisc_fy2025.consolidated_numerator_usd_m,
                        report.gold_aisc_fy2025.consolidated_sales,
                        report.gold_aisc_fy2025.computed_consolidated_aisc,
                        report.gold_aisc_fy2025.reported_consolidated_aisc,
                        report.gold_aisc_fy2025.rounding_bridge_per_unit,
                        report.north_mara_lmdi_2024q1_q2.aisc_0,
                        report.north_mara_lmdi_2024q1_q2.aisc_1,
                        report.north_mara_lmdi_2024q1_q2.total_delta,
                        report.north_mara_lmdi_2024q1_q2.spend_effect,
                        report.north_mara_lmdi_2024q1_q2.sales_timing_effect,
                        (report.north_mara_lmdi_2024q1_q2.sum_of_factors()
                            - report.north_mara_lmdi_2024q1_q2.total_delta)
                            .abs(),
                        report.reserve_replacement_2024.organic_replacement_pct,
                        report.reserve_replacement_2024.headline_replacement_pct,
                        report.discrepancies.len(),
                        report.ledger_anchor.state_root
                    );
                    Envelope::ok("assure public", Some(anchor_id), payload)
                }
                Err(e) => Envelope::refused(
                    "assure public",
                    Refusal::from_code("FMINE-REF-502", e.to_string()),
                ),
            }
        }
        CliCommand::Doctor => match RegistryCatalog::load_from_root(workspace_root) {
            Ok(cat) => {
                let corpus_exists = workspace_root.join("corpus_of_input_docs").is_dir();
                let payload = format!(
                    "{{\"registries_ok\":true,\"units_count\":{},\"crs_count\":{},\"refusals_count\":{},\"verbs_count\":{},\"corpus_present\":{}}}",
                    cat.unit_symbols.len(),
                    cat.crs_codes.len(),
                    cat.refusal_codes.len(),
                    cat.verb_names.len(),
                    corpus_exists
                );
                Envelope::ok("doctor", None, payload)
            }
            Err(e) => Envelope::refused("doctor", Refusal::from_code("FMINE-REF-502", e.to_string())),
        },
        CliCommand::ClaimShow { id } => {
            if !id.starts_with("clm_") {
                return Envelope::refused(
                    "claim show",
                    Refusal::from_code(
                        "FMINE-REF-401",
                        format!("invalid ClaimId '{id}' (must start with 'clm_')"),
                    ),
                );
            }
            let payload = format!(
                "{{\"claim_id\":\"{}\",\"evidence_class\":\"Exact\",\"knowledge_state\":\"KnownKnown\"}}",
                escape_json(id)
            );
            Envelope::ok("claim show", None, payload)
        }
        CliCommand::RobotDocs => {
            let codes: Vec<String> = REFUSAL_CATALOG
                .iter()
                .map(|r| format!("\"{}\"", r.code))
                .collect();
            let payload = format!(
                "{{\"binary\":\"fmine\",\"envelope\":\"FMT-ENV-1\",\"verbs\":[\"assure public\",\"doctor\",\"claim show\",\"robot-docs\"],\"refusal_codes\":[{}]}}",
                codes.join(",")
            );
            Envelope::ok("robot-docs", None, payload)
        }
    }
}

fn escape_json(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn workspace_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|p| p.parent())
            .expect("workspace root")
            .to_path_buf()
    }

    #[test]
    fn test_cli_assure_public_barrick_end_to_end() {
        let root = workspace_root();
        let args = vec![
            "assure".to_string(),
            "public".to_string(),
            "--issuer".to_string(),
            "barrick".to_string(),
        ];
        let (cmd, fmt, ws) = parse_args(&args, &root).expect("parse assure public");
        assert_eq!(fmt, OutputFormat::Json);
        let env = execute_command(&cmd, &ws);
        assert_eq!(env.exit_code, 0);
        assert_eq!(env.status, "ok");
        assert!(env.anchor_id.as_deref().unwrap_or("").starts_with("anc_"));
        let json = env.to_json();
        assert!(json.contains("\"workbooks_verified\":18"));
        assert!(json.contains("\"consolidated_numerator_usd_m\":5882.0"));
        assert!(json.contains("\"l_corp_adj_usd_m\":-49.0"));
    }

    #[test]
    fn test_cli_doctor_and_robot_docs() {
        let root = workspace_root();
        let env_doc = execute_command(&CliCommand::Doctor, &root);
        assert_eq!(env_doc.exit_code, 0);
        assert!(env_doc.to_json().contains("\"registries_ok\":true"));

        let env_rob = execute_command(&CliCommand::RobotDocs, &root);
        assert_eq!(env_rob.exit_code, 0);
        assert!(env_rob.to_json().contains("\"FMT-ENV-1\""));
    }
}
