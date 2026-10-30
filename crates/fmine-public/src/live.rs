//! Live-dependency proof (BD-338).
//!
//! **Files (S1).** Every registered file is load-bearing at the gate. For each registered file, a scratch
//! workspace holds copies of the other files, and that file absent, written as zero bytes, or replaced by a valid
//! but foreign xlsx. The loader must refuse each case with `FMINE-REF-502` (exit 8, a catalogued code), count
//! "1 of n registered", and name that file, and no other, under the label that applies: "missing" for an absent
//! file, "digest mismatch" for a changed one. Only a refusal passes; a loader that falls back to other bytes, or
//! re-raises under an unregistered code, fails by name.
//!
//! **Leaves (S2).** Every cell an instance cites is live. Each operand is perturbed alone, on a copy of the
//! parsed sources, and the run repeated. The field located at the leaf must read the new value ("value did not
//! move" otherwise). A field derived from it, or the state, must move ("dead ancestor" otherwise). The state
//! root must move too, which is necessary but never sufficient. Leaves the state did not reach (a refusal before
//! any arithmetic, a stored value that is not whole) are skipped with their reason, never dropped.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::Path;

use fmine_evidence::{Refusal, lookup_refusal_spec};
use fmine_io::styles::NumberFormat;
use fmine_io::xlsx::{CellValue, ExtractedCell, Sheet, build_minimal_xlsx, parse_a1_ref};

use crate::aisc::{
    AnchorRule, Calc, IdentityInstance, IdentityState, Operand, OperandRole, OperandValue,
    ROW_ANCHORS, locate_table, rate_interval_holds, rate_range_text,
};
use crate::sources::IssuerProfile;
use crate::{LoadedSource, PublicAssuranceReport};

/// What a check does to the one file it is about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckKind {
    FileDeleted,
    FileZeroBytes,
    FileForeign,
}

impl CheckKind {
    pub const ALL: [Self; 3] = [Self::FileDeleted, Self::FileZeroBytes, Self::FileForeign];

    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::FileDeleted => "file_deleted",
            Self::FileZeroBytes => "file_zero_bytes",
            Self::FileForeign => "file_foreign",
        }
    }

    /// The refusal label the file must be named under.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::FileDeleted => "missing",
            Self::FileZeroBytes | Self::FileForeign => "digest mismatch",
        }
    }

    fn not_refused(self) -> &'static str {
        match self {
            Self::FileDeleted => "file deletion did not refuse",
            Self::FileZeroBytes => "zero-byte file did not refuse",
            Self::FileForeign => "foreign file did not refuse",
        }
    }
}

/// One check: the loader's answer for one file and one kind.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LiveCheck {
    pub kind: CheckKind,
    pub file: String,
    /// The refusal's code and exit, or `None` when the loader returned `Ok`.
    pub observed: Option<(&'static str, u8)>,
    pub names_only_this_file: bool,
    /// Why the check failed; `None` when it passed.
    pub failure: Option<String>,
}

impl LiveCheck {
    #[must_use]
    pub fn pass(&self) -> bool {
        self.failure.is_none()
    }

    /// One JSON log line.
    #[must_use]
    pub fn to_json(&self) -> String {
        let (code, exit) = self.observed.map_or_else(
            || ("null".to_string(), "null".to_string()),
            |(c, e)| (format!("\"{c}\""), e.to_string()),
        );
        let mut o = format!(
            "{{\"kind\":\"{}\",\"file\":\"{}\",\"expected_code\":\"FMINE-REF-502\",\"expected_exit\":8,\
             \"observed_code\":{code},\"observed_exit\":{exit},\"label\":\"{}\",\"names_only_this_file\":{},\
             \"pass\":{}",
            self.kind.as_str(),
            self.file,
            self.kind.label(),
            self.names_only_this_file,
            self.pass()
        );
        if let Some(f) = &self.failure {
            let _ = write!(o, ",\"failure\":\"{}\"", f.replace('"', "'"));
        }
        o.push('}');
        o
    }
}

/// The checks of one proof, keyed by the ledger anchor of the intact run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LiveReport {
    pub anchor: String,
    pub checks: Vec<LiveCheck>,
}

impl LiveReport {
    /// One line per check, then `{"anchor","files","checks","pass"}`.
    #[must_use]
    pub fn to_jsonl(&self) -> String {
        let mut o = String::new();
        let mut files: Vec<&str> = self.checks.iter().map(|c| c.file.as_str()).collect();
        files.dedup();
        for c in &self.checks {
            o.push_str(&c.to_json());
            o.push('\n');
        }
        let _ = writeln!(
            o,
            "{{\"anchor\":\"{}\",\"files\":{},\"checks\":{},\"pass\":{}}}",
            self.anchor,
            files.len(),
            self.checks.len(),
            self.checks.iter().filter(|c| c.pass()).count()
        );
        o
    }
}

/// Signature of the loader under proof: [`crate::load_registered_sources`], or a wrapper around it.
pub type Loader<'a> = &'a dyn Fn(&IssuerProfile, &Path) -> Result<Vec<LoadedSource>, Refusal>;

/// Runs the three file checks on every registered file of `profile`. The intact files are read from
/// `corpus_root`/`corpus_dir`; each case gets its own workspace under `scratch` (never the real corpus).
///
/// # Errors
/// An I/O error reading an intact file or writing a scratch workspace.
pub fn prove_files(
    profile: &IssuerProfile,
    corpus_root: &Path,
    scratch: &Path,
    load: Loader<'_>,
) -> std::io::Result<Vec<LiveCheck>> {
    let n = profile.sources.len();
    let intact = corpus_root.join(&profile.corpus_dir);
    let names: Vec<&str> = profile
        .sources
        .iter()
        .map(|s| s.file_name.as_str())
        .collect();
    let mut checks = Vec::with_capacity(n * CheckKind::ALL.len());
    for (i, file) in names.iter().enumerate() {
        for kind in CheckKind::ALL {
            let ws = scratch.join(format!("{i}-{}", kind.as_str()));
            let corpus = ws.join(&profile.corpus_dir);
            std::fs::create_dir_all(&corpus)?;
            for other in names.iter().filter(|o| *o != file) {
                std::fs::copy(intact.join(other), corpus.join(other))?;
            }
            match kind {
                CheckKind::FileDeleted => {}
                CheckKind::FileZeroBytes => std::fs::write(corpus.join(file), [])?,
                CheckKind::FileForeign => {
                    std::fs::write(corpus.join(file), build_minimal_xlsx(&[]))?
                }
            }
            checks.push(judge(kind, file, &names, load(profile, &ws)));
        }
    }
    Ok(checks)
}

fn judge(
    kind: CheckKind,
    file: &str,
    names: &[&str],
    result: Result<Vec<LoadedSource>, Refusal>,
) -> LiveCheck {
    let refusal = match result {
        Ok(_) => {
            return LiveCheck {
                kind,
                file: file.to_string(),
                observed: None,
                names_only_this_file: false,
                failure: Some(format!("{}: {file}", kind.not_refused())),
            };
        }
        Err(r) => r,
    };
    let what = refusal.what.as_str();
    let named = what.contains(&format!("{}: {file} (", kind.label()));
    let others = names
        .iter()
        .filter(|o| **o != file)
        .any(|o| what.contains(&format!("{o} (")));
    let names_only_this_file = named && !others;
    let failure = if refusal.exit_code != 8 {
        Some(format!("refusal exit {} is not 8", refusal.exit_code))
    } else if refusal.code != "FMINE-REF-502" || lookup_refusal_spec(refusal.code).is_none() {
        Some(format!(
            "refusal code {} is not FMINE-REF-502",
            refusal.code
        ))
    } else if !what.contains(&format!("1 of {} registered", names.len())) {
        Some(format!(
            "refusal does not count 1 of {} registered",
            names.len()
        ))
    } else if !named {
        Some(format!("{file} not named under \"{}\"", kind.label()))
    } else if others {
        Some(format!("refusal for {file} names another file"))
    } else {
        None
    };
    LiveCheck {
        kind,
        file: file.to_string(),
        observed: Some((refusal.code, refusal.exit_code)),
        names_only_this_file,
        failure,
    }
}

