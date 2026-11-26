# GATE-DET-01: BitwisePortable Fixture Families on Both ISAs (`BD-141`, `PO-NUM-1`)

Verdict: PASS

- **Bead:** `BD-141` (`bd-141`, WP-07 Testpit)
- **Claim type:** Parity (cross-ISA and cross-profile determinism across `x86_64-unknown-linux-gnu` and `aarch64-apple-darwin`, `dev` and `release`)
- **Plan specification:** §29.2, §29.7 (`PO-NUM-1`, `GATE-DET-01`)
- **Pinned manifest:** `fixtures/det/bitwise_portable_families.txt` (18 families across 14 crates)
- **Test harness:** `crates/fmine-gen/src/det_families.rs` (`test_gate_det_01_all_18_bitwise_portable_families_match_pinned_manifest`)
- **CI workflow:** `.github/workflows/ci.yml` step `GATE-DET-01 BitwisePortable families` across `matrix.os: [ubuntu-latest, macos-15]` × `matrix.profile: [dev, release]`

## Hosted Cross-ISA Verification Receipt (`ci:45671436272@26c065c`)

All four hosted matrix legs on GitHub Actions CI run `45671436272` (commit `26c065cb851b4f461a3e37b4d2db336c8adfd650`, with zero changes to `crates/fmine-gen/src/det_families.rs` or `fixtures/det/bitwise_portable_families.txt` through `HEAD`) executed `GATE-DET-01 BitwisePortable families` and emitted 18/18 byte-identical JSON digest lines matching `fixtures/det/bitwise_portable_families.txt`:

| Matrix Leg | Target Architecture | Profile | Job ID | Step Verdict | Families Matched |
|---|---|---|---|---|---|
| `ubuntu-latest (dev)` | `x86_64-unknown-linux-gnu` | `dev` | `139407479562` | `success` | 18 / 18 |
| `ubuntu-latest (release)` | `x86_64-unknown-linux-gnu` | `release` | `139407479403` | `success` | 18 / 18 |
| `macos-15 (dev)` | `aarch64-apple-darwin` | `dev` | `139407479448` | `success` | 18 / 18 |
| `macos-15 (release)` | `aarch64-apple-darwin` | `release` | `139407479492` | `success` | 18 / 18 |

Cross-leg verification summary (parsed from the four hosted job logs on 2026-11-25):
- All 4 legs identical: `true` (18/18 JSON family digest lines match byte-for-byte across `ubuntu-latest` and `macos-15`, `dev` and `release`).
- Pinned manifest match: `true` (all 18 `(family, digest)` pairs equal `fixtures/det/bitwise_portable_families.txt`).

## Prior Local & Partial Verification (2026-11-22)

- **x86_64-linux (`dev` via RCH `vmi1152480`):** 18 / 18 families computed and verified repeat-identical.
- **x86_64-linux (`release` via RCH `ovh-b`):** 18 / 18 families matched `dev` bit-for-bit (`0.24s`).
- **Families 1–8 (`fmine-core`, `fmine-numerics`, `fmine-calib`, `fmine-metrics`):** previously verified bit-identical across `x86_64-linux` and `aarch64-darwin` in CI run `41237993223` (`84f32ac`).
