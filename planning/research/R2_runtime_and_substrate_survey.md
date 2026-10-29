# R2 — Runtime and Substrate Survey for FrankenMining (`franken_mining`)

| Field | Value |
|---|---|
| Research track | R2: asupersync runtime and Franken-suite substrate |
| Inspection date | 2026-10-28 (all repos read at their local `HEAD` on this date) |
| Method | Read-only inspection of README / AGENTS.md / Cargo manifests / `lib.rs` docs / status files / plans; `git log`; crude `wc -l` and `#[test]` greps. No surveyed repository was modified. |
| Consumers studied | `franken_surveillance_system` (FSS) plan Draft 0.4, `franken_accounting` (FA) plan + `franken-stack.lock`, `franken_textiles` (FTX) plan |
| Evidence labels | Same as FSS's document control: **FACT** (checked by direct inspection on the date above), **DESIGN** (proposed choice for FrankenMining), **HYPOTHESIS**, **TARGET**, **OPEN**. Numbers labelled "crude" come from `wc -l` / `grep -c '#[test]'` over `*.rs`, including inline tests and generated code. They indicate scale only. |

---

## 0. Key findings

1. **FACT: the suite as built shares one asupersync revision, which is not the newest one.** Asupersync's `main` is `0.6.0`, and its CHANGELOG records the 0.6.0 release on 2026-10-16, published through DSR. The local clone has tags only up to `v0.5.0`. Every downstream consumer inspected pins **0.5.0**:
   - FrankenGraphDB and FrankenAccounting pin git rev `78b64636e99fea4ea2d868096576021dd3b8e519`.
   - FSS, fastmcp_rust, frankensearch, frankensqlite, mcp_agent_mail_rust, eidetic_engine_cli and frankenfs pin `=0.5.0` or `0.5.0` from the registry.
   - franken_networkx accepts `>=0.3.4,<0.6`.
   - frankensim uses a mutable path dependency, `../asupersync`.

   FrankenMining has to join one of two version universes, and that choice is the first architecture decision record (ADR) the plan needs.
2. **FACT: FrankenAccounting is the only sibling that consumes the substrate as code.**
   - FA: `asupersync::` appears in 274 files and `fgdb::` in 220. Pins are recorded in `franken-stack.lock`, and a `[patch.crates-io]` entry keeps a single `Cx` type in the build graph.
   - FSS: the plan is the most elaborate constitution in the suite, but the code reaches asupersync in only 5 places (`fss-runtime/src/authority.rs`). Its `IMPLEMENTATION_STATUS.md` (2026-10-14) says device and alert I/O use blocking `std::net` "rather than Asupersync (owner decision `fss-x4a.8.1` is open)". It also lists "Persistent FrankenSQLite/FrankenFS/ATP integration" under "Work packages still materially open". FSS imports **mechanism designs**, not crates.
   - FTX: depends on no suite crate. It says "Sibling Franken projects are resources, not assumptions" (FTX §16) and bans Tokio because "nothing here is async".
3. **FACT: FrankenGraphDB is a serious codebase, but it is unreleased, and several capabilities a mining operating system needs are missing.**
   - Size: 31 crates, about 12,087 `#[test]` functions (crude), version `0.0.1` with `publish = false`, no tags.
   - Live today: durable Chronicle commit stream; `FOR SYSTEM_TIME AS OF SEQ`; first-committer-wins write transactions over point-read witnesses; certified, replayable native reads; `CALL fnx.*`; bounded GQL.
   - Planned: valid-time contracts, branch fork/merge, Ripple materialized views, Graph-SSI, durable range indexes, HLC wall-clock commit instants, a durable catalog, and group commit.
   - Rejected for 1.0: **a spatial index.** `Point` values are "not orderable and not indexable" (fgdb plan §3, rejection (k)).
4. **FACT: FrankenSQLite is the mature, released store.** It is at `v0.4.12` with about 27k `#[test]` and is the persistence layer for beads_rust, Agent Mail, Eidetic, frankengit, frankensearch-storage and frankensim's `fs-ledger`. Its limits:
   - Only compatibility mode over ordinary SQLite files is live. The native ECS/RaptorQ mode is design plus partial implementation.
   - `FOR SYSTEM_TIME AS OF` works for `:memory:` databases only.
   - The write-merge ladder is dormant code.
   - It has **R-tree/geopoly, FTS5 and JSON1** live.
5. **FACT: five FSS Part IX concepts are FSS-level designs, assembled from mechanisms that live in other repos.** These are one version universe, semantic transactions, decision cards, certified graph intelligence and the ATP archive.
   - ATP: Asupersync Transfer Protocol, implemented in `asupersync/src/atp`.
   - Decision cards: the `franken_decision` and `franken_evidence` crates inside asupersync.
   - Certified graph results: CGSE in franken_networkx, plus `execute_certified` in fgdb.
   - One version universe: Chronicle in fgdb.
   - DSR: the separate `doodlestein_self_releaser` repo, a bash tool built on nektos/act.

   FSS implements its own versions in `fss-*` crates. Examples: `EvidenceDeltaBatch` appears in 48 files, `GraphAlgorithmWitness` in 21, and `DecisionCard` in only 2.
6. **DESIGN (headline recommendation):**
   - Build FrankenMining in the FrankenAccounting manner, linking the substrate as code with exact pins, rather than the FSS manner of imitating it.
   - Keep the FSS doctrine of a single FrankenMining-owned canonical ledger contract with derived projections.
   - Admit each foundation (FrankenGraphDB, FrankenSQLite, frankensearch, fnx, fastmcp, ftui) per bounded context, behind an explicit `INT-*` admission gate and an Appendix-H-style foundation dependency ledger.
   - Treat FrankenAccounting as a **sibling product**, not a library, for the finance and accounting domain.

---

## 1. Snapshot table (FACT, 2026-10-28)

| Repo | Purpose | Version | HEAD (`%h %ad %s`) | Commits since | Crates | Crude `.rs` lines / `#[test]` | Release status | asupersync pin |
|---|---|---|---|---|---|---|---|---|
| `asupersync` | Async runtime / operating system of the suite | `0.6.0` (workspace) | `523e86c00 2026-10-28 fix(release): bind publication to tagged source and gate TLS regression (#86)` | 24,993 since 2026-01-16 | 10 workspace members + wasm/fuzz | `src/` ≈2.58M lines; ≈52,188 `#[test]` (src+tests) | Released on crates.io / GitHub via DSR; v0.6.0 dated 2026-10-16 in CHANGELOG; local tags end at `v0.5.0` | — |
| `frankengraphdb` | Property-graph DB (GQL), bitemporal by design (system time live, valid time planned) | `0.0.1`, `publish = false` | `f46f53d08 2026-10-28 fix(gql): return the caller's control refusal without cloning it` | 5,911 since 2026-07-15 | 31 | ≈1.36M / ≈12,087 | **Unreleased**, no tags | git rev `78b64636` (=0.5.0) |
| `frankensqlite` | Ground-up Rust SQLite with page-level MVCC (`BEGIN CONCURRENT`) | `0.4.12` | `9090854e1 2026-10-28 chore(beads): bd-hga35 landed …` | 10,160 since 2026-02-25 | 28 | ≈1.96M / ≈27,254 | Released (installers, crates.io) | `0.5.0` |
| `frankensearch` | Two-tier hybrid local search (Quill BM25 + vectors + RRF + rerank); `fsfs` CLI | facade `frankensearch 0.7.1`; sub-crates `0.4.x`; `fsfs` 1.12.x | `b2c09398 2026-10-28 chore(beads): keep ranking, upgrade, and current-source probe notes open` | 5,520 since 2026-02-13 | 13 | ≈0.97M / ≈12,210 | Released | `0.5.0` |
| `franken_networkx` | NetworkX-compatible Rust graph algorithms (`fnx-*`) + PyPI wheel | ws `0.2.3`; `fnx-algorithms/classes/runtime 0.3.1` | `de58cd218 2026-10-27 merge(main): keep fail-closed mean-geodesic receipts with upstream` | 9,234 since 2026-02-13 | 12 | ≈0.30M / ≈2,260 Rust (+ ~1,100 Python parity files) | Released (PyPI, crates) | `>=0.3.4, <0.6` |
| `fastmcp_rust` | Cancel-aware MCP server/client framework | `0.13.0` | `5c140922 2026-10-28 tracker: first green on http_03_integration …` | 4,909 since 2026-01-18 | 9 | ≈1.05M / ≈12,567 | Released `v0.13.0` | `=0.5.0` (registry) |
| `frankentui` | Terminal UI kernel (`ftui`), Elm-style runtime, WASM web backend | `0.9.0` | `0f5bd244 2026-10-27 merge(origin/main): keep the parallel full-gates fix …` | 4,374 since 2026-01-31 | 20 | ≈1.15M / ≈28,864 | Released `v0.9.0` (badge "status: WIP") | `=0.5.0` (optional executor lane) |
| `mcp_agent_mail_rust` | Agent coordination mail + file reservations (MCP) | `0.3.39` | `9ff9dc12 2026-10-28 chore(beads): record integration repairs …` | 5,712 since 2026-02-05 | — | — | Released | `=0.5.0` |
| `beads_rust` | `br` issue/work graph (SQLite + JSONL) | `0.7.6` | `6678e72b 2026-10-27 ci(sync): validate receipt recovery …` | 3,696 since 2026-01-15 | single crate | — | Released | via `fsqlite =0.4.10` |
| `ntm` | Named Tmux Manager: agent-swarm control plane (**Go**) | `1.37.0` | `4073bd1b 2026-10-28 fix(tmux): wait for a slow reader when the macOS FIFO is full` | 5,536 since 2025-12-10 | Go module | — | Released | n/a |
| `frankenredis` | Clean-room Redis 7.2.4 in Rust | `0.1.2` | `79b61ed52 2026-10-26 feat(server): thread cached command gates …` | 8,781 since 2026-02-13 | 16 (`fr-*`) | ≈0.57M / ≈5,238 | Tags `v0.1.1`, `v0.1.2` | **none** (own event loop / io_uring) |
| `frankenfs` | Userspace ext4/btrfs (+FAT/NTFS/ZFS) with block MVCC + RaptorQ repair | tags to `v0.2.0` | `bc3de2c09 2026-10-27 merge(main): keep FrankenFS tracker movement with upstream` | 9,746 since 2026-02-09 | 24 (`ffs-*`) | ≈0.85M / ≈11,893 | "All profiles remain experimental" | `0.5.0` |
| `doodlestein_self_releaser` (DSR) | Local release authority (bash + nektos/act + SSH hosts, minisign, SBOM) | `dsr 0.2.3` (installed) | `98eaa55 2026-10-26 docs(readme): genericize example build-host names` | — | bash | — | Used to release asupersync 0.5.0 / 0.6.0 | n/a |
| *Consumers* `franken_accounting` | Accounting OS on fgdb | — | `53c6e953 2026-10-28` | — | 31 (`fa-*`, `fad`) | ≈0.79M / ≈4,721 | unreleased | git rev `78b64636` + `[patch.crates-io]` |
| *Consumers* `franken_surveillance_system` | Home surveillance OS | ws `0.0.1` | `8d810de7 2026-10-28` | — | 21 (`fss-*`) | ≈1.21M / ≈10,982 | pre-release reference | `=0.5.0` in `fss-runtime` only |
| *Consumers* `franken_textiles` | Garment patterning / drape | ws `0.1.0` | `4a11a6b 2026-10-28` | — | 10 (`ftx-*`) | — | — | **none** |

Toolchains (FACT): asupersync, frankengraphdb and franken_accounting pin `nightly-2026-08-31`. fastmcp_rust pins `nightly-2026-08-25`; FA records this drift as a risk row. Asupersync's default features also build on stable 1.95+. On stable, `?` on `Outcome` (`nightly-outcome-try`) is unavailable.

License (FACT): asupersync, frankengraphdb, frankensqlite, frankentui and fastmcp_rust use "MIT + OpenAI/Anthropic Rider", a restrictive custom licence. franken_networkx's README shows plain MIT, and franken_textiles declares MIT. FrankenMining's licence should be chosen with this in mind (**OPEN**).

