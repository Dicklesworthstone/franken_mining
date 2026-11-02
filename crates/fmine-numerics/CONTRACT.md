# CONTRACT.md — `fmine-numerics`

## 1. Purpose
The K0 numerical kernels that the domain crates call:
- in `lib.rs`: a dense SPD Cholesky solve, Brent's bracketed root-finder, and the standard normal CDF and inverse CDF;
- anytime-valid betting e-processes, on PIT values and on bounded scores (`eproc`), EVPI and EVSI in closed form and by Monte Carlo (`voi`), empirical VaR and CVaR (`risk`), and `(max, +)` / `(min, +)` precedence scheduling (`tropical`);
- `sparse`: CSR with deterministic COO assembly, AMD with a supernodal LDLᵀ, and PCG with ILU(0);
- `exact_feas`: exact rounding-interval feasibility for published tables (BD-340), with its discovery reports, table generator and power study.

It does no I/O and holds no domain rules (L-3). `xtask check-claims` exempts it from L-6, so its kernels take bare `f64`.

`src/sparse.rs`, `src/sparse/direct.rs` and `src/sparse/precond.rs` are lifted from frankensim `crates/fs-sparse/src/{lib,direct,precond}.rs` at `20463525d9775a18c1b237d2676f364f367175ab`, under `LicenseRef-MIT-OpenAI-Anthropic-Rider` (BD-145, plan §27.1 wave 0). Each has a `[[lift]]` row in `registries/lifts.toml`. Each file's first line lists its changes, chiefly panics turned into typed refusals, and what was left out: BSR, SELL-C-σ, the FMA capsule, `ops`, nested dissection, Chebyshev and SA-AMG. `src/eproc.rs`, `src/voi.rs`, `src/risk.rs` and `src/tropical.rs` are original code, with `[[analogue]]` rows for fs-eproc, fs-voi, fs-robust and fs-tropical, so GATE-FSIM-DRIFT watches those. A real lift is BD-022's decision (bd-022).

- **Crate:** `fmine-numerics`
- **Layer:** K0
- **Subsystem:** Bedrock
- **LOC budget:** 30,000 planned / 35,000 ceiling
- **Allowed Module Families:** `["eproc", "voi", "risk", "tropical", "exact_feas", "sparse"]`

## 2. Inputs
- **Dense kernels.** Bare `f64`, unitless to this crate:
  - `cholesky_solve` takes a row-major `n × n` matrix, a right-hand side and `n`;
  - `brent_root` takes a bracket, `tol`, `max_iter` and an `FnMut(f64) -> f64`;
  - `norm_cdf` takes `z` and `norm_inv` takes `p`.
- **Original kernels.**
  - `voi`: state probabilities and a `payoffs[state][action]` matrix (its doc comment says USD millions), a `likelihood[state][signal]` matrix and a non-negative sample cost. The Monte Carlo estimate also takes a draw count and a Philox `RngKey`.
  - `risk`: a slice of losses and `alpha` in (0, 1).
  - `eproc`: for `BettingEProcess`, a null mean and `alpha` in (0, 1), then PIT values in [0, 1]. For `ScoreBettingEProcess`, a score range `[lo, hi]` with `lo < 0 < hi`, a prior second moment > 0, a stake truncation and `alpha` in (0, 1), then scores in the range.
  - `tropical`: node counts, arcs with finite `f64` minimum lags, and a finite deadline.
- **`sparse`.** COO triplets `(row, col, value)`, CSR parts, `LdltOptions`, right-hand sides, a PCG tolerance and an iteration limit.
- **`exact_feas`.** Cells with exact printed values and rounding steps (`Rat = fmine_units::ExactQty`), identities over cell ids, and `Rules`, each declaring a convention and one construction per family. The discovery reports take footings, printed values, factors, unit pairs and price decks. The generator takes a Philox key and a `GenSpec`.
- **No files.** It reads no files or registries.

## 3. Outputs
None is a Claim; callers wrap results.
- **Dense and original kernels.**
  - `cholesky_solve` gives `Result<Vec<f64>, Refusal>`, and `brent_root` gives `Result<Root, RootError>`.
  - `norm_cdf` and `norm_inv` give an `f64`.
  - `BettingEProcess` gives its wealth and `rejected_null`, or an `EProcError`. `ScoreBettingEProcess` gives its log wealth, its next stake and `rejected_null`, or an `EProcError`.
  - `voi` gives `VoiSummary`; `VoiAnalysis`, which adds the prior action values, every tied optimal action and, per signal, its probability, posterior, action values, tied optimal actions and EVSI contribution; `McEvsi`, an estimate with its standard error and a `VoiStatus`; or a `VoiError`.
  - `risk` gives `TailRiskSummary` and a drawdown, or a `RiskError`.
  - `TropicalDag` gives start-time vectors, or a `TropicalError`.
