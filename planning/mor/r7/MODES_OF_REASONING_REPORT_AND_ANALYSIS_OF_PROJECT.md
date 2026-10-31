# FrankenMining — Audited Multi-Mode Analysis (Round 7)

**Primary question:** Before the M0 gate, which material claims, numbers, gates, dependencies and design choices in
the FrankenMining plan and its committed M0 code are wrong, unsupported, inconsistent or mis-sequenced, and which
changes most raise the chance that M0–M2 produce outputs a Barrick-class executive committee, QPs and site teams
would trust and use?
**Scope:** the plan, `planning/`, README, AGENTS.md, `crates/`, `registries/`, `docs/`, `.beads/`, CI. Excluded:
substrate changes, implementation, feature ideation (run separately in `planning/duel/`).
**Decision horizon:** now to the month-12 program gate, weighted to M0–M2.
**Project revision / evidence cutoff:** HEAD `68e7091` (unpushed; origin `main`/`master` at `30ee8f5`), 2026-10-29.
**Run ID:** `20261029T183653Z`. Case: `MOR_CASE.md`. Ledger: `MOR_CLAIMS.jsonl` (171 claims).

## Bottom Line

The plan's design holds up better than its first implementation. The plan's cross-references are sound: §39 and
`.beads/issues.jsonl` agree exactly (320 beads, 533 edges, acyclic, 29-bead critical path; C068, C074), §38 equals
the transitive reduction of the bead graph (C069), §28.3 equals the LOC registry (C070), every inter-crate edge
points downward (C071), and the recomputed galaxy-brain cards hold (C055, C056, C125).

The committed M0 slice does not do what its commit message and bead closures say. Every number in
`fmine assure public --issuer barrick` comes from an in-code stand-in workbook or from literals, not from Barrick's
files (C001, C011, C059). Several of those numbers are presented as Barrick's and are wrong: FY2025 AISC 1,638
against a published 1,637; 3,255 is production, not ounces sold; the corporate adjustment has the wrong sign; the
North Mara waterfall is labelled with periods it does not match; the reserve-replacement inputs were back-solved
(C002–C006, C159). With the real workbooks in place the command refuses; with any file missing it silently
succeeds (C058, C059). All 87 closed beads share one generic close reason, and at least 23 have unmet done-when
conditions (C013, C015). Two integration records say PASS for crates that are not in the build (C017).

Governing value (C048): the costliest error is a false statement reaching Barrick-class management. Under that
value, the commit must not be pushed as it stands.

### Recommended posture

**Action class:** DOMINANT for the pre-push corrections (R001–R003); ROBUST for the gate and forecast redefinitions
(R004–R007); CONDITIONAL for the disclosure and data-terms items, which need the owner's or counsel's authority
(R008–R009).

Do not push 68e7091 as is. Remove the silent fallbacks, label or remove the stand-in, correct the bead state and
the two INT records, and then rebuild the importer on label anchors against a pinned registry of the 18 real
workbooks before the Q3 2026 release (9 Nov 2026, 06:00 ET). Redefine GATE-BULL-01 so it can be both passed and
failed, and turn the FY2026 forecast into a scoreable, externally time-stamped record.

### Confidence basis

High for the code and data findings: every one was reproduced by execution (the HEAD binary run against real and
altered workspaces) and by an independent reader of the workbooks, and checked by an auditor who did not write the
claim. High for the plan-consistency findings (mechanical comparisons, reproduced). Moderate for the statistical
gate findings (exact binomial computations; the readings of ambiguous gate text are stated). Moderate to low for
the disclosure items: the regime facts are verified, but how they apply is for counsel.

## Decision Contract

- **Decision owner / authority:** the repository owner. Plan edits, bead state, history rewrites and pushes are
  the owner's call; this report proposes.
- **Success criterion:** audited corrections that can be written into the plan with evidence ids, plus checked
  nulls.
- **Most costly errors:** a false statement reaching Barrick-class management; a gate that passes vacuously or on
  fabricated inputs. Next: a gate that cannot be passed; a hidden blocking dependency.
- **Attributed values / obligations:** the owner's goals as recorded in the case (C048, attributed, not verbatim);
  AGENTS.md RULE 0.1 (no faked tests, no fixtures presented as live proof); plan §0.2 (definition of
  "implemented"), line 7611 (beads close with cited evidence); securities rules as cited in C132, C135.
- **Project identity / non-negotiables:** public lane only; never classify reserves; no number without a source;
  refuse rather than plug.
- **Deployment / use context:** nothing deployed. The repository is public and already forked by a third party
  (C053). Remote is still at 30ee8f5 (C047).
- **Reversible choices:** everything except what is pushed.

## Verified Material Findings and Decision Inputs

### C001 / C011 / C059 — the M0 output is synthetic and says it is not

**Status:** VERIFIED (C001, C059); PARTIALLY_SUPPORTED (C011, scope narrowed). **Type:** observation, deductive.
**Decision relevance:** Critical.

**Supported statement:** at HEAD every figure in the E676 payload comes from `build_canonical_barrick_xlsx_bytes`
or from literals in `run_barrick_m0_assurance`. `BarrickCorpus::load_from_workspace` substitutes the stand-in for
any file whose read fails (not found, a directory, mode 000), per file and without a flag; with the Q4 2025 AISC
file absent the command exits 0 with status `ok`.
**Evidence:** E677–E679; crates/fmine-public/src/lib.rs 224–265, 425–; AUD01, AUD02 executions; P01 workspaces.
**Independent evidence paths:** static trace; execution of the HEAD binary on five workspace variants; an
independent ElementTree census of the 22 local workbooks (50,294 cells; C159).
**Alternative / defeater check:** "the stand-in is a deliberate fixture" — it is not labelled as one; its source
comment calls them verified Barrick cell coordinates, and none of its 41 explicit values sits at a published
coordinate (C159).
**Action implication:** R001.

### C002–C006 — Barrick figures in the output are wrong or unsourced

**Status:** VERIFIED (C003–C006); PARTIALLY_SUPPORTED (C002). **Decision relevance:** Critical.

**Supported statement:** FY2025 gold AISC is published as $1,637/oz on 3,318 koz sold ($5,428M); the payload uses
1,638 and 3,255 (attributable production). FY2024's 3,973 matches no gold-ounce cell (produced 3,911, sold 3,798).
The corporate layer is positive in both years, so −49 and −8 have the wrong sign (C003). The North Mara block is
labelled 2024 Q1→Q2 with values that match no cell; the real large move is Q2 2025 $1,292 → Q2 2026 $2,577 (C004,
C054). Reserve replacement 190%/504% comes from inputs (3.91, 7.429, 12.277) that were solved to hit the targets;
the filings give three-year 2023–25 figures, gold ounces versus gold-equivalent (C005, C131). The price decks in
`fmine-metrics` are shifted by one year (C006).
**Evidence:** Q4_2025 Mine Stats YTD!G22, C22, D22; Q4_2024 YTD!C23/D23; Q4_2025 AISC sheets '2025'/'2024' row 17;
2026 Circular p.41 (E604); E606, E613.
**Action implication:** R001, R002. No company-level figure may leave the system without its filed page (R006).

### C013 / C015 / C016 / C017 — bead and gate records overstate

**Status:** VERIFIED. **Decision relevance:** Critical.

**Supported statement:** all 87 closed beads share the reason "Implemented and verified by cargo test --workspace
and xtask check-all (GATE-BULL-01 …)", closed within 12 seconds. Four were closed over open blockers (C014; br
needs `--force` for that). At least 23 closed beads have unmet done-when conditions, including BD-065 and BD-272
(no GATE-BULL-01 scorer exists), BD-111 (no INV-002 flags in the envelope) and BD-225 (no golden cards). The
INT-AS-001 and INT-NUM-001 records say PASS while no external crate is in the build graph (C017, C018, C083).
**Action implication:** R002.

### C023 / C024 / C050 / C110 — GATE-BULL-01's 95% bar is ambiguous and its stated basis is a different test

**Status:** VERIFIED (C023 part, C024, C050); PARTIALLY_SUPPORTED (C110). **Decision relevance:** High.

