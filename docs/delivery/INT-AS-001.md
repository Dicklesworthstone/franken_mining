# INT-AS-001: `asupersync` (`=0.5.0`) and Adapter Admission Record, split into 001a (WP-00) and 001b (WP-05)

- **Bead:** 001a: BD-325 (`bd-wso3`, WP-00); 001b: BD-257 (`bd-257`, WP-05, M1) (plan §34.2)
- **Date:** 2026-10-29 (rewritten 2026-10-30 by BD-323.S3)
- **Foundation Row:** `FD-AS-00` (`franken-stack.lock`)
- **Pre-admission desk check (not admission evidence):**
  1. FACT: the newest `asupersync` published on `crates.io` is `0.5.0` (tag `v0.5.0`, commit `78b64636e`), the
     version that `frankengraphdb`, `franken_accounting`, `frankensqlite`, `frankensearch`, `fastmcp =0.13.0` and
     `frankentui =0.9.0` use (E653–E655, Suite-Fit Audit H.3).
  2. FACT: no fmine crate depends on `franken-evidence` or `franken-decision` today. `crates/fmine-decide/Cargo.toml`
     has five path dependencies only (`fmine-core`, `fmine-evidence`, `fmine-claim`, `fmine-numerics`,
     `fmine-calib`). When admitted, the two crates are confined to `fmine-decide` (`L-4`), because they depend on
     `serde`; the S0 crates keep their own `std`-only types.
  3. FACT, static half of criterion 5 only: `xtask check-caps` bans `open_child_region` outside `fmine-runtime`
     (`crates/xtask/src/lib.rs:401`, defect `S3(a)`). No runtime check exists, because no crate runs asupersync.
- **Status:** INT-AS-001a: NOT_APPLICABLE. asupersync is absent from `Cargo.lock` at `6a14976` (16 workspace
  packages, 0 `source` lines); its checks are wired by BD-325. INT-AS-001b (criteria 1–8): PENDING, WP-05, M1.
