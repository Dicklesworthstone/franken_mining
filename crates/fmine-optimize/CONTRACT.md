# CONTRACT.md — `fmine-optimize`

## 1. Purpose
The Pushback optimisation crate. Five solver slices, four readers and an LP bridge so far; `toposort::pcpsp` is both a reader and a search:
- `pitflow` (`BD-250`, plan §15.4): the ultimate pit as a maximum-weight closure, by Hochbaum's pseudoflow (lowest-label variant, gap termination) on an `i64` CSR precedence graph. Each pit comes with its pseudoflow certificate, which `verify` checks without the solver.
- `pitflow::shells` (`bd-nzwd`, plan §15.4 and PO-PUSH-3): nested pit shells across an increasing list of exact revenue factors. These are the pit-by-pit sequence from which pushbacks are chosen.
  - Shell `k` is the union of the pit solved at factor `k` and shell `k − 1`.
  - With non-negative revenue that union is a best pit at factor `k`; the module documentation gives the proof.
  - Each shell's closure, and its value against the certified optimum, is checked before it is returned.
- `pitflow::slope` (`bd-t75z`, plan §15.4: "implicit precedence arcs from slope templates (azimuth sector × geotechnical domain from §14.2)"): precedence arcs for a regular block grid, from exact slope tangents per domain and azimuth sector.
  - Each block requires every block whose centre lies in its upward cone, up to K layers above.
  - Arcs implied by shorter ones are dropped, and every block's closure stays the same.
