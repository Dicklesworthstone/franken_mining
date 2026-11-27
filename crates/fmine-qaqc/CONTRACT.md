# CONTRACT.md — `fmine-qaqc`

## 1. Purpose
Corebox QA/QC kernel (`BD-167`, `BD-168`, `BD-169`, `BD-282`, `BD-283`, `BD-284`; plan §12.1–§12.4). Evaluates certified reference materials (CRMs) under explicit named SD bases (`σ_C`, `σ_L`, `S_W`) and the CANMET batch-mean test with mandatory Student-t bias intervals and versioned site rule texts; accounts for QC insertion rates with explicit denominators and `PolicyDefault` vs `QpPolicy` targets; evaluates blanks under versioned contamination rules (including conjunctive `"and"` vs disjunctive `"either/or"` carryover logic) and disaggregates failure rates per laboratory with failed-batch cascades to routine samples (`PO-CORE-4`); runs sequential anytime-valid PIT betting e-process drift monitors (`fmine-numerics::eproc`) alongside multi-rule pattern monitors and a dispersion-collapse integrity monitor (`GATE-CORE-02`); computes canonical `CV_AVR%` duplicate precision, derived `HARD`/`AMPD`, Thompson–Howarth (`N >= 50`, labelled optimistic), `E374` mass matching, `FieldDuplicateAbsent`, Gy's general and calibrated FSE nomogram (`OutsideValidity` off its calibration grade), and nugget-split protocol-vs-infill options; and fits multi-detection-limit left-censored lognormal distributions by MLE and robust ROS while enforcing guard `G-CENS` against DL/2 substitution.

- **Crate:** `fmine-qaqc`
- **Layer:** K1
- **Subsystem:** Corebox
- **LOC budget:** 15,000 planned / 17,000 ceiling
- **Allowed Module Families:** `["censored", "crm", "duplicates", "error", "insertion", "monitors", "stats"]`

## 2. Inputs
- Canonical `fmine-drill::records` types (`Sample`, `QcInsertion`, `AssayResult`, `AssayValue`, `AssayMethod`, `BlankStage`, `DuplicateStage`, `SampleType`) and typed policy/input structs (`SiteCrmRule`, `CrmBatchInput`, `InsertionTargetSpec`, `SiteBlankRule`, `CrmMonitorConfig`, `DuplicatePair`, `GyCalibratedConstants`, `GyNomogramRequest`, `NuggetSplitInput`, `CensoredDataset`).
- Zero bare `f64` parameters in public functions (`L-6`).
- Dependencies: `fmine-core`, `fmine-evidence`, `fmine-claim`, `fmine-numerics` (`K1 -> K0 / S0`), and `fmine-drill` (registered intra-layer `K1` DAG edge in `registries/workspace_topology.toml`). Zero external crates; zero I/O, network, or ambient clock APIs (`L-3`).

## 3. Outputs
- `crm::CrmBatchEvaluation` and `crm::MultiBasisComparison`: per-point verdicts, named SD basis, CANMET batch detail, grade-tiered re-assay brackets, and mandatory `BiasInterval` (mean bias, SE, Student-t statistic, two-sided p, confidence interval, and `excludes_zero`).
- `insertion::InsertionRateSummary`, `insertion::BlankBatchEvaluation`, and `insertion::CampaignQaqcReport`: achieved-vs-target rates with explicit numerator/denominator, absent controls, per-lab disaggregated summaries (`E167`), `E374` mass-mismatch duplicate list, and `FailedBatchCascade` records (`PO-CORE-4`).
- `monitors::CrmMonitorSnapshot` and `MonitorAlarm`: e-process wealth and Ville alarm step, pattern alarms (`2 > ±2 SD`, `4 > ±1 SD`, sign imbalance), and dispersion-collapse alarms tagged strictly `AlarmScope::Integrity`.
- `duplicates::DuplicatePrecisionReport`, `StageVarianceDecomposition`, `GyNomogramReport`, and `NuggetSplitDecision`: canonical `CV_AVR%`, derived `HARD`/`AMPD`, optional `ThompsonHowarthEstimate` (`N >= 50`, caveat attached), `FieldDuplicateAbsent`, Gy FSE variance with `Derived`/`Speculative` evidence and `WithinValidity`/`OutsideValidity`, and Helm information options.
- `censored::LognormalMleFit` and `censored::RosFit`: fitted log-scale parameters, arithmetic moments, quantiles, log-likelihood, and high-censoring warning without mutating raw L0 records.

