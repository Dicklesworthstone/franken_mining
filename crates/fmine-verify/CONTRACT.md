# CONTRACT.md — `fmine-verify`

## 1. Purpose
Std-only offline auditor verifier (`BD-140`, `GOAL-024`, plan §33.6). Verifies certificates, CAS object bundles, MMR prefix-consistency proofs, and ledger segments offline without running any FrankenMining service or network stack:
- `FMT-CERT-1` execution certificates (`fmine_core::cert::Certificate`), including optional on-disk re-hashing of every `InputDigest` in the certificate's closure;
- `FMT-PCERT-1` three-gap mine-plan certificates (`fmine_core::plan_cert::PlanCertificate`, BD-253,
  plan §15.2). The optimizer checks its feasibility and bound witnesses when building the frame.
  The offline reader checks the encoded bound and gap arithmetic, declared sampling-set identities,
  calibration rules, all four false authority flags, and optional on-disk `--inputs-dir` hashes;
- `FMT-CAS-1` content-addressed object manifests, fanout-16 Merkle roots, and every chunk hash on disk (`PO-BED-3`);
- `SyncCert` (`FMNE` + `SCER`) RFC 6962 MMR prefix-consistency proofs (`PO-BED-2`);
- `FMT-SEG-1` append-only ledger segment hash chains, per-record CRC32C checksums, and sealed MMR peak footers.

- **Crate:** `fmine-verify`
- **Layer:** T
- **Subsystem:** Testpit
- **LOC budget:** 12,000 planned / 14,000 ceiling
- **Allowed Module Families:** `["checks"]`

## 2. Inputs
- Read-only local files or directories passed on the command line or to the library functions in `checks`:
  - `cert <file> [--inputs-dir <dir>]`: an `FMT-CERT-1` binary frame and optional directory containing the certificate's input closure;
  - `cas <cas-dir> <root-sha256-hex>`: a local `FMT-CAS-1` store directory and expected 64-hex-character Merkle root;
  - `synccert <file>`: an `FMNE` + `SCER` `SyncCert` frame;
  - `segment <file>`: an `FMT-SEG-1` ledger segment file.
- Dependencies: `fmine-core`, `fmine-evidence`, `fmine-ledger`, `fmine-object`, `fmine-sync` (strictly downward `T → S1/S0`, std-only, zero external crates).

## 3. Outputs
- Library (`checks`): typed verification receipts (`Certificate`, `CasManifest`, `SyncCert`, `SegmentVerification`) or `VerifyError`.
- Binary (`fmine-verify`): deterministic single-line JSON verdict on stdout and exit code:
  - `0` on full cryptographic verification;
  - `8` (`FMINE-REF-505`) on ANY tamper, CRC32C failure, digest mismatch, Merkle root mismatch, missing published chunk, torn tail, or MMR proof failure;
  - `5` (`FMINE-REF-506`) on missing or unreadable top-level input file/directory;
  - `2` on CLI usage error.
- Emits no `Claim`s and mutates no files.

## 4. Invariants
- Any single-bit flip in a verified `FMT-CERT-1` certificate, referenced input file, `FMT-CAS-1` manifest or chunk, `SyncCert` proof, or `FMT-SEG-1` segment causes a refusal with exit code `8` (`FMINE-REF-505`).
- Read-only: `fmine-verify` never writes, creates, repairs, or deletes any file in the verified target.
- Path traversal guard: `--inputs-dir` verification refuses any `InputDigest.locator` containing `..` or absolute path components.
- Tests (`tests/tamper_exit_8.rs`):
  - `test_fmine_verify_cert_roundtrip_and_tamper_exits_8`;
  - `test_fmine_verify_cas_root_and_chunk_tamper_exits_8`;
  - `test_fmine_verify_synccert_mmr_proof_and_tamper_exits_8`;
  - `test_fmine_verify_segment_and_tamper_exits_8`.

## 5. Refusals
- `VerifyError::Tamper` → `FMINE-REF-505` (exit `8`): CRC32C mismatch, digest mismatch, non-canonical input order, Merkle root mismatch, missing chunk under published root, torn segment tail, or invalid RFC 6962 proof.
- `VerifyError::MissingOrUnreadable` → `FMINE-REF-506` (exit `5`): requested file or CAS directory does not exist or cannot be read.
- `VerifyError::Usage` → exit `2`: malformed CLI arguments or malformed 64-character hex digest.

## 6. Determinism class
Exact over read-only input bytes: identical files and arguments always produce byte-identical JSON stdout and identical exit codes. No clock, RNG, thread, or network APIs are used.

## 7. Evidence class of outputs
Not applicable: emits cryptographic verification verdicts rather than domain `Claim`s.

## 8. Validity domain
- Single-file / single-store offline verification up to `MAX_FRAME_BYTES` (64 MiB) per frame and `MAX_SEGMENT_BYTES` (256 MiB) per segment.
- Verifies cryptographic integrity, canonical encoding, and proof consistency; does not re-execute domain engines or assert external timestamp/issuer authority.

## 9. Oracles
- Known-answer tamper tests in `tests/tamper_exit_8.rs` executing the compiled `fmine-verify` binary on valid and single-bit-flipped `FMT-CERT-1`, `FMT-CAS-1`, `SyncCert`, and `FMT-SEG-1` artifacts.
- Underlying RFC 6962 MMR (`fixtures/fmine-ledger/rfc6962_probes.txt`) and SHA-256 / CRC32C vectors in `fmine-core`.
- `tests/f7_lab.rs`: the `F7` lab test for `fmine-ledger` segments (`BD-088`), on `fmine-lab`'s `LabRuntime` (a dev-dependency). A crash at every byte of the write in flight, and seeds 0..999 that also lose sectors and cut the repair short; every state is detected, repaired with its evidence kept, and resumed to bytes identical to a crash-free segment.

## 10. Known gaps
- `FMT-POST-1` Merkle-folded posting detail verification against `franken_accounting` rows will be added once WP-13 (`fmine-fa-bridge`) lands.
- BitwisePortable full-snapshot re-execution check awaits engine runner integration in M1+.

## 11. Public API
- `checks::{VerifyError, verify_certificate_bytes, verify_certificate_file, verify_plan_cert_bytes, verify_plan_cert_file, verify_cas_root, verify_sync_cert_bytes, verify_sync_cert_file, verify_segment_file, verify_mmr_inclusion, parse_digest32_hex}`
- `run_cli(args: &[String]) -> (String, u8)`
- Binary `fmine-verify` (`cert | plancert | cas | synccert | segment`).

## 12. Proof obligations
- `PO-BED-2` (offline MMR prefix-consistency verification): `test_fmine_verify_synccert_mmr_proof_and_tamper_exits_8`.
- `PO-BED-3` (published CAS root chunk integrity): `test_fmine_verify_cas_root_and_chunk_tamper_exits_8`.
- `PO-BED-9` / `BD-140` (`FMT-CERT-1` integrity + tamper exit 8): `test_fmine_verify_cert_roundtrip_and_tamper_exits_8`, `test_fmine_verify_segment_and_tamper_exits_8`.
- `BD-253` (`FMT-PCERT-1` three-gap `PlanCertificate` offline verification + tamper/forgery exit 8): `test_fmine_verify_plancert_pit_and_pcpsp_roundtrip_and_tamper_exits_8`.
