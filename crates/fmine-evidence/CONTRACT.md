# CONTRACT.md — `fmine-evidence`

## 1. Charter & Layer
- **Crate:** `fmine-evidence`
- **Layer:** `S0` (Substrate)
- **Subsystem:** `Bedrock`
- **LOC Budget:** `12k planned / 14k ceiling` (`registries/loc_budget.toml`)
- **Allowed Module Families:** `["lattice", "refusal"]`

## 2. Mathematical & Domain Formulation
Defines the 8-element `EvidenceClass` bounded distributive lattice (`Exact` down to `Unverified`), the 4-element `KnowledgeState` Rumsfeld lattice (`KnownKnown`, `KnownUnknown`, `UnknownKnown`, `UnknownUnknown`), `ClaimColor`, `project_color`, and the 22-code `REFUSAL_CATALOG` (`FMINE-REF-*`).

## 3. Authority, Epistemics & Representation
Owns the monotone epistemic join laws (`join` = weakest link) and the canonical `Refusal` type (`INV-006`).

## 4. Public API Surface
- `lattice::{EvidenceClass, KnowledgeState, ClaimColor, project_color}`
- `Refusal`, `RefusalSpec`, `REFUSAL_CATALOG`, `lookup_refusal_spec`

## 5. Guards & Typed Refusals
- Defines all 22 `FMINE-REF-*` refusal codes (`101..503`) verified against `registries/refusals.toml`.

## 6. Proof Obligations (`PO-*`)
- `PO-EVID-1`: Exhaustive finite-domain verification of idempotence, commutativity, associativity, absorption, and distributivity over all $8^3 = 512$ `EvidenceClass` triples and $4^3 = 64$ `KnowledgeState` triples.

## 7. Failure Modes & Deterministic Fallbacks
Unknown refusal codes default to exit code `50` with explicit diagnostic metadata.

## 8. Gates & Kill Criteria (`GATE-*` / `KILL-*`)
- `GATE-CLAIM-01`

## 9. Lifted Code & Foundation Dependencies
- `crates/fs-evidence/src/lattice.rs` @ `20463525d9775a18c1b237d2676f364f367175ab` -> `src/lattice.rs`

## 10. Non-Goals & Scope Boundary
Does not store claim payloads or provenance DAGs (`fmine-claim`).
