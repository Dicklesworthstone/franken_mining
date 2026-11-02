//! Mine Stats identity histories over verified canonical Claims (bd-504e).
//! Determinism: DeterministicPerIsa (the shared descriptive binomial interval uses floats).
//! This projection retains the ledger's arithmetic findings; it does not rerun or strengthen them.

use std::collections::{BTreeMap, BTreeSet};
use std::ops::Deref;

use fmine_core::hash::{Digest32, sha256};
use fmine_numerics::exact_feas::Rat;
use fmine_registry::transcriptions::{Token, parse_token};

use super::{Consistency, ConsistencyFamily, UnkeyedIdentity, finish_histories};
use crate::js;
use crate::key::mine_stats::KeyedClaims;
use crate::key::{KeyedCell, PublicValue, SeriesKey, canonical_value_tag, claims_digest};
use crate::mine_ledger::{Family, Instance, MineLedger, State};

impl ConsistencyFamily for Family {
    const ALL: &'static [Self] = &Family::ALL;
    const SCHEMA: &'static str = "fmine.issuer_consistency.mine-stats.v1";
    fn as_str(self) -> &'static str {
        self.as_str()
    }
}

/// An exact original ledger instance and its uniquely identified canonical anchor.
#[derive(Debug, Clone, PartialEq)]
pub struct Binding {
    pub source: KeyedCell,
    pub instance: Instance,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct MineStatsConsistency {
    pub identities: Consistency<Family>,
    pub bindings: Vec<Binding>,
    /// The registered label interpretation used to bind raw ledger subjects to canonical Claims.
    pub subject_labels: BTreeMap<String, String>,
    /// Every candidate used by an anchor or operand, including candidates which prevent unique binding.
    pub source_claims: Vec<KeyedCell>,
}

impl Deref for MineStatsConsistency {
    type Target = Consistency<Family>;
    fn deref(&self) -> &Self::Target {
        &self.identities
    }
}

fn raw_locator(cell: &KeyedCell) -> Option<String> {
    let (_, position) = cell.locator.split_once(':')?;
    Some(format!("{}:{position}", cell.key.reporting_vintage.release))
}

fn valid(cell: &KeyedCell) -> bool {
    cell.claim.check_id().is_ok()
        && cell.claim.metric_key == cell.key.line.label
        && cell.claim.value_tag() == canonical_value_tag(&cell.key, &cell.claim.value)
        && cell.claim.anchors.iter().any(|a| {
            Digest32(a.artifact_sha256).to_hex() == cell.key.reporting_vintage.file_sha256
                && format!(
                    "{}:{}",
                    a.artifact_path.rsplit('/').next().unwrap_or(""),
                    a.locator
                ) == cell.locator
        })
}

type Aliases<'a> = BTreeMap<String, Vec<&'a KeyedCell>>;

fn unique<'a>(aliases: &Aliases<'a>, at: &str) -> Option<&'a KeyedCell> {
    let [cell] = aliases.get(at)?.as_slice() else {
        return None;
    };
    valid(cell).then_some(*cell)
}

/// Roles preserve noncommutative operands while allowing member rows to move in additive aggregates.
fn role(family: Family, slot: usize, len: usize) -> Option<&'static str> {
    match family {
        Family::Ownership if len == 2 => ["full", "attributable"].get(slot).copied(),
        Family::QuarterSum | Family::SiteQuarterSum if len == 3 => {
            ["previous", "quarter", "total"].get(slot).copied()
        }
        Family::SiteSheet if len == 2 => ["summary", "site"].get(slot).copied(),
        Family::RegionToTotal | Family::SitesToRegion if len >= 2 => {
            Some(if slot + 1 == len { "total" } else { "member" })
        }
        Family::SoldWeightedRate if len >= 3 && len % 2 == 1 => Some(if slot + 1 == len {
            "total"
        } else if slot.is_multiple_of(2) {
            "rate"
        } else {
            "weight"
        }),
        _ => None,
    }
}

fn displayed(value: &PublicValue) -> Option<String> {
    Some(match value {
        PublicValue::Number {
            displayed: Ok(text),
            under_status,
            ..
        } => {
            let Token::Point(d) = parse_token(text).ok()? else {
                return None;
            };
            format!("number:{}:{}:{under_status:?}", d.mantissa, d.scale)
        }
        PublicValue::Nil { token, reason } => format!("nil:{reason:?}:{token}"),
        PublicValue::NotApplicable { token } => format!("not_applicable:{token}"),
        _ => return None,
    })
}

