//! DESIGN (BD-332.S3): a Mine Stats inventory over the canonical public-key adapter. Reading a claim by id
//! uses the same source binding, interpretation, claim id and projection digest as an assurance run.

use std::collections::BTreeMap;

use fmine_core::hash::Digest32;

use super::{KeyedCell, NilReason, PublicValue, claims_digest, from_mine_stats};
use crate::mine_stats::{CellRefusal, MineStatsRead};
use crate::sources::{IssuerProfile, SourceKind};
use crate::{LoadedSource, js};

/// Every read cell is either a keyed Claim or a named refusal; neither branch disappears from the census.
#[derive(Debug, Clone)]
pub struct KeyedClaims {
    pub claims: Vec<KeyedCell>,
    pub refusals: Vec<CellRefusal>,
    /// Exact registered printed labels and their canonical subjects, using the adapter's first match.
    /// These are interpretation inputs; no unregistered spelling or suffix is normalized.
    pub subject_labels: BTreeMap<String, String>,
    /// The canonical digest of the successful keyed projection; refusals remain explicit in the inventory.
    pub digest: Digest32,
}

impl KeyedClaims {
    /// The assurance envelope carries a bounded-size census. Individual observations are available by id
    /// in `claims`; cross-vintage findings cite them rather than repeating the entire inventory.
    #[must_use]
    pub fn summary_json(&self) -> String {
        let mut kinds = BTreeMap::<&str, usize>::new();
        for c in &self.claims {
            *kinds.entry(value_kind(&c.claim.value)).or_default() += 1;
        }
        let counts = kinds
            .iter()
            .map(|(k, n)| format!("{}:{n}", js(k)))
            .collect::<Vec<_>>();
        let refused = self
            .refusals
            .iter()
            .map(|r| {
                format!(
                    "{{\"locator\":{},\"reason\":{}}}",
                    js(&r.locator),
                    js(&r.reason)
                )
            })
            .collect::<Vec<_>>();
        format!(
            "{{\"rules_version\":\"mine-stats-claims/1\",\"claims\":{},\"by_kind\":{{{}}},\
             \"digest\":{},\"refusals\":[{}]}}",
            self.claims.len(),
            counts.join(","),
            js(&self.digest.to_hex()),
            refused.join(","),
        )
    }
}

/// Builds Claims from already verified sources and already parsed reads, without rereading any files.
#[must_use]
pub fn build_claims(
    profile: &IssuerProfile,
    sources: &[LoadedSource],
    reads: &[(String, MineStatsRead)],
) -> KeyedClaims {
    let by_file: BTreeMap<&str, &LoadedSource> = sources
        .iter()
        .filter(|s| s.kind == SourceKind::MineStats)
        .map(|s| (s.file_name.as_str(), s))
        .collect();
    let mut claims = Vec::new();
    let mut refusals = Vec::new();
    for (file, read) in reads {
        refusals.extend(read.refusals.iter().cloned());
        for v in &read.values {
            let result = by_file
                .get(file.as_str())
                .ok_or_else(|| "read has no verified source workbook".to_string())
                .and_then(|s| {
                    from_mine_stats(profile, &s.workbook, &s.quarter, v)
                        .map_err(|reason| reason.to_string())
                });
            match result {
                Ok(c) => claims.push(c),
                Err(reason) => refusals.push(CellRefusal {
                    locator: format!("{file}:{}!{}", v.sheet, v.cell),
                    reason,
                }),
            }
        }
    }
    claims.sort_by(|a, b| (&a.key, &a.locator).cmp(&(&b.key, &b.locator)));
    refusals.sort_by(|a, b| (&a.locator, &a.reason).cmp(&(&b.locator, &b.reason)));
    let digest = claims_digest(&claims);
    let mut subject_labels = BTreeMap::new();
    for share in &profile.shares {
        subject_labels
            .entry(share.label.clone())
            .or_insert_with(|| share.subject.clone());
    }
    KeyedClaims {
        claims,
        refusals,
        subject_labels,
        digest,
    }
}

/// Reads the registered inputs and materializes their public Claims without running the identity solver.
/// This is the read path for `fmine claim show`; every source is still verified against its registry digest.
pub fn load_claims(
    profile: &IssuerProfile,
    root: &std::path::Path,
) -> Result<KeyedClaims, fmine_evidence::Refusal> {
    let sources = crate::load_registered_sources(profile, root)?;
    let vocabulary: std::collections::BTreeSet<_> =
        profile.mine_stats_labels.iter().cloned().collect();
    let mut reads = Vec::new();
    for s in sources.iter().filter(|s| s.kind == SourceKind::MineStats) {
        reads.push((
            s.file_name.clone(),
            crate::mine_stats::read_mine_stats(
                &s.workbook,
                &s.quarter,
                (!vocabulary.is_empty()).then_some(&vocabulary),
            )?,
        ));
    }
    Ok(build_claims(profile, &sources, &reads))
}

#[must_use]
pub const fn value_kind(v: &PublicValue) -> &'static str {
    match v {
        PublicValue::Number { .. } => "number",
        PublicValue::Nil {
            reason: NilReason::Unknown,
            ..
        } => "nil_unknown",
        PublicValue::Nil {
            reason: NilReason::Disposed,
            ..
        } => "nil_disposed",
        PublicValue::Nil {
            reason: NilReason::Withheld,
            ..
        } => "nil_withheld",
        PublicValue::NotApplicable { .. } => "not_applicable",
        PublicValue::Missing => "missing",
    }
}

// Cross-crate inventory checks live in xtask/tests/public_mine_stats_inventory.rs.
