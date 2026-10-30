//! Label-anchored reader for the issuer's AISC reconciliation tables and the identity ledger over them
//! (`GATE-BULL-01` part 2, plan §21.6).
//!
//! The declared construction is the component sum (DESIGN, stated before any run):
//! - total cash costs (TCC) = Σ block-1 lines (the cost-of-sales row down to the row before TCC);
//! - AISC = TCC + Σ block-2 lines (the rows between TCC and AISC);
//! - each per-ounce rate r = 1000·N/Q, checked on intervals, because every printed value is rounded to a
//!   whole number: N ± k/2 (k = numeric lines summed into N), Q ± 1/2, r ± 1/2.
//!
//! All arithmetic is exact (integers, doubled integers and `i128` cross-multiplication). The alternative
//! construction (subtotal rounded once: N ± 1/2) is computed and reported, never scored.
//!
//! Every instance lands in exactly one state: (i) consistent, (ii) explained by a registered
//! reconstruction, (iii) issuer discrepancy with an exact certificate, (iv) undecidable from public data,
//! (v) refused, or not published (excluded by name).

use std::cmp::Ordering;
use std::collections::BTreeMap;

use fmine_io::xlsx::{CellValue, Sheet, col_index_to_letters};

/// The five identity families of one subject column.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum IdentityFamily {
    TccFooting,
    AiscFooting,
    CosPerOunce,
    TccPerOunce,
    AiscPerOunce,
}

impl IdentityFamily {
    pub const ALL: [Self; 5] = [
        Self::TccFooting,
        Self::AiscFooting,
        Self::CosPerOunce,
        Self::TccPerOunce,
        Self::AiscPerOunce,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::TccFooting => "tcc_footing",
            Self::AiscFooting => "aisc_footing",
            Self::CosPerOunce => "cos_per_oz",
            Self::TccPerOunce => "tcc_per_oz",
            Self::AiscPerOunce => "aisc_per_oz",
        }
    }

    /// Ownership basis of the quantities the identity relates, as the table prints them.
    #[must_use]
    pub const fn basis(self) -> &'static str {
        match self {
            Self::TccFooting => {
                "US$M; cost of sales on a 100% consolidated basis plus adjustments, including the \
                 non-controlling-interest line, giving attributable total cash costs"
            }
            Self::AiscFooting => "US$M; attributable total cash costs plus sustaining lines",
            Self::CosPerOunce => {
                "cost of sales US$M (100% consolidated) over attributable ounces sold (thousands); \
                 refused when a non-zero non-controlling-interest line shows the two bases differ"
            }
            Self::TccPerOunce => {
                "attributable total cash costs US$M over attributable ounces sold (thousands)"
            }
            Self::AiscPerOunce => {
                "attributable AISC US$M over attributable ounces sold (thousands)"
            }
        }
    }
}

/// Exact certificate for an issuer discrepancy (state (iii)).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FootingCertificate {
    pub printed: i64,
    pub component_sum: i64,
    /// `printed − component_sum`.
    pub residual: i64,
    pub total_cell: String,
    pub line_cells: Vec<String>,
    /// A single line whose sign flip closes the footing exactly, if one exists.
    pub sign_flip_witness: Option<String>,
    /// Whether the alternative construction (subtotal rounded once) would accept the printed total.
    pub alternate_consistent: bool,
}

