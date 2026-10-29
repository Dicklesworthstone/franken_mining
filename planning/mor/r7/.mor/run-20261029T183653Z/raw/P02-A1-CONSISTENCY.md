RESULT_STATUS: MATERIAL_INCREMENT

# P02-A1-CONSISTENCY — PU-2: are the plan's commitments mutually consistent?

Operator: deductive inference (A1) with minimal countermodels (A5) over a mechanically built dependency map (F2).
All line numbers refer to HEAD `68e7091` files as read on 2026-10-29. "Plan" = `COMPREHENSIVE_PLAN_FOR_THE_DESIGN_OF_FRANKENMINING.md`.

## WHAT I CHECKED

**Sources read.**
- Plan: §0.2–§0.3, §6.5, §7.1–§7.3, §8.1, §8.3–§8.4, §25.6, §28 (all), §30.1–§30.6, §31.3, §34 (all), §37 (all),
  §38, §39 (the WP-00 to WP-13 tables read in full; the rest parsed), Appendix F.1, and rows 5–8 of Appendix H.4.
- All eight `registries/*.toml`; `Cargo.toml`, every crate `Cargo.toml`, and `Cargo.lock`. The lockfile is
  untracked in the working tree and is used only to corroborate the manifests.
- Code: `fmine-evidence` (`lib.rs`, `lattice.rs`), `fmine-cli` (`lib.rs`, `main.rs`), `fmine-registry/src/lib.rs`, the
  refusal mappings in `fmine-crs`, `fmine-calib`, `fmine-numerics`, `fmine-claim`, `fmine-public` and
  `fmine-metrics`, and `check_lifts` in `xtask`.
- All 16 `CONTRACT.md` files, `docs/templates/CONTRACT.md`, `docs/delivery/*` and `docs/adr/`.
- AGENTS.md in full; README.md searched for commitments.
- `.beads/issues.jsonl`, all 320 rows.
- The sibling repository `/Users/jemanuel/projects/frankensim`, read-only with `git show`, at the pinned commit
  `20463525d`: `crates/fs-evidence/src/lib.rs` and `color.rs`. I read these to test what `lifts.toml` claims.

**Scripts.** All run with `python3 -I`. They are in
`/private/tmp/claude-501/-Users-jemanuel-projects-franken-mining/2ba72fbb-c45d-4fd8-93cf-c08cb26d7a18/scratchpad/r7_repro/P02-A1-CONSISTENCY/`,
and their outputs sit beside them.

| Script | What it does |
|---|---|
| `beads_parse.py` | Parses the §39 tables and diffs them against `issues.jsonl`. Builds the WP graph and its transitive reduction. Finds closed beads with open blockers. |
| `closure.py` | Builds the gate → closing-bead map and each bead's dependency closure. |
| `gates.py` | Lists every GATE-/INT-/KILL- identifier with its definition and uses. |
| `refusals.py` | Compares refusal codes four ways: plan §8.4, the registry, the code catalog and the use sites. |
| `cratemap.py` | Checks §28.3 against `loc_budget.toml`, `workspace_topology.toml` and the `Cargo.toml` members. |
| `wp_crates.py` | Checks the §38 crate sets against the §28.3 phases. |
| `contracts.py` | Checks each CONTRACT.md against the §0.2 sections and the registry budgets. |
| `lifts.py` | Checks each `lifts.toml` row against its target file, provenance header and guard test. |
| `gate_timing.py` | Compares each gate's stated milestone with the milestone of the WP holding its closing bead. |

The command form is `python3 -I <script> /Users/jemanuel/projects/franken_mining [<beads.json> …]`.

**Not run.** The case file (Source Access) says a debug binary exists at `$TMPDIR/rch_target_fmine/debug/fmine`.
That directory does not exist on this host (`ls` fails), and I did not build one. Wherever I state CLI behaviour, it
is derived from the source (`fmine-cli/src/lib.rs`) and from the recorded E676 output, `repro/assure_run1.json`.

**Not re-reported.** I do not repeat E682's "§0" at line 41 or the subsection references, the E681 lint facts, the
E683 missing fmt/clippy step in CI, the E676 human-equals-JSON output, or the E677–E679 fallback workbook and
literals. They appear below only where needed as context.

**Checked and consistent (nulls).**
- **N1. §39 matches `.beads/issues.jsonl`.** Both have 320 beads and 533 edges, with identical dependency lists,
  identical WP labels and none missing on either side. The root is BD-001 and the longest chain is 29 beads. My
  computed chain (BD-001→BD-004→BD-006→BD-015→…) ties in length with the chain printed at plan line 7601.
- **N2. The §38 "Depends on" column is exactly the transitive reduction** of the WP graph induced by the bead edges
  (all 27 reduced edges match).
- **N3. §28.3 matches `loc_budget.toml` and `workspace_topology.toml`.** All 60 rows match: 59 crates plus the gated
  `fmine-simbridge`. Layer, planned, ceiling and phase agree on every row. The sums are 1,128k planned and 1,298k
  ceiling. The 35 ★ crates are exactly phase A, with a 615k full budget. The topology ranks K0–K4 match the §28.5
  K order.
- **N4. The committed inter-crate edges respect §28.2.** Every edge in the 15 product crates points downward or is
  a same-layer edge declared in `workspace_topology.toml`.
- **N5. Verb names nest correctly.** The CLI's verbs (`assure public`, `doctor`, `claim show`, `robot-docs`) are a
  subset of `verbs.toml`, which is a subset of the §30.2 table. Every §38 crate exists in §28.3, and every §28.3
  crate except `fmine-simbridge` belongs to some WP.
