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

use fmine_io::xlsx::{CellValue, Sheet, col_index_to_letters, parse_a1_ref};

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
                "cost of sales US$M (100% consolidated) over attributable ounces sold (thousands); when a \
                 non-zero non-controlling-interest line shows the two bases differ, the issuer's footnote states \
                 both on an attributable basis, so the rate is checked at the registered share s: 1000·s·N/Q"
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
    /// (i) The printed numbers satisfy the declared construction. `basis` cites the stated share, footnote and
    /// label when the construction needed one (BD-331.S3).
    Consistent {
        alternate_consistent: bool,
        basis: Option<StatedShare>,
    },
    /// (ii) The numbers close only with a registered reconstruction, re-verified on this run. `basis` says
    /// whether an issuer note states the rule (BD-331.S2).
    Explained {
        evidence_id: String,
        rule: String,
        component: CitedCell,
        basis: ExplanationBasis,
    },
    /// (iii) The printed total differs from the sum of its printed lines.
    IssuerDiscrepancy(FootingCertificate),
    /// (iv) The printed rate lies outside every value the printed inputs allow, and no registered
    /// reconstruction explains it. `missing_fact` names what would decide it (prose, no digits), and
    /// `closing_document` a document known to hold that fact, if any (BD-331.S2).
    Undecidable {
        why: Undecided,
        missing_fact: String,
        closing_document: Option<String>,
    },
    /// (v) The instance cannot be scored from what is printed.
    Refused { reason: RefusedReason },
    /// (v) Refused because of a named cell or file (`at`).
    RefusedAt { reason: RefusedReason, at: String },
    /// The issuer published nothing for this instance (excluded from the ledger by name).
    NotPublished { reason: NotPublishedReason },
}

/// Why an instance is refused (v): a stable snake_case code and a reason with no digits (bd-sjrh.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefusedReason {
    FootnoteNotFound,
    RegionalNumerator,
    NoRegisteredShare,
    ShareNotConfirmed,
    ShareRuleNeedsPositive,
    OuncesNotNumeric,
    OneOfRateAndOuncesZero,
    NumeratorLineNotNumeric,
    NumeratorPlaceholder,
    LineNotNumeric,
    Unrounded,
}

impl RefusedReason {
    pub const ALL: [Self; 11] = [
        Self::FootnoteNotFound,
        Self::RegionalNumerator,
        Self::NoRegisteredShare,
        Self::ShareNotConfirmed,
        Self::ShareRuleNeedsPositive,
        Self::OuncesNotNumeric,
        Self::OneOfRateAndOuncesZero,
        Self::NumeratorLineNotNumeric,
        Self::NumeratorPlaceholder,
        Self::LineNotNumeric,
        Self::Unrounded,
    ];

    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::FootnoteNotFound => "attributable_footnote_not_found",
            Self::RegionalNumerator => "regional_numerator_needs_weighting",
            Self::NoRegisteredShare => "no_registered_share",
            Self::ShareNotConfirmed => "share_not_confirmed_by_label",
            Self::ShareRuleNeedsPositive => "share_rule_needs_positive_inputs",
            Self::OuncesNotNumeric => "ounces_not_numeric",
            Self::OneOfRateAndOuncesZero => "one_of_rate_and_ounces_zero",
            Self::NumeratorLineNotNumeric => "numerator_line_not_numeric",
            Self::NumeratorPlaceholder => "numerator_total_placeholder",
            Self::LineNotNumeric => "line_not_numeric",
            Self::Unrounded => "unrounded_stored_value",
        }
    }

    #[must_use]
    pub const fn why(self) -> &'static str {
        match self {
            Self::FootnoteNotFound => "attributable-basis footnote not found on the sheet",
            Self::RegionalNumerator => "regional numerator needs member weighting",
            Self::NoRegisteredShare => "no registered share",
            Self::ShareNotConfirmed => "registered share not confirmed by label",
            Self::ShareRuleNeedsPositive => {
                "stated share rule needs positive cost of sales, ounces and rate"
            }
            Self::OuncesNotNumeric => "ounces not numeric",
            Self::OneOfRateAndOuncesZero => "one of rate and ounces is zero",
            Self::NumeratorLineNotNumeric => "numerator line not numeric",
            Self::NumeratorPlaceholder => "numerator total is a placeholder",
            Self::LineNotNumeric => "line not numeric",
            Self::Unrounded => "unrounded stored value; needs cell number format",
        }
    }
}

/// Why an instance is not published: a stable snake_case code and a reason with no digits (bd-sjrh.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotPublishedReason {
    AllZeroColumn,
    RateNotPublished,
    ZeroRateAndOunces,
    TotalNotPublished,
}

impl NotPublishedReason {
    pub const ALL: [Self; 4] = [
        Self::AllZeroColumn,
        Self::RateNotPublished,
        Self::ZeroRateAndOunces,
        Self::TotalNotPublished,
    ];

    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::AllZeroColumn => "all_zero_column",
            Self::RateNotPublished => "rate_not_published",
            Self::ZeroRateAndOunces => "zero_rate_and_ounces",
            Self::TotalNotPublished => "total_not_published",
        }
    }

    #[must_use]
    pub const fn why(self) -> &'static str {
        match self {
            Self::AllZeroColumn => "all-zero column",
            Self::RateNotPublished => "rate not published",
            Self::ZeroRateAndOunces => "zero rate and zero ounces",
            Self::TotalNotPublished => "total not published",
        }
    }
}

/// A registered share the run confirmed against its Mine Stats label, at the sheet's attributable-basis
/// footnote (BD-331.S3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatedShare {
    pub num: i64,
    pub den: i64,
    pub evidence: String,
    /// `sheet!cell` of the footnote.
    pub footnote: String,
    pub label_cell: String,
}

impl StatedShare {
    #[must_use]
    pub fn text(&self) -> String {
        format!(
            "stated share {}/{} ({}); footnote {}; label {}",
            self.num, self.den, self.evidence, self.footnote, self.label_cell
        )
    }
}

/// A whole-number cell a (ii) explanation read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CitedCell {
    pub file: String,
    pub sheet: String,
    pub cell: String,
    pub value: i64,
}

impl CitedCell {
    #[must_use]
    pub fn text(&self) -> String {
        format!("{}:{}!{}={}", self.file, self.sheet, self.cell, self.value)
    }
}

/// What leaves a (iv) instance undecided.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Undecided {
    /// The printed rate lies outside 1000·N/Q for every N and Q the printed inputs allow.
    RateOutside(RateMiss),
    /// The printed rate lies outside 1000·s·N/Q at the stated share; the printed numbers allow s in
    /// `s_interval`.
    ShareOutside {
        miss: RateMiss,
        share: StatedShare,
        s_interval: String,
    },
    /// A registered reconstruction applies to the instance but failed `check` on this run.
    ReconstructionNotApplied {
        evidence_id: String,
        check: ReconstructionCheck,
    },
}

/// The check a registered reconstruction failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReconstructionCheck {
    /// The stated quote at `at` (`doc:sheet!cell`) no longer starts with `expected_prefix`.
    QuoteMismatch {
        at: String,
        found: String,
        expected_prefix: String,
    },
    ComponentFileNotLoaded {
        file: String,
    },
    NoSheet {
        sheet: String,
    },
    /// The label at `at` (`sheet!cell`) does not start with `expected_prefix`.
    LabelMismatch {
        at: String,
        found: String,
        expected_prefix: String,
    },
    /// The component at `at` (`sheet!cell`) is not a whole number.
    NotWhole {
        at: String,
    },
    DoesNotClose {
        component: CitedCell,
    },
}

