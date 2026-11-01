# CONTRACT.md — `fmine-numerics`

## 1. Purpose
The K0 numerical kernels that the domain crates call:
- in `lib.rs`: a dense SPD Cholesky solve, Brent's bracketed root-finder, and the standard normal CDF and inverse CDF;
- an anytime-valid betting e-process (`eproc`), EVPI and EVSI (`voi`), empirical VaR and CVaR (`risk`), and `(max, +)` / `(min, +)` precedence scheduling (`tropical`);
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
  - `voi`: state probabilities and a `payoffs[state][action]` matrix (its doc comment says USD millions), a `likelihood[state][signal]` matrix and a sample cost.
  - `risk`: a slice of losses and `alpha`.
  - `eproc`: PIT values in [0, 1].
  - `tropical`: node counts and arcs with `f64` minimum lags.
- **`sparse`.** COO triplets `(row, col, value)`, CSR parts, `LdltOptions`, right-hand sides, a PCG tolerance and an iteration limit.
- **`exact_feas`.** Cells with exact printed values and rounding steps (`Rat = fmine_units::ExactQty`), identities over cell ids, and `Rules`, each declaring a convention and one construction per family. The discovery reports take footings, printed values, factors, unit pairs and price decks. The generator takes a Philox key and a `GenSpec`.
- **No files.** It reads no files or registries.

## 3. Outputs
None is a Claim; callers wrap results.
- **Dense and original kernels.**
  - `cholesky_solve` gives `Result<Vec<f64>, Refusal>`, and `brent_root` gives `Result<Root, RootError>`.
  - `norm_cdf` and `norm_inv` give an `f64`.
  - `BettingEProcess` gives its wealth and `rejected_null`.
  - `voi` gives `VoiSummary` and `risk` gives `TailRiskSummary`.
  - `TropicalDag` gives start-time vectors, or `None` on a positive cycle.
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
  - `ImpliedError::{Refused, Infeasible, Unbounded, Indeterminate}`, `TieRefusal`, `ShareRefusal::{NotPositive, NegativeStep, Interval}`, `GenError`, `PowerError` and `IntervalError`.
- **No refusal at all.** `TropicalDag` gives `None` on a positive precedence cycle. `eproc`, `voi` and `risk` clamp or default instead of refusing (Validity domain; bd-a482).

## 6. Determinism class
Each file's `//! Determinism:` header:
- `lib.rs`: **BitwisePortable**. `norm_cdf` and `norm_inv` delegate to `fmine_core::det` (bd-hp2j.2). `brent_root` and `cholesky_solve` use only `+ − × ÷` and `sqrt`. bd-hp2j.3 records that both golden digests agreed on ubuntu-latest (x86_64) and macos-15 (aarch64) in CI run 41237993223 at 84f32ac.
- `exact_feas.rs` and `exact_feas/generator.rs`: **Exact**. They use rational arithmetic, and the generator's draws are Philox integers.
- `exact_feas/discover.rs` and `exact_feas/power.rs`: **DeterministicPerIsa**. The counts are exact, but the tails and the Clopper–Pearson bounds are `f64` through `fmine_core::det`.
- `eproc.rs`, `voi.rs`, `risk.rs`, `tropical.rs`, `sparse.rs`, `sparse/direct.rs` and `sparse/precond.rs`: **DeterministicPerIsa**. None has a cross-host digest test. The `sparse` files repeat bitwise on one host (Invariants).

## 7. Evidence class of outputs
Not applicable: no Claims are emitted. A caller that wraps a result in a Claim decides its class and state.

