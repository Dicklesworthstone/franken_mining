# Source-bound spatial assessment within the existing calibration ceiling

- **Crate:** `fmine-calib`
- **Planned / ceiling now:** 27000 / 31000 (`registries/loc_budget.toml`)
- **Actual:** 28373 nonblank Rust lines with the spatial adapter and its direct tests.
- **New planned:** 30750
- **New ceiling:** 31000
- **New scope:** The source-bound spatial-front worksheet adapter, conditional diagnostics, sticky alarm receipts and direct known-answer/admission tests for BD-051 and BD-311. The existing numerical monitor is reused without changing its goldens.
- Includes the append-only forecast investigation, human resolution and prospective rearm records with replay tests.
- Includes the dual surprise monitor for the synthetic control-mine gate (BD-051).
- Includes standardized residual updates and direct scale and dispersion tests (7ea543e7).
- **Gate served:** `GATE-LAYERS-00`; the new `forecast spatial-monitor` command exposes the existing calibration kernel with explicit units, support, chronology and Claim ancestry.
- **Alternatives:** Moving the numerical adapter into the CLI would split the pure calibration API across layers; removing evidence bindings or independent tests would weaken the delivered feature. This proposal does not raise the existing hard ceiling or change the budget registry.
- **Approval:** approved by project owner, 2026-11-24
- **Authorization:** The project owner explicitly instructed in this session: "And I don't care about any LoC budgets!!" This waiver authorizes the feature without a budget-based pause.
