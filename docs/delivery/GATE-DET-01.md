# GATE-DET-01: BitwisePortable Fixture Families on Both ISAs (`BD-141`, `PO-NUM-1`)

Status: **PENDING_MACOS_CI** (not yet `Verdict: PASS`; `bd-141` stays `in_progress` until CI runs `macos-15` dev + release on the 18-family manifest)

- **Bead:** `BD-141` (`bd-141`, WP-07 Testpit)
- **Plan specification:** §29.2, §29.7 (`PO-NUM-1`, `GATE-DET-01`)
- **Pinned manifest:** `fixtures/det/bitwise_portable_families.txt` (18 families across 14 crates)
- **Test harness:** `crates/fmine-gen/src/det_families.rs` (`test_gate_det_01_all_18_bitwise_portable_families_match_pinned_manifest`)
- **CI lane:** `.github/workflows/ci.yml` step `GATE-DET-01 BitwisePortable families` across `matrix.os: [ubuntu-latest, macos-15]` × `matrix.profile: [dev, release]`

## Verified so far (2026-11-22)

- **x86_64-linux (`dev` via RCH `vmi1152480`):** 18 / 18 families computed and verified repeat-identical.
- **x86_64-linux (`release` via RCH `ovh-b`):** 18 / 18 families matched `dev` bit-for-bit (`0.24s`).
- **Families 1–8 (`fmine-core`, `fmine-numerics`, `fmine-calib`, `fmine-metrics`):** already verified bit-identical across `x86_64-linux` and `aarch64-darwin` in CI run `41237993223` (`84f32ac`).
- **Remaining before closing `BD-141`:** push to `main` and record the GitHub Actions run ID verifying all 18 families on BOTH `ubuntu-latest` (`dev` + `release`) AND `macos-15` (`dev` + `release`), then update this record to `Verdict: PASS`.
