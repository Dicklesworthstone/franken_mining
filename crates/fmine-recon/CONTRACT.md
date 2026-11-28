# CONTRACT.md — `fmine-recon`

## 1. Purpose
Touchstone reconciliation kernel (`BD-149`, `BD-150`, `BD-151`; plan §20.1–§20.6, §20.10–§20.12). Implements R1 linear weighted-least-squares (WLS) material-balance reconciliation over `fmine-flow::FlowGraph` topologies with full measurement covariance and Joseph-form posterior covariance (`fmine-numerics::reconciliation`), observable unmeasured stream/inventory inference (`fmine-numerics::reconciliation::partial`), global chi-square and per-measurement normalized-adjustment gross-error tests with exact theoretical detection power curves and quarterly Minimum Detectable Bias (MDB) publication, bilinear tonnes × grade metallurgical reconciliation over extensive `[dry_mass, elemental_metal]` state vectors (`fmine-numerics::reconciliation::nonlinear`) with post-reconciliation derived grades and recoveries, and Bayesian bias identifiability guards (`G-TOUCH-1` `FMINE-REF-103`, `G-TOUCH-2` `FMINE-REF-104`, `G-TOUCH-3` `FMINE-REF-102`).

- **Crate:** `fmine-recon`
- **Layer:** K2
- **Subsystem:** Touchstone
- **LOC budget:** 18,000 planned / 21,000 ceiling
- **Allowed Module Families:** `["bilinear", "error", "gross_error", "linear", "stats"]`

## 2. Inputs
- Validated `fmine_flow::graph::FlowGraph` topologies and typed domain requests (`LinearFlowRequest`, `NamedLinearRequest`, `PartialFlowRequest`, `BiasIdentifiabilityRequest`, `GrossErrorPolicy`, `BiasVectorSpec`, `BilinearFlowRequest`, `DirectGradeReconciliationAttempt`).
- Zero bare `f64` parameters in public functions (`L-6`).
- Strictly downward dependencies: `fmine-core`, `fmine-evidence` (`S0`), `fmine-numerics` (`K0`), `fmine-flow` (`K1`). Zero external crates; zero I/O, network, or ambient clock APIs (`L-3`).

## 3. Outputs
- `linear::LinearReconResult` and `linear::PartialReconResult`: reconciled extensive estimates, adjustments, full prior/posterior/adjustment covariances, raw and reconciled residuals, max relative closure error (`PO-TOUCH-1`), global `chi_square`, degrees of freedom, normalized adjustments, `|corr| > 0.8` high-correlation diagnostics, and BLAKE3 determinism digest (`PO-TOUCH-6`).
- `linear::BiasIdentifiabilityReport`: Bayesian posterior means, covariance, pair correlation, determined contrast interval, and either attributable findings or `FMINE-REF-104` refusal (`GB-25`, `G-TOUCH-2`).
- `gross_error::GrossErrorReport`, `BiasPowerAnalysis`, and `NodeDetectableBias`: global chi-square p-value and critical value, family-wise-corrected per-measurement suspect ranking, commit disposition (`Admit` vs `RefuseForInvestigation` -> `FMINE-REF-101`), exact non-central chi-square and normal two-sided theoretical power for any candidate bias vector, and single-period / quarterly Minimum Detectable Bias per redundant node (§20.6).
- `bilinear::BilinearReconResult`: reconciled extensive dry masses and elemental metal masses for all circuit streams, reconstructed full local covariance, post-reconciliation derived grades with first-order delta-method uncertainty, node mass pulls and elemental recoveries, constraint closure audit, and BLAKE3 digest.

## 4. Invariants
- **Reconcile extensive quantities only (`G-TOUCH-1`, §20.2):** direct grade reconciliation is refused with `FMINE-REF-103`; all balance constraints `A x = b` act strictly on extensive dry mass, water mass, and elemental metal masses, with grades derived post-reconciliation (`metal / dry_mass`).
- **Exact balance closure (`PO-TOUCH-1`, §20.11):** every reconciled linear or bilinear estimate satisfies `|A x̂ - b| <= 1e-9` relative (audited fail-closed; zero plugs).
- **Identifiability refusal (`G-TOUCH-2`, §20.6):** pairs whose posterior `|correlation| > 0.8` are flagged on every reconciliation result, and `analyze_bias_identifiability` refuses prior-dominated individual attribution with `FMINE-REF-104` while publishing the determined linear contrast.
- **Stockpile decoupling guard (`G-TOUCH-3`, §20.1):** stockpile nodes (`NodeClass::StockpileOrPad`) cannot silently assume zero inventory delta in linear or bilinear balances; missing inventory deltas refuse.
- **Commit-time gross-error gate (`INV-005`, §20.2):** `enforce_commit_gate` refuses periods failing global or measurement gross-error thresholds with `FMINE-REF-101`, returning ranked suspects for an `InvestigationCase`.
- **No silent clamping or regularization:** non-SPD covariances, dependent constraint rows, unobservable latent streams (`FMINE-REF-104`), and infeasible bilinear domain trials refuse cleanly.