- **`sparse`.** `Csr`, `SymbolicLdlt`, `LdltFactor` (solve, `Inertia`, `FactorStats`), `Ilu0` and `PcgSolution`, or a typed error.
- **`exact_feas`.**
  - One `Component` per connected group of cells, with an `Outcome`.
  - `Blame` with `ConflictSet`s and `CorrectionSet`s, `PresenceConflict`s, and implied `Bound`s.
  - A hex `certificate_digest`.
  - Discovery reports (`ConstructionReport`, `TieDiscovery`, `UnitDiscovery`, `DeckDiscovery`), which decide no state.
  - Generated tables, and power and kill tables (`PowerTable`, `KillReport`) with JSON log lines.
- **Golden digests.** Hex BLAKE3 digests from `golden_digest_norm_cdf_inv_10k` and `golden_digest_cholesky_brent_10k`.

## 4. Invariants
- **`lib.rs` is the same bits on both reference hosts.** `test_golden_digest_norm_cdf_inv_10k` and `test_golden_digest_cholesky_brent_10k` pin BLAKE3 digests over 10,000 `NUMERICS_DIGEST_KEY` inputs. The inputs reach |z| up to 12 and p from 1e-304 to 1 − 1e-16, plus seeded 3×3 SPD systems and cube roots. `test_normal_digest_inputs_cover_tails_and_change_with_key` checks the inputs themselves.
- **No unconverged or clamped answer from `lib.rs`.**
  - `test_brent_refuses_non_convergence_and_bad_inputs`, and `test_brent_property_monotone_cubics` (seeded cubics).
  - `test_norm_inv_out_of_domain_is_nan_not_clamped`: NaN outside [0, 1], ±∞ at 0 and 1.
  - `test_cholesky_solve_and_singular_refusal` and `test_cholesky_non_spd_refuses_207`.
- **No clamped or made-up answer from the original kernels (bd-a482).** Out-of-domain input refuses and leaves the state as it was:
  - `test_eproc_refuses_out_of_domain_parameters_and_observations`: parameters stand as given, and a refused observation changes nothing;
  - `test_score_eproc_refuses_out_of_domain_parameters_and_scores`: the same for `ScoreBettingEProcess`, whose range endpoints are scores, not errors;
  - `malformed_models_and_experiments_refuse_without_a_zero_score`: bad priors, payoffs, likelihoods and costs, and a malformed likelihood row for a zero-prior state, all refuse;
  - `test_risk_refuses_empty_non_finite_and_alpha_out_of_domain`: an `alpha` of 1 − 2⁻⁵³ over 49 losses gives the largest loss;
  - `test_tropical_refuses_bad_arcs_cycles_and_deadlines`: a positive cycle refuses, and a zero-lag cycle is feasible.
- **The finite VOI evaluator matches exhaustive enumeration.** `every_small_discrete_policy_matches_an_independent_exhaustive_oracle` checks EVSI, net VOI and the reported policy against all 27 deterministic policies on each of 729 two-state, three-action payoff matrices, with dyadic probabilities so that the oracle's arithmetic is exact. Known answers also cover uninformative and perfect signals and refinement (`test_evpi_non_negative_and_evsi_monotone`), large common offsets and cancellation (`no_signal_cannot_create_value_from_large_common_offsets_or_cancellation`, `common_payoff_offsets_preserve_decision_value`), and a fully revealing signal split in pieces (`splitting_a_fully_revealing_signal_preserves_perfect_information`).
- **The Monte Carlo EVSI recovers the closed form (GATE-HELM-02, bd-227).** `test_mc_evsi_recovers_closed_form_chi2` runs K = 12 problems: GB-29, its two flips, a three-action variant and eight seeded problems. Each gets 100 seeds of 2,000 draws.
  - The χ² statistic of the seed means' standardised deviations is 5.87, with upper tail 0.92. The test is two-sided at α = 0.01, so the tail must lie in [0.005, 0.995].
  - The deviations are standardised by the estimator's own reported SEs, so the test checks the SE as well as the mean.
  - Three planted negatives on the same seeds fail it: a bias of half a reported SE, an SE understated 3× and an SE overstated 3×.
  - With no signal that changes the choice, the estimate is exactly 0 and nothing is sampled: `test_mc_evsi_without_a_flip_is_exactly_zero_and_unsampled`. A signal on which the prior action ties for best keeps it, so an exact tie is not sampled as a change (same test).
  - A key fixes the result bitwise: `test_mc_evsi_is_a_pure_function_of_its_key`.
- **`sparse` is deterministic and never perturbs.**
  - The matrix is bitwise independent of push order: `test_assembly_is_stream_order_invariant_bitwise`.
  - Factor and PCG repeat bitwise on one host: `test_factorization_is_bitwise_deterministic` and `test_setup_and_solve_are_deterministic`.
  - A singular matrix refuses rather than being perturbed: `test_exactly_singular_shift_refuses_instead_of_perturbing`.
  - No PCG refusal carries an iterate: `test_pcg_refusals_carry_no_iterate`.
  - A bad push or SpMV writes nothing: `test_push_outside_the_matrix_refuses_and_stages_nothing` and `test_spmv_refuses_wrong_lengths_and_writes_nothing`.
  - Inertia matches analytic eigenvalue counts across shifts: `test_inertia_certifies_analytic_eigenvalue_counts_across_shifts`.
