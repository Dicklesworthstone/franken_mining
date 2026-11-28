//! DESIGN (`BD-162`, plan §20.9): Touchstone posterior reconciliation (`fmine recon run` on
//! `fmine.recon-posterior.v1`) and attested period close (`fmine recon close`), emitting canonical
//! `ReconciliationCertificate` (`RCRT`) + `FMT-CERT-1` certificates verified offline by
//! `fmine-verify`.
//! Determinism: DeterministicPerIsa (delegates WLS/bias/identifiability to `fmine-recon` with
//! canonical ordering and SHA-256 / BLAKE3 replay digests).

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

use fmine_core::anchor::SourceAnchor;
use fmine_core::cert::{DeterminismClass, InputDigest};
use fmine_core::det;
use fmine_core::hash::{Digest32, blake3_hash, sha256};
use fmine_evidence::{Refusal, RefusalCode};
use fmine_recon::bias::{
    BiasObservation, BiasTargetKind, HierarchicalBiasRequest, HierarchicalBiasResult,
    HierarchicalGroupPrior, MiningSourceKind, NodeBiasSpec, StandaloneNodePrior,
    fit_hierarchical_bias_posteriors,
};
use fmine_recon::certificate::{
    AttestationStatus, BalanceCertificateSummary, CertifiedFactorInterval,
    CertifiedQuantityInterval, IdentifiabilityCertificateSummary, NodeBalanceRecord,
    ParityDifferenceStatus, ParityExplanation, PosteriorCertificateSummary,
    ReconciliationCertificate, ReconciliationCertificateRequest, RefusedPairVoiRecord,
    RegisteredForecastScore,
};
use fmine_recon::gross_error::{
    CommitDisposition, GrossErrorPolicy, GrossErrorReport, MultiplicityCorrection,
    enforce_commit_gate, evaluate_gross_errors,
};
use fmine_recon::identifiability::{
    CandidateMeasurement, CandidateMeasurementKind, IdentifiabilityReport, MeasurementVoiRequest,
    MeasurementVoiScore, analyze_hierarchical_identifiability, evaluate_measurement_voi,
};
use fmine_recon::linear::{
    CovarianceOffDiagonal, DirectGradeReconciliationAttempt, ExtensiveChannel, LinearReconResult,
    MeasuredVariable, NamedLinearConstraint, NamedLinearRequest, VariableKey,
    reconcile_named_system, refuse_direct_grade_reconciliation,
};
use fmine_registry::toml_subset::{TomlField, TomlValue, parse_subset};

use crate::worksheet_input::markdown_cell;
use crate::{Envelope, NextAction, json_string as quote, worksheet_input};

pub(crate) const POSTERIOR_SCHEMA: &str = "fmine.recon-posterior.v1";
pub(crate) const CLOSE_VERB: &str = "recon close";
pub(crate) const CLOSE_USAGE: &str = "recon close --input <recon-posterior.toml> --out-dir <dir> (--reviewer-role <role> --against-digest <hex> | --allow-draft) [--code-revision <rev>]";
pub(crate) const POSTERIOR_NO_CLAIM: &str = "Posterior reconciliation conditioned on declared measurement error models and balance topology. reserve_disclosure_claim=false, execution_authorized=false, geotechnical_signoff=false, regulatory_compliance_claim=false (INV-002). FrankenMining never signs human attestations.";
pub(crate) const ROBOT_DOCS_JSON: &str = concat!(
    r#"{"schemas":["fmine.matched-cohorts.v1","fmine.recon-posterior.v1"],"#,
    r#""run_command":"recon run --input <worksheet.toml>","#,
    r#""close_command":"recon close --input <recon-posterior.toml> --out-dir <dir> (--reviewer-role <role> --against-digest <hex> | --allow-draft) [--code-revision <rev>]","#,
    r#""posterior_guards":["G-TOUCH-1 extensive-only (FMINE-REF-103)","G-TOUCH-2 identifiability attribution refusal (FMINE-REF-104)","G-TOUCH-4 F-factor definition_version & interval (FMINE-REF-107)","INV-005 gross-error commit gate (FMINE-REF-101)","INV-002 all four authority flags false"],"#,
    r#""attestation":"recon run emits review_target_digest (BLAKE3) without writing files; recon close verifies --against-digest matches review_target_digest unless --allow-draft is passed, then writes recon_certificate.rcrt, recon_certificate.cert (FMT-CERT-1), and inputs/<locator> verifiable by fmine-verify cert --inputs-dir","#,
    r#""example":"tests/recon/posterior_gb24.toml"}"#,
);

const DEFAULT_CODE_REVISION: &str = concat!("fmine-cli-", env!("CARGO_PKG_VERSION"));
const MAX_WORKSHEET_BYTES: u64 = 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CloseCommand {
    pub input: PathBuf,
    pub out_dir: PathBuf,
    pub reviewer_role: Option<String>,
    pub against_digest: Option<String>,
    pub allow_draft: bool,
    pub code_revision: Option<String>,
}

pub fn parse_close(args: &[String]) -> Result<CloseCommand, String> {
    if args.len() < 2 || args[0] != "recon" || args[1] != "close" {
        return Err(format!("expected 'fmine {CLOSE_USAGE}'"));
    }
    let mut input: Option<PathBuf> = None;
    let mut out_dir: Option<PathBuf> = None;
    let mut reviewer_role: Option<String> = None;
    let mut against_digest: Option<String> = None;
    let mut allow_draft = false;
    let mut code_revision: Option<String> = None;

    let mut i = 2usize;
    while i < args.len() {
        match args[i].as_str() {
            "--allow-draft" => {
                if allow_draft {
                    return Err("duplicate --allow-draft flag".to_string());
                }
                allow_draft = true;
                i += 1;
            }
            flag @ ("--input" | "--out-dir" | "--reviewer-role" | "--against-digest"
            | "--code-revision") => {
                let Some(val) = args.get(i + 1) else {
                    return Err(format!(
                        "missing value for {flag}; expected 'fmine {CLOSE_USAGE}'"
                    ));
                };
                if val.trim().is_empty() || val.starts_with("--") {
                    return Err(format!(
                        "invalid value for {flag}; expected 'fmine {CLOSE_USAGE}'"
                    ));
                }
                let slot = match flag {
                    "--input" => {
                        if input.replace(PathBuf::from(val)).is_some() {
                            return Err("duplicate --input flag".to_string());
                        }
                        None
                    }
                    "--out-dir" => {
                        if out_dir.replace(PathBuf::from(val)).is_some() {
                            return Err("duplicate --out-dir flag".to_string());
                        }
                        None
                    }
                    "--reviewer-role" => {
                        Some((&mut reviewer_role, "duplicate --reviewer-role flag"))
                    }
                    "--against-digest" => {
                        Some((&mut against_digest, "duplicate --against-digest flag"))
                    }
                    "--code-revision" => {
                        Some((&mut code_revision, "duplicate --code-revision flag"))
                    }
                    _ => None,
                };
                if let Some((target, dup_msg)) = slot
                    && target.replace(val.clone()).is_some()
                {
                    return Err(dup_msg.to_string());
                }
                i += 2;
            }
            other => {
                return Err(format!(
                    "unexpected argument '{other}'; expected 'fmine {CLOSE_USAGE}'"
                ));
            }
        }
    }

    let Some(input) = input else {
        return Err(format!("missing --input; expected 'fmine {CLOSE_USAGE}'"));
    };
    let Some(out_dir) = out_dir else {
        return Err(format!("missing --out-dir; expected 'fmine {CLOSE_USAGE}'"));
    };
    Ok(CloseCommand {
        input,
        out_dir,
        reviewer_role,
        against_digest,
        allow_draft,
        code_revision,
    })
}

fn input_err(detail: impl Into<String>) -> Refusal {
    Refusal::new(RefusalCode::InputMissingOrUnreadable506, detail)
}

