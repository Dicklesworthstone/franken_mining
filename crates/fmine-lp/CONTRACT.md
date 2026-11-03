# CONTRACT.md — `fmine-lp`

## 1. Purpose
The Pushback linear-programming crate. Two slices so far:
- `pdlp` (`BD-251`, plan §15.4): restarted primal–dual hybrid gradient (PDLP) for large sparse LPs, with a certified dual bound (PO-PUSH-4). Plan §15.4 names its uses: multi-site blend networks, SAA extensive forms and CVaR LPs. It is the solver behind KILL-PUSH-PDLP: "1e-4 relative gap on real-size non-closure LPs".
- `mps` (bd-j2ay, plan §15.4's "MPS I/O"): free-format MPS read into an `LpProblem` and written from one, so an LP can go to HiGHS, Gurobi or a client's solver, and a standard test LP can come in.

`fmine-io`'s OMF schedules (bd-djb7) call `pdlp`. MILP, QP and decomposition are later slices.

- **Crate:** `fmine-lp`
- **Layer:** E
- **Subsystem:** Pushback
- **LOC budget:** 25,000 planned / 29,000 ceiling
- **Allowed Module Families:** `["pdlp", "mps"]`

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
- `mps::read(text)`: free-format MPS text. Its sections are `NAME`, `OBJSENSE` (`MIN` or `MAX`), `ROWS` (`N`, `L`, `G`, `E`), `COLUMNS`, `RHS`, `RANGES` and `BOUNDS` (`UP`, `LO`, `FX`, `FR`, `MI`, `PL`), then `ENDATA`. The module notes give each rule:
  - the first `N` row is the objective; a right-hand side `b` on it is the objective constant `−b`;
  - a range `R` on a row with right-hand side `b`: `G` gives `[b, b + |R|]`, `L` gives `[b − |R|, b]`, and `E` gives `[b, b + R]` or `[b + R, b]` by the sign of `R`;
  - a column's default bounds are `[0, +∞)`.
- `mps::write(name, problem)`: any `LpProblem`, under a name with no space or tab.

## 3. Outputs
- `PdlpSolution`:
  - `status`: `Optimal` (the stopping rule held), `IterationLimit` or `Stopped`;
  - `x`, one value per column, always within the column bounds;
  - `y`, one multiplier per row;
  - `progress`, the measures at the returned point;
  - `bound`, the highest `certified_bound` of up to four multipliers, a tie going to the earlier:
    - `y`;
    - those with the highest dual objective seen at any evaluation (the current point and the average, every 64 steps). The dual objective is not monotone along a run, so these can be the tighter;
    - each of the two moved by the shift for open columns (§4, bd-tcp4). On a boxed problem nothing is moved;
  - `bound_multipliers`, the multipliers `bound` came from: `certified_bound(problem, bound_multipliers)` gives it again.
- `Progress`: iterations, restarts, primal and dual objectives, relative primal residual, relative dual residual and relative gap (§4), step size and primal weight.
- `DualBound`:
  - `value`: no feasible `x` has `c·x` below it;
  - `repaired_rows`: the count of multipliers set to zero because their sign needed an infinite row bound;
  - `implied_columns`: the count of columns whose infinite bound met a reduced cost of the wrong sign, and whose term used a bound the rows imply instead (bd-tcp4).

  `None` means an infinite column bound met a reduced cost of the wrong sign, and the rows imply no finite bound on that side either, so this `y` bounds nothing.
- `LpProblem::objective(x)` and `LpProblem::max_violation(x)`: the latter is the largest row or column bound violation, in the problem's units.
- `mps::MpsModel`: the `NAME` text, the objective row's name, the row and column names in file order, whether the file maximised, the objective constant in the file's sense, and the `LpProblem`. A maximisation is stored as the minimisation of `−c`.
- `mps::write`'s text: rows `R0`, `R1`, … under the objective `OBJ`, columns `C0`, `C1`, …, numbers in their shortest round-trip form. A row with two different finite bounds is written as `G` or `L` with a range, whichever reads back exactly.

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
  - **Implied bounds** (bd-tcp4). Where a column's infinite bound meets a reduced cost of the wrong sign, its term uses a finite bound that the rows imply for every feasible `x` instead:
    - row `i` gives `aᵢⱼxⱼ ≤ hiᵢ − Σₖ≠ⱼ min aᵢₖxₖ` and `≥ loᵢ − Σₖ≠ⱼ max aᵢₖxₖ` over the column bounds;
    - a bound once found feeds later rows, for up to 16 passes;
    - each sum, product and quotient is rounded outward too, a quotient stepping by the sign of its exact remainder `mul_add(q, b, −a)`.

    Only such a column is touched, so a boxed problem's bound does not change by a bit.
  - **The shift for open columns** (bd-tcp4) is a heuristic whose output is certified like any `y`. Some columns are open on one side with no bound the rows imply, and have a reduced cost on the wrong side of a margin (10⁻¹¹ of `1 + |cⱼ| + Σᵢ |aᵢⱼ yᵢ|`). For each, one row's multiplier moves to set that reduced cost to the margin. The row is the one of largest `|aᵢⱼ|` whose new multiplier keeps a sign its sides allow, and that pushes no other such column past its margin. A column open on both sides with no implied bound is never moved.
- The bound's checks:
  - `certified_bound_is_valid_at_any_multipliers`: 400 random LPs, half with open column tops, each at 25 random `y` from 10⁻⁶ to 10⁶ in scale. No bound exceeds the optimum found by vertex enumeration, and more than 5,000 bounds are finite. 9,573 of 10,000 were finite with implied bounds; with them switched off, 8,726;
  - `implied_column_bounds_hold_at_every_feasible_point`: 300 random LPs with open tops, and each mirrored (`x → −x`) so the tops become bottoms. For each open column `j`, `c = ±eⱼ` and `y = 0` make the certified bound minus the implied bound. That bound may not exceed the LP's own optimum, found by vertex enumeration with open bounds cut at 10⁶, so an LP unbounded in `xⱼ` fails any finite implied bound. 254 implied bounds were checked and 226 columns had none;
  - `rows_imply_bounds_for_open_columns_only`: hand cases of one round, two rounds, a negative entry, a finite bound left alone, and a row that implies nothing;
  - `directed_quotients_enclose_the_exact_result`: exact quotients stay exact, `1/3` and `1/10` land on the right adjacent doubles, and a quotient near underflow steps either way;
  - `open_columns_certify_through_the_tops_their_rows_imply` and `the_shift_moves_only_what_it_can_certify`: a wrong-signed reduced cost met by an implied top, and the shift fixing one where the rows imply nothing. The shift leaves boxed problems alone, never moves a free column with no implied bound, and never gives a row a multiplier its sides forbid (on the unbounded `min −x₀, x₀ ≥ 1` it makes nothing);
  - `directed_operations_enclose_the_exact_result`: exact sums and products stay exact. Inexact ones (`0.1 · 3`, `1 ± 2⁻⁶⁰`, overflow past `f64::MAX`, underflow below the smallest subnormal) land on the adjacent double on the safe side;
  - `wrong_signed_multipliers_are_repaired` and `equality_row_and_free_column`. In the latter, the exact multiplier certifies the optimum 2 exactly, and a multiplier off by 10⁻¹² certifies `2 − 5×10⁻¹²` through the top the row implies for the free column.
- The solver against oracles that share no code with it (`tests/common`):
  - `tiny_lps_match_vertex_enumeration`: 300 random LPs with up to 4 columns and 4 rows of mixed kinds, at ε = 10⁻⁹. Each is `Optimal`, its objective within 10⁻⁶ relative of the vertex optimum, `x` within 10⁻⁶ of feasible, and its bound below the optimum and within 10⁻⁶ of it;
  - `closure_lps_match_the_best_closure`: 150 random closure LPs on up to 12 blocks. The value is within 10⁻⁶ relative of the best closure found over all subsets, and the bound is not below it;
  - `two_variable_known_answer`: a hand-solved LP, whose multipliers match the exact duals.
- `bound_is_the_best_seen_and_reproducible_from_its_multipliers`: 240 small LPs (dense, closure, and CPIT-shaped from `random_cpit`), each run for 12 evaluations. The bound equals `certified_bound` of `bound_multipliers`, is not below that of the final `y`, and is not below the final `y`'s bound of a run cut at any earlier evaluation, less 10⁻⁹ relative for rounding. The solver is deterministic, so a shorter run's final point is one the longer run evaluated. In 18 of the 240 the final `y` alone certified less than an earlier evaluation, and the test requires at least one such case.
- `infeasible_problems_never_report_optimal`: an infeasible LP runs to the iteration limit.
- `any_thread_count_gives_the_same_bits`: a 72,000-column transport LP (144,000 nonzeros) gives the same iterations, `x` and `y`, bit for bit, at 1, 2 and 5 threads.
- `spmv_matches_the_numerics_csr_bit_for_bit_at_any_piece_count`: the solver's own `u32` CSR product equals `fmine_numerics::sparse::Csr::spmv`.
- Planted bugs, each caught and then reverted:
  - dropping the dual repair (caught by the unit test and the oracle);
  - taking the wrong row bound in the dual objective (caught by the tiny and closure oracles);
  - `||` for `&&` in the stopping rule (caught by five tests);
  - returning the final `y` as `bound_multipliers` when the best-seen bound wins (caught by the reproducibility check);
  - never updating the best-seen multipliers. The dense and closure LPs alone did not catch it, because their final multipliers were always the best; the CPIT-shaped ones do (case 11).
- **MPS** (`mps::tests`):
  - `every_section_and_bound_type_reads_as_written`: one file with every section, row type, range sign and bound type, tab-led lines, set names given and left out, and an objective constant reads to a hand-built `LpProblem`, bit for bit;
  - `a_written_problem_reads_back_bit_for_bit`: that problem, and one of tenths, ±10³⁰⁰, a subnormal, signed zeros and a free row, read back from `write` bit for bit. A row whose range reads back exactly as neither `G` nor `L` (`[1, 2⁵³ + 2]`) is refused;
  - `each_defect_is_refused_naming_its_line`: 34 one-defect files, each refused with its line and reason, and `LpProblem::new`'s own refusal passed through;
  - `a_small_lp_solved_from_its_mps_text_certifies_its_hand_optimum`: `max 3x + 2y` over two rows, read and solved at ε = 1e-9, certifies a bound at or below −11 and within 1e-6 of it.
  - Planted bug: the `E` row's range sign rule reversed. The every-section test caught it, and it was reverted.
- **KILL-PUSH-PDLP** (`tests/minelib_cpit.rs`, ignored, run by hand). It runs the LP relaxations of MineLib's CPIT instances, whose optima MineLib publishes (§9). A run ends at the stopping rule or at the 30-minute limit. It passes when its certified bound then lies within 1e-4 relative of the published optimum, allowing one unit for MineLib's rounding. Whether the stopping rule fired is not judged, since it does not bound that gap (§8). §8 has the results.

## 5. Refusals
Every public function returns `Result<_, Refusal>`; none panics on bad input. `mps`'s own are `FMINE-REF-506` (proposed), naming the line; all others are `FMINE-REF-207` (proposed).
- From `mps::read`:
  - an unknown section, a data line outside a section, no `ENDATA`, or text after it;
  - a row type other than `N`, `L`, `G`, `E`; a row named twice; a row or column no line declared; a column whose lines are not contiguous;
  - integer markers and integer bound types (`BV`, `LI`, `UI`, `SC`): this reads LPs;
  - a second entry for one row and column, a second objective entry, right-hand side or range for one row; a right-hand side on a free row; a range on an `N` row; a range that overflows;
  - a second `RHS`, `RANGES` or `BOUNDS` set;
  - a negative `UP` bound on a column whose lower bound is still the default 0, which MPS readers take differently;
  - a field that is not a finite number, or a line with the wrong number of fields;
  - `LpProblem::new`'s refusals, passed through as 207.
- From `mps::write`: a name holding a space or tab, and a row whose two finite bounds no range reads back exactly.
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
- `pdlp::bound` is `BitwisePortable`: IEEE `+ − × ÷`, `mul_add`, `next_up` and `next_down` only, in a fixed order.
- `mps` is `BitwisePortable`: Rust's correctly rounded decimal parse and shortest round-trip print, negation, and the range sums, each checked exact when written.

Each is declared on its file and checked by `xtask check-det`.

## 7. Evidence class of outputs
Not applicable: no Claims are emitted, so no `EvidenceClass` or `KnowledgeState` is assigned.

## 8. Validity domain
- **The bound is for the problem as stored in `f64`.** How the caller rounded its own data into `f64` is not bounded here.
- **The stopping rule does not bound the true gap.** The returned `x` is not feasible: it may break rows with a zero right-hand side, such as precedence rows, by whole units. Meanwhile the residual, relative to `‖b‖`, which large capacity rows dominate, stays below ε. The primal objective can then sit above the true optimum (of the maximisation), and the dual objective with it.
  - At ε = 1e-4, kd's certified bound was 1.05% above MineLib's optimum, with the relative gap at 4.9e-5.
  - At ε = 1e-6 the bound was within 3.2e-5.

  The bound's distance from the optimum is the measure to trust. It can be judged only against an outside optimum or a feasible point, and this crate produces neither.
- **Open columns.** A column with an infinite bound on the side its reduced cost points to certifies through a bound its rows imply (§4). Where they imply none, it certifies only when its reduced cost has the right sign, exactly. The shift (§4) tries to give it that sign. A free column with no implied bound needs a reduced cost of exactly zero, which an approximate `y` rarely has. Give such columns a finite bound when one is known.
- **netlib** (`tests/netlib.rs`, ignored, run by hand on 2026-11-03 from a local debug build). Four netlib LPs, all with every column open above, at ε = 1e-9 and one thread. The gap is `(published − bound) / |published|`, against netlib's MINOS 5.3 optima. Every bound lay at or below the published optimum.

  | LP | Rows | Columns | Nonzeros | Steps | Columns on an implied bound | Gap |
  |---|---|---|---|---|---|---|
  | AFIRO | 27 | 32 | 83 | 640 | 7 | 6.9e-11 |
  | ADLITTLE | 56 | 97 | 383 | 11,712 | 34 | 2.0e-10 |
  | E226 | 223 | 282 | 2,578 | 44,032 | 56 | 3.3e-8 |
  | ISRAEL | 174 | 142 | 2,269 | 19,008 | 64 | 1.2e-8 |

  - Before bd-tcp4, none of the four had a finite bound at its final `y`.
  - With implied bounds and no shift, ADLITTLE's and E226's final `y` still certified nothing. A column with no implied top had a wrong-signed reduced cost. Their bounds came from the best-seen multipliers, with gaps of 0.67 and 4.6.
  - E226's objective row has the right-hand side −7.113, so its objective has the constant 7.113. netlib's optimum leaves it out: the certified bound on `c·x` alone is within 3.3e-8 of it.
- **Memory**, counted from the code and not measured:
  - about 48 bytes per nonzero, because `A` and `Aᵀ`, as a `u32` index and an `f64` value, are held by `LpProblem` and again, scaled, by the solver;
  - about 14 `f64` vectors of column length and 12 of row length;
  - the caller's own `Csr`.
- **Wall times.** These are local runs of `tests/minelib_cpit.rs` built at `9b9fe28`, not a benchmark suite: aarch64-macos, 36 cores, release, 16 threads, 2026-11-02, on a host loaded by other work.
  - "Gap" is the certified bound's relative distance above MineLib's published CPIT LP optimum, `(bound − published) / published`. Every bound lay above the published optimum.
  - "Dual within 1e-4" is when the unrounded dual objective first came within 1e-4 of the published optimum. It is diagnostic only, since it needs the answer.

  | Instance | Rows | Columns | Nonzeros | ε = 1e-4: steps, solve, gap | ε = 1e-6: end, steps, solve, gap | Dual within 1e-4 (ε = 1e-6) |
  |---|---|---|---|---|---|---|
  | newman1 | 28,844 | 6,360 | 75,616 | 256, 0.1 s, 2.8e-4 | rule, 2,112, 0.6 s, 2.8e-7 | 0.1 s |
  | kd | 2,793,031 | 169,836 | 5,722,451 | 704, 5.4 s, 1.05e-2 | rule, 38,528, 307.4 s, 3.2e-5 | 117.4 s |
  | zuck_small | 3,091,440 | 188,000 | 6,738,901 | 9,664, 82.8 s, 2.1e-5 | rule, 27,776, 190.1 s, 9.3e-6 | 29.4 s |
  | sm2 | 5,770,726 | 2,970,420 | 18,047,203 | 8,896, 126.2 s, 3.8e-4 | rule, 81,024, 1,161.8 s, 5.8e-6 | 182.2 s |
  | p4hd | 7,754,633 | 409,470 | 16,712,572 | 3,520, 46.5 s, 1.8e-5 | rule, 16,896, 274.3 s, 1.3e-5 | 11.8 s |
  | w23 | 9,994,328 | 891,120 | 22,183,244 | 2,432, 42.6 s, 1.9e-4 | rule, 10,240, 184.7 s, 1.7e-5 | 37.8 s |
  | marvin | 14,024,809 | 1,065,420 | 30,317,388 | 7,040, 150.3 s, 4.7e-5 | rule, 31,680, 828.1 s, 2.1e-5 | 67.9 s |
  | zuck_medium | 19,478,013 | 439,155 | 40,139,050 | 9,536, 325.7 s, 1.3e-5 | limit, 69,120, 1,801.2 s, 3.0e-6 | 271.5 s |
  | zuck_large | 34,401,019 | 2,904,630 | 77,728,028 | 6,016, 355.5 s, 7.1e-5 | limit, 47,936, 1,803.1 s, 3.4e-5 | 175.6 s |
  | mclaughlin_limit | 47,109,878 | 1,690,305 | 95,145,725 | 2,560, 171.6 s, 1.1e-4 | limit, 32,768, 1,801.8 s, 7.8e-6 | 110.9 s |

  - **At ε = 1e-4** every run reached the stopping rule, and five of ten bounds are within KILL-PUSH-PDLP's 1e-4.
  - **At ε = 1e-6** seven runs reached the rule and three were stopped by the 30-minute limit. All ten bounds are within 1e-4; the largest gap is 3.4e-5.
  - **Reproducible.** Earlier runs at `f920f73` gave the same steps and bounds on every run that ended at the rule, at both ε. That includes kd at 1e-6 run with 8 threads against 16 here. Runs stopped by the limit end at a step count that depends on speed.
  - **Primal violation.** The returned `x` broke some row by up to 1.7×10⁴ units at 1e-4 (w23), and by up to 671 at 1e-6 (zuck_large).
  - **The table's bounds are the final `y`'s only.** It predates best-seen tracking (§3). On w23 at 1e-4 the dual objective came within 1e-4 at 32 s, but the final `y` was 1.9e-4 off.
  - **Best-seen bound.** With tracking added, the five instances that missed 1e-4 at ε = 1e-4 were run again, the same way, on 2026-11-02. Every run took the same steps as in the table:

    | Instance | Gap, final `y` (table) | Gap, best seen |
    |---|---|---|
    | newman1 | 2.8e-4 | 1.3e-4 |
    | kd | 1.05e-2 | 9.5e-3 |
    | sm2 | 3.8e-4 | 2.7e-4 |
    | w23 | 1.9e-4 | 4.2e-5 |
    | mclaughlin_limit | 1.1e-4 | 8.4e-5 |

    The other runs were not repeated. Tracking changes no step, and the returned bound is never below the final `y`'s, so their gaps can only be smaller than the table's. That is by construction, not measured. At ε = 1e-4, seven of ten bounds are therefore within 1e-4. The ε = 1e-6 column was not re-run either, and it can likewise only tighten.

  Plan §15.4's scale TARGET is 10⁷ nonzeros to a 1e-4 relative gap within 30 minutes, with a bound.
  - At ε = 1e-6 the certified bound meets it on all seven instances above 10⁷ nonzeros (1.7×10⁷ to 9.5×10⁷).
  - At ε = 1e-4 the solver's own stopping rule is met on all seven in under six minutes each. With the best-seen bound, the certified bound misses 1e-4 on one of them, sm2. With the final `y` alone it missed on three: sm2, w23 and mclaughlin_limit.
  - ε = 1e-6 was chosen after the 1e-4 run, on the same instances, so there is no held-out set.

  `docs/delivery/KILL-PUSH-PDLP.md` records the decision.
- MineLib's CPIT LPs are one family: 0–1 boxes, precedence and monotonicity rows, and one to three capacity rows per period. Blend networks, SAA extensive forms and CVaR LPs, the uses plan §15.4 names, are not tested at scale.

## 9. Oracles
- Vertex enumeration over every choice of tight constraints, for tiny bounded LPs. Brute force over all subsets, for closure LPs, whose relaxation is integral. Both are in `tests/common`, share no code with `pdlp`, and have the same author, so their agreement is same-origin evidence.
- External: MineLib's published "CPIT LP upper bound: objective value of the optimal LP relaxation". The source is Espinoza, Goycoolea, Moreno and Newman, MineLib, Annals of Operations Research 2013; the Results page is mirrored at github.com/borelian/minelib and was read 2026-11-01.
  - The LP is built in "by" variables (`w[b][t]` is the fraction of block `b` mined by period `t`), which keeps the optimum.
  - The published optima match a discount exponent counted from 0. On newman1 at ε = 1e-7 that gives 24,486,185.05 against the published 24,486,184. Counting from 1, as the format note reads, gives 22,672,394.94.
- External: netlib's published LP optima (`lp/data/readme`, "Optimal Value", MINOS 5.3, 11 significant figures) for AFIRO, ADLITTLE, E226 and ISRAEL, from the uncompressed MPS files in HiGHS's `check/instances` (MIT, commit `0edab95`), read 2026-11-03.
- HiGHS and OR-Tools PDLP (plan §15.4's lab oracles) are not run.

## 10. Known gaps
- `BD-251`: one library caller, `fmine-io`'s `omf::pit::PitModel::lp_schedule` (bd-djb7), which solves `toposort::lp`'s relaxation of an OMF schedule and reports minus the dual bound as its upper bound. The MineLib harnesses in `crates/xtask/tests/` also call `pdlp::solve`. The plan's blend, SAA and CVaR uses have no caller.
- No presolve, no feasibility polishing and no infeasibility or unboundedness detection. An infeasible or unbounded LP runs to `max_iterations`.
- The returned `x` is not feasible (§8). Nothing here turns it into a feasible point or a primal bound.
- Implied bounds come from one row at a time over the column bounds, a single-row propagation. A column bounded only by a combination of rows gets no implied bound. Only the shift then helps, and it does not help a free column (§8).
- At the default ε = 1e-4, the certified bound misses KILL-PUSH-PDLP's 1e-4 on three of ten MineLib instances, newman1, kd and sm2, even with the best-seen bound (§8). The default is cuPDLP's and is kept, and the MineLib test runs at 1e-6.
- At 1e-6 the three largest MineLib instances do not meet the stopping rule within 30 minutes. Their bounds were within 1e-4, but the solver cannot tell that without an outside optimum or a feasible point.
- Best-seen tracking keeps the multipliers with the highest dual objective and certifies only those, the final `y` and the two shifted. The dual objective leaves out wrong-signed reduced costs of open columns. Where the certified value departs from it, another evaluation may have certified more.
- No QP, MILP or decomposition (the crate's other planned slices).
- `mps` reads free format only: fixed-column MPS with spaces in names is not read. Integer markers are refused, not skipped. Only one `RHS`, `RANGES` and `BOUNDS` set is read. Nothing in the workspace writes MPS yet, and no outside solver has read `write`'s output.
- netlib's files are not in the repository, because no licence for the collection is recorded here (HiGHS carries these four under its MIT). `tests/netlib.rs` is ignored, run by hand with `FMINE_NETLIB_DIR`, and CI does not run it. Four of netlib's LPs were run, all small.
- MineLib's files are not in the repository, because their licence is not recorded. KILL-PUSH-PDLP is an ignored test, run by hand with `FMINE_MINELIB_DIR`, and CI does not run it.
- MineLib's mclaughlin (2.1 million blocks, 73 million arcs, 20 periods) is not run. It has about 2.9×10⁹ nonzeros in this formulation: about 140 GB at §8's rate for this crate alone, plus the test's own COO and CSR copies. It was not attempted on this shared 256 GB host.
- The timings come from one loaded machine.

## 11. Public API
- `pdlp::LpProblem`: `new`, `rows`, `cols`, `nnz`, `objective`, `max_violation`
- `pdlp::PdlpOptions` (`tolerance`, `max_iterations`, `threads`; `Default`)
- `pdlp::PdlpStatus` (`Optimal`, `IterationLimit`, `Stopped`)
- `pdlp::Progress` (public fields)
- `pdlp::PdlpSolution`: `status`, `x`, `y`, `progress`, `bound`, `bound_multipliers`
- `pdlp::solve`, `pdlp::solve_observed`
- `pdlp::DualBound`: `value`, `repaired_rows`, `implied_columns`
- `pdlp::certified_bound`
- `mps::read`, `mps::write`, `mps::MpsModel` (public fields)

## 12. Proof obligations
- PO-PUSH-4 ("Every dual bound is valid"; plan: "Dual feasibility verified with outward interval rounding, then weak duality") is discharged by `pdlp::bound`.
  - The weak-duality inequality and the rounding argument are in its module documentation and are not machine-checked.
  - `certified_bound_is_valid_at_any_multipliers`, `implied_column_bounds_hold_at_every_feasible_point`, `directed_operations_enclose_the_exact_result` and `directed_quotients_enclose_the_exact_result` check it on random and hand cases.
  - The implied bounds' validity (each holds at every feasible `x`) is argued in `pdlp::bound`'s module notes, and is not machine-checked either. The shift needs no argument, since its output is certified like any `y`.
- No other PO-* is discharged here. That PDHG with restarts converges is Applegate et al.'s and Lu and Yang's result, not proved or machine-checked here.
