//! Determinism: BitwisePortable
//!
//! Exact drill-release remainder-grade interval arithmetic and disclosure flags (`BD-350.S2`,
//! `bd-gqu3.1`, plan §23.2 & §39 WP-15).
//!
//! Every public quantity uses `fmine_units::ExactQty` (zero bare `f64`, rule L-6) and zero
//! transcendentals (`BitwisePortable`). Verdicts report `"reproduces"` or
//! `"does not reproduce under the stated inputs"` — never classifying a resource (`INV-002`),
//! calling a disclosure false, or implying affiliation.

use core::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};

use fmine_core::codec::Encoder;
use fmine_core::hash::{Digest32, sha256};
use fmine_evidence::{Refusal, RefusalCode};
use fmine_units::{ExactQty, UnitsError};

/// Mandatory no-claim boundary (`INV-002`, `BD-350.S2`).
pub const DRILL_RELEASE_NO_CLAIM: &str = "Exact interval arithmetic and disclosure-flag audit over stated public drill intercepts. Does not classify any mineral resource or reserve, call any disclosure false, or imply affiliation with any issuer.";

const ZERO: ExactQty = ExactQty::from_integer(0);
const TWO: ExactQty = ExactQty::from_integer(2);

/// Length unit reported for a drill intercept.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum LengthUnit {
    Metres,
    Feet,
}

impl LengthUnit {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Metres => "m",
            Self::Feet => "ft",
        }
    }
}

/// Grade unit reported for a drill intercept.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum GradeUnit {
    GramsPerTonne,
    OzPerShortTon,
    Percent,
}

impl GradeUnit {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::GramsPerTonne => "g/t",
            Self::OzPerShortTon => "oz/st",
            Self::Percent => "%",
        }
    }

    /// Canonical target unit for exact comparison within compatible physical dimensions.
    #[must_use]
    pub const fn canonical(self) -> Self {
        match self {
            Self::GramsPerTonne | Self::OzPerShortTon => Self::GramsPerTonne,
            Self::Percent => Self::Percent,
        }
    }

    const fn tag(self) -> u8 {
        match self {
            Self::GramsPerTonne => 1,
            Self::OzPerShortTon => 2,
            Self::Percent => 3,
        }
    }
}

/// Grammatical relation linking a child intercept to its host.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum InclusionRelation {
    Host,
    Including,
    Within,
}

impl InclusionRelation {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Host => "host",
            Self::Including => "including",
            Self::Within => "within",
        }
    }
}

/// Printed decimal number together with its rounding step (derived from printed decimal places).
///
/// A printed token like `"47.07"` carries `value = 4707/100` and `step = 1/100`, defining the
/// closed rounding interval `[value - step/2, value + step/2] = [47.065, 47.075]`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PrintedQty {
    pub value: ExactQty,
    pub step: ExactQty,
}

impl PrintedQty {
    /// Construct a validated `PrintedQty` (`step >= 0`).
    pub fn new(value: ExactQty, step: ExactQty) -> Result<Self, Refusal> {
        if step.signum() < 0 {
            return Err(Refusal::new(
                RefusalCode::OutsidePhysicalDomain109,
                "printed rounding step must be >= 0",
            ));
        }
        Ok(Self { value, step })
    }

    /// Construct an exact zero-step `PrintedQty` (`[value, value]`).
    #[must_use]
    pub const fn exact(value: ExactQty) -> Self {
        Self { value, step: ZERO }
    }

    /// Parse a printed decimal literal (e.g. `"47.07"` -> `step = 1/100`, `"16"` -> `step = 1`).
    pub fn parse_decimal(token: &str) -> Result<Self, Refusal> {
        let trimmed = token.trim();
        if trimmed.is_empty()
            || trimmed.contains(['e', 'E'])
            || trimmed.bytes().any(|b| b.is_ascii_control())
        {
            return Err(Refusal::new(
                RefusalCode::SourceCellUnusable208,
                format!("unusable printed decimal token {token:?}"),
            ));
        }
        let body = trimmed
            .strip_prefix('-')
            .or_else(|| trimmed.strip_prefix('+'))
            .unwrap_or(trimmed);
        let mut parts = body.split('.');
        let int_part = parts.next().unwrap_or("");
        let frac_part = parts.next();
        if parts.next().is_some()
            || (int_part.is_empty() && frac_part.is_none_or(str::is_empty))
            || !int_part.bytes().all(|b| b.is_ascii_digit())
            || frac_part.is_some_and(|f| !f.bytes().all(|b| b.is_ascii_digit()))
        {
            return Err(Refusal::new(
                RefusalCode::SourceCellUnusable208,
                format!("malformed printed decimal token {token:?}"),
            ));
        }
        let decimals = frac_part.map_or(0usize, str::len);
        if decimals > 18 {
            return Err(Refusal::new(
                RefusalCode::OutsidePhysicalDomain109,
                format!("printed decimal token {token:?} exceeds 18 fractional digits"),
            ));
        }
        let normalized = trimmed.strip_prefix('+').unwrap_or(trimmed);
        let value = ExactQty::from_decimal_str(normalized).map_err(units_refusal)?;
        let den = 10_i128
            .checked_pow(decimals as u32)
            .ok_or_else(|| Refusal::new(RefusalCode::OutsidePhysicalDomain109, "step overflow"))?;
        let step = ExactQty::new(1, den).map_err(units_refusal)?;
        Ok(Self { value, step })
    }

    /// Closed rounding interval `[value - step/2, value + step/2]`.
    pub fn interval(self) -> Result<ExactInterval, Refusal> {
        let half = self.step.checked_div(TWO).map_err(units_refusal)?;
        let lo = self.value.checked_sub(half).map_err(units_refusal)?;
        let hi = self.value.checked_add(half).map_err(units_refusal)?;
        ExactInterval::new(lo, hi)
    }

    /// Convert length into canonical metres using exact `3048/10000` for feet.
    pub fn to_metres(self, unit: LengthUnit) -> Result<Self, Refusal> {
        match unit {
            LengthUnit::Metres => Ok(self),
            LengthUnit::Feet => Ok(Self {
                value: self.value.feet_to_metres().map_err(units_refusal)?,
                step: self.step.feet_to_metres().map_err(units_refusal)?,
            }),
        }
    }

    /// Convert grade into its canonical unit (`oz/st -> g/t` via exact `240/7`).
    pub fn to_canonical_grade(self, unit: GradeUnit) -> Result<(Self, GradeUnit), Refusal> {
        match unit {
            GradeUnit::GramsPerTonne => Ok((self, GradeUnit::GramsPerTonne)),
            GradeUnit::OzPerShortTon => Ok((
                Self {
                    value: self.value.oz_per_st_to_g_per_t().map_err(units_refusal)?,
                    step: self.step.oz_per_st_to_g_per_t().map_err(units_refusal)?,
                },
                GradeUnit::GramsPerTonne,
            )),
            GradeUnit::Percent => Ok((self, GradeUnit::Percent)),
        }
    }
}

/// Closed exact rational interval `[lo, hi]` with `lo <= hi`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExactInterval {
    pub lo: ExactQty,
    pub hi: ExactQty,
}

impl ExactInterval {
    /// Construct a validated exact interval (`lo <= hi`).
    pub fn new(lo: ExactQty, hi: ExactQty) -> Result<Self, Refusal> {
        if cmp_exact_qty(lo, hi) == Ordering::Greater {
            return Err(Refusal::new(
                RefusalCode::OutsidePhysicalDomain109,
                "interval lower bound exceeds upper bound",
            ));
        }
        Ok(Self { lo, hi })
    }

    /// True iff `x ∈ [lo, hi]`.
    #[must_use]
    pub fn contains(self, x: ExactQty) -> bool {
        cmp_exact_qty(x, self.lo) != Ordering::Less
            && cmp_exact_qty(x, self.hi) != Ordering::Greater
    }
}

/// One drill-release intercept node (possibly containing nested child inclusions).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Intercept {
    pub hole: String,
    pub from: Option<PrintedQty>,
    pub to: Option<PrintedQty>,
    pub length: PrintedQty,
    pub length_unit: LengthUnit,
    pub grade: PrintedQty,
    pub grade_unit: GradeUnit,
    pub metal_symbol: Option<String>,
    pub is_metal_equivalent: bool,
    pub is_downhole_length: bool,
    pub true_width_stated: bool,
    pub relation: InclusionRelation,
    pub children: Vec<Intercept>,
}

