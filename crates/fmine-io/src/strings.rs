//! Determinism: Exact
//!
//! Datamine CSV string/collar and Surpac ASCII `.str` text dialect readers (`BD-189`, plan §32.2).
//!
//! Surpac `.str` files are plain-text comma-separated coordinate streams:
//! - Line 1: header (`location, date, purpose, memo`)
//! - Line 2: axis record (`0, 0, 0, 0, 0, 0, 0`)
//! - Body: `string_id, Y (northing), X (easting), Z (elevation)[, desc1, ...]`
//! - Segment terminator: `0, 0, 0, 0` (or `0, 0.0, 0.0, 0.0`)
//! - File terminator: `0, 0, 0, 0, END`
//!
//! Both Datamine CSV exports (`datamine_string_v1`) and Surpac `.str` files (`surpac_string_v1`)
//! persist raw bytes root-last into `FMT-CAS-1`, bind their declared CRS from `io_dialects.toml`,
//! and report malformed coordinate lines as structured rejections rather than dropping them.

use std::collections::BTreeSet;

use fmine_core::hash::{Digest32, sha256};
use fmine_evidence::{Refusal, RefusalCode};
use fmine_object::cas::DEFAULT_CHUNK_BYTES;
use fmine_object::store::CasStore;

use crate::dialects::{IoDialectRegistry, ResolvedCrs};
use crate::quarantine::{
    ImportReport, MAX_QUARANTINE_BYTES, QuarantineError, RejectedRow, import_csv_through_quarantine,
};

/// One 3-D vertex in a Surpac `.str` polyline segment (Surpac order: Y=northing, X=easting, Z=RL).
#[derive(Debug, Clone, PartialEq)]
pub struct SurpacPoint {
    pub line_number: usize,
    pub string_id: u32,
    pub northing_y: f64,
    pub easting_x: f64,
    pub elevation_z: f64,
    pub descriptors: Vec<String>,
}

/// One contiguous polyline segment terminated by `0, 0, 0, 0` in a Surpac `.str` file.
#[derive(Debug, Clone, PartialEq)]
pub struct SurpacSegment {
    pub string_id: u32,
    pub points: Vec<SurpacPoint>,
}

/// Complete imported Surpac `.str` file bound to CAS bytes and its `surpac_string_v1` dialect CRS.
#[derive(Debug, Clone, PartialEq)]
pub struct SurpacImport {
    pub header_line: String,
    pub crs: ResolvedCrs,
    pub source_cas_root: Digest32,
    pub source_sha256: String,
    pub segments: Vec<SurpacSegment>,
    pub rejected_rows: Vec<RejectedRow>,
    pub saw_end_marker: bool,
}

