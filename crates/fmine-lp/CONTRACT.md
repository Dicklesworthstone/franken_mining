# CONTRACT.md — `fmine-lp`

## 1. Purpose
The Pushback linear-programming crate. One slice so far:
- `pdlp` (`BD-251`, plan §15.4): restarted primal–dual hybrid gradient (PDLP) for large sparse LPs, with a certified dual bound (PO-PUSH-4). Plan §15.4 names its uses: multi-site blend networks, SAA extensive forms and CVaR LPs. It is the solver behind KILL-PUSH-PDLP: "1e-4 relative gap on real-size non-closure LPs".

Nothing in the workspace calls it yet. MILP, QP, MPS I/O and decomposition are later slices.

- **Crate:** `fmine-lp`
- **Layer:** E
- **Subsystem:** Pushback
- **LOC budget:** 25,000 planned / 29,000 ceiling
- **Allowed Module Families:** `["pdlp"]`

## 2. Inputs
- `LpProblem::new(c, a, row_lo, row_hi, col_lo, col_hi)`: `minimise c·x subject to row_lo ≤ A x ≤ row_hi and col_lo ≤ x ≤ col_hi`.
  - `a` is a `fmine_numerics::sparse::Csr`.
  - A bound may be infinite. An equality row has `row_lo = row_hi`; a free row has both infinite.
  - A maximisation is passed as the minimisation of `−c`.
- `PdlpOptions`:
  - `tolerance` ε in `(0, 1)`, default 1e-4;
  - `max_iterations`, at least 1, default 1,000,000;
  - `threads`, where 0 means the machine's parallelism, at most 16.
- For `solve_observed`, an observer called at each evaluation (every 64 steps). It returns `false` to stop the run.
- For `certified_bound(problem, y)`, any row multipliers `y`, one finite value per row.

## 3. Outputs
- `PdlpSolution`:
  - `status`: `Optimal` (the stopping rule held), `IterationLimit` or `Stopped`;
  - `x`, one value per column, always within the column bounds;
  - `y`, one multiplier per row;
  - `progress`, the measures at the returned point;
  - `bound`, the `certified_bound` of `y`.
- `Progress`: iterations, restarts, primal and dual objectives, relative primal residual, relative dual residual and relative gap (§4), step size and primal weight.
- `DualBound`:
  - `value`: no feasible `x` has `c·x` below it;
  - `repaired_rows`: the count of multipliers set to zero because their sign needed an infinite row bound.

  `None` means an infinite column bound met a reduced cost of the wrong sign, so this `y` bounds nothing.
- `LpProblem::objective(x)` and `LpProblem::max_violation(x)`: the latter is the largest row or column bound violation, in the problem's units.

None is a Claim.

## 4. Invariants
- **The method.** Applegate et al. (2021), as cuPDLP (Lu and Yang, 2023) runs it:
  - ten rounds of Ruiz equilibration, then one Pock–Chambolle round (α = 1);
  - PDLP's adaptive step rule;
  - every 64 steps, the KKT error of the current point and of the step-weighted average, the better being the restart candidate. A restart happens at 0.2 of the error at the last restart, at 0.8 and rising, or past 0.36 of all steps;
  - the primal weight moves to the geometric mean across each restart.
- **The stopping rule.** In the unscaled problem:
  - `‖primal residual‖₂ ≤ ε(1 + ‖b‖₂)`;
  - `‖dual residual‖₂ ≤ ε(1 + ‖c‖₂)`;
  - `|c·x − dual objective| ≤ ε(1 + |c·x| + |dual objective|)`;

  where `b` holds each row's largest finite bound magnitude. A reduced cost that a finite column bound can absorb is not a dual residual, so on a fully boxed problem the dual residual is 0.
- **The bound is always valid** (PO-PUSH-4). `certified_bound` applies weak duality: `c·x ≥ Σᵢ min_{s∈[loᵢ,hiᵢ]} yᵢ s + Σⱼ min_{t∈[lⱼ,uⱼ]} rⱼ t` with `r = c − Aᵀy`.
  - Multipliers whose sign needs an infinite row bound are zeroed (dual repair).
  - Every `+` and `×` is rounded toward the safe side. Each one's exact error is found by an error-free transformation (TwoSum, and the `mul_add` residual), and the result moves one step only when that error is non-zero. So an exact computation stays exact.