/// How far [`rate_flip_up`] and [`rate_flip_down`] search.
const FLIP_SEARCH: i64 = 1_000_000;

/// The smallest printed rate above `r` that the interval for (n, k, q) excludes: where a consistent rate flips
/// upward. `None` when none lies within the search bound.
#[must_use]
pub fn rate_flip_up(n: i64, k: i64, q: i64, r: i64) -> Option<i64> {
    (r.saturating_add(1)..=r.saturating_add(FLIP_SEARCH))
        .find(|&x| !rate_interval_holds(n, k, q, x))
}

/// The largest printed rate below `r` that the interval for (n, k, q) excludes.
#[must_use]
pub fn rate_flip_down(n: i64, k: i64, q: i64, r: i64) -> Option<i64> {
    (r.saturating_sub(FLIP_SEARCH)..r)
        .rev()
        .find(|&x| !rate_interval_holds(n, k, q, x))
}

/// What the leaf proof does to one operand cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Perturbation {
    /// A whole number plus one.
    PlusOne,
    /// A placeholder set to the number 1.
    ToOne,
    /// A counted number set to the placeholder "-", so k drops by one.
    ToPlaceholder,
    /// A rate consistent on the plain construction, moved to its flip margin ([`rate_flip_up`]).
    RateFlip,
    /// A rate consistent at a stated share, or explained by a reconstruction, moved outside any interval:
    /// 3r + 1000.
    RateFar,
    /// A non-zero NCI line set to zero.
    ToZero,
    /// A label or quote replaced by text no prefix matches.
    BreakText,
}

impl Perturbation {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::PlusOne => "plus_one",
            Self::ToOne => "to_one",
            Self::ToPlaceholder => "to_placeholder",
            Self::RateFlip => "rate_flip",
            Self::RateFar => "rate_far",
            Self::ToZero => "to_zero",
            Self::BreakText => "break_text",
        }
    }
}

/// What must hold after a perturbation, besides the leaf reading its new value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Expect {
    /// The printed total reads this, in the calc and in any footing certificate; the state moves.
    Printed(i64),
    /// The component sum reads this; the state moves.
    Sum(i64),
    /// The count of numeric lines reads this.
    K(i64),
    /// The printed rate reads this; the state moves.
    Rate(i64),
    /// The numerator reads this; the rate range moves.
    N(i64),
    /// The ounces read this; the rate range moves.
    Q(i64),
    /// The state moves (the leaf may stop being an operand, e.g. a broken footnote).
    State,
}

/// One leaf check: one operand of one instance, perturbed alone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LeafCheck {
    /// `file:sheet!cell family`.
    pub instance: String,
    /// `file:sheet!cell`.
    pub leaf: String,
    pub role: OperandRole,
    /// `None` when the leaf was skipped.
    pub perturbation: Option<Perturbation>,
    pub state_before: &'static str,
    pub state_after: Option<&'static str>,
    /// The fields that moved, by name, never their values (RISK-31).
    pub moved: Vec<&'static str>,
    pub root_moved: bool,
    /// Why the leaf was not perturbed.
    pub skipped: Option<&'static str>,
    /// Why the check failed; `None` when it passed or was skipped.
    pub failure: Option<String>,
}

impl LeafCheck {
    #[must_use]
    pub fn pass(&self) -> bool {
        self.skipped.is_none() && self.failure.is_none()
    }

    /// One JSON log line: locators, codes and field names only.
    #[must_use]
    pub fn to_json(&self) -> String {
        let opt = |s: Option<&str>| s.map_or_else(|| "null".to_string(), |s| format!("\"{s}\""));
        let moved: Vec<String> = self.moved.iter().map(|m| format!("\"{m}\"")).collect();
        let mut o = format!(
            "{{\"kind\":\"leaf\",\"instance\":\"{}\",\"leaf\":\"{}\",\"role\":\"{}\",\"perturbation\":{},\
             \"state_before\":\"{}\",\"state_after\":{},\"moved\":[{}],\"root_moved\":{},\"skipped\":{},\"pass\":{}",
            self.instance,
            self.leaf,
            self.role.as_str(),
            opt(self.perturbation.map(Perturbation::as_str)),
            self.state_before,
            opt(self.state_after),
            moved.join(","),
            self.root_moved,
            opt(self.skipped),
            self.pass()
        );
        if let Some(f) = &self.failure {
            let _ = write!(o, ",\"failure\":\"{}\"", f.replace('"', "'"));
        }
        o.push('}');
        o
    }
}

/// `{"leaves","instances","pass","failed","skipped","by_role","by_family","by_perturbation",
/// "skipped_by_reason","elapsed_ms"}`.
#[must_use]
pub fn leaf_summary(checks: &[LeafCheck], elapsed_ms: u128) -> String {
    let render = |m: &BTreeMap<&str, usize>| {
        let parts: Vec<String> = m.iter().map(|(k, v)| format!("\"{k}\":{v}")).collect();
        format!("{{{}}}", parts.join(","))
    };
    let mut by_role = BTreeMap::new();
    let mut by_family = BTreeMap::new();
    let mut by_perturbation = BTreeMap::new();
    let mut skipped = BTreeMap::new();
    let mut instances: Vec<&str> = Vec::new();
    for c in checks {
        *by_role.entry(c.role.as_str()).or_insert(0) += 1;
        let family = c.instance.rsplit_once(' ').map_or("", |(_, f)| f);
        *by_family.entry(family).or_insert(0) += 1;
        if let Some(p) = c.perturbation {
            *by_perturbation.entry(p.as_str()).or_insert(0) += 1;
        }
        if let Some(s) = c.skipped {
            *skipped.entry(s).or_insert(0) += 1;
        }
        if !instances.contains(&c.instance.as_str()) {
            instances.push(&c.instance);
        }
    }
    let failed = checks.iter().filter(|c| c.failure.is_some()).count();
    format!(
        "{{\"leaves\":{},\"instances\":{},\"pass\":{},\"failed\":{failed},\"skipped\":{},\"by_role\":{},\
         \"by_family\":{},\"by_perturbation\":{},\"skipped_by_reason\":{},\"elapsed_ms\":{elapsed_ms}}}",
        checks.len(),
        instances.len(),
        checks.iter().filter(|c| c.pass()).count(),
        skipped.values().sum::<usize>(),
        render(&by_role),
        render(&by_family),
        render(&by_perturbation),
        render(&skipped)
    )
}

/// Refusals that come from the stated-share rule, whose NCI lines and label cells decide them.
const SHARE_RULE_REFUSALS: [&str; 4] = [
    "attributable-basis footnote not found on the sheet",
    "regional numerator needs member weighting",
    "no registered share",
    "registered share not confirmed by label",
];

fn locator(o: &Operand) -> String {
    format!("{}:{}!{}", o.file, o.sheet, o.cell)
}

fn same_cell(a: &Operand, b: &Operand) -> bool {
    a.file == b.file && a.sheet == b.sheet && a.cell == b.cell
}

