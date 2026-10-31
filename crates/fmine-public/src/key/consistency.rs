//! DESIGN (BD-504e.3): compare source assertions across registered releases by semantic key. A changed
//! value is a finding, never a choice of which release is true. No disclosure-note registry is consulted by
//! this slice, so a change never claims that the issuer omitted a note. Hidden sheets never enter its input.

use std::collections::{BTreeMap, BTreeSet};

use fmine_numerics::exact_feas::Rat;

use super::{KeyedCell, PublicValue, Scope, SeriesKey, SeriesOwnershipBasis};
use crate::js;

/// One classification for each appearance of a candidate figure in a release.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum VintageState {
    FirstRegistered,
    SingleVintage,
    Unchanged,
    ChangedWithoutRegisteredExplanation,
    Conflict,
    NotComparable,
}

impl VintageState {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::FirstRegistered => "first_registered",
            Self::SingleVintage => "single_vintage",
            Self::Unchanged => "unchanged",
            Self::ChangedWithoutRegisteredExplanation => "changed_without_registered_explanation",
            Self::Conflict => "conflict",
            Self::NotComparable => "not_comparable",
        }
    }
}

/// A comparison never converts units or ownership interests. `reason` identifies the changed value class
/// or the precise reason a numerical comparison was not made.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Comparison {
    pub state: VintageState,
    pub reason: Option<&'static str>,
    pub token_glyph_changed: bool,
}

impl Comparison {
    fn new(state: VintageState, reason: Option<&'static str>) -> Self {
        Self {
            state,
            reason,
            token_glyph_changed: false,
        }
    }

    fn to_json(self) -> String {
        format!(
            "{{\"state\":{},\"reason\":{},\"token_glyph_changed\":{},\"rounding_only\":null}}",
            js(self.state.as_str()),
            self.reason.map_or_else(|| "null".into(), js),
            self.token_glyph_changed,
        )
    }
}

/// Cited exceptions only. Every candidate is counted, but unchanged inventories are not repeated in the
/// JSON. An ambiguous release keeps *all* assertions rather than selecting the first column or file.
#[derive(Debug, Clone)]
pub struct VintageItem {
    pub first: Vec<KeyedCell>,
    pub previous: Vec<KeyedCell>,
    pub current: Vec<KeyedCell>,
    pub against_first: Comparison,
    pub against_previous: Option<Comparison>,
}

impl VintageItem {
    fn to_json(&self) -> String {
        let claims = |c: &[KeyedCell]| {
            c.iter()
                .map(KeyedCell::to_json)
                .collect::<Vec<_>>()
                .join(",")
        };
        format!(
            "{{\"against_first\":{},\"against_previous\":{},\"first\":[{}],\"previous\":[{}],\"current\":[{}]}}",
            self.against_first.to_json(),
            self.against_previous
                .map_or_else(|| "null".into(), Comparison::to_json),
            claims(&self.first),
            claims(&self.previous),
            claims(&self.current),
        )
    }
}

/// Counts refer to release appearances: a figure in three releases contributes one first-registration
/// classification and two comparisons. A lone appearance contributes `single_vintage` instead.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct VintageCounts {
    pub appearances: usize,
    pub comparisons: usize,
    pub states: BTreeMap<VintageState, usize>,
}

impl VintageCounts {
    fn count(&mut self, comparison: Comparison, later: bool) {
        self.appearances += 1;
        self.comparisons += usize::from(later);
        *self.states.entry(comparison.state).or_default() += 1;
    }

    fn to_json(&self) -> String {
        let states = self
            .states
            .iter()
            .map(|(s, n)| format!("{}:{n}", js(s.as_str())))
            .collect::<Vec<_>>()
            .join(",");
        format!(
            "{{\"appearances\":{},\"comparisons\":{},\"by_state\":{{{states}}}}}",
            self.appearances, self.comparisons
        )
    }
}

/// Deterministic findings on the registered corpus. `series` counts complete semantic keys. Candidate
/// groups additionally relate keys across a basis or region-map change solely to report non-comparability.
#[derive(Debug, Clone, Default)]
pub struct VintageReport {
    pub series: usize,
    pub candidate_groups: usize,
    pub multi_vintage: usize,
    pub totals: VintageCounts,
    pub by_vintage: BTreeMap<String, VintageCounts>,
    pub items: Vec<VintageItem>,
}