impl ReconstructionCheck {
    #[must_use]
    pub const fn code(&self) -> &'static str {
        match self {
            Self::QuoteMismatch { .. } => "quote_mismatch",
            Self::ComponentFileNotLoaded { .. } => "component_file_not_loaded",
            Self::NoSheet { .. } => "no_sheet",
            Self::LabelMismatch { .. } => "label_mismatch",
            Self::NotWhole { .. } => "component_not_whole",
            Self::DoesNotClose { .. } => "does_not_close",
        }
    }

    fn text(&self) -> String {
        match self {
            Self::QuoteMismatch {
                at,
                found,
                expected_prefix,
            } => format!(
                "stated quote {at} reads {found:?}, expected text starting {expected_prefix:?}"
            ),
            Self::ComponentFileNotLoaded { file } => format!("{file} not loaded"),
            Self::NoSheet { sheet } => format!("no visible sheet '{sheet}'"),
            Self::LabelMismatch {
                at,
                found,
                expected_prefix,
            } => format!("{at} reads {found:?}, expected text starting {expected_prefix:?}"),
            Self::NotWhole { at } => format!("{at} is not a whole number"),
            Self::DoesNotClose { component } => {
                format!(
                    "adding {} does not close the rate interval",
                    component.text()
                )
            }
        }
    }
}

impl Undecided {
    #[must_use]
    pub const fn code(&self) -> &'static str {
        match self {
            Self::RateOutside(_) => "rate_outside_interval",
            Self::ShareOutside { .. } => "rate_outside_interval_at_stated_share",
            Self::ReconstructionNotApplied { .. } => "reconstruction_not_applied",
        }
    }

    #[must_use]
    pub fn text(&self) -> String {
        match self {
            Self::RateOutside(m) => format!(
                "printed {} outside 1000·N/Q range {} for N = {} ± {}/2, Q = {} ± 1/2",
                m.r,
                rate_range_text(m.n, m.k, m.q),
                m.n,
                m.k,
                m.q
            ),
            Self::ShareOutside {
                miss: m,
                share,
                s_interval,
            } => format!(
                "printed {} outside 1000·s·N/Q at {}, for N = {} ± {}/2, Q = {} ± 1/2; the printed numbers allow s \
                 in {s_interval}",
                m.r,
                share.text(),
                m.n,
                m.k,
                m.q
            ),
            Self::ReconstructionNotApplied { evidence_id, check } => {
                format!(
                    "registered reconstruction {evidence_id} not applied: {}",
                    check.text()
                )
            }
        }
    }
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
            Self::Refused { .. } | Self::RefusedAt { .. } => "v",
            Self::NotPublished { .. } => "np",
        }
    }

    #[must_use]
    pub const fn label(&self) -> &'static str {
        match self {
            Self::Consistent { .. } => "consistent",
            Self::Explained {
                basis: ExplanationBasis::Stated { .. },
                ..
            } => "explained_stated",
            Self::Explained {
                basis: ExplanationBasis::Reconstructed,
                ..
            } => "explained_reconstructed",
            Self::IssuerDiscrepancy(_) => "issuer_discrepancy",
            Self::Undecidable { .. } => "undecidable_from_public_data",
            Self::Refused { .. } | Self::RefusedAt { .. } => "refused",
            Self::NotPublished { .. } => "not_published",
        }
    }

    /// One-line detail used in the report and bound into the state root.
    #[must_use]
    pub fn detail(&self) -> String {
        match self {
            Self::Consistent {
                alternate_consistent,
                basis,
            } => match basis {
                None => format!("alternate_construction_consistent={alternate_consistent}"),
                Some(b) => format!(
                    "alternate_construction_consistent={alternate_consistent}; {}",
                    b.text()
                ),
            },
            Self::Explained {
                evidence_id,
                component,
                basis,
                ..
            } => match basis {
                ExplanationBasis::Reconstructed => format!("{evidence_id}: {}", component.text()),
                ExplanationBasis::Stated { doc, locator, .. } => {
                    format!(
                        "{evidence_id}: {}; stated at {doc}:{locator}",
                        component.text()
                    )
                }
            },
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
            Self::Undecidable {
                why,
                missing_fact,
                closing_document,
            } => match closing_document {
                None => format!("{}; missing fact: {missing_fact}", why.text()),
                Some(d) => format!(
                    "{}; missing fact: {missing_fact} (closed by {d})",
                    why.text()
                ),
            },
            Self::Refused { reason } => reason.why().to_string(),
            Self::NotPublished { reason } => reason.why().to_string(),
            Self::RefusedAt { reason, at } => format!("{}: {at}", reason.why()),
        }
    }

    /// The stable snake_case code of a (iv), (v) or not-published reason; `None` for the other states.
    #[must_use]
    pub const fn reason_code(&self) -> Option<&'static str> {
        match self {
            Self::Undecidable { why, .. } => Some(why.code()),
            Self::Refused { reason } | Self::RefusedAt { reason, .. } => Some(reason.code()),
            Self::NotPublished { reason } => Some(reason.code()),
            _ => None,
        }
    }
}

/// Whether an issuer note states a (ii) explanation's rule (BD-331.S2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExplanationBasis {
    /// The issuer states it: `doc` at `locator` (`sheet!cell`) reads text starting `quote_prefix`, re-read on
    /// every run.
    Stated {
        doc: String,
        locator: String,
        quote_prefix: String,
    },
    /// No issuer note states it; the registry reconstructs it from published cells.
    Reconstructed,
}

/// The missing fact a (iv) instance names when no `(subject, family)` row is registered.
pub const DEFAULT_MISSING_FACT: &str = "the numerator or denominator the issuer used for this rate";

/// The missing fact of a stated-share miss (BD-331.S3).
pub const SHARE_MISSING_FACT: &str = "the attributable cost of sales or ounces the issuer used";

/// What the run knows about one subject's ownership share, for the per-ounce rate the issuer states on an
/// attributable basis (BD-331.S3). A subject with no entry has no registered share.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShareBasis {
    /// A registered share `num/den` that the same-release Mine Stats label prints; `label_cell` names it.
    Confirmed {
        num: i64,
        den: i64,
        evidence: String,
        label_cell: String,
    },
    /// A registered share the label does not confirm; `why` names the cell, or the label or sheet not found.
    Unconfirmed { why: String },
    /// A region: its numerator rolls up member sites held at different shares.
    Region,
}

/// Locates, by its text, the footnote stating that cost of sales per ounce is cost of sales over ounces sold,
/// both on an attributable basis. Its letter and row differ between vintages, so neither is used. Returns the
/// first matching cell in row order.
#[must_use]
pub fn find_share_footnote(sheet: &Sheet) -> Option<String> {
    const PHRASES: [&str; 4] = [
        "is calculated as cost of sales",
        "divided by ounces sold",
        "attributable basis using",
        "ownership share",
    ];
    sheet.cells_by_coord.keys().find_map(|&(row, col)| {
        let text = cell_text(sheet, row, col)?.to_lowercase();
        PHRASES
            .iter()
            .all(|p| text.contains(p))
            .then(|| format!("{}{row}", col_index_to_letters(col)))
    })
}

const QUARTER_END_MONTHS: [&str; 4] = ["march", "june", "september", "december"];

