//! Determinism: Exact
//!
//! acQuire GIM Suite drillhole export dialect importer (`BD-188`, plan §12.1 & §32.1–§32.2,
//! `PO-PORT-1..3`, `INV-014`).
//!
//! Imports multi-table acQuire drillhole export sets (**collar**, **survey**, and **assay**
//! tables in either RFC 4180 CSV or `.xlsx` format) through CAS quarantine (`FMT-CAS-1`,
//! root-last), resolving every column strictly through `registries/io_dialects.toml`
//! (`acquire_collar_v1`, `acquire_survey_v1`, `acquire_assay_v1`), redacting personal data
//! columns and inline PII on ingest (`BD-292`), and enforcing mining-domain integrity:
//! - Every numeric coordinate, depth, orientation, and grade is parsed into exact `ExactQty`
//!   rationals (zero binary64 drift).
//! - Left-censored below-detection-limit tokens (e.g. `<0.005`) are preserved as
//!   [`AssayReading::BelowDetection`] with their exact limit — never silently converted to `0`
//!   or half-DL. Placeholders (`<DL`, `BDL`, `NS`, `NSS`, `SNR`, `IS`, `-`, `N/A`, blank) stay
//!   [`AssayReading::Placeholder`].
//! - Physical domain bounds prevent Porgera-class errors (`S_PCT > 100%`, `AU_GPT > 1_000_000 g/t`,
//!   negative grades/depths, `FROM >= TO`, `DIP` outside `[-90, 90]`, `AZIMUTH` outside `[0, 360)`).
//! - Cross-table referential integrity enforces that every survey and assay references an admitted
//!   collar, depths never exceed `MAXDEPTH`, duplicate collar/survey keys are rejected, and
//!   overlapping assay intervals on the same hole are rejected (`FMINE-REF-102`).
//! - Nothing is silently dropped: every table-quarantine or domain rejection is returned in
//!   [`AcquireExportBundle::rejected_rows`] with table, 1-based row number, column, raw value,
//!   registered `FMINE-REF-*` code, explanation, and fix.

pub mod composite;
pub mod drill;

use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use fmine_core::codec::Encoder;
use fmine_core::hash::{Digest32, blake3_hash};
use fmine_evidence::{Refusal, RefusalCode};
use fmine_object::store::CasStore;
use fmine_units::ExactQty;

use crate::dialects::{ColumnBasis, IoDialectRegistry, ResolvedColumn, ResolvedCrs, ResolvedUnit};
use crate::quarantine::{
    ImportReport, QuarantineError, RejectedRow, import_csv_through_quarantine,
    import_xlsx_sheet_through_quarantine, normalize_decimal,
};
use crate::redact::scrub_free_text;

/// Formats an [`ExactQty`] canonically as an exact integer or reduced fraction (`n/d`).
fn fmt_qty(q: ExactQty) -> String {
    if q.den() == 1 {
        format!("{}", q.num())
    } else {
        format!("{}/{}", q.num(), q.den())
    }
}

/// Overflow-free exact total ordering over reduced `i128/i128` [`ExactQty`] rationals.
/// Uses `checked_cmp` on the fast path and falls back to exact continued-fraction comparison
/// (comparing Euclidean quotients and reciprocating remainders) when cross-multiplication would
/// overflow `i128`, so boundary checks can never fail open.
fn exact_cmp(a: ExactQty, b: ExactQty) -> Ordering {
    if let Ok(ord) = a.checked_cmp(b) {
        return ord;
    }
    let mut n1 = a.num();
    let mut d1 = a.den();
    let mut n2 = b.num();
    let mut d2 = b.den();
    let mut flipped = false;
    loop {
        let q1 = n1.div_euclid(d1);
        let q2 = n2.div_euclid(d2);
        if q1 != q2 {
            let ord = q1.cmp(&q2);
            return if flipped { ord.reverse() } else { ord };
        }
        let r1 = n1.rem_euclid(d1);
        let r2 = n2.rem_euclid(d2);
        match (r1 == 0, r2 == 0) {
            (true, true) => return Ordering::Equal,
            (true, false) => {
                return if flipped {
                    Ordering::Greater
                } else {
                    Ordering::Less
                };
            }
            (false, true) => {
                return if flipped {
                    Ordering::Less
                } else {
                    Ordering::Greater
                };
            }
            (false, false) => {
                // Compare d1/r1 vs d2/r2 with inverted sense since 0 < r_i < d_i.
                n1 = d1;
                d1 = r1;
                n2 = d2;
                d2 = r2;
                flipped = !flipped;
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct OrderedExactQty(ExactQty);

impl PartialOrd for OrderedExactQty {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for OrderedExactQty {
    fn cmp(&self, other: &Self) -> Ordering {
        exact_cmp(self.0, other.0)
    }
}

/// Registered acQuire collar dialect id in `registries/io_dialects.toml`.
pub const ACQUIRE_COLLAR_DIALECT: &str = "acquire_collar_v1";
/// Registered acQuire downhole survey dialect id in `registries/io_dialects.toml`.
pub const ACQUIRE_SURVEY_DIALECT: &str = "acquire_survey_v1";
/// Registered acQuire assay interval dialect id in `registries/io_dialects.toml`.
pub const ACQUIRE_ASSAY_DIALECT: &str = "acquire_assay_v1";

/// Source table role inside an acQuire drillhole export bundle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum AcquireTableKind {
    Collar,
    Survey,
    Assay,
}

impl AcquireTableKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Collar => "collar",
            Self::Survey => "survey",
            Self::Assay => "assay",
        }
    }
}

/// Source format of a single acQuire export table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AcquireTableInput<'a> {
    /// Raw RFC 4180 CSV bytes.
    Csv(&'a [u8]),
    /// Raw `.xlsx` workbook bytes and optional visible worksheet name (`None` = first visible sheet).
    Xlsx {
        bytes: &'a [u8],
        sheet: Option<&'a str>,
    },
}

/// Three-table acQuire drillhole export bundle input (collar, survey, assay).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcquireBundleInput<'a> {
    pub collars: AcquireTableInput<'a>,
    pub surveys: AcquireTableInput<'a>,
    pub assays: AcquireTableInput<'a>,
    /// Optional explicit site CRS override (must be registered in `registries/crs.toml`).
    /// When `None`, uses the CRS declared by `acquire_collar_v1` in `registries/io_dialects.toml`.
    pub site_crs: Option<&'a str>,
}

/// Preserved reading for one elemental assay cell (never silently substitutes 0 or DL/2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AssayReading {
    /// Uncensored numeric assay measurement in exact `ExactQty` rationals.
    Measured {
        value: ExactQty,
        unit: String,
        basis: ColumnBasis,
    },
    /// Left-censored below-detection-limit assay (e.g. `<0.005`), preserving exact upper bound.
    BelowDetection {
        detection_limit: ExactQty,
        unit: String,
        basis: ColumnBasis,
        raw: String,
    },
    /// Explicit missing / unassayed placeholder (`<DL`, `BDL`, `NS`, `NSS`, `SNR`, `IS`, `-`, `N/A`, blank).
    Placeholder { raw: String },
}

/// Admitted acQuire collar row with exact coordinates and registered CRS.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcquireCollar {
    pub row_index: usize,
    pub hole_id: String,
    pub project_code: Option<String>,
    pub easting_m: ExactQty,
    pub northing_m: ExactQty,
    pub rl_m: ExactQty,
    pub max_depth_m: ExactQty,
    pub crs: String,
    pub geologist_redacted: Option<String>,
}

/// Admitted acQuire downhole survey station (`DEPTH` in `[0, MAXDEPTH]`, `AZIMUTH` in `[0, 360)`, `DIP` in `[-90, 90]`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcquireSurvey {
    pub row_index: usize,
    pub hole_id: String,
    pub depth_m: ExactQty,
    pub azimuth_deg: ExactQty,
    pub dip_deg: ExactQty,
    pub survey_type: Option<String>,
}

