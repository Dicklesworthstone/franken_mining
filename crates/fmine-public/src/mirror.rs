//! Determinism: BitwisePortable
//! JV mirror pair registry (`BD-346.S2`, `bd-xzvb.2`, plan §21.6): dated perimeters, exact
//! `ExactQty` ownership shares, and reporting granularities for Barrick's three dual-reported
//! joint ventures (NGM with Newmont, Pueblo Viejo with Newmont, Kibali with AngloGold Ashanti).
//!
//! No share is ever encoded as `f64` or defaulted to 100%. A reporting-scope change (such as
//! Pueblo Viejo's Q4 2025 move to Barrick's North America region, `E461`) never mutates an
//! asset's physical `perimeter`. Any period straddling or falling after an unresolved perimeter
//! or share change (`NGM` 2026 vend-in, `E658`/`E558`) refuses with `FMINE-REF-104`, naming the
//! missing parameter.

use core::cmp::Ordering;
use std::collections::BTreeSet;

use fmine_core::codec::Encoder;
use fmine_core::hash::{Digest32, sha256};
use fmine_evidence::{Refusal, RefusalCode};
use fmine_time::{CivilDate, days_in_month};
use fmine_units::ExactQty;

use crate::key::{OwnershipBasis, Period, PeriodKind, Scope, ShareSource};

/// Joint-venture assets tracked in the dual-reporter mirror registry (`BD-346`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AssetId {
    Ngm,
    PuebloViejo,
    Kibali,
}

impl AssetId {
    /// Parse a CLI/registry asset identifier; unknown assets refuse with `FMINE-REF-107`.
    pub fn parse(raw: &str) -> Result<Self, Refusal> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "ngm" | "nevada_gold_mines" | "nevada-gold-mines" => Ok(Self::Ngm),
            "pv" | "pueblo_viejo" | "pueblo-viejo" => Ok(Self::PuebloViejo),
            "kibali" => Ok(Self::Kibali),
            other => Err(Refusal::new(
                RefusalCode::UndeclaredUnitBasisOrTz107,
                format!("unknown JV mirror asset {other:?}; expected ngm, pueblo_viejo, or kibali"),
            )),
        }
    }

    /// Canonical lowercase registry key.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Ngm => "ngm",
            Self::PuebloViejo => "pueblo_viejo",
            Self::Kibali => "kibali",
        }
    }

    const fn tag(self) -> u8 {
        match self {
            Self::Ngm => 1,
            Self::PuebloViejo => 2,
            Self::Kibali => 3,
        }
    }
}

/// Coarsest reporting perimeter printed by a JV partner (`BD-346.S2` -> `S3`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PerimeterGranularity {
    Site,
    Segment,
    Region,
}

impl PerimeterGranularity {
    const fn tag(self) -> u8 {
        match self {
            Self::Site => 1,
            Self::Segment => 2,
            Self::Region => 3,
        }
    }

    /// Coarsest common perimeter between two reporters (`Site < Segment < Region`).
    #[must_use]
    pub const fn coarsest(self, other: Self) -> Self {
        if self.tag() >= other.tag() {
            self
        } else {
            other
        }
    }
}

/// Exact registered share or explicitly unresolved open parameter (never `f64`, never default 100%).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShareValue {
    Exact(ExactQty),
    Unresolved {
        parameter: &'static str,
        evidence: &'static str,
    },
}

/// One dated physical perimeter for a JV asset.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DatedPerimeter {
    pub from: CivilDate,
    pub to: Option<CivilDate>,
    pub operations: &'static [&'static str],
    /// When `true`, the effective date of this perimeter transition is still `OPEN` in public
    /// filings; periods overlapping or following it refuse with `FMINE-REF-104`.
    pub effective_date_parameter: Option<&'static str>,
    pub evidence: &'static str,
}

/// One JV partner's dated reporting entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reporter {
    pub issuer: &'static str,
    pub share: ShareValue,
    pub basis: OwnershipBasis,
    pub granularity: PerimeterGranularity,
    pub from: CivilDate,
    pub to: Option<CivilDate>,
    pub evidence: &'static str,
}

/// Non-reporting equity holder tracked so total ownership sums to `<= 1` (e.g., SOKIMO 1/10 at Kibali).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NonReporterHolder {
    pub holder: &'static str,
    pub share: ExactQty,
    pub from: CivilDate,
    pub to: Option<CivilDate>,
    pub evidence: &'static str,
}

/// Complete dated mirror entry for one JV asset.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MirrorPair {
    pub asset: AssetId,
    pub perimeter: Vec<DatedPerimeter>,
    pub reporters: Vec<Reporter>,
    pub non_reporter_holders: Vec<NonReporterHolder>,
}

/// Resolved reporter share for a specific period (guaranteed exact, never `Unprinted` or default 100%).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedReporter {
    pub issuer: &'static str,
    pub share: ExactQty,
    pub basis: OwnershipBasis,
    pub granularity: PerimeterGranularity,
    pub evidence: &'static str,
}

/// Resolved mirror pair for one `(asset, period)` handed to `BD-346.S3..S6`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedPair {
    pub asset: AssetId,
    pub period: Period,
    pub period_start: CivilDate,
    pub period_end: CivilDate,
    pub operations: &'static [&'static str],
    pub perimeter_evidence: &'static str,
    pub reporters: Vec<ResolvedReporter>,
    pub non_reporter_holders: Vec<NonReporterHolder>,
    pub coarsest_common_granularity: PerimeterGranularity,
}

/// Output of validating and digesting the registered JV mirror pairs (`BD-346.S2`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PairRegistryCheck {
    pub assets: usize,
    pub perimeter_entries: usize,
    pub unresolved_entries: usize,
    pub digest: Digest32,
}

impl PairRegistryCheck {
    /// Structured JSON log line required by `BD-346.S2`.
    #[must_use]
    pub fn to_json_log(&self) -> String {
        format!(
            "{{\"event\":\"pair_registry\",\"assets\":{},\"perimeter_entries\":{},\"unresolved_entries\":{},\"digest\":\"{}\"}}",
            self.assets,
            self.perimeter_entries,
            self.unresolved_entries,
            self.digest.to_hex()
        )
    }
}

