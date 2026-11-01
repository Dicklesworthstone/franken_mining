# CONTRACT.md — `fmine-runtime`

## 1. Purpose
The runtime owns the host wall clock. Today that is all it holds:
- a `WallClock` trait, so callers take the clock as an injected capability;
- the `HostWallClock` adapter, which reads `SystemTime::now()` and converts it exactly to signed Unix nanoseconds.

A reading claims no synchronisation, uncertainty bound, monotonicity or authority. The caller decides whether to trust it, under its own policy.

- **Crate:** `fmine-runtime`
- **Layer:** S1
- **Subsystem:** Bedrock
- **LOC budget:** 12,000 planned / 14,000 ceiling
- **Allowed Module Families:** `["clock"]`

## 2. Inputs
- **The host clock**, through `HostWallClock::now_ns`.
- **Nothing else.** It has no workspace or external dependencies, and reads no files or registries.

## 3. Outputs
- **`WallClock::now_ns() -> Result<i64, ClockError>`**: nanoseconds since the Unix epoch, in 0 to `i64::MAX` inclusive.
- **No Claim and no trusted time.** The one caller, `fmine-cli`'s `reliance` module (BD-347), maps a `ClockError` to `FMINE-REF-506`. It builds a `TrustedTime` from the reading only when its access policy sets `trust_host_clock`, and takes the uncertainty from its own header's `max_clock_skew_ns`.

## 4. Invariants
- **Exact or refused, never wrapped or clamped.** `nanosecond_conversion_is_exact_and_never_wraps_or_clamps` checks 1.234567890 s, `i64::MAX` ns, `i64::MAX + 1` ns and `Duration::MAX`.
- **The epoch is 0, and earlier is refused.** `unix_epoch_is_zero_and_pre_epoch_is_refused`.
- **An injected clock passes its reading or error through unchanged**, with no host clock: `injected_clock_keeps_its_reading_or_failure_without_a_host_clock_dependency`.
- **No fallback.** A failed reading is an error. There is no cached value, no zero and no substitute clock.
- **Only this crate and `fmine-lab` may read ambient time or start a detached worker.** `xtask check-caps` bans `SystemTime::now`, `Instant::now`, `thread::spawn`, `thread::Builder` and `thread::sleep` in every other crate's production code (INT-AS-001 criterion 9). It also bans `open_child_region` outside this crate. Seeded negatives: `test_seeded_detached_worker_or_timer_fails_outside_the_runtime` and `test_seeded_open_child_region_fails`, in `crates/xtask/src/lib.rs`.

## 5. Refusals
- **`ClockError::{BeforeUnixEpoch, NanosecondOverflow}`**, a local error with no refusal code. Callers map it to their own refusal. `fmine-cli` uses `FMINE-REF-506`.

## 6. Determinism class
`src/lib.rs` has no `//! Determinism:` header. The conversion is exact integer arithmetic. The reading is external, so `HostWallClock` gives a different value on every call by design. A deterministic consumer injects a fixed `WallClock`.

## 7. Evidence class of outputs
Not applicable: it emits no Claim. A reading is not evidence of the true time.

## 8. Validity domain
- **The range is 1970-01-01T00:00:00Z to about 2262-04-11.** Past that, `i64` nanoseconds overflow, which refuses.
- **Rollback is not hidden.** Successive readings may go backwards, and the adapter neither detects nor corrects that.
- **No quality.** Skew, leap-second handling and synchronisation are whatever the host gives.

## 9. Oracles
No fixture. The tests are known answers at the conversion's edges: the epoch, one second before it, an exact sub-second value, `i64::MAX` and its successor, and `Duration::MAX`.

## 10. Known gaps
- **Almost all of the planned runtime is absent.** The registry charter and plan §28.3 list a region tree, CapSet profiles, an AppSpec per role, budgets and an obligation registry. None exists.
- **No asupersync.** The workspace pins it as the sole async runtime (`asupersync_pin = "=0.5.0"`), but it is not in `Cargo.lock`, so `check-all` reports INT-AS-001a as `NOT_APPLICABLE`.
- **Bedrock obligations that name the runtime are not discharged here:** PO-BED-5 (obligation closure on region close) and PO-BED-8 (K/E crates never obtain IO or REMOTE). `check-caps` is the static half of PO-BED-8 only.
- **No clock-quality validator, synchronisation service or authority ledger.** A trusted time comes only from the caller's own policy.
- **No `clock` module.** The listed family `clock` is the code in `src/lib.rs`.

## 11. Public API
- `trait WallClock { fn now_ns(&self) -> Result<i64, ClockError>; }`
- `HostWallClock` (`Debug, Clone, Copy, Default`), which implements `WallClock`.
- `ClockError::{BeforeUnixEpoch, NanosecondOverflow}`, with `Display` and `Error`.

## 12. Proof obligations
- None is discharged here. PO-BED-5 and PO-BED-8 wait on the region runtime (Known gaps).
- The crate's own property is tested: an exact, unclamped conversion with the pre-epoch and overflow refusals (Invariants).
