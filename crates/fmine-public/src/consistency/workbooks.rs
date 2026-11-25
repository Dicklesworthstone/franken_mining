//! DESIGN (BD-504e): compare original-quarter gold figures in the two public workbook families.
//! Exact displayed values and closed half-step rounding enclosures describe agreement, not source truth.
//! Determinism: Exact
//! Missing, ambiguous, status-qualified and incompatible assertions remain visible.

use std::collections::BTreeMap;

use fmine_core::hash::{Digest32, blake3_hash};
use fmine_numerics::exact_feas::Rat;

use crate::js;
use crate::key::{KeyedCell, OwnershipBasis, PublicValue};
use crate::sources::IssuerProfile;
use crate::{LoadedSource, aisc};

pub const LIMITATION: &str = "Original-quarter gold cost-of-sales, total-cash-cost and AISC rates per ounce sold, and attributable ounces sold, compared between the AISC reconciliation and Mine Stats summary. No comparative substitution, ownership conversion, true-source selection or causal claim. Closed half-displayed-step overlap is only a possible rounding explanation; touching endpoints do not establish a rounding convention. Missing and ambiguous counterparts stay in the population. This separately digested projection does not change identity-ledger counts or its certificate.";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkbookState {
    Equal,
    RoundingOverlap,
    Different,
    NotPublished,
    Unavailable,
}

impl WorkbookState {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Equal => "equal",
            Self::RoundingOverlap => "rounding_overlap",
            Self::Different => "different",
            Self::NotPublished => "not_published",
            Self::Unavailable => "unavailable",
        }
    }
}

/// Difference is reconciliation minus Mine Stats, in the unchanged source unit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Difference {
    pub point: Rat,
    pub lower: Rat,
    pub upper: Rat,
}

#[derive(Debug, Clone)]
pub struct BasisCitation {
    pub at: String,
    pub sha256: String,
    pub text: String,
}

#[derive(Debug, Clone)]
pub struct WorkbookComparison {
    pub issuer: String,
    pub release: String,
    pub subject: String,
    pub metric: &'static str,
    pub reconciliation: Vec<KeyedCell>,
    pub mine_stats: Vec<KeyedCell>,
    pub state: WorkbookState,
    pub reason: Option<&'static str>,
    pub difference: Option<Difference>,
    /// Only the existing gold-table bridge may supply an otherwise unprinted rate basis.
    pub confirmed_share: Option<Rat>,
    pub basis_citations: Vec<BasisCitation>,
}

#[derive(Debug, Clone, Default)]
pub struct WorkbookComparisons {
    pub rows: Vec<WorkbookComparison>,
}

fn metric(label: &str, reconciliation: bool) -> Option<&'static str> {
    match (reconciliation, label) {
        (true, "cost of sales per ounce") | (false, "summary:cos_usd_per_oz") => {
            Some("cos_usd_per_oz")
        }
        (true, "total cash costs per ounce") | (false, "summary:tcc_usd_per_oz") => {
            Some("tcc_usd_per_oz")
        }
        (true, "all-in sustaining costs per ounce") | (false, "summary:aisc_usd_per_oz") => {
            Some("aisc_usd_per_oz")
        }
        (true, "ounces sold") | (false, "summary:au_sold_attr_koz") => Some("au_sold_attr_koz"),
        _ => None,
    }
}

fn share(basis: OwnershipBasis) -> Option<Rat> {
    match basis {
        OwnershipBasis::Consolidated100 => Some(Rat::from_integer(1)),
        OwnershipBasis::Attributable { num, den, .. } if num > 0 && den > 0 && num <= den => {
            Rat::new(i128::from(num), i128::from(den)).ok()
        }
        _ => None,
    }
}

fn number(cell: &KeyedCell) -> Result<(Rat, Rat), &'static str> {
    let PublicValue::Number {
        displayed: Ok(text),
        under_status: None,
        ..
    } = &cell.claim.value
    else {
        return Err("unusable_numeric_assertion");
    };
    let digits = text.strip_prefix('-').unwrap_or(text);
    let (whole, fractional) = digits.split_once('.').unwrap_or((digits, ""));
    if whole.is_empty()
        || !whole
            .bytes()
            .chain(fractional.bytes())
            .all(|b| b.is_ascii_digit())
    {
        return Err("display_not_decimal");
    }
    let places = u32::try_from(fractional.len()).map_err(|_| "rounding_overflow")?;
    let denominator = 10_i128.checked_pow(places).ok_or("rounding_overflow")?;
    let point = Rat::from_decimal_str(text).map_err(|_| "display_not_decimal")?;
    let half_step = Rat::new(1, denominator.checked_mul(2).ok_or("rounding_overflow")?)
        .map_err(|_| "rounding_overflow")?;
    Ok((point, half_step))
}

