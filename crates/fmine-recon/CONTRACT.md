# CONTRACT.md — `fmine-recon`

## 1. Purpose
Touchstone reconciliation kernel (`BD-149`, `BD-150`, `BD-151`, `BD-152`, `BD-153`, `BD-156`; plan §20.1–§20.12). Implements R1 linear weighted-least-squares (WLS) material-balance reconciliation over `fmine-flow::FlowGraph` topologies with full measurement covariance and Joseph-form posterior covariance (`fmine-numerics::reconciliation`), observable unmeasured stream/inventory inference (`fmine-numerics::reconciliation::partial`), global chi-square and per-measurement normalized-adjustment gross-error tests with exact theoretical detection power curves and quarterly Minimum Detectable Bias (MDB) publication, bilinear tonnes × grade metallurgical reconciliation over extensive `[dry_mass, elemental_metal]` state vectors (`fmine-numerics::reconciliation::nonlinear`), exact hierarchical per-node / per-source bias posteriors with partial pooling and SBC posterior sampling, observation-operator rank/nullspace identifiability analysis with Measurement VOI ranking, and canonical `ReconciliationCertificate` (`FMNE` + `RCRT` + `FMT-CERT-1` bridge) with `G-TOUCH-1`..`G-TOUCH-5` and `INV-002` guards.

- **Crate:** `fmine-recon`
- **Layer:** K2
- **Subsystem:** Touchstone
- **LOC budget:** 18,000 planned / 21,000 ceiling
- **Allowed Module Families:** `["bias", "bilinear", "certificate", "error", "gross_error", "identifiability", "linear", "stats"]`

## 2. Inputs
- Validated `fmine_flow::graph::FlowGraph` topologies and typed domain requests (`LinearFlowRequest`, `NamedLinearRequest`, `PartialFlowRequest`, `BiasIdentifiabilityRequest`, `GrossErrorPolicy`, `BiasVectorSpec`, `BilinearFlowRequest`, `DirectGradeReconciliationAttempt`, `HierarchicalBiasRequest`, `PosteriorSampleRequest`, `FactorMultiplierAttempt`, `IdentifiabilityRequest`, `MeasurementVoiRequest`, `ReconciliationCertificateRequest`).
- Zero bare `f64` parameters in public functions (`L-6`).
- Strictly downward dependencies: `fmine-claim`, `fmine-core`, `fmine-evidence` (`S0`), `fmine-numerics` (`K0`), `fmine-flow` (`K1`). Zero external crates; zero I/O, network, or ambient clock APIs (`L-3`).

## 3. Outputs
- `linear::LinearReconResult` and `linear::PartialReconResult`: reconciled extensive estimates, adjustments, full prior/posterior/adjustment covariances, raw and reconciled residuals, max relative closure error (`PO-TOUCH-1`), global `chi_square`, degrees of freedom, normalized adjustments, `|corr| > 0.8` high-correlation diagnostics, and BLAKE3 determinism digest (`PO-TOUCH-6`).
- `gross_error::GrossErrorReport`, `BiasPowerAnalysis`, and `NodeDetectableBias`: global chi-square p-value and critical value, family-wise-corrected per-measurement suspect ranking, commit disposition (`Admit` vs `RefuseForInvestigation` -> `FMINE-REF-101`), exact non-central chi-square and normal two-sided theoretical power for any candidate bias vector, and single-period / quarterly Minimum Detectable Bias per redundant node (§20.6, `BD-159`).
- `bilinear::BilinearReconResult`: reconciled extensive dry masses and elemental metal masses for all circuit streams, reconstructed full local covariance, post-reconciliation derived grades with first-order delta-method uncertainty, node mass pulls and elemental recoveries, constraint closure audit, and BLAKE3 digest.
- `bias::HierarchicalBiasResult`, `BiasUpdateEvent`, and `HierarchicalPosteriorDraws`: exact partially-pooled group and node bias posteriors, full joint covariance, `REFCLASS-` provenance linkage (`BD-050`), surprise-monitor `InvestigationCase` routing safeguard (§20.5), and deterministic Cholesky posterior samples for SBC (`BD-152`).
- `identifiability::IdentifiabilityReport` and `MeasurementVoiScore`: standardized observation-operator rank, orthonormal `row(H)` determined contrast basis, orthonormal `ker(H)` confounded directions, `FMINE-REF-104` refusal guards, and exact Gaussian linear-loss Measurement VOI ranking (§20.6, `BD-153`).
- `certificate::ReconciliationCertificate`: self-verifying `FMNE` + `RCRT` binary frame covering all 8 plan §20.9 rows, hard-coded `INV-002` false non-claim flags, `G-TOUCH-4` definition-version validation, and `to_fmt_cert_1` conversion verified offline by `fmine-verify` (`BD-156`).

