# GATE-LAYERS-00: every xtask check wired from commit 1 (BD-015)

- **Status:** PASS measured locally at `2f87dfd` on 2026-11-01. **CI: pending.** It needs the first CI run whose
  head contains `2f87dfd`. The L-6 check in its current form (`6b105e3`) is not yet on `origin/main`, so no CI run
  has checked it.
- **Recorded by:** SwiftBasin (agent). Self-verified only.

## What is wired

`check_all` (`crates/xtask/src/lib.rs`) runs, in order:
1. the LOC budget (§28.4);
2. layers, deps (L-5) and caps;
3. lifts (L-7);
4. scope;
5. claims (L-6);
6. det;
7. the sources registry;
8. calendars;
9. refusals;
10. the stack lock;
11. `check-amend` against `HEAD` (§28.4.3).

CI (`.github/workflows/ci.yml`) runs `cargo run -p xtask -- check-all` as the step "Run GATE-LAYERS-00 (xtask
check-all)" in every matrix job. Three steps run beside it: `check-amend --base` over the pushed range, `cargo deny
check`, and clippy with warnings denied.

## Measured at `2f87dfd`

`cargo run -q -p xtask -- check-all` exits 0:

```
GATE-LAYERS-00 PASS: 60 crates registered, 90771 actual Rust LOC (planned 1128000, ceiling 1298000)
{"check":"check-det","files":103,"headers_missing":0,"violations":0,"bitwise_files":10}
{"check":"stack-lock","lock_ids":15,"f1_ids":15,"missing":[],"extra":[],"bad_suffix":[]}
{"check":"gate-oracle-01","closure_packages":79,"fixtures":2,"oracles_in_closure":0}
{"check":"calendars","sites":8,"tzdata":"2026e"}
{"check":"check-refusals","rows":33,"active":25,"proposed":8,"reserved":0,"plan_codes":25,"files":163,"tokens":687,"warnings":8}
{"check":"check-amend","measured":true,"changed":1,"over_planned":1,"amendments":1,"approved":0,"warnings":1}
```

## Known limits, recorded rather than hidden

- **L-6 debt.** Five allow rows in `registries/l6_allow.toml` are `kind = "debt"`. `bd-u56b` tracks them and waits
  on `bd-062`, `bd-063`, `bd-274` and `bd-225`, whose redesigns remove them. New bare-`f64` parameters fail.
- **Budget.** `fmine-public` is over its planned 18,000 lines (20,567), with `BUDGET-AMEND-001` pending owner
  approval. A change that adds to the crate fails `check-amend`. The block is procedural
  (`DECISION-BD-012-direct-push-blocking.md`).
- **Refusals.** Eight codes are proposed in `docs/adr/ADR-REFUSALS-01.md` but not yet in plan §8.4. They are
  warnings.

## To close BD-015

Cite this record together with `ci:<run-id>@<sha7>`, from a run whose head contains `2f87dfd` and whose check-all
step passed.
