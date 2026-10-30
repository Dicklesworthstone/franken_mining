//! The Mine Statistics identity ledger (BD-331.S4, bd-prmn.4): two of the plan's Mine Stats families on the
//! `QTD`/`YTD` summary sheets, decided by the exact kernel (`fmine_numerics::exact_feas`).
//!
//! - **Ownership.** Attributable production = s × 100%-basis production, per site row and column group (the
//!   release's own year, or the prior-year comparative). s is the share the row's label prints. A registered
//!   printed-share correction replaces it, and the printed share is kept beside (W1-DR-01: Porgera's stale
//!   "(47.5%)"). Region and metal-total rows mix shares and pose no ownership identity; any other row with no
//!   printed share is refused (`share_unprinted`), never read as 100%.
//! - **Quarter sums.** H1 = Q1 + Q2, 9M = H1 + Q3 and FY = 9M + Q4 on the three flow columns (100% production,
//!   attributable production, attributable sold), anchored on each `YTD` cell. The operands come from releases
//!   of the anchor's year, in the anchor's column group, on the row of the same subject.
//!
//! Exclusions by name come first: a subject whose registered status is `withheld` for the whole period of an
//! operand (Loulo-Gounkoto Q1–Q3 2025, E818), and a region whose registered members differ between the
//! operands' releases (the Q4 2025 map change). Then, in order: an operand row the release does not print, a
//! blank operand, all operands placeholders (not published), some placeholders. An instance whose operands are
//! all numbers is decided alone: each value as displayed stands for the closed box [v − step/2, v + step/2]
//! (rounded once, the step from the displayed decimals), the identity holds exactly between true values, and a
//! feasible run is (i), an infeasible one (iii).
//!
//! Every decided instance is then posed jointly. Instances that share a cell form a component: a 100% cell is
//! in its ownership identity and in quarter sums, and a cumulative is the total of one sum and an operand of the
//! next. For each infeasible component the kernel's blame gives every minimum correction set, with the range
//! each of its cells can take, never a corrected value. A component can be infeasible though each of its
//! identities is consistent alone (`joint_only`); its instances stay (i).
//!
//! A cell the Mine Stats reader refused is not in its values, so an instance that needs it is refused as a
//! missing row; the reader's refusals are reported with the read.
//!
//! The reference is `scripts/oracle/mine_stats_census.py`, written first by a separate reader from the same
//! rule; the ignored test `barrick_mine_ledger_matches_census` compares every instance and component.

use std::collections::{BTreeMap, BTreeSet};

use fmine_io::xlsx::{Workbook, parse_a1_ref};
use fmine_numerics::exact_feas::{self as xf, Rat};
use fmine_registry::sources::PrintedShareCorrection;

use crate::key::{Period, PeriodKind, StatusKind, SubjectStatus};
use crate::mine_stats::{MineStatsCell, MineStatsRead, MineStatsValue, View};
use crate::sources::RegionMemberRow;

/// The flow columns of the summary sheets per metal: 100% production, attributable production, attributable
/// sold (field ids of the Mine Stats reader).
pub const FLOW_FIELDS: [[&str; 3]; 2] = [
    ["au_prod_100_koz", "au_prod_attr_koz", "au_sold_attr_koz"],
    ["cu_prod_100_kt", "cu_prod_attr_kt", "cu_sold_attr_kt"],
];

/// Metal totals: like regions, they roll up sites held at different shares.
const TOTALS: [&str; 2] = ["Total Gold", "Total Copper"];

const QUARTER_SUM: &str = "quarter_sum";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Family {
    Ownership,
    QuarterSum,
}

impl Family {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Ownership => "ownership",
            Self::QuarterSum => QUARTER_SUM,
        }
    }
}

/// Which year's columns of a summary sheet: the release's own year, or the prior-year comparative.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Group {
    Current,
    Prior,
}

impl Group {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Current => "current",
            Self::Prior => "prior",
        }
    }
}

/// Why an instance is refused (v).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Reason {
    /// The row prints no `(x%)` and is not a region or metal total.
    ShareUnprinted,
    /// Excluded by name: the subject's registered status is `withheld` for an operand's whole period.
    Withheld {
        evidence: String,
    },
    /// Excluded by name: a region whose registered members differ between the operands' releases.
    CrossMapRegion,
    /// An operand's release does not print a row for the subject.
    OperandRowMissing,
    /// An operand's release is not among the files read.
    ReleaseMissing,
    BlankOperand,
    /// Some operands are placeholders and some are numbers.
    PlaceholderOperand,
    /// A number with no exact displayed value, or shown as a percentage.
    OperandUnreadable,
    /// The kernel made no claim (a cap, an overflow, a checker rejection).
    KernelIndeterminate(String),
}

impl Reason {
    #[must_use]
    pub const fn code(&self) -> &'static str {
        match self {
            Self::ShareUnprinted => "share_unprinted",
            Self::Withheld { .. } => "withheld",
            Self::CrossMapRegion => "cross_map_region",
            Self::OperandRowMissing => "operand_row_missing",
            Self::ReleaseMissing => "release_missing",
            Self::BlankOperand => "blank_operand",
            Self::PlaceholderOperand => "placeholder_operand",
            Self::OperandUnreadable => "operand_unreadable",
            Self::KernelIndeterminate(_) => "kernel_indeterminate",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum State {
    /// (i): true values within the printed boxes satisfy the identity.
    Consistent,
    /// (iii): no true values within the printed boxes satisfy it.
    Discrepancy,
    /// (v), with the first offending operand where there is one.
    Refused { reason: Reason, at: Option<String> },
    /// Every operand is a placeholder.
    NotPublished,
}

impl State {
    #[must_use]
    pub const fn code(&self) -> &'static str {
        match self {
            Self::Consistent => "i",
            Self::Discrepancy => "iii",
            Self::Refused { .. } => "v",
            Self::NotPublished => "np",
        }
    }
}

/// One identity instance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Instance {
    pub family: Family,
    pub subject: String,
    /// The anchor's field id: the attributable production field for ownership.
    pub field: String,
    pub group: Group,
    /// The anchor's period.
    pub period: Period,
    /// The attributable cell (ownership) or the cumulative `YTD` cell (quarter sum).
    pub anchor: String,
    /// Operand locators in operand order (100%, attributable; or previous cumulative, quarter, total), `None` for
    /// a row the release does not print.
    pub cells: Vec<Option<String>>,
    pub state: State,
    /// The share used, for an ownership instance whose row prints one.
    pub share: Option<Rat>,
    /// The share as printed (`"61.5"`).
    pub printed_share: Option<String>,
    /// The id of the jointly infeasible component the instance is in, if any.
    pub component: Option<String>,
}

