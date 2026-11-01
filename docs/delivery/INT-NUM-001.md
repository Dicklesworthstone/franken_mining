# INT-NUM-001: Numerics Foundations Admission and Determinism Scope Record

- **Bead:** `bd-308` (`WP-00`)
- **Date:** 2026-10-29 (rewritten 2026-10-30 by BD-323.S3)
- **Foundation Rows:** `FD-FSCI-00`, `FD-FP-00`, `FD-FNP-00`, `FD-FJ-00` (`franken-stack.lock`)
- **Closures and rules (`§27.3`, `§29`, `§34.2`):**
  1. **L-5:** FACT: `fsci`, `fp`, `fnp` and `fj` are not consumed; no fsci, fp, fnp or fj crate is in `Cargo.lock`
     (16 workspace packages, 0 `source` lines at `6a14976`). Intentions, when admitted: `fsci-* =0.4.0` with
     `default-features = false`; `fp` without `fp-io` (`KILL-6`: `fp-io` pulls `arrow`, `parquet` and `chrono`;
     `fmine-io` reads CSV and XLSX in pure Rust); `fj` through `fj-core`, `fj-lax` and `fj-ad` only.
  2. **Cross-ISA determinism scope (`§29.1`):**
     - FACT: the modules marked `//! Determinism: BitwisePortable` are the seven fmine-core files `lib.rs`, `id.rs`,
       `det.rs`, `hash.rs`, `anchor.rs`, `rand.rs` and `codec.rs`. Their tests, including the 10k-input golden
       hash, pass on `x86_64-linux` and `aarch64-darwin` (`ci:40974504857@748da79`). `wasm32` is untested.
     - FACT (NEW-det, `bd-hp2j`, 2026-11-01): `fmine-calib/src/lib.rs`, `fmine-metrics/src/lmdi.rs` and
       `fmine-numerics/src/lib.rs` make no std transcendental call (`xtask check-det`; R7-C019's `exp` and `ln` go
       through `fmine_core::det`), and are marked BitwisePortable (commit `9e7a96c`), ten files in all. Their seven 10k-input golden digests printed the
       same values on `ubuntu-latest` (x86_64) and `macos-15` (aarch64) in `ci:41237993223@84f32ac`.
     - `TargetDeterministic` linear-algebra and optimization solves record the target triple and certificate
       tolerance (`FMT-CERT-1`) once they exist.
- **Status:** FD-row fallback taken (own kernels in `fmine-numerics`); admission PENDING until a foundation enters
  `Cargo.lock`.