fn domain_err(detail: impl Into<String>) -> Refusal {
    Refusal::new(RefusalCode::OutsidePhysicalDomain109, detail)
}

fn check_keys(fields: &[TomlField], required: &[&str], optional: &[&str]) -> Result<(), Refusal> {
    let mut seen = BTreeSet::new();
    for f in fields {
        if !required.contains(&f.key.as_str()) && !optional.contains(&f.key.as_str()) {
            return Err(input_err(format!(
                "unknown field '{}' at line {}",
                f.key, f.line
            )));
        }
        if !seen.insert(f.key.as_str()) {
            return Err(input_err(format!(
                "duplicate field '{}' at line {}",
                f.key, f.line
            )));
        }
    }
    for req in required {
        if !seen.contains(*req) {
            return Err(input_err(format!("missing required field '{req}'")));
        }
    }
    Ok(())
}

fn get_field<'a>(fields: &'a [TomlField], key: &str) -> Option<&'a TomlValue> {
    fields.iter().find(|f| f.key == key).map(|f| &f.value)
}

fn req_str<'a>(fields: &'a [TomlField], key: &str) -> Result<&'a str, Refusal> {
    match get_field(fields, key) {
        Some(TomlValue::Str(s))
            if !s.trim().is_empty()
                && s.trim() == s
                && s.len() <= 4096
                && !s.chars().any(char::is_control) =>
        {
            Ok(s.as_str())
        }
        _ => Err(input_err(format!(
            "field '{key}' must be a non-empty bounded string without edge whitespace or control characters"
        ))),
    }
}

fn opt_str<'a>(fields: &'a [TomlField], key: &str) -> Result<Option<&'a str>, Refusal> {
    match get_field(fields, key) {
        None => Ok(None),
        Some(TomlValue::Str(s))
            if !s.trim().is_empty()
                && s.trim() == s
                && s.len() <= 4096
                && !s.chars().any(char::is_control) =>
        {
            Ok(Some(s.as_str()))
        }
        _ => Err(input_err(format!(
            "optional field '{key}' must be a non-empty bounded string without edge whitespace or control characters"
        ))),
    }
}

fn parse_f64_token(token: &str, key: &str) -> Result<f64, Refusal> {
    let val = token
        .parse::<f64>()
        .map_err(|_| domain_err(format!("field '{key}' has invalid numeric value '{token}'")))?;
    let nonzero_mantissa = token
        .split(['e', 'E'])
        .next()
        .unwrap_or("")
        .bytes()
        .any(|b| matches!(b, b'1'..=b'9'));
    if !val.is_finite() || (val == 0.0 && nonzero_mantissa) {
        return Err(domain_err(format!(
            "field '{key}' has non-finite or underflowing value '{token}'"
        )));
    }
    Ok(val)
}

fn req_f64(fields: &[TomlField], key: &str) -> Result<f64, Refusal> {
    match get_field(fields, key) {
        Some(TomlValue::Int(n)) => Ok(*n as f64),
        Some(TomlValue::Str(s)) => parse_f64_token(s, key),
        None => Err(input_err(format!("missing required numeric field '{key}'"))),
    }
}

fn opt_f64(fields: &[TomlField], key: &str, default: f64) -> Result<f64, Refusal> {
    match get_field(fields, key) {
        None => Ok(default),
        Some(TomlValue::Int(n)) => Ok(*n as f64),
        Some(TomlValue::Str(s)) => parse_f64_token(s, key),
    }
}

fn opt_u64(fields: &[TomlField], key: &str, default: u64) -> Result<u64, Refusal> {
    match get_field(fields, key) {
        None => Ok(default),
        Some(TomlValue::Int(n)) if *n >= 0 => Ok(*n as u64),
        Some(TomlValue::Str(s)) => s
            .parse::<u64>()
            .map_err(|_| domain_err(format!("field '{key}' must be an unsigned integer"))),
        _ => Err(domain_err(format!(
            "field '{key}' must be an unsigned integer"
        ))),
    }
}

fn parse_bool_field(
    fields: &[TomlField],
    key: &str,
    default: Option<bool>,
) -> Result<bool, Refusal> {
    match get_field(fields, key) {
        None => default.ok_or_else(|| input_err(format!("missing required boolean field '{key}'"))),
        Some(TomlValue::Str(s)) if s == "true" => Ok(true),
        Some(TomlValue::Str(s)) if s == "false" => Ok(false),
        _ => Err(input_err(format!(
            "field '{key}' must be quoted \"true\" or \"false\""
        ))),
    }
}

fn parse_terms(spec: &str, key: &str) -> Result<Vec<(String, f64)>, Refusal> {
    let mut out = Vec::new();
    let mut seen = BTreeSet::new();
    for raw in spec.split(',') {
        let piece = raw.trim();
        let Some((id_raw, coeff_raw)) = piece.split_once(':') else {
            return Err(input_err(format!(
                "field '{key}' term '{piece}' must have format 'id:coeff'"
            )));
        };
        let id = id_raw.trim();
        if id.is_empty() {
            return Err(input_err(format!("field '{key}' has empty variable id")));
        }
        if !seen.insert(id.to_string()) {
            return Err(input_err(format!(
                "field '{key}' contains duplicate variable id '{id}'"
            )));
        }
        let coeff = parse_f64_token(coeff_raw.trim(), key)?;
        out.push((id.to_string(), coeff));
    }
    if out.is_empty() {
        return Err(input_err(format!(
            "field '{key}' must list at least one term"
        )));
    }
    Ok(out)
}

fn parse_channel(raw: &str, id: &str) -> Result<ExtensiveChannel, Refusal> {
    if raw.eq_ignore_ascii_case("grade") {
        let Err(err) = refuse_direct_grade_reconciliation(&DirectGradeReconciliationAttempt {
            context: format!("variable '{id}' declared direct grade channel"),
            element: id.to_string(),
        }) else {
            return Err(Refusal::new(
                RefusalCode::NonAdditiveAverage103,
                format!("G-TOUCH-1: direct grade channel refused on variable '{id}'"),
            ));
        };
        return Err(err.to_refusal());
    }
    match raw {
        "dry_mass" => Ok(ExtensiveChannel::DryMass),
        "water_mass" => Ok(ExtensiveChannel::WaterMass),
        other => {
            if let Some(el) = other
                .strip_prefix("element_mass:")
                .or_else(|| other.strip_prefix("element:"))
                && !el.trim().is_empty()
            {
                return Ok(ExtensiveChannel::Element(el.trim().to_string()));
            }
            Err(domain_err(format!(
                "unsupported channel '{other}' on variable '{id}'; use dry_mass, water_mass, or element:<El>"
            )))
        }
    }
}

fn parse_mining_source(raw: &str) -> Result<MiningSourceKind, Refusal> {
    match raw {
        "open_pit" => Ok(MiningSourceKind::OpenPit),
        "underground" => Ok(MiningSourceKind::Underground),
        "stockpile" => Ok(MiningSourceKind::Stockpile),
        "plant" => Ok(MiningSourceKind::Plant),
        "shared" => Ok(MiningSourceKind::Shared),
        other => Err(domain_err(format!("unknown mining source '{other}'"))),
    }
}

fn parse_bias_target(raw: &str) -> Result<BiasTargetKind, Refusal> {
    match raw {
        "in_situ_metal" => Ok(BiasTargetKind::InSituMetal),
        "dilution" => Ok(BiasTargetKind::Dilution),
        "weightometer" => Ok(BiasTargetKind::Weightometer),
        "stockpile_survey" => Ok(BiasTargetKind::StockpileSurvey),
        "mill_sampler" => Ok(BiasTargetKind::MillSampler),
        "density" => Ok(BiasTargetKind::Density),
        "truck_payload" => Ok(BiasTargetKind::TruckPayload),
        "recovery_card" => Ok(BiasTargetKind::RecoveryCard),
        "moisture" => Ok(BiasTargetKind::Moisture),
        other => Err(domain_err(format!("unknown bias target '{other}'"))),
    }
}

