# ADR-REFUSALS-01: Proposed Refusal Codes, Two Exit-Code Exceptions, and Call-Site Choices

- **Status:** Proposed (2026-10-30, `BD-328.S1`, `bd-h0q1.1`, `§8.4`, `§30.1`). It awaits the owner's decision.
  The decision id may be `decision:ADR-REFUSALS-01` or a CELL-DEC entry, as the owner prefers.
- **Context:** Before BD-328, only 4 of the 25 §8.4 codes (102, 103, 108, 504) meant the same thing in the plan,
  `registries/refusals.toml` and the hand-written catalogue (R7-C080). For example, the Q2 2026 Mine Stats
  workbook has 1,092 formula-error cells (E820). A claim built from one refused with `FMINE-REF-401`, exit 6.
  §8.4 defines 401 as "unknown reactive ground → no ANFO plan". Plan codes 204, 205 and 503 were missing from the
  registry. Unknown codes fell back to `FMINE-REF-000`.
- **Done so far (BD-328.S2, S3; BD-344):** the registry is the single source (schema 2, 33 rows).
  `xtask gen-refusals` compiles it into a closed `RefusalCode` enum. `Refusal::new` takes only that enum, so an
  unregistered code does not compile. `xtask check-refusals` fails on drift between plan §8.4, registry, generated
  catalogue and every `FMINE-REF-nnn` token in code and contracts. The 25 plan codes are `active`. The eight
  below are `proposed`, and the check warns on each until §8.4 lists it. Call sites already use them. This runs
  ahead of the owner's decision (S3 asked for the final numbers), and the fallback below undoes it.
- **Decision requested:**
  1. **Add eight codes to §8.4.** None renumbers an existing code (§0.3).

     | Code | Meaning | Exit | Used by today |
     |---|---|---|---|
     | 109 | value outside its physical domain (refuse, never clamp) | 3 | fractions and moisture, LMDI states, ledger values |
     | 110 | complete temporal ancestry or guarded replay cannot establish the decision cut (INV-016, BD-344) | 3 | temporal graph, replay, retrospective NPV evidence and optional forecast provenance checks |
     | 206 | forecast registered after freeze or resolution (INV-008) | 3 | `ForecastLedger::register`, `record_outcome` |
     | 207 | numerical certificate failed (non-SPD, residual; §29.6) | 3 | Cholesky |
     | 208 | source cell unusable (formula error or text for a number) | 3 | `cell_to_public_claim` |
     | 505 | artifact digest or signature mismatch | 8 | an altered registered workbook or document |
     | 506 | registered input missing or unreadable, naming every file | 5 | missing or unreadable files, bad registries |
     | 507 | frozen-record mutation attempted | 6 | a second registration or outcome in the ledger |

  2. **§30.1 exceptions.** 504 (uncured backfill) keeps its number and exits 7: it is a safety refusal in the 5xx
     family. 506 exits 5 (foundation I/O). The family rule then gives 501 and 503 exit 8. The owner may prefer 7
     for 501 (FA's refusal relayed) and 4 for 503 (sibling unavailable, fallback engaged). Either is a one-row
     registry edit.
  3. **Not refusals.** LP infeasibility goes to the three-gap ledger (§29.6). Usage errors stay at exit 2 with no
     code (`test_cli_usage_errors`).
  4. **Call-site choices to confirm.** Each is the nearest plan meaning. The alternative is in brackets.
     - `cholesky_solve`'s dimension check → 207. It is a caller error, but no code fits better [a new 1xx row].
     - `claim show` without a claim store → 201 [503].
     - `doctor` when the registries cannot load → 506.
     - `through` bound leaving no rows → 201.
     - two readers disagreeing on a token → 202: the evidence conflicts, so the value is not observable.
     - unregistered Mine Stats quarter, missing or malformed registry → 506.
     - internal data exported to a public reader → 301, the MNPI wall [a new 3xx row]. A missing wall is 301 and
       a missing venture 302, as §31 has them (bd-lf4q), so only the internal case is a choice.
     - an outcome for an unknown forecast id → 201.
  5. **Scope of the check.** `check-refusals` reads §8.4 only. §25.6 and §31.3 also name codes and are not checked.
- **Left open for their own beads:** G-DISC (plan line 751) has no code (`bd-306n.3`). NEW-fa-sap needs a 1xx
  trunk-post code (`bd-l7mc.1`) and a 2xx certification refusal. BD-357 needs a 3xx salted-packet code
  (`bd-kftl.2`).
- **Fallback if declined:** each declined row returns its call sites to the nearest active code, with a precise
  `what` and the mismatch recorded under BD-327's Known gaps. No code is ever invented outside the registry.
- **Consequences if accepted:** set the approved rows to `status = "active"` in `registries/refusals.toml`. Add
  them to §8.4, with the exceptions in §30.1, so the check's warnings stop. Whoever the owner names applies the
  plan edits.
