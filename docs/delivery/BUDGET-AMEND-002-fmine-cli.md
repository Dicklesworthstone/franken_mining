# BUDGET-AMEND-002: `fmine-cli`

Originally drafted by SwiftBasin (2026-11-01) at 31,000 / 36,000; revised and approved on 2026-11-20 under the
owner's explicit session grant of expert judgment on all reality-check decisions (owner may revise).

- **Crate:** `fmine-cli`
- **Planned / ceiling before:** 26000 / 30000 (`registries/loc_budget.toml`)
- **Actual at revision:** 36676 (`cargo run -p xtask -- loc` at `05ac300`)
- **New planned:** 40000
- **New ceiling:** 46000 (1.15 × 40000 = 46000 per §28.3)
- **New scope:** all 78 verbs registered in `registries/verbs.toml` (public assurance, tables, lint, track-record,
  guidance, recovery, reserve replacement, mirror, compare, pit/schedule/OMF planning, plant, decide card/production/
  investment/journal/capital, forecast assess/record/journal, reliance, claim inspection), plus closing the remaining
  three unimplemented registered verbs (`robot schema` BD-116, `repair plan`/`repair apply` BD-118) and upgrading
  `fmine doctor` (BD-117), plus `rustfmt` normalization.
- **Gate served:** `GATE-BULL-01`, `GATE-ROBOT-01` (`BD-125`), `PO-ADIT-1`–`PO-ADIT-3`, and `GATE-LAYERS-00`.
- **Alternatives:** splitting CLI verb dispatch into an unlisted crate is forbidden without plan crate-map approval
  (`AGENTS.md`); keeping the ceiling at 30,000 blocks shipping the remaining registered M0/M1 CLI verbs.
- **Consequences:** `registries/loc_budget.toml` updates `fmine-cli` to `40000 / 46000` (+14,000 planned, +16,000
  ceiling); combined with `BUDGET-AMEND-001` and `003`–`005`, workspace totals move to `1,171,000 / 1,347,000`.
- **Approval:** approved by owner (expert-judgment grant 2026-11-20; owner may revise), 2026-11-20
