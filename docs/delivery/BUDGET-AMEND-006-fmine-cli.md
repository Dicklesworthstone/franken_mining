# BUDGET-AMEND-006 for `fmine-cli`

- **Crate:** `fmine-cli`
- **Planned / ceiling now:** 40000 / 46000
- **Actual:** 43325 lines in the combined tree from `9f0d823` and `4dde74d` (`xtask loc`).
- **New planned:** 44000
- **New ceiling:** 51000
- **New scope:** The command layer exposes source-bound operating, investment and portfolio decisions, raw plant reconciliation, and supplied-forecast comparisons (BD-048, BD-177, BD-225, BD-229).
- **Gate served:** `GATE-LAYERS-00` and the two-parent integration tracked in `bd-hjtb`.
- **Alternatives:** The command builds complete receipts from user inputs. Moving source IO into numerical kernels would violate the layer boundary.
- **Approval:** approved by project owner, 2026-11-23
