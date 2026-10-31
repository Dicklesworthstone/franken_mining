# FrankenMining Delivery & Gate Records (`docs/delivery/`)

This directory holds machine- and human-verifiable admission records (`INT-*`), milestone gate records (`GATE-*`), and budget amendments (`BUDGET-AMEND-*`). Per `§0.7` and `§28.4`, delivery logs live here rather than mutating the architecture specification.

## Admission records (none admitted)
- [`INT-AS-001.md`](./INT-AS-001.md) — `asupersync =0.5.0` and `franken-evidence` / `franken-decision` adapter boundary (`BD-257`) (not admitted).
- [`INT-NUM-001.md`](./INT-NUM-001.md) — Numerics foundation stack (`fsci =0.4.0`, `fp`, `fnp`, `fj`) closure & cross-ISA determinism scope (`BD-308`) (not admitted).

## Budget amendments (none approved)
Form: [`docs/templates/BUDGET-AMEND.md`](../templates/BUDGET-AMEND.md). `xtask check-amend` reads these records (§28.4.3, `BD-012`).
- [`BUDGET-AMEND-001-fmine-public.md`](./BUDGET-AMEND-001-fmine-public.md) — `fmine-public` 18000 → 22000 planned, for the `check tables` binding (`bd-4wvl.8`) (PENDING).