/// Finds the quarter-to-date "Summary of Operations" sheet of a Mine Stats workbook for `quarter` (`YYYYQn`) by
/// content: A1 reads "Summary of Operations", and a column-A period line in rows 1–10 reads "For the three
/// months ended <last month of the quarter>" with the year as the first number on that row. Exactly one sheet
/// must match.
///
/// # Errors
/// Why no single sheet qualifies.
pub fn find_qtd_sheet<'a>(sheets: &'a [Sheet], quarter: &str) -> Result<&'a Sheet, String> {
    let (year, q) = quarter
        .split_once('Q')
        .and_then(|(y, q)| Some((y.parse::<f64>().ok()?, q.parse::<usize>().ok()?)))
        .filter(|&(_, q)| (1..=4).contains(&q))
        .ok_or_else(|| format!("quarter {quarter:?} is not YYYYQn"))?;
    let period = format!("for the three months ended {}", QUARTER_END_MONTHS[q - 1]);
    let qualifies = |s: &&Sheet| {
        cell_text(s, 1, 1).is_some_and(|t| t.to_lowercase() == "summary of operations")
            && (1..=10).any(|row| {
                cell_text(s, row, 1).is_some_and(|t| t.to_lowercase().starts_with(&period))
                    && s.row_cells(row)
                        .iter()
                        .filter(|c| c.col >= 2)
                        .find_map(|c| c.value.as_number())
                        == Some(year)
            })
    };
    let found: Vec<&Sheet> = sheets.iter().filter(qualifies).collect();
    match found.as_slice() {
        [one] => Ok(one),
        [] => Err(format!(
            "no sheet with A1 'Summary of Operations' and a column-A line '{period}' for {quarter}"
        )),
        many => Err(format!(
            "{} sheets qualify as the {quarter} quarter-to-date sheet",
            many.len()
        )),
    }
}

/// Confirms a registered share against the issuer's label: exactly one column-A cell of `sheet` must read
/// `<label> (<p>%)`, after stripping trailing footnote digits, with p/100 = num/den exactly. Returns that cell.
///
/// # Errors
/// The cell and the share it prints when they differ, or that the label is missing or repeated.
pub fn confirm_share_label(
    sheet: &Sheet,
    label: &str,
    num: i64,
    den: i64,
) -> Result<String, String> {
    let mut found: Vec<(String, String)> = Vec::new();
    for &(row, col) in sheet.cells_by_coord.keys() {
        if col != 1 {
            continue;
        }
        let Some(text) = cell_text(sheet, row, col) else {
            continue;
        };
        let pct = text
            .trim_end_matches(|c: char| c.is_ascii_digit())
            .strip_prefix(label)
            .map(str::trim_start)
            .and_then(|rest| rest.strip_prefix('('))
            .and_then(|rest| rest.strip_suffix("%)"));
        if let Some(pct) = pct {
            found.push((format!("A{row}"), pct.to_string()));
        }
    }
    let (cell, pct) = match found.as_slice() {
        [one] => one,
        [] => {
            return Err(format!(
                "no column-A label '{label} (x%)' on {}",
                sheet.name
            ));
        }
        many => {
            let cells: Vec<&str> = many.iter().map(|(c, _)| c.as_str()).collect();
            return Err(format!(
                "label '{label}' repeats on {} at {}",
                sheet.name,
                cells.join(", ")
            ));
        }
    };
    let printed = decimal_fraction(pct);
    let matches = printed.is_some_and(|(p_num, p_den)| {
        i128::from(p_num) * i128::from(den) == i128::from(num) * 100 * i128::from(p_den)
    });
    if matches {
        Ok(cell.clone())
    } else {
        Err(format!(
            "{}!{cell} prints {pct}%, registered {num}/{den}",
            sheet.name
        ))
    }
}

/// `"61.5"` → `(615, 10)`: a non-negative decimal with at most four places, as an exact fraction.
fn decimal_fraction(s: &str) -> Option<(i64, i64)> {
    let (whole, frac) = s.split_once('.').unwrap_or((s, ""));
    if whole.is_empty()
        || frac.len() > 4
        || !whole
            .bytes()
            .chain(frac.bytes())
            .all(|b| b.is_ascii_digit())
    {
        return None;
    }
    let den = 10_i64.pow(u32::try_from(frac.len()).ok()?);
    let num = format!("{whole}{frac}").parse::<i64>().ok()?;
    Some((num, den))
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
    /// Every cell the state was computed from (bd-sjrh.1, BD-329.S1), with its role and value as read. Not yet
    /// in the payload or the state root.
    pub operands: Vec<Operand>,
    /// The numbers the state was computed from, as the ledger read them (bd-sjrh.1).
    pub calc: Calc,
}

/// The numbers behind an instance's state, out of prose (bd-sjrh.1): what the live-dependency proof checks a
/// perturbed leaf against (BD-338.S2). Not in the payload or the state root; `detail()` is unchanged.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Calc {
    /// A footing: the printed total, the sum of its summands, and how many numeric lines its alternate rule
    /// counts.
    Footing {
        printed: i64,
        component_sum: i64,
        k_alt: i64,
    },
    /// A per-ounce rate: printed r, numerator n, counted lines k and ounces q.
    Rate(RateMiss),
    /// The state was decided before any arithmetic (refused or not published).
    None,
}

/// What an operand cell is to its instance.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperandRole {
    /// The printed total a footing checks.
    Total,
    /// A summand of a footing.
    Line,
    /// A line read only to count it (k): a rate's numerator lines, or block 1 under the AISC footing's alternate
    /// rule. Its value never enters a sum; whether it is a number does.
    Counted,
    /// The printed per-ounce rate.
    Rate,
    Ounces,
    /// The printed total a rate divides.
    Numerator,
    /// A non-controlling-interest line, which decides a cost-of-sales rate's basis.
    NciLine,
    /// A registered reconstruction's component cell.
    Component,
    /// A cell whose text a registered rule checks (label or stated quote).
    LabelCheck,
}

impl OperandRole {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Total => "total",
            Self::Line => "line",
            Self::Counted => "counted",
            Self::Rate => "rate",
            Self::Ounces => "ounces",
            Self::Numerator => "numerator",
            Self::NciLine => "nci_line",
            Self::Component => "component",
            Self::LabelCheck => "label_check",
        }
    }
}

/// An operand's value as read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OperandValue {
    Int(i64),
    /// A placeholder, text or formula error, as stored.
    Token(String),
    Blank,
    /// A number that is not a whole number, in `f64` display form.
    Unrounded(String),
}

/// One cell an instance was computed from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Operand {
    pub role: OperandRole,
    pub file: String,
    pub sheet: String,
    pub cell: String,
    pub value: OperandValue,
}

/// The operand at `(row, col)` of `sheet` in `file`.
#[must_use]
pub fn operand_at(file: &str, sheet: &Sheet, row: u32, col: u32, role: OperandRole) -> Operand {
    let value = match sheet.cell(row, col).map(|c| &c.value) {
        None => OperandValue::Blank,
        Some(CellValue::Number(x)) => as_int(Val::Num(*x))
            .map_or_else(|| OperandValue::Unrounded(x.to_string()), OperandValue::Int),
        Some(CellValue::Placeholder(t) | CellValue::Text(t) | CellValue::FormulaError(t)) => {
            OperandValue::Token(t.clone())
        }
    };
    Operand {
        role,
        file: file.to_string(),
        sheet: sheet.name.clone(),
        cell: format!("{}{row}", col_index_to_letters(col)),
        value,
    }
}