## 4. Invariants
- **No anonymous CRM SD basis (`BD-282`, E364–E366):** every CRM rule explicitly names `SdBasis` (`sigma_c`, `sigma_l`, or `running_sw`) and always emits `BiasInterval` beside pass/fail.
- **No silent method merge (`PO-CORE-3`):** `insertion::evaluate_campaign_qaqc` refuses mixed analytes or `AssayMethod`s (`FMINE-REF-105`).
- **No DL/2 substitution (`G-CENS`, §12.2):** `censored::refuse_dl_substitution` refuses any DL fraction; CRM evaluation refuses censored results; blank evaluation preserves `<DL` as `PassBelowDetection`; duplicate evaluation excludes censored pairs transparently.
- **Dispersion collapse is integrity scope (`BD-283`, E368):** `MonitorKind::DispersionCollapse` unconditionally emits `AlarmScope::Integrity`, never `CalibrationOrBias`.
- **Anytime-valid e-process (`BD-169`, GATE-CORE-02):** `CrmSequentialMonitor` feeds standard-normal PIT transforms into `fmine_numerics::eproc::BettingEProcess(0.5, alpha)`, bounding null false alarms by Ville's inequality (`<= alpha`).
- **Duplicate mass & identifiability (`BD-284`, E374, C4 P12):** quarter-core duplicates of half-core originals or mass-mismatched duplicates are flagged/refused (`FMINE-REF-102`); missing field duplicates report `FieldDuplicateAbsent`; duplicate variance exceeding variogram nugget refuses (`FMINE-REF-104`) rather than clamping geological nugget negative.
- **Gy calibration validity (`BD-284`, E359–E363):** uncalibrated `Assumed` constants (`K = 470, α = 1.5`) render `NomogramEvidence::Speculative`; evaluating outside `[valid_grade_min, valid_grade_max]` renders `NomogramValidity::OutsideValidity`; `< 30` increments flags the stage.
- **Thompson–Howarth discipline (`E369–E370`):** omitted below `N = 50` pairs and always labelled `"assumes normal errors; optimistic"`.

## 5. Refusals
- `QaqcError::InvalidParameter` / `SubstitutionRefused` / `EProcess` -> `FMINE-REF-109` (domain violations, `G-CENS` substitution attempts, zero basis SD, `M_S >= M_L`).
- `QaqcError::DuplicateSupportMismatch` -> `FMINE-REF-102` (strict `E374` mass/support mismatch).
- `QaqcError::NonIdentifiable` -> `FMINE-REF-104` (duplicate variance exceeding total variogram nugget).
- `QaqcError::MethodOrAnalyteMismatch` -> `FMINE-REF-105` (`PO-CORE-3` mixed analyte/method).
- `QaqcError::InsufficientData` -> `FMINE-REF-201` (empty batch, `< 2` observations for CANMET/running SD/censored fits).
- `QaqcError::NumericalFailure` -> `FMINE-REF-207` (degenerate zero-variance uncensored sample or non-convergent MLE).
- `QaqcError::UnusableInput` -> `FMINE-REF-208` (censored CRM observation, unknown sample reference, unsupported `AboveUpper` in left-censored fit).

## 6. Determinism class
`BitwisePortable` (plan §29.2): IEEE-754 arithmetic, `fmine_core::det` special functions (`exp`, `expm1`, `ln`, `sqrt`, `hypot`, `norm_cdf`, `norm_ppf`), Laplace tail continued fractions, Lanczos `ln_gamma`, Lentz beta/gamma continued fractions, and `BTreeMap`/`BTreeSet` iteration only.

## 7. Evidence class of outputs
Derived analytical QA/QC diagnostics and error-model parameters (layer L0/Corebox). Never promotes `Assumed` constants (`Speculative`) or textbook defaults (`PolicyDefault`) into compliance or measured truth (`INV-006`).

## 8. Validity domain
- Assay values are mass fractions in `[0, 1]` (or strictly `(0, 1]` for lognormal MLE/ROS).
- `CrmSequentialMonitor` assumes independent sequential CRM observations under the null with Gaussian errors `N(μ, σ_ref^2)` for the PIT transform and chi-square dispersion check.
- `censored` fits left-censored (`<DL`) lognormal models only; right-censored (`>UL`) values are explicitly refused.
- Gy calibrated nomograms apply only inside their declared calibration grade range (`E359`).

## 9. Oracles
- Abzalov AMG Ex. 10.1.1.a (`FX-2`, E364–E366): exact reproduction of 10/10 warnings & 8/10 failures on `σ_C`, 3/10 warnings & 0/10 failures on `σ_L`, CANMET pass, and +0.357% Fe bias (~12.3 SE, 95% CI excluding 0).
- Abzalov AMG 30-pieces calibration (`FX-1`, E361): `rel_var = 9.41`, `mass = 2889 g`, `sphere_d = 12.25 cm` -> `d_N = 19.45 cm`, `K = 316.9`.
- Hand-worked `CV_AVR%`, `HARD`, `AMPD`, Carlin `"and"` vs Turquoise Ridge `"either/or"` blank rules (`C2 §4.5b`), NGM/Loulo/Kibali CRM site rules (`E166`, `E221`, `E222`), and Carlin/Kibali insertion denominators (`E165`, `E226`).
- `GATE-CORE-02` seeded synthetic stream Monte Carlo suite (500 null streams verifying FPR `<= α`, 1 SD drift detection delay, and 0.3× variance-collapse integrity alarms).
- Seeded multi-DL left-censored lognormal recovery oracle comparing MLE and ROS against ground truth and demonstrating biased DL/2 control rejection.