- `pitflow::minelib` (`bd-ip99`): MineLib's UPIT text pair (`.upit` values, `.prec` precedence) read into a `ClosureProblem`. No I/O.
  - Values are decimals scaled to `i64` at a declared number of places, at most `MAX_DECIMALS` (9). An exponent of one or two digits (`-5.65503E+6`, as MineLib's newman1.pcpsp writes) moves the point exactly before scaling.
  - A value with more non-zero places is refused unless the caller passes `Rounding::HalfAwayFromZero`; each rounded value is counted in `UpitModel::rounded`.
  - Every block must appear exactly once in each file. Headers, counts, ids and duplicates are checked; nothing is defaulted.
- `pitflow::candidate` (`bd-nq06`, plan §15.4 `UntrustedCandidate`): an external pit, given as block ids, checked on a `ClosureProblem`. No I/O.
  - `parse_ids` reads whitespace-separated ids with `%` comments.
  - `check` counts every unmet requirement (block in, required block out; per arc), keeps the first `WITNESSES` (10), and sums the exact value.
  - Nothing the candidate claims about itself is read. Its gap to the optimum comes from `solve`'s certified value, not from the candidate.
- `toposort` (`bd-kcpj`, plan §15.4 row `fmine-optimize::toposort` and PO-PUSH-5): multi-period schedules on a precedence graph, with per-period upper limits on resources. A use may be negative (`bd-ixfw`), as in a blending row, where a block below the target grade makes room for one above it.
  - `check` is PO-PUSH-5's feasibility check. Every mined block's required blocks must be mined in the same period or earlier, and no resource may go over its limit in any period. It returns each period's exact value sum, and every broken requirement and limit.
  - `greedy` is TopoSort (Chicoisne, Espinoza, Goycoolea, Moreno and Rubio, Operations Research 2012). It takes Kahn's topological order with the caller's rank breaking ties, then puts each block in the earliest period with room. It runs `check` on its schedule and refuses one that fails.
  - `greedy_from` (`bd-wife`) is `greedy` with a release period per block: no block is mined before its release. As with `pcpsp::greedy_from`, a block below a blend target released at its LP median period is not used up in an early period. On MineLib's sm2 it raised the searched schedule by 1.06% (§9).
- `toposort::search` (`bd-5lxu`, plan §15.4's "plus local search and repair"): `improve` takes a feasible schedule and makes moves that keep it feasible and strictly raise its discounted value. The discount factors are the caller's integers; `discount_factors` gives `(1 + rate)^−t` times 2⁴⁰, rounded down period by period.
  - The moves, in each round's order: a positive block to an earlier period; two blocks traded between adjacent periods, the higher-valued going earlier; a positive block added under mined blocks; a negative block to a later period; and a negative block that no mined block requires dropped.
  - A round in which none of those moves then tries a cone (`bd-ynoj`). A positive block that a required block in its own period keeps there goes one period earlier, together with every block it requires, directly or not, in that period. The cone holds at most `CONE_LIMIT` (64) blocks, must be worth more than 0, and must fit. `improve_without_cones` leaves the cone out. `pcpsp::improve` uses it until destination moves stop too, and only then `improve`, because a cone can take room that a destination switch would use better.
  - With negative uses, taking a block out of a period can put that period over a limit. The moves that take a block out (earlier, later, drop) check the period left as well as the one entered; a trade checks both periods.
  - It ends with `check` and refuses a result that fails it or is worth less than its input. It is a heuristic, with no bound on its distance from the optimum.
- `toposort::cpit` (`bd-kcpj`): MineLib's CPIT text pair (`.cpit` periods, limits, values, uses and discount rate; `.prec` as for UPIT) read into a `ScheduleProblem`. No I/O.
  - Values are scaled as UPIT values are. Uses are rounded up (`Rounding::Ceiling`) and limits down (`Rounding::Floor`) when places are dropped, and both are counted.
  - Only `L` (upper) limits are read; `G` and `I` are refused. At most `MAX_RESOURCES` (64) resources.
  - Solutions (`bd-3l5o`, plan §15.4 `UntrustedCandidate`): `parse_solution` reads a schedule from anywhere in MineLib's solution format, one `<block> <destination> <period> <fraction>` line per mined block. The destination must be 0 and the fraction exactly 1, since CPIT has one destination and mines whole blocks. It trusts nothing in the file: `check` decides feasibility and the exact value, and `search::improve` can start from it, so the outside schedule can become the incumbent. `write_solution` writes the same format, which `parse_solution` reads back.
- `toposort::pcpsp` (`bd-2rne`): MineLib's PCPSP text pair (destinations, with one value per block and destination and uses per block, destination and resource) read into a `PcpspModel`. No I/O.
  - Scaling and rounding are as for CPIT. Uses round toward +∞ whatever their sign, so the rounded blend is never looser than the file's. Negative uses are read (`bd-ixfw`; MineLib's w23 has them in four blending rows). General side constraints and `G` and `I` limits are refused, since `ScheduleProblem` holds neither.
  - Each mined block goes whole to one destination; PCPSP's split blocks are not represented. `with_destinations` turns one destination per block into a `ScheduleProblem`, so `check`, `greedy` and `search::improve` apply unchanged.
  - `pcpsp::greedy` is TopoSort with a fallback: a ranked block that fits at its destination in no period from its required blocks' on goes to another destination, the most valuable first, in the earliest period where one fits. Plain `greedy` would leave it, and every block under it, unmined. On w23 that stopped all mining after period 2 (§9).
  - `pcpsp::greedy_from` (`bd-tll7`) is `pcpsp::greedy` with a release period per block: no block is mined before its release. Released at its LP median period, a block below a blend target is not used up in an early period, and the blocks above the target in its own period still have it to blend with.
  - `destinations` moves blocks between destinations within their period: a switch to a more valuable destination that fits, or an exchange in which one block makes room for another that gains more. It also gives each unmined block its most valuable destination, which changes no value. `improve` alternates `search::improve` and `destinations` until neither moves. Every move strictly raises the discounted value, and both end with `check`.
  - **Price** (`bd-rbxh`, `pcpsp::price`) is `destinations`' third move. Once switches and exchanges stop in a period, it reassigns all of that period's mined blocks together:
    - Row prices come from the period's destination LP, solved by Dantzig–Wolfe. The master starts from the current assignment and is solved by a dense revised simplex.
    - The candidates are the assignment at the last prices, the master's mixture rounded block by block, and the mixture's columns. Each is repaired to every limit in `i128`.
    - The best is taken only when it is worth strictly more.
    - On w23, where a blend allows no single block or pair into the mill, it raised the searched NPV by 20.5% (§9).
- `toposort::lp` and `toposort::pcpsp::lp` (`bd-ubzs`, plan §15.4: "feasible incumbents from LP solutions"): schedules from an LP relaxation, the pipeline behind §9's MineLib tables as library code. The crate formulates the LP and reads its solution; the caller solves it, since this crate does not depend on `fmine-lp` and the topology registry has no same-layer edge for it.
  - `relaxation` gives the LP as plain parts, `Relaxation { cost, matrix, row_lo, row_hi, col_lo, col_hi }`: what `fmine_lp::pdlp::LpProblem::new` takes. CPIT's is in "by" shares `w[b][t]`. PCPSP's adds `y[b][d][t]`, the share sent to each destination in each period, over `pcpsp::lp::scope`'s blocks: the ultimate pit under each block's most valuable destination, certificate checked, or every block when a use is negative.
  - `Shares` and `pcpsp::lp::Solution` read a solution back. A value that is not a number in `[0, 1]` is refused, not clamped. PCPSP blocks go whole to the destination the LP sends most of them to.
  - `starts` gives TopoSort's six schedules: three thresholds on the LP's final share, each with `greedy` and with `greedy_from` at the LP median periods. `incumbent` searches each, with `search::improve` or `pcpsp::improve`, and picks the first with the highest discounted value.
  - Moved from the two xtask harnesses without changing a bit (§9).

`fmine plan pit` (`fmine-cli`, `bd-ip99`, `bd-nq06`) calls `minelib::parse`, `solve`, `verify` and, with `--candidate`, `candidate::parse_ids` and `candidate::check`. `fmine-io`'s `omf::pit::PitModel` (bd-joiv) calls `slope::slope_arcs`, `solve` and `shells::nested_shells` on OMF block models. It also calls `toposort::greedy`, `toposort::search::discount_factors` and `toposort::search::improve` (bd-fr8x) to schedule OMF block models under capacities, and `toposort::lp`'s `relaxation`, `Shares`, `starts` and `incumbent` (bd-djb7), solving the relaxation with fmine-lp's PDLP in between. `toposort::pcpsp` has no caller outside tests. The MineLib harnesses in `crates/xtask/tests/` call `toposort::lp` and `toposort::pcpsp::lp`, and `cpit_schedule.rs` reads MineLib's schedules through `cpit::parse_solution` and writes its own with `write_solution` (bd-3l5o). Bienstock–Zuckerberg, Lagrangian and the stochastic layer are later slices.

- **Crate:** `fmine-optimize`
- **Layer:** E
- **Subsystem:** Pushback
- **LOC budget:** 36,000 planned / 41,000 ceiling
- **Allowed Module Families:** `["pitflow", "toposort"]`

## 2. Inputs
- One `i64` value per block, in a caller-chosen integer money unit; the caller scales and rounds, never this crate. The one exception is `pitflow::minelib`, which scales a UPIT file's decimals at the places the caller declares and rounds only when the caller asks.
- For `pitflow::minelib`: the two files' text, the number of decimal places and a `Rounding`.
- Precedence arcs `u → v`, read "taking `u` requires taking `v`", either as CSR (`ClosureProblem::new(weights, offsets, targets)`) or as one list per block (`from_requirements`). Duplicate arcs and self-loops are accepted. Cycles are accepted too: a cycle's blocks are taken together or not at all.
- For shells (`ShellModel`): one non-negative `i64` revenue and one `i64` cost per block, the same arcs, and strictly increasing `RevenueFactor`s `num / den` with both parts positive. At a factor a block is worth `num · revenue − den · cost`. That is its value scaled by `den`, so no fraction or float is formed.
- For slope arcs, all integers:
  - a `BlockGrid`: block counts east, north and down, and positive block sizes in one length unit;
  - a `SlopeTable`: per domain, sectors listed counterclockwise, each starting at a boundary direction `(east, north)` and holding a tangent `rise / run`;
  - one `u16` domain per block, and K, the number of layers a cone spans.

  Block `(i, j, k)` is `(k·ny + j)·nx + i`, with `k = 0` at the top. The slope that applies is the lower block's, for the direction from it to the upper block. No angle in degrees or float crosses the API: the caller turns an angle into a tangent.
- For `toposort` (`ScheduleProblem::new`): a `ClosureProblem` (values and requirements, as above), a period count, and per resource one `i64` use per block, of either sign, and one non-negative `i64` upper limit per period.
  - A schedule is one `Option<u32>` per block: its period, or `None` for unmined.
  - `greedy` takes one `Option<i64>` rank per block, lower first, or `None` to leave the block out. `greedy_from` also takes one release period per block, each a period of the problem.
- For `toposort::cpit`: the two files' text, the value places and `Rounding`, and the use places. For `parse_solution`: a solution's text and the `ScheduleProblem` it is for.
- For `toposort::search::improve`: a `ScheduleProblem`, a schedule that passes `check`, one non-negative `i64` discount factor per period that never rises, and a round limit. `discount_factors` takes a non-negative rate over a positive scale (`CpitModel::discount_rate` over `cpit::RATE_SCALE` fits) and a period count.
- For `toposort::pcpsp`: `parse` as `cpit::parse`; `with_destinations` one destination per block; `greedy` one rank and one first destination per block, and `greedy_from` also one release period per block, each a period of the model; `destinations` and `improve` a schedule and destinations that pass `check` together, one discount factor per period (non-negative and never rising for `improve`), and a cycle limit for `improve`.

## 3. Outputs
- `PitSolution`: `in_pit` (one flag per block), `value` (the pit's total, the maximum over all closed sets), `pit_size`, `arc_flow` (the certificate, one flow per arc in CSR order) and `stats` (`PitStats`: mergers, splits, relabels, largest label).
- `verify(problem, in_pit, arc_flow)`: the pit's value if the certificate holds.
- `NestedShells` provides:
  - `first_shell`, one `u32` per block: the first shell holding it, or `u32::MAX` if none does;
  - `contains(k, b)`;
  - one `ShellSummary` per factor, holding the factor, the shell's value in units of `1 / den`, its block count and the solved pit's block count.
- `SlopeArcs` provides:
  - CSR `offsets` and `targets`, ready for `ClosureProblem::new`;
  - `arc_count`, and `full_cone_arcs`, the count with every cone block required directly;
  - `boundary_blocks`, the blocks near a side of the grid or a domain contact whose arcs were reduced one by one.
- `ScheduleCheck`: `mined`, `period_values` (exact and undiscounted, `i128`), `used[r][t]`, `unmet` with the first `WITNESSES` (10) `(block, required)` pairs, `over_limit` (every `(resource, period)` over its limit), `feasible()` and `value()`.
- `greedy`: one `Option<u32>` period per block, a schedule that has passed `check`. `greedy_from`: the same, with every mined block at or after its release. `pcpsp::greedy`: such a schedule and one destination per block, which have passed `check` together; a destination differs from the one given only for a block that fell back. `pcpsp::greedy_from`: the same, with every mined block at or after its release.
- `search::Improvement`: the improved schedule (it has passed `check`), its discounted value `after` and the input's `before` (exact `i128`, `after ≥ before`), the rounds run, and `Moves` (counts of earlier, cones, swaps, added, later and dropped). `discounted_value` gives `Σ value · factor[t]` exactly.
- `pcpsp::PcpspModel`: `name`, `discount_rate`, the rounding counts, and `blocks`, `periods`, `destinations`, `resources`, `value(b, d)`, `uses(r, b, d)`, `limits(r)`, `requires(b)`. `pcpsp::DestinationMoves` counts switches, exchanges, periods reassigned by Price (`priced`) and unmined blocks given a destination. `pcpsp::PcpspImprovement` holds the improved schedule and destinations (they pass `check` together), `before` and `after` (exact `i128`, `after ≥ before`), the cycles run, and both kinds of move counts.
- `CpitModel`: `name`, `problem`, `discount_rate` (times `RATE_SCALE`, 10⁹), `rounded_values`, `rounded_uses` and `rounded_limits`. Discounting is the caller's.
- `lp::Relaxation`: `cost`, `matrix` (`fmine_numerics::sparse::Csr`), `row_lo`, `row_hi`, `col_lo` and `col_hi`, a minimisation in `f64`. `lp::Shares`: `blocks`, `periods`, `share(b, t)`, `expected(b)` and `medians()`. `pcpsp::lp::Scope`: the LP's `blocks`, `pit_blocks`, `pit_value` (certificate checked) and `whole`. `pcpsp::lp::Solution`: `shares()` and `destinations()`.
- `lp::Start` and `pcpsp::lp::Start`: the `threshold`, whether `released`, and the schedule (with its `destination` for PCPSP), which has passed `check`. `lp::Incumbent` and `pcpsp::lp::Incumbent`: the `searched` improvements, one per start, and the index of the `best`.

None is a Claim.

## 4. Invariants
- The returned pit is closed and its value is the maximum over all closed sets. `solve` runs `verify` before returning, so an uncertified pit is refused, never returned.
- `verify` accepts exactly when the pit is closed, all flows are non-negative, no arc entering the pit carries flow, every block in the pit has non-negative excess and every block outside non-positive excess. These make the pit the source side of a minimum s–t cut. It shares no code with the solver.
- The solver's forest invariants (labels monotone down each tree, `label(x) ≤ label(y) + 1` on residual arcs, flow only on tree arcs, excess only at roots, weak roots at label 0, every strong root in its bucket) are checked after every step on 3,000 random small instances (`random_small_instances_match_brute_force_and_keep_invariants`), and each value equals a brute force over all subsets.
- Values equal an independent Dinic maximum flow (`tests/common`, no shared code) on 300 random graphs (`random_graphs_match_dinic`) and on 15 open-pit-shaped block models up to 30 × 30 × 8 with 3 × 3 slope cones (`block_models_match_dinic`). `block_model_pit_is_a_union_of_cones` checks closure on a block model.
- The same input gives the same pit, flows and counters (`rerun_is_identical`).
- `verify` refuses planted bad certificates: an unclosed pit, flow into the pit, a wrong-signed excess, a negative flow (`verify_refuses_planted_certificates`).
- Shells nest, and each is a closed pit worth the best value at its factor (PO-PUSH-3). The nesting holds by construction; `nested_shells` checks closure and value for every shell.
  - `tests/shells_oracles.rs` checks both against Dinic, which computes the scaled values itself: 150 random graphs, with costs of either sign, and two block models, each over 8 factors from 1/4 to 3.
  - `two_ore_blocks_enter_at_their_factors` is a hand-derived known answer.
  - `negative_revenue_past_the_constructor_is_caught_by_the_value_check` shows the value check refusing a shell that the union rule cannot make optimal.
- Slope arcs give every block exactly the closure that its full K-level cones give. `tests/slope_oracles.rs` checks this against a brute-force oracle with its own angle order and cone test, which tests every block of every layer up to K above:
  - on 80 random grids, block sizes, slopes, sectors and domain maps (one to three domains, split, layered or noise), every block's closure is equal, and every arc kept is a cone arc;
  - the same on 60 layered cases, a near-vertical domain above or below a random steep one. A hand case is added where (0, −2, 4) is implied only through a block two layers up, in the other domain. The 80 random cases missed a planted bug in the interior check's vertical test, even at 4,000 rounds; these catch it;
  - with one slope everywhere, on 40 random grids, no closure leaves its block's true cone at any height;
  - on two block models with two domains, the pit solved on slope arcs equals Dinic on the full cones and is closed under them.
- Known answers (unit tests):
  - 45° on cubic blocks with K = 1 gives the block above and its four edge neighbours, five arcs;
  - 60° on cubic blocks with K = 3 gives nine;
  - flat 20 × 20 × 10 blocks at 45° give only the block above;
  - a direction on a sector boundary takes that sector's slope;
  - levels past the bottom of the grid change nothing.
- At scale (`tests/scale_pit.rs`, ignored, run by hand), three synthetic block models of about 10⁶ arcs, 10⁷ arcs and 10⁷ blocks each yield a certified, non-empty pit. The smallest also equals Dinic, and the largest must solve within KILL-PUSH-PIT's 600 s. `model_has_the_planned_arc_count_and_shape` checks the generator. `slope_arc_pits_at_scale` solves the same ore model on slope arcs, at 2·10⁶ and 10⁷ blocks (§8).
- GATE-PUSH-PIT1 (`tests/minelib_upit.rs`, ignored by default, run by hand): MineLib UPIT instances integer-scaled at 10⁶ are solved with the certificate verified, equal Dinic exactly, and fall within one whole unit of MineLib's published optimum, a band widened by the bound for any values rounded at read time.
- Schedules (`toposort`):
  - `check` agrees with feasibility written out from the definition on every schedule of 60 random problems and of the same 60 with one more resource whose uses run from −3 to 3, and its value equals the direct sum (`check_agrees_with_the_definition_on_every_schedule`). The problems have up to 6 blocks, 3 periods and 3 resources, and every schedule means each block unmined or in any period.
  - `greedy`'s schedule is feasible under any ranking on 400 random problems (`greedy_is_feasible_under_any_ranking_and_mines_whatever_fits`). There, a ranked block left unmined although its required blocks were mined fits no period from theirs on. With signed uses that last property does not hold, since a later block may make room; feasibility is checked on 400 such problems (`greedy_is_feasible_with_signed_uses`), and a known case shows the order mattering (`a_negative_use_makes_room_only_for_blocks_taken_after_it`).
  - With room for everything and the ultimate pit ranked, `greedy` mines exactly the pit in period 0, at `solve`'s value (`with_room_to_spare_the_ultimate_pit_is_mined_in_period_zero`, 100 random problems).
  - `greedy_from` under random releases on the same 400 random problems and the 400 with a signed resource (`released_greedy_is_feasible_and_mines_nothing_before_its_release`): feasible by the definition, no block mined before its release, and with every release 0 the same schedule as `greedy`. The releases change the schedule on more than 100 of the 800.
  - On a known blend, `greedy` mines the two blocks below the target in period 0, and the two above it then fit nowhere. Released one of each to period 1, every block is mined, at period values [5, 5] and blend use [0, 0] (`a_release_keeps_a_blend_partner_for_a_later_period`, in `toposort`'s tests; `pcpsp` has its own of that name).
  - Known answers: `greedy_follows_rank_within_precedence_and_capacity`, `a_precedence_cycle_stays_unmined`, `check_counts_late_and_missing_requirements_and_overfull_periods`, and `cpit::tests::tiny_pair_reads_exactly_and_schedules`.
  - `search::improve`, on greedy's schedules for 400 random problems under random rankings (`improve_is_feasible_monotone_and_bounded_by_the_exhaustive_optimum`):
    - the result is feasible by the definition, worth at least its input, and never more than the best of every schedule;
    - a second call makes no move.
    - On the 2026-11-02 run it raised 275 of the 400 and brought those at the exhaustive optimum from 97 to 334.
    - The same on 400 problems with a signed resource (`improve_keeps_every_limit_with_signed_uses`): it raised 241 and brought those at the optimum from 130 to 328.
  - From every feasible schedule of 60 random problems and of the same 60 with a signed resource, not only from greedy's: 3,722 starts (`improve_from_every_feasible_schedule_is_monotone_and_bounded`). Each result is feasible by the definition, worth at least its start and never more than the best schedule, and a second call makes no move. 86 cones were moved; from greedy's schedules on the 800 problems above, none was.
  - Each move on a known case, `Earlier` before `Later` included: `each_move_on_a_known_case`. That includes a cone: ore and its waste in period 1, which only the cone moves, and the same case with the waste outweighing the ore or no room for both, where nothing moves. With a blend, a block below the target stays where its period needs it under earlier, later and drop: `a_block_below_the_blend_stays_where_its_period_needs_it`. Discount factors to the unit: `discount_factors_fall_by_the_rate_rounding_down`.
  - Planted negatives, each run once and reverted: a capacity test that always passes, and a drop of positive blocks. `improve`'s final check or value test refused both.
  - Planted on 2026-11-02, each run once and reverted. A `check` that accepts a required block mined one period late failed 2 tests. A `greedy` that ignores limits was refused by its own `check` in 3 tests. For `bd-wife`, a `greedy_from` that ignores the release failed both release tests. For `bd-ynoj`, a cone without the block's own required blocks was refused by `improve`'s final check in 6 tests: 4 in `search`, 2 in `pcpsp`.
  - `pcpsp::improve`, from greedy's schedules under random destinations and rankings on 300 random problems with 2 to 5 blocks, 2 or 3 destinations and up to 2 resources (`pcpsp::tests::improve_is_feasible_monotone_and_bounded_by_the_exhaustive_optimum`):
    - the result is feasible and valued exactly by a check written from the model, not from `with_destinations`;
    - it is worth at least its input and never more than the best over every period and destination;
    - a second call makes no move.
    - On the 2026-11-02 run it raised 253 of the 300 and brought those at the exhaustive optimum from 32 to 237.
    - The same with a signed resource (`improve_keeps_every_limit_with_signed_uses`): 222 of 300 raised, at the optimum from 48 to 207 (2026-11-03, with Price; 206 with Price disabled).
  - `pcpsp::greedy` on the 300 random models, with and without the signed resource (`the_fallback_greedy_is_feasible_and_falls_back_only_when_stuck`): feasible by the model's own check; a destination changes only for a block it mined; without negative uses, a ranked block it leaves unmined fits at no destination in any period from its required blocks' on.
  - A blending row read with a negative use rounded toward +∞, and both greedies and the search on it: `pcpsp::tests::blending_rows_read_signed_uses_and_schedule_within_the_blend`.
  - `pcpsp::greedy_from` on the same 300 models, signed and not, under random releases (`released_greedy_is_feasible_and_mines_nothing_before_its_release`). It is feasible by the definition, mines no block before its release, and changes a destination only for a block it mined. With every release 0 it equals `pcpsp::greedy`. On a known blend, a release keeps a block below the target for the period whose block above the target needs it, and every block is milled where earliest-first sends two to the dump (`a_release_keeps_a_blend_partner_for_a_later_period`).
  - Each destination move on a known case, no move where the discount factor is 0: `pcpsp::tests::switches_and_exchanges_on_known_cases`.
  - Price (`bd-rbxh`):
    - `pcpsp::tests::destinations_with_price_are_feasible_monotone_and_bounded_per_period`: `destinations` on greedy schedules of the 300 random models, signed and not. It is feasible by the definition and never worth less. In each period it is worth at most the best whole-block destinations for the blocks mined there, found by enumeration. A second call moves nothing. On the 2026-11-03 run, 505 of the 900 periods started at that best and all 900 ended there, with 11 Price moves; with Price disabled, 889 ended there.
    - Known cases in `pcpsp::price::tests`:
      - `three_blocks_go_to_the_mill_together`: two blocks above a blend target and one below it go to the mill together, a gain no switch or exchange makes;
      - `each_block_goes_where_value_less_priced_use_is_largest`: the assignment at given prices, ties to the current destination;
      - `repair_lowers_the_excess_at_the_least_loss`;
      - `the_inverse_and_the_master_on_a_small_case`: a 2 × 2 inverse, a singular basis, and the master's prices and mixture on a two-column case.
    - Planted negatives on 2026-11-03, each run once and reverted, with a grep showing none left:
      - Price disabled: the per-period test failed.
      - No repair and no limit test at acceptance: `destinations` refused its own result as a defect (infeasible); 5 tests failed.
      - The price's sign flipped in the assignment, `value + λ · use`: at first every test passed, since the repair still reached the known cases' answers and only 898 of the 900 periods ended at their best. `each_block_goes_where_value_less_priced_use_is_largest` was then added, and it fails.
  - Planted negatives on 2026-11-02, each run once and reverted. A destination limit test that always passes was refused by `destinations`' final check (infeasible), and a switch to a less valuable destination by its value test. Each failed 2 of the 5 `pcpsp` tests. A `parse` that checks the arcs with zero values again, as before the fuzz finding in §8, read the case that must be refused and failed `every_malformed_pair_refuses`. A `check` that ignores every limit but the first (in the tiny pair, the mill's) failed `pcpsp::tests::bad_inputs_refuse` and `check_agrees_with_the_definition_on_every_schedule`. For `bd-ixfw`: a search that takes a block out of a period as if no use were negative (the check before signed uses) failed the known blend case and both signed `improve` tests; a `pcpsp::greedy` whose fallback ignores limits was refused by its own `check` in the fallback test. For `bd-tll7`: a `greedy_from` that ignores the release failed both release tests.
  - Schedules from an LP (`bd-ubzs`):
    - `lp::tests`:
      - `the_relaxation_of_a_small_case_has_each_row_and_cost` checks every row, bound and cost of a three-block, two-period case by hand, at whole units and at tenths.
      - `power_matches_powi_bit_for_bit` checks the discount power against `powi` bitwise, for six rates and 200 periods.
      - `shares_refuse_what_is_not_a_share_in_range`, `the_relaxation_refuses_a_bad_rate_or_places`, `ranks_and_medians_follow_the_lp`, `starts_and_the_incumbent_on_a_small_case` and `the_incumbent_is_the_first_of_the_best`. The last two cover the six starts' order, a tie on rank broken by id, and a swap that the search finds.
    - `pcpsp::lp::tests`:
      - `the_scope_is_the_pit_unless_a_use_is_negative` and `the_relaxation_over_the_pit_has_each_row_and_cost`.
      - `the_relaxation_refuses_blocks_that_are_not_closed_or_in_order` and `the_solution_refuses_what_is_not_a_share_in_range`.
      - `the_solution_sends_each_block_where_the_lp_sends_most_of_it` covers a block outside the LP, a tie between destinations, and a majority.
      - `starts_and_the_incumbent_on_the_small_pair`: **Later** moves a waste block beside the ore it lies under.
    - Planted negatives on 2026-11-03, each run once and reverted, with a grep showing none left:
      - A median counted at a share above one half, not at least one half: 4 tests failed.
      - Destination ties sent to the highest destination: the tie case failed.
      - A scope that ignores negative uses: the scope test failed.
  - On MineLib's CPIT instances, see §9.

## 5. Refusals
Every public function returns `Result<_, Refusal>`; none panics on bad input.
- `FMINE-REF-207` (proposed), from `ClosureProblem::new` and `from_requirements`:
  - `u32::MAX` or more blocks or arcs;
  - offsets that are not `n + 1` non-decreasing entries from 0 to the arc count;
  - a target that is not a block;
  - a requirement list count that differs from the value count;
  - absolute values summing to 2⁶² or more (`ABS_VALUE_SUM_LIMIT`, plan §15.4's overflow guard).

  Tests: `malformed_input_and_overflow_are_refused`, and `values_near_the_overflow_guard_solve_exactly` for the edge just inside it.
- `FMINE-REF-207` (proposed), from `verify` and therefore `solve`: lengths that do not match the problem, a negative flow, an unclosed pit, flow entering the pit, a wrong-signed excess, or a value overflowing `i64`.
- `FMINE-REF-207` (proposed), from `solve`: an intermediate flow overflowing `i64` or a label overflowing `u32`. The overflow guard makes both unreachable in practice.
- `FMINE-REF-207` (proposed), from `shells`:
  - a factor whose numerator or denominator is not positive;
  - revenue and cost lists of different lengths;
  - a negative revenue;
  - factors that do not strictly increase, or `u32::MAX` or more of them;
  - a block value overflowing `i64`, or values breaking the 2⁶² guard, at any factor;
  - a shell that is not closed or is worth less than the best pit at its factor, which the construction rules out.

  Tests: `bad_input_is_refused`, and the negative-revenue test above.
- `FMINE-REF-207` (proposed), from `slope`:
  - a zero block count or size, or `u32::MAX` or more blocks;
  - a tangent with a zero rise or run;
  - a sector boundary of `(0, 0)`;
  - no domains, more than 65,536, or a domain with no sectors;
  - boundaries that do not go strictly counterclockwise, within one turn, from a domain's first;
  - K of 0, a domain list of the wrong length, or a block in a domain the table lacks;
  - reach boxes over `MAX_SCAN` (2²⁶) candidate offsets in all, or a cone over `MAX_PATTERN` (2¹⁶) offsets;
  - a cone test that would overflow `u128`;
  - `u32::MAX` or more arcs.

  Tests: `slope::tests::bad_input_is_refused`, one row each.
- `FMINE-REF-506` (proposed), from `minelib::parse` and `minelib::scaled`:
  - a missing, repeated or unknown header, or a `TYPE` other than `UPIT`;
  - a value that is not a decimal (an exponent of more than two digits included), does not fit `i64` at the places declared, or has non-zero places past them under `Rounding::Refuse`; more than 9 declared places;
  - an objective line count that differs from `NBLOCKS`, or a block id out of range or given twice;
  - a precedence line whose count differs from its list, that requires itself, an out-of-range block or one block twice, or a block with no precedence line or two.

  Tests: `minelib::tests::every_malformed_pair_refuses`, one row each, and `crates/xtask/tests/plan_pit.rs` through the CLI.
- `FMINE-REF-506` (proposed), from `candidate::parse_ids`: a token that is not a non-negative integer, an id that is not a block, or an id given twice. Test: `candidate::tests::ids_parse_to_membership_and_bad_ids_refuse`.
- `FMINE-REF-207` (proposed), from `candidate::check`: a membership list whose length is not the block count. Test: `candidate::tests::closure_witnesses_and_value_are_exact`.
- `FMINE-REF-207` (proposed), from `toposort`:
  - `ScheduleProblem::new`: no periods or more than `MAX_PERIODS` (65,536); resources without one use per block and one limit per period; a negative limit;
  - `check`: a schedule whose length is not the block count, or a period past the last;
  - `greedy`: ranks whose length is not the block count, or a schedule that fails `check`, which would be a defect. `greedy_from` also refuses release periods whose length is not the block count, or one past the last period.
  - `search::discount_factors`: a negative rate, a scale that is not positive, or periods outside `1..=MAX_PERIODS`;
  - `search::discounted_value`: a schedule or factor list of the wrong length, or a period past the last;
  - `search::improve` and `search::improve_without_cones`: an input schedule that fails `check`, factors of the wrong length, negative or rising; and, as defects, a result that fails `check` or is worth less than the input.

  Tests: `malformed_problems_refuse`, `check_counts_late_and_missing_requirements_and_overfull_periods`, `greedy_follows_rank_within_precedence_and_capacity`, `a_release_keeps_a_blend_partner_for_a_later_period`.
- `FMINE-REF-506` (proposed), from `toposort::cpit::parse`:
  - a missing, repeated or unknown header or section, a `TYPE` other than `CPIT`, `NPERIODS` of 0 or above `MAX_PERIODS`, a negative `DISCOUNT_RATE`, more than `MAX_RESOURCES` resources;
  - a limit line count other than resources × periods, a limit kind other than `L`, a resource or period out of range or given twice;
  - an objective line count other than `NBLOCKS`, a block out of range or given twice;
  - a use for a block or resource out of range or given twice, or a negative use or limit;
  - a line that fits no section, or data before the first section;
  - the `.prec` and decimal refusals of `minelib`.

  Test: `cpit::tests::every_malformed_pair_refuses`, one row each.
- `FMINE-REF-506` (proposed), from `toposort::cpit::parse_solution`, naming the line: a line that is not four fields, a block or period that is not a non-negative integer below the problem's count, a destination other than 0, a fraction other than exactly 1 (`0.999999`, `1.0000001`, `1e0` and `+1` included), or a block listed twice. Test: `cpit::tests::a_solution_reads_back_and_an_outside_one_is_checked_before_any_search`, 13 cases. In the same test an infeasible candidate is reported by `check` (over its limit in period 0) and refused by `improve` as a start, not repaired. Planted, reverted: a period equal to the count let through; the test failed.
- `FMINE-REF-506` (proposed), from `toposort::pcpsp::parse`: as for `cpit::parse` but reading negative uses (a negative limit is still refused), with a `TYPE` other than `PCPSP`, `NDESTINATIONS` of 0 or above `MAX_DESTINATIONS` (16), any general side constraint, an objective line without one value per destination, and a use for a destination out of range. Test: `pcpsp::tests::every_malformed_pair_refuses`, one row each.
- `FMINE-REF-207` (proposed), from `toposort::pcpsp`: `parse` when each block's value largest in magnitude, summed over the blocks, reaches `ABS_VALUE_SUM_LIMIT` (2⁶²) (test: `pcpsp::tests::every_malformed_pair_refuses`, at the limit's edge); `with_destinations`, and `greedy` with its first destinations, given the wrong count or a destination past the last (`greedy` also refuses ranks of the wrong count, and a result that fails `check`, as a defect; `greedy_from` also release periods of the wrong count or past the last period); `destinations` and `improve` given a schedule and destinations that fail `check`, or factors of the wrong length (`improve` also refuses negative or rising ones); and, as defects, a result that fails `check` or is worth less than the input. Test: `pcpsp::tests::bad_inputs_refuse`.

No value is clamped, rescaled or perturbed to make a problem fit.

## 6. Determinism class
`BitwisePortable`, declared on every file and checked by `xtask check-det`. Integer arithmetic only (`i64`, with `i128` sums in `new`, `verify` and `toposort`, and `i128` and `u128` in `slope`'s angle and cone tests); no hashing, threads or platform-dependent iteration order. `greedy`'s ready set is a binary heap keyed by `(rank, block id)`, a total order.

One exception is `toposort::pcpsp::price` (`bd-rbxh`). Its prices and its simplex use `f64`, with IEEE `+ − × ÷`, `abs`, `max` and comparisons only, in a fixed order: no libm call, `mul_add` or reduction whose order varies. These are correctly rounded, so the floats, and therefore Price's choices, are the same on every IEEE platform. The floats only guide. Price's repair, its acceptance and `destinations`' final `check` are in `i128`.

`toposort::lp` and `toposort::pcpsp::lp` (`bd-ubzs`) are the other `f64` files, under the same rule. The relaxation's coefficients are integers divided by `10^decimals`. Its discount factors are `1 / (1 + rate)^t`, the power taken by repeated squaring in the order `powi` uses, without a libm call; a test checks them against `powi` bit for bit. The solution's shares are read, summed and compared, and ranks are rounded with `round`, which is exact. The schedules they lead to are checked, valued and compared in `i128`.

## 7. Evidence class of outputs
Not applicable: no Claims are emitted, so no `EvidenceClass` or `KnowledgeState` is assigned.

## 8. Validity domain
- The pit is optimal for the integer values given. Any rounding in converting money to `i64` is the caller's, and its effect on the pit is not bounded here. `minelib`'s rounding is the exception: each rounded value moves any pit's value by at most half a unit in the last kept place, so the rounded problem's optimum lies within `rounded / 2` such units of the file's.
- Fewer than `u32::MAX` blocks and arcs, and absolute values summing below 2⁶².
- When several closed sets tie for the maximum, one of them is returned. Which one is fixed by the input but is not promised to be the smallest or the largest.
- Shells need non-negative revenue: the union rule rests on it, and the constructor refuses anything else. A sweep costs one full solve per factor, plus O(n + m) for each shell's check.
- Slope arcs are the usual K-level approximation of the true cone.
  - Within K layers, a block's closure holds its whole cone.
  - Further up it holds what chains of K-level cones reach, so a wall may be steeper there than its slope allows. On the 2·10⁶-block model below, the pit's value fell 2.1% from K = 4 to K = 8.
  - With one slope everywhere, no closure leaves the true cone. Where sectors or domains differ, chains can reach blocks outside a block's own cone: 3,441 of 109,782 closure blocks within K layers, 3.1%, in `slope_oracles`' random cases.
  - Near sides and domain contacts, each implied arc is tested against at most `WITNESSES` (8) shorter ones, so a few arcs a full search would drop are kept: 59,083 against 58,279 in those cases, 1.4% more. The closure is unchanged.
- Memory is linear in blocks and arcs. Each arc costs 24 bytes: a target, a tail and two incidence entries (`u32`), and a flow (`i64`). Each block costs two 8-byte and eight 4-byte fields of solver state, on top of its value and offset. The GATE-PUSH-PIT1 process for mclaughlin, Dinic and parsing included, peaked at 7.3 GB RSS.
- Schedules:
  - Limits are upper limits and not negative, so mining nothing is feasible; uses may be negative. A block on a precedence cycle is never mined by `greedy`.
  - At most `MAX_PERIODS` (65,536) periods. `check` and `greedy` keep a sum per period, so the bound also caps their memory when there are no resources. The `cpit` fuzz target's first crash was 4.4×10¹⁸ periods with no resources, which allocated before any check.
  - PCPSP values: one per block, the largest in magnitude over its destinations, must sum below 2⁶², so that every choice of destinations can be scheduled. The `pcpsp` fuzz target's first crash (2026-11-02) was a mill value of 7.8×10¹⁸ scaled units. The dump was chosen at first, so the problem was built; the search's move of that block to the mill was then refused. `parse` checked the arcs with zero values. It now checks them with these values.
  - `greedy` is a heuristic. It claims feasibility, not optimality; its distance from the optimum is measured against an LP bound from outside this crate (§9).
  - CPIT uses are rounded up and limits down, so a schedule within the scaled limits is within the file's. Values are rounded as UPIT values are.
- Wall times measured on one machine (aarch64-macos, release, 2026-11-01). They are local runs, not a benchmark suite.

  | Instance | Blocks | Arcs | Solve | Peak RSS |
  |---|---|---|---|---|
  | MineLib mclaughlin_limit | 112,687 | 3,035,483 | 0.22–0.26 s | — |
  | MineLib mclaughlin | 2,140,342 | 73,143,770 | 1.1–1.2 s | 7.3 GB, Dinic and parsing included |
  | `tests/scale_pit.rs` arcs_1e6 | 115,200 | 982,204 | 0.06 s | — |
  | `tests/scale_pit.rs` arcs_1e7 | 1,166,400 | 10,130,540 | 2.2 s | 0.43 GB, two models in one process |
  | `tests/scale_pit.rs` blocks_1e7 | 10,000,000 | 87,935,596 | 40.7 s | 3.1 GB |
  | `scale_pit` slope_2e6, K = 4 | 2,000,000 | 33,564,897 of 102,636,304 | 8.3 s, arcs 0.07 s | 1.0 GB |
  | `scale_pit` slope_2e6, K = 8 | 2,000,000 | 65,223,274 of 620,800,754 | 16.3 s, arcs 0.47 s | 1.8 GB |
  | `scale_pit` slope_1e7, K = 4 | 10,000,000 | 168,721,897 of 515,782,954 | 51.8 s, arcs 0.33 s | 5.0 GB |

  Plan §15.4's scale TARGET for `pitflow` is 10⁷ blocks and about 2×10⁸ implicit arcs in at most 120 s and 32 GB. slope_1e7 comes near it, with 1.69×10⁸ arcs stored explicitly, in 51.4 s and 5.0 GB. That is one synthetic model on one machine, with 15% fewer arcs than the target and none of them implicit, so the TARGET is not claimed as met.

  The slope rows use the blocks_1e7 ore model with slope arcs in place of the 3 × 3 ones. They have 10 m cubes and two domains: 45° to 55° by sector in the west half, and 60° in the east half. "Of" gives the full-cone count.

  The synthetic models cost far more per block than MineLib's. blocks_1e7 made 193M relabels against mclaughlin's 1.3M. Time grows faster than linearly in this family: 8.6× the blocks of arcs_1e7 took 18.6× the time.
- KILL-PUSH-PIT has two halves:
  - 10⁷ blocks in 10 minutes or less: `scale_pit` asserts this, and the one run took 40.7 s;
  - within 5× MineFlow on the largest MineLib UPIT: not run, because MineFlow is not available here.

## 9. Oracles
- Brute force over all subsets, on the unit tests' small instances.
- An independent Dinic maximum flow with `i128` capacities (`tests/common/mod.rs`), sharing no code with `pitflow`. Same author, so its agreement is same-origin evidence.
- External: MineLib's published UPIT optima (Espinoza, Goycoolea, Moreno and Newman, MineLib, Annals of Operations Research 2013; Results page mirrored at github.com/borelian/minelib), and the 26,086,899.03 Gurobi reports for newman1 in AMPL's MineLib notebook.

  GATE-PUSH-PIT1 run of 2026-11-01:

  | Instance | Blocks | Published optimum | Value read |
  |---|---|---|---|
  | newman1 | 1,060 | 26,086,899 | 26,086,899.025970 |
  | zuck_small | 9,400 | 1,422,726,898 | 1,422,726,897.734958 |
  | kd | 14,153 | 652,195,037 | 652,195,036.914887 |
  | zuck_medium | 29,277 | 1,075,124,490 | 1,075,124,490.289275 |
  | p4hd | 40,947 | 293,373,256 | 293,373,256 |
  | marvin | 53,271 | 1,415,655,436 | 1,415,655,436.136541 |
  | w23 | 74,260 | 510,973,998 | 510,973,998 |
  | zuck_large | 96,821 | 122,220,280 | 122,220,279.680000 |
  | sm2 | 99,014 | 2,743,603,730 | 2,743,603,730.300000 |
  | mclaughlin_limit | 112,687 | 1,495,726,474 | 1,495,726,474 |
  | mclaughlin | 2,140,342 | 1,495,886,962 | 1,495,886,962 |

  Every value read rounds to its published figure. On 2026-11-02 `fmine plan pit`, reading the files with `minelib` instead of the test's own parser, gave the same eleven values, rounded counts and pit sizes (`minelib_instances_match_gate_push_pit1_through_the_verb`, ignored, run by hand on one machine).
- Seven of the eleven files are read exactly at 10⁶. zuck_small (30 values), zuck_medium (88) and zuck_large (34) print float noise such as `4362.94593000001`, and marvin (4,874 values) carries ten to twelve decimals. Those values are rounded to 10⁻⁶ at read time, and the test widens its band by the resulting bound.
- MineFlow (Deutsch et al.) is not run.
- Schedules (`bd-kcpj`): brute force over every schedule of the unit tests' small problems for `check`, and MineLib's own CPIT schedules (`sols/<name>_cpit_gmunoz120723.sol` in the mirror) for `check` and the NPV convention. All ten that were run check feasible, and each NPV recomputes to within one unit of its published best-known value, with the discount counted from period 0. The LP bounds come from `fmine-lp`'s PDLP, whose dual bound is certified. PDLP is a sibling crate, not an external solver.

  `minelib_cpit_schedules_pass_the_checker_under_the_certified_bound` run of 2026-11-02 (ignored, run by hand; aarch64-macos, release, PDLP tolerance 10⁻⁴, 1,363.7 s for the ten instances). Ranks come from each block's LP period weights, and blocks below a final weight of 10⁻⁶, 0.5 or 0.9 are left out. `greedy` runs once per threshold, and the best NPV is kept. Every schedule passed `check` and stayed under both its certified bound and MineLib's published LP bound. The gap is (certified bound − NPV) / certified bound.

  | Instance | Blocks | Periods | LP solve | NPV | Certified bound | Gap | Of best known |
  |---|---|---|---|---|---|---|---|
  | newman1 | 1,060 | 6 | 0.1 s | 23,863,335 | 24,489,274 | 2.56% | 1.016 |
  | kd | 14,153 | 12 | 4.2 s | 372,132,947 | 413,354,914 | 9.97% | 0.938 |
  | zuck_small | 9,400 | 20 | 62.6 s | 730,614,526 | 854,193,026 | 14.5% | 0.926 |
  | p4hd | 40,947 | 10 | 37.4 s | 241,054,573 | 247,419,299 | 2.57% | 0.979 |
  | w23 | 74,260 | 12 | 37.2 s | 374,482,241 | 400,666,427 | 6.54% | 0.955 |
  | sm2 | 99,014 | 30 | 112.7 s | 1,622,531,548 | 1,648,501,297 | 1.58% | 0.986 |
  | marvin | 53,271 | 20 | 141.3 s | 745,899,525 | 863,937,582 | 13.7% | 0.909 |
  | zuck_medium | 29,277 | 15 | 221.7 s | 494,176,101 | 710,656,274 | 30.5% | 0.803 |
  | zuck_large | 96,821 | 30 | 568.9 s | 51,137,752 | 57,391,118 | 10.9% | 0.901 |
  | mclaughlin_limit | 112,687 | 15 | 155.3 s | 1,061,128,685 | 1,079,070,887 | 1.66% | 0.989 |

  Every certified bound but kd's is within 0.03% of MineLib's published LP bound. kd's is 0.94% above it, so against the published bound kd's gap is 9.12%. newman1's schedule is worth 1.6% more than MineLib's 2013 best-known file; later published values were not checked. mclaughlin (2,140,342 blocks) was not run. On zuck_large `greedy` mines every block. On eight of the ten, the mined count moves by under 0.1% between thresholds 10⁻⁶ and 0.9 (sm2's by 0.8%, kd's by 10%). How much of each gap comes from timing and how much from block choice was not measured. The count suggested timing, which the local search below works on.

  Local search (`bd-5lxu`): the same test rerun at 2a3e3fa on 2026-11-02 (aarch64-macos, release, each LP read from the cache an earlier run wrote). `search::improve` runs on each threshold's schedule, with `discount_factors` at the file's rate and at most 1,000 rounds, and the best schedule is kept. Every schedule passed `check`, no search lowered an NPV, and every NPV stayed under both bounds. Each best schedule was also written out in MineLib's format and checked by `crates/xtask/testdata/minelib_cpit_oracle.py`. That script reads only the MineLib pair and the schedule and sums in exact fractions. All ten were feasible, with NPVs equal to the test's to 10⁻³.

  Release periods (`bd-wife`): rerun on 2026-11-02 at 36a18de with `greedy_from` added, the same way. Each threshold now also gives a schedule with every block released at its LP median period: the first period by which the LP has mined half of the block, or the last period if it never has. The LP mines a required block at least as fast as the block itself, so no block is released before a block it requires. Six schedules are searched per instance. sm2's best rose 1.06%, from 1,623,480,554 (gap 1.52%, 0.987 of best known) to 1,640,623,307 (0.48%, 0.997), from threshold 0.5 released. The other nine kept their searched values and byte-identical schedules. Released, greedy itself did better on p4hd, w23, sm2, zuck_large and mclaughlin_limit, but on those four the search from the earliest-first schedule still came out ahead. sm2's new schedule passed the oracle, feasible, with an NPV equal to the test's to 10⁻³.

  Cone moves (`bd-ynoj`): rerun on 2026-11-02 at f6adfab plus the cone move, the same way. No searched value went down, since a cone is tried only where the old search stopped. Six rose:
  - zuck_medium by 8.12%, from 594,436,768 (gap 16.4%, 0.966 of best known) to 642,707,459;
  - zuck_small by 0.78%, marvin by 0.76%, w23 by 0.55%, newman1 by 0.42% and sm2 by 0.009%.

  The other four kept byte-identical schedules. The six new best schedules passed the exact oracle, feasible, with NPVs equal to the test's to 10⁻³. An earlier build of this bead, before the change to `pcpsp::improve`, gave byte-identical schedules. The table is this run. Greedy is the best of the six greedy schedules and Searched the best of the six searched.

  | Instance | Greedy | Searched | Gap | Of best known | Search time |
  |---|---|---|---|---|---|
  | newman1 | 23,863,335 | 23,988,996 | 2.04% | 1.022 | under 0.01 s |
  | kd | 372,132,947 | 393,795,083 | 4.73% | 0.992 | 0.12 s |
  | zuck_small | 730,614,526 | 822,141,680 | 3.75% | 1.042 | 0.19 s |
  | p4hd | 246,912,627 | 247,170,155 | 0.10% | 1.004 | 0.37 s |
  | w23 | 390,236,126 | 396,029,021 | 1.16% | 1.010 | 0.90 s |
  | sm2 | 1,634,277,708 | 1,640,765,421 | 0.47% | 0.997 | 1.96 s |
  | marvin | 745,899,525 | 831,803,179 | 3.72% | 1.013 | 0.70 s |
  | zuck_medium | 494,176,101 | 642,707,459 | 9.56% | 1.044 | 3.97 s |
  | zuck_large | 53,710,027 | 57,137,644 | 0.44% | 1.006 | 1.88 s |
  | mclaughlin_limit | 1,065,136,537 | 1,076,149,599 | 0.27% | 1.003 | 2.46 s |

  The gap is still to the certified bound. Search time is the total over the six schedules. All ten are within 3% of MineLib's best-known value: kd 0.8% and sm2 0.3% below it, the other eight at or above. Against the certified bound, six are within 3% and nine within 5%; zuck_medium is the outlier at 9.6%. Of the moves in the ten best schedules, 109,574 were "later" moves (a block of negative value put off to a period with a smaller factor), 13,412 cones, 10,134 swaps, 5,773 "earlier" moves, 65 dropped (all on kd) and none added. zuck_large's cached LP comes from a run PDLP stopped at its 600 s limit. Its certified bound is the same 57,391,118, and `greedy` on its ranks gives 51,185,071, not the 51,137,752 above (53,710,027 released).

  PDLP tolerance 10⁻⁶ (`bd-iw7m`, 2026-11-02 and 2026-11-03), the same test and code (2414f7a; `bd-rbxh` changes PCPSP only), with `FMINE_PDLP_TOL=1e-6` and LPs solved fresh, at most 600 s each.
  - Every certified bound comes within 0.0034% of MineLib's published LP bound. At 10⁻⁴, nine were within 0.03% and kd was 0.94% above.
  - The LPs took 3,590 s in all, against about 1,580 s at 10⁻⁴. Five stopped at the 600 s cap.
  - Eight searched values rose and two fell, so neither tolerance gives the better ranks everywhere. A tight bound did not mean a good point to rank from.
  - The harness default stays 10⁻⁴, and the table above stays the 10⁻⁴ run. Ranking from more than one LP point and keeping the best schedule is not built.
  - Every schedule in the table below passed the exact oracle: feasible, no problem, at NPVs equal to the test's to 10⁻³.

  | Instance | LP at 10⁻⁶ | Certified bound | Above published | Searched | At 10⁻⁴ | Change | Of best known |
  |---|---|---|---|---|---|---|---|
  | newman1 | optimal, 0.5 s | 24,486,188 | 0.0000% | 24,011,422 | 23,988,996 | +0.09% | 1.022 |
  | kd | optimal, 125.6 s | 409,511,851 | 0.0032% | 406,516,814 | 393,795,083 | +3.23% | 1.024 |
  | zuck_small | optimal, 197.8 s | 854,190,258 | 0.0009% | 829,063,066 | 822,141,680 | +0.84% | 1.051 |
  | p4hd | optimal, 127.2 s | 247,418,956 | 0.0013% | 247,223,012 | 247,170,155 | +0.02% | 1.004 |
  | w23 | optimal, 119.5 s | 400,660,466 | 0.0018% | 396,071,945 | 396,029,021 | +0.01% | 1.010 |
  | sm2 | stopped, 600.7 s | 1,648,060,717 | 0.0006% | 1,646,061,148 | 1,640,765,421 | +0.32% | 1.000 |
  | marvin | stopped, 600.8 s | 863,934,506 | 0.0021% | 842,971,109 | 831,803,179 | +1.34% | 1.027 |
  | zuck_medium | stopped, 602.6 s | 710,643,524 | 0.0003% | 638,403,145 | 642,707,459 | −0.67% | 1.037 |
  | zuck_large | stopped, 604.4 s | 57,391,041 | 0.0034% | 57,188,844 | 57,137,644 | +0.09% | 1.007 |
  | mclaughlin_limit | stopped, 610.6 s | 1,078,988,679 | 0.0009% | 1,074,914,865 | 1,076,149,599 | −0.11% | 1.001 |

  From MineLib's own schedules (`bd-3l5o`, 2026-11-03), at 10⁻⁴ with the cached LPs: each `sols/<name>_cpit_gmunoz120723.sol` is read by `cpit::parse_solution` as an untrusted candidate, checked, and searched from by `improve` with the harness's factors and rounds. All ten are raised, and each result passed the exact oracle (`crates/xtask/testdata/minelib_cpit_oracle.py`) feasible, at NPVs equal to the test's to 10⁻³. This is a warm start from MineLib's schedule. The cold-start rows above are unchanged.

  | Instance | MineLib's schedule, rechecked | Searched from it | Of best known | Rounds | Search time | Cold start at 10⁻⁴ | Cold start at 10⁻⁶ |
  |---|---|---|---|---|---|---|---|
  | kd | 396,858,192 | 405,742,817 | 1.0224 | 6 | 0.02 s | 393,795,083 | 406,516,814 |
  | sm2 | 1,645,242,774 | 1,647,092,291 | 1.0011 | 17 | 0.33 s | 1,640,765,421 | 1,646,061,148 |
  | newman1 | 23,483,671 | 23,864,851 | 1.0162 | 14 | 0.00 s | 23,988,996 | 24,011,422 |
  | zuck_small | 788,652,600 | 813,233,968 | 1.0312 | 16 | 0.02 s | 822,141,680 | 829,063,066 |
  | p4hd | 246,138,696 | 246,636,264 | 1.0020 | 4 | 0.05 s | 247,170,155 | 247,223,012 |
  | w23 | 392,226,063 | 396,046,533 | 1.0097 | 8 | 0.10 s | 396,029,021 | 396,071,945 |
  | marvin | 820,726,048 | 834,429,479 | 1.0167 | 15 | 0.08 s | 831,803,179 | 842,971,109 |
  | zuck_medium | 615,411,415 | 645,522,363 | 1.0489 | 14 | 0.28 s | 642,707,459 | 638,403,145 |
  | zuck_large | 56,777,190 | 57,054,925 | 1.0049 | 13 | 0.31 s | 57,137,644 | 57,188,844 |
  | mclaughlin_limit | 1,073,327,197 | 1,075,397,040 | 1.0019 | 11 | 0.34 s | 1,076,149,599 | 1,074,914,865 |

  - The search from MineLib's schedule gives the best of the three on sm2 (1,647,092,291, within 0.09% of its certified bound 1,648,501,297) and on zuck_medium. Elsewhere one of the cold starts is higher.
  - Every value is under its certified bound and MineLib's LP bound.
  - kd mines the same 12,154 blocks as MineLib's schedule, so its 2.24% gain comes from timing.
  - "Best known" is MineLib's 2013 Results page. Newer published values were not checked.

  PCPSP w23 at 10⁻⁶, over all of its blocks:
  - The LP stopped at 601.3 s. Its certified bound, 387,719,818, is 0.07% below the one at 10⁻⁴.
  - Before Price (2414f7a) the best schedule fell 0.40%, to 306,685,623.
  - With Price (ad169df) it rose 1.07% above the 10⁻⁴ run's, to 374,843,739, which is 3.32% below its own bound.
  - Both schedules passed the exact oracle.
- PCPSP schedules (`bd-2rne`): MineLib's own PCPSP schedules (`sols/<name>_pcpsp_gmunoz120723.sol` in the mirror; nine, none for w23), and `crates/xtask/testdata/minelib_pcpsp_oracle.py`. That script reads only the MineLib pair and a schedule, splits included, and sums in exact fractions.

  `minelib_pcpsp_schedules_pass_the_checker_and_report_gate_push_topo` (`crates/xtask/tests/pcpsp_schedule.rs`, ignored, run by hand) on 2026-11-02: aarch64-macos, release, PDLP tolerance 10⁻⁴ with at most 600 s per LP. The test:
  - takes the ultimate pit under each block's most valuable destination (`pitflow::solve`, certificate checked by `pitflow::verify`); only its blocks enter the LP. That leaves the LP's optimum and the schedule problem's unchanged; the argument is in the test's doc comment. It needs every use non-negative, so w23's LP has all its blocks;
  - solves the LP relaxation with PDLP, splits included, for a certified bound;
  - sends each block whole to the destination the LP sends most of it to;
  - ranks blocks by LP expected period as for CPIT. At each of the three thresholds it runs `pcpsp::greedy`, and `pcpsp::greedy_from` with each block released at its LP median period (`bd-tll7`), then `pcpsp::improve` (at most 1,000 rounds) on each, and keeps the best of the six.

  Every schedule passed `check`, no search lowered an NPV, and every NPV stayed under both bounds. A second run from the LP cache, with the `parse` fix of §8 built in, gave the same values and byte-identical schedules. Each best schedule passed the exact oracle, with NPVs equal to the test's to 10⁻³. Rerun for `bd-ixfw` on 2026-11-02: w23's LP was solved fresh, and the other nine were read from the cache. Those nine gave the same values and byte-identical schedules, since the fallback in `pcpsp::greedy` never fired on them; w23's row is new. Rerun for `bd-tll7` the same day, all ten from the cache. The released schedules raised w23's searched NPV by 11.0% and sm2's by 0.8%; both new best schedules passed the exact oracle. They raised the greedy value on kd, mclaughlin_limit, sm2, zuck_large and mclaughlin, and no searched value but w23's and sm2's.

  Rerun for `bd-ynoj` the same day, all ten from the cache. `pcpsp::improve` now tries cones once period and destination moves have both stopped, so no searched value can go down, and none did.
  - w23 rose 1.77% to 307,904,500, 20.6% below its certified bound.
  - marvin rose 0.35%, newman1 0.13%, sm2 0.002% and zuck_small 0.0002%. The other five kept byte-identical schedules.
  - The five new best schedules passed the exact oracle, feasible with no problem, at NPVs equal to the test's to 10⁻³.

  Two variants allowed cones within every period search, from the first destination pass on. In one, cones were tried in every round; in the other, once each search's single-block moves stopped.
  - Both raised zuck_medium by about 1.9% and zuck_small by about 0.5%.
  - Both left w23 at 272,688,147 (−9.9%), its value before `bd-tll7`. The first also left sm2 0.006% lower.
  - The cones took early room that destination switches had used: on w23's released schedules, switches fell from about 1,040 to between 180 and 310.

  Neither variant was kept.

  Rerun for `bd-rbxh` on 2026-11-03, all ten from the cache, with Price in `destinations`. Price is accepted only when it raises a period's value, so no searched value can go down, and none did.
  - w23 rose 20.46%, from 307,904,500 to 370,886,746. Its gap to the certified bound fell from 20.6% to 4.41%.
  - zuck_small rose 0.18%, zuck_medium 0.10%, sm2 0.046% and mclaughlin_limit 0.015%. mclaughlin rose by 4,192, or 0.0003%.
  - newman1, kd, marvin and zuck_large kept byte-identical schedules. Price made no move on newman1, kd or marvin. On zuck_large it made two, and the search still ended at the same schedule.
  - In the best schedules, Price reassigned 36 periods on w23, 13 on zuck_small, 11 on zuck_medium, 16 on sm2, 15 on mclaughlin_limit and 21 on mclaughlin. Counts are summed over `improve`'s cycles.
  - The six new best schedules passed the exact oracle: feasible, no problem, at NPVs equal to the test's to 5×10⁻⁴.

  Diagnosis of w23, done outside the repository with HiGHS through SciPy 1.18.1 on the instance file: the mining periods of each best schedule were kept, and each period's destination problem was solved alone.
  - Before Price, the destination LP was worth 372,394,083, 20.9% more than the schedule. Whole blocks (HiGHS's MIP) were worth 372,362,605, and the LP split two or three blocks per period. The value had been lost to destinations, not to periods.
  - After Price, the destination LP at the new schedule's periods is worth 370,939,782, only 0.014% more than the schedule. What remains of the 4.41% gap lies in the mining periods, or in the LP relaxation's own gap.

  The table is the `bd-rbxh` run.

  Rerun for `bd-ubzs` on 2026-11-02, both tests on all ten instances from the cache, through `toposort::lp` and `toposort::pcpsp::lp` in place of each test's own relaxation, ranks, starts and pick. All twenty best schedules were byte-identical to the `bd-ynoj` (CPIT) and `bd-rbxh` (PCPSP) runs, and every reported value, gap, count and gate line was the same; only the timings differ. newman1's LPs, solved fresh by PDLP from the crate's formulations, were bitwise equal to the cached points and bounds, for CPIT and PCPSP.

  | Instance | Blocks | In the LP | LP solve | Greedy | Searched | Certified bound | Gap | Of best known |
  |---|---|---|---|---|---|---|---|---|
  | newman1 | 1,060 | 1,059 | 4.0 s | 23,932,689 | 23,983,606 | 24,496,143 | 2.09% | 1.014 |
  | kd | 14,153 | 12,154 | 64.3 s | 379,799,033 | 408,065,484 | 411,334,938 | 0.79% | 1.003 |
  | zuck_small | 9,400 | 9,399 | 168.2 s | 787,518,026 | 883,815,459 | 913,089,333 | 3.21% | 1.013 |
  | zuck_medium | 29,277 | 27,387 | 137.9 s | 630,984,945 | 706,229,924 | 760,479,523 | 7.13% | 1.045 |
  | w23 | 74,260 | 74,260 (pit 51,315) | 219.5 s | 252,826,676 | 370,886,746 | 387,980,079 | 4.41% | best known 0 |
  | marvin | 53,271 | 8,516 | 206.6 s | 834,859,361 | 898,875,170 | 917,322,920 | 2.01% | 1.015 |
  | mclaughlin_limit | 112,687 | 110,768 | 215.7 s | 1,309,846,722 | 1,323,650,701 | 1,325,368,622 | 0.13% | 1.002 |
  | sm2 | 99,014 | 18,388 | 277.9 s | 1,639,435,326 | 1,647,670,312 | 1,653,131,545 | 0.33% | 0.998 |
  | zuck_large | 96,821 | 96,821 | 316.9 s | 53,332,163 | 57,735,942 | 58,039,350 | 0.52% | 1.007 |
  | mclaughlin | 2,140,342 | 180,749 | 587.3 s | 1,500,066,364 | 1,511,675,539 | 1,513,152,607 | 0.10% | 1.001 |

  GATE-PUSH-TOPO (plan §16: within 3% of the best-known value on at least 80% of the instances, within 5% on all) is met as written, with one qualification. w23's best-known value is 0, because MineLib publishes no feasible PCPSP schedule for it, so any schedule worth at least 0 meets it. The test counts w23 that way and names it in the gate line's `best_known_zero`. Its schedule is 4.41% below its certified bound (20.6% before `bd-rbxh`), so it would pass the gate's 5% even against the bound. The other nine are within 3% of their best-known values: eight at or above, sm2 0.2% below. Against the certified bound, seven of the ten are within 3% and nine within 5%; zuck_medium is at 7.1%. The best-known values are from MineLib's 2013 Results page; newer published values were not checked.

  - w23 (`bd-ixfw`): the certified bound is 0.07% above MineLib's published LP bound (387,693,394). Plain `greedy` on the same ranks gave 33,008,025 and the search 155,643,355, with nothing mined after period 2. A block above a blend target that fit at the mill in no period stayed unmined, and so did every block under it. With `pcpsp::greedy`'s fallback the searched NPV was 272,688,147. The mill was full in periods 0 to 4, and 2,829 of the 10,364 blocks mined of the 11,969 worth something at the mill went to the dump: earliest-first placement used up the blocks below the blend targets in the first periods.
  - w23 (`bd-tll7`): released at their LP median periods, the best schedule is worth 302,546,829. Of the 11,969 blocks worth something at the mill, 10,401 are mined and 1,357 of them go to the dump. The LP itself fills the mill in periods 0 to 4 and to 96% in period 5, then uses 5% of it in period 6 and at most 1% in each period after. That is its cached point, worth 388,484,235 and slightly infeasible within PDLP's tolerance. So the empty later periods are not where the value is lost; the earlier diagnosis (`bd-ixfw`) blamed them wrongly. The schedule's mill runs at 88%, 100%, 100%, 75%, 98% and 72% in periods 0 to 5, and its blends are less valuable than the LP's fractional ones. In periods 0 to 5 its undiscounted value is between 56% (period 5) and 85% (period 0) of the LP's. The exact oracle found the schedule feasible, with no problem, at the test's NPV.
  - w23 (`bd-ynoj`): with cones after the other moves stop, 307,904,500. The mill and blend figures above are for the `bd-tll7` schedule and were not measured again.
  - w23 (`bd-rbxh`): with Price, 370,886,746, mining 46,547 blocks. The `bd-tll7` diagnosis above was right that the blends were the loss, but it did not say why: switches and exchanges could not move value into a blend that needs several blocks to move at once.

  - Each certified bound is 0.01% (mclaughlin) to 1.33% (zuck_medium) above MineLib's published LP bound, so the gaps against the published bounds are smaller.
  - On newman1, kd, zuck_small and zuck_medium the LP was also solved over every block. The certified bounds agree with the pit's within 0.36%, which fits one optimum under PDLP's certification slack.
  - The schedules depend more on the LP point PDLP returns. From the full LP's ranks, zuck_medium's best was 672,633,956 (0.995 of best known), kd's 408,816,605 and zuck_small's 889,990,164. How widely this varies was not measured further.
  - Rounding: values were rounded at 10⁻⁶, each by at most 5×10⁻⁷: zuck_small 31, zuck_medium 88, zuck_large 34, marvin 6,564, w23 276,617, mclaughlin_limit 10,853 and mclaughlin 2,292,503 values. w23 also had 5,941 uses, printed to 12 places, rounded up. The six searches took at most 11.4 s per instance (w23), and 73.5 s on mclaughlin (`bd-rbxh` run). They took 4.3 s and 51.3 s before Price (`bd-ynoj` run), and 3.2 s and 39.0 s before the cone move.
  - MineLib's nine schedules, valued by the test and by the oracle, come to their published values within 9 units; marvin's is 885,968,061.5 against 885,968,070. Their fractions are printed to six places. Some single fractions read 1.000001, a block's fractions sum to 1 within 10⁻⁴, and limits are exceeded by at most 1.1×10⁻⁸ of the limit. Those are the only problems the exact oracle finds (none on newman1, 2 to 518 elsewhere), so they are infeasible by rounding only: no precedence is unmet and no block is in two periods. `check`, which takes whole blocks, does not read them.

## 10. Known gaps
- `bd-nzwd` and `bd-t75z` have one caller, `fmine-io`'s `omf::pit::PitModel` (bd-joiv). It feeds `slope_arcs`, `solve` and `nested_shells` a regular OMF block model whose values are attributes, tested so far only on synthetic files. `BD-250`'s other caller is `fmine plan pit`, on MineLib UPIT pairs only. No price deck feeds any of them, and no `fmine` verb reads OMF (fmine-cli is over its LOC budget).
- PCPSP (`toposort::pcpsp`):
  - Blocks are not split between destinations, so a schedule can be worth less than PCPSP's best. The LP relaxation splits them.
  - General side constraints and lower limits are not read.
  - w23's schedule is 4.41% below its certified bound (§9). At its own mining periods, the destination LP is worth only 0.014% more, so what remains is in the mining periods or in the LP relaxation's gap. No move changes a block's period with the destination LP in view.
  - Price (`bd-rbxh`) solves each period's destination LP approximately, in `f64`: there is no anti-cycling rule, and at most `PRICE_COLUMNS` (256) columns, `MASTER_STEPS` (1,024) simplex steps per solve and `REPAIR_MOVES` (256) repair moves. A weak solve costs value, never feasibility. It runs at most once per period per `destinations` call, and only after switches and exchanges stop. On MineLib, w23's six searches went from 4.1 s to 11.4 s and mclaughlin's from 51.3 s to 73.5 s (§9).
  - Destination moves stay within a period; a block never changes period and destination in one move.
  - MineLib's own split schedules are valued only in `f64` (the xtask test) and by the exact oracle script; nothing in this crate reads a split schedule.
- Schedules:
  - `search::improve` moves one block or trades two at a time, and its cone (`bd-ynoj`) moves a block with what it requires, but only within one period. The cone goes one period earlier at a time and holds at most 64 blocks. Cones wait for the other moves to stop. On PCPSP that costs zuck_medium the 1.9% that earlier cones found, which is the price of not losing w23's 9.9% (§9). It has no "repair" of infeasible schedules; it only takes feasible ones. Its moves are monotone, so it stops at the first local optimum, with no perturbation or restart.
  - The discount factors are rounded down period by period, so `improve`'s value can differ from the exact NPV in the last of about 12 significant digits. Every move it makes also raises the exact NPV, since each one's sign is that of a difference of factors.
  - `check` lives in this crate beside `greedy`, not in the separate `fmine-verify` crate plan §15.4 names. It shares no code with `greedy` beyond the problem type.
  - The LP bridge (`bd-ubzs`) formulates the relaxation and reads its solution back, but does not solve it: `fmine-lp` is in the same layer as this crate, and the topology allows no edge between them. The caller solves it; today only `crates/xtask/tests/cpit_schedule.rs` and `pcpsp_schedule.rs` do, with PDLP.
  - No `fmine` verb schedules: `fmine-cli` is at its planned LOC budget, and BUDGET-AMEND-002 awaits the owner.
- `candidate` reads a plain id list only, and `cpit::parse_solution` only MineLib's CPIT solution format (no destinations, no part blocks; no PCPSP reader). Which vendor formats (Whittle, Deswik, COMET) should be read is OPEN-PUSH-4. An OMF schedule cannot yet come in this way: `fmine-io` has no reader keyed by OMF block order. The candidate check was built ahead of the owner's answer to OPEN-PUSH-1, on its recommended answer (data files only, no linked code); it is removed if the owner rules otherwise.
- Slope arcs have no external gate. MineLib's UPIT files come with their arcs already made, and the workspace holds no geotechnical slope data. The slopes in the tests are invented.
- Slope arcs cover regular grids only: no sub-blocks, rotated grids, or slopes that vary with depth inside a domain.
- With more than one domain, the prefix sums cost 4 bytes per block on top of the arcs.
- Shells have no MineLib gate, because the UPIT files give one value per block with revenue and cost not split. Block models and random graphs stand in for it.
- MineLib's files are not in the repository, because their licence is not recorded. GATE-PUSH-PIT1 is an ignored test, run by hand with `FMINE_MINELIB_DIR`, and CI does not run it.
- No current-arc pointer: each merger search rescans all the node's incident arcs from the first. Whether a pointer that skips arcs already passed over is valid under these invariants has not been worked out. There is no highest-label or FIFO variant.
- Not yet built:
  - warm starts across revenue factors, since each factor is a fresh solve;
  - a minimal or maximal choice among tied pits. The shell union gives nesting without one.
- KILL-PUSH-PIT's MineFlow half is not run (§8). Its 10⁷-block half is an ignored test, run by hand on one machine; CI does not run it.
- No fnx (franken_networkx) graph code is used. Precedence graphs are this crate's own CSR (`BD-266`, the fallback in plan §27.4's fnx row), and fnx's `preflow_push` has not been run as an oracle.

## 11. Public API
- `pitflow::ClosureProblem`: `new`, `from_requirements`, `node_count`, `arc_count`, `weights`, `requires`
- `pitflow::PitSolution`: `in_pit`, `value`, `arc_flow`, `pit_size`, `stats`
- `pitflow::PitStats` (`mergers`, `splits`, `relabels`, `max_label`)
- `pitflow::solve`, `pitflow::verify`
- `pitflow::ABS_VALUE_SUM_LIMIT`
- `pitflow::candidate::parse_ids`, `pitflow::candidate::check`, `pitflow::candidate::CandidateCheck` (`blocks`, `value`, `unmet`, `first_unmet`, `closed`), `pitflow::candidate::WITNESSES`
- `pitflow::minelib::parse`, `pitflow::minelib::scaled`, `pitflow::minelib::UpitModel` (`name`, `problem`, `rounded`), `pitflow::minelib::Rounding`, `pitflow::minelib::MAX_DECIMALS`
- `pitflow::shells::RevenueFactor` (`new`, `num`, `den`)
- `pitflow::shells::ShellModel` (`new`, `from_requirements`, `block_count`, `values_at`)
- `pitflow::shells::NestedShells` (`first_shell`, `shells`, `contains`)
- `pitflow::shells::ShellSummary`
- `pitflow::shells::nested_shells`
- `pitflow::slope::BlockGrid` (`new`, `block_count`, `index`)
- `pitflow::slope::SlopeTangent` (`new`), `pitflow::slope::Sector` (`new`), `pitflow::slope::SlopeTable` (`new`, `uniform`)
- `pitflow::slope::SlopeArcs` (`offsets`, `targets`, `arc_count`, `full_cone_arcs`, `boundary_blocks`, `into_csr`)
- `pitflow::slope::slope_arcs`
- `pitflow::slope::MAX_PATTERN`, `MAX_SCAN`, `WITNESSES`
- `pitflow::minelib::Rounding` (`Refuse`, `HalfAwayFromZero`, `Ceiling`, `Floor`)
- `toposort::ScheduleProblem` (`new`, `blocks`, `periods`, `resources`, `usage`, `limits`)
- `toposort::ScheduleCheck` (`mined`, `period_values`, `used`, `unmet`, `first_unmet`, `over_limit`, `feasible`, `value`), `toposort::WITNESSES`, `toposort::MAX_PERIODS`
- `toposort::search::improve`, `toposort::search::improve_without_cones`, `toposort::search::Improvement` (`schedule`, `before`, `after`, `rounds`, `moves`), `toposort::search::Moves` (`later`, `earlier`, `cones`, `swaps`, `added`, `dropped`), `toposort::search::discount_factors`, `toposort::search::discounted_value`, `toposort::search::DISCOUNT_SCALE`, `toposort::search::SWAP_SCAN`, `toposort::search::CONE_LIMIT`
- `toposort::check`, `toposort::greedy`, `toposort::greedy_from`
- `toposort::cpit::parse_solution`, `toposort::cpit::write_solution` (`bd-3l5o`)
- `toposort::cpit::parse`, `toposort::cpit::CpitModel` (`name`, `problem`, `discount_rate`, `rounded_values`, `rounded_uses`, `rounded_limits`), `toposort::cpit::MAX_RESOURCES`, `toposort::cpit::RATE_SCALE`
- `toposort::pcpsp::parse`, `toposort::pcpsp::PcpspModel` (`name`, `discount_rate`, `rounded_values`, `rounded_uses`, `rounded_limits`, `blocks`, `periods`, `destinations`, `resources`, `value`, `uses`, `limits`, `requires`, `with_destinations`), `toposort::pcpsp::greedy`, `toposort::pcpsp::greedy_from`, `toposort::pcpsp::destinations`, `toposort::pcpsp::DestinationMoves` (`switches`, `exchanges`, `priced`, `unmined`), `toposort::pcpsp::improve`, `toposort::pcpsp::PcpspImprovement` (`schedule`, `destination`, `before`, `after`, `cycles`, `moves`, `destination_moves`), `toposort::pcpsp::MAX_DESTINATIONS`, `toposort::pcpsp::price::PRICE_COLUMNS`, `toposort::pcpsp::price::MASTER_STEPS`, `toposort::pcpsp::price::REPAIR_MOVES`
- `toposort::lp::relaxation`, `toposort::lp::Relaxation` (`cost`, `matrix`, `row_lo`, `row_hi`, `col_lo`, `col_hi`), `toposort::lp::Shares` (`new`, `blocks`, `periods`, `share`, `expected`, `medians`), `toposort::lp::starts`, `toposort::lp::Start` (`threshold`, `released`, `schedule`), `toposort::lp::incumbent`, `toposort::lp::Incumbent` (`searched`, `best`, `schedule`), `toposort::lp::THRESHOLDS`. `Shares::ranks` is crate-only: `starts` takes its thresholds from `THRESHOLDS`, and a public bare-`f64` threshold would need an L-6 exception.
- `toposort::pcpsp::lp::scope`, `toposort::pcpsp::lp::Scope` (`blocks`, `pit_blocks`, `pit_value`, `whole`), `toposort::pcpsp::lp::relaxation`, `toposort::pcpsp::lp::Solution` (`new`, `shares`, `destinations`), `toposort::pcpsp::lp::starts`, `toposort::pcpsp::lp::Start` (`threshold`, `released`, `schedule`, `destination`), `toposort::pcpsp::lp::incumbent`, `toposort::pcpsp::lp::Incumbent` (`searched`, `best`, `chosen`)

## 12. Proof obligations
- PO-PUSH-3 (nested shells are monotone in λ when block values are monotone in λ; plan §15.9 names its discharge "property test"):
  - discharged by `tests/shells_oracles.rs`, under §4's conditions;
  - the union lemma in `pitflow::shells`' documentation is the argument for why it holds. It is not machine-checked.
- PO-PUSH-5 (the schedule is feasible: precedence across periods, capacities; "Independent checker"): `toposort::check`, tested against the definition (§4), and run by `greedy` on every schedule it returns. It is independent of `greedy` in code only, not in crate or author (§10).
- No PO-* covers slope arcs. That dropping implied arcs keeps every closure is argued by induction in `pitflow::slope`'s documentation, not machine-checked; `tests/slope_oracles.rs` checks it on random cases.
- No other PO-* is discharged here. That the certificate conditions imply optimality is the max-flow/min-cut theorem applied to the closure network (Picard 1976), stated in the module documentation and not machine-checked. That pseudoflow terminates with such a certificate is Hochbaum's (2008) result. The tests check values against oracles and do not prove either.
