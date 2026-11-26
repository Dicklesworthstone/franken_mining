# GATE-PLUMB-E5 — Surprise Monitors on the Correctly Specified Control Mine (`BD-051`)

- **Gate ID:** `GATE-PLUMB-E5` (plan §9.2, §10.4, §10.7, §33.3, `BD-051`)
- **Claim type:** Known answers on synthetic control and misspecified mines (NOT live site reconciliation or performance)
- **Date:** 2026-11-26

Verdict: PASS (RCH remote Linux `x86_64` synthetic known-answer run; self-verified only; independent GitHub Actions CI pending)

## 1. Gate Criteria & Analytic Tuning

Plan §10.7 requires that on a correctly specified control mine (`WorldCondition::CorrectlySpecified`,
`BiasVector::unbiased()`), over `N >= 1,000` null runs:

1. The one-sided 95% Clopper–Pearson upper bound on the false-alarm rate is `<= alpha` (`0.05`).
2. Sharpness loss with the layered L3 recalibration stack active is `< 10%`.

Analytic monitor budget for `alpha = 0.05` over `T = 120` periods per run (chosen before running seed
`20_260_511`; an earlier trial at `cusum_h = 6.8` on seed `20_260_510` yielded `88/2,000` CUSUM false alarms,
95% CP upper bound `0.0523 > 0.05`, matching Siegmund's `ARL_0 ≈ 2,881`, and was retired before choosing the
analytic split below on an untouched seed):

- **E-process pair (`DualSurpriseMonitor`):** Waudby-Smith/Ramdas `ScoreBettingEProcess` on location
  `tanh(z/2)` and centred dispersion `tanh((z² - 1)/2) - c0` at `eproc_alpha = 0.04` (`0.02` each),
  guaranteeing anytime false-alarm probability `<= 0.04` by Ville's inequality.
- **Two-sided Page CUSUM:** `k = 0.5, h = 8.5` (Siegmund `ARL_0 ≈ exp(8.5 + 1.166) - 10.7 ≈ 15,760`,
  satisfying `KILL-PLUMB-6`'s `ARL_0 >= 85·M` for up to `M = 185` streams; expected 120-period FAR `≈ 0.0076`).
- **Union bound:** `0.04 + 0.0076 = 0.0476 < 0.05`.
- **Causal L3 scale factor:** `1.0` prior to alarm; `1.50x` predictive standard deviation on subsequent periods
  only after a sticky alarm (`INV-008`, zero lookahead).

## 2. Measured Results (`N = 2,000` Control Runs + Misspecified Power + Planted Negatives)

**FACT.** Executed on RCH worker `vmi1264463` (`x86_64-unknown-linux-gnu`) via:

```bash
rch exec -- env CARGO_TARGET_DIR="${RCH_TARGET_BASE:-${TMPDIR:-/tmp}}/rch_target_fmine_verify" \
  cargo test -j 1 --locked -p fmine-gen --test gate_plumb_e5 -- --nocapture
```

Test: `fmine-gen::tests/gate_plumb_e5.rs::test_gate_plumb_e5_control_mine_false_alarm_sharpness_and_power`
(`1 passed, 0 failed`, `7.55s`) plus `fmine-calib::tests::test_dual_surprise_monitor_atomic_refusal_and_sticky_alarms`
and `test_dual_surprise_monitor_catches_variance_inflation_via_dispersion_eproc` (`2 passed, 0 failed`) plus
existing `fmine-calib::tests::test_spatial_front_pit_detects_58pct_coverage_shift_and_keeps_ville_bound`
(`694/20,000` null spatial runs, 95% CP upper bound `0.0369 <= 0.05`).

### Control Mine (`WorldCondition::CorrectlySpecified`, `BiasVector::unbiased()`, `N = 2,000`, `T = 120`, seed base `20_260_511`)

| Channel | E-proc alarms (CP95) | CUSUM alarms (CP95) | Combined alarms (CP95) | Sharpness loss | CRPS loss | Raw / Layered 80% cov |
|---|---:|---:|---:|---:|---:|---:|
| Grade forecast innovation (`ch 0`) | 35 / 2,000 (`0.02314`) | 13 / 2,000 (`0.01031`) | 46 / 2,000 (`0.02931`) | `0.512%` | `0.051%` | `0.7990` / `0.8005` |
| Resource-vs-sampler recon (`ch 1`) | 19 / 2,000 (`0.01391`) | 20 / 2,000 (`0.01450`) | 38 / 2,000 (`0.02483`) | `0.466%` | `0.046%` | `0.7993` / `0.8007` |

Every Clopper–Pearson 95% upper bound is strictly `<= 0.05`, and both sharpness loss (`< 0.6%`) and CRPS
degradation (`< 0.06%`) are strictly `< 10%`.

### Positive Power on Misspecified Mines (`N = 200` runs each, identical parameters)

| Misspecified Regime | E-proc power (median period) | CUSUM power (median period) | Raw -> Layered 80% cov |
|---|---:|---:|---:|
| `MisspecKind::OutOfModelFault` (step fault at `t = 60`) | 200 / 200 (`t = 81`) | 200 / 200 (`t = 66`) | `0.5304` -> `0.6414` |
| `BiasVector` (`+10%` resource grade bias) | 200 / 200 (`t = 18`) | 200 / 200 (`t = 10`) | — |

### Planted Negative Controls (All Rejected by Gate)

- Mis-tuned CUSUM (`h = 2.5`, `N = 1,000` control runs): 95% CP upper bound **`0.98671 > 0.05`** (FAIL).
- Unmodelled `+5%` resource grade bias leaked into control (`N = 1,000`): combined 95% CP upper bound **`1.00000 > 0.05`** (FAIL).
- Unconditional `1.25x` L3 width inflation (`N = 200`): sharpness loss **`25.141% >= 10%`** (FAIL).

## 3. Limits & Honesty Disclosure

- **Synthetic known-answer evidence only:** Exercises `fmine-gen::worlds` sealed-truth synthetic worlds (`OpenPitAuConfig`), not reconciled live mine operations.
- **Self-verification only:** Authored and run by the implementing agent on RCH Linux workers (`x86_64`); independent human review and hosted GitHub Actions CI confirmation are pending.
- **Downstream gates remain separate:** `GATE-PLUMB-E3` (`bd-259`, misspecified coverage restoration within 12 resolutions) and `KILL-PLUMB-6` (`bd-261`, full multi-stream quarterly site control check) remain open on their own beads.