**Supported statement:** 41–46% of numeric cells in the ten known workbooks sit in no instance of the plan's
named identities, so for them "reproduced" reduces to extraction fidelity. A perfect extractor reaches 90.5–92.4%
if a failing identity disqualifies every participating cell and 98.0–99.7% if it disqualifies only the computed
cell. The 95% bar was set from C1's T1 pass rate of 84/89 (94.4%), a different quantity.
**Action implication:** R004.

### C025 / C026 / C041 / C157 — the Q3 2026 blind test and the FY2026 forecast are timing-bound

**Status:** VERIFIED (C025, C026, C041); PARTIALLY_SUPPORTED (C157). **Decision relevance:** High.

**Supported statement:** the Q3 2026 workbook arrives on 9 Nov 2026 at 06:00 ET (issuer notice). It can serve as
an out-of-sample importer test only if a label-anchored importer exists and its revision is publicly time-proved
before then. Forecasts hashed after that release are mostly a Q4 nowcast (Q4 was 26.8% of FY2025 attributable
production). At HEAD, the importer refuses the real files (FMINE-REF-502).
**Action implication:** R003, R005.

### C060 / C065 / C022 / C147 — no gate can currently fail for the right reason

**Status:** VERIFIED. **Decision relevance:** High.

**Supported statement:** there is no scorer, denominator or site-quarter enumeration; the state root does not
change when an input cell changes; the envelope lacks the four INV-002 flags, certificate, `next_actions` and
`no_claim` line required by FMT-ENV-1 and G-BULL-6.
**Action implication:** R004, R006.

### C080 / C081 / C087 / C088 / C096 / C099 / C150 — code and plan disagree on contracts

**Status:** VERIFIED. **Decision relevance:** Moderate (High for C150 before asupersync enters).

**Supported statement:** refusal codes mean different things in the plan, `registries/refusals.toml` and the
compiled catalogue; the evidence types are not §7.1/§7.2's; the CONTRACT template lacks six of §0.2's ten
sections and 14 of 15 LOC budgets differ from the registry; five of eight lift guard tests do not exist;
`unwrap_used`/`expect_used`/`panic` are not denied despite AGENTS.md; `deny.toml` would fail asupersync 0.5.0
(licences, duplicates) and is not run in CI.
**Action implication:** R007.

### C118 / C120 / C121 / C122 / C124 — several statistical gates cannot discriminate

**Status:** VERIFIED (C120, C122, C124); PARTIALLY_SUPPORTED (C118, C121). **Decision relevance:** Moderate.

**Supported statement:** no positive calibration claim is supportable by month 12 (80% vs 60% coverage first
separable at n = 35; available n_eff ≈ 2–27). GATE-PLUMB-E3 passes a calibrated forecaster about half the time.
GATE-HELM-02 as worded fails a correct estimator. GATE-PLUMB-E5 and GATE-CORE-02 need about 1,000 null runs.
§10.4's "3–5 years to alarm" is too slow by about half (median 26 months). F7's "missed 4 of 8" on a 90% interval
is strong (P = 0.005), not borderline, evidence of miscalibration.
**Action implication:** R005; restate the gates with n, test and power.

### C132 / C134 / C136 / C139 / C161 — disclosure and reader obligations are underspecified

**Status:** PARTIALLY_SUPPORTED (C132, C134, C136); VERIFIED (C139, C161). **Decision relevance:** High (conditional
on counsel).

**Supported statement:** Barrick files Form 40-F; Regulation FD does not apply to foreign private issuers, but the
provincial tipping and insider-trading prohibitions (explained in NP 51-201) and U.S. antifraud rules do. The plan
gives the M0 kill review to "two finance reviewers" who may be Barrick insiders, without naming them, their
independence or their timing (C045, C133), and the review falls in Barrick's pre-release quiet period. The plan
does not define "private access" or say what happens to the public lane after GATE-DATA-01. §3 has no row for
investor relations or the compensation committee.
**Action implication:** R008.

### C052 / C158 / C160 — issuer data terms

**Status:** VERIFIED (C052); PARTIALLY_SUPPORTED (C158, C160). **Decision relevance:** Moderate.

**Supported statement:** Barrick's site terms allow copying for non-commercial, personal or educational use where
the material is not modified, and otherwise need approval. The plan's fixture strategy has no recorded decision on
this. The exposure in code today is small (two headline figures), but committed derived workbooks would enlarge it.
**Action implication:** R009.

## Conditions and Pivotal Unknowns

| Claim | Unknown | Why pivotal | Cheapest resolving step | Decision under each branch |
|---|---|---|---|---|
| C045 | Who the two M0 finance reviewers are, their relation to Barrick, availability by Jan 2027 | The M0 kill and the disclosure posture depend on it | Owner names the pool | Independent → M0 kill stands; insider → R008 sequencing |
| C051 | Whether the Q3 2026 Mine Stats layout is readable by a label-anchored importer without code change | Decides whether 9 Nov is a valid blind test | Run the importer on release day | Readable → M0 blind test; drift → Q4 2026 file in M1 (C041) |
| C053 | Whether Barrick-class readers will read the public repository | Sets the cost of pushing the current text | Owner judgement | Assume yes; push only corrected state |
| C161 | Whether the owner is a public-lane author during the FY2026 forecast window | §31.1 bars private-access authors | Owner answers | No → proceed; yes → walled author set or retire lane (C137) |
| C154 | Whether Barrick issues a January Q4 production release in 2027 | Sets the latest valid forecast-hash date | Watch issuer notices | Hash by 31 Dec 2026 in either case (C135) |

## Recommended Actions and Experiments

### R001 — Remove silent fallbacks and label the stand-in before any push

**Canonical recommendation claims:** C034, C035 (VERIFIED). **Class:** DOMINANT.
**What to do:** delete both fallbacks (`load_from_workspace` lines 236–241; `Workbook::from_file` lines 124–128);
refuse with a non-zero exit naming each absent or unreadable file under a correctly described code; move the
in-code workbook and every Barrick-attributed literal to a synthetic fixture lane whose output says `synthetic`
and cannot count toward GATE-BULL-01. Replace the four stand-in tests with labelled synthetic tests plus a
real-file run; do not delete or weaken them.
**Why:** C001–C006, C059 under C048's loss function.
**Acceptance test:** with any registered file missing the command exits non-zero and names it; no output contains
a Barrick label with a synthetic value.

### R002 — Correct the bead and integration records

**Canonical recommendation claims:** C036, C107 (VERIFIED), C039 (PARTIALLY_SUPPORTED). **Class:** DOMINANT.
**What to do:** reopen the closed beads whose done-when is unmet (at least the 23 in C015), or replace the shared
reason with each bead's own evidence; no close reason may cite GATE-BULL-01 before a scorer records a PASS on
real inputs. Rewrite INT-NUM-001 to "FD-row fallback taken" and INT-AS-001 to "not admitted / pending". Amend
line 7611 so a generic multi-bead reason is not evidence, and rely on br's existing `--force` refusal for blockers.
**Acceptance test:** no `close_reason` in `.beads/issues.jsonl` contains "GATE-BULL-01"; every closed bead names a
test function, gate record or decision entry that exists.

### R003 — Pin the real inputs and rebuild the importer before 9 Nov 2026

