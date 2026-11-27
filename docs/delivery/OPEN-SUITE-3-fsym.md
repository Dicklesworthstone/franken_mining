# OPEN-SUITE-3 Resolution — `fmine-derive` and `frankensympy@9d785d0` on `nightly-2026-08-31` (`BD-139`)

- **Open Question:** `OPEN-SUITE-3` (plan §27.5, §27.7, §39 `BD-139`, `franken-stack.lock` `FD-FSYM-00`)
- **Question:** Do the `fsym` crates compile on FrankenMining's pinned toolchain `nightly-2026-08-31` (rustc 1.100, Rust 2024 edition) when pulled individually?
- **Claim type:** Parity / Known answers (build-time symbolic oracle verification; NOT live site data or performance)
- **Date:** 2026-11-27

Verdict: **RESOLVED — YES (for the 16-crate pure symbolic closure; no separate toolchain required for `fmine-derive`)**

## 1. Empirical Compiler Finding (`frankensympy@9d785d0`)

**FACT.** An isolated read-only archive of the exact pinned `frankensympy` revision `9d785d0ef485e02ffce670bf77b4bb280b157c3f` (`franken-stack.lock` `FD-FSYM-00`) was tested on `nightly-2026-08-31` (`aarch64-apple-darwin`, rustc 1.100.0-nightly):

1. **Whole-workspace check caveat:** `frankensympy`'s full root workspace cannot be materialized standalone because three non-symbolic crates declare sibling relative path dependencies outside the repository (`fsym-durable` and `fsym-runtime` reference `../asupersync`; `fsym-numeric-consumer` references `../franken_numpy-pin` and `../frankenscipy-pin`).
2. **Individual pure symbolic closure (what `OPEN-SUITE-3` asks):** Pulling the 16-crate pure symbolic closure — `fsym-id`, `fsym-budget`, `fsym-outcome`, `fsym-bigint`, `fsym-rational`, `fsym-modular`, `fsym-core`, `fsym-polys`, `fsym-evidence`, `fsym-proof-kernel`, `fsym-assumptions`, `fsym-sets`, `fsym-simplify`, `fsym-calculus`, `fsym-solvers`, and `fsym-printing` — requires **zero** `asupersync` or sibling path dependencies and compiled cleanly on `nightly-2026-08-31` in `37.94s` (`exit 0`, zero errors):

```bash
RUSTUP_TOOLCHAIN=nightly-2026-08-31 \
  cargo check --offline -j 2 \
  -p fsym-core -p fsym-printing -p fsym-simplify -p fsym-calculus -p fsym-solvers
```

3. **No separate toolchain needed:** Although `frankensympy` pins `nightly-2026-08-20` in its own `rust-toolchain.toml`, its pure symbolic crates compile and execute cleanly on FrankenMining's pinned `nightly-2026-08-31`.

## 2. Captured Golden Formulas (`GATE-ORACLE-01`)

**FACT.** A capture binary compiled directly against pinned `frankensympy@9d785d0` on `nightly-2026-08-31` derived **26 symbolic golden formulas** (symbolic solutions via `fsym_solvers::solve_linear`, exact partial derivatives via `fsym_calculus::diff`, simplification via `fsym_simplify::{cancel, simplify}`, LaTeX and Rust codegen via `fsym_printing::{latex, to_rust_code}`, and exact rational evaluations) across five core mining domains:

1. **Lane's 3 limiting cut-off grades & Jacobians (`fm-cutoff`, unblocking `BD-252`):** $g_m$, $g_h$, $g_r$, $\partial g_m / \partial(c_h, P, R)$, $\partial g_h / \partial(F, H)$, $\partial g_r / \partial K$.
2. **AMIRA P754 two-product metallurgical recovery & Jacobian (`fm-recon`):** $R(f, c, t) = \frac{c(f - t)}{f(c - t)}$ and $\nabla R(f, c, t)$.
3. **Ad-valorem NSR & break-even grade (`fm-fiscal`):** $\text{NSR}(g, R, P, \tau, c_t, c_m)$, $\partial \text{NSR}/\partial g$, $\partial \text{NSR}/\partial \tau$, and $g_{\text{be}}$.
4. **WGC AISC unit cost & Jacobian (`fm-metrics` / `GB-26`):** $\text{AISC}(C, T, g, R) = C / (T g R)$ and $\nabla \text{AISC}$.
5. **Bilinear contained-metal residual & Jacobian (`fm-recon`):** $r(T, g, M) = T g - M$ and $\nabla r$.

Pinned oracle artifact and `GATE-ORACLE-01` capture record:
- Fixture: `fixtures/fmine-derive/golden_formulas.txt` (`sha256:56f60d5f268350305d5806bd03a3653860c201ffdfd6a3ff8d036d57672732a4`)
- Record: `fixtures/fmine-derive/golden_formulas.txt.capture.toml`

## 3. Workspace Isolation & Verification (`crates/fmine-derive`)

- **Posture (`franken-stack.lock` `FD-FSYM-00`, `GATE-ORACLE-01`):** `crates/fmine-derive` joins layer `T` (Testpit) with zero external dependencies (`fmine-core`, `fmine-evidence` only), keeping offline/RCH/CI builds free of `frankensympy` path dependencies and guaranteeing zero oracle crates in any production closure (`xtask check-oracle`).
- **Tests (`crates/fmine-derive/src/lib.rs`):**
  - `test_fixture_matches_catalog_and_detects_tampering` (26/26 rows + planted tampering refusal `FMINE-REF-505`)
  - `test_exact_rational_fixture_points_match_evaluators` (all 26 `fsym` exact rationals verified to `1e-13`)
  - `test_symbolic_jacobians_match_central_finite_differences` (all 16 symbolic partial derivatives cross-checked independently)
  - `test_algebraic_domain_invariants`, `test_pole_and_domain_refusals_fail_closed`, `test_golden_evaluators_are_bitwise_deterministic`.

## 4. Limits

- The `frankensympy@9d785d0` compilation probe ran locally on `aarch64-apple-darwin` (because `frankensympy` lives outside the `franken_mining` workspace and RCH does not materialize cross-repo sibling path dependencies). `crates/fmine-derive` and `xtask check-all` (including `GATE-ORACLE-01`) are verified on RCH remote `x86_64-unknown-linux-gnu`; hosted cross-ISA CI confirmation runs after commit to `main`.