fn parse_voi_kind(raw: &str) -> Result<CandidateMeasurementKind, Refusal> {
    match raw {
        "sampler_audit" => Ok(CandidateMeasurementKind::SamplerAudit),
        "cross_belt_analyser" => Ok(CandidateMeasurementKind::CrossBeltAnalyser),
        "stockpile_resurvey" => Ok(CandidateMeasurementKind::StockpileResurvey),
        "check_assay_round_robin" => Ok(CandidateMeasurementKind::CheckAssayRoundRobin),
        "density_programme" => Ok(CandidateMeasurementKind::DensityProgramme),
        "grade_control_infill" => Ok(CandidateMeasurementKind::GradeControlInfill),
        other => Err(domain_err(format!("unknown voi_candidate kind '{other}'"))),
    }
}

fn sanitize_token(s: &str) -> String {
    const MAX_SLUG_CHARS: usize = 40;
    let mut out = String::with_capacity(s.len().min(MAX_SLUG_CHARS));
    let mut truncated = false;
    for ch in s.chars() {
        if out.len() >= MAX_SLUG_CHARS {
            truncated = true;
            break;
        }
        if ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' {
            out.push(ch);
        } else {
            out.push('_');
        }
    }
    if out.is_empty() {
        out.push_str("recon");
    }
    if truncated {
        let h = blake3_hash(s.as_bytes()).to_hex();
        out.push('_');
        out.push_str(&h[..8]);
    }
    out
}

fn format_channel(channel: &ExtensiveChannel) -> String {
    match channel {
        ExtensiveChannel::DryMass => "dry_mass".to_string(),
        ExtensiveChannel::WaterMass => "water_mass".to_string(),
        ExtensiveChannel::Element(el) => format!("element:{el}"),
    }
}

fn var_label(key: &VariableKey) -> String {
    match key {
        VariableKey::Stream { stream_id, channel } => {
            format!("stream:{stream_id}:{}", format_channel(channel))
        }
        VariableKey::InventoryDelta { node_id, channel } => {
            format!("inv:{node_id}:{}", format_channel(channel))
        }
        VariableKey::Named { name, channel } => {
            format!("named:{name}:{}", format_channel(channel))
        }
    }
}

fn is_lower_hex_64(s: &str) -> bool {
    s.len() == 64 && s.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

fn is_ascii_hex_64(s: &str) -> bool {
    s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit())
}

fn json_f64(v: f64) -> String {
    if v.is_finite() {
        if v == 0.0 {
            "0".to_string()
        } else {
            v.to_string()
        }
    } else {
        "null".to_string()
    }
}

struct FactorSpec {
    registry_key: String,
    definition_version: String,
    numerator: String,
    denominator: String,
}

struct VoiCandSpec {
    id: String,
    kind: CandidateMeasurementKind,
    description: String,
    terms: Vec<(String, f64)>,
    obs_sd: f64,
    cost_usd: f64,
}

struct EvaluatedPosterior {
    site: String,
    period: String,
    evidence_cut_id: String,
    error_model_version: String,
    ownership_basis: String,
    code_revision: String,
    seed_root: u64,
    confidence_level: f64,
    input_locator: String,
    input_sha256: Digest32,
    linear: LinearReconResult,
    gross_error: GrossErrorReport,
    f_factors: Vec<CertifiedFactorInterval>,
    parity_notes: Vec<ParityExplanation>,
    forecast_scores: Vec<RegisteredForecastScore>,
    bias_fit: Option<HierarchicalBiasResult>,
    identifiability: Option<IdentifiabilityReport>,
    voi_scores: Vec<MeasurementVoiScore>,
    refused_pairs: Vec<RefusedPairVoiRecord>,
    node_quantities: Vec<CertifiedQuantityInterval>,
    latent_inventories: Vec<CertifiedQuantityInterval>,
    bias_terms: Vec<CertifiedQuantityInterval>,
    cert_balance: BalanceCertificateSummary,
    cert_identifiability: IdentifiabilityCertificateSummary,
    review_target_digest: String,
}

impl EvaluatedPosterior {
    fn build_certificate(
        &self,
        attestation: AttestationStatus,
        require_admitted_balance: bool,
    ) -> Result<ReconciliationCertificate, Refusal> {
        ReconciliationCertificate::build(ReconciliationCertificateRequest {
            period_id: self.period.clone(),
            site_id: self.site.clone(),
            evidence_cut_id: self.evidence_cut_id.clone(),
            error_model_version: self.error_model_version.clone(),
            code_revision: self.code_revision.clone(),
            determinism_class: DeterminismClass::DeterministicPerIsa,
            seed_root: self.seed_root,
            ledger_anchor: None,
            inputs: vec![InputDigest::new(
                "worksheet",
                self.input_locator.clone(),
                self.input_sha256,
            )],
            parity_notes: self.parity_notes.clone(),
            balance: self.cert_balance.clone(),
            posteriors: PosteriorCertificateSummary {
                node_quantities: self.node_quantities.clone(),
                f_factors: self.f_factors.clone(),
                bias_terms: self.bias_terms.clone(),
                latent_inventories: self.latent_inventories.clone(),
            },
            identifiability: self.cert_identifiability.clone(),
            forecast_scores: self.forecast_scores.clone(),
            attestation,
            require_admitted_balance,
        })
        .map_err(|e| e.to_refusal())
    }
}