/// Admitted acQuire assay interval (`0 <= FROM < TO <= MAXDEPTH`, non-overlapping within each hole).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcquireAssayInterval {
    pub row_index: usize,
    pub hole_id: String,
    pub sample_id: Option<String>,
    pub from_m: ExactQty,
    pub to_m: ExactQty,
    pub lab_job: Option<String>,
    pub method: Option<String>,
    /// Map from canonical dialect assay column name (e.g. `AU_GPT`, `CU_PCT`, `AG_GPT`, `S_PCT`)
    /// to its exact typed [`AssayReading`].
    pub assays: BTreeMap<String, AssayReading>,
}

/// Structured rejection record for any collar, survey, or assay row (quarantine or domain validation).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcquireRejection {
    pub table: AcquireTableKind,
    pub row_index: usize,
    pub column: String,
    pub raw_value: String,
    pub code: &'static str,
    pub explanation: String,
    pub fix: &'static str,
}

impl AcquireRejection {
    fn from_quarantine(table: AcquireTableKind, rej: &RejectedRow) -> Self {
        Self {
            table,
            row_index: rej.row_index,
            column: rej.column.clone(),
            raw_value: rej.raw_value.clone(),
            code: rej.code,
            explanation: rej.explanation.clone(),
            fix: rej.fix,
        }
    }

    fn from_refusal(
        table: AcquireTableKind,
        row_index: usize,
        column: impl Into<String>,
        raw_value: impl Into<String>,
        refusal: Refusal,
    ) -> Self {
        let raw_value = raw_value.into();
        // Known numeric values keep their precision; free-text diagnostics obey ingest redaction.
        let raw_value = if normalize_decimal(&raw_value).is_some() {
            raw_value
        } else {
            scrub_free_text(&raw_value).0
        };
        Self {
            table,
            row_index,
            column: scrub_free_text(&column.into()).0,
            raw_value,
            code: refusal.code,
            explanation: scrub_free_text(&refusal.what).0,
            fix: refusal.fix,
        }
    }
}

/// Complete imported acQuire drillhole export bundle bound to CAS roots and deterministic digest (`BD-188`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcquireExportBundle {
    pub crs: String,
    pub collar_report: ImportReport,
    pub survey_report: ImportReport,
    pub assay_report: ImportReport,
    pub collars: Vec<AcquireCollar>,
    pub surveys: Vec<AcquireSurvey>,
    pub assays: Vec<AcquireAssayInterval>,
    pub rejected_rows: Vec<AcquireRejection>,
    pub bundle_digest: Digest32,
}

/// Bundle-level errors that prevent importing an acQuire export set (before row classification).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AcquireError {
    Quarantine {
        table: AcquireTableKind,
        source: QuarantineError,
    },
    MissingRequiredColumn {
        table: AcquireTableKind,
        column: &'static str,
    },
    DuplicateHeaderColumn {
        table: AcquireTableKind,
        column: String,
    },
    UnregisteredCrs {
        crs: String,
    },
}

impl AcquireError {
    #[must_use]
    pub fn to_refusal(&self) -> Refusal {
        match self {
            Self::Quarantine { source, .. } => source.to_refusal(),
            Self::MissingRequiredColumn { .. } | Self::DuplicateHeaderColumn { .. } => {
                Refusal::new(
                    RefusalCode::SourceCellUnusable208,
                    scrub_free_text(&self.to_string()).0,
                )
            }
            Self::UnregisteredCrs { .. } => Refusal::new(
                RefusalCode::CrsUnregisteredOrIncompatible106,
                scrub_free_text(&self.to_string()).0,
            ),
        }
    }
}

impl fmt::Display for AcquireError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Quarantine { table, source } => {
                write!(f, "acQuire {} quarantine error: {source}", table.as_str())
            }
            Self::MissingRequiredColumn { table, column } => {
                write!(
                    f,
                    "acQuire {} export missing required column '{column}'",
                    table.as_str()
                )
            }
            Self::DuplicateHeaderColumn { table, column } => {
                write!(
                    f,
                    "acQuire {} export contains duplicate header column '{column}'",
                    table.as_str()
                )
            }
            Self::UnregisteredCrs { crs } => {
                write!(
                    f,
                    "acQuire collar CRS '{crs}' is not registered in registries/crs.toml"
                )
            }
        }
    }
}

impl std::error::Error for AcquireError {}