/// The table cells one instance of `family` in column `col` is computed from. An all-zero column's instances
/// are decided by every used cell of the column. The AISC footing's alternate rule counts block 1's lines too.
/// A cost-of-sales rate with a non-zero NCI line also reads the attributable-basis footnote (`footnote`, a cell
/// of this sheet); the registered share's label cell is in another workbook and is added by the caller.
fn family_operands(
    file: &str,
    sheet: &Sheet,
    t: &AiscTable,
    col: u32,
    family: IdentityFamily,
    all_zero: bool,
    footnote: Option<&str>,
) -> Vec<Operand> {
    use OperandRole as R;
    let at = |row: u32, role: OperandRole| operand_at(file, sheet, row, col, role);
    let lines = |rows: &[u32]| rows.iter().map(|&r| at(r, R::Line)).collect::<Vec<_>>();
    if all_zero {
        let mut out = lines(&t.block1);
        out.push(at(t.tcc_row, R::Total));
        out.extend(lines(&t.block2));
        out.push(at(t.aisc_row, R::Total));
        out.push(at(t.ounces_row, R::Ounces));
        out.extend([t.cos_rate_row, t.tcc_rate_row, t.aisc_rate_row].map(|r| at(r, R::Rate)));
        return out;
    }
    let mut out = match family {
        IdentityFamily::TccFooting => vec![at(t.tcc_row, R::Total)],
        IdentityFamily::AiscFooting => vec![at(t.aisc_row, R::Total), at(t.tcc_row, R::Line)],
        IdentityFamily::CosPerOunce => vec![
            at(t.cos_rate_row, R::Rate),
            at(t.ounces_row, R::Ounces),
            at(t.cos_row, R::Numerator),
        ],
        IdentityFamily::TccPerOunce => vec![
            at(t.tcc_rate_row, R::Rate),
            at(t.ounces_row, R::Ounces),
            at(t.tcc_row, R::Numerator),
        ],
        IdentityFamily::AiscPerOunce => vec![
            at(t.aisc_rate_row, R::Rate),
            at(t.ounces_row, R::Ounces),
            at(t.aisc_row, R::Numerator),
        ],
    };
    let counted = |rows: &[u32]| rows.iter().map(|&r| at(r, R::Counted)).collect::<Vec<_>>();
    match family {
        IdentityFamily::TccFooting => out.extend(lines(&t.block1)),
        IdentityFamily::AiscFooting => {
            out.extend(counted(&t.block1));
            out.extend(lines(&t.block2));
        }
        IdentityFamily::TccPerOunce => out.extend(counted(&t.block1)),
        IdentityFamily::AiscPerOunce => {
            out.extend(counted(&t.block1));
            out.extend(counted(&t.block2));
        }
        IdentityFamily::CosPerOunce => {
            out.extend(t.nci_rows.iter().map(|&r| at(r, R::NciLine)));
            let nci = t
                .nci_rows
                .iter()
                .any(|&r| matches!(val(sheet, r, col), Val::Num(x) if x != 0.0));
            if let Some((r, c)) = footnote.filter(|_| nci).and_then(parse_a1_ref) {
                out.push(operand_at(file, sheet, r, c, R::LabelCheck));
            }
        }
    }
    out
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
pub fn match_subject<'a, S: AsRef<str>>(header: &str, subjects: &'a [S]) -> Option<&'a str> {
    subjects.iter().map(AsRef::as_ref).find(|s| {
        header == *s
            || header.strip_prefix(*s).is_some_and(|rest| {
                rest.chars().count() == 1 && rest.chars().all(char::is_lowercase)
            })
    })
}

const COS_LABEL: &str = "cost of sales applicable to gold production";

/// How a [`RowAnchor`] name matches a column-A label (lower-case, whitespace-collapsed).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnchorRule {
    /// The label equals one of the names.
    Exact,
    /// The label starts with one of the names (the ounces row carries a unit suffix).
    Prefix,
}

/// One row `locate_table` anchors on: the first column-A label, in row order, that matches any of `names`.
/// `role` names the row in a table-level refusal ("no '{role}' row").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RowAnchor {
    pub role: &'static str,
    pub names: &'static [&'static str],
    pub rule: AnchorRule,
}

impl RowAnchor {
    /// Whether a lower-case, whitespace-collapsed label matches this anchor.
    #[must_use]
    pub fn matches(&self, label: &str) -> bool {
        match self.rule {
            AnchorRule::Exact => self.names.contains(&label),
            AnchorRule::Prefix => self.names.iter().any(|n| label.starts_with(n)),
        }
    }
}

/// The seven rows of an AISC table, in the order `locate_table` reads them (BD-339.S4). The first, cost of
/// sales, decides whether a sheet holds an AISC table at all. The reproduction kit's README prints this list.
pub const ROW_ANCHORS: [RowAnchor; 7] = [
    RowAnchor {
        role: "Cost of sales",
        names: &[COS_LABEL],
        rule: AnchorRule::Exact,
    },
    RowAnchor {
        role: "Total cash costs",
        names: &["total cash costs"],
        rule: AnchorRule::Exact,
    },
    RowAnchor {
        role: "All-in sustaining costs",
        names: &["all-in sustaining costs"],
        rule: AnchorRule::Exact,
    },
    RowAnchor {
        role: "Ounces sold",
        names: &["ounces sold"],
        rule: AnchorRule::Prefix,
    },
    RowAnchor {
        role: "Cost of sales per ounce",
        names: &["cost of sales per ounce", "cos/oz"],
        rule: AnchorRule::Exact,
    },
    RowAnchor {
        role: "Total cash costs per ounce",
        names: &["total cash costs per ounce", "tcc/oz"],
        rule: AnchorRule::Exact,
    },
    RowAnchor {
        role: "All-in sustaining costs per ounce",
        names: &["all-in sustaining costs per ounce", "aisc/oz"],
        rule: AnchorRule::Exact,
    },
];

/// Locates the AISC table on a sheet. `Ok(None)` when the sheet has no cost-of-sales row (not an AISC
/// table); `Err` when it has one but the remaining structure cannot be found (a table-level refusal).
pub fn locate_table<S: AsRef<str>>(
    sheet: &Sheet,
    subjects: &[S],
) -> Result<Option<AiscTable>, String> {
    let mut labels: BTreeMap<u32, String> = BTreeMap::new();
    for &(row, col) in sheet.cells_by_coord.keys() {
        if col == 1
            && let Some(t) = cell_text(sheet, row, 1)
        {
            labels.insert(row, t.to_lowercase());
        }
    }
    let find = |anchor: &RowAnchor| -> Option<u32> {
        labels
            .iter()
            .find(|(_, l)| anchor.matches(l))
            .map(|(r, _)| *r)
    };
    let [cos, tcc, aisc, ounces, cos_rate, tcc_rate, aisc_rate] = &ROW_ANCHORS;
    let Some(cos_row) = find(cos) else {
        return Ok(None);
    };
    let need = |anchor: &RowAnchor| find(anchor).ok_or_else(|| format!("no '{}' row", anchor.role));
    let tcc_row = need(tcc)?;
    let aisc_row = need(aisc)?;
    let ounces_row = need(ounces)?;
    let cos_rate_row = need(cos_rate)?;
    let tcc_rate_row = need(tcc_rate)?;
    let aisc_rate_row = need(aisc_rate)?;
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
    let header_row =
        header_row.ok_or_else(|| "no 'header with a registered subject' row".to_string())?;
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
    scaled_rate_holds(n, k, q, r, 1, 1)
}