- **`exact_feas` reports only what an independent checker accepts.** Every `Feasible` witness passed `check_witness`, and every `Infeasible` certificate passed `check_certificate`. A rejected one is `Indeterminate`.
  - `test_witness_checker_rejects_tampered_witness` and `test_certificate_checker_rejects_wrong_sign_multiplier`.
  - Verdicts and digests do not depend on row order or exact scaling: `test_verdicts_invariant_under_row_permutation_and_exact_scaling` and `test_certificate_digest_invariant_under_row_order`.
  - A cap or an overflow is `Indeterminate`, never a verdict: `test_caps_give_indeterminate` and `test_overflow_gives_indeterminate`.
  - A weighted mean's feasible relaxation is `NotRefuted`, never `Feasible`: `test_feasible_envelope_reports_not_refuted_never_consistent`.
  - A correction set is labelled `Minimum` only after an exhaustive search: `test_correction_label_minimum_only_when_exhaustive`.
- **Generator truth satisfies its identities exactly.** `test_generator_truth_satisfies_identities_exactly`, and `test_generator_deterministic_for_key`.
- **The kill report fires only when all four measures are empty.** `test_kill_report_fires_when_all_measures_empty`. Its seeded negative is `test_mutation_joint_solve_replaced_by_baseline_fires_kill`.

## 5. Refusals
- **`FMINE-REF-207`** (numerical certificate failed; proposed; exit 3), from `cholesky_solve`. It fires on `n = 0`, on lengths that do not describe an `n × n` system, or on a pivot that is non-finite or ≤ 1e-14: `test_cholesky_non_spd_refuses_207`.
- **`NON_IDENTIFIABLE` (`"FMINE-REF-104"`)** is the `reason_code()` of an ambiguous discovery finding (`Discovered`, `TieFinding`, `UnitFinding`, `DeckFinding`). It is a label on a report, not a `Refusal`: `test_ambiguous_discovery_refuses_ref_104` and `test_deck_ambiguity_refuses_ref_104`.
- **Typed errors, which carry no refusal code:**
  - `RootError::{NoSignChange, NonFinite, NotConverged}`;
  - `SparseError::{TooManyRows, OutOfRange, NonCanonical { reason, row }, LengthMismatch}`;
  - `LdltError::{NotSquare, StructurallyAsymmetric, NonFiniteEntry, PatternMismatch, InvalidOptions, FactorBudget, PivotBreakdown, RhsLength, NonFiniteRhs, Invariant}`. `PivotBreakdown` is a root front left with no acceptable pivot after delays, and `Invariant` replaces upstream's asserts;
  - `IluError::{NotSquare, NonFiniteEntry, Breakdown { row }}`;
  - `PcgError::{Sparse, NotSquare, NonFinite, InvalidTolerance, Breakdown { iter, quantity, value }, NotConverged}`;
  - `XfRefusal`'s twelve variants: `NegativeStep`, `OffGrid`, `DuplicateCell`, `DuplicateIdentity`, `UnknownCell`, `BadSigns`, `Degenerate`, `NoConstruction`, `DenominatorSpansZero`, `NotLinear`, `UnknownVariable` and `PlaceholderInIdentity`.
    - `test_refusals_name_the_input` covers seven of them.
    - `test_den_interval_through_zero_is_refused` and `test_presence_conflicts_refuse_and_locate` cover two more, and other tests reach `DuplicateCell` and `UnknownVariable`.
    - No test reaches `BadSigns`;
  - `ImpliedError::{Refused, Infeasible, Unbounded, Indeterminate}`, `TieRefusal`, `ShareRefusal::{NotPositive, NegativeStep, Interval}`, `GenError`, `PowerError` and `IntervalError`;
  - the original kernels' errors (bd-a482), each reached by its module's refusal test (Invariants):
    - `EProcError::{NullMeanOutOfDomain, AlphaOutOfDomain, ObservationOutOfDomain, ScoreRangeOutOfDomain, PriorSecondMomentOutOfDomain, TruncationOutOfDomain, ScoreOutOfRange}`;
    - `VoiError::{EmptyStates, EmptyActions, Shape, Probability, ProbabilityMass, Payoff, InvalidCost, Arithmetic, InformationBound, TooFewSamples}`. `is_numerical_failure` is true for `Arithmetic` and `InformationBound`, so a caller can tell a numerical refusal from a domain one without parsing text. `TooFewSamples` is a Monte Carlo request for fewer than two draws: `test_mc_evsi_refuses_what_the_closed_form_refuses`;
    - `RiskError::{Empty, NonFinite { index, value }, AlphaOutOfDomain}`;
    - `TropicalError::{ArcOutOfRange, NonFiniteLag, PositiveCycle, NonFiniteDeadline}`.