/// Disclosure context declared alongside a set of drill-release intercepts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReleaseDisclosureContext {
    /// Stated composite cut-off threshold (exact policy rule, not a rounded assay measurement).
    pub composite_cutoff: Option<(ExactQty, GradeUnit)>,
    pub internal_dilution_stated: bool,
    pub top_cut_stated: bool,
    pub metal_eq_prices_stated: bool,
    pub metal_eq_recoveries_stated: bool,
    /// Optional `"results from n holes"` headline claim (`HYPOTHESIS` gap check).
    pub declared_hole_count: Option<u32>,
}

impl ReleaseDisclosureContext {
    /// Complete, fully-disclosed context (zero disclosure-omission flags on clean intervals).
    #[must_use]
    pub fn complete(composite_cutoff: ExactQty, unit: GradeUnit) -> Self {
        Self {
            composite_cutoff: Some((composite_cutoff, unit)),
            internal_dilution_stated: true,
            top_cut_stated: true,
            metal_eq_prices_stated: true,
            metal_eq_recoveries_stated: true,
            declared_hole_count: None,
        }
    }
}

/// Structured disclosure flags emitted by drill-release auditing (`BD-350.S2`).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum DisclosureFlag {
    /// Remainder grade strictly below stated composite cut-off (headline carried by inclusion).
    RemainderBelowCutoff { hole: String },
    /// Inclusion grade strictly below its host intercept grade (e.g. E656 `30.6` within `47.07`).
    InclusionBelowHost { hole: String },
    /// Downhole length reported without a true-width statement.
    NoTrueWidth { hole: String },
    /// Composite cut-off, internal dilution, or top cut is unstated.
    UnstatedCutoffOrDilution {
        missing_cutoff: bool,
        missing_internal_dilution: bool,
        missing_top_cut: bool,
    },
    /// Metal-equivalent grade reported without stated metal prices or recoveries.
    MetalEquivalentUnstatedTerms {
        hole: String,
        missing_prices: bool,
        missing_recoveries: bool,
    },
    /// `HYPOTHESIS`: hole sequence has gaps or fewer holes than `"results from n holes"`.
    HoleSequenceGap {
        declared_hole_count: u32,
        observed_hole_count: u32,
        missing_sequence_numbers: Vec<u32>,
    },
}

impl DisclosureFlag {
    #[must_use]
    pub const fn kind_str(&self) -> &'static str {
        match self {
            Self::RemainderBelowCutoff { .. } => "remainder_below_cutoff",
            Self::InclusionBelowHost { .. } => "inclusion_below_host",
            Self::NoTrueWidth { .. } => "no_true_width",
            Self::UnstatedCutoffOrDilution { .. } => "unstated_cutoff_or_dilution",
            Self::MetalEquivalentUnstatedTerms { .. } => "metal_equivalent_unstated_terms",
            Self::HoleSequenceGap { .. } => "hole_sequence_gap",
        }
    }

    fn encode(&self, enc: &mut Encoder) {
        match self {
            Self::RemainderBelowCutoff { hole } => {
                enc.put_u8(1);
                enc.put_str(hole);
            }
            Self::InclusionBelowHost { hole } => {
                enc.put_u8(2);
                enc.put_str(hole);
            }
            Self::NoTrueWidth { hole } => {
                enc.put_u8(3);
                enc.put_str(hole);
            }
            Self::UnstatedCutoffOrDilution {
                missing_cutoff,
                missing_internal_dilution,
                missing_top_cut,
            } => {
                enc.put_u8(4);
                enc.put_u8(u8::from(*missing_cutoff));
                enc.put_u8(u8::from(*missing_internal_dilution));
                enc.put_u8(u8::from(*missing_top_cut));
            }
            Self::MetalEquivalentUnstatedTerms {
                hole,
                missing_prices,
                missing_recoveries,
            } => {
                enc.put_u8(5);
                enc.put_str(hole);
                enc.put_u8(u8::from(*missing_prices));
                enc.put_u8(u8::from(*missing_recoveries));
            }
            Self::HoleSequenceGap {
                declared_hole_count,
                observed_hole_count,
                missing_sequence_numbers,
            } => {
                enc.put_u8(6);
                enc.put_u64(u64::from(*declared_hole_count));
                enc.put_u64(u64::from(*observed_hole_count));
                enc.put_u64(missing_sequence_numbers.len() as u64);
                for &n in missing_sequence_numbers {
                    enc.put_u64(u64::from(n));
                }
            }
        }
    }
}

/// Exact remainder-grade output for one intercept node with one or more children.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemainderResult {
    pub hole: String,
    pub canonical_grade_unit: GradeUnit,
    /// Host length in canonical metres (`value`, `step`).
    pub host_length_m: PrintedQty,
    /// Host grade in canonical grade unit (`value`, `step`).
    pub host_grade: PrintedQty,
    /// Exact point remainder length `L - Σ l_i` (metres).
    pub remainder_length_m: ExactQty,
    /// Exact remainder length interval `[L_lo - Σ l_i_hi, L_hi - Σ l_i_lo]` (metres).
    pub remainder_length_interval_m: ExactInterval,
    /// Exact point remainder grade `(L·g - Σ l_i·g_i) / (L - Σ l_i)`.
    pub remainder_grade_point: ExactQty,
    /// Exact global `[min, max]` of remainder grade across the entire rounding box.
    pub remainder_grade_interval: ExactInterval,
    /// Recursive bottom-up remainder results for nested children that themselves carry inclusions.
    pub nested_results: Vec<RemainderResult>,
}

/// Complete audited drill-intercept record within a release report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InterceptAudit {
    pub hole: String,
    pub canonical_length_m: PrintedQty,
    pub canonical_grade: PrintedQty,
    pub canonical_grade_unit: GradeUnit,
    pub child_count: usize,
    pub remainder: Option<RemainderResult>,
    pub flags: Vec<DisclosureFlag>,
}

impl InterceptAudit {
    /// Structured single-line NDJSON event matching `bd-gqu3.1`.
    #[must_use]
    pub fn to_ndjson_line(&self) -> String {
        let (rem_lo, rem_hi) = match &self.remainder {
            Some(r) => (
                format!(
                    "\"{}/{}\"",
                    r.remainder_grade_interval.lo.num(),
                    r.remainder_grade_interval.lo.den()
                ),
                format!(
                    "\"{}/{}\"",
                    r.remainder_grade_interval.hi.num(),
                    r.remainder_grade_interval.hi.den()
                ),
            ),
            None => ("null".to_string(), "null".to_string()),
        };
        let flags_json: Vec<String> = self
            .flags
            .iter()
            .map(|f| format!("\"{}\"", f.kind_str()))
            .collect();
        format!(
            "{{\"event\":\"intercept\",\"hole\":{},\"length\":\"{}/{}\",\"grade\":\"{}/{}\",\"children\":{},\"remainder_lo\":{},\"remainder_hi\":{},\"flags\":[{}]}}",
            json_quote(&self.hole),
            self.canonical_length_m.value.num(),
            self.canonical_length_m.value.den(),
            self.canonical_grade.value.num(),
            self.canonical_grade.value.den(),
            self.child_count,
            rem_lo,
            rem_hi,
            flags_json.join(",")
        )
    }
}

/// Order- and unit-invariant audit report for a drill release (`BD-350.S2`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DrillReleaseAudit {
    pub holes_count: usize,
    pub intervals: Vec<InterceptAudit>,
    pub release_flags: Vec<DisclosureFlag>,
    pub flags_by_kind: BTreeMap<String, usize>,
    pub no_claim: &'static str,
    pub digest: Digest32,
}

impl DrillReleaseAudit {
    /// Structured summary NDJSON line matching `bd-gqu3.1`.
    #[must_use]
    pub fn summary_ndjson_line(&self, refused: usize) -> String {
        let entries: Vec<String> = self
            .flags_by_kind
            .iter()
            .map(|(k, v)| format!("{}:{}", json_quote(k), v))
            .collect();
        format!(
            "{{\"event\":\"intercept_summary\",\"holes\":{},\"intervals\":{},\"flags_by_kind\":{{{}}},\"refused\":{}}}",
            self.holes_count,
            self.intervals.len(),
            entries.join(","),
            refused
        )
    }

