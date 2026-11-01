//! DESIGN (BD-047, BD-068): externally timestamped existence of a selected cohort's exact prefix.
//! This checks current journal declarations under caller-selected roots, not source truth or host time.

use fmine_calib::ledger::OutcomeKind;
use fmine_calib::record::format_utc;
use fmine_io::forecast::prefix::{self, PrefixCommitment, VerifiedPrefix};

use super::{
    Assessment, Digest32, Entry, Journal, Path, Refusal, Replay, TimestampProof, absent, apply, js,
    late,
};

#[derive(Debug, Clone, Copy)]
pub(super) struct Evidence {
    verified: VerifiedPrefix,
    cohort: Digest32,
    n_members: usize,
    earliest_deadline_ns: i64,
    earliest_publication_ns: Option<i64>,
}

pub(super) fn commitment_json(commitment: PrefixCommitment) -> String {
    format!(
        "{{\"schema\":{},\"head\":{},\"sequence\":{},\"imprint_sha256\":{}}}",
        js(prefix::SCHEMA),
        js(&commitment.head.to_hex()),
        commitment.sequence,
        js(&commitment.imprint_sha256.to_hex()),
    )
}

fn strictly_before(upper_seconds: i64, boundary_ns: i64, boundary: &str) -> Result<(), Refusal> {
    // i128 preserves nanoseconds at both i64 extremes without overflow or inward rounding.
    if i128::from(upper_seconds) * 1_000_000_000 >= i128::from(boundary_ns) {
        return Err(late(format!(
            "authenticated timestamp upper bound must strictly precede {boundary} ({boundary_ns} ns)"
        )));
    }
    Ok(())
}

pub(super) fn verify(
    journal: &Journal,
    state: &Replay,
    assessment: &Assessment,
    request: &TimestampProof,
    root: &Path,
) -> Result<Evidence, Refusal> {
    let intermediates = request.intermediates.as_ref().map(|path| root.join(path));
    let verified = prefix::verify(
        journal,
        request.prefix_head,
        &root.join(&request.receipt),
        &root.join(&request.trust_roots),
        intermediates.as_deref(),
    )?;
    let mut frozen = Replay {
        lane: "supplied".into(),
        ..Replay::default()
    };
    for record in journal.records() {
        if record.sequence > verified.commitment.sequence {
            break;
        }
        if matches!(record.entry, Entry::Operation(_)) {
            apply(&mut frozen, record, journal.header(), None)?;
        }
    }
    let cohort = frozen.ledger.cohort(assessment.cohort).ok_or_else(|| {
        late("timestamped prefix does not contain the selected assessment's exact frozen cohort")
    })?;
    let mut earliest_deadline_ns = i64::MAX;
    let mut earliest_publication_ns: Option<i64> = None;
    for id in &cohort.spec.members {
        let forecast = frozen
            .ledger
            .forecast(*id)
            .ok_or_else(|| absent("timestamped cohort member is absent from its prefix"))?;
        if !frozen.ledger.outcomes(*id).is_empty() {
            return Err(late(format!(
                "timestamped prefix already records an outcome for cohort member {id}"
            )));
        }
        earliest_deadline_ns = earliest_deadline_ns.min(forecast.freeze_time_ns);
        for event in state.ledger.outcomes(*id) {
            if matches!(
                event.kind,
                OutcomeKind::Published { .. } | OutcomeKind::Restated { .. }
            ) {
                earliest_publication_ns = Some(
                    earliest_publication_ns.map_or(event.availability_time_ns, |previous| {
                        previous.min(event.availability_time_ns)
                    }),
                );
            }
        }
    }
    strictly_before(
        verified.timestamp.times.upper_utc,
        earliest_deadline_ns,
        "every frozen member's outcome deadline",
    )?;
    if let Some(publication) = earliest_publication_ns {
        strictly_before(
            verified.timestamp.times.upper_utc,
            publication,
            "every recorded publication for the selected members",
        )?;
    }
    Ok(Evidence {
        verified,
        cohort: assessment.cohort,
        n_members: cohort.spec.members.len(),
        earliest_deadline_ns,
        earliest_publication_ns,
    })
}

impl Evidence {
    pub(super) fn json(self) -> String {
        let timestamp = self.verified.timestamp;
        format!(
            "{{\"schema\":\"fmine.forecast-journal-preregistration.v1\",\"status\":\"verified\",\"trust_basis\":\"caller_selected_roots\",\"prefix\":{},\"cohort_digest\":{},\"n_members\":{},\"receipt_sha256\":{},\"trust_roots_sha256\":{},\"time_bounds\":{{\"lower_utc\":{},\"upper_utc\":{},\"lower_seconds\":{},\"upper_seconds\":{}}},\"earliest_deadline_ns\":{},\"earliest_recorded_publication_ns\":{},\"publication_scope\":\"current_journal_declarations\",\"host_clock\":\"not_verified\",\"source_truth\":\"not_verified\"}}",
            commitment_json(self.verified.commitment),
            js(&self.cohort.to_hex()),
            self.n_members,
            js(&timestamp.receipt_sha256.to_hex()),
            js(&timestamp.trust_roots_sha256.to_hex()),
            js(&format_utc(timestamp.times.lower_utc)),
            js(&format_utc(timestamp.times.upper_utc)),
            timestamp.times.lower_utc,
            timestamp.times.upper_utc,
            self.earliest_deadline_ns,
            self.earliest_publication_ns
                .map_or_else(|| "null".into(), |time| time.to_string()),
        )
    }

    pub(super) fn markdown(self) -> String {
        format!(
            "\n## External preregistration evidence\n\nRFC 3161 verification passed under caller-selected trust roots (SHA-256 `{}`). Prefix `{}` at sequence {} contains all {} frozen cohort members and none of their recorded outcomes. Its authenticated upper time bound, {}, strictly precedes their declared deadlines and every publication recorded for them in the current journal.\n\nReceipt SHA-256: `{}`. This establishes prefix existence under those roots; original host-clock times and source truth remain unverified. The historical assessment report is unchanged.\n",
            self.verified.timestamp.trust_roots_sha256,
            self.verified.commitment.head,
            self.verified.commitment.sequence,
            self.n_members,
            format_utc(self.verified.timestamp.times.upper_utc),
            self.verified.timestamp.receipt_sha256,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn authenticated_upper_bound_is_strict_at_nanosecond_boundaries_and_integer_extremes() {
        assert!(strictly_before(2, 2_000_000_001, "deadline").is_ok());
        for boundary in [1_999_999_999, 2_000_000_000] {
            assert_eq!(
                strictly_before(2, boundary, "deadline").unwrap_err().code,
                "FMINE-REF-206"
            );
        }
        assert!(strictly_before(i64::MIN, i64::MIN, "deadline").is_ok());
        assert_eq!(
            strictly_before(i64::MAX, i64::MAX, "deadline")
                .unwrap_err()
                .code,
            "FMINE-REF-206"
        );
        assert!(strictly_before(9_223_372_036, i64::MAX, "deadline").is_ok());
        assert!(strictly_before(9_223_372_037, i64::MAX, "deadline").is_err());
    }
}