/// The perturbation for an NCI line: whether any NCI line is non-zero decides the basis, so a line is perturbed
/// only when no other NCI line of the instance is non-zero.
fn nci_plan(
    i: &IdentityInstance,
    o: &Operand,
) -> Result<(Perturbation, OperandValue, Expect), &'static str> {
    let other_nonzero = i.operands.iter().any(|x| {
        x.role == OperandRole::NciLine
            && !same_cell(x, o)
            && matches!(x.value, OperandValue::Int(v) if v != 0)
    });
    if other_nonzero {
        return Err("nci_not_decisive");
    }
    Ok(match o.value {
        OperandValue::Int(x) if x != 0 => {
            (Perturbation::ToZero, OperandValue::Int(0), Expect::State)
        }
        OperandValue::Int(x) => (
            Perturbation::PlusOne,
            OperandValue::Int(x + 1),
            Expect::State,
        ),
        _ => (Perturbation::ToOne, OperandValue::Int(1), Expect::State),
    })
}

/// The perturbation for one leaf of one instance, and what must move; or why the leaf is skipped.
fn plan(
    i: &IdentityInstance,
    o: &Operand,
) -> Result<(Perturbation, OperandValue, Expect), &'static str> {
    use OperandRole as R;
    use OperandValue::{Int, Token, Unrounded};
    use Perturbation as P;
    if matches!(o.value, Unrounded(_)) {
        return Err("leaf_skipped_no_format");
    }
    let bump = |x: &OperandValue| match x {
        Int(v) => (P::PlusOne, Int(v + 1)),
        _ => (P::ToOne, Int(1)),
    };
    let reason = match &i.state {
        IdentityState::NotPublished { reason }
        | IdentityState::Refused { reason }
        | IdentityState::RefusedAt { reason, .. } => Some(*reason),
        _ => None,
    };
    match reason {
        Some("all-zero column") => {
            let (p, v) = bump(&o.value);
            return Ok((p, v, Expect::State));
        }
        Some("rate not published") if o.role == R::Rate => {
            return Ok((P::ToOne, Int(1), Expect::State));
        }
        Some(reason) if SHARE_RULE_REFUSALS.contains(&reason) => {
            return match o.role {
                R::NciLine => nci_plan(i, o),
                R::LabelCheck => Ok((P::BreakText, Token("x".to_string()), Expect::State)),
                _ => Err("refused_by_share_rule"),
            };
        }
        Some(_) => return Err("decided_before_arithmetic"),
        None => {}
    }
    match (o.role, i.calc, &o.value) {
        (R::Total, Calc::Footing { printed, .. }, Int(x)) => {
            Ok((P::PlusOne, Int(x + 1), Expect::Printed(printed + 1)))
        }
        (R::Line, Calc::Footing { component_sum, .. }, v) => {
            let (p, new) = bump(v);
            Ok((p, new, Expect::Sum(component_sum + 1)))
        }
        (
            R::Counted,
            Calc::Footing { k_alt: k, .. } | Calc::Rate(crate::aisc::RateMiss { k, .. }),
            v,
        ) => Ok(match v {
            Int(_) => (P::ToPlaceholder, Token("-".to_string()), Expect::K(k - 1)),
            _ => (P::ToOne, Int(1), Expect::K(k + 1)),
        }),
        (R::Rate, Calc::Rate(m), Int(_)) => match &i.state {
            IdentityState::Consistent { basis: None, .. } => rate_flip_up(m.n, m.k, m.q, m.r)
                .map(|r| (P::RateFlip, Int(r), Expect::Rate(r)))
                .ok_or("no_flip_margin_within_search"),
            IdentityState::Consistent { .. } | IdentityState::Explained { .. } => {
                let r = m.r.saturating_mul(3).saturating_add(1000);
                Ok((P::RateFar, Int(r), Expect::Rate(r)))
            }
            _ => Ok((P::PlusOne, Int(m.r + 1), Expect::Rate(m.r + 1))),
        },
        (R::Numerator, Calc::Rate(m), Int(x)) => Ok((P::PlusOne, Int(x + 1), Expect::N(m.n + 1))),
        (R::Ounces, Calc::Rate(m), Int(x)) => Ok((P::PlusOne, Int(x + 1), Expect::Q(m.q + 1))),
        (R::NciLine, Calc::Rate(_), _) => nci_plan(i, o),
        (R::LabelCheck, _, _) => Ok((P::BreakText, Token("x".to_string()), Expect::State)),
        (R::Component, _, Int(x)) => Ok((P::PlusOne, Int(x + 1), Expect::State)),
        _ => Err("role_outside_calc"),
    }
}

/// Sets one operand's cell in `sources` and recomputes that file's census. False when the file, sheet or cell
/// reference is not loaded.
fn set_leaf(sources: &mut [LoadedSource], o: &Operand, new: &OperandValue) -> bool {
    parse_a1_ref(&o.cell)
        .is_some_and(|(row, col)| edit_cell(sources, &o.file, &o.sheet, row, col, new))
}

/// Sets `(row, col)` of a visible sheet in `sources` and recomputes that file's census.
fn edit_cell(
    sources: &mut [LoadedSource],
    file: &str,
    sheet: &str,
    row: u32,
    col: u32,
    new: &OperandValue,
) -> bool {
    let Some(src) = sources.iter_mut().find(|s| s.file_name == file) else {
        return false;
    };
    let Some(ws) = src.workbook.sheets.iter_mut().find(|s| s.name == sheet) else {
        return false;
    };
    set_cell(ws, row, col, new);
    src.census = src.workbook.cell_census();
    true
}

fn set_cell(sheet: &mut Sheet, row: u32, col: u32, new: &OperandValue) {
    let (value, number_text) = match new {
        OperandValue::Int(x) => (CellValue::Number(*x as f64), Some(x.to_string())),
        OperandValue::Token(t) if t == "-" => (CellValue::Placeholder(t.clone()), None),
        OperandValue::Token(t) | OperandValue::Unrounded(t) => (CellValue::Text(t.clone()), None),
        OperandValue::Blank => {
            sheet.cells_by_coord.remove(&(row, col));
            return;
        }
    };
    let cell = sheet
        .cells_by_coord
        .entry((row, col))
        .or_insert_with(|| ExtractedCell {
            cell_ref: format!("{}{row}", fmine_io::xlsx::col_index_to_letters(col)),
            row,
            col,
            value: CellValue::Number(0.0),
            formula: None,
            number_text: None,
            style: None,
            format: NumberFormat::Builtin(0),
        });
    cell.value = value;
    cell.number_text = number_text;
}

/// The calc fields and state that differ between an instance before and after a perturbation.
fn moved_fields(b: &IdentityInstance, a: &IdentityInstance) -> Vec<&'static str> {
    let mut out = Vec::new();
    match (b.calc, a.calc) {
        (
            Calc::Footing {
                printed: p0,
                component_sum: s0,
                k_alt: k0,
            },
            Calc::Footing {
                printed: p1,
                component_sum: s1,
                k_alt: k1,
            },
        ) => {
            for (name, moved) in [
                ("printed", p0 != p1),
                ("component_sum", s0 != s1),
                ("residual", p0 - s0 != p1 - s1),
                ("k", k0 != k1),
            ] {
                if moved {
                    out.push(name);
                }
            }
        }
        (Calc::Rate(m0), Calc::Rate(m1)) => {
            let range = rate_range_text(m0.n, m0.k, m0.q) != rate_range_text(m1.n, m1.k, m1.q);
            for (name, moved) in [
                ("r", m0.r != m1.r),
                ("n", m0.n != m1.n),
                ("k", m0.k != m1.k),
                ("q", m0.q != m1.q),
                ("range", range),
            ] {
                if moved {
                    out.push(name);
                }
            }
        }
        (x, y) if x != y => out.push("calc"),
        _ => {}
    }
    if a.state != b.state {
        out.push("state");
    }
    out
}

