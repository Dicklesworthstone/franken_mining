# CONTRACT.md — `fmine-claim`

## 1. Charter & Layer
- **Crate:** `fmine-claim`
- **Layer:** `S0` (Substrate)
- **Subsystem:** `Bedrock`
- **LOC Budget:** `14k planned / 16k ceiling` (`registries/loc_budget.toml`)
- **Allowed Module Families:** `["claim", "dag", "label", "uu"]`

## 2. Mathematical & Domain Formulation
Implements `Claim<T>`, `Representation`, `SpatialSupport`, `AdditivityClass`, `Confidentiality`, `UuTerm` (`INV-015`), non-lossy binary composition (`compose_binary`, `NL-1..NL-8`), and `ClaimDag` transitive staleness propagation (`INV-013`).

`Confidentiality` (§31.1) is a product lattice: an internal bit, a set of ventures and a set of MNPI walls. Join is
OR on the bit and union on each set; `a ⊑ b` holds when each part of `a` sits under `b`'s. Public is bottom.
Ventures and walls come only from a `LabelRegistry`, which refuses malformed or repeated ids, so every id in a label
is well formed and was held by some registry. Any caller can build a registry, so it is not a trust root: the
deployment builds its own from its venture and wall list. Distinct walls, and distinct ventures, are incomparable:
there is no total order, and `<` does not compile (`compile_fail` doctest). §31.1 writes the labels as a chain; the
product refines it so that two ventures, or two walls, never collapse into one tier. The owner should confirm the
refinement.
`compose_binary` labels a result with the join of its parents' labels, so composing can only raise a label.

A label's claim-id bytes keep the old tier bytes, 0 for public and 1 for internal, so no existing claim id moves; any
other label is tag 5, then its ventures and walls.

Identity (§8.1, BD-040): `claim_id` is `clm_` and the first 16 bytes of BLAKE3(`clm_` ‖ `canonical_body()`). The
body is the `CLM1` frame, schema 2: metric key, value tag, evidence, epistemic status, knowledge state, label, parents,
anchors. `Claim` stores its value tag, so any holder can recompute the id; `check_id` refuses (`FMINE-REF-507`) a claim
whose hashed fields were written after construction, and `ClaimDag::insert` runs it. `downgraded(to)` is the
supersession path for evidence: the class becomes `meet(class, to)`, so it never rises, and a changed claim gets its
own id with `supersedes` naming the old one. Limits, stated plainly: representation, support, additivity, UU terms and
`supersedes` are not in the body, so claims differing only there share an id; `value` enters only through its tag,
so a write to `value` alone is not detected; the fields stay `pub`. Widening the body changes every claim id and the
public lane's state root, so it waits on an owner decision.

DESIGN (BD-046, INV-013): the dependency index retains invalidation facts, including identities whose bodies have
not arrived. Insertion computes staleness from these facts, the claim's stale status and known parents, and
propagates it to existing descendants. Replaying a body cannot make that identity current again or erase its
supersession. Inserting a distinct identity with `supersedes` invalidates the old identity and its dependants;
the replacement is not stale merely because it replaces an old version. Its own stale dependencies still apply.
A claim cannot supersede itself, and that refusal occurs before the graph changes. This is an in-memory dependency
index; it does not add a durable format or independently authenticate the unhashed `supersedes` field.

A claim carries `evidence`, `epistemic` and `state` separately. Its colour is computed by `Claim::color`, never set: no
constructor takes one and its basis is private (`compile_fail` doctest on `Claim`). `with_regime` names a claim's anchor,
which counts only for a class `project_color` lets stand on one, so it cannot raise a colour.