    /// True iff zero flags fired across every interval and the release header.
    #[must_use]
    pub fn is_clean(&self) -> bool {
        self.flags_by_kind.is_empty()
    }
}

/// Compute exact remainder grade interval bottom-up over an intercept tree (`BD-350.S2`).
///
/// Refuses if:
/// - `host` has no child inclusions (`FMINE-REF-201`);
/// - any length interval reaches `<= 0` or any grade value is `< 0` (`FMINE-REF-109`);
/// - any direct child uses an incompatible grade dimension (e.g. `%` inside `g/t`, `FMINE-REF-105`);
/// - any direct children overlap in downhole `(from, to)` span (`FMINE-REF-109`);
/// - `L - Σ l_i <= 0` anywhere on the rounding box (`FMINE-REF-109`).
pub fn remainder_grade(host: &Intercept) -> Result<RemainderResult, Refusal> {
    if host.children.is_empty() {
        return Err(Refusal::new(
            RefusalCode::NoClaimInDomain201,
            format!(
                "hole {}: remainder_grade requires at least one child inclusion",
                host.hole
            ),
        ));
    }
    evaluate_remainder_recursive(host)
}

/// Audit a full drill release (one or more host intercepts plus disclosure context).
///
/// Output ordering, flag sets, and `DRI1` digest are invariant to input intercept order,
/// child inclusion order, and exact unit representation (`m` vs `ft`, `g/t` vs `oz/st`).
pub fn audit_drill_release(
    intercepts: &[Intercept],
    context: &ReleaseDisclosureContext,
) -> Result<DrillReleaseAudit, Refusal> {
    if intercepts.is_empty() {
        return Err(Refusal::new(
            RefusalCode::NoClaimInDomain201,
            "drill release audit requires at least one intercept",
        ));
    }

    // Validate optional composite cutoff (exact policy threshold) in canonical units.
    let canonical_cutoff = match context.composite_cutoff {
        Some((qty, unit)) => {
            if qty.signum() < 0 {
                return Err(Refusal::new(
                    RefusalCode::OutsidePhysicalDomain109,
                    "composite cut-off grade must be >= 0",
                ));
            }
            let (canon, canon_unit) = PrintedQty::exact(qty).to_canonical_grade(unit)?;
            Some((canon.value, canon_unit))
        }
        None => None,
    };

    let mut audits = Vec::with_capacity(intercepts.len());
    let mut distinct_holes = BTreeSet::new();

    for host in intercepts {
        validate_intercept_scalars(host)?;
        distinct_holes.insert(host.hole.clone());
        let canon_len = host.length.to_metres(host.length_unit)?;
        let (canon_grade, canon_unit) = host.grade.to_canonical_grade(host.grade_unit)?;

        let mut flags_set = BTreeSet::new();
        let remainder = if host.children.is_empty() {
            None
        } else {
            let rem = evaluate_remainder_recursive(host)?;
            collect_tree_flags(host, &rem, canonical_cutoff, &mut flags_set)?;
            Some(rem)
        };

        // Flags that apply even on childless intercepts.
        collect_node_disclosure_flags(host, context, &mut flags_set);

        audits.push(InterceptAudit {
            hole: host.hole.clone(),
            canonical_length_m: canon_len,
            canonical_grade: canon_grade,
            canonical_grade_unit: canon_unit,
            child_count: host.children.len(),
            remainder,
            flags: flags_set.into_iter().collect(),
        });
    }

    // Canonical order independent of input slice permutation.
    audits.sort_by(cmp_intercept_audit);

    let mut release_flags_set = BTreeSet::new();
    let missing_cutoff = context.composite_cutoff.is_none();
    let missing_internal_dilution = !context.internal_dilution_stated;
    let missing_top_cut = !context.top_cut_stated;
    if missing_cutoff || missing_internal_dilution || missing_top_cut {
        release_flags_set.insert(DisclosureFlag::UnstatedCutoffOrDilution {
            missing_cutoff,
            missing_internal_dilution,
            missing_top_cut,
        });
    }

    if let Some(declared) = context.declared_hole_count
        && let Some(gap_flag) = detect_hole_sequence_gap(declared, &distinct_holes)
    {
        release_flags_set.insert(gap_flag);
    }

    let release_flags: Vec<DisclosureFlag> = release_flags_set.into_iter().collect();
    let mut flags_by_kind: BTreeMap<String, usize> = BTreeMap::new();
    for a in &audits {
        for f in &a.flags {
            *flags_by_kind.entry(f.kind_str().to_string()).or_insert(0) += 1;
        }
    }
    for f in &release_flags {
        *flags_by_kind.entry(f.kind_str().to_string()).or_insert(0) += 1;
    }

    let digest = compute_audit_digest(distinct_holes.len(), &audits, &release_flags);
    Ok(DrillReleaseAudit {
        holes_count: distinct_holes.len(),
        intervals: audits,
        release_flags,
        flags_by_kind,
        no_claim: DRILL_RELEASE_NO_CLAIM,
        digest,
    })
}

