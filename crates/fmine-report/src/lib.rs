//! Determinism: BitwisePortable
//! Deterministic plain-Markdown report renderer (`BD-123`, `BD-124`, plan §30.5, §39 WP-06).
//!
//! Operates under the explicitly authorized `FD-FMD-00` fallback (`franken-stack.lock`:
//! `pin = "OPEN"`, fallback `"Plain markdown rendering without diagrams."`, plan §39 BD-123):
//! - Zero external dependencies; renders canonical LF-terminated UTF-8 Markdown.
//! - Every Claim row is checked through `fmine_claim::render::build_fm_row`, enforcing
//!   confidentiality clearance (`INV-007`), canonical `claim_id` integrity (`FMINE-REF-507`),
//!   and G-EXEC numeric suppression (`§7.2`).
//! - Every rendered report embeds canonicalized anchors, certificates, `NO_CLAIM_BANNER`, and all
//!   four `INV-002` authority flags unconditionally `false`.
//! - Rerendering identical inputs — or any permutation of anchor, certificate, or section-claim
//!   inputs — produces byte-identical UTF-8 Markdown and identical SHA-256 / BLAKE3 digests.

use std::fmt::Write as _;

use fmine_claim::Confidentiality;
use fmine_claim::render::{ClaimRenderInput, FmClaimRow, NO_CLAIM_BANNER, build_fm_row};
use fmine_core::codec::Encoder;
use fmine_core::hash::{Digest32, blake3_hash, sha256};
use fmine_evidence::{Refusal, RefusalCode};

/// Canonical report binary-frame and Markdown format version.
pub const REPORT_FORMAT_VERSION: u16 = 1;

/// Explicit renderer tag identifying the authorized `FD-FMD-00` plain-Markdown fallback.
pub const FD_FMD_00_FALLBACK_TAG: &str = "fd-fmd-00-plain-markdown-v1";

/// Mandatory `INV-002` no-claim boundary line carried on every rendered report.
pub const REPORT_NO_CLAIM_BANNER: &str = NO_CLAIM_BANNER;

/// Non-forgable `INV-002` authority flags: every flag is unconditionally `false`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReportAuthorityFlags {
    _private: (),
}

impl Default for ReportAuthorityFlags {
    fn default() -> Self {
        Self::new()
    }
}

impl ReportAuthorityFlags {
    /// Construct the sole valid `INV-002` non-authority flag set (all four flags `false`).
    #[must_use]
    pub const fn new() -> Self {
        Self { _private: () }
    }

    /// Always `false` (`INV-002`).
    #[must_use]
    pub const fn reserve_disclosure_claim(self) -> bool {
        false
    }

    /// Always `false` (`INV-002`).
    #[must_use]
    pub const fn execution_authorized(self) -> bool {
        false
    }

    /// Always `false` (`INV-002`).
    #[must_use]
    pub const fn geotechnical_signoff(self) -> bool {
        false
    }

    /// Always `false` (`INV-002`).
    #[must_use]
    pub const fn regulatory_compliance_claim(self) -> bool {
        false
    }
}

/// Source or ledger anchor cited in a rendered report (`BD-123`).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ReportAnchorRef {
    /// Human-readable label (e.g. `"barrick-q2-2026-minestats"`).
    pub label: String,
    /// Anchor identifier (e.g. `"anc_..."` or ledger high-water mark id).
    pub anchor_id: String,
    /// 64-character lowercase hexadecimal SHA-256 digest of the anchored artifact.
    pub sha256_hex: String,
    /// Exact workbook cell, page, or ledger locator.
    pub locator: String,
}

/// Execution, verification, or solver certificate cited in a rendered report (`BD-123`).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ReportCertificateRef {
    /// Human-readable certificate role (e.g. `"public-assure"`, `"plan-cert"`).
    pub label: String,
    /// Certificate identifier or format tag (e.g. `"FMT-PCERT-1"`).
    pub cert_id: String,
    /// 64-character lowercase hexadecimal digest of the certificate.
    pub digest_hex: String,
    /// Declared determinism class (e.g. `"BitwisePortable"`, `"Exact"`).
    pub determinism_class: String,
}

/// Checked Claim entry ready for deterministic report rendering.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReportClaimEntry {
    pub row: FmClaimRow,
}

impl ReportClaimEntry {
    /// Build a checked report entry from a canonical `Claim<T>`, enforcing confidentiality
    /// clearance (`INV-007`), `claim_id` verification (`FMINE-REF-507`), and §7.2 G-EXEC numeric
    /// suppression.
    pub fn from_claim<T>(
        input: &ClaimRenderInput<'_, T>,
        reader: &Confidentiality,
    ) -> Result<Self, Refusal> {
        let row = build_fm_row(input, reader)?;
        Ok(Self { row })
    }