/// [`rate_interval_holds`] for the rate 1000·s·N/Q at a stated share s = `s_num/s_den` > 0 (BD-331.S3). A positive
/// factor keeps each edge monotone, so the corners are 1000·s_num·(2n ± k)/(s_den·(2q ± 1)).
#[must_use]
pub fn scaled_rate_holds(n: i64, k: i64, q: i64, r: i64, s_num: i64, s_den: i64) -> bool {
    let (n2, k, q2, r2, a, b) = (
        2 * i128::from(n),
        i128::from(k),
        2 * i128::from(q),
        2 * i128::from(r),
        i128::from(s_num),
        i128::from(s_den),
    );
    let corners = [
        (1000 * a * (n2 - k), b * (q2 - 1)),
        (1000 * a * (n2 - k), b * (q2 + 1)),
        (1000 * a * (n2 + k), b * (q2 - 1)),
        (1000 * a * (n2 + k), b * (q2 + 1)),
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
pub(crate) fn rate_range_text(n: i64, k: i64, q: i64) -> String {
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

/// The shares s for which the printed n, q, r fit 1000·s·N/Q (N = n ± k/2, Q = q ± 1/2, R = r ± 1/2, all
/// positive), as exact fractions `(num, den)`: s ∈ [(r − ½)(q − ½)/(1000(n + k/2)), (r + ½)(q + ½)/(1000(n − k/2))].
/// The ratio R·Q/(1000·N) is continuous on the box, so every share in between is attained.
#[must_use]
pub fn feasible_share(n: i64, k: i64, q: i64, r: i64) -> Option<((i128, i128), (i128, i128))> {
    let (n, k, q, r) = (i128::from(n), i128::from(k), i128::from(q), i128::from(r));
    if 2 * n - k < 1 || k < 0 || q < 1 || r < 1 {
        return None;
    }
    Some((
        ((2 * r - 1) * (2 * q - 1), 2000 * (2 * n + k)),
        ((2 * r + 1) * (2 * q + 1), 2000 * (2 * n - k)),
    ))
}

/// A positive fraction as decimal text with four places, rounded down or up (display only).
fn four_places((num, den): (i128, i128), round_up: bool) -> String {
    let scaled = num * 10_000;
    let t = if round_up {
        (scaled + den - 1) / den
    } else {
        scaled / den
    };
    format!("{}.{:04}", t / 10_000, t % 10_000)
}

/// [`feasible_share`] as `[lo, hi]` to four places, rounded outward so the text contains the exact interval.
#[must_use]
pub fn feasible_share_text(n: i64, k: i64, q: i64, r: i64) -> Option<String> {
    feasible_share(n, k, q, r)
        .map(|(lo, hi)| format!("[{}, {}]", four_places(lo, false), four_places(hi, true)))
}

/// The state of a cost-of-sales-per-ounce instance whose column has a non-zero non-controlling-interest line
/// (BD-331.S3). The sheet's footnote states the rate on an attributable basis, so the rate is checked at the
/// subject's registered, label-confirmed share; regions and sites without a confirmed share stay (v).
fn stated_share_state(
    sheet: &str,
    footnote: Option<&str>,
    share: Option<&ShareBasis>,
    miss: RateMiss,
) -> IdentityState {
    let refused = |reason| IdentityState::Refused { reason };
    let Some(footnote) = footnote else {
        return refused(RefusedReason::FootnoteNotFound);
    };
    let (num, den, evidence, label_cell) = match share {
        Some(ShareBasis::Region) => return refused(RefusedReason::RegionalNumerator),
        None => return refused(RefusedReason::NoRegisteredShare),
        Some(ShareBasis::Unconfirmed { why }) => {
            return IdentityState::RefusedAt {
                reason: RefusedReason::ShareNotConfirmed,
                at: why.clone(),
            };
        }
        Some(ShareBasis::Confirmed {
            num,
            den,
            evidence,
            label_cell,
        }) => (*num, *den, evidence, label_cell),
    };
    let Some(s_interval) = feasible_share_text(miss.n, miss.k, miss.q, miss.r) else {
        return refused(RefusedReason::ShareRuleNeedsPositive);
    };
    let share = StatedShare {
        num,
        den,
        evidence: evidence.clone(),
        footnote: format!("{sheet}!{footnote}"),
        label_cell: label_cell.clone(),
    };
    if scaled_rate_holds(miss.n, miss.k, miss.q, miss.r, num, den) {
        IdentityState::Consistent {
            alternate_consistent: scaled_rate_holds(miss.n, 1, miss.q, miss.r, num, den),
            basis: Some(share),
        }
    } else {
        IdentityState::Undecidable {
            why: Undecided::ShareOutside {
                miss,
                share,
                s_interval,
            },
            missing_fact: SHARE_MISSING_FACT.to_string(),
            closing_document: None,
        }
    }
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

/// One column of Appendix K.4's cheapest common test (bd-prmn.8): the printed cost of sales per ounce against
/// 1000·s·N/Q at the column's basis. A diagnostic: never a ledger state and not in the state root.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OneCellCheck {
    pub file: String,
    pub sheet: String,
    pub column: String,
    pub subject: String,
    pub region: bool,
    pub outcome: OneCellOutcome,
}

/// What the one-cell check found for one column.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OneCellOutcome {
    /// No basis to check at. When only the basis is refused (the operands are valid integers), `at_full` and
    /// `at_share` still report the checks at 100% and at the registered share; otherwise both are `None`.
    Refused {
        reason: &'static str,
        at_full: Option<bool>,
        at_share: Option<bool>,
    },
    Checked {
        /// The share `(num, den)` the column is stated on: `(1, 1)` at 100%, the registered, label-confirmed
        /// share where a non-controlling-interest line is non-zero.
        basis: (i64, i64),
        pass: bool,
        /// The check at 100%.
        at_full: bool,
        /// The check at the subject's registered share, for a site that has one, whatever the column's basis.
        at_share: Option<bool>,
        /// The ounces (000s) that would close the check at `basis`, three places, rounded outward.
        implied_oz: (String, String),
    },
}

impl OneCellCheck {
    /// `pass`, `fail` or `refused`.
    #[must_use]
    pub fn state(&self) -> &'static str {
        match self.outcome {
            OneCellOutcome::Refused { .. } => "refused",
            OneCellOutcome::Checked { pass: true, .. } => "pass",
            OneCellOutcome::Checked { pass: false, .. } => "fail",
        }
    }
}

/// The ounces Q for which 1000·s·N/Q meets [r − ½, r + ½] for some N ∈ [n − ½, n + ½], at s = a/b (n, r, a, b
/// ≥ 1): [1000·a·(2n − 1)/(b·(2r + 1)), 1000·a·(2n + 1)/(b·(2r − 1))], to three places, rounded outward.
#[must_use]
pub fn implied_ounces(n: i64, r: i64, a: i64, b: i64) -> (String, String) {
    let (n, r, a, b) = (i128::from(n), i128::from(r), i128::from(a), i128::from(b));
    let three = |num: i128, den: i128, up: bool| {
        let t = if up {
            (num * 1000 + den - 1) / den
        } else {
            num * 1000 / den
        };
        format!("{}.{:03}", t / 1000, t % 1000)
    };
    (
        three(1000 * a * (2 * n - 1), b * (2 * r + 1), false),
        three(1000 * a * (2 * n + 1), b * (2 * r - 1), true),
    )
}

/// Appendix K.4's one-cell check on every registered-subject column of one located table (bd-prmn.8). `shares`
/// is what the run confirmed (as for the ledger); `registered` each subject's registered share, confirmed or not.
pub fn one_cell_checks<S: AsRef<str>>(
    file: &str,
    sheet: &Sheet,
    table: &AiscTable,
    subjects: &[S],
    shares: &BTreeMap<String, ShareBasis>,
    registered: &BTreeMap<String, (i64, i64)>,
    out: &mut Vec<OneCellCheck>,
) {
    let footnote = find_share_footnote(sheet).is_some();
    for c in sheet
        .row_cells(table.header_row)
        .iter()
        .filter(|c| c.col >= 2)
    {
        let Some(header) = cell_text(sheet, table.header_row, c.col) else {
            continue;
        };
        if header.to_lowercase() == "footnote" {
            continue;
        }
        let Some(subject) = match_subject(&header, subjects) else {
            continue;
        };
        let share = shares.get(subject);
        let region = matches!(share, Some(ShareBasis::Region));
        let site_share = registered.get(subject).copied().filter(|_| !region);
        out.push(OneCellCheck {
            file: file.to_string(),
            sheet: sheet.name.clone(),
            column: col_index_to_letters(c.col),
            subject: subject.to_string(),
            region,
            outcome: one_cell_outcome(sheet, table, c.col, footnote, share, site_share),
        });
    }
}

fn one_cell_outcome(
    sheet: &Sheet,
    t: &AiscTable,
    col: u32,
    footnote: bool,
    share: Option<&ShareBasis>,
    site_share: Option<(i64, i64)>,
) -> OneCellOutcome {
    let v = |row: u32| val(sheet, row, col);
    let refused = |reason: &'static str| OneCellOutcome::Refused {
        reason,
        at_full: None,
        at_share: None,
    };
    let mut used: Vec<u32> = t.block1.clone();
    used.push(t.tcc_row);
    used.extend(&t.block2);
    used.extend([
        t.aisc_row,
        t.ounces_row,
        t.cos_rate_row,
        t.tcc_rate_row,
        t.aisc_rate_row,
    ]);
    if used
        .iter()
        .all(|&r| matches!(v(r), Val::Place) || v(r) == Val::Num(0.0))
    {
        return refused("all-zero column");
    }
    let (vn, vq, vr) = (v(t.cos_row), v(t.ounces_row), v(t.cos_rate_row));
    if !matches!(vr, Val::Num(_)) {
        return refused("rate not published");
    }
    if !matches!(vq, Val::Num(_)) {
        return refused("ounces not numeric");
    }
    if vr == Val::Num(0.0) || vq == Val::Num(0.0) {
        return refused("zero rate or ounces");
    }
    if !matches!(vn, Val::Num(_)) {
        return refused("cost of sales not numeric");
    }
    let (Some(n), Some(q), Some(r)) = (as_int(vn), as_int(vq), as_int(vr)) else {
        return refused("unrounded stored value");
    };
    if n < 1 || q < 1 || r < 1 {
        return refused("non-positive operand");
    }
    let at_full = rate_interval_holds(n, 1, q, r);
    let at_share = site_share.map(|(a, b)| scaled_rate_holds(n, 1, q, r, a, b));
    let nci = t
        .nci_rows
        .iter()
        .any(|&row| matches!(v(row), Val::Num(x) if x != 0.0));
    let unbased = |reason: &'static str| OneCellOutcome::Refused {
        reason,
        at_full: Some(at_full),
        at_share,
    };
    let basis = if !nci {
        (1, 1)
    } else {
        match share {
            Some(ShareBasis::Region) => {
                return unbased("regional numerator needs member weighting");
            }
            _ if !footnote => return unbased("attributable-basis footnote not found"),
            None => return unbased("no registered share"),
            Some(ShareBasis::Unconfirmed { .. }) => {
                return unbased("registered share not confirmed by label");
            }
            Some(ShareBasis::Confirmed { num, den, .. }) => (*num, *den),
        }
    };
    let (a, b) = basis;
    OneCellOutcome::Checked {
        basis,
        pass: scaled_rate_holds(n, 1, q, r, a, b),
        at_full,
        at_share,
        implied_oz: implied_ounces(n, r, a, b),
    }
}