impl VintageReport {
    #[must_use]
    pub fn to_json(&self) -> String {
        let vintages = self
            .by_vintage
            .iter()
            .map(|(v, c)| format!("{}:{}", js(v), c.to_json()))
            .collect::<Vec<_>>()
            .join(",");
        let items = self
            .items
            .iter()
            .map(VintageItem::to_json)
            .collect::<Vec<_>>()
            .join(",");
        format!(
            "{{\"rules_version\":\"mine-stats-vintages/1\",\"scope\":\"mine_stats\",\
             \"first_registered_caveat\":\"The earliest registered release need not be the issuer's first publication.\",\
             \"disclosure_check\":\"not_performed\",\
             \"interpretation_limit\":\"A difference does not establish which assertion is true or whether the issuer disclosed a reason.\",\
             \"series\":{},\"candidate_groups\":{},\"multi_vintage\":{},\"totals\":{},\
             \"by_vintage\":{{{vintages}}},\"items\":[{items}]}}",
            self.series,
            self.candidate_groups,
            self.multi_vintage,
            self.totals.to_json(),
        )
    }
}

fn candidate_key(c: &KeyedCell) -> SeriesKey {
    let mut key = c.key.series_key();
    key.ownership_basis = SeriesOwnershipBasis::Unprinted;
    if let Scope::Region { map_version } = &mut key.scope {
        map_version.clear();
    }
    // FACT: Pueblo Viejo's section gains the redundant ($/oz) qualifier when it moves sheets in 2025Q4.
    // The canonical unit must already agree. Preserve both original keys and Claim IDs; this narrowly
    // relates comparison candidates without stripping arbitrary qualifiers or inferring any unit.
    if key.unit == "usd_per_oz"
        && let Some(label) = key.line.label.strip_prefix("site:costs per ounce ($/oz):")
    {
        key.line.label = format!("site:costs per ounce:{label}");
    }
    key
}

fn intrinsic(c: &[&KeyedCell]) -> Option<Comparison> {
    if c.len() != 1 {
        return Some(Comparison::new(
            VintageState::Conflict,
            Some("ambiguous_same_release"),
        ));
    }
    let c = c[0];
    if c.basis_evidence.is_some() {
        return Some(Comparison::new(
            VintageState::Conflict,
            Some("registered_basis_correction"),
        ));
    }
    match &c.claim.value {
        PublicValue::Number {
            displayed: Err(_), ..
        } => Some(Comparison::new(
            VintageState::NotComparable,
            Some("display_refused"),
        )),
        PublicValue::Number {
            displayed: Ok(s),
            stored,
            ..
        } => displayed_number(s, stored, &c.key.unit)
            .err()
            .map(|reason| Comparison::new(VintageState::NotComparable, Some(reason))),
        _ => None,
    }
}

fn displayed_number(s: &str, stored: &str, unit: &str) -> Result<Rat, &'static str> {
    // KeyedCell units describe stored quantities. Undo Excel's percent presentation exactly, retaining
    // the displayed precision: 80% and 0.8 agree, but 80% and 80 do not. No units are converted or inferred.
    if let Some(percent) = s.strip_suffix('%') {
        if !matches!(unit, "mass_fraction" | "percent" | "wt_pct" | "ratio") {
            return Err("display_unit_ambiguous");
        }
        return Rat::from_decimal_str(percent)
            .and_then(|n| n.checked_div(Rat::from_integer(100)))
            .map_err(|_| "display_not_numeric");
    }
    Rat::from_decimal_str(s)
        .ok()
        .or_else(|| {
            // A number format may print a dash for an exact zero. It remains a Number; a nonzero value under
            // a literal-only format never receives this interpretation (and a Nil token is handled separately).
            let zero = Rat::from_integer(0);
            (matches!(s.trim(), "-" | "–" | "—")
                && Rat::from_decimal_str(stored).ok() == Some(zero))
            .then_some(zero)
        })
        .ok_or("display_not_numeric")
}