/// Cells whose printed values, disregarded together, make a component feasible, each with the range it can then
/// take (or why there is none, e.g. unbounded).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Correction {
    pub cells: Vec<String>,
    pub implied: Vec<(String, Result<xf::Interval, String>)>,
}

/// A jointly infeasible component.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Component {
    /// Its smallest cell locator.
    pub id: String,
    pub cells: Vec<String>,
    /// Indices into [`MineLedger::instances`], ascending.
    pub instances: Vec<usize>,
    /// Every one of its instances is consistent alone.
    pub joint_only: bool,
    /// The corrections are every set of the smallest size (an exhaustive search), not one inclusion-minimal set.
    pub minimum: bool,
    pub corrections: Vec<Correction>,
}

/// The ledger over every registered Mine Stats release read.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MineLedger {
    /// Sorted by family, anchor, group and field.
    pub instances: Vec<Instance>,
    /// Components of the decided instances.
    pub components: usize,
    /// The jointly infeasible ones, by id.
    pub infeasible: Vec<Component>,
    /// Components the kernel made no joint claim on: `(id, reason)`.
    pub indeterminate: Vec<(String, String)>,
}

impl MineLedger {
    /// Instance count of `family` in state `code`.
    #[must_use]
    pub fn count(&self, family: Family, code: &str) -> usize {
        self.instances
            .iter()
            .filter(|i| i.family == family && i.state.code() == code)
            .count()
    }
}

/// The registry data the ledger reads.
#[derive(Debug, Clone, Copy)]
pub struct LedgerRules<'a> {
    pub regions: &'a [String],
    pub region_members: &'a [RegionMemberRow],
    pub statuses: &'a [SubjectStatus],
    pub corrections: &'a [PrintedShareCorrection],
}

impl LedgerRules<'_> {
    fn members_of(&self, region: &str, release: &str) -> BTreeSet<String> {
        self.region_members
            .iter()
            .filter(|m| m.region == region && m.holds_for(release))
            .map(|m| m.member.clone())
            .collect()
    }

    fn withheld(&self, subject: &str, period: &Period) -> Option<String> {
        self.statuses
            .iter()
            .find(|s| s.subject == subject && s.kind == StatusKind::Withheld && s.applies(period))
            .map(|s| s.evidence.clone())
    }

    fn aggregate(&self, subject: &str) -> bool {
        self.regions.iter().any(|r| r == subject) || TOTALS.contains(&subject)
    }
}

/// One release read: its quarter (`YYYYQn`), its Mine Stats read and the workbook (for displayed values).
#[derive(Debug, Clone, Copy)]
pub struct LedgerFile<'a> {
    pub quarter: &'a str,
    pub read: &'a MineStatsRead,
    pub workbook: &'a Workbook,
}

/// A summary cell as an operand.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Operand {
    Number { value: Rat, step: Rat },
    Placeholder,
    Blank,
    Unreadable,
}

#[derive(Debug, Clone)]
struct Slot {
    locator: String,
    operand: Operand,
    period: Period,
    printed_share: Option<String>,
}

/// `(release, sheet, field, group, subject)`.
type SlotKey = (String, String, String, Group, String);

/// `2026Q2`, `2026H1`, `20269M` or `2026FY`, as the reader writes periods.
fn parse_period(code: &str) -> Option<Period> {
    let year: u16 = code.get(..4)?.parse().ok()?;
    let kind = match code.get(4..)? {
        "Q1" => PeriodKind::Quarter(1),
        "Q2" => PeriodKind::Quarter(2),
        "Q3" => PeriodKind::Quarter(3),
        "Q4" => PeriodKind::Quarter(4),
        "H1" => PeriodKind::H1,
        "9M" => PeriodKind::NineMonths,
        "FY" => PeriodKind::FullYear,
        _ => return None,
    };
    Some(Period { year, kind })
}

fn release_parts(quarter: &str) -> Option<(u16, u8)> {
    let (y, q) = quarter.split_once('Q')?;
    Some((y.parse().ok()?, q.parse().ok()?))
}

/// The operand a reader value stands for: its value as displayed under the cell's number format.
fn operand(v: &MineStatsValue, workbook: &Workbook) -> Operand {
    match &v.value {
        MineStatsCell::Missing => Operand::Blank,
        MineStatsCell::Nil { .. } | MineStatsCell::NotApplicable { .. } => Operand::Placeholder,
        MineStatsCell::Number { .. } => {
            let shown = workbook
                .sheet(&v.sheet)
                .zip(parse_a1_ref(&v.cell))
                .and_then(|(s, (row, col))| s.cell(row, col))
                .and_then(|c| c.displayed());
            match shown {
                Some(Ok(d)) if d.token.is_some() => Operand::Placeholder,
                Some(Ok(d)) if !d.percent => {
                    let scale = 10_i128.checked_pow(u32::from(d.d));
                    match scale.map(|s| (Rat::new(d.scaled, s), Rat::new(1, s))) {
                        Some((Ok(value), Ok(step))) => Operand::Number { value, step },
                        _ => Operand::Unreadable,
                    }
                }
                _ => Operand::Unreadable,
            }
        }
    }
}

