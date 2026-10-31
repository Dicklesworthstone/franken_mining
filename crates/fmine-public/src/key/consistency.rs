//! DESIGN (BD-504e.3): compare source assertions across registered releases by semantic key. A changed
//! value is a finding, never a choice of which release is true. A verified registry supplies disclosure
//! citations; an absent or incomplete registry never proves silence. Hidden sheets never enter its input.

use std::collections::{BTreeMap, BTreeSet};

use fmine_core::hash::Digest32;
use fmine_numerics::exact_feas::Rat;

use super::disclosure::{DisclosureCitation, VerifiedDisclosureRegistry};
use super::{
    KeyedCell, OwnershipBasis, PublicValue, Scope, SeriesKey, SeriesOwnershipBasis, claims_digest,
};
use crate::js;

/// The caller supplies visible canonical Claims for exactly this public document family.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum VintageScope {
    #[default]
    MineStats,
    Aisc,
}

impl VintageScope {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::MineStats => "mine_stats",
            Self::Aisc => "aisc",
        }
    }
}

/// One classification for each appearance of a candidate figure in a release.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum VintageState {
    FirstRegistered,
    SingleVintage,
    Unchanged,
    ChangedWithoutRegisteredExplanation,
    RestatedDisclosed,
    RestatedSilent,
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
            Self::RestatedDisclosed => "restated_disclosed",
            Self::RestatedSilent => "restated_silent",
            Self::Conflict => "conflict",
            Self::NotComparable => "not_comparable",
        }
    }
}

/// A comparison never converts units or ownership interests. `reason` identifies the changed value class
/// or the precise reason a numerical comparison was not made.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Comparison {
    pub state: VintageState,
    pub reason: Option<&'static str>,
    pub token_glyph_changed: bool,
    /// Only verified, temporally applicable notes covering the actual canonical keys appear here.
    pub disclosures: Vec<DisclosureCitation>,
}

impl Comparison {
    fn new(state: VintageState, reason: Option<&'static str>) -> Self {
        Self {
            state,
            reason,
            token_glyph_changed: false,
            disclosures: Vec::new(),
        }
    }