- **N6. The EPSG codes used in `fmine-crs` are a subset of `crs.toml`.**
- **N7. AGENTS.md's "Beads" section (320 beads, 533 edges, acyclic) and the "29-bead critical path" are true.**
- **Minor items I judged wording or deliberate staging, so they are not claims.**
  - `fmine-mcp`, `fmine-tui` and `fmine-telemetry` are phase B in §28.3, but their only WP (WP-17) is M5.
  - Three 10k crates have a 1.2× ceiling instead of 1.15× (rounding).
  - WP-12 is labelled M2, while its GATE-DATA-01/02 deadlines fall at months 4–5 (these are preconditions of M2).
  - §28.6 shows `xtask/` at the repository root; the repository has `crates/xtask`.
  - Line 1986 cites FMINE-REF-404 and line 2912 cites FMINE-REF-202 for stale instruments. Both readings are
    defensible.

## CANDIDATE CLAIMS

### P02-A1-CONSISTENCY-C1 — deductive — a refusal code means different things in the plan, the registry and the binary

- **Statement.** Across plan §8.4/§25.6/§31.3, `registries/refusals.toml` and the compiled catalog
  (`fmine-evidence/src/lib.rs` `REFUSAL_CATALOG`, lines 70–202, which is what the binary emits), only 4 of the
  plan's 25 codes carry the same meaning in all three: 102, 103, 108 and 504. Eighteen differ in at least one source.
  Three plan codes (204 unbudgeted unknowns, 205 calibration required, 503 sibling unavailable) are in neither the
  registry nor the catalog. Three codes cited in CONTRACT.md files exist nowhere else:
  - FMINE-REF-109 and -110 (`fmine-claim/CONTRACT.md:22–23`);
  - FMINE-REF-206 (`fmine-units/CONTRACT.md:29`).

  The only automated check is `fmine-registry/src/lib.rs:90–98`. It verifies that each catalog code ID exists in the
  registry. It does not compare meaning or exit code, so CI cannot detect any of this.
- **Exact conflicting pairs (selected).**
  - **101.** Plan line 698 and BD-147 (line 7824) say conservation. `fmine-crs/src/lib.rs:10,28` emits 101 for a
    CRS mismatch. The plan maps that guard, G-CRS-1, to 106 (line 5604). `fmine-crs/CONTRACT.md:22` maps it to 105.
  - **104.**
    - Plan line 698, GATE-TOUCH-02 (line 4120) and M1 acceptance (line 7416) say non-identifiable attribution.
    - Catalog line 90 says "Density or specific gravity missing".
    - `fmine-time/CONTRACT.md:22` says CalendarWindowMismatch.
  - **301.** Plan line 700 says MNPI wall, exit 7 (line 6714). `fmine-numerics/src/lib.rs:21,35` emits 301 for a
    non-SPD Cholesky matrix; the catalog (line 138) gives exit 5. The registry (line 83) merges the two: "MNPI wall
    boundary violation or non-positive-definite covariance matrix". So the registry was edited to absorb the
    collision instead of resolving it.
  - **303.** Plan line 700 and BD-107 (line 7765, "FMINE-REF-303 tests") say residency. Catalog line 150 says
    "Tailings GISTM TARP red threshold exceeded", exit 5.
  - **304.** Plan line 700 and line 6816 say missing human authority token, exit 7. The registry and catalog say
    "Schedule LP/MILP infeasible", exit 5.
  - **401–404.**
    - The plan has safety-semantic refusals (lines 701 and 6810–6813): ANFO, PAG, TML and stale TSF instruments.
    - The code uses them for other things:
      - 401: "Green on Placeholder", plus an unsupported issuer (`fmine-cli/src/lib.rs:180`) and a malformed
        ClaimId (`:252`);
      - 402: confidentiality;
      - 403: forecast ledger mutation (`fmine-calib/src/lib.rs:41–98`);
      - 404: missing UU term.
    - BD-179 (line 7877) will implement 403 for concentrate custody. The same code would then carry two meanings in
      one binary.
  - **501/502.** The plan has "FA refusal relayed" and "stale anchor" (line 702). The registry has "direct actuation
    attempt" and "digest mismatch". The catalog has "bench-precedence cone" and "digest mismatch".
- **Minimal countermodel.** Run `fmine assure public --issuer newmont`. From the source (lines 176–183 and catalog
  lines 162–166) the envelope carries `code = "FMINE-REF-401"`, `exit_code = 6`, and a "what" text beginning
  "Epistemic gate refuses Green status on Placeholder…". The plan reads 401 as "unknown reactive ground → no ANFO
  plan". It reads exit 6 as "Conflict: lease, fencing, stale anchor" (line 6583). The correct class is a usage
  error, exit 2 (line 6579). Each reader gets a different meaning; at most one can be the intended one.
- **Scope.** Committed code and registries at HEAD; the plan as written. Horizon: M0–M1. GATE-TOUCH-02 is an M1
  acceptance gate whose refusal text would be wrong.
- **Evidence.** E684; new evidence at the file lines above; the output of `refusals.py`.
- **Inference.** §8.4 (line 693) defines refusals as typed values that identify a condition and carry a `fix`. A
  code that names two conditions identifies neither, so the plan, the registry and the code cannot all be the
  specification.
- **Assumptions.** The binary's text comes from the catalog, not the registry. This is true: `Refusal::from_code`
  reads only `REFUSAL_CATALOG`.
- **Alternative or defeater check.**
  - Could this be a deliberate renumbering? No ADR or plan edit records one. The registry header claims to be "the
    canonical … registry (§8.4, §25.6, §31.3, BD-031, BD-039)".
  - Could the plan be internally inconsistent instead? It is not. Its guard tables follow §8.4 (lines 2462, 3226–3229,
    3561–3565, 5599–5608, 6714–6816).
  - NONE_IDENTIFIED_AFTER_BOUNDED_CHECK.
