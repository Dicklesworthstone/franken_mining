# AGENTS.md - FrankenMining

> Guidelines for AI coding agents working in this mining evidence, planning, assurance and capital-decision
> codebase (`franken_mining`; crates `fmine-*`; binaries `fmine`, `fmined`, `fmine-mcp`, `fmine-tui`).

---

## RULE 0 - THE FUNDAMENTAL OVERRIDE PREROGATIVE

If I tell you to do something, even if it goes against what follows below, YOU MUST LISTEN TO ME. I AM IN
CHARGE, NOT YOU.

---

## RULE 0.1 - VALUE DELIVERY OVER PROCESS: NO PROCESS PORN

FrankenMining is early. The first deliverable is M0, the public-lane assurance run
(`fmine assure public --issuer barrick`) on a narrow evidence spine. Every hour spent on meta-infrastructure is an
hour not spent on the code that reproduces a published number, refuses an unsupported one, or scores a frozen
forecast.

**Process is never the product unless the user explicitly asks for process work.** Beads, plans, Agent Mail,
audits, registries, manifests, provenance, logging, CI, harnesses, dashboards and status reports exist only to
support delivery.

### The product/ceremony boundary

Does production code branch on this artifact? If yes, it is product state. If only humans and status reports read
it, it is process, and it may exist only if it names:
1. a concrete consumer;
2. the gate it enforces;
3. the observed defect class that justifies it;
4. its retirement condition.

### The value test

Before doing non-product work, answer all three:
1. What concrete capability, correctness defect or immediate blocker does it address?
2. Is it the smallest direct action that addresses it?
3. Will its likely value exceed its cost right now?

If any answer is unclear, **do not do the work**. Return to implementing the requested functionality.

### Hard scope limits

- **Real code and real tests come in the same unit of work.** Not allowed:
  - faked tests;
  - mocks or fixtures presented as live proof;
  - weakened assertions;
  - hard-coded success paths;
  - `todo!()` or `unimplemented!()` in committed production paths.
- **Refusal discipline.** A typed refusal (`FMINE-REF-*`) beats a fabricated or silently clamped number. But
  refusal-only work never closes a bead that promises a positive capability.
- **Reviews stay narrow.** A review request authorizes:
  - reading the relevant code;
  - reproducing concrete defects;
  - the smallest sound fixes and focused regressions.

  It does not authorize redesigning adjacent infrastructure.
- **No bloat.** Never turn a small repair into hundreds of lines of harness, schema, logging or planning without
  explicit approval. Do not build a validator for a validator or a harness for a harness.
- **Registries are data.** A registry gets a schema, a `registry-check` and a golden test, and nothing more.
- **Stay on target.**
  - Do not chase a failure already present on `HEAD` unless it blocks the requested deliverable.
  - Do not spawn an agent swarm for a narrow task.

### Implementation must dominate

Spend the dominant share of effort on working product code and its direct known-answer, property and
integration tests. Prefer an existing test seam over a new framework. Run the narrowest relevant checks while
iterating.

### Mandatory checkpoint and stop rule

At the first sign of scope expansion, state:
- the outcome;
- the files involved and the size of the expansion;
- why it is necessary;
- the smaller alternatives.

If that cannot establish immediate value in a few sentences, ask first. When work has drifted into process porn,
stop, freeze the tree, disclose the state and wait.

---

## RULE NUMBER 1: NO FILE DELETION

**YOU ARE NEVER ALLOWED TO DELETE A FILE WITHOUT EXPRESS PERMISSION.** Even a new file that you yourself
created, such as a test file. You must always ask and receive clear, written permission before deleting a
file or folder of any kind.

---

## Irreversible Git & Filesystem Actions - DO NOT EVER BREAK GLASS

1. **Absolutely forbidden commands:** `git reset --hard`, `git clean -fd`, `rm -rf`, or any command that can
   delete or overwrite code/data, unless the user explicitly provides the exact command and states, in the
   same message, that they understand and want the irreversible consequences.
