//! Determinism: BitwisePortable
//!
//! Redact-on-ingest filter for stakeholder, consultation, and grievance records (`BD-292`, plan §31.2).
//!
//! Importers redact personal data on ingest **even from public documents** (e.g. public ESIA
//! consultation appendices and grievance forms, C5 §4.9): names, contact numbers, email addresses,
//! house/GPS locations, national IDs, and signatures are stripped before records leave quarantine.
//! Free-text fields are also scrubbed for inline email addresses, phone numbers, coordinates, and
//! `Contact:`/`Signed by:` signatures.

use std::collections::BTreeSet;

use fmine_authz::residency::{DataClass, ResidencyDecision, ResidencyRegistry};
use fmine_core::hash::sha256;

use crate::dialects::{PiiClass, ResolvedColumn, ResolvedUnit};

/// Replacement marker emitted whenever a personal field or inline PII token is stripped on ingest.
pub const REDACTED_MARKER: &str = "[REDACTED:PII]";

/// Column header keywords that trigger unconditional redact-on-ingest even if a dialect failed to
/// tag `pii_class = "personal_redact"` (defense in depth, §31.2).
const PERSONAL_HEADER_KEYWORDS: [&str; 17] = [
    "name",
    "attendee",
    "complainant",
    "stakeholder",
    "contact",
    "phone",
    "mobile",
    "tel",
    "email",
    "e_mail",
    "address",
    "house",
    "gps",
    "lat_lon",
    "signature",
    "national_id",
    "passport",
];

/// Audit count of redactions performed on one imported row.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RedactionSummary {
    pub columns_redacted: usize,
    pub inline_tokens_scrubbed: usize,
}

/// Result of redacting one stakeholder/ESIA record on ingest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RedactedRow {
    pub row_index: usize,
    pub cells: Vec<String>,
    pub summary: RedactionSummary,
    /// Optional keyed pseudonym derived from a primary identifier when a privacy-officer key is
    /// provided (never reversible without the key, PO-MUST-4).
    pub subject_pseudonym: Option<String>,
}

/// Returns `true` if `col` must be redacted unconditionally on ingest.
#[must_use]
pub fn is_personal_column(col: &ResolvedColumn) -> bool {
    if col.pii_class == PiiClass::PersonalRedact {
        return true;
    }
    let lower = col.column_name.to_ascii_lowercase();
    PERSONAL_HEADER_KEYWORDS.iter().any(|kw| lower.contains(kw))
}

/// Derives a deterministic keyed pseudonym `psn_<16 hex>` from a raw subject identifier using
/// `privacy_officer_key` (zero plain PII retained, PO-MUST-4).
#[must_use]
pub fn pseudonymize_subject(privacy_officer_key: &[u8; 32], raw_identifier: &str) -> String {
    let mut buf = Vec::with_capacity(64 + raw_identifier.len());
    buf.extend_from_slice(b"FMINE-PII-PSEUDONYM-V1:");
    buf.extend_from_slice(privacy_officer_key);
    buf.extend_from_slice(raw_identifier.trim().as_bytes());
    let hex = sha256(&buf).to_hex();
    format!("psn_{}", &hex[..16])
}

/// Redacts all personal columns and scrubs inline PII tokens (emails, phone numbers, coordinates,
/// `Signed by:` / `Contact:` trailers) from every surviving free-text cell of a row.
#[must_use]
pub fn redact_row_on_ingest(
    row_index: usize,
    columns: &[ResolvedColumn],
    raw_cells: &[String],
    privacy_officer_key: Option<&[u8; 32]>,
) -> RedactedRow {
    let mut out = Vec::with_capacity(raw_cells.len());
    let mut summary = RedactionSummary::default();
    let mut subject_pseudonym: Option<String> = None;

    for (idx, raw) in raw_cells.iter().enumerate() {
        let personal_col = columns.get(idx).is_some_and(is_personal_column);
        if personal_col {
            if subject_pseudonym.is_none()
                && !raw.trim().is_empty()
                && let Some(key) = privacy_officer_key
            {
                subject_pseudonym = Some(pseudonymize_subject(key, raw));
            }
            if !raw.trim().is_empty() {
                summary.columns_redacted += 1;
            }
            out.push(REDACTED_MARKER.to_string());
        } else if columns
            .get(idx)
            .is_some_and(|c| matches!(c.unit, ResolvedUnit::Registered(_)))
        {
            // Registered physical numeric columns (e.g. 7-digit UTM Northings like 4500000 m
            // or 7-digit tonnages) are validated as numbers/placeholders by quarantine and are
            // not free-text narratives; do not misclassify them as phone numbers.
            out.push(raw.clone());
        } else {
            let (scrubbed, hits) = scrub_free_text(raw);
            summary.inline_tokens_scrubbed += hits;
            out.push(scrubbed);
        }
    }

    RedactedRow {
        row_index,
        cells: out,
        summary,
        subject_pseudonym,
    }
}

