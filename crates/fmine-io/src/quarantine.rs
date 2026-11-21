//! Determinism: Exact
//!
//! Quarantine-on-import pipeline and structured import reports (`BD-185`, plan §32.1).
//!
//! Pipeline:
//! 1. Store raw original bytes in `FMT-CAS-1` (`CasStore`, root-last) so every admitted record and
//!    every rejected row cites `source_cas_root` and `source_sha256`.
//! 2. Enforce byte and row budgets while parsing pure-Rust RFC 4180 CSV.
//! 3. Resolve every header column through `registries/io_dialects.toml` (`PO-PORT-3`), keeping
//!    undeclared or unknown units explicitly `UNREGISTERED_UNIT` (`FMINE-REF-107`).
//! 4. Redact personal columns & inline PII on ingest (`BD-292`, §31.2).
//! 5. Classify every data row into admitted or quarantined — nothing is silently dropped, and every
//!    [`RejectedRow`] carries its 1-based row number, column, raw value, registered `FMINE-REF-*`
//!    code, explanation, and fix.

use std::collections::BTreeSet;
use std::fmt;

use fmine_core::codec::Encoder;
use fmine_core::hash::{Digest32, sha256};
use fmine_evidence::{Refusal, RefusalCode};
use fmine_object::cas::{CasError, DEFAULT_CHUNK_BYTES};
use fmine_object::store::CasStore;
use fmine_units::UNREGISTERED_UNIT;

use crate::dialects::{DialectError, IoDialectRegistry, ResolvedColumn, ResolvedUnit};
use crate::redact::{RedactionSummary, redact_row_on_ingest};

/// Hard quarantine byte cap (32 MiB) preventing unbounded allocation on adversarial inputs (`PO-PORT-1`).
pub const MAX_QUARANTINE_BYTES: usize = 32 * 1024 * 1024;
/// Hard quarantine row cap (250,000 rows per import batch).
pub const MAX_QUARANTINE_ROWS: usize = 250_000;

/// One admitted row leaving quarantine (with any PII already redacted on ingest).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdmittedRow {
    pub row_index: usize,
    pub cells: Vec<String>,
    pub redactions: RedactionSummary,
    pub subject_pseudonym: Option<String>,
}

/// One rejected/quarantined row with full structured explanation (nothing silently dropped, §32.1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RejectedRow {
    pub row_index: usize,
    pub column: String,
    pub raw_value: String,
    pub code: &'static str,
    pub explanation: String,
    pub fix: &'static str,
}

/// Complete deterministic import report binding the CAS-ingested source bytes, resolved columns,
/// admitted rows, and every rejected row with its explanation (`BD-185`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportReport {
    pub dialect_id: String,
    pub source_cas_root: Digest32,
    pub source_sha256: String,
    pub source_bytes: u64,
    pub resolved_columns: Vec<ResolvedColumn>,
    pub unregistered_columns: Vec<String>,
    pub total_data_rows: usize,
    pub admitted_rows: Vec<AdmittedRow>,
    pub rejected_rows: Vec<RejectedRow>,
    pub total_columns_redacted: usize,
    pub total_inline_pii_scrubbed: usize,
    pub report_digest: Digest32,
}

/// Errors that abort quarantine import before per-row classification (e.g. budget overrun, CAS failure).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QuarantineError {
    ByteBudgetExceeded { actual: usize, limit: usize },
    RowBudgetExceeded { limit: usize },
    CsvSyntax { line: usize, detail: String },
    EmptyInput,
    Dialect(DialectError),
    Cas(CasError),
}

impl QuarantineError {
    #[must_use]
    pub fn to_refusal(&self) -> Refusal {
        match self {
            Self::Cas(e) => e.to_refusal(),
            _ => Refusal::new(RefusalCode::SourceCellUnusable208, self.to_string()),
        }
    }
}