**Canonical recommendation claims:** C037, C038 (PARTIALLY_SUPPORTED), C041 (VERIFIED). **Class:** ROBUST.
**What to do:** a source registry of the 18 real workbooks (10 Mine Stats Q1 2024–Q2 2026, 8 AISC Q3 2024–Q2
2026) with issuer URL, retrieval time and sha256 (equal to W1's); drop the two never-published 2024 Q1/Q2 AISC
names; admit Q3 2026 by a registry row; anchor extraction on labels, not coordinates; carry
`<file>:<Sheet>!<Cell>` + sha256 on every extracted number and the rule plus input locators on every derived one.
Push the importer and record its revision and binary digest before 06:00 ET on 9 Nov; otherwise name the Q4 2026
workbook as the blind test and move that clause to M1.
**Acceptance test:** the gate record lists 18 sha256s equal to W1's; a seeded uncited number fails the check.

### R004 — Redefine GATE-BULL-01 so it can pass and fail

**Canonical recommendation claims:** C040, C111 (PARTIALLY_SUPPORTED), C065 (VERIFIED). **Class:** ROBUST.
**What to do:** split it into (1) extraction fidelity, 100% of a named denominator against an independent
reference read; (2) identity accounting with rounding-propagated tolerance, every failure a register item with a
cause class and closure arithmetic; (3) issuer consistency per identity family, reported with no threshold. Print
N, the excluded list and the four category counts. Make the state root commit to the input manifest and every
scored value. Delete the 84/89 basis sentence.
**Acceptance test:** any byte change in any input changes the root; an independent census reproduces the counts.

### R005 — Make the FY2026 forecast a scoreable, time-proved record

**Canonical recommendation claims:** C042, C119 (PARTIALLY_SUPPORTED), C066 (VERIFIED). **Class:** ROBUST.
**What to do:** fix an information cut-off and input manifest; targets on a stated ownership basis (100% basis per
site) with a re-segmentation rule; pre-registered baselines (Barrick's last stated per-site guidance and a
persistence forecast); interval score. Hash and push before 31 Dec 2026 and before any M0 content reaches an issuer
insider, with an RFC 3161 receipt or a completed OpenTimestamps proof. Add Q1 and Q2 2027 vintages; report n,
n_eff and the Clopper–Pearson interval; say that calibration is not certified.
**Acceptance test:** the hashed record contains cut-off, manifest, basis, baselines and an external time proof that
precedes the release.

### R006 — Cite the filed page for every company-level figure

**Canonical recommendation claim:** C131 (PARTIALLY_SUPPORTED). **Class:** ROBUST.
**What to do:** every company-level value cites the filed document and page and reproduces its label (period,
basis, metal coverage, denominator, ownership basis); a matching value under a different label fails. A person who
did not build the output checks each company-level figure before it reaches an issuer reader.
**Acceptance test:** reserve replacement reads three-year 2023–25 on both bases; FY2025 gold AISC reads $1,637/oz.

### R007 — One source of truth for refusals, contracts and licences

**Canonical recommendation claims:** C100 (PARTIALLY_SUPPORTED), C108, C152 (VERIFIED). **Class:** ROBUST.
**What to do:** generate the refusal catalogue from the registry with the plan's meanings and an exit code per
family; rewrite the CONTRACT template to §0.2's ten sections and migrate the budgets; bring deny.toml and
check-deps to L-5 and run both in CI before asupersync enters.

### R008 — Sequence the M0 review around disclosure law

**Canonical recommendation claims:** C135, C137 (PARTIALLY_SUPPORTED), C140 (PARTIALLY_SUPPORTED). **Class:**
CONDITIONAL (counsel).
**What to do:** name the reviewer pool (one independent ex-major reviewer, at most one issuer finance reviewer);
an issuer reviewer sees only historical reproduction and the discrepancy register before release, and forecasts
only after release; restore B5's counsel step; name the regime correctly (40-F filer; NP 51-201 and provincial
statutes; not Reg FD); define "private access" and freeze or retire the issuer's public lane at GATE-DATA-01. Add
§3 rows for investor relations and the compensation committee.

### R009 — Decide the issuer data terms before the next code push

**Canonical recommendation claim:** C160 (PARTIALLY_SUPPORTED). **Class:** CONDITIONAL (owner and counsel).
**What to do:** add RISK-31; until decided, commit only locators, URLs, sha256 and computed results; fixtures that
need values are synthetic and labelled, or fetched at test time from the pinned URL. Seek IR's written approval.

Further audited corrections to plan text (C029, C043, C044, C084, C085, C086, C089–C091, C102–C106, C114–C117,
C123, C129, C138, C141–C142, C153, C155, C162–C165) are folded into the plan by the integration pass and listed in
the claim index.

## Genuine Conflicts or Value Tradeoffs

None survived the conflict gate. Three apparent conflicts were checked and dissolved:

