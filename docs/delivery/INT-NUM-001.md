# INT-NUM-001: Numerics Foundations Admission & Determinism Scope Record

- **Bead:** `bd-308` (`WP-00`)
- **Date:** 2026-10-29
- **Foundation Rows:** `FD-FSCI-01`, `FD-FP-01`, `FD-FNP-01`, `FD-FJ-01` (`franken-stack.lock`)
- **Verified Closures & Rules (`§27.3`, `§29`, `§34.2`):**
  1. **L-5 Compliance:** `fsci-* =0.4.0` is consumed with `default-features = false` (no `asupersync` feature split). `fp-io` is excluded (`KILL-6` path taken: `fp-io` pulls `arrow`, `parquet`, and `chrono`; `fmine-io` provides pure-Rust CSV/XLSX/OMF readers). `fj` is consumed via `fj-core`, `fj-lax`, `fj-ad` only (excluding `fj-api` / `fj-ffi`).
  2. **Cross-ISA Determinism Scope (`§29.1`):**
     - `BitwisePortable` kernels (`fmine-core::det`, `fmine-core::rand`, `fmine-core::hash`, `fmine-units`, `fmine-time`, `fmine-claim`, `fmine-calib`, `fmine-metrics`) use `fmine_core::det` fixed-polynomial transcendentals and integer/rational arithmetic, producing bit-identical digests across `x86_64-linux`, `aarch64-darwin`, and `wasm32`.
     - `TargetDeterministic` linear-algebra/optimization solves record target triple and certificate tolerance (`FMT-CERT-1`).
- **Verdict:** **PASS**.
