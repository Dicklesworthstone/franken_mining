# CONTRACT.md — `fmine-derive`

## 1. Purpose
Testpit T-layer build-time / dev-only symbolic derivation crate (`BD-139`, plan §27.3 & §39, `OPEN-SUITE-3`, `GATE-ORACLE-01`). Holds the pinned `frankensympy@9d785d0` symbolic golden formula catalog (Lane limiting cut-off grades for `fm-cutoff` / `BD-252`, two-product metallurgical recovery and Jacobian for `fm-recon` / AMIRA P754, ad-valorem NSR and break-even grade for `fm-fiscal`, WGC AISC unit cost and Jacobian for `fm-metrics`, and bilinear contained-metal residual for `fm-recon`), verifies checked-in `fixtures/fmine-derive/golden_formulas.txt` byte-for-byte against the catalog, and provides fail-closed Rust golden evaluators and exact Jacobians for downstream oracle tests. Never linked into any production closure (`xtask check-oracle`, `GATE-ORACLE-01`).

- **Crate:** `fmine-derive`
- **Layer:** T
- **Subsystem:** Testpit
- **LOC budget:** 8,000 planned / 9,000 ceiling
- **Allowed Module Families:** `["golden"]`

## 2. Inputs
- Pinned symbolic fixture text `fixtures/fmine-derive/golden_formulas.txt` captured from `frankensympy@9d785d0` (`fsym-core`, `fsym-calculus`, `fsym-simplify`, `fsym-solvers`, `fsym-printing` on `nightly-2026-08-31`).
- Typed domain operating-point parameter structs (`LaneInputs`, `TwoProductInputs`, `NsrInputs`, `WgcAiscInputs`, `BilinearMetalInputs`).
- Dependencies: `fmine-core`, `fmine-evidence` (`T → S0`, zero external crates; `fsym` runs strictly out-of-band as a dev-only capture oracle per `franken-stack.lock` `FD-FSYM-00`).

## 3. Outputs
- `GoldenFormulaRow`, `GoldenFormulaSpec`, and `GOLDEN_CATALOG` (26 `fsym@9d785d0`-derived formulas with canonical AST string, LaTeX, Rust expression, and exact rational evaluation).
- Typed golden evaluations and exact symbolic Jacobians (`LaneCutoffsWithJacobian`, `TwoProductWithJacobian`, `NsrWithSensitivities`, `WgcAiscWithJacobian`, `BilinearMetalWithJacobian`). Not `Claim` objects (Testpit oracle layer only).

## 4. Invariants
- `GATE-ORACLE-01`: `fmine-derive` and `fsym-*` never appear in any production crate closure (`xtask check-oracle`).
- Every row of `fixtures/fmine-derive/golden_formulas.txt` must match `GOLDEN_CATALOG` byte-for-byte (`test_fixture_matches_catalog_and_detects_tampering`).
- Every symbolic partial derivative agrees with independent central finite differences (`test_symbolic_jacobians_match_central_finite_differences`).
- No `NaN`, `Inf`, or clamped zero-denominator value is ever emitted (`test_pole_and_domain_refusals_fail_closed`).

## 5. Refusals
- `FMINE-REF-109` (exit 3): non-finite inputs, out-of-domain physical parameters (`R <= 0`, `R > 1`, `P <= r`, `H <= 0`, `K <= 0`, `f <= 0`, `c <= 0`, `t < 0`, `c == t`, `tau < 0`, `tau >= 1`, `P <= c_t`, `T <= 0`, `g <= 0`, `C < 0`), or zero/negative net refining margin `P - r - (f + F)/K <= 0`.
- `FMINE-REF-207` (exit 3): floating-point overflow or non-finite intermediate/output derivative evaluation.
- `FMINE-REF-505` (exit 8): malformed, missing, duplicate, or drifted row in `fixtures/fmine-derive/golden_formulas.txt` against `GOLDEN_CATALOG`.