- The bound's checks:
  - `certified_bound_is_valid_at_any_multipliers`: 400 random LPs, half with open column tops, each at 25 random `y` from 10⁻⁶ to 10⁶ in scale. No bound exceeds the optimum found by vertex enumeration, and more than 5,000 bounds are finite;
  - `directed_operations_enclose_the_exact_result`: exact sums and products stay exact. Inexact ones (`0.1 · 3`, `1 ± 2⁻⁶⁰`, overflow past `f64::MAX`, underflow below the smallest subnormal) land on the adjacent double on the safe side;
  - `wrong_signed_multipliers_are_repaired`, `equality_row_and_free_column` (the exact multiplier certifies the optimum 2 exactly; a multiplier off by 10⁻¹² certifies nothing) and `unbounded_columns_certify_only_with_sign_feasible_reduced_costs`.
- The solver against oracles that share no code with it (`tests/common`):
  - `tiny_lps_match_vertex_enumeration`: 300 random LPs with up to 4 columns and 4 rows of mixed kinds, at ε = 10⁻⁹. Each is `Optimal`, its objective within 10⁻⁶ relative of the vertex optimum, `x` within 10⁻⁶ of feasible, and its bound below the optimum and within 10⁻⁶ of it;
  - `closure_lps_match_the_best_closure`: 150 random closure LPs on up to 12 blocks. The value is within 10⁻⁶ relative of the best closure found over all subsets, and the bound is not below it;
  - `two_variable_known_answer`: a hand-solved LP, whose multipliers match the exact duals.
- `infeasible_problems_never_report_optimal`: an infeasible LP runs to the iteration limit.
- `any_thread_count_gives_the_same_bits`: a 72,000-column transport LP (144,000 nonzeros) gives the same iterations, `x` and `y`, bit for bit, at 1, 2 and 5 threads.
- `spmv_matches_the_numerics_csr_bit_for_bit_at_any_piece_count`: the solver's own `u32` CSR product equals `fmine_numerics::sparse::Csr::spmv`.
- Planted bugs, each caught and then reverted:
  - dropping the dual repair (caught by the unit test and the oracle);
  - taking the wrong row bound in the dual objective (caught by the tiny and closure oracles);
  - `||` for `&&` in the stopping rule (caught by five tests).
- **KILL-PUSH-PDLP** (`tests/minelib_cpit.rs`, ignored, run by hand). It runs the LP relaxations of MineLib's CPIT instances, whose optima MineLib publishes (§9). A run ends at the stopping rule or at the 30-minute limit. It passes when its certified bound then lies within 1e-4 relative of the published optimum, allowing one unit for MineLib's rounding. Whether the stopping rule fired is not judged, since it does not bound that gap (§8). §8 has the results.

## 5. Refusals
Every public function returns `Result<_, Refusal>`; none panics on bad input. All are `FMINE-REF-207` (proposed).
- From `LpProblem::new`:
  - a cost, bound or row length that does not match `a`;
  - a cost or matrix entry that is not finite;
  - a NaN bound, a lower bound of `+∞` or above its upper bound, or an upper bound of `−∞`;
  - `u32::MAX` or more rows or columns.
- From `solve` and `solve_observed`:
  - a tolerance outside `(0, 1)`, or `max_iterations` of 0;
  - a step size or iterate that becomes non-finite;
  - a worker thread that panicked.
- From `certified_bound`: multipliers that are not one finite value per row.
- From `objective` and `max_violation`: `x` that is not one value per column.

Tests: `bad_input_is_refused`, one row each. No value is clamped or rescaled to make a problem fit. The solver's scaling is internal, and results are returned unscaled.

## 6. Determinism class
- `pdlp`, `pdlp::engine` and `pdlp::mat` are `DeterministicPerIsa`. They use `sqrt` and `powf`, and every sum runs in a fixed order:
  - each row of a product is summed in ascending column order with `mul_add`;
  - each reduction adds per-chunk partial sums (`CHUNK` = 2¹⁴ entries, at absolute index boundaries) in chunk order.

  So the thread count never changes a bit (`any_thread_count_gives_the_same_bits`). There is no cross-host digest test.
- `pdlp::bound` is `BitwisePortable`: IEEE `+ − ×`, `mul_add`, `next_up` and `next_down` only.

Each is declared on its file and checked by `xtask check-det`.

## 7. Evidence class of outputs
Not applicable: no Claims are emitted, so no `EvidenceClass` or `KnowledgeState` is assigned.