## 4. Invariants
- **Reconcile extensive quantities only (`G-TOUCH-1`, §20.2):** direct grade reconciliation is refused with `FMINE-REF-103`; all balance constraints `A x = b` act strictly on extensive dry mass, water mass, and elemental metal masses, with grades derived post-reconciliation (`metal / dry_mass`).
- **Exact balance closure (`PO-TOUCH-1`, §20.11):** every reconciled linear or bilinear estimate satisfies `|A x̂ - b| <= 1e-9` relative (audited fail-closed; zero plugs).
- **Identifiability refusal (`G-TOUCH-2`, §20.6):** rank-deficient observation operators (`rank(H) < k`) or parameter pairs with `|posterior_corr| > 0.80` refuse individual attribution with `FMINE-REF-104` while publishing the exact determined contrasts and separating-measurement VOI.
- **Stockpile / inventory decoupling guard (`G-TOUCH-3`, §20.1):** interior stockpile or inventory-holding nodes (`NodeClass::StockpileOrPad || holds_inventory`) cannot silently assume zero inventory delta in linear or bilinear balances; missing inventory deltas refuse (`FMINE-REF-102`).
- **F-factor definition version & interval guard (`G-TOUCH-4`, §20.4):** `ReconciliationCertificate::build` refuses any F-factor missing its `registry_key`, `definition_version`, or finite credible interval (`FMINE-REF-107`).
- **No factor multipliers (`G-TOUCH-5`, §20.5):** direct factor-multiplier application is refused (`FMINE-REF-109`); bias updates flow as logged hierarchical prior events with evidence cuts, routed to an `InvestigationCase` whenever a surprise alarm is active.
- **Commit-time gross-error gate (`INV-005`, §20.2):** `enforce_commit_gate` and committed `ReconciliationCertificate` builds refuse periods failing gross-error thresholds with `FMINE-REF-101`.
- **Non-claim certificate flags (`INV-002`):** `reserve_disclosure_claim`, `execution_authorized`, `geotechnical_signoff`, and `regulatory_compliance_claim` are always `false` and verified zero on decode.

## 5. Refusals
- `ReconError::ConservationGrossError` -> `FMINE-REF-101` (commit refused on gross-error failure).
- `ReconError::StockpileDecouplingRefused` -> `FMINE-REF-102` (`G-TOUCH-3` stockpile/inventory decoupling violation).
- `ReconError::DirectGradeReconciliationRefused` -> `FMINE-REF-103` (`G-TOUCH-1` direct grade reconciliation attempt).
- `ReconError::NonIdentifiable` -> `FMINE-REF-104` (`G-TOUCH-2` rank-deficient or `|corr| > 0.8` attribution refusal, or unobservable unmeasured stream).
- `ReconError::MissingDefinitionVersion` -> `FMINE-REF-107` (`G-TOUCH-4` F-factor without definition version/interval).
- `ReconError::FactorMultiplierRefused` / `ReconError::InvalidParameter` -> `FMINE-REF-109` (`G-TOUCH-5` multiplier attempt or out-of-domain parameters).
- `ReconError::InsufficientRedundancy` -> `FMINE-REF-201` (zero interior balance equations or zero degrees of freedom).
- `ReconError::NumericalFailure` / `ReconError::UpstreamRefusal` -> `FMINE-REF-207` (or preserved upstream refusal code).
- `ReconError::CertificateTampered` -> `FMINE-REF-505` (tampered RCRT frame, digest mismatch, or non-zero `INV-002` flag).