## 8. Validity domain
- **`norm_cdf`** is Abramowitz & Stegun 7.1.26 on `erf`, with absolute error below 1.5 × 10⁻⁷ in `erf`. **`norm_inv`** is Acklam's rational approximation. Neither is exact in the tails.
- **`cholesky_solve`** refuses a pivot ≤ 1e-14 in absolute terms, whatever the matrix's scale.
- **`brent_root`** needs a sign change on `[a, b]` and a finite `f` at every point it evaluates.
- **`eproc`.** `BettingEProcess::new` clamps `null_mean` into [0.01, 0.99] and `alpha` into [1e-6, 0.5], and `observe` clamps `u` into [0, 1]. The bet fraction is clamped to ±0.75, while the doc comment says ±0.95. Its null is two-sided, `E[U_t] = μ0`, while the struct's doc comment says `≤`. `test_ville_inequality_type1_error_bound` checks the false-alarm rate against α + 0.02 over 500 seeded runs, plus one alternative.
- **`voi`.** An empty problem gives zeros. Ragged payoffs, or fewer payoff rows than states, panic on an index. Probabilities are not checked. A likelihood that does not match the states gives EVSI 0, and EVSI is clamped into [0, EVPI]. `test_evpi_non_negative_and_evsi_monotone` covers one two-state problem.
- **`risk`.** No losses gives zeros. `alpha` is clamped into [1e-6, 1 − 1e-6], and a NaN loss makes the mean and the CVaR NaN. `max_drawdown` of an empty slice is 0. `test_cvar_coherent_subadditivity_and_monotonicity` covers one pair of eight-point samples.
- **`tropical`.** An arc naming a node ≥ `n_nodes` panics on an index. Comparisons use a fixed 1e-12 tolerance, and a NaN lag is not refused. `test_min_plus_longest_path_duality` covers one four-node DAG. No test reaches the positive-cycle `None`.
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
- **Published-document known answers.** These use values transcribed from issuer documents, and so rest on those transcriptions:
  - `test_e114_turquoise_ridge_certificate` and `test_certificate_digest_golden_e114`;
  - `test_e691_veladero_implied_q`;
  - `test_e144_copper_rollforward_single_cell_correction`;
  - `test_e698_convention_pairs_fixture`, `test_e819_share_interval`, `test_e463_unit_kt_fixture` and `test_e147_deck_found_fixture`.
  The cross-crate AISC tables are checked in `crates/xtask/tests/public_exact_feas_oracle.rs` and `public_exact_feas_blame.rs`.

## 10. Known gaps
- **Clamps and made-up defaults in `eproc`, `voi`, `risk` and `tropical`** (Validity domain): bd-a482. `fmine-decide` (`rank_voi_campaigns`) and `fmine-calib` (`SpatialFrontPitMonitor`) call two of them.
- **The four original kernels are not lifts.** Each has one example test, and no test reaches `tropical`'s cycle refusal. A real lift waits on bd-022.
- **GATE-HELM-02 is not met here.** It needs a Monte Carlo VOI estimator to recover closed-form EVSI over K problems by a χ² test. `voi` has only the closed form: bd-227.
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
- **`eproc::BettingEProcess`** has `new(null_mean, alpha)`, `threshold` (1/α), `observe(u) -> f64` (the wealth) and `rejected_null` (the running maximum wealth ≥ 1/α).
- **`voi::{VoiProblem { state_probs, payoffs }, VoiSummary}`**, with `compute_evpi() -> (best_action, prior_ev, evpi)` and `compute_evsi(likelihood, cost) -> VoiSummary`.
- **`risk::{TailRiskSummary, compute_cvar(losses, alpha), max_drawdown}`.**
- **`tropical::{PrecedenceArc { from, to, min_lag }, TropicalDag}`**, with `new`, `add_arc`, `earliest_starts() -> Option<Vec<f64>>` and `latest_starts(deadline) -> Option<Vec<f64>>`.
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
- **Mislabelled in the old contract.** It used PO-NUM-1 for `0 ≤ EVSI ≤ EVPI`, PO-NUM-3 for Ville and CVaR subadditivity, and PO-NUM-4 for tropical duality. The plan's PO-NUM-1 and PO-NUM-3 say something else, and it has no PO-NUM-4. Those properties are tested by `test_evpi_non_negative_and_evsi_monotone`, `test_ville_inequality_type1_error_bound`, `test_cvar_coherent_subadditivity_and_monotonicity` and `test_min_plus_longest_path_duality` (Validity domain).