fn evaluate_posterior(
    text: &str,
    raw_sha256: Digest32,
    code_revision_override: Option<&str>,
) -> Result<EvaluatedPosterior, Refusal> {
    let doc = parse_subset(text).map_err(|e| input_err(e.to_string()))?;
    check_keys(
        &doc.top,
        &[
            "schema",
            "site",
            "period",
            "evidence_cut_id",
            "error_model_version",
            "ownership_basis",
            "confidence_level",
            "global_alpha",
            "measurement_alpha",
        ],
        &[
            "unit_error_value_usd",
            "seed_root",
            "surprise_alarm_active",
            "code_revision",
        ],
    )?;
    if req_str(&doc.top, "schema")? != POSTERIOR_SCHEMA {
        return Err(input_err(format!(
            "expected schema = \"{POSTERIOR_SCHEMA}\""
        )));
    }

    let site = req_str(&doc.top, "site")?.to_string();
    let period = req_str(&doc.top, "period")?.to_string();
    let evidence_cut_id = req_str(&doc.top, "evidence_cut_id")?.to_string();
    let error_model_version = req_str(&doc.top, "error_model_version")?.to_string();
    let ownership_basis = req_str(&doc.top, "ownership_basis")?.to_string();
    let confidence_level = req_f64(&doc.top, "confidence_level")?;
    if !(confidence_level > 0.0 && confidence_level < 1.0) {
        return Err(domain_err("confidence_level must lie strictly in (0, 1)"));
    }
    let global_alpha = req_f64(&doc.top, "global_alpha")?;
    let measurement_alpha = req_f64(&doc.top, "measurement_alpha")?;
    let unit_error_value_usd = opt_f64(&doc.top, "unit_error_value_usd", 0.0)?;
    let seed_root = opt_u64(&doc.top, "seed_root", 0)?;
    let surprise_alarm_active = parse_bool_field(&doc.top, "surprise_alarm_active", Some(false))?;
    let code_revision = match code_revision_override {
        Some(rev) => rev.to_string(),
        None => opt_str(&doc.top, "code_revision")?
            .unwrap_or(DEFAULT_CODE_REVISION)
            .to_string(),
    };

    let mut id_to_key: BTreeMap<String, VariableKey> = BTreeMap::new();
    let mut measurements: Vec<MeasuredVariable> = Vec::new();
    let mut covariances: Vec<CovarianceOffDiagonal> = Vec::new();
    let mut constraints: Vec<NamedLinearConstraint> = Vec::new();
    let mut factor_specs: Vec<FactorSpec> = Vec::new();
    let mut seen_factor_keys: BTreeSet<String> = BTreeSet::new();
    let mut parity_notes: Vec<ParityExplanation> = Vec::new();
    let mut bias_groups: Vec<HierarchicalGroupPrior> = Vec::new();
    let mut bias_nodes: Vec<NodeBiasSpec> = Vec::new();
    let mut bias_obs: Vec<BiasObservation> = Vec::new();
    let mut voi_specs: Vec<VoiCandSpec> = Vec::new();
    let mut forecast_scores: Vec<RegisteredForecastScore> = Vec::new();
    let mut seen_forecast_ids: BTreeSet<String> = BTreeSet::new();

    for row in &doc.rows {
        let f = &row.fields;
        match row.table.as_str() {
            "variable" => {
                check_keys(f, &["id", "channel", "observed", "std_dev"], &["kind"])?;
                let id = req_str(f, "id")?.to_string();
                let channel = parse_channel(req_str(f, "channel")?, &id)?;
                let observed = req_f64(f, "observed")?;
                let std_dev = req_f64(f, "std_dev")?;
                let kind = opt_str(f, "kind")?.unwrap_or("stream");
                let key = match kind {
                    "stream" => VariableKey::Stream {
                        stream_id: id.clone(),
                        channel,
                    },
                    "inventory_delta" => VariableKey::InventoryDelta {
                        node_id: id.clone(),
                        channel,
                    },
                    "named" => VariableKey::Named {
                        name: id.clone(),
                        channel,
                    },
                    other => {
                        return Err(domain_err(format!(
                            "line {}: unknown variable kind '{other}'",
                            row.line
                        )));
                    }
                };
                if id_to_key.insert(id.clone(), key.clone()).is_some() {
                    return Err(input_err(format!(
                        "line {}: duplicate variable id '{id}'",
                        row.line
                    )));
                }
                measurements.push(MeasuredVariable {
                    key,
                    observed,
                    std_dev,
                });
            }
            "covariance" => {
                check_keys(f, &["first", "second", "value"], &[])?;
                let first_id = req_str(f, "first")?;
                let second_id = req_str(f, "second")?;
                let value = req_f64(f, "value")?;
                let first = id_to_key.get(first_id).cloned().ok_or_else(|| {
                    input_err(format!(
                        "line {}: covariance references undeclared variable '{first_id}'",
                        row.line
                    ))
                })?;
                let second = id_to_key.get(second_id).cloned().ok_or_else(|| {
                    input_err(format!(
                        "line {}: covariance references undeclared variable '{second_id}'",
                        row.line
                    ))
                })?;
                covariances.push(CovarianceOffDiagonal {
                    a: first,
                    b: second,
                    covariance: value,
                });
            }
            "constraint" => {
                check_keys(f, &["label", "terms", "target"], &[])?;
                let label = req_str(f, "label")?.to_string();
                let raw_terms = parse_terms(req_str(f, "terms")?, "constraint.terms")?;
                let target = req_f64(f, "target")?;
                let mut terms = Vec::with_capacity(raw_terms.len());
                for (vid, coeff) in raw_terms {
                    let key = id_to_key.get(&vid).cloned().ok_or_else(|| {
                        input_err(format!(
                            "line {}: constraint '{label}' references undeclared variable '{vid}'",
                            row.line
                        ))
                    })?;
                    terms.push((key, coeff));
                }
                constraints.push(NamedLinearConstraint {
                    label,
                    terms,
                    target,
                });
            }
            "f_factor" => {
                check_keys(
                    f,
                    &[
                        "registry_key",
                        "definition_version",
                        "numerator",
                        "denominator",
                    ],
                    &[],
                )?;
                let registry_key = req_str(f, "registry_key")?.to_string();
                if !seen_factor_keys.insert(registry_key.clone()) {
                    return Err(input_err(format!(
                        "line {}: duplicate f_factor registry_key '{registry_key}'",
                        row.line
                    )));
                }
                let definition_version = match get_field(f, "definition_version") {
                    Some(TomlValue::Str(s)) => s.clone(),
                    _ => String::new(),
                };
                if definition_version.trim().is_empty() {
                    return Err(Refusal::new(
                        RefusalCode::UndeclaredUnitBasisOrTz107,
                        format!(
                            "line {}: G-TOUCH-4 requires non-empty definition_version for F-factor '{registry_key}'",
                            row.line
                        ),
                    ));
                }
                if definition_version.len() > 4096
                    || definition_version.trim() != definition_version
                    || definition_version.chars().any(char::is_control)
                {
                    return Err(input_err(format!(
                        "line {}: f_factor '{registry_key}' definition_version must be bounded text without edge whitespace or control chars",
                        row.line
                    )));
                }
                factor_specs.push(FactorSpec {
                    registry_key,
                    definition_version,
                    numerator: req_str(f, "numerator")?.to_string(),
                    denominator: req_str(f, "denominator")?.to_string(),
                });
            }
            "parity" => {
                check_keys(
                    f,
                    &[
                        "metric_id",
                        "site_value",
                        "recomputed_value",
                        "status",
                        "rationale",
                    ],
                    &[],
                )?;
                let status = match req_str(f, "status")? {
                    "explained" => ParityDifferenceStatus::Explained,
                    "refused" => ParityDifferenceStatus::Refused,
                    other => {
                        return Err(domain_err(format!(
                            "line {}: parity status must be 'explained' or 'refused', got '{other}'",
                            row.line
                        )));
                    }
                };
                parity_notes.push(ParityExplanation {
                    metric_id: req_str(f, "metric_id")?.to_string(),
                    site_value: req_f64(f, "site_value")?,
                    recomputed_value: req_f64(f, "recomputed_value")?,
                    status,
                    rationale: req_str(f, "rationale")?.to_string(),
                });
            }
            "bias_group" => {
                check_keys(
                    f,
                    &[
                        "group_id",
                        "source",
                        "target",
                        "hyper_mean",
                        "hyper_sd",
                        "between_node_sd",
                    ],
                    &[],
                )?;
                bias_groups.push(HierarchicalGroupPrior {
                    group_id: req_str(f, "group_id")?.to_string(),
                    source: parse_mining_source(req_str(f, "source")?)?,
                    target: parse_bias_target(req_str(f, "target")?)?,
                    hyper_mean: req_f64(f, "hyper_mean")?,
                    hyper_sd: req_f64(f, "hyper_sd")?,
                    between_node_sd: req_f64(f, "between_node_sd")?,
                    refclass_id: None,
                });
            }
            "bias_node" => {
                check_keys(
                    f,
                    &["node_id", "source", "target"],
                    &["group_id", "prior_mean", "prior_sd"],
                )?;
                let group_id = opt_str(f, "group_id")?.map(ToString::to_string);
                let standalone_prior =
                    if get_field(f, "prior_mean").is_some() || get_field(f, "prior_sd").is_some() {
                        Some(StandaloneNodePrior {
                            mean: req_f64(f, "prior_mean")?,
                            std_dev: req_f64(f, "prior_sd")?,
                        })
                    } else {
                        None
                    };
                bias_nodes.push(NodeBiasSpec {
                    node_id: req_str(f, "node_id")?.to_string(),
                    source: parse_mining_source(req_str(f, "source")?)?,
                    target: parse_bias_target(req_str(f, "target")?)?,
                    group_id,
                    standalone_prior,
                    refclass_id: None,
                });
            }
            "bias_obs" => {
                check_keys(
                    f,
                    &["label", "terms", "observed", "obs_sd"],
                    &["period_index"],
                )?;
                let period_index_u64 = opt_u64(f, "period_index", 0)?;
                let period_index = u32::try_from(period_index_u64).map_err(|_| {
                    input_err(format!(
                        "line {}: period_index {period_index_u64} exceeds u32::MAX",
                        row.line
                    ))
                })?;
                bias_obs.push(BiasObservation {
                    label: req_str(f, "label")?.to_string(),
                    period_index,
                    terms: parse_terms(req_str(f, "terms")?, "bias_obs.terms")?,
                    observed: req_f64(f, "observed")?,
                    obs_sd: req_f64(f, "obs_sd")?,
                });
            }
            "voi_candidate" => {
                check_keys(
                    f,
                    &["id", "kind", "description", "terms", "obs_sd", "cost_usd"],
                    &[],
                )?;
                voi_specs.push(VoiCandSpec {
                    id: req_str(f, "id")?.to_string(),
                    kind: parse_voi_kind(req_str(f, "kind")?)?,
                    description: req_str(f, "description")?.to_string(),
                    terms: parse_terms(req_str(f, "terms")?, "voi_candidate.terms")?,
                    obs_sd: req_f64(f, "obs_sd")?,
                    cost_usd: req_f64(f, "cost_usd")?,
                });
            }
            "forecast_score" => {
                check_keys(
                    f,
                    &[
                        "forecast_id",
                        "forecast_digest_hex",
                        "realized_mean",
                        "interval_covered",
                        "crps",
                    ],
                    &[],
                )?;
                let forecast_id = req_str(f, "forecast_id")?.to_string();
                if !seen_forecast_ids.insert(forecast_id.clone()) {
                    return Err(input_err(format!(
                        "line {}: duplicate forecast_score forecast_id '{forecast_id}'",
                        row.line
                    )));
                }
                let forecast_digest_hex = req_str(f, "forecast_digest_hex")?.to_string();
                if !is_lower_hex_64(&forecast_digest_hex) {
                    return Err(input_err(format!(
                        "line {}: forecast_score '{forecast_id}' forecast_digest_hex must be 64 lowercase hex characters",
                        row.line
                    )));
                }
                forecast_scores.push(RegisteredForecastScore {
                    forecast_id,
                    forecast_digest_hex,
                    realized_mean: req_f64(f, "realized_mean")?,
                    interval_covered: parse_bool_field(f, "interval_covered", None)?,
                    crps: req_f64(f, "crps")?,
                });
            }
            other => {
                return Err(input_err(format!(
                    "line {}: unexpected table '[[{other}]]'",
                    row.line
                )));
            }
        }
    }

    if measurements.is_empty() || constraints.is_empty() {
        return Err(input_err(
            "posterior reconciliation requires at least one [[variable]] and one [[constraint]]",
        ));
    }

    let linear = reconcile_named_system(&NamedLinearRequest {
        measurements,
        off_diagonal_covariances: covariances,
        constraints,
    })
    .map_err(|e| e.to_refusal())?;

    let policy = GrossErrorPolicy {
        global_alpha,
        measurement_alpha,
        multiplicity: MultiplicityCorrection::Sidak,
        target_power: 0.80,
        periods_per_quarter: 3,
    };
    let gross_error = evaluate_gross_errors(&linear, &policy).map_err(|e| e.to_refusal())?;

    let z_crit = det::norm_ppf(0.5 + 0.5 * confidence_level);
    let n_vars = linear.variables.len();

    let mut node_quantities = Vec::new();
    let mut latent_inventories = Vec::new();
    for (idx, key) in linear.variables.iter().enumerate() {
        let mean = linear.estimates[idx];
        let sd = linear.posterior_std_devs[idx];
        let ci_lower = mean - z_crit * sd;
        let ci_upper = mean + z_crit * sd;
        if !(mean.is_finite() && sd.is_finite() && ci_lower.is_finite() && ci_upper.is_finite()) {
            return Err(Refusal::new(
                RefusalCode::NumericalCertificateFailed207,
                format!(
                    "non-finite posterior interval on variable '{}'",
                    var_label(key)
                ),
            ));
        }
        let entry = CertifiedQuantityInterval {
            id: var_label(key),
            mean,
            std_dev: sd,
            ci_lower,
            ci_upper,
        };
        if matches!(key, VariableKey::InventoryDelta { .. }) {
            latent_inventories.push(entry);
        } else {
            node_quantities.push(entry);
        }
    }

    // Compute F-factors + delta-method intervals from full reconciled posterior covariance.
    let mut f_factors = Vec::with_capacity(factor_specs.len());
    for fs in &factor_specs {
        let num_key = id_to_key.get(&fs.numerator).ok_or_else(|| {
            input_err(format!(
                "f_factor '{}' references unknown numerator '{}'",
                fs.registry_key, fs.numerator
            ))
        })?;
        let den_key = id_to_key.get(&fs.denominator).ok_or_else(|| {
            input_err(format!(
                "f_factor '{}' references unknown denominator '{}'",
                fs.registry_key, fs.denominator
            ))
        })?;
        let i_num = linear.index_of(num_key).ok_or_else(|| {
            domain_err(format!(
                "f_factor '{}' numerator missing from solution",
                fs.registry_key
            ))
        })?;
        let i_den = linear.index_of(den_key).ok_or_else(|| {
            domain_err(format!(
                "f_factor '{}' denominator missing from solution",
                fs.registry_key
            ))
        })?;
        let mu_n = linear.estimates[i_num];
        let mu_d = linear.estimates[i_den];
        if !(mu_d.is_finite() && mu_d > 0.0) {
            return Err(domain_err(format!(
                "f_factor '{}' reconciled denominator '{}' must be strictly positive, got {mu_d}",
                fs.registry_key, fs.denominator
            )));
        }
        let ratio = mu_n / mu_d;
        let v_nn = linear.posterior_covariance[i_num * n_vars + i_num];
        let v_dd = linear.posterior_covariance[i_den * n_vars + i_den];
        let v_nd = linear.posterior_covariance[i_num * n_vars + i_den];
        let g_n = 1.0 / mu_d;
        let g_d = -mu_n / (mu_d * mu_d);
        let var_r = (g_n * g_n * v_nn + 2.0 * g_n * g_d * v_nd + g_d * g_d * v_dd).max(0.0);
        let sd_r = det::sqrt(var_r);
        let ci_lower = ratio - z_crit * sd_r;
        let ci_upper = ratio + z_crit * sd_r;
        if !(ratio.is_finite()
            && g_n.is_finite()
            && g_d.is_finite()
            && var_r.is_finite()
            && sd_r.is_finite()
            && ci_lower.is_finite()
            && ci_upper.is_finite())
        {
            return Err(Refusal::new(
                RefusalCode::NumericalCertificateFailed207,
                format!(
                    "f_factor '{}' delta-method ratio/variance overflowed binary64 range",
                    fs.registry_key
                ),
            ));
        }
        f_factors.push(CertifiedFactorInterval {
            registry_key: fs.registry_key.clone(),
            definition_version: fs.definition_version.clone(),
            mean: ratio,
            ci_lower,
            ci_upper,
        });
    }

    // Optional hierarchical bias posteriors + identifiability + Measurement VOI.
    let mut bias_fit = None;
    let mut identifiability = None;
    let mut voi_scores = Vec::new();
    let mut refused_pairs = Vec::new();
    let mut bias_terms = Vec::new();

    if !bias_nodes.is_empty() {
        let fit = fit_hierarchical_bias_posteriors(&HierarchicalBiasRequest {
            evidence_cut_id: evidence_cut_id.clone(),
            confidence_level,
            surprise_alarm_active,
            groups: bias_groups,
            nodes: bias_nodes,
            observations: bias_obs,
        })
        .map_err(|e| e.to_refusal())?;

        for p in &fit.node_posteriors {
            bias_terms.push(CertifiedQuantityInterval {
                id: format!("bias:{}", p.id),
                mean: p.posterior_mean,
                std_dev: p.posterior_sd,
                ci_lower: p.ci_lower,
                ci_upper: p.ci_upper,
            });
        }

        let id_rep = analyze_hierarchical_identifiability(&fit).map_err(|e| e.to_refusal())?;
        if !voi_specs.is_empty() {
            let k = id_rep.parameter_count;
            let mut candidates = Vec::with_capacity(voi_specs.len());
            for vc in &voi_specs {
                let mut design_row = vec![0.0_f64; k];
                for (pid, coeff) in &vc.terms {
                    let Some(pos) = id_rep.parameter_names.iter().position(|n| n == pid) else {
                        return Err(input_err(format!(
                            "voi_candidate '{}' references unknown bias node '{pid}'",
                            vc.id
                        )));
                    };
                    design_row[pos] += *coeff;
                }
                candidates.push(CandidateMeasurement {
                    id: vc.id.clone(),
                    kind: vc.kind,
                    description: vc.description.clone(),
                    design_row,
                    obs_sd: vc.obs_sd,
                    cost_usd: vc.cost_usd,
                });
            }
            voi_scores = evaluate_measurement_voi(&MeasurementVoiRequest {
                baseline: &id_rep,
                unit_error_value_usd,
                target_indices: Vec::new(),
                candidates,
            })
            .map_err(|e| e.to_refusal())?;
        }

        let best_sep = voi_scores.first().map_or_else(
            || ("none_declared".to_string(), 0.0_f64),
            |s| (s.id.clone(), s.net_voi_usd),
        );
        let contrast = id_rep
            .identifiable_contrasts
            .first()
            .map_or_else(|| "none".to_string(), |c| c.formula.clone());
        for pair in &id_rep.confounded_pairs {
            refused_pairs.push(RefusedPairVoiRecord {
                first: pair.first.clone(),
                second: pair.second.clone(),
                correlation: pair.correlation,
                determined_contrast: contrast.clone(),
                separating_measurement: best_sep.0.clone(),
                net_voi_usd: best_sep.1,
            });
        }

        bias_fit = Some(fit);
        identifiability = Some(id_rep);
    } else if !bias_groups.is_empty() || !bias_obs.is_empty() || !voi_specs.is_empty() {
        return Err(input_err(
            "[[bias_group]], [[bias_obs]], or [[voi_candidate]] requires at least one [[bias_node]]",
        ));
    }

    let node_residuals: Vec<NodeBalanceRecord> = linear
        .constraint_names
        .iter()
        .enumerate()
        .map(|(i, lbl)| NodeBalanceRecord {
            node_label: lbl.clone(),
            raw_residual: linear.raw_residuals[i],
            reconciled_residual: linear.reconciled_residuals[i],
        })
        .collect();
    let flagged_suspects: Vec<String> = gross_error
        .suspect_ranking
        .iter()
        .filter(|s| s.flagged)
        .map(|s| var_label(&s.variable))
        .collect();
    let degrees_of_freedom = u32::try_from(gross_error.degrees_of_freedom).map_err(|_| {
        Refusal::new(
            RefusalCode::NumericalCertificateFailed207,
            "degrees_of_freedom exceeds u32::MAX",
        )
    })?;
    let cert_balance = BalanceCertificateSummary {
        node_residuals,
        chi_square: gross_error.global_chi_square,
        degrees_of_freedom,
        global_p_value: gross_error.global_p_value,
        max_relative_closure_error: linear.max_relative_closure_error,
        disposition: gross_error.disposition,
        flagged_suspects,
    };
    let cert_identifiability = match &identifiability {
        Some(rep) => IdentifiabilityCertificateSummary {
            observation_rank: u32::try_from(rep.observation_rank).map_err(|_| {
                Refusal::new(
                    RefusalCode::NumericalCertificateFailed207,
                    "observation_rank exceeds u32::MAX",
                )
            })?,
            parameter_count: u32::try_from(rep.parameter_count).map_err(|_| {
                Refusal::new(
                    RefusalCode::NumericalCertificateFailed207,
                    "parameter_count exceeds u32::MAX",
                )
            })?,
            refused_pairs: refused_pairs.clone(),
        },
        None => IdentifiabilityCertificateSummary {
            observation_rank: 0,
            parameter_count: 0,
            refused_pairs: Vec::new(),
        },
    };
    let input_locator = format!(
        "{}_{}_input.toml",
        sanitize_token(&site),
        sanitize_token(&period)
    );

    let mut eval = EvaluatedPosterior {
        site,
        period,
        evidence_cut_id,
        error_model_version,
        ownership_basis,
        code_revision,
        seed_root,
        confidence_level,
        input_locator,
        input_sha256: raw_sha256,
        linear,
        gross_error,
        f_factors,
        parity_notes,
        forecast_scores,
        bias_fit,
        identifiability,
        voi_scores,
        refused_pairs,
        node_quantities,
        latent_inventories,
        bias_terms,
        cert_balance,
        cert_identifiability,
        review_target_digest: String::new(),
    };
    let preview = eval.build_certificate(AttestationStatus::DraftUnreviewed, false)?;
    eval.review_target_digest = preview.replay_digest_blake3.to_hex();
    Ok(eval)
}

