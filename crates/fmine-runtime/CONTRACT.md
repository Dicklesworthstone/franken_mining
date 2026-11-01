# CONTRACT.md — `fmine-runtime`

## 1. Charter & Layer

- **Crate:** `fmine-runtime`
- **Layer:** `S1` (Runtime)
- **Subsystem:** Bedrock
- **LOC Budget:** 12k planned / 14k ceiling (`registries/loc_budget.toml`)
- **Allowed Module Families:** `["clock", "process"]`

## 2. Mathematical & Domain Formulation

DESIGN: the first runtime slice exposes an injected `WallClock` and the explicit `HostWallClock` adapter.
It converts elapsed time since the Unix epoch to nonnegative signed 64-bit nanoseconds using checked conversion.
The admitted numeric range is 0 through `i64::MAX`, inclusive. Pre-epoch and overflowing times are errors.

DESIGN (BD-076): `process` owns synchronous subprocess pipe workers used by offline timestamp verification.
It sends an exact input snapshot while draining stdout and stderr concurrently. Each captured stream is
bounded by a caller-selected byte limit plus one overflow-detection byte. All workers are scoped and joined;
failure kills and reaps the child. This supplies structured process lifetime, not a general executor.

## 3. Authority, Epistemics & Representation

A host reading does not establish synchronization, uncertainty, monotonicity or external timestamp authority.
The caller must separately supply any owner-declared quality policy and decide whether the reading qualifies.
The adapter never makes a historical journal timestamp trusted and never corrects or conceals clock rollback.

## 4. Public API Surface

- `WallClock::now_ns() -> Result<i64, ClockError>`
- `HostWallClock`
- `ClockError::{BeforeUnixEpoch, NanosecondOverflow}`
- `process::output_with_input(&mut Command, &[u8], limit) -> Result<PipedOutput, ProcessError>` overrides
  stdin/stdout/stderr with owned pipes. Output preserves the exit status and both byte streams; a separate
  input error lets consumers distinguish early stdin closure from a successfully consumed input snapshot.

## 5. Guards & Typed Refusals

`ClockError` is the runtime boundary's local error; caller adapters map it to their existing typed refusal.
The adapter does not clamp, substitute zero, return a cached reading or assert a quality bound on failure.
`ProcessError` distinguishes process/worker startup, output reads, byte limits, wait failures and worker
panics. Consumers retain their domain-specific refusal mapping. A failed input write remains explicit in
`PipedOutput` while the child completes its bounded outputs and actual exit status; it does not cancel
legitimate diagnostics after stdin closure. Success requires both a successful status and no input error.

## 6. Proof Obligations

Known-answer tests cover epoch zero, pre-epoch refusal, exact seconds/subsecond conversion, `i64::MAX`, its
immediate successor, and the largest `Duration`. An injected fixed clock demonstrates deterministic consumers
and unchanged error propagation without reading the host clock.
Process tests send a binary input larger than pipe capacity while the child fills both output pipes, verify
exact capture, exercise each output bound with a blocked input writer, and retain nonzero status/stderr when
the child closes stdin early. A delayed response after explicit stdin closure retains both streams and
either a zero or nonzero exit without hiding the input error. Overflow does not leave a writer waiting on
an unowned child.

## 7. Failure Modes & Deterministic Fallbacks

Invalid readings return an error. An unavailable trusted time remains unavailable to the caller's protocol;
the runtime does not fabricate a fallback. The conversion is deterministic; the host clock reading is external.
Subprocess results depend on the explicitly supplied executable and environment. No timeout or host-clock
sample is introduced. Every worker and direct child is joined/reaped before return; descendant process-tree
management is outside this narrow adapter, whose timestamp consumer invokes OpenSSL directly.

## 8. Gates & Kill Criteria

The existing capability-ownership check keeps host-clock access in this runtime owner. No policy exemption is
introduced. `cargo test -p fmine-runtime` verifies the bounded conversion and injection seam.

## 9. Lifted Code & Foundation Dependencies

Standard library only; no external or workspace dependencies.

## 10. Non-Goals & Scope Boundary

This slice does not introduce an async executor, detached tasks, timers, synchronization service, NTP client,
authority ledger, capability issuer or time-quality validator. The subprocess helper does not select a command,
endorse its output, authenticate a timestamp or introduce shell interpretation. Its pipe workers have only the
lexical lifetime of the call. The planned runtime crate receives further capabilities with their own working slices.
