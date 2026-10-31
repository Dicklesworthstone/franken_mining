# CONTRACT.md — `fmine-claim`

## 1. Charter & Layer
- **Crate:** `fmine-claim`
- **Layer:** `S0` (Substrate)
- **Subsystem:** `Bedrock`
- **LOC Budget:** `14k planned / 16k ceiling` (`registries/loc_budget.toml`)
- **Allowed Module Families:** `["claim", "dag", "uu"]`

## 2. Mathematical & Domain Formulation
Implements `Claim<T>`, `Representation`, `SpatialSupport`, `AdditivityClass`, `ConfidentialityTier`, `UuTerm` (`INV-015`), non-lossy binary composition (`compose_binary`, `NL-1..NL-8`), and `ClaimDag` transitive staleness propagation (`INV-013`).

## 3. Authority, Epistemics & Representation
Every decision-facing claim requires a non-empty `uu_register: Vec<UuTerm>` (`G-UU`, `FMINE-REF-204`) and enforces `ConfidentialityTier` isolation (`INV-007`, `FMINE-REF-301`).

## 4. Public API Surface
- `Claim<T>`, `Representation`, `SpatialSupport`, `AdditivityClass`, `ConfidentialityTier`, `UuTerm`
- `compose_binary`
- `ClaimDag`, `DagNode`

## 5. Guards & Typed Refusals
- `FMINE-REF-102` (support mismatch, `verify_support_compatible`, `G-GEO-1`), exit 3
- `FMINE-REF-103` (non-additive average, `verify_unweighted_additive`, `INV-014`), exit 3
- `FMINE-REF-204` (unbudgeted unknowns, `verify_uu_slot_for_decision`, `INV-015`), exit 3
- `FMINE-REF-205` (calibration required, a `Red` claim in decision-grade use), exit 3
- `FMINE-REF-301` (`verify_export_clearance`, `INV-007`), exit 7. §8.4 has no confidentiality-tier code, so the
  MNPI wall is the nearest; `docs/adr/ADR-REFUSALS-01.md` asks the owner to confirm it.

## 6. Proof Obligations (`PO-*`)
- `PO-CLAIM-1`: `NL-1..NL-8` composition preserves `Placeholder` (`None`), propagates weakest `EvidenceClass` and `KnowledgeState`, and unions `anchors` and `uu_register`.
- `PO-CLAIM-2`: `ClaimDag::invalidate_root` marks all transitive descendants `stale = true` (`INV-013`).

## 7. Failure Modes & Deterministic Fallbacks
Adding `IntensiveMassWeighted` claims without mass weights or combining mismatched `SpatialSupport` fails closed with `FMINE-REF-103` / `FMINE-REF-102`.

## 8. Gates & Kill Criteria (`GATE-*` / `KILL-*`)
- `GATE-CLAIM-01`

## 9. Lifted Code & Foundation Dependencies
- Depends on `fmine-core`, `fmine-units`, `fmine-time`, `fmine-evidence`.

## 10. Non-Goals & Scope Boundary
Does not perform workbook I/O (`fmine-io`) or CLI formatting (`fmine-cli`).