- **Callers map them.** `fmine_decide::rank_voi_campaigns` maps a `VoiError` to `FMINE-REF-207` when `is_numerical_failure`, else to `FMINE-REF-109`. `fmine_calib::SpatialFrontPitMonitor::new` maps an `EProcError` to `ScoreError::AlphaOutOfDomain`, and `observe_sample` refuses a non-finite sample with `FMINE-REF-109` before either e-process moves. Its two `ScoreBettingEProcess`es are updated together or not at all.

## 6. Determinism class
Each file's `//! Determinism:` header:
- `lib.rs`: **BitwisePortable**. `norm_cdf` and `norm_inv` delegate to `fmine_core::det` (bd-hp2j.2). `brent_root` and `cholesky_solve` use only `+ − × ÷` and `sqrt`. bd-hp2j.3 records that both golden digests agreed on ubuntu-latest (x86_64) and macos-15 (aarch64) in CI run 41237993223 at 84f32ac.
- `exact_feas.rs` and `exact_feas/generator.rs`: **Exact**. They use rational arithmetic, and the generator's draws are Philox integers.
- `exact_feas/discover.rs` and `exact_feas/power.rs`: **DeterministicPerIsa**. The counts are exact, but the tails and the Clopper–Pearson bounds are `f64` through `fmine_core::det`.
- `eproc.rs`, `voi.rs`, `risk.rs`, `tropical.rs`, `sparse.rs`, `sparse/direct.rs` and `sparse/precond.rs`: **DeterministicPerIsa**. None has a cross-host digest test. The `sparse` files repeat bitwise on one host (Invariants).

## 7. Evidence class of outputs
Not applicable: no Claims are emitted. A caller that wraps a result in a Claim decides its class and state.

## 8. Validity domain
- **`norm_cdf`** is `½(1 + erf(z/√2))` on `fmine_core::det::erf`, which bd-hp2j.2 rewrote from Abramowitz & Stegun 7.1.26. Its comment gives about 8 ulp from the correctly rounded value on a 0.005 grid over (0, 6), measured on an f64 prototype, and exactly ±1 from |x| ≥ 6. **`norm_inv`** is Acklam's rational approximation. Neither is exact in the tails.
- **`cholesky_solve`** refuses a pivot ≤ 1e-14 in absolute terms, whatever the matrix's scale.
- **`brent_root`** needs a sign change on `[a, b]` and a finite `f` at every point it evaluates.
- **`eproc`: `ScoreBettingEProcess`** tests a score's conditional null mean of 0. Its stake is aGRAPA-type (Waudby-Smith and Ramdas 2024): the running sum of scores over the prior second moment plus the running sum of squares, so one pseudo-observation at the null. It reads only earlier scores (`test_score_eproc_stake_is_predictable_and_bounded`). It is held to `[-c/hi, c/|lo|]`, so every factor is at least `1 - c`. Wealth is kept as a logarithm, so a long null run cannot underflow to 0. An alarm is the running maximum crossing `1/α` and is never withdrawn (`test_score_eproc_alarm_persists`). `test_score_eproc_ville_bound_and_uncentred_negative` runs 4,000 null runs of 600 location scores `tanh(z/2)` at α = 0.025 and asserts a false-alarm rate ≤ α (0.0185). Its planted negative, the dispersion score left uncentred (null mean about −0.075), alarms in 0.80 of runs. Validity needs the caller's score to have conditional mean exactly 0 under its null; the e-process cannot check that.
- **`eproc`: `BettingEProcess`.** `BettingEProcess::new` refuses a `null_mean` or `alpha` outside (0, 1), and `observe` refuses `u` outside [0, 1]; neither clamps (bd-a482). The bet fraction is held to ±0.75 by design, so every factor is at least 0.25. The null is two-sided, `E[U_t | U_<t] = μ0`. `test_ville_inequality_type1_error_bound` checks the false-alarm rate against α + 0.02 over 500 seeded runs, plus one alternative.
- **`voi`.** A problem must have states and actions, `payoffs` one row per state of one length, finite payoffs, and a prior that is a probability vector. Each likelihood row, a zero-prior state's included, must be a probability vector over the same signals, and the cost finite and ≥ 0. Probability sums may be off by `PROBABILITY_MASS_TOLERANCE` = 1e-12. Accepted probabilities are used as given, never renormalised. EVPI and EVSI are not clamped.
  - Actions are compared by compensated weighted payoff differences, with the product residuals, so a large common payoff does not hide which action is better. EVSI is the direct gain near 0 and EVPI minus the residual regret near EVPI. The two must agree within the mass tolerance plus a floating-point allowance scaled by the information values, and `0 ≤ EVSI ≤ EVPI` must hold, or the evaluation refuses. These are finite-model results in `f64`, not exact arithmetic or calibration evidence.
  - The Monte Carlo estimate is unbiased because its decision rule is the exact Bayes action, which needs finite states and signals. It takes that action from `evaluate`'s per-signal optimal actions, keeping the prior action on a tie. A problem with continuous states or a simulated payoff, where the posterior must itself be estimated, is not covered.
  - The sampler gives a row's shortfall below 1, within the tolerance, to its last positive entry.
  - The gate's power is limited (noncentral χ², scipy). A bias of two SEs of the 100-seed mean in every problem is caught with probability 0.99; that is a fifth of one seed's SE at 2,000 draws. One SE in every problem is caught with probability 0.28. Five SEs in one problem alone is caught with probability 0.77. A smaller bias can pass.
