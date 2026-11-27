# DECISION-bd-lf4q: confidentiality labels form a product lattice

- **Date:** 2026-11-27.
- **Decided by:** the owner, who approved this refinement on 2026-11-27.
- **Bead:** `bd-lf4q` (plan §31.1–31.2, INV-007). Implemented in `fc0e0566` (`crates/fmine-claim/src/label.rs`).

## Context

Plan §31.1 writes the label order as one chain, `Public < Internal < Venture(V) < Mnpi(Wall)`, and also says that
two different ventures, or two different walls, are incomparable. Those two statements cannot both hold in a single
chain. If every venture label sat below every wall label, data joined from venture A and wall W would carry W only,
and a reader cleared for W would see venture A's data.

## Decision

A label is a product: `(internal: bool, ventures: set of VentureId, walls: set of WallId)`, where a non-empty
venture or wall set implies `internal`.

- `Public = (false, ∅, ∅)`, `Internal = (true, ∅, ∅)`, `Venture(V) = (true, {V}, ∅)`, `Mnpi(W) = (true, ∅, {W})`.
- Join is componentwise (or, union, union). `a ≤ b` is componentwise (implies, subset, subset).
- `Public < Internal < Venture(V)` and `Internal < Mnpi(W)` hold. Two different ventures, two different walls, and
  a venture and a wall are incomparable.
- Data joined from walls A and B carry `{A, B}`, and only a reader cleared for both may read them. A wall never
  leaves a label through a join, so MNPI-tainted data stay MNPI.
- A reader's clearance is a label of the same type, and reading is allowed iff `label ≤ clearance`. A refused
  export is `FMINE-REF-402` and names the label and the clearance.
- The old `JvPartner`, `SovereignRestricted` and `BoardEyesOnly` tiers are mapped explicitly or refused, never
  silently (`fc0e0566`).

The plan's chain is read as describing the order within one venture and one wall. The product form keeps it there
and makes the incomparability the plan asks for exact.

## Evidence

- `fmine-claim` unit tests at `7ca05cd5`, 73 passed and 4 passed in the doc tests, including
  `label::tests::test_join_semilattice_laws_exhaustive`, `label::tests::test_distinct_walls_and_ventures_are_incomparable`
  and `tests::test_two_venture_composition_refuses_a_single_venture_reader`.

## Not decided here

- `Claim.confidentiality` is still a public field, so a caller can lower a label after a join. Making it private,
  with a raising setter that recomputes the claim id, belongs to `bd-043` (INV-007, monotone taint).
- What wall and venture ids name, and who registers them, is plan §31.2's and `bd-093`'s.