type Signature = (
    Vec<(&'static str, SeriesKey, String)>,
    Option<Rat>,
    Option<String>,
    bool,
);

fn signature(aliases: &Aliases<'_>, i: &Instance) -> Option<Signature> {
    let mut operands = Vec::new();
    for (slot, at) in i.cells.iter().enumerate() {
        let source = unique(aliases, at.as_deref()?)?;
        operands.push((
            role(i.family, slot, i.cells.len())?,
            source.key.series_key(),
            displayed(&source.claim.value)?,
        ));
    }
    if operands.is_empty() {
        return None;
    }
    operands.sort();
    Some((operands, i.share, i.printed_share.clone(), i.nil_token))
}

fn same_state(a: &Instance, b: &Instance) -> bool {
    match (&a.state, &b.state) {
        (State::Refused { reason: a, .. }, State::Refused { reason: b, .. }) => a == b,
        (a, b) => a == b,
    }
}

fn field_matches(i: &Instance, source: &KeyedCell) -> bool {
    if i.family != Family::SiteQuarterSum {
        return source.key.line.label == format!("summary:{}", i.field);
    }
    let normalize = |s: &str| {
        s.split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
            .to_lowercase()
    };
    crate::mine_ledger::SITE_QUARTER_ROWS
        .iter()
        .any(|(_, section, label, field)| {
            *field == i.field
                && source.key.line.label
                    == format!(
                        "site:{}:{}",
                        normalize(section.unwrap_or("")),
                        normalize(label)
                    )
        })
}

/// All-file counts retain every ledger instance, including unkeyed and unresolved ones. The Current/Prior
/// column group is retained in bindings but excluded from the history key: later comparatives are not new facts.
#[must_use]
pub fn assess(claims: &KeyedClaims, ledger: &MineLedger) -> MineStatsConsistency {
    let mut aliases: Aliases<'_> = BTreeMap::new();
    let mut semantic = BTreeMap::<_, BTreeSet<_>>::new();
    for cell in &claims.claims {
        if let Some(raw) = raw_locator(cell) {
            aliases.entry(raw).or_default().push(cell);
        }
        semantic
            .entry((
                cell.key.series_key(),
                cell.key.reporting_vintage.release.clone(),
            ))
            .or_default()
            .insert(cell.locator.clone());
    }
    let ambiguous: BTreeSet<_> = semantic
        .into_iter()
        .filter(|(_, at)| at.len() > 1)
        .map(|(key, _)| key)
        .collect();
    let mut report = MineStatsConsistency {
        subject_labels: claims.subject_labels.clone(),
        ..MineStatsConsistency::default()
    };
    let mut groups = BTreeMap::new();
    let mut used = BTreeSet::new();
    let mut relevant = BTreeSet::new();
    for i in &ledger.instances {
        *report
            .identities
            .all_files
            .entry((i.family, i.state.code()))
            .or_default() += 1;
        used.extend(i.cells.iter().flatten().cloned());
        used.insert(i.anchor.clone());
        let subject = claims
            .subject_labels
            .get(&i.subject)
            .map_or(i.subject.as_str(), String::as_str);
        if let Some(total) = unique(&aliases, &i.anchor).filter(|c| {
            c.key.subject == subject && c.key.valid_period == i.period && field_matches(i, c)
        }) {
            let series = total.key.series_key();
            relevant.insert(series.clone());
            groups
                .entry((series, i.family))
                .or_insert_with(Vec::new)
                .push((i, total));
            report.bindings.push(Binding {
                source: total.clone(),
                instance: i.clone(),
            });
        } else {
            let candidates = aliases
                .get(&i.anchor)
                .into_iter()
                .flatten()
                .map(|c| c.locator.as_str())
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect::<Vec<_>>()
                .join(", ");
            report.identities.unkeyed.push(UnkeyedIdentity {
                at: i.anchor.clone(), family: i.family,
                reason: format!("anchor needs one canonical Claim with the same subject, period and field; candidates: {candidates}"),
            });
        }
    }
    finish_histories(
        groups,
        &mut report.identities,
        |i| i.state.code(),
        same_state,
        |i| signature(&aliases, i),
        &ambiguous,
    );
    report
        .identities
        .unkeyed
        .sort_by(|a, b| (&a.at, a.family, &a.reason).cmp(&(&b.at, b.family, &b.reason)));
    report.bindings.sort_by_key(binding_json);
    report.source_claims = claims
        .claims
        .iter()
        .filter(|c| {
            raw_locator(c).is_some_and(|raw| used.contains(&raw))
                || (relevant.contains(&c.key.series_key())
                    && ambiguous
                        .contains(&(c.key.series_key(), c.key.reporting_vintage.release.clone())))
        })
        .cloned()
        .collect();
    report.source_claims.sort_by(|a, b| {
        (&a.key, &a.locator, a.claim.claim_id).cmp(&(&b.key, &b.locator, b.claim.claim_id))
    });
    report
}

fn binding_json(binding: &Binding) -> String {
    let i = &binding.instance;
    let reason = match &i.state {
        State::Refused { reason, .. } => Some(reason.code()),
        State::Unresolved(need) => Some(need.code()),
        _ => None,
    };
    let cells = i
        .cells
        .iter()
        .map(|a| a.as_deref().map_or_else(|| "null".into(), js))
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{{\"source\":{},\"family\":{},\"raw_subject\":{},\"period\":{},\"field\":{},\"group\":{},\"state\":{},\"reason\":{},\"raw_anchor\":{},\"cells\":[{cells}],\"share\":{},\"printed_share\":{},\"nil_token\":{},\"component\":{},\"state_detail\":{}}}",
        binding.source.to_json(),
        js(i.family.as_str()),
        js(&i.subject),
        js(&i.period.code()),
        js(&i.field),
        js(i.group.as_str()),
        js(i.state.code()),
        reason.map_or_else(|| "null".into(), js),
        js(&i.anchor),
        i.share
            .map_or_else(|| "null".into(), |n| js(&crate::mine_ledger::rat_text(n))),
        i.printed_share.as_deref().map_or_else(|| "null".into(), js),
        i.nil_token,
        i.component.as_deref().map_or_else(|| "null".into(), js),
        js(&format!("{:?}", i.state))
    )
}

impl MineStatsConsistency {
    fn body(&self) -> String {
        let mut identities = self.identities.to_json();
        identities.pop();
        let bindings = self
            .bindings
            .iter()
            .map(binding_json)
            .collect::<Vec<_>>()
            .join(",");
        let sources = self
            .source_claims
            .iter()
            .map(KeyedCell::to_json)
            .collect::<Vec<_>>()
            .join(",");
        let labels = self
            .subject_labels
            .iter()
            .map(|(label, subject)| format!("{}:{}", js(label), js(subject)))
            .collect::<Vec<_>>()
            .join(",");
        format!(
            "{identities},\"input_claims_digest\":{},\"registered_subject_labels\":{{{labels}}},\"bindings\":[{bindings}],\"source_claims\":[{sources}]}}",
            js(&claims_digest(&self.source_claims).to_hex())
        )
    }
    #[must_use]
    pub fn digest(&self) -> String {
        sha256(self.body().as_bytes()).to_hex()
    }
    #[must_use]
    pub fn to_json(&self) -> String {
        let mut body = self.body();
        body.pop();
        format!("{body},\"digest\":{}}}", js(&self.digest()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn member_order_is_commutative_but_equation_roles_are_not() {
        assert_eq!(
            role(Family::SitesToRegion, 0, 3),
            role(Family::SitesToRegion, 1, 3)
        );
        assert_ne!(
            role(Family::QuarterSum, 0, 3),
            role(Family::QuarterSum, 1, 3)
        );
        assert_ne!(
            role(Family::SoldWeightedRate, 0, 5),
            role(Family::SoldWeightedRate, 1, 5)
        );
        assert!(role(Family::Ownership, 0, 1).is_none());
    }
    #[test]
    fn displayed_precision_is_retained_but_hidden_storage_digits_are_not() {
        let number = |stored: &str, shown: &str| PublicValue::Number {
            stored: stored.into(),
            displayed: Ok(shown.into()),
            under_status: None,
        };
        assert_eq!(
            displayed(&number("100.001", "100")),
            displayed(&number("100.002", "100"))
        );
        assert_ne!(
            displayed(&number("100", "100")),
            displayed(&number("100", "100.0"))
        );
        assert_ne!(
            displayed(&number("0", "0")),
            displayed(&PublicValue::Nil {
                token: "0".into(),
                reason: crate::key::NilReason::Withheld
            })
        );
    }
}
