# FrankenMining Delivery & Gate Records (`docs/delivery/`)

This directory holds machine- and human-verifiable admission records (`INT-*`), milestone gate records (`GATE-*`), and budget amendments (`BUDGET-AMEND-*`). Per `§0.7` and `§28.4`, delivery logs live here rather than mutating the architecture specification.

## Admission records (none admitted)
- [`INT-AS-001.md`](./INT-AS-001.md) — `asupersync =0.5.0` and `franken-evidence` / `franken-decision` adapter boundary (`BD-257`) (not admitted).
- [`INT-NUM-001.md`](./INT-NUM-001.md) — Numerics foundation stack (`fsci =0.4.0`, `fp`, `fnp`, `fj`) closure & cross-ISA determinism scope (`BD-308`) (not admitted).

## Gate records
- [`GATE-LAYERS-00.md`](./GATE-LAYERS-00.md) — every xtask check wired (`BD-015`): PASS measured locally at `2f87dfd`; CI pending.
- [`GATE-BM-01.md`](./GATE-BM-01.md) — lifted `VdbGrid` at 10⁸ active cells (`BD-191`): PASS measured locally at `ef49fcf` (ratio 1.352 ≤ 2.5); self-verified; the test is ignored in CI.
- [`GATE-PUSH-PIT1.md`](./GATE-PUSH-PIT1.md) — MineLib UPIT exact with verified closure (`BD-250`): PASS measured locally at `ef49fcf` on all 11 instances; self-verified; MineLib's files are not committed, so CI cannot run it.

## Kill-gate records
- [`KILL-PUSH-PDLP.md`](./KILL-PUSH-PDLP.md) — PDLP to a 1e-4 certified gap on real-size non-closure LPs (`BD-251`). PASS, so the kill does not fire, measured locally at `9b9fe28`:
  - all 10 MineLib CPIT LP relaxations, up to 9.5×10⁷ nonzeros, at ε = 1e-6 within 30 minutes;
  - at the default 1e-4, 5 of 10 miss;
  - self-verified, and CI cannot run it.

## Decisions
- [`DECISION-BD-012-direct-push-blocking.md`](./DECISION-BD-012-direct-push-blocking.md) — what "CI blocks merges" (§28.4.4) means when every change is pushed straight to `main` (`BD-012`).

## Budget amendments (none approved)
Form: [`docs/templates/BUDGET-AMEND.md`](../templates/BUDGET-AMEND.md). `xtask check-amend` reads these records (§28.4.3, `BD-012`).
- [`BUDGET-AMEND-001-fmine-public.md`](./BUDGET-AMEND-001-fmine-public.md) — `fmine-public` 18000 → 22000 planned, for the `check tables` binding (`bd-4wvl.8`) (PENDING; the binding later landed in `fmine-cli`, see 002).
- [`BUDGET-AMEND-002-fmine-cli.md`](./BUDGET-AMEND-002-fmine-cli.md) — `fmine-cli` 26000 → 31000 planned, for the verbs already landed past 26000 and the open `lint`, `track-record` and pit verbs (PENDING).
