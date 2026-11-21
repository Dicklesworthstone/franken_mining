# CONTRACT.md — `fmine-object`

## 1. Purpose
Bedrock's S1 content-addressed object store (`FMT-CAS-1`, plan §26.5, `BD-090`, `BD-091`):
- `merkle`: fanout-16 SHA-256 Merkle tree and chunk inclusion proofs over CAS chunk digests;
- `cas`: `FMT-CAS-1` canonical manifest codec (`FMNE` + `CASM` + CRC32C, 4 MiB default chunk, N/N-1 versioning);
- `store`: root-last atomic CAS publication (`PO-BED-3`, `OB-SEAL` staging receipts);
- `gc`: fail-closed reachability garbage collection from live ledger roots (`BD-091`).

- **Crate:** `fmine-object`
- **Layer:** S1
- **Subsystem:** Bedrock
- **LOC budget:** 15,000 planned / 17,000 ceiling
- **Allowed Module Families:** `["cas", "merkle", "store", "gc"]`

## 2. Inputs
- Arbitrary byte slices and `media_type` strings (`CasStore::stage_object`, `CasStore::put_bytes`), chunk size (`1..=16 MiB`, default 4 MiB).
- Canonical `FMT-CAS-1` manifest bytes (`CasManifest::from_bytes`).
- Live root sets `BTreeSet<Digest32>` (`gc::plan_gc`).
- Dependencies: `fmine-core` and `fmine-evidence` (strictly `S1 → S0`).

## 3. Outputs
- `CasManifest`, `ChunkRef`, `StagedObject`, `Fanout16Proof`, `GcPlan`, `GcReceipt`, and reassembled verified byte slices. None is a `Claim`.

## 4. Invariants
- **Fanout-16 Merkle integrity:** `fanout16_root` and `verify_inclusion` authenticate every chunk across multi-level trees (`test_fanout16_merkle_roots_and_inclusion_proofs_across_heights`).
- **Canonical manifest framing:** `CasManifest::from_bytes` enforces contiguity, bounds, fanout-16 root equality, CRC32C, N/N-1 compatibility (`0..=1`), and single-bit flip refusal (`test_fmt_cas_1_manifest_roundtrip_n_minus_1_and_tamper_refusal`).
- **Root-last publication (`PO-BED-3`):** no manifest root becomes visible in `manifests/` until every referenced chunk is persisted, fsynced, and digest-verified on disk (`test_po_bed_3_crash_boundaries_keep_partial_roots_invisible_and_resume_cleanly`).
- **Safe reachability GC (`BD-091`):** `plan_gc` verifies every live root and its chunks before sweeping, and preserves content-addressed chunks shared between reachable and unreachable objects (`test_reachability_gc_preserves_shared_chunks_and_sweeps_unreachable_objects`).

## 5. Refusals
`CasError::{Codec, InvalidField, RootMismatch, MissingChunk, ChunkDigestMismatch, ManifestNotFound, Io}`, mapped via `CasError::to_refusal()` to `FMINE-REF-505` (integrity/digest/codec) or `FMINE-REF-506` (missing chunk/manifest or I/O).

## 6. Determinism class
- `src/merkle.rs` and `src/cas.rs` are `BitwisePortable` (integers, canonical framing, and SHA-256 only).
- `src/store.rs` and `src/gc.rs` perform atomic local filesystem I/O with deterministic sorted listings.

## 7. Evidence class of outputs
Not applicable: Bedrock CAS storage emits content-addressed bytes and proofs, not physical `Claim`s.

## 8. Validity domain
Chunk sizes `1..=16 MiB` (default 4 MiB), up to `1,048,576` chunks per manifest, `media_type` up to 256 UTF-8 bytes, schema versions `0..=1`.

## 9. Oracles
- Known-answer multi-height fanout-16 Merkle proof tests (`1..=257` chunks), 1-bit manifest tamper sweeps, crash-boundary `PO-BED-3` tests, and shared-chunk GC tests.

## 10. Known gaps
- `BD-104` chunk want/have sync anti-entropy and `BD-056` CLI/public-lane filing ingest into `CasStore` are not yet wired to `fmine-object`.
- No fuzz target for `FMT-CAS-1` yet; LabRuntime DPOR crash injection remains under `BD-101`.
- Self-verified only; no independent review or multi-ISA CI run yet.

## 11. Public API
- `merkle::{FANOUT, empty_cas_root, cas_leaf_hash, cas_internal_hash, fanout16_root, Fanout16Proof, Fanout16ProofStep, inclusion_proof, verify_inclusion}`
- `cas::{CasError, CasManifest, ChunkRef, chunk_bytes, TAG_CAS_MANIFEST, FMT_CAS_V1, DEFAULT_CHUNK_BYTES}`
- `store::{CasStore, StagedObject}`
- `gc::{GcPlan, GcReceipt, plan_gc, apply_gc}`.

## 12. Proof obligations
- `PO-BED-3` (root-last publication & crash-boundary invisibility/resumability): unit-tested in `store::tests::test_po_bed_3_crash_boundaries_keep_partial_roots_invisible_and_resume_cleanly`.
- `BD-091` (reachability GC preserving shared chunks): unit-tested in `gc::tests::test_reachability_gc_preserves_shared_chunks_and_sweeps_unreachable_objects`.