/// K.4's verdict over a run's one-cell checks: the failing columns outside the items `k4_item` names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OneCellVerdict {
    pub fail: usize,
    pub k4_fail: usize,
    pub other_fail: usize,
    /// "site-specific" when no column outside the K.4 items fails, otherwise "basis rule".
    pub verdict: &'static str,
    /// The failing columns outside the K.4 items by group, most first.
    pub pattern: Vec<(FailGroup, usize)>,
}

/// A group of failing columns: (subject, basis, passes at the other basis).
pub type FailGroup = (String, (i64, i64), Option<bool>);

/// Groups the failing checks outside `k4_item` (Appendix K.4: "if only these columns fail, the cause is
/// site-specific, not a basis rule"). The other basis is the registered share for a column on 100%, and 100%
/// for a column on a share.
#[must_use]
pub fn one_cell_verdict(
    checks: &[OneCellCheck],
    k4_item: &dyn Fn(&OneCellCheck) -> bool,
) -> OneCellVerdict {
    let mut groups: BTreeMap<FailGroup, usize> = BTreeMap::new();
    let (mut fail, mut k4_fail) = (0, 0);
    for c in checks {
        let OneCellOutcome::Checked {
            basis,
            pass: false,
            at_full,
            at_share,
            ..
        } = &c.outcome
        else {
            continue;
        };
        fail += 1;
        if k4_item(c) {
            k4_fail += 1;
            continue;
        }
        let other = if *basis == (1, 1) {
            *at_share
        } else {
            Some(*at_full)
        };
        *groups
            .entry((c.subject.clone(), *basis, other))
            .or_default() += 1;
    }
    let mut pattern: Vec<_> = groups.into_iter().collect();
    pattern.sort_by(|x, y| y.1.cmp(&x.1).then_with(|| x.0.0.cmp(&y.0.0)));
    let other_fail = fail - k4_fail;
    OneCellVerdict {
        fail,
        k4_fail,
        other_fail,
        verdict: if other_fail == 0 {
            "site-specific"
        } else {
            "basis rule"
        },
        pattern,
    }
}

/// Builds the identity ledger for one located table. `explain` is offered every per-ounce miss and may
/// return an explained state (it must itself re-verify the component); otherwise the miss is (iv). `shares`
/// holds what the run knows about each subject's ownership share for this release (BD-331.S3).
#[allow(clippy::too_many_arguments)]
pub fn ledger_for_table<S: AsRef<str>>(
    file: &str,
    sheet: &Sheet,
    table: &AiscTable,
    subjects: &[S],
    shares: &BTreeMap<String, ShareBasis>,
    explain: &mut dyn FnMut(&str, IdentityFamily, RateMiss) -> Option<IdentityState>,
    instances: &mut Vec<IdentityInstance>,
    unregistered: &mut Vec<UnregisteredHeader>,
) {
    let footnote = find_share_footnote(sheet);
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
        let mut push = |family: IdentityFamily, row: u32, state: IdentityState, calc: Calc| {
            let all_zero = state
                == IdentityState::NotPublished {
                    reason: NotPublishedReason::AllZeroColumn,
                };
            instances.push(IdentityInstance {
                file: file.to_string(),
                sheet: sheet.name.clone(),
                column: letters.clone(),
                subject: subject.to_string(),
                family,
                cell: format!("{letters}{row}"),
                state,
                operands: family_operands(
                    file,
                    sheet,
                    table,
                    col,
                    family,
                    all_zero,
                    footnote.as_deref(),
                ),
                calc,
            });
        };
        let share = shares.get(subject);
        let stated = ShareInputs {
            footnote: footnote.as_deref(),
            share,
        };
        column_ledger(sheet, table, col, subject, &stated, explain, &mut push);
    }
}