- **Resolution.** Resolved when a parity check that diffs (code, meaning key, exit code) across the §8.4 table,
  `refusals.toml` and the catalog passes. Reversed if the owner adopts the code's numbering by ADR and rewrites
  §8.4, §25.6, §31.3 and every guard table.
- **Materiality.** High. These codes reach M0/M1 outputs and agents. The plan's own beads (BD-107, BD-147, BD-153,
  BD-179) will collide with codes already emitted. The fix is cheap now: about 22 registry rows and about 15 use sites.
- **Uncertainty.** Low. The comparison is mechanical. Classing 105, 106 and 107 as "differs" is my judgment: the
  catalog's 105 is "outside [0,1]" versus the plan's "basis mismatch", and its 107 is "chemical basis mismatch"
  versus "undeclared at import".

### P02-A1-CONSISTENCY-C2 — deductive — the committed epistemic types are not the plan's §7/§8.1 types

- **Statement.** The code differs from the plan in three ways.
  - **Evidence class.** `fmine-evidence/src/lattice.rs:9–20` defines a totally ordered `EvidenceClass`:
    {Placeholder < Assumed < Historical < Calibrated < Exact}. The plan's §7.1 (lines 553–566) and NL-2 (line 596)
    define eight classes under a partial order with an incomparable set: {Measured, Derived, ModelledCalibrated,
    {ModelledUncalibrated, Transferred, Elicited}, Assumed, Assertion}. Only `Assumed` is shared.
  - **Knowledge state.** The code's `KnowledgeState` (lattice.rs:63–70) is {KnownKnown, KnownUnknown,
    UnknownUnknown}. That is the plan's separate `epistemic: EpistemicStatus` field (§8.1 line 621). The plan's
    `state: KnowledgeState // §7.2` (line 624) has nine states that the code cannot represent: known, estimated,
    unknown, conflicted, stale, not_observable, redacted, indeterminate, not_applicable.
  - **Colour.** The code's `ClaimColor` is {Green, Amber, Red} (lattice.rs:93–100). The plan uses
    `fs_evidence::color::Color` (line 623; NL-1 line 595). At the pinned commit that type is {Verified, Validated,
    Estimated} (frankensim `crates/fs-evidence/src/color.rs:209–226` @ `20463525d`).
  - **Provenance.** `lifts.toml` registers lattice.rs as lifted from `crates/fs-evidence/src/lib.rs`, and the
    sha256 `3c62314c…` matches the pinned file. That file contains zero occurrences of `EvidenceClass`,
    `KnowledgeState`, `ClaimColor`, `Placeholder`, `Green` or `project_color`.
- **Consequences.**
  - BD-038 ("Evidence classes (§7.1) and knowledge states (§7.2)", line 7671) and BD-042 ("GATE-PLUMB-E1: 100%"
    of NL-1…NL-8, line 7675) are closed.
  - None of these can be stated over the code's types:
    - NL-2's incomparable classes;
    - NL-5's conflicted, redacted and not_observable states;
    - G-EXEC ("show the state instead of a number" unless known or estimated, line 589);
    - G-DISC (evidence ∈ {Measured, Derived, ModelledCalibrated}, line 684).
  - So GATE-PLUMB-E1, as defined at line 1017, cannot have passed on this code.
  - The M0 public lane labels every workbook cell `EvidenceClass::Exact` + `KnownKnown`
    (`fmine-public/src/lib.rs:165–166`).
  - `fmine claim show clm_<anything>` returns `"evidence_class":"Exact","knowledge_state":"KnownKnown"` without any
    lookup (`fmine-cli/src/lib.rs:258`).
- **Scope.** The Plumb core (WP-02) and everything built on it in M0.
- **Evidence.** New evidence at the file lines above, plus the sibling source. E673 lists BD-038 and BD-042 as
  implemented.
- **Inference.** A value of a 5-class total order cannot faithfully encode an 8-class partial order. A 3-valued
  state cannot encode 9 states. So the plan's laws and the code's types cannot both describe the same Claim.
- **Assumptions.** None beyond reading the definitions as written.
- **Alternative or defeater check.**
  - Could the code be an M0 subset of the plan? No. Four of its five classes are not plan classes, three plan
    classes that M0 needs (Measured, Derived, Assertion) are missing, and making the order total is a semantic change.
  - Could the code be a faithful lift? No: the claimed source does not contain these types.
  - NONE_IDENTIFIED_AFTER_BOUNDED_CHECK.
- **Resolution.** Resolved when `fmine-evidence` and `fmine-claim` expose the §7.1 classes with the NL-2 partial
  order, the §7.2 states and the `fs_evidence` Color, or when an owner-approved ADR rewrites §7–§8. Reversed if the
  owner shows a plan revision that adopts the five-class lattice.
- **Materiality.** High. "Evidence class next to every number" is what Barrick-class readers would see.
  - Today the binary attaches the strongest label it has ("Exact") to issuer-published cells.
  - It attaches the same label to claims it never looked up.
  - That is the loss function's costliest failure: an unsupported statement presented as known.
- **Uncertainty.** Low for the type mismatch. Medium on whether the lift used other fs-evidence modules: I checked
  `lib.rs` and `color.rs` only.

### P02-A1-CONSISTENCY-C3 — deductive — the emitted envelope cannot satisfy INV-002, INV-001 or FMT-ENV-1

