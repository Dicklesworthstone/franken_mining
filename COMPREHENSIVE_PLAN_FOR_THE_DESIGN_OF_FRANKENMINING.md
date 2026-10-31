# COMPREHENSIVE PLAN FOR THE DESIGN OF FRANKENMINING

> **FrankenMining** (`franken_mining`; crates `fmine-*`; binaries `fmine`, `fmined`, `fmine-mcp`, `fmine-tui`)
> is an evidence-native, asupersync-based operating system of record for integrated gold, copper, silver and tin
> mining companies. It covers the chain from the first soil sample to the board's capital decision. Every number
> it emits carries its support, its evidence class, its calibration record, its unknown-unknown term and its
> provenance. Every forecast it makes is frozen before the outcome and scored after.

| Field | Value |
|---|---|
| Document | `COMPREHENSIVE_PLAN_FOR_THE_DESIGN_OF_FRANKENMINING.md` |
| Version | 0.1, first comprehensive pass. It goes to multi-model refinement rounds next (see §0.6). |
| Date | 2026-10-28 |
| Status | PLAN. An M0 scaffold of 15 crates exists in `crates/` (local commit `68e7091`); nothing in it is implemented in the §0.2 sense, and its M0 output is synthetic (Appendix K.2). Every capability statement in this document is DESIGN or TARGET unless it is labelled FACT. |
| Sibling plans mimicked | `franken_surveillance_system/COMPREHENSIVE_PLAN_FOR_FRANKEN_SURVEILLANCE_SYSTEM.md` (FSS), `franken_accounting/COMPREHENSIVE_PLAN_FOR_THE_DESIGN_OF_FRANKENACCOUNTING.md` (FA), `franken_textiles/COMPREHENSIVE_PLAN_FOR_THE_DESIGN_OF_FRANKENTEXTILES.md` (FTX) |
| Process artifacts | `planning/research/R1–R8`, `planning/mor/MOR_CASE.md`, `planning/brenner/B1–B7`, `planning/mor/audits/` (audit reports), `planning/corpus/C1–C6`, `planning/research/W1–W5`, `planning/mor/r7/` (round-7 audited review), `planning/duel/` (idea duel) (§0.6, Appendix K) |

---

# PART 0 — DOCUMENT CONTROL

## 0.1 Normative language and evidence labels

The words MUST, MUST NOT, SHOULD, SHOULD NOT and MAY follow RFC 2119. Every load-bearing statement carries one
of these labels.

| Label | Meaning | Who may change it |
|---|---|---|
| **FACT** | Verified in a cited file, commit, filing or publication. Code facts cite `repo/path:line @ commit`. Industry facts cite the R-report or the URL and date. | Only new evidence |
| **DESIGN** | A decision this plan makes. Each carries its rationale, and it may be revised by an ADR. | ADR |
| **HYPOTHESIS** | A testable belief. Each names the test that would confirm or kill it. | The named test |
| **TARGET** | A number to hit. It is not a measurement until a gate records the measured value. | Gate result |
| **OPEN** | Undecided or unknown, with a named owner and the decision rule in §36. | Owner decision |

Two plan-wide rules apply:
1. A number in this plan without one of these labels is a defect in the plan.
2. All galaxy-brain card values (GB-*) are illustrative unless tagged FACT.

## 0.2 What "implemented" means

A capability is *implemented* only when all seven of the following hold (after FSS §0):

1. Code exists in the crate the crate map (§28) assigns it to, within that crate's LOC budget.
2. Its CONTRACT.md has all ten sections filled (purpose, inputs, outputs, invariants, refusals, determinism class,
   evidence class of outputs, validity domain, oracles, known gaps).
3. Its known-answer tests pass against a **pinned oracle that was never regenerated from the candidate** (§33.2).
4. Its outputs are `Claim`s (§8) with evidence class, calibration flag and UU term. No naked numbers cross a crate
   boundary.
5. It has a LabRuntime test for every protocol it participates in (cancellation, crash, partition).
6. It is reachable from the `fmine` robot CLI with a documented NDJSON schema and exit codes.
7. Its gate (§34) has a recorded PASS with the measured values, not the targets.

Anything short of all seven is *partial*, and it MUST be described as partial in every status document.

## 0.3 Stable identifier prefixes (never renumbered; retired IDs are tombstoned)

| Prefix | Object | Prefix | Object |
|---|---|---|---|
| `GOAL-` | Goal | `NONGOAL-` | Non-goal |
| `NS-` | North-Star scenario | `BET-` | Thesis bet with falsifier |
| `INV-` | Invariant | `PO-` | Proof obligation |
| `NL-` | No-laundering rule | `G-` | Guard (deterministic check before render/export) |
| `GATE-` | Convergence gate | `INT-` | Integration/admission gate |
| `KILL-` | Kill gate with fallback | `ADR-` | Architecture decision record |
| `FMT-` | Durable format | `FMINE-REF-nnn` | Refusal code |
| `RISK-` | Risk register row | `OPEN-` | Open question |
| `OB-` | Durable obligation kind | `DEC-` | Decision record |
| `EFFECT-` | Prepared (never actuated) effect | `SLO-` | Service-level objective |
| `COST-` | Operation-cost registry row | `FD-` | Foundation dependency (sibling repo) |
| `WP-` | Work package | `M` | Milestone (M0–M6) |
| `BD-` | Plan bead (§39; mapped one-to-one onto `br` issues) | `GATE-DATA-` | Data-acquisition gate (§34.4) |
| `GB-` | Galaxy-brain card | `E###` | Evidence row in `planning/mor/MOR_CASE.md` (E001–E035 plan-wide; corpus blocks E101–E399) |
| `CELL-` | Authority cell class | `BUDGET-AMEND-` | LOC budget amendment |
| `REFCLASS-` | Reference-class record | `HYP-` | Geological/operational hypothesis in a HypothesisSet |

Short labels are local to the section that defines them: R0–R3 are reconciliation levels (§20.3) while R1–R8 are
research reports; F1, F2 and F3 are reconciliation factors (§20) while F1–F12 are Bedrock failure modes (§26.11)
and F1–F16 are pre-mortem stories (§35.1); S+n is a Strata-relative month (§13.12); C1–C6 are the corpus
reports (`planning/corpus/`), and FX-n are textbook known answers in C6.

Runtime object prefixes (DESIGN, from B4):
- **Core records:** `site_ cell_ node_ dh_ smp_ asy_ cert_ qc_ bm_ real_ pln_ sch_ rec_ rsv_ att_ dec_ post_ obj_ seg_ anc_ tok_ eff_ clm_ fc_ hyp_ eff_ dep_ trc_`.
- **Later subsystems:** `blast_ shot_ load_ haul_ eq_ wo_ plant_ strm_ tsf_ pz_ crew_ ccv_`.

## 0.4 Reading guide

The most important sentences in this document:

1. **FrankenMining never classifies, signs, certifies safety, or actuates.** Four flags are hard-wired false in
   every output envelope: `reserve_disclosure_claim`, `execution_authorized`, `geotechnical_signoff` and
   `regulatory_compliance_claim`. Humans sign, and the system supplies the evidence and the record of who signed
   what (§4, §8).
2. **The authority is evidence plus recipes.** Block models, realizations, plans, shells and statements are
   *certified projections* of that authority. They are regenerable from a snapshot by an independent party (§13).
3. **No naked numbers.** A number crossing any boundary is a `Claim`: value or distribution, support, evidence
   class, calibration flag, one admissible unknown-unknown term, validity and provenance (§8).
4. **Register before resolve.** Every resolvable forecast is frozen and hashed before its outcome exists, then
   scored with a proper scoring rule. Trust is earned by track record, by models, people and agents alike (§10).
5. **Reconciliation is the truth signal.** The one place every model in a mine meets reality is the material-flow
   reconciliation. It is a joint state estimate with uncertainty, not a ratio of two estimates (§20).
6. **Absence of evidence is evidence only where looking could have worked.** Negative exploration results enter
   only through a detection-thinning model with typed absence classes (§11).
7. **Every plan carries three gaps:** an optimization gap (dual bound), a sampling gap (scenario error) and a
   calibration gap (how wrong the inputs have been). Plans with unknown gaps are labelled as such (§15).
8. **FA is the book of record; FrankenMining is the operational and estimation truth.** Posting is monthly
   summaries with Merkle-folded detail, and a human reviews every Workpaper (§21).
9. **Consume, never fork.** Sibling repos are consumed at pinned revisions or lifted with provenance. No
   executive deliverable may be blocked by an unconverged sibling (§27).
10. **Start where trust is cheapest to earn.** The first product is *Guidance & Reserve Assurance*: certified
    metric reproduction, a reconciliation posterior, and a scored forecast ledger. The full twin comes last
    (§37).

How to read this document, by reader:

| Reader | Read |
|---|---|
| Executive | §1–§5, §23 (capital), §37 (milestones) |
| QP / CP / Head of MRM | §8–§10, §13, §20, §24, Appendix I |
| Engineer | Part II, Part IV, Part V, Appendices E and F |
| Auditor | §8, §10, §21, §24, §33, Appendices G–J |

## 0.5 Subsystem names

Named subsystems give each bounded context a short, memorable handle, as FA's Folio/Abacus/Almanac do. Each maps to a set of crates (§28).

| Subsystem | Bounded context | Primary crates |
|---|---|---|
| **Bedrock** | Substrate: delta ledger cells, content-addressed objects, sync, anchors, runtime regions | `fmine-core`, `fmine-ledger`, `fmine-object`, `fmine-sync`, `fmine-runtime`, `fmine-authz` |
| **Plumb** | Epistemic core: claims, evidence classes, KK/KU/UU, calibration ledger, surprise monitors, reference classes | `fmine-evidence`, `fmine-claim`, `fmine-calib` |
| **Lodestar** | Exploration and prospectivity: deposits, effort, thinned point processes, Bayesian search, drill-target VOI | `fmine-explore`, `fmine-prospect`, `fmine-infer` |
| **Corebox** | Drilling, sampling, QA/QC, assays, geochemistry, core logging, sampling theory | `fmine-drill`, `fmine-qaqc` |
| **Strata** | Subsurface Belief Ledger: geological interpretation, domains, spatial random fields, projections | `fmine-geom`, `fmine-geostat`, `fmine-belief`, `fmine-estimate` |
| **Highwall** | Geotechnics and ground control: rock mass, slopes, underground support, monitoring, TARPs | `fmine-geotech` |
| **Pushback** | Mine planning and optimization: shells, pushbacks, schedules, cut-off, stockpiles, blending | `fmine-plan`, `fmine-optimize`, `fmine-lp` |
| **Headframe** | Operations twin: as-built state, drill and blast, load and haul, equipment health, ventilation, dewatering | `fmine-blast`, `fmine-ops` |
| **Crucible** | Processing: comminution, flotation, leaching, CIL/CIP, refractory treatment, SX-EW, refining, geometallurgy | `fmine-plant` |
| **Freeboard** | Tailings, water and closure: TSF evidence state, GISTM traceability, water balance, ARD/PAG, closure | `fmine-tailings` |
| **Muster** | Workforce: rosters, competencies, fatigue, critical-control verification, productivity, budgets by driver | `fmine-workforce` |
| **Touchstone** | Reconciliation and assurance: material-flow graph, conservation, reconciliation posterior, F-factors | `fmine-flow`, `fmine-recon` |
| **Bullion** | Accounting: metal-accounting subledger, mining domain pack over FA, metric engine (AISC/AIC/C1/GEO) | `fmine-metalacct`, `fmine-acctpack`, `fmine-metrics`, `fmine-fa-bridge` |
| **Hallmark** | Reserves and disclosure: resource/reserve register, QP/CP attestation, code renderers, data-verification index | `fmine-reserves` |
| **Helm** | Decisions and capital: DecisionCards, VOI, the CapitalProgram, scenario index, real options, juniors | `fmine-decide`, `fmine-capital`, `fmine-econ` |
| **Portal** | Interoperability: vendor formats, CRS, quarantine-on-import, public-filings lane | `fmine-io`, `fmine-crs`, `fmine-public` |
| **Adit** | Surfaces: robot CLI, daemon, MCP, TUI, reports, viewer | `fmine-cli`, `fmine-service`, `fmine-mcp`, `fmine-tui`, `fmine-report`, `fmine-view` |
| **Testpit** | Verification: synthetic-truth mines and districts, oracles, hindcasts, offline verifier | `fmine-gen`, `fmine-oracles`, `fmine-verify`, `fmine-lab`, `fmine-derive` |

## 0.6 How this plan was produced (compact provenance)

The process is recorded so that refinement rounds can audit it. It is not a deliverable.

1. **Research (R1–R8):** eight surveys covering:
   - frankensim;
   - the runtime and substrate (asupersync, frankengraphdb, frankensqlite, frankensearch, fastmcp, frankentui);
   - the numerics, ML and perception repos;
   - franken_accounting fit;
   - FrankenTextiles patterns;
   - the industry landscape (Barrick, Newmont, OEMs, Avathon);
   - technical methods state of the art (prospectivity, geostatistics, planning, reporting codes, epistemics);
   - the numerical and symbolic stack (frankensympy, franken_numpy, frankenpandas, frankenscipy, frankenjax).
2. **Neutral case file:** `planning/mor/MOR_CASE.md`, with stable evidence IDs E001–E035 and seven pivotal
   questions.
3. **Brenner hypothesis passes (B1–B6).** Each pivotal question had at least three hypotheses, including a forced
   third alternative, plus falsifiers, cheap discriminating tests, refuter-first attacks, verdicts, and
   alien-artifact compilation (EV matrices, proof obligations, failure modes, galaxy-brain cards). The passes:
   - B1 subsurface representation;
   - B2 prospectivity and epistemics;
   - B3 planning and capital allocation;
   - B4 substrate and integration;
   - B5 wedge, sequencing and pre-mortem;
   - B6 operations, plant, tailings and workforce (`Headframe` §16, `Crucible` §17, `Freeboard` §18, `Muster`
     §19; `planning/brenner/B6_operations_plant_tailings_workforce.md`).

   These were run as independent subagent "panes" in place of a live NTM tmux swarm. The verdict register is
   Appendix B.
4. **Idea-wizard:** 30 ideas winnowed to 5, then the next 10 (Appendix C).
5. **Modes-of-reasoning and primary-source audits (`AUDIT_1`–`AUDIT_5`).** Fresh-context auditors checked the draft
   for domain accuracy, suite fit and internal consistency (`AUDIT_1`–`AUDIT_3`). A second reader re-checked the
   corpus rows E101–E399 and the quarterly workbooks (`AUDIT_4`), and a second-wave review covered equations,
   guards and the bead graph (`AUDIT_5`). Their findings, and the corrections to them, are folded in (Appendix H).
6. **Corpus pass:** the owner's documents (Barrick Mine Stats workbooks, technical reports, site decks, the Reko
   Diq ESIA and reference textbooks; 18 PDFs, 4,299 pages) were converted to markdown and read by six agents
   against the PDF pages and xlsx cells. Their findings are folded into the body (Appendix I).
7. **Public-source research pass (W1–W5, 2026-10-29):** five agents answered the owner questions the corpus left
   open from public primary sources: every Barrick Mine Stats workbook Q1 2024–Q2 2026 (W1), host-state events
   (W2), data-residency statutes in 12 countries (W3), the M2 site and the data-access probability (W4), and 72 of
   the plan's industry figures (W5). Crate naming and the asupersync 0.6 question were checked directly. Findings
   are folded into the body (Appendix J).
8. **Round 7 (2026-10-29): audited review, assurance-chain pass and idea duel.** A modes-of-reasoning run with
   six reasoning passes and eight independent auditors checked the plan and the first M0 code (171 claims;
   `planning/mor/r7/`). Brenner pass B7 ran on a live NTM squad (Claude, two Codex, Gemini) to test the assurance
   chain against the real workbooks and the obligations of QPs, CFOs and IR (`planning/brenner/B7_*`). A
   three-model idea duel (Claude, Codex, Gemini) generated, cross-scored and defended 45 ideas
   (`planning/duel/`). The corrections, the new capabilities and the new beads are in Appendix K.

Refinement rounds by other models SHOULD use the planning-workflow review prompt against this file. Changes SHOULD
cite the § and ID they modify.

## 0.7 Plan hygiene rules (FTX lessons)

- The delivery log lives in `docs/delivery/`, never in this plan. FTX's §25 grew to 82% of its plan; that MUST NOT
  recur.
- Every crate has a LOC budget and a ceiling from day one. The budget-overrun protocol (§28.4) applies from
  commit 1. FTX overran its plan 2.7×.
- Data acquisition has its own gates (§34.4), because the best model without data access is a demo.
- Four claims are never conflated (§33.1):
  - **Parity:** reproduces a reference tool or published number;
  - **Known answers:** recovers synthetic truth;
  - **Reconciliation:** agrees with mined and processed reality;
  - **Performance:** meets speed and scale targets.

---

# PART I — THESIS

## 1. What the mining software stack gets wrong, and what FrankenMining is

### 1.1 The fossils

Today's stack is a chain of capable tools held together by one epistemic habit: **each stage hands the next a
single number and throws away how it knows it.** The tools support better practice; the habit is the industry's.
Documented practice is marked FACT (R7 §9.1). Product-specific characterizations are HYPOTHESIS, because there is
no authoritative public critique of named products (R6 §5.1).

| Stage | Fossil | Consequence |
|---|---|---|
| Estimation (Leapfrog, Vulcan, Datamine, Surpac) | **One estimated block model.** Uncertainty survives only as a resource-class label assigned afterwards. | Conditional bias is reported as kriging efficiency and slope of regression where it is reported at all, and then against the estimate, not against grade-control outcomes. NGM's NI 43-101 reports for Carlin, Cortez and Turquoise Ridge report neither and classify by drill-spacing geometry (FACT, E158, E162, E163). In one published case, Localised Uniform Conditioning (LUC) estimates regressed on grade control with a slope ≈ 0.1 (FACT[U], single study, page not pinned in the corpus; E029). Capping and domaining are hidden choices. |
| Strategic planning (Whittle, Deswik, XPAC, Evolution) | **One shell at one price; a deterministic schedule.** | Spatial schedule compliance is reported as low as 30% (FACT, E026). No plan states how far it is from optimal, nor how wrong its inputs have historically been. |
| Fleet and operations (DISPATCH, MineStar, FrontRunner, Pitram) | **Telemetry owned by the company that sold the machine** (E027). | One-way integrations. Cycle-time truth is siloed from the plan that assumed it. |
| Reconciliation (Reconcilor, spreadsheets) | **F-factors as retrospective fudge multipliers.** | F1 and F2 errors compensate each other. Mill head grade, itself a back-calculation, is treated as truth. Nothing feeds back into the next estimate (FACT, R7 §4.3). |
| Plant metal accounting (historians, AMIRA P754 balances) | **Rigorous, but it stops at the plant gate.** | Disconnected from the model that predicted the feed. |
| Finance (S/4HANA, XERAS, spreadsheets) | **Cost centres, not material flows.** | The sustaining vs non-sustaining split in AISC is a spreadsheet judgment (R6 §4.1). The same pay metric appears on different bases in one disclosure: Barrick's 2026 Circular gives 2023–25 reserve replacement as ~190% (gold ounces) and 504% (gold-equivalent ounces), and 2025 gold AISC as $1,423/oz (pay scorecard) and $1,637/oz (non-GAAP table) (FACT, E603, E604, E605, E607). |
| Capital allocation | **Spreadsheet DCF.** One-at-a-time sensitivities; risk buried in the discount rate. | Capex overruns average +14% and about half fall outside the claimed ±15% (FACT, Bertisen & Davis, R7 §9.1), with no institutional learning. The fiscal, security, JV and entity-perimeter shocks that actually moved capital in 2025–26 are absent from single-entity DCF models: Loulo-Gounkoto's loss and return of control, the Hemlo and Tongon exits, and a North American IPO carve-out beside the 61.5%/38.5% NGM JV (R6 §6.6; §23.2, §23.4). |
| Exploration | **Uncalibrated scores validated by random cross-validation.** | Absence of evidence is read as absence. "Not looked" and "looked and found nothing" are the same zero (FACT, E028). |
| Tailings | **Conformance checklists.** | An offline piezometer reads as "normal" (R7 §8.4). |
| AI operating layers (knowledge graph plus agents; the 2025–26 wave) | **Reasoning over inputs that carry no reconciliation state or version.** | An answer cannot cite the reconciled figure it used, and its forecasts are not scored against a published baseline (HYPOTHESIS, from B5 W1–W2). |

### 1.2 What is already solved, and what nobody has composed

Every hard sub-problem has been solved somewhere in isolation:
- geostatistical conditional simulation;
- AMIRA P754 metal accounting;
- Bayesian data reconciliation;
- reference-class forecasting;
- proper scoring rules and conformal coverage;
- Bayesian search theory;
- thinned point processes from ecology;
- pseudoflow for ultimate pits;
- the draft JORC requirement for mandatory reconciliation (E031).

**No shipping system composes them into one ledger** where:
- every number carries its support, evidence class, calibration flag and unknown-unknown term;
- every forecast is frozen before its outcome and scored after;
- every material decision cites both, with its alternatives and the conditions that would flip it.

**FrankenMining is that composition.** It is built on the frankensuite's evidence and runtime substrate:
- asupersync's structured concurrency, cancellation, obligations and deterministic lab runtime;
- fs-evidence's no-laundering colours and flip margins;
- franken-evidence and franken-decision;
- franken_accounting as the book of record;
- frankenscipy, franken_numpy, frankenpandas, frankenjax and frankensympy for numerics;
- frankensim's geometric primitives, sparse solvers and materials-database discipline.

### 1.3 The one-paragraph pitch (for a CEO)

> A miner's numbers are produced by a dozen systems that each pass along a single value and discard how sure they were.
> The result is guidance that misses, reserves that reconcile badly, capital projects that overrun by a predictable
> amount, and exploration budgets that cannot say what they have ruled out. FrankenMining is a system of record for *how
> the company knows what it knows*. It reproduces every published metric from cited sources. It turns reconciliation
> into a calibrated attribution of where metal goes. It freezes every forecast and scores it. It puts a one-page
> DecisionCard in front of every material capital, reserve or dam decision, showing the alternatives, the
> probability-weighted outcomes, the unknowns that would flip the call, and the cheapest way to learn more. It never
> classifies a reserve, never signs a dam and never drives a truck. It makes the people who do those things faster,
> better evidenced, and accountable to a track record. It reads the systems the company already runs (the ERP, fleet,
> plant, laboratory and geology tools) and replaces none of them, and it gives people and AI agents one checked,
> cited view across all of them (§1.5, §1.6).

### 1.4 What is new here (claims of novelty, kept honest)

The individual methods are known (checked null on novelty, B2 §2.6, B3). The contributions this plan claims, each
testable:

1. **A universal Claim object** that carries an admissible unknown-unknown term. There are exactly three
   admissible forms: a reference-class ε-contamination, a catch-all hypothesis with a reset floor, or a flip
   margin whose probability is *typed* as not stated (§9.3).
2. **Typed absence classes** (A0–A4) and two typed Bayesian-search modes (Cox vs single-target) for exploration,
   with partial-identification (Manski) bounds where effort is unrecorded (§11).
3. **Regenerable, certified subsurface projections.** Realizations are keyed random streams, rebuilt
   bit-identically, with support as a type and averaging refusals for non-additive attributes (§13).
4. **The three-gap plan certificate** (optimization, sampling, calibration) on every plan (§15).
5. **Reconciliation as a joint posterior.** It feeds hierarchical bias priors back into estimation and
   refuses non-identifiable attributions (§20).
6. **Merkle-folded accounting.** Each FA summary row carries the MMR root of the FrankenMining ledger slice that
   folds to it, so an auditor descends from a ledger leg to a truck ticket by digest (§21).
7. **One comparable capital decision object** (`CapitalProgram`) over a shared scenario index. Pay metrics are
   constraints with published shadow prices, and buybacks and streams are candidates (§23).

### 1.5 Alongside the systems of record, never instead of them

**DESIGN.** A major miner has spent years and, at group scale, often hundreds of millions of dollars (owner's
estimate, 2026-10-29) on its ERP (SAP S/4HANA, Oracle, JD Edwards, Pronto), fleet management, historians,
laboratory systems and geology and planning tools. Those systems run the transactions, the fleet and the plant.
Auditors rely on them, and the company's people know them. No such company will rip them out for a newcomer, and
it should not have to. FrankenMining is designed to add the layer they lack, not to replace any of them
(NONGOAL-001).

- **It reads; it never takes over.** Every connector is read-only. The ERP supplies flat-file, IDoc or OData
  extracts. Fleet and historian systems supply content-hashed snapshots. Modelling and planning tools supply
  exports (§32.2). Nothing writes back to a transactional or control system (§31.3). Nothing changes how those
  systems are run, audited or upgraded.
- **The ERP stays the statutory ledger.** At a company that runs an ERP, the ERP's general ledger remains the
  ledger of record. FA (§21) runs beside it as a reconciling analytical book: it rebuilds the ERP's trial balance
  from extracts and must agree with it, and every difference is a cited register item. Estimates that move money
  (NRV, depletion, ARO, royalties, survey true-ups) reach the controllers as Workpapers. The controllers post the
  entries in their own ERP.
- **What it adds is the evidence layer.** Each existing system passes along one value. FrankenMining keeps the
  rest:
  - where the number came from;
  - how sure it is;
  - how it reconciles with the other systems' figures;
  - how good past forecasts of it were (§1.1–§1.4).
- **Adoption is incremental.** The M0 lane needs no system access at all (§31.1, §37). The first private pilot
  needs a handful of read-only extracts, not an integration programme. Each further source is one more dialect in
  `registries/io_dialects.toml`, never a migration.

### 1.6 The agentic nerve centre

**DESIGN; capabilities are TARGET until their gates pass.** An AI agent at a mining company today faces a dozen
systems. They give different names to the same ore, the same truck and the same dollar, and the agent cannot tell a
measured figure from a guess. FrankenMining is designed to be the one place every agent reads the operation from.
It is a business operating system for analysis and decision support, running on top of the systems that execute the
business.

- **One typed model of the business.** Sites, ventures, ore, metal, costs, plans, forecasts and decisions are
  typed Claims with stable ids, units, ownership basis and provenance (§8). An agent that asks for an attributable
  cost per ounce gets the figure with its cells, its rounding state and its knowledge state.
- **A robot-first surface.** Every verb answers in one JSON envelope with typed refusals, `next_actions` and
  stable schema ids. `robot-docs` lets a fresh agent learn the surface from the tool, and the MCP server exposes
  the same verbs (§30.1–§30.3, GATE-ROBOT-01).
- **Safe by construction.** Agents read and prepare; humans authorize. Agent output enters quarantine as evidence,
  never as authority. Walls and confidentiality bind agents unchanged, and no conduit reaches control systems
  (§30.4, §31.3).
- **Trust earned on the record.** Agent forecasts are frozen and scored like anyone's (§10), so the company learns
  which agents to rely on, and for what.
- **Vendor-neutral.** Avathon or any other AI platform reads anchored export projections (§32.3), so every agent's
  answer can cite the same FrankenMining anchor.

The ERP keeps running the transactions. People and agents get one legible, checked and cited view of the whole
operation.

## 2. Bets, each with its falsifier

| ID | Bet | Falsifier (what would prove us wrong) | Tested at |
|---|---|---|---|
| BET-01 | **Reconciliation is the truth signal.** A calibrated reconciliation posterior is the highest-leverage object in a mine. | At two pilot sites, the posterior yields no attested material finding beyond the existing F-factor report. | M2 |
| BET-02 | **Scored beats persuasive.** Executives value a forecast with a visible track record over a better-looking model. | After four scored quarters, no guidance or capital discussion cites the calibration record. | M3 |
| BET-03 | **Assurance complements agents.** AI operating layers raise demand for a certified evidence substrate. | The customer and its AI partner decline a read-API integration, and sponsors say the knowledge graph suffices. | M2–M4 |
| BET-04 | **One definition, many renderings.** Versioned rule-sets for AISC, reserve replacement, GEO, C1 and reserve codes remove a recurring class of disclosure friction. | Finance shows the E025 bases are already bridged internally, at no measurable cost. | M0–M3 |
| BET-05 | **Honest uncertainty is adoptable** if the system never classifies, signs or decides (E023, E031). | QPs or counsel refuse to let the system's probabilistic records exist, even after the §9 and §24 mitigations. | M2 |
| BET-06 | **Synthetic truth buys time.** A synthetic-truth mine plus a public-data lane establish credibility before data access. | M0–M1 artifacts do not move the data-access decision. | M1 |
| BET-07 | **Narrow purity is affordable.** The wedge needs only primitives the suite can supply within 12 months. | M0 plus M1 slip by more than 50% because of missing infrastructure. | M1 |
| BET-08 | **Exploration is an options portfolio.** Effort-aware prospectivity plus deal structuring improves capital allocation at the margin. | A temporal hindcast in a mature public district cannot beat the USGS three-part parity baseline with calibrated intervals. | M4 |
| BET-09 | **Thinning beats pixels where it matters.** The thinned point process outperforms pixel ML in low-effort and deep-cover ground and under leave-one-terrane-out. | Pixel ML wins everywhere, including leave-one-terrane-out, in two districts (KILL-LODE-1). | M4 |
| BET-10 | **Regenerate, don't store.** Seeded realizations rebuilt on demand are cheaper and more auditable than stored ensembles. | Regulators or QPs require stored realizations for disclosed snapshots, *and* storage then dominates cost (OPEN-STRATA-4). | M3 |
| BET-11 | **Certified gaps beat optimal-looking plans.** Planners and boards prefer a plan with stated optimization, sampling and calibration gaps. | Planners reject gap certificates as noise after two planning cycles. | M6 |
| BET-12 | **One scenario currency.** A shared realization index across geology, price, cost, execution and country risk makes portfolio decisions coherent. | Correlated-scenario capital allocation selects the same portfolio as independent-NPV ranking in more than 90% of resamples (no decision change). | M4 |

## 3. Readers and their questions

| Reader | Their question | Where FrankenMining answers |
|---|---|---|
| CEO and board (E614) | "Which of my numbers can I trust, and which decision this quarter would change if I knew more?" | Epistemic map (§9.6), DecisionCards (§22), `CapitalEntityGraph` (§23.2) |
| CFO and controller | "Why did AISC move against guidance, which parts are controllable, and can I replay every published non-GAAP figure from source?" | Metric engine and causal waterfall (§21.6) |
| COO, technical leadership and site GMs (E671) | "Where is metal lost between the block model and the bullion: geology or execution?" | Reconciliation posterior (§20), Crucible (§17) |
| Head of Exploration (E671) | "Own drilling, earn-in, stake or walk away, and how sure are we? What have we actually ruled out?" | Lodestar (§11), capital (§23) |
| Head of Mineral Resource Management and QPs/CPs (E671) | "Does reconciliation confirm my model, does the evidence support my classification, and will this system ever speak for me?" | Strata (§13), Hallmark (§24); never classifies |
| Mine planner | "How far from optimal is this plan, how robust is it, and how wrong have my inputs been?" | Pushback (§15) |
| Plant metallurgist | "What feed is coming, how will it behave, and what is my real recovery with its uncertainty?" | Crucible (§17), Touchstone (§20) |
| Geotechnical engineer and Engineer of Record (EoR) | "What is the evidence state of each failure mode, and what is stale?" | Highwall (§14), Freeboard (§18) |
| Accountable Executive (tailings) | "Can I show the board the evidence behind every GISTM requirement?" | Freeboard (§18) |
| Superintendent and HR lead | "Do I have the right competent people, rested, on the right shifts, and are critical controls verified?" | Muster (§19) |
| Audit committee and external auditor | "Is every disclosed number reproducible, and who attested what, when?" | Bedrock (§26), Bullion (§21), the offline verifier |
| Counsel | "What does this create that is discoverable, and does it ever make a disclosure claim?" | §24, §31 |
| IT/OT security | "What leaves our network, what runs where, and can it ever command equipment?" | §31 |
| JV partners | "What do I see, and can I verify it without seeing my partner's private data?" | §31.2, FA `.fab` bundles |
| Investor relations and the disclosure committee | "Which of our published figures does an outside system find inconsistent, with what citation, and can we see it before anyone else does?" | M0 discrepancy register (§37.3), worded as unexplained differences between cited cells; disclosure sequencing (§31.1, K.3) |
| Compensation (remuneration) committee | "Are the scorecard metrics we pay on measured the same way every year, at the price deck and basis we fixed?" | Bullion metric definitions and the reserve-replacement and GEO bridges (§21.6), each with its filed page (K.3) |
| AI agents (internal or third-party) | "Give me certified, cited numbers, or a typed refusal I can act on." | Agent surface (§30) |

## 4. Goals, non-goals and anti-goals

### 4.1 Goals

| ID | Goal | Primary section |
|---|---|---|
| GOAL-001 | Every number crossing a module boundary is a `Claim` with support, evidence class, calibration flag, UU term, validity and provenance. | §8 |
| GOAL-002 | Every resolvable forecast is registered before resolution and scored by a proper rule. Calibration is reported per family, segment and author (models, people, agents). | §10 |
| GOAL-003 | Known knowns, known unknowns and unknown unknowns are represented natively with honest probabilities, or explicitly unpriced flip margins. There are no fabricated probabilities. | §9 |
| GOAL-004 | Regional prospectivity reasons Bayesianly about undiscovered deposits using spatial context, analogue terranes, deposit models, exploration effort and typed negative evidence. | §11 |
| GOAL-005 | The value of the next survey or drill program is computed as decision-relevant VOI, and the budgeted drill portfolio carries an approximation certificate. | §11 |
| GOAL-006 | Drillhole, sample, assay and QA/QC data are immutable, superseded never edited, MNPI-embargoed and conserved through compositing. | §12 |
| GOAL-007 | The subsurface authority is evidence plus a hash-addressed belief specification. Every block model, realization and wireframe is a certified, regenerable projection at a declared support. | §13 |
| GOAL-008 | frankensim's geometric primitives, sparse solvers, evidence colours and materials-database discipline are used, by lift with provenance until re-convergence, for block grids, wireframes, exact predicates, volumes, seepage, ventilation and wear. | §13, §27 |
| GOAL-009 | Geotechnical evidence state per failure mode, TARP automata and slope/stope design inputs are first-class. The system never signs off. | §14 |
| GOAL-010 | Strategic, tactical and short-term plans carry the three-gap certificate. Pseudoflow pit limits match a lab oracle exactly. | §15 |
| GOAL-011 | "Optimal ore mix at best unit cost" is computed by cut-off, stockpile and blend policy optimization under geometallurgical constraints, consistent with accounting truth. | §15.6 |
| GOAL-012 | An operations twin of record holds as-built geometry, drill and blast, load and haul cycles, equipment health, ventilation and dewatering state. It is read-only to OT. | §16 |
| GOAL-013 | Processing models (comminution, flotation, leach, CIL/CIP, refractory, SX-EW, tin gravity) predict throughput and recovery with uncertainty and reconcile to the metallurgical balance. | §17 |
| GOAL-014 | Tailings, water and closure: an evidence state for all GISTM requirements, water balance, ARD classification refusals, closure cost estimates linked to ARO. | §18 |
| GOAL-015 | Workforce: rosters, competencies, fatigue, critical-control verification, driver-based budgets and productivity, with forecastable schedule compliance. | §19 |
| GOAL-016 | Reconciliation: a joint posterior over the material-flow graph, conservation enforced at commit, F-factors as posterior quantities, attribution with refusals. | §20 |
| GOAL-017 | Accounting through franken_accounting: the 17-part mining domain pack, monthly Merkle-folded summary posting, human review, and acceptance tests against the pinned FA rev. | §21 |
| GOAL-018 | Reproducible, versioned metric definitions (AISC, AIC, cash cost, C1, GEO, reserve replacement, ROCE) with certified causal waterfalls. | §21.6 |
| GOAL-019 | DecisionCards for every material decision: alternatives, loss matrix, expected loss, regret, flip margins, VOI, monitors, kill gates and required attestations. | §22 |
| GOAL-020 | Executive capital allocation as one `CapitalProgram` over a shared scenario index: sustaining, growth, exploration plays, junior equity and earn-ins, M&A, streams, buybacks. | §23 |
| GOAL-021 | A resource/reserve register with QP/CP attestation workflow, code-agnostic renderers (JORC, NI 43-101, S-K 1300, CRIRSCO) and a data-verification evidence index. | §24 |
| GOAL-022 | Multi-site federation: single-writer authority cells, certified sync over intermittent links, residency-aware, MNPI- and JV-walled. | §26, §31 |
| GOAL-023 | A robot-first CLI, daemon, MCP and TUI with one verb registry. Agents get certified numbers or typed refusals. | §30 |
| GOAL-024 | Deterministic replay: the same inputs, seed and code revision give the same certificate digest. An auditor replays offline with `fmine-verify`. | §26, §33 |
| GOAL-025 | Synthetic-truth mines and districts exercise every engine with known answers, misspecification and control conditions. | §33.3 |

### 4.2 Non-goals

| ID | Non-goal | Reason |
|---|---|---|
| NONGOAL-001 | Replacing SAP S/4HANA, Pitram, DISPATCH/MineStar/FrontRunner, Leapfrog, Whittle or Deswik, at any horizon | FrankenMining is the evidence layer that reads their exports (§1.5). Replacement is neither the wedge nor the destination (B5 R-b1, R-c1). |
| NONGOAL-002 | Fleet, plant or any other equipment control | No write conduit to ISA-95 Levels 0–2 exists or will exist (§31.3). |
| NONGOAL-003 | Classifying resources or reserves; signing geotechnical or tailings safety; asserting regulatory compliance | E023, E031. These belong to humans. |
| NONGOAL-004 | Building a general ledger | FA is the book of record (E019). |
| NONGOAL-005 | Competing with AI operating platforms on computer-vision safety, predictive-maintenance or procurement agents | B5 F1. FrankenMining is what those agents cite. |
| NONGOAL-006 | A general-purpose commercial MILP solver | Mining problems decompose; domain decomposition with dual bounds wins (§15). |
| NONGOAL-007 | Full-physics multiphysics in year 1 (coupled hydro-mechanical TSF models, DEM comminution) | frankensim is unconverged (E001). Use correlation cards and lifted solvers with validity domains. |
| NONGOAL-008 | A hidden aggregate "ESG score" | R5 #45. Components are shown, never a composite. |

### 4.3 Anti-goals (things that would make the product worse even if requested)

- No probability without a calibration flag. No single number without an interval, an evidence class and a
  source.
- No dashboard-first product. The unit of delivery is a Claim, a scored forecast or a DecisionCard.
- No private-data forecast leaves the tenancy. No public-lane author holds private access (§31.1).
- No hindsight claims ("we would have saved project X") unless they are built bitemporally *as known at* the
  decision date.
- No optimizer that is "merely different" from its oracle (R5). Known answers first, then improvements with
  certificates.
- No silent retraining. Every model change after a surprise goes through an InvestigationCase (§10.5).
- No plug figures. Conservation failures are refusals, not adjustments (§20.2).

## 5. North-Star scenarios (before → after; TARGETs)

Each scenario is an acceptance narrative. The milestone in brackets is where it first becomes demonstrable.

- **NS-01 AISC waterfall [M0/M2].** Before: the CFO asks why gold AISC moved between periods, and FP&A returns a
  narrative citing "lower grades, fuel, royalties" three days later. After: a certified waterfall (grade,
  recovery, tonnes, price-linked royalty, fuel, FX, sustaining capex), descriptive on public data and causal only
  once the site's metallurgical balance is in (M2; §21.6). Each bar cites its evidence, and the whole
  waterfall replays offline. At M0, public data support only these bars for gold sites: tonnes, grade, recovery,
  the sold/produced ratio, royalties per ounce and the AISC-workbook components (cost of sales, depreciation,
  by-product credits, NCI, G&A, minesite exploration, sustaining capex and leases, rehabilitation accretion), with
  no residual plug. Fuel, FX and labour bars need site data and arrive at M2 (R7-C044, Appendix K.3).
- **NS-02 Reserve-replacement bridge [M0/M3].** Before: different reserve-replacement percentages sit in
  different documents. After: one page renders reserve replacement under each named basis from one cited
  roll-forward.
- **NS-03 Monthly reconciliation [M2].** Before: a five-day spreadsheet exercise. After: a posterior in hours,
  with F1/F2/F3 intervals. A gross error is localized to the mill sampler, and the metallurgist attests the
  finding.
- **NS-04 Guidance setting [M3/M5].** Before: ranges set without a track record in view. After: ranges derived
  from calibrated site intervals, shown beside the track record ("5 of the last 6 quarters inside the 80% band").
- **NS-05 QP support [M3].** Before: the classification rationale lives in the QP's head. After: per-domain
  evidence (data spacing, slope of regression, conditional-simulation probability, reconciliation record) sits
  beside the QP's signed act. A reconciliation section is drafted for the QP's review.
- **NS-06 Expansion card [M4].** Before: a single capex point estimate. After: a reference-class-shifted capex
  distribution, P(overrun > 25%), the VOI of further geotechnical drilling, and "what would change our mind".
- **NS-07 Junior deal [M4].** Before: a spreadsheet for one structure. After: earn-in, stake, option and
  walk-away are compared on the same prospectivity posterior and stage gates. Each card shows the implied
  resource-conversion rate in the junior's share price and its distance to Tier One.
- **NS-08 Hindcast honesty [M4].** "Would we have flagged this district's discoveries with what was known in
  2005?" The answer comes with its confidence interval and its count of positives, including a null if that is
  the truth.
- **NS-09 Dry holes, honestly [M4].** Before: three dry holes "kill" a target. After: Lodestar shows the target's
  probability fell from 0.20 to 0.06, *if* the holes could see the footprint, and to 0.09 under the detection
  uncertainty. Because the search is regional (Cox mode), no probability was moved elsewhere. The cheapest next
  move is a deeper hole on the eastern flank (GB-02).
- **NS-10 Stockpile truth [M2].** Before: survey, dispatch and weightometer are averaged. After: tonnage is
  admitted only when the three agree within tolerance. Otherwise its state is `conflicted` and an investigation
  opens.
- **NS-11 Price-deck change [M3].** Before: the reserve price deck moves and weeks of re-runs follow. After:
  reserves, DD&A inputs, impairment headroom and Tier-One filters recompute with new digests, and no stale
  caches survive.
- **NS-12 Surprise monitor [M5].** The anytime-valid monitor on F2 coverage trips, and a hypothesis-review case
  opens with the evidence attached. Nothing retrains silently.
- **NS-13 TSF evidence state [M5].** Before: a green dashboard. After: "4 of 11 failure modes monitored; 2
  piezometers stale more than 14 days; last independent review 7 months ago". The system never says "safe".
- **NS-14 Auditor replay [M3].** The external auditor verifies the AISC certificate and its FA bundle offline,
  without access to the live system.
- **NS-15 Agent query [M2].** An agent asks for metal in circuit last week. It receives a certified figure, or
  `FMINE-REF-201 (no claim in domain): mill sampler offline 2 days`.
- **NS-16 Plan with gaps [M6].** Before: "the optimal LOM plan". After: an NPV distribution, a dual bound 1.8%
  above the plan, a sampling standard error of 0.9%, and a calibration note that tonnes-to-mill forecasts have
  run 6% high for 8 quarters (the correction is applied and visible).
- **NS-17 Blast to mill [M6].** Before: fragmentation is anecdotal. After: each shot's design, as-drilled
  deviations, measured fragmentation and downstream mill throughput sit in one lineage. Powder factor changes
  are DecisionCards with measured outcomes.
- **NS-18 Feed forecast [M5/M6].** The plant manager sees next week's feed blend as a distribution (hardness,
  head grade, preg-robbing carbon, arsenic) with predicted throughput and recovery. Last month's prediction is
  scored beside it.
- **NS-19 Roster risk [M5].** Before: roster gaps surface on the day. After: Muster forecasts the probability of
  falling below minimum competent crew per critical role per shift, flags fatigue-rule violations before
  approval, and records critical-control verifications with evidence.
- **NS-20 Capital program [M4].** The board sees one `CapitalProgram`: sustaining, expansions, exploration
  stages, a junior earn-in, a stream as tail insurance, and buybacks. Each is valued on the same scenarios, with
  CVaR, selection stability across resampled scenarios, the shadow price of each pay-metric constraint, and the
  three flip conditions that would change the allocation.

---

# PART II — THE EPISTEMIC CORE (Plumb)

## 6. Semantic truth model

### 6.1 Planes (after FSS)

FrankenMining separates four planes. Most category errors in mining software come from collapsing them (DESIGN).

| Plane | What lives there | Who may write | Examples |
|---|---|---|---|
| **Authority plane** | Facts the company is accountable for: observations, attestations, decisions, approved plans, posted summaries. Single-writer authority cells (§26) and content-addressed objects. | Writers holding a cell lease, after laws and witnesses pass | An assay result, a QP attestation, an approved LOM plan version, a reconciliation close, an FA `EntryRef` receipt |
| **Cognition plane** | Everything inferred: belief projections, realizations, posteriors, forecasts, scores, rankings, agent analyses. Always derived and rebuildable from the authority plane plus recipes. | Engines and agents, as *Claims* citing authority | A block-model projection, P(≥1 deposit in tract T7), an NPV distribution, a drill-target ranking |
| **Effect plane** | Prepared actions on the world: work orders, dig-line advice, blast designs for approval, posting intents, data releases. **Never actuated by FrankenMining.** | Engines prepare; a human authority token releases (§31.3) | `EFFECT-blast-design`, `EFFECT-shipment-release`, `EFFECT-fa-post` |
| **Packet plane** | High-rate raw streams: fleet telemetry, plant historian tags, piezometers, GNSS, vibration. Site-sovereign, columnar, segment-sealed. | Ingest regions | Truck payload events, mill power at 1 Hz, piezometer readings |

**Agents are not a fifth plane.** An agent is a principal that reads the cognition plane, proposes Claims (as
`Assertion` or `Elicited`), opens investigations and drafts effects. It never writes authority directly and never
upgrades evidence (§30.4).

### 6.2 The value chain as a claim graph

```text
            ┌──────────────── Helm: CapitalProgram / DecisionCards (§22–23) ────────────────┐
            │                                                                               │
 Lodestar ──► Corebox ──► Strata ──► Pushback ──► Headframe ──► Crucible ──► sales/refinery │
 (prospect)   (drill,     (belief    (shells,     (blast,       (plant,      (Bullion §21)  │
  §11          assay,      ledger     schedules,   load-haul,    recovery,                   │
               QAQC §12)   §13)       cut-off §15) twin §16)     §17)                        │
                  ▲           ▲  ▲          ▲           │             │                       │
                  │           │  └─Highwall §14──┘      ▼             ▼                       │
                  │           │                     Freeboard §18 (tailings, water, closure)  │
                  │           └──────── Touchstone §20: reconciliation posterior ◄────────────┤
                  │                     (F-factors → bias priors back into Strata & Pushback) │
                  └── Hallmark §24 (register, attestations) ◄── Plumb §7–10 (claims, calibration)
                                                Muster §19 (people, CCV, budgets) feeds all ──┘
```

Every arrow is a set of Claims with provenance. The reconciliation loop is the system's spine, because it is the
only place where predictions from Strata, Pushback, Headframe and Crucible all meet measured reality. Its
posterior updates the bias priors of every upstream model (§20.5).

### 6.3 Time

- **Quad-temporal records (DESIGN).** Valid time and record time (the bitemporal core) are not enough in mining.
  A blasthole is sampled before the blast, assayed hours to days later and released after QA/QC; a core
  interval's umpire check can arrive weeks later. Collapsing these instants leaks assays into backtests that the
  decision-maker could not yet see (§15.7), or puts metal in the wrong accounting period (§20). Every observation
  and Claim therefore carries a `QuadTemporalStamp` (§25.4):
  1. `physical_valid_time`: the interval over which the physical state held (in-situ grade until extraction; a
     truck movement at its instant);
  2. `collection_time`: when the sample, survey pickup or sensor frame was taken;
  3. `availability_time`: when the certificate, QA/QC disposition or processed reading became visible to a site
     decision-maker (`availability_time ≥ collection_time`);
  4. `record_time`: when the ledger committed it, as `(cell, seq)` under an anchor.

  Typical lags are a HYPOTHESIS to be measured per site lab: blasthole fire assay 1–3 days, umpire and leachwell
  checks 2–3 weeks. Ownership, tenure and fiscal records have no sampling lag and carry only the bitemporal core.
- **Two query modes (DESIGN).**
  - `as_of_decision(t_dec, anchor)` serves hindcasts (§10, §11.6), dig-line backtests (§15.7), retrospective
    DecisionCards (§22) and MNPI release reasoning (§31.1). It keeps a record only if `availability_time ≤ t_dec`
    and `record_time ≤ anchor(t_dec)` (INV-016, G-TIME-2). An assay collected before t_dec whose certificate
    arrived after it is excluded.
  - `for_physical_period([t₀, t₁), anchor)` serves period reconciliation (§20) and the monthly close (§21). It
    groups movements, and assays that arrived after the period, by `physical_valid_time`, as known at the anchor.
  - Elsewhere in this plan "bitemporal" names the valid/record core, and "as known at T" means
    `as_of_decision(T)`.
- **Anchors.** An anchor is a vector of `(cell, seq, digest)` high-water marks (FMT-ANCHOR-1, adopted from FSS
  `LedgerAnchor` with `cell_id` and `residency_epoch`). Every Claim, report and certificate names the anchor it
  was computed at. A consumer that cannot resolve a cross-cell reference at its anchor reports `stale` or
  `not_observable`, never a guess (INV-012).
- **Calendars** are first-class (`fmine-time`): fiscal periods, production months, shift patterns,
  fly-in/fly-out (FIFO) rosters, blast windows and lab turnaround clocks.

### 6.4 Identity, generations and supersession

- Observations are immutable. A correction is a new record with a `supersedes` link and a reason code. History is
  never edited (INV-003).
- Derived objects are content-addressed (BLAKE3 for claim bodies, SHA-256 for ledger chains and CAS chunks, which
  aligns with ATP). Identity *is* content, plus the anchor and recipe that produced it.
- Generations: a belief specification, plan, price deck or metric definition has a version. Every Claim names the
  versions it depends on, and staleness is computed, not remembered (INV-013).

### 6.5 Core invariants

| ID | Invariant | Enforced by |
|---|---|---|
| INV-001 | No number crosses a crate boundary except as a `Claim`, or as a typed quantity inside a Claim-producing engine's private API. | Type system; `xtask check-claims` lint on public signatures |
| INV-002 | The four flags `reserve_disclosure_claim`, `execution_authorized`, `geotechnical_signoff` and `regulatory_compliance_claim` are `false` in every envelope. No code path can set them. | Envelope type has no setter; golden test |
| INV-003 | Authority records are append-only. Corrections supersede; nothing is edited in place. | Ledger API; hash chain |
| INV-004 | Each authority cell has exactly one writer per epoch, and lower-epoch batches are refused. | Lease and fencing (§26.3) |
| INV-005 | Conservation holds at commit for every material-flow node and period within the registered measurement tolerance; otherwise the commit is refused, never plugged. | `fmine-flow` laws (§20.2) |
| INV-006 | Evidence class, colour, knowledge state and calibration never upgrade through derivation (NL-1…NL-8). | `fmine-claim` algebra; property tests |
| INV-007 | Confidentiality is monotone: derived ≥ join of inputs. MNPI-derived bytes never reach a public export. | Taint lattice; export fuzzing |
| INV-008 | Every registered forecast has record time strictly before its outcome's valid time. | Ledger check at registration and at scoring |
| INV-009 | A `Refused` value never becomes a number downstream without a declared fallback. | Type-level plus tests |
| INV-010 | The same inputs, seed, code revision and determinism class produce the same certificate digest. | Determinism lanes in CI |
| INV-011 | No FrankenMining process holds a write path to ISA-95 Levels 0–2. | Architecture; `check-deps`; deployment review |
| INV-012 | Cross-cell references resolve at the consumer's anchor or yield `stale`/`not_observable`. | Gluing checker on anchor publish |
| INV-013 | A Claim whose dependency version (belief spec, price deck, metric definition, calibration record) is superseded is marked stale on read. | Dependency index |
| INV-014 | Support is part of a quantity's type. Averaging across supports, or averaging non-additive attributes, is refused. | `fmine-units` and `fmine-geostat` support types (§13.4) |
| INV-015 | A decision-facing Claim carries exactly one admissible UU term (§9.3). | Guard G-UU |
| INV-016 | No operational hindcast, forecast registration or short-term decision backtest (`as_of_decision(t_dec)`) may condition on an observation whose `availability_time > t_dec`, even if its `physical_valid_time <= t_dec`. | `QuadTemporalStamp` filter; guard G-TIME-2; PO-TIME-2 |

## 7. Evidence classes, knowledge states, and the no-laundering rules

### 7.1 Evidence class: *how* something is known

| Class | Meaning | Mining examples | Projected `fs_evidence::Color` |
|---|---|---|---|
| `Measured` | QA/QC-passed measurement with a characterized error model. Known, but revocable: a failed CRM cascades. | Fire assay inside a passing CRM batch; weighbridge ticket; survey pickup within tolerance | `Validated{regime: measurement envelope, dataset: QA/QC record}`. `Verified{lo,hi}` is reserved for interval enclosures such as mass-balance bounds. |
| `Derived` | A deterministic function of other claims | Composite grade; dry tonnes from wet tonnes and a moisture assay | `compose(inputs)` (weakest link) |
| `ModelledCalibrated` | Model output inside a domain where its forecasts have a scored track record | Block grade in a domain with 20 reconciled months | `Validated{regime: calibration domain, dataset: CalibRecordId}`; auto-demotes outside the domain |
| `ModelledUncalibrated` | Model output without a usable track record | A new domain's block grades; a greenfield recovery curve | `Estimated{estimator, dispersion}` |
| `Elicited` | Structured expert judgment (IDEA protocol), stored and scored | N90/N50/N10 deposit counts; a geotechnical engineer's prior on joint persistence | `Estimated` |
| `Transferred{aoa}` | Carried from an analogue with an area-of-applicability status | A recovery curve from a sister mine; prospectivity priors from another terrane | `Estimated` outside the AOA; `Validated` inside a validated regime's AOA |
| `Assumed` | Stated without support | A planning assumption with no basis yet | `Estimated{dispersion: ∞}` |
| `Assertion` | Unverified statement: an LLM extraction, an imported claim, an agent proposal | "Extensive RAB drilling" in a 1987 report; a junior's press release | Refused as a colour leaf until reviewed |

**Attestation is not an evidence class (NL-3).** A CP, QP, EoR, controller or ITRB signature changes a Claim's
`authority` field (who stands behind it). It never changes how well the thing is known. This prevents the most
dangerous laundering in mining, where "the QP signed it" becomes "it is known".

Do not use "Measured" as a label in user-facing reserve contexts without qualification (R5). It collides with the
resource category "Measured". UIs render the evidence class as **"Measured (instrument)"**, and the resource
category is always prefixed with the code: "JORC Measured".

### 7.2 Knowledge state: what the system may say about it now (FSS lattice)

| State | Meaning | Example |
|---|---|---|
| `known` | Evidence suffices at the claim's support and validity | Monthly mill feed tonnes from a calibrated weightometer |
| `estimated` | Model-based, with stated uncertainty | Block grade; P(≥1 deposit) |
| `unknown` | No admissible basis | Recovery of a never-tested lithology |
| `conflicted` | Admissible sources disagree beyond tolerance | Stockpile tonnage: survey vs dispatch vs weightometer |
| `stale` | The basis is older than its validity window, or a dependency was superseded | Block model built before the latest 40 holes |
| `not_observable` | The sensor or method could not have seen it | An offline piezometer; soil geochemistry under 60 m of transported cover |
| `redacted` | Exists, but the viewer lacks the capability | Embargoed drill intercepts (MNPI) |
| `indeterminate` | A flip margin is unbounded: the verdict can go either way | A compliance check against an unbounded unknown |
| `not_applicable` | The question does not apply to this subject | Preg-robbing for a copper-porphyry flotation feed |

`unknown` is not false. `not_observable` is not "no detection". `redacted` is not "absent" (FSS). Renderers MUST
show the state *instead of* a number when the state is anything other than `known` or `estimated` (guard G-EXEC).

### 7.3 No-laundering rules (each is a property-tested invariant)

| Rule | Statement |
|---|---|
| NL-1 | The colour of a derived claim is `fs_evidence::color::compose` over its inputs. It never exceeds the weakest input; headlines use `fs_robust::weakest_color`. |
| NL-2 | The evidence class of a derived claim is the weakest input class under the partial order Measured > Derived > ModelledCalibrated > {ModelledUncalibrated, Transferred, Elicited} > Assumed > Assertion. |
| NL-3 | Attestation is orthogonal. Signatures change `authority` only. |
| NL-4 | Calibration is not inherited. A derived claim is `Uncalibrated` until its own forecast family has resolved outcomes at its own support. |
| NL-5 | Knowledge states never upgrade under conjunction (unknown ∧ known = unknown). `redacted` stays redacted downstream. `not_observable` never becomes "absent". |
| NL-6 | Refusals are absorbing unless a declared fallback exists, for example a MapMark-degenerate mode or Manski bounds. |
| NL-7 | Confidentiality is a join: MNPI ∨ public = MNPI. |
| NL-8 | `franken_evidence::EvidenceLedger.calibration_score` is never hand-entered. It is computed from the claim family's calibration record as `1 − min(1, max(|cov80 − 0.8|/0.2, KS_PIT))` and is 0 when Uncalibrated. `fallback_active` mirrors `franken_decision::FallbackPolicy::should_fallback`. |

## 8. The universal Claim

### 8.1 Rust sketch (DESIGN; crate `fmine-claim`, std-only substrate layer)

```rust
#![forbid(unsafe_code)]
pub struct Claim {
    pub id: ClaimId,                      // BLAKE3 of canonical body; append-only; supersede, never edit
    pub subject: SubjectRef,              // DepositId | TractId | BlockSetId | DomainId | PlanPeriodId | StockpileId
                                          // | StreamId | EquipmentId | WbsId | TsfModeId | EstimateId | MetricId | CrewId
    pub quantity: QuantityKind,           // ContainedMetal{metal} | Tonnage{basis: Wet|Dry} | Grade(Compositional)
                                          // | Recovery | Throughput | Count | Probability{event} | Npv{deck} | UnitCost | Ratio ...
    pub unit: UnitSpec,                   // fmine-units: t, dmt, wmt, oz_t, lb, g/t, ppm, %, kWh/t, money(ccy)
    pub support: Support,                 // Spatial{Point|Interval|Composite|Smu|Panel|Bench|Stope|Polygon|Tract|Site}
                                          // × Temporal{Instant|Shift|Day|Month|Quarter|Year|Lom}
    pub additivity: Additivity,           // Additive{by: Mass|Volume|Metal} | NonAdditive{mix_rule: RuleId|Refuse}
    pub repr: Representation,
    pub epistemic: EpistemicStatus,       // KnownKnown | KnownUnknown(KuKind) | UnmodelledBudget
    pub evidence: EvidenceClass,          // §7.1
    pub color: fs_evidence::color::Color, // COMPUTED by fmine-claim::project_color; never caller-set
    pub state: KnowledgeState,            // §7.2; computed from validity, monitors, anchor
    pub calibration: CalibrationFlag,
    pub uu: UuTerm,                       // mandatory on decision-facing claims (G-UU)
    pub validity: Validity,               // ValidityDomain + price deck + technology + valid-time window + review_by
    pub provenance: ProvenanceDag,        // input ClaimIds; transforms {code rev, params, seed}; human acts; DocRefs
    pub confidentiality: Confidentiality, // join-semilattice {Public < Internal < Venture(V) < Mnpi(Wall)}; monotone taint
    pub authority: Authority,             // Computed{module, version} | Elicited{expert, protocol} | Attested{role, person, sig, scope}
    pub forecast: Option<ForecastRegistration>, // resolvable? source, horizon, scoring rule, segment, author
    pub time: QuadTemporalStamp,          // physical_valid_time, collection_time, availability_time, record_time (§6.3, §25.4)
    pub supersedes: Option<ClaimId>,
}

pub enum Representation {
    Closed(DistSpec),                                    // distribution family + parameters (fsci-stats backed)
    Ensemble { index_space: RealizationSpaceId, artifact: ContentHash }, // shared realization index (§9.5)
    Quantiles(Vec<(f64, f64)>),
    Interval { lo: f64, hi: f64, kind: IntervalKind },   // Enclosure | Credible{lvl} | Conformal{lvl, calib} | Manski
    Category(CodeClass),                                 // e.g. "JORC Indicated": a label, NEVER mapped to a probability
    Projected { value: f64, policy: ProjectionPolicyId, of: ClaimId }, // the point a book or disclosure must carry
    Refused(Refusal),                                    // NoClaimInDomain | NotObservable | NoUsefulBound(cause)
                                                         // | MnpiBarrier | NonIdentifiable | Stale | SupportMismatch
}
pub enum KuKind { Aleatory, Parametric, Structural { set: HypothesisSetId } }
pub enum CalibrationFlag {
    Calibrated { record: CalibRecordId, n: u32, cov80: f64 },
    Uncalibrated,
    Miscalibrated { record: CalibRecordId, bias: f64, inflation: f64 },
    NotCalibratable { reason: NotCalibReason },          // horizon > ledger history; unique event
}
pub enum UuTerm {
    ReferenceClass { epsilon: f64, shape: ContaminationShape, refclass: RefClassId },
    CatchAll { set: HypothesisSetId, floor: f64, refclass: RefClassId },
    FlipMargin { flips: Vec<fs_evidence::uncertainty::FlippingUnknown>, probability: NotStated },
    NotDecisionRelevant { justification: NonEmptyString },
}
```

### 8.2 Why each field exists (rationale, so later agents keep it)

- **`support` and `additivity`.** Most mining errors are change-of-support errors: a point assay treated as a
  block grade, block-average hardness treated as additive, or monthly totals scored per block. Making support part
  of the type turns these into compile-time or commit-time refusals (INV-014, §13.4).
- **`Projected`.** Books and disclosures need a point (FA posts one amount; a reserve statement prints one
  tonnage). `Projected` names the policy that chose the point (mean, P50, policy-defined conservative quantile)
  and keeps the distribution reachable through `of`. A point is never naked.
- **`Category`.** Resource and reserve categories are human classifications. Mapping "Indicated" to "±15% at 90%"
  would be a fabricated probability (E031). Categories are carried as labels, beside quantitative evidence, never
  converted.
- **`authority` vs `evidence`.** NL-3.
- **`forecast`.** Only registered forecasts can earn calibration. Unregistered model outputs are permanently
  `Uncalibrated` (NL-4).
- **`uu`.** Without it, every probability silently assumes its model is the whole world. §9.3 restricts how the
  residual may be stated.

### 8.3 Guards (deterministic checks before render, export or decision use)

| Guard | Passes when | On failure |
|---|---|---|
| G-UU | A decision-facing claim has exactly one admissible `UuTerm`, and any ε cites a `RefClass` | Refuse render: "unbudgeted unknowns" |
| G-EXEC | Not `Refused`, not `stale`; calibration shown verbatim; colour shown | Show the knowledge state *instead of* a number |
| G-DISC (disclosure-facing) | Evidence ∈ {Measured, Derived, ModelledCalibrated}; no `Assertion`/`Assumed` in provenance; every RefClass tagged verified; a CP/QP `Attested` record present | Block export. `reserve_disclosure_claim` stays false regardless (INV-002). |
| G-MNPI | Viewer capability ≥ claim confidentiality; `basis = Full` never crosses a wall | Serve the `basis = Public` twin, or `Refused(MnpiBarrier)` |
| G-ABS | A negative-evidence update happens only where effort has coverage ≥ Medium and detection q ≥ q_min | Treat as absence class A0/A1 (no update) or A4 (Manski bounds) |
| G-AOA | Dissimilarity ≤ d_max for any absolute probability transferred from analogues | Interval or refusal |
| G-SUPPORT | Aggregation respects support and additivity | `Refused(SupportMismatch)` |
| G-CONS | Material-flow commits conserve mass and metal within tolerance | Commit refused (exit 3); reconciliation case opened |

### 8.4 Refusal codes

Refusals are typed values, not errors. They render as plain sentences with a `fix` hint (fs-scenario
`Violation{code, what, fix}` pattern, `crates/fs-scenario/src/scenario.rs:279`).

| Code family | Example |
|---|---|
| `FMINE-REF-1xx` domain-law refusals | 101 conservation; 102 support mismatch; 103 non-additive average; 104 non-identifiable attribution; 105 basis mismatch (wet/dry, metal stage); 106 unregistered or incompatible CRS; 107 undeclared unit, basis or time zone on import (§25); 108 assay-availability lookahead (`AssayLagLookahead`; §6.3, G-TIME-2) |
| `FMINE-REF-2xx` evidence refusals | 201 no claim in domain; 202 not observable; 203 outside AOA; 204 unbudgeted unknowns; 205 calibration required for decision-grade use |
| `FMINE-REF-3xx` authorization refusals | 301 MNPI wall; 302 venture wall; 303 residency; 304 missing human authority token |
| `FMINE-REF-4xx` safety-semantic refusals | 401 unknown reactive ground → no ANFO plan; 402 unknown PAG → no NAF dump plan; 403 unknown moisture vs TML → no shipment release; 404 stale TSF instrumentation → no "within TARP" statement |
| `FMINE-REF-5xx` integration & sequencing refusals | 501 FA refusal relayed (`FA-REF-nnn`); 502 stale anchor; 503 sibling capability unavailable, fallback engaged; 504 adjacent paste/cemented backfill uncured or strength-degraded (`FillStrength(t) < UCS_design`) → no secondary stope blast plan (§16.3) |

## 9. Known knowns, known unknowns, unknown unknowns

### 9.1 Definitions (DESIGN; the vocabulary is load-bearing)

- **Known known (KK).** A claim whose evidence suffices at its support, *revocably*. A failed CRM or a recalled
  survey cascades it to `stale` or `conflicted` along the provenance DAG.
- **Known unknown (KU)** comes in three kinds:
  - `Aleatory`: irreducible variability at the stated support, such as block-scale grade variability given the
    model;
  - `Parametric`: uncertainty in parameters of a named model, such as variogram range, recovery-curve
    coefficients or detection efficiency;
  - `Structural{set}`: competing named models in a `HypothesisSet`, such as alternative fault interpretations,
    deposit types or failure mechanisms.
- **Unknown unknown (UU).** Mass outside every named model. It is never silently zero. It is budgeted with
  exactly one admissible form (§9.3), detected by surprise monitors (§10.4), and converted to KU through
  InvestigationCases (§10.5).
- **Unknown known.** Information the organization *holds* but a claim did not use: legacy reports, uncaptured
  effort records, core in the shed, a retired geologist's knowledge. It is computed as a set difference (§9.4),
  not remembered.

### 9.2 The layered epistemic stack (B2 verdict: the layered stack beats a single global posterior and credal sets)

Why not one global Bayesian posterior? Under misspecification the model cannot represent (M-open), a posterior
concentrates on the KL-nearest wrong model, and coverage *decays* as data grow. A single posterior also cannot
represent CP/QP/controller judgment (FACT, Grünwald & van Ommen 2017, R7).

Why not credal sets or Dempster–Shafer as the stored belief?
- Conditioning dilates intervals.
- Dempster–Shafer combination is paradoxical under conflict.
- There is no strictly proper scoring rule for imprecise forecasts, so they cannot be calibrated by a ledger.

Both survive in narrow roles:
- Bayes inside models (L1);
- an ε-contamination decision envelope for irreversible actions (L7);
- Manski bounds for unidentified inputs.

| Layer | Content | Update cadence | Artifact |
|---|---|---|---|
| L0 Measurement | QA/QC-passed measurements with error models; KK revocable | Per event | `Claim{epistemic: KnownKnown, evidence: Measured}` |
| L1 Within-model Bayes | Geostatistical simulation, point processes, recovery and cost models. Tempered posterior power η ≤ 1 where misspecification is suspected, with η calibrated on hindcasts, never hand-set. | Per data event | Ensembles on a shared realization index |
| L2 Across-model | **Stacking** over KU-structural alternatives, plus a catch-all h₀ with a floor. BMA only with an explicit M-closed justification. | Per model revision | `HypothesisSet{weights, h0_floor}` |
| L3 Discrepancy and reference class | Hierarchical bias and variance inflation per forecast family, learned from the ledger (F-factors as bias priors), plus outside-view priors (capex overruns, ramp-up curves, schedule slip) | Per reconciliation period | Visible `Correction{family, bias, κ}` claims |
| L4 UU term | One admissible form (§9.3) | Per review or alarm | `UuTerm` on every decision-facing claim |
| L5 Surprise | Anytime-valid coverage monitors, model races, null race, CUSUM, Mondrian conformal where exchangeable, AOA, staleness | Continuous | `Monitor` automata (§10.4) |
| L6 Projection | FSS knowledge state, fs-evidence Colour, franken-evidence `EvidenceLedger` | On read | `KnowledgeCell`, the executive map |
| L7 Decision | franken-decision contracts for discrete gates; CVaR / ε-envelope robust choice for irreversible actions; human attestation gates | Per decision | `DecisionRecord` + `EvidenceLedger` |

**Timescale separation and the interference rule (DESIGN):**
- L1 updates per event, L3 per period, L5 continuously, and L7 per decision. No two adaptive layers write the same
  parameter.
- The calibration ledger scores *both* the raw L1/L2 forecast and the published L3-corrected forecast as
  separate segments, so corrections are themselves tested and cannot hide model drift.
- L3 never reads a resolution that L5 has not yet logged.
- The interference test is the correctly specified control mine (§33.3): false alarms ≤ α and sharpness loss
  < 10% with every layer on.

### 9.3 Stating probabilities on unknown unknowns without fabrication

A decision-facing Claim MUST carry exactly one `UuTerm`. Only three forms are admissible (B2 §3.9).

1. **Reference-class ε-contamination.** The belief set is {(1−ε)·P_model + ε·Q}.
   - ε and Q come from a `REFCLASS-` record: population, n, period, inclusion rule, similarity justification,
     verification tag and source document.
   - A reference class tagged *unverified* may inform internal decisions but is blocked from disclosure-facing
     claims (G-DISC).
   - Example: capex. Bertisen & Davis report a mean overrun of +14%, about half of projects outside ±15%, and about
     1 in 13 at ≥ +100%. A core-plus-blowout mixture fits all three; a single lognormal cannot (GB-32).
2. **Catch-all hypothesis h₀** (a maximum-entropy null) with floor ε_ref.
   - Naming a new hypothesis after a surprise debits h₀. Then h₀ resets to ε_ref by renormalizing the named set
     (PO-PLUMB-11), because the base rate of conceptual surprise belongs to the domain and is not used up by
     naming one surprise.
   - For novel deposit types and novel failure modes, ε_ref is the Good–Turing unseen mass n₁/N over the
     versioned catalogue (GB-09).
3. **Flip margin, probability NOT STATED** (info-gap).
   - It uses `fs_evidence::uncertainty::FlippingUnknown{required_magnitude, bound}`, which already exists in
     frankensim (FACT, `fs-evidence/src/uncertainty.rs`).
   - The type carries `NotStated`, so no code path can turn it into a number.
   - The executive reads: "unpriced unknown: the decision flips if X exceeds Y; no admissible reference class."
   - Typical uses: tailings probability of failure without a vetted reference class; jurisdictional/security
     shocks (fiscal regime changes, security events); the impairment price at which headroom vanishes.

**Coverage of the stack across the business** (which UU form is the default, and what calibrates each domain):

| Domain | Dominant KU | Default UU form | Fast-resolving forecasts (calibration fuel) | Who signs |
|---|---|---|---|---|
| Exploration | Intensity λ, detection q, terrane effects, size-distribution tail | Good–Turing novel-type slot + Manski bounds for unrecorded effort | Prospect-scale drill outcomes; discoveries via hindcast backfill | No signer |
| Resources | Grade/tonnage realizations, domaining, variogram | Catch-all conceptual model h₀ | **F1 per bench per month** (the richest stream) | CP/QP signs the category; evidence beside it |
| Reserves | Modifying factors, price, recovery | Reference class (reserve-to-production grade bias; OPEN-PLUMB-7) | F2/F3 quarterly | QP signs |
| Plans and schedules | Productivity, availability, sequencing | Reference class (schedule compliance) | Tonnes and grade to mill per period | Planner attests |
| Cost forecasts | Unit costs; capex by WBS | Reference-class mixture; a jurisdiction residual as a flip margin | Monthly actuals per WBS | Project controls attest |
| Processing | Recovery, throughput, reagent use by geometallurgical domain | Catch-all over ore-type hypotheses (e.g., unrecognized preg-robbing) | Daily metallurgical balance | Metallurgist attests |
| Tailings | Pore pressure, strength, failure modes | **Flip margin only.** PoF is not stated without a vetted reference class. | Piezometer and InSAR series | EoR signs. `geotechnical_signoff` is never set by software. |
| Accounting estimates | Recoverable amount, ARO, UoP reserves, NRV, provisional pricing | Flip margins feed the estimation-uncertainty disclosures (IAS 1.125-style; OPEN-PLUMB-9) | Settlements, final assays, actual closure costs | Controller attests |

### 9.4 Unknown knowns: the computed gap

- `fmine-calib::source` registers every data asset: reports, databases, surveys, core libraries, people.
- Every model declares its consumed inputs.
- `UnusedRelevant(claim) = {registered assets intersecting the claim's support and validity} − provenance(claim)`.
- Unknown knowns are ranked by `EVOI(ingest) / digitization cost`. Ingesting is modelled as a survey whose
  "detection" is the expected effort and deposit information recovered.
- Expect legacy exploration-effort records at the top of the ranking, because they convert Manski-bounded (A4)
  cells into informative ones (§11.3).
- LLM-extracted facts from legacy reports enter as `Assertion` with document provenance. Promotion requires
  deterministic re-derivation (e.g., collars re-georeferenced against a control survey) or a human `Reviewed`
  attestation.
- Tacit expert knowledge enters through IDEA-protocol elicitation and is stored and scored (§10.6).

### 9.5 The shared realization index (one scenario currency)

- Every stochastic engine draws from a **registered realization space**.
- A realization ID `r` names the same state of the world across modules:
  - geology realization g;
  - price path p;
  - cost and escalation path c;
  - execution draw x (productivity, availability, schedule slip);
  - country-risk draw k.
- Common random numbers make cross-module comparisons paired, not independent. The cost is that two Claims with
  `Ensemble` representations over the same space must be joined on `r`, never by independent sampling.
- This is what lets the capital program compute correlated portfolio risk (§23) and lets reconciliation compare a
  realization-wise prediction with actuals (§20).
- RNG: keyed Philox streams lifted from frankensim `fs-rand`. Key = (space, r, module, block or entity id). This
  gives bitwise-reproducible regeneration (§13.5) on every ISA for the integer streams. Transcendental functions
  go through `fmine-core::det` deterministic math, because platform `libm` differs across ISAs (FACT, R8).

### 9.6 How executives see it: the one-page epistemic map

Illustrative layout; one row per decision-relevant Claim, with colour from fs-evidence:

```text
EPISTEMIC MAP · Region R · as-of anchor 2026-Q4 · basis: PUBLIC
Claim                            P10 / P50 / P90             Evidence · Calibration (track)     UU term                      Top driver → cheapest move (net VOI)
P(≥1 econ Au deposit, tract T7)  0.06 / 0.11 / 0.19          Transferred · Uncal (hindcast: 1)  novel type ε=0.025 (G–T)     effort coverage 38% → digitize open-file (+$1.1M)
Undiscovered Au, T7              0.4 / 2.1 / 9.8 Moz         Modelled · Uncal                   Manski [1.6, 3.4] Moz P50    size-tail ξ → hindcast district 2
Reserve Au, Site A (QP)          QP: 8.1 Moz | P(±10%)=0.71  ModelCal · cov80 15/20 qtrs       catch-all h₀ 0.05            F1 bias −4% domain D3 → infill (+$0.6M)
Capex, Project P                 $2.06B / mean $2.31B / $3.2B RefClass[verified] · n/a          jurisdiction: FLIP @1.7×     scope maturity → FEL-3 review
TSF-3 static liquefaction        4/6 failure modes monitored · 2 piezometers stale >14 d (NOT OBSERVABLE) · PoF: NOT STATED · EoR review 2026-08
Impairment headroom, CGU-2       −$0.1B / +$0.4B / +$0.9B    ModelUncal · n/a                   FLIP: Au < $2,900/oz         price deck → none (exogenous)
```

**"What would change our mind" card**, one per decision:
- the decision, the current preferred action, and expected loss by action;
- each flip condition with its `required_magnitude`, its probability *if priced*, the evidence that would reveal
  it, and that evidence's cost, time and net VOI;
- monitors armed and their thresholds;
- the kill gate;
- who must attest.

Every number is clickable down to its Claim, then its provenance DAG, then the source documents. Rendering uses
`franken_evidence::render::{level1, markdown, html}` plus FrankenMining rows (§30.5). A K5-style user test
(KILL-PLUMB-5) falls back to a three-field card: range, evidence class, flip margin.

**GB-03 · Plug-in vs posterior P(≥1)** (`posterior_core`; illustrative):
- **Equation.** P(≥1) = 1 − E[e^{−Λ}]. For Λ ~ Gamma(a, b), P = 1 − (b/(b+1))^a.
- **Substituted.** Posterior mean Λ = 0.3, shape a = 0.5, b = 1.667. The posterior P(≥1) = 1 − 0.625^0.5 =
  **0.209**. Plug-in 1 − e^{−0.3} = **0.259**, which overstates by 24%. But P(≥2) is 0.061 under the posterior and
  0.037 under plug-in.
- **Intuition.** Uncertainty about the rate makes a *first* discovery less likely and a *camp* more likely.
- **Assumption.** A gamma-shaped rate posterior.
- **Flip.** A posterior shape a > 5 makes plug-in adequate (< 3% error).

## 10. The forecast and calibration ledger, and surprise

### 10.1 Register before resolve (INV-008)

- Every resolvable Claim is registered with `record_time < outcome.valid_time`. Examples:
  - next month's F1 by domain;
  - next quarter's AISC per site;
  - the capex outturn ratio;
  - this hole's intercept;
  - next week's mill throughput;
  - P(≥1 discovery) in a tract over 10 years.
- The registration stores the full distribution, the scoring rule, the resolution source, the horizon, the
  segment (model × domain × horizon × site) and the **author**, which may be a model version, a named person or
  `agent:model@version`.
- The ledger is append-only and hash-chained, and supports bitemporal as-of replay (PO-PLUMB-8).
- The FY forecasts of the public lane are hashed and timestamped *before* the issuer's release (M0, §37).
- **Technical-report assumptions are scoreable frozen forecasts** once their resolution criteria are written down
  in advance. Loulo's LOM processing cost of $19.63/t sits against a 2022 actual of $24.68/t and a plan of
  $17.49/t (FACT, E249); its LOM recovered gold for 2023–2025 is 680/680/666 koz (100%). Risk ratings resolve only
  on their stated event: Loulo's "loss of operational control" in 2025 was not "loss of ownership": Barrick kept 80%
  legal ownership throughout (FACT, E247, E248, E406). The QP's "Possible / Moderate" fiscal-stability rating did
  resolve: the 2023 Code was applied retroactively (E412).

### 10.2 Scoring

| Quantity | Proper score | Diagnostics |
|---|---|---|
| Continuous (grade, tonnes, capex ratio, AISC) | CRPS (and CRPS normalized by a declared range for racing) | PIT histogram; coverage at 50/80/95% |
| Events (P(≥1 deposit), "within ±10%", "TARP level reached") | Brier; log score floored at p_min (the floor is a recorded design choice) | Reliability diagram |
| Spatial discoveries | Point-process log score of held-out discoveries | Expected vs observed discoveries by intensity decile |
| Quantile claims | Interval score (Gneiting & Raftery 2007) | — |

- **Racing models** uses `fs_eproc::PairwiseRace` with a declared `LossSpan`. Only bounded scores qualify, and the
  race correctly refuses out-of-span losses (FACT, `fs-eproc/src/lib.rs`).
- **Expert scoring** uses Cooke's classical-model performance weights over seed questions.

### 10.3 Feedback into models (L3)

- Per forecast family: log(actual/forecast) ~ N(μ_f + u_site, σ_f²), with coverage-restoring inflation κ_f.
- Both are published as visible `Correction` claims. A published forecast shows raw, correction and corrected
  values.
- **Small-n remedies, in order:**
  1. score at the finest conditionally independent support;
  2. backfill hindcasts from the bitemporal record;
  3. simulation-based calibration on synthetic truth;
  4. reference classes.

### 10.4 Surprise monitors (L5)

```rust
pub enum Monitor {
    Coverage(fs_eproc::bernoulli::BernoulliMixtureCs),   // anytime-valid CS on 1{outcome ∈ 80% interval}
    SpatialFrontPit {                                     // sequential PIT along the mining front (below)
        whitener: VecchiaWhitener,                        // registered joint model → conditional predictive
        loc: BettingEProcess,                             // ψ = tanh(z/2); E₀ψ = 0
        disp: BettingEProcess,                            // ψ = tanh((z²−1)/2) − c₀, c₀ ≈ −0.0748
    },
    Race(fs_eproc::PairwiseRace),                         // model vs model on bounded scores
    NullRace { log_lr: f64 },                             // h₀ vs best named hypothesis
    Cusum { s: f64, h: f64 },                             // fast large shifts (pairs with Coverage)
    Aoa { di: f64, threshold: f64 },                      // dissimilarity index of new inputs vs training
    Staleness { max_age: Duration },                      // instruments, surveys, reviews
    Conformal { stratum: StratumId, alpha: f64 },         // Mondrian coverage where exchangeable
}
```

- **Why `Coverage` and `SpatialFrontPit` are paired (DESIGN).**
  - `Coverage` scores monthly reconciled domain totals, the conditionally independent support (assumption A8:
    within-month spatial correlation voids the martingale argument at finer support). It binarizes each month, so
    it is slow: when 80% coverage slips to about 58%, the median time to alarm is about 2 years (26 months), and
    about 1 run in 6–7 takes more than 5 years (exact first passage of the e ≥ 20 mixture; R7-C124, GB-08).
  - `SpatialFrontPit` scores each blast polygon or stope ring k = 1, 2, … in `availability_time` order. Its null
    is stronger: the registered joint model (the Strata ensemble, or its Vecchia approximation on m nearest
    predecessors) is the true joint law of the polygon outcomes. Under that null the sequential probability
    integral transform u_k = F̂_k(y_k | y₁…y_{k−1}) is i.i.d. Uniform(0, 1) however strongly the polygons are
    correlated (Rosenblatt 1952), and z_k = Φ⁻¹(u_k) is i.i.d. N(0, 1). A Vecchia model is itself a valid joint
    law, so the PIT is exact for it and approximate against the full Gaussian-process model. With a finite
    ensemble, F̂_k is a randomized rank.
  - Two bounded scores bet against that null: ψ_loc(z) = tanh(z/2), mean 0 by symmetry, and
    ψ_disp(z) = tanh((z² − 1)/2) − c₀ with c₀ = E₀[tanh((z² − 1)/2)] ≈ −0.0748. **The centring is required:
    uncentred, the dispersion score has mean −0.075 under the null, a negative stake earns wealth, and Ville's
    bound fails.** Each wealth process M_K = Π_{k≤K} (1 + λ_k ψ(z_k)), with a predictable stake λ_k that keeps
    1 + λ_k ψ > 0 (aGRAPA; Waudby-Smith and Ramdas 2024), is a nonnegative martingale under the null, so
    P₀(sup_K M_K ≥ 2/α) ≤ α/2 per score and ≤ α for the pair.
  - Simulated detection at α = 0.05 with aGRAPA stakes on both scores (INFERENCE from simulation; HYPOTHESIS for
    any real pit): median 22 polygons when 80% coverage falls to 58% through a location shift, 36 through a
    dispersion shift (σ × 1.59), 50 for a 0.5σ bias and about 220 for a 1.4× variance excess. The false-alarm rate
    over 600 null polygons was 3.8%. At 10 polygons a month the first three alarm in 2–5 months, against a median
    of about 2 years for `Coverage`.
  - An alarm says the joint model is wrong somewhere (mean, variance or covariance). The InvestigationCase (§10.5)
    attributes it, and `Coverage` stays the test of the published 80% intervals.

**GB-08 · Anytime-valid coverage alarm: monthly Bernoulli vs. the spatial-front PIT** (`e_fdr`; illustrative):
- **Equation.**
  - Monthly Bernoulli mixture: e = B(k+1, n−k+1) / (0.8^k · 0.2^{n−k}). Alarm at e ≥ 1/α = 20 (the
    franken-decision default).
  - Spatial-front PIT: M_K = Π_{k≤K} (1 + λ_k ψ(z_k)) per score, with z_k = Φ⁻¹(F̂_k(y_k | y₁…y_{k−1})). The pair
    alarms when either wealth reaches 2/α = 40.
- **Substituted.**
  - Monthly: the 80% intervals held in 7 of 12 months gives e = **1.45**. 14 of 24 gives **4.5**. 36 of 60 gives
    **84 → alarm**.
  - Polygon level, 10 polygons a month, coverage 80% → 58% by a dispersion shift (σ × 1.59): the oracle stake
    grows log-wealth at 0.146 nats per polygon, so ln 40 ≈ 3.69 takes about 25 polygons. The predictable aGRAPA
    stake needs a median of **36 polygons (10–90%: 16–71), about 3.6 months**.
- **Intuition.** "7 of 12 looks bad" is not yet evidence, because a binary monthly score discards both the size of
  each miss and the polygon sequence. Conditioning each polygon on the ones already mined removes the spatial
  correlation without coarsening. CUSUM still catches large abrupt shifts.
- **Flip.** If the grade-control measurement error is uncharacterized (G-CORE-2), the dispersion score cannot
  separate geological under-dispersion from assay noise. Run the location score only, and keep `Coverage`.

### 10.5 The UU → KU conversion automaton and InvestigationCase

```text
Armed ──alarm(e ≥ 1/α | CUSUM | AOA exit | h₀ wins race | staleness)──► InvestigationOpen{case}
InvestigationOpen ──► { HypothesisAdded(debit h₀, reset floor)
                      | Recalibrated(visible Correction claim)
                      | DataDefect(QA/QC finding; supersede inputs)
                      | FalseAlarmLogged }
                  ──► Rearmed
```

- **There is never a silent retrain.**
- An `InvestigationCase` (after FSS §20.9) carries:
  - the triggering monitor and evidence;
  - competing explanations, each a `HYP-`;
  - VOI-ranked probes, ordered by "expected decision-loss reduction per resource, not information volume";
  - an owner, deadlines and the resolution.
- Cases are first-class ledger objects. Agents may open them and propose probes. Humans close them.

### 10.6 Expert elicitation

- Structured elicitation uses the IDEA protocol (Investigate, Discuss, Estimate, Aggregate).
- Elicited quantities are registered forecasts when resolvable.
- Experts are scored on seed questions with known answers, and pooled with performance weights.
- Scoring experts is politically sensitive (OPEN-PLUMB-11). Scores are visible to the expert and the case owner
  by default, not published.

### 10.7 Proof obligations for Plumb

| ID | Statement | Verification | If violated |
|---|---|---|---|
| PO-PLUMB-1 | NL-1…NL-8 hold over 10⁵ random provenance DAGs | Proptest with fs-evidence `compose` | Block release |
| PO-PLUMB-2 | Taint monotone; no Full-basis bytes in Public exports | Taint test + export fuzz | Block export |
| PO-PLUMB-8 | Ledger append-only and hash-chained; every scored forecast had record time < outcome valid time; as-of replay bit-identical | Chain audit | Quarantine the segment |
| PO-PLUMB-10 | Monitor false-alarm rate ≤ α under a calibrated null at the declared support (Ville) | Control-mine replay | Re-specify the support |
| PO-PLUMB-11 | P(h₀) ≥ ε_ref after every revision, including after hypothesis additions | Invariant test | Block |
| PO-PLUMB-15 | `Refused` never becomes a number without a declared fallback | Type-level + tests | — |
| PO-PLUMB-16 | Mondrian conformal coverage ≥ 1−α within each exchangeable stratum | Held-out strata | Drop the conformal claim |

### 10.8 Plumb gates and kill gates

| Gate | Threshold (TARGET) |
|---|---|
| GATE-PLUMB-E1 | No-laundering proptests 100% |
| GATE-PLUMB-E2 | 100% of resolvable decision claims registered before resolution; as-of replay bit-identical |
| GATE-PLUMB-E3 | Misspecified synthetic mine (out-of-model fault; biased capex process): the layered stack restores 80% coverage to ±5 points within 12 resolutions after an alarm, pooled over R ≥ 50 replicate worlds (P(pass ∣ true 0.8) ≈ 0.998, P(pass ∣ 0.7) ≈ 0.004; a single 12-resolution run passes a calibrated stack only 52% of the time, R7-C120), while a single global posterior documentably fails |
| GATE-PLUMB-E4 | Zero ε without a `RefClass`; zero unverified reference classes on disclosure-facing claims |
| GATE-PLUMB-E5 | Correctly specified control mine: one-sided 95% Clopper–Pearson upper bound on the false-alarm rate ≤ α over N ≥ 1,000 null runs (at the simulated 3.8%, N = 600 gives 0.054 and fails; R7-C122); sharpness loss < 10% |

| Kill | Trigger | Fallback |
|---|---|---|
| KILL-PLUMB-5 | Executive user test rejects the epistemic map | Three-field card: range · evidence class · flip margin |
| KILL-PLUMB-6 | Monitors raise more than 1 false alarm per site per quarter in the control. Each CUSUM stream is tuned to ARL₀ ≥ 85·M polygons (M = streams per site), so the expected rate is ≤ 0.36 per quarter and the kill fires by chance in ≤ 5% of quarters; at an expected rate of exactly 1 it would fire in 26% (R7-C122) | Re-specify the support; CUSUM-only for fast shifts |

---

# PART III — THE MINE, END TO END

## 11. Lodestar: exploration and Bayesian prospectivity

### 11.1 Decisions served, and the base rates that bound every output

- **Decisions served.**
  - Which ground to hold, acquire, earn into or drop.
  - Which survey or drill program to fund next, and when to stop on a target.
  - How much undiscovered endowment backs a district's option value. This is the interface to the
    CapitalProgram (§23).
- **Loss asymmetry.** Both errors are false certainty, and both cost more than missed upside, the cheapest error:
  - a falsely "cleared" area, from overconfident negative evidence;
  - a falsely "hot" area, from overconfident analogue transfer.

  So absolute probabilities MUST be refused or bounded wherever exploration effort or analogue transfer is
  unidentified.
- **Base rates (FACT, R6 §4.4–4.5, E026).**
  - About $218M of exploration spend per discovery (2011–20).
  - About 0.69 of value created per exploration dollar.
  - 2–3 Tier-1 discoveries per year globally.
  - 15.7 years from discovery to production for 127 mines started 2002–23, and 17.9 years for mines started 2020–23 (FACT[U], S&P Global Market Intelligence, two different samples; E636).
- **Sanity rule (DESIGN).** A model that implies a materially better hit rate than these base rates over a broad
  area is presumptively miscalibrated until its hindcast says otherwise.

### 11.2 Verdicts on method (Brenner pass B2)

| Candidate | Verdict | Role |
|---|---|---|
| (a) Pixel-wise ML prospectivity (RF/GBT/CNN on evidence rasters) | **Refuted as the object of record.** It has no probability semantics. It treats unlabelled cells as negatives (SCAR), collapsing all absence classes into one zero. Its scores proxy past effort. It degrades under leave-one-terrane-out (E028). | **Survives as a cross-fitted covariate supplier** and a proposal generator |
| (b) Hierarchical marked, thinned Cox process (LGCP/SPDE intensity; grade–tonnage marks; discovery thinning by effort × detectability; terrane partial pooling) | **Survives as the regional object of record**, conditional on the effort-data gate GATE-LODE-P4 | Core of `fmine-prospect` |
| (c) Mineral-systems causal model (source–pathway–trap–preservation) | **Refuted standalone**: it has no calibration and no discovery-bias model | **Survives as the factorization of log-intensity** (a noisy-AND prior) and as the transport argument for dissimilar terranes |
| (d1) Search-theoretic POMDP over competing geological hypotheses (Intelligent Prospector; Popper–Bayes) | **Survives at prospect scale only** (≲10² km², ≲10² holes); intractable regionally | Phase 2 (`fmine-prospect::pomdp`), fed by (b)'s local posterior as its prior |
| (d2) Hybrid: (a)+(c) feed (b), which feeds (d1), with typed search modes | **Adopted architecture** | — |
| N0 MapMark4-degenerate tract model (negative-binomial counts × grade–tonnage pdfs) | Checked null | Parity oracle and deterministic fallback |

### 11.3 Data model (`fmine-explore`)

```rust
pub struct DepositRecord { id: DepositId, geom: Geom, deposit_type: TypeProbs /* vector over catalogue + k* */,
    marks: MarkRecord /* tonnage, compositional grade, depth, style; censored where sub-economic */,
    discovery: Option<DiscoveryEvent{ year, method: ExploreMethod, company, doc: DocRef }>,
    aggregation_rule: RuleId /* USGS consistency rule: one rule for labels, marks and outputs */, provenance: ProvenanceDag }
pub struct EffortRecord { id: EffortId, method: ExploreMethod, geom: Geom, valid: TimeInterval,
    spec: MethodSpec /* line spacing, depth of investigation, detection limits, hole depth, sample medium */,
    source: DocRef, coverage: CoverageConfidence /* High | Medium | Low | Unknown */ }
pub struct CovariateField { id: FieldId, ingredient: Ingredient /* Source|Energy|Pathway|Trap|Preservation|Cover */,
    realizations: RealizationSetId /* stochastic interpolation/inversion; never a single deterministic raster */,
    support: Support, evidence: EvidenceClass }
pub struct ElicitationRecord { tract: TractId, experts: Vec<PersonId>, protocol: Idea, n90_n50_n10: [u32; 3], seeds_scored: CalibRecordId }
pub struct TractMap { version: SemVer, tracts: Vec<Tract>, alternatives: HypothesisSetId /* map uncertainty is KU-structural */ }
```

**Absence classes (DESIGN; extends R7 §1.7 with a fifth class).**

| Class | Typed as | Enters the likelihood as | Knowledge state | Example |
|---|---|---|---|---|
| **A0 Not looked** | No `EffortRecord` intersects s | Nothing | `not_observable` | Unmapped tenement under 60 m of transported cover |
| **A1 Looked, insensitive** | Effort with q_j(m, cover) ≤ q_min for that style and depth | ≈ nothing (the hazard is computed and logged) | `not_observable` for that style and depth | Soil geochemistry over transported cover; early multispectral imagery |
| **A2 Looked, sensitive, nothing** | Hazard H > 0 | −∫λπ penalty, scaled by q | `estimated` (absence of detection, with π) | Close-spaced aircore through thin cover |
| **A3 Found sub-economic** | Occurrence with a censored mark | A positive point plus a mark upper bound | `known` occurrence | A 0.3 g/t intercept over 20 m |
| **A4 Effort suspected, unrecorded** | `coverage ∈ {Low, Unknown}` or a report mention without geometry | **Manski bounds**: π ∈ [0, π_era-typical] | `unknown` (an unknown known) | A 1987 report cites "extensive RAB drilling" with no collars |

**Data sources.**
- Each source enters with its evidence class and its detection model q_j, drawn from the method-specification
  catalogue:
  - airborne magnetics and radiometrics; gravity; AEM (time-domain and frequency-domain); IP/resistivity; MT;
  - multispectral and hyperspectral remote sensing; stream-sediment, soil, lag and biogeochemistry;
  - geological and structural mapping; historical drilling; deposit catalogues; open-file company reports.
- **Geophysical inversions** (gravity, magnetics, IP/EM) enter as *realizations* of covariate fields from
  stochastic inversion, never as one deterministic model. Deterministic interpolation halves the apparent
  variance (FACT, Caers 2025, R7).
- Inversion engines are a build item: `fmine-prospect::inversion`. It starts with linear Bayesian inversion for
  potential fields (gravity, magnetics) on lifted `fs-sparse` and defers EM/IP to Phase 2 (OPEN-LODE-3).

### 11.4 The generative model (DESIGN)

```text
Types         k ∈ K_named ∪ {k*}       k* = "novel/unclassified type" slot (UU; Good–Turing, GB-09)
Terranes      τ(s) from a versioned tract map (each version is a Claim; map uncertainty = KU-structural alternatives)
Intensity     log λ_k(s) = α_{k,τ} + Σ_i log p_{k,i}(s) + w_k(s)
                  p_{k,i}(s) = logistic(γ_{k,i,τ}ᵀ z_i(s))     ingredient i's proxies z_i (mineral-systems noisy-AND)
                  z_i may include cross-fitted ML scores, stored as realizations, never as labels
                  w_k ~ SPDE-Matérn(range ρ_k, sd σ_k) on mesh M (2D continental; nested 2D/3D district)
Hierarchy     θ_{k,τ} = (α, γ) ~ N(θ_k + GP_terrane(d(τ,τ′)), Σ_k)     σ_τ hyperprior ≈ 1–1.5 (GB-07)
Marks         log T ~ spliced{ lognormal body | GPD tail above u_k };  ilr(G) | T ~ N(·);  depth z;  style
              observed tonnage = true × growth(era, age);  truncated below the reporting threshold
Thinning      π(s,m,t) = 1 − exp(−H(s,m,t)),   H = Σ_j ∫_0^t e_j(s,τ) q_j(m, cover(s), τ) dτ
              q_j hierarchical: q_j = q̄_j(era) · g_j(footprint(T), cover/DOI_j);  discovery-method labels → competing hazards
Observation   (1) deposit catalogue (presence-only, thinned, marked)
              (2) drill intercepts with assays (presence/absence at known support, prospect scale)
              (3) geochemistry/geophysics as covariate realizations z_i^{(r)}, r = 1..R
              (4) elicited N90/N50/N10 per tract as a pseudo-likelihood on ∫λ (enables MapMark parity)
Likelihood    ℓ = Σ_obs log[λ π f] − ∫∫ λ π f dm ds  (+ intercept and elicitation terms)   ← the integral is where absence lives
Inference     Laplace / INLA-lite: Newton on the latent field per hyperparameter point (log-concave, PO-LODE-4),
              lifted fs-sparse supernodal LDLᵀ with symbolic reuse; hyperparameters by CCD/grid; covariate
              uncertainty by mixing over r; HMC/NUTS fallback at district scale (fmine-infer); SBC mandatory
Posterior     IntensityPosterior{ basis: Public | Full }: draws of (θ, w at mesh nodes, q, mark parameters)
```

**Why each modelling choice (rationale for future agents):**
- **Cover enters detection (q) and economics, never existence (λ).** Preservation and erosion affect existence.
  Confusing the two is the classic way pixel models "learn" that covered ground is barren.
- **Spliced lognormal–GPD marks.** The top 10% of deposits hold 71–86% of the metal (FACT, Singer 1995, R7). That
  implies a lognormal σ of ≈1.8–2.4, and the tail is probably still understated by a lognormal. Valuing with
  medians flips decisions (GB-05).
- **Growth model on reported tonnage.** Resources grow after discovery (e.g., the Golden Mile from 36 to 72 Moz,
  FACT, R7). Catalogue tonnage at year t is not the endowment.
- **Effort ignorability is an assumption, and it is tested.** The thinned likelihood is valid only if effort
  depends on recorded information. If prospectors followed unrecorded signs (gossans, old workings), then
  unexplored ground was *judged* poorer. Test A2 (§11.8): does lagged effort predict post-T discoveries beyond the
  covariates? If yes, model effort jointly as preferential sampling, or widen to Manski bounds.

### 11.5 Decision outputs and their compiled artifacts

Every output is a Claim with:
- a representation;
- an evidence class;
- a calibration flag;
- a UU term;
- validity (price deck and economic-filter version);
- a basis (Public or Full);
- a confidentiality level.

**(A) P(≥1 economic deposit in area A | E, t).**

```text
Λ_econ,undisc(A) = ∫_A ∫_{M_econ(deck)} λ(s) f(m|s) (1 − π(s,m,t)) dm ds
P(N ≥ 1)         = 1 − E_post[ exp(−Λ_econ,undisc(A)) ]        ← expectation OUTSIDE the exponential (GB-03)
```

- `M_econ(deck)` is a versioned economic-mark filter: minimum contained metal and grade at depth z under a
  declared price/cost/recovery deck.
- Output fields:
  - P10/P50/P90 of the probability itself;
  - an AOA flag;
  - a Manski bound pair when the unknown-effort area exceeds 10% of A;
  - the **limiting ingredient** argmin_i p_i (the explanation: "the trap proxy is the binding constraint here");
  - the top uncertainty driver (range ρ, detection q, terrane effect θ_τ, mark tail).
- **Refusal.** If dissimilarity(τ) > d_max or GATE-LODE-P4 fails, the output is `Interval{Manski}` or
  `Refused(NoClaimInDomain)`. Never a point probability.

**(B) Distribution of undiscovered contained metal.**
- A compound Cox sum Σ T_n·G_n over undiscovered deposits, by Monte Carlo over posterior draws.
- Report P10/P50/P90, the mean, and P(at least one Tier-One-size deposit). State which Tier-One definition was
  used: an issuer's own definition and industry definitions differ, so they MUST NOT be mixed (R6 §1.3).
- The output is an `Ensemble` Claim on the shared realization index, so the CapitalProgram joins it with price
  paths using common random numbers.

**(C) EV and VOI of the next survey or drill program (preposterior).**

```text
EV(a | E)  = E_post[ U(a, θ) ]    U includes lead-time discounting (15.7–17.9 yr), P(develop | discovered), deal terms
VOI(o | E) = E_{y ~ p(y|E,o)} [ max_a E[U(a) | E, y] ] − max_a E[U(a) | E]        (≥ 0)
net(o)     = VOI(o) − cost(o) − delay_cost(o)
y-model    = thinned-process predictions of survey/drill outcomes: detect / false-positive anomaly / sub-economic / dry
```

- Estimated by nested Monte Carlo with common random numbers and control variates, reporting the Monte Carlo
  standard error.
- `VoiStatus::Indeterminate` applies when the sign of net(o) is unresolved at 2·SE. In that case the ranking falls
  back to uncertainty-proportional ranking, labelled as heuristic.
- Information-theoretic EOI (expected entropy reduction) is shown as a secondary, decision-agnostic column. It is
  never the ranking key.
- `fs_voi` (frankensim) is used as a pattern and for Gaussian sub-problems only. Its closed form assumes Gaussian
  outcomes, which is unfit for heavy-tailed, discrete discovery outcomes (FACT, `fs-voi/src/lib.rs` header).

**(D) Bayesian search after negative drilling: two typed modes.**

| Mode | When valid | Update | Effect elsewhere |
|---|---|---|---|
| `SearchMode::Cox` | Unknown number of deposits (regional, district) | λ_post(s,m) = λ(s,m)·(1 − d_prog(s,m)) for the undiscovered part; θ_τ updated through the likelihood integral | **No renormalization upward.** Elsewhere changes only through a slight shrink in the shared terrane effect, and through hypothesis reweighting where a hypothesis predicted this spot. |
| `SearchMode::SingleTarget{anomaly}` | One causative body is asserted to exist: a boulder-train source, a geochemical dispersion fan, an EM conductor. The assertion is itself a Claim with P(exists). | Koopman: p′_i = p_i(1 − d_i) / (1 − Σ p_j d_j) | Other cells renormalize upward by 1/(1 − pD) |

- **Hole detection model.**
  - d_hole(s,m) = P(the hole trace intersects the *footprint* of a deposit at s with mark m) × P(recognized | intersected, logging and assay protocol).
  - The footprint is the alteration and geochemical halo, not the ore.
  - Detection is never 1.
- **The AF447 failure.** Overstating d clears ground too hard, which is the AF447 search failure (R7 §1.7). The
  posterior over d is displayed with the update (GB-02).
- **Artifact.** `SearchUpdate{mode, before, after, d_posterior, hypothesis_reweights, mass_moved_elsewhere}`, an
  append-only claim pair with a galaxy-brain card.

**(E) Drill-target and pad/wedge fan ranking under budget.**
- **Objective.** F(S) = E_post[Σ_deposits V(m)·(1 − Π_{h∈S}(1 − d_h(deposit)))]. For each fixed deposit
  realization the coverage term is monotone submodular in the hole set S, and a non-negative mixture over the
  posterior preserves this.
- **Cost.** Holes share set-up costs: cost(S) = Σ_{p∈P(S)} C_pad,p + Σ_{w∈W(S)} C_parent,w + Σ_{h∈S} c_h ≤ B,
  where P(S) is the set of drill pads or underground drill bays the holes use and W(S) the parent holes from which
  wedge daughters are kicked off (Fourmile names daughters like `FM25-260DW1`; FACT, E656). An underground
  platform can replace deep surface fans: Fourmile's timeline starts Bullion Hill decline development in 2026 and
  underground drilling from the Goldrush MPD in 2027 (FACT, E657), and each enters as a pad option.
- **Algorithm.**
  - With zero pad and parent costs, cost is modular. Lazy greedy (CELF) gives ≥ (1 − 1/e) under a cardinality
    constraint; cost-benefit greedy plus the best singleton gives ≥ (1 − 1/e)/2 under a knapsack budget.
  - With set-up costs, cost(S) is submodular rather than modular, and no constant-factor guarantee is claimed.
    `fmine-prospect` runs bundle greedy: each candidate pad or parent hole with its best fan (inner CELF) is one
    bundle, ranked by marginal coverage per incremental cost, and the best single bundle is kept as a fallback.
  - The certificate carries a data-dependent upper bound instead of a ratio. By monotone submodularity,
    OPT ≤ F(Ŝ) + max_{T: cost(T) ≤ B} Σ_{h∈T} δ_h(Ŝ), where δ_h(Ŝ) is the marginal gain of hole h given the chosen
    set Ŝ; the LP relaxation of that fixed-charge knapsack bounds the max from above (Leskovec et al. 2007). The
    gap between F(Ŝ) and the bound is reported (PO-LODE-15).
  - All guarantees apply to the monotone coverage term under the budget. F(S) − cost(S) is not monotone, so cost
    enters only as the constraint, never as a subtracted term inside the guaranteed objective.
  - The result is a `PortfolioCertificate`: the pad/parent/wedge activation tree, the marginal-gain trace, the
    upper bound and a submodularity spot-check.
- **Not claimed.** The VOI objective is generally *not* submodular. Adaptive sequential choice at prospect scale
  (for example, kicking off a wedge only if the parent hole's intercept justifies it) goes to the phase-2 POMDP, or
  to myopic VOI with one-step lookahead over parent → daughter branches.
- **Deals.** The ranking accepts options whose cost and value are shaped by deal terms (earn-in, stake, option).
  The deal economics live in §23 (`ProgramOption{deal}`).

### 11.6 Discriminating experiment and hindcast protocol (cheap, one district, one quarter of effort)

- **District.** A mature public district with dated deposits and open-file effort: Yilgarn (Guj et al. 2011
  precedent), Abitibi or Nevada. Data licensing is OPEN-LODE-1.
- **Freeze.** Evidence and labels as known at T, via a bitemporal as-of query. Score discoveries in (T, T+20 yr].
- **Models compared:**
  - (a) RF with pseudo-absences;
  - (a′) bagging-PU;
  - (b) full thinned process;
  - (b0) (b) without thinning;
  - the mineral-systems-structured (b);
  - N0.
- **Scores:**
  - point-process log score of held-out discoveries;
  - PR-AUC on cells;
  - capture efficiency at the top 1%, 5% and 10% of area;
  - expected vs observed discoveries per intensity decile;
  - 80% coverage of tract counts;
  - **the count of positives**. Fewer than about 15 in-tract post-T discoveries means the test cannot
    discriminate, and the report says so.
- **Splits.** Blocked cross-validation with block size from the variogram range; leave-one-terrane-out; temporal.

| Prediction if (b)/(d2) is right | If instead | Then |
|---|---|---|
| The (b) − (b0) log-score gain concentrates in low-effort and deep-cover strata | The gain is uniform or zero | Thinning does no work. Check effort coverage before concluding anything about the model. |
| (a) ≥ (b) in high-effort shallow cells; (b) > (a) in low-effort and leave-one-terrane-out cells | (a) wins everywhere, including leave-one-terrane-out | KILL-LODE-1: (b) is demoted to the counting layer |
| The mineral-systems structure gains more under leave-one-terrane-out than in-terrane | No difference | The factorization is ornamental; keep flat covariates |
| (b) decile calibration within the 80% band; N0 beaten on tract counts | (b) not better than N0 | Fall back to N0 plus effort-adjusted tracts |

### 11.7 Confidentiality: the dual-basis posterior (MNPI)

- When MNPI inputs exist (unreleased drill results are material non-public information, E023), every posterior is
  computed twice:
  - `basis = Public`, from public evidence only;
  - `basis = Full`.
- Only the Public posterior may leave an information wall. The *difference* between the two bases is itself MNPI,
  because it encodes the drill results.
- A Claim whose provenance contains an MNPI leaf stays MNPI until that leaf's public-release valid time.
- **Transfer-price inputs are walled too.** Barrick "anticipates Fourmile will be incorporated into the Nevada
  Gold Mines joint venture, at fair market value, if certain criteria are met" (FACT, E139). Any Fourmile
  valuation is therefore a potential JV transfer-price input: walled lane, insider-listed, never in NGM-partner-
  visible outputs (§31.2). Fourmile's PEA uses a $1,900/oz stope optimiser against the $1,500 reserve deck (E141),
  so its inventories are re-priced before any haircut (§23.5). Its one-year resource growth (Indicated 1.4 → 2.6
  Moz, Inferred 6.4 → 13 Moz at 35 m and 80–90 m target spacing; E140, E142) is a conversion-prior datum. The
  deck labels two processing end-members (FACT, E656): the Wenban 5/8 contact as "Double Refractory Roaster feed"
  (FM24-200D, 1.3 m @ 16.25 g/t) and the silicified, structurally controlled zone as "Single Refractory Roaster or
  Autoclave feed" (FM24-209D, 1.6 m @ 30.6 g/t inside 16.3 m @ 47.07 g/t). Valuation and drill VOI (§11.5(E))
  keep them apart, because single-refractory ounces carry a higher shadow value whenever roaster capacity binds
  (§15.6). Since 9 Aug 2026 Fourmile is to be vended into NGM (FACT, E658), so its value is now an NGM transfer
  price as well as a Barrick asset.
- A capability gate forbids junior-equity decisions (§23) from consuming `basis = Full` claims about that junior's
  ground (OPEN-LODE-6 with counsel).
- Model weights trained on MNPI are MNPI (membership leakage).

### 11.8 Assumptions ledger (each with its test)

| ID | Assumption | Test / monitor | If false |
|---|---|---|---|
| A1 | Independent thinning given λ (no deposit-to-deposit detection interaction) | District shutdown effects in hindcast | Add a district-maturity covariate to q |
| A2 | **Effort ignorability**: effort depends on recorded information only | Does lagged effort in λ predict post-T discoveries? | Preferential-sampling joint model, or Manski widening |
| A3 | Poisson/Cox with clustering in w; inhibition via the aggregation rule | Pair-correlation residuals | Add an inhibition term |
| A4 | Marks independent of location given covariates | Mark–covariate residual test | Mark regression |
| A5 | One aggregation rule applied identically to labels, marks and outputs | Versioned rule hash on all three | Block the fit |
| A6 | Covariate realizations represent interpolation uncertainty | Variance-ratio check against deterministic interpolation | Flag `Estimated`; widen |
| A7 | Reported tonnage = true × growth(era) | Inventory time series | Wider growth prior |

### 11.9 Galaxy-brain cards (illustrative values)

**GB-01 · The discovery-thinning integral.**
- **Equation.**
  - Λ_undisc = Σ_cells Λ·e^{−H}, where H = Σ_j e_j q_j.
  - Log-likelihood penalty = −Σ Λ(1 − e^{−H}).
- **Substituted.** One 400 km² cell with a prior of 0.5 deposits per 1,000 km², so Λ = 0.1 per half.
  - Shallow half: soil q = 0.9 plus magnetics 0.1 gives H = 1.0, so π = 0.632 and Λ_undisc = 0.037.
  - Deep half (40–80 m of transported cover): soil is *insensitive* (A1, q = 0). Magnetics 0.1 plus aircore 0.15
    gives H = 0.25, so π = 0.221 and Λ_undisc = 0.078.
- **Result.**

  | Treatment | P(≥1 undiscovered) |
  |---|---|
  | Thinning model | 1 − e^{−0.115} = **0.108** |
  | Ignore effort | 0.181 |
  | "Explored = barren" (pseudo-absence) | ≈ 0 |

  68% of what remains lies under cover.
- **Intuition.** You only learn from looking where looking could have worked.
- **Assumptions.** A1, A2, and q at that cover depth.
- **Flip.** If deep-cover aircore q is really 0.40, the ranking barely moves. If effort records are incomplete,
  report the Manski bounds [0.108, 0.181].

**GB-02 · Bayesian search after three dry holes.**
- **Equation.**
  - Cox mode: Λ′ = Λ·Π(1 − d_h).
  - Single-target mode: p′ = p(1 − D)/(1 − pD).
- **Substituted.** Prior P(≥1) = 0.20, so Λ = 0.223. d_h = 0.35 per hole, so the miss probability is 0.65³ = 0.275.
  - **Cox mode:** Λ′ = 0.0613, so **P′ = 0.059**. Elsewhere is unchanged.
  - **Single-target mode** (D = 0.725): p′ = **0.064**, and every other cell scales by 1/(1 − 0.145) = **×1.17**.
- **AF447 check.** If the true d_h is 0.15 (holes too shallow beneath cover), the true P′ is 0.128. The target was
  cleared 2.2× too hard. A hierarchical d that is 50/50 on {0.35, 0.15} gives **P′ = 0.094**.
- **Intuition.** Dry holes clear ground only as well as the holes could see. They push probability elsewhere only
  if you assert that the body exists.
- **Flip.** The d posterior (d < 0.2 → keep the target), or an A3 sub-economic intercept (positive evidence).

**GB-04 · EVOI of an IP survey before a six-hole program.**
- **Equation.** VOI = Σ_y P(y)·max(EV_drill | y, 0) − max(EV_drill, 0).
- **Substituted.**
  - Inputs: P(present) = 0.10; program detection 0.70; V_net = $125M (lead-time-discounted, net of definition
    cost); drilling $3M; survey $0.4M with sensitivity 0.85 and false-positive rate 0.30.
  - Drill now: EV = 0.07·125 − 3 = **$5.75M**.
  - P(anomaly) = 0.355. P(present | anomaly) = 0.239, so EV = $17.95M. P(present | no anomaly) = 0.023, so walk away.
  - Survey policy gives $6.37M, so **VOI is $0.62M gross, +$0.22M net. Survey first.**
- **Flip.** VOI_gross = 1.4325 − 2.7·FP. Above a false-positive rate of **≈ 0.38**, skip the survey and drill.
  Graphite and clays are chargeable, so a black-shale host flips the decision.
- **Assumption.** Survey and drill errors are independent given the deposit.

**GB-05 · Heavy-tailed marks: never value with the median.**
- **Equation.** V ~ LN(μ, σ) with σ = √(2 ln(mean/median)). Top-decile value share = 1 − Φ(z₀.₉ − σ).
- **Substituted.** Median $40M, mean $150M, so σ = 1.63.
  - Valued at the median, GB-04's drill EV is 0.07·(40 − 25) − 3 = **−$1.95M (drop)**.
  - Valued at the mean, it is **+$5.75M (drill)**.
  - 79% of discoveries are worth less than the mean, and the top decile holds 64% of the value.
- **Flip.** Only portfolio-level risk limits (CVaR), not medians, may overrule expected value.

**GB-07 · Analogue transfer and partial pooling** (calibrated on Ellefsen 2026: pooled and local assessments differ
in 14 of 16 cases, FACT, R7).
- **Equation.**
  - Expected range of 16 normal draws ≈ 3.53σ, so σ_τ ≈ (ln 5.53 + ln 26.6)/3.53.
  - Pooling weight on local data = σ_τ²/(σ_τ² + 1/n).
- **Substituted.** σ_τ ≈ 1.41, so an unexplored terrane's intensity is uncertain by a factor of about **16 either
  way** at 95%. The weight on local data is 0.67 at n = 1 deposit and 0.91 at n = 5.
- **Intuition.** The honest analogue prior is wide, and a few well-explored local deposits dominate it fast.
- **Assumption (HYPOTHESIS).** The ratio statistics proxy tract log-intensity differences.
- **Flip.** A direct hierarchical fit on 30 or more tracts replaces this card.

**GB-09 · Novel deposit-type mass via Good–Turing.**
- **Equation.** P(next discovery is of an unseen type) ≈ n₁/N.
- **Substituted.** N = 120 regional discoveries in the versioned type catalogue, with n₁ = 3 types seen exactly
  once, so **ε_type = 0.025** of the discovery rate goes to the k* slot. Precedent: IOCG was a new deposit type when
  Olympic Dam was found in 1975.
- **Intuition.** The catalogue's own singletons price its incompleteness.
- **Flip.** A catalogue revision that reclassifies the singletons.

### 11.10 Proof obligations, gates and kill gates

| ID | Statement | Verification | If violated |
|---|---|---|---|
| PO-LODE-1 | For fixed hyperparameters, adding effort with no discoveries never increases λ_undisc(s) | Metamorphic property tests | Block release |
| PO-LODE-2 | E[N_obs(A)] + E[N_undisc(A)] = E[N(A)] within 1e-6 relative | Exact identity test | Block |
| PO-LODE-3 | Polygon-integrated counts invariant under mesh refinement within 2% | Refinement ladder | Coarse-mesh claim plus an explicit discretization term |
| PO-LODE-4 | For fixed (θ, q) the latent posterior is strictly log-concave, so the Newton mode is unique. Certificate: ‖∇‖∞ < tol and all LDLᵀ pivots > 0. | `LaplaceCertificate` | HMC (district) or N0 |
| PO-LODE-5 | The degenerate configuration matches MapMark4 endowment quantiles within Monte Carlo error (KS < 0.02 at 10⁵ draws) | Lab oracle (test time only) | Block the parity claim |
| PO-LODE-6 | A single-target update equals p(1−d)/(1−pd), and total mass is conserved | Unit test | Block |
| PO-LODE-7 | Simulation-based calibration rank histograms are uniform (χ² p > 0.01). Undiscovered-count and metal coverage within ±3 points of nominal over ≥200 replicates. | Synthetic district (§33.3) | Demote to `Uncalibrated` |
| PO-LODE-13 | The greedy portfolio achieves its stated ratio; submodularity is spot-checked on samples | Marginal-gain trace | Report "heuristic, no bound" |
| PO-LODE-14 | VOI sign is reported only when resolved at 2·SE | Monte Carlo CI | `Indeterminate` plus the heuristic baseline |
| PO-LODE-15 | Under pad, parent-hole and wedge costs the certificate's upper bound is ≥ the brute-force optimum and F(Ŝ) ≤ that optimum; with zero pad and parent costs the result meets the knapsack (1 − 1/e)/2 guarantee | Brute force on synthetic pad trees (≤ 20 holes) | Block the `PortfolioCertificate` |

| Gate | Threshold (TARGET) |
|---|---|
| GATE-LODE-P1 | Synthetic coverage within ±3 points (≥200 replicates); SBC p > 0.01 |
| GATE-LODE-P2 | MapMark4 parity (KS < 0.02) |
| GATE-LODE-P3 | In ≥1 public district, (b) beats N0 and (a) on held-out point-process log score (paired race e ≥ 20, or a 95% bootstrap CI excluding 0); the gain concentrates in low-effort strata; decile calibration within the 80% band |
| GATE-LODE-P4 | ≥60% of area-weighted effort digitized at coverage ≥ Medium before any *absolute* probability leaves the lab |
| GATE-LODE-P5 | Metamorphic suite 100% |
| GATE-LODE-P6 | A 10⁶-node 2D Laplace fit takes < 1 h on one workstation; a polygon query < 1 s |

| Kill | Trigger | Deterministic fallback |
|---|---|---|
| KILL-LODE-1 | (b) fails GATE-LODE-P3 in two districts | N0 plus (a) ranking labelled `Uncalibrated`; thinning only as a tract-level effort adjustment |
| KILL-LODE-2 | GATE-LODE-P4 unreachable for any target district within 6 months | Ship Manski bounds plus an unknown-knowns digitization VOI list as the exploration product |
| KILL-LODE-3 | PO-LODE-4 or GATE-LODE-P6 fails | Vecchia approximation or a coarse mesh; tract mode |
| KILL-LODE-4 | The prospect-scale POMDP saves < 20% of holes against myopic VOI at equal accuracy | Myopic VOI |

**Open questions.**
- **OPEN-LODE-1 (the decisive risk).** Effort-data availability and licensing (WAMEX/MINEDEX, OGS, NBMG, company
  archives).
- **OPEN-LODE-2.** The magnitude of effort endogeneity in real districts.
- **OPEN-LODE-3.** Three-dimensional under-cover point-process scale, and EM/IP stochastic inversion.
- **OPEN-LODE-4.** Pure-Rust nested-dissection ordering plus selected inversion for marginal variances. The lifted
  fs-sparse has AMD plus LDLᵀ only.
- **OPEN-LODE-5.** The value function V(m) for undiscovered deposits; it is the interface to §23.
- **OPEN-LODE-6.** MNPI and JV data-room policy with counsel.

## 12. Corebox: drilling, sampling, QA/QC and assays

Corebox owns layer L0 of the subsurface: immutable, bitemporal observations with characterized error. Everything
in Strata, Lodestar and Touchstone is only as good as Corebox's error models. It is also where MNPI is born.

### 12.1 Records (`fmine-drill`, `fmine-qaqc`)

| Record | Key fields | Notes |
|---|---|---|
| `Collar` | hole id, type (diamond DD, RC, aircore, RAB, sonic, blasthole), CRS-registered position with survey method and accuracy, planned vs actual, program, embargo state | Position accuracy is an error model, not a comment |
| `Survey` | depth, azimuth, dip, instrument (gyro, magnetic multishot, single-shot, none), magnetic interference flag, run direction (in/out), error model | "No survey" is a state, never an assumed straight hole. A magnetic survey in a domain with logged magnetite or high susceptibility is `Suspect` until a gyro check exists: shifts > 30 m are recorded in 150–200 m holes (E380, E381) |
| `Interval` | from/to, recovery % **with method** (length, mass, volume, unknown), lithology, alteration (type, intensity), mineralization, weathering, logger (human, scanner, ML model@version) | Each logger is a different evidence source; ML logs are `ModelledUncalibrated` until scored against human relogs |
| `Structure` | depth, alpha/beta (oriented core), type, confidence, orientation-line quality | Converted to true orientation with desurvey uncertainty |
| `Sample` | interval(s), mass, sample type (half core, quarter core, RC split, blasthole cone/full), splitter, chain of custody, dispatch batch | Support (mass and length) is first-class (§13.4) |
| `AssayResult` | sample, analyte, **method = measurand** (fire assay–AAS/gravimetric, screen fire assay, PhotonAssay, LeachWELL/BLEG, 4-acid ICP-MS/OES, aqua regia, fusion XRF, LECO S/C, sequential Cu leach CuT/CuAS/CuCN), value or censored (<DL, >UL), lab, batch, certificate, `qaqc_regime` (None, Partial, Current) per campaign | **Different methods measure different things.** A cyanide-soluble Au is not a total Au. Never merge methods silently. A method changeover is a covariate: Kibali CPA reads +2.7% against fire assay over 62,545 tandem samples (FACT, E225). Analyte mass fractions outside [0, 1] are refused on import: Porgera's database held S > 100% (FACT, C4 P12). |
| `QaqcInsertion` | CRM (certified mean, σ_C and σ_L, certificate source), blank (coarse/pulp), duplicate (field, coarse reject, pulp), umpire | Insertion rates per program are TARGETs set by QP policy. Hallmark shows target vs achieved, each with its denominator: Carlin achieved CRM 4.38% and duplicates 3.76% against 5% (FACT, E165); Kibali's Table 11-2 mixes two bases (INFERENCE, E226). Textbook defaults (field duplicates 5–10%, pulp 3–5%, CRMs 3–5%, ≥ 5% of duplicates external; E373) are `PolicyDefault`, never compliance. |
| `Density` | method (water immersion with/without wax coating, gamma log, pycnometer), moisture state | Density is a field (§13.2), not a constant per domain |
| `LabCertificate` | signed original in CAS; parsed results reference it; or the explicit state `LimsDirectNoCertificate` | Imports as `ThirdPartySigned` evidence for FA purposes (§21). Porgera's assays arrived with no certificates and "no way to validate" them (FACT, E299); §24.5 reports the no-certificate share per domain. The same record serves environmental samples (§18.4). |
| `GeometTest` | Bond work indices (BWi, RWi, CWi), SMC/JK drop-weight (A×b, ta), abrasion index, flotation and leach tests, preg-robbing index (PRI), mineralogy (QEMSCAN/MLA) | Located on composites with explicit support; feeds transfer cards (§13, §17) |
| `GeotechLog` | RQD (run-based), fracture frequency, joint condition, UCS/Is50 tests, GSI/RMR/Q (judgement indices, ordinal) | Ordinal indices are never averaged (INV-014) |
| `TwinPair` | original and twin hole, separation (m, from desurveyed traces), drilling techniques, matched intervals, per-pair CV_AVR%, RMA slope, paired test | Separation above the policy limit (default 5 m) renders `WeakTwin`; a twin study is a Claim over its pairs (E375, E376) |

### 12.2 Processing laws

- **Desurvey.** Minimum-curvature desurvey with survey-error propagation (an ISCWSA-style error model), giving
  each interval a positional covariance. Straight-line desurvey is only a declared fallback for unsurveyed short
  holes.
- **Compositing.** Domain-respecting, length- or mass-weighted, with residual handling declared. Conservation
  (Σ length × grade) is checked for every composite set (PO-CORE-2).
- **Interval joins.** Interval overlap joins are built in `fmine-drill` on an interval tree. frankenpandas
  `IntervalIndex` lookups are linear scans with no overlap join (FACT, R8), and `merge_asof` is point-based
  (FACT, R3).
- **Censored data.** Below-detection results are handled by censored likelihood (Tobit-style, or
  Kaplan–Meier/ROS for descriptive statistics). **Substituting DL/2 is refused** (guard G-CENS).
  frankenscipy's `CensoredData` exists but is used only in a log-rank test, so censored fitting is a build item
  (FACT, R8).
- **Embargo.** Every assay and intercept starts `Embargoed{wall}`. Release is a human act citing the public
  release (news release URL, filing ID) with its valid time. MNPI handling is in §31.1.
- **Recovery–grade bias test.** Per (domain, campaign, drilling method), report the dependence of grade on core
  recovery. A significant dependence opens an InvestigationCase and marks the affected intervals `Conflicted`.
  Poor recovery is the main diamond-drilling sampling error (E378).
- **Data-generation comparison.** Each campaign is compared with the others (Q–Q plots, means by domain) before
  it joins an estimate; one biased campaign was found only this way (E379).
- **Survey checks.** In/out run consistency and collar-vs-DTM residuals are recorded per hole (E381).

### 12.3 Sampling theory and measurement-error models (DESIGN)

- **Gy's theory of sampling.** For each sampling protocol (split sizes, crushing and pulverizing stages), compute
  the fundamental sampling error (FSE) variance:
  - the general form is σ²_FSE = (1/M_S − 1/M_L)·c·f·g·ℓ·d³, with d in cm, M in g and c in g/cm³;
  - the calibrated form is σ²_FSE = K·d_N^α·(1/M_S − 1/M_L). K and α are Claims with a calibration method
    (`Mste`, `ThirtyPieces`, `HeterogeneityTest`, `DrillData`) or `Assumed` (default K = 470, α = 1.5; E363). In
    lode gold K spans 1 to > 10,000 (E362), so an `Assumed` K makes the nomogram `Speculative`;
  - for coarse gold, nugget-effect (Ingamells) formulations apply (HYPOTHESIS: not confirmed by the corpus).

  The protocol's nomogram is a Claim (`Derived`, from declared constants with their own evidence classes). It
  records the grade at which K was calibrated and is `OutsideValidity` at other grades (E359). The default
  acceptance line is a relative variance of 0.01 (E360), registered as policy data. A sample made of fewer than
  30 increments is flagged.
- **Error-model decomposition.** The measurement-error variance for each (analyte, method, sample type) is
  decomposed into FSE, analytical error and method bias:
  - field, coarse and pulp duplicate pairs give the variance at each stage. The canonical statistic is
    CV_AVR% = 100·√((2/N)·Σ(a−b)²/(a+b)²) per (stage, analyte, grade band); HARD (= CV/√2) and AMPD (= √2·CV)
    are derived views (E371). Thompson–Howarth plots show their N (≥ 50 required) and are labelled "assumes
    normal errors; optimistic" (E369, E370). A duplicate's mass must match the original's (E374). No field
    duplicates at all (Porgera, C4 P12) sets `FieldDuplicateAbsent`: the stage decomposition is not identifiable;
  - CRMs give accuracy and bias;
  - blanks give contamination.

  This decomposition drives the geostatistical nugget split (geological vs sampling nugget, §13.4) and the
  reconciliation measurement nodes (§20). When the sampling share of the nugget exceeds a policy share, Helm's
  information options (§22) include "improve the sampling protocol" beside "infill drilling", priced by the
  same EVSI (E390).
- **Blasthole samples** have their own measurand error and bias models: cone sampling and sub-drill
  contamination. They are observations of the same belief at a different support, not a separate model (§13.6).

### 12.4 QA/QC as a sequential-evidence problem (DESIGN)

- **CRM monitoring.**
  - A CRM rule is policy data that names (a) its **SD basis**: certified within-lab σ_C, certified between-lab
    σ_L, or the lab's running S_W; and (b) its **test**: single-value |X − μ| ≤ k·SD, or the batch-mean
    CANMET test |m − μ| ≤ 2√(σ_L² + S_W²/n) (E364, E365). The default is a warning at k = 2 and a failure at
    k = 3 **on the named basis**. The same ten-result batch gives 10 warnings and 8 failures on σ_C, 3 and 0 on
    σ_L, and a CANMET pass, with a bias of about 12 standard errors (INFERENCE, E366). **Bias magnitude with
    its interval is always reported beside pass/fail.**
  - Site rules differ and are versioned per source document: Kibali fails a single result beyond 3 SD (E221);
    Loulo also fails three consecutive results beyond 2 SD on one side (E222); NGM fails two consecutive beyond
    ±2 SD on one side, with grade-tiered re-assay windows (E166). The same blank rule reads "and" in one NGM
    report and "either/or" in another (C2 §4.5b), so rule text is a metric-definition registry entry.
  - **Pattern monitors** run beside the e-process per (CRM, lab, method): two successive beyond 2 SD, four
    successive beyond 1 SD on one side, sign imbalance, and a **dispersion-collapse** monitor (running SD well
    below certified), which can indicate tampering (E367, E368). A dispersion-collapse alarm opens an
    InvestigationCase with integrity scope, not a recalibration.
  - Rates are disaggregated per lab: internal mine labs failed one CRM at 12.8% and 27% against 4.8% at an
    accredited lab (FACT, E167).
  - In addition to the threshold rules, an **anytime-valid e-process** per (CRM, lab, method) detects drift that
    threshold rules miss. Its alarm opens a QA/QC investigation (lifted `fs-eproc`).
- **Failure cascade.** A failed batch flips its results to `stale`/`conflicted` and propagates down the provenance
  DAG: composites, then projections, then claims. Affected projections are marked `Stale`, so a known known is
  revocable (§9.1). Re-assays supersede.
  - A batch committed with an unresolved failure is `CommittedFlagged` (Loulo commits re-failed batches; E223).
    Strata decides explicitly, per estimate, whether flagged assays enter.
  - Sample retention is a data gate: where originals are discarded after QC (Kibali CPA; E224), re-assay is
    impossible and the state says so.
  - "Control absent" periods are first-class: Cortez stopped outside check assays from 2015 to 2021 and used
    assay bias-adjustment factors until 2007 (FACT, E168, E169).
- **Duplicates** feed the measurement-error model continuously. Precision outside policy triggers an
  InvestigationCase.
- **Umpire assays** quantify inter-laboratory bias as a hierarchical bias term.
- **Twin holes.** G-TWIN: a correction derived from a twin study may feed a `Full`-basis internal projection with
  its uncertainty. It never enters a `Public` or disclosure-basis estimate without a recorded QP acceptance
  (E377). An **RC smearing monitor** opens an InvestigationCase proposing diamond twins when an RC hole shows a
  significant down-hole grade decline inside mineralized intervals (E391); NGM's audit records RC contamination
  below the water table (E164).
- **Lab turnaround** is a durable obligation (`OB-LAB`). A dispatched sample must resolve to a result or a
  declared loss with a QA/QC finding, so missing samples are never silent.

### 12.5 Statistical sampling of quality and quantity beyond drillholes

The same error-model discipline applies to every quantity-and-quality measurement in the mine. Each node's error
model enters reconciliation (§20).

| Measurement | Error model source | Consumer |
|---|---|---|
| Truck payload (onboard weighing) | Calibration against weighbridge; per-truck bias | Headframe (§16), Touchstone |
| Belt weightometer | Calibration chain, drift (e-process) | Crucible, Touchstone |
| Cross-belt / slurry analysers (PGNAA, XRF, LIBS) | Calibration to physical samples; matrix effects | Crucible blending, Touchstone |
| Mill feed / tails / concentrate samplers | Sampler design (cross-stream cut), Gy errors, bias tests | Metallurgical balance (§17, §20) |
| Stockpile surveys (drone/LiDAR) | Registration accuracy; bulking factor; base-surface uncertainty | Touchstone |
| Bullion / doré | Weight and fire assay of bars; refinery outturn | Bullion (§21) |
| Concentrate shipments | Draft survey, moisture determination, assay exchange, umpire | Bullion provisional pricing (§21) |

### 12.6 Guards, proof obligations and gates

| Guard | Condition | Action |
|---|---|---|
| G-CORE-1 (`G-TWIN`) | A twin-hole bias correction enters a `Public` or disclosure-basis estimate without a recorded QP acceptance (§12.4; E377) | Refuse publish; restrict twin-derived adjustments to `Full`-basis internal projections |
| G-CORE-2 | A drillhole interval, survey, density or plant/fleet sample enters L0 without a characterized measurement-error model or explicit `Uncharacterized` status (§12.1–12.5) | Refuse ingest or downgrade to `ModelledUncalibrated` |

| ID | Statement | Verification |
|---|---|---|
| PO-CORE-1 | Desurvey matches reference minimum-curvature results within 1 mm per 1,000 m | Oracle harness (textbook cases, lab oracle) |
| PO-CORE-2 | Compositing conserves Σ length × grade (and mass × grade) to 1e-12 relative | Property tests |
| PO-CORE-3 | No method merge without an explicit `MeasurandMapping` claim | Lint + test |
| PO-CORE-4 | A QA/QC failure cascades to every dependent projection within one anchor publication | Lab test |
| PO-CORE-5 | Embargoed results never appear in a Public basis or an unwalled projection | Wall differential tests (§31) |

| Gate | Threshold (TARGET) |
|---|---|
| GATE-CORE-01 | 10⁵ holes / 2×10⁷ intervals ingested, desurveyed and composited; full rebuild ≤ 15 min; as-known-at query from one anchor |
| GATE-CORE-02 | On synthetic QA/QC streams with injected drift, the e-process detects drift of 1 SD within the documented detection delay, with false alarms ≤ α; on streams with injected variance collapse (running SD at 0.3 of certified), the dispersion monitor alarms within its documented delay; the CRM scorer reports pass/fail on every named SD basis plus bias with its interval |

## 13. Strata: the Subsurface Belief Ledger

### 13.1 Verdict (Brenner pass B1)

| Hypothesis | Verdict | Role |
|---|---|---|
| Kriged block model as the authority (industry practice) | **Killed as authority** | Survives as a mandatory **declared-estimator projection** (`Estimator::DeclaredOk`) with bit-for-bit parity, so the QP signs on familiar ground |
| Ensemble of conditional simulations as the authority | **Downgraded** | Realizations are the exchange currency: regenerable, indexed by common random numbers, cached, never stored as truth |
| Generative hierarchical belief object as the authority | **Survives, amended** | Core design: staged ("cut") inference; a hypothesis set plus catch-all; procedure certificates in place of truth certificates |
| Event-sourced data plus recipes as the only authority | **Survives, merged** | Storage and provenance substrate. A recipe without a probabilistic model collapses back to the kriged model; reproducibility is not validity. |
| Support-indexed authorities glued by restriction maps | **Downgraded** | Kept as the reconciliation consistency layer. F-factor residuals are obstruction witnesses, but gluing alone cannot *learn* from a defect. |
| QP wireframes as the authority (legacy practice) | **Downgraded** | Wireframes become `AttestedInterpretation` inputs with a contact-uncertainty model |

**Why the kriged model cannot be the authority (GB-11).** Selecting blocks on an estimate selects its errors.
Under multi-Gaussian regression, E[Z | Z* = z*] = m + b(z* − m).
- Substituted: m = 1.0 g/t, selected-block mean estimate 2.0 g/t.
- With slope b = 0.8, the expected true grade is 1.8 g/t (−10%).
- With b = 0.5, a poorly informed domain, it is 1.5 g/t (−25%).
- Stress case: estimators that restore SMU-scale variance (LUC, single realizations) have σ_Z* ≈ σ_Z, so
  b = ρ·σ_Z/σ_Z* ≈ ρ(Z, Z*) by construction (INFERENCE, E355). Where local correlation is poor the slope
  collapses. One published check of Localised Uniform Conditioning (Abzalov 2006; E352) against grade control
  reported b ≈ 0.1 (Assibey-Bonsu & Muller 2014; FACT[U], single study, page not pinned in the corpus; E029).
  Then it is **1.1 g/t (−45%)**. The originator's own LUC check reports an RMA slope of 1 with r = 0.85, which
  implies b ≈ 0.85 (E354, E355), and restricts LUC to low-nugget domains whose data are not too sparse or remote
  from the panel (E353). The design does not depend on this extreme; it depends on the slope being *measured per
  domain against outcomes* rather than assumed.
- **Flip:** in a domain where, against grade-control truth, (i) b ≥ 0.95, (ii) ρ(Z, Z*) ≥ the policy minimum and
  (iii) tonnage and metal above the operating cut-off are within policy tolerance, `DeclaredOk` at SMU is adequate
  there and simulation buys little. **Slope alone is not enough:** correlation can be 0.5 while the slope is
  0.9–1.0 (E357), and in a textbook synthetic gold case b = 0.89–1.05 at every block size while tonnage error at
  cut-off stays excessive below the data spacing (E358).
- **Data gate.** Public filings cannot populate per-domain slopes. NGM's reports for Carlin, Cortez and Turquoise
  Ridge disclose neither kriging efficiency nor slope and classify by drill-spacing geometry (E158, E162, E163);
  Kibali and Loulo display slope only visually, with no numeric threshold (E227). A measured slope needs
  blasthole and GC data from the pilot (§37.3 M2). Grade-control grids of 5×5 to 20×20 m support using GC as the
  "truth" term (E384). The nearest public practice is estimator choice tuned against GC (Cortez ID3; E161).
- **The SMU carries the dilution.** Carlin and Cortez take open-pit dilution and mining recovery as "inherent in
  the SMU" (FACT, E185), so the SMU block size, and therefore this change-of-support argument, carries every
  dilution assumption in those reserves.

### 13.2 Layers and authority

| Layer | Content | Mutability | Authority? |
|---|---|---|---|
| L0 Evidence | Corebox records (§12), structural measurements, density, geophysics grids, mapping, blasthole samples, survey epochs, plant reconciliation | Append-only, bitemporal | **Yes (data)** |
| L1 Interpretation | Structural framework: stratigraphic/intrusive potential fields; faults with offset and chronology (cut/onlap/erode rules after GemPy and LoopStructural); vein and lode surfaces; weathering surfaces; **several domain partitions** (grade, oxidation, geometallurgy, geotechnics, hydrogeology); alternative hypotheses with weights plus a catch-all | Versioned; every version is a signed human act (`AttestedInterpretation`). **Software never edits L1.** | **Yes (spec)** |
| L2 Property fields | Per (partition, domain, variable group): anamorphosis, decorrelation, nested anisotropic covariance, trend, **hyperparameter particles with weights**, measurement-error model, method-bias terms, reconciliation bias hyperprior | Versioned; reweighting is a logged event | **Yes (spec)** |
| L3 Transfer cards | Recovery(Cu, CuAS/CuT, S, As…), throughput(A×b, BWi…), PRI(TOC), UCS(Is50), density(S, Fe), Hoek–Brown parameters, following frankensim's `CorrelationCard` pattern with `ValidityDomain` and colour | Versioned | **Yes (spec)** |
| L4 Projections | Block models (any frame, support, estimator), realizations, wireframes and grade shells, entropy maps, classification-support metrics, recoverable resources, domain summary material cards | Immutable, content-addressed, **regenerable**, cached with eviction | **No.** Each carries a `ProjectionReceipt`. |
| L5 Twin geometry | Depletion epochs (as-mined solids), design solids (plans, not belief), anthropogenic bodies (stockpiles, ROM, heaps, TSF) | Epoch-indexed events plus projections | Data yes; current state is a projection |

- The **`BeliefSpec`** is L1 + L2 + L3 + an evidence cut + a seed root: a small, canonical, hash-addressed
  document of KB–MB.
- **The QP/CP snapshot** is the triple `(EvidenceCut, SpecHash, Vec<ProjectionReceiptHash>)` plus a human
  `CompetentPersonAct` record. An independent QP MUST be able to reproduce the declared-estimator projection, the
  validation pack and the tonnage–grade certificate from the triple alone (GATE-STRATA-G5).

### 13.3 Type sketches (DESIGN; stable Rust, `#![forbid(unsafe_code)]`)

```rust
pub struct SpecHash([u8; 32]);
pub struct EvidenceCut { record_seq: u64, valid_time: TimeInterval, qaqc_policy: PolicyHash, exclusions: Vec<ExclusionActId> }
pub struct BeliefSpec {
    cut: EvidenceCut, seed_root: SeedRoot,
    structure: StructuralModelRef,                    // L1, attested
    hypotheses: Vec<(HypothesisId, Weight)>, catch_all: Weight,
    partitions: Vec<PartitionSpec>,                   // grade, oxidation, geomet, geotech, hydro
    fields: Vec<FieldSpec>, transfers: Vec<CardRef>,
}
pub enum Additivity { Extensive, MassWeighted, VolumeWeighted, NonAdditive { blend: BlendRuleId }, Ordinal, Categorical }
pub struct AttributeSpec { id: AttrId, unit: UnitId, measurand: MeasurandId, additivity: Additivity,
                           censoring: Option<DetectionLimitModel> }
pub struct FieldSpec { partition: PartitionId, domain: DomainId, vars: Vec<AttributeSpec>,
    anamorphosis: AnamorphosisRef, decorrelation: Decorrelation /* None | Maf | Ppmt */,
    covariance: NestedAnisotropic, trend: Option<TrendSpec>, hyper: ParticleSet<Hyper>,
    meas_error: MeasErrorModelRef, bias_prior: BiasPriorRef }
pub enum Support { Point, Interval{len_m: f64}, Composite{len_m: f64}, Block{d: [f64;3]}, Smu{d: [f64;3]},
                   Panel(PanelId), Stope(StopeId), Bench(BenchId), Parcel(ParcelId) }
pub enum Estimator { DeclaredOk(OkParams), Sk, Etype, Quantiles(Vec<f64>), ProbAbove(Vec<Cutoff>), Realization(u32),
                     SimulatedGc { pattern: GcPattern, dig: DigRule }, Luc, Uc, Dgm }
pub struct ProjectionRequest { spec: SpecHash, frame: GridFrame, support: Support, estimator: Estimator,
                               realizations: RealizationSet, attrs: Vec<AttrId>, region: RegionSel, det: DetClass }
pub struct ProjectionReceipt { request: RequestHash, spec: SpecHash, code: CodeVersion, det: DetClass,
    merkle_root: [u8; 32], conservation: ConservationWitness,
    validation: ValidationStats /* swath, NN-vs-estimate bias, kriging efficiency,
        slope {kind: RegressionTrueOnEstimate | Rma}, rho(Z, Z*), variance ratio σ_Z*/σ_Z,
        OK-vs-DGM grade–tonnage deviation at registered cut-offs, variance reproduction */,
    color: Color, staleness: Staleness, mnpi: MnpiLabel }
pub enum Staleness { Fresh, Stale { since: EventSeq, cause: StaleCause }, Superseded(RequestHash) }
```

- **Typed slope.** A lint refuses any rule that compares an `Rma` slope with a regression-slope threshold
  (E354, E355). Known answer: the validation stats flag 1×1 m blocks with σ_Z*/σ_Z = 0.45 and ρ = 0.41 even
  though b = 0.92 (re-derived synthetic case after E358).
- **`Estimator::Luc` carries a `ValidityDomain`:** nugget/sill ratio, data spacing vs variogram range,
  data-to-panel distance and a border-effect flag (E353). Outside it the projection renders `OutsideValidity`.
- **Imported legacy models.** A projection's receipt records model age and the shell and cut-off it was
  re-reported under: Kibali re-reports 2019–2020 models under a new shell and COG (C3 §3.11). An importer that
  meets a negative-grade reset (Loulo resets negative kriged grades to the nearest sample's grade; E229) records
  it as a `ModellingDecision`, and Strata never applies it silently.

### 13.4 Change of support and the information effect

- **Support is a type** (INV-014). Any operation mixing supports without a declared operator is refused.
- **Linear variables, and transforms that are not (G-STRATA-11):**
  - simulate at fine support, target ≤ SMU/4 per axis (e.g., 2.5 m nodes, 4 × 4 × 4 per 10 m SMU);
  - regularize in physical space. Normal-score anamorphosis, ILR and PPMT are nonlinear, so an average of
    transformed values back-transforms to the wrong grade, and an average of grades ignores density. Back-transform
    each node x_q to dry bulk density ρ(x_q) and concentrations z_m(x_q) first, then accumulate with node volume
    weights w_q: M_V = Σ_q w_q ρ(x_q), Q_{V,m} = Σ_q w_q ρ(x_q) z_m(x_q), g_{V,m} = Q_{V,m} / M_V, the
    density-weighted average Σρᵢvᵢgᵢ / Σρᵢvᵢ. G-STRATA-11 refuses any path that averages normal-score, ILR, MAF or
    PPMT values across supports before back-transformation;
  - density is itself a field. To first order, CV_T² ≈ CV_V² + CV_ρ², and in massive sulphides or variably
    decalcified Carlin-type host rocks density error can rival volume error.
- **The information effect** is modelled by `Estimator::SimulatedGc{pattern, dig}`. On each realization:
  1. sample a virtual blasthole grid with blasthole measurand error;
  2. re-estimate;
  3. classify by the operating rule, including minimum mining width;
  4. account the *true* tonnes and metal of what is sent where.

  This is the simulation route to recoverable resources.
- **Parity paths.** DGM/UC/LUC exist as projections because QPs and reviewers know them. They are never the
  authority.
- **Non-additive variables** carry `Additivity::NonAdditive{blend_rule}`, and naive averaging is refused.
  - Hardness: aggregate an additive proxy (specific energy at a declared circuit, or 1/throughput) and transform
    at the end.
  - Recovery: applies at a declared *processing* support (a shift or day blend), not per block (§17.3). Loulo
    estimates recovery per block by inverse distance (FACT, E230); FrankenMining imports such a field as a
    `NonAdditive` attribute that blends metal-weighted.
- **Grade remapping.** Grades are remapped only as metal and tonnes, never directly.
  frankensim's `conservative_cell_remap` conserves extensive quantities from caller-supplied overlap fractions
  (FACT, `fs-mesh/src/adaptivity.rs:1171`). FrankenMining computes exact overlap fractions itself for rotated and
  sub-blocked grids.

### 13.5 Scale: regenerate, don't store

- **Stored:**
  - evidence: 10⁵ holes ≈ 2×10⁷ intervals ≈ a few GB;
  - the spec and hyperparameter particles;
  - per-block **summaries** at SMU support: e-type, conditional variance, P(>c) at up to 4 cut-offs,
    P10/P50/P90 of metal, domain-membership probabilities, distance to data.

  TARGET ≤ 64 B per block, so 10⁸ SMU blocks ≈ 6.4 GB per version.
- **Regenerated.**
  - Any realization tile is rebuilt bit-identically from `(SpecHash, realization r, tile key)`. It uses keyed
    counter-based RNG (lifted fs-rand Philox `StreamKey`) plus conditioning by kriging:
    `realization_r = Z_uc(r) + K·(data − Z_uc(r)@data)`.
  - **Soft contacts keep common random numbers (DESIGN).** If the Philox key embeds a hard `domain_id`, moving a
    contact by 5 m between L1 hypotheses or after infill flips the domain of boundary blocks. Those blocks then
    draw independent innovations, and paired realizations decorrelate exactly where the decision lives (GB-12).
    For soft or uncertain contacts, `fmine-belief` keys the innovations on a domain-free background stream
    `("strata_bg", var_group, r, brick)` and blends the domain fields across the contact with a smooth weight of
    the signed distance φ(x): w = σ(φ(x)/ℓ) and Z = (w Z_a + (1 − w) Z_b) / √(w² + (1 − w)² + 2w(1 − w)ρ_ab), where
    ρ_ab is the correlation of the two domain fields at x, so the blend keeps unit variance in normal-score space.
    A small contact move then changes block metal continuously. The paired-correlation target (ρ ≥ 0.90 for a 5 m
    move) is an acceptance test (BD-317), not a property. Hard contacts keep the domain key. This is a
    partition-of-unity blend, not plurigaussian simulation, which thresholds Gaussian fields into categories.
  - Tiles are cached under a cost policy. The cache is never authoritative: a regeneration hash mismatch is
    refused (G-STRATA-5).
- **Grid substrate.**
  - Sparse bricks (8³ leaf, 32³ internal) after frankensim's `VdbGrid`, lifted for topology only.
  - Attributes are stored as **columnar per-attribute chunks**. `VdbGrid` leaves are dense arrays of structs
    (`[T; 512]` per leaf regardless of occupancy, FACT, `vdb.rs:20-31`), which wastes memory on thin veins.
  - Per-brick hashes roll up into a Merkle root per projection, which drives incremental diffs.
- **Simulation engines.** Fine-support realizations use turning bands, FFT moving average or sequential Gaussian
  simulation, plus kriging correction. SPDE/GMRF is used only for hyperparameter inference and coarse latent
  fields (≤ 10⁶–10⁷ nodes), because nested-dissection fill for a 3D GMRF grows about as n^(4/3).

**GB-10 · Store or regenerate realizations?**
- **Equation.** S_store = N_b·R·V·b; T_regen = N_b·R·c_node/P.
- **Substituted.**
  - Storing: N_b = 10⁹ blocks, R = 100 realizations, V = 10 attributes at 4 B gives **4 TB per model version**.
  - Regenerating: an SMU projection of 10⁸ blocks at R = 100, with c_node = 2 µs (TARGET) on 64 cores, takes
    **≈5.2 min**; at 10⁹ blocks, ≈52 min.
- **Flip.** Any of these changes the answer and leads to materializing SMU ensembles for active zones only:
  - c_node > ~50 µs (e.g., multiple-point statistics);
  - more than ~20 full-ensemble queries per day;
  - a regulator requiring retained realizations (OPEN-STRATA-4).

**GB-12 · Common random numbers make "did the new drilling change the decision?" answerable.**
- **Equation.** Var(X_new − X_old) = Var(X_new) + Var(X_old) − 2ρσ_newσ_old.
- **Substituted.** σ = 10 koz of panel metal.
  - Independent seeds give an SD of the difference of 14.1 koz.
  - Common random numbers with ρ = 0.95 give **3.2 koz**.
  - Detecting a 5 koz shift then needs about 20× fewer realizations.
- **Flip.** An interpretation change or a large hyperparameter shift decorrelates the realizations. The change
  is then reported as a structural update, not a paired delta.

### 13.6 Elements, deleterious elements, geometallurgy and geotechnics

| Element / attribute | Issue | Design response |
|---|---|---|
| Au (nuggety, coarse) | High nugget; FSE dominates; capping is an estimator choice | Measurement-error model separates geological from sampling nugget. Capping is a logged `ModellingDecision` with a metal-at-risk sensitivity. Use indicator or mixture fields where multi-Gaussianity fails. |
| Ag | Correlated with Au (epithermal) or base metals | Co-simulation in decorrelated factor space |
| Cu (CuT, CuAS, CuCN) | Sequential-leach ratios route ore to oxide, transition or primary processing | Model CuT plus *ratios* in log-ratio space with closure. Never krige CuAS and CuT independently. |
| Sn | Recoverable Sn depends on mineralogy (cassiterite vs stannite) | Distinct measurands `Sn_total` and `Sn_cassiterite`; geometallurgical card for recovery |
| As, S, Hg, Sb, Bi, F, Cl | Smelter penalties; ARD (PAG/NAF); Hg often below detection | **Censored likelihood**; heterotopic co-simulation; S feeds ARD classification (NAPP/NPR projections, §18.5) |
| Carbonaceous / preg-robbing (TOC, PRI) | Few PRI tests on large composites | `CorrelationCard` PRI(TOC, domain) with a validity box; a field on TOC; recovery card consumes PRI per realization |
| Major components (Cu %, S %, Fe %) | Closed compositions | ilr coordinates where closure is material; trace elements in log space |
| Density | Varies with sulphide content and porosity | A field, with a regression card on S and Fe |
| Hardness (A×b, BWi, SPI) | Non-additive | Additive proxy, aggregated then transformed (§13.4) |
| RQD, GSI, RMR, Q | Ordinal or judgement indices | `Additivity::Ordinal`, `Estimated` colour only; categorical or truncated-Gaussian fields with an elicitation record |
| UCS | Sparse lab tests, many point-load tests | `CorrelationCard` UCS(Is50) per domain; a field with per-method error |
| Structural fabric | Directional data | Fisher statistics per structural domain; discrete fracture network later (OPEN-STRATA-9) |

Different domain partitions over one structural framework (grade ≠ geotechnical ≠ weathering ≠ hydrogeological
units) are first-class: `Partition{id, kind, boundary_rules}`.

### 13.7 Reconciliation and incremental update inside Strata

- **Grade control is an observation, not a peer model (DESIGN).** Blasthole and RC grade-control assays enter L0
  with their own measurand, support (cone or full hole, RC interval), error variance σ²_ε,BH and bias prior, beside
  diamond core (support v_DD, error variance σ²_ε,DD). Fine-support realizations and SMU projections are
  conditioned on both supports at once by multi-support kriging with heteroscedastic measurement error:
  Σ_j λ_j (C̄(v_i, v_j) + δ_ij σ²_ε,i) + μ = C̄(v_i, V) for each i, with Σ_j λ_j = 1, where
  C̄(v_i, v_j) = (1 / |v_i||v_j|) ∫_{v_i}∫_{v_j} C(x − x′) dx dx′ is evaluated by product Gauss–Legendre quadrature
  along both desurveyed segments and σ²_ε,i comes from Corebox's Gy FSE, duplicate and CRM model (§12.3–12.4). A
  blasthole bias that is not zero enters as its own drift term with its own unbiasedness constraint. The "GC
  model" is the projection of the belief at T_gc, built with `as_of_decision(T_gc)` (§6.3).
- **F-factors.**
  - F1 = proj(spec@T_gc) / proj(spec@T_resource) over the *as-mined* volume.
  - F2 = plant back-calculated / proj(spec@T_gc).
  - Each period's forecast is registered before the outcome (§10).
  - Residuals update **domain-period bias hyperpriors**: the learning loop that factor multipliers lack (§20.5).
- **New drilling** changes only the kriging correction inside the neighbourhood. Only dirty bricks are recomputed
  (Merkle diff), and realization indices persist, so before/after comparisons are paired (GB-12).
- **Hyperparameter particles** are updated by importance reweighting. ESS < 0.3·N triggers a refit (G-STRATA-8).
- **Contradictions.** An interpretation contradiction (ore logged outside its domain beyond tolerance, or an
  unexpected fault offset) raises a surprise event (a posterior-predictive e-value). The affected volume becomes
  `conflicted`, and a human re-interpretation is required.

### 13.8 Exploration-stage instantiation

When Lodestar's prospect is drilled, the deposit hypothesis instantiates a `BeliefSpec`:
- its priors (global mean, volume, grade–tonnage) come from the deposit-type model as hierarchical hyperpriors;
- L1 starts hypothesis-heavy, with many structural alternatives and a large catch-all.

The same types are used from first hole to closure (HYPOTHESIS; tested by instantiating on a public early-stage
dataset).

### 13.9 frankensim mapping (verified read-only at frankensim `20463525d`, 2026-10-26)

**Code facts that shape the plan (FACT):**
1. **`VdbGrid` is std-only and liftable** (`fs-rep-sdf/src/vdb.rs`, 310 lines; imports only `std`). The Cx
   coupling is at crate level, not in this file.
2. Its leaves are dense arrays of structs (`Leaf<T>{mask: [u64;8], values: [T;512]}`). It has no frame or rotation,
   no serialization and no out-of-core iteration. The 10⁹ target is "scaled in CI (~12.5k actives)"
   (`fs-rep-sdf/CONTRACT.md:203`).
3. **`fs-rep-voxel::DensityField` is a volume fraction in [0,1]**, not rock density. It is a false friend.
4. **`fs-geocon::volume_certified` cannot certify realistic orebody geometry.**
   - It requires an exact-distance chart (`primitives.rs:783-793`).
   - Mesh, tiled-SDF and occupancy charts make no such claim, and F-rep claims exactness only for primitives and
     *axis-aligned* half-spaces, so an oblique fault plane does not qualify.
   - It caps work at 256³ cells on a serial dense grid.
5. **The EnSRF core is Cx-free** (`fs-assimilate/src/nonlinear/ensemble.rs`, 273 lines). Its member-major dense
   state limits it to low-dimensional states (10⁸ blocks × 100 members would need 80 GB).
6. **`fs-evidence` needs nightly Rust** through `fs-qty`'s `generic_const_exprs`. "Asupersync-free" does not mean
   "stable Rust".
7. **`winding_exact` is liftable** (214 lines) but uses `std::atan2`, which must be swapped for deterministic
   math for cross-ISA reproducibility, and it imports `fs_geom::{Point3, Vec3}`, which the lift replaces with
   `fmine-geom` types.
8. **`fs-uq::KlExpansion`** is dense and isotropic-only: exponential and squared-exponential covariances.
9. **`fs-sparse`** has a supernodal LDLᵀ with AMD ordering, but no selected inversion for marginal variances.
10. **The geometry trait uses `fs_exec::Cx`**, a per-tile context, not `asupersync::Cx`. The suite has three
    distinct `Cx` types (FACT, B4 S1).
11. The matdb seed corpus has no rock, ore, soil or tailings bundles (FACT, E009; re-counted in E824).

**Per-crate disposition (DESIGN).** Postures: **Lift** (copy with a `//! Provenance:` header plus a golden-hash
test), **Bridge** (gated dependency through `fmine-simbridge`), **Pattern** (copy the design, not the code),
**Oracle** (dev-only), **Don't**.

| frankensim crate / item | Posture | FrankenMining target | What FrankenMining builds itself |
|---|---|---|---|
| fs-rep-sdf `VdbGrid` | Lift `vdb.rs` (topology/index only) | `fmine-geom::vdb` | Columnar attribute chunks; rotated/offset grid frames; sub-blocking (scheme OPEN-STRATA-1); per-brick Merkle; out-of-core paging; parallel brick iteration |
| fs-rep-mesh `winding_exact`, `WindingOctree` | Lift `winding.rs` (swap `atan2` → `det::atan2`) | `fmine-geom::winding` | Mesh ingest (OMF/DXF); proportion-in-wireframe per block with an error bound |
| fs-ivl, `fs-math/eft.rs`, exact predicates | Lift | `fmine-geom::exact` | Robust orient3d for wireframes and shells |
| fs-geocon `volume_certified` | **Don't** | — | `fmine-geom::volcert`: divergence-theorem volume of certified closed meshes in interval arithmetic; sure/band block-proportion enclosures |
| fs-topo `manifold_certificate`, `self_intersection_certificate` | Bridge after GATE-FSIM-RECONV, else Pattern | `fmine-geom::meshcheck` | Wireframe validity report (watertight, no self-intersection) in the QP pack |
| fs-rep-mesh `dual_contour`, `mesh_to_sdf` | Bridge later, or reimplement | `fmine-geom::contour` | Wireframe and grade-shell extraction from implicit fields on bricks |
| fs-rep-frep | Pattern | `fmine-belief::structure` | Hard/blend CSG for design envelopes (pit/stope solids, exclusion zones). Geological chronology (fault offsets, erode/onlap) is *not* min/max CSG. |
| fs-rep-voxel | Pattern | `fmine-geom` | Domain-code bricks; `FrameMismatch` refusal |
| fs-mesh `delaunay`, `conservative_cell_remap` | Pattern now; Bridge later for geotech meshes | `fmine-geom::remap` | Exact overlap fractions; tonnes-plus-metal conservation witness |
| fs-query BVH | Pattern (FrankenTextiles already lifted `bvh.rs`) | `fmine-geom::bvh` | Exact segment–triangle tests for drillhole–wireframe intersection |
| fs-asbuilt | Pattern (Kabsch/Umeyama plus pose covariance) | `fmine-ops::asbuilt` | Survey-epoch ingest (LAS/LAZ, DTM), ICP registration, surface-to-surface volumes |
| fs-rand (Philox `StreamKey`, Sobol/Owen) | **Lift** | `fmine-core::rand` | Key schema `(seed_root, partition, domain, var_group, r, brick)` |
| fs-sparse (CSR/COO, supernodal LDLᵀ + AMD, PCG, smoothed-aggregation AMG) | **Lift** | `fmine-numerics::sparse` | Selected inversion, or Hutchinson/RBMC variance estimates; SPDE mesh builder |
| fs-assimilate EnSRF (`nonlinear/ensemble.rs`) | **Lift** | `fmine-numerics::enkf` | Low-dimensional states only: domain bias factors, density regressions, grade-control zone patches |
| fs-uq `KlExpansion`; fs-bo `Gp` | **Oracle** | `fmine-oracles` | Full kriging engine (SK/OK/UK/co-kriging/block); turning bands, FFT-MA, SGS |
| fs-evidence | **Lift** types; shim `QuantitySpec` for stable Rust | `fmine-evidence` | Evidence-class mapping (§7) |
| fs-eproc, fs-voi, fs-robust (cvar), fs-tropical | **Lift** | `fmine-numerics::{eproc, voi, risk, tropical}` | Monitor wiring; Monte Carlo preposterior VOI (§11.5) |
| fs-qty | **Don't** (nightly) | — | `fmine-units` on stable Rust, modelled on fs-qty |
| fs-matdb / fs-matdb-store | **Pattern + format-compatible export**; sealed-corpus pattern | `fmine-materials` | Geomaterial packs in `frankensim.matdb-manifest.v1` format (§13.10) |
| fs-convection `CorrelationCard` | Pattern | `fmine-materials::cards` | Transfer cards (L3) |
| fs-airflow (ventilation networks) | Bridge after RECONV; otherwise Pattern | `fmine-ops::vent` (§16.6) | Network solve from mine development geometry |
| fs-conduction (operator twin of confined Darcy seepage) | Bridge after RECONV | `fmine-tailings::seepage`, `fmine-geotech::pore` (§14, §18) | Unconfined/Richards seepage is a gap (E010); build or request upstream |
| fs-tribo `ArchardLaw` | Lift or Pattern | `fmine-ops::wear` (§16.5) | Liner, ground-engaging-tool and pipe wear cards |
| fs-solid (J2 plasticity only) | Bridge only when Mohr–Coulomb/Hoek–Brown exist upstream (upstream ask) | `fmine-geotech::fem` | Limit-equilibrium analysis is built in-house (§14.2) |
| fs-lbm (no Bingham/Herschel–Bulkley) | Don't (gap) | — | Tailings and paste rheology as correlation cards (§17, §18) |
| fs-vvreg | Pattern | `fmine-oracles::registry` | Benchmark registry with pinned editions |
| fs-blake3 | Lift | `fmine-core::hash` | — |
| fs-rep-nurbs, fs-rep-neural, fs-xform, fs-motion | Don't (checked null) | — | — |

**Rules (DESIGN).**
- **R-1.** No FrankenMining *authority* type (evidence, BeliefSpec, receipts, brick format) mentions a frankensim
  type. Lifted code is re-owned under `fmine-*` with provenance headers and golden hashes.
- **R-2.** FrankenMining numerical kernels take a cancellation/budget closure (`FnMut() -> bool`, as EnSRF does)
  and are driven from asupersync regions at the orchestration layer. This avoids a second `Cx` type
  (`fs_exec::Cx`) and asupersync 0.5/0.6 mixing (E011).
- **R-3.** One optional crate, `fmine-simbridge`, implements `fs_geom::Chart` for FrankenMining implicit fields
  and block projections, so frankensim solvers (CutFEM, seepage, ventilation, eventually Mohr–Coulomb FEM) can
  consume them. It is the only place `fs_exec::Cx` may appear. It does not exist until GATE-FSIM-RECONV passes
  (§27.1).
- **Consequence.** frankensim's non-convergence can delay *engineering consumers* (geotechnical FEM, seepage,
  ventilation solves) but cannot block the resource representation.

### 13.10 Where material properties live (the materials database answer)

| Kind of knowledge | Home | Examples | Colour / authority |
|---|---|---|---|
| Located measurements | L0 evidence (spatial) | Assays, density determinations, UCS/Is50, Bond/SMC/A×b on located composites, PRI | Measured; they feed fields |
| Spatially varying attributes | L2 fields → L4 block projections | Grades, density, hardness proxies, RQD, UCS, TOC, oxidation state | Estimated; Validated only after reconciliation calibration |
| Empirical laws | L3 correlation cards (`fmine-materials::cards`) | UCS(Is50), density(S, Fe), Hoek–Brown, Bond's law, PRI(TOC), recovery(CuAS/CuT), Archard wear coefficients, slurry rheology | Validated(regime, dataset) or Estimated; never promoted by verification |
| Non-spatial material classes | `fmine-materials` sealed private corpus in `frankensim.matdb-manifest.v1` format | Mill-liner and GET steels, grinding media, reagents (cyanide, lime, xanthates, frothers, flocculants), process water chemistry, CRMs and reference ores, TSF construction-zone design parameters, geomembranes, explosives products | Per-claim provenance; `NoClaimInDomain` refusal outside the corpus |
| Domain summary cards (export) | L4 projection → matdb-format bundle | "Domain 12 fresh granodiorite: UCS distribution, E, ν", for frankensim solvers through the bridge | Projection receipt hash in `citation`; Estimated |

- Client test data is confidential and MNPI-adjacent, so only **sealed** corpora are used (`seal_corpus`;
  `StoreError::CorpusChanged`, FACT, `fs-matdb-store/src/lib.rs`), never seed bundles.
- The same located test result lives once in L0. matdb cards are *projections* of it (one authority per number).
- matdb's pack schema is versioned upstream (v8 at `20463525d`, E824), so frankensim-side additions are governed
  upstream changes. FrankenMining emits compatible bundles and never edits frankensim's corpus.

### 13.11 Certificates, guards and proof obligations

| Certificate | Content | Verifiable by |
|---|---|---|
| `TonnageGradeCertificate` | Region, support and cut-off spec. Tonnes and metal per element as distributions (P10/P50/P90 plus Monte Carlo half-width); grade derived as metal ÷ tonnes. `procedure`: regeneration hash OK, conservation OK, optional volume enclosure. Calibration flag and colour. | `fmine-verify` replay from the snapshot |
| `VolumeEnclosure` | [lo, hi] from the interval divergence theorem on a certified closed mesh | Interval re-evaluation; admissible as `Certified<f64>` (Enclosure) |
| `ConservationWitness` | Σ tonnes and Σ metal per element before and after every support change; max relative residual | Recompute |
| `RegenerationWitness` | Per-brick BLAKE3 of regenerated bytes; determinism class | Regenerate on the reference ISA |
| `ClassificationSupportPack` | Per block or volume: kriging efficiency, slope of regression, drill spacing vs range, P(±15% over a quarterly volume at 90%) (a heuristic, labelled as such), AOA distance | Evidence **for** the CP act; never a class label |
| `ReconciliationWitness` | Per period and domain: forecast registered before the outcome, outcome, PIT value, e-process state | Forecast ledger |

| Guard | Condition | Action |
|---|---|---|
| G-STRATA-1 | Aggregating a `NonAdditive`/`Ordinal` attribute across supports without a blend rule | Refuse `NonAdditiveAggregation{attr, fix}` |
| G-STRATA-2 | Mixing supports without a declared operator | Refuse `SupportMismatch` |
| G-STRATA-3 | Mixing grid frames | Refuse `FrameMismatch` |
| G-STRATA-4 | Conservation residual > 1e-9 relative (extensive attributes) | Refuse publish |
| G-STRATA-5 | Regenerated brick hash ≠ receipt | Refuse; mark `Corrupt`; re-materialize on the reference ISA |
| G-STRATA-6 | QA/QC invalidation or new evidence in the region after the cut | Mark `Stale`; block use in disclosure packages |
| G-STRATA-7 | Block beyond k × range from data | Label `Extrapolated`; exclude from classification-support metrics |
| G-STRATA-8 | Particle ESS < 0.3·N | Flag `HyperDegenerate`; schedule a refit |
| G-STRATA-9 | Intercept contradicts interpretation beyond tolerance | Surprise event; volume `conflicted`; human re-interpretation |
| G-STRATA-10 | Below-detection values substituted by constants | Refuse; require the censored model |
| G-STRATA-11 | Averaging `ILR`, `MAF`, `PPMT` or normal-score values across supports before back-transforming to density and concentrations (§13.4) | Refuse `TransformSpaceAverage`; regularize mass and metal in physical space |

| PO | Statement | Verification | Failure → mitigation |
|---|---|---|---|
| PO-STRATA-1 Regeneration determinism | Same (spec, request, code, determinism class) ⇒ bit-identical bricks | Golden hashes across runs and machines | G-STRATA-5; reference-ISA rebuild |
| PO-STRATA-2 Conditioning | Zero-nugget realizations honour data exactly; with an error model, residuals follow the declared error | Unit and statistical tests | Refuse the spec |
| PO-STRATA-3 Conservation | Extensive totals invariant under every support change | Witness | Refuse publish |
| PO-STRATA-4 Dispersion | Variance of regularized realizations ≈ γ̄(D,D) − γ̄(v,v) (Krige's relation) | Monte Carlo test with tolerance from the number of realizations | Flag a covariance or regularization bug |
| PO-STRATA-5 Parity | `DeclaredOk` equals a reference ordinary kriging (GSLIB kt3d-class lab oracle) within 1e-6 relative | Oracle harness | Block the QP pack |
| PO-STRATA-6 Calibration | On synthetic truth, nominal 80% panel intervals cover 0.72–0.88 | GATE-STRATA-G3 | Downgrade to "uncalibrated"; declared-estimator fallback |
| PO-STRATA-7 Incremental equivalence | Incremental update ≡ from-scratch (bitwise with identical neighbourhoods); indices preserved | Replay test | Force full recompute |
| PO-STRATA-8 Volume enclosure | True volume ∈ [lo, hi] for a certified closed mesh | Analytic shapes | Refuse the certificate |
| PO-STRATA-11 Reweighting | Self-normalized importance weights consistent; ESS guard enforced | Known-posterior (conjugate) test | Refit |

### 13.12 Gates, failure modes and open questions

| Gate | When | Pass criterion (TARGET) | On fail |
|---|---|---|---|
| ✖ GATE-STRATA-G0 (frankensim spike) | S+5 (month 17) | A separate clone at a pinned commit: a scratch crate depending on fs-rep-mesh + fs-topo + fs-sparse builds with one unified asupersync; one `Chart` round-trip from a FrankenMining field | Kill the bridge; lift-only (no impact on the resource path) |
| GATE-STRATA-G1 (parity) | S+6 (month 18) | `DeclaredOk` vs reference OK: estimates and kriging variances ≤ 1e-6 relative; desurvey ≤ 1 mm/1,000 m; identical composite intervals | Block the QP pack until fixed |
| GATE-STRATA-G2 (regeneration and throughput) | S+7 (month 19) | Bit-identical bricks across 3 runs × 2 machines (same ISA); 10⁷ blocks × 100 conditional realizations ≤ 3.3 min on 64 cores (c_node ≈ 12.5 µs; GB-10's TARGET of 2 µs gives ≈ 31 s); summaries ≤ 64 B/block | Optimize; if > 4× over (c_node > 50 µs, GB-10's flip), materialize SMU ensembles for active zones only |
| ✖ GATE-STRATA-G3 (calibration) | S+8 (month 20) | Synthetic-truth mine: 80% panel-metal intervals cover 0.72–0.88 over ≥ 50 panels; injected 5% F2 bias recovered within ±2%; injected 8% resource bias reduced to < 2% after 12 periods | Ship only `DeclaredOk` plus ensembles labelled "uncalibrated"; no probabilistic claims in management views |
| GATE-STRATA-G4 (scale and incremental) | S+10 (month 22) | 10⁸ SMU blocks end to end with out-of-core paging, peak RAM ≤ 50% of host; +100 holes recomputes ≤ 5% of bricks and satisfies PO-STRATA-7 | Restrict v1 to ≤ 10⁷ blocks per domain-region |
| ✖ GATE-STRATA-G5 (QP replay) | S+12 (month 24) | An independent QP reproduces `DeclaredOk`, the validation pack and the TonnageGradeCertificate from the snapshot triple alone, in ≤ 1 day | Revert to the declared-estimator authority, with the belief ledger as advisory |

"S+n" is month n of Strata work, which begins at M3 (month 12, §37); "S+" avoids any clash with milestone
ids M0–M6. Strata is not on the
wedge's critical path: the wedge consumes vendor block models as observations (B5).

| Failure | Detection | Deterministic fallback |
|---|---|---|
| Simulation infeasible within budget | Budget exhaustion | Publish `DeclaredOk` + LUC/DGM only, labelled `UncertaintyUncharacterized`; no intervals shown |
| Hyperparameter inference degenerate | G-STRATA-8 | Ensemble over ≥3 QP-declared defensible variograms (equal weights), labelled elicited |
| Structural contradiction | G-STRATA-9 | Freeze the last attested L1; volume `conflicted`; excluded from classification support |
| Calibration fails in reconciliation | e-process alarm | Widen by measured variance inflation (an explicit, visible claim); open a hypothesis review; never a silent F-multiplier |

**Open questions.**
- **OPEN-STRATA-1.** The sub-blocking scheme sites use (octree vs per-parent variable sub-cells), and its
  interoperability.
- **OPEN-STRATA-2.** Will QPs sign `DeclaredOk`, the simulation e-type, or both side by side?
- **OPEN-STRATA-3.** Which grade-control models are produced in other software and must be ingested as
  observations?
- **OPEN-STRATA-4.** Do regulators or auditors require *retained* realizations? If so, GB-10 flips for disclosed
  snapshots only.
- **OPEN-STRATA-5.** The dig-line rule under several live hypotheses (expected-loss dig lines; §15.7).
- **OPEN-STRATA-6.** Licensing of lifted MIT-plus-rider files in a client-facing product (RISK-17).
- **OPEN-STRATA-7.** How much the staged ("cut") inference understates uncertainty on realistic deposits.
- **OPEN-STRATA-8.** The minimum geometallurgical sampling density at which transfer cards beat domain averages.
- **OPEN-STRATA-9.** Discrete fracture network modelling scope.

## 14. Highwall: geotechnics and ground control

Highwall is the most safety-critical part of the system's cognition. Its doctrine is therefore the strictest:
- **FrankenMining computes, tracks evidence state, forecasts and scores. The geotechnical engineer and the EoR
  sign.** `geotechnical_signoff` never appears as true (INV-002).
- A stale instrument is `not_observable`, never "stable" (FMINE-REF-404).
- Probabilities of failure are stated only with an admissible reference class or a vetted model. Otherwise the UU
  term is a flip margin with the probability *not stated* (§9.3).

### 14.1 Rock mass model

- Inputs from Strata:
  - geotechnical domain partition;
  - RQD, fracture frequency, joint condition;
  - UCS fields (via the UCS(Is50) card);
  - GSI/RMR/Q as ordinal fields with elicitation records;
  - structural fabric per structural domain (Fisher or bimodal Bingham distributions of joint sets, with
    persistence and spacing distributions).
- Material models are **cards** that consume field projections:
  - Hoek–Brown (σci, mi, GSI, D disturbance), Mohr–Coulomb equivalents, and anisotropic (ubiquitous-joint) models;
  - each card has a validity domain and colour.
- **Card rules from the corpus (E392, E396–E398).**
  - *Water counted once.* RMR carries a groundwater rating and Q carries Jw. An index-derived strength card
    declares `water_basis: dry_normalised | as_logged`; an LEM run with explicit §14.5 pore pressure accepts
    only `dry_normalised` inputs (groundwater rating dry, Jw = 1). Otherwise lint raises "water
    double-counted" (INFERENCE).
  - *Q card:* Q = (max(RQD, 10)/Jn)·(Jr/Ja)·(Jw/SRF) (E397). Its basis is tunnel support, so open-pit use is
    `OutsideValidity` without site calibration (INFERENCE).
  - *UCS(Is50) card:* UCS = k·Is50 with k calibrated per domain; the textbook prior k ≈ 20 with ±20% test error
    (E396) alone is `Assumed`.
  - Classification indices are never the sole design input (E392).
  - *Model currency.* Every interpretation and analysis feeding a design carries its date and review state. Carlin's
    GQ7CN highwall failed on 11 Feb 2024 (~1 Mt); the review board found the LOM designs rested on a 2008
    geology model and 2018 analyses that were not independently reviewed (FACT, E195). An unreviewed or stale
    basis renders on the slope card.

### 14.2 Open-pit slope design support

| Analysis | Method | FrankenMining output |
|---|---|---|
| Bench-scale kinematics | Planar, wedge and toppling admissibility from joint-set orientation distributions against face orientation per design sector (stereographic logic); Monte Carlo over orientation, persistence and friction | P(kinematic release) per bench face per sector, with expected spill and catch-bench retention; registered for scoring against mapped bench failures |
| Inter-ramp and overall | Limit equilibrium: Bishop simplified, Janbu, Spencer, Morgenstern–Price on 2D sections cut from the belief ledger, with pore pressure from §14.5 | FoS distributions over realizations of strength, structure and pore pressure; PoF = P(FoS < 1) **only** when the inputs are calibrated; otherwise a flip margin ("the overall slope reaches FoS 1.0 if pore pressure ratio rᵤ > 0.32") |
| Acceptance criteria | Site acceptance policy registered as data, with `dac_basis ∈ {FoS, PoF, FoS+PoF}` and a nullable `pof_threshold`. The corpus's site criteria are deterministic FoS only: Kibali open pits use a consequence-and-confidence DAC with an example lower DAC of FoS 1.2 (FACT, E214); Kibali CTSF3 requires FoS > 1.5 static and > 1.1 pseudo-static (FACT, E215). PoF criteria (e.g. Read & Stacey 2009) are FACT[U] here | Pass/margin per sector on the site's own basis, rendered as *evidence for* the engineer's design acceptance; PoF is shown beside an FoS-only criterion as evidence, never as the criterion |
| Numerical analysis | Shear-strength reduction FEM, via `fmine-simbridge` only once frankensim has Mohr–Coulomb/Hoek–Brown (an upstream ask; fs-solid is J2-only today, E007) | Until then, LEM plus lab-oracle comparison (Slide/SLOPE/W-class tools) on benchmark sections |

Limit-equilibrium solvers are built in `fmine-geotech::lem`. They take known-answer tests from published benchmark
problems (ACADS-style), and their pinned oracle outputs are never regenerated from the candidate.

### 14.3 Underground ground control

- **Stope stability.** Empirical stability graph (Mathews/Potvin) as a card with validity domain; hydraulic radius
  vs modified stability number N′; dilution (ELOS) forecasts registered and scored against cavity monitoring
  scans (CMS) of mined stopes. The scan-vs-design comparison is computed in `fmine-ops::asbuilt` (§16.2).
- **Ground support.** Demand (dead-weight wedge, depth-of-failure from empirical relations) vs capacity (bolt, mesh,
  shotcrete capacity cards) per excavation class; factor-of-safety distributions; installation QA records (bolt
  pull tests) as evidence.
- **Mine seismicity.** Microseismic event catalogues → Gutenberg–Richter b-value and activity-rate estimates per
  volume and time; anytime-valid change detection on event rates (lifted `fs-eproc`) after blasts and production
  changes; exposure-based re-entry rules are policy data. FrankenMining never declares an area "safe"; it states
  the evidence state and the rule outcome.
- **Caving (block/panel cave) — Phase 2.** Cave propagation and fragmentation are research-grade (OPEN-HIGH-3);
  Laubscher-style empirical cards with explicit validity domains only.

### 14.4 Monitoring and TARPs (Trigger Action Response Plans)

- Instruments: survey prisms, slope stability radar, InSAR, GNSS, extensometers, inclinometers, time-domain
  reflectometry, piezometers, microseismic. Each is a packet-plane stream (§6.1) with a calibration and a staleness
  monitor.
- **TARP automaton (DESIGN).** Each monitored domain has a registered TARP (policy data authored by the geotechnical
  team): levels (Normal/Level 1/Level 2/Level 3) with trigger predicates over instrument evidence (displacement,
  velocity, acceleration, pore pressure, event rate) and *required responses* (inspect, restrict access, evacuate,
  notify). FrankenMining evaluates triggers, raises `OB-TARP` obligations (acknowledged and escalated per the TARP)
  and records responses; it never executes them (§31.3).
- Trigger predicates may use antecedent rainfall over a registered window (e.g. 60 days, set by site
  calibration) and reservoir or pit-lake level, not only piezometers: rising groundwater is the dominant landslide
  trigger (E394, E395). A creep forecast that assumes deceleration is a registered hypothesis scored like any
  other; "creep will stabilise" is never a default (Vaiont; E395).
- Trigger evaluation is three-valued: `Triggered` / `NotTriggered` / `Indeterminate` (instrument stale, coverage
  gap, conflicted sources). **`Indeterminate` escalates like `Triggered` for notification purposes** (DESIGN; the
  safe default) while being labelled honestly.
- **Time-of-failure forecasts.** Inverse-velocity methods (Fukuzono-type, 1/v → 0) are allowed only as
  registered, scored forecasts with explicit uncertainty, never as a deterministic "failure at 14:20". Their track
  record in the calibration ledger is shown with every forecast (§10).
- **Coupled radar and piezometer precursor (DESIGN).** Radar sees surface displacement, while effective stress
  σ′ = σ − u falls at depth first. `fmine-geotech` therefore pairs radar inverse-velocity tracking with the rate of
  rise at vibrating-wire piezometers (VWP). A pore-pressure rise beyond its TARP step (or a drop in horizontal-drain
  flow) together with positive radar acceleration raises a registered hydro-mechanical precursor alert, scored like
  any other forecast.

### 14.5 Hydrogeology, pore pressure and FoS sensitivity

- Hydrogeological units are a Strata partition. Piezometer data and pumping records condition pore-pressure fields.
- Seepage and depressurization: a confined Darcy operator is available through frankensim's `fs-conduction`
  operator-twin (bridge-gated); unconfined/Richards flow and coupled consolidation are gaps (E010) — build a
  pragmatic 2D/quasi-3D groundwater model in `fmine-geotech::pore`, or request upstream (FD-FSIM-03).
- **Pore-pressure sensitivity of FoS (DESIGN).** Each limit-equilibrium solve also emits J_u = ∂FoS_s/∂u_k for
  sector s and each piezometer or phreatic control node k. For Bishop's simplified method, with slice weight W_i,
  base inclination α_i, width b_i, strength c′_i and φ′_i, pore pressure u_i = Σ_k w_ik u_k (w_ik the
  interpolation weight from node k to slice base i), T_i = c′_i b_i + (W_i − u_i b_i) tan φ′_i and
  m_α,i = cos α_i + sin α_i tan φ′_i / F:
  ∂F/∂u_k = −[Σ_i w_ik b_i tan φ′_i / m_α,i] / [Σ_i W_i sin α_i − Σ_i T_i sin α_i tan φ′_i / (F² m_α,i²)].
  The second denominator term is the implicit dependence of m_α on F. Without it the sensitivity was 10–14% low on
  a test section, while the full form matches finite differences. Spencer and Morgenstern–Price take the same
  implicit-function step on their converged equilibrium system. Uses:
  1. piezometer deltas update a linearized ΔFoS_s = Σ_k J_u,k Δu_k between full solves, and a full re-solve runs
     when |ΔFoS_s| > 0.02;
  2. drain holes and replacement piezometers are ranked by |J_u,k| · σ_u,k, the input GB-39 needs.
- Pore-pressure realizations and J_u feed LEM (§14.2) and the TSF (§18).

### 14.6 Highwall gates and kills

| Gate | Threshold (TARGET) |
|---|---|
| GATE-HIGH-01 | LEM solvers reproduce benchmark FoS within ±0.01 on the published problem set; metamorphic test: on a single planar slip surface every method reproduces FoS = (c·A + (W cos α − U) tan φ)/(W sin α) (E393) |
| GATE-HIGH-02 | TARP automaton: 100% of synthetic trigger scenarios (including stale and conflicted instruments) produce the registered response obligations; `Indeterminate` never silently maps to `NotTriggered` |
| GATE-HIGH-03 | Kinematic P(release) forecasts registered for ≥ 1 year of bench mapping at a pilot site; reliability reported |

| Kill | Trigger | Fallback |
|---|---|---|
| KILL-HIGH-1 | Engineers reject probabilistic slope outputs | Deterministic FoS per section with sensitivity tables and flip margins; probabilities hidden behind an opt-in view |
| KILL-HIGH-2 | frankensim never gains Mohr–Coulomb/HB | LEM-only; numerical analysis stays in the engineer's existing tools, ingested as attested results |

**Guards.**

| Guard | Fires when | Action |
|---|---|---|
| G-HIGH-1 | A slope FoS or PoF is requested without pore-pressure provenance (piezometer evidence or an attested assumption) | Refuse; list the missing input |
| G-HIGH-2 | A PoF would be stated without an admissible reference class or a vetted model (§9.3) | State the UU flip margin; the probability is *not stated* |
| G-HIGH-3 | A card (Hoek–Brown, Mathews/Potvin, Laubscher, UCS(Is50)) is evaluated outside its validity domain | Refuse (out-of-box); show the domain |
| G-HIGH-4 | An inverse-velocity time-of-failure forecast is rendered without its calibration record | Refuse |
| G-HIGH-5 | A TARP trigger reads an instrument past its staleness bound | Evaluate as `Indeterminate`; escalate like `Triggered` (FMINE-REF-404) |

**Proof obligations.**

| PO | Statement | Method |
|---|---|---|
| PO-HIGH-1 | TARP evaluation is three-valued, and `Indeterminate` never maps to `NotTriggered` | Exhaustive automaton test over synthetic trigger scenarios (shared with PO-FREE-1; GATE-HIGH-02) |
| PO-HIGH-2 | LEM solvers reproduce published benchmark FoS within ±0.01 | Known answers against pinned oracle outputs (GATE-HIGH-01) |
| PO-HIGH-3 | A stale instrument is `not_observable`, never "stable" | Property test on virtual time (LabRuntime) |
| PO-HIGH-4 | `geotechnical_signoff` is false in every output | Type-level (INV-002) plus a grep-style output test |
| PO-HIGH-5 | Kinematic and time-of-failure forecasts are registered before resolution | INV-008 late-registration refusal test |
| PO-HIGH-6 | PoF is computed from the realizations, never from a fitted FoS distribution alone | Differential test: realization PoF vs normal and lognormal fits on skewed fixtures |

**GB-39 · A section that passes on FoS can fail on PoF once pore pressure is uncertain.**
- **Policy basis.** Acceptance criteria are site policy data, not FrankenMining's. The only site criteria in the
  corpus are deterministic FoS: Kibali's open-pit DAC is a function of consequence and confidence, with an
  example lower DAC of FoS 1.2 (FACT, E214); Kibali CTSF3 uses > 1.5 static and > 1.1 pseudo-static (FACT,
  E215). Neither Kibali nor Loulo states a PoF criterion. A PoF limit (Read & Stacey 2009 style) is FACT[U]. The
  card therefore runs on the site's `dac_basis`; where that is FoS-only, PoF is evidence beside it.
- **Equation.** PoF = P(FoS < 1), estimated over realizations of strength, structure and pore pressure.
  Illustrative policy with `dac_basis = FoS+PoF` (values FACT[U]): overall slope, high consequence, mean
  FoS ≥ 1.3 **and** PoF ≤ 5%.
- **Substituted.**
  - Mean FoS = 1.35 passes the FoS criterion.
  - Strength uncertainty only: sd = 0.12 → PoF ≈ Φ((1 − 1.35)/0.12) = **0.2%**.
  - Adding pore-pressure realizations conditioned on sparse piezometers: sd = 0.22 → PoF ≈ **5.6%**, which fails.
  - Two more piezometers in the section narrow sd to 0.18 → PoF ≈ **2.6%**, which passes.
- **Intuition.** The deterministic FoS hides the input that dominates the tail. The piezometers have a
  quantifiable value of information, and the card shows it next to the engineer's design acceptance.
- **Assumptions.** A normal approximation is used only for the illustration. A lognormal FoS with the same mean
  and sd gives 3.8% at sd 0.22, so the distributional form itself is a flip; that is why PO-HIGH-6 requires PoF
  from realizations.
- **Flip.** Pore-pressure sd falls (drainage holes, more piezometers) or the consequence class is moderate (PoF
  limit 10%) → passes without new instruments. Under an FoS-only policy (Kibali-style DAC 1.2) the section
  passes outright, and the 5.6% PoF is the evidence the engineer weighs.

OPEN-HIGH-1 (site TARP formats and ownership), OPEN-HIGH-2 (slope radar vendor data access), OPEN-HIGH-3 (caving
scope).

## 15. Pushback: mine planning and optimization under uncertainty

Pushback answers three questions:
- what to mine;
- when to mine it;
- where each tonne goes.

It answers them with plans that state how far from optimal they are, how much of that statement rests on a finite
scenario sample, and how wrong the inputs have historically been. Its crates are `fmine-plan` (models and
policies), `fmine-optimize` (closure and decomposition solvers, stochastic programming) and `fmine-lp` (certified
first-order LP and a small MILP). Haulage simulation lives in Headframe (`fmine-ops::haul`, §16.4).

### 15.1 Verdict (Brenner pass B3, PQ-3)

| Hypothesis | Verdict | Role in FrankenMining |
|---|---|---|
| H3a Deterministic chain: kriged model → Lerchs–Grossmann/pseudoflow pit → revenue-factor shells → pushbacks → MILP schedule at fixed cut-off → DCF with ± sensitivities | **Rejected as the decision basis** | Retained as (i) the parity baseline in every `PlanCertificate`, (ii) the shell and pushback skeleton generator, and (iii) the single-price-deck rendering the reporting codes require. VSS is undefined without it. |
| H3b Simultaneous stochastic optimization of the mining complex (COSMO-style) | **Conditional adopt** | Admitted only through the three-arm out-of-sample VSS gate (GATE-PUSH-STO). The default stochastic engine is SAA over the BZ LP relaxation with non-anticipativity by Lagrangian or progressive hedging. A metaheuristic solve with no attached bound is never published as "optimal". |
| H3c Information acquisition as a decision (EVSI, adaptive policies) | **Adopt narrowly** | `InformationOption`s on discrete commitments: pushback start, plant expansion, cave footprint, TSF raise (§22). **Whole-mine POMDP rejected for v1**: published successes are 2D, synthetic and noiseless, and wrongly abandoned 32% of multi-body cases (FACT, R7 §1.10). |
| H3d Hierarchical decomposition with certified gaps at every level (forced alternative) | **Adopt as the architecture** | Strategic, tactical and short-term layers pass targets and prices down and attainment up. Every plan carries the three-gap certificate. Algorithmic novelty: checked null. The novelty is the certificate discipline. |
| H3e Policy search over a frozen geometric skeleton (second forced alternative) | **Adopt** | The tactical cut-off, stockpile-reclaim, blend-target and pushback-trigger policy layer. It is legible to operators and costs a fraction of COSMO. |

### 15.2 The three-gap ledger and the `PlanCertificate`

The decomposition is the single most important honesty device in planning:

```text
reported_value − true_value = (z_model − z*_model)       optimization gap   (certified by a verified dual bound)
                            + (z*_model − z*_population)  sampling gap       (SAA statistical confidence interval)
                            + (z*_population − z_truth)   calibration gap    (synthetic truth, then reconciliation history)
```

**The third term is often the largest in mining** (FACT inputs, each a single source):
- a Localised Uniform Conditioning case with slope of regression ≈ 0.1 against grade control (FACT[U], page not
  pinned in the corpus; E029). The mechanism is b ≈ ρ for any variance-restoring estimator (GB-11; E355);
- only 21% of 412 interpreters identified the correct tectonic setting of one synthetic seismic image in Bond et al. 2007 (FACT, E639);
- conditional simulations rarely validated against outcomes (R7 §11.5).

So a "0.1% from optimal" headline on a miscalibrated model is a false-certainty number. FrankenMining renders it
with colour `Estimated` and an "uncalibrated" banner (NL-1…NL-8, §7.3).

```rust
pub struct PlanCertificate {                       // attached to every published plan, at every horizon
    pub plan: PlanId, pub model: ModelDigest, pub opt_set: RealizationSetId,
    pub primal: Money, pub bound: Option<CertifiedBound>, pub opt_gap: Option<Ratio>,
    pub saa: Option<SaaGap>, pub vss: Option<VssCertificate>, pub execution: Option<ComplianceAdjusted>,
    pub calibration: CalibrationStatus,            // Calibrated{ledger_ref} | Uncalibrated | Miscalibrated{correction}
    pub color: Color, pub refusals: Vec<Refusal>,
}
pub struct CertifiedBound { pub value: Interval<Money>, pub kind: BoundKind, pub witness: ContentHash, pub verified_by: VerifierId }
pub enum BoundKind { ClosureExact, BzLpRelaxation, Lagrangian { iters: u32 }, PdlpDualRepair, SaaStatistical { conf: f64, batches: u16 } }
pub struct VssCertificate {                        // the constructor refuses eval_set == opt_set or unpaired draws
    pub det_plan: PlanId, pub sto_plan: PlanId, pub eval_set: RealizationSetId, pub n_eval: u32,
    pub vss_oos: Interval<Money>, pub vss_in_sample: Money, pub optimism: Money, pub truth_vss: Option<Interval<Money>>,
}
pub struct ShadowPrice { pub constraint: ConstraintId, pub units: UnitExpr, pub dual: f64, pub valid_range: (f64, f64) }
```

**What profit means here (DESIGN).** "Maximize profit" means maximizing E[discounted free cash flow], and at
enterprise level E[ΔNAV per share] (§23). It does not mean accounting profit. Accounting profit sits downstream of
DD&A, stripping-cost capitalization (IFRIC 20) and impairment policy (E021), which FA renders (§21).

### 15.3 Hierarchy, horizons and timescale separation

| Level | Cadence | Decision content | Methods | Passes down | Reports up |
|---|---|---|---|---|---|
| Strategic (life of mine) | Annual, or on trigger | Ultimate pit, pushbacks, underground envelope and mining method, LOM schedule by phase-bench, plant capacity and expansions, cut-off policy seed | Pseudoflow UPIT and parametric shells; BZ LP relaxation of PCPSP; Lagrangian bounds; TopoSort plus local search; SAA when gated | Phase/bench tonnage and metal targets; dual prices ($/t mill hour, $/t blend tolerance, $/t stockpile capacity) | Plan certificate; reserve rendering at the disclosure deck (§24) |
| Tactical (budget, quarter, month) | Quarterly/monthly, plus hysteresis triggers | Sequence within the strategic envelope; stockpile policy; blend targets; equipment and crew allocation | SAA over the realization ensemble; H3e policy search; blend LP | Monthly faces, blend bands, stockpile thresholds | Attainment, compliance, F-factor posteriors |
| Short-term (week, shift) | Weekly and per shift | Dig sequence, dig lines, blend targets, dispatch targets (advisory) | Receding-horizon MPC (LP/QP over 1–2 weeks); expected-loss dig lines (§15.7) | Recommended targets as prepared effects (never actuated, §31.3) | Shift actuals, reconciliation residuals |

**Separation rules (DESIGN):**
- Cadences differ by at least 12× (strategic vs tactical) and 4× (tactical vs short-term). The MPC runs per
  shift with a 1–2 week prediction horizon.
- The tactical plan is re-solved only when a trigger fires:
  - an `eproc` alarm on compliance or reconciliation drift;
  - the price leaves its declared band;
  - a geotechnical, plant or tailings event occurs.
- **Plan churn is tracked as a KPI** (HYPOTHESIS: churn is itself a cause of low schedule compliance).
  Re-planning on noise is a measurable defect.

### 15.4 Solver stack: build in pure Rust, with lab oracles only

**Structural fact (HYPOTHESIS, high confidence).** Strategic scheduling is mostly maximum-closure plus a few
coupling rows per period. Bienstock–Zuckerberg (BZ) and Lagrangian relaxation both reduce it to closure
subproblems. So the hard kernel is one pseudoflow implementation, which also serves the ultimate pit and nested
shells. The BZ master LP is small and dense, which frankenscipy's in-universe simplex/IPM handles (FACT, B3 V2:
`fsci-opt/src/linprog.rs:83–120` names its methods `Highs*` but runs its own revised simplex and Mehrotra IPM; there
is no HiGHS dependency).

| Module (B3 name) | What | Algorithm | Scale TARGET | Lab oracle |
|---|---|---|---|---|
| `fmine-optimize::pitflow` (`fm-pitflow`) | Ultimate pit, parametric nested shells, closure oracle | Hochbaum pseudoflow on i64 CSR. Overflow guard: Σ\|vᵢ\| < 2⁶², else refuse. **Implicit precedence arcs** from slope templates (azimuth sector × geotechnical domain from §14.2). Parametric λ warm-started across revenue factors. Returns the cut **and** the flow, so closure is independently checkable | MineLib UPIT exact. 10⁷ blocks / ~2×10⁸ implicit arcs ≤ 120 s, ≤ 32 GB | MineFlow (MIT C++); fsci-sparse `maximum_flow` on small int32-scaled instances (it refuses capacities outside int32, FACT B3 V3); fnx `preflow_push` |
| `fmine-optimize::bz` (`fm-bz`) | PCPSP / multi-period LP relaxations | BZ partitions; master on fsci-opt `linprog`; subproblem on `pitflow` | MineLib PCPSP LP within 1e-6 relative of the oracle; 10⁶ blocks × 20 periods ≤ 30 min | HiGHS |
| `fmine-optimize::lag` (`fm-lag`) | Lagrangian or proximal bundle relaxation of blend and multi-processor rows | Every iterate is a valid dual bound | Always-valid bound; gap ≤ 5% | HiGHS |
| `fmine-optimize::toposort` (`fm-topo`) | Feasible incumbents from LP solutions | TopoSort (Chicoisne et al. 2012) plus local search and repair; checked in `fmine-verify` | Gap ≤ 3% on ≥ 80% of MineLib PCPSP instances, ≤ 5% on all | MineLib best-known values |
| `fmine-lp::pdlp` (`fm-pdlp`) | Large sparse non-closure LPs: multi-site blend networks, SAA extensive forms, CVaR LPs | Restarted PDHG with diagonal preconditioning. **Certified bound** by dual repair plus outward interval rounding, copying the `fs-truss` PDHG certificate *pattern* (FACT, B3 V4; that crate depends on fs-exec and fs-solid, so it is not lifted) | 10⁷ nonzeros to 1e-4 relative gap ≤ 30 min, with bound | HiGHS IPM; OR-Tools PDLP |
| `fmine-lp::milp` (`fm-milp`) | **Small** MILP: blend with on/off, fleet assignment, pushback selection, capital programme (§23) | Sparse bounded dual simplex with warm start, bound-tightening presolve, best-bound plus diving; cuts only if a gate demands them; MPS I/O | ≤ 10⁴ variables, ≤ 500 binaries, ≤ 1% gap ≤ 10 min | HiGHS, CBC; fsci-opt `milp` (dense rows, 2,048-node default, FACT B3 V1) |
| `fmine-optimize::stoch` (`fm-stoch`) | SAA, Kantorovich scenario reduction, Mak–Morton–Wood bounds, VSS_oos, progressive hedging | Common random numbers across candidates; disjoint optimization and evaluation sets enforced by type | S ≤ 200 in optimization; ≥ 10³ in evaluation | Brute force on toys |
| `fmine-plan::sched` (`fm-sched`) | Strategic and tactical scheduling models (open pit and underground) | Model builders over the solvers above | — | — |
| `fmine-plan::blend` (`fm-blend`) | Blend and destination LP with grade-binned stockpiles (§15.6) | — | HiGHS parity 1e-7 | HiGHS |
| `fmine-plan::cutoff` (`fm-cutoff`) | Lane cut-off seed plus H3e policy search | CMA-ES lifted with provenance (fs-dfo depends on fs-exec, so the algorithm is lifted rather than the crate, FACT B3 V6) | — | — |
| `fmine-plan::mpc` (`fm-mpc`) | Shift/week receding horizon | LP/QP; infeasibility fallback is the last feasible plan plus slack | ≤ 5 s per solve | HiGHS |
| `fmine-plan::compliance` (`fm-compliance`) | Schedule-compliance forecasts (§15.8) | Beta-binomial hierarchy plus `eproc` | — | — |

**NONGOAL-006 restated.** FrankenMining does not build a general commercial MILP. The `fmine-lp::milp` scale
ceiling is a design limit, not a temporary one.

**Lab oracles (DESIGN).**
- HiGHS (MIT), CBC (EPL), MineFlow (MIT) and OR-Tools PDLP (Apache) live only in `fmine-oracles`. Gurobi and
  CPLEX are used only where a client's lab licenses them.
- Pseudoflow is the production algorithm at Carlin (2024) and Turquoise Ridge (2023), for pit shells and for
  underground stope net-value selection; Cortez (2021) still used Whittle Lerchs–Grossmann (FACT, E186). The
  parity oracle therefore covers both LG and pseudoflow, because the algorithm changes between filings.
- Every oracle is version-pinned, and its outputs are frozen as fixtures that are **never regenerated from the
  candidate**.
- No oracle appears in the runtime or CI dependency graph. Fixtures are produced by a separate lab runner (§33.2).

**`UntrustedCandidate` import (DESIGN; OPEN-PUSH-1 for owner sign-off).**
- FrankenMining exports MPS plus a certificate request.
- Any external solution, such as one from a client's Whittle, Deswik, COMET or Gurobi, can come back as an
  `UntrustedCandidate`.
- `fmine-verify` (std-only) checks feasibility and recomputes the objective independently. The candidate may
  become the incumbent.
- Its claimed bound is ignored unless a dual witness verifies.
- **Why this matters:** it lets FrankenMining sit beside incumbent planning software (NONGOAL-001) and *certify*
  its plans, rather than compete with it. The verifier stays the authority, which keeps "one authority per number".

### 15.5 Open pit, underground and transition specifics

| Topic | Design | Status |
|---|---|---|
| Slope templates | Precedence arcs generated from Highwall sector angles per geotechnical domain and azimuth (§14.2). Changing a slope angle is a new geotech claim, and it re-stales the shells. | DESIGN |
| Pit and ramp design | Designed pits are engineers' acts in their CAD tools. FrankenMining ingests the designed pit as an L5 `DesignSolid` and computes the **shell-to-design loss** (tonnes, metal, value) with uncertainty, plus ramp-induced slope flattening. No CAD is built (NONGOAL-001). | DESIGN |
| Stope optimization | `fmine-plan::stope`: floating-stope / mineable-shape optimization over realizations. Dilution comes from the ELOS card (§14.3) and minimum mining width from the method. The output is P(stope economic) and expected value per candidate shape. These heuristics have no bound, so they are labelled `heuristic` (guard G-PUSH-3), and solutions from MSO-class client tools import as `UntrustedCandidate`s. | DESIGN; heuristic |
| Underground scheduling | An activity network covers development metres, stope cycles (drill, blast, muck, backfill pour and cure), and ventilation and services prerequisites. The critical path and max-cycle-mean bounds come from max-plus algebra (lifted `fs-tropical`, FACT E003). Resource-constrained scheduling uses `fmine-lp::milp` up to ≤ 10⁴ variables, and priority-rule heuristics with a reported bound beyond that. | DESIGN |
| Open pit → underground transition | The transition depth and crown pillar are a `CapitalCandidate` (§23) over the shared scenario index, not a fixed design input. | DESIGN |
| Caving (block/panel) | Footprint selection, draw-point scheduling and dilution entry are Phase 2 and research-grade. Laubscher-style empirical cards only (OPEN-HIGH-3). | OPEN |
| Waste dumps and PAG | Dump destinations sit in the destination LP with an encapsulation constraint: NAF cover tonnes ≥ r × PAG tonnes per cell and period, with r as policy data. If a block's ARD class is `unknown`, no NAF-dump destination is offered (FMINE-REF-402, §18.5). | DESIGN |
| Dilution attributes | Dilution material carries its deleterious attributes, not only zero grade: Turquoise Ridge underground dilution carries in-situ TCM at 0 g/t Au, which raises TCM/Au and lowers recovery (FACT, E187). Residual dilution and ore loss are attributed to four registered sources: internal, planned, accidental (→ Highwall, Headframe) and geologic (→ Strata) (E399). | DESIGN |
| Haul distances | Taken from the haul-road network geometry per period (Headframe §16.4). They change with pit depth and dump fill, so haulage cost is a function of the schedule, not a constant. | DESIGN |

### 15.6 Optimal ore mix at best unit cost

This answers GOAL-011. It is the core of "what should we feed the mill, and from where?"

**Blending and destination LP (DESIGN).**

Indices:
- s: a source or parcel (a block, a bench-phase, or a stockpile grade bin);
- p: a destination (CIL, flotation, heap leach, POX, waste dump cell, stockpile bin k);
- t: a period.

```text
max Σ_t δ_t [ Σ_{s,p} x_spt (NSR_sp − c^proc_p − c^haul_sp(t)) − Σ_s c^mine_s m_st − f_t ]
s.t.  Σ_s x_spt ≤ Cap_p,t                                (tonnes)
      Σ_s x_spt · E_s ≤ P_p · H_p,t                      (energy: E_s = specific energy kWh/t from the hardness proxy, §13.4)
      Σ_s x_spt (a_s,e − A^max_p,e) ≤ 0                   (deleterious e: As, Hg, Sb, Bi, F, Cl, CN-soluble Cu, organic C, PRI)
      Σ_p x_spt = m_st ;  precedence/closure on m        (envelope from pitflow / bz)
      I_k,t+1 = I_k,t + inflow_k,t − reclaim_k,t ; 0 ≤ I ≤ Î_k     (grade-binned stockpiles)
      NAF_cover_t ≥ r · PAG_t per dump cell               (§15.5, §18.5)
NSR_sp = Σ_metal g_s,m · R_p,m(domain_s) · payable_p,m · (price_m − TC/RC/refining_m) − penalties_p(s) − royalty_m(price)
```

**Design choices, each with its reason:**
1. **Recovery is per source geometallurgical domain, not per blend.** This keeps recovered metal linear.
   Blend-dependent recovery uses piecewise-linear domain-mix classes. Recovery applies at a declared processing
   support, a shift or day blend (§17.3).
2. **Hardness enters as specific energy under mass-weighted additivity.** Hardness indices are non-additive
   (§13.4), so this is **HYPOTHESIS H-ADD**: kWh/t is additive within ±8% over the operating blend range. It is
   validated on plant data. If falsified, the model uses piecewise blend classes.
3. **Stockpiles use fixed grade bins.** Exact mixing is bilinear and non-convex.
   - Certificate: a McCormick relaxation of the bilinear model gives a valid upper bound.
   - TARGET: binned-LP vs McCormick gap ≤ 1%.
4. **The throughput–recovery trade-off is explicit.**
   - Grind modes m (fine, medium, coarse) carry pairs (Q_m t/h, ΔR_m), and the LP picks a convex combination per
     period.
   - The dual on mill hours says whether to "push tonnes". Tin needs the coupled comminution model because its
     recovery is strongly size-dependent and fines are lost (§17.3).
5. **All payable metals are in the NSR.** By-product vs co-product treatment is a *presentation* layer for
   AISC and C1 (§21.6) and never changes the optimization.
6. **Royalties and price-linked costs are functions of the same price draw as revenue.** The +16% industry AISC
   rise in Q1 2026 was driven by royalties and corporate G&A (FACT, R6 §4.2).
7. **Blend windows have two registered forms, and refractory circuits add two-sided windows and campaign modes
   (FACT plus DESIGN).** A window can enter pit optimisation as linearised per-block penalties (Turquoise Ridge: an
   acid cost when CO3 > 4.3% and a credit when CO3 < 3.7%, mirroring the 3.5–4.3% feed window) while hard annual
   caps apply only in scheduling (FACT, E180). Roasters and autoclaves need two-sided windows: too little sulphide
   sulphur starves a roaster of heat, and too much sulphide or carbon overloads it. For refractory circuit p,
   element e and period t, with campaign mode y_{p,t}:
   L_{p,e}(y_{p,t}) · Σ_s x_{spt} ≤ Σ_s a_{s,e} x_{spt} ≤ U_{p,e}(y_{p,t}) · Σ_s x_{spt}.
   - Goldstrike roaster (Carlin TR Table 13-7): SS 0.8–1.8% (optimum 1.6), CO3 > 14%, TCM 0.8–1.2% (0.95),
     As < 1,800 ppm (optimum < 1,200). Gold Quarry roaster (Table 13-8): SS 1.4–2.5% (2.1), CO3 10–18% (12),
     TCM 0.4–0.75% (0.6), As < 1,800 ppm, Hg < 25 ppm. Units are weight percent and ppm (FACT, E172).
   - Goldstrike autoclave: alkaline mode needs CO3 > 8% (Table 13-9); acid mode needs SS < 2.8% and CO3 < 8%
     (Table 13-10); the preg-rob cap depends on the source (E173). Both modes use the same piecewise cubic in grade
     (opt), with acid recovery about 8.09 points higher (Tables 13-13 and 13-14; FACT, E659). A mode switch costs
     c_switch·|y_t − y_{t−1}| (DESIGN).
   - Four Corners ore at the roaster (Table 13-16): recovery = (g − T)/g with T = 1.821×10⁻⁵·As + 0.127·g −
     4.890×10⁻⁴, g in opt and As in ppm (FACT, E661; the printed formula has an extra parenthesis, E171). Recovered
     grade 0.873·g − 1.821×10⁻⁵·As + 4.890×10⁻⁴ is affine in (g, As), so it enters the LP exactly as a per-block
     coefficient, within the fitted range, instead of as an arsenic penalty.
   - **Curvature correction (DESIGN).** Recovered metal per tonne is f(g) = g·R(g), so
     E[f(g)] − f(E[g]) ≈ ½ f″(ĝ) σ²_g. The sign follows f″, not the shape of R. For the Goldstrike roaster curve
     R = 92.03 − 37.36·exp(−12.94·g) (% with g in opt; Table 13-12; FACT, E660), f is convex below about 5.3 g/t
     and concave above it. Applying R to block-average grades therefore understates recovered metal for low-grade
     blocks and overstates it for high-grade ones; a curve of the form a − b/g makes f linear and unbiased.
     `fmine-plan` evaluates f per realization, or applies the second-order term when only moments exist.
8. **Two process routes, priced separately (FACT plus DESIGN).** Pueblo Viejo can send ore to direct POX or to
   flotation then POX. The TR's pit optimisation assumed the costlier flotation + POX route for every block, a
   "conservative" case, because short-term plant constraints decide routing (FACT, E265). Route recoveries are in
   TR §13.3 (Tables 13-4 to 13-6); only flotation + POX costs are published. `fmine-plan` carries route-split
   variables x_{s,direct,t} and x_{s,float,t} under the autoclave's sulphur capacity,
   Σ_s S_s (x_{s,direct,t} + η_S x_{s,float,t}) ≤ Cap_S,t, with η_S the sulphur reporting to concentrate.
   - Flotation + POX variable cost (Table 15-4; FACT, E664): plant −1911.2S⁴ + 857.27S³ − 128.24S² + 11.236S +
     6.8302 US$/t and power (794.9833S + 22.3323) kWh/t × P, where S is total sulphur as a fraction (9.04% →
     0.0904, never 9.04; G-UNIT-4) and P = US$0.07912/kWh. At S = 0.0904 they give 7.30 + 7.45 = 14.76 US$/t, the
     Table 15-8 value.
   - Waste costs more to mine than ore (3.68 vs 3.20 US$/t), so the incremental ore mining cost is −0.48 US$/t
     (Table 15-2): waste goes to a temporary dump and is rehandled, or is co-disposed in the TSF. TSF sustaining
     capital is 5.96 US$/t for ore and 2.25 for waste (Table 15-5; FACT, E665).
9. **Inter-complex haulage and tolling (DESIGN).** In Nevada, Goldrush ore goes to the Goldstrike and Gold Quarry
   roasters, Cortez refractory ore is trucked about 115 km to Carlin, and Turquoise Ridge ore goes to the Sage
   autoclave (FACT, E663). The destination index p therefore spans complexes, with haulage cost c^haul_{s→p}(t).
   Where ownership differs across an arc, a shadow toll τ_{s→p} applies: a 100% Barrick source milled in the 61.5%
   NGM JV, as Fourmile was until its vend-in (E658; §23.2).
10. **Reagent supply can be a mined stream.** Pueblo Viejo's LOM quarries 321,561 kt of limestone against
    217,649 kt of pit ore (FACT, E264); it is scheduled here with its own cost and quality model.
11. **TSF capacity is a plan constraint.** Kibali's FTSF limits oxide feed until 2028, and a delayed Loulo TSF
    expansion forced over-height deposition behind a sacrificial buttress (FACT, E218, E219); Goldstrike's storage
    lasts "through 2028" while a 180 Mm³ TSF is permitted (FACT, E190). Freeboard (§18) supplies the capacity
    runway and its permitting lead time as a constraint.

**Lane's cut-off theory as policy seed and explanation (DESIGN plus FACT).**
- **The cut-off grades:**
  - Mine-limited: g_m = h / ((p−k)y).
  - Mill-limited: g_h = (h + (f+F)/H) / ((p−k)y), with opportunity cost F = δ·V.
  - Market-limited: g_k = h / ((p − k − (f+F)/K)·y).
  - Balancing cut-offs come from the grade–tonnage curve. The operative cut-off is the median of the three for
    each capacity pair, iterated on V. This gives a declining cut-off policy (Lane 1964/1988, FACT R7 §5.2).
  - When recovery depends on grade (or on TCM/Au), the cut-off is a fixed point g = cost/(net·R(g)).
    Turquoise Ridge's 5.66 g/t underground cut-off reproduces only as that fixed point (g = 5.665, R = 79.6%;
    INFERENCE, E182).
  - **Published cut-off reproduction** is an M0 check (seven cut-offs reproduced; E182–E184). It catches the
    $/t vs $/st trap: Cortez cut-offs reproduce only with ×0.90718, and with the AB495 tax omitted (E183).
- **Extensions:**
  - Material between the marginal and opportunity-cost cut-offs goes to a stockpile when its discounted
    end-of-life value exceeds the rehandle cost.
  - Under grade uncertainty, classification uses expected loss over the local posterior, not the kriged grade
    versus cut-off.
- **Consistency check.** Lane is the closed-form seed and explanation card, H3e policy search refines it, and the
  LP is the arbiter. When mine life is endogenous (so time-based fixed cost f and opportunity cost F respond to
  the plan), the LP's mill-hour dual equals Lane's (f+F)/H term, and PO-PUSH-15 checks this. In a fixed-horizon
  period LP, f is sunk and the dual reflects only the opportunity cost.

**Unit cost consistent with the accounting truth.**
- One `aisc(rules_version, inputs)` function computes both plan AISC (from plan quantities × `CostDriverCard`
  rates) and actual AISC (from FA-certified rows). The full specification is in §21.6.
- **AISC/oz is never the objective.** It is a ratio, and high-grading, deferring sustaining capital or
  reclassifying capital as growth can all lower it while destroying NPV.
- AISC may enter as a linear constraint, Σ cost_t − A·oz_t ≤ 0. Its **shadow price is always published**, as
  "$ of NPV per $/oz of AISC target" (§23.6).

### 15.7 Grade control: dig lines under uncertainty

- **The information.** Grade-control data (blastholes or RC grade-control holes) enters Strata as observations
  (§13.7). The short-term model is a projection of the belief at T_gc, at SMU support, with realizations.
- **The decision rule (DESIGN).** Each material polygon's destination minimizes **expected loss** over the
  posterior: E[loss(d, Z)] for d ∈ {mill, leach, stockpile bins, waste}, where the loss matrix comes from the
  destination economics of §15.6. This is the Verly/Isaaks expected-loss approach (FACT, R7 §6.2), and it
  replaces "estimated grade ≥ cut-off".
- **Blast movement.** Dig lines are shifted by measured or modelled blast movement (§16.3) *before*
  classification. Unshifted dig lines are a principal source of ore loss and dilution (HYPOTHESIS; scored at
  the pilot).
- **Minimum mining width and digability.** Polygons respect equipment selectivity (bucket width, bench height),
  through a morphological constraint on the classified grid.
- **Several live hypotheses** (OPEN-STRATA-5). When L1 carries several interpretations with material weight,
  expected loss is taken over the hypothesis mixture. The DecisionCard shows the destination flip under each
  hypothesis.
- **Output.** Dig lines are *recommendations* (`EFFECT-DIGLINE` prepared effects). The grade-control geologist
  signs them, and they are never pushed to the FMS automatically.

### 15.8 Schedule compliance as a forecastable, calibrated quantity

**Definitions:**
- Spatial compliance: SC_t = (tonnes mined in t from blocks scheduled in t) / (tonnes scheduled in t).
- Also tracked: sequence compliance, volume compliance, metal compliance, and **sequence debt** (scheduled but
  unmined tonnes carried forward, including stripping debt).

**Model.**
- k_t ~ BetaBinomial(n_t, α_site, β_site), with a hierarchy over site, pit and horizon.
- Covariates: fleet availability, plan churn, geotechnical events, blast delays, weather, number of active faces.
- The prior is *wide*. The industry figure "as low as 30%" (E026) is a lower anecdote, not a base rate
  (OPEN-PUSH-2).
- Each period's forecast is registered (§10) and scored by PIT and coverage, with an `eproc` drift monitor.

**Use in planning.**
- Plans are scored under a **compliance kernel K(plan, state)**, which draws realized extraction with
  substitution to available adjacent blocks at a grade ratio ρ.
- A plan that wins only without K is flagged "brittle".
- Executability features are rewarded: active faces and precedence slack.

### 15.9 Guards, monitors and proof obligations

| Guard (evaluated in order; first match wins) | Condition | Action |
|---|---|---|
| G-PUSH-1 | A safety constraint would be priced, or any INV-002 flag (MNPI, safety or sign-off semantics) would be set | Refuse and route to a human. No card is emitted. |
| G-PUSH-2 | Bound not verified by `fmine-verify` | No gap is shown; label "heuristic, unbounded" |
| G-PUSH-3 | Optimization gap > 5% | Label "heuristic plan"; the word "optimal" is banned from the render |
| G-PUSH-4 | VSS computed with eval_set = opt_set, unpaired draws, or n_eval < 200 | Refuse the VSS claim |
| G-PUSH-5 | VSS_oos CI includes 0 | The deterministic baseline is the default plan; the stochastic plan is advisory |
| G-PUSH-6 | Calibration ≠ Calibrated | Colour ≤ Estimated; refuse the P(target) headline; widen by reference class |
| G-PUSH-7 | AISC forecast from drivers lacking FA certificates | Watermark "plan-only metric" |
| G-PUSH-8 | A top-3 sensitivity input has evidence class `Assumed` | Banner: "decision rests on assumption X" |
| G-PUSH-9 | A block's ARD class is unknown and a NAF destination is requested | FMINE-REF-402 |
| G-PUSH-10 | Schedule references slope angles from a superseded geotech claim | Mark the plan `Stale`; block its use in disclosure |

**Monitors:**
- M-PUSH-1: ensemble PIT/coverage, on synthetic truth first, then on reconciliation.
- M-PUSH-2: schedule-compliance e-process.
- M-PUSH-3: F-factor drift e-process (shared with §20).
- M-PUSH-4: price-band re-plan trigger.
- M-PUSH-5: plan churn.
- M-PUSH-6: CI solver regression on MineLib gaps and times against pinned fixtures.

| PO | Statement | Verification |
|---|---|---|
| PO-PUSH-1 | The returned pit is a closure (feasible) | O(arcs) independent check in `fmine-verify` |
| PO-PUSH-2 | Max-flow equals min-cut, flow is conserved, no i64 overflow | Pre-solve Σ\|v\| < 2⁶² guard plus flow check |
| PO-PUSH-3 | Nested shells are monotone: λ₁ ≤ λ₂ ⇒ pit(λ₁) ⊆ pit(λ₂) when block values are monotone in λ | Property test |
| PO-PUSH-4 | Every dual bound is valid | Dual feasibility verified with outward interval rounding, then weak duality |
| PO-PUSH-5 | The schedule is feasible: precedence across periods, capacities | Independent checker |
| PO-PUSH-6 | SAA bound CIs achieve nominal coverage | Toys with known optimum |
| PO-PUSH-7 | VSS sets are disjoint and paired | Type-enforced |
| PO-PUSH-13 | Same inputs and seeds ⇒ same plan digest | Replay test across LabRuntime seeds |
| PO-PUSH-14 | Metamorphic laws: price ↑ ⇒ pit ⊇ (for positive-metal blocks); capacity ↑ ⇒ LP value non-decreasing; a duplicated scenario leaves the SAA solution invariant; rescaling money units leaves the plan unchanged | Metamorphic suite |
| PO-PUSH-15 | Lane–LP consistency: in a single-mill toy with endogenous mine life, the mill-hour dual equals (f+F)/H | Known-answer test |

PO-PUSH-8 to PO-PUSH-12 are capital and metric obligations. They are listed under §21.6 and §23.

### 15.10 Galaxy-brain cards (illustrative values)

**GB-13 · Lane's cut-off with a stockpile option.**
- **Equation.** g_m = h/((p−k)y); g_h = (h + (f+F)/H)/((p−k)y); F = δ·V.
- **Substituted inputs.**
  - Price p = $3,000/oz = $96.45/g; k = $6/oz; y = 0.90, so (p−k)y = $86.63/g.
  - h = $22/t; f = $60M/y; H = 10 Mt/y.
  - δ = 8% and V = $2.0B give F = $160M/y.
- **Cut-offs.**
  - g_m = 22/86.63 = 0.254 g/t.
  - g_h = (22 + 220/10)/86.63 = **0.508 g/t**.
  - Late in life, at V = $0.5B: g_h = (22 + 10)/86.63 = 0.369 g/t.
- **A 0.40 g/t parcel stockpiled for 12 years** is worth (0.40·86.63 − 22 − 1.8)·1.08⁻¹² − 0.6 = **+$3.7/t**.
  It is stockpiled, not wasted.
- **Intuition.** Early in mine life, the opportunity cost of time doubles the mill cut-off. The 0.25–0.51 g/t
  band is option inventory, not waste.
- **Assumptions.** The mill binds (otherwise the balancing cut-off applies); V is iterated from the plan; grades
  are posterior means, with expected-loss classes applied under uncertainty.
- **Flip.** A stockpile recovery loss ≥ 27% (from oxidation) or a price fall of 27% makes the parcel value ≤ its
  placement cost, and it becomes waste.

**GB-14 · Out-of-sample value of the stochastic solution (three-arm, synthetic-truth mine).**
- **Equation.** VSS_oos = E_B[NPV(x_sto)] − E_B[NPV(x_det)], where B is disjoint from the optimization set A and
  draws are paired; SE = sd(diff)/√n_B.
- **Calibrated arm.**
  - VSS_oos = 1,905 − 1,812 = **$93M (5.1%)**. With sd 110 and n_B = 200, SE = 7.8, so the 95% CI is [78, 108].
  - In-sample on A it was $140M, so the optimizer's optimism is $47M.
  - Against 30 independent truths, VSS is $71M with CI [21, 121].
- **Under-dispersed arm** (σ × 0.6): VSS_oos = $12M, CI [−5, 29]. Stochastic value is **not** demonstrated.
- **Intuition.** A third of the in-sample "stochastic value" is optimizer's curse. The rest survives only with
  calibrated realizations.
- **Assumptions.** The generator is representative; S_A = 50; the compliance kernel is off (turned on, VSS_oos
  falls to $58M).
- **Flip.** The truth-arm CI includes 0, or M-PUSH-1 fails. Either demotes the stochastic plan to advisory
  (G-PUSH-5/6).

**GB-15 · The three-gap ledger on one plan.**
- **Optimization gap.** BZ LP 2,140.0 has a verified bound UB* = 2,140.4. TopoSort plus local search reaches
  2,091.0, so gap_opt = 49.4/2,140.4 = **2.31%**.
- **Sampling gap.** UB_SAA = 1,948 ± 18 (10 batches) and LB_eval = 1,905 ± 8. The sampling gap is 43 ± 20, or
  2.2% [1.2, 3.3].
- **Calibration gap.** Nominal 90% NPV intervals covered 24 of 30 truths. P(X ≤ 24 | Bin(30, 0.9)) = 0.073, so the
  status is **amber**.
- **Intuition.** "2.3% from optimal" is a statement about the model. The model is amber-calibrated, so the
  headline stays `Estimated`.
- **Flip.**
  - Coverage ≤ 22/30 (p < 0.01) → `Miscalibrated` → widen.
  - gap_opt > 5% → "heuristic plan".

**GB-16 · The plan number is not a forecast (schedule compliance).**
- **Equation.** c ~ beta-binomial posterior predictive; metal = plan·(c + (1−c)·ρ).
- **Substituted inputs.**
  - Over 12 months, 2,520 of 4,800 planned blocks were mined as planned (0.525), with a between-month sd of 0.07.
  - So next month's c is 0.53, with 80% PI [0.44, 0.62].
- **Forecast.** A plan of 95 koz with substitution grade ratio ρ = 0.82 gives 95·(0.53 + 0.47·0.82) = **87.0 koz**
  [85.4, 88.5]. Past 80% PIs covered 9 of 12 months.
- **Intuition.** Execution alone produces a production variance of about 8 koz per month against plan (≈ $36M of
  revenue timing at $4,500/oz) before any grade uncertainty. Unmined planned blocks are deferred (sequence debt),
  not lost; their value at stake is the NPV of the deferral plus any sterilization, shown separately.
- **Flip.** If ρ → 1 (substitutes are as rich), the metal gap vanishes but sequence debt grows. An M-PUSH-2 alarm
  resets the prior window.

### 15.11 Assumptions, gates, kills and open questions

| ID | Assumption | Measured by | If false |
|---|---|---|---|
| A-PUSH-1 | Ensembles are calibrated at decision support (block, period) | M-PUSH-1, coverage tests | G-PUSH-6; KILL-PUSH-CAL |
| A-PUSH-2 | kWh/t is additive over blends within ±8% (H-ADD) | Plant data | Piecewise blend classes |
| A-PUSH-3 | There are few coupling rows per period (BZ-friendly) | KILL-PUSH-BZ | `lag` plus aggregation |
| A-PUSH-4 | The compliance kernel is stationary within a quarter | M-PUSH-2 | Shorter windows; wider intervals |
| A-PUSH-5 | Recovery is per source domain, not per blend | Plant reconciliation (§20) | Blend-class recovery |

| Gate / kill | Threshold (TARGET) | When (months after Pushback start at M4 = month 18; all inside month 36) | Deterministic fallback if killed |
|---|---|---|---|
| GATE-PUSH-PIT1 | MineLib UPIT exact (integer-scaled) with verified closure | +2 months | — (must pass) |
| ✖ KILL-PUSH-PIT | `pitflow` within 5× MineFlow wall time on the largest MineLib UPIT, and 10⁷ blocks ≤ 10 min | +4 months | Bench/panel super-block aggregation, re-verified at fine scale inside the shell. **Never** a runtime foreign dependency. |
| ✖ KILL-PUSH-BZ | PCPSP LP within 1% of the oracle in ≤ 1 h on side-constrained instances | +6 months | `lag` bound plus rolling-horizon aggregation; publish the gap |
| GATE-PUSH-TOPO | Gap ≤ 3% on ≥ 80% of PCPSP instances, ≤ 5% on all | +6 months | Report the honest gap |
| ✖ KILL-PUSH-MILP | Blend, fleet and capital subproblems to ≤ 1% within SLA | +8 months | LP plus rounding with a reported gap; MPS export to the lab |
| ✖ KILL-PUSH-PDLP | 1e-4 relative gap on real-size non-closure LPs | +9 months | BZ/Lagrangian where structure allows; otherwise scenario reduction to sizes fsci handles |
| ✖ GATE-PUSH-STO (K-STO) | Calibrated-arm VSS_oos CI excludes 0 **and** the miscalibrated arms trip M-PUSH-1 before publication | +10 months | Drop COSMO-scale (b). Keep (a) plus robust overlay plus (e) policy search. Use stochastic methods only for stockpile and cut-off policy. |
| GATE-PUSH-BLEND | HiGHS parity 1e-7; bins vs McCormick ≤ 1% | +6 months | Piecewise classes |
| GATE-PUSH-COMP | ≥ 24-month hindcast; 80% PI coverage in [0.70, 0.90] | Pilot + 6 months | Wider prior; label uncalibrated |
| KILL-PUSH-CAL | Ensemble coverage demonstrated on synthetic truth, then on reconciliation | Continuous | Stochastic outputs stay `Estimated` and "uncalibrated"; capital decisions use reference-class widening |

**Open questions.**
- **OPEN-PUSH-1.** Does `UntrustedCandidate` import respect the suite's closed-dependency rule? (Owner decision;
  recommended yes, because no dependency is introduced.)
- **OPEN-PUSH-2.** What is the real distribution of schedule compliance across sites?
- **OPEN-PUSH-3.** Can aggregation error be bounded provably at 10⁸–10⁹ blocks?
- **OPEN-PUSH-4.** How should a client's Deswik/Whittle/COMET plans be ingested as candidates, and which formats?
- **OPEN-PUSH-5.** Underground scheduling scale at large sublevel-stoping operations: is ≤ 10⁴ variables enough
  after aggregation?

## 16. Headframe: the operations twin of record

Source: Brenner pass B6 (`planning/brenner/B6_operations_plant_tailings_workforce.md`, sub-question B6-SQ1),
building on R1, R3, R6 and R7. Every verdict survived a refuter-first attempt and a forced third alternative:
`H6.1c` (AHS intersection-token + safety-bubble DES on `LabRuntime` + `EnergyEnvelope` microgrid co-optimizer +
`G-HEAD-7` uncured-fill refusal) is adopted over `H6.1a` (smooth cycle-time curves, kept only as `PO-HEAD-2`
analytic ceiling) and `H6.1b` (real-time OT closed-loop dispatch, rejected by `INV-002` / `G-HEAD-1`), with
`H6.1d` (event-sourced state-transition twin + MWD/blast-movement spatial assimilation) merged as the site
authority substrate.

### 16.1 Doctrine

- **Twin of record, not twin of control.**
  - Headframe holds the evidence-backed state of the operation: as-built geometry, blasts, loads, cycles,
    equipment condition, airflow, water, energy. Every displayed state is a projection with a staleness tag.
  - No write conduit to ISA-95 Levels 0–2 exists (NONGOAL-002, §31.3).
  - Recommendations (dispatch targets, blend set-points, dig lines, maintenance timing) are `EFFECT-*` prepared
    effects. A human or a third-party control system may act on them, but FrankenMining never does.
- **It coexists with the OEM stack.** Fleet management, autonomy and dispatch belong to the OEMs:
  - Komatsu FrontRunner (Cortez: 23 haul-truck retrofits completed May 2026, site-wide rollout planned for Q3 2026;
    Carlin: 40 retrofits from Q2 2026, autonomous operations planned for Q1 2027; FACT, E618, E619), on a new
    fleet management system whose vendor Barrick does not name (Modular DISPATCH is HYPOTHESIS);
  - Caterpillar MineStar (Boddington: 41 autonomous trucks; FACT R6);
  - Micromine Pitram underground (NGM Goldrush; FACT R6);
  - Wenco, Sandvik and Hexagon.

  Headframe ingests their exports as observations with error models, and **scores their predictions** (§16.5).
- **The Avathon contract** (E024, B5 W1). Data exchange with an incumbent AI platform is limited to
  anchor-pinned exports of certified projections, plus evidence-only write-back: their outputs enter as
  `ModelledUncalibrated` claims until scored. FrankenMining is the auditable kernel that agents cite, not a
  competing agent platform (NONGOAL-005).
- **The time model is strict.** Every telemetry record has sensor time, ingest time and anchor time.
  - Sensor clock drift is estimated per source.
  - Records arriving after an anchor seals are late evidence. They supersede projections; they never edit
    history (§6.3).

### 16.2 As-built geometry and survey epochs

| Input | Processing (`fmine-ops::asbuilt`) | Output |
|---|---|---|
| Surface DTM epochs (drone photogrammetry, LiDAR, GNSS pickups) | Registration to the mine grid. ICP with pose covariance follows frankensim's `fs-asbuilt` Kabsch/Umeyama pattern (Pattern, §13.9). Ground-control residuals become the registration error model. | Surface epochs with vertical uncertainty per cell |
| Underground cavity monitoring scans (CMS), drive scans | Mesh ingest (LAZ/E57, §32) → watertight check → comparison against design solids | Overbreak, underbreak, ELOS per stope, scored against Highwall dilution forecasts (§14.3) |
| End-of-period surveys | Surface-to-surface volume with an **interval enclosure**, using the divergence-theorem volume over registered meshes (`fmine-geom::volcert`, §13.9) | Mined volume per period and polygon, with uncertainty |
| Depletion | Volumes intersected with Strata L5 → depletion epochs (§13.2) | As-mined solids. These are the denominators of F-factors (§20.4). |
| Stockpile and heap surveys | Volume above a base surface whose own uncertainty is modelled. Bulking factor is a claim. | Stockpile tonnes posterior (§20.8) |

**FACT (R6, AusIMM 2022).** Standard F2 can misstate tonnes and grade because in-situ models ignore blast swell.
Headframe therefore carries swell and bulking as explicit, uncertain parameters per domain, never as a constant.

### 16.3 Drill and blast (`fmine-blast`)

**Records.** The blast lifecycle is:
- design: pattern, burden, spacing, hole diameter, depth, sub-drill, stemming, explosive product and density,
  initiation and timing;
- as-drilled: collar survey, depth, deviation;
- as-charged: per-hole kg, product, gassing;
- fired: date and time, weather, misfires;
- post-blast: muckpile survey, fragmentation images, blast-movement monitors, vibration and airblast records.

The `shot_` and `blast_` prefixes apply (§0.3).

**Measure-while-drilling (MWD) is free geology.**
- Rotary penetration rate, torque, pull-down and specific energy of drilling, at blasthole support, are
  observations of rock hardness and competence. They feed the Strata hardness and UCS fields (§13.6) through
  `CorrelationCard`s (specific energy of drilling → UCS proxy) per rig and bit type.
- Rig-specific bias is a hierarchical term.
- HYPOTHESIS: MWD-conditioned hardness improves SAG-throughput forecasts measurably over drillhole-only hardness.
  Tested at the pilot site through registered forecasts.

**Fragmentation.**
- Kuz-Ram predicts x50 = A·K^−0.8·Q^(1/6)·(115/RWS)^(19/30), with rock factor A, powder factor K and charge mass
  Q (FACT[U], R7 §6.1). The uniformity index comes from the design.
- Swebrec, P(x) = 1/{1 + [ln(x_max/x)/ln(x_max/x50)]^b}, fixes Rosin–Rammler's fines tail (FACT[U], R7 §6.1).
- Both are **cards with per-domain learned parameters**. Image-based fragmentation measurement (shovel, truck
  tray or crusher cameras) is an observation with its own bias, because image methods under-count fines.
- The fragmentation → mill throughput card closes the mine-to-mill loop (§17.3).

**Blast movement.**
- Blasting displaces *and mixes* material (FACT, R7 §6.1).
- Movement vectors come from buried monitors (BMM-class) or a movement model calibrated to them. They shift
  ore/waste polygons *before* dig-line classification (§15.7).
- Mixing is modelled as a dilution kernel. Shifting alone does not recover it.

**Vibration, airblast and flyrock.**
- Peak particle velocity follows a site law, PPV = k·(D/√W)^−β (FACT[U]). It is fitted by Bayesian regression
  per monitoring location, with prediction intervals.
- Charge-per-delay limits at sensitive receptors are computed at a declared exceedance probability (policy data,
  e.g. AS 2187.2 or USBM RI 8507 frequency-dependent criteria, as a regulatory pack).
- Flyrock exclusion zones are policy data. FrankenMining computes and records them; it never clears a zone.

**Underground development and production blasting.**
- **Development rounds are cycle records:** drill (jumbo face pattern: cut/burn holes, easers, perimeter holes for
  smooth-wall), charge (emulsion or ANFO; a charge plan per hole), fire (in the blasting window, from the central
  blasting system), ventilate and re-enter (re-entry time is policy data from §16.6, with §14.3 seismic exposure
  rules), scale, muck (LHD), survey, then support (bolts, mesh, shotcrete per §14.3), and the next round.
- **Advance per round** = pull fraction × hole depth. Pull is observed per round (often 0.85–0.95, FACT[U]).
  Overbreak and underbreak come from scanned profiles against design (§16.2) and feed the perimeter-control cards.
- **Development rate** is a DES over headings on the §16.4 engine: one jumbo, charge-up unit and LHD are shared
  across several headings, so interference is explicit. Planned development metres per month are a registered
  forecast scored monthly (§10), because development shortfall is the usual leading indicator of underground
  production shortfall.
- **Longhole and ring production blasting:** ring design records (toe spacing, burden, hole length), hole
  deviation from gyro surveys (an observation with its own error model), charging records, and outcomes per stope:
  fragmentation at the drawpoint and dilution as ELOS from cavity scans (§14.3). Deviation above tolerance updates
  the predicted fragmentation and dilution before the ring is fired.
- **Backfill** (paste or cemented rock fill) is a cycle dependency. Cure time to the required strength is a
  precedence constraint in the stope sequence (§15), with strength from cylinder tests as evidence.
  - Strength is a `FillStrength(t)` curve with a degradation flag, not a monotone cure: Loulo paste at 3.5%
    binder falls from 2.65 MPa at 56 d to 1.10 at 112 d, and Kibali's legacy slag-cement fill has degraded
    (FACT, E233, E234). Design strength depends on exposure (E236).
  - Planned dilution is a typed `DilutionMatrix(stope_type × sequence × wall × n_exposures)`: Kibali plans
    2.0–2.5% per paste wall exposed (FACT, E235).
  - **Uncured or degraded fill refuses (G-HEAD-7, FMINE-REF-504).** A secondary or tertiary stope that exposes a
    wall of filled primary stope gets no ring-charge plan or extraction step if the fill's `FillStrength(t)` is
    below `UCS_design(wall height, span, exposures)`, or if its cylinder and binder-dosage QA is missing or
    conflicted. `FillStrength(t)` covers early-age cure and late-age binder loss from sulphate attack.

**Safety-semantic refusal FMINE-REF-401.**
- Reactive ground is pyritic ground where ammonium nitrate can react exothermically. If a hole lies in a domain
  whose reactivity class is `unknown`, or whose measured ground temperature exceeds the policy threshold, no
  ANFO or emulsion charge plan is produced.
- Only an inhibited-product plan, which requires a human authority token, may be prepared.
- This uses Strata's S-field projection plus hole temperature records.

### 16.4 Load and haul (`fmine-ops::haul`, `fmine-ops::fleet`)

**Ingest.**
- FMS cycle records are snapshot extracts from DISPATCH, MineStar, FrontRunner, Wenco or Pitram (§32). Each cycle
  carries its components: queue, spot, load, haul loaded, dump, return.
- Payload comes from on-board weighing with a per-truck bias model calibrated against the weighbridge (§12.5).
- Assignment of loads to material polygons is a probabilistic join: GNSS position at load time plus bucket
  position uncertainty. A load is never assumed to come from exactly one block.

**The time-usage model** is a registered standard (GMG time categories): calendar → available → utilized →
productive, with delays typed.
- Availability alone misleads. A fleet can show 95.7% availability with 28.1% utilization (FACT, R7 §6.3).
- OEE is computed on calendar time, not loading time. Loading-time OEE overstates performance (FACT, R7 §6.3,
  Elevli & Elevli 2010).
- Every time-usage KPI states its category basis, or it is refused (G-HEAD-2).

**Haulage model.**
- The haul-road network is a graph with segment grade, rolling resistance, speed limits and surface condition.
  Routing uses frankennetworkx Dijkstra/A* (FACT, R3 §4.4).
- Travel time on loaded and empty segments comes from OEM rimpull and retarder **performance cards**, by truck
  class and payload.
- Fuel and energy come from the same cards, which supports diesel vs trolley-assist vs battery-electric
  comparisons as `CapitalCandidate`s (§23).

**Fleet simulation, sizing and autonomous haulage (`fmine-ops::haul`).**
- A discrete-event simulation runs on deterministic virtual time (asupersync `LabRuntime`, E012), with stochastic
  cycle components fitted from FMS records.
- **Autonomous haulage changes the queueing (FACT plus DESIGN).** NGM runs Komatsu FrontRunner (partnership
  launched July 2025; E618). Cortez finished 23 truck retrofits in May 2026 ahead of a site-wide rollout planned
  for Q3 2026, and Carlin began 40 retrofits in Q2 2026 with autonomous operations planned for Q1 2027 (FACT,
  E619). Carlin's TR lists 40 Komatsu 930E trucks in a mixed fleet and says nothing of autonomy (FACT, E662). The
  DES models what governs autonomous throughput (DESIGN; parameters are site-fitted):
  1. reservations: a truck must hold the path lock for its braking envelope before entering an intersection,
     switchback or dump bay, so bunches become token waits;
  2. staffed vehicles in the autonomous zone: a grader or light vehicle inside a geo-fence slows or stops nearby
     trucks, often the largest source of cycle variance;
  3. weather and dust: sector speed caps when sensing degrades.
- Cross-checks:
  - the match factor MF = (N_trucks × loader cycle) / (N_loaders × truck cycle);
  - finite-source queueing (machine-repair model) as an analytic bound;
  - max-plus cycle bounds from lifted `fs-tropical`.
- TARGET: 1 shift of 100 trucks in ≤ 10 s.
- Fleet-size and AHS-retrofit decisions are DecisionCards with the queueing and DES results side by side (GB-17).

**Dispatch policy.**
- Real-time dispatch belongs to the FMS.
- FrankenMining evaluates dispatch policies *offline* in the DES (assignment by `fsci-opt::linear_sum_assignment`
  or min-cost flow) and publishes recommended shovel targets and priorities as prepared effects.
- Reinforcement-learning dispatch results in the literature are simulation-only (FACT, R7 §5.7). Any learned
  policy is a forecast that is registered and scored before it may be recommended.

**Underground loading, hauling and hoisting.**
- LHD and underground truck cycles come from the same FMS ingest. Cycle components: tram empty, load at the
  drawpoint, tram loaded, dump to orepass or truck. Drawpoint and orepass states (hang-ups, oversize, wet muck) are
  typed events with durations.
- **Shaft hoisting.** Skip cycle = load + accelerate + full-speed travel + decelerate + dump + settle; hoisting
  capacity per hour = 3,600 × skip payload ÷ cycle time × availability × utilisation × fill factor. Each factor
  is a separately registered parameter with the site's own definition. Kibali's 34 skips/h × 14.8 t × 8,760 h ×
  0.90 = 3.97 Mt against a stated 3.2 Mtpa implies a further ≈ 0.81 factor (INFERENCE, E231, E232). Rope, skip
  and brake inspections are reliability records (§16.5). The hoist is usually the system bottleneck, so its
  capacity is a hard constraint in the underground schedule (§15) and its dual value is displayed. A schedule
  that needs a hoist upgrade (Kibali: 3.4–3.6 Mt/yr UG ore from 2028 against 3.2 Mtpa today) registers the
  upgrade as a schedule dependency.
- **Decline haulage versus shaft** is a `CapitalCandidate` comparison (§23) on the same performance cards, with
  ventilation (§16.6) and energy (§16.8) costs from the same realizations.

**Dozing, ripping and scraping.**
- Dozer and scraper productivity are performance cards: OEM handbook curves × correction factors (grade, material,
  operator, visibility), each with a validity domain, calibrated against survey-measured volume changes (§16.2).
- **Ripping versus blasting** is a decision card. Rippability enters as an uncertain input from the Strata P-wave
  velocity or rippability-class field, and the card shows the flip margin on velocity.
- Dozing and scraping in rehabilitation and TSF construction are earthworks events with as-built survey. They feed
  the closure cost estimate (§18.6) and the ARO in Bullion (§21).

**Demolition and decommissioning.**
- Demolition of structures, plant and infrastructure is closure-plan work: quantities from as-built records, a
  hazardous-materials register, unit costs with an uncertainty class, all feeding the ARO (§18.6, §21).
- Each closure domain carries an evidence state: demolished, verified, signed off by the responsible person. As
  everywhere, FrankenMining records the state; a human signs it.

### 16.5 Equipment health, reliability and maintenance

**Reliability models** (`fmine-ops::reliability`):
- Weibull life models for components that are replaced (renewal).
- Power-law NHPP for repairable systems that are not renewal processes (FACT, R7 §6.3, Kumar & Klefsjö 1992).
- Cox proportional hazards with condition covariates: oil analysis, vibration, temperature, payload history, haul
  profile.
- All of these are hierarchical over component → fleet → site → company, so sparse failure records borrow
  strength.
- Censoring is honoured: a component removed while still working is right-censored and never treated as a
  failure (G-HEAD-3).

**Wear.**
- Archard's law (volume = k·F·s/H) is lifted or pattern-copied from `fs-tribo::ArchardLaw` (§13.9).
- It is used as a card for ground-engaging tools, truck tray liners, chute and mill liners (§17.7) and slurry
  pipes, with abrasion-index inputs from Strata.

**Tyres.**
- TKPH limits per haul profile, with tyre life as a survival model.
- FACT (R7 §6.3): one study found 93.6% of 455 tyres scrapped before wear-out. Premature scrapping causes are
  typed (cut, separation, heat) and modelled as competing risks.

**Maintenance decisions.**
- Age replacement, condition-based thresholds and component change-out timing are DecisionCards. They are
  priced in avoided unplanned downtime × production value at the bottleneck, which comes from the Pushback duals
  (§15.6).
- RCM taxonomies follow SAE JA1011 (FACT, R7 §6.3).
- Work orders and spares are SAP PM/MM extracts (§32).

**Scoring vendor predictions (DESIGN; the differentiator).**
- Every predictive-maintenance alert from an OEM or AI platform (Avathon, MineStar Health, Modular Predict) is
  ingested as a registered forecast: asset, failure mode, horizon, stated probability or severity.
- It is scored when the window closes, using maintenance records and failure codes. The calibration ledger (§10)
  then holds a per-vendor, per-failure-mode track record: precision, lead time and reliability diagram.
- **Why.** These platforms are adopted on business-case IRRs (INFERENCE, R6 §6.8: in the Barrick and Newmont
  filings read, the only quantified digital results are autonomous-haulage cases; Freeport reports 214 Mlb of
  incremental copper from leach analytics in each of 2024 and 2025, FACT, E645). No shared score across vendors is
  published. FrankenMining keeps one, without competing with them.

### 16.6 Ventilation and underground environment (`fmine-ops::vent`)

- **Network model.**
  - Airways, regulators, doors and fans form a network.
  - Branch resistance follows Atkinson's square law, p = R·Q², which is a quadratic-loss network (FACT, R1:
    `fs-airflow` solves fan operating points by interval Newton, and its general-topology `graph` module is
    nominal and uncertified).
  - Posture: Bridge after GATE-FSIM-RECONV; until then, Pattern (a Hardy Cross / Newton network solve built in
    `fmine-ops::vent`, with fs-airflow fixtures as oracles).
- **Geometry from the twin.** The development as-built (§16.2) generates the airway network. Each airway's
  resistance is a claim from its friction factor, perimeter, length and area.
- **Requirements as policy data.**
  - Diesel dilution rates (m³/s per kW, which are jurisdiction-specific). Both Kibali and Loulo use 0.05 m³/s
    per kW; Kibali states it without units (FACT, E238).
  - The registry carries `adopted_standard` separately from jurisdiction: Kibali, in the DRC, adopts Western
    Australian regulations (FACT, E237).
  - Gas, dust and diesel particulate limits.
  - Heat: wet-bulb or other heat-stress index limits.

  These are jurisdictional packs. FrankenMining computes compliance *evidence* and never asserts compliance
  (`regulatory_compliance_claim = false`).
- **Heat.** Autocompression, wall-rock heat flux (geothermal gradient and rock thermal properties from Strata),
  diesel and electrical heat, and groundwater. The rock heat flux is an `fs-conduction`-type operator (Bridge
  later); a correlation-card heat load is used before then.
- **Ventilation-on-demand.** Set-point recommendations only. Fans and regulators are OT.
- **Refuge chambers and egress.** Their positions, capacities and inspection records are tracked as
  critical-control evidence (§19.5).

### 16.7 Dewatering and mine water

- Pump stations, sumps and pipelines form a network.
- Inflow forecasts come from the hydrogeological model (§14.5) and rainfall.
- Pump curves and energy are cards.
- Outputs:
  - P(sump level exceeds alarm level) over the forecast horizon;
  - pumping energy per period;
  - inflow forecasts registered and scored.
- Mine water flows into the site water balance (§18.4).

### 16.8 Energy, power supply and emissions (`EnergyEnvelope`)

- Diesel, power and explosives consumption are attributed to activities using the same drivers as cost
  (`CostDriverCard`, §21.6): litres per tonne-km, kWh per tonne milled.
- **Power is a binding constraint at remote and weak-grid sites (`EnergyEnvelope`; FACT plus DESIGN).**
  - Lumwana (FACT, E668): the expansion targets a 52 Mtpa run rate during 2028 (peak design 54) with electrified
    ultra-class shovels. A 2024 grid study found that "for every MW generated more than a third is lost as
    unusable reactive power". The response combines 330 kV transmission lines (complementing STATCOMs at
    neighbouring mines), 100% BESS backup, 50% emergency generating capacity on site (a target of 31.5 MW by
    Q4 2025) and 45% imported power within the Southern African Power Pool.
  - Kibali (FACT, E669): three hydropower stations (Nzoro II about 22 MW, Ambarau 10.6 MW, Azambi 10.2 MW; 42.8 MW
    peak), a 32 MW high-speed diesel bank, a 7 MW BESS (2020) that cut the spinning reserve from nine generators to
    four, and a 16 MW solar plant with a 15 MW BESS (2025). Renewables supply about 85% of the site's energy; the
    diesel bank sets the marginal cost in the dry season.
  - Coupling (DESIGN; `fmine-ops::energy` ↔ `fmine-plan` §15.6 ↔ `fmine-plant` §17). Each period has a
    piecewise-linear supply stack of tiers (hydro and solar, grid or imports, BESS discharge, diesel), each with
    capacity P_j,t and cost c_j. Demand must fit it:
    Σ_{s,p} x_{spt} (e_grind(s, p) + κ_O2 S_s 1[p ∈ POX]) + P_hoist,vent,dewater(t) + P_shovels(t) ≤ Σ_j P_j,t,
    with reactive balance Q_load(t) − Q_comp(t) ≤ Q_max(t) and apparent power P² + Q² ≤ S_max(t)², the latter as a
    piecewise-linear outer approximation in the LP. The power shadow price λ_MWh(t) then moves soft or oxidized ore
    to the mills in dry-season or high-tariff windows, shifts deferrable dewatering and hoisting, and prices BESS,
    compensation and solar expansions as `CapitalCandidate`s against avoided diesel and mill trips.
- Scope 1 and 2 emissions are computed per activity with factor provenance, following the GHG Protocol. Emission
  factors are claims with sources and validity.
- Carbon prices and renewable PPAs enter cost scenarios through the shared scenario index (§23.4).
- Electrification, trolley assist and renewables are `CapitalCandidate`s.

### 16.9 OT data ingestion

| Source | Path | Notes |
|---|---|---|
| Historians (AVEVA PI and others) | Exported snapshots, or a read-only OPC UA client on a DMZ relay node, outbound-only from site (§26, §31.3) | Never a write session. Tag maps are versioned. |
| FMS (DISPATCH, MineStar, FrontRunner, Wenco, Pitram) | Vendor export or API snapshots | Each vendor schema is a `Portal` adapter with quarantine-on-import (§32) |
| Drill rig MWD, blast design software | File exports (vendor formats, IREDES where available) | — |
| Condition monitoring | Oil-lab certificates, vibration exports | Lab certificates are `ThirdPartySigned` |
| Fragmentation, BMM, vibration monitors | Vendor exports | Each carries a measurement-error model |

- Telemetry lands in segment logs (`FMT-TELSEG-1`, §26) and is compacted into anchor-sealed summaries.
- fgdb and frankensearch are projections over it, never the authority (B4 G2: a graph-database authority fails at
  telemetry rates).

### 16.10 Guards, proof obligations, gates and kills

| Guard | Condition | Action |
|---|---|---|
| G-HEAD-1 | A recommendation would be delivered to an OT endpoint | Refuse. No such conduit exists (§31.3). |
| G-HEAD-2 | A time-usage KPI without its category basis | Refuse the render |
| G-HEAD-3 | A censored component life treated as a failure | Refuse the fit |
| G-HEAD-4 | A stale instrument is used for a TARP, ventilation or gas statement | `not_observable` (FMINE-REF-202) |
| G-HEAD-5 | A charge plan in a reactive or unknown-reactivity domain | FMINE-REF-401 |
| G-HEAD-6 | A payload or weightometer reading without a calibration in validity | Down-weight to `ModelledUncalibrated`; flag in reconciliation |
| G-HEAD-7 | A secondary/tertiary stope blast or extraction step adjacent to a paste/CRF-filled stope whose `FillStrength(t) < UCS_design(exposure)` or whose cure/degradation QA is `missing`/`conflicted` | FMINE-REF-504 (`AdjacentFillUncured`, §16.3) |

| PO | Statement | Verification |
|---|---|---|
| PO-HEAD-1 | The DES is deterministic: same seed and inputs ⇒ same event trace digest | LabRuntime replay |
| PO-HEAD-2 | DES throughput converges to the analytic finite-source queue for exponential components | Known-answer test |
| PO-HEAD-3 | Survey volume enclosures contain the true volume on synthetic surfaces | Analytic shapes |
| PO-HEAD-4 | Ventilation network solve satisfies Kirchhoff's laws to tolerance; fan operating point matches the fs-airflow fixture | Oracle fixture |
| PO-HEAD-5 | Every ingested vendor prediction resolves to a scored outcome or a declared non-resolution | Ledger audit |

| Gate / kill | Threshold (TARGET) | Fallback |
|---|---|---|
| GATE-HEAD-01 | One year of FMS cycle data from a pilot pit ingested; load-to-polygon join coverage ≥ 95% of tonnes | Polygon-level aggregates only |
| GATE-HEAD-02 | DES reproduces 12 months of actual shift production within ± 5% (median shift), with calibrated 80% intervals | Analytic queueing bounds only; DES labelled uncalibrated |
| GATE-HEAD-03 | Vendor-prediction scoring live for ≥ 1 vendor at the pilot | — |
| ✖ KILL-HEAD-1 | No FMS export access by month 9 of a site engagement (§34.4) | Headframe limited to survey epochs and FA-certified activity drivers |

### 16.11 Galaxy-brain cards (illustrative values)

**GB-17 · How many trucks per shovel? Match factor vs queueing.**
- **Equation.**
  - MF = N·t_load / (N_loaders·(t_load + t_travel)).
  - Finite-source queue (one shovel, N trucks, exponential times): utilization U = 1 − P₀, with
    P₀ = [Σ_{n=0}^{N} N!/(N−n)!·rⁿ]⁻¹ and r = t_load/t_travel.
- **Substituted.**
  - t_load = 2.5 min and t_travel (haul, dump, return, spot) = 22.5 min, so r = 0.111.
  - At N = 10, MF = 1.00, so the deterministic answer is a fully used shovel at 24 loads/h.
  - The queue model gives U = 0.832 and **20.0 loads/h (−17%)**, with an average queue of 5.1 min per cycle.
  - At N = 12, U = 0.917 and 22.0 loads/h, but per-truck productivity falls from 2.00 to 1.83 loads/h.
- **Intuition.**
  - MF = 1 is the point where variability costs the most.
  - The deterministic answer overstates production. The exponential model understates it, because real cycle
    CVs of about 0.2–0.4 are smaller than exponential's 1.0.
  - The DES with fitted distributions sits between them. The decision depends on whether the shovel (an ore
    face whose margin is priced by the §15.6 duals) or the trucks are the expensive resource.
- **Assumptions.** One shovel; a homogeneous fleet; no breakdowns. Breakdowns enter through availability.
- **Flip.** If the shovel serves the bottleneck ore face (high dual), add trucks until marginal loads × margin <
  truck hourly cost. If it serves waste with stripping slack, run MF < 1.

**GB-18 · What is blast-movement monitoring worth on one ore blast?**
- **Substituted geometry.** An ore zone 40 m wide × 100 m long × 10 m bench at 2.7 t/m³ is 108,000 t. A uniform
  3 m horizontal movement with unshifted polygons misallocates 3 × 100 × 10 × 2.7 = 8,100 t on each side.
- **Ore loss.** 8,100 t at 1.5 g/t is 390.6 oz; at 90% recovery and $3,000/oz, that is $1.055M of revenue, less
  $178k of avoided processing at $22/t, net **−$0.877M**.
- **Dilution.** 8,100 t of 0.1 g/t waste processed costs $178k against $70k recovered: net **−$0.108M**.
- **Total: ≈ $0.98M per blast.** Because blasting also mixes, perhaps 60% is recoverable by shifting, so about
  **$0.59M**. Monitoring costs perhaps $15k per blast.
- **Intuition.** On narrow ore, movement-blind dig lines are one of the cheapest leaks to plug. The value scales
  as movement ÷ ore width.
- **Assumptions.** Uniform movement (real movement varies with depth and the free face); no mill-capacity
  displacement cost (adding it increases the value).
- **Flip.** Movement < 0.5 m, or ore zones ≫ 100 m wide, make monitoring marginal. The pilot's registered
  F1/F2 changes after adopting shifted dig lines are the test.

**GB-40 · AHS intersection tokens vs. staffed haulage (Carlin's planned 40-truck autonomous fleet).**
- **Equation.**
  - Token service time at a single-lane switchback or dump bay: s = τ_clear + v₀/(2a_brake) + τ_comms.
  - With N trucks of cycle T, each passing each of m single-token points once per cycle, every point sees
    λ = N/T. Pollaczek–Khinchine wait: W = ρs(1 + C_s²) / (2(1 − ρ)), with ρ = λs.
  - Staffed-vehicle stops cost ΔT_LV = ν_LV · p_overlap · τ_stop · T per cycle.
- **Substituted** (illustrative: N = 40, T = 24 min, m = 2, s = 0.367 min, C_s² = 0.25; ν_LV = 0.05 min⁻¹,
  p_overlap = 0.25, τ_stop = 3.5 min):
  - λ = 1.667 min⁻¹ per point, ρ = **0.611**, W = 0.360 min per pass, **0.72 min per cycle**;
  - ΔT_LV = 0.05 × 0.25 × 3.5 × 24 = **1.05 min per cycle**;
  - the cycle rises 24.0 → **25.77 min**, −6.9% fleet capacity, about **3.8 Mt/yr** on a 55 Mt/yr pit. A smooth
    staffed-haulage model predicts no token or geo-fence delay.
- **Intuition.** Autonomy removes operator shift-change losses but turns geometric bottlenecks and staffed-vehicle
  intrusions into hard queueing locks.
- **Assumptions.** Single-token points, Poisson-like arrivals and unscheduled staffed entries. Carlin's autonomous
  operations start in Q1 2027 (E619), so every parameter is a prior until its FMS data exist.
- **Flip.** A second lane (c = 2 tokens) gives ρ = 0.306 per server and W ≈ 0.02 min per pass (M/G/2,
  Lee–Longton), and moving grader work into scheduled windows cuts ΔT_LV by about 80%. Road widening and
  staffed-vehicle windows become a quantified DecisionCard.

**Open questions.**
- **OPEN-HEAD-1.** FMS export rights and formats at the pilot site (OEM contracts).
- **OPEN-HEAD-2.** Whether Avathon will exchange anchor-pinned certified projections (B5 F1).
- **OPEN-HEAD-3.** Fatigue and collision-avoidance event data: lawful-basis and works-council constraints (§19.4).
- **OPEN-HEAD-4.** Ventilation jurisdiction packs needed for the first underground site.

## 17. Crucible: processing, metallurgical accounting and on-site refining

Source: Brenner pass B6 (`planning/brenner/B6_operations_plant_tailings_workforce.md`, sub-question B6-SQ2),
building on R7 §7, R1 (cards, wear) and the C2–C4 technical-report corpus. Every verdict survived a refuter-first
attempt and a forced third alternative: `H6.2b` (four-layer versioned `CorrelationCard`s + N-tank RTD
deconvolution `fmine-plant::rtd` + curvature correction + two-sided stoichiometric blend windows)
and `H6.2d` (AMIRA P754 check-in/check-out bilinear reconciliation with `G-CRUC-6` refusing tautological tonnage MCF)
are adopted over `H6.2a` (steady-state shift regression, kept only as `R0` published-card reproduction mode) and
`H6.2c` (first-principles population-balance/CFD twin, rejected for v1 on unidentifiability and scale). The crate
is `fmine-plant`.

### 17.1 Doctrine

- **The plant is where the subsurface belief meets money.** Every block's value depends on a recovery and
  throughput prediction. Those predictions are the least-validated numbers in most mine plans: recovery is
  routinely extrapolated beyond the tested range (FACT[U], R7 §7.2).
- **Every process prediction is a card with a validity domain.** A prediction outside the validity box is refused
  (FMINE-REF-203) or explicitly extrapolated with colour `Estimated` and a UU term.
- **Recovery and throughput apply at processing support.** That is a shift or day blend, declared per circuit, not
  a block (§13.4). The block-level "recovery" in planning (§15.6) is a domain attribute used under the linearity
  assumption A-PUSH-5, and the plant reconciliation tests it.
- **Control belongs to the DCS and the plant's advanced process control vendor.** Crucible recommends set-points
  as prepared effects and scores every vendor twin's predictions (§16.5 pattern).
  - Example: Metso's Geminex at Lihir was "technically deployed" but not operationally commissioned, with no
    quantified benefits published; Metso asserts only unquantified energy, water and reagent savings (FACT, E643).
  - The industry's believed proof is Freeport's TROI at Bagdad: about +10% throughput and about +1 point copper
    recovery (FACT[U], consultant-reported by McKinsey in 2020 and seen only second-hand; absent from Freeport's
    filings; E644).

  Crucible's job is to make such claims *measurable*: registered before, scored after.

### 17.2 The flowsheet graph (`fmine-plant::flowsheet`)

- **Units.** A flowsheet is a typed directed graph of unit operations, versioned with valid time (plant
  modifications are supersessions). Unit types:
  - crushers and HPGR;
  - SAG, AG and ball mills;
  - cyclones and screens;
  - gravity concentrators (centrifugal, spirals, shaking tables);
  - flotation banks and columns;
  - thickeners and filters;
  - leach tanks; CIL/CIP trains; elution; electrowinning cells; induction or tilting furnaces;
  - heap leach pads with ADR plants;
  - autoclaves (POX), roasters, BIOX reactors;
  - SX mixer-settlers and EW tank houses;
  - Merrill–Crowe;
  - tailings pumping;
  - water recovery.
- **Streams.** Each stream carries:
  - solids and water mass flow;
  - per-element assays with measurand identity (§12.1);
  - a particle size distribution (PSD);
  - optionally, mineralogy and liberation classes;
  - solution assays (Au, Cu, CN-WAD, pH, Eh);
  - each measurement node's error model (§12.5).
- **The same graph serves three purposes:**
  1. **Prediction:** steady-state mass balance driven by unit cards.
  2. **Metallurgical accounting:** data reconciliation of measured streams (§17.5).
  3. **Material-flow reconciliation:** the plant is a subgraph of Touchstone's mine-to-market graph (§20.1).

### 17.3 Process models as cards

Every model below follows frankensim's `CorrelationCard` pattern (Pattern, §13.9). Each card has inputs with units,
a validity box, fitted parameters with a covariance, the fitting dataset's evidence references, and a colour.

| Process | Model family | Card inputs | Notes and doctrine |
|---|---|---|---|
| Crushing and grinding energy | Bond's law, W = 10·Wi·(1/√P80 − 1/√F80) kWh/t (FACT[U], R7 §7.1); SMC/Morrell specific energy for AG/SAG; JK drop-weight A×b, ta | Hardness at processing support (additive proxy, §13.4), F80, target P80, circuit configuration | Wi depends on closing screen and feed preparation (FACT, R7 §7.1). Throughput = installed power × utilization ÷ specific energy, with **hardness variability as the main driver**. |
| Fragmentation → SAG throughput | Mine-to-mill card on ROM x50/x80 and fines fraction (§16.3) | Fragmentation observations | HYPOTHESIS per site; the card is fitted only after registered forecasts are scored |
| Classification | Efficiency-curve models (e.g. Plitt-type corrected partition curves) | Cyclone geometry, pressure, solids % | — |
| Gravity (Au) | GRG (gravity-recoverable gold) test-based recovery | GRG tests on composites | Gravity also reduces the coarse-gold nugget problem in downstream sampling (FACT[U], R7 §7.4) |
| Gravity (Sn) | **Size-by-size** recovery curves for cassiterite, with heavy fines and slimes losses | PSD and liberation by size class | Tin recovery is strongly size-dependent (FACT, R7 §7.5, Angadi et al. 2015), so the Sn card is coupled to the comminution card, never evaluated alone |
| Flotation (Cu, Ag, Au in sulphides) | First-order R(t) = R∞(1 − e^(−kt)) with a **distribution of k** (rectangular/Klimpel or gamma); grade–recovery frontier | Mineralogy (CuS speciation, pyrite), grind, reagents, residence time | FACT (R7 §7.3): 71% of rectangular-model R∞ estimates fell below the measured 32-minute recovery. The rate-distribution choice changes extrapolation, so every card records it, and model choice is a `ModellingDecision`. |
| Cyanide leach and CIL/CIP | Leach kinetics (shrinking-core or empirical); carbon adsorption isotherms and kinetics; cyanide and lime consumption | Head grade, gravity tail, PRI and TOC (§13.6), grind, sulphide content, cyanide-soluble Cu | Preg-robbing is the classic surprise. The PRI(TOC) card feeds recovery per realization. CIL mitigates mild preg-robbing; strongly preg-robbing carbonaceous ores need roasting, pressure oxidation with thiosulphate leaching, or blanking agents (FACT[U], R7 §7.4), so the route itself is a decision variable. NGM measures preg-robbing as a **percentage from a spike test**, with source-specific feed caps (< 40% or < 60%; FACT, E173, E175); the same organic carbon gives 0–40% or 75–100% by source, confirmed by Raman, so the card takes an optional `carbon_type` covariate (E175). A TCM threshold is scoped to its use: TCM > 1% was waste for one test-sample selection yet is capped feed (< 120,000 t/y) at the Sage mill (C2 §4.3e). Goldstrike's calcium thiosulphate RIL circuit was converted to CIL in Q1 2023 for $30M (FACT, E174); no demo text presents it as running thiosulphate after that date. |
| Heap leach | Recovery-vs-time curve per ore type and crush size, R(t) = R∞(1 − e^(−kt)) with column-to-field scale-up factors; solution application rate; lift schedule | Column tests, crush size, clay content, agglomeration | Scale-up from columns to the field is the dominant uncertainty and is explicit (GB-20). Heap inventory is a latent state (§20.8). **Book convention** (FACT, Veladero 2013–2017; E275, E276): sites carry heap WIP as a recoverable-basis roll-forward, Inv_t = Inv_{t−1} + Σ T·g·R_card − produced, with R_card a fixed grade/ore-type formula. The card exposes R_card (book) and R∞ (posterior) as separate named parameters, and the inventory's `MetalStage` is `Recoverable` (§25.2). |
| Refractory gold: POX, roasting, BIOX | Sulphide oxidation extent → downstream CIL recovery; oxygen and acid balance; As fixation | S²⁻, As, carbonate, organic C | No independent head-to-head review exists, only site comparisons (FACT, R7 §7.4). Cards are site-specific and `Estimated` until reconciled. |
| Copper oxide/secondary leach and SX-EW | Heap or dump leach curves; SX extraction and stripping stages (McCabe–Thiele on isotherms); EW mass by Faraday's law, m = I·t·M/(n·F) × current efficiency | CuAS/CuT ratios (§13.6), acid consumption (gangue), PLS grade | Cathode output is a check-out measurement with high precision, which makes it a strong reconciliation anchor |
| Silver | Flotation concentrate payables or cyanidation (Merrill–Crowe or carbon) | Ag mineralogy | Silver is mostly a by-product; its payable terms dominate its value (FACT, R7 §7.5) |
| Thickening, filtration, tailings rheology | Settling flux and yield stress cards | Clay mineralogy, PSD | frankensim LBM has no Bingham or Herschel–Bulkley rheology (FACT, R1). These are correlation cards until upstream adds them. |

**Validity enforcement (DESIGN).** A plan whose feed blend leaves a card's validity box (hardness, grind, PRI or
clay outside the tested range) is published with the out-of-domain claim highlighted in red and its UU term
priced as a flip margin (§9.3). Example: "Recovery falls below the plan's break-even if preg-rob > 60% in the Q3
blend; testwork covers only < 40% for this source."

**Card layers (DESIGN; C4 P8).** A recovery card has four layers, each versioned:
1. the test-work card with its validity box;
2. named stage operational factors, attested (Pueblo Viejo business plans apply them on top of test-work cards;
   FACT, E256);
3. caps and correction factors as explicit parameters (Zaldívar caps insoluble-Cu recovery at 55%, cutting
   average recovery by about 9%; FACT, E290);
4. the route assumed for reserves, as a `ModellingDecision` (Pueblo Viejo assumes the costlier flotation+POX route
   for all ore; FACT, E265).

- The Cu heap/dump card is speciation-weighted, TCu_rec = F1·ASCu/TCu + F2·CNCu/TCu + F3·SCu/TCu, with residence
  time in the validity box (Zaldívar F1/F2/F3 = 96/78/40; E289). It is not an R(t) curve.
- If Pushback or Hallmark use a recovery other than Crucible's current card, the statement shows the divergence.
  Porgera's recovery equations "are not used" in its cut-off or reserve models (FACT, E295).
- **Versions are real and mixed.** One Turquoise Ridge reserve statement uses three intercepts of the same surface
  card (100.79, 97.14, 101.79), plus a +3% plant adjustment (FACT, E181). Every Reserve element cites its card
  version.
- **Unit-aware ingestion.** One company publishes three unit conventions for its cards (g/t converted inside the
  formula by ×0.029167; oz/st; g/t with fractions labelled "%"), and one printed formula has unbalanced
  parentheses (FACT, E171). `CorrelationCard` gains `input_units`, `output_scale` and `parse_check`. G-UNIT
  refuses a card whose inputs mix units, and an import gate evaluates every card at three reference grades
  against the published LOM average.
- **Fitting hygiene and residence time (`fmine-plant::rtd`; FACT plus DESIGN).** The Goldstrike roaster curve was
  fitted on 12-hour shifts, dropping shifts below 90% availability and their neighbours "to avoid upset plant
  conditions and false CIL retention times" (FACT, E170). Leach trains hold a day or more of inventory: the Cortez
  Pipeline CIL's nominal retention is 36–48 h (FACT, E663), and the Carlin TR gives no hours. Shift-paired head and
  tail assays therefore misstate recovery whenever feed grade moves. `fmine-plant::rtd` models a leach train as N
  stirred tanks in series with bypass fraction α and dead-volume fraction β:
  h(τ) = α δ(τ) + (1 − α) (N/τ̄) (Nτ/τ̄)^{N−1} e^{−Nτ/τ̄} / (N − 1)!, with τ̄ = (1 − β) V / Q.
  When the slurry flow Q(t) varies (the < 90% availability shifts), the convolution runs in throughput-volume
  coordinates ∫Q dt rather than clock time, because the kernel is then time-varying. Convolving feed tonnes and
  grade with h tracks gold in circuit and removes the hold-up bias from fitted cards and from shift and daily
  balances (§20.4; GB-41).
- **A ready calibration demo.** Turquoise Ridge registers 71 monthly lab-vs-plant pairs (2008–2014): plant minus
  bottle-roll mean +2.35 points, SD 4.49, with 17 lots above +5 (FACT, E177). It is a public dataset for the first
  recovery-forecast calibration card (§10).

### 17.4 The geometallurgical chain, end to end

1. **Strata (§13.6)** holds the spatial fields: grades, hardness proxies, PRI and TOC, CuAS/CuT, S, As, clay,
   density.
2. **Pushback (§15.6)** chooses blends at processing support.
3. **Crucible** predicts throughput, recovery, reagent use and concentrate quality for the blend, per
   realization, and registers the shift/day/month forecast (§10).
4. **Touchstone (§20)** reconciles actual plant performance and scores the forecast. Residuals update card
   parameters (with an InvestigationCase when a surprise monitor fires, §10.5) and Strata's geomet field bias
   priors.

**Sampling density (OPEN-STRATA-8).** The minimum geometallurgical sampling density at which transfer cards beat
domain averages is an OPEN question. Crucible measures it on the pilot by comparing registered forecasts made with
cards against forecasts made with domain averages.

**Geomet coverage report.** For each metallurgical domain, Crucible lists which percentile bands of Strata's
SMU-support distribution of each primary variable are covered by tested samples. The target is every 5th
percentile; with fewer than ten samples, at least the 5th, 25th, 50th, 75th and 95th (E389). Cards evaluated
outside the covered bands render `OutsideValidity`. This turns OPEN-STRATA-8 into a measurable gap before the
forecast comparison runs.

### 17.5 Metallurgical accounting (AMIRA P754 discipline)

**Principles adopted (FACT, R7 §7.6).** The AMIRA P754 Code of Practice for Metal Accounting requires:
- unbiased and precise accounting;
- check-in/check-out measurement;
- a stated precision for every mass, sampling and assay stream;
- data reconciliation;
- stocktakes;
- competent-person sign-off and independent audits;
- separation of primary (whole-plant) and secondary (circuit) accounting.

**Implementation (DESIGN).**
- **Data reconciliation.** Weighted least squares or Bayesian reconciliation over the plant subgraph, using the
  measurement-error covariances of §12.5. It is run by the same engine as mine-to-mill reconciliation (§20.3), so
  plant and mine numbers never disagree by construction.
- **Gross-error detection.** A global χ² test, per-measurement tests and an `eproc` drift monitor per sampler and
  weightometer. A failed test opens an InvestigationCase. Measurements are never silently dropped.
- **Stocktakes** cover gold in circuit (carbon inventory, solution and tank inventory, clean-up results) and
  concentrate stockpiles. A stocktake is a measurement with an error model and resets the latent-state
  uncertainty.
- **Attestation.** The metallurgical accountant signs the monthly metal balance (`AttestedInterpretation`
  pattern). FrankenMining supplies the reconciliation certificate (§20.9) and never signs.
- **Precision targets.** Per-stream precision is policy data, set by the site's P754 assessment. The system
  reports achieved precision against target, and the gap is a VOI candidate (§20.6).
- **Check-in and check-out stay separate.** MCF (in) and MCF (out) are both exposed; their gap is the plant's
  unaccounted gain (Kibali 2025: check-out 2.6% above check-in; FACT, E210). A tonnage MCF that uses the same
  flowmeter tonnes as plant feed cannot detect flowmeter bias (Loulo; C3 §3.12), and the report says so.
- **Capacity is two numbers.** Nameplate and demonstrated capacity are stored separately (Kibali: design 7.2,
  target 8.2, peak 8.5 Mtpa; E239).
- **Assurance inputs.** A site that applies P754 "without necessarily going through formal training" (Loulo;
  C3 §3.12) has that recorded as an input to metal-accounting assurance scoring, not as a finding against people.

### 17.6 On-site refining and product custody

| Product | Process chain | Custody evidence | Accounting hand-off (§21) |
|---|---|---|---|
| Gold/silver doré | Electrowinning sludge or Merrill–Crowe precipitate → calcining → smelting → bars | Bar weight, fire assay of bar samples (top/bottom drill or pin), seal numbers, vault records, shipment manifests; dual-control sign-offs | Doré inventory (oz Au, oz Ag) → shipment → **refinery outturn** (refined oz credited, typically after weeks) → settlement. Outturn vs mine assay differences are reconciled as a measurement node with bias tracking. |
| Copper concentrate | Flotation → thickening → filtration → stockpile → shipment | Draft survey or weighbridge, **moisture determination**, assay exchange, umpire, splitting limits | Provisional invoice at shipment; quotational-period price adjustments; final invoice after assay exchange. Under IFRS 9 the provisionally priced receivable is measured at FVTPL as a whole (no bifurcation); under US GAAP (ASC 815) the embedded derivative is bifurcated. Bullion computes the fair-value change per the book's policy and FA posts it. |
| Copper cathode | SX-EW | Cathode weights, LME grade sampling | Sales at shipment or warehouse warrant |
| Tin concentrate | Gravity → upgrade → shipment | As for copper concentrate; Sn assay methods are distinct measurands (§13.6) | Smelter terms (payable Sn, deductions, penalties) |

**Safety-semantic refusal FMINE-REF-403 (concentrate shipment).**
- Concentrates can liquefy at sea above their transportable moisture limit (TML) under the IMSBC Code.
- If the moisture determination is missing, stale or above TML at the declared confidence, FrankenMining does not
  produce a "ready to ship" status.
- The release is a human act with a certificate.

**Security.** Bullion and doré custody records are a high-assurance lane:
- dual attestation with HumanAuthorityToken (§31.3);
- an append-only seal and vault log;
- discrepancies above tolerance opening an InvestigationCase with restricted visibility.

### 17.7 Reagents, consumables and wear

- **Consumption models.** Cyanide, lime, xanthates and other collectors, frothers, flocculant, acid (SX-EW and
  POX) and oxygen. Each is a card with feed-dependent inputs (sulphides, cyanide-soluble Cu, acid-consuming
  gangue). Consumption feeds `CostDriverCard`s (§21.6).
- **Cyanide custody.** The cyanide lifecycle follows the International Cyanide Management Code: purchase,
  transport, storage, use, WAD-CN at the tailings discharge, decommissioning. Discharge criteria are policy data.
  FrankenMining records the evidence state and never asserts code compliance.
- **Hazardous by-products.** Mercury from retorting is a `HazardousByproduct(Hg)` material with custody records.
  Veladero stores flasked mercury on site and its removal "is not in the Closure Plan" (FACT, E285); the closure
  completeness check (§18.6) maps every stored hazardous inventory to an ARO line.
- **Grinding media and liners.**
  - Media consumption (kg/t) and liner wear use Archard-type cards (§16.5) with abrasion-index inputs from
    Strata.
  - Measured liner profiles from scanners are observations, and liner change-out timing is a DecisionCard
    priced at the mill-hour dual.
- **Process alternatives are `CapitalCandidate`s (§23).**
  - Examples: glycine leach (Draslovka has piloted it with Barrick, and its February 2026 alliance with Avathon
    targets leach recovery and reagent use; FACT, B5 W3), HPGR retrofits, flotation column additions, coarse
    particle flotation.
  - Each needs testwork evidence with a validity box. A candidate without testwork has evidence class `Assumed`
    and triggers guard G-PUSH-8's banner.

### 17.8 Plant optimization: what Crucible recommends

- **Throughput vs recovery.** Grind modes, flotation residence time and leach residence time are traded at the
  dual prices from Pushback (§15.6). Crucible supplies the (Q, ΔR) frontiers (GB-19).
- **Blend set-points per shift.** Crucible recommends the feed blend within bands, based on the forecast hardness,
  PRI and deleterious elements of the available stockpile bins and faces. These are prepared effects (§31.3).
- **Campaigning.** Refractory ore campaigns versus blending, priced by the LP.
- **No closed-loop control.** Plant MPC and expert systems belong to the DCS. Crucible scores their stated
  benefits.

### 17.9 Guards, proof obligations, gates and kills

| Guard | Condition | Action |
|---|---|---|
| G-CRUC-1 | Card evaluated outside its validity box | FMINE-REF-203, or explicit extrapolation with colour `Estimated` and a priced UU term |
| G-CRUC-2 | Recovery applied at block support | FMINE-REF-102 (support mismatch); must use processing support |
| G-CRUC-3 | Metal balance closes only with an unexplained adjustment | FMINE-REF-101 (no plug); open an InvestigationCase |
| G-CRUC-4 | Concentrate moisture missing, stale or above TML | FMINE-REF-403 |
| G-CRUC-5 | A vendor-twin benefit claim with no registered pre-period forecast | Rendered as an `Assertion` (§7.1), not a measured benefit |
| G-CRUC-6 | A tonnage MCF or check-in/check-out balance uses the same flowmeter/weightometer stream as both numerator and denominator (Loulo; C3 §3.12) | Refuse precision claim (`TautologicalTonnageMcf`); watermark "cannot detect flowmeter bias" |

| PO | Statement | Verification |
|---|---|---|
| PO-CRUC-1 | Flowsheet mass balance conserves solids, water and each element at every node | Property tests on generated flowsheets |
| PO-CRUC-2 | Data reconciliation is unbiased with the stated covariance on synthetic flowsheets with known true flows | Monte Carlo known-answer test (R7 §7.6 test design) |
| PO-CRUC-3 | Gross errors of stated size are detected at stated power and false-alarm rate | Simulation |
| PO-CRUC-4 | Bond and Faraday cards reproduce textbook worked examples exactly | Known answers |
| PO-CRUC-5 | Size-by-size Sn recovery conserves mass across size classes | Property test |

| Gate / kill | Threshold (TARGET) | Fallback |
|---|---|---|
| GATE-CRUC-01 | Pilot plant: 12 months of monthly metallurgical balances reproduced from raw measurements with reconciled values within stated precision; gross-error detection live | Secondary accounting only; flag |
| GATE-CRUC-02 | Registered monthly recovery and throughput forecasts for 12 months; 80% interval coverage in [0.70, 0.90], pooled over both quantities (n = 24: P(pass ∣ 0.8) ≈ 0.80, P(pass ∣ 0.6) ≈ 0.19 before autocorrelation; 12 outcomes alone pass a calibrated forecaster 52% of the time, R7-C120). The twelfth outcome resolves 12 months after plant data first flows (GATE-DATA-03) | Cards stay `Estimated`; planning uses reference-class widening |
| GATE-CRUC-03 | Heap inventory posterior (where a heap exists) narrows as leach-curve data accrue, and P(inventory < book) is reported monthly to Bullion. Known answer: the Veladero 2013–2017 roll-forward reproduces the published leach-pad inventory to ±1 oz (E275) | Point inventory with a documented NRV sensitivity |
| ✖ KILL-CRUC-1 | Plant historian or lab data not accessible by month 9 of a site engagement (§34.4) | Crucible limited to the monthly metallurgical report as an attested observation |

### 17.10 Galaxy-brain cards (illustrative values)

**GB-19 · Push tonnes or chase recovery? (copper flotation concentrator)**
- **Equation.**
  - Q = P/W with W = 10·Wi·(1/√P80 − 1/√F80).
  - Value per hour V = Q·(g·R·net − c_var).
- **Substituted.**
  - Wi = 15 kWh/t, F80 = 2,000 µm and ball-mill power P = 16.8 MW.
  - At P80 = 106 µm: W = 11.22 kWh/t and Q = 1,498 t/h.
  - At P80 = 150 µm: W = 8.89 kWh/t and Q = 1,889 t/h (+26%).
  - Recovery 90% → 87%; g = 0.5% Cu; net $8,000/t payable Cu; c_var = $6/t.
- **Result.**
  - Fine grind: V = $44,939/h. Coarse grind: $54,405/h, which is **+$9,466/h**, or about **$76M/y** at 8,059
    operating hours.
  - The break-even coarse recovery is 74.5%.
  - **But if the mine cannot deliver the extra ore** (mine-limited), coarsening simply loses 3 points of recovery:
    **−$1,798/h**.
- **Intuition.** The right grind is a property of which constraint binds, not of the plant alone. That is
  exactly what the LP's mill-hour and ore-supply duals say.
- **Assumptions.** Ball-mill-limited circuit; flotation residence time and tailings capacity absorb +26%; Bond's
  law holds over the range.
- **Flip.** The ore-supply dual is zero (mine-limited) → fine grind. Flotation residence time binds → recovery
  falls faster than 3 points → re-solve.

**GB-20 · How much gold is really on the heap?**
- **Equation.**
  - Remaining recoverable = T·g·R∞ − recovered to date, with R∞ uncertain from column-to-field scale-up.
  - Leach-curve observations R(t) = R∞(1 − e^(−kt)) narrow R∞ over time.
  - The site's book is Σ T·g·R_card − recovered, a recoverable-basis roll-forward (FACT, E275). The latent is
    Σ T·g·R∞ − recovered − solution and channelling losses. The card reports Δ = latent − book with its interval.
- **Substituted.**
  - 20 Mt stacked at 0.6 g/t is 385,809 oz contained.
  - R∞ = 70% ± 8% (1σ) gives 270,066 ± 30,865 oz recoverable.
  - 200,000 oz have been recovered, so remaining inventory = **70,066 oz, 80% interval [30,510, 109,622]**.
- **Intuition.**
  - The balance sheet carries one number. The truth is a distribution three times wider than its centre suggests.
  - The cost per recoverable ounce, and therefore the NRV test (IAS 2), depend on R∞.
- **Assumptions.** A declared, possibly skewed, scale-up prior (normal here only for illustration); no
  solution-inventory or channelling effects; no grade bias in placed ounces. Production shortfalls under a
  regulatory restriction enter the book as inventory automatically: in Veladero's 2017 cyanide-restriction year
  the book rose 83,900 oz (+33%) while production ran 21.9% below budget (FACT, E278). The restriction is
  therefore a covariate. The physical tail can also sit *above* the book: about 0.5 Moz of residual leach was
  excluded from Veladero's reserves (FACT, E279).
- **Flip.** P10 inventory < ounces implied by the carrying cost at the long-term price → Bullion prepares an NRV
  `Workpaper` for human review (§21.5). Six more months of lift-level leach curves halve σ (HYPOTHESIS, tested on
  the pilot heap).

**GB-41 · Shift-paired recovery vs. residence-time deconvolution.**
- **Equation.** Shift-paired recovery is R̂_k = 1 − g_tail,k / g_feed,k. The tails actually leave on the convolved
  feed (h ∗ g_feed)(t), with h the N-tank kernel of §17.3.
- **Substituted** (illustrative: N = 8, τ̄ = 24 h, true recovery 88% at every grade; feed 3.0 g/t except one 12 h
  shift at 9.0 g/t):
  - an impulse leaves within 12 h with probability 1 − Σ_{j=0}^{7} 4^j e^{−4}/j! = **0.051**, and 86% of it leaves
    between 12 h and 36 h;
  - mean tail grades in the four shifts from the pulse: 0.366, 0.549, 0.714 and 0.504 g/t;
  - shift pairing reports **95.9%** for the high-grade shift (+7.9 points), then **81.7%, 76.2% and 83.2%** (−6.3,
    −11.8 and −4.8 points). A regression of R̂ on feed grade invents a steep grade–recovery slope where true
    recovery is flat.
- **Intuition.** Same-shift pairing ignores a day of inventory in the tanks. Deconvolving by h recovers 88% in every
  shift, including slow shifts where Q drops and τ̄ stretches.
- **Assumptions.** Known tank volume, a slurry flowmeter, and tracer-calibrated α and β.
- **Flip.** At monthly aggregation (Δt ≫ τ̄) with stable month-end inventory the boundary effect is small, and
  steady-state cards suffice.

**Open questions.**
- **OPEN-CRUC-1.** Which plant historian and LIMS exports the pilot site permits.
- **OPEN-CRUC-2.** Whether the site's P754 assessment exists and what its precision targets are.
- **OPEN-CRUC-3.** Whether refinery outturn data (refiner statements) can be ingested electronically.
- **OPEN-CRUC-4.** How to express vendor-twin predictions (Geminex, MetOptima) as registrable forecasts.

## 18. Freeboard: tailings, water and closure

Source: Brenner pass B6 (`planning/brenner/B6_operations_plant_tailings_workforce.md`, sub-question B6-SQ3),
building on R6 §1.8, §2.8 and §6.5, R7 §8, B4's security model and the C2–C5 corpus. Every verdict survived a
refuter-first attempt and a forced third alternative: `H6.3b` (generalized `RequirementSet` traceability engine,
`EsCommitment` + `OB-COMMIT`, three-valued `OB-TARP` where stale/missing sensors or open manual gates escalate to
`Indeterminate`, and environmental certificate reconciliation) and `H6.3d` (coupled physical-financial twin:
undrained excess pore-pressure ratio B̄ = Δu/Δσ_v, Naranjo-style PAG waste
co-disposal storage shadow pricing, and three-measurand closure/ARO completeness with `G-FREE-11`) are adopted over
`H6.3a` (compliance dashboard / green-light checklist, banned by `INV-002` / `G-FREE-1`) and `H6.3c` (autonomous
3D coupled Biot/liquefaction FEM as sole authority, rejected by `NONGOAL-007` and `INV-002`). The crate is
`fmine-tailings`.

### 18.1 Doctrine: evidence state, never "safe"

- **FrankenMining never presents a tailings facility as safe, stable or conformant.**
  `geotechnical_signoff` and `regulatory_compliance_claim` are hard-wired false (§0.4).
- What it presents is the **evidence state** per facility and failure mode:
  - which failure modes are monitored, by what, and with what detection capability;
  - which instruments are stale;
  - when the Engineer of Record (EoR) last reviewed, and the latest Independent Tailings Review Board (ITRB)
    opinion;
  - which GISTM requirements have current evidence.

  This follows R7 §8.4.
- **Missing data is not reassurance.** An offline piezometer gives *no* evidence, not "pressure normal" (FACT,
  R7 §8.4; FMINE-REF-404).
- **InSAR "no acceleration detected" is weak evidence for many failure modes.**
  - InSAR is a screening tool. "Most potential failure modes may not exhibit InSAR-detectable accelerations," and
    line-of-sight geometry limits it (FACT, R7 §8.3, Rana et al. 2024).
  - Its likelihood ratio for "no failure in progress" is therefore set per failure mode, and is near 1 for modes
    it cannot see.
- **The historical lesson is workflow, not sensing.**
  - At Jagersfontein, toe movement was visible in satellite imagery from February 2019, years before the 2022
    failure (FACT, R7 §8.1).
  - Brumadinho needed no specific external trigger (FACT, R7 §8.1, Robertson et al. 2019).
  - So Freeboard's core artifacts are obligations and escalation (`OB-TARP`, `OB-REVIEW`), not dashboards.

### 18.2 GISTM as a requirements-traceability engine

**FACT (R6 §6.5; R7 §8.2):**
- GISTM was launched in August 2020 by ICMM, UNEP and PRI. It has 6 topics, 15 principles and **77
  requirements**, which map to 219 conformance-protocol criteria.
- Its roles are the Accountable Executive, the Responsible Tailings Facility Engineer, the EoR, and an ITRB or
  senior independent reviewer. An ITRB is required for Very High and Extreme consequence classes.
- Consequence classes: Low, Significant, High, Very High, Extreme.
- ICMM members committed to conformance by August 2023 for Very High/Extreme facilities and by August 2025 for
  all others.
- **Barrick (FACT, R6 §1.8):**
  - 64 facilities, 18 of them Extreme (6) or Very High (12).
  - Of those 18: 14 fully conformant, 1 partial, 1 in safe closure, and 2 at Reko Diq for which Barrick "cannot
    disclose conformance to GISTM in 2026".
- **Newmont (FACT, R6 §2.8):** "meeting the majority of GISTM requirements across all tailings storage
  facilities", with 11 priority Very High/Extreme facilities.

**Design (DESIGN).**
- `GistmRequirement` is registry data, versioned by standard edition and conformance protocol.
- For each (facility, requirement), FrankenMining maintains an evidence dossier:
  - documents (design reports, dam safety reviews, EoR inspection reports, ITRB minutes, emergency preparedness
    plans, consequence classification studies, community engagement records);
  - monitoring evidence;
  - the attestations of named role holders;
  - freshness rules, e.g. an annual EoR inspection, or a dam safety review at the frequency the classification
    sets.
- Each dossier cell's knowledge state comes from the FSS lattice (§7.2): `current`, `stale`, `missing`,
  `conflicted`, `not_applicable{attested reason}`.
- **The output** is the facility × requirement matrix with evidence state and the actions due. It renders into
  the annual GISTM disclosure package as a *draft* for the Accountable Executive. That person signs, and the
  signature is recorded as a `DisclosureAct` (§24 pattern).
- **Why this is a strong second-wave module (B5, R6 [INF]).** Both majors disclose against it every August. It is
  bounded, high-trust and requirement-shaped, and it needs no OT write path. B5 placed it in M5 (§37).
- **Consequence class is per facility and may be absent.** Kibali CTSF1/2 are "High" and CTSF3 "Very High"
  (FACT, E216); Kibali's FTSF and the Loulo TSF state no class, and NGM's reports disclose none, only
  conformance claims (FACT, E218, E191). A missing class is stored as NULL and never inferred; it is a data gate
  for the matrix. Design-flood criteria are per facility too (CTSF3: EDF 1:100 24 h 153 mm, IDF 72 h PMP 487 mm,
  1.5 m freeboard; Loulo: 2.7 m for 1:50 72 h; E217, E220).

**Requirement sets beyond GISTM, and the ESIA commitments register (DESIGN; C5 §4.1).**
- `RequirementSet` is a registry kind with variants {GISTM, ESMP, PermitCondition, IFC_PS, EP4,
  LocalContentAgreement}. Each set carries kind, edition, jurisdiction, a `DocLocator` (doc no, revision, page,
  table/row) and `DocApproval` (Draft{rev} or Approved{stamp}). The §18.2 matrix, freshness rules and
  `DisclosureAct` are reused unchanged. Solution-holding heaps and valley-fill leach facilities (§18.3) use
  permit-derived sets: Veladero's requirement set is its permit plus ICMC certification, not GISTM (FACT, E280).
- **`EsCommitment`** = {source locator, verbatim text, `firmness` (Will, Hedged{phrase}, ToBeDefined), mitigated
  impacts, the stakeholder concerns it responds to, standard clauses, owner role, phase, monitoring spec (measurand
  with its chemical basis §25.2, CRS location, frequency, limit and averaging time), trigger (`None` is shown,
  never hidden), reporting and freshness}. `Impact` ratings keep a fixed identity and sign, and a sign change is a
  new Impact (Reko Diq's Social 3 flips sign; E338). A commitment due at a frequency is an `OB-COMMIT`
  obligation (§26).
- Reko Diq's Early Works ESIA is the demonstration source: its ESMP tables are requirement-shaped, and it is an
  unapproved Rev A (FACT, E304), so every cell sourced from it caps at `conflicted` or `stale` (RISK-29).
- **Facility lifecycle.** A facility that is designed but not built is `not_constructed{design_stage}`, separate
  from `venture_wall` and `embargo`. Reko Diq had no construction as of Feb 2024 (FACT, E308).
- **Standards are registry rows** {instrument, clause, jurisdiction, edition, phase applicability, averaging
  time}. Conservation status is a Claim keyed by (list, edition); a renderer shows every list's status, and a
  score that uses one list names it. Reko Diq scored sensitivity on IUCN only, while the national list rates two
  species Critically Endangered (FACT, E345).

### 18.3 The TSF twin of record

| Element | Content | Evidence class |
|---|---|---|
| Geometry | Embankment raises by epoch, beach, pond extent and level, decant structures, spillways, freeboard; L5 anthropogenic body (§13.2) | Survey (Measured) |
| Construction | `TailingsDisposalMethod {Impoundment{raise: Upstream, Centreline, Downstream}, FilteredDryStack, InPit, PasteBackfill, CoDisposalWithWaste, Riverine, Submarine}`; zone materials and their specifications; construction QA records. For riverine disposal the twin of record is the downstream compliance-monitoring network and its exceedance history; there are no embankment failure modes and GISTM's facility framing does not apply (Porgera; FACT, E297) | Attested design + Measured QA |
| Materials | Tailings PSD, density, consolidation and strength (CPTu-derived state parameters), embankment fill | Measured, via cards |
| Instrumentation | Vibrating-wire piezometers (with drift checks), inclinometers, settlement monuments, survey prisms, InSAR, seepage weirs, pond level, rainfall | Packet-plane streams with calibration and staleness monitors (§6.1) |
| Deposition | Deposition plan, spigot positions, rate of rise, water recovery | Plan (DESIGN) + Measured |
| Studies | Consequence classification, dam-break and inundation studies, stability analyses, seismic hazard | Attested external studies, ingested with validity |
| Gates and valves | Manual gate and valve positions on solution and tailings circuits, as attested observations with a freshness bound. A stale or unknown position renders `position unknown`, never "contained". Veladero's 2015 release passed through a diversion-channel gate "that was open at the time" (FACT, E282). FrankenMining has no write path to these devices (§31.3) | Attested observation |

**Solution-holding heap and valley-fill facilities (DESIGN; C4 P4).** The same twin covers them: in-heap solution
level against the permitted limit; leak-collection (LCRS) level and pumped volume as liner-leak indicators; a
seasonal melt forecast registered as P(level > limit); and a permit-derived requirement set. Veladero's Trigger
Limits are PSSA ≤ 3,927 MASL, LCRS ≤ 3,914.7 MASL and LCRS pumping ≤ 270 m³/day, and an exceedance restricts
water make-up and cyanide addition (FACT, E280, E281). The evidence state is rendered, never "within limits" from
stale data (FMINE-REF-404).

**Failure-mode evidence state (DESIGN).** For each credible failure mode, the twin shows:
- the monitored indicators;
- the detection capability: what change, how fast, by which instrument;
- current freshness;
- TARP state, using the TARP automaton shared with Highwall (§14.4);
- the last engineering assessment.

The failure modes covered are overtopping, slope instability, static liquefaction, seismic liquefaction, internal
erosion and piping, foundation failure, and decant or spillway failure.

**Stability and seepage.**
- Limit-equilibrium sections reuse Highwall's LEM solvers (§14.2), with pore pressures from piezometers.
- Liquefaction triggering and post-liquefaction strength come from CPTu state-parameter cards.
- Seepage uses confined Darcy through the `fs-conduction` operator twin (Bridge after GATE-FSIM-RECONV).
  Unconfined and coupled consolidation are gaps (E010; NONGOAL-007). The EoR's own numerical models enter as
  attested results.

**Rate of rise, the pore-pressure ratio B̄ and PAG co-disposal (DESIGN plus FACT).**
- **Pore-pressure response to loading.** For each raise or deposition lift with vertical stress increment
  Δσ_v = γ ΔH, `fmine-tailings` tracks B̄_k(t) = Δu_k / Δσ_v,k at each foundation and slimes piezometer k as a
  registered, scored forecast family, linked to the FoS sensitivity J_u of §14.5. B̄ near 1 means pore water
  carries the load; B̄ falling with time means drainage. A site threshold B̄_crit, set by the EoR from CPTu
  state-parameter screening and laboratory work, raises an `OB-TARP` rate-of-rise obligation. B̄ is an indicator,
  not a liquefaction criterion; triggering analysis stays with the EoR (NONGOAL-007; GB-42).
- **PAG waste inside the TSF (FACT, E667).** At Pueblo Viejo, tailings go to El Llagal until 2030 and then to the
  new Naranjo TSF. From 2028 to 2045 PAG waste rock is placed in a dump inside the Naranjo impoundment and is
  submerged beneath tailings at the end of mining, under at least 10 m of tailings cover; until Naranjo is ready,
  PAG goes to temporary dumps and is rehandled. `fmine-tailings` keeps one storage balance,
  V_tailings(t) + V_PAG(t) + V_water,min(t) ≤ V_permitted(t), whose shadow price per m³ feeds Pushback's pit and
  dump optimisation (§15.6), with the Table 15-5 sustaining-capital rates as the cost prior (E665).

### 18.4 Site water balance

- **Model (`fmine-tailings::water`).** A stochastic site-wide water balance covers:
  - precipitation, evaporation and runoff;
  - pit inflows (§14.5) and dewatering (§16.7);
  - process demand and reclaim;
  - TSF pond, decant and seepage;
  - treatment and discharge.

  Monte Carlo over climate sequences follows the GoldSim-class practice (FACT[U], R7 §8.3).
- **Conservation is enforced** (INV-005): every water node balances per period within measurement tolerance, or the
  period is refused.
- **Climate non-stationarity is a known unknown** (FACT[U], R7 §8.3). Climate scenarios are an index dimension
  (§23.4). The UU term for unprecedented storms is a flip margin, e.g. "freeboard is breached if the wet season
  exceeds X".
- **Outputs** (each a registered forecast):
  - P(pond level exceeds maximum operating level) over horizons;
  - P(discharge exceeds permitted volume or quality);
  - make-up water needs.
- **Discharge criteria** are jurisdictional policy data. FrankenMining reports evidence against them and never
  asserts compliance.
- **Limits are typed by chemical basis and averaging time** (§25.2). Reko Diq's lab reports nitrate as N; the
  ESIA compares the same values as NO3 against ≤ 50 mg/L and reports FS13 as within the limits; on one basis
  FS13 is 59.3 mg/L as NO3 (INFERENCE, E322–E324). Two-week samples are not compared with annual limits.
- **Reported tables reconcile to certificates.** Environmental samples use `LabCertificate` (§12.1). A certificate
  sample with no reported row, an ID that needs an alias map, a date outside the stated campaign window, or a text
  claim that contradicts the cell becomes a discrepancy-register entry. Fixture: the Reko Diq certificate has six
  samples and the ESIA reports five; the unreported sample, a supply bore, is above the ESIA's Cr and Se limits (FACT,
  E325–E328). Lab qualifiers travel with the value.
- **Water-balance nodes for greenfield abstraction.** Abstraction permit vs installed capacity vs demand (Reko Diq:
  10–50 L/s permitted against 140 L/s of listed bores; E312, E313); RO product and RO reject (unstated; E318);
  camp design vs occupancy. Climate inputs are tagged `Transferred` until the on-site record passes a registered
  length (Reko Diq: a commercial modelled dataset and about 2 months on site; E334, E335).
- **DATA-GATE: groundwater TARP.** A TARP for an abstraction field needs piezometer streams and community-well
  baselines. Without them its state is `Indeterminate`, which escalates per §14.4. The Reko Diq ESIA's monitoring
  table records water used and its source (FACT, E333), and institutions and local residents raise depletion
  (E330, E331).

### 18.5 Acid rock drainage and geochemistry

- **Static tests (FACT[U], R7 §8.3; GARD Guide, MEND 1.20.1):**
  - Maximum potential acidity MPA = 30.6 × S% (kg H₂SO₄/t).
  - Acid neutralizing capacity ANC.
  - Net acid producing potential NAPP = MPA − ANC.
  - Neutralization potential ratio NPR = ANC/MPA.
  - NAG pH.
- **Classification thresholds** (e.g. NPR < 1 PAG, 1–2 uncertain, > 2 NAF) are **policy data** from the
  site's management plan, not code constants.
- **Kinetic tests** (humidity cells, columns) are long-run evidence. Prediction uncertainty is large and long-tailed
  in time (FACT[U], R7 §8.3).
- **Block-level P(PAG)** is a Strata projection (§13.6): NAPP per realization from the S and ANC fields, with
  ANC's sparse measurements carried as a field with its own measurement error.
- **Destination classification by expected loss (DESIGN; GB-22).** The cost of placing PAG as NAF (later
  remediation, perpetual treatment) is far larger than the cost of encapsulating NAF. So the expected-loss
  threshold on P(PAG) is low, and many point-estimate NAF blocks are treated as PAG or sent for more testing.
- **Refusal FMINE-REF-402.** No NAF-dump plan is produced for material whose ARD class is `unknown` (§15.5).
- **Leading indicators.** Seep chemistry (pH, sulphate, metals) at monitoring points is a registered forecast
  family, scored against the dump's predicted evolution.
- **Stockpile oxidation is one latent.** The weathering state that degrades a sulphide stockpile's recovery
  (§20.8) also drives its ARD; Pueblo Viejo assumes all stockpiles are PAG (FACT, C4 P3).

### 18.6 Closure, rehabilitation and asset retirement obligations

- **Closure cost estimate.** A stochastic estimate over activities (earthworks, covers, revegetation, water
  treatment, demolition, monitoring, social transition). It uses reference-class priors for closure cost growth
  (`REFCLASS-` records, §9.3) and the ICMM Integrated Mine Closure Good Practice Guide (updated February 2025;
  FACT, R7 §8.3) as the activity taxonomy.
- **Perpetual water treatment is the tail.** Its PV is extremely sensitive to the discount rate and to its
  duration, which is unknown. FrankenMining shows that sensitivity as a flip margin and never hides it inside a
  point estimate.
- **ARO accounting is done by FA (§21.5).** FrankenMining computes the PV inputs: cash-flow profile, inflation,
  the discount-rate basis per IAS 37 or ASC 410-20 (book-specific), and revision drivers. It posts revisions as
  `Adjusting` entries with reason `EstimateChange` and the closure study as evidence (FACT, R4 §5.7: FA has no PV
  or accretion law, so FrankenMining computes and FA records).
- **Progressive rehabilitation** is tracked as completed areas with evidence (surveys, vegetation monitoring).
  It reduces the obligation only through an attested revision.
- **Financial assurance** (bonds, trusts) per jurisdiction is registry data, compared against the stochastic
  estimate's P90.
- **At least three closure numbers exist per site, each a named measurand with its basis label:** the regulator's
  assurance (bond), the operator's estimate (an IFRS/IAS 37 provision only where so labelled), and the stochastic
  estimate. They are rendered side by side, undiscounted and discounted, and never reconciled as one quantity.
  - Bond ÷ operator estimate is 2.7× at Carlin, 2.2× at Turquoise Ridge and 1.5× at Cortez (INFERENCE, E188).
  - Pueblo Viejo: $569M LOM, of which $273M is in the provision ($126M discounted), against $250M of assurance
    (FACT, E271). Kibali carries $41.5M undiscounted and $25.8M discounted (FACT, C3 §3.7).
  - An unlabelled "closure model" cost is never assumed to be the IFRS provision.
- **Closure completeness check.** Every stored hazardous inventory maps to an ARO line (Veladero's mercury does
  not; E285).
- **Tailings capacity runway** is a monitored quantity with permitting lead time: Goldstrike's storage lasts
  "through 2028" while its new TSF is still in permitting (FACT, E190). Pushback consumes it as a constraint
  (§15.6).

### 18.7 Guards, proof obligations, gates and kills

| Guard | Condition | Action |
|---|---|---|
| G-FREE-1 | Any system-authored rendering that would state or imply "safe", "stable" or "conformant" | Refuse; render the evidence state instead. A source document's own wording ("will safely store"; E267) is imported as an attributed `Assertion`, rendered in quotation marks with author and locator, never restated without attribution |
| G-FREE-2 | A stale or offline instrument, or a gate or valve position past its freshness bound, contributes to a "within TARP" or "contained" statement | FMINE-REF-404 |
| G-FREE-3 | A GISTM requirement shown as satisfied without a current, attested evidence item | Refuse; show `missing` or `stale` |
| G-FREE-4 | A water balance period that does not close within tolerance | FMINE-REF-101 |
| G-FREE-5 | ARD class `unknown` and a NAF destination is requested | FMINE-REF-402 |
| G-FREE-6 | Tailings data from a facility under a venture wall or disclosure embargo (e.g. paused project review) | FMINE-REF-302 or 301 |
| G-FREE-7 | A stakeholder `Concern` has no `responds_to` commitment and no attested `not_addressed{reason}` | Show `missing`. Demo: groundwater-depletion concerns from three institutions, absent from the ESIA's consultation summary (E331, E332) |
| G-FREE-8 | A reported (consequence, significance) differs from the registered rating method applied to its own inputs | `conflicted`. Demo: Reko Diq Social 4 residual, reported High/High, recomputes Medium/Medium (E337) |
| G-FREE-9 | A monitored parameter with a limit or receptor has no trigger | Render "no TARP registered"; never "compliant" |
| G-FREE-10 | A commitment with `firmness = ToBeDefined` is past its phase start | `missing` |
| G-FREE-11 | A registered `HazardousByproduct` inventory (e.g. Veladero flasked Hg; E285) or engineered structure has no mapped ARO/closure activity line (`UnmappedClosureInventory`) | Refuse closure-completeness pass; surface unmapped liability on the Closure DecisionCard |

| PO | Statement | Verification |
|---|---|---|
| PO-FREE-1 | TARP evaluation is three-valued, and `Indeterminate` escalates like `Triggered` for notification | Exhaustive automaton test (shared with PO-HIGH-1) |
| PO-FREE-2 | Water balance conserves volume per node and period | Property tests |
| PO-FREE-3 | The GISTM matrix is a pure function of (registry edition, evidence cut, freshness policy) | Replay test |
| PO-FREE-4 | Closure estimate PV is reproducible from cash flows and the declared rate basis to $1 | Known-answer test |

| Gate / kill | Threshold (TARGET) | Fallback |
|---|---|---|
| GATE-FREE-01 | Pilot: one company's full facility × requirement GISTM matrix assembled from its own documents, reviewed by its tailings function, with ≥ 95% of cells resolved to a knowledge state | — |
| GATE-FREE-02 | TARP automaton live on ≥ 1 facility's instrumentation export with staleness monitors; zero silent `Indeterminate → NotTriggered` mappings in a 3-month shadow run | Document-only evidence state |
| GATE-FREE-03 | Water balance forecasts registered for 12 months; 80% interval coverage of pond level in [0.70, 0.90] | Deterministic balance with sensitivity, labelled uncalibrated |
| ✖ KILL-FREE-1 | The client's tailings function declines to let software hold the evidence dossier (accountability concern) | Read-only mirror of their system, with FrankenMining computing only freshness and the TARP evidence state |

### 18.8 Galaxy-brain cards (illustrative values)

**GB-21 · Freeboard exceedance under climate shift (and what a pump buys).**
- **Equation.**
  - The pond exceeds its maximum operating level if wet-season net inflow I > storage S + pump-out capacity C.
  - With I ~ lognormal(median m, σ_ln = 0.35): p = 1 − Φ(ln((S + C)/m)/σ). Over a life of n years,
    1 − (1 − p)ⁿ.
- **Substituted (S = 1.8 Mm³, C = 0.5 Mm³):**
  - Today, m = 0.9 Mm³: p = **0.37%/y (1 in 272)**, which is 5.4% over 15 years.
  - With a climate shift (m + 15%): p = **1.13%/y (1 in 89)**, which is **15.6% over 15 years**.
  - Adding 0.3 Mm³/season of pump capacity restores p = 0.43%/y (6.2% over 15 years).
- **Intuition.**
  - A 15% shift in the median triples the tail probability.
  - Sustaining capital spent on pumping is risk reduction, not an NPV project (§23.2). The DecisionCard prices it
    against the CVaR of an exceedance, not against AISC, where deferral looks free.
- **Assumptions.** Lognormal tail; independent years; storage fixed by the raise schedule. Real tails may be
  heavier, and the UU term is a flip margin on σ.
- **Flip.** σ_ln > 0.45 or a raise delay that shrinks S → re-rank; the TARP for pond level binds first.

**GB-22 · Point-estimate NAF that should be treated as PAG.**
- **Equation.**
  - NAPP = 30.6·S − ANC. Under Gaussian errors, P(PAG) = Φ(E[NAPP]/sd[NAPP]).
  - Treat as PAG when P(PAG) > c_FP/(c_FP + c_FN).
- **Substituted.**
  - Block A: S = 0.8% ± 0.25 and ANC = 20 ± 6 kg/t. NAPP = 4.5 ± 9.7 and NPR = 0.82, so P(PAG) = 0.68 (PAG
    either way).
  - Block B: S = 0.3% gives NAPP = −10.8 and NPR = 2.18, which **classifies as NAF by point estimate**, yet
    P(PAG) = **0.13**.
  - With encapsulation cost c_FP = $2/t and later remediation cost c_FN = $40/t, the threshold is P* = **0.048**,
    so Block B is treated as PAG or tested further.
- **Intuition.** Asymmetric losses plus sparse data mean the "uncertain" class is much larger than the NPR
  thresholds suggest. Sulphur carries a 30.6× multiplier, so at Block B sulphur precision dominates sd[NAPP]
  (7.65 of 9.72 kg/t): repeat S assays are worth more than ANC tests here, and both have quantifiable VOI.
- **Assumptions.** Gaussian errors; independent S and ANC errors (they are often correlated in carbonate-bearing
  units, which a joint field handles); c_FN excludes perpetual treatment (adding it lowers P* further).
- **Flip.** P* = 0.048 needs sd[NAPP] ≤ 6.47 kg/t. ANC testing alone cannot get there (ANC sd = 2 still gives
  P(PAG) = 0.086). S sd ≤ 0.15% together with ANC sd ≤ 4 kg/t gives sd 6.09 and P(PAG) = 0.038 → NAF.

**GB-42 · Pore-pressure ratio B̄ vs. a drained assumption on a raise over contractive slimes.**
- **Equation.** A raise ΔH adds Δσ_v = γΔH and Δu = B̄Δσ_v, so σ′_v = σ′_v0 + (1 − B̄)Δσ_v. In a one-dimensional
  simplification τ_f = σ′_v tan φ′ and FoS = τ_f / τ. After triggering, strength falls to the liquefied ratio
  s_u(liq)/σ′_v0, typically 0.05–0.12 (Olson and Stark 2002).
- **Substituted** (illustrative: σ′_v0 = 275 kPa, φ′ = 28°, τ = 88 + 24 = 112 kPa, ΔH = 5 m, γ = 18 kN/m³, so
  Δσ_v = 90 kPa):
  - drained (B̄ = 0): σ′_v = 365 kPa, τ_f = 194.1 kPa, FoS = **1.73**;
  - measured B̄ = 0.72: Δu = 64.8 kPa (6.6 m of head), σ′_v = 300.2 kPa, FoS = **1.43**, below a 1.50 criterion;
  - if 40% of the slip surface liquefies at s_u = 0.08 × 275 = 22 kPa: FoS = (0.40 × 22 + 0.60 × 159.6) / 112 =
    **0.93**.
- **Intuition.** A drained FoS of 1.73 can sit beside an undrained one below 1 when the raise outpaces drainage, and
  tracking B̄ is what tells them apart.
- **Assumptions.** Contractive state (ψ > −0.05 from CPTu); one-dimensional loading; the 40% liquefied fraction is
  a scenario, not a prediction.
- **Flip.** Slowing the rate of rise until B̄ ≤ 0.25 keeps FoS ≈ 1.63, and the rate-of-rise TARP acts on B̄ before
  total head reaches its static trigger.

**Open questions.**
- **OPEN-FREE-1.** Whether tailings functions will accept FrankenMining as the evidence-dossier system, or only
  as a mirror (KILL-FREE-1).
- **OPEN-FREE-2.** Instrument export formats and staleness semantics at the pilot facility.
- **OPEN-FREE-3.** Which jurisdictional packs (CDA, ANCOLD, national regulations) the first clients need.
- **OPEN-FREE-4.** How Reko Diq-style "cannot disclose" states are represented: a venture-wall or embargo state
  with an attested reason (§31.2). Partial answer: as of Feb 2024 there was no construction and the TSF was
  planned inside the mining lease (FACT, E308), so `not_constructed{design_stage}` may be the right state
  (INFERENCE). The reason must come from Barrick as an attested reason.

## 19. Muster: workforce, rosters, critical controls, productivity and budgets

Source: Brenner pass B6 (`planning/brenner/B6_operations_plant_tailings_workforce.md`, sub-question B6-SQ4),
building on FSS privacy doctrine, R6 §1.5, R7 and W3 data-residency statutes (`E501–E549`). Every verdict survived
a refuter-first attempt and a forced third alternative: `H6.4b` (privacy-by-design pseudonymous crew-level
analytics, hard hours/statutory/competency constraints `G-MUST-3/4`, covariate-adjusted Bayesian partial pooling
`G-MUST-2`, and `G-MUST-6` anti-Goodhart CCV quality monitoring) and `H6.4d` (causal DiD / staggered-rollout
intervention attribution + exact LMDI labour variance bridge `PO-MUST-3` + denominator-typed local-content
commitments) are adopted over `H6.4a` (individual surveillance leaderboard + soft fatigue penalties, banned by
`G-PUSH-1` and `G-MUST-1..5`) and `H6.4c` (unadjusted raw shift-tonnage ratios and raw CCV completion counts,
refuted by geological confounding and Goodhart suppression). The crate is `fmine-workforce`. Muster covers:
- the people who run the mine, and whether the plan is executable with the crews, competencies and hours
  available;
- whether critical controls are verified;
- what labour costs and produces, measured honestly.

### 19.1 Doctrine

- **People data is the most sensitive non-MNPI data in the system.** Muster follows FSS privacy doctrine:
  - purpose limitation and data minimization;
  - a lawful basis recorded per data category;
  - aggregation by default;
  - jurisdiction-specific packs for labour law, works councils and collective agreements;
  - residency rules (§31).
- **Individual-level performance metrics are off by default.** Enabling one requires a recorded lawful basis, a
  policy attestation and, where applicable, works-council or union consultation evidence (guard G-MUST-1).
- **Fatigue and health-adjacent data** (fatigue-detection events, fitness-for-work tests, biometric wearables) is
  a restricted lane with its own access policy. It is never used for discipline through FrankenMining.
- **Safety constraints are hard constraints, never priced** (G-PUSH-1). Hours-of-work limits, statutory
  appointments and competency requirements cannot be traded against dollars in any optimization. GB-23 shows why
  this matters.

### 19.2 Workforce model

| Object | Content |
|---|---|
| `Position` | Seat type (haul truck operator, shovel operator, fitter, process technician, shift supervisor, geotechnical engineer…), site, department, coverage requirement (24/7, day only), statutory flag |
| `Person` (pseudonymous ID by default) | Employment type (employee, contractor, labour hire), home base, residency pattern (FIFO, DIDO, residential), competencies, agreements |
| `Roster` | Swing pattern (e.g. 14/7, 8/6, 4-panel residential), shift length, day/night rotation, panel assignments, valid time |
| `Competency` | Training records, licences and tickets (equipment class, blasting licence, confined space, working at heights), statutory certificates, expiry, assessor |
| `StatutoryAppointment` | Jurisdictional roles (e.g. mine manager, underground manager, quarry manager, ventilation officer, registered EoR where applicable), holder, deputy, valid time |
| `Agreement` | Enterprise or collective agreement rules: allowances, overtime, minimum rest, maximum consecutive shifts, notice periods. These are rule data, not code. |
| `Absence` | Planned leave, sick leave, training, travel days. Aggregated by default. |

### 19.3 Rosters and executability

- **Local content is a commitment with a denominator.** An `OB-REVIEW` local-content commitment declares its
  denominator scope (owner vs contractor employees), its locality definition and its target. Without a target it
  renders as "indicator without commitment". Reko Diq reports 317 of 427 owner staff as local, excludes
  contractors, and monitors a count with no target (FACT, E348).

- **Coverage.** For each position and period, the roster supplies seats × hours. FTE per seat depends on the swing
  pattern, shift length and absence rate (GB-23).
- **Feasibility checks (deterministic):**
  - hours-of-work rules (maximum shift length, minimum rest, maximum consecutive shifts and nights);
  - statutory appointments covered at all times, deputies included;
  - competency coverage per shift: every seat in the plan has a qualified, unexpired person.
- **Plan executability (link to Pushback, §15.8).**
  - A tactical plan's equipment hours imply crew hours per competency. A shortfall is a constraint violation
    surfaced *before* the plan is published.
  - Labour availability is a covariate of schedule compliance.
- **Contractors and campaigns.** Shutdown and maintenance campaigns, development contractors and drilling
  contractors are capacity blocks with their own competencies and rates. Contract terms are documents in the FA
  Docket (§21).
- **Local employment and community commitments.** Local-content percentages and training commitments from
  community or government agreements are constraints with evidence, tracked as obligations (`OB-REVIEW`).

### 19.4 Fatigue risk

- **Biomathematical scoring.** Rosters are scored with a biomathematical fatigue model:
  - the three-process model of alertness (Åkerstedt–Folkard) as the open reference implementation;
  - vendor models (FAID or SAFTE-class) as `UntrustedCandidate` scores where licensed.

  Scores are claims with a validity domain: shift-worker populations, not individuals.
- **Fatigue events from detection systems** (in-cab camera alerts, collision-avoidance interventions) are restricted
  evidence. They are aggregated to roster × shift-hour patterns to test whether a roster change reduced events.
  That test is a registered forecast.
- **Outputs.**
  - Fatigue-risk distributions by roster and shift block.
  - The hours where risk peaks (end of night shifts, first nights after a swap).
  - Mitigations (rotation direction, swing length, break scheduling), proposed as roster alternatives on a
    DecisionCard. Hours limits remain hard constraints.

### 19.5 Critical control management

**FACT[U] (ICMM 2015 Critical Control Management guidance):**
- Material unwanted events (MUEs) have critical controls.
- Controls are verified in the field by supervisors (critical control verification, CCV).
- Verification quality matters as much as completion rate.

**DESIGN.**
- **The control registry.** Each MUE has its critical controls, performance requirements and verification
  questions as registry data. Examples:
  - vehicle interaction;
  - fall from height;
  - ground failure;
  - TSF failure;
  - fire underground;
  - hazardous substances (cyanide);
  - energy isolation;
  - explosives.
- **Evidence state per control instance.** Each `ccv_` record (verifier, time, location, answers, photos,
  non-conformances, actions) updates the control's state to `verified-current`, `verified-stale`, `failed` or
  `not_verified`.
- **Never "safe".** The output is the control evidence state across the site, not a safety score. Freshness
  windows are policy per control.
- **CCV is already a pay metric, so Muster measures its quality as well as its completion.**
  - Critical control verifications are 5% of Barrick's annual incentive, with a 75% threshold and 85% maximum
    (2025 actual 84%). Barrick says it is "continuing to shift toward the use of leading indicators" (FACT, R6
    §1.5).
  - A completion rate shows that checks were done, not how well. Muster therefore reports verification
    *quality* beside completion: answer variance, photo evidence, non-conformances found per verification, and
    re-verification agreement on a random audit sample.
  - "Zero non-conformances found across 300 verifications" is flagged for audit, not celebrated.
- **Leading indicators.** CCV completion and quality, non-conformance closure times, and high-potential incidents
  (HPIs). The hypothesis "failed CCV rates predict HPIs" is registered and tested. Muster does not presume it.
- **Linkages:**
  - Highwall TARP states (§14.4) and Freeboard TARP states (§18.3) appear as critical-control inputs for ground
    failure and TSF MUEs.
  - Ventilation and gas evidence (§16.6) feed underground fire and atmosphere MUEs.
  - Blast-reactive ground refusals (FMINE-REF-401) feed the explosives MUE.
- **Incidents** (HPIs, injuries, near misses) are recorded with investigation outcomes. Causal factors link to the
  controls that failed, which makes the MUE → control → evidence → incident graph queryable.

### 19.6 Productivity, measured honestly

- **Driver-based productivity.** Productivity is measured per process with its natural driver:
  - t/operator-hour (load and haul);
  - metres advanced per crew-shift (development);
  - metres drilled per rig-hour;
  - t milled per plant-hour;
  - work orders closed per fitter-hour, by complexity class.
- **Confounding is explicit.** Productivity depends on geology (hardness, ground conditions), equipment
  availability, haul distance and weather as much as on people.
  - Hierarchical models with these covariates separate *system* effects from *crew* effects.
  - Crew-level estimates are shrunk (partial pooling) and shown with intervals.
  - Crew rankings without intervals are refused (G-MUST-2).
- **Attribution.** Productivity changes after an intervention (a new roster, training, autonomy, a new FMS) are
  estimated with the counterfactual design registered *before* the change. Typical designs are
  difference-in-differences across comparable pits or a staggered rollout.
  - Example: autonomy rollouts at Cortez and Carlin are staggered by site and time (FACT, R6 §1.7), which is a
    natural experiment if the analysis plan is registered before results.
- **Unit labour cost** = labour cost ÷ driver units, consistent with the metric engine (§21.6).

### 19.7 Labour budgets and scheduling

- **Driver-based labour budget:** seats from the plan's equipment and process hours, × FTE per seat from the
  roster, × fully-loaded rates from agreements. Fully-loaded rates include allowances, overtime expectations,
  FIFO travel, camp and payroll taxes.
- **Budgets are forecasts.** They are registered (§10), and the actual-vs-budget variance is decomposed:
  - volume (more seats needed);
  - roster (FTE/seat);
  - rate (agreements, overtime);
  - absence;
  - mix (contractor vs employee).

  The decomposition is computed by the same bridge logic as AISC (§21.6), and the bridge must sum exactly
  (PO-PUSH-10).
- **Labour costs flow to FA** as monthly summaries per cost centre with Merkle-folded detail (§21.3). Payroll is
  run by the payroll system of record, and FrankenMining never computes pay.

### 19.8 Guards, proof obligations and gates

| Guard | Condition | Action |
|---|---|---|
| G-MUST-1 | Individual-level performance metric requested without lawful basis, policy attestation and consultation evidence | FMINE-REF-3xx (authorization) |
| G-MUST-2 | Crew or individual ranking without intervals and covariate adjustment | Refuse the render |
| G-MUST-3 | A published plan needs seats that the roster cannot cover with qualified, unexpired people | Plan marked `infeasible-labour`; shortfall listed |
| G-MUST-4 | Hours-of-work or statutory-appointment rule would be violated | Hard refusal; never priced |
| G-MUST-5 | Restricted-lane fatigue or health data requested outside its policy | FMINE-REF-301-class authorization refusal |
| G-MUST-6 | A CCV campaign reports zero non-conformances over n verifications while an independent random audit gives P(X = 0) = (1 − p)ⁿ < α at its lower credible bound for p, or checklist answer entropy collapses (§19.5, GB-43) | Flag `Conflicted(TickAndFlickSuppressionAlarm)` and open an audit `InvestigationCase`; never render as `verified-current` |

| PO | Statement | Verification |
|---|---|---|
| PO-MUST-1 | Roster feasibility checker agrees with an independent brute-force checker on generated rosters | Property test |
| PO-MUST-2 | FTE-per-seat calculation reproduces hand-computed examples (GB-23) | Known answers |
| PO-MUST-3 | Labour variance bridge sums exactly to the total variance | Property test |
| PO-MUST-4 | Pseudonymization is stable within a tenancy and irreversible without the key held by the privacy officer role | Security test (§31) |

| Gate | Threshold (TARGET) |
|---|---|
| GATE-MUST-01 | Pilot: competency coverage check run against 12 months of rosters and training records; every historical gap found is confirmed by the site |
| GATE-MUST-02 | CCV evidence state live for ≥ 3 MUEs at the pilot with freshness policies attested by the site's safety function |
| GATE-MUST-03 | Privacy impact assessment and works-council review (where applicable) completed before any person-level data is ingested (§34.4) |

### 19.9 Galaxy-brain card (illustrative values)

**GB-23 · FIFO 14/7 vs 8/6: cheaper per seat, but hours and fatigue are not for sale.**
- **Equation.**
  - FTE per seat = 8,760 h ÷ (12 h × 365 × on-fraction) ÷ (1 − absence).
  - Cost per seat = FTE × (salary + trips × flight + on-site nights × camp).
- **Substituted** (absence 12%; salary $150k; flight $1,200; camp $150/night):
  - **8/6:** 2,503 h per person, **3.98 FTE/seat**, 26.1 trips. Cost **$845k per seat per year**.
  - **14/7:** 2,920 h per person, **3.41 FTE/seat**, 17.4 trips. Cost **$707k per seat per year**.
  - Difference: $139k per seat, or about **$55M/y** across 400 seats.
- **Intuition.** Longer swings need fewer people and fewer flights. The saving is large enough that a
  cost-minimizing optimizer will always push toward longer swings and more consecutive nights.
- **That is exactly why** hours-of-work limits, maximum consecutive nights and fatigue thresholds are **hard
  constraints**, not costs (G-MUST-4). The DecisionCard shows the saving *and* the fatigue-risk profile, and a
  human chooses within the legal and agreed envelope.
- **Assumptions.** Same absence and the same *annual* salary across rosters (FIFO allowances often differ);
  flights per swing. 14/7 works 17% more hours, so the headline mostly reflects the salary assumption: at equal
  *hourly* pay ($175k for 14/7) the saving is about **$53k per seat, or ≈ $21M/y**. The DecisionCard shows both.
- **Flip.** Agreement or statute caps swings at 8 days → the 14/7 option disappears. Fatigue-event analysis
  shows night-block risk rising sharply after night 4 → the roster alternative with a mid-swing day/night swap is
  preferred.

**GB-43 · CCV completion vs. a random audit: when "0 non-conformances in 300 checks" signals suppression.**
- **Equation.** An independent random audit of m controls finds k defects, giving a Beta posterior for the true
  defect rate p. If routine CCVs report honestly, X ~ Binomial(n, p) and P(X = 0) = (1 − p)ⁿ. If they detect only
  a fraction q, X ~ Binomial(n, qp), and the likelihood ratio of X = 0 for q₁ against q = 1 is
  E = ((1 − q₁p)/(1 − p))ⁿ. Using the audit's lower credible bound for p keeps E conservative.
- **Substituted** (illustrative: n = 300 routine CCVs, X = 0; audit m = 30, k = 3, p̂ = 10%, Jeffreys 90% interval
  3.7–21.7%):
  - at p = 3.7%: P(X = 0) = **1.2 × 10⁻⁵**, and E against q₁ = 0.1 is **2.7 × 10⁴** ≫ 1/α = 20;
  - at p = 10%: P(X = 0) = 1.9 × 10⁻¹⁴;
  - the 95% upper bound on qp is 1.0%, so q < 10% at the point estimate and q < 27% at the lower bound.
- **Intuition.** A completion count measures n, not the detection fraction q. CCV completion is a scorecard metric
  at Barrick (5 points of the 70% scorecard, threshold 75%, maximum 85%, 2025 actual 84%; FACT, E601), so Muster
  reports q beside n at every site. G-MUST-6 flags a contradiction as `Conflicted(TickAndFlickSuppressionAlarm)`.
- **Assumptions.** The audit samples the same control population and shift mix.
- **Flip.** If an audit of m = 50 also finds no defects, and answer-entropy and photo-timestamp checks pass, the
  streak is corroborated instead of flagged.

**Open questions.**
- **OPEN-MUST-1.** Which HR, training and roster systems export what, under which privacy agreements, at the
  pilot.
- **OPEN-MUST-2.** Whether the company's CCV app data can be exported with verification-quality fields, not just
  completion.
- **OPEN-MUST-3.** Jurisdictions requiring works-council or union consultation before workforce analytics.

## 20. Touchstone: reconciliation, the truth signal

> Reconciliation is the one place where every model in a mine meets reality. FrankenMining treats it as a joint
> state estimate with uncertainty over the material-flow graph, not as a ratio of two estimates. It is the
> wedge's engine (B5 H-f; §37 M1–M2).

Crates:
- `fmine-flow`: the material-flow graph and conservation laws.
- `fmine-recon`: the estimation engine. It uses `fmine-infer` (NUTS, Kalman smoothing) and lifted sparse
  linear algebra.

### 20.1 The material-flow graph (mine to market)

| Node class | Examples | Quantities | Typical measurements (error model from §12.5) |
|---|---|---|---|
| In-situ volumes | Resource-model and GC-model projections over the as-mined solid (§13.7, §16.2) | Tonnes, metal per element (model claims at their supports) | Survey volumes, density field |
| Blasted and dug | Blast polygons shifted for movement (§16.3), dig blocks | Tonnes, metal | Truck counts × payload, bucket counts |
| Transport | Truck loads by destination | Tonnes (wet), assigned polygon probabilities | Payload weighing, FMS cycles |
| ROM pads, stockpiles, heaps | Grade-binned stockpiles, long-term low-grade, leach pads | Tonnes, metal, *latent* inventory | Periodic surveys, reclaim records, leach curves |
| Plant | Crusher feed, mill feed, intermediate streams, concentrate, tails, doré, cathode | Dry tonnes, metal, water | Weightometers, samplers, slurry analysers, stocktakes |
| Products and market | Shipments, refinery outturn, smelter settlement | Payable metal | Draft surveys, assay exchange, outturn statements |
| Cross-complex and third-party sources | Ore from another complex, purchased concentrates, toll feed | Tonnes, metal, owner | Back-allocation only; Carlin plants take Cortez ore and third-party concentrates, and "it is not possible to break out individual sources beyond what is back allocated" (FACT, E157), so source attribution is a §20.6 identifiability question |
| Waste and tailings | Dumps by class (PAG, NAF), TSF | Tonnes, S, metal losses | Surveys, tailings samplers |

- Edges are typed transfers with valid time.
- **Time alignment is explicit:**
  - stockpiles, pads and plant inventories decouple mining time from processing time;
  - the graph is a state-space model over periods, not one static balance;
  - a period's reconciliation is never computed as "mined this month ÷ milled this month" across a stockpile
    (guard G-TOUCH-3).

### 20.2 Conservation laws (INV-005)

- **The law.** For every node, period and extensive quantity q (dry tonnes, water, metal per element):
  Σ in − Σ out − Δ inventory = 0, within the registered measurement tolerance.
- **Reconcile extensive quantities only.** Tonnes and metal are reconciled as extensive variables, which keeps
  the constraints linear. Grades are *derived* (metal ÷ tonnes) after reconciliation and never reconciled
  directly. This mirrors Strata's remap rule (§13.4).
- **Commit refusal.** A period whose reconciled residuals fail the gross-error test is not committed (FMINE-REF-101).
  It becomes an InvestigationCase with the suspect measurements ranked.
- **No plugs.** A survey true-up, stocktake or outturn adjustment is a *measurement* with an error model and
  evidence, never an unexplained balancing figure. "Unaccounted metal" is a posterior quantity with an interval,
  attributed where identifiable (§20.6) and explicitly unattributed where not.
- **Published balancing rows are latent terms.** NGM's "GC adjustment" line sits between theoretical feed (mined −
  feeder-pad change) and GC call: −49, +20 and +25 koz across three plants (INFERENCE, C2 §4.2b). Kibali's "GC
  Adjustment" row carries −10,466 t at **−31.15 g/t** (FACT, E207); Loulo's stockpile-change row carries a
  negative grade (E208). Touchstone models each as an explicit latent term (feeder-pad survey and grade error).
- **Import rule.** Importers store printed reconciliation cells verbatim, recompute every ratio and t·g → oz
  identity, and file mismatches beyond rounding as `DiscrepancyRecord`s citing both pages. A residual row is
  imported as (Δt, Δoz) with grade = `NOT_DEFINED`. Fixtures: Kibali's 2024 MCF row recomputes to
  107.1/94.6/101.2% against a printed 100/96/95% (E206); Kibali Tables 6-2 and 15-27 disagree for the same years
  (E241); Loulo's RCF plant ounces print 683,184 against 753,220 elsewhere (E209).

### 20.3 Reconciliation as joint state estimation

| Level | Method | Purpose | Status |
|---|---|---|---|
| R0 Parity | Reproduce the site's own monthly reconciliation pack (F1/F2/F3, tonnes/grade/metal) from the same inputs and conventions | Trust: "we get your numbers first, and every difference is explained" (B5 M2 acceptance 1) | DESIGN; mandatory |
| R1 Linear data reconciliation | Weighted least squares on the linear constraints: x̂ = x − ΣAᵀ(AΣAᵀ)⁻¹Ax, with posterior covariance Σ − ΣAᵀ(AΣAᵀ)⁻¹AΣ; global χ² and per-measurement tests (FACT[U], R7 §7.6) | Closes balances with honest uncertainty; finds gross errors | DESIGN; P0 |
| R2 Hierarchical Bayesian state-space | Latent inventories over periods (Kalman smoother in the linear-Gaussian case); **bias terms per measurement node and per domain-period** with hierarchical priors; Student-t likelihoods for gross-error robustness; stockpile mixing model (well-mixed vs layered vs FIFO) as a model choice with evidence | F-factors as bias posteriors; learning across periods; attribution | DESIGN; P1, NUTS via `fmine-infer` |
| R3 Model-integrated | Strata realizations at T_resource and T_gc projected over the as-mined volume give *prior predictive* distributions for in-situ nodes; Crucible cards give plant node priors | The posterior is consistent with the belief ledger, so its residuals become Strata's bias priors | DESIGN; P2 |

**Why R2 rather than ratio factors:**
1. F1 and F2 errors can cancel and hide problems ("mutual compensation"; FACT, R7 §4.3).
2. The mill head grade is itself an estimate back-calculated through metallurgical accounting, not truth (FACT, R7
   §4.3).
3. Stockpiles decouple mining from milling in time.
4. Factors applied as fudge multipliers creep (FACT, R7 §4.3).

A state-space posterior handles all four by construction, and it says when the data cannot separate two
explanations.

### 20.4 F-factors as posterior quantities

- **Definitions are registry data**, because variants differ by company (FACT, R7 §4.3; R6 §6.1):
  - F1 = GC model ÷ depleted resource (or reserve) model over the mined volume;
  - F2 = mill received ÷ mine delivered (GC model);
  - F3 = F1 × F2;
  - F0 (reserve ÷ resource), F2′ (post-blast, accounting for swell; FACT, R6 §6.1, AusIMM 2022) and the South
    African mine call factor (MCF) are also defined.

  Each definition is versioned and named in every output.
- **Site-specific definitions registry (FACT; C2 §4.2, C3 §3.1).** At Barrick's own sites the same names mean
  different numerators and denominators:

  | Registry key | Numerator | Denominator | Support | Source |
  |---|---|---|---|---|
  | `F1.barrick_kibali.conversion` | Short-range (GC) model | Long-range model | Areas converted Ind→Meas, year on year (not the mined volume) | E203 |
  | `MCF_out.barrick_africa` | Mill check-out (bullion + inventory change) | GC call | Plant year or month | E204 |
  | `MCF_in.barrick_africa` | Mill check-in (grade reconstituted from CIL and gravity sampling) | GC call | Plant year | E210 |
  | `MCF_t.barrick_loulo` / `MCF_g.barrick_loulo` | Tails tonnes / check-out grade | GC call tonnes / grade | Plant year | E205 |
  | `RCF.barrick_loulo` | Plant check-out | Resource model over the 3D mined volume | Plant year | E209 |
  | `MCF_out.ngm` | Mill check-out | GC call (tonnes, grade, metal) | Plant year | E154–E156 |

  NGM's MCF out sits closer to F2 than to F1, and **no NGM F1 is public** (DATA-GATE). Kibali's "F1" is a
  model-to-model conversion factor, not the plan's F1, so G-TOUCH-4 renders the registry key, never the bare
  label.
- **Each factor is reported as a posterior** for tonnes, grade and metal, with an interval, per domain and period,
  together with the bias decomposition behind it.
- **Compensation detector.** When F3 ≈ 1 while F1 and F2 each deviate in opposite directions beyond their
  intervals, the period is flagged "compensating errors" and opened as an InvestigationCase. The good-looking
  F3 is not reported alone. Reconciling against production "must be used strictly together with" reconciling
  against grade control (E385).
  - The detector also runs **inside one factor**, tonnes against grade: the Goldstrike Roaster's tonnes 109% and
    grade 94% net to metal 102% (FACT, E154).
  - Public case and known answer `KA-TOUCH-KIBALI-2016-2025` (100% basis; Barrick 45%): Kibali's open-pit GC call
    held fewer ounces than the raw GC model in every year (−2% to −13%), with more tonnes and lower grade from
    2022, while the 2025 mill MCF (out) read 100/101/101, with +2.6% check-out over check-in (FACT, E210–E212).
    It supplies ten years to fit a signed bias prior.
- **Tolerance bands.** No published industry benchmark for typical F1/F2/F3 tolerance bands exists; the ±5–10%
  figure is practitioner lore (INFERENCE: a checked null in R6 §6.1). FrankenMining therefore reports intervals, not band compliance.
  Bands, where a company has them, are policy data.

### 20.5 Bias priors back upstream (the learning loop)

| Posterior residual | Updated object | Mechanism |
|---|---|---|
| In-situ metal bias by domain and period (resource vs GC vs mill) | Strata L2 `bias_prior` per domain (§13.3) | Hierarchical update; the next estimate carries the learned bias with its uncertainty |
| Density bias | Strata density field and regression card | Card refit through an InvestigationCase |
| Payload and truck-factor bias | Headframe per-truck payload model (§16.4) | Hierarchical per-truck and fleet bias |
| Sampler and weightometer bias | Crucible measurement node models (§17.5) | Bias term with an `eproc` drift monitor |
| Stockpile survey and bulking bias | Headframe survey-epoch model (§16.2) | Bulking factor posterior |
| Dilution and ore loss beyond plan | Pushback dig-line and blast-movement models (§15.7, §16.3) | Card refit; registered before/after test |
| Recovery card residuals | Crucible cards (§17.3) | Refit through an InvestigationCase |
| Source-specific bias (open pit, underground, stockpile) | Bias priors keyed by mining source | Porgera 2007–2011 shows opposite signs: underground declared ore +36% oz against reserve, open pit −8% (FACT, E298) |

- **Dilution attribution.** Residual dilution and ore loss are attributed to internal, planned, accidental and
  geologic sources (E399; §15.5).
- **Safeguards.** Every update is a logged event with the evidence cut. A surprise monitor alarm (§10.4) routes the
  update through an InvestigationCase; there is no silent retraining (anti-goal, §4.3).
- **The difference from factor multipliers:**
  - a factor multiplier says "multiply next month's GC estimate by 0.93";
  - a bias posterior says "the GC model in domain 12 has over-estimated metal by 7% ± 2% over 9 periods, the bias
    does not depend on bench, and here is the evidence".

  The latter can be audited, tested and retired.

### 20.6 Identifiability, attribution and the value of measurement

- **Identifiability analysis.** Before any attribution, the engine computes posterior correlations among bias
  terms. It also checks the rank of the observation operator restricted to the bias parameters.
- **Refusal FMINE-REF-104.** When two biases are not separately identifiable (posterior |correlation| > 0.8 and
  the split is prior-dominated), the engine refuses to attribute. It reports what *is* determined (their
  difference or sum) and what measurement would separate them (GB-25).
- **Measurement VOI.** For each non-identifiable pair, the engine estimates the posterior-variance reduction and
  decision value of candidate measurements, using the Helm VOI machinery (§22):
  - a sampler audit;
  - a cross-belt analyser campaign;
  - a stockpile resurvey;
  - check-assay round-robins;
  - density programmes;
  - a reconciliation-grade sample of grade-control holes.
- **Detection power is stated.** For each measurement node, the bias size detectable over one quarter at the
  declared power and false-alarm rate is computed and published (B5 M1 acceptance).

### 20.7 Forecast registration and disclosure-grade reconciliation

- **Registered forecasts.** Before each period, the resource-model, GC-model and plant forecasts of tonnes, grade
  and metal for the planned volumes are registered (§10). After the period, they are scored against the
  reconciliation posterior, not against a single measured number.
- **Reporting codes.** The JORC exposure draft proposes a mandatory reconciliation section (final wording and
  commencement unverified; JORC 2012 remains in force; E031). Touchstone
  produces the reconciliation evidence package and a *draft* reconciliation section for the CP's review (§24).
  The CP writes and signs the disclosure.

### 20.8 Stockpiles, heaps and inventory as latent states

- **Stockpile states.** Tonnes and metal per element are posteriors updated by inflows (with their GC grade
  uncertainty), reclaims and surveys. Surveys reset the tonnage uncertainty but not the grade uncertainty.
  - **Parcel provenance (`StockpileVoxelTwin`; DESIGN).** Where FMS dump events and loader GPS exist,
    `fmine-flow::stockpile` keeps a voxel model (2.5–5 m, on the lifted `VdbGrid`). Each dump places a parcel with
    its source polygon and angle-of-repose spreading; each reclaim cut removes voxels and passes their parcel mix
    and age to the crusher feed; each voxel integrates its oxidation exposure θ_ox(v, t) = ∫ r_ox(T, O₂, S²⁻) dτ
    since dumping, which indexes Crucible's degraded-recovery card and Freeboard's ARD source term (§18.5).
  - **Plant cone and surge inventory.** The cone between crushers and mills is a latent inventory, not a constant.
    Pueblo Viejo's 2025 MCF reconciliation opens with 281.7 kt at 2.41 g/t and closes with 352.1 kt at 2.44 g/t
    (FACT, E666), a 70 kt swing that the monthly balance must carry.
  - **Mixing model.** Without dump and reclaim positions, stockpile grade at reclaim depends on a declared mixing
    model (well-mixed, layered/FIFO, or campaign-separated). The reconciliation compares the candidates'
    predictive performance, and the voxel model is one more candidate where its data exist.
- **Stockpile tracking practice (FACT, E194).** Cortez records each truck's majority block ID, grade and
  constituents, corrects tonnage to survey, scores each pile annually, and sonic-drills piles on a 15–18 m grid.
  These are the observations the stockpile state consumes; sonic-drill intercepts also condition voxel grade and
  oxidation state.
- **Stockpile metallurgical state (DESIGN; C4 P3).** Per pile, lift or voxel, carry Stot, S²⁻, Ctot/Corg and
  exposure age as posteriors, with a declared degradation model. Crucible's recovery card is indexed by that
  state, and the same
  oxidation latent feeds ARD (§18.5). Pueblo Viejo: stockpiles hold 6.0 of 20 Moz of reserves and supplied 70.6%
  of 2025 crusher feed; recovery falls to 74% for one ore type above 8.5% Stot; the stockpiles are Probable only
  because sulphur degradation affects recovery (FACT, E252–E255, E258). The §24.4 support pack shows the
  degradation uncertainty; the class remains the QP's call.
- **Heap leach and gold-in-circuit inventories** are latent states with leach-curve dynamics (§17.3, GB-20). A
  multi-lift pad (Veladero, Cortez, Carlin, Zaldívar) is discretized by lift ℓ and irrigation panel c (DESIGN):
  ṁ_PLS(t) = Σ_{ℓ,c} M_ℓc · R∞(domain_ℓc, P80_ℓc) · h_ℓc(t − t_irrig,ℓc; Z_ℓ, q_ℓc(t), κ), with Z_ℓ the drainage
  depth through the lifts below, q_ℓc the application flux (L/h/m²) and κ a channelling parameter. PLS flow and
  grade update the posterior over (κ, R∞) and the recoverable inventory per cell. The site book is usually a
  recoverable-basis roll-forward (E275), stored as `MetalStage::Recoverable` (§25.2), and Touchstone reconciles
  the cell posterior against it every month.
- **Accounting hand-off (§21.5).** Inventory posteriors feed FA through Bullion:
  - quantity and cost layers;
  - NRV tests at a price basis set per inventory class by policy (expected selling price for doré, concentrate
    and metal in circuit; long-term price only where the book's policy allows it for long-term stockpiles); a
    degrading long-term stockpile is valued at the recovery expected at its scheduled reclaim date, not today's;
  - current vs non-current classification of long-term stockpiles.

  A true-up is posted as an `Adjusting` entry with the survey or reconciliation `Workpaper` as evidence (FACT, R4
  §7: FA records, FrankenMining computes).

### 20.9 The reconciliation certificate

Every committed period produces a `ReconciliationCertificate`. It extends the `ReconciliationWitness` of §13.11:

| Field | Content |
|---|---|
| Inputs | Evidence cut; measurement set with error-model versions; F-factor definition versions |
| Parity | R0 differences from the site's pack, each explained or refused |
| Balance | Pre- and post-reconciliation residuals per node; χ² statistics; gross errors detected and their disposition |
| Posteriors | Node quantities, F-factors, bias terms (mean, interval), latent inventories |
| Identifiability | Pairs refused attribution, with the separating measurement and its VOI |
| Forecast scores | Registered forecasts for the period and their scores |
| Attestation | The site reconciler or metallurgical accountant signs (`Reviewed{against}`); FrankenMining never signs |
| Determinism | Replay digest (same inputs and seed ⇒ same certificate digest) |

### 20.10 Synthetic truth, known answers and gates

The synthetic-truth generator (`fmine-gen`, §33.3) builds open-pit Au and Cu–Au mines with **seeded biases** in:
- the resource model;
- dilution and ore loss;
- stockpile survey and bulking;
- mill sampler;
- density;
- truck payload.

The generator produces every measurement the real graph has, with known error models.

| Gate / kill | Threshold (TARGET) | When | Fallback |
|---|---|---|---|
| GATE-TOUCH-01 (R1 known answers) | Linear reconciliation unbiased with the stated covariance on synthetic flowsheets; gross errors of stated size detected at stated power | M1 | — (must pass) |
| ✖ GATE-TOUCH-02 (bias recovery) | Over ≥ 200 replicates, 90% intervals on each injected bias cover truth **85–95%** of the time; the metal balance closes within stated uncertainty; non-identifiable pairs return FMINE-REF-104, not a number; detection power for a 5% sampler bias over one quarter stated | M1 (B5) | **Kill if coverage is outside 80–97% after one fix cycle, or identifiability is not characterised.** Fallback: no site proposal on reconciliation; lead with the metric engine (§21.6) and capital (§23) |
| GATE-TOUCH-03 (parity) | Site pilot: the site's F1/F2/F3 reproduced from the same inputs, every difference explained | M2 | Parity-only deliverable |
| ✖ GATE-TOUCH-04 (material finding) | ≥ 1 material finding (pre-registered as ≥ 1% of contained metal or ≥ a $M/yr threshold set with the sponsor) attested `Reviewed{against}` by the site QP or metallurgist as new and plausible | M2 (month 12) | B5 F8 "checked null": a well-reconciling site yields a certified clean bill, the second pilot site is engaged, and the program gate at month 12 weighs it (§37) |
| GATE-TOUCH-05 (hindcast) | 8–12 historical quarters run rolling-origin; coverage reported with a small-sample caveat | M2 | — |
| GATE-TOUCH-06 (public known answers) | The §20.4 registry reproduces the published Kibali 2016–2025 MCF series, the Pueblo Viejo 2025 stock-and-flow table (stockpile additions inferred at about 4,560 kt; E258, E259) and the Porgera 2007–2011 source tables, with every irreproducible cell filed as a `DiscrepancyRecord` | M1 | Fixtures only |

### 20.11 Guards and proof obligations

| Guard | Condition | Action |
|---|---|---|
| G-TOUCH-1 | Grade reconciled directly instead of tonnes and metal | FMINE-REF-103 |
| G-TOUCH-2 | Attribution requested for a non-identifiable pair | FMINE-REF-104 |
| G-TOUCH-3 | Period ratio computed across a stockpile without the latent-inventory model | Refuse; require the state-space form |
| G-TOUCH-4 | An F-factor rendered without its definition version and interval | Refuse the render |
| G-TOUCH-5 | A factor applied as a multiplier to a future estimate | Refuse; require a bias prior through Strata (§20.5) |
| G-TOUCH-6 | A grade is imported or rendered on a residual (difference) row | Store (Δt, Δoz) with grade `NOT_DEFINED`; file a `DiscrepancyRecord` |
| G-TOUCH-CAVE | Block-cave draw-point samples used as the grade-control term of F1 for reserve verification | Refuse; they are admissible only as observations of cave-draw grade (E386) |

| PO | Statement | Verification |
|---|---|---|
| PO-TOUCH-1 | R1 reconciled estimates satisfy every constraint exactly (to 1e-9 relative) | Property test |
| PO-TOUCH-2 | R1 posterior covariance matches Monte Carlo on synthetic flowsheets | Statistical test |
| PO-TOUCH-3 | The R2 Kalman smoother equals the batch Gaussian posterior on linear-Gaussian cases | Known-answer test |
| PO-TOUCH-4 | NUTS posteriors match conjugate known answers within Monte Carlo error; R̂ < 1.01 and ESS above the threshold are required for publication | Diagnostic gate |
| PO-TOUCH-5 | Bias recovery coverage (GATE-TOUCH-02) | Synthetic replicates |
| PO-TOUCH-6 | Certificate digest is replay-stable | LabRuntime replay |

### 20.12 Galaxy-brain cards (illustrative values)

**GB-24 · One month of tonnes, reconciled.**
- **Measurements.**
  - Truck tonnes: 1,000,000 t, σ = 3%.
  - Mill weightometer: 960,000 t, σ = 1%.
  - Stockpile change from survey: +25,000 t, σ = 15,000 t.
- **Imbalance.** r = 1,000,000 − 960,000 − 25,000 = 15,000 t.
- **Equation.** x̂ = x − ΣAᵀ(AΣAᵀ)⁻¹r, with AΣAᵀ = 1.217×10⁹.
- **Reconciled.**
  - Trucks 988,909 t (**−11,091**).
  - Mill 961,136 t (+1,136).
  - Stockpile +27,773 t (+2,773).
  - χ² = 0.185 (p ≈ 0.67): no gross error.
- **Metal.**
  - Mine-claimed 988,909 t × 1.20 g/t = 38,153 oz.
  - Mill-received 961,136 t × 1.10 g/t = 33,991 oz.
  - Stockpiled 536 oz.
  - Unexplained 3,626 oz (**9.5%**), so F2(metal) = 0.904.
- **Intuition.** The least precise measurement (truck counts) absorbs most of the tonnage correction. Over many
  months that becomes a payload-bias posterior. The 9.5% metal gap is the real question, and GB-25 asks who owns
  it.
- **Assumptions.** Independent Gaussian errors; one period; the stockpile grade is known (it is not; R2 makes it
  a latent state).
- **Flip.** A χ² p-value < 0.01 → gross-error InvestigationCase before any F-factor is published.

**GB-25 · Whose fault is the missing 9.5%? (identifiability and the value of an audit)**
- **Model.**
  - The observed F2 log-ratio y = b_gc − b_mill + ε = −9.6%, with σ_ε = 1.5%.
  - Priors: b_gc and b_mill ~ N(0, 5%) each.
- **Posterior from F2 alone.**
  - b_gc − b_mill = −9.6% ± 1.5% (determined).
  - Individually, b_gc = −4.6% ± 3.6% and b_mill = +4.6% ± 3.6%, with **correlation 0.92**.
  - The even split simply mirrors the equal priors, so attribution is **refused** (FMINE-REF-104).
- **After a mill-sampler audit.** An audit measures b_mill = +1.0% ± 2.0%. Then:
  - b_gc = **−7.1% ± 2.2%** (95% interval excludes 0);
  - b_mill = +1.8% ± 1.8%.
- **Intuition.**
  - Without an independent measurement, any "it's the grade-control model" story is a prior dressed as a finding.
  - One audit turns a ratio into an attributable, ≥ 1%-of-metal finding: the kind GATE-TOUCH-04 requires.
- **Assumptions.** Linear-Gaussian; no stockpile or density terms (R2 includes them, which widens the posterior).
- **Flip.** The audit shows b_mill ≈ −9% → the mill sampler owns the gap, and the GC model is exonerated. Either
  way, the decision is which party fixes what, and that is a DecisionCard.

**Open questions.**
- **OPEN-TOUCH-1.** The pilot site's F-factor definitions and pack conventions (R0 parity depends on them).
- **OPEN-TOUCH-2.** Whether the GC model is produced in other software and must be ingested as an observation
  (OPEN-STRATA-3).
- **OPEN-TOUCH-3.** Stockpile mixing-model evidence at the pilot (reclaim sampling).
- **OPEN-TOUCH-4.** Whether the CP will accept a reconciliation-section draft for JORC 2026-style disclosure.

## 21. Bullion: accounting through franken_accounting, and the metric engine

Crates:
- `fmine-metalacct`: the metal and inventory subledger;
- `fmine-acctpack`: the mining domain pack;
- `fmine-metrics`: the metric engine;
- `fmine-fa-bridge`: the only FA client.

Sources: R4 (the FA fit, read at FA `53c6e953`), B4 §5 (bridge design) and B3 §5.3 (AISC consistency).

### 21.1 Doctrine

1. **FA is FrankenMining's only book of record.** FrankenMining never computes an authoritative balance.
   Balances are FA folds, read through certified reads and cited by certificate (FACT, R4 §7.1). At a company
   that already runs an ERP, the ERP stays the statutory ledger, and FA is a reconciling analytical book that must
   agree with the ERP's trial balance; every difference is a cited register item, and Workpapers support entries
   the controllers post in their own ERP (DESIGN, §1.5).
2. **FrankenMining owns operational truth and estimates; FA owns the mechanical record.** Every FrankenMining
   estimate that moves money enters FA as a certified computation document (`Workpaper`) on the entry it supports.
   This covers NRV, depletion rates, ARO PV, impairment, MTM, stream drawdown, royalties and survey true-ups.
3. **No second posting universe.** Physical and inventory detail lives in FrankenMining and enters FA only as
   reviewed summaries (FA-INV-12 external-subledger form).
4. **Judgment stays with humans.** FA "will never decide GAAP treatment" (FACT, R4 §10). Every policy choice is a
   named, versioned FrankenMining policy object with an approver:
   - E&E capitalization;
   - stripping components;
   - NRV reversal policy;
   - CGU definition;
   - the sustaining vs growth classification.
5. **Agents prepare; humans authorize** (§30.4).

### 21.2 FA facts that shape this design (FACT, R4 at FA `53c6e953`)

| Fact | Consequence |
|---|---|
| About 793K lines of Rust at `53c6e953` (≈ 796K at HEAD `41363acb`); FA's own reality check records a large share as PARTIAL, UNPROVEN or NOT_STARTED, including cross-book consolidation with CTA, the CloseRun DAG and operability at scale | Treat FA as an evolving dependency: pin the rev, run FrankenMining's own acceptance suite (§21.3), track V27/V37/V38 |
| Single writer per book; per-post latency measured at 118 ms rising to 439 ms over 300 posts in a debug build | Post summaries, never transactions (GB from B4 G1, §21.7) |
| No metal or mass units registered | Interim money-only legs; metal quantities live in Workpaper rows and the FrankenMining flow ledger (§21.3.6) |
| Canon cannot multiply variables, divide or use tables; outputs only Original or Adjusting | Canon rules are **account determination only**. FrankenMining computes every amount (R4 §7.6). |
| Eight fixed dimension kinds plus `legal_entity` and `party`; no custom kinds | Mapping in §21.4; never repurpose `fund` or `grant` (they drive fund-balance closing) |
| Eight implemented REA event kinds; no `Custom(id)` | `PostEntry`/`PostBatch` for most mining events; `RecordEvent` only for `Shipment`/`Sale` |
| FrankenMining-generated evidence grades `Generated` and is self-sourced | Every entry cites a third-party or connector document **and** a human review (§21.3.5) |
| SoD binds asserted names; approvals unsigned (V19) | FA SoD evidence is labelled HYPOTHESIS-grade in FrankenMining control reports until V19 closes |
| FA has no plugin or domain-pack concept | The domain pack lives entirely in FrankenMining (§21.5). Registry rows are upstream asks (Appendix F). |

### 21.3 The bridge (`fmine-fa-bridge`; B4 §5)

**21.3.1 Transport.**
- **Primary path:** `fad` hosts each book (writer lease) at the enterprise finance node. `fmine-fa-bridge` calls
  `fa --json` robot verbs as a subprocess, using FA's NDJSON envelope, exit codes 0 and 2–8, and refusal objects.
- **Fallback:** a **signed intent outbox**. FrankenMining writes canonical intent files, and a finance operator or
  scheduled job feeds them to `fa post --intent … --dry-run` / `fa commit --plan …`.
  - The intent bytes and idempotency keys are identical on both paths, so switching paths cannot double-post.
- **Agents** use `fa-mcp` on drafts.
- **Embedded Rust** (`fa::Book`) is used only in `tools/fa-conformance`. FA's toolchain and pins (nightly, its
  asupersync and fgdb revs) are not imposed on FrankenMining (R4 §7.11).

**21.3.2 Book topology.**
- One FA book per (functional-currency group × primary basis), with `legal_entity` as the balancing dimension.
- Secondary-basis and local statutory books run as *full* books until `AdjustmentLayer` is surfaced (V27).
- A management book carries AISC and cost-category tagging.
- When the company is JV operator, each JV entity has its own book, and partners receive `.fab` bundles verifiable
  with `fa-verify`. When the company is a non-operating partner, the operator's certified statements are imported
  as evidence.
- `registries/books.toml` maps (site, legal entity) → books.

**21.3.3 Granularity (DESIGN).**
- **One external-subledger summary per (site, flow family, month, book).** The flow families are:
  - `ore`: ROM and stockpiles;
  - `wip`: leach pads and circuit;
  - `fg`: doré, concentrate, cathode;
  - `cost`: absorption.
- Close entries (DD&A, ARO accretion, NRV, MTM, royalties) are one per site-month-book. Sales are per shipment
  or invoice.
- **Truck-, assay- and shift-level postings are prohibited.** They fail on writer throughput and on the human
  review that FA-INV-12 requires (§21.7).

**21.3.4 Merkle-folded detail and keys (DESIGN).**
- Each (account, side, dimension tuple) aggregates to one external detail row. The row's `source_digest` is the
  MMR root of the FrankenMining ledger slice that folds to it, so an auditor can descend from an FA leg to every
  daily movement and source certificate by digest.
- Limits: ≤ 128 rows, ≤ 32 legs and ≤ 8 KiB per entry. Overflow splits deterministically by `cost_center`, then
  by `product`.
- **Idempotency key grammar:** `fmine1:<book>:<slot_kind>:<site>:<family|step>:<period>:g<gen>`, for example
  `fmine1:BK-USD-IFRS:extsub:SITE-NV01:ore:2027-03:g0`.
  - B4 drafted `fm1:`. The `fmine1:` prefix follows the crate-prefix decision (§28.1; OPEN-B4-09).
  - `gen` increments only through a supersession record.
  - FA's identity rule is vid(Entry) = H(book ‖ principal_scope ‖ idempotency_key).
- **The posting-slot automaton** (Petri invariant: one token per slot, at most one live `Original`):

```text
Planned ─prepare─▶ Prepared{workpaper_digest, payload}
  ─import─▶ Imported{doc_oid}            (ImportReceipt Connector{franken_mining/metalacct, v})
  ─review─▶ Reviewed{attestation_vid}    (independent human principal; OB-REVIEW resolves)
  ─dry-run─▶ Planned@FA{plan_digest}     (fa post --intent --dry-run --plan-out)
  ─commit─▶ Committed{EntryRef, cert}    terminal-live
            │exit 3/7 ─▶ Refused{FA-REF-nnn}  (human triage; never auto-modify)
            │exit 6   ─▶ Replan | ALARM (IDEMPOTENCY_CONFLICT ⇒ FrankenMining bug)
            │timeout  ─▶ Indeterminate ─reconcile by key─▶ Committed | Planned
Committed ─recompute differs─▶ Drift ─▶ Replace (open period: Reversal + Replacement, g+1)
                                    └▶ Adjusting EstimateChange (closed period, new slot)
```

**21.3.5 The evidence grading triple (B4 §5.5).** Every summary entry cites three things:
1. the FrankenMining `Workpaper` (grade `Generated`);
2. at least one non-self-sourced document: the survey report, lab batch certificates, weighbridge totals, the
   invoice;
3. the independent human `Reviewed{against: workpaper_digest}` attestation.

FrankenMining service principals use `Production`/`Preparer` profiles with caveats (`entities ⊆ site`,
`accounts ⊆ subtree`, `amount ≤`, `requires_dry_run`). They never use the `Agent` profile on trunk.

**21.3.6 Interim postures while FA's P0 asks are pending (rows `FD-FA-*`).** Each interim posture is correct today
and stays correct afterwards:

| Ask (Appendix F) | Interim posture | Switch-over rule |
|---|---|---|
| Metal and mass units | Money-only legs; metal in Workpaper rows and the flow ledger | From the first *open* period after FA ships balanced `XAU_OZ`/`CU_LB`/… rows. No back-posting. |
| Custom dimension kinds | Chart-leaf encoding plus the eight-kind map | New dimension values apply prospectively |
| Custom REA event kinds | `PostEntry`/`PostBatch`; `RecordEvent` only for `Shipment`/`Sale` | Canon account-determination rules after `Custom(id)` |
| Tiered NCI and cross-book consolidation | FrankenMining's JV engine computes attributable figures citing FA certificates; FA `consolidate` only for non-tiered groups | When fa-76hy lands, FrankenMining's computation becomes an oracle cross-check |

**21.3.7 Acceptance suite (`INT-FA-001` = all pass; run against the pinned FA rev).**

| ID | Test | Pass |
|---|---|---|
| FA-ACC-01 | Robot envelope and exit dictionary (0, 2–8) for every verb used | Schema matches; exit 1 never emitted |
| FA-ACC-02 | Identical `PostBatch` twice | `already_recorded = true`, same `EntryRef`s |
| FA-ACC-03 | Same key, different members | Exit 6; nothing posted |
| FA-ACC-04 | External Workpaper with 128 rows, with 129 rows, and with a non-folding summary | Accept, refuse, refuse |
| FA-ACC-05 | Generated-only evidence under the Agent profile on trunk | Refused |
| FA-ACC-06 | Original into a Closed period; Adjusting with approval | Exit 7 `FA-REF-010`; accepted |
| FA-ACC-07 | `fa replay` of the certified trial balance for FrankenMining-posted months | Byte-identical |
| FA-ACC-08 | `fa bundle produce` → `fa-verify`; one-byte tamper | Exit 0; exit 8 |
| FA-ACC-09 | Release build, book seeded to 10⁵ entries; 300 summaries | p99 ≤ 2 s/post and slope ≤ 0.02 ms per entry (TARGET) |
| FA-ACC-10 | After unit rows exist: an unbalanced metal leg | `FA-REF-001` |
| FA-ACC-11 | A second `fad` on the same book | `FA-REF-018`, exit 6; the bridge degrades to the outbox |
| FA-ACC-12 | Kill `fad` between dry-run and commit, over 1,000 LabRuntime seeds | No duplicate; plan invalidated → replan |
| FA-ACC-13 | `PostBatch` whose members are external-subledger summaries | Accept, or document per-entry posting (OPEN-B4-03) |

✖ **KILL-1 (FA scale and fit; the single definition, cited by §37.3).** Two triggers, two tiers of fallback:
- **(a) Scale, month 9.** FA-ACC-09 fails at the FA rev pinned by month 9 (p99 > 10 s at 10⁵ entries) →
  post monthly only, at ≤ 300 publications per book per month; keep quantities authoritative in the
  FrankenMining flow ledger; file the upstream bead.
- **(b) Fit, month 15 (the M3 deadline).** FA still lacks metal units (FD-FA-01) or still fails (a) → a
  FrankenMining-local certified ledger with FA as a draft adapter. No executive deliverable slips.

**Never fork FA, never build a GL.**

### 21.4 Chart and dimensions (FACT inputs from R4 §7.3; DESIGN mapping)

- **Mining chart spec.** FrankenMining ships IFRS and US GAAP variants, applied by `OpenAccount` schema-control
  intents. Leaf families:
  - ore stockpiles (current and non-current);
  - heap leach WIP, metal in circuit, doré/bullion, concentrate, cathode;
  - materials and supplies;
  - mineral properties (acquired, development, producing);
  - stripping activity assets per component;
  - E&E (IFRS book);
  - CIP split into sustaining and growth leaves;
  - ARO asset and provision (operating and non-operating sites);
  - stream deferred revenue;
  - provisional-pricing receivable and adjustments;
  - royalties and mining taxes payable;
  - hedge assets and liabilities, and the CFH reserve;
  - NCI;
  - statistical production and reserve accounts.

| FA dimension | FrankenMining meaning |
|---|---|
| `legal_entity` (balancing) | Legal entity |
| `party` | Smelters, offtakers, refiners, JV partners, streamers, royalty holders, governments, suppliers |
| `region` | Operation hierarchy: country > complex > mine > pit/underground |
| `segment` | IFRS 8 / ASC 280 segment; CGU or asset group |
| `cost_center` | Process stage or department: mining, processing (mill, leach, flotation), refining, site G&A, corporate |
| `project` | Capital project/AFE, stripping component, exploration project |
| `product` | Au, Ag, Cu, Sn; doré, concentrate, cathode |
| `department` | Spare, optionally the WGC cost category |

### 21.5 The mining domain pack (`fmine-acctpack`; the 17 parts from R4 §8)

FrankenMining owns all 17 parts. Phases refer to §37.

| # | Part | FrankenMining computes | FA records via | Phase |
|---|---|---|---|---|
| 1 | Metal and inventory subledger (ROM, stockpiles, heap WIP, metal in circuit, doré, concentrate, cathode, supplies) with survey true-ups and NRV tests | Quantities, costs, true-ups from Touchstone posteriors (§20.8); NRV at the per-class policy price basis | External-subledger summaries; `Adjusting` `EstimateChange` true-ups; NRV `PostEntry` per book | M3 |
| 2 | Production cost engine (process costing, absorption, by-product vs co-product, relative-sales-value drivers) | Cost pools from `CostDriverCard`s (§21.6) | `Allocate`, `Reclass` | M3 |
| 3 | Deferred stripping (IFRIC 20 components; US GAAP variable-cost treatment) | Stripping ratios per component from Pushback plans and actuals | `Reclass`/`PostEntry` per book | M4 |
| 4 | Mineral property register and DD&A (units of production from reserves, prospective revisions) | UoP rates from the Hallmark register (§24) as at the effective date | `Adjusting` rule with amount input; later `Depreciate` | M3 |
| 5 | E&E policy (IFRS 6 capitalization vs US GAAP expense; triggers; transfer to development) | Policy application per project from Lodestar status (§11) | Per-book entries | M4 |
| 6 | Closure and ARO (inflation, discount bases, accretion, revisions, settlement) | Freeboard closure estimates (§18.6) | `Accrue`/`PostEntry`; revisions `EstimateChange` | M5 |
| 7 | Impairment (CGU registry, LOM and price-deck models, FVLCD/VIU, ASC 360 two-step) | LOM cash flows over the scenario index (§23.4) with FA scenarios for sensitivities | `PostEntry` after scenario rehearsal | M4 |
| 8 | Concentrate and bullion sales (payables, TC/RC, penalties, quotational period, MTM, provisional and final invoices, assay exchange, outturn) | Settlement engine from Crucible custody (§17.6) | `RecordEvent(Shipment/Sale)`, `RecordInvoice`, `ApplyReceipt` plus explicit adjustment legs | M3 |
| 9 | Streams and royalties (deposits, drawdown, financing component, catch-ups; NSR/NPI/gross royalties) | Contract terms as policy objects; price-linked amounts on the scenario index | `PostEntry`/`Accrue` | M4 |
| 10 | Hedge accounting (designation, effectiveness, valuation, OCI recycling) | Valuations with counterparty confirmations | `PostEntry`/`Reclass` | M5 |
| 11 | Mining taxes and government royalties, per jurisdiction | Jurisdiction packs | `Accrue` | M4 |
| 12 | JV and joint arrangements (IFRS 11, operator billing, proportionate consolidation, equity method) | Attributable figures; billing | Native FA subledgers; `RecordInvoice`/`RecordBill` | M3 |
| 13 | Capital project accounting (AFE, CIP, sustaining vs growth, capitalized borrowing costs, pre-production proceeds) | WGC-rule classification (§21.6) with an attested decision per project | `project` dimension; CIP leaves | M3 |
| 14 | Non-GAAP metrics (AISC, AIC, cash costs, C1, GEO, attributable vs consolidated) | The metric engine (§21.6) | Certified reads of FA rows; reconciliation schedules as `StatementDefinition`s | **M0 (public lane)**, M3 (private) |
| 15 | Reserves and resources register | Hallmark (§24) | `ImportDocument(kind Disclosure)` plus QP `Attest` | M3 |
| 16 | Chart spec, dimension map, Canon account-determination rules, `StatementDefinition`s | Shipped as data | Schema-control intents; `PolicyControl` activation with dual control | M3 |
| 17 | Mining close orchestrator and mining control procedures (metal-balance variance, F1/F2/F3, stockpile variance, capitalization tests) | §21.7 | Results filed as evidence over certified reads | M3 |

**NRV workflow (example of the pattern).**
1. Touchstone's inventory posterior (§20.8) and Crucible's heap inventory (GB-20) give quantity distributions.
2. Bullion computes cost per recoverable ounce and NRV at the per-inventory-class price basis (a `Rate` document
   per class, set by accounting policy), and
   computes P(NRV < carrying value).
3. If that probability exceeds a policy threshold, or at period end, a `Workpaper` with the computation, the
   price evidence and the survey/reconciliation evidence goes to the controller.
4. The controller reviews it, and an `Adjusting` entry is posted per book. IAS 2.33 permits reversals; under ASC
   330 the write-down sets a new cost basis. That basis difference is a per-book policy object (FACT, R4 §5.2).

### 21.6 The metric engine (`fmine-metrics`): AISC, AIC, C1, GEO, reserve replacement, ROCE

**Doctrine (from B3 §5.3).**
1. **One authority per number.** FA owns actual costs as certified rows. FrankenMining owns physical drivers,
   plans and the *metric definitions*.
2. **One function, two inputs (PO-PUSH-9).** `aisc(rules_version, inputs)` runs identically on:
   - plan quantities × driver rates (forecast);
   - FA-certified actuals (actual).
3. **Rules are versioned data.**
   - The WGC guidance note was issued on 26 June 2013 and updated in 2018 (FACT, E611; the November 2018 month is FACT[U]).
   - AISC includes on-site costs, corporate G&A including share-based pay, lease payments, sustaining
     exploration and capital, production-phase stripping and underground development, current NRV write-downs,
     and reclamation at operating sites.
   - It excludes interest, M&A, impairments, one-off severance, income tax and growth.
   - AIC adds non-sustaining items.
   - **The sustaining vs non-sustaining test:** a project is non-sustaining if it raises annual or LOM production,
     NPV or reserves by ≥ 10% *and* generates no net cash within 12 months. Pushbacks count as sustaining unless
     they take ≥ 12 months *and* the ore phase exceeds 5 years.
   - Barrick cites the 2013 definition in its 2026 Circular (FACT, R6 §4.1).
4. **Variants are named adjustment layers with reason codes.** Examples:
   - by-product vs co-product: Newmont's Q3 2026 by-product AISC is $1,454/oz against a co-product AISC of
     $1,963/oz (FACT, R6 §2.3; the $1,520/oz figure is the full-year guidance, a different basis);
   - compensation-adjusted: Barrick's API "actual" of $1,423/oz vs $1,637/oz reported for FY2025, with the
     adjustment basis not located (FACT, R6 §1.5);
   - attributable vs consolidated, through FA party ownership;
   - **aggregation layers:** company AISC = Σ sold-weighted site AISC + L_corp + L_JV. In Barrick's Mine Stats,
     Total Gold sits $47.6–55.7/oz above its sold-weighted regions in the six blocks Q1, Q2 and H1 2025 and Q1,
     Q2 and H1 2026 (Q3 2025 is +$88.66; R7-C125), while
     regions reconcile to 0.1% (INFERENCE, E110, E111). Across all sixteen quarters 2023Q1–2026Q2 the layer is
     always positive, from +$23.57 to +$88.66/oz, and it roughly doubled from 2024 to 2025–26; the AISC workbooks
     carry G&A only in their total column: the row is zero for every site and region on all 40 sheets (FACT,
     E697), so corporate G&A sits wholly in L_corp (E496; W1 §11, §12); NGM sits $8.7–19.4/oz above its four
     mines (INFERENCE, E112). Each layer is named and cites the Quarter Report reconciliation (E107).

   **A reported gap that cannot be decomposed into named layers is a refusal** (G-BULL-3), not a footnote.

   **Definition flags** on every registered AISC-family metric (C4 P14): `byproduct_credit_location`
   (SeparateLine, InsideSellingCosts, Excluded), `includes_inventory_adjustments`, and `per_oz_denominator`
   (Produced, Sold). Public unit costs are per attributable ounce **sold** (INFERENCE, E110). Veladero's 2014–2017
   AISC carries silver credits inside selling costs and inventory adjustments (FACT, E284), and Porgera's 2011
   "$841/oz" is a departmental cost per ounce produced, not AISC (C4 P14).
5. **AISC is never an objective** (§15.6). Its shadow price is published (§23.6).

**`CostDriverCard` (from B3 §5.3).** One card per site and cost pool:
- driver: t mined, t·km hauled, t milled, kWh, kg reagent/t, labour hours;
- fixed/variable split;
- rate distribution;
- the FA certificate IDs of the registers it was fitted on;
- valid time;
- colour `Estimated`.

Plan cost = Σ driver quantities × rates. Price-linked lines (royalties, profit-sharing, by-product credits) are
functions of the *same* price draw as revenue (§23.4).

**The variance waterfall, decomposed exactly and order-independently (DESIGN).**
- The bridge components are volume (tonnes), grade, recovery, unit rates by cost line, price-linked items,
  by-product price, classification and FX.
- A sequential decomposition ("change grade first, then recovery…") is **order-dependent**: the same grade change
  gets a different number depending on where it sits in the sequence (GB-26).
- FrankenMining uses the **log-mean Divisia index (LMDI)** for the multiplicative identity
  AISC = C / oz_sold, with oz_sold = T·g·R·B·S, where B = produced / (T·g·R) is the metal-balance and in-circuit
  inventory factor and S = sold / produced is the finished-goods inventory factor (both named effects; the
  public lane computes them from Mine Stats rows):
  - each factor's effect is L(AISC_a, AISC_p)·Δln(factor), with L(x, y) = (x − y)/(ln x − ln y);
  - this decomposition is exact (it sums to the total) and independent of order.
- Additive cost lines are split within the cost effect pro rata.
- **PO-PUSH-10: the bridge sums exactly** to the total variance, to the cent.
- The waterfall's residuals feed the calibration ledger: the plan was a registered forecast (§10).
- **On public data the waterfall is a descriptive accounting bridge, not a causal one (B7; E699).**
  - LMDI needs every factor finite and positive. Heap-leach sites publish no recovery (Veladero, Zaldívar) and
    some factors are zero or negative, so 26 of 127 gold site-quarter year-on-year pairs cannot be factored. They
    fall back to a signed additive split of the $M numerator lines, labelled as such, never to a forced factor.
    By-product credits are zero or positive in 124 of 519 AISC columns, so a credit-value effect is not a price
    effect.
  - Every bar carries the interval implied by the half-unit rounding of its inputs. A bar whose interval contains
    zero prints *sign not determined* (72 of 606 factor effects on the public tables).
  - Closure is arithmetic, not attribution: a +10% error in North Mara's 2026 grade moves about $177/oz from grade
    into B while the bridge still sums exactly, so PO-PUSH-10 holds either way. Causal labels wait for M2 and the
    metallurgical balance (GATE-DATA-03; §20).
  - The bridge closes on the recomputed rate 1,000 × Δ($M / koz). The gap to the printed rate is a listed rounding
    or denominator item (E691, E695), never absorbed into a bar.

**Other metrics.**
- **Copper C1 cash cost** per lb (by-product credits netted).
- **Cash cost** per oz.
- **GEO** (gold-equivalent ounces), with **the price-ratio basis named**. Budget ratios vs realized ratios give
  different answers (GB-27), and GEO is 25% of Barrick's API (FACT, R6 §1.5).
- **Newmont CSC/GEO** (adjusted cash sustaining cost per GEO, 35% of its bonus; FACT, R6 §2.5).
- **Reserve replacement under one registered definition.**
  - Barrick's FY2025 release and its 2026 Circular give three-year (2023–25) reserve replacement as "close to 190%"
    in gold ounces and as 504% in gold-equivalent ounces (the published figure; 87 Moz ÷ 17.2 Moz on the rounded
    inputs gives 506%, 499–514% under rounding; R7-C029), both excluding acquisitions and
    divestments (FACT, E603, E604): the bases differ by metal coverage, not by window.
  - The engine computes every basis and names each.
  - A KPI constraint uses only a reconciled definition (§23.6).
- **ROCE** on the stated capital-employed basis.

**The public lane (M0; §31.1, B5).** `fmine assure public --issuer barrick` reproduces published metrics from public
filings (EDGAR/SEDAR+, XBRL where present) and the issuer's quarterly Mine Stats and AISC workbooks:
- TARGET: GATE-BULL-01's three parts (below; §21.8): extraction fidelity, identity accounting with every
  instance in one of five cited states, and issuer consistency reported per identity family;
- every scored value passes the live-dependency proof: deleting or blanking any registered file refuses with a
  non-zero exit, and perturbing any cited cell by one unit in its last place moves the value and the state root
  (BD-338; it would have caught every stand-in in K.2 finding 1);
- byte-identical reruns, with a state root that commits to the input manifest and to every scored value, so any
  byte change in any input changes the root (R7-C065);
- zero uncited numbers: each extracted number carries its locator and file sha256; each derived number carries its
  rule (`rules_version`, G-BULL-6) and its input locators; each company-level figure cites the filed page that
  states it and reproduces that page's label (period or window, basis, metal coverage, denominator, ownership
  basis). A matching value under a different label fails (R7-C038, R7-C131);
- registered inputs only: a pinned source registry lists each workbook with its issuer URL, retrieval time and
  sha256 (equal to W1's). An absent, unreadable or mismatched file is a typed refusal that names the file. No
  in-code workbook ever stands in for a registered one; synthetic fixtures print `synthetic` in every output and
  never count toward a gate (R7-C034, R7-C037; Appendix K.2).

**The FY2026 forecast record (R7-C042, R7-C066, R7-C119, R7-C135, R7-C155; DESIGN).** The FY2026 per-site forecasts
are a scoreable, externally time-proved record, not a promise:
- **Information set.** A stated information cut-off and an input manifest (each document, its publication date,
  its sha256).
- **Targets and basis.** Per-site production on the 100% basis (Mine Stats column B, E456), with a pre-written
  rule for re-segmentation, divestment or ownership change; unit costs per ounce sold carry their own basis rule.
- **Scored quantity and baselines.** The Q4-implied value (FY − published 9M), scored with the interval score
  against two pre-registered baselines: Barrick's last stated per-site guidance and a run-rate persistence forecast.
  A per-ounce cost is not additive, so FY − 9M is undefined for it: the unit cost is scored on the Q4 value as
  printed (QTD). The rules (`forecasts/fy2026/rules.toml`: estimator, n_eff range, the named 13-site universe with
  Hemlo and Tongon `NOT_PUBLISHED`, guidance and basis rules) are hashed and time-proved before any forecast.
- **Deadline and time proof.** Hashed and pushed to the public repository with an RFC 3161 receipt or a completed
  OpenTimestamps proof by 31 Dec 2026, before any M0 content reaches an issuer insider, and before any January
  production release (Barrick issued mid-January Q4 releases for 2020–2024; R7-C154). Caller-supplied timestamps do
  not count (R7-C061). Forecasts hashed after the 9 Nov 2026 Q3 release are mostly a Q4 nowcast (Q4 was 26.8% of
  FY2025 attributable production; R7-C026) and are labelled so.
- **Attestation.** The author attests that no non-public issuer information was used (extends §10.1 `author`);
  any revision after first issuer contact is flagged `post-contact`.
- **Further vintages.** Q1 and Q2 2027 vintages follow, each with as-of date and horizon; a vintage hashed after
  its quarter ends is a nowcast and is scored separately (R7-C119). A vintage is issued only while its author
  holds no private access (§31.1): once the author joins the issuer's insider list, which the proposal places in
  the first three months of the engagement, no further vintage is issued, and the frozen ones are only scored.
- **What it can show.** Every score report gives n, an n_eff with its assumed correlation range, and the
  Clopper–Pearson interval. One period × about 14 sites demonstrates the protocol, not skill: separating 80% from
  60% coverage first needs n ≈ 35 independent outcomes, so no positive calibration claim is made by month 12
  (R7-C118). A severe miss can still be reported as evidence of miscalibration.

**Public-lane sources and scoring (C1 §4; C3 §3.5).**
- **Per-site source.** The only per-site AISC/TCC/grade/recovery source in the corpus is Barrick's quarterly Mine
  Stats workbook (xlsx). The corpus held two workbooks (Q1 and Q2 2026; E148). The research pass retrieved all ten
  quarterly workbooks from **Q1 2024 to Q2 2026** from Barrick's public IR host, with their MD&As and, from Q3 2024,
  the per-site AISC reconciliation workbooks; the Q1/Q2 2026 downloads are byte-identical to the corpus copies, and
  comparative columns add the 2023 quarters as restated (FACT, E451, E452; W1 §2). **Q3 2026 is the only
  DATA-GATE**: Barrick releases it on 9 Nov 2026 (FACT, E454, E455). AISC workbooks were never published for Q1
  and Q2 2024 (E499). **There is no per-site GEO field**; GEO is company-level only, on a
  named price basis (GB-27).
- **Locator and import.** A workbook has no filing accession. Each cell cites
  `<file-period>:<Sheet>!<Cell>` + the file's sha256, plus the filed Quarter Report page that reconciles it (Q1
  2026 from p.36, Q2 2026 from p.44; E107). Import is **label-anchored**, because rows drift between files (CoS
  moved from row 38 to 40; E121). Footnote markers are stripped, ownership is parsed from labels, the tokens
  `—`, `n/a`, 0 and blank are typed (E119), and `#REF!` residue and the 15,903-column `YTD` range are excluded
  (E120). Ten vintages add rules (W1 §13): the region map is versioned by file (Pueblo Viejo moves into North
  America from Q4 2025, E461), so region aggregates are never compared across that boundary; 2024 copper rows
  labelled "millions of pounds" hold kt (E463); `n/a`, `—`, blank and numeric-zero placeholders in divested or
  withheld cells are all `Nil` with the token kept, including the AISC workbooks' zeros for Loulo-Gounkoto 2025
  (E497); the column scan is bounded and formula cells are ignored (Q2 2026 holds 1,092 `#REF!` cells, E466);
  first publication and later comparatives are both kept, because 9 of about 6,400 cells changed without a
  footnote (E474; W1 T16).
- **Denominator.** Numeric site-quarter cells only; `—`, `n/a` and placeholders are excluded and listed.
  Site-quarters that are `NOT_PUBLISHED` are excluded by name (Loulo-Gounkoto Q1–Q3 2025; E122, E248).
- **GATE-BULL-01 in three parts (R7-C040, R7-C111; replaces the single "≥ 95% reproduced" bar).**
  1. *Extraction fidelity.* 100% of the numeric cells in the named denominator N are extracted equal to the
     published value, with locators, measured against a reference read independent of the importer (a second
     reader, or BD-271's hand-checked cells). Reported separately for the ten known files and the blind file; every
     miss is a listed defect.
  2. *Identity accounting, in five states (B7; E691–E698, E815–E818).* Every identity instance (attributable =
     ownership × 100%, Σ components, Q1 + Q2 = H1, sold-weighted roll-up, $M × 1,000 / koz = $/oz for cost of
     sales, cash costs and AISC) is evaluated on rounding intervals under a construction declared per table family:
     a subtotal rounded once from unrounded inputs, or a sum of separately rounded component lines. Both are run
     and the declared one scores; the construction is never chosen per instance to make a check pass. The AISC
     tables declare the second: 1,207 of 1,212 printed TCC and AISC totals equal the sum of their displayed lines
     (E815). Cost of sales per ounce has a one-cell numerator, so its failures hold on any construction (E817).
     Each instance lands in exactly one state:
     (i) *consistent*;
     (ii) *explained* by a named layer (L_JV, L_corp, unpublished component such as capitalized interest,
     region-map boundary, restatement), citing the public pages or cells whose printed numbers close the
     arithmetic, and marked *stated* where an issuer note gives the rule or *reconstructed* where only the numbers
     close (E816);
     (iii) *issuer discrepancy*, with an exact certificate checked in rationals and the inclusion-minimal set of
     cells involved (BD-340);
     (iv) *undecidable from public data*, naming the missing fact;
     (v) *refused*, naming the missing input.
     Acceptance: 0 unclassified; an independent census assigns the same state to every instance; a second reader
     confirms each state-(iii) and state-(iv) item. Classification never stands in for part 1: a swapped-value
     mutation keeps 0 unclassified and 0 uncited rows while fidelity falls to 0 of 2 (E696). Known answers:
     Veladero Q4 2025, whose three per-ounce rates imply 44 koz against 47 printed (E691); Africa & Middle East
     Q1 2025, explained by reconstruction from Loulo-Gounkoto's public cells (E694, E816); Kibali Q2 2024, whose
     printed by-product sign breaks its own column's footing (E815); Kibali and Hemlo Q4 2025 cost of sales per
     ounce (E817); the first-filed 2024 GEO row (E692); the 2025 deck and MD&A split (E698).
  3. *Issuer consistency.* Reported per identity family as a finding, with no threshold. Barrick's own identities
     hold in 84/89 cases (94.4%) under strict rounding (E113–E115).
  The gate record prints N, the excluded placeholders and NOT_PUBLISHED list, and four counts: cells with a
  passing identity, cells matched by extraction only, explained register cells and refused cells. An independent
  census on the same files reproduces the counts. Why the single bar was withdrawn: 41–46% of numeric cells sit in
  no named identity, so for them "reproduced" is extraction fidelity (R7-C023, R7-C050); a perfect extractor scores
  90.5–92.4% if a failing identity disqualifies every participating cell and 98.0–99.7% if only the computed cell
  (R7-C110); and the 84/89 consistency rate it was set against measures a different quantity (R7-C024).
- **Component split (BD-058, partial).** Mine Stats publish per site CoS, royalties/oz, depreciation/oz, TCC,
  AISC, sustaining and project capex (cash basis), and cost per tonne. They do not publish G&A $/oz, sustaining
  leases, minesite exploration, reclamation accretion or by-product credits, so TCC → AISC is only a residual
  ($5–80/oz "other"). The per-site split needs the Quarter Report reconciliation and stays OPEN.

### 21.7 Close orchestration and the human-review bottleneck

FA's CloseRun DAG is NOT_STARTED (V07), so FrankenMining orchestrates the mining close in R4 §7.9's order. Each
step is a durable obligation with its inputs, its evidence and its reviewer:
1. bank reconciliations;
2. stockpile and pad surveys;
3. metal-balance reconciliation (Touchstone certificate, §20.9);
4. inventory valuation and NRV;
5. provisional-pricing MTM;
6. DD&A and stripping;
7. ARO accretion;
8. royalties and mining taxes;
9. stream deliveries and accretion;
10. hedge MTM;
11. JV billing and partner reconciliation;
12. intercompany matching;
13. FX revaluation (`fa revalue`);
14. consolidation (`fa consolidate --certify`);
15. statements and `IssueStatements`;
16. non-GAAP metrics (§21.6).

When FA's CloseRun lands, these become `CloseTask` vertices with `DEPENDS_ON` edges.

**Why monthly summaries, quantified (B4 card G1, renumbered GB-28).**
- **Equation.** t(n) = a + b·n with a = 0.118 s and b = 1.07 ms per entry already in the book (FACT-derived from
  the debug-build measurement; linear growth is a HYPOTHESIS).
- **Writer time.**
  - Per-truck posting (30,000 per day) would need about 4.85×10⁵ s of writer time on day one (ρ ≈ 5.6).
    Infeasible.
  - Daily site-family posting fits the writer, at about 29 min/day in year 1.
- **Human review.**
  - Daily site-family posting needs 4 families × 3 books = 12 reviews per site per day. For a site controller with
    8 h/month at 10 min/review who reviews only their own site, ρ_review = 12 × 30 × 10 ÷ 480 = 7.5. Infeasible.
    (Read as 60 reviews per book per day for one reviewer, it is 37.5; R7-C123.)
  - Monthly posting needs 12 reviews per site per month: ρ_review ≈ 0.25. If each of B4's 16 close steps per site
    also needs the same controller, ρ = 1.25 (0.42 with one book). G-BULL-2 needs a non-self-sourced document OR a
    human review, and §21.7 gives each close step its own reviewer, so the card states the review rule per step.
  - Writer time "≈ 29 min/day in year 1" is the end-of-year rate; the year-1 average is about 12–18 min/day.
- **Intuition.** The binding server is the human reviewer that FA-INV-12 requires, not FA's writer.
- **Flip.** FA admits an authenticated delegated reviewer (V19) → daily trunk posting becomes possible.

### 21.8 Guards, proof obligations and gates

| Guard | Condition | Action |
|---|---|---|
| G-BULL-1 | FrankenMining computes or displays a balance as authoritative | Refuse; cite the FA certificate instead |
| G-BULL-2 | A summary entry lacking a non-self-sourced document or a human review | Hold in `Prepared`; never commit |
| G-BULL-3 | A metric variant gap not decomposable into named layers | Refuse |
| G-BULL-4 | AISC forecast from drivers lacking FA certificates | Watermark "plan-only metric" (G-PUSH-7) |
| G-BULL-5 | Truck-, assay- or shift-level posting attempted | Refuse (§21.3.3) |
| G-BULL-6 | A metric rendered without `rules_version` and basis | Refuse |

| PO | Statement | Verification |
|---|---|---|
| PO-PUSH-9 | Plan and actual AISC use the same `aisc()` and `rules_version` | Type-level single function; test |
| PO-PUSH-10 | The plan→actual bridge sums exactly; LMDI effects are order-independent | Property tests over random permutations |
| PO-BULL-1 | Exactly-once posting: at most one live Original per slot (B4 PO-4) | FA-ACC-02/03/12; automaton model check |
| PO-BULL-2 | Bridge idempotency: replaying the outbox after any crash leaves FA unchanged (B4 PO-10) | FA-ACC-12 over 1,000 seeds |
| PO-BULL-3 | Merkle-folded detail rows fold exactly to the summary leg | Property test; FA-ACC-04 |
| PO-BULL-4 | Synthetic FA book: AISC by-product, co-product and comp-adjusted variants reproduced to $0.01/oz from certified rows | GATE-BULL-02 |

| Gate | Threshold (TARGET) | When |
|---|---|---|
| INT-FA-001 | FA-ACC-01…13 all pass at the pinned FA rev | Before any trunk posting (M3) |
| ✖ GATE-BULL-01 (public metric reproduction) | On registered inputs only (§21.6): (1) extraction fidelity 100% of N against an independent reference read; (2) identity accounting with every instance in one of five cited states, confirmed by an independent census (§21.6); (3) issuer consistency reported. Every scored value passes the live-dependency proof (BD-338). Reruns byte-identical with the input manifest in the state root; zero uncited numbers; every company-level figure matches its filed page and label, checked by a person who did not build the output; the FY2026 forecast record hashed with an external time proof (§21.6). The blind-layout clause runs on the Q3 2026 workbook only if the importer's revision was pushed before 06:00 ET on 9 Nov 2026; otherwise on the Q4 2026 workbook at M1 (R7-C041) | M0. Kill if blind-file extraction fidelity is < 80%, or two reviewers from the named pool (§37.3) rate it "nothing new": the public metric engine becomes test fixtures only, and weight shifts to M1 reconciliation and an early M4 (B5) |
| GATE-BULL-02 | Synthetic FA book reproduces all AISC variants to $0.01/oz; the bridge closes exactly | M3 (WP-13, with INT-FA-001; R7-C091, R7-C105) |
| GATE-BULL-03 | Site pilot: per-quarter AISC waterfall from tonnes, grade and recovery to unit cost, reconciled to the site's reported figures, in the site's reporting unit ($/lb at Lumwana; Minestats4!D25). Cost-driver bars (fuel, FX, royalty, sustaining capex) count only with GATE-DATA-03's cost-centre extracts; otherwise one aggregate cost effect is shown and labelled as such (R7-C142) | M2 |

### 21.9 Galaxy-brain cards (illustrative values)

**GB-26 · An exact, order-independent AISC waterfall.**
- **Identity.** AISC = C / oz_sold, oz_sold = T·g·R·B·S (§21.6). In this synthetic example B = S = 1.
- **Plan.** $800M of cost on 10.0 Mt at 1.728 g/t and 90% recovery = 500,000 oz, so AISC = **$1,600/oz**.
- **Actual.**
  - 9.9 Mt (−1%), grade −4%, recovery 89%, giving 469,920 oz.
  - Cost $820M: royalty +$15M (price-linked), diesel +$10M, labour +$15M, sustaining deferral −$20M.
  - AISC = **$1,745/oz** (+$144.98).
- **LMDI effects (sum exactly to +$144.98).**
  - Grade **+$68.23**.
  - Cost +$41.27, made up of royalty +$30.95, diesel +$20.64, labour +$30.95 and sustaining deferral −$41.27.
  - Recovery +$18.68.
  - Tonnes +$16.80.
- **The order problem.** A sequential waterfall assigns grade +$66.67 if grade goes first and +$69.80 if it goes
  last. The answer depends on the analyst's arbitrary ordering, and LMDI removes that choice.
- **Intuition.** Two things a CFO should see:
  - the "lower grades processed" explanation has an exact, auditable number;
  - the **sustaining deferral flatters AISC by $41/oz** while raising future hazard, which the DecisionCard shows
    beside it (§23.2).
- **Flip.** Reclassifying the deferred item as growth under the WGC test would also move it out of AISC.
  G-BULL-3 requires that move to be a named layer with an attested project classification.
- **Public case: the bridge does not close without S.** North Mara Q2 2025 → Q2 2026, +$1,285/oz, decomposes
  exactly into grade +$1,047, cost +$725, recovery +$65, tonnes +$27, metal balance B −$75 and **sold/produced S
  −$504** (INFERENCE, E137). Sold/produced ranges from 0.81 to 1.26 across sites and quarters (E138).

**GB-27 · GEO moves when the gold price moves.**
- **Equation.** GEO = Au oz + Cu lb × (P_Cu/P_Au) + Ag oz × (P_Ag/P_Au) + ….
- **Substituted.** 3,200 koz Au and 440 Mlb Cu at $4.50/lb.
  - At $3,000/oz gold: GEO = **3,860 koz**, of which copper contributes 660 koz.
  - At $4,500/oz gold: GEO = **3,640 koz**, of which copper contributes 440 koz.
- **Result.** GEO falls **220 koz** with no physical change. That is 38% of an illustrative threshold-to-maximum
  pay range of 579 koz.
- **Intuition.** A "production" KPI with a price-ratio basis is partly a price KPI. The engine renders GEO on
  budget ratios and realized ratios side by side, and names which one pay uses.
- **Flip (met for Barrick).** Barrick's 2025 API computes GEO at fixed long-term reserve prices ($1,500/oz Au,
  $3.25/lb Cu; FACT, E606), so the effect disappears from pay and reappears in the plan-vs-actual bridge as a
  "ratio basis" layer. The 579 koz pay range above is therefore insulated from the gold price.

**Open questions.**
- **OPEN-BULL-1.** Which basis Barrick's compensation-adjusted AISC uses (R6 [UNV]); needed for exact
  reproduction.
- **OPEN-BULL-2.** The cause of FA's O(n) per-post growth: a defect or inherent? It decides KILL-1 risk
  (OPEN-B4-04).
- **OPEN-BULL-3.** Whether FA will accept a delegated reviewer (V19) to permit finer posting.
- **OPEN-BULL-4 (resolved from primary sources, W5).** Both figures are three-year (2023–2025), exclude
  acquisitions and divestments, and divide reserve additions by depletion. 190% is gold ounces (FY2025 release
  endnote 7; Circular p.41); 504% is gold-equivalent ounces at reserve prices, 87 Moz ÷ 17.2 Moz, with 2024
  contributing 79 Moz (Circular p.86) (FACT, E603, E604). The corpus's pooled 3-year GEO figure (506%, 499–514%
  under rounding; E146) agrees. The 2020–2024 pooled 186% (E145) fits a differently footnoted "close to 190%"
  and is not the results-release basis. BD-063 reproduces both from the Circular table.

## 22. Helm I: DecisionCards and the value of information

Crate: `fmine-decide`, built over asupersync's `franken_decision` (`LossMatrix`, `Posterior`, `FallbackPolicy`,
`DecisionContract`, `DecisionAuditEntry`, `evaluate`; FACT, B3 V9, E012), with lifted `fs-voi` and `fs-robust`
patterns.

### 22.1 Doctrine

- **The DecisionCard is the unit of delivery to executives**, not a dashboard (anti-goal, §4.3). Every material
  decision gets one. This covers capital, reserves, dams, plans, plant changes and roster changes.
- **FrankenMining never decides.** `execution_authorized` is hard-wired false, and the card has no authorization
  field (E023). A card lists the sign-offs it needs; humans give them, and the record of who signed what is the
  audit trail.
- **A card is honest about its weakest input.** Its headline colour is the weakest colour among material inputs
  (`fs-robust` `weakest_color`; no laundering, NL rules §7.3).
- **A card is scoreable.** Its outcome-relevant predictions are registered forecasts (§10). Decisions are audited
  on *process*: was the card complete, built as known at the time, and were the flips stated? Outcomes are scored
  too, but a decision is never judged by its outcome alone, because a good decision can meet bad luck.
- **A finding is not a decision (B7).** When the supported action is to check or correct a source, the deliverable
  is an evidence-challenge packet (BD-349): every operand's locator, the owner role, the uses blocked until it
  closes, and the document that would close it. A DecisionCard needs actions, states and utilities. Public
  exceptions supply none of them, so a card built from them refuses rather than inventing an NPV, a CVaR or a test
  likelihood (B7 EV-P2-3, EV-P3-5).

### 22.2 Schema (DESIGN)

```rust
pub struct DecisionCard {                         // emitted with a franken_decision DecisionAuditEntry
    pub id: DecisionId, pub question: String, pub owner: RoleRef, pub as_known_at: AnchorRef,
    pub alternatives: Vec<AltStats>,               // always includes "do nothing" and, where relevant, "buy information first"
    pub loss: LossMatrixRef,                        // utility/loss over (alternative × scenario), with units
    pub flips: Vec<FlipCondition>,                  // break-evens and ranking-flip probabilities
    pub kpi_prices: Vec<ShadowPrice>,               // §23.6
    pub info_options: Vec<InformationOption>,       // §22.4
    pub uu: UuTerm,                                 // §9.3: priced, bounded or flip margin, never absent
    pub monitors: Vec<MonitorSpec>, pub kill_gates: Vec<KillGateSpec>,   // what will be watched after the decision
    pub registered_forecasts: Vec<ForecastId>,      // the predictions this card stakes its credibility on (§10)
    pub color: Color, pub signoffs: Vec<SignoffRequirement>,   // QP/CP, EoR, CFO, board…; no authorization field exists
    pub refusals: Vec<Refusal>,
}
pub struct AltStats { pub alt: AltId, pub mean: Money, pub p10_p50_p90: [Money; 3], pub cvar: Option<(f64, Money)>,
    pub regret_mean_max: (Money, Money), pub selection_freq: Option<f64>, pub evidence: EvidenceSummary }
pub struct FlipCondition { pub input: ClaimRef, pub threshold: Quantity, pub direction: Dir, pub p_flip: Option<f64>, pub new_choice: AltId }
pub struct InformationOption { pub action: InfoAction, pub cost: Money, pub delay: Duration, pub evsi: Estimate<Money>,
    pub status: VoiStatus /* Positive | Negative | Indeterminate{se} */, pub flips_if: Vec<FlipCondition> }
```

### 22.3 Decision catalogue

| Decision | Card source | Typical alternatives | Dominant uncertainty |
|---|---|---|---|
| Exploration programme and drill targets | Lodestar (§11.5) | Programmes, budgets, stop | Deposit presence and size; detection |
| Infill or resource-conversion drilling | Strata + Helm VOI | Spacings, holes, defer | Local grade and continuity |
| Pushback start, cut-off policy, stockpile policy | Pushback (§15) | Plans and policies with certificates | Calibration gap; price |
| Grade-control destinations | Pushback (§15.7) | Batched per blast, not per polygon | Local posterior |
| Fleet size, maintenance strategy | Headframe (§16) | Fleet counts; age vs condition | Cycle variability; failure hazards |
| Grind, blend, campaign, process change | Crucible (§17) | Set-point regimes; capital candidates | Card validity; recovery |
| TSF raise, pumping, closure option | Freeboard (§18) | Options with risk reduction | Climate tail; failure-mode evidence |
| Roster change | Muster (§19) | Rosters within legal envelope | Fatigue; retention |
| Measurement and audit | Touchstone (§20.6) | Audits, campaigns, resurveys | Identifiability |
| Capital programme, juniors, M&A, buybacks, streams | Helm II (§23) | Portfolios | Joint price, geology, country, execution |
| Reserve and resource statement support | Hallmark (§24) | Price decks, cut-offs | Classification support (the CP decides) |

### 22.4 Value of information

- **The general estimator.** Preposterior analysis:
  VOI(o) = E_y[max_a E(U | a, y)] − max_a E(U | a).
  - It is computed by nested Monte Carlo with common random numbers and control variates, and reports its Monte
    Carlo standard error (as in §11.5(C)).
  - `VoiStatus::Indeterminate` when the sign of net VOI is unresolved at 2·SE.
- **Closed forms where valid.** `fs-voi`'s ranking-flip probability and top-two EVPI surrogate assume Gaussian
  objectives (FACT, B3 V7). They are used only on Gaussian sub-problems, with that assumption stated.
- **Narrow adoption (B3 H3c).** Information options attach to *discrete commitments*: pushback start, plant
  expansion, cave footprint, TSF raise, acquisition, earn-in stage. **Whole-mine POMDPs are rejected for v1.**
- **VOI is zero when no outcome of the test can flip the decision.** The engine checks this first and reports
  "no test result changes the choice" as the cheapest correct answer (GB-29).
- **Entropy-based "expected information" is a secondary column**, never the ranking key. Information that cannot
  change a decision has no decision value, however much it reduces entropy.
- **Submodular batching.** Batch drill selection uses the coverage objective of §11.5(E), with its approximation
  certificate. VOI itself is not submodular in general (FACT[U], Krause & Guestrin), so no such certificate is
  claimed for VOI batches.

### 22.5 Robustness views on every card

- **Regret.** Per scenario, regret is the best-in-hindsight value over the *same* scenario set minus the chosen
  alternative's value (PO-PUSH-11: regret ≥ 0). Mean and maximum regret are shown.
- **Minimax regret and Wasserstein DRO** are robustness checks over contested scenario-family weights: country
  hazard, climate, price regime. They use the exact Mohajerin Esfahani–Kuhn dual on discrete support, pattern from
  `fs-dfo::dro` (FACT, B3 V6).
- **Selection stability.** For portfolio decisions, selection frequency is computed over ≥ 200 bootstrap
  resamples of the scenario set. Below 0.8 the card is flagged "fragile" and shows the runner-up and the flip
  probability (B3 guard 10).
- **Unknown unknowns.** The UU term is a flip margin when no probability is admissible (§9.3), e.g. "this
  choice reverses if an unmodelled event costs more than $X before 2029".

### 22.6 Retrospective cards and the no-hindsight rule

- Retrospective cards, such as "what the card would have said for Project X in 2019", are built **bitemporally,
  as known at the decision date**. They use only evidence recorded by then, through `AS KNOWN AT` selectors (§6.3).
- A retrospective card that uses later evidence is refused (G-HELM-4). This is the anti-goal against hindsight
  claims (§4.3).
- B5 places two retrospective cards in M1 as the test of whether cards change executive conversations (B5 H-e).

### 22.7 Guards and proof obligations

| Guard (evaluated in order) | Condition | Action |
|---|---|---|
| G-HELM-1 | A safety or hard constraint would be priced, or any INV-002 flag (MNPI, safety or sign-off semantics) would be set | Refuse; route to a human; no card |
| G-HELM-2 | No "do nothing" alternative | Refuse the card |
| G-HELM-3 | Card colour above the weakest material input | Refuse (no laundering) |
| G-HELM-4 | A retrospective card uses evidence recorded after its as-known-at anchor | Refuse |
| G-HELM-5 | A top-3 sensitivity input is `Assumed` | Banner "decision rests on assumption X" |
| G-HELM-6 | VOI reported without its Monte Carlo SE or status | Refuse the VOI column |
| G-HELM-7 | Regret computed against a different scenario set from the alternatives' values | Refuse |

| PO | Statement | Verification |
|---|---|---|
| PO-PUSH-11 | Regret ≥ 0 and is computed from best-in-hindsight over the same scenario set | Property test |
| PO-HELM-1 | VOI ≥ 0 in expectation; estimator unbiased on conjugate toys | Known-answer tests |
| PO-HELM-2 | Card digest is a pure function of (anchor, inputs, seeds, code) | Replay test |
| PO-HELM-3 | Every card's registered forecasts resolve or carry a declared non-resolution | Ledger audit |

| Gate | Threshold (TARGET) | When |
|---|---|---|
| GATE-HELM-01 | Two retrospective cards built as known at the decision date, reviewed by an executive or ex-executive panel; at least one judged decision-changing | M1 (B5 H-e) |
| GATE-HELM-02 | Monte Carlo VOI estimator recovers the closed-form EVSI of K GB-29-type discrete problems: a χ² test of the K standardised deviations of the 100-seed means at α = 0.01 (no per-seed 2·SE rule: every seed within 2·SE passes an unbiased estimator with probability 0.0095, and the per-problem mean rule passes it with 0.9545^K, R7-C121) | M1 |

### 22.8 Galaxy-brain card (illustrative values)

**GB-29 · Should we buy metallurgical testwork before the final investment decision?**
- **Set-up.**
  - An expansion's recovery is high (P = 0.6, NPV +$300M) or low (P = 0.4, NPV −$200M).
  - EV(build now) = 0.6·300 − 0.4·200 = **$100M**, so the default is to build. EVPI = $80M.
- **Testwork** is 85% accurate both ways and costs $6M plus six months of delay, which is worth $3.8M at 8%.
  - P(test says "high") = 0.57, and then P(high) = 0.895 and EV = $247M: build.
  - P(test says "low") = 0.43, and then P(high) = 0.209 and EV = −$95M: don't build.
  - So EV with the test = 0.57 × 247.4 = $141.0M, and **EVSI = $41.0M** against $9.8M of cost and delay. Buy the
    testwork.
- **Intuition.** The test is worth buying because a "low" result *flips* the decision. Information has value only
  through the decisions it can change.
- **Flips.**
  - Test accuracy below about **73%** → EVSI < $9.8M → build without it.
  - Prior P(high) ≥ **34/43 ≈ 0.79** → even a "low" result leaves EV(build) > 0 → **EVSI = 0** → build without
    it. At 0.78 a "low" result still flips the decision, but EVSI is only about $2.3M, below the $9.8M cost
    (B7 EV-P2-4).
    The engine reports "no test result changes the choice".
- **Assumptions.** Two-state recovery (the real card uses the Crucible card's posterior); test errors are
  symmetric and independent of the state.

## 23. Helm II: the CapitalProgram, executive optimization and the scenario index

Crates:
- `fmine-capital`: candidates, programme, options, country regimes;
- `fmine-econ`: cash-flow models, fiscal terms, scenario index;
- `fmine-calib::refclass`: reference-class priors.

Source: B3 PQ-7. All galaxy-brain numbers are illustrative.

### 23.1 Verdict (Brenner pass B3, PQ-7)

| Hypothesis | Verdict | Role |
|---|---|---|
| H7a NPV/IRR plus hurdle rate | **Rejected as the rule; retained as a reporting column** | Ignores overrun bias, concentrates price beta, and makes exploration look NPV-negative. Hindcast evidence against: Tanami 2's estimate rose from $850–950M (Feb 2022, production 2024) to $1.7–1.8B (Feb 2026, production H1 2027), roughly 1.8–2.1×; Pascua-Lama was written down by $5.2B in Q2 2013 and $6.1B for the year (FACT, E631, E632, E633). |
| H7b Mean-CVaR over shared scenarios | **Adopted as the risk layer** | Always reported with selection frequency, never as a single "optimal portfolio" |
| H7c Real options plus bandit/Gittins indices | **Staged-option valuation adopted; Gittins and regret *guarantees* rejected** | Gittins needs independent arms, one arm played at a time, frozen idle arms and no budget coupling, and all are violated (shared price, regional geology, parallel programmes, tenure expiry, competitors). O(√T) regret bounds are vacuous at 10–30 decisions per decade. Indices are triage heuristics only. Option value is shown *separately* from static NPV to prevent option-value laundering. |
| H7d Per-share two-stage stochastic `CapitalProgram` with recourse (forced alternative) | **Adopted as THE decision object** | Contains (a) as a column, (b) as the risk layer and (c) as recourse |

### 23.2 Every capital alternative is one `CapitalCandidate`

| Type | First-stage action | Recourse | Cash-flow source | Key priors | Typical evidence class |
|---|---|---|---|---|---|
| Sustaining capex | Fund / defer | Catch-up later at higher cost; failure hazard | Pushback plan with vs without; hazard models (TSF §18, fleet §16, plant §17) | Failure hazards, overrun | ModelledUncalibrated |
| Brownfield expansion | Study → FEED → build gates | Delay, rescale, abandon | Plan delta under the same scenario index | Overrun, ramp-up, conversion | Modelled |
| Greenfield exploration play | Fund stage k | Advance, abandon, JV-out | Lodestar posterior (§11.5(B)) × V(m) (§23.5) × conversion × development path; 15.7–17.9-year lead time (FACT[U], S&P; E636) | Discovery probability, size, lead time | Elicited / Transferred |
| Junior equity stake | Buy x% | Top-up, ROFR, sell | Junior's public NI 43-101/JORC resource, **haircut** (§23.5); market price process | h_pub, conversion, P_dev, dilution | Transferred |
| Earn-in / option JV | Stage spend | Walk away / complete | As above, then own-drill data | Conversion | Transferred → Measured |
| M&A | Bid | Integrate, divest | Target assets as candidates on public data, plus synergies | Synergy realization (OPEN), premium. Build-vs-buy parity: greenfield median $18,200/tpa plus a 40% overrun ≈ $25,480/tpa, about equal to the 2020–26 copper M&A median of $25,182/tpa (FACT, FactSet 16 Jul 2026, E642; $18,048 is the all-project median) | Transferred |
| Stream or royalty sale | Sell x% of metal for deposit D | Buy-back clauses | Own production × price paths: the payout is **scenario-dependent**, like a put | Price paths | Modelled |
| Share buyback | Repurchase $C | — | ΔNAV/share = C·(NAV_ω/P − 1)/shares, which is **correlated with the gold price**, not risk-free | P/NAV, own NAV ensemble | Modelled |
| Dividend top-up | Pay | — | Policy (e.g. Barrick's 50% of attributable FCF; FACT, R6 §1.4) | — | Policy (attested) |

**Sustaining capex is risk reduction, not an NPV project.**
- Its payoff is the reduction in expected loss *and* in the CVaR of catastrophic events, such as TSF loss of
  containment (GB-21).
- Deferral lowers current AISC, because AISC includes sustaining capex (GB-26). The KPI therefore *rewards*
  deferral.
- Every sustaining card shows that hazard–AISC trade-off explicitly.

```rust
pub struct CapitalCandidate {
    pub id: CandidateId, pub kind: CandidateKind, pub stages: Vec<StageGate>,
    pub entity: EntityNodeId,                // where the asset or spend sits in the CapitalEntityGraph
    pub cashflows: CashflowEnsemble,         // 100% basis, keyed by ScenarioIndex (common random numbers)
    pub kpis: KpiEnsemble, pub tier_one: Option<TierOneAssessment>,
    pub adjustments: Vec<ReferenceClassAdjustment>, pub evidence: EvidenceSummary,
}
pub struct StageGate { pub cost: CashflowEnsemble, pub reveals: InformationSpec, pub recourse: Vec<RecourseAction>,
    pub approvals: Vec<PermitGate> /* authority, filed, statutory_clock, deemed_rule, reopen_thresholds */ }
pub struct CapitalEntityGraph {              // parent, carve-out, JV and state-sharing waterfall (DESIGN)
    pub nodes: Vec<EntityNode>,              // Parent | CarveOut | NgmJv | PvJv | KibaliJv | TwigaJv | RekoDiqJv
    pub edges: Vec<WaterfallEdge>,           // equity %, funding or free carry, DSCR locks, repatriation, sharing
}
pub struct CapitalProgram { pub entity_graph: CapitalEntityGraph, pub candidates: Vec<CapitalCandidate>,
    pub budget: Money, pub liquidity_floor: Money, pub cvar: CvarConstraint,
    pub kpis: Vec<KpiConstraint>, pub objective: PerShareNav /* evaluated at Parent and CarveOut */ }
```

- **Ownership tiers, JV waterfalls and the IPO perimeter (`CapitalEntityGraph`; FACT plus DESIGN).** A major does
  not receive 100% of asset cash flow, and its interest is not a constant multiplier:
  1. **Parent and carve-out.** Barrick is evaluating an IPO of a small minority interest in a North American
     company anchored by its NGM and Pueblo Viejo JV interests and the Fourmile discovery; under the 9 Aug 2026
     agreement with Newmont, Fourmile is to be vended into NGM (FACT, E658, E629). `CapitalProgram` evaluates each
     candidate's ΔNAV/share, AISC and FCF yield for both the parent and the carve-out, so vend-in terms and any
     inter-entity toll (§15.6 item 9) are priced on both denominators.
  2. **NGM (61.5% Barrick, 38.5% Newmont; E457) and Pueblo Viejo (60/40; E672).** Growth candidates need partner
     funding or trigger the JV agreements' dilution terms (DESIGN; terms to be registered from the agreements).
  3. **Kibali (45% Barrick, 45% AngloGold Ashanti, 10% SOKIMO; equity-accounted).** Cash leaves the JV only as
     dividends or loan repayments. Royalties and charges total 5.7% of gold revenue including shipment fees, and the
     50% super-profits tax applies to the increase in gross operating surplus when gold exceeds $2,000/oz (FACT,
     E242, E243).
  4. **Twiga (North Mara and Bulyanhulu: 84% Barrick, 16% Government of Tanzania free-carried).** The 50/50 division
     of economic benefits is kept by an annual true-up (FACT, E448), so state taxes, royalties, fees and the 16%
     dividend are tracked against the 50% share.
  5. **Reko Diq (50% Barrick; 25% federal SOEs; Balochistan 15% funded and 10% free-carried; E302, E303).**
     Project-finance cash sweeps and DSCR locks gate distributions, and Barrick recovers its funding of the
     free-carried share (DESIGN; terms to be registered).
- **Permit gates (C5 §4.7).** Early-works spend that proceeds while the main approval is pending is a real option
  with regulatory recourse. Reko Diq split its Early Works ESIA from the main ESIA so construction could proceed
  concurrently; the provincial review has a four-month clock with deemed approval, and first-order changes such
  as > 33% more power capacity reopen approval (FACT, E305, E306, E339).

### 23.3 The optimization

- **Problem.** Maximize E[ΔNAV_ω / shares_ω], since buybacks and issuance change the share count, subject to:
  - budget and liquidity: a minimum-cash floor (e.g. Newmont's $5B; FACT, R6 §2.4);
  - CVaR_α(ΔNAV) ≥ τ, using the Rockafellar–Uryasev LP;
  - KPI soft constraints with published shadow prices (§23.6);
  - stage-gate recourse per scenario.
- **Scale (TARGET).**
  - 50–500 candidates, typically ≤ 200 binaries after grouping mutually exclusive stage variants.
  - 2,000 scenarios for optimization after Kantorovich reduction; ≥ 10⁴ for evaluation.
- **Solver.** `fmine-lp::milp` to ≤ 0.5% gap in ≤ 10 min. Fallback: greedy plus a Lagrangian bound, with the gap
  reported.
- **The bottleneck is input validity, not compute.** That is why reference classes, conversion haircuts and
  country processes (§23.5) get more design attention than the solver.
- **Robustness.** Minimax regret and Wasserstein DRO over contested scenario-family weights (§22.5).

### 23.4 The shared scenario index

```rust
pub struct ScenarioIndex {          // one draw ω of the enterprise world
    pub market: PricePathId,        // Au, Cu, Ag, Sn, FX, diesel, power, carbon: joint process plus copula
    pub inflation: CostPathId,      // industry-wide cost factor (the common cause of overruns)
    pub country: CountryPathId,     // one regime path per country, shared by all assets in it
    pub geology: RealizationSetId,  // per-asset realization ids from Strata and Lodestar ensembles (§9.5)
    pub execution: ExecDrawId,      // per-project overrun, slip and ramp-up draws, plus a common factor
    pub climate: ClimatePathId,     // water balance and TSF scenarios (§18.4)
}
```

- **Common random numbers** across all candidates make comparisons paired and low-variance. Streams use Philox and
  Sobol keyed by logical identity (lifted `fs-rand`; §9.5).
- **Portfolio CVaR is computed only on joint draws**, never by combining marginal distributions.
- **Price-linked costs** (royalties, profit-sharing, by-product credits) share the `market` draw (§21.6).
- **Country regimes are a stochastic process, not a discount-rate fudge (B3 §6.5).**
  - Each country has a `CountryRegime` Markov chain over {Stable, FiscalShock, SecurityDisruption,
    Suspension/Expropriation}. Operation-level states add `Restricted{reagent | water}`, distinct from suspension
    (Veladero's cyanide-addition restriction, 29 March–15 June 2017; E278, E281), and `NOT_PUBLISHED` site-quarters.
  - Impacts are cash-flow transforms:
    - royalty or tax step-ups, including retroactive ones. Mali's Mining Code is Loi n°2023-040 of 29 Aug 2023
      (JO 1 Sep 2023; FACT, E415). Its Art. 218 keeps valid permits on their original regime until renewal but
      gives the Code precedence over conventions (E421). Loulo-Gounkoto moved to the 2023 Code by settlement
      (24 Nov 2025; E410), and the Code was applied retroactively to 2024–2025: $200M of additional royalties,
      penalties and interest paid in April 2026 (FACT, E412, E413). The average royalty is now about 18%, against
      6% under the 1991-Code conventions of the March 2023 report (FACT, E414, E246);
    - suspension, loss of control, and post-settlement fiscal reset. Loulo-Gounkoto: export restriction Nov 2024,
      four employees detained 25 Nov 2024, gold stock removed 11 Jan 2025, suspension announced and initiated
      14 Jan 2025, court-ordered six-month provisional administration and loss of control 16 Jun 2025
      (administrator in day-to-day charge from 23 Jun), settlement 24 Nov 2025 ($253M paid 28 Nov), control
      returned 16 Dec 2025 (FACT, E248, E401–E409). Barrick's later footnote reports no operating data for
      Q1–Q3 2025; the first Q1 2025 publication carries production (22 koz at 100%) and withholds only
      per-ounce data, and FY2025 production is reported as 29 koz (FACT, E122, E477, E478; W1-DR-17). After the
      reset, H1 2026 production was **152 koz Au** (64 koz in Q1; 88 koz in Q2 from 914 kt at 3.25 g/t and 92%
      recovery), with royalties of $585/oz in Q2 2026 ($629/oz in H1 2026); Q2 2026 TCC was $1,897/oz and AISC
      $2,240/oz, H1 2026 TCC $1,906/oz and AISC $2,104/oz (Barrick 80% basis; FACT, E483, E638,
      E670);
    - portfolio exits that end a reported series: **Tongon** sold to the Atlantic Group for up to $305M, closed
      1 Dec 2025; **Hemlo** sold to Carcetti Capital for up to $1.09B, closed 26 Nov 2025. Both keep their columns
      with `—` and numeric-zero placeholders in 2026 files (FACT, E485–E488, E553, E554);
    - tenure refusal and ownership reset: Porgera, where the State declined to extend the SML (24 Apr 2020), the
      mine was on care and maintenance from 25 Apr 2020 to 22 Dec 2023, and it reopened with Barrick's equity halved
      from 47.5% to 24.5% (FACT, E425–E432);
    - contract annulment: Cobre Panamá (industry reference case), where Law 406 was declared unconstitutional on
      28 Nov 2023, the mine has been under Preservation and Safe Management since, stockpile-only processing was
      approved on 7 Apr 2026, and as of 28 Oct 2026 the President is weighing a commission's recommendation to
      negotiate an orderly closure (FACT, E433–E442);
    - slowdown with capex escalation (Reko Diq 2026).
  - **Each regime row carries its instrument (convention vs code) and an as-of date.** Ready rows (FACT unless
    marked; C3 §3.6; W2 §3):
    - DRC/Kibali (as of 2026-02-27): Law 007/2002 as amended by Law 18/001 of 9 Mar 2018 (regulations 8 Jun 2018);
      the Art. 276 five-year stability on existing rights lapsed about Mar 2023 (INFERENCE). Statutory gold royalty
      3.5% of gross commercial value (Art. 241, E445); Kibali's total royalty and charges are 5.7% of revenue
      including 2% shipment fees (E243; the TR's pages label it differently, E449), and it is exempt from the 2025
      Finance Law's +2% export charge (E447). Super-profits tax 50% of profit from prices more than 25% above the
      BFS price, i.e. above $2,000/oz on Kibali's $1,600 (Art. 251 bis; E242, E446); CIT 30%; State 10% via SOKIMO.
      Permits: seven expire 11 May 2029 and three on 3 Jun 2030, against a LOM to 2043 (E245). Renewal is for at
      most 15 years each (Art. 67) and needs a new feasibility study proving reserves, financing, an approved ESIA
      update, profitability, fiscal compliance and **a further 5% of equity to the State per renewal** (Art. 80;
      E443, E444). The application window for the 2029 permits opened 11 May 2024 (INFERENCE). The TR summarises
      renewal as conditional only on fees and environmental standards, which is narrower than Art. 80.
    - Mali/Loulo-Gounkoto (as of 2026-08-10): Loi n°2023-040, adopted by settlement in place of the 1991-Code
      conventions; Somilo permit renewed for 10 years under it on 13 Feb 2026 (E411). Average royalty about 18% at
      Barrick's $4,500/oz planning price: ISCP 5% of turnover, TAV 3–7.5% up to $2,500/oz plus 0.5 points per
      $400/oz above, revenue funds 2.25%, stamp 0.6% (E414, E422–E424). CIT 30%. State 20%; the new-project
      template is 10% free carry, a 20% cash option within 12 months and 5% to nationals, up to 35% (E416–E418).
      Stability runs to the 10th anniversary of first commercial production and does not shield mining duties or
      royalties (INFERENCE, E419). The royalty is a step function of price, so Helm draws it from the `market`
      draw like Kibali's super-profits tax.
    - [superseded 2025-11-24] Mali/Loulo-Gounkoto (as of 2023-03): 1991-Code Establishment Conventions; royalty
      6%; CIT 30%; minimum 0.75% of gross revenue; state 20% (E202, E246).
    - PNG/Porgera (as of 2026-08-10): an agreement stack, not the code alone: the Porgera Project Commencement
      Agreement (effective 3 Feb 2022, completed 22 Dec 2023), Special Mining Lease 13 to New Porgera Ltd and a
      Fiscal Stability Agreement (both 13 Oct 2023). New Porgera is 51% PNG stakeholders (KMHL 36, MRE 5, other 10)
      and 49% Porgera (Jersey) Ltd, owned 50/50 by Barrick and Zijin; Barrick holds 24.5% equity and 23.5% of
      economic benefits, and PNG stakeholders receive 53% of lifetime benefits. The State may buy the 49% at fair
      value after 10 years (window from about Dec 2033, INFERENCE). FSA royalty and tax rates are FACT[U]
      (E425–E432).
    - Panama/Cobre Panamá (industry reference case, as of 2026-10-28): no instrument in force. Law 406 (20 Oct 2023)
      was struck down on 28 Nov 2023 (gazetted 2 Dec), and Law 407 (3 Nov 2023) bars new concessions and renewals.
      The operating basis is the Preservation and Safe Management plan (30 May 2025) plus stockpile processing
      (7 Apr 2026, "not a reopening"). Regime path: Stable → Suspension (28 Nov 2023 →) with a
      `Restricted{stockpile-only}` sub-state from 2026 Q2 (E433–E442).
    - Tanzania/North Mara–Bulyanhulu (from 24 Jan 2020): the Twiga framework agreement under the Mining Act 2010 as
      amended in 2017. Royalty 6% plus a 1% clearing fee; the Government's 16% is free-carried from 1 Jan 2020;
      economic benefits are split 50/50 through an annual true-up that excludes the $300M settlement payment.
      Regime path: FiscalShock plus concentrate-export ban (3 Mar 2017 → 24 Jan 2020) → Stable (E135, E448).
  - **Reference class for host-state interruptions (W2 §4; label `Transferred`, §23.5).** Porgera: 3.66 years of
    care and maintenance, equity 47.5% → 24.5%. Cobre Panamá: at least 2.92 years and unresolved. Loulo-Gounkoto:
    336 days suspended, 183 days without control, $537M paid in three tranches. Tanzania/Acacia: a 2.89-year export
    ban ending in a 16% free carry and 50/50 sharing. In three of four resolved or ongoing cases the owner kept or
    regained operation on worse fiscal or equity terms, and every resolution included a lump sum or a larger free
    carry (INFERENCE; Barrick- and First Quantum-heavy, not an industry base rate).
  - **Price-contingent fiscal terms.** Kibali's super-profits tax is excluded from reserve economics because
    $1,500 is below its trigger; Helm models it as a function of the price draw, so after-tax value departs from
    the technical report at any price above $2,000. An importer flags arithmetic that mixes decks: Kibali's COG
    table charges $85.5/oz royalty at a $2,000 price, which is 5.7% of $1,500, not of $2,000 (INFERENCE, E244).
  - One regime path per country is shared by all assets in it.
  - **HYPOTHESIS H-FISC:** fiscal-shock hazard rises with the metal price (windfall-seeking). That creates a
    negative price–regime correlation that caps upside, which a hurdle-rate model cannot express (OPEN-HELM-4).
  - Priors come from structured elicitation (IDEA protocol, §10.6) plus a reference class, with Wasserstein DRO
    over hazard multipliers because these probabilities are contested.

### 23.5 Priors that matter more than the solver

**Reference-class priors for capex overruns and schedule slips (`fmine-calib::refclass`).**
- **Classes (FACT inputs):**
  - Bertisen & Davis, 63 projects: mean +14%, about half outside ±15%, about 1 in 13 at ≥ +100%;
  - McKinsey (Nov 2024, 80 projects): 83% of recent major projects with cost or schedule problems, overruns
    > 40%, delays 20–30%; megaprojects ≥ $1B: overruns ≥ 79%, delays 52% (FACT, E641);
  - schedule in years: no sourced reference class yet (the earlier "1.2-year mean across 58 projects" has no
    located source; OPEN);
  - lead time: 15.7 years (127 mines, 2002–23 starts) to 17.9 years (2020–23 starts) (FACT[U], S&P; E636).
- **Hierarchy:** industry class → company random effect → project. The company effect updates from the company's
  **own** approvals in its bitemporal record.
- **Adjustments are explicit and visible** (evidence class `Transferred`), never buried in the discount rate.
- **Fit check (GB-32).** A single lognormal cannot reproduce Bertisen & Davis's three statistics together, so a
  core-plus-blowout mixture is used. Gate GATE-CAP-02.

**Resource conversion and public-resource haircuts.**
- Reserve-equivalent R = h_pub · (c_IR·M_ind + c_FI·c_IR·M_inf), where:
  - h_pub is the public-report haircut (junior-reported vs the major's re-estimate);
  - c_FI is inferred → indicated conversion;
  - c_IR is indicated → reserve conversion;
  - P_dev is the development probability.
- Priors are Beta per deposit style × drill density × reporting code × price deck. They are learned from
  statement-to-statement flows in the company's own reserve history, junior filing histories and post-acquisition
  re-estimates.
- **Price decks differ.** Barrick reserves at $1,400 or $1,500/oz (FACT, R6 §1.3); its gold deck was $1,200
  (2020–21), $1,300 (2022–23), $1,400 (2024) and $1,500 (2025) (FACT, E143). Juniors and PEAs often use higher
  decks (Fourmile's $1,900; E141). Re-price before haircutting.

**The value function of an undiscovered deposit (the Lodestar interface; resolves OPEN-LODE-5 as DESIGN).**
- V(m) = E_ω[NPV of development | mark m = (metal, size, grade, depth, style, jurisdiction)]. It is built from:
  - capital-intensity reference classes (FACT, R6 §4.3);
  - the lead-time distribution;
  - P(develop | discovered);
  - the fiscal regime;
  - price paths.
- Every exploration play and junior stake is valued on the same scenario index as the operating assets.
- V(m) is a **model**, labelled `ModelledUncalibrated` until hindcast against real discoveries' outcomes (the
  Lodestar hindcast protocol, §11.6).

### 23.6 Executive pay metrics as constraints with published prices: the KPI price list

| Metric (weights FACT, R6 §1.5, §2.5) | Treatment | Why |
|---|---|---|
| AISC; Newmont adjusted CSC/GEO (35%) | Soft constraint per site-year; shadow price published | A ratio metric: high-grading, deferral or reclassification can move it without changing value (GB-26) |
| ROCE (Barrick 15% LTI, 10–15% range; Newmont 30% PSU) | 3-year floor constraint; shadow price | Penalizes long-gestation growth, because capital employed rises years before EBIT: a **structural bias against greenfield** made visible |
| Reserve replacement (3-year, 15% LTI) | Computed by `fmine-metrics` under one definition; a constraint only after the definition is reconciled (190% vs 504%) | Rises with acquisitions or a higher deck as well as with discovery |
| GEO (25% API) | Soft floor, on a named ratio basis (GB-27) | Volume can conflict with NPV; GEO moves with the price ratio |
| rTSR vs GDX (35% Barrick; 60% Newmont) | **Excluded from optimization**; context only | Not controllable; market-relative |
| FCF/share, dividend policy | Liquidity constraints and minimum-cash floor | Real harm when breached |
| Safety and critical controls | **Hard** constraints, never priced | E023; not tradeable (G-HELM-1) |

**The KPI price list (board artifact).** It answers questions such as:
- "Holding 2027 Au AISC ≤ $X/oz costs $Y M of E[NAV]."
- "Relaxing the ROCE floor from 12% to 10% admits greenfield G, worth +$Z/share."

Each answer is a constraint dual with a validity range from post-optimality analysis.

**Monitor M-HELM-8 (Goodhart alarm).** It fires when a KPI's shadow price exceeds 5% of programme E[NAV]. The
board sees the price, and FrankenMining never adjusts KPIs automatically.

### 23.7 How a board sees it

```text
┌ DC-2027-CAP-014  "FY27 growth envelope $800M"   color: ESTIMATED (weakest input: country hazard, elicited) ┐
│ Alternatives (incl. do-nothing, buyback): A+E | A+D | E+D | A+E_stream | hold cash                        │
│ E[ΔNAV] $M: 372 | 241 | 253 | 376 | 0      CVaR20: −210 | −131 | −283 | −150 | 0   P(Tier One E): 0.31    │
│ Selection frequency (200 bootstraps): A 0.97, E 0.62, D 0.41, E_stream 0.58  → "E is fragile"            │
│ Regret (mean/max): A+E 91/377 | A+D 222/701 | E+D 210/550                                                  │
│ Chosen under board risk appetite CVaR20 ≥ −150: A+E_stream (binding). Without the stream: A+D, −$131M     │
│ What would change it: E's S1 loss > $150M after stream; stream PV cost > $135M; P/NAV < 0.66 → A+D       │
│ KPI prices: AISC FY27 cap $1,650/oz → −$38M; ROCE floor 12% excludes a greenfield G (−$0.21/sh)           │
│ Reference-class adjustments shown: capex ×1.14 mean (mixture, GB-32), slip +1.2y; conversion h_pub 0.85  │
│ Best next information: EVSI(E metallurgy testwork) $41M vs cost $6M → fund before FID (GB-29)            │
│ Sign-offs required: QP/CP (resources), CFO (financing), Board. execution_authorized = false (E023)        │
└────────────────────────────────────────────────────────────────────────────────────────────────────────────┘
```

All numbers in this card are illustrative (GB-30).

### 23.8 The Tier One screen, juniors and deal structures

- **The Tier One screen is versioned.**
  - Gold: Barrick's definition at a $1,400 (Circular) or $1,500/oz (Q2 2026 release) deck, ≥ 10-year life,
    ≥ 500 koz/y, lower half of the cost curve.
  - Copper: $3.00 or $3.25/lb, ≥ 5 Mt, ≥ 20 years, ≥ 200 ktpa (FACT, R6 §1.3). The Sep 2025 copper definition
    ($3.00/lb, +5 Mt) is confirmed in the corpus (FACT, E134).
  - The output is **P(Tier One)** over the ensemble plus a distance vector, not pass/fail.
  - MinEx's Tier 1 definition is a different object, and the two are never mixed.
- **Junior investment logic.** The haircut machinery (§23.5) turns a junior's market price into the conversion it
  implies (GB-31). A staged earn-in buys *information*, and its value is option value shown separately.
- **Deal terms are recourse structures** (stake, top-up rights, ROFR, information rights, earn-in stages). The
  `ProgramOption{deal}` from Lodestar §11.5(E) is the same object.
- **MNPI.** Junior and M&A analysis is a walled lane. Public-basis and full-basis posteriors are kept separate
  (§11.7, §31.1), and every deal card carries its insider-list membership. Intra-JV transfers (Fourmile into NGM
  at fair market value; E139) are walled from the JV partner's view.

### 23.9 Monitors, guards, proof obligations, assumptions and gates

**Monitors:**
- M-HELM-5: capex and slip tracking. Alarm when P(final > AFE) > 0.5 under the project's posterior.
- M-HELM-8: the Goodhart alarm (§23.6).
- Country regime state changes trigger programme recomputation, with the liquidity floor binding first.

| Guard | Condition | Action |
|---|---|---|
| G-CAP-A | A capital card without reference-class adjustments shown | Refuse |
| G-CAP-B | Portfolio CVaR computed from marginals | Refuse |
| G-CAP-C | Selection frequency < 0.8 | Flag "fragile"; show runner-up |
| G-CAP-D | Option value merged into static NPV without a separate line | Refuse (option-value laundering) |
| G-CAP-E | Tier One definitions mixed | Refuse |

| PO | Statement | Verification |
|---|---|---|
| PO-PUSH-8 | The CVaR LP equals the sorted-tail average | Property test against `fs-robust::empirical_cvar` |
| PO-PUSH-12 | Calibration claims hold (binomial CI or e-process) | Ledger tests |
| PO-CAP-1 | Brute-force parity on ≤ 20 binaries | Known-answer test (GATE-CAP-01) |
| PO-CAP-2 | Per-share objective is invariant to share-count units; buyback ΔNAV formula reproduces hand examples | Known answers |

| ID | Assumption | Measured by | If false |
|---|---|---|---|
| A-CAP-1 | The scenario index captures the dominant correlations (price–cost, country–price) | Bootstrap stability; sensitivity to P/NAV, α and hazard multipliers | DRO over family weights |
| A-CAP-2 | Reference classes transfer to this company | Company random effect; M-HELM-5 | Widen; company-only after ≥ 10 projects |
| A-CAP-3 | The P/NAV model is adequate for buyback valuation | Sensitivity test | Report the P/NAV-conditional hurdle |
| A-CAP-4 | Conversion priors are learnable from statement histories | Data audit (OPEN) | Elicited priors, flagged `Elicited` |

| Gate | Threshold (TARGET) | When |
|---|---|---|
| GATE-CAP-01 | Brute-force parity on ≤ 20 binaries; CVaR LP equals the sorted tail | M4 |
| GATE-CAP-02 | Reference-class prior reproduces Bertisen & Davis's three statistics within ±0.03 | M4 |
| GATE-CAP-03 | Bootstrap selection frequency reported on every card | M4 |
| GATE-CAP-04 | Bitemporal hindcast of ≥ 5 past approvals (as known at approval) scored against realized outcomes; H7a vs H7d decisions compared | M4–M5 |
| GATE-CAP-05 | Synthetic exploration district: 50 synthetic decades comparing static NPV rank, Gittins heuristic, Thompson, knowledge gradient and staged-option policies on discoveries and NAV per $ | M4 |

### 23.10 Galaxy-brain cards (illustrative values)

**GB-30 · CVaR capital allocation, and why the buyback is not a hurdle rate.**
- **Set-up.** Five joint scenarios stand in for ≥ 2,000: S1 low price plus overrun; S2 low; S3 base; S4 high; S5
  country shock. The budget of $800M buys two $400M packages.
- **Packages (ΔNAV in $M by scenario).**
  - A, brownfield: [−60, 40, 220, 480, 220], mean 180.
  - E, greenfield in a fiscal-risk country: [−150, 60, 420, 960, −330], mean 192.
  - D, buyback at P/NAV 0.85, with own-NAV multipliers [0.70, 0.85, 1.0, 1.4, 0.95]:
    400(m/0.85 − 1) = [−70.6, 0, 70.6, 258.8, 47.1], mean 61.2.
- **Combinations.**
  - A+E: mean 372, CVaR −210.
  - A+D: mean 241.2, CVaR −130.6.
  - E+D: mean 253.2, CVaR −282.9.
- **Result.** NPV rank and minimax regret pick A+E (max regret 377 vs 701). Under the board's risk appetite τ =
  −150, A+D is chosen, so the **price of risk appetite is $130.8M**.
- **Adding a stream on E** (deposit $120M, scenario cost [60, 80, 130, 260, 50], roughly fair) gives E_s =
  [−90, 100, 410, 820, −260]. A+E_s has mean 376 and CVaR −150, and is **chosen**.
- **Intuition.** The buyback is a levered bet on the company's own NAV, not a risk-free hurdle. The stream is bought
  as tail insurance, not as cheap money.
- **Flip.** Stream PV cost > $135M → A+D. P/NAV < 0.66 → A+D overtakes on the mean. S5 hazard ×2 under DRO →
  re-rank.

**GB-31 · A junior's market price implies 93% conversion of inferred ounces.**
- **Equations.** R = h_pub(c_IR·M_ind + c_FI·c_IR·M_inf) and NAV = R·v·P_dev.
- **Substituted.**
  - M_ind = 1.0 Moz and M_inf = 3.0 Moz; h_pub = 0.85, c_IR = 0.75, c_FI = 0.45, v = $300/oz, P_dev = 0.65.
  - So R = 1.498 Moz and NAV = **$292M**, against a market capitalization of $350M.
- **Options.**
  - Acquisition at a 35% premium ($472.5M) → **−$180M**.
  - A 19.9% stake at market → −$11.5M.
  - **A 51% earn-in.** K₁ = $35M reveals c_FI, and the same drilling revises the development probability:
    {c_FI = 0.75, P_dev = 0.80 with p = 0.35, giving NAV $497M; c_FI = 0.29, P_dev = 0.55 with p = 0.65, giving
    NAV $197M}, then K₂ = $110M. (Prior-mean P_dev = 0.64 ≈ 0.65.)
    - Good branch: 0.51·497 − 110 = 143.6. Bad branch: 0.51·197 − 110 = −9.5, so walk away.
    - Value = 0.35·143.6/1.08 − 35 = **+$11.5M**, of which the staging option is worth $5.8M.
- **Acquisition break-even.** 0.85(0.75 + 2.25·c_FI)·300·0.65 = 472.5 gives **c_FI = 0.93**. The price implies
  that 93% of inferred ounces convert.
- **Intuition.** Buy information (the earn-in), not ounces.
- **Flip.** The company's own-history c_FI prior ≥ 0.6 with tight spread, or P_dev ≥ 0.85 → the gap narrows. The
  junior refuses terms → stake plus information rights.

**GB-32 · The reference-class capex prior needs a blowout regime.**
- **Targets** (X = final ÷ feasibility-study capex; Bertisen & Davis 2008): E[X] = 1.14; P(|X − 1| > 0.15) ≈ 0.5;
  P(X ≥ 2) ≈ 1/13.
- **A single lognormal fails.** Fitted to the mean and tail (μ = 0.020, σ = 0.472), it gives
  P(|X − 1| > 0.15) = **0.75**, not about 0.5.
- **A mixture works.** 0.923·N(1.043, 0.20) + 0.077·Blowout(mean 2.3) gives:
  - mean 1.140 ✓;
  - outside ±15%: 0.505 ✓;
  - P(X ≥ 2) ≈ 0.077 ✓.
- **Applied.** A $1.6B feasibility estimate has a mean of $1.82B and P90 ≈ $2.31B, with P(≥ $3.2B) ≈ 7.7%.
- **Intuition.** Overruns are a well-behaved core plus a separate blowout regime. One lognormal over-spreads the
  core to fit the tail.
- **Flip.** The company's own ≥ 10 approvals show a lighter blowout share → a narrower posterior. The class choice
  (megaprojects vs Bertisen & Davis) is itself a decision (OPEN-HELM-5).

**Open questions (B3 §8.4, carried):**
- **OPEN-HELM-1.** Conversion and h_pub datasets.
- **OPEN-HELM-2.** Stream and royalty market cost of capital.
- **OPEN-HELM-3.** Whether the CFO accepts P/NAV valuation of the company's own shares.
- **OPEN-HELM-4.** Is H-FISC (price-dependent fiscal hazard) supported by data?
- **OPEN-HELM-5.** Reference-class choice: megaprojects vs Bertisen & Davis.
- **OPEN-HELM-6.** Who signs the risk appetite τ? (The board; mechanism OPEN.)

## 24. Hallmark: the resource and reserve register, attestation and disclosure

Crate: `fmine-reserves`. Sources: R7 §4.2 (codes verified to October 2026), B1 (snapshot triple), B5 W7, E031.

### 24.1 Doctrine

- **FrankenMining never classifies resources or reserves.** `reserve_disclosure_claim` is hard-wired false
  (§0.4; NONGOAL-003). Classes (Measured, Indicated, Inferred; Proved/Proven, Probable) are qualitative labels a
  named Competent or Qualified Person assigns. None of the codes prescribes an estimation method or a
  probabilistic definition of the classes (FACT[U], R7 §4.2).
- **FrankenMining computes the evidence *for* the CP's decision** and records the decision as a signed, versioned
  human act with rationale: the `CompetentPersonAct`.
- **Store once, render many.** Estimates are stored once with full uncertainty and rendered by versioned rule
  sets per code, because the codes are in flux (2024–2026).
- **The regulatory tailwind is probable, not certain (R7 §4.2; E031).** The JORC exposure draft proposes a
  **mandatory reconciliation section** and defines **risk as the effect of uncertainty**; final wording and
  commencement are unverified and JORC 2012 remains in force. That aligns directly with Touchstone
  (§20.7) and the calibration ledger (§10).

### 24.2 Code status that shapes the renderers (FACT, R7 §4.2; B5 W7)

| Code | Status (October 2026) | Features the renderer must handle |
|---|---|---|
| JORC 2012 | In force | Table 1 checklist; CP sign-off |
| JORC 2026 (provisional) | Under parent-body, ASX and ASIC review; about a one-year transition after finalization; no commencement date | Draft content (wording unverified): "Specialist" role; reasonable prospects "more likely than not" with a documented Reasonable Prospects Assessment; **mandatory reconciliation section**; risk as the effect of uncertainty; rounding to reflect relative uncertainty |
| CRIRSCO Template (June 2024 update) | Current | "Eventual" removed; CP requires a recognised professional organisation with enforceable discipline and ≥ 5 years' relevant experience |
| NI 43-101 | Current form in force. Replacement proposed 12 June 2025, not final. | Proposal: "mineral project"; QP needs 5 years' relevant experience without the formal-education requirement; "relevant" replaces "material"; **specific disclosure of QP data verification per technical-report item** (B5 W7); more Indigenous, community and environmental disclosure |
| SEC S-K 1300 | In force since fiscal years from 1 January 2021 | Cut-off grade and price must be disclosed; technical report summary on first or materially changed disclosure; "proven/probable" |
| PERC 2021, SAMREC 2016 | In force | Renderer packs added on client demand |

A renderer for a code version not yet in force is labelled "draft standard" on every page (G-HALL-2).

### 24.3 The register model

```rust
pub struct EstimateStatement {                       // one per (deposit, effective date, basis)
    pub id: StatementId, pub deposit: DepositId, pub effective: Date, pub kind: StatementKind, // Resource | Reserve
    pub snapshot: SnapshotTriple,                    // (EvidenceCut, SpecHash, Vec<ProjectionReceiptHash>), §13.2
    pub process_inventory: Inclusion,                // Included | Excluded (PV excludes it; Veladero and Porgera include it)
    pub price_deck: DeckRef,                         // { evaluation: DeckRef, shell_selection: Option<{revenue_price, discount_rate}> }
    pub cutoffs: Vec<CutoffSpec>, pub attributable: OwnershipRef,
    pub resource_inclusion: Inclusive | Exclusive, pub rounding: RoundingRule,
    pub qp_relationship: Independent | Employee, pub item22: Included | ProducingIssuerExemption,
    pub rows: Vec<StatementRow>,                     // domain × class, with intervals for internal use
    pub modifying_factors: Vec<ModifyingFactorClaim>,// mining, processing, metallurgical, infrastructure, economic, marketing,
                                                     // legal, environmental, social, governmental: each a Claim with evidence
    pub rpee: Option<ReasonableProspectsAssessment>,  // documented assessment (JORC 2026 draft, CRIRSCO)
    pub cp_acts: Vec<CompetentPersonActRef>, pub mnpi: MnpiLabel, pub supersedes: Option<StatementId>,
}
pub struct StatementRow {
    pub source: SourceKind,                          // InSituOpenPit | InSituUnderground | Stockpile | ProcessInventory
    pub route: Option<ProcessRoute>,                 // HeapLeach | DumpLeach | Mill | FlotationPox | ...
    pub point_of_reference: PointOfReference,        // per row, not per statement: in-situ | ROM at crusher | plant feed | product
    pub metal_stage: MetalStage,                     // Contained, or Recoverable{card} for process inventory (§25.2)
    pub tonnes: Option<DryTonnes>,                   // None only for ProcessInventory (Porgera's 102 koz row has no tonnes)
    pub class: Class, pub grades: Vec<GradeQty>, pub metal: Vec<MetalQty>, pub intervals: Option<RowIntervals>,
}
pub struct OwnershipRef {                            // bitemporal
    pub valid_from: Date, pub valid_to: Option<Date>,
    pub interests: Vec<(HolderRef, Ratio, HolderKind /* ListedCorp | FederalSOE | ProvincialGovt | ... */,
                        InterestBasis /* Funded | FreeCarried */)>,
    pub control_basis: ControlRef,                   // separate from economic interest
    pub arrangement: InKind | Entity | MarketingAgent { agent: PartyRef },
}
pub enum ModifyingFactorClaim { /* ...as listed above..., */
    Tenure { instrument: InstrumentRef, expiry: Date,
             renewal_state: Granted | Applied | Pending { expected: Date } | NotApplied | Refused,
             depended_on_by: DateRange },
}
pub struct CompetentPersonAct {
    pub person: PersonRef, pub rpo: MembershipRef, pub relevant_experience: DocumentRef,
    pub scope: ActScope,                             // which deposits, domains, items
    pub decisions: Vec<ClassificationDecision>,      // volume → class, with rationale and the support pack consulted
    pub support_pack: ClassificationSupportPackRef,  // §13.11: evidence for, never a class
    pub signature: AttestationRef, pub date: Date,
}
```

- **Internal intervals vs disclosure.** Rows carry intervals internally. Disclosure renders what the code
  requires, with rounding per the code version.
- **Point estimates.** The disclosed point estimate is the CP's chosen estimator, usually `DeclaredOk` (§13.1).
  The belief-ledger e-type is shown beside it to the CP, but never substituted silently.
- **Reasonable prospects.** The RPEE assessment record captures the "more likely than not" reasoning the JORC
  2026 draft would require, with the pit shell or stope envelope at a declared deck as evidence.
- **Why rows carry the point of reference (C4 P2).** Real statements mix sources and bases: Pueblo Viejo states
  ROM delivered to the crusher and excludes process inventory (FACT, E261); Veladero includes pit, stockpile and
  leach-inventory rows, and its "contained" leach row arithmetically equals the recoverable-basis book (FACT,
  E274, E276); Zaldívar splits rows by route (E288). When a source label contradicts the arithmetic, the importer
  files a discrepancy row and never relabels silently.
- **Ownership is bitemporal (C4 P10; C5 §4.6).** Veladero went from 100% to 50% on 30 Jun 2017, inside a
  reported series, and its doré is sold to Barrick "as agent for" the partner (FACT, E273); Porgera was 95% (BNL) in
  2011; since 22 Dec 2023 it is 51% PNG stakeholders and 49% Porgera (Jersey) Ltd (50/50 Barrick–Zijin), so Barrick
  holds 24.5% equity but 23.5% of economic benefits, and the Q2/Q3 2024 summary sheets label it 47.5% while
  their values are at 24.5% (FACT, E293, E428–E432, E468, E469); Zaldívar's boundary agreement stockpiles ore for the other
  owner (E292). Reko Diq's province holds 15% funded and 10% free-carried, and documents disagree on who holds
  50% (FACT, E302, E303). Series that cross a change render per interval; disagreeing documents are
  `conflicted`; free-carried interests change attributable cash flow (§21) and capex funding (§23). Material lots
  carry `owner` where a boundary agreement exists.
- **Tenure (C4 P6; C3 §3.6).** G-HALL-6 highlights, in the QP pack and the §35 register, any scheduled reserve
  period after an instrument's expiry or dependent on a permit whose state is not `Granted`. Cases: Porgera's
  2012 reserve LOM ran past the 2019 SML expiry before any extension application had been made (FACT as of
  March 2012, E296); BNL applied in June 2017, the SML expired on 16 Aug 2019 and the State refused the extension
  on 24 Apr 2020 (FACT, E426, E425); Pueblo Viejo's reserve
  LOM to 2048–2049 depends on a Naranjo starter-dam permit not yet granted and about 680 households resettled
  (FACT, E269, E270); Kibali's permits expire 2029/2030 against a LOM to 2043 (FACT, E245). The software does not
  decide the class.
- **Reserve Claims cite the technical report or the annual reserve table**, never investor-deck endnotes: deck
  components can fail to sum to the deck's own total (C1 DR-13; E133). A secondary "reserve" (Reko Diq's ESIA
  quotes USGS 2011 "estimated reserves"; E310) is stored as `Assertion{source: secondary}`, and G-HALL-1/3 refuse
  any render of it as a reserve.
- **Issuer reserve rows are imported assertions (B7).** Through M2, public reserve and roll-forward figures are
  stored as `IssuerReserveRow` (`Assertion{source: issuer}`), with the price deck and the filing vintage as separate
  keys. They never enter `EstimateStatement`, which needs a snapshot triple and a CP act (G-HALL-3) that no public
  filing supplies. Where filings disagree, the first-filed row and the later one are both kept (E692: the 2024 GEO
  net change was printed as 6.7, then as 79), and a deck row is a cross-check, never the source (E698).
- **Imported column semantics are asserted, not read from headers.** Kibali's "Contained Gold" column holds
  recovered ounces (FACT, E240). Every imported column carries an asserted basis (contained or recovered; 100% or
  attributable) checked by an identity test (§32).

### 24.4 The classification support pack (evidence for the CP)

Per block, volume or domain, from Strata (§13.11):
- kriging efficiency and slope of regression;
- drill spacing vs variogram ranges;
- distance-to-data and extrapolation flags;
- simulation-based P(metal within ±t over a production period at confidence c), where (period, t, c) is a
  registered policy triple with named variants: quarterly/annual ±15% at 90%, and ±10% at 95% (E387). Pueblo
  Viejo uses ±15% at 90% on quarterly (Measured) and annual (Indicated) volumes and ±30% for Inferred (FACT, E263).
  The pack prints the caveat that ±15% can be too loose because margins rarely exceed 15% (E388), and the
  production-volume choice is a declared parameter, because NGM notes the result is "highly sensitive" to it and
  "assumes stationarity" (FACT, E159). This remains an industry heuristic, **labelled as such**. Internal non-CIM
  classes (Pueblo Viejo's "Inventory" class) are allowed and never render publicly;
- interpretation-hypothesis weights (L1): how much of the volume's metal depends on contested geology;
- the reconciliation track record for analogous mined domains (§20).

**What the pack never does:** emit a class label, a "suggested class" or a colour-coded class map. A
volume-by-metric view is allowed. A view that thresholds metrics into class names is refused (G-HALL-1), because
it would be a classification in all but name.

### 24.5 The data-verification evidence index

- **The proposed NI 43-101 replacement requires specific disclosure of QP data verification per technical-report
  item** (FACT, B5 W7).
- Hallmark assembles, per item, the evidence trail the QP needs:
  - drillhole database audit results (Corebox lineage: what was checked, against which originals);
  - QA/QC performance (CRM, blank and duplicate statistics, e-process alarms and their disposition; §12.4);
  - umpire results;
  - density and survey verification: the laboratory check fraction (default 5–10%; E382) and the density grid
    against an optimal grid per confidence class (density grids are far sparser than grade grids; E383);
  - site visit records;
  - check sampling;
  - limitations and unresolved issues;
  - target vs achieved insertion rates with denominators, per-lab failure rates, "control absent" periods, the
    no-certificate share, `qaqc_regime` per campaign, and audit findings (RC vs DD bias, porosity assumptions, a
    "lack of Data Quality Objectives"; E164) (§12).
- Each entry links to signed originals in the CAS (lab certificates, survey reports).
- Disclosure depth differs by report: Carlin gives counts and failure rates, while Turquoise Ridge and Cortez give
  rules only (C2 §4.5f). That gap is what the index fills.
- **Why this is a strong product (B5).** It turns the QP's most tedious and liability-laden task into a
  reproducible, auditable index. It is valuable under any code, and it creates no classification risk.

### 24.6 Renderers

| Renderer | Output (always a draft for the CP/QP) |
|---|---|
| `render::jorc2012` | Public report tables; Table 1 checklist populated with evidence links, with gaps marked |
| `render::jorc2026_draft` | As above, plus the reconciliation section from Touchstone (§20.7) and the RPEE record. Labelled "draft standard". |
| `render::ni43101` (current) and `render::ni43101_proposed` | Technical-report item skeletons with the data-verification index (§24.5) |
| `render::sk1300` | Summary and individual-property disclosure tables with cut-off and price; technical report summary skeleton |
| `render::crirsco` | Template-aligned tables |
| `render::internal` | Intervals, hypothesis dependence, sensitivity ladders (MNPI; never public) |

**Every renderer is a pure function of (statement, code version, rounding rules)** (PO-HALL-1). The system never
files anything. Filing is a human act recorded as a `DisclosureAct`. Tests: render Pueblo Viejo's 2026 rows from
unrounded internal rows and match the published two-significant-digit table (E262); recompute Carlin's
attributable column at 61.5%, which catches its internal errors (E153, E193).

### 24.7 Statement-to-statement reconciliation

- Year-over-year changes are decomposed exactly:
  - depletion (from Touchstone's as-mined volumes and posteriors);
  - model updates (new drilling; interpretation changes);
  - price-deck and cost changes;
  - cut-off changes;
  - conversion between classes;
  - acquisitions and divestments;
  - ownership changes (attributable).
- Model and deck effects use the same order-independent decomposition as the AISC waterfall (§21.6).
- **Reserve replacement** is computed from this waterfall under each named definition (§21.6).
- The statement histories also train the conversion priors (c_FI, c_IR) used in capital allocation (§23.5).
- **DATA-GATE.** Public bridges give only net change per deposit with qualitative drivers (Carlin 2023 → 2024),
  so the exact decomposition cannot be reproduced from public text (C2 §4.7).

### 24.8 Links to accounting and capital

- A new reserve statement enters FA as `ImportDocument(kind Disclosure)` plus the QP's `Attest`.
- DD&A units-of-production rates reprice prospectively from the effective date (§21.5 part 4).
- Impairment indicators, stream drawdown and ARO timing read the same statement (§21.5).
- Reserves are evaluated at the **disclosure deck**. A ladder of other decks is internal sensitivity, labelled
  MNPI, and feeds Helm (§23).

### 24.9 Guards, proof obligations and gates

| Guard | Condition | Action |
|---|---|---|
| G-HALL-1 | Software emits a class label, suggested class or thresholded class map | Refuse (E023, E031) |
| G-HALL-2 | A renderer for a code version not in force | Every page labelled "draft standard" |
| G-HALL-3 | A statement without its snapshot triple and CP act | Refuse the render |
| G-HALL-4 | S-K 1300 render without cut-off grade and price | Refuse |
| G-HALL-5 | Internal sensitivity content reaches a public renderer | FMINE-REF-301 (MNPI wall) |
| G-HALL-6 | A scheduled reserve period falls after a tenure expiry, or depends on a permit whose `renewal_state` is not `Granted` | Highlight in the QP pack and the §35 register; never decide the class |

| PO | Statement | Verification |
|---|---|---|
| PO-HALL-1 | Renderers are pure functions of (statement, code version, rounding rules) | Golden tests per code |
| PO-HALL-2 | Statement-to-statement reconciliation sums exactly and is order-independent | Property tests |
| PO-HALL-3 | The snapshot triple regenerates the declared-estimator projection bit-identically | GATE-STRATA-G5 |

| Gate | Threshold (TARGET) | When |
|---|---|---|
| GATE-HALL-01 | Public lane: a published reserve reconciliation (Barrick or Newmont) reproduced from filings, with every unexplained difference listed. First fixture: Barrick's filed 2019–2025 roll-forward (Q4 2024 MD&A p.77; Q4 2025 MD&A p.71), with the Fourmile deck p.14 as a cross-check that differs by 0.1 Moz on 2025 depletion and net change (E698); the engine flags the printed 2025 copper net-change sign (closure gap 263 Mlb; E144), the 2024 GEO row's price-basis mismatch (E147) and the first-filed 2024 GEO net change of 6.7 against a total that implies 79 (E692) as discrepancy items with closure arithmetic, never silently corrected | M0–M1 |
| ✖ GATE-HALL-02 | Independent QP replay from the snapshot triple in ≤ 1 day (= GATE-STRATA-G5) | S+12 (month 24). Kill: revert to the declared-estimator authority, with the belief ledger as advisory. |
| GATE-HALL-03 | The data-verification index accepted by a pilot QP as usable for a technical report | M3 |

### 24.10 Galaxy-brain card (illustrative values)

**GB-33 · The "±15% at 90% over a quarter" heuristic turns on one correlation.**
- **Practice anchor.** Pueblo Viejo uses exactly this rule, ±15% at 90% on quarterly (Measured) and annual
  (Indicated) volumes (FACT, E263); a textbook proposal gives the same triple and warns it can be too loose (E387,
  E388).
- **Equation.**
  - The requirement is CV_quarter ≤ 0.15/1.645 = 0.0912.
  - With SMU-level relative error CV = 0.6 and N = 2,000 SMUs mined per quarter, CV_quarter = 0.6/√n_eff, where
    n_eff = N/(1 + (N − 1)·ρ̄) and ρ̄ is the average error correlation between SMUs.
  - The requirement needs n_eff ≥ 43.3.
- **Substituted.**
  - ρ̄ = 0.01: n_eff = 95 and P = 0.985. Passes.
  - ρ̄ = 0.02: n_eff = 49 and P = **0.919**. Passes.
  - ρ̄ = 0.025: n_eff = 39 and P = 0.883. Fails.
  - ρ̄ = 0.03: n_eff = 33 and P = **0.848**. Fails.
- **Intuition.**
  - Whether a volume "supports Measured" under this heuristic hinges on the long-range error correlation, which
    comes from the variogram's longest structure and the drill spacing. Small, defensible modelling choices move
    it across the line.
  - That is why the pack shows the metric *with its sensitivity to the variogram particles* (§13.3), and why the
    class is the CP's call.
- **Assumptions.** Equicorrelated errors (simulation computes the real thing directly); Gaussian quarterly error.
- **Flip.** Closer drill spacing reduces both CV and ρ̄. The VOI of infill drilling for classification is a §22
  card.

**Open questions.**
- **OPEN-HALL-1 (= OPEN-STRATA-2).** Will QPs sign `DeclaredOk`, the simulation e-type, or both side by side?
- **OPEN-HALL-2.** Final wording and commencement of JORC 2026 and the NI 43-101 replacement.
- **OPEN-HALL-3.** Whether regulators or auditors require retained realizations for disclosed snapshots
  (OPEN-STRATA-4).
- **OPEN-HALL-4.** Liability posture: does a data-verification index prepared by software change the QP's
  liability? Counsel review is needed before M3.


# PART IV — SUBSTRATE, SUITE AND ENGINEERING

Part III described the mine. Part IV describes the machine underneath it. Each section answers "what does the
code stand on, and why can it be trusted?" The order matters: units and time (§25) come before storage (§26),
because every durable format encodes quantities and times, and a format that gets them wrong cannot be repaired
by better storage.

## 25. Units, money, time and coordinate reference systems

### 25.1 Why this is a section and not a utility module

Many material reconciliation failures in mining are not modelling failures but basis failures (HYPOTHESIS,
tested by the M2 parity reproduction and GB-34):
- wet tonnes treated as dry;
- `%` Cu averaged by truck count;
- ounces per short ton read as grams per tonne;
- a mine grid read as UTM;
- a month-end cut at midnight UTC instead of at the site's production-day boundary.

Each produces a believable number that is wrong by a few percent. That is exactly the size of the signal §20
tries to detect. FrankenMining therefore makes *basis* part of the type, and refuses at the boundary rather than
converting silently (INV-014).

### 25.2 `fmine-units` (S0, std-only, stable Rust)

**Lineage.** It is modelled on frankensim `fs-qty`, but **not lifted**: fs-qty needs `generic_const_exprs` on
`nightly-2026-07-06` (E001, B4 §4.3). `fmine-units` uses ordinary const generics and marker types on the suite
toolchain, with no nightly features.

**Type shape (DESIGN).**

```rust
#![forbid(unsafe_code)]
/// A quantity is a value + a dimension + a basis. Basis is not a unit; it is what the number is *of*.
pub struct Qty<D: Dim, B: Basis, R: Repr = f64> { v: R, _p: PhantomData<(D, B)> }

pub enum MassBasis { Wet, Dry }                                       // tonnes of what moisture state
pub enum MaterialOf { Ore, Waste, Concentrate(ConcId), Dore, Cathode, Bullion, Reagent(ReagentId),
                     HazardousByproduct(HazardId) /* e.g. Hg from retorts; custody records, §17.7 */ }
pub enum MetalStage { InSitu, Mined, Contained, Recoverable { card: CardRef }, Recovered, Payable, Sold }
                                                                      // §21: each is a distinct type
pub enum ChemBasis { AsN, AsNO3, AsCaCO3, AsElement, Dissolved, Total } // what a concentration is "of"

pub type DryTonnes       = Qty<Mass, Dry>;
pub type WetTonnes       = Qty<Mass, Wet>;
pub type MetalMass<S>    = Qty<Mass, Metal<S>>;                       // grams internally; oz_t and lb are views
pub type Grade           = Qty<MassFraction, DryBasis>;               // g/t ≡ ppm; % Cu; never Add
pub type Recovery        = Qty<Fraction, ProcessStep>;                // 0..1; a change is PercentagePoints
pub type PercentagePoints = Qty<FractionDelta, ()>;                   // "recovery up 2 pp", not "up 2%"
pub type SpecificEnergy  = Qty<EnergyPerMass, Dry>;                   // kWh/t (BWi, SAG specific energy)
pub type BankVolume      = Qty<Volume, InSitu>;                       // bcm; loose volume (lcm) is a different basis
pub type Density         = Qty<MassPerVolume, DryInSitu>;             // a Claim with method; §13
```

**Arithmetic laws compiled into the types.**

| Law | Type-level rule | Runtime refusal (when data arrives untyped) |
|---|---|---|
| Grade is intensive | `Grade` has no `Add`. The only average is `mass_weighted_mean(&[(Grade, DryTonnes)])`. | FMINE-REF-103 (non-additive average) |
| Metal is extensive | `MetalMass<S>` adds only with the same stage `S`. `Contained + Recovered` does not compile. | FMINE-REF-105 (basis mismatch) |
| Contained metal | `Grade × DryTonnes → MetalMass<Contained>`. `Grade × WetTonnes` does not compile. | FMINE-REF-105 |
| Wet to dry | **No law exists.** `WetTonnes → DryTonnes` requires a `MoistureClaim` for *that lot* (§25.3). | FMINE-REF-105; FMINE-REF-403 for shipments |
| Volume to mass | `BankVolume × Density → DryTonnes`, where the density Claim carries method and support. Loose volume needs a swell Claim. | FMINE-REF-102 (support mismatch) |
| Recovery | `MetalMass<Contained> × Recovery → MetalMass<Recovered>`. Recovery applies at processing support only. | FMINE-REF-102 (G-CRUC-2) |
| Recoverable inventory | `MetalMass<Contained> × card → MetalMass<Recoverable{card}>`. A `ProcessInventory` row declares `Contained` or `Recoverable` (heap books are usually recoverable-basis; E275, E276). `Recoverable` never adds to `Contained`. | FMINE-REF-105 |
| Chemical basis | `Qty<MassConc, AsN>` meets a limit typed `AsNO3` only through a registered stoichiometric law (× 4.427); likewise as-CaCO3 and dissolved vs total. Water, effluent and assay limits are typed the same way. | FMINE-REF-105. Fixture: Reko Diq FS13 nitrate, reported "within limits" yet 59.3 mg/L as NO3 against ≤ 50 (E322–E324) |
| Mass fraction | An analyte mass fraction lies in [0, 1] on import (Porgera's database held S > 100%; C4 P12). | Refuse the row; discrepancy entry |
| Payable | `Payable` is produced only by contract terms in `fmine-acctpack` (payable %, minimum deductions, penalties). | — |
| Percent | `%` used as a ratio and percentage points used as a delta are different types. | — |
| Non-additive geomet attributes (BWi, recovery, Ai) | Mixing needs a registered `MixRule` (§13.6, §17.4), or the mix is refused. | FMINE-REF-103 |

**Exact constants.** Conversion factors that are exact by definition are stored as rationals, and a conversion
rounds **once**, with a declared rounding rule:
- 1 troy ounce = 31.1034768 g;
- 1 lb = 0.45359237 kg;
- 1 short ton = 2,000 lb = 0.90718474 t;
- 1 oz/st = **240/7 g/t** (≈ 34.2857). A historical US resource quoted in opt is converted exactly, not with
  "34.29".

**Representations.**
- Engines compute in `f64` (`Repr = f64`), under the determinism classes of §29.
- Accountable values crossing into the ledger or FA use `ExactQty`: an i128 scaled decimal with a declared scale
  per dimension (milligrams for metal mass, grams for tonnage, 10⁻⁹ for fractions).
- The conversion `f64 → ExactQty` is an explicit, logged rounding step and never implicit.

**Money.**
- `Money { ccy: Currency, minor: i128 }` is a **boundary type** only. FrankenMining never computes authoritative
  balances; FA does (§21).
- FX is always a Claim with source, date and rate type (spot, average, closing). There is no implicit
  conversion, and two currencies never add.
- Prices (`$/oz`, `$/lb`) are `Money`-per-`MetalMass` and come from a versioned price deck (INV-013).
- At the bridge, `Money` maps one-to-one onto FA's amount type. FA's `units.toml` has no metals (B4 S6), so
  metal *quantities* travel as Statistical-account rows (§21.4) until the FA unit ask lands (Appendix F).

**Import boundary.**
- Untyped numbers enter only through `Portal` (§32), where a dialect row must declare unit, basis and support for
  every column.
- An undeclared unit is stored as `UNREGISTERED_UNIT`, is viewable, and cannot be combined (FMINE-REF-107).

### 25.3 The wet/dry rule and moisture as a per-lot measurement

- A **lot** is the unit of moisture measurement: a truck-load group, a stockpile reclaim campaign, a concentrate
  parcel, a shipment or a filter-cake batch. Moisture is a Claim per lot, with a sampling method, a drying method
  and a time.
- Moisture is never a site constant. When only a *planning* moisture exists, the dry tonnes are a Claim of evidence
  class `Assumed`, with the UU term priced by the moisture variance observed in reconciled history (§9.3).
- **Concentrate.** Wet-to-dry conversion for sale uses the contract's moisture determination. For shipment
  release, FMINE-REF-403 (moisture vs TML) applies (§17.6).
- **Why this matters (GB-34).** A constant moisture assumption is a hidden bias that §20 would attribute to the
  wrong node (mill vs mine).

### 25.4 `fmine-time` (S0)

§6.3 fixes the quad-temporal semantics (`QuadTemporalStamp`, anchors, `as_of_decision` vs `for_physical_period`). This crate supplies the types.

| Type | Meaning | Rule |
|---|---|---|
| `RecordTime` | `(cell, seq)` plus `TimeEvidence` for the wall clock | Ordering across cells is **never** by wall clock, only by anchors and cross-cell references (INV-012) |
| `ValidTime` | Interval `[from, to)` in UTC nanoseconds (i64) with a granularity tag | An instant is a zero-width interval at its granularity; used for `physical_valid_time` |
| `CollectionTime` | UTC nanoseconds (i64) plus `TimeEvidence` | Physical acquisition instant of sample, core interval, survey pickup, or sensor frame (`collection_time >= physical_valid_time.from`) |
| `AvailabilityTime` | UTC nanoseconds (i64) plus `TimeEvidence` | Instant when lab assay certificate, QA/QC disposition, or processed reading became operationally observable (`availability_time >= collection_time`) |
| `QuadTemporalStamp` | `{physical_valid_time: ValidTime, collection_time: CollectionTime, availability_time: AvailabilityTime, record_time: RecordTime}` | Mandatory on observations and Claims (§6.3, §8.1). Operational hindcasts and short-term decision backtests filter on `availability_time <= t_dec` (**INV-016**, **G-TIME-2**); physical period closes group by `physical_valid_time`. |
| `TimeEvidence` | `{source: Gnss \| Ptp \| Ntp \| DeviceRtc \| Manual, offset_bound, sync_status, observed_at}` | Clock uncertainty is explicit. A manual time carries a ≥ 60 s bound by default. |
| `SiteCalendar` | IANA zone id, production-day boundary (e.g. 06:00 local), shift pattern, month-end cut rule | A registry row per site (`registries/calendars.toml`). DST rules are data. Leap seconds are not represented; the ≤ 1 s ambiguity is recorded in `TimeEvidence`. |
| `FiscalCalendar` | Issuer fiscal periods and close calendars | Matches FA periods exactly (FA-ACC rows, §21.3) |
| `Roster` | FIFO and residential rotation patterns (14/7, 8/6, 4×4 …) | Hours rules are hard constraints (§19, GB-23) |
| `Clock<'cx>` | Time read through `Cx` capability `TIME` only | K and E crates have no `TIME` capability (§26.6). Engines receive times as inputs, so replay is deterministic. |

**No chrono or time crates in FrankenMining code.** Both are banned as direct dependencies and in public APIs
(§28.5, L-5); they appear only transitively inside foundations, on an explicit allow-list. The few calendar algorithms needed (civil-from-days,
zone offsets from a registry snapshot, ISO weeks) are std-only, with known-answer tests against a pinned oracle
table.

### 25.5 `fmine-crs` (S0)

| Concern | Design |
|---|---|
| Scope | An EPSG subset, not a PROJ port: geographic WGS 84, WGS 84 / UTM zones, the national datums actually used at operating sites (for example Arc 1950/1960 in East and Southern Africa, NAD83 in Nevada, PSAD56 legacy data in South America), and their registered Helmert transforms with stated accuracy |
| Mine grids | A local grid is a fitted transform (2-D similarity or affine plus vertical offset) from surveyed control points. The transform **is a Claim**: RMS residual, control-point hull as its validity domain, surveyor attestation. Use outside the hull is flagged (G-CRS-2). |
| Vertical | Ellipsoidal vs orthometric heights are distinct types. Mine RL = orthometric + registered offset. Geoid grids are CAS objects with provenance. |
| Unregistered | Data imported without a declared CRS keeps raw coordinates tagged `UNREGISTERED`. It is viewable in its own frame. Any combination with registered data is refused (FMINE-REF-106). |
| Canonical encoding | f64 metres in a declared CRS; for hashing, canonical bit patterns (no −0.0, no NaN) |
| Oracle | PROJ in the lab lane only (§33.2), with known-answer transforms at ≤ 1 mm for projection math; datum transforms checked against the EPSG-stated accuracy |
| Units in coordinates | Feet-based legacy grids (US survey foot vs international foot) are distinct units. The difference is 2 ppm, which is 2 cm over 10 km. |

### 25.6 Guards and refusal codes added by this section

The §8.4 code table gains four domain-law refusals:
- **FMINE-REF-105:** basis mismatch (wet/dry, metal stage, material).
- **FMINE-REF-106:** unregistered or incompatible CRS/datum combined.
- **FMINE-REF-107:** undeclared unit, basis or time zone on import.
- **FMINE-REF-108:** assay-availability lookahead (`AssayLagLookahead`, `availability_time > t_dec`; §6.3, G-TIME-2).

| Guard | Trigger | Action |
|---|---|---|
| G-UNIT-1 | Grade averaged by count, volume or wet mass | FMINE-REF-103 |
| G-UNIT-2 | Wet tonnes reach a contained-metal calculation without a lot moisture Claim | FMINE-REF-105 |
| G-UNIT-3 | A conversion factor typed by a user (not a registered exact constant) | Refuse; the constant must be registered |
| G-UNIT-4 | An imported `CorrelationCard` mixes input units (g/t and oz/st), or its output scale (fraction vs %) is unparsed, or its formula fails `parse_check` | Refuse the card (§17.3; E171) |
| G-UNIT-5 | A document quantity with a stated derivation fails replay on import: a dimension mismatch, or an input that conflicts with the same document | Discrepancy entry. Fixtures: a pipeline "0.0038 km²" that is the pipe's internal volume in m³ (E347); fuel and GHG at 8 h/day for a site lit 24 h (E346) |
| G-CRS-1 | `UNREGISTERED` coordinates joined to registered data | FMINE-REF-106 |
| G-CRS-2 | A mine-grid transform used outside its control hull | Claim colour `Estimated` plus a UU term; a decision-grade use refuses |
| G-CRS-3 | Coordinates with no stated CRS | A CRS recovered by fit is a Claim with its residual, never silently assigned (Reko Diq's Table 3-1 fits EPSG:32641 within 0.1 m; E314) |
| G-TIME-1 | A period aggregate built on a UTC midnight boundary for a site with a different production-day boundary | Refuse; use `SiteCalendar` |
| G-TIME-2 | An operational hindcast, forecast registration, or short-term dig-line backtest (`as_of_decision(t_dec)`) conditions on an assay or observation with `availability_time > t_dec` | Refuse (`FMINE-REF-108: AssayLagLookahead`; INV-016) |

| PO | Statement | Verification |
|---|---|---|
| PO-UNIT-1 | Every exact conversion round-trips to within one unit of the declared scale | Property tests over random quantities |
| PO-UNIT-2 | No public function in K/E crates takes a bare `f64` for a physical quantity | `xtask check-claims` lint (INV-001) |
| PO-UNIT-3 | Mass-weighted means are invariant to splitting and merging lots | Metamorphic tests (§33) |
| PO-CRS-1 | Registered transforms agree with the PROJ oracle within stated accuracy over a 10⁴-point corpus per site CRS | Lab fixtures |
| PO-TIME-1 | Calendar algorithms agree with the pinned oracle table over 1900–2200 | Known-answer tests |
| PO-TIME-2 | `as_of_decision(t_dec)` excludes every record with `availability_time > t_dec` even when `physical_valid_time <= t_dec` | Property test with synthetic 1–21 day lab-turnaround delays |

### 25.7 Galaxy-brain card

**GB-34 · A constant moisture assumption manufactures a "mill problem".**
- **Equation.** dry = wet × (1 − m). The contained-metal error is Δdry × grade, and the revenue error is
  Δdry × grade × recovery × price.
- **Substituted.**
  - One month of 1,000,000 wmt of ROM ore at 1.2 g/t.
  - Planning moisture 6% gives 940,000 dmt. The measured lot moisture is 9%, giving 910,000 dmt.
  - Δ = 30,000 dmt → 36,000 g = **1,157 oz contained**, and 1,042 oz recovered at 90%. At $3,000/oz that is
    **$3.1M per month** (≈ $37.5M per year).
  - In §20 this appears as a 3.2% gap between the mine-side tonnes claim and the mill feed.
- **Intuition.** An unmeasured moisture difference looks exactly like a mill under-recovery or a weightometer
  bias. If moisture is a constant, the reconciliation posterior will attribute it to the wrong node, and it may
  refuse attribution altogether (FMINE-REF-104), because the moisture and weightometer biases are not separately
  identifiable without lot moisture data.
- **Assumptions.** A single blended moisture; no weightometer bias; a constant grade.
- **Flip.** Per-lot moisture sampling makes the two biases identifiable. Its VOI is a §22 card, and is usually
  large relative to sampling cost.

## 26. Bedrock: authority cells, objects, sync and the runtime

Source: Brenner pass B4 (`planning/brenner/B4_substrate_and_integration.md`). Each verdict below survived a
refuter-first test, and each has a cheap discriminating experiment and a kill gate.

### 26.1 Verdict: one authority mechanism

FrankenMining has **one** authority mechanism for its own facts:
1. an append-only **delta ledger** partitioned into single-writer **authority cells**; and
2. a **content-addressed object store** (CAS) for large immutable objects: 4 MiB chunks, a fanout-16 Merkle tree
   per object, and **root-last** publication.

fgdb, fsqlite and frankensearch are **derived projections only**: rebuildable, high-water-marked and never
consulted as truth. FA is a separate product and the book of record (§21).

**Hypotheses tested (B4 SQ1).**

| ID | Hypothesis | Outcome |
|---|---|---|
| H1-A | fgdb as authority for evidence-shaped contexts | **Falsified for 3 of 5 contexts.** Assays have writers at more than one node; drillhole intervals need a spatial index, which fgdb rejects for 1.0 ("a declared index over a `Point` property fails closed", S8; a post-1.0 Z-order route is recorded upstream); reserve effective dates need valid time before fgdb ships it. Splitting authority across two engines for the remaining two would recreate FA's "two storage engines in one book" smell. |
| H1-C (forced third) | fsqlite as relational authority | **Falsified as authority.** `FOR SYSTEM_TIME AS OF` works only for `:memory:`, so recorded-time history would be hand-rolled, and federation would rest on last-writer-wins changesets, which the doctrine forbids for semantic state. **Retained as the best projection store.** |
| H1-B | One FM-owned mechanism; everything else projects | **Survives**, conditional on GATE-STORE-01, with KILL-4 as the fallback |

**Commit-time witnesses without a database.**
- A batch carries its deltas plus **witnesses**: assertions about the state it was computed against, for
  example "sample `smp_X` exists and is not superseded" or "the conservation residual is within tolerance".
- The cell's **narrow combiner** checks every witness at commit against an in-memory keyed index, which is rebuilt
  from the cell log at open. A failed witness refuses the batch (exit 6), and the caller recomputes.
- This is optimistic concurrency without a database. The largest cell's working set is a site assay cell at about
  10⁷ intervals × 48 B ≈ 0.5 GB (TARGET; RISK-16 (scale) covers sharding by project or year if it grows).

### 26.2 Per-context authority (cells)

A cell is `(context, scope)` with exactly one writer node. Cell classes use the `CELL-` prefix.

| Cell class | Context | Authority | Projections (derived) | Write rate (TARGET) | Scope |
|---|---|---|---|---|---|
| CELL-EVID | Evidence and claims | Delta ledger (`FMT-LEDGER-1`); originals in CAS | fgdb evidence graph (GQL, `FOR SYSTEM_TIME AS OF SEQ`, `CALL fnx.*`); frankensearch (`quill` + hash only) | 10–10³ batches/day/site | per site; per enterprise function |
| CELL-DRILL | Drillholes, samples, assays, QA/QC | Typed ledger records (collar, survey, interval, sample, dispatch, result, QC insertion, certificate); lab originals in CAS | fsqlite tables + **R-tree over 3-D interval boxes**; desurveyed traces as CAS objects; fgdb lineage | 10³–10⁴ results/day/site | per site (brownfield); per project (greenfield) |
| CELL-BELIEF | Subsurface belief objects | **CAS** roots named by the ledger, with lineage, ModelCard and evidence colour | in-memory `VdbGrid` tiles; grade–tonnage tables in fsqlite | 1–10 roots/day; objects 10⁸–10¹⁰ B | per model domain |
| (none) | Spatial indexes | **Never authority** | fsqlite R-tree/geopoly; owned BVH/VDB; Z-order keys | rebuild only | per projection |
| CELL-TEL | High-rate telemetry | **Site segment log**: columnar CAS segments + per-source index. *Transitions* (load, dump, delay, TARP state) go to the site ledger. | fsqlite segment catalog; 1-min rollups replicated upstream | 10³–10⁵ msg/s/site | per site per source family |
| CELL-PLAN | Plans and schedules | Ledger (versions, scenario labels, approvals) + CAS (shells, schedules, designs) | fsqlite (period queries); fgdb (plan↔evidence lineage) | 10–10² versions/day | LOM: enterprise; short-term: site |
| CELL-FLOW | Material-flow and reconciliation | Ledger with **conservation at commit** (PO-BED-6, INV-005) | fsqlite; FA external summaries (§21.3) | 10²–10³ movements/day/site | per site |
| CELL-RSV | Reserves register and QP/CP attestations | Enterprise ledger cell; attestation records; reports in CAS | fgdb; FA disclosure documents and Statistical accounts | 10⁰–10¹ per quarter | enterprise (regional where residency requires) |
| CELL-DEC | Decision ledger | Enterprise cell; rows typed as S0 `DecisionRow` + `EvidenceRow` (FMT-LEDGER-1), schema-compatible with `franken_decision::DecisionAuditEntry` and `franken_evidence::EvidenceLedger` through `fmine-decide`'s converter, so the format never depends on a runtime version | fgdb | 10⁰–10² per day | enterprise |
| CELL-POST | Posting slots | Enterprise finance node cell; slot records and FA receipts only | — | §21.3 | per FA book |
| CELL-QUAR | Quarantined submissions | Agent uploads, Avathon write-backs, foreign imports pending review | none until adopted | — | per source |
| (FA) | Accounting | **FA book** (fgdb inside FA), a separate product | — | §21 | per FA book |

**The census rule.** Every context gets exactly one writer scope. The only contested case was assay *corrections*
made at the enterprise. They are routed as **correction requests** (evidence in an enterprise cell) that the site
writer adopts or refuses. There are no shared writes.

### 26.3 Leases, fencing and cross-cell references

- **INV-CELL-1 (= INV-004).** Each cell has exactly one lease-holding writer. A **fencing epoch** rises on every
  lease transfer, and it is persisted before the lease is used.
- Every batch carries `(cell, epoch, seq, prev_hash)`. Receivers refuse an epoch lower than the highest they have
  seen (PO-BED-1).
- **Lease transfer** (for example, moving an exploration project cell from a regional node to a new site node):
  1. the old holder seals and publishes its last batch;
  2. a transfer record names the new holder and epoch + 1;
  3. the new holder starts writing only after that record is anchored.

  A crash in the middle leaves the cell read-only until `fmine doctor` confirms which epoch is live.
- **Split brain** (two writers) shows up as an epoch regression at a receiver. Lower-epoch batches are quarantined
  and a conflict record is opened for human resolution. Nothing is merged automatically.
- **Cross-cell references** are `(cell, seq, digest)` triples. A consumer resolves them only at an anchor it
  holds, otherwise the knowledge state is `stale` or `not_observable` (INV-012).
- **The gluing check.** The enterprise view is the union of site logs, and it is coherent iff every cross-cell
  reference resolves.
  - This compiles to a dangling-reference checker that runs on every anchor publication. A dependent Claim whose
    reference dangles is not published.
  - This is the only use of descent/sheaf mathematics in Bedrock. Anything beyond a checker would be ornamental,
    and is not adopted.

### 26.4 Topology: hybrid cells with certified sync

**Hypotheses tested (B4 SQ2).**
- **H2-A, central enterprise authority, is falsified by arithmetic and safety.**
  - Raw telemetry at 10⁵ msg/s × 200 B is 160 Mbit/s, which is 3–16× a 10–50 Mbit/s satellite uplink.
  - Sites must keep TARP and QA/QC obligations live through multi-day outages.
  - Residency law forbids centralizing some data classes.
- **H2-B, site-sovereign everything, is falsified for enterprise-born facts.** LOM plans, the reserves register,
  decisions and postings are created centrally. Giving each site a writable copy invents multi-master conflicts.
- **H2-C, hybrid per-context cells, survives.**

**Node roles.**

| Role | Holds leases for | Notes |
|---|---|---|
| `site` | Site cells (drill, flow, telemetry, short-term plans, site evidence) | ISA-95 Level 3/3.5 only, never Levels 0–2. Accepts no inbound Internet connections. |
| `regional` | In-country enterprise cells where residency requires it | One per residency jurisdiction that needs it |
| `enterprise` | LOM plans, reserves register, decisions, posting slots | Co-located with the FA book it posts to (`fad` takes the book writer lease, B4 S6) |
| `relay` | Nothing | Store-and-forward only; useful on intermittent links |

**Sync design.**
- Pull-based from the enterprise and relay side, through **outbound-initiated** sessions from the site (the
  IEC 62443 conduit pattern).
- Logs reconcile by **version vector** in O(#cells). Objects reconcile by plain chunk manifests (want/have).
- Every applied range is checked against a **`SyncCert`** (§26.9 A2), which proves the replica is a prefix of the
  writer's log.
- CAS chunks move over QUIC/TLS through ATP. RaptorQ is consumed as is (it adds ≈ 2–5% on lossy GEO links).
- **Raw telemetry is site-sovereign** and never leaves the site by default. 1-minute rollups replicate.
- The **residency planner** treats `registries/residency.toml` as a hard constraint and refuses a transfer before
  any bytes move (§31.2).

### 26.5 Persistence ladder and durable formats

**Ladder (after FSS §13.2).** The same semantic test suite runs against every rung.

| Rung | Implementation | Status |
|---|---|---|
| L0 | In-memory reference, `fmine-ledger::reference` | Semantic oracle |
| L1 | Append-only segment files (`FMT-SEG-1`: magic, CRC32C per record, hash chain, MMR peak footer) | **Production** |
| L2 | fsqlite persistence under unchanged ledger semantics | Only if L1 fails GATE-STORE-01 (KILL-4) |

**Durable formats.** All are hand-written, little-endian and versioned, and live in S0 so that **formats never
depend on a runtime version**. Readers accept versions N and N−1. Migration is a journaled rewrite into new CAS
objects (never in place), with golden fixtures per version, a fuzz target per decoder and a
`fmine verify` reconstruction command.

| FMT | Content | Notes |
|---|---|---|
| FMT-LEDGER-1 | Delta batch `{cell, epoch, seq, prev_hash, anchor, deltas[], witnesses[]}` | Canonical order; SHA-256 chain, which aligns with ATP Merkle hashing |
| FMT-SEG-1 | Ledger segment file | Magic `FMNE` + kind; CRC32C per record; MMR peak list in the footer |
| FMT-CAS-1 | Chunk and manifest | 4 MiB default chunk; fanout-16 Merkle; root-last |
| FMT-TELSEG-1 | Columnar telemetry segment | Codec id per column; bounds; min/max footer |
| FMT-BM-1 | Block-model projection | VDB tile layout + attribute columns + ModelCard digest |
| FMT-ANCHOR-1 | Anchor / high-water mark | FSS `LedgerAnchor` fields (`ledger_epoch, commit_sequence, adapter_registry_epoch, schema_epoch, policy_epoch, privacy_epoch, state_root`) with `site_lineage → cell_id`, plus `residency_epoch` |
| FMT-SYNC-1 | Sync frames | Version vector, consistency proof, chunk want/have |
| FMT-POST-1 | Posting slot + Merkle-folded detail | Maps to FA `fa:external-subledger-detail:v1` rows (§21.3) |
| FMT-CERT-1 | FM result certificate | Inputs closure, code revision, seed, determinism class, output digest |
| FMT-ENV-1 | Robot NDJSON envelope | The only *format* where serde is permitted (§30.1); serde is otherwise confined to the `fmine-decide` adapter closure (L-4) |
| FMT-EXPORT-1 | Export projection rows (Avathon, partners) | Anchor + certificate on every row (§32.3) |

### 26.6 The runtime: asupersync region tree and capabilities

The runtime is asupersync at the suite pin, **0.5.0** (§27.5). The capability rows below are compile-time
narrowings, written `CapSet<SPAWN, TIME, RANDOM, IO, REMOTE>`.

```text
ProcessRegion(fmined, role ∈ {site, regional, enterprise, relay})
└── NodeRegion{node_id, role}                                        supervisor: rest_for_one
    ├── AuthorityRegion                                               rest_for_one (projections restart after the writer)
    │   ├── LedgerWriterRegion{cell}*      CapSet<0,1,0,1,0>  lease + fencing; TIME only as journaled TimeEvidence
    │   └── ObjectStoreRegion              CapSet<0,0,0,1,0>  CAS stage → verify → root-last publish
    ├── IngestRegion (site)                                           one_for_one
    │   ├── TelemetryRegion{source}*       CapSet<0,1,0,1,0>  bounded edges; segment builder
    │   └── ImportQuarantineRegion{fmt}*   CapSet<0,0,0,1,0>  read-only staging; parsers; no TIME or RANDOM
    ├── ComputeRegion                                                 one_for_one
    │   ├── LawRegion                      CapSet<0,0,0,0,0>  stripped Cx: conservation, QA/QC, posting laws
    │   └── EstimationRunRegion*           CapSet<1,0,0,0,0>  tiles may spawn; seeds explicit; no IO
    ├── ProjectionRegion                   CapSet<1,0,0,1,0>  fsqlite / fgdb / frankensearch projectors
    ├── SyncRegion                                                    one_for_all per peer
    │   ├── PeerSessionRegion{peer}*       CapSet<1,1,0,1,1>  the ONLY REMOTE-capable region besides FinanceBridge
    │   └── TransferRegion{root}*          CapSet<0,1,0,1,1>  CAS chunks over QUIC/ATP
    ├── FinanceBridgeRegion (enterprise)   CapSet<0,1,0,1,0>  spawns the `fa` subprocess; local socket only
    ├── EffectRegion                       CapSet<0,1,0,0,0>  prepares EFFECT-* documents; never actuates
    └── OperationsRegion                   CapSet<1,1,0,1,0>  doctor, repair plan/apply, CLI socket, MCP, TUI feed
```

- Engines in layers K and E receive `Cx<cap::None>` (plus SPAWN for estimation tiles). They get times and seeds
  as **inputs**, which is what makes replay deterministic (INV-010).
- **Defect S3(a) at 0.5.0:** a restricted `Cx<cap::None>` can call `open_child_region` and receive
  `Cx<cap::All>`. So `xtask check-caps` bans `open_child_region` outside `fmine-runtime`, and a lab test asserts
  each region's runtime capability mask (PO-BED-8, §31.4).

### 26.7 Budgets, durable obligations, cancellation, supervision and the lab

**Budgets.** `Budget{deadline, poll_quota, cost_quota, priority}` composes by `meet`.

| Region | Deadline | cost_quota | Priority | On exhaust |
|---|---|---|---|---|
| LedgerWriter commit | 250 ms | — | 200 | Refuse the batch (exit 6); the caller retries with the same batch id |
| Telemetry segment seal | 1 s | 64 MiB per segment | 180 | Seal early; downsample with a continuity receipt; never drop silently |
| EstimationRun | per run (e.g. 4 h) | blocks × realizations | 80 | Cancel; partial results are never published (root-last) |
| PeerSession window | link window | bytes per window | 120 | Drain; persist the chunk receipt; resume next window |
| FinanceBridge post | 120 s (debug FA); 10 s TARGET | — | 150 | Indeterminate → reconcile by idempotency key; never re-post blindly |

**Durable obligations.** asupersync obligations are lifted so that they survive crashes.

| ID | Reserved when | Must resolve by |
|---|---|---|
| OB-SYNC-ACK | A batch is shipped | An enterprise ack naming `(cell, seq, mmr_root)`. Until then the site retains it and re-offers. |
| OB-SEAL | Chunks are staged | The root is published, or the staged set is aborted |
| OB-POST | A posting slot is reserved | FA `EntryRef` + certificate digest recorded, or a refusal recorded (§21.3) |
| OB-REVIEW | A Workpaper is prepared | A human `Reviewed{against}` attestation or an explicit rejection |
| OB-EFFECT | An effect is prepared | A human authority-token decision, then actuals reconciled (§31.3) |
| OB-LAB | A sample is dispatched | The assay is received, or the sample is declared lost with a QA/QC finding (§12) |
| OB-TARP | A trigger is raised | Acknowledged and escalated per the TARP (§14.4) |
| OB-COMMIT | An ESIA, permit or agreement commitment falls due at its frequency | Attested evidence of performance, or a recorded miss (§18.2) |

**Cancellation** (request → drain → finalize), shown for a sync session:
1. *Request:* stop offering new batches and chunks.
2. *Drain:* finish the in-flight chunk, persist a receipt of the chunk hashes received, flush the
   `ProgressCertificate`.
3. *Finalize:* publish no root unless complete, release the transfer lease, record a quiescence proof.

A hard kill leaves OB-SEAL live, so the next session resumes from the receipt. This gives resumability **without**
relying on 0.6.0 ATP journaling (defect S3(e)).

**Supervision (Spork).** There is one `AppSpec` per role, compiled to a `CompiledSupervisor`.
- Restart intensity is 5 restarts per 60 s per child; beyond that the failure escalates.
- An escalated PeerSession marks the peer `stale` and keeps local authority running.
- An escalated LedgerWriter puts the node into **read-only degraded mode** (writes refuse with exit 4). A writer
  is never silently restarted onto a possibly torn segment. `fmine doctor` must first verify the hash chain.

**LabRuntime.** Every protocol (sync, posting, effect, ingest backpressure) has `#[lab_test]` suites with:
- virtual time;
- seeded schedules: seeds 0..999 in CI and 10⁴ nightly, with DPOR;
- chaos: link drop p ∈ {0.01, 0.2}, and a crash at every reserve/commit/publish boundary;
- asupersync oracles (task leak, obligation leak, quiescence, region tree, cancellation protocol);
- FM oracles PO-BED-1, 2, 3, 4 and 10.

The same seed must give the same trace digest. A failing seed retains a `CrashPack` for replay.

### 26.8 The telemetry segment log

- Telemetry is the one high-rate context, and the arithmetic in GB-35 forces its design: batch into columnar
  segments, seal on a timer (250 ms), and keep the raw stream at the site.
- **Transitions are facts; samples are evidence.** A truck's load and dump events, a delay code and a TARP state
  change are ledger records. The 10 Hz payload-weighing trace behind a load event is segment data, referenced by
  `(segment root, offset range)`.
- **Backpressure.**
  - Every ingest edge is bounded.
  - Under overload, segments seal early and downsampling engages with a **continuity receipt** stating exactly
    what was dropped and why. Data is never dropped silently.
  - A raw ring buffer retains the original stream while disk allows.
- **Rollups** default to 1 minute and replicate. 5,000 signals at 16 B per minute is ≈ 0.12 GB/day, about 1% of a
  10 Mbit/s link.

### 26.9 Alien-artifact compilation (B4 §9; families 12, 35 + hashing, 31)

| Family | Impact | Conf. | Effort | Score | Admitted |
|---|---:|---:|---:|---:|---|
| 31 Queueing capacity budgets | 4 | 4 | 1 | 16 | yes (A3) |
| Merkle/MMR certificates | 5 | 5 | 2 | 12.5 | yes (A2) |
| 12 Concurrency and obligation automata | 5 | 4 | 2 | 10 | yes (A1) |
| 05 Descent/gluing | 2 | 4 | 1 | 8 | checker only (§26.3) |
| RaptorQ erasure tuning | 2 | 3 | 3 | 2 | no; consume ATP as is |

**A1 · Protocol automata.**
- `registries/automata/{sync,posting,effect,ingest,lab,tarp}.toml` hold states, labelled transitions, guards, and
  the obligation each transition reserves or resolves.
- CI checks:
  - every state is reachable;
  - no non-terminal state is dead;
  - every non-terminal state has a cancel edge;
  - the Petri token-conservation invariant holds: reserved = committed + aborted + live, per obligation kind;
  - the transition log replays deterministically.
- At runtime, every transition goes through one `step(state, event) → Result<state, Refusal>` table. There are
  no ad-hoc state flips anywhere in the code.

**A2 · Merkle/MMR certificates.**
- Each cell is a hash-chained log with a Merkle Mountain Range over batch digests.
- `SyncCert{cell, epoch, from_seq, to_seq, mmr_root_before, mmr_root_after, consistency_proof}` proves that a
  replica is a prefix of the writer's log in ⌈log₂ n⌉ hashes (Certificate-Transparency style).
- CAS roots are fanout-16 Merkle trees. FA rows carry slice MMR roots (§21.3).
- **Checked null:** Merkle *set reconciliation* is **not** needed on the hot path (GB-36). Merkle structures earn
  their keep as **proofs**, not as diff engines.

**A3 · Queueing budgets.**
- Per-site ingest is modelled as M/D/1 with a Kingman correction for bursty arrivals. The FA writer and the human
  reviewer are modelled as servers too (GB-28).
- Monitors on the `fmine doctor` page:
  - a Little's-law audit (L = λW within 5%);
  - an arrival-drift alarm when the 5-minute λ exceeds 1.3× the planned peak.

### 26.10 Proof obligations

| PO | Statement | Verification | Failure → fallback |
|---|---|---|---|
| PO-BED-1 Single writer | Per cell, at most one lease holder per epoch; receivers refuse lower epochs | LabRuntime DPOR over lease transfer + crash; fencing unit tests | Lower-epoch batches quarantined; conflict record; human resolution |
| PO-BED-2 Prefix consistency | An enterprise replica of a cell is always a prefix of the writer's log | `SyncCert` verified on every apply; `fmine-verify` offline | Stop applying that cell; mark `stale`; doctor |
| PO-BED-3 Root-last | No CAS root is visible unless all its chunks are verified | Crash at every stage boundary in the lab | The partial set stays staged; OB-SEAL resumes |
| PO-BED-4 Exactly-once posting | ≤ 1 live original per FA slot; Petri token = 1 | FA-ACC-02/03/12; automaton check | Exit-6 conflict → alarm; freeze the bridge |
| PO-BED-5 Obligation closure | Closing a region implies every OB-* is resolved or persisted durably | Leak oracle; durable-obligation replay | The node will not report quiescent; doctor lists the obligations |
| PO-BED-6 Conservation at commit | Per flow node and period, Σin − Σout − Δstock = 0 within the registered measurement tolerance, else refusal (no plug) | Property and metamorphic tests (split and merge stockpiles) | Refusal exit 3; reconciliation case opened (§20) |
| PO-BED-7 Queue stability | ρ_ingest ≤ 0.7 at the planned P95 rate; P(queue > K) ≤ 10⁻⁶ per hour | GATE-INGEST-01; Little audit | Degrade to coarser rollups with continuity receipts |
| PO-BED-8 Non-escalation and noninterference | K/E crates never obtain IO or REMOTE; wall-scoped queries reveal nothing across walls, including counts and absence | Lint + runtime-mask lab test + wall differential tests | Build fails; projection quarantined |
| PO-BED-9 Determinism | Same inputs + seed + code revision + determinism class ⇒ same certificate digest (INV-010) | Determinism lanes in CI (§29) | Digest drift blocks release |
| PO-BED-10 Bridge idempotency | Replaying the outbox after any crash leaves FA state unchanged (`already_recorded`) | FA-ACC-12; 1,000 seeds | Indeterminate → reconcile by key, never re-key |

### 26.11 Failure modes and deterministic fallbacks

| # | Failure | Detection | Fallback |
|---|---|---|---|
| F1 | Satellite link down for days | Session timeouts; version-vector age | The site keeps full authority. The enterprise marks those cells `stale(age)`. Decisions needing fresh site data return exit 9 or refuse. OB-SYNC-ACK queues. |
| F2 | `fad` down, or its lease held elsewhere | FA exit 6 `FA-REF-018`; socket error | The bridge appends canonical intents to the outbox and replays later under the same keys (PO-BED-10) |
| F3 | FA law refusal (exit 3/7) | Robot refusal object | Slot → `Refused`; human triage; never auto-edit and re-key |
| F4 | Ingest overload | Queue > K; ρ monitor | Seal early; downsample with a continuity receipt; raw ring buffer if disk allows |
| F5 | Projection corrupt or behind | Doctor digest check; high-water mark | Discard and rebuild from the ledger. Answers state their high-water mark; freshness-required queries refuse. |
| F6 | Split brain | Epoch regression at a receiver | Quarantine lower-epoch batches; record a conflict; humans adopt or refuse |
| F7 | Torn segment after a crash | Hash-chain or CRC failure at open | Read-only degraded mode; `repair plan` truncates to the last verified record (evidence retained); history is never rewritten |
| F8 | Lifted-code drift or an upstream bug | GATE-FSIM-DRIFT; golden hash | Freeze at the lifted version; re-lift by bead; no dependency switch |
| F9 | Residency or MNPI breach attempt | Planner/caveat check | Refuse before bytes move (exit 7); AccessEvent recorded |
| F10 | Missing or expired human authority token | Token verification | Refuse (exit 7); no alternative path exists |
| F11 | asupersync pin mismatch in a closure | One-package-id CI gate | Merge blocked; coordinated bump only (GATE-PIN-06) |
| F12 | Dangling cross-cell reference | Gluing checker on anchor publish | Consumer state `not_observable` or `stale`; the dependent Claim is not published |

### 26.12 Gates and kills

| Gate | Threshold (TARGET) | When |
|---|---|---|
| GATE-STORE-01 | Ledger → fsqlite projection rebuild of 10⁵ holes / 10⁷ intervals in ≤ 15 min on the reference site node; as-known-at and bbox answers from one anchor | M1 |
| GATE-SYNC-01 | 1,000 seeded lab runs (link drop; crash at every boundary): 0 divergence, 0 duplicate applies, 100% `SyncCert` verified, 0 obligation leaks | M1 |
| GATE-SYNC-02 | 7-day partition catch-up in ≤ 2 h at 10 Mbit/s and 50% availability | M5 (first needed when ≥ 2 sites sync; the M2 pilot ingests snapshots) |
| GATE-INGEST-01 | 10⁵ msg/s sustained for 1 h: ρ ≤ 0.7, 0 unreceipted loss, p99 ingest-to-durable ≤ 2 s | M4 (synthetic), M5 (site) |
| GATE-BM-01 | Lifted `VdbGrid`: 10⁸ active cells × 8 B payload, resident memory ≤ 2.5× payload, deterministic `iter_active` digest | M1 |

| Kill | Trigger | Fallback |
|---|---|---|
| ✖ KILL-4 Owned ledger persistence | GATE-STORE-01 fails, or witness checks prove infeasible in memory | Rung L2: fsqlite persistence under unchanged ledger semantics |
| ✖ KILL-5 Telemetry rate | GATE-INGEST-01 fails at 10⁵ msg/s | Edge pre-aggregation; publish the supported rate honestly |

KILL-1 (FA scale) is in §21.3, KILL-2 (fgdb projection) and KILL-3 (frankensim dependency) are in §27, and KILL-6
(OMF purity) is in §32.

### 26.13 Galaxy-brain cards

**GB-35 · Telemetry: fgdb as authority fails by arithmetic; the segment log passes with room to spare.**
- **Equation.** ρ = λ·S; the Kingman wait is W_q ≈ (ρ/(1−ρ))·((C_a² + C_s²)/2)·S.
- **Substituted (segment log).**
  - 10⁵ msg/s × 200 B = 20 MB/s. Sealing every 250 ms gives λ = 4 seals/s.
  - S = encode 5 MB at 400 MB/s (12.5 ms) + compress (≈ 12 ms) + fsync (2 ms TARGET on rugged NVMe) ≈ 27 ms.
  - ρ = 4 × 0.027 = **0.11**. W_q ≈ 0.124 × 0.25 × 27 ms ≈ **0.8 ms**.
- **Substituted (fgdb as authority).** One publication per message, two fsyncs, no group commit: μ ≤ 250/s, so
  **ρ = 4 at 10³ msg/s** and 400 at 10⁵. It is unstable at the *bottom* of the stated range.
- **Upstream.** Raw 160 Mbit/s against a 10–50 Mbit/s uplink is 3–16× over capacity, while 1-minute rollups use
  about 1% of a 10 Mbit/s link.
- **Intuition.** Telemetry must be site-sovereign, batched into segments and summarized upstream. No database
  choice changes that.
- **Assumptions.** 200 B messages; 2 ms fsync; timer-driven seals (C_a ≈ 0).
- **Flip.** A measured S > 175 ms at 10⁵ msg/s gives ρ > 0.7 → KILL-5.

**GB-36 · Sync over satellite, and why Merkle diffs are a checked null.**
- **Catch-up after a 7-day partition.** (0.05 GB deltas + 0.12 GB rollups) × 7 ≈ 1.2 GB. At 10 Mbit/s and 50%
  availability that is 1.2·10⁹ × 8 / 5·10⁶ ≈ **32 min**, inside GATE-SYNC-02's 2 h.
- **Anti-entropy per round.** 12 cells × (24 B version-vector entry + 24 hashes × 32 B) ≈ **9.5 KB**.
- **Block model.**
  - 10⁸ blocks × 20 attributes × 4 B = 8 GiB = 2,048 chunks, so the manifest is 64 KiB.
  - A Merkle diff with 30% changed (d = 614) costs d × depth × fanout × 32 B = 614 × 3 × 16 × 32 ≈
    **0.94 MB, worse than sending the whole manifest**. A Merkle diff wins only if d ≪ N/48.
  - Moving the 2.4 GB of changed chunks takes 32–64 min.
- **Intuition.** Use version vectors for logs and plain manifests for objects. Merkle/MMR structures are for
  **proofs**.
- **Flip.** If telemetry segments (N ≈ 10⁶) ever had to replicate, Merkle diffs would come back into play. So
  would residency rules that forbid even rollups.

### 26.14 Open questions

- **OPEN-B4-01 (narrowed 2026-10-29).** Statutes are now cited row by row for 12 countries × 6 data classes
  (W3, E501–E549; §31.2). Hard localisation in force: Zambia personal data (DPA 2021 s.70), Tanzania geoscience
  export (Mining Act s.35(6)) and personal data (PDP Regulations 2023 reg. 20 permit), DRC personal data (Code du
  numérique art. 201) and Mali personal data (Loi 2013-015 art. 11). Pakistan has no data-protection statute in
  force; Reko Diq is governed by its mineral agreement and BMR 2002 (BMMA 2025 s.122(4)): records are kept in
  Balochistan (r.56) and the State holds data rights (r.71). Counsel is still needed on the eight clauses of W3 §7.
  Unknown pairs still default to `in_country_only`.
- **OPEN-B4-05.** If fgdb ships valid time, constraints and a catalog, does the decision ledger or reserves register
  ever move to fgdb authority? The default is no (one mechanism).
- **OPEN-B4-10.** Reference site node hardware (fsync distribution, RAM). Every TARGET in this section depends on
  it.

## 27. Suite consumption: what FrankenMining takes from its siblings, and how

**Doctrine.** *Consume, never fork* (§0.4, sentence 9). A sibling is used in exactly one of five postures:

| Posture | Meaning | Example |
|---|---|---|
| **Consume** | Cargo dependency at an exact pin, default features off unless listed | `fsci-*` `=0.4.0` |
| **Subprocess** | A robot CLI or daemon over a local socket; no crate in the graph | FA through `fad` + `fa --json` |
| **Lift** | Copy with a provenance header, registry row and golden test; re-owned under `fmine-*` | frankensim `VdbGrid` |
| **Oracle** | Dev or lab only; never in the runtime or CI dependency graph of production crates | fsym, PROJ, fnp-linalg |
| **Pattern** | Copy the design, not the code | FSS anchors, FTX lift headers |

Each consumed foundation passes an `INT-*` admission gate before its first production use (§34.2). Every
foundation has an `FD-` row in the dependency ledger (Appendix F) with its pin, posture, owner and switch-over rule.
No executive deliverable may be blocked by an unconverged sibling: each `FD-` row has a fallback that keeps the
product shippable and honest.

### 27.1 frankensim: lift-only until re-convergence

**Verdict (B4 SQ3).** Lift-only in every production posture. A path or git dependency is refuted today:
- it would link asupersync 0.6.0 beside the 0.5.0 of fgdb, FA and fastmcp, giving two asupersync packages, two
  `Budget` types and two runtimes (B4 S2);
- frankensim needs `nightly-2026-07-06` for `generic_const_exprs`, against the suite's `nightly-2026-08-31`;
- `cargo check --workspace --all-targets` exits 101 on 15 targets (E001);
- a git dependency hits out-of-repo `../../../asupersync` paths.

"Reimplement everything" is also refuted, on cost: exact predicates, sparse LDLᵀ/AMG, deterministic transcendentals,
Philox and e-processes are already written, tested and contracted upstream.

**Lift waves.** The full per-crate disposition is in §13.9.

| Wave | Source (frankensim @ pinned commit) | Target | Guard |
|---|---|---|---|
| 0 | `fs-math/src/det.rs` (deterministic transcendentals) | `fmine-core::det` | Golden hash of 10⁴ fixed inputs |
| 0 | `fs-rand` Philox + Sobol/Owen | `fmine-core::rand` | Known-answer vectors |
| 0 | `fs-evidence` (closure of 3 modules), types only, with a stable-Rust `QuantitySpec` shim | `fmine-evidence` | Property test: `compose` never promotes |
| 0 | `fs-ivl` predicates/expansions, `fs-math/eft.rs` | `fmine-geom::exact` | Adversarial predicate corpus |
| 0 | `fs-sparse` (CSR/COO, supernodal LDLᵀ + AMD, PCG, smoothed-aggregation AMG) | `fmine-numerics::sparse` | Residual known-answers |
| 0 | `fs-eproc`, `fs-voi`, `fs-robust` (CVaR), `fs-tropical` | `fmine-numerics::{eproc, voi, risk, tropical}` | Per-module known answers |
| 0 | `fs-assimilate/src/nonlinear/ensemble.rs` (EnSRF core, Cx-free) | `fmine-numerics::enkf` | Known-answer filter runs |
| 0 | `fs-blake3` | `fmine-core::hash` | Official test vectors |
| 1 | `fs-rep-sdf/src/vdb.rs` (`VdbGrid<T: Copy>`, 310 lines, std-only) | `fmine-geom::vdb` | Golden `iter_active` order digest; GATE-BM-01 |
| 1 | `fs-rep-mesh/src/winding.rs` (`atan2` → `det::atan2`) | `fmine-geom::winding` | Differential corpus vs upstream vectors |
| — | `fs-qty` | **Not lifted** (nightly); `fmine-units` is built on stable Rust (§25) | — |

**Header contract (from FTX).** Every lifted file starts with:

```text
//! Provenance: frankensim <path> @ <commit>; licence <id>; Changes: <list>; Left out: <list>
```

- `registries/lifts.toml` holds one row per file: `{source, commit, sha256_at_lift, target, guard_test}`.
- `xtask check-lifts` fails CI on any mismatch between header and registry.

**Re-convergence gates.**
- **GATE-FSIM-DRIFT** (quarterly):
  - Diff each lifted source at frankensim HEAD against its pinned commit.
  - Any upstream change inside a lifted region opens a bead within 10 working days.
  - The decision (re-lift, or no-op with a reason) is recorded.
- **GATE-FSIM-RECONV** is the precondition for any Cargo dependency on frankensim. All of the following must hold:
  1. frankensim pins asupersync by exact revision, equal to the suite pin;
  2. `cargo check` is green on the consumed crates at that commit;
  3. the consumed crates' toolchain matches FrankenMining's;
  4. the INT-FSIM-001 spike (a separate clone) produces a posture closure with **one** asupersync package id.
     Today it is expected to fail.

  Only then may `fmine-simbridge` be created, and only for capabilities with no lifted equivalent:
  - the `fs-airflow` network solve (§16.6);
  - the `fs-conduction` seepage operator (§14.5, §18);
  - `fs-solid`, once Mohr–Coulomb exists upstream.
- ✖ **KILL-3 (frankensim dependency).** INT-FSIM-001 fails, or RECONV never holds. Fallback: lift-only,
  permanently, and say so. Engineering consumers that need a bridge (seepage, ventilation network, FEM) stay on
  in-house limit-equilibrium and correlation-card methods, with that limitation recorded in their ModelCards.

### 27.2 franken_accounting: subprocess, never a crate

- The full contract is in §21.3. In summary:
  - posting goes through `fad` plus the `fa --json` robot CLI, from one `fmine-fa-bridge` at the enterprise
    finance node;
  - a signed intent outbox is the deterministic fallback;
  - **no FA crate appears anywhere in the FrankenMining graph** (L-4, §28.5);
  - embedded `fa::Book` is used only in an isolated conformance workspace for FA-ACC tests.
- Admission: **INT-FA-001** = FA-ACC-01…13 green at the pinned FA revision.
- FA's HEAD moved three commits during the survey (`53c6e953` → `3323cf0c`, B4 S6). The pin is re-read at every
  `GATE-PIN-*` review, never assumed.

### 27.3 Numerics: the frankenscience stack (R8)

**What was found.**
- There is **no shared array type** across fsym, fnp, fp, fsci and fj (R8 §7.1). FrankenMining builds one
  (`fmine-array`) plus adapters.
- frankensympy is a design-time oracle, not a runtime dependency.
- franken_numpy's `memmap` is not a memory map, and its `UFuncArray` stores values as f64 with an exact
  i64/u64 sidecar for integers.
- frankenscipy is the backbone, with real gaps:
  - no sparse Cholesky;
  - CG is ILU-preconditioned only (`pcg`), with no AMG or generic preconditioner hook, and MINRES is
    unpreconditioned;
  - its `linprog` "Highs" variants are a dense revised simplex or IPM, not a HiGHS port;
  - no censored MLE;
  - `RbfInterpolator` is capped at 4,096 points.
- frankenjax has `grad`, `vmap` and `hvp`, but its "jit" is interpretation, so NUTS must be built. FrankenMining
  consumes `fj-ad` directly (`vjp`, `compile_value_and_grad_jaxpr_for_repeated_eval`) because `fj-api` pulls
  `fj-ffi`; Hessian-vector products are composed from `fj-ad`.

**Consumption table (exact pins; numeric crates always with their asupersync features off).**

| Repo | Crates | Pin | Posture | Use |
|---|---|---|---|---|
| frankenscipy | `fsci-linalg`, `fsci-sparse`, `fsci-opt`, `fsci-stats`, `fsci-spatial`, `fsci-interpolate`, `fsci-integrate`, `fsci-signal`, `fsci-fft`, `fsci-special`, `fsci-ndimage` | crates.io `=0.4.0`, default features off | Consume | Distributions, KDTree, NLLS, ODEs (leach/flotation/CIL kinetics), signal processing (vibration), root finding (IRR), Sobol/LHS sampling, small QPs |
| frankenpandas | `fp-frame`, `fp-columnar`, `fp-types`, `fp-index`, `fp-join`; **not `fp-io`** (arrow/parquet, OPEN-SUITE-4 closed) | git `37bb94101`, `default-features = false` (no `sql-sqlite`, no hdf5). crates.io `fp-frame 0.6.0` exists but predates the pin by 68 commits and about 7,300 changed lines in the consumed crates, so it is not an alternative (R7-C165) | Consume | Telemetry rolling/resample/`merge_asof`, grade bins (`cut`/`qcut`), report tables. **Not** money (no decimal dtype) and **not** interval algebra (linear-scan `IntervalIndex`). |
| franken_numpy | `fnp-random`, `fnp-io` | git `d32c4e04`; never `fnp-python` | Oracle (`fnp-random`); consume (`fnp-io` for `.npy`/`.npz` interchange) | RNG known-answer cross-checks; array interchange with Python users |
| frankenjax | `fj-core`, `fj-lax`, `fj-ad` | git `fa93fb77`. Not `fj-api`, `fj-dispatch` or `fj-interpreters`, which depend on `fj-ffi` unconditionally; no `fj-backend-gpu` | Consume | Gradients and Hessian-vector products for NUTS (`fmine-infer`), and sensitivities |
| frankensympy | `fsym-core`, `-calculus`, `-matrices`, `-solvers`, `-simplify`, `-printing` | `9d785d0`, vendored or per-crate git | **Oracle / build-time only**, inside `fmine-derive` | Golden formulas (two-product balances, Lane's cut-offs, fiscal terms), symbolic Jacobians for tests, LaTeX for reports. Never on a runtime path until an fsym compatibility profile is certified. |

**One RNG (ADR-RNG-01).**
- Production randomness comes from **one** generator: lifted frankensim Philox in `fmine-core::rand`, keyed
  `(seed_root, partition, domain, var_group, r, brick)`.
- R8 proposed consuming `fnp-random` instead. Two production RNG families would double the golden-stream surface
  and the determinism argument, so `fnp-random` is an **oracle**. Its Philox stream is a known-answer
  cross-check if the variant and counter layout match (OPEN-SUITE-2), and golden vectors otherwise.

**Admission.** The consumed numerics crates (fsci, fp without `fp-io`, `fnp-io`, fj) pass INT-NUM-001 (§34.2;
BD-308) before their first production use. fsym and `fnp-random` are oracles: they need only a pinned dev-workspace
record.

**The "build in `fmine-*`" list.** R8's names are mapped onto this plan's crate map (§28).

| Pri | R8 name | FrankenMining home | Scope | Builds on |
|---|---|---|---|---|
| P0 | `fm-array` + bridge | `fmine-array` (S0) + `fmine-numerics::bridge` | Typed contiguous grid (`Arc<[T]>` + dims + strides, T ∈ {f64, f32, i64, u8, bitset}); zero-copy views into `&[f64]` consumers; copying adapters to `&[Vec<f64>]` | fnp stride calculus (reference) |
| P0 | `fm-rng` | `fmine-core::rand` | Keyed streams; golden streams | Lifted Philox (ADR-RNG-01) |
| P0 | `fm-units` | `fmine-units` | §25 | none |
| P0 | `fm-drillhole` | `fmine-drill` | Desurvey; sweep-line from/to overlap join per hole; length-weighted compositing | fp I/O; `fmine-array` |
| P0 | `fm-geostat` | `fmine-geostat` | Variography; SK/OK/block kriging; numeric gamma-bar (Gauss–Legendre); anisotropic search | fsci `KDTree`, `least_squares`, `fsci-special`; lifted batched Cholesky |
| P0 | `fm-pseudoflow` | `fmine-optimize::pitflow` | HPF on i64 CSR; precedence templates; nested pits (§15) | Oracles: fsci `maximum_flow`, fnx `preflow_push` |
| P1 | `fm-recon` | `fmine-recon` | WLS and bilinear reconciliation; gross-error tests (§20) | fsci-linalg, fsci-opt `trust_constr`, fsci-stats χ²; fsym-derived Jacobians in tests |
| P1 | `fm-fiscal` + `fm-kernels` | `fmine-econ::fiscal` + `fmine-derive` (build-time codegen) | NSR, NPI, sliding-scale royalty and tax as an AST; canonical hash and LaTeX through fsym | fsym `Expr`, `TermDag`, `latex`, `to_rust_code` |
| P1 | `fm-lp` | `fmine-lp` | PDLP first-order LP on CSR; QP; MPS I/O; decomposition (§15) | fsci-sparse CSR; oracle fsci `linprog`/`milp` |
| P1 | `fm-mcmc` | `fmine-infer::nuts` | NUTS with adaptation, bijectors, R-hat/ESS diagnostics (≈ 3–5k lines) | fj `value_and_grad`; `fmine-core::rand` |
| P1 | `fm-gmrf` | `fmine-numerics::gmrf` | SPDE precision assembly, sparse LDLᵀ, log-determinant, selected inverse, AMG-PCG, sampling | Lifted fs-sparse; fsci `cg` as oracle |
| P1 | `fm-sgs` | `fmine-geostat::sim` | SGS and turning bands; post-processing | `fmine-geostat`, `fmine-core::rand` |
| P2 | `fm-geochem` | `fmine-qaqc::censored` | Left-censored lognormal MLE, ROS, CLR/ILR (§12.2) | fsci-stats, fsci-opt |
| P2 | `fm-implicit` | `fmine-belief::implicit` | Partition-of-unity or local RBF at 10⁵–10⁶ points | fsci `RbfInterpolator` as small-case oracle |
| P2 | `fm-blockstore` | `fmine-geom::bricks` | Chunked out-of-core block storage with `.npy`-compatible tiles | fnp-io header codec |
| P2 | `fm-cutoff` | `fmine-plan::cutoff` | Lane's limiting and balancing cut-offs; DP over the life of mine (§15.6) | fsci `brentq`; fsym-derived goldens |
| P2 | `fm-sensitivity` / `fm-uq` | `fmine-decide::sens` | Tornado charts, elasticities, delta method, Sobol indices | fj `jvp`/`hessian`; fsci `SobolSampler` |
| P2 | `fm-libm` | `fmine-core::det` (lifted, Wave 0) | Fixed-algorithm transcendentals for cross-ISA reproducibility (§29) | Lifted `fs-math/det.rs` |

**Oracles, never production kernels:** fsci `maximum_flow` and fnx `preflow_push` for `pitflow`; fsci
`linprog`/`milp` for `fmine-lp`; fnp-linalg for `fmine-geostat` solves; fsym for closed-form formulas; fs-uq
`KlExpansion` and fs-bo `Gp` for kriging (§13.9).

**Hot-path rule.** Hot loops never round-trip through `Vec<Vec<f64>>`. Where fsci only offers that signature,
flat-slice overloads are an upstream ask (Appendix F), and a copying adapter is used until then.

### 27.4 Storage and search foundations: projections only

| Foundation | Pin (FACT, R2) | Role in FrankenMining | Admission gate | Kill |
|---|---|---|---|---|
| frankensqlite (fsqlite) | `v0.4.12` (mature, released) | **Primary projection store**: relational tables, R-tree over 3-D interval boxes and geofences, FTS5, segment catalog, Z-order keys | INT-FSQL-001: the FSS-DD ten criteria in the projection role; bbox query p99 ≤ 50 ms over 10⁷ intervals | KILL-4 promotes it to persistence rung L2 (§26.5), never to authority |
| frankengraphdb (fgdb) | rev pinned with the suite (asupersync 0.5.0) | **Optional** evidence-lineage projection: GQL, `execute_certified`, `FOR SYSTEM_TIME AS OF SEQ`, `CALL fnx.*` | INT-FGDB-001: one high-water mark; certified replay; **measured benefit** over fsqlite + in-memory fnx on 3 named queries | ✖ **KILL-2:** no measured benefit → drop the fgdb projection; fsqlite tables + in-memory fnx serve the evidence graph |
| frankensearch | facade `0.7.1` | Document search over reports, logs and correspondence (quill BM25 + hash/model2vec embeddings) | INT-FSEARCH-001: no `ort`/`fastembed` in the closure; **wall-scoped indexes only** (§31.1) | Drop to fsqlite FTS5 |
| franken_networkx (fnx) | git `9d710b1c` (fgdb-prism's pin; fnx-classes 0.2.0) while fgdb is in the graph. `preflow_push` exists only at fnx HEAD (0.3.1), so that oracle runs in a separate dev-only oracle workspace until fgdb bumps fnx (upstream ask, Appendix F) | Precedence graphs, critical paths, flow-graph analytics; oracle for pitflow | INT-FNX-001: scale benchmark on synthetic precedence graphs (10⁶–10⁷ arcs) | In-house graph code in `fmine-optimize` |

**Why fgdb is a projection, even though FA uses it as authority.** fgdb cannot index `Point` properties by
constitution (B4 S8). Its replication (Aegis) is a Raft kernel, which needs quorums and suits intermittently linked
writers poorly (HYPOTHESIS; unbenchmarked). FrankenMining's facts are spatial and federated, while FA's are neither.

### 27.5 Runtime, surfaces and shared schemas

| Foundation | Pin | Confined to | Gate |
|---|---|---|---|
| asupersync | **`0.5.0`**: asupersync rev `78b64636e` (tag v0.5.0, the revision fgdb and FA pin; fgdb itself at FA's pin `b323bd17`), plus a `[patch.crates-io]` that keeps one `Cx` type | `fmine-runtime` and above; K crates see only `Cx<cap::None>`, `Budget` and `checkpoint` | INT-AS-001, INT-AS-SEC-001 (§31.4), GATE-PIN-06 |
| `franken-evidence`, `franken-decision`, `franken-kernel` (asupersync workspace) | Same asupersync revision | **`fmine-decide` only** (L-4) | Part of INT-AS-001. The APIs exist at v0.5.0 (`EvidenceLedger`, `LossMatrix`, `DecisionContract`, `DecisionAuditEntry`, `render`; OPEN-SUITE-1 closed). They depend on serde, so they never enter S0: `fmine-evidence` (S0) owns schema-compatible types, and `fmine-decide` is the single round-trip-tested converter and renderer. |
| fastmcp (crates.io package `fastmcp-rust`; the crates.io name `fastmcp` is an unrelated project, R7-C163) | `=0.13.0` (every release from 0.11.0 pins asupersync `=0.5.0`; release 0.5.0 would bring a second asupersync, 0.4.5; toolchain `nightly-2026-08-25`, drift recorded) | `fmine-mcp` only | INT-FMCP-001 (parity gate) |
| frankentui (ftui) | `=0.9.0` (0.8.0 and later pin asupersync `=0.5.0`; 0.7.x would bring 0.4.9), asupersync lane | `fmine-tui` only | INT-FTUI-001 |
| franken_markdown | library, pinned | `fmine-report` only | INT-FMD-001 (deterministic by digest) |
| frankenmermaid | default features only (its optional HiGHS backend needs C++/CMake) | `fmine-report` only | INT-FMERM-001 |

**Three `Cx` types (B4 S1).** `asupersync::Cx` is the runtime authority. `franken_kernel::Cx` and `fs_exec::Cx`
are different types. Plan prose and code always say which is meant:
- `fs_exec::Cx` may appear only in `fmine-simbridge`, which does not exist yet;
- `franken_kernel::Cx` does not appear at all.

**Version split (B4 S2).** asupersync 0.5 and 0.6 are semver-incompatible, so Cargo links both without
complaint. That is why the one-package-id check (L-5) is a CI gate, not a convention.

**Toolchain.** Pin `nightly-2026-08-31` with the suite. It is OPEN whether the fsym crates compile on it
(OPEN-SUITE-3). Since fsym is build-time only, a failure costs a separate pinned toolchain for `fmine-derive`, not a
runtime risk.

### 27.6 Patterns imported without code

| Pattern | Source | Where used |
|---|---|---|
| Anchors, the knowledge-state lattice, the cognition/authority/effect plane split | FSS | §6, §7, §26 |
| Lift headers, golden-hash guards, budget-overrun discipline | FrankenTextiles (FTX) | §27.1, §28.4 |
| Typed refusals `Violation{code, what, fix}` | frankensim fs-scenario | §8.4 |
| Posture closures, `franken-stack.lock`, foundation dependency rows | FA | §28, Appendix F |
| Robot envelope, exit-code superset, doctor/repair | FA, fgdb | §30 |
| Sealed corpora and matdb manifests | frankensim fs-matdb | §13.10 |

### 27.7 Open questions

- **OPEN-SUITE-1 (closed by the suite-fit audit).** `franken-evidence`, `franken-decision` and `franken-kernel` exist
  at v0.5.0 with `EvidenceLedger`, `DecisionContract`, `LossMatrix`, `DecisionAuditEntry` and `render`. The
  INT-AS-001 spike (BD-257) re-confirms at the pinned revision.
- **OPEN-SUITE-2 (closed: no).** fnp uses Philox4x64-10 and frankensim Philox4x32-10, so cross-checks use golden
  vectors, never stream equality (ADR-RNG-01 is unchanged: the lifted 4x32 is the one RNG).
- **OPEN-SUITE-3.** Do the fsym crates compile on `nightly-2026-08-31` when pulled individually?
- **OPEN-SUITE-4 (closed: yes, and worse).** `fp-io` depends on arrow, parquet and chrono unconditionally, and
  `read_parquet` decodes a whole file as one Arrow batch. `fp-io` is therefore replaced by FrankenMining's own
  CSV/Excel readers in `fmine-io` plus the minimal Parquet subset of §32 (KILL-6 path, taken).
- **OPEN-SUITE-5.** KDTree throughput at 10⁶–10⁷ samples, and which fsci/fnp kernels give thread-count-dependent
  results (§29).

## 28. Workspace, crate map and budgets

### 28.1 Naming

- Crate prefix **`fmine-`**. Binaries: **`fmine`** (CLI), **`fmined`** (node daemon), **`fmine-mcp`**,
  **`fmine-tui`**. Environment variables: `FMINE_*`. Format magic: `FMNE`.
- **Why not `fm`.** Every shorter name collides, verified in code (B4 S7):
  - `fm-*` is frankenmermaid's published crate prefix (`fm-core`, `fm-parser`, … `fm-cli`). The report path links
    frankenmermaid, so an `fm-core` here would put two `fm-core` packages in one graph.
  - `fmd` is franken_markdown's binary.
  - `fm` is `/usr/bin/fm` on macOS (Apple's Foundation Models CLI).
  - `fmn` is franken_manim.
- **`fmine` is free (checked 2026-10-29).** crates.io returns 404 for `fmine`, `fmined`, `fmine-core`, `fmine-cli`,
  `franken_mining`, `franken-mining` and `frankenmining`, and a crates.io search for "fmine" finds no crate (FACT,
  E651). OPEN-B4-09 is closed with the default; BD-014's naming ADR records the check and reserves the names at
  commit 1.
- Plan prose may say "FM". The owner may overrule the prefix, but must then register the collisions as RISK rows.

### 28.2 Layers

Dependencies point downward only: `T → U → V → E → K → S1 → S0`.

| Code | Layer | May | May not |
|---|---|---|---|
| S0 | Substrate | std plus lifted code. **Durable formats live here**, so formats never depend on a runtime version. | Depend on asupersync or any foundation |
| S1 | Runtime and authority | asupersync regions, capabilities, obligations; IO for the ledger, CAS, telemetry segment log and sync transport (QUIC/ATP), the only IO outside V | Contain domain logic |
| K | Domain kernels | Pure functions over `Cx<cap::None>`, `Budget` and `checkpoint`; inner numeric kernels take a `FnMut() -> bool` cancellation closure (§13.9 R-2) | SPAWN, TIME, RANDOM, IO, REMOTE |
| E | Engines | SPAWN tiles; seeded RANDOM from `fmine-core::rand` | IO, REMOTE, TIME (times and seeds arrive as inputs) |
| V | Services and adapters | All other IO and REMOTE: importers, projections, the FA subprocess, services | Domain logic beyond parsing and projection |
| U | Surfaces | Present verbs; no authority path of their own | Write anything except through the verb registry |
| T | Verification | Reach anything (dev-only) | Be depended on by production crates |

**Dependency inversion where §15 names solvers inside `fmine-plan`.** The K-layer modules (`sched`, `blend`, `mpc`)
hold formulations, models and certificate checks. Solves are invoked through a `Solve` trait that K defines and E
implements (`fmine-lp`, `fmine-optimize`), so K never depends on E.

### 28.3 The crate map: 59 crates, 7 layers

Phase of first real slice: **A** = M0–M2 (the 12-month demonstrator), **B** = M3–M4, **C** = M5–M6. ★ marks
the 35 crates that receive a slice in phase A. A crate joins the workspace only with its first real slice. Budgets
are Rust LOC in thousands, including tests. They are TARGETs, and each ceiling is 1.15× the plan.

| Layer | Crate | Subsystem | Responsibility (one line) | Plan k | Ceil k | Phase |
|---|---|---|---|---:|---:|:---:|
| S0 | ★`fmine-core` | Bedrock | IDs and prefixes, anchors, codec kit (magic, LE, bounds, CRC, canonical order), lifted `det`, `rand` (Philox), `hash` (BLAKE3) | 12 | 14 | A |
| S0 | ★`fmine-units` | Bedrock | §25 quantities, bases, exact constants, money boundary type | 10 | 12 | A |
| S0 | ★`fmine-time` | Bedrock | Quad-temporal types (`QuadTemporalStamp`), `TimeEvidence`, site and fiscal calendars, rosters | 8 | 9 | A |
| S0 | ★`fmine-crs` | Portal | EPSG subset, mine grids as Claims, vertical datums, `UNREGISTERED` | 15 | 17 | A |
| S0 | ★`fmine-evidence` | Plumb | Lifted fs-evidence subset; evidence classes; colours; typed refusals; `EvidenceRow`/`DecisionRow` schema types (no franken-evidence dependency) | 14 | 16 | A |
| S0 | ★`fmine-claim` | Plumb | The universal `Claim` (§8), its algebra and the no-laundering rules | 16 | 18 | A |
| S0 | ★`fmine-registry` | Bedrock | Registries-as-data loader and the `registry-check` library | 8 | 9 | A |
| S0 | ★`fmine-array` | Bedrock | Typed contiguous grids and views (§27.3) | 6 | 7 | A |
| S1 | ★`fmine-runtime` | Bedrock | Region tree, CapSet profiles, AppSpec per role, budgets, obligation registry | 12 | 14 | A |
| S1 | ★`fmine-authz` | Bedrock | First-party macaroon caveats, MNPI and venture walls, residency, human authority tokens (§31) | 14 | 16 | A |
| S1 | ★`fmine-ledger` | Bedrock | Delta batches, cells, leases and fencing, witnesses, narrow combiner, MMR, segment files | 24 | 28 | A |
| S1 | ★`fmine-object` | Bedrock | CAS chunks, manifests, fanout-16 Merkle, root-last publication, GC by reachability | 15 | 17 | A |
| S1 | `fmine-telemetry` | Headframe | Site segment log, columnar codecs, ingest backpressure, rollups (§26.8) | 16 | 18 | B |
| S1 | ★`fmine-sync` | Bedrock | Version vectors, `SyncCert`, chunk anti-entropy, QUIC/ATP adapter, residency planner | 16 | 18 | A |
| S1 | ★`fmine-lab` | Testpit | LabRuntime harness, chaos profiles, FM oracles (dev-only) | 10 | 12 | A |
| K | ★`fmine-geom` | Strata | Lifted `VdbGrid`, winding, exact predicates; meshes, block grids, sub-blocks, `volcert`, bricks | 30 | 35 | A |
| K | ★`fmine-numerics` | Bedrock | Lifted sparse/eproc/voi/risk/tropical/enkf; `gmrf`; the array bridge | 30 | 35 | A |
| K | `fmine-materials` | Strata | Sealed geomaterial corpora, correlation cards, matdb-format export (§13.10) | 12 | 14 | B |
| K | ★`fmine-drill` | Corebox | Collars, surveys, desurvey, intervals, compositing, sample support | 15 | 17 | A |
| K | ★`fmine-qaqc` | Corebox | CRMs, blanks, duplicates, detection limits, censored fitting, e-process drift, lab certificates | 15 | 17 | A |
| K | `fmine-geostat` | Strata | Variography, kriging, change of support, conditional simulation | 40 | 46 | B |
| K | `fmine-belief` | Strata | Subsurface belief objects, realization sets, implicit structure, block-model projections | 25 | 29 | B |
| K | ★`fmine-calib` | Plumb | Forecast registration, proper scoring, calibration records, surprise monitors, reference classes (§10) | 18 | 21 | A |
| K | `fmine-explore` | Lodestar | Deposits, exploration effort, absence classes, detection-thinning data model (§11) | 15 | 17 | B |
| K | `fmine-geotech` | Highwall | Rock mass, slope and support laws, instrumentation semantics, TARP automata | 22 | 25 | C |
| K | ★`fmine-flow` | Touchstone | Material-flow graph, conservation laws, F-factors (§20) | 20 | 23 | A |
| K | ★`fmine-recon` | Touchstone | Reconciliation as joint state estimation; gross-error tests; identifiability | 18 | 21 | A |
| K | `fmine-plan` | Pushback | Plan and schedule models, precedence, scenario labels, cut-off, blend, compliance math | 30 | 35 | B |
| K | `fmine-blast` | Headframe | Drill and blast design records, fragmentation and blast-movement models | 15 | 17 | C |
| K | `fmine-ops` | Headframe | As-built, load and haul, reliability, ventilation, dewatering, energy, wear | 35 | 40 | C |
| K | ★`fmine-plant` | Crucible | Flowsheet graph, process cards, geomet chain, P754 balances, refining custody | 32 | 37 | A |
| K | `fmine-tailings` | Freeboard | TSF evidence state, GISTM traceability, water balance, ARD/PAG, closure | 18 | 21 | C |
| K | `fmine-workforce` | Muster | Rosters, competencies, fatigue, CCV, productivity, budgets by driver (privacy by design) | 16 | 18 | C |
| K | ★`fmine-reserves` | Hallmark | Register model, CP/QP acts, renderers, data-verification index, statement reconciliation | 20 | 23 | A |
| K | `fmine-econ` | Helm | Price decks, cost models, fiscal terms (NSR, royalties, tax), NPV with exact money | 20 | 23 | B |
| E | `fmine-estimate` | Strata | Estimation-run orchestration and certificates | 15 | 17 | B |
| E | `fmine-optimize` | Pushback | Pit limits (pseudoflow), Bienstock–Zuckerberg, Lagrangian, TopoSort, stochastic | 36 | 41 | B |
| E | `fmine-lp` | Pushback | PDLP, MILP, QP, MPS I/O, decomposition | 25 | 29 | B |
| E | `fmine-infer` | Lodestar | NUTS, SMC and the thinned point-process posteriors | 24 | 28 | B |
| E | `fmine-prospect` | Lodestar | Prospectivity engine, Bayesian search, drill-target VOI | 22 | 25 | B |
| E | ★`fmine-decide` | Helm | DecisionCards, VOI, sensitivities, robustness views; the only franken-evidence/franken-decision adapter and renderer | 20 | 23 | A |
| E | `fmine-capital` | Helm | CapitalProgram optimization, scenario index, real options, deal cards | 20 | 23 | B |
| E | `fmine-metalacct` | Bullion | Metal-accounting subledger, costing, FA summary preparation, Merkle folding | 30 | 35 | B |
| E | `fmine-acctpack` | Bullion | The 17-part mining domain pack over FA (NRV, stockpile costing, streams, ARO …) | 22 | 25 | B |
| E | ★`fmine-metrics` | Bullion | AISC/AIC/C1/GEO/reserve replacement/ROCE variants, LMDI waterfalls, public lane | 18 | 21 | A |
| V | ★`fmine-io` | Portal | Quarantine-on-import parsers and writers (§32) | 40 | 46 | A |
| V | ★`fmine-public` | Portal | Public-filings lane: issuer filings parsed into Public-basis Claims (§31.1) | 18 | 21 | A |
| V | ★`fmine-projection` | Bedrock | Projectors to fsqlite, fgdb and frankensearch; high-water marks | 20 | 23 | A |
| V | `fmine-fa-bridge` | Bullion | Posting-slot automaton over `fa --json`; outbox; FA-ACC client side | 15 | 17 | B |
| V | ★`fmine-service` | Adit | Node composition (roles, supervisors, leases) → binary `fmined` | 15 | 17 | A |
| U | ★`fmine-cli` | Adit | Binary `fmine`; robot NDJSON; verb registry; doctor and repair | 26 | 30 | A |
| U | `fmine-mcp` | Adit | Thin adapter over verbs (the only fastmcp dependent); parity gate | 10 | 12 | B |
| U | `fmine-tui` | Adit | frankentui ops console (inline mode, asupersync lane) | 15 | 17 | B |
| U | ★`fmine-report` | Adit | franken_markdown + frankenmermaid reports with anchors and `no_claim` lines | 12 | 14 | A |
| U | `fmine-view` | Adit | Read-only 3-D scene export (glTF subset) and a static viewer; no authority path | 15 | 17 | C |
| T | ★`fmine-oracles` | Testpit | Oracle fixtures, known answers, metamorphic relations, conformance | 28 | 32 | A |
| T | ★`fmine-gen` | Testpit | Synthetic mines and districts with seeded truth (§33.3) | 20 | 23 | A |
| T | ★`fmine-verify` | Testpit | std-only offline verifier of certificates, bundles and MMR proofs | 12 | 14 | A |
| T | ★`fmine-derive` | Testpit | Build-time fsym derivations, golden formulas, LaTeX (§27.3) | 8 | 9 | A |
| (gated) | `fmine-simbridge` | Strata | `fs_geom::Chart` adapter for frankensim solvers | 0 | 0 | after GATE-FSIM-RECONV |

**Totals (TARGETs).**

| Layer | Crates | Plan k | Ceiling k |
|---|---:|---:|---:|
| S0 Substrate | 8 | 89 | 102 |
| S1 Runtime and authority | 7 | 107 | 123 |
| K Domain kernels | 20 | 446 | 514 |
| E Engines | 10 | 232 | 267 |
| V Services and adapters | 5 | 108 | 124 |
| U Surfaces | 5 | 78 | 90 |
| T Verification | 4 | 68 | 78 |
| **Total** | **59** | **1,128** | **1,298** |

| Phase | Crates first sliced | Full budgets of those crates (k) | Slice actually built in the phase (k, TARGET) |
|---|---:|---:|---|
| A (M0–M2, demonstrator) | 35 ★ | 615 | ≈ 390 planned, 450 ceiling (subsets of the ★ crates) |
| B (M3–M4) | 18 | 392 | the remainder of phase-A crates as their slices widen, plus these |
| C (M5–M6) | 6 | 121 | — |

- **Calibration of these sizes.** FA is ≈ 796k LOC for accounting alone (R4 §4.2; HEAD `41363acb`). FTX reached 324k for a
  narrower product and overran its plan 2.7× (R5 §2.3). B4's 39-crate map was 738k. This map adds 20 crates for
  the subsystems Part III specified (Lodestar, Headframe, Crucible, Freeboard, Muster, Helm, the public lane and
  the viewer).
- **Reading the numbers.** 1.13M LOC is a ceiling-governed *envelope*, not a promise. The phase-A demonstrator
  is the only commitment, and §37's program gate at month 12 decides whether phases B and C are funded. LOC
  figures are scope caps, never effort or schedule estimates.
- **Velocity reference class (R7-C112, R7-C113, R7-C115).** Across 25 FrankenSuite siblings, lifetime non-blank Rust
  LOC per calendar day (tests included, as `xtask loc` counts) is min 1,106, Q1 ≈ 1,900, median ≈ 3,540, Q3 ≈
  9,000–11,200 and max 38,541. The plan implies ≈ 1,030–1,070 per day (390k in 12 months). Code volume is
  therefore unlikely to bind M0–M2 dates; data gates, reviewers and site access are (FORECAST, unscored; scored
  over the first 60 days as LOC per day against days blocked on a human or external gate). If code runs near
  sibling rates, the §28.4 80/100/115% triggers fire within months, which is their purpose.

### 28.4 Budget-overrun protocol (FTX lesson)

1. `registries/loc_budget.toml` has one row per crate: `{planned, ceiling = 1.15 × planned, charter}`.
   - `xtask loc` is std-only and runs on every commit.
   - Generated fixtures are excluded from LOC and counted separately against a fixture-bytes budget.
2. **At 80% of planned**, `fmine doctor` warns.
3. **At 100%**, the next merge touching the crate needs a `BUDGET-AMEND-nnn` record: the new scope, the gate it
   serves, the new number and owner approval.
4. **At the ceiling**, CI blocks merges to that crate except deletions, test fixes and refactors with net LOC ≤ 0.
5. **A program total above the ceiling** freezes new crates and frontier work. Frontier work never borrows from the
   parity budget.
6. **No smuggling.**
   - `xtask check-scope` maps module families to each crate's `CONTRACT.md` charter, so a new domain cannot hide
     inside an existing crate (the FTX `ftx-pattern` anti-pattern).
   - The delivery log lives in `docs/delivery/`, never in this plan (§0.7).

### 28.5 Layering and dependency rules (`xtask check-*`; GATE-LAYERS-00 from commit 1)

- **L-1 (direction).** Dependencies point downward. T may reach anything, and nothing depends on T through normal
  dependencies. `registries/workspace_topology.toml` declares the same-layer edges in K and E, and CI checks that
  they are acyclic. The intended K order is:
  - K0: `geom`, `numerics`, `materials`, `calib`;
  - K1: `drill`, `qaqc`, `flow`, `econ`, `explore`;
  - K2: `geostat`, `geotech`, `plant`, `tailings`, `recon`, `blast`, `workforce`;
  - K3: `belief`, `plan`, `ops`;
  - K4: `reserves`.
- **L-2 (formats).** S0 is std-only plus lifted code, and durable formats live in S0.
- **L-3 (capabilities).** K may use only `asupersync::{Cx<cap::None>, Budget, checkpoint}`. `check-caps` bans
  `open_child_region`, spawn, IO and time APIs in K and E (defect S3(a), §31.4).
- **L-4 (foundation confinement).**
  - fgdb, fsqlite and frankensearch only in `fmine-projection`, plus the fsqlite catalog in `fmine-telemetry`.
  - fastmcp only in `fmine-mcp`; ftui only in `fmine-tui`.
  - franken_markdown and frankenmermaid only in `fmine-report`.
  - franken-evidence and franken-decision only in `fmine-decide` (the converter to and from the S0 schema types;
    `fmine-report` renders claims through it). serde enters the build only through FMT-ENV-1 and this adapter's
    closure.
  - fsym only in `fmine-derive`.
  - **No FA crate anywhere** (subprocess only).
- **L-5 (banned dependencies).** `check-deps` (deny.toml plus `cargo tree` inversion) bans:
  - tokio, hyper, reqwest, axum (the suite runtime is asupersync);
  - rusqlite, sqlx (fsqlite is the SQL store);
  - arrow, parquet, polars (`fp-io` pulls arrow and parquet unconditionally, so it is not consumed: KILL-6 path,
    §27.7);
  - petgraph (fnx is the graph library);
  - chrono, time **as direct dependencies of any `fmine-*` crate or in any public API** (`fmine-time` is
    std-only). They arrive transitively and unconditionally through fsqlite (`fsqlite-func`), fastmcp
    (`fastmcp-server`), frankenpandas (`fp-types`) and asupersync's `tls` feature (`x509-parser` → `time`), so
    `deny.toml` carries a per-foundation allow-list for exactly those paths and `check-deps` fails on any new path;
  - ort, fastembed (no ONNX runtime in the closure);
  - tantivy (oracle lane only);
  - C solver features such as HiGHS.

  It also requires **one package id** each for `asupersync`, `fgdb-types` and `fnx-classes` per posture closure.
- **L-6 (claims).** `xtask check-claims` rejects a public K/E signature that takes or returns a bare `f64` for a
  physical or monetary quantity (INV-001, PO-UNIT-2).
- **L-7 (lifts).** `xtask check-lifts` checks provenance headers against `registries/lifts.toml` (§27.1).

### 28.6 Repository layout

```text
franken_mining/
├── Cargo.toml                 workspace; [patch.crates-io] keeps one asupersync
├── rust-toolchain.toml        nightly-2026-08-31 (suite pin)
├── franken-stack.lock         exact revisions of every sibling (FA pattern)
├── crates/fmine-*/            each with CONTRACT.md (the ten sections of §0.2) plus public API, POs and budget
├── registries/                loc_budget, lifts, workspace_topology, residency, calendars, crs, units,
│                              io_dialects, mix_rules, refusals, verbs, automata/*, refclass/*, kpi_prices
├── xtask/                     check-layers, check-caps, check-deps, check-lifts, check-scope, check-claims,
│                              loc, registry-check
├── fixtures/                  oracle fixtures, CAS-addressed, under a fixture-bytes budget
├── docs/                      adr/, delivery/, robot-docs/
├── planning/                  this plan, research R1–R8 & W1–W5, Brenner B1–B6, MOR case & audits
└── .beads/                    the work graph (JSONL is truth)
```

**Registries are data, not code.** Every behaviour that an owner, QP or regulator may need to change (residency
rules, KPI prices, mix rules, refusal texts, calendars) is a registry row with a schema, a `registry-check` and a
golden test. Code changes are needed only for new *kinds* of rule.

## 29. Numerics posture and determinism

### 29.1 Why determinism is a product feature here

GOAL-024 promises that an auditor can replay a disclosed number offline and get the same certificate digest.
That promise is cheap for integer and exact computations, expensive for floating-point ones, and false for some
of them unless designed in. This section says which outputs carry which promise.

### 29.2 Determinism classes

Every certificate (`FMT-CERT-1`) records its class. A consumer may not claim more than the class allows.

| Class | Meaning | How achieved | Required for |
|---|---|---|---|
| **Exact** | Bit-identical everywhere, by construction | Integers, `ExactQty`, rationals, content hashes | Ledger, money, posting summaries, conservation checks, Merkle folding |
| **BitwisePortable** | Bit-identical across x86_64-linux and aarch64-darwin | `fmine-core::det` transcendentals (lifted), fixed-chunk reductions in a fixed order, keyed Philox streams, no `HashMap` iteration in outputs, canonical NaN/−0.0 | Disclosure-grade outputs: published realization sets, statement numbers, reconciliation certificates, public-lane metrics |
| **DeterministicPerIsa** | Bit-identical on the same ISA and OS, single-threaded or with deterministic partitioning | Platform libm allowed; same build | Planning and optimization (verified by certificate gaps, not bits), engineering cards |
| **ThreadVariant** | Results may depend on thread count | Consumed kernels with unverified parallel reductions | Interactive exploration only. **Never certified.** |

### 29.3 The hazards, and the rule for each

| Hazard | Fact | Rule |
|---|---|---|
| Transcendentals | `f64::exp`, `ln`, `sin` and `powf` call the platform libm (glibc vs Apple libm), so samplers, CDFs and back-transforms differ across ISAs (R8 §7.3) | BitwisePortable code calls `fmine-core::det` only. `xtask check-det` flags std transcendentals in modules tagged BitwisePortable. |
| FMA | rustc never contracts `a*b + c` without an explicit `mul_add`, and `f64::mul_add` is correctly rounded everywhere | Not a hazard by itself. Explicit `mul_add` is allowed. |
| SIMD lane width | Hand-chunked SIMD sums can change with target features (fp's `fp-dot-kernel` builds with `+avx2,+fma`) | OPEN per kernel. Consumed kernels used in BitwisePortable paths need a cross-ISA golden test, or are replaced. |
| Parallel reductions | rayon in fnp-linalg GEMM and fsci-sparse; fp's per-call thread fan-out | Thread-count dependence is OPEN per kernel (OPEN-SUITE-5). FrankenMining's own reductions use fixed chunks combined in a fixed tree order. |
| Summation order | Floating-point addition is not associative | Pairwise sums over fixed chunks. Where exactness matters (balances), sum in `ExactQty` or with lifted error-free transformations. |
| Hash-map order | Iteration order varies between runs | `HashMap` never feeds an output. Use `BTreeMap` or sort explicitly (PO-BED-9). |
| Discrete branch points | Accept/reject tests, neighbour-search ties and threshold tests turn tiny differences into different paths (GB-37) | Ties are broken by key, never by float order. Stochastic engines whose outputs are disclosed run BitwisePortable, or their outputs are stored rather than regenerated (§13.5). |

### 29.4 Arrays and adapters

There are about nine array representations across the consumed repos (R8 §7.1): fsym `Matrix`, fnp
`UFuncArray`, fp `ColumnData`, fsci-linalg `&[Vec<f64>]`, fsci-sparse `CsrMatrix`, fsci-ndimage `NdArray`,
fsci-spatial input slices, fsci-arrayapi `CoreArray`, and fj `TensorValue`.

- `fmine-array` is the canonical representation: contiguous `Arc<[T]>` plus dims and strides, row-major.
- **`FMT-COL-1` columnar interval chunks (DESIGN).** `fp-io` is not consumed (arrow and parquet are banned, L-5;
  §28.5), yet GATE-STORE-01 and GATE-CORE-01 rebuild and composite 10⁵ holes and 10⁷–2×10⁷ intervals in ≤ 15 min.
  `fmine-array` therefore defines `FMT-COL-1`, an S0, std-only chunk format stored in CAS (the `CELL-DRILL`
  projection cache and `CELL-TEL` segments):
  - run-length `HoleId` (u32 run ends plus a dictionary);
  - delta-varint or packed i32 millimetre depths (`from_mm`, `to_mm`);
  - bit-packed `MeasurandId` and `QualifierBits` (`<DL`, `>UL`, `CommittedFlagged`) with a validity bitmap;
  - 64-byte-aligned f64 buffers for value, dry density and error variance (alignment through `#[repr(align(64))]`,
    no unsafe), and per-chunk zone maps of depth range and 3D bounding box.

  TARGET: the sweep-line overlap join and length-weighted compositing stream 10⁷ intervals in < 3 s on one core
  with no per-interval allocation. Chunks convert to `fp-columnar` and fsci slices without copying where layouts
  match.
- `fmine-numerics::bridge` provides zero-copy views into flat `&[f64]` consumers (fnp-linalg, fsci-sparse, fp)
  and copying adapters for everything else.
- An adapter declares whether it copies. Copying adapters on hot paths are tracked as upstream asks (flat-slice
  overloads in fsci, Appendix F).

### 29.5 Randomness

- One generator: keyed Philox in `fmine-core::rand` (ADR-RNG-01, §27.3). The key schema is
  `(seed_root, partition, domain, var_group, r, brick)`. Any block of any realization can be regenerated
  independently, which is what makes "regenerate, don't store" possible (§13.5).
- Integer streams are bitwise portable. Distribution transforms (normal, gamma, beta) use `det` transcendentals
  in BitwisePortable code.
- Every engine receives its seed as an input, and the seed is recorded in the certificate. No engine reads
  entropy (E layer has no TIME or IO, §28.2).
- The realization index is shared across engines (§9.5), so the same `r` means the same world in geology,
  planning and economics.

### 29.6 Numerical certificates: a number never claims more than its certificate

| Computation | Certificate fields | Refusal or downgrade when it fails |
|---|---|---|
| Linear solves (kriging, GMRF, reconciliation) | Relative residual ‖Ax − b‖/‖b‖, condition estimate, ordering | Residual > tolerance → the Claim is refused (FMINE-REF-2xx) or recomputed with a fallback solver |
| LP/MILP (§15) | Primal and dual objectives, gap, feasibility violations | The three-gap ledger (§15.2); unknown gap → labelled as such |
| Volumes and proportions | Interval enclosures (`volcert`) | Band too wide → colour `Estimated` |
| MCMC and SMC | R-hat ≤ 1.01, bulk and tail ESS ≥ 400, divergences, SBC rank checks on synthetic truth | Fails → `Uncalibrated`, never published as a posterior |
| Quadrature (gamma-bar) | Rule order, error estimate against fsym-derived goldens | Error above tolerance → finer rule |
| Root finding (IRR, cut-offs) | Bracket, iterations, residual | No bracket → refusal with the reason (e.g. multiple IRRs) |

### 29.7 Determinism lanes, golden hashes and performance

- CI runs determinism lanes on x86_64-linux and aarch64-darwin:
  - BitwisePortable fixtures must produce identical digests on both;
  - DeterministicPerIsa fixtures must reproduce per ISA, and are compared across ISAs by certificate tolerance;
  - debug and release builds must agree.
- Digest drift blocks release (PO-BED-9).
- Performance is profile-driven. There is no GPU path in v1, because frankenjax's GPU backend is a stub and has no
  WASM target. Scale targets (10⁸ blocks, 100+ realizations) are met by regeneration and bricking (§13.5), not by
  accelerators.

| PO / Gate | Statement | Verification |
|---|---|---|
| PO-NUM-1 | BitwisePortable fixtures give identical digests across both reference ISAs | Determinism lanes |
| PO-NUM-2 | No std transcendental appears in a module tagged BitwisePortable | `xtask check-det` |
| PO-NUM-3 | Every consumed kernel used in a certified path has a recorded determinism class and a thread-count test | Kernel registry `registries/kernels.toml` |
| GATE-DET-01 | 100% of disclosure-grade fixture families BitwisePortable, verified on both ISAs | M1, then every release |

### 29.8 Galaxy-brain card

**GB-37 · One ulp is harmless to a decision and fatal to a digest, and a few engines turn it into a different
world.**
- **Equation.**
  - A perturbation δ₀ ≈ 2⁻⁵² ≈ 2.2·10⁻¹⁶ (one libm ulp) grows per iteration by a factor g.
  - It reaches a size δ* that flips discrete choices with appreciable probability after
    k = ln(δ*/δ₀)/ln g iterations.
- **Substituted.**
  - In a non-contractive sampler regime (a multimodal posterior over a HypothesisSet), take g = 1.01 and
    δ* = 10⁻³ relative. Then k = ln(10¹³)/ln(1.01) ≈ **3,000 iterations**. Inside a 10⁴-iteration run, the two
    replays become **different samples** of the same distribution.
  - In a contractive regime (strongly log-concave, where synchronous coupling of HMC contracts), g < 1 and the
    replays converge.
  - For a single cut-off decision on 10⁸ blocks with smooth grades, a 1-ulp perturbation flips an expected
    ~10⁸ × 10⁻¹⁵ ≈ 10⁻⁷ blocks: negligible.
- **Intuition.**
  - Decisions are robust to ulps. Digests are not.
  - Engines with discrete branch points and non-contractive dynamics turn ulps into different realization
    sets, which would make an auditor's replay *statistically* equivalent but not *verifiably* identical.
- **Decision.**
  - Disclosure-grade stochastic outputs run BitwisePortable, or are stored as objects and never regenerated.
  - Everything else is verified by certificate tolerance and statistical equivalence.
- **Assumptions.** g = 1.01 is illustrative. The real growth rate is measured per engine on synthetic truth.
- **Flip.** If `det` transcendentals cost more than ~2× the platform libm in profiling, store disclosed
  realization sets instead (§13.5 GB-10 trade-off).

## 30. Adit: surfaces and the agent model

### 30.1 The robot envelope and exit codes

**Envelope (`FMT-ENV-1`).** `--json` emits one event per line:

```json
{"v":1,"event":"<schema-id>","node":"<node_id>","anchor":{"<cell>":<seq>},
 "data":{},"certificate":"<digest or null>",
 "flags":{"reserve_disclosure_claim":false,"execution_authorized":false,
          "geotechnical_signoff":false,"regulatory_compliance_claim":false},
 "warnings":[],"next_actions":[{"command":"…","why":"…"}]}
```

- The four flags are **always false**. The envelope type has no setter for them (INV-002), and a golden test
  checks every schema.
- Every number in `data` is a serialized `Claim` or a reference to one. There are no naked numbers (§8).
- `next_actions` tells an agent or operator exactly what to run next, with the reason.
- serde is permitted in this format only.

**Exit codes** are a superset of `fa`/fgdb:

| Code | Meaning |
|---|---|
| 0 | OK |
| 2 | Usage error |
| 3 | Domain-law refusal (conservation, QA/QC, basis, support) |
| 4 | Environment or degraded mode (e.g. a read-only node after writer escalation) |
| 5 | Foundation I/O error |
| 6 | Conflict: lease, fencing, stale anchor, idempotency |
| 7 | Safety or authorization: MNPI wall, venture wall, residency, missing human token |
| 8 | Verification failure (certificate, proof, digest) |
| **9** | **Indeterminate.** An effect or post may have happened. Reconcile by key; never retry blindly. |

`1` is never emitted, so a generic crash can never be mistaken for a typed outcome.

**Refusal family → exit code (R7-C092, R7-C100).** 1xx domain law → 3; 2xx evidence (insufficient, conflicted, stale,
uncalibrated) → 3; 3xx authorization and walls → 7; 4xx safety-semantic → 7; 5xx anchor and artifact integrity →
8 for a digest or signature mismatch and 6 for a stale anchor. Usage errors (unknown issuer, malformed id) are exit
2 with no refusal code. A missing or unreadable registered input is a typed refusal that names the file, never a
digest-mismatch code. `registries/refusals.toml` is the single source: the compiled catalogue is generated from it,
and a CI check compares code, meaning and exit code in both directions with §8.4, §25.6 and §31.3.

### 30.2 The verb registry, CLI, doctor and repair

- **One registry, many projections.** `registries/verbs.toml` defines every verb:
  - name, cluster and kind (`read`, `prepare`, `admin`);
  - required capabilities and walls;
  - input and output schema ids;
  - possible refusals and exit codes;
  - idempotency key rule;
  - examples.

  The CLI, MCP and TUI are generated from it, and none of them has a verb the registry lacks.
- **Bare `fmine`** prints triage: live obligations, stale cells, open refusals, gates near threshold, budgets
  near their limits.
- **Representative verbs:**

| Verb | Kind | What it does |
|---|---|---|
| `fmine claim show <id>` / `why <id>` / `trace <id>` | read | Show a Claim; explain its value (evidence, colour, UU term); walk its provenance DAG |
| `fmine assure public --issuer <ticker>` | read | The M0 public lane: reproduce published metrics from filings with certificates (§21.6, §37) |
| `fmine forecast register` / `score` | prepare | Freeze and hash a forecast before resolution; score it after (§10) |
| `fmine recon run` / `close` | prepare | Run the reconciliation posterior; prepare a period close for attestation (§20) |
| `fmine plan certify <plan>` | read | Produce the three-gap PlanCertificate (§15.2) |
| `fmine post prepare` / `status` | prepare | Prepare a posting slot; show slot states (§21.3) |
| `fmine decide card <decision>` | prepare | Draft a DecisionCard with VOI and flip margins (§22) |
| `fmine reserve pack <estimate>` | prepare | Assemble the CP/QP support pack and data-verification index (§24) |
| `fmine verify <bundle>` | read | Offline verification of certificates, bundles and MMR proofs (also as `fmine-verify`) |
| `fmine report <kind>` | read | Render a report with anchor, certificate and `no_claim` line |
| `fmine robot-docs` / `robot schema <id>` | read | Machine-readable documentation and JSON Schemas for agents |
| `fmine doctor` | read | Hash chains, high-water marks, pins, budgets, obligations, the Little audit, residency and wall configuration |
| `fmine repair plan` / `repair apply <digest>` | admin | `plan` emits a digest-addressed repair plan. `apply` executes exactly that plan and nothing else, so nothing can drift between review and execution. |

### 30.3 Daemon, MCP and TUI

- **`fmined`**, one per node, holds the cell leases and serves `<node>/fmined.sock`. Its supervisor tree is the
  role's AppSpec (§26.7). The CLI talks to it when present. Without it, the CLI runs read-only against
  projections and says so.
- **`fmine-mcp`** projects the verb registry as MCP tools.
  - Tools come in clusters of at most 7, each cluster a task (for example *reconciliation*, *forecasts*,
    *reserves pack*), so an agent's tool list stays legible.
  - It is read-mostly. Effects are `prepare_*` only.
  - A **parity gate** (INT-FMCP-001) diffs tool schemas and refusals against the registry in CI.
- **`fmine-tui`** is an ops console in frankentui inline mode. It shows sync lag per cell, ingest ρ, obligation
  queues, posting slots and refusals. It adds no authority path.

### 30.4 The agent model: agents prepare, humans authorize, and everyone is scored

AI agents (internal, Avathon's, or a third party's) are first-class principals, not a back door.

| Rule | Mechanism |
|---|---|
| Agents read the cognition plane and cite Claims | Caveated tokens: agent id, scope, walls, ventures, residency |
| Agents never write authority | Agent outputs enter `CELL-QUAR` as evidence class `AgentUpload`, `REVIEW_REQUIRED`. They become authority only when a human with the role adopts them. |
| Agents never upgrade evidence | The no-laundering rules apply to agents as to engines (§7.3) |
| Agents may prepare effects and DecisionCard drafts | `prepare_*` verbs only. Release needs a HumanAuthorityToken (§31.3), which agents cannot mint. |
| Agent forecasts are registered and scored | An agent's forecasts enter the calibration ledger with the agent as author (§10). Calibration is reported per agent, and trust (for example, triage ordering) is earned by track record. |
| Walls apply to agents unchanged | MNPI and venture walls bind agent sessions. Every agent read is an AccessEvent and appears in insider-list logs (§31.1). |
| Imported text is data, not instructions | Documents, filings and vendor exports pass quarantine-on-import. Text in them never becomes a command. Agent-proposed effects cite the Claims they rely on, and the release card shows them. |

**Agent ergonomics.** Typed refusals with a `fix` hint, `next_actions`, stable schema ids, idempotent `prepare`
verbs with keys, and `robot-docs` that a fresh agent can learn from. GATE-ROBOT-01 tests this directly.

### 30.5 Rendering and reports

- Claims render through `franken_evidence::render::{level1, markdown, html}` plus FrankenMining rows: the
  evidence class badge, KK/KU/UU, the flip margin and the calibration flag.
- Reports render through franken_markdown (HTML/PDF), with diagrams from frankenmermaid (default features):
  flow graphs, claim DAGs, plan Gantt charts.
- Every report carries its anchor, its certificate digest and a `no_claim` line, for example: "This report makes
  no reserve disclosure claim, no execution authorization, no geotechnical sign-off and no regulatory compliance
  claim."
- Reports are deterministic by digest (INT-FMD-001): the same anchor and inputs give the same bytes.
- **Language rules:**
  - ranges, not points;
  - evidence class next to every number;
  - "evidence state", never "safe" (§18.1);
  - colour-blind-safe palettes;
  - every number clickable to its Claim;
  - plain sentences on executive pages: each §9.6 field written out in words (for example "inside its 80% range
    in 15 of 20 quarters", "the conclusion reverses if gold falls below $2,900/oz"), with the coded map kept as
    the analyst view.
- The executive one-pager is the epistemic map (§9.6). The K5-style test (KILL-PLUMB-5) shows its plain-sentence
  rendering first and the coded layout second; if users fail both, it falls back to a three-field card: range,
  evidence class, flip margin.

### 30.6 Proof obligations and gates

| PO / Gate | Statement | Verification / threshold |
|---|---|---|
| PO-ADIT-1 | The four flags are false in every envelope of every schema | Golden test over all schema ids (INV-002) |
| PO-ADIT-2 | CLI, MCP and TUI expose exactly the registry's verbs, with identical schemas and refusals | Parity diff in CI |
| PO-ADIT-3 | `repair apply <digest>` executes exactly the plan with that digest | Lab test with plan tampering |
| PO-ADIT-4 | No exit code 1 is emitted on any path | Fuzzed CLI invocations |
| GATE-ROBOT-01 | A fresh agent, given only `fmine robot-docs`, completes 20 scripted tasks (≥ 18 correct, 0 authority writes, 0 wall breaches) | M1 |
| GATE-REPORT-01 | A board-pack report regenerates byte-identically from its anchor | M3 |

## 31. Security, confidentiality and OT safety

**Verdict (B4 SQ6): layered security.** Two simpler hypotheses were refuted:
- *Capabilities alone* (trust `Cx`) is refuted by defects S3(a)–(c) at the 0.5.0 pin (§31.4).
- *Perimeter only* (FrankenMining trusts its callers) is refuted by securities and JV law: insiders need
  need-to-know walls, not just a perimeter.

The adopted design layers four things: typed CapSet regions, first-party macaroon caveats compiled before query
expansion, physically separate projections where caveat compilation is not yet live, and fail-closed registries.

### 31.0 Threat model

| Actor | Goal or risk | Primary controls |
|---|---|---|
| Insider with a trading motive | Learn drill results or reserve changes before release | MNPI walls, wall-scoped projections, insider-list logs (§31.1) |
| JV partner | See the other partner's private data | Venture walls, per-venture projections, verifiable bundles (§31.2) |
| External attacker via vendor files | Code execution or data poisoning through imports | Quarantine-on-import, pure-Rust reimplemented parsers, fuzzing (§32) |
| Compromised or manipulated agent | Prompt injection, authority writes, wall breaches | Agent model (§30.4), `CELL-QUAR`, tokens agents cannot mint |
| Attacker pivoting toward OT | Command equipment through the twin | No write conduit to Levels 0–2; outbound-only sessions (§31.3) |
| Cross-border transfer | Breach of residency or mining-data law | Residency registry enforced before bytes move (§31.2) |
| Litigation or discovery | Records that read as admissions or disclosure claims | Four false flags, `no_claim` lines, the counsel review in OPEN-HALL-4 |

### 31.1 MNPI: embargo states, walls and the public lane

- **Embargo state.** Every assay and intercept record carries
  `disclosure: Embargoed{wall, since} → Released{public_ref, at}`. The transition to `Released` cites the public
  document (news release URL or filing id) and its valid time.
- **Walls.**
  - Reading embargoed data requires the caveat `mnpi_wall=W-…`.
  - There is **no global search or graph index over embargoed data.** Projections are wall-scoped, so neither
    counts nor absence leak across a wall.
  - Every access is an `AccessEvent`. Together they form an insider-list-ready log per wall. The exact list
    format per listing venue (TSX, NYSE, and any proposed listing) is OPEN-B4-07.
- **Exports refuse.** Export, report, MCP and Avathon feeds refuse embargoed rows with exit 7 (FMINE-REF-301),
  unless the request cites a human-held `mnpi_release` capability.
- **Taint is monotone (INV-007).** Confidentiality is a join-semilattice
  `Public < Internal < Venture(V) < Mnpi(Wall)`, and derived data is at least the join of its inputs. MNPI-derived
  bytes never reach a public export, and export fuzzing tests this.
- **Site code stays in a private overlay.** Site registries, profiles, configuration, fitted models and any
  site-specific code live in a private overlay on the issuer's network, never in this repository. A change moves
  from the overlay to the public core only as a reviewed patch: the issuer approves it, and it carries no site data,
  fitted values, site identifiers or fixtures built from site data (INV-007's taint rule applied to code).
- **The public lane is separate.**
  - `fmine assure public` (M0) uses only public filings and issuer-website workbooks. A website workbook has no
    filing accession, so its registry entry holds the file's sha256 and the filed report page that reconciles it
    (§21.6; E107).
  - No public-lane author holds private access. *Private access* means being on an issuer insider list, being a
    party to a data agreement with the issuer, or having received non-public issuer information (R7-C136, R7-C137).
  - At GATE-DATA-01 the issuer's public lane is frozen at its last registered state. It is then either retired for
    that issuer or authored only by a walled author set with signed no-access attestations. AI agents cannot sign
    and a one-owner team may have no eligible author, so retirement is the default. M2′ public-lane content is
    then limited to reproducing already-published values and scoring the pre-registered FY2026 forecasts unless
    counsel clears more; BET-02's scored quarters come from the in-tenancy forward quarters plus FY2026.
  - Public-lane forecasts are hashed, pushed and externally time-stamped *before* the issuer's release and before
    any issuer insider sees M0 content (§10, §21.6, §37).
  - Issuer data terms (RISK-31): until the owner and counsel decide, the repository holds only locators
    (`file:Sheet!Cell`), source URLs, sha256 and computed check results. Fixtures that need values are synthetic and
    labelled, or fetched at test time from the pinned URL. Barrick's site terms allow copying for non-commercial,
    personal or educational use of unmodified material and otherwise need approval (R7-C052, R7-C158, R7-C160).
  - Internal sensitivity content never reaches a public renderer (G-HALL-5).

### 31.2 Venture walls, residency and personal data

**Joint-venture walls.**
- Cells and projections are partitioned by `venture`. Partner principals receive `venture=` and redaction
  caveats.
- Until fgdb's caveat compilation (Warden) is live, **per-venture projection instances** (physical separation)
  are used instead of row filtering.
- Partners receive FA `.fab` bundles, verifiable with `fa-verify`, and FrankenMining certificate bundles,
  verifiable with `fmine-verify`. A partner can check numbers without seeing the other partner's private data.
- A query spanning ventures without a cross-venture capability is refused (FMINE-REF-302).

**Data residency.**
- `registries/residency.toml` maps `(country, data_class)` to `{in_country_only, aggregates_exportable,
  exportable}`.
- The sync planner treats it as a hard constraint. Regional nodes hold in-country authority, and
  `residency_epoch` is part of every anchor.
- An unauthorized class transfer is refused before any bytes move (exit 7, FMINE-REF-303), and `fmine doctor`
  reports it.
- Each row of `registries/residency.toml` cites its statute and section, a status (FACT, FACT[U], INFERENCE), a
  confidence and a `review_by` date; the 72 rows researched for 12 countries are in W3 §4 (10 FACT, 23 FACT[U], 39
  INFERENCE; E501–E549). An unknown `(country, class)` pair still defaults to `in_country_only`. Research, not legal
  advice: counsel confirms the W3 §7 clauses before any regional node goes live (OPEN-B4-01, narrowed).

  | Country | `personal` | `geoscience` | `production` | `financial` | `state_secret` | `aggregate` |
  |---|---|---|---|---|---|---|
  | PK | INⁱ | AGGⁱ | AGGⁱ | AGG | INᵘ | EXPⁱ |
  | ZM | IN | AGGⁱ | AGGⁱ | AGGᵘ | IN | EXPⁱ |
  | TZ | IN | IN | AGGⁱ | AGG | INᵘ | EXPⁱ |
  | CD | IN | AGGⁱ | AGGⁱ | AGGᵘ | INᵘ | EXPⁱ |
  | ML | IN | AGGⁱ | AGGⁱ | AGGᵘ | INᵘ | EXPⁱ |
  | PG | INⁱ | AGGⁱ | AGGⁱ | AGGᵘ | INᵘ | EXPⁱ |
  | DO | INᵘ | AGGⁱ | AGGⁱ | AGGᵘ | INᵘ | EXPⁱ |
  | AR | INᵘ | AGGⁱ | AGGⁱ | AGGᵘ | INᵘ | EXPⁱ |
  | CL | IN | EXPⁱ | EXPⁱ | EXPⁱ | INᵘ | EXPⁱ |
  | SA | INᵘ | AGGⁱ | AGGⁱ | AGGᵘ | INᵘ | EXPⁱ |
  | US | EXPᵘ | EXPⁱ | EXPⁱ | EXPⁱ | INᵘ | EXPⁱ |
  | CA | EXPᵘ | EXPⁱ | EXPⁱ | AGG | INᵘ | EXPⁱ |

  IN = `in_country_only`, AGG = `aggregates_exportable`, EXP = `exportable`; ᵘ FACT[U], ⁱ INFERENCE, unmarked FACT.
  Pakistan (Reko Diq): `personal` and `state_secret` stay in-country; `geoscience`, `production` and `financial`
  are `aggregates_exportable` (BMR 2002 r.56, r.71; Companies Act 2017 s.220; E504–E508). Zambia (M2): `personal`
  is `in_country_only` (DPA 2021 s.70; E514), so the pilot node keeps rosters, fatigue and HR data on site; mining
  is a critical sector under the Cyber Security Act 2025, and a Gazette designation would pull `production` in-country
  (s.12; E520–E522).
- **Two legal shapes the three-value rule cannot express (W3 §1).** "The authoritative copy stays in-country,
  replicas are allowed" (most book-keeping statutes) and "exportable once instrument X exists" (most data-protection
  laws: a permit, model clauses, consent). Rows may therefore carry optional `requires` (the instrument whose signed
  registration would relax the row: a Zambian ODPC-approved scheme, a Tanzanian PDPC permit or GST authorisation, a
  Chilean model clause), `authority_in_country` and `deny_destinations` fields. Until that instrument is registered
  the row's `rule` stands; `rule` alone remains the fail-closed contract. One row differs: Pakistan `geoscience` is
  researched as `aggregates_exportable`, but its `requires` is counsel's reading of BMR r.71(3)(i) and the Reko Diq
  agreement, so the planner holds it `in_country_only` until that reading is registered.
- **State-equity ventures.** A venture with government holders (Reko Diq: federal SOEs 25%, Balochistan 25%, of
  which 10% free-carried; E302) adds sovereign partner principals, and §31.2 partner bundles serve government
  holders as well as listed companies. Holder kinds and interest bases are typed in `OwnershipRef` (§24.3).

**Personal data (Muster).** Person-level data (rosters, fatigue, competencies, CCV) is pseudonymized with a key
held by the privacy-officer role (PO-MUST-4). It lives in restricted lanes, and its use is gated on a privacy
impact assessment and, where applicable, works-council review (GATE-MUST-03).

**Community and stakeholder data.** GATE-DATA-05 and PO-MUST-4 pseudonymization also apply to stakeholder,
consultation and grievance records. Importers redact on ingest **even from public documents**: Reko Diq's ESIA
appendices include individuals' names and contact details, and its grievance form collects name, contact,
house location and signature (C5 §4.9). FrankenMining never reproduces those fields.

### 31.3 OT safety and the HumanAuthorityToken

- **No write conduit to ISA-95 Levels 0–2 exists or will exist** (INV-011, NONGOAL-002).
  - Ingest is pulled through a DMZ relay or a data diode, using outbound-only sessions.
  - Historians are read through exported snapshots or a read-only OPC UA client on the relay, never a write
    session.
- **Effects are prepared, never actuated.** `EffectRegion` emits `EFFECT-*` documents only: work orders, dig-line
  advice, blast designs for approval, shipment releases, posting intents and data releases.
- **HumanAuthorityToken.** Even releasing a *prepared* effect needs a token:
  - a first-party macaroon minted by a named human through an interactive, strongly authenticated step;
  - caveats `{effect_digest, site, expires ≤ 15 min, single-use nonce}`;
  - mint is an `admin` verb that is never exposed through MCP, so agents cannot mint tokens.
- Some effects need **dual attestation**, for example concentrate shipment release (§17.6).
- **Manual gate and valve positions are evidence, not commands.** They enter as attested observations with a
  freshness bound (§18.3). A stale or unknown position renders `position unknown` and can never support a
  "contained" statement (G-FREE-2). FrankenMining has no write path to the devices.
- `execution_authorized` is never set true (INV-002).
- **Safety-semantic refusals** apply before any effect is prepared:
  - FMINE-REF-401: unknown reactive ground → no ANFO plan;
  - FMINE-REF-402: unknown PAG → no NAF dump plan;
  - FMINE-REF-403: unknown moisture vs TML → no shipment release;
  - FMINE-REF-404: stale TSF instrumentation → no "within TARP" statement;
  - FMINE-REF-504: adjacent paste or cemented fill uncured or degraded (`FillStrength(t) < UCS_design`) → no
    secondary-stope blast plan (§16.3, G-HEAD-7).
- A missing or expired token is refused (exit 7, FMINE-REF-304). There is no fallback actuation path, so nothing
  can fail open.

### 31.4 asupersync 0.5.0 defects and their mitigations

The suite pin, 0.5.0, carries known defects (B4 S3; asupersync CHANGELOG §v0.6.0). 0.6.0 fixes (a), (b) and (d)
and adds (e). **(c) is unchanged in 0.6.0**, which only adds a protected third-party-caveat API, so the
first-party-only rule stays in force after the bump.
FrankenMining stays on 0.5.0 with the rest of the suite and mitigates each defect explicitly.

| Defect at 0.5.0 | Risk | Mitigation | Test (INT-AS-SEC-001) |
|---|---|---|---|
| (a) A restricted `Cx<cap::None>` can call `open_child_region` and receive `Cx<cap::All>` at the type level (at 0.5.0 the runtime mask still refuses SPAWN, so the lab mask test is the real control) | Capability escalation from kernels | `xtask check-caps` bans `open_child_region` outside `fmine-runtime`; each region's runtime mask is asserted in the lab | Lint + runtime-mask lab test |
| (b) `MacaroonKeyRing::verify` accepts an all-zero signature when no retired key is set | Token forgery | The key ring is always constructed with a random retired key, plus an FM wrapper that rejects all-zero signatures | Regression test with a zero signature |
| (c) Legacy third-party caveats can be stripped by the third party | Delegation bypass | **First-party caveats only.** A third-party caveat in an FM token is refused at mint. | Mint refusal test |
| (d) Plaintext ATP-over-TCP is not refused off loopback; its manifest is unauthenticated | Interception or tampering in transit | ATP and sync run **only over QUIC/TLS**. The plaintext transport is banned by config and `check-deps`. Chunk hashes are verified end to end regardless of transport. | Plaintext attempt → refusal (exit 7) |
| (e) Durable ATP sender/receiver journaling is a 0.6.0 feature | Lost progress across crashes | Resumable chunk receipts and OB-SEAL (§26.7) | Lab partition matrix (INT-ATP-001) |

**GATE-PIN-06 (coordinated bump).** FrankenMining moves to 0.6.x only when fgdb, FA and fastmcp have moved, and it
re-runs INT-AS-SEC-001 when it does. It follows the suite and never leads.

**OPEN-B4-02, answered as far as the record allows (2026-10-29).**
- There is no 0.6 release yet. crates.io's newest asupersync is 0.5.0 (12 Sep 2026); the repository's main branch
  declares 0.6.0, but no v0.6 tag exists (FACT, E653, E654).
- Every sibling that takes asupersync from crates.io pins 0.5.x (`=0.5.0`, or a caret `0.5.0` that excludes 0.6),
  and the fgdb, FA, frankensqlite and frankensearch lockfiles resolve 0.5.0. Only frankensympy's runtime crates
  (`fsym-runtime`, `fsym-durable`, `fsym-python`) and frankensim follow main by path (FACT, E655).
- FrankenMining is unaffected today: `fmine-derive` uses `fsym-core`, which has no asupersync dependency, and
  frankensim enters by lift, not by link.
- The bump is coordinated by the suite maintainer who owns asupersync and the sibling repos. The trigger is a
  published 0.6.x followed by fgdb, FA and fastmcp pins; until then FrankenMining stays on `=0.5.0` with the §31.4
  mitigations, and `check-deps` fails any posture closure in which a path dependency brings in a second asupersync.

### 31.5 Supply chain and build integrity

- **Closed dependency universe.** Every external crate is on an allow-list with a reason. `deny.toml` and the L-5
  bans are CI gates. `xtask check-deps` inverts `cargo tree` to prove that no banned crate is present
  transitively.
- **Reproducible releases** go through DSR with minisign signatures and an SBOM per binary. The release digest is
  recorded in every certificate's code-revision field.
- **Parsers are pure Rust and fuzzed** (one fuzz target per decoder, §26.5 and §32).
- **Licence.** Lifted code and much of the substrate carry "MIT + OpenAI/Anthropic Rider". Whether that rider is
  acceptable in a client-facing product is an owner and counsel decision (RISK-17, OPEN-STRATA-6).

### 31.6 Proof obligations and gates

| PO / Gate | Statement | Verification / threshold |
|---|---|---|
| PO-SEC-1 | Wall-scoped queries reveal nothing across walls, counts and absence included (= PO-BED-8) | Differential tests: the same query with and without the wall's data must produce indistinguishable outputs for an outside principal |
| PO-SEC-2 | No embargoed byte reaches a public export | Export fuzzing over the taint lattice |
| PO-SEC-3 | A HumanAuthorityToken is single-use, expires within 15 minutes and binds one effect digest | Token property tests; replay attempts in the lab |
| PO-SEC-4 | No code path writes to an OT endpoint | `check-deps` (no OT write client in the closure) + deployment review |
| GATE-WALL-01 | 0 cross-wall leaks over 10⁴ generated query pairs | M1 |
| GATE-SEC-01 | An independent penetration test of a site node and relay before any production site data is ingested | Before M2 |

## 32. Portal: industry interoperability inside a closed universe

### 32.1 The rule

Production code is either lifted (with provenance and a licence check) or reimplemented. Foreign tools are pinned
**lab oracles**: they capture fixtures, and those fixtures are never regenerated from the candidate (§33.2).

**Quarantine-on-import.** Every import follows the same pipeline:
1. bytes land in a read-only staging area inside `ImportQuarantineRegion` (no TIME, no RANDOM, no REMOTE);
2. a pure-Rust parser decodes them under a byte and time budget;
3. a **dialect row** declares the unit, basis, support and CRS of every column, or the column stays
   `UNREGISTERED` (FMINE-REF-106/107);
4. the parsed records become evidence in a cell, citing the original file's CAS digest;
5. the import report lists rejected rows with reasons. Nothing is silently dropped.

**Import rules from the corpus.**
- **Header text is not semantics.** A column's basis (contained vs recovered; 100% vs attributable) is asserted
  in the dialect row and checked by an identity test (Kibali's "Contained Gold" column holds recovered ounces;
  E240).
- **Workbooks are label-anchored.** Spreadsheet rows drift between releases (E121), so xlsx tables are located by
  label, not by row number, and `#REF!` residue is excluded (E120).
- **Textbook guideline tables** are imported with column names mapped to neutral terms (e.g. "guideline
  cut-slope angle"); the word "safe" never reaches a rendered label (E398; G-FREE-1).
- **Draft documents.** Every `DocLocator` carries `DocApproval`; cells sourced from an unapproved revision cap at
  `conflicted` or `stale` (RISK-29).
- **Rights.** The corpus textbooks (Abzalov, *Applied Mining Geology*; Waltham, *Foundations of Engineering
  Geology*; Lacy; a lecture note) are reference-only. The plan cites their pages. Shipped fixtures are synthetic
  and re-derived from the cited method; a textbook's own tables and worked numbers are used only as a private
  cross-check and are never shipped (C6).

### 32.2 Formats and systems

| Format / system | Posture | Oracle | Note |
|---|---|---|---|
| OMF v2 | Lift the omf-rust (MIT) schema/JSON layer **only after a dependency audit** | Pinned omf-rust build, lab lane only | HYPOTHESIS: OMF v2 stores arrays as Parquet inside a zip, and omf-rust may link `arrow`/`parquet` (banned, L-5). If so, reimplement a minimal Parquet subset (plain + RLE/bit-packed levels). OPEN-B4-06. |
| LAS 1.4 | Reimplement (fixed binary layout) | PDAL / LAStools | — |
| LAZ | Lift a pure-Rust laszip decoder after a licence check, or defer | LAStools | Decode first, encode later |
| E57 | Reimplement a subset (XML + CRC'd binary pages; cartesian, intensity, pose) | libE57Format | — |
| GeoTIFF / DEM | Reimplement TIFF baseline + GeoKeys (none/deflate/LZW) | GDAL | — |
| DXF | Reimplement an ASCII subset (LINE, (LW)POLYLINE, 3DFACE, POINT, layers) | ezdxf / ODA | DWG is out of scope |
| CRS | `fmine-crs` (§25.5) | PROJ | — |
| acQuire, Datamine, Surpac, Leapfrog, Deswik | **Dialect registry** `registries/io_dialects.toml` over their CSV, OMF, DXF and string-file exports. Surpac `.str` text is reimplemented; Datamine `.dm` enters only through export. | Vendor exports as fixtures | Units and CRS declared or `UNREGISTERED` |
| SAP S/4HANA | Read-only flat-file or IDoc extracts, or OData JSON over asupersync HTTPS. **No RFC SDK.** | Sample extracts | SAP remains the ERP and the statutory ledger. FA runs beside it as an analytical book that rebuilds SAP's trial balance from extracts and must agree with it (§1.5). FM↔SAP is snapshots (work orders and spares in §16.5). |
| Fleet management (DISPATCH, MineStar, FrontRunner/Modular, Wenco, Pitram) | External collectors publish **content-hashed immutable snapshots** into quarantine. Engines read snapshots only. | Vendor exports | FrankenMining never writes back to dispatch or autonomous haulage (§31.3) |
| Historians (AVEVA PI and others) | Exported snapshots, or a read-only OPC UA client on a DMZ relay, outbound-only | Recorded sessions | Tag maps are versioned registry rows (§16.9) |
| Public filings (SEDAR+, EDGAR, issuer releases) | `fmine-public` parses filings into Public-basis Claims | Hand-checked fixtures | The M0 lane (§31.1) |

✖ **KILL-6 (OMF purity).** If omf-rust needs banned dependencies *and* the Parquet subset exceeds 8k LOC: exchange
OMF through lab-oracle conversion to CSV/DXF in the interim, and say so.

### 32.3 Coexistence with Avathon

Barrick has an AI operating-layer partner (Avathon, E024). Three postures were tested (B4 SQ7):
- *Replace Avathon* is refuted: FrankenMining does not compete with AI operating platforms (NONGOAL-005).
- *Feed Avathon blindly* is refuted, because blind feeds would make Avathon a second authority without anchors.
- **Verdict C: FrankenMining is the anchor-pinned system of record, and Avathon is a cognition-plane consumer.**

**The five-point contract.**
1. Avathon reads per-venture **export projections** (`FMT-EXPORT-1`), with an anchor and certificate on every row.
   Every Avathon answer can therefore cite a FrankenMining anchor.
2. Avathon writes back only as *evidence submissions*, graded `AgentUpload` or elicited, `REVIEW_REQUIRED`, into
   `CELL-QUAR`. It never writes authority cells.
3. MNPI and venture walls apply unchanged.
4. Avathon has no effect path.
5. When Avathon and FrankenMining disagree, the knowledge state becomes `conflicted` (E033). It is surfaced, not
   merged.

Under this contract an AI platform's answers become *more* defensible by citing FrankenMining, and Avathon's
forecasts can be registered and scored like anyone's (§30.4). Whether Avathon accepts anchor-pinned feeds and
evidence-only write-back is OPEN-B4-08.

### 32.4 Proof obligations, gates and open questions

| PO / Gate | Statement | Verification / threshold |
|---|---|---|
| PO-PORT-1 | Every decoder survives 24 h of fuzzing with no panic, no unbounded allocation and no budget overrun | Fuzz targets per decoder |
| PO-PORT-2 | Re-exporting an imported file and re-importing it yields identical records (for formats with writers) | Round-trip property tests |
| PO-PORT-3 | Every imported column has a declared unit, basis, support and CRS, or is `UNREGISTERED` | Import report check |
| GATE-PORT-01 | Oracle agreement on fixture corpora: LAS/E57 point counts and bounds exact; GeoTIFF values exact; DXF entities exact; CRS within §25.5 tolerances | M1 (LAS, DXF, CSV dialects), M2 (E57, GeoTIFF) |
| GATE-PORT-02 | One full acQuire/Datamine export set from the pilot site imported with ≤ 0.1% rows rejected, every rejection explained | M2 |

- **OPEN-B4-06.** omf-rust's actual dependency closure.
- **OPEN-B4-08.** Will Avathon consume anchor-pinned feeds and accept evidence-only write-back?
- **OPEN-PORT-1.** Which pilot-site systems export what, and how often. This is a data-acquisition gate input
  (§34.4).

# PART V — VERIFICATION, GATES, RISKS AND OPEN QUESTIONS

## 33. Testpit: verification

### 33.1 Four claims, never conflated

A result may be described only by the claim its evidence supports. Each claim type has its own evidence, gate
family and permitted wording.

| Claim | What it proves | What it does **not** prove | Evidence | Permitted wording |
|---|---|---|---|---|
| **Parity** | FrankenMining reproduces a reference tool or a published number from the same inputs | That either is right | Pinned oracle fixtures (§33.2); public filings | "reproduces X to tolerance t" |
| **Known answers** | FrankenMining recovers seeded truth on synthetic data, with calibrated intervals | That real deposits behave like the generator | `fmine-gen` replicates, SBC, coverage (§33.3) | "recovers injected biases with c% coverage over n replicates" |
| **Reconciliation** | Forecasts agree with mined and processed reality, scored after the fact | That the next period will | The calibration ledger (§10); hindcasts; reconciled outcomes (§20) | "80% intervals covered k of n outcomes" |
| **Performance** | Speed and scale targets are met on named hardware | Correctness | Benchmarks with hardware, data size and seed recorded | "p99 = x on host H at size N" |

- A gate names exactly one claim type.
- A demo, a slide or a report that mixes them, for example by citing a performance number as evidence of
  correctness, fails review.

### 33.2 Oracles: pinned, separate, never regenerated from the candidate

- **The rule.** An oracle fixture is produced by a pinned external tool or a published source, in a separate lab
  runner. It is content-addressed and never regenerated from FrankenMining output. Regenerating a fixture needs a
  bead, a reason and a reviewer who is not the author of the code under test.
- No oracle appears in the runtime or CI dependency graph of production crates (GATE-ORACLE-01).

| Domain | Oracle(s) | Used for |
|---|---|---|
| Pit limits and scheduling | MineFlow, MineLib instances (UPIT, CPIT, PCPSP); fsci `maximum_flow`; fnx `preflow_push` | GATE-PUSH-PIT1, KILL-PUSH-* |
| LP, MILP, QP | HiGHS (lab lane only); fsci `linprog`/`milp` | GATE-PUSH-BLEND, KILL-PUSH-PDLP |
| Geostatistics | GSLIB-style reference implementations; fs-uq `KlExpansion`, fs-bo `Gp`; fsym-derived gamma-bar integrals | GATE-STRATA-G1 |
| Exploration | USGS three-part assessments and MapMark4 | GATE-LODE-P2 |
| Reconciliation and metallurgical accounting | AMIRA P754 worked examples; published two-product formula cases; fsym-derived balances | GATE-TOUCH-01, GATE-CRUC-01 |
| Metrics | World Gold Council AISC/AIC guidance examples; issuer filings | GATE-BULL-01/02 |
| Geotechnics | Published slope-stability benchmark problems | GATE-HIGH-01 |
| Formats and CRS | PROJ, GDAL, PDAL/LAStools, libE57Format, ezdxf | GATE-PORT-01, PO-CRS-1 |
| Numerics | fnp-linalg, fsci, fsym exact rationals | Kernel known-answer tests |
| Accounting | FA conformance workspace (`fa::Book` embedded, isolated) | INT-FA-001 |

**Fixture capture record.** `{oracle, version, platform, inputs digest, outputs digest, command, captured_by,
date, licence}`, stored next to the fixture in `fixtures/` and counted against the fixture-bytes budget (§28.4).

### 33.3 Synthetic truth (`fmine-gen`)

The generator builds worlds whose truth is known and **sealed**: the engine under test never sees it, and only the
scoring harness opens the seal.

| World | Seeded truth | Exercises |
|---|---|---|
| Open-pit Au mine | Geology, grades, density; biases in resource model, dilution, survey, sampler, density, moisture | Strata, Touchstone, Bullion, Pushback |
| Open-pit Cu–Au mine with flotation | Plus recovery-geomet transfer and concentrate custody | Crucible, Touchstone, metrics with by-products |
| Underground stoping Au mine | Stope geometry, overbreak, ventilation demand | Headframe, Highwall, underground scheduling |
| Heap leach | Leach kinetics and a latent inventory | GB-20 style inventory posteriors |
| Synthetic district | A thinned marked point process with known deposits, effort and detectability | Lodestar SBC and coverage (GATE-LODE-P1) |
| Synthetic company | Several sites, ventures, walls and an FA book | Bedrock sync, walls, the FA bridge, group metrics |

**Control and misspecification conditions.**
- **Correctly specified control.** Monitors must raise false alarms at ≤ α, with sharpness loss < 10%
  (GATE-PLUMB-E5, KILL-PLUMB-6).
- **Misspecified variants:**
  - wrong variogram;
  - wrong domain boundary;
  - an out-of-model fault;
  - non-stationary grade;
  - a biased capex process.

  The layered epistemic stack must restore coverage after an alarm (GATE-PLUMB-E3).
- **Identifiability probes.** Pairs of biases that cannot be separated must produce FMINE-REF-104, not a number
  (GATE-TOUCH-02).

### 33.4 Property, metamorphic and replay tests

| Family | Examples |
|---|---|
| Property | No-laundering (`compose` never promotes); conservation at commit; taint monotonicity; exact unit round-trips |
| Metamorphic | Splitting or merging stockpiles and lots leaves balances unchanged; permuting input order leaves digests unchanged; changing units or CRS and back is the identity; adding a zero-length interval changes nothing |
| As-of replay | A query at a past anchor returns bit-identical answers before and after later data arrives (GATE-PLUMB-E2) |
| Hindcast | Rolling-origin evaluation with as-known-at inputs only; no hindsight (§22.6) |
| Lab | LabRuntime schedules, chaos and crash points for every protocol (§26.7) |
| Fuzz | One target per decoder; the envelope; the verb parser (§32.4) |

### 33.5 End-to-end scripts and logging

- Every milestone acceptance criterion has an e2e script under `tests/e2e/` that runs against real binaries
  (`fmine`, `fmined`) on generated worlds. There are no mocks of FrankenMining components.
- Scripts log structured NDJSON: step, command, anchor before and after, exit code, refusals, certificate digests
  and timings. A failing run leaves a `CrashPack` that reproduces it.
- A human-readable summary is rendered from the log. The log, not the summary, is the evidence.

### 33.6 The offline verifier (`fmine-verify`)

An auditor, partner or regulator can verify, offline and without FrankenMining's services:
- certificate chains (inputs closure, code revision, seed, determinism class, output digest);
- MMR consistency proofs for cell ranges;
- CAS object Merkle roots;
- Merkle-folded posting detail against FA rows;
- that a BitwisePortable result regenerates bit-identically from its snapshot.

It is std-only, small (budget 12k) and has its own conformance fixtures.

| Gate | Threshold | When |
|---|---|---|
| GATE-ORACLE-01 | No oracle crate or tool in any production posture closure; every fixture has a capture record | From commit 1 |
| GATE-GEN-01 | The sealed-truth harness: a test that reads truth outside the scoring harness fails CI | M1 |
| GATE-E2E-01 | Every milestone acceptance criterion has a passing e2e script with structured logs | Each milestone |

## 34. The gate register

### 34.1 Gate semantics

- A gate **passes** only with a recorded measurement against its threshold: the measured value, the anchor, the
  certificate and the date (§0.2). Targets are not results.
- A **✖ kill gate** names its fallback in advance. When it fires, the fallback ships and the plan says so. A kill
  gate is never quietly re-scoped.
- Gate records live in the decision ledger (`CELL-DEC`) as typed entries, so the program's own decisions are
  auditable in the same way as the mine's.

### 34.2 Admission gates (foundations)

| Gate | Foundation | Pass criteria | Section |
|---|---|---|---|
| INT-AS-001 | asupersync 0.5.0 | The nine admission criteria below (from FSS-DD §2.9); one asupersync package id per posture | §27.5 |
| INT-AS-SEC-001 | asupersync 0.5.0 | Mitigation tests for defects S3(a)–(d) | §31.4 |
| INT-ATP-001 | ATP | QUIC/TLS only; chunk-receipt resume without the 0.6.0 journal; lab partition matrix green | §26.7, §31.4 |
| INT-FA-001 | franken_accounting | FA-ACC-01…13 green at the pinned FA revision | §21.3 |
| INT-FSQL-001 | frankensqlite | The ten admission criteria below (from FSS-DD §3.10), in the projection role; R-tree bbox p99 ≤ 50 ms over 10⁷ intervals | §27.4 |
| INT-FGDB-001 | frankengraphdb | Projection only; certified replay; measured benefit on 3 named queries (else KILL-2) | §27.4 |
| INT-FSEARCH-001 | frankensearch | quill + hash/model2vec only; no ort/fastembed; wall-scoped indexes | §27.4 |
| INT-FNX-001 | franken_networkx | Scale benchmark on synthetic precedence graphs | §27.4 |
| INT-FMCP-001 | fastmcp | Parity gate; confined to `fmine-mcp` | §30.3 |
| INT-FTUI-001 | frankentui | asupersync lane; inline mode; no write verbs outside the registry | §30.3 |
| INT-FMD-001 | franken_markdown | Library rendering; deterministic by digest | §30.5 |
| INT-FMERM-001 | frankenmermaid | Default features; no HiGHS or C toolchain in the closure | §30.5 |
| INT-FSIM-001 | frankensim | Separate-clone spike; one-package-id result recorded (expected FAIL today) | §27.1 |
| INT-NUM-001 | frankenscipy, frankenpandas (not `fp-io`), franken_numpy (`fnp-io`), frankenjax (`fj-core`, `fj-lax`, `fj-ad`) | Closures pass L-5 and the one-package-id rule; default features off; oracle parity on every consumed kernel; cross-ISA determinism scope recorded | §27.3 |

**What makes an admission record valid (R7-C017, R7-C083, R7-C101, R7-C153).** An INT-* record is valid only if it
cites a
commit in which `Cargo.lock` is tracked and contains the foundation, with the cargo-deny and one-package-id output
for that lock committed beside it (CI run ids alone expire after the retention period). A record written before
the foundation is in the graph is a pre-admission desk check and says `NOT_APPLICABLE` or `PENDING`, never PASS.
INT-AS-001 is split: **INT-AS-001a** (closure checks: one package id, criterion 9, `[patch.crates-io]` single
`Cx`) stays in WP-00 and reads `NOT_APPLICABLE` while asupersync is absent; **INT-AS-001b** (criteria 1–8) runs
in WP-05 at M1 and depends on the residency planner (BD-107), a wall-test bead and a named criterion-8 bead
(BD-337). INT-NUM-001 likewise records the FD-row fallback (own kernels in `fmine-numerics`) until a foundation
crate enters the closure.

**INT-AS-001 criteria (inlined from FSS-DD §2.9).** Integration is admitted only when:
1. the same end-to-end scenario passes under production and lab time;
2. no owned work survives region close;
3. every non-trivial drain emits a progress certificate;
4. every obligation reaches a terminal or an explicit indeterminate state;
5. capability narrowing is tested statically and dynamically (here: `check-caps` plus the runtime-mask lab test,
   §31.4);
6. routing cannot cross unauthorized site, venture or MNPI scopes;
7. ATP corruption, truncation, reordering, path-failure, resume and repair campaigns preserve root integrity;
8. scheduler or diagnostic adaptation cannot alter authoritative results;
9. no second runtime, detached worker or ambient timer appears in the dependency closure.

**INT-FSQL-001 criteria (inlined from FSS-DD §3.10).** fsqlite is a *projection* here (single writer, rebuildable),
so criteria 2, 6 and 9 apply only if `BEGIN CONCURRENT` is used, which FrankenMining does not plan:
1. a single-threaded reference implementation of each projection;
2. schedule exploration over concurrent reads and projection writes;
3. crash injection at every reserve/materialize/publish boundary;
4. positive and negative-read phantom tests;
5. deterministic semantic-rebase equivalence;
6. commit-combiner sequence and active-registration proofs;
7. stale-anchor refusal and historical `AS OF` replay;
8. recovery that distinguishes published, staged, aborted and indeterminate states;
9. witness-refinement exhaustion creates false conflicts only;
10. no direct use of lower-level APIs that bypass the connection and witness pipeline.

### 34.3 Convergence and kill gates by subsystem

| Subsystem | Convergence gates | Kill gates (✖, fallback in the section) | Section | Earliest |
|---|---|---|---|---|
| Plumb | GATE-PLUMB-E1…E5 | KILL-PLUMB-5 (executive card test), KILL-PLUMB-6 (false alarms) | §9, §10 | M1 |
| Lodestar | GATE-LODE-P1…P6 | KILL-LODE-1…4 | §11 | M4 |
| Corebox | GATE-CORE-01, -02 | — | §12 | M2 |
| Strata | GATE-STRATA-G0…G5 | G0, G3, G5 carry kill fallbacks | §13 | M3 + n |
| Highwall | GATE-HIGH-01…03 | KILL-HIGH-1, -2 | §14 | M5 |
| Pushback | GATE-PUSH-PIT1, -TOPO, -BLEND, -COMP, -STO | KILL-PUSH-PIT, -BZ, -MILP, -PDLP, -CAL | §15 | Starts M4 (month 18); PIT1 by month 20; acceptance M6 |
| Headframe | GATE-HEAD-01…03 | KILL-HEAD-1 (no FMS export by month 9) | §16 | M5 |
| Crucible | GATE-CRUC-01…03 | KILL-CRUC-1 (no plant data by month 9) | §17 | M2 |
| Freeboard | GATE-FREE-01…03 | KILL-FREE-1 | §18 | M5 |
| Muster | GATE-MUST-01…03 | — | §19 | M5 |
| Touchstone | GATE-TOUCH-01…06 | GATE-TOUCH-02 (bias recovery), GATE-TOUCH-04 (material finding) | §20 | M1–M2 |
| Bullion | GATE-BULL-01…03; GATE-POST-01 (3 sites × 3 months of synthetic close through `fad`: 0 duplicates, every month `fa replay`-identical, every Workpaper ≤ 128 rows) | GATE-BULL-01 (public reproduction), KILL-1 (FA scale) | §21 | M0 (M3 for GATE-POST-01) |
| Helm | GATE-HELM-01, -02; GATE-CAP-04, -05 | — | §22, §23 | M1 |
| Hallmark | GATE-HALL-01…03 | GATE-HALL-02 (independent QP replay) | §24 | M0–M1 |
| Units, time, CRS | PO-UNIT/CRS/TIME (§25.6) | — | §25 | M0 |
| Bedrock | GATE-STORE-01, GATE-SYNC-01/02, GATE-INGEST-01, GATE-BM-01 | KILL-4 (ledger persistence), KILL-5 (telemetry rate) | §26 | M1 |
| Suite | GATE-FSIM-DRIFT, GATE-FSIM-RECONV, GATE-PIN-06 | KILL-2 (fgdb), KILL-3 (frankensim) | §27, §31.4 | M1 |
| Workspace | GATE-LAYERS-00, GATE-BUDGET (§28.4) | — | §28 | Commit 1 |
| Numerics | GATE-DET-01 | — | §29 | M1 |
| Adit | GATE-ROBOT-01, GATE-REPORT-01 | — | §30 | M1, M3 |
| Security | GATE-WALL-01, GATE-SEC-01 | — | §31 | M1, before M2 |
| Portal | GATE-PORT-01, -02 | KILL-6 (OMF purity) | §32 | M1–M2 |
| Testpit | GATE-ORACLE-01, GATE-GEN-01, GATE-E2E-01 | — | §33 | Commit 1–M1 |

### 34.4 Data-acquisition gates

The best model without data access is a demo (§0.7). These gates measure access itself, and each has a fallback
that keeps delivering value without data (B5 M2′).

| Gate | Threshold | Deadline | Fallback if missed |
|---|---|---|---|
| GATE-DATA-01 Agreement | Signed data agreement, named data steward and site sponsor, insider list and blackout policy in force; the issuer's public lane frozen and its state change recorded in BD-200 (§31.1; R7-C137); README's "not affiliated" wording updated | Month 4 | Escalate once through the executive sponsor, then M2′ planning |
| GATE-DATA-02 Security | IT security review passed for the on-prem binary (SBOM, threat model, no egress) | Month 5 (GATE-SEC-01) | Offer the review pack to a delivery partner (F15) |
| GATE-DATA-03 First snapshot | Grade-led threshold: 8–12 historical quarters of reconciliation inputs (block and GC models, dispatch tonnages, surveys, mill sampler, metallurgical balance, cost-centre extracts) imported with explained rejections (GATE-PORT-02). Recovery-led threshold (Lumwana): the §37.3 recovery set imported with explained rejections; either threshold meets the gate (R7-C084, R7-C102) | **Month 7** | **M2′:** parent-level metric and reserve assurance on public data, the Nevada exploration hindcast and PQ-7 deal cards |
| GATE-DATA-04 Operations data | FMS cycle export (KILL-HEAD-1) and plant historian/LIMS export (KILL-CRUC-1) | Month 9 of a site engagement | Headframe and Crucible limited to survey epochs, monthly reports and certified activity drivers |
| GATE-DATA-05 Personal data | Privacy impact assessment and works-council review completed before person-level ingest (GATE-MUST-03) | Before Muster at any site | Aggregate-only workforce analytics |
| GATE-DATA-06 Exploration effort | Effort data (drilling, geophysics, geochemistry coverage) with licence for one public district (OPEN-LODE-1) | Month 15 (M4) | Tract-mode exploration with Manski bounds (KILL-LODE-2) |

**The probability that matters most.** P(site data by month 7) was elicited at 0.5 (B5 §5.3). The research pass
replaces it with an evidence-based **0.3 for Lumwana (plausible range 0.15–0.45)** (INFERENCE, E599):
- P(executive and Lumwana site sponsor committed, data list accepted, by month 4) ≈ 0.6 (0.45–0.75);
- × P(agreement, insider list and blackout in force by month 4–5 | sponsor) ≈ 0.8 (0.65–0.9);
- × P(security review passed and 8–12 quarters imported by month 7 | both) ≈ 0.6 (0.4–0.75).

The fastest Barrick precedent, Avathon, went from first collaboration (December 2025) to software at selected U.S.
sites in Q2 2026, inside the North American perimeter (FACT[secondary], E591). It is a benchmark for pace, not a
forecast for FrankenMining. The alternates are lower: North Mara/Bulyanhulu about 0.15–0.2, Kibali about
0.1–0.15, NGM and Pueblo Viejo below 0.1.

The wedge choice is unchanged: it is the only option that is useful at p = 0 and decisive at p = 1, and at p = 0.3
H-f's EV is 4.7 against 2.3 for the next options (GB-38, §37.2). What changes is planning weight: **M2′ is the modal
path (≈ 0.7 with Lumwana alone; ≈ 0.5 if the alternates are pursued in parallel and independently, which they are
not, because they share the executive sponsor and Barrick's corporate security review; R7-C126) and is resourced from
M1**, not held in reserve. p moves only on observable signals (W4 §6.5): a named
Lumwana site sponsor by month 2 (×2 odds), a security questionnaire issued within 30 days (×1.5), or one sample
month of the reconciliation pack by month 4 (×3) raise it; no site sponsor by month 3 (×0.5) lowers it; no draft
agreement at GATE-DATA-01 sets p ≤ 0.1 and triggers M2′.

### 34.5 The program gate (month 12)

If neither M2 nor M2′ yields an **executive-recognised use**, the program is re-scoped before any M3 spending.
- An executive-recognised use is a FrankenMining number or card cited in a guidance, capital or reserve meeting
  minute, or the sponsor's written equivalent.
- This is the "stop when the bottleneck has shifted" rule applied to the program itself.

## 35. Risks and the pre-mortem

### 35.1 Pre-mortem: "It is 2029 and FrankenMining failed at Barrick. Why?"

Source: B5 §7. The stories are ordered roughly by likelihood × impact (HYPOTHESIS). Each gives a leading indicator
visible by about month 6 and a countermeasure already designed into this plan.

| # | Failure story | Leading indicator | Countermeasure in this plan | Residual |
|---|---|---|---|---|
| F1 | **An incumbent AI platform absorbed the space.** FrankenMining was seen as overlapping with the AI operating layer already in place and was not taken forward. | The evaluation is folded into the existing AI-platform programme; "how is this different from the AI platform?" | Position as the **evidence substrate agents cite** (§32.3); pilot outside NA; offer no computer-vision, maintenance or supply-chain agents (NONGOAL-005) | **High** |
| F2 | **Data never arrived.** Security review of an unknown pure-Rust vendor took 9 months. | No signed agreement or data steward by month 4 | Security pack at M0; on-prem single binary; public and synthetic lanes deliver with no data (§34.4) | Medium |
| F3 | **MNPI breach or lane contamination.** A forecast leaked, or someone traded in a blackout. | No insider list; lanes share a machine | Two physically separate lanes; private-access people never author public forecasts; insider list and blackout for the whole team (§31.1) | Low probability, **fatal impact** |
| F4 | **QPs refused, and legal agreed.** Probabilistic metrics read as automation of judgment and as discoverable contradictions. | QP declines attestations; legal asks to remove intervals | Never classify; the CP act makes the QP the author; `no_claim` lines; co-design with the Head of MRM from M1; counsel designs retention and privilege (§24) | Medium |
| F5 | **The site ignored it.** A senior geologist's spreadsheet stayed the recon of record. | After 3 shadow months, the old pack still rules | **Parity first**: reproduce the site's own pack exactly; the site geologist is the named author; reduced pack time is the benefit; embed a person on site (§20) | Medium |
| F6 | **Yet another dashboard.** Executives said "interesting" and changed nothing. | No executive reuses a number in a meeting | The unit of delivery is a DecisionCard plus a scored forecast; the program gate (§34.5) | Medium |
| F7 | **Overclaimed probabilities.** A "90%" interval missed 4 of 8 quarters in front of the CFO (P = 0.005 if it were calibrated: strong evidence of miscalibration, R7-C118). | Coverage below nominal; intervals narrower than reference classes | Default `uncalibrated`; track record beside every forecast; UU budgets; refuse decision-grade probabilities without calibration (§9, §10) | Medium |
| F8 | **The value was not there.** The pilot site already reconciled well. | Attribution matches the site's commentary by month 9 | Pre-select a site with recorded variance; report the null honestly; allow one second site (§37) | Medium |
| F9 | **The suite never converged.** 30–40% of time went to sibling fixes. | > 20% of hours on sibling repos | Dependency firewall; lift with provenance; FA through the CLI; no deliverable depends on a sibling (§27) | Low–medium |
| F10 | **Purity cost the year.** Rebuilding parsers, CRS and samplers ate M0–M1. | M0 slips > 4 weeks; infrastructure > 40% of LOC | A wedge with minimal missing primitives; foreign tools as oracles; budget protocol (§28.4) | Medium |
| F11 | **Key-person risk on both sides.** One relationship, one architect; changes of role at the client. | One person answers every question; the sponsor moves | Named counterparts in several roles (CFO, Head of MRM, a site GM); plan-first documentation; DecisionCards make reasoning transferable | **High** for a small team |
| F12 | **The IPO took priority.** The NA carve-out drew on the same finance, IT and MRM teams. | Repeated deferrals; perimeter changes | Stay off the IPO critical path; pilot in the remaining-parent perimeter | Medium |
| F13 | **JV partner conflict.** Operating data became dispute evidence (cf. the NGM 2026 default notice, E034). | Legal asks about partner consent | Pilot at a 100%-owned site; partner outputs only through governed channels (§31.2) | Low once the site is chosen |
| F14 | **OEM lock-in.** Exports dropped fields or became paid services. | Exports need vendor professional services | OMF v2 from day one; snapshot importers; flat-file fallbacks; never require write or real-time APIs (§32) | Medium (real-time); low (monthly) |
| F15 | **Procurement and vendor viability.** A tiny shop failed onboarding. | Requests for references and SOC 2 | A delivery or assurance partner as the contracting wrapper; source escrow; a support runbook | Medium |
| F16 | **Scale broke trust.** 10⁸–10⁹ blocks and 100–1,000 realizations overwhelmed the stack at M5–M6. | The pilot's mined-volume subset takes hours to load | The wedge needs only mined SMUs per period (10⁵–10⁶); regenerate, don't store; benchmarks before claims (§13.5, §26) | Deferred, not removed |

### 35.2 Risk register

| ID | Risk | Severity | Mitigation | Owner |
|---|---|---|---|---|
| RISK-01 | Overlap with an incumbent AI platform (F1) | High | §32.3 contract; pilot outside NA | Owner + sponsor |
| RISK-02 | Data access (F2) | High | §34.4 gates; M2′ | Sponsor |
| RISK-03 | MNPI breach (F3) | Fatal impact | §31.1 | Counsel + owner |
| RISK-04 | QP and legal rejection (F4) | Medium | §24 doctrine; counsel review (OPEN-HALL-4) | Head of MRM |
| RISK-05 | Site non-adoption (F5) | Medium | Parity first; embedded engineer | Site GM |
| RISK-06 | Dashboard syndrome (F6) | Medium | DecisionCards; program gate | Owner |
| RISK-07 | Overclaimed probabilities (F7) | Medium | Calibration ledger; `uncalibrated` default | Head of MRM |
| RISK-08 | Checked null at the pilot (F8) | Medium | Site selection; second site | Sponsor |
| RISK-09 | Suite non-convergence (F9) | Low–medium | §27 firewall | Owner |
| RISK-10 | Purity cost (F10) | Medium | §28.4; oracles | Owner |
| RISK-11 | Key persons (F11) | High | Multi-threading; documentation | Owner |
| RISK-12 | IPO calendar (F12) | Medium | Off the critical path | Sponsor |
| RISK-13 | JV dispute exposure (F13) | Low | 100%-owned pilot | Counsel |
| RISK-14 | OEM export lock-in (F14) | Medium | Snapshot importers; data-ownership clauses | Sponsor |
| RISK-15 | Procurement (F15) | Medium | Delivery partner; escrow | Owner |
| RISK-16 | Scale (F16) | Deferred | Benchmarks; regeneration | Owner |
| RISK-17 | **Licence rider** (MIT + OpenAI/Anthropic) on lifted code, the substrate and FrankenMining's own `LICENSE` (E681) in a client-facing product. The rider reaches anyone acting on behalf of, for the benefit of or under the direction of OpenAI or Anthropic, and bars making the software available to or for them, so the decision also covers agents hosted by those parties reading through `fmine-mcp` or the CLI (§30.4) | OPEN | Owner and counsel decision before M2, and before the M2 agent slice opens | Owner + counsel |
| RISK-18 | asupersync 0.5.0 security and durability defects (B4 S3) | High | §31.4 mitigations; GATE-PIN-06 | Owner |
| RISK-19 | Human review is the FA posting bottleneck (GB-28) | Medium | Monthly trunk; delegated reviewer ask (OPEN-BULL-3) | Controller |
| RISK-20 | Name collisions (`fm`, `fmd`, `fm-*`) | Medium | `fmine-` prefix (OPEN-B4-09) | Owner |
| RISK-21 | Residency statutes misread: 39 of 72 researched rows are INFERENCE (W3) | High | Fail-closed default; each row carries status, confidence and `review_by`; counsel on the W3 §7 clauses before any regional node (OPEN-B4-01) | Counsel |
| RISK-22 | Goodhart: a metric used in pay can drift from the quantity it measures (AISC deferral, CCV completion) | Medium | KPI price list (§23.6); deferral detection (GB-26); CCV quality, not counts (§19) | Owner |
| RISK-23 | Code changes (JORC 2026, the NI 43-101 replacement) shift disclosure requirements mid-build | Medium | Renderers as registry rows (§24); OPEN-HALL-2 | Head of MRM |
| RISK-24 | Evidence state misread as a safety statement (TSF, slopes) | High impact | Four false flags; "evidence state, never safe" wording; no `geotechnical_signoff` (§18) | Tailings function |
| RISK-25 | LOC overrun repeats FTX (2.7×) | High (sibling velocity reference class, §28.3; R7-C115) | §28.4 protocol from commit 1 | Owner |
| RISK-26 | Host-state action at a demonstration site: suspension, provisional administration, retroactive code, tenure lapse (Loulo-Gounkoto 2025: suspended, loss of control, no Q1–Q3 2025 data, then settlement and retroactive 2023-Code royalties for 2024–2025; E248, E412; Kibali permits 2029/2030 vs LOM 2043, with renewal under Art. 80 requiring +5% State equity; E245, E444) | High | Site-quarter state `NOT_PUBLISHED`; `CountryRegime` as-of dating (§23.4); G-HALL-6 tenure flags; GATE-BULL-01 excludes NOT_PUBLISHED by name; no silent fill | Owner + counsel |
| RISK-27 | Water-use conflict in a hyper-arid, state-equity project: community and institutional groundwater concerns; the ESIA's monitoring table records water use and source (E330–E333) | High | G-FREE-7/9; groundwater DATA-GATE (§18.4); evidence state, never "no impact" | Head of Environment |
| RISK-28 | Security provided by state forces (Frontier Corps, Levies) at a state-equity project (E342) | Medium | VPSHR commitment set as registry data; 24 h human-rights grievance escalation tracked as `OB-COMMIT` | Counsel + Sustainability |
| RISK-29 | Unapproved revisions ("Rev A … NOT approved"; E304) ingested as evidence | Medium | `DocApproval` on every `DocLocator`; draft-sourced cells cap at `conflicted`/`stale` | Owner |
| RISK-30 | Demo text misstates a site fact taken from a superseded filing (e.g. Goldstrike's thiosulphate circuit, converted to CIL in 2023; E174) | Medium | Every demo claim cites a dated source; sources older than the latest filing are flagged | Owner |
| RISK-31 | Issuer data terms: Barrick's site terms allow copying only for non-commercial, personal or educational use of unmodified material; committed derived workbooks or values may need approval (R7-C052, R7-C158, R7-C160) | Medium | Locators, URLs, sha256 and computed results only until decided; synthetic or fetch-at-test fixtures; seek IR's written approval | Owner + counsel, before the next code push |
| RISK-32 | A synthetic, stand-in or mislabelled figure reaches an issuer reader as the issuer's own (the unpushed M0 slice did this; Appendix K.2) | High impact | Registered inputs only, typed refusal on any missing file, `synthetic` printed on fixture output, filed-page check by a second person, and an executable uncited-number check with a seeded negative control (§21.6) | Owner |
| RISK-33 | Process records overstate: beads closed with a generic reason, integration records marked PASS for crates not in the build (R7-C013–R7-C018) | High | Close reasons name the bead's own test, gate record or decision entry; INT records cite a tracked Cargo.lock containing the foundation and its cargo-deny output (§34.2); an xtask check rejects generic reasons | Owner |

## 36. Open questions and the decision rule

### 36.1 The decision rule

Every OPEN item has:
- an **owner** who can decide it;
- a **deadline**, expressed as a milestone or gate it blocks;
- a **fail-closed default** that applies until it is decided;
- the **evidence** that would decide it.

An OPEN item past its deadline is escalated to the program review. Its default stays in force, and the plan
records that the default, not a decision, is governing. Nothing is decided by silence.

### 36.2 Owner decisions needed first

| OPEN | Question | Owner | Deadline | Default until decided |
|---|---|---|---|---|
| OPEN-B4-09 | Crate and binary naming: closed 2026-10-29, `fmine` (no crates.io collision, E651; `fm` collisions stay in RISK-20) | Owner | Commit 1 | `fmine` |
| RISK-17 / OPEN-STRATA-6 | Is the MIT + OpenAI/Anthropic rider acceptable for lifted code in a client-facing product? | Owner + counsel | Before M2 | Lift only into internal builds; no client distribution |
| OPEN-B4-01 | Residency statutes: narrowed by W3 (72 rows, 12 countries); counsel confirms the eight W3 §7 clauses | Counsel | Before any regional node | W3 rows as registered; `in_country_only` for every unknown pair |
| OPEN-B4-02 | asupersync 0.6 bump: unreleased (crates.io max 0.5.0; E653–E655); coordinated by the suite maintainer once 0.6.x publishes and fgdb, FA and fastmcp move | Suite maintainer | GATE-PIN-06 | Stay on `=0.5.0` with §31.4 mitigations |
| B5 p | P(site data by month 7) | Sponsor | Month 4 | 0.3 for Lumwana (0.15–0.45; W4, E599); M2′ is the modal path, resourced from M1 |
| OPEN-PUSH-1 | Does `UntrustedCandidate` import of external plans respect the closed-dependency rule? | Owner | Pushback start (M4) | External plans enter as data files only, never as linked code |
| OPEN-B4-07 | MNPI insider-list format per listing venue | Counsel | Before M2 | The strictest known format |
| OPEN-HALL-4 | Does a software-prepared data-verification index change QP liability? | Counsel | Before GATE-DATA-01 (month 4; R7-C138) | Index labelled as support material only |
| OPEN-R7-1 | Issuer data terms: may derived Barrick workbook values be committed to the public repository? (RISK-31) The 2026-10-29 push (`748da79`) carried MOR_CASE rows that quote cell values (E820–E823). Files citing workbook cells, to read for quoted values in one pass: W1, C1, C3, C4, W4, MOR_CASE, AUDIT_4, the R7 MOR run, `repro/barrick/README.md`, `scripts/oracle/share_census.py`, `forecasts/fy2026/rules.toml` (locators only) and fmine-public's sources and tests. Undecided; the default binds new commits | Owner + counsel | Before the next push that adds derived values | Locators, URLs, sha256 and computed results only |
| OPEN-R7-2 | Who are the two M0 reviewers, what is their relation to Barrick, and are they available by January 2027? (R7-C045) | Owner | M0 week 4 | One independent ex-major reviewer; any issuer reviewer sees forecasts only after release |
| OPEN-R7-3 | Is the owner a public-lane author during the FY2026 forecast window, given §31.1's private-access rule? (R7-C161) | Owner | Before the forecast hash | The owner authors only if they hold no private access; otherwise a walled author set |
| OPEN-R7-4 | How should the public plan treat statements about Barrick's people, board and partners (§32.3, §34.4, §35 F1)? (R7-C030, R7-C032) **Resolved 2026-10-30:** the owner decided to keep factual, sourced statements, drop characterisations and anything the design does not need, and redact the same way going forward; the passages were revised on that basis | Owner | Done | — |
| OPEN-R7-5 | What happens to the unpushed M0 commit `68e7091` (amend, follow with a fix commit, or push as is)? (Appendix K.2) **Resolved 2026-10-29:** pushed at the owner's instruction and followed by correcting commits (`543df53` … `748da79`); public history is not rewritten. K.2 step 2 was not complete at the push and is tracked as BD-323 | Owner | Done | — |
| OPEN-B4-08 / OPEN-HEAD-2 | Will Avathon consume anchor-pinned feeds and write back evidence only? | Sponsor | M2 | Export projections offered; no write-back channel |
| OPEN-B4-10 | Reference site node hardware | Sponsor IT | M1 | Targets stay TARGETs |

### 36.3 Index of OPEN items by section

Section-level OPEN items satisfy §36.1 by inheritance unless §36.2 says otherwise: the **owner** is the
subsystem's owner role in §34.3 and §38; the **deadline** is the earliest gate of that subsystem that the item
blocks (shown in the "Earliest" column of §34.3); the **fail-closed default** is the posture in the table below;
the **deciding evidence** is stated with the item in its section.

| Section | OPEN items | Default posture |
|---|---|---|
| §9–§10 Plumb | OPEN-PLUMB-7 (reference class for reserve-to-production grade bias), -9 (estimation-uncertainty disclosures, IAS 1.125-style), -11 (politics of scoring experts) | Uncalibrated flags; scores visible only to the expert and case owner |
| §11 Lodestar | OPEN-LODE-1…6 | Tract mode and Manski bounds where effort data are missing |
| §13 Strata | OPEN-STRATA-1…9 | Declared-estimator authority; belief ledger advisory |
| §14 Highwall | OPEN-HIGH-1…3 | Engineer's existing tools stay authoritative; FrankenMining tracks evidence state and TARP obligations only |
| §15 Pushback | OPEN-PUSH-1…5 | Vendor plans as candidates; honest gaps |
| §16 Headframe | OPEN-HEAD-1…4 | Survey epochs and certified drivers only |
| §17 Crucible | OPEN-CRUC-1…4 | Monthly metallurgical report as an attested observation |
| §18 Freeboard | OPEN-FREE-1…4 | Document-only evidence state |
| §19 Muster | OPEN-MUST-1…3 | Aggregate-only analytics |
| §20 Touchstone | OPEN-TOUCH-1…4 | Parity-only deliverable |
| §21 Bullion | OPEN-BULL-1…4; OPEN-B4-03 (does `PostBatch` accept external-subledger members, FA-ACC-13), OPEN-B4-04 (= OPEN-BULL-2) | Monthly posting; local certified ledger |
| §23 Helm II | OPEN-HELM-1…6 | Reference-class widening; board sets τ |
| §24 Hallmark | OPEN-HALL-1…4 | QP authorship; no class labels from software |
| §26 Bedrock | OPEN-B4-01, -05, -07, -09, -10 | Fail-closed residency; one authority mechanism; strictest insider-list format; `fmine` naming |
| §27 Suite | OPEN-SUITE-3, -5 (OPEN-SUITE-1, -2 and -4 closed by the suite-fit audit, §27.7) | Local schema-compatible types; oracles not dependencies |
| §32 Portal | OPEN-B4-06, -08; OPEN-PORT-1 | Lab-oracle conversion; export projections only |

### 36.4 Planned refinement passes

- **B6 Brenner pass (done 2026-10-29; `planning/brenner/B6_operations_plant_tailings_workforce.md`).** Headframe
  (§16), Crucible (§17), Freeboard (§18) and Muster (§19) received a refuter-first pass over 17 hypotheses,
  including four forced third alternatives. It added discriminating experiments T6-1 and T6-2, guards G-CRUC-6,
  G-FREE-11 and G-MUST-6, and cards GB-40 to GB-43 (Appendix B). Its AHS premise, a live 40-truck autonomous
  fleet at Carlin, was corrected against E619 (Appendix H.5b).
- **Plan review rounds.** Round 1 (`AUDIT_1`–`AUDIT_3`) and round 2 (`AUDIT_4` corpus second read; `AUDIT_5`
  equations, guards and bead graph) are folded in (Appendix H). At least two further external rounds
  (planning-workflow) remain before beads are finalized at commit 1, each diffed against the previous round until
  changes are marginal.

# PART VI — DELIVERY

## 37. Milestones M0–M6

### 37.1 The wedge: Guidance & Reserve Assurance

Brenner pass B5 tested seven first wedges. The verdicts:

| Hypothesis | Verdict | Role |
|---|---|---|
| H-a Exploration prospectivity and capital allocation first | Deferred, not rejected | The second wedge, at M4, led by deal-structure capital allocation (PQ-7) and an effort-aware hindcast |
| H-b Reconciliation and metal accounting as the system of record first | Rejected as system of record; kept as assurance | System of record waits for FA convergence (M3 at the earliest) |
| H-c Full operating twin first | Rejected for 24 months | Becomes the M6 read-only "twin of record". Never a control system. |
| H-d Evidence spine plus a thin vertical slice | Mandatory but narrowed | The engineering spine under H-f, with the budget protocol from day one |
| H-e DecisionCards for the board | Adopted as the delivery format | Every executive output is a DecisionCard |
| **H-f Guidance & Reserve Assurance** | **Recommended wedge** | M0–M3: a read-only overlay delivered through H-e cards on a narrowed H-d spine |
| H-g TSF evidence state or CCV first | Deferred to M5 | After trust is earned; never "safe" or "unsafe" |

**Why H-f.**
- It is the only option that is useful with **no data**, through the public lane and synthetic truth, and decisive
  with **some data**, at one site (GB-38).
- It targets measurement integrity behind the metrics the company publishes and steers by: gold and copper AISC,
  GEO and reserve replacement (E025).
- **Honest wording:** software does not *move* AISC. It moves the accuracy, timeliness and attribution of AISC,
  which management then acts on.

### 37.2 Galaxy-brain card

**GB-38 · The wedge under data-access uncertainty.**
- **Equation.** EV(h) = p·V(h | S1) + (1 − p)·V(h | S2) − λ·T(h).
  - S1: site data by month 7. S2: none in year 1.
  - V is the executive-perceived decisive value at month 12, on a 0–10 scale.
  - T is the tail exposure to the worst loss: a false-certainty number reaching a disclosure, capital decision or
    dam.
- **Substituted** (elicited values from B5; λ = 1):

  | h | V(S1) | V(S2) | T | EV at p = 0 | EV at p = 0.5 | EV at p = 1 |
  |---|---:|---:|---:|---:|---:|---:|
  | **H-f** | 8 | 4 | 0.5 | **3.5** | **5.5** | **7.5** |
  | H-a | 4 | 3 | 1.0 | 2.0 | 2.5 | 3.0 |
  | H-e | 4 | 3 | 1.0 | 2.0 | 2.5 | 3.0 |
  | H-d | 3 | 2 | 0 | 2.0 | 2.5 | 3.0 |
  | H-b | 3 | 1 | 1.0 | 0.0 | 1.0 | 2.0 |
  | H-c | 2 | 0 | 2.0 | −2.0 | −1.0 | 0.0 |

  At the research-pass estimate p = 0.3 (§34.4): H-f 4.7; H-a, H-e and H-d 2.3; H-b 0.6; H-c −1.4.
- **Intuition.** H-f dominates at every p under these values. That makes the dominant uncertainty (data access)
  irrelevant to *which* wedge to start. It matters only for *when* M2 delivers.
- **Assumptions.**
  - Executives care about guidance credibility.
  - Filings contain per-site components (verify in M0 week 1).
  - A pre-registered scored forecast is persuasive.
- **Flip.**
  - At p = 0, H-f loses to H-a only if V_f(S2) < 2.5, i.e. the public lane impresses no one. Then weight shifts
    to H-a plus PQ-7 for the parent.
  - If the sponsor's stated priority is exploration or capital allocation, start M4 at month 3, in parallel.
  - If Barrick's existing reconciliation already attributes its variances, H-f loses its finding mechanism. Pivot
    to H-e plus PQ-7.

### 37.3 The ladder (about 36 months from November 2026)

Each milestone has an artifact, acceptance criteria (TARGETs), a kill gate with a fallback, and the published metrics
whose decision inputs it touches.

**Clocks (DESIGN).** M0 keeps absolute dates, because its deadlines are Barrick's release dates. Steps that need
Barrick's people, data or signature (the site decision rule, GATE-DATA-01 to -04 in §34.4, M2 to M6 and the program
gate) count months from the start of the engagement, so a later signature moves all of them by the same amount. On
a January 2027 start, GATE-DATA-01 (month 4) falls at the end of April 2027 and the program gate (month 12) at the
end of December 2027. Work that needs nothing from Barrick (engineering gates, M1's synthetic and public-lane items)
keeps month 0 = November 2026.

**M0 — Public Mirror plus Spine (months 0–3; Nov 2026–Jan 2027).**
- *Artifact:* `fmine assure public --issuer barrick`, emitting:
  - per-site AISC/TCC (C1 for copper)/CoS/grade/recovery/capex for every quarter whose Mine Stats workbook is in
    the source registry (today: the ten quarters **2024Q1–2026Q2**, their half-year and year-to-date blocks, and the
    2023 comparatives; E148, E451). Every cell cites `<file-period>:<Sheet>!<Cell>`
    + the workbook's sha256 and the filed Quarter Report page that reconciles it (E107). GEO is company-level only,
    on a named price basis (GB-27). **Q3 2026 is the only DATA-GATE**. It is released on 9 Nov 2026 at 06:00 ET,
    inside M0, so the importer meets a workbook it was not developed against (E454; W1 found layout drift in
    every vintage). That counts as a blind test only if a label-anchored importer exists and its commit and
    built-binary sha256 are pushed to the public repository before the release; if that is missed, the Q4 2026
    workbook (February 2027, inside M1) is the blind test and that clause of GATE-BULL-01 moves to M1 (R7-C025,
    R7-C041);
  - published cut-off reproduction for every cut-off a technical report states (seven reproduced in the corpus,
    including the $/t vs $/st trap; E182–E184);
  - the metric-definition registry v1 (WGC 2013/2018 AISC/AIC; by-product vs co-product; GEO price ratio; named
    reserve-replacement bases);
  - a discrepancy register, each item bridged with citations or marked OPEN with the document that would close
    it, and each issuer discrepancy carrying its exact certificate and shipped as an evidence-challenge packet
    (BD-340, BD-349);
  - the live-dependency proof and a reproduction kit for outside readers: source URLs, sha256 digests, label
    selectors and expected state roots, with no workbook bytes in the repository (BD-338, BD-339);
  - TARGET, not gating: the issuer's per-site guidance FY2023–FY2026 transcribed by two readers with page
    citations, and FY2026 guidance registered beside FrankenMining's own forecasts as an issuer forecast family.
    It uses released documents only, so it can be shown to the issuer reviewer before the FY2026 release, and it is
    the answer to "nothing new" that does not depend on FrankenMining's own forecasting skill (BD-341);
  - **the FY2026 forecast record** (§21.6): per-site forecasts with 80% intervals and the flag `uncalibrated`,
    with information cut-off, input manifest, basis rule and baselines, hashed and pushed with an external time
    proof by 31 Dec 2026 and before any M0 content reaches an issuer insider;
  - the evidence spine: Claim, ledger, refusal codes;
  - a security pack: SBOM, threat model, offline single binary.
- *Acceptance (GATE-BULL-01, three parts; §21.6, §21.8):* extraction fidelity of 100% of the named denominator
  against an independent reference read; identity accounting with every instance in one of five cited states;
  issuer consistency reported per identity family; the live-dependency proof on every scored value; byte-identical
  reruns with the input manifest in the state root; zero uncited
  numbers; every company-level figure matches its filed page and label (reserve replacement reads three-year
  2023–25 on both bases, about 190% gold ounces and 504% GEO, Circular p.41; FY2025 gold AISC reads $1,637/oz on
  3,318 koz sold; R7-C131); the forecast record time-proved before the release. The denominator is numeric
  site-quarter cells only, with placeholders and `NOT_PUBLISHED` site-quarters (Loulo-Gounkoto Q1–Q3 2025)
  excluded and listed by name. The gate record cites a run whose input manifest equals the source registry.
- *Reviewers and disclosure sequencing (R7-C045, R7-C133–R7-C135; CONDITIONAL on counsel).* The kill is judged by a
  named pool: one independent ex-major finance reviewer and at most one issuer finance reviewer. Before the FY2026
  release the issuer reviewer sees only the historical reproduction and the discrepancy register; the forecast
  set reaches issuer staff only after release, as scored output. Counsel reviews any external use of public-lane
  forecasts (B5 F3; default: share with the sponsor only, after release). Barrick files on Form 40-F as a foreign
  private issuer, so Regulation FD does not apply; the provincial tipping and insider-trading prohibitions
  (explained in National Policy 51-201) and U.S. antifraud rules do. No M0 content is presented to issuer insiders
  in Barrick's pre-release quiet period without counsel's clearance.
- *Kill:* blind-file extraction fidelity below 80%, or the two reviewers rate it "nothing new". **Fallback:** the
  public metric engine becomes test fixtures only, and weight shifts to M1 reconciliation and an early M4.
- *Crates:* WP-00 to WP-03 and WP-06 slices, plus BD-090 (WP-04, FMT-CAS-1) and BD-142 (WP-07, e2e harness),
  which the M0 deliverables' dependency closures contain (§38; R7-C086, R7-C104).
- *Dependency calendar (R7-C116).* Each human or external dependency is listed by role with a commit-by date and a
  private-register id (no personal names in this repository): the two M0 reviewers by M0 week 4; BD-075 complete
  by month 2, because the month-2 Lumwana decision (BD-199) waits on it (R7-C089, R7-C106); the executive or
  ex-executive
  panel (GATE-HELM-01, KILL-PLUMB-5) before M1 starts; the Lumwana sponsor by month 2; the penetration tester
  (GATE-SEC-01) by month 4; GATE-SYNC-01 and INT-ATP-001 by month 5, because GATE-DATA-02 depends on them (R7-C090);
  the QP or metallurgist for GATE-TOUCH-04 named at GATE-DATA-01. A missed date is logged as a gate event.

**M1 — Synthetic truth, reconciliation known answers, retrospective cards (months 3–6; Feb–Apr 2027).**
- *Artifact:*
  - the synthetic-truth generator (open-pit Au and Cu–Au) with seeded biases;
  - the reconciliation engine (§20);
  - calibration ledger v1;
  - scoring of the FY2026 public forecasts at release, **reported whatever they show**, against the two
    pre-registered baselines, with n, n_eff and the Clopper–Pearson interval (§21.6); the Q1 2027 vintage hashed
    before its quarter begins;
  - two retrospective DecisionCards built as known at decision time (GATE-HELM-01), each with a temporal-cone
    certificate over all its ancestors, including fitted parameters, exclusion lists and decks (BD-344);
  - from public data only (Appendix K.5): the guidance track record and the guidance completion test (BD-341,
    BD-342); forecast cohorts with outcome vintages (BD-343); disclosure lint in cited mode and the reliance check
    for issued outputs (BD-345, BD-347); the JV mirror for NGM, Pueblo Viejo and Kibali (BD-346).
- *Acceptance:*
  - each K.5 capability shipped at M1 passes its K.5 test, and any whose kill condition fired is recorded as
    killed;
  - GATE-TOUCH-01/02: over ≥ 200 replicates, 90% intervals on each injected bias cover truth 85–95% of the time;
    the metal balance closes within stated uncertainty; non-identifiable pairs return FMINE-REF-104;
  - GATE-STORE-01, GATE-SYNC-01, GATE-DET-01, GATE-ROBOT-01, GATE-WALL-01 pass.
- *Kill:* coverage outside 80–97% after one fix cycle, or identifiability not characterised. **Fallback:** no site
  proposal on reconciliation; lead with the metric engine and H-a/PQ-7.

**M2 — Site Assurance Pilot in shadow mode (engagement months 6–12; Jul–Dec 2027 on a January 2027 start). The
12-month demonstrator.**
- *Preconditions:* GATE-DATA-01/02 (data agreement, insider list and blackout policy, security review, a named
  site sponsor plus QP and metallurgist).
- *Site preference:* **Lumwana** (Zambia; 100% through Lumwana Mining Company, E551), Barrick's only wholly
  owned operating mine after the Hemlo and Tongon sales (E552–E554). It is outside the NGM/Newmont JV and outside
  the North American Barrick perimeter where Avathon is deploying (E563, E590, E591). The finding mechanism is
  **recovery-led** (W4 §5.2):
  - Headline recovery went from 89.4–89.8% (2023–24) to 91–92% (2025–H1 2026), mostly because head grade rose to
    0.57–0.67% Cu. Every 2025–26 quarter sits 1.7–2.7 points below the site's own 2013–22 grade–recovery
    relation: 2.4–3.4% of output (3.4–4.0% in 2023–24), roughly $30–57M/yr at $4.00–5.50/lb (INFERENCE, E566, E567).
    Metal balance reproduces the printed recovery within ±1.5 points, so the gap is not a print artefact (E127).
  - Barrick describes 2021–25 recovery (91.15%, fresh plus transitional ore) as reasonable and attributes changes
    to grade and the mine plan; no breakdown by mechanism is published (E569, E573). The pre-registered
    deliverable is therefore an **attribution** of the grade-normalised gap among measurement (head and tail
    sampling, assay), ore type (fresh vs transitional) and operations (throughput and grind, downtime, power), not
    "recovery is low". Grind (P80) is split from throughput only where sizing data exist (LIMS or historian,
    GATE-DATA-04); otherwise the two are reported as one cause.
  - The same recovery card is scored against the feasibility study's 92.7% fresh-ore recovery for the 52 Mtpa
    parallel plant, which uses a similar flowsheet. One point at 240 ktpa ≈ 2.6 kt Cu/yr (E570–E572).
  - The site reports its grade reconciliation as confirming the model (E130). A grade null is the expected result
    there and is reported as such.
  - Non-stationarity: the window holds a prolonged SAG shutdown (Q1 2025), crusher downtime (Q3 2025), a reline
    (Feb 2026), the pebble-crusher tie-in (Q2 2026) and grid-power instability (2024) (E580–E584); each is a
    segment boundary. Strip ratio rises 4.37 → 5.74 and new-plant commissioning runs from H2 2027, with first copper
    at end-Q1 2028 (E131, E132, E578). Historical quarters are 2024Q1–2027Q1; the two forward quarters are
    pre-registered for the **existing circuit only**, and new-plant tonnage is a separate circuit with `Estimated`
    cards. On a January 2027 start they are Q3 and Q4 2027, each registered before it begins (by 30 June and
    30 September 2027) and scored by January 2028, before first copper; Q3 needs GATE-DATA-03 met by month 6.
  - The recovery-capable GATE-DATA-03 set is the monthly metallurgical balance; head, tail and concentrate sampler
    and assay records; concentrate shipment and settlement assays; fresh/transitional tags on feed; and the monthly
    reconciliation pack. Historian and LIMS access (GATE-DATA-04) refines the finding but does not enable it.
  - **GATE-DATA-03 has two thresholds (R7-C084, R7-C102).** The *recovery-led* threshold is the set above; the
    *grade-led* threshold is §34.4's full set (block and GC models, dispatch tonnages, surveys, cost-centre
    extracts, one GATE-PORT-02 export). Lumwana delivering only the recovery-led set does not trigger M2′. On the
    recovery-led route BD-204 does not wait on BD-188, BD-189 or BD-194; a metallurgical-balance and sampler-record
    import bead is added (BD-336), WP-12 gains an edge to WP-10, and GATE-TOUCH-03 has a recovery-led variant
    (parity with the site's monthly metallurgical balance and recovery statement rather than F1/F2/F3).
  - **A material finding is routed to disclosure (R7-C138).** Lumwana is a material property in the 2025 AIF, and the
    92.7% feasibility recovery is an AIF statement. A finding that bears on an AIF statement is sent to Barrick's
    disclosure QP and disclosure committee the day it is attested, before any wider sharing; counsel's QP-liability
    item OPEN-HALL-4 moves from "before M3" to before GATE-DATA-01.
  - Residency: Zambia's DPA 2021 s.70 keeps personal data in Zambia (FACT, E514), which the on-prem artifact
    already satisfies; rosters and fatigue data never leave the site node, and only aggregates may (W3).
- *Ranked alternates* (used only if Lumwana has no named site sponsor by month 3):
  1. **North Mara with Bulyanhulu** under one Twiga data-governance paper. The Government of Tanzania's 16% is
     free-carried from 1 Jan 2020, it appoints Twiga board members, and economic benefits are split 50/50 through
     an annual true-up that excludes the $300M settlement (FACT, E135, E448, E562). Mechanism: plan vs actual on
     grade and ore delivery. Processed grade fell from 4.29 g/t (2024Q4) to 1.80 g/t (2026Q2) while AISC rose from
     $1,098 to $2,577/oz; Q1 2026 mined 7 kt of open-pit ore at a strip ratio of 514 during the Gokona pre-strip,
     and H1 2026 delivered 68 koz attributable against 200–230 koz guidance (FACT, E493–E495, E587, E588). Its
     recovery fall to 85% is grade-driven (E589), not a separate finding. Mining Act s.35(6) needs Geological
     Survey authorisation to export "any other data collected" (E527), so its node stays in Tanzania.
  2. **Kibali** (45%; Barrick operates; AngloGold Ashanti 45%, SOKIMO 10%; E560), if AngloGold consent is
     obtainable. It has the richest public reconciliation record in the corpus (C3 §2.3: the GC call is below
     model metal in every year 2016–25).
  - Not candidates: NGM and Pueblo Viejo (Newmont JV, the IPO vehicle and Avathon; E556–E558, E591); Fourmile
    (moving into NGM at fair market value, so any Fourmile work is a walled lane, §11.7; E139); Loulo-Gounkoto,
    Porgera, Reko Diq and the non-operated JVs (E559, E561).
- *Site decision rule.* Month 1: the executive sponsor is asked to name a Lumwana site sponsor, metallurgist and QP
  and to accept the data list. Month 2: if all are named, the pilot is Lumwana. Month 3: otherwise open North Mara
  (Twiga paper) and Kibali (AngloGold consent) in parallel and take the first to reach GATE-DATA-01, preferring
  Kibali if both do. Month 4: no signed agreement means M2′.
- *Artifact:* an on-prem single binary ingesting read-only snapshots for 8–12 historical quarters. Outputs:
  - **parity reproduction of the site's own monthly reconciliation pack**;
  - the reconciliation posterior and attribution;
  - a per-quarter AISC waterfall from tonnes, grade and recovery to unit cost, in $/lb for a copper site, with
    each bar classed controllable, price-linked or structural so the CFO's §3 question is answered by name (R7-C141,
    R7-C142);
  - a rolling-origin hindcast;
  - two forward quarters pre-registered **inside the tenancy**;
  - a Site Assurance DecisionCard that names, for each attributed cause, the cheapest trial or audit that would
    confirm it, with its expected effect registered before it runs; the metallurgist decides what to try;
  - a read-only agent interface on the node: the robot CLI and a read-only `fmine-mcp` slice over the pilot's
    Claims, with no `prepare_*` effects and the §30.4 rules (walls, AccessEvents, imported text as data). It opens
    only after RISK-17 is decided.
- *Acceptance:*
  1. Parity with the site's F1/F2/F3 from the same inputs, every difference explained (GATE-TOUCH-03).
  2. **At least one material finding** attested `Reviewed{against}` by the site QP or metallurgist as new and
     plausible. "Material" is pre-registered: ≥ 1% of contained metal, or ≥ a $M/yr threshold set with the
     sponsor (GATE-TOUCH-04).
  3. Hindcast coverage reported with a small-sample caveat (GATE-TOUCH-05).
  4. No data egress, verified by Barrick IT.
  5. Reconciliation pack time down ≥ 50%, or the site elects to co-sign. The baseline is registered in BD-356's
     card before GATE-DATA-01: person-hours per pack for the last three packs, by role and site total, with no
     per-person figures, held in the private register.
- *Kill (month 12):*
  - No data by month 7 → **M2′**: parent-level metric and reserve assurance on public data, the Nevada
    exploration hindcast and PQ-7 deal cards.
  - Data, but zero attested findings → report the null honestly and offer **one** second site with a
    recorded variance.
  - A second null → H-f is falsified as the wedge; pivot to H-e plus PQ-7.

**Program gate (month 12; §34.5).**

**M3 — Group metric and reserve assurance plus the FA bridge (months 12–18).**
- *Artifact:*
  - a certified AISC/AIC/C1/TCC engine reading SAP extracts at the pilot and consolidating published group
    figures;
  - the reserves register with roll-forward, every named reserve-replacement basis and the QP attestation
    workflow (§24);
  - the data-verification index (GATE-HALL-03);
  - a JORC-style reconciliation section drafted for QP review if the code is in force;
  - an FA draft book with a statistical roll-forward and summary metal subledger through `fa --json` (§21).
- *Acceptance:* three years of published reserve statements reproduced; an auditor replays the AISC certificate
  offline with `fmine-verify`; FA dry-run plan certificates for one month of summary posts, or a documented FA
  blocker list.
- *Kill:* KILL-1 trigger (b) at month 15, as defined in §21.3. **Fallback:** as defined there (a local certified
  ledger with FA as a draft adapter). No executive deliverable slips.

**M4 — Exploration and capital allocation, the second wedge (months 15–24).**
- *Artifact:*
  - a Nevada public-district model: the thinned marked point process with effort (§11), plus a USGS three-part
    parity mode with MapMark4 as lab oracle;
  - a temporal hindcast;
  - one Barrick region on internal data, outside North America unless OPEN-B4-08 is settled (§32.3);
  - junior earn-in, stake and option DecisionCards with distance-to-Tier-One (§23.8).
- *Acceptance:* three-part parity on a published tract (GATE-LODE-P2); hindcast PR-AUC vs baseline with CIs and
  the count of positives; coverage reported; at least one live deal evaluated **before** the decision, recorded in
  the decision ledger.
- *Kill:* the hindcast cannot beat parity, or has too few positives. **Fallback:** ship parity, effort accounting
  and the deal-structure engine without ML lift claims.

**M5 — Multi-site rollout, TSF evidence state, group calibration ledger (months 24–30).**
- *Artifact:*
  - 3–5 sites;
  - a group calibration ledger with surprise monitors (CUSUM on F-factors that opens a hypothesis review and
    never retrains silently);
  - guidance ranges derived from calibrated intervals;
  - the GISTM evidence-state map for Extreme and Very High facilities (§18);
  - Highwall TARP automata and Muster CCV evidence at the first sites.
- *Acceptance:* ≥ 3 sites use it, or co-sign it, as their reconciliation pack of record; calibration reported over
  ≥ 24 site-quarters; the TSF map covers all requirements for ≥ 3 facilities, with staleness and **no safety
  claim**.
- *Kill:* fewer than 2 sites adopt after 6 months. **Fallback:** freeze the rollout and keep it as an audit and
  assurance tool.

**M6 — Planning under calibrated uncertainty and a twin of record (months 30–36).**
- *Artifact:*
  - a stochastic LOM evaluation at one site, using simulations whose bias and variance are **calibrated by
    M2–M5 reconciliation**;
  - the value of the stochastic solution measured on the site's own plan;
  - a read-only "twin of record" API that any agent, Avathon's included, can cite (§16, §32.3);
  - frankensim capabilities only where they have passed GATE-FSIM-RECONV.
- *Acceptance:* pit limits equal MineFlow/MineLib optima exactly (GATE-PUSH-PIT1); the stochastic-solution value
  reported with CIs; at least one external agent consumes certificates.
- *Kill:* the optimizer differs from the oracle. **Fallback:** keep the vendor optimizer as the oracle. Do not ship
  an optimizer that is merely different.

### 37.4 The dependency firewall

**No milestone's executive deliverable may be blocked by a sibling repo.** A sibling capability enters only behind
an adapter with a working local fallback.

| Milestone | frankensim | FA | fgdb | Lifted / consumed |
|---|---|---|---|---|
| M0–M2 | **None** | **None**: a local exact ledger with FA-compatible ids and idempotency keys | None (fsqlite projections) | fs-evidence, fs-eproc, fs-robust, fs-rand, fs-sparse; franken-evidence/decision (OPEN-SUITE-1); fsci, fp |
| M3 | None | `fa --json` on a **draft book**, summaries only; local fallback | Optional (KILL-2) | + fs-voi |
| M4 | None | — | Optional | + SPDE/GMRF on lifted fs-sparse; fj for NUTS |
| M5–M6 | Lifted crates; `fmine-simbridge` only after RECONV | CloseRun if live | If spatial needs are met | + fs-uq/fs-assimilate patterns |

## 38. Work packages

Each work package (WP) has an owner role, a crate set (§28.3), the gates it must pass and its dependencies. The
first 357 beads (§39) instantiate WP-00 to WP-12 in detail and the rest as epics. The "Depends on" column is
generated from the bead graph (transitive reduction), so it cannot drift from §39.

| WP | Name | Crates | Milestone | Gates | Depends on |
|---|---|---|---|---|---|
| WP-00 | Workspace and discipline | `xtask`, registries, CI lanes | Commit 1 | GATE-LAYERS-00, GATE-BUDGET, GATE-ORACLE-01, INT-AS-001 | — |
| WP-01 | S0 substrate | `fmine-core`, `-units`, `-time`, `-crs`, `-array`, `-registry` | M0 | PO-UNIT/CRS/TIME | WP-00 |
| WP-02 | Plumb core and DecisionCards | `fmine-evidence`, `-claim`, `-calib`; `fmine-decide` (card schema, retrospective cards, VOI) | M0–M1 | GATE-PLUMB-E1, -E2, GATE-HELM-01/02 | WP-01 |
| WP-03 | Public lane and metrics | `fmine-public`, `-metrics`, `-reserves` (public slice) | M0–M1 | GATE-BULL-01, GATE-HALL-01, GATE-ROBOT-01, KILL-PLUMB-5 | WP-07 |
| WP-04 | Bedrock authority | `fmine-runtime`, `-authz`, `-ledger`, `-object` | M0–M1 | PO-BED-1/3/5/6/9, INT-AS-SEC-001 | WP-02 |
| WP-05 | Sync and lab | `fmine-sync`, `-lab` | M1 | GATE-SYNC-01, INT-ATP-001 | WP-04 |
| WP-06 | Surfaces and projections | `fmine-cli`, `-service`, `-report`, `-projection` | M0–M1 | INT-FSQL-001, INT-FMD-001, INT-FMERM-001, INT-FSEARCH-001, INT-FGDB-001 | WP-04 |
| WP-07 | Testpit | `fmine-gen`, `-oracles`, `-verify`, `-derive` | M0–M1 (the e2e harness in M0) | GATE-GEN-01, GATE-DET-01, GATE-E2E-01, GATE-STORE-01, GATE-WALL-01, GATE-PLUMB-E3 | WP-06 |
| WP-08 | Touchstone | `fmine-flow`, `-recon`, `fmine-numerics` (slice) | M1–M2 | GATE-TOUCH-01…06, KILL-PLUMB-6 | WP-07 |
| WP-09 | Corebox import | `fmine-drill`, `-qaqc` | M2 | GATE-CORE-01, -02 | WP-07 |
| WP-10 | Crucible balance slice | `fmine-plant` (flowsheet + P754) | M2 (GATE-CRUC-02 scores through month 18) | GATE-CRUC-01, -02 | WP-08, WP-11 |
| WP-11 | Portal importers | `fmine-io`, `fmine-geom` (slice) | M1–M2 | GATE-PORT-01, -02, GATE-BM-01 | WP-09 |
| WP-12 | Site pilot | deployment, security pack, shadow operations, Site Assurance card, read-only agent slice (`fmine-mcp` read verbs only; BD-244 completes the crate at M5) | M2 | GATE-DATA-01…03, GATE-SEC-01, GATE-TOUCH-03/04, GATE-BULL-03 | WP-03, WP-05, WP-10 |
| WP-13 | Bullion and Hallmark | `fmine-metalacct`, `-fa-bridge`, `-acctpack`, `-reserves` | M3 | INT-FA-001, GATE-BULL-02, GATE-HALL-03, GATE-POST-01, GATE-CRUC-03, GATE-REPORT-01 | WP-03, WP-05, WP-10 |
| WP-14 | Helm capital | `fmine-capital`, `-econ`; `fmine-decide` widened | M3–M4 | GATE-CAP-01…05 | WP-03 |
| WP-15 | Lodestar | `fmine-explore`, `-infer`, `-prospect` | M4 | GATE-LODE-P1…P6 | WP-03, WP-08 |
| WP-16 | Strata | `fmine-geom`, `-geostat`, `-belief`, `-estimate`, `-materials` | M3–M5 (S+0 = month 12) | GATE-STRATA-G0…G5 (G5 = GATE-HALL-02) | WP-12, WP-13 |
| WP-17 | Multi-site operations | `fmine-telemetry`, `-mcp`, `-tui` | M5 | GATE-INGEST-01, GATE-SYNC-02, INT-FMCP-001, INT-FTUI-001 | WP-12 |
| WP-18 | Freeboard, Highwall, Muster | `fmine-tailings`, `-geotech`, `-workforce` | M5 | GATE-FREE/HIGH/MUST-*, GATE-DATA-05 | WP-12 |
| WP-19 | Pushback | `fmine-plan`, `-optimize`, `-lp` | Starts M4 (month 18); acceptance M6 | GATE-PUSH-*, KILL-PUSH-*, INT-FNX-001 | WP-16 |
| WP-20 | Headframe twin of record | `fmine-ops`, `-blast`, `-view` | M6 | GATE-HEAD-01…03, GATE-DATA-04 | WP-17 |

## 39. The first 357 beads

These 357 beads instantiate WP-00 to WP-12 in detail and WP-13 to WP-20 as epics. They are generated
from one structured list, and the generator refuses unknown dependencies and cycles. Each bead becomes one `br`
issue with the same title, its dependency edges and a body expanded from the cited sections (the idea-wizard and
beads-workflow polishing rounds add tests, logging and acceptance detail before implementation starts).

**Graph facts (computed by the generator).**
- 357 beads; 589 dependency edges; 0 cycles; 0 unknown dependencies; the work-package graph
  derived from these edges is acyclic and is the "Depends on" column of §38.
- Root beads (no dependencies): BD-001, BD-321, BD-323, BD-328.
- Longest dependency chain: 29 beads:
  BD-001 → BD-002 → BD-008 → BD-015 → BD-016 → BD-023 → BD-037 → BD-038 → BD-040 → BD-041 → BD-042 → BD-061 → BD-065 → BD-071 → BD-072 → BD-074 → BD-075 → BD-199 → BD-200 → BD-204 → BD-205 → BD-206 → BD-207 → BD-210 → BD-211 → BD-212 → BD-246 → BD-247 → BD-301.
  It passes through WP-00 → WP-01 → WP-02 → WP-03 → WP-12 → WP-18, ending at "`RequirementSet` beyond GISTM, `EsCommitment` register, `OB-COMMIT`, G-FREE-7…10".
  The public lane sits on the critical path because pilot-site selection waits for the M0 finance review
  (§37.3). Any slip in WP-00 to WP-03 therefore moves the site pilot one-for-one.
- Bead ids are stable. A bead moved between work packages keeps its id, so ids are not monotonic within a
  section. Beads BD-257 to BD-270 were added by the first audit round to give every early gate a closing bead;
  BD-271 to BD-307 carry the corpus findings (C1–C6, Appendix I) into owned work; BD-308 is the numerics
  admission gate (INT-NUM-001) added by the appendix cross-check (Appendix H); BD-309 to BD-320 carry the twelve
  upgrades of the concurrent revision into owned work; BD-321 to BD-337 carry round 7's corrections (Appendix K.2,
  K.3); BD-338 to BD-357 carry the capabilities adopted from the idea duel and B7 (Appendix K.5).

**Rules for every bead.** A bead closes only with cited evidence: a gate record, a test name or a decision entry.
The close reason names that bead's own evidence; a reason shared across beads is not evidence, and no close
reason cites a gate before that gate's record exists in `docs/delivery/`. A bead is never closed over an open
blocker (`br close --force` is not used for this), and an xtask check rejects generic close reasons (R7-C039,
RISK-33). Every implementation bead includes unit tests and, where it has a user-visible effect, an e2e script with
structured logs (§33.5). A bead that would exceed its crate's budget needs a BUDGET-AMEND record first (§28.4).

### WP-00 · Workspace and discipline

| Bead | Title | Depends on | Done when |
|---|---|---|---|
| BD-001 | Workspace skeleton: Cargo workspace, `rust-toolchain.toml` (nightly-2026-08-31), `franken-stack.lock`, `[patch.crates-io]` for one asupersync | — | `cargo check` green on the empty workspace |
| BD-002 | AGENTS.md and the per-crate `CONTRACT.md` template (the ten §0.2 sections plus API, POs, budget) | BD-001 | Template used by the first crate |
| BD-003 | `xtask loc` and `registries/loc_budget.toml` with all 59 rows (§28.3) | BD-001 | CI prints the budget table |
| BD-004 | `xtask check-layers` and `workspace_topology.toml` (L-1, K/E order) | BD-001 | A deliberate upward edge fails CI |
| BD-005 | `xtask check-deps`: deny.toml, banned list (L-5), one-package-id rule | BD-001 | A test crate pulling tokio fails CI |
| BD-006 | `xtask check-caps`: `open_child_region` ban, K/E capability lint (L-3) | BD-004 | Seeded violation fails CI |
| BD-007 | `xtask check-lifts` and the `registries/lifts.toml` schema (L-7) | BD-001 | Header/registry mismatch fails CI |
| BD-008 | `xtask check-scope`: module families vs charters | BD-002 | Smuggled module fails CI |
| BD-009 | `xtask check-claims`: no bare f64 quantities in K/E public signatures (L-6) | BD-004 | Seeded violation fails CI |
| BD-010 | `xtask check-det`: std transcendentals banned in BitwisePortable modules (PO-NUM-2) | BD-004 | Seeded violation fails CI |
| BD-011 | CI lanes on x86_64-linux and aarch64-darwin, debug and release | BD-003, BD-004, BD-005 | Both lanes green |
| BD-012 | Budget-overrun automation: 80% warn, BUDGET-AMEND template, ceiling block (§28.4) | BD-003 | Synthetic overrun blocks a merge |
| BD-013 | GATE-ORACLE-01 check and the fixture capture-record schema (§33.2) | BD-005 | Oracle crate in a prod closure fails CI |
| BD-014 | `docs/delivery/` and `docs/adr/` with ADR-RNG-01 and the naming ADR (OPEN-B4-09) | BD-002 | ADRs merged |
| BD-015 | GATE-LAYERS-00: every xtask check wired from commit 1 | BD-006, BD-007, BD-008, BD-009, BD-010, BD-011, BD-012, BD-013 | Gate record PASS |
| BD-257 | INT-AS-001 admission: asupersync `=0.5.0` closure, one package id per posture closure, no second runtime, `[patch.crates-io]` single `Cx`; franken-evidence/-decision API check | BD-001 | Admission record in CELL-DEC; OPEN-SUITE-1 closed |
| BD-308 | INT-NUM-001 admission: fsci `=0.4.0` (default features off), fp without `fp-io`, `fnp-io`, fj (`fj-core`, `fj-lax`, `fj-ad`) closures pass L-5 and the one-package-id rule; oracle parity on every consumed kernel; cross-ISA determinism scope recorded | BD-005 | Admission record per foundation, or the FD-row fallback taken |
| BD-323 | Correct the bead and INT records: reopen beads whose done-when is unmet; each close reason cites its own evidence; INT-AS-001 reads not admitted, INT-NUM-001 the FD-row fallback (K.2 findings 4–5) | — | No closed bead cites GATE-BULL-01 before a scorer PASS on real inputs; INT records match §34.2 |
| BD-324 | `xtask` close-reason check: rejects shared or generic close reasons and closing over open blockers | BD-323 | A shared close reason or a close over a blocker fails CI |
| BD-325 | INT-AS-001a closure checks (one package id, criterion 9, one `Cx` through `[patch.crates-io]`) | BD-005 | Record reads NOT_APPLICABLE while asupersync is absent; the checks run in CI |
| BD-326 | `deny.toml` in CI with the rider-licence allow-list and duplicate policy; `franken-stack.lock` row ids aligned with F.2 | BD-005 | CI runs cargo-deny; no lock row id collides with an F.2 upstream ask |
| BD-327 | CONTRACT template completed with all ten §0.2 sections; existing CONTRACT.md files regenerated | BD-002 | Every crate's CONTRACT.md has the ten sections |
| BD-357 | Salted review: a handful of sealed, consented seeds per internal review round (bead closes, gate records, then the QP pack) to measure whether sign-off catches errors; never an issuer-attributed false figure before an issuer reader | BD-324 | Two rounds report the catch rate with a Clopper–Pearson bound; no seed reaches an export |

### WP-01 · S0 substrate

| Bead | Title | Depends on | Done when |
|---|---|---|---|
| BD-016 | `fmine-core` IDs and runtime object prefixes (§0.3) | BD-015 | Property tests on parsing and display |
| BD-017 | Codec kit: magic `FMNE`, LE, bounds, CRC32C, canonical order; fuzz target | BD-016 | 24 h fuzz clean |
| BD-018 | Lift fs-blake3 to `fmine-core::hash`; SHA-256 for chains | BD-007, BD-016 | Official test vectors pass |
| BD-019 | Lift `fs-math/det.rs` to `fmine-core::det` | BD-007, BD-016 | Golden hash of 10^4 inputs on both ISAs |
| BD-020 | Lift fs-rand Philox + Sobol/Owen; key schema `(seed_root, partition, domain, var_group, r, brick)` | BD-007, BD-016 | Known-answer vectors pass |
| BD-021 | FMT-ANCHOR-1 (FSS fields + `cell_id` + `residency_epoch`) | BD-017, BD-018 | Round-trip and N/N-1 reader tests |
| BD-022 | Lift fs-eproc, fs-voi, fs-robust (CVaR), fs-tropical into `fmine-numerics` | BD-007, BD-019 | Per-module known answers |
| BD-023 | `Qty<D, B, R>`, mass bases, `MetalStage`, material kinds (§25.2) | BD-016 | Compile-fail tests for basis mixing |
| BD-024 | Arithmetic laws: no `Add` for Grade, `mass_weighted_mean`, contained = grade × dry | BD-023 | PO-UNIT-3 metamorphic tests |
| BD-025 | `ExactQty` i128 scaled decimal and exact constants (oz t, lb, 240/7 g/t) | BD-023 | PO-UNIT-1 round trips |
| BD-026 | `Money{ccy, minor}` boundary type; no implicit FX | BD-025 | Cross-currency add fails to compile |
| BD-027 | `UNREGISTERED_UNIT`; refusals FMINE-REF-105/107 | BD-024 | Refusal tests |
| BD-028 | `RecordTime`, `ValidTime`, `TimeEvidence` (§25.4) | BD-016 | Ordering-by-anchor tests |
| BD-029 | Civil-from-days, ISO weeks, zone offsets from a registry snapshot | BD-028 | PO-TIME-1 vs oracle table 1900–2200 |
| BD-030 | `fmine-registry` loader and `registry-check` library | BD-017 | Golden registry tests |
| BD-031 | Initial registries: units, crs, calendars, refusals, verbs skeleton | BD-030 | `registry-check` green |
| BD-032 | `SiteCalendar`, `FiscalCalendar`, `Roster`; guard G-TIME-1 | BD-029, BD-031 | Production-day boundary tests |
| BD-033 | EPSG subset, UTM, registered Helmert transforms (§25.5) | BD-023 | Unit tests |
| BD-034 | Mine-grid transform fitted from control points with residuals and hull; G-CRS-2 | BD-033 | Out-of-hull use flagged |
| BD-035 | PROJ fixture corpus capture (lab lane) | BD-033, BD-013 | PO-CRS-1 within stated accuracy |
| BD-036 | `fmine-array` typed grids and views (§29.4) | BD-016 | Stride and view tests |
| BD-277 | `ChemBasis` axis and stoichiometric laws; `Recoverable` metal stage; `HazardousByproduct`; mass fraction in [0, 1] (§25.2) | BD-024 | Compile-fail tests; FS13 nitrate known answer (E322–E324) |
| BD-309 | `QuadTemporalStamp` (`physical_valid_time`, `collection_time`, `availability_time`, `record_time`), `as_of_decision` vs `for_physical_period`, guard G-TIME-2 (INV-016, PO-TIME-2; §6.3, §25.4) | BD-028, BD-032 | Synthetic assay-turnaround lookahead (1–21 d lags) refused by G-TIME-2 |
| BD-310 | `FMT-COL-1` columnar interval chunks in `fmine-array`: run-length `HoleId`, delta depths, bit-packed measurands and qualifiers, 64-byte-aligned value buffers, zone maps (§29.4) | BD-036, BD-017 | 10⁷ intervals composited by sweep line in < 3 s on one core with no per-interval allocation |

### WP-02 · Plumb core

| Bead | Title | Depends on | Done when |
|---|---|---|---|
| BD-037 | Lift fs-evidence types (closure of 3) with a stable `QuantitySpec` shim | BD-007, BD-023 | Builds on the suite toolchain |
| BD-038 | Evidence classes (§7.1) and knowledge states (§7.2) | BD-037 | Lattice property tests |
| BD-039 | Typed refusals `{code, what, fix}` and the FMINE-REF registry (§8.4, §25.6) | BD-037, BD-031 | Every code has text and a fix hint |
| BD-040 | `Claim` (§8.1): canonical body, BLAKE3 id, supersession | BD-038, BD-024, BD-028, BD-018 | Canonical-encoding golden tests |
| BD-041 | `Representation` variants; `DistSpec` over fsci-stats | BD-040, BD-308 | Round-trip tests |
| BD-042 | `compose` and `project_color`; NL-1…NL-8 property tests | BD-041 | GATE-PLUMB-E1: 100% |
| BD-043 | Confidentiality lattice and monotone taint (INV-007) | BD-040 | Property tests |
| BD-044 | `UuTerm` and guard G-UU (INV-015) | BD-040 | Decision-facing claims without UU refused |
| BD-045 | Support and additivity types; FMINE-REF-102/103 (INV-014) | BD-040, BD-024 | Refusal tests |
| BD-046 | Provenance DAG and dependency index for staleness (INV-013) | BD-040 | Superseded deck marks dependants stale |
| BD-047 | Forecast registration: freeze and hash before resolution (INV-008) | BD-040, BD-028 | Late registration refused |
| BD-048 | Proper scoring rules (log, CRPS, interval score) and coverage | BD-047 | Known-answer scores |
| BD-049 | Calibration records per family, segment and author; `CalibrationFlag` | BD-048 | Flag transitions tested |
| BD-050 | Reference-class registry (`REFCLASS-`) and ε contamination (§9.3) | BD-049, BD-030 | GATE-PLUMB-E4 check |
| BD-051 | Surprise monitors (e-process, CUSUM) on lifted fs-eproc | BD-049, BD-022 | GATE-PLUMB-E5 on the control mine |
| BD-052 | Claim rendering: level1, markdown and FM rows (§30.5) | BD-044 | Golden renders |
| BD-053 | As-of replay harness | BD-046, BD-080 | GATE-PLUMB-E2: bit-identical replay |
| BD-054 | Epistemic one-pager data model (§9.6) | BD-044, BD-049 | Golden one-pager |
| BD-080 | L0 in-memory reference ledger (semantic oracle) | BD-021, BD-040 | Semantic suite green |
| BD-225 | DecisionCard schema, rendering and decision ledger entries | BD-044, BD-049 | Golden cards |
| BD-226 | Two retrospective cards built as known at decision time | BD-225, BD-053 | GATE-HELM-01 |
| BD-227 | VOI estimator vs closed-form EVSI | BD-225, BD-022 | GATE-HELM-02 |
| BD-311 | `Monitor::SpatialFrontPit`: sequential PIT under the registered joint (Vecchia) model along the mining front, with aGRAPA betting e-processes on the location score and the centred dispersion score (§10.4, GB-08) | BD-051, BD-048 | On synthetic truth: median alarm ≤ 40 polygons for the 58%-coverage dispersion shift and ≤ 25 for the location shift; null false-alarm rate ≤ α over 600 polygons |
| BD-343 | Forecast cohorts and outcome vintages: frozen membership, append-only assessments, overdue and missing outcomes shown, best and worst bounds for bounded scores, restated outcomes scored separately | BD-047, BD-048 | No missing outcome is dropped from a denominator; restatement never overwrites an assessment |
| BD-352 | Evidence-origin accounting: origin identities and typed relations (publication copy, superseding publication, repeated reading, replicate, new observation, shared systematic-error group) | BD-042 | Batch-bias known answer σ_b² + σ_ε²/n reproduced; a reprint never narrows an interval |

### WP-03 · Public lane and metrics

| Bead | Title | Depends on | Done when |
|---|---|---|---|
| BD-055 | Filing source registry: accessions and table locators | BD-030 | Registry check; website-published workbooks (no accession) carry a sha256 and the reconciling filed page |
| BD-056 | Offline filing ingest into CAS with digests (downloads are manual; engines never fetch) | BD-090, BD-055 | Digest-stable re-ingest |
| BD-057 | Pure-Rust HTML table extractor with locator provenance | BD-056 | Hand-checked fixtures exact |
| BD-058 | Verify that MD&As carry per-site components (B5 open question 2) | BD-057 | Decision recorded in CELL-DEC (partial answer in §21.6: Mine Stats give TCC → AISC only as a residual) |
| BD-059 | Public-basis Claims per site-quarter (AISC, TCC, GEO, grade, recovery) | BD-057, BD-040, BD-271 | Every Claim cites a locator |
| BD-060 | Metric-definition registry v1 (WGC AISC/AIC; by-/co-product; GEO ratio; RR bases) | BD-030 | Registry check |
| BD-061 | Metric engine over Claims with exact money | BD-060, BD-042, BD-026 | WGC examples reproduced |
| BD-062 | GEO with versioned price decks (GB-27) | BD-061 | Known answers |
| BD-063 | Reserve replacement under every named basis | BD-061 | 190% vs 504% bridge reproduced (gold 3-yr 2023–25 vs GEO 3-yr 2023–25, both ex-A&D; OPEN-BULL-4 resolved, E603, E604) |
| BD-064 | LMDI waterfall (GB-26) with metal-balance B and sold/produced S factors | BD-061 | Exact decomposition tests; North Mara Q2 2025 → Q2 2026 known answer (E137) |
| BD-065 | Reproduction harness vs published values to rounding | BD-061, BD-059, BD-322 | GATE-BULL-01 scorer runs |
| BD-066 | Discrepancy register with citations or the closing document | BD-065 | Every miss bridged or OPEN |
| BD-067 | FY2026 per-site forecasts, 80% intervals, flag `uncalibrated` | BD-322, BD-334 (rules), Q3 2026 vintage | Forecast set complete |
| BD-068 | Freeze, hash and externally timestamp forecasts before the Feb 2027 release | BD-067, BD-334 | Timestamp precedes release |
| BD-069 | Lane separation: public build profile and access attestations (§31.1) | BD-093 | No private-access author in the lane |
| BD-070 | Published reserve roll-forward reproduction | BD-059, BD-063 | GATE-HALL-01 |
| BD-071 | `fmine assure public --issuer` end to end | BD-114, BD-065 | Robot output validates |
| BD-072 | M0 report with anchors, certificates and the `no_claim` line | BD-123, BD-071 | Byte-identical reruns |
| BD-073 | M0 security pack: SBOM, threat model, offline single binary | BD-011 | Pack reviewed |
| BD-074 | M0 e2e script with structured logs | BD-071, BD-072, BD-142, BD-338 | GATE-E2E-01 for M0 |
| BD-075 | Two finance reviewers' assessment recorded (kill criterion) | BD-074 | Assessment in CELL-DEC |
| BD-076 | Score FY2026 forecasts at release and report whatever they show | BD-068, BD-048, BD-343 | Scores published to the sponsor |
| BD-125 | GATE-ROBOT-01: a fresh agent completes 20 tasks from robot-docs alone | BD-116, BD-115, BD-071 | ≥ 18 correct, 0 authority writes, 0 wall breaches |
| BD-260 | KILL-PLUMB-5 check: executive user test of the epistemic map on the M0 cards | BD-072, BD-054, BD-225 | Test recorded; three-field card adopted if rejected |
| BD-271 | Pure-Rust xlsx table extractor: label-anchored, placeholder-typed, `#REF!`/oversized-range excluding, `<file-period>:<Sheet>!<Cell>` + sha256 provenance (§21.6) | BD-056 | The 3,996 parsed cells of the Q1/Q2 2026 Mine Stats reproduce exactly; the 7 zero placeholders import as Nil; all ten W1 workbooks (Q1 2024–Q2 2026) import under the versioned region map and unit rules (E461, E463), and the Q3 2026 file imports on release day without code changes |
| BD-272 | GATE-BULL-01 scoring rule: numeric-cell denominator, NOT_PUBLISHED site-quarters excluded by name, identities on rounding intervals under a declared construction (the ±1-last-digit rule is withdrawn: it flips 21 of 306 regional verdicts, B7 EV2-P1-1), five states (§21.6) | BD-065 | Scorer reports the five states and the excluded list; an independent census agrees on every state |
| BD-273 | AISC aggregation layers (L_corp, L_JV) and definition flags (by-product credit location, inventory adjustments, per-oz denominator) | BD-061 | Barrick Total Gold and NGM gaps decomposed into named layers or refused (G-BULL-3) |
| BD-274 | Published cut-off reproduction: fixed-point cut-off with R(g), exact $/t ↔ $/st | BD-061, BD-025 | Seven corpus cut-offs reproduced (E182–E184) |
| BD-275 | Corpus discrepancy fixtures: identity breaks, copied cells, roll-forward sign and deck mismatches | BD-066, BD-070 | E113–E116, E144, E147, E153, E193 filed with closure arithmetic |
| BD-276 | Technical-report assumptions registered as frozen forecasts with resolution criteria | BD-067 | Loulo LOM cost and recovered-gold claims registered and scored |
| BD-321 | Remove both silent fallbacks; move the stand-in workbook and every issuer-attributed literal to a synthetic lane that prints `synthetic` and cannot count toward GATE-BULL-01; relabel the stand-in tests and add a real-file run (K.2 step 1) | — | A missing or unreadable registered file exits non-zero naming it; synthetic output is labelled and excluded |
| BD-322 | Source registry pinning the real workbooks (URL, sha256, retrieval date) and the label-anchored importer rebuilt on it before the Q3 2026 release (K.2 step 3; E693) | BD-321, BD-271 | All 18 registered files import by label; the run's input manifest equals the registry |
| BD-329 | Uncited-number check and the full envelope: locator and sha256 or rule id and input locators on every number; the four INV-002 flags, certificate, `next_actions` and the `no_claim` line | BD-322, BD-065 | A number without its citation fails the run |
| BD-330 | Filed-page label check for company-level figures by a person who did not build the output (three-year reserve replacement 190% and 504%; FY2025 AISC $1,637/oz on 3,318 koz sold) | BD-329 | Check record signed; any mismatch is a register item |
| BD-331 | Five-state identity ledger with a rounding construction declared per table family, explained items marked stated or reconstructed (§21.6 GATE-BULL-01 part 2; E691–E698, E815–E818) | BD-272, BD-340 | Independent census assigns the same state to every instance; known answers E691, E692, E694, E698, E815, E816 and E817 land in their states |
| BD-332 | Vintage and basis key on public Claims (subject, valid period, reporting vintage, ownership basis, scope, unit) with the raw token; numeric zero in a disposed or withheld column stays `Nil` | BD-059, BD-322 | First publication and restated comparatives coexist; the Tongon disposed-site zero is never a recovery |
| BD-333 | Waterfall on public data: factor-domain refusal with the signed additive fallback, rounding interval per bar with `sign not determined`, printed-rate gap listed (§21.6; E699) | BD-064, BD-331 | Veladero and Zaldívar fall back; the North Mara grade-error control is reported as non-identifying |
| BD-334 | FY2026 forecast record manifest: information cut-off, input manifest, 100% basis rule and the two baselines (§21.6) | BD-067 | Manifest hashed with the forecasts |
| BD-338 | Live-dependency proof: per registered file, deletion or blanking refuses; per cited leaf cell, a one-unit perturbation moves the value and the state root (threshold rules are crossed at their FlipMargin) | BD-322, BD-065 | Seeded hard-coded literal, dead ancestor edge and silent fallback all fail; every M0 scored value passes |
| BD-339 | Reproduction kit for outside readers: source URLs, sha256 digests, label selectors and expected state roots, with no workbook bytes in the repository | BD-322 | A clean checkout plus the issuer's own downloads reproduces every state root |
| BD-340 | Exact rounding-interval consistency solver: linear identities first, certificates checked in exact rationals, inclusion-minimal conflict sets, implied intervals, discovery of rounding convention, share, deck and unit | BD-024, BD-061 | E113–E116, E144, E147, E691, E692 and E698 reproduced; the component-sum construction discovered on the AISC tables with the three non-footing cells of E815 certified; zero certified conflicts over 10⁴ generated clean tables |
| BD-341 | Guidance track record: issuer per-site guidance FY2023–FY2026 transcribed by two readers with pages; outcomes from first-published vintages; hit rate, bias and price-conditional split, descriptive only below n_eff ≈ 35 | BD-332, BD-047, BD-048 | Every guidance and outcome value reproduces its cited page or cell; estimators cover truth 85–95% on synthetic ranges |
| BD-349 | Evidence-challenge packet for public exceptions: every operand locator, owner role, blocked uses and the closing document (§22.1) | BD-066 | Every register item renders as a packet; no packet carries an invented NPV, CVaR or likelihood |
| BD-345 | Disclosure lint, cited mode: each number in a draft carries a Claim link and gets verified, value, basis, period, rounding, stale, uncalibrated, uncited or ambiguous; checks value and basis, not link presence; a publicness verdict per number (public since which filing and date, on which basis, or not found in any public filing, routed to counsel) | BD-329, BD-332 | Two-reader labelled set: no false `verified`; prose outside supported predicates is labelled unchecked |
| BD-347 | Reliance status for issued outputs and reviewed decisions: issuance record (digest, output Claims, anchor), withdrawal and supersession events, an outbound register of issued copies by recipient role, consumer check current / withdrawn / superseded / unknown | BD-329 | A withdrawn output is refused for current reuse while its historical digest still verifies |

### WP-04 · Bedrock authority

| Bead | Title | Depends on | Done when |
|---|---|---|---|
| BD-077 | Region tree and CapSet profiles per role (§26.6) | BD-015 | Compile-time narrowing tests |
| BD-078 | Budget table per region and on-exhaust behaviour (§26.7) | BD-077 | Exhaust tests |
| BD-079 | AppSpec per role, restart intensity, read-only degraded mode | BD-077 | Escalation lab tests |
| BD-081 | FMT-LEDGER-1 batch codec; fuzz target | BD-017, BD-018 | 24 h fuzz clean |
| BD-082 | FMT-SEG-1 segment files: CRC32C, hash chain, MMR footer (L1) | BD-081 | Torn-write recovery tests |
| BD-083 | MMR over batch digests and consistency proofs | BD-018 | Proof known answers |
| BD-084 | Cells, leases, fencing epochs | BD-082 | PO-BED-1 unit tests |
| BD-085 | Witnesses, narrow combiner, keyed index rebuilt at open | BD-084 | Optimistic-concurrency tests |
| BD-086 | Commit-time law hook (generic), used by conservation | BD-085 | Refusal path tested |
| BD-087 | Shared semantic suite run against L0 and L1 | BD-080, BD-085 | Both rungs green |
| BD-088 | Hash-chain verification; repair plan truncates to the last verified record | BD-082 | F7 lab test |
| BD-089 | Durable obligation registry (OB-*) | BD-077, BD-082 | PO-BED-5 replay |
| BD-090 | FMT-CAS-1: chunks, manifests, fanout-16 Merkle, root-last | BD-018, BD-017 | PO-BED-3 crash tests |
| BD-091 | GC by reachability from ledger roots | BD-090, BD-084 | No reachable object collected |
| BD-092 | First-party macaroons; random retired key; zero-signature rejection (S3 b, c) | BD-077 | INT-AS-SEC-001 (b), (c) |
| BD-093 | MNPI and venture caveats; AccessEvent log (§31.1–31.2) | BD-092, BD-043 | Wall differential tests |
| BD-094 | Residency registry with a fail-closed default | BD-030, BD-092 | Unknown pair → in-country only |
| BD-095 | HumanAuthorityToken: ≤ 15 min, single-use, effect digest | BD-092 | PO-SEC-3 |
| BD-096 | FMT-CERT-1 certificates (inputs closure, code rev, seed, class, digest) | BD-021, BD-046 | Verifier round trip |

### WP-05 · Sync and lab

| Bead | Title | Depends on | Done when |
|---|---|---|---|
| BD-099 | LabRuntime wiring: virtual time, seeds 0..999 in CI, DPOR | BD-077 | Same seed → same trace digest |
| BD-100 | Chaos profiles (link drop, crash at every boundary) and CrashPack | BD-099 | Failing seed reproduces |
| BD-101 | FM oracles PO-BED-1, 2, 3, 4 and 10 | BD-100, BD-084, BD-090 | Oracles fire on seeded bugs |
| BD-102 | Version vectors per cell | BD-084 | Lag detection tests |
| BD-103 | `SyncCert` verified on every apply | BD-083, BD-102 | PO-BED-2 |
| BD-104 | Chunk want/have manifest anti-entropy | BD-090 | GB-36 sizes measured |
| BD-105 | QUIC/TLS transport over ATP; plaintext banned (S3 d) | BD-104, BD-092 | Plaintext attempt refused |
| BD-106 | Chunk receipts and OB-SEAL resume; request → drain → finalize | BD-105, BD-089 | Resume after hard kill |
| BD-107 | Residency planner refuses before bytes move | BD-094, BD-102 | FMINE-REF-303 tests |
| BD-108 | Gluing checker on anchor publish (INV-012) | BD-103 | Dangling reference blocks publication |
| BD-109 | GATE-SYNC-01: 1,000-seed partition matrix | BD-101, BD-106, BD-108 | Gate record PASS |
| BD-110 | INT-ATP-001 admission record | BD-109 | Record filed |
| BD-097 | Runtime capability-mask lab test | BD-077, BD-099 | PO-BED-8 |
| BD-337 | INT-AS-001 criterion 8: scheduler or diagnostic adaptation cannot alter authoritative results | BD-099 | Authoritative outputs byte-identical over seeds 0..999 and three diagnostic levels |

### WP-06 · Surfaces and projections

| Bead | Title | Depends on | Done when |
|---|---|---|---|
| BD-111 | FMT-ENV-1 envelope with the four flags false | BD-040 | PO-ADIT-1 golden test |
| BD-112 | Exit-code dictionary 0, 2–9; never 1 | BD-111 | PO-ADIT-4 fuzz |
| BD-113 | `registries/verbs.toml` schema and generator | BD-030, BD-111 | Generator golden output |
| BD-114 | `fmine` binary; bare triage | BD-113, BD-112 | Smoke e2e |
| BD-115 | `claim show`, `why`, `trace` | BD-114, BD-046 | Golden outputs |
| BD-116 | `robot-docs` and `robot schema` | BD-113 | Schemas validate all outputs |
| BD-117 | `fmine doctor`: chains, HWMs, pins, budgets, obligations, Little audit | BD-114, BD-088, BD-089 | Doctor finds seeded faults |
| BD-118 | `repair plan` and `repair apply <digest>` | BD-117 | PO-ADIT-3 tamper test |
| BD-119 | `fmined` per role: socket, leases, supervisors | BD-079, BD-084, BD-114 | Daemon lab tests |
| BD-120 | Projector framework with high-water marks | BD-084 | HWM tests |
| BD-121 | fsqlite projection: tables, 3-D R-tree, FTS5 | BD-120 | INT-FSQL-001: bbox p99 ≤ 50 ms |
| BD-122 | Discard and rebuild; freshness-required refusals (F5) | BD-121 | Corruption drill passes |
| BD-123 | `fmine-report` over franken_markdown and frankenmermaid (default features) | BD-111 | INT-FMD-001, INT-FMERM-001, or the FD-FMD-00 fallback taken (plain markdown without diagrams; §37.4 firewall, R7-C085, R7-C103) |
| BD-124 | Report determinism by digest | BD-123 | Byte-identical rerender |
| BD-126 | INT-FGDB-001 measured-benefit spike on 3 named queries | BD-120 | KILL-2 decision recorded |
| BD-262 | INT-FSEARCH-001: quill + hash/model2vec only, no ort/fastembed, wall-scoped indexes | BD-120, BD-093 | Admission record, or fallback to fsqlite FTS5 |
| BD-328 | Refusal registry as the single source: plan, registry and compiled catalogue agree; family → exit-code table (§30.1) | — | All refusal codes agree across the three; usage errors exit 2 |

### WP-07 · Testpit

| Bead | Title | Depends on | Done when |
|---|---|---|---|
| BD-127 | `fmine-gen` framework with sealed truth | BD-020, BD-040 | GATE-GEN-01 |
| BD-128 | Open-pit Au world: geology, grades, density | BD-127 | Truth statistics match spec |
| BD-129 | Cu–Au flotation world | BD-128 | Truth statistics match spec |
| BD-130 | Seeded biases: resource model, dilution, survey, sampler, density, moisture | BD-128 | Bias registry complete |
| BD-131 | Drillhole and assay generator at 10^5 holes / 10^7 intervals | BD-127 | Size and determinism tests |
| BD-132 | Dispatch tonnage generator | BD-128 | Determinism tests |
| BD-133 | Mill sampler and metallurgical balance generator | BD-129 | Determinism tests |
| BD-134 | Synthetic FA-compatible book on an exact local ledger | BD-127, BD-026 | Balances exact |
| BD-135 | Multi-site company with ventures and walls | BD-128, BD-093 | Wall fixtures generated |
| BD-136 | Fixture capture tooling and records | BD-013 | Record schema enforced |
| BD-137 | AMIRA P754 and two-product formula fixtures | BD-136 | Fixtures captured |
| BD-138 | WGC AISC example fixtures | BD-136 | Fixtures captured |
| BD-139 | `fmine-derive` with fsym (dev-only) golden formulas | BD-015 | OPEN-SUITE-3 resolved |
| BD-140 | `fmine-verify`: certificates, MMR proofs, CAS roots | BD-096, BD-083, BD-090 | Tamper tests exit 8 |
| BD-141 | BitwisePortable fixture families on both ISAs | BD-011, BD-019 | GATE-DET-01 |
| BD-142 | `tests/e2e` NDJSON logging harness and CrashPack | BD-114 | Harness self-test |
| BD-143 | Metamorphic library: split/merge lots, permutations, unit round trips | BD-024, BD-127 | Library used by 3 crates |
| BD-144 | SBC rank histograms and coverage harness | BD-127, BD-048 | Known-calibrated control passes |
| BD-098 | GATE-STORE-01: 10^5 holes / 10^7 intervals rebuilt into fsqlite in ≤ 15 min | BD-121, BD-131 | Gate record PASS or KILL-4 |
| BD-258 | GATE-WALL-01: 10^4 generated cross-wall query pairs over the multi-site venture world | BD-093, BD-135 | 0 leaks; gate record PASS |
| BD-259 | GATE-PLUMB-E3: misspecified synthetic mine (out-of-model fault; biased capex process) | BD-051, BD-130, BD-144 | 80% coverage restored to ±5 points within 12 resolutions; global-posterior failure documented |

### WP-08 · Touchstone

| Bead | Title | Depends on | Done when |
|---|---|---|---|
| BD-145 | Lift fs-sparse (CSR, LDLᵀ + AMD, PCG) into `fmine-numerics::sparse` | BD-007 | Residual known answers |
| BD-146 | Material-flow graph: nodes, streams, stockpiles (§20.1) | BD-045, BD-024 | Graph invariants tested |
| BD-147 | Conservation at commit; FMINE-REF-101 (INV-005) | BD-146, BD-086 | PO-BED-6 |
| BD-148 | F-factors as posterior quantities; site-profile definitions | BD-146 | Definitions registry |
| BD-149 | Linear WLS reconciliation with covariance | BD-147, BD-145 | P754 fixtures reproduced |
| BD-150 | Gross-error tests with stated power | BD-149 | Power curves match theory |
| BD-151 | Bilinear tonnes × grade reconciliation | BD-149 | Known answers |
| BD-152 | Hierarchical bias posteriors per node (§20.5) | BD-151, BD-050 | SBC passes |
| BD-153 | Identifiability analysis; FMINE-REF-104 | BD-152 | Non-identifiable pairs refused |
| BD-154 | Latent stockpile states (§20.8) | BD-152 | Known answers |
| BD-155 | Per-lot moisture in balances (GB-34) | BD-151, BD-024 | Moisture vs weightometer separable with lot data |
| BD-156 | ReconciliationCertificate | BD-153, BD-096 | Verifier round trip |
| BD-157 | GATE-TOUCH-01 known answers on synthetic flowsheets | BD-150, BD-133 | Gate record PASS |
| BD-158 | GATE-TOUCH-02: ≥ 200 replicates, coverage 85–95% | BD-153, BD-130, BD-144 | Gate record PASS |
| BD-159 | Detection power for a 5% sampler bias per quarter | BD-158 | Power statement published |
| BD-160 | R0 parity mode adopting a site's F-factor conventions | BD-148 | Synthetic site pack reproduced |
| BD-161 | Rolling-origin hindcast tool | BD-156, BD-053, BD-309 | No-hindsight test passes |
| BD-162 | `fmine recon run` and `close` verbs | BD-156, BD-114 | e2e green |
| BD-261 | KILL-PLUMB-6 check: monitor false-alarm rate on the control mine | BD-051, BD-158 | ≤ 1 false alarm per site per quarter, or support re-specified |
| BD-278 | Site-specific F-factor and MCF definitions registry (Kibali, Loulo, NGM; §20.4) | BD-148 | Registry rows with locators; G-TOUCH-4 renders registry keys |
| BD-279 | Verbatim reconciliation-table importer: recompute identities; residual rows as (Δt, Δoz) with grade NOT_DEFINED (G-TOUCH-6) | BD-278 | Kibali 2024 MCF row and Loulo RCF ounces filed as DiscrepancyRecords |
| BD-280 | Within-factor compensation detector and KA-TOUCH-KIBALI-2016-2025 | BD-278, BD-136 | Goldstrike Roaster and Kibali 2016–2025 flagged |
| BD-281 | GATE-TOUCH-06: public reconciliation known answers (Kibali, Pueblo Viejo 2025, Porgera 2007–2011) | BD-279, BD-280, BD-154 | Gate record PASS (M1) |
| BD-312 | `StockpileVoxelTwin` (voxel parcel provenance on `VdbGrid`, oxidation exposure, plant cone as a latent inventory) and lift/panel heap-leach convolution (§20.8) | BD-154, BD-281, BD-309 | Reclaim grade and oxidation posteriors tracked on synthetic piles; the Pueblo Viejo 2025 plant-cone swing (E666) carried in the monthly balance |
| BD-344 | Temporal-cone check over every ancestor of a hindcast or retrospective card (fitted parameters, exclusion lists, decks, registry rows, priors) with the minimal leakage path; replay with post-cut objects removed from the store must match | BD-309, BD-161 | Seeded leaks through a variogram fit, an exclusion list and a deck are each caught |

### WP-09 · Corebox import

| Bead | Title | Depends on | Done when |
|---|---|---|---|
| BD-163 | Ledger records: collar, survey, interval, sample, dispatch, result, QC, certificate | BD-040, BD-084 | Supersession tests |
| BD-164 | Desurvey (minimum curvature) with oracle tolerance | BD-163, BD-033 | ≤ 1 mm / 1,000 m |
| BD-165 | Sweep-line from/to overlap join per hole | BD-163, BD-036 | Matches brute force |
| BD-166 | Length-weighted compositing with support types | BD-165, BD-045 | Known answers |
| BD-167 | CRMs, blanks and duplicates: insertion and evaluation | BD-163 | Known answers |
| BD-168 | Left-censored lognormal MLE and ROS (§12.2) | BD-167 | Matches reference fits |
| BD-169 | E-process drift detection | BD-167, BD-022 | GATE-CORE-02 |
| BD-170 | MNPI embargo state on results | BD-163, BD-093 | PO-CORE-5 |
| BD-171 | Lab certificate ingest with originals in CAS | BD-163, BD-090 | Digest-linked |
| BD-172 | OB-LAB: dispatched → received or declared lost | BD-163, BD-089 | Obligation replay |
| BD-173 | GATE-CORE-01: 10^5 holes ingested, desurveyed and composited in ≤ 15 min | BD-166, BD-131, BD-121 | Gate record PASS |
| BD-174 | Data-verification index skeleton per technical-report item (§24.5) | BD-167, BD-166, BD-282 | Index renders |
| BD-282 | CRM rules with a named SD basis and test (single-value, CANMET), bias with interval, versioned site rule texts | BD-167 | Re-derived synthetic batch gives per-basis results and a bias interval |
| BD-283 | QA/QC pattern monitors including dispersion collapse (§12.4) | BD-282, BD-169 | GATE-CORE-02 variance-collapse clause passes |
| BD-284 | Duplicates as CV_AVR%; Gy calibrated form with K/α provenance; nugget split → protocol-vs-infill option | BD-167 | Known answers; nomogram `OutsideValidity` off its calibration grade |
| BD-285 | `TwinPair` records, G-TWIN disclosure guard and RC smearing monitor | BD-163, BD-164 | `WeakTwin` above 5 m; disclosure-basis refusal test |
| BD-286 | Core recovery with method, recovery–grade and data-generation tests, survey checks, `CommittedFlagged`, retention gate, `qaqc_regime` | BD-163, BD-164 | Each check fires on a seeded synthetic campaign |

### WP-10 · Crucible balance slice

| Bead | Title | Depends on | Done when |
|---|---|---|---|
| BD-175 | Flowsheet graph types (§17.2) | BD-146 | Invariants tested |
| BD-176 | Process cards with validity boxes; G-CRUC-1 | BD-175, BD-041 | Out-of-box refused |
| BD-177 | P754 metallurgical balance from raw measurements | BD-175, BD-149 | P754 fixtures exact |
| BD-178 | Sampler bias and precision models | BD-177 | Known answers |
| BD-179 | Concentrate and doré custody; FMINE-REF-403 | BD-175, BD-024 | Refusal tests |
| BD-180 | Shipment-release prepared effect with dual attestation | BD-179, BD-095 | Token tests |
| BD-181 | Registered monthly recovery and throughput forecasts | BD-176, BD-047 | Registrations frozen |
| BD-182 | Recovery transfer card slice (e.g. CuAS/CuT) | BD-176 | Card validity tests |
| BD-183 | Heap inventory as a latent state (GB-20) | BD-154 | Known answers |
| BD-184 | GATE-CRUC-01 on synthetic plants first | BD-177, BD-133 | Gate record PASS (synthetic) |
| BD-263 | GATE-CRUC-02: register monthly recovery and throughput forecasts for 12 months and score them | BD-181, BD-049 | Coverage in [0.70, 0.90] at month 18, else cards stay `Estimated` |
| BD-287 | `CorrelationCard` input_units/output_scale/parse_check, G-UNIT-4, card layers and versions | BD-176 | NGM's three unit conventions import; an unbalanced formula is refused |
| BD-288 | Preg-rob card as a spike-test percentage with feed caps and `carbon_type` | BD-287 | Validity example renders the 40%/60% caps |
| BD-289 | First recovery calibration card from Turquoise Ridge's 71 lab-vs-plant pairs | BD-287, BD-049 | Bias and coverage reported (E177) |
| BD-290 | Heap book convention: R_card vs R∞, `Recoverable` stage, Veladero roll-forward known answer | BD-183, BD-277 | Veladero 2013–2017 inventory reproduced to ±1 oz |
| BD-313 | N-tank residence-time deconvolution (`fmine-plant::rtd`) with bypass, dead volume and throughput-volume time for CIL/CIP and POX trains (§17.3, GB-41) | BD-177, BD-287 | Synthetic shift-pulse fixture reproduces GB-41, and deconvolution recovers the flat true recovery |
| BD-336 | Metallurgical-balance and sampler-record import for the recovery-led GATE-DATA-03 route (§37.3 M2) | BD-187 | Site's monthly balance and recovery statement imported with parity |

### WP-11 · Portal importers and geometry lifts

| Bead | Title | Depends on | Done when |
|---|---|---|---|
| BD-185 | Quarantine-on-import pipeline and import reports (§32.1) | BD-077, BD-090 | Rejected rows explained |
| BD-186 | `io_dialects.toml`: unit, basis, support and CRS per column | BD-030, BD-027 | Undeclared → UNREGISTERED |
| BD-187 | CSV/Excel readers (own; `fp-io` excluded, OPEN-SUITE-4) | BD-185 | Fixture parity |
| BD-188 | acQuire export dialect | BD-186, BD-187, BD-163 | Sample export imported |
| BD-189 | Datamine and Surpac export dialects (`.str` text) | BD-186, BD-187 | Sample exports imported |
| BD-190 | omf-rust dependency audit (OPEN-B4-06) | BD-005 | KILL-6 decision recorded |
| BD-191 | Lift `VdbGrid` into `fmine-geom::vdb` | BD-007 | GATE-BM-01 |
| BD-192 | Lift `winding_exact` with `det::atan2` | BD-019, BD-007 | Differential corpus |
| BD-193 | Lift fs-ivl predicates and error-free transformations | BD-007 | Adversarial predicate corpus |
| BD-194 | OMF v2 reader (lifted schema or own Parquet subset) | BD-190, BD-191 | Vendor OMF samples imported |
| BD-195 | DXF ASCII subset | BD-185 | ezdxf fixtures |
| BD-196 | LAS 1.4 reader | BD-185 | PDAL fixtures |
| BD-197 | Fuzz targets per decoder | BD-187, BD-195, BD-196, BD-194 | PO-PORT-1: 24 h clean |
| BD-198 | GATE-PORT-01 oracle agreement (LAS, DXF, CSV dialects) | BD-197, BD-136 | Gate record PASS |
| BD-291 | Import rules: document-quantity replay (G-UNIT-5), CRS-by-fit Claims (G-CRS-3), asserted column basis, `DocApproval` | BD-185 | Reko Diq and Kibali fixtures filed as discrepancies |
| BD-292 | Redact-on-ingest for stakeholder and grievance records; the 72 W3 residency rows (12 countries × 6 classes, statute-cited, E501–E549) with the fail-closed default | BD-185, BD-094 | No personal field survives import of a public ESIA |

### WP-12 · Site pilot

| Bead | Title | Depends on | Done when |
|---|---|---|---|
| BD-199 | Site selection memo: 100% owned, recorded variance (Lumwana, recovery-led; North Mara/Bulyanhulu alternate, grade-led) | BD-075 | Decision in CELL-DEC, with Lumwana's commissioning segmentation |
| BD-200 | Data agreement, insider list and blackout policy | BD-199 | GATE-DATA-01 |
| BD-201 | Security review pack and independent penetration test | BD-073, BD-110 | GATE-SEC-01, GATE-DATA-02 |
| BD-202 | On-prem site node deployment runbook | BD-119, BD-201 | Node live read-only; site IT runs install, upgrade, restore from backup and `fmine doctor` from the runbook alone |
| BD-203 | No-egress verification by site IT | BD-202 | IT sign-off recorded |
| BD-204 | 8–12 quarters of snapshots imported | BD-202, BD-200, BD-188, BD-194, BD-189, BD-336 | GATE-DATA-03, GATE-PORT-02 |
| BD-205 | Site profile: F-factor conventions and pack definitions | BD-204, BD-160, BD-278 | Profile attested by the site |
| BD-206 | Parity with the site's F1/F2/F3 | BD-205 | GATE-TOUCH-03 |
| BD-207 | Reconciliation posterior and attribution at the pilot | BD-206, BD-156 | Certificates issued |
| BD-208 | Per-quarter AISC waterfall | BD-207, BD-064 | GATE-BULL-03 |
| BD-209 | Hindcast plus two forward quarters registered inside the tenancy | BD-207, BD-161 | GATE-TOUCH-05 |
| BD-210 | Material-finding review with the QP and metallurgist | BD-207 | GATE-TOUCH-04 |
| BD-211 | Site Assurance DecisionCard | BD-210, BD-225 | Card reviewed by the sponsor |
| BD-212 | Program-gate evidence pack (§34.5) | BD-211, BD-208, BD-209, BD-203 | Program gate decision recorded |
| BD-348 | Non-interference check for outputs crossing a wall: rerun over a store purged of objects above the recipient's clearance or after the last public filing; the output crosses only if the digests match; disclosure-calendar registry | BD-258, BD-344 | A seeded MNPI note that changes a ranking is caught; deterministic tool outputs only |
| BD-356 | Pilot protocol card: pre-registered pilot design and minimal data ask, power reported as an envelope across misspecified twins | BD-199 | Card reviewed by the sponsor before GATE-DATA-01 |

### WP-13 · Bullion and Hallmark

| Bead | Title | Depends on | Done when |
|---|---|---|---|
| BD-213 | FA conformance workspace and INT-FA-001 (FA-ACC-01…13) | BD-136 | Admission record |
| BD-214 | Posting-slot automaton (`registries/automata/posting.toml`) | BD-099 | A1 checks pass |
| BD-215 | `fa --json` subprocess client in FinanceBridgeRegion | BD-214, BD-077 | FA-ACC-01 |
| BD-216 | Signed intent outbox and replay | BD-215 | PO-BED-10 over 1,000 seeds |
| BD-217 | Merkle folding into ≤ 128 rows / 32 legs / 8 KiB (FMT-POST-1) | BD-214, BD-083 | FA-ACC-04 |
| BD-218 | GATE-POST-01: 3 sites × 3 months synthetic close | BD-216, BD-217, BD-134 | Gate record PASS |
| BD-219 | Metal-accounting subledger | BD-147, BD-026 | Exact balances |
| BD-220 | Domain-pack NRV and stockpile costing slice | BD-219 | Known answers |
| BD-221 | Synthetic FA book reproduces AISC variants to $0.01/oz | BD-061, BD-134 | GATE-BULL-02 |
| BD-222 | Reserves register, CompetentPersonAct, roll-forward | BD-040, BD-084 | PO-HALL-2 |
| BD-223 | Classification support pack (no class labels) | BD-222, BD-174 | Pack renders; G-HALL checks |
| BD-224 | Independent QP replay harness from the snapshot triple | BD-223, BD-140 | Harness ready; GATE-HALL-02 closes with GATE-STRATA-G5 (BD-269) |
| BD-264 | GATE-CRUC-03: heap inventory posterior and monthly P(inventory < book) to Bullion | BD-183, BD-220, BD-290 | Posterior narrows with leach-curve data; monthly report generated |
| BD-265 | GATE-REPORT-01: board-pack report regenerates byte-identically from its anchor | BD-124, BD-221 | Gate record PASS (M3) |
| BD-293 | `StatementRow` with per-row point of reference, route and metal stage; statement attributes; two-significant-digit render test | BD-222 | Pueblo Viejo 2026 table matched from unrounded rows |
| BD-294 | Bitemporal `OwnershipRef` (holder kinds, funded/free-carried, control vs interest) and `Tenure` claims with G-HALL-6 | BD-222 | Veladero 2017, Porgera and Reko Diq fixtures; Pueblo Viejo and Kibali tenure flags |
| BD-295 | Stockpile metallurgical state with a degradation model; NRV at reclaim-time recovery | BD-220, BD-176 | Synthetic degrading pile: NRV moves with reclaim date |
| BD-335 | Issuer reserve rows: public reserves as `IssuerReserveRow` with price deck and filing vintage keys; filed rows win over decks (§24.3) | BD-070 | E692 and E698 both kept as discrepancy items; no public row enters `EstimateStatement` |
| BD-342 | Guidance completion test: required remaining production and AISC numerator from year-to-date actuals, tested against the site's own quarterly envelope and its technical-report nameplate | BD-341, BD-340 | North Mara H1 2026 (E587) reproduced; seasonality labelled; every result names its envelope tier |
| BD-346 | JV mirror: one asset as reported by both partners (NGM and Pueblo Viejo with Newmont, Kibali with AngloGold Ashanti); consistent, explained, conflict or refused per metric at the coarsest common perimeter | BD-340, BD-332, BD-273 | Production mirrors for FY2023–FY2025 reproduce both filings' cells |
| BD-355 | Comparability engine: per-issuer definition rows from each issuer's own non-GAAP reconciliation; rank intervals; checks of comparative claims; no bridge transferred between assets | BD-346 | Synthetic claim set classified correctly; refusals name the missing definition |

### WP-14 · Helm (epics)

| Bead | Title | Depends on | Done when |
|---|---|---|---|
| BD-228 | Versioned price decks and fiscal terms (NSR, royalties, tax) | BD-026, BD-046 | Known answers |
| BD-229 | `CapitalCandidate` and the CapitalProgram optimization | BD-225, BD-228 | GB-30 reproduced |
| BD-230 | Bitemporal hindcast of ≥ 5 past approvals | BD-229, BD-053 | GATE-CAP-04 |
| BD-296 | `CountryRegime` rows with instrument and as-of date; `Restricted` and `NOT_PUBLISHED` states; price-contingent fiscal terms | BD-228 | Kibali and Loulo rows reproduce; the $85.5/oz deck mix is flagged |
| BD-297 | `PermitGate` on `StageGate`: statutory clocks, deemed approval, reopen thresholds | BD-229 | Early-works real-option card renders |
| BD-314 | `CapitalEntityGraph`: parent and carve-out denominators, NGM 61.5/38.5 and PV 60/40 funding and dilution, Fourmile vend-in, Kibali distributions and fiscal terms, Twiga 50/50 true-up, Reko Diq DSCR locks; Loulo-Gounkoto 2026 and Tongon exit fixtures (§23.2–23.4) | BD-229, BD-296 | Dual-denominator ΔNAV/share and JV waterfall tests pass on the Barrick portfolio fixtures |
| BD-353 | Compensation-formula exposure: exact piecewise evaluator of a scheme formula over admitted metric definitions and rounding intervals; synthetic first, then one publicly disclosed formula; no personal data | BD-340, BD-061 | Every threshold boundary enumerated; a range straddling a threshold is never reported as a point |
| BD-354 | Insurance recovery evidence: conditional indemnity over approved terms (waiting period, deductible, limit, indemnity period); economic loss, policy-eligible loss and recovery kept apart | BD-226 | Synthetic known answers; no coverage judgment emitted |

### WP-15 · Lodestar (epics)

| Bead | Title | Depends on | Done when |
|---|---|---|---|
| BD-231 | Deposits, effort and absence classes (§11) | BD-040, BD-033 | Data model tests |
| BD-232 | NUTS on frankenjax with diagnostics | BD-020 | R-hat/ESS certificates |
| BD-233 | Thinned marked Cox process on SPDE/GMRF | BD-231, BD-232, BD-145 | GATE-LODE-P6 scale |
| BD-234 | Synthetic-district coverage and SBC | BD-233, BD-144 | GATE-LODE-P1 |
| BD-235 | USGS three-part parity vs MapMark4 | BD-231, BD-136 | GATE-LODE-P2 |
| BD-315 | Pad/parent/wedge bundle greedy with a data-dependent upper bound (`PortfolioCertificate`, PO-LODE-15) and the Fourmile double- vs single-refractory VOI split (§11.5(E), §11.7) | BD-231, BD-233 | Upper bound ≥ brute-force optimum on synthetic pad trees; Fourmile fixtures built from E656–E658 |
| BD-350 | Target disclosure checks: remainder-grade arithmetic on drill releases, true-width and cut-off flags; one developer cash-flow replay with discounting-convention discovery | BD-340 | Synthetic juniors with seeded selective reporting caught; one public replay reproduces its NPV |

### WP-16 · Strata (epics)

| Bead | Title | Depends on | Done when |
|---|---|---|---|
| BD-236 | INT-FSIM-001 separate-clone spike | BD-015 | GATE-STRATA-G0 recorded (expected FAIL) |
| BD-237 | Variography with fitted models | BD-166 | Known answers |
| BD-238 | `DeclaredOk` kriging parity | BD-237, BD-145 | GATE-STRATA-G1 |
| BD-239 | SGS and turning-bands simulation on keyed streams | BD-238, BD-020 | BitwisePortable fixtures |
| BD-240 | Regenerate-don't-store bricks | BD-239, BD-191 | GATE-STRATA-G2 |
| BD-241 | Sealed geomaterial corpus in matdb-manifest format | BD-040 | Corpus sealed |
| BD-268 | GATE-STRATA-G3: synthetic-truth calibration (80% panel-metal intervals cover 0.72–0.88 over ≥ 50 panels) | BD-240, BD-128, BD-144 | Gate record PASS or the G3 fallback |
| BD-269 | GATE-STRATA-G5 = GATE-HALL-02: independent QP replay from the snapshot triple in ≤ 1 day | BD-224, BD-240 | Gate record PASS, or revert to the declared-estimator authority |
| BD-298 | Typed slope in `ValidationStats` (regression vs RMA, ρ, variance ratio, grade–tonnage at cut-off); `Estimator::Luc` validity domain | BD-238 | Lint refuses RMA-vs-regression comparisons; the 1×1 m synthetic case is flagged |
| BD-299 | Per-domain slope, ρ and cut-off tonnage against grade control at the pilot (GB-11 data gate) | BD-298, BD-204 | Measured values per domain, or the gate recorded as unmet |
| BD-300 | Geomet coverage report against the SMU distribution (§17.4) | BD-240, BD-176 | Coverage bands rendered; cards outside them are `OutsideValidity` |
| BD-316 | Physical-space regularization before normal-score, ILR or PPMT back-transforms (G-STRATA-11) and multi-support kriging with heteroscedastic error for joint DD and BH conditioning (§13.4, §13.7) | BD-238, BD-240 | G-STRATA-11 refuses transform-space averaging; joint DD+BH kriging passes synthetic mass-conservation tests |
| BD-317 | Soft-contact blending on domain-free background Philox streams (`"strata_bg"`, …) that keeps common random numbers and unit variance across L1 contact moves (§13.5, GB-12) | BD-239, BD-240 | A 5 m contact move keeps paired realization correlation ρ ≥ 0.90 and unit variance across the blend |
| BD-351 | Untested-ore decision bounds: two strata, one route, exact weighted bounds and a stable or unstable choice; additivity stated as a validity condition | BD-300 | Known answers on synthetic strata; refuses when blending interactions are declared |

### WP-17 · Multi-site operations (epics)

| Bead | Title | Depends on | Done when |
|---|---|---|---|
| BD-242 | Telemetry segment log, backpressure and rollups | BD-090, BD-078 | Continuity receipts tested |
| BD-243 | 10^5 msg/s for 1 h | BD-242 | GATE-INGEST-01 or KILL-5 |
| BD-244 | `fmine-mcp` clusters and the parity gate | BD-113 | INT-FMCP-001 |
| BD-245 | `fmine-tui` ops console | BD-119 | INT-FTUI-001 |
| BD-267 | GATE-SYNC-02: 7-day partition catch-up across ≥ 2 sites in ≤ 2 h at 10 Mbit/s and 50% availability | BD-109, BD-212 | Gate record PASS (M5) |

### WP-18 · Freeboard, Highwall, Muster (epics)

| Bead | Title | Depends on | Done when |
|---|---|---|---|
| BD-246 | B6 discriminating experiments T6-1 (AHS token DES vs smooth haulage) and T6-2 (N-tank RTD vs shift pairing), and guards G-CRUC-6, G-FREE-11 and G-MUST-6 (§16–§19, §36.4) | BD-212 | T6-1 and T6-2 harnesses pass; GB-40 to GB-43 reproduced from their fixtures |
| BD-247 | GISTM facility × requirement matrix | BD-040, BD-246 | GATE-FREE-01 |
| BD-248 | TARP automaton with staleness (three-valued) | BD-040, BD-246 | GATE-HIGH-02 |
| BD-249 | CCV evidence state for ≥ 3 MUEs | BD-040, BD-246 | GATE-MUST-02 |
| BD-301 | `RequirementSet` beyond GISTM, `EsCommitment` register, `OB-COMMIT`, G-FREE-7…10 | BD-247, BD-089 | Reko Diq Early Works commitments loaded; G-FREE-7/8 demo cases fire |
| BD-302 | `TailingsDisposalMethod` incl. riverine; heap/VLF facility class; gate and valve positions as stale-able evidence; NULL consequence class | BD-247, BD-248 | Veladero trigger-limit and stale-gate fixtures render evidence states |
| BD-303 | Environmental certificate reconciliation, chemical-basis limits, groundwater TARP data gate, abstraction water-balance nodes | BD-171, BD-277, BD-248 | Reko Diq certificate vs ESIA discrepancies filed |
| BD-304 | Closure measurands side by side, hazardous-inventory completeness check, tailings capacity runway | BD-246 | Carlin, Pueblo Viejo and Veladero fixtures render |
| BD-305 | Highwall acceptance policy (`dac_basis`), card rules (water basis, Q floor, UCS(Is50) k), antecedent-rainfall triggers, planar-slide metamorphic test | BD-248, BD-246 | GATE-HIGH-01 metamorphic clause passes; water double-count lint fires |
| BD-318 | FoS pore-pressure sensitivity J_u (implicit Bishop form), TSF pore-pressure ratio B̄, coupled radar–piezometer precursor and PAG co-disposal storage balance (§14.4–14.5, §18.3) | BD-248, BD-246 | J_u matches finite differences to 10⁻⁶ relative; B̄ TARP fixture; Naranjo PAG storage fixture (E667) |

### WP-19 · Pushback (epics)

| Bead | Title | Depends on | Done when |
|---|---|---|---|
| BD-250 | `pitflow` pseudoflow on i64 CSR | BD-136 | GATE-PUSH-PIT1 |
| BD-251 | PDLP first-order LP | BD-145 | KILL-PUSH-PDLP decision |
| BD-252 | Ore-mix blend LP and Lane cut-offs | BD-251, BD-139 | GATE-PUSH-BLEND |
| BD-253 | Three-gap PlanCertificate | BD-250, BD-251, BD-049 | Certificates verify offline |
| BD-266 | INT-FNX-001: precedence-graph scale benchmark (10^6–10^7 arcs) at the fgdb-pinned fnx | BD-250 | Benchmark record, or in-house graph code in `fmine-optimize` |
| BD-270 | Stochastic LOM evaluation on Strata realizations calibrated by M2–M5 reconciliation | BD-253, BD-240, BD-161 | Value of the stochastic solution with CIs (M6 acceptance) |
| BD-306 | Blend windows as per-block penalties or schedule caps; fixed-point cut-off with R(g); dilution carrying deleterious attributes | BD-252 | PlanCertificate records the blend form; TRUG dilution fixture |
| BD-319 | Two-sided refractory feed windows and autoclave campaign modes (Carlin Tables 13-7 to 13-16), the Four Corners affine recovery coefficient, the curvature correction, the Pueblo Viejo route split with the Table 15-4 cost card, and inter-complex haulage and toll arcs (§15.6) | BD-252, BD-306 | Carlin and Pueblo Viejo fixtures reproduce the printed tables (Table 15-8: 14.76 US$/t at S = 0.0904) |

### WP-20 · Headframe twin of record (epics)

| Bead | Title | Depends on | Done when |
|---|---|---|---|
| BD-254 | As-built survey epochs and surface-to-surface volumes | BD-193, BD-196 | Volume known answers |
| BD-255 | Load-and-haul DES calibrated to shift production | BD-254, BD-242 | GATE-HEAD-02 |
| BD-256 | Read-only glTF-subset scene export and viewer | BD-191 | Golden scenes |
| BD-307 | Hoist availability/utilisation/fill factors and upgrade dependencies; `FillStrength(t)`; `DilutionMatrix`; `adopted_standard` | BD-254 | Kibali hoist arithmetic reproduced; degraded-fill fixture |
| BD-320 | AHS reservation-token and geo-fence DES (Komatsu FrontRunner), the G-HEAD-7 uncured-fill refusal (FMINE-REF-504) and the `EnergyEnvelope` power co-optimizer (§16.3–16.4, §16.8) | BD-255, BD-307 | AHS DES reproduces GB-40; FMINE-REF-504 fixture; Lumwana and Kibali `EnergyEnvelope` fixtures built from E668–E669 |

---

# APPENDICES

## Appendix A · Glossary

Domain terms are defined as this plan uses them. Where industry usage varies by site, the plan treats the definition
as registry data (for example F-factors, §20.4), and this glossary gives the default.

| Term | Meaning in this plan |
|---|---|
| AIC | All-in costs (WGC 2013/2018): AISC plus non-sustaining capital and other growth costs (§21.6) |
| AISC | All-in sustaining costs (WGC): cost of sales components, sustaining capital, G&A, exploration and reclamation for current operations, per ounce *sold*, with a by-product or co-product basis (§21.6) |
| Anchor | An immutable, content-addressed publication point of the authority ledger. Every published number cites one (§26) |
| ARD / PAG / NAF | Acid rock drainage; potentially acid-forming; non-acid-forming. NAPP = 30.6·S − ANC (kg H₂SO₄/t); NPR = ANC/MPA (§18.5, GB-22) |
| ARO | Asset retirement obligation: the closure provision under IAS 37 / ASC 410-20 (§18.6, §21) |
| Attestation | A signed human act (CP, QP, EoR, controller, ITRB). It changes a Claim's `authority` field, never its evidence class (NL-3). Software never attests (INV-002) |
| Basis | The declared accounting of a quantity: wet/dry, 100%/attributable, contained/recoverable/recovered, metal stage, by-/co-product (§25) |
| Bead (BD-) | One unit of planned work, mapped one-to-one onto a `br` issue (§39) |
| Bench / berm / inter-ramp | Open-pit slope geometry at increasing scale; acceptance criteria differ by scale and consequence (§14.2) |
| BZ | Bienstock–Zuckerberg decomposition for precedence-constrained production scheduling LPs (§15) |
| CAS | Content-addressed storage: 4 MiB chunks, fanout-16 Merkle manifests, root published last (§26) |
| CCV | Critical-control verification (ICMM critical control management) (§19) |
| CIL / CIP | Carbon-in-leach / carbon-in-pulp gold recovery (§17.3) |
| Claim | The universal output object: value, support, basis, evidence class, calibration flag, UU term and provenance (§8) |
| CP / QP | Competent Person (JORC) / Qualified Person (NI 43-101, S-K 1300). Signs disclosure; software prepares support only (§24) |
| CRM | Certified reference material inserted into assay batches (§12.4) |
| CRN | Common random numbers: the same realization index across alternatives, so differences are not sampling noise (§9.5, GB-12) |
| Cut-off grade | The grade separating destinations; Lane's three limiting cut-offs (mine, mill, market) and their balancing grades (§15.6) |
| DecisionCard | The only executive output format: recommendation, distribution, evidence classes, flip margins, VOI and the human decision recorded (§22) |
| Declared estimator | The QP's estimator (e.g. ordinary kriging) reproduced bit-for-bit as `Estimator::DeclaredOk`, a projection beside the belief ledger (§13.1) |
| Desurvey | Converting collar plus downhole survey into 3-D interval positions (§12.2) |
| Dilution / ore loss | Waste included in ore, and ore sent to waste, at mining (§15.7, §16.3) |
| e-process | An anytime-valid test statistic (supermartingale under the null); alarms can be checked continuously without inflating error (§10.4, §12.4) |
| Evidence class | How something is known: `Measured`, `Derived`, `ModelledCalibrated`, `ModelledUncalibrated`, `Elicited`, `Transferred{aoa}`, `Assumed`, `Assertion` (§7.1). Attestation is not an evidence class |
| EVSI / VOI | Expected value of sample information / value of information (§22) |
| F1 / F2 / F3 | Reconciliation factors. Defaults: F1 = GC model ÷ depleted resource model; F2 = mill received ÷ mine delivered; F3 = F1 × F2. Site definitions differ and are registry rows (§20.4) |
| FA | franken_accounting, the book of record, reached only through the `fa --json` subprocess (§21) |
| FIFO | Fly-in fly-out rosters such as 14/7 and 8/6 (§19, GB-23) |
| Flip margin | How far an input, or the unmodelled-UU budget, must move before the recommendation changes; a UU form (§9.3) and a field of every DecisionCard (§22) |
| FMT- | A durable binary format with a magic number, version and CRC; formats live in S0 (§26) |
| GEO | Gold-equivalent ounces: other metals converted at a declared, versioned price ratio (§21.6, GB-27) |
| GISTM | Global Industry Standard on Tailings Management: 15 principles, 77 requirements, consequence classes (§18.2) |
| Grade control (GC) | Short-range sampling and modelling that decides ore/waste at the dig face (§16.3, §20) |
| Heap leach | Ore stacked on a lined pad and irrigated with solution; the heap holds an inventory that is a latent state (§17, GB-20) |
| HumanAuthorityToken | The token required to release a prepared effect with safety or legal consequence; agents cannot mint it (§31.3) |
| Indeterminate | The third value of a trigger: evidence is stale, missing or conflicted. It escalates like Triggered (§14.4) |
| KK / KU / UU | Known knowns; known unknowns (`KuKind`: aleatory, parametric, structural over a hypothesis set); unknown unknowns as an unmodelled budget, never zero (§9) |
| Kriging / OK | Best linear unbiased spatial estimation; ordinary kriging is the usual declared estimator (§13) |
| LabRuntime | asupersync's deterministic virtual-time runtime used for every concurrency test (§26, §33) |
| LGCP / Cox process | Log-Gaussian Cox process: the regional deposit-intensity model, thinned by exploration effort and detectability (§11) |
| LHD | Load-haul-dump underground loader (§16.4) |
| LMDI | Logarithmic mean Divisia index: an exact, order-independent decomposition used for AISC waterfalls (GB-26) |
| LUC | Localised Uniform Conditioning (Abzalov 2006): localizes uniform-conditioning panel distributions to SMUs (§13.1) |
| MCF | Mine call factor: metal accounted for at the plant over metal called by grade control; definitions vary by site (§20.4) |
| Metal stage | `MetalStage`: in situ, mined, contained, recoverable (with its recovery card), recovered, payable, sold (§25.2) |
| MNPI | Material non-public information (e.g. drill results before release); walled by compiled caveats (§31.1) |
| MOR | Modes-of-reasoning analysis: the neutral case file and evidence ids that the Brenner passes argued from (§0.6) |
| NI 43-101 / JORC / S-K 1300 | Canadian, Australasian and US disclosure codes for mineral resources and reserves (§24) |
| NRV | Net realizable value test for inventory (IAS 2); price basis set per inventory class (§21.5) |
| Obligation (OB-) | A runtime duty with a deadline and an owner (e.g. OB-TARP, OB-SEAL, OB-COMMIT) that must reach a terminal state (§26.7) |
| P754 | AMIRA P754 code of practice for metal accounting (§17.5) |
| PoF / FoS | Probability of failure; factor of safety (§14.2, GB-39) |
| POX | Pressure oxidation (autoclave) pre-treatment of refractory sulphide gold ore (§17) |
| Preg-robbing | Adsorption of dissolved gold onto carbonaceous ore; reported as a percentage from a spike test (§17.3) |
| Projection | A derived, rebuildable view (fsqlite, fgdb, frankensearch). Never an authority (§27.4) |
| Pseudoflow | Hochbaum's max-flow algorithm used for ultimate pit limits; MineFlow is the parity oracle (§15) |
| Reference class | A set of comparable past cases used to set a prior (e.g. capex overruns, GB-32); registered as `REFCLASS-` rows (§9.3) |
| Refusal | A typed `{code, what, fix}` outcome instead of a clamped or fabricated number (§8.4) |
| Region (asupersync) | A structured-concurrency scope owning tasks, budgets and obligations; closes cleanly or reports why not (§26) |
| SAA | Sample-average approximation: optimizing over a finite scenario sample, with a stated sampling gap (§15) |
| SMU | Selective mining unit: the smallest volume that can be mined selectively (§13.4) |
| Spork | asupersync's supervision framework (§26) |
| Support | The physical size and shape a value refers to (core interval, block, SMU, panel, stockpile) (§13.4) |
| SX-EW | Solvent extraction and electrowinning of copper cathode (§17) |
| TARP | Trigger action response plan: registered levels, triggers and required responses (§14.4) |
| TCC | Total cash costs (non-GAAP; Barrick definition per its endnotes) (§21.6) |
| TML | Transportable moisture limit for concentrate shipping under the IMSBC Code (§17.6) |
| TSF | Tailings storage facility (§18) |
| Twin of record | The read-only, anchor-pinned operations model that other agents can cite (§16, M6) |
| Walls | MNPI, venture (JV) and residency boundaries compiled into every query (§31) |

## Appendix B · Brenner verdict register

The six Brenner passes argued from the MOR case file (`planning/mor/MOR_CASE.md`, pivotal questions PQ-1 to PQ-7 plus
B6-SQ1 to B6-SQ4). Each verdict survived a refuter-first attempt and a forced third alternative. Sources:
`planning/brenner/B1–B6`.

| Pass · question | Hypothesis | Verdict | Where it lives |
|---|---|---|---|
| B1 · PQ-1 subsurface representation | Kriged block model as the authority | Killed as authority; survives as the `DeclaredOk` projection with bit-for-bit parity | §13.1 |
| B1 | Ensemble of conditional simulations as the authority | Downgraded: realizations are the exchange currency, regenerated, never stored as truth | §13.1, §13.5 |
| B1 | Generative hierarchical belief object | Survives, amended: staged ("cut") inference, hypothesis set plus catch-all, procedure certificates | §13.1–13.3 |
| B1 | Event-sourced data plus recipes only | Survives, merged as the storage and provenance substrate | §13.1, §26 |
| B1 | Support-indexed authorities glued by restriction maps | Downgraded to the reconciliation consistency layer | §13.1, §20 |
| B1 | QP wireframes as the authority | Downgraded to `AttestedInterpretation` inputs with a contact-uncertainty model | §13.1 |
| B2 · PQ-2 prospectivity | (a) Pixel-wise ML as the object of record | Refuted as the object of record; kept as a covariate | §11.2 |
| B2 | (b) Hierarchical marked, thinned Cox process | Survives as the regional object of record | §11.2–11.5 |
| B2 | (c) Mineral-systems model standalone | Refuted standalone; survives as the factorization of log-intensity | §11.2 |
| B2 | (d1) Search-theoretic POMDP | Survives at prospect scale only (phase 2) | §11.2 |
| B2 | (d2) Hybrid (a)+(c) → (b) → (d1) | Adopted architecture | §11.2 |
| B2 | N0 MapMark4-degenerate tract model | Checked null; parity oracle and deterministic fallback | §11.2 |
| B2 · PQ-5 epistemics | Single global posterior / credal sets as stored belief | Both refuted as the stored belief; the layered epistemic stack is adopted (Bayes inside models, ε-contamination envelope for irreversible actions, Manski bounds) | §9.2 |
| B3 · PQ-3 planning | H3a Deterministic chain | Rejected as the decision basis; retained as the parity baseline in every `PlanCertificate` | §15.1 |
| B3 | H3b Simultaneous stochastic optimization (COSMO-style) | Conditional adopt, only through the three-arm out-of-sample VSS gate | §15.1, GB-14 |
| B3 | H3c Information acquisition as a decision | Adopt narrowly on discrete commitments | §15.1, §22 |
| B3 | H3d Hierarchical decomposition with certified gaps | Adopt as the architecture | §15.1, GB-15 |
| B3 | H3e Policy search over a frozen skeleton | Adopt for tactical policies | §15.1 |
| B3 · PQ-7 capital | H7a NPV/IRR plus hurdle | Rejected as the rule; retained as a reporting column | §23.1 |
| B3 | H7b Mean-CVaR over shared scenarios | Adopted as the risk layer, always with selection frequency | §23.1, GB-30 |
| B3 | H7c Real options plus Gittins/bandits | Staged-option valuation adopted; Gittins and regret guarantees rejected | §23.1 |
| B3 | H7d Per-share two-stage `CapitalProgram` | Adopted as the decision object | §23.1–23.3 |
| B4 · PQ-4 SQ1 authority | H1-A fgdb as authority | Falsified for 3 of 5 contexts | §26.1 |
| B4 | H1-C fsqlite as relational authority | Falsified as authority | §26.1 |
| B4 | H1-B One FM-owned mechanism; everything else projects | Survives, conditional on GATE-STORE-01 (KILL-4 fallback) | §26.1 |
| B4 · SQ2 topology | H2-A central / H2-B site-sovereign / H2-C hybrid per-context cells | A and B falsified; H2-C survives | §26.2–26.4 |
| B4 · SQ3 frankensim | Path/git dependency vs reimplementation vs lift | Lift-only in every production posture | §27.1 |
| B4 · SQ4 FA contract | Direct library use vs subprocess | `fad` plus `fa --json` subprocess from one bridge crate with a signed intent outbox | §21.3, §27.2 |
| B4 · SQ5 workspace | `fm-` vs `fmine-`; crate map | `fmine-` prefix (verified collisions); B4's 39 crates grown to 59 in §28.3 | §28 |
| B4 · SQ6 security | Capabilities alone / perimeter only / layered | Layered security adopted | §31 |
| B4 · SQ7 Avathon | Replace / feed blindly / anchor-pinned system of record | Verdict C: FrankenMining is the system of record; Avathon is a cognition-plane consumer under a five-point contract | §32.3 |
| B5 · PQ-6 wedge | H-a Exploration and capital first | Deferred to M4 (second wedge) | §37.1 |
| B5 | H-b Reconciliation and metal accounting as system of record first | Rejected as system of record; kept as assurance | §37.1 |
| B5 | H-c Full operating twin first | Rejected for 24 months; becomes the M6 twin of record | §37.1 |
| B5 | H-d Evidence spine plus thin slice | Mandatory but narrowed | §37.1 |
| B5 | H-e DecisionCards | Adopted as the delivery format | §37.1 |
| B5 | **H-f Guidance & Reserve Assurance** | **Recommended wedge** (GB-38) | §37.1–37.2 |
| B5 | H-g TSF evidence state or CCV first | Deferred to M5 | §37.1 |
| B6 · B6-SQ1 Headframe | H6.1a Smooth cycle-time curves + match factor + static diesel/grid power | Killed as operational authority; kept as `PO-HEAD-2` analytic ceiling | §16.4, GB-17 |
| B6 | H6.1b Real-time closed-loop dispatch twin writing to FMS/AHS/PLC | Rejected permanently (`INV-002`, `NONGOAL-002`, `G-HEAD-1`) | §16.1, §31.3 |
| B6 | **H6.1c AHS intersection-token & safety-bubble DES + `EnergyEnvelope` microgrid co-optimizer + `G-HEAD-7` uncured-fill refusal** | **Adopted as the operational simulation & scheduling-coupling architecture** | §16.3–16.4, §16.8, GB-40 |
| B6 | H6.1d (forced third) Event-sourced state-transition twin + MWD/blast-movement spatial assimilation | Adopted and merged with H6.1c as the site authority substrate (`CELL-TEL`, `CELL-FLOW`) | §16.2–16.3, §26.8 |
| B6 · B6-SQ2 Crucible | H6.2a Static domain-average or shift-by-shift steady-state regression | Killed as fitting/reconciliation model; kept as `R0` published-card reproduction mode | §17.3, GB-41 |
| B6 | **H6.2b Four-layer versioned `CorrelationCard`s + N-tank RTD deconvolution + curvature correction + two-sided stoichiometric blend windows** | **Adopted as the plant prediction & planning-coupling architecture** | §15.6, §17.3, GB-41 |
| B6 | H6.2c First-principles multiphase CFD / population-balance digital twin | Rejected for v1 (unidentifiable parameters, violates `G-CRUC-5` when uncalibrated) | §17.1, §17.8 |
| B6 | **H6.2d (forced third) AMIRA P754 check-in/check-out bilinear state estimation + 3D heap/stockpile voxel twin (`G-CRUC-6`)** | **Adopted and merged with H6.2b as the plant accounting authority** | §17.5, §20.8 |
| B6 · B6-SQ3 Freeboard | H6.3a Compliance dashboard ("safe / conformant / green-light") | Rejected permanently (`INV-002`, `G-FREE-1`, `G-FREE-2`) | §18.1, §18.7 |
| B6 | **H6.3b Generalized `RequirementSet` traceability + `EsCommitment` (`OB-COMMIT`) + three-valued `OB-TARP` + environmental certificate reconciliation** | **Adopted as the governance & obligation substrate** | §18.2–18.4 |
| B6 | H6.3c Autonomous 3D coupled hydro-mechanical Biot/liquefaction FEM as sole authority | Rejected as sole authority (`NONGOAL-007`, `INV-002`); external EoR models enter as attested claims | §18.3 |
| B6 | **H6.3d (forced third) Coupled physical-financial twin (B̄ = Δu/Δσ_v, Naranjo PAG co-disposal shadow pricing, three-measurand ARO completeness `G-FREE-11`)** | **Adopted and merged with H6.3b** | §18.3, §18.6, GB-42 |
| B6 · B6-SQ4 Muster | H6.4a Individual operator surveillance leaderboard + soft fatigue penalties | Rejected permanently (`G-PUSH-1`, `G-MUST-1..5`, W3 residency statutes) | §19.1, §19.8 |
| B6 | **H6.4b Privacy-by-design pseudonymous crew analytics + hard hours/statutory/competency constraints + covariate-adjusted partial pooling + `G-MUST-6` anti-Goodhart CCV monitor** | **Adopted as the workforce & safety-verification architecture** | §19.1–19.6, GB-43 |
| B6 | H6.4c Unadjusted raw shift-tonnage ratios and raw CCV completion counts | Killed as decision metrics (confounded by geology/haul and Goodharted under pay incentives) | §19.5–19.6, GB-43 |
| B6 | **H6.4d (forced third) Registered causal DiD / staggered-rollout attribution + exact LMDI labour variance bridge (`PO-MUST-3`) + denominator-typed local content** | **Adopted and merged with H6.4b** | §19.3, §19.6–19.7 |

## Appendix C · Idea-wizard: 30 ideas → 5 → 15

The idea-wizard pass generated 30 ideas against the brief, thought each through (mechanism, user perception,
implementation, failure mode), and winnowed them. Every survivor is already designed into the plan; the table says
where. The ranking criterion was *obviously accretive and pragmatic for a Tier-One miner's top management*, not
mathematical interest.

**The 30.** (1) registered forecast ledger frozen before resolution; (2) reconciliation as joint state estimation;
(3) public-lane mirror of the issuer's own disclosures; (4) DecisionCards with flip margins; (5) declared-estimator
parity plus regenerate-don't-store realizations; (6) discrepancy register for published numbers; (7) per-lot
moisture as a first-class quantity; (8) common random numbers across alternatives; (9) three-gap plan certificate;
(10) typed refusals instead of clamps; (11) TSF evidence-state map that never says "safe"; (12) three-valued TARP
automaton; (13) retrospective DecisionCards built as known at decision time; (14) staged earn-in deal engine for
juniors; (15) reference-class capex prior with a blowout regime; (16) synthetic-truth mines with seeded biases;
(17) robot-first CLI with four false flags; (18) offline verifier for auditors; (19) MNPI and venture walls compiled
into queries; (20) thinned Cox-process prospectivity with effort accounting; (21) Bayesian search after dry holes;
(22) exact LMDI AISC waterfall; (23) GEO with versioned price decks; (24) site-sovereign telemetry segment log;
(25) commitments register generalizing GISTM to ESIA and permits; (26) per-site F-factor definitions registry;
(27) heap-leach recoverable-book known answer; (28) executive epistemic one-pager; (29) VOI on every card;
(30) read-only twin-of-record API for external agents.

**The best five, best first.**

1. **The registered forecast ledger (§10; BD-047–BD-049, BD-067–BD-068, BD-076).** Every forecast (resource
   tonnes and grade, plant recovery, guidance, capex) is frozen and hashed before the outcome and scored after.
   *Why it wins:* it is the only mechanism that turns "trust our model" into a track record a board, an auditor or a
   QP can check, and it costs almost nothing per forecast. Executives already live by guidance credibility; the
   ledger makes it measurable per site, per model family and per author. *Confidence:* high, because it needs no
   site data to start (M0 freezes FY2026 public forecasts before the February 2027 release) and it is falsifiable
   in public.
2. **Reconciliation as joint state estimation (§20; BD-146–BD-162).** Mine-to-mill reconciliation is solved as a
   weighted, bilinear estimation over the material-flow graph, with identifiability analysis and gross-error tests,
   instead of ratio factors. *Why:* the corpus shows published reconciliation tables with balancing rows,
   compensating errors inside one factor and cells that do not reproduce (Appendix I); a joint estimate says which
   sensor or model is biased and how sure that is (GB-24, GB-25). *Confidence:* high; the mathematics is standard
   (P754 data reconciliation) and the known-answer harness is synthetic (GATE-TOUCH-01/02).
3. **The public-lane mirror (§37 M0; BD-055–BD-076, BD-271).** Reproduce Barrick's own per-site disclosures from
   its own files, cite every cell, register every discrepancy and freeze forecasts. *Why:* it needs no data
   agreement, no site access and no integration, and it shows management something they recognize on day 90. The
   corpus pass already found unexplained differences between cited cells in public workbooks (Appendix I).
   *Confidence:* high for reproduction; medium for "nothing new" risk, which is the M0 kill criterion.
4. **DecisionCards with flip margins (§22; BD-225–BD-227).** Every executive output is a card: the recommendation,
   its distribution, evidence classes, the inputs whose movement would flip it and by how much, and the VOI of
   resolving them. *Why:* it replaces dashboards (pre-mortem F6) with decisions, and it makes uncertainty
   actionable rather than decorative. *Confidence:* high; GATE-HELM-01 tests it on two retrospective decisions.
5. **Declared-estimator parity with regenerated realizations (§13; BD-238–BD-240).** The QP's kriging is
   reproduced exactly, while simulations are regenerated on demand from seeds. *Why:* QPs sign on familiar ground,
   and storage stays bounded (GB-10). It is the bridge that lets uncertainty enter disclosure work without asking
   anyone to trust a new estimator first. *Confidence:* high for parity; the simulation layer is gated (GATE-STRATA-
   G1–G5).

**The next ten.**

6. **Discrepancy register** (M0; BD-066 and corpus fixtures): every miss is bridged with citations or marked OPEN
   with the document that would close it. It turns reproduction failures into findings.
7. **Synthetic-truth mines** (§33.3; BD-127–BD-135): the only place where truth is known, so calibration claims can
   be tested before site data exist.
8. **Offline verifier** (`fmine-verify`, BD-140): an auditor checks certificates, MMR proofs and CAS roots without
   the live system (NS-14).
9. **Three-gap plan certificate** (§15; GB-15, BD-253): optimality gap, sampling gap and calibration gap stated
   separately, so "0.1% from optimal" cannot hide a miscalibrated model.
10. **Per-lot moisture** (§25; GB-34, BD-155): a constant moisture assumption manufactures mill problems; per-lot
    moisture removes a whole class of false reconciliation alarms.
11. **Retrospective DecisionCards** (GATE-HELM-01; BD-226): rebuild two past decisions as known at the time. It is
    the fastest credibility test with executives.
12. **MNPI and venture walls** (§31; BD-093, BD-258): drill results and JV data are walled by construction; without
    this, no listed miner can deploy the system.
13. **Staged earn-in deal engine** (§23; GB-31, BD-229): it prices information rather than ounces and explains why
    an acquisition at the market price implies implausible conversion.
14. **TSF evidence-state map** (§18; BD-247 builds the GISTM facility × requirement matrix under it): what is
    monitored, what is stale and when it was last reviewed, never "safe"; aligned with GISTM traceability.
15. **Per-site F-factor and MCF definitions registry** (§20.4; corpus beads): sites define the same names
    differently (Kibali's F1 is not Carlin's), so comparisons across sites are refused until definitions match.

## Appendix D · EV matrices and the galaxy-brain card index

### D.1 Math-family EV matrix (alien-artifact selection)

Score = (Impact × Confidence) ÷ Effort, each 1–5. Families below 2.0 were rejected or deferred.

| Family | Where it compiles to artifacts | Impact | Conf. | Effort | Score | Decision |
|---|---|---:|---:|---:|---:|---|
| Bayesian decision theory with loss matrices | DecisionCards, destination classification (GB-22), VOI | 5 | 5 | 2 | 12.5 | Core |
| Proper scoring and calibration ledgers | Forecast ledger, calibration flags (§10) | 5 | 5 | 2 | 12.5 | Core |
| Data reconciliation (WLS, gross-error tests) | Touchstone joint estimation (§20) | 5 | 5 | 3 | 8.3 | Core |
| Anytime-valid sequential tests (e-processes, CUSUM) | Surprise monitors, QA/QC drift (§10, §12) | 4 | 5 | 2 | 10.0 | Core |
| Geostatistics and change of support | Strata parity and simulation (§13) | 5 | 4 | 4 | 5.0 | Core |
| Network flow and LP duality | Pit limits, schedules, Lane duals (§15) | 5 | 5 | 4 | 6.3 | Core |
| Stochastic programming (SAA, VSS) | Three-gap certificate, GB-14 | 4 | 3 | 4 | 3.0 | Gated (GATE-PUSH-STO) |
| Point processes (thinned LGCP) | Lodestar regional model (§11) | 4 | 3 | 4 | 3.0 | Adopted at M4 |
| Bayesian search theory | Dry-hole updates (GB-02) | 3 | 5 | 1 | 15.0 | Adopted |
| Submodular selection | Drill-target portfolios (§11) | 3 | 4 | 2 | 6.0 | Adopted, guarantees scoped |
| CVaR / distributionally robust choice | Capital programme, irreversible actions | 4 | 4 | 3 | 5.3 | Adopted |
| Real options (staged) | Earn-ins, study gates (§23) | 4 | 4 | 3 | 5.3 | Adopted |
| Index decompositions (LMDI) | AISC waterfalls (GB-26) | 4 | 5 | 1 | 20.0 | Adopted |
| Queueing and max-plus algebra | Fleet bounds, AHS token locks, FA posting capacity (GB-17, GB-28, GB-40) | 4 | 5 | 2 | 10.0 | Adopted |
| Residence-time deconvolution and curvature correction | `fmine-plant::rtd` N-tank convolution, recovery curvature correction (§15.6, §17.3, GB-41) | 5 | 5 | 2 | 12.5 | Adopted |
| FoS pore-pressure sensitivity and pore-pressure ratio B̄ | Real-time VWP-to-FoS propagation, contractive TSF raise TARP (§14.5, §18.3, GB-42) | 5 | 4 | 2 | 10.0 | Adopted |
| Beta-binomial / e-process anti-Goodhart verification audit | CCV tick-and-flick suppression detection (`G-MUST-6`, §19.5, GB-43) | 4 | 5 | 1 | 20.0 | Adopted |
| Merkle/MMR and CRDT-free single writers | Ledger, sync certificates (§26) | 5 | 5 | 3 | 8.3 | Core |
| Gittins indices / bandit regret bounds | Exploration budgets | 2 | 1 | 3 | 0.7 | Rejected (§23.1) |
| Credal sets / Dempster–Shafer as stored belief | Epistemic core | 2 | 2 | 3 | 1.3 | Rejected (§9.2) |
| Pixel ML as prospectivity of record | Lodestar | 2 | 2 | 2 | 1.0 | Rejected (§11.2) |

### D.2 Galaxy-brain card index

Every card states the equation, substituted values, intuition, assumptions and what would flip it. Arithmetic was
recomputed by the domain audit (Appendix H). † = corrected by the domain audit; ‡ = amended with corpus evidence
(Appendix I).

| Card | § | Title | Revised |
|---|---|---|---|
| GB-01 | 11.9 | The discovery-thinning integral | |
| GB-02 | 11.9 | Bayesian search after three dry holes | |
| GB-03 | 9.6 | Plug-in vs posterior P(≥1) | |
| GB-04 | 11.9 | EVOI of an IP survey before a six-hole program | |
| GB-05 | 11.9 | Heavy-tailed marks: never value with the median | |
| GB-07 | 11.9 | Analogue transfer and partial pooling | |
| GB-08 | 10.4 | Anytime-valid coverage alarm | |
| GB-09 | 11.9 | Novel deposit-type mass via Good–Turing | |
| GB-10 | 13.5 | Store or regenerate realizations? | |
| GB-11 | 13.1 | Why the kriged model cannot be the authority (conditional bias; slope of regression) | † ‡ |
| GB-12 | 13.5 | Common random numbers make "did the new drilling change the decision?" answerable | |
| GB-13 | 15.10 | Lane's cut-off with a stockpile option | |
| GB-14 | 15.10 | Out-of-sample value of the stochastic solution | |
| GB-15 | 15.10 | The three-gap ledger on one plan | |
| GB-16 | 15.10 | The plan number is not a forecast (schedule compliance) | † |
| GB-17 | 16.11 | How many trucks per shovel? Match factor vs queueing | |
| GB-18 | 16.11 | What is blast-movement monitoring worth on one ore blast? | |
| GB-19 | 17.10 | Push tonnes or chase recovery? (copper flotation) | |
| GB-20 | 17.10 | How much gold is really on the heap? | ‡ |
| GB-21 | 18.8 | Freeboard exceedance under climate shift (and what a pump buys) | |
| GB-22 | 18.8 | Point-estimate NAF that should be treated as PAG | † ‡ |
| GB-23 | 19.9 | FIFO 14/7 vs 8/6: cheaper per seat, but hours and fatigue are not for sale | † |
| GB-24 | 20.12 | One month of tonnes, reconciled | |
| GB-25 | 20.12 | Whose fault is the missing 9.5%? | |
| GB-26 | 21.9 | An exact, order-independent AISC waterfall | ‡ |
| GB-27 | 21.9 | GEO moves when the gold price moves | ‡ |
| GB-28 | 21.7 | Why monthly summaries, quantified (FA posting capacity) | |
| GB-29 | 22.8 | Should we buy metallurgical testwork before the final investment decision? | |
| GB-30 | 23.10 | CVaR capital allocation, and why the buyback is not a hurdle rate | |
| GB-31 | 23.10 | A junior's market price implies 93% conversion of inferred ounces | † |
| GB-32 | 23.10 | The reference-class capex prior needs a blowout regime | |
| GB-33 | 24.10 | The "±15% at 90% over a quarter" heuristic turns on one correlation | ‡ |
| GB-34 | 25.7 | A constant moisture assumption manufactures a "mill problem" | |
| GB-35 | 26.13 | Telemetry: fgdb as authority fails by arithmetic | |
| GB-36 | 26.13 | Sync over satellite, and why Merkle diffs are a checked null | |
| GB-37 | 29.8 | One ulp is harmless to a decision and fatal to a digest | |
| GB-38 | 37.2 | The wedge under data-access uncertainty | |
| GB-39 | 14.6 | A section that passes on FoS can fail on PoF once pore pressure is uncertain | ‡ |
| GB-40 | 16.11 | AHS intersection tokens vs. staffed haulage (Carlin's planned 40-truck autonomous fleet) | B6 |
| GB-41 | 17.10 | Shift-paired recovery vs. residence-time deconvolution | B6 |
| GB-42 | 18.8 | Pore-pressure ratio B̄ vs. a drained assumption on a contractive TSF raise | B6 |
| GB-43 | 19.9 | CCV completion vs. a random audit: when "0 non-conformances in 300 checks" signals suppression | B6 |

GB-06 was retired before publication; its capex-mixture content is GB-32, and references were redirected.

## Appendix E · Mathematics compendium

Equations used by the plan, with where they are used and the assumptions that bound them. Labels follow the plan:
FACT means stated in a cited source; FACT[U] means cited but not yet verified against the primary source.

| Name | Equation | Used in | Assumptions and validity |
|---|---|---|---|
| Multi-Gaussian regression (conditional bias) | E[Z \| Z* = z*] = m + b(z* − m); b = slope of regression | GB-11, §13.1 | Gaussian-type relation; b measured per domain against outcomes; report ρ and cut-off tonnage too |
| Krige's relation | σ²(SMU in deposit) = σ²(points in deposit) − σ²(points in SMU) | §13.1, §13.4 | Stationarity within domain; dispersion variances from the variogram |
| Gy fundamental sampling error | σ²_FSE = (1/M_s − 1/M_L)·K·d^α | §12.3 | K and α calibrated per ore type, valid near the calibration grade (C6) |
| WLS data reconciliation | min (y − x)ᵀV⁻¹(y − x) s.t. Ax = 0 | §20.4, P754 | Measurement covariance V known or estimated; linear balances (bilinear for tonnes × grade) |
| Global gross-error test | (y − x̂)ᵀV⁻¹(y − x̂) ~ χ²(rank A) | §20.4 | Gaussian errors; power stated (BD-150) |
| F-factors | F1 = GC/RM, F2 = Mill/GC, F3 = F1·F2 (tonnes, grade, metal) | §20.4 | Definitions are site registry data |
| Lane's limiting cut-offs | g_m = h/((p − k)y); g_h = (h + (f + F)/H)/((p − k)y); g_k = h/((p − k − (f + F)/K)y); F = δ·V | §15.6, GB-13 | Single product, fixed capacities, V iterated from the plan; the mill-hour LP dual equals (f + F)/H only when mine life is endogenous (PO-PUSH-15) |
| Ultimate pit (closure) | max Σ v_i x_i s.t. x_i ≤ x_j for precedence (i→j); solved by max-flow min-cut | §15.4 | Block values fixed per scenario; slope templates define arcs |
| Bienstock–Zuckerberg | Iterative LP over precedence-constrained schedule with aggregated columns | §15.4 | Few coupling rows per period (A-PUSH-3) |
| SAA gaps | Optimality gap; sampling gap via confidence interval; calibration gap via synthetic truth | §15.2, GB-15 | i.i.d. scenarios for the CI; calibration needs reconciliation history |
| VSS (out of sample) | VSS_oos = E[f(x_stoch)] − E[f(x_det)] on held-out scenarios | GB-14 | Calibrated scenario generator; three-arm design |
| Kuz-Ram | x50 = A·K^−0.8·Q^(1/6)·(115/RWS)^(19/30) | §16.3 | Card with per-domain learned rock factor A; the exponent is pinned to the card's cited source (FACT[U]) |
| Swebrec | P(x) = 1/{1 + [ln(x_max/x)/ln(x_max/x50)]^b} | §16.3 | Fines tail; image measurement under-counts fines |
| Site vibration law | PPV = k·(D/√W)^−β | §16.3 | Fitted per monitoring location with prediction intervals |
| Match factor | MF = (N_trucks × loader cycle)/(N_loaders × truck cycle) | §16.4, GB-17 | Deterministic; queueing model bounds it |
| Hoisting capacity | t/h = 3,600 × payload ÷ skip cycle × availability | §16.4 | Cycle includes acceleration, travel, deceleration, settling |
| Bond work index | W = 10·Wi·(1/√P80 − 1/√F80) | §17.3 | Card with validity domain per ore type |
| NAPP / NPR | NAPP = 30.6·S − ANC; NPR = ANC/(30.6·S) | §18.5, GB-22 | Sulphide-S basis; Gaussian error for P(PAG) |
| P(PAG) decision | Treat as PAG if Φ(E[NAPP]/sd) > c_FP/(c_FP + c_FN) | GB-22 | Independent S and ANC errors unless joint field |
| PoF | PoF = P(FoS < 1) over realizations | §14.2, GB-39 | Computed from realizations, not a fitted form (PO-HIGH-6) |
| Mathews stability | N′ = Q′·A·B·C vs hydraulic radius | §14.3 | Empirical card with validity domain |
| Gutenberg–Richter | log₁₀N(≥M) = a − bM | §14.3 | Catalogue completeness above M_c |
| Discovery thinning | Λ_undisc = Σ_cells Λ·e^(−H), H = Σ_j e_j·q_j (effort × sensitivity); detection π = 1 − e^(−H) | §11, GB-01 | Per-method sensitivity q by cover depth; effort data or Manski bounds |
| P(at least one deposit) | P(N ≥ 1) = 1 − E_post[exp(−Λ(A))] | §9.6, §11.5, GB-03 | Expectation outside the exponential |
| Bayesian search update | p′ = p(1 − q)/(1 − pq) after an unsuccessful search with detection q | GB-02 | Detection probability per hole calibrated |
| Good–Turing unseen mass | P(new type) ≈ n₁/N | GB-09 | Exchangeable discoveries |
| Proper scores | log score; CRPS; interval score IS_α = (u − l) + (2/α)(l − y)⁺ + (2/α)(y − u)⁺ | §10 | Strictly proper; compare like with like |
| e-process alarm | Alarm when E_t ≥ 1/α | §10.4, GB-08 | Anytime-valid under the null; α = 0.05 → 20 |
| EVSI | EVSI = E_y[max_a E(U \| a, y)] − max_a E(U \| a) | §22, GB-04, GB-29 | Decision-relevant prior; costs in the same units |
| CVaR | CVaR_α = min_η {η + E[(L − η)⁺]/(1 − α)} | §23, GB-30 | Shared scenario index across projects |
| Staged option value | V = Σ p_s·max(0, V_s − K₂)/(1 + r) − K₁ | §23, GB-31 | Branch probabilities and per-branch P_dev |
| Junior NAV | R = h_pub(c_IR·M_ind + c_FI·c_IR·M_inf); NAV = R·v·P_dev | GB-31 | Haircut and conversion priors from history |
| LMDI | ΔC = Σ_k L(C₁, C₀)·ln(x_k,1/x_k,0) with L the logarithmic mean | §21.9, GB-26 | Multiplicative identity; exact, order-independent |
| GEO | GEO = Au + Σ_m (P_m/P_Au)·Q_m | §21.6, GB-27 | Price ratio declared and versioned |
| Troy-ounce grade | 1 oz/st = 240/7 g/t ≈ 34.2857 g/t | §25 | Exact constant |
| FIFO staffing | FTE/seat = 8,760 ÷ (12·365·on-fraction) ÷ (1 − absence) | GB-23 | Equal hourly vs equal annual pay changes the result |
| Little's law | L = λW | §26.9, GB-28 | Stationary flow |
| Sequential PIT betting e-process | u_k = F̂_k(y_k \| y₁…y_{k−1}), z_k = Φ⁻¹(u_k), M_K = Π(1 + λ_k ψ(z_k)) with ψ centred under N(0, 1) | §10.4, GB-08 | Null: the registered joint model; P₀(sup M_K ≥ 2/α) ≤ α/2 per score (Ville) |
| Multi-support kriging and physical-space regularization | Σ_j λ_j (C̄(v_i, v_j) + δ_ij σ²_ε,i) + μ = C̄(v_i, V), Σλ = 1; g_V = Σ w_q ρ_q z_q / Σ w_q ρ_q | §13.4, §13.7 | Back-transform before accumulating (G-STRATA-11) |
| FoS pore-pressure sensitivity and B̄ | ∂F/∂u_k = −Σ w_ik b_i tan φ′_i / m_α,i ÷ (Σ W_i sin α_i − Σ T_i sin α_i tan φ′_i / (F² m_α,i²)); B̄ = Δu/Δσ_v | §14.5, §18.3, GB-39, GB-42 | Implicit in F through m_α (Bishop); the same step for Spencer and M–P |
| Two-sided feed windows, Four Corners recovery, curvature correction | L·Σx ≤ Σax ≤ U·Σx; T = 1.821×10⁻⁵·As + 0.127·g − 4.890×10⁻⁴ (opt); E[gR(g)] ≈ ĝR(ĝ) + ½(gR)″σ² | §15.6, §17.3 | Carlin Tables 13-7 to 13-16 (E172, E659–E661); Pueblo Viejo Table 15-4 (E664) |
| Tanks-in-series RTD and heap percolation kernels | h(τ) = αδ(τ) + (1 − α)(N/τ̄)(Nτ/τ̄)^{N−1}e^{−Nτ/τ̄}/(N − 1)!; ṁ_PLS(t) = Σ M_ℓc R∞ h_ℓc(t − t₀) | §17.3, §20.8 | Convolve in throughput-volume time when flow varies |

## Appendix F · Upstream asks and the foundation dependency ledger

Sibling repos are read-only to this program. FrankenMining's platform owner holds every `FD-` row; the fix for an
ask belongs to the sibling's maintainers, who file it as a bead in their own repo. Pins are restated from §27 for
convenience; §27 and `franken-stack.lock` are authoritative.

### F.1 Foundation ledger (one `FD-…-00` row per foundation)

The `FD-…-00` ids here are canonical. `franken-stack.lock` must use them; its current `-01` row ids collide
with the F.2 upstream-ask ids and are renamed, with the INT records and ADR-RNG-01 that cite them (R7-C162).

| FD | Foundation | Posture (§27) | Pin | Admission | Fallback that keeps the product shippable |
|---|---|---|---|---|---|
| FD-AS-00 | asupersync, plus franken-evidence/-decision/-kernel inside `fmine-decide` only | Consume | `0.5.0`, rev `78b64636e` (§27.5) | INT-AS-001, INT-AS-SEC-001, INT-ATP-001 | Stay on 0.5.0 with the §31.4 mitigations; move only with the suite (GATE-PIN-06) |
| FD-FA-00 | franken_accounting | Subprocess (`fad` + `fa --json`) | Pinned FA rev, re-read at every pin review (§27.2) | INT-FA-001 (FA-ACC-01…13) | KILL-1: monthly posting; then a local certified ledger with FA as a draft adapter |
| FD-FSIM-00 | frankensim | Lift-only | Lifted files with provenance headers and golden hashes (§27.1) | INT-FSIM-001; GATE-FSIM-RECONV before any Cargo dependency | KILL-3: stay lift-only; GATE-FSIM-DRIFT watches the lifted regions |
| FD-FSQL-00 | frankensqlite | Consume, as a projection | `v0.4.12` (§27.4) | INT-FSQL-001 | KILL-4 promotes it to persistence rung L2, never to authority |
| FD-FGDB-00 | frankengraphdb | Optional projection | Suite pin (§27.4) | INT-FGDB-001 | KILL-2: fsqlite tables plus in-memory fnx |
| FD-FSEARCH-00 | frankensearch | Consume | facade `0.7.1` | INT-FSEARCH-001 | fsqlite FTS5 |
| FD-FNX-00 | franken_networkx | Consume; `preflow_push` oracle in a dev workspace | git `9d710b1c` | INT-FNX-001 | In-house graph code in `fmine-optimize` |
| FD-FSCI-00 | frankenscipy | Consume, default features off | `=0.4.0` (§27.3) | INT-NUM-001 | Own kernels in `fmine-numerics`, with fsci as an oracle |
| FD-FP-00 | frankenpandas, without `fp-io` | Consume | git `37bb94101` | INT-NUM-001 | Own readers (BD-187) and columnar code |
| FD-FNP-00 | franken_numpy | Oracle (`fnp-random`); consume (`fnp-io`) | git `d32c4e04` | INT-NUM-001 (`fnp-io` only) | Own `.npy` header codec in `fmine-array` |
| FD-FJ-00 | frankenjax (`fj-core`, `fj-lax`, `fj-ad`) | Consume | git `fa93fb77` | INT-NUM-001 | Gradients generated at build time by `fmine-derive` for fixed model families |
| FD-FSYM-00 | frankensympy | Oracle, build time only | `9d785d0` | Pinned toolchain record (OPEN-SUITE-3) | A separate pinned toolchain for `fmine-derive` |
| FD-FMCP-00 | fastmcp | Consume, in `fmine-mcp` only | `=0.13.0` | INT-FMCP-001 | The robot CLI stays the complete surface |
| FD-FTUI-00 | frankentui | Consume, in `fmine-tui` only | `=0.9.0` | INT-FTUI-001 | CLI and reports only |
| FD-FMD-00 | franken_markdown, frankenmermaid | Consume, in `fmine-report` only | Pinned library; default features | INT-FMD-001, INT-FMERM-001 | Plain markdown rendering without diagrams |

### F.2 Upstream asks

Each row is an ask with FrankenMining's interim posture and the switch-over rule (for FA, the postures match
§21.3.6).

| FD | Repo | Ask | Priority | Interim posture | Switch-over rule |
|---|---|---|---|---|---|
| FD-FA-01 | franken_accounting | Metal and mass units (balanced and single-sided) with exact conversion laws, or a journaled `fa unit add` | P0 | Quantities travel as Statistical-account rows; FrankenMining's flow ledger is authoritative (KILL-1 (b) at month 15) | FA release passing FA-ACC-01…13 with metal units |
| FD-FA-02 | franken_accounting | Tiered NCI, multi-parent ownership, cross-book consolidation with translation and CTA (fa-76hy; V27) | P0 | Attributable figures computed in Bullion with party ownership | Consolidation passes the JV fixtures |
| FD-FA-03 | franken_accounting | Custom dimension kinds | P0 | Dimensions encoded in memo fields of summaries | Kinds registered |
| FD-FA-04 | franken_accounting | Custom REA event kinds | P0 | Domain events stay in FrankenMining; FA receives summaries | Kinds registered |
| FD-FA-05 | franken_accounting | Canon expressiveness: quantity × price, division, table inputs, folds over lots, Allocation/Revaluation outputs (V21, fa-c34) | P1 | Computed in Bullion; FA posts results | Canon rules reproduce Bullion's |
| FD-FA-06 | franken_accounting | Non-currency `Rate` documents and a commodity remeasurement law | P1 | Price decks held in FrankenMining | — |
| FD-FA-07 | franken_accounting | Difference kinds: provisional pricing, assay, TC/RC, penalty | P1 | Bullion computes; FA posts adjustments | — |
| FD-FA-08 | franken_accounting | Mining document kinds: assay certificate, survey, weighbridge ticket, bill of lading, technical report, price fixing | P1 | Documents attached as Workpaper digests | — |
| FD-FA-09 | franken_accounting | `InstrumentKind::{FixedAsset, Derivative}` and `Depreciate`/`Amortize` schedules (V37) | P1 | UoP DD&A computed in the domain pack | — |
| FD-FA-10 | franken_accounting | Surfaced `AdjustmentLayer` books; `recon.book_to_book`; reason codes as registry data | P1 | AISC adjustment layers held in Bullion (§21.6) | — |
| FD-FA-11 | franken_accounting | Allocation laws `Tiered` and `ByUnitsThenValue` | P1 | Allocations in Bullion | — |
| FD-FA-12 | franken_accounting | CloseRun DAG (V07) | P1 | Close orchestration in FrankenMining | CloseRun live |
| FD-FA-13 | franken_accounting | Decide where equity-method and proportionate-consolidation laws live | P1 | Domain pack | — |
| FD-FA-14 | franken_accounting | SoD with authenticated principals (V19) | P0 for production | Draft books only | V19 verified |
| FD-FA-15 | franken_accounting | `PostBatch` with external-subledger members (FA-ACC-13; OPEN-B4-03) | P1 | Per-entry posting | Accepted by FA |
| FD-FA-16 | franken_accounting | Explain and fix O(n) per-post growth (OPEN-BULL-2) | P0 for scale | Monthly posting, ≤ 300 publications per book (KILL-1 (a)) | FA-ACC-09 passes |
| FD-FSIM-01 | frankensim | Rock, ore, slurry, tailings, liner and Bond Wi bundles, or support for a sealed private corpus in the v7 pack schema | P1 | `fmine-materials` sealed private corpus | — |
| FD-FSIM-02 | frankensim | Mohr–Coulomb and Hoek–Brown plasticity in fs-solid | P1 | LEM only; numerical analysis as attested external results (KILL-HIGH-2) | GATE-FSIM-RECONV plus benchmark parity |
| FD-FSIM-03 | frankensim | Unconfined (Richards) seepage and coupled consolidation | P1 | `fmine-geotech::pore` quasi-3-D model | As above |
| FD-FSIM-04 | frankensim | Bingham and Herschel–Bulkley rheology in fs-lbm | P2 | Correlation cards | As above |
| FD-FSIM-05 | frankensim | Selected inversion in fs-sparse for marginal variances | P1 | Own implementation in `fmine-numerics` | Upstream parity |
| FD-FSIM-06 | frankensim | Workspace builds green (15 failing targets); remove out-of-repo `../../../asupersync` paths; stable-Rust path for fs-qty | P0 for any dependency | Lift-only (§27.1) | GATE-FSIM-RECONV |
| FD-AS-01 | asupersync | Protected third-party caveats in the default macaroon path (defect (c) is unchanged in 0.6.0) | P1 | First-party caveats only (§31.4) | Verified fix plus INT-AS-SEC-001 |
| FD-AS-02 | asupersync | Suite-wide coordinated 0.6 bump (OPEN-B4-02) | P1 | Stay on 0.5.0 with mitigations | GATE-PIN-06 |
| FD-FGDB-01 | frankengraphdb | Bump fnx so `preflow_push` (fnx 0.3.x) is available at fgdb's pin | P2 | Oracle runs in a separate dev workspace | One fnx-classes package id with `preflow_push` |
| FD-FSYM-01…07 | frankensympy | Resolvable workspace; tagged release; `CompiledOp` comparisons/Piecewise/Min/Max/floor/erf; `diff` rules for Max/Min/Abs; LaTeX for Piecewise and sums; multivariate delta method; definite integration of piecewise polynomials (R8 §10) | P2 | `fmine-derive` build-time only | Per item |
| FD-FNP-01…03 | franken_numpy | Real memory map or `read_npy_slice`; typed storage (f32, u8, bitset); documented cross-ISA determinism and a pure-Rust libm option | P2 | `fmine-array` owns grids | Per item |
| FD-FP-01…05 | frankenpandas | Interval-tree `IntervalIndex` and overlap joins; `Decimal128`; streaming `read_parquet`; non-default `sql-sqlite` and x86-scoped `fp-dot-kernel` flags; **feature-gate arrow, parquet and chrono in `fp-io`** | P2 (P1 for FD-FP-05) | `fp-io` not consumed (KILL-6 path) | `fp-io` closure passes L-5 |
| FD-FSCI-01…09 | frankenscipy | Preconditioner hook for `cg`/`minres` plus IC(0)/AMG; sparse Cholesky with log-det; CSR `linprog`, MPS reader, warm-started dual simplex; censored `fit_mle`; RBF neighbours mode; arbitrary-footprint `grey_dilation`; `least_squares` with sparse Jacobians; flat-slice overloads; KDTree benchmark at 10⁶–10⁷ | P1–P2 | Own kernels in `fmine-numerics`/`fmine-lp`; copying adapters | Per item, with oracle parity |
| FD-FJ-01…05 | frankenjax | `wasm32` build without `fj-ffi`; documented interpretation overhead and a fused path; a home for samplers; a tagged release; **`fj-api` without an unconditional `fj-ffi` dependency** | P2 | Consume `fj-core`, `fj-lax`, `fj-ad` only | Per item |

## Appendix G · Evidence register index

The register is `planning/mor/MOR_CASE.md`. It is append-only: corrections are appended rows (e.g. the E029
correction), never overwrites.

| Ids | Topic | Source |
|---|---|---|
| E001–E010 | frankensim state, liftable crates, geometry coupling, physics gaps | R1, read-only survey |
| E011–E016 | asupersync, fgdb, fsqlite, suite idioms, storage by authority shape | R2 |
| E017–E018 | Numerics coverage and gaps | R3 |
| E019–E021 | franken_accounting posture and the 17-part mining domain pack | R4 |
| E022–E023 | FrankenTextiles lessons; the four false flags and authorization refusals | R5 |
| E024–E027 | Barrick, executive pay metrics, industry economics, OEM consolidation | R6 |
| E028–E032 | Prospectivity, geostatistics and reconciliation, planning, reporting codes, epistemics | R7 (E029 and E031 carry appended corrections) |
| E033 | The FrankenSurveillance pattern | FSS, read directly |
| E034 | Newmont's NGM default notice (3 Feb 2026) and settlement (10 Aug 2026); NGM data JV-sensitive | Newmont FY2025 10-K and Barrick Q2 2026 release (W5 E628, E629); corpus E139 |
| E035 | The industry's believed proofs are single-asset uplifts (Freeport TROI at Bagdad, consultant-reported, FACT[U]); twins stall on people (Lihir) | B5 §9; W5 E643, E644 |
| E101–E149 | Barrick public statistics and site decks | `planning/corpus/C1` |
| E151–E199 | Nevada technical reports | `planning/corpus/C2` |
| E201–E249 | Africa technical reports | `planning/corpus/C3` |
| E251–E299 | Americas and PNG technical reports | `planning/corpus/C4` |
| E301–E349 | Reko Diq Early Works ESIA | `planning/corpus/C5` |
| E351–E399 | Textbooks (reference only) | `planning/corpus/C6` |
| E401–E449 | Host-state events: Mali 2023 Code and the Loulo-Gounkoto dispute, Porgera, Cobre Panamá, DRC Code 2018, Twiga | `planning/research/W2` |
| E451–E499 | Barrick Mine Stats Q1 2024–Q2 2026: retrieval, layout, discrepancies, site series | `planning/research/W1` |
| E501–E549 | Data-residency and mining-data statutes, 12 countries | `planning/research/W3` |
| E551–E599 | M2 site choice and P(site data by month 7) | `planning/research/W4` |
| E601–E649 | Primary-source verification of the plan's figures | `planning/research/W5` |
| E651–E655 | Crate-name and asupersync-pin checks made directly | `planning/mor/MOR_CASE.md` |

Status: E001–E035 were used by the Brenner passes and audited in the review rounds. For the corpus rows E101–E399,
`AUDIT_4` (a second reader) reports 273 exact, 18 confirmed with nuance and 3 page-locator fixes (E146, E211,
E212), with no substantive errors, and it re-read the 18 quarterly Mine Stats and AISC workbooks
(`planning/mor/audits/AUDIT_4_corpus_claim_verification.md`). The research rows E401–E655 were written from primary
sources by the agents that cite them and have had no second reader yet (H-4). E656–E672 record the second-reader
checks of the concurrent revision (Appendix H.5b).

## Appendix H · Audit fold-in

### H.1 Process

1. **MOR case file** (`planning/mor/MOR_CASE.md`): a neutral decision contract, E001–E033 and pivotal questions
   PQ-1 to PQ-7, built before any design argument.
2. **Brenner passes B1–B6** answered the pivotal questions across all eleven engines (Appendix B;
   `planning/brenner/B1…B6`).
3. **Fresh-eyes audits** (2026-10-28; reports in `planning/mor/audits/AUDIT_1…AUDIT_3`): three independent
   read-only reviewers who did not write the plan checked domain accuracy, suite fit (against the sibling repos'
   code at their pinned revisions) and internal consistency.
4. **Corpus pass** (2026-10-28): six read-only agents over the supplied documents (Appendix I), then a second-reader
   audit of the corpus rows and the quarterly workbooks (`planning/mor/audits/AUDIT_4_corpus_claim_verification.md`).
5. **Public-source research pass and second-wave review** (2026-10-29): five primary-source research agents
   (`planning/research/W1…W5`, Appendix J) and a review of equations, guards and the bead graph
   (`planning/mor/audits/AUDIT_5_second_wave_review.md`).
6. **Second-reader checks of the concurrent revision** (2026-10-29): two primary-source agents re-read every new
   FACT claim, and the new mathematics was checked numerically (Appendix H.5b).

### H.2 Domain-accuracy audit: 15 findings, all folded in

| # | Finding | Disposition |
|---|---|---|
| 1 | LUC misnamed; the slope ≈ 0.1 case presented as general; "conditional bias goes unreported" | Fixed: Localised Uniform Conditioning; single-case stress case; GB-11 range b = 0.8/0.5/0.1; later strengthened by the corpus (b ≈ ρ; correlation and cut-off tonnage in the flip) |
| 2 | Newmont AISC example mixed guidance and actuals | Fixed: Q3 2026 by-product $1,454/oz vs co-product $1,963/oz |
| 3 | JORC 2026 reconciliation labelled FACT | Fixed: exposure-draft content, wording and commencement unverified |
| 4 | IFRS 9 provisional pricing described as a bifurcated embedded derivative | Fixed: FVTPL as a whole under IFRS 9; bifurcation under US GAAP |
| 5 | CIL called the standard response to preg-robbing | Fixed: CIL for mild; roasting, POX with thiosulphate, or blanking for strong |
| 6 | Kuz-Ram exponent 19/20 | Fixed: 19/30 |
| 7 | GB-22 flip impossible as stated | Fixed: sulphur precision dominates; S sd ≤ 0.15% with ANC sd ≤ 4 gives P(PAG) 0.038 |
| 8 | GB-31 branch NAVs inconsistent with stated P_dev | Fixed: per-branch P_dev 0.80/0.55 restored from B3 |
| 9 | GB-23 saving mostly the salary assumption | Fixed: equal-hourly case ($53k/seat, ≈ $21M/y) shown beside the headline |
| 10 | M2 alternate site contradicts the 100%-owned preference | Fixed: Twiga 16% governance stated |
| 11 | NRV at the long-term price for all inventory | Fixed: per-inventory-class price basis |
| 12 | GB-16 counted deferred metal as lost | Fixed: production variance; deferral valued separately |
| 13 | Q1 2026 AISC rise attributed to royalties alone | Fixed: royalties and corporate G&A |
| 14 | "Most reconciliation failures are basis failures" unsupported | Fixed: "many", labelled HYPOTHESIS |
| 15 | LP dual and greedy-guarantee statements too strong | Fixed: endogenous mine life; guarantees scoped to the monotone coverage term |

Checked and sound: the arithmetic of the 28 cards the report lists, the cited formulas, planning algorithms, GISTM and TML facts,
accounting standards and the Barrick and industry figures (to be re-verified against primary filings before
executive presentation).

### H.3 Suite-fit audit: 30 claims checked, 11 problems, all folded in

| # | Problem | Disposition |
|---|---|---|
| 1 | The chrono/time ban could never pass (transitive through fsqlite, fastmcp, frankenpandas, asupersync `tls`) | Fixed: the ban applies to direct dependencies and public APIs, with a per-foundation allow-list (L-5) |
| 2 | fastmcp `=0.5.0` brings asupersync 0.4.5 | Fixed: `=0.13.0` |
| 3 | ftui 0.7.x brings asupersync 0.4.9 | Fixed: `=0.9.0` |
| 4 | fnx pin contradicted itself; `preflow_push` absent at fgdb's pin | Fixed: fnx at `9d710b1c`; the oracle runs in a dev workspace; FD-FGDB-01 |
| 5 | "frankenjax without fj-ffi" unsatisfiable through `fj-api` | Fixed: consume `fj-core`, `fj-lax`, `fj-ad`; FD-FJ-05 |
| 6 | franken-evidence/-decision in S0 broke the layer and serde rules | Fixed: S0 schema types; `fmine-decide` is the single adapter; CELL-DEC rows use S0 types |
| 7 | OPEN-SUITE-1, -2, -4 resolvable | Closed: APIs exist; Philox variants differ (golden vectors); `fp-io` excluded (KILL-6 path) |
| 8 | asupersync pin labelled as an fgdb revision | Fixed |
| 9 | S3(c) described as fixed in 0.6.0 | Fixed: unchanged in 0.6.0; first-party-only rule persists; FD-AS-01 |
| 10 | fsci "no preconditioned CG/MINRES" overstated | Fixed: ILU-preconditioned `pcg`; unpreconditioned MINRES; FD-FSCI-01 |
| 11 | Minor: winding lift imports, UFuncArray sidecar, fgdb Point index scope, FA HEAD | Fixed |

### H.4 Consistency audit: 15 findings, all folded in

| # | Finding | Disposition |
|---|---|---|
| 1 | M0–M2 deliverables without crates; phase-A crates without phase-A work | Fixed: `fmine-decide` to phase A (BD-225–227 into WP-02); `fmine-metalacct` and `fmine-fa-bridge` to phase B; phases now 35/18/6 crates, 615/392/121k |
| 2 | Pushback start M4 vs M6 | Fixed: starts M4 (month 18), acceptance M6; `fmine-optimize` and `fmine-lp` to phase B; all relative gates inside month 36 |
| 3 | §38 dependencies contradicted the bead graph (cycles) | Fixed: beads relocated (BD-080, -097, -098, -125); §38's column is generated from the bead graph and the WP graph is acyclic |
| 4 | KILL-1 defined twice | Fixed: one definition in §21.3 with two triggers; §37.3 cites it |
| 5 | Early gates without closing beads | Fixed: BD-257–BD-270 (INT-AS-001, GATE-WALL-01, GATE-PLUMB-E3, KILL-PLUMB-5/6, INT-FSEARCH-001, GATE-CRUC-02/03, GATE-REPORT-01, INT-FNX-001, GATE-SYNC-02, GATE-STRATA-G3/G5, the M6 stochastic LOM) |
| 6 | Gate milestone conflicts; Strata months collided with milestone ids | Fixed: GATE-SYNC-02 at M5; GATE-HALL-02 = GATE-STRATA-G5 in WP-16; Strata months written S+n |
| 7 | GB-31 numbers | Fixed (with H.2 #8) |
| 8 | Identifier defects (G-CAP, GATE-PUSH-BZ, KILL-PROS-1, OPEN-STRATA-3/4, GB-06, RISK-B4-10, PO-HIGH) | Fixed: GATE-CAP-01…03; KILL-PUSH-BZ; KILL-LODE-1; OPEN-STRATA-4; GB-32; RISK-16; PO-HIGH-1…6 added |
| 9 | Seven cross-references to the wrong subsection | Fixed |
| 10 | Layer rules vs `fmine-sync`/`fmine-telemetry` IO | Fixed: S1 may do ledger, CAS, telemetry and sync IO; V does all other IO |
| 11 | OPEN-item rule not met by section-level items | Fixed: inheritance rule in §36.3; missing rows added |
| 12 | K-layer ceiling sum | Fixed: 514; total 1,298 |
| 13 | Underground equipment and Highwall obligations thin | Fixed: underground development cycle, longhole/ring blasting, backfill, LHD, hoisting, dozing/ripping/scraping and demolition (§16.3–16.4); Highwall guards, PO-HIGH-1…6 and GB-39 (§14.6) |
| 14 | External criteria and ids not self-contained | Fixed: INT-AS-001 and INT-FSQL-001 criteria inlined; E023 references replaced by INV-002 |
| 15 | Minor contradictions and label collisions | Fixed: E001–E035; FTX overran its plan; CONTRACT.md ten sections; label scope note in §0.3; refusal code 201 |

### H.5 Corpus pass

84 proposed plan changes: 75 accepted in full, 8 in part, 1 validation only; 111 in-place edits; beads BD-271 to
BD-307. Details, declined items and data gates are in Appendix I.

**Appendix cross-check.** Writing these appendices against the body found two more defects, both fixed:
- **M0 coverage was overstated.** "2025Q1–2026Q2" read as six contiguous quarters, but the two workbooks hold
  2025Q1, 2025Q2, 2026Q1 and 2026Q2 plus both half-years (period headers re-read from the xlsx). 2025Q3 and 2025Q4
  joined the data gate (§21.6, §37.3; E148 correction appended in the case file).
- **The consumed numerics foundations had no admission gate**, although §27 requires one for every consumed
  foundation. INT-NUM-001 (§34.2) and BD-308 were added, and BD-041 (the first production use of fsci-stats) now
  depends on it.

### H.5b Second-wave review, corpus second read, and corrections to the concurrent revision

- **AUDIT_5** (`planning/mor/audits/AUDIT_5_second_wave_review.md`, 2026-10-29) reviewed the plan after BD-309–BD-320
  and W1–W5 and folded in: guard tables for G-CORE-1/2 (§12.6) and G-STRATA-11; FMINE-REF-108 and FMINE-REF-504 in
  §25.6 and §31.3; PO-LODE-15 naming; BD-309 edges into BD-161 and BD-312 (533 edges, 0 cycles); `fmine-time` in
  §28.3; and the B6 fold-in. Its two primary-source items in §15.6 were themselves wrong and are corrected below:
  the "Carlin exponential 0.8926·[1 − exp(−22.2995·(Au − 0.032))]" and "Cortez rational cubic 1.1536 − 0.0046/g −
  …" roaster laws appear in no corpus document (E154 and E181 are unrelated rows), acid POX recovery is not linear,
  and the Table 15-8 check works at S = 0.0904, not 0.0368.
- **AUDIT_4** (`planning/mor/audits/AUDIT_4_corpus_claim_verification.md`) re-read E101–E399 and the 18 quarterly
  workbooks: 273 exact, 18 confirmed with nuance and 3 page-locator fixes (E146, E211, E212; the MOR_CASE note of
  the same day lists E124, E134 and E239 by mistake). It also found two hidden external-link sheets in the Q1 and
  Q2 2026 AISC workbooks holding 154 `#REF!` cells.
- **Second-reader checks of the concurrent revision (2026-10-29).** Two primary-source agents re-read every new FACT
  claim against the PDFs and workbooks, and the new mathematics was checked numerically. Corrections, with new
  evidence E656–E672:
  1. Fourmile: FM24-200D is 1.3 m @ 16.25 g/t, and the 30.6 g/t interval in FM24-209D is 1.6 m inside 16.3 m @
     47.07 g/t (not 28.0 m and 57.9 m). "30–37 km of drilling" and the "Birch–Dorothy corridor" are not in the
     deck, FM25-260DW1 is on p. 8, and since 9 Aug 2026 Fourmile is to be vended into NGM (E656–E658).
  2. Carlin: the roaster curve is 92.03 − 37.36·exp(−12.94·g) with g in opt; both POX modes share one cubic, acid
     about 8.09 points higher; the Gold Quarry window is Table 13-8; the TR says nothing about autonomy (E659–E662).
  3. Autonomous haulage: Cortez finished 23 retrofits in May 2026 for a Q3 2026 rollout, and Carlin's 40 started
     in Q2 2026 for operations in Q1 2027 (E619). Nothing was "deployed 2023–2025", and ore moves by truck, not
     rail (E663).
  4. Pueblo Viejo: Tables 15-2 and 15-3 are mining and fixed costs, not a route comparison; 281.7 and 352.1 kt are
     the 2025 opening and closing plant-cone inventories in an MCF reconciliation, not capacities; waste costs more
     because of rehandling and co-disposal, not sub-aqueous placement (E664–E667).
  5. Lumwana and Kibali: the 12% → 9% loss figures, the 68 km line, the 27 Mtpa baseline and the "330 kV STATCOM"
     are not in the deck (330 kV is the line voltage; the STATCOMs are at neighbouring mines). Kibali's diesel bank
     is 32 MW, not 36, and no 50% diesel cut is attributed to solar and BESS (E668, E669).
  6. Loulo-Gounkoto: TCC $1,897/oz and AISC $2,240/oz are Q2 2026; H1 2026 is $1,906/oz and $2,104/oz (E670).
  7. Mathematics: the dispersion score of `SpatialFrontPit` was not centred (mean −0.075 under the null, which
     breaks Ville's bound), and its detection times were optimistic (a 1.4× variance excess needs about 220
     polygons, not 15–30). The FoS sensitivity omitted the implicit dependence of m_α on F (10–14% low). GB-41's
     shift tails, GB-40's second-lane wait and GB-43's interval were recomputed. The curvature correction's sign
     follows (gR)″, which changes sign near 5.3 g/t on the Carlin curve. The "tree-knapsack matroid" with a
     Lagrangian dual is replaced by bundle greedy with a valid data-dependent upper bound, and the soft-contact
     blend now keeps unit variance.

### H.6 Open audit items

- **H-1.** External review rounds (planning-workflow: at least four rounds by a strong reasoning model until the
  diff is marginal). Round 1 (`AUDIT_1`–`AUDIT_3`) and round 2 (`AUDIT_5`) are folded in; at least two remain.
- **H-2 (done 2026-10-29).** Brenner pass B6 for Headframe, Crucible, Freeboard and Muster (§36.4;
  `planning/brenner/B6_operations_plant_tailings_workforce.md`), folded into §16–§19, Appendix B and Appendix D.
- **H-3 (done 2026-10-29; Appendix J).** Primary-source verification of the Barrick and industry figures used in
  Part I: 72 claims, 49 verified, 2 corrected, 15 partly right, 4 secondary-only, 2 unverifiable (W5). Porgera
  2020, Cobre Panamá 2023 and the Mali 2023 Code now rest on primary sources (W2). Residual FACT[U]: the Panama
  sentencia number and Gazette issue numbers, the Mali implementing-decree number and the Porgera FSA rates.
- **H-4 (partly done).** `AUDIT_4` re-read E101–E399 and the quarterly workbooks. The research rows E401–E655 still
  need a second reader, and `AUDIT_4` itself deserves a sampled third read, because `AUDIT_5`'s own primary-source
  "verifications" introduced errors (H.5b).
- **H-5 (done 2026-10-29).** The 320 plan beads and 533 dependency edges of §39 are instantiated in `.beads/`
  (`br`), and `br dep cycles` reports no cycles. The idea-wizard and beads-workflow polishing rounds remain.
  Round 7 added BD-321 to BD-357 to §39 (357 beads, 589 edges, 0 cycles); they are not yet in `.beads/`, and
  BD-323 corrects the closed beads there before any new bead is created (Appendix K).

## Appendix I · The document corpus: what it showed and what it changed

This appendix records how the supplied documents were read, what they established, and where each finding now
lives in the plan. The findings themselves are in the plan body (§10–§39). This appendix is the audit trail.
Evidence ids E101–E399 are rows in `planning/corpus/C1…C6`, indexed in `planning/mor/MOR_CASE.md`.

### I.1 Corpus and method

| Group | Documents | Report | Evidence block |
|---|---|---|---|
| Barrick public statistics | Q1 and Q2 2026 Mine Stats (xlsx); Fourmile (Feb 2026), Lumwana site visit (Sep 2025), Tanzania media day (Q2 2025) and Mine Africa 2021 decks | C1 | E101–E149 |
| Nevada technical reports | Carlin (2025), Cortez (2022), Turquoise Ridge (2024) | C2 | E151–E199 |
| Africa technical reports | Kibali (Feb 2026), Loulo-Gounkoto (2023) | C3 | E201–E249 |
| Americas and PNG technical reports | Pueblo Viejo (Feb 2026), Veladero (2018), Zaldívar (2012), Porgera (2012) | C4 | E251–E299 |
| ESIA | Reko Diq Early Works ESIA (Rev A, not approved) | C5 | E301–E349 |
| Textbooks (reference-only) | Abzalov, *Applied Mining Geology*; Waltham, *Foundations of Engineering Geology*; Lacy, *Introduction to Geology and Hard Rock Mining*; a mining-geology lecture note | C6 | E351–E399 |

- **Conversion.** 18 PDFs (4,299 pages) were converted to markdown in 6.7 minutes in quick mode (mechanical
  extraction plus a pre-pass; no swarm fix-and-verify pass). The mirror is `corpus_of_input_docs/markdown_extractions/`
  with page anchors `<!-- p.N -->`.
- **Verification.** The mirror was used to *find* pages. Every value labelled FACT was re-read from the PDF text
  layer (`pdftotext -layout`) or the xlsx cell. Known mirror defects: the Waltham OCR is garbled; the Reko Diq mirror
  has digit errors; the Q2 Mine Stats mirror is unusable (a 15,903-column `YTD` range and 1,092 `#REF!` cells).
- **Cites.** PDF page numbers, not printed folios; xlsx cells as `<file-period>:<Sheet>!<Cell>`.
- **Agents.** Six read-only agents (C1–C6), one per group, under a shared brief (`planning/corpus/BRIEF.md`). One
  writer then folded the results into the plan.

### I.2 What the corpus establishes (the twelve findings that matter most)

1. **The demonstrator's public data was narrower than the plan assumed, and the gap is now closed.** Per-site
   AISC/TCC/grade/recovery exists only in the quarterly Mine Stats workbooks. The corpus held two (Q1 and Q2 2026);
   the research pass added the other eight from Q1 2024 (W1; E451). No per-site GEO field exists in any of them
   (FACT, E148, E499). Q3 2026 (9 Nov 2026) is the only open data gate (§37.3).
2. **Strict-rounding identity checks hold for 84 of 89 published cells, about 95%** (E113–E115). GATE-BULL-01
   therefore scores identities against an explicit denominator and identity rule, in the three parts of §21.6 that
   replaced the single 95% bar, and NOT_PUBLISHED quarters are excluded by name.
3. **Company AISC is not the sold-weighted sum of its sites.** Total Gold sits $47.6–55.7/oz above its regions in
   the Q1, Q2 and H1 blocks of 2025 and 2026 (+$23.57 to +$88.66 across sixteen quarters; R7-C125) and
   NGM $8.7–19.4/oz above its mines (INFERENCE, E111, E112). These become named layers, or G-BULL-3 refuses.
4. **A public AISC bridge does not close without inventory factors.** North Mara's +$1,285/oz needs a sold/produced
   effect of −$504 (INFERENCE, E137). GB-26's identity now carries B and S.
5. **No public filing reports kriging efficiency or slope.** NGM classifies by drill spacing (E158, E162, E163).
   GB-11's per-domain slope is a pilot data gate, not a public-lane output.
6. **The LUC "b ≈ 0.1" is a mechanism, not an anomaly.** Variance-restoring estimators have b ≈ ρ (INFERENCE,
   E355); the 0.1 source is not in the corpus (FACT[U]). The flip now needs correlation and cut-off tonnage, not
   slope alone (E357, E358).
7. **"F1" and "MCF" mean different things at different Barrick sites** (E203–E205). Touchstone gets a
   site-specific definitions registry, and published reconciliation tables contain balancing rows, grades on
   residual rows and cells that do not reproduce (E206–E209).
8. **Site acceptance criteria are deterministic FoS, not PoF** (Kibali; E214, E215). GB-39 is reframed: policy
   carries its basis, and PoF is evidence beside an FoS-only criterion.
9. **Heap inventory is booked on a recoverable basis and labelled "contained"** (Veladero; E274–E276). A
   `Recoverable` metal stage and a ±1 oz known answer for GATE-CRUC-03 follow.
10. **Stockpiles degrade** (Pueblo Viejo: 30% of reserves, 70.6% of 2025 feed, Probable because of sulphur
    degradation; E252–E258). Stockpile state carries a metallurgical state, and NRV uses reclaim-time recovery.
11. **Host-state action is not hypothetical.** Loulo-Gounkoto was suspended and lost to provisional administration
    in 2025, with no published operating data for three quarters (FACT, E248). RISK-26 and the NOT_PUBLISHED state
    follow.
12. **Environmental tables need a declared chemical basis and a certificate check.** In the draft ESIA, a nitrate
    value reported within limits is above the limit on a consistent chemical basis, and one certificate sample
    with two exceedances has no reported row (E322–E326). A chemical-basis axis and certificate reconciliation
    follow.

### I.3 Disposition of the 84 proposed plan impacts

| Report | Items | Accepted in full | Accepted in part | Validation only (no edit) | Where they landed |
|---|---:|---:|---:|---:|---|
| C1 | 12 | 11 | 1 | 0 | §21.6, GB-26, §23.5, §23.8, §11.7, §24.3, §24.9, §31.1, §37.3, BD-055/058/063/064/199 |
| C2 | 10 | 9 | 1 | 0 | Part I table, GB-11, §12, §14.1, §15, §17.3, §18, §20, §24 |
| C3 | 15 | 13 | 1 | 1 | §10.1, §12, §13, §14.2, GB-39, §16, §17.5, §18.2, §18.6, §20.2, §20.4, §23.4, §24.3, §35.2 |
| C4 | 17 | 16 | 1 | 0 | §12, §15.6, §17.3, §17.7, GB-20, GATE-CRUC-03, §18, §20, §23.4, §24, §25.2, §31.3 |
| C5 | 13 | 11 | 2 | 0 | §18.2, §18.4, §18.7, §19.3, §23.2, §24.3, §25, §26 obligations, §31.2, §32.1, §35.2 |
| C6 | 17 | 15 | 2 | 0 | Part I table, GB-11, §12, §13.3, §14, §15.1, §15.5, §17.4, §20, §24.4, §24.5 |
| **Total** | **84** | **75** | **8** | **1** | 111 exact replacements (plus 6 housekeeping), 0 new section numbers |

Every edit uses existing headings; new material is a bold sub-block, a table row, a guard row or a bullet.

### I.4 Declined or deferred, with reasons

| Item | Decision | Reason |
|---|---|---|
| C6 FX-1…FX-10 shipped as fixtures | Declined as written | Textbooks are reference-only. Fixtures are re-derived synthetic cases; textbook numbers serve only as a private cross-check (§32.1). |
| C6 FX-4 (Abzalov Table 10.3 precision levels) as `PolicyDefault` rows | Declined | The table is the author's work; QP policy supplies the values. The insertion-rate ranges are cited, not tabulated. |
| C6 "Ingamells formulations apply" removal | Deferred | Neither confirmed nor refuted by the corpus; relabelled HYPOTHESIS (§12.3). |
| C5 §4.7 Reko Diq Suspension → Stable as a `REFCLASS-` datum | Deferred | Dates must come from the primary record, not the ESIA (DATA-GATE). |
| C5 §4.4 noise-vs-wind covariate conditioning for environmental TARPs | Declined in the body | Low value at plan level; the general rule (averaging-time-typed limits) was kept. |
| C1 §4.10 Bulyanhulu cash-vs-accrual detail | Declined in the body | Detail belongs in BD-058's decision record. |
| C2 §4.3e list of preg-robbing mitigations | Kept as validation | The plan's route list is already correct; only the TCM-scoping rule and the Goldstrike CIL fact were added. |
| C3 §3.11 Kibali classification at quarterly/annual support | Validation only | It confirms §24.4 as written. |
| C6 "validations needing no change" (Krige's relation, blasthole errors, method = measurand, plant-tolerance simulation, NN cross-check) | No edit | They confirm existing text. |
| C5 G-FREE-6…9 numbering | Renumbered to G-FREE-7…10 | G-FREE-6 already exists (venture wall); stable ids are never reused. |
| C3 RISK-26 vs C5 RISK-26…28 numbering | Merged and renumbered | RISK-26 host-state action; RISK-27 water conflict; RISK-28 security forces; RISK-29 unapproved revisions; RISK-30 superseded-filing demo text (new). |
| MOR_CASE E029 overwrite | Appended a correction instead | The case file is append-only. |

### I.5 Data gates opened or sharpened

| Gate | What is missing | Plan |
|---|---|---|
| M0 coverage | Q3 2026 Mine Stats only (release 9 Nov 2026); AISC workbooks for Q1/Q2 2024 were never published (W1 closed the rest) | §37.3, §21.6 |
| Per-site GEO | Not published per site | §21.6 (company-level only) |
| Per-site AISC components | G&A, leases, exploration, accretion, credits per site | §21.6, BD-058 |
| Per-domain slope against GC | Blasthole and GC data | GB-11, BD-299 |
| NGM F1 | Not public | §20.4 |
| TSF consequence class | Not disclosed for NGM, Kibali FTSF or Loulo | §18.2 |
| Groundwater TARP | Piezometers and community-well baselines | §18.4 |
| Year-over-year reserve bridge | Drivers given only qualitatively | §24.7 |
| Porgera 2020, Cobre Panamá 2023, Mali 2023 Code | Closed by W2 from primary sources (E401–E449) | §23.4 |
| Kibali CPA re-assay | Originals discarded | §12.4 |

### I.6 Discrepancy-register seeds (public fixtures)

| Seed | Evidence | Engine behaviour |
|---|---|---|
| Carlin Q1 2026 100% value breaks two identities | E113 | Filed; 281 satisfies both |
| Turquoise Ridge H1 2026 ≠ Q1 + Q2 | E114 | Filed |
| Jabal Sayid identity breaks | E115 | Filed |
| Hemlo H1 2026 CoS equal to Pueblo Viejo's cell | E116 | Filed as a possible duplicate |
| Total Copper recovery 94% not reproducible from site cells: constant 0.94 in all 20 quarterly cells since 2024Q1, including 2024Q2 when Lumwana was 85% and Jabal Sayid 93% | E117, E475 | Filed; never scored |
| Loulo-Gounkoto Q1 2025 cells vs "no data" footnote; the first-publication footnote withheld only per-ounce data | E122, E477, E478 | Filed; NOT_PUBLISHED wins for scoring |
| Porgera labelled 47.5% in the Q2/Q3 2024 summaries, values at 24.5% | E468, E469 | Filed; ownership from the Minestats label |
| Q3 2025 file dates the Loulo-Gounkoto loss of control 16 Sep, every other vintage 16 Jun | E476 | Filed; 16 Jun preferred |
| Pueblo Viejo 2024Q3 processed 1,605 kt breaks 9M additivity (implied 1,475) | E470 | Filed |
| Carlin 2024Q2 100% value 327 fails T1 and H1 additivity (329 fits) | E471 | Filed |
| Total Gold 2024Q1 AISC restated 1,479 → 1,474 without a footnote | E474 | Filed; both vintages kept |
| Loulo-Gounkoto FY2025 unit costs ≠ Q4 although FY sold = Q4 sold | E473 | Filed |
| Hemlo FY2023 capex exceeds the sum of its quarters | E472 | Filed |
| Copper roll-forward 2025 sign (gap 263 Mlb) | E144 | GATE-HALL-01 fixture |
| GEO 2024 row priced at the 2025 deck | E147 | GATE-HALL-01 fixture |
| Lumwana reserve endnote (7.95 vs 8.3 Mt) | E133 | Filed; deck endnotes never source Reserve Claims |
| Carlin attributable table differences | E153, E193 | Render test |
| Kibali 2024 MCF row; Tables 6-2 vs 15-27 | E206, E241 | G-TOUCH-6, GATE-TOUCH-06 |
| Loulo RCF plant ounces | E209 | GATE-TOUCH-06 |
| Kibali umpire count conflict | E226 | Filed |
| Kibali COG royalty at a different deck from the stated price | E244 | Flagged by the regime importer |
| Loulo plant expansion: text 6.2 Mtpa vs table 4.5 Mt/yr | C3 §3.14 | Filed |
| Porgera summary price "$575" vs $1,200 evaluation | C4 P11 | Filed |
| Porgera underground ore +36% (table) vs 24% (text) | E298 | Filed |
| Reko Diq nitrate basis; omitted sample; Cr claim; survey window | E322–E328 | §18.4 certificate reconciliation |
| Reko Diq Social 4 residual | E337 | G-FREE-8 |
| Reko Diq pipeline area and genset hours | E346, E347 | G-UNIT-5 |

### I.7 Questions for the owner raised by the corpus

Questions 1, 2, 3 and 5 of the first list were answered from public sources by the research pass (Appendix J):
the workbooks were published and are now in hand except Q3 2026 (W1); "504%" is GEO over 2023–25 (W5);
Lumwana stays the M2 primary on a grade-adjusted recovery gap (W4); Porgera, Cobre Panamá and the Mali 2023 Code
rest on primary sources (W2). Still open:

1. How does the scorecard's $1,423/oz gold AISC bridge to the $1,637/oz reported (OPEN-BULL-1)? The Circular
   does not itemise the AISC adjustment (E608).
2. Is the Reko Diq "cannot disclose" state a design-stage state (OPEN-FREE-4)?
3. Residency (OPEN-B4-01, narrowed): the eight counsel questions of W3 §7, led by the Reko Diq mineral agreement
   against BMR 2002 r.56/r.71, Zambia DPA s.70(3) against s.71(4)(b), and the scope of Tanzania's Mining Act
   s.35(6).
4. Which Copper-Total recovery definition yields a constant 94% (E475)?

### I.8 Beads added (BD-271 to BD-307)

| WP | Beads | Subject |
|---|---|---|
| WP-01 | BD-277 | Chemical basis, `Recoverable` stage, hazardous by-products, mass-fraction law |
| WP-03 | BD-271–276 | xlsx extractor, GATE-BULL-01 scoring rule, aggregation layers and definition flags, cut-off reproduction, discrepancy fixtures, technical-report forecasts |
| WP-08 | BD-278–281 | F-factor registry, verbatim importer, within-factor compensation, GATE-TOUCH-06 |
| WP-09 | BD-282–286 | CRM SD basis, pattern monitors, duplicates and Gy, twins, core recovery and surveys |
| WP-10 | BD-287–290 | Card units and layers, preg-rob card, first calibration card, heap book convention |
| WP-11 | BD-291–292 | Import replay and basis assertion; community-data redaction and PK residency |
| WP-13 | BD-293–295 | `StatementRow`, ownership and tenure, stockpile metallurgical state |
| WP-14 | BD-296–297 | Country-regime rows, permit gates |
| WP-16 | BD-298–300 | Typed slope and LUC validity, pilot slope gate, geomet coverage |
| WP-18 | BD-301–305 | Commitments register, facility classes and gates/valves, environmental certificates and groundwater, closure measurands, Highwall policy and cards |
| WP-19 | BD-306, BD-319 | Blend forms, fixed-point cut-off, dilution attributes; refractory feed windows and autoclave modes, Four Corners recovery, curvature correction, Pueblo Viejo route split, inter-complex tolling |
| WP-20 | BD-307, BD-320 | Hoist factors, fill strength, dilution matrix, adopted standards; AHS token DES, `FMINE-REF-504` uncured-fill refusal, `EnergyEnvelope` |

Graph after the fold-in: 307 beads, 505 edges, 0 cycles, an acyclic work-package graph, and a longest chain of 29
beads (WP-00 → 01 → 02 → 03 → 12 → 18). The appendix cross-check then added BD-308 (308 beads, 507 edges). The
concurrent revision added BD-309 to BD-320 across WP-01 (BD-309, BD-310), WP-02 (BD-311), WP-08 (BD-312), WP-10
(BD-313), WP-14 (BD-314), WP-15 (BD-315), WP-16 (BD-316, BD-317), WP-18 (BD-318), WP-19 (BD-319) and WP-20
(BD-320), and the second-wave review wired BD-309 into BD-161 and BD-312: 320 beads, 533 dependency edges and 0
cycles, with the same acyclic work-package graph and 29-bead critical path. WP-16 depends on WP-12 because the
per-domain slope needs pilot data.

### I.9 The demonstrator dataset (what M0 can use today)

The M0 public lane (`fmine assure public --issuer barrick`) can start from the corpus now. Source: C1 §3, values
read from the xlsx cells.

| Source | Content | Basis | Use |
|---|---|---|---|
| `QTD` (both files) and `YTD` (Q2 file, columns A–N only) | Per-site gold for 14 sites, 3 regions and Total Gold; copper for Zaldívar, Lumwana, Jabal Sayid and Total Copper. Fields: production (100% and attributable), sales (attributable), CoS, TCC (gold) or C1 (copper), AISC | Production at 100% and attributable; unit costs per attributable unit **sold**; TCC, C1 and AISC are non-GAAP as defined in Endnotes #1 and #2 | Scored M0 cells; regional and total identities |
| `Minestats1`–`4` | Tonnes mined (open-pit ore and waste, underground), strip ratio, cost per tonne mined and processed (before capitalized stripping), tonnes processed by circuit, grade (open-pit mined, underground mined, processed), recovery by circuit, production and sales, revenue (Q2 file only), CoS, royalty, depreciation, TCC and AISC per ounce, sustaining, project and total capex ($M, cash basis, Barrick share) | **Attributable** (E109: North Mara and Bulyanhulu physicals ÷ 0.84 match the 100% site decks) | Scored M0 cells; metal-balance and cost diagnostics |
| `Endnote` | AISC, TCC and C1 definitions; reconciliation pages (Q1 2026 Report from p.36; Q2 2026 Report from p.44); QPs; reserve as-of date 31 Dec 2025 | Definitions as published | Metric-definition registry v1; the second locator on every Claim (E107) |
| Site decks (Lumwana Sep 2025, Tanzania Q2 2025, Fourmile Feb 2026, Mine Africa 2021) | Charts, guidance, timelines, resource bridges | Mostly 100% basis, stated per chart | M2 site choice, Lodestar and Helm inputs; never a source for Reserve Claims (E133) |
| Technical reports (C2–C4) | Reserve and resource tables, reconciliation tables, cut-offs, recovery equations | Per report, recorded with ownership | Cut-off reproduction (7 cut-offs), discrepancy register, Touchstone and Crucible fixtures |

- **Periods.** In the corpus: 2025Q1, 2025Q2, 2026Q1, 2026Q2, H1 2025 and H1 2026, six period blocks. With W1:
  every quarter 2024Q1–2026Q2 plus the half-year, nine-month and full-year blocks and the 2023 comparatives; the
  counts below are for the two corpus files. Loulo-Gounkoto's 2025
  periods are NOT_PUBLISHED per the workbook footnote, although some Q1 2025 cells hold values (E122); the scorer
  excludes them by name.
- **Size.** 3,996 parsed metric cells. The five scored per-site fields (CoS, TCC/C1, AISC, processed grade,
  recovery), excluding the NGM and Copper-Total aggregates, give 480 cells: 427 numeric, 36 `—`, 12 `n/a`, 5
  numeric-zero placeholders.
- **What the data already shows (C1 §3.5).** Attributable = 100% × ownership holds for 84 of 89 cells; flows sum to
  regions and totals 108/108; site-sheet values equal summary values 539/540; regional unit costs equal
  sold-weighted site costs in 87 of 108 cases, and all 21 misses are aggregation layers, not errors.
- **Import rules.** Label-anchored rows (rows drift between files); footnote digits stripped from labels;
  ownership parsed from the label and never defaulted to 100%; `—` → `Nil{reason: unknown}`, `n/a` →
  `NotApplicable`, blank → `Missing`, numeric-zero placeholders → `Nil`; units as stored (recovery and copper grade
  as fractions, tonnes in kt, gold in koz); `#REF!` residue, the 15,903-column `YTD` range and footnote rows
  excluded.
- **Cannot support.** Per-site GEO; per-site AISC components (G&A, leases, exploration, accretion, by-product
  credits) before Q3 2024, when the AISC workbooks start; Q3 2026 until 9 Nov 2026 (I.5); NGM's F1 or any public
  slope of regression.
- **Ownership caution.** NGM sites (Carlin, Cortez, Turquoise Ridge, Phoenix) are 61.5% Barrick and 38.5% Newmont;
  every NGM figure carries its basis and is JV-sensitive (E034, §31.1).

### I.10 Rights and privacy

- The four textbooks are cited by page and never shipped. One copy carries a third-party watermark; another is a
  commercial scan.
- The Reko Diq ESIA is an unapproved Rev A marked confidential. Cells sourced from it cap at `conflicted` or
  `stale` (RISK-29). Its appendices include individuals' names and contact details. None of that appears in
  the plan, the case file or this appendix, and the importers redact such fields on ingest (§31.2).

## Appendix J · The public-source research pass (W1–W5)

### J.1 Why it ran

Appendix I ended with questions "only the owner can answer". Most of them have public answers. Five agents
looked for primary sources on 2026-10-28/29 (filings on SEC EDGAR, Barrick's IR host, official gazettes, statute
texts) and wrote one report each to `planning/research/`. Two smaller questions (crate naming, the asupersync 0.6
bump) were checked directly. No agent edited the plan; the fold-in was done afterwards with exact-match patches.

### J.2 What each pass found

| Pass | Question | Result | Where folded |
|---|---|---|---|
| W1 | The missing Mine Stats workbooks | All ten quarters Q1 2024–Q2 2026 retrieved with MD&As and AISC workbooks; Q3 2026 releases 9 Nov 2026. Structural identities hold everywhere; 17 new discrepancy candidates; Copper-Total recovery is a constant 94%; a Porgera label of 47.5% against values at 24.5%; the corporate AISC layer is positive in all 16 periods | §21.6, §37.3, I.2, I.5, I.6, I.9 |
| W2 | Porgera 2020, Cobre Panamá 2023, Mali 2023 Code | All primary-sourced. Mali's code is Loi n°2023-040, applied retroactively to Loulo-Gounkoto ($200M, Apr 2026; royalty about 18%); Porgera's 2017 extension application existed; DRC Art. 80 renewal costs +5% State equity | §10.1, §21, §23.4, RISK-26 |
| W3 | Data-residency statutes (OPEN-B4-01) | 72 statute-cited rows for 12 countries; hard localisation in Zambia, Tanzania, DRC and Mali for personal data; Reko Diq governed by its agreement and BMR 2002 | §26.14, §31.2, RISK-21, BD-292 |
| W4 | M2 site and P(site data by month 7) | Keep Lumwana (only wholly owned operating mine; grade-adjusted recovery gap 1.7–2.7 points, $30–57M/yr); p = 0.3, so M2′ is the modal path | §34.4, §37.2, §37.3 |
| W5 | The plan's industry and Barrick figures | 72 claims: 49 verified, 2 corrected (Tanami 2 history; the 190%/504% bases), 15 partly right, 4 secondary-only, 2 unverifiable | Part I, §11, §14, §16, §21, §23, Appendix G |
| Direct | `fmine` naming; asupersync 0.6 | `fmine` is free on crates.io; asupersync 0.6 is unreleased and only frankensympy's runtime crates follow main | §28.1, §31.4 |

### J.3 Corrections that change an executive statement

1. **Tanami 2** went from $850–950M to $1.7–1.8B (about 1.8–2.1×), not "from $395M": $395M was spend to June 2022
   (E631, E632).
2. **Reserve replacement**: 190% and 504% cover the same three years; they differ by metal (gold vs GEO), not by
   window (E603, E604). OPEN-BULL-4 is closed.
3. **Barrick's pay GEO** uses fixed reserve prices, so the gold price does not move pay GEO (E606; GB-27's flip).
4. **North American Barrick IPO**: the official target is still end-2026; the CEO said on 29 Sep 2026 it may slip
   into January 2027 (E615, E616).
5. **Mali settlement**: Barrick's filing gives $253M (28 Nov 2025), not the $430M press figure in R6 (E408).
6. **Autonomy at Cortez** was retrofitted in May 2026 and was due to roll out in Q3 2026; it was not yet "live" (E619).
7. **Loulo-Gounkoto**: the reset is a fiscal one (2023 Code, 18% royalty, retroactive to 2024), with ownership
   unchanged at 80% (E406, E412–E414).

### J.4 Collisions repaired in a concurrent edit

A concurrent revision (2026-10-28, 23:44 local) added BD-309–BD-320 and new design text. It reused two identifiers that already had meanings: the new support-regularization guard took `G-STRATA-6` (already
"QA/QC invalidation after the cut") and the pad-tree dual bound took `PO-LODE-5` (already the MapMark4 parity
test). They are renumbered `G-STRATA-11` and `PO-LODE-15`, with table rows, and the new refusal code 108 was added
to the `FMINE-REF-1xx` registry. That edit's content is otherwise kept as written. Its Loulo-Gounkoto cost figures
were Q2-only and are now labelled with the H1 values beside them (E638).

## Appendix K · Round 7: the M0 reality audit, the assurance-chain pass (B7) and the idea duel

### K.1 What ran (2026-10-29)

| Pass | What it did | Where |
|---|---|---|
| Modes-of-reasoning round 7 | A neutral case (E673–E690 new, E001–E672 imported), six reasoning passes (baseline, M0 reality, internal consistency, quantitative checks, readers and obligations, dependencies), a normalised ledger of 171 claims and eight auditors who did not write the claims they checked. Auditors re-ran the M0 binary against real and altered workspaces, re-read workbook cells and recomputed the arithmetic. Result: 102 VERIFIED, 59 PARTIALLY_SUPPORTED, 2 REJECTED, 1 HYPOTHESIS, 1 SUPERSEDED, 6 NON_MATERIAL; no genuine conflict survived | `planning/mor/r7/` (report, case, ledger, audits) |
| Brenner B7 | The assurance chain (§20, §21, §22, §24, §30) tested against the real Barrick workbooks and filings and the obligations of QPs, CFOs and IR, on a live NTM squad (Claude, two Codex, Gemini), with a forced third alternative and a cross-family disagreement register | `planning/brenner/B7_assurance_chain.md` |
| Idea duel | Claude, Codex and Gemini each generated 30 ideas, kept 15, scored the other two models' 15 on 0–1000, saw their own scores, rebutted, steelmanned the rival's best idea and named blind spots | `planning/duel/` (`DUELING_WIZARDS_REPORT.md`) |

Claim ids in this plan of the form `R7-C###` refer to `planning/mor/r7/MOR_CLAIMS.jsonl`.

### K.2 The committed M0 slice against this plan (FACT; executions of 2026-10-29)

A local commit, `68e7091`, added 15 crates, `xtask`, registries, CI and 320 beads (87 closed). It has not been
pushed; `origin/main` and `origin/master` are at `30ee8f5`. What the auditors reproduced:

| # | Finding | Claims |
|---|---|---|
| 1 | Every figure in `fmine assure public --issuer barrick` comes from an in-code stand-in workbook or from literals. Any registered file that is missing or unreadable is silently replaced by the stand-in, per file, with exit 0. With the real workbooks under the expected names the command refuses (FMINE-REF-502: no "Gold AISC" sheet). Changing an input cell leaves the output and state root byte-identical | R7-C001, R7-C011, R7-C058, R7-C059, R7-C065 |
| 2 | Figures labelled as Barrick's are wrong: FY2025 AISC 1,638 against a published $1,637/oz; 3,255 is attributable production, not ounces sold (3,318); FY2024 3,973 matches no gold-ounce cell; the corporate layer has the wrong sign; the North Mara block is labelled 2024 Q1→Q2 with values that match no cell (the real move is Q2 2025 $1,292 → Q2 2026 $2,577); reserve-replacement inputs were solved to hit 190%/504%; the price decks are shifted by a year. None of the stand-in's 41 explicit values sits at a published coordinate | R7-C002–R7-C006, R7-C054, R7-C159 |
| 3 | The parser itself works: it reads all 18 real workbooks and matches an independent reader exactly. The obstacles are the compiled-in file list, the hard-coded sheet name and fixed coordinates | R7-C007–R7-C010, R7-C063 |
| 4 | All 87 closed beads share one close reason citing GATE-BULL-01, closed within 12 seconds; four were closed over open blockers; at least 23 have unmet done-when conditions (no GATE-BULL-01 scorer, no INV-002 flags, no golden cards, no robot schema) | R7-C013–R7-C016 |
| 5 | INT-AS-001 and INT-NUM-001 records say PASS while no external crate is in the build graph; `lmdi.rs` and `fmine-calib` call `std` transcendentals although INT-NUM-001 lists them as BitwisePortable | R7-C017–R7-C019, R7-C083 |
| 6 | The envelope lacks the four INV-002 flags, certificate, `next_actions` and the `no_claim` line; `claim show` returns `Exact`/`KnownKnown` for any id with the right prefix | R7-C021, R7-C022, R7-C062, R7-C147 |
| 7 | Refusal codes mean different things in the plan, the registry and the compiled catalogue (4 of 25 agree); the evidence types are not §7.1/§7.2's; the CONTRACT template lacks six of §0.2's ten sections; 14 of 15 LOC budgets differ from the registry; five of eight lift guard tests do not exist; five lift headers misstate frankensim's licence; `unwrap`/`expect`/`panic` are not denied | R7-C080, R7-C081, R7-C087, R7-C088, R7-C096, R7-C099, R7-C164 |
| 8 | `deny.toml` would fail asupersync 0.5.0 (rider licences, duplicates) and is not run in CI; `franken-stack.lock` uses `-01` row ids that collide with F.2's upstream-ask ids and names an unrelated crates.io `fastmcp` | R7-C150–R7-C152, R7-C162, R7-C163 |
| 9 | What holds: §39 and `.beads/issues.jsonl` agree exactly (320 beads, 533 edges, acyclic, 29-bead critical path); §38 is the transitive reduction of the bead graph; §28.3 equals the LOC registry; all 51 inter-crate edges point downward; every lock pin resolves in the sibling repositories; the toolchain pin matches | R7-C068–R7-C074, R7-C166, R7-C170 |

**Before any push (DOMINANT under the owner's loss function; R7 report R001–R003).**
1. Remove both silent fallbacks; refuse with a non-zero exit that names each absent or unreadable registered file.
   Move the stand-in and every Barrick-attributed literal to a synthetic lane whose output prints `synthetic` and
   cannot count toward GATE-BULL-01. Replace the four stand-in tests with labelled synthetic tests plus a real-file
   run; do not delete or weaken them (BD-321).
2. Reopen the closed beads whose done-when is unmet, or replace the shared reason with each bead's own evidence; no
   close reason cites GATE-BULL-01 before a scorer records a PASS on real inputs. Rewrite INT-AS-001 to "not
   admitted" and INT-NUM-001 to "FD-row fallback taken" (BD-323).
3. Pin the 18 real workbooks in a source registry and rebuild the importer on label anchors before the Q3 2026
   release (BD-322).

`68e7091` was pushed on 2026-10-29 at the owner's instruction, followed by correcting commits (`543df53` …
`748da79`); public history is never rewritten, so it stays (OPEN-R7-5, resolved). Step 2 was not complete at that
push and is tracked as BD-323.

**State after the corrective work of 2026-10-29 (FACT, executions; MOR_CASE E820–E823).** Items 1 and 3 are done.
- **The stand-in and both fallbacks are removed.**
  - A workspace without the files refuses with FMINE-REF-502 (exit 8), naming all 18.
  - An altered file refuses with both digests.
- **`fmine-public` now holds:**
  - a registry pinning the 18 workbooks (URL, bytes, SHA-256);
  - a label-anchored AISC reader;
  - the five-state identity ledger with exact certificates and registered reconstructions;
  - a synthetic lane that prints `synthetic` and is never gate-eligible.
- **The real-file run reproduces an independent reader exactly.** This covers the census and numeric digest of all
  18 workbooks, and the full ledger: 34 tables, 2,595 instances, footing 1,004/4/6/24, per ounce
  1,116/3/17/385/36.
- **The envelope** carries the INV-002 flags, `no_claim`, cited exceptions with certificates, and
  `next_actions`. `claim show` refuses.
- **In `fmine-metrics`:**
  - the price decks are the Circular's (E606);
  - reserve replacement refuses a non-positive depletion;
  - the four mislabelled tests are synthetic.

Still open:
- item 2 (BD-323);
- findings 5, 7 and 8;
- GATE-BULL-01 part 3 (issuer consistency across vintages).

Cell number formats are now read and the ledger scores displayed values (bd-jg43.1–.3, MOR_CASE E833, E834 and
E838). E823's twelve refusals are all (i). The LibreOffice cross-check of the display rules is still to run.

### K.3 Corrections folded into this plan

| # | Where | Change | Claims |
|---|---|---|---|
| 1 | Document control | Status reflects the M0 scaffold; process artifacts list R7, B7 and the duel | R7-C028, R7-C043 |
| 2 | §3 | Rows added for investor relations and the disclosure committee, and for the compensation committee | R7-C139, R7-C140 |
| 3 | §5 NS-01 | M0 bars restricted to what public data support; fuel, FX and labour bars wait for M2 | R7-C033, R7-C044 |
| 4 | §10.4 | `Coverage` time to alarm at 58% coverage: median about 2 years, not 3–5 years | R7-C124 |
| 5 | §10.7 gates | GATE-PLUMB-E3 pooled over ≥ 50 replicate worlds; GATE-PLUMB-E5 needs ≥ 1,000 null runs and a Clopper–Pearson bound; KILL-PLUMB-6 states the ARL₀ that keeps chance kills ≤ 5% | R7-C120, R7-C122 |
| 6 | §17 GATE-CRUC-02 | Pooled over recovery and throughput; the twelfth outcome is dated from first plant data | R7-C120 |
| 7 | §21.6 | Public lane: registered inputs only, typed refusal on any missing file, state root over the input manifest, filed-page labels for company-level figures | R7-C034, R7-C037, R7-C038, R7-C065, R7-C131 |
| 8 | §21.6, §21.8, §37.3 | GATE-BULL-01 split into extraction fidelity, identity accounting and issuer consistency; the single 95% bar and its 84/89 basis withdrawn; kill on blind-file extraction fidelity | R7-C023, R7-C024, R7-C040, R7-C050, R7-C110, R7-C111 |
| 9 | §21.6, §37.3 | The FY2026 forecast record: cut-off, manifest, 100% basis, baselines, interval score, external time proof by 31 Dec 2026, attestation, Q1/Q2 2027 vintages, no positive calibration claim before n ≈ 35 | R7-C026, R7-C042, R7-C061, R7-C066, R7-C118, R7-C119, R7-C154, R7-C155 |
| 10 | §21.6 | Corporate-layer band stated for the six blocks it fits (Q3 2025 is +$88.66); 504% labelled as the published figure | R7-C029, R7-C125 |
| 11 | §21.8 | GATE-BULL-02 is M3 (with INT-FA-001); GATE-BULL-03 in the site's unit ($/lb at Lumwana) and honest about cost-driver bars | R7-C091, R7-C105, R7-C142 |
| 12 | §21.9 GB-28 | Review-load arithmetic restated with its assumptions; year-1 writer time is 12–18 min/day on average | R7-C123 |
| 13 | §22 GATE-HELM-02 | χ² test over K problems replaces the per-seed 2·SE rule | R7-C121 |
| 14 | §27.5, F.1 | frankenpandas pin fixed at git `37bb94101` (crates.io 0.6.0 is older); `fastmcp-rust` named; FD row ids canonical | R7-C162, R7-C163, R7-C165 |
| 15 | §28.3, RISK-25 | Sibling velocity reference class; LOC figures are scope caps; RISK-25 raised to High | R7-C112, R7-C113, R7-C115 |
| 16 | §30.1 | Refusal family → exit code table; usage errors exit 2; registry is the single source | R7-C080, R7-C092, R7-C093, R7-C100 |
| 17 | §31.1 | "Private access" defined; the issuer's public lane frozen at GATE-DATA-01; issuer data terms | R7-C052, R7-C136, R7-C137, R7-C158, R7-C160 |
| 18 | §34.2 | Admission records need a tracked `Cargo.lock` with the foundation; INT-AS-001 split into 001a (WP-00) and 001b (WP-05, M1) | R7-C017, R7-C083, R7-C101, R7-C153 |
| 19 | §34.4 | GATE-DATA-01 freezes the public lane; GATE-DATA-03 has recovery-led and grade-led thresholds; P(M2′) ≈ 0.7 with Lumwana alone, about 0.5 only if the alternates were independent | R7-C084, R7-C102, R7-C126, R7-C137 |
| 20 | §35.1 F7, §35.2 | F7's 4-of-8 miss is strong evidence (P = 0.005); RISK-31 issuer data terms, RISK-32 synthetic figures reaching an issuer, RISK-33 overstated process records | R7-C118, R7-C160 |
| 21 | §36.2 | OPEN-HALL-4 moved before GATE-DATA-01; OPEN-R7-1…5 added (data terms, reviewers, the owner's lane status, the board and partner passages, the fate of `68e7091`) | R7-C030, R7-C032, R7-C045, R7-C138, R7-C161 |
| 22 | §37.3 M0 | Blind-test timing (push before 06:00 ET on 9 Nov 2026, else Q4 2026 at M1); named reviewer pool and disclosure sequencing (40-F filer; NP 51-201 and provincial statutes, not Reg FD); crates include BD-090 and BD-142; dependency calendar by role | R7-C025, R7-C041, R7-C086, R7-C089, R7-C090, R7-C104, R7-C116, R7-C132–R7-C135 |
| 23 | §37.3 M2 | Recovery-led GATE-DATA-03 route; a material finding routed to Barrick's disclosure QP and committee the day it is attested; waterfall bars classed controllable, price-linked or structural | R7-C084, R7-C102, R7-C138, R7-C141 |
| 24 | §39 | Bead rule: own-evidence close reasons, no closing over blockers, an xtask check; BD-123 accepts the FD-FMD-00 fallback | R7-C039, R7-C085, R7-C103 |
| 25 | §21.6, §21.8, §37.3 | GATE-BULL-01 part 2 rewritten as five cited states under a declared rounding construction; the live-dependency proof added to M0 acceptance | B7; E691–E698 |
| 26 | §21.6 | G&A sits wholly in L_corp (FACT); the waterfall is descriptive on public data, with factor-domain refusal, rounding intervals per bar and the printed-rate gap listed | E697, E699 |
| 27 | §5 NS-01 | "Certified causal waterfall" becomes causal only once the metallurgical balance is in (M2) | E699 |
| 28 | §22.1 | A finding is not a decision: evidence-challenge packets for public exceptions | B7 EV-P2-3, EV-P3-5 |
| 29 | §22.8 GB-29 | No-flip prior 34/43 ≈ 0.79, not about 0.78 | B7 EV-P2-4 |
| 30 | §24.3, §24.9 | Issuer reserve rows as imported assertions; GATE-HALL-01's fixture is the filed roll-forward, with the deck as a cross-check | E692, E698 |
| 31 | §37.3 M0, M1 | M0 adds the live-dependency proof, the reproduction kit, certified discrepancies and (as a target) the guidance transcription; M1 adds the public-data capabilities of K.5 | K.5 |
| 32 | §39, BD-272 | BD-272 drops the ±1-last-digit rule; BD-321 to BD-357 added (K.6) | B7 EV2-P1-1 |

**Rejected during audit (kept as lessons).** "The 95% bar is reachable because AISC values match Mine Stats in 420
of 432 cells" (R7-C064): that agreement rate is a different test on a different denominator. "INV-009 has no
enforcing bead" (R7-C095): the same rule is NL-6, covered by BD-042 and GATE-PLUMB-E1.

**Checked nulls.** JV partners, regulators and QPs have no material reader gap in M0–M1 (R7-C143–R7-C145);
retrospective DecisionCards built only from public evidence touch no disclosure rule (R7-C146); site access and
suite non-convergence are controlled for M0–M2 deliverables (R7-C168, R7-C169); the build-host disk shortage
affects local throughput, not plan control (R7-C171). The recomputed cards GB-03, GB-08, GB-26, GB-29 and GB-35
hold (R7-C055, R7-C056, R7-C125).

### K.4 What B7 established about the assurance chain

B7 tested 24 design hypotheses (six per pane, one pane per model family plus a second Codex pane) against the real
workbooks and filings, in two refutation rounds, a decisive pilot and three per-family distillations. The full
record and the verdict table are in `planning/brenner/B7_assurance_chain.md`, the register in
`planning/brenner/B7_disagreement_register.md`.

**Adopted into this plan (rule → where).**
1. Bind every imported value to its bytes *and* its interpretation: cell or page, header, unit, basis, subject and
   publication vintage, with the raw token kept. An anchor alone proves too little (§21.6; BD-322, BD-332).
2. Extraction fidelity is its own gate. A five-state identity ledger reports what the identities show; no
   completeness count stands in for fidelity (E696; §21.6 part 2; BD-331).
3. Name the rounding construction before judging an identity: a subtotal rounded once, or a sum of rounded
   component lines. The AISC tables identify the second (1,207 of 1,212 totals foot; E815). Feasibility,
   explanation and physical accuracy are separate claims, and an explanation is either stated by the issuer or
   reconstructed from public cells (E691, E694, E695, E698, E816; BD-340).
4. On public data the waterfall is a descriptive bridge: factor-domain refusal with a signed additive fallback,
   rounding intervals on every bar, and closure treated as arithmetic, not attribution (E699; §21.6; BD-333).
5. A finding is not a decision: public exceptions ship as evidence-challenge packets, and DecisionCards refuse
   without actions, states and utilities (§22.1; BD-349).
6. Issuer reserve rows are imported assertions with deck and vintage keys; filed rows win over decks, and both
   vintages of a corrected row are kept (§24.3; GATE-HALL-01; E692, E698; BD-335).
7. Exact finite VOI where the decision supplies finite states and utilities. GB-29's no-flip prior is exactly
   34/43 ≈ 0.791, not about 0.78: at 0.78 a "low" result still flips the decision and EVSI is about $2.3M, below the
   test's $9.8M cost (B7 EV-P2-4; GB-29 corrected).
8. The agent surface reads from the store: an invented Claim id is refused, and a draft is distinguishable from its
   registration or adoption (§30; K.2 finding 6).

**Killed or narrowed in the plan's earlier text.** The ±1-last-digit identity rule (it flips 21 of 306 regional
verdicts; BD-272); "zero unexplained failures" as identity acceptance (replaced by five states); a universal causal
waterfall on public data; a monetary DecisionCard for every public finding; investor-deck endnotes as the
roll-forward fixture; G&A as an inference about the total layer (now FACT, E697).

**Settled by B7's decisive pilot, then checked against the bytes.** The Africa & Middle East Q1 2025 gap closes
from Loulo-Gounkoto's public cells, with the inclusion rule unstated (E816). Loulo-Gounkoto lost control on June
16, 2025, as both filed MD&As say (W1-DR-03 closes), and the Q1 2026 MD&A states that no operating data were
provided for Q1–Q3 2025. NOT_PUBLISHED therefore stands for scoring (Appendix I.6), and the Q1 2025 workbook's
production cells stay a register item (E818).

**Still open (owner or next pass).** Why Veladero's per-ounce rows sit on an ounce series no public cell carries
(six quantities, Q1 2024–FY2025; E691, E817), and North Mara's Q2 2026 rows on one about 1 koz below the printed
ounces (E819); Hemlo's Q4 2025 and FY2025 cost of sales per ounce; and the 2023 Latin America & Asia Pacific region
rows that disagree with their own members (E817). Each is a state-(iv) item naming the document that would close
it. The cheapest common test is to run the one-cell cost-of-sales check, share-scaled, on every site column of the
eight AISC workbooks: if only these columns fail, the cause is site-specific, not a basis rule.

### K.5 New capabilities from the idea duel and B7

Three models each proposed 15 ideas (45 in all), scored the other two models' 15 on 0–1000, saw their own scores,
rebutted, steelmanned the rivals' best ideas and named blind spots (`planning/duel/DUELING_WIZARDS_REPORT.md`). Each
capability below has a first slice that does not assume the M0 code works. Scores are the two rivals' scores,
then any revision after steelmanning.

**The headline (INFERENCE).** All three models' own first-ranked ideas were about the issuer's published guidance
(CC-01, COD-01, AGY-01). Two independent pairs converged on an exact rounding-aware consistency check (CC-02, COD-02)
and on per-value provenance tests (blind spots of Claude and Gemini). A third pair converged on recall of issued
outputs (blind spots of Claude and Codex). The duel's strongest advice is therefore not a new subsystem: test the
evidence chain itself, and use the issuer's own forward statements as the pre-registered forecasts that M0 lacks.

| # | Capability (origin; rival scores) | Subsystem and reader | First slice | Test | Kill or fallback | Bead |
|---|---|---|---|---|---|---|
| 1 | Live-dependency proof (Gemini and Claude blind spots, independent) | Testpit, Portal; gate reviewers, auditors | M0: delete or blank each registered file; perturb each cited cell by one unit in its last place | Seeded hard-coded literal, dead ancestor edge and silent fallback each fail; every M0 scored value passes | If the CI budget is exceeded, sample cells stratified by output, never skip a file | BD-338 |
| 2 | Reproduction kit for outside readers (Gemini blind spot) | Portal; auditors, readers of the public repository | M0: URLs, sha256, label selectors, expected state roots; no workbook bytes committed | A clean checkout plus the issuer's downloads reproduces every state root | A re-posted file with new bytes is a new vintage in the registry | BD-339 |
| 3 | Exact rounding-interval consistency solver (CC-02: 875, 910; COD-02: 830, 875) | Bullion, Portal, Hallmark; controller, IR, auditor, JV partner | M0: linear identities on the ten Mine Stats workbooks; M1: per-ounce identities and roll-forwards | 0 certified conflicts on 10⁴ generated clean tables; known answers E113–E116, E144, E147, E691, E692, E698 | If it never finds more than the interval rule alone, it stays as BD-331's kernel and is not a product | BD-340 |
| 4 | Guidance track record (CC-01: 730 → 790, 865 → 905) | Plumb, Bullion; CFO, IR, compensation committee, board | M0 target: two-reader transcription FY2023–FY2026 with pages; M1: scores | Every value reproduces its page or cell; estimators cover truth 85–95% over ≥ 200 synthetic replicates | Fewer than three resolved years × ten sites with citations in week 1: single-site guidance cards | BD-341 |
| 5 | Guidance completion test (COD-01: 610 → 720, 860 → 900; AGY-01 narrowed: 540 → 640, 430 → 610) | Bullion, Helm; CFO, disclosure committee, site GMs | M1, public lane only: required remaining production and AISC numerator against the site's quarterly envelope and its technical-report nameplate | North Mara H1 2026 reproduced (68 koz against 200–230 koz; E587); synthetic sites with known feasibility | If a year of results never changes an envelope tier, fold it into the track record | BD-342 |
| 6 | Forecast cohorts and outcome vintages (COD-08: 700, 735) | Plumb; every reader of a track record | M0–M1, with the FY2026 forecast record | No missing outcome leaves a denominator; a restatement never overwrites an assessment | None: it is a ledger property | BD-343 |
| 7 | Temporal-cone check (AGY-15: 720, 790) | Plumb, Testpit; QPs, auditors | M1, on the first hindcast and both retrospective cards | Seeded leaks through a variogram fit, an exclusion list and a deck are caught; replay with post-cut objects removed matches | Ancestors without availability times make the card refuse; the rule is never relaxed | BD-344 |
| 8 | Disclosure lint in cited mode, with a publicness verdict (CC-04: 810, 810; converges with COD-10, AGY-05 and Claude's blind spot B2) | Adit; disclosure committee, IR, AI agents | M1: cited mode and six metric keys over public text | Two-reader labelled set: no false `verified`; publicness verdicts match filing dates | Discovery precision below about 90%: cited mode only; counsel declines the publicness verdict: drop it | BD-345 |
| 9 | Reliance status and outbound register for issued outputs (Codex and Claude blind spots, independent; COD-04 narrowed) | Adit, Plumb; IR, audit committee, JV recipients, agents | M1: synthetic issuer, one report, two holders, one later defect | A withdrawn output is refused for current reuse while its digest still verifies | Holders never reuse detached outputs: keep only the export-path check | BD-347 |
| 10 | JV mirror (CC-06: 830, 850) | Portal, Bullion; JV partners, both audit committees | M1: NGM and Pueblo Viejo against Newmont's filings, Kibali against AngloGold Ashanti's | Production mirrors FY2023–FY2025 reproduce both filings' cells; each bridge cites its own reconciliation | Coarser partner perimeter: reconcile there; cost lines refuse: production and capex only | BD-346 |
| 11 | Evidence-challenge packet (B7 H-P2-6, survived) | Plumb, Bullion; source owners | M0, with the discrepancy register | Every register item renders; none carries an invented NPV, CVaR or likelihood | None | BD-349 |
| 12 | Non-interference check at the wall (Gemini blind spot) | Bedrock; general counsel, IR, agents | M2, when private data arrive (GATE-DATA-01) | A seeded MNPI note that changes a ranking is caught | Deterministic tool outputs only; prose stays behind the lint | BD-348 |
| 13 | Target disclosure checks (CC-10: 760, 785) | Lodestar, Helm; BD and M&A | M1–M2: synthetic juniors, one developer cash-flow replay | Seeded selective reporting caught; one public NPV reproduced | Irregular release formats: cited mode, a person enters intervals | BD-350 |
| 14 | Comparability engine (CC-12: 705, 705) | Bullion, Portal; CFO, IR, remuneration committee | M1–M2: AISC and production for four issuers | Synthetic claim set classified correctly | Definitions too thin: production and capex only | BD-355 |
| 15 | Untested-ore decision bounds (COD-28: 590, 815; contested) | Crucible, Strata, Helm; metallurgists, QPs | M2 | Synthetic strata known answers | Blending interactions declared: refuse | BD-351 |
| 16 | Evidence-origin accounting (COD-03: 620, 805; contested) | Plumb, Corebox; QPs, statisticians | M1–M2 | Batch-bias known answer; a reprint never narrows an interval | Overlaps domain bias models: keep only the schema | BD-352 |
| 17 | Compensation-formula exposure (Codex blind spot; the R7 compensation-committee row) | Bullion, Helm; remuneration committee | M1: synthetic scheme; one disclosed formula if its terms suffice | Every threshold boundary enumerated; a range across a threshold is never a point | No formula reproducible from disclosures: an ambiguity warning only | BD-353 |
| 18 | Insurance recovery evidence (Gemini and Codex blind spots, independent) | Helm, Bullion; risk and insurance team, CFO | M3, after the sponsor gate | Synthetic known answers | No repeatable insurer question, or terms unavailable | BD-354 |
| 19 | Pilot protocol card (CC-05: 625, 765) | Testpit, Crucible; sponsor, site GM | M1, before GATE-DATA-01 | Reviewed by the sponsor | Power stated as an envelope; no ranking fallback | BD-356 |
| 20 | Salted review (Claude blind spot B4) | Testpit; gate reviewers, QPs | M0, internal reviews only | Two rounds report a catch rate with a Clopper–Pearson bound (0 of 5 caught bounds it below about 52%) | Reviewers decline; any seed reaches an export: stop | BD-357 |

**Merged, parked or dropped (with the reason).**

| Idea | Disposition |
|---|---|
| CC-09 guidance lifecycle; COD-16 intervention-affected outcomes | Merged into BD-341 and BD-343; the sequential revision guarantee is dropped |
| COD-10 bounded sentences; AGY-05 prose verifier | Merged into BD-345 |
| COD-04 decision recall; Claude's B3 outbound register | Merged into BD-347 |
| COD-07 JV disagreement; COD-09 comparability frontier | Public versions are BD-346 and BD-355; partner-agreed exports wait |
| CC-13 sealed-answer benchmark | Folded into GATE-ROBOT-01 |
| CC-03 spreadsheet lifter; AGY-02 workbook autopsy | Narrowed to a structural audit at M2: public formulas sit almost wholly in three workbooks (1,092 `#REF!` cells in Q2 2026 Mine Stats, E466; 304 formula cells in each 2026 AISC file, 302 of them external references) |
| AGY-12 geomet bridge | Its critique of BD-300's guarantee stands; folded into BD-300 |
| AGY-07 stream terms; COD-05 metal to spendable cash | Folded into BD-314 |
| AGY-09 stockpile segregation; AGY-14 stope sequencing | One candidate operator in BD-312; the two-sided paste window in BD-307 |
| CC-07 intertemporal-shift ledger (450, 825); CC-08 fiscal mirror; CC-11 conversion bounds; CC-14 board delta pack | Parked: contested, or their formulations need repair first |
| AGY-04 block-model forensics (450, 340); AGY-03 shadow-price loop (340, 300) | Parked: both rivals scored them low, and AGY-03's chance constraint is unproven |
| AGY-08 data room (400, 180); CC-15 disruption library | Dropped by their authors; CC-15 survives as a cited fixture of quantified events |
| Gemini blind spots: statutory workpapers (IFRIC 20, IAS 2, IAS 36, IAS 37); lender completion tests | Parked to M3 with FA: per-pushback stripping data and credit agreements are not public |

### K.6 Beads added (BD-321 to BD-357)

| Range | Work packages | What they carry |
|---|---|---|
| BD-321, BD-322, BD-329–BD-334 | WP-03 | K.2's pre-push steps, the uncited-number and filed-page checks, the five-state ledger, vintage keys, the public-data waterfall, the forecast-record manifest |
| BD-323–BD-327, BD-357 | WP-00 | Bead and INT record corrections, the close-reason check, INT-AS-001a, `deny.toml` in CI, the CONTRACT template, salted review |
| BD-328 | WP-06 | One refusal registry |
| BD-335, BD-342, BD-346, BD-355 | WP-13 | Issuer reserve rows, the completion test, the JV mirror, the comparability engine |
| BD-336 | WP-10 | Metallurgical-balance import for the recovery-led route; BD-204 now depends on it, so WP-12 depends on WP-10 |
| BD-337 | WP-05 | INT-AS-001 criterion 8 |
| BD-338–BD-341, BD-345, BD-347, BD-349 | WP-03 | Live-dependency proof, reproduction kit, the exact solver, the track record, the lint, the reliance check, challenge packets |
| BD-343, BD-352 | WP-02 | Forecast cohorts, evidence-origin accounting |
| BD-344 | WP-08 | Temporal-cone check, beside the hindcast tool |
| BD-348, BD-356 | WP-12 | Non-interference at the wall, the pilot protocol card |
| BD-350 | WP-15 | Target disclosure checks |
| BD-351 | WP-16 | Untested-ore bounds, beside geomet coverage |
| BD-353, BD-354 | WP-14 | Compensation-formula exposure, insurance recovery evidence |

Edges added to existing beads: BD-065 ← BD-322 (the harness runs on registered inputs only); BD-068 ← BD-334;
BD-074 ← BD-338 (the M0 end-to-end run includes the live-dependency proof); BD-076 ← BD-343; BD-204 ← BD-336. The
generator reports 357 beads, 589 edges, 0 cycles and an unchanged 29-bead longest chain.

### K.7 Alignment with the client proposal (2026-10-30)

The client proposal (October 2026) commits in writing to four public-lane dates: the Q3 2026 workbook read on
9 Nov 2026; per-site Q4 2026 estimates with 80% ranges, time-proved by 31 Dec 2026; the demonstration, cut-off
reproduction included, ready at the end of January 2027; scoring against the FY2026 results in February 2027.
- **BD-067's edges.** BD-047 and BD-059 are replaced by BD-322 (the reader), BD-334's stamped rules and the Q3 2026
  vintage. With the old edges BD-068 sat behind 31 open beads, mostly WP-00/WP-01. BD-334's frame freezes and
  hashes the record itself; BD-047 later ingests it.
- **Priorities.** BD-334, BD-067 and BD-068 are P0 with due dates; the Lumwana gap (bd-srqc) is P1, since the
  proposal's first headline figure rests on it.
- **Open scope.** The proposal says "every cut-off grade stated in a technical report"; BD-274 reproduces seven
  (E182–E184). The owner widens the bead or narrows the wording.
- **Later vintages** stop when the author gains private access (§21.6).