/// The stated-share inputs of one column: the sheet's attributable-basis footnote and the subject's share.
struct ShareInputs<'a> {
    footnote: Option<&'a str>,
    share: Option<&'a ShareBasis>,
}

fn column_ledger(
    sheet: &Sheet,
    t: &AiscTable,
    col: u32,
    subject: &str,
    stated: &ShareInputs<'_>,
    explain: &mut dyn FnMut(&str, IdentityFamily, RateMiss) -> Option<IdentityState>,
    push: &mut dyn FnMut(IdentityFamily, u32, IdentityState, Calc),
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
        let footings = [
            (IdentityFamily::TccFooting, t.tcc_row),
            (IdentityFamily::AiscFooting, t.aisc_row),
        ];
        for (family, row) in footings.into_iter().chain(rate_rows) {
            push(
                family,
                row,
                IdentityState::NotPublished {
                    reason: NotPublishedReason::AllZeroColumn,
                },
                Calc::None,
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
        let (state, calc) =
            footing_state(&v, &cell_ref, total, lines, family, &t.block1, &all_lines);
        push(family, total, state, calc);
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
        let mut calc = Calc::None;
        let refused = |reason| IdentityState::Refused { reason };
        let state = if !matches!(rv, Val::Num(_)) {
            IdentityState::NotPublished {
                reason: NotPublishedReason::RateNotPublished,
            }
        } else if !matches!(q, Val::Num(_)) {
            refused(RefusedReason::OuncesNotNumeric)
        } else if rv == Val::Num(0.0) && q == Val::Num(0.0) {
            IdentityState::NotPublished {
                reason: NotPublishedReason::ZeroRateAndOunces,
            }
        } else if rv == Val::Num(0.0) || q == Val::Num(0.0) {
            refused(RefusedReason::OneOfRateAndOuncesZero)
        } else {
            let mut cells = vec![rate_row, t.ounces_row, numerator_row];
            cells.extend(&lines);
            if cells.iter().any(|&r| non_numeric(v(r))) {
                refused(RefusedReason::NumeratorLineNotNumeric)
            } else if !cells.iter().all(|&r| int_or_place(v(r))) {
                refused(RefusedReason::Unrounded)
            } else if as_int(v(numerator_row)).is_none() {
                refused(RefusedReason::NumeratorPlaceholder)
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
                calc = Calc::Rate(miss);
                let nci = t
                    .nci_rows
                    .iter()
                    .any(|&r| matches!(v(r), Val::Num(x) if x != 0.0));
                if family == IdentityFamily::CosPerOunce && nci {
                    stated_share_state(&sheet.name, stated.footnote, stated.share, miss)
                } else if rate_interval_holds(miss.n, miss.k, miss.q, miss.r) {
                    IdentityState::Consistent {
                        alternate_consistent: rate_interval_holds(miss.n, 1, miss.q, miss.r),
                        basis: None,
                    }
                } else if let Some(explained) = explain(subject, family, miss) {
                    explained
                } else {
                    IdentityState::Undecidable {
                        why: Undecided::RateOutside(miss),
                        missing_fact: DEFAULT_MISSING_FACT.to_string(),
                        closing_document: None,
                    }
                }
            }
        };
        push(family, rate_row, state, calc);
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
) -> (IdentityState, Calc) {
    if !matches!(v(total), Val::Num(_)) {
        let state = IdentityState::NotPublished {
            reason: NotPublishedReason::TotalNotPublished,
        };
        return (state, Calc::None);
    }
    if lines.iter().any(|&r| non_numeric(v(r))) {
        let state = IdentityState::Refused {
            reason: RefusedReason::LineNotNumeric,
        };
        return (state, Calc::None);
    }
    if !lines.iter().chain([&total]).all(|&r| int_or_place(v(r))) {
        let state = IdentityState::Refused {
            reason: RefusedReason::Unrounded,
        };
        return (state, Calc::None);
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
    let calc = Calc::Footing {
        printed,
        component_sum: sum,
        k_alt,
    };
    if sum == printed {
        let state = IdentityState::Consistent {
            alternate_consistent,
            basis: None,
        };
        return (state, calc);
    }
    let sign_flip_witness = lines
        .iter()
        .find(|&&r| {
            let x = line_value(v(r));
            x != 0 && sum - 2 * x == printed
        })
        .map(|&r| cell_ref(r));
    let state = IdentityState::IssuerDiscrepancy(FootingCertificate {
        printed,
        component_sum: sum,
        residual: printed - sum,
        total_cell: cell_ref(total),
        line_cells: lines.iter().map(|&r| cell_ref(r)).collect(),
        sign_flip_witness,
        alternate_consistent,
    });
    (state, calc)
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
        // bd-sjrh.1, NEW-cell-number-formats' worked example: 129500/69.5 = 1863.3093… and 134500/68.5 =
        // 1963.5036…; 127500/69.5 = 1834.5323… and 138500/68.5 = 2021.8978….
        assert_eq!(rate_range_text(132, 5, 69), "[1863.309, 1963.504]");
        assert_eq!(rate_range_text(133, 11, 69), "[1834.532, 2021.898]");
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

    /// bd-prmn.8: the one-cell check and its implied ounces on hand-computed columns.
    #[test]
    fn one_cell_implied_ounces_known_answers() {
        // n = 100, q = 50, r = 2000: 1000·100/50 = 2000 exactly, inside.
        assert!(rate_interval_holds(100, 1, 50, 2000));
        // The implied ounces at 100%: [1000·199/4001, 1000·201/3999] = [49.7375…, 50.2625…], which holds 50.
        assert_eq!(
            implied_ounces(100, 2000, 1, 1),
            ("49.737".to_string(), "50.263".to_string())
        );
        // r = 2100: [1000·199/4201, 1000·201/4199] = [47.3696…, 47.8685…] misses [49.5, 50.5], so it fails.
        assert!(!rate_interval_holds(100, 1, 50, 2100));
        assert_eq!(
            implied_ounces(100, 2100, 1, 1),
            ("47.369".to_string(), "47.869".to_string())
        );
        // At s = 3/4 the implied ounces scale by 3/4: [1000·3·199/(4·4201), 1000·3·201/(4·4199)]
        // = [35.5272…, 35.9014…]; q = 36 lies in [35.5, 36.5] and meets it, so the scaled check passes.
        assert_eq!(
            implied_ounces(100, 2100, 3, 4),
            ("35.527".to_string(), "35.902".to_string())
        );
        assert!(scaled_rate_holds(100, 1, 36, 2100, 3, 4));
        assert!(!scaled_rate_holds(100, 1, 34, 2100, 3, 4));
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

    /// BD-331.S3 known answer: North Mara, `Barrick_Q2_2026_AISC.xlsx` 'Q2 2026', (n, q, r) = (117, 37, 2736).
    #[test]
    fn test_scaled_rate_north_mara_q2_2026_misses() {
        assert!(!scaled_rate_holds(117, 1, 37, 2736, 21, 25));
        // s ∈ [5471·73/(2000·235), 5473·75/(2000·233)] = [0.849751…, 0.880847…].
        assert_eq!(
            feasible_share(117, 1, 37, 2736),
            Some(((399_383, 470_000), (410_475, 466_000)))
        );
        // Rounded outward; the bead's [0.8498, 0.8808] is the same exact interval rounded to nearest.
        assert_eq!(
            feasible_share_text(117, 1, 37, 2736).as_deref(),
            Some("[0.8497, 0.8809]")
        );
        assert!(scaled_rate_holds(117, 1, 37, 2736, 85, 100));
        assert!(scaled_rate_holds(117, 1, 37, 2736, 88, 100));
        assert!(!scaled_rate_holds(117, 1, 37, 2736, 89, 100));
    }

    #[test]
    fn test_scaled_rate_fit_synthetic() {
        // Synthetic Site H: 1000·¾·400/150 = 2000 fits; 2200 does not.
        assert!(scaled_rate_holds(400, 1, 150, 2000, 3, 4));
        assert!(!scaled_rate_holds(400, 1, 150, 2200, 3, 4));
        // 4399·299/(2000·801) = 0.821037…, 4401·301/(2000·799) = 0.828974…
        assert_eq!(
            feasible_share_text(400, 1, 150, 2200).as_deref(),
            Some("[0.8210, 0.8290]")
        );
        // At s = 1 the scaled test is the unscaled one.
        for (n, k, q, r) in [(79, 1, 68, 1162), (79, 1, 68, 1141), (200, 5, 100, 2400)] {
            assert_eq!(
                scaled_rate_holds(n, k, q, r, 1, 1),
                rate_interval_holds(n, k, q, r)
            );
        }
        // Non-positive inputs have no share interval (the run refuses them).
        assert_eq!(feasible_share(0, 1, 37, 2736), None);
        assert_eq!(feasible_share(117, 1, 0, 2736), None);
        assert_eq!(feasible_share(117, 1, 37, -5), None);
    }

    /// The corner test and the feasible interval are the same rule: s fits iff lo ≤ s ≤ hi, end points included.
    #[test]
    fn test_corner_test_agrees_with_feasible_interval() {
        let shares = [
            (21, 25),
            (123, 200),
            (3, 5),
            (4, 5),
            (897, 1000),
            (3, 4),
            (1, 1),
        ];
        let mut fits = 0;
        for n in [1, 2, 7, 117, 400] {
            for q in [1, 3, 37, 150] {
                for r in [1, 500, 2000, 2736, 9000] {
                    let Some(((ln, ld), (hn, hd))) = feasible_share(n, 1, q, r) else {
                        panic!("positive inputs have an interval")
                    };
                    for (a, b) in shares {
                        let (a128, b128) = (i128::from(a), i128::from(b));
                        let inside = cmp_frac(a128, b128, ln, ld) != Ordering::Less
                            && cmp_frac(a128, b128, hn, hd) != Ordering::Greater;
                        assert_eq!(
                            scaled_rate_holds(n, 1, q, r, a, b),
                            inside,
                            "n={n} q={q} r={r} s={a}/{b}"
                        );
                        fits += usize::from(inside);
                    }
                }
            }
        }
        // Both outcomes occur, so the agreement is not vacuous.
        assert!(fits > 0 && fits < 5 * 4 * 5 * shares.len(), "{fits}");
    }

    fn sheet_of(name: &str, cells: Vec<(String, fmine_io::xlsx::MinimalCell)>) -> Sheet {
        use fmine_io::xlsx::{MinimalSheet, Workbook, build_minimal_xlsx};
        let bytes = build_minimal_xlsx(&[MinimalSheet {
            name: name.to_string(),
            hidden: false,
            cells,
        }]);
        let wb = Workbook::from_bytes("t.xlsx", &bytes).expect("parses");
        wb.sheet(name).expect("sheet").clone()
    }

    fn text(cell: &str, s: &str) -> (String, fmine_io::xlsx::MinimalCell) {
        (
            cell.to_string(),
            fmine_io::xlsx::MinimalCell::Text(s.to_string()),
        )
    }

    #[test]
    fn test_share_label_confirmation() {
        let qtd = sheet_of(
            "QTD",
            vec![
                text("A5", "Carlin (61.5%)"),
                text("A9", "Nevada Gold Mines LLC (61.5%)2"),
                text("A10", "Cortez Hills (61.5%)"),
                text("A19", "North Mara (84%)"),
                text("B19", "North Mara (85%)"),
            ],
        );
        assert_eq!(
            confirm_share_label(&qtd, "Carlin", 123, 200).as_deref(),
            Ok("A5")
        );
        // Trailing footnote digits are stripped; the registered label carries the "LLC".
        assert_eq!(
            confirm_share_label(&qtd, "Nevada Gold Mines LLC", 123, 200).as_deref(),
            Ok("A9")
        );
        assert!(confirm_share_label(&qtd, "Nevada Gold Mines", 123, 200).is_err());
        // Only column A is read, and the printed share must equal the registered one exactly.
        assert_eq!(
            confirm_share_label(&qtd, "North Mara", 21, 25).as_deref(),
            Ok("A19")
        );
        assert_eq!(
            confirm_share_label(&qtd, "North Mara", 17, 20),
            Err("QTD!A19 prints 84%, registered 17/20".to_string())
        );
        // "Cortez" is not a prefix match for "Cortez Hills".
        assert_eq!(
            confirm_share_label(&qtd, "Cortez", 123, 200),
            Err("no column-A label 'Cortez (x%)' on QTD".to_string())
        );
        let twice = sheet_of(
            "QTD",
            vec![text("A5", "Tongon (89.7%)"), text("A30", "Tongon (89.7%)6")],
        );
        assert_eq!(
            confirm_share_label(&twice, "Tongon", 897, 1000),
            Err("label 'Tongon' repeats on QTD at A5, A30".to_string())
        );
        assert_eq!(decimal_fraction("89.7"), Some((897, 10)));
        assert_eq!(decimal_fraction("84"), Some((84, 1)));
        for bad in ["", ".5", "8.4.1", "61.55555", "-5", "x"] {
            assert_eq!(decimal_fraction(bad), None, "{bad}");
        }
    }

    #[test]
    fn test_qtd_sheet_found_by_content() {
        use fmine_io::xlsx::MinimalCell;
        let qtd = |name: &str, period: &str, year: f64| {
            sheet_of(
                name,
                vec![
                    text("A1", "Summary of Operations"),
                    text("A3", period),
                    ("B3".to_string(), MinimalCell::Number(year)),
                ],
            )
        };
        let sheets = vec![
            qtd("YTD", "For the six months ended June 30,", 2026.0),
            qtd("Q", "For the three months ended June 30,", 2026.0),
            qtd("Prior", "For the three months ended June 30,", 2025.0),
        ];
        assert_eq!(
            find_qtd_sheet(&sheets, "2026Q2").map(|s| s.name.as_str()),
            Ok("Q")
        );
        assert_eq!(
            find_qtd_sheet(&sheets, "2025Q2").map(|s| s.name.as_str()),
            Ok("Prior")
        );
        assert!(find_qtd_sheet(&sheets, "2026Q1").is_err());
        assert!(find_qtd_sheet(&sheets, "2026Q5").is_err());
    }

    #[test]
    fn test_share_footnote_found_by_text() {
        let s = sheet_of(
            "Q2 2026",
            vec![
                text("A30", "f.  Other footnote."),
                text(
                    "A32",
                    "g.  COS/oz - Gold COS/oz is calculated as cost of sales across our gold operations \
                     (excluding sites in closure or care and maintenance) divided by ounces sold (both on an \
                     attributable basis using Barrick's ownership share). ",
                ),
            ],
        );
        assert_eq!(find_share_footnote(&s).as_deref(), Some("A32"));
        let none = sheet_of(
            "Q2 2026",
            vec![text(
                "A32",
                "g.  Gold COS/oz is calculated as cost of sales divided by ounces sold.",
            )],
        );
        assert_eq!(find_share_footnote(&none), None);
    }
}