/// Imports a multi-table acQuire drillhole export (collar + survey + assay) through CAS quarantine,
/// dialect resolution, PII redaction, and Corebox physical/referential integrity checks (`BD-188`).
pub fn import_acquire_export_bundle(
    store: &CasStore,
    dialects: &IoDialectRegistry,
    input: &AcquireBundleInput<'_>,
    registered_units: &BTreeSet<String>,
    registered_crs: &BTreeSet<String>,
    privacy_officer_key: Option<&[u8; 32]>,
) -> Result<AcquireExportBundle, AcquireError> {
    let collar_report = run_table_quarantine(
        store,
        dialects,
        AcquireTableKind::Collar,
        ACQUIRE_COLLAR_DIALECT,
        &input.collars,
        registered_units,
        registered_crs,
        privacy_officer_key,
    )?;
    let survey_report = run_table_quarantine(
        store,
        dialects,
        AcquireTableKind::Survey,
        ACQUIRE_SURVEY_DIALECT,
        &input.surveys,
        registered_units,
        registered_crs,
        privacy_officer_key,
    )?;
    let assay_report = run_table_quarantine(
        store,
        dialects,
        AcquireTableKind::Assay,
        ACQUIRE_ASSAY_DIALECT,
        &input.assays,
        registered_units,
        registered_crs,
        privacy_officer_key,
    )?;

    let collar_idx = header_index_map(AcquireTableKind::Collar, &collar_report.resolved_columns)?;
    let survey_idx = header_index_map(AcquireTableKind::Survey, &survey_report.resolved_columns)?;
    let assay_idx = header_index_map(AcquireTableKind::Assay, &assay_report.resolved_columns)?;

    let c_hole = req_col(AcquireTableKind::Collar, &collar_idx, "HOLEID")?;
    let c_east = req_col(AcquireTableKind::Collar, &collar_idx, "EAST")?;
    let c_north = req_col(AcquireTableKind::Collar, &collar_idx, "NORTH")?;
    let c_rl = req_col(AcquireTableKind::Collar, &collar_idx, "RL")?;
    let c_max = req_col(AcquireTableKind::Collar, &collar_idx, "MAXDEPTH")?;
    let c_proj = collar_idx.get("PROJECTCODE").copied();
    let c_geo = collar_idx.get("GEOLOGIST").copied();

    let s_hole = req_col(AcquireTableKind::Survey, &survey_idx, "HOLEID")?;
    let s_depth = req_col(AcquireTableKind::Survey, &survey_idx, "DEPTH")?;
    let s_azim = req_col(AcquireTableKind::Survey, &survey_idx, "AZIMUTH")?;
    let s_dip = req_col(AcquireTableKind::Survey, &survey_idx, "DIP")?;
    let s_type = survey_idx.get("SURVEYTYPE").copied();

    let a_hole = req_col(AcquireTableKind::Assay, &assay_idx, "HOLEID")?;
    let a_from = req_col(AcquireTableKind::Assay, &assay_idx, "FROM")?;
    let a_to = req_col(AcquireTableKind::Assay, &assay_idx, "TO")?;
    let a_sample = assay_idx.get("SAMPLEID").copied();
    let a_job = assay_idx.get("LABJOBNO").copied();
    let a_method = assay_idx.get("METHOD").copied();

    let crs = resolve_bundle_crs(
        input.site_crs,
        &[
            &collar_report.resolved_columns[c_east],
            &collar_report.resolved_columns[c_north],
            &collar_report.resolved_columns[c_rl],
        ],
        registered_crs,
    )?;

    let mut rejected_rows = Vec::new();
    for rej in &collar_report.rejected_rows {
        rejected_rows.push(AcquireRejection::from_quarantine(
            AcquireTableKind::Collar,
            rej,
        ));
    }
    for rej in &survey_report.rejected_rows {
        rejected_rows.push(AcquireRejection::from_quarantine(
            AcquireTableKind::Survey,
            rej,
        ));
    }
    for rej in &assay_report.rejected_rows {
        rejected_rows.push(AcquireRejection::from_quarantine(
            AcquireTableKind::Assay,
            rej,
        ));
    }

    // 1. Validate & admit collars.
    let mut collars_by_hole: BTreeMap<String, AcquireCollar> = BTreeMap::new();
    for row in &collar_report.admitted_rows {
        let hole_id = row.cells[c_hole].trim();
        if hole_id.is_empty() {
            rejected_rows.push(AcquireRejection::from_refusal(
                AcquireTableKind::Collar,
                row.row_index,
                "HOLEID",
                &row.cells[c_hole],
                Refusal::new(
                    RefusalCode::SourceCellUnusable208,
                    "acQuire collar row has empty HOLEID",
                ),
            ));
            continue;
        }
        if let Some(existing) = collars_by_hole.get(hole_id) {
            rejected_rows.push(AcquireRejection::from_refusal(
                AcquireTableKind::Collar,
                row.row_index,
                "HOLEID",
                hole_id,
                Refusal::new(
                    RefusalCode::SourceCellUnusable208,
                    format!(
                        "duplicate collar HOLEID '{hole_id}' (first admitted at row {})",
                        existing.row_index
                    ),
                ),
            ));
            continue;
        }
        let easting_m = match parse_exact_required(
            AcquireTableKind::Collar,
            row.row_index,
            "EAST",
            &row.cells[c_east],
        ) {
            Ok(v) => v,
            Err(rej) => {
                rejected_rows.push(rej);
                continue;
            }
        };
        let northing_m = match parse_exact_required(
            AcquireTableKind::Collar,
            row.row_index,
            "NORTH",
            &row.cells[c_north],
        ) {
            Ok(v) => v,
            Err(rej) => {
                rejected_rows.push(rej);
                continue;
            }
        };
        let rl_m = match parse_exact_required(
            AcquireTableKind::Collar,
            row.row_index,
            "RL",
            &row.cells[c_rl],
        ) {
            Ok(v) => v,
            Err(rej) => {
                rejected_rows.push(rej);
                continue;
            }
        };
        let max_depth_m = match parse_exact_required(
            AcquireTableKind::Collar,
            row.row_index,
            "MAXDEPTH",
            &row.cells[c_max],
        ) {
            Ok(v) => v,
            Err(rej) => {
                rejected_rows.push(rej);
                continue;
            }
        };
        if max_depth_m.signum() <= 0 {
            rejected_rows.push(AcquireRejection::from_refusal(
                AcquireTableKind::Collar,
                row.row_index,
                "MAXDEPTH",
                &row.cells[c_max],
                Refusal::new(
                    RefusalCode::OutsidePhysicalDomain109,
                    format!(
                        "collar '{hole_id}' MAXDEPTH must be strictly positive, got '{}'",
                        row.cells[c_max].trim()
                    ),
                ),
            ));
            continue;
        }
        let project_code = c_proj
            .map(|i| row.cells[i].trim())
            .filter(|s| !s.is_empty())
            .map(ToOwned::to_owned);
        let geologist_redacted = c_geo
            .map(|i| row.cells[i].trim())
            .filter(|s| !s.is_empty())
            .map(ToOwned::to_owned);
        collars_by_hole.insert(
            hole_id.to_string(),
            AcquireCollar {
                row_index: row.row_index,
                hole_id: hole_id.to_string(),
                project_code,
                easting_m,
                northing_m,
                rl_m,
                max_depth_m,
                crs: crs.clone(),
                geologist_redacted,
            },
        );
    }

    // 2. Validate & admit downhole surveys against admitted collars.
    let zero = ExactQty::from_integer(0);
    let deg_360 = ExactQty::from_integer(360);
    let deg_neg_90 = ExactQty::from_integer(-90);
    let deg_pos_90 = ExactQty::from_integer(90);
    let mut surveys_by_key: BTreeMap<(String, OrderedExactQty), AcquireSurvey> = BTreeMap::new();
    for row in &survey_report.admitted_rows {
        let hole_id = row.cells[s_hole].trim();
        let Some(collar) = collars_by_hole.get(hole_id) else {
            rejected_rows.push(AcquireRejection::from_refusal(
                AcquireTableKind::Survey,
                row.row_index,
                "HOLEID",
                hole_id,
                Refusal::new(
                    RefusalCode::NoClaimInDomain201,
                    format!("survey references unknown or rejected collar HOLEID '{hole_id}'"),
                ),
            ));
            continue;
        };
        let depth_m = match parse_exact_required(
            AcquireTableKind::Survey,
            row.row_index,
            "DEPTH",
            &row.cells[s_depth],
        ) {
            Ok(v) => v,
            Err(rej) => {
                rejected_rows.push(rej);
                continue;
            }
        };
        if depth_m.signum() < 0 || exact_cmp(depth_m, collar.max_depth_m).is_gt() {
            rejected_rows.push(AcquireRejection::from_refusal(
                AcquireTableKind::Survey,
                row.row_index,
                "DEPTH",
                &row.cells[s_depth],
                Refusal::new(
                    RefusalCode::OutsidePhysicalDomain109,
                    format!(
                        "survey DEPTH '{}' for '{hole_id}' outside [0, MAXDEPTH {}]",
                        row.cells[s_depth].trim(),
                        fmt_qty(collar.max_depth_m)
                    ),
                ),
            ));
            continue;
        }
        let azimuth_deg = match parse_exact_required(
            AcquireTableKind::Survey,
            row.row_index,
            "AZIMUTH",
            &row.cells[s_azim],
        ) {
            Ok(v) => v,
            Err(rej) => {
                rejected_rows.push(rej);
                continue;
            }
        };
        if exact_cmp(azimuth_deg, zero).is_lt() || exact_cmp(azimuth_deg, deg_360).is_ge() {
            rejected_rows.push(AcquireRejection::from_refusal(
                AcquireTableKind::Survey,
                row.row_index,
                "AZIMUTH",
                &row.cells[s_azim],
                Refusal::new(
                    RefusalCode::OutsidePhysicalDomain109,
                    format!(
                        "survey AZIMUTH '{}' for '{hole_id}' outside [0, 360) degrees",
                        row.cells[s_azim].trim()
                    ),
                ),
            ));
            continue;
        }
        let dip_deg = match parse_exact_required(
            AcquireTableKind::Survey,
            row.row_index,
            "DIP",
            &row.cells[s_dip],
        ) {
            Ok(v) => v,
            Err(rej) => {
                rejected_rows.push(rej);
                continue;
            }
        };
        if exact_cmp(dip_deg, deg_neg_90).is_lt() || exact_cmp(dip_deg, deg_pos_90).is_gt() {
            rejected_rows.push(AcquireRejection::from_refusal(
                AcquireTableKind::Survey,
                row.row_index,
                "DIP",
                &row.cells[s_dip],
                Refusal::new(
                    RefusalCode::OutsidePhysicalDomain109,
                    format!(
                        "survey DIP '{}' for '{hole_id}' outside [-90, 90] degrees",
                        row.cells[s_dip].trim()
                    ),
                ),
            ));
            continue;
        }
        let key = (hole_id.to_string(), OrderedExactQty(depth_m));
        if let Some(existing) = surveys_by_key.get(&key) {
            rejected_rows.push(AcquireRejection::from_refusal(
                AcquireTableKind::Survey,
                row.row_index,
                "DEPTH",
                &row.cells[s_depth],
                Refusal::new(
                    RefusalCode::SourceCellUnusable208,
                    format!(
                        "duplicate survey station for '{hole_id}' at depth {} (first at row {})",
                        fmt_qty(depth_m),
                        existing.row_index
                    ),
                ),
            ));
            continue;
        }
        let survey_type = s_type
            .map(|i| row.cells[i].trim())
            .filter(|s| !s.is_empty())
            .map(ToOwned::to_owned);
        surveys_by_key.insert(
            key,
            AcquireSurvey {
                row_index: row.row_index,
                hole_id: hole_id.to_string(),
                depth_m,
                azimuth_deg,
                dip_deg,
                survey_type,
            },
        );
    }

    // 3. Identify registered physical assay columns (excluding FROM/TO depth columns).
    let assay_value_cols: Vec<(usize, &ResolvedColumn, String, ColumnBasis)> = assay_report
        .resolved_columns
        .iter()
        .enumerate()
        .filter_map(|(idx, col)| {
            if idx == a_from || idx == a_to {
                return None;
            }
            if let ResolvedUnit::Registered(unit) = &col.unit {
                Some((
                    idx,
                    col,
                    unit.clone(),
                    col.basis.unwrap_or(ColumnBasis::None),
                ))
            } else {
                None
            }
        })
        .collect();

    // 4. Validate & admit assay intervals against collars + physical bounds + non-overlap check.
    let mut candidate_assays: Vec<AcquireAssayInterval> = Vec::new();
    for row in &assay_report.admitted_rows {
        let hole_id = row.cells[a_hole].trim();
        let Some(collar) = collars_by_hole.get(hole_id) else {
            rejected_rows.push(AcquireRejection::from_refusal(
                AcquireTableKind::Assay,
                row.row_index,
                "HOLEID",
                hole_id,
                Refusal::new(
                    RefusalCode::NoClaimInDomain201,
                    format!(
                        "assay interval references unknown or rejected collar HOLEID '{hole_id}'"
                    ),
                ),
            ));
            continue;
        };
        let from_m = match parse_exact_required(
            AcquireTableKind::Assay,
            row.row_index,
            "FROM",
            &row.cells[a_from],
        ) {
            Ok(v) => v,
            Err(rej) => {
                rejected_rows.push(rej);
                continue;
            }
        };
        let to_m = match parse_exact_required(
            AcquireTableKind::Assay,
            row.row_index,
            "TO",
            &row.cells[a_to],
        ) {
            Ok(v) => v,
            Err(rej) => {
                rejected_rows.push(rej);
                continue;
            }
        };
        if from_m.signum() < 0
            || exact_cmp(to_m, from_m).is_le()
            || exact_cmp(to_m, collar.max_depth_m).is_gt()
        {
            rejected_rows.push(AcquireRejection::from_refusal(
                AcquireTableKind::Assay,
                row.row_index,
                "FROM,TO",
                format!("{}..{}", row.cells[a_from].trim(), row.cells[a_to].trim()),
                Refusal::new(
                    RefusalCode::OutsidePhysicalDomain109,
                    format!(
                        "assay interval [{}, {}] for '{hole_id}' violates 0 <= FROM < TO <= MAXDEPTH ({})",
                        fmt_qty(from_m),
                        fmt_qty(to_m),
                        fmt_qty(collar.max_depth_m)
                    ),
                ),
            ));
            continue;
        }

        let mut readings = BTreeMap::new();
        let mut cell_rejection: Option<AcquireRejection> = None;
        for &(col_idx, col, ref unit, basis) in &assay_value_cols {
            let raw = row.cells[col_idx].trim();
            match parse_assay_reading(row.row_index, &col.column_name, raw, unit, basis) {
                Ok(reading) => {
                    readings.insert(col.column_name.clone(), reading);
                }
                Err(rej) => {
                    cell_rejection = Some(rej);
                    break;
                }
            }
        }
        if let Some(rej) = cell_rejection {
            rejected_rows.push(rej);
            continue;
        }

        candidate_assays.push(AcquireAssayInterval {
            row_index: row.row_index,
            hole_id: hole_id.to_string(),
            sample_id: a_sample
                .map(|i| row.cells[i].trim())
                .filter(|s| !s.is_empty())
                .map(ToOwned::to_owned),
            from_m,
            to_m,
            lab_job: a_job
                .map(|i| row.cells[i].trim())
                .filter(|s| !s.is_empty())
                .map(ToOwned::to_owned),
            method: a_method
                .map(|i| row.cells[i].trim())
                .filter(|s| !s.is_empty())
                .map(ToOwned::to_owned),
            assays: readings,
        });
    }

    // Sort intervals canonically by (hole_id, from_m, to_m, row_index) and reject overlaps per hole.
    candidate_assays.sort_by(|a, b| {
        a.hole_id
            .cmp(&b.hole_id)
            .then_with(|| exact_cmp(a.from_m, b.from_m))
            .then_with(|| exact_cmp(a.to_m, b.to_m))
            .then_with(|| a.row_index.cmp(&b.row_index))
    });
    let mut assays: Vec<AcquireAssayInterval> = Vec::with_capacity(candidate_assays.len());
    let mut last_by_hole: BTreeMap<String, (ExactQty, ExactQty, usize)> = BTreeMap::new();
    for iv in candidate_assays {
        if let Some(&(prev_from, prev_to, prev_row)) = last_by_hole.get(&iv.hole_id)
            && exact_cmp(iv.from_m, prev_to).is_lt()
        {
            rejected_rows.push(AcquireRejection::from_refusal(
                AcquireTableKind::Assay,
                iv.row_index,
                "FROM,TO",
                format!("{}..{}", fmt_qty(iv.from_m), fmt_qty(iv.to_m)),
                Refusal::new(
                    RefusalCode::SupportMismatch102,
                    format!(
                        "overlapping assay interval [{}, {}] on hole '{}' overlaps [{}, {}] from row {prev_row}",
                        fmt_qty(iv.from_m),
                        fmt_qty(iv.to_m),
                        iv.hole_id,
                        fmt_qty(prev_from),
                        fmt_qty(prev_to)
                    ),
                ),
            ));
            continue;
        }
        last_by_hole.insert(iv.hole_id.clone(), (iv.from_m, iv.to_m, iv.row_index));
        assays.push(iv);
    }

    rejected_rows.sort_by(|a, b| {
        a.table
            .cmp(&b.table)
            .then_with(|| a.row_index.cmp(&b.row_index))
            .then_with(|| a.column.cmp(&b.column))
            .then_with(|| a.code.cmp(b.code))
    });

    let collars: Vec<AcquireCollar> = collars_by_hole.into_values().collect();
    let surveys: Vec<AcquireSurvey> = surveys_by_key.into_values().collect();
    let bundle_digest = compute_bundle_digest(
        &crs,
        &collar_report,
        &survey_report,
        &assay_report,
        &collars,
        &surveys,
        &assays,
        &rejected_rows,
    );

    Ok(AcquireExportBundle {
        crs,
        collar_report,
        survey_report,
        assay_report,
        collars,
        surveys,
        assays,
        rejected_rows,
        bundle_digest,
    })
}