/// Every flow cell of the summary sheets, by key. A key read twice (one subject on two rows of a block) keeps
/// neither value: both become unreadable.
fn slots(files: &[LedgerFile<'_>]) -> BTreeMap<SlotKey, Slot> {
    let flows: BTreeSet<&str> = FLOW_FIELDS.iter().flatten().copied().collect();
    let mut out: BTreeMap<SlotKey, Slot> = BTreeMap::new();
    for f in files {
        let Some((year, _)) = release_parts(f.quarter) else {
            continue;
        };
        for v in f.read.values.iter().filter(|v| {
            v.view == View::Summary
                && (v.sheet == "QTD" || v.sheet == "YTD")
                && flows.contains(v.label.as_str())
        }) {
            let Some(period) = parse_period(&v.period) else {
                continue;
            };
            let group = if period.year == year {
                Group::Current
            } else if period.year + 1 == year {
                Group::Prior
            } else {
                continue;
            };
            let key = (
                f.quarter.to_string(),
                v.sheet.clone(),
                v.label.clone(),
                group,
                v.site.clone(),
            );
            let slot = Slot {
                locator: v.locator.clone(),
                operand: operand(v, f.workbook),
                period,
                printed_share: v.printed_share.clone(),
            };
            match out.get_mut(&key) {
                Some(seen) => seen.operand = Operand::Unreadable,
                None => {
                    out.insert(key, slot);
                }
            }
        }
    }
    out
}

/// States 2–4 of the rule over operands that all exist: a blank, all placeholders, some placeholders, an
/// unreadable number. `None` when every operand is a number.
fn operand_state(ops: &[&Slot]) -> Option<State> {
    let first = |pred: fn(&Operand) -> bool| {
        ops.iter()
            .find(|s| pred(&s.operand))
            .map(|s| s.locator.clone())
    };
    let refused = |reason: Reason, at: Option<String>| Some(State::Refused { reason, at });
    if let Some(at) = first(|o| *o == Operand::Blank) {
        return refused(Reason::BlankOperand, Some(at));
    }
    if ops.iter().all(|s| s.operand == Operand::Placeholder) {
        return Some(State::NotPublished);
    }
    if let Some(at) = first(|o| *o == Operand::Placeholder) {
        return refused(Reason::PlaceholderOperand, Some(at));
    }
    if let Some(at) = first(|o| *o == Operand::Unreadable) {
        return refused(Reason::OperandUnreadable, Some(at));
    }
    None
}

fn xf_cell(slot: &Slot) -> Option<xf::Cell> {
    let Operand::Number { value, step } = slot.operand else {
        return None;
    };
    Some(xf::Cell {
        locator: slot.locator.clone(),
        sha256: String::new(),
        token: String::new(),
        printed: value,
        step,
        scope: xf::Scope::Visible,
    })
}

fn rules() -> xf::Rules {
    xf::Rules::new(xf::Convention::Closed).with(QUARTER_SUM, xf::Construction::RoundedOnce)
}

/// A decided instance's kernel posing: its cells and its identity.
struct Posed {
    cells: Vec<xf::Cell>,
    identity: xf::Identity,
}

/// Decides one posing alone.
fn decide(posed: &Posed) -> State {
    match xf::solve(
        &posed.cells,
        std::slice::from_ref(&posed.identity),
        &rules(),
    ) {
        Ok(components) => match components.as_slice() {
            [c] => match &c.outcome {
                xf::Outcome::Feasible { .. } => State::Consistent,
                xf::Outcome::Infeasible { .. } => State::Discrepancy,
                xf::Outcome::NotRefuted => kernel_refused("not refuted".to_string()),
                xf::Outcome::Indeterminate { reason } => kernel_refused(reason.clone()),
            },
            other => kernel_refused(format!("{} components where one was posed", other.len())),
        },
        Err(e) => kernel_refused(e.to_string()),
    }
}

fn kernel_refused(reason: String) -> State {
    State::Refused {
        reason: Reason::KernelIndeterminate(reason),
        at: None,
    }
}

fn identity_id(family: Family, anchor: &str) -> String {
    format!("{}@{anchor}", family.as_str())
}

/// The share an ownership row uses: a registered correction of the printed share, else the printed share.
fn share_of(rules: &LedgerRules<'_>, subject: &str, printed: &str) -> Option<Rat> {
    if let Some(c) = rules
        .corrections
        .iter()
        .find(|c| c.subject == subject && c.printed == printed)
    {
        return Rat::new(i128::from(c.num), i128::from(c.den)).ok();
    }
    Rat::from_decimal_str(printed)
        .and_then(|p| p.checked_div(Rat::from_integer(100)))
        .ok()
}

fn ownership(
    rules: &LedgerRules<'_>,
    slots: &BTreeMap<SlotKey, Slot>,
    out: &mut Vec<(Instance, Option<Posed>)>,
) {
    for [full, attr, _] in FLOW_FIELDS {
        for ((release, sheet, field, group, subject), c) in
            slots.iter().filter(|((_, _, f, _, _), _)| f == attr)
        {
            if rules.aggregate(subject) {
                continue;
            }
            let b = slots.get(&(
                release.clone(),
                sheet.clone(),
                full.to_string(),
                *group,
                subject.clone(),
            ));
            // The share is the row's, whatever the state.
            let share = c
                .printed_share
                .as_deref()
                .and_then(|p| share_of(rules, subject, p));
            let instance = |state| Instance {
                family: Family::Ownership,
                subject: subject.clone(),
                field: field.clone(),
                group: *group,
                period: c.period,
                anchor: c.locator.clone(),
                cells: vec![b.map(|b| b.locator.clone()), Some(c.locator.clone())],
                state,
                share,
                printed_share: c.printed_share.clone(),
                component: None,
            };
            let refused = |reason, at| State::Refused { reason, at };
            if let Some(evidence) = rules.withheld(subject, &c.period) {
                out.push((instance(refused(Reason::Withheld { evidence }, None)), None));
                continue;
            }
            let Some(b) = b else {
                out.push((instance(refused(Reason::OperandRowMissing, None)), None));
                continue;
            };
            if let Some(state) = operand_state(&[b, c]) {
                out.push((instance(state), None));
                continue;
            }
            let s = match (&c.printed_share, share) {
                (None, _) => {
                    out.push((instance(refused(Reason::ShareUnprinted, None)), None));
                    continue;
                }
                (Some(_), None) => {
                    let state = refused(Reason::OperandUnreadable, Some(c.locator.clone()));
                    out.push((instance(state), None));
                    continue;
                }
                (Some(_), Some(s)) => s,
            };
            let posed = Posed {
                cells: [b, c].into_iter().filter_map(xf_cell).collect(),
                identity: xf::Identity {
                    id: identity_id(Family::Ownership, &c.locator),
                    family: Family::Ownership.as_str().to_string(),
                    closure: xf::Closure::Definitional,
                    kind: xf::IdentityKind::Scale {
                        target: c.locator.clone(),
                        source: b.locator.clone(),
                        factor: s,
                    },
                },
            };
            out.push((instance(decide(&posed)), Some(posed)));
        }
    }
}

fn quarter_sums(
    rules: &LedgerRules<'_>,
    slots: &BTreeMap<SlotKey, Slot>,
    releases: &BTreeSet<&str>,
    out: &mut Vec<(Instance, Option<Posed>)>,
) {
    for ((release, _, field, group, subject), total) in
        slots.iter().filter(|((_, s, _, _, _), _)| s == "YTD")
    {
        let Some((year, n @ 2..=4)) = release_parts(release) else {
            continue;
        };
        let prev_release = format!("{year}Q{}", n - 1);
        let prev_sheet = if n == 2 { "QTD" } else { "YTD" };
        let key = |r: &str, s: &str| {
            (
                r.to_string(),
                s.to_string(),
                field.clone(),
                *group,
                subject.clone(),
            )
        };
        let prev = slots.get(&key(&prev_release, prev_sheet));
        let quarter = slots.get(&key(release, "QTD"));
        let period_year = match group {
            Group::Current => year,
            Group::Prior => year - 1,
        };
        let at = |kind| Period {
            year: period_year,
            kind,
        };
        let prev_kind = match n {
            2 => PeriodKind::Quarter(1),
            3 => PeriodKind::H1,
            _ => PeriodKind::NineMonths,
        };
        let periods = [at(prev_kind), at(PeriodKind::Quarter(n)), total.period];
        let mut instance = Instance {
            family: Family::QuarterSum,
            subject: subject.clone(),
            field: field.clone(),
            group: *group,
            period: total.period,
            anchor: total.locator.clone(),
            cells: vec![
                prev.map(|s| s.locator.clone()),
                quarter.map(|s| s.locator.clone()),
                Some(total.locator.clone()),
            ],
            state: State::Consistent,
            share: None,
            printed_share: None,
            component: None,
        };
        let refused = |reason| State::Refused { reason, at: None };
        let mut posed = None;
        let withheld = periods.iter().find_map(|p| rules.withheld(subject, p));
        let region = rules.regions.iter().any(|r| r == subject);
        instance.state = if let Some(evidence) = withheld {
            refused(Reason::Withheld { evidence })
        } else if region
            && rules.members_of(subject, &prev_release) != rules.members_of(subject, release)
        {
            refused(Reason::CrossMapRegion)
        } else if !releases.contains(prev_release.as_str()) {
            refused(Reason::ReleaseMissing)
        } else if let (Some(a), Some(q)) = (prev, quarter) {
            match operand_state(&[a, q, total]) {
                Some(state) => state,
                None => {
                    let p = Posed {
                        cells: [a, q, total].into_iter().filter_map(xf_cell).collect(),
                        identity: xf::Identity {
                            id: identity_id(Family::QuarterSum, &total.locator),
                            family: QUARTER_SUM.to_string(),
                            closure: xf::Closure::Definitional,
                            kind: xf::IdentityKind::Sum {
                                total: total.locator.clone(),
                                lines: vec![a.locator.clone(), q.locator.clone()],
                                signs: vec![1, 1],
                            },
                        },
                    };
                    let state = decide(&p);
                    posed = Some(p);
                    state
                }
            }
        } else {
            refused(Reason::OperandRowMissing)
        };
        out.push((instance, posed));
    }
}

/// Poses every decided instance jointly and blames each infeasible component.
fn joint(
    instances: &mut [Instance],
    posed: &[Option<Posed>],
) -> (usize, Vec<Component>, Vec<(String, String)>) {
    let mut cells: BTreeMap<String, xf::Cell> = BTreeMap::new();
    let mut identities = Vec::new();
    let mut owner: BTreeMap<String, usize> = BTreeMap::new();
    for (k, (i, p)) in instances.iter().zip(posed).enumerate() {
        let Some(p) = p else { continue };
        if !matches!(i.state, State::Consistent | State::Discrepancy) {
            continue;
        }
        for c in &p.cells {
            cells.entry(c.locator.clone()).or_insert_with(|| c.clone());
        }
        owner.insert(p.identity.id.clone(), k);
        identities.push(p.identity.clone());
    }
    let cells: Vec<xf::Cell> = cells.into_values().collect();
    let rules = rules();
    let solved = match xf::solve(&cells, &identities, &rules) {
        Ok(s) => s,
        Err(e) => return (0, Vec::new(), vec![(String::new(), e.to_string())]),
    };
    let blamed = xf::blame(&cells, &identities, &rules).unwrap_or_default();
    let mut infeasible = Vec::new();
    let mut indeterminate = Vec::new();
    for c in &solved {
        let id = c.cells.first().cloned().unwrap_or_default();
        let mut members: Vec<usize> = c
            .identities
            .iter()
            .filter_map(|i| owner.get(i).copied())
            .collect();
        members.sort_unstable();
        match &c.outcome {
            xf::Outcome::Feasible { .. } => {}
            xf::Outcome::Infeasible { .. } => {
                let blame = blamed.iter().find(|b| b.cells == c.cells);
                let (minimum, corrections) = match blame.map(|b| &b.outcome) {
                    Some(xf::BlameOutcome::Blamed(b)) => {
                        let mut sets: Vec<Correction> = b
                            .corrections
                            .iter()
                            .map(|s| {
                                let mut cells = s.cells.clone();
                                cells.sort();
                                let mut implied: Vec<(String, Result<xf::Interval, String>)> = s
                                    .implied
                                    .iter()
                                    .map(|(l, r)| {
                                        (l.clone(), r.clone().map_err(|e| implied_error(&e)))
                                    })
                                    .collect();
                                implied.sort_by(|a, b| a.0.cmp(&b.0));
                                Correction { cells, implied }
                            })
                            .collect();
                        sets.sort_by(|a, b| a.cells.cmp(&b.cells));
                        (b.label == xf::CorrectionLabel::Minimum, sets)
                    }
                    Some(xf::BlameOutcome::Indeterminate { reason }) => {
                        indeterminate.push((id.clone(), format!("blame: {reason}")));
                        (false, Vec::new())
                    }
                    None => {
                        indeterminate
                            .push((id.clone(), "blame: no blame for the component".to_string()));
                        (false, Vec::new())
                    }
                };
                let mut joint_only = true;
                for &k in &members {
                    if let Some(i) = instances.get_mut(k) {
                        i.component = Some(id.clone());
                        joint_only &= i.state == State::Consistent;
                    }
                }
                infeasible.push(Component {
                    id,
                    cells: c.cells.clone(),
                    joint_only,
                    instances: members,
                    minimum,
                    corrections,
                });
            }
            xf::Outcome::NotRefuted => indeterminate.push((id, "not refuted".to_string())),
            xf::Outcome::Indeterminate { reason } => indeterminate.push((id, reason.clone())),
        }
    }
    infeasible.sort_by(|a, b| a.id.cmp(&b.id));
    (solved.len(), infeasible, indeterminate)
}

fn implied_error(e: &xf::ImpliedError) -> String {
    match e {
        xf::ImpliedError::Unbounded => "unbounded".to_string(),
        xf::ImpliedError::Infeasible => "infeasible".to_string(),
        xf::ImpliedError::Refused(r) => format!("refused: {r}"),
        xf::ImpliedError::Indeterminate(r) => format!("indeterminate: {r}"),
    }
}

/// The ledger over `files` (any order; one per release quarter).
#[must_use]
pub fn mine_ledger(files: &[LedgerFile<'_>], rules: &LedgerRules<'_>) -> MineLedger {
    let slots = slots(files);
    let releases: BTreeSet<&str> = files.iter().map(|f| f.quarter).collect();
    let mut pairs = Vec::new();
    ownership(rules, &slots, &mut pairs);
    quarter_sums(rules, &slots, &releases, &mut pairs);
    pairs.sort_by(|(a, _), (b, _)| {
        (a.family, &a.anchor, a.group, &a.field).cmp(&(b.family, &b.anchor, b.group, &b.field))
    });
    let (mut instances, posed): (Vec<Instance>, Vec<Option<Posed>>) = pairs.into_iter().unzip();
    let (components, infeasible, indeterminate) = joint(&mut instances, &posed);
    MineLedger {
        instances,
        components,
        infeasible,
        indeterminate,
    }
}

/// An implied range as `[lo, hi]` with `(`/`)` for an open end, or why there is none.
#[must_use]
pub fn range_text(range: &Result<xf::Interval, String>) -> String {
    match range {
        Ok(i) => format!(
            "{}{}, {}{}",
            if i.lo_strict { '(' } else { '[' },
            rat_text(i.lo),
            rat_text(i.hi),
            if i.hi_strict { ')' } else { ']' }
        ),
        Err(why) => why.clone(),
    }
}

/// An exact rational as `p` or `p/q`.
#[must_use]
pub fn rat_text(r: Rat) -> String {
    if r.den() == 1 {
        r.num().to_string()
    } else {
        format!("{}/{}", r.num(), r.den())
    }
}

#[cfg(test)]
mod tests {
    use fmine_io::xlsx::{MinimalCell, MinimalSheet, MinimalStyles, build_minimal_xlsx_styled};

    use super::*;
    use crate::mine_stats::read_mine_stats;

    /// A summary row: the label, then 100% production, attributable production and attributable sold for the
    /// release's own year (columns B–D). A cell is `""` for blank, `—` or `n/a` as printed, a number as its
    /// stored text under General, or `#` (`##`) and the stored text for a number shown under `0.0` (`0.00`).
    type Row<'a> = (&'a str, [&'a str; 3]);

    /// `(n, QTD rows, YTD rows)` for release `2030Qn`; a first quarter has no `YTD` sheet.
    type Release<'a> = (u8, Vec<Row<'a>>, Vec<Row<'a>>);

    const NONE: LedgerRules<'static> = LedgerRules {
        regions: &[],
        region_members: &[],
        statuses: &[],
        corrections: &[],
    };

    fn sheet(name: &str, months: &str, rows: &[Row<'_>]) -> MinimalSheet {
        let text = |s: &str| MinimalCell::Text(s.to_string());
        let number = |raw: &str, xf| MinimalCell::Styled {
            raw: raw.to_string(),
            xf,
        };
        let mut cells = vec![
            (
                "A3".to_string(),
                text(&format!("For the {months} months ended December 31,")),
            ),
            ("B3".to_string(), number("2030", 0)),
        ];
        let headers = [
            "Gold produced (000s oz 100%)",
            "Gold produced (000s oz attributable)",
            "Gold sold (000s oz attributable)",
        ];
        for (col, h) in ["B", "C", "D"].into_iter().zip(headers) {
            cells.push((format!("{col}4"), text(h)));
        }
        for (k, (label, values)) in rows.iter().enumerate() {
            let row = 5 + k;
            cells.push((format!("A{row}"), text(label)));
            for (col, v) in ["B", "C", "D"].into_iter().zip(values) {
                let c = match *v {
                    "" => continue,
                    "—" | "n/a" => text(v),
                    _ => match (v.strip_prefix("##"), v.strip_prefix('#')) {
                        (Some(raw), _) => number(raw, 2),
                        (None, Some(raw)) => number(raw, 1),
                        (None, None) => number(v, 0),
                    },
                };
                cells.push((format!("{col}{row}"), c));
            }
        }
        MinimalSheet {
            name: name.to_string(),
            hidden: false,
            cells,
        }
    }

    fn ledger(releases: &[Release<'_>], rules: &LedgerRules<'_>) -> MineLedger {
        let styles = MinimalStyles {
            num_fmts: vec![(164, "0.0".to_string())],
            xfs: vec![0, 164, 2],
        };
        let books: Vec<(String, Workbook)> = releases
            .iter()
            .map(|(n, qtd, ytd)| {
                let mut sheets = vec![sheet("QTD", "three", qtd)];
                if *n > 1 {
                    let months = ["three", "six", "nine", "twelve"][usize::from(*n) - 1];
                    sheets.push(sheet("YTD", months, ytd));
                }
                let bytes = build_minimal_xlsx_styled(&sheets, Some(&styles));
                let wb = Workbook::from_bytes("synthetic_mine_ledger.xlsx", &bytes)
                    .expect("synthetic workbook parses");
                (format!("2030Q{n}"), wb)
            })
            .collect();
        let reads: Vec<MineStatsRead> = books
            .iter()
            .map(|(q, wb)| {
                let read = read_mine_stats(wb, q, None).expect("reads");
                assert!(read.refusals.is_empty(), "{q}: {:?}", read.refusals);
                read
            })
            .collect();
        let files: Vec<LedgerFile<'_>> = books
            .iter()
            .zip(&reads)
            .map(|((quarter, workbook), read)| LedgerFile {
                quarter,
                read,
                workbook,
            })
            .collect();
        mine_ledger(&files, rules)
    }

    fn at<'a>(l: &'a MineLedger, family: Family, anchor: &str) -> &'a Instance {
        l.instances
            .iter()
            .find(|i| i.family == family && i.anchor == anchor)
            .unwrap_or_else(|| panic!("no {} instance at {anchor}", family.as_str()))
    }

    fn sets(c: &Component) -> Vec<Vec<&str>> {
        c.corrections
            .iter()
            .map(|s| s.cells.iter().map(String::as_str).collect())
            .collect()
    }

    fn refused(reason: Reason, at: Option<&str>) -> State {
        State::Refused {
            reason,
            at: at.map(str::to_string),
        }
    }

    /// Carlin at 50%: Q1 100% production printed 300 where the other cells imply 200 (the shape of E113).
    fn e113_shape<'a>() -> Vec<Release<'a>> {
        vec![
            (1, vec![("Carlin (50%)", ["300", "100", "90"])], vec![]),
            (
                2,
                vec![("Carlin (50%)", ["150", "75", "70"])],
                vec![("Carlin (50%)", ["350", "175", "160"])],
            ),
        ]
    }

    #[test]
    fn test_one_shared_cell_is_the_one_minimum_set() {
        let l = ledger(&e113_shape(), &NONE);
        let own = at(&l, Family::Ownership, "2030Q1:QTD!C5");
        let sum = at(&l, Family::QuarterSum, "2030Q2:YTD!B5");
        assert_eq!((own.state.code(), sum.state.code()), ("iii", "iii"));
        assert_eq!(own.share, Rat::new(1, 2).ok());
        assert_eq!(
            (
                l.count(Family::Ownership, "i"),
                l.count(Family::QuarterSum, "i")
            ),
            (2, 2)
        );
        assert_eq!(l.instances.len(), 6);
        let [c] = l.infeasible.as_slice() else {
            panic!("one infeasible component: {:?}", l.infeasible);
        };
        assert_eq!(own.component.as_deref(), Some(c.id.as_str()));
        assert_eq!(sum.component, own.component);
        assert!(c.minimum && !c.joint_only);
        assert_eq!(sets(c), [["2030Q1:QTD!B5"]]);
        // The range B5 can take from the other cells holds 200 and not the printed 300.
        let [(cell, Ok(range))] = c.corrections[0].implied.as_slice() else {
            panic!("one implied range: {:?}", c.corrections[0].implied);
        };
        assert_eq!(cell, "2030Q1:QTD!B5");
        assert_eq!(range.contains(Rat::from_integer(200)), Ok(true));
        assert_eq!(range.contains(Rat::from_integer(300)), Ok(false));
        // The sold column is its own component, feasible.
        assert_eq!(l.components, 2);
    }

    /// Every identity is consistent alone; together they are not. 51 attributable at 50% puts each quarter's
    /// true 100% production at 101 or more, so H1 is at least 202, above the 201.5 that 201 allows.
    #[test]
    fn test_joint_only_component_keeps_its_instances_consistent() {
        let carlin = |v| vec![("Carlin (50%)", v)];
        let l = ledger(
            &[
                (1, carlin(["101", "51", "40"]), vec![]),
                (2, carlin(["101", "51", "40"]), carlin(["201", "101", "80"])),
            ],
            &NONE,
        );
        assert!(l.instances.iter().all(|i| i.state == State::Consistent));
        let [c] = l.infeasible.as_slice() else {
            panic!("one infeasible component: {:?}", l.infeasible);
        };
        assert!(c.joint_only && c.minimum);
        assert_eq!(c.instances.len(), 5);
        assert_eq!(
            sets(c),
            [["2030Q1:QTD!C5"], ["2030Q2:QTD!C5"], ["2030Q2:YTD!B5"]]
        );
        let ranges: Vec<(&str, String)> = c
            .corrections
            .iter()
            .flat_map(|s| &s.implied)
            .map(|(cell, r)| (cell.as_str(), range_text(r)))
            .collect();
        assert_eq!(
            ranges,
            [
                ("2030Q1:QTD!C5", "[201/4, 201/4]".to_string()),
                ("2030Q2:QTD!C5", "[201/4, 201/4]".to_string()),
                ("2030Q2:YTD!B5", "[202, 203]".to_string()),
            ]
        );
    }

    #[test]
    fn test_unprinted_share_is_refused_and_aggregates_pose_no_ownership() {
        let regions = ["Nevada".to_string()];
        let rules = LedgerRules {
            regions: &regions,
            ..NONE
        };
        let rows = vec![
            ("Hemlo", ["100", "100", "90"]),
            ("Nevada", ["200", "100", "90"]),
            ("Total Gold", ["300", "200", "180"]),
        ];
        let l = ledger(&[(1, rows, vec![])], &rules);
        let [hemlo] = l.instances.as_slice() else {
            panic!("ownership for Hemlo only: {:?}", l.instances);
        };
        // 100 = 100% × 100 would hold; an unprinted share is never read as 100%.
        assert_eq!(hemlo.subject, "Hemlo");
        assert_eq!(hemlo.state, refused(Reason::ShareUnprinted, None));
        assert_eq!((hemlo.share, hemlo.printed_share.as_deref()), (None, None));
    }

    #[test]
    fn test_registered_share_replaces_the_printed_one_kept_beside() {
        let rows = || vec![(1, vec![("Porgera (47.5%)", ["200", "49", "45"])], vec![])];
        let porgera = |printed: &str| PrintedShareCorrection {
            subject: "Porgera".to_string(),
            printed: printed.to_string(),
            num: 49,
            den: 200,
            evidence: "W1-DR-01".to_string(),
        };
        let state = |corrections: &[PrintedShareCorrection]| {
            let l = ledger(
                &rows(),
                &LedgerRules {
                    corrections,
                    ..NONE
                },
            );
            at(&l, Family::Ownership, "2030Q1:QTD!C5").clone()
        };
        // 47.5% of 200 is 95, not 49.
        assert_eq!(state(&[]).state, State::Discrepancy);
        // A correction for another printed share does not apply.
        assert_eq!(state(&[porgera("50")]).state, State::Discrepancy);
        let corrected = state(&[porgera("47.5")]);
        assert_eq!(corrected.state, State::Consistent);
        assert_eq!(corrected.share, Rat::new(49, 200).ok());
        assert_eq!(corrected.printed_share.as_deref(), Some("47.5"));
    }

    #[test]
    fn test_withheld_subject_is_excluded_for_the_periods_it_covers() {
        let lg = |v| vec![("Loulo-Gounkoto (80%)", v)];
        let releases = vec![
            (1, lg(["100", "10", "9"]), vec![]),
            (2, lg(["100", "80", "70"]), lg(["200", "90", "79"])),
        ];
        let statuses = [SubjectStatus {
            subject: "Loulo-Gounkoto".to_string(),
            kind: StatusKind::Withheld,
            first: (2030, 1),
            last: Some((2030, 3)),
            evidence: "E818".to_string(),
        }];
        let withheld = refused(
            Reason::Withheld {
                evidence: "E818".to_string(),
            },
            None,
        );
        let l = ledger(
            &releases,
            &LedgerRules {
                statuses: &statuses,
                ..NONE
            },
        );
        assert_eq!(at(&l, Family::Ownership, "2030Q1:QTD!C5").state, withheld);
        assert_eq!(
            at(&l, Family::Ownership, "2030Q2:QTD!C5").state,
            State::Consistent
        );
        // H1 is not entirely within Q1: decided, and 80% of 200 is not 90.
        assert_eq!(
            at(&l, Family::Ownership, "2030Q2:YTD!C5").state,
            State::Discrepancy
        );
        for col in ["B", "C", "D"] {
            let anchor = format!("2030Q2:YTD!{col}5");
            assert_eq!(at(&l, Family::QuarterSum, &anchor).state, withheld);
        }
        // Without the status the Q1 row is decided.
        let open = ledger(&releases, &NONE);
        assert_eq!(
            at(&open, Family::Ownership, "2030Q1:QTD!C5").state,
            State::Discrepancy
        );
    }

    #[test]
    fn test_region_whose_members_change_between_releases_is_excluded() {
        let africa = |v| vec![("Africa", v)];
        let releases = vec![
            (1, africa(["100", "60", "50"]), vec![]),
            (
                2,
                africa(["100", "60", "50"]),
                africa(["250", "120", "100"]),
            ),
        ];
        let regions = ["Africa".to_string()];
        let member = |m: &str, first: &str| RegionMemberRow {
            region: "Africa".to_string(),
            member: m.to_string(),
            first: first.to_string(),
            last: None,
        };
        let states = |members: &[RegionMemberRow]| {
            let rules = LedgerRules {
                regions: &regions,
                region_members: members,
                ..NONE
            };
            let l = ledger(&releases, &rules);
            assert_eq!(
                l.count(Family::Ownership, "i") + l.count(Family::Ownership, "iii"),
                0
            );
            ["B", "C", "D"].map(|col| {
                let anchor = format!("2030Q2:YTD!{col}5");
                at(&l, Family::QuarterSum, &anchor).state.clone()
            })
        };
        // One map: decided (250 is not 100 + 100).
        assert_eq!(
            states(&[member("Kibali", "2030Q1")]),
            [State::Discrepancy, State::Consistent, State::Consistent]
        );
        // Tongon joins in Q2: Q1 + Q2 would add two different regions.
        let cross = refused(Reason::CrossMapRegion, None);
        assert_eq!(
            states(&[member("Kibali", "2030Q1"), member("Tongon", "2030Q2")]),
            [cross.clone(), cross.clone(), cross]
        );
    }

    #[test]
    fn test_placeholders_blanks_and_repeated_rows() {
        let rows = vec![
            ("Tongon (89.7%)", ["—", "—", "—"]),
            ("Kibali (45%)", ["—", "45", "40"]),
            ("Hemlo (100%)", ["", "45", "40"]),
            ("Zaldivar (50%)", ["n/a", "n/a", ""]),
            ("Carlin (50%)", ["100", "50", "40"]),
            ("Carlin (50%)", ["100", "50", "40"]),
        ];
        let l = ledger(&[(1, rows, vec![])], &NONE);
        let state = |row: u32| {
            at(&l, Family::Ownership, &format!("2030Q1:QTD!C{row}"))
                .state
                .clone()
        };
        assert_eq!(state(5), State::NotPublished);
        assert_eq!(
            state(6),
            refused(Reason::PlaceholderOperand, Some("2030Q1:QTD!B6"))
        );
        assert_eq!(
            state(7),
            refused(Reason::BlankOperand, Some("2030Q1:QTD!B7"))
        );
        assert_eq!(state(8), State::NotPublished);
        // One subject on two rows of a block keeps neither value.
        assert_eq!(
            state(9),
            refused(Reason::OperandUnreadable, Some("2030Q1:QTD!B9"))
        );
        assert_eq!(l.instances.len(), 5);
    }

    /// The same stored text decides differently under different number formats: the box is the displayed
    /// value's, never the stored text's.
    #[test]
    fn test_displayed_value_sets_the_box() {
        let state = |b: &str, c: &str| {
            let l = ledger(&[(1, vec![("Carlin (50%)", [b, c, "40"])], vec![])], &NONE);
            at(&l, Family::Ownership, "2030Q1:QTD!C5").state.clone()
        };
        // Shown as 100.1 and 50.0: half of [100.05, 100.15] meets [49.95, 50.05].
        assert_eq!(state("#100.14", "#50.04"), State::Consistent);
        // Shown as stored, under 0.00: half of [100.135, 100.145] misses [50.035, 50.045].
        assert_eq!(state("##100.14", "##50.04"), State::Discrepancy);
        // General shows a non-integer by column width, which the file does not fix: refused, never guessed.
        assert_eq!(
            state("100.14", "#50.04"),
            refused(Reason::OperandUnreadable, Some("2030Q1:QTD!B5"))
        );
    }

    #[test]
    fn test_order_of_rows_and_files_changes_no_state() {
        let mut releases = e113_shape();
        for (_, qtd, ytd) in &mut releases {
            qtd.push(("Hemlo (100%)", ["20", "20", "18"]));
            ytd.push(("Hemlo (100%)", ["42", "40", "36"]));
        }
        let project = |l: &MineLedger| {
            let mut v: Vec<_> = l
                .instances
                .iter()
                .map(|i| {
                    (
                        i.family,
                        i.subject.clone(),
                        i.field.clone(),
                        i.period.code(),
                        i.state.code(),
                        i.component.is_some(),
                    )
                })
                .collect();
            v.sort();
            (v, l.components, l.infeasible.len())
        };
        let base = ledger(&releases, &NONE);
        let mut permuted = releases.clone();
        permuted.reverse();
        for (_, qtd, ytd) in &mut permuted {
            qtd.reverse();
            ytd.reverse();
        }
        assert_eq!(project(&ledger(&permuted, &NONE)), project(&base));
        // Hemlo's H1 100% (42 against 20 + 20, and against its own 40 attributable) is a second component.
        assert_eq!(base.infeasible.len(), 2);
    }

    #[test]
    fn test_mutations_move_the_state() {
        let own_q2 = |releases: &[Release<'_>]| {
            let l = ledger(releases, &NONE);
            at(&l, Family::Ownership, "2030Q2:QTD!C5").state.clone()
        };
        let base = e113_shape();
        assert_eq!(own_q2(&base), State::Consistent);
        // A sign flip on the attributable cell.
        let mut flipped = base.clone();
        flipped[1].1[0].1[1] = "-75";
        assert_eq!(own_q2(&flipped), State::Discrepancy);
        // The share dropped from the label.
        let mut unshared = base.clone();
        unshared[1].1[0].0 = "Carlin";
        assert_eq!(own_q2(&unshared), refused(Reason::ShareUnprinted, None));
        // The first quarter's file missing: the H1 sums are refused, the Q2 rows still decided.
        let l = ledger(&base[1..], &NONE);
        assert_eq!(
            at(&l, Family::QuarterSum, "2030Q2:YTD!B5").state,
            refused(Reason::ReleaseMissing, None)
        );
        assert_eq!(
            at(&l, Family::Ownership, "2030Q2:QTD!C5").state,
            State::Consistent
        );
    }
}
