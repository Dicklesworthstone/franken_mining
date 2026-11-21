# BUDGET-AMEND-004: `fmine-calib`

Drafted and approved on 2026-11-20 under the owner's explicit session grant of expert judgment on all reality-check
decisions (owner may revise).

- **Crate:** `fmine-calib`
- **Planned / ceiling before:** 18000 / 21000 (`registries/loc_budget.toml`)
- **Actual at revision:** 24751 (`cargo run -p xtask -- loc` at `05ac300`)
- **New planned:** 26000
- **New ceiling:** 30000 (1.15 × 26000 = 29900, rounded to the nearest thousand per §28.3)
- **New scope:** exact rational proper scoring (log, CRPS, Winkler, paired CRPS), Clopper-Pearson certified intervals,
  missing-outcome completion bounds, e-process and CUSUM surprise monitors (`BD-051`), guidance cluster intervals
  (`BD-341`), empirical/additive forecast corrections, and forecast record/journal ledger binding (`BD-047`–`BD-049`,
  `BD-067`), plus `rustfmt` normalization.
- **Gate served:** `GATE-BULL-01`, `BD-047`–`BD-051`, `BD-067`/`BD-068`, and `GATE-LAYERS-00`.
- **Alternatives:** splitting calibration math into an unlisted crate is forbidden without crate-map approval.
- **Consequences:** `registries/loc_budget.toml` updates `fmine-calib` to `26000 / 30000` (+8,000 planned, +9,000
  ceiling); combined with `BUDGET-AMEND-001`–`005`, workspace totals move to `1,171,000 / 1,347,000`.
- **Approval:** approved by owner (expert-judgment grant 2026-11-20; owner may revise), 2026-11-20