DESIGN (BD-344, INV-016): `dag::TemporalGraph` seals provenance metadata for observations, interpretations,
fitted parameters, correlation cards, exclusion acts, decks, registry rows, priors, Claims, forecasts, decisions
and recipes. Derived availability is max(all input availability, performed time), optionally delayed by an
explicit declaration; a declaration before that maximum refuses. Every ancestor needs availability and record
evidence. Admission uses the decision cut and the anchor's cell/sequence marks; wall clocks never order cells.
Iterative traversal checks only the root's ancestry, detects missing nodes and cycles, and finds frontier
violators with no violating ancestors. Each has a deterministic shortest root-to-node path. The cone digest
binds rules, root, cut, relevant anchor marks and all checked metadata; unrelated store entries do not change it.

DESIGN (BD-344): `dag::replay` freezes canonical object bytes under their content-derived IDs and runs the same
generator against full and guarded views. The guarded view checks a read's complete ancestry. Identical output
bytes, identical ordered read receipts and membership of every read in the declared root ancestry are required.
A denied read stays a failure even if the generator swallows its error. Certificates have private fields and
bind the checked cone, exact output and receipts. This is a pure in-memory replay seam, not an authority ledger.

## 3. Authority, Epistemics & Representation
A decision-facing claim whose status is not `KnownKnown` requires a non-empty `uu_terms` (`G-UU`, `FMINE-REF-204`). Every claim enforces `Confidentiality` isolation (`INV-007`, `FMINE-REF-301` and `FMINE-REF-302`).

## 4. Public API Surface
- `Claim<T>` (`with_regime`, `color`, `value_tag`, `canonical_body`, `check_id`, `downgraded`), `Representation`,
  `SpatialSupport`, `AdditivityClass`, `UuTerm`
- `Confidentiality` (`public`, `internal`, `venture`, `mnpi`, `join`, `leq`, `refusal_over`), `LabelRegistry`,
  `VentureId`, `WallId`, `LabelError`
- `compose_binary`, which takes the `IntervalOp` its colour composes under
- `ClaimDag` (`insert`, which refuses as `check_id` does; `guard_disclosure_export`), `DagNode`
- `dag::{TemporalGraph, TemporalNode, NodeKind, Availability, ConeCut, CheckedCone, ConeError, ConeViolation}`;
  `TemporalGraph::check` and `verify_claim` bind the actual Claim's ID, kind and immediate parent set.
- `dag::replay::{ReplayStore, ReplayView, ReplayCertificate, ReplayError, verify_replay}`; store insertion
  checks canonical IDs, and certificates expose immutable output and read receipts.

## 5. Guards & Typed Refusals
- `FMINE-REF-102` (support mismatch, `verify_support_compatible`, `G-GEO-1`), exit 3
- `FMINE-REF-103` (non-additive average, `verify_unweighted_additive`, `INV-014`), exit 3
- `FMINE-REF-202` (not observable, `ClaimDag::guard_disclosure_export`, INV-013), exit 3: the supplied claim or
  an ancestor is stale, including a recorded invalidation before the affected body arrives. The supplied claim
  may be uninserted, but its current body and all required parents must still be checked.
- `FMINE-REF-204` (unbudgeted unknowns, `verify_uu_slot_for_decision`, `INV-015`), exit 3
- `FMINE-REF-205` (calibration required, `Claim::color`), exit 3: failed calibration, or an `Assertion`, which §7.1
  refuses as a colour leaf until reviewed. §8.4 has no code for the second, so this is the nearest.
- `FMINE-REF-301` (MNPI wall, `verify_export_clearance`, `INV-007`), exit 7: the label holds a wall the reader is
  not cleared behind, or internal data would reach a public reader. §8.4 has no code for the second, so the MNPI
  wall is the nearest; `docs/adr/ADR-REFUSALS-01.md` asks the owner to confirm it.
- `FMINE-REF-302` (venture wall, `verify_export_clearance`, §31.1), exit 7: the reader's clearance covers every wall
  but not every venture, as when a result pooled from two ventures goes to one partner.
