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
Ventures and walls come only from a `LabelRegistry`, which refuses malformed or repeated ids, so a label cannot name
one the deployment did not register. Distinct walls, and distinct ventures, are incomparable: there is no total
order, and `<` does not compile (`compile_fail` doctest). §31.1 writes the labels as a chain; the product refines it
so that two ventures, or two walls, never collapse into one tier. The owner should confirm the refinement.
`compose_binary` labels a result with the join of its parents' labels, so composing can only raise a label.

A label's claim-id bytes keep the old tier bytes, 0 for public and 1 for internal, so no existing claim id moves; any
other label is tag 5, then its ventures and walls.

A claim carries `evidence`, `epistemic` and `state` separately. Its colour is computed by `Claim::color`, never set: no
constructor takes one and its basis is private (`compile_fail` doctest on `Claim`). `with_regime` names a claim's anchor,
which counts only for a class `project_color` lets stand on one, so it cannot raise a colour.

## 3. Authority, Epistemics & Representation
A decision-facing claim whose status is not `KnownKnown` requires a non-empty `uu_terms` (`G-UU`, `FMINE-REF-204`). Every claim enforces `Confidentiality` isolation (`INV-007`, `FMINE-REF-301` and `FMINE-REF-302`).

## 4. Public API Surface
- `Claim<T>` (`with_regime`, `color`), `Representation`, `SpatialSupport`, `AdditivityClass`, `UuTerm`
- `Confidentiality` (`public`, `internal`, `venture`, `mnpi`, `join`, `leq`, `refusal_over`), `LabelRegistry`,
  `VentureId`, `WallId`, `LabelError`
- `compose_binary`, which takes the `IntervalOp` its colour composes under
- `ClaimDag` (`guard_disclosure_export`), `DagNode`

## 5. Guards & Typed Refusals
- `FMINE-REF-102` (support mismatch, `verify_support_compatible`, `G-GEO-1`), exit 3
- `FMINE-REF-103` (non-additive average, `verify_unweighted_additive`, `INV-014`), exit 3
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

## 6. Proof Obligations (`PO-*`)
- `PO-CLAIM-1`: `compose_binary` keeps the placeholder (`None`), takes the `meet` of classes and the `and` of states
  and statuses, composes colours, and unions `uu_terms`. Over $10^5$ random claim trees no node outranks a parent in
  class, state or colour, and a node is refused exactly when a leaf under it is (`test_random_claim_dags_nl1_nl2_nl5_100k`).
- `PO-CLAIM-2`: `ClaimDag::invalidate_and_propagate` marks all transitive descendants stale (`INV-013`).

## 7. Failure Modes & Deterministic Fallbacks
Adding `IntensiveMassWeighted` claims without mass weights or combining mismatched `SpatialSupport` fails closed with `FMINE-REF-103` / `FMINE-REF-102`.

## 8. Gates & Kill Criteria (`GATE-*` / `KILL-*`)
- `GATE-CLAIM-01`

## 9. Lifted Code & Foundation Dependencies
- Depends on `fmine-core`, `fmine-units`, `fmine-time`, `fmine-evidence`.

## 10. Non-Goals & Scope Boundary
Does not perform workbook I/O (`fmine-io`) or CLI formatting (`fmine-cli`).
