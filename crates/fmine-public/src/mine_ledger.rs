//! The Mine Statistics identity ledger (BD-331.S4, bd-prmn.4): five of the plan's Mine Stats families, anchored on
//! the `QTD`/`YTD` summary sheets and decided in exact arithmetic (the kernel `fmine_numerics::exact_feas`, or
//! its closed form for a lone sum).
//!
//! - **Ownership.** Attributable production = s × 100%-basis production, per site row and column group (the
//!   release's own year, or the prior-year comparative). s is the share the row's label prints. A registered
//!   printed-share correction replaces it, and the printed share is kept beside (W1-DR-01: Porgera's stale
//!   "(47.5%)"). Region and metal-total rows mix shares and pose no ownership identity; any other row with no
//!   printed share is refused (`share_unprinted`), never read as 100%.
//! - **Quarter sums.** H1 = Q1 + Q2, 9M = H1 + Q3 and FY = 9M + Q4 on the three flow columns (100% production,
//!   attributable production, attributable sold), anchored on each `YTD` cell. The operands come from releases
//!   of the anchor's year, in the anchor's column group, on the row of the same subject.
//! - **Summary against site sheet** (family 7). Each summary value of production, sales, cost of sales, TCC or
//!   C1 and AISC equals the same site's value on a `Minestats*` sheet of the metal (title `(Gold)` or
//!   `(Copper)`), same release, period and column group: [`SITE_FIELDS`] names the row. A registered label alias
//!   reads a stale row label as the registered one (W1-DR-02: the 2024 copper "(millions of pounds)" rows hold
//!   kilotonnes). Subjects with no column on such a sheet (regions, `Total Gold`) pose none. A subject on two
//!   sheets of one metal, or a row label twice in one block, is refused. Two numbers are one equal identity
//!   over their boxes; the nil token `—` against a number reads as number = 0 and is decided alone
//!   (`nil_token`, E116); `n/a` against a number is refused.
//! - **Sites to region** (family 3). A region row = Σ its registered members in the release's map, on the three
//!   gold flow columns. A member's row is found by its subject or by its registered Mine Stats label ("Nevada Gold
//!   Mines LLC"); a member printed under both is refused at the row printed first. A region row with no member in
//!   the release's map is refused (`region_not_in_map`).
//! - **Region to total** (family 4). `Total Gold` = Σ the regions of the release's map (not every region row
//!   printed); `Total Copper` = Σ the copper block's site rows, in row order.
//!
//!   In both, the nil token is exactly 0 (`nil_token`); any other placeholder beside a number is refused. A sum
//!   is decided alone by |Σ parts − total| ≤ Σ half-steps over every operand, which is the kernel's answer for
//!   one sum with every box rounded once (a test checks the two agree). Sums are never posed jointly: a sum links
//!   every site of a block, so posing them would merge the other families' components past the exhaustive
//!   correction search. Not yet posed: the Nevada Gold Mines LLC sub-sum (Carlin, Cortez, Turquoise Ridge,
//!   Phoenix, Long Canyon), which needs a registry table of its own.
//!
//! Exclusions by name come first: a subject whose registered status is `withheld` for the whole period of an
//! operand (Loulo-Gounkoto Q1–Q3 2025, E818; for a region sum, a member's), and a region whose registered
//! members differ between the operands' releases (the Q4 2025 map change). Then, in order: an operand row the
//! release does not print, a blank operand, all operands placeholders (not published), some placeholders. An
//! instance whose operands are all numbers is decided alone: each value as displayed stands for the closed box
//! [v − step/2, v + step/2] (rounded once, the step from the displayed decimals), the identity holds exactly
//! between true values, and a feasible run is (i), an infeasible one (iii).
//!
//! Every decided instance but the sums is then posed jointly. Instances that share a cell form a component: a
//! 100% cell is in its ownership identity and in quarter sums, and a cumulative is the total of one sum and an
//! operand of the next. For each infeasible component the kernel's blame gives every minimum correction set, with
//! the range each of its cells can take, never a corrected value. A component can be infeasible though each of
//! its identities is consistent alone (`joint_only`); its instances stay (i).
//!
//! A cell the Mine Stats reader refused is not in its values, so an instance that needs it is refused as a
//! missing row; the reader's refusals are reported with the read.
//!
//! The reference is `scripts/oracle/mine_stats_census.py`, written first by a separate reader from the same
//! rule; the ignored test `barrick_mine_ledger_matches_census` compares every instance and component.

use std::collections::{BTreeMap, BTreeSet};

use fmine_io::xlsx::{CellValue, Workbook, parse_a1_ref};
use fmine_numerics::exact_feas::{self as xf, Rat};
use fmine_registry::sources::{MineStatsLabelAlias, PrintedShareCorrection};

use crate::key::{Period, PeriodKind, StatusKind, SubjectStatus};
use crate::mine_stats::{MineStatsCell, MineStatsRead, MineStatsValue, View};
use crate::sources::{RegionMemberRow, RegisteredShare};

/// The flow columns of the summary sheets per metal: 100% production, attributable production, attributable
/// sold (field ids of the Mine Stats reader).
pub const FLOW_FIELDS: [[&str; 3]; 2] = [
    ["au_prod_100_koz", "au_prod_attr_koz", "au_sold_attr_koz"],
    ["cu_prod_100_kt", "cu_prod_attr_kt", "cu_sold_attr_kt"],
];

/// Family 7's fields: `(metal, summary field id, Minestats row label in lower case)`. The site sheets print
/// Barrick's share, so production and sales pair with the attributable summary columns.
pub const SITE_FIELDS: [(Metal, &str, &str); 10] = [
    (
        Metal::Gold,
        "au_prod_attr_koz",
        "production (thousands of ounces)",
    ),
    (
        Metal::Gold,
        "au_sold_attr_koz",
        "sales (thousands of ounces)",
    ),
    (
        Metal::Gold,
        "cos_usd_per_oz",
        "cost of sales per ounce ($/oz)",
    ),
    (Metal::Gold, "tcc_usd_per_oz", "total cash costs"),
    (Metal::Gold, "aisc_usd_per_oz", "all-in sustaining costs"),
    (
        Metal::Copper,
        "cu_prod_attr_kt",
        "production (thousands of tonnes)",
    ),
    (
        Metal::Copper,
        "cu_sold_attr_kt",
        "sales (thousands of tonnes)",
    ),
    (
        Metal::Copper,
        "cos_usd_per_lb",
        "cost of sales per pound ($/lb)",
    ),
    (Metal::Copper, "c1_usd_per_lb", "c1 cash costs"),
    (Metal::Copper, "aisc_usd_per_lb", "all-in sustaining costs"),
];

/// Metal totals: like regions, they roll up sites held at different shares.
const TOTALS: [&str; 2] = ["Total Gold", "Total Copper"];

const QUARTER_SUM: &str = "quarter_sum";

/// Components up to this many cells get the exhaustive correction search (the kernel's default is 20). A summary
/// cell's site-sheet print joins it to its component, so one site's year of quarters can reach 21 cells. The run
/// cap still bounds the search: at 32 cells every set up to size 3 takes at most 32 + 496 + 4,960 = 5,488 runs,
/// within [`xf::MAX_BLAME_RUNS`]; past the cap the set is labelled inclusion-minimal only.
const EXHAUSTIVE_CELLS: usize = 32;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Metal {
    Gold,
    Copper,
}

/// The families, declared in the order of their names, which is the order instances are sorted in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Family {
    Ownership,
    QuarterSum,
    /// Family 4: `Total Gold` = Σ the release's regions; `Total Copper` = Σ the copper sites.
    RegionToTotal,
    SiteSheet,
    /// Family 3: a region = Σ its registered members in the release's map.
    SitesToRegion,
}