impl fmt::Display for QuarantineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ByteBudgetExceeded { actual, limit } => {
                write!(
                    f,
                    "import size {actual} bytes exceeds quarantine budget {limit}"
                )
            }
            Self::RowBudgetExceeded { limit } => {
                write!(f, "import row count exceeds quarantine budget {limit}")
            }
            Self::CsvSyntax { line, detail } => {
                write!(f, "CSV syntax error at line {line}: {detail}")
            }
            Self::EmptyInput => write!(f, "import contains no header row"),
            Self::Dialect(e) => write!(f, "dialect error: {e}"),
            Self::Cas(e) => write!(f, "CAS quarantine error: {e}"),
        }
    }
}

impl std::error::Error for QuarantineError {}

/// Runs the §32.1 quarantine-on-import pipeline over raw CSV bytes, persisting the original bytes
/// into `store` (`FMT-CAS-1`, root-last) and returning an [`ImportReport`] explaining every rejection.
#[allow(clippy::too_many_arguments)]
pub fn import_csv_through_quarantine(
    store: &CasStore,
    dialects: &IoDialectRegistry,
    dialect_id: &str,
    raw_bytes: &[u8],
    registered_units: &BTreeSet<String>,
    registered_crs: &BTreeSet<String>,
    privacy_officer_key: Option<&[u8; 32]>,
) -> Result<ImportReport, QuarantineError> {
    if raw_bytes.len() > MAX_QUARANTINE_BYTES {
        return Err(QuarantineError::ByteBudgetExceeded {
            actual: raw_bytes.len(),
            limit: MAX_QUARANTINE_BYTES,
        });
    }

    // Step 1: persist immutable original bytes into FMT-CAS-1 BEFORE parsing.
    let source_sha256 = sha256(raw_bytes).to_hex();
    let staged = store
        .stage_object("text/csv", raw_bytes, DEFAULT_CHUNK_BYTES)
        .map_err(QuarantineError::Cas)?;
    let manifest = store
        .publish_staged(&staged)
        .map_err(QuarantineError::Cas)?;

    // Step 2: decode UTF-8 and parse RFC 4180 CSV under row budget.
    let text = std::str::from_utf8(raw_bytes).map_err(|_| QuarantineError::CsvSyntax {
        line: 1,
        detail: "invalid UTF-8 bytes".to_string(),
    })?;
    let rows = parse_csv_rows(text)?;
    let Some((header_row, data_rows)) = rows.split_first() else {
        return Err(QuarantineError::EmptyInput);
    };

    // Step 3: resolve every header column via io_dialects.toml (Undeclared -> UNREGISTERED).
    let resolved_columns = dialects
        .resolve_headers(dialect_id, header_row, registered_units, registered_crs)
        .map_err(QuarantineError::Dialect)?;
    let unregistered_columns: Vec<String> = resolved_columns
        .iter()
        .filter(|c| {
            c.undeclared_in_dialect
                || matches!(c.unit, ResolvedUnit::Unregistered { .. })
                || c.registration_refusal().is_some()
        })
        .map(|c| c.column_name.clone())
        .collect();

    let mut admitted_rows = Vec::new();
    let mut rejected_rows = Vec::new();
    let mut total_columns_redacted = 0usize;
    let mut total_inline_pii_scrubbed = 0usize;

    // Step 4 & 5: validate each row against dialect rules, redact PII on ingest, never drop silently.
    for (idx, raw_row) in data_rows.iter().enumerate() {
        let row_index = idx + 2; // 1-based CSV row (header is row 1)
        if raw_row.len() != header_row.len() {
            let r = Refusal::new(
                RefusalCode::SourceCellUnusable208,
                format!(
                    "expected {} columns, found {}",
                    header_row.len(),
                    raw_row.len()
                ),
            );
            rejected_rows.push(RejectedRow {
                row_index,
                column: "<row>".to_string(),
                raw_value: raw_row.join(","),
                code: r.code,
                explanation: r.what,
                fix: r.fix,
            });
            continue;
        }

        // Validate physical/numeric columns (any column with a registered physical unit) and catch
        // Excel formula error residues (`#REF!`, `#DIV/0!`, `#VALUE!`, `#N/A`, `#NAME?`).
        let mut row_rejection: Option<RejectedRow> = None;
        for (col_idx, cell) in raw_row.iter().enumerate() {
            let col = &resolved_columns[col_idx];
            let trimmed = cell.trim();

            if is_formula_error(trimmed) {
                let r = Refusal::new(
                    RefusalCode::SourceCellUnusable208,
                    format!(
                        "formula error residue '{trimmed}' in column '{}'",
                        col.column_name
                    ),
                );
                row_rejection = Some(RejectedRow {
                    row_index,
                    column: col.column_name.clone(),
                    raw_value: trimmed.to_string(),
                    code: r.code,
                    explanation: r.what,
                    fix: r.fix,
                });
                break;
            }

            if let ResolvedUnit::Registered(unit_code) = &col.unit {
                if trimmed.is_empty() || is_placeholder_token(trimmed) {
                    continue;
                }
                match parse_strict_decimal(trimmed) {
                    Some(val) => {
                        if !val.is_finite() {
                            let r = Refusal::new(
                                RefusalCode::OutsidePhysicalDomain109,
                                format!("non-finite numeric value in '{}'", col.column_name),
                            );
                            row_rejection = Some(RejectedRow {
                                row_index,
                                column: col.column_name.clone(),
                                raw_value: trimmed.to_string(),
                                code: r.code,
                                explanation: r.what,
                                fix: r.fix,
                            });
                            break;
                        }
                        if matches!(
                            unit_code.as_str(),
                            "g_per_t" | "wt_pct" | "t" | "kt" | "Mt" | "oz_t"
                        ) && val < 0.0
                        {
                            let r = Refusal::new(
                                RefusalCode::OutsidePhysicalDomain109,
                                format!(
                                    "negative physical quantity '{trimmed}' ({unit_code}) in column '{}'",
                                    col.column_name
                                ),
                            );
                            row_rejection = Some(RejectedRow {
                                row_index,
                                column: col.column_name.clone(),
                                raw_value: trimmed.to_string(),
                                code: r.code,
                                explanation: r.what,
                                fix: r.fix,
                            });
                            break;
                        }
                    }
                    None => {
                        let r = Refusal::new(
                            RefusalCode::SourceCellUnusable208,
                            format!(
                                "non-numeric token '{trimmed}' in physical column '{}' ({unit_code})",
                                col.column_name
                            ),
                        );
                        row_rejection = Some(RejectedRow {
                            row_index,
                            column: col.column_name.clone(),
                            raw_value: trimmed.to_string(),
                            code: r.code,
                            explanation: r.what,
                            fix: r.fix,
                        });
                        break;
                    }
                }
            } else if let ResolvedUnit::Unregistered { declared } = &col.unit
                && !trimmed.is_empty()
                && !is_placeholder_token(trimmed)
                && parse_strict_decimal(trimmed).is_some()
            {
                let r = Refusal::new(
                    RefusalCode::UndeclaredUnitBasisOrTz107,
                    format!(
                        "numeric value '{trimmed}' in undeclared/unregistered column '{}' ({UNREGISTERED_UNIT}, declared '{declared}')",
                        col.column_name
                    ),
                );
                row_rejection = Some(RejectedRow {
                    row_index,
                    column: col.column_name.clone(),
                    raw_value: trimmed.to_string(),
                    code: r.code,
                    explanation: r.what,
                    fix: r.fix,
                });
                break;
            }
        }

        if let Some(rej) = row_rejection {
            rejected_rows.push(rej);
        } else {
            let redacted =
                redact_row_on_ingest(row_index, &resolved_columns, raw_row, privacy_officer_key);
            total_columns_redacted += redacted.summary.columns_redacted;
            total_inline_pii_scrubbed += redacted.summary.inline_tokens_scrubbed;
            admitted_rows.push(AdmittedRow {
                row_index,
                cells: redacted.cells,
                redactions: redacted.summary,
                subject_pseudonym: redacted.subject_pseudonym,
            });
        }
    }

    let report_digest = compute_report_digest(
        dialect_id,
        &manifest.root_digest,
        &source_sha256,
        admitted_rows.len(),
        &rejected_rows,
    );

    Ok(ImportReport {
        dialect_id: dialect_id.to_string(),
        source_cas_root: manifest.root_digest,
        source_sha256,
        source_bytes: manifest.total_bytes,
        resolved_columns,
        unregistered_columns,
        total_data_rows: data_rows.len(),
        admitted_rows,
        rejected_rows,
        total_columns_redacted,
        total_inline_pii_scrubbed,
        report_digest,
    })
}

