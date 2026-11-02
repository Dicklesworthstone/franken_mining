# CONTRACT.md — `fmine-optimize`

## 1. Purpose
The Pushback optimisation crate. Three slices so far:
- `pitflow` (`BD-250`, plan §15.4): the ultimate pit as a maximum-weight closure, by Hochbaum's pseudoflow (lowest-label variant, gap termination) on an `i64` CSR precedence graph. Each pit comes with its pseudoflow certificate, which `verify` checks without the solver.
- `pitflow::shells` (`bd-nzwd`, plan §15.4 and PO-PUSH-3): nested pit shells across an increasing list of exact revenue factors. These are the pit-by-pit sequence from which pushbacks are chosen.
  - Shell `k` is the union of the pit solved at factor `k` and shell `k − 1`.
  - With non-negative revenue that union is a best pit at factor `k`; the module documentation gives the proof.
  - Each shell's closure, and its value against the certified optimum, is checked before it is returned.
- `pitflow::slope` (`bd-t75z`, plan §15.4: "implicit precedence arcs from slope templates (azimuth sector × geotechnical domain from §14.2)"): precedence arcs for a regular block grid, from exact slope tangents per domain and azimuth sector.
  - Each block requires every block whose centre lies in its upward cone, up to K layers above.
  - Arcs implied by shorter ones are dropped, and every block's closure stays the same.
- `pitflow::minelib` (`bd-ip99`): MineLib's UPIT text pair (`.upit` values, `.prec` precedence) read into a `ClosureProblem`. No I/O.
  - Values are plain decimals scaled to `i64` at a declared number of places, at most `MAX_DECIMALS` (9).
  - A value with more non-zero places is refused unless the caller passes `Rounding::HalfAwayFromZero`; each rounded value is counted in `UpitModel::rounded`.
  - Every block must appear exactly once in each file. Headers, counts, ids and duplicates are checked; nothing is defaulted.

`fmine plan pit` (`fmine-cli`, `bd-ip99`) calls `minelib::parse`, `solve` and `verify`. Shells and slope arcs have no caller yet. Bienstock–Zuckerberg, Lagrangian, TopoSort and the stochastic layer are later slices.

- **Crate:** `fmine-optimize`
- **Layer:** E
- **Subsystem:** Pushback
- **LOC budget:** 36,000 planned / 41,000 ceiling
- **Allowed Module Families:** `["pitflow"]`

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
  - a value that is not a plain decimal, does not fit `i64` at the places declared, or has non-zero places past them under `Rounding::Refuse`; more than 9 declared places;
  - an objective line count that differs from `NBLOCKS`, or a block id out of range or given twice;
  - a precedence line whose count differs from its list, that requires itself, an out-of-range block or one block twice, or a block with no precedence line or two.

  Tests: `minelib::tests::every_malformed_pair_refuses`, one row each, and `crates/xtask/tests/plan_pit.rs` through the CLI.

No value is clamped, rescaled or perturbed to make a problem fit.

## 6. Determinism class
`BitwisePortable`, declared on every file and checked by `xtask check-det`. Integer arithmetic only (`i64`, with `i128` sums in `new` and `verify`, and `i128` and `u128` in `slope`'s angle and cone tests); no floating point, hashing, threads or platform-dependent iteration order.

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

## 10. Known gaps
- `bd-nzwd` and `bd-t75z`: no caller in the workspace yet. `BD-250`'s one caller is `fmine plan pit`, on MineLib UPIT pairs only; no block model or price deck feeds it.
- `minelib` reads the UPIT pair only, not MineLib's CPIT or PCPSP files (capacities, periods, destinations).
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

## 12. Proof obligations
- PO-PUSH-3 (nested shells are monotone in λ when block values are monotone in λ; plan §15.9 names its discharge "property test"):
  - discharged by `tests/shells_oracles.rs`, under §4's conditions;
  - the union lemma in `pitflow::shells`' documentation is the argument for why it holds. It is not machine-checked.
- No PO-* covers slope arcs. That dropping implied arcs keeps every closure is argued by induction in `pitflow::slope`'s documentation, not machine-checked; `tests/slope_oracles.rs` checks it on random cases.
- No other PO-* is discharged here. That the certificate conditions imply optimality is the max-flow/min-cut theorem applied to the closure network (Picard 1976), stated in the module documentation and not machine-checked. That pseudoflow terminates with such a certificate is Hochbaum's (2008) result. The tests check values against oracles and do not prove either.
