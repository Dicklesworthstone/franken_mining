# BUDGET-AMEND-002: `fmine-cli`

Drafted by an agent (SwiftBasin, 2026-11-01) for the owner's decision, under the owner's grant to use expert
judgment. The number and scope below are a proposal. The record covers nothing until the owner replaces the
Approval line.

- **Crate:** `fmine-cli`
- **Planned / ceiling now:** 26000 / 30000 (`registries/loc_budget.toml`)
- **Actual:** 27215 (`cargo run -p xtask -- loc` at `403ef11`). origin/main is over planned too (27200 at
  `0894761`), so the next change on either that adds a line to the crate fails `check-amend --base` in CI.
- **New planned:** 31000
- **New ceiling:** 36000 (1.15 × 31000 = 35650, to the nearest thousand, as §28.3 rounds)
- **New scope:** the verbs that already took the crate past 26000, without a record: `check tables` (BD-340.S8,
  `27ffdc7`), the guidance cohort comparisons (BD-341, `9f296ce`, `e299b5b`), `lint` on registered disclosures
  (BD-345.S4, `31e20bf`) and the JV mirrors (BD-346, `0894761`). Then about 3,800 lines for the verbs still open:
  `lint` (bd-vnik.6), `track-record` (bd-bi9f.6), and a pit verb over the certified ultimate-pit kernel in
  `fmine-optimize` (TODO E8 in this session, parked until this record exists; OPEN-PUSH-1 is a separate owner
  decision).
- **Gate served:** the done-when of each bead above that names a verb, and GATE-PUSH-PIT1's kernel reaching a user
  through the binary.
- **Alternatives:** shrink the crate first with net LOC ≤ 0. `src/lib.rs` holds argument parsing, envelopes and
  several verbs' logic, so some logic could move to the library crates it calls. That is a refactor of the WP-03
  lane's busiest file while other agents are working in it. Note for BUDGET-AMEND-001: the `check tables` binding
  landed here, not in `fmine-public` as that record proposed, so 001's stated scope no longer applies. fmine-public
  is still over its planned 18000 (20487), so a change that adds to it still needs a record.
- **Consequences if approved:** the fmine-cli row in `registries/loc_budget.toml` becomes 31000 / 36000. The plan's
  §28.3 row and totals change the same way, from 1,128,000 / 1,298,000 to 1,133,000 / 1,304,000, so that
  `check_loc_budget` still holds. If 001 is also approved, the two changes add.
- **Approval:** PENDING
