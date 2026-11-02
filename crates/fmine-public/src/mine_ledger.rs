//! The Mine Statistics identity ledger (BD-331.S4, bd-prmn.4): six of the plan's Mine Stats families, anchored on
//! the `QTD`/`YTD` summary sheets (family 2 on the `Minestats*` site sheets too) and decided in exact arithmetic
//! (the kernel `fmine_numerics::exact_feas`, or an exact closed form for a lone sum or weighted mean).
//!
//! - **Ownership.** Attributable production = s × 100%-basis production, per site row and column group (the
//!   release's own year, or the prior-year comparative). s is the share the row's label prints. A registered
//!   printed-share correction replaces it, and the printed share is kept beside (W1-DR-01: Porgera's stale
//!   "(47.5%)"). Region and metal-total rows mix shares and pose no ownership identity; any other row with no
//!   printed share is refused (`share_unprinted`), never read as 100%.
//! - **Quarter sums.** H1 = Q1 + Q2, 9M = H1 + Q3 and FY = 9M + Q4 on the three flow columns (100% production,
//!   attributable production, attributable sold), anchored on each `YTD` cell. The operands come from releases
//!   of the anchor's year, in the anchor's column group, on the row of the same subject.
//! - **Site-sheet quarter sums** (family 2 on the `Minestats*` sheets). The same sums on each additive row of
//!   [`SITE_QUARTER_ROWS`] (tonnes mined and processed and their sub-rows, ounces or pounds, revenue, capital
//!   expenditures), anchored on each cumulative block's cell: the previous release's block of the same site,
//!   row and column group (its quarter block Q1 for H1, else its cumulative block) plus the anchor release's
//!   quarter block. A site is found by name on whichever sheet of its metal it prints in each release; one with
//!   columns on two sheets of its metal in an operand's release is refused, and so is a row printed twice in one
//!   block. Rates, grades, recovery, strip ratio and unit costs are not additive and pose none.
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
//!   In both, and in the site-sheet quarter sums, the nil token is exactly 0 (`nil_token`); any other placeholder
//!   beside a number is refused. A sum is decided alone by |Σ parts − total| ≤ Σ half-steps over every operand,
//!   which is the kernel's answer for one sum with every box rounded once (a test checks the two agree). These
//!   sums are never posed jointly: a region sum links every site of a block, and a site's chain of quarters
//!   joined to the summary's would grow the other families' components past the exhaustive correction search.
//!   Not yet posed: the Nevada Gold Mines LLC sub-sum (Carlin, Cortez, Turquoise Ridge, Phoenix, Long Canyon),
//!   which needs a registry table of its own.
//! - **Sold-weighted rate** (family 5). A region's cost of sales, total cash costs and AISC per ounce = its
//!   members' rates weighted by their attributable gold sold, R·Σwᵢ = Σ rᵢ·wᵢ, members found as in family 3 and
//!   taken in row order. A member whose sold cell is the nil token sold nothing and leaves the identity; any other
//!   placeholder still in it refuses (a rate is never read as 0), and so do no member left (`weights_all_nil`) and
//!   a weight printed at or below 0 (`weight_not_positive`). The rest is decided alone and exactly: the extremes of
//!   the members' weighted means sit at vertices of the weight box, and a feasible answer carries a witness checked
//!   exactly. A conflict is (iv), naming the region's and members' costs in dollars, not (iii): the issuer may
//!   weight by quantities it does not publish. Never posed jointly. The AISC tables' regional check
//!   (`aisc::regional_state`) runs the same decision where the kernel's McCormick relaxation does not refute.
//!
//! Exclusions by name come first: a subject whose registered status is `withheld` for the whole period of an
//! operand (Loulo-Gounkoto Q1–Q3 2025, E818; for a region sum, a member's), and a region whose registered
//! members differ between the operands' releases (the Q4 2025 map change). Then, in order: an operand row the
//! release does not print, a blank operand, all operands placeholders (not published), some placeholders. An
//! instance whose operands are all numbers is decided alone: each value as displayed stands for the closed box
//! [v − step/2, v + step/2] (rounded once, the step from the displayed decimals), the identity holds exactly
//! between true values, and a feasible run is (i), an infeasible one (iii).
//!
//! Every decided instance but the region, total and site-sheet sums and the sold-weighted rates is then posed
//! jointly. Instances that share a cell
//! form a component: a 100% cell is in its ownership identity and in quarter sums, and a cumulative is the total
//! of one sum and an operand of the next. For each infeasible component the kernel's blame gives every minimum
//! correction set, with the range each of its cells can take, never a corrected value. A component can be
//! infeasible though each of its identities is consistent alone (`joint_only`); its instances stay (i).
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