/// Checks cross-border export of raw personal vs anonymised aggregate stakeholder records against
/// the committed 72-row `registries/residency.toml` (`BD-292`, §31.2).
#[must_use]
pub fn check_stakeholder_residency(
    registry: &ResidencyRegistry,
    origin_country: &str,
    destination_country: &str,
    is_anonymised_aggregate: bool,
    registered_instruments: &BTreeSet<String>,
) -> ResidencyDecision {
    let class = if is_anonymised_aggregate {
        DataClass::Aggregate
    } else {
        DataClass::Personal
    };
    registry.evaluate_transfer(
        origin_country,
        class,
        destination_country,
        is_anonymised_aggregate,
        registered_instruments,
    )
}

/// Scrubs inline email addresses, phone numbers, GPS lat/lon pairs, and contact/signature clauses
/// from free-text narratives.
#[must_use]
pub fn scrub_free_text(input: &str) -> (String, usize) {
    let mut hits = 0usize;

    // 1. Strip trailing contact/signature clauses (`Signed by: ...`, `Contact: ...`, `Tel: ...`).
    let mut working = input.to_string();
    for marker in [
        "Signed by:",
        "signed by:",
        "Signature:",
        "signature:",
        "Contact:",
        "contact:",
        "Phone:",
        "phone:",
        "Tel:",
        "tel:",
        "Address:",
        "address:",
    ] {
        if let Some(pos) = working.find(marker) {
            let suffix = &working[pos..];
            let end = suffix.find(';').map_or(working.len(), |semi| pos + semi);
            working.replace_range(pos..end, REDACTED_MARKER);
            hits += 1;
        }
    }

    // 2. Token-level scrub for emails, phone numbers (>=7 digits), and decimal GPS coordinates.
    let mut tokens_out = Vec::new();
    for tok in working.split_whitespace() {
        let core = tok.trim_matches(|c: char| matches!(c, ',' | '.' | ';' | '(' | ')' | '<' | '>'));
        if is_email_token(core) || is_phone_token(core) || is_gps_token(core) {
            hits += 1;
            tokens_out.push(REDACTED_MARKER.to_string());
        } else {
            tokens_out.push(tok.to_string());
        }
    }

    (tokens_out.join(" "), hits)
}

fn is_email_token(s: &str) -> bool {
    let Some((local, domain)) = s.split_once('@') else {
        return false;
    };
    !local.is_empty() && domain.contains('.') && domain.len() >= 3
}

fn is_phone_token(s: &str) -> bool {
    if s.is_empty() {
        return false;
    }
    let digits = s.bytes().filter(u8::is_ascii_digit).count();
    let allowed = s
        .bytes()
        .all(|b| b.is_ascii_digit() || matches!(b, b'+' | b'-' | b'(' | b')' | b'/'));
    allowed && digits >= 7
}

fn is_gps_token(s: &str) -> bool {
    let Some((lat, lon)) = s.split_once(',') else {
        return false;
    };
    is_signed_decimal(lat) && is_signed_decimal(lon)
}

