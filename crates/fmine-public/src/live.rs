//! Live-dependency proof (BD-338).
//!
//! **Files (S1).** Every registered file is load-bearing at the gate. For each registered file, a scratch
//! workspace holds copies of the other files, and that file absent, written as zero bytes, or replaced by a valid
//! but foreign xlsx. The loader must refuse each case with the code §30.1 gives it (an absent file
//! `FMINE-REF-506`, exit 5; a changed one `FMINE-REF-505`, exit 8), count "1 of n registered", and name that
//! file, and no other, under the label that applies: "missing" for an absent file, "digest mismatch" for a
//! changed one. Only a refusal passes; a loader that falls back to other bytes, or re-raises under another
//! code, fails by name. An unregistered code cannot be raised at all: `Refusal` takes a `RefusalCode`.
//!
//! **Leaves (S2).** Every cell an instance cites is live. Each operand is perturbed alone, on a copy of the
//! parsed sources, and the run repeated. The field located at the leaf must read the new value ("value did not
//! move" otherwise). A field derived from it, or the state, must move ("dead ancestor" otherwise). The state
//! root must move too, which is necessary but never sufficient. Leaves the state did not reach (a refusal before
//! any arithmetic, a stored value that is not whole) are skipped with their reason, never dropped.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::Path;

use fmine_evidence::{Refusal, RefusalCode};
use fmine_io::styles::NumberFormat;
use fmine_io::xlsx::{CellValue, ExtractedCell, Sheet, build_minimal_xlsx, parse_a1_ref};