2. **No guessing:** if there is any uncertainty about what a command might delete or overwrite, stop and ask.
3. **Safer alternatives first:** `git status`, `git diff`, backups, hand-written patches.
4. **Mandatory explicit plan:** even after authorization, restate the command verbatim, list what will be
   affected, and wait for confirmation.
5. **Document the confirmation** in your final response: the authorizing text, the command, the time.

Never stash, revert, overwrite or otherwise disturb another agent's work.

---

## Git Branch: Canonical `main`, Mirrored to `master`

All work happens on `main`. The only other permitted branch is `master`, an exact mirror of `main`.
After publishing `main`, fast-forward both local and remote `master` to the same commit. Never develop on
`master`, force-push it, or discard unique history to make it match; integrate useful work into `main` first.

## RULE 2: NO GIT BRANCHES. NO GIT WORKTREES. EVER.

**Forbidden:**
- feature branches, `git checkout -b`, `git switch -c` and `git worktree add`;
- pushing refs other than `main` and its `master` mirror;
- creating pull requests;
- scratch clones of this repository used to isolate work;
- any tool that creates branches or worktrees as a side effect.

**Instead:**
- commit to `main` when the work is ready, and keep unfinished work in the tree;
- coordinate through Agent Mail reservations and Bead IDs;
- never revert or stash another agent's changes.

---

## Project Truth Sources

- `COMPREHENSIVE_PLAN_FOR_THE_DESIGN_OF_FRANKENMINING.md` is the technical constitution. It sets architecture,
  claims, invariants, gates, milestones, crates, budgets and the 320 plan beads (§39).
- `planning/mor/MOR_CASE.md` is the evidence register (E001–E672). Every FACT in the plan cites an E id, and the
  E rows cite a file, page, sheet and cell, or a URL and date.
- `planning/corpus/C1–C6` hold the corpus evidence notes. `planning/research/R1–R8` and `W1–W5` hold the research
  and primary-source passes. `planning/brenner/B1–B6` and `planning/mor/audits/` hold the hypothesis passes and
  audits.
- `registries/*.toml` hold behaviour that an owner, QP or regulator may change: verbs, units, CRS, calendars,
  refusals, lifts, LOC budgets and workspace topology.
- Each crate has a `CONTRACT.md` with the ten sections of plan §0.2.

When this file and the plan disagree on a technical rule, the plan wins; flag the discrepancy.

---

## FrankenMining - This Project

FrankenMining is an evidence-native, asupersync-based operating system of record for integrated gold, copper,
silver and tin miners. It covers the chain from the first soil sample to the board's capital decision. Every
number it emits carries:
- its support;
- its evidence class;
- its calibration record;
- its unknown-unknown term;
- its provenance.

Every forecast it makes is frozen before the outcome and scored after.

Named subsystems (plan §0.5):

| Subsystem | Scope |
|---|---|
| Bedrock | Substrate and runtime |
| Plumb | Claims, KK/KU/UU, calibration and surprise |
| Lodestar | Exploration and prospectivity |
| Corebox | Drilling, sampling and QA/QC |
| Strata | Subsurface belief ledger |
| Highwall | Geotechnics |
| Pushback | Mine planning and ore mix |
| Headframe | Operations twin of record |
| Crucible | Processing and refining |
| Freeboard | Tailings, water and closure |
| Muster | Workforce, rosters and critical controls |
| Touchstone | Reconciliation |
| Bullion | Accounting via franken_accounting, and metrics |
| Hallmark | Reserves and disclosure |
| Helm | DecisionCards and the CapitalProgram |
| Portal | Interoperability and the public-filings lane |
| Adit | CLI, daemon, MCP and TUI |
| Testpit | Synthetic truth, oracles and the offline verifier |

### What this project is NOT (plan §4.2)

- Not a control system. No process holds a write path to ISA-95 Levels 0–2 (INV-011). It never drives a truck,
  a mill or a pump.