## 6. Determinism class
- `src/lib.rs`: `BitwisePortable` (pure IEEE-754 arithmetic and exact string/catalog comparison; zero platform libm transcendentals, zero RNG, zero hash-map iteration). Verified by `test_golden_evaluators_are_bitwise_deterministic`.

## 7. Evidence class of outputs
- Synthetic / Symbolic Oracle (`Testpit` layer only; never promoted to `Measured` or `ModelledCalibrated` per `INV-006`).

## 8. Validity domain
- Strictly positive finite capacities (`H > 0`, `K > 0`), positive metallurgical recovery (`0 < R <= 1`), positive net metal price (`P > r` and `P - r > (f + F)/K` for refinery-limited Lane cut-off; `P > c_t` and `0 <= tau < 1` for NSR), positive feed/concentrate grades with `c != t` for two-product recovery, and positive denominator factors (`T > 0, g > 0, R > 0`) for WGC AISC. Any boundary or pole violation refuses closed with `FMINE-REF-109` or `FMINE-REF-207`.

## 9. Oracles
- `fixtures/fmine-derive/golden_formulas.txt` (`sha256:56f60d5f268350305d5806bd03a3653860c201ffdfd6a3ff8d036d57672732a4`), captured from pinned `frankensympy@9d785d0ef485e02ffce670bf77b4bb280b157c3f` on `nightly-2026-08-31` with `GATE-ORACLE-01` record `fixtures/fmine-derive/golden_formulas.txt.capture.toml`.
- Cross-checked independently by exact closed-form rational verification and central finite differences in `crates/fmine-derive/src/lib.rs` unit tests.

## 10. Known gaps
- `frankensympy` (`FD-FSYM-00`) is invoked out-of-band on the 16-crate pure symbolic closure (excluding `fsym-durable`/`fsym-runtime`/`fsym-numeric-consumer`, which carry sibling relative path dependencies); live re-derivation requires a local checkout of `frankensympy@9d785d0` (documented in `docs/delivery/OPEN-SUITE-3-fsym.md`).
- Higher-order non-linear fiscal regimes (progressive sliding-scale tax brackets, depreciation schedules) and multi-element N-product reconciliation Jacobians remain future extensions when `fmine-econ::fiscal` and `fmine-recon` add those kernels.

## 11. Public API
- `FSYM_PIN_REV`, `GOLDEN_FIXTURE_SHA256`, `GoldenFormulaSpec`, `GoldenFormulaRow`, `GOLDEN_CATALOG`
- `parse_golden_fixture`, `verify_fixture_against_catalog`
- `LaneInputs`, `LaneCutoffsWithJacobian`, `eval_lane_cutoffs`
- `TwoProductInputs`, `TwoProductWithJacobian`, `eval_two_product_recovery`
- `NsrInputs`, `NsrWithSensitivities`, `eval_nsr`
- `WgcAiscInputs`, `WgcAiscWithJacobian`, `eval_wgc_aisc`
- `BilinearMetalInputs`, `BilinearMetalWithJacobian`, `eval_bilinear_metal`

## 12. Proof obligations
- `GATE-ORACLE-01`: `xtask check-oracle` verifies zero `fmine-derive`/`fsym-*` presence in any production closure and validates `fixtures/fmine-derive/golden_formulas.txt.capture.toml` SHA-256 integrity.
- `OPEN-SUITE-3`: Resolved in `docs/delivery/OPEN-SUITE-3-fsym.md` (16-crate pure symbolic closure of `frankensympy@9d785d0` compiles and runs on `nightly-2026-08-31`).
- Golden parity & Jacobian soundness: `test_fixture_matches_catalog_and_detects_tampering`, `test_exact_rational_fixture_points_match_evaluators`, `test_symbolic_jacobians_match_central_finite_differences`, `test_algebraic_domain_invariants`, `test_pole_and_domain_refusals_fail_closed`, `test_golden_evaluators_are_bitwise_deterministic`.
