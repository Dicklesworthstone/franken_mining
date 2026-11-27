# GATE-PLUMB-E3 — Misspecified Synthetic Mine Coverage Restoration (`BD-259`)

- **Gate ID:** `GATE-PLUMB-E3` (plan §9.2, §10.7, §33.3, R7-C120, `BD-259`)
- **Claim type:** Known answers on synthetic misspecified and control mines (NOT live site reconciliation or performance)
- **Date:** 2026-11-27

Verdict: PASS (RCH remote Linux `x86_64` synthetic known-answer run across 3 misspecified regimes + control mine; hosted GitHub Actions CI across `ubuntu-latest` + `macos-15` pending post-commit)

## 1. Gate Criteria & Layered Design

Plan §10.7 `GATE-PLUMB-E3` requires that on synthetic misspecified mines (both an unmodelled fault and a biased capex process; additionally hardened with a pure `2.5x` dispersion explosion regime), pooled over `R >= 50` independent replicate worlds:

1. Layer 5 (`DualSurpriseMonitor`, proven under `GATE-PLUMB-E5`) detects the structural break.
2. Layer 3 (`LayeredRecalibrator` post-alarm segmented standardized-residual Student-$t$ discrepancy model, `BitwisePortable`) restores pooled 80% coverage to `[0.75, 0.85]` (`±5` percentage points) within `12` post-alarm resolutions (`k = 1..=12`, `k = 4..=12`, and at `k = 12`).
3. A single unsegmented global posterior (`GlobalPosteriorBaseline`, using the exact same standardized-residual Student-$t$ predictive law without alarm segmentation) documentably fails on the same worlds.
4. Control-mine non-interference: running `LayeredRecalibrator` with every layer active on `WorldCondition::CorrectlySpecified` preserves 95% Clopper–Pearson false-alarm upper bound `<= 0.05` and sharpness loss `< 10%`.

Key epistemic guarantees (plan §9.2, `INV-008`):
- **Dual-surface output:** `LayeredRecalibrator::predict` returns both `raw_interval_80` (L1/L2) and `published_interval_80` (L3) so the calibration ledger scores both separately.
- **Pass-through before alarm:** When L5 is quiet, `published_interval_80 == raw_interval_80`, avoiding unnecessary sharpness loss on well-specified streams.
- **Post-alarm support:** L3 fits standardized residuals `z = (y - mu_raw) / sigma_raw` only for
  `t > t_alarm`. The triggering observation stays outside the fitted segment. For `n < 2`, the
  predictor uses its declared Normal-Inverse-Gamma prior. From two observations onward, it uses the
  segment mean and scatter with a positive Student-t scale ridge. The next forecast's raw scale
  maps the interval back to physical units.
- **Finite-sample Student-$t$ critical values:** `student_t_quantile_90` uses portable Lanczos $\ln\Gamma$ and Lentz incomplete-beta bisection via `fmine_core::det::{ln, exp, sqrt}` (`BitwisePortable`), verified against independent closed-form elementary-sum values for $\nu \in \{1, 2, 4, 10, 1000\}$ to `1e-10` and strictly monotone through $\nu = 999{,}999$.

Configuration for the run at seed base `20_260_259`:
- Layer 5 `DualSurpriseConfig`: `eproc_alpha = 0.04`, `cusum_k = 0.5`, `cusum_h = 8.5`, `alarm_scale = 1.50` (identical to `GATE-PLUMB-E5`).
- Layer 3: `prior_kappa = 0.25`, `prior_alpha = 1.0` ($\nu_0 = 2$), `prior_scale_mult = 1.5`; the
  Student-t variance ridge uses shape `0.01` and the declared prior scale for `n >= 2`.

## 2. Measured Results (`R = 200` Replicate Worlds per Regime + `N = 1,000` Control Worlds)

**FACT.** Executed on RCH remote Linux `x86_64-unknown-linux-gnu` via:

```bash
rch exec -- env CARGO_TARGET_DIR="${RCH_TARGET_BASE:-${TMPDIR:-/tmp}}/rch_target_fmine_verify" \
  cargo test -j 1 --locked -p fmine-gen --test gate_plumb_e3 -- --nocapture
```

Test: `fmine-gen::tests/gate_plumb_e3.rs::test_gate_plumb_e3_layered_stack_restores_coverage_while_global_posterior_fails`
(`1 passed, 0 failed`, `23.10s`) plus `fmine-calib --lib` (`292 passed, 0 failed`, including `test_student_t_quantile_90_known_answers`, `test_student_t_quantile_90_monotone_through_large_nu`, `test_layered_recalibrator_pass_through_before_alarm_and_atomic_refusal`, and `test_layered_recalibrator_scale_equivariance_heteroscedasticity_and_determinism`).