/// Canonical pinned SHA-256 digest of `MPR1` v1 over [`pair_registry`], independently verified
/// against the `FMNE`/`MPR1` little-endian + CRC32C wire specification.
pub const PAIR_REGISTRY_GOLDEN_SHA256: &str =
    "8f996b6ea40fe52ccb8bae66890b1808a2cc38b191dabebd1b02d6c83a1cb649";

/// Exact Barrick regional reporting scope for Pueblo Viejo (`E461`).
///
/// From `2025Q4` onward Barrick reports Pueblo Viejo under North America (`map_version = 2`)
/// instead of Latin America & Asia Pacific (`map_version = 1`). This is strictly a reporting-scope
/// change (`BD-332` `Scope::Region { map_version }`), never a change to Pueblo Viejo's physical
/// [`DatedPerimeter`].
#[must_use]
pub const fn barrick_pv_reporting_region(period: Period) -> (&'static str, u8) {
    let (_, (end_y, end_m)) = period.months();
    if end_y > 2025 || (end_y == 2025 && end_m >= 10) {
        ("North America", 2)
    } else {
        ("Latin America & Asia Pacific", 1)
    }
}

/// `BD-332` [`Scope::Region`] for Barrick's reporting of Pueblo Viejo (`E461`).
#[must_use]
pub fn barrick_pv_reporting_scope(period: Period) -> Scope {
    let (region, version) = barrick_pv_reporting_region(period);
    Scope::Region {
        map_version: format!("barrick_pv_region_v{version}:{region}"),
    }
}

/// Construct the canonical code-constant JV mirror pair registry (`FACT`: `E457`, `E672`, `E560`,
/// `E658`, `E558`).
pub fn pair_registry() -> Result<Vec<MirrorPair>, Refusal> {
    let ngm_start = CivilDate {
        year: 2019,
        month: 7,
        day: 1,
    };
    let ngm_pre_end = CivilDate {
        year: 2026,
        month: 8,
        day: 8,
    };
    let ngm_vend_agreement = CivilDate {
        year: 2026,
        month: 8,
        day: 9,
    };
    let pv_start = CivilDate {
        year: 2012,
        month: 8,
        day: 1,
    };
    let kibali_start = CivilDate {
        year: 2013,
        month: 9,
        day: 1,
    };

    let ngm_barrick = qty(615, 1_000)?;
    let ngm_newmont = qty(385, 1_000)?;
    let pv_barrick = qty(3, 5)?;
    let pv_newmont = qty(2, 5)?;
    let kibali_barrick = qty(9, 20)?;
    let kibali_aga = qty(9, 20)?;
    let kibali_sokimo = qty(1, 10)?;

    Ok(vec![
        MirrorPair {
            asset: AssetId::Ngm,
            perimeter: vec![
                DatedPerimeter {
                    from: ngm_start,
                    to: Some(ngm_pre_end),
                    operations: &[
                        "Carlin",
                        "Cortez",
                        "Turquoise Ridge",
                        "Phoenix",
                        "Long Canyon",
                    ],
                    effective_date_parameter: None,
                    evidence: "E457",
                },
                // 9 Aug 2026 agreement (E658) / 10 Aug 2026 dispute settlement release (E558):
                // Fourmile (Barrick) + Mike & Fiberline (Newmont) vended into NGM; effective date
                // and post-vend-in equity shares remain OPEN until a filing states them.
                DatedPerimeter {
                    from: ngm_vend_agreement,
                    to: None,
                    operations: &[
                        "Carlin",
                        "Cortez",
                        "Turquoise Ridge",
                        "Phoenix",
                        "Long Canyon",
                        "Fourmile",
                        "Mike",
                        "Fiberline",
                    ],
                    effective_date_parameter: Some("ngm_2026_vend_in_effective_date"),
                    evidence: "E658,E558",
                },
            ],
            reporters: vec![
                Reporter {
                    issuer: "barrick",
                    share: ShareValue::Exact(ngm_barrick),
                    basis: OwnershipBasis::Attributable {
                        num: 123,
                        den: 200,
                        source: ShareSource::Registry,
                    },
                    granularity: PerimeterGranularity::Site,
                    from: ngm_start,
                    to: Some(ngm_pre_end),
                    evidence: "E457",
                },
                Reporter {
                    issuer: "newmont",
                    share: ShareValue::Exact(ngm_newmont),
                    basis: OwnershipBasis::Attributable {
                        num: 77,
                        den: 200,
                        source: ShareSource::Registry,
                    },
                    granularity: PerimeterGranularity::Segment,
                    from: ngm_start,
                    to: Some(ngm_pre_end),
                    evidence: "E457",
                },
                Reporter {
                    issuer: "barrick",
                    share: ShareValue::Unresolved {
                        parameter: "ngm_post_vend_in_barrick_share",
                        evidence: "E658,E558",
                    },
                    basis: OwnershipBasis::Unprinted,
                    granularity: PerimeterGranularity::Site,
                    from: ngm_vend_agreement,
                    to: None,
                    evidence: "E658,E558",
                },
                Reporter {
                    issuer: "newmont",
                    share: ShareValue::Unresolved {
                        parameter: "ngm_post_vend_in_newmont_share",
                        evidence: "E658,E558",
                    },
                    basis: OwnershipBasis::Unprinted,
                    granularity: PerimeterGranularity::Segment,
                    from: ngm_vend_agreement,
                    to: None,
                    evidence: "E658,E558",
                },
            ],
            non_reporter_holders: Vec::new(),
        },
        MirrorPair {
            asset: AssetId::PuebloViejo,
            perimeter: vec![DatedPerimeter {
                from: pv_start,
                to: None,
                operations: &["Pueblo Viejo"],
                effective_date_parameter: None,
                evidence: "E672",
            }],
            reporters: vec![
                Reporter {
                    issuer: "barrick",
                    share: ShareValue::Exact(pv_barrick),
                    basis: OwnershipBasis::Attributable {
                        num: 3,
                        den: 5,
                        source: ShareSource::Registry,
                    },
                    granularity: PerimeterGranularity::Site,
                    from: pv_start,
                    to: None,
                    evidence: "E672",
                },
                Reporter {
                    issuer: "newmont",
                    share: ShareValue::Exact(pv_newmont),
                    basis: OwnershipBasis::Attributable {
                        num: 2,
                        den: 5,
                        source: ShareSource::Registry,
                    },
                    granularity: PerimeterGranularity::Site,
                    from: pv_start,
                    to: None,
                    evidence: "E672",
                },
            ],
            non_reporter_holders: Vec::new(),
        },
        MirrorPair {
            asset: AssetId::Kibali,
            perimeter: vec![DatedPerimeter {
                from: kibali_start,
                to: None,
                operations: &["Kibali"],
                effective_date_parameter: None,
                evidence: "E560",
            }],
            reporters: vec![
                Reporter {
                    issuer: "barrick",
                    share: ShareValue::Exact(kibali_barrick),
                    basis: OwnershipBasis::Attributable {
                        num: 9,
                        den: 20,
                        source: ShareSource::Registry,
                    },
                    granularity: PerimeterGranularity::Site,
                    from: kibali_start,
                    to: None,
                    evidence: "E560",
                },
                Reporter {
                    issuer: "anglogold_ashanti",
                    share: ShareValue::Exact(kibali_aga),
                    basis: OwnershipBasis::Attributable {
                        num: 9,
                        den: 20,
                        source: ShareSource::Registry,
                    },
                    granularity: PerimeterGranularity::Site,
                    from: kibali_start,
                    to: None,
                    evidence: "E560",
                },
            ],
            non_reporter_holders: vec![NonReporterHolder {
                holder: "sokimo",
                share: kibali_sokimo,
                from: kibali_start,
                to: None,
                evidence: "E560",
            }],
        },
    ])
}