- **`risk`.** It needs at least one value, every one finite, and `alpha` in (0, 1). The tail bounds are `i / n`, so the largest loss always has weight. `test_cvar_coherent_subadditivity_and_monotonicity` covers one pair of eight-point samples.
- **`tropical`.** Arcs must name nodes in `0..n_nodes` and carry finite lags, and the deadline must be finite. Comparisons use `RELAXATION_TOLERANCE` = 1e-12, absolute. A start can be left up to that much short per arc on its path, a positive cycle below it is not detected, and above about 1e4 an ulp exceeds it, so a zero-total cycle whose lags do not cancel exactly can be refused as positive. `test_min_plus_longest_path_duality` covers one four-node DAG.
- **`sparse`.** A solve's accuracy is what the residual known answers and the dense comparisons show, within the tolerances in their tests. There is no cross-host bitwise guarantee.
- **`exact_feas` caps.**
  - 256 cells per component (`MAX_VARIABLES`) and 10⁴ pivots per run (`MAX_PIVOTS`).
  - Blame searches exhaustively only up to 20 cells (`MAX_EXHAUSTIVE_CELLS`), within `MAX_BLAME_RUNS`.
  - Discovery scores footings only up to `MAX_K` = 20 terms, and 2 to `MAX_DOCUMENTS` = 4 documents.
  Past a cap the answer is `Indeterminate`, or the footing is not scored.
- **The certificate checker shares the linearisation.** A fault in the linearisation would pass the checker. The AISC oracle tests in `crates/xtask/tests/public_exact_feas_oracle.rs`, which run in CI, are what catch it.
- **The discovery tails assume independence.** The caller must pass each distinct footing once.

## 9. Oracles
No fixture under `fixtures/` (§33.2).
- **Golden digests.** The two `lib.rs` digests were first observed on aarch64-macos and are pinned in the tests. The CI run recorded in bd-hp2j.3 shows them agreeing on x86_64-linux.
- **Hand and textbook known answers.**
  - `test_ldlt_residual_known_answers` and `test_pcg_residual_known_answers`, which are the lift guard tests.
  - `test_spmv_matches_dense_reference`, `test_ldlt_agrees_with_dense_cholesky` and `test_etree_matches_hand_example`.
  - `test_p0_exact_values` and `test_poisson_binomial_tails_known_answers`.
  - `test_clopper_pearson_known_answers`.
  - `test_gb29_closed_form_known_answers`: plan §22.8's hand computation of GB-29. EV(build) is $100M, EVPI $80M and EVSI $41.0M; the 73% accuracy flip gives $9.8M and the 0.78 prior $2.3M.
  - `test_chi_square_sf_known_answers`: scipy's χ² upper tails, which `scripts/oracle/chi_square_known_answers.py` cross-checks by Simpson integration on CPython's libm. The χ² function is test-only, for GATE-HELM-02.
- **Exhaustive enumeration.** `every_small_discrete_policy_matches_an_independent_exhaustive_oracle` scores all 27 policies by direct sums in the test, sharing no code with `evaluate`.
- **Published-document known answers.** These use values transcribed from issuer documents, and so rest on those transcriptions:
  - `test_e114_turquoise_ridge_certificate` and `test_certificate_digest_golden_e114`;
  - `test_e691_veladero_implied_q`;
  - `test_e144_copper_rollforward_single_cell_correction`;
  - `test_e698_convention_pairs_fixture`, `test_e819_share_interval`, `test_e463_unit_kt_fixture` and `test_e147_deck_found_fixture`.
  The cross-crate AISC tables are checked in `crates/xtask/tests/public_exact_feas_oracle.rs` and `public_exact_feas_blame.rs`.

## 10. Known gaps
- **The four original kernels are not lifts.** Beyond its refusal test, each has one example test of its statistics or algebra (Ville, EVSI monotone, CVaR coherence, duality). `voi` also has GB-29's known answers, the exhaustive policy check and GATE-HELM-02, and `eproc` the score e-process's Ville test with its planted negative. The refusals make bad input visible; they do not make the kernels correct in general. A real lift waits on bd-022.
- **GATE-HELM-02 passes locally only, and bd-227 stays open.**
  - CI has not run it.
  - There is no gate record in `docs/delivery/`.
  - br will not claim bd-227 while bd-022 and bd-225 are open.