fn judge_leaf(
    mut check: LeafCheck,
    i: &IdentityInstance,
    o: &Operand,
    new: &OperandValue,
    expect: Expect,
    before: &PublicAssuranceReport,
    after: &PublicAssuranceReport,
) -> LeafCheck {
    check.root_moved = before.ledger_anchor.state_root != after.ledger_anchor.state_root;
    let Some(a) = after
        .instances
        .iter()
        .find(|x| x.file == i.file && x.sheet == i.sheet && x.cell == i.cell)
    else {
        check.failure = Some(format!(
            "instance vanished: {} after {}",
            check.instance, check.leaf
        ));
        return check;
    };
    check.state_after = Some(a.state.code());
    check.moved = moved_fields(i, a);
    let state_moved = a.state != i.state;
    // (a) The leaf reads its new value, and so does every field located at it.
    let read = a
        .operands
        .iter()
        .find(|x| same_cell(x, o))
        .map(|x| &x.value);
    let leaf_reads = if expect == Expect::State {
        read.is_none_or(|v| v == new)
    } else {
        read == Some(new)
    };
    let certificate = match &a.state {
        IdentityState::IssuerDiscrepancy(c) => Some(c.printed),
        _ => None,
    };
    let field_reads = match (expect, a.calc) {
        (Expect::Printed(p), Calc::Footing { printed, .. }) => {
            printed == p && certificate.is_none_or(|c| c == p)
        }
        (Expect::Rate(r), Calc::Rate(m)) => m.r == r,
        (Expect::N(n), Calc::Rate(m)) => m.n == n,
        (Expect::Q(q), Calc::Rate(m)) => m.q == q,
        (Expect::Printed(_) | Expect::Rate(_) | Expect::N(_) | Expect::Q(_), _) => false,
        _ => true,
    };
    // (b) A field derived from the leaf, or the state, moves.
    let range_moved = check.moved.contains(&"range");
    let derived = match (expect, a.calc) {
        (Expect::Printed(_) | Expect::Rate(_) | Expect::State, _) => state_moved,
        (Expect::Sum(s), Calc::Footing { component_sum, .. }) => component_sum == s && state_moved,
        (Expect::K(k), Calc::Footing { k_alt, .. }) => k_alt == k,
        (Expect::K(k), Calc::Rate(m)) => m.k == k,
        (Expect::N(_) | Expect::Q(_), Calc::Rate(_)) => range_moved,
        _ => false,
    };
    check.failure = if !(leaf_reads && field_reads) {
        Some(format!("value did not move: {}", check.leaf))
    } else if !derived {
        Some(format!(
            "dead ancestor: {} → {}",
            check.leaf, check.instance
        ))
    } else if !check.root_moved {
        Some(format!("root did not move: {}", check.leaf))
    } else {
        None
    };
    check
}

/// One label check (BD-338.S3): an anchor label of a located table, mutated so it should stop matching.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LabelCheck {
    pub file: String,
    pub sheet: String,
    pub cell: String,
    /// The anchor's role from [`ROW_ANCHORS`], never the cell's own text.
    pub anchor: &'static str,
    /// "append_x", or "prepend_x" for a prefix anchor, which an appended letter can never break.
    pub mutation: &'static str,
    /// "table_refusal", "instances_changed" or "none" (a failure).
    pub effect: &'static str,
}

impl LabelCheck {
    #[must_use]
    pub fn pass(&self) -> bool {
        self.effect != "none"
    }

    #[must_use]
    pub fn to_json(&self) -> String {
        format!(
            "{{\"kind\":\"label\",\"file\":\"{}\",\"sheet\":\"{}\",\"cell\":\"{}\",\"anchor\":\"{}\",\
             \"mutation\":\"{}\",\"effect\":\"{}\",\"pass\":{}}}",
            self.file,
            self.sheet,
            self.cell,
            self.anchor,
            self.mutation,
            self.effect,
            self.pass()
        )
    }
}

/// Mutates each of the seven anchor labels of the table on each `(file, sheet)` of `tables`, one at a time, and
/// reruns `run`. Each must give a new table refusal on that sheet or change its instances. A sheet with no
/// located table gives no checks; the caller counts them.
pub fn prove_labels(
    base: &[LoadedSource],
    run: &dyn Fn(Vec<LoadedSource>) -> PublicAssuranceReport,
    subjects: &[String],
    tables: &[(String, String)],
) -> Vec<LabelCheck> {
    let before = run(base.to_vec());
    let on = |r: &PublicAssuranceReport, file: &str, sheet: &str| {
        let refused = r
            .table_refusals
            .iter()
            .any(|t| t.file == file && t.sheet == sheet);
        let instances: Vec<IdentityInstance> = r
            .instances
            .iter()
            .filter(|i| i.file == file && i.sheet == sheet)
            .cloned()
            .collect();
        (refused, instances)
    };
    let mut out = Vec::new();
    for (file, sheet) in tables {
        let ws = base
            .iter()
            .find(|s| s.file_name == *file)
            .and_then(|s| s.workbook.sheets.iter().find(|w| w.name == *sheet));
        let Some(ws) = ws else { continue };
        let Ok(Some(t)) = locate_table(ws, subjects) else {
            continue;
        };
        let (refused_before, instances_before) = on(&before, file, sheet);
        let rows = [
            t.cos_row,
            t.tcc_row,
            t.aisc_row,
            t.ounces_row,
            t.cos_rate_row,
            t.tcc_rate_row,
            t.aisc_rate_row,
        ];
        for (row, anchor) in rows.into_iter().zip(&ROW_ANCHORS) {
            let text = match ws.cell(row, 1).map(|c| &c.value) {
                Some(CellValue::Text(s)) => s.clone(),
                _ => continue,
            };
            let (mutation, new) = match anchor.rule {
                AnchorRule::Prefix => ("prepend_x", format!("x{text}")),
                AnchorRule::Exact => ("append_x", format!("{text}x")),
            };
            let mut sources = base.to_vec();
            if !edit_cell(&mut sources, file, sheet, row, 1, &OperandValue::Token(new)) {
                continue;
            }
            let (refused, instances) = on(&run(sources), file, sheet);
            let effect = if refused && !refused_before {
                "table_refusal"
            } else if instances != instances_before {
                "instances_changed"
            } else {
                "none"
            };
            out.push(LabelCheck {
                file: file.clone(),
                sheet: sheet.clone(),
                cell: format!("A{row}"),
                anchor: anchor.role,
                mutation,
                effect,
            });
        }
    }
    out
}

/// Runs a leaf check on every operand of every visible instance of `run(base)`. `run` is the pipeline under
/// proof (`assess` on a profile, or a wrapper around it). Each check gets its own copy of `base` with one cell
/// changed and its census recomputed. Hidden-scope instances are not checked.
pub fn prove_leaves(
    base: &[LoadedSource],
    run: &dyn Fn(Vec<LoadedSource>) -> PublicAssuranceReport,
) -> Vec<LeafCheck> {
    prove_leaves_in(base, run, &|_, _, _| true)
}

/// The sample frame of the real lane (BD-338.S3): one leaf per instance, the operand at index i mod n of the
/// i-th instance (n operands), so every instance and, across instances, every role is reached.
#[must_use]
pub fn one_leaf_per_instance(instance: usize, operand: usize, n_operands: usize) -> bool {
    n_operands > 0 && operand == instance % n_operands
}