fn render_markdown(
    eval: &EvaluatedPosterior,
    anchor: &SourceAnchor,
    closed_cert: Option<&ReconciliationCertificate>,
) -> String {
    let mut md = String::new();
    let heading = if closed_cert.is_some() {
        "Touchstone Reconciliation Close Certificate"
    } else {
        "Touchstone Posterior Reconciliation Run"
    };
    let _ = writeln!(md, "# {heading}\n");
    let _ = writeln!(
        md,
        "{} · {} · cut `{}` · error model `{}` · {}\n",
        markdown_cell(&eval.site),
        markdown_cell(&eval.period),
        markdown_cell(&eval.evidence_cut_id),
        markdown_cell(&eval.error_model_version),
        markdown_cell(&eval.ownership_basis),
    );
    let disp_str = match eval.gross_error.disposition {
        CommitDisposition::Admit => "admit",
        CommitDisposition::RefuseForInvestigation => "refuse_for_investigation",
    };
    let _ = writeln!(
        md,
        "## Balance & Gross-Error Diagnostics\n\n- Global chi-square: `{:.4}` (df = {}, p = `{:.4}`, crit = `{:.4}`)\n- Max relative closure error: `{:.3e}` (`PO-TOUCH-1`)\n- Commit disposition (`INV-005`): `{disp_str}`\n",
        eval.gross_error.global_chi_square,
        eval.gross_error.degrees_of_freedom,
        eval.gross_error.global_p_value,
        eval.gross_error.global_critical_value,
        eval.linear.max_relative_closure_error,
    );
    md.push_str(
        "\n| Variable | Observed | Reconciled | Adjustment | Posterior SD | Norm Adj (z) |\n|---|---:|---:|---:|---:|---:|\n",
    );
    for (i, key) in eval.linear.variables.iter().enumerate() {
        let z_str = eval.linear.normalized_adjustments[i]
            .map_or_else(|| "n/a".to_string(), |z| format!("{z:+.3}"));
        let _ = writeln!(
            md,
            "| {} | {:.4} | {:.4} | {:+.4} | {:.4} | {} |",
            markdown_cell(&var_label(key)),
            eval.linear.observed[i],
            eval.linear.estimates[i],
            eval.linear.adjustments[i],
            eval.linear.posterior_std_devs[i],
            z_str,
        );
    }

    if !eval.f_factors.is_empty() {
        md.push_str("\n## F-Factors (`G-TOUCH-4`)\n\n| Registry Key | Version | Mean | CI Lower | CI Upper |\n|---|---|---:|---:|---:|\n");
        for ff in &eval.f_factors {
            let _ = writeln!(
                md,
                "| {} | {} | {:.5} | {:.5} | {:.5} |",
                markdown_cell(&ff.registry_key),
                markdown_cell(&ff.definition_version),
                ff.mean,
                ff.ci_lower,
                ff.ci_upper,
            );
        }
    }

    if let Some(id_rep) = &eval.identifiability {
        let _ = writeln!(
            md,
            "\n## Bias Posteriors & Identifiability (`G-TOUCH-2`)\n\n- Observation operator rank: `{}/{}` (fully identifiable: `{}`)\n",
            id_rep.observation_rank, id_rep.parameter_count, id_rep.is_fully_identifiable
        );
        if !eval.refused_pairs.is_empty() {
            md.push_str(
                "\n| Confounded Pair | Corr | Determined Contrast | Separating Measurement | Net VOI (USD) |\n|---|---:|---|---|---:|\n",
            );
            for rp in &eval.refused_pairs {
                let _ = writeln!(
                    md,
                    "| {} / {} | {:.3} | {} | {} | {:.2} |",
                    markdown_cell(&rp.first),
                    markdown_cell(&rp.second),
                    rp.correlation,
                    markdown_cell(&rp.determined_contrast),
                    markdown_cell(&rp.separating_measurement),
                    rp.net_voi_usd,
                );
            }
        }
    }

    let _ = writeln!(
        md,
        "\n## Digest & Attestation\n\n- Review target digest (BLAKE3): `{}`",
        eval.review_target_digest
    );
    if let Some(cert) = closed_cert {
        let _ = writeln!(
            md,
            "- Sealed certificate replay digest (BLAKE3): `{}`\n- Sealed payload SHA-256: `{}`",
            cert.replay_digest_blake3.to_hex(),
            cert.payload_sha256.to_hex(),
        );
        match &cert.attestation {
            AttestationStatus::DraftUnreviewed => {
                md.push_str("- Attestation: `draft_unreviewed`\n");
            }
            AttestationStatus::ReviewedAgainst {
                reviewer_role,
                against_digest_hex,
            } => {
                let _ = writeln!(
                    md,
                    "- Attestation: `reviewed_against` by `{}` against `{}`",
                    markdown_cell(reviewer_role),
                    against_digest_hex
                );
            }
        }
    }
    let _ = writeln!(
        md,
        "\nSource {} (SHA-256 `{}`).\n\n{POSTERIOR_NO_CLAIM}",
        markdown_cell(&anchor.artifact_path),
        Digest32(anchor.artifact_sha256)
    );
    md
}