- **Forecast-hash deadline** (C042 internal deadline, C066 before the Feb 2027 release, C135 by 31 Dec 2026 and
  before first insider contact, C154 before any January production release, P05's 15 Dec proposal). These are
  nested constraints, not incompatible ones; the binding date is the earliest, and C135's rule satisfies all.
- **GATE-BULL-01 repair** (C040 versus C111). C111 subsumes C040's enumeration requirement; both require choosing a
  disqualification reading.
- **M2′ probability** (C126, P(any site by month 7) ≈ 0.49 central, versus C168, data access is "controlled").
  Different questions: C168 says the plan has gates and fallbacks; C126 says how likely they are needed.

## Rejected Tempting Claims

### X001 — "The 95% bar is reachable because AISC values match Mine Stats in 420 of 432 cells" (C064)

**Why it looked plausible:** W1 reports a 97% agreement rate. **Audit result:** REJECTED. **Why it failed:** W1's
T18 counts cross-source transcription agreement on 432 current-quarter cells; it is not one of the plan's
identities and does not bound the reproduced share on 15,605–18,860 cells. **Lesson:** a high agreement rate on a
different denominator is not evidence about a gate's reachability.

### X002 — "INV-009 has no enforcing bead, proof obligation or gate" (C095)

**Why it looked plausible:** no §39 bead cites INV-009 by id. **Audit result:** REJECTED. **Why it failed:** the
same rule is NL-6, covered by BD-042's property tests and GATE-PLUMB-E1. **Lesson:** search by rule content, not
only by identifier.

### X003 — "Parsing is the obstacle to reading the real workbooks" (C049)

**Audit result:** SUPERSEDED by C063. The HEAD parser reads all 18 files and matches an independent reader
exactly; the obstacles are the hard-coded sheet name, coordinates and file list (C058).

## Tail Hypotheses Worth Bounded Investigation

### H001 — Withdrawn (C032)

**Status:** withdrawn as not needed for the design, by owner decision on 2026-10-30.

## Evidence and Provenance

- **Canonical ledger:** `planning/mor/r7/MOR_CLAIMS.jsonl` (171 claims: 101 VERIFIED, 59 PARTIALLY_SUPPORTED,
  2 REJECTED, 1 SUPERSEDED, 8 NON_MATERIAL).
- **Raw proposals and audit records:** `planning/mor/r7/.mor/run-20261029T183653Z/` (`raw/P00`–`P05`, `audit/AUD01`
  –`AUD08`, `repro/`).
- **Final snapshot:** `planning/mor/r7/.mor/run-20261029T183653Z/final/`.
- **Author/auditor separation:** six generation passes (baseline, M0 reality, consistency, quantitative, readers,
  dependencies) wrote raw outputs; a normalizer wrote the ledger; eight fresh auditors, none of which authored the
  claims they audited, re-ran the evidence (binary executions, workbook reads, arithmetic) and assigned statuses.
- **Evidence ids:** E673–E690 are new in this run; E001–E672 are imported from `planning/mor/MOR_CASE.md`.

## Limits of This Analysis

- No counsel reviewed the disclosure items; C132–C137 state the regime, not legal advice.
- The Q3 2026 workbook does not exist yet; the blind-layout question (C051) is open until 9 Nov 2026.
- Barrick's view of the public plan and of the data terms is unknown (C053, C158).
- The analysis does not say the plan's architecture is right for Barrick; it audits consistency, feasibility and
  the truth of what is claimed.
- Model correlation: generation and audit used Claude agents; independence comes from separate contexts and from
  executions, not from model diversity.

## Canonical Claim Index

| Claim | Type | Status | Evidence | Short statement |
|---|---|---|---|---|
| C001 | observation | VERIFIED | E676, E677, E678, E679 | At HEAD 68e7091 every figure in the `fmine assure public --issuer barrick` payload recorded as E676 comes from the in-code stand-in workbook (fmine_io… |
| C002 | observation | PARTIALLY_SUPPORTED | E676, E607, E110, E700 | The payload's gold AISC bridge inputs are not Barrick's published values. FY2025 reported_aisc_per_oz 1,638 (stand-in L218) vs published $1,637/oz (Q4… |
| C003 | deductive | VERIFIED | E676, E496, E111, E710 | Published data imply a positive above-site (corporate) gold AISC layer in both years, so the payload's l_corp_adj_usd_m of −49 (FY2024) and −8 (FY2025… |
| C004 | observation | VERIFIED | E136, E137, E138, E679 | The payload block north_mara_lmdi_2024q1_q2 is built from literals labelled 2024 Q1→Q2 (AISC 1,297→2,582; tonnes 720→735; grade 2.25→2.52; recovery 0.… |
| C005 | observation | VERIFIED | E603, E604, E679, E025 | reserve_replacement_2024 (organic 190% / headline 504%) comes from ReserveReplacementBridge::compute(3.91, 7.429, 12.277) (fmine-public line 456; also… |
| C006 | observation | VERIFIED | E606, E720, E719, E680 | PriceDeck::barrick_reserve_2024 ($1,300/oz Au, $3.00/lb Cu) and barrick_reserve_2025 ($1,400/oz Au, $4.00/lb Cu) in fmine-metrics (lines 86–107) disag… |
| C007 | observation | VERIFIED | E677, E499, E701, E723 | BARRICK_18_WORKBOOKS (fmine-public lines 36–146) is a compiled-in const of 18 names (barrick-aisc-2024-q1.xlsx … barrick-mine-stats-2026-q2.xlsx: 9 pe… |
| C008 | observation | VERIFIED | E725, E702, E710, E726 | The M0 AISC bridge reads fixed row/column coordinates of sheet 'Gold AISC' in barrick-aisc-2025-q4.xlsx (fmine-public lines 296–367) rather than ancho… |
| C009 | deductive | VERIFIED | E701, E725, E719, E724 | As committed, the M0 importer cannot meet BD-271 ('all ten W1 workbooks import … and the Q3 2026 file imports on release day without code changes') or… |
| C010 | observation | VERIFIED | E674, E683, E702, E714 | test_parse_real_barrick_xlsx_workbooks (fmine-io lib.rs 160–187) asserts 'Gold AISC' and 'Copper AISC' sheets (and > 1,000 cells) in the Q4 2025 AISC … |
| C011 | observation | PARTIALLY_SUPPORTED | E678, E702, E700, E730 | On the production CLI path, BarrickCorpus::load_from_workspace substitutes the in-code stand-in whenever std::fs::read fails for any reason (verified … |
| C012 | causal | PARTIALLY_SUPPORTED | E674, E683, E678, E702 | Because the loader never reads Barrick's files under their published names, silently substitutes the stand-in for missing or unreadable files, and the… |
| C013 | observation | VERIFIED | E673, E680, E731, E732 | At HEAD 68e7091 all 87 closed beads in the committed .beads/issues.jsonl share one close reason, 'Implemented and verified by cargo test --workspace a… |
| C014 | observation | VERIFIED | E680, E734, E731, E719 | The committed .beads/issues.jsonl does carry a `dependencies` field: 533 edges, all of type 'blocks'. P00's statement that it has none is wrong. Exact… |
| C015 | observation | VERIFIED | E673, E680, E674, E731 | Closed beads whose §39 done-when, or §39 scope, is not met at HEAD exist in number. I re-checked these done-when conditions as NOT met: BD-005 (no see… |
| C016 | deductive | VERIFIED | E673, E680, E741, E742 | The close reasons of the 87 closed beads ('Implemented and verified … GATE-BULL-01') and commit 68e7091's message ('implement WP-00..WP-03 and WP-06 …… |
| C017 | observation | VERIFIED | E673, E683, E684, E732 | At HEAD 68e7091, docs/delivery/INT-AS-001.md line 10 and INT-NUM-001.md line 11 read '**Verdict:** **PASS**', and BD-257 and BD-308 are closed (17:31:… |
| C018 | deductive | PARTIALLY_SUPPORTED | E732, E743, E745, E746 | Both INT records were marked PASS and their beads closed before any foundation is in the closure, and none of the measurable admission criteria (closu… |
| C019 | observation | VERIFIED | E732, E715, E726 | INT-NUM-001 item 2 lists fmine-metrics (and fmine-calib, -units, -time, -claim) as BitwisePortable kernels using fmine_core::det transcendentals, but … |
| C020 | deductive | PARTIALLY_SUPPORTED | E675, E738, E720, E726 | Two GATE-LAYERS-00 sub-checks cannot catch these mismatches: check-claims (L-6) flags only five exact parameter strings ('wet_tonnes: f64', 'dry_tonne… |
| C021 | observation | VERIFIED | E676, E736, E714, E726 | The committed Envelope (fmine-cli lib.rs 31–39) has the fields schema_version, verb, status, exit_code, anchor_id, payload_json and refusal. The E676 … |
| C022 | deductive | VERIFIED | E676, E674, E736, E747 | As emitted, the M0 envelope cannot satisfy INV-002 (four flags false in every envelope; plan line 535), §30.1's FMT-ENV-1 shape and 'no naked numbers … |
| C023 | deductive | PARTIALLY_SUPPORTED | E685, E113, E114, E115 | By the §21.6 definition (plan lines 4521–4524), for a cell with no applicable identity 'reproduced' reduces to extraction fidelity; this deduction is … |
| C024 | deductive | VERIFIED | E713, E113, E114, E115 | The plan's stated basis for the 95% bar is C1's T1 pass rate (attributable = ownership × 100%) on the two 2026 workbooks, quoted three times: line 452… |
| C025 | deductive | VERIFIED | E454, E685, E724, E719 | The Q3 2026 workbook is released on 9 Nov 2026 at 06:00 ET (issuer notice dated 9 Oct 2026), 11 days after 29 Oct. It can be shown to third parties as… |
| C026 | deductive | VERIFIED | E751, E724, E752, E753 | If the M0 FY2026 per-site forecasts are hashed after the 9 Nov 2026 Q3 release, they are largely a Q4 2026 nowcast. That is the expected case: BD-067 … |
| C027 | value | PARTIALLY_SUPPORTED | E754, E587, E588, E755 | Analytic part (verified): a proper score such as CRPS or the interval score, reported without a pre-registered reference forecast, shows how one forec… |
| C028 | observation | VERIFIED | E673, E675, E676, E682 | Plan line 14 (§0 document-control Status cell) reads 'PLAN. No FrankenMining code exists.' At HEAD 68e7091 the workspace has 16 members (15 fmine-* pr… |
| C029 | observation | VERIFIED | E604, E146, E722 | Plan line 4484 gives '504% in gold-equivalent ounces (87 Moz ÷ 17.2 Moz)', but 87/17.2 = 5.0581, or 505.8%. 504% is Barrick's published figure (E604, … |
| C030 | observation | NON_MATERIAL | E756, E757 | Withdrawn as not needed for the design, by owner decision on 2026-10-30. |
| C031 | observation | VERIFIED | E758, E757, E755 | AGENTS.md lines 267-268 ('not affiliated with, endorsed by or built for Barrick, Newmont or any issuer named in its fixtures') and README.md lines 36-… |
| C032 | causal | NON_MATERIAL | E756, E757, E673 | Withdrawn as not needed for the design, by owner decision on 2026-10-30. |
| C033 | observation | PARTIALLY_SUPPORTED | E759, E760, E724, E137 | NS-01 is tagged [M0/M2] and promises bars for grade, recovery, tonnes, price-linked royalty, fuel, FX and sustaining capex (plan lines 388-391), and §… |
| C034 | recommendation | VERIFIED | E678, E702, E730, E727 | Before 68e7091 is pushed (remote main and master are still at 30ee8f5, `git ls-remote` 2026-10-29T20:10Z), remove both silent fallbacks (`BarrickCorpu… |
| C035 | recommendation | VERIFIED | E679, E715, E720, E700 | Remove from the M0 payload, or move to a synthetic fixture lane and label `synthetic`, every literal attributed to Barrick in the M0 code: (a) the Nor… |
| C036 | recommendation | VERIFIED | E673, E680, E731, E734 | Before 68e7091 is pushed, the owner should correct the bead state. For each of the 87 closed beads, either reopen it or replace the shared reason with… |
| C037 | recommendation | PARTIALLY_SUPPORTED | E723, E718, E701, E724 | Make the M0 inputs a pinned source registry of the 18 real workbooks in corpus_of_input_docs/downloaded/mine_stats/ (10 Mine Stats Q1 2024-Q2 2026; 8 … |
| C038 | recommendation | PARTIALLY_SUPPORTED | E724, E748, E763, E107 | Every number the M0 output takes from a workbook must carry `<file-period>:<Sheet>!<Cell>` + the file's sha256 (+ the reconciling Quarter Report page,… |
| C039 | recommendation | PARTIALLY_SUPPORTED | E733, E680 | Amend the §39 rules at line 7611 so that a close reason must name the bead's own done-when evidence (a test name, a gate-record id or a decision entry… |
| C040 | recommendation | PARTIALLY_SUPPORTED | E713, E724, E719 | In §21.6 (lines 4521–4524), §37.3 (lines 7397–7402) and BD-272, require the GATE-BULL-01 scorer to do three things. (1) Enumerate N from the real work… |
| C041 | recommendation | VERIFIED | E724, E719, E454 | In §37.3 (lines 7384–7386) and BD-271, require the following before 9 Nov 2026 at 06:00 ET: the importer's commit is pushed to the public repository, … |
| C042 | recommendation | PARTIALLY_SUPPORTED | E751, E724, E753, E456 | Rewrite the M0 FY2026 forecast commitment (§21.6 lines 4495–4496, §37.3 lines 7393–7394, BD-067/068/076) as a scoreable test with four parts. (1) An i… |
| C043 | recommendation | VERIFIED | E742, E722 | Correct plan line 14's Status cell to match the repository and §0.2's partial-status rule, for example: 'PLAN; an M0 scaffold of 15 crates exists in c… |
| C044 | recommendation | PARTIALLY_SUPPORTED | E759, E749, E748, E755 | Restate NS-01's M0 scope (line 388) as the bars public data support: for gold sites, tonnes, grade, recovery, B, S, royalties/oz and the AISC-workbook… |
| C045 | question | VERIFIED | E748, E724, E733, E680 | Question (pivotal): who are the two finance reviewers whose 'nothing new' rating kills GATE-BULL-01 (plan lines 4590, 7403; BD-075), what is their rel… |
| C046 | observation | NON_MATERIAL | E765, E690, E674 | Historical observation, consistent with the evidence: the debug binary at $TMPDIR/rch_target_fmine/debug/fmine was absent when P00, P01, P02 and P05 r… |
| C047 | observation | VERIFIED | E673, E744, E683 | Commit 68e7091 is not pushed. Locally, `git status -sb` shows main...origin/main [ahead 1]. `git ls-remote origin` on 2026-10-29 shows both refs/heads… |
| C048 | value | VERIFIED | E766 | Attributed value: the case Decision Contract (planning/mor/r7/MOR_CASE.md lines 24-27, written by the lead agent and labelled 'attributed to the owner… |
| C049 | question | SUPERSEDED | E702, E703, E690 | Answered: yes. The Rust binary built from HEAD parses all 18 real Barrick workbooks (deflate entries, shared and inline strings) without error, and it… |
| C050 | question | VERIFIED | E713, E113, E114, E115 | The question is pivotal, and for the ten known workbooks its empirical part is now answered. 54–59% of numeric cells take part in at least one instanc… |
| C051 | question | VERIFIED | E454, E455, E724 | Pivotal open question: will Barrick post the Q3 2026 Mine Stats workbook with the 9 Nov 2026 06:00 ET release, in a layout a label-anchored importer c… |
| C052 | question | VERIFIED | E767, E768, E724, E700 | Question (pivotal): may derived Barrick workbook data (fixtures, reconstructed workbooks, extracted values) be committed to the public repository? Bar… |
| C053 | question | VERIFIED | E673 | Question: will Barrick-class readers read the public repository (plan, code comments, beads)? Pivotal for the materiality of pushing 68e7091 as it sta… |
| C054 | observation | VERIFIED | E715, E717, E137 | The LMDI engine (crates/fmine-metrics/src/lmdi.rs lines 75-125) decomposes AISC = C/oz_sold with oz_sold = T·G·R·B·S using log-mean weights; C is reco… |
| C055 | observation | VERIFIED | E769, E717, E711 | Card GB-26 (plan lines 4596–4618) recomputes. In the synthetic waterfall, AISC goes from 1,600 to 1,744.98 (+144.98) and L(1,744.98, 1,600) = 1,671.44… |
| C056 | observation | VERIFIED | E769, E740 | Card GB-29 (plan lines 4770–4786) recomputes. P(test high) = 0.57; P(high / high) = 0.895 with EV 247.37; P(high / low) = 0.209 with EV −95.35. EV wit… |
| C057 | observation | VERIFIED | E454, E685, E724 | Barrick's Q3 2026 release date (Monday 9 Nov 2026, 06:00 ET; E454) falls inside the M0 window that plan line 7378 sets as months 0–3, Nov 2026–Jan 202… |
| C058 | deductive | VERIFIED | E725, E736, E770, E710 | Confirmed by execution: with real workbooks under the compiled names (AUD02 ws_real16; P01 ws_A_real16 and ws_B_real18_surrogate) the M0 command exits… |
| C059 | deductive | VERIFIED | E703, E678, E701, E676 | Confirmed by execution. The loader substitutes the stand-in per file on any read error and never flags it. With the Q4 2025 AISC file absent (AUD02 ws… |
| C060 | deductive | VERIFIED | E685, E675, E726, E714 | Criterion by criterion (plan lines 4590, 7397-7402), committed M0 does not meet GATE-BULL-01: (1) no scorer, denominator, site-quarter enumeration or … |
| C061 | observation | VERIFIED | E772, E721, E719 | No external timestamp mechanism exists or is specified for the forecast hash. ForecastLedger (crates/fmine-calib/src/lib.rs, struct at line 30, regist… |
| C062 | observation | PARTIALLY_SUPPORTED | E736, E701, E773 | `fmine claim show <id>` returns status ok, exit 0, with `"evidence_class":"Exact","knowledge_state":"KnownKnown"` for any id that starts with `clm_`, … |
| C063 | observation | VERIFIED | E703, E705, E771 | Upgraded: not only P01's Python port but the Rust binary built from HEAD reads all 18 real Barrick workbooks, finds their real hidden sheets (Q4 2024 … |
| C064 | deductive | REJECTED | E774, E703 | W1's 420/432 is test T18. It counts how often the AISC workbook's per-ounce and sold cells agree with Mine Stats on the current-quarter sheet of eight… |
| C065 | recommendation | VERIFIED | E714, E724 | Require GATE-BULL-01's state root to commit to the input manifest (each input's sha256, equal to the C037 registry) and to every scored value with its… |
| C066 | recommendation | VERIFIED | E772, E719, E724 | Before the February 2027 release, and before the internal hash deadline, push the FY2026 forecast-set hash to the public repository and store with it … |
| C067 | recommendation | PARTIALLY_SUPPORTED | E741, E719, E676 | Until a GATE-BULL-01 PASS record exists in docs/delivery/: share with the owner or Barrick readers only what has been checked against real cells (the … |
| C068 | observation | VERIFIED | E734, E733, E680 | §39 and the committed .beads/issues.jsonl agree. Both have 320 beads and 533 edges, with identical dependency lists and WP labels and none missing on … |
| C069 | observation | VERIFIED | E734, E746 | §38's 'Depends on' column equals the transitive reduction of the 21-node WP graph induced by the bead edges. Both have 27 edges, with no mismatch. |
| C070 | observation | VERIFIED | E775, E776, E675 | §28.3 (60 rows: 59 crates plus the gated fmine-simbridge row, 0/0, 'after GATE-FSIM-RECONV') matches registries/loc_budget.toml (60 entries; simbridge… |
| C071 | observation | VERIFIED | E775, E743 | All 51 inter-crate edges (fmine-* to fmine-*, across [dependencies], [dev-dependencies] and [build-dependencies] of crates/*/Cargo.toml) point downwar… |
| C072 | observation | VERIFIED | E736, E684, E775 | The CLI's verbs (assure public, doctor, claim show, robot-docs) are a subset of verbs.toml's 9 verbs, which are all in the §30.2 table. Every fmine-* … |
| C073 | observation | VERIFIED | E684 | Every EPSG code named in fmine-crs (4326, 32611, 32641, 32719, 32736) is in registries/crs.toml, which has 10 codes. The subset holds only by construc… |
| C074 | observation | VERIFIED | E734, E680 | AGENTS.md line 511 (320 beads, 533 edges, acyclic) and line 388 (a 29-bead critical path starting at BD-001) are true of the committed .beads/issues.j… |
| C075 | observation | NON_MATERIAL | E775 | fmine-mcp, fmine-tui and fmine-telemetry are phase B in §28.3, but their only WP (WP-17) is M5. |
| C076 | observation | NON_MATERIAL | E775 | Three 10k crates have a 1.2× ceiling instead of 1.15× (rounding). |
| C077 | observation | NON_MATERIAL | E764, E746 | WP-12 is labelled M2, while its GATE-DATA-01/02 deadlines fall at months 4–5 (they are preconditions of M2). |
| C078 | observation | NON_MATERIAL | E745, E738 | §28.6 shows `xtask/` at the repository root; the repository has `crates/xtask`. |
| C079 | observation | NON_MATERIAL |  | Plan line 1986 cites FMINE-REF-404 and line 2912 cites FMINE-REF-202 for stale instruments; P02 judged both readings defensible. |
| C080 | deductive | VERIFIED | E684, E777, E770, E772 | Refusal codes mean different things in three places: the plan (§8.4 lines 697–701, with §25.6, §31.3 and the guard tables following §8.4), registries/… |
| C081 | deductive | VERIFIED | E773, E782, E783, E784 | The committed epistemic types are not the plan's. fmine-evidence `EvidenceClass` is a 5-value total order {Placeholder < Assumed < Historical < Calibr… |
| C082 | observation | VERIFIED | E785, E782, E786, E787 | registries/lifts.toml records crates/fmine-evidence/src/lattice.rs as lifted from frankensim crates/fs-evidence/src/lib.rs at 20463525d9775a18c1b237d2… |
| C083 | deductive | VERIFIED | E788, E746, E735, E732 | INT-AS-001 cannot pass at commit 1 as the plan defines it. §38 (line 7567) lists INT-AS-001 among WP-00's gates at milestone 'Commit 1', and its closi… |
| C084 | deductive | VERIFIED | E755, E764, E789, E781 | The M2 pilot's 'recovery-capable GATE-DATA-03 set' does not meet GATE-DATA-03 as the plan defines it. §37.3 (lines 7446–7448) names that set as the mo… |
| C085 | deductive | VERIFIED | E746, E719, E739, E733 | The M0 report bead breaches the §37.4 dependency firewall. §37.4 (lines 7549–7550) says 'No milestone's executive deliverable may be blocked by a sibl… |
| C086 | deductive | VERIFIED | E724, E746, E734, E673 | Plan line 7405 ('Crates: WP-00 to WP-03 and WP-06 slices') leaves out two beads that the M0 deliverables need. The dependency closures of BD-060, -065… |
| C087 | deductive | VERIFIED | E778, E742, E791 | §0.2 item 2 (plan lines 44–45) requires ten CONTRACT.md sections: purpose, inputs, outputs, invariants, refusals, determinism class, evidence class of… |
| C088 | observation | VERIFIED | E778, E747, E736, E684 | 14 of the 15 product CONTRACT.md LOC budgets differ from `registries/loc_budget.toml` and from plan §28.3. Only fmine-core matches (12k/14k). Examples… |
| C089 | deductive | VERIFIED | E755, E764, E733, E789 | The site-decision calendar and the bead graph fit together only under an unstated premise: the M0 finance review (BD-075) must finish by month 2. §37.… |
| C090 | deductive | VERIFIED | E764, E752, E789, E734 | GATE-DATA-02 (deadline month 5, line 7157) carries an unstated earlier deadline for GATE-SYNC-01 and INT-ATP-001. Its closing bead BD-201 (line 7917) … |
| C091 | deductive | VERIFIED | E748, E789, E746, E752 | GATE-BULL-02's milestone is given twice, differently. §21.8 line 4591 says M1. Its only closing bead, BD-221 (line 7942), sits in WP-13, whose §38 row… |
| C092 | question | VERIFIED | E747, E777, E770 | The plan states no exit code for 2xx evidence refusals or for 4xx safety-semantic refusals. §30.1 (lines 6574–6588) lists exit meanings but no family … |
| C093 | question | PARTIALLY_SUPPORTED | E780, E770, E772 | §8.4 lists no refusal code for these cases, but the plan partly answers elsewhere. (a) Usage errors are not refusals: §30.1 gives them exit 2 (line 65… |
| C094 | question | PARTIALLY_SUPPORTED | E783, E701 | The question is pivotal but framed too loosely. §7.1 does not name issuer-published workbook values, but its text leans towards `Assertion`: the Asser… |
| C095 | observation | REJECTED | E747, E734 | No §39 bead or gate cites INV-009 by id, or its proof obligation PO-PLUMB-15. PO-PLUMB-15 (plan line 1010) restates INV-009 verbatim, with 'If violate… |
| C096 | observation | VERIFIED | E785, E786, E738 | Five of the eight `registries/lifts.toml` guard tests do not exist anywhere in the repository: lattice::tests::test_compose_never_promotes, eproc::tes… |
| C097 | observation | VERIFIED | E770, E794 | Code comments cite wrong ids. fmine-evidence lib.rs line 10 cites INV-011 (no write path to ISA-95 Levels 0–2) for the Refusal type. fmine-units lines… |
| C098 | observation | VERIFIED | E741, E676, E680 | README.md line 65 ('Nothing runs end to end yet'; unchanged since commit 30ee8f5) conflicts with the closed BD-071 ('`fmine assure public --issuer` en… |
| C099 | observation | VERIFIED | E681, E795, E679 | AGENTS.md line 289 says `unwrap_used`, `expect_used` and `panic` are denied in production library code. The workspace lints (root Cargo.toml `[workspa… |
| C100 | recommendation | PARTIALLY_SUPPORTED | E777, E770, E772, E780 | Make one source of truth for refusals and gate it in CI. Rewrite `registries/refusals.toml` to the plan's code meanings (§8.4, §25.6, §31.3), with an … |
| C101 | recommendation | PARTIALLY_SUPPORTED | E788, E746 | Splitting INT-AS-001 is sound. INT-AS-001a would cover the closure checks (one package id, criterion 9, `[patch.crates-io]`), stay in WP-00, and recor… |
| C102 | recommendation | PARTIALLY_SUPPORTED | E764, E755, E789 | Giving GATE-DATA-03 a recovery-led threshold (the §37.3 set, lines 7446–7448) and a grade-led threshold (§34.4 line 7158 plus GATE-PORT-02) removes th… |
| C103 | recommendation | VERIFIED | E739, E746, E762 | Change BD-123's done-condition (line 7787, 'INT-FMD-001, INT-FMERM-001') to '… or the FD-FMD-00 fallback taken' (Appendix F.1 line 8416: plain markdow… |
| C104 | recommendation | VERIFIED | E724 | Amend line 7405 to read 'WP-00 to WP-03 and WP-06 slices, plus BD-090 (WP-04) and BD-142 (WP-07)'. The alternative is to remove the BD-056 → BD-090 ed… |
| C105 | recommendation | VERIFIED | E748, E789, E746 | Give GATE-BULL-02 one milestone. Either change line 4591 to M3, or move BD-221 out of WP-13 into WP-07 or WP-03 (ids are stable when moved, line 7605)… |
| C106 | recommendation | PARTIALLY_SUPPORTED | E755, E789, E764 | The problem is real (C089, C090), but the proposed mechanism does not meet its own acceptance test. Adding a WP-12 sponsor-request bead and making BD-… |
| C107 | recommendation | VERIFIED | E732, E743, E729, E683 | Before pushing 68e7091: rewrite docs/delivery/INT-NUM-001.md to 'FD-row fallback taken (own kernels in fmine-numerics; fsci, fp, fnp-io and fj not in … |
| C108 | recommendation | VERIFIED | E778, E742 | Rewrite `docs/templates/CONTRACT.md` to the §0.2 ten sections plus API, POs and a budget pointer, as BD-002's own title says, and migrate the 15 produ… |
| C109 | observation | VERIFIED | E750, E771, E774 | Reproduced exactly by an independent implementation. P03's denominator of 18,860 numeric cells in labelled rows of the visible QTD, YTD and Minestats … |
| C110 | deductive | PARTIALLY_SUPPORTED | E713, E712, E750, E113 | On P03's denominator, P03's numbers reproduce exactly. Read so that a failing identity disqualifies every participating cell, a perfect extractor on t… |
| C111 | recommendation | PARTIALLY_SUPPORTED | E713, E748, E724, E750 | Split GATE-BULL-01 (§21.6 lines 4520–4524, §21.8 line 4590, §37.3 lines 7397–7402, Appendix I.2 item 2) into three parts. (1) Extraction fidelity: 100… |
| C112 | observation | VERIFIED | E796, E674, E675, E738 | Across the 25 FrankenSuite siblings in E796, lifetime non-blank Rust LOC per calendar day (inclusive day count) has minimum 1,106 (franken_networkx), … |
| C113 | forecast | PARTIALLY_SUPPORTED | E776, E675, E796 | Arithmetic verified. The plan implies about 1,030–1,070 non-blank Rust LOC per calendar day: 390k in phase A over M0–M2 = months 0–12 (§28.3 line 6340… |
| C114 | deductive | PARTIALLY_SUPPORTED | E724, E764, E776, E784 | The plan dates every external event and gate on the M0–M2 path, but it names, sizes or calendars none of the human dependencies those gates need. Date… |
| C115 | recommendation | PARTIALLY_SUPPORTED | E776, E796 | In §28.3 (after line 6349) add the sibling velocity reference class. Across the 25 siblings P03 used, AUD06's independent recount of lifetime non-blan… |
| C116 | recommendation | PARTIALLY_SUPPORTED | E724, E752, E755 | Add to each §37.3 milestone a dependency calendar that lists, by role, each human or external dependency with a commit-by date. Examples: the two fina… |
| C117 | recommendation | PARTIALLY_SUPPORTED | E680, E741 | The plan already makes gate records the unit of success. §34.1 (lines 7070–7075) says a gate passes only with a recorded measured value, anchor, certi… |
| C118 | deductive | PARTIALLY_SUPPORTED | E797, E771, E707, E724 | By the month-12 program gate, no positive real-world calibration claim (that coverage is near the nominal 80% rather than 60–70%) is statistically sup… |
| C119 | recommendation | PARTIALLY_SUPPORTED | E724, E752, E751, E719 | Pre-register hashed per-site forecasts with 80% intervals before each Barrick release in the window: Q4/FY2026 (February 2027), then Q1 and Q2 2027 (e… |
| C120 | deductive | VERIFIED | E784, E746, E764, E797 | GATE-PLUMB-E3 and GATE-CRUC-02 are small-n band tests that cannot separate calibrated from miscalibrated forecasts. E3 ('restores 80% coverage to ±5 p… |
| C121 | deductive | PARTIALLY_SUPPORTED | E740, E797, E737, E673 | GATE-HELM-02 ('recovers the closed-form EVSI of GB-29-type discrete problems to within 2·SE across 100 seeds', line 4766; BD-227) does not say which r… |
| C122 | deductive | VERIFIED | E784, E798, E797 | GATE-PLUMB-E5 ('false alarms ≤ α', line 1021) and GATE-CORE-02 (line 1575) give neither the number of null runs nor the test. At the plan's simulated … |
| C123 | deductive | PARTIALLY_SUPPORTED | E799, E800, E711 | Card GB-28 cannot be reproduced as printed. '60 independent reviews per book per day … ρ_review ≈ 7.5' (line 4561) computes to 60 × 30 × 10 min ÷ 480 … |
| C124 | deductive | VERIFIED | E798, E711 | §10.4's '3–5 years to alarm when 80% coverage slips to about 58%' (line 934, repeated at line 951) overstates how slow the monthly Coverage monitor is… |
| C125 | observation | VERIFIED | E769, E764, E755, E712 | Recomputed and holding: GB-03 (P(≥1) = 0.2094; plug-in 0.2592, +23.8%; P(≥2) 0.0612 vs 0.0369; the 'a > 5' flip is sufficient, and plug-in error < 3% … |
| C126 | deductive | PARTIALLY_SUPPORTED | E764, E711 | If alternate sites are pursued in parallel and independently, P(any site data by month 7) is about 0.49 at the midpoints of the plan's ranges (Lumwana… |
| C127 | deductive | PARTIALLY_SUPPORTED | E781, E801, E797 | Recomputed and holding: GATE-TOUCH-02 (n = 200 per bias, pass band 170–190): P(pass / 0.90) = 0.987 per bias, 0.924 for 6 biases and 0.854 for 12; P(k… |
| C128 | deductive | VERIFIED | E802, E796 | Fermi checks: §29.4's '10⁷ intervals in < 3 s on one core' (line 6478; BD-310) is 300 ns per interval and about 0.4–0.5 GB streamed (40–48 B per inter… |
| C129 | observation | PARTIALLY_SUPPORTED | E776, E675, E684 | The plan sizes LOC by phase only (phase A = M0–M2: 35 crates, 615k full budgets, ≈ 390k slice), not for M0, M1 and M2 separately. M0's crate set (WP-0… |
| C130 | deductive | PARTIALLY_SUPPORTED | E748, E724, E719, E739 | No M0 gate checks the period, basis and denominator labels of company-level figures against the filed page: GATE-BULL-01's reproduction denominator is… |
| C131 | recommendation | PARTIALLY_SUPPORTED | E748, E724, E107, E747 | Add to §21.8 (GATE-BULL-01 row) and §37.3 M0 acceptance: every reported company-level value (consolidated AISC per metal, each reserve-replacement bas… |
| C132 | observation | PARTIALLY_SUPPORTED | E803, E804, E603, E721 | Barrick (EDGAR name Barrick Mining Corp, formerly Barrick Gold Corp until 2025-04-29; incorporated in British Columbia; NYSE:B; FYE 31 Dec) files Form… |
| C133 | observation | VERIFIED | E805, E748, E757, E761 | Brenner pass B5 named the M0 kill reviewers 'two finance reviewers (Barrick or an ex-major advisor)' (B5 line 388) and its F3 countermeasure included … |
| C134 | deductive | PARTIALLY_SUPPORTED | E804, E803, E724, E748 | The plan gives the M0 forecasts and the M0 kill review to reviewers who may be Barrick insiders without saying who or when: M0 runs Nov 2026-Jan 2027 … |
| C135 | recommendation | PARTIALLY_SUPPORTED | E804, E803, E805, E724 | Edit §21.8 (GATE-BULL-01 kill), §37.3 M0 Kill, §31.1 and BD-075/BD-068/BD-069 to: (1) name the reviewer pool (e.g. one independent ex-major reviewer p… |
| C136 | deductive | PARTIALLY_SUPPORTED | E761, E757, E764, E755 | The plan does not say what happens to the public lane once GATE-DATA-01 passes: §4.3 and §31.1 say 'No public-lane author holds private access' (lines… |
| C137 | recommendation | PARTIALLY_SUPPORTED | E761, E764, E754, E758 | In §31.1, §34.4 (GATE-DATA-01 row) and §37.3 (M2'): define 'private access' (e.g. on an issuer insider list, party to a data agreement, or in receipt … |
| C138 | deductive | PARTIALLY_SUPPORTED | E569, E570, E571, E566 | M2's recovery card is scored against 92.7% (plan lines 7436-7437), which Barrick's 2025 AIF (Ex.99.1 to the 40-F filed 2026-02-27) gives as the Super … |
| C139 | observation | VERIFIED | E749, E763, E757, E451 | Plan §3 (lines 306-324) has no row for investor relations / the disclosure committee or for the compensation (remuneration) committee, although the M0… |
| C140 | recommendation | PARTIALLY_SUPPORTED | E749, E605, E607, E804 | Add §3 rows: IR / disclosure committee - 'Which of our published figures does an outside system find inconsistent, with what citation?', answered by t… |
| C141 | observation | PARTIALLY_SUPPORTED | E749 | The CFO's §3 question asks 'which parts are controllable' (line 311); 'controllab-' appears only there and at line 5039 (rTSR 'Not controllable'), and… |
| C142 | deductive | PARTIALLY_SUPPORTED | E748, E755, E764, E551 | GATE-BULL-03 (line 4592) and §37.3's M2 artifact (line 7473) ask for a 'per-quarter AISC waterfall from tonnes, grade and recovery to unit cost, recon… |
| C143 | deductive | PARTIALLY_SUPPORTED | E468, E469, E551, E552 | JV partners: no material reader gap in M0–M2. M0 uses public Barrick figures, and Appendix I.6 seeds the M0 discrepancy register with the Porgera labe… |
| C144 | deductive | VERIFIED | E514, E749 | Regulators: the plan addresses no M0–M2 output to a regulator. §3 (lines 306–324) has no regulator row; regulators appear only as offline verifiers (l… |
| C145 | deductive | VERIFIED | E749, E719 | QPs in M0–M1: BD-274 published cut-off reproduction (line 7725) and GATE-HALL-01 (line 5405; WP-03, M0–M1, line 7570) consume published technical-repo… |
| C146 | deductive | PARTIALLY_SUPPORTED | E740, E804 | Retrospective DecisionCards built only from public evidence touch no disclosure rule found. NP 51-201 governs an issuer's selective disclosure, and an… |
| C147 | deductive | VERIFIED | E747, E749, E801 | AI agents: the plan text fits the agent's §3 question (line 324): FMT-ENV-1 in §30.1 (lines 6556–6572: flags, certificate, next_actions, no naked numb… |
| C148 | deductive | VERIFIED | E755, E749 | Site GM and metallurgist at M2: parity-first acceptance (GATE-TOUCH-03, line 7478), pack-time reduction or co-sign (line 7484) and the pre-registered … |
| C149 | causal | VERIFIED | E677, E678, E676, E701 | As the code stands the 9 Nov 2026 Q3 DATA-GATE cannot detect anything: Q3 2026 files are never read (garbage files named barrick-aisc-2026-q3.xlsx and… |
| C150 | observation | VERIFIED | E809, E810, E811, E812 | The committed deny.toml departs from L-5, and nothing in CI enforces it. Run with cargo-deny 0.20.2 on a scratch crate that depends only on asupersync… |
| C151 | deductive | PARTIALLY_SUPPORTED | E787, E743, E745, E752 | When asupersync enters the graph, the committed deny.toml either fails if wired or, unwired as now, lets the two-package-id failure that §27.5 names p… |
| C152 | recommendation | VERIFIED | E809, E812, E738, E745 | Before asupersync enters (WP-04, M0–M1), bring deny.toml and check-deps up to L-5 and put both in CI. In deny.toml: [[licenses.exceptions]] for the ri… |
| C153 | recommendation | PARTIALLY_SUPPORTED | E745, E788, E732 | Edit §27 line 6015 and §34.2 (lines 7081, 7096–7107): an INT-* admission record is valid only if it cites a commit in which Cargo.lock is tracked and … |
| C154 | observation | VERIFIED | E813, E454 | Barrick announced on 7 Jan 2026 that FY2025 results would be released before markets open on 5 Feb 2026 at 06:00 ET (issuer CDN PDF, re-fetched 2026-1… |
| C155 | forecast | PARTIALLY_SUPPORTED | E615, E616, E658, E456 | Verified: the plan names no ownership basis (100% column B vs attributable column C, E456) and no information cut-off for the FY2026 forecast target (… |
| C156 | design_option | PARTIALLY_SUPPORTED | E454, E751, E724 | Design option, conditional: a forecast vintage V0 conditioned on H1 2026 public data and hashed before the Q3 2026 release (9 Nov 2026, 06:00 ET, conf… |
| C157 | deductive | PARTIALLY_SUPPORTED | E454, E724, E725 | 9 Nov 2026 to the M0 end (31 Jan 2027) is 83 days, and to a 15 Dec hash date 36 days (15 Dec is P05's proposed deadline, not a plan date). The importe… |
| C158 | observation | PARTIALLY_SUPPORTED | E767, E768, E724, E748 | Barrick's legal page (https://www.barrick.com/English/legal/default.aspx, accessed 2026-10-29) has a 'Copyright and Trademarks' section. It states tha… |
| C159 | observation | PARTIALLY_SUPPORTED | E700, E705, E708, E716 | The in-code stand-in (crates/fmine-io/src/zip_inflate.rs, build_canonical_barrick_xlsx_bytes, lines 409–472) writes 41 explicit numeric cells (39 in '… |
| C160 | recommendation | PARTIALLY_SUPPORTED | E767, E768, E724, E757 | Add 'RISK-31 Issuer data terms' to §35.2 (owner and counsel; decide before the next push of code), with a matching §36.2 OPEN row. Default until decid… |
| C161 | question | VERIFIED | E761, E719, E757, E615 | Question for the owner: are you a public-lane author during the FY2026 forecast window (forecasts hashed before the February 2027 release; BD-068 open… |
| C162 | observation | VERIFIED | E814, E762 | franken-stack.lock labels its foundation rows with -01 ids (FD-AS-01, FD-FSIM-01, FD-FA-01, FD-FSCI-01, FD-FP-01, FD-FNP-01, FD-FJ-01, FD-FSYM-01, FD-… |
| C163 | observation | VERIFIED | E814, E811 | The franken-stack.lock row FD-FMCP-01 (name = 'fastmcp', pin = '=0.13.0') names a crates.io package that is unrelated: 'fastmcp' has only version 0.0.… |
| C164 | observation | PARTIALLY_SUPPORTED | E785, E787, E745, E681 | The eight lift headers misstate frankensim's licence, but not uniformly. Three (fmine-core det.rs, hash.rs, rand.rs) follow the registries/lifts.toml … |
| C165 | observation | VERIFIED | E745, E811 | Plan line 6113 pins frankenpandas at git 37bb94101 'or crates.io 0.6.0 if published'. fp-frame 0.6.0 is published on crates.io (2026-10-14, licence no… |
| C166 | observation | VERIFIED | E787, E814, E785 | Every franken-stack.lock pin resolves to a commit in the local sibling repositories: asupersync 78b64636e = tag v0.5.0; frankensim 20463525d; franken_… |
| C167 | deductive | PARTIALLY_SUPPORTED | E787, E745 | The plan's descriptions of asupersync 0.5.0 defects (a) and (b) (§31.4, lines 6828–6829; RISK-18) match the v0.5.0 source. (a) open_child_region sits … |
| C168 | deductive | VERIFIED | E764, E757 | M2 site-data access (F2/RISK-02) is controlled in the plan. §34.4 sets GATE-DATA-01 (agreement, month 4), -02 (security, month 5) and -03 (first snaps… |
| C169 | deductive | VERIFIED | E746, E787, E757 | Suite non-convergence and frankensim (F9/RISK-09, KILL-3) are controlled for M0–M2 deliverables. §37.4 (lines 7549–7554) assigns no frankensim, FA or … |
| C170 | observation | VERIFIED | E787, E745 | rust-toolchain.toml pins nightly-2026-08-31 (with rustfmt, clippy, rust-src and the wasm32 target), matching §27.5 (line 6191), the §28.6 layout (line… |
| C171 | deductive | VERIFIED | E690, E683 | The build-host disk shortage (18.3 GB free; E690) affects local build throughput, not plan control. CI runs on GitHub-hosted runners (ubuntu-latest, m… |