fn confirm_rate_basis(
    profile: &IssuerProfile,
    sources: &[LoadedSource],
    row: &mut WorkbookComparison,
) {
    if !matches!(row.metric, "tcc_usd_per_oz" | "aisc_usd_per_oz") {
        return;
    }
    let ([a], [b]) = (row.reconciliation.as_slice(), row.mine_stats.as_slice()) else {
        return;
    };
    if a.key.ownership_basis != OwnershipBasis::Unprinted {
        return;
    }
    let confirmed = (|| {
        let (file, rest) = a.locator.split_once(':')?;
        let (sheet, _) = rest.rsplit_once('!')?;
        let source = sources.iter().find(|s| {
            s.file_name == file
                && Digest32(s.workbook.sha256).to_hex() == a.key.reporting_vintage.file_sha256
        })?;
        let sheet = source.workbook.sheets.iter().find(|s| s.name == sheet)?;
        let table = aisc::locate_table(sheet, &profile.subjects).ok()??;
        let basis = aisc::bridge_basis(
            profile,
            sources,
            source,
            sheet,
            &table,
            &row.subject,
            b.key.ownership_basis,
        )
        .ok()?;
        let citations = basis
            .evidence
            .iter()
            .map(|at| {
                let (file, rest) = at.split_once(':')?;
                let (sheet, cell) = rest.rsplit_once('!')?;
                let source = sources.iter().find(|s| s.file_name == file)?;
                let sheet = source.workbook.sheets.iter().find(|s| s.name == sheet)?;
                let (r, c) = fmine_io::xlsx::parse_a1_ref(cell)?;
                Some(BasisCitation {
                    at: at.clone(),
                    sha256: Digest32(source.workbook.sha256).to_hex(),
                    text: aisc::cell_text(sheet, r, c)?,
                })
            })
            .collect::<Option<Vec<_>>>()?;
        Some((share(basis.basis)?, citations))
    })();
    if let Some((share, citations)) = confirmed {
        row.confirmed_share = Some(share);
        row.basis_citations = citations;
    }
}

fn classify(row: &WorkbookComparison) -> Result<(WorkbookState, Option<Difference>), &'static str> {
    let [a] = row.reconciliation.as_slice() else {
        return Err(if row.reconciliation.is_empty() {
            "reconciliation_counterpart_missing"
        } else {
            "ambiguous_reconciliation"
        });
    };
    let [b] = row.mine_stats.as_slice() else {
        return Err(if row.mine_stats.is_empty() {
            "mine_stats_counterpart_missing"
        } else {
            "ambiguous_mine_stats"
        });
    };
    for cell in [a, b] {
        if cell.claim.check_id().is_err()
            || cell.claim.value_tag()
                != crate::key::canonical_value_tag(&cell.key, &cell.claim.value)
            || cell.claim.metric_key != cell.key.line.label
        {
            return Err("noncanonical_claim");
        }
    }
    if a.key.scope != b.key.scope {
        return Err("scope_mismatch");
    }
    let unit = if row.metric == "au_sold_attr_koz" {
        "koz"
    } else {
        "usd_per_oz"
    };
    if a.key.unit != unit || b.key.unit != unit {
        return Err("unit_mismatch");
    }
    // Two absent observations are not numeric agreement, even where the ownership label is unprinted.
    if matches!(
        (&a.claim.value, &b.claim.value),
        (PublicValue::Nil { .. }, PublicValue::Nil { .. })
            | (
                PublicValue::NotApplicable { .. },
                PublicValue::NotApplicable { .. }
            )
    ) {
        return Ok((WorkbookState::NotPublished, None));
    }
    let (Some(a_share), Some(b_share)) = (
        share(a.key.ownership_basis).or(row.confirmed_share),
        share(b.key.ownership_basis),
    ) else {
        return Err("ownership_unprinted");
    };
    if a_share != b_share {
        return Err("ownership_mismatch");
    }
    let (a, ah) = number(a)?;
    let (b, bh) = number(b)?;
    let point = a.checked_sub(b).map_err(|_| "rounding_overflow")?;
    let width = ah.checked_add(bh).map_err(|_| "rounding_overflow")?;
    let lower = point.checked_sub(width).map_err(|_| "rounding_overflow")?;
    let upper = point.checked_add(width).map_err(|_| "rounding_overflow")?;
    let zero = Rat::from_integer(0);
    let state = if point == zero {
        WorkbookState::Equal
    } else if lower.signum() <= 0 && upper.signum() >= 0 {
        WorkbookState::RoundingOverlap
    } else {
        WorkbookState::Different
    };
    Ok((
        state,
        Some(Difference {
            point,
            lower,
            upper,
        }),
    ))
}