fn posterior_core_json(eval: &EvaluatedPosterior, source_json: &str) -> String {
    let disp = match eval.gross_error.disposition {
        CommitDisposition::Admit => "admit",
        CommitDisposition::RefuseForInvestigation => "refuse_for_investigation",
    };
    let vars_json: Vec<String> = eval
        .linear
        .variables
        .iter()
        .enumerate()
        .map(|(i, k)| {
            let z = eval.linear.normalized_adjustments[i]
                .map_or_else(|| "null".to_string(), json_f64);
            format!(
                "{{\"variable\":{},\"observed\":{},\"reconciled\":{},\"adjustment\":{},\"posterior_sd\":{},\"normalized_adjustment\":{z}}}",
                quote(&var_label(k)),
                json_f64(eval.linear.observed[i]),
                json_f64(eval.linear.estimates[i]),
                json_f64(eval.linear.adjustments[i]),
                json_f64(eval.linear.posterior_std_devs[i]),
            )
        })
        .collect();
    let ff_json: Vec<String> = eval
        .f_factors
        .iter()
        .map(|f| {
            format!(
                "{{\"registry_key\":{},\"definition_version\":{},\"mean\":{},\"ci_lower\":{},\"ci_upper\":{}}}",
                quote(&f.registry_key),
                quote(&f.definition_version),
                json_f64(f.mean),
                json_f64(f.ci_lower),
                json_f64(f.ci_upper),
            )
        })
        .collect();
    let refused_json: Vec<String> = eval
        .refused_pairs
        .iter()
        .map(|rp| {
            format!(
                "{{\"first\":{},\"second\":{},\"correlation\":{},\"refusal_code\":\"FMINE-REF-104\",\"determined_contrast\":{},\"separating_measurement\":{},\"net_voi_usd\":{}}}",
                quote(&rp.first),
                quote(&rp.second),
                json_f64(rp.correlation),
                quote(&rp.determined_contrast),
                quote(&rp.separating_measurement),
                json_f64(rp.net_voi_usd),
            )
        })
        .collect();
    let mdb_json: Vec<String> = eval
        .gross_error
        .detectable_biases
        .iter()
        .map(|m| {
            let q_pct = m
                .measurement_mdb_quarter_pct
                .map_or_else(|| "null".to_string(), json_f64);
            format!(
                "{{\"variable\":{},\"single_period_mdb\":{},\"quarter_mdb\":{},\"quarter_mdb_pct\":{q_pct}}}",
                quote(&var_label(&m.variable)),
                json_f64(m.measurement_mdb_single_period),
                json_f64(m.measurement_mdb_quarter),
            )
        })
        .collect();
    let voi_json: Vec<String> = eval
        .voi_scores
        .iter()
        .map(|s| {
            format!(
                "{{\"id\":{},\"kind\":{},\"resolves_identifiability\":{},\"post_rank\":{},\"max_abs_corr_after\":{},\"expected_value_usd\":{},\"cost_usd\":{},\"net_voi_usd\":{}}}",
                quote(&s.id),
                quote(s.kind.as_str()),
                s.resolves_identifiability,
                s.post_rank,
                json_f64(s.max_abs_corr_after),
                json_f64(s.expected_value_usd),
                json_f64(s.cost_usd),
                json_f64(s.net_voi_usd),
            )
        })
        .collect();
    let bias_digest = eval
        .bias_fit
        .as_ref()
        .map_or_else(|| "null".to_string(), |b| quote(&b.update_event.digest_hex));

    format!(
        "\"source\":{source_json},\"site\":{},\"period\":{},\"evidence_cut_id\":{},\"error_model_version\":{},\"ownership_basis\":{},\"confidence_level\":{},\"linear_digest_blake3\":{},\"bias_digest_blake3\":{bias_digest},\"review_target_digest\":{},\"balance\":{{\"chi_square\":{},\"degrees_of_freedom\":{},\"global_p_value\":{},\"global_critical_value\":{},\"max_relative_closure_error\":{},\"disposition\":{},\"flagged_suspects\":[{}]}},\"variables\":[{}],\"detectable_biases\":[{}],\"f_factors\":[{}],\"identifiability\":{{\"observation_rank\":{},\"parameter_count\":{},\"refused_pairs\":[{}],\"measurement_voi\":[{}]}},\"authority_flags\":{{\"reserve_disclosure_claim\":false,\"execution_authorized\":false,\"geotechnical_signoff\":false,\"regulatory_compliance_claim\":false}}",
        quote(&eval.site),
        quote(&eval.period),
        quote(&eval.evidence_cut_id),
        quote(&eval.error_model_version),
        quote(&eval.ownership_basis),
        json_f64(eval.confidence_level),
        quote(&eval.linear.digest_hex),
        quote(&eval.review_target_digest),
        json_f64(eval.gross_error.global_chi_square),
        eval.gross_error.degrees_of_freedom,
        json_f64(eval.gross_error.global_p_value),
        json_f64(eval.gross_error.global_critical_value),
        json_f64(eval.linear.max_relative_closure_error),
        quote(disp),
        eval.cert_balance
            .flagged_suspects
            .iter()
            .map(|s| quote(s))
            .collect::<Vec<_>>()
            .join(","),
        vars_json.join(","),
        mdb_json.join(","),
        ff_json.join(","),
        eval.cert_identifiability.observation_rank,
        eval.cert_identifiability.parameter_count,
        refused_json.join(","),
        voi_json.join(","),
    )
}

