# R5 — Reusable planning and architecture patterns from FrankenTextiles

Research track R5 for FrankenMining (`franken_mining`). Read-only survey of
`/Users/jemanuel/projects/franken_textiles` (plan, workspace and history) done on 2026-10-28. Nothing in that
repository was modified.

Sources read:
- `COMPREHENSIVE_PLAN_FOR_THE_DESIGN_OF_FRANKENTEXTILES.md` (4,399 lines). §0–§24 read in full (lines
  1–771). §25.1–§25.11 read in full. Sampled: §25.19, §25.28, §25.29, §25.61, §25.64, §25.82–§25.84, §25.94,
  §25.101, §25.115, §25.121–§25.123.
- `AGENTS.md` (operating contract), `README.md` (honest status, budget), `FEATURE_PARITY.md`,
  `docs/spec/06-textile-materials.md`, `Cargo.toml` and every crate manifest, provenance headers of the
  lifted files, `.beads/issues.jsonl`, `git log`.
- Crate-level `//!` docs of the frankensim crates relevant to mining (`fs-matdb`, `fs-material`,
  `fs-evidence`, `fs-uq`, `fs-ledger`, `fs-thermochem`, `fs-geocon`, `fs-dimine`).

Citation convention: "§n" means a section of the FrankenTextiles plan. Quoted phrases are the plan's own
words. Paraphrases stay close to the source.

---

## 1. Executive summary

FrankenTextiles is a plan-first Rust port of a Python agent skill. Its plan is a **technical
constitution**: `AGENTS.md` says "When this file and the plan disagree on a technical rule, the plan wins."
The design rests on a few ideas that recur in every section:

1. **Claims are typed and kept apart** (§3). Parity, Physics, Fit and Performance each have their own
   evidence. "Nothing computed establishes" Fit. Values a model fills in carry a label (`MODEL_FIT`,
   `UNCERTAIN`, `UNSCALED`) and are never presented as measured.
2. **Truth comes from pinned oracles and known answers, never from the candidate** (§4, §11.9, §17).
   Fixtures are "never regenerated from the candidate".
3. **The code refuses instead of clamping** (§6). Typed refusal records carry a stable code, a message and
   ranked repairs. A failed cell counts as failed and is never excluded (§3 rule 5).
4. **Determinism is a design property** (§6, §17.5). Native debug, native release and wasm must produce
   identical digests.
5. **The architecture is budgeted and bounded** (§16). It has ten crates with planned line counts, a
   one-way dependency direction, a pinned dependency allowlist and an explicit forbidden list. Sibling
   code is lifted file by file with provenance headers. Siblings are never workspace dependencies.
