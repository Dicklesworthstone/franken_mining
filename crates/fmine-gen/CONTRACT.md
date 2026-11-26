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
- **Allowed Module Families:** `["seal", "worlds", "harness", "geology", "cu_au_world", "biases", "drillholes", "dispatch", "mill_balance", "fa_book", "metamorphic", "plant_twin", "pilot_power", "pilot_card", "origin_coverage", "review", "det_families", "sbc", "junior"]`

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
- Additional synthetic worlds (underground stoping, heap leach, synthetic district point process `BD-135`, multi-site company) build on this framework in subsequent WP-07 beads.

## 11. Public API
- `GenError`
- `seal::{SealError, SealKey, SealedTruth}`
- `worlds::{WorldCondition, MisspecKind, BiasVector, OpenPitAuConfig, PeriodObservation, OpenPitAuObservations, PeriodTruth, OpenPitAuTruth}`
- `harness::{ScoringHarness, IntervalPrediction, OpenPitAuPrediction, ScoreCard}`
- `geology::{AuDomain, DomainSpec, OpenPitAuGeologySpec, GeologyBlock, DomainStats, GradeTonnagePoint, OpenPitAuGeologyTruth}` (`BD-128`)
- `cu_au_world::{CuAuDomain, CuAuDomainSpec, ConcentrateCustodyTerms, CuAuFlotationSpec, CuAuBlockTruth, CuAuDomainStats, CuAuFlotationTruth}` (`BD-129`)
- `biases::{BiasFamily, BiasRegistryEntry, BiasRegistry, SeededBiasProfile, TrueOreLot, BiasedLotObservation, parse_bias_registry, apply_seeded_biases}` (`BD-130`)
- `drillholes::{DrillholeCampaignConfig, DrillholeCollar, CampaignManifestSummary, generate_collar, generate_interval_chunk, stream_campaign_summary}` (`BD-131`, `10^5` holes / `10^7` intervals)
- `dispatch::{DispatchDestination, DispatchConfig, TruckLoadRecord, DestinationRollup, DispatchCampaign, generate_dispatch_campaign}` (`BD-132`)
- `mill_balance::{MillBalanceConfig, MillPeriodObservation, MillBalanceObservations, MillPeriodTruth, MillBalanceTruth, two_product_balance}` (`BD-133`)
- `fa_book::{MiningAccount, EntrySide, PostingLine, JournalEntry, LocalAccountingBook, generate_synthetic_mining_book}` (`BD-134`)
- `metamorphic::{LotSpec, assert_split_merge_lot_invariance, canonical_set_digest, assert_permutation_digest_invariance, assert_exact_statutory_unit_roundtrips}` (`BD-143`)
- `plant_twin::{SYNTHETIC_PUBLIC_LABEL, TwinAxes, AxisRanges, SegmentBoundary, CandidateFileCostRow, PilotTwinRegistrySpec, CitedPoint, CitedSeries, QuarterAgg, DailyMicroPoint, PlantTwin, TwinFamily, parse_pilot_twins_registry}` (`BD-356.S1`, `registries/pilot_twins.toml`)
- `pilot_power::{AttributionCause, LinearBiasObs, TwoBiasPosterior, TwinCausePower, PowerEnvelope, FileLabel, LabelledCandidateFile, solve_two_bias_posterior, reject_personal_data_files, subset_identifiability_rank, estimate_twin_cause_power, estimate_power_envelope, render_power_table, greedy_select_for_twin, label_candidate_files}` (`BD-356.S2`)
- `pilot_card::{EARLY_KILL_ALTERNATE_SITES, FrozenArtifactKind, FrozenArtifact, ArtifactLedger, FindingDefinition, AnalysisPlan, FileRow, EarlyKillOutcome, PilotProtocolCard, compute_inputs_digest, compute_card_digest, evaluate_early_kill, build_pilot_protocol_card}` (`BD-356.S3`)
- `origin_coverage::{OriginCoverageReport, dense_cholesky_batch_gls, run_origin_coverage_experiment}` (`BD-352`, sealed-truth 500-replicate coverage proof & `fmine_numerics::cholesky_solve` cross-check)
- `review::{MAX_EXACT_CLOPPER_PEARSON_N, SeedClass, ReviewKey, clopper_pearson, commit, verify, generate_os_nonce}` (`BD-357.S1`)
- `det_families::{FamilyDigest, evaluate_all_bitwise_portable_families}` (`BD-141`, 18 `BitwisePortable` families pinned in `fixtures/det/bitwise_portable_families.txt`)
- `sbc::{MAX_SBC_REPLICATES, MAX_SBC_POSTERIOR_DRAWS, MAX_SBC_BINS, SbcConfig, SbcReplicate, SbcCoverageRow, SbcDiagnosis, SbcReport, NormalNormalControl, GammaPoissonControl, BetaBinomialControl, chi_square_sf, clopper_pearson_interval, evaluate_sbc_replicates, run_sbc, sample_gamma, sample_beta, sample_poisson, sample_binomial}` (`BD-144`, Talts et al. 2018 SBC rank-histogram & empirical-coverage harness)
- `junior::{SYNTHETIC_JUNIOR_LABEL, JuniorSeedClass, JuniorCampaignConfig, JuniorReleasePacket, JuniorWorldTruth, JuniorAuditOutput, ClassDetectionStats, HoleGapHypothesisScore, JuniorCampaignScorecard, JuniorScoringHarness}` (`BD-350.S5`, sealed-truth synthetic junior drill-release campaign generator & 90% Clopper–Pearson detector scorer)