---

## 2. Per-project fact sheets

### 2.1 asupersync: "spec-first, cancel-correct, capability-secure async runtime"

**Purpose.** Asupersync is the runtime and authority model for the whole suite. FA calls it "the operating system" (FA §2.2). It turns task ownership, cancellation, two-phase effects and capability scoping into structural properties. In the README's words: "Tasks are owned by regions that close to quiescence. Cancellation is an explicit request → drain → finalize protocol … The lab runtime makes controlled schedules deterministic and replayable."

**Exact vocabulary a plan must use (FACT, from `README.md` "Core Types Reference", "Core Exports", `src/cx/cap.rs`, `src/record/obligation.rs`).**

| Concept | Exact names |
|---|---|
| Capability context | `Cx`, `Scope<'r, P: Policy = FailFast>`, `cx.spawn(\|cx\| …)`, `cx.spawn_in(&scope, …)`, `cx.spawn_blocking`, `cx.checkpoint()` (returns `Err` once cancellation is requested), `cx.masked(…)`, `cx.trace(…)`, `cx.budget()`, `cx.is_cancel_requested()`, `cx.scope_with_budget(…)`, `cx.with_ambient(future)`, `Cx::set_current` (since 0.5 it preserves restrictions; narrowing cannot be undone by reinstalling) |
| Typed capability rows | `CapSet<SPAWN, TIME, RANDOM, IO, REMOTE>` (const-generic, zero-sized); aliases `cap::All`, `cap::None`; `SubsetOf` narrowing (widening is a compile error); sealed markers prevent forging. Missing IO capability on an I/O entry point yields `[ASUP-E009]`. |
| Identity | `RegionId`, `TaskId`, `ObligationId`, `Time` |
| Tasks | `TaskHandle<T>` (`.join(cx)` → `Result<T, JoinError>`), `JoinSet<T, E, P>` (`JoinSet::in_cx`, `join_next`, `join_all`, `cancel_all`), fibers `cx::fiber::scope` |
| Combinators / macros | `race!`, `join!`, `join_all!`, `scope!`, `spawn!`, `select!`, `cx.race_drained_with`, `Scope::timeout`, `time::timeout(now, dur, fut)`, `sleep(now, dur)`, `interval(now, dur)`; `#[main]`, `#[test]`, `#[lab_test]` |
| Outcomes | `Outcome<T, E> = Ok(T) \| Err(E) \| Cancelled(CancelReason) \| Panicked(PanicPayload)`, severity lattice `Ok < Err < Cancelled < Panicked`, `Severity`, `join_outcomes`, `OutcomeError`; HTTP mapping 200 / 4xx-5xx / **499** / 500 |
| Cancellation | `CancelReason`, `CancelKind` = `User, Timeout, Deadline, PollQuota, CostBudget, FailFast, RaceLost, ParentCancelled, ResourceUnavailable, Shutdown, LinkedExit` (cleanup budgets scale inversely with severity); states `Running → CancelRequested → Cancelling → Finalizing → Completed(Cancelled)`; `ProgressCertificate` (drain regimes `warmup, rapid_drain, slow_tail, stalled, quiescent`); `ResponsivenessRegistry` (published conditional bounds); `Error::cancel_reason()` |
| Budgets | `Budget { deadline: Option<Time>, poll_quota: u32, cost_quota: Option<u64>, priority: u8 }`, `meet` = tighter wins (semiring) |
| Obligations | `ObligationKind = SendPermit, Ack, Lease, IoOp, SemaphorePermit, Transaction`. Two-phase send `tx.reserve(&cx).await?.send(v)`; dropping a permit aborts. Invariant: "region_close requires all obligations resolved". Advanced modules: `src/obligation/{saga, ledger, leak_check, eprocess, calm, crdt, dialectica, separation_logic, session_types, no_leak_proof, lyapunov}`; `SagaPlan`; feature `obligation-leak-detection`. |
| Errors | `Error`, `ErrorKind`, `Recoverability`; error-code registry `ASUP-E###` (`docs/error_codes/`) |
| Scheduler | Three lanes: **Cancel** (priority 200–255) → **Timed** (EDF) → **Ready**; `cancel_streak_limit` default 16; optional Lyapunov governor (off by default) |
| Deterministic testing | `LabRuntime`, `LabConfig`, virtual time, seeded scheduling, trace capture/replay, DPOR with Mazurkiewicz/Foata dedup, chaos injection, `CrashPack` (`src/trace/crashpack.rs`), lab VFS, oracles (11 of 24 are fed from runtime state: task leak, obligation leak, quiescence, loser drain, finalizer, region tree, deadline monotonicity, cancellation protocol, DOWN order, supervisor restarts, GenServer replies) |
| OTP layer ("Spork") | `CompiledSupervisor` (`bind_managed`; one-for-one / one-for-all / rest-for-one), `AppSpec` (declarative topology → lab proof), actors, `gen_server`, `monitor`, `link`, registry name leases (`src/cx/registry.rs`), `spork::process_group` |
| Security | Macaroons `MacaroonToken`, `MacaroonKeyRing`, `MacaroonSignature` (`src/cx/macaroon.rs`), authenticated types, `src/security/` |
| Networking | TCP, UDP, DNS, HTTP/1.1, HTTP/2 (`http2-streaming`), native QUIC/HTTP-3 (`quic`, `http3`), WebSocket, TLS (rustls; `tls`, `tls-ring`, `tls-native-roots`, `tls-webpki-roots`), gRPC (`src/grpc`), web (`src/web`, SSE, multipart, sessions), transport router |
| Databases | Feature-gated clients: `sqlite` (rusqlite on the blocking pool; **C SQLite, not FrankenSQLite**), `postgres` (native wire v3, SCRAM), `mysql` (native wire); `kafka` (rdkafka, C) |
| Messaging | Redis, NATS, JetStream and Kafka clients today. The future native **Semantic Subject Fabric** ("FABRIC North Star", `src/messaging/mod.rs`) has its feature `messaging-fabric` reserved but not fully gated. |
| Distributed | `NativeRemoteRuntime`, TCP+mTLS V3 transport, leases, idempotency, sagas (`src/remote.rs`, `Saga`), `remote-service` static host, consistent hashing, snapshot distribution over RaptorQ symbols |
| Erasure coding | `src/raptorq/` (RFC 6330: `rfc6330.rs`, `systematic.rs`, `gf256.rs`, `linalg.rs`, `pipeline.rs` with `verify_auth`, `proof.rs` decode proofs) |
| **ATP** | **Asupersync Transfer Protocol** (`docs/atp_architecture.md`): "verified object-graph transfer". Object graph, manifests, Merkle roots, graph commits (`src/atp/{object,manifest,path}.rs`); binary frames (`src/net/atp/protocol/*`); RaptorQ and TCP transports; bonded multi-donor `atp bond-pull`; `atp` CLI (feature `atp-cli`) and **unpublished** `atpd` daemon (`atpd-daemon`); durable sender/receiver journaling and resume across process loss (v0.6.0); repair coordinator/scheduler; `atp doctor --platform` |
| Suite-wide "foundation tripod" crates (in the asupersync workspace, v0.6.0) | `franken-kernel` ("Suite-wide type substrate": `TraceId`, `DecisionId`, `PolicyId`, `SchemaVersion`, and its **own** `Cx` with a tropical-semiring `Budget` and `NoCaps`); `franken-evidence` (`EvidenceLedger`, `EvidenceLedgerBuilder`: `ts_unix_ms, component, action, posterior, expected_loss_by_action, chosen_expected_loss, calibration_score, fallback_active, top_features`; "All FrankenSuite projects import this crate — no forking allowed"); `franken-decision` (`DecisionContract`, `LossMatrix`, `Posterior`, `FallbackPolicy`, `DecisionAuditEntry`, `evaluate`, `EvalContext`); `frankenlab` ("record, replay, and minimize concurrency bugs") |
| Observability | `metrics` (OpenTelemetry, Tokio-free normal graph), `tracing-integration`, `runtime-metrics`, deadline monitor, task inspector, `debug-server`, console |
| WASM | `wasm-browser-{minimal,dev,prod,deterministic}`, `asupersync-browser-core`, `asupersync-wasm` |

**Dependency doctrine (FACT, AGENTS.md "Async Runtime: THIS IS IT (NO TOKIO)").** The runtime crate has no transitive Tokio dependency, and `cargo tree -e normal -p asupersync -i tokio` must print nothing. The only carve-outs are `asupersync-tokio-compat` and the conformance crate. Core dependencies are `thiserror`, `crossbeam-queue`, `parking_lot`, `polling`, `slab`, `smallvec`, `pin-project`, `serde` + `serde_json` and `socket2`; `rustls` is optional. Asupersync is therefore not "zero third-party". FA and FSS both register serde as a confined transitive exception.

**Maturity statements (FACT).**
- README: "These guarantees are enforced by the runtime, not by the type system."
- Cleanup budgets are "advisory on the production runtime".
- "A task that never reaches [a cancellation point] is never forcibly stopped."
- Most public I/O entry points (`TcpStream::connect`, `File::open`) **take no `Cx` parameter**. They check the calling task's ambient context, and `spawn_blocking` "still runs its closure under a restricted context" (tracked as `asupersync-issue65-criticisms-kpmoy5.5`).
- 13 of 24 lab oracles are not yet fed from runtime state (`asupersync-52hxjz`).
- The region heap has no public allocation API (`asupersync-bi2462.39`).
- The sharded-state lock order is "a convention, not a checked one" in production.
- ATP over RaptorQ is fail-closed on symbol authentication, but "the TCP control channel + manifest are still unauthenticated" without TLS.
- Measured ATP vs rsync on 2026-09-02: faster on lossy links and single large files, 4× slower on small trees over clean links, 1.6–2.1× the sender memory. Across real WAN paths, "the QUIC sender is window-limited at ~90 ms RTT", so the README makes "no unqualified 'beats rsync' claim".
- Semver: breaking changes may ship in `0.(x+1).0`. The 0.6.0 release lists breaking changes.

**Known gaps relevant to FrankenMining (FACT → implication).** There is no native FrankenSQLite client in asupersync (its `sqlite` feature wraps C rusqlite), so FrankenSQLite must be consumed directly. The Semantic Subject Fabric is not shipped. `atpd` is unpublished. `franken-kernel::Cx` and `asupersync::Cx` are **different types**. A plan must say which one is meant, and asupersync's is the runtime authority.

### 2.2 frankengraphdb (fgdb): "a database written in the asupersync programming model"

**Purpose (FACT, README).** "A blank-slate, memory-safe … property-graph database … It unifies MVCC, time-travel history, git-style branches, replication, and change subscriptions into a single fountain-coded commit stream." It runs in three postures: the embedded library `fgdb`, the server `fgdbd` (FGP native protocol, HTTP/2, gRPC, WebSocket, Bolt subset) and the CLI `fgdb`. The language is GQL (ISO/IEC 39075:2024), with an openCypher on-ramp. The README is written in the present tense "as if the entire design … is fully realized". It must be read together with `IMPLEMENTATION_STATUS.md`, whose last re-derivation is dated 2026-10-11.

**Subsystem names (plan §5–§14).**
- **Chronicle**: commit stream, durability, time, branches.
- **Strata**: storage engine, with temperature tiers (inline micro-adjacency → sorted delta blocks → sealed CSR runs).
- **Loom**: languages, algebra, optimizer and execution (Free-Join, worst-case-optimal joins, factorized execution).
- **Ripple**: DBSP-style Z-set incremental views and subscriptions.
- **Beacon**: index fabric (B-tree/hash, FTS/BM25, HNSW, `hybrid.search`).
- **Prism**: in-database analytics through franken_networkx.
- **Warden**: macaroon capability tokens with graph caveats compiled to planner predicates (FG-INV-20).
- **Fabric**: server, protocols and embedded API.
- **Aegis**: replication and HA.