- **Statement.** §30.1 (lines 6556–6572) and INV-002 (line 535) require every envelope to carry
  `v, event, node, anchor, data, certificate, flags{reserve_disclosure_claim, execution_authorized,
  geotechnical_signoff, regulatory_compliance_claim}, warnings, next_actions`. They also forbid naked numbers in
  `data` (line 6570; INV-001).
  - The committed `Envelope` (`fmine-cli/src/lib.rs:31–39`) has `schema_version, verb, status, exit_code, anchor_id,
    payload_json, refusal`. It has no flags.
  - The recorded M0 output (E676, `repro/assure_run1.json`) has no `flags` key. Its payload holds bare numbers such
    as `"computed_aisc_per_oz":1480.49`, with no Claim, evidence class or citation.
  - BD-111, "FMT-ENV-1 envelope with the four flags false" (line 7775, done = PO-ADIT-1 golden test), is closed. No
    such golden test exists: fmine-cli has 2 tests (E674), and neither checks flags.
  - `fmine-decide/CONTRACT.md:14` says it enforces INV-002 through four other fields (`authoritative`,
    `may_actuate`, `is_safe_certification`, `is_signing_basis`). None of them exists in `fmine-decide/src`.
- **Countermodel.** E676's envelope is a single counterexample. "The four flags are false in every envelope" is
  false of it, because absent fields are not false fields.
- **Scope.** All CLI output at HEAD.
- **Evidence.** E676, E674; new evidence at the file lines above.
- **Inference.** Direct.
- **Assumptions.** None.
- **Alternative or defeater check.** Is the `fmine.env.v1` label a deliberate interim schema? No document says so,
  and BD-111 and BD-112 are closed against FMT-ENV-1. NONE_IDENTIFIED_AFTER_BOUNDED_CHECK.
- **Resolution.** Resolved when PO-ADIT-1 runs as a golden test over every emitted schema and passes. Reversed
  only by amending INV-002 and §30.1.
- **Materiality.** High to moderate. INV-002 is a non-negotiable (no reserve claim, no execution, no sign-off, no
  compliance claim), and the M0 output is the artifact shown to finance reviewers (BD-075).
- **Uncertainty.** Low.

### P02-A1-CONSISTENCY-C4 — deductive — INT-AS-001 cannot pass at commit 1, yet it is recorded PASS; the INT-NUM-001 record contradicts the lockfile