use crate::aisc::{
    AnchorRule, Calc, IdentityInstance, IdentityState, KernelIndeterminate, NotPublishedReason,
    Operand, OperandRole, OperandValue, ROW_ANCHORS, RefusedReason, admitted_rates,
    implied_rate_text, locate_table, rate_holds,
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

    /// The code the loader must refuse with (§30.1): an absent file is foundation I/O, a changed one a digest
    /// mismatch.
    #[must_use]
    pub fn expected(self) -> RefusalCode {
        match self {
            Self::FileDeleted => RefusalCode::InputMissingOrUnreadable506,
            Self::FileZeroBytes | Self::FileForeign => RefusalCode::DigestMismatch505,
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
        let want = self.kind.expected().spec();
        let mut o = format!(
            "{{\"kind\":\"{}\",\"file\":\"{}\",\"expected_code\":\"{}\",\"expected_exit\":{},\
             \"observed_code\":{code},\"observed_exit\":{exit},\"label\":\"{}\",\"names_only_this_file\":{},\
             \"pass\":{}",
            self.kind.as_str(),
            self.file,
            want.code,
            want.exit_code,
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
    let want = kind.expected().spec();
    let failure = if refusal.exit_code != want.exit_code {
        Some(format!(
            "refusal exit {} is not {}",
            refusal.exit_code, want.exit_code
        ))
    } else if refusal.code != want.code {
        Some(format!(
            "refusal code {} is not {}",
            refusal.code, want.code
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
/// upward. `None` when none lies within the search bound. Found from the kernel's admitted rates, then confirmed
/// by its checked decisions at the flip and just inside it.
pub fn rate_flip_up(n: i64, k: i64, q: i64, r: i64) -> Result<Option<i64>, KernelIndeterminate> {
    let (lo, hi) = admitted_rates(n, k, q)?;
    let x = if r.saturating_add(1) < lo {
        r.saturating_add(1)
    } else {
        r.saturating_add(1).max(hi.saturating_add(1))
    };
    if x > r.saturating_add(FLIP_SEARCH) {
        return Ok(None);
    }
    confirm_flip(n, k, q, x, (x > r + 1).then(|| x - 1))
}

/// The largest printed rate below `r` that the interval for (n, k, q) excludes.
pub fn rate_flip_down(n: i64, k: i64, q: i64, r: i64) -> Result<Option<i64>, KernelIndeterminate> {
    let (lo, hi) = admitted_rates(n, k, q)?;
    let x = if r.saturating_sub(1) > hi {
        r.saturating_sub(1)
    } else {
        r.saturating_sub(1).min(lo.saturating_sub(1))
    };
    if x < r.saturating_sub(FLIP_SEARCH) {
        return Ok(None);
    }
    confirm_flip(n, k, q, x, (x < r - 1).then(|| x + 1))
}

/// `Some(x)` once the kernel decides that x does not hold and `inside` (the neighbour toward r, if it is not r)
/// does.
fn confirm_flip(
    n: i64,
    k: i64,
    q: i64,
    x: i64,
    inside: Option<i64>,
) -> Result<Option<i64>, KernelIndeterminate> {
    let flips = !rate_holds(n, k, q, x, (1, 1))?;
    let held = match inside {
        Some(y) => rate_holds(n, k, q, y, (1, 1))?,
        None => true,
    };
    if flips && held {
        Ok(Some(x))
    } else {
        Err(KernelIndeterminate(format!(
            "the flip at {x} is not confirmed by the kernel's decisions"
        )))
    }
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
const SHARE_RULE_REFUSALS: [RefusedReason; 4] = [
    RefusedReason::FootnoteNotFound,
    RefusedReason::RegionalNumerator,
    RefusedReason::NoRegisteredShare,
    RefusedReason::ShareNotConfirmed,
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
    match &i.state {
        IdentityState::NotPublished {
            reason: NotPublishedReason::AllZeroColumn,
        } => {
            let (p, v) = bump(&o.value);
            return Ok((p, v, Expect::State));
        }
        IdentityState::NotPublished {
            reason: NotPublishedReason::RateNotPublished,
        } if o.role == R::Rate => {
            return Ok((P::ToOne, Int(1), Expect::State));
        }
        IdentityState::Refused { reason } | IdentityState::RefusedAt { reason, .. }
            if SHARE_RULE_REFUSALS.contains(reason) =>
        {
            return match o.role {
                R::NciLine => nci_plan(i, o),
                R::LabelCheck => Ok((P::BreakText, Token("x".to_string()), Expect::State)),
                _ => Err("refused_by_share_rule"),
            };
        }
        IdentityState::NotPublished { .. }
        | IdentityState::Refused { .. }
        | IdentityState::RefusedAt { .. } => return Err("decided_before_arithmetic"),
        _ => {}
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
            IdentityState::Consistent { basis: None, .. } => match rate_flip_up(m.n, m.k, m.q, m.r)
            {
                Ok(Some(r)) => Ok((P::RateFlip, Int(r), Expect::Rate(r))),
                Ok(None) => Err("no_flip_margin_within_search"),
                Err(_) => Err("exact_kernel_indeterminate"),
            },
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
        // A region's state carries every weighed member's rate and ounces (BD-331.S4), and a member that sold
        // nothing stops being one when a value moves off zero.
        (R::MemberRate | R::MemberOunces, Calc::Rate(_), v) => {
            let (p, new) = bump(v);
            Ok((p, new, Expect::State))
        }
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
            let range = implied_rate_text(m0.n, m0.k, m0.q) != implied_rate_text(m1.n, m1.k, m1.q);
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
    run: &(dyn Fn(Vec<LoadedSource>) -> PublicAssuranceReport + Sync),
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
/// outside the frame are not reported; the caller records the frame. The checks are independent runs of
/// `run`, so they are spread over the available cores; the result is in instance and operand order, as a
/// sequential pass would give it.
pub fn prove_leaves_in(
    base: &[LoadedSource],
    run: &(dyn Fn(Vec<LoadedSource>) -> PublicAssuranceReport + Sync),
    select: &(dyn Fn(usize, usize, usize) -> bool + Sync),
) -> Vec<LeafCheck> {
    let before = run(base.to_vec());
    let leaves: Vec<(&IdentityInstance, &Operand)> = before
        .instances
        .iter()
        .enumerate()
        .flat_map(|(ii, i)| {
            i.operands
                .iter()
                .enumerate()
                .filter(move |(oi, _)| select(ii, *oi, i.operands.len()))
                .map(move |(_, o)| (i, o))
        })
        .collect();
    let workers = std::thread::available_parallelism()
        .map_or(1, std::num::NonZeroUsize::get)
        .clamp(1, leaves.len().max(1));
    // Worker w takes leaves w, w + workers, w + 2 * workers, ..., which spreads slow tables evenly.
    let mut parts: Vec<Vec<LeafCheck>> = vec![Vec::new(); workers];
    std::thread::scope(|scope| {
        for (w, part) in parts.iter_mut().enumerate() {
            let (leaves, before) = (&leaves, &before);
            scope.spawn(move || {
                *part = leaves
                    .iter()
                    .skip(w)
                    .step_by(workers)
                    .map(|&(i, o)| leaf_check(base, run, before, i, o))
                    .collect();
            });
        }
    });
    let mut parts: Vec<std::vec::IntoIter<LeafCheck>> =
        parts.into_iter().map(Vec::into_iter).collect();
    (0..leaves.len())
        .filter_map(|k| parts[k % workers].next())
        .collect()
}

/// One leaf's check: `o` of instance `i`, perturbed in a copy of `base` and judged against `before`.
fn leaf_check(
    base: &[LoadedSource],
    run: &(dyn Fn(Vec<LoadedSource>) -> PublicAssuranceReport + Sync),
    before: &PublicAssuranceReport,
    i: &IdentityInstance,
    o: &Operand,
) -> LeafCheck {
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
    match plan(i, o) {
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
                judge_leaf(check, i, o, &new, expect, before, &after)
            } else {
                LeafCheck {
                    skipped: Some("leaf_not_in_loaded_sheet"),
                    ..check
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    // Cross-crate workbook mutation contracts retain this private adapter seam and their original names.
    include!("../../xtask/testdata/public_live_mutation_contract.rs");

    /// The threaded pass gives what one sequential pass gives, check for check and in the same order.
    #[test]
    fn test_live_leaves_threaded_matches_sequential() {
        let base = synthetic_sources();
        let threaded = prove_leaves(&base, &assess_synthetic);
        let before = assess_synthetic(base.clone());
        let sequential: Vec<LeafCheck> = before
            .instances
            .iter()
            .flat_map(|i| i.operands.iter().map(move |o| (i, o)))
            .map(|(i, o)| leaf_check(&base, &assess_synthetic, &before, i, o))
            .collect();
        let workers = std::thread::available_parallelism().map_or(1, std::num::NonZeroUsize::get);
        println!(
            "{{\"test\":\"leaves_threaded\",\"workers\":{workers},\"checks\":{}}}",
            threaded.len()
        );
        assert!(threaded.len() > 50, "{}", threaded.len());
        assert_eq!(threaded, sequential);
    }
}