fn compare(from: &[&KeyedCell], to: &[&KeyedCell]) -> Comparison {
    if let Some(c) = intrinsic(from).or_else(|| intrinsic(to)) {
        return c;
    }
    let (a, b) = (from[0], to[0]);
    if a.key.scope != b.key.scope {
        return Comparison::new(VintageState::NotComparable, Some("region_map"));
    }
    if SeriesOwnershipBasis::from(a.key.ownership_basis)
        != SeriesOwnershipBasis::from(b.key.ownership_basis)
    {
        return Comparison::new(VintageState::NotComparable, Some("ownership_basis"));
    }
    let (equal, numeric, glyph) = match (&a.claim.value, &b.claim.value) {
        (
            PublicValue::Number {
                displayed: Ok(x),
                stored: xs,
                ..
            },
            PublicValue::Number {
                displayed: Ok(y),
                stored: ys,
                ..
            },
        ) => (
            displayed_number(x, xs, &a.key.unit) == displayed_number(y, ys, &b.key.unit),
            true,
            false,
        ),
        (PublicValue::Nil { token: x, .. }, PublicValue::Nil { token: y, .. })
        | (PublicValue::NotApplicable { token: x }, PublicValue::NotApplicable { token: y }) => {
            (true, false, x != y)
        }
        (PublicValue::Missing, PublicValue::Missing) => (true, false, false),
        _ => (false, false, false),
    };
    if equal {
        Comparison {
            state: VintageState::Unchanged,
            reason: None,
            token_glyph_changed: glyph,
        }
    } else {
        Comparison::new(
            VintageState::ChangedWithoutRegisteredExplanation,
            Some(if numeric { "value" } else { "token" }),
        )
    }
}

/// Compares each later appearance with both the earliest registered release and its predecessor. Exact
/// displayed decimals decide numeric equality; raw-token changes remain visible for placeholders. Input
/// order, row position and column position do not choose a source or affect the report.
#[must_use]
pub fn compare_vintages(claims: &[KeyedCell]) -> VintageReport {
    let mut groups: BTreeMap<SeriesKey, BTreeMap<&str, Vec<&KeyedCell>>> = BTreeMap::new();
    let mut series = BTreeSet::new();
    for c in claims {
        series.insert(c.key.series_key());
        groups
            .entry(candidate_key(c))
            .or_default()
            .entry(&c.key.reporting_vintage.release)
            .or_default()
            .push(c);
    }
    let mut report = VintageReport {
        series: series.len(),
        candidate_groups: groups.len(),
        ..VintageReport::default()
    };
    for releases in groups.values_mut() {
        for group in releases.values_mut() {
            group.sort_by(|a, b| {
                (&a.key, &a.locator, a.claim.claim_id)
                    .cmp(&(&b.key, &b.locator, b.claim.claim_id))
                    .then_with(|| a.to_json().cmp(&b.to_json()))
            });
        }
        report.multi_vintage += usize::from(releases.len() > 1);
        let Some(first) = releases.values().next() else {
            continue;
        };
        let mut previous = first;
        for (index, (release, current)) in releases.iter().enumerate() {
            let against_first = if index == 0 {
                intrinsic(current).unwrap_or_else(|| {
                    Comparison::new(
                        if releases.len() == 1 {
                            VintageState::SingleVintage
                        } else {
                            VintageState::FirstRegistered
                        },
                        None,
                    )
                })
            } else {
                compare(first, current)
            };
            let against_previous = (index != 0).then(|| compare(previous, current));
            report.totals.count(against_first, index != 0);
            report
                .by_vintage
                .entry((*release).to_string())
                .or_default()
                .count(against_first, index != 0);
            let exceptional = |c: Comparison| {
                c.token_glyph_changed
                    || matches!(
                        c.state,
                        VintageState::ChangedWithoutRegisteredExplanation
                            | VintageState::Conflict
                            | VintageState::NotComparable
                    )
            };
            if exceptional(against_first) || against_previous.is_some_and(exceptional) {
                let owned = |g: &[&KeyedCell]| g.iter().map(|c| (*c).clone()).collect();
                report.items.push(VintageItem {
                    first: owned(first),
                    previous: if index == 0 {
                        Vec::new()
                    } else {
                        owned(previous)
                    },
                    current: owned(current),
                    against_first,
                    against_previous,
                });
            }
            previous = current;
        }
    }
    report
}