### Misspecified Regimes (`R = 200` worlds each, `T = 120` periods, regime break at `t = 60`, seed base `20_260_259`)

| Regime | Alarmed / Full 12-step Window | Median Alarm Period | Layered `k=1..=12` | Layered `k=4..=12` | Layered `k=12` | Global Posterior `k=1..=12` / `k=12` | Raw L1/L2 `k=1..=12` |
|---|---:|---:|---:|---:|---:|---:|---:|
| `MisspecKind::OutOfModelFault` (`-22%` grade drop, `OpenPitAuConfig` + `ScoringHarness`) | `200 / 200` | `66` | **`0.84458`** | **`0.81111`** | **`0.84000`** | `0.48292` / `0.62500` (FAIL) | `0.24750` (FAIL) |
| `MisspecKind::BiasedCapexProcess` (`+0.18` overrun drift, `1.4x` spread, `1/13` blowout tail) | `200 / 200` | `66` | **`0.82125`** | **`0.80389`** | **`0.81500`** | `0.54625` / `0.63500` (FAIL) | `0.32667` (FAIL) |
| Pure dispersion shift (`0` mean shift, `2.5x` standard deviation) | `200 / 196` | `74` | **`0.80395`** | **`0.80392`** | **`0.80102`** | `0.58312` / `0.58673` (FAIL) | `0.39463` (FAIL) |

Every layered coverage metric across all three regimes falls strictly inside `[0.75, 0.85]` (`±5` points of nominal 80%), while both the unsegmented single global posterior (`< 0.65` over `k=1..=12`, `< 0.70` at `k=12`) and raw L1/L2 (`< 0.55`) documentably fail.

In the pure dispersion regime the two-sided CUSUM also reacts to the larger residuals and usually alarms first, so coverage alone would pass with the dispersion e-process dead: a review (2026-11-27) found that switching the e-processes off left coverage in band and cost only 11 full windows (185, against the 190 required). The gate now also requires the e-process pair to alarm in at least 190 of the 200 dispersion worlds. It does in `196`; a dead e-process gives `0`.

### Control-Mine Non-Interference (`WorldCondition::CorrectlySpecified`, `N = 1,000`, `T = 120`, seed base `20_280_259`)

| Worlds | False Alarms | 95% CP Upper Bound | Sharpness Loss |
|---:|---:|---:|---:|
| `1,000` | `16` | **`0.02420`** (`<= 0.05`) | **`0.166%`** (`< 10%`) |

## 3. Limits & Honesty Disclosure

- **Synthetic known-answer evidence only:** Exercises `fmine-gen::worlds::OpenPitAuConfig` (`OutOfModelFault` and `CorrectlySpecified`) and a seeded Bertisen–Davis-style capex overrun process (`BiasedCapexProcess`), not live site reconciliation data.
- **Single-break post-alarm segmentation:** `LayeredRecalibrator` segments once upon the first L5 sticky alarm; multi-regime repeated re-segmentation and site-level `KILL-PLUMB-6` (`bd-261`) remain separate work.
- **Verification scope:** Verified on RCH remote Linux `x86_64` with `cargo test`, `cargo clippy --all-targets -- -D warnings`, and `xtask check-all`. Hosted cross-ISA CI confirmation (`ubuntu-latest` + `macos-15`) runs after commit to `main`; independent human review remains separate.

## 4. Dispersion regression and prior sensitivity (bd-9mk9)

**FACT.** On `aarch64-apple-darwin`, published commit `9fa80061` reached `0.6935` pooled coverage in
the pure-dispersion case. Commit `65675b60` restored the original `[0.75, 0.85]` gate with the
original prior scale of `1.0`. Pure-dispersion coverage over the first 12 post-alarm resolutions was
`0.78463` at seed `20260259` and `0.79723` and `0.78956` at seed bases `34700259` and `51800259`.
Each run had 200 replicate worlds, with 196, 197 and 196 full 12-resolution windows respectively.
The original-seed global and raw comparisons were `0.58270` and `0.39463`. E3's other regimes and
control assertions passed on all three seeds.

**FACT.** The direct transition test at the same revision compares the interval widths to numerical
integration of the Student-t density at zero, one and two fitted observations. It checks the alarm
exclusion, equality with the global predictor on identical inputs, and a positive width after repeated residuals.