## 8. Validity domain
- **The bound is for the problem as stored in `f64`.** How the caller rounded its own data into `f64` is not bounded here.
- **The stopping rule does not bound the true gap.** The returned `x` is not feasible: it may break rows with a zero right-hand side, such as precedence rows, by whole units. Meanwhile the residual, relative to `‖b‖`, which large capacity rows dominate, stays below ε. The primal objective can then sit above the true optimum (of the maximisation), and the dual objective with it.
  - At ε = 1e-4, kd's certified bound was 1.05% above MineLib's optimum, with the relative gap at 4.9e-5.
  - At ε = 1e-6 the bound was within 3.2e-5.

  The bound's distance from the optimum is the measure to trust. It can be judged only against an outside optimum or a feasible point, and this crate produces neither.
- **Unbounded columns.** A column with an infinite bound certifies only when its reduced cost has the right sign, exactly. An approximate `y` therefore usually gives no bound when any column is unbounded on the side its reduced cost points to. Give such columns a finite bound when one is known.
- **Memory**, counted from the code and not measured:
  - about 48 bytes per nonzero, because `A` and `Aᵀ`, as a `u32` index and an `f64` value, are held by `LpProblem` and again, scaled, by the solver;
  - about 14 `f64` vectors of column length and 12 of row length;
  - the caller's own `Csr`.
- **Wall times.** These are local runs of `tests/minelib_cpit.rs`, not a benchmark suite: aarch64-macos, 36 cores, release, 16 threads, 2026-11-01, on a host at load 56 to 109 from other work. "Gap" is the certified bound's relative distance above MineLib's published CPIT LP optimum.

  | Instance | Rows | Columns | Nonzeros | ε = 1e-4: steps, solve, gap | ε = 1e-6: steps, solve, gap |
  |---|---|---|---|---|---|
  | newman1 | 28,844 | 6,360 | 75,616 | 256, 0.1 s, 2.8e-4 | 2,112, 0.7 s, 2.8e-7 |
  | kd | 2,793,031 | 169,836 | 5,722,451 | 704, 3.2 s, 1.05e-2 | 38,528, 364.8 s, 3.2e-5 |
  | zuck_small | 3,091,440 | 188,000 | 6,738,901 | 9,664, 49.6 s, 2.1e-5 | 27,776, 278.7 s, 9.3e-6 |
  | sm2 | 5,770,726 | 2,970,420 | 18,047,203 | 8,896, 88.9 s, 3.8e-4 | 81,024, 1,387.4 s, 5.8e-6 |
  | p4hd | 7,754,633 | 409,470 | 16,712,572 | 3,520, 32.0 s, 1.8e-5 | 16,896, 214.7 s, 1.3e-5 |
  | w23 | 9,994,328 | 891,120 | 22,183,244 | 2,432, 26.8 s, 1.9e-4 | 10,240, 181.3 s, 1.7e-5 |
  | marvin | 14,024,809 | 1,065,420 | 30,317,388 | 7,040, 96.8 s, 4.7e-5 | 31,680, 678.5 s, 2.1e-5 |
  | zuck_medium | 19,478,013 | 439,155 | 40,139,050 | 9,536, 232.1 s, 1.3e-5 | stopped at 30 min: 65,088, 1,801.3 s, 3.0e-6 |
  | zuck_large | 34,401,019 | 2,904,630 | 77,728,028 | 6,016, 222.8 s, 7.1e-5 | stopped at 30 min: 47,744, 1,801.6 s, 3.3e-5 |
  | mclaughlin_limit | 47,109,878 | 1,690,305 | 95,145,725 | 2,560, 149.2 s, 1.1e-4 | stopped at 30 min: 34,752, 1,802.2 s, 7.8e-6 |

  Gap is `(bound − published) / published`, and every bound lay above the published optimum.
  - At ε = 1e-4 every run reached the stopping rule (`Optimal`), and five of ten bounds are within KILL-PUSH-PDLP's 1e-4.
  - At ε = 1e-6 seven runs reached the stopping rule and three were stopped by the 30-minute limit. All ten bounds are within 1e-4: the largest gap is 3.3e-5.
  - kd at 1e-6 ran on its own, alongside the 1e-4 run. The other 1e-6 rows are one process, `tests/minelib_cpit.rs` built from `f920f73`.
  - The returned `x` broke some row by up to 1.7×10⁴ units at 1e-4 (w23), and by 5.3 (newman1) to 269 units (zuck_small) at 1e-6.

  Plan §15.4's scale TARGET is 10⁷ nonzeros to a 1e-4 relative gap within 30 minutes, with a bound. At ε = 1e-6 the certified bound meets it on all seven instances above 10⁷ nonzeros (1.7×10⁷ to 9.5×10⁷). At ε = 1e-4 the solver's own stopping rule is met on all seven in under four minutes each, but the certified bound misses 1e-4 on three of them: sm2, w23 and mclaughlin_limit. ε = 1e-6 was chosen after the 1e-4 run, on the same instances; there is no held-out set. `docs/delivery/KILL-PUSH-PDLP.md` records the decision.
