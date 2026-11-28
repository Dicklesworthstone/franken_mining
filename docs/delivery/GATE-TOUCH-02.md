# GATE-TOUCH-02: Bias Recovery, SBC Calibration, Identifiability, and 5% Quarterly Sampler Power (`BD-158`, `BD-159`)

Verdict: PASS

- **Beads:** `BD-158` (`bd-158`, M1 kill gate `GATE-TOUCH-02`), `BD-159` (`bd-159`, quarterly 5% sampler bias detection power), `BD-152` (`bd-152`, hierarchical bias posteriors SBC), `BD-153` (`bd-153`, identifiability & Measurement VOI), `BD-156` (`bd-156`, `ReconciliationCertificate` `FMT-CERT-1` verifier round-trip) — all WP-08 Touchstone
- **Claim type:** Known answers (400-replicate synthetic-truth bias recovery & Talts et al. 2018 SBC via `fmine-gen::sbc` and `fmine-gen::biases`, 24-period `fmine-gen::mill_balance` metal closure, confounded-pair `FMINE-REF-104` refusals + Measurement VOI, 2,000-replicate quarterly 5% sampler bias detection power, and `fmine-verify` offline `FMT-CERT-1` round-trip / tamper exit-8)
- **Plan specification:** §20.5–§20.11 (`GATE-TOUCH-02`, `PO-TOUCH-1`, `PO-TOUCH-5`, `PO-TOUCH-6`, `G-TOUCH-2`, `G-TOUCH-4`, `G-TOUCH-5`, `INV-002`, `INV-005`)
- **Test harnesses:**
  - `crates/fmine-verify/tests/gate_touch_02.rs`:
    - `test_gate_touch_02_bias_recovery_sbc_identifiability_and_5pct_power`
    - `test_bd156_reconciliation_certificate_fmine_verify_roundtrip_and_tamper`
  - `crates/fmine-recon/tests/recon_known_answers.rs` (`12 passed; 0 failed` in `25.36s` on `hz3`)

## Measured Gate Run (FACT)

- **Command:** `cargo test -j 1 --locked -p fmine-verify --test gate_touch_01 --test gate_touch_02 --test kill_plumb_6 -- --nocapture`
- **Host / Target:** RCH remote worker `vmi1149989` (`x86_64-unknown-linux-gnu`), `dev` profile, `rustc 1.100` (`nightly-2026-08-31`)
- **Duration (`gate_touch_02`):** `64.48s` test execution, `2 passed; 0 failed`
- **Aggregate `GATE-TOUCH-02` BLAKE3 digest (`PO-TOUCH-6`):** `11278829f5c92d31ef2f7f6dfa12bc381097bc79a8b06baa98ecea1670129060`
- **`BD-156` `ReconciliationCertificate` verifier digests:**
  - `payload_sha256`: `7288c3efbf183d831f95eeb81a3bffee6775706444131bd1aab19d13613ab0e6`
  - `replay_blake3`: `0dc4b2c39342f25ad28bcf65ee7b53c640bbf16baee8d836b851c908ba5e0256`
  - `fmt_cert_digest`: `53eef9e291ff917e996ce602ead761be14ffb8eede0e63009df27fd000749e43`

| Metric | Plan §20.10 / Kill Threshold | Observed | Verdict |
|---|---|---|---|
| Seeded bias-recovery replicates (`BD-130` 6 families + 2 hierarchical domains + group hyperprior) | `>= 200` | `400` (`8` tracked parameters) | PASS |
| `PO-TOUCH-5` / `GATE-TOUCH-02` 90% credible-interval coverage across all 8 parameters | `[0.850, 0.950]` (kill outside `80–97%`) | `[0.8950, 0.9250]` | PASS |
| `BD-152` Talts et al. SBC rank-histogram uniformity (`K = 10` bins, `L = 99` draws, `alpha = 0.01`) | `8 / 8 Calibrated`, `p > 0.01` | `8 / 8 Calibrated`, `min p = 0.0408` | PASS |
| `PO-TOUCH-1` metal balance closure across 24 monthly `mill_balance` periods | `< 1e-10` | `9.34e-17` | PASS |
| Reconciled Cu metal uncertainty coverage vs seeded truth (`±2.2 sigma`) | `>= 20 / 24` | `24 / 24` | PASS |
| Confounded pairs (`registries/biases.toml`) refused attribution (`G-TOUCH-2`) | `4 / 4` -> `FMINE-REF-104` | `4 / 4` (`rank 1/2`, VOI resolves `rank 2/2`) | PASS |
| `BD-156` offline `fmine-verify cert --inputs-dir` round-trip & single-bit tamper | valid exit `0`, tamper exit `8` (`FMINE-REF-505`) | valid exit `0`, cert/input/RCRT tamper exit `8` | PASS |

## Quarterly 5% Mill Head Sampler Bias Detection Power Statement (`BD-159`, FACT)

Evaluated on a Cu–Au concentrator balance (`feed = 1,000 t Cu/mo`, `conc = 880 t Cu/mo` at `0.25x` settlement assay SD, `tails = 120 t Cu/mo`) over one quarter (`3` independent monthly periods, global `alpha = 0.05`, measurement Šidák family-wise `alpha = 0.05`, target MDB power `80%`), cross-validated against `2,000` seeded Monte Carlo replicates per noise regime:

| Monthly Feed Assay Rel. SD | Planted Bias | Analytical Quarterly Power | Empirical MC Power (`2,000` reps) | Quarterly Measurement MDB (`80%` power) |
|---|---|---|---|---|
| `1.5%` | `-5.0%` (`-50 t Cu/mo`) | `99.99%` (`0.9999`) | `100.00%` (`1.0000`) | `2.88%` |
| `2.0%` | `-5.0%` (`-50 t Cu/mo`) | `98.75%` (`0.9875`) | `98.85%` (`0.9885`) | `3.84%` |
| `3.0%` | `-5.0%` (`-50 t Cu/mo`) | `79.96%` (`0.7996`) | `80.50%` (`0.8050`) | `5.77%` |

Under nominal concentrator assay precision (`1.5–2.0%` monthly feed SD), a persistent `5%` head-sampler bias is detected within one quarter at `>= 98.7%` statistical power; when feed assay SD degrades to `3.0%`, one-quarter power drops to `~80.0%` (requiring `5.77%` quarterly MDB on the per-measurement Šidák test), quantifying the exact instrumentation precision floor needed before a site pilot.

## Validity Boundary

Exercises `fmine-recon` (`bias`, `identifiability`, `certificate`, `linear`, `gross_error`, `bilinear`), `fmine-gen` (`biases`, `sbc`, `mill_balance`), and `fmine-verify` on seeded linear-Gaussian hierarchical bias models and synthetic concentrator balances. Live site pilot parity (`GATE-TOUCH-03`, `BD-206`), rolling-origin multi-quarter hindcasts (`BD-161`, `GATE-TOUCH-05`), and dynamic stockpile voxel tracking (`BD-312`) belong to downstream beads.