## 12. Proof obligations
- `GATE-GEN-01` (sealed truth inaccessible outside scoring harness): `seal.rs` `compile_fail` doctests + `harness::tests::test_sealed_truth_isolation_and_deterministic_replay` + `junior::tests::test_engine_input_has_no_seed_labels`.
- `BD-350.S5` (synthetic junior selective-reporting detection, clean zero-flag control, grammar round-trip, and `HoleSequenceGap` hypothesis scoring): `junior::tests::test_clean_synthetic_releases_raise_zero_flags`, `junior::tests::test_each_seed_class_detected_with_rate`, `junior::tests::test_hole_gap_hypothesis_scored_against_full_release`, `junior::tests::test_rendered_release_round_trips_through_grammar`, `junior::tests::test_generator_deterministic_under_rng_key`, `junior::tests::test_engine_input_has_no_seed_labels`.
- Known-answer & coverage recovery: `harness::tests::test_scoring_harness_recovers_seeded_biases_and_coverage`.
- `BD-144` (SBC rank histograms & coverage harness): `sbc::tests::test_chi_square_sf_matches_scipy_known_answers`, `sbc::tests::test_clopper_pearson_matches_exact_review_module_and_large_n`, `sbc::tests::test_sbc_normal_normal_known_calibrated_control_passes`, `sbc::tests::test_sbc_gamma_poisson_known_calibrated_control_passes`, `sbc::tests::test_sbc_beta_binomial_known_calibrated_control_passes`, `sbc::tests::test_sbc_detects_planted_miscalibrated_controls`, `sbc::tests::test_sbc_refuses_invalid_configs_and_non_finite_replicates`, `sbc::tests::test_sbc_is_bitwise_deterministic_across_runs`.
- `BD-357.S1` (transcendental-free exact Clopper–Pearson coverage & BLAKE3 `ReviewKey` commitment): `review::tests::test_clopper_pearson_known_answers`, `review::tests::test_clopper_pearson_exact_coverage_ge_95`, `review::tests::test_clopper_pearson_simulated_coverage_10k`, `review::tests::test_review_key_commit_verify_roundtrip`, `review::tests::test_review_key_tamper_fails`.
- `BD-356.S3` (AnalysisPlan pre-registration ledger, pure `card_digest`, early-kill alternate recommendation, envelope enforcement, and `INV-002` render flags): `pilot_card::tests::test_card_digest_pure_function_of_inputs_seeds_code_rev`, `pilot_card::tests::test_card_digest_changes_with_seed`, `pilot_card::tests::test_analysis_plan_registration_refuses_after_freeze`, `pilot_card::tests::test_analysis_plan_mutation_refused`, `pilot_card::tests::test_analysis_plan_digest_stable_under_field_order`, `pilot_card::tests::test_early_kill_recommends_alternate_site_when_no_subset_reaches_bar`, `pilot_card::tests::test_card_without_envelope_rejected`, `pilot_card::tests::test_card_render_flags_false_and_labels`.
- `BD-352` (evidence-origin sealed-truth 500-replicate empirical coverage & dense `cholesky_solve` parity): `origin_coverage::tests::test_bd352_sealed_truth_coverage_500_replicates_and_cholesky_parity`.
