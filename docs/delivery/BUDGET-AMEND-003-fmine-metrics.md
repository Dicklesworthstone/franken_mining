# BUDGET-AMEND-003: `fmine-metrics`

Drafted and approved on 2026-11-20 under the owner's explicit session grant of expert judgment on all reality-check
decisions (owner may revise).

- **Crate:** `fmine-metrics`
- **Planned / ceiling before:** 18000 / 21000 (`registries/loc_budget.toml`)
- **Actual at revision:** 26004 (`cargo run -p xtask -- loc` at `05ac300`)
- **New planned:** 28000
- **New ceiling:** 32000 (1.15 × 28000 = 32200, rounded to the nearest thousand per §28.3)
- **New scope:** exact rational AISC/AIC/TCC/C1 gold & copper calculators, fitted cost-driver models (`BD-061`),
  published cut-off reproduction (`BD-274`), GEO decks (`BD-063`), reserve-replacement bridge (`BD-064`), LMDI
  waterfalls (`BD-333`), guidance completion (`BD-342`), multi-period production valuation, and eliminating the
  remaining L-6 bare-`f64` debt signatures (`bd-u56b`).
- **Gate served:** `GATE-BULL-01`, `PO-UNIT-2` (`bd-u56b`), and `GATE-LAYERS-00`.
- **Alternatives:** moving metric kernels into `fmine-public` or `fmine-cli` violates layer boundaries (`E` → `V`/`U`).
- **Consequences:** `registries/loc_budget.toml` updates `fmine-metrics` to `28000 / 32000` (+10,000 planned, +11,000
  ceiling); combined with `BUDGET-AMEND-001`–`005`, workspace totals move to `1,171,000 / 1,347,000`.
- **Approval:** approved by owner (expert-judgment grant 2026-11-20; owner may revise), 2026-11-20