fn evaluate_remainder_recursive(host: &Intercept) -> Result<RemainderResult, Refusal> {
    validate_intercept_scalars(host)?;
    let host_len = host.length.to_metres(host.length_unit)?;
    let host_len_iv = host_len.interval()?;
    if host_len_iv.lo.signum() <= 0 {
        return Err(Refusal::new(
            RefusalCode::OutsidePhysicalDomain109,
            format!(
                "hole {}: host length interval must remain strictly positive",
                host.hole
            ),
        ));
    }
    let (host_grade, canon_unit) = host.grade.to_canonical_grade(host.grade_unit)?;
    let host_grade_iv = host_grade.interval()?;

    let host_span = extract_span_m(host)?;

    // Evaluate children bottom-up and normalize each direct child into canonical units.
    struct CanonChild {
        len: PrintedQty,
        len_iv: ExactInterval,
        grade: PrintedQty,
        grade_iv: ExactInterval,
        span_m: Option<(ExactQty, ExactQty)>,
    }

    let mut canon_children = Vec::with_capacity(host.children.len());
    let mut nested_results = Vec::new();

    for child in &host.children {
        validate_intercept_scalars(child)?;
        let child_len = child.length.to_metres(child.length_unit)?;
        let child_len_iv = child_len.interval()?;
        if child_len_iv.lo.signum() <= 0 {
            return Err(Refusal::new(
                RefusalCode::OutsidePhysicalDomain109,
                format!(
                    "hole {}: child inclusion length interval must remain strictly positive",
                    host.hole
                ),
            ));
        }
        let (child_grade, child_unit) = child.grade.to_canonical_grade(child.grade_unit)?;
        if child_unit != canon_unit {
            return Err(Refusal::new(
                RefusalCode::BasisMismatch105,
                format!(
                    "hole {}: incompatible grade units {} vs {}",
                    host.hole,
                    child.grade_unit.as_str(),
                    host.grade_unit.as_str()
                ),
            ));
        }
        let child_grade_iv = child_grade.interval()?;
        let child_span = extract_span_m(child)?;
        if let (Some((h_from, h_to)), Some((c_from, c_to))) = (host_span, child_span)
            && (cmp_exact_qty(c_from, h_from) == Ordering::Less
                || cmp_exact_qty(c_to, h_to) == Ordering::Greater)
        {
            return Err(Refusal::new(
                RefusalCode::OutsidePhysicalDomain109,
                format!(
                    "hole {}: child inclusion span lies outside host downhole span",
                    host.hole
                ),
            ));
        }

        if !child.children.is_empty() {
            nested_results.push(evaluate_remainder_recursive(child)?);
        }

        canon_children.push(CanonChild {
            len: child_len,
            len_iv: child_len_iv,
            grade: child_grade,
            grade_iv: child_grade_iv,
            span_m: child_span,
        });
    }

    // Check pairwise downhole overlap among direct children carrying explicit (from, to) spans.
    let mut spans: Vec<(ExactQty, ExactQty)> =
        canon_children.iter().filter_map(|c| c.span_m).collect();
    spans.sort_by(|a, b| cmp_exact_qty(a.0, b.0).then_with(|| cmp_exact_qty(a.1, b.1)));
    for pair in spans.windows(2) {
        if cmp_exact_qty(pair[0].1, pair[1].0) == Ordering::Greater {
            return Err(Refusal::new(
                RefusalCode::OutsidePhysicalDomain109,
                format!("hole {}: overlapping child inclusions refused", host.hole),
            ));
        }
    }

    // Order-invariant canonical sort of direct children before accumulation.
    canon_children.sort_by(|a, b| {
        cmp_exact_qty(a.len.value, b.len.value)
            .then_with(|| cmp_exact_qty(a.grade.value, b.grade.value))
            .then_with(|| cmp_exact_qty(a.len.step, b.len.step))
            .then_with(|| cmp_exact_qty(a.grade.step, b.grade.step))
    });
    nested_results.sort_by(cmp_remainder_result);

    // Check remainder length strictly positive across the entire rounding box: L_lo - Σ l_i_hi > 0.
    let mut sum_child_lo = ZERO;
    let mut sum_child_hi = ZERO;
    let mut sum_child_pt = ZERO;
    let mut sum_child_metal_pt = ZERO;
    for c in &canon_children {
        sum_child_lo = sum_child_lo
            .checked_add(c.len_iv.lo)
            .map_err(units_refusal)?;
        sum_child_hi = sum_child_hi
            .checked_add(c.len_iv.hi)
            .map_err(units_refusal)?;
        sum_child_pt = sum_child_pt
            .checked_add(c.len.value)
            .map_err(units_refusal)?;
        let metal = c
            .len
            .value
            .checked_mul(c.grade.value)
            .map_err(units_refusal)?;
        sum_child_metal_pt = sum_child_metal_pt
            .checked_add(metal)
            .map_err(units_refusal)?;
    }

    let rem_len_lo = host_len_iv
        .lo
        .checked_sub(sum_child_hi)
        .map_err(units_refusal)?;
    let rem_len_hi = host_len_iv
        .hi
        .checked_sub(sum_child_lo)
        .map_err(units_refusal)?;
    if rem_len_lo.signum() <= 0 {
        return Err(Refusal::new(
            RefusalCode::OutsidePhysicalDomain109,
            format!(
                "hole {}: remainder length L - sum(l_i) is non-positive on the rounding box",
                host.hole
            ),
        ));
    }
    let rem_len_pt = host_len
        .value
        .checked_sub(sum_child_pt)
        .map_err(units_refusal)?;
    if rem_len_pt.signum() <= 0 {
        return Err(Refusal::new(
            RefusalCode::OutsidePhysicalDomain109,
            format!(
                "hole {}: point remainder length L - sum(l_i) must be strictly positive",
                host.hole
            ),
        ));
    }

    let host_metal_pt = host_len
        .value
        .checked_mul(host_grade.value)
        .map_err(units_refusal)?;
    let rem_metal_pt = host_metal_pt
        .checked_sub(sum_child_metal_pt)
        .map_err(units_refusal)?;
    let rem_grade_pt = rem_metal_pt
        .checked_div(rem_len_pt)
        .map_err(units_refusal)?;

    // Exact linear-fractional extrema over the full (2k + 2)-dimensional box in O(k^2):
    // 1. Minimize f -> fix g = g_lo, g_i = g_i_hi, then scan threshold regions over (L, l_i).
    // 2. Maximize f -> fix g = g_hi, g_i = g_i_lo, then scan threshold regions over (L, l_i).
    let child_min_terms: Vec<(ExactInterval, ExactQty)> = canon_children
        .iter()
        .map(|c| (c.len_iv, c.grade_iv.hi))
        .collect();
    let child_max_terms: Vec<(ExactInterval, ExactQty)> = canon_children
        .iter()
        .map(|c| (c.len_iv, c.grade_iv.lo))
        .collect();

    let rem_grade_lo =
        exact_linear_fractional_extremum(host_len_iv, host_grade_iv.lo, &child_min_terms, false)?;
    let rem_grade_hi =
        exact_linear_fractional_extremum(host_len_iv, host_grade_iv.hi, &child_max_terms, true)?;

    Ok(RemainderResult {
        hole: host.hole.clone(),
        canonical_grade_unit: canon_unit,
        host_length_m: host_len,
        host_grade,
        remainder_length_m: rem_len_pt,
        remainder_length_interval_m: ExactInterval::new(rem_len_lo, rem_len_hi)?,
        remainder_grade_point: rem_grade_pt,
        remainder_grade_interval: ExactInterval::new(rem_grade_lo, rem_grade_hi)?,
        nested_results,
    })
}

/// Exact global minimum (`maximize == false`) or maximum (`maximize == true`) of
/// `f(L, l) = (L·g_host - Σ l_i·g_i) / (L - Σ l_i)` over `L ∈ [L_lo, L_hi]`, `l_i ∈ [l_i_lo, l_i_hi]`
/// assuming `L_lo - Σ l_i_hi > 0`.
fn exact_linear_fractional_extremum(
    host_len: ExactInterval,
    host_grade: ExactQty,
    children: &[(ExactInterval, ExactQty)],
    maximize: bool,
) -> Result<ExactQty, Refusal> {
    let mut breakpoints = Vec::with_capacity(children.len() + 1);
    breakpoints.push(host_grade);
    for &(_, g) in children {
        breakpoints.push(g);
    }
    breakpoints.sort_by(|a, b| cmp_exact_qty(*a, *b));
    breakpoints.dedup();

    let mut best: Option<ExactQty> = None;
    for region in 0..=breakpoints.len() {
        let is_below = |x: ExactQty| -> bool {
            region > 0 && cmp_exact_qty(x, breakpoints[region - 1]) != Ordering::Greater
        };

        let l_host = if maximize {
            if !is_below(host_grade) {
                host_len.hi
            } else {
                host_len.lo
            }
        } else if is_below(host_grade) {
            host_len.hi
        } else {
            host_len.lo
        };

        let mut num = l_host.checked_mul(host_grade).map_err(units_refusal)?;
        let mut den = l_host;
        for &(len_iv, g_i) in children {
            let l_i = if maximize {
                if is_below(g_i) { len_iv.hi } else { len_iv.lo }
            } else if !is_below(g_i) {
                len_iv.hi
            } else {
                len_iv.lo
            };
            let prod = l_i.checked_mul(g_i).map_err(units_refusal)?;
            num = num.checked_sub(prod).map_err(units_refusal)?;
            den = den.checked_sub(l_i).map_err(units_refusal)?;
        }
        let val = num.checked_div(den).map_err(units_refusal)?;
        best = Some(match best {
            None => val,
            Some(cur) => {
                let cmp = cmp_exact_qty(val, cur);
                if (maximize && cmp == Ordering::Greater) || (!maximize && cmp == Ordering::Less) {
                    val
                } else {
                    cur
                }
            }
        });
    }

    best.ok_or_else(|| {
        Refusal::new(
            RefusalCode::NoClaimInDomain201,
            "linear-fractional extremum had zero candidate regions",
        )
    })
}

fn collect_node_disclosure_flags(
    node: &Intercept,
    context: &ReleaseDisclosureContext,
    flags: &mut BTreeSet<DisclosureFlag>,
) {
    if node.is_downhole_length && !node.true_width_stated {
        flags.insert(DisclosureFlag::NoTrueWidth {
            hole: node.hole.clone(),
        });
    }
    if node.is_metal_equivalent
        && (!context.metal_eq_prices_stated || !context.metal_eq_recoveries_stated)
    {
        flags.insert(DisclosureFlag::MetalEquivalentUnstatedTerms {
            hole: node.hole.clone(),
            missing_prices: !context.metal_eq_prices_stated,
            missing_recoveries: !context.metal_eq_recoveries_stated,
        });
    }
    for child in &node.children {
        collect_node_disclosure_flags(child, context, flags);
    }
}