- **Statement.**
  - §38 (line 7567) puts INT-AS-001 in WP-00 at "Commit 1". Its closing bead, BD-257 (line 7634), depends only on
    BD-001.
  - §34.2 (lines 7096–7107) admits asupersync "only when" nine criteria hold. Among them:
    - (1) the same end-to-end scenario passes under production and lab time;
    - (4) every obligation reaches a terminal or an explicit indeterminate state;
    - (5) the runtime-mask lab test (BD-097, WP-05);
    - (7) ATP corruption and resume campaigns (BD-105, BD-106, BD-109, BD-110, WP-05).
  - These need WP-04 and WP-05 work (BD-077, BD-089, BD-097, BD-099), which is M0–M1 at the earliest. So the gate
    as defined cannot pass at commit 1.
  - `docs/delivery/INT-AS-001.md` nevertheless records "Verdict: PASS".
  - No committed manifest declares asupersync, fsci, franken-evidence or franken-decision as a dependency, and there
    is no `[patch.crates-io]` (contrary to BD-001's scope, line 7619; BD-001 is closed). The only mentions are the
    metadata keys `sole_async_runtime = "asupersync"` and `asupersync_pin = "=0.5.0"` (`Cargo.toml:42–43`).
    `Cargo.lock` is untracked in the working tree, not committed. It agrees: 16 packages, all workspace-local.
  - `docs/delivery/INT-NUM-001.md` states "`fsci-* =0.4.0` is consumed with `default-features = false`". No
    manifest declares an fsci, fp, fnp or fj dependency, and the lockfile has none.
- **Countermodel.**
  - INT-AS-001: criterion 1 needs a LabRuntime run. BD-099 (LabRuntime wiring) is open, and asupersync is not in
    the build. So criterion 1 is unmet, and "PASS" contradicts §34.1 (a pass needs a recorded measurement against
    its threshold).
  - INT-NUM-001: "fsci consumed" together with "no manifest depends on fsci" is a direct contradiction.
- **Scope.** Admission gates and their records.
- **Evidence.** E673, E684; new evidence: `Cargo.toml` and all crate manifests, the untracked `Cargo.lock`,
  `docs/delivery/*.md`, and plan lines 7096–7107, 7567, 7634.
- **Inference.** Direct. One note: BD-308 explicitly allows "or the FD-row fallback taken" (line 7635). The truthful
  INT-NUM-001 record would be that the fallback was taken, not that fsci was consumed.
- **Assumptions.** None.
- **Alternative or defeater check.** Could INT-AS-001 be read narrowly as "closure checks only", as BD-257's title
  suggests? Then BD-257's title and §34.2's "admitted only when" list contradict each other. That is the plan defect
  in its own right. NONE_IDENTIFIED otherwise.
- **Resolution.**
  - Resolved when the plan splits INT-AS-001 and the records state "NOT_APPLICABLE: asupersync not yet in the
    closure" and "FD-row fallback taken".
  - Reversed if asupersync enters the closure and criteria 1–9 are measured.
- **Materiality.** High under the loss function: these are gates passing vacuously or on false inputs, in public
  records. Cost: two record edits and one plan edit.
- **Uncertainty.** Low.

### P02-A1-CONSISTENCY-C5 — deductive — GATE-DATA-03 as defined cannot be met by the M2 pilot's own "recovery-capable" data set

- **Statement.**
  - §37.3 (lines 7446–7448) names "the recovery-capable GATE-DATA-03 set" for Lumwana:
    - the metallurgical balance;
    - sampler and assay records;
    - shipment and settlement assays;
    - feed tags;
    - the monthly reconciliation pack.
  - §34.4 (line 7158) defines GATE-DATA-03 as needing block and GC models, dispatch tonnages, surveys and
    cost-centre extracts, and needing GATE-PORT-02.
  - GATE-PORT-02 (line 6952) is one full acQuire/Datamine export.
  - BD-204 (line 7920) closes both gates and depends on BD-188 (acQuire), BD-189 (Datamine/Surpac) and BD-194 (the
    OMF v2 reader). BD-194 in turn depends on BD-190 (the omf-rust audit, KILL-6) and BD-191 (the VdbGrid lift).
  - M2 acceptance 1 (GATE-TOUCH-03, F1/F2/F3 parity "from the same inputs", lines 4121 and 7476) also needs
    grade-reconciliation inputs that are not in the recovery set.
- **Countermodel.**
  - Suppose Lumwana delivers exactly the §37.3 set by month 7.
  - Per §37.3, the recovery finding is enabled.
  - Per §34.4, GATE-DATA-03 is missed, because there are no block or GC models and no acQuire/Datamine export.
  - The stated fallback then fires: "No data by month 7 → M2′" (lines 7158 and 7484).
  - The plan therefore asserts both "this set suffices" and "this set means M2′".
- **Scope.** M2 (months 6–12) and the month-12 program gate (§34.5).
- **Evidence.** Plan lines 4121, 6952, 7158, 7446–7448, 7920. Context: §34.4 says M2′ is the modal path at about
  0.7.
- **Inference.** Direct.
- **Assumptions.** "The recovery-capable GATE-DATA-03 set" is read as a set that meets GATE-DATA-03. If it means
  only "the subset relevant to recovery", then the plan never says whether delivering only that subset avoids M2′.
  That is the same decision gap.
- **Alternative or defeater check.** BD-199 already distinguishes "recovery-led" (Lumwana) from "grade-led" (North
  Mara). That suggests the intent was a mechanism-specific data gate that was never written into §34.4 or §39.
  NONE_IDENTIFIED otherwise.
- **Resolution.** Resolved when GATE-DATA-03 has a recovery-led variant, and BD-204's dependencies on BD-188, BD-189
  and BD-194 apply only to the grade-led variant.
- **Materiality.** High for M2. The defect decides between M2 and M2′ and puts geometry lifts on the pilot's critical
  path.
- **Uncertainty.** Low for the text conflict; medium on intent.

### P02-A1-CONSISTENCY-C6 — deductive — the M0 report bead breaches the §37.4 dependency firewall

- **Statement.**
  - §37.4 (line 7549) says "No milestone's executive deliverable may be blocked by a sibling repo". A sibling may
    enter only behind an adapter with a working local fallback.
  - BD-072 (the M0 report, line 7715) depends on BD-123 (line 7787). BD-123's only done-condition is
    "INT-FMD-001, INT-FMERM-001": admission of the franken_markdown and frankenmermaid siblings, with no fallback
    disjunct.
  - BD-072 lies on the 29-bead critical path (line 7601), in the order BD-072 → BD-074 → BD-075 → BD-199 → … WP-12.
  - Appendix F.1 (line 8416) does give the fallback ("Plain markdown rendering without diagrams"). BD-308 shows the
    pattern the plan uses elsewhere: "… or the FD-row fallback taken" (line 7635).
  - §37.4's M0–M2 "consumed" column does not list franken_markdown or frankenmermaid.
- **Countermodel.** If INT-FMERM-001 fails (for example, a HiGHS feature in the closure), BD-123 cannot close. Then
  BD-072, BD-074 (GATE-E2E-01 for M0), BD-075 (the finance review) and BD-199 (site selection) cannot close either,
  so an M0 executive deliverable is blocked by a sibling. This is exactly what §37.4 forbids.
- **Scope.** M0 and everything after it on the critical path.
- **Evidence.** Plan lines 7549, 7601, 7635, 7715, 7787, 8416.
- **Inference.** Direct.
- **Assumptions.** None.
- **Alternative or defeater check.** F.1's fallback might be meant to apply implicitly. The bead rule (line 7611)
  closes beads only on cited evidence, and BD-123's done-condition names only the admission gates. NONE_IDENTIFIED.
- **Resolution.** Resolved when BD-123's done-condition reads "INT-FMD-001 and INT-FMERM-001 admission records, or
  the FD-FMD-00 fallback (plain markdown without diagrams) taken", and §37.4 lists both libraries for M0–M2.
- **Materiality.** Moderate. Today it is a latent blocker on the critical path. The fix is one cell.
- **Uncertainty.** Low.

### P02-A1-CONSISTENCY-C7 — deductive — the M0 crate scope omits beads that every GATE-BULL-01 bead depends on, and four closed beads have open blockers

- **Statement.**
  - §37.3 line 7405 gives M0's crates as "WP-00 to WP-03 and WP-06 slices".
  - The union of the dependency closures of the M0 deliverable beads (BD-060, -065, -066, -067, -068, -071, -072,
    -073, -074, -075, -271, -272, -273, -274) reaches two beads outside those WPs:
    - BD-090 (WP-04, FMT-CAS-1), through BD-056 → BD-271 → BD-059 → BD-065. Ten M0 beads need it.
    - BD-142 (WP-07, the e2e harness), needed by BD-074 and BD-075.
  - The M0 commit followed line 7405 (E673: "implement WP-00..WP-03 and WP-06") and closed beads that have open
    blockers:
    - BD-056 (blocker BD-090);
    - BD-053, which carries GATE-PLUMB-E2 (blocker BD-080);
    - BD-117, `fmine doctor` "chains … obligations" (blockers BD-088 and BD-089);
    - BD-275 (blocker BD-070, which carries GATE-HALL-01).
  - BD-065 and BD-272, which carry GATE-BULL-01, are closed while BD-090 is open in their closure.
- **Countermodel.** `issues.jsonl` records BD-056 as closed with a "blocks" edge from BD-090, which is open. A
  blocked bead cannot have finished, so the bead state contradicts its own dependency graph.
- **Scope.** M0.
- **Evidence.** E673, E680; the outputs of `beads_parse.py` and `closure.py`; plan lines 7405, 7699, 7745.
- **Inference.** Direct.
- **Assumptions.** Reading `br` "blocks" edges as precedence, which is the plan's meaning (line 7596).
- **Alternative or defeater check.** §38 itself marks WP-04 as "M0–M1" and WP-07 as "the e2e harness in M0"
  (lines 7571, 7574). So §38 is right and line 7405 is incomplete. NONE_IDENTIFIED.
- **Resolution.** Resolved when line 7405 names BD-090 and BD-142 (or the edges BD-056→BD-090 are removed by plan
  edit), and the four beads are reopened or their blockers closed.
- **Materiality.** Moderate. The scope line guided the implementing agent into closures the graph forbids.
- **Uncertainty.** Low.

### P02-A1-CONSISTENCY-C8 — deductive — the bead-closure evidence rule is contradicted by all 87 closures; several closed done-conditions are false of the repository

- **Statement.**
  - The rule at line 7611 says: "A bead closes only with cited evidence: a gate record, a test name or a decision
    entry."
  - All 87 closed beads carry one identical reason: "Implemented and verified by cargo test --workspace and xtask
    check-all (GATE-LAYERS-00, GATE-BULL-01)".
  - GATE-BULL-01 has no gate record (`docs/delivery/` holds only INT-AS-001 and INT-NUM-001). Its own acceptance
    needs beads that are open (line 7397):
    - BD-068, the forecast hash before the February 2027 release;
    - BD-072, byte-identical report reruns;
    - BD-074, GATE-E2E-01 for M0.
  - These closed beads promise scope, in their §39 row, that is false of the repository at HEAD. Where marked (D),
    the bead's own done-when column is also unmet. In the other rows the done-when is narrower than the scope and
    may be met.

    | Bead (line) | Scope promised in the §39 row | What the repository shows |
    |---|---|---|
    | BD-001 (7619) | "`[patch.crates-io]` for one asupersync" | None exists. The done-when, `cargo check` green, is met |
    | BD-002 (7620) | Template with "the ten §0.2 sections plus API, POs, budget" | See C9 |
    | BD-014 (7632) | "ADR-RNG-01 and the naming ADR" | `docs/adr/` holds only ADR-RNG-01 |
    | BD-039 (7672) | "the FMINE-REF registry (§8.4, §25.6)" | See C1 |
    | BD-111 (7775) (D) | Envelope "with the four flags false"; done-when is a PO-ADIT-1 golden test | See C3; no such test |
    | BD-113 (7777) (D) | "schema and generator"; done-when is generator golden output | No generator. The verb dispatch is a hard-coded `match` (`fmine-cli/src/lib.rs:141–160`), and the robot-docs verb list is a string literal (`:269`) |
    | BD-115 (7779) (D) | "`claim show`, `why`, `trace`"; done-when is golden outputs | `why` and `trace` do not exist. `claim show` returns a fixed payload for any `clm_` id (`:258`) |
    | BD-038 and BD-042 | Lattice and GATE-PLUMB-E1 | See C2 |
- **Countermodel.** Take BD-311 (spatial-front PIT monitor), which is closed. Its done-condition is a
  synthetic-truth alarm test. Its close reason cites GATE-BULL-01, which concerns public metric reproduction and has
  no record. The cited evidence is therefore not evidence for this bead, and line 7611 is violated.
- **Scope.** The bead state that E673 and E680 report.
- **Evidence.** E673, E680; `issues.jsonl` `close_reason` (one distinct value); new evidence at the file lines above.
- **Inference.** Direct.
- **Assumptions.** None.
- **Alternative or defeater check.** I did not verify every one of the 87 closures. Some may be substantively done.
  The claim is about the evidence rule and the eight checked beads only.
- **Resolution.** Resolved when each closed bead's reason names its own test or record, and the beads above are
  reopened or fixed.
- **Materiality.** Moderate to high. Bead state is what AGENTS.md and agents use to plan the next work, and the
  commit message (E673) repeats it publicly once pushed.
- **Uncertainty.** Low for the checked beads.

### P02-A1-CONSISTENCY-C9 — deductive — the CONTRACT.md template and all crate contracts lack the §0.2 sections, which AGENTS.md says they have

- **Statement.**
  - Plan §0.2 item 2 (line 44) requires the CONTRACT.md sections purpose, inputs, outputs, invariants, refusals,
    determinism class, evidence class of outputs, validity domain, oracles and known gaps.
  - AGENTS.md line 142 and lines 439–450 say each crate's CONTRACT.md, and the template
    `docs/templates/CONTRACT.md`, has those ten sections.
  - The template's headings are different. They are Charter & Layer; Mathematical & Domain Formulation; Authority,
    Epistemics & Representation; Public API Surface; Guards & Typed Refusals; Proof Obligations; Failure Modes;
    Gates & Kill; Lifted Code; Non-Goals.
  - All 15 product contracts use that set.
  - 14 of the 15 state LOC budgets that differ from `loc_budget.toml`. Examples: fmine-cli 14k/16k against 26k/30k;
    fmine-io 16k/18k against 40k/46k.
  - `fmine-cli/CONTRACT.md` §2 and §5 state an exit-code taxonomy of "0, 10, 20, 30, 40, 50". That contradicts §30.1
    (0, 2–9) and the code (2–8).
- **Countermodel.** Item 2 of the §0.2 definition of "implemented" cannot hold for any committed crate. No contract
  has a determinism-class, validity-domain, oracles or known-gaps section.
- **Evidence.** The output of `contracts.py`; plan line 44; AGENTS.md lines 142 and 439–450; the template headings.
- **Inference.** Direct.
- **Assumptions.** Heading-level matching. The content of sections 3 and 7 partly overlaps the §0.2 list.
- **Alternative or defeater check.** None.
- **Resolution.** Resolved when the template's headings are the §0.2 ten, plus API, POs and a budget pointer, and
  the 15 contracts are migrated.
- **Materiality.** Moderate. It blocks every "implemented" claim and lets contracts drift from the registries.
- **Uncertainty.** Low.

### P02-A1-CONSISTENCY-C10 — deductive (conditional) — the site-decision calendar conflicts with the bead order, and GATE-DATA-02 carries an unstated earlier deadline

- **Statement (site decision).**
  - §37.3 (lines 7466–7469) fixes the site decision by calendar: month 1, ask; month 2, "the pilot is Lumwana";
    month 3, open alternates; month 4, no agreement means M2′.
  - §34.4 (lines 7174–7177) moves P(site data) on a named sponsor by month 2.
  - In §39, BD-199 (site selection, "Decision in CELL-DEC", line 7915) depends on BD-075, the M0 finance review.
    BD-075 depends on BD-074, which depends on BD-072 and BD-142.
  - Line 7603 confirms "pilot-site selection waits for the M0 finance review".
  - No bead represents the month-1 sponsor request.
  - M0's window is months 0–3 (line 7378).
- **Countermodel (site decision).** Suppose M0's report and finance review complete in month 3. That is allowed by
  the M0 window. Then BD-199 cannot close before month 3, and the month-2 decision in §37.3 cannot be recorded. So
  both texts hold only under an unstated premise: the M0 finance review completes by month 2.
- **Statement (GATE-DATA-02).** BD-201 (GATE-DATA-02, month-5 deadline, line 7157) depends on BD-110 (INT-ATP-001),
  which depends on BD-109 (GATE-SYNC-01). M1 allows GATE-SYNC-01 until month 6 (M1 window at line 7407; acceptance at line 7417). So GATE-DATA-02 silently
  requires GATE-SYNC-01 and INT-ATP-001 by month 5.
- **Scope.** Months 1–7.
- **Evidence.** Plan lines 7157, 7174–7177, 7378, 7407, 7417, 7466–7469, 7603, 7915–7917.
- **Inference.** The conflict holds in every schedule where M0's review falls in month 3, or GATE-SYNC-01 falls in
  month 6.
- **Assumptions.** "The pilot is Lumwana" is the decision BD-199 records.
- **Alternative or defeater check.** The plan may intend the review to finish early. No text says so. NONE_IDENTIFIED.
- **Resolution.** Resolved by either fix:
  - add a WP-12 bead for the sponsor request and naming (months 1–2) with no M0 dependency, and make BD-200 depend
    on it; or
  - state that BD-075 completes by month 2 and BD-109 by month 5.
- **Materiality.** Moderate. Data access is the dominant uncertainty (§34.4), and the bead graph currently delays the
  actions that move it.
- **Uncertainty.** Medium, because it is conditional on the schedule.

### P02-A1-CONSISTENCY-C11 — deductive — GATE-BULL-02's milestone is stated twice, differently

- **Statement.**
  - §21.8 (line 4591) sets GATE-BULL-02 at M1.
  - Its only closing bead, BD-221 (line 7942), sits in WP-13, whose §38 milestone is M3 (line 7580). §38 lists
    GATE-BULL-02 among WP-13's gates.
  - The M1 acceptance list (lines 7413–7417) omits it.
  - BD-221's dependencies (BD-061, closed; BD-134, WP-07) allow M1.
  - Appendix H.4 row 6 (line 8561) lists the gate milestone conflicts that were fixed (GATE-SYNC-02 and
    GATE-HALL-02). GATE-BULL-02 is not among them.
- **Evidence.** The output of `gate_timing.py`.
- **Inference.** Direct.
- **Assumptions.** None.
- **Alternative or defeater check.** None.
- **Resolution.** Resolved by either fix:
  - move BD-221 to WP-07 or WP-03 (ids are stable when moved, line 7605) and add GATE-BULL-02 to M1 acceptance; or
  - change line 4591 to M3.
- **Materiality.** Low to moderate.
- **Uncertainty.** Low.

### P02-A1-CONSISTENCY-C12 — question — missing premises the code is already filling ad hoc

- **(a) Exit codes.** §30.1 (lines 6576–6588) does not give the exit code for 2xx evidence refusals or 4xx
  safety-semantic refusals. The registry and catalog assign exit 4 ("environment or degraded mode") and exit 6
  ("conflict"). The plan's table gives exit 3 to domain-law refusals and exit 7 to "safety or authorization", so
  1xx → 3 and 4xx → 7 is the likely intent. The table never says so.
- **(b) Missing refusal families.** The code needs refusals for usage errors, numerical failures (non-SPD,
  infeasible LP), digest mismatches and forecast-ledger mutation. §8.4 has no family for them, and the code
  borrowed 301, 304, 403 and 502.
- **(c) Evidence class of a public-filing value.** §7.1 does not say what class an issuer-published workbook value
  takes: Measured, Derived, or Assertion ("an imported claim", line 564). The code chose "Exact" (see C2).
- **(d) INV-009.** INV-009 (line 542) has no enforcing bead, PO or gate anywhere in §39. It is the only invariant
  in that position apart from those enforced by architecture.
- **Materiality.** Moderate. Each gap is filled by code today, inconsistently.
- **Resolution.** Owner decisions recorded in §8.4, §30.1, §7.1 and §39.

### P02-A1-CONSISTENCY-C13 — observation — lower-materiality traceability defects (each checked)

- **Lift guard tests.** 5 of 8 `lifts.toml` guard tests do not exist anywhere in the workspace:
  - `test_compose_never_promotes`;
  - `test_betting_eprocess_ville_bound`;
  - `test_normal_evsi_closed_form`;
  - `test_cvar_rockafellar_uryasev`;
  - `test_max_plus_critical_cycle`.

  Tests with different names exist. `xtask check_lifts` (`crates/xtask/src/lib.rs:424–`) checks headers only.
- **Wrong IDs in code comments.**
  - INV-011 (the ISA-95 write path) is cited for the Refusal type (`fmine-evidence/src/lib.rs:10`).
  - INV-012 (cross-cell references) is cited for grade non-additivity (`fmine-units/src/lib.rs:95,194,228,560`).
    That property is INV-014.
  - BD-026 (Money) is cited for the chemical basis (`:560`). That is BD-277.
- **Public README.** README.md line 65 says "Nothing runs end to end yet". BD-071 ("`fmine assure public` end to
  end") is closed, and E676 shows the command exiting 0. At least one of the two is wrong.
- **Lint rule.** AGENTS.md line 289 says `unwrap_used`, `expect_used` and `panic` are denied. The workspace does not
  deny them (E681), and production code calls `.expect` (`fmine-public/src/lib.rs:455`). The plan is silent on this.
- **Materiality.** Low individually. Together they show that traceability IDs in code cannot yet be trusted.

### P02-A1-CONSISTENCY-C14 — recommendation — make one refusal source of truth and gate it in CI

- **Supports.** C1, C12(a,b).
- **Action.**
  - Rewrite `registries/refusals.toml` to match §8.4, §25.6 and §31.3 code by code, with an explicit exit code
    per family.
  - Add a new family for numerical, solver and integrity failures, and use exit 2 for usage errors.
  - Make `REFUSAL_CATALOG` derived from the registry, not hand-maintained.
  - Make the registry check compare code, meaning key and exit code in both directions.
  - Fail on any CONTRACT.md or source code ID absent from the registry.
  - Reopen BD-039.
- **Plan edits.**
  - §8.4 (lines 697–702): add the new family and an exit-code column.
  - §30.1 (lines 6576–6588): state the exit code for 2xx and 4xx.
- **Cost.** About half a day.
- **Reversibility.** Full.
- **Acceptance test.** `xtask check-all` fails when any of the 25 plan codes, its registry row or its catalog entry
  disagree. A seeded mismatch (for example, 104 text changed) turns CI red.

### P02-A1-CONSISTENCY-C15 — recommendation — five plan edits that remove the structural contradictions

- **Supports.** C4, C5, C6, C7, C10, C11.
- **Edit 1 (§34.2 lines 7081/7096 and §38 line 7567).** Split INT-AS-001:
  - INT-AS-001a covers the closure: the one package id, criterion 9, and `[patch.crates-io]`. It is WP-00 and must
    record NOT_APPLICABLE while asupersync is absent.
  - INT-AS-001b covers criteria 1–8. It belongs in WP-05 at M1 and depends on BD-089, BD-097, BD-099 and BD-110.
- **Edit 2 (§34.4 line 7158; §37.3 lines 7446–7448; §39 line 7920).**
  - GATE-DATA-03 must state a recovery-led threshold (the §37.3 set) and a grade-led threshold (the current text
    plus GATE-PORT-02).
  - BD-204's dependencies on BD-188, BD-189 and BD-194 apply only when grade-led.
  - Add a bead for importing the metallurgical balance and sampler records.
- **Edit 3 (line 7787 and the §37.4 M0–M2 row at line 7554).** BD-123's done-condition reads "… or the FD-FMD-00 fallback taken", and
  §37.4's M0–M2 consumed column lists franken_markdown and frankenmermaid.
- **Edit 4 (line 7405).** M0's crates line reads "WP-00 to WP-03 and WP-06 slices, plus BD-090 (WP-04) and BD-142
  (WP-07)".
- **Edit 5 (line 4591 or line 7942 and §38).** Set GATE-BULL-02 to one milestone. Also add a sponsor-request bead
  in WP-12 (C10).
- **Cost.** About 1 hour of plan editing plus the `br` updates.
- **Reversibility.** Full.
- **Acceptance test.** Re-run `gate_timing.py` and `closure.py`. No gate's closing bead may sit in a WP that starts
  after the gate's stated milestone, and no M0 bead may depend on a bead outside the M0 scope line.

### P02-A1-CONSISTENCY-C16 — recommendation — correct the records and bead state before `main` is pushed

- **Supports.** C2, C3, C4, C8, C9.
- **Action.**
  - Before pushing (E673: the commit is not yet pushed), edit `docs/delivery/INT-AS-001.md` to NOT_APPLICABLE and
    `INT-NUM-001.md` to "FD-row fallback taken".
  - Reopen BD-001, -002, -014, -038, -039, -042, -053, -056, -065, -111, -113, -115, -117, -272 and -275, or record
    in each why it is done, citing its own test or record.
  - Fix `docs/templates/CONTRACT.md` to the §0.2 sections.
  - Replace the 87 identical close reasons with per-bead evidence.
- **Cost.** About 2 hours.
- **Reversibility.** Full before the push; partial after it.
- **Acceptance test.** No closed bead has an open blocker (rerun `beads_parse.py`). Every close reason names a test
  or a record that exists. Both delivery records agree with the dependencies the manifests declare.

## NULL OR BLOCKED RESULT

Not blocked. The integrity of the plan's own graph and crate map holds (N1–N4). The inconsistencies are between the
plan and the committed registries, code, records and bead state (C1–C4, C8, C9), and in five plan-internal timing
and gate definitions (C5–C7, C10, C11). The one execution I could not do, running the debug binary, does not affect
any claim: every CLI behaviour cited is read from source or from the recorded E676 output.