impl Family {
    pub const ALL: [Self; 5] = [
        Self::Ownership,
        Self::QuarterSum,
        Self::RegionToTotal,
        Self::SiteSheet,
        Self::SitesToRegion,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Ownership => "ownership",
            Self::QuarterSum => QUARTER_SUM,
            Self::RegionToTotal => "region_to_total",
            Self::SiteSheet => "site_sheet",
            Self::SitesToRegion => "sites_to_region",
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
    /// A region row whose region has no registered member in the release's map.
    RegionNotInMap,
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
            Self::RegionNotInMap => "region_not_in_map",
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
    /// A site-sheet instance decided as number = 0 because the other print is the nil token (`nil_token`); it is
    /// never posed jointly.
    pub nil_token: bool,
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
    pub aliases: &'a [MineStatsLabelAlias],
    /// The registered shares, for the label a subject prints under in Mine Stats ("Nevada Gold Mines LLC" for
    /// the subject "Nevada Gold Mines"); the sum families find a member's row by it.
    pub labels: &'a [RegisteredShare],
}

impl LedgerRules<'_> {
    fn members_of(&self, region: &str, release: &str) -> BTreeSet<String> {
        self.region_members
            .iter()
            .filter(|m| m.region == region && m.holds_for(release))
            .map(|m| m.member.clone())
            .collect()
    }

    /// `region`'s members in the release's map, in registry row order.
    fn members_in_order(&self, region: &str, release: &str) -> Vec<String> {
        self.region_members
            .iter()
            .filter(|m| m.region == region && m.holds_for(release))
            .map(|m| m.member.clone())
            .collect()
    }

    /// The regions of the release's map, in the order of their first registry row.
    fn map_regions(&self, release: &str) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for m in self.region_members.iter().filter(|m| m.holds_for(release)) {
            if !out.contains(&m.region) {
                out.push(m.region.clone());
            }
        }
        out
    }

    /// The Mine Stats label `subject` prints under, where it is registered and differs from the subject.
    fn label_of(&self, subject: &str) -> Option<&str> {
        self.labels
            .iter()
            .find(|s| s.subject == subject && s.label != subject)
            .map(|s| s.label.as_str())
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

/// A cell as an operand.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Operand {
    Number { value: Rat, step: Rat },
    Placeholder(Mark),
    Blank,
    Unreadable,
}

/// A placeholder's kind. Ownership and quarter sums read every kind alike; the site-sheet family reads only the
/// nil token (`"—"` or `"— "` exactly) and `"n/a"` (exactly), and any other placeholder as unreadable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mark {
    Nil,
    NotApplicable,
    /// Another spelling, or a number shown as a format token.
    Other,
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

/// `(release, metal, row label as read, period, site)`: a site's cell on a `Minestats*` sheet.
type SiteKey = (String, Metal, String, Period, String);

