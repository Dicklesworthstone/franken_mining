# GATE-TOUCH-01: Synthetic Flowsheets and Mill Balance Known Answers (`BD-157`)

Verdict: PASS

- **Bead:** `BD-157` (`bd-157`, WP-08 Touchstone reconciliation; covers `BD-149`, `BD-150`, `BD-151`)
- **Claim type:** Known answers (seeded synthetic multi-node linear WLS, gross-error power curves, coupled Cu–Au bilinear reconciliation over `fmine-gen::mill_balance`, and planted-fault guard controls)
- **Plan specification:** §20.1–§20.4, §20.10–§20.11 (`GATE-TOUCH-01`, `PO-TOUCH-1`, `PO-TOUCH-2`, `PO-TOUCH-3`, `PO-TOUCH-6`, `G-TOUCH-1`, `G-TOUCH-2`, `G-TOUCH-3`, `INV-005`)
- **Test harness:** `crates/fmine-verify/tests/gate_touch_01.rs`
  - `test_gate_touch_01_synthetic_flowsheets_and_mill_balance`
  - `test_gate_touch_01_planted_faults_and_guards`
- **Crate test suites (also run):**
  - `crates/fmine-recon/tests/recon_known_answers.rs` (`8 passed; 0 failed` in `14.81s` on `ovh-a`)
  - `crates/fmine-numerics` `reconciliation` unit tests (`33 passed; 0 failed` in `0.00s` on `ovh-a`)

## Measured Gate Run (FACT)

- **Command:** `cargo test -j 1 --locked -p fmine-verify --test gate_touch_01 -- --nocapture`
- **Host / Target:** RCH remote worker `ovh-a` (`x86_64-unknown-linux-gnu`), `dev` profile, `rustc 1.100` (`nightly-2026-08-31`)
- **Duration:** `10.81s` test execution (`80.4s` remote build + test), `2 passed; 0 failed`
- **Aggregate gate digest (BLAKE3 over reconciled digests and power curves, `PO-TOUCH-6`):** `19304723e0f88244deaf71f899cfdcc9ffae9848c965c84d37ec1dc6d6b7b88b`

| Metric | Gate Threshold | Observed | Verdict |
|---|---|---|---|
| Part A: Linear WLS Monte Carlo replicates (2-node, 5 streams, `rho = 0.30`) | `>= 1,000` | `2,000` | PASS |
| Part A: `PO-TOUCH-1` max relative linear closure error | `< 1e-11` | `7.63e-17` | PASS |
| Part A: `PO-TOUCH-2` empirical `90%` CI coverage across all 5 streams | `[0.860, 0.940]` | `[0.902, 0.910]` | PASS |
| Part A: `PO-TOUCH-2` max normalized `5 x 5` posterior covariance error vs analytic | `< 0.110` | `0.052` | PASS |
| Part B: `BD-150` null-size false-positive rate (`0.0 sigma`, `alpha = 0.05`, `2,000` reps) | `0.0500 +/- 0.035` | `0.0510` (theory `0.0500`) | PASS |
| Part B: `BD-150` global chi-square power at `3.0 sigma` (`2,000` reps) | `0.5379 +/- 0.035` | `0.5575` (theory `0.5379`) | PASS |
| Part B: `BD-150` global chi-square power at `5.0 sigma` (`2,000` reps) | `0.9445 +/- 0.035` | `0.9480` (theory `0.9445`) | PASS |
| Part C: `fmine-gen::mill_balance` (`BD-133`) monthly periods (unbiased + biased) | `24 + 24` | `24 + 24` | PASS |
| Part C: `PO-TOUCH-1` max relative bilinear closure (dry mass + Cu + Au) | `< 1e-10` | `9.97e-17` | PASS |
| Part C: Unbiased Cu recovery RMSE vs seeded truth (reconciled vs raw two-product) | `recon < raw` | `0.00524 < 0.01042` (`-49.7%`) | PASS |
| Part C: Planted sampler (`0.96`) + moisture (`1.08`) bias Mahalanobis objective shift | `biased > 2.0x unbiased` | `8.14` vs `2.47` (`3.30x`) | PASS |

## Planted-Fault Negative Controls

`test_gate_touch_01_planted_faults_and_guards` verifies four independent guard refusals fail closed:
1. `G-TOUCH-1`: Direct intensive grade reconciliation refuses with `FMINE-REF-103` (`NonAdditiveAverage103`).
2. `G-TOUCH-2`: Confounded bias pair (`b_gc - b_mill`, `GB-25`) refuses attribution with `FMINE-REF-104` (`NonIdentifiableAttribution104`).
3. `G-TOUCH-3`: Steady-state bilinear balance over an interior `StockpileOrPad` node refuses with `FMINE-REF-102` (`SupportMismatch102`).
4. `INV-005`: Commit gate refuses a planted `+8 sigma` feed weighbridge gross error (`+200 t` on `25 t` SD) with `FMINE-REF-101` (`Conservation101`).

## Validity Boundary

Exercises `fmine-flow`, `fmine-numerics::reconciliation` (linear WLS, partial observability, and constrained Gauss–Newton SQP), `fmine-recon` (`linear`, `gross_error`, `bilinear`), and `fmine-gen::mill_balance` on seeded synthetic circuits and concentrator observations. Hierarchical bias posteriors per node (`BD-152`), per-lot moisture balances (`BD-155`), and live mine-to-mill site exports belong to subsequent WP-08 beads and are not claimed by `GATE-TOUCH-01`.
