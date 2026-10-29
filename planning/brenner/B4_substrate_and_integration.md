# B4: PQ-4, substrate, storage authority and integration (Brenner pass)

| Field | Value |
|---|---|
| Run | `fm-2026-10-28-a`, pass B4 (independent of the lead agent's thesis) |
| Question | MOR_CASE PQ-4: storage authority, runtime region tree, and the frankensim, FA and fgdb consumption postures |
| Inputs | MOR_CASE (E001–E033); R2 in full; R4 in full; R1 §0–§2, §5–§6; R5 §1–§2, §3.16–§3.19, §6–§8; alien-artifact-coding SKILL plus refs 12, 05, 15, 31, 35 |
| Repos verified read-only (2026-10-28) | asupersync `523e86c00` (0.6.0) and pin `78b64636` (0.5.0, 2026-09-12); frankengraphdb `5b5538e6a`; franken_accounting `3323cf0c`; frankensim `20463525d`; frankensqlite `66cb7efab`; FSS `7fc1bf47`; franken_textiles `971adff`; frankenmermaid `6880dc8e`; franken_markdown 0.6.0 |
| Labels | **FACT** (checked here or cited E###), **DESIGN** (proposed), **HYPOTHESIS**, **TARGET**, **OPEN**. Numbers in cards are budgets or derivations, not measurements |
| Method | Brenner: at least three hypotheses per sub-question, each set including a forced third alternative; falsifier; cheap discriminating test; refuter-first; verdict |

---

## 0. Verdicts at a glance

| # | Sub-question | Verdict (DESIGN unless marked) |
|---|---|---|
| SQ1 | Storage authority | **One authority mechanism.** A FrankenMining-owned append-only delta ledger is partitioned into single-writer *authority cells*, plus a content-addressed object store (CAS). fgdb, fsqlite and frankensearch are **derived projections only**. FA is the separate book of record. The forced-third "fsqlite as relational authority" hypothesis is refuted, and so is the FA-style "fgdb as authority" (§2). |
| SQ2 | Topology | **Hybrid.** Each context×site pair is an authority cell with exactly one writer node (a site, or a regional enterprise node). Sync uses certified, append-only replication: version vectors plus MMR consistency proofs, with CAS chunk transfer over QUIC/ATP. Raw telemetry is site-sovereign and never leaves the site by default. |
| SQ3 | frankensim | **Lift-only in every production posture.** Lift two waves (asupersync-free leaves now; Cx-free geometry data structures next). No Cargo dependency until GATE-FSIM-RECONV: frankensim must pin the *same* asupersync rev as fgdb/FA/fastmcp. |
| SQ4 | FA contract | **`fad` + `fa --json` robot CLI subprocess** from one `fmine-fa-bridge` at the enterprise finance node. A signed intent outbox is the deterministic fallback. fa-mcp is for human-supervised agent work only. Embedded `fa::Book` is used in an isolated conformance workspace only. Trunk granularity is **site × flow-family × month × book**. Detail is Merkle-folded into FA's 128-row external Workpaper. |
| SQ5 | Workspace | **39 crates in 7 layers** under the prefix **`fmine-`**, not `fm-`, because of verified name collisions (§1, S7). Planned 740k LOC with a 850k ceiling. The overrun protocol and `xtask check-layers/check-caps/check-deps` apply from commit 1. Binaries: `fmine`, `fmined`, `fmine-mcp`, `fmine-tui`. |
| SQ6 | Security | MNPI embargo states and wall caveats; per-venture projections; a residency registry the sync planner obeys; **no OT write conduit**; human authority tokens are needed even for *prepared* effects. Explicit mitigations cover asupersync 0.5.0 capability and macaroon defects (§1, S3). |
| SQ7 | Interop | Reimplement LAS/E57/GeoTIFF/DXF/CRS subsets. Lift the omf-rust schema layer only after a dependency audit (Parquet/Arrow hazard). Vendor systems enter as content-hashed snapshot files or HTTPS/JSON. Foreign tools are lab oracles. FM is the anchor-pinned system of record that Avathon reads from and writes *evidence* to, never authority. |

---

## 1. Verified facts and surprises found in the code

**S1. FACT: geometry is coupled to `fs_exec::Cx`, not to `asupersync::Cx`, and much of it needs no context at all. This sharpens E004.**
- `fs_geom::Chart::eval(&self, x: Point3, cx: &Cx<'_>)` (`frankensim/crates/fs-geom/src/lib.rs:689`) uses `use fs_exec::Cx` (line 36).
- `fs_exec::Cx<'s>` (`crates/fs-exec/src/cx.rs:579`) is a per-tile context `{gate: &CancelGate, arena: &fs_alloc::Arena, key: StreamKey, budget: Budget, mode: ExecMode, refusals, lease, clock: Option<&dyn asupersync::time::TimeSource>}`. Its only asupersync items are `pub use asupersync::types::Budget` and the `TimeSource` trait. fs-exec's asupersync dependency is target-gated (`Cargo.toml` lines 25–29).
- **`VdbGrid<T: Copy>` (`fs-rep-sdf/src/vdb.rs`, 310 lines) imports only `std::collections::BTreeMap`.** `winding.rs` imports only `fs_geom::{Point3, Vec3}`.
- So the block-model substrate and robust inside/outside tests are liftable without any context. The Cx coupling lives in `Chart` impls, tiling and executors.
- The suite therefore has **three distinct `Cx` types**: `asupersync::Cx`, `franken_kernel::Cx` (R2 §2.1) and `fs_exec::Cx`.

**S2. FACT: frankensim builds against asupersync 0.6.0, while every other consumer is on 0.5.0. This sharpens E011.**
- `frankensim/Cargo.toml:194` patches `asupersync = { path = "../asupersync" }`, and `fs-exec` uses `path = "../../../asupersync"`. Both resolve to the local checkout, which is at `523e86c00` with `version = "0.6.0"`.
- The 0.5.0 pins are fgdb (`crates/fgdb/Cargo.toml:12`, rev `78b64636`), FA (`Cargo.toml:64,120`), fastmcp (`=0.5.0`), fsqlite and frankensearch (`0.5.0`), ftui and FSS (`=0.5.0`).
- 0.5 and 0.6 are semver-incompatible, so Cargo will link **both** without complaint. `[patch.crates-io]` cannot unify them, because a patch must satisfy the `0.5` requirement.
- So no posture closure can contain frankensim geometry together with fgdb, FA or fastmcp until the suite converges.

**S3. FACT: the 0.5.0 universe carries known security and durability defects that are fixed only in 0.6.0** (`asupersync/CHANGELOG.md` §v0.6.0):
- (a) **Capability escalation.** A restricted `Cx<cap::None>` could `open_child_region` and receive a `Cx<cap::All>`. The method exists at the pin (`src/cx/cx.rs:4086` @`78b64636`) and is fixed in 0.6.0 (asupersync-cwxavr).
- (b) `MacaroonKeyRing::verify` accepted an all-zero signature when no retired key was set (fixed in `43aca3a0d`).
- (c) Legacy third-party caveats can be stripped by the third party. The protected delegation API (`add_protected_third_party_caveat`) exists only at HEAD.
- (d) Plaintext ATP-over-TCP is not refused off loopback, and its manifest is unauthenticated.
- (e) Durable ATP sender/receiver journaling and resume across process loss is a 0.6.0 feature: `src/atp/journal` has 12 files at the pin and 53 at HEAD.

FrankenMining's capability-based security (SQ6) and satellite transfer (SQ2) therefore cannot simply "trust Cx and ATP" at the pin. §7 lists the mitigations.

**S4. FACT: FA's external-subledger seam has hard caps.** `fa-journal/src/external_subledger.rs:38–40` sets `MAX_EXTERNAL_DETAIL_ROWS = 128`, `MAX_EXTERNAL_SUMMARY_LEGS = 32` and `MAX_EXTERNAL_DETAIL_BYTES = 8192`.
- Each row is `{source_digest: [u8;32], account, side, amount, dimensions}`.
- The refusal text requires "an independent principal [to] review it" (`fa/src/external_subledger.rs`).
- Consequences: summaries must be coarse, and the 32-byte `source_digest` per row is the natural place to anchor FM-side Merkle detail (§5).

**S5. FACT: FA's `PostBatch` is atomic, with one batch key.** An identical retry returns the recorded outcome; the same key with different members is refused with exit 6. Agent-profile plans "remain rehearsal only". `Entry.plan_certificate` "remains absent because the pinned substrate has no generic object-store API" (`registries/verbs.toml` lines 263, 343, 399).

**S6. FACT: `fad` takes the book writer lease** (a second daemon gets FA-REF-018, exit 6), listens on `<book>/fad.sock` and "answers `fa`" (`crates/fad/src/main.rs:1–12`). So the FA book must be co-located with whichever FM node posts to it. FA HEAD has moved 3 commits since R4 (`53c6e953`→`3323cf0c`), and `units.toml` still has 13 units and no metals.

**S7. FACT: the requested names collide.**
- (a) **`fmd`** is franken_markdown's short binary (`franken_markdown/Cargo.toml` `[[bin]] name = "fmd"`), and it is also a shell alias on this machine.
- (b) **`fm-*`** is frankenmermaid's crate prefix: `fm-core, fm-parser, fm-layout, fm-render-svg, fm-render-canvas, fm-render-term, fm-wasm`, published to crates.io in that order, with a binary named `fm-cli` (`frankenmermaid/README.md` §Crates.io status).
- (c) **`fm`** is `/usr/bin/fm` on this macOS host, Apple's Foundation Models CLI.
- `fmn` is franken_manim (as briefed).
- Because the reporting path is required to link frankenmermaid, an `fm-core` in FrankenMining would put two `fm-core` packages in one graph.
- frankenmermaid also has an optional native HiGHS (C++) backend, and its source builds "require a C/C++ toolchain and CMake". FM must use the pure default features.

**S8. FACT: fgdb's spatial non-goal is constitutional, not a backlog item.** Plan §3 (k) and line 822: "a declared index over a `Point` property fails closed as a typed error". The post-1.0 route is Z-order/Hilbert over B-trees. fgdb replication (Aegis) is a Raft kernel, which is quorum-based and unsuited to intermittently linked writers (HYPOTHESIS from design shape; no Aegis benchmark exists).

**S9. FACT: FSS's anchor already has a federation field.** `LedgerAnchor` = `site_lineage, ledger_epoch, commit_sequence, adapter_registry_epoch, schema_epoch, policy_epoch, privacy_epoch, state_root` (`FSS/docs/ONE_VERSION_UNIVERSE.md` §2). FM can adopt it with `site_lineage → cell_id` and add `residency_epoch`.

**S10. FACT: all asupersync primitives this design names exist at the pin:** `CompiledSupervisor` (`src/supervision.rs`), `AppSpec` (`src/app.rs`), `LabRuntime` (`src/lab/runtime.rs`), `CapSet` (`src/cx/cap.rs`), `MacaroonToken`, `ObligationKind`, `ProgressCertificate` and `Budget`.

---

## 2. SQ1: storage authority per bounded context

### 2.1 Hypotheses

| ID | Hypothesis | Lineage |
|---|---|---|
| H1-A | **fgdb as authority for evidence-shaped contexts** (evidence/claims, assays, reserves, decisions, plans); delta ledger for telemetry; CAS for block models | FA §2.5 decision record; E016; R2 §5.3 |
| H1-B | **One FM-owned authority mechanism** (delta ledger cells + CAS) for every FM context; fgdb, fsqlite and frankensearch are derived projections; FA stays a separate product | FSS §6.6, §13.2, §27.6; I-14, I-15 |
| H1-C (forced third) | **fsqlite as relational authority** for the table-shaped contexts (drillhole/assay/QAQC, plans, reconciliation), because it is the mature released store (E014) and incumbent tools are SQL Server-backed | beads/Agent Mail/fs-ledger practice (R2 §4.1) |

### 2.2 Refuter-first

- **H1-A falsifier: some evidence-shaped context needs (i) writers at more than one node, or (ii) a spatial index, or (iii) valid time before fgdb ships it.**
  - Assays are approved at site labs and also corrected at enterprise; that is (i).
  - Drillhole intervals are 3D; that is (ii) (S8).
  - Reserve effective dates need valid time, which is planned (E013); that is (iii).
  - **Falsified for 3 of 5 contexts.**
  - Fact-check by census: only the decision ledger and the reserves register are written at one node, at low rate, without spatial queries.
  - Splitting authority across two engines for those two would recreate FA's "two storage engines in one book is a smell" (R2 §4.1), so they stay on the ledger with a registered switch-over rule (OPEN-B4-05).
- **H1-C falsifier: fsqlite cannot give recorded-time history on files**, since `FOR SYSTEM_TIME AS OF` works for `:memory:` only (R2 §2.3).
  - FM would hand-roll supersession tables, which is "the application grows a database", and federation would rest on changesets with last-writer-wins semantics. LWW is forbidden for semantic state (I-16).
  - **Falsified as authority; retained as the best projection store.**
- **H1-B falsifier:**
  - (a) Projection rebuild exceeds the recovery objective, or
  - (b) commit-time witnesses (I-16) cannot be validated without a database.
  - Test of (a): GATE-STORE-01.
  - (b) is answered by design: the narrow combiner keeps an in-memory keyed index per cell, rebuilt from the cell log at open. Each cell's working set is small; the largest is a site assay cell at about 10⁷ intervals × ~48 B ≈ 0.5 GB (TARGET).

**Cheap discriminating test (2 days, DESIGN):**
1. Implement `FMT-LEDGER-1` reference (in-memory and append-only file) for a 10⁵-hole / 10⁷-interval synthetic fixture (`fmine-gen`).
2. Project it to fsqlite (+R-tree) and to fgdb.
3. Measure the full rebuild, an as-known-at query, and a bbox query.

H1-B survives if the rebuild is ≤15 min and both projections answer from one anchor. **Verdict: H1-B (DESIGN), conditional on GATE-STORE-01, with KILL-4 as the fallback.**

### 2.3 Per-context authority table (DESIGN)

The "shape of authority" column applies E016 and I-14. A *cell* is `(context, scope)` with exactly one writer node (§3).

| Context | Authority | Shape-of-authority justification | Projections (derived, rebuildable, high-water-marked) | Write rate (TARGET) | Scope of cells |
|---|---|---|---|---|---|
| Evidence and claims ledger | Delta ledger cells (`FMT-LEDGER-1`); originals in CAS | Append-only, multi-writer across nodes, "as known at" = cell sequence + anchor; claims cite evidence by digest | fgdb enterprise evidence graph (GQL, `execute_certified`, `FOR SYSTEM_TIME AS OF SEQ`, `CALL fnx.*`); frankensearch (quill + hash only) for documents | 10–10³ batches/day/site | per site; per enterprise function |
| Drillhole / assay / QAQC | Delta ledger typed records (collar, survey, interval, sample, dispatch, result, QAQC insertion, certificate) + CAS for lab originals | Immutable observations, corrections by supersession (never edit), MNPI embargo state per record, one authority per number (E022) | fsqlite relational tables + **R-tree on 3D interval boxes**; desurveyed traces as CAS objects; fgdb for lineage | 10³–10⁴ results/day/site | per site (brownfield); per exploration project (greenfield) |
| Subsurface belief objects, block-model projections | **CAS** (4 MiB chunks, fanout-16 Merkle, root-last), with the ledger naming roots, lineage, ModelCard and evidence colour | Large, immutable, content-addressed (E005, R2 §4.3); "an object-store listing never substitutes for ledger truth" | In-memory `VdbGrid`-style tiles (lifted, §4); grade-tonnage summaries in fsqlite | 1–10 roots/day; objects 10⁸–10¹⁰ B | per model domain, at the owning node |
| Spatial indexes | **None (never authority)** | Derived by definition; fgdb cannot index `Point` (S8) | fsqlite R-tree/geopoly (holes, polygons, geofences, shell bboxes); owned BVH/VDB for block containment; Z-order keys in fsqlite | rebuild only | per projection |
| High-rate telemetry | **Site telemetry segment log**: columnar CAS segments plus per-source append-only index; state *transitions* (load, dump, delay, TARP) go to the site delta ledger | High rate, custody-critical, site-bound; the graph is cognition (FSS §6.6); fgdb arithmetic fails (Card G2) | fsqlite segment catalog (source, time range → segment root); rollups (1 min default) replicated to enterprise | 10³–10⁵ msg/s/site | per site per source family |
| Plans and schedules | Delta ledger (plan versions, scenario labels, approvals) + CAS (shells, schedules, designs) | Versioned documents with lineage; scenario = labelled branch of plan cells, not a database fork | fsqlite (Gantt/period queries); fgdb (plan↔evidence lineage) | 10–10² versions/day | LOM: enterprise; short-term: site |
| Reconciliation / material-flow ledger | Delta ledger with **conservation law at commit** (PO-6) | Physical double entry: one mass and metal authority (R5 #56, #60); refusals, not plugs | fsqlite; FA external summaries (§5) | 10²–10³ movements/day/site summarised | per site |
| Reserves register + QP attestations | Enterprise delta ledger cell; QP-signed attestation records; CAS for technical reports | Single writer, legally attributable, `reserve_disclosure_claim` never set by software (E023, E031) | fgdb; FA (`Disclosure` document + Statistical accounts) | 10⁰–10¹/quarter | enterprise (regional nodes for residency) |
| Decision ledger | Enterprise cell; rows typed as `franken-decision::DecisionAuditEntry` + `franken-evidence::EvidenceLedger` | Decisions are evidence-citing, append-only cards (I-29) | fgdb | 10⁰–10²/day | enterprise |
| Accounting | **FA book (fgdb inside FA)**, a separate product | Book of record (E019); FM never computes authoritative balances | FM keeps only posting-slot records and FA receipts (`EntryRef`, certificate digests) as evidence | §5 | per FA book |

**Persistence ladder for the ledger (DESIGN, after FSS §13.2):**
1. L0: in-memory reference, `fmine-ledger::reference`, the semantic oracle.
2. L1: append-only segment files (`FMT-SEG-1`, magic, CRC, hash chain). **This is production.**
3. L2: fsqlite persistence, only if L1 fails GATE-STORE-01 (KILL-4).

Semantic tests run against every rung.

---

## 3. SQ2: site/enterprise topology

### 3.1 Hypotheses

| ID | Hypothesis |
|---|---|
| H2-A | **Central enterprise authority.** Sites are thin clients and caches; every write is committed at HQ. |
| H2-B | **Site-sovereign authority.** Every site is a full authority; the enterprise holds a federated, read-only replica of all sites. |
| H2-C (forced third) | **Hybrid per-context authority cells with certified sync.** Each `(context, scope)` cell has exactly one writer node. Site cells are written at the site, enterprise cells at the enterprise or regional node. Replication is append-only, version vectors detect lag and Merkle MMR proofs certify integrity. |

### 3.2 Refuter-first

- **H2-A is falsified by arithmetic and by safety.**
  - Raw telemetry at 10⁵ msg/s × 200 B ≈ 20 MB/s = 160 Mbit/s. That is 3–16× a 10–50 Mbit/s satellite uplink (Card G2).
  - Sites must keep TARP and QAQC obligations live through multi-day outages.
  - Data residency forbids centralizing some classes (OPEN-B4-01).
- **H2-B is falsified for enterprise-born facts.** LOM plans, the reserves register, the decision ledger and FA postings are created centrally. Giving each site a writable copy invents multi-master conflicts that the doctrine forbids resolving by LWW (I-16).
- **H2-C falsifier:** some fact genuinely needs two writers. Test: the census in §2.3 assigns every context a single writer scope. The only contested case is assay *corrections* at enterprise, which is resolved by routing them as **correction requests** (evidence in an enterprise cell) that the site writer adopts or refuses. There are no shared writes.
- **Cheap discriminating test:** a LabRuntime partition matrix, GATE-SYNC-01 (1,000 seeds, link drops, crash at every boundary). Pass means zero divergence, zero duplicate batch application and all proofs verified.

**Verdict: H2-C.**

### 3.3 Node roles and the authority-cell rule (DESIGN)

- Node roles:
  - `site` (one per mine; Level 3/3.5 of ISA-95, never Levels 0–2);
  - `regional` (an in-country enterprise node where residency requires it);
  - `enterprise`;
  - `relay` (store-and-forward only; holds no lease).
- **INV-CELL-1:** each cell has exactly one lease-holding writer, with a fencing epoch that rises on every transfer. A batch carries `(cell, epoch, seq, prev_hash)`, and receivers refuse an epoch below the highest seen (PO-1).
- Cross-cell references are `(cell, seq, digest)` triples. A consumer resolves them only at an anchor it holds, otherwise the knowledge state is `stale` or `not_observable` (E033). This is the *gluing condition*: the enterprise view is the union of site logs, and is coherent iff every cross-cell reference resolves.
  - This is the one place Family 05 (descent) fits naturally. It compiles to a dangling-reference checker that runs on every anchor publication. Category-theoretic machinery beyond that is ornamental here and is **not** adopted.
- Sync is pull-based from the enterprise and relay side, through outbound-initiated sessions from the site. Sites never accept inbound connections from the Internet; this is the IEC 62443 conduit design.

### 3.4 asupersync region tree (DESIGN; names from E012 / R2 §2.1)

```text
ProcessRegion(fmined, role ∈ {site, regional, enterprise, relay})
└── NodeRegion{node_id, role}                                          supervisor: rest_for_one
    ├── AuthorityRegion                                                 rest_for_one (projections restart after writer)
    │   ├── LedgerWriterRegion{cell}*        CapSet<0,1,0,1,0>  lease + fencing; TIME only as journaled TimeEvidence
    │   └── ObjectStoreRegion                CapSet<0,0,0,1,0>  CAS stage → verify → root-last publish
    ├── IngestRegion (site)                                             one_for_one
    │   ├── TelemetryRegion{source}*         CapSet<0,1,0,1,0>  bounded edges; segment builder
    │   └── ImportQuarantineRegion{fmt}*     CapSet<0,0,0,1,0>  read-only staging dir; parsers; no TIME/RANDOM
    ├── ComputeRegion                                                   one_for_one
    │   ├── LawRegion                        CapSet<0,0,0,0,0>  stripped Cx: mass balance, QAQC, posting laws
    │   └── EstimationRunRegion*             CapSet<1,0,0,0,0>  tiles may spawn; seeds explicit; no IO
    ├── ProjectionRegion                     CapSet<1,0,0,1,0>  fsqlite / fgdb / frankensearch projectors
    ├── SyncRegion                                                      one_for_all per peer
    │   ├── PeerSessionRegion{peer}*         CapSet<1,1,0,1,1>  the ONLY REMOTE-capable region (besides FinanceBridge)
    │   └── TransferRegion{root}*            CapSet<0,1,0,1,1>  CAS chunks over QUIC/ATP
    ├── FinanceBridgeRegion (enterprise)     CapSet<0,1,0,1,0>  spawns `fa` subprocess; local socket only
    ├── EffectRegion                         CapSet<0,1,0,0,0>  prepares EFFECT-* documents; never actuates
    └── OperationsRegion                     CapSet<1,1,0,1,0>  doctor, repair plan/apply, CLI socket, MCP, TUI feed
```

The `CapSet<SPAWN,TIME,RANDOM,IO,REMOTE>` rows are compile-time narrowings. Because of S3(a), `xtask check-caps` bans `open_child_region` outside `fmine-runtime`, and a lab test asserts each region's runtime mask (PO-8).

### 3.5 Budgets, obligations, cancellation, supervision, lab (DESIGN / TARGET)

**Budgets.** `Budget{deadline, poll_quota, cost_quota, priority}` composes by `meet`.

| Region | Deadline | cost_quota | Priority | On exhaust |
|---|---|---|---|---|
| LedgerWriter commit | 250 ms | — | 200 | refuse batch, exit 6, caller retries with the same batch id |
| Telemetry segment seal | 1 s | 64 MiB/segment | 180 | seal early; downsample with continuity receipt; never drop silently |
| EstimationRun | per run, e.g. 4 h | blocks × realizations | 80 | cancel; partial results are never published (root-last) |
| PeerSession window | link window | bytes per window | 120 | drain; persist chunk receipt; resume next window |
| FinanceBridge post | 120 s (debug FA), 10 s TARGET | — | 150 | indeterminate → reconcile via the idempotency key, never re-post blindly |

**Durable obligations (`OB-*`; I-6 lifted to survive crashes).**

| ID | Reserve | Must resolve by |
|---|---|---|
| OB-SYNC-ACK | batch shipped | enterprise ack naming `(cell, seq, mmr_root)`; until then the site retains and re-offers |
| OB-SEAL | chunks staged | root published or staged set aborted |
| OB-POST | posting slot reserved | FA `EntryRef` + certificate digest recorded, or a refusal recorded |
| OB-REVIEW | Workpaper prepared | human `Reviewed{against}` attestation or explicit rejection |
| OB-EFFECT | effect prepared | human authority-token decision, then actuals reconciled |
| OB-LAB | sample dispatched | assay received, or the sample declared lost with a QAQC finding |
| OB-TARP | trigger raised | acknowledged and escalated per TARP |

**Cancellation (request → drain → finalize, I-3) for a sync session:**
1. *request*: stop offering new batches and chunks.
2. *drain*: finish the in-flight chunk; persist a receipt of the chunk hashes received; flush `ProgressCertificate`.
3. *finalize*: publish no root unless complete; release the transfer lease; record a quiescence proof.

A hard kill leaves OB-SEAL live, so the next session resumes from the receipt. This gives resumability **without** relying on 0.6.0 ATP journaling (S3(e)).

**Spork.** An `AppSpec` per role, compiled to `CompiledSupervisor`:
- Restart intensity: 5 restarts / 60 s per child, then escalate.
- Escalation of a PeerSession marks the peer `stale` and keeps local authority running.
- Escalation of a LedgerWriter puts the node in **read-only degraded mode**, so writes refuse with exit 4. A writer is never silently restarted onto a possibly torn segment until `fmine doctor` verifies the hash chain.

**LabRuntime.** Every protocol (sync, posting, effect, ingest backpressure) has `#[lab_test]` suites with:
- virtual time;
- seeded schedules (seeds 0..999 in CI, 10⁴ nightly), with DPOR;
- chaos (link drop p ∈ {0.01, 0.2}, crash at every reserve/commit/publish boundary);
- asupersync oracles (task leak, obligation leak, quiescence, region tree, cancellation protocol);
- FM oracles PO-1/2/3/4/10.

The same seed must give the same trace digest, and a failing seed retains a `CrashPack`.

---

## 4. SQ3: frankensim consumption posture

### 4.1 Hypotheses

| ID | Hypothesis |
|---|---|
| H3-A | **Path or git dependency** on frankensim crates (full fidelity, upstream fixes for free) |
| H3-B | **Lift-only** with `//! Provenance:` headers + golden-hash tests (the FTX pattern, E002) |
| H3-C (forced third) | **Hybrid:** lift asupersync-free leaves now; consume Cx-coupled geometry later through a boundary adapter crate, behind re-convergence gates |
| H3-D (checked null) | **Reimplement everything; use frankensim as a pattern only** |

### 4.2 Refuter-first

- **H3-A, refuted now (FACT-based):**
  - The path dependency drags in asupersync 0.6.0 beside the 0.5.0 of fgdb/FA/fastmcp, giving two asupersync packages, two `Budget` types and two runtimes (S2).
  - frankensim needs `nightly-2026-07-06` for `generic_const_exprs` in fs-qty, against the suite's `nightly-2026-08-31` (E001, R2 §1).
  - `cargo check --workspace --all-targets` exits 101 on 15 targets (E001).
  - A git dependency hits out-of-repo `../../../asupersync` paths (E002).
  - Cheap test (30 min, separate clone, R1 §6.4): build a scratch crate depending on `fs-rep-sdf`, run `cargo metadata`, and count asupersync package ids. Prediction: 2 when combined with `fgdb`.
- **H3-D, refuted on cost.** It would rebuild exact predicates, sparse LDLᵀ/AMG, deterministic transcendentals, Philox and e-processes that are already written, tested and contracted (R1 §3). That is loss item 4 in MOR ("duplicating suite substrate").
- **H3-C as originally framed (adapter for Cx-coupled *geometry*) is mostly unnecessary.** S1 shows the valuable geometry data structures are Cx-free. What remains Cx-coupled is the `Chart` evaluation framework, which FrankenMining does not need for block models (it needs grids, meshes and predicates, not chart routing).
- **Verdict: H3-B now, with H3-C kept only as a gated future option** (`fmine-simbridge` is *not* in the crate map until GATE-FSIM-RECONV).

### 4.3 Lift plan (DESIGN; crates per §6)

| Wave | Source (frankensim @ pinned commit) | Target | Why | Guard |
|---|---|---|---|---|
| 0 | `fs-math/src/det.rs` | `fmine-core::det` | Determinism lanes (PO-9) | golden hash of 10⁴ fixed inputs, FTX `frankensim_core_golden_hash` pattern |
| 0 | `fs-rand` Philox + Sobol/Owen | `fmine-core::rand` | Per-block, per-realization reproducible streams (R1 §5 #6) | known-answer vectors |
| 0 | `fs-evidence` (closure of 3): `Evidence<T>`, `Certified<T>`, `Color`, `ValidityDomain`, `ModelCard`, typed refusals | `fmine-evidence` | Claim system, no laundering (E008); mapped to KK/KU/UU and evidence classes (E032), never to Measured/Indicated/Inferred | property tests: `compose` never promotes |
| 0 | `fs-ivl` predicates/expansion, `fs-math/eft.rs` | `fmine-geom::exact` | Robust orient3d for wireframes and shells | FTX `exact.rs` precedent; adversarial predicate corpus |
| 0 | `fs-sparse` (Csr/Coo, LDLᵀ+AMD, PCG, SaAmg) | `fmine-numerics::sparse` | Kriging systems, SPDE/GMRF, seepage | FTX `sparse/direct.rs` precedent; residual known-answers |
| 0 | `fs-eproc`, `fs-voi`, `fs-robust` (cvar), `fs-tropical` | `fmine-numerics::{eproc, voi, risk, tropical}` | QAQC drift and reconciliation e-tests; drill VOI; CVaR; critical path (E006) | per-module known answers |
| 1 | `fs-rep-sdf/src/vdb.rs` (`VdbGrid<T: Copy>`, 310 lines, std-only) | `fmine-geom::vdb` | Sparse block-model tiles (E005); the 10⁹ target is unbenchmarked, so `fmine` benchmarks it | golden digest of `iter_active` order; scale benchmark GATE-BM-01 |
| 1 | `fs-rep-mesh/src/winding.rs` (`winding_exact`, octree) with local `Point3/Vec3` | `fmine-geom::winding` | Inside/outside on dirty wireframes | differential corpus vs the upstream test vectors |
| — | `fs-qty` | **not lifted** (nightly); `fmine-units` is built on stable Rust, modelled on it | E001 toolchain | — |
| — | `fs-uq::KlExpansion`, `fs-assimilate` (EnSRF), `fs-matdb` | Pattern only, or Wave 2 after a Cx audit; FM emits `frankensim.matdb-manifest.v1` packs for geomaterials (R1 §4.7) | dense O(n³) / Belief ≤ 256 (E006) | — |

**Header contract** (from the FTX observed form, R5 §2.2): `//! Provenance: frankensim <path> @ <commit>; licence; Changes: …; Left out: …`. A registry, `registries/lifts.toml`, holds one row per file `{source, commit, sha256_at_lift, target, guard_test}`. `xtask check-lifts` fails CI on a header/registry mismatch.

**Re-convergence gates.**
- **GATE-FSIM-DRIFT** (quarterly): diff each lifted source at frankensim HEAD against its pinned commit. Any upstream change inside a lifted region opens a bead within 10 working days, and the decision (re-lift, or a no-op with reason) is recorded.
- **GATE-FSIM-RECONV** (precondition for any Cargo dependency, all must hold):
  - frankensim pins asupersync by exact rev equal to the suite pin;
  - `cargo check` is green on the consumed crates at that commit;
  - the consumed crates' toolchain matches FM's;
  - INT-FSIM-001 spike: a one-package-id posture closure.
  - Only then may `fmine-simbridge` be created, and only for capabilities with no lifted equivalent (candidates: `fs-airflow` network solve, `fs-conduction` seepage operator, `fs-solid` once Mohr-Coulomb exists upstream).

---

## 5. SQ4: FA integration contract

### 5.1 Hypotheses (channel)

| ID | Hypothesis |
|---|---|
| H4-A | **Embedded `fa::Book` / `AsyncBook`** inside FM processes |
| H4-B | **`fad` + `fa --json` robot CLI** invoked by one FM bridge at the node co-located with the book |
| H4-C | **fa-mcp** as the integration API |
| H4-D (forced third) | **Offline signed intent outbox.** FM writes canonical intent files that a finance operator (or a cron job) feeds to `fa post --intent … --dry-run` / `fa commit --plan …` |

### 5.2 Refuter-first

- **H4-A, refuted for production:**
  - The facade is "still moving" (R4 §2.7).
  - It couples FM's whole closure to FA's fgdb rev and toolchain (E011).
  - It contests the writer lease that `fad` owns (S6).
  - It violates segregation of duties, because the operational system would host the book writer.
- **H4-C, refuted as a system channel:**
  - MCP is the agent surface. FA's bridge freeze blocks new tools (R4 §4.3).
  - A system integration must not depend on fastmcp's "MCP 2026-07-28 … unverified" (R2 §2.6).
  - fa-mcp is retained for human-supervised agent workflows (drafts, explanations).
- **H4-B falsifier:** a subprocess round trip is too slow, or the robot contract is unstable.
  - Test: FA-ACC-01 (schema/exit codes) and FA-ACC-09 (latency).
  - At the chosen granularity, about 300 publications per book per month (Card G1), even 10 s per call is under one hour.
- **H4-D on its own is too slow operationally, but it is the right deterministic fallback.** The same canonical intent bytes and idempotency keys flow through either path, so switching paths cannot double-post (PO-10).

**Verdict: H4-B primary, H4-D fallback, H4-A only in an isolated `tools/fa-conformance` workspace (its own Cargo.lock, so FA's pins never enter FM's closure), and H4-C for agents.**

### 5.3 Posting granularity and topology (DESIGN)

- **Book topology** follows R4 §7.2:
  - one FA book per (functional-currency group × primary basis);
  - the secondary basis and local statutory books run as *full* books until `AdjustmentLayer` is surfaced (V27);
  - JV-operator books are one per JV entity.
- FM's `registries/books.toml` maps `(site, legal_entity) → [book]`.
- The FA books live at the enterprise finance node, co-located with `fad` (S6). Regional nodes host in-country books where statutory residency requires.
- **Trunk granularity is one external-subledger summary per (site, flow family, month, book).**
  - Families: `ore` (ROM and stockpiles), `wip` (leach pads, circuit), `fg` (doré, concentrate, cathode), `cost` (absorption).
  - Close entries (DD&A, ARO accretion, NRV, MTM, royalties) are one per site-month-book.
  - Sales are per shipment or invoice.
  - Daily figures stay in FM. **Truck-, assay- and shift-level postings are prohibited** (Card G1 shows they fail on throughput and on human review).
- **Merkle-folded detail** (alien artifact A2):
  - FM aggregates each `(account, side, dimension tuple)` into one external detail row.
  - That row's `source_digest` is the **MMR root of the FM ledger slice** that folds to it, covering every daily movement and source certificate.
  - At most 128 rows, 32 legs and 8 KiB (S4). A family that exceeds this splits deterministically by `cost_center`, then by `product`, never ad hoc.
  - An auditor can descend from an FA leg to FM evidence by digest, and FA never stores FM detail twice (FA-INV-12).

### 5.4 Idempotency keys and the posting-slot automaton (DESIGN)

Key grammar: `fm1:<book>:<slot_kind>:<site>:<family|step>:<period>:g<gen>`, for example `fm1:BK-USD-IFRS:extsub:SITE-NV01:ore:2027-03:g0`. It uses FA's identifier alphabet `[A-Za-z0-9._:/-]` (`fa-journal` `identifier()`).

- `gen` increments **only** through a supersession record. Recomputing a slot never mints a new Original key.
- FA's identity rule is `vid(Entry) = H(book ‖ principal_scope ‖ idempotency_key)`.

```text
Planned ─prepare─▶ Prepared{workpaper_digest, payload}
  ─import─▶ Imported{doc_oid}            (fa evidence/document import; ImportReceipt Connector{franken_mining/metalacct, v})
  ─review─▶ Reviewed{attestation_vid}    (independent human principal; OB-REVIEW resolves)
  ─dry-run─▶ Planned@FA{plan_digest}     (fa post --intent --dry-run --plan-out)
  ─commit─▶ Committed{EntryRef, cert}    terminal-live
            │exit 3/7 ─▶ Refused{FA-REF-nnn}  (human triage; never auto-modify)
            │exit 6   ─▶ Replan (plan invalidated) | ALARM (IDEMPOTENCY_CONFLICT ⇒ FM bug)
            │timeout  ─▶ Indeterminate ─reconcile by key─▶ Committed | Planned
Committed ─FM recompute differs─▶ Drift ─▶ Replace (open period: Reversal+Replacement, g+1)
                                         └▶ Adjusting EstimateChange (closed period, new slot)
```

The Petri invariant (PO-4) is that each slot holds exactly one token, and at most one `Original` is live per slot.

### 5.5 Evidence grading: solving the self-sourced problem (DESIGN)

- Every summary entry cites three things:
  - (i) the FM Workpaper (grade `Generated`);
  - (ii) at least one **non-self-sourced** document, imported as `Connector{franken_mining/<module>, version}` or `ThirdPartySigned`: the month-end survey report, lab batch certificates and weighbridge totals;
  - (iii) the independent human `Reviewed{against: workpaper_digest}` attestation that the external-subledger law already requires (S4).
- FM never uses the `Agent` profile on trunk (S5). FM service principals use `Production`/`Preparer` profiles with caveats `entities ⊆ site`, `accounts ⊆ subtree`, `amount ≤`, `requires_dry_run` (R4 §7.10).
- The binding constraint is human review: about 12 Workpapers per site-controller per month (Card G1).
- Until FA's V19 lands (SoD binds asserted names), FM labels FA SoD evidence **HYPOTHESIS-grade** in its own control reports.

### 5.6 Interim posture while FA's P0 asks are pending (DESIGN; FM foundation-dependency rows `FD-FA-*`)

| Ask (R4 §9) | Interim (correct today, still correct after) | Switch-over rule |
|---|---|---|
| Metal/mass units | Money-only legs. Contained, recoverable and payable metal live in Workpaper rows and FM's flow ledger | From the first *open* period after FA ships balanced `XAU_OZ`/`CU_LB`/… rows, summaries add balanced commodity legs. No back-posting |
| Custom dimension kinds | Chart-leaf encoding (sustaining/growth CIP leaves, stockpile classes) + the 8-kind map (R4 §7.3); never `fund`/`grant` | New dimension values apply prospectively; the chart leaves stay valid |
| Custom REA event kinds | `PostEntry`/`PostBatch`; `RecordEvent` only for `Shipment`/`Sale` | Canon account-determination rules after `Custom(id)` lands |
| Tiered NCI / cross-book consolidation | FM's JV engine computes attributable figures and cites FA certificates; FA `consolidate` only for non-tiered groups | When fa-76hy lands, FM's computation becomes an oracle cross-check |

### 5.7 Acceptance tests FM runs against the pinned FA rev (`INT-FA-001` = all pass)

| ID | Test | Pass |
|---|---|---|
| FA-ACC-01 | Robot envelope `{"v":1,"event",…}` and exit dictionary 0,2–8 for every verb FM uses | Schema matches; `1` is never emitted |
| FA-ACC-02 | Identical `PostBatch` twice | `already_recorded=true`, same `EntryRef`s |
| FA-ACC-03 | Same key, different members | exit 6, nothing posted |
| FA-ACC-04 | External Workpaper with 128 rows / 129 rows / non-folding summary | accept / refuse / refuse |
| FA-ACC-05 | Generated-only evidence under the Agent profile on trunk | refused |
| FA-ACC-06 | Original into a Closed period; Adjusting with approval | exit 7 `FA-REF-010`; accepted |
| FA-ACC-07 | `fa replay` of the certified TB for FM-posted months | byte-identical |
| FA-ACC-08 | `fa bundle produce` → `fa-verify`; one-byte tamper | exit 0; exit 8 |
| FA-ACC-09 | Release build, book seeded to 10⁵ entries (`fa-gen`); 300 summaries | p99 ≤ 2 s/post and slope ≤ 0.02 ms per entry in book (TARGET) |
| FA-ACC-10 | After unit rows exist: an unbalanced metal leg | `FA-REF-001` |
| FA-ACC-11 | A second `fad` on the same book | `FA-REF-018`, exit 6; the bridge degrades to the outbox |
| FA-ACC-12 | Kill `fad` between dry-run and commit, 1,000 lab seeds | no duplicate; plan invalidated → replan; PO-10 holds |
| FA-ACC-13 (OPEN-B4-03) | `PostBatch` whose members are external-subledger summaries | accept, or document per-entry posting |

---

## 6. SQ5: workspace and crate map

### 6.1 Naming decision (FACT S7 → DESIGN)

- Crate prefix: **`fmine-`**. Binaries: **`fmine`** (CLI), **`fmined`** (node daemon), **`fmine-mcp`**, **`fmine-tui`**. Environment variables: `FMINE_*`. Format magic: `FMNE`.
- Plan prose may say "FM". Do not use `fm-`, `fm`, `fmd`, `fmn`, or `FM_` (frankenmermaid's installer variables).
- The owner may overrule this, but must then register the collisions as RISK rows (OPEN-B4-09).

### 6.2 Crate map: 39 crates, 7 layers

★ marks the 28 crates admitted for the 12-month demonstrator (each in a subset slice). A crate joins only with its first real slice (I-32). Budgets are Rust LOC including tests.

| Layer (code) | Crate | Responsibility (one line) | Budget k |
|---|---|---|---:|
| **S0 Substrate** (std-only; no runtime; lifted code lands here) | ★`fmine-core` | IDs/prefixes, anchors, codec kit (magic, LE, bounds, CRC, canonical order), lifted det/philox | 12 |
| | ★`fmine-units` | Exact quantities: t, dmt/wmt, oz t, lb, g/t, ppm, %, kWh/t; money boundary type; conversion laws | 10 |
| | ★`fmine-time` | Recorded vs valid time; shifts, rosters, fiscal calendars; TimeEvidence | 8 |
| | ★`fmine-evidence` | Lifted fs-evidence subset; evidence classes (E032); claim classes; refusal records | 14 |
| | ★`fmine-registry` | Registries-as-data loader + `registry-check` library | 8 |
| | ★`fmine-crs` | EPSG subset, mine grids (Helmert/affine with control), vertical datums; `UNREGISTERED` state | 15 |
| **S1 Runtime and authority** | ★`fmine-runtime` | Region tree, CapSet profiles, AppSpec per role, budgets, obligation registry | 12 |
| | ★`fmine-authz` | Macaroon caveats (first-party only at 0.5.0), MNPI walls, venture walls, residency, human authority tokens | 12 |
| | ★`fmine-ledger` | Delta batches, cells, leases + fencing, witnesses, narrow combiner, MMR, segment files | 25 |
| | ★`fmine-object` | CAS chunks, manifests, fanout-16 Merkle, root-last publication, GC by reachability | 15 |
| | `fmine-telemetry` | Site segment log, columnar codecs, ingest backpressure, rollups | 18 |
| | ★`fmine-sync` | Version vectors, consistency proofs, chunk anti-entropy, QUIC/ATP transport adapter, residency planner | 18 |
| | ★`fmine-lab` | LabRuntime harness, chaos profiles, FM oracles (dev-only) | 10 |
| **K Domain kernels** (pure; `Cx<cap::None>` only) | ★`fmine-geom` | Lifted VdbGrid, winding, exact predicates; meshes, block grids, sub-blocks | 30 |
| | ★`fmine-numerics` | Lifted fs-sparse, eproc, voi, cvar, tropical | 25 |
| | ★`fmine-drill` | Collars, surveys, desurvey, intervals, compositing, sample support | 15 |
| | ★`fmine-qaqc` | CRMs/blanks/duplicates, detection limits, e-process drift, lab certificates | 12 |
| | `fmine-geostat` | Variography, kriging, conditional simulation (PQ-1 owns method) | 40 |
| | `fmine-belief` | Subsurface belief objects, realization sets, block-model projections | 25 |
| | ★`fmine-flow` | Material-flow graph, mass/metal conservation laws, F-factors | 20 |
| | `fmine-plan` | Plan/schedule model, precedence, scenario labels | 18 |
| | ★`fmine-reserves` | Resource/reserve register model, QP attestation records, roll-forward | 12 |
| | `fmine-econ` | Price decks, cost models, NPV with exact money | 15 |
| | `fmine-geotech` | Slope/TARP laws, instrumentation semantics | 25 |
| **E Engines** (may SPAWN; seeded RANDOM; no IO) | `fmine-estimate` | Estimation run orchestration and certificates | 15 |
| | `fmine-optimize` | Pit limits (pseudoflow), schedule solvers (PQ-3) | 40 |
| | `fmine-decide` | VOI, DecisionCards over `franken-decision` | 15 |
| | ★`fmine-metalacct` | Metal-accounting subledger, costing, FA summary preparation and Merkle folding | 35 |
| **V Services/adapters** (only IO/REMOTE layer) | ★`fmine-io` | Quarantine-on-import parsers/writers: OMF, LAS, E57, GeoTIFF, DXF, vendor CSV dialects | 45 |
| | ★`fmine-projection` | Projectors to fsqlite (R-tree/FTS5), fgdb, frankensearch; high-water marks | 20 |
| | ★`fmine-fa-bridge` | Posting-slot automaton over `fa --json`; outbox; FA-ACC client side | 15 |
| | ★`fmine-service` | Node composition (roles, supervisors, leases) → binary `fmined` | 15 |
| **U Surfaces** | ★`fmine-cli` | Binary `fmine`; robot NDJSON; verb registry; doctor/repair | 30 |
| | `fmine-mcp` | Thin adapter over verbs (only fastmcp dependent); parity gate | 10 |
| | `fmine-tui` | frankentui ops console (inline mode, asupersync lane) | 15 |
| | ★`fmine-report` | franken_markdown + frankenmermaid (default features) reports with anchors and no_claim lines | 12 |
| **T Verification** (dev-only) | ★`fmine-oracles` | Oracle fixtures, known answers, metamorphic relations, conformance | 30 |
| | ★`fmine-gen` | Synthetic mines (drillholes, fleets, plants, books) | 15 |
| | ★`fmine-verify` | std-only offline verifier of FM certificates, bundles and MMR proofs | 12 |

**Totals.** Planned 738k; contingency 15%; **ceiling 850k** (1.0). Demonstrator slice: ≈ 340k planned, 390k ceiling, implemented as subsets of the ★ crates (their full 1.0 budgets sum to 502k). These sizes are calibrated on FA (≈793k, R4 §4.2) and FTX (324k actual for a narrower product, R5 §2.3). They are TARGETs.

### 6.3 Budget-overrun protocol (DESIGN; FTX lesson, E022)

1. `registries/loc_budget.toml` has one row per crate `{planned, ceiling = 1.15×planned, charter}`. `xtask loc` is std-only, counts every commit, and excludes generated fixtures, which are counted separately against a fixture-bytes budget.
2. **At 80% of planned**, `fmine doctor` warns.
3. **At 100%**, the next merge touching the crate needs a `BUDGET-AMEND-nnn` record: the new scope, the gate it serves, the new number and owner approval.
4. **At ceiling**, CI blocks merges to that crate except deletions, test fixes and refactors with net LOC ≤ 0.
5. **Program total above the ceiling** freezes new crates and frontier work ("frontier never borrows from the parity budget", R5 §3.20).
6. **No smuggling:**
   - `xtask check-scope` maps module families to each crate's `CONTRACT.md` charter, so a new domain cannot hide inside an existing crate (the FTX `ftx-pattern` anti-pattern, R5 §8).
   - The delivery log lives in `docs/delivery/`, never in the plan (FTX §25 bloat).

### 6.4 Layering rules (`xtask check-layers`, `check-caps`, `check-deps`; GATE-LAYERS-00 from commit 1)

- **L-1:** dependencies point downward `T→U→V→E→K→S1→S0` (T may reach anything; nothing depends on T in normal dependencies). `registries/workspace_topology.toml` declares same-layer edges in K and checks them acyclic.
- **L-2:** S0 is std-only plus lifted code. **Durable formats live in S0, so formats never depend on a runtime version.** That makes a future split posture (S2/S3) possible without format forks.
- **L-3:** K may use only `asupersync::{Cx<cap::None>, Budget, checkpoint}`. `check-caps` bans `open_child_region`, spawn, IO and time APIs in K and E (S3(a)).
- **L-4:** foundation confinement:
  - fgdb, fsqlite and frankensearch only in `fmine-projection` (plus fsqlite catalog use in `fmine-telemetry`);
  - fastmcp only in `fmine-mcp`; ftui only in `fmine-tui`;
  - franken_markdown and frankenmermaid only in `fmine-report`;
  - **no FA crate anywhere** (subprocess only).
- **L-5:** `check-deps` (deny.toml + `cargo tree` inversion) bans tokio, rusqlite, sqlx, arrow, parquet, polars, petgraph, chrono, time, hyper, reqwest, axum, ort/fastembed, tantivy (oracle lane only) and C solver features (HiGHS). It also requires **one package id** each for `asupersync`, `fgdb-types` and `fnx-classes` per posture closure (I-10).

### 6.5 Durable formats (`FMT-*`; hand-written, versioned, I-13)

| FMT | Content | Notes |
|---|---|---|
| FMT-LEDGER-1 | Delta batch `{cell, epoch, seq, prev_hash, anchor, deltas[], witnesses[]}` | canonical order; SHA-256 chain (aligns with ATP Merkle, `atp_architecture.md` l.160) |
| FMT-SEG-1 | Ledger segment file | magic `FMNE`+kind, LE, CRC32C per record, footer MMR peak list |
| FMT-CAS-1 | Chunk + manifest | 4 MiB default chunk; fanout-16 Merkle; root-last |
| FMT-TELSEG-1 | Columnar telemetry segment | per-column codec ids; bounds; min/max footer |
| FMT-BM-1 | Block-model projection | VDB tile layout + attribute columns + ModelCard digest |
| FMT-ANCHOR-1 | Anchor / high-water mark | FSS fields (S9) with `cell_id`, `residency_epoch` |
| FMT-SYNC-1 | Sync frames | version vector, consistency proof, chunk want/have |
| FMT-POST-1 | Posting slot + Merkle-folded detail | maps to FA `fa:external-subledger-detail:v1` rows |
| FMT-CERT-1 | FM result certificate | inputs closure, code rev, seed, output digest |
| FMT-ENV-1 | Robot NDJSON envelope | control/report JSON; serde permitted here only |

Readers accept versions N and N−1. Migration is a journaled rewrite into new CAS objects (never in place) with golden fixtures per version, a fuzz target per decoder, and a `fmine verify` reconstruction command.

### 6.6 ID prefix registry (never renumbered; tombstoned)

- **Document IDs:** `INV- GATE- INT- KILL- ADR- FMT- ERR- RISK- OPEN- OB- PO- DEC- EFFECT- SLO- COST- FD- BUDGET-AMEND- CELL-`.
- **Refusal codes:** `FMINE-REF-nnn`.
- **Runtime object prefixes:** `site_ cell_ node_ dh_ smp_ asy_ cert_ qc_ bm_ real_ pln_ sch_ rec_ rsv_ att_ dec_ post_ obj_ seg_ anc_ tok_ eff_`.

### 6.7 Surfaces (DESIGN, I-22)

- **`fmine` robot mode.** `--json` emits one event per line:
  `{"v":1,"event":"<schema-id>","node":…,"anchor":{"<cell>":seq,…},"data":…,"certificate":…,"flags":{"reserve_disclosure_claim":false,"execution_authorized":false,"geotechnical_signoff":false,"regulatory_compliance_claim":false},"warnings":[],"next_actions":[{"command","why"}]}`.
- **Exit codes** are a superset of `fa`/fgdb: `0` ok · `2` usage · `3` domain-law refusal (conservation, QAQC) · `4` environment/degraded · `5` foundation I/O · `6` conflict (lease, fencing, stale anchor, idempotency) · `7` safety/authorization (MNPI wall, residency, missing human token) · `8` verification failure · **`9` indeterminate** (an effect or post may have happened; reconcile).
- `1` is never emitted. Bare `fmine` prints triage. Also provided: `fmine robot-docs`, `fmine robot schema`, `fmine doctor`, `fmine repair plan|apply <digest>`.
- **`fmined`**: one per node. It holds the cell leases and serves `<node>/fmined.sock`. Spork AppSpec per role (§3.5).
- **`fmine-mcp`**: verb-registry projection with clusters of ≤7 tools. It is read-mostly; effects are `prepare_*` only. The parity gate covers verbs and tools.
- **`fmine-tui`**: ops console showing sync lag per cell, ingest ρ, obligation queues, posting slots and refusals. It adds no authority path.
- **Reports**: `fmine report` uses the franken_markdown library to render HTML/PDF and the frankenmermaid library for diagrams. Every report carries its anchor, certificate digest and a `no_claim` line.

---

## 7. SQ6: security and confidentiality (DESIGN)

Hypotheses:
- **H6-A:** perimeter/IT controls only, with FM trusting its callers.
- **H6-B:** Cx capabilities alone.
- **H6-C (forced third):** layered. Typed CapSet regions, macaroon caveats compiled before expansion (I-21), physically separate projections where caveat compilation is not yet live, and fail-closed registries.

H6-B is refuted by S3(a)–(c) at the pin. H6-A is refuted by MNPI and JV law: insiders need need-to-know walls, not just a perimeter. **Verdict: H6-C.**

| Concern | Mechanism | Fail-closed behaviour |
|---|---|---|
| **MNPI drill results** (E023) | Every `asy_`/intercept record carries `disclosure: Embargoed{wall, since} → Released{public_ref, at}`. The caveat `mnpi_wall=W-…` is required to read; there is **no global search or graph index over embargoed data** (wall-scoped projections only, so no count or absence leakage). Every access is an AccessEvent, giving an insider-list-ready per-wall log (OPEN-B4-07 for the jurisdictional list format). | Export, report, MCP and Avathon feeds refuse embargoed rows (exit 7) unless a human-held `mnpi_release` capability is cited |
| **JV partner walls** | Cells and projections are partitioned by `venture`. Partner principals get `venture=` and redaction caveats. Until fgdb Warden caveat compilation is live (R2 §2.2), **per-venture projection instances** (physical separation) are used instead of row filtering. Partners receive FA `.fab` bundles verifiable with `fa-verify` | A query spanning ventures without a cross-venture capability → refusal |
| **Country data residency** | `registries/residency.toml`: `(country, data_class) → {in_country_only, aggregates_exportable, exportable}`. The sync planner treats it as a hard constraint, and regional nodes hold in-country authority. `residency_epoch` sits in the anchor | Unauthorized class transfer → refused before bytes move (exit 7); doctor reports |
| **OT safety** | No write conduit to ISA-95 Levels 0–2. Ingest is pulled through a DMZ or a data diode. `EffectRegion` emits prepared `EFFECT-*` documents (work orders, advice) only. Even a prepared effect needs a **HumanAuthorityToken**: a first-party macaroon minted by a named human, with caveats `{effect_digest, site, expires ≤ 15 min, single-use nonce}`. `execution_authorized` is never set true (E023). Authorization refusals: unknown reactive ground → no ANFO plan; unknown PAG → no NAF dump plan; unknown moisture vs TML → no shipment release | Missing or expired token → exit 7; no fallback actuation path exists to fail open |
| **Capabilities via Cx** | CapSet rows per region (§3.4); purpose wrappers in the fgdb projection (`QueryCx` …). `check-caps` lint and runtime-mask tests (S3(a)) | Widening attempt → compile error or lab-oracle failure |
| **Macaroons at 0.5.0** | **First-party caveats only** (S3(c)). `MacaroonKeyRing` is always constructed with a random retired key, plus an FM wrapper that rejects all-zero signatures (S3(b)) with a regression test | Third-party caveat in an FM token → refused at mint |
| **Transport** | ATP and sync **only over QUIC/TLS**. The plaintext TCP transport is banned by `check-deps`/config (S3(d)). Chunk hashes are verified end to end regardless of transport | Plaintext attempt → refusal (exit 7) |

---

## 8. SQ7: industry interoperability inside the closed universe (DESIGN)

The rule (I-12): production code is lifted (with provenance and licence check) or reimplemented. Foreign tools are **pinned lab oracles** that capture fixtures and are never regenerated from the candidate (E022).

| Format / system | Posture | Oracle | Note |
|---|---|---|---|
| OMF v2 | Lift the omf-rust (MIT, E027) **schema/JSON layer only after a dependency audit** | omf-rust pinned build in the lab lane | HYPOTHESIS: OMF v2 stores arrays as Parquet in a zip, and omf-rust may link `arrow`/`parquet` (banned, L-5). If so, reimplement a minimal Parquet subset (plain + RLE/bit-packed levels) behind `fmine-io`. OPEN-B4-06 |
| LAS 1.4 | Reimplement (fixed binary) | PDAL/LAStools | — |
| LAZ | Lift a pure-Rust laszip decoder with provenance after a licence check, or defer | LAStools | decode first; encode later |
| E57 | Reimplement subset (XML + CRC'd binary pages; cartesian, intensity, pose) | libE57Format | — |
| GeoTIFF / DEM | Reimplement TIFF baseline + GeoKeys (none/deflate/LZW) | GDAL | — |
| DXF | Reimplement ASCII subset (LINE, (LW)POLYLINE, 3DFACE, POINT, layers) | ezdxf/ODA | DWG out of scope |
| CRS | `fmine-crs` EPSG subset + mine grids | PROJ | E010 gap |
| acQuire, Datamine, Surpac, Leapfrog, Deswik | **Dialect registry** `io_dialects.toml` over CSV/OMF/DXF/string-file exports; Surpac `.str` text reimplemented; Datamine `.dm` only via export | vendor exports as fixtures | quarantine-on-import; unit and CRS declared or `UNREGISTERED` |
| SAP S/4HANA | Read-only flat-file/IDoc extracts or OData JSON over asupersync HTTPS; **no RFC SDK**. The GL lives in FA, not SAP, inside FM's scope | sample extracts | SAP remains the ERP; FM↔SAP is snapshots |
| Pitram, Komatsu FrontRunner/Modular, Wenco | External collectors publish **content-hashed immutable snapshots** (R5 #50) into `ImportQuarantineRegion`; engines read snapshots only | vendor exports | FM never writes back to dispatch/AHS (§7 OT) |

**Coexistence with Avathon (E024).** Hypotheses:
- A: replace Avathon.
- B: FM feeds Avathon blindly.
- C (forced third): **FM as the anchor-pinned system of record; Avathon is a cognition-plane consumer** (I-20).

A is refuted: it is a political non-starter, since Avathon's lead director is Barrick's Chair. B is refuted: blind feeds would make Avathon a second authority without anchors.

**Verdict C**, with this contract:
1. Avathon reads per-venture **export projections** (`FMT-EXPORT`, anchor + certificate on every row). Every Avathon answer can cite an FM anchor.
2. Avathon writes back only as *evidence submissions* graded `AgentUpload`/elicited, `REVIEW_REQUIRED`, into a quarantine cell, never into authority cells.
3. MNPI and venture walls apply unchanged.
4. Avathon has no effect path.
5. Disagreement becomes `conflicted` knowledge state (E033), surfaced, not merged.

---

## 9. Alien-artifact compilation (selected families: 12, 35 + content addressing, 31; 05 only as the §3.3 gluing check)

EV screen: (Impact × Confidence)/Effort.

| Family | Impact | Conf. | Effort | Score | Admitted |
|---|---:|---:|---:|---:|---|
| 12 Concurrency/obligation automata | 5 | 4 | 2 | 10 | yes |
| Merkle/MMR certificates (35 + hashing) | 5 | 5 | 2 | 12.5 | yes |
| 31 Queueing capacity budgets | 4 | 4 | 1 | 16 | yes |
| 05 Descent/gluing | 2 | 4 | 1 | 8 | checker only |
| RaptorQ erasure tuning | 2 | 3 | 3 | 2 | no (consume ATP as is) |

**A1, protocol automata.** `registries/automata/{sync,posting,effect,ingest}.toml` holds states, labelled transitions, guards and the obligation each transition reserves or resolves (§3.5, §5.4).

CI checks:
- every state is reachable;
- no dead non-terminal state;
- every non-terminal state has a cancel edge;
- the Petri token-conservation invariant holds (reserve = commit + abort + live, per obligation kind);
- the transition log replays deterministically.

Runtime: transitions go through one `step(state, event) → Result<state, Refusal>` table, so there are no ad-hoc state flips.

**A2, Merkle/MMR certificates.**
- Each cell is a hash-chained log with an MMR over batch digests.
- `SyncCert{cell, epoch, from_seq, to_seq, mmr_root_before, mmr_root_after, consistency_proof}` proves that the replica is a prefix of the writer's log in ⌈log₂ n⌉ hashes (Certificate-Transparency style).
- CAS roots are fanout-16 Merkle trees.
- FA rows carry slice MMR roots (§5.3).
- **Checked null:** Merkle *set reconciliation* is **not** needed on the hot path. Append-only cells reconcile by version vector in O(#cells), and chunk manifests are tiny (Card G3). Merkle structures earn their keep as **proofs**, not as diff engines.

**A3, queueing budgets.** A per-site ingest model (M/D/1 with a Kingman correction for burst C_a), FA writer and human-reviewer servers, backpressure thresholds and monitors: the Little's-law audit (`L = λW` within 5%) on the `fmine doctor` page, and an arrival-drift alarm when the 5-min λ exceeds 1.3× the planned peak.

### 9.1 Proof obligations (template per ref 15)

| PO | Statement | Assumptions | Verification | Failure → fallback |
|---|---|---|---|---|
| PO-1 Single writer | ∀cell, at most one lease holder per epoch; receivers refuse lower epochs | monotone epoch persisted before lease use | LabRuntime DPOR over lease transfer + crash; fencing unit tests | split-brain → lower-epoch batches quarantined, conflict record, human resolution |
| PO-2 Prefix consistency | an enterprise replica of a cell is always a prefix of the writer's log | SHA-256 collision resistance | `SyncCert` verified on every apply; `fmine-verify` offline | proof fails → stop applying that cell, `stale`, doctor |
| PO-3 Root-last | no CAS root visible unless all chunks are verified | chunk hash check before stage-complete | crash at every stage boundary in the lab | partial set stays staged; OB-SEAL resumes |
| PO-4 Exactly-once posting | ≤1 live Original per FA slot; Petri token = 1 | FA idempotency (FA-INV-09) | FA-ACC-02/03/12; automaton check | exit-6 conflict → ALARM, freeze bridge |
| PO-5 Obligation closure | region close ⇒ every OB-* resolved or persisted durable | asupersync obligation tracking (advisory budgets, R2 §2.1) | leak oracle; durable-obligation replay | node will not report quiescent; doctor lists the obligations |
| PO-6 Conservation at commit | per flow node and period, Σin − Σout − Δstock = 0 within declared measurement tolerance, else refusal (no plug) | tolerances registered per measurement method | property and metamorphic tests (split/merge stockpiles) | refusal exit 3; reconciliation case opened |
| PO-7 Queue stability | ρ_ingest ≤ 0.7 at the planned P95 rate; P(queue > K) ≤ 10⁻⁶/h | timer-driven seals; measured S distribution | GATE-INGEST-01 load test; Little audit | degrade: coarser rollups with continuity receipts |
| PO-8 Non-escalation and noninterference | K/E crates never obtain IO/REMOTE; wall-scoped queries reveal nothing across walls (counts and absence included) | check-caps covers all call paths | lint + runtime-mask lab test + wall differential tests | build fails; projection quarantined |
| PO-9 Determinism | same inputs + seed + code rev ⇒ same certificate digest (debug/release; lab trace digest) | lifted det math; no HashMap iteration in outputs | determinism lanes in CI | digest drift → block release |
| PO-10 Bridge idempotency | replaying the outbox after any crash leaves FA state unchanged (`already_recorded`) | FA identity rule (S5) | FA-ACC-12; 1,000 seeds | indeterminate → reconcile by key, never re-key |

---

## 10. Galaxy-brain cards

**Card G1: FA posting capacity vs required summaries (15 sites × N books).**
- *Equation.* Per-post latency `t(n) = a + b·n`, where `a = 0.118 s` and `b = (0.439 − 0.118)/300 = 1.07 ms` per entry already in the book. This is FACT-derived from E019 (debug build), assuming linear growth (HYPOTHESIS).
- *Demand (DESIGN), worst case of all 15 sites in one primary book:* per month, 15 × (4 families + 16 close steps) = 300 trunk publications, plus about 150 shipments. That is n ≈ 5,400 per year.
- *Substituted:*

  | Year | n at year end | t(n) | Month-end window (300 posts) |
  |---|---:|---:|---:|
  | 1 | 5,400 | 5.9 s | 30 min |
  | 3 | 16,200 | 17.5 s | 87 min |
  | 5 | 27,000 | 29 s | 145 min |

  - Per-truck posting (30,000/day) costs Σt ≈ 30,000·0.118 + b·30,000²/2 ≈ 4.85·10⁵ s of writer time on **day one**, so ρ ≈ 5.6. Infeasible.
  - Daily site-family posting fits the writer (≈29 min/day in year 1) but needs 60 independent reviews per book per day. For a site controller with 8 h/month allotted at 10 min/review, ρ_review ≈ 7.5, so it is infeasible. Monthly posting gives 12 reviews per site per month, ρ_review ≈ 0.25.
- *Intuition:* the binding server is the **human reviewer** that FA-INV-12 requires, not FA's writer. FA's O(n) growth is tolerable at monthly granularity even in debug.
- *Assumptions:* linear growth; review at 10 min/Workpaper; 3 books per site. A release build is plausibly ≥10× faster (HYPOTHESIS, unmeasured).
- *What would change the decision:*
  - FA-ACC-09 slope > 0.02 ms/entry in release → shard books by site group, or KILL-1.
  - FA admitting a delegated (non-human) independent reviewer with authenticated SoD (V19) → daily trunk becomes possible.

**Card G2: telemetry ingest. The fgdb authority is falsified by arithmetic; the segment log passes.**
- *Owned segment log.*
  - λ_msg = 10⁵/s × 200 B = 20 MB/s. Seal every 250 ms, so λ_seal = 4/s.
  - S = encode 5 MB at 400 MB/s (12.5 ms) + compress (≈12 ms) + fsync (2 ms, TARGET for rugged NVMe) ≈ 27 ms.
  - ρ = 4 × 0.027 = **0.11**. Kingman Wq ≈ (ρ/(1−ρ))·((C_a²+C_s²)/2)·S = 0.124 × 0.25 × 27 ms ≈ **0.8 ms**.
- *fgdb as authority* (one publication per message, two fsyncs, no group commit, E013/E019): μ ≤ 1/(2 × 2 ms) = 250/s, so **ρ = 4 at 10³ msg/s and 400 at 10⁵**. It is unstable at the *bottom* of the stated range.
- *Upstream:* raw 160 Mbit/s against a 10–50 Mbit/s uplink is 3–16× over capacity. One-minute rollups of 5,000 signals at 16 B ≈ 0.12 GB/day (≈1% of a 10 Mbit/s link).
- *Intuition:* telemetry must be site-sovereign, batched into segments, and summarized upstream.
- *Assumptions:* message size 200 B; fsync 2 ms; timer-driven seals (C_a ≈ 0).
- *What would change the decision:* measured S > 175 ms at 10⁵ msg/s, giving ρ > 0.7 → KILL-5 (edge pre-aggregation, declare a lower supported rate).

**Card G3: satellite sync and the Merkle checked null.**
- *Catch-up after a 7-day partition:* (0.05 GB deltas + 0.12 GB rollups) × 7 = 1.2 GB. At 10 Mbit/s with 50% availability that takes 1.2·10⁹·8 / 5·10⁶ ≈ **32 min**, inside the GATE-SYNC-02 budget of ≤2 h.
- *Anti-entropy per round:* 12 cells × (24 B version-vector entry + ⌈log₂10⁷⌉ = 24 hashes × 32 B) ≈ **9.5 KB**.
- *Block model:* 10⁸ blocks × 20 attributes × 4 B = 8 GB = 2,048 chunks.
  - The manifest is 64 KB. A Merkle diff with 30% changed (d = 614) costs ≈ d·depth·fanout·32 B = 614 × 3 × 16 × 32 ≈ **0.94 MB, worse than sending the manifest**. Merkle diff wins only if d ≪ N/48.
  - Moving 2.4 GB of changed chunks takes 32–64 min. RaptorQ over lossy GEO adds ≈ 2–5% via ATP, consumed as is.
- *Intuition:* use version vectors for logs and plain manifests for objects. Merkle/MMR is for **proofs**.
- *What would change the decision:* telemetry segments (N ≈ 10⁶) ever replicating, which would bring Merkle diff back into play; or residency rules forbidding even rollups.

---

## 11. Failure modes and deterministic fallbacks

| # | Failure | Detection | Deterministic fallback |
|---|---|---|---|
| F1 | Satellite link down for days | session timeouts; version-vector age | Site keeps full authority. Enterprise marks those cells `stale(age)`. Decisions needing fresh site data return exit 9 or refuse. OB-SYNC-ACK queues |
| F2 | `fad` down or lease held elsewhere | FA exit 6 `FA-REF-018` / socket error | Bridge appends canonical intents to the outbox (H4-D); replays later by the same keys (PO-10) |
| F3 | FA law refusal (exit 3/7) | robot refusal object | Slot → `Refused`; human triage; never auto-edit and re-key |
| F4 | Ingest overload | queue > K; ρ monitor | Seal early, downsample with a continuity receipt; raw ring buffer if disk allows; never silent drop |
| F5 | Projection corrupt or behind | doctor digest check; high-water mark | Discard and rebuild from the ledger; answers state the high-water mark; freshness-required queries refuse |
| F6 | Split brain (two writers) | epoch regression at a receiver | Fencing: quarantine lower-epoch batches, record a conflict; humans adopt or refuse |
| F7 | Torn segment after crash | hash-chain or CRC failure at open | Read-only degraded mode; `repair plan` truncates to the last verified record (evidence retained); never rewrite history |
| F8 | Lifted-code drift or bug upstream | GATE-FSIM-DRIFT; golden hash | Freeze at the lifted version, re-lift by bead; no dependency switch |
| F9 | Residency or MNPI breach attempt | planner/caveat check | Refuse before bytes move (exit 7); AccessEvent |
| F10 | Missing or expired human authority token | token verify | Refuse effect preparation (exit 7); no alternative path |
| F11 | asupersync pin mismatch in a closure | one-package-id CI gate | Merge blocked; GATE-PIN-06 coordinated bump only |
| F12 | Cross-cell reference dangling | gluing checker on anchor publish | Consumer state `not_observable`/`stale`; publication of the dependent claim refused |

---

## 12. Gates (plan-ready; thresholds are TARGETs until measured)

**Admission gates (`INT-*`).**

| Gate | Pass criteria |
|---|---|
| INT-AS-001 | The nine FSS-DD §2.9 criteria; no second runtime in any closure; one asupersync package id per posture |
| INT-AS-SEC-001 | Tests for S3(a)–(d): child-region type narrowing lint plus runtime-mask assertion; zero-signature rejection; no third-party caveats; QUIC-only transport |
| INT-FSQL-001 | FSS-DD §3.10 ten criteria, applied to the projection role; R-tree bbox query p99 ≤ 50 ms over 10⁷ intervals |
| INT-FGDB-001 | Projection-only: one high-water mark; certified replay; **measured benefit** over fsqlite + in-memory fnx on 3 named queries, else KILL-2 |
| INT-FSEARCH-001 | `quill` + `hash`/`model2vec` only; no `ort`/`fastembed` in the closure; wall-scoped indexes |
| INT-ATP-001 | QUIC/TLS only; chunk-receipt resume without the 0.6.0 journal; lab partition matrix green |
| INT-FA-001 | FA-ACC-01…12 green at the pinned FA rev |
| INT-FMCP-001 | Parity gate; fastmcp confined to `fmine-mcp` |
| INT-FTUI-001 | asupersync lane; inline mode; no write verbs that bypass the registry |
| INT-FMD-001 | franken_markdown library rendering; reports deterministic by digest |
| INT-FMERM-001 | frankenmermaid default features; no HiGHS/C toolchain in the closure |
| INT-FSIM-001 | Separate-clone spike; one-package-id result recorded (expected FAIL today, S2) |
| INT-FNX-001 | fnx at fgdb-prism's rev; scale benchmark on synthetic precedence graphs (PQ-3) |

**Convergence gates (`GATE-*`).**

| Gate | Threshold |
|---|---|
| GATE-LAYERS-00 | check-layers / check-caps / check-deps / check-lifts / check-scope green from commit 1 |
| GATE-STORE-01 | Ledger → fsqlite projection rebuild of 10⁵ holes / 10⁷ intervals ≤ 15 min on the reference site node; as-known-at and bbox answers from one anchor |
| GATE-SYNC-01 | 1,000 seeded lab runs (link drop, crash at every boundary): 0 divergence, 0 duplicate applies, 100% `SyncCert` verified, 0 obligation leaks |
| GATE-SYNC-02 | 7-day partition catch-up ≤ 2 h at 10 Mbit/s, 50% availability |
| GATE-INGEST-01 | 10⁵ msg/s sustained 1 h: ρ ≤ 0.7, 0 unreceipted loss, p99 ingest-to-durable ≤ 2 s |
| GATE-POST-01 | 3 sites × 3 months synthetic close through `fad`: 0 duplicate, every month `fa replay`-identical, every Workpaper ≤128 rows |
| GATE-BM-01 | Lifted `VdbGrid`: 10⁸ active cells × 8 B payload, resident ≤ 2.5× payload, deterministic `iter_active` digest |
| GATE-PIN-06 | Coordinated asupersync bump: FM moves only when fgdb, FA and fastmcp have moved; re-run INT-AS-SEC-001 |
| GATE-FSIM-RECONV / -DRIFT | as §4.3 |
| GATE-BUDGET | §6.3 thresholds enforced in CI |

**Kill gates (with fallbacks that keep the product shippable and honest).**

| Kill | Trigger | Fallback |
|---|---|---|
| KILL-1 FA scale | FA-ACC-09 fails at the FA rev pinned by month 9 (p99 > 10 s at 10⁵ entries) | Monthly-only posting; ≤ 300 publications/book/month; quantities authoritative in the FM flow ledger; upstream bead. Never fork FA, never build a GL |
| KILL-2 fgdb projection | INT-FGDB-001 shows no measured benefit | Drop the fgdb projection; fsqlite tables + in-memory fnx serve the evidence graph |
| KILL-3 frankensim dependency | INT-FSIM-001 fails, or RECONV never holds | Lift-only, permanently; say so |
| KILL-4 owned ledger persistence | GATE-STORE-01 fails, or witness checks prove infeasible in memory | FSS ladder rung L2: fsqlite persistence under unchanged FM ledger semantics |
| KILL-5 telemetry rate | GATE-INGEST-01 fails at 10⁵ | Edge pre-aggregation; publish the supported rate honestly |
| KILL-6 OMF purity | omf-rust needs banned deps and the Parquet subset exceeds 8k LOC | OMF via lab-oracle conversion to CSV/DXF for interim exchange; say so |

---

## 13. Risks and open questions

| ID | Risk | Severity | Mitigation |
|---|---|---|---|
| RISK-B4-01 | 0.5.0 capability, macaroon and transport defects (S3) underpin FM security | High | INT-AS-SEC-001; GATE-PIN-06; never rely on Cx alone (§7) |
| RISK-B4-02 | Name collisions (`fm`, `fmd`, `fm-*`) (S7) | Medium | `fmine-` prefix; OPEN-B4-09 |
| RISK-B4-03 | Human review is the posting bottleneck (Card G1) | Medium | Monthly trunk; workflow design; upstream V19 |
| RISK-B4-04 | FA external-subledger caps (S4) force coarse summaries | Medium | Merkle-folded rows; deterministic splitting |
| RISK-B4-05 | The suite never converges on one asupersync; frankensim stays on a path dependency | Medium | Lift-only; formats in S0 keep a split-posture option open |
| RISK-B4-06 | Residency statutes per country unknown | High | Registry fail-closed; regional nodes; OPEN-B4-01 |
| RISK-B4-07 | Avathon positions itself as system of record | Medium (strategic) | Anchor-pinned export contract; evidence-only write-back |
| RISK-B4-08 | fgdb unreleased; projection churn | Low (projection only) | KILL-2 |
| RISK-B4-09 | OMF v2 Parquet dependency | Medium | KILL-6 |
| RISK-B4-10 | The ledger working set for the largest site cell exceeds node memory | Medium | Cell sharding by project or year; GATE-STORE-01 sizing |
| RISK-B4-11 | Licence rider (MIT + OpenAI/Anthropic) on lifted code and the substrate | OPEN | Owner decision (R2 RISK-11) |
| RISK-B4-12 | Budget overrun repeats FTX (2.7×) | Medium | §6.3 protocol from commit 1 |

**OPEN items.**
- **OPEN-B4-01:** data-residency and mining-data export statutes per country of operation. Legal input is needed; no code assumption is made.
- **OPEN-B4-02:** who coordinates the suite's asupersync 0.6.x bump, and when. FM follows and never leads (GATE-PIN-06).
- **OPEN-B4-03:** does `PostBatch` accept external-subledger prepared members (FA-ACC-13)?
- **OPEN-B4-04:** the cause of FA's O(n) per-post growth: a defect or inherent? It decides KILL-1 risk.
- **OPEN-B4-05:** the switch-over rule if fgdb ships valid time, constraints and a catalog. Does the decision ledger or reserves register ever move to fgdb authority? The default is no (one mechanism).
- **OPEN-B4-06:** omf-rust's actual dependency closure.
- **OPEN-B4-07:** the MNPI insider-list format per listing venue (TSX/NYSE/proposed NA IPO, E024).
- **OPEN-B4-08:** will Avathon consume anchor-pinned feeds and accept evidence-only write-back?
- **OPEN-B4-09:** does the owner accept `fmine`/`fmined`, or prefer `fm` with registered collision risk?
- **OPEN-B4-10:** reference site node hardware (fsync distribution, RAM), which every TARGET above depends on.

*Checked nulls:*
- No context passes all of fgdb-authority criteria (a)–(e) plus single-writer *and* sits outside the delta ledger's reach.
- Merkle set reconciliation is not needed on the sync hot path.
- No frankensim crate needed for the 12-month demonstrator requires `fs_exec::Cx`.
