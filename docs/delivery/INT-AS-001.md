# INT-AS-001: `asupersync` (`=0.6.2`) and Adapter Admission Record, split into 001a (WP-00) and 001b (WP-05)

- **Bead:** 001a: BD-325 (`bd-wso3`, WP-00); 001b: BD-257 (`bd-257`, WP-05, M1) (plan §34.2)
- **Date:** 2026-10-29 (rewritten 2026-10-30 by BD-323.S3; pin moved 2026-11-27 by BD-121)
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
- **Pin move (2026-11-27, BD-121, owner-approved):** the first crate to bring asupersync into the graph is
  `fsqlite =0.5.1` (`fmine-projection`, native targets only), which requires asupersync `^0.6.2`. `FD-AS-00` moved
  from `=0.5.0` (`78b64636e`) to `=0.6.2` (tag `v0.6.2`, `a5d23e8dd2c8d776bd02e2da8e38e6296600e84c`), and the
  root `Cargo.toml` patches crates.io's `asupersync` to that revision (check C). Commit `021a58e6`.
  1. FACT: suite crates pin different asupersync versions today: `fastmcp 0.14.0` `=0.6.2`, `frankensearch`
     `0.6.2`, `frankentui 0.11.1` `0.7.0`, `frankengraphdb` HEAD `=0.6.0` (git `3d0151ae`), `franken_accounting`
     `0.5.0` (a subprocess, not linked). `franken-kernel`, `franken-decision` and `franken-evidence` `0.6.0` come
     from crates.io in the closure. A crate that links another version into the workspace graph fails check A.
  2. FACT: the closure grew from 43 packages (1 from crates.io) to 279. tokio appears only in the all-targets
     union, through `wasm-bindgen-futures` on wasm32 under `cfg(all(target_os = "emscripten",
     wasm_bindgen_unstable_tokio))`, which no build here sets. So `check-int-as-001a` now walks one closure per
     target in `deny.toml [graph] targets` (x86_64 Linux, aarch64 macOS, wasm32): asupersync is in the two native
     closures and absent from wasm32.
- **Status:** INT-AS-001a: PASS. `xtask check-int-as-001a` prints `INT-AS-001a: checks PASS at
  8b38c0b67ada133a0cc0c3d8ef4b315153b4334194fd5a55d91e8ff9ce973034` (the `Cargo.lock` SHA-256 at `021a58e6`):
  one asupersync package id per closure (A), no second runtime, detached worker or ambient timer (B(i), B(ii)),
  and the patch at the `franken-stack.lock` revision (C). Tests:
  `xtask::int_as::tests::test_workspace_closures_pass_on_every_target` and the 7 others in `int_as`. INT-AS-001b
  (criteria 1–8): PENDING, WP-05, M1.
- **Superseded status (kept):** INT-AS-001a: NOT_APPLICABLE. asupersync was absent from `Cargo.lock` at `d1d3b18`
  (16 workspace packages, 0 `source` lines); `xtask check-int-as-001a`, run by `check-all` in CI, printed this line
  until it was present (checks wired: `4416b0a`).
