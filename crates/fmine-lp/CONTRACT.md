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
- For `certified_bound(problem, y)` and `certified_infeasibility(problem, y)`, any row multipliers `y`, one finite value per row.
- `mps::read(text)`: free-format MPS text. Its sections are `NAME`, `OBJSENSE` (`MIN` or `MAX`), `ROWS` (`N`, `L`, `G`, `E`), `COLUMNS`, `RHS`, `RANGES` and `BOUNDS` (`UP`, `LO`, `FX`, `FR`, `MI`, `PL`), then `ENDATA`. The module notes give each rule:
  - the first `N` row is the objective; a right-hand side `b` on it is the objective constant `−b`;
  - a range `R` on a row with right-hand side `b`: `G` gives `[b, b + |R|]`, `L` gives `[b − |R|, b]`, and `E` gives `[b, b + R]` or `[b + R, b]` by the sign of `R`;
  - a column's default bounds are `[0, +∞)`.
- `mps::write(name, problem)`: any `LpProblem`, under a name with no space or tab.
- `mps::read_values(text, names, problem)` (bd-06kg): a solution of `problem` from an outside solver that read `write`'s file. It has one `name value` line for each column (`Names::Columns`, `C0`, `C1`, …) or each row (`Names::Rows`, `R0`, `R1`, …). Lines whose first field starts with `#` are comments, as in Gurobi's `.sol` files.

## 3. Outputs
- `PdlpSolution`:
  - `status`: `Optimal` (the stopping rule held), `IterationLimit`, `Stopped`, or `Infeasible` (a Farkas ray certified that no `x` meets the rows and column bounds; bd-gj80);
  - `x`, one value per column, always within the column bounds;
  - `y`, one multiplier per row;
  - `progress`, the measures at the returned point;
  - `bound`, the highest `certified_bound` of up to four multipliers, a tie going to the earlier:
    - `y`;
    - those with the highest dual objective seen at any evaluation (the current point and the average, every 64 steps). The dual objective is not monotone along a run, so these can be the tighter;
    - each of the two moved by the shift for open columns (§4, bd-tcp4). On a boxed problem nothing is moved;
  - `bound_multipliers`, the multipliers `bound` came from: `certified_bound(problem, bound_multipliers)` gives it again;
  - `infeasibility_ray`, with `Infeasible` only: the row multipliers that prove it, which `certified_infeasibility` certifies again. The `bound` of an infeasible problem holds but says nothing.
- `Progress`: iterations, restarts, primal and dual objectives, relative primal residual, relative dual residual and relative gap (§4), step size and primal weight.
- `DualBound`:
  - `value`: no feasible `x` has `c·x` below it;
  - `repaired_rows`: the count of multipliers set to zero because their sign needed an infinite row bound;
  - `implied_columns`: the count of columns whose infinite bound met a reduced cost of the wrong sign, and whose term used a bound the rows imply instead (bd-tcp4).

  `None` means an infinite column bound met a reduced cost of the wrong sign, and the rows imply no finite bound on that side either, so this `y` bounds nothing.