pub(super) fn run_posterior(
    text: &str,
    anchor: &SourceAnchor,
    input_path: &Path,
) -> Result<Envelope, Refusal> {
    let raw_sha = Digest32(anchor.artifact_sha256);
    let eval = evaluate_posterior(text, raw_sha, None)?;
    let source_json = worksheet_input::source_json(anchor, text.len());
    let core = posterior_core_json(&eval, &source_json);
    let payload = format!("{{\"schema\":\"fmine.recon-posterior-result.v1\",{core}}}");
    let markdown = render_markdown(&eval, anchor, None);
    let next = match eval.gross_error.disposition {
        CommitDisposition::Admit => vec![NextAction::new(
            format!(
                "fmine recon close --input {} --out-dir <out-dir> --reviewer-role <role> --against-digest {}",
                input_path.display(),
                eval.review_target_digest
            ),
            "Seal attested ReconciliationCertificate and FMT-CERT-1 after human review of this digest",
        )],
        CommitDisposition::RefuseForInvestigation => vec![NextAction::new(
            "Investigate flagged gross-error suspects before attempting recon close",
            "INV-005 blocks sealing periods with Reject/RefuseForInvestigation disposition (FMINE-REF-101)",
        )],
    };
    let mut env = Envelope::ok("recon run", payload, next);
    env.rendered_markdown = Some(markdown);
    env.no_claim = Some(POSTERIOR_NO_CLAIM);
    Ok(env)
}

