# CONTRACT.md — `fmine-evidence`

## 1. Charter & Layer
- **Crate:** `fmine-evidence`
- **Layer:** `S0` (Substrate)
- **Subsystem:** `Bedrock`
- **LOC Budget:** `12k planned / 14k ceiling` (`registries/loc_budget.toml`)
- **Allowed Module Families:** `["color", "epistemic", "lattice", "refusal_catalog"]`

## 2. Mathematical & Domain Formulation
Defines plan §7.1's eight `EvidenceClass` variants under the NL-2 partial order, with `meet` as the greatest lower
bound; §7.2's nine `KnowledgeState` variants with the NL-5 conjunction `and`; §8.1's `EpistemicStatus`; the lifted
`Color` with `compose` and `check_regime`; `project_color`, the last column of the §7.1 table; and the 32-code refusal
catalogue (`FMINE-REF-*`).

`Measured > Derived > ModelledCalibrated > {ModelledUncalibrated, Transferred, Elicited} > Assumed > Assertion`.
The three middle classes are incomparable, so the type has no `Ord`.

`src/refusal_catalog.rs` is generated from `registries/refusals.toml` by `cargo run -p xtask -- gen-refusals` and
never edited by hand; `xtask check-refusals` fails when it differs from the regenerated text (BD-328).

## 3. Authority, Epistemics & Representation
Owns the monotone epistemic laws and the canonical `Refusal` type (`INV-006`). `meet`, `and` and `compose` never
raise anything (NL-1, NL-2, NL-5). A class says how something is known and a state what may be said about it now;
neither says who stands behind it (NL-3), so an issuer's figure is `Assertion { source: Issuer }` whatever its state.

## 4. Public API Surface
- `epistemic::{EvidenceClass, AoaStatus, AssertionSource, KnowledgeState, EpistemicStatus, KuKind, HypothesisSetId}`
- `color::{Color, ColorRank, ColorPayloadError, ValidityDomain, IntervalOp, Demotion}`, with `compose`,
  `check_regime`, `intersect_domains`, `regime_demotion` and `validate_color_payload`.
- `lattice::project_color(class, basis) -> Option<Color>`: `None` for an `Assertion`, `Estimated` for every class
  §7.1 does not let stand on an anchor, whatever `basis` says.
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
- `PO-EVID-1`: The plan's tables, written out in the tests independently of the code: the order and `meet` over all 64
  class pairs, `and` over all 81 state pairs. Commutativity, associativity and idempotence are exhaustive over every
  class with every payload. `meet` is the greatest lower bound, so it is not a distributive lattice: the three
  middle classes have no join.
- `PO-EVID-2`: NL-2 and NL-5 hold over $10^5$ random provenance DAGs (`test_random_dags_nl2_nl5_100k`).
- `PO-EVID-3`: `compose` never returns a colour stronger than its weakest input (`test_compose_never_promotes`).

## 7. Failure Modes & Deterministic Fallbacks
There is no unknown code: a refusal is built only from a `RefusalCode`, and `RefusalCode::parse` returns `None`
for any string outside the catalogue. Nothing defaults.

Open for the owner, each with the working choice in force until answered:
- **Plan erratum.** NL-2 says "the weakest input" but `Transferred` and `Elicited` are incomparable, so there may be
  no weakest. `meet` gives `Assumed`, their lower bound: it never promotes, and it demotes two mid-tier inputs.
- **`AoaStatus`.** The plan names `Transferred{aoa}` without defining `aoa`. Working values: `Outside`, `NotAssessed`,
  `Inside`, weakest first.
- **Assertion sources.** When two sources meet, the alphabetically first survives. Arbitrary but total; both stay
  reachable through provenance.
- **NL-5's order.** The plan gives the states and the conjunction's direction, not a total precedence. Working
  order, weakest knowledge last: `NotApplicable`, `Known`, `Estimated`, `Stale`, `Unknown`, `Indeterminate`,
  `Conflicted`, `NotObservable`, `Redacted`.
- **Structural hypothesis sets.** Two different `Structural` sets conjoin to one of them.

## 8. Gates & Kill Criteria (`GATE-*` / `KILL-*`)
- `GATE-CLAIM-01`

## 9. Lifted Code & Foundation Dependencies
`src/color.rs` is lifted from frankensim `crates/fs-evidence/src/color.rs`; its first line states the changes and what
was left out, and its identity digests differ from upstream's. `src/lattice.rs` is original, listed as an
`[[analogue]]` row in `registries/lifts.toml`.

## 10. Non-Goals & Scope Boundary
Does not store claim payloads or provenance DAGs (`fmine-claim`).