/// The `Minestats*` cells of every release read.
#[derive(Debug, Default)]
struct SiteSheets {
    /// Every cell of a key with its row, in row order (an aliased label beside its registered one gives two), and
    /// whether the reader found the row's label printed again in its block.
    cells: BTreeMap<SiteKey, Vec<(u32, bool, Slot)>>,
    /// `(release, metal, site)` → the sheets of that metal with a column for the site.
    sheets: BTreeMap<(String, Metal, String), BTreeSet<String>>,
}

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
        MineStatsCell::Nil { token } if token == "—" || token == "— " => {
            Operand::Placeholder(Mark::Nil)
        }
        MineStatsCell::NotApplicable { token } if token == "n/a" => {
            Operand::Placeholder(Mark::NotApplicable)
        }
        MineStatsCell::Nil { .. } | MineStatsCell::NotApplicable { .. } => {
            Operand::Placeholder(Mark::Other)
        }
        MineStatsCell::Number { .. } => {
            let shown = workbook
                .sheet(&v.sheet)
                .zip(parse_a1_ref(&v.cell))
                .and_then(|(s, (row, col))| s.cell(row, col))
                .and_then(|c| c.displayed());
            match shown {
                Some(Ok(d)) if d.token.is_some() => Operand::Placeholder(Mark::Other),
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

fn is_flow(field: &str) -> bool {
    FLOW_FIELDS.iter().flatten().any(|f| *f == field)
}

/// Every flow cell and every [`SITE_FIELDS`] cell of the summary sheets, by key. A key read twice (one subject
/// on two rows of a block) keeps neither value: both become unreadable.
fn slots(files: &[LedgerFile<'_>]) -> BTreeMap<SlotKey, Slot> {
    let mut out: BTreeMap<SlotKey, Slot> = BTreeMap::new();
    for f in files {
        let Some((year, _)) = release_parts(f.quarter) else {
            continue;
        };
        for v in f.read.values.iter().filter(|v| {
            v.view == View::Summary
                && (v.sheet == "QTD" || v.sheet == "YTD")
                && (is_flow(&v.label) || SITE_FIELDS.iter().any(|(_, s, _)| *s == v.label))
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

/// The metal a `Minestats*` sheet's A1 title names: `(Gold)` or `(Copper)`, not both.
fn sheet_metal(workbook: &Workbook, sheet: &str) -> Option<Metal> {
    let title = match &workbook.sheet(sheet)?.cell(1, 1)?.value {
        CellValue::Text(t) => t,
        _ => return None,
    };
    match (title.contains("(Gold)"), title.contains("(Copper)")) {
        (true, false) => Some(Metal::Gold),
        (false, true) => Some(Metal::Copper),
        _ => None,
    }
}

/// A `Minestats*` row label as matched: whitespace collapsed, lower case, and read as its registered label
/// where an alias holds for the release.
fn site_label(aliases: &[MineStatsLabelAlias], release: &str, label: &str) -> String {
    let norm = |s: &str| {
        s.split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
            .to_lowercase()
    };
    let label = norm(label);
    aliases
        .iter()
        .find(|a| a.holds_for(release) && norm(&a.label) == label)
        .map_or(label, |a| norm(&a.reads_as))
}

/// Every cell of the `Minestats*` sheets whose title names one metal.
fn site_sheets(files: &[LedgerFile<'_>], aliases: &[MineStatsLabelAlias]) -> SiteSheets {
    let mut out = SiteSheets::default();
    for f in files {
        let mut metals: BTreeMap<&str, Option<Metal>> = BTreeMap::new();
        let repeated: BTreeSet<&str> = f.read.repeated.iter().map(String::as_str).collect();
        for v in f.read.values.iter().filter(|v| v.view == View::Minestats) {
            let metal = *metals
                .entry(v.sheet.as_str())
                .or_insert_with(|| sheet_metal(f.workbook, &v.sheet));
            let (Some(metal), Some(period), Some((row, _))) =
                (metal, parse_period(&v.period), parse_a1_ref(&v.cell))
            else {
                continue;
            };
            out.sheets
                .entry((f.quarter.to_string(), metal, v.site.clone()))
                .or_default()
                .insert(v.sheet.clone());
            let slot = Slot {
                locator: v.locator.clone(),
                operand: operand(v, f.workbook),
                period,
                printed_share: None,
            };
            let key = (
                f.quarter.to_string(),
                metal,
                site_label(aliases, f.quarter, &v.label),
                period,
                v.site.clone(),
            );
            let twice = repeated.contains(format!("{}:{}!A{row}", f.quarter, v.sheet).as_str());
            out.cells.entry(key).or_default().push((row, twice, slot));
        }
    }
    for cells in out.cells.values_mut() {
        cells.sort_by_key(|(row, _, _)| *row);
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
    if ops
        .iter()
        .all(|s| matches!(s.operand, Operand::Placeholder(_)))
    {
        return Some(State::NotPublished);
    }
    if let Some(at) = first(|o| matches!(o, Operand::Placeholder(_))) {
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
                nil_token: false,
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
    for ((release, _, field, group, subject), total) in slots
        .iter()
        .filter(|((_, s, f, _, _), _)| s == "YTD" && is_flow(f))
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
            nil_token: false,
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

/// Whether 0 lies in the number's closed box `[v − step/2, v + step/2]`; `None` on overflow.
fn box_holds_zero(value: Rat, step: Rat) -> Option<bool> {
    let half = step.checked_div(Rat::from_integer(2)).ok()?;
    let lo = value.checked_sub(half).ok()?;
    let hi = value.checked_add(half).ok()?;
    Some(lo.signum() <= 0 && hi.signum() >= 0)
}

/// Steps 3–8 of family 7 for a summary operand `s` and its site-sheet counterpart `m`: the state, with the
/// posing for two numbers, and whether the nil token decided it.
fn decide_site(s: &Slot, m: &Slot, posed: xf::Identity) -> (State, Option<Posed>, bool) {
    let refused = |reason, at: &Slot| State::Refused {
        reason,
        at: Some(at.locator.clone()),
    };
    let ops = [s, m];
    if let Some(b) = ops.iter().find(|o| o.operand == Operand::Blank) {
        return (refused(Reason::BlankOperand, b), None, false);
    }
    let token = |o: &Slot| {
        matches!(
            o.operand,
            Operand::Placeholder(Mark::Nil | Mark::NotApplicable)
        )
    };
    if ops.iter().all(|o| token(o)) {
        return (State::NotPublished, None, false);
    }
    let number = ops.iter().find_map(|o| match o.operand {
        Operand::Number { value, step } => Some((value, step)),
        _ => None,
    });
    let other = ops
        .iter()
        .find(|o| !matches!(o.operand, Operand::Number { .. }));
    match (number, other.map(|o| (&o.operand, *o))) {
        (Some((value, step)), Some((Operand::Placeholder(Mark::Nil), _))) => {
            let state = match box_holds_zero(value, step) {
                Some(true) => State::Consistent,
                Some(false) => State::Discrepancy,
                None => kernel_refused("overflow testing the nil token".to_string()),
            };
            return (state, None, true);
        }
        (Some(_), Some((Operand::Placeholder(Mark::NotApplicable), na))) => {
            return (refused(Reason::PlaceholderOperand, na), None, false);
        }
        _ => {}
    }
    if let Some(u) = ops.iter().find(|o| {
        matches!(
            o.operand,
            Operand::Unreadable | Operand::Placeholder(Mark::Other)
        )
    }) {
        return (refused(Reason::OperandUnreadable, u), None, false);
    }
    let p = Posed {
        cells: ops.into_iter().filter_map(xf_cell).collect(),
        identity: posed,
    };
    (decide(&p), Some(p), false)
}

/// Family 7: each summary value of a [`SITE_FIELDS`] field whose subject has a column on a `Minestats*` sheet of
/// the field's metal in the same release.
fn site_sheet(
    rules: &LedgerRules<'_>,
    slots: &BTreeMap<SlotKey, Slot>,
    sites: &SiteSheets,
    out: &mut Vec<(Instance, Option<Posed>)>,
) {
    for ((release, _, field, group, subject), s) in slots {
        let Some(&(metal, _, label)) = SITE_FIELDS.iter().find(|(_, f, _)| f == field) else {
            continue;
        };
        let Some(sheets) = sites.sheets.get(&(release.clone(), metal, subject.clone())) else {
            continue;
        };
        let two_sheets = sheets.len() > 1;
        let found = sites
            .cells
            .get(&(
                release.clone(),
                metal,
                label.to_string(),
                s.period,
                subject.clone(),
            ))
            .filter(|_| !two_sheets);
        let first = found.and_then(|cells| cells.first()).map(|(_, _, m)| m);
        let mut instance = Instance {
            family: Family::SiteSheet,
            subject: subject.clone(),
            field: field.clone(),
            group: *group,
            period: s.period,
            anchor: s.locator.clone(),
            cells: vec![Some(s.locator.clone()), first.map(|m| m.locator.clone())],
            state: State::Consistent,
            share: None,
            printed_share: None,
            nil_token: false,
            component: None,
        };
        let refused = |reason, at: Option<&Slot>| State::Refused {
            reason,
            at: at.map(|m| m.locator.clone()),
        };
        let mut posed = None;
        instance.state = if two_sheets {
            refused(Reason::OperandUnreadable, None)
        } else if let Some(evidence) = rules.withheld(subject, &s.period) {
            refused(Reason::Withheld { evidence }, None)
        } else if let Some(m) = first {
            if found
                .is_some_and(|cells| cells.len() > 1 || cells.iter().any(|(_, twice, _)| *twice))
            {
                refused(Reason::OperandUnreadable, Some(m))
            } else {
                let identity = xf::Identity {
                    id: identity_id(Family::SiteSheet, &s.locator),
                    family: Family::SiteSheet.as_str().to_string(),
                    closure: xf::Closure::Definitional,
                    kind: xf::IdentityKind::Equal {
                        a: s.locator.clone(),
                        b: m.locator.clone(),
                    },
                };
                let (state, p, nil_token) = decide_site(s, m, identity);
                posed = p;
                instance.nil_token = nil_token;
                state
            }
        } else {
            refused(Reason::OperandRowMissing, None)
        };
        out.push((instance, posed));
    }
}

/// The row of a locator `release:sheet!A1`; 0 when it does not parse (locators come from the reader, so it does).
fn row_of(locator: &str) -> u32 {
    locator
        .rsplit_once('!')
        .and_then(|(_, cell)| parse_a1_ref(cell))
        .map_or(0, |(row, _)| row)
}

/// A sum operand's row in a block: found once, absent, or found under both the subject and its registered
/// label (the cell of the lower row number).
enum Found<'s> {
    One(&'s Slot),
    Missing,
    Both(&'s Slot),
}

fn find_member<'s>(
    rows: &BTreeMap<&str, &'s Slot>,
    rules: &LedgerRules<'_>,
    subject: &str,
) -> Found<'s> {
    let mut hits: Vec<&Slot> = [Some(subject), rules.label_of(subject)]
        .into_iter()
        .flatten()
        .filter_map(|name| rows.get(name).copied())
        .collect();
    hits.sort_by_key(|s| row_of(&s.locator));
    match hits.as_slice() {
        [] => Found::Missing,
        [one] => Found::One(one),
        [first, ..] => Found::Both(first),
    }
}

/// Whether Σ parts = total holds for true values in the operands' closed boxes, the nil token being exactly 0:
/// |Σ parts − total| ≤ Σ step/2 over every operand. `None` on overflow. Every operand is a number or the nil
/// token.
fn sum_holds(parts: &[&Slot], total: &Slot) -> Option<bool> {
    let two = Rat::from_integer(2);
    let (mut gap, mut slack) = (Rat::from_integer(0), Rat::from_integer(0));
    for (k, o) in parts.iter().chain(std::iter::once(&total)).enumerate() {
        let (value, half) = match o.operand {
            Operand::Number { value, step } => (value, step.checked_div(two).ok()?),
            Operand::Placeholder(Mark::Nil) => (Rat::from_integer(0), Rat::from_integer(0)),
            _ => return None,
        };
        gap = if k < parts.len() {
            gap.checked_add(value).ok()?
        } else {
            gap.checked_sub(value).ok()?
        };
        slack = slack.checked_add(half).ok()?;
    }
    let gap = if gap.signum() < 0 {
        gap.checked_neg().ok()?
    } else {
        gap
    };
    Some(gap.checked_cmp(slack).ok()? != std::cmp::Ordering::Greater)
}

/// Families 3 and 4 on each summary sheet, flow field and column group: a region's value is the sum of its
/// registered members in the release's map (`sites_to_region`); `Total Gold` is the sum of the map's regions and
/// `Total Copper` the sum of the copper block's sites (`region_to_total`). A member's row is found by its subject
/// or by its registered Mine Stats label. Each instance is decided alone, in exact arithmetic, and never posed
/// jointly: a sum links every site of a block, so posing the sums would merge the other families' components
/// into ones far past the exhaustive correction search.
fn sums(
    rules: &LedgerRules<'_>,
    slots: &BTreeMap<SlotKey, Slot>,
    out: &mut Vec<(Instance, Option<Posed>)>,
) {
    let mut blocks: BTreeMap<(&str, &str, &str, Group), BTreeMap<&str, &Slot>> = BTreeMap::new();
    for ((release, sheet, field, group, subject), slot) in slots {
        if is_flow(field) {
            blocks
                .entry((release, sheet, field, *group))
                .or_default()
                .insert(subject, slot);
        }
    }
    for ((release, _, field, group), rows) in &blocks {
        // (family, aggregate, operand subjects in order, a refusal decided before the operands)
        let mut posings: Vec<(Family, &str, Vec<String>, Option<Reason>)> = Vec::new();
        if FLOW_FIELDS[0].contains(field) {
            for region in rows
                .keys()
                .filter(|s| rules.regions.iter().any(|r| r == **s))
            {
                let members = rules.members_in_order(region, release);
                let why = members.is_empty().then_some(Reason::RegionNotInMap);
                posings.push((Family::SitesToRegion, region, members, why));
            }
            posings.push((
                Family::RegionToTotal,
                "Total Gold",
                rules.map_regions(release),
                None,
            ));
        } else {
            let mut sites: Vec<(&str, u32)> = rows
                .iter()
                .filter(|(s, _)| !rules.aggregate(s))
                .map(|(s, slot)| (*s, row_of(&slot.locator)))
                .collect();
            sites.sort_by_key(|(_, row)| *row);
            let sites = sites.into_iter().map(|(s, _)| s.to_string()).collect();
            posings.push((Family::RegionToTotal, "Total Copper", sites, None));
        }
        for (family, aggregate, parts, why) in posings {
            // No aggregate row, no anchor: nothing is posed (the census stops there; no registered file lacks it).
            let Some(&total) = rows.get(aggregate) else {
                continue;
            };
            let found: Vec<Found<'_>> = parts.iter().map(|p| find_member(rows, rules, p)).collect();
            let mut cells: Vec<Option<String>> = found
                .iter()
                .map(|f| match f {
                    Found::One(s) | Found::Both(s) => Some(s.locator.clone()),
                    Found::Missing => None,
                })
                .collect();
            cells.push(Some(total.locator.clone()));
            let refused = |reason, at: Option<&Slot>| State::Refused {
                reason,
                at: at.map(|s| s.locator.clone()),
            };
            let withheld = (family == Family::SitesToRegion)
                .then(|| parts.iter().find_map(|m| rules.withheld(m, &total.period)))
                .flatten();
            let present: Vec<&Slot> = found
                .iter()
                .filter_map(|f| match f {
                    Found::One(s) | Found::Both(s) => Some(*s),
                    Found::Missing => None,
                })
                .collect();
            let ops: Vec<&Slot> = present
                .iter()
                .copied()
                .chain(std::iter::once(total))
                .collect();
            let first = |p: &dyn Fn(&Operand) -> bool| ops.iter().copied().find(|s| p(&s.operand));
            let mut nil_token = false;
            let state = if let Some(evidence) = withheld {
                refused(Reason::Withheld { evidence }, None)
            } else if let Some(reason) = why {
                refused(reason, None)
            } else if parts.is_empty() || found.iter().any(|f| matches!(f, Found::Missing)) {
                refused(Reason::OperandRowMissing, None)
            } else if let Some(Found::Both(s)) = found.iter().find(|f| matches!(f, Found::Both(_)))
            {
                refused(Reason::OperandUnreadable, Some(s))
            } else if let Some(b) = first(&|o| *o == Operand::Blank) {
                refused(Reason::BlankOperand, Some(b))
            } else if ops
                .iter()
                .all(|s| matches!(s.operand, Operand::Placeholder(_)))
            {
                State::NotPublished
            } else if let Some(p) =
                first(&|o| matches!(o, Operand::Placeholder(m) if *m != Mark::Nil))
            {
                refused(Reason::PlaceholderOperand, Some(p))
            } else if let Some(u) = first(&|o| *o == Operand::Unreadable) {
                refused(Reason::OperandUnreadable, Some(u))
            } else {
                match sum_holds(&present, total) {
                    Some(holds) => {
                        nil_token = ops
                            .iter()
                            .any(|s| s.operand == Operand::Placeholder(Mark::Nil));
                        if holds {
                            State::Consistent
                        } else {
                            State::Discrepancy
                        }
                    }
                    None => kernel_refused("overflow in the exact sum".to_string()),
                }
            };
            out.push((
                Instance {
                    family,
                    subject: aggregate.to_string(),
                    field: (*field).to_string(),
                    group: *group,
                    period: total.period,
                    anchor: total.locator.clone(),
                    cells,
                    state,
                    share: None,
                    printed_share: None,
                    nil_token,
                    component: None,
                },
                None,
            ));
        }
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
    let blamed = xf::blame_with_caps(
        &cells,
        &identities,
        &rules,
        EXHAUSTIVE_CELLS,
        xf::MAX_BLAME_RUNS,
    )
    .unwrap_or_default();
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
    site_sheet(
        rules,
        &slots,
        &site_sheets(files, rules.aliases),
        &mut pairs,
    );
    sums(rules, &slots, &mut pairs);
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
        aliases: &[],
        labels: &[],
    };

    const GOLD: [&str; 3] = [
        "Gold produced (000s oz 100%)",
        "Gold produced (000s oz attributable)",
        "Gold sold (000s oz attributable)",
    ];

    const COPPER: [&str; 3] = [
        "Copper produced (thousands of tonnes 100%)",
        "Copper produced (thousands of tonnes attributable)",
        "Copper sold (thousands of tonnes attributable)",
    ];

    fn text(s: &str) -> MinimalCell {
        MinimalCell::Text(s.to_string())
    }

    /// A data cell from its spec (see [`Row`]); a spec holding `—` is that text as printed. `None` for blank.
    fn data(v: &str) -> Option<MinimalCell> {
        let number = |raw: &str, xf| MinimalCell::Styled {
            raw: raw.to_string(),
            xf,
        };
        match v {
            "" => None,
            _ if v.contains('—') || v == "n/a" => Some(text(v)),
            _ => Some(match (v.strip_prefix("##"), v.strip_prefix('#')) {
                (Some(raw), _) => number(raw, 2),
                (None, Some(raw)) => number(raw, 1),
                (None, None) => number(v, 0),
            }),
        }
    }

    /// A summary sheet with one block of the release's own year: `headers` in B4–D4, rows from row 5.
    fn summary(name: &str, months: &str, headers: [&str; 3], rows: &[Row<'_>]) -> MinimalSheet {
        let mut cells = vec![
            (
                "A3".to_string(),
                text(&format!("For the {months} months ended December 31,")),
            ),
            (
                "B3".to_string(),
                MinimalCell::Styled {
                    raw: "2030".to_string(),
                    xf: 0,
                },
            ),
        ];
        for (col, h) in ["B", "C", "D"].into_iter().zip(headers) {
            cells.push((format!("{col}4"), text(h)));
        }
        for (k, (label, values)) in rows.iter().enumerate() {
            let row = 5 + k;
            cells.push((format!("A{row}"), text(label)));
            for (col, v) in ["B", "C", "D"].into_iter().zip(values) {
                if let Some(c) = data(v) {
                    cells.push((format!("{col}{row}"), c));
                }
            }
        }
        MinimalSheet {
            name: name.to_string(),
            hidden: false,
            cells,
        }
    }

    /// A `Minestats*` sheet titled `title` with one three-month block: each site's name on row 2 over its two
    /// columns (from B), the block header on row 3 with 2030 and 2029 in each site's columns, then from row 4 one
    /// row per label with each site's current-year and prior-year cells (specs as in [`Row`]).
    fn minestats(
        name: &str,
        title: &str,
        sites: &[&str],
        rows: &[(&str, Vec<&str>)],
    ) -> MinimalSheet {
        const COLS: [&str; 6] = ["B", "C", "D", "E", "F", "G"];
        let mut cells = vec![
            ("A1".to_string(), text(title)),
            (
                "A3".to_string(),
                text("For the three months ended March 31,"),
            ),
        ];
        for (k, site) in sites.iter().enumerate() {
            let (cur, prior) = (COLS[2 * k], COLS[2 * k + 1]);
            cells.push((format!("{cur}2"), text(site)));
            for (col, year) in [(cur, "2030"), (prior, "2029")] {
                let year = MinimalCell::Styled {
                    raw: year.to_string(),
                    xf: 0,
                };
                cells.push((format!("{col}3"), year));
            }
        }
        for (k, (label, values)) in rows.iter().enumerate() {
            let row = 4 + k;
            cells.push((format!("A{row}"), text(label)));
            for (col, v) in COLS.into_iter().zip(values) {
                if let Some(c) = data(v) {
                    cells.push((format!("{col}{row}"), c));
                }
            }
        }
        MinimalSheet {
            name: name.to_string(),
            hidden: false,
            cells,
        }
    }

    /// The ledger over `(quarter, sheets)` workbooks, with the reader's refusal locators.
    fn run(
        books: Vec<(String, Vec<MinimalSheet>)>,
        rules: &LedgerRules<'_>,
    ) -> (MineLedger, Vec<String>) {
        let styles = MinimalStyles {
            num_fmts: vec![(164, "0.0".to_string())],
            xfs: vec![0, 164, 2],
        };
        let books: Vec<(String, Workbook)> = books
            .into_iter()
            .map(|(q, sheets)| {
                let bytes = build_minimal_xlsx_styled(&sheets, Some(&styles));
                let wb = Workbook::from_bytes("synthetic_mine_ledger.xlsx", &bytes)
                    .expect("synthetic workbook parses");
                (q, wb)
            })
            .collect();
        let reads: Vec<MineStatsRead> = books
            .iter()
            .map(|(q, wb)| read_mine_stats(wb, q, None).expect("reads"))
            .collect();
        let refusals = reads
            .iter()
            .flat_map(|r| &r.refusals)
            .map(|r| r.locator.clone())
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
        (mine_ledger(&files, rules), refusals)
    }

    fn ledger(releases: &[Release<'_>], rules: &LedgerRules<'_>) -> MineLedger {
        let books = releases
            .iter()
            .map(|(n, qtd, ytd)| {
                let mut sheets = vec![summary("QTD", "three", GOLD, qtd)];
                if *n > 1 {
                    let months = ["three", "six", "nine", "twelve"][usize::from(*n) - 1];
                    sheets.push(summary("YTD", months, GOLD, ytd));
                }
                (format!("2030Q{n}"), sheets)
            })
            .collect();
        let (l, refusals) = run(books, rules);
        assert!(refusals.is_empty(), "{refusals:?}");
        l
    }

    /// Release `2030Q1`: a `QTD` sheet of `metal` headers and `rows`, beside the given `Minestats*` sheets.
    fn site_ledger(
        metal: [&str; 3],
        rows: &[Row<'_>],
        sheets: Vec<MinimalSheet>,
        rules: &LedgerRules<'_>,
    ) -> (MineLedger, Vec<String>) {
        let mut all = vec![summary("QTD", "three", metal, rows)];
        all.extend(sheets);
        run(vec![("2030Q1".to_string(), all)], rules)
    }

    const AU: &str = "Mine Statistics (Gold)";
    const CU: &str = "Mine Statistics (Copper)";

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
        let owned: Vec<&Instance> = l
            .instances
            .iter()
            .filter(|i| i.family == Family::Ownership)
            .collect();
        let [hemlo] = owned.as_slice() else {
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

    fn site_at(l: &MineLedger, cell: &str) -> (State, Option<String>) {
        let i = at(l, Family::SiteSheet, &format!("2030Q1:QTD!{cell}"));
        (i.state.clone(), i.cells.get(1).cloned().flatten())
    }

    fn ms(cell: &str) -> Option<String> {
        Some(format!("2030Q1:Minestats1!{cell}"))
    }

    /// Family 7: a summary value pairs with its site's print of the same row label (matched in lower case,
    /// whitespace collapsed, footnote stripped), period and year. 90 and 92 are boxes [89.5, 90.5] and
    /// [91.5, 92.5]: (iii), posed jointly, either print alone a correction. A subject with no column on a site
    /// sheet of the metal poses none, nor does the 100% column.
    #[test]
    fn test_site_sheet_pairs_each_summary_value_with_its_site_print() {
        let (l, refusals) = site_ledger(
            GOLD,
            &[
                ("Hemlo", ["", "100", "90"]),
                ("Turquoise Ridge", ["", "50", "45"]),
            ],
            vec![minestats(
                "Minestats1",
                AU,
                &["Hemlo2"],
                &[
                    ("Production (Thousands of  ounces)", vec!["100", "80"]),
                    ("Sales (thousands of ounces)1", vec!["92", "85"]),
                ],
            )],
            &NONE,
        );
        assert!(refusals.is_empty(), "{refusals:?}");
        assert_eq!(site_at(&l, "C5"), (State::Consistent, ms("B4")));
        assert_eq!(site_at(&l, "D5"), (State::Discrepancy, ms("B5")));
        assert_eq!(
            l.instances
                .iter()
                .filter(|i| i.family == Family::SiteSheet)
                .count(),
            2
        );
        let [c] = l.infeasible.as_slice() else {
            panic!("one infeasible component: {:?}", l.infeasible);
        };
        assert_eq!(c.id, "2030Q1:Minestats1!B5");
        assert_eq!(
            at(&l, Family::SiteSheet, "2030Q1:QTD!D5")
                .component
                .as_deref(),
            Some(c.id.as_str())
        );
        assert!(c.minimum && !c.joint_only);
        assert_eq!(sets(c), [["2030Q1:Minestats1!B5"], ["2030Q1:QTD!D5"]]);
        assert_eq!(l.components, 2);
    }

    /// The nil token against a number reads as number = 0 and is decided alone (E116's shape): 5 is [4.5, 5.5],
    /// (iii); 0 is [−0.5, 0.5], (i). Two tokens are not published; `n/a` against a number is refused at the `n/a`.
    #[test]
    fn test_site_sheet_nil_token_and_placeholders() {
        let (l, refusals) = site_ledger(
            GOLD,
            &[
                ("Hemlo", ["", "5", "0"]),
                ("Tongon (89.7%)", ["", "—", "12"]),
            ],
            vec![minestats(
                "Minestats1",
                AU,
                &["Hemlo", "Tongon (89.7%)"],
                &[
                    ("Production (thousands of ounces)", vec!["—", "", "n/a", ""]),
                    ("Sales (thousands of ounces)", vec!["— ", "", "n/a", ""]),
                ],
            )],
            &NONE,
        );
        assert!(refusals.is_empty(), "{refusals:?}");
        assert_eq!(site_at(&l, "C5"), (State::Discrepancy, ms("B4")));
        assert_eq!(site_at(&l, "D5"), (State::Consistent, ms("B5")));
        for cell in ["C5", "D5"] {
            let i = at(&l, Family::SiteSheet, &format!("2030Q1:QTD!{cell}"));
            assert!(i.nil_token && i.component.is_none(), "{cell}");
        }
        assert_eq!(site_at(&l, "C6"), (State::NotPublished, ms("D4")));
        assert_eq!(
            site_at(&l, "D6").0,
            refused(Reason::PlaceholderOperand, Some("2030Q1:Minestats1!D5"))
        );
        // Nothing is posed jointly.
        assert_eq!((l.components, l.infeasible.len()), (0, 0));
    }

    /// Only `"—"` and `"— "` are the nil token here: another spelling of it is unreadable, never read as zero.
    #[test]
    fn test_other_nil_spelling_is_unreadable() {
        let (l, refusals) = site_ledger(
            GOLD,
            &[("Hemlo", ["", "5", "0"])],
            vec![minestats(
                "Minestats1",
                AU,
                &["Hemlo"],
                &[
                    ("Production (thousands of ounces)", vec!["  —", ""]),
                    ("Sales (thousands of ounces)", vec!["—", ""]),
                ],
            )],
            &NONE,
        );
        assert!(refusals.is_empty(), "{refusals:?}");
        assert_eq!(
            site_at(&l, "C5"),
            (
                refused(Reason::OperandUnreadable, Some("2030Q1:Minestats1!B4")),
                ms("B4")
            )
        );
        assert_eq!(site_at(&l, "D5").0, State::Consistent);
    }

    /// The census's order: a subject on two site sheets of one metal; a withheld subject (its counterpart still
    /// named); a missing row; then a blank or unreadable operand at that cell.
    #[test]
    fn test_site_sheet_refusals_in_order() {
        let statuses = [SubjectStatus {
            subject: "Loulo-Gounkoto".to_string(),
            kind: StatusKind::Withheld,
            first: (2030, 1),
            last: Some((2030, 3)),
            evidence: "E818".to_string(),
        }];
        let (l, refusals) = site_ledger(
            GOLD,
            &[
                ("Hemlo", ["", "100", "90"]),
                ("Loulo-Gounkoto (80%)", ["", "50", "40"]),
                ("Kibali (45%)", ["", "30", "20"]),
                ("Tongon (89.7%)", ["", "60", "55"]),
            ],
            vec![
                minestats(
                    "Minestats1",
                    AU,
                    &["Hemlo", "Loulo-Gounkoto (80%)"],
                    &[
                        ("Production (thousands of ounces)", vec!["", "", "50", ""]),
                        ("Sales (thousands of ounces)", vec!["90.4", "", "40", ""]),
                    ],
                ),
                minestats(
                    "Minestats2",
                    AU,
                    &["Kibali (45%)", "Tongon (89.7%)"],
                    &[("Production (thousands of ounces)", vec!["30", "", "60", ""])],
                ),
                minestats(
                    "Minestats3",
                    AU,
                    &["Kibali (45%)"],
                    &[("Production (thousands of ounces)", vec!["30", ""])],
                ),
            ],
            &LedgerRules {
                statuses: &statuses,
                ..NONE
            },
        );
        assert!(refusals.is_empty(), "{refusals:?}");
        let blank = refused(Reason::BlankOperand, Some("2030Q1:Minestats1!B4"));
        assert_eq!(site_at(&l, "C5"), (blank, ms("B4")));
        // General shows 90.4 by column width: unreadable, never guessed.
        let unreadable = refused(Reason::OperandUnreadable, Some("2030Q1:Minestats1!B5"));
        assert_eq!(site_at(&l, "D5"), (unreadable, ms("B5")));
        let withheld = refused(
            Reason::Withheld {
                evidence: "E818".to_string(),
            },
            None,
        );
        assert_eq!(site_at(&l, "C6"), (withheld.clone(), ms("D4")));
        assert_eq!(site_at(&l, "D6"), (withheld, ms("D5")));
        for cell in ["C7", "D7"] {
            assert_eq!(
                site_at(&l, cell),
                (refused(Reason::OperandUnreadable, None), None)
            );
        }
        assert_eq!(
            site_at(&l, "C8"),
            (State::Consistent, Some("2030Q1:Minestats2!D4".to_string()))
        );
        assert_eq!(
            site_at(&l, "D8"),
            (refused(Reason::OperandRowMissing, None), None)
        );
    }

    /// W1-DR-02: a registered alias reads a stale row label as the registered one, for its releases only, and
    /// converts nothing. The stale row beside the registered one is one label printed twice: refused at the first.
    #[test]
    fn test_label_alias_reads_a_stale_row_for_its_releases() {
        let alias = |what: &str, first: &str, last: &str| MineStatsLabelAlias {
            label: format!("{what} (millions of pounds)"),
            reads_as: format!("{what} (Thousands of tonnes)"),
            first: first.to_string(),
            last: last.to_string(),
            evidence: "W1-DR-02".to_string(),
        };
        let states = |aliases: &[MineStatsLabelAlias], both: bool| {
            let mut rows = vec![
                ("Production (millions of pounds)", vec!["30", ""]),
                ("Sales (millions of pounds)", vec!["29", ""]),
            ];
            if both {
                rows.push(("Production (thousands of tonnes)", vec!["30", ""]));
            }
            let (l, refusals) = site_ledger(
                COPPER,
                &[("Lumwana", ["", "30", "29"])],
                vec![minestats("Minestats1", CU, &["Lumwana"], &rows)],
                &LedgerRules { aliases, ..NONE },
            );
            assert!(refusals.is_empty(), "{refusals:?}");
            [site_at(&l, "C5"), site_at(&l, "D5")]
        };
        let missing = (refused(Reason::OperandRowMissing, None), None);
        assert_eq!(states(&[], false), [missing.clone(), missing.clone()]);
        let now = [
            alias("Production", "2030Q1", "2030Q4"),
            alias("Sales", "2030Q1", "2030Q4"),
        ];
        assert_eq!(
            states(&now, false),
            [(State::Consistent, ms("B4")), (State::Consistent, ms("B5"))]
        );
        let later = [
            alias("Production", "2031Q1", "2031Q4"),
            alias("Sales", "2031Q1", "2031Q4"),
        ];
        assert_eq!(states(&later, false), [missing.clone(), missing]);
        let [production, sales] = states(&now, true);
        assert_eq!(
            production,
            (
                refused(Reason::OperandUnreadable, Some("2030Q1:Minestats1!B4")),
                ms("B4")
            )
        );
        assert_eq!(sales, (State::Consistent, ms("B5")));
    }

    /// A row label printed twice in one block: the reader refuses the second row, and the ledger refuses the
    /// pair at the first rather than take one print as the label's only one.
    #[test]
    fn test_label_printed_twice_is_refused_at_the_first() {
        let (l, refusals) = site_ledger(
            GOLD,
            &[("Hemlo", ["", "100", "90"])],
            vec![minestats(
                "Minestats1",
                AU,
                &["Hemlo"],
                &[
                    ("Production (thousands of ounces)", vec!["100", ""]),
                    ("Sales (thousands of ounces)", vec!["90", ""]),
                    ("Production (thousands of ounces)", vec!["101", ""]),
                ],
            )],
            &NONE,
        );
        assert_eq!(refusals, ["2030Q1:Minestats1!A6"]);
        assert_eq!(
            site_at(&l, "C5"),
            (
                refused(Reason::OperandUnreadable, Some("2030Q1:Minestats1!B4")),
                ms("B4")
            )
        );
        assert_eq!(site_at(&l, "D5"), (State::Consistent, ms("B5")));
    }

    /// The site print joins the ownership identity's component. Alone each holds: C = B/2 in [99.75, 100.25]
    /// meets C's [99.5, 100.5], and 100 meets 101 at 100.5. Together C is 100.5 and B = 201 misses
    /// [199.5, 200.5]. Disregarding B leaves C = 100.5 (B in [201, 201]); disregarding the site print leaves
    /// C in [99.75, 100.25] (the print in [399/4, 401/4]); disregarding C leaves the two in conflict.
    #[test]
    fn test_site_print_joins_the_ownership_component() {
        let (l, refusals) = site_ledger(
            GOLD,
            &[("Carlin (50%)", ["200", "100", "90"])],
            vec![minestats(
                "Minestats1",
                AU,
                &["Carlin (50%)"],
                &[
                    ("Production (thousands of ounces)", vec!["101", ""]),
                    ("Sales (thousands of ounces)", vec!["90", ""]),
                ],
            )],
            &NONE,
        );
        assert!(refusals.is_empty(), "{refusals:?}");
        assert_eq!(
            at(&l, Family::Ownership, "2030Q1:QTD!C5").state,
            State::Consistent
        );
        assert_eq!(site_at(&l, "C5"), (State::Consistent, ms("B4")));
        let [c] = l.infeasible.as_slice() else {
            panic!("one infeasible component: {:?}", l.infeasible);
        };
        assert!(c.joint_only && c.minimum);
        assert_eq!(c.id, "2030Q1:Minestats1!B4");
        assert_eq!(c.instances.len(), 2);
        assert_eq!(sets(c), [["2030Q1:Minestats1!B4"], ["2030Q1:QTD!B5"]]);
        let ranges: Vec<(&str, String)> = c
            .corrections
            .iter()
            .flat_map(|s| &s.implied)
            .map(|(cell, r)| (cell.as_str(), range_text(r)))
            .collect();
        assert_eq!(
            ranges,
            [
                ("2030Q1:Minestats1!B4", "[399/4, 401/4]".to_string()),
                ("2030Q1:QTD!B5", "[201, 201]".to_string()),
            ]
        );
        assert_eq!(l.components, 2);
    }

    fn member(region: &str, member: &str, first: &str) -> RegionMemberRow {
        RegionMemberRow {
            region: region.to_string(),
            member: member.to_string(),
            first: first.to_string(),
            last: None,
        }
    }

    /// Locators on release 2030Q1's `QTD` sheet, `None` kept.
    fn locs(cells: &[Option<&str>]) -> Vec<Option<String>> {
        cells
            .iter()
            .map(|c| c.map(|c| format!("2030Q1:QTD!{c}")))
            .collect()
    }

    fn sum_state(l: &MineLedger, family: Family, cell: &str) -> State {
        at(l, family, &format!("2030Q1:QTD!{cell}")).state.clone()
    }

    /// The first region sum on the 100% column.
    fn region_b(l: &MineLedger) -> &Instance {
        l.instances
            .iter()
            .find(|i| i.family == Family::SitesToRegion && i.field == "au_prod_100_koz")
            .expect("a region sum on the 100% column")
    }

    /// Release 2030Q1, rows 5–10: North America = Carlin + Hemlo, Latin America = Pueblo Viejo, `Total Gold` = the
    /// two regions.
    fn americas<'a>(na: [&'a str; 3], total: [&'a str; 3]) -> Vec<Release<'a>> {
        vec![(
            1,
            vec![
                ("Carlin (50%)", ["100", "50", "45"]),
                ("Hemlo (100%)", ["50", "50", "45"]),
                ("North America", na),
                ("Pueblo Viejo (60%)", ["100", "60", "55"]),
                ("Latin America", ["100", "60", "55"]),
                ("Total Gold", total),
            ],
            vec![],
        )]
    }

    #[test]
    fn test_region_and_total_sums_hold_within_the_rounding_slack() {
        let regions = ["North America".to_string(), "Latin America".to_string()];
        let members = [
            member("North America", "Carlin", "2030Q1"),
            member("North America", "Hemlo", "2030Q1"),
            member("Latin America", "Pueblo Viejo", "2030Q1"),
        ];
        let rules = LedgerRules {
            regions: &regions,
            region_members: &members,
            ..NONE
        };
        // Three operands shown to the unit: 100 + 50 meets the printed region within 1.5.
        for (printed, code) in [
            ("148", "iii"),
            ("149", "i"),
            ("150", "i"),
            ("151", "i"),
            ("152", "iii"),
        ] {
            let l = ledger(
                &americas([printed, "100", "90"], ["250", "160", "145"]),
                &rules,
            );
            let state = sum_state(&l, Family::SitesToRegion, "B7");
            assert_eq!(state.code(), code, "North America printed {printed}");
        }
        let l = ledger(
            &americas(["150", "102", "90"], ["250", "162", "145"]),
            &rules,
        );
        let na = at(&l, Family::SitesToRegion, "2030Q1:QTD!B7");
        assert_eq!(
            (na.subject.as_str(), na.field.as_str()),
            ("North America", "au_prod_100_koz")
        );
        assert_eq!(na.cells, locs(&[Some("B5"), Some("B6"), Some("B7")]));
        assert_eq!(
            sum_state(&l, Family::SitesToRegion, "C7"),
            State::Discrepancy
        );
        // The total adds the regions as printed: North America's discrepancy does not reach it.
        let total = at(&l, Family::RegionToTotal, "2030Q1:QTD!C10");
        assert_eq!(
            (total.subject.as_str(), &total.state),
            ("Total Gold", &State::Consistent)
        );
        assert_eq!(total.cells, locs(&[Some("C7"), Some("C9"), Some("C10")]));
        assert_eq!(
            [Family::SitesToRegion, Family::RegionToTotal]
                .map(|f| [l.count(f, "i"), l.count(f, "iii")]),
            [[5, 1], [3, 0]]
        );
        // Decided alone and never posed: the discrepancy is in no component.
        assert!(l.infeasible.is_empty());
        assert!(
            l.instances
                .iter()
                .all(|i| i.component.is_none() && !i.nil_token)
        );
    }

    #[test]
    fn test_sum_operands_nil_token_and_placeholders() {
        let regions = ["North America".to_string()];
        let members = [
            member("North America", "Carlin", "2030Q1"),
            member("North America", "Hemlo", "2030Q1"),
        ];
        let rules = LedgerRules {
            regions: &regions,
            region_members: &members,
            ..NONE
        };
        let states = |hemlo: [&str; 3], na: [&str; 3]| {
            let rows = vec![
                ("Carlin (50%)", ["100", "—", "45"]),
                ("Hemlo (100%)", hemlo),
                ("North America", na),
            ];
            let l = ledger(&[(1, rows, vec![])], &rules);
            ["B7", "C7", "D7"].map(|c| {
                let i = at(&l, Family::SitesToRegion, &format!("2030Q1:QTD!{c}"));
                (i.state.clone(), i.nil_token)
            })
        };
        let at6 = |c: &str| Some(format!("2030Q1:QTD!{c}6"));
        // The nil token is exactly 0 with no box: 100 + 0 meets 101 within 1.
        assert_eq!(
            states(["—", "—", "n/a"], ["101", "—", "45"]),
            [
                (State::Consistent, true),
                (State::NotPublished, false),
                (
                    State::Refused {
                        reason: Reason::PlaceholderOperand,
                        at: at6("D")
                    },
                    false
                ),
            ]
        );
        // 102 is past it; a blank comes before the placeholders; a nil region is 0 against 45.
        assert_eq!(
            states(["—", "", "—"], ["102", "50", "—"]),
            [
                (State::Discrepancy, true),
                (
                    State::Refused {
                        reason: Reason::BlankOperand,
                        at: at6("C")
                    },
                    false
                ),
                (State::Discrepancy, true),
            ]
        );
    }

    #[test]
    fn test_member_is_found_by_its_registered_label_and_refused_under_both() {
        let regions = ["North America".to_string()];
        let members = [
            member("North America", "Nevada Gold Mines", "2030Q1"),
            member("North America", "Hemlo", "2030Q1"),
        ];
        let labels = [RegisteredShare {
            subject: "Nevada Gold Mines".to_string(),
            num: 1,
            den: 2,
            evidence: "E1".to_string(),
            label: "Nevada Gold Mines LLC".to_string(),
        }];
        let llc = ("Nevada Gold Mines LLC (50%)", ["100", "50", "45"]);
        let plain = ("Nevada Gold Mines (50%)", ["100", "50", "45"]);
        let hemlo = ("Hemlo (100%)", ["50", "50", "45"]);
        let na = ("North America", ["150", "100", "90"]);
        let sum = |rows: Vec<Row<'_>>, labels: &[RegisteredShare]| {
            let rules = LedgerRules {
                regions: &regions,
                region_members: &members,
                labels,
                ..NONE
            };
            let i = region_b(&ledger(&[(1, rows, vec![])], &rules)).clone();
            (i.state, i.cells)
        };
        assert_eq!(
            sum(vec![llc, hemlo, na], &labels),
            (
                State::Consistent,
                locs(&[Some("B5"), Some("B6"), Some("B7")])
            )
        );
        assert_eq!(
            sum(vec![llc, hemlo, na], &[]),
            (
                refused(Reason::OperandRowMissing, None),
                locs(&[None, Some("B6"), Some("B7")])
            )
        );
        // Printed under both names: refused at the row printed first, whichever name it carries.
        for rows in [vec![llc, plain, hemlo, na], vec![plain, llc, hemlo, na]] {
            assert_eq!(
                sum(rows, &labels),
                (
                    refused(Reason::OperandUnreadable, Some("2030Q1:QTD!B5")),
                    locs(&[Some("B5"), Some("B7"), Some("B8")])
                )
            );
        }
    }

    #[test]
    fn test_region_outside_the_map_and_missing_operand_rows_are_refused() {
        let regions = [
            "North America".to_string(),
            "Africa".to_string(),
            "Latin America".to_string(),
        ];
        let members = [
            member("North America", "Carlin", "2030Q1"),
            member("North America", "Cortez", "2030Q1"),
            member("Africa", "Kibali", "2031Q1"),
        ];
        let with_latin = [
            members.as_slice(),
            &[member("Latin America", "Pueblo Viejo", "2030Q1")],
        ]
        .concat();
        let rows = || {
            vec![
                ("Carlin (50%)", ["100", "50", "45"]),
                ("North America", ["100", "50", "45"]),
                ("Africa", ["10", "5", "4"]),
                ("Total Gold", ["100", "50", "45"]),
            ]
        };
        let with = |members: &[RegionMemberRow]| {
            let rules = LedgerRules {
                regions: &regions,
                region_members: members,
                ..NONE
            };
            ledger(&[(1, rows(), vec![])], &rules)
        };
        let l = with(&members);
        // Africa's row is printed, but its members join the map in 2031.
        assert_eq!(
            sum_state(&l, Family::SitesToRegion, "B7"),
            refused(Reason::RegionNotInMap, None)
        );
        // Cortez prints no row.
        let na = at(&l, Family::SitesToRegion, "2030Q1:QTD!B6");
        assert_eq!(na.state, refused(Reason::OperandRowMissing, None));
        assert_eq!(na.cells, locs(&[Some("B5"), None, Some("B6")]));
        // The total adds the map's regions, so North America alone and not the printed Africa.
        let total = at(&l, Family::RegionToTotal, "2030Q1:QTD!B8");
        assert_eq!(total.state, State::Consistent);
        assert_eq!(total.cells, locs(&[Some("B6"), Some("B8")]));
        // A region of the map with no row refuses the total.
        let l = with(&with_latin);
        let total = at(&l, Family::RegionToTotal, "2030Q1:QTD!B8");
        assert_eq!(total.state, refused(Reason::OperandRowMissing, None));
        assert_eq!(total.cells, locs(&[Some("B6"), None, Some("B8")]));
    }

    #[test]
    fn test_withheld_member_excludes_its_region_for_the_periods_it_covers() {
        let regions = ["Africa".to_string()];
        let members = [
            member("Africa", "Loulo-Gounkoto", "2030Q1"),
            member("Africa", "Kibali", "2030Q1"),
        ];
        let statuses = [SubjectStatus {
            subject: "Loulo-Gounkoto".to_string(),
            kind: StatusKind::Withheld,
            first: (2030, 1),
            last: Some((2030, 3)),
            evidence: "E818".to_string(),
        }];
        let quarter = || {
            vec![
                ("Loulo-Gounkoto (80%)", ["100", "80", "70"]),
                ("Kibali (45%)", ["100", "45", "40"]),
                ("Africa", ["200", "125", "110"]),
            ]
        };
        let h1 = vec![
            ("Loulo-Gounkoto (80%)", ["200", "160", "140"]),
            ("Kibali (45%)", ["200", "90", "80"]),
            ("Africa", ["400", "250", "220"]),
        ];
        let releases = vec![(1, quarter(), vec![]), (2, quarter(), h1)];
        let sums = |statuses: &[SubjectStatus]| {
            let rules = LedgerRules {
                regions: &regions,
                region_members: &members,
                statuses,
                ..NONE
            };
            let l = ledger(&releases, &rules);
            ["2030Q1:QTD!B7", "2030Q2:QTD!B7", "2030Q2:YTD!B7"]
                .map(|a| at(&l, Family::SitesToRegion, a).state.clone())
        };
        let withheld = refused(
            Reason::Withheld {
                evidence: "E818".to_string(),
            },
            None,
        );
        assert_eq!(
            sums(&statuses),
            [withheld, State::Consistent, State::Consistent]
        );
        assert_eq!(sums(&[]), [(); 3].map(|()| State::Consistent));
    }

    #[test]
    fn test_total_copper_sums_the_copper_sites_in_row_order() {
        let total = |d: &str| {
            let (l, refusals) = site_ledger(
                COPPER,
                &[
                    ("Zaldivar (50%)", ["50", "25", "24"]),
                    ("Lumwana (100%)", ["30", "30", "28"]),
                    ("Jabal Sayid (50%)", ["10", "5", "5"]),
                    ("Total Copper", ["90", "60", d]),
                ],
                vec![],
                &NONE,
            );
            assert!(refusals.is_empty(), "{refusals:?}");
            // 90 = 50 + 30 + 10 and 60 = 25 + 30 + 5; no copper row is a region.
            assert_eq!(
                [
                    sum_state(&l, Family::RegionToTotal, "B8"),
                    sum_state(&l, Family::RegionToTotal, "C8")
                ],
                [State::Consistent, State::Consistent]
            );
            assert!(
                l.instances
                    .iter()
                    .all(|i| i.family != Family::SitesToRegion)
            );
            at(&l, Family::RegionToTotal, "2030Q1:QTD!D8").clone()
        };
        let i = total("57");
        assert_eq!(
            (i.subject.as_str(), i.field.as_str()),
            ("Total Copper", "cu_sold_attr_kt")
        );
        assert_eq!(
            i.cells,
            locs(&[Some("D5"), Some("D6"), Some("D7"), Some("D8")])
        );
        // Four operands shown to the unit: 24 + 28 + 5 = 57 meets the printed total within 2, the bound included.
        for (printed, code) in [("54", "iii"), ("55", "i"), ("59", "i"), ("60", "iii")] {
            assert_eq!(total(printed).state.code(), code, "Total Copper {printed}");
        }
    }

    /// [`sum_holds`] against the kernel posing Σ lines = total with every box rounded once, on pseudo-random
    /// values of mixed steps around the slack bound.
    #[test]
    fn test_closed_form_sum_matches_the_kernel() {
        let period = Period {
            year: 2030,
            kind: PeriodKind::Quarter(1),
        };
        let steps = [
            Rat::from_integer(1),
            Rat::new(1, 10).expect("step"),
            Rat::new(1, 100).expect("step"),
        ];
        let mut seed = 0x5eed_u64;
        let mut next = |n: u64| {
            seed = seed
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            (seed >> 33) % n
        };
        let slot = |locator: String, units: i128, step: Rat| Slot {
            locator,
            operand: Operand::Number {
                value: Rat::from_integer(units).checked_mul(step).expect("value"),
                step,
            },
            period,
            printed_share: None,
        };
        let rules = xf::Rules::new(xf::Convention::Closed)
            .with("sites_to_region", xf::Construction::RoundedOnce);
        let mut outcomes = [0_usize; 2];
        for trial in 0..400 {
            let parts: Vec<Slot> = (0..2 + next(4))
                .map(|k| {
                    let step = steps[next(3) as usize];
                    slot(format!("T!B{k}"), i128::from(next(100_000)), step)
                })
                .collect();
            let mut sum = Rat::from_integer(0);
            for p in &parts {
                if let Operand::Number { value, .. } = p.operand {
                    sum = sum.checked_add(value).expect("sum");
                }
            }
            let step = steps[next(3) as usize];
            let q = sum.checked_div(step).expect("units");
            let total = slot(
                "T!Z1".to_string(),
                q.num().div_euclid(q.den()) + i128::from(next(7)) - 3,
                step,
            );
            let refs: Vec<&Slot> = parts.iter().collect();
            let closed = sum_holds(&refs, &total).expect("no overflow");
            let identity = xf::Identity {
                id: format!("sum{trial}"),
                family: "sites_to_region".to_string(),
                closure: xf::Closure::Definitional,
                kind: xf::IdentityKind::Sum {
                    total: total.locator.clone(),
                    lines: parts.iter().map(|p| p.locator.clone()).collect(),
                    signs: vec![1; parts.len()],
                },
            };
            let cells: Vec<xf::Cell> = parts.iter().chain([&total]).filter_map(xf_cell).collect();
            let solved = xf::solve(&cells, &[identity], &rules).expect("solves");
            let [c] = solved.as_slice() else {
                panic!("trial {trial}: one component, got {}", solved.len());
            };
            let kernel = match &c.outcome {
                xf::Outcome::Feasible { .. } => true,
                xf::Outcome::Infeasible { .. } => false,
                o => panic!("trial {trial}: {o:?}"),
            };
            assert_eq!(closed, kernel, "trial {trial}");
            outcomes[usize::from(closed)] += 1;
        }
        assert!(outcomes.iter().all(|&n| n >= 50), "{outcomes:?}");
    }
}
