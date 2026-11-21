# CONTRACT.md — `fmine-gen`

## 1. Purpose
Testpit T-layer synthetic-truth world generator and sealed scoring harness (`BD-127`, plan §33.3, `GOAL-025`, `GATE-GEN-01`):
- `seal`: compile-time and runtime sealed ground-truth envelope (`SealedTruth<T>`, `SealKey`, `SealError`) whose inner truth field and key constructor are inaccessible outside the scoring harness;
- `worlds`: deterministic `Philox4x32-10` synthetic mine world generators (`OpenPitAuConfig`, `OpenPitAuObservations`, `OpenPitAuTruth`, `WorldCondition` covering `CorrectlySpecified`, `Misspecified`, and `IdentifiabilityProbe` regimes);
- `harness`: `ScoringHarness` that mints the sole `SealKey`, hands engines ONLY observations, and opens the seal exclusively during post-hoc scoring to measure bias recovery, RMSE, and nominal interval coverage.

- **Crate:** `fmine-gen`
- **Layer:** T
- **Subsystem:** Testpit
- **LOC budget:** 20,000 planned / 23,000 ceiling
- **Allowed Module Families:** `["seal", "worlds", "harness"]`

## 2. Inputs
- Deterministic `seed_root: u64`, `WorldCondition` (`CorrectlySpecified`, `Misspecified`, `IdentifiabilityProbe`), and world configuration (`OpenPitAuConfig`).
- Engine predictions (`OpenPitAuPrediction`) submitted to `ScoringHarness::score_open_pit_au`.
- Dependencies: `fmine-core`, `fmine-units`, `fmine-evidence`, `fmine-claim` (`T → S0`, zero external crates). Excluded from all production closures by `xtask check-oracle` (`GATE-ORACLE-01`).

## 3. Outputs
- Separated `(OpenPitAuObservations, SealedTruth<OpenPitAuTruth>)` pairs and `ScoreCard` receipts (`coverage_80`, `bias_errors`, `truth_commitment`, `all_intervals_finite`).
- Outputs are explicitly synthetic and never enter production authority ledgers (`INV-006`).

## 4. Invariants
- **GATE-GEN-01 (sealed truth isolation):** `SealedTruth<T>` exposes no public field or unkeyed getter for its inner truth (`compile_fail,E0616` doctest), and `SealKey` cannot be constructed outside `fmine-gen` (`compile_fail,E0451` doctest). Opening a seal with another harness's `SealKey` refuses with `SealError::InvalidKey`.
- **GATE-ORACLE-01:** `fmine-gen` is listed in `xtask::oracle::ORACLE_CRATES`; any production crate linking `fmine-gen` fails `xtask check-all`.
- **Reproducibility:** identical `(seed_root, config, condition)` produces bit-identical observations and `truth_commitment` SHA-256 digests across runs (`test_sealed_truth_isolation_and_deterministic_replay`).
- **Known-answer recovery:** on `OpenPitAuWorld`, a bias-aware estimator recovers injected resource-model, dilution, and moisture biases with calibrated interval coverage (`test_scoring_harness_recovers_seeded_biases_and_coverage`).

## 5. Refusals
- `SealError::InvalidKey` → `FMINE-REF-301` (unauthorized seal opening attempt).
- `GenError::InvalidConfig` → `FMINE-REF-109` (non-positive periods/blocks, invalid bias bounds, or non-finite parameters).
- `GenError::NonIdentifiableProbe` → `FMINE-REF-104` (confounded bias pairs in `IdentifiabilityProbe` mode must refuse separation rather than emit a fabricated split).

## 6. Determinism class
`BitwisePortable` (plan §29.2): all random draws come exclusively from `fmine_core::rand::PhiloxStream` keyed by 6-tuple `RngKey` and `fmine_core::det::{exp, norm_ppf, sqrt}`, never wall clocks, OS randomness, or std transcendentals.

## 7. Evidence class of outputs
Synthetic only (`EvidenceClass::ModelledUncalibrated` / synthetic test harness); never measured or operational evidence (`INV-006`).

## 8. Validity domain
- Synthetic worlds are for known-answer, coverage, misspecification, and identifiability testing; synthetic recovery never proves a real deposit behaves like the generator (`AGENTS.md` Four Claims).

## 9. Oracles
- Ground truth sealed inside `SealedTruth<T>` and verified by `ScoringHarness` known-answer, coverage, and wrong-key refusal tests in `seal.rs`, `worlds.rs`, and `harness.rs`.

## 10. Known gaps
- Additional synthetic worlds (`BD-128` full open-pit Au, `BD-129` Cu–Au flotation, `BD-130` underground stoping, `BD-131` heap leach, `BD-132` district point process, `BD-133` multi-site company) build on this framework in subsequent WP-07 beads.

## 11. Public API
- `GenError`
- `seal::{SealError, SealKey, SealedTruth}`
- `worlds::{WorldCondition, MisspecKind, BiasVector, OpenPitAuConfig, PeriodObservation, OpenPitAuObservations, PeriodTruth, OpenPitAuTruth}`
- `harness::{ScoringHarness, IntervalPrediction, OpenPitAuPrediction, ScoreCard}`

## 12. Proof obligations
- `GATE-GEN-01` (sealed truth inaccessible outside scoring harness): `seal.rs` `compile_fail` doctests + `harness::tests::test_sealed_truth_isolation_and_deterministic_replay`.
- Known-answer & coverage recovery: `harness::tests::test_scoring_harness_recovers_seeded_biases_and_coverage`.