## 10. Known gaps
- Does not persist `BatchFlag` or `InvestigationCase` ledger records directly; emits `FailedBatchCascade` and `MonitorAlarm` values for `fmine-drill` / `fmine-calib` callers to commit.
- Does not store CAS lab-certificate bytes (`BD-171` lives in ingest/CAS adapters).
- Umpire inter-laboratory hierarchical bias model (RMA + mixed-effects bias) and technical-report Data-Verification Index rendering live in downstream beads (`BD-174`).

## 11. Public API
- `QaqcError` (`error::QaqcError`)
- `crm::{SdBasis, CrmTestKind, ConsecutiveBiasRule, GradeReassayWindow, SiteCrmRule, CrmCertificateSpec, CrmObservation, CrmBatchInput, CrmPointStatus, CrmPointVerdict, BiasInterval, CanmetBatchDetail, CrmBatchEvaluation, MultiBasisComparison, MultiBasisConfig, evaluate_crm_batch, evaluate_crm_all_bases}`
- `insertion::{InsertionDenominator, TargetBasis, RateBandPct, InsertionTargetSpec, AchievedRate, InsertionRateSummary, BlankFailureMode, BlankCarryoverThresholds, SiteBlankRule, BlankObservation, BlankStatus, BlankVerdict, BlankBatchEvaluation, ControlAbsentPeriod, LabQaqcSummary, FailedBatchCascade, CampaignQaqcInput, CampaignQaqcReport, compute_insertion_rates, evaluate_campaign_qaqc}`
- `monitors::{AlarmScope, MonitorKind, MonitorAlarm, CrmStreamKey, CrmReferenceParams, CrmMonitorConfig, CrmMonitorSnapshot, CrmStreamValue, CrmSequentialMonitor}`
- `duplicates::{THOMPSON_HOWARTH_CAVEAT, THOMPSON_HOWARTH_MIN_PAIRS, GY_DEFAULT_K, GY_DEFAULT_ALPHA, GY_SAFETY_LINE_REL_VAR, GY_MIN_INCREMENTS, DuplicatePair, DuplicatePolicy, ThompsonHowarthEstimate, DuplicatePrecisionReport, StageDecompositionStatus, StageVarianceDecomposition, PrecisionReferenceLevel, evaluate_duplicates, decompose_duplicate_stages, reference_precision_policy_defaults, GyGeneralInput, gy_general_fse_variance, GyCalibrationMethod, GyGradeWindow, GyCalibratedConstants, ThirtyPiecesInput, ThirtyPiecesDiameter, ThirtyPiecesCalibration, calibrate_thirty_pieces, GySamplingStage, GyNomogramRequest, NomogramEvidence, NomogramValidity, GyStageOutcome, GyNomogramReport, evaluate_gy_nomogram, InformationOption, NuggetSplitInput, NuggetSplitDecision, evaluate_nugget_split}`
- `censored::{SubstitutionAttempt, refuse_dl_substitution, CensoredDataset, LognormalMleFit, RosFit, fit_censored_lognormal_mle, fit_censored_ros}`

## 12. Proof obligations
- `BD-167` (`tests/qaqc_known_answers.rs`):
  - `qaqc_known_answers::test_insertion_rates_denominators_control_absent_and_campaign_cascade`
  - `qaqc_known_answers::test_blanks_carlin_and_vs_turquoise_ridge_or_and_g_cens`
  - `qaqc_known_answers::test_po_core_3_refuses_mixed_methods_and_disaggregates_by_lab`
- `BD-282` (`tests/qaqc_known_answers.rs`):
  - `qaqc_known_answers::test_fx2_crm_multi_basis_canmet_and_bias_interval`
  - `qaqc_known_answers::test_site_crm_rules_kibali_loulo_ngm_and_grade_reassay_windows`
- `BD-284` (`tests/qaqc_known_answers.rs`):
  - `qaqc_known_answers::test_duplicates_cv_avr_hard_ampd_mass_guard_thompson_howarth_and_field_absent`
  - `qaqc_known_answers::test_gy_fx1_calibration_nomogram_validity_and_nugget_split`
- `BD-168` (`tests/qaqc_known_answers.rs`):
  - `qaqc_known_answers::test_censored_mle_and_ros_recover_seeded_lognormal_and_refuse_dl_half`
- `BD-169`, `BD-283`, `GATE-CORE-02` (`tests/gate_core_02.rs`):
  - `gate_core_02::test_gate_core_02_eprocess_null_fpr_and_1sd_drift_detection_delay`
  - `gate_core_02::test_gate_core_02_dispersion_collapse_integrity_alarm_and_pattern_monitors`
