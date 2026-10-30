# CONTRACT.md — `fmine-calib`

## 1. Charter & Layer
- **Crate:** `fmine-calib`
- **Layer:** `K0` (Kernel Foundation)
- **Subsystem:** `Kernels`
- **LOC Budget:** `14k planned / 16k ceiling` (`registries/loc_budget.toml`)
- **Allowed Module Families:** `["ledger", "scoring", "prior", "pit"]`

## 2. Mathematical & Domain Formulation
Implements the append-only pre-registration `ForecastLedger` (`INV-008`, `FMINE-REF-403`), strictly proper scoring rules (`gaussian_crps`, `gaussian_log_score`, `winkler_interval_score`), `ReferenceClassPrior` (`GB-32`: capex `+28%/+43%`, schedule `+31%`, ramp-up `74%` Yr-1), and `SpatialFrontPitMonitor` (`GB-08` Vecchia/Rosenblatt conditional PIT + `BettingEProcess`).

## 3. Authority, Epistemics & Representation
Enforces pre-registration of forecasts prior to outcome observation (`INV-008`) and monitors spatial front miscalibration (`G-CAL-1`).

## 4. Public API Surface
- `ForecastEntry`, `ForecastResolution`, `ForecastLedger`
- `gaussian_crps`, `gaussian_log_score`, `winkler_interval_score` → `Result<f64, ScoreError>` (non-finite inputs, non-positive std_dev, alpha outside (0, 1) and inverted intervals refuse; nothing is clamped, bd-8wrk)
- `ForecastLedger::evaluate_model` → `Result<CalibrationReport, ScoreError>`; `register` and `record_outcome` refuse non-finite or non-positive-std forecasts and non-finite outcomes at entry
- `ProjectArchetype`, `ReferenceClassPrior`
- `SpatialFrontPitMonitor`

## 5. Guards & Typed Refusals
- `FMINE-REF-403` (`RetroactiveForecastEdit`, `INV-008`)

## 6. Proof Obligations (`PO-*`)
- `PO-CAL-1`: Expected Gaussian CRPS is uniquely minimized at the true data-generating $(\mu_0, \sigma_0)$.
- `PO-CAL-2`: Retroactive forecast registration (`t_registered >= t_outcome_avail`) is refused with `FMINE-REF-403`.

## 7. Failure Modes & Deterministic Fallbacks
Miscalibrated spatial fronts trip `BettingEProcess` at `E_t >= 20.0` (`alpha = 0.05`) and downgrade downstream claims.

## 8. Gates & Kill Criteria (`GATE-*` / `KILL-*`)
- `GATE-HELM-01`

## 9. Lifted Code & Foundation Dependencies
- Depends on `fmine-core`, `fmine-time`, `fmine-evidence`, `fmine-claim`, `fmine-numerics`.

## 10. Non-Goals & Scope Boundary
Pure calibration and scoring kernel; does not perform file I/O (`L-3`).