/// Each input is the visible canonical Claim population for its named workbook family.
/// A candidate comes from either family; a missing or duplicate counterpart never disappears.
#[must_use]
pub fn compare(
    profile: &IssuerProfile,
    sources: &[LoadedSource],
    reconciliation: &[KeyedCell],
    mine_stats: &[KeyedCell],
) -> WorkbookComparisons {
    let mut groups = BTreeMap::new();
    for (is_reconciliation, cells) in [(true, reconciliation), (false, mine_stats)] {
        for cell in cells {
            let Some(metric) = metric(&cell.key.line.label, is_reconciliation) else {
                continue;
            };
            if cell.key.valid_period.code() != cell.key.reporting_vintage.release {
                continue;
            }
            let key = (
                cell.key.issuer.clone(),
                cell.key.reporting_vintage.release.clone(),
                cell.key.subject.clone(),
                metric,
            );
            let row = groups.entry(key).or_insert_with(|| WorkbookComparison {
                issuer: cell.key.issuer.clone(),
                release: cell.key.reporting_vintage.release.clone(),
                subject: cell.key.subject.clone(),
                metric,
                reconciliation: Vec::new(),
                mine_stats: Vec::new(),
                state: WorkbookState::Unavailable,
                reason: None,
                difference: None,
                confirmed_share: None,
                basis_citations: Vec::new(),
            });
            if is_reconciliation {
                row.reconciliation.push(cell.clone())
            } else {
                row.mine_stats.push(cell.clone())
            }
        }
    }
    let mut rows: Vec<_> = groups.into_values().collect();
    for row in &mut rows {
        for cells in [&mut row.reconciliation, &mut row.mine_stats] {
            cells.sort_by(|a, b| {
                (&a.key, &a.locator)
                    .cmp(&(&b.key, &b.locator))
                    .then_with(|| a.to_json().cmp(&b.to_json()))
            });
        }
        confirm_rate_basis(profile, sources, row);
        match classify(row) {
            Ok((state, difference)) => {
                row.state = state;
                row.difference = difference;
            }
            Err(reason) => row.reason = Some(reason),
        }
    }
    WorkbookComparisons { rows }
}

impl WorkbookComparisons {
    fn body_json(&self) -> String {
        let rows = self.rows.iter().map(|row| {
            let claims = |cells: &[KeyedCell]| cells.iter().map(KeyedCell::to_json).collect::<Vec<_>>().join(",");
            let difference = row.difference.as_ref().map_or_else(|| "null".into(), |d| {
                let rat = |r: Rat| js(&format!("{}/{}", r.num(), r.den()));
                format!("{{\"point\":{},\"lower\":{},\"upper\":{}}}", rat(d.point), rat(d.lower), rat(d.upper))
            });
            let share = row.confirmed_share.map_or_else(|| "null".into(), |s| js(&format!("{}/{}", s.num(), s.den())));
            let citations = row.basis_citations.iter().map(|c| format!("{{\"at\":{},\"sha256\":{},\"text\":{}}}", js(&c.at), js(&c.sha256), js(&c.text))).collect::<Vec<_>>().join(",");
            format!("{{\"issuer\":{},\"release\":{},\"period\":{},\"subject\":{},\"metric\":{},\"state\":{},\"reason\":{},\"difference\":{difference},\"confirmed_share\":{share},\"basis_citations\":[{citations}],\"reconciliation\":[{}],\"mine_stats\":[{}]}}",
                js(&row.issuer), js(&row.release), js(&row.release), js(&row.subject), js(row.metric), js(row.state.as_str()),
                row.reason.map_or_else(|| "null".into(), js), claims(&row.reconciliation), claims(&row.mine_stats))
        }).collect::<Vec<_>>().join(",");
        format!(
            "{{\"schema\":\"fmine.workbook-comparisons.v1\",\"rules_version\":\"original-quarter-gold-workbooks/1\",\"limitations\":{},\"rows\":[{rows}]}}",
            js(LIMITATION)
        )
    }

    /// Binds the complete compared population, original Claim keys and findings without changing M0AS.
    #[must_use]
    pub fn digest(&self) -> Digest32 {
        blake3_hash(self.body_json().as_bytes())
    }

    #[must_use]
    pub fn to_json(&self) -> String {
        let mut body = self.body_json();
        let digest = blake3_hash(body.as_bytes()).to_hex();
        body.pop();
        format!("{body},\"digest\":{}}}", js(&digest))
    }
}