#[cfg(test)]
mod tests {
    use super::*;

    // Public-API comparison checks live in xtask/tests/public_mine_stats_vintages.rs.

    /// FACT: E474, W1-DR-09 and W1's T16 population predate this comparator. W1 lists passes/failures:
    /// 6,391 + 9 = 6,400 comparisons, excluding 30 region-map changes. Our additional display/basis states
    /// remain diagnostics; this test does not claim that GATE-BULL-01 part 3 passed.
    #[test]
    #[ignore = "needs the registered Barrick Mine Stats workbooks under corpus_of_input_docs/"]
    fn real_mine_stats_vintages_keep_e474_and_basis_conflicts() {
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|p| p.parent())
            .expect("workspace")
            .to_path_buf();
        let profile = crate::sources::load_issuer_profile(&root, "barrick").expect("profile");
        let sources = crate::load_registered_sources(&profile, &root).expect("verified sources");
        let vocabulary = profile.mine_stats_labels.iter().cloned().collect();
        let mut claims = Vec::new();
        for source in sources
            .iter()
            .filter(|s| s.kind == crate::sources::SourceKind::MineStats)
        {
            let read = crate::mine_stats::read_mine_stats(
                &source.workbook,
                &source.quarter,
                Some(&vocabulary),
            )
            .expect("read");
            assert!(read.refusals.is_empty(), "{:?}", read.refusals);
            for value in &read.values {
                claims.push(
                    crate::key::from_mine_stats(&profile, &source.workbook, &source.quarter, value)
                        .expect("canonical source-backed Claim"),
                );
            }
        }
        let r = compare_vintages(&claims);
        // FACT: these 20 Pueblo Viejo pairs have the same metric/unit/value in the independent W1 read.
        // Only the section's redundant ($/oz) changes; never rewrite the canonical keys to make them match.
        let by_locator: BTreeMap<_, _> = claims.iter().map(|c| (c.locator.as_str(), c)).collect();
        let section_pairs = [
            (
                "Barrick_Q4_2024_Mine_Stats.xlsx",
                "Barrick_Q4_2025_Mine_Stats.xlsx",
                &[
                    ("B33", "M41"),
                    ("B34", "M42"),
                    ("B35", "M43"),
                    ("B36", "M44"),
                    ("B82", "M102"),
                    ("B83", "M103"),
                    ("B84", "M104"),
                    ("B85", "M105"),
                ][..],
            ),
            (
                "Barrick_Q1_2025_Mine_Stats.xlsx",
                "Barrick_Q1_2026_Mine_Stats.xlsx",
                &[
                    ("B33", "M41"),
                    ("B34", "M42"),
                    ("B35", "M43"),
                    ("B36", "M44"),
                ][..],
            ),
            (
                "Barrick_Q2_2025_Mine_Stats.xlsx",
                "Barrick_Q2_2026_Mine_Stats.xlsx",
                &[
                    ("B33", "M43"),
                    ("B34", "M44"),
                    ("B35", "M45"),
                    ("B36", "M46"),
                    ("B82", "M106"),
                    ("B83", "M107"),
                    ("B84", "M108"),
                    ("B85", "M109"),
                ][..],
            ),
        ];
        for (from_file, to_file, cells) in section_pairs {
            for (from_cell, to_cell) in cells {
                let first = by_locator[format!("{from_file}:Minestats2!{from_cell}").as_str()];
                let later = by_locator[format!("{to_file}:Minestats1!{to_cell}").as_str()];
                assert_eq!(first.key.subject, "Pueblo Viejo");
                assert_ne!(first.key.series_key(), later.key.series_key());
                assert_eq!(candidate_key(first), candidate_key(later));
                assert_eq!(compare(&[first], &[later]).state, VintageState::Unchanged);
            }
        }
        let e474 = r
            .items
            .iter()
            .find(|i| {
                i.current
                    .iter()
                    .any(|c| c.locator == "Barrick_Q1_2025_Mine_Stats.xlsx:QTD!N22")
            })
            .expect("E474 later cell");
        assert!(
            e474.first
                .iter()
                .any(|c| c.locator == "Barrick_Q1_2024_Mine_Stats.xlsx:QTD!G23")
        );
        assert_eq!(
            e474.against_first.state,
            VintageState::ChangedWithoutRegisteredExplanation
        );
        assert_eq!(e474.against_first.reason, Some("value"));
        let changed: BTreeSet<_> = r
            .items
            .iter()
            .filter(|i| i.against_first.state == VintageState::ChangedWithoutRegisteredExplanation)
            .map(|i| i.first[0].locator.as_str())
            .collect();
        // FACT: the original locators of E474 and W1-DR-09's eight changes were captured before this code.
        assert_eq!(
            changed,
            BTreeSet::from([
                "Barrick_Q1_2024_Mine_Stats.xlsx:QTD!G23",
                "Barrick_Q3_2024_Mine_Stats.xlsx:Minestats3!F57",
                "Barrick_Q2_2025_Mine_Stats.xlsx:Minestats1!F107",
                "Barrick_Q2_2025_Mine_Stats.xlsx:Minestats1!F108",
                "Barrick_Q2_2024_Mine_Stats.xlsx:Minestats2!F14",
                "Barrick_Q2_2024_Mine_Stats.xlsx:Minestats2!F27",
                "Barrick_Q2_2024_Mine_Stats.xlsx:Minestats2!F63",
                "Barrick_Q2_2024_Mine_Stats.xlsx:Minestats2!F76",
                "Barrick_Q2_2025_Mine_Stats.xlsx:Minestats3!B8",
            ])
        );
        let mut region_changes = BTreeMap::<&str, usize>::new();
        for item in r
            .items
            .iter()
            .filter(|i| !i.previous.is_empty() && i.against_first.reason == Some("region_map"))
        {
            *region_changes
                .entry(&item.current[0].key.reporting_vintage.release)
                .or_default() += 1;
        }
        // W1 §5.1 lists passes/failures, not total/failures; §13 explicitly states 6,400 cells.
        for (release, passed, changed, regions) in [
            ("2025Q1", 645, 1, 0),
            ("2025Q2", 1_288, 4, 0),
            ("2025Q3", 1_291, 1, 0),
            ("2025Q4", 1_268, 0, 12),
            ("2026Q1", 634, 0, 6),
            ("2026Q2", 1_265, 3, 12),
        ] {
            let counts = &r.by_vintage[release];
            assert_eq!(
                region_changes.get(release).copied().unwrap_or(0),
                regions,
                "{release} region boundary"
            );
            assert_eq!(
                counts.comparisons,
                passed + changed + regions,
                "{release} T16 population plus explicit region changes"
            );
            assert_eq!(
                counts
                    .states
                    .get(&VintageState::ChangedWithoutRegisteredExplanation)
                    .copied()
                    .unwrap_or(0),
                changed,
                "{release} independently recorded changes"
            );
        }
        assert_eq!(region_changes.values().sum::<usize>(), 30);
        assert_eq!(r.totals.comparisons - 30, 6_400);
        assert!(r.items.iter().any(|i| {
            i.against_first.reason == Some("registered_basis_correction")
                && i.current.iter().any(|c| {
                    c.key.subject == "Porgera" && c.basis_evidence.as_deref() == Some("W1-DR-01")
                })
        }));
        assert!(
            r.items
                .iter()
                .any(|i| i.against_first.reason == Some("region_map"))
        );
        assert_eq!(
            r.totals.appearances,
            r.totals.states.values().sum::<usize>()
        );
        let mut reversed = claims;
        reversed.reverse();
        assert_eq!(r.to_json(), compare_vintages(&reversed).to_json());
        println!(
            "{{\"test\":\"mine_stats_vintages\",\"series\":{},\"candidate_groups\":{},\"multi_vintage\":{},\"totals\":{},\"independent_census\":false}}",
            r.series,
            r.candidate_groups,
            r.multi_vintage,
            r.totals.to_json()
        );
        let mut reasons = BTreeMap::<&str, usize>::new();
        for i in &r.items {
            if let Some(reason) = i.against_first.reason {
                *reasons.entry(reason).or_default() += 1;
            }
        }
        println!("finding reasons: {reasons:?}");
        for i in &r.items {
            if i.against_first.state == VintageState::ChangedWithoutRegisteredExplanation {
                println!("{}", i.to_json());
            }
        }
    }
}
