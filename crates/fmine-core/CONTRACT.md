# CONTRACT.md — `fmine-core`

## 1. Charter & Layer
- **Crate:** `fmine-core`
- **Layer:** `S0` (Substrate)
- **Subsystem:** `Bedrock`
- **LOC Budget:** `12k planned / 14k ceiling` (`registries/loc_budget.toml`)
- **Allowed Module Families:** `["id", "codec", "hash", "det", "rand", "anchor"]`

## 2. Mathematical & Domain Formulation
Provides the foundational `BitwisePortable` primitives for the entire workspace:
- Content-addressed 256-bit BLAKE3 digests and SHA-256 hash-chain/MMR digests (`hash`).
- Elementary functions free of platform `libm` (`det`): `exp`, `expm1`, `ln`, `sin`, `cos`, `sin_cos`, `tanh`, `atan`,
  `atan2`, `erf`, `norm_cdf`, `norm_ppf`, `powi`, `hypot` and `sqrt`. Same bits on every host; not correctly rounded
  (`ln(2)` is `LN_2` exactly, `exp(1)` within 1 ulp of `E`, `ln(2π)` 1 ulp above the correctly rounded value). `erf` is
  A&S 7.1.6's positive series below 2.5 and 1 − erfc by continued fraction above, within 10 ulp of 60-digit
  references (8 measured); it replaced A&S 7.1.26, whose 1.5e-7 error put `norm_cdf(0)` off 0.5 (bd-hp2j.2).
- Counter-based `Philox4x32-10` and scrambled `Sobol/Owen` quasi-Monte Carlo sequences keyed by the 6-tuple `(seed_root, partition, domain, var_group, realization, brick)` (`rand`, `ADR-RNG-01`).
- Canonical little-endian binary framing (`FMNE` magic, bounds checks, Castagnoli CRC32C) (`codec`) and `FMT-ANCHOR-1` `LedgerAnchor` (`anchor`).

## 3. Authority, Epistemics & Representation
Owns `LedgerAnchor` (`FMT-ANCHOR-1`) and canonical object identifiers (`ObjectId`, `PrefixKind`, `§0.3`). Every `Claim`, `DecisionCard`, and projection row pins an anchor and a BLAKE3 content digest.

## 4. Public API Surface
- `id::{PrefixKind, ObjectId, IdParseError}`
- `codec::{MAGIC_FMNE, CodecError, Encoder, Decoder, crc32c}`
- `hash::{Blake3, Digest32, sha256, blake3_hash, mmr_parent}`
- `det::{exp, expm1, ln, sin, cos, sin_cos, tanh, atan, atan2, erf, norm_cdf, norm_ppf, powi, hypot, sqrt,
  golden_digest_10k}`
- `rand::{RngKey, PhiloxStream, philox4x32_10, sobol_owen_2d}`
- `anchor::{LedgerAnchor, FMT_ANCHOR_V1}`

## 5. Guards & Typed Refusals
- `IdParseError`: rejects unknown prefixes, uppercase characters, or invalid hex payloads.
- `CodecError`: rejects invalid `FMNE` magic, unsupported format versions, length-bound violations, trailing bytes, or CRC32C mismatches.

## 6. Proof Obligations (`PO-*`)
- `PO-NUM-2`: `det` golden digest over $10^4$ inputs is bit-identical across `aarch64-darwin`, `x86_64-linux`, and `wasm32`.
- `PO-BED-9`: Official BLAKE3 and Random123 `Philox4x32-10` known-answer vectors pass bit-for-bit.

## 7. Failure Modes & Deterministic Fallbacks
Corrupted binary frames or anchors fail closed with `CodecError::CrcMismatch` or `CodecError::UnsupportedVersion`; N/N-1 schema versions decode deterministically.

## 8. Gates & Kill Criteria (`GATE-*` / `KILL-*`)
- `GATE-DET-01` (cross-ISA bitwise determinism)
- `GATE-FSIM-DRIFT` (`registries/lifts.toml`)

## 9. Lifted Code & Foundation Dependencies
- `crates/fs-math/src/det.rs` @ `20463525d9775a18c1b237d2676f364f367175ab` -> `src/det.rs`
- `crates/fs-blake3/src/lib.rs` @ `20463525d9775a18c1b237d2676f364f367175ab` -> `src/hash.rs`
- `crates/fs-rand/src/philox.rs` @ `20463525d9775a18c1b237d2676f364f367175ab` -> `src/rand.rs`

Each lift is under `LicenseRef-MIT-OpenAI-Anthropic-Rider`, frankensim's licence at that commit.

## 10. Non-Goals & Scope Boundary
Contains zero domain quantities (`fmine-units`), zero civil/quad-temporal calendars (`fmine-time`), and zero I/O or async runtime dependencies (`L-2`).