/// Family 5's rates: the gold unit-cost columns of a summary sheet, each weighted by attributable gold sold.
pub const RATE_FIELDS: [&str; 3] = ["cos_usd_per_oz", "tcc_usd_per_oz", "aisc_usd_per_oz"];

/// The additive `Minestats*` rows the site-sheet quarter sums pose, as `(metal, section, label, field id)`:
/// tonnes, ounces and pounds, revenue and capital expenditures. Rates, grades, recovery, strip ratio and unit
/// costs are not additive and pose none. Section and label match the reader's case-insensitively.
pub const SITE_QUARTER_ROWS: [(Metal, Option<&str>, &str, &str); 32] = {
    const TM: Option<&str> = Some("Tonnes mined (thousands)");
    const TP: Option<&str> = Some("Tonnes processed (thousands)");
    const PR: Option<&str> = Some("Production (thousands of ounces)");
    const CX: Option<&str> = Some("Capital expenditures ($ millions)");
    use Metal::{Copper as C, Gold as G};
    [
        (G, None, "Tonnes mined (thousands)", "au_tonnes_mined_kt"),
        (G, TM, "Open Pit Ore", "au_tonnes_mined_open_pit_ore_kt"),
        (G, TM, "Open Pit Waste", "au_tonnes_mined_open_pit_waste_kt"),
        (G, TM, "Underground", "au_tonnes_mined_underground_kt"),
        (
            G,
            None,
            "Tonnes processed (thousands)",
            "au_tonnes_processed_kt",
        ),
        (G, TP, "Oxide Mill", "au_tonnes_processed_oxide_mill_kt"),
        (G, TP, "Roasters", "au_tonnes_processed_roasters_kt"),
        (G, TP, "Autoclave", "au_tonnes_processed_autoclave_kt"),
        (G, TP, "Heap Leach", "au_tonnes_processed_heap_leach_kt"),
        (
            G,
            None,
            "Production (thousands of ounces)",
            "au_production_koz",
        ),
        (G, PR, "Oxide Mill", "au_production_oxide_mill_koz"),
        (G, PR, "Roasters", "au_production_roasters_koz"),
        (G, PR, "Autoclave", "au_production_autoclave_koz"),
        (G, PR, "Heap Leach", "au_production_heap_leach_koz"),
        (G, None, "Sales (thousands of ounces)", "au_sales_koz"),
        (G, None, "Revenue ($ millions)", "au_revenue_usd_m"),
        (
            G,
            CX,
            "Minesite sustaining",
            "au_capex_minesite_sustaining_usd_m",
        ),
        (G, CX, "Projects", "au_capex_projects_usd_m"),
        (
            G,
            None,
            "Total capital expenditures ($ millions)",
            "au_capex_total_usd_m",
        ),
        (C, None, "Tonnes mined (thousands)", "cu_tonnes_mined_kt"),
        (C, TM, "Open Pit Ore", "cu_tonnes_mined_open_pit_ore_kt"),
        (C, TM, "Open Pit Waste", "cu_tonnes_mined_open_pit_waste_kt"),
        (C, TM, "Underground", "cu_tonnes_mined_underground_kt"),
        (
            C,
            None,
            "Tonnes processed (thousands)",
            "cu_tonnes_processed_kt",
        ),
        (
            C,
            None,
            "Production (millions of pounds)",
            "cu_production_mlb",
        ),
        (
            C,
            None,
            "Production (Thousands of tonnes)",
            "cu_production_kt",
        ),
        (C, None, "Sales (millions of pounds)", "cu_sales_mlb"),
        (C, None, "Sales (Thousands of tonnes)", "cu_sales_kt"),
        (C, None, "Revenue ($ millions)", "cu_revenue_usd_m"),
        (
            C,
            CX,
            "Minesite sustaining",
            "cu_capex_minesite_sustaining_usd_m",
        ),
        (C, CX, "Projects", "cu_capex_projects_usd_m"),
        (
            C,
            None,
            "Total capital expenditures ($ millions)",
            "cu_capex_total_usd_m",
        ),
    ]
};

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
    /// Family 2 on the `Minestats*` site sheets: a cumulative block's value = the previous release's block + the
    /// quarter, on the [`SITE_QUARTER_ROWS`].
    SiteQuarterSum,
    SiteSheet,
    /// Family 3: a region = Σ its registered members in the release's map.
    SitesToRegion,
    /// Family 5: a region's gold unit cost = its members' unit costs weighted by attributable gold sold.
    SoldWeightedRate,
}

