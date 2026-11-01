# CONTRACT.md — `fmine-optimize`

## 1. Purpose
The Pushback optimisation crate. One slice so far:
- `pitflow` (`BD-250`, plan §15.4): the ultimate pit as a maximum-weight closure, by Hochbaum's pseudoflow (lowest-label variant, gap termination) on an `i64` CSR precedence graph. Each pit comes with its pseudoflow certificate, which `verify` checks without the solver.

Nothing in the workspace calls it yet. Slope-template arc generation, nested shells (parametric λ), Bienstock–Zuckerberg, Lagrangian, TopoSort and the stochastic layer are later slices.

- **Crate:** `fmine-optimize`
- **Layer:** E
- **Subsystem:** Pushback
- **LOC budget:** 36,000 planned / 41,000 ceiling
- **Allowed Module Families:** `["pitflow"]`

## 2. Inputs
- One `i64` value per block, in a caller-chosen integer money unit; the caller scales and rounds, never this crate.
- Precedence arcs `u → v`, read "taking `u` requires taking `v`", either as CSR (`ClosureProblem::new(weights, offsets, targets)`) or as one list per block (`from_requirements`). Duplicate arcs and self-loops are accepted. Cycles are accepted too: a cycle's blocks are taken together or not at all.
- No frame, slope or block geometry: deriving arcs from slope templates is the caller's for now.

## 3. Outputs
- `PitSolution`: `in_pit` (one flag per block), `value` (the pit's total, the maximum over all closed sets), `pit_size`, `arc_flow` (the certificate, one flow per arc in CSR order) and `stats` (`PitStats`: mergers, splits, relabels, largest label).
- `verify(problem, in_pit, arc_flow)`: the pit's value if the certificate holds.

None is a Claim.

## 4. Invariants
- The returned pit is closed and its value is the maximum over all closed sets. `solve` runs `verify` before returning, so an uncertified pit is refused, never returned.
- `verify` accepts exactly when the pit is closed, all flows are non-negative, no arc entering the pit carries flow, every block in the pit has non-negative excess and every block outside non-positive excess. These make the pit the source side of a minimum s–t cut. It shares no code with the solver.
- The solver's forest invariants (labels monotone down each tree, `label(x) ≤ label(y) + 1` on residual arcs, flow only on tree arcs, excess only at roots, weak roots at label 0, every strong root in its bucket) are checked after every step on 3,000 random small instances (`random_small_instances_match_brute_force_and_keep_invariants`), and each value equals a brute force over all subsets.
- Values equal an independent Dinic maximum flow (`tests/common`, no shared code) on 300 random graphs (`random_graphs_match_dinic`) and on 15 open-pit-shaped block models up to 30 × 30 × 8 with 3 × 3 slope cones (`block_models_match_dinic`). `block_model_pit_is_a_union_of_cones` checks closure on a block model.
- The same input gives the same pit, flows and counters (`rerun_is_identical`).
- `verify` refuses planted bad certificates: an unclosed pit, flow into the pit, a wrong-signed excess, a negative flow (`verify_refuses_planted_certificates`).
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

No value is clamped, rescaled or perturbed to make a problem fit.

## 6. Determinism class
`BitwisePortable`, declared on every file and checked by `xtask check-det`. Integer arithmetic only (`i64`, with `i128` sums in `new` and `verify`); no floating point, hashing, threads or platform-dependent iteration order.

## 7. Evidence class of outputs
Not applicable: no Claims are emitted, so no `EvidenceClass` or `KnowledgeState` is assigned.

## 8. Validity domain
- The pit is optimal for the integer values given. Any rounding in converting money to `i64` is the caller's, and its effect on the pit is not bounded here.
- Fewer than `u32::MAX` blocks and arcs, and absolute values summing below 2⁶².
- When several closed sets tie for the maximum, one of them is returned. Which one is fixed by the input but is not promised to be the smallest or the largest.
- Memory is linear in blocks and arcs. Each arc costs 24 bytes: a target, a tail and two incidence entries (`u32`), and a flow (`i64`). Each block costs two 8-byte and eight 4-byte fields of solver state, on top of its value and offset. The GATE-PUSH-PIT1 process for mclaughlin, Dinic and parsing included, peaked at 7.3 GB RSS.
- Measured once on one machine (aarch64-macos, release, GATE-PUSH-PIT1 runs of 2026-11-01): 0.255 s for mclaughlin_limit (112,687 blocks, 3,035,483 arcs) and 1.243 s for mclaughlin (2,140,342 blocks, 73,143,770 arcs). These are single local runs, not a benchmark. KILL-PUSH-PIT (within 5× MineFlow on the largest MineLib UPIT, and 10⁷ blocks in 10 minutes or less) has not been run, because MineFlow is not available here.

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

  Every value read rounds to its published figure.
- Seven of the eleven files are read exactly at 10⁶. zuck_small (30 values), zuck_medium (88) and zuck_large (34) print float noise such as `4362.94593000001`, and marvin (4,874 values) carries ten to twelve decimals. Those values are rounded to 10⁻⁶ at read time, and the test widens its band by the resulting bound.
- MineFlow (Deutsch et al.) is not run.

## 10. Known gaps
- `BD-250` (bd-250): no caller in the workspace yet. The first will be the pushback/nested-shell slice.
- MineLib's files are not in the repository, because their licence is not recorded. GATE-PUSH-PIT1 is an ignored test, run by hand with `FMINE_MINELIB_DIR`, and CI does not run it.
- No current-arc pointer: each merger search rescans all the node's incident arcs from the first. Whether a pointer that skips arcs already passed over is valid under these invariants has not been worked out. There is no highest-label or FIFO variant.
- No slope-template arc generation (plan §15.4's 45° cone templates), no nested shells or parametric λ sweep, no warm start, and no minimal or maximal choice among tied pits.
- No performance gate. KILL-PUSH-PIT is not run (§8), and no 10⁷-block instance exists here.

## 11. Public API
- `pitflow::ClosureProblem`: `new`, `from_requirements`, `node_count`, `arc_count`, `weights`, `requires`
- `pitflow::PitSolution`: `in_pit`, `value`, `arc_flow`, `pit_size`, `stats`
- `pitflow::PitStats` (`mergers`, `splits`, `relabels`, `max_label`)
- `pitflow::solve`, `pitflow::verify`
- `pitflow::ABS_VALUE_SUM_LIMIT`

## 12. Proof obligations
No PO-* is discharged here. That the certificate conditions imply optimality is the max-flow/min-cut theorem applied to the closure network (Picard 1976), stated in the module documentation and not machine-checked. That pseudoflow terminates with such a certificate is Hochbaum's (2008) result. The tests check values against oracles and do not prove either.