fn collect_tree_flags(
    node: &Intercept,
    rem: &RemainderResult,
    canonical_cutoff: Option<(ExactQty, GradeUnit)>,
    flags: &mut BTreeSet<DisclosureFlag>,
) -> Result<(), Refusal> {
    let host_grade_iv = rem.host_grade.interval()?;
    if let Some((cutoff_exact, cutoff_unit)) = canonical_cutoff {
        if cutoff_unit != rem.canonical_grade_unit {
            return Err(Refusal::new(
                RefusalCode::BasisMismatch105,
                format!(
                    "hole {}: composite cut-off unit {} does not match intercept unit {}",
                    node.hole,
                    cutoff_unit.as_str(),
                    rem.canonical_grade_unit.as_str()
                ),
            ));
        }
        // Strict interval dominance: entire rounding box of remainder grade lies strictly below stated cut-off.
        if cmp_exact_qty(rem.remainder_grade_interval.hi, cutoff_exact) == Ordering::Less {
            flags.insert(DisclosureFlag::RemainderBelowCutoff {
                hole: node.hole.clone(),
            });
        }
    }

    for child in &node.children {
        let (child_grade, _) = child.grade.to_canonical_grade(child.grade_unit)?;
        let child_grade_iv = child_grade.interval()?;
        if cmp_exact_qty(child_grade_iv.hi, host_grade_iv.lo) == Ordering::Less {
            flags.insert(DisclosureFlag::InclusionBelowHost {
                hole: node.hole.clone(),
            });
        }
        if !child.children.is_empty() {
            let sub_rem = evaluate_remainder_recursive(child)?;
            collect_tree_flags(child, &sub_rem, canonical_cutoff, flags)?;
        }
    }

    Ok(())
}

fn detect_hole_sequence_gap(
    declared_hole_count: u32,
    distinct_holes: &BTreeSet<String>,
) -> Option<DisclosureFlag> {
    let observed = u32::try_from(distinct_holes.len()).unwrap_or(u32::MAX);
    // Group holes by alphabetic/hyphen prefix + trailing integer sequence (e.g. "DDH-" + 001).
    let mut by_prefix: BTreeMap<String, Vec<u32>> = BTreeMap::new();
    for hole in distinct_holes {
        if let Some((prefix, num)) = split_hole_sequence(hole) {
            by_prefix.entry(prefix).or_default().push(num);
        }
    }
    let mut missing = Vec::new();
    for nums in by_prefix.values_mut() {
        nums.sort_unstable();
        nums.dedup();
        if nums.len() >= 2 {
            let first = nums[0];
            let last = nums[nums.len() - 1];
            let present: BTreeSet<u32> = nums.iter().copied().collect();
            for candidate in first..=last {
                if !present.contains(&candidate) {
                    missing.push(candidate);
                }
            }
        }
    }
    missing.sort_unstable();
    missing.dedup();

    if observed < declared_hole_count || !missing.is_empty() {
        Some(DisclosureFlag::HoleSequenceGap {
            declared_hole_count,
            observed_hole_count: observed,
            missing_sequence_numbers: missing,
        })
    } else {
        None
    }
}

fn split_hole_sequence(hole: &str) -> Option<(String, u32)> {
    let trimmed = hole.trim();
    // Strip trailing single alphabetic suffix like 'D' in FM24-209D if preceded by digits.
    let core = if trimmed.len() >= 2
        && trimmed.as_bytes()[trimmed.len() - 1].is_ascii_alphabetic()
        && trimmed.as_bytes()[trimmed.len() - 2].is_ascii_digit()
    {
        &trimmed[..trimmed.len() - 1]
    } else {
        trimmed
    };
    let digit_start = core
        .rfind(|c: char| !c.is_ascii_digit())
        .map_or(0, |idx| idx + 1);
    if digit_start >= core.len() {
        return None;
    }
    let prefix = core[..digit_start].to_string();
    let num = core[digit_start..].parse::<u32>().ok()?;
    Some((prefix, num))
}

fn extract_span_m(node: &Intercept) -> Result<Option<(ExactQty, ExactQty)>, Refusal> {
    match (node.from, node.to) {
        (Some(f), Some(t)) => {
            let f_m = f.to_metres(node.length_unit)?;
            let t_m = t.to_metres(node.length_unit)?;
            if f_m.value.signum() < 0 || cmp_exact_qty(t_m.value, f_m.value) != Ordering::Greater {
                return Err(Refusal::new(
                    RefusalCode::OutsidePhysicalDomain109,
                    format!(
                        "hole {}: downhole span must satisfy 0 <= from < to",
                        node.hole
                    ),
                ));
            }
            let f_iv = f_m.interval()?;
            let t_iv = t_m.interval()?;
            let span_lo = t_iv.lo.checked_sub(f_iv.hi).map_err(units_refusal)?;
            let span_hi = t_iv.hi.checked_sub(f_iv.lo).map_err(units_refusal)?;
            let len_iv = node.length.to_metres(node.length_unit)?.interval()?;
            if cmp_exact_qty(span_hi, len_iv.lo) == Ordering::Less
                || cmp_exact_qty(len_iv.hi, span_lo) == Ordering::Less
            {
                return Err(Refusal::new(
                    RefusalCode::OutsidePhysicalDomain109,
                    format!(
                        "hole {}: downhole span (to - from) is inconsistent with printed length",
                        node.hole
                    ),
                ));
            }
            Ok(Some((f_m.value, t_m.value)))
        }
        (None, None) => Ok(None),
        _ => Err(Refusal::new(
            RefusalCode::UndeclaredUnitBasisOrTz107,
            format!(
                "hole {}: downhole span requires both 'from' and 'to' or neither",
                node.hole
            ),
        )),
    }
}

fn validate_intercept_scalars(node: &Intercept) -> Result<(), Refusal> {
    if node.hole.trim().is_empty()
        || node.hole != node.hole.trim()
        || node.hole.bytes().any(|b| b.is_ascii_control())
    {
        return Err(Refusal::new(
            RefusalCode::UndeclaredUnitBasisOrTz107,
            format!("invalid hole identifier {:?}", node.hole),
        ));
    }
    if node.length.value.signum() <= 0 || node.length.step.signum() < 0 {
        return Err(Refusal::new(
            RefusalCode::OutsidePhysicalDomain109,
            format!("hole {}: length must be > 0 and step >= 0", node.hole),
        ));
    }
    if node.grade.value.signum() < 0 || node.grade.step.signum() < 0 {
        return Err(Refusal::new(
            RefusalCode::OutsidePhysicalDomain109,
            format!("hole {}: grade must be >= 0 and step >= 0", node.hole),
        ));
    }
    Ok(())
}

fn cmp_intercept_audit(a: &InterceptAudit, b: &InterceptAudit) -> Ordering {
    a.hole
        .cmp(&b.hole)
        .then_with(|| cmp_exact_qty(a.canonical_length_m.value, b.canonical_length_m.value))
        .then_with(|| cmp_exact_qty(a.canonical_grade.value, b.canonical_grade.value))
        .then_with(|| a.canonical_grade_unit.cmp(&b.canonical_grade_unit))
        .then_with(|| a.child_count.cmp(&b.child_count))
        .then_with(|| match (&a.remainder, &b.remainder) {
            (None, None) => Ordering::Equal,
            (None, Some(_)) => Ordering::Less,
            (Some(_), None) => Ordering::Greater,
            (Some(ra), Some(rb)) => cmp_remainder_result(ra, rb),
        })
        .then_with(|| a.flags.cmp(&b.flags))
}

fn cmp_remainder_result(a: &RemainderResult, b: &RemainderResult) -> Ordering {
    a.hole
        .cmp(&b.hole)
        .then_with(|| cmp_exact_qty(a.host_length_m.value, b.host_length_m.value))
        .then_with(|| cmp_exact_qty(a.host_grade.value, b.host_grade.value))
        .then_with(|| cmp_exact_qty(a.remainder_grade_point, b.remainder_grade_point))
}

fn compute_audit_digest(
    holes_count: usize,
    audits: &[InterceptAudit],
    release_flags: &[DisclosureFlag],
) -> Digest32 {
    let mut enc = Encoder::new_frame(*b"DRI1", 1);
    enc.put_u64(holes_count as u64);
    enc.put_u64(audits.len() as u64);
    for a in audits {
        enc.put_str(&a.hole);
        put_printed(&mut enc, a.canonical_length_m);
        put_printed(&mut enc, a.canonical_grade);
        enc.put_u8(a.canonical_grade_unit.tag());
        enc.put_u64(a.child_count as u64);
        match &a.remainder {
            None => enc.put_u8(0),
            Some(r) => {
                enc.put_u8(1);
                encode_remainder(&mut enc, r);
            }
        }
        enc.put_u64(a.flags.len() as u64);
        for f in &a.flags {
            f.encode(&mut enc);
        }
    }
    enc.put_u64(release_flags.len() as u64);
    for f in release_flags {
        f.encode(&mut enc);
    }
    sha256(&enc.finish())
}

