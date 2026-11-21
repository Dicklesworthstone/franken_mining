# BUDGET-AMEND-005: `fmine-capital`

Drafted and approved on 2026-11-20 under the owner's explicit session grant of expert judgment on all reality-check
decisions (owner may revise).

- **Crate:** `fmine-capital`
- **Planned / ceiling before:** 20000 / 23000 (`registries/loc_budget.toml`)
- **Actual at revision:** 22995 before `rustfmt` (~23140 after `rustfmt`, `05ac300`)
- **New planned:** 25000
- **New ceiling:** 29000 (1.15 × 25000 = 28750, rounded to the nearest thousand per §28.3)
- **New scope:** shared-scenario capital portfolio selection, exact Wasserstein-1 DRO robustness, multi-stage recourse,
  multi-entity JV waterfalls, debt service/covenants/sweeps, capital calls, equity per-share valuation, and declared
  KPI constraint frontiers (`BD-229`, `BD-314`), plus `rustfmt` normalization so the crate no longer relies on
  unformatted single-line packing to stay under 23,000 LOC.
- **Gate served:** `BD-229`, `BD-314`, `rustfmt`, and `GATE-LAYERS-00`.
- **Alternatives:** leaving `fmine-capital` packed on unformatted one-line statements violates `cargo fmt --check`
  in CI.
- **Consequences:** `registries/loc_budget.toml` updates `fmine-capital` to `25000 / 29000` (+5,000 planned, +6,000
  ceiling); combined with `BUDGET-AMEND-001`–`004`, workspace totals move to `1,171,000 / 1,347,000`.
- **Approval:** approved by owner (expert-judgment grant 2026-11-20; owner may revise), 2026-11-20