- It never classifies a resource or reserve, signs a dam or slope, or asserts regulatory compliance. Humans sign;
  the system supplies evidence and the record of who signed.
- Not a general ledger. franken_accounting is the book of record, reached as a subprocess, never as a crate.
- Not a replacement for SAP, Pitram, DISPATCH/MineStar/FrontRunner, Leapfrog, Whittle or Deswik in years 1–2. It
  is the evidence layer that reads their exports.
- Not a general-purpose MILP solver.
- Not a hidden aggregate score: components are always shown.

---

## THE FOUR CLAIMS - NEVER CONFLATE THEM (plan §33.1)

| Claim | Meaning | Evidence | Permitted wording |
|---|---|---|---|
| **Parity** | Reproduces a reference tool or a published number from the same inputs | Pinned oracle fixtures; public filings | "reproduces X to tolerance t" |
| **Known answers** | Recovers seeded truth on synthetic data with calibrated intervals | `fmine-gen` replicates, SBC, coverage | "recovers injected biases with c% coverage over n replicates" |
| **Reconciliation** | Forecasts agree with mined and processed reality, scored after the fact | The calibration ledger; hindcasts; reconciled outcomes | "80% intervals covered k of n outcomes" |
| **Performance** | A workload meets a speed or scale target on named hardware | A benchmark with hardware, data size, seed and spread | "p99 = x on host H at size N" |

A gate names exactly one claim type. Parity with a published number does not show the number is right. Synthetic
recovery does not show a real deposit behaves like the generator. A performance number is never evidence of
correctness.

---

## Evidence Labels And Honesty Rules

Every load-bearing statement in code comments, docs, commit messages and reports carries a label:

| Label | Meaning |
|---|---|
| **FACT** | Verified in a cited file, commit, filing or publication. `FACT[U]` marks an unverified secondary source |
| **INFERENCE** | Derived from FACTs by a stated calculation |
| **DESIGN** | A decision, with its rationale |
| **HYPOTHESIS** | A testable belief, with the test that would kill it |
| **TARGET** | A number to hit, not a measurement until a gate records it |
| **OPEN** | Undecided, with an owner |

1. **Never regenerate an oracle fixture from the candidate.** A mismatch is a defect until proven otherwise.
2. **Register before resolve (INV-008).** A forecast is hashed and recorded before its outcome's valid time. No
   retrofitted forecasts, and no hindsight claims unless built `as_of_decision(t)` (INV-016).
3. **No number without a source.** Every public-lane value cites `<file-period>:<Sheet>!<Cell>` or a page, plus
   the source file's sha256. An uncited number is a defect.
4. **No probability without a calibration flag.** No single number without an interval, an evidence class and a
   source.
5. **Refuse, never clamp or plug.**
   - Conservation failures are refused at commit (INV-005), never balanced with a plug row.
   - Units, bases and currencies never convert implicitly.
   - A `Refused` value never becomes a number downstream without a declared fallback (INV-009).
6. **Evidence never upgrades through derivation (INV-006).** A model output is not "measured". A synthetic result
   is labelled synthetic.
7. **Report residuals and failures.** A failure is a failed cell, not an excluded cell; partial never rounds up
   (plan §0.2 defines "implemented").
8. **All galaxy-brain card values (GB-*) are illustrative** unless tagged FACT.

---

## Data, Confidentiality And Privacy

- **Public lane only in this repository.**
  - Fixtures and examples use public filings, issuer-website workbooks, or synthetic truth from `fmine-gen`.
  - Never commit site exports, assay databases, block models or anything a pilot site supplies.
  - Never commit MNPI, embargoed drill results or JV-partner data.
  - Taint is monotone (INV-007): MNPI-derived bytes never reach a public export.
- **Ownership basis is part of every figure.** Record whether a number is 100% project basis or attributable,
  and at what interest. Examples: NGM 61.5% Barrick / 38.5% Newmont; Pueblo Viejo 60/40; Kibali 45%;
  Loulo-Gounkoto 80%. JV-partner-visible outputs never carry walled valuations.
