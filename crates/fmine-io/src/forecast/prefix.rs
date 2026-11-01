//! DESIGN (BD-068): timestamp commitments to an exact, retained forecast-journal prefix.
//! Determinism: BitwisePortable
//! The SHA-256 imprint binds a versioned frame containing the BLAKE3
//! chain head and sequence; verifying it does not authenticate host-clock times or forecast semantics.

use std::path::Path;

use fmine_claim::forecast_journal::Record;
use fmine_core::hash::{Digest32, sha256};
use fmine_evidence::{Refusal, RefusalCode};

use super::Journal;
use super::timestamp::{VerifiedTimestamp, verify_rfc3161_with_intermediates};

pub const SCHEMA: &str = "fmine.forecast-journal-prefix.v1";

/// SHA-256 of the exact ASCII frame `SCHEMA\nhead=<lowercase hex>\nsequence=<decimal>\n`.
/// The BLAKE3 head already commits to genesis and every preceding canonical record.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PrefixCommitment {
    pub head: Digest32,
    pub sequence: u64,
    pub imprint_sha256: Digest32,
}

fn frame(head: Digest32, sequence: u64) -> String {
    format!("{SCHEMA}\nhead={head}\nsequence={sequence}\n")
}

impl PrefixCommitment {
    fn from_parts(head: Digest32, sequence: u64) -> Self {
        Self {
            head,
            sequence,
            imprint_sha256: sha256(frame(head, sequence).as_bytes()),
        }
    }

    /// Prepare a commitment from canonical record bytes, including before an append has synced.
    /// This alone asserts neither membership in a validated journal nor durable publication.
    pub fn for_record(record: &Record) -> Result<Self, Refusal> {
        Ok(Self::from_parts(record.chain_head()?, record.sequence))
    }
}

/// A receipt authenticated under the explicitly supplied roots for a prefix in the retained snapshot.
/// Cohort membership, absence of outcomes, and preregistration deadlines are checked by the caller.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VerifiedPrefix {
    pub commitment: PrefixCommitment,
    pub timestamp: VerifiedTimestamp,
}

/// Find an exact prefix in the journal's validated, locked snapshot and derive its sequence internally.
/// A valid earlier prefix is allowed; this does not establish that the final head is independently pinned.
pub fn commitment(journal: &Journal, prefix_head: Digest32) -> Result<PrefixCommitment, Refusal> {
    if journal.poisoned {
        return Err(Refusal::new(
            RefusalCode::StaleAnchor502,
            "forecast prefix commitment requires a healthy journal snapshot",
        ));
    }
    for record in journal.records() {
        let head = record.chain_head()?;
        if head == prefix_head {
            return Ok(PrefixCommitment::from_parts(head, record.sequence));
        }
    }
    Err(Refusal::new(
        RefusalCode::StaleAnchor502,
        format!("forecast prefix head {prefix_head} is absent from the retained journal"),
    ))
}

/// Verify an RFC 3161 receipt for an exact retained prefix, without reading a clock or writing a record.
/// Neither a caller-supplied sequence nor a precomputed imprint can replace the journal-derived values.
pub fn verify(
    journal: &Journal,
    prefix_head: Digest32,
    receipt: &Path,
    roots: &Path,
    intermediates: Option<&Path>,
) -> Result<VerifiedPrefix, Refusal> {
    let commitment = commitment(journal, prefix_head)?;
    let timestamp = verify_rfc3161_with_intermediates(
        receipt,
        roots,
        commitment.imprint_sha256,
        intermediates,
    )?;
    Ok(VerifiedPrefix {
        commitment,
        timestamp,
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use fmine_claim::forecast_journal::Entry;

    #[test]
    fn canonical_frame_has_an_independent_sha256_known_answer() {
        let head = Digest32(std::array::from_fn(|index| index as u8));
        assert_eq!(
            frame(head, 42),
            "fmine.forecast-journal-prefix.v1\nhead=000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f\nsequence=42\n"
        );
        // Independently calculated from these literal ASCII frames with Python hashlib.sha256.
        assert_eq!(
            PrefixCommitment::from_parts(head, 42)
                .imprint_sha256
                .to_hex(),
            "778224d5fb07dbda4877139b5830b3dbe7a5d80f277d86d89f575898e6db7cc8"
        );
        assert_eq!(
            PrefixCommitment::from_parts(head, 43)
                .imprint_sha256
                .to_hex(),
            "b777295e2d81966dec7e8acf7fee8f5249131ca10bbc15e4cfcd8f7f124ae219"
        );
        let mut changed_head = head;
        changed_head.0[0] = 0xff;
        assert_eq!(
            PrefixCommitment::from_parts(changed_head, 42)
                .imprint_sha256
                .to_hex(),
            "bdc96c941dbed2673ac96c4d2e1f9889461f0cb815d06ee9d58936cc161faa75"
        );
        assert_eq!(
            PrefixCommitment::from_parts(head, u64::MAX)
                .imprint_sha256
                .to_hex(),
            "7628b50e4953d6ddfc441c72bad3ea62d0933565e3c4986cc315a6800c840352"
        );
    }

    #[test]
    fn staged_commitment_binds_record_bytes_and_rejects_invalid_frames() {
        let record = Record {
            sequence: 42,
            previous: Digest32([7; 32]),
            observed_ns: 100,
            entry: Entry::Operation("kind = \"synthetic\"\n".into()),
        };
        let original = PrefixCommitment::for_record(&record).expect("canonical operation");
        assert_eq!(original.sequence, 42);
        assert_eq!(original.head, record.chain_head().unwrap());
        for mutation in 0..4 {
            let mut changed = record.clone();
            match mutation {
                0 => changed.sequence += 1,
                1 => changed.previous.0[0] ^= 1,
                2 => changed.observed_ns += 1,
                _ => changed.entry = Entry::Operation("kind = \"changed\"\n".into()),
            }
            let altered = PrefixCommitment::for_record(&changed).expect("changed operation");
            assert_ne!(altered.head, original.head);
            assert_ne!(altered.imprint_sha256, original.imprint_sha256);
        }
        let mut invalid = record;
        invalid.sequence = 0;
        assert!(PrefixCommitment::for_record(&invalid).is_err());
    }
}