    fn to_json(&self) -> String {
        let disclosures = self
            .disclosures
            .iter()
            .map(DisclosureCitation::to_json)
            .collect::<Vec<_>>()
            .join(",");
        let interpretation = if self.state == VintageState::RestatedSilent {
            js("Value differs between vintages; no note found in registered documents.")
        } else {
            "null".to_string()
        };
        format!(
            "{{\"state\":{},\"reason\":{},\"token_glyph_changed\":{},\"rounding_only\":null,\
             \"disclosures\":[{disclosures}],\"interpretation\":{interpretation}}}",
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
                .as_ref()
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
    fn count(&mut self, comparison: &Comparison, later: bool) {
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
    pub scope: VintageScope,
    pub input_claims_digest: Digest32,
    pub disclosure_registry_digest: Option<Digest32>,
    /// `None` means unassessed; an incomplete registry cannot establish the absence of a covering note.
    pub disclosure_registry_complete: Option<bool>,
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
        let disclosure_check = match self.disclosure_registry_complete {
            None => "not_performed",
            Some(true) => "verified_registry",
            Some(false) => "incomplete_registry",
        };
        let disclosure_digest = self
            .disclosure_registry_digest
            .map_or_else(|| "null".into(), |d| js(&d.to_hex()));
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
            "{{\"rules_version\":\"public-vintages/2\",\"scope\":{},\
             \"first_registered_caveat\":\"The earliest registered release need not be the issuer's first publication.\",\
             \"disclosure_check\":{},\"input_claims_digest\":{},\
             \"disclosure_registry_digest\":{disclosure_digest},\
             \"interpretation_limit\":\"A difference does not establish which assertion is true. Silence means no applicable verified note in the registered documents; it does not establish absence of disclosure elsewhere.\",\
             \"series\":{},\"candidate_groups\":{},\"multi_vintage\":{},\"totals\":{},\
             \"by_vintage\":{{{vintages}}},\"items\":[{items}]}}",
            js(self.scope.as_str()),
            js(disclosure_check),
            js(&self.input_claims_digest.to_hex()),
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
    if c.basis_evidence.is_some()
        && c.printed_basis != OwnershipBasis::Unprinted
        && SeriesOwnershipBasis::from(c.printed_basis)
            != SeriesOwnershipBasis::from(c.key.ownership_basis)
    {
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
            disclosures: Vec::new(),
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
    compare_vintages_with_disclosures(claims, VintageScope::MineStats, None)
}

fn assess_disclosure(
    mut comparison: Comparison,
    from: &[&KeyedCell],
    to: &[&KeyedCell],
    registry: Option<&VerifiedDisclosureRegistry>,
) -> Comparison {
    let Some(registry) = registry else {
        return comparison;
    };
    let changed = comparison.state == VintageState::ChangedWithoutRegisteredExplanation;
    if (changed || comparison.reason == Some("region_map")) && from.len() == 1 && to.len() == 1 {
        // Coverage uses the actual keys, never the relaxed ownership, region or section candidate key.
        comparison.disclosures = registry.matching(&from[0].key, &to[0].key);
        if changed {
            comparison.state = if !comparison.disclosures.is_empty() {
                VintageState::RestatedDisclosed
            } else if registry.is_complete() {
                VintageState::RestatedSilent
            } else {
                VintageState::ChangedWithoutRegisteredExplanation
            };
        }
    }
    comparison
}

/// Compare one document family's canonical Claims and optionally assess disclosures against a verified
/// registry. Uncovered changes become `restated_silent` only after a complete registry check; a region-map
/// boundary stays `not_comparable` even when a verified note explains it. Both comparisons keep citations.
#[must_use]
pub fn compare_vintages_with_disclosures(
    claims: &[KeyedCell],
    scope: VintageScope,
    registry: Option<&VerifiedDisclosureRegistry>,
) -> VintageReport {
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
    for releases in groups.values_mut() {
        for group in releases.values_mut() {
            group.sort_by(|a, b| {
                (&a.key, &a.locator, a.claim.claim_id)
                    .cmp(&(&b.key, &b.locator, b.claim.claim_id))
                    .then_with(|| a.to_json().cmp(&b.to_json()))
            });
        }
    }
    // A conflicting caller can supply multiple values at the same canonical locator. Establish their
    // deterministic tie order before the canonical digest's key/locator sort, without dropping any input.
    let input_claims_digest = claims_digest(
        groups
            .values()
            .flat_map(|releases| releases.values().flat_map(|group| group.iter().copied())),
    );
    let mut report = VintageReport {
        scope,
        input_claims_digest,
        disclosure_registry_digest: registry.map(VerifiedDisclosureRegistry::digest),
        disclosure_registry_complete: registry.map(VerifiedDisclosureRegistry::is_complete),
        series: series.len(),
        candidate_groups: groups.len(),
        ..VintageReport::default()
    };
    for releases in groups.values() {
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
                assess_disclosure(compare(first, current), first, current, registry)
            };
            let against_previous = (index != 0).then(|| {
                assess_disclosure(compare(previous, current), previous, current, registry)
            });
            report.totals.count(&against_first, index != 0);
            report
                .by_vintage
                .entry((*release).to_string())
                .or_default()
                .count(&against_first, index != 0);
            let exceptional = |c: &Comparison| {
                c.token_glyph_changed
                    || matches!(
                        c.state,
                        VintageState::ChangedWithoutRegisteredExplanation
                            | VintageState::RestatedDisclosed
                            | VintageState::RestatedSilent
                            | VintageState::Conflict
                            | VintageState::NotComparable
                    )
            };
            if exceptional(&against_first) || against_previous.as_ref().is_some_and(exceptional) {
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

// Cross-crate vintage comparisons live in xtask/tests/public_mine_stats_vintages.rs.
