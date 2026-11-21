# BUDGET-AMEND-001: `fmine-public`

Originally drafted by SwiftBasin (2026-10-31) at 22,000 / 25,000; revised and approved on 2026-11-20 under the
owner's explicit session grant of expert judgment on all reality-check decisions (owner may revise).

- **Crate:** `fmine-public`
- **Planned / ceiling before:** 18000 / 21000 (`registries/loc_budget.toml`)
- **Actual at revision:** 22615 (`cargo run -p xtask -- loc` at `05ac300`)
- **New planned:** 24000
- **New ceiling:** 28000 (1.15 × 24000 = 27600, rounded to the nearest thousand per §28.3)
- **New scope:** full public-lane assurance spine shipped through `05ac300`: source-bound citation validation
  (`bd-sjrh.2`), cross-vintage and Mine Stats identity consistency, public LMDI waterfalls (`BD-333`), guidance
  completion and cluster track-record binders (`BD-341`, `BD-342`), copper/gold recovery and reserve-replacement
  adapters, HTML table projection, JV mirror, and disclosure lint, plus `rustfmt` normalization.
- **Gate served:** `GATE-BULL-01` (M0 public mirror) and `GATE-LAYERS-00` (`xtask check-amend`).
- **Alternatives:** moving public-lane parsers into `fmine-cli` violates layer separation (`V` → `U`); stripping
  unit tests into `xtask/testdata/` solely to hide LOC is prohibited ceremony.
- **Consequences:** `registries/loc_budget.toml` updates `fmine-public` to `24000 / 28000` (+6,000 planned, +7,000
  ceiling); combined with `BUDGET-AMEND-002` through `005`, workspace totals move from `1,128,000 / 1,298,000` to
  `1,171,000 / 1,347,000`.
- **Approval:** approved by owner (expert-judgment grant 2026-11-20; owner may revise), 2026-11-20