- **The Monte Carlo estimator is not nested.** Plan §22.4's general estimator is nested Monte Carlo. This one computes the posterior exactly, which only a finite problem allows (Validity domain).
- **No caller uses the Monte Carlo estimate yet.** `fmine_decide::rank_voi_campaigns` ranks by the closed form and reports no SE or status (G-HELM-6).
- **PO-NUM-1 holds for `lib.rs` only.** Every other module is DeterministicPerIsa, with no cross-host digest.
- **PO-NUM-3** asks for a recorded determinism class and a thread-count test for each kernel on a certified path, kept in `registries/kernels.toml`. That registry does not exist, and no bead covers it.
- **`sparse` has no caller yet.** Its planned consumers are WLS (bd-149), kriging parity (bd-238) and PDLP (bd-251).
- **The registry charter names things not in the crate:** `enkf`, `gmrf` and "the array bridge".
- **BD-340 is open (bd-4wvl).**
  - S5's roll-forward is in the solver, but the generator has no roll-forward block (bd-4wvl.5).
  - The `fmine check tables` verb waits on an owner question (bd-4wvl.8).

## 11. Public API
- **`lib.rs`.**
  - `cholesky_solve(a: &[f64], b: &[f64], n: usize) -> Result<Vec<f64>, Refusal>`.
  - `brent_root(a, b, tol, max_iter, f) -> Result<Root, RootError>`, with `Root { x, f_x, bracket, iterations }`. An unconverged iterate is never returned (bd-t1dk).
  - `norm_cdf`, `norm_inv`. `norm_inv` clamps nothing: bd-hp2j.2 removed a silent clamp to [1e-15, 1 − 1e-15].
  - `NUMERICS_DIGEST_KEY`, `normal_digest_inputs`, `digest_norm_cdf_inv`, `golden_digest_norm_cdf_inv_10k`, `digest_cholesky_brent` and `golden_digest_cholesky_brent_10k` (bd-hp2j.3).
- **`eproc::{BettingEProcess, EProcError}`.** `new(null_mean, alpha) -> Result<Self, EProcError>`, `threshold` (1/α), `observe(u) -> Result<f64, EProcError>` (the wealth) and `rejected_null` (the running maximum wealth ≥ 1/α).
- **`eproc::ScoreBettingEProcess`.** `new(lo, hi, prior_second_moment, truncation, alpha) -> Result<Self, EProcError>`, `stake`, `observe(score) -> Result<f64, EProcError>` (the log wealth), `log_wealth`, `max_log_wealth`, `n_steps`, `log_threshold` (−ln α) and `rejected_null`. Its caller is `fmine_calib::SpatialFrontPitMonitor` (bd-311).
- **`voi::{VoiProblem { state_probs, payoffs }, VoiExperiment { likelihood, cost }, ValidatedVoiProblem, VoiAnalysis, VoiSignalPolicy, VoiSummary, VoiError, PROBABILITY_MASS_TOLERANCE}`**.
  - `VoiProblem`: `validate() -> Result<ValidatedVoiProblem, VoiError>`, `compute_evpi() -> Result<(best_action, prior_ev, evpi), VoiError>`, `compute_evsi(likelihood, cost) -> Result<VoiSummary, VoiError>`, `evaluate(experiment) -> Result<VoiAnalysis, VoiError>` and `estimate_evsi_mc(likelihood, cost, n_samples, key) -> Result<McEvsi, VoiError>`.
  - `ValidatedVoiProblem` borrows the problem immutably and checks it once for many experiments: `evaluate(experiment)` and `estimate_evsi_mc(experiment, n_samples, key)`.
  - `McEvsi { prior_best_action, evsi, se, n_samples, net_voi, status, changes_choice }`, and `VoiStatus::{Positive, Negative, Indeterminate { se }}` with `at_two_se(net, se)`.
  - `VoiError::is_numerical_failure`.
- **`risk::{TailRiskSummary, RiskError}`**, with `compute_cvar(losses, alpha) -> Result<TailRiskSummary, RiskError>` and `max_drawdown(trajectory) -> Result<f64, RiskError>`.
- **`risk::exact::{ExactDistribution, MAX_OUTCOMES}`** (BD-229, PO-CAP-2): exact rational payoff laws for
  per-share objectives and other scalar quantities whose precision cannot be reduced to fixed-decimal money.
  `new(&[(payoff, weight)])` admits 1–4096 outcomes with strictly positive weights summing exactly to one.
  `mean()` returns the exact mean; `quantile(p)` is the weighted inverse CDF for `p` in `(0,1]`;
  `tail_loss(mass)` integrates the worst `mass` of signed loss `-payoff`, including a fractional boundary.
  Both query methods retain the original payoff unit. All-gain populations keep negative tail loss.
  One canonical fallible merge sort uses O(n log n) comparisons and O(n) storage; repeated queries reuse
  that immutable order. Equal payoff atoms combine their exact mass before arithmetic; callers retain
  the original scenario rows. No probability normalization, sampling, floating conversion or monetary rounding
  occurs. Invalid declarations refuse; unrepresentable exact arithmetic refuses with `FMINE-REF-207`.
  Callers own units, complete state identity, feasibility, Claim admission and calibration boundaries.
  Known fractional answers, permutation/mass-splitting invariance, an independent equal-mass expansion,
  singleton cases, bounds and overflow controls verify this kernel.