/// Convert a [`Period`] to its exact inclusive `(start_date, end_date)` civil dates.
#[must_use]
pub const fn period_date_bounds(period: Period) -> (CivilDate, CivilDate) {
    let ((sy, sm), (ey, em)) = period.months();
    let start = CivilDate {
        year: sy as i32,
        month: sm,
        day: 1,
    };
    let end = CivilDate {
        year: ey as i32,
        month: em,
        day: days_in_month(ey as i32, em),
    };
    (start, end)
}

/// Resolve a JV mirror pair for `(asset, period)` against the canonical registry.
pub fn resolve_pair(asset: AssetId, period: Period) -> Result<ResolvedPair, Refusal> {
    let pairs = pair_registry()?;
    resolve_pair_in(&pairs, asset, period)
}

/// Resolve a JV mirror pair against an explicit slice of [`MirrorPair`] entries (enables negative
/// control tests such as verifying unprinted shares never default to 100%).
pub fn resolve_pair_in(
    pairs: &[MirrorPair],
    asset: AssetId,
    period: Period,
) -> Result<ResolvedPair, Refusal> {
    if let PeriodKind::Quarter(q) = period.kind
        && !(1..=4).contains(&q)
    {
        return Err(Refusal::new(
            RefusalCode::UndeclaredUnitBasisOrTz107,
            format!("invalid quarter {q} in period; expected 1..=4"),
        ));
    }
    let pair = pairs.iter().find(|p| p.asset == asset).ok_or_else(|| {
        Refusal::new(
            RefusalCode::UndeclaredUnitBasisOrTz107,
            format!(
                "asset {:?} not present in mirror pair registry",
                asset.as_str()
            ),
        )
    })?;
    let (p_start, p_end) = period_date_bounds(period);

    // Validate stored civil dates before range comparisons.
    for dp in &pair.perimeter {
        validate_civil_date("perimeter.from", dp.from)?;
        if let Some(to) = dp.to {
            validate_civil_date("perimeter.to", to)?;
        }
    }
    for r in &pair.reporters {
        validate_civil_date("reporter.from", r.from)?;
        if let Some(to) = r.to {
            validate_civil_date("reporter.to", to)?;
        }
    }
    for nr in &pair.non_reporter_holders {
        validate_civil_date("non_reporter.from", nr.from)?;
        if let Some(to) = nr.to {
            validate_civil_date("non_reporter.to", to)?;
        }
    }

    // 1. Resolve dated perimeter: must match exactly one entry fully covering [p_start, p_end].
    let overlapping_perimeters: Vec<&DatedPerimeter> = pair
        .perimeter
        .iter()
        .filter(|dp| date_ranges_overlap(dp.from, dp.to, p_start, p_end))
        .collect();
    if overlapping_perimeters.is_empty() {
        return Err(Refusal::new(
            RefusalCode::NonIdentifiableAttribution104,
            format!(
                "no registered perimeter covers {} for period {}",
                asset.as_str(),
                period.code()
            ),
        ));
    }
    if overlapping_perimeters.len() > 1 {
        let param = overlapping_perimeters
            .iter()
            .find_map(|dp| dp.effective_date_parameter)
            .unwrap_or("perimeter_transition_date");
        return Err(Refusal::new(
            RefusalCode::NonIdentifiableAttribution104,
            format!(
                "period {} straddles {} perimeter change (unresolved parameter: {param}); never interpolates",
                period.code(),
                asset.as_str()
            ),
        ));
    }
    let perimeter = overlapping_perimeters[0];
    if !date_range_covers(perimeter.from, perimeter.to, p_start, p_end) {
        return Err(Refusal::new(
            RefusalCode::NonIdentifiableAttribution104,
            format!(
                "registered perimeter for {} does not cover full period {}",
                asset.as_str(),
                period.code()
            ),
        ));
    }
    if let Some(param) = perimeter.effective_date_parameter {
        return Err(Refusal::new(
            RefusalCode::NonIdentifiableAttribution104,
            format!(
                "{} perimeter for period {} depends on unresolved parameter '{param}' ({})",
                asset.as_str(),
                period.code(),
                perimeter.evidence
            ),
        ));
    }

    // 2. Resolve reporters active in [p_start, p_end]: a JV mirror pair requires at least two
    //    distinct active reporters, each covering the full period with an exact share and an
    //    explicit Attributable basis (never Unprinted / never default 100%).
    let mut active_issuers: BTreeSet<&str> = BTreeSet::new();
    for r in &pair.reporters {
        if date_ranges_overlap(r.from, r.to, p_start, p_end) {
            active_issuers.insert(r.issuer);
        }
    }
    if active_issuers.len() < 2 {
        return Err(Refusal::new(
            RefusalCode::NonIdentifiableAttribution104,
            format!(
                "fewer than 2 active reporters cover {} in {} (found {})",
                asset.as_str(),
                period.code(),
                active_issuers.len()
            ),
        ));
    }

    let mut resolved_reporters = Vec::new();
    let mut share_sum = ExactQty::from_integer(0);
    let mut coarsest = PerimeterGranularity::Site;

    for issuer in active_issuers {
        let active: Vec<&Reporter> = pair
            .reporters
            .iter()
            .filter(|r| r.issuer == issuer && date_ranges_overlap(r.from, r.to, p_start, p_end))
            .collect();
        if active.len() > 1 {
            let param = active
                .iter()
                .find_map(|r| match r.share {
                    ShareValue::Unresolved { parameter, .. } => Some(parameter),
                    ShareValue::Exact(_) => None,
                })
                .unwrap_or("reporter_share_transition");
            return Err(Refusal::new(
                RefusalCode::NonIdentifiableAttribution104,
                format!(
                    "reporter '{issuer}' straddles share change on {} in {} (unresolved parameter: {param})",
                    asset.as_str(),
                    period.code()
                ),
            ));
        }
        let rep = active[0];
        if !date_range_covers(rep.from, rep.to, p_start, p_end) {
            return Err(Refusal::new(
                RefusalCode::NonIdentifiableAttribution104,
                format!(
                    "reporter '{issuer}' entry does not cover full period {} on {}",
                    period.code(),
                    asset.as_str()
                ),
            ));
        }
        let exact_share = match rep.share {
            ShareValue::Unresolved {
                parameter,
                evidence,
            } => {
                return Err(Refusal::new(
                    RefusalCode::NonIdentifiableAttribution104,
                    format!(
                        "unresolved share parameter '{parameter}' for reporter '{issuer}' on {} in {} ({evidence})",
                        asset.as_str(),
                        period.code()
                    ),
                ));
            }
            ShareValue::Exact(s) => s,
        };
        match rep.basis {
            OwnershipBasis::Unprinted => {
                return Err(Refusal::new(
                    RefusalCode::NonIdentifiableAttribution104,
                    format!(
                        "reporter '{issuer}' on {} has OwnershipBasis::Unprinted; shares never default to 100%",
                        asset.as_str()
                    ),
                ));
            }
            OwnershipBasis::Consolidated100 => {
                return Err(Refusal::new(
                    RefusalCode::NonIdentifiableAttribution104,
                    format!(
                        "JV mirror reporter '{issuer}' on {} cannot silently use Consolidated100",
                        asset.as_str()
                    ),
                ));
            }
            OwnershipBasis::Attributable { num, den, .. } => {
                let basis_qty = qty(i128::from(num), i128::from(den))?;
                if basis_qty != exact_share {
                    return Err(Refusal::new(
                        RefusalCode::OutsidePhysicalDomain109,
                        format!(
                            "reporter '{issuer}' basis {num}/{den} disagrees with exact share {}/{}",
                            exact_share.num(),
                            exact_share.den()
                        ),
                    ));
                }
            }
        }
        validate_share_open_unit("reporter.share", exact_share)?;
        share_sum = share_sum.checked_add(exact_share).map_err(units_err)?;
        coarsest = coarsest.coarsest(rep.granularity);
        resolved_reporters.push(ResolvedReporter {
            issuer: rep.issuer,
            share: exact_share,
            basis: rep.basis,
            granularity: rep.granularity,
            evidence: rep.evidence,
        });
    }

    // 3. Active non-reporter holders (e.g., SOKIMO 1/10 at Kibali).
    let mut seen_non_reporters = BTreeSet::new();
    let mut active_non_reporters = Vec::new();
    for nr in &pair.non_reporter_holders {
        if date_ranges_overlap(nr.from, nr.to, p_start, p_end) {
            if !seen_non_reporters.insert(nr.holder) {
                return Err(Refusal::new(
                    RefusalCode::NonIdentifiableAttribution104,
                    format!(
                        "non-reporter holder '{}' has multiple overlapping entries in {} on {}",
                        nr.holder,
                        period.code(),
                        asset.as_str()
                    ),
                ));
            }
            if !date_range_covers(nr.from, nr.to, p_start, p_end) {
                return Err(Refusal::new(
                    RefusalCode::NonIdentifiableAttribution104,
                    format!(
                        "non-reporter holder '{}' does not cover full period {} on {}",
                        nr.holder,
                        period.code(),
                        asset.as_str()
                    ),
                ));
            }
            validate_share_open_unit("non_reporter.share", nr.share)?;
            share_sum = share_sum.checked_add(nr.share).map_err(units_err)?;
            active_non_reporters.push(nr.clone());
        }
    }

    if share_sum
        .checked_cmp(ExactQty::from_integer(1))
        .map_err(units_err)?
        == Ordering::Greater
    {
        return Err(Refusal::new(
            RefusalCode::OutsidePhysicalDomain109,
            format!(
                "total ownership shares on {} in {} exceed 1 ({}/{})",
                asset.as_str(),
                period.code(),
                share_sum.num(),
                share_sum.den()
            ),
        ));
    }

    Ok(ResolvedPair {
        asset,
        period,
        period_start: p_start,
        period_end: p_end,
        operations: perimeter.operations,
        perimeter_evidence: perimeter.evidence,
        reporters: resolved_reporters,
        non_reporter_holders: active_non_reporters,
        coarsest_common_granularity: coarsest,
    })
}