## 5. Refusals
- `ReconError::ConservationGrossError` -> `FMINE-REF-101` (commit refused on gross-error failure).
- `ReconError::StockpileDecouplingRefused` -> `FMINE-REF-102` (`G-TOUCH-3` stockpile decoupling violation).
- `ReconError::DirectGradeReconciliationRefused` -> `FMINE-REF-103` (`G-TOUCH-1` direct grade reconciliation attempt).
- `ReconError::NonIdentifiable` -> `FMINE-REF-104` (`G-TOUCH-2` `|corr| > 0.8` attribution refusal or unobservable unmeasured stream).
- `ReconError::InvalidParameter` -> `FMINE-REF-109` (non-positive SD/mass, invalid alpha/power, out-of-domain grades, unknown stream/node).
- `ReconError::InsufficientRedundancy` -> `FMINE-REF-201` (zero interior balance equations or zero degrees of freedom for gross-error testing).
- `ReconError::NumericalFailure` / `ReconError::UpstreamRefusal` -> `FMINE-REF-207` (or preserved upstream refusal code from `fmine-numerics` / `fmine-flow`).

## 6. Determinism class
`DeterministicPerIsa` (plan §29.2): delegates linear/partial/nonlinear LDLᵀ solves to `fmine-numerics::reconciliation` (`DeterministicPerIsa`); special functions in `stats` use `fmine_core::det` (`BitwisePortable`); canonical `BTreeMap`/`BTreeSet` variable ordering and BLAKE3 result digests ensure replay stability on the same ISA (`PO-TOUCH-6`).

## 7. Evidence class of outputs
Derived state estimates and statistical gross-error / power diagnostics conditioned on declared measurement error models. Never asserts compliance, never overwrites raw L0 observations, and never promotes prior-dominated splits into attributable findings (`INV-006`).

## 8. Validity domain
- Bounded to `fmine-numerics::reconciliation` limits (`<= 128` variables/observations, `<= 64` independent linear constraints).
- Chi-square p-values and exact analytical power curves assume unbiased Gaussian measurement errors with the declared SPD covariance `V` under `H_0`.
- Bilinear local covariances are Gauss–Newton tangent approximations (`NonlinearReconciliation`), valid locally around interior solutions (`M_s > 0`, `0 < m_{s,e} < M_s`).

## 9. Oracles
- Plan §20.12 `GB-24` (truck 1,000,000 t ± 3%, mill 960,000 t ± 1%, stockpile +25,000 t ± 15,000 t -> exact WLS 988,908.61 / 961,135.76 / 27,772.85 t, `χ² = 0.1849`, `p = 0.667`).
- Plan §20.12 `GB-25` (F2 log-ratio `-9.6% ± 1.5%` with symmetric `5%` priors -> `corr = 0.92` refuses `FMINE-REF-104`; adding `+1.0% ± 2.0%` mill-sampler audit -> `b_gc = -7.1% ± 2.2%`, `b_mill = +1.8% ± 1.8%`).
- AMIRA P754 single-node and 2-node rougher–cleaner mass & metal balance closed-form WLS oracles + `PO-TOUCH-2` Monte Carlo posterior covariance verification.
- Exact analytical vs Monte Carlo empirical gross-error power curves (`BD-150`) across bias sweeps `δ/σ ∈ {0, 1, 2, 3, 4, 5}` for both global non-central `χ²` and per-measurement tests.
- Exact two-product concentrator formula `C/F = (f - t)/(c - t)` and `R = c(f - t)/(f(c - t))` plus `fmine-gen::mill_balance` Cu–Au concentrator verification (`BD-151`, `GATE-TOUCH-01`).

## 10. Known gaps
- R2 multi-period Kalman smoother / NUTS hierarchical bias posteriors (`BD-152`, `BD-153`, `BD-154`) and per-lot moisture reconciliation (`BD-155`) live in subsequent Touchstone beads.
- R0 site parity F-factor registry and compensation detector (`BD-156`) live downstream.

## 11. Public API
- `ReconError` (`error::ReconError`)
- `linear::{IDENTIFIABILITY_CORRELATION_LIMIT, PO_TOUCH_1_RELATIVE_TOLERANCE, ExtensiveChannel, VariableKey, MeasuredVariable, CovarianceOffDiagonal, DirectGradeReconciliationAttempt, refuse_direct_grade_reconciliation, LinearFlowRequest, NamedLinearConstraint, NamedLinearRequest, CorrelatedVariablePair, LinearReconResult, PartialFlowRequest, PartialReconResult, BiasPriorSpec, BiasLinearObservation, BiasIdentifiabilityRequest, BiasPosteriorEstimate, ContrastEstimate, BiasIdentifiabilityReport, reconcile_flow_graph, reconcile_named_system, reconcile_partial_flow_graph, analyze_bias_identifiability, require_identifiable_attribution}`
- `gross_error::{MultiplicityCorrection, GrossErrorPolicy, CommitDisposition, SuspectMeasurement, NodeDetectableBias, GrossErrorReport, BiasVectorSpec, VariableBiasPower, BiasPowerAnalysis, evaluate_gross_errors, enforce_commit_gate, analyze_bias_power}`
- `bilinear::{GradeValue, ObservedScalar, ObservedGrade, BilinearObsRef, BilinearObsCorrelation, BilinearStreamObservation, BilinearFlowRequest, ReconciledGrade, ReconciledStreamState, NodeElementRecovery, BilinearReconResult, reconcile_bilinear_flow}`

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
- `BD-157`, `GATE-TOUCH-01` (`crates/fmine-verify/tests/gate_touch_01.rs`):
  - `gate_touch_01::test_gate_touch_01_synthetic_flowsheets_and_mill_balance`
  - `gate_touch_01::test_gate_touch_01_planted_faults_and_guards`