impl Family {
    pub const ALL: [Self; 7] = [
        Self::Ownership,
        Self::QuarterSum,
        Self::RegionToTotal,
        Self::SiteQuarterSum,
        Self::SiteSheet,
        Self::SitesToRegion,
        Self::SoldWeightedRate,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Ownership => "ownership",
            Self::QuarterSum => QUARTER_SUM,
            Self::RegionToTotal => "region_to_total",
            Self::SiteQuarterSum => "site_quarter_sum",
            Self::SiteSheet => "site_sheet",
            Self::SitesToRegion => "sites_to_region",
            Self::SoldWeightedRate => "sold_weighted_rate",
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
    /// A weighted rate whose every member sold nothing (each sold cell the nil token).
    WeightsAllNil,
    /// A weight of a weighted rate printed at or below 0: a weighted mean is bounded only for positive weights.
    WeightNotPositive,
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
            Self::WeightsAllNil => "weights_all_nil",
            Self::WeightNotPositive => "weight_not_positive",
            Self::KernelIndeterminate(_) => "kernel_indeterminate",
        }
    }
}

/// The fact that would decide an (iv) instance.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Need {
    /// The region's and its members' costs in dollars, which Mine Stats does not print.
    CostTotals,
}

impl Need {
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::CostTotals => "cost_totals_unpublished",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum State {
    /// (i): true values within the printed boxes satisfy the identity.
    Consistent,
    /// (iii): no true values within the printed boxes satisfy it.
    Discrepancy,
    /// (iv): no true values within the printed boxes satisfy it, but the issuer may use quantities it does not
    /// publish; the named fact would decide it.
    Unresolved(Need),
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
            Self::Unresolved(_) => "iv",
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

/// A reader value as the ledger takes it, `(value, step)` as displayed, if it is a number.
#[must_use]
pub fn displayed_number(v: &MineStatsValue, workbook: &Workbook) -> Option<(Rat, Rat)> {
    match operand(v, workbook) {
        Operand::Number { value, step } => Some((value, step)),
        _ => None,
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

/// `Σ xᵢ·wᵢ / Σ wᵢ`; `None` on overflow or a zero total weight.
fn mean(xs: &[Rat], ws: &[Rat]) -> Option<Rat> {
    let (mut num, mut den) = (Rat::from_integer(0), Rat::from_integer(0));
    for (x, w) in xs.iter().zip(ws) {
        num = num.checked_add(x.checked_mul(*w).ok()?).ok()?;
        den = den.checked_add(*w).ok()?;
    }
    num.checked_div(den).ok()
}

fn at_most(x: Rat, y: Rat) -> Option<bool> {
    Some(x.checked_cmp(y).ok()? != std::cmp::Ordering::Greater)
}

/// A closed box `[lo, hi]`.
pub(crate) type Interval = (Rat, Rat);

/// Whether `R·Σwᵢ = Σ rᵢ·wᵢ` holds for true values in the closed boxes `region` = [e, f] (R) and `parts` =
/// ([aᵢ, bᵢ] for rᵢ, [cᵢ, dᵢ] for wᵢ), every cᵢ > 0. `None` on overflow, past 16 parts, or a witness that fails
/// its exact check (never a claim).
///
/// With W = Σwᵢ > 0 the identity reads R = Σ rᵢwᵢ/W. For fixed weights the rates' weighted mean takes every value
/// in [L(w), U(w)], L(w) = Σ aᵢwᵢ/W and U(w) = Σ bᵢwᵢ/W, so it holds at w iff that range meets [e, f]. It holds
/// somewhere iff min L ≤ f and max U ≥ e: if no point has both, take one with L ≤ f (so U < e) and one with
/// U ≥ e; along the segment between them U crosses e, where L ≤ U = e ≤ f. L and U are linear-fractional with a
/// positive denominator, so their extremes over the weight box are at its vertices. A feasible answer is returned
/// with a witness: weights at a vertex or at that crossing, R = max(e, L(w)), rates on the segment from (aᵢ) to
/// (bᵢ), each value checked against its box and the identity checked exactly. The AISC tables' regional rule uses
/// it too (`aisc::regional_state`).
pub(crate) fn weighted_mean_holds(
    region: Interval,
    parts: &[(Interval, Interval)],
) -> Option<bool> {
    let n = parts.len();
    if n == 0 || n > 16 {
        return None;
    }
    let (e, f) = region;
    let a: Vec<Rat> = parts.iter().map(|((lo, _), _)| *lo).collect();
    let b: Vec<Rat> = parts.iter().map(|((_, hi), _)| *hi).collect();
    let vertex = |mask: u32| -> Vec<Rat> {
        parts
            .iter()
            .enumerate()
            .map(|(k, (_, (lo, hi)))| if (mask >> k) & 1 == 1 { *hi } else { *lo })
            .collect()
    };
    let (mut v_lo, mut v_hi) = (vertex(0), vertex(0));
    let (mut min_l, mut max_u) = (mean(&a, &v_lo)?, mean(&b, &v_hi)?);
    for mask in 1..(1_u32 << n) {
        let w = vertex(mask);
        let l = mean(&a, &w)?;
        if !at_most(min_l, l)? {
            (min_l, v_lo) = (l, w.clone());
        }
        let u = mean(&b, &w)?;
        if !at_most(u, max_u)? {
            (max_u, v_hi) = (u, w);
        }
    }
    if !(at_most(min_l, f)? && at_most(e, max_u)?) {
        return Some(false);
    }
    let w = if at_most(e, mean(&b, &v_lo)?)? {
        v_lo
    } else if at_most(mean(&a, &v_hi)?, f)? {
        v_hi
    } else {
        // g(w) = Σ (bᵢ − e)·wᵢ is negative at v_lo (U < e) and not negative at v_hi (U ≥ e), and affine along the
        // segment: its root t = g₀ / (g₀ − g₁).
        let g = |w: &[Rat]| {
            b.iter()
                .zip(w)
                .try_fold(Rat::from_integer(0), |acc, (bi, wi)| {
                    acc.checked_add(bi.checked_sub(e).ok()?.checked_mul(*wi).ok()?)
                        .ok()
                })
        };
        let (g0, g1) = (g(&v_lo)?, g(&v_hi)?);
        let t = g0.checked_div(g0.checked_sub(g1).ok()?).ok()?;
        v_lo.iter()
            .zip(&v_hi)
            .map(|(x, y)| {
                x.checked_add(t.checked_mul(y.checked_sub(*x).ok()?).ok()?)
                    .ok()
            })
            .collect::<Option<Vec<Rat>>>()?
    };
    let l = mean(&a, &w)?;
    let r = if at_most(l, e)? { e } else { l };
    let total = w
        .iter()
        .try_fold(Rat::from_integer(0), |acc, x| acc.checked_add(*x).ok())?;
    let dot = |x: &[Rat]| {
        x.iter()
            .zip(&w)
            .try_fold(Rat::from_integer(0), |acc, (xi, wi)| {
                acc.checked_add(xi.checked_mul(*wi).ok()?).ok()
            })
    };
    let (base, top) = (dot(&a)?, dot(&b)?);
    let spread = top.checked_sub(base).ok()?;
    let lambda = if spread.signum() == 0 {
        Rat::from_integer(0)
    } else {
        r.checked_mul(total)
            .ok()?
            .checked_sub(base)
            .ok()?
            .checked_div(spread)
            .ok()?
    };
    let rates = a
        .iter()
        .zip(&b)
        .map(|(lo, hi)| {
            lo.checked_add(lambda.checked_mul(hi.checked_sub(*lo).ok()?).ok()?)
                .ok()
        })
        .collect::<Option<Vec<Rat>>>()?;
    let within = |x: Rat, (lo, hi): Interval| Some(at_most(lo, x)? && at_most(x, hi)?);
    let mut checked = within(r, region)? && r.checked_mul(total).ok()? == dot(&rates)?;
    for ((rate_box, weight_box), (rk, wk)) in parts.iter().zip(rates.iter().zip(&w)) {
        checked &= within(*rk, *rate_box)? && within(*wk, *weight_box)?;
    }
    checked.then_some(true)
}

/// A number operand's closed box `[v − step/2, v + step/2]`.
fn closed_box(slot: &Slot) -> Option<Interval> {
    let Operand::Number { value, step } = slot.operand else {
        return None;
    };
    let half = step.checked_div(Rat::from_integer(2)).ok()?;
    Some((value.checked_sub(half).ok()?, value.checked_add(half).ok()?))
}

/// Steps 1–7 of family 5 for members found in row order, `(rate, sold)` each, and the region's rate: the state,
/// and whether a member left the identity by its nil sold cell.
fn decide_weighted(members: &[(&Slot, &Slot)], region: &Slot) -> (State, bool) {
    let refused = |reason, at: Option<&Slot>| State::Refused {
        reason,
        at: at.map(|s| s.locator.clone()),
    };
    let ops: Vec<&Slot> = members
        .iter()
        .flat_map(|(r, w)| [*r, *w])
        .chain(std::iter::once(region))
        .collect();
    if let Some(b) = ops.iter().find(|o| o.operand == Operand::Blank) {
        return (refused(Reason::BlankOperand, Some(b)), false);
    }
    if ops
        .iter()
        .all(|o| matches!(o.operand, Operand::Placeholder(_)))
    {
        return (State::NotPublished, false);
    }
    // A nil sold cell sold nothing: weight exactly 0, so the member leaves the identity whatever its rate prints.
    let kept: Vec<(&Slot, &Slot)> = members
        .iter()
        .copied()
        .filter(|(_, w)| w.operand != Operand::Placeholder(Mark::Nil))
        .collect();
    let live: Vec<&Slot> = kept
        .iter()
        .flat_map(|(r, w)| [*r, *w])
        .chain(std::iter::once(region))
        .collect();
    // A rate is never read as 0: any placeholder still in the identity refuses.
    if let Some(p) = live
        .iter()
        .find(|o| matches!(o.operand, Operand::Placeholder(_)))
    {
        return (refused(Reason::PlaceholderOperand, Some(p)), false);
    }
    if let Some(u) = live.iter().find(|o| o.operand == Operand::Unreadable) {
        return (refused(Reason::OperandUnreadable, Some(u)), false);
    }
    if kept.is_empty() {
        return (refused(Reason::WeightsAllNil, None), false);
    }
    if let Some((_, w)) = kept
        .iter()
        .find(|(_, w)| matches!(w.operand, Operand::Number { value, .. } if value.signum() <= 0))
    {
        return (refused(Reason::WeightNotPositive, Some(w)), false);
    }
    let parts: Option<Vec<(Interval, Interval)>> = kept
        .iter()
        .map(|(r, w)| Some((closed_box(r)?, closed_box(w)?)))
        .collect();
    let left = kept.len() < members.len();
    match parts
        .zip(closed_box(region))
        .and_then(|(parts, region)| weighted_mean_holds(region, &parts))
    {
        Some(true) => (State::Consistent, left),
        Some(false) => (State::Unresolved(Need::CostTotals), left),
        None => (
            kernel_refused("the exact weighted mean overflowed or its witness failed".to_string()),
            false,
        ),
    }
}

/// Family 5 on each summary sheet, column group and gold unit-cost field: a region's rate is its members' rates
/// weighted by their attributable gold sold, `R·Σwᵢ = Σ rᵢ·wᵢ`, the members found as the sum families find them
/// and taken in row order. Decided alone and exactly ([`weighted_mean_holds`]); a conflict is (iv), since the
/// issuer may weight by quantities it does not publish, and the instance is never posed jointly.
fn sold_weighted(
    rules: &LedgerRules<'_>,
    slots: &BTreeMap<SlotKey, Slot>,
    out: &mut Vec<(Instance, Option<Posed>)>,
) {
    let sold_field = FLOW_FIELDS[0][2];
    let mut blocks: BTreeMap<(&str, &str, &str, Group), BTreeMap<&str, &Slot>> = BTreeMap::new();
    for ((release, sheet, field, group, subject), slot) in slots {
        if RATE_FIELDS.contains(&field.as_str()) || field == sold_field {
            blocks
                .entry((release, sheet, field, *group))
                .or_default()
                .insert(subject, slot);
        }
    }
    for (&(release, sheet, field, group), rates) in &blocks {
        if !RATE_FIELDS.contains(&field) {
            continue;
        }
        let Some(sold) = blocks.get(&(release, sheet, sold_field, group)) else {
            continue;
        };
        for (&region, &total) in rates
            .iter()
            .filter(|(s, _)| rules.regions.iter().any(|r| r == **s))
        {
            let members = rules.members_in_order(region, release);
            let mut found: Vec<(&Slot, &Slot, bool)> = Vec::new();
            let mut missing = false;
            for m in &members {
                match (find_member(rates, rules, m), find_member(sold, rules, m)) {
                    (Found::One(r), Found::One(w)) => found.push((r, w, false)),
                    (Found::Both(r), Found::Both(w)) => found.push((r, w, true)),
                    _ => missing = true,
                }
            }
            found.sort_by_key(|(r, _, _)| row_of(&r.locator));
            let mut cells: Vec<Option<String>> = found
                .iter()
                .flat_map(|(r, w, _)| [Some(r.locator.clone()), Some(w.locator.clone())])
                .collect();
            cells.push(Some(total.locator.clone()));
            let refused = |reason, at: Option<&Slot>| State::Refused {
                reason,
                at: at.map(|s| s.locator.clone()),
            };
            let withheld = members
                .iter()
                .find_map(|m| rules.withheld(m, &total.period));
            let mut nil_token = false;
            let state = if let Some(evidence) = withheld {
                refused(Reason::Withheld { evidence }, None)
            } else if members.is_empty() {
                refused(Reason::RegionNotInMap, None)
            } else if missing {
                refused(Reason::OperandRowMissing, None)
            } else if let Some((r, _, _)) = found.iter().find(|(_, _, both)| *both) {
                refused(Reason::OperandUnreadable, Some(r))
            } else {
                let pairs: Vec<(&Slot, &Slot)> = found.iter().map(|(r, w, _)| (*r, *w)).collect();
                let (state, left) = decide_weighted(&pairs, total);
                nil_token = left;
                state
            };
            out.push((
                Instance {
                    family: Family::SoldWeightedRate,
                    subject: region.to_string(),
                    field: field.to_string(),
                    group,
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

/// `(release, metal, field id, period, site)`: a [`SITE_QUARTER_ROWS`] cell on a `Minestats*` sheet.
type SiteRowKey = (String, Metal, &'static str, Period, String);

/// Every [`SITE_QUARTER_ROWS`] cell of the `Minestats*` sheets, by key: each with its row, in row order, and
/// whether the reader found its `(section, label)` printed again in its block. A row's label is read under its
/// registered alias for the release, and its section is the reader's.
fn site_quarter_cells(
    files: &[LedgerFile<'_>],
    aliases: &[MineStatsLabelAlias],
) -> BTreeMap<SiteRowKey, Vec<(u32, bool, Slot)>> {
    let norm = |s: &str| {
        s.split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
            .to_lowercase()
    };
    let mut out: BTreeMap<SiteRowKey, Vec<(u32, bool, Slot)>> = BTreeMap::new();
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
            let label = site_label(aliases, f.quarter, &v.label);
            let section = v.section.as_deref().map(norm);
            let Some(&(_, _, _, field)) = SITE_QUARTER_ROWS
                .iter()
                .find(|(m, s, l, _)| *m == metal && s.map(norm) == section && norm(l) == label)
            else {
                continue;
            };
            let slot = Slot {
                locator: v.locator.clone(),
                operand: operand(v, f.workbook),
                period,
                printed_share: None,
            };
            let twice = repeated.contains(format!("{}:{}!A{row}", f.quarter, v.sheet).as_str());
            out.entry((f.quarter.to_string(), metal, field, period, v.site.clone()))
                .or_default()
                .push((row, twice, slot));
        }
    }
    for cells in out.values_mut() {
        cells.sort_by_key(|(row, _, _)| *row);
    }
    out
}

/// Family 2 on the `Minestats*` site sheets: for each cumulative block's [`SITE_QUARTER_ROWS`] cell (H1 in a Q2
/// release, 9M in Q3, FY in Q4), the previous release's block of the same site, field and year (its quarter block
/// Q1 for H1, else its cumulative block) plus this release's quarter block. A site is found on any sheet of the
/// metal, so a site that moves between sheets is still matched. Each instance is decided alone, the nil token
/// exactly 0, and never posed jointly: a site's chain of quarters joined to the summary's would grow the other
/// families' components.
fn site_quarter_sums(
    rules: &LedgerRules<'_>,
    files: &[LedgerFile<'_>],
    sites: &SiteSheets,
    releases: &BTreeSet<&str>,
    out: &mut Vec<(Instance, Option<Posed>)>,
) {
    let cells = site_quarter_cells(files, rules.aliases);
    let two_sheets = |release: &str, metal: Metal, site: &str| {
        sites
            .sheets
            .get(&(release.to_string(), metal, site.to_string()))
            .is_some_and(|s| s.len() > 1)
    };
    for ((release, metal, field, period, site), totals) in &cells {
        let Some((year, n @ 2..=4)) = release_parts(release) else {
            continue;
        };
        let cumulative = match n {
            2 => PeriodKind::H1,
            3 => PeriodKind::NineMonths,
            _ => PeriodKind::FullYear,
        };
        let group = if period.year == year {
            Group::Current
        } else if period.year + 1 == year {
            Group::Prior
        } else {
            continue;
        };
        let Some((_, _, total)) = totals.first().filter(|_| period.kind == cumulative) else {
            continue;
        };
        let prev_release = format!("{year}Q{}", n - 1);
        let at = |kind| Period {
            year: period.year,
            kind,
        };
        let prev_period = at(match n {
            2 => PeriodKind::Quarter(1),
            3 => PeriodKind::H1,
            _ => PeriodKind::NineMonths,
        });
        let quarter_period = at(PeriodKind::Quarter(n));
        let moved = two_sheets(&prev_release, *metal, site) || two_sheets(release, *metal, site);
        let find = |r: &str, p: Period| {
            (!two_sheets(r, *metal, site))
                .then(|| cells.get(&(r.to_string(), *metal, *field, p, site.clone())))
                .flatten()
        };
        let prev = find(&prev_release, prev_period);
        let quarter = find(release, quarter_period);
        fn first(c: Option<&Vec<(u32, bool, Slot)>>) -> Option<&Slot> {
            c.and_then(|c| c.first()).map(|(_, _, s)| s)
        }
        let refused = |reason, at: Option<&Slot>| State::Refused {
            reason,
            at: at.map(|s| s.locator.clone()),
        };
        let mut nil_token = false;
        let state = if let Some(evidence) = [prev_period, quarter_period, *period]
            .iter()
            .find_map(|p| rules.withheld(site, p))
        {
            refused(Reason::Withheld { evidence }, None)
        } else if !releases.contains(prev_release.as_str()) {
            refused(Reason::ReleaseMissing, None)
        } else if moved {
            refused(Reason::OperandUnreadable, None)
        } else if let (Some(p), Some(q)) = (prev, quarter) {
            let blocks = [p, q, totals];
            let ops: Vec<&Slot> = blocks.iter().filter_map(|c| first(Some(c))).collect();
            let find_op =
                |pred: &dyn Fn(&Operand) -> bool| ops.iter().copied().find(|s| pred(&s.operand));
            if let Some(dup) = blocks
                .iter()
                .find(|c| c.len() > 1 || c.iter().any(|(_, twice, _)| *twice))
            {
                refused(Reason::OperandUnreadable, first(Some(dup)))
            } else if let Some(b) = find_op(&|o| *o == Operand::Blank) {
                refused(Reason::BlankOperand, Some(b))
            } else if ops.iter().all(|s| {
                matches!(
                    s.operand,
                    Operand::Placeholder(Mark::Nil | Mark::NotApplicable)
                )
            }) {
                State::NotPublished
            } else if let Some(na) = find_op(&|o| *o == Operand::Placeholder(Mark::NotApplicable)) {
                refused(Reason::PlaceholderOperand, Some(na))
            } else if let Some(u) =
                find_op(&|o| matches!(o, Operand::Unreadable | Operand::Placeholder(Mark::Other)))
            {
                refused(Reason::OperandUnreadable, Some(u))
            } else {
                match ops.as_slice() {
                    [a, b, t] => match sum_holds(&[a, b], t) {
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
                    },
                    _ => kernel_refused("an operand block with no cell".to_string()),
                }
            }
        } else {
            refused(Reason::OperandRowMissing, None)
        };
        out.push((
            Instance {
                family: Family::SiteQuarterSum,
                subject: site.clone(),
                field: (*field).to_string(),
                group,
                period: *period,
                anchor: total.locator.clone(),
                cells: vec![
                    first(prev).map(|s| s.locator.clone()),
                    first(quarter).map(|s| s.locator.clone()),
                    Some(total.locator.clone()),
                ],
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
    let sites = site_sheets(files, rules.aliases);
    site_sheet(rules, &slots, &sites, &mut pairs);
    sums(rules, &slots, &mut pairs);
    sold_weighted(rules, &slots, &mut pairs);
    site_quarter_sums(rules, files, &sites, &releases, &mut pairs);
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

include!("../../xtask/testdata/public_mine_ledger_parity_contract.rs");