/// [`prove_leaves`] on the leaves `select(instance index, operand index, operand count)` admits. Leaves
/// outside the frame are not reported; the caller records the frame.
pub fn prove_leaves_in(
    base: &[LoadedSource],
    run: &dyn Fn(Vec<LoadedSource>) -> PublicAssuranceReport,
    select: &dyn Fn(usize, usize, usize) -> bool,
) -> Vec<LeafCheck> {
    let before = run(base.to_vec());
    let mut out = Vec::new();
    for (ii, i) in before.instances.iter().enumerate() {
        for (oi, o) in i.operands.iter().enumerate() {
            if !select(ii, oi, i.operands.len()) {
                continue;
            }
            let check = LeafCheck {
                instance: format!("{}:{}!{} {}", i.file, i.sheet, i.cell, i.family.as_str()),
                leaf: locator(o),
                role: o.role,
                perturbation: None,
                state_before: i.state.code(),
                state_after: None,
                moved: Vec::new(),
                root_moved: false,
                skipped: None,
                failure: None,
            };
            out.push(match plan(i, o) {
                Err(why) => LeafCheck {
                    skipped: Some(why),
                    ..check
                },
                Ok((p, new, expect)) => {
                    let mut sources = base.to_vec();
                    if set_leaf(&mut sources, o, &new) {
                        let after = run(sources);
                        let check = LeafCheck {
                            perturbation: Some(p),
                            ..check
                        };
                        judge_leaf(check, i, o, &new, expect, &before, &after)
                    } else {
                        LeafCheck {
                            skipped: Some("leaf_not_in_loaded_sheet"),
                            ..check
                        }
                    }
                }
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sources::load_issuer_profile;
    use crate::{Lane, load_registered_sources, run_public_assurance, synthetic};
    use fmine_core::hash::sha256;
    use std::path::PathBuf;

    /// A fresh synthetic registered workspace (three files, issuer `synthetic`) and its profile.
    fn workspace(tag: &str) -> (PathBuf, IssuerProfile) {
        let ws = std::env::temp_dir().join(format!("fmine-live-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&ws);
        synthetic::write_issuer_workspace(&ws).expect("write workspace");
        let profile = load_issuer_profile(&ws, "synthetic").expect("generated registry loads");
        (ws, profile)
    }

    fn log(tag: &str, profile: &IssuerProfile, ws: &Path, checks: &[LiveCheck]) {
        let anchor = run_public_assurance(profile, ws)
            .expect("intact")
            .ledger_anchor
            .anchor_id()
            .to_string();
        let report = LiveReport {
            anchor: anchor.clone(),
            checks: checks.to_vec(),
        };
        let dir = std::env::temp_dir().join("live_dependency");
        let _ = std::fs::create_dir_all(&dir);
        let _ = std::fs::write(dir.join(format!("{anchor}-{tag}.jsonl")), report.to_jsonl());
        print!("{}", report.to_jsonl());
    }

    #[test]
    fn test_synthetic_registered_profile_digests_match_written_bytes() {
        let (ws, profile) = workspace("digests");
        assert_eq!(profile.sources.len(), 3);
        for s in &profile.sources {
            let bytes =
                std::fs::read(ws.join(&profile.corpus_dir).join(&s.file_name)).expect("written");
            assert_eq!(bytes.len() as u64, s.bytes, "{}", s.file_name);
            assert_eq!(sha256(&bytes).to_hex(), s.sha256_hex, "{}", s.file_name);
        }
        let _ = std::fs::remove_dir_all(&ws);
    }

    #[test]
    fn test_synthetic_registered_profile_runs_as_synthetic_lane() {
        let (ws, profile) = workspace("lane");
        let r = run_public_assurance(&profile, &ws).expect("intact");
        assert_eq!(r.lane, Lane::Synthetic);
        assert!(!r.gate_eligible());
        let json = r.to_json();
        assert!(!json.contains("\"lane\":\"public\""), "{json}");
        assert!(!json.contains("\"gate_eligible\":true"), "{json}");
        let _ = std::fs::remove_dir_all(&ws);
    }

    /// Positive control: 3 files × {absent, 0 bytes, foreign xlsx} = 9 checks, every one refused as required.
    #[test]
    fn test_live_files_synthetic_all_refuse() {
        let (ws, profile) = workspace("all");
        let checks = prove_files(&profile, &ws, &ws.join("scratch"), &load_registered_sources)
            .expect("scratch workspaces");
        log("all", &profile, &ws, &checks);
        assert_eq!(checks.len(), 9);
        for c in &checks {
            assert!(c.pass(), "{}", c.to_json());
            assert_eq!(c.observed, Some(("FMINE-REF-502", 8)));
            assert!(c.names_only_this_file);
        }
        let _ = std::fs::remove_dir_all(&ws);
    }

    #[test]
    fn test_live_files_refusal_names_only_the_removed_file() {
        let (ws, profile) = workspace("names");
        let checks = prove_files(&profile, &ws, &ws.join("scratch"), &load_registered_sources)
            .expect("scratch workspaces");
        let c = checks
            .iter()
            .find(|c| c.kind == CheckKind::FileDeleted && c.file == synthetic::SYNTHETIC_AISC_FILE)
            .expect("check");
        assert!(c.pass() && c.names_only_this_file, "{}", c.to_json());
        // The same refusal, read directly: the counted prefix and the label, and no other file name.
        let scratch = ws.join("scratch").join("1-file_deleted");
        let r = load_registered_sources(&profile, &scratch).expect_err("refuses");
        assert!(r.what.contains("1 of 3 registered"), "{}", r.what);
        assert!(
            r.what.contains("missing: Synthetic_AISC.xlsx ("),
            "{}",
            r.what
        );
        for other in [
            synthetic::SYNTHETIC_STATS_FILE,
            synthetic::SYNTHETIC_LATER_STATS_FILE,
        ] {
            assert!(!r.what.contains(other), "{}", r.what);
        }
        let _ = std::fs::remove_dir_all(&ws);
    }

    /// M3, silent fallback: a loader that answers a refusal by putting the intact bytes back is caught for every
    /// file, by name.
    #[test]
    fn test_live_files_mutant_m3_silent_fallback_is_caught() {
        let (ws, profile) = workspace("m3");
        let intact = ws.join(&profile.corpus_dir);
        let fallback = |p: &IssuerProfile, root: &Path| match load_registered_sources(p, root) {
            Ok(v) => Ok(v),
            Err(_) => {
                for s in &p.sources {
                    let _ = std::fs::copy(
                        intact.join(&s.file_name),
                        root.join(&p.corpus_dir).join(&s.file_name),
                    );
                }
                load_registered_sources(p, root)
            }
        };
        let checks =
            prove_files(&profile, &ws, &ws.join("scratch"), &fallback).expect("scratch workspaces");
        log("m3", &profile, &ws, &checks);
        assert!(checks.iter().all(|c| !c.pass()));
        let failures: Vec<&str> = checks.iter().filter_map(|c| c.failure.as_deref()).collect();
        for file in [
            synthetic::SYNTHETIC_AISC_FILE,
            synthetic::SYNTHETIC_STATS_FILE,
            synthetic::SYNTHETIC_LATER_STATS_FILE,
        ] {
            assert!(
                failures.contains(&format!("file deletion did not refuse: {file}").as_str()),
                "{failures:?}"
            );
        }
        let _ = std::fs::remove_dir_all(&ws);
    }

    /// M4, unregistered code: re-raising under a code the catalogue lacks degrades to FMINE-REF-000, exit 2.
    #[test]
    fn test_live_files_mutant_m4_unregistered_code_is_caught() {
        let (ws, profile) = workspace("m4");
        let reraise = |p: &IssuerProfile, root: &Path| {
            load_registered_sources(p, root)
                .map_err(|r| Refusal::from_code("FMINE-REF-5O2", r.what))
        };
        let checks =
            prove_files(&profile, &ws, &ws.join("scratch"), &reraise).expect("scratch workspaces");
        log("m4", &profile, &ws, &checks);
        assert_eq!(checks.len(), 9);
        for c in &checks {
            assert_eq!(
                c.failure.as_deref(),
                Some("refusal exit 2 is not 8"),
                "{}",
                c.to_json()
            );
            assert_eq!(c.observed, Some(("FMINE-REF-000", 2)));
        }
        let _ = std::fs::remove_dir_all(&ws);
    }

    /// An unreadable file is named. Today the loader files a read error under "missing" (`lib.rs`), and this
    /// test records that label.
    #[cfg(unix)]
    #[test]
    fn test_live_files_unreadable_file_is_named() {
        use std::os::unix::fs::PermissionsExt as _;
        let (ws, profile) = workspace("unreadable");
        let path = ws
            .join(&profile.corpus_dir)
            .join(synthetic::SYNTHETIC_AISC_FILE);
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o000)).expect("chmod");
        if std::fs::read(&path).is_ok() {
            // Running as root: permissions do not stop the read, so there is nothing to observe. Say so.
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).expect("chmod");
            let _ = std::fs::remove_dir_all(&ws);
            println!(
                "{{\"test\":\"unreadable_file_is_named\",\"skipped\":\"chmod 000 file still readable (root)\"}}"
            );
            return;
        }
        let r = load_registered_sources(&profile, &ws).expect_err("refuses");
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).expect("chmod");
        assert_eq!((r.code, r.exit_code), ("FMINE-REF-502", 8));
        assert!(r.what.contains("1 of 3 registered"), "{}", r.what);
        assert!(
            r.what.contains("missing: Synthetic_AISC.xlsx ("),
            "{}",
            r.what
        );
        let _ = std::fs::remove_dir_all(&ws);
    }

    // S2: leaves. Every expected value below is read from the seeded answers in `synthetic.rs`, never from a run.

    fn synthetic_sources() -> Vec<LoadedSource> {
        crate::load_synthetic(synthetic::synthetic_workbooks(&[]))
            .expect("synthetic workbooks parse")
    }

    fn assess_synthetic(sources: Vec<LoadedSource>) -> PublicAssuranceReport {
        crate::assess(&synthetic::synthetic_profile(), Lane::Synthetic, sources)
    }

    fn aisc(cell: &str) -> String {
        format!(
            "{}:{}!{cell}",
            synthetic::SYNTHETIC_AISC_FILE,
            synthetic::SYNTHETIC_TABLE_SHEET
        )
    }

    /// The synthetic run after setting one AISC-sheet cell.
    fn edited(cell: &str, value: OperandValue) -> PublicAssuranceReport {
        let mut sources = synthetic_sources();
        let o = Operand {
            role: OperandRole::Line,
            file: synthetic::SYNTHETIC_AISC_FILE.to_string(),
            sheet: synthetic::SYNTHETIC_TABLE_SHEET.to_string(),
            cell: cell.to_string(),
            value: OperandValue::Blank,
        };
        assert!(set_leaf(&mut sources, &o, &value));
        assess_synthetic(sources)
    }

    fn state_at<'a>(r: &'a PublicAssuranceReport, cell: &str) -> &'a IdentityState {
        match r.instances.iter().find(|i| i.cell == cell) {
            Some(i) => &i.state,
            None => panic!("no instance at {cell}"),
        }
    }

    fn write_leaf_log(tag: &str, checks: &[LeafCheck], elapsed_ms: u128) -> String {
        let mut log: String = checks.iter().map(|c| c.to_json() + "\n").collect();
        log.push_str(&leaf_summary(checks, elapsed_ms));
        log.push('\n');
        let dir = std::env::temp_dir().join("live_dependency");
        let _ = std::fs::create_dir_all(&dir);
        let _ = std::fs::write(dir.join(format!("synthetic-leaves-{tag}.jsonl")), &log);
        log
    }

    /// (79, 1, 68) spans [1145.985, 1177.778]: 1146 and 1178 hold, 1145 and 1179 do not.
    #[test]
    fn test_flip_margin_rate_known_answer() {
        assert!(rate_interval_holds(79, 1, 68, 1146) && rate_interval_holds(79, 1, 68, 1178));
        assert!(!rate_interval_holds(79, 1, 68, 1145) && !rate_interval_holds(79, 1, 68, 1179));
        assert_eq!(rate_flip_up(79, 1, 68, 1162), Some(1179));
        assert_eq!(rate_flip_down(79, 1, 68, 1162), Some(1145));
    }

    /// B8 prints 400 against 500 − 100 + 0; 401 leaves a residual of 1.
    #[test]
    fn test_flip_margin_footing_plus_one_gives_residual_one() {
        let r = edited("B8", OperandValue::Int(401));
        match state_at(&r, "B8") {
            IdentityState::IssuerDiscrepancy(c) => assert_eq!((c.printed, c.residual), (401, 1)),
            other => panic!("B8: {other:?}"),
        }
    }

    /// E7 −160 → 0: E14 is checked plainly, 1000·[999.5, 1000.5]/[399.5, 400.5] spans [2495.63, 2504.38] and holds
    /// 2500; E8's lines now sum to 800 against 640 printed.
    #[test]
    fn test_flip_margin_nci_to_zero_evaluates_cos_rate() {
        let base = assess_synthetic(synthetic_sources());
        assert_eq!(state_at(&base, "E14").code(), "v");
        let r = edited("E7", OperandValue::Int(0));
        assert_eq!(state_at(&r, "E14").code(), "i");
        match state_at(&r, "E8") {
            IdentityState::IssuerDiscrepancy(c) => {
                assert_eq!((c.printed, c.component_sum), (640, 800))
            }
            other => panic!("E8: {other:?}"),
        }
    }

    /// F5 0 → 1: no instance of column F is an all-zero column any more.
    #[test]
    fn test_flip_margin_all_zero_column_leaves_np() {
        let all_zero = IdentityState::NotPublished {
            reason: "all-zero column",
        };
        let base = assess_synthetic(synthetic_sources());
        let r = edited("F5", OperandValue::Int(1));
        for cell in ["F8", "F11", "F14", "F15", "F16"] {
            assert_eq!(state_at(&base, cell), &all_zero);
            assert_ne!(state_at(&r, cell), &all_zero, "{cell}");
        }
    }

    /// Region North's reconstruction checks `Minestats3!B2` starts "Site G"; broken, H16 is (iv) and names the cell.
    #[test]
    fn test_flip_margin_label_check_breaks_reconstruction() {
        let mut sources = synthetic_sources();
        let o = Operand {
            role: OperandRole::LabelCheck,
            file: synthetic::SYNTHETIC_STATS_FILE.to_string(),
            sheet: "Minestats3".to_string(),
            cell: "B2".to_string(),
            value: OperandValue::Blank,
        };
        assert!(set_leaf(
            &mut sources,
            &o,
            &OperandValue::Token("x".to_string())
        ));
        let r = assess_synthetic(sources);
        let s = state_at(&r, "H16");
        assert_eq!(s.code(), "iv");
        assert!(s.detail().contains("Minestats3!B2 reads"), "{}", s.detail());
    }

    /// A parsed-level edit recomputes the file's census: Z99 = 1 changes the numeric digest.
    #[test]
    fn test_live_leaves_census_recomputed() {
        let base = synthetic_sources();
        let mut sources = base.clone();
        let o = Operand {
            role: OperandRole::Line,
            file: synthetic::SYNTHETIC_AISC_FILE.to_string(),
            sheet: synthetic::SYNTHETIC_TABLE_SHEET.to_string(),
            cell: "Z99".to_string(),
            value: OperandValue::Blank,
        };
        assert!(set_leaf(&mut sources, &o, &OperandValue::Int(1)));
        let file = |s: &[LoadedSource]| {
            s.iter()
                .find(|x| x.file_name == synthetic::SYNTHETIC_AISC_FILE)
                .map(|x| x.census)
                .expect("aisc")
        };
        assert_ne!(file(&base).numeric_digest, file(&sources).numeric_digest);
        assert_eq!(file(&sources).numeric_cells, file(&base).numeric_cells + 1);
    }

    /// The same edit made in the xlsx bytes and in the parsed cells gives the same instances and root: C8 = 351 in
    /// the AISC workbook, and Site H's `QTD` label broken in the Mine Stats workbook.
    #[test]
    fn test_live_leaves_byte_level_matches_parsed_level() {
        // The root also binds each file's sha256, which only the byte-level edit changes, so it is not compared.
        let compare = |bytes: Vec<LoadedSource>, parsed: Vec<LoadedSource>| {
            let censuses = |s: &[LoadedSource]| s.iter().map(|x| x.census).collect::<Vec<_>>();
            assert_eq!(censuses(&bytes), censuses(&parsed));
            let (a, b) = (assess_synthetic(bytes), assess_synthetic(parsed));
            assert_eq!(a.instances, b.instances);
        };
        let at = |file: &str, sheet: &str, cell: &str| Operand {
            role: OperandRole::Line,
            file: file.to_string(),
            sheet: sheet.to_string(),
            cell: cell.to_string(),
            value: OperandValue::Blank,
        };
        let bytes = crate::load_synthetic(synthetic::synthetic_workbooks(&[(
            "C8".to_string(),
            fmine_io::xlsx::MinimalCell::Number(351.0),
        )]))
        .expect("parse");
        let mut parsed = synthetic_sources();
        let c8 = at(
            synthetic::SYNTHETIC_AISC_FILE,
            synthetic::SYNTHETIC_TABLE_SHEET,
            "C8",
        );
        assert!(set_leaf(&mut parsed, &c8, &OperandValue::Int(351)));
        compare(bytes, parsed);

        let mut files = synthetic::synthetic_workbooks(&[]);
        for f in &mut files {
            if f.0 == synthetic::SYNTHETIC_STATS_FILE {
                f.3 = synthetic::synthetic_stats_workbook("x");
            }
        }
        let bytes = crate::load_synthetic(files).expect("parse");
        let mut parsed = synthetic_sources();
        let a5 = at(synthetic::SYNTHETIC_STATS_FILE, "QTD", "A5");
        assert!(set_leaf(
            &mut parsed,
            &a5,
            &OperandValue::Token("x".to_string())
        ));
        compare(bytes, parsed);
    }

    /// Positive control: every cited leaf of every visible synthetic instance moves what it should. Skips are
    /// only where the seeded answers put a state before any arithmetic:
    /// - Site F's unrounded column (G), all five instances;
    /// - Site D's cost of sales per ounce (E14), refused by the share rule: its rate, ounces and numerator. Its
    ///   NCI line and footnote are checked.
    #[test]
    fn test_live_leaves_synthetic_all_pass() {
        let start = std::time::Instant::now();
        let checks = prove_leaves(&synthetic_sources(), &assess_synthetic);
        let log = write_leaf_log("all", &checks, start.elapsed().as_millis());
        print!("{log}");
        let failures: Vec<&str> = checks.iter().filter_map(|c| c.failure.as_deref()).collect();
        assert!(failures.is_empty(), "{failures:#?}");
        let mut skipped: Vec<String> = checks
            .iter()
            .filter(|c| c.skipped.is_some())
            .map(|c| {
                c.instance
                    .rsplit_once(' ')
                    .map_or("", |(at, _)| at)
                    .to_string()
            })
            .collect();
        skipped.sort();
        skipped.dedup();
        let mut want: Vec<String> = ["G8", "G11", "G14", "G15", "G16", "E14"]
            .iter()
            .map(|c| aisc(c))
            .collect();
        want.sort();
        assert_eq!(skipped, want);
        let e14: Vec<(&str, Option<&str>)> = checks
            .iter()
            .filter(|c| c.instance.starts_with(&aisc("E14")))
            .map(|c| (c.role.as_str(), c.skipped))
            .collect();
        assert_eq!(
            e14,
            [
                ("rate", Some("refused_by_share_rule")),
                ("ounces", Some("refused_by_share_rule")),
                ("numerator", Some("refused_by_share_rule")),
                ("nci_line", None),
                ("label_check", None),
            ]
        );
        // 50 instances; all but Site F's five have a checked leaf.
        let instances: Vec<&str> = checks.iter().map(|c| c.instance.as_str()).collect();
        let mut distinct = instances.clone();
        distinct.dedup();
        assert_eq!(distinct.len(), 50);
        assert!(checks.iter().filter(|c| c.pass()).count() > 250);
    }

    /// A counted line is perturbed by token, not by value. B15 is 1000·400/400 with k = 3 (B5, B6, B7). With B6 as
    /// "-", k = 2, so the range [996.25, 1003.76] narrows but still holds 1000: only k and the range move.
    #[test]
    fn test_live_leaves_count_operand_perturbed_by_token() {
        let checks = prove_leaves(&synthetic_sources(), &assess_synthetic);
        let Some(c) = checks
            .iter()
            .find(|c| c.instance == format!("{} tcc_per_oz", aisc("B15")) && c.leaf == aisc("B6"))
        else {
            panic!("no check for B6 under B15");
        };
        assert_eq!(c.role, OperandRole::Counted);
        assert_eq!(c.perturbation, Some(Perturbation::ToPlaceholder));
        assert_eq!(c.moved, ["k", "range"]);
        assert_eq!((c.state_before, c.state_after), ("i", Some("i")));
        assert!(c.pass(), "{}", c.to_json());
    }

    fn barrick() -> (IssuerProfile, Vec<LoadedSource>) {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..");
        let profile = load_issuer_profile(&root, "barrick")
            .and_then(|p| p.through("2026Q2"))
            .expect("registry");
        let sources = load_registered_sources(&profile, &root)
            .expect("registered files present and unaltered");
        (profile, sources)
    }

    /// BD-338.S3, leaves, on the real lane: frame `one_leaf_per_instance`, so 2,595 leaves, one per instance.
    /// Prints the summary and any failure. Both carry locators, codes and counts, never cell values (RISK-31).
    #[test]
    #[ignore = "needs the registered Barrick workbooks (through 2026Q2) under corpus_of_input_docs/downloaded/mine_stats/"]
    fn barrick_live_leaves_one_per_instance() {
        let (profile, sources) = barrick();
        let run = |s: Vec<LoadedSource>| crate::assess(&profile, Lane::Public, s);
        let start = std::time::Instant::now();
        let checks = prove_leaves_in(&sources, &run, &one_leaf_per_instance);
        let summary = leaf_summary(&checks, start.elapsed().as_millis());
        println!("{{\"frame\":\"one_leaf_per_instance\",\"summary\":{summary}}}");
        let failures: Vec<&str> = checks.iter().filter_map(|c| c.failure.as_deref()).collect();
        for f in &failures {
            println!("{f}");
        }
        assert_eq!(checks.len(), 2595);
        assert!(failures.is_empty(), "{} failures", failures.len());
    }

    /// BD-338.S3, files, on the real lane: 18 registered workbooks × {absent, zero bytes, foreign xlsx} = 54
    /// checks, each refusing `FMINE-REF-502`, exit 8, "1 of 18 registered", naming only that file.
    #[test]
    #[ignore = "needs the registered Barrick workbooks (through 2026Q2) under corpus_of_input_docs/downloaded/mine_stats/"]
    fn barrick_live_files_all_refuse() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..");
        let profile = load_issuer_profile(&root, "barrick")
            .and_then(|p| p.through("2026Q2"))
            .expect("registry");
        assert_eq!(profile.sources.len(), 18);
        let scratch =
            std::env::temp_dir().join(format!("fmine-live-barrick-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&scratch);
        let checks = prove_files(&profile, &root, &scratch, &load_registered_sources)
            .expect("scratch workspaces");
        let _ = std::fs::remove_dir_all(&scratch);
        for c in &checks {
            println!("{}", c.to_json());
        }
        assert_eq!(checks.len(), 54);
        let failures: Vec<&str> = checks.iter().filter_map(|c| c.failure.as_deref()).collect();
        assert!(failures.is_empty(), "{failures:?}");
    }

    /// BD-338.S3, labels, on the real lane: the seven anchor labels of the current-quarter table in each of the 8
    /// AISC workbooks (`'Q2 2026'` in `Barrick_Q2_2026_AISC.xlsx`, and so on): 56 runs, none without effect.
    #[test]
    #[ignore = "needs the registered Barrick workbooks (through 2026Q2) under corpus_of_input_docs/downloaded/mine_stats/"]
    fn barrick_live_label_sample_never_no_effect() {
        let (profile, sources) = barrick();
        let tables: Vec<(String, String)> = sources
            .iter()
            .filter(|s| s.kind == crate::sources::SourceKind::AiscReconciliation)
            .filter_map(|s| {
                let (year, q) = s.quarter.split_once('Q')?;
                Some((s.file_name.clone(), format!("Q{q} {year}")))
            })
            .collect();
        assert_eq!(tables.len(), 8);
        let run = |s: Vec<LoadedSource>| crate::assess(&profile, Lane::Public, s);
        let checks = prove_labels(&sources, &run, &profile.subjects, &tables);
        for c in &checks {
            println!("{}", c.to_json());
        }
        assert_eq!(checks.len(), 56);
        let none: Vec<String> = checks
            .iter()
            .filter(|c| !c.pass())
            .map(LabelCheck::to_json)
            .collect();
        assert!(none.is_empty(), "{none:#?}");
    }

    /// The ounces anchor is a prefix: appending "x" to synthetic A13 changes nothing, and prepending "x" refuses
    /// the table with "no 'Ounces sold' row".
    #[test]
    fn test_label_perturbation_prefix_label_is_prepended() {
        let base = assess_synthetic(synthetic_sources());
        let label = "Ounces sold - attributable basis (000s ounces)";
        let appended = edited("A13", OperandValue::Token(format!("{label}x")));
        assert_eq!(appended.instances, base.instances);
        assert!(appended.table_refusals.is_empty());
        let prepended = edited("A13", OperandValue::Token(format!("x{label}")));
        let refusals: Vec<(&str, &str)> = prepended
            .table_refusals
            .iter()
            .map(|t| (t.sheet.as_str(), t.reason.as_str()))
            .collect();
        assert_eq!(
            refusals,
            [(synthetic::SYNTHETIC_TABLE_SHEET, "no 'Ounces sold' row")]
        );
        // All seven anchors of the synthetic table, each mutated alone, have an effect.
        let tables = [(
            synthetic::SYNTHETIC_AISC_FILE.to_string(),
            synthetic::SYNTHETIC_TABLE_SHEET.to_string(),
        )];
        let checks = prove_labels(
            &synthetic_sources(),
            &assess_synthetic,
            &synthetic::synthetic_profile().subjects,
            &tables,
        );
        assert_eq!(checks.len(), 7);
        assert!(checks.iter().all(LabelCheck::pass), "{checks:#?}");
        assert_eq!(checks[3].mutation, "prepend_x");
        assert_eq!(checks[3].effect, "table_refusal");
    }

    /// The real M1 (B7 H-P3-6): Kibali's `'Q2 2024'!O11` in `Barrick_Q3_2024_AISC.xlsx` kept at its literal value,
    /// 70 (E822). The literal equals the cell, so only perturbing the cell rejects it.
    #[test]
    #[ignore = "needs the registered Barrick workbooks (through 2026Q2) under corpus_of_input_docs/downloaded/mine_stats/"]
    fn barrick_live_leaves_mutant_m1_literal_is_caught() {
        let (profile, sources) = barrick();
        let is_o11 = |i: &IdentityInstance| {
            i.file == "Barrick_Q3_2024_AISC.xlsx" && i.sheet == "Q2 2024" && i.cell == "O11"
        };
        let literal = |s: Vec<LoadedSource>| {
            let mut r = crate::assess(&profile, Lane::Public, s);
            for i in &mut r.instances {
                if let (true, IdentityState::IssuerDiscrepancy(c)) = (is_o11(i), &mut i.state) {
                    c.printed = 70;
                }
            }
            r
        };
        let base = crate::assess(&profile, Lane::Public, sources.clone());
        let Some(at) = base.instances.iter().position(is_o11) else {
            panic!("no instance at O11");
        };
        assert_eq!(base.instances[at].state.code(), "iii");
        let checks = prove_leaves_in(&sources, &literal, &|i, _, _| i == at);
        let failures: Vec<&str> = checks.iter().filter_map(|c| c.failure.as_deref()).collect();
        assert_eq!(
            failures,
            ["value did not move: Barrick_Q3_2024_AISC.xlsx:Q2 2024!O11"]
        );
        // Without the mutant, every leaf of O11 passes.
        let honest = prove_leaves_in(
            &sources,
            &|s| crate::assess(&profile, Lane::Public, s),
            &|i, _, _| i == at,
        );
        assert!(honest.iter().all(LeafCheck::pass), "{}", honest.len());
    }

    /// M1: C8's certificate keeps the literal 350, its real value, so only a perturbation can tell.
    #[test]
    fn test_live_leaves_mutant_m1_literal_is_caught() {
        let literal = |sources: Vec<LoadedSource>| {
            let mut r = assess_synthetic(sources);
            for i in &mut r.instances {
                if let (true, IdentityState::IssuerDiscrepancy(c)) = (i.cell == "C8", &mut i.state)
                {
                    c.printed = 350;
                }
            }
            r
        };
        let checks = prove_leaves(&synthetic_sources(), &literal);
        let failures: Vec<&str> = checks.iter().filter_map(|c| c.failure.as_deref()).collect();
        assert_eq!(failures, [format!("value did not move: {}", aisc("C8"))]);
    }

    /// M2: B8 also cites P5, read honestly but used by nothing.
    #[test]
    fn test_live_leaves_mutant_m2_dead_ancestor_is_caught() {
        let extra = |sources: Vec<LoadedSource>| {
            let p5 = sources
                .iter()
                .find(|s| s.file_name == synthetic::SYNTHETIC_AISC_FILE)
                .and_then(|s| {
                    s.workbook
                        .sheets
                        .iter()
                        .find(|w| w.name == synthetic::SYNTHETIC_TABLE_SHEET)
                })
                .map(|w| {
                    crate::aisc::operand_at(
                        synthetic::SYNTHETIC_AISC_FILE,
                        w,
                        5,
                        16,
                        OperandRole::Line,
                    )
                });
            let mut r = assess_synthetic(sources);
            if let (Some(p5), Some(b8)) = (p5, r.instances.iter_mut().find(|i| i.cell == "B8")) {
                b8.operands.push(p5);
            }
            r
        };
        let checks = prove_leaves(&synthetic_sources(), &extra);
        let failures: Vec<&str> = checks.iter().filter_map(|c| c.failure.as_deref()).collect();
        assert_eq!(
            failures,
            [format!(
                "dead ancestor: {} → {} tcc_footing",
                aisc("P5"),
                aisc("B8")
            )]
        );
    }
}