- `certified_infeasibility(problem, y)`: the `DualBound` that `certified_bound` would give `y` with every cost taken as zero, when its value is positive, and `None` otherwise. A positive bound on `0·x` over the points meeting the rows and column bounds means there are none.
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
- **Infeasibility is reported only when proved** (bd-gj80). `certified_infeasibility` is `certified_bound`'s sum with every cost taken as zero, the same dual repair, outward rounding and implied bounds. It bounds `0·x` at every point that meets the rows and column bounds, so a positive value leaves no such point. The implied bounds hold at every such point, so they may be used; if there is none, the argument needs nothing from them.
  - At each evaluation the engine computes, in plain arithmetic and scaled units, three candidates' value with the infinite sides left out: the last step's change in `y`, its change since the last restart, and `y` itself. It costs `O(rows + columns)` from products already held.
  - A candidate with a positive value is unscaled and certified. A certified one ends the run with `Infeasible` and is returned.
  - After a failed certification, the next attempt waits 1, then 2, 4, … up to 64 evaluations. The check runs after the stopping rule, so a run that meets the rule ends `Optimal` first.
  - Nothing else changes: the 35 feasible netlib LPs took the same steps to the same bounds, bit for bit, as before it.
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
- Infeasibility (bd-gj80):
  - `farkas_rays_prove_infeasibility_only_where_it_holds`: hand cases. One row proves it, two rows prove it only together, a free column certifies with a reduced cost of exactly zero, and an open column certifies through the top a row implies. A feasible variant certifies nothing at five multipliers, up to ±10³⁰⁰;
  - `farkas_rays_never_certify_a_feasible_lp`: 400 random LPs of integer data, half with open tops. Each has an integer point meeting every bound exactly in `f64`. 8,800 multipliers were tried and none certified: the solver's final and bound multipliers, and random ones at five scales. With the costs not set aside (planted, below) this test fails;
  - `infeasible_lps_end_with_a_certified_ray`: 300 random boxed LPs made infeasible by integer multipliers on two or more rows, with a margin of at least 1. Vertex enumeration finds no feasible vertex in any. Each run ends `Infeasible` with a ray that certifies again, in at most 128 steps;
  - `infeasible_lps_with_open_columns_end_with_a_certified_ray`: the same with some columns open above (215 in all), keeping the integer proof. 300 of 300 were proved;
  - `an_infeasible_lp_is_proved_infeasible` and `infeasible_problems_never_report_optimal`: hand LPs. Before bd-gj80 the latter ran to the iteration limit; now it ends `Infeasible`.
  - bd-tcp4's multiplier shift, tried on the rays with costs zero, changed nothing on the open-column oracle (300 of 300 either way) or on netlib, so it is not used.
- `any_thread_count_gives_the_same_bits`: a 72,000-column transport LP (144,000 nonzeros) gives the same iterations, `x` and `y`, bit for bit, at 1, 2 and 5 threads.
- `spmv_matches_the_numerics_csr_bit_for_bit_at_any_piece_count`: the solver's own `u32` CSR product equals `fmine_numerics::sparse::Csr::spmv`.
- Planted bugs, each caught and then reverted:
  - dropping the dual repair (caught by the unit test and the oracle);
  - taking the wrong row bound in the dual objective (caught by the tiny and closure oracles);
  - `||` for `&&` in the stopping rule (caught by five tests);
  - returning the final `y` as `bound_multipliers` when the best-seen bound wins (caught by the reproducibility check);
  - never updating the best-seen multipliers. The dense and closure LPs alone did not catch it, because their final multipliers were always the best; the CPIT-shaped ones do (case 11);
  - certifying infeasibility with the real costs instead of zero (bd-gj80). `farkas_rays_never_certify_a_feasible_lp` caught it on its first case. The hand unit test did not, since its costs are zero.