/// Validate the entire canonical pair registry and compute its deterministic `MPR1` SHA-256 digest.
pub fn check_pair_registry() -> Result<PairRegistryCheck, Refusal> {
    let pairs = pair_registry()?;
    check_pair_registry_slice(&pairs)
}

/// Validate an arbitrary slice of [`MirrorPair`] records and compute its canonical `MPR1` digest.
pub fn check_pair_registry_slice(pairs: &[MirrorPair]) -> Result<PairRegistryCheck, Refusal> {
    let one = ExactQty::from_integer(1);
    let mut seen_assets = BTreeSet::new();
    let mut perimeter_entries = 0usize;
    let mut unresolved_entries = 0usize;

    let mut enc = Encoder::new_frame(*b"MPR1", 1);
    enc.put_u64(pairs.len() as u64);

    for pair in pairs {
        if !seen_assets.insert(pair.asset) {
            return Err(Refusal::new(
                RefusalCode::UndeclaredUnitBasisOrTz107,
                format!(
                    "duplicate asset '{}' in mirror pair registry",
                    pair.asset.as_str()
                ),
            ));
        }
        if pair.perimeter.is_empty() || pair.reporters.is_empty() {
            return Err(Refusal::new(
                RefusalCode::UndeclaredUnitBasisOrTz107,
                format!(
                    "asset '{}' requires >= 1 perimeter and >= 1 reporter",
                    pair.asset.as_str()
                ),
            ));
        }
        enc.put_u8(pair.asset.tag());
        enc.put_str(pair.asset.as_str());

        // Validate non-overlapping chronological perimeters.
        enc.put_u64(pair.perimeter.len() as u64);
        let mut prev_end: Option<Option<CivilDate>> = None;
        for dp in &pair.perimeter {
            perimeter_entries += 1;
            validate_civil_date("perimeter.from", dp.from)?;
            if let Some(to) = dp.to {
                validate_civil_date("perimeter.to", to)?;
                if dp.from.to_epoch_days() > to.to_epoch_days() {
                    return Err(Refusal::new(
                        RefusalCode::OutsidePhysicalDomain109,
                        format!("invalid perimeter date order on '{}'", pair.asset.as_str()),
                    ));
                }
            }
            if let Some(prev) = prev_end {
                match prev {
                    None => {
                        return Err(Refusal::new(
                            RefusalCode::OutsidePhysicalDomain109,
                            format!(
                                "open-ended perimeter precedes another on '{}'",
                                pair.asset.as_str()
                            ),
                        ));
                    }
                    Some(pe) if dp.from.to_epoch_days() <= pe.to_epoch_days() => {
                        return Err(Refusal::new(
                            RefusalCode::OutsidePhysicalDomain109,
                            format!("overlapping perimeters on '{}'", pair.asset.as_str()),
                        ));
                    }
                    Some(_) => {}
                }
            }
            prev_end = Some(dp.to);
            if dp.operations.is_empty() || dp.evidence.trim().is_empty() {
                return Err(Refusal::new(
                    RefusalCode::UndeclaredUnitBasisOrTz107,
                    format!(
                        "perimeter on '{}' missing operations or evidence",
                        pair.asset.as_str()
                    ),
                ));
            }
            let mut seen_ops = BTreeSet::new();
            for &op in dp.operations {
                if op.trim().is_empty() || op != op.trim() || !seen_ops.insert(op) {
                    return Err(Refusal::new(
                        RefusalCode::UndeclaredUnitBasisOrTz107,
                        format!(
                            "invalid or duplicate operation {op:?} on '{}'",
                            pair.asset.as_str()
                        ),
                    ));
                }
            }
            if let Some(param) = dp.effective_date_parameter
                && (param.trim().is_empty() || param != param.trim())
            {
                return Err(Refusal::new(
                    RefusalCode::UndeclaredUnitBasisOrTz107,
                    format!(
                        "blank or untrimmed effective_date_parameter {param:?} on '{}'",
                        pair.asset.as_str()
                    ),
                ));
            }
            put_date(&mut enc, dp.from);
            put_opt_date(&mut enc, dp.to);
            enc.put_u64(dp.operations.len() as u64);
            for op in dp.operations {
                enc.put_str(op);
            }
            enc.put_u8(u8::from(dp.effective_date_parameter.is_some()));
            if let Some(param) = dp.effective_date_parameter {
                enc.put_str(param);
            }
            enc.put_str(dp.evidence);
        }

        // Validate reporters, non-overlapping per-issuer ranges, basis consistency, and share sums.
        enc.put_u64(pair.reporters.len() as u64);
        let mut checkpoints = BTreeSet::new();
        let mut issuer_spans: std::collections::BTreeMap<
            &str,
            Vec<(CivilDate, Option<CivilDate>)>,
        > = std::collections::BTreeMap::new();
        for r in &pair.reporters {
            if r.issuer.trim().is_empty() || r.evidence.trim().is_empty() {
                return Err(Refusal::new(
                    RefusalCode::UndeclaredUnitBasisOrTz107,
                    format!(
                        "reporter on '{}' missing issuer or evidence",
                        pair.asset.as_str()
                    ),
                ));
            }
            validate_civil_date("reporter.from", r.from)?;
            checkpoints.insert(r.from.to_epoch_days());
            if let Some(to) = r.to {
                validate_civil_date("reporter.to", to)?;
                if r.from.to_epoch_days() > to.to_epoch_days() {
                    return Err(Refusal::new(
                        RefusalCode::OutsidePhysicalDomain109,
                        format!(
                            "invalid reporter date order for '{}' on '{}'",
                            r.issuer,
                            pair.asset.as_str()
                        ),
                    ));
                }
            }
            let spans = issuer_spans.entry(r.issuer).or_default();
            for &(ef, et) in spans.iter() {
                if spans_overlap(ef, et, r.from, r.to) {
                    return Err(Refusal::new(
                        RefusalCode::OutsidePhysicalDomain109,
                        format!(
                            "overlapping reporter date spans for '{}' on '{}'",
                            r.issuer,
                            pair.asset.as_str()
                        ),
                    ));
                }
            }
            spans.push((r.from, r.to));

            enc.put_str(r.issuer);
            match r.share {
                ShareValue::Exact(s) => {
                    validate_share_open_unit("reporter.share", s)?;
                    match r.basis {
                        OwnershipBasis::Attributable { num, den, .. }
                            if qty(i128::from(num), i128::from(den))? == s => {}
                        _ => {
                            return Err(Refusal::new(
                                RefusalCode::OutsidePhysicalDomain109,
                                format!(
                                    "reporter '{}' exact share disagrees with basis on '{}'",
                                    r.issuer,
                                    pair.asset.as_str()
                                ),
                            ));
                        }
                    }
                    enc.put_u8(1);
                    enc.put_i128(s.num());
                    enc.put_i128(s.den());
                }
                ShareValue::Unresolved {
                    parameter,
                    evidence,
                } => {
                    if parameter.trim().is_empty()
                        || parameter != parameter.trim()
                        || evidence.trim().is_empty()
                    {
                        return Err(Refusal::new(
                            RefusalCode::UndeclaredUnitBasisOrTz107,
                            format!(
                                "unresolved reporter '{}' on '{}' missing parameter or evidence",
                                r.issuer,
                                pair.asset.as_str()
                            ),
                        ));
                    }
                    if r.basis != OwnershipBasis::Unprinted {
                        return Err(Refusal::new(
                            RefusalCode::OutsidePhysicalDomain109,
                            format!(
                                "unresolved reporter '{}' on '{}' must carry OwnershipBasis::Unprinted",
                                r.issuer,
                                pair.asset.as_str()
                            ),
                        ));
                    }
                    unresolved_entries += 1;
                    enc.put_u8(2);
                    enc.put_str(parameter);
                    enc.put_str(evidence);
                }
            }
            put_basis(&mut enc, r.basis);
            enc.put_u8(r.granularity.tag());
            put_date(&mut enc, r.from);
            put_opt_date(&mut enc, r.to);
            enc.put_str(r.evidence);
        }

        enc.put_u64(pair.non_reporter_holders.len() as u64);
        let mut holder_spans: std::collections::BTreeMap<
            &str,
            Vec<(CivilDate, Option<CivilDate>)>,
        > = std::collections::BTreeMap::new();
        for nr in &pair.non_reporter_holders {
            if nr.holder.trim().is_empty() || nr.evidence.trim().is_empty() {
                return Err(Refusal::new(
                    RefusalCode::UndeclaredUnitBasisOrTz107,
                    format!(
                        "non-reporter holder on '{}' missing holder or evidence",
                        pair.asset.as_str()
                    ),
                ));
            }
            validate_civil_date("non_reporter.from", nr.from)?;
            if let Some(to) = nr.to {
                validate_civil_date("non_reporter.to", to)?;
                if nr.from.to_epoch_days() > to.to_epoch_days() {
                    return Err(Refusal::new(
                        RefusalCode::OutsidePhysicalDomain109,
                        format!(
                            "invalid date order for non-reporter holder '{}' on '{}'",
                            nr.holder,
                            pair.asset.as_str()
                        ),
                    ));
                }
            }
            let spans = holder_spans.entry(nr.holder).or_default();
            for &(ef, et) in spans.iter() {
                if spans_overlap(ef, et, nr.from, nr.to) {
                    return Err(Refusal::new(
                        RefusalCode::OutsidePhysicalDomain109,
                        format!(
                            "overlapping non-reporter holder spans for '{}' on '{}'",
                            nr.holder,
                            pair.asset.as_str()
                        ),
                    ));
                }
            }
            spans.push((nr.from, nr.to));
            validate_share_open_unit("non_reporter.share", nr.share)?;
            checkpoints.insert(nr.from.to_epoch_days());
            enc.put_str(nr.holder);
            enc.put_i128(nr.share.num());
            enc.put_i128(nr.share.den());
            put_date(&mut enc, nr.from);
            put_opt_date(&mut enc, nr.to);
            enc.put_str(nr.evidence);
        }

        for day in checkpoints {
            let mut sum = ExactQty::from_integer(0);
            for r in &pair.reporters {
                if date_contains_day(r.from, r.to, day)
                    && let ShareValue::Exact(s) = r.share
                {
                    sum = sum.checked_add(s).map_err(units_err)?;
                }
            }
            for nr in &pair.non_reporter_holders {
                if date_contains_day(nr.from, nr.to, day) {
                    sum = sum.checked_add(nr.share).map_err(units_err)?;
                }
            }
            if sum.checked_cmp(one).map_err(units_err)? == Ordering::Greater {
                return Err(Refusal::new(
                    RefusalCode::OutsidePhysicalDomain109,
                    format!(
                        "shares on '{}' exceed 1 ({}/{})",
                        pair.asset.as_str(),
                        sum.num(),
                        sum.den()
                    ),
                ));
            }
        }
    }

    Ok(PairRegistryCheck {
        assets: pairs.len(),
        perimeter_entries,
        unresolved_entries,
        digest: sha256(&enc.finish()),
    })
}

