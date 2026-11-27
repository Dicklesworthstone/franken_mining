# GATE-ORACLE-01: no oracle in a production closure; a capture record beside every fixture (BD-013)

Verdict: PASS (measured locally at `e9400927` on 2026-11-27; the same check runs in every CI job's check-all step; self-verified only)

- **Gate (plan §33.2):** no oracle crate or tool in any production posture closure, and every fixture has a capture
  record. It applies from commit 1.
- **Measured:** `cargo run -q -p xtask -- check-oracle` at `e9400927` on 2026-11-27, on `aarch64-apple-darwin`
  through rch (run locally). Exit 0:

  ```
  {"check":"gate-oracle-01","closure_packages":191,"fixtures":11,"oracles_in_closure":0}
  check-oracle: PASS
  ```

- **Recorded by:** SilentBison (agent). Self-verified only.

## What the check does

`check_oracle` (`crates/xtask/src/lib.rs`, `crates/xtask/src/oracle.rs`) runs:
1. `check_deps`: no workspace member's manifest names an oracle crate (`ORACLE_CRATES`);
2. the closure: `cargo tree -e normal,build` over the workspace, without `xtask` and the oracle members, names no
   oracle package at any depth. A failure prints the path that brought the oracle in;
3. the capture records: every file under `fixtures/` has a `<file>.capture.toml` beside it, with the nine §33.2
   fields once each and an `outputs_digest` equal to the fixture's sha256. A record without a fixture fails too.

`check-all` runs the same check. CI runs `check-all` as the step "Run GATE-LAYERS-00 (xtask check-all)" in every
matrix job, so each push re-measures this gate. BD-013 closed on `ci:41267868225@3fdc08b`, where that step passed
in all four jobs, and on `test:xtask::oracle::tests::test_seeded_oracle_outside_the_workspace_fails_the_closure_but_not_check_deps`.

## Why this record exists now

BD-232 closed on 2026-11-27 at 14:11Z with a reason that names GATE-ORACLE-01. The pinned `frankenjax` gradient
oracle serves through a captured fixture (`fixtures/fmine-infer/fj_ad_gradients.txt`) and is not linked. Check-all's
bead rule R-GATE requires a `docs/delivery/GATE-ORACLE-01*.md` with a PASS verdict for any close that names the
gate. There was none, so `check-all` exited 2 on main from that close on. This record writes down the gate's
measured state; it changes no code.

## Limits

- Dev-dependencies are outside the closure by design. A test may use an oracle crate.
- The closure is what `cargo tree` resolves for this host. Target-specific dependencies for other hosts are covered
  by the CI matrix (`ubuntu-latest`, `macos-15`).
- An untracked `fixtures/.DS_Store` on this machine has no record and is not counted. The 11 tracked fixtures each
  have one.