6. **Progress goes through gates** (§19, §20). Kill gates have stated fallbacks ("keep the Blender path …
   and say so"). Each phase has an exit gate. "Frontier work never borrows from the parity budget."
7. **§25 adds "one physical authority" and "source-bearing" data**. One canonical material record feeds
   every consumer (mechanics, heat, cutting, sewing, purchasing) without solver edits. Every number keeps
   its unit, method, conditions, evidence class, source locator, date and bounds. A missing property
   blocks only the consumers that need it. Supplier offers, inventory and prices are immutable dated
   observations bound by content hash to the exact material revision.
8. **Delivery is incremental and honest about scope** (§25.5 onward). Each increment says what was
   delivered, which invariants hold, which consumers share it, what evidence exists and what it does
   **not** establish. It names open gates by bead ID. Flags such as `physical_validation_claim` and
   `hardware_execution_authorized` are always false in software outputs.

Main mining translations (detail in §6 of this report):
- The four claims become **Parity / Known answers / Reconciliation / Performance**. Reconciliation means
  F1/F2/F3 factors, survey volumes, metal accounting, bulk samples and pilot plants. Separate
  never-set-by-software flags cover **QP/CP disclosure sign-off** and **operational execution
  authorization**.
- `MODEL_FIT` becomes an **`ESTIMATED`** label for interpolated block values, with estimation variance.
  Warning: "Measured" is a JORC/NI 43-101/S-K 1300 resource category, so the evidence-class vocabulary
  must not use the word "measured" (use `ASSAYED`/`OBSERVED`).
- Source-qualified material records become **assay certificates**: lab, method code, aliquot, detection
  limits, batch/certificate ID and QA/QC (CRMs, blanks, duplicates) status. The same pattern covers
  density determinations, geotechnical specimen tests (UCS, point load, triaxial) with geometry and
  moisture conditioning, metallurgical testwork (Bond Wi with closing screen, leach columns) and supplier
  certificates of analysis for reagents and explosives.
- One physical authority becomes **one geometallurgical domain/material authority**. The same frozen
  domain record feeds estimation (density), slope design, drill and blast, haulage (swell, moisture),
  comminution, recovery, tailings, water and acid rock drainage, procurement and sales (payables and
  penalties).
- The central acceptance scenario ("fixed jacket, fixed body, swap linen → leather → knit") becomes
  **fixed pit/stope design and schedule, swap the ore-domain characterization**. Geometry stays unchanged.
  Every applicable consumer updates. Digests and source hashes change. No stale cached values survive.
- Kill gates become G1 estimation parity, G2 pit/schedule known answers (for example MineLib optima), G3
  an end-to-end planning round through the CLI, G4 browser and G5 gauntlet. Each has a stated fallback.

---

## 2. Codebase facts (task 3)

### 2.1 Workspace

`Cargo.toml` (resolver 3, edition 2024, MIT) has ten members, exactly the plan's §16 table:
`ftx-core, ftx-body, ftx-pattern, ftx-measure, ftx-drape, ftx-export, ftx-trace, ftx-cli, ftx-wasm,
ftx-conformance`. The manifest repeats the budget table as a comment and says: "A crate is added to
`members` when it gets real code, not before. No crate outside this list without explicit approval.
Ceiling: 120,000 lines of Rust including tests."

Workspace lints: `unsafe_code = "forbid"`, `missing_docs = "warn"`, `unreachable_pub = "warn"` and clippy
groups (correctness denied). The workspace dependency allowlist pins exact versions (`=x.y.z`): `serde`,
`serde_json` (with `float_roundtrip`), `spade`, `miniz_oxide` and `wasm-bindgen`. Each dependency has a
justification comment giving the version, the API used, its transitive closure and why it is needed. For
example, `float_roundtrip` exists because otherwise "serde_json may read a decimal one ULP off, which
flipped point-in-polygon verdicts … in the oracle fixtures." `wasm-bindgen` is pinned to match the
installed CLI and is used only in `ftx-wasm`.

Dependency direction in the manifests matches §16:
- `core` has no internal dependencies.
- `pattern`, `body` and `measure` depend on `core` only.
- `drape` depends on `core`, `pattern` and `body`.
- `export` depends on `core` and `pattern`.
- `trace` depends on `core`, `pattern` and `export`. This is a small addition to the plan's diagram.
- `cli`, `wasm` and `conformance` depend on everything.

### 2.2 frankensim: lifted files, not dependencies

**No Cargo dependency on frankensim exists**, neither path nor git. frankensim code is **lifted file by
file**. 22 `.rs` files mention frankensim. Each lifted file opens with a `//! Provenance:` header that
records:
- the frankensim source path, for example `crates/fs-sparse/src/direct.rs`;
- the exact commit (ten distinct frankensim commits across the files, such as `93fc9b26…`, `e76c9421…`,
  `c991e5f9…` and `fe9ae4ea…`);
- the licence ("same author, MIT + OpenAI/Anthropic rider");
- a **"Changes:"** list. `sparse/direct.rs` records, for example, a new `analyze_with_permutation`,
  deferred Schur updates and four-partial-sum solves: "factorization 2.9×, solve 1.3× faster … every
  result bit-identical", "different rounding, the same on every target";
- a **"Left out:"** list (for example `exact.rs` leaves out "the stage telemetry, the other predicates and
  simulation of simplicity");
- after that, the original module documentation.

| Lifted into | From frankensim |
|---|---|
| `ftx-core/src/det.rs` | `fs-math/src/det.rs` (deterministic elementary functions) |
| `ftx-core/src/exact.rs` | `fs-ivl/predicates.rs` + `expansion.rs`, `fs-math/eft.rs` (Shewchuk orient3d) |
| `ftx-core/src/intersection.rs` | adapted from `fs-topo/src/intersect.rs` |
| `ftx-core/src/philox.rs` | `fs-rand/src/philox.rs` (counter-based RNG) |
| `ftx-core/src/response_curve.rs` | adapted from `fs-contact/.../response_curve.rs` |
| `ftx-drape/src/sparse/{direct,mod,precond}.rs` | `fs-sparse` (supernodal LDLᵀ with AMD and inertia, COO/CSR, PCG) |
| `ftx-drape/src/eigen.rs` | `fs-la/src/eigen.rs` (`jacobi_eigh`) |
| `ftx-drape/src/bvh.rs` | `fs-query/src/features.rs` (BVH) |
| `ftx-drape/src/ad.rs` | frankensim nested dual numbers and gradcheck (derivative oracle) |
| `ftx-conformance/src/detaudit.rs` | `fs-detaudit` (determinism audit) |
| `ftx-conformance/src/propcheck.rs` | `fs-propcheck` + `metamorphic.rs` |
| `ftx-conformance/tests/no_platform_libm.rs` | rules from frankensim's `xtask check_libm`/`check_powi` (bans platform libm in deterministic paths) |

The same header convention covers other upstreams. `ftx-trace/src/raster/jpeg.rs` and `webp.rs` cite
libjpeg-turbo 3.1.4.1 and libwebp 1.6.0, with their licence files placed in the crate.

The materials platform did **not** lift code. `docs/spec/06` says the design "follows frankensim's
distinction between material cards, physical consumers and uncertain inputs; no frankensim source code is
copied." frankensim's own `fs-matdb` states the same philosophy: material data is "TYPED …, IMMUTABLE …,
PROVENANCE-COMPLETE", and "every downstream physics claim inherits its weakest load-bearing material
datum".

### 2.3 Size, tests and history

| Crate | Rust lines (all `.rs`) | Files | `#[test]` fns | Plan budget |
|---|---:|---:|---:|---:|
| ftx-core | 5,205 | 15 | 59 | 8,000 |
| ftx-pattern | 48,596 | 118 | 467 | 16,000 |
| ftx-body | 6,002 | 14 | 43 | 9,000 |
| ftx-measure | 18,714 | 52 | 165 | 16,000 |
| ftx-drape | 80,018 | 162 | 707 | 14,000 |
| ftx-export | 72,034 | 110 | 598 | 14,000 |
| ftx-trace | 18,956 | 30 | 38 | 7,000 |
| ftx-cli | 51,544 | 134 | 465 | 4,000 |
| ftx-wasm | 14,327 | 44 | 173 | 3,000 |
| ftx-conformance | 8,667 | 32 | 63 | 9,000 |
| **Total** | **324,063** | 711 | **2,778** | 100,000 planned / 120,000 ceiling |

Other facts:
- 159 fixture files under `tests/fixtures/{oracle,materials,import,photo}`. The oracle subdirectories are
  alter, body, draft, drape, export, measure, pattern, trace, validate and web.
- Python tests in `scripts/` cover price refresh, HFM import and JSON-LD. Node checks live in
  `tools/*.cjs`.
- `docs/` holds about 100 markdown files, mostly one per §25 increment, plus `docs/spec/01…06`.
- `FEATURE_PARITY.md` has about 63 `present` and 14 `partial` rows.
- `.beads/issues.jsonl` has 197 beads: 149 closed, 9 in progress and 39 open.
- There are two binaries: `ftx` and `ftx-materials`. `ftx-materials` lives in `ftx-cli/src/bin/` rather
  than in a new crate.
- The whole materials platform sits as `material_*` modules inside `ftx-pattern`, again with no new crate.
  This keeps the ten-crate rule at the cost of large per-crate budget overruns.
- **The budget was exceeded about 2.7×.** `README.md` admits it in the open with a badge reading "source
  budget exceeded". It cites a frozen audit at `2bc8b402` that counted 262,470 lines and says the §25
  expansion "does not silently waive the ceiling". A maintainer decision remains open as `ftx-onn.13`, and
  "tests and working features will not be removed merely to reduce the count".
- Git has 886 commits from 2026-10-12 ("Plan-first bootstrap: design, behaviour specs, operating
  contract, work graph") to 2026-10-28. Last five:

```
4a11a6b 2026-10-28 chore(beads): retain native export formatting after main sync [ftx-onn.11]
c8f2380 2026-10-28 fix(drape): admit static body surfaces before garment placement (ftx-onn.16)
8cea37b 2026-10-28 docs(beads): record targeted photo editing and browser proof [ftx-onn.11]
095e0d3 2026-10-28 merge origin/main: keep the photo-pair edits with qualified seam exchange
93457ba 2026-10-28 merge origin/main: keep named-pair photo edits with bounded body self-collision
```

The first commit contained only the plan, the behaviour specs, the operating contract and the bead graph.
Code began with commit 3 (`ftx-core`, FTX-01). Commit messages carry bead IDs.

---

## 3. Doctrines in §0–§24 (task 1)

Each entry gives the plan's own definition (closely paraphrased, with key phrases quoted), the section,
and a FrankenMining translation. §6 of this report collects all translations in one table.

### 3.0 Document-level conventions (header, §0)

- **Plan-first header**: "nothing below is implemented unless a later commit says so. Every number in this
  document is a target, a budget or a measurement taken on the reference implementations, never a
  FrankenTextiles result." The plan has a version and date (1.1, 2026-10-13).
- **Three tiers, kept distinguishable "in code, docs and status reports"** (§0):
  - **Solid**: ports of behaviour the reference already has and tests. Success is measured against
    captured oracle fixtures.
  - **Frontier**: new capability the reference lacks and that the project exists to provide. Success is
    measured by known-answer tests and stated acceptance criteria.
  - **Optional research**: ideas that "may never ship". They "never consume budget before the kill gates
    pass".
- **Reading order**: "Read §3 (the four claims) and §19 (kill gates) before anything else."
- **Plan states architecture and decisions; specs state exact behaviour.** `docs/spec/01…05` hold the
  behaviour extracted from the reference, which was *run* to produce them. `AGENTS.md` says: "implement
  from the spec, not from the legacy code. If the spec is wrong or silent, fix the spec first (with the
  evidence), then the code."
- **The core sections are living text.** §11.1 already cites §25.93 and §11.1 describes the later
  `--two-start-sew` flag. Core doctrine sections are amended in place when deliveries change them, while
  §25 grows as a log.

*Mining*: the same header and tiers apply.
- Solid: ports of existing, tested behaviour (the company's spreadsheets and Python, or open references
  such as GSLIB estimation, Lerchs–Grossmann/pseudoflow pit limits and standard comminution equations).
- Frontier: capability the company lacks (an integrated geomet-to-market authority, constrained
  multi-site scheduling, a live reconciliation loop).
- Optional research: ML grade prediction from core imagery, reinforcement-learning dispatch, autonomous
  execution.

### 3.1 Product contract (§1)

The contract is stated as **Inputs**, **Outputs**, **Who uses it** and **What it is not**.
- Inputs are typed by source kind, and each kind has its own provenance and uncertainty. Tape
  measurements come "with provenance and uncertainty". A size-chart body is "clearly labelled as such".
  Fabric defaults are "labelled UNCERTAIN until sourced".
- Outputs include "a verdict with its basis", whose parts are "each reported separately (§3)".
- The "is not" list is explicit: not a CAD editor, not a renderer, not a body-scanning product, "not a
  replacement for a muslin. The muslin and a photo of it on the wearer remain the truth", and not a
  general cloth library.

*Mining*: write an "is not" list early. Candidates:
- not a replacement for a Competent/Qualified Person's sign-off;
- not a fleet-control system that commands equipment;
- not a statutory compliance certificate;
- not a general geostatistics or CFD library;
- not a substitute for reconciliation against what was actually mined and milled.

The equivalents of the muslin are the reconciled production period, the bulk sample and the pilot plant.

### 3.2 Lessons from the reference implementation (§2)

The plan states what the reference "established and FrankenTextiles keeps" and what "does not work, and
FrankenTextiles replaces". Each replacement comes with the measured failure that justifies it ("armpit
sits about 45 mm above", "every girth gains 4.4 cm", "19 MB first load"). Three principles are kept:
- **"The pattern JSON is the editable explanation."** Lengths and ease "are decided on the flat pattern,
  where they are exact. **One authority per number.**" `AGENTS.md` expands this: "lengths and ease from
  the flat pattern; hang, contact and construction from the drape; fit from the muslin."
- **The round loop** with an explicit verdict basis. It includes a measurement gate ("a pattern fitted to
  size-chart guesses fits the chart, not the wearer") and export acceptance checks inside the round.
- **Fail closed**: "an unscaled trace gets no true-size files; a crashed simulation never reports a
  previous run's result; a misspelt style parameter is an error."

*Mining*: "one authority per number" works as an ownership map.
- Tonnes and grade in situ belong to the resource model.
- Diluted and recovered tonnes belong to the mine design and schedule.
- Mill head grade belongs to the plant sampler and metal accounting.
- Payable metal belongs to the sales contract and assay exchange.
- Truth belongs to reconciliation.

The measurement gate becomes a **data-readiness gate**: "a model estimated from analogue-deposit priors
or unvalidated assays fits the analogue (or the lab error), not the orebody." Write the section as a list
of the failures of the current systems (spreadsheets, siloed vendor packages), each with a measured
consequence.

### 3.3 The four claims, never conflated (§3)

| Claim | Meaning | Evidence |
|---|---|---|
| **Parity** | A ported subsystem reproduces the reference on the fixture corpus | Oracle fixtures from the pinned reference; differential tests |
| **Physics** | The solver "behaves like cloth on a body where the answer is known" | Known-answer tests with closed-form or measured thresholds (§11.9) |
| **Fit** | A garment fits a real person | "A muslin or a dressed photo … Nothing computed establishes it" |
| **Performance** | "A stated workload runs in a stated time on stated hardware" | "A benchmark with machine fingerprint, inputs and spread" |

Five rules follow:
1. Passing parity is not evidence that the reference was right. A known quirk is either reproduced "under
   a named flag" or fixed "with a regression test and a parity exception, never silently".
2. "A drape with no gating flags is a prediction about construction and contact, not a fit claim."
3. Model-filled values are `MODEL_FIT` with their own uncertainty and "never masquerade as measured".
4. "No speed claim without a benchmark."
5. "A failure is a failed cell, never an excluded cell. Partial coverage never rounds up."

`AGENTS.md` adds: "Never report them as one number."

**How they are kept apart in practice:**
- Separate evidence types live in separate places: `tests/fixtures/oracle/` for parity, known-answer
  tests for physics, benchmarks with fingerprints for performance, and nothing in code for fit.
- Verdicts name their basis, with each part reported separately.
- `no_claim` lines appear in reports (§11.8).
- Labels on derived values: `MODEL_FIT`, `UNCERTAIN`, `UNSCALED`, `REVIEW_REQUIRED`.
- In §25, boolean flags in outputs are always false: `physical_validation_claim`,
  `hardware_execution_authorized`, `physical_seam_quality_validated` and `geometry_changed`. Prose
  disclaimers sit at the end of every increment.

*Mining (proposal)*:

| Claim | Meaning | Evidence that establishes it |
|---|---|---|
| **Parity** | A ported subsystem reproduces the pinned reference: legacy spreadsheets/Python, GSLIB-family estimation, a pit-limit reference, vendor exports captured as fixtures | Oracle fixtures; differential tests |
| **Known answers** (Physics) | Models behave correctly where the answer is known: mass and metal balance closes; kriging is exact at data locations and reproduces the theoretical variance; ultimate pit limit equals the known optimum on benchmark instances; infinite-slope factor of safety matches closed form; Bond energy, Atkinson ventilation and Hardy Cross networks; conditional simulations reproduce the input histogram and variogram | Known-answer tests |
| **Reconciliation** (the analogue of Fit) | The plan or model agrees with what was actually mined, milled and sold | F1 (grade control vs resource model), F2 (mill vs grade control), F3 (metal accounting vs model); survey volumes; weightometer and truck scales; bulk samples; pilot plants; realized price and cost. **Nothing computed establishes it** |
| **Performance** | A stated workload (estimation of N blocks, a UPL solve, a schedule) runs in a stated time on stated hardware | Benchmark with fingerprint, inputs and spread |

Add **authority flags that software never sets to true**, mirroring `hardware_execution_authorized`:
- `reserve_disclosure_claim` (CP/QP sign-off under JORC 2012, NI 43-101 or S-K 1300);
- `geotechnical_signoff`;
- `execution_authorized` (firing a blast, dispatching autonomous equipment, changing a plant set-point,
  placing waste as non-acid-forming);
- `regulatory_compliance_claim`.

Rule 3 becomes: block values interpolated from data and variogram are **`ESTIMATED`** with estimation
variance and are never "assayed". **Vocabulary trap**: "Measured / Indicated / Inferred" are resource
*confidence categories*. Do not reuse "measured" as an evidence class. Use `ASSAYED` / `OBSERVED` /
`PUBLISHED` / `ESTIMATED`, and treat classification as a separate QP-owned attribute that kriging variance
informs but does not establish.

### 3.4 Oracles and pins (§4)

A table lists Reference, Pin and Used for:
- the Python skill at a git revision;
- Anny at a PyPI version with the wheel SHA-256 in `tools/body-model/PIN.md`;
- frankensim with the "commit recorded per lifted file";
- ipc-toolkit as a "formula cross-check (not vendored)";
- FFmpeg and OpenCV, with "versions recorded in fixtures" and kept as "external tools until a Rust
  replacement passes the same fixtures".

Rules:
- Fixtures are captured from the reference into `tests/fixtures/oracle/` and "**never regenerated from the
  candidate**. A mismatch is a defect until proven otherwise."
- "Moving a pin is a deliberate event: new fixtures, a diff of what changed, and the old results kept."
- "The reference stays runnable alongside the port until the gauntlet … certifies the subsystem."

*Mining*: pin every reference with a version and hash.
- Candidate references (to be confirmed by other research tracks): GSLIB/pygslib or geostatspy for
  estimation and simulation; MineLib benchmark instances with published UPL optima; a pseudoflow
  implementation for pit limits; Open Mining Format (OMF) for interchange; the company's own legacy
  models and spreadsheets captured as fixtures.
- Pin datasets as well as code: public deposits or public technical-report datasets, with hashes. A
  geostatistical "reference wearer" is a public synthetic deposit.
- Vendor packages (Vulcan, Datamine, Deswik, Whittle) cannot be vendored. Treat them like ipc-toolkit:
  formula and output cross-checks captured as fixtures, with the version recorded.

### 3.5 Architecture overview (§5)

The overview is a single ASCII data-flow diagram: body evidence → measure → body; garment source → trace
or pattern → validation → drape → export.
- The CLI runs a **round**: measurement gate → flat validation → drape → exports. It writes "one round
  directory with a verdict that names its basis".
- "The browser page runs the same crates compiled to wasm."
- "**Data contracts are files**": versioned schema IDs such as `fabric-pattern/1`, `fabric-fit/1`,
  `fabric-measurements/1`, `ftx-body/1`, `ftx-drape-scene/1` and `ftx-drape-report/1`.

*Mining*: the data flow is drillholes, assays and surveys → QA/QC and compositing → geological/domain
model → estimate → classification (QP) → pit or stope optimization → design → schedule → plant model →
sales → reconciliation. Make every hand-off a versioned file contract (illustrative names):
`fmine-collars/1`, `fmine-assays/1`, `fmine-qaqc/1`, `fmine-domains/1`, `fmine-block-model/1`,
`fmine-pit/1`, `fmine-schedule/1`, `fmine-plant-forecast/1`, `fmine-recon/1`. OMF is the external
interchange, playing the role GarmentCode/DXF-AAMA plays for textiles. The round is a planning cycle
(monthly, quarterly, budget), with one immutable directory per cycle.

### 3.6 Units, coordinates, determinism and refusals (§6)

- **Units and frames**: pattern files are in millimetres (2D, y up, CCW). Bodies and drapes are in metres,
  kilograms and seconds (3D, z up). "Conversions happen at one boundary each."
- **Determinism**:
  - single-threaded fixed-order assembly;
  - candidates sorted with `total_cmp` and index tie-breaks;
  - "no iteration over hash maps in anything that affects output";
  - deterministic elementary functions lifted from frankensim "instead of the platform maths library
    where results must match across native and wasm";
  - no compiler-contracted FMA in bit-identical hot loops (`AGENTS.md`: an explicit `mul_add` is fine);
  - target: identical final-state digests across native debug, native release and wasm.
- **"Refusals, never silent clamps"**: "typed refusal records with a stable code, a message and a ranked
  list of repairs". The plan lists the cases: non-finite input, degenerate geometry, inward-wound body,
  intersecting start, candidate-budget overflow ("refuse, never truncate"), Newton stall, budget stop
  ("publishes nothing partial"), unscaled pattern asked for true-size files, misspelt parameter.
- **Rounding** follows the reference's ties-to-even behaviour where byte parity is required.

*Mining*:
- **Units and frames** are central. Mine grid, projected CRS and geodetic coordinates each need an
  explicit transform and datum. Track elevation datum, tonnes against short tons, grade units (g/t, %,
  ppm, oz/st), **dry against wet tonnes** (moisture) and **in situ against broken** (swell). Each pair
  gets one conversion boundary.
- **Determinism**: estimation, simulation (counter-based RNG such as philox, lifted), pit optimization and
  scheduling produce identical digests across native and wasm. Sort solver candidates with index
  tie-breaks.
- **Refusals**: unclosed wireframes; overlapping domains; a hole with a collar and no survey; an assay
  with no certificate or QA/QC batch; missing density; a CRS mismatch; negative or non-finite grades;
  below-detection values with no declared policy.
- **Grade capping is exactly the "never a silent clamp" case**. A top-cut is a declared, sourced decision
  with its rationale, recorded like a parity exception. It is never an implicit clamp.

### 3.7 `ftx-core` (§7)

"Shared foundations, no domain logic". It holds vectors, deterministic functions, curves, polygons, shared
JSON types, refusal and report types, and "a **run receipt** (input hashes, versions, parameters, output
hash)".

*Mining*: `core` holds units and quantities (with dimensional checking), CRS transforms, deterministic
maths, geometry kernels (meshes, solids, exact predicates), refusals and run receipts. Every report
carries its receipt.

### 3.8 Reference quirks are decisions (§8)

The spec records each reference quirk with a decision: keep (documented), keep and flag, or **fix**
("refuse with a typed error; parity exception recorded"). "Each decision is a line in
`FEATURE_PARITY.md`." Drafting "reads body values through one interface", so a program drafts the same
way from tape measurements, from a fitted body or from analytic landmarks.

*Mining*: legacy spreadsheet and vendor quirks get the same treatment. Examples: GSLIB angle and rotation
conventions, revenue-factor conventions in pit-shell tools, rounding in reserve tables. Use a single
"read values through one interface" for consumers of grade, density and recovery, so a schedule runs the
same way from an estimate, a simulation realization or grade-control data.

### 3.9 Parametric model ports (§9)

These are frontier/solid hybrids with gates:
- **Port the forward model, not the training code** (§9.2). A one-time pinned conversion script builds
  data from the pinned package and checks hashes. Converted data is "never committed"; "the pin and the
  converter are."
- **Parity gate G1a** sets a numeric tolerance (0.05 mm).
- **"Measuring a body the way a tape does"** (§9.3): the model's measurements are defined like the
  instrument they are compared with ("a girth is the length of a convex hull of a plane section … a tape
  bridges hollows").
- **Fitting** (§9.4) has three levels of evidence: from little information (priors), from tape
  measurements (Gauss–Newton/LM, σ-weighted, with priors) and from photos. Gate G1b: recover synthetic
  targets within a stated tolerance, and on held-out data "residuals reported, never hidden". Unmeasured
  outputs are `MODEL_FIT` with σ.

*Mining*:
- The forward model is the geological/implicit model and the estimate.
- "Measure the model the way the instrument does" is the **change-of-support** principle. Compare model
  grades at the support of the samples or the mill: composite length, SMU, mill sampler, truck scale.
  Compute stockpile volumes as the drone survey does.
- Synthetic recover-the-parameters gates generate a field from a known variogram, sample it, and recover
  variogram and grade within tolerance.
- Report residuals (cross-validation errors) and never hide them.

### 3.10 Measurement and photogrammetry (§10)

- **Port order** is explicit, following a research file.
- The **frontier design** lists its own failure modes. Data lacks identity unless frames are named by
  timestamp ("never a frame number"). Gates have thresholds (ray angle ≥ 3°, reprojection ≤ 3 px).
- "**Reports what the data cannot separate**": "the multistart pattern reports such ambiguity instead of
  picking one."
- "**External tools stay external**" (FFmpeg, OpenCV) until a Rust replacement passes the same fixtures.
  Optional ML models (keypoints, masks) are user-installed ONNX files, "never bundled".

*Mining*:
- Identify data by immutable keys: hole ID plus from/to depth plus sample ID and certificate, never row
  order.
- Report non-uniqueness: alternative geological interpretations, geophysical inversion ambiguity,
  conditional simulations instead of one "best" model.
- External tools (vendor packages, LIMS, fleet management systems) stay external behind fixture-tested
  adapters.

### 3.11 The frontier solver (§11)

- **"What it must fix" table** (§11.1): each failure of the reference (Blender) is mapped to the mechanism
  that fixes it. Equally explicit about "**what it does not fix**": non-convex energy means local minima.
  The mitigation is an opt-in **two-path sensitivity check** whose "spread is review evidence, not an
  error bar, confidence interval or uniqueness proof". "Ordinary runs remain one-path and must say that
  path sensitivity was not checked."
- **§11.2 Energy and step**: exact maths. **§11.3 Schedule**: sew → drape → settle → optional static
  solve.
- **§11.5** is the lifted-code table.
- **§11.6**: metrics and flags are preserved from the reference with the same gating/advisory split; "new
  quantities are added, not substituted".
- **§11.7**: "**Performance targets (not measurements)** … these targets are budgets until benchmarked."
- **§11.8**: file formats, including a report with a **`no_claim` line**. Reference formats are kept "so
  existing tools keep working".
- **§11.9**: ten numbered known-answer tests with closed forms (ring on a cylinder holds iff `W < 2πμT`
  within ±10 %; Peirce cantilever; ease sweep; determinism; bounded two-start spread).

*Mining*:
- Frontier solvers include geomechanics, blast fragmentation, comminution and flotation circuits, haulage
  simulation, ventilation and water networks, and integrated scheduling.
- For each one, write the failure-to-mechanism table against current tools and a "what it does not fix"
  paragraph. Examples: MIP schedules have optimality gaps; one estimate is one realization. The
  mitigation is an explicit multi-realization or multi-start check, reported as review evidence, not an
  error bar.
- Add a numbered known-answer list per solver.
- Keep legacy KPI and flag definitions unchanged; add new ones beside them.

### 3.12 Exports (§12)

- Reproducibility over wall clock: "creation dates come from the pattern's `created` field, so every file
  is reproducible".
- One shared vertex list across outputs.
- "A missing listed file fails its check instead of crashing the checker."
- Deterministic name-collision suffixes.
- DST output is "verified against an independent decoder".
- The checks are written as **acceptance checks** (51 of them).

*Mining*: exports (DXF/DWG pit and stope designs, OMF, block-model CSV, vendor string and triangulation
formats, GeoJSON/Shapefile for permits, regulator report tables) carry the same rules: reproducible
timestamps from the model, independent re-reading, and acceptance checks per file.

### 3.13 Tracing and the "UNSCALED" state (§13)

Traced patterns "stay `UNSCALED` until scale-fit; exports refuse them". The vision-language step "stays an
agent/API step outside the crate".

*Mining*: digitized historical paper plans (old workings, voids, level plans, scanned logs) stay
**`UNREGISTERED`** until control points tie them to the mine grid. True-coordinate exports refuse them.
LLM or OCR reading of scanned logs stays an agent step outside the crates, and every result is marked
`REVIEW_REQUIRED`.

### 3.14 CLI and the skill boundary (§14)

- `ftx` mirrors the reference's `fab` commands "so the skill's workflow does not change".
- Human text by default; "`--json`, a stable JSON envelope".
- Documented exit codes: **0** converged or pass, **2** not converged or flags, **3** refused input,
  **4** budget stop.
- The round keeps a status precedence: `BLOCKED → UNSCALED → flat status → drape gating → export checks →
  measurement gate → CONVERGED`. It has a basis line and `rounds.jsonl`.
- **The skill boundary**: "the skill keeps the workflow, judgement and documentation … Its Python scripts
  become thin wrappers that call `ftx` subsystem by subsystem as each reaches parity, and are retired only
  after the gauntlet certifies that subsystem." Image generation and Blender stay in the skill.
- `AGENTS.md`: library crates do not print; refusals are structured; solvers report per-step telemetry
  into their report, "never per-iteration JSON events in hot loops".

*Mining*:
- One binary with subcommands that mirror the planning workflow: `data`, `qaqc`, `domain`, `estimate`,
  `classify-report`, `pit`, `design`, `schedule`, `plant`, `sales`, `recon`, `round`, `report`.
- The same `--json` envelope and exit codes.
- Status precedence for a planning round: `BLOCKED → UNREGISTERED → data-readiness (QA/QC) → model
  validation (swaths, global mean, cross-validation) → design gating → schedule feasibility → export
  checks → reconciliation gate → CONVERGED`.
- The **skill boundary** puts judgement (geological interpretation, QP classification, when to drill
  more, strategy) in agent skills and human workflow. Deterministic mechanics live in the binary.
- **Name-collision warning**: `fmn` is already franken_manim's CLI. Choose a different binary name and
  crate prefix (for example `fmine`, or a crate prefix such as `fmi-`) and check the sibling projects.

### 3.15 The browser (§15)

"The same crates compiled to `wasm32-unknown-unknown` with a thin `wasm-bindgen` boundary (numerical
crates never see browser types)." Large data (the converted body model) "is fetched once from the user's
own installation … never bundled with a person's data". There is a gate (G4). §18 adds that the browser
"uploads nothing".

*Mining*: a field or office browser workbench runs the same engines: reconciliation views, schedule
what-ifs, material-swap studies. Block models are large, so the "fetch from the user's own installation"
rule applies, and nothing is uploaded.

### 3.16 Crates, budgets, dependency direction and policy (§16)

- "**Ten crates, not a tower of tiny layers. A crate joins the workspace when it gets real code. No crate
  outside this table without explicit approval.**"
- A table of crates with responsibility and **planned Rust lines including tests**, plus contingency and
  a hard ceiling (100k + 20k = 120k).
- Dependency direction is drawn as a small diagram. Cycles are avoided through traits ("measure uses body
  for model fits through a trait, not a cycle"). `AGENTS.md`: "If you need to violate the direction, stop
  and redesign the API."
- **Dependency policy**: "a small pinned allowlist", with uses scoped ("files only, never inside a solver
  loop"; "in `ftx-wasm` only").
  - **Forbidden** in library crates: Python at runtime, PyTorch, BLAS/LAPACK, C/C++ FFI, Tokio ("nothing
    here is async"), a mandatory Rayon pool ("parallelism, if any, behind a feature with a deterministic
    reduction").
  - "Each candidate crate records its version, used API, wasm build result, transitive closure and
    demonstrated benefit."
- "**Sibling Franken projects are resources, not assumptions: frankensim code is lifted file by file with
  provenance, not depended on as a workspace.**"

Observed reality (§2.3 of this report): the crate count held, but the line budget did not, and the
overrun is disclosed.

*Mining*: an "operating system for an integrated mining company" is far broader than one skill's
mechanics. Two options:
- (a) A crate table per domain with honest budgets: core, data/QA-QC, geology/estimation, geotech, mine
  planning/optimization, scheduling, processing/geomet, materials and commercial (procurement and sales),
  reconciliation, exports, cli, wasm, conformance.
- (b) A first release slice with a narrow ceiling, and further domains added only with explicit approval
  and a recorded budget amendment.

Either way, write the **overrun protocol** into the plan from day one (textiles had to improvise it). Keep
the forbidden list. "Nothing here is async" may not hold for live data adapters. If so, confine async to
an adapter crate that publishes immutable snapshots and never enters a solver (see §25.1 below).

### 3.17 Verification program (§17)

- **§17.1 Oracle fixtures (Parity)**: capture scripts in `tools/oracle/` write
  `tests/fixtures/oracle/<subsystem>/<case>.json` with inputs, outputs, reference pin and capture date.
  Specs list the cases. **Comparison classes**: "byte-exact, canonical-structural (parsed and
  normalised), tolerance (stated per quantity)".
- **§17.2 Known answers (Physics)**: §11.9 plus synthetic recover-the-parameters tests and synthetic
  scenes with exact answers.
- **§17.3 Properties and metamorphic relations**: mirror symmetry; invariance under rigid motion; scaling
  a pattern and body together leaves ease ratios unchanged; reordering seams leaves the verdict
  unchanged; export and re-import round-trips.
- **§17.4 Feature parity tracking**: `FEATURE_PARITY.md`, "one row per reference feature with `present |
  partial | missing | excluded` and the fixture IDs that prove it. Partial never rounds up."
- **§17.5 Determinism lanes**: "Two native runs, native debug versus release, and wasm in Node versus
  native must produce identical digests."
- **§17.6 The gauntlet (release gate G5)**: the `running-the-gauntlet-on-your-rust-port` method, in
  `gauntlet-full` mode for ports and `gauntlet-greenfield` mode for frontier work. It uses a sibling
  `…__gauntlet_workspace/` and three negative ledgers (performance, conformance, surface). It needs "at
  least ten rounds with two consecutive clean rounds and every open hypothesis resolved". "It runs when
  there is a port to certify (after Phase 2), not before."

*Mining*: metamorphic relations are natural here.
- Rotating or translating the coordinate frame leaves tonnes and grade unchanged.
- Permuting drillhole order leaves the estimate unchanged.
- Scaling all prices by k leaves the pit shell unchanged when costs scale too.
- A finer block discretization converges.
- Splitting and merging stockpiles conserves metal.
- Exports round-trip through OMF.

A `FEATURE_PARITY.md` row exists per legacy capability (spreadsheet tab, vendor report). Determinism
lanes cover estimates, pit shells and schedules.

### 3.18 Privacy (§18)

"The usual wearer is a young child." No real person's data is ever committed: fixtures are synthetic or
use a public-prior reference wearer. Everything runs locally. User data lives in user workspaces. External
models are opt-in with licences shown.

*Mining*: the sensitive classes are different but the doctrine carries over.
- **Unreleased drill results and resource data are material non-public information** (insider-trading and
  disclosure risk).
- Worker health, fatigue and location telemetry.
- Community, landholder and **Indigenous cultural-heritage site data**.
- Commercially confidential contracts (offtake terms, smelter TC/RCs).

Fixtures must be synthetic or public datasets, with nothing in tests, docs, examples or commit messages.

### 3.19 Kill gates (§19)

| Gate | Criterion | Fallback |
|---|---|---|
| G1 Body | (a) forward model within 0.05 mm of reference; (b) fitting recovers synthetic targets (3 mm girth / 2 mm length) | "ship the body through a converted mesh cache … and reopen the browser plan" |
| G2 Drape | §11.9 tests 1–7, and the converged garments settle on fitted bodies with no gating flags within the §11.7 budget | "keep the Blender path for draping and say so; **do not ship a solver that is merely different**" |
| G3 Port slice | patterns, validation, drafting and exports pass oracle fixtures; the skill's round runs end to end through `ftx` | — |
| G4 Browser | wasm page drafts, validates and exports with parity, and drapes a small garment | — |
| G5 Gauntlet | convergence per §17.6 before 1.0 | — |

Kill gates are numerical, early, falsifiable, and come with fallbacks that keep the product shippable and
honest.

*Mining (proposal)*:

| Gate | Criterion | Fallback |
|---|---|---|
| G1 Resource | Estimation parity against the pinned reference within stated tolerance on fixtures; synthetic recover-the-field test | Keep the external estimation package as the source; import its block model as an oracle-pinned input; say so |
| G2 Optimization | UPL/pit limits equal known optima on benchmark instances exactly; schedule bounds and gaps reported honestly | Keep the vendor optimizer; do not ship an optimizer that is merely different |
| G3 Planning round | Data → QA/QC gate → model → pit → schedule → exports → reconciliation runs end to end through the CLI on fixtures | — |
| G4 Browser | The workbench runs the same engines with identical digests | — |
| G5 Gauntlet | Convergence before 1.0 | — |
| (G-geomet) | One domain-substitution acceptance on a fixed design (see §25.1 translation) | — |

### 3.20 Phases (§20)

A table gives **Phase / Content / Exit**. Phase 0 is the workspace, core and the oracle capture harness,
plus the hardest frontier kernel without its hardest feature (the drape core without contact). Phases 1–5
build up to the gauntlet. "Build Phase 0 first. **Frontier work never borrows from the parity budget, and
optional research waits for G2.**"

*Mining*: Phase 0 is the workspace, core (units, CRS, determinism), the oracle capture harness,
source-qualified data records (collars, surveys, assays, QA/QC), and the estimation kernel without
anisotropy or unfolding. Phase 1 covers domains, estimation, the QA/QC gate and pit limits. Phase 2
covers design, schedule, exports and the CLI round. Phase 3 covers processing/geomet and the material
authority. Phase 4 covers commercial (procurement, sales, payables) and the browser. Phase 5 is the
gauntlet.

### 3.21 Work breakdown (§21)

24 units `FTX-01 … FTX-24`. "Work units … are the Bead titles." Each unit names the gate it closes, for
example "(G1a)" or "(G3 with FTX-18)".

*Mining*: use `FMI-nn` style units mapped 1:1 to beads, each naming its gate.

### 3.22 Risks, exclusions, glossary (§22–§24)

- **Risks** are stated with mechanism and mitigation, including data gaps ("No sourced … parameters …;
  all UNCERTAIN. A research pass … is required before fit-sensitive gates"). Undecided items are
  "Decided at G4, measured, not guessed." "The reference keeps moving … pins and fixture refreshes are
  deliberate events."
- **Explicit exclusions** are "Not ported, by decision". Non-commercial models may be "opt-in user
  installs with a licence notice, never bundled".
- **Glossary**: short domain definitions, including the project's own labels (`MODEL_FIT`).

*Mining risks to state up front*:
- Sparse rock-property and metallurgical testwork data, all UNCERTAIN until sourced.
- Moving legacy references.
- Vendor lock-in and file formats.
- Data-confidentiality and MNPI.
- The scale of block models in wasm.
- MIP solver licences, which would conflict with the "no C/C++ FFI" rule.

Exclusions might cover real-time equipment control, statutory sign-offs and proprietary solver bundling.

---

## 4. §25: one physical authority and source-bearing data (task 2)

### 4.1 Direction and the "one physical authority" (§25.1)

- **Scope expansion stated explicitly**: §25.1 "deliberately expands the garment-only scope stated in
  §1.4; it does not weaken the parity, physics, fit or performance claims in §3." Future robotics is
  framed as "future consumers of a verified assembly/process model, not a claim that exporting a cutter
  outline is already a manufacturing robot."
- **Central acceptance scenario**: "a fixed jacket pattern on a fixed fitted body. Replace characterized
  linen with characterized cow leather, then with a characterized performance knit/blend. **Without
  editing solver code, all applicable consumers must use the selected material**": inertia, directional
  response, grain rotation, bending, thickness, contact and friction, sewing and gathers, strain and fit
  reports, insulation, purchasing and manufacturing constraints. "Pattern geometry remains fixed unless an
  explicit subsequent design optimization changes it."
- **Run records**: "Every run records the resolved material snapshot, source evidence, estimate policy,
  model limits and digest; **no stale cached defaults survive a substitution**."
- **Capability-scoped gaps**: "**A missing property blocks only consumers that require it and is reported
  as unknown to the others; an incomplete commercial record is not made simulation-ready with invented
  values.**"
- **Boundary**: "Keep **one canonical material-resolution boundary** within the existing crates and front
  ends; **external data collectors publish immutable inputs, never participate in the solver's time
  stepping.**" The frankensim materials-card/consumer boundary is "an architectural reference". Shared
  numerics are lifted "only with pins, provenance and tests". Other projects' catalog ideas are
  "inspiration, not empirical textile evidence".

Later increments repeat the doctrine in their own words:
- "No second mass or scoring authority is introduced" (§25.121).
- "A new product must not require a brand-specific branch in mechanics, heat, cutting, sewing or
  purchasing" (§25.11).
- "One canonical material authority continues to feed mechanical, transport, procurement and process
  consumers, each with its own readiness and missing-data report" (§25.9).
- "Build the operator workflow rather than a second database or browser-only physics implementation"
  (§25.9).

**Mechanics of the authority** (from §25.5, §25.10, §25.11 and spec 06):
- `ftx-pattern::materials` owns `ftx-textile-catalog/1` records.
- `ftx materials list | show | compare | freeze`. `freeze` "emits a content-locked, portable selection
  compatible with the existing `fabric.material` resolver".
- Studies record `frozen_changes` that "replay against the original pattern". "Every variant starts from
  the original, never the preceding variant."
- Portable reports "refuse modified receipts rather than resolving a newer catalogue entry by name"
  (§25.6).
- Browser inputs that change "withdraw dependent outputs" or "cancel and invalidate" (§25.19, §25.64).

### 4.2 Source-bearing and source-qualified data (§25.2, §25.3, §25.7, §25.10)

**Hierarchy, not conflation** (§25.2): "Represent distinct levels rather than conflating a fiber with a
finished fabric": fiber, yarn and thread, sheet, hide and finish, laminate, fill, and the garment's
ordered layer assembly. "Stable identifiers distinguish supplier, product, variant, batch, construction,
finish and conditioning." "A bamboo-derived viscose yarn is not silently relabelled bamboo bast." "A brand
name or four-way-stretch claim is not a measured constitutive law or a raw-fabric sales quote."

**The observation record** (§25.2): "The database is **sparse and extensible, not a table padded with
zeroes**. Each quantitative observation needs its **unit, method, direction, specimen/conditioning
context, measured/published/estimated evidence class, source locator and retrieval/measurement date, plus
bounds or uncertainty when reported**." "Preserve original units and ranges alongside normalized values.
**Do not choose an unreported range midpoint or turn a confidence interval into a tolerance.** Track
source rights and permitted retention; prefer primary supplier specifications, open research datasets and
reproducible specimen measurements." Spec 06 adds: "Unknown is an absent property, not zero; a misspelled
material identifier never falls back to a cotton-like default", and "Estimates require explicit opt-in."

**Gates, not suggestions** (§25.3):
- "Positive-definiteness, finite values, dimensions and explicit estimate admission are gates, not
  suggestions. Bulk fiber modulus in Pa is not sheet stiffness in N/m."
- "Until implemented, heterogeneous assignments must refuse rather than silently use the global
  material."
- Claims are ordered: "Derivative checks, energy/objectivity tests, known-answer patch tests and measured
  drape/stretch validation precede claims that a construction descriptor alone predicts fabric behavior."

**Condition qualification** (§25.6, §25.7):
- "Preserve specimen identity, test geometry, original mean and separately labelled standard deviation."
- "Distinguish sample variability from uncertainty of the mean and confidence intervals."
- "Interpolation is an explicit fitted/estimated operation with source provenance and validity domain;
  never extrapolate silently, mix dried and unconditioned data …"
- "Interpolation is an estimated modeling choice even when its knots were measured."
- "Companion mass/geometry observations must match the specimen and conditioning."
- Validity domains are enforced. For example, the "FIT-only raw-Green strain box" is carried into solver
  trials, which "cannot extrapolate outside it". "Held-out samples never widen the acquisition range"
  (§25.28).

**Specimen identity** (§25.10): "One specimen measured by three characterization methods remains one
specimen, not three independent fabrics." "AI-derived CAD coefficients are not relabeled measured."
"Tensile elongation at a reported load is not breaking strain."

**Held-out acceptance** (§25.7, §25.10): "Do not tune against the held-out acceptance case." "A physical
validation failure is not repaired by silently fitting the held-out case or replacing unknown data with a
generic fiber constant." "Reproduce the published observations before deriving material-law parameters."

**Screening and optimization bound to qualified records** (§25.10 `material_screen`/`material_study`):
- Scalar constraints apply only with "exact method/condition restrictions and explicit estimate consent".
- "Nominal comparison and conservative reported-interval comparison are explicit alternatives. Missing
  bounds never become zero-width intervals."
- "Pareto dominance requires all objectives to be no worse, at least one strictly better, and matching
  full reported protocols unless cross-protocol comparison is explicitly admitted. Ties, overlapping
  intervals and incomparable protocols remain visible; **there is no invented aggregate textile score**."
- Downstream consumers rerun independently: "a thickness-based sewing refusal does not erase valid
  cutting, missing width does not prevent sewing, and absent prices or incomplete mass coverage remain
  unknown."
- "Scalar-frontier pruning before manufacturing requires explicit consent, since a scalar-dominated
  textile can still have a better marker or whole-order cost."
- Named operating cases (§25.64): "different environments are not silently averaged, assigned
  probabilities or treated as one experimental protocol."

### 4.3 Purchasing bound to actual qualified records (§25.4, §25.11, §25.29, §25.61, §25.101)

- "**Purchasing remains a source-observation problem separate from material physics.**" "Compare only
  matching currency, region, quality, sale basis and tax/shipping terms; convert linear yard/metre prices
  through sourced usable width; retain order minima/increments and sample counts."
- "Distinguish observed-offer p10/median/p90 from supplier-balanced statistics and from a representative
  market survey." "**Never infer exchange rates, hide yield or unknown shipping costs.**" "One offer's
  p10/median/p90 is a one-observation summary, not a world-market distribution." "An out-of-stock
  observation remains unavailable."
- **Immutable evidence**: "Refresh must preserve raw evidence, timestamps, source policies and old
  snapshots on failure." "New runs retain immutable dated quotes and raw evidence separately; no previous
  observation is overwritten or made current by changing its date." "Intraday quote-history semantics
  require a deliberate schema extension, not timestamp rewriting." "Source changes refuse publication;
  they never silently reinterpret a half-metre price as a full-metre price."
- **Binding to revisions** (§25.29):
  - A multi-observation bracket (HTML, JSON, HTML) must agree on product, variant, SKU, currency and
    price.
  - "Numeric inventory is admitted only when all three observations agree"; "Unreported capacity stays
    unknown; a newly reported quantity requires review."
  - "**Canonical material hashes bind every offer** through the normal pricing and whole-order capacity
    evaluator."
  - "Nominal width does not establish usable cutting width, and total inventory does not establish one
    continuous cut."
  - "Quotes neither reserve inventory nor establish delivery, landed cost or worldwide price
    distributions."
- **Supplier revision review** (§25.11, `ftx-sr2`): compare newly observed specifications "against the
  exact retained product revision before accepting updates"; "require explicit review for changed
  identities or specifications"; "distinguish product/SKU from measured specimen/lot".
- **Bounded multi-supplier search** (§25.61):
  - "The bounded search reports a complete feasible selected order separately from a proved minimum."
  - "Budget exhaustion preserves a feasible incumbent without claiming optimality; an incomplete order
    never supplies a cheap subtotal."
  - "Study requests reserve the declared work across every alternative against one aggregate cap."
  - "Aliases cannot create duplicate stock."
- Sources obey robots rules and rights (§25.11). Primary sources are cited with DOIs (§25.10, §25.83).

### 4.4 Manufacturing and production bound to qualified records (§25.4, §25.8, §25.84, §25.94, §25.115)

- "**Keep geometric export acceptance separate from process feasibility and from permission to operate
  hardware.**" "An unknown coating, finish or ignition characteristic cannot authorize thermal cutting or
  processing." A robot adapter "needs machine-specific capability limits, calibration, workholding/sensing,
  collision checks, interlocks and an explicit execution boundary."
- Sewing plans resolve "the same per-piece selections used by draping and cutting". "A material-only
  change can therefore invalidate machine feasibility without solver or pattern edits." Unknowns refuse.
  "The sewing output always sets `hardware_execution_authorized` and `physical_seam_quality_validated` to
  false."
- **Production as bounded optimization and replay** (§25.84):
  - Copy and operation expansion with scoped dependencies, and a bounded solver with "complete baseline
    and selected schedules, lower bounds and honest incumbent/budget status".
  - "Cutting, thread purchasing and sewing scheduling share a reconciled production count … Conflicting
    quantities refuse the affected sewing consumer while independent cutting remains available."
- **Parallel cells** (§25.94):
  - "a shared operator is not silently multiplied";
  - "Aggregate machine work remains distinct from elapsed completion";
  - "One shared parallel work cap covers all nested … schedules";
  - "The old outer serial report remains a reference and cannot supply the selected parallel time."
- **Feasibility before optimization** (§25.115): "A budget limit before any complete schedule remains
  unresolved; exhaustive failure is scoped to the declared finite scheduling model. Once found, a complete
  incumbent survives later budget exhaustion."
- **Treatments** (§25.83):
  - Before and after states are bound to an "exact specimen, process, cycle count and before/after
    conditioning".
  - "no original coefficient or mass conservation is inferred"; "Five-cycle observations are one complete
    transition, not an invented per-cycle law".
  - The output is "an ordinary frozen pattern, so the existing … authorities consume its changed geometry
    and material without special-case physics".

### 4.5 Recurring §25 invariants (vocabulary to reuse)

The same phrases recur across increments. They work as a house style:
- "without editing solver code"
- "no second X authority / database / implementation"
- "remains unknown / unavailable / open"
- "refuse rather than …"
- "never silently"
- "source-locked / content-locked / frozen / exact replay"
- "bounded"
- "honest incumbent/proof status"
- "one aggregate cap"
- "independent consumer failures"
- "X is not Y" (for example "feed-only time is not robot cycle time")
- "software propagation, not measured validation"
- "does not establish …"
- "the browser displays the Rust frontier … rather than computing another score"

### 4.6 How §25 increments are written (the delivery cadence)

**Macro-structure.**
- §25.1–§25.4 state the requirements of the new direction.
- §25.5 "Delivery state and next acceptance gates" records the first commits and the next gate.
- §25.6–§25.123 (118 subsections in about two weeks, 2026-10-14 to 2026-10-28) are an append-only
  delivery log. There is roughly one subsection per delivered bead slice. Periodic "remaining priorities"
  subsections (§25.82) reset the gate ordering.

**Title style.**
- Early titles: "X: delivered path and remaining (physical) gates" (§25.6–§25.8).
- Later titles: one declarative sentence saying what is now true, phrased around the real consumer. For
  example "Supplied and condensed liquid changes the actual moving garment", "Find feasible production
  orders before optimizing scarce fixture use", "Public supplier observations bound to actual textile
  revisions".

**Paragraph template** (consistent from about §25.9 onward):
1. **Bold lead with the bead ID and its parent gate**, for example "**Delivered contact-data propagation
   (`ftx-if1`, supports `ftx-tm3`/`ftx-tm8`).**" Or the opening sentence frames the delivery as an
   extension of an *existing* authority: "The existing accepted-step liquid authority now admits …".
2. **Invariants and refusals.** Which inputs must match, what is preserved, what refuses, what stays
   unknown, and what is *not* inferred. This is usually a run of short "X is not Y" sentences.
3. **Consumers.** Native commands (`ftx …` and `ftx-materials …`), portable and wasm APIs and the browser
   workbench share one engine, and outputs are invalidated when inputs change.
4. **Evidence and scope.** Commits, test counts (early on, for example "1,127 Rust tests …, strict
   Clippy, wasm32 compilation, 49 Python and 18 Node tests"), the kind of tests (synthetic, known-answer,
   real-binary regression), a doc link (`docs/<topic>.md`, with executable examples under `examples/`),
   and a closing **disclaimer**: "These are software checks, not measured material, garment, price-market
   or machine-performance validation."
5. **Status and remaining gates.** "This implements `ftx-xxx.n`; … remain open under …". Sometimes a
   numbered "**Next highest-impact gates**" list follows. Each item has a bead ID, a priority (P0/P1), a
   "supports" relation and falsifiable acceptance criteria. Typical criteria: held-out comparison,
   matched lots, "do not tune against the held-out case".

Dataset tranches follow the same template, plus corpus counts. The catalog grew 41 → 81 → 96 → 166 → 275
records, each tranche with a DOI or URL, access date and source digest, and "partial observations, not a
comprehensive characterized library".

**Why it works.** Every increment is small, ends in a real consumer, extends an existing authority and
leaves the large empirical gates open by name. The blocking empirical gate (`ftx-tm2`/`ftx-tm8`, matched
linen/leather/knit characterization) is repeated in nearly every increment, so software progress is never
mistaken for physical validation.

**Cost of the pattern (observed).**
- §25 is about 3,600 of the plan's 4,400 lines. The plan has become a changelog.
- The repetition is deliberate but heavy.
- The crate budget was exceeded about 2.7×.
- The direction was set by §25.1 expanding §1.4's "is not" list.

For FrankenMining: keep the template, but consider moving the delivery log to `docs/delivery/` or a
`PLAN_DELIVERY_LOG.md`, with a short status table in the plan. Write the scope-expansion and
budget-amendment protocol in advance.

---

## 5. Further notes from the operating contract (`AGENTS.md`)

- "Project Truth Sources": the plan, the specs and `FEATURE_PARITY.md`.
- "Honesty Rules Specific To This Project" (seven rules), among them "No fabric number without a source"
  and "Report residuals, never hide them".
- "Core Invariants": "One authority per number"; "The verdict names its basis"; "Every round is a new
  directory; nothing overwrites a user's previous round".
- "Code Editing Discipline": no script-based rewrites, no file proliferation, no new crates without
  approval. "Backwards compatibility: none owed to our own pre-1.0 APIs; full compatibility owed to the
  file formats … and to the skill's round semantics."
- Comments "document invariants, units, coordinate frames and no-claim boundaries".
- Multi-agent operation: commit to `main`, Agent Mail reservations and Beads IDs, "never revert or stash
  another agent's changes".

---

## 6. Mining translation table (task 4)

| # | FrankenTextiles pattern (section) | FrankenMining translation |
|---|---|---|
| 1 | Plan-first header; numbers are targets or budgets (§header) | Same header. Every tonnage, NPV or throughput figure in the plan is a target, a budget or a reference measurement, never a FrankenMining result |
| 2 | Solid / Frontier / Optional research tiers (§0) | Solid: ports of legacy spreadsheets/Python and standard methods (OK/IDW, LG/pseudoflow, Bond). Frontier: integrated geomet-to-market authority, uncertainty-aware scheduling, live reconciliation. Optional: ML from core imagery, RL dispatch, autonomy |
| 3 | Product contract with an "is not" list; the muslin is the truth (§1) | Not a QP sign-off, not equipment control, not a statutory certificate, not a general geostatistics library. The reconciled production period, bulk sample or pilot plant is the truth |
| 4 | Lessons from the reference: keep vs replace, with measured failures (§2) | Inventory the failures of current spreadsheets and siloed vendor packages with measured consequences (reconciliation gaps, version drift, manual transcription) |
| 5 | One authority per number (§2, AGENTS) | Resource model owns in situ tonnes and grade; design and schedule own diluted and recovered; plant sampler and metal accounting own head grade; contract owns payable; reconciliation owns truth |
| 6 | Four claims never conflated (§3) | Parity / Known answers / **Reconciliation** / Performance, plus never-true flags `reserve_disclosure_claim`, `execution_authorized`, `geotechnical_signoff`, `regulatory_compliance_claim` |
| 7 | `MODEL_FIT`, `UNCERTAIN`, `UNSCALED`, `REVIEW_REQUIRED` labels | `ESTIMATED` (with estimation variance), `UNCERTAIN` (unsourced parameter), `UNREGISTERED` (no CRS tie), `REVIEW_REQUIRED` (agent/OCR output). Never call values "measured", because that word is a resource category |
| 8 | Oracles and pins; never regenerate fixtures from the candidate (§4) | Pin GSLIB-family code, MineLib, a pseudoflow implementation, OMF, legacy models and datasets by version and hash. Vendor tools serve as cross-check fixtures, not vendored code |
| 9 | Reference quirks are recorded decisions (§8) | GSLIB rotation conventions, revenue-factor conventions, spreadsheet rounding: reproduce under a flag or fix with a parity exception |
| 10 | Data contracts are versioned files (§5) | `fmine-collars/1`, `fmine-assays/1`, `fmine-qaqc/1`, `fmine-block-model/1`, `fmine-pit/1`, `fmine-schedule/1`, `fmine-recon/1`; OMF for interchange |
| 11 | One conversion boundary per unit or frame (§6) | Mine grid ↔ projected CRS ↔ geodetic; elevation datum; dry ↔ wet tonnes; in situ ↔ broken; g/t ↔ oz/st ↔ ppm; metric ↔ short tons |
| 12 | Determinism lanes (§6, §17.5) | Identical digests for estimates, simulations (lifted philox RNG), pit shells and schedules across native and wasm, debug and release |
| 13 | Refusals, never silent clamps (§6) | Unclosed wireframes, missing density, assay without certificate or QA/QC, CRS mismatch, below-detection without a policy. **Top-cuts are declared decisions, never clamps** |
| 14 | Run receipt (§7) | Every estimate, shell and schedule carries input hashes, versions, parameters and output hash |
| 15 | Measure the model the way the tape does (§9.3) | Change of support: compare at composite, SMU, mill-sampler or truck-scale support; volumes as the survey computes them |
| 16 | Synthetic recover-the-parameters gates (§9.4) | Simulate a field from a known variogram, sample it, recover variogram and grade within tolerance; report cross-validation residuals |
| 17 | Report what the data cannot separate (§10.2) | Several geological interpretations or conditional realizations; inversion non-uniqueness. Do not pick one silently |
| 18 | "What it must fix / does not fix"; two-path sensitivity as review evidence (§11.1) | Multi-realization or multi-start schedule spread is review evidence, not a confidence interval. Ordinary runs state that it was not checked |
| 19 | Performance targets are budgets until benchmarked (§11.7) | "No faster-than-Vulcan/Deswik claim without a benchmark" |
| 20 | Reports carry a `no_claim` line (§11.8) | Every model and plan report carries `no_claim` text (for example "not a Mineral Reserve; not reconciled") |
| 21 | Numbered known-answer tests (§11.9) | Kriging exact at data; LG/UPL optima on benchmark instances; infinite-slope FoS; Bond energy; mass and metal balance closure; Atkinson ventilation; Hardy Cross water network; stockpile blend conservation |
| 22 | Reproducible exports; missing file fails its check (§12) | DXF/OMF/CSV exports with model-sourced timestamps, independent re-read and acceptance checks |
| 23 | `UNSCALED` traces refused for true-size export (§13) | Digitized old plans and voids stay `UNREGISTERED` until surveyed control points tie them in |
| 24 | CLI mirrors the workflow; `--json`; exit codes 0/2/3/4; round directory; status precedence (§14) | Same envelope and exit codes. Round = planning cycle. Precedence: BLOCKED → UNREGISTERED → data-readiness (QA/QC) → model validation → design gating → schedule feasibility → export checks → reconciliation → CONVERGED |
| 25 | Skill boundary: the skill keeps judgement, the CLI does mechanics (§14) | Agent skills and humans keep interpretation, QP classification, strategy and drilling decisions. The binary does deterministic mechanics. Legacy scripts become thin wrappers, retired after the gauntlet |
| 26 | Browser = same crates in wasm; numerical crates never see browser types; uploads nothing (§15, §18) | Field and office workbench runs the same engines; block models fetched from the user's own store; nothing uploaded |
| 27 | Crate table with budgets; no crate without approval; dependency direction; allowlist and forbidden list (§16) | Domain crate table with honest budgets **plus an overrun and scope-amendment protocol**. Same forbidden list; async confined to data adapters |
| 28 | Lift sibling code file by file with provenance header (§11.5, §16) | Lift from frankensim (`fs-sparse`, `fs-la`, `fs-math` det, `fs-rand` philox, `fs-ivl` predicates, `fs-propcheck`, `fs-detaudit`; candidates `fs-uq` KL fields/QMC, `fs-thermochem` for leach chemistry, `fs-solid`/`fs-contact` for geomechanics) with path, commit, licence, Changes and Left out. No workspace dependency |
| 29 | Oracle comparison classes byte / structural / tolerance (§17.1) | The same three classes, stated per quantity |
| 30 | Metamorphic relations (§17.3) | Frame rotation invariance; hole-order invariance; price/cost scaling invariance of shells; metal conservation in splits and merges; OMF round-trip |
| 31 | `FEATURE_PARITY.md`, partial never rounds up (§17.4) | One row per legacy capability with fixture IDs |
| 32 | Gauntlet release gate (§17.6) | Same method, full mode for ports and greenfield mode for frontier engines |
| 33 | Privacy: child wearer (§18) | MNPI drill results, worker telemetry, heritage sites, community data, confidential contracts. Synthetic or public fixtures only |
| 34 | Kill gates with fallbacks (§19) | G1 estimation parity, G2 optimization known answers, G3 planning round, G4 browser, G5 gauntlet. Fallback: keep the vendor tool "and say so" |
| 35 | Phases with exits; frontier never borrows from parity (§20) | Phase 0: core, CRS, oracle harness, source-qualified data records, estimation kernel. Later: estimation/pit → design/schedule/CLI → geomet authority → commercial/browser → gauntlet |
| 36 | Work units = bead titles (§21) | `FMI-nn` units mapped 1:1 to beads, each naming its gate |
| 37 | Risks: "decided at G4, measured, not guessed" (§22) | Data sparsity (testwork, rock properties), MIP licences against no-FFI, wasm block-model size, moving legacy references |
| 38 | **One physical authority** (§25.1) | **One geometallurgical domain/material authority**: frozen domain records feed density, slope design, drill and blast, haulage, comminution, recovery, tailings, water/ARD, procurement and payables, without solver edits |
| 39 | Central acceptance: fixed pattern and body, swap linen → leather → knit (§25.1, §25.5) | Fixed pit/stope design and schedule; swap the ore-domain characterization (oxide → transitional → fresh sulfide). Geometry unchanged; every consumer updates; digests change; no stale caches |
| 40 | A missing property blocks only consumers that need it (§25.1, §25.9) | A missing Bond Wi blocks throughput forecasts but not grade–tonnage; missing NAG/ABA blocks waste classification but not haulage |
| 41 | Hierarchy, not conflation: fiber → yarn → sheet → assembly; stable identifiers (§25.2) | Sample → interval → composite → domain → stockpile/blend → concentrate lot; IDs for lab, method, batch, certificate, CRM. pXRF is screening data, not assay data; a logged lithology is not geochemistry; an OEM brochure is not a measured cycle time |
| 42 | Observation record: unit, method, direction, conditioning, evidence class, locator, date, bounds; sparse; preserve originals (§25.2) | Assay record: element, unit, method code (for example FA-AAS 30 g, 4-acid ICP), detection limits, lab, certificate, batch QA/QC status, date. Geotech specimen: geometry, L/D, moisture, orientation to foliation. Density: method (wax immersion) |
| 43 | No midpoints of unreported ranges; CI ≠ tolerance; variability ≠ uncertainty of the mean (§25.2, §25.6) | No silent half-detection-limit substitution; duplicates (precision) vs CRMs (accuracy) vs estimation variance kept distinct |
| 44 | One specimen measured three ways is one specimen (§25.10) | One pulp assayed by the primary and umpire labs is one sample, not two; field duplicates are distinct from lab replicates |
| 45 | Matching full protocols before Pareto comparison; no invented aggregate score (§25.10) | Bond Wi at different closing screens and FA vs aqua regia digestion are not comparable without explicit admission. Keep NPV, risk, closure liability and emissions as a Pareto set, with no hidden "ESG score" |
| 46 | Explicit estimate consent; interpolation is a modeling choice (§25.7, §25.10) | Using estimated density or recovery curves needs explicit opt-in; regression curves of recovery against grind carry their validity domain; no extrapolation |
| 47 | Validity domains enforced through solver trials; held-out never widens range (§25.28) | Recovery and throughput models refuse feed outside the tested hardness and grade range; held-out testwork validates and never re-fits |
| 48 | Purchasing is a source-observation problem separate from physics (§25.4) | Reagents, explosives, fuel, power and spares: dated, currency- and terms-matched quotes; convert via sourced concentration (NaCN purity, available CaO in lime); never infer FX, freight or recovery |
| 49 | Immutable dated quotes; schema extension, not timestamp rewriting; source changes refuse publication (§25.4, §25.11) | Commodity price decks, LIMS imports, fleet-system and SCADA snapshots are immutable and dated; shift-level data needs a deliberate schema, not timestamp rewrites |
| 50 | External collectors publish immutable inputs, never enter the solver (§25.1) | Live adapters (LIMS, fleet management, historian, weighbridges) publish content-hashed snapshots; optimizers consume snapshots only |
| 51 | Multi-observation agreement before numeric inventory is admitted (§25.29) | Stockpile tonnage admitted only when survey, dispatch and weightometer agree within tolerance; otherwise unknown |
| 52 | Canonical material hashes bind every offer (§25.29) | Concentrate sale valuation binds to the exact lot's assay certificate and contract revision (payables, TC/RC, penalty elements) |
| 53 | Nominal width ≠ usable width; total inventory ≠ continuous cut (§25.29) | Nameplate ≠ demonstrated throughput; rated ≠ achieved payload (fill factor, carryback); total stockpile ≠ blendable tonnage |
| 54 | Feasible incumbent vs proved minimum; budget exhaustion keeps the incumbent; incomplete order never gives a cheap subtotal (§25.61, §25.115) | Schedules and blends report incumbent, bound, gap and status; an infeasible partial plan never supplies NPV |
| 55 | A shared operator is not silently multiplied; aggregate work ≠ elapsed completion (§25.94) | One crew or remote operator across several rigs; equipment hours ≠ schedule makespan |
| 56 | Shared reconciled production count across consumers (§25.84) | One mined/milled/shipped tonnage ledger reconciled across mine, plant, logistics and sales; conflicts refuse the affected consumer |
| 57 | Geometric export acceptance ≠ process feasibility ≠ permission to operate hardware (§25.4) | Design-file acceptance ≠ geotechnical/operational feasibility ≠ permission to fire, dig or dispatch |
| 58 | Unknown finish cannot authorize thermal processing (§25.4) | Unknown sulfide reactivity cannot authorize ANFO loading in reactive ground; unknown PAG status cannot authorize NAF dumping; unknown moisture vs TML cannot authorize concentrate shipping |
| 59 | Treatment before/after bound to specimen, process and cycle count (§25.83) | Stockpile oxidation/aging, heap-leach column duration, weathering: before and after states with exact test identity; no invented per-day law |
| 60 | One mass authority for retained liquid (§25.102, §25.121) | One wet/dry mass and moisture ledger from pit to port (haulage, crushing, filtration, TML for shipping) |
| 61 | "Comparison calculator, not body thermoregulation"; no safety rating from a model (§25.4, §25.6) | A ventilation or heat model is a comparison calculator, not a heat-stress or re-entry clearance; slope FoS output is not a geotechnical sign-off |
| 62 | Delivery-increment template (§25.6 onward) | Same five-part template, bead IDs, doc per increment, closing disclaimer. Keep the log out of the constitution |

---

## 7. Recommended FrankenMining plan skeleton (mirrors FrankenTextiles)

0. How to read this plan (tiers; read the claims and kill gates first; plan vs specs)
1. Product contract (inputs by evidence kind; outputs with verdict basis; users; what it is NOT)
2. Where this comes from: the reference systems and what they taught (keep / replace, with measured
   failures)
3. The four claims, never conflated (Parity, Known answers, Reconciliation, Performance) plus the
   never-true authority flags
4. Oracles and pins
5. Architecture overview (data-flow diagram; the planning round; versioned file contracts)
6. Units, coordinate reference systems, determinism and refusals
7. `core`
8. …per-domain crate chapters (data and QA/QC; geology and estimation; geotech; optimization and design;
   scheduling; processing and geomet; materials and commercial; reconciliation; exports), each with
   "what it must fix / does not fix", lifted code with provenance, metrics and flags, performance budgets
   (not measurements), files and known-answer tests
9. CLI and the skill boundary
10. The browser
11. Workspace crates, budgets, dependency direction, dependency policy, **overrun and scope-amendment
    protocol**
12. Verification program (oracle fixtures, known answers, properties and metamorphic, feature parity,
    determinism lanes, gauntlet)
13. Confidentiality, MNPI, people and heritage data
14. Kill gates (with fallbacks)
15. Phases (with exits)
16. Work breakdown (bead titles)
17. Risks and open questions
18. Explicit exclusions
19. Glossary (including `ESTIMATED`, `UNREGISTERED`, F1/F2/F3, SMU, support)
20. One geometallurgical authority and source-qualified records (requirements, in the style of §25.1–§25.4)
21. Delivery state and next acceptance gates (short; detailed log in `docs/delivery/`)
Appendix: Sources

---

## 8. Anti-patterns and cautions seen in FrankenTextiles

- **Budget drift**: the line count is about 2.7× the ceiling (cli 51k vs 4k; drape 80k vs 14k). The plan
  had no amendment protocol, and the README had to add one after the fact.
- **The plan became a log**: §25 is about 80 % of the document. Searching for the current doctrine means
  reading 118 increments.
- **Unbounded scope expansion through §25.1**: the "is not" list in §1.4 was overridden by a later
  section. For mining, define in advance the gate that admits a scope expansion.
- **The ten-crate rule held only by loading domains into existing crates**: the materials platform sits
  inside `ftx-pattern`, and `ftx-materials` is a second binary in `ftx-cli`. Consider budgeted per-domain
  crates from the start.
- **The blocking empirical gate stayed open throughout §25** (matched linen/leather/knit
  characterization). Software progress outran data acquisition. For mining, put data acquisition (testwork
  programs, QA/QC audits, public datasets) in the phase plan as first-class work with its own gates.
