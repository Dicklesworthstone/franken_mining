# BD-232 Delivery Record — `fmine-infer` NUTS, Reverse-Mode AD, Bijectors, and Vehtari $\hat{R}$/ESS Certificates

- **Bead:** `bd-232` (`BD-232 [WP-15]: NUTS on frankenjax with diagnostics`)
- **Unblocks:** `bd-233` (`BD-233 [WP-15]: Thinned marked Cox process on SPDE/GMRF`)
- **Claim types:** Parity (`frankenjax@fa93fb77` `fj-ad` gradient oracle), Known answers (conjugate posteriors, AR(1) exact autocorrelation time, SBC rank uniformity, planted bimodal/divergent refusals)
- **Date:** 2026-11-27

## 1. Pinned `frankenjax@fa93fb77` Empirical Verification (`FD-FJ-00`, `GATE-ORACLE-01`)

**FACT.** An isolated read-only archive of pinned `frankenjax@fa93fb775e1cd73471cd9b2b3e3a014762d1066c` (`franken-stack.lock` `FD-FJ-00`) pulling only the pure AD closure (`fj-core`, `fj-lax`, `fj-ad`; zero `fj-api`/`fj-ffi`/`fj-backend-gpu`) compiled and executed cleanly on `nightly-2026-08-31` (`x86_64-unknown-linux-gnu` via RCH worker `ovh-b`, `exit 0`).

Using `fj_ad::value_and_grad_jaxpr`, 10 log-density and VJP gradient cases across five mining/Bayesian models (`normal_2d_corr`, `beta_logit_transformed`, `gamma_positive_transformed`, `hierarchical_ncp_4d`, `student_t_robust`) were captured into:
- Fixture: `fixtures/fmine-infer/fj_ad_gradients.txt` (`sha256:866f0b911310d1046dbe56cfff96ff520de407d71e9879a40c47ea447db91b4b`)
- Capture record: `fixtures/fmine-infer/fj_ad_gradients.txt.capture.toml`

Per `FD-FJ-00`'s workspace posture, `crates/fmine-infer` stays zero-external-dependency in the offline Cargo workspace (using its `BitwisePortable` reverse-mode `Tape` AD engine, verified against `fj-ad@fa93fb77` to `< 1e-13` on all 10 cases).

## 2. Implemented Capabilities (`crates/fmine-infer`, Layer `E`, `BitwisePortable`)

1. **`ad.rs`:** Reverse-mode Wengert `Tape` / `Var`, L-6-clean `Real`, `LogDensity`, and `TapeLogDensity` using `fmine_core::det::{exp, ln, sqrt, tanh}` with fail-closed non-finite/domain checks.
2. **`bijector.rs`:** `Identity`, `Positive`, `UnitInterval`, `LowerBounded`, and `Bounded` bijectors with overflow-safe `log_det_jacobian` and `Tape`-differentiated `forward_var`.
3. **`nuts.rs`:** `BitwisePortable` No-U-Turn Sampler (Hoffman & Gelman 2014 + Betancourt 2017 multinomial trajectory sampling), bounded initial step-size heuristic, Nesterov dual-averaging step-size adaptation, Stan 3-stage windowed Welford diagonal mass-matrix adaptation, and `fmine_core::rand::PhiloxStream` (`RngKey`, `BD-020`).
4. **`diagnostics.rs`:** Vehtari et al. (2021) Blom rank-normalized bulk and folded split-$\hat{R}$, multi-chain Geyer Initial Positive Monotone Sequence Bulk ESS and Tail ESS (with `BitwisePortable` radix-2 FFT autocovariance for $n > 256$), per-chain E-BFMI, and fail-closed `RhatEssCertificate::require_certified()` (`PO-TOUCH-4`, `FMINE-REF-207`).

## 3. Limits

- Direct Cargo linkage of `fj-core`/`fj-lax`/`fj-ad` into `Cargo.lock` awaits the workspace external-dependency wave (`WP-04`); until then, `fj-ad@fa93fb77` serves as the pinned `GATE-ORACLE-01` gradient parity oracle.
- Dense mass-matrix adaptation, SMC, and SPDE/GMRF Cox process kernels belong to `BD-233`.
