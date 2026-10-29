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
Every decision-facing claim requires a non-empty `uu_register: Vec<UuTerm>` (`G-UU`, `FMINE-REF-404`) and enforces `ConfidentialityTier` isolation (`INV-007`, `FMINE-REF-402`).

## 4. Public API Surface
- `Claim<T>`, `Representation`, `SpatialSupport`, `AdditivityClass`, `ConfidentialityTier`, `UuTerm`
- `compose_binary`
- `ClaimDag`, `DagNode`

## 5. Guards & Typed Refusals
- `FMINE-REF-109` (`IntensiveExtensiveAddError`, `INV-014`)
- `FMINE-REF-110` (`SupportMismatchError`, `G-GEO-1`)
- `FMINE-REF-401` (`UnanchoredClaimError`)
- `FMINE-REF-402` (`ConfidentialityCrossCellExport`, `INV-007`)
- `FMINE-REF-404` (`MissingUuRegisterError`, `INV-015`)

## 6. Proof Obligations (`PO-*`)
- `PO-CLAIM-1`: `NL-1..NL-8` composition preserves `Placeholder` (`None`), propagates weakest `EvidenceClass` and `KnowledgeState`, and unions `anchors` and `uu_register`.
- `PO-CLAIM-2`: `ClaimDag::invalidate_root` marks all transitive descendants `stale = true` (`INV-013`).

## 7. Failure Modes & Deterministic Fallbacks
Adding `IntensiveMassWeighted` claims without mass weights or combining mismatched `SpatialSupport` fails closed with `FMINE-REF-109` / `FMINE-REF-110`.

## 8. Gates & Kill Criteria (`GATE-*` / `KILL-*`)
- `GATE-CLAIM-01`

## 9. Lifted Code & Foundation Dependencies
- Depends on `fmine-core`, `fmine-units`, `fmine-time`, `fmine-evidence`.

## 10. Non-Goals & Scope Boundary
Does not perform workbook I/O (`fmine-io`) or CLI formatting (`fmine-cli`).
