//! Label-anchored reader for the issuer's AISC reconciliation tables and the identity ledger over them
//! (`GATE-BULL-01` part 2, plan §21.6).
//!
//! The declared construction is the component sum (DESIGN, stated before any run):
//! - total cash costs (TCC) = Σ block-1 lines (the cost-of-sales row down to the row before TCC);
//! - AISC = TCC + Σ block-2 lines (the rows between TCC and AISC);
//! - each per-ounce rate r = 1000·N/Q, checked on intervals, because every printed value is rounded to a
//!   whole number: N ± k/2 (k = numeric lines summed into N), Q ± 1/2, r ± 1/2.
//!
//! Every footing and rate decision is a run of the exact kernel `fmine_numerics::exact_feas` (BD-340; the
//! binding is BD-331.S6), so each answer is an exact rational witness or certificate that the kernel's own
//! checker re-verified. The alternative construction (subtotal rounded once: N ± 1/2) is computed and reported,
//! never scored.
//!
//! Every instance lands in exactly one state: (i) consistent, (ii) explained by a registered
//! reconstruction, (iii) issuer discrepancy with an exact certificate, (iv) undecidable from public data,
//! (v) refused, or not published (excluded by name).
//!
//! A region's cost-of-sales rate (BD-331.S4) is checked against its registered members for the release, weighted
//! by attributable ounces sold: the members' ounces must foot to the region's under the component sum, and the
//! McCormick relaxation of `rate·Σwᵢ = Σ rateᵢ·wᵢ` either refutes the rate with a checked certificate (iv) or
//! does not. Then the exact decision (`mine_ledger::weighted_mean_holds`) finds weights and rates in their boxes
//! that reach the rate, checked exactly, which is (i) `MembersExact`, or none, which is (iv); only when it gives
//! no answer is the rate (i) qualified `relaxation_only`: no conflict shown, no value proved.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use fmine_core::hash::Digest32;
use fmine_io::numfmt;
use fmine_io::xlsx::{CellValue, ExtractedCell, Sheet, col_index_to_letters, parse_a1_ref};
use fmine_numerics::exact_feas::{self as xf, Rat};

use crate::LoadedSource;
use crate::key::{
    KeyedCell, LineKey, OwnershipBasis, PublicKey, ShareSource, Vintage, classify,
    parse_sheet_period, public_claim, scope_for, status_for,
};
use crate::sources::{IssuerProfile, SourceKind};

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
    /// label when the construction needed one (BD-331.S3), or the members a region's rate was weighed against,
    /// exactly or `relaxation_only` (BD-331.S4).
    Consistent {
        alternate_consistent: bool,
        basis: Option<ConsistentBasis>,
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
    /// An operand's number has no displayed value the declared construction can use (bd-jg43.3): see
    /// [`Undisplayable`].
    GeneralNonInteger,
    UnsupportedFormat,
    DecimalsShown,
    /// The exact kernel made no claim (BD-331.S6): a cap, an overflow, a refused posing or a checker rejection.
    KernelIndeterminate,
    /// A region member's rate or ounces is neither a whole printed number (ounces not zero) nor, with the other,
    /// a zero or a placeholder (BD-331.S4); `RefusedAt` names the cell.
    RegionMemberOperand,
    /// No registered member of the region prints a rate and ounces.
    RegionNoMembers,
    /// The members' printed ounces do not sum to the region's (component sum); `RefusedAt` names the region's.
    RegionOuncesDoNotFoot,
}

impl RefusedReason {
    pub const ALL: [Self; 17] = [
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
        Self::GeneralNonInteger,
        Self::UnsupportedFormat,
        Self::DecimalsShown,
        Self::KernelIndeterminate,
        Self::RegionMemberOperand,
        Self::RegionNoMembers,
        Self::RegionOuncesDoNotFoot,
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
            Self::GeneralNonInteger => "general_format_non_integer",
            Self::UnsupportedFormat => "unsupported_number_format",
            Self::DecimalsShown => "displayed_decimals_not_supported",
            Self::KernelIndeterminate => "exact_kernel_indeterminate",
            Self::RegionMemberOperand => "region_member_operand",
            Self::RegionNoMembers => "region_no_members",
            Self::RegionOuncesDoNotFoot => "region_ounces_do_not_foot",
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
            Self::GeneralNonInteger => "general format with non-integer stored value",
            Self::UnsupportedFormat => "unsupported number format",
            Self::DecimalsShown => {
                "decimals displayed; the declared construction sums whole displayed numbers"
            }
            Self::KernelIndeterminate => "the exact kernel made no claim",
            Self::RegionMemberOperand => "a region member's rate or ounces cannot be weighed",
            Self::RegionNoMembers => "no region member prints a rate and ounces",
            Self::RegionOuncesDoNotFoot => "the region's ounces do not foot to its members'",
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

/// Why a cost-of-sales rate is consistent beyond its own column.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConsistentBasis {
    /// The registered, label-confirmed share the rate was checked at (BD-331.S3).
    Share(StatedShare),
    /// A region the McCormick relaxation of its members' weighted mean does not refute and the exact decision
    /// gave no answer for (BD-331.S4, `relaxation_only`): no conflict is shown, and no value is proved.
    Members(MemberWeighting),
    /// A region some weighted mean of its members reaches exactly (BD-331.S4): weights and rates inside their
    /// printed boxes whose weighted mean is a rate inside the region's, checked exactly
    /// (`mine_ledger::weighted_mean_holds`).
    MembersExact(MemberWeighting),
}

impl ConsistentBasis {
    #[must_use]
    pub fn text(&self) -> String {
        match self {
            Self::Share(s) => s.text(),
            Self::Members(m) => format!("relaxation_only; {}", m.text()),
            Self::MembersExact(m) => format!("exact weighted mean; {}", m.text()),
        }
    }
}

/// A region's cost of sales per ounce against its registered members, weighted by attributable ounces sold
/// (BD-331.S4): `rate·Σwᵢ = Σ rateᵢ·wᵢ` with the region's ounces footing to the members'.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemberWeighting {
    /// The members weighed, in registry order.
    pub used: Vec<WeightedMember>,
    /// Members whose rate and ounces both print zero, or both a placeholder: they sold nothing and enter as
    /// nothing.
    pub excluded: Vec<String>,
    /// Members registered for the release that the sheet does not print.
    pub absent: Vec<String>,
}

/// One member's printed rate and attributable ounces.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WeightedMember {
    pub subject: String,
    pub rate_cell: String,
    pub rate: i64,
    pub ounces_cell: String,
    pub ounces: i64,
}

impl MemberWeighting {
    #[must_use]
    pub fn text(&self) -> String {
        let used: Vec<String> = self
            .used
            .iter()
            .map(|m| format!("{} {}/{}", m.subject, m.rate_cell, m.ounces_cell))
            .collect();
        let mut out = format!("weighted by ounces sold over {}", used.join(", "));
        for (what, names) in [
            ("sold nothing", &self.excluded),
            ("not printed", &self.absent),
        ] {
            if !names.is_empty() {
                let _ = write!(out, "; {what}: {}", names.join(", "));
            }
        }
        out
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
    /// A region's rate lies outside every weighted mean its members' printed rates and ounces allow
    /// (BD-331.S4): the McCormick certificate (`certificate`, its digest) refutes "the region is these members,
    /// weighted by attributable ounces sold", or, where the relaxation does not, the exact decision does
    /// (`certificate` is then [`EXACT_REGION_REFUTATION`]). It blames no cell.
    RegionOutsideMembers {
        weighting: MemberWeighting,
        certificate: String,
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
            Self::RegionOutsideMembers { .. } => "region_outside_members",
        }
    }

