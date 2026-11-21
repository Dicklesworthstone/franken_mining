//! Determinism: BitwisePortable
//! Fanout-16 SHA-256 Merkle tree for `FMT-CAS-1` content-addressed objects (`BD-090`, plan §26.5, §26.9 A2).
//!
//! Domain separation:
//! - Empty tree: `SHA-256(b"FMNE-CAS16-EMPTY")`
//! - Leaf node: `SHA-256(0x10 || chunk_sha256_32)`
//! - Internal node (1..=16 children): `SHA-256(0x11 || child_count_u8 || child_0 || ... || child_{k-1})`

use fmine_core::hash::{Digest32, sha256};

/// Branching factor for `FMT-CAS-1` CAS Merkle trees (plan §26.5).
pub const FANOUT: usize = 16;

const TAG_LEAF: u8 = 0x10;
const TAG_INTERNAL: u8 = 0x11;

/// Deterministic root of an empty chunk list.
#[must_use]
pub fn empty_cas_root() -> Digest32 {
    sha256(b"FMNE-CAS16-EMPTY")
}

/// Domain-separated leaf hash for one 32-byte chunk digest.
#[must_use]
pub fn cas_leaf_hash(chunk_digest: &Digest32) -> Digest32 {
    let mut buf = [0u8; 33];
    buf[0] = TAG_LEAF;
    buf[1..33].copy_from_slice(chunk_digest.as_bytes());
    sha256(&buf)
}

/// Domain-separated internal node hash over `1..=16` child digests.
#[must_use]
pub fn cas_internal_hash(children: &[Digest32]) -> Digest32 {
    let mut buf = Vec::with_capacity(2 + children.len() * 32);
    buf.push(TAG_INTERNAL);
    buf.push(children.len() as u8);
    for child in children {
        buf.extend_from_slice(child.as_bytes());
    }
    sha256(&buf)
}

/// Compute the fanout-16 Merkle root over a sequence of chunk SHA-256 digests (`BD-090`).
#[must_use]
pub fn fanout16_root(chunk_digests: &[Digest32]) -> Digest32 {
    if chunk_digests.is_empty() {
        return empty_cas_root();
    }
    let mut level: Vec<Digest32> = chunk_digests.iter().map(cas_leaf_hash).collect();
    while level.len() > 1 {
        let mut next = Vec::with_capacity(level.len().div_ceil(FANOUT));
        for group in level.chunks(FANOUT) {
            next.push(cas_internal_hash(group));
        }
        level = next;
    }
    level[0]
}

/// One level of a fanout-16 Merkle inclusion proof.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Fanout16ProofStep {
    /// Sibling digests to the left of the proven child in this group.
    pub left_siblings: Vec<Digest32>,
    /// Sibling digests to the right of the proven child in this group.
    pub right_siblings: Vec<Digest32>,
}

/// Inclusion proof authenticating chunk `index` against a [`fanout16_root`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Fanout16Proof {
    pub chunk_index: usize,
    pub total_chunks: usize,
    pub steps: Vec<Fanout16ProofStep>,
}

/// Generate a fanout-16 inclusion proof for `chunk_index` in `chunk_digests`.
#[must_use]
pub fn inclusion_proof(chunk_digests: &[Digest32], chunk_index: usize) -> Option<Fanout16Proof> {
    if chunk_index >= chunk_digests.len() {
        return None;
    }
    let mut level: Vec<Digest32> = chunk_digests.iter().map(cas_leaf_hash).collect();
    let mut idx = chunk_index;
    let mut steps = Vec::new();
    while level.len() > 1 {
        let group_start = (idx / FANOUT) * FANOUT;
        let group_end = (group_start + FANOUT).min(level.len());
        let pos_in_group = idx - group_start;
        let left_siblings = level[group_start..group_start + pos_in_group].to_vec();
        let right_siblings = level[group_start + pos_in_group + 1..group_end].to_vec();
        steps.push(Fanout16ProofStep {
            left_siblings,
            right_siblings,
        });

        let mut next = Vec::with_capacity(level.len().div_ceil(FANOUT));
        for group in level.chunks(FANOUT) {
            next.push(cas_internal_hash(group));
        }
        level = next;
        idx /= FANOUT;
    }
    Some(Fanout16Proof {
        chunk_index,
        total_chunks: chunk_digests.len(),
        steps,
    })
}

/// Verify a [`Fanout16Proof`] for `chunk_digest` against `expected_root`.
#[must_use]
pub fn verify_inclusion(
    chunk_digest: &Digest32,
    expected_root: &Digest32,
    proof: &Fanout16Proof,
) -> bool {
    if proof.total_chunks == 0 || proof.chunk_index >= proof.total_chunks {
        return false;
    }
    let mut expected_width = proof.total_chunks;
    let mut idx = proof.chunk_index;
    let mut acc = cas_leaf_hash(chunk_digest);
    let mut step_idx = 0usize;

    while expected_width > 1 {
        let Some(step) = proof.steps.get(step_idx) else {
            return false;
        };
        let group_start = (idx / FANOUT) * FANOUT;
        let group_len = (group_start + FANOUT).min(expected_width) - group_start;
        let pos_in_group = idx - group_start;
        if step.left_siblings.len() != pos_in_group
            || step.right_siblings.len() != group_len - 1 - pos_in_group
        {
            return false;
        }
        let mut children = Vec::with_capacity(group_len);
        children.extend_from_slice(&step.left_siblings);
        children.push(acc);
        children.extend_from_slice(&step.right_siblings);
        acc = cas_internal_hash(&children);
        expected_width = expected_width.div_ceil(FANOUT);
        idx /= FANOUT;
        step_idx += 1;
    }
    step_idx == proof.steps.len() && &acc == expected_root
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fanout16_merkle_roots_and_inclusion_proofs_across_heights() {
        assert_eq!(fanout16_root(&[]), empty_cas_root());
        for count in [1usize, 2, 15, 16, 17, 32, 33, 256, 257] {
            let leaves: Vec<Digest32> = (0..count)
                .map(|i| sha256(format!("chunk-{i}").as_bytes()))
                .collect();
            let root = fanout16_root(&leaves);
            for probe in [0, count / 2, count - 1] {
                let proof = inclusion_proof(&leaves, probe).expect("proof");
                assert!(verify_inclusion(&leaves[probe], &root, &proof));
                // Flipping one bit in the leaf must fail verification.
                let mut bad_leaf = *leaves[probe].as_bytes();
                bad_leaf[0] ^= 0x01;
                assert!(!verify_inclusion(&Digest32(bad_leaf), &root, &proof));
            }
        }
    }
}