/// The state of one identity instance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IdentityState {
    /// (i) The printed numbers satisfy the declared construction.
    Consistent { alternate_consistent: bool },
    /// (ii) The numbers close only with a registered reconstruction, re-verified on this run.
    Explained {
        evidence_id: &'static str,
        rule: &'static str,
        component: String,
    },
    /// (iii) The printed total differs from the sum of its printed lines.
    IssuerDiscrepancy(FootingCertificate),
    /// (iv) The printed rate lies outside every value the printed inputs allow, and no registered
    /// reconstruction explains it.
    Undecidable { detail: String },
    /// (v) The instance cannot be scored from what is printed.
    Refused { reason: &'static str },
    /// The issuer published nothing for this instance (excluded from the ledger by name).
    NotPublished { reason: &'static str },
}

impl IdentityState {
    /// Short state code: `i`, `ii`, `iii`, `iv`, `v` or `np`.
    #[must_use]
    pub const fn code(&self) -> &'static str {
        match self {
            Self::Consistent { .. } => "i",
            Self::Explained { .. } => "ii",
            Self::IssuerDiscrepancy(_) => "iii",
            Self::Undecidable { .. } => "iv",
            Self::Refused { .. } => "v",
            Self::NotPublished { .. } => "np",
        }
    }

    #[must_use]
    pub const fn label(&self) -> &'static str {
        match self {
            Self::Consistent { .. } => "consistent",
            Self::Explained { .. } => "explained_reconstructed",
            Self::IssuerDiscrepancy(_) => "issuer_discrepancy",
            Self::Undecidable { .. } => "undecidable_from_public_data",
            Self::Refused { .. } => "refused",
            Self::NotPublished { .. } => "not_published",
        }
    }

    /// One-line detail used in the report and bound into the state root.
    #[must_use]
    pub fn detail(&self) -> String {
        match self {
            Self::Consistent {
                alternate_consistent,
            } => format!("alternate_construction_consistent={alternate_consistent}"),
            Self::Explained {
                evidence_id,
                component,
                ..
            } => format!("{evidence_id}: {component}"),
            Self::IssuerDiscrepancy(c) => format!(
                "printed {} vs component sum {} (residual {}){}",
                c.printed,
                c.component_sum,
                c.residual,
                c.sign_flip_witness
                    .as_ref()
                    .map(|w| format!("; sign-flip witness {w}"))
                    .unwrap_or_default()
            ),
            Self::Undecidable { detail } => detail.clone(),
            Self::Refused { reason } | Self::NotPublished { reason } => (*reason).to_string(),
        }
    }
}

/// One identity instance: a family checked in one subject column of one table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdentityInstance {
    pub file: String,
    pub sheet: String,
    pub column: String,
    pub subject: String,
    pub family: IdentityFamily,
    /// The printed total or rate cell the instance is about.
    pub cell: String,
    pub state: IdentityState,
}

/// A table header that names no registered subject (the column is refused as a whole).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnregisteredHeader {
    pub file: String,
    pub sheet: String,
    pub cell: String,
    pub header: String,
}

/// Located rows of one AISC reconciliation table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiscTable {
    pub cos_row: u32,
    pub tcc_row: u32,
    pub aisc_row: u32,
    pub ounces_row: u32,
    pub cos_rate_row: u32,
    pub tcc_rate_row: u32,
    pub aisc_rate_row: u32,
    pub header_row: u32,
    /// Labelled rows from the cost-of-sales row up to the row before TCC.
    pub block1: Vec<u32>,
    /// Labelled rows strictly between TCC and AISC.
    pub block2: Vec<u32>,
    /// Rows of `block1 ∪ block2` labelled "Non-controlling interests".
    pub nci_rows: Vec<u32>,
}

/// Lower-case, whitespace-collapsed text of a cell (numbers in `f64` `Display` form).
#[must_use]
pub fn cell_text(sheet: &Sheet, row: u32, col: u32) -> Option<String> {
    let cell = sheet.cell(row, col)?;
    let raw = match &cell.value {
        CellValue::Number(v) => format!("{v}"),
        CellValue::Placeholder(s) | CellValue::FormulaError(s) | CellValue::Text(s) => s.clone(),
    };
    Some(raw.split_whitespace().collect::<Vec<_>>().join(" "))
}