    #[must_use]
    pub fn text(&self) -> String {
        match self {
            Self::RateOutside(m) => format!(
                "printed {} outside 1000·N/Q range {} for N = {} ± {}/2, Q = {} ± 1/2",
                m.r,
                implied_rate_text(m.n, m.k, m.q),
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
            Self::RegionOutsideMembers {
                weighting,
                certificate,
            } if certificate == EXACT_REGION_REFUTATION => format!(
                "the region's rate lies outside every weighted mean of its members' printed rates, {}; refuted by \
                 the exact decision, which the McCormick relaxation does not refute",
                weighting.text()
            ),
            Self::RegionOutsideMembers {
                weighting,
                certificate,
            } => format!(
                "the region's rate lies outside every weighted mean of its members' printed rates, {}; McCormick \
                 certificate {certificate}",
                weighting.text()
            ),
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
    /// A region: its numerator rolls up member sites held at different shares. `members` are the sites the
    /// registry maps into it for the release, in registry order (BD-331.S4); none means no registered map.
    Region { members: Vec<String> },
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
    /// A region member's printed cost-of-sales rate (BD-331.S4).
    MemberRate,
    /// A region member's printed attributable ounces sold (BD-331.S4).
    MemberOunces,
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
            Self::MemberRate => "member_rate",
            Self::MemberOunces => "member_ounces",
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
    /// A number with no whole displayed value under its format (bd-jg43.3), as its stored text.
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

/// A cited operand for which no semantic key can be constructed. The original operand remains in the ledger;
/// keying never drops it or changes its state (BD-332.S2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyingRefusal {
    pub locator: String,
    pub reason: String,
}

/// Numeric and placeholder AISC cells keyed independently of their spreadsheet coordinates (BD-332.S2).
/// Textual `LabelCheck` operands remain context references. Numeric operands in Mine Stats files are keyed by
/// that reader's adapter, which knows their block periods and units.
#[derive(Debug, Clone, Default)]
pub struct AiscClaimIndex {
    pub cells: BTreeMap<String, KeyedCell>,
    pub refusals: Vec<KeyingRefusal>,
    /// All-zero columns with no registered subject status for their period, named `file:sheet!column`.
    /// Their numeric zeros remain numbers; the ledger's existing not-published state is not upgraded to a fact.
    pub unregistered_all_zero: Vec<String>,
}

/// Builds one public Claim per distinct AISC operand and visible co-product rate, keeping every reporting
/// vintage. The subject is read from the cell's own registered column, including regional member columns.
/// A row's basis remains `Unprinted` unless its label explicitly states an attributable basis and a share
/// is registered. Co-product rates have their own semantic line; they do not enter the by-product identity
/// ledger or inherit its arithmetic conclusions.
#[must_use]
pub fn key_aisc_operands(
    profile: &IssuerProfile,
    sources: &[LoadedSource],
    instances: &[IdentityInstance],
) -> AiscClaimIndex {
    let sources: BTreeMap<&str, &LoadedSource> =
        sources.iter().map(|s| (s.file_name.as_str(), s)).collect();
    let operands: BTreeMap<String, &Operand> = instances
        .iter()
        .flat_map(|i| &i.operands)
        .filter(|o| o.role != OperandRole::LabelCheck)
        .map(|o| (format!("{}:{}!{}", o.file, o.sheet, o.cell), o))
        .collect();
    let mut tables: BTreeMap<(String, String), Result<AiscTable, String>> = BTreeMap::new();
    let mut coproduct_rows: BTreeMap<(String, String), BTreeSet<u32>> = BTreeMap::new();
    let mut index = AiscClaimIndex::default();
    for (locator, operand) in operands {
        let result = (|| {
            let source = sources
                .get(operand.file.as_str())
                .ok_or_else(|| "the operand's source file was not loaded".to_string())?;
            if source.kind != SourceKind::AiscReconciliation {
                return Ok(None);
            }
            let sheet = source
                .workbook
                .sheets
                .iter()
                .chain(&source.workbook.hidden_sheets)
                .find(|s| s.name == operand.sheet)
                .ok_or_else(|| "the operand's worksheet was not loaded".to_string())?;
            let table = tables
                .entry((operand.file.clone(), operand.sheet.clone()))
                .or_insert_with(|| {
                    locate_table(sheet, &profile.subjects)?
                        .ok_or_else(|| "the worksheet has no AISC table".to_string())
                })
                .as_ref()
                .map_err(Clone::clone)?;
            let rows = coproduct_rows
                .entry((operand.file.clone(), operand.sheet.clone()))
                .or_insert_with(|| coproduct_rate_rows(sheet));
            key_aisc_operand(profile, source, sheet, table, rows, operand).map(Some)
        })();
        match result {
            Ok(Some(cell)) => {
                index.cells.insert(locator, cell);
            }
            Ok(None) => {}
            Err(reason) => index.refusals.push(KeyingRefusal { locator, reason }),
        }
    }
    // FACT: the published co-product rate is outside the by-product identity operands (for example,
    // Barrick_Q1_2025_AISC.xlsx:Q1 2024!C22). Re-read its visible row and registered subject columns rather
    // than manufacturing a numeric identity merely to make the published Claim available.
    for source in sources
        .values()
        .filter(|s| s.kind == SourceKind::AiscReconciliation)
    {
        for sheet in &source.workbook.sheets {
            let rows = coproduct_rows
                .entry((source.file_name.clone(), sheet.name.clone()))
                .or_insert_with(|| coproduct_rate_rows(sheet));
            if rows.is_empty() {
                continue;
            }
            let table = tables
                .entry((source.file_name.clone(), sheet.name.clone()))
                .or_insert_with(|| {
                    locate_table(sheet, &profile.subjects)?
                        .ok_or_else(|| "the worksheet has no AISC table".to_string())
                });
            let table = match table.as_ref() {
                Ok(table) => table,
                Err(reason) => {
                    for row in rows.iter() {
                        index.refusals.push(KeyingRefusal {
                            locator: format!("{}:{}!A{row}", source.file_name, sheet.name),
                            reason: reason.clone(),
                        });
                    }
                    continue;
                }
            };
            for header in sheet.row_cells(table.header_row) {
                if header.col < 2
                    || cell_text(sheet, table.header_row, header.col)
                        .and_then(|h| match_subject(&h, &profile.subjects))
                        .is_none()
                {
                    continue;
                }
                for &row in rows.iter() {
                    let operand =
                        operand_at(&source.file_name, sheet, row, header.col, OperandRole::Rate);
                    let locator = format!("{}:{}!{}", operand.file, operand.sheet, operand.cell);
                    if index.cells.contains_key(&locator) {
                        continue;
                    }
                    match key_aisc_operand(profile, source, sheet, table, rows, &operand) {
                        Ok(cell) => {
                            index.cells.insert(locator, cell);
                        }
                        Err(reason) => index.refusals.push(KeyingRefusal { locator, reason }),
                    }
                }
            }
        }
    }
    let mut unregistered = BTreeSet::new();
    for instance in instances.iter().filter(|i| {
        matches!(
            i.state,
            IdentityState::NotPublished {
                reason: NotPublishedReason::AllZeroColumn
            }
        )
    }) {
        if let Ok(period) = parse_sheet_period(&instance.sheet)
            && status_for(&profile.subject_statuses, &instance.subject, &period).is_none()
        {
            unregistered.insert(format!(
                "{}:{}!{}",
                instance.file, instance.sheet, instance.column
            ));
        }
    }
    index.unregistered_all_zero = unregistered.into_iter().collect();
    index
}

fn key_aisc_operand(
    profile: &IssuerProfile,
    source: &LoadedSource,
    sheet: &Sheet,
    table: &AiscTable,
    coproduct_rows: &BTreeSet<u32>,
    operand: &Operand,
) -> Result<KeyedCell, String> {
    let (row, col) =
        parse_a1_ref(&operand.cell).ok_or_else(|| "invalid operand cell reference".to_string())?;
    if operand_at(&operand.file, sheet, row, col, operand.role).value != operand.value {
        return Err("the ledger operand no longer matches its source cell".to_string());
    }
    let header = cell_text(sheet, table.header_row, col)
        .ok_or_else(|| "the operand's subject header is missing".to_string())?;
    let subject = match_subject(&header, &profile.subjects)
        .ok_or_else(|| "the operand's subject header is not registered".to_string())?;
    let period = parse_sheet_period(&sheet.name).map_err(|e| e.to_string())?;
    let value = classify(
        sheet.cell(row, col),
        status_for(&profile.subject_statuses, subject, &period),
    )
    .map_err(|e| e.to_string())?;
    let mut rows = aisc_key_rows(table);
    rows.extend(coproduct_rows);
    if !rows.contains(&row) {
        return Err("the operand is outside the table's registered metric rows".to_string());
    }
    let label = aisc_key_label(sheet, table, row)?;
    let occurrence = rows
        .iter()
        .filter(|&&r| r < row)
        .filter(|&&r| aisc_key_label(sheet, table, r).as_deref() == Ok(label.as_str()))
        .count();
    let unit = if row == table.ounces_row {
        "koz"
    } else if [table.cos_rate_row, table.tcc_rate_row, table.aisc_rate_row].contains(&row)
        || coproduct_rows.contains(&row)
    {
        "usd_per_oz"
    } else {
        "usd_m"
    };
    let raw_label = cell_text(sheet, row, 1).unwrap_or_default().to_lowercase();
    let registered_share = profile.shares.iter().find(|s| s.subject == subject);
    let attributable = raw_label.contains("attributable basis");
    let basis = if attributable {
        registered_share.map_or(OwnershipBasis::Unprinted, |s| {
            OwnershipBasis::Attributable {
                num: s.num,
                den: s.den,
                source: ShareSource::Registry,
            }
        })
    } else {
        OwnershipBasis::Unprinted
    };
    let key = PublicKey {
        issuer: profile.issuer.clone(),
        subject: subject.to_string(),
        valid_period: period,
        reporting_vintage: Vintage {
            release: source.quarter.clone(),
            file_sha256: Digest32(source.workbook.sha256).to_hex(),
        },
        ownership_basis: basis,
        scope: scope_for(profile, subject, &source.quarter),
        unit: unit.to_string(),
        line: LineKey {
            label,
            occurrence: u32::try_from(occurrence)
                .map_err(|_| "too many repeated metric labels".to_string())?,
        },
    };
    let mut cell = public_claim(&source.workbook, &sheet.name, &operand.cell, key, value);
    // The label states attributable basis but contains no numeric ownership share. Preserve that distinction
    // from the registered share used in the key; removing the registry row must remove the supplied basis.
    cell.printed_basis = OwnershipBasis::Unprinted;
    if attributable && let Some(share) = registered_share {
        cell.basis_evidence = Some(format!(
            "{}; {}:{}!A{}",
            share.evidence, source.file_name, sheet.name, row
        ));
    }
    cell.cite_status(profile);
    Ok(cell)
}

const COPRODUCT_RATE_LABEL: &str = "total cash costs per ounce (on a co-product basis)";

fn coproduct_rate_rows(sheet: &Sheet) -> BTreeSet<u32> {
    sheet
        .cells_by_coord
        .keys()
        .filter_map(|&(row, col)| {
            (col == 1
                && cell_text(sheet, row, col)
                    .is_some_and(|label| label.eq_ignore_ascii_case(COPRODUCT_RATE_LABEL)))
            .then_some(row)
        })
        .collect()
}

fn aisc_key_rows(table: &AiscTable) -> BTreeSet<u32> {
    table
        .block1
        .iter()
        .chain(&table.block2)
        .copied()
        .chain([
            table.tcc_row,
            table.aisc_row,
            table.ounces_row,
            table.cos_rate_row,
            table.tcc_rate_row,
            table.aisc_rate_row,
        ])
        .collect()
}

/// Evidence for the net AISC reconciliation's attributable-ounce denominator (BD-333).
/// Individual gross-cost and NCI lines retain their own printed Claims and are not relabelled.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BridgeBasisEvidence {
    pub basis: OwnershipBasis,
    pub evidence: Vec<String>,
}

/// Confirms the bridge's basis from the current reconciliation and same-release Mine Stats summary.
/// The caller supplies the physical input's already source-bound ownership basis and separately verifies
/// the signed component footing. An absent share, mismatching label or duplicate source refuses.
pub fn bridge_basis(
    profile: &IssuerProfile,
    sources: &[LoadedSource],
    source: &LoadedSource,
    sheet: &Sheet,
    table: &AiscTable,
    subject: &str,
    physical_basis: OwnershipBasis,
) -> Result<BridgeBasisEvidence, String> {
    if source.kind != SourceKind::AiscReconciliation || profile.regions.iter().any(|s| s == subject)
    {
        return Err("the bridge requires a gold-site AISC reconciliation".to_string());
    }
    let title = cell_text(sheet, 1, 1).unwrap_or_default().to_lowercase();
    let ounces = cell_text(sheet, table.ounces_row, 1)
        .unwrap_or_default()
        .to_lowercase();
    if !title.contains("reconciliation of gold cost of sales")
        || !title.contains("all-in sustaining costs")
        || !ounces.starts_with("ounces sold")
        || !ounces.contains("attributable basis")
        || !(ounces.contains("000s ounces") || ounces.contains("thousands of ounces"))
    {
        return Err(
            "the AISC title or attributable thousand-ounce denominator is not stated".to_string(),
        );
    }
    let (num, den) = match physical_basis {
        OwnershipBasis::Attributable { num, den, .. } if num > 0 && num <= den => (num, den),
        OwnershipBasis::Consolidated100 => (1, 1),
        _ => return Err("the physical input has no explicit ownership share".to_string()),
    };
    let share = profile.shares.iter().find(|s| s.subject == subject);
    if share
        .is_some_and(|s| i128::from(s.num) * i128::from(den) != i128::from(num) * i128::from(s.den))
    {
        return Err(
            "the registered share differs from the physical input's printed share".to_string(),
        );
    }
    let candidates: Vec<_> = sources
        .iter()
        .filter(|s| s.kind == SourceKind::MineStats && s.quarter == source.quarter)
        .collect();
    let [stats] = candidates.as_slice() else {
        return Err(
            "exactly one same-release Mine Stats source is required for the basis".to_string(),
        );
    };
    let summary = find_qtd_sheet(&stats.workbook.sheets, &source.quarter)?;
    let label = share.map_or(subject, |s| s.label.as_str());
    let at = confirm_share_label(summary, label, num, den)?;
    Ok(BridgeBasisEvidence {
        basis: physical_basis,
        evidence: vec![
            format!("{}:{}!A1", source.file_name, sheet.name),
            format!("{}:{}!A{}", source.file_name, sheet.name, table.ounces_row),
            format!("{}:{}!{at}", stats.file_name, summary.name),
        ],
    })
}

fn aisc_key_label(sheet: &Sheet, table: &AiscTable, row: u32) -> Result<String, String> {
    // A canonical anchor name makes an abbreviated rate label or a changing unit suffix the same metric.
    for (r, anchor) in [
        table.cos_row,
        table.tcc_row,
        table.aisc_row,
        table.ounces_row,
        table.cos_rate_row,
        table.tcc_rate_row,
        table.aisc_rate_row,
    ]
    .into_iter()
    .zip(ROW_ANCHORS)
    {
        if row == r {
            return Ok(anchor.names[0].to_string());
        }
    }
    cell_text(sheet, row, 1)
        .filter(|label| !label.is_empty())
        .map(|label| label.to_lowercase())
        .ok_or_else(|| "the operand's metric label is missing".to_string())
}

/// The operand at `(row, col)` of `sheet` in `file`.
#[must_use]
pub fn operand_at(file: &str, sheet: &Sheet, row: u32, col: u32, role: OperandRole) -> Operand {
    let value = match sheet.cell(row, col) {
        None => OperandValue::Blank,
        Some(c) => match &c.value {
            // The displayed whole number is what the table prints and what the identity sums.
            CellValue::Number(x) => as_int(number_val(c)).map_or_else(
                || OperandValue::Unrounded(c.number_text.clone().unwrap_or_else(|| x.to_string())),
                OperandValue::Int,
            ),
            CellValue::Placeholder(t) | CellValue::Text(t) | CellValue::FormulaError(t) => {
                OperandValue::Token(t.clone())
            }
        },
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
/// of this sheet); the registered share's label cell is in another workbook and is added by the caller. A region's
/// such rate also reads each printed member's rate and ounces (`members`, BD-331.S4).
#[allow(clippy::too_many_arguments)]
fn family_operands(
    file: &str,
    sheet: &Sheet,
    t: &AiscTable,
    col: u32,
    family: IdentityFamily,
    all_zero: bool,
    footnote: Option<&str>,
    members: &[(String, u32)],
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
                .any(|&r| non_zero_number(val(sheet, r, col)));
            if let Some((r, c)) = footnote.filter(|_| nci).and_then(parse_a1_ref) {
                out.push(operand_at(file, sheet, r, c, R::LabelCheck));
            }
            for &(_, m) in members.iter().filter(|_| nci) {
                out.push(operand_at(file, sheet, t.cos_rate_row, m, R::MemberRate));
                out.push(operand_at(file, sheet, t.ounces_row, m, R::MemberOunces));
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

/// Why a numeric cell has no displayed whole number for the declared construction (bd-jg43.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Undisplayable {
    /// General (no format) with a non-integer stored value: what it displays depends on the column width.
    GeneralNonInteger,
    /// A format the interpreter refuses (date, scientific, conditional, an unknown id, …) or a bad stored text.
    UnsupportedFormat,
    /// The format shows decimals or a percentage: not a whole displayed number.
    DecimalsShown,
}

impl Undisplayable {
    const fn reason(self) -> RefusedReason {
        match self {
            Self::GeneralNonInteger => RefusedReason::GeneralNonInteger,
            Self::UnsupportedFormat => RefusedReason::UnsupportedFormat,
            Self::DecimalsShown => RefusedReason::DecimalsShown,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Val {
    Blank,
    Place,
    /// A number as displayed under its cell's format: always a whole number below 2^53.
    Num(f64),
    /// A number is printed, but not as a whole displayed number; never scored.
    Undisplayable(Undisplayable),
    /// Text or a formula error: present but not a number.
    Other,
}

/// The table's construction sums the numbers as printed (E815), so a numeric cell enters as its displayed value
/// (bd-jg43.2's interpreter over the stored decimal text), never as the stored double: a stored -37.899643
/// under `#,##0` enters as -38 (E823).
fn number_val(cell: &ExtractedCell) -> Val {
    const LIMIT: u128 = 1 << 53;
    match cell.displayed() {
        // A zero section's token (the dash under `"-"`) stands for 0; BD-332 keeps the token itself (E497).
        Some(Ok(d)) if d.token.is_some() => Val::Num(0.0),
        Some(Ok(d)) if d.d == 0 && !d.percent && d.scaled.unsigned_abs() < LIMIT => {
            Val::Num(d.scaled as f64)
        }
        Some(Ok(d)) if d.d > 0 || d.percent => Val::Undisplayable(Undisplayable::DecimalsShown),
        Some(Err(r)) if r.code == numfmt::GENERAL_NON_INTEGER => {
            Val::Undisplayable(Undisplayable::GeneralNonInteger)
        }
        _ => Val::Undisplayable(Undisplayable::UnsupportedFormat),
    }
}

fn val(sheet: &Sheet, row: u32, col: u32) -> Val {
    match sheet.cell(row, col) {
        None => Val::Blank,
        Some(c) => match &c.value {
            CellValue::Placeholder(_) => Val::Place,
            CellValue::Number(_) => number_val(c),
            CellValue::Text(_) | CellValue::FormulaError(_) => Val::Other,
        },
    }
}

/// A number is printed in the cell, whether or not it displays as a whole number.
const fn printed_number(v: Val) -> bool {
    matches!(v, Val::Num(_) | Val::Undisplayable(_))
}

/// A printed number that is not zero, or one whose displayed value is unknown (counted as possibly non-zero).
fn non_zero_number(v: Val) -> bool {
    matches!(v, Val::Undisplayable(_)) || matches!(v, Val::Num(x) if x != 0.0)
}

/// The reason for the first operand with no whole displayed number; `UnsupportedFormat` if none is found.
fn undisplayable_reason(vals: impl IntoIterator<Item = Val>) -> RefusedReason {
    vals.into_iter()
        .find_map(|v| match v {
            Val::Undisplayable(u) => Some(u.reason()),
            _ => None,
        })
        .unwrap_or(RefusedReason::UnsupportedFormat)
}

/// (v) for the first of `rows` with no whole displayed number, naming its cell and format code (`at`).
fn undisplayable_state(
    rows: &[u32],
    v: &dyn Fn(u32) -> Val,
    at: &dyn Fn(u32) -> String,
) -> IdentityState {
    rows.iter()
        .find_map(|&r| match v(r) {
            Val::Undisplayable(u) => Some(IdentityState::RefusedAt {
                reason: u.reason(),
                at: at(r),
            }),
            _ => None,
        })
        .unwrap_or(IdentityState::Refused {
            reason: RefusedReason::UnsupportedFormat,
        })
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

// The exact kernel binding (BD-331.S6). Every arithmetic decision of the ledger is a run of BD-340's kernel
// (`fmine_numerics::exact_feas`) under the `Closed` tie convention, whose own checkers re-verify each witness and
// certificate before it is reported. The posing is the one this module always used:
// - a footing is its printed total and lines under `ComponentSum`: the printed lines must sum to the printed total
//   (otherwise a residual certificate). A placeholder line is an exact 0 (a zero step), so it widens nothing;
// - its alternative (subtotal rounded once) is the same sum under `RoundedOnce`, where TCC, as a line of the AISC
//   footing, stands for its k₁ numeric block-1 lines: TCC ± k₁/2;
// - a per-ounce rate is R = 1000·s·N/Q with N = n ± k/2 (k numeric lines summed into N; N is exact when k = 0),
//   Q = q ± ½ and R = r ± ½, at the share s (1 at 100%).

/// Why the exact kernel made no claim on an instance: a cap, an overflow, a refusal of the posed input, or a checker
/// rejection. The instance is refused, never decided.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KernelIndeterminate(pub String);

/// One printed whole number: step 1 for a rounded figure, 0 for an exact one (the 0 a placeholder stands for).
fn xf_cell(locator: &str, printed: i64, step: i64) -> xf::Cell {
    xf::Cell {
        locator: locator.to_string(),
        sha256: String::new(),
        token: printed.to_string(),
        printed: Rat::from_integer(i128::from(printed)),
        step: Rat::from_integer(i128::from(step)),
        scope: xf::Scope::Visible,
    }
}

/// One connected problem: `Ok(None)` when feasible, `Ok(Some(certificate))` when infeasible.
fn xf_decide(
    cells: &[xf::Cell],
    identities: &[xf::Identity],
    rules: &xf::Rules,
) -> Result<Option<xf::Certificate>, KernelIndeterminate> {
    let components =
        xf::solve(cells, identities, rules).map_err(|e| KernelIndeterminate(e.to_string()))?;
    let [component] = components.as_slice() else {
        return Err(KernelIndeterminate(format!(
            "{} components where one was posed",
            components.len()
        )));
    };
    match &component.outcome {
        xf::Outcome::Feasible { .. } => Ok(None),
        xf::Outcome::Infeasible { certificate } => Ok(Some(certificate.clone())),
        xf::Outcome::NotRefuted => Err(KernelIndeterminate("not refuted".to_string())),
        xf::Outcome::Indeterminate { reason } => Err(KernelIndeterminate(reason.clone())),
    }
}

/// The footing `total = Σ lines` under `construction`, each line `(locator, printed, step)`.
fn footing_decision(
    total: (&str, i64),
    lines: &[(String, i64, i64)],
    construction: xf::Construction,
) -> Result<Option<xf::Certificate>, KernelIndeterminate> {
    let mut cells = vec![xf_cell(total.0, total.1, 1)];
    cells.extend(lines.iter().map(|(l, v, step)| xf_cell(l, *v, *step)));
    let identity = xf::Identity {
        id: "footing".to_string(),
        family: "footing".to_string(),
        closure: xf::Closure::Definitional,
        kind: xf::IdentityKind::Sum {
            total: total.0.to_string(),
            lines: lines.iter().map(|(l, _, _)| l.clone()).collect(),
            signs: vec![1; lines.len()],
        },
    };
    let rules = xf::Rules::new(xf::Convention::Closed).with("footing", construction);
    xf_decide(&cells, &[identity], &rules)
}

/// The per-ounce rate R = 1000·s·N/Q as posed above, at the share s = `share.0/share.1`.
fn rate_posed(
    n: i64,
    k: i64,
    q: i64,
    r: i64,
    share: (i64, i64),
) -> Result<(Vec<xf::Cell>, Vec<xf::Identity>), KernelIndeterminate> {
    let factor = Rat::new(1000 * i128::from(share.0), i128::from(share.1))
        .map_err(|e| KernelIndeterminate(format!("share {}/{}: {e:?}", share.0, share.1)))?;
    let identity = xf::Identity {
        id: "rate".to_string(),
        family: "per_ounce".to_string(),
        closure: xf::Closure::MayUseUnpublished,
        kind: xf::IdentityKind::Ratio {
            rate: "R".to_string(),
            num: "N".to_string(),
            den: "Q".to_string(),
            factor,
        },
    };
    Ok((
        vec![xf_cell("N", n, k), xf_cell("Q", q, 1), xf_cell("R", r, 1)],
        vec![identity],
    ))
}

/// Whether some N ∈ [n − k/2, n + k/2], Q ∈ [q − ½, q + ½] and R ∈ [r − ½, r + ½] give R = 1000·s·N/Q, at the
/// share s = `share.0/share.1` (`(1, 1)` at 100%): a checked witness or a checked Farkas certificate.
pub fn rate_holds(
    n: i64,
    k: i64,
    q: i64,
    r: i64,
    share: (i64, i64),
) -> Result<bool, KernelIndeterminate> {
    let (cells, identities) = rate_posed(n, k, q, r, share)?;
    let rules = xf::Rules::new(xf::Convention::Closed);
    Ok(xf_decide(&cells, &identities, &rules)?.is_none())
}

/// The exact range of 1000·N/Q over N = n ± k/2, Q = q ± ½: BD-340's implied interval of the rate with its own box
/// dropped (Charnes–Cooper, each end re-decided by checked runs).
pub fn rate_range(n: i64, k: i64, q: i64) -> Result<xf::Interval, KernelIndeterminate> {
    let (cells, identities) = rate_posed(n, k, q, 0, (1, 1))?;
    let rules = xf::Rules::new(xf::Convention::Closed);
    xf::implied_interval(&cells, &identities, &rules, "R", true)
        .map_err(|e| KernelIndeterminate(format!("{e:?}")))
}

/// The printed whole-number rates `[lo, hi]` that [`rate_range`] admits: those x whose box x ± ½ meets it.
pub fn admitted_rates(n: i64, k: i64, q: i64) -> Result<(i64, i64), KernelIndeterminate> {
    let iv = rate_range(n, k, q)?;
    let half = Rat::new(1, 2).map_err(|e| KernelIndeterminate(format!("{e:?}")))?;
    let overflow = |e| KernelIndeterminate(format!("{e:?}"));
    // x + ½ reaches lo (strictly past it at an open end), and x − ½ reaches hi.
    let lo = iv.lo.checked_sub(half).map_err(overflow)?;
    let hi = iv.hi.checked_add(half).map_err(overflow)?;
    let lo = if iv.lo_strict {
        floor(lo) + 1
    } else {
        -floor(Rat::from_integer(0).checked_sub(lo).map_err(overflow)?)
    };
    let hi = if iv.hi_strict {
        -floor(Rat::from_integer(0).checked_sub(hi).map_err(overflow)?) - 1
    } else {
        floor(hi)
    };
    let fit = |x: i128| i64::try_from(x).map_err(|e| KernelIndeterminate(format!("{e}")));
    Ok((fit(lo)?, fit(hi)?))
}

/// `x` as a whole number, if it is one within `i64`.
fn whole(x: Rat) -> Option<i64> {
    if x.den() == 1 {
        i64::try_from(x.num()).ok()
    } else {
        None
    }
}

/// ⌊x⌋.
fn floor(x: Rat) -> i128 {
    let (num, den) = if x.den() < 0 {
        (-x.num(), -x.den())
    } else {
        (x.num(), x.den())
    };
    num.div_euclid(den)
}

/// [`rate_range`] as `[lo, hi]` to three places, rounded outward so the text contains the exact range (display
/// only).
pub(crate) fn implied_rate_text(n: i64, k: i64, q: i64) -> String {
    let milli = |x: Rat, up: bool| -> Option<String> {
        let thousand = Rat::from_integer(1000);
        let scaled = x.checked_mul(thousand).ok()?;
        let m = if up {
            -floor(Rat::from_integer(0).checked_sub(scaled).ok()?)
        } else {
            floor(scaled)
        };
        let sign = if m < 0 { "-" } else { "" };
        let m = m.abs();
        Some(format!("{sign}{}.{:03}", m / 1000, m % 1000))
    };
    match rate_range(n, k, q) {
        Ok(iv) => match (milli(iv.lo, false), milli(iv.hi, true)) {
            (Some(lo), Some(hi)) => format!("[{lo}, {hi}]"),
            _ => "[no range: overflow]".to_string(),
        },
        Err(KernelIndeterminate(why)) => format!("[no range: {why}]"),
    }
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
        Some(ShareBasis::Region { .. }) => return refused(RefusedReason::RegionalNumerator),
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
    let at = |k| rate_holds(miss.n, k, miss.q, miss.r, (num, den));
    match (at(miss.k), at(1)) {
        (Err(_), _) | (_, Err(_)) => refused(RefusedReason::KernelIndeterminate),
        (Ok(true), Ok(alternate_consistent)) => IdentityState::Consistent {
            alternate_consistent,
            basis: Some(ConsistentBasis::Share(share)),
        },
        (Ok(false), Ok(_)) => IdentityState::Undecidable {
            why: Undecided::ShareOutside {
                miss,
                share,
                s_interval,
            },
            missing_fact: SHARE_MISSING_FACT.to_string(),
            closing_document: None,
        },
    }
}

/// What a region's (iv) leaves open (BD-331.S4): prose, no digits.
pub const REGION_MISSING_FACT: &str = "each member's attributable cost of sales and ounces sold behind the \
region's figure, including members that sold nothing or that the table does not print";

/// A region member's rate or ounces cell as the regional rule reads it (BD-331.S4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MemberVal {
    /// A whole displayed number.
    Whole(i64),
    /// A numeric zero its format shows as a token (the dash of a zero section).
    ZeroToken,
    /// A text placeholder.
    Place,
    Other,
}

impl MemberVal {
    fn read(sheet: &Sheet, row: u32, col: u32) -> Self {
        let Some(c) = sheet.cell(row, col) else {
            return Self::Other;
        };
        match &c.value {
            CellValue::Placeholder(_) => Self::Place,
            CellValue::Number(_) if matches!(c.displayed(), Some(Ok(d)) if d.token.is_some()) => {
                Self::ZeroToken
            }
            CellValue::Number(_) => as_int(number_val(c)).map_or(Self::Other, Self::Whole),
            CellValue::Text(_) | CellValue::FormulaError(_) => Self::Other,
        }
    }

    const fn is_zero(self) -> bool {
        matches!(self, Self::ZeroToken | Self::Whole(0))
    }
}

/// A region's cost of sales per ounce against its registered members (BD-331.S4), on a sheet whose footnote
/// states the rate on an attributable basis. Each printed member is weighed when its rate and ounces are whole
/// numbers with ounces not zero, and left out as having sold nothing when both are zero or both placeholders;
/// anything else refuses, naming the cell (the rate's if it is not whole, else the ounces'). The weighed members'
/// ounces must sum to the region's (the declared component sum). Then the McCormick relaxation of the weighted
/// mean: a checked certificate is (iv). Where it does not refute, the exact decision runs: weights reaching the
/// rate, their witness checked, are (i) `MembersExact`; none is (iv) ([`EXACT_REGION_REFUTATION`]); no answer
/// (overflow, more than 16 members) leaves (i) `relaxation_only`. The alternative construction (the region's
/// ounces rounded once) is reported, never scored.
fn regional_state(
    sheet: &Sheet,
    t: &AiscTable,
    col: u32,
    region: &RegionColumns,
    miss: RateMiss,
) -> IdentityState {
    let at = |row: u32, c: u32| format!("{}{row}", col_index_to_letters(c));
    let refused_at = |reason, at: String| IdentityState::RefusedAt { reason, at };
    if let Some(header) = &region.ambiguous {
        return refused_at(RefusedReason::RegionMemberOperand, header.clone());
    }
    let mut weighting = MemberWeighting {
        used: Vec::new(),
        excluded: Vec::new(),
        absent: region.absent.clone(),
    };
    for (subject, m) in &region.members {
        let (rate_cell, ounces_cell) = (at(t.cos_rate_row, *m), at(t.ounces_row, *m));
        match (
            MemberVal::read(sheet, t.cos_rate_row, *m),
            MemberVal::read(sheet, t.ounces_row, *m),
        ) {
            (MemberVal::Whole(rate), MemberVal::Whole(ounces)) if ounces != 0 => {
                if ounces < 0 {
                    return refused_at(RefusedReason::RegionMemberOperand, ounces_cell);
                }
                weighting.used.push(WeightedMember {
                    subject: subject.clone(),
                    rate_cell,
                    rate,
                    ounces_cell,
                    ounces,
                });
            }
            (r, o)
                if (r.is_zero() && o.is_zero())
                    || (r, o) == (MemberVal::Place, MemberVal::Place) =>
            {
                weighting.excluded.push(subject.clone());
            }
            (MemberVal::Whole(_), _) => {
                return refused_at(RefusedReason::RegionMemberOperand, ounces_cell);
            }
            _ => return refused_at(RefusedReason::RegionMemberOperand, rate_cell),
        }
    }
    if weighting.used.is_empty() {
        return IdentityState::Refused {
            reason: RefusedReason::RegionNoMembers,
        };
    }
    match weighting
        .used
        .iter()
        .try_fold(0_i64, |sum, m| sum.checked_add(m.ounces))
    {
        Some(sum) if sum == miss.q => {}
        Some(_) => return refused_at(RefusedReason::RegionOuncesDoNotFoot, at(t.ounces_row, col)),
        None => {
            return IdentityState::Refused {
                reason: RefusedReason::KernelIndeterminate,
            };
        }
    }
    match region_decision(
        &at(t.cos_rate_row, col),
        &at(t.ounces_row, col),
        miss,
        &weighting,
    ) {
        Ok((Some(certificate), _)) => IdentityState::Undecidable {
            why: Undecided::RegionOutsideMembers {
                weighting,
                certificate,
            },
            missing_fact: REGION_MISSING_FACT.to_string(),
            closing_document: None,
        },
        Ok((None, alternate_consistent)) => match exact_region_holds(miss.r, &weighting) {
            Some(true) => IdentityState::Consistent {
                alternate_consistent,
                basis: Some(ConsistentBasis::MembersExact(weighting)),
            },
            Some(false) => IdentityState::Undecidable {
                why: Undecided::RegionOutsideMembers {
                    weighting,
                    certificate: EXACT_REGION_REFUTATION.to_string(),
                },
                missing_fact: REGION_MISSING_FACT.to_string(),
                closing_document: None,
            },
            None => IdentityState::Consistent {
                alternate_consistent,
                basis: Some(ConsistentBasis::Members(weighting)),
            },
        },
        Err(_) => IdentityState::Refused {
            reason: RefusedReason::KernelIndeterminate,
        },
    }
}

/// The `certificate` of a region the exact decision refutes where the McCormick relaxation does not (BD-331.S4).
pub const EXACT_REGION_REFUTATION: &str = "exact_weighted_mean";

/// The exact decision on a region's printed rate `r` against its weighed members, every printed value a whole
/// number standing for its closed box ±½, the members' ounces free of the region's (the component sum):
/// `Some(true)` with a witness checked exactly, `Some(false)` when no weights reach the rate, `None` on overflow
/// or more than 16 members.
fn exact_region_holds(r: i64, weighting: &MemberWeighting) -> Option<bool> {
    let half = Rat::from_integer(1)
        .checked_div(Rat::from_integer(2))
        .ok()?;
    let closed = |v: i64| -> Option<crate::mine_ledger::Interval> {
        let v = Rat::from_integer(i128::from(v));
        Some((v.checked_sub(half).ok()?, v.checked_add(half).ok()?))
    };
    let parts = weighting
        .used
        .iter()
        .map(|m| Some((closed(m.rate)?, closed(m.ounces)?)))
        .collect::<Option<Vec<_>>>()?;
    crate::mine_ledger::weighted_mean_holds(closed(r)?, &parts)
}

/// The kernel's verdict on a region against its weighed members: under the component sum, the digest of a checked
/// refutation or `None` when not refuted; and whether the rounded-once alternative (the region's ounces kept in
/// their own printed box) is not refuted either. No rate is posed with a numerator: the table prints cost of sales
/// on a consolidated basis, never the attributable numerator the rate divides, so the check is the weighted mean,
/// which can refute but never give a witness.
fn region_decision(
    rate_cell: &str,
    ounces_cell: &str,
    miss: RateMiss,
    weighting: &MemberWeighting,
) -> Result<(Option<String>, bool), KernelIndeterminate> {
    const FAMILY: &str = "region_ounces";
    let mut cells = vec![
        xf_cell(rate_cell, miss.r, 1),
        xf_cell(ounces_cell, miss.q, 1),
    ];
    let mut members = Vec::new();
    for m in &weighting.used {
        cells.push(xf_cell(&m.rate_cell, m.rate, 1));
        cells.push(xf_cell(&m.ounces_cell, m.ounces, 1));
        members.push(xf::RateCells {
            rate: m.rate_cell.clone(),
            weight: m.ounces_cell.clone(),
            numerator: None,
        });
    }
    let region = xf::RateCells {
        rate: rate_cell.to_string(),
        weight: ounces_cell.to_string(),
        numerator: None,
    };
    let identities = xf::region_against_members(
        "region",
        FAMILY,
        xf::Closure::MayUseUnpublished,
        Rat::from_integer(1000),
        &region,
        &members,
    );
    let run = |construction| -> Result<(xf::Rules, xf::Outcome), KernelIndeterminate> {
        let rules = xf::Rules::new(xf::Convention::Closed).with(FAMILY, construction);
        let components = xf::solve(&cells, &identities, &rules)
            .map_err(|e| KernelIndeterminate(e.to_string()))?;
        let [component] = components.as_slice() else {
            return Err(KernelIndeterminate(format!(
                "{} components where one was posed",
                components.len()
            )));
        };
        Ok((rules, component.outcome.clone()))
    };
    let refuted = |rules: &xf::Rules, outcome: xf::Outcome| match outcome {
        xf::Outcome::Infeasible { certificate } => {
            xf::certificate_digest(&cells, &identities, rules, &certificate)
                .map(Some)
                .map_err(|e| KernelIndeterminate(format!("{e:?}")))
        }
        xf::Outcome::NotRefuted => Ok(None),
        xf::Outcome::Feasible { .. } => Err(KernelIndeterminate(
            "a weighted mean was reported feasible".to_string(),
        )),
        xf::Outcome::Indeterminate { reason } => Err(KernelIndeterminate(reason)),
    };
    let (rules, outcome) = run(xf::Construction::ComponentSum)?;
    let primary = refuted(&rules, outcome)?;
    let (rules, outcome) = run(xf::Construction::RoundedOnce)?;
    let alternate_consistent = refuted(&rules, outcome)?.is_none();
    Ok((primary, alternate_consistent))
}

/// Whether adding a registered component c (printed, so c ± ½) to the numerator closes the rate: [`rate_holds`]
/// at 100% for N = (n + c) ± (k + 1)/2.
pub fn reconstruction_holds(miss: RateMiss, c: i64) -> Result<bool, KernelIndeterminate> {
    let Some(n) = miss.n.checked_add(c) else {
        return Err(KernelIndeterminate(
            "numerator plus component overflows".to_string(),
        ));
    };
    rate_holds(n, miss.k + 1, miss.q, miss.r, (1, 1))
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
        let region = matches!(share, Some(ShareBasis::Region { .. }));
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
    if !printed_number(vr) {
        return refused("rate not published");
    }
    if !printed_number(vq) {
        return refused("ounces not numeric");
    }
    if vr == Val::Num(0.0) || vq == Val::Num(0.0) {
        return refused("zero rate or ounces");
    }
    if !printed_number(vn) {
        return refused("cost of sales not numeric");
    }
    let (Some(n), Some(q), Some(r)) = (as_int(vn), as_int(vq), as_int(vr)) else {
        return refused(undisplayable_reason([vn, vq, vr]).why());
    };
    if n < 1 || q < 1 || r < 1 {
        return refused("non-positive operand");
    }
    let at = |share| rate_holds(n, 1, q, r, share);
    let (Ok(at_full), Ok(at_share)) = (at((1, 1)), site_share.map(at).transpose()) else {
        return refused("exact kernel made no claim");
    };
    let nci = t.nci_rows.iter().any(|&row| non_zero_number(v(row)));
    let unbased = |reason: &'static str| OneCellOutcome::Refused {
        reason,
        at_full: Some(at_full),
        at_share,
    };
    let basis = if !nci {
        (1, 1)
    } else {
        match share {
            Some(ShareBasis::Region { .. }) => {
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
    let Ok(pass) = at(basis) else {
        return refused("exact kernel made no claim");
    };
    OneCellOutcome::Checked {
        basis,
        pass,
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
    let columns: Vec<(&str, u32, &str)> = header_cells
        .iter()
        .filter_map(|(col, header, cell)| {
            match_subject(header, subjects).map(|s| (s, *col, cell.as_str()))
        })
        .collect();
    let region_columns = |members: &[String]| {
        let mut out = RegionColumns::default();
        for m in members {
            let mut found = columns.iter().filter(|(s, _, _)| s == m);
            match (found.next(), found.next()) {
                (None, _) => out.absent.push(m.clone()),
                (Some(&(_, c, _)), None) => out.members.push((m.clone(), c)),
                (Some(_), Some(&(_, _, cell))) => out.ambiguous = Some(cell.to_string()),
            }
        }
        out
    };
    for (col, header, header_ref) in header_cells.iter().cloned() {
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
        let share = shares.get(subject);
        let region = match share {
            Some(ShareBasis::Region { members }) if !members.is_empty() => {
                Some(region_columns(members))
            }
            _ => None,
        };
        let member_cols = region.as_ref().map_or(&[][..], |r| r.members.as_slice());
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
                    member_cols,
                ),
                calc,
            });
        };
        let stated = ShareInputs {
            footnote: footnote.as_deref(),
            share,
            region: region.as_ref(),
        };
        column_ledger(sheet, table, col, subject, &stated, explain, &mut push);
    }
}

/// The stated-share inputs of one column: the sheet's attributable-basis footnote, the subject's share and, for a
/// region with registered members, where they print on this sheet.
struct ShareInputs<'a> {
    footnote: Option<&'a str>,
    share: Option<&'a ShareBasis>,
    region: Option<&'a RegionColumns>,
}

/// A region's registered members on one sheet (BD-331.S4): each printed member's column, the members the sheet
/// does not print, and the header cell of a member printed twice (which refuses the region).
#[derive(Debug, Default)]
struct RegionColumns {
    members: Vec<(String, u32)>,
    absent: Vec<String>,
    ambiguous: Option<String>,
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
    // A refused operand by cell and format code, e.g. `N13 (m/d/yyyy)` (bd-jg43.3).
    let format_at = |row: u32| {
        let code = sheet
            .cell(row, col)
            .map(|c| c.format.code())
            .unwrap_or_default();
        format!("{} ({code})", cell_ref(row))
    };
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
        let (state, calc) = footing_state(
            &v, &cell_ref, &format_at, total, lines, family, &t.block1, &all_lines,
        );
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
        let state = if !printed_number(rv) {
            IdentityState::NotPublished {
                reason: NotPublishedReason::RateNotPublished,
            }
        } else if !printed_number(q) {
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
                undisplayable_state(&cells, &v, &format_at)
            } else if as_int(v(numerator_row)).is_none() {
                refused(RefusedReason::NumeratorPlaceholder)
            } else {
                let k = lines.iter().filter(|&&r| printed_number(v(r))).count() as i64;
                let miss = RateMiss {
                    n: line_value(v(numerator_row)),
                    k,
                    q: line_value(q),
                    r: line_value(rv),
                };
                calc = Calc::Rate(miss);
                let nci = t.nci_rows.iter().any(|&r| non_zero_number(v(r)));
                let at = |k| rate_holds(miss.n, k, miss.q, miss.r, (1, 1));
                if family == IdentityFamily::CosPerOunce && nci {
                    match (stated.footnote, stated.region) {
                        (Some(_), Some(region)) => regional_state(sheet, t, col, region, miss),
                        _ => stated_share_state(&sheet.name, stated.footnote, stated.share, miss),
                    }
                } else {
                    match (at(miss.k), at(1)) {
                        (Err(_), _) | (_, Err(_)) => refused(RefusedReason::KernelIndeterminate),
                        (Ok(true), Ok(alternate_consistent)) => IdentityState::Consistent {
                            alternate_consistent,
                            basis: None,
                        },
                        (Ok(false), Ok(_)) => explain(subject, family, miss).unwrap_or_else(|| {
                            IdentityState::Undecidable {
                                why: Undecided::RateOutside(miss),
                                missing_fact: DEFAULT_MISSING_FACT.to_string(),
                                closing_document: None,
                            }
                        }),
                    }
                }
            }
        };
        push(family, rate_row, state, calc);
    }
}

#[allow(clippy::too_many_arguments, reason = "eight footing-check inputs")]
fn footing_state(
    v: &dyn Fn(u32) -> Val,
    cell_ref: &dyn Fn(u32) -> String,
    format_at: &dyn Fn(u32) -> String,
    total: u32,
    lines: &[u32],
    family: IdentityFamily,
    block1: &[u32],
    all_lines: &[u32],
) -> (IdentityState, Calc) {
    if !printed_number(v(total)) {
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
    let operands: Vec<u32> = lines.iter().chain([&total]).copied().collect();
    if !operands.iter().all(|&r| int_or_place(v(r))) {
        return (undisplayable_state(&operands, v, format_at), Calc::None);
    }
    let sum: i64 = lines.iter().map(|&r| line_value(v(r))).sum();
    let printed = line_value(v(total));
    let alt_lines = if family == IdentityFamily::TccFooting {
        block1
    } else {
        all_lines
    };
    let k_alt = alt_lines.iter().filter(|&&r| printed_number(v(r))).count() as i64;
    let calc = Calc::Footing {
        printed,
        component_sum: sum,
        k_alt,
    };
    // Each line as the kernel takes it: a placeholder is an exact 0.
    let terms: Vec<(String, i64, i64)> = lines
        .iter()
        .map(|&r| {
            let step = i64::from(matches!(v(r), Val::Num(_)));
            (cell_ref(r), line_value(v(r)), step)
        })
        .collect();
    // The alternative: TCC, the first line of the AISC footing, stands for its k₁ numeric block-1 lines.
    let mut alt_terms = terms.clone();
    if family == IdentityFamily::AiscFooting
        && let Some(tcc) = alt_terms.first_mut()
    {
        tcc.2 = block1.iter().filter(|&&r| printed_number(v(r))).count() as i64;
    }
    let total_cell = cell_ref(total);
    let refused = IdentityState::Refused {
        reason: RefusedReason::KernelIndeterminate,
    };
    let decided = (
        footing_decision(
            (&total_cell, printed),
            &terms,
            xf::Construction::ComponentSum,
        ),
        footing_decision(
            (&total_cell, printed),
            &alt_terms,
            xf::Construction::RoundedOnce,
        ),
    );
    let (Ok(primary), Ok(alternate)) = decided else {
        return (refused, calc);
    };
    let alternate_consistent = alternate.is_none();
    let Some(certificate) = primary else {
        let state = IdentityState::Consistent {
            alternate_consistent,
            basis: None,
        };
        return (state, calc);
    };
    // A component-sum footing fails only on its residual: printed total − Σ printed lines.
    let xf::Certificate::Residual { residual, .. } = certificate else {
        return (refused, calc);
    };
    let Some(residual) = whole(residual) else {
        return (refused, calc);
    };
    // Flipping the sign of a line x moves the sum by −2x, so a single flip closes the footing when −2x = residual.
    let sign_flip_witness = lines
        .iter()
        .find(|&&r| {
            let x = line_value(v(r));
            x != 0 && -2 * x == residual
        })
        .map(|&r| cell_ref(r));
    let state = IdentityState::IssuerDiscrepancy(FootingCertificate {
        printed,
        component_sum: printed - residual,
        residual,
        total_cell,
        line_cells: lines.iter().map(|&r| cell_ref(r)).collect(),
        sign_flip_witness,
        alternate_consistent,
    });
    (state, calc)
}

/// Which rounding construction one footing family's decided totals fit (bd-4wvl.4, BD-340.S4 part 1). A report
/// over finished states: it changes none and is not in the state root.
#[derive(Debug, Clone, PartialEq)]
pub struct FootingDiscovery {
    pub family: IdentityFamily,
    /// Decided footing instances: states (i) and (iii).
    pub totals: usize,
    /// Of those, the ones whose printed total is the sum of its printed lines.
    pub foot_exact: usize,
    /// Over the distinct footings. A footing printed again with the same subject and the same numbers (a later
    /// release's comparative column) is the same draw and counts once.
    pub report: xf::discover::ConstructionReport,
}

/// [`FootingDiscovery`] for the TCC and AISC footings of `instances`, in that order. A footing's k is its numeric
/// lines (a placeholder line is an exact 0, never rounded); its residual is Σ lines − total.
pub fn footing_discovery(
    instances: &[IdentityInstance],
) -> Result<Vec<FootingDiscovery>, fmine_units::UnitsError> {
    let mut out = Vec::new();
    for family in [IdentityFamily::TccFooting, IdentityFamily::AiscFooting] {
        let (mut totals, mut foot_exact) = (0, 0);
        let mut seen = BTreeSet::new();
        let mut footings = Vec::new();
        for i in instances.iter().filter(|i| i.family == family) {
            let Calc::Footing {
                printed,
                component_sum,
                ..
            } = i.calc
            else {
                continue;
            };
            if !matches!(
                i.state,
                IdentityState::Consistent { .. } | IdentityState::IssuerDiscrepancy(_)
            ) {
                continue;
            }
            totals += 1;
            if component_sum == printed {
                foot_exact += 1;
            }
            let summed = |o: &&Operand| matches!(o.role, OperandRole::Total | OperandRole::Line);
            let values: Vec<Option<i64>> = i
                .operands
                .iter()
                .filter(summed)
                .map(|o| match o.value {
                    OperandValue::Int(x) => Some(x),
                    _ => None,
                })
                .collect();
            if !seen.insert((i.subject.clone(), values)) {
                continue;
            }
            let k = i
                .operands
                .iter()
                .filter(|o| o.role == OperandRole::Line && matches!(o.value, OperandValue::Int(_)))
                .count();
            footings.push(xf::discover::Footing {
                k: u32::try_from(k).unwrap_or(u32::MAX),
                residual: i128::from(component_sum) - i128::from(printed),
            });
        }
        out.push(FootingDiscovery {
            family,
            totals,
            foot_exact,
            report: xf::discover::discover_construction(&footings)?,
        });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Column K of a one-sheet workbook: `(row, stored text, number format id)`, formats listed as `custom`.
    fn styled_column(cells: &[(u32, &str, u32)], custom: &[(u32, &str)]) -> Sheet {
        use fmine_io::xlsx::{
            MinimalCell, MinimalSheet, MinimalStyles, Workbook, build_minimal_xlsx_styled,
        };
        let mut xfs: Vec<u32> = Vec::new();
        let cells = cells
            .iter()
            .map(|&(row, raw, fmt)| {
                let xf = xfs.iter().position(|&f| f == fmt).unwrap_or_else(|| {
                    xfs.push(fmt);
                    xfs.len() - 1
                });
                let raw = raw.to_string();
                (
                    format!("K{row}"),
                    MinimalCell::Styled { raw, xf: xf as u32 },
                )
            })
            .collect();
        let styles = MinimalStyles {
            num_fmts: custom.iter().map(|&(id, c)| (id, c.to_string())).collect(),
            xfs,
        };
        let bytes = build_minimal_xlsx_styled(
            &[MinimalSheet {
                name: "S".to_string(),
                hidden: false,
                cells,
            }],
            Some(&styles),
        );
        let wb = Workbook::from_bytes("t.xlsx", &bytes).expect("parses");
        wb.sheet("S").expect("sheet").clone()
    }

    /// TCC footing of column K (11) over lines 5–8 and the total at row 10, read through `val`.
    fn k_footing(sheet: &Sheet) -> (IdentityState, Calc) {
        let v = |r: u32| val(sheet, r, 11);
        let cell_ref = |r: u32| format!("K{r}");
        let format_at = |r: u32| {
            let code = sheet
                .cell(r, 11)
                .map(|c| c.format.code())
                .unwrap_or_default();
            format!("K{r} ({code})")
        };
        let lines = [5, 6, 7, 8];
        let family = IdentityFamily::TccFooting;
        footing_state(
            &v, &cell_ref, &format_at, 10, &lines, family, &lines, &lines,
        )
    }

    /// E823's quoted values under `#,##0` (id 3): 241 − 38 − 37.899643 − 33 = 132.100357 stored.
    fn e823_column() -> Sheet {
        styled_column(
            &[
                (5, "241", 3),
                (6, "-38", 3),
                (7, "-37.899643", 3),
                (8, "-33", 3),
                (10, "132.100357", 3),
            ],
            &[],
        )
    }

    /// bd-jg43.3: on displayed values E823's footing closes in integers, 241 − 38 − 38 − 33 = 132 against a
    /// printed 132, so it is (i) where the stored values were refused.
    #[test]
    fn test_e823_tcc_footing_fixture_scores_on_displayed() {
        let (state, calc) = k_footing(&e823_column());
        assert_eq!(
            state,
            IdentityState::Consistent {
                alternate_consistent: true,
                basis: None
            }
        );
        assert_eq!(
            calc,
            Calc::Footing {
                printed: 132,
                component_sum: 132,
                k_alt: 4
            }
        );
    }

    /// The E823 fixture tells the displayed reading from two wrong ones: the stored doubles (the ledger before
    /// bd-jg43.3) refuse it, and truncating instead of rounding (−37.899643 → −37) makes it a discrepancy.
    #[test]
    fn test_mutation_ledger_on_stored_value_fails_e823_fixture() {
        let sheet = e823_column();
        let stored = |r: u32| match sheet.cell(r, 11).map(|c| &c.value) {
            Some(CellValue::Number(x)) => Val::Num(*x),
            _ => Val::Blank,
        };
        let truncated = |r: u32| match sheet.cell(r, 11).map(|c| &c.value) {
            Some(CellValue::Number(x)) => Val::Num(x.trunc()),
            _ => Val::Blank,
        };
        let cell_ref = |r: u32| format!("K{r}");
        let lines = [5, 6, 7, 8];
        let family = IdentityFamily::TccFooting;
        let run = |v: &dyn Fn(u32) -> Val| {
            footing_state(v, &cell_ref, &cell_ref, 10, &lines, family, &lines, &lines).0
        };
        assert_eq!(run(&stored).code(), "v");
        assert_eq!(run(&truncated).code(), "iii");
        assert_eq!(k_footing(&sheet).0.code(), "i");
    }

    /// A zero line shown as the zero section's dash is the number 0 (BD-332 keeps the token itself): the footing
    /// sums it as 0 and counts it as a printed line.
    #[test]
    fn test_zero_token_counts_as_zero_in_footing() {
        let sheet = styled_column(
            &[
                (5, "300", 164),
                (6, "-50", 164),
                (7, "0", 164),
                (8, "-20", 164),
                (10, "230", 164),
            ],
            &[(164, "#,##0;(#,##0);\"-\"")],
        );
        assert_eq!(val(&sheet, 7, 11), Val::Num(0.0));
        let (state, calc) = k_footing(&sheet);
        assert_eq!(state.code(), "i");
        assert_eq!(
            calc,
            Calc::Footing {
                printed: 230,
                component_sum: 230,
                k_alt: 4
            }
        );
    }

    /// A line shown with a decimal (`#,##0.0`) is not a whole displayed number: the footing is refused, naming
    /// the cell and its format code.
    #[test]
    fn test_d_gt_0_operand_refused_with_reason() {
        let sheet = styled_column(
            &[
                (5, "300", 3),
                (6, "-50.4", 164),
                (7, "0", 3),
                (8, "-20", 3),
                (10, "230", 3),
            ],
            &[(164, "#,##0.0")],
        );
        let (state, calc) = k_footing(&sheet);
        assert_eq!(
            state,
            IdentityState::RefusedAt {
                reason: RefusedReason::DecimalsShown,
                at: "K6 (#,##0.0)".to_string(),
            }
        );
        assert_eq!(
            state.detail(),
            "decimals displayed; the declared construction sums whole displayed numbers: K6 (#,##0.0)"
        );
        assert_eq!(calc, Calc::None);
    }

    /// [`rate_holds`] at a share, decided.
    fn holds_at(n: i64, k: i64, q: i64, r: i64, share: (i64, i64)) -> bool {
        rate_holds(n, k, q, r, share).expect("the kernel decides")
    }

    /// [`rate_holds`] at 100%, decided.
    fn holds(n: i64, k: i64, q: i64, r: i64) -> bool {
        holds_at(n, k, q, r, (1, 1))
    }

    #[test]
    fn rate_interval_known_answers() {
        // 1000·79/68 = 1161.76…; with N ± 1/2 and Q ± 1/2 the range is [1145.985…, 1177.777…].
        assert!(holds(79, 1, 68, 1162));
        assert!(holds(79, 1, 68, 1146));
        assert!(!holds(79, 1, 68, 1141));
        assert_eq!(admitted_rates(79, 1, 68), Ok((1146, 1178)));
        // Outward rounding: 1177.777… prints as 1177.778 (it printed 1177.777 before 2026-10-29, below the bound).
        assert_eq!(implied_rate_text(79, 1, 68), "[1145.985, 1177.778]");
        // bd-sjrh.1, NEW-cell-number-formats' worked example: 129500/69.5 = 1863.3093… and 134500/68.5 =
        // 1963.5036…; 127500/69.5 = 1834.5323… and 138500/68.5 = 2021.8978….
        assert_eq!(implied_rate_text(132, 5, 69), "[1863.309, 1963.504]");
        assert_eq!(implied_rate_text(133, 11, 69), "[1834.532, 2021.898]");
        // Negative range: floor of −578.94… is −578.948; ceiling of −428.57… is −428.571.
        assert_eq!(implied_rate_text(-5, 1, 10), "[-578.948, -428.571]");
        // Negative numerators: 1000·(−5 ± ½)/(10 ± ½) spans [−578.9…, −428.5…].
        assert!(holds(-5, 1, 10, -500));
        assert!(!holds(-5, 1, 10, 500));
        // k widens the numerator: 1000·(100 ± 2)/(100 ± ½) reaches 1025.125…, which meets [1024.5, 1025.5]
        // but not [1026.5, 1027.5].
        assert!(holds(100, 4, 100, 1025));
        assert!(!holds(100, 4, 100, 1027));
        // k = 0 (every line a placeholder): N is exact. 1000·100/(100 ± ½) = [995.02…, 1005.02…] holds 1005
        // ([1004.5, 1005.5] meets it) and not 1006; with k = 1 the range reaches 1010.05… and holds 1006.
        assert!(holds(100, 0, 100, 1005));
        assert!(!holds(100, 0, 100, 1006));
        assert!(holds(100, 1, 100, 1006));
    }

    /// bd-prmn.8: the one-cell check and its implied ounces on hand-computed columns.
    #[test]
    fn one_cell_implied_ounces_known_answers() {
        // n = 100, q = 50, r = 2000: 1000·100/50 = 2000 exactly, inside.
        assert!(holds(100, 1, 50, 2000));
        // The implied ounces at 100%: [1000·199/4001, 1000·201/3999] = [49.7375…, 50.2625…], which holds 50.
        assert_eq!(
            implied_ounces(100, 2000, 1, 1),
            ("49.737".to_string(), "50.263".to_string())
        );
        // r = 2100: [1000·199/4201, 1000·201/4199] = [47.3696…, 47.8685…] misses [49.5, 50.5], so it fails.
        assert!(!holds(100, 1, 50, 2100));
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
        assert!(holds_at(100, 1, 36, 2100, (3, 4)));
        assert!(!holds_at(100, 1, 34, 2100, (3, 4)));
    }

    #[test]
    fn reconstruction_known_answers() {
        // N = 30 ± 1, Q = 20 ± ½, printed 2000 needs N ∈ [38.990…, 41.010…]: adding 10 ± ½ gives
        // [38.5, 41.5] and closes it; adding 5 ± ½ gives [33.5, 36.5] and does not.
        let miss = RateMiss {
            n: 30,
            k: 2,
            q: 20,
            r: 2000,
        };
        assert_eq!(reconstruction_holds(miss, 10), Ok(true));
        assert_eq!(reconstruction_holds(miss, 5), Ok(false));
        assert!(!holds(30, 2, 20, 2000));
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
        assert!(!holds_at(117, 1, 37, 2736, (21, 25)));
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
        assert!(holds_at(117, 1, 37, 2736, (85, 100)));
        assert!(holds_at(117, 1, 37, 2736, (88, 100)));
        assert!(!holds_at(117, 1, 37, 2736, (89, 100)));
    }

    /// The closed form against the kernel's share interval (bd-4wvl.4 part 3), which derives it from each
    /// factor's rounding box: r and q at step 1, N = n at step k (k summed lines), 1/1000 the constant.
    #[test]
    fn test_feasible_share_matches_kernel_share_interval() {
        use xf::discover::{Factor, share_interval};
        let at = |v: i64, step: i64| Factor {
            printed: Rat::from_integer(i128::from(v)),
            step: Rat::from_integer(i128::from(step)),
        };
        let mut compared = 0;
        for n in 1..=40 {
            for k in 0..=3 {
                for q in 1..=20 {
                    for r in [1, 7, 950, 1501, 2736] {
                        let Some(((ln, ld), (hn, hd))) = feasible_share(n, k, q, r) else {
                            continue;
                        };
                        let s = share_interval(
                            Rat::new(1, 1000).expect("1/1000"),
                            &[at(r, 1), at(q, 1)],
                            &[at(n, k)],
                            xf::Convention::Closed,
                        )
                        .expect("kernel interval");
                        let ends = (
                            s.lo.map(|e| (e.at, e.strict)),
                            s.hi.map(|e| (e.at, e.strict)),
                        );
                        let want = (
                            Some((Rat::new(ln, ld).expect("lo"), false)),
                            Some((Rat::new(hn, hd).expect("hi"), false)),
                        );
                        assert_eq!(ends, want, "n {n} k {k} q {q} r {r}");
                        compared += 1;
                    }
                }
            }
        }
        // 40·4·20·5 = 16,000 points, less (n, k) = (1, 2) and (1, 3) at 100 each, which have 2n − k < 1.
        assert_eq!(compared, 15_800, "every grid point with 2n − k ≥ 1");
    }

    #[test]
    fn test_scaled_rate_fit_synthetic() {
        // Synthetic Site H: 1000·¾·400/150 = 2000 fits; 2200 does not.
        assert!(holds_at(400, 1, 150, 2000, (3, 4)));
        assert!(!holds_at(400, 1, 150, 2200, (3, 4)));
        // 4399·299/(2000·801) = 0.821037…, 4401·301/(2000·799) = 0.828974…
        assert_eq!(
            feasible_share_text(400, 1, 150, 2200).as_deref(),
            Some("[0.8210, 0.8290]")
        );
        // At s = 1 the scaled test is the unscaled one.
        for (n, k, q, r) in [(79, 1, 68, 1162), (79, 1, 68, 1141), (200, 5, 100, 2400)] {
            assert_eq!(holds_at(n, k, q, r, (2, 2)), holds(n, k, q, r));
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
                    let (lo, hi) = (Rat::new(ln, ld).expect("lo"), Rat::new(hn, hd).expect("hi"));
                    for (a, b) in shares {
                        let s = Rat::new(i128::from(a), i128::from(b)).expect("share");
                        let inside = s.checked_cmp(lo).expect("cmp").is_ge()
                            && s.checked_cmp(hi).expect("cmp").is_le();
                        assert_eq!(
                            holds_at(n, 1, q, r, (a, b)),
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

    /// For a (iii) footing: BD-340.S3's conflict sets for the footing as the ledger poses it (from its cited
    /// operands; a placeholder is an exact 0), and the certificate's own cells, total first. Under the component
    /// sum, disregarding any one printed value drops the footing, so the one minimal set is every cell.
    fn footing_conflicts(inst: &IdentityInstance) -> (Vec<Vec<String>>, Vec<String>) {
        let IdentityState::IssuerDiscrepancy(c) = &inst.state else {
            panic!("{}: not (iii)", inst.cell)
        };
        let (mut cells, mut lines) = (Vec::new(), Vec::new());
        for o in &inst.operands {
            let (value, step) = match &o.value {
                OperandValue::Int(x) => (*x, 1),
                OperandValue::Token(_) => (0, 0),
                other => panic!("{}: {other:?}", o.cell),
            };
            match o.role {
                OperandRole::Total => cells.insert(0, xf_cell(&o.cell, value, step)),
                OperandRole::Line => {
                    lines.push(o.cell.clone());
                    cells.push(xf_cell(&o.cell, value, step));
                }
                _ => {}
            }
        }
        let identity = xf::Identity {
            id: "footing".to_string(),
            family: "footing".to_string(),
            closure: xf::Closure::Definitional,
            kind: xf::IdentityKind::Sum {
                total: c.total_cell.clone(),
                signs: vec![1; lines.len()],
                lines,
            },
        };
        let rules =
            xf::Rules::new(xf::Convention::Closed).with("footing", xf::Construction::ComponentSum);
        let found = xf::blame(&cells, std::slice::from_ref(&identity), &rules).expect("blames");
        let [component] = found.as_slice() else {
            panic!("{}: {found:?}", inst.cell)
        };
        let xf::BlameOutcome::Blamed(b) = &component.outcome else {
            panic!("{}: {:?}", inst.cell, component.outcome)
        };
        for set in &b.conflicts {
            let checked =
                xf::check_conflict_set(&cells, std::slice::from_ref(&identity), &rules, set);
            assert!(checked.is_ok(), "{}: {checked:?}", inst.cell);
        }
        let mut own: Vec<String> = std::iter::once(c.total_cell.clone())
            .chain(c.line_cells.iter().cloned())
            .collect();
        own.sort();
        (b.conflicts.iter().map(|s| s.cells.clone()).collect(), own)
    }

    /// (iii) carries S3's minimal set and a witness derived from the residual certificate: synthetic C8 prints 350
    /// against 250, and flipping C6 (−50) closes it.
    #[test]
    fn test_sign_flip_witness_from_certificate_c8_c6() {
        let run = crate::run_synthetic_assurance(&[]).expect("synthetic run");
        let discrepancies: Vec<&IdentityInstance> = run
            .instances
            .iter()
            .chain(&run.hidden_scope.instances)
            .filter(|i| matches!(i.state, IdentityState::IssuerDiscrepancy(_)))
            .collect();
        assert!(!discrepancies.is_empty());
        for inst in &discrepancies {
            let (conflicts, own) = footing_conflicts(inst);
            assert_eq!(conflicts, vec![own], "{}", inst.cell);
        }
        let c8 = discrepancies
            .iter()
            .find(|i| i.cell == "C8" && i.family == IdentityFamily::TccFooting)
            .expect("C8");
        let IdentityState::IssuerDiscrepancy(c) = &c8.state else {
            panic!("C8")
        };
        assert_eq!((c.printed, c.residual), (350, 100));
        assert_eq!(c.sign_flip_witness.as_deref(), Some("C6"));
        assert_eq!(footing_conflicts(c8).0, vec![vec!["C5", "C6", "C7", "C8"]]);
    }

    /// BD-331.S6: the port changed no state. At cd423b9 the two kernels ran side by side on all 55 synthetic
    /// instances with no difference; the corner kernel is gone, and its root at 9aab4a6
    /// (`fmine assure public --synthetic`), b50f7779…, pinned the result. bd-jg43.3 then changed the inputs on
    /// purpose (column G styled, column N added, frame v5), so this is now a regression pin of that commit's
    /// output: the five moved and five new states are pinned one by one by the synthetic tests in `lib.rs`.
    /// Frame v6 (bd-prmn.4) appends the Mine Stats ledger section, empty here: at v5 without it the root is
    /// still 54f1201b…, checked when the pin moved. Frame v7 (site-sheet family) changes only the version here:
    /// at v6 the root is still fd7864f9…, checked when the pin moved. BD-333's frame v8 adds the
    /// empty waterfall digest here; the v7 regression root was
    /// dfb17a08c70d5307ece489544334cea72086234182fe9d3d0580e3a0d209f5e5. These are regression
    /// values, not independent numerical oracles; the unchanged 60 states remain separately checked.
    #[test]
    fn test_port_keeps_synthetic_root() {
        let run = crate::run_synthetic_assurance(&[]).expect("synthetic run");
        assert_eq!(run.instances.len() + run.hidden_scope.instances.len(), 60);
        assert_eq!(run.mine_ledger, crate::mine_ledger::MineLedger::default());
        assert!(run.waterfall.pairs.is_empty());
        assert_eq!(
            run.ledger_anchor.state_root.to_hex(),
            "eb8bd4e4ae85d74b07b14e29e9c05a198b90491ea821e269bb72127972fe2ff0"
        );
    }

    /// 10⁴ seeded footings with placeholder lines against the closed-form rules the corner kernel used: the
    /// primary holds iff the printed lines sum to the printed total, the alternate iff |printed − sum|·2 ≤ 1 + k.
    /// The rates' closed-form oracle is `xtask/tests/public_exact_feas_oracle.rs`. The BD-340 S7 table generator does not
    /// exist yet, so these stand in for its tables.
    #[test]
    fn test_property_footings_match_closed_form() {
        use fmine_core::{PhiloxStream, RngKey};
        let mut rng = PhiloxStream::new(RngKey {
            seed_root: 331,
            partition: 6,
            domain: 0,
            var_group: 0,
            realization: 0,
            brick: 0,
        });
        let mut next = |lo: i64, hi: i64| lo + (rng.next_u64() % (hi - lo + 1) as u64) as i64;
        let mut footing_alt_only = 0;
        for case in 0..10_000 {
            // A footing of 1–6 lines, some placeholders (exact 0), printed near its sum.
            let lines: Vec<Option<i64>> = (0..next(1, 6))
                .map(|_| (next(0, 3) > 0).then(|| next(-500, 500)))
                .collect();
            let sum: i64 = lines.iter().flatten().sum();
            let printed = sum + next(-3, 3);
            let k_alt = lines.iter().flatten().count() as i64;
            let terms: Vec<(String, i64, i64)> = lines
                .iter()
                .enumerate()
                .map(|(i, l)| (format!("L{i}"), l.unwrap_or(0), i64::from(l.is_some())))
                .collect();
            let primary = footing_decision(("T", printed), &terms, xf::Construction::ComponentSum)
                .expect("decided");
            let alternate = footing_decision(("T", printed), &terms, xf::Construction::RoundedOnce)
                .expect("decided");
            assert_eq!(primary.is_none(), printed == sum, "case {case}");
            let old_alt = (printed - sum).abs() * 2 <= 1 + k_alt;
            assert_eq!(
                alternate.is_none(),
                old_alt,
                "case {case}: {lines:?} {printed}"
            );
            footing_alt_only += usize::from(old_alt && printed != sum);
        }
        println!(
            "{{\"test\":\"footing_property\",\"cases\":10000,\"seed\":331,\
             \"footing_alternate_only\":{footing_alt_only},\"disagree\":0}}"
        );
        assert!(footing_alt_only > 500, "{footing_alt_only}");
    }

    /// The alternative construction survives as a `RoundedOnce` run: TCC, as a line of the AISC footing, stands
    /// for its k₁ numeric block-1 lines (TCC ± k₁/2), as `aisc.rs` always counted it.
    #[test]
    fn test_alternate_consistent_preserved() {
        let vals: BTreeMap<u32, Val> = [
            (1, Val::Num(50.0)),
            (2, Val::Num(50.0)),
            (3, Val::Num(101.0)), // TCC, against 100
            (4, Val::Num(10.0)),
            (5, Val::Place),
            (6, Val::Num(113.0)), // AISC, against 101 + 10 = 111
        ]
        .into_iter()
        .collect();
        let v = |r: u32| vals.get(&r).copied().unwrap_or(Val::Blank);
        let cell_ref = |r: u32| format!("C{r}");
        let (block1, block2) = ([1, 2], [4, 5]);
        let all: Vec<u32> = block1.iter().chain(&block2).copied().collect();
        let footing = |total, lines: &[u32], family| {
            footing_state(
                &v, &cell_ref, &cell_ref, total, lines, family, &block1, &all,
            )
            .0
        };
        // TCC: residual 1 within (1 + 2)/2.
        let IdentityState::IssuerDiscrepancy(tcc) = footing(3, &block1, IdentityFamily::TccFooting)
        else {
            panic!("TCC footing")
        };
        assert_eq!((tcc.residual, tcc.alternate_consistent), (1, true));
        // AISC: residual 2 within (1 + 2 + 1)/2, which needs TCC ± 2/2; a TCC rounded once (± ½) would miss it.
        let IdentityState::IssuerDiscrepancy(aisc) =
            footing(6, &[3, 4, 5], IdentityFamily::AiscFooting)
        else {
            panic!("AISC footing")
        };
        assert_eq!((aisc.residual, aisc.alternate_consistent), (2, true));
        let rounded_once_tcc = [
            ("C3".to_string(), 101, 1),
            ("C4".to_string(), 10, 1),
            ("C5".to_string(), 0, 0),
        ];
        assert!(
            footing_decision(
                ("C6", 113),
                &rounded_once_tcc,
                xf::Construction::RoundedOnce
            )
            .expect("decided")
            .is_some()
        );
        // Rates: the alternate is the same rate with one half-width (N rounded once). 1000·(100 ± 2)/(100 ± ½)
        // reaches 1025.125…; 1000·(100 ± ½)/(100 ± ½) only 1010.05….
        assert!(holds(100, 4, 100, 1025) && !holds(100, 1, 100, 1025));
    }

    /// BD-331.S6 on the registered Barrick workbooks through 2026Q2. At cd423b9 the two kernels ran side by side
    /// on all 3,030 instances with no difference; the corner kernel is gone, and the root pins the result:
    /// `root_old` is the last row of `repro/barrick/expected.toml` (the corner kernel's until bd-jg43.3 moved
    /// the root on purpose, at 2386e66). Every (iii) carries
    /// S3's one minimal set. Prints locators and counts only (RISK-31).
    #[test]
    #[ignore = "needs the registered Barrick workbooks (through 2026Q2) under corpus_of_input_docs/downloaded/mine_stats/"]
    fn barrick_port_keeps_root_and_minimal_sets() {
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..");
        let profile = crate::sources::load_issuer_profile(&root, "barrick")
            .and_then(|p| p.through("2026Q2"))
            .expect("registry");
        let run = crate::run_public_assurance(&profile, &root)
            .expect("registered files present and unaltered");
        let expected = crate::sources::load_expected_roots(&crate::sources::expected_roots_path(
            &root, "barrick",
        ))
        .expect("expected roots");
        let root_old = &expected.last().expect("a row").state_root;
        let all: Vec<&IdentityInstance> = run
            .instances
            .iter()
            .chain(&run.hidden_scope.instances)
            .collect();
        let root_new = run.ledger_anchor.state_root.to_hex();
        println!(
            "{{\"instances\":{},\"visible\":{},\"root_old\":\"{root_old}\",\"root_new\":\"{root_new}\"}}",
            all.len(),
            run.instances.len()
        );
        assert_eq!((all.len(), run.instances.len()), (3030, 2595));
        assert_eq!(&root_new, root_old);
        // Every (iii) carries S3's minimal set; Kibali 'Q2 2024'!O11 (E815) has the witness O7.
        let mut discrepancies = 0;
        for inst in all
            .iter()
            .filter(|i| matches!(i.state, IdentityState::IssuerDiscrepancy(_)))
        {
            let (conflicts, own) = footing_conflicts(inst);
            assert_eq!(
                conflicts,
                vec![own],
                "{}:{}!{}",
                inst.file,
                inst.sheet,
                inst.cell
            );
            discrepancies += 1;
        }
        let kibali = all
            .iter()
            .find(|i| {
                i.file == "Barrick_Q3_2024_AISC.xlsx"
                    && i.sheet == "Q2 2024"
                    && i.cell == "O11"
                    && i.family == IdentityFamily::TccFooting
            })
            .expect("Kibali O11");
        let IdentityState::IssuerDiscrepancy(c) = &kibali.state else {
            panic!("Kibali O11 is not (iii)")
        };
        assert_eq!(c.sign_flip_witness.as_deref(), Some("O7"));
        println!("{{\"discrepancies_with_minimal_set\":{discrepancies}}}");
    }

    /// A placeholder line is an exact 0: it never widens a footing's alternate, while a printed 0 does.
    #[test]
    fn test_placeholder_line_not_in_k() {
        let terms = |placeholder: bool| -> Vec<(String, i64, i64)> {
            (0..5)
                .map(|i| {
                    let value = if i < 2 { 50 } else { 0 };
                    let step = i64::from(i < 2 || !placeholder);
                    (format!("L{i}"), value, step)
                })
                .collect()
        };
        // Printed 102 against 100: the alternate needs |2| ≤ (1 + k)/2.
        let alternate = |placeholder| {
            footing_decision(
                ("T", 102),
                &terms(placeholder),
                xf::Construction::RoundedOnce,
            )
            .expect("decided")
            .is_none()
        };
        assert!(!alternate(true), "three placeholders: k = 2, (1 + 2)/2 < 2");
        assert!(
            alternate(false),
            "three printed zeros: k = 5, (1 + 5)/2 ≥ 2"
        );
        // The same through the ledger: lines 50, 50, —, —, — and TCC 102.
        let vals: BTreeMap<u32, Val> = [
            (1, Val::Num(50.0)),
            (2, Val::Num(50.0)),
            (3, Val::Place),
            (4, Val::Place),
            (5, Val::Place),
            (6, Val::Num(102.0)),
        ]
        .into_iter()
        .collect();
        let v = |r: u32| vals.get(&r).copied().unwrap_or(Val::Blank);
        let cell_ref = |r: u32| format!("C{r}");
        let block1 = [1, 2, 3, 4, 5];
        let (state, calc) = footing_state(
            &v,
            &cell_ref,
            &cell_ref,
            6,
            &block1,
            IdentityFamily::TccFooting,
            &block1,
            &block1,
        );
        assert!(matches!(calc, Calc::Footing { k_alt: 2, .. }), "{calc:?}");
        let IdentityState::IssuerDiscrepancy(c) = state else {
            panic!("{state:?}")
        };
        assert_eq!((c.residual, c.alternate_consistent), (2, false));
        assert_eq!(c.line_cells, vec!["C1", "C2", "C3", "C4", "C5"]);
    }

    #[test]
    fn test_numeric_zero_line_counts_in_k() {
        // TCC footing with lines 50, 50, 0, 0, 0 printed as numbers, total 102: k_alt = 5 and the alternate holds.
        let vals: BTreeMap<u32, Val> = [
            (1, Val::Num(50.0)),
            (2, Val::Num(50.0)),
            (3, Val::Num(0.0)),
            (4, Val::Num(0.0)),
            (5, Val::Num(0.0)),
            (6, Val::Num(102.0)),
        ]
        .into_iter()
        .collect();
        let v = |r: u32| vals.get(&r).copied().unwrap_or(Val::Blank);
        let cell_ref = |r: u32| format!("C{r}");
        let block1 = [1, 2, 3, 4, 5];
        let (state, calc) = footing_state(
            &v,
            &cell_ref,
            &cell_ref,
            6,
            &block1,
            IdentityFamily::TccFooting,
            &block1,
            &block1,
        );
        assert!(matches!(calc, Calc::Footing { k_alt: 5, .. }), "{calc:?}");
        let IdentityState::IssuerDiscrepancy(c) = state else {
            panic!("{state:?}")
        };
        assert!(c.alternate_consistent);
    }

    /// The bead's mutation: n = 200 from two numeric lines and one placeholder, q = 100, r = 2021. At k = 2 the
    /// range is [1980.10, 2020.10] and the rate misses; counting the placeholder (k = 3) reaches 2025.13 and would
    /// wrongly pass.
    #[test]
    fn test_mutation_placeholder_counted_in_k_flips_verdict() {
        assert_eq!(implied_rate_text(200, 2, 100), "[1980.099, 2020.101]");
        assert!(!holds(200, 2, 100, 2021));
        assert!(holds(200, 3, 100, 2021));
    }

    /// D16 (synthetic): N = 200 ± 5/2, Q = 100 ± ½, so 1000·N/Q spans [395000/201, 405000/199], shown outward.
    #[test]
    fn test_d16_implied_interval_outward() {
        let iv = rate_range(200, 5, 100).expect("a range");
        assert_eq!(
            (iv.lo, iv.hi),
            (
                Rat::new(395_000, 201).expect("lo"),
                Rat::new(405_000, 199).expect("hi")
            )
        );
        assert_eq!(implied_rate_text(200, 5, 100), "[1965.174, 2035.176]");
        // E691, Veladero cost of sales per ounce ('Q4 2025'!J19): N = 67 ± ½, Q = 47 ± ½ spans [1400, 45000/31].
        let j19 = rate_range(67, 1, 47).expect("a range");
        assert_eq!(
            (j19.lo, j19.hi),
            (Rat::from_integer(1400), Rat::new(45_000, 31).expect("hi"))
        );
        assert_eq!(implied_rate_text(67, 1, 47), "[1400.000, 1451.613]");
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