- **MPS** (`mps::tests`):
  - `every_section_and_bound_type_reads_as_written`: one file with every section, row type, range sign and bound type, tab-led lines, set names given and left out, and an objective constant reads to a hand-built `LpProblem`, bit for bit;
  - `a_written_problem_reads_back_bit_for_bit`: that problem, and one of tenths, ±10³⁰⁰, a subnormal, signed zeros and a free row, read back from `write` bit for bit. A row whose range reads back exactly as neither `G` nor `L` (`[1, 2⁵³ + 2]`) is refused;
  - `each_defect_is_refused_naming_its_line`: 34 one-defect files, each refused with its line and reason, and `LpProblem::new`'s own refusal passed through;
  - `a_small_lp_solved_from_its_mps_text_certifies_its_hand_optimum`: `max 3x + 2y` over two rows, read and solved at ε = 1e-9, certifies a bound at or below −11 and within 1e-6 of it.
  - Planted bug: the `E` row's range sign rule reversed. The every-section test caught it, and it was reverted.
  - `a_solution_file_reads_back_by_name_and_its_multipliers_certify_again` (bd-06kg): out-of-order lines, tabs, comments and blank lines read to the right indices. PDLP's bound multipliers, written in Rust's shortest form and read back, certify the same bound bit for bit. 18 one-defect files are refused: missing, repeated and unknown names, leading zeros, the other kind of name, `OBJ`, wrong field counts, and six non-finite or malformed numbers.
  - The outside half is in fmine-io (PO-IO-11). HiGHS's solution of an exported schedule LP, pinned as an oracle fixture, comes back through `read_values`, and its multipliers certify within one ulp of the hand optimum.
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
- From `mps::read_values`: a line that is not a name and a value, a name that is not one of the problem's columns or rows (`C7`, no leading zeros), a value that is not a finite number, a name given twice, and names given no value (the first is named). A missing value is refused, not taken as 0.
- From `LpProblem::new`:
  - a cost, bound or row length that does not match `a`;
  - a cost or matrix entry that is not finite;
  - a NaN bound, a lower bound of `+∞` or above its upper bound, or an upper bound of `−∞`;
  - `u32::MAX` or more rows or columns.
- From `solve` and `solve_observed`:
  - a tolerance outside `(0, 1)`, or `max_iterations` of 0;
  - a step size or iterate that becomes non-finite;
  - a worker thread that panicked.
- From `certified_bound` and `certified_infeasibility`: multipliers that are not one finite value per row.
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
- **netlib** (`tests/netlib.rs`, ignored, run by hand on 2026-11-03). 35 small netlib LPs, from 27 to 515 rows, 32 to 1,075 columns and 83 to 4,563 nonzeros, at ε = 1e-9, one million steps and one thread. Release and debug builds gave the same bounds and step counts. The release run took 141 s. The gap is `(published − bound) / |published|`, against netlib's MINOS 5.3 optima.
  - Every file read and wrote back to the same text, including the four with `RANGES` (BOEING1, BOEING2, FORPLAN, SEBA).
  - No bound lay above a published optimum. BEACONFD's and GROW7's lay 6e-12 above the 11-figure value, inside its rounding.
  - 24 certified within 2.5e-7. The largest gaps were SCAGR7 2.4e-7 and ETAMACRO 2.2e-7, and the test now holds those 24 to 1e-6.
  - FORPLAN stopped at the step limit with a bound 1.9e-2 below.
  - 10 met the stopping rule and certified nothing: BORE3D, BRANDY, CAPRI, FINNIS, LOTFI, SCORPION, SCSD1, SHARE1B, SHARE2B and STAIR.
    - Each has columns open above that no single row bounds. A re-run of the propagation outside the crate counted SCSD1's 760 of 760, SCORPION's 303 of 358 and FINNIS's 298 of 533.
    - The shift did not give all of them a reduced cost of the right sign.
    - Their dual objectives were within 2.6e-9 of the optimum, except FINNIS's, which was 5.9e-7 above it. That is the value an uncertified dual objective would have reported as a bound.

  The four run first:

  | LP | Rows | Columns | Nonzeros | Steps | Columns on an implied bound | Gap |
  |---|---|---|---|---|---|---|
  | AFIRO | 27 | 32 | 83 | 640 | 7 | 6.9e-11 |
  | ADLITTLE | 56 | 97 | 383 | 11,712 | 34 | 2.0e-10 |
  | E226 | 223 | 282 | 2,578 | 44,032 | 56 | 3.3e-8 |
  | ISRAEL | 174 | 142 | 2,269 | 19,008 | 64 | 1.2e-8 |

  - Before bd-tcp4, none of the four had a finite bound at its final `y`.
  - With implied bounds and no shift, ADLITTLE's and E226's final `y` still certified nothing. A column with no implied top had a wrong-signed reduced cost. Their bounds came from the best-seen multipliers, with gaps of 0.67 and 4.6.
  - E226's objective row has the right-hand side −7.113, so its objective has the constant 7.113. netlib's optimum leaves it out: the certified bound on `c·x` alone is within 3.3e-8 of it.
