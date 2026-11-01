# FrankenMining

<div align="center">

[![Status](https://img.shields.io/badge/status-design%20plan%20%C2%B7%20scaffold%20in%20progress-orange)](#honest-status)
[![Runtime](https://img.shields.io/badge/runtime-asupersync%200.5.0-6f42c1)](#oracles-and-pins)
[![Core](https://img.shields.io/badge/core-Rust%202024-b7410e)](#architecture)
[![Plan](https://img.shields.io/badge/plan-485%20beads%20%C2%B7%20E001%E2%80%93E852-2ea44f)](COMPREHENSIVE_PLAN_FOR_THE_DESIGN_OF_FRANKENMINING.md)
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
| Is it evidenced? | **Yes, with a register.** [`planning/mor/MOR_CASE.md`](planning/mor/MOR_CASE.md) holds E001–E852. The corpus rows (E101–E399) were re-read by a second reader against the PDFs and workbooks: 273 exact, 18 confirmed with nuance, 3 page-locator fixes, 0 substantive errors. The research rows (E401–E655) still await a second reader |
| Has it been reviewed? | **Five audits and six hypothesis passes.** AUDIT_1–AUDIT_5 covered domain accuracy, suite fit, internal consistency, a corpus second read and a second-wave review of equations, guards and the bead graph. Brenner passes B1–B6 covered subsurface, prospectivity, planning and capital, substrate, wedge and pre-mortem, and operations, plant, tailings and workforce. Their corrections are folded in (plan Appendix H) |
| Is there a work plan? | **Yes.** 21 work packages and 485 beads with 917 dependency edges, acyclic. Instantiated in `.beads/` for `br` |
| Is there code? | **Yes, and the public lane runs.** `fmine assure public --issuer barrick` runs end to end on the 18 registered workbooks (not in git) and reproduces the state root pinned in `repro/barrick/expected.toml`. The M0 gate is not passed: the M0 report, the security pack and the hashed FY2026 forecasts are still open |
| Has anything been benchmarked? | **No.** Every speed and scale number in the plan is a TARGET |

### Run the public lane

With the pinned Rust toolchain, fetch the registered public workbooks and run:

```bash
bash repro/barrick/fetch.sh
cargo run -p fmine-cli -- assure public --issuer barrick > run.json
```

**FACT, the registered-corpus integration tests:** the Mine Stats Claim inventory contains 22,732
observations, with no mapping refusals. Each Claim retains its publication vintage, period, ownership basis,
unit, raw token, displayed value and source cell/hash. Numeric zeros at registered disposed sites remain
`nil` with reason `disposed`; missing cells, dashes and `n/a` remain distinct.

The JSON payload includes `public_claims.cells` (the canonical AISC and Mine Stats Claims), `mine_stats_claims`
(the Mine Stats census and digest), and `issuer_consistency.mine_stats_vintages` (counts and cited comparisons
against both the earliest registered and previous releases). The comparison tests
reproduce E474 and all eight W1-DR-09 changes, while listing ownership conflicts and changed regional membership
separately. The comparison population reconciles with W1 T16: 6,391 passes plus nine changes, with 30
regional-membership changes listed separately. The nine changes are classified as `restated_silent` because
no applicable verified note is registered. All 30 region-map exclusions cite E461's exact workbook note and
remain `not_comparable`. A difference does not identify which publication is correct, and this classification
does not establish whether an explanation exists outside the registered evidence.

The same engine compares the AISC workbooks under `issuer_consistency.aisc_vintages`: 9,541 appearances,
5,819 series and 3,722 later comparisons, all unchanged in the registered corpus. This includes 359 explicitly
labelled co-product cash-cost rates, kept distinct from the by-product rates used by the identity ledger.

Disclosure coverage comes from explicit `restatement_note` and `restatement_scope` registry rows. Each run
checks the source digest and complete quoted cell text, then matches exact subjects, lines, occurrences and
whole reporting periods. A future note cannot explain an earlier release. Missing, altered or unusable note
evidence leaves a change unassessed rather than silently declaring it unexplained. The JSON reports source
Claims, applicable note citations, the claim-population digest and the checked disclosure-registry digest.

Inspect a comparison's `current[].claim_id` or `first[].claim_id`, then resolve it directly:

```bash
cargo run -p fmine-cli -- claim show clm_REPLACE_WITH_THE_EMITTED_ID
cargo run -p fmine-cli -- claim why clm_REPLACE_WITH_THE_EMITTED_ID
cargo run -p fmine-cli -- claim trace clm_REPLACE_WITH_THE_EMITTED_ID
```

All three views recheck the registered source hashes and refuse missing or altered inputs. `show` returns
the complete source Claim; `why` explains its evidence, knowledge state, colour and actual admission guards;
`trace` follows its source anchors to the registered workbook, cell, URL and digest. Assessments that the
public projection has not made, including clock freshness and flip margin, remain explicitly absent.
Exact Claim IDs emitted by the built-in synthetic run also resolve, retaining their synthetic evidence.
These additions do not constitute a pass of the full M0 gate.

To resolve a Claim against a particular assurance snapshot, use the same issuer and release bound:

```bash
cargo run -p fmine-cli -- claim show clm_REPLACE_WITH_THE_EMITTED_ID --issuer barrick --through 2026Q2
```

The response includes both the assurance root and the keyed Claim projection digest. Missing ids and altered
source files refuse instead of returning a stale cached Claim.

### Check cited numbers in a draft

```bash
cargo run -p fmine-cli -- lint draft.md --issuer barrick --through 2026Q2
cargo run -p fmine-cli -- lint draft.md --issuer barrick --through 2026Q2 --format markdown
```

DESIGN (BD-345): cited mode checks explicit equalities against canonical Claims from one hash-verified source
snapshot. Link the printed number and unit to its exact Claim ID, and state the subject, reporting period,
ownership basis and metric. For example, using the registered North Mara source Claim:

```markdown
North Mara Q2 2026 attributable AISC per ounce sold is [$2,577/oz](fmine:clm_f6e6c9b2ea703557f2a3b2f49b22b1aa).
```

The checker uses exact rounding intervals. It distinguishes value, period and basis mismatches, excessive
rounding, missing or ambiguous citations, and superseded Claims. It accepts only complete supported equality
predicates; negations, comparisons and other prose remain `unchecked`. A matching number cannot compensate for
a wrong site, period, metric or basis. `verified` means agreement with the cited source assertion, not that the
issuer assertion is true or that disclosure is permitted. Publication-time/publicness checks are not part of
this slice; `--through` selects registered vintages, not an information-availability date.

The command reads the draft locally, writes no files, and emits number spans and verdicts rather than whole
sentences. JSON includes the draft and Claim-population digests and the source snapshot root. Findings are data:
a completed lint exits zero even when it finds mismatches. Missing or altered registered source files refuse.
Use `--synthetic` with Claims emitted by the built-in synthetic assurance run for an invented demonstration.
Relative draft paths are resolved against `--workspace`, which defaults to the current workspace.

### Assess a frozen forecast cohort

```bash
cargo run -p fmine-cli -- forecast assess --ledger tests/forecasts/cohort_synthetic.toml \
  --cohort synthetic-demo --as-of 2026-10-31T00:00:00Z
cargo run -p fmine-cli -- forecast assess --ledger tests/forecasts/cohort_synthetic.toml \
  --cohort synthetic-demo --as-of 2026-10-31T00:00:00Z --selector latest-as-of
```

DESIGN (BD-343): this read-only adapter replays an ordered transcript through the existing forecast ledger.
Registrations, cohort freezes, published or missing outcomes, restatements and saved assessments retain their
order. It reports every cohort member, selected outcome citations, missing reasons, observed scores and
whole-cohort completion bounds, plus the frozen assumptions behind the effective sample size. The default
selector preserves the first published outcome; `latest-as-of` reports restatements separately.

The committed example is explicitly synthetic. Transcript times and citations are supplied inputs, not
independently verified time proofs or source documents. This command neither issues the FY2026 forecast set
nor replaces `xtask forecast-record-verify`; it makes the cohort and scoring kernel usable while retaining those
boundaries. Relative transcript paths are resolved against the selected workspace.

### Export evidence challenges

```bash
cargo run -p fmine-cli -- assure challenges --issuer barrick --through 2026Q2 > challenges.json
cargo run -p fmine-cli -- assure challenges --issuer barrick --through 2026Q2 --format markdown > challenges.md
```

DESIGN (BD-349): these packets turn visible AISC and Mine Stats exceptions, including joint inconsistencies,
into cited review material. Each preserves the original finding, source filenames and hashes, displayed values,
Claim references, checked arithmetic, correction alternatives and the evidence needed to close the question.
Packet ids and a packet-set digest bind their content separately from the unchanged assurance root. They are
internal-review drafts: human checks remain unfilled and external use remains disabled. Cross-vintage packets
retain the earliest, previous and current canonical assertions, both comparison results and any applicable
verified note. Periods, units and ownership bases remain distinct. The renderer rechecks the classifications
against the canonical comparison engine before exporting them. On the registered corpus through 2026Q2,
all nine known unexplained changes and all 60 ownership-conflict findings appear once in the exported packets;
E474 retains both the original 1,479 and later 1,474 USD/oz assertions with their source cells. Disclosed-only
changes and non-comparable region boundaries stay in the assurance report. PDF transcription conflicts are
not yet packetized. Use `--synthetic` in place of the issuer selector for an invented known-answer demonstration.

Identity-transition packets additionally preserve corrected, broken or reclassified AISC calculations across
releases, including a change in classification with identical printed values. They keep the original states,
operands, certificates and source Claims for both appearances, and request the evidence or inclusion rule that
explains the transition. Ambiguous earliest releases retain every assertion with no selected first assertion.
Transition classifications and chronology are rechecked against the canonical identity ledger before export.

### Calculate guidance completion requirements

```bash
cargo run -p fmine-cli -- guidance completion --synthetic
cargo run -p fmine-cli -- guidance completion --issuer barrick --site north-mara --through 2026Q2 --as-of 2026-10-31
cargo run -p fmine-cli -- guidance completion --issuer barrick --site north-mara --through 2026Q2 --as-of 2026-10-31 --format markdown
```

DESIGN (BD-342): the public command requires registered, hash-verified documents and two agreeing reader
entries in the local, ignored `registries/transcriptions/<issuer>.toml`. Use the existing transcription schema
with quantities `guidance_<site_key_with_underscores>_production_koz` and
`guidance_<site_key_with_underscores>_aisc_usd_per_oz`, an annual period such as `2026FY`, units `koz` and
`usd_per_oz`, and an explicit ownership basis (`100%` or `attributable:n/d`). The AISC basis must also carry
`;aisc=issuer-reported-per-sold-oz`. Both guidance rows must come from the same latest eligible registered
document. Missing or conflicting evidence refuses; no real guidance values are bundled in the code.

The result reports exact remaining production and an AISC spending/rate frontier. AISC uses summed costs and
sold ounces; it never averages quarterly rates or treats production as sales. Remaining sales explicitly assume
the preceding year's sales for the same remaining months, with source Claims and rounding intervals retained.
The report also tests completion jointly against every usable contiguous historical quarter window of the
remaining duration, including windows crossing a year boundary. Production, sales and AISC come from each
quarter's own original release before the guidance year; later comparatives cannot replace a missing original.
The **T1 historical comparison** keeps each quarter's sales and costs coupled. Separate best-case production
and cost ranges cannot establish a feasible result. A positive result includes an independently checked exact
replay; an exclusion includes checked certificates and applies only to those historical windows under the
declared sales assumption. Missing or incompatible windows have explicit reasons, and no usable history stays
**T3: necessary arithmetic conditions only**. Solver limits remain indeterminate.

The report includes the guidance vintage, as-of selection and all source hashes. Workbook availability uses its
verified retrieval date conservatively where no publication date is registered. JSON preserves exact Claims,
source-quarter witnesses and certificates; Markdown presents requirements, comparison results and citations.
Its content digest creates no forecast registration or ledger certificate, and neither tier gives a probability
or mine-capacity conclusion. The metrics library separately provides an exact feed/grade/recovery ceiling
kernel for cited physical limits. Public technical-report binding and the remaining BD-342/M0 gates stay open.

### Compare Lumwana recovery with its historical grade relation

```bash
cargo run -p fmine-cli -- recovery gap --synthetic
cargo run -p fmine-cli -- recovery gap --issuer barrick --through 2026Q2 > recovery.json
```

The command fits two descriptive models to the annual head-grade and recovery observations in the
2013–2022 part of the Lumwana site-visit presentation's Appendix A, then compares both with the registered
quarterly Mine Stats. M-a holds implied tails grade constant; M-b fits a line relating implied tails grade
to head grade. Both use `tails = head_grade × (1 − recovery/100)`, with grade and recovery explicitly in
percent units. Their calculations and rounding propagation live in the
[recovery metric kernel](crates/fmine-metrics/src/recovery.rs).

The synthetic command runs without external data. To use the Barrick command:

1. Fetch the registered workbooks with `bash repro/barrick/fetch.sh`.
2. Download `Lumwana_Site_Visit_25092025.pdf` and `Barrick_AIF_2025.pdf` from their `[[document]]` URLs in
   [the source registry](registries/sources/barrick.toml), placing them in each row's registered `dir`.
   The command verifies their byte counts and SHA-256 digests. The workbook fetch script does not fetch PDFs.
3. Supply the two agreeing reader entries in the local, ignored `registries/transcriptions/barrick.toml`.
   Each annual head-grade and recovery field must cite the presentation's printed page 39, table
   `Appendix A: Past Production Records for Lumwana`, year row and printed column. The separate ownership
   entry cites PDF page 38 (printed page 37), table `Copper Mineral Reserves`, row
   `Lumwana surface (100%)`, column `Based on attributable tonnes`. The
   [public recovery contract](crates/fmine-public/CONTRACT.md) specifies the quantity keys, units and support;
   the [transcription schema](crates/fmine-registry/src/transcriptions.rs) defines the entry fields.

Only source metadata and hashes are committed. Historical row values and PDFs remain local while
OPEN-R7-1 is unresolved. Missing or conflicting evidence produces a typed refusal; the real command never
substitutes synthetic inputs.

For every available quarter, the report retains the earliest registered visible workbook appearance and
prints both models' expected recovery, signed gap in percentage points, implied copper difference in kt,
and that difference as a fraction of reported production. A fraction can be multiplied by one hundred to
express it as a percentage. The original source Claims, PDF citations, ownership interpretation and model
limitations accompany the results. An unusable quarter remains in the report with its refusal; a later
comparative does not replace it. The available 2023 observations are explicitly labelled as comparative
appearances in the 2024 releases, rather than first-published 2023 results.

The exact rational points have conservative enclosures over the declared input-rounding boxes. These bounds
do not assert statistical coverage. The fit also distinguishes the descriptive residual sample SD, using
`SSE/(n−1)`, from the conventional regression residual-error SD, using `SSE/(n−2)`; their square-root bounds
cover numerical evaluation only. The models omit ore type, grind, throughput effects and concentrate mass
pull, and apply annual training to quarterly observations. Operational attribution, recoverable production,
forecasts and dollar sensitivity are outside this output.

**`--through` limits workbook releases for a retrospective comparison.** The command still reads the current
verified historical model documents, so the bound does not establish what was knowable at that earlier date.
Omit it to use all registered workbook releases. The payload's `fmine.recovery.gap.v1` digest binds the model,
source Claims and results; the CLI envelope has no ledger anchor or certificate. An unbounded
`assure public --issuer barrick` run also includes the optional `recovery` section when these transcriptions
are supplied. The existing bounded assurance command continues to skip PDF transcriptions.

---

## TL;DR

**The problem.** Mining runs on single numbers. A reserve is one tonnage at one grade, and a plan is one schedule.
A grade-control model silently replaces the resource model it was meant to test. Reconciliation factors are
reported but rarely attributed. Capex overruns average about +14%, with about half outside the claimed ±15%
(plan §1). The shocks that actually moved capital in 2025–26 sit outside single-entity spreadsheet models:
- host-state disputes and the loss and return of control of a mine;
- asset exits;
- JV disputes;
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
with **some data** is assurance of the numbers behind guidance and disclosure: AISC, GEO, reserve replacement and
grade and recovery by site (plan §37.1).

- **M0, Public Mirror plus Spine (Nov 2026–Jan 2027).** `fmine assure public --issuer barrick` will:
  - reproduce every per-site value in Barrick's published quarterly Mine Stats workbooks (2024Q1–2026Q2, plus
    Q3 2026 when it is released), each cell cited by `<file-period>:<Sheet>!<Cell>` and file hash;
  - reproduce every cut-off grade stated in a technical report;
  - keep a discrepancy register;
  - hash FY2026 per-site forecasts, with 80% intervals, *before* the February 2027 release.

  Its gate, GATE-BULL-01, has three parts (plan §21.6): every numeric cell in a named denominator extracted equal
  to the published value against an independent read; every identity instance placed in one of five states; and
  issuer consistency across vintages. Zero uncited numbers and byte-identical reruns still apply.
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

## Command surface

```bash
fmine assure public --issuer barrick    # M0: reproduce, cite or refuse every published site metric
fmine claim show <id>                   # a Claim with its evidence class, knowledge state, UU term and provenance
fmine claim why <id>                    # evidence, colour, admission guards and unassessed fields
fmine claim trace <id>                  # source anchors, registered files and unresolved parent IDs
fmine doctor                            # hash chains, pins, LOC budgets, obligations
fmine robot-docs                        # machine-readable docs and NDJSON schemas for agents
```

Every verb is registered in `registries/verbs.toml`. Every command prints human text and, with `--format json`, a stable
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
  cell. One note is held back pending owner review (`planning/corpus/C5`).

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
planning/mor/MOR_CASE.md                                the evidence register (E001–E852)
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