fn qty(num: i128, den: i128) -> Result<ExactQty, Refusal> {
    ExactQty::new(num, den).map_err(units_err)
}

fn units_err(err: fmine_units::UnitsError) -> Refusal {
    Refusal::new(
        RefusalCode::OutsidePhysicalDomain109,
        format!("exact share arithmetic error: {err}"),
    )
}

fn validate_share_open_unit(field: &str, share: ExactQty) -> Result<(), Refusal> {
    let one = ExactQty::from_integer(1);
    if share.signum() <= 0 || share.checked_cmp(one).map_err(units_err)? != Ordering::Less {
        return Err(Refusal::new(
            RefusalCode::OutsidePhysicalDomain109,
            format!(
                "{field} for a JV partner must lie strictly in (0, 1), got {}/{}",
                share.num(),
                share.den()
            ),
        ));
    }
    Ok(())
}

fn validate_civil_date(field: &str, d: CivilDate) -> Result<(), Refusal> {
    if !(1..=12).contains(&d.month) {
        return Err(Refusal::new(
            RefusalCode::OutsidePhysicalDomain109,
            format!("{field} has invalid month {} (expected 1..=12)", d.month),
        ));
    }
    let max_day = days_in_month(d.year, d.month);
    if d.day < 1 || d.day > max_day {
        return Err(Refusal::new(
            RefusalCode::OutsidePhysicalDomain109,
            format!(
                "{field} has invalid day {} for {:04}-{:02} (expected 1..={max_day})",
                d.day, d.year, d.month
            ),
        ));
    }
    Ok(())
}

