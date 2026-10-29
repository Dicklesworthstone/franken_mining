# FrankenMining

<div align="center">

[![Status](https://img.shields.io/badge/status-design%20plan%20%C2%B7%20scaffold%20in%20progress-orange)](#honest-status)
[![Runtime](https://img.shields.io/badge/runtime-asupersync%200.5.0-6f42c1)](#oracles-and-pins)
[![Core](https://img.shields.io/badge/core-Rust%202024-b7410e)](#architecture)
[![Plan](https://img.shields.io/badge/plan-320%20beads%20%C2%B7%20E001%E2%80%93E672-2ea44f)](COMPREHENSIVE_PLAN_FOR_THE_DESIGN_OF_FRANKENMINING.md)
[![License](https://img.shields.io/badge/license-MIT%20%2B%20AI%20rider-yellow)](LICENSE)

</div>

**FrankenMining is a system of record for how a mining company knows what it knows: from the first soil sample to
the board's capital decision, every number carries its evidence, its uncertainty and its track record.**

A miner's numbers are produced by a dozen systems that each pass along a single value and discard how sure they
were. The result is predictable:
- guidance that misses;
- reserves that reconcile badly;
- capital projects that overrun by a predictable amount;
- exploration budgets that cannot say what they have ruled out.

FrankenMining is built for integrated gold, copper, silver and tin producers.
- Every published metric is reproduced from cited sources.
- Reconciliation becomes a calibrated attribution of where metal goes.
- Every forecast is frozen before the outcome and scored after.
- Every material capital, reserve or dam decision gets a one-page DecisionCard. It shows:
  - the alternatives;
  - the probability-weighted outcomes;
  - the unknowns that would flip the call;
  - the cheapest way to learn more.

It never classifies a reserve, never signs a dam and never drives a truck. It makes the people who do those
things faster, better evidenced, and accountable to a track record.

It works alongside the systems a miner already runs (the ERP, fleet management, historians, laboratory and geology
tools), reading their exports and replacing none of them. It gives people and AI agents one checked, cited view
of the whole operation. See [Alongside your systems](#alongside-your-systems-and-a-nerve-centre-for-agents).

> **Not affiliated.** FrankenMining is an independent open-source project. It is not affiliated with, endorsed by
> or built for Barrick, Newmont or any other company named here. The demonstration lane uses public disclosures
> only: technical reports, quarterly Mine Stats workbooks and investor presentations, cited by file, page and
> cell.

---

## Honest status

**Status.** The design is written down and has been audited. The code is starting.
- [`COMPREHENSIVE_PLAN_FOR_THE_DESIGN_OF_FRANKENMINING.md`](COMPREHENSIVE_PLAN_FOR_THE_DESIGN_OF_FRANKENMINING.md)
  (about 8,900 lines) is the technical constitution.
- A Rust workspace scaffold (the S0 substrate, the first public-lane crates and the `xtask` layering checks) is
  being built in `crates/`.
- Nothing is *implemented* in the plan's sense (§0.2), which requires all of:
  - a pinned oracle;
  - a filled `CONTRACT.md`;
  - Claim-typed outputs;
  - a robot-CLI entry point;
  - a recorded gate PASS.

  Every capability below is DESIGN or TARGET until its gate says otherwise.

| Question | Answer |
|---|---|
| Is there a design? | **Yes.** §0–§39 plus Appendices A–J. Every load-bearing statement is labelled FACT, DESIGN, HYPOTHESIS, TARGET or OPEN, and every FACT cites an evidence row |
| Is it evidenced? | **Yes, with a register.** [`planning/mor/MOR_CASE.md`](planning/mor/MOR_CASE.md) holds E001–E672. The corpus rows (E101–E399) were re-read by a second reader against the PDFs and workbooks: 273 exact, 18 confirmed with nuance, 3 page-locator fixes, 0 substantive errors. The research rows (E401–E655) still await a second reader |
| Has it been reviewed? | **Five audits and six hypothesis passes.** AUDIT_1–AUDIT_5 covered domain accuracy, suite fit, internal consistency, a corpus second read and a second-wave review of equations, guards and the bead graph. Brenner passes B1–B6 covered subsurface, prospectivity, planning and capital, substrate, wedge and pre-mortem, and operations, plant, tailings and workforce. Their corrections are folded in (plan Appendix H) |
| Is there a work plan? | **Yes.** 21 work packages and 320 beads with 533 dependency edges, acyclic; the critical path is 29 beads. Instantiated in `.beads/` for `br` |
| Is there code? | **A scaffold, in progress.** Nothing runs end to end yet. The first end-to-end target is `fmine assure public --issuer barrick` (M0, below) |
| Has anything been benchmarked? | **No.** Every speed and scale number in the plan is a TARGET |

---

## TL;DR

**The problem.** Mining runs on single numbers. A reserve is one tonnage at one grade, and a plan is one schedule.
A grade-control model silently replaces the resource model it was meant to test. Reconciliation factors are
reported but rarely attributed. Capex overruns average about +14%, with about half outside the claimed ±15%
(plan §1). The shocks that actually moved capital in 2025–26 sit outside single-entity spreadsheet models:
- host-state disputes and the loss and return of control of a mine;
- asset exits;
- JV defaults;
- an IPO carve-out.

**What already exists.** The methods are known: geostatistics, pit optimisation, metallurgical accounting,
Bayesian search and sequential testing. The enterprise systems are mature: ERP, fleet management, geological
modelling and mine planning.

**What nobody has composed.** One evidence substrate in which each of those numbers is a `Claim`:
- with its support and evidence class;
- with a calibration record and a typed unknown-unknown term;
- frozen and scored when it is a forecast;
- carried through reconciliation, accounting and the capital decision without losing any of that.

**FrankenMining** is that substrate and the engines that use it. It is written in Rust on the
[asupersync](https://github.com/Dicklesworthstone/asupersync) runtime. It reuses the
[FrankenSuite](https://github.com/Dicklesworthstone) siblings: frankensim for geometry and solvers, and
franken_accounting as the book of record. At a company that already runs an ERP, the ERP stays the statutory
ledger and franken_accounting reconciles to it.

---

## Alongside your systems, and a nerve centre for agents

A major miner has spent years, and often hundreds of millions of dollars, on SAP S/4HANA or another ERP, fleet
management, historians, laboratory systems and geology and planning tools. FrankenMining is designed to sit beside
them, not to replace them (plan §1.5, NONGOAL-001).
- **Read-only connectors.** ERP extracts (flat file, IDoc or OData), content-hashed fleet and historian snapshots,
  and modelling-tool exports. Nothing writes back to a transactional or control system.
- **The ERP stays the ledger.** franken_accounting rebuilds the ERP's trial balance from extracts and must agree
  with it, and every difference is a cited register item. Estimates that move money reach the controllers as
  workpapers for entries they post in their own ERP.
- **Incremental adoption.** The public demonstration needs no system access. A pilot needs a handful of read-only
  extracts, and each further source is one more registered dialect, never a migration.

On top of those systems, FrankenMining is designed to be the **agentic nerve centre**: the one place AI agents read
the operation from (plan §1.6).
- **One typed model of the business.** Sites, ventures, ore, metal, costs, plans, forecasts and decisions are typed
  Claims with units, ownership basis and provenance.
- **A robot-first surface.** One JSON envelope, typed refusals, `next_actions` and `robot-docs`, also served over
  MCP.
- **Safe by construction.** Agents read and prepare, and humans authorize. Agent forecasts are frozen and scored
  like anyone's, so trust in an agent is earned on its record.

The ERP keeps running the transactions. People and agents get one legible, checked and cited view of the business.

---

## How it knows what it knows

- **The universal Claim.** Every number that crosses a crate boundary is a Claim (INV-001). It carries:
  - support: point, interval, composite, block or SMU, and larger mining and accounting units;
  - evidence class: measured, derived, modelled (calibrated or not), elicited, transferred, assumed or asserted;
  - knowledge state and calibration flag;
  - validity domain;
  - provenance down to a sheet cell or a truck ticket;
  - exactly one admissible **unknown-unknown term**. There are three forms: a reference-class ε-contamination, a
    catch-all hypothesis with a reset floor, or a flip margin whose probability is typed as *not stated*.

  No probability is fabricated.
- **Known knowns, known unknowns, unknown unknowns.** A layered epistemic stack keeps:
  - what is measured;
  - what is modelled with a calibration record;
  - what is modelled without one;
  - what lies outside every model.

  An executive sees them on one page (plan §9).
- **Register before resolve.** Forecasts are hashed before their outcome exists and scored by proper rules after.
  Calibration is reported per family, segment and author, whether model, person or agent (§10). Anytime-valid
  monitors watch for drift: coverage e-processes, and a sequential PIT along the mining front that alarms in
  months rather than years (GB-08).
- **Refuse, never plug.** Conservation failures, unit or basis mismatches, averaging across supports and assay
  look-ahead are typed refusals (`FMINE-REF-*`), never silently clamped numbers.
- **Four time axes.** Every record carries:
  - when the physical state held;
  - when it was sampled;
  - when the result became visible to a decision-maker;
  - when the ledger committed it.

  A backtest therefore never sees an assay before its certificate existed (INV-016).

---

## Subsystems

| Subsystem | What it does | Plan |
|---|---|---|
| **Bedrock** | Append-only authority cells, content-addressed objects, certified sync, asupersync regions | §26 |
| **Plumb** | Claims, evidence classes, KK/KU/UU, the calibration ledger, surprise monitors | §7–§10 |
| **Lodestar** | Exploration: thinned point processes over exploration effort, typed negative evidence, drill-portfolio VOI | §11 |
| **Corebox** | Drilling, sampling theory, QA/QC as sequential evidence, assays | §12 |
| **Strata** | The subsurface belief ledger: regenerable, seeded realizations at a declared support | §13 |
| **Highwall** | Rock mass, slopes, ground control, TARP automata, pore-pressure sensitivity | §14 |
| **Pushback** | Plans with a three-gap certificate (optimization, sampling, calibration); ore mix at best unit cost | §15 |
| **Headframe** | The read-only operations twin: drill and blast, load and haul, autonomous-haulage queueing, energy | §16 |
| **Crucible** | Comminution, flotation, leach, refractory circuits, residence-time deconvolution, metallurgical accounting | §17 |
| **Freeboard** | Tailings evidence state (never "safe"), GISTM traceability, water balance, ARD, closure | §18 |
| **Muster** | Rosters, fatigue, critical-control verification, driver-based labour budgets | §19 |
| **Touchstone** | Reconciliation as a joint posterior over the material-flow graph; F-factors as posteriors | §20 |
| **Bullion** | Metal accounting over franken_accounting; AISC, AIC, C1, GEO and reserve replacement as versioned definitions | §21 |
| **Helm** | DecisionCards; one `CapitalProgram` over a shared scenario index, JV waterfalls, juniors and deal structures | §22–§23 |
| **Hallmark** | Resource and reserve register, QP/CP attestation, JORC/NI 43-101/S-K 1300 renderers | §24 |
| **Portal** | Vendor formats, CRS, quarantine on import, the public-filings lane | §32 |
| **Adit** | Robot CLI, daemon, MCP, TUI and reports | §30 |
| **Testpit** | Synthetic-truth mines and districts, pinned oracles, hindcasts, the offline verifier | §33 |

---

## The four claims

| Claim | Meaning | What establishes it |
|---|---|---|
| **Parity** | A published number or reference tool is reproduced from the same inputs | Pinned oracle fixtures; public filings |
| **Known answers** | Seeded truth is recovered on synthetic data, with calibrated intervals | `fmine-gen` replicates and coverage |
| **Reconciliation** | Forecasts agree with mined and processed reality, scored after the fact | The calibration ledger |
| **Performance** | A workload meets a target on named hardware | A benchmark with machine, data size and spread |

They are never averaged into one number. Reproducing a published AISC does not show the AISC is right. Recovering
a synthetic deposit does not show a real one behaves like the generator.

---

## Architecture

```text
 soil, geophysics, drilling ─► Lodestar ─► Corebox ─► Strata (belief ledger, regenerable projections)
                                                        │
                                  Highwall ◄────────────┤
                                                        ▼
 FMS, historian, LIMS exports ─► Headframe ─► Pushback (plans with three-gap certificates)
                                     │            │
                                     ▼            ▼
                                 Crucible ──► Touchstone (reconciliation posterior) ─► Bullion ─► franken_accounting
                                 Freeboard       Muster                                 Hallmark (register)
                                                   │
                     Helm: DecisionCards and the CapitalProgram over one scenario index ─► the board

 Plumb (Claims, KK/KU/UU, calibration ledger, monitors) and Bedrock (ledger, sync, runtime) sit under all of it.
```

Dependencies point downward only, checked by `xtask check-layers` from the first commit (GATE-LAYERS-00):

```text
T   Testpit: verification; reaches anything, depended on by nothing
U   Adit surfaces: fmine, fmined, fmine-mcp, fmine-tui
V   services and adapters: importers, projections, the franken_accounting subprocess
E   engines: seeded tiles and solvers; no IO, no clock
K   domain kernels K0–K4: pure functions under Cx<cap::None>
S1  runtime and authority: asupersync regions, ledger, CAS, sync
S0  substrate: std-only types and durable formats (FMT-*)
```

The design has hard boundaries:
- No process holds a write path to plant or fleet control (INV-011).
- Four envelope flags are false in every output and cannot be set: `reserve_disclosure_claim`,
  `execution_authorized`, `geotechnical_signoff` and `regulatory_compliance_claim` (INV-002).

---

## The wedge: Guidance & Reserve Assurance

The wedge pass (Brenner B5) tested seven candidate first products. The one that is useful with **no data** and decisive
with **some data** is assurance of the numbers that drive guidance and pay: AISC, GEO, reserve replacement and
grade and recovery by site (plan §37.1).

- **M0, Public Mirror plus Spine (Nov 2026–Jan 2027).** `fmine assure public --issuer barrick` will:
  - reproduce every per-site value in Barrick's published quarterly Mine Stats workbooks (2024Q1–2026Q2, plus
    Q3 2026 when it is released), each cell cited by `<file-period>:<Sheet>!<Cell>` and file hash;
  - reproduce every cut-off grade stated in a technical report;
  - keep a discrepancy register;
  - hash FY2026 per-site forecasts, with 80% intervals, *before* the February 2027 release.

  Its gate, GATE-BULL-01, requires ≥ 95% of numeric site-quarter values reproduced to published rounding, the
  rest explained or refused, zero uncited numbers and byte-identical reruns.
- **M1–M2.** Synthetic truth, reconciliation known answers, then a site assurance pilot in shadow mode: the
  12-month demonstrator.
- **M3–M6.** Group metric and reserve assurance with the accounting bridge. Then exploration and capital
  allocation, multi-site rollout with a tailings evidence state, and planning under calibrated uncertainty.

The program gate at month 12 re-scopes everything if no executive meeting has cited a FrankenMining number or
card (§34.5).

---

## Workspace and budget

The crate map has 59 crates in 7 layers, each with a LOC budget (`registries/loc_budget.toml`; ceiling 1.15× plan).
A crate joins the workspace only with its first real slice. The first slice covers:
- the S0 substrate: `fmine-core`, `-units`, `-time`, `-crs`, `-evidence`, `-claim`, `-registry`, `-array`;
- `fmine-numerics` and `fmine-calib`;
- the public lane: `fmine-io`, `fmine-public`, `fmine-metrics`;
- `fmine-decide` and `fmine-cli`;
- `xtask`.

The workspace runs under these rules:
- `unsafe_code = "forbid"` everywhere.
- asupersync is the only async runtime.
- tokio, arrow, polars, petgraph, rusqlite and C solver features are banned (plan §28.5).
- Numerical code lifted from [frankensim](https://github.com/Dicklesworthstone/frankensim) carries a provenance
  header per file.

---

## Command surface (planned)

```bash
fmine assure public --issuer barrick    # M0: reproduce, cite or refuse every published site metric
fmine claim show <id>                   # a Claim with its evidence class, knowledge state, UU term and provenance
fmine claim why <id>                    # why it has the colour, flip margin and staleness it has
fmine claim trace <id>                  # the provenance DAG back to source cells and ledger anchors
fmine doctor                            # hash chains, pins, LOC budgets, obligations
fmine robot-docs                        # machine-readable docs and NDJSON schemas for agents
```

Every verb is registered in `registries/verbs.toml`. Every command prints human text and, with `--json`, a stable
NDJSON envelope with documented exit codes. Agents get certified numbers or typed refusals, never prose.

---

## Oracles and pins

| Reference | Pin | Role |
|---|---|---|
| asupersync | `=0.5.0` | Sole async runtime (known defects and mitigations: plan §31.4) |
| Rust toolchain | `nightly-2026-08-31` | Suite-wide pin |
| frankensim | commit per lifted file (`registries/lifts.toml`) | Geometry, sparse solvers, exact predicates |
| franken_accounting | revision in `franken-stack.lock` | Book of record, reached as a subprocess |
| Issuer filings | sha256 per file in the source registry | Public-lane parity fixtures |

Fixtures are captured from references and never regenerated from the candidate. A mismatch is a defect until
proven otherwise.

---

## Data, confidentiality and privacy

- **No site data, MNPI or personal data in this repository.** Confidentiality is monotone (INV-007): MNPI-derived
  bytes never reach a public export, and there is no global index over embargoed data.
- **Ownership basis is part of every figure.** 100% project basis vs attributable, at the stated interest, for
  example NGM 61.5/38.5 or Pueblo Viejo 60/40.
- **Source documents are not committed.** The repository carries evidence notes that cite them by file, page and
  cell. One note is held back pending owner review (`planning/corpus/C5`), because its source carries a
  confidentiality notice.

---

## Scope boundaries

Not a control system. Not a reserve classifier, a dam sign-off or a compliance certificate. Not a general ledger.
Not a replacement for ERP, fleet management, geological modelling or mine-planning packages in years 1–2: it reads
their exports. Not a general MILP solver. Not an aggregate ESG score.

---

## Repository layout

```text
COMPREHENSIVE_PLAN_FOR_THE_DESIGN_OF_FRANKENMINING.md   the technical constitution
AGENTS.md                                               the operating contract for agents
planning/mor/MOR_CASE.md                                the evidence register (E001–E672)
planning/corpus/                                        evidence notes on the source documents (C1–C6)
planning/research/                                      research R1–R8 and primary-source passes W1–W5
planning/brenner/                                       hypothesis passes B1–B6
planning/mor/audits/                                    AUDIT_1–AUDIT_5
crates/                                                 the fmine-* workspace crates (scaffold in progress)
registries/                                             verbs, units, CRS, calendars, refusals, lifts, budgets, topology
docs/                                                   ADRs, delivery notes, CONTRACT template
.beads/                                                 the work graph (JSONL is truth)
```

---

## Reading paths

- **What and why:** this README, then plan §1–§5.
- **The epistemics:** §7–§10, then the galaxy-brain cards (Appendix D).
- **The first deliverable:** §21.6, §31.1 and §37.
- **A subsystem:** its section (§11–§24), its guards and proof obligations, then its beads in §39.
- **Implementing:** `AGENTS.md`, plan §28–§29, the crate's `CONTRACT.md`, then `br ready`.

---

## FAQ

**Is this a Barrick product?** No. It is independent, and it uses Barrick's public disclosures as its
demonstration data because they are detailed, public and auditable.

**Does it replace Leapfrog, Whittle, Deswik or the ERP?** No. It reads their exports and makes their outputs
evidence: cited, versioned, scored and reconciled.

**Will it tell me a dam is safe?** No. It reports the evidence state of each failure mode. Missing, stale or
conflicting instrumentation is shown as such, and humans sign.

**Why Rust and a custom runtime?** Every certificate must replay bit-identically offline (INV-010). One audited
async runtime with structured concurrency, cancellation and a deterministic lab makes that testable.

**Why so much planning before code?** The expensive mistakes in this domain are false certainty and wrong
architecture. Examples are a number that reaches a reserve disclosure without its uncertainty, or a substrate
that cannot scale to 10⁸–10⁹ blocks. The plan names its kill gates so those mistakes surface early.

---

## License

MIT with the OpenAI/Anthropic rider. See [LICENSE](LICENSE).