#[allow(clippy::too_many_arguments)]
fn run_table_quarantine(
    store: &CasStore,
    dialects: &IoDialectRegistry,
    table: AcquireTableKind,
    dialect_id: &str,
    input: &AcquireTableInput<'_>,
    registered_units: &BTreeSet<String>,
    registered_crs: &BTreeSet<String>,
    privacy_officer_key: Option<&[u8; 32]>,
) -> Result<ImportReport, AcquireError> {
    let res = match input {
        AcquireTableInput::Csv(bytes) => import_csv_through_quarantine(
            store,
            dialects,
            dialect_id,
            bytes,
            registered_units,
            registered_crs,
            privacy_officer_key,
        ),
        AcquireTableInput::Xlsx { bytes, sheet } => import_xlsx_sheet_through_quarantine(
            store,
            dialects,
            dialect_id,
            bytes,
            *sheet,
            registered_units,
            registered_crs,
            privacy_officer_key,
        ),
    };
    res.map_err(|source| AcquireError::Quarantine { table, source })
}

fn header_index_map(
    table: AcquireTableKind,
    columns: &[ResolvedColumn],
) -> Result<BTreeMap<String, usize>, AcquireError> {
    let mut map = BTreeMap::new();
    for (idx, col) in columns.iter().enumerate() {
        if map.insert(col.column_name.clone(), idx).is_some() {
            return Err(AcquireError::DuplicateHeaderColumn {
                table,
                column: col.column_name.clone(),
            });
        }
    }
    Ok(map)
}