fn spans_overlap(
    a_from: CivilDate,
    a_to: Option<CivilDate>,
    b_from: CivilDate,
    b_to: Option<CivilDate>,
) -> bool {
    match (a_to, b_to) {
        (Some(at), Some(bt)) => {
            a_from.to_epoch_days() <= bt.to_epoch_days()
                && b_from.to_epoch_days() <= at.to_epoch_days()
        }
        (None, Some(bt)) => a_from.to_epoch_days() <= bt.to_epoch_days(),
        (Some(at), None) => b_from.to_epoch_days() <= at.to_epoch_days(),
        (None, None) => true,
    }
}

fn date_contains_day(from: CivilDate, to: Option<CivilDate>, day: i64) -> bool {
    day >= from.to_epoch_days() && to.is_none_or(|t| day <= t.to_epoch_days())
}

fn date_ranges_overlap(
    a_from: CivilDate,
    a_to: Option<CivilDate>,
    b_from: CivilDate,
    b_to: CivilDate,
) -> bool {
    let af = a_from.to_epoch_days();
    let bf = b_from.to_epoch_days();
    let bt = b_to.to_epoch_days();
    af <= bt && a_to.is_none_or(|at| at.to_epoch_days() >= bf)
}

fn date_range_covers(
    outer_from: CivilDate,
    outer_to: Option<CivilDate>,
    inner_from: CivilDate,
    inner_to: CivilDate,
) -> bool {
    outer_from.to_epoch_days() <= inner_from.to_epoch_days()
        && outer_to.is_none_or(|ot| ot.to_epoch_days() >= inner_to.to_epoch_days())
}