## 6. Determinism class
`DeterministicPerIsa` (plan §29.2): delegates linear/partial/nonlinear LDLᵀ solves to `fmine-numerics::reconciliation` (`DeterministicPerIsa`); special functions in `stats` and Box–Muller `PhiloxStream` draws use `fmine_core::det` (`BitwisePortable`); canonical `BTreeMap`/`BTreeSet` parameter ordering and SHA-256 / BLAKE3 certificate digests ensure replay stability on the same ISA (`PO-TOUCH-6`).

## 7. Evidence class of outputs
Derived state estimates, hierarchical bias posteriors, and statistical gross-error / VOI diagnostics conditioned on declared measurement error models. Never asserts compliance, never overwrites raw L0 observations, never promotes prior-dominated splits into attributable findings (`INV-006`), and never signs human attestations (`INV-002`).

## 8. Validity domain
- Bounded to `fmine-numerics::reconciliation` limits (`<= 128` variables/observations, `<= 64` independent linear constraints) and `<= 128` hierarchical bias parameters (`<= 64` identifiability parameters).
- Exact hierarchical bias posteriors, SBC draws, chi-square p-values, and analytical power curves assume linear-Gaussian measurement error models under declared SPD covariances.
- Bilinear local covariances are Gauss–Newton tangent approximations (`NonlinearReconciliation`), valid locally around interior solutions (`M_s > 0`, `0 < m_{s,e} < M_s`).

## 9. Oracles
- Plan §20.12 `GB-24` and `GB-25` closed-form WLS and Bayesian identifiability oracles.
- AMIRA P754 single-node and 2-node rougher–cleaner mass & metal balance oracles + `PO-TOUCH-2` Monte Carlo posterior covariance verification.
- Exact analytical vs Monte Carlo empirical gross-error power curves (`BD-150`, `BD-159` quarterly 5% sampler bias power).
- Exact two-product concentrator formula + `fmine-gen::mill_balance` Cu–Au concentrator verification (`BD-151`, `GATE-TOUCH-01`).
- Exact hierarchical partial-pooling shrinkage oracle + Talts et al. (2018) SBC uniformity & `90%` empirical coverage (`85–95%`) across `>= 200` replicates over all `BD-130` bias families (`BD-152`, `BD-153`, `GATE-TOUCH-02`).
- `FMT-CERT-1` offline round-trip & single-bit tamper exit-8 verification via `fmine-verify` (`BD-156`).

## 10. Known gaps
- R2 multi-period dynamic stockpile voxel/Kalman state-space smoothing (`BD-154`, `BD-312`) and rolling-origin hindcast CLI (`BD-161`, `BD-162`) remain in subsequent Touchstone beads.
- Public reconciliation table known answers (`GATE-TOUCH-06`, `BD-281`) and live site pilot parity (`GATE-TOUCH-03`, `BD-206`) remain downstream.