- **`tropical::{PrecedenceArc { from, to, min_lag }, TropicalDag, TropicalError, RELAXATION_TOLERANCE}`**, with `new`, `add_arc` (checked when a pass runs), `earliest_starts() -> Result<Vec<f64>, TropicalError>` and `latest_starts(deadline) -> Result<Vec<f64>, TropicalError>`.
- **`sparse`** (BD-145):
  - **`Coo::{new, push, staged, assemble}`.** Duplicates accumulate in insertion order after a stable (row, col) sort.
  - **`Csr::{from_parts, nrows, ncols, nnz, row, get, spmv}`.** `from_parts` checks the canonical invariant, strictly ascending columns in each row. `row` and `get` are `None` outside the matrix.
  - **`direct`.**
    - `amd_order`, `SymbolicLdlt::{analyze, n, nnz_l, supernode_count, permutation, symbolic_bytes, estimated_factor_bytes, estimated_flops, factor}` and `LdltFactor::{solve, inertia, stats}`.
    - `LdltOptions`, `DirectOrdering::{Natural, Amd}`, `Inertia` and `FactorStats`.
    - Analyse once per pattern, then factor once per set of values: the K − σM shift ladder.
  - **`precond`.** `Precond`, `IdentityPrecond`, `Ilu0`, `ilu0`, `pcg` and `PcgSolution`. `pcg` returns the iterate only when ‖r‖₂/‖b‖₂ ≤ `tol`.
- **`exact_feas`** (BD-340): exact rounding-interval feasibility over `Rat = fmine_units::ExactQty`.
  - **Cells and conventions.** `Cell`, `Scope`, `Convention::{Closed, HalfUp, HalfDown, HalfEven, Truncate}` and `Convention::interval`, `Interval`, `End` and `Bound`, the last two with open or infinite ends. A zero step marks an exact value, which is a closed point under every convention.
  - **Identities.** `Identity { id, family, closure, kind: Sum | Equal | Scale | Ratio | WeightedMean }`, `Closure`, and `Identity::row` for the linear kinds.
    - `Ratio { rate, num, den, factor }` is linearised when its rate cell is in no other identity and its denominator keeps one sign. Otherwise it is `Indeterminate` ("bilinear").
    - `Identity::roll_forward(id, family, open, moves, close)` gives `close = open + Σ signᵢ·moveᵢ` as a `Sum`.
    - `WeightedMean { rate, parts }` is relaxed by McCormick envelopes. An infeasible relaxation is `Infeasible` with a checked Farkas certificate, and a feasible one is `NotRefuted`.
    - `region_against_members(…, RateCells { rate, weight, numerator })` builds the linear form when every numerator is printed, and one `WeightedMean` otherwise.
  - **`Rules { convention, constructions }`.** `Construction::{ComponentSum, RoundedOnce}` is declared per family before a run. A `Sum` whose family has none is refused. `ComponentSum` requires the printed footing, else `Certificate::Residual`.
  - **Solving.** `solve` and `solve_with_caps` give one `Component` per connected group, with `Outcome::{Feasible { witness }, Infeasible { certificate }, NotRefuted, Indeterminate { reason }}`. `Certificate` is `Farkas { multipliers }` by row id, or `Residual { identity, residual }`.
  - **Checkers.** `check_witness`, `check_certificate` (with `CertificateRange`) and `CheckFailure` are independent of the simplex.
  - **`implied_interval(cells, identities, rules, var, free_own_box) -> Result<Bound, ImpliedError>`.** Each end is re-decided by checked runs. A rate gets its ratio's exact range, by Charnes–Cooper over the closure.
  - **Blame.** `blame`, `blame_with_caps` and `check_conflict_set`, with `Blame`, `BlameOutcome`, `ComponentBlame`, `ConflictSet`, `CorrectionSet` and `CorrectionLabel::{Minimum, InclusionMinimalOnly}`. A correction set gives the implied range, never a corrected value.
  - **Presence conflicts.** `presence_conflicts` uses `Placeholder`, `PresenceConflict`, `Signature` and `SignatureLabel`. An `Equal` between a number and a placeholder is a conflict located on the number.
  - **`certificate_digest`.** The hex BLAKE3 of an `XFCT` v2 frame, independent of input order.
  - **`MAX_VARIABLES`, `MAX_PIVOTS`, `MAX_EXHAUSTIVE_CELLS` and `MAX_BLAME_RUNS`.**