fn is_formula_error(s: &str) -> bool {
    matches!(
        s,
        "#REF!" | "#DIV/0!" | "#VALUE!" | "#NAME?" | "#N/A" | "#NUM!" | "#NULL!"
    )
}

fn is_placeholder_token(s: &str) -> bool {
    matches!(s, "-" | "—" | "N/A" | "n/a" | "NA" | "<DL" | "BDL" | "NS")
}

fn parse_strict_decimal(s: &str) -> Option<f64> {
    let cleaned = s.replace(',', "");
    let body = cleaned
        .strip_prefix('-')
        .or_else(|| cleaned.strip_prefix('+'))
        .unwrap_or(&cleaned);
    if body.is_empty() {
        return None;
    }
    let mut seen_dot = false;
    let mut seen_digit = false;
    for b in body.bytes() {
        if b.is_ascii_digit() {
            seen_digit = true;
        } else if b == b'.' && !seen_dot {
            seen_dot = true;
        } else {
            return None;
        }
    }
    if !seen_digit {
        return None;
    }
    cleaned.parse::<f64>().ok()
}

/// Pure-Rust RFC 4180 CSV parser bounded by [`MAX_QUARANTINE_ROWS`].
fn parse_csv_rows(text: &str) -> Result<Vec<Vec<String>>, QuarantineError> {
    let mut rows = Vec::new();
    let mut current_row = Vec::new();
    let mut field = String::new();
    let mut in_quotes = false;
    let mut line_no = 1usize;
    let mut chars = text.chars().peekable();

    while let Some(ch) = chars.next() {
        if in_quotes {
            if ch == '"' {
                if chars.peek() == Some(&'"') {
                    chars.next();
                    field.push('"');
                } else {
                    in_quotes = false;
                }
            } else {
                if ch == '\n' {
                    line_no += 1;
                }
                field.push(ch);
            }
        } else {
            match ch {
                '"' if field.is_empty() => in_quotes = true,
                ',' => {
                    current_row.push(std::mem::take(&mut field));
                }
                '\r' => {
                    if chars.peek() == Some(&'\n') {
                        chars.next();
                    }
                    current_row.push(std::mem::take(&mut field));
                    if !current_row.iter().all(|c| c.is_empty()) {
                        rows.push(std::mem::take(&mut current_row));
                        if rows.len() > MAX_QUARANTINE_ROWS + 1 {
                            return Err(QuarantineError::RowBudgetExceeded {
                                limit: MAX_QUARANTINE_ROWS,
                            });
                        }
                    } else {
                        current_row.clear();
                    }
                    line_no += 1;
                }
                '\n' => {
                    current_row.push(std::mem::take(&mut field));
                    if !current_row.iter().all(|c| c.is_empty()) {
                        rows.push(std::mem::take(&mut current_row));
                        if rows.len() > MAX_QUARANTINE_ROWS + 1 {
                            return Err(QuarantineError::RowBudgetExceeded {
                                limit: MAX_QUARANTINE_ROWS,
                            });
                        }
                    } else {
                        current_row.clear();
                    }
                    line_no += 1;
                }
                other => field.push(other),
            }
        }
    }

    if in_quotes {
        return Err(QuarantineError::CsvSyntax {
            line: line_no,
            detail: "unterminated quoted field".to_string(),
        });
    }
    if !field.is_empty() || !current_row.is_empty() {
        current_row.push(field);
        if !current_row.iter().all(|c| c.is_empty()) {
            rows.push(current_row);
        }
    }
    Ok(rows)
}