## 11. Public API
- `ReconError` (`error::ReconError`)
- `linear::{IDENTIFIABILITY_CORRELATION_LIMIT, PO_TOUCH_1_RELATIVE_TOLERANCE, ExtensiveChannel, VariableKey, MeasuredVariable, CovarianceOffDiagonal, DirectGradeReconciliationAttempt, refuse_direct_grade_reconciliation, LinearFlowRequest, NamedLinearConstraint, NamedLinearRequest, CorrelatedVariablePair, LinearReconResult, PartialFlowRequest, PartialReconResult, BiasPriorSpec, BiasLinearObservation, BiasIdentifiabilityRequest, BiasPosteriorEstimate, ContrastEstimate, BiasIdentifiabilityReport, reconcile_flow_graph, reconcile_named_system, reconcile_partial_flow_graph, analyze_bias_identifiability, require_identifiable_attribution}`
- `gross_error::{MultiplicityCorrection, GrossErrorPolicy, CommitDisposition, SuspectMeasurement, NodeDetectableBias, GrossErrorReport, BiasVectorSpec, VariableBiasPower, BiasPowerAnalysis, evaluate_gross_errors, enforce_commit_gate, analyze_bias_power}`
- `bilinear::{GradeValue, ObservedScalar, ObservedGrade, BilinearObsRef, BilinearObsCorrelation, BilinearStreamObservation, BilinearFlowRequest, ReconciledGrade, ReconciledStreamState, NodeElementRecovery, BilinearReconResult, reconcile_bilinear_flow}`
- `bias::{MAX_HIERARCHICAL_PARAMS, BiasTargetKind, MiningSourceKind, FactorMultiplierAttempt, refuse_factor_multiplier_application, HierarchicalGroupPrior, StandaloneNodePrior, NodeBiasSpec, BiasObservation, HierarchicalBiasRequest, HierarchicalPosteriorSummary, BiasUpdateEvent, HierarchicalBiasResult, PosteriorSampleRequest, HierarchicalPosteriorDraws, fit_hierarchical_bias_posteriors, sample_hierarchical_posterior}`
- `identifiability::{CandidateMeasurementKind, IdentifiabilityRequest, DeterminedContrastBasis, ParameterIdentifiability, ConfoundedBiasPair, IdentifiabilityReport, CandidateMeasurement, MeasurementVoiRequest, MeasurementVoiScore, analyze_identifiability, analyze_hierarchical_identifiability, require_full_identifiability, require_parameter_identifiable, evaluate_measurement_voi}`
- `certificate::{TAG_RECON_CERT, FMT_RECON_CERT_V1, RECON_CERT_SUBJECT_KIND, ParityDifferenceStatus, ParityExplanation, NodeBalanceRecord, BalanceCertificateSummary, CertifiedFactorInterval, CertifiedQuantityInterval, PosteriorCertificateSummary, RefusedPairVoiRecord, IdentifiabilityCertificateSummary, RegisteredForecastScore, AttestationStatus, ReconciliationCertificateRequest, ReconciliationCertificate}`

## 12. Proof obligations
- `BD-149`, `PO-TOUCH-1`, `PO-TOUCH-2`, `G-TOUCH-1`, `G-TOUCH-2`, `G-TOUCH-3` (`tests/recon_known_answers.rs`):
  - `recon_known_answers::test_gb24_linear_wls_reconciliation_and_guards`
  - `recon_known_answers::test_amira_p754_multi_node_wls_covariance_and_partial_observability`
  - `recon_known_answers::test_gb25_bias_identifiability_refusal_and_audit_resolution`
  - `recon_known_answers::test_po_touch_2_monte_carlo_posterior_covariance_match`
- `BD-150` (`tests/recon_known_answers.rs`):
  - `recon_known_answers::test_gross_error_power_curves_match_exact_theory_and_quarterly_mdb`
- `BD-151` (`tests/recon_known_answers.rs`):
  - `recon_known_answers::test_bilinear_two_product_exact_and_noisy_cu_au_concentrator`
  - `recon_known_answers::test_bilinear_two_node_rougher_cleaner_circuit`
- `BD-152`, `BD-153`, `BD-156` (`tests/recon_known_answers.rs` + `crates/fmine-verify/tests/gate_touch_02.rs`):
  - `recon_known_answers::test_bd152_hierarchical_bias_pooling_porgera_sources_and_safeguards`
  - `recon_known_answers::test_bd153_observation_rank_nullspace_and_measurement_voi`
  - `recon_known_answers::test_bd156_reconciliation_certificate_roundtrip_and_guards`
  - `recon_known_answers::test_fresh_eyes_hardening_guards_and_large_lambda_power`
- `BD-157`, `GATE-TOUCH-01` (`crates/fmine-verify/tests/gate_touch_01.rs`):
  - `gate_touch_01::test_gate_touch_01_synthetic_flowsheets_and_mill_balance`
  - `gate_touch_01::test_gate_touch_01_planted_faults_and_guards`
- `BD-158`, `BD-159`, `GATE-TOUCH-02`, `PO-TOUCH-5`, `PO-TOUCH-6` (`crates/fmine-verify/tests/gate_touch_02.rs`):
  - `gate_touch_02::test_gate_touch_02_bias_recovery_sbc_identifiability_and_5pct_power`
  - `gate_touch_02::test_bd156_reconciliation_certificate_fmine_verify_roundtrip_and_tamper`
- `BD-261`, `KILL-PLUMB-6` (`crates/fmine-verify/tests/kill_plumb_6.rs`):
  - `kill_plumb_6::test_kill_plumb_6_control_mine_quarterly_false_alarm_rate_and_shift_power`