- **`exact_feas::discover`** (BD-340.S4) reports which construction or convention fits. It decides no state.
  - **`p0(k)`.** The exact Irwin–Hall P(|Σeᵢ| < ½), or `None` past `MAX_K`.
  - **`discover_construction(&[Footing { k, residual }]) -> Result<ConstructionReport, UnitsError>`.** `Discovered::{ComponentSum, RoundedOnce, Ambiguous}`, with `ALPHA` = 10⁻⁶ and `MAX_ERROR_RATE` = 1/10.
  - **`discover_tie_conventions(&[Printed]) -> Result<TieDiscovery, TieRefusal>`.** One convention per document, from `STRICT_CONVENTIONS`. It reports `Fit::{Empty, Points, Proper}`, `Assignment`, `TieFinding::{Identified, Ambiguous, NoProperFit}` and the irreconcilable `conflicts`.
  - **`share_interval(c, numerators, denominators, convention) -> Result<Bound, ShareRefusal>`**, over each `Factor { printed, step }`.
  - **`discover_unit(pairs, candidates, convention) -> Result<UnitDiscovery, ShareRefusal>`**, with `UnitPair` and `UnitFinding`.
  - **`discover_deck(&[GeoRelation], &[Deck]) -> Result<DeckDiscovery, ShareRefusal>`**, with `DeckFit` and `DeckFinding::{Found, Ambiguous, NoFit}`. Here geo = gold + copper · (copper price / gold price), in Moz and Mlb. A found deck is not registered.
  - **`evaluate_geo(relation, deck, convention) -> Result<GeoEvaluation, ShareRefusal>`** gives the same exact conversion bounds for a stated deck and convention, `Closed` included. It keeps the converted and reported intervals, strict endpoints and the proper, point-only or empty fit, and chooses no deck.
- **`exact_feas::generator`** (BD-340.S7).
  - **`generate(key, &GenSpec)` and `generate_vintage(key, spec, vintage)`** give a `Generated` or a `GenError`. There is one `Block::{Sum, HalfYear, Breakdowns, Share, Ratio, Reprint, WeightedMean}` per table, with `SUM_FAMILY`, `SHARES` and `MAX_TREE_CELLS`.
  - **`round(x, step, rule)`.** `Closed` is refused.
  - **`Generated::{rules, bracket, witness}`.**
- **`exact_feas::power`** (BD-340.S7).
  - **Injection.** `Injection::{SignFlip, Transposition, Neighbour, WrongShare, StaleVintage, Units}`, with `inject` and `inject_at` giving an `Injected`. `BINS` gives five slack bins.
  - **Detection.** The `Detector` type, `joint_detects`, `baseline_detects`, `joint_only`, `blame_accuracy` (`BlameAccuracy`) and `size1_blame`.
  - **`clopper_pearson(k, n, alpha) -> Option<(f64, f64)>`.**
  - **`power_study(&PowerSpec, Detector) -> Result<PowerTable, PowerError>`**, with `PowerCell`, `KindRow`, `joint_only_by_kind` and `log_lines`.
  - **`KillReport { joint_only_detections, size1, discoveries, implied_consumers }`**, plan K.5 row 3, with `fired`, `pending` and `log_line`. `fired` is computed, never stored.

## 12. Proof obligations
- **PO-NUM-1** (BitwisePortable fixtures give identical digests across both reference ISAs). Discharged for `lib.rs`, the only BitwisePortable file:
  - `test_golden_digest_norm_cdf_inv_10k` and `test_golden_digest_cholesky_brent_10k`;
  - the CI run recorded in bd-hp2j.3.
- **PO-NUM-2** (no std transcendental in a BitwisePortable module). `xtask check-det` checks it over `lib.rs` in `check-all`.
- **PO-NUM-3** (a determinism class and a thread-count test for each kernel on a certified path) is not discharged (Known gaps).
- **PO-HELM-1** (VOI ≥ 0 in expectation; the estimator is unbiased on conjugate toys; known-answer tests). Partly here, for finite problems only: `test_gb29_closed_form_known_answers`, the exhaustive policy check (`every_small_discrete_policy_matches_an_independent_exhaustive_oracle`), and GATE-HELM-02's check of the Monte Carlo mean and SE. No conjugate continuous toy is tested.
- **Mislabelled in the old contract.** It used PO-NUM-1 for `0 ≤ EVSI ≤ EVPI`, PO-NUM-3 for Ville and CVaR subadditivity, and PO-NUM-4 for tropical duality. The plan's PO-NUM-1 and PO-NUM-3 say something else, and it has no PO-NUM-4. Those properties are tested by `test_evpi_non_negative_and_evsi_monotone`, `test_ville_inequality_type1_error_bound`, `test_cvar_coherent_subadditivity_and_monotonicity` and `test_min_plus_longest_path_duality` (Validity domain).