/// Parses and imports a Surpac ASCII `.str` string file through CAS quarantine (`BD-189`).
pub fn import_surpac_str_through_quarantine(
    store: &CasStore,
    dialects: &IoDialectRegistry,
    raw_bytes: &[u8],
    registered_units: &BTreeSet<String>,
    registered_crs: &BTreeSet<String>,
) -> Result<SurpacImport, QuarantineError> {
    if raw_bytes.len() > MAX_QUARANTINE_BYTES {
        return Err(QuarantineError::ByteBudgetExceeded {
            actual: raw_bytes.len(),
            limit: MAX_QUARANTINE_BYTES,
        });
    }
    let cols = dialects
        .resolve_headers(
            "surpac_string_v1",
            &["Y".to_string(), "X".to_string(), "Z".to_string()],
            registered_units,
            registered_crs,
        )
        .map_err(QuarantineError::Dialect)?;
    let crs = cols.first().map_or(ResolvedCrs::None, |c| c.crs.clone());

    let source_sha256 = sha256(raw_bytes).to_hex();
    let staged = store
        .stage_object("text/x-surpac-str", raw_bytes, DEFAULT_CHUNK_BYTES)
        .map_err(QuarantineError::Cas)?;
    let manifest = store
        .publish_staged(&staged)
        .map_err(QuarantineError::Cas)?;

    let text = std::str::from_utf8(raw_bytes).map_err(|_| QuarantineError::CsvSyntax {
        line: 1,
        detail: "Surpac .str file is not valid UTF-8".to_string(),
    })?;

    let mut lines = text.lines().enumerate();
    let Some((_, header_line)) = lines.next() else {
        return Err(QuarantineError::EmptyInput);
    };
    let Some((_, _axis_line)) = lines.next() else {
        return Err(QuarantineError::CsvSyntax {
            line: 2,
            detail: "missing Surpac axis line".to_string(),
        });
    };

    let mut segments = Vec::new();
    let mut current_points: Vec<SurpacPoint> = Vec::new();
    let mut current_string_id: Option<u32> = None;
    let mut rejected_rows = Vec::new();
    let mut saw_end_marker = false;

    for (idx, raw_line) in lines {
        let line_no = idx + 1;
        let trimmed = raw_line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let parts: Vec<&str> = trimmed.split(',').map(str::trim).collect();
        if parts.len() < 4 {
            let r = Refusal::new(
                RefusalCode::SourceCellUnusable208,
                format!(
                    "Surpac record needs >= 4 fields (id, Y, X, Z), found {}",
                    parts.len()
                ),
            );
            rejected_rows.push(RejectedRow {
                row_index: line_no,
                column: "<str_line>".to_string(),
                raw_value: trimmed.to_string(),
                code: r.code,
                explanation: r.what,
                fix: r.fix,
            });
            continue;
        }

        let Ok(string_id) = parts[0].parse::<u32>() else {
            let r = Refusal::new(
                RefusalCode::SourceCellUnusable208,
                format!("invalid Surpac string_id '{}'", parts[0]),
            );
            rejected_rows.push(RejectedRow {
                row_index: line_no,
                column: "STRING_ID".to_string(),
                raw_value: parts[0].to_string(),
                code: r.code,
                explanation: r.what,
                fix: r.fix,
            });
            continue;
        };

        if string_id == 0 {
            if let Some(sid) = current_string_id.take()
                && !current_points.is_empty()
            {
                segments.push(SurpacSegment {
                    string_id: sid,
                    points: std::mem::take(&mut current_points),
                });
            }
            if parts.get(4).is_some_and(|d| d.eq_ignore_ascii_case("END")) {
                saw_end_marker = true;
                break;
            }
            continue;
        }

        let (Ok(y), Ok(x), Ok(z)) = (
            parts[1].parse::<f64>(),
            parts[2].parse::<f64>(),
            parts[3].parse::<f64>(),
        ) else {
            let r = Refusal::new(
                RefusalCode::SourceCellUnusable208,
                format!("non-numeric Y/X/Z coordinate in Surpac string {string_id}"),
            );
            rejected_rows.push(RejectedRow {
                row_index: line_no,
                column: "Y,X,Z".to_string(),
                raw_value: trimmed.to_string(),
                code: r.code,
                explanation: r.what,
                fix: r.fix,
            });
            continue;
        };
        if !y.is_finite() || !x.is_finite() || !z.is_finite() {
            let r = Refusal::new(
                RefusalCode::OutsidePhysicalDomain109,
                "non-finite coordinate in Surpac string".to_string(),
            );
            rejected_rows.push(RejectedRow {
                row_index: line_no,
                column: "Y,X,Z".to_string(),
                raw_value: trimmed.to_string(),
                code: r.code,
                explanation: r.what,
                fix: r.fix,
            });
            continue;
        }

        if let Some(prev_sid) = current_string_id
            && prev_sid != string_id
            && !current_points.is_empty()
        {
            segments.push(SurpacSegment {
                string_id: prev_sid,
                points: std::mem::take(&mut current_points),
            });
        }
        current_string_id = Some(string_id);
        let descriptors = parts[4..].iter().map(|s| (*s).to_string()).collect();
        current_points.push(SurpacPoint {
            line_number: line_no,
            string_id,
            northing_y: y,
            easting_x: x,
            elevation_z: z,
            descriptors,
        });
    }

    if let Some(sid) = current_string_id
        && !current_points.is_empty()
    {
        segments.push(SurpacSegment {
            string_id: sid,
            points: current_points,
        });
    }

    Ok(SurpacImport {
        header_line: header_line.to_string(),
        crs,
        source_cas_root: manifest.root_digest,
        source_sha256,
        segments,
        rejected_rows,
        saw_end_marker,
    })
}