- `FMINE-REF-304` (`ClaimDag::guard_disclosure_export`, G-DISC's evidence clauses), exit 7: a class outside
  {Measured, Derived, ModelledCalibrated}, an `Assertion` or `Assumed` anywhere in the provenance, or a parent the
  DAG does not hold. §8.4 has no G-DISC code; 304 is the nearest, because the clause this cannot yet check is the
  attested record. Passing is necessary, not sufficient: G-DISC's RefClass and attestation clauses wait on fields
  `Claim` does not have. The owner should confirm the code with the others in `docs/adr/ADR-REFUSALS-01.md`.
- `FMINE-REF-507` (frozen record mutation, proposed, `Claim::check_id`, `ClaimDag::insert` and
  `ClaimDag::guard_disclosure_export`), exit 6: the claim's id is not the id of its current body. Insertion also
  refuses a self-supersession because a correction must have a distinct identity.
- `FMINE-REF-110` (temporal cone or replay, proposed under ADR-REFUSALS-01), exit 3: inadmissible or incomplete
  ancestry, backdated availability, a cycle, an undeclared read, a denied read, or differing replay outputs.
  Replay-store replacement or a payload whose content-derived ID differs refuses with `FMINE-REF-507`.

## 6. Proof Obligations (`PO-*`)
- `PO-CLAIM-1`: `compose_binary` keeps the placeholder (`None`), takes the `meet` of classes and the `and` of states
  and statuses, composes colours, and unions `uu_terms`. Over $10^5$ random claim trees no node outranks a parent in
  class, state or colour, and a node is refused exactly when a leaf under it is (`test_random_claim_dags_nl1_nl2_nl5_100k`).
- `PO-CLAIM-2`: `ClaimDag::invalidate_and_propagate` marks all transitive descendants stale (`INV-013`).
  FACT (BD-046): `test_claim_dag_invalidation_is_monotone_across_arrival_orders_and_replay` covers all six
  arrival orders of a three-node chain with invalidation at each of four positions, then replays every body.
  `test_claim_dag_late_stale_parent_invalidates_existing_descendants` covers late parent status and stale
  knowledge states. `test_claim_dag_supersession_invalidates_old_dependants_without_staling_new_version`
  covers evidence downgrades before and after the old body arrives.
  `test_claim_dag_rejects_self_supersession_without_mutation` checks atomic refusal.
  `test_g_disc_refuses_stale_claims_and_ancestors_before_root_insertion` checks stale export refusal, while
  `test_g_disc_accepts_measured_derived_chain` retains the fresh, uninserted-root positive path.
- `PO-CLAIM-3` (BD-040): two claims' canonical bytes and ids equal known answers built from the codec and BLAKE3
  specs by `scripts/oracle/claim_body_known_answers.py`, whose BLAKE3 port passes the official vectors first
  (`test_canonical_body_and_claim_id_golden`). A write to each hashed field is refused by `check_id` and by
  `ClaimDag::insert`, which then stores nothing (`test_edited_claim_is_refused_and_unhashed_fields_are_not`).
  `downgraded` never raises a class (`test_downgraded_supersedes_and_never_raises`).

## 7. Failure Modes & Deterministic Fallbacks
Adding `IntensiveMassWeighted` claims without mass weights or combining mismatched `SpatialSupport` fails closed with `FMINE-REF-103` / `FMINE-REF-102`.

## 8. Gates & Kill Criteria (`GATE-*` / `KILL-*`)
- `GATE-CLAIM-01`

## 9. Lifted Code & Foundation Dependencies
- Depends on `fmine-core`, `fmine-units`, `fmine-time`, `fmine-evidence`.

## 10. Non-Goals & Scope Boundary
Does not perform workbook I/O (`fmine-io`) or CLI formatting (`fmine-cli`).

Temporal metadata is supplied evidence, not an independently authenticated time or ledger proof. The graph
cannot prove that an engine declared every input; the replay seam checks only reads mediated by `ReplayView`.
Callers must rebuild outputs with the same seeds and avoid external captures, I/O and caches in generators.
Canonical-key verification binds exactly the canonical bytes supplied, including the existing Claim value-tag
limitation above. No durable L0 ledger, cross-host replay gate or complete retrospective DecisionCard is asserted
by this slice. Pure integer/digest operations introduce no floating-point time comparison.