Crates include `fgdb, fgdb-chronicle, fgdb-strata, fgdb-gql, fgdb-beacon, fgdb-prism, fgdb-warden, fgdb-policy, fgdb-types, fgdb-delta-types, fgdb-bigint, fgdb-claim, fgdb-evidence, fgdb-crypto, fgdb-sim, fgdb-reference, fgdb-server, fgdb-protocol, fgdb-bolt, fgdb-cli`. The unsafe boundary crates are `fgdb-unsafe-arena`, `fgdb-unsafe-simd` and `fgdb-unsafe-vfs`.

**Exact API (FACT, from FA §2.1 checked against `crates/fgdb/src/lib.rs` and fgdb's IMPLEMENTATION_STATUS).**
- `fgdb::Database` is a single-writer handle (`&mut self`).
- Opening: `create/open/open_memory/open_read_view(&CommitCx)`.
- Writing: `write(&CommitCx, WriteBatch)`; `begin(&TxnCx) -> WriteTxn` with `vertex_property`/`vertex_has_label` point reads, `write_ordered`, `refresh_snapshot` and `commit`; `prepare_write`/`commit_prepared`.
- Querying: `query(&QueryCx, text, &GqlParameters, GraphSymbolResolver, GqlQueryPolicy)`; `execute_certified` returns `NativeResultCertificate { plan, values_digest, result_digest, statement, snapshot_identity, database_identity, procedures }`; `replay`.
- Sessions and feeds: `read_session()` returns `EmbeddedReadView`; `subscribe_native`; `pinned_read_view`; `delta_since`/`ensure_delta_window`; `bulk_load` (`BulkLoadPolicy`, `BulkLoadCheckpoint`).
- Batch operations: `WriteBatch::{create_vertex, add_edge, set_*_property, compare_and_set_*}`; key material via `DatabaseKeys`.
- Purpose-typed `Cx` wrappers: `QueryCx`, `TxnCx`, `CommitCx`, `MaintCx`, `ReplCx` (fgdb plan §1 constraint 8).
- Chronicle: `MarkerRef`, `CommitSeq`, capsule-first, marker-last two-fsync commit, `manifest.root` as "the only mutable file".
- Storable values: `CanonicalScalar = Null | Bool | Int(i64) | Decimal | Float | Text | Timestamp | Bytes(≤64 MiB)` (also `Point`). There are no storable `Date`, `Duration` or 128-bit integers. `CanonicalDecimal` is an i128 at fixed scale 18.

**Live vs planned (FACT, FA Appendix H dated 2026-10-10, cross-checked against fgdb IMPLEMENTATION_STATUS).**

| Capability | Status |
|---|---|
| Durable embedded DB, Chronicle, `FOR SYSTEM_TIME AS OF SEQ`, retention | Live |
| Bounded optimistic `WriteTxn`, first-committer-wins over explicit point/scan witnesses ("deliberately not SSI") | Live |
| Full Graph-SSI (`fgdb-txn`) | Planned ("Full SSI, merge-ladder integration and transaction ownership/session policy remain incomplete") |
| Valid time (`ValidTimeContract`, `FOR VALID_TIME`) | Planned |
| Branch read selectors (`AT BRANCH`) / fork + merge + `DIFF` | Read selectors live / fork and merge planned |
| Subscriptions / Ripple materialized views | Subscriptions live (in-process, no durable resume, unrestricted capability required) / MVs planned |
| Indexes | In-memory property-equality index only; no durable, range or commit-maintained index; FTS/HNSW computed per call |
| `CALL fnx.*` (Prism) | Live for 14 registered procedures; `hybrid.search` composes |
| Certified reads + replay | Live for native reads (not writes or subscriptions) |
| Warden caveat compilation / redaction / audit pipeline | Types live / compilation in progress / audit pipeline planned |
| Durable catalog | **Absent**: host-interned `u64` ids; the host supplies a `GraphSymbolResolver` |
| HLC wall-clock on commits | Planned (`commit_hlc` carries the sequence number) |
| Constraints (key / unique / cardinality) | Planned |
| Group commit | Planned (one publication with two fsyncs per write) |
| MMR accumulator / transparency checkpoints | Planned |
| **Spatial index** | **Rejected for 1.0** (plan §3 (k)); a Z-order/Hilbert route is recorded for post-1.0 |
| Robot CLI | Live: `--robot` first, `event`-discriminated NDJSON (`invocation, columns, row, result, error, …`), `robot schema`, exit codes `success 0, usage 2, query 3, open 4, io 5`, 0600 three-line key file |

**How FrankenAccounting uses it (FACT, FA §1 constraints 1, 3, 14; §2.1; Appendix H).** FA describes itself as "an application written in the FrankenGraphDB programming model". In FA's words, FrankenGraphDB is the "sole book storage engine", "consumed, never forked". Every substrate capability FA relies on has an Appendix H row giving its status, FA's interim posture and its switch-over rule. Pins: `fgdb, fgdb-types, fgdb-delta-types, fgdb-gql, fgdb-crypto, fgdb-bigint` (and `fgdb-sim` as a dev dependency) at git rev `b323bd17…` (2026-10-09). fgdb's HEAD is now 19 days and many commits ahead of that pin.

### 2.3 frankensqlite (`fsqlite`): released Rust SQLite with concurrent writers

**Purpose (FACT).** "An independent ground-up Rust reimplementation of SQLite with page-level MVCC concurrent-writer support."
- `BEGIN CONCURRENT` targets SERIALIZABLE isolation through page-level SSI ("Page-SSI"), and plain `BEGIN` promotes to it.
- Same-page base drift makes the loser retry with `SQLITE_BUSY_SNAPSHOT`.
- `PRAGMA fsqlite.serializable = OFF` downgrades to plain snapshot isolation.
- Async API: `fsqlite::Connection` (`open`, `execute`, `execute_with_params`, `prepare`, `transaction()`), `SqliteValue`, `FrankenError`, `ConnectionEnv::new_with_root_cx` to root the `Cx` lineage. "Every trait method that touches I/O, acquires locks, or could block accepts `&Cx`."
- Crates: `fsqlite, -core, -mvcc, -pager, -wal, -btree, -vdbe, -planner, -parser, -types, -error, -func, -vfs, -ext-{fts3,fts5,json,rtree,icu,misc,session}, -c-api, -wasm, -harness, -e2e, -observability, -cli`.
- The README badge says "unsafe only in VFS + C ABI".

**Maturity (FACT, README "Current Implementation Status" and "Limitations").**
- "Compatibility mode over standard SQLite files is the live runtime path today". Native mode / ECS (RaptorQ erasure-coded stream) is "design plus partial implementation".
- The safe write-merge ladder is "design plus dormant implementation, not live commit behavior".
- `FOR SYSTEM_TIME AS OF` "works for `:memory:` databases only".
- The vectorized kernels and the Silo epoch group commit are unwired, and the JIT is off by default.
- Extensions live: **FTS5, JSON1, R-tree/geopoly, ICU, misc**. FTS3/4 are helper-level only.
- FA §2.5 cited "a data-integrity advisory open on 0.4.10". **No such advisory was found in the current README**, so treat that claim as stale or unverified.

**Consumers (FACT):** beads_rust (`fsqlite =0.4.10` with mixed 0.4.10–0.4.12 sub-crates), mcp_agent_mail_rust (through `sqlmodel-frankensqlite =0.5.0`), eidetic_engine_cli (`=0.4.10`), frankengit (`=0.4.0`), `frankensearch-storage`, and frankensim `fs-ledger` ("Design Ledger v0 … events on FrankenSQLite").

**When siblings choose it vs fgdb:** see §4.

### 2.4 frankensearch: two-tier hybrid local search

**Purpose (FACT).** "Two-tier hybrid local search for Rust and the `fsfs` standalone CLI: fast first-pass results, then quality refinement." The pipeline is canonicalize → classify → fast embed + **Quill BM25** → RRF fusion (initial) → quality embed (top candidates) → blend → optional rerank.

Crates:
- `frankensearch-core`
- `-embed` (`hash`, `model2vec`, `fastembed`)
- `-index` (FSVI vector storage, SIMD top-k, optional native HNSW)
- `-quill` (native pure-Rust BM25; FSLX segments; delta-visible indexing)
- `-quill-gauntlet` (differential certification against pinned Tantivy)
- `-lexical` (Tantivy, kept as a conformance oracle and interop lane)
- `-fusion` (RRF, two-tier)
- `-rerank` (pure-Rust frankentorch cross-encoder)
- `-storage` (FrankenSQLite metadata and embedding queue)
- `-durability` (repair primitives)
- `-fsfs` (CLI)
- `-tui`
- `-ops` (experimental)

Features: `default` (`hash`), `semantic`, `hybrid`, `persistent`, `durable`, `quill`, `lexical-tantivy`, `full`, `full-fts5`.

**Purity caveat (FACT → risk).** `semantic` enables `fastembed`, and the standard full install provisions an **ONNX Runtime** with the Potion and MiniLM models (about 621 MB). Intel macOS is unsupported. Under the FSS/FA closed-universe doctrine, that is a foreign production runtime. A pure-Rust configuration exists: `quill` + `hash`/`model2vec` + frankentorch rerank. FSS deep-dive §5 imports "progressive pinned retrieval, searchable delta versus durable seal … absence claims require coverage certificates" and gate `INT-FSEARCH-001`. FA leaves frankensearch eligible but not linked (FA §2.6).

### 2.5 franken_networkx (`fnx-*`): deterministic graph algorithms

**Purpose (FACT).** A "Rust port of NetworkX that treats **observable behavior** as a hard constraint".
- Crates: `fnx-classes` (`Graph`, `DiGraph`, `MultiGraph`, `MultiDiGraph`), `fnx-algorithms`, `fnx-cgse`, `fnx-views`, `fnx-dispatch`, `fnx-convert`, `fnx-readwrite`, `fnx-generators`, `fnx-runtime`, `fnx-durability`, `fnx-conformance`, `fnx-python`.
- **CGSE** provides a 13-variant `TieBreakPolicy`, a per-call `ComplexityWitness` ("length-prefixed Blake3 decision-path ledger") and a strict/hardened `CgsePolicyEngine`.
- Conformance artifacts carry RaptorQ sidecars with decode-proof receipts.
- NetworkX 3.6.1 surface: "4,129 / 4,129 strictly present". NetworkX's own suite: "6,815 pass, 0 fail, 0 error" with fnx as backend (2026-09-24). 315 backend-dispatched algorithms.
- Quality gate: `dsr quality --tool franken_networkx`.
- Relevant to mining: `preflow_push`, `minimum_cut_edmonds_karp`, `network_simplex_int` and max-flow / min-cost-flow families. Pit-limit optimization is a max-closure / min-cut problem, which makes these relevant (**HYPOTHESIS**; scale not verified).

**What "certified" means here (FACT).** It means deterministic tie-breaks plus complexity and decision-path witnesses plus differential conformance. It does **not** mean machine-checked optimality proofs. In fgdb, `CALL fnx.*` evidence is folded into `NativeResultCertificate`.

**Pinning hazard (FACT, FA §2.3).** Consumers must pin fnx at **the substrate's** git rev (`9d710b1c…`, the rev `fgdb-prism` uses), "never a registry version, or two `fnx_classes::Graph` types would link".

### 2.6 fastmcp_rust: MCP framework

**Purpose (FACT).** A "Cancel-aware Model Context Protocol (MCP) framework for Rust", ported from jlowin/fastmcp, extended with asupersync capability contexts.
- Handler macros: `#[tool]`, `#[resource]`, `#[prompt]`.
- Handlers take `ctx: &McpContext` (cancellation and budget via `Cx`) and return `McpResult<T>`.
- Builders: `auto::ServerBuilder` serves 2026-07-28 and older clients; `modern::ServerBuilder` pins 2026-07-28.
- Traits: `ToolHandler`, `ResourceHandler`, `PromptHandler`.
- Transports: stdio, HTTP/SSE, WebSocket.
- Crates: `fastmcp, -core, -protocol, -transport, -server, -client, -macros, -console, -cli`.
- Published `0.13.0`.

**Maturity (FACT).** The README states that "MCP 2026-07-28 support is under implementation and remains unverified". The root `PROTOCOL_VERSION` is `2024-11-05`. The official conformance suite (`@modelcontextprotocol/conformance@0.2.0-alpha.10`) at `1c2dc896` (2026-10-12) measured **112/115 SUCCESS, 0 FAILURE**. The toolchain is pinned to `nightly-2026-08-25`.

**Consumer conventions (FA §2.4, §15.4, §18.4, Appendix H R-MCP).**
- `fa-mcp` is the *only* crate allowed to depend on fastmcp.
- It is "a thin adapter from the Teller verb registry … it defines no accounting semantics of its own".
- Tool clusters have at most 7 tools each, and a parity gate checks verbs against tools.
- Transitive `serde`, `chrono`, `time`, `ring`, `cap-std`, `num-*` are confined to the MCP posture through `deny.toml` wrapper rows and `scripts/check-forbidden-deps.sh`.
- fastmcp pins asupersync `=0.5.0` from the registry, so `[patch.crates-io]` is required to keep one `Cx` in the build graph.

### 2.7 frankentui (`ftui`): TUI kernel

**Purpose (FACT).**
- "High-performance terminal UI kernel": 20 crates, 70 widgets, 45 demo screens, plus a WASM/WebGPU web backend.
- `ftui-runtime` is an "Elm/Bubbletea runtime" (`Program`, `update(&mut self, msg) -> Cmd<Msg>`).
- **Inline mode** (`ScreenMode::Inline { ui_height }`) keeps scrollback.
- Evidence JSONL diagnostics; shadow-run validation (`RolloutPolicy::Shadow`, `ShadowRun::compare_recorded`); `doctor_frankentui`.
- Runtime lanes `FTUI_RUNTIME_LANE = legacy | structured | asupersync`, with **default `structured`**. The asupersync lane needs `ftui-runtime/asupersync-executor`.
- Version `0.9.0`, with a "status: WIP" badge.
- Consumers are on mixed versions: Agent Mail 0.7.0, frankenfs 0.3.1, fnx optional 0.5.0.

**How siblings treat it.** FA lists it as "eligible for an inline progress view" but, as a product choice, ships no TUI in 1.0 (FA §2.6, §15.3: "it never opens a TUI"). FSS §27.1 makes `fss-ui` "optional … pure-Rust UI/TUI".

### 2.8 Agent-coordination facilities (brief)

These are development-process tooling, not product dependencies. A product plan references them for *how the plan is executed* by agent swarms.

- **mcp_agent_mail_rust (`am`, 0.3.39).** "A mail-like coordination layer for AI coding agents, exposed as an MCP server with 45 tools and 25 resources, Git-backed archive, SQLite indexing, an interactive 16-screen TUI … and an agent-first robot CLI". Tools include `macro_start_session`, `register_agent`, `send_message`, `fetch_inbox`, `acknowledge_message`, `file_reservation_paths` (advisory file leases), `install_precommit_guard`, `acquire_build_slot`, `summarize_thread` and product-level cross-repo search. Built on fastmcp 0.13, asupersync =0.5.0, fsqlite =0.4.10 and ftui 0.7.0.
- **beads_rust (`br`, 0.7.6).** A Rust port of Yegge's beads frozen at the "classic SQLite + JSONL" architecture. It is the work graph: `br ready`, dependencies, `br sync --import-only`. The companion `bv` does graph-aware triage. Suite discipline (asupersync AGENTS.md RULE 0.5): "JSONL is truth and `beads.db` is disposable … closure on cited evidence … `br dep cycles` stays empty". Sibling plans cite bead ids such as `fa-0hs4.5`, `asupersync-52hxjz` and `fgdb-txn` as tracking handles.
- **ntm (1.37.0, Go).** "Turns `tmux` into a local control plane for multi-agent software development": session orchestration, graph-aware triage, safety policy and approvals, Agent Mail integration, robot surfaces, and a local REST/WebSocket API. Being Go, it does not fit a "pure Rust" claim; it is tooling only.

### 2.9 frankenredis and frankenfs (brief)

- **frankenredis (0.1.2).** A "memory-safe, clean-room Rust reimplementation of Redis 7.2.4 with strict drop-in protocol parity". It has a strict/hardened split, a Sentinel state machine and 16 `fr-*` crates. `unsafe` is fenced into `fr-simd`, `fr-uring` and two others. **It does not use asupersync.** Plausible FrankenMining role: none in core (**DESIGN**). If a cache or pub/sub is wanted, asupersync messaging or derived projections are the suite-native path. No sibling plan uses it.
- **frankenfs (to v0.2.0).** "Rust userspace filesystems: ext4 and btrfs, expanding to FAT16, FAT32, NTFS and ZFS", with "Block-level MVCC · RaptorQ repair · Explicit durability boundaries"; "All profiles remain experimental". The FSS deep-dive §4 imports its *ideas*: rooted capability I/O, the staged/visible/durable/protected publication lattice, COW generations, sealed repair plans, evidence-dimensional readiness, and admission gate `INT-FFS-001`. FA §2.6: frankenfs "confirms that RaptorQ is consumed from asupersync, never reimplemented."

### 2.10 FSS Part IX concepts: where each one lives and its status

| Concept (FSS section) | Defined (design) | Mechanism source repo / crate | Status (FACT, 2026-10-28) |
|---|---|---|---|
| Closed pure-Rust dependency universe (§31, §6.3, §27.3) | FSS `DEPENDENCY_CONSTITUTION.md`, `architecture/dependency_allowlist.toml` | Pattern from fgdb §1 c1, FA §18.4, eidetic `deny.toml` + `scripts/check-forbidden-deps.sh`, frankengit `constellation.lock` | Enforced in FA (posture closures). FSS workspace has "zero third-party crates" and only asupersync =0.5.0. |
| One version universe + semantic transactions (§32, §27.7, §5.5) | FSS `docs/ONE_VERSION_UNIVERSE.md` (rev 2, 2026-09-24): `EvidenceDeltaBatch`, `EvidenceAnchor` / `LedgerAnchor` (`site_lineage, ledger_epoch, commit_sequence, …, state_root`); `schemas/evidence_delta_batch.v1.json` | fgdb Chronicle ("the commit stream *is* the delta stream", `delta_since`); fsqlite positive/negative witnesses, Page-SSI, merge ladder (dormant); fgdb FCW witnesses (live), Graph-SSI and merge ladder (planned) | FSS reference implementation: `EvidenceDeltaBatch` in 48 files; "Ordered `EvidenceDeltaBatch` authority history and exact `LedgerAnchor` succession" implemented. Persistent integration open. |
| Pure-Rust media/streaming kernel (§33) | FSS `docs/STREAMING_AND_MEDIA_KERNEL.md` | FSS's own `fss-codec-*`, `fss-container`, `fss-packet` | FSS reference |
| Certified graph intelligence (§34, §19.5) | FSS `docs/GRAPH_INTELLIGENCE_ARCHITECTURE.md`, `registries/GRAPH_ALGORITHMS.md` (`ALG-*`), `GraphAlgorithmWitness` (`fss.graph_algorithm_witness.v1`) | fnx CGSE (`TieBreakPolicy`, `ComplexityWitness`); fgdb Prism / `NativeResultCertificate`; fgdb Strata tiers, Ripple Z-sets | **FSS reimplements** in `fss-graph-algorithms` (does not link fnx): "19 families are implemented and oracle-certified" of 27 registered (unqualified) |
| Pure-Rust model runtime (§35) | FSS `PURE_RUST_MODEL_RUNTIME.md` | frankentorch (adjacent) | FSS `fss-tensor`, `fss-model-ir` reference |
| ATP archive, replication, repair, deletion (§36) | FSS `docs/ATP_ARCHIVE_AND_REPLICATION.md`, `ATP_AND_DISTRIBUTED_EVIDENCE.md` | **asupersync `src/atp/`, `src/net/atp/`** (Asupersync Transfer Protocol), RaptorQ | asupersync: live CLI, durable journaling and resume (0.6.0), `atpd` unpublished. FSS: ATP integration listed as open. |
| Decision cards, adaptive policy, performance proof (§37) | FSS `docs/DECISION_CARDS_AND_EXPERIMENTS.md`, `architecture/decision_cards.json`, `DecisionCard` | asupersync `franken-decision` (`DecisionContract`, `LossMatrix`, `Posterior`, `FallbackPolicy`, `DecisionAuditEntry`) + `franken-evidence` (`EvidenceLedger`); asupersync scheduler `decision_contract.rs`; fgdb plan §1 c10 ("emit decision cards") | Crates exist in asupersync (v0.6.0). FSS: `DecisionCard` appears in only 2 files. |
| Local DSR qualification and release authority (§38) | FSS `LOCAL_QUALIFICATION_AND_RELEASE.md`, `docs/deep-dives/DOODLESTEIN_SELF_RELEASER.md`; `scripts/qualify.sh` | **`doodlestein_self_releaser`** (bash; `dsr check/build/release/quality`, nektos/act, SSH hosts, minisign, SBOM) | Live tool; asupersync 0.5.0 and 0.6.0 released through DSR "without GitHub Actions"; fnx uses `dsr quality` |
| Agent cognition, control and accretion (§39) | FSS `AGENT_COGNITION_AND_CONTROL.md`, `AGENT_COGNITIVE_CONTROL_PLANE.md`, `AGENT_OPERATING_MODEL.md`: `SituationCapsule`, `SituationFrame`, affordance frontier, root-last handoff | dwarf_fortress_mcp (small transactional vocabulary), eidetic_engine_cli (provenance memory), fastmcp (bounded request ownership) | FSS reference; `SituationCapsule` in 23 files |

### 2.11 Adjacent suite projects (outside the brief, flagged for other research tracks)

- **frankensim** (`20463525d 2026-10-26`, 164 native `fs-*` crates). Candidates for resource modeling and geomechanics:
  - `fs-rep-voxel` (VDB voxel charts: block-model-like)
  - `fs-uq` (Karhunen–Loève random fields, polynomial chaos, MLMC)
  - `fs-assimilate` (linear-Gaussian belief updates from instrument observations)
  - `fs-voi` / `fs-oed-e2e` (value-of-information drilling and sensor placement)
  - `fs-solid` (elasticity / FEM)
  - `fs-sparse` (AMG)
  - `fs-opt` (optimization IR)
  - `fs-ledger` (design ledger on FrankenSQLite)

  It depends on asupersync through a **path dependency** (`../asupersync`), which is unpinned. FTX's approach is to lift frankensim files with provenance headers instead of depending on the workspace.
- **frankenscipy** (`fsci-opt` with `linprog`; `fsci-spatial` with KD-tree/qhull; `fsci-interpolate`, `fsci-stats`, `fsci-sparse`), **franken_numpy**, **frankenpandas**. FA treats these as "eligible suite projects … adoption follows a concrete consumer" (FA §2.6).
- **frankensympy** (`fsym-bigint`, `fsym-rational`, `fsym-budget`). Adopted by FA for exact rates.
- **eidetic_engine_cli** (0.18.0). Per FA §2.6, "the reference consumer of the stack — exact `=` pins, no path deps, a `deny.toml` that bans tokio/rusqlite/petgraph/hyper/reqwest/axum", plus `franken-stack.lock`.
- **frankengit**. Provides `constellation.lock` (per-crate checksum and unsafe digest) and `registries/dependency_policy.tsv`.
- **sqlmodel_rust** (0.5.0). An asupersync-native ORM over FrankenSQLite (`sqlmodel-frankensqlite`).
- **franken_markdown** (`fmd`, 0.6.0). Report rendering to HTML/PDF; FSS imports "exact spans … one representation for deterministic multi-output evidence" (`INT-FMD-001`).
- **frankentorch**. Pure-Rust model execution, used by the frankensearch rerank path.

---

## 3. Architecture idioms of the frankensuite

Each idiom gives a definition in the suite's own vocabulary and the location of its canonical text. Abbreviations: **AS** = asupersync README, **FSS** = surveillance plan, **FSS-DD** = `FRANKENSTACK_DEEP_DIVE.md`, **FA** = accounting plan, **FG** = frankengraphdb plan, **FTX** = textiles plan.

### 3.1 Runtime idioms (from asupersync)

**I-1. Region tree (structured concurrency, "no orphans").**
- Every task belongs to exactly one region, and regions form a tree.
- `close(region) → quiescence(all descendants)`: all children complete, finalizers run and registered obligations are resolved.
- Consumers draw their own physical topology as a region tree. FSS §7.1 uses `ProcessRegion └── PropertyRegion ├── LedgerRegion, ObjectStoreRegion, ProjectionRegion, SensorRegion*, EventRegion*, OperationsRegion`. FA makes each background service (register maintainer, bundle builder, forensic batch) "a supervised region".
- *Defined:* AS "Design Philosophy §1" and the "Architecture" diagram; FSS §7.1; FSS-DD §2.1 ("Region ownership becomes the physical topology of FSS"); FA §2.2.

**I-2. `Cx` authority, capability narrowing and purpose-typed contexts.**
- Every function that does I/O, takes a lock, allocates from a shared arena or can block takes `&Cx` ("`Cx` everywhere").
- Cloning a context narrows or preserves authority and never broadens it. In asupersync this is enforced at type level by `CapSet<SPAWN,TIME,RANDOM,IO,REMOTE>`.
- Pure law code runs under a **stripped `Cx`** with no clock, entropy, network or filesystem: `TIME=false, RANDOM=false, IO=false`. Rule evaluation is therefore deterministic "by construction, not by review".
- Substrates expose purpose wrappers (`QueryCx`, `TxnCx`, `CommitCx`, `MaintCx`, `ReplCx`).
- FSS adds domain fields to the context: principal, privacy scope, anchor/version universe, lease/fence, idempotency identity and seed.
- *Defined:* AS §4 and "Cx — Capability Context"; `src/cx/cap.rs`; FG §1 constraints 3, 8; FA §1 constraint 3; FSS §7.2; FSS-DD §2.2.

**I-3. Cancellation protocol (request → drain → finalize).**
- Cancellation is a protocol, not `drop`.
  1. `request` stops new work and effects.
  2. `drain` propagates to children, closes inputs, settles channels and persists partial receipts.
  3. `finalize` commits or aborts staged state, reconciles ambiguous effects, releases leases and emits a quiescence proof.
- Hard kill is containment that "produces a recovery obligation".
- Drains publish `ProgressCertificate`s where wired.
- *Defined:* AS §2; FSS §7.3; FSS-DD §2.5 ("Cancellation requires progress certificates").

**I-4. Four-valued outcomes, plus domain indeterminacy.**
- `Outcome = Ok | Err | Cancelled | Panicked`, with lattice `Ok < Err < Cancelled < Panicked`.
- Consumers MUST NOT flatten cancellation or panic into ordinary errors.
- Domains add **indeterminate** when a remote or physical effect may have occurred.
- *Defined:* AS "Outcome — Four-Valued Result"; FSS §7.5; FSS-DD §2.3 ("prevent epistemic flattening").

**I-5. Budgets and bounded backpressure.**
- `Budget` (deadline, poll and cost quotas, priority) composes by `meet` (tighter wins).
- Every edge is bounded. Dropped data emits a continuity/derivation receipt, and source evidence is never silently dropped.
- FSS extends budgets to bytes, frames, tokens, model time and memory.
- *Defined:* AS "Budget"; FSS §7.2, §7.4, §25.4.

**I-6. Obligations and two-phase (reserve/commit) effects.**
- Permits, acks, leases, I/O ops, semaphore permits and transactions are tracked obligations that must commit or abort, and a region cannot close with live ones.
- Consumers lift this to **durable domain obligations** that survive a crash. FSS examples: "after alert commit, reconcile delivery status", "after archive child upload, publish/abort root", "after deletion commit, prove closure".
- *Defined:* AS §3 and "OBLIGATION REGISTRY"; `ObligationKind`; FSS §7.6; FSS-DD §2.4.

**I-7. Deterministic lab runtime and reference oracle first.**
- Swapping the `Cx` runs the *whole system*, including the MCP server, under virtual time, seeded scheduling, DPOR, chaos, crash points and replay.
- Before optimized or concurrent paths exist, a **single-threaded reference implementation** is the semantic oracle: FSS §7.7 "deterministic reference runtime", FA `fa-reference` ("own limb arithmetic; no engine crate"), fgdb `fgdb-reference`.
- `fa-sim-harness` (lab harness) exists "before the first kernel line".
- *Defined:* AS §5; FSS §7.7, §23; FA §16.2–16.5, §18.1; FG §15 (Bet B5).

**I-8. Supervision (Spork / OTP) for long-lived services.**
- Long-running daemons are `CompiledSupervisor` / `AppSpec` topologies with deterministic restart policies. FA's `fad` process tree uses Spork.
- *Defined:* AS "Spork (OTP Mental Model)"; FA §2.2.

### 3.2 Dependency, safety and format idioms

**I-9. Closed dependency universe.**
- Production resolution = `std/core/alloc` + project workspace + asupersync + exact admitted Franken revisions + "tiny DEP-recorded foundational exceptions".
- "'Widely used,' 'fundamental,' and 'convenient' are not sufficient arguments."
- Machine-enforced with `deny.toml` bans (tokio, async-std, smol, rusqlite, sqlx, chrono, time, rust_decimal, bigdecimal, num-bigint, arrow, polars, petgraph, hyper, axum, reqwest, tantivy, and serde/ring except through wrappers) and `scripts/check-forbidden-deps.sh` (`cargo tree -e normal … --invert serde`).
- FA clarified that the universe is closed "to unrelated third-party additions, not to the owner's Franken projects".
- FTX shows the light variant: a small pinned allowlist (serde, spade, image decoders, wasm-bindgen), each candidate recording version, used API, transitive closure and benefit.
- *Defined:* FSS §6.3, §27.3, §31 and `DEPENDENCY_CONSTITUTION.md`; FA §1 constraint 1, §18.4; FG §1 constraint 1; FTX §16.

**I-10. Exact foundation pins and one package id per foundation.**
- Each foundation is pinned to an exact git rev or `=` version, recorded with its rationale in `franken-stack.lock` ("A bump of one is a bump of all").
- `cargo metadata` proves "exactly one `asupersync`, one `fnx-classes`, and one `fgdb-types` package id per posture closure".
- `[patch.crates-io] asupersync = { git, rev }` unifies registry consumers (fastmcp) with git consumers (fgdb).
- FSS: "First-party path work is captured at exact clean revisions so mutable sibling checkouts cannot leak into a binary."
- *Defined:* FA §2.4, §18.4, `franken-stack.lock`; FSS §6.3; frankengit `constellation.lock`.

**I-11. Pure Rust; unsafe forbidden except in ledgered substrate islands.**
- Every crate root has `#![forbid(unsafe_code)]` and the workspace sets `unsafe_code = "forbid"`.
- Raw-pointer needs live only in separately named substrate boundary crates (`fgdb-unsafe-*`, `fr-simd`), each with an unsafe-boundary ledger row (path, invariant, evidence, scalar fallback, no-claim boundary).
- Products add none: FA says "A proposed unsafe island is a design defect to be routed upstream".
- *Defined:* FSS §6.4; FA §1 constraint 2; FG §1 constraint 2; AS "Unsafe Boundaries" (`artifacts/unsafe_boundary_ledger_v1.json`).

**I-12. Foreign tools are laboratory oracles only (no-substitute architecture).**
- Python, FFmpeg, vendor SDKs, ONNX/CUDA stacks, Tantivy and C SQLite may serve as *pinned differential oracles*, but they never ship, never act as fallback and never define production truth.
- "'Out of process' does not turn a foreign runtime into pure Rust."
- FTX §4: fixtures are "captured from the reference … **never regenerated from the candidate**"; moving a pin is a deliberate event.
- *Defined:* FSS §6.2 (trust-domain table), §6.4, §6.6; FSS-DD conclusion 26; FA §16.3; FTX §4.

**I-13. Durable-format boundary.**
- Control and report JSON may use Serde.
- Canonical evidence, ledger, index, graph, transfer, repair and release bytes use hand-written versioned formats: magic, explicit endianness, hard bounds, canonical ordering, checksums, migration fixtures, corruption and fuzz tests, and an independent reconstruction command.
- Each format has a `FMT-*` owner.
- "No persistent bytes are Rust layout, bincode default, or Serde derive output." FG and FA list "serde-derived enum as a durable format" among the constitutional prohibited shortcuts.
- *Defined:* FSS §6.5, §27.4; FG §1 constraint 9; FA §1 constraint 13.

### 3.3 State and truth idioms

**I-14. Canonical ledger plus derived, rebuildable projections.**
- One authoritative append-only ledger or journal. Registers, indexes, search, graph analytics, caches and memory are *derived*, "never more authoritative than the journal" (FA-INV-04, inherited from FG-INV-18), discarded and rebuilt on recovery, and verifiable by recomputation.
- The ledger holds typed identity, transitions and small records. Large immutable artifacts live in content-addressed object graphs: "an object-store listing never substitutes for ledger truth."
- *Defined:* FSS §13.1, §19.1, §27.6; FA §1 constraint 5; FSS-DD conclusion 22.

**I-15. One version universe (anchors and high-water marks).**
- Every canonical change is an immutable ordered delta batch. Every derived system (graph, search, model cache, subscription, replica, branch) consumes the same stream and publishes its exact high-water mark.
- "No query can silently combine 'latest' rows from unrelated versions."
- Answers pin an anchor.
- *Defined:* FSS §5.5, §27.7, §32, `docs/ONE_VERSION_UNIVERSE.md`; fgdb README ("the commit stream *is* the delta stream"); FSS-DD §7.1.

**I-16. Semantic transactions with witnesses.**
- Decisions are optimistic transactions carrying **positive and negative read witnesses**, validated at commit by a narrow deterministic combiner: "expensive work never runs at the commit point".
- Conflicts resolve through the merge ladder: intent replay → stable-key structural merge → registered commutative composition → reconcile/replan → reject.
- Last-writer-wins is forbidden for semantic state.
- FA instance: period law is "enforced at commit, not reported after", using a retained point-read witness on `Period.state`.
- *Defined:* FSS §32; FSS-DD §3.2–3.5 and conclusions 5–8; FG §7; FA §1 constraint 7.

**I-17. Negative evidence and absence claims.**
- "A cache/search miss treated as evidence of absence" is explicitly rejected.
- Absence is "a predicate over an observed domain and interval with a coverage certificate".
- *Defined:* FSS §4.5, §6.6; FSS-DD conclusion 5, §5.7.

**I-18. Root-last (child-first) publication.**
- Multi-artifact state becomes visible only when all children are staged and verified, then the root is published last.
- "A remote PUT response is not root publication. Root publication is not retrievability."
- The same shape appears at four scales: fgdb's commit ("capsule-first, marker-last, two fsyncs"); FrankenFS's staged/visible/durable/protected lattice; FSS archive (§13.4, ten steps); DSR releases (signed manifest published "only after every required target/lane … passes").
- *Defined:* FSS §13.4, §38; FSS-DD conclusion 11, §4.1, §4.3.

**I-19. Time discipline: recorded time vs valid time.**
- Recorded (system) time is assigned by the commit stream and never supplied by users.
- Wall-clock instants enter only as journaled time evidence.
- Valid or effective time is a separate typed interval axis.
- *Defined:* FA §1 constraint 6, §7.1, §7.1a; FG §5 (Chronicle) and valid-time contracts; FSS §5.7–5.9.

**I-20. Planes and trust domains.**
- FSS separates **packet → authority → cognition → effect** planes. Cognition is rebuildable from authority roots. Effects are "narrow, fenced, idempotent, and reconciliation-first".
- The agent layer is "an orthogonal control membrane … explicitly not a fourth truth plane".
- *Defined:* FSS §4.1, §4.2, §6.1, §6.2, §18.1, §39.

**I-21. Capability before expansion.**
- Authorization compiles into the query plan or projection *before* expansion, so hidden counts, paths and absence cannot leak. Refusal is preferred over post-filtering.
- *Defined:* fgdb Warden (FG-INV-20); FA §1 constraint 8; FSS §19.5.

### 3.4 Surface and operations idioms

**I-22. Single artifact, several postures, with robot-first surfaces.**
- One codebase produces an embedded library, a CLI with `--robot`/`--json` NDJSON envelopes (`event`-discriminated, versioned `robot schema`, exit-code dictionary, `robot-docs`), a daemon owning the writer lease, and an MCP adapter.
- The CLI, MCP, TUI and reports are **projections of one verb/operation registry**: "A button is not a new effect; a MCP tool is not a new authority path."
- Bare invocation prints triage and never a TUI (FA).
- A parity gate checks that every verb is a tool or a registered exclusion.
- Errors teach: `{code, what_failed, where, why, fix, alternatives, docs}`.
- *Defined:* FA §1 constraint 9, §15.3–15.4, Appendix D; FG §1 constraint 5; fgdb-cli `robot schema`; FSS §20.6, §20.15.

**I-23. Doctor and repair (doctor → sealed plan → apply).**
- `doctor --json` is read-only. It returns versions/generations, degraded checks with evidence handles, suspected causes, bounded next commands and proposed repair-plan handles.
- Repair is two-phase: `repair plan` produces a sealed plan with observations, mutations, risks, rollback, cost and expiry; `repair apply <plan-digest>` revalidates, reserves obligations, executes bounded effects and publishes a receipt.
- "No repair rewrites immutable evidence history."
- *Defined:* FSS §26.3–26.4; FSS-DD conclusion 12, §4.5; FA §15.3 (`fa doctor`); `doctor_frankentui`; `atp doctor`; fgdb W10 doctor target.

### 3.5 Evidence and governance idioms

**I-24. Evidence labels and typed claims.**
- FSS labels prose as **FACT / DESIGN / HYPOTHESIS / TARGET / OPEN**.
- FG and FA register every load-bearing claim as exactly one of `invariant`, `proof`, `bounded_model`, `statistical`, `slo`, `benchmark`. "A weaker class may inform policy but may not enforce or justify a stronger one."
- FTX uses "four claims, never conflated": **Parity / Physics / Fit / Performance**. Its rule is that "A failure is a failed cell, never an excluded cell."
- FSS-DD conclusion 28: "No readiness claim is aggregate."
- *Defined:* FSS "Document control → Evidence labels"; FG §1 constraint 11; FA §1 constraint 11; FTX §3.

**I-25. Stable identifiers (prefix registry; never renumbered; tombstones).**
- FSS publishes a prefix table, including `INV-, GOAL-, NONGOAL-, CAP-, EFFECT-, ERR-, SCHEMA-, ADR-, INT-, WP-, GATE-, TEST-, SLO-, COST-, RISK-, OPEN-, ALG-, PUB-, DEC-, DEP-, REL-, FMT-, TRACE-, ATP-, AGT-, AOP-, AVIEW-, KSTATE-, PROV-, QL-`.
- FA uses `FA-INV-*`, `FA-EVID-*`, `FA-CAL-*`, `FA-CFG-*`, `FA-REF-*` refusal codes and Appendix H rows `R-*`. FG uses `FG-INV-*`.
- *Defined:* FSS "Document control → Stable identifiers"; FA Appendix F, Appendix H.

**I-26. Registries as data, validated by tooling.**
- Contracts live in machine-checked registries, and prose mirrors them.
  - FA: `registries/*.toml` (`constitution`, `invariants`, `evidence`, `slo`, `workspace_topology`, `foundation_dependencies`, `operation_costs`, `verbs`, `exit_codes`, `refusal_catalog`, …), validated by std-only `tools/registry-check` at gate G0.
  - FG: `registries/*.toml`, including `unsafe_boundary_ledger.toml` and `proof_lanes.toml`.
  - FSS: `architecture/*.json` + `registries/*` + `schemas/*`.
- `workspace_topology.toml` and the beads graph are checked acyclic in CI.
- *Defined:* FA §18.1–18.2; FG registries/; FSS front-matter "Normative companions".

**I-27. Costs are registered claims (operation-cost registry).**
- Every operation class has a row giving its substrate transactions, ordered durability barriers, external authorities and minted durable objects.
- A latency gate below its barrier floor "is a registry error that fails CI, never an ambitious target".
- Products map their rows onto the substrate's rows.
- *Defined:* FG §1 constraint 12 and Appendix G; FA §1 constraint 12 and Appendix G; FSS `COST-` rows and `architecture/operation_cost_registry.toml`.

**I-28. Certified, replayable results.**
- Same evidence closure gives byte-identical canonical results, including order.
- Every report, finding, plan and bundle carries a certificate naming that closure: fgdb `NativeResultCertificate`, `ReplayManifest`, `ReplayCompleteness` grades. Graph answers carry CGSE tie-break and `ComplexityWitness` / `GraphAlgorithmWitness`.
- Nondeterminism is opt-in, seeded and declared.
- *Defined:* FG §1 constraint 4, §15; FA §1 constraint 10, §13.6; FSS §19.5, §34; fnx README (CGSE).

**I-29. Decision cards and classified adaptivity.**
- Every adaptive choice is a `DecisionCard`: feasible arms, hard clamps, objective/loss, evidence, selected arm, tie-break, safe fallback, rollback and outcome.
- New policies run shadow-first, and missing or uncertain adaptation falls back to the safe static arm.
- fgdb classifies adaptivity as answer-preserving physical, answer-affecting execution or canonical-state-affecting.
- The runtime embodiment is asupersync `franken-decision` + `franken-evidence`.
- *Defined:* FSS §37, `docs/DECISION_CARDS_AND_EXPERIMENTS.md`; FG §1 constraint 10; asupersync `franken_decision/src/lib.rs`, `franken_evidence/src/lib.rs`.

**I-30. Same-binary performance proof and negative evidence.**
- Performance claims use same-binary A/A and A/B with identical input root and output digest, and timing starts only after semantic equivalence.
- Failed optimizations are durable negative evidence.
- Suite rule: "a self-speedup is MAINTENANCE, not a win — a win needs the incumbent live in the SAME invocation; never weaken a gate to land a change; reporting a loss is a success."
- *Defined:* FSS §25.10, §37, §23.8; asupersync AGENTS.md RULE 0.5.

**I-31. Gates: constitutional, admission, convergence and kill.**
- Constitutional and convergence gates: FSS `GATE-000` (architecture constitution) … `GATE-120` (release candidate); FA G0 "Constitutional Contracts (before any engine code)" → G1 "The Journal Lives" → G2 "Time and Truth" → G3 "Agents and Auditors" → G4 "Published".
- **Admission gates** per foundation: FSS-DD `INT-AS-001` (nine criteria, e.g. "no owned work survives region close", "no second runtime … in the dependency closure"), `INT-FSQL-001`, `INT-FFS-001`, `INT-FSEARCH-001`, `INT-FMD-001`, `INT-FGDB-001`.
- **Kill gates** carry a fallback: FTX §19, e.g. "If contact or performance cannot be made to work, keep the Blender path … do not ship a solver that is merely different."
- "A gate is a retained evidence decision, not 'the code seems done.'"
- *Defined:* FSS §29; FSS-DD §2.9, §3.10, §4.9, §5.9, §6.5, §7.9; FA §19; FTX §19.

**I-32. Subset, never substitute; no empty crates.**
- "Early code may implement a *subset* of each final abstraction — never a substitute for it." Prohibited shortcuts include a global-lock "interim" transaction model, `HashMap` presented as storage, soft-delete flags, mutable balance columns and detached background threads.
- A crate joins the workspace only with its first real slice.
- *Defined:* FG §1 constraint 9; FA §1 constraint 13, §18.1; FSS §27; FTX §16.

**I-33. Consume, never fork, through a foundation dependency ledger with interim postures.**
- When a needed substrate capability is not live, the product picks either a deterministic interim mechanism that stays correct after the capability lands, or a feature-flagged absence. It never copies substrate code and never adds a second storage engine.
- Each row records status, interim posture and switch-over rule (FA Appendix H, `registries/foundation_dependencies.toml`).
- FSS: "FrankenSQLite is not represented as ready merely because a dependency can be added."
- *Defined:* FA §1 constraint 14, Appendix H; FSS §13.2.

**I-34. Local release authority (DSR) and honest status.**
- Hosted CI is not release authority.
- Release begins from a clean snapshot with exact sibling revisions and a locked offline closure.
- `scripts/qualify.sh` is the semantic entrypoint. Assets are uploaded, then downloaded and reverified, and "Signing authority is separate from builders."
- Honest status documents are required: fgdb and FSS `IMPLEMENTATION_STATUS.md`; FA §20.1 "Reality check and bridge plan"; fgdb's README "note on tense".
- *Defined:* FSS §38, §29.13–29.15, `LOCAL_QUALIFICATION_AND_RELEASE.md`; FSS-DD conclusion 27.

**I-35. Work-graph and agent-swarm discipline (process, not product).**
- Beads (`br`/`bv`) hold the work graph, with JSONL as truth. Agent Mail handles file reservations and threads. ntm runs tmux swarms.
- Plans enumerate implementation issues: FSS Appendix F "First 250 implementation issues"; FTX §21 "Work breakdown (bead titles)".
- *Defined:* asupersync AGENTS.md RULE 0.5; FSS §21.8, §20.12, Appendix F; FTX §21.

---

## 4. Storage choice guidance

### 4.1 Recorded decisions in the sibling plans (quoted or paraphrased)

**FA §2.5, "frankensqlite: why it is not the substrate (decision record)."** FA evaluated frankensqlite (then 0.4.10, about 26,500 tests, released) and rejected it.

| Dimension | frankensqlite | FrankenGraphDB |
|---|---|---|
| Data model | relational; tracing becomes recursive CTEs; "the ledger is a graph" bet becomes ETL | property graph; REA native; `fnx` calls over the live snapshot |
| System time | snapshot ring; `VersionStore` design-only | immutable commit stream; `FOR SYSTEM_TIME AS OF` live |
| Valid time | none | `ValidTimeContract` (planned, normative) |
| Branches | none ("a scenario is a copy of the database file") | fork/merge planned; read selectors live |
| Proofs | none | content-addressed objects live; MMR planned |
| Capabilities | connection-level | macaroon caveats compiled to planner predicates |
| Determinism | deterministic but uncertified | plan certificates, replay, CGSE witnesses |
| Incremental views | none | Ripple (planned) |
| Maturity | released | unreleased; several surfaces planned |

> "The decision: **FrankenGraphDB is the substrate**, because five of the six bets are properties the substrate must have at byte zero and frankensqlite has none of them; choosing it would mean rebuilding Chronicle, valid time, branches, Ripple, Warden, and the proof layer inside FrankenAccounting — the 'application grows a database' fossil. … frankensqlite is not linked at all … two storage engines in one book is a smell with no upside. … If the substrate's schedule slips catastrophically, the registered response is to pin a known-good revision and contribute upstream — never to swap engines."

**FA §1 constraint 1 and §2.6.** FrankenGraphDB "remains the sole book storage engine". frankensearch is "eligible, but not currently linked … A future adoption must preserve FrankenGraphDB as the sole authoritative book store and keep search indexes derived."

**FSS §13.2, Persistence adapters.**
1. deterministic in-memory reference;
2. deterministic append-only fixture/file adapter for replay;
3. FrankenSQLite production adapter after `INT-FSQL-001`.

"Semantic tests run against all applicable adapters. FrankenSQLite is not represented as ready merely because a dependency can be added." FSS-DD §3.9 also records the "Ledger and object store split", and §3.10 lists the ten `INT-FSQL-001` criteria. These include phantom tests for positive and negative reads, crash injection at every reserve/materialize/publish boundary, and "no direct use of lower-level APIs that bypass the connection/semantic witness pipeline".

**FSS §6.6 and FSS-DD conclusion 22 / §7.8.** "A generic graph database as authority" is explicitly rejected.

> "The graph database is never placed in the camera effect path merely because graphs are useful. Canonical evidence and effect truth remain in the authoritative ledger; graph/search engines are projections with high-water marks and certificates." … "Optimized graph storage enters only after reference equivalence and workload evidence."

`INT-FGDB-001` requires one-high-water-mark provenance, reference graph equivalence, immutable snapshots, incremental/full equivalence, capability noninterference, branch isolation, plan-certificate replay, and "measured benefit over simpler ordered maps".

**FSS §27.6, Ledger versus object graph.** "A transaction never embeds arbitrary video bytes, and an object-store listing never substitutes for ledger truth."

**FG §1 constraint 1.** fgdb reuses frankensqlite at the *design level* ("we fork/adapt specific modules into `fgdb-*` crates rather than linking `fsqlite-*` wholesale, because graph objects are not SQLite pages"). `fsqlite-*` crates are forbidden in fgdb's closure (FA §2.5).

**FTX §5.** "Data contracts are files" (`fabric-pattern/1`, `ftx-body/1`, `ftx-drape-report/1`, …), with no database at all. This is the right choice for a single-user computation pipeline.

**Tools in the ecosystem.** beads_rust, Agent Mail, Eidetic, frankengit and frankensim `fs-ledger` all choose **FrankenSQLite** for local, tool-scoped, relational state. This is the de facto default when the domain is not a temporal evidence graph.

### 4.2 Distilled rule (DESIGN, derived from the records above)

Pick the store by **what the authority is shaped like**, and keep exactly one authority per bounded context.

| If the bounded context's truth is… | Choose | Reason (citation) |
|---|---|---|
| A temporal, evidence-bearing, multi-party graph with algebraic laws, where "as known at t", branches/scenarios, certified reads and capability-compiled visibility are needed at byte zero | **FrankenGraphDB** as sole authority, with an Appendix-H-style ledger of planned capabilities and interim postures | FA §2.5 |
| An event/effect/custody ledger with high-rate ingestion, where graphs are useful for cognition but not for authority | **FrankenMining-owned ledger contract** (delta batches) with adapters: reference → append-only → FrankenSQLite after its admission gate; graph and search as derived projections | FSS §13.2, §6.6, FSS-DD §7.8 |
| Large immutable artifacts (media, tensors, models, indexes, proofs) | **Content-addressed object graph**, root-last publication, moved by **ATP**, RaptorQ repair | FSS §13.3–13.4, §27.6, §36 |
| Tool-local relational state (queues, caches, work graphs, metadata) | **FrankenSQLite** | beads, Agent Mail, Eidetic, frankensearch-storage, `fs-ledger` |
| Text or semantic retrieval | **frankensearch** (Quill + pure-Rust embedders), always derived | FA §2.6; FSS §19.2–19.4 |
| A batch computation whose inputs and outputs are documents | **Versioned files** (schema-tagged) | FTX §5 |
| Spatial indexing (2D/3D points, extents) | **Not fgdb in 1.0** (no spatial index, by decision). FrankenSQLite R-tree/geopoly live; frankensim VDB voxel charts and frankenscipy KD-tree are candidates for derived spatial projections | fgdb plan §3 (k); fsqlite README |

### 4.3 Mining-specific storage consequences (DESIGN / HYPOTHESIS)

- **Corporate books, revenue, royalties, cost of production, reserves-based depreciation.** These belong in FrankenAccounting, which runs on fgdb. FrankenMining should not build a second general ledger.
- **Tenure, permits, approvals, ownership, royalties and streams, chain of custody (drillhole → sample → assay → QA/QC → estimate → reserve statement), competent-person sign-offs.** This is FA-shaped: a temporal evidence graph that needs "as known at", restatements and valid-time intervals. fgdb fits, *but* valid time, constraints, catalog and HLC are only planned (FA's interim postures apply). **OPEN** for an ADR; criteria in §5.3.
- **Fleet, SCADA, telemetry, plant historian, geotech and tailings instrumentation.** This is FSS-shaped: high-rate, custody-critical, with the graph as cognition. Use a FrankenMining-owned delta ledger for state transitions and columnar segment objects for raw series. fgdb (no group commit, single writer, two fsyncs per write) is a poor fit for raw telemetry (**HYPOTHESIS**, not measured).
- **Block models, grade-control models, point clouds, survey meshes, pit shells, schedules.** These are large immutable artifacts. Use content-addressed object graphs with manifests and root-last publication, and record lineage in the ledger. They must not be stored as graph vertices.
- **Spatial queries** (which blocks or holes fall within a shell, polygon or geofence). Use derived spatial projections (fsqlite R-tree or a first-party index); fgdb cannot index `Point`.

---

## 5. Recommendations: how FrankenMining should layer itself on the substrate

### 5.1 Posture (DESIGN)

1. **Adopt the FA consumption model and the FSS truth doctrine.** Link the substrate as code at exact revisions (FA). Keep FSS's planes, one version universe, root-last publication, obligations, negative evidence, doctor/repair and admission gates. Do not imitate mechanisms in-house when a suite crate exists: FSS's in-house reimplementation (`fss-graph-algorithms` instead of fnx; `std::net` instead of asupersync I/O) has left it with "Architectural deviations to resolve".
2. **FrankenMining is a product of laws, vocabulary, surfaces and verification corpora.** FA says it "does not add a storage engine, a transaction model, a time model, a proof system, or a security model". FrankenMining should say the same. It adds mining laws (mass balance, reconciliation, reserve classification rules, precedence and slope constraints, TARP escalation), mining vocabulary, and mining surfaces.
3. **Physical effects stay prepared and advisory in 1.0.** Following FSS §18 and §21.9, dispatch changes, blasting, pump and valve control, and autonomous haulage are `EFFECT-*` plans with revalidation, idempotency and reconciliation-first receipts. Direct control loops are a `NONGOAL` for 1.0 (**DESIGN**, mirroring FSS's drone "manual flight only" posture).

### 5.2 Dependency map: which project for which concern

| Concern | Use | How (exact names) | Admission gate (proposed) |
|---|---|---|---|
| Runtime, ownership, cancellation, supervision | **asupersync** (all crates) | `Cx`, `Scope`, regions, `Outcome`, `Budget`, `ObligationKind`, `LabRuntime`, `CompiledSupervisor`/`AppSpec`; stripped `CapSet` for pure law and estimation code | `INT-AS-001` (reuse FSS-DD §2.9's nine criteria) |
| Suite IDs, evidence ledger, decision cards | **asupersync `franken-kernel`, `franken-evidence`, `franken-decision`** | `TraceId`, `DecisionId`, `PolicyId`, `SchemaVersion`; `EvidenceLedger`; `DecisionContract`, `LossMatrix`, `FallbackPolicy` | part of `INT-AS-001` (confirm `franken-kernel::Cx` is not confused with `asupersync::Cx`) |
| Bulk movement of artifacts between site, cloud and DR (block models, survey data, archives) | **asupersync ATP** (+ RaptorQ) | object graphs, manifests, Merkle roots, root-last; `atp` CLI; TLS-authenticated control channel required | `INT-ATP-001` (FSS §36 criteria; note `atpd` unpublished) |
| Network services (site ↔ HQ APIs, historian connectors) | **asupersync net/http/grpc/tls/quic** | native HTTP/1.1/2/3, gRPC, WebSocket, TLS | inside `INT-AS-001` |
| Title, provenance, reserves audit graph (bitemporal) | **frankengraphdb**, conditionally | `fgdb::Database`, `WriteBatch`/`WriteTxn` with `TxnCx`/`CommitCx`, `query(&QueryCx…)`, `execute_certified`, `delta_since`, `FOR SYSTEM_TIME AS OF SEQ` | `INT-FGDB-001` + an Appendix-H ledger (valid time, catalog, constraints, HLC, branches) |
| Operational ledger persistence / tool state / spatial projections | **frankensqlite** | `fsqlite::Connection`, `BEGIN CONCURRENT`, `ConnectionEnv::new_with_root_cx`, R-tree/geopoly, FTS5 | `INT-FSQL-001` (FSS-DD §3.10) |
| Graph algorithms (precedence DAGs, haul-network routing, max-closure / min-cut pit limits, critical path, flow) | **franken_networkx** (`fnx-classes`, `fnx-algorithms`, `fnx-cgse`) at fgdb-prism's rev; in-database via `CALL fnx.*` | `TieBreakPolicy`, `ComplexityWitness`; `preflow_push`, `network_simplex_int`, min-cut | `INT-FNX-001` (scale benchmark on realistic block counts) |
| Document and report search (logs, assay certificates, technical reports, permits) | **frankensearch** (`quill`, `hash`/`model2vec`, frankentorch rerank; **not** `semantic`/fastembed/ONNX) | Quill BM25, FSVI, RRF two-tier; durable seal vs searchable delta | `INT-FSEARCH-001` (FSS-DD §5.9) |
| Agent surface | **fastmcp_rust**, confined to `fm-mcp` | `#[tool]`, `McpContext`, `auto::ServerBuilder`; `[patch.crates-io] asupersync` | `INT-FMCP-001` (parity gate, posture confinement) |
| Human terminal surface | **frankentui** (optional `fm-tui`, inline mode, asupersync executor lane) | `Program`, `Cmd<Msg>`, `ScreenMode::Inline` | `INT-FTUI-001` |
| Finance / accounting | **franken_accounting** as a sibling product | emit FA typed intents (`fa post --intent`) and import FA bundles / certificates (`fa-verify`); share asupersync / fgdb pins | `INT-FA-001` (interface-level; see risks) |
| Exact money / rate arithmetic in mining laws | **frankensympy** (`fsym-rational`, `fsym-bigint`, `fsym-budget`) or FA's `fa-money` laws | — | reuse FA's R-RATIONAL pattern |
| Numerics (geostatistics, UQ, FEM, optimization) | candidates: **frankensim** (`fs-uq`, `fs-rep-voxel`, `fs-assimilate`, `fs-voi`, `fs-solid`, `fs-sparse`), **frankenscipy** (`fsci-opt`, `fsci-spatial`, `fsci-interpolate`) | lift-with-provenance (FTX §11.5) or exact pins; never path deps | separate research track (**OPEN**) |
| Report rendering | **franken_markdown** (`fmd`) | HTML / PDF from one source | `INT-FMD-001` |
| Release | **DSR** + repo `scripts/qualify.sh` | clean snapshot, exact sibling revs, root-last signed manifest | `REL-*` lanes (FSS §38) |
| Dev process | beads (`br`/`bv`), Agent Mail, ntm | — | not product dependencies |

### 5.3 Decision criteria for the fgdb-vs-own-ledger ADR (OPEN → decide at FrankenMining G0)

Choose **fgdb as authority** for a bounded context only if all of the following hold. Otherwise use the FSS adapter ladder and keep fgdb as a derived projection.
- (a) The context's core value comes from "as known at t" plus restatement, branches and scenarios, and certified graph reads. Reserve and resource statement history and title chains qualify.
- (b) The required valid-time semantics can use FA's interim posture (indexed effective-date properties plus a FrankenMining interval algebra) until fgdb's `ValidTimeContract` lands.
- (c) Write rates fit one publication with two fsyncs per commit (no group commit) on a single-writer handle.
- (d) The context needs no spatial indexing inside the store.
- (e) FrankenMining accepts owning a journaled symbol table, since fgdb has no catalog (FA R-SYM).

### 5.4 Workspace shape (DESIGN, modelled on FSS §27.1 and FA §18.1)

The `fm-*` crate layers are below. A crate is created only with its first real slice, every root is `#![forbid(unsafe_code)]`, and dependencies point strictly downward.

- **Foundation:** `fm-types` (stable IDs, generations, anchors), `fm-units` (exact quantities: tonnes, grades, ounces, m³, with conservation laws), `fm-time` (recorded vs valid time, shifts, fiscal calendars), `fm-registry`, `fm-sim-harness` (lab runtime harness, before the first kernel line), `fm-reference` (single-threaded oracle with no engine dependency).
- **Runtime/fabric:** `fm-runtime` (asupersync integration, region tree), `fm-capability`, `fm-obligation` (durable domain obligations, e.g. sample-dispatch → assay-received, TARP-trigger → acknowledge/escalate, blast-plan → reconcile-actuals), `fm-decision` (over `franken-decision`).
- **Authority/storage:** `fm-ledger` (delta batches, anchors, witnesses, combiner), `fm-object` (content-addressed artifacts, root-last publication), `fm-transfer` (ATP), adapter crates per admitted store.
- **Domain laws (pure; stripped `Cx`):** exploration / QA-QC, resource estimation, reserve classification, mine planning constraints, mass balance and reconciliation, tailings TARPs, cost and royalty bridges to FA.
- **Cognition / projections:** `fm-graph` (fnx / fgdb projections with witnesses), `fm-search`, `fm-spatial`, `fm-twin`.
- **Effects:** `fm-plan`, `fm-effect`, `fm-alert` (prepared, revalidated, idempotent).
- **Surfaces:** `fm` (facade), `fm-cli` (robot mode, exit-code dictionary a superset of `fgdb`/`fa`), `fmd`-style daemon (writer lease), `fm-mcp` (only fastmcp dependent), optional `fm-tui`.
- **Verification / release:** `fm-sim`, `fm-oracles`, `fm-gen` (synthetic mines), `fm-bench`, `fm-conformance`, `fm-fuzz`, `fm-verify` (std-only), plus `tools/registry-check`.

Proposed **region tree** (DESIGN, after FSS §7.1):

```text
ProcessRegion
└── EnterpriseRegion
    ├── LedgerRegion / ObjectStoreRegion / ProjectionRegion
    ├── ExplorationRegion (ingest, QA/QC, LabDispatchObligation*)
    ├── ModelingRegion (EstimationRunRegion*, stripped Cx)
    ├── PlanningRegion (ScenarioRegion* = branch or scratch replay; OptimizationRunRegion*)
    ├── SiteRegion*
    │   ├── TelemetryIngestRegion (fleet/SCADA/historian; bounded edges)
    │   ├── TwinRegion
    │   ├── PlantRegion (mass balance, reconciliation)
    │   ├── TailingsRegion (instrumentation, TARP obligations)
    │   └── ArchiveRegion (ATP)
    ├── FinanceBridgeRegion (FA intents and bundles)
    └── OperationsRegion (doctor, repair plan/apply, qualification)
```

### 5.5 Pinning policy (DESIGN)

- Join the **0.5.0 universe** that fgdb, FA and fastmcp share: asupersync rev `78b64636…`, with `[patch.crates-io]`. Pin fgdb at an exact rev and fnx at **fgdb-prism's** rev.
- Record everything in `franken-stack.lock`, with "a bump of one is a bump of all". Gate in CI on one package id each for `asupersync`, `fnx-classes` and `fgdb-types`.
- Adopt `nightly-2026-08-31` and register fastmcp's `nightly-2026-08-25` drift as a risk, as FA does.
- Coordinate the asupersync 0.6.0 migration with the fgdb and FA owners rather than moving alone. 0.6.0 has breaking changes, and moving alone would create two `Cx` types.
- No path dependencies (frankensim's `../asupersync` path dependency is the anti-pattern).

### 5.6 Risk register (FACT-based risks, severity is DESIGN judgment)

| ID | Risk | Evidence | Severity | Mitigation |
|---|---|---|---|---|
| RISK-01 | fgdb unreleased; load-bearing capabilities planned (valid time, branches, Ripple MVs, SSI, catalog, constraints, HLC, group commit, MMR); spatial index rejected for 1.0 | fgdb IMPLEMENTATION_STATUS; FA Appendix H; fgdb plan §3 (k) | High | Per-context ADR (§5.3); Appendix-H ledger with interim postures; never fork, never add a second engine inside a context |
| RISK-02 | asupersync version split (main 0.6.0 released; suite pinned 0.5.0); two-`Cx` hazard with registry vs git pins | Cargo manifests; FA §2.4 | High | Join the 0.5.0 rev universe, use `[patch.crates-io]`, one-package-id CI gate, coordinated bump |
| RISK-03 | No sibling has asupersync-native production I/O end to end except FA; FSS still uses `std::net` | FSS IMPLEMENTATION_STATUS (owner decision `fss-x4a.8.1` open) | Medium | `INT-AS-001` early, in a walking skeleton; I/O through `Cx`-checked paths |
| RISK-04 | Asupersync guarantees are runtime-enforced; cleanup budgets advisory; I/O entry points take no `Cx` parameter; 13 of 24 lab oracles unfed | AS README | Medium | Lab-runtime crash/cancel matrices of FrankenMining's own; do not claim stronger than AS claims |
| RISK-05 | FrankenAccounting's own reality check: "No genuine full verdict exists"; scale issues (single posts 118→439 ms over 300 posts in debug; `fa verify book` 15 s at 324 entries and exits 8 on a healthy book) | FA §20.1 (2026-10-26) | Medium–High for a finance bridge | Integrate at the intent / bundle interface; keep FrankenMining's semantic core testable without FA; gate on FA's V38 "operable at real-book scale" |
| RISK-06 | frankensearch semantic default uses ONNX Runtime via fastembed (a foreign runtime) | frankensearch README feature table, installer notes | Medium | Use `quill` + `hash`/`model2vec` + frankentorch rerank only; `deny.toml` ban on `ort`/`fastembed` |
| RISK-07 | frankensqlite: native ECS/RaptorQ mode and write-merge ladder not live; time travel `:memory:` only | fsqlite README | Medium | Use it as compatibility-mode relational store; FrankenMining owns history (delta batches) and does not rely on fsqlite time travel |
| RISK-08 | fastmcp: MCP 2026-07-28 "unverified"; transitive serde / chrono / ring / time; older nightly | fastmcp README; FA R-MCP | Low–Medium | Confine to `fm-mcp`; parity gate; posture-closure checks |
| RISK-09 | fnx algorithm scale for mining-size graphs (millions of blocks, tens of millions of precedence arcs) unproven; FSS chose to reimplement graph families | fnx README; FSS IMPLEMENTATION_STATUS | Medium | `INT-FNX-001` benchmark on synthetic block models; contribute faster max-closure (pseudoflow) upstream rather than forking |
| RISK-10 | Tooling not pure Rust (ntm in Go; DSR in bash + act) | repo inspection | Low | Process tooling only; never in release closure |
| RISK-11 | License rider (MIT + OpenAI/Anthropic) on core suite crates | LICENSE badges | OPEN | Owner decision for FrankenMining's licence and downstream users |
| RISK-12 | Documentation tense: fgdb README describes the target state; FSS plan is normative design; both overstate what is live | fgdb README "note on tense"; FSS/FA status docs | Medium (planning error) | Cite IMPLEMENTATION_STATUS / Appendix-H rows, never READMEs, for "live" claims; use FACT/DESIGN labels |

### 5.7 Open questions for the FrankenMining plan

- **OPEN-1:** Per bounded context, which store holds authority (fgdb vs FrankenMining ledger + fsqlite adapter)? Decide at G0 with the §5.3 criteria.
- **OPEN-2:** Does FrankenMining link `franken_accounting` crates (`fa` embedded API) or interoperate only through intents and bundles? The plan proposes interface-level integration first.
- **OPEN-3:** When do FrankenMining and its siblings migrate to asupersync 0.6.0, and who coordinates?
- **OPEN-4:** Spatial index ownership: FrankenMining-owned `fm-spatial`, fsqlite R-tree projection, or an upstream contribution to fgdb's post-1.0 Z-order route?
- **OPEN-5:** Numerics substrate (frankensim vs frankenscipy vs first-party) for kriging, conditional simulation and pit optimization. This belongs to another research track.

---

## Appendix A — Vocabulary crib (exact suite terms to use in the plan)

| Term | Meaning | Source |
|---|---|---|
| region / region tree / quiescence | ownership scope; closes only when descendants, finalizers and obligations are done | AS §1 |
| `Cx` / capability narrowing / stripped `Cx` / `CapSet` | explicit authority context; narrowing only; no TIME/RANDOM/IO for pure laws | AS §4; `src/cx/cap.rs`; FA c3 |
| `QueryCx` / `TxnCx` / `CommitCx` / `MaintCx` / `ReplCx` | fgdb purpose-typed contexts | FG c8 |
| request → drain → finalize; `ProgressCertificate` | cancellation protocol and drain evidence | AS §2; FSS §7.3 |
| `Outcome` (Ok/Err/Cancelled/Panicked) + indeterminate | non-flattened results | AS; FSS §7.5 |
| `Budget` / `meet` | composable resource bounds | AS |
| obligation (`SendPermit`, `Ack`, `Lease`, `IoOp`, `SemaphorePermit`, `Transaction`); reserve/commit | linear effects that must resolve | AS §3 |
| `LabRuntime`, virtual time, DPOR, `CrashPack`, oracle | deterministic verification | AS §5 |
| Spork, `CompiledSupervisor`, `AppSpec` | OTP-style supervision | AS "Spork" |
| ATP (Asupersync Transfer Protocol), object graph, manifest, Merkle root, bond-pull | verified bulk transfer | `docs/atp_architecture.md` |
| `EvidenceLedger`, `DecisionContract`, `LossMatrix`, `FallbackPolicy` | suite decision and evidence schema | `franken_evidence`, `franken_decision` |
| Chronicle, `CommitSeq`, `MarkerRef`, capsule-first / marker-last | fgdb commit stream | FG §5 |
| `NativeResultCertificate`, `ReplayManifest`, `execute_certified`, `replay` | certified reads | FA §2.1 |
| Strata / Loom / Ripple / Beacon / Prism / Warden / Aegis | fgdb subsystems | FG §6–§14 |
| CGSE, `TieBreakPolicy`, `ComplexityWitness` | deterministic graph algorithms | fnx README |
| `EvidenceDeltaBatch`, `EvidenceAnchor`, `LedgerAnchor`, high-water mark | one version universe | FSS `docs/ONE_VERSION_UNIVERSE.md` |
| positive / negative witness; merge ladder; first-committer-wins; SSI | semantic transactions | FSS §32; FSS-DD §3 |
| root-last publication; staged / visible / durable / protected | publication lattice | FSS §13.4; FSS-DD §4.1 |
| doctor; repair plan / apply; sealed plan | operations | FSS §26.3–26.4 |
| FACT / DESIGN / HYPOTHESIS / TARGET / OPEN | evidence labels | FSS document control |
| invariant / proof / bounded_model / statistical / slo / benchmark | claim classes | FG c11; FA c11 |
| `INT-*` admission gate; `GATE-*`; G0–G4; kill gate | gates | FSS §29; FSS-DD; FA §19; FTX §19 |
| `franken-stack.lock`; posture closure; `[patch.crates-io]` | pinning | FA §18.4 |
| interim posture; switch-over rule; foundation dependency ledger | consuming planned capabilities | FA Appendix H |
| `SituationCapsule`, `SituationFrame`, affordance frontier | agent operating layer | FSS §20, §39 |
| DSR (`dsr quality`, `dsr release`), `scripts/qualify.sh` | local release authority | FSS §38 |

## Appendix B — Reproduction pointers

- HEADs: `git -C <repo> log -1 --format='%h %ad %s' --date=short` (values in §1).
- Asupersync pins across consumers: `grep -E '^asupersync' <repo>/Cargo.toml <repo>/crates/*/Cargo.toml`.
- FSS asupersync usage: `grep -rn 'asupersync::' franken_surveillance_system/crates` (5 hits); status: `IMPLEMENTATION_STATUS.md` ("Architectural deviations to resolve", "Work packages still materially open").
- FA usage: `grep -rl 'asupersync::' franken_accounting/crates` (274 files), `grep -rl 'fgdb::'` (220 files); `franken_accounting/franken-stack.lock`.
- fgdb live/planned: `frankengraphdb/IMPLEMENTATION_STATUS.md`; FA plan Appendix H; spatial rejection at fgdb plan §3 ("Further rejections … (k)").
- fsqlite status: `frankensqlite/README.md` sections "Current Implementation Status", "Time Travel Queries", "Limitations".
- frankensearch ONNX: `frankensearch/README.md` ("Install In One Line", "Feature-Flag Decision Table").