pub fn execute_close(cmd: &CloseCommand, root: &Path) -> Envelope {
    let result = close_inner(cmd, root);
    let mut env = result.unwrap_or_else(|r| Envelope::refused(CLOSE_VERB, r, vec![]));
    env.no_claim = Some(POSTERIOR_NO_CLAIM);
    env
}

fn close_inner(cmd: &CloseCommand, root: &Path) -> Result<Envelope, Refusal> {
    let input_abs = root.join(&cmd.input);
    let (text, anchor) = worksheet_input::read(
        &input_abs,
        "posterior reconciliation close",
        MAX_WORKSHEET_BYTES,
    )?;
    let raw_bytes = fs::read(&input_abs).map_err(|e| {
        input_err(format!(
            "cannot re-read input worksheet '{}': {e}",
            input_abs.display()
        ))
    })?;
    let raw_sha = sha256(&raw_bytes);
    if raw_sha.0 != anchor.artifact_sha256 {
        return Err(Refusal::new(
            RefusalCode::DigestMismatch505,
            "input worksheet changed during reconciliation close read",
        ));
    }

    let eval = evaluate_posterior(&text, raw_sha, cmd.code_revision.as_deref())?;

    // Hard gate INV-005: refuse commit if gross error rejected — write ZERO files.
    enforce_commit_gate(&eval.gross_error).map_err(|e| e.to_refusal())?;

    // Hard attestation gate (§20.9, INV-002): verify reviewer against exact draft digest unless --allow-draft.
    let attestation = if cmd.allow_draft {
        if cmd.reviewer_role.is_some() || cmd.against_digest.is_some() {
            return Err(domain_err(
                "--allow-draft cannot be combined with --reviewer-role or --against-digest",
            ));
        }
        AttestationStatus::DraftUnreviewed
    } else {
        let Some(role) = cmd.reviewer_role.as_deref() else {
            return Err(domain_err(
                "committed close requires --reviewer-role and --against-digest (or explicit --allow-draft)",
            ));
        };
        let Some(against) = cmd.against_digest.as_deref() else {
            return Err(domain_err(
                "committed close requires --against-digest matching `fmine recon run` review_target_digest",
            ));
        };
        if role.trim().is_empty()
            || role.trim() != role
            || role.len() > 4096
            || role.chars().any(char::is_control)
        {
            return Err(domain_err("invalid --reviewer-role value"));
        }
        if !is_ascii_hex_64(against) {
            return Err(Refusal::new(
                RefusalCode::DigestMismatch505,
                format!("--against-digest '{against}' must be a 64-character hex BLAKE3 digest"),
            ));
        }
        if !against.eq_ignore_ascii_case(&eval.review_target_digest) {
            return Err(Refusal::new(
                RefusalCode::DigestMismatch505,
                format!(
                    "--against-digest '{against}' does not match current run review_target_digest '{}'; re-review `fmine recon run` output before closing",
                    eval.review_target_digest
                ),
            ));
        }
        AttestationStatus::ReviewedAgainst {
            reviewer_role: role.to_string(),
            against_digest_hex: eval.review_target_digest.clone(),
        }
    };

    let cert = eval.build_certificate(attestation, true)?;
    let fmt_cert = cert.to_fmt_cert_1().map_err(|e| e.to_refusal())?;
    let rcrt_bytes = cert.to_bytes();
    let fmt_bytes = fmt_cert
        .to_bytes()
        .map_err(|e| domain_err(format!("FMT-CERT-1 encode failed: {e}")))?;

    let out_dir = root.join(&cmd.out_dir);
    let rcrt_path = out_dir.join("recon_certificate.rcrt");
    let cert_path = out_dir.join("recon_certificate.cert");
    let md_path = out_dir.join("recon_certificate.md");
    let inputs_dir = out_dir.join("inputs");
    let input_copy_path = inputs_dir.join(&eval.input_locator);

    if rcrt_path.exists() || cert_path.exists() || md_path.exists() || input_copy_path.exists() {
        return Err(input_err(format!(
            "refusing to overwrite existing reconciliation certificate artifacts in '{}' (append-only close)",
            out_dir.display()
        )));
    }

    fs::create_dir_all(&inputs_dir).map_err(|e| {
        input_err(format!(
            "cannot create close output directory '{}': {e}",
            inputs_dir.display()
        ))
    })?;
    fs::write(&input_copy_path, &raw_bytes).map_err(|e| {
        input_err(format!(
            "cannot write input closure copy '{}': {e}",
            input_copy_path.display()
        ))
    })?;
    fs::write(&rcrt_path, &rcrt_bytes).map_err(|e| {
        input_err(format!(
            "cannot write RCRT certificate '{}': {e}",
            rcrt_path.display()
        ))
    })?;
    fs::write(&cert_path, &fmt_bytes).map_err(|e| {
        input_err(format!(
            "cannot write FMT-CERT-1 certificate '{}': {e}",
            cert_path.display()
        ))
    })?;
    let markdown = render_markdown(&eval, &anchor, Some(&cert));
    fs::write(&md_path, &markdown).map_err(|e| {
        input_err(format!(
            "cannot write Markdown summary '{}': {e}",
            md_path.display()
        ))
    })?;

    let verify_cmd = format!(
        "fmine-verify cert {} --inputs-dir {}",
        cert_path.display(),
        inputs_dir.display()
    );
    let att_json = match &cert.attestation {
        AttestationStatus::DraftUnreviewed => "{\"status\":\"draft_unreviewed\"}".to_string(),
        AttestationStatus::ReviewedAgainst {
            reviewer_role,
            against_digest_hex,
        } => format!(
            "{{\"status\":\"reviewed_against\",\"reviewer_role\":{},\"against_digest_hex\":{}}}",
            quote(reviewer_role),
            quote(against_digest_hex)
        ),
    };
    let source_json = worksheet_input::source_json(&anchor, text.len());
    let core = posterior_core_json(&eval, &source_json);
    let payload = format!(
        "{{\"schema\":\"fmine.recon-close-result.v1\",{core},\"certificate\":{{\"rcrt_path\":{},\"fmt_cert_path\":{},\"inputs_dir\":{},\"markdown_path\":{},\"payload_sha256\":{},\"replay_digest_blake3\":{},\"fmt_cert_digest\":{},\"attestation\":{att_json},\"verify_command\":{}}}}}",
        quote(&rcrt_path.display().to_string()),
        quote(&cert_path.display().to_string()),
        quote(&inputs_dir.display().to_string()),
        quote(&md_path.display().to_string()),
        quote(&cert.payload_sha256.to_hex()),
        quote(&cert.replay_digest_blake3.to_hex()),
        quote(&fmt_cert.certificate_digest.to_hex()),
        quote(&verify_cmd),
    );
    let mut env = Envelope::ok(
        CLOSE_VERB,
        payload,
        vec![NextAction::new(
            verify_cmd,
            "Verify sealed FMT-CERT-1 certificate and input closure offline with fmine-verify",
        )],
    );
    env.rendered_markdown = Some(markdown);
    Ok(env)
}
