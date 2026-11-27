# CONTRACT.md — `fmine-infer`

## 1. Purpose
Lodestar Bayesian inference engine (`BD-232`, plan §27.3, §28.3, §39, `FD-FJ-00`, `PO-TOUCH-4`). Provides a `BitwisePortable` No-U-Turn Sampler (NUTS, Hoffman & Gelman 2014 + Betancourt 2017 multinomial trajectory sampling) with dual-averaging step-size adaptation, Stan 3-stage windowed diagonal mass-matrix adaptation, smooth parameter bijectors with exact log-abs-det-Jacobians, reverse-mode automatic differentiation (`Tape` / `Var`) verified against pinned `frankenjax@fa93fb77` (`fj-core`, `fj-lax`, `fj-ad`) gradient fixtures, and fail-closed Vehtari et al. (2021) rank-normalized split-$\hat{R}$, Bulk ESS, Tail ESS, and E-BFMI publication certificates (`RhatEssCertificate`).

- **Crate:** `fmine-infer`
- **Layer:** E
- **Subsystem:** Lodestar
- **LOC budget:** 24,000 planned / 28,000 ceiling
- **Allowed Module Families:** `["ad", "bijector", "diagnostics", "nuts"]`

## 2. Inputs
- Differentiable unconstrained target log-densities implementing [`LogDensity`] (via [`TapeLogDensity`] or analytic gradients).
- [`NutsConfig`] (deterministic `seed_root`, warmup/draw counts, `target_accept`, `max_treedepth`, `max_delta_h`, optional fixed step size, [`DiagnosticThresholds`]) and initial chain coordinates (`>= 2` chains).
- Smooth [`Bijector`] specifications (`Identity`, `Positive`, `UnitInterval`, `LowerBounded`, `Bounded`) with finite [`Real`] coordinates.
- Dependencies: `fmine-core`, `fmine-evidence` (`E -> S0`; zero external crates in workspace closure per `FD-FJ-00` fallback; `frankenjax@fa93fb77` `fj-ad` verified out-of-band as a `GATE-ORACLE-01` gradient parity oracle).

## 3. Outputs
- [`NutsResult`] carrying unconstrained posterior draws (`chains[chain][draw][param]`), per-chain [`ChainRuntimeStats`] (adapted step size, divergences, max-treedepth hits, acceptance probability, Hamiltonian energies), and [`RhatEssCertificate`].
- [`RhatEssCertificate`] reporting per-parameter posterior summaries (`mean`, `sd`, `q05`, `q50`, `q95`), rank-normalized bulk and folded split-$\hat{R}$, Bulk ESS, Tail ESS, raw ESS, MCSE, per-chain E-BFMI, and a [`DiagnosticVerdict`] (`Certified` or `Refused(Vec<DiagnosticFailure>)`).

## 4. Invariants
- `PO-TOUCH-4`: `RhatEssCertificate::require_certified()` refuses publication (`FMINE-REF-207`) whenever any parameter has rank-normalized split-$\hat{R} > 1.01$, Bulk or Tail ESS below threshold, constant/degenerate draws, post-warmup divergences above limit, or per-chain E-BFMI below threshold.
- `INV-010`: Same `seed_root`, initial positions, config, and model produce bitwise-identical draws and certificates across reruns and reference ISAs (`BitwisePortable` RNG via `fmine_core::rand::PhiloxStream` and `fmine_core::det`).
- `L-6`: Zero bare `f64` parameters in `pub fn` signatures (`xtask check-claims`).
- `GATE-ORACLE-01`: `fixtures/fmine-infer/fj_ad_gradients.txt` is captured directly from pinned `frankenjax@fa93fb77` (`fj-core`, `fj-lax`, `fj-ad`) with a 9-field `.capture.toml` record and verified against `fmine-infer::ad::Tape`.