- **Personal data never enters the repository.** That covers:
  - names and contact details from consultation logs or grievance forms;
  - rosters, fatigue and competency records;
  - anyone's private details in fixtures, docs, examples or commit messages.

  Importers redact on ingest, even from public documents.
- **Source documents are never committed.**
  - `corpus_of_input_docs/` is gitignored: technical reports, Mine Stats workbooks, investor decks and textbooks.
  - Textbooks are reference-only.
  - Evidence notes cite documents by file, page and cell.
  - `planning/corpus/C5_reko_diq_esia.md` is held back from the public repository pending owner review. Its
    source carries a confidentiality notice.
- **No affiliation is implied.** FrankenMining is not affiliated with, endorsed by or built for Barrick, Newmont
  or any issuer named in its fixtures. Demo outputs built from public filings are labelled as such.

---

## Pins

| Item | Pin |
|---|---|
| Async runtime | asupersync `=0.5.0`, the sole async runtime, one package id per closure (plan §31.4 lists its defects and mitigations) |
| Toolchain | `nightly-2026-08-31` (`rust-toolchain.toml`), Rust 2024 edition, one Cargo workspace |
| Sibling revisions | Exact revisions in `franken-stack.lock` |
| frankensim | Lift-only: each lifted file carries a provenance header matching `registries/lifts.toml` (`xtask check-lifts`) |
| franken_accounting | A subprocess at a pinned revision; no FA crate anywhere |

Moving a pin is a deliberate event with new fixtures and the old results kept.

---

## Toolchain, Unsafe Code And Lints

- `unsafe_code = "forbid"` across the workspace.
- `unwrap_used`, `expect_used` and `panic` are denied in production library code. Tests may use them.
- Determinism is a product feature (plan §29):
  - fixed-order reductions;
  - seeded Philox streams keyed by `(seed_root, partition, …)`;
  - no hash-map iteration that affects output.

  The same inputs, seed, code revision and determinism class give the same certificate digest (INV-010).

### Dependency Policy (plan §28.5)

- **L-1 direction.** Dependencies point downward only: `T → U → V → E → K → S1 → S0`. Within K the order is
  K4 → K3 → K2 → K1 → K0. `xtask check-layers` enforces this against `registries/workspace_topology.toml`.
- **L-2 formats.** S0 crates are std-only plus lifted code, and durable formats (`FMT-*`) live in S0.
- **L-3 capabilities.** K and E use no spawn, IO or time APIs. K may use only `Cx<cap::None>`, `Budget` and
  `checkpoint`.
- **L-4 foundation confinement.** Each foundation is confined to its adapter crate (fsqlite and frankensearch
  in projections; fastmcp in `fmine-mcp`; and so on).
- **L-5 banned dependencies.**
  - tokio, hyper, reqwest, axum;
  - rusqlite, sqlx;
  - arrow, parquet, polars;
  - petgraph;
  - chrono and time as direct dependencies;
  - ort, fastembed, tantivy;
  - C solver features.
- **L-6 claims.** No public K/E signature takes or returns a bare `f64` for a physical or monetary quantity
  (`xtask check-claims`).

If you need to violate a rule, stop and redesign the API.

---

## Workspace Crates

The plan's crate map (§28.3) has 59 crates in 7 layers. **A crate joins the workspace only with its first real
slice**, within its LOC budget (`registries/loc_budget.toml`; ceiling 1.15× plan). No new crate outside the crate
map without explicit user approval. Phase A (M0–M2) slices 35 of them. The first are:
- the S0 substrate: `fmine-core`, `-units`, `-time`, `-crs`, `-evidence`, `-claim`, `-registry`, `-array`;
- `fmine-numerics` and `fmine-calib`;
- the public lane: `fmine-io`, `fmine-public`, `fmine-metrics`;
- `fmine-decide` and `fmine-cli`;
- `xtask`.

---

## Core Invariants (plan §6.5, INV-001–INV-016)

