# CONTRACT.md — `fmine-runtime`

## 1. Charter & Layer

- **Crate:** `fmine-runtime`
- **Layer:** `S1` (Runtime)
- **Subsystem:** Bedrock
- **LOC Budget:** 12k planned / 14k ceiling (`registries/loc_budget.toml`)
- **Allowed Module Families:** `["clock"]`

## 2. Mathematical & Domain Formulation

DESIGN: the first runtime slice exposes an injected `WallClock` and the explicit `HostWallClock` adapter.
It converts elapsed time since the Unix epoch to nonnegative signed 64-bit nanoseconds using checked conversion.
The admitted numeric range is 0 through `i64::MAX`, inclusive. Pre-epoch and overflowing times are errors.

## 3. Authority, Epistemics & Representation

A host reading does not establish synchronization, uncertainty, monotonicity or external timestamp authority.
The caller must separately supply any owner-declared quality policy and decide whether the reading qualifies.
The adapter never makes a historical journal timestamp trusted and never corrects or conceals clock rollback.

## 4. Public API Surface

- `WallClock::now_ns() -> Result<i64, ClockError>`
- `HostWallClock`
- `ClockError::{BeforeUnixEpoch, NanosecondOverflow}`

## 5. Guards & Typed Refusals

`ClockError` is the runtime boundary's local error; caller adapters map it to their existing typed refusal.
The adapter does not clamp, substitute zero, return a cached reading or assert a quality bound on failure.

## 6. Proof Obligations

Known-answer tests cover epoch zero, pre-epoch refusal, exact seconds/subsecond conversion, `i64::MAX`, its
immediate successor, and the largest `Duration`. An injected fixed clock demonstrates deterministic consumers
and unchanged error propagation without reading the host clock.

## 7. Failure Modes & Deterministic Fallbacks

Invalid readings return an error. An unavailable trusted time remains unavailable to the caller's protocol;
the runtime does not fabricate a fallback. The conversion is deterministic; the host clock reading is external.

## 8. Gates & Kill Criteria

The existing capability-ownership check keeps host-clock access in this runtime owner. No policy exemption is
introduced. `cargo test -p fmine-runtime` verifies the bounded conversion and injection seam.

## 9. Lifted Code & Foundation Dependencies

Standard library only; no external or workspace dependencies.

## 10. Non-Goals & Scope Boundary

This slice does not introduce an async executor, tasks, timers, synchronization service, NTP client, authority
ledger, capability issuer or time-quality validator. The planned runtime crate receives those capabilities only
with their own working slices; the sole current consumer is the explicit wall-clock boundary.