fn encode_remainder(enc: &mut Encoder, r: &RemainderResult) {
    enc.put_str(&r.hole);
    enc.put_u8(r.canonical_grade_unit.tag());
    put_printed(enc, r.host_length_m);
    put_printed(enc, r.host_grade);
    put_qty(enc, r.remainder_length_m);
    put_qty(enc, r.remainder_length_interval_m.lo);
    put_qty(enc, r.remainder_length_interval_m.hi);
    put_qty(enc, r.remainder_grade_point);
    put_qty(enc, r.remainder_grade_interval.lo);
    put_qty(enc, r.remainder_grade_interval.hi);
    enc.put_u64(r.nested_results.len() as u64);
    for sub in &r.nested_results {
        encode_remainder(enc, sub);
    }
}

fn put_printed(enc: &mut Encoder, p: PrintedQty) {
    put_qty(enc, p.value);
    put_qty(enc, p.step);
}

fn put_qty(enc: &mut Encoder, q: ExactQty) {
    enc.put_bytes(&q.num().to_le_bytes());
    enc.put_bytes(&q.den().to_le_bytes());
}

fn units_refusal(e: UnitsError) -> Refusal {
    Refusal::new(RefusalCode::OutsidePhysicalDomain109, e.to_string())
}

/// Overflow-free exact comparison of two `ExactQty` rationals via 256-bit `u128` products.
#[must_use]
pub fn cmp_exact_qty(a: ExactQty, b: ExactQty) -> Ordering {
    let sa = a.signum();
    let sb = b.signum();
    match sa.cmp(&sb) {
        Ordering::Equal => {}
        ord => return ord,
    }
    if sa == 0 {
        return Ordering::Equal;
    }
    let lhs = mul_u128_wide(a.num().unsigned_abs(), b.den().unsigned_abs());
    let rhs = mul_u128_wide(b.num().unsigned_abs(), a.den().unsigned_abs());
    let mag = lhs.cmp(&rhs);
    if sa > 0 { mag } else { mag.reverse() }
}

const fn mul_u128_wide(a: u128, b: u128) -> (u128, u128) {
    let a_lo = a as u64 as u128;
    let a_hi = a >> 64;
    let b_lo = b as u64 as u128;
    let b_hi = b >> 64;

    let p0 = a_lo * b_lo;
    let p1 = a_lo * b_hi;
    let p2 = a_hi * b_lo;
    let p3 = a_hi * b_hi;

    let mid1 = (p0 >> 64) + (p1 as u64 as u128) + (p2 as u64 as u128);
    let hi = p3 + (p1 >> 64) + (p2 >> 64) + (mid1 >> 64);
    let lo = (p0 as u64 as u128) | ((mid1 as u64 as u128) << 64);
    (hi, lo)
}