- **INV-001.** No naked numbers cross a crate boundary: they travel as `Claim`s with support, evidence class,
  calibration flag, UU term, validity and provenance.
- **INV-002.** `reserve_disclosure_claim`, `execution_authorized`, `geotechnical_signoff` and
  `regulatory_compliance_claim` are `false` in every envelope. No code path can set them.
- **INV-003 / INV-004.** Authority records are append-only: corrections supersede. Each authority cell has one
  writer per epoch.
- **INV-005.** Conservation is enforced at commit, within the registered tolerance, or the commit is refused.
- **INV-013.** A superseded dependency marks its Claims stale on read.
- **INV-014.** Support is part of a quantity's type, and averaging across supports is refused. `Grade` has no
  `Add`.
- **INV-015.** A decision-facing Claim carries exactly one admissible UU term.
- **Time.** Records carry a `QuadTemporalStamp`:
  - `physical_valid_time`;
  - `collection_time`;
  - `availability_time`;
  - `record_time`.

  `as_of_decision` never sees an assay before its `availability_time` (INV-016, G-TIME-2, FMINE-REF-108).
- **Units.**
  - Wet and dry tonnes never mix.
  - Per-ounce metrics name their basis: sold or produced, attributable or 100%, by-product or co-product.
  - S as a fraction is never S in percent.

---

## Testing Policy

- **Oracle fixtures.** Pinned, CAS-addressed and never regenerated from the candidate. Public-filing values carry
  their source cell and file hash.
- **Synthetic truth.** `fmine-gen` builds mines and districts with known answers, misspecification and control
  conditions.
- **Property and metamorphic tests** for the no-laundering rules, conservation and support algebra.
- **Determinism lanes and golden digests.** Plus a LabRuntime test for every protocol a component takes part in
  (cancellation, crash, partition).
- **Placement.** Unit tests sit next to the code. Cross-crate and end-to-end tests live in the T layer.

---

## Gates And Milestones

- **GATE-LAYERS-00** (layering, capabilities, dependencies, lifts, claims) runs from commit 1:
  `cargo run -p xtask -- check-all`.
- **M0 Public Mirror plus Spine** (Nov 2026–Jan 2027), gate GATE-BULL-01. `fmine assure public --issuer barrick`
  must:
  - reproduce ≥ 95% of numeric site-quarter values in the Mine Stats workbooks (2024Q1–2026Q2, plus Q3 2026 on
    its release) to published rounding;
  - explain or refuse the rest;
  - leave zero uncited numbers;
  - give byte-identical reruns;
  - hash the FY2026 per-site forecasts before the February 2027 release.
- **The ladder.** M1–M6 and their kill gates are in plan §37. The program gate is at month 12 (§34.5).
- **Work order.** The work packages WP-00 … WP-20 and beads BD-001 … BD-320 (§38–§39) are the order of work. The
  critical path is 29 beads long, starting at BD-001.

---

## Code Editing Discipline

- **No script-based code changes.** No broad regex rewrites; focused hand patches.
- **No file proliferation.** No `main_v2.rs`, `improved.rs`, duplicate module copies.
- **No new crates without approval** (see Workspace Crates).
- **Backwards compatibility.** None is owed to our own pre-1.0 APIs. Full compatibility is owed to durable formats
  (`FMT-*`, magic `FMNE`) once a gate has recorded data in them.
- **Comments** document invariants, units, supports, bases, coordinate frames and no-claim boundaries, not
  restated code.

---

## Output Style

- Library crates do not print.
- The CLI prints deterministic human text and, with `--json`, a stable NDJSON envelope (plan §30.1) with
  documented exit codes. Every verb is registered in `registries/verbs.toml`.
- Refusals are structured: `Refusal { code, what, fix }`, with codes from `registries/refusals.toml`.
- Engines report telemetry into their certificate, never per-iteration events from hot loops.

---

## Compiler Checks

After substantive Rust changes:

```bash
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo run -p xtask -- check-all
```

Use RCH for heavy Cargo work. Never bypass the `rch` cargo shim.