fn put_date(enc: &mut Encoder, d: CivilDate) {
    enc.put_i64(i64::from(d.year));
    enc.put_u8(d.month);
    enc.put_u8(d.day);
}

fn put_opt_date(enc: &mut Encoder, d: Option<CivilDate>) {
    enc.put_u8(u8::from(d.is_some()));
    if let Some(v) = d {
        put_date(enc, v);
    }
}

fn put_basis(enc: &mut Encoder, basis: OwnershipBasis) {
    match basis {
        OwnershipBasis::Consolidated100 => enc.put_u8(1),
        OwnershipBasis::Attributable { num, den, source } => {
            enc.put_u8(2);
            enc.put_i64(num);
            enc.put_i64(den);
            enc.put_u8(match source {
                ShareSource::Printed => 1,
                ShareSource::Registry => 2,
            });
        }
        OwnershipBasis::Unprinted => enc.put_u8(3),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::key::PeriodKind;

    #[test]
    fn test_pair_registry_shares_exact_and_sum_le_one() {
        let fy2025 = Period {
            year: 2025,
            kind: PeriodKind::FullYear,
        };
        let one = ExactQty::from_integer(1);

        // NGM: 615/1000 Barrick + 385/1000 Newmont = 1 (E457).
        let ngm = resolve_pair(AssetId::Ngm, fy2025).expect("resolve NGM FY2025");
        assert_eq!(ngm.perimeter_evidence, "E457");
        assert_eq!(ngm.reporters.len(), 2);
        let b_ngm = ngm
            .reporters
            .iter()
            .find(|r| r.issuer == "barrick")
            .unwrap();
        let n_ngm = ngm
            .reporters
            .iter()
            .find(|r| r.issuer == "newmont")
            .unwrap();
        assert_eq!(b_ngm.share, qty(615, 1_000).unwrap());
        assert_eq!(n_ngm.share, qty(385, 1_000).unwrap());
        assert_eq!(b_ngm.evidence, "E457");
        assert_eq!(b_ngm.share.checked_add(n_ngm.share).unwrap(), one);
        assert_eq!(
            ngm.coarsest_common_granularity,
            PerimeterGranularity::Segment
        );

        // Pueblo Viejo: 3/5 Barrick + 2/5 Newmont = 1 (E672).
        let pv = resolve_pair(AssetId::PuebloViejo, fy2025).expect("resolve PV FY2025");
        let b_pv = pv.reporters.iter().find(|r| r.issuer == "barrick").unwrap();
        let n_pv = pv.reporters.iter().find(|r| r.issuer == "newmont").unwrap();
        assert_eq!(b_pv.share, qty(3, 5).unwrap());
        assert_eq!(n_pv.share, qty(2, 5).unwrap());
        assert_eq!(b_pv.share.checked_add(n_pv.share).unwrap(), one);

        // Kibali: 9/20 Barrick + 9/20 AngloGold Ashanti + 1/10 SOKIMO (non-reporter) = 1 exact (E560).
        let kib = resolve_pair(AssetId::Kibali, fy2025).expect("resolve Kibali FY2025");
        assert_eq!(kib.reporters.len(), 2);
        assert_eq!(kib.non_reporter_holders.len(), 1);
        assert_eq!(kib.non_reporter_holders[0].holder, "sokimo");
        let kib_sum = kib
            .reporters
            .iter()
            .map(|r| r.share)
            .chain(kib.non_reporter_holders.iter().map(|n| n.share))
            .try_fold(ExactQty::from_integer(0), |acc, s| acc.checked_add(s))
            .unwrap();
        assert_eq!(kib_sum, one);
    }

    #[test]
    fn test_ngm_2026_perimeter_change_represented() {
        let pairs = pair_registry().expect("pair registry");
        let ngm = pairs.iter().find(|p| p.asset == AssetId::Ngm).expect("NGM");
        assert_eq!(ngm.perimeter.len(), 2);
        assert_eq!(ngm.perimeter[0].evidence, "E457");
        assert_eq!(ngm.perimeter[1].evidence, "E658,E558");
        assert_eq!(
            ngm.perimeter[1].from,
            CivilDate {
                year: 2026,
                month: 8,
                day: 9
            }
        );
        assert!(ngm.perimeter[1].operations.contains(&"Fourmile"));
        assert!(ngm.perimeter[1].operations.contains(&"Mike"));
        assert!(ngm.perimeter[1].operations.contains(&"Fiberline"));

        // Pre-change 2026Q2 (ends 2026-06-30 < 2026-08-09) still resolves cleanly on E457.
        let q2_2026 = resolve_pair(
            AssetId::Ngm,
            Period {
                year: 2026,
                kind: PeriodKind::Quarter(2),
            },
        )
        .expect("2026Q2 resolves pre-vend-in");
        assert_eq!(q2_2026.perimeter_evidence, "E457");
        assert!(!q2_2026.operations.contains(&"Fourmile"));
    }

    #[test]
    fn test_pv_region_move_is_not_perimeter_change() {
        let q3_2025 = Period {
            year: 2025,
            kind: PeriodKind::Quarter(3),
        };
        let q4_2025 = Period {
            year: 2025,
            kind: PeriodKind::Quarter(4),
        };
        // E461: Barrick moved Pueblo Viejo into North America starting Q4 2025 (BD-332 Scope::Region).
        assert_eq!(
            barrick_pv_reporting_region(q3_2025),
            ("Latin America & Asia Pacific", 1)
        );
        assert_eq!(barrick_pv_reporting_region(q4_2025), ("North America", 2));
        assert_ne!(
            barrick_pv_reporting_scope(q3_2025),
            barrick_pv_reporting_scope(q4_2025)
        );
        assert!(matches!(
            barrick_pv_reporting_scope(q4_2025),
            Scope::Region { ref map_version } if map_version.contains("North America")
        ));

        // Perimeter and ownership shares remain completely unchanged across the region move.
        let r_q3 = resolve_pair(AssetId::PuebloViejo, q3_2025).expect("PV 2025Q3");
        let r_q4 = resolve_pair(AssetId::PuebloViejo, q4_2025).expect("PV 2025Q4");
        assert_eq!(r_q3.operations, r_q4.operations);
        assert_eq!(r_q3.perimeter_evidence, r_q4.perimeter_evidence);
        assert_eq!(r_q3.reporters, r_q4.reporters);

        let pairs = pair_registry().unwrap();
        let pv = pairs
            .iter()
            .find(|p| p.asset == AssetId::PuebloViejo)
            .unwrap();
        assert_eq!(pv.perimeter.len(), 1);
    }

    #[test]
    fn test_pair_registry_golden_digest() {
        let report = check_pair_registry().expect("canonical pair registry valid");
        assert_eq!(report.assets, 3);
        assert_eq!(report.perimeter_entries, 4);
        assert_eq!(report.unresolved_entries, 2);
        let hex = report.digest.to_hex();
        assert_eq!(
            hex,
            PAIR_REGISTRY_GOLDEN_SHA256,
            "pair_registry golden digest mismatch; log={}",
            report.to_json_log()
        );
        let log = report.to_json_log();
        assert!(log.contains("\"event\":\"pair_registry\""));
        assert!(log.contains("\"assets\":3"));
        assert!(log.contains("\"perimeter_entries\":4"));
        assert!(log.contains("\"unresolved_entries\":2"));
    }

    #[test]
    fn test_period_straddling_perimeter_change_refuses() {
        for kind in [
            PeriodKind::Quarter(3),
            PeriodKind::NineMonths,
            PeriodKind::FullYear,
        ] {
            let err = resolve_pair(AssetId::Ngm, Period { year: 2026, kind })
                .expect_err("2026 straddling period must refuse");
            assert_eq!(err.code, "FMINE-REF-104");
            assert!(
                err.what.contains("ngm_2026_vend_in_effective_date"),
                "refusal must name missing parameter, got: {}",
                err.what
            );
        }
    }

    #[test]
    fn test_unresolved_post_vend_share_refuses_naming_parameter() {
        // 2026Q4 falls wholly after 2026-08-09 -> refuses naming the unresolved perimeter/share parameter.
        let err_peri = resolve_pair(
            AssetId::Ngm,
            Period {
                year: 2026,
                kind: PeriodKind::Quarter(4),
            },
        )
        .expect_err("2026Q4 post-vend-in must refuse");
        assert_eq!(err_peri.code, "FMINE-REF-104");
        assert!(err_peri.what.contains("ngm_2026_vend_in_effective_date"));

        // Even once the effective date is fixed, an unresolved post-vend-in share STILL refuses
        // naming `ngm_post_vend_in_barrick_share`.
        let mut pairs = pair_registry().unwrap();
        let ngm = pairs.iter_mut().find(|p| p.asset == AssetId::Ngm).unwrap();
        ngm.perimeter[1].effective_date_parameter = None;
        let err_share = resolve_pair_in(
            &pairs,
            AssetId::Ngm,
            Period {
                year: 2026,
                kind: PeriodKind::Quarter(4),
            },
        )
        .expect_err("unresolved share must refuse");
        assert_eq!(err_share.code, "FMINE-REF-104");
        assert!(
            err_share.what.contains("ngm_post_vend_in_barrick_share"),
            "refusal must name missing share parameter, got: {}",
            err_share.what
        );
    }

    #[test]
    fn test_share_never_defaults_to_one() {
        // Unprinted basis or Consolidated100 on a JV mirror reporter MUST refuse (never default 100%).
        let mut pairs = pair_registry().unwrap();
        let pv_idx = pairs
            .iter()
            .position(|p| p.asset == AssetId::PuebloViejo)
            .unwrap();
        pairs[pv_idx].reporters[0].basis = OwnershipBasis::Unprinted;
        let err_unprinted = resolve_pair_in(
            &pairs,
            AssetId::PuebloViejo,
            Period {
                year: 2025,
                kind: PeriodKind::FullYear,
            },
        )
        .expect_err("Unprinted basis must refuse");
        assert_eq!(err_unprinted.code, "FMINE-REF-104");

        pairs[pv_idx].reporters[0].basis = OwnershipBasis::Consolidated100;
        let err_100 = resolve_pair_in(
            &pairs,
            AssetId::PuebloViejo,
            Period {
                year: 2025,
                kind: PeriodKind::FullYear,
            },
        )
        .expect_err("Consolidated100 on JV mirror reporter must refuse");
        assert_eq!(err_100.code, "FMINE-REF-104");

        // Unknown asset and out-of-range quarter refuse cleanly with FMINE-REF-107.
        assert_eq!(
            AssetId::parse("mystery_mine").unwrap_err().code,
            "FMINE-REF-107"
        );
        assert_eq!(
            resolve_pair(
                AssetId::Ngm,
                Period {
                    year: 2025,
                    kind: PeriodKind::Quarter(0),
                }
            )
            .unwrap_err()
            .code,
            "FMINE-REF-107"
        );
    }
}
