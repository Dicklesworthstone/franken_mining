# GATE-TOUCH-06: Public Reconciliation Known Answers — Kibali, Pueblo Viejo 2025, and Porgera 2007–2011 (`BD-281`, `BD-154`)

Verdict: PASS

- **Beads:** `BD-281` (`bd-281`, M1 gate `GATE-TOUCH-06`), `BD-154` (`bd-154`, R2 latent stockpile states & `PO-TOUCH-3`), building on closed `BD-279` (`verbatim_table`, `G-TOUCH-6`) and `BD-280` (`compensation`, `KA-TOUCH-KIBALI-2016-2025`) — all WP-08 Touchstone
- **Claim type:** Parity (reproduces published public technical-report reconciliation tables and flags every irreproducible or cross-section-conflicting cell as a `DiscrepancyRecord`, without affiliation to or endorsement by any named issuer)
- **Plan specification:** §20.4, §20.5, §20.8, §20.10, §20.11 (`GATE-TOUCH-06`, `PO-TOUCH-3`, `G-TOUCH-1`, `G-TOUCH-3`, `G-TOUCH-6`, `E206–E212`, `E255`, `E258–E259`, `E298`, `E666`)
- **Test harnesses:**
  - `crates/fmine-verify/tests/gate_touch_06.rs`:
    - `test_gate_touch_06_kibali_pueblo_viejo_and_porgera_public_known_answers`
  - `crates/fmine-recon/tests/stockpile_known_answers.rs`:
    - `test_po_touch_3_kalman_rts_smoother_equals_batch_gaussian_posterior`
    - `test_bd154_volumetric_survey_resets_tonnage_sd_without_collapsing_grade_sd`
    - `test_bd154_mixing_model_predictive_comparison_and_sulfide_degradation`
    - `test_bd154_pueblo_viejo_2025_additions_and_cone_inference`

## Measured Gate Run (FACT)

- **Commands:**
  - `cargo clippy -p fmine-recon -p fmine-verify --all-targets -j 1 --locked -- -D warnings` (exit `0`)
  - `cargo test -p fmine-recon -j 1 --locked` (`13` + `4` = `17 passed; 0 failed`)
  - `cargo test -p fmine-verify --test gate_touch_06 -j 1 --locked -- --nocapture` (`1 passed; 0 failed`)
  - `cargo run -p xtask -j 1 --locked -- check-all` (exit `0`)
- **Host / Target:** RCH remote workers (`ovh-b` / `omarchy`, `x86_64-unknown-linux-gnu`), `dev` profile, `rustc 1.100` (`nightly-2026-08-31`)
- **Aggregate `GATE-TOUCH-06` BLAKE3 digest:** `f9e73832d5bdb2be64df91ca1b22a1224341c52039613586fb15cf853218799a`

| Fixture / Check | Ownership Basis | Evidence IDs | Observed Result | Verdict |
|---|---|---|---|---|
| Kibali 2016–2025 OP GC series (`audit_multi_year_series`) | 100% project (Barrick 45% attributable) | `E210–E212` | Persistent negative metal bias `-6.50%` (`-13/200`); masked years `2016, 2018, 2024`; partially masked `2023, 2025` | PASS |
| Kibali 2024 Table 15-27 verbatim MCF ratios & `GC Adjustment` residual row (`G-TOUCH-6`) | 100% project (Barrick 45% attributable) | `E206`, `E207` | `3` `RatioIdentityMismatch` + `1` `ResidualRowPrintedGrade` (`-31.15 g/t`, grade stored `NOT_DEFINED`) filed as `DiscrepancyRecord`s | PASS |
| Pueblo Viejo 2025 Table 15-10 verbatim rows & MCF (`95% t / 103% g / 99% oz`) | 100% project (Barrick 60% / Newmont 40% via PVD) | `E258`, `E259` | All `7` physical rows close within 1% rounding bound; MCF reproduces `95% / 103% / 99%` with `0` ratio discrepancies | PASS |
| Pueblo Viejo 2025 unreported stockpile additions, reclaim share & plant cone swing | 100% project (Barrick 60% / Newmont 40%) | `E258`, `E666` | Inferred additions `4,559.5 kt @ 1.433 g/t` (`210.1 koz`), reclaim share `70.58%`, cone swing `+70.4 kt` (`281.7 -> 352.1 kt`); naive pit→crusher ratio refused (`G-TOUCH-3`, `FMINE-REF-102`) | PASS |
| Pueblo Viejo 2025 cross-section throughput conflict (p.139 `10,868 kt` vs p.222 `10,715.3 kt`) & `E255` BSD degradation | 100% project | `E255`, `E257`, `C4 §2.5 #3` | `2` `RatioIdentityMismatch` `DiscrepancyRecord`s filed (`10,868` vs `10,715.3 kt` + grade); BSD L3 (`9.04% Stot`) recovery `74.2%` | PASS |
| Porgera 2007–2011 DOM vs OR source tables & opposite-sign hierarchical bias posteriors | 100% project (Barrick 95% at 2011-12-31) | `E298`, `C4 §2.4` | UG `+35.78%` (`≈ +36%`, posterior `+0.3529`, CI excludes 0), OP `-7.63%` (`≈ -8%`, posterior `-0.0758`, CI excludes 0); all `5` 2011 reserve rows close within 1% | PASS |
| `PO-TOUCH-3` multi-period R2 Kalman/RTS smoother vs exact joint batch Gaussian posterior (`BD-154`) | Synthetic linear-Gaussian (`6` time boundaries) | `PO-TOUCH-3`, §20.8 | Max `|mu_rts - mu_batch| < 1e-9`, max `|P_rts - P_batch| < 1e-9`, exact lag-1 delta SDs; volumetric survey resets tonnage SD (`50 -> <2.1 kt`) while preserving grade SD (`0.25 g/t`) | PASS |
| Total `DiscrepancyRecord`s filed across irreproducible public cells | — | `E206`, `E207`, `E257` | `6` immutable `DiscrepancyRecord`s filed | PASS |

## Validity Boundary

Parity checks reproduce published public technical-report figures from transcribed corpus evidence notes (`C3`, `C4`) and flag irreproducible cells without correcting or fabricating underlying site measurements. Parity with a published reconciliation table does not prove site measurements were unbiased; 3D voxel/lift-parcel stockpile provenance (`BD-312`, `StockpileVoxelTwin`) and live site pilot parity (`GATE-TOUCH-03`, `BD-206`) belong to downstream beads.