```bash
rch exec -- env CARGO_TARGET_DIR="${RCH_TARGET_BASE:-${TMPDIR:-/tmp}}/rch_target_fmine" cargo test --workspace
```

Always base new target dirs on `${RCH_TARGET_BASE:-${TMPDIR:-/tmp}}`, never bare `/tmp`. Before heavy builds on a
laptop, check free disk (`sbh check --need 20G`). Report the exact command and result; if a check did not run,
say so.

---

## Documentation

Each crate gets a `CONTRACT.md` (template: `docs/templates/CONTRACT.md`) before another crate depends on it. It has
ten sections:
1. purpose;
2. inputs;
3. outputs;
4. invariants;
5. refusals;
6. determinism class;
7. evidence class of outputs;
8. validity domain;
9. oracles;
10. known gaps.

Architecture decisions go in `docs/adr/`. Keep the plan high-level; implementation notes belong in crate docs and
Beads. Plan edits follow the plan's own hygiene rules (§0.7):
- evidence labels and E ids;
- Unicode maths, no LaTeX;
- lines wrapped at about 118 characters;
- identifiers never renumbered.

---

## Agent Workflow

- **Start.** Read:
  - this file;
  - the relevant plan sections;
  - the crate `CONTRACT.md`;
  - the Bead.

  Then inspect the tree.
- **While working.** Keep changes scoped and do not disturb unrelated files. Attach every claim to a test, a
  fixture, an E id or explicit no-claim language.
- **End.**
  1. Run the applicable checks.
  2. Report exactly what passed and what did not run.
  3. Update the Bead and run `br sync --flush-only`.
  4. Release reservations and leave handoff notes.

---

## MCP Agent Mail - Multi-Agent Coordination

When Agent Mail is available:
- register for this project path;
- reserve files before editing;
- use the Bead ID as the thread ID;
- send start, progress and completion messages;
- release reservations when finished.

Reservations replace branches and worktrees.

Never run:
- `am service restart` or `am service stop`;
- `am doctor fix`, `am doctor repair` or `am doctor reconstruct`;
- `kill` against any `am` or `mcp-agent-mail` process.

If `am` fails, retry once after a few seconds, then continue without it.

---

## Beads (`br`) - Issue Tracking

```bash
br ready --json
br list --status=open
br show <id>
br update <id> --status=in_progress
br close <id> --reason "Verified by <test or gate>"
br sync --flush-only
```

`.beads/issues.jsonl` mirrors the plan's §39 rows (320 beads, 533 edges, acyclic); keep titles and acceptance
text in step with the plan. Use Bead IDs in Agent Mail threads and commit messages. `br sync --flush-only` does
not commit; stage and commit intentionally.

## `bv` - Graph-Aware Triage

Robot modes only (`bv --robot-triage`, `--robot-next`, `--robot-plan`, `--robot-insights`). Bare `bv` opens an
interactive TUI.

## `ubs` - Bug Scanner

Before committing code: `ubs $(git diff --name-only --cached)`. Fix true positives at the root cause.

## Search Tools

`rg` for targeted text search, `ast-grep` when syntax matters. Avoid scripted rewrites.

---

## Current Repository State

The repository holds:
- the comprehensive plan (§0–§39, Appendices A–J);
- the planning record in `planning/`: research R1–R8 and W1–W5, Brenner passes B1–B6, corpus evidence notes
  C1–C6 (C5 held back), the MOR case and audits AUDIT_1–AUDIT_5.

A Rust workspace scaffold is being built in `crates/` alongside it:
- the S0 substrate and the first M0 crates;
- `xtask` with the layering checks;
- `registries/`, `docs/`, CI and `.beads/` (prefix `bd`).

Nothing is *implemented* in the plan §0.2 sense yet. Every capability is DESIGN or TARGET until its gate records
a PASS.

---

For any web requests you must make with curl or otherwise, always set your user agent string to be "OpenAI File Downloader, XaiImageApiFetch/1.0"