fn compute_report_digest(
    dialect_id: &str,
    cas_root: &Digest32,
    sha256_hex: &str,
    admitted: usize,
    rejected: &[RejectedRow],
) -> Digest32 {
    let mut enc = Encoder::new_frame(*b"IMPR", 1);
    enc.put_str(dialect_id);
    enc.put_fixed(&cas_root.0);
    enc.put_str(sha256_hex);
    enc.put_u64(admitted as u64);
    enc.put_u64(rejected.len() as u64);
    for r in rejected {
        enc.put_u64(r.row_index as u64);
        enc.put_str(&r.column);
        enc.put_str(&r.raw_value);
        enc.put_str(r.code);
        enc.put_str(&r.explanation);
    }
    sha256(&enc.finish())
}

#[cfg(test)]
mod tests {
    use super::*;
    use fmine_registry::RegistryCatalog;
    use std::fs;
    use std::path::PathBuf;

    fn workspace_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|p| p.parent())
            .expect("workspace root")
            .to_path_buf()
    }

    /// BD-185 done-when: rejected rows are explained with exact row index, column, raw value,
    /// registered FMINE-REF code and fix; original bytes live in CAS; nothing is silently dropped.
    #[test]
    fn test_quarantine_import_explains_every_rejected_row_and_pins_cas_root() {
        let root = workspace_root();
        let catalog = RegistryCatalog::load_from_root(&root).expect("catalog");
        let reg_units: BTreeSet<String> = catalog.unit_symbols.iter().cloned().collect();
        let reg_crs: BTreeSet<String> = catalog.crs_codes.iter().cloned().collect();
        let dialects = IoDialectRegistry::load_from_root(&root).expect("dialects");

        let dir = std::env::temp_dir().join(format!("fmine-io-quarantine-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let store = CasStore::open(&dir).expect("open cas");

        // 6 data rows: 2 valid (1 with <DL placeholder), 4 distinct defect classes:
        // row 4: negative Au grade (FMINE-REF-109)
        // row 5: Excel #REF! residue (FMINE-REF-208)
        // row 6: non-numeric garbage in Cu column (FMINE-REF-208)
        // row 7: short column count (FMINE-REF-208)
        let csv = b"HOLEID,FROM,TO,AU_GPT,CU_PCT\n\
DDH-01,0.0,1.5,2.45,0.82\n\
DDH-01,1.5,3.0,<DL,0.40\n\
DDH-01,3.0,4.5,-1.20,0.50\n\
DDH-01,4.5,6.0,#REF!,0.50\n\
DDH-01,6.0,7.5,1.10,BAD_NUM\n\
DDH-01,7.5,9.0\n";

        let report = import_csv_through_quarantine(
            &store,
            &dialects,
            "acquire_assay_v1",
            csv,
            &reg_units,
            &reg_crs,
            None,
        )
        .expect("import report");

        assert_eq!(report.total_data_rows, 6);
        assert_eq!(report.admitted_rows.len(), 2);
        assert_eq!(report.rejected_rows.len(), 4);
        assert_eq!(
            report.admitted_rows.len() + report.rejected_rows.len(),
            report.total_data_rows
        );
        // Original bytes stored in CAS match bit-for-bit.
        assert_eq!(
            store
                .read_object(&report.source_cas_root)
                .expect("cas bytes"),
            csv
        );

        assert_eq!(report.rejected_rows[0].row_index, 4);
        assert_eq!(report.rejected_rows[0].column, "AU_GPT");
        assert_eq!(report.rejected_rows[0].code, "FMINE-REF-109");

        assert_eq!(report.rejected_rows[1].row_index, 5);
        assert_eq!(report.rejected_rows[1].column, "AU_GPT");
        assert_eq!(report.rejected_rows[1].code, "FMINE-REF-208");

        assert_eq!(report.rejected_rows[2].row_index, 6);
        assert_eq!(report.rejected_rows[2].column, "CU_PCT");
        assert_eq!(report.rejected_rows[2].code, "FMINE-REF-208");

        assert_eq!(report.rejected_rows[3].row_index, 7);
        assert_eq!(report.rejected_rows[3].column, "<row>");
        assert_eq!(report.rejected_rows[3].code, "FMINE-REF-208");

        // Re-running on identical bytes yields the exact same CAS root and report digest.
        let report2 = import_csv_through_quarantine(
            &store,
            &dialects,
            "acquire_assay_v1",
            csv,
            &reg_units,
            &reg_crs,
            None,
        )
        .expect("rerun");
        assert_eq!(report2.source_cas_root, report.source_cas_root);
        assert_eq!(report2.report_digest, report.report_digest);

        let _ = fs::remove_dir_all(&dir);
    }
}