## 5. Refusals
- `FMINE-REF-109` (exit 3): `< 2` chains, `< 4` post-warmup draws, mismatched dimensions, non-finite inputs/initial densities/gradients, invalid `NutsConfig` or `DiagnosticThresholds`, division by zero or non-positive `ln`/`sqrt` on `Tape`, or out-of-domain `Bijector` bounds/inverses.
- `FMINE-REF-207` (exit 3): non-finite reverse-mode AD evaluation/adjoint, bijector overflow, or `RhatEssCertificate::require_certified()` failing $\hat{R}$, Bulk ESS, Tail ESS, divergence, E-BFMI, or degeneracy gates.
- `FMINE-REF-505` (exit 8): malformed or tampered `fj-ad` oracle parity fixture.

## 6. Determinism class
- `src/lib.rs`, `src/ad.rs`, `src/bijector.rs`, `src/diagnostics.rs`, `src/nuts.rs`: `BitwisePortable` (all transcendentals routed through `fmine_core::det::{exp, ln, tanh, sqrt, sin_cos, norm_ppf}`, all randomness through `fmine_core::rand::PhiloxStream`, fixed-order loops, zero `HashMap`/`HashSet`).

## 7. Evidence class of outputs
- Posterior summaries and certificates are `ModelledUncalibrated` until scored out-of-sample by `fmine-calib` (never promoted to `Measured` per `INV-006`). Synthetic conjugate and SBC test recoveries are labelled synthetic.

## 8. Validity domain
- Smooth, continuously differentiable log-densities on $\mathbb{R}^d$ ($d \ge 1$) with finite initial density and gradient. Discrete latent variables must be marginalized analytically before NUTS. Strongly multimodal targets that trap separate chains are detected and refused by `RhatEssCertificate` ($\hat{R} > 1.01$).

## 9. Oracles
- `fixtures/fmine-infer/fj_ad_gradients.txt` + `fixtures/fmine-infer/fj_ad_gradients.txt.capture.toml`, captured directly from pinned `frankenjax@fa93fb775e1cd73471cd9b2b3e3a014762d1066c` (`fj-core`, `fj-lax`, `fj-ad`) on `nightly-2026-08-31`.
- Closed-form conjugate Bayesian posteriors (1D Normal-Normal, 4D correlated Gaussian, Beta-Binomial via `UnitInterval`, Gamma-Exponential via `Positive`), AR(1) exact autocorrelation time $\tau = (1 + \phi)/(1 - \phi)$, and Simulation-Based Calibration (SBC, Talts et al. 2018).

## 10. Known gaps
- `frankenjax@fa93fb77` (`FD-FJ-00`) is verified out-of-band as a gradient oracle while FrankenMining maintains its zero-external-dependency offline workspace posture; direct Cargo linkage of `fj-core`/`fj-lax`/`fj-ad` awaits the workspace external-dependency wave (`WP-04`).
- Dense (full-covariance) mass-matrix adaptation, Sequential Monte Carlo (SMC), and SPDE/GMRF thinned marked Cox processes belong to `BD-233`.

## 11. Public API
- `ad`: `Real`, `Var`, `Tape`, `LogDensity`, `TapeLogDensity`
- `bijector`: `Bijector`
- `diagnostics`: `DiagnosticThresholds`, `ChainRuntimeStats`, `ParameterDiagnostics`, `DiagnosticFailure`, `DiagnosticVerdict`, `RhatEssCertificate`, `compute_ebfmi`, `compute_parameter_diagnostics`, `certify_chains`
- `nuts`: `NutsConfig`, `NutsResult`, `sample_nuts`
- Crate root: `FJ_PIN_REV`, `FJ_ORACLE_FIXTURE_SHA256`, `FjGradOracleRow`, `parse_fj_oracle_fixture`, `verify_fj_oracle_fixture`

## 12. Proof obligations
- `BD-232` & `PO-TOUCH-4`: conjugate posterior recovery within Monte Carlo error with certified $\hat{R} < 1.01$ and Bulk/Tail ESS $\ge 400$; planted non-convergent/divergent refusal (`FMINE-REF-207`); SBC rank uniformity; AR(1) exact ESS known answer; pinned `frankenjax@fa93fb77` AD gradient parity (`< 1e-13`); bitwise determinism across reruns.
