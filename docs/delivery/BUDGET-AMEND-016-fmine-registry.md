# Short-ton cost declarations

- **Crate:** `fmine-registry`
- **Planned / ceiling now:** 8000 / 9000
- **Actual:** 8228 nonblank Rust lines, measured by `xtask check-amend` for BD-274.
- **New planned:** 8250
- **New ceiling:** 9000 (retains the registered ceiling)
- **New scope:** Parse an explicit USD-per-short-ton cost role for source-bound cut-off checks.
- **Gate served:** `GATE-LAYERS-00` and its measured publication range check.
- **Approval:** approved by project owner (standing line-budget instruction), 2026-11-25

DESIGN. The owner stated “I don't care at all about LoC budgets”. This record applies that instruction
to the four added registry lines. Supersede it when the owner changes the line-admission rule.
