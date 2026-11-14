//! DESIGN (BD-225, §22.1): verified forecast references for a decision and saved score retrieval.
//! The original journal prefix, complete cohort membership and frozen rules retain their identities.
//! Determinism: DeterministicPerIsa (inherited forecast replay and scoring).

use std::collections::BTreeSet;
use std::path::Path;

use fmine_core::ObjectId;

use super::{
    Digest32, Entry, Journal, Refusal, RefusalCode, Replay, absent, apply, digest_is_bound,
    invalid, late, persisted_report, replay, version_counts,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CohortReference {
    pub name: String,
    pub digest: Digest32,
    pub frozen_at_ns: i64,
    /// The earliest frozen outcome deadline, or the start of an admitted plant support.
    pub prospective_until_ns: i64,
    pub members: Vec<ObjectId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Commitments {
    pub cell_id: String,
    pub head: Digest32,
    pub sequence: u64,
    pub latest_ns: i64,
    pub cohorts: Vec<CohortReference>,
}

pub(crate) struct SavedAssessment {
    pub cohort: Digest32,
    pub digest: Digest32,
    pub report: String,
}

fn integrity(detail: &str) -> Refusal {
    Refusal::new(RefusalCode::DigestMismatch505, detail)
}

fn references(state: &Replay, names: &[String]) -> Result<Vec<CohortReference>, Refusal> {
    if names.is_empty() || names.len() > 32 {
        return Err(invalid(
            "a decision needs one through 32 frozen forecast cohorts",
        ));
    }
    let mut names = names.to_vec();
    names.sort();
    if names.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(invalid("a decision cannot repeat a forecast cohort"));
    }
    names
        .into_iter()
        .map(|name| {
            let digest = state.cohorts.get(&name).copied().ok_or_else(|| {
                absent(format!("decision forecast cohort {name:?} is not frozen"))
            })?;
            let cohort = state
                .ledger
                .cohort(digest)
                .ok_or_else(|| absent("decision forecast cohort is unavailable"))?;
            let mut prospective_until_ns = i64::MAX;
            for member in &cohort.spec.members {
                let forecast = state
                    .ledger
                    .forecast(*member)
                    .ok_or_else(|| absent("decision forecast member is unavailable"))?;
                if !state.ledger.outcomes(*member).is_empty() {
                    return Err(late(
                        "decision forecast commitments must precede every recorded member outcome",
                    ));
                }
                prospective_until_ns = prospective_until_ns.min(forecast.freeze_time_ns);
                if let Some(start) = state.plant_registration_starts.get(member) {
                    prospective_until_ns = prospective_until_ns.min(*start);
                }
            }
            Ok(CohortReference {
                name,
                digest,
                frozen_at_ns: cohort.spec.frozen_at_ns,
                prospective_until_ns,
                members: cohort.spec.members.clone(),
            })
        })
        .collect()
}

/// Keep the shared journal lock alive through the decision append. This prevents an outcome append
/// between verification of the requested final head and admission of the decision.
pub(crate) fn admit(
    path: &Path,
    head: Digest32,
    names: &[String],
) -> Result<(Journal, Commitments), Refusal> {
    let journal = Journal::open_read(path, Some(head))?;
    let state = replay(&journal)?;
    let last = journal
        .records()
        .last()
        .ok_or_else(|| absent("decision forecast journal is empty"))?;
    let commitments = Commitments {
        cell_id: journal.header().cell_id.clone(),
        head,
        sequence: last.sequence,
        latest_ns: last.time_bounds(journal.header())?.1,
        cohorts: references(&state, names)?,
    };
    Ok((journal, commitments))
}

/// Reconstruct the retained prefix first, then advance through its verified continuation. A fresh
/// same-named cohort in another history cannot substitute for the decision's original predictions.
pub(crate) fn assessments(
    path: &Path,
    head: Digest32,
    commitments: &Commitments,
    selected: &[Digest32],
) -> Result<Vec<SavedAssessment>, Refusal> {
    if selected.is_empty() || selected.len() > 64 {
        return Err(invalid("select one through 64 saved forecast assessments"));
    }
    let journal = Journal::open_read(path, Some(head))?;
    let position = usize::try_from(commitments.sequence)
        .map_err(|_| invalid("decision forecast prefix sequence exceeds capacity"))?;
    let prefix = journal
        .records()
        .get(position)
        .ok_or_else(|| absent("decision forecast prefix is absent from this history"))?;
    if journal.header().cell_id != commitments.cell_id
        || prefix.chain_head()? != commitments.head
        || prefix.time_bounds(journal.header())?.1 != commitments.latest_ns
    {
        return Err(integrity(
            "decision forecast prefix does not match the retained commitment",
        ));
    }
    let mut state = Replay {
        lane: "supplied".into(),
        ..Replay::default()
    };
    for record in &journal.records()[..=position] {
        if matches!(record.entry, Entry::Operation(_)) {
            apply(&mut state, record, journal.header(), None)?;
        }
    }
    let names = commitments
        .cohorts
        .iter()
        .map(|cohort| cohort.name.clone())
        .collect::<Vec<_>>();
    if references(&state, &names)? != commitments.cohorts {
        return Err(integrity(
            "decision forecast cohorts differ from their retained identities or complete membership",
        ));
    }
    for record in &journal.records()[position + 1..] {
        apply(&mut state, record, journal.header(), None)?;
    }
    let cohort_ids: BTreeSet<_> = commitments.cohorts.iter().map(|c| c.digest).collect();
    let mut seen = BTreeSet::new();
    let mut reports = Vec::new();
    for digest in selected {
        if !seen.insert(*digest) {
            return Err(invalid("a saved assessment cannot be selected twice"));
        }
        let assessment = persisted_report(&mut state, *digest)?;
        if !digest_is_bound(&state, *digest) {
            return Err(invalid(
                "decision review requires a saved assessment with a persisted result digest",
            ));
        }
        if !cohort_ids.contains(&assessment.cohort) {
            return Err(invalid(
                "selected assessment was not among the decision's frozen forecast commitments",
            ));
        }
        let origin = version_counts(journal.records(), &state, Some(*digest))?
            .report_origin
            .ok_or_else(|| absent("saved decision assessment has no journal origin"))?;
        let rendered = state.render_origin(
            &assessment,
            origin.checkpoint_head,
            "journal_head",
            "durable_journal",
            "operator_declared_bounded_host_clock",
            Some(origin.checkpoint_count),
        )?;
        let prefix = rendered
            .strip_suffix('}')
            .ok_or_else(|| invalid("invalid saved decision forecast report"))?;
        reports.push(SavedAssessment {
            cohort: assessment.cohort,
            digest: *digest,
            report: format!("{prefix},{}}}", origin.versions.fields()),
        });
    }
    reports.sort_by_key(|report| (report.cohort, report.digest));
    Ok(reports)
}
