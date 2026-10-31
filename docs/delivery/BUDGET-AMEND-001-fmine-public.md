# BUDGET-AMEND-001: `fmine-public`

Drafted by an agent (SwiftBasin, 2026-10-31) for the owner's decision. The number and scope below are a proposal.
The record covers nothing until the owner replaces the Approval line.

- **Crate:** `fmine-public`
- **Planned / ceiling now:** 18000 / 21000 (`registries/loc_budget.toml`)
- **Actual:** 20977 (`cargo run -p xtask -- loc` at `b2c4ef1`): over planned since before this record, with 23 lines
  left to the ceiling.
- **New planned:** 22000
- **New ceiling:** 25000 (1.15 × 22000, to the nearest thousand, as §28.3 rounds)
- **New scope:** the `fmine check tables` binding that `bd-4wvl.8` (BD-340.S8) names,
  `crates/fmine-public/src/identities.rs::check_tables`. It joins the exact-feasibility solver to the registered
  tables. Other open public-lane beads (quarterly vintages, vintage and basis keys, guidance track record) would
  need their own records.
- **Gate served:** BD-340's done-when through S8: per-instance verdicts on the registered files equal to BD-331.S6's
  states (GATE-BULL-01).
- **Alternatives:** the binding needs `LoadedSource` from fmine-public, so it can live only there or in a crate that
  depends on it. The one such crate with room is `fmine-cli` (layer U), which would put table logic in the CLI. The
  other way is to make room: shrink fmine-public first by refactoring with net LOC ≤ 0.
- **Consequences if approved:** the fmine-public row in `registries/loc_budget.toml` becomes 22000 / 25000. The plan's
  §28.3 row and totals change the same way, from 1,128,000 / 1,298,000 to 1,132,000 / 1,302,000, so that
  `check_loc_budget` still holds.
- **Approval:** PENDING