fn req_col(
    table: AcquireTableKind,
    map: &BTreeMap<String, usize>,
    column: &'static str,
) -> Result<usize, AcquireError> {
    map.get(column)
        .copied()
        .ok_or(AcquireError::MissingRequiredColumn { table, column })
}

fn resolve_bundle_crs(
    site_crs_override: Option<&str>,
    coord_cols: &[&ResolvedColumn],
    registered_crs: &BTreeSet<String>,
) -> Result<String, AcquireError> {
    if let Some(crs) = site_crs_override {
        let trimmed = crs.trim();
        if registered_crs.contains(trimmed) {
            return Ok(trimmed.to_string());
        }
        return Err(AcquireError::UnregisteredCrs {
            crs: trimmed.to_string(),
        });
    }
    let mut resolved_code: Option<String> = None;
    for col in coord_cols {
        match &col.crs {
            ResolvedCrs::Registered(code) => {
                if let Some(prev) = &resolved_code {
                    if prev != code {
                        return Err(AcquireError::UnregisteredCrs {
                            crs: format!("mismatched collar coordinate CRS '{prev}' vs '{code}'"),
                        });
                    }
                } else {
                    resolved_code = Some(code.clone());
                }
            }
            ResolvedCrs::Unregistered { declared } => {
                return Err(AcquireError::UnregisteredCrs {
                    crs: declared.clone(),
                });
            }
            ResolvedCrs::None => {
                return Err(AcquireError::UnregisteredCrs {
                    crs: format!("none on collar column '{}'", col.column_name),
                });
            }
        }
    }
    resolved_code.ok_or_else(|| AcquireError::UnregisteredCrs {
        crs: "none".to_string(),
    })
}

fn parse_exact_required(
    table: AcquireTableKind,
    row_index: usize,
    column: &str,
    raw: &str,
) -> Result<ExactQty, AcquireRejection> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(AcquireRejection::from_refusal(
            table,
            row_index,
            column,
            raw,
            Refusal::new(
                RefusalCode::SourceCellUnusable208,
                format!("required numeric column '{column}' is blank"),
            ),
        ));
    }
    normalize_decimal(trimmed)
        .and_then(|cleaned| ExactQty::from_decimal_str(&cleaned).ok())
        .ok_or_else(|| {
            AcquireRejection::from_refusal(
                table,
                row_index,
                column,
                raw,
                Refusal::new(
                    RefusalCode::SourceCellUnusable208,
                    format!("invalid exact decimal '{trimmed}' in required column '{column}'"),
                ),
            )
        })
}

fn parse_assay_reading(
    row_index: usize,
    column: &str,
    raw: &str,
    unit: &str,
    basis: ColumnBasis,
) -> Result<AssayReading, AcquireRejection> {
    if raw.is_empty()
        || matches!(
            raw,
            "-" | "—" | "N/A" | "n/a" | "NA" | "<DL" | "BDL" | "NS" | "NSS" | "SNR" | "IS"
        )
    {
        return Ok(AssayReading::Placeholder {
            raw: raw.to_string(),
        });
    }
    if let Some(rest) = raw.strip_prefix('<') {
        let trimmed_rest = rest.trim();
        if trimmed_rest.starts_with('+') || trimmed_rest.starts_with('-') {
            return Err(AcquireRejection::from_refusal(
                AcquireTableKind::Assay,
                row_index,
                column,
                raw,
                Refusal::new(
                    RefusalCode::SourceCellUnusable208,
                    format!("malformed signed below-detection-limit token '{raw}' in '{column}'"),
                ),
            ));
        }
        let limit = normalize_decimal(trimmed_rest)
            .and_then(|cleaned| ExactQty::from_decimal_str(&cleaned).ok())
            .ok_or_else(|| {
                AcquireRejection::from_refusal(
                    AcquireTableKind::Assay,
                    row_index,
                    column,
                    raw,
                    Refusal::new(
                        RefusalCode::SourceCellUnusable208,
                        format!("malformed below-detection-limit token '{raw}' in '{column}'"),
                    ),
                )
            })?;
        if limit.signum() <= 0 {
            return Err(AcquireRejection::from_refusal(
                AcquireTableKind::Assay,
                row_index,
                column,
                raw,
                Refusal::new(
                    RefusalCode::OutsidePhysicalDomain109,
                    format!("below-detection limit in '{column}' must be > 0, got '{raw}'"),
                ),
            ));
        }
        check_physical_upper_bound(row_index, column, raw, unit, limit)?;
        return Ok(AssayReading::BelowDetection {
            detection_limit: limit,
            unit: unit.to_string(),
            basis,
            raw: raw.to_string(),
        });
    }
    let value = normalize_decimal(raw)
        .and_then(|cleaned| ExactQty::from_decimal_str(&cleaned).ok())
        .ok_or_else(|| {
            AcquireRejection::from_refusal(
                AcquireTableKind::Assay,
                row_index,
                column,
                raw,
                Refusal::new(
                    RefusalCode::SourceCellUnusable208,
                    format!("invalid numeric assay '{raw}' in '{column}'"),
                ),
            )
        })?;
    if value.signum() < 0 {
        return Err(AcquireRejection::from_refusal(
            AcquireTableKind::Assay,
            row_index,
            column,
            raw,
            Refusal::new(
                RefusalCode::OutsidePhysicalDomain109,
                format!("negative assay '{raw}' ({unit}) in '{column}'"),
            ),
        ));
    }
    check_physical_upper_bound(row_index, column, raw, unit, value)?;
    Ok(AssayReading::Measured {
        value,
        unit: unit.to_string(),
        basis,
    })
}