- **netlib's infeasible LPs** (`netlib_infeasible_lps_are_proved_infeasible`, ignored, run by hand on 2026-11-03 in release, with the same settings). These are the 8 of `lp/infeas` that HiGHS's `check/instances` carries. Each read and wrote back to the same text, and none was reported optimal.

  | LP | Rows | Columns | Nonzeros | Steps | Columns on an implied bound | End |
  |---|---|---|---|---|---|---|
  | BGETAM | 400 | 688 | 2,409 | 64 | 123 | proved |
  | GALENET | 8 | 8 | 16 | 64 | 0 | proved |
  | REFINERY | 323 | 464 | 1,626 | 3,712 | 100 | proved |
  | FOREST6 | 66 | 95 | 210 | 4,480 | 34 | proved |
  | BOX1 | 231 | 261 | 651 | 1,000,000 | | limit |
  | EX72A | 197 | 215 | 467 | 1,000,000 | | limit |
  | KLEIN1 | 54 | 54 | 696 | 1,000,000 | | limit |
  | WOODINFE | 35 | 89 | 140 | 1,000,000 | | limit |

  Every column of BOX1, EX72A and KLEIN1 is open above, as are 75 of WOODINFE's 89. A ray certifies only when every open column with no top the rows imply has `−aⱼ·y` of the right sign exactly. None of the candidates did, and neither did the shift.
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
- External: netlib's published LP optima (`lp/data/readme`, "Optimal Value", MINOS 5.3, 11 significant figures) for the 35 LPs `tests/netlib.rs` lists, from the uncompressed MPS files in HiGHS's `check/instances` (MIT, commit `0edab95`), read 2026-11-03. Also from there, 8 LPs that netlib lists as infeasible (`lp/infeas`), each matching the git blob hash HiGHS records. Their infeasibility is netlib's statement, and the four proved here confirm it for those four.
- HiGHS (bd-06kg), through scipy 1.18.1's `linprog(method="highs")`, run once in the lab by `scripts/oracle/highs_lp.py`. That script has its own MPS parser and shares no code with this crate, but has the same author. Its solution of one exported schedule LP is pinned under `fixtures/fmine-io/` with capture records. The script also matched netlib's published optima for AFIRO, SC50A and BLEND, read from netlib's own files. OR-Tools PDLP is not run.

