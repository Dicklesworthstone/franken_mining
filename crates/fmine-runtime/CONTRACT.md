# CONTRACT.md — `fmine-runtime`

## 1. Purpose
The runtime owns the host wall clock, the subprocess pipes and the capability model of the region tree. Today it holds:
- a `WallClock` trait, so callers take the clock as an injected capability;
- the `HostWallClock` adapter, which reads `SystemTime::now()` and converts it exactly to signed Unix nanoseconds;
- `process::output_with_input` (BD-076), the synchronous subprocess pipe workers that offline timestamp verification uses. It sends an exact input snapshot while it drains stdout and stderr at the same time. Each captured stream is bounded by a caller-chosen byte limit, plus one byte to detect overflow. All workers are scoped and joined, and a failure kills and reaps the child. This gives a structured process lifetime, not a general executor;
- `region` (BD-077), the plan §26.6 region tree and its capability sets as types. `CapSet<SPAWN, TIME, RANDOM, IO, REMOTE>` is a type, and a context `Cx<C>` only narrows: `open::<R>()` and `narrow::<D>()` compile only when the new set is within the old one. The clock and the subprocess pipes are reachable through a context only when it holds TIME (`Cx::clock`) or IO (`Cx::run`). Each of the 18 regions is a type, and its set is checked against its parent's when the crate compiles. `REGIONS` records each region's parent, set, supervisor, instances and role, and `profile(role)` gives the tree a node of that role runs.
- `budget` (BD-078), the plan §26.7 budget table and a meter that spends a budget. `Budget{deadline_ns, poll_quota, cost_quota, priority}` composes by `meet`: the tighter of each limit and the higher priority. `budget_table(&Params)` gives the five rows (ledger writer, telemetry, estimation run, peer session, finance bridge), with what the plan leaves to the run passed in as `Params`. A `Meter` takes every time from its caller, refuses a poll or charge past a limit without recording it, and then stays exhausted, reporting the row's `OnExhaust`.
- `supervise` (BD-079), one `AppSpec` per role (the role's `profile`), compiled to a `CompiledSupervisor` that decides restarts and escalations. The caller reports starts and failures with its own times. A child's own failures restart it with its group under its parent's strategy, up to 5 in any 60 s. The sync region's one-for-all is per peer (§26.6). The sixth escalates. An escalated peer session stops with its transfers, the peer is marked stale and the rest of the node runs on. An escalated ledger writer stops every writer and turns the node read-only: writes refuse with exit 4, and no writer starts again in the process. Any other escalation fails the parent in turn, and one that reaches the process root shuts the node down.

A reading claims no synchronisation, uncertainty bound, monotonicity or authority. The caller decides whether to trust it, under its own policy.

- **Crate:** `fmine-runtime`
- **Layer:** S1
- **Subsystem:** Bedrock
- **LOC budget:** 12,000 planned / 14,000 ceiling
- **Allowed Module Families:** `["budget", "clock", "process", "region", "supervise"]`

## 2. Inputs
- **The host clock**, through `HostWallClock::now_ns`.
- **A caller's `Command`, input bytes and output limit**, through `process::output_with_input`. The caller chooses the executable and the environment.
- **A run's budget parameters and a meter's times and costs**, through `budget_table(&Params)`, `Meter::poll(now_ns)` and `Meter::charge(cost, now_ns)`. The meter reads no clock.
- **A node's starts and failures with their times**, through `CompiledSupervisor::start(child, now_ns)` and `fail(&child, now_ns)`. The supervisor reads no clock and starts nothing.
- **Nothing else.** It has no workspace or external dependencies, and reads no files or registries.

## 3. Outputs
- **`WallClock::now_ns() -> Result<i64, ClockError>`**: nanoseconds since the Unix epoch, in 0 to `i64::MAX` inclusive.
- **`process::output_with_input(&mut Command, &[u8], limit) -> Result<PipedOutput, ProcessError>`**. It replaces stdin, stdout and stderr with owned pipes. `PipedOutput` keeps the exit status and both byte streams. A separate `input_error` lets a consumer tell an early stdin closure from a fully consumed input. Success needs both a successful status and no input error. The one caller is `fmine-io`'s `forecast::timestamp`, which runs OpenSSL directly.
- **`region`:**
  - `Cx::ambient_root() -> Cx<CapSet<true, true, true, true, true>>`, the one source of every capability;
  - `Cx::open::<R: Region>()`, `Cx::narrow::<D>()`, `Cx::caps() -> Mask` and `Cx::region() -> RegionKind`;
  - `Cx::clock() -> HostWallClock` where the set holds TIME, and `Cx::run(..)`, `output_with_input` behind IO;
  - `REGIONS: &[RegionSpec]` and `profile(Role) -> Vec<RegionSpec>`.

  A leaf region's set is the plan's row. An interior region (process, node, authority, ingest, compute, sync), for which the plan gives no set, holds the union of its children's: the least that lets every child narrow from it. Only the process root holds RANDOM, because no row grants it and engines get seeds as inputs.
- **`budget`:**
  - `budget_table(&Params) -> Result<[BudgetRow; 5], BudgetError>`. Each row is a region, a `Budget`, a `CostUnit` where it has a cost quota, and an `OnExhaust`;
  - `Budget::meet` and `Budget::UNLIMITED`, its identity;
  - `BudgetRow::meter(parent, now_ns)`, a `Meter` for the row's budget met with its parent's;
  - `Meter::poll` and `Meter::charge`, each `Result<(), Stop>`, and `Meter::spent() -> (polls, cost)`;
  - `OnExhaust::exit_code()`: 6 for the ledger writer's refused batch and 9 for the finance bridge's indeterminate post (§30.1). Sealing early, cancelling and draining carry on inside the region, so they have no exit code.
- **`supervise`:**
  - `AppSpec::for_role(role)` and `AppSpec::compile() -> Result<CompiledSupervisor, SpecError>`;
  - `CompiledSupervisor::fail(&child, now_ns) -> Result<Outcome, Refused>`. The `Outcome` is one of: `Restart{failed, restarted}`, in start order and with every running descendant; `PeerStale{peer, stopped}`; `ReadOnly{cell, stopped}`; or `Shutdown`;
  - `CompiledSupervisor::start(child, now_ns)`, `running()`, `is_stale(peer)` and `writes()`. `writes()` refuses with `Refused::ReadOnly`, exit 4, on a read-only node.
- **No Claim and no trusted time.** The one clock caller, `fmine-cli`'s `reliance` module (BD-347), maps a `ClockError` to `FMINE-REF-506`. It builds a `TrustedTime` from the reading only when its access policy sets `trust_host_clock`, and takes the uncertainty from its own header's `max_clock_skew_ns`.

## 4. Invariants
- **Exact or refused, never wrapped or clamped.** `nanosecond_conversion_is_exact_and_never_wraps_or_clamps` checks 1.234567890 s, `i64::MAX` ns, `i64::MAX + 1` ns and `Duration::MAX`.
- **The epoch is 0, and earlier is refused.** `unix_epoch_is_zero_and_pre_epoch_is_refused`.
- **An injected clock passes its reading or error through unchanged**, with no host clock: `injected_clock_keeps_its_reading_or_failure_without_a_host_clock_dependency`.
- **No fallback.** A failed reading is an error. There is no cached value, no zero and no substitute clock.
- **Pipes never deadlock and outputs are exact.** `full_pipes_preserve_exact_input_and_both_outputs` sends binary input larger than the pipe capacity while the child fills both output pipes.
- **A limit stops the child and joins every worker.** `either_output_limit_stops_and_joins_a_blocked_input_writer` hits each output's bound while the input writer is blocked. Overflow never leaves a writer waiting on a child no one owns.
- **An early stdin closure hides nothing.** `nonzero_status_and_stderr_survive_early_stdin_closure` and `closed_stdin_does_not_cancel_later_diagnostics_or_exit_status`: both streams and the real exit status survive, zero or not, and the input error stays explicit.
- **Only this crate and `fmine-lab` may read ambient time or start a detached worker.** `xtask check-caps` bans `SystemTime::now`, `Instant::now`, `thread::spawn`, `thread::Builder` and `thread::sleep` in every other crate's production code (INT-AS-001 criterion 9). It also bans `open_child_region` and `ambient_root` outside this crate. Seeded negatives: `test_seeded_detached_worker_or_timer_fails_outside_the_runtime`, `test_seeded_open_child_region_fails` and `test_seeded_ambient_root_fails_outside_the_runtime`, in `crates/xtask/src/lib.rs`.
- **A context never widens (BD-077).** Four `compile_fail,E0277` doctests in `src/region.rs`, error code checked on the pinned nightly:
  - a law region cannot open the projection region;
  - the object store's context cannot be narrowed up to TIME;
  - an estimation run has no `clock()`;
  - the effect region has no `run()`.

  The module doctest is the positive control: the same calls with a set that is within the parent's compile and run. The parent edges are compile-time assertions, one `const` per region.
- **The table is the plan's tree.** `region::tests::the_table_is_the_plans_tree` parses the §26.6 code block from the plan file. It checks each region's name, order, parent (from the drawing's indentation), leaf set, supervisor, instances and role (inherited down the tree) against `REGIONS`. It also checks that the plan draws a set on every leaf and on no interior region.
- **Derived rules.**
  - `an_interior_set_is_the_union_of_its_childrens_and_the_root_holds_all`.
  - `random_stays_at_the_root_and_remote_on_the_sync_path`: RANDOM is held by the process root alone. REMOTE is held by the process, node, sync, peer-session and transfer regions only.
  - `engines_get_no_time_io_or_remote` (PO-BED-8, compile-time half): the law region holds nothing and an estimation run holds SPAWN alone.
  - `each_role_runs_a_whole_tree`: every region in a profile has its parent there. Ingest and its children run on site nodes only, and the finance bridge on enterprise nodes only.
  - `every_row_is_its_variant_and_its_type` and `mask_within_and_union_follow_the_bits` (all 1,024 pairs).
- **Planted bugs, each reverted:**
  - Transfer's REMOTE set to 0 fails the plan-tree and REMOTE tests.
  - Law given IO under a compute region without it fails to compile (E0277 at the edge `const`).
  - A rule letting a child gain a capability makes the `open` and `narrow` compile-fail doctests compile, so they fail.
- **The budget table is the plan's table (BD-078).** `budget::tests::the_table_is_the_plans_table` parses the §26.7 table from the plan file. It checks each row's region and order, deadline, cost quota and unit, priority and on-exhaust action, and that no row has a poll quota. Fixed durations and sizes are read from the cell text, and the per-run cells are checked against distinct test parameters. The exit codes are read from the cell (`exit 6`), and for the indeterminate post from §30.1's table (9).
- **Each region exhausts as its row says**, on virtual time, one test per row:
  - the ledger writer refuses at 250 ms (exit 6);
  - telemetry seals early past 64 MiB or at 1 s;
  - an estimation run cancels past blocks × realizations, or at the tighter of its own deadline and its parent's;
  - a peer session drains at its window's bytes or end, and the next window's meter starts at zero;
  - a finance post is indeterminate at 120 s against a debug `fa` and at 10 s otherwise (exit 9).

  Each test checks the last allowed poll or charge, one nanosecond or one unit before the limit.
- **Refused, not clamped.** A charge past the quota is refused whole and `spent()` does not change. Exhaustion is sticky: a later free charge, or a time before the one that exhausted it, gets the same answer. A time before the start (`BeforeStart`) and a running cost past `u64` (`Overflow`) are refused without spending or exhausting. A zero parameter, and blocks × realizations past `u64`, are refused by `budget_table`.
- **Meet laws.** `meet_is_a_semilattice_with_unlimited_as_its_identity`: commutative and idempotent over 192 budgets (each pair), associative over 28 (each triple), `UNLIMITED` is its identity, each limit is one of its operands' and no looser than either, and the priority is the higher.
- **Planted bugs, all four at once, reverted:** a deadline reached only past it (`>`), a cost quota refused when met (`>=`), `meet` keeping the lower priority, and telemetry's priority 181. Eight of the ten budget tests failed, and every plant failed at least one.
- **Escalation lab tests (BD-079), on virtual time, from a full site tree:**
  - `the_intensity_is_the_plans_and_the_sixth_failure_in_60_s_escalates`. It reads "Restart intensity is 5 restarts per 60 s per child" from §26.7. Five failures of an estimation run restart it, and the sixth fails the compute region, which the node restarts with everything after it. That restart clears the run's history. At exactly 60 s the oldest failure has left the window.
  - `strategies_restart_the_failed_child_its_peer_group_or_the_rest`: one-for-one in ingest and compute; rest-for-one in authority and the node, in start order with descendants; one-for-all per peer in sync.
  - `an_escalated_peer_goes_stale_and_local_authority_keeps_running`: the session and its transfers stop and the peer is stale. Writers, the other peer and writes run on. A transfer cannot start over the stale peer, and a new session clears the mark with a fresh history.
  - `an_escalated_ledger_writer_turns_the_node_read_only_and_is_never_restarted`: every writer stops, and writes refuse with the code that §30.1's "writer escalation" row gives (4). No writer starts, for that cell or any other, and later authority restarts bring none back.
  - `escalation_climbs_to_the_root_and_shuts_the_node_down`: telemetry fails ingest, ingest fails the node, and the node stops. Then every call refuses with `Stopped`.
  - `every_role_compiles_and_a_broken_spec_is_refused`: each `SpecError` is reached by a broken site spec.
  - `starts_are_checked_against_the_spec_and_time_never_goes_back`: the starts that are refused are a region outside the role, a key or peer that does not fit, a duplicate, a missing parent or peer session, and an earlier time.
  - `seeded_schedules_match_the_model_and_replay_the_same`: 1,000 seeds of 200 steps, each of a failure or a start. Each outcome is checked against a model of each child's restarts that is built from the outcomes alone. Every failure an escalation passes must have a full window, and a restarted child must not. After each step, the running set is a closed tree, a read-only node runs no writer, and a stale peer runs nothing. Each seed replays to the same trace. Across the seeds there were 59,555 restarts (each strategy came up), 1,141 escalated restarts, 226 stale peers, 226 read-only nodes, 956 shutdowns and 159 refused writer starts, and the test asserts that each count is nonzero.
- **Planted bugs in the supervisor, one at a time, reverted.** Each failed at least two tests, the seeded lab among them:
  - a sixth restart allowed in the window;
  - sync's group ignoring the peer;
  - read-only stopping only the escalated writer;
  - a restart keeping descendants' histories;
  - a writer allowed to start on a read-only node.

## 5. Refusals
- **`ClockError::{BeforeUnixEpoch, NanosecondOverflow}`**, a local error with no refusal code. Callers map it to their own refusal. `fmine-cli` uses `FMINE-REF-506`.
- **`ProcessError::{Spawn, Worker, Read, OutputLimit, Wait, MissingPipe, WorkerPanicked, InvalidLimit}`**, also local. It separates failures to start the process or a worker, output reads, byte limits, waits and worker panics. Consumers keep their own refusal mapping.
- **`SpecError::{Empty, RootHasParent, SecondRoot, Duplicate, MissingParent, KeyedParent, NoStrategy, WrongRole}`** and **`Refused::{NotInSpec, Shape, AlreadyRunning, ParentNotRunning, NotRunning, ReadOnly{cell}, TimeWentBack, Stopped}`**, local. `Refused::ReadOnly` is exit 4 (§30.1). The others are the caller's errors.
- **`BudgetError::{Zero(name), Overflow, BeforeStart}`** and **`Stop::{Exhausted(Exhausted{limit, on_exhaust}), Invalid(BudgetError)}`**, local. Running out of budget is `Stop::Exhausted`, an expected outcome that carries the region's action. A bad input is `Stop::Invalid`.

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
No fixture. The clock tests are known answers at the conversion's edges: the epoch, one second before it, an exact sub-second value, `i64::MAX` and its successor, and `Duration::MAX`. The process tests run real children against pipe capacity and the output bounds (Invariants). The region table's oracle is the plan's own §26.6 drawing, read from `COMPREHENSIVE_PLAN_FOR_THE_DESIGN_OF_FRANKENMINING.md` by a test. An edit to either side that the other does not share fails it. The budget table's oracle is the same file's §26.7 table, with §30.1's exit codes.

## 10. Known gaps
- **Most of the planned runtime is absent.** The registry charter and plan §28.3 list a region tree, CapSet profiles, an AppSpec per role, budgets and an obligation registry. The tree and the profiles exist as types (BD-077). The budget table and its meter (BD-078) and the AppSpec and its supervisor (BD-079) exist as decision procedures. Nothing runs, restarts or cancels a region yet, and there is no obligation registry (BD-089). A meter reports an `OnExhaust`, and a supervisor reports an `Outcome`. Carrying them out is the caller's job: refusing the batch, sealing, cancelling, draining, reconciling, restarting, stopping, or refusing writes.
- **Supervision rules the plan does not state.** These are this crate's reading:
  - A child's intensity counts its own failures only, not restarts it shares with a failed sibling.
  - A restart clears its descendants' histories, because they are fresh instances.
  - An escalated writer stops every writer, because the node refuses all writes.
  - A transfer belongs to the peer whose session carries it.
  - The process root has no strategy, so a failure of the node itself shuts it down.
  - Read-only mode lasts for the process. The plan's way out (`fmine doctor` verifies the hash chain, then a new process starts) is not here.
  - The lab is this crate's own seeded schedule, not asupersync's LabRuntime, so there is no DPOR or crash-point chaos.
- **Budget rules the plan does not state.** §26.7 says budgets compose by `meet` but not what that does to priority. Here it takes the higher, so a child never runs below its parent. The plan gives no poll quota, so every row's is `None`. No scheduler reads priority yet. Only the five §26.7 regions have rows; the others run on their parent's budget.
- **The capability model binds only code that uses it.** `HostWallClock` and `process::output_with_input` are still public, and `fmine-cli` and `fmine-io` call them directly. `Cx::clock` and `Cx::run` are the capability paths, not the only paths. Moving those callers onto a `Cx` is later work. The `ambient_root` ban is a token check, like `open_child_region`'s. It cannot be dodged by renaming, because an associated function cannot be aliased with `use`.
- **The plan's §26.6 text disagrees with its own rows on REMOTE.** The peer-session line says "the ONLY REMOTE-capable region besides FinanceBridge". But the finance bridge's row is `CapSet<0,1,0,1,0>` ("local socket only"), and the transfer region's is `CapSet<0,1,0,1,1>` ("CAS chunks over QUIC/ATP"). The code follows the rows, so REMOTE goes to the peer-session and transfer regions. Which is right is the owner's call.
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
- `region::{CapSet, Mask, Caps, Within, Has, Spawn, Time, Random, Io, Remote}`. The narrowing rule's `rule::{Bit, Allowed}` is public but hidden, because `Within`'s impl names it.
- `region::{Region, RegionKind, RegionSpec, Supervisor, Instances, Role, REGIONS, profile}`, and one unit type per region (`ProcessRegion` … `OperationsRegion`).
- `region::Cx` with `ambient_root`, `open`, `narrow`, `caps`, `region`, `clock` and `run`.
- `budget::{Budget, BudgetRow, CostUnit, OnExhaust, FaBuild, Params, BudgetError, Limit, Exhausted, Stop, Meter, budget_table}`.
- `supervise::{AppSpec, CompiledSupervisor, Child, Outcome, Refused, SpecError, RESTARTS, WINDOW_NS}`.

## 12. Proof obligations
- None is discharged here. PO-BED-5 waits on the region runtime. Of PO-BED-8, this crate holds the compile-time half: engine regions' sets are empty or SPAWN alone, and a context cannot widen. The runtime mask test is BD-097's (Known gaps).
- The crate's own properties are tested: an exact, unclamped conversion with the pre-epoch and overflow refusals, and bounded, joined subprocess pipes (Invariants).