/// Imports a Datamine CSV string/collar export through CAS quarantine using `datamine_string_v1`.
pub fn import_datamine_csv_through_quarantine(
    store: &CasStore,
    dialects: &IoDialectRegistry,
    raw_bytes: &[u8],
    registered_units: &BTreeSet<String>,
    registered_crs: &BTreeSet<String>,
) -> Result<ImportReport, QuarantineError> {
    import_csv_through_quarantine(
        store,
        dialects,
        "datamine_string_v1",
        raw_bytes,
        registered_units,
        registered_crs,
        None,
    )
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

    /// BD-189 done-when: sample Surpac `.str` text export and Datamine CSV string export both
    /// import cleanly with CRS binding, CAS root provenance, and malformed-row rejection.
    #[test]
    fn test_datamine_and_surpac_str_sample_exports_imported() {
        let root = workspace_root();
        let catalog = RegistryCatalog::load_from_root(&root).expect("catalog");
        let reg_units: BTreeSet<String> = catalog.unit_symbols.iter().cloned().collect();
        let reg_crs: BTreeSet<String> = catalog.crs_codes.iter().cloned().collect();
        let dialects = IoDialectRegistry::load_from_root(&root).expect("dialects");

        let dir = std::env::temp_dir().join(format!("fmine-io-strings-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let store = CasStore::open(&dir).expect("open cas");

        // 1. Surpac ASCII .str sample with 2 segments + 1 malformed line + END marker.
        let surpac_bytes = b"lumwana_pit_crest, 20-Nov-26, synthetic pit bench crest, \n\
0, 0.000, 0.000, 0.000, 0.000, 0.000, 0.000\n\
1, 8642100.50, 376450.25, 1320.00, CREST_1320\n\
1, 8642125.00, 376480.75, 1320.00, CREST_1320\n\
0, 0.000, 0.000, 0.000\n\
2, 8642105.00, 376455.00, 1308.00, TOE_1308\n\
2, NOT_A_NUM, 376460.00, 1308.00, BAD_PT\n\
2, 8642130.00, 376485.00, 1308.00, TOE_1308\n\
0, 0.000, 0.000, 0.000, END\n";

        let surpac = import_surpac_str_through_quarantine(
            &store,
            &dialects,
            surpac_bytes,
            &reg_units,
            &reg_crs,
        )
        .expect("import surpac");
        assert!(surpac.saw_end_marker);
        assert_eq!(
            surpac.crs,
            ResolvedCrs::Registered("EPSG:32735".to_string())
        );
        assert_eq!(surpac.segments.len(), 2);
        assert_eq!(surpac.segments[0].string_id, 1);
        assert_eq!(surpac.segments[0].points.len(), 2);
        assert_eq!(surpac.segments[1].string_id, 2);
        assert_eq!(surpac.segments[1].points.len(), 2);
        assert_eq!(surpac.rejected_rows.len(), 1);
        assert_eq!(surpac.rejected_rows[0].row_index, 7);
        assert_eq!(surpac.rejected_rows[0].code, "FMINE-REF-208");
        assert_eq!(
            store.read_object(&surpac.source_cas_root).expect("cas"),
            surpac_bytes
        );

        // 2. Datamine CSV string export with declared EPSG:32611 coordinates.
        let datamine_bytes = b"XP,YP,ZP\n551200.5,4521000.2,1650.0\n551225.0,4521015.8,1650.0\n";
        let dm = import_datamine_csv_through_quarantine(
            &store,
            &dialects,
            datamine_bytes,
            &reg_units,
            &reg_crs,
        )
        .expect("import datamine");
        assert_eq!(dm.admitted_rows.len(), 2);
        assert_eq!(dm.rejected_rows.len(), 0);
        assert_eq!(
            dm.resolved_columns[0].crs,
            ResolvedCrs::Registered("EPSG:32611".to_string())
        );

        let _ = fs::remove_dir_all(&dir);
    }
}
