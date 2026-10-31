# CONTRACT.md — `fmine-evidence`

## 1. Charter & Layer
- **Crate:** `fmine-evidence`
- **Layer:** `S0` (Substrate)
- **Subsystem:** `Bedrock`
- **LOC Budget:** `12k planned / 14k ceiling` (`registries/loc_budget.toml`)
- **Allowed Module Families:** `["lattice", "refusal_catalog"]`

## 2. Mathematical & Domain Formulation
Defines the 8-element `EvidenceClass` bounded distributive lattice (`Exact` down to `Unverified`), the 4-element `KnowledgeState` Rumsfeld lattice (`KnownKnown`, `KnownUnknown`, `UnknownKnown`, `UnknownUnknown`), `ClaimColor`, `project_color`, and the 32-code refusal catalogue (`FMINE-REF-*`).

`src/refusal_catalog.rs` is generated from `registries/refusals.toml` by `cargo run -p xtask -- gen-refusals` and
never edited by hand; `xtask check-refusals` fails when it differs from the regenerated text (BD-328).

## 3. Authority, Epistemics & Representation
Owns the monotone epistemic join laws (`join` = weakest link) and the canonical `Refusal` type (`INV-006`).

## 4. Public API Surface
- `lattice::{EvidenceClass, KnowledgeState, ClaimColor, project_color}`
- `RefusalCode`: one variant per registry row (`NoClaimInDomain201`, …), with `ALL`, `spec()`, `as_str()`,
  `parse(&str) -> Option<Self>` and `Display`.
- `CATALOG: [RefusalSpec; 32]`, in code order; `RefusalSpec { code, key, exit_code, status, summary, fix }` and
  `RefusalStatus { Active, Proposed, Reserved }`.
- `Refusal::new(code: RefusalCode, detail)`: `code`, `exit_code` and `fix` come from the catalogue row, and `what`
  is the row's summary followed by the detail. `Refusal` is `#[non_exhaustive]`, so a refusal outside the
  catalogue does not compile (doctests `compile_fail`).

## 5. Guards & Typed Refusals
- Defines all 32 `FMINE-REF-*` codes (`101..507`): the 25 that plan §8.4 lists (`active`), and 7 `proposed` codes
  (109, 206, 207, 208, 505, 506, 507) awaiting the owner (`docs/adr/ADR-REFUSALS-01.md`).
- Exit codes follow the §30.1 family rule (1xx/2xx 3, 3xx/4xx 7, 5xx 8, stale anchor 6) with two listed
  exceptions: 504 is 7 (a safety refusal) and 506 is 5 (foundation I/O).

## 6. Proof Obligations (`PO-*`)
- `PO-EVID-1`: Exhaustive finite-domain verification of idempotence, commutativity, associativity, absorption, and distributivity over all $8^3 = 512$ `EvidenceClass` triples and $4^3 = 64$ `KnowledgeState` triples.

## 7. Failure Modes & Deterministic Fallbacks
There is no unknown code: a refusal is built only from a `RefusalCode`, and `RefusalCode::parse` returns `None`
for any string outside the catalogue. Nothing defaults.

## 8. Gates & Kill Criteria (`GATE-*` / `KILL-*`)
- `GATE-CLAIM-01`

## 9. Lifted Code & Foundation Dependencies
- `crates/fs-evidence/src/lattice.rs` @ `20463525d9775a18c1b237d2676f364f367175ab` -> `src/lattice.rs`

## 10. Non-Goals & Scope Boundary
Does not store claim payloads or provenance DAGs (`fmine-claim`).
