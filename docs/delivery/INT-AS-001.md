# INT-AS-001: `asupersync` (`=0.5.0`) & Decision/Evidence Adapter Admission Record

- **Bead:** `bd-257` (`WP-00`)
- **Date:** 2026-10-29
- **Foundation Row:** `FD-AS-01` (`franken-stack.lock`)
- **Verified Facts (`E653–E655`, Suite-Fit Audit H.3):**
  1. `crates.io` newest published `asupersync` version is `0.5.0` (tag `v0.5.0`, commit `78b64636e`), matching `frankengraphdb`, `franken_accounting`, `frankensqlite`, `frankensearch`, `fastmcp =0.13.0`, and `frankentui =0.9.0`.
  2. `franken-evidence` and `franken-decision` at `v0.5.0` expose `EvidenceLedger`, `LossMatrix`, `DecisionContract`, `DecisionAuditEntry`, and `render` (`OPEN-SUITE-1` closed). Because they depend on `serde`, they are confined exclusively to `fmine-decide` (`L-4`); `S0` crates (`fmine-evidence`, `fmine-claim`) own `std`-only schema-compatible types.
  3. Defect `S3(a)` (`Cx<cap::None>::open_child_region` escalation) is mitigated at compile/CI time by `xtask check-caps` banning `open_child_region` outside `fmine-runtime`.
- **Verdict:** **PASS** (recorded for `CELL-DEC`).