/// Enforces physical mass-fraction upper bounds (`wt_pct <= 100`, `g_per_t <= 1_000_000`,
/// `mass_fraction <= 1`), catching Porgera-style `S > 100%` corruption (`plan §12.1`).
fn check_physical_upper_bound(
    row_index: usize,
    column: &str,
    raw: &str,
    unit: &str,
    qty: ExactQty,
) -> Result<(), AcquireRejection> {
    let max_opt = match unit {
        "wt_pct" => Some((ExactQty::from_integer(100), "100 wt%")),
        "g_per_t" => Some((ExactQty::from_integer(1_000_000), "1,000,000 g/t (100%)")),
        "mass_fraction" => Some((ExactQty::from_integer(1), "1.0 mass fraction")),
        _ => None,
    };
    if let Some((limit, label)) = max_opt
        && exact_cmp(qty, limit).is_gt()
    {
        return Err(AcquireRejection::from_refusal(
            AcquireTableKind::Assay,
            row_index,
            column,
            raw,
            Refusal::new(
                RefusalCode::OutsidePhysicalDomain109,
                format!("assay '{raw}' ({unit}) in '{column}' exceeds physical maximum {label}"),
            ),
        ));
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn compute_bundle_digest(
    crs: &str,
    collar_report: &ImportReport,
    survey_report: &ImportReport,
    assay_report: &ImportReport,
    collars: &[AcquireCollar],
    surveys: &[AcquireSurvey],
    assays: &[AcquireAssayInterval],
    rejected_rows: &[AcquireRejection],
) -> Digest32 {
    let mut enc = Encoder::new_frame(*b"ACQR", 1);
    enc.put_str(crs);
    enc.put_fixed(collar_report.source_cas_root.as_bytes());
    enc.put_fixed(survey_report.source_cas_root.as_bytes());
    enc.put_fixed(assay_report.source_cas_root.as_bytes());
    enc.put_u64(collars.len() as u64);
    for c in collars {
        enc.put_str(&c.hole_id);
        enc.put_str(c.project_code.as_deref().unwrap_or(""));
        enc.put_str(c.geologist_redacted.as_deref().unwrap_or(""));
        enc.put_i128(c.easting_m.num());
        enc.put_i128(c.easting_m.den());
        enc.put_i128(c.northing_m.num());
        enc.put_i128(c.northing_m.den());
        enc.put_i128(c.rl_m.num());
        enc.put_i128(c.rl_m.den());
        enc.put_i128(c.max_depth_m.num());
        enc.put_i128(c.max_depth_m.den());
    }
    enc.put_u64(surveys.len() as u64);
    for s in surveys {
        enc.put_str(&s.hole_id);
        enc.put_str(s.survey_type.as_deref().unwrap_or(""));
        enc.put_i128(s.depth_m.num());
        enc.put_i128(s.depth_m.den());
        enc.put_i128(s.azimuth_deg.num());
        enc.put_i128(s.azimuth_deg.den());
        enc.put_i128(s.dip_deg.num());
        enc.put_i128(s.dip_deg.den());
    }
    enc.put_u64(assays.len() as u64);
    for a in assays {
        enc.put_str(&a.hole_id);
        enc.put_str(a.sample_id.as_deref().unwrap_or(""));
        enc.put_str(a.lab_job.as_deref().unwrap_or(""));
        enc.put_str(a.method.as_deref().unwrap_or(""));
        enc.put_i128(a.from_m.num());
        enc.put_i128(a.from_m.den());
        enc.put_i128(a.to_m.num());
        enc.put_i128(a.to_m.den());
        enc.put_u64(a.assays.len() as u64);
        for (col, reading) in &a.assays {
            enc.put_str(col);
            match reading {
                AssayReading::Measured { value, unit, basis } => {
                    enc.put_u8(1);
                    enc.put_str(unit);
                    enc.put_u8(*basis as u8);
                    enc.put_i128(value.num());
                    enc.put_i128(value.den());
                }
                AssayReading::BelowDetection {
                    detection_limit,
                    unit,
                    basis,
                    raw,
                } => {
                    enc.put_u8(2);
                    enc.put_str(unit);
                    enc.put_u8(*basis as u8);
                    enc.put_str(raw);
                    enc.put_i128(detection_limit.num());
                    enc.put_i128(detection_limit.den());
                }
                AssayReading::Placeholder { raw } => {
                    enc.put_u8(3);
                    enc.put_str(raw);
                }
            }
        }
    }
    enc.put_u64(rejected_rows.len() as u64);
    for r in rejected_rows {
        enc.put_str(r.table.as_str());
        enc.put_u64(r.row_index as u64);
        enc.put_str(&r.column);
        enc.put_str(r.code);
    }
    blake3_hash(&enc.finish())
}

#[cfg(test)]
mod tests {
    use super::*;
    use fmine_registry::RegistryCatalog;
    use std::fs;
    use std::path::PathBuf;

    struct TempDirGuard(PathBuf);

    impl Drop for TempDirGuard {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn workspace_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|p| p.parent())
            .expect("workspace root")
            .to_path_buf()
    }

    #[test]
    fn test_malformed_grouping_cannot_change_exact_geometry_or_assays() {
        let expected = ExactQty::from_decimal_str("1234.5").expect("known decimal");
        assert_eq!(
            parse_exact_required(AcquireTableKind::Collar, 2, "EAST", "1,234.5")
                .expect("grouped coordinate"),
            expected
        );
        assert_eq!(
            parse_assay_reading(2, "AU_GPT", "<1,234.5", "g_per_t", ColumnBasis::Dry)
                .expect("grouped detection limit"),
            AssayReading::BelowDetection {
                detection_limit: expected,
                unit: "g_per_t".into(),
                basis: ColumnBasis::Dry,
                raw: "<1,234.5".into(),
            }
        );
        for raw in ["1,25", "1,,250", "12,34", "1,000.5,0"] {
            assert!(parse_exact_required(AcquireTableKind::Survey, 2, "DEPTH", raw).is_err());
            assert!(parse_assay_reading(2, "AU_GPT", raw, "g_per_t", ColumnBasis::Dry).is_err());
            assert!(
                parse_assay_reading(2, "AU_GPT", &format!("<{raw}"), "g_per_t", ColumnBasis::Dry,)
                    .is_err()
            );
        }
    }

    #[test]
    fn test_domain_rejection_scrubs_contact_text_without_losing_numeric_precision() {
        let rejected = AcquireRejection::from_refusal(
            AcquireTableKind::Survey,
            7,
            "HOLEID",
            "Contact: private@example.test",
            Refusal::new(
                RefusalCode::NoClaimInDomain201,
                "unknown hole private@example.test",
            ),
        );
        assert!(!rejected.raw_value.contains("private@example.test"));
        assert!(!rejected.explanation.contains("private@example.test"));
        assert_eq!(rejected.row_index, 7);
        assert_eq!(rejected.code, "FMINE-REF-201");
        let numeric = AcquireRejection::from_refusal(
            AcquireTableKind::Survey,
            8,
            "DEPTH",
            "4500000.125",
            Refusal::new(
                RefusalCode::OutsidePhysicalDomain109,
                "depth exceeds MAXDEPTH",
            ),
        );
        assert_eq!(numeric.raw_value, "4500000.125");
        let duplicate_header = AcquireError::DuplicateHeaderColumn {
            table: AcquireTableKind::Survey,
            column: "private@example.test".into(),
        }
        .to_refusal();
        assert!(!duplicate_header.what.contains("private@example.test"));
    }

    /// BD-188 done-when: sample multi-table acQuire export (collar + survey + assay) imported
    /// through CAS quarantine with exact rationals, censored `<0.005` preservation, PII redaction,
    /// Porgera `S > 100%` refusal, overlap/orphan/domain rejections, and bit-identical reruns.
    #[test]
    fn test_acquire_sample_export_bundle_import_and_domain_rejections() {
        let root = workspace_root();
        let catalog = RegistryCatalog::load_from_root(&root).expect("catalog");
        let reg_units: BTreeSet<String> = catalog.unit_symbols.iter().cloned().collect();
        let reg_crs: BTreeSet<String> = catalog.crs_codes.iter().cloned().collect();
        let dialects = IoDialectRegistry::load_from_root(&root).expect("io_dialects.toml");

        let guard = TempDirGuard(
            std::env::temp_dir().join(format!("fmine-io-acquire-bd188-{}", std::process::id())),
        );
        let _ = fs::remove_dir_all(&guard.0);
        let store = CasStore::open(&guard.0).expect("open cas");

        // Sample acQuire collar table:
        // - 2 valid collars (DDH-101, DDH-102) with GEOLOGIST PII redacted on ingest
        // - 1 duplicate collar (DDH-101) -> FMINE-REF-208
        // - 1 non-positive MAXDEPTH (DDH-103) -> FMINE-REF-109
        let collars_csv = b"HOLEID,PROJECTCODE,EAST,NORTH,RL,MAXDEPTH,GEOLOGIST\n\
DDH-101,NGM-FM,542100.25,4512300.50,1850.0,300.0,Jane Smith\n\
DDH-102,NGM-FM,542200.00,4512350.00,1855.5,250.0,Alex Doe\n\
DDH-101,NGM-FM,542100.25,4512300.50,1850.0,300.0,Duplicate Geo\n\
DDH-103,NGM-FM,542300.00,4512400.00,1860.0,0.0,Zero Depth\n";

        // Sample acQuire survey table:
        // - 3 valid stations
        // - 1 orphan hole (DDH-999) -> FMINE-REF-201
        // - 1 dip out of [-90, 90] (-95.0) -> FMINE-REF-109
        // - 1 azimuth out of [0, 360) (360.0) -> FMINE-REF-109
        // - 1 depth > MAXDEPTH (305.0 > 300.0) -> FMINE-REF-109
        // - 1 duplicate station (DDH-101 @ 0.0) -> FMINE-REF-208
        let surveys_csv = b"HOLEID,DEPTH,AZIMUTH,DIP,SURVEYTYPE\n\
DDH-101,0.0,045.5,-60.0,GYRO\n\
DDH-101,150.0,047.0,-58.5,GYRO\n\
DDH-102,0.0,180.0,-90.0,COLLAR\n\
DDH-999,0.0,090.0,-60.0,GYRO\n\
DDH-101,200.0,048.0,-95.0,GYRO\n\
DDH-101,210.0,360.0,-55.0,GYRO\n\
DDH-101,305.0,048.0,-55.0,GYRO\n\
DDH-101,0.0,045.5,-60.0,GYRO\n";

        // Sample acQuire assay table:
        // - row 2: valid measured Au/Cu/Ag/S
        // - row 3: valid left-censored `<0.005` Au + `NSS` placeholder Ag (NEVER coerced to 0!)
        // - row 4: valid DDH-102 interval
        // - row 5: Porgera-class S_PCT = 104.5 > 100% -> FMINE-REF-109
        // - row 6: overlapping interval [1.0, 2.5] on DDH-101 -> FMINE-REF-102
        // - row 7: reversed FROM >= TO [4.0, 4.0] -> FMINE-REF-109
        // - row 8: negative Au grade (-0.5) -> FMINE-REF-109 (caught in quarantine)
        // - row 9: orphan collar DDH-999 -> FMINE-REF-201
        let assays_csv =
            b"HOLEID,SAMPLEID,FROM,TO,AU_GPT,CU_PCT,AG_GPT,S_PCT,LABJOBNO,METHOD,LAB_CONTACT\n\
DDH-101,S0001,0.0,1.5,3.45,0.82,12.5,2.40,ALS-2026-01,Au-AA25,lab@als.example\n\
DDH-101,S0002,1.5,3.0,<0.005,0.15,NSS,0.80,ALS-2026-01,Au-AA25,lab@als.example\n\
DDH-102,S0003,0.0,2.0,1.10,0.40,<0.5,1.25,ALS-2026-01,Au-AA25,lab@als.example\n\
DDH-101,S0004,3.0,4.5,0.90,0.20,1.0,104.5,ALS-2026-01,Au-AA25,lab@als.example\n\
DDH-101,S0005,1.0,2.5,2.00,0.50,3.0,1.50,ALS-2026-01,Au-AA25,lab@als.example\n\
DDH-101,S0006,4.0,4.0,0.50,0.10,1.0,0.50,ALS-2026-01,Au-AA25,lab@als.example\n\
DDH-101,S0007,4.5,6.0,-0.50,0.10,1.0,0.50,ALS-2026-01,Au-AA25,lab@als.example\n\
DDH-999,S0008,0.0,1.5,5.00,1.00,4.0,1.00,ALS-2026-01,Au-AA25,lab@als.example\n";

        let input = AcquireBundleInput {
            collars: AcquireTableInput::Csv(collars_csv),
            surveys: AcquireTableInput::Csv(surveys_csv),
            assays: AcquireTableInput::Csv(assays_csv),
            site_crs: Some("EPSG:32611"),
        };

        let bundle =
            import_acquire_export_bundle(&store, &dialects, &input, &reg_units, &reg_crs, None)
                .expect("import acQuire bundle");

        assert_eq!(bundle.crs, "EPSG:32611");
        assert_eq!(bundle.collars.len(), 2);
        assert_eq!(bundle.surveys.len(), 3);
        assert_eq!(bundle.assays.len(), 3);
        // 2 collar rejections + 5 survey rejections + 5 assay rejections = 12 total explained rejections!
        assert_eq!(
            bundle.rejected_rows.len(),
            12,
            "{:#?}",
            bundle.rejected_rows
        );

        // PII redaction verified on GEOLOGIST and LAB_CONTACT (zero plaintext personal data leaks!).
        assert!(
            bundle.collars.iter().all(|c| {
                c.geologist_redacted
                    .as_deref()
                    .is_some_and(|s| !s.contains("Jane Smith") && !s.contains("Alex Doe"))
            }),
            "geologist PII must be redacted on ingest"
        );
        assert!(bundle.collar_report.total_columns_redacted >= 2);
        assert!(bundle.assay_report.total_columns_redacted >= 3);

        // Left-censored `<0.005` and `NSS` placeholder preserved exactly (never coerced to 0 or DL/2!).
        let censored_iv = bundle
            .assays
            .iter()
            .find(|a| a.sample_id.as_deref() == Some("S0002"))
            .expect("S0002");
        assert_eq!(
            censored_iv.assays.get("AU_GPT"),
            Some(&AssayReading::BelowDetection {
                detection_limit: ExactQty::from_decimal_str("0.005").expect("0.005"),
                unit: "g_per_t".to_string(),
                basis: ColumnBasis::Dry,
                raw: "<0.005".to_string(),
            })
        );
        assert_eq!(
            censored_iv.assays.get("AG_GPT"),
            Some(&AssayReading::Placeholder {
                raw: "NSS".to_string(),
            })
        );

        // Verify exact refusal codes on key mining failure modes (Porgera S>100%, overlap, orphan, dip).
        assert!(bundle.rejected_rows.iter().any(|r| {
            r.table == AcquireTableKind::Assay && r.column == "S_PCT" && r.code == "FMINE-REF-109"
        }));
        assert!(bundle.rejected_rows.iter().any(|r| {
            r.table == AcquireTableKind::Assay && r.column == "FROM,TO" && r.code == "FMINE-REF-102"
        }));
        assert!(bundle.rejected_rows.iter().any(|r| {
            r.table == AcquireTableKind::Survey && r.column == "DIP" && r.code == "FMINE-REF-109"
        }));
        assert!(bundle.rejected_rows.iter().any(|r| {
            r.table == AcquireTableKind::Survey && r.column == "HOLEID" && r.code == "FMINE-REF-201"
        }));

        // All three raw CSV sources preserved bit-for-bit in CAS.
        assert_eq!(
            store
                .read_object(&bundle.collar_report.source_cas_root)
                .expect("cas collars"),
            collars_csv
        );
        assert_eq!(
            store
                .read_object(&bundle.survey_report.source_cas_root)
                .expect("cas surveys"),
            surveys_csv
        );
        assert_eq!(
            store
                .read_object(&bundle.assay_report.source_cas_root)
                .expect("cas assays"),
            assays_csv
        );

        // Determinism: re-running produces bit-identical bundle_digest.
        let rerun =
            import_acquire_export_bundle(&store, &dialects, &input, &reg_units, &reg_crs, None)
                .expect("rerun");
        assert_eq!(rerun.bundle_digest, bundle.bundle_digest);
        assert_eq!(rerun, bundle);

        // Unregistered site CRS refuses before admitting any collar (`FMINE-REF-106`).
        let bad_crs_input = AcquireBundleInput {
            collars: AcquireTableInput::Csv(collars_csv),
            surveys: AcquireTableInput::Csv(surveys_csv),
            assays: AcquireTableInput::Csv(assays_csv),
            site_crs: Some("EPSG:999999"),
        };
        let err = import_acquire_export_bundle(
            &store,
            &dialects,
            &bad_crs_input,
            &reg_units,
            &reg_crs,
            None,
        )
        .expect_err("unregistered CRS must refuse");
        assert_eq!(err.to_refusal().code, "FMINE-REF-106");
    }

    #[test]
    fn test_acquire_xlsx_and_csv_export_parity() {
        use crate::xlsx::{MinimalCell, MinimalSheet, build_minimal_xlsx};

        let root = workspace_root();
        let catalog = RegistryCatalog::load_from_root(&root).expect("catalog");
        let reg_units: BTreeSet<String> = catalog.unit_symbols.iter().cloned().collect();
        let reg_crs: BTreeSet<String> = catalog.crs_codes.iter().cloned().collect();
        let dialects = IoDialectRegistry::load_from_root(&root).expect("io_dialects.toml");

        let guard = TempDirGuard(
            std::env::temp_dir().join(format!("fmine-io-acquire-xlsx-{}", std::process::id())),
        );
        let _ = fs::remove_dir_all(&guard.0);
        let store = CasStore::open(&guard.0).expect("open cas");

        let collars_csv = b"HOLEID,EAST,NORTH,RL,MAXDEPTH\nDDH-01,500000,4500000,1500,100\n";
        let surveys_csv = b"HOLEID,DEPTH,AZIMUTH,DIP\nDDH-01,0,90,-60\n";
        let assays_csv = b"HOLEID,FROM,TO,AU_GPT,CU_PCT\nDDH-01,0,2,<0.005,0.45\n";

        let xlsx = build_minimal_xlsx(&[
            MinimalSheet {
                name: "Collars".into(),
                hidden: false,
                cells: vec![
                    ("A1".into(), MinimalCell::Text("HOLEID".into())),
                    ("B1".into(), MinimalCell::Text("EAST".into())),
                    ("C1".into(), MinimalCell::Text("NORTH".into())),
                    ("D1".into(), MinimalCell::Text("RL".into())),
                    ("E1".into(), MinimalCell::Text("MAXDEPTH".into())),
                    ("A2".into(), MinimalCell::Text("DDH-01".into())),
                    ("B2".into(), MinimalCell::Number(500_000.0)),
                    ("C2".into(), MinimalCell::Number(4_500_000.0)),
                    ("D2".into(), MinimalCell::Number(1_500.0)),
                    ("E2".into(), MinimalCell::Number(100.0)),
                ],
            },
            MinimalSheet {
                name: "Surveys".into(),
                hidden: false,
                cells: vec![
                    ("A1".into(), MinimalCell::Text("HOLEID".into())),
                    ("B1".into(), MinimalCell::Text("DEPTH".into())),
                    ("C1".into(), MinimalCell::Text("AZIMUTH".into())),
                    ("D1".into(), MinimalCell::Text("DIP".into())),
                    ("A2".into(), MinimalCell::Text("DDH-01".into())),
                    ("B2".into(), MinimalCell::Number(0.0)),
                    ("C2".into(), MinimalCell::Number(90.0)),
                    ("D2".into(), MinimalCell::Number(-60.0)),
                ],
            },
            MinimalSheet {
                name: "Assays".into(),
                hidden: false,
                cells: vec![
                    ("A1".into(), MinimalCell::Text("HOLEID".into())),
                    ("B1".into(), MinimalCell::Text("FROM".into())),
                    ("C1".into(), MinimalCell::Text("TO".into())),
                    ("D1".into(), MinimalCell::Text("AU_GPT".into())),
                    ("E1".into(), MinimalCell::Text("CU_PCT".into())),
                    ("A2".into(), MinimalCell::Text("DDH-01".into())),
                    ("B2".into(), MinimalCell::Number(0.0)),
                    ("C2".into(), MinimalCell::Number(2.0)),
                    ("D2".into(), MinimalCell::Text("<0.005".into())),
                    ("E2".into(), MinimalCell::Number(0.45)),
                ],
            },
        ]);

        let csv_bundle = import_acquire_export_bundle(
            &store,
            &dialects,
            &AcquireBundleInput {
                collars: AcquireTableInput::Csv(collars_csv),
                surveys: AcquireTableInput::Csv(surveys_csv),
                assays: AcquireTableInput::Csv(assays_csv),
                site_crs: None,
            },
            &reg_units,
            &reg_crs,
            None,
        )
        .expect("csv bundle");

        let xlsx_bundle = import_acquire_export_bundle(
            &store,
            &dialects,
            &AcquireBundleInput {
                collars: AcquireTableInput::Xlsx {
                    bytes: &xlsx,
                    sheet: Some("Collars"),
                },
                surveys: AcquireTableInput::Xlsx {
                    bytes: &xlsx,
                    sheet: Some("Surveys"),
                },
                assays: AcquireTableInput::Xlsx {
                    bytes: &xlsx,
                    sheet: Some("Assays"),
                },
                site_crs: None,
            },
            &reg_units,
            &reg_crs,
            None,
        )
        .expect("xlsx bundle");

        assert_eq!(xlsx_bundle.crs, csv_bundle.crs);
        assert_eq!(xlsx_bundle.collars.len(), 1);
        assert_eq!(xlsx_bundle.surveys.len(), 1);
        assert_eq!(xlsx_bundle.assays.len(), 1);
        assert_eq!(xlsx_bundle.collars, csv_bundle.collars);
        assert_eq!(xlsx_bundle.surveys, csv_bundle.surveys);
        assert_eq!(xlsx_bundle.assays, csv_bundle.assays);
        assert!(
            xlsx_bundle.rejected_rows.is_empty(),
            "unexpected xlsx rejections: {:#?}",
            xlsx_bundle.rejected_rows
        );
        assert!(
            csv_bundle.rejected_rows.is_empty(),
            "unexpected csv rejections: {:#?}",
            csv_bundle.rejected_rows
        );
    }

    #[test]
    fn test_acquire_exact_cmp_overflow_free_and_header_guards() {
        // Huge denominators where i128 cross-multiplication overflows must still compare accurately!
        let d_big = i128::MAX / 3;
        let a = ExactQty::new(d_big - 1, d_big).expect("a");
        let b = ExactQty::new(d_big - 2, d_big - 1).expect("b");
        // (d-1)/d = 1 - 1/d > 1 - 1/(d-1) = (d-2)/(d-1)
        assert!(a.checked_cmp(b).is_err(), "cross-multiply should overflow");
        assert_eq!(exact_cmp(a, b), Ordering::Greater);
        assert_eq!(exact_cmp(b, a), Ordering::Less);
        assert_eq!(exact_cmp(a, a), Ordering::Equal);

        // Missing required column and duplicate header columns refuse at bundle level.
        let root = workspace_root();
        let catalog = RegistryCatalog::load_from_root(&root).expect("catalog");
        let reg_units: BTreeSet<String> = catalog.unit_symbols.iter().cloned().collect();
        let reg_crs: BTreeSet<String> = catalog.crs_codes.iter().cloned().collect();
        let dialects = IoDialectRegistry::load_from_root(&root).expect("dialects");
        let guard = TempDirGuard(
            std::env::temp_dir().join(format!("fmine-io-acquire-guards-{}", std::process::id())),
        );
        let _ = fs::remove_dir_all(&guard.0);
        let store = CasStore::open(&guard.0).expect("open cas");

        let missing_rl = b"HOLEID,EAST,NORTH,MAXDEPTH\nDDH-01,500000,4500000,100\n";
        let dup_hdr = b"HOLEID,EAST,NORTH,RL,MAXDEPTH,RL\nDDH-01,500000,4500000,1500,100,1500\n";
        let surveys = b"HOLEID,DEPTH,AZIMUTH,DIP\nDDH-01,0,90,-60\n";
        let assays = b"HOLEID,FROM,TO,AU_GPT\nDDH-01,0,1,1.0\n";

        let err_missing = import_acquire_export_bundle(
            &store,
            &dialects,
            &AcquireBundleInput {
                collars: AcquireTableInput::Csv(missing_rl),
                surveys: AcquireTableInput::Csv(surveys),
                assays: AcquireTableInput::Csv(assays),
                site_crs: None,
            },
            &reg_units,
            &reg_crs,
            None,
        )
        .expect_err("missing RL must refuse");
        assert!(matches!(
            err_missing,
            AcquireError::MissingRequiredColumn {
                table: AcquireTableKind::Collar,
                column: "RL"
            }
        ));

        let err_dup = import_acquire_export_bundle(
            &store,
            &dialects,
            &AcquireBundleInput {
                collars: AcquireTableInput::Csv(dup_hdr),
                surveys: AcquireTableInput::Csv(surveys),
                assays: AcquireTableInput::Csv(assays),
                site_crs: None,
            },
            &reg_units,
            &reg_crs,
            None,
        )
        .expect_err("duplicate header must refuse");
        assert!(matches!(
            err_dup,
            AcquireError::DuplicateHeaderColumn {
                table: AcquireTableKind::Collar,
                ..
            }
        ));
    }
}
