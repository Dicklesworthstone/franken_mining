# CONTRACT.md — `fmine-runtime`

## 1. Purpose
The runtime owns the host wall clock and the subprocess pipes. Today that is all it holds:
- a `WallClock` trait, so callers take the clock as an injected capability;
- the `HostWallClock` adapter, which reads `SystemTime::now()` and converts it exactly to signed Unix nanoseconds;
- `process::output_with_input` (BD-076), the synchronous subprocess pipe workers that offline timestamp verification uses. It sends an exact input snapshot while it drains stdout and stderr at the same time. Each captured stream is bounded by a caller-chosen byte limit, plus one byte to detect overflow. All workers are scoped and joined, and a failure kills and reaps the child. This gives a structured process lifetime, not a general executor.

A reading claims no synchronisation, uncertainty bound, monotonicity or authority. The caller decides whether to trust it, under its own policy.

- **Crate:** `fmine-runtime`
- **Layer:** S1
- **Subsystem:** Bedrock
- **LOC budget:** 12,000 planned / 14,000 ceiling
- **Allowed Module Families:** `["clock", "process"]`

## 2. Inputs
- **The host clock**, through `HostWallClock::now_ns`.
- **A caller's `Command`, input bytes and output limit**, through `process::output_with_input`. The caller chooses the executable and the environment.
- **Nothing else.** It has no workspace or external dependencies, and reads no files or registries.

## 3. Outputs
- **`WallClock::now_ns() -> Result<i64, ClockError>`**: nanoseconds since the Unix epoch, in 0 to `i64::MAX` inclusive.
- **`process::output_with_input(&mut Command, &[u8], limit) -> Result<PipedOutput, ProcessError>`**. It replaces stdin, stdout and stderr with owned pipes. `PipedOutput` keeps the exit status and both byte streams. A separate `input_error` lets a consumer tell an early stdin closure from a fully consumed input. Success needs both a successful status and no input error. The one caller is `fmine-io`'s `forecast::timestamp`, which runs OpenSSL directly.
- **No Claim and no trusted time.** The one clock caller, `fmine-cli`'s `reliance` module (BD-347), maps a `ClockError` to `FMINE-REF-506`. It builds a `TrustedTime` from the reading only when its access policy sets `trust_host_clock`, and takes the uncertainty from its own header's `max_clock_skew_ns`.

## 4. Invariants
- **Exact or refused, never wrapped or clamped.** `nanosecond_conversion_is_exact_and_never_wraps_or_clamps` checks 1.234567890 s, `i64::MAX` ns, `i64::MAX + 1` ns and `Duration::MAX`.
- **The epoch is 0, and earlier is refused.** `unix_epoch_is_zero_and_pre_epoch_is_refused`.
- **An injected clock passes its reading or error through unchanged**, with no host clock: `injected_clock_keeps_its_reading_or_failure_without_a_host_clock_dependency`.
- **No fallback.** A failed reading is an error. There is no cached value, no zero and no substitute clock.
- **Pipes never deadlock and outputs are exact.** `full_pipes_preserve_exact_input_and_both_outputs` sends binary input larger than the pipe capacity while the child fills both output pipes.
- **A limit stops the child and joins every worker.** `either_output_limit_stops_and_joins_a_blocked_input_writer` hits each output's bound while the input writer is blocked. Overflow never leaves a writer waiting on a child no one owns.
- **An early stdin closure hides nothing.** `nonzero_status_and_stderr_survive_early_stdin_closure` and `closed_stdin_does_not_cancel_later_diagnostics_or_exit_status`: both streams and the real exit status survive, zero or not, and the input error stays explicit.
- **Only this crate and `fmine-lab` may read ambient time or start a detached worker.** `xtask check-caps` bans `SystemTime::now`, `Instant::now`, `thread::spawn`, `thread::Builder` and `thread::sleep` in every other crate's production code (INT-AS-001 criterion 9). It also bans `open_child_region` outside this crate. Seeded negatives: `test_seeded_detached_worker_or_timer_fails_outside_the_runtime` and `test_seeded_open_child_region_fails`, in `crates/xtask/src/lib.rs`.

## 5. Refusals
- **`ClockError::{BeforeUnixEpoch, NanosecondOverflow}`**, a local error with no refusal code. Callers map it to their own refusal. `fmine-cli` uses `FMINE-REF-506`.
- **`ProcessError::{Spawn, Worker, Read, OutputLimit, Wait, MissingPipe, WorkerPanicked, InvalidLimit}`**, also local. It separates failures to start the process or a worker, output reads, byte limits, waits and worker panics. Consumers keep their own refusal mapping.

## 6. Determinism class
`src/lib.rs` has no `//! Determinism:` header. The conversion is exact integer arithmetic. The reading is external, so `HostWallClock` gives a different value on every call by design. A deterministic consumer injects a fixed `WallClock`. A subprocess result depends on the executable and environment the caller supplies. No timeout and no host-clock sample is added.

## 7. Evidence class of outputs
Not applicable: it emits no Claim. A reading is not evidence of the true time, and a subprocess's output is not endorsed here.

## 8. Validity domain
- **The range is 1970-01-01T00:00:00Z to about 2262-04-11.** Past that, `i64` nanoseconds overflow, which refuses.
- **Rollback is not hidden.** Successive readings may go backwards, and the adapter neither detects nor corrects that.
- **No quality.** Skew, leap-second handling and synchronisation are whatever the host gives.
- **Direct children only.** Every worker and direct child is joined and reaped before return. Managing a descendant process tree is out of scope; the timestamp consumer runs OpenSSL directly.

## 9. Oracles
No fixture. The clock tests are known answers at the conversion's edges: the epoch, one second before it, an exact sub-second value, `i64::MAX` and its successor, and `Duration::MAX`. The process tests run real children against pipe capacity and the output bounds (Invariants).

## 10. Known gaps
- **Almost all of the planned runtime is absent.** The registry charter and plan §28.3 list a region tree, CapSet profiles, an AppSpec per role, budgets and an obligation registry. None exists.
- **No asupersync.** The workspace pins it as the sole async runtime (`asupersync_pin = "=0.5.0"`), but it is not in `Cargo.lock`, so `check-all` reports INT-AS-001a as `NOT_APPLICABLE`.
- **Bedrock obligations that name the runtime are not discharged here:** PO-BED-5 (obligation closure on region close) and PO-BED-8 (K/E crates never obtain IO or REMOTE). `check-caps` is the static half of PO-BED-8 only.
- **No clock-quality validator, synchronisation service or authority ledger.** A trusted time comes only from the caller's own policy.
- **No `clock` module.** The listed family `clock` is the code in `src/lib.rs`.
- **No async executor, detached tasks, timers or NTP client.** The subprocess helper does not choose a command, endorse its output, authenticate a timestamp or interpret a shell. Its pipe workers live only for the call.

## 11. Public API
- `trait WallClock { fn now_ns(&self) -> Result<i64, ClockError>; }`
- `HostWallClock` (`Debug, Clone, Copy, Default`), which implements `WallClock`.
- `ClockError::{BeforeUnixEpoch, NanosecondOverflow}`, with `Display` and `Error`.
- `process::{output_with_input, PipedOutput, ProcessError}`.

## 12. Proof obligations
- None is discharged here. PO-BED-5 and PO-BED-8 wait on the region runtime (Known gaps).
- The crate's own properties are tested: an exact, unclamped conversion with the pre-epoch and overflow refusals, and bounded, joined subprocess pipes (Invariants).
