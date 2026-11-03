# CONTRACT.md — `fmine-ledger`

## 1. Purpose
The ledger is Bedrock's authority store (plan §26). Today it holds one piece:
- `mmr` (BD-083), the Merkle Mountain Range over batch digests that §26.9 A2 puts under every cell's hash-chained log. An `Mmr` keeps every complete subtree of an append-only log. It gives the root of any prefix, its peaks, and RFC 6962 inclusion and consistency proofs. `verify_inclusion` and `verify_consistency` check them with RFC 9162's algorithms (§2.1.3.2, §2.1.4.2). A leaf hashes as SHA-256(0x00 ‖ entry) and a node as `fmine_core::hash::mmr_parent`, SHA-256(0x01 ‖ left ‖ right). The peaks, bagged right to left, give RFC 6962's tree hash, so a root here is a Certificate Transparency root.

That is what `SyncCert`'s `consistency_proof` (BD-103), the offline verifier (BD-140), the segment footer's peak list (BD-082) and Merkle-folded posting detail (BD-217) build on.

- **Crate:** `fmine-ledger`
- **Layer:** S1
- **Subsystem:** Bedrock
- **LOC budget:** 24,000 planned / 28,000 ceiling
- **Allowed Module Families:** `["mmr"]`

## 2. Inputs
- **Entries**, through `Mmr::push(&[u8])`. The ledger's entries are batch digests, so a caller pushes `digest.as_bytes()`.
- **Sizes, indices, roots and proofs**, through the proof functions. A hash is a `Digest32`, so a hash of another length cannot be passed at all.
- **Nothing else.** The one dependency is `fmine-core`. It reads no files, clock or registries.

## 3. Outputs
- `leaf_hash(entry)`, `empty_root()` (SHA-256 of no bytes) and `root_from_peaks(&[Digest32])`.
- `Mmr::new`, `push(entry) -> Result<u64, MmrError>` (the entry's index), `len`, `is_empty` and `root()`.
- `Mmr::root_at(size)` and `peaks_at(size)` for any prefix, the peaks largest first.
- `Mmr::inclusion_proof(index, size)`, RFC 6962's `PATH`, nearest hash first.
- `Mmr::consistency_proof(first, second)`, RFC 6962's `PROOF`. Equal sizes give an empty proof.
- `verify_inclusion(index, size, leaf, root, proof)` and `verify_consistency(first, second, first_root, second_root, proof)`, each `Result<(), MmrError>`.

## 4. Invariants
- **Upstream's answers, all of them.** `published_probes_verify_or_refuse_as_upstream_says` runs transparency-dev/merkle's 196 inclusion and consistency probes at commit `c39fbf3` (the fixture below). Each of the 143 whose hashes are 32 bytes verifies or refuses as its `wantErr` says. Of the 53 that hold a hash of another length, 52 are refusals; that `Digest32` cannot carry one is the refusal. The 53rd, equal sizes with equal 12-byte roots and an empty proof, is run with 32-byte roots instead.
- **Proofs equal upstream's.** `generated_proofs_equal_the_published_happy_paths`: the log of upstream's eight leaf inputs generates the same proof, hash for hash, for all 10 happy paths, and the same roots.
- **Leaves, subtrees, peaks and roots are RFC 6962's.** `leaves_levels_peaks_and_roots_match_the_rfc6962_known_answers` checks the stored levels against upstream's `NodeHashes`, the peaks of every size 0 to 8 against `CompactTrees`, and the roots against `RootHashes`, both from `root()` after each push and from `root_at`.
- **Every proof up to 64 leaves.** `every_tree_up_to_64_leaves_matches_the_definitions_and_verifies` compares every root, `PATH` and `PROOF` against RFC 6962 §2.1 written straight from its definitions in the test, and verifies each. An inclusion proof takes at most ⌈log₂ n⌉ hashes. A consistency proof takes up to one more: 63 to 64 takes 7.
- **Tampering is refused.** `a_changed_hash_or_a_resized_proof_is_refused`, over every proof up to 20 leaves: a flipped bit in the leaf, either root or any proof hash, swapped roots, one hash more and one hash less.
- **Refuse, never clamp.** `sizes_outside_the_log_or_the_proof_rules_are_refused`: a size past the log, an index not below its size, a consistency proof from the empty tree, a first size past the second, equal sizes with a proof or different roots, an empty proof, and a push past `u64::MAX` entries.

## 5. Refusals
`MmrError::{Full, BeyondLog{size, len}, IndexOutOfRange{index, size}, EmptyFirst, FirstAfterSecond{first, second}, ProofTooLong, ProofTooShort, RootMismatch, FirstRootMismatch, SecondRootMismatch}`, local, with `Display` and `Error`. It has no refusal code. The offline verifier (BD-140, not yet written) is to exit 8 on a tampered proof.

`EmptyFirst` follows upstream: any log extends the empty one, so a consistency proof from size 0 proves nothing. A replica that starts empty checks the batches themselves.

## 6. Determinism class
`src/mmr.rs` is `BitwisePortable`: integers and SHA-256 only.

## 7. Evidence class of outputs
Not applicable: it emits no Claim. A verified proof shows a prefix or a member against the roots the caller supplies. Whether to trust those roots is the caller's decision.

## 8. Validity domain
- **Up to `u64::MAX` entries**, and as many as memory holds: the log keeps about two hashes per entry.
- **In memory only.** Nothing is persisted. Segment files and their peak footer are BD-082.

## 9. Oracles
- `fixtures/fmine-ledger/rfc6962_probes.txt`: transparency-dev/merkle's `testdata/inclusion` and `testdata/consistency` at commit `c39fbf350a405f68571bf4c42a6a518eb0a46a4b` (Apache-2.0), converted to lines by `scripts/oracle/ct_merkle_probes.py`, with its capture record beside it. The probes were written for upstream's Go verifier, not for this code.
- The leaf inputs, subtree hashes, peaks and roots in `src/mmr/tests.rs` are transcribed from the same commit's `testonly/constants.go`.
- The 64-leaf comparison is against RFC 6962 written out in the test by the same author as the code. It is a self-consistency check, not an independent oracle.

## 10. Known gaps
- **Most of the ledger is absent.** The charter's delta batches (BD-081), cells, leases and fencing, witnesses, the narrow combiner and segment files (BD-082) are not here. No log pushes into an `Mmr` yet, and no `SyncCert` carries a proof (BD-103).
- **The plan's proof length is one short.** §26.9 A2 says a consistency proof is ⌈log₂ n⌉ hashes. RFC 6962's take up to one more, which this crate measures (63 to 64 takes 7).
- **No subtree proofs.** Upstream also has subtree inclusion and consistency proofs (its `subtreeinclusion` and `subtreeconsistency` probes). Nothing in the plan asks for them, and they are not here.
- **No compact form.** The log keeps every subtree, so it can prove any prefix. A log that kept only its peaks could append and give roots but not proofs. Resuming from a segment footer's peaks is BD-082's.
- **Self-verified only.** No independent verifier has re-run these checks.

## 11. Public API
- `mmr::{Mmr, MmrError, leaf_hash, empty_root, root_from_peaks, verify_inclusion, verify_consistency}`.

## 12. Proof obligations
None is discharged here. BD-083's acceptance is proof known answers (Invariants), not a plan proof obligation.
