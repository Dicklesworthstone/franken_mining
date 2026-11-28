# KILL-PLUMB-6: Control-Mine Quarterly Monitor False-Alarm Check (`BD-261`)

Verdict: PASS

- **Bead:** `BD-261` (`bd-261`, WP-08 Touchstone; depends on `BD-051`, `BD-158`)
- **Claim type:** Known answers (2,000 unbiased synthetic control-mine site-quarters across `M = 4` Touchstone reconciliation residual streams monitored by `fmine-calib::LayeredRecalibrator` / `DualSurpriseConfig`, plus positive `-5%` bias shift control and mis-tuned negative control)
- **Plan specification:** §10.4, §34.5 (`KILL-PLUMB-6`: `<= 1` false alarm per site per quarter; CUSUM tuned to `ARL_0 >= 85 * M`, target expected rate `<= 0.36` per site per quarter, `<= 5%` of quarters exceeding `1` false alarm)
- **Test harness:** `crates/fmine-verify/tests/kill_plumb_6.rs`
  - `test_kill_plumb_6_control_mine_quarterly_false_alarm_rate_and_shift_power`

## Measured Gate Run (FACT)

- **Command:** `cargo test -j 1 --locked -p fmine-verify --test gate_touch_01 --test gate_touch_02 --test kill_plumb_6 -- --nocapture`
- **Host / Target:** RCH remote worker `vmi1149989` (`x86_64-unknown-linux-gnu`), `dev` profile, `rustc 1.100` (`nightly-2026-08-31`)
- **Duration (`kill_plumb_6`):** `1.57s` test execution, `1 passed; 0 failed`
- **Aggregate `KILL-PLUMB-6` BLAKE3 digest:** `f25d11a7fc74e6f871c94e6463b64a5928492107dc07998fa46928d75647e10c`
- **Monitor tuning (`M = 4` streams, `12` weekly periods per quarter):** `eproc_alpha = 0.01`, two-sided Page CUSUM `k = 0.5`, `h = 8.5` (Siegmund `ARL_0 ≈ 15,760 >> 85 * 4 = 340`).

| Metric | Plan `KILL-PLUMB-6` Threshold | Observed | Verdict |
|---|---|---|---|
| Control-mine site-quarters (`500` sites × `4` quarters × `4` streams) | large control sample | `2,000` site-quarters (`8,000` stream-quarters) | PASS |
| Mean false alarms per site per quarter | `<= 0.36` (kill `> 1.0`) | `0.0005` (`1 / 2,000`) | PASS |
| Site-quarters with `> 1` false alarm | `<= 5.0%` | `0 / 2,000` (`0.00%`; 95% CP upper bound `0.18%`) | PASS |
| Positive control: `-5%` sampler bias detection within 2 quarters (`100` sites, untuned) | `>= 90%` | `100%` (`100 / 100`) | PASS |
| Negative control: mis-tuned CUSUM `h = 1.5` (`100` sites) | breaches `> 1.0 / site-qtr` | `2.857` alarms / site-quarter | PASS (caught) |

## Validity Boundary

Exercises `fmine-gen::worlds` (`OpenPitAuConfig`, `WorldCondition::CorrectlySpecified`, accessed strictly through `ScoringHarness` sealed truth) coupled to `fmine-calib::LayeredRecalibrator` (`DualSurpriseConfig`: `ScoreBettingEProcess` + `TwoSidedCusum`) across four Touchstone reconciliation log-ratio streams (`truck/resource` tonnes, `resource/sampler` grade, `mill/refinery` metal, `resource/refinery` F3). Live site non-stationary seasonality and unmodeled autocorrelation are evaluated during pilot hindcasts (`GATE-TOUCH-05`).