## 10. Known gaps
- `BD-251`: one library caller, `fmine-io`'s `omf::pit::PitModel::lp_schedule` (bd-djb7), which solves `toposort::lp`'s relaxation of an OMF schedule and reports minus the dual bound as its upper bound. The MineLib harnesses in `crates/xtask/tests/` also call `pdlp::solve`. The plan's blend, SAA and CVaR uses have no caller.
- No presolve, no feasibility polishing and no unboundedness detection: an unbounded LP runs to `max_iterations`. A primal ray would need equality rows met exactly, which floating point does not give.
- Infeasibility is proved only where some candidate ray certifies. A column open on the side its term needs, with no implied bound, needs `−aⱼ·y` of the right sign exactly. 4 of netlib's 8 infeasible LPs ran to the step limit for that reason (§8). The random oracles' LPs are small and their infeasibility has a margin of at least 1, so they do not show how detection fares near the edge of feasibility.
- The returned `x` is not feasible (§8). Nothing here turns it into a feasible point or a primal bound.
- Implied bounds come from one row at a time over the column bounds, a single-row propagation. A column bounded only by a combination of rows gets no implied bound. Only the shift then helps, and it does not help a free column (§8). On netlib, 10 of 35 LPs certify nothing at multipliers whose dual objective is within 6e-7 of the optimum (§8).
- At the default ε = 1e-4, the certified bound misses KILL-PUSH-PDLP's 1e-4 on three of ten MineLib instances, newman1, kd and sm2, even with the best-seen bound (§8). The default is cuPDLP's and is kept, and the MineLib test runs at 1e-6.
- At 1e-6 the three largest MineLib instances do not meet the stopping rule within 30 minutes. Their bounds were within 1e-4, but the solver cannot tell that without an outside optimum or a feasible point.
- Best-seen tracking keeps the multipliers with the highest dual objective and certifies only those, the final `y` and the two shifted. The dual objective leaves out wrong-signed reduced costs of open columns. Where the certified value departs from it, another evaluation may have certified more.
- No QP, MILP or decomposition (the crate's other planned slices).
- `mps` reads free format only: fixed-column MPS with spaces in names is not read. Integer markers are refused, not skipped. Only one `RHS`, `RANGES` and `BOUNDS` set is read. fmine-io's `PitModel::schedule_lp` is the one caller that exports (bd-06kg). HiGHS has read `write`'s output for that one 111-row LP, through a parser of the same author, not HiGHS's own MPS reader. No commercial solver has. No CLI verb exports or imports.
- netlib's files are not in the repository, because no licence for the collection is recorded here (HiGHS carries these under its MIT). `tests/netlib.rs` is ignored, run by hand with `FMINE_NETLIB_DIR`, and CI does not run it. 35 of netlib's LPs were run, all small; the larger ones were not.
- MineLib's files are not in the repository, because their licence is not recorded. KILL-PUSH-PDLP is an ignored test, run by hand with `FMINE_MINELIB_DIR`, and CI does not run it.
- MineLib's mclaughlin (2.1 million blocks, 73 million arcs, 20 periods) is not run. It has about 2.9×10⁹ nonzeros in this formulation: about 140 GB at §8's rate for this crate alone, plus the test's own COO and CSR copies. It was not attempted on this shared 256 GB host.
- The timings come from one loaded machine.

## 11. Public API
- `pdlp::LpProblem`: `new`, `rows`, `cols`, `nnz`, `objective`, `max_violation`
- `pdlp::PdlpOptions` (`tolerance`, `max_iterations`, `threads`; `Default`)
- `pdlp::PdlpStatus` (`Optimal`, `IterationLimit`, `Stopped`, `Infeasible`)
- `pdlp::Progress` (public fields)
- `pdlp::PdlpSolution`: `status`, `x`, `y`, `progress`, `bound`, `bound_multipliers`, `infeasibility_ray`
- `pdlp::solve`, `pdlp::solve_observed`
- `pdlp::DualBound`: `value`, `repaired_rows`, `implied_columns`
- `pdlp::certified_bound`, `pdlp::certified_infeasibility`
- `mps::read`, `mps::write`, `mps::MpsModel` (public fields), `mps::read_values`, `mps::Names`

## 12. Proof obligations
- PO-PUSH-4 ("Every dual bound is valid"; plan: "Dual feasibility verified with outward interval rounding, then weak duality") is discharged by `pdlp::bound`.
  - The weak-duality inequality and the rounding argument are in its module documentation and are not machine-checked.
  - `certified_bound_is_valid_at_any_multipliers`, `implied_column_bounds_hold_at_every_feasible_point`, `directed_operations_enclose_the_exact_result` and `directed_quotients_enclose_the_exact_result` check it on random and hand cases.
  - The implied bounds' validity (each holds at every feasible `x`) is argued in `pdlp::bound`'s module notes, and is not machine-checked either. The shift needs no argument, since its output is certified like any `y`.
- An `Infeasible` status rests on the same argument with every cost zero (bd-gj80; `pdlp::bound`'s notes). `farkas_rays_never_certify_a_feasible_lp` checks it on random cases, and it is not machine-checked.
- No other PO-* is discharged here. That PDHG with restarts converges is Applegate et al.'s and Lu and Yang's result, not proved or machine-checked here.