/// Matches a header against the registered subjects: exact, or the subject followed by one lower-case
/// footnote letter.
#[must_use]
pub fn match_subject(header: &str, subjects: &[&'static str]) -> Option<&'static str> {
    subjects.iter().copied().find(|s| {
        header == *s
            || header.strip_prefix(*s).is_some_and(|rest| {
                rest.chars().count() == 1 && rest.chars().all(char::is_lowercase)
            })
    })
}

const COS_LABEL: &str = "cost of sales applicable to gold production";

/// Locates the AISC table on a sheet. `Ok(None)` when the sheet has no cost-of-sales row (not an AISC
/// table); `Err` when it has one but the remaining structure cannot be found (a table-level refusal).
pub fn locate_table(sheet: &Sheet, subjects: &[&'static str]) -> Result<Option<AiscTable>, String> {
    let mut labels: BTreeMap<u32, String> = BTreeMap::new();
    for &(row, col) in sheet.cells_by_coord.keys() {
        if col == 1 {
            if let Some(t) = cell_text(sheet, row, 1) {
                labels.insert(row, t.to_lowercase());
            }
        }
    }
    let first = |names: &[&str]| -> Option<u32> {
        labels
            .iter()
            .find(|(_, l)| names.contains(&l.as_str()))
            .map(|(r, _)| *r)
    };
    let Some(cos_row) = first(&[COS_LABEL]) else {
        return Ok(None);
    };
    let need = |row: Option<u32>, what: &str| row.ok_or_else(|| format!("no '{what}' row"));
    let tcc_row = need(first(&["total cash costs"]), "Total cash costs")?;
    let aisc_row = need(
        first(&["all-in sustaining costs"]),
        "All-in sustaining costs",
    )?;
    let ounces_row = need(
        labels
            .iter()
            .find(|(_, l)| l.starts_with("ounces sold"))
            .map(|(r, _)| *r),
        "Ounces sold",
    )?;
    let cos_rate_row = need(
        first(&["cost of sales per ounce", "cos/oz"]),
        "Cost of sales per ounce",
    )?;
    let tcc_rate_row = need(
        first(&["total cash costs per ounce", "tcc/oz"]),
        "Total cash costs per ounce",
    )?;
    let aisc_rate_row = need(
        first(&["all-in sustaining costs per ounce", "aisc/oz"]),
        "All-in sustaining costs per ounce",
    )?;
    if !(cos_row < tcc_row && tcc_row < aisc_row) {
        return Err("cost of sales, total cash costs and AISC rows are out of order".to_string());
    }
    let block1: Vec<u32> = labels.range(cos_row..tcc_row).map(|(r, _)| *r).collect();
    let block2: Vec<u32> = labels
        .range(tcc_row + 1..aisc_row)
        .map(|(r, _)| *r)
        .collect();
    let nci_rows = block1
        .iter()
        .chain(&block2)
        .copied()
        .filter(|r| {
            labels
                .get(r)
                .is_some_and(|l| l == "non-controlling interests")
        })
        .collect();

    let mut header_row = None;
    let mut best = 0_usize;
    for row in 1..cos_row {
        let n = sheet
            .row_cells(row)
            .iter()
            .filter(|c| c.col >= 2)
            .filter(|c| match &c.value {
                CellValue::Text(t) => {
                    let h = t.split_whitespace().collect::<Vec<_>>().join(" ");
                    match_subject(&h, subjects).is_some()
                }
                _ => false,
            })
            .count();
        if n > best {
            best = n;
            header_row = Some(row);
        }
    }
    let header_row = need(header_row, "header with a registered subject")?;
    Ok(Some(AiscTable {
        cos_row,
        tcc_row,
        aisc_row,
        ounces_row,
        cos_rate_row,
        tcc_rate_row,
        aisc_rate_row,
        header_row,
        block1,
        block2,
        nci_rows,
    }))
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Val {
    Blank,
    Place,
    Num(f64),
    /// Text or a formula error: present but not a number.
    Other,
}

fn val(sheet: &Sheet, row: u32, col: u32) -> Val {
    match sheet.cell(row, col).map(|c| &c.value) {
        None => Val::Blank,
        Some(CellValue::Placeholder(_)) => Val::Place,
        Some(CellValue::Number(v)) => Val::Num(*v),
        Some(CellValue::Text(_) | CellValue::FormulaError(_)) => Val::Other,
    }
}

/// Whole number within the exactly representable range.
fn as_int(v: Val) -> Option<i64> {
    const LIMIT: f64 = 9_007_199_254_740_992.0;
    match v {
        Val::Num(x) if x.fract() == 0.0 && x.abs() < LIMIT => Some(x as i64),
        _ => None,
    }
}

fn int_or_place(v: Val) -> bool {
    matches!(v, Val::Place) || as_int(v).is_some()
}

fn line_value(v: Val) -> i64 {
    as_int(v).unwrap_or(0)
}

fn non_numeric(v: Val) -> bool {
    matches!(v, Val::Blank | Val::Other)
}

/// Compares the rationals `a_num / a_den` and `b_num / b_den` (denominators non-zero) exactly.
fn cmp_frac(a_num: i128, a_den: i128, b_num: i128, b_den: i128) -> Ordering {
    let ord = (a_num * b_den).cmp(&(b_num * a_den));
    if (a_den < 0) != (b_den < 0) {
        ord.reverse()
    } else {
        ord
    }
}

/// Whether some N ∈ [n − k/2, n + k/2] and Q ∈ [q − 1/2, q + 1/2] give 1000·N/Q ∈ [r − 1/2, r + 1/2].
/// 1000·N/Q is monotone along each edge of the box (Q never crosses zero for |q| ≥ 1), so its range is
/// spanned by the four corners. In doubled integers a corner is 1000·(2n ± k)/(2q ± 1).
#[must_use]
pub fn rate_interval_holds(n: i64, k: i64, q: i64, r: i64) -> bool {
    let (n2, k, q2, r2) = (
        2 * i128::from(n),
        i128::from(k),
        2 * i128::from(q),
        2 * i128::from(r),
    );
    let corners = [
        (1000 * (n2 - k), q2 - 1),
        (1000 * (n2 - k), q2 + 1),
        (1000 * (n2 + k), q2 - 1),
        (1000 * (n2 + k), q2 + 1),
    ];
    // Lower bound of the rate interval is (r2 − 1)/2, upper (r2 + 1)/2.
    let reaches_low = corners
        .iter()
        .any(|&(num, den)| cmp_frac(num, den, r2 - 1, 2) != Ordering::Less);
    let reaches_high = corners
        .iter()
        .any(|&(num, den)| cmp_frac(num, den, r2 + 1, 2) != Ordering::Greater);
    reaches_low && reaches_high
}

/// Exact range of 1000·N/Q over the box, as decimal text with three places (display only). The lower bound is
/// rounded down and the upper bound up, so the printed interval always contains the exact one.
fn rate_range_text(n: i64, k: i64, q: i64) -> String {
    let (n2, k, q2) = (2 * i128::from(n), i128::from(k), 2 * i128::from(q));
    let mut corners = [
        (1000 * (n2 - k), q2 - 1),
        (1000 * (n2 - k), q2 + 1),
        (1000 * (n2 + k), q2 - 1),
        (1000 * (n2 + k), q2 + 1),
    ];
    corners.sort_by(|a, b| cmp_frac(a.0, a.1, b.0, b.1));
    let fmt = |(num, den): (i128, i128), round_up: bool| {
        let (num, den) = if den < 0 { (-num, -den) } else { (num, den) };
        let milli = if round_up {
            -(-num * 1000).div_euclid(den)
        } else {
            (num * 1000).div_euclid(den)
        };
        let sign = if milli < 0 { "-" } else { "" };
        let m = milli.abs();
        format!("{sign}{}.{:03}", m / 1000, m % 1000)
    };
    format!("[{}, {}]", fmt(corners[0], false), fmt(corners[3], true))
}

/// Whether adding a component c (printed, so c ± 1/2) to the numerator closes the rate interval:
/// [n − k/2 + c − 1/2, n + k/2 + c + 1/2] ∩ [(r − ½)(q − ½)/1000, (r + ½)(q + ½)/1000] ≠ ∅ (q, r > 0).
/// Scaled by 4000 to integers.
#[must_use]
pub fn reconstruction_closes(n: i64, k: i64, q: i64, r: i64, c: i64) -> bool {
    let (n, k, q, r, c) = (
        i128::from(n),
        i128::from(k),
        i128::from(q),
        i128::from(r),
        i128::from(c),
    );
    if q <= 0 || r <= 0 {
        return false;
    }
    let have_lo = 2000 * (2 * n - k + 2 * c - 1);
    let have_hi = 2000 * (2 * n + k + 2 * c + 1);
    let need_lo = (2 * r - 1) * (2 * q - 1);
    let need_hi = (2 * r + 1) * (2 * q + 1);
    have_hi >= need_lo && have_lo <= need_hi
}

/// A per-ounce miss handed to the caller for a registered-reconstruction attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RateMiss {
    pub n: i64,
    pub k: i64,
    pub q: i64,
    pub r: i64,
}

/// Builds the identity ledger for one located table. `explain` is offered every per-ounce miss and may
/// return an explained state (it must itself re-verify the component); otherwise the miss is (iv).
pub fn ledger_for_table(
    file: &str,
    sheet: &Sheet,
    table: &AiscTable,
    subjects: &[&'static str],
    explain: &mut dyn FnMut(&str, IdentityFamily, RateMiss) -> Option<IdentityState>,
    instances: &mut Vec<IdentityInstance>,
    unregistered: &mut Vec<UnregisteredHeader>,
) {
    let header_cells: Vec<(u32, String, String)> = sheet
        .row_cells(table.header_row)
        .iter()
        .filter(|c| c.col >= 2)
        .filter_map(|c| {
            cell_text(sheet, table.header_row, c.col).map(|t| (c.col, t, c.cell_ref.clone()))
        })
        .collect();
    for (col, header, header_ref) in header_cells {
        if header.to_lowercase() == "footnote" {
            continue;
        }
        let Some(subject) = match_subject(&header, subjects) else {
            unregistered.push(UnregisteredHeader {
                file: file.to_string(),
                sheet: sheet.name.clone(),
                cell: header_ref,
                header,
            });
            continue;
        };
        let letters = col_index_to_letters(col);
        let mut push = |family: IdentityFamily, row: u32, state: IdentityState| {
            instances.push(IdentityInstance {
                file: file.to_string(),
                sheet: sheet.name.clone(),
                column: letters.clone(),
                subject: subject.to_string(),
                family,
                cell: format!("{letters}{row}"),
                state,
            });
        };
        column_ledger(sheet, table, col, subject, explain, &mut push);
    }
}

fn column_ledger(
    sheet: &Sheet,
    t: &AiscTable,
    col: u32,
    subject: &str,
    explain: &mut dyn FnMut(&str, IdentityFamily, RateMiss) -> Option<IdentityState>,
    push: &mut dyn FnMut(IdentityFamily, u32, IdentityState),
) {
    let v = |row: u32| val(sheet, row, col);
    let cell_ref = |row: u32| format!("{}{row}", col_index_to_letters(col));
    let rate_rows = [
        (IdentityFamily::CosPerOunce, t.cos_rate_row),
        (IdentityFamily::TccPerOunce, t.tcc_rate_row),
        (IdentityFamily::AiscPerOunce, t.aisc_rate_row),
    ];

    let mut used: Vec<u32> = t.block1.clone();
    used.push(t.tcc_row);
    used.extend(&t.block2);
    used.extend([t.aisc_row, t.ounces_row]);
    used.extend(rate_rows.iter().map(|(_, r)| *r));
    if used
        .iter()
        .all(|&r| matches!(v(r), Val::Place) || v(r) == Val::Num(0.0))
    {
        push(
            IdentityFamily::TccFooting,
            t.tcc_row,
            IdentityState::NotPublished {
                reason: "all-zero column",
            },
        );
        push(
            IdentityFamily::AiscFooting,
            t.aisc_row,
            IdentityState::NotPublished {
                reason: "all-zero column",
            },
        );
        for (family, row) in rate_rows {
            push(
                family,
                row,
                IdentityState::NotPublished {
                    reason: "all-zero column",
                },
            );
        }
        return;
    }

    // Footing: TCC = Σ block 1; AISC = TCC + Σ block 2.
    let mut tcc_and_block2 = vec![t.tcc_row];
    tcc_and_block2.extend(&t.block2);
    let all_lines: Vec<u32> = t.block1.iter().chain(&t.block2).copied().collect();
    for (family, total, lines) in [
        (IdentityFamily::TccFooting, t.tcc_row, &t.block1),
        (IdentityFamily::AiscFooting, t.aisc_row, &tcc_and_block2),
    ] {
        let state = footing_state(&v, &cell_ref, total, lines, family, &t.block1, &all_lines);
        push(family, total, state);
    }

    // Per ounce: r = 1000·N/Q on intervals.
    let q = v(t.ounces_row);
    for (family, rate_row) in rate_rows {
        let rv = v(rate_row);
        let (numerator_row, lines): (u32, Vec<u32>) = match family {
            IdentityFamily::CosPerOunce => (t.cos_row, vec![t.cos_row]),
            IdentityFamily::TccPerOunce => (t.tcc_row, t.block1.clone()),
            _ => (t.aisc_row, all_lines.clone()),
        };
        let state = if !matches!(rv, Val::Num(_)) {
            IdentityState::NotPublished {
                reason: "rate not published",
            }
        } else if !matches!(q, Val::Num(_)) {
            IdentityState::Refused {
                reason: "ounces not numeric",
            }
        } else if rv == Val::Num(0.0) && q == Val::Num(0.0) {
            IdentityState::NotPublished {
                reason: "zero rate and zero ounces",
            }
        } else if rv == Val::Num(0.0) || q == Val::Num(0.0) {
            IdentityState::Refused {
                reason: "one of rate and ounces is zero",
            }
        } else {
            let mut cells = vec![rate_row, t.ounces_row, numerator_row];
            cells.extend(&lines);
            if cells.iter().any(|&r| non_numeric(v(r))) {
                IdentityState::Refused {
                    reason: "numerator line not numeric",
                }
            } else if !cells.iter().all(|&r| int_or_place(v(r))) {
                IdentityState::Refused {
                    reason: "unrounded stored value; needs cell number format",
                }
            } else if as_int(v(numerator_row)).is_none() {
                IdentityState::Refused {
                    reason: "numerator total is a placeholder",
                }
            } else if family == IdentityFamily::CosPerOunce
                && t.nci_rows
                    .iter()
                    .any(|&r| matches!(v(r), Val::Num(x) if x != 0.0))
            {
                IdentityState::Refused {
                    reason: "attributable share needed",
                }
            } else {
                let k = lines
                    .iter()
                    .filter(|&&r| matches!(v(r), Val::Num(_)))
                    .count() as i64;
                let miss = RateMiss {
                    n: line_value(v(numerator_row)),
                    k,
                    q: line_value(q),
                    r: line_value(rv),
                };
                if rate_interval_holds(miss.n, miss.k, miss.q, miss.r) {
                    IdentityState::Consistent {
                        alternate_consistent: rate_interval_holds(miss.n, 1, miss.q, miss.r),
                    }
                } else if let Some(explained) = explain(subject, family, miss) {
                    explained
                } else {
                    IdentityState::Undecidable {
                        detail: format!(
                            "printed {} outside 1000·N/Q range {} for N = {} ± {}/2, Q = {} ± 1/2",
                            miss.r,
                            rate_range_text(miss.n, miss.k, miss.q),
                            miss.n,
                            miss.k,
                            miss.q
                        ),
                    }
                }
            }
        };
        push(family, rate_row, state);
    }
}

fn footing_state(
    v: &dyn Fn(u32) -> Val,
    cell_ref: &dyn Fn(u32) -> String,
    total: u32,
    lines: &[u32],
    family: IdentityFamily,
    block1: &[u32],
    all_lines: &[u32],
) -> IdentityState {
    if !matches!(v(total), Val::Num(_)) {
        return IdentityState::NotPublished {
            reason: "total not published",
        };
    }
    if lines.iter().any(|&r| non_numeric(v(r))) {
        return IdentityState::Refused {
            reason: "line not numeric",
        };
    }
    if !lines.iter().chain([&total]).all(|&r| int_or_place(v(r))) {
        return IdentityState::Refused {
            reason: "unrounded stored value; needs cell number format",
        };
    }
    let sum: i64 = lines.iter().map(|&r| line_value(v(r))).sum();
    let printed = line_value(v(total));
    let alt_lines = if family == IdentityFamily::TccFooting {
        block1
    } else {
        all_lines
    };
    let k_alt = alt_lines
        .iter()
        .filter(|&&r| matches!(v(r), Val::Num(_)))
        .count() as i64;
    let alternate_consistent = (printed - sum).abs() * 2 <= 1 + k_alt;
    if sum == printed {
        return IdentityState::Consistent {
            alternate_consistent,
        };
    }
    let sign_flip_witness = lines
        .iter()
        .find(|&&r| {
            let x = line_value(v(r));
            x != 0 && sum - 2 * x == printed
        })
        .map(|&r| cell_ref(r));
    IdentityState::IssuerDiscrepancy(FootingCertificate {
        printed,
        component_sum: sum,
        residual: printed - sum,
        total_cell: cell_ref(total),
        line_cells: lines.iter().map(|&r| cell_ref(r)).collect(),
        sign_flip_witness,
        alternate_consistent,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rate_interval_known_answers() {
        // 1000·79/68 = 1161.76…; with N ± 1/2 and Q ± 1/2 the range is [1145.985…, 1177.777…].
        assert!(rate_interval_holds(79, 1, 68, 1162));
        assert!(rate_interval_holds(79, 1, 68, 1146));
        assert!(!rate_interval_holds(79, 1, 68, 1141));
        // Outward rounding: 1177.777… prints as 1177.778 (it printed 1177.777 before 2026-10-29, below the bound).
        assert_eq!(rate_range_text(79, 1, 68), "[1145.985, 1177.778]");
        // Negative range: floor of −578.94… is −578.948; ceiling of −428.57… is −428.571.
        assert_eq!(rate_range_text(-5, 1, 10), "[-578.948, -428.571]");
        // No corner can equal a bound r ± ½ exactly: 2000·(2n ± k) is even, (2r ∓ 1)(2q ∓ 1) is odd.
        // Negative numerators: 1000·(−5 ± ½)/(10 ± ½) spans [−578.9…, −428.5…].
        assert!(rate_interval_holds(-5, 1, 10, -500));
        assert!(!rate_interval_holds(-5, 1, 10, 500));
        // k widens the numerator: 1000·(100 ± 2)/(100 ± ½) reaches 1025.125…, which meets [1024.5, 1025.5]
        // but not [1026.5, 1027.5].
        assert!(rate_interval_holds(100, 4, 100, 1025));
        assert!(!rate_interval_holds(100, 4, 100, 1027));
    }

    #[test]
    fn reconstruction_known_answers() {
        // N = 30 ± 1, Q = 20 ± ½, printed 2000 needs N ∈ [38.990…, 41.010…]: adding 10 ± ½ gives
        // [38.5, 41.5] and closes it; adding 5 ± ½ gives [33.5, 36.5] and does not.
        assert!(reconstruction_closes(30, 2, 20, 2000, 10));
        assert!(!reconstruction_closes(30, 2, 20, 2000, 5));
        assert!(!rate_interval_holds(30, 2, 20, 2000));
    }

    #[test]
    fn subject_matching_takes_one_footnote_letter_only() {
        let subjects = ["Kibali", "North Mara", "North America"];
        assert_eq!(match_subject("Kibali", &subjects), Some("Kibali"));
        assert_eq!(match_subject("Kibalia", &subjects), Some("Kibali"));
        assert_eq!(match_subject("KibaliA", &subjects), None);
        assert_eq!(match_subject("Kibaliab", &subjects), None);
        assert_eq!(match_subject("North Marab", &subjects), Some("North Mara"));
        assert_eq!(match_subject("Kibali (45%)", &subjects), None);
    }
}