fn json_quote(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                use core::fmt::Write as _;
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mk_intercept(
        hole: &str,
        len: &str,
        len_u: LengthUnit,
        grade: &str,
        grade_u: GradeUnit,
        relation: InclusionRelation,
        children: Vec<Intercept>,
    ) -> Intercept {
        Intercept {
            hole: hole.to_string(),
            from: None,
            to: None,
            length: PrintedQty::parse_decimal(len).expect("len"),
            length_unit: len_u,
            grade: PrintedQty::parse_decimal(grade).expect("grade"),
            grade_unit: grade_u,
            metal_symbol: Some("Au".to_string()),
            is_metal_equivalent: false,
            is_downhole_length: true,
            true_width_stated: true,
            relation,
            children,
        }
    }

    #[test]
    fn test_remainder_identity_exact() {
        let host = mk_intercept(
            "SYN-001",
            "18.5",
            LengthUnit::Metres,
            "6.40",
            GradeUnit::GramsPerTonne,
            InclusionRelation::Host,
            vec![
                mk_intercept(
                    "SYN-001",
                    "2.5",
                    LengthUnit::Metres,
                    "24.00",
                    GradeUnit::GramsPerTonne,
                    InclusionRelation::Including,
                    vec![],
                ),
                mk_intercept(
                    "SYN-001",
                    "1.0",
                    LengthUnit::Metres,
                    "15.20",
                    GradeUnit::GramsPerTonne,
                    InclusionRelation::Including,
                    vec![],
                ),
            ],
        );
        let res = remainder_grade(&host).expect("remainder");
        let l_host = host.length.value;
        let g_host = host.grade.value;
        let l1 = host.children[0].length.value;
        let g1 = host.children[0].grade.value;
        let l2 = host.children[1].length.value;
        let g2 = host.children[1].grade.value;

        let lhs = l_host.checked_mul(g_host).unwrap();
        let child_sum = l1
            .checked_mul(g1)
            .unwrap()
            .checked_add(l2.checked_mul(g2).unwrap())
            .unwrap();
        let rem_term = res
            .remainder_length_m
            .checked_mul(res.remainder_grade_point)
            .unwrap();
        assert_eq!(lhs, child_sum.checked_add(rem_term).unwrap());
        assert!(
            res.remainder_grade_interval
                .contains(res.remainder_grade_point)
        );

        // Cross-check our O(k^2) exact extremum against brute-force enumeration of all 2^(2k+2) = 64 vertices!
        let hl = host.length.interval().unwrap();
        let hg = host.grade.interval().unwrap();
        let c0l = host.children[0].length.interval().unwrap();
        let c0g = host.children[0].grade.interval().unwrap();
        let c1l = host.children[1].length.interval().unwrap();
        let c1g = host.children[1].grade.interval().unwrap();
        let mut bf_min: Option<ExactQty> = None;
        let mut bf_max: Option<ExactQty> = None;
        for l_v in [hl.lo, hl.hi] {
            for g_v in [hg.lo, hg.hi] {
                for l0 in [c0l.lo, c0l.hi] {
                    for g0 in [c0g.lo, c0g.hi] {
                        for l1v in [c1l.lo, c1l.hi] {
                            for g1v in [c1g.lo, c1g.hi] {
                                let num = l_v
                                    .checked_mul(g_v)
                                    .unwrap()
                                    .checked_sub(l0.checked_mul(g0).unwrap())
                                    .unwrap()
                                    .checked_sub(l1v.checked_mul(g1v).unwrap())
                                    .unwrap();
                                let den = l_v.checked_sub(l0).unwrap().checked_sub(l1v).unwrap();
                                let v = num.checked_div(den).unwrap();
                                bf_min = Some(match bf_min {
                                    None => v,
                                    Some(cur) if cmp_exact_qty(v, cur) == Ordering::Less => v,
                                    Some(cur) => cur,
                                });
                                bf_max = Some(match bf_max {
                                    None => v,
                                    Some(cur) if cmp_exact_qty(v, cur) == Ordering::Greater => v,
                                    Some(cur) => cur,
                                });
                            }
                        }
                    }
                }
            }
        }
        assert_eq!(res.remainder_grade_interval.lo, bf_min.unwrap());
        assert_eq!(res.remainder_grade_interval.hi, bf_max.unwrap());
    }

    #[test]
    fn test_e656_fm24_209d_remainder_718281_over_14700() {
        // FACT (E656 arithmetic check): FM24-209D "1.6 m @ 30.6 g/t within 16.3 m @ 47.07 g/t".
        let e656 = mk_intercept(
            "FM24-209D",
            "16.3",
            LengthUnit::Metres,
            "47.07",
            GradeUnit::GramsPerTonne,
            InclusionRelation::Host,
            vec![mk_intercept(
                "FM24-209D",
                "1.6",
                LengthUnit::Metres,
                "30.6",
                GradeUnit::GramsPerTonne,
                InclusionRelation::Within,
                vec![],
            )],
        );
        let rem = remainder_grade(&e656).expect("E656 remainder");
        let expected_exact = ExactQty::new(718_281, 14_700).unwrap();
        assert_eq!(rem.remainder_grade_point, expected_exact);
        assert!(rem.remainder_grade_interval.contains(expected_exact));
        assert_eq!(rem.remainder_length_m, ExactQty::new(147, 10).unwrap());

        let ctx =
            ReleaseDisclosureContext::complete(ExactQty::from_integer(1), GradeUnit::GramsPerTonne);
        let audit = audit_drill_release(&[e656], &ctx).expect("audit E656");
        assert!(
            audit.intervals[0]
                .flags
                .contains(&DisclosureFlag::InclusionBelowHost {
                    hole: "FM24-209D".to_string(),
                })
        );
        assert_eq!(audit.flags_by_kind.get("inclusion_below_host"), Some(&1));
    }

    #[test]
    fn test_nested_inclusions_bottom_up() {
        // 3-level nested inclusion tree:
        // 20.0 m @ 4.0 g/t -> including 5.0 m @ 10.0 g/t -> including 1.0 m @ 30.0 g/t
        // Inner remainder: (5*10 - 1*30) / (5 - 1) = 20 / 4 = 5.0 g/t
        // Outer remainder: (20*4 - 5*10) / (20 - 5) = 30 / 15 = 2.0 g/t
        let inner_core = mk_intercept(
            "SYN-NEST-01",
            "1.0",
            LengthUnit::Metres,
            "30.0",
            GradeUnit::GramsPerTonne,
            InclusionRelation::Including,
            vec![],
        );
        let mid_zone = mk_intercept(
            "SYN-NEST-01",
            "5.0",
            LengthUnit::Metres,
            "10.0",
            GradeUnit::GramsPerTonne,
            InclusionRelation::Including,
            vec![inner_core],
        );
        let host = mk_intercept(
            "SYN-NEST-01",
            "20.0",
            LengthUnit::Metres,
            "4.0",
            GradeUnit::GramsPerTonne,
            InclusionRelation::Host,
            vec![mid_zone],
        );

        let res = remainder_grade(&host).expect("nested remainder");
        assert_eq!(res.remainder_grade_point, ExactQty::from_integer(2));
        assert_eq!(res.nested_results.len(), 1);
        assert_eq!(
            res.nested_results[0].remainder_grade_point,
            ExactQty::from_integer(5)
        );
    }

    #[test]
    fn test_feet_to_metres_and_oz_per_st_exact() {
        let len_ft = PrintedQty::exact(ExactQty::from_integer(100));
        let len_m = len_ft.to_metres(LengthUnit::Feet).unwrap();
        assert_eq!(len_m.value, ExactQty::new(3048, 100).unwrap());

        let grade_opt = PrintedQty::exact(ExactQty::new(7, 10).unwrap());
        let (grade_gt, unit) = grade_opt
            .to_canonical_grade(GradeUnit::OzPerShortTon)
            .unwrap();
        assert_eq!(unit, GradeUnit::GramsPerTonne);
        assert_eq!(grade_gt.value, ExactQty::from_integer(24));
    }

    #[test]
    fn test_inclusion_below_host_flag() {
        let host = mk_intercept(
            "SYN-IBH-01",
            "10.0",
            LengthUnit::Metres,
            "8.0",
            GradeUnit::GramsPerTonne,
            InclusionRelation::Host,
            vec![mk_intercept(
                "SYN-IBH-01",
                "2.0",
                LengthUnit::Metres,
                "3.0",
                GradeUnit::GramsPerTonne,
                InclusionRelation::Including,
                vec![],
            )],
        );
        let ctx =
            ReleaseDisclosureContext::complete(ExactQty::from_integer(1), GradeUnit::GramsPerTonne);
        let audit = audit_drill_release(&[host], &ctx).unwrap();
        assert_eq!(audit.flags_by_kind.get("inclusion_below_host"), Some(&1));
    }

    #[test]
    fn test_remainder_below_cutoff_flag() {
        // 10.0 m @ 2.0 g/t including 1.0 m @ 17.3 g/t -> remainder = (20 - 17.3)/9 = 0.3 g/t < 0.5 g/t cutoff
        let host = mk_intercept(
            "SYN-RBC-01",
            "10.0",
            LengthUnit::Metres,
            "2.0",
            GradeUnit::GramsPerTonne,
            InclusionRelation::Host,
            vec![mk_intercept(
                "SYN-RBC-01",
                "1.0",
                LengthUnit::Metres,
                "17.3",
                GradeUnit::GramsPerTonne,
                InclusionRelation::Including,
                vec![],
            )],
        );
        let ctx = ReleaseDisclosureContext::complete(
            ExactQty::new(1, 2).unwrap(),
            GradeUnit::GramsPerTonne,
        );
        let audit = audit_drill_release(&[host], &ctx).unwrap();
        assert_eq!(audit.flags_by_kind.get("remainder_below_cutoff"), Some(&1));
    }

    #[test]
    fn test_no_true_width_flag() {
        let mut host = mk_intercept(
            "SYN-NTW-01",
            "12.0",
            LengthUnit::Metres,
            "3.5",
            GradeUnit::GramsPerTonne,
            InclusionRelation::Host,
            vec![],
        );
        host.true_width_stated = false;
        let ctx = ReleaseDisclosureContext::complete(
            ExactQty::new(1, 2).unwrap(),
            GradeUnit::GramsPerTonne,
        );
        let audit = audit_drill_release(&[host], &ctx).unwrap();
        assert_eq!(audit.flags_by_kind.get("no_true_width"), Some(&1));
    }

    #[test]
    fn test_unstated_cutoff_flag() {
        let host = mk_intercept(
            "SYN-UC-01",
            "12.0",
            LengthUnit::Metres,
            "3.5",
            GradeUnit::GramsPerTonne,
            InclusionRelation::Host,
            vec![],
        );
        let ctx = ReleaseDisclosureContext {
            composite_cutoff: None,
            internal_dilution_stated: false,
            top_cut_stated: true,
            metal_eq_prices_stated: true,
            metal_eq_recoveries_stated: true,
            declared_hole_count: None,
        };
        let audit = audit_drill_release(&[host], &ctx).unwrap();
        assert_eq!(
            audit.flags_by_kind.get("unstated_cutoff_or_dilution"),
            Some(&1)
        );
        assert_eq!(
            audit.release_flags,
            vec![DisclosureFlag::UnstatedCutoffOrDilution {
                missing_cutoff: true,
                missing_internal_dilution: true,
                missing_top_cut: false,
            }]
        );
    }

    #[test]
    fn test_metal_equivalent_unstated_prices_flag() {
        let mut host = mk_intercept(
            "SYN-MEQ-01",
            "15.0",
            LengthUnit::Metres,
            "4.2",
            GradeUnit::GramsPerTonne,
            InclusionRelation::Host,
            vec![],
        );
        host.metal_symbol = Some("AuEq".to_string());
        host.is_metal_equivalent = true;
        let ctx = ReleaseDisclosureContext {
            composite_cutoff: Some((ExactQty::new(1, 2).unwrap(), GradeUnit::GramsPerTonne)),
            internal_dilution_stated: true,
            top_cut_stated: true,
            metal_eq_prices_stated: false,
            metal_eq_recoveries_stated: true,
            declared_hole_count: None,
        };
        let audit = audit_drill_release(&[host], &ctx).unwrap();
        assert_eq!(
            audit.flags_by_kind.get("metal_equivalent_unstated_terms"),
            Some(&1)
        );
    }

    #[test]
    fn test_hole_sequence_gap_flag() {
        let h1 = mk_intercept(
            "SYN-001",
            "10.0",
            LengthUnit::Metres,
            "3.0",
            GradeUnit::GramsPerTonne,
            InclusionRelation::Host,
            vec![],
        );
        let h3 = mk_intercept(
            "SYN-003",
            "12.0",
            LengthUnit::Metres,
            "4.0",
            GradeUnit::GramsPerTonne,
            InclusionRelation::Host,
            vec![],
        );
        let mut ctx = ReleaseDisclosureContext::complete(
            ExactQty::new(1, 2).unwrap(),
            GradeUnit::GramsPerTonne,
        );
        ctx.declared_hole_count = Some(3);
        let audit = audit_drill_release(&[h1, h3], &ctx).unwrap();
        assert_eq!(audit.flags_by_kind.get("hole_sequence_gap"), Some(&1));
        assert_eq!(
            audit.release_flags,
            vec![DisclosureFlag::HoleSequenceGap {
                declared_hole_count: 3,
                observed_hole_count: 2,
                missing_sequence_numbers: vec![2],
            }]
        );
    }

    #[test]
    fn test_clean_release_raises_no_flags() {
        // 10.0 m @ 4.0 g/t including 2.0 m @ 12.0 g/t -> remainder = (40 - 24)/8 = 2.0 g/t >= 0.5 g/t cutoff
        let host = mk_intercept(
            "SYN-CLN-001",
            "10.0",
            LengthUnit::Metres,
            "4.0",
            GradeUnit::GramsPerTonne,
            InclusionRelation::Host,
            vec![mk_intercept(
                "SYN-CLN-001",
                "2.0",
                LengthUnit::Metres,
                "12.0",
                GradeUnit::GramsPerTonne,
                InclusionRelation::Including,
                vec![],
            )],
        );
        let mut ctx = ReleaseDisclosureContext::complete(
            ExactQty::new(1, 2).unwrap(),
            GradeUnit::GramsPerTonne,
        );
        ctx.declared_hole_count = Some(1);
        let audit = audit_drill_release(&[host], &ctx).unwrap();
        assert!(audit.is_clean(), "expected zero flags, got {:?}", audit);
        assert!(
            audit.intervals[0]
                .to_ndjson_line()
                .contains("\"event\":\"intercept\"")
        );
        assert!(
            audit
                .summary_ndjson_line(0)
                .contains("\"event\":\"intercept_summary\"")
        );
    }

    #[test]
    fn test_reordering_intervals_invariant() {
        let c1 = mk_intercept(
            "SYN-A",
            "1.5",
            LengthUnit::Metres,
            "18.0",
            GradeUnit::GramsPerTonne,
            InclusionRelation::Including,
            vec![],
        );
        let c2 = mk_intercept(
            "SYN-A",
            "2.0",
            LengthUnit::Metres,
            "10.0",
            GradeUnit::GramsPerTonne,
            InclusionRelation::Including,
            vec![],
        );
        let h_a1 = mk_intercept(
            "SYN-A",
            "15.0",
            LengthUnit::Metres,
            "5.0",
            GradeUnit::GramsPerTonne,
            InclusionRelation::Host,
            vec![c1.clone(), c2.clone()],
        );
        let h_a2 = mk_intercept(
            "SYN-A",
            "15.0",
            LengthUnit::Metres,
            "5.0",
            GradeUnit::GramsPerTonne,
            InclusionRelation::Host,
            vec![c2, c1],
        );
        let h_b = mk_intercept(
            "SYN-B",
            "8.0",
            LengthUnit::Metres,
            "3.2",
            GradeUnit::GramsPerTonne,
            InclusionRelation::Host,
            vec![],
        );
        let ctx = ReleaseDisclosureContext::complete(
            ExactQty::new(1, 2).unwrap(),
            GradeUnit::GramsPerTonne,
        );
        let a = audit_drill_release(&[h_a1, h_b.clone()], &ctx).unwrap();
        let b = audit_drill_release(&[h_b, h_a2], &ctx).unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn test_unit_change_invariant() {
        // Exact unit change: convert both value and step from (m, g/t) to (ft, oz/st) via exact ratios.
        let to_ft = |p: PrintedQty| PrintedQty {
            value: p.value.metres_to_feet().unwrap(),
            step: p.step.metres_to_feet().unwrap(),
        };
        let to_opt = |p: PrintedQty| PrintedQty {
            value: p.value.g_per_t_to_oz_per_st().unwrap(),
            step: p.step.g_per_t_to_oz_per_st().unwrap(),
        };

        let metric_child = mk_intercept(
            "SYN-U-01",
            "1.6",
            LengthUnit::Metres,
            "30.6",
            GradeUnit::GramsPerTonne,
            InclusionRelation::Within,
            vec![],
        );
        let metric_host = mk_intercept(
            "SYN-U-01",
            "16.3",
            LengthUnit::Metres,
            "47.07",
            GradeUnit::GramsPerTonne,
            InclusionRelation::Host,
            vec![metric_child.clone()],
        );
        let imperial_host = Intercept {
            length: to_ft(metric_host.length),
            length_unit: LengthUnit::Feet,
            grade: to_opt(metric_host.grade),
            grade_unit: GradeUnit::OzPerShortTon,
            children: vec![Intercept {
                length: to_ft(metric_child.length),
                length_unit: LengthUnit::Feet,
                grade: to_opt(metric_child.grade),
                grade_unit: GradeUnit::OzPerShortTon,
                ..metric_child
            }],
            ..metric_host.clone()
        };

        let cutoff_m = ExactQty::from_integer(1);
        let ctx_m = ReleaseDisclosureContext::complete(cutoff_m, GradeUnit::GramsPerTonne);
        let ctx_imp = ReleaseDisclosureContext::complete(
            cutoff_m.g_per_t_to_oz_per_st().unwrap(),
            GradeUnit::OzPerShortTon,
        );

        let a = audit_drill_release(&[metric_host], &ctx_m).unwrap();
        let b = audit_drill_release(&[imperial_host], &ctx_imp).unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn test_overlapping_inclusions_refused() {
        let mut c1 = mk_intercept(
            "SYN-OVL-01",
            "3.0",
            LengthUnit::Metres,
            "10.0",
            GradeUnit::GramsPerTonne,
            InclusionRelation::Including,
            vec![],
        );
        c1.from = Some(PrintedQty::parse_decimal("102.0").unwrap());
        c1.to = Some(PrintedQty::parse_decimal("105.0").unwrap());

        let mut c2 = mk_intercept(
            "SYN-OVL-01",
            "3.0",
            LengthUnit::Metres,
            "12.0",
            GradeUnit::GramsPerTonne,
            InclusionRelation::Including,
            vec![],
        );
        c2.from = Some(PrintedQty::parse_decimal("104.0").unwrap());
        c2.to = Some(PrintedQty::parse_decimal("107.0").unwrap());

        let mut host = mk_intercept(
            "SYN-OVL-01",
            "10.0",
            LengthUnit::Metres,
            "5.0",
            GradeUnit::GramsPerTonne,
            InclusionRelation::Host,
            vec![c1, c2],
        );
        host.from = Some(PrintedQty::parse_decimal("100.0").unwrap());
        host.to = Some(PrintedQty::parse_decimal("110.0").unwrap());

        let err = remainder_grade(&host).expect_err("overlapping inclusions must refuse");
        assert_eq!(err.code, "FMINE-REF-109");
    }

    #[test]
    fn test_zero_remainder_length_refused() {
        // Host 5.0 ± 0.05 ([4.95, 5.05]) vs child 5.0 ± 0.05 -> L_lo - l_hi = -0.10 <= 0 -> refused.
        let host = mk_intercept(
            "SYN-ZRL-01",
            "5.0",
            LengthUnit::Metres,
            "4.0",
            GradeUnit::GramsPerTonne,
            InclusionRelation::Host,
            vec![mk_intercept(
                "SYN-ZRL-01",
                "5.0",
                LengthUnit::Metres,
                "4.0",
                GradeUnit::GramsPerTonne,
                InclusionRelation::Including,
                vec![],
            )],
        );
        let err = remainder_grade(&host).expect_err("zero remainder length must refuse");
        assert_eq!(err.code, "FMINE-REF-109");

        // Also test strictly positive point remainder (5.0 - 4.95 = 0.05) whose rounding box still hits 0:
        // Host 5.0 ([4.95, 5.05]) minus child 4.95 ([4.945, 4.955]) -> L_lo - l_hi = 4.95 - 4.955 = -0.005 <= 0!
        let near_zero = mk_intercept(
            "SYN-ZRL-02",
            "5.0",
            LengthUnit::Metres,
            "4.0",
            GradeUnit::GramsPerTonne,
            InclusionRelation::Host,
            vec![mk_intercept(
                "SYN-ZRL-02",
                "4.95",
                LengthUnit::Metres,
                "4.0",
                GradeUnit::GramsPerTonne,
                InclusionRelation::Including,
                vec![],
            )],
        );
        let err2 = remainder_grade(&near_zero)
            .expect_err("box touching non-positive remainder length must refuse");
        assert_eq!(err2.code, "FMINE-REF-109");
    }
}