    /// Wrap an already-checked [`FmClaimRow`].
    #[must_use]
    pub fn from_row(row: FmClaimRow) -> Self {
        Self { row }
    }
}

/// Narrative section in a rendered report containing checked Claim entries and notes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReportSection {
    pub heading: String,
    pub summary: String,
    pub entries: Vec<ReportClaimEntry>,
    pub notes: Vec<String>,
}

/// Complete specification for rendering a deterministic Markdown report (`BD-123`, `BD-124`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReportSpec {
    pub title: String,
    pub subtitle: Option<String>,
    pub lane_label: String,
    pub issuer_disclaimer: Option<String>,
    pub reader_clearance: Confidentiality,
    pub anchors: Vec<ReportAnchorRef>,
    pub certificates: Vec<ReportCertificateRef>,
    pub sections: Vec<ReportSection>,
}

/// Deterministic rendered report output with bound cryptographic digests (`BD-123`, `BD-124`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderedReport {
    pub format_version: u16,
    pub renderer_tag: &'static str,
    pub markdown: String,
    pub canonical_inputs_digest: Digest32,
    pub markdown_sha256: Digest32,
    pub markdown_blake3: Digest32,
    pub authority_flags: ReportAuthorityFlags,
    pub claim_count: usize,
    pub suppressed_claim_count: usize,
    pub anchor_count: usize,
    pub certificate_count: usize,
}