fn is_signed_decimal(s: &str) -> bool {
    let stripped = s
        .strip_prefix('-')
        .or_else(|| s.strip_prefix('+'))
        .unwrap_or(s);
    let Some((a, b)) = stripped.split_once('.') else {
        return false;
    };
    !a.is_empty()
        && b.len() >= 3
        && a.bytes().all(|c| c.is_ascii_digit())
        && b.bytes().all(|c| c.is_ascii_digit())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dialects::IoDialectRegistry;
    use fmine_registry::RegistryCatalog;
    use std::path::PathBuf;

    fn workspace_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|p| p.parent())
            .expect("workspace root")
            .to_path_buf()
    }

    /// BD-292 done-when: no personal field survives import of a synthetic public ESIA consultation
    /// and grievance log, and the 72 W3 residency rows hold personal data in-country while permitting
    /// anonymised aggregates.
    #[test]
    fn test_esia_redact_on_ingest_strips_all_personal_fields_and_enforces_w3_residency() {
        let root = workspace_root();
        let catalog = RegistryCatalog::load_from_root(&root).expect("catalog");
        let reg_units: BTreeSet<String> = catalog.unit_symbols.iter().cloned().collect();
        let reg_crs: BTreeSet<String> = catalog.crs_codes.iter().cloned().collect();
        let dialects = IoDialectRegistry::load_from_root(&root).expect("dialects");

        let headers: Vec<String> = [
            "RECORD_ID",
            "FULL_NAME",
            "PHONE",
            "EMAIL",
            "HOUSE_LOCATION",
            "SIGNATURE",
            "TOPIC",
            "SUMMARY",
        ]
        .iter()
        .map(|s| (*s).to_string())
        .collect();
        let cols = dialects
            .resolve_headers("esia_stakeholder_v1", &headers, &reg_units, &reg_crs)
            .expect("resolve");

        // Synthetic ESIA grievance row modelled on C5 §4.9 (invented placeholder PII, never real data).
        let raw_row: Vec<String> = vec![
            "GRV-001".into(),
            "Synthetic Person Alpha".into(),
            "+92-300-5550199".into(),
            "alpha.example@village.invalid".into(),
            "House 14, Sector B, 29.1234,62.0456".into(),
            "Signed by Alpha".into(),
            "Water well monitoring".into(),
            "Requested borehole sampling; Contact: alpha.example@village.invalid +92-300-5550199; GPS 29.1234,62.0456; Signed by: Synthetic Person Alpha".into(),
        ];

        let po_key = [0x42u8; 32];
        let redacted = redact_row_on_ingest(1, &cols, &raw_row, Some(&po_key));
        assert_eq!(redacted.cells[0], "GRV-001");
        assert_eq!(redacted.cells[6], "Water well monitoring");
        assert_eq!(redacted.summary.columns_redacted, 5);
        assert!(redacted.summary.inline_tokens_scrubbed >= 2);
        assert!(
            redacted
                .subject_pseudonym
                .as_deref()
                .is_some_and(|p| p.starts_with("psn_"))
        );

        // Assert ZERO personal tokens survive anywhere in the emitted record!
        let joined = redacted.cells.join(" | ");
        for forbidden in [
            "Synthetic Person Alpha",
            "5550199",
            "alpha.example",
            "village.invalid",
            "House 14",
            "29.1234",
            "62.0456",
            "Signed by Alpha",
        ] {
            assert!(
                !joined.contains(forbidden),
                "personal token {forbidden:?} survived redact-on-ingest: {joined}"
            );
        }

        // Verify 72-row W3 residency enforcement on PK (Reko Diq) & ZM (Lumwana) stakeholder records.
        let residency = ResidencyRegistry::load_from_root(&root).expect("residency.toml");
        assert_eq!(residency.len(), 72);
        let empty = BTreeSet::new();
        for origin in ["PK", "ZM", "TZ", "CD", "ML"] {
            assert!(matches!(
                check_stakeholder_residency(&residency, origin, "CA", false, &empty),
                ResidencyDecision::Refused(_)
            ));
            assert!(matches!(
                check_stakeholder_residency(&residency, origin, "CA", true, &empty),
                ResidencyDecision::Allowed { .. }
            ));
        }
    }

    #[test]
    fn test_redact_preserves_registered_physical_utm_coordinates_while_scrubbing_free_text() {
        use crate::dialects::{ColumnBasis, ColumnSupport, ResolvedCrs};
        let cols = vec![
            ResolvedColumn {
                column_name: "NORTH".to_string(),
                unit: ResolvedUnit::Registered("m".to_string()),
                basis: Some(ColumnBasis::None),
                support: Some(ColumnSupport::PointCore),
                crs: ResolvedCrs::Registered("EPSG:32611".to_string()),
                pii_class: PiiClass::None,
                undeclared_in_dialect: false,
            },
            ResolvedColumn {
                column_name: "SUMMARY".to_string(),
                unit: ResolvedUnit::None,
                basis: Some(ColumnBasis::None),
                support: Some(ColumnSupport::None),
                crs: ResolvedCrs::None,
                pii_class: PiiClass::None,
                undeclared_in_dialect: false,
            },
        ];
        let raw = vec![
            "4500000".to_string(),
            "Call 5551234567 about road access".to_string(),
        ];
        let redacted = redact_row_on_ingest(2, &cols, &raw, None);
        assert_eq!(
            redacted.cells[0], "4500000",
            "7-digit UTM Northing in registered physical column must not be scrubbed as phone PII"
        );
        assert!(
            !redacted.cells[1].contains("5551234567")
                && redacted.cells[1].contains(REDACTED_MARKER),
            "phone token in free-text column must still be scrubbed"
        );
        assert_eq!(redacted.summary.inline_tokens_scrubbed, 1);
    }
}