- MineLib's CPIT LPs are one family: 0–1 boxes, precedence and monotonicity rows, and one to three capacity rows per period. Blend networks, SAA extensive forms and CVaR LPs, the uses plan §15.4 names, are not tested at scale.

## 9. Oracles
- Vertex enumeration over every choice of tight constraints, for tiny bounded LPs. Brute force over all subsets, for closure LPs, whose relaxation is integral. Both are in `tests/common`, share no code with `pdlp`, and have the same author, so their agreement is same-origin evidence.
- External: MineLib's published "CPIT LP upper bound: objective value of the optimal LP relaxation". The source is Espinoza, Goycoolea, Moreno and Newman, MineLib, Annals of Operations Research 2013; the Results page is mirrored at github.com/borelian/minelib and was read 2026-11-01.
  - The LP is built in "by" variables (`w[b][t]` is the fraction of block `b` mined by period `t`), which keeps the optimum.
  - The published optima match a discount exponent counted from 0. On newman1 at ε = 1e-7 that gives 24,486,185.05 against the published 24,486,184. Counting from 1, as the format note reads, gives 22,672,394.94.
- HiGHS and OR-Tools PDLP (plan §15.4's lab oracles) are not run.

## 10. Known gaps
- `BD-251`: no caller in the workspace yet.
- No presolve, no feasibility polishing and no infeasibility or unboundedness detection. An infeasible or unbounded LP runs to `max_iterations`.
- The returned `x` is not feasible (§8). Nothing here turns it into a feasible point or a primal bound.
- No implied-bound repair, so unbounded columns rarely certify (§8).
- At the default ε = 1e-4, the certified bound misses KILL-PUSH-PDLP's 1e-4 on five of ten MineLib instances (§8). The default is cuPDLP's and is kept, and the MineLib test runs at 1e-6.
- At 1e-6 the three largest MineLib instances do not meet the stopping rule within 30 minutes. Their bounds were within 1e-4, but the solver cannot tell that without an outside optimum or a feasible point.
- No MPS I/O, QP, MILP or decomposition (the crate's other planned slices).
- MineLib's files are not in the repository, because their licence is not recorded. KILL-PUSH-PDLP is an ignored test, run by hand with `FMINE_MINELIB_DIR`, and CI does not run it.
- MineLib's mclaughlin (2.1 million blocks, 73 million arcs, 20 periods) is not run. It has about 2.9×10⁹ nonzeros in this formulation: about 140 GB at §8's rate for this crate alone, plus the test's own COO and CSR copies. It was not attempted on this shared 256 GB host.
- The timings come from one loaded machine.

## 11. Public API
- `pdlp::LpProblem`: `new`, `rows`, `cols`, `nnz`, `objective`, `max_violation`
- `pdlp::PdlpOptions` (`tolerance`, `max_iterations`, `threads`; `Default`)
- `pdlp::PdlpStatus` (`Optimal`, `IterationLimit`, `Stopped`)
- `pdlp::Progress` (public fields)
- `pdlp::PdlpSolution`: `status`, `x`, `y`, `progress`, `bound`
- `pdlp::solve`, `pdlp::solve_observed`
- `pdlp::DualBound`: `value`, `repaired_rows`
- `pdlp::certified_bound`

## 12. Proof obligations
- PO-PUSH-4 ("Every dual bound is valid"; plan: "Dual feasibility verified with outward interval rounding, then weak duality") is discharged by `pdlp::bound`.
  - The weak-duality inequality and the rounding argument are in its module documentation and are not machine-checked.
  - `certified_bound_is_valid_at_any_multipliers` and `directed_operations_enclose_the_exact_result` check it on random cases.
- No other PO-* is discharged here. That PDHG with restarts converges is Applegate et al.'s and Lu and Yang's result, not proved or machine-checked here.