fn is_lower_hex_64(s: &str) -> bool {
    s.len() == 64
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

fn require_non_empty(field: &str, value: &str) -> Result<String, Refusal> {
    let clean = sanitize_inline(value);
    if clean.trim().is_empty() {
        return Err(Refusal::new(
            RefusalCode::UndeclaredUnitBasisOrTz107,
            format!("fmine-report: field '{field}' must be non-empty"),
        ));
    }
    Ok(clean)
}

fn sanitize_inline(s: &str) -> String {
    s.replace("\r\n", " ").replace(['\r', '\n'], " ")
}

fn sanitize_block(s: &str) -> String {
    s.replace("\r\n", "\n").replace('\r', "\n")
}

fn escape_md_cell(s: &str) -> String {
    sanitize_inline(s).replace('|', "\\|")
}

fn escape_json_str(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out
}

fn canonicalize_anchors(raw: &[ReportAnchorRef]) -> Result<Vec<ReportAnchorRef>, Refusal> {
    if raw.is_empty() {
        return Err(Refusal::new(
            RefusalCode::NoClaimInDomain201,
            "fmine-report: at least one ReportAnchorRef is required",
        ));
    }
    let mut out = Vec::with_capacity(raw.len());
    for a in raw {
        let label = require_non_empty("anchor.label", &a.label)?;
        let anchor_id = require_non_empty("anchor.anchor_id", &a.anchor_id)?;
        let locator = require_non_empty("anchor.locator", &a.locator)?;
        if !is_lower_hex_64(&a.sha256_hex) {
            return Err(Refusal::new(
                RefusalCode::UndeclaredUnitBasisOrTz107,
                format!(
                    "fmine-report: anchor '{}' sha256_hex must be 64 lowercase hex chars",
                    anchor_id
                ),
            ));
        }
        out.push(ReportAnchorRef {
            label,
            anchor_id,
            sha256_hex: a.sha256_hex.clone(),
            locator,
        });
    }
    out.sort();
    out.dedup();
    Ok(out)
}

fn canonicalize_certificates(
    raw: &[ReportCertificateRef],
) -> Result<Vec<ReportCertificateRef>, Refusal> {
    if raw.is_empty() {
        return Err(Refusal::new(
            RefusalCode::NoClaimInDomain201,
            "fmine-report: at least one ReportCertificateRef is required",
        ));
    }
    let mut out = Vec::with_capacity(raw.len());
    for c in raw {
        let label = require_non_empty("certificate.label", &c.label)?;
        let cert_id = require_non_empty("certificate.cert_id", &c.cert_id)?;
        let determinism_class =
            require_non_empty("certificate.determinism_class", &c.determinism_class)?;
        if !is_lower_hex_64(&c.digest_hex) {
            return Err(Refusal::new(
                RefusalCode::UndeclaredUnitBasisOrTz107,
                format!(
                    "fmine-report: certificate '{}' digest_hex must be 64 lowercase hex chars",
                    cert_id
                ),
            ));
        }
        out.push(ReportCertificateRef {
            label,
            cert_id,
            digest_hex: c.digest_hex.clone(),
            determinism_class,
        });
    }
    out.sort();
    out.dedup();
    Ok(out)
}

struct NormalizedSection {
    heading: String,
    summary: String,
    rows: Vec<FmClaimRow>,
    notes: Vec<String>,
}

fn normalize_sections(raw: &[ReportSection]) -> Result<Vec<NormalizedSection>, Refusal> {
    let mut out = Vec::with_capacity(raw.len());
    for sec in raw {
        let heading = require_non_empty("section.heading", &sec.heading)?;
        let summary = sanitize_block(&sec.summary).trim().to_string();
        let mut rows: Vec<FmClaimRow> = sec.entries.iter().map(|e| e.row.clone()).collect();
        rows.sort_by(|a, b| {
            a.metric_key
                .cmp(&b.metric_key)
                .then_with(|| a.claim_id.cmp(&b.claim_id))
        });
        let notes: Vec<String> = sec
            .notes
            .iter()
            .map(|n| sanitize_inline(n))
            .filter(|n| !n.trim().is_empty())
            .collect();
        out.push(NormalizedSection {
            heading,
            summary,
            rows,
            notes,
        });
    }
    Ok(out)
}

#[allow(clippy::too_many_arguments)]
fn encode_canonical_inputs(
    title: &str,
    subtitle: Option<&str>,
    lane_label: &str,
    issuer_disclaimer: Option<&str>,
    clearance: &Confidentiality,
    anchors: &[ReportAnchorRef],
    certificates: &[ReportCertificateRef],
    sections: &[NormalizedSection],
) -> Digest32 {
    let mut enc = Encoder::new_frame(*b"RPT1", REPORT_FORMAT_VERSION);
    enc.put_str(FD_FMD_00_FALLBACK_TAG);
    enc.put_str(title);
    enc.put_str(subtitle.unwrap_or(""));
    enc.put_str(lane_label);
    enc.put_str(issuer_disclaimer.unwrap_or(""));
    enc.put_str(&clearance.to_string());
    enc.put_u64(anchors.len() as u64);
    for a in anchors {
        enc.put_str(&a.label);
        enc.put_str(&a.anchor_id);
        enc.put_str(&a.sha256_hex);
        enc.put_str(&a.locator);
    }
    enc.put_u64(certificates.len() as u64);
    for c in certificates {
        enc.put_str(&c.label);
        enc.put_str(&c.cert_id);
        enc.put_str(&c.digest_hex);
        enc.put_str(&c.determinism_class);
    }
    enc.put_u64(sections.len() as u64);
    for s in sections {
        enc.put_str(&s.heading);
        enc.put_str(&s.summary);
        enc.put_u64(s.rows.len() as u64);
        for r in &s.rows {
            enc.put_str(&r.claim_id);
            enc.put_str(&r.metric_key);
            enc.put_str(&r.display_value);
            enc.put_str(&r.unit);
            enc.put_u8(u8::from(r.suppressed));
            enc.put_str(&r.evidence_badge);
            enc.put_str(r.knowledge_state);
            enc.put_str(r.epistemic_axis);
            enc.put_str(&r.color_label);
            enc.put_str(r.calibration_flag);
            enc.put_str(&r.calibration_detail);
            enc.put_str(&r.flip_margin_coded);
            enc.put_str(&r.uu_summary);
            enc.put_u8(u8::from(r.stale));
        }
        enc.put_u64(s.notes.len() as u64);
        for n in &s.notes {
            enc.put_str(n);
        }
    }
    blake3_hash(&enc.finish())
}

/// Render a deterministic plain-Markdown report (`BD-123`, `BD-124`, `FD-FMD-00` fallback).
pub fn render_report(spec: &ReportSpec) -> Result<RenderedReport, Refusal> {
    let title = require_non_empty("title", &spec.title)?;
    let lane_label = require_non_empty("lane_label", &spec.lane_label)?;
    let subtitle = spec
        .subtitle
        .as_deref()
        .map(sanitize_inline)
        .filter(|s| !s.trim().is_empty());
    let issuer_disclaimer = spec
        .issuer_disclaimer
        .as_deref()
        .map(sanitize_inline)
        .filter(|s| !s.trim().is_empty());

    let anchors = canonicalize_anchors(&spec.anchors)?;
    let certificates = canonicalize_certificates(&spec.certificates)?;
    let sections = normalize_sections(&spec.sections)?;

    let canonical_inputs_digest = encode_canonical_inputs(
        &title,
        subtitle.as_deref(),
        &lane_label,
        issuer_disclaimer.as_deref(),
        &spec.reader_clearance,
        &anchors,
        &certificates,
        &sections,
    );

    let flags = ReportAuthorityFlags::new();
    let mut md = String::new();
    let _ = writeln!(md, "# {title}");
    if let Some(sub) = &subtitle {
        let _ = writeln!(md, "\n_{sub}_");
    }
    let _ = writeln!(md);
    let _ = writeln!(
        md,
        "- **Lane:** `{lane_label}` | **Clearance:** `{}` | **Renderer:** `{FD_FMD_00_FALLBACK_TAG}` (plain Markdown, no diagrams)",
        spec.reader_clearance
    );
    if let Some(disc) = &issuer_disclaimer {
        let _ = writeln!(md, "- **Source notice:** {disc}");
    }
    let _ = writeln!(md);
    let _ = writeln!(md, "> {REPORT_NO_CLAIM_BANNER}");
    let _ = writeln!(md);
    let _ = writeln!(
        md,
        "| Authority Flag (`INV-002`) | Value |\n|---|---|\n\
         | `reserve_disclosure_claim` | `{}` |\n\
         | `execution_authorized` | `{}` |\n\
         | `geotechnical_signoff` | `{}` |\n\
         | `regulatory_compliance_claim` | `{}` |",
        flags.reserve_disclosure_claim(),
        flags.execution_authorized(),
        flags.geotechnical_signoff(),
        flags.regulatory_compliance_claim(),
    );

    let _ = writeln!(md, "\n## Anchors\n");
    let _ = writeln!(md, "| Label | Anchor ID | SHA-256 | Locator |");
    let _ = writeln!(md, "|---|---|---|---|");
    for a in &anchors {
        let _ = writeln!(
            md,
            "| {} | `{}` | `{}` | {} |",
            escape_md_cell(&a.label),
            escape_md_cell(&a.anchor_id),
            a.sha256_hex,
            escape_md_cell(&a.locator),
        );
    }

    let _ = writeln!(md, "\n## Certificates\n");
    let _ = writeln!(md, "| Label | Certificate ID | Determinism | Digest |");
    let _ = writeln!(md, "|---|---|---|---|");
    for c in &certificates {
        let _ = writeln!(
            md,
            "| {} | `{}` | `{}` | `{}` |",
            escape_md_cell(&c.label),
            escape_md_cell(&c.cert_id),
            escape_md_cell(&c.determinism_class),
            c.digest_hex,
        );
    }

    let mut claim_count = 0usize;
    let mut suppressed_claim_count = 0usize;

    for sec in &sections {
        let _ = writeln!(md, "\n## {}", sec.heading);
        if !sec.summary.is_empty() {
            let _ = writeln!(md, "\n{}", sec.summary);
        }
        if !sec.rows.is_empty() {
            let _ = writeln!(md, "\n### Executive Summary\n");
            for r in &sec.rows {
                let _ = writeln!(md, "- {}", sanitize_inline(&r.plain_sentence));
            }
            let _ = writeln!(md, "\n### Evidence Table\n");
            let _ = writeln!(
                md,
                "| Metric | Value | Evidence | State | KK/KU/UU | Colour | Calibration | Flip Margin | UU Terms |"
            );
            let _ = writeln!(md, "|---|---|---|---|---|---|---|---|---|");
            for r in &sec.rows {
                claim_count += 1;
                if r.suppressed {
                    suppressed_claim_count += 1;
                }
                let state_cell = if r.stale {
                    format!("{} (STALE)", r.knowledge_state)
                } else {
                    r.knowledge_state.to_string()
                };
                let _ = writeln!(
                    md,
                    "| {} | {} | `{}` | `{}` | `{}` | `{}` | `{}` | {} | {} |",
                    escape_md_cell(&r.metric_key),
                    escape_md_cell(&r.clickable_link),
                    escape_md_cell(&r.evidence_badge),
                    state_cell,
                    r.epistemic_axis,
                    escape_md_cell(&r.color_label),
                    r.calibration_flag,
                    escape_md_cell(&r.flip_margin_coded),
                    escape_md_cell(&r.uu_summary),
                );
            }
        }
        if !sec.notes.is_empty() {
            let _ = writeln!(md, "\n### Notes\n");
            for note in &sec.notes {
                let _ = writeln!(md, "- {note}");
            }
        }
    }

    let _ = writeln!(md, "\n---");
    let _ = writeln!(
        md,
        "Canonical inputs digest (BLAKE3): `{canonical_inputs_digest}` | Anchors: `{}` | Certificates: `{}` | Claims: `{claim_count}` (`{suppressed_claim_count}` G-EXEC suppressed)",
        anchors.len(),
        certificates.len()
    );
    let _ = writeln!(md, "> {REPORT_NO_CLAIM_BANNER}");

    let markdown_sha256 = sha256(md.as_bytes());
    let markdown_blake3 = blake3_hash(md.as_bytes());

    Ok(RenderedReport {
        format_version: REPORT_FORMAT_VERSION,
        renderer_tag: FD_FMD_00_FALLBACK_TAG,
        markdown: md,
        canonical_inputs_digest,
        markdown_sha256,
        markdown_blake3,
        authority_flags: flags,
        claim_count,
        suppressed_claim_count,
        anchor_count: anchors.len(),
        certificate_count: certificates.len(),
    })
}

/// Recompute SHA-256 and BLAKE3 digests over `report.markdown` and verify mandatory `NO_CLAIM_BANNER`
/// presence (`BD-124`). Refuses any single-byte modification with `FMINE-REF-505`.
pub fn verify_rendered_report(report: &RenderedReport) -> Result<(), Refusal> {
    let actual_sha256 = sha256(report.markdown.as_bytes());
    if actual_sha256 != report.markdown_sha256 {
        return Err(Refusal::new(
            RefusalCode::DigestMismatch505,
            format!(
                "report SHA-256 mismatch: expected {}, got {actual_sha256}",
                report.markdown_sha256
            ),
        ));
    }
    let actual_blake3 = blake3_hash(report.markdown.as_bytes());
    if actual_blake3 != report.markdown_blake3 {
        return Err(Refusal::new(
            RefusalCode::DigestMismatch505,
            format!(
                "report BLAKE3 mismatch: expected {}, got {actual_blake3}",
                report.markdown_blake3
            ),
        ));
    }
    if !report.markdown.contains(REPORT_NO_CLAIM_BANNER) {
        return Err(Refusal::new(
            RefusalCode::DigestMismatch505,
            "rendered report is missing mandatory INV-002 NO_CLAIM_BANNER",
        ));
    }
    Ok(())
}

/// Deterministic JSON metadata summary for a [`RenderedReport`] carrying all four `INV-002`
/// `false` authority flags and bound digests (`BD-123`, `BD-124`).
#[must_use]
pub fn render_report_metadata_json(report: &RenderedReport) -> String {
    let f = report.authority_flags;
    format!(
        "{{\"format_version\":{},\"renderer_tag\":\"{}\",\"canonical_inputs_digest\":\"{}\",\"markdown_sha256\":\"{}\",\"markdown_blake3\":\"{}\",\"anchor_count\":{},\"certificate_count\":{},\"claim_count\":{},\"suppressed_claim_count\":{},\"reserve_disclosure_claim\":{},\"execution_authorized\":{},\"geotechnical_signoff\":{},\"regulatory_compliance_claim\":{},\"no_claim\":\"{}\"}}",
        report.format_version,
        escape_json_str(report.renderer_tag),
        report.canonical_inputs_digest,
        report.markdown_sha256,
        report.markdown_blake3,
        report.anchor_count,
        report.certificate_count,
        report.claim_count,
        report.suppressed_claim_count,
        f.reserve_disclosure_claim(),
        f.execution_authorized(),
        f.geotechnical_signoff(),
        f.regulatory_compliance_claim(),
        escape_json_str(REPORT_NO_CLAIM_BANNER),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use fmine_claim::render::FlipMarginSummary;
    use fmine_claim::{
        AdditivityClass, Claim, LabelRegistry, Representation, SpatialSupport, UuTerm,
    };
    use fmine_core::anchor::SourceAnchor;
    use fmine_evidence::color::ValidityDomain;
    use fmine_evidence::{EpistemicStatus, EvidenceClass, KnowledgeState, KuKind};

    fn sample_anchor_obj() -> SourceAnchor {
        SourceAnchor::new(
            "corpus/Barrick_Q2_2026.xlsx",
            sha256(b"barrick-q2-2026-workbook").0,
            "Gold AISC!L14",
        )
    }

    fn sample_anchor_refs() -> Vec<ReportAnchorRef> {
        vec![
            ReportAnchorRef {
                label: "barrick-q2-2026".to_string(),
                anchor_id: "anc_2026q2_minestats".to_string(),
                sha256_hex: sha256(b"barrick-q2-2026-workbook").to_hex(),
                locator: "2026Q2:Gold AISC!L14".to_string(),
            },
            ReportAnchorRef {
                label: "barrick-q1-2026".to_string(),
                anchor_id: "anc_2026q1_minestats".to_string(),
                sha256_hex: sha256(b"barrick-q1-2026-workbook").to_hex(),
                locator: "2026Q1:Gold AISC!L14".to_string(),
            },
        ]
    }

    fn sample_cert_refs() -> Vec<ReportCertificateRef> {
        vec![
            ReportCertificateRef {
                label: "public-assure".to_string(),
                cert_id: "FMT-CERT-1".to_string(),
                digest_hex: blake3_hash(b"public-assure-cert").to_hex(),
                determinism_class: "Exact".to_string(),
            },
            ReportCertificateRef {
                label: "plan-cert".to_string(),
                cert_id: "FMT-PCERT-1".to_string(),
                digest_hex: blake3_hash(b"plan-cert-bytes").to_hex(),
                determinism_class: "BitwisePortable".to_string(),
            },
        ]
    }

    fn sample_entries() -> (Vec<ReportClaimEntry>, String) {
        let anchor = sample_anchor_obj();
        let measured = Claim::new(
            "barrick.ngm.aisc_2026q2",
            1450.0_f64,
            "1450",
            Representation::Interval {
                lo: 1445.0,
                hi: 1455.0,
            },
            SpatialSupport::PeriodAggregate,
            AdditivityClass::IntensiveWeighted,
            EvidenceClass::Measured,
            EpistemicStatus::KnownKnown,
            KnowledgeState::Known,
            Confidentiality::public(),
            Vec::new(),
            Vec::new(),
            vec![anchor.clone()],
        )
        .with_regime(
            ValidityDomain::unconstrained().with("cost-usd-per-oz", 1440.0, 1460.0),
            "qaqc-2026q2",
        );

        let flip = FlipMarginSummary {
            driver: "copper_price".to_string(),
            threshold_text: "$3.40/lb".to_string(),
            margin_text: "+$0.75/lb".to_string(),
            plain_sentence: "the conclusion reverses if copper falls below $3.40/lb".to_string(),
        };
        let modeled = Claim::new(
            "lumwana.superpit.npv_delta",
            420.0_f64,
            "420",
            Representation::Quantiles {
                p10: 180.0,
                p50: 420.0,
                p90: 690.0,
            },
            SpatialSupport::PeriodAggregate,
            AdditivityClass::ExtensiveAdditive,
            EvidenceClass::ModelledUncalibrated,
            EpistemicStatus::KnownUnknown(KuKind::Parametric),
            KnowledgeState::Estimated,
            Confidentiality::public(),
            vec![UuTerm {
                scenario_id: "UU-ZAMBIA-POWER".to_string(),
                description: "Hydro drought grid curtailment".to_string(),
                downside_exposure_usd_m: 85.0,
                trigger_indicator: "Kariba dam level < 15%".to_string(),
            }],
            Vec::new(),
            vec![anchor.clone()],
        );

        let unknown = Claim::new(
            "tsf.piezometer.margin",
            9999.0_f64,
            "9999",
            Representation::Deterministic,
            SpatialSupport::PointCore,
            AdditivityClass::NonAdditive,
            EvidenceClass::Assumed,
            EpistemicStatus::UnmodelledBudget,
            KnowledgeState::Unknown,
            Confidentiality::public(),
            Vec::new(),
            Vec::new(),
            vec![anchor],
        );

        let public = Confidentiality::public();
        let e1 = ReportClaimEntry::from_claim(
            &ClaimRenderInput {
                claim: &measured,
                value_range_text: "1,445–1,455",
                unit: "$/oz sold (61.5% attributable)",
                calibration_summary: None,
                flip_margin: None,
            },
            &public,
        )
        .expect("measured entry");
        let e2 = ReportClaimEntry::from_claim(
            &ClaimRenderInput {
                claim: &modeled,
                value_range_text: "P10=$180M / P50=$420M / P90=$690M",
                unit: "USD_M (100% project)",
                calibration_summary: Some("inside its 80% range in 15 of 20 quarters"),
                flip_margin: Some(&flip),
            },
            &public,
        )
        .expect("modeled entry");
        let e3 = ReportClaimEntry::from_claim(
            &ClaimRenderInput {
                claim: &unknown,
                value_range_text: "9999.0",
                unit: "kPa",
                calibration_summary: None,
                flip_margin: None,
            },
            &public,
        )
        .expect("unknown entry");

        (vec![e1, e2, e3], measured.claim_id.to_string())
    }

    #[test]
    fn test_report_renders_anchors_certificates_g_exec_and_no_claim_banner() {
        let (entries, measured_id) = sample_entries();
        let spec = ReportSpec {
            title: "M0 Public-Lane Assurance Report".to_string(),
            subtitle: Some("Built from public filings; no affiliation implied".to_string()),
            lane_label: "public".to_string(),
            issuer_disclaimer: Some(
                "Public filings only; no issuer affiliation implied.".to_string(),
            ),
            reader_clearance: Confidentiality::public(),
            anchors: sample_anchor_refs(),
            certificates: sample_cert_refs(),
            sections: vec![ReportSection {
                heading: "Published Cost & Margin Claims".to_string(),
                summary: "Deterministic plain-Markdown rendering under FD-FMD-00 fallback."
                    .to_string(),
                entries,
                notes: vec![
                    "All numbers carry source cell anchors and evidence classes.".to_string(),
                ],
            }],
        };

        let rendered = render_report(&spec).expect("render report");
        verify_rendered_report(&rendered).expect("digest verifies");

        assert_eq!(rendered.format_version, REPORT_FORMAT_VERSION);
        assert_eq!(rendered.renderer_tag, FD_FMD_00_FALLBACK_TAG);
        assert_eq!(rendered.claim_count, 3);
        assert_eq!(rendered.suppressed_claim_count, 1);
        assert_eq!(rendered.anchor_count, 2);
        assert_eq!(rendered.certificate_count, 2);
        assert!(!rendered.authority_flags.reserve_disclosure_claim());
        assert!(!rendered.authority_flags.execution_authorized());
        assert!(!rendered.authority_flags.geotechnical_signoff());
        assert!(!rendered.authority_flags.regulatory_compliance_claim());

        let md = &rendered.markdown;
        assert!(md.contains("# M0 Public-Lane Assurance Report"));
        assert!(md.contains(REPORT_NO_CLAIM_BANNER));
        assert!(md.contains("`reserve_disclosure_claim` | `false`"));
        assert!(md.contains("`execution_authorized` | `false`"));
        assert!(md.contains("`geotechnical_signoff` | `false`"));
        assert!(md.contains("`regulatory_compliance_claim` | `false`"));
        assert!(md.contains("## Anchors"));
        assert!(md.contains("anc_2026q2_minestats"));
        assert!(md.contains("## Certificates"));
        assert!(md.contains("FMT-PCERT-1"));
        assert!(md.contains(&format!("fmine:{measured_id}")));
        assert!(md.contains("UU-ZAMBIA-POWER"));
        // G-EXEC numeric suppression: 9999.0 MUST NOT appear anywhere in rendered Markdown.
        assert!(md.contains("[evidence state: unknown]"));
        assert!(!md.contains("9999.0"));
        assert!(!md.contains('\r'));

        let meta = render_report_metadata_json(&rendered);
        assert!(meta.contains("\"reserve_disclosure_claim\":false"));
        assert!(meta.contains("\"execution_authorized\":false"));
        assert!(meta.contains("\"geotechnical_signoff\":false"));
        assert!(meta.contains("\"regulatory_compliance_claim\":false"));
        assert!(meta.contains(&rendered.markdown_sha256.to_hex()));
        assert!(meta.contains(FD_FMD_00_FALLBACK_TAG));
    }

    #[test]
    fn test_report_determinism_by_digest_byte_identical_rerender_and_order_invariance() {
        let (entries_a, _) = sample_entries();
        let mut entries_b = entries_a.clone();
        entries_b.reverse();

        let mut anchors_b = sample_anchor_refs();
        anchors_b.reverse();
        // Also add a duplicate anchor to verify canonical deduplication.
        anchors_b.push(anchors_b[0].clone());

        let mut certs_b = sample_cert_refs();
        certs_b.reverse();
        certs_b.push(certs_b[0].clone());

        let spec_a = ReportSpec {
            title: "Determinism Check Report".to_string(),
            subtitle: None,
            lane_label: "public".to_string(),
            issuer_disclaimer: None,
            reader_clearance: Confidentiality::public(),
            anchors: sample_anchor_refs(),
            certificates: sample_cert_refs(),
            sections: vec![ReportSection {
                heading: "Claims".to_string(),
                summary: "Line 1\nLine 2".to_string(),
                entries: entries_a,
                notes: vec!["Note A".to_string()],
            }],
        };

        let spec_b = ReportSpec {
            title: "Determinism Check Report".to_string(),
            subtitle: None,
            lane_label: "public".to_string(),
            issuer_disclaimer: None,
            reader_clearance: Confidentiality::public(),
            anchors: anchors_b,
            certificates: certs_b,
            sections: vec![ReportSection {
                heading: "Claims".to_string(),
                summary: "Line 1\r\nLine 2".to_string(),
                entries: entries_b,
                notes: vec!["Note A".to_string()],
            }],
        };

        let r1 = render_report(&spec_a).expect("render 1");
        let r2 = render_report(&spec_a).expect("render 2");
        let r_perm = render_report(&spec_b).expect("render permuted");

        // Exact byte-identical rerender and permutation invariance (BD-124).
        assert_eq!(r1.markdown.as_bytes(), r2.markdown.as_bytes());
        assert_eq!(r1.markdown_sha256, r2.markdown_sha256);
        assert_eq!(r1.markdown_blake3, r2.markdown_blake3);
        assert_eq!(r1.canonical_inputs_digest, r2.canonical_inputs_digest);

        assert_eq!(r1.markdown.as_bytes(), r_perm.markdown.as_bytes());
        assert_eq!(r1.markdown_sha256, r_perm.markdown_sha256);
        assert_eq!(r1.markdown_blake3, r_perm.markdown_blake3);
        assert_eq!(r1.canonical_inputs_digest, r_perm.canonical_inputs_digest);

        // Tampering with a single byte of rendered Markdown refuses with FMINE-REF-505.
        let mut tampered = r1.clone();
        tampered.markdown.push(' ');
        let err = verify_rendered_report(&tampered).expect_err("tampered report must refuse");
        assert_eq!((err.code, err.exit_code), ("FMINE-REF-505", 8));
    }

    #[test]
    fn test_report_refuses_mnpi_clearance_violation_tampered_claim_and_missing_anchors() {
        let anchor = sample_anchor_obj();
        let reg = LabelRegistry::new([], ["W-mnpi-q3"]).expect("wall registry");
        let wall = reg.wall("W-mnpi-q3").expect("wall");

        // 1. MNPI claim refused for public reader (FMINE-REF-301).
        let mnpi_claim = Claim::new(
            "secret.assay.grade",
            14.2_f64,
            "14.2",
            Representation::Deterministic,
            SpatialSupport::PointCore,
            AdditivityClass::IntensiveWeighted,
            EvidenceClass::Measured,
            EpistemicStatus::KnownKnown,
            KnowledgeState::Known,
            Confidentiality::mnpi(wall),
            Vec::new(),
            Vec::new(),
            vec![anchor.clone()],
        );
        let mnpi_err = ReportClaimEntry::from_claim(
            &ClaimRenderInput {
                claim: &mnpi_claim,
                value_range_text: "14.2",
                unit: "g/t",
                calibration_summary: None,
                flip_margin: None,
            },
            &Confidentiality::public(),
        )
        .expect_err("public reader cannot render MNPI claim");
        assert_eq!((mnpi_err.code, mnpi_err.exit_code), ("FMINE-REF-301", 7));

        // 2. Tampered Claim ID refused (FMINE-REF-507).
        let mut tampered_claim = Claim::new(
            "public.tonnes",
            100.0_f64,
            "100",
            Representation::Deterministic,
            SpatialSupport::PeriodAggregate,
            AdditivityClass::ExtensiveAdditive,
            EvidenceClass::Measured,
            EpistemicStatus::KnownKnown,
            KnowledgeState::Known,
            Confidentiality::public(),
            Vec::new(),
            Vec::new(),
            vec![anchor],
        );
        tampered_claim.metric_key = "forged.tonnes".to_string();
        let tamper_err = ReportClaimEntry::from_claim(
            &ClaimRenderInput {
                claim: &tampered_claim,
                value_range_text: "100",
                unit: "kt",
                calibration_summary: None,
                flip_margin: None,
            },
            &Confidentiality::public(),
        )
        .expect_err("tampered claim must refuse");
        assert_eq!(
            (tamper_err.code, tamper_err.exit_code),
            ("FMINE-REF-507", 6)
        );

        // 3. Missing anchors or certificates refuse with FMINE-REF-201.
        let no_anchors = ReportSpec {
            title: "Missing Anchors".to_string(),
            subtitle: None,
            lane_label: "public".to_string(),
            issuer_disclaimer: None,
            reader_clearance: Confidentiality::public(),
            anchors: Vec::new(),
            certificates: sample_cert_refs(),
            sections: Vec::new(),
        };
        assert_eq!(
            render_report(&no_anchors).unwrap_err().code,
            "FMINE-REF-201"
        );

        let no_certs = ReportSpec {
            title: "Missing Certs".to_string(),
            subtitle: None,
            lane_label: "public".to_string(),
            issuer_disclaimer: None,
            reader_clearance: Confidentiality::public(),
            anchors: sample_anchor_refs(),
            certificates: Vec::new(),
            sections: Vec::new(),
        };
        assert_eq!(render_report(&no_certs).unwrap_err().code, "FMINE-REF-201");

        // 4. Empty title and malformed anchor SHA-256 refuse with FMINE-REF-107.
        let empty_title = ReportSpec {
            title: "   ".to_string(),
            subtitle: None,
            lane_label: "public".to_string(),
            issuer_disclaimer: None,
            reader_clearance: Confidentiality::public(),
            anchors: sample_anchor_refs(),
            certificates: sample_cert_refs(),
            sections: Vec::new(),
        };
        assert_eq!(
            render_report(&empty_title).unwrap_err().code,
            "FMINE-REF-107"
        );

        let mut bad_hex = sample_anchor_refs();
        bad_hex[0].sha256_hex = "NOT-HEX".to_string();
        let bad_anchor = ReportSpec {
            title: "Bad Hex".to_string(),
            subtitle: None,
            lane_label: "public".to_string(),
            issuer_disclaimer: None,
            reader_clearance: Confidentiality::public(),
            anchors: bad_hex,
            certificates: sample_cert_refs(),
            sections: Vec::new(),
        };
        assert_eq!(
            render_report(&bad_anchor).unwrap_err().code,
            "FMINE-REF-107"
        );
    }
}
