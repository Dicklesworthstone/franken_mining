# CONTRACT.md — `fmine-core`

## 1. Purpose
The S0 substrate every crate builds on: object identifiers, the `FMNE` binary codec, BLAKE3 and SHA-256 digests, deterministic elementary functions, counter-based random streams and the `FMT-ANCHOR-1` ledger anchor. It holds no domain quantities (`fmine-units`), no calendars (`fmine-time`), and no I/O or async runtime.

Three files are lifted from frankensim at `20463525d9775a18c1b237d2676f364f367175ab` under `LicenseRef-MIT-OpenAI-Anthropic-Rider` (`registries/lifts.toml`): `fs-math/src/det.rs` → `src/det.rs`, `fs-blake3/src/lib.rs` → `src/hash.rs`, `fs-rand/src/philox.rs` → `src/rand.rs`.

- **Crate:** `fmine-core`
- **Layer:** S0
- **Subsystem:** Bedrock
- **LOC budget:** 12,000 planned / 14,000 ceiling
- **Allowed Module Families:** `["id", "codec", "hash", "det", "rand", "anchor"]`

## 2. Inputs
Byte slices (hashing, frame decoding), ID strings (`ObjectId` parsing), `f64` arguments (`det`), `RngKey` 6-tuples `(seed_root, partition, domain, var_group, realization, brick)` (`rand`), and `LedgerAnchor` fields. It reads no files and no registries.

## 3. Outputs
`Digest32` (BLAKE3, SHA-256, `mmr_parent`), `ObjectId` and its canonical text, `FMNE` frames (`Encoder::finish`), `f64` results, Philox blocks and Sobol/Owen points, and `LedgerAnchor` frames. None is a Claim.

## 4. Invariants
- A frame decodes only with its magic, tag, a supported version (N or N-1), in-bounds lengths, no trailing bytes and a matching CRC32C: `test_frame_roundtrip_and_tamper_detection`, `test_crc32c_iscsi_known_answer`.
- ID text and bytes round-trip for all 16 prefixes, in a fixed order: `test_all_prefixes_roundtrip_and_order`, `test_id_display_and_parse_property_seeded` (10,000 seeded rounds).
- A version-0 (N-1) anchor still decodes: `test_anchor_v1_and_n_minus_1_roundtrip`.

## 5. Refusals
No `FMINE-REF-nnn` code is emitted here; callers map these typed errors.
- `IdParseError`: `UnknownPrefix`, `InvalidHexLength`, `InvalidHexChar` (`src/id.rs:111`); `test_invalid_ids_refused`.
- `CodecError`: `InvalidMagic`, `UnexpectedTag`, `UnsupportedVersion`, `UnexpectedEof`, `LengthOutOfBounds`, `NonCanonicalOrder`, `TrailingBytes`, `CrcMismatch` (`src/codec.rs:14`). Only `CrcMismatch` has a test of its own.

## 6. Determinism class
All seven files declare `//! Determinism: BitwisePortable`: `lib.rs`, `anchor.rs`, `codec.rs`, `det.rs`, `hash.rs`, `id.rs`, `rand.rs`. `xtask check-det` checks that none calls a std transcendental (PO-NUM-2); `det.rs` is its one exemption (`DET_EXEMPT`), being the replacement for them.

## 7. Evidence class of outputs
Not applicable: the crate emits digests, identifiers and numbers, not Claims, so it assigns no `EvidenceClass` or `KnowledgeState`.

## 8. Validity domain
- `det` is deterministic, not correctly rounded: `ln(2)` is `LN_2` exactly (`test_det_ln_2_equals_ln_2_constant`), `exp(1)` is within 1 ulp of `E` (`test_det_exp_1_within_one_ulp`), and `erf` is within 10 ulp of 60-digit references (`test_det_erf_within_ten_ulp_of_reference`).
- `norm_ppf(p)` is finite only for `p` in (0, 1). It returns NaN outside [0, 1] and ±∞ at 0 and 1, rather than refusing (`src/det.rs:316`); `test_norm_cdf_and_ppf_roundtrip` covers the round trip.
- Decoded frames are capped at `MAX_FRAME_BYTES` (64 MiB).

## 9. Oracles
- Official BLAKE3 test vectors: `test_official_blake3_vectors`; FIPS 180 SHA-256 vectors: `test_official_sha256_vectors`; the iSCSI CRC32C check value: `test_crc32c_iscsi_known_answer`.
- Random123 `Philox4x32-10` known answers: `test_philox4x32_known_answers`. The vectors' source was not re-checked (BD-323.S2 note on BD-020).
- `det` golden digest over 10^4 inputs, pinned in `test_golden_hash_10k_inputs` (`927a7106…1684`). CI runs it on x86_64-linux and aarch64-darwin; wasm32 is not run.

## 10. Known gaps
- BD-017 (bd-017): the codec has coverage-guided fuzz targets (`fuzz/fuzz_targets/codec.rs`, `anchor.rs` and `journal.rs`, run by `scripts/fuzz.sh`), but no 24 h run and no record of one yet.
- `Encoder::put_bytes` writes `u32::MAX` as the length of a slice longer than `u32::MAX` bytes, then the whole slice. That clamps instead of refusing: the length no longer matches the bytes that follow. No caller comes near 4 GiB, and the decoder's 64 MiB cap would refuse the frame, but the encoder should refuse it itself.
- BD-016, BD-018, BD-019, BD-020 (bd-016, bd-018, bd-019, bd-020): the code and tests exist, but each stays open behind the GATE-LAYERS-00 chain (bd-016 waits on bd-015, which waits on bd-012). BD-021 (bd-021) also waits on BD-017.
- No wasm32 run of the `det` golden digest (PO-NUM-2's third target).
- Seven of the eight `CodecError` variants have no test of their own.

## 11. Public API
- `id::{PrefixKind, ObjectId, IdParseError}`
- `codec::{MAGIC_FMNE, MAX_FRAME_BYTES, CodecError, Encoder, Decoder, crc32c}`
- `hash::{Blake3, Digest32, blake3_hash, sha256, mmr_parent}`
- `det::{exp, expm1, ln, sin, cos, sin_cos, tanh, atan, atan2, erf, norm_cdf, norm_ppf, powi, hypot, sqrt, golden_digest_10k}`
- `rand::{RngKey, PhiloxStream, philox4x32_10, sobol_owen_2d}`
- `anchor::{LedgerAnchor, SourceAnchor, FMT_ANCHOR_V1}`

## 12. Proof obligations
- PO-NUM-2 (no std transcendental in a BitwisePortable module): `xtask check-det` in `check-all`; the pinned digest `test_golden_hash_10k_inputs`.
- PO-BED-9 (same inputs, seed and revision give the same digest): `test_golden_hash_10k_inputs`, `test_philox4x32_known_answers`, `test_sobol_owen_stratification`.
