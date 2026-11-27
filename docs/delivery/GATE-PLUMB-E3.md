# GATE-PLUMB-E3 — Misspecified Synthetic Mine Coverage Restoration (`BD-259`)

- **Gate ID:** `GATE-PLUMB-E3` (plan §9.2, §10.7, §33.3, R7-C120, `BD-259`)
- **Claim type:** Known answers on synthetic misspecified and control mines (NOT live site reconciliation or performance)
- **Date:** 2026-11-26

Verdict: PASS (RCH remote Linux `x86_64` synthetic known-answer run; hosted GitHub Actions CI across `ubuntu-latest` + `macos-15` pending post-commit)

## 1. Gate Criteria & Layered Design

Plan §10.7 `GATE-PLUMB-E3` requires that on synthetic misspecified mines (both an unmodelled fault and a biased capex process), pooled over `R >= 50` independent replicate worlds:

1. Layer 5 (`DualSurpriseMonitor`, proven under `GATE-PLUMB-E5`) detects the structural break.
2. Layer 3 (`LayeredRecalibrator` post-alarm segmented conjugate Normal-Inverse-Gamma / Student-$t$ discrepancy model, `BitwisePortable`) restores pooled 80% coverage to `[0.75, 0.85]` (`±5` percentage points) within `12` post-alarm resolutions (`k = 1..=12`, `k = 4..=12`, and at `k = 12`).
3. A single unsegmented global posterior (`GlobalPosteriorBaseline`, using the exact same conjugate Normal-Inverse-Gamma Student-$t$ prior and likelihood without alarm segmentation) documentably fails on the same worlds.
4. Control-mine non-interference: running `LayeredRecalibrator` with every layer active on `WorldCondition::CorrectlySpecified` preserves 95% Clopper–Pearson false-alarm upper bound `<= 0.05` and sharpness loss `< 10%`.

Key epistemic guarantees (plan §9.2, `INV-008`):
- **Dual-surface output:** `LayeredRecalibrator::predict` returns both `raw_interval_80` (L1/L2) and `published_interval_80` (L3) so the calibration ledger scores both separately.
- **Pass-through before alarm:** When L5 is quiet, `published_interval_80 == raw_interval_80`, avoiding unnecessary sharpness loss on well-specified streams.
- **Zero stopping-time selection bias:** Once L5 alarms at `t_alarm`, L3 accumulates strictly post-alarm independent resolutions (`t > t_alarm`), dropping stale pre-fault history without using the triggering observation to fit the post-alarm segment.
- **Exact finite-sample Student-$t$ critical values:** `student_t_quantile_90` uses portable Lanczos $\ln\Gamma$ and Lentz incomplete-beta bisection via `fmine_core::det::{ln, exp, sqrt}` (`BitwisePortable`), verified against independent closed-form elementary-sum values for $\nu \in \{1, 2, 4, 10, 1000\}$ to `1e-10`.

Configuration (fixed analytically before running seed base `20_260_259`, zero seed tuning):
- Layer 5 `DualSurpriseConfig`: `eproc_alpha = 0.04`, `cusum_k = 0.5`, `cusum_h = 8.5`, `alarm_scale = 1.50` (identical to `GATE-PLUMB-E5`).
- Layer 3 conjugate prior: `prior_kappa = 0.25`, `prior_alpha = 1.0` ($\nu_0 = 2$), `prior_scale_mult = 1.0`.

## 2. Measured Results (`R = 200` Replicate Worlds per Regime + `N = 1,000` Control Worlds)

**FACT.** Executed on RCH worker `vmi1227854` (`x86_64-unknown-linux-gnu`) via:

```bash
rch exec -- env CARGO_TARGET_DIR="${RCH_TARGET_BASE:-${TMPDIR:-/tmp}}/rch_target_fmine_verify" \
  cargo test -j 1 --locked -p fmine-gen --test gate_plumb_e3 -- --nocapture
```

Test: `fmine-gen::tests/gate_plumb_e3.rs::test_gate_plumb_e3_layered_stack_restores_coverage_while_global_posterior_fails`
(`1 passed, 0 failed`, `8.58s`) plus `fmine-calib::tests::test_student_t_quantile_90_known_answers` and
`test_layered_recalibrator_pass_through_before_alarm_and_atomic_refusal` (`2 passed, 0 failed`).

### Misspecified Regimes (`R = 200` worlds each, `T = 120` periods, regime break at `t = 60`, seed base `20_260_259`)

| Regime | Alarmed / Full 12-step Window | Median Alarm Period | Layered `k=1..=12` | Layered `k=4..=12` | Layered `k=12` | Global Posterior `k=1..=12` / `k=12` | Raw L1/L2 `k=1..=12` |
|---|---:|---:|---:|---:|---:|---:|---:|
| `MisspecKind::OutOfModelFault` (`-22%` grade drop, `OpenPitAuConfig` + `ScoringHarness`) | `200 / 200` | `66` | **`0.84500`** | **`0.82444`** | **`0.83000`** | `0.47417` / `0.62000` (FAIL) | `0.24750` (FAIL) |
| `MisspecKind::BiasedCapexProcess` (`+0.18` overrun drift, `1.4x` spread, `1/13` blowout tail) | `200 / 200` | `66` | **`0.77292`** | **`0.76444`** | **`0.80500`** | `0.53875` / `0.63000` (FAIL) | `0.32667` (FAIL) |

Every layered coverage metric falls strictly inside `[0.75, 0.85]` (`±5` points of nominal 80%), while both the unsegmented single global posterior (`< 0.65` over `k=1..=12`, `< 0.70` at `k=12`) and raw L1/L2 (`< 0.55`) documentably fail.

### Control-Mine Non-Interference (`WorldCondition::CorrectlySpecified`, `N = 1,000`, `T = 120`, seed base `20_280_259`)

| Worlds | False Alarms | 95% CP Upper Bound | Sharpness Loss |
|---:|---:|---:|---:|
| `1,000` | `16` | **`0.02420`** (`<= 0.05`) | **`0.091%`** (`< 10%`) |

## 3. Limits & Honesty Disclosure

- **Synthetic known-answer evidence only:** Exercises `fmine-gen::worlds::OpenPitAuConfig` (`OutOfModelFault` and `CorrectlySpecified`) and a seeded Bertisen–Davis-style capex overrun process (`BiasedCapexProcess`), not live site reconciliation data.
- **Single-break post-alarm segmentation:** `LayeredRecalibrator` segments once upon the first L5 sticky alarm; multi-regime repeated re-segmentation and site-level `KILL-PLUMB-6` (`bd-261`) remain separate work.
- **Verification scope:** Verified on RCH remote Linux `x86_64` with `cargo test`, `cargo clippy --all-targets -- -D warnings`, and `xtask check-all`. Hosted cross-ISA CI confirmation (`ubuntu-latest` + `macos-15`) runs after commit to `main`; independent human review remains separate.
