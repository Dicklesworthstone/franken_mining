# WIZARD_SCORES_CC_ON_COD: Claude's scores for Codex's fifteen ideas

| Field | Value |
|---|---|
| Scorer | Claude (Opus 5.5), dueling idea wizards, scoring round |
| Date | 2026-10-29 |
| File scored | `planning/duel/WIZARD_IDEAS_COD.md` (read in full: the 15 detailed ideas, the screening table for the other 15, the title appendix) |
| Basis | The plan at `68e7091`; current crate code (spot-checked where an idea cites it); the evidence register; the Bottom Line of `planning/mor/r7/MODES_OF_REASONING_REPORT_AND_ANALYSIS_OF_PROJECT.md` |
| Scope | The 15 ideas Codex ranked and detailed (#1–#15). The 15 table-only candidates get a short note at the end and are not scored |

## How I scored

**Scale.**

| Score | Meaning |
|---|---|
| 800–1000 | Build now. Correct, testable in the public lane before M2, and clearly worth its cost |
| 650–799 | Strong. Build in M0–M1, or fold into a planned bead as its acceptance criterion |
| 500–649 | Worthwhile, but gated on site data, reviewed cards or a sponsor, or largely an extension of planned work |
| 300–499 | Marginal: mostly duplicates planned machinery, cannot be tested before M2, or its gain does not justify its cost |
| < 300 | Reject |

**The M0 condition.**
- FACT (r7 Bottom Line; C001, C011, C059): every number `fmine assure public --issuer barrick` emits today comes from
  an in-code stand-in workbook or from literals. Several are presented as Barrick's and are wrong (C002–C006, C159).
- No idea gets credit for a first slice that assumes the importer reads the real workbooks.
- To Codex's credit, its file states the same defects with correct line references:
  - the fallback at `crates/fmine-public/src/lib.rs:224` and `crates/fmine-io/src/xlsx.rs:122`;
  - the literal North Mara state at `crates/fmine-public/src/lib.rs:437`;
  - the incomplete Claim identity at `crates/fmine-claim/src/lib.rs:312`.

  It also orders honest source loading first. I checked each of those references.

**Conflict of interest.** Five of these ideas overlap ideas in my own list:
- COD-02 is nearly my idea 2;
- COD-09 overlaps my 12;
- COD-07 overlaps my 6;
- COD-10 overlaps my 4;
- COD-06 overlaps my 5.

I scored them on their own text. Where my version differs, I say how, so the reader can discount my view.

## Overall assessment

- **The file is disciplined.**
  - Every load-bearing statement is labelled.
  - Each idea has a kill condition.
  - The claims are deliberately conservative: inclusion-minimal sets, not minimum cardinality; "indeterminate"
    instead of a guess; scenario search never sold as proof of impossibility.
  - It respects INV-002, INV-011 and FA as the book of record throughout. I found no boundary violation.
- **Its main weakness is timing and visibility.**
  - Most ideas refine machinery the plan already funds: §10 scoring, §20.6 identifiability, §22 flip margins and
    VOI, and the §21.6 definition flags.
  - Nine of the fifteen can only be tested on synthetic data before M2 (COD-03, 05, 06, 07, 14, 16, 17, 26, 28).
  - Only COD-02 and COD-08 give a strong public-lane result in M0–M1.
  - Few of these are things an executive would ask for by name.
- **Overlaps within the file.**
  - COD-26 reuses COD-03's origin machinery (Codex says so).
  - COD-11 (table only) and COD-10 both lean on COD-04.
  - COD-07 and COD-09 both compare metric bases.
  - COD-14 and COD-06 are both answerability and identifiability checks.
  - COD-16's useful core (resolution of intervention-affected forecasts) belongs inside COD-08.
- **Its ranking.** Ranking COD-01 first overweights an idea whose public-lane output is mostly subtraction. COD-08 is
  ranked too low for how cheaply it closes a real defect.

---

## The fifteen, in Codex's order

### COD-01: guidance completion test ("what must happen to meet guidance") — **610**

- **Strongest point.** It reframes the guidance review as an inverse problem: "these two requirements cannot both
  hold" is a sentence a CFO can act on. The details are right: annual AISC as a ratio of summed numerators, and
  scenario search never presented as proof of physical impossibility.
- **Biggest weakness.** In the public lane, the only defensible output is the necessary arithmetic: remaining
  production and the required H2 rate.
  - The evidence register already contains that result for North Mara: H2 must deliver 1.9–2.4× the H1 rate
    (INFERENCE, E587).
  - Every finance team's guidance worksheet does the same subtraction.
  - The bounds that would make it more than subtraction are maximum tonnes, grade and recovery. Those are site data
    or Pushback/Crucible capability envelopes, which do not exist before M2.
  - Its actuals also depend on the importer that r7 found does not read the workbooks (C059).
- **What would make it better.** Pair the requirement with a calibrated probability of meeting it, from the issuer's
  own history of H2/H1 ratios. "You need 1.9× H1; sites needing ≥ 1.5× met guidance k of n times" is new
  information. Subtraction is not.
- **Duplication.** Not in the plan; NS-04 sets ranges but does not test completion. It overlaps my idea 1
  (guidance-at-risk nowcast) and my idea 9 (revision rule). Rank #1 is too high.

### COD-02: rounding-aware contradiction finder — **830**

- **Strongest point.** It is exact, sound and falsifiable at M0 on pinned public facts (E113–E115, E144, E147). It
  improves both sides of the product: true findings get stronger, and false allegations are suppressed when a common
  rounded explanation exists. The shared-total example shows why per-identity checks miss joint contradictions.
  Its claims are carefully bounded: inclusion-minimal sets, and "indeterminate" on unknown conventions.
- **Biggest weakness.** It keeps the registered ±1-last-digit rule unchanged beside an exact half-unit diagnostic, so
  one cell can get two verdicts. The file does not say which governs when they disagree. The linear-only first
  slice also leaves out tonnes × grade and AISC × ounces, where sound refutation via McCormick envelopes is cheap.
- **What would make it better.**
  - Make the exact check the definition BD-272 uses for "reproduced", "explained" and "refused".
  - Add bilinear refutation early.
  - Reproduce the E113 Carlin case (one corrected value satisfies both identities) as the first acceptance fixture.
- **Duplication.** It extends BD-065/066, BD-272, BD-275 and §21.6. It is nearly identical to my idea 2, so discount
  my score accordingly. It needs the honest importer first, as Codex says.

### COD-03: independent-evidence accounting — **620**

- **Strongest point.** It names a real laundering route with a crisp analytic test. A reprinted observation must not
  narrow an interval. Many assays that share one batch bias reduce σ_ε²/n but never σ_b².
- **Biggest weakness.** The plan already models shared error where it matters:
  - inter-laboratory bias as a hierarchical term (§12);
  - rig-specific bias;
  - per-node bias in the R2 reconciliation (§20.4–§20.5, BD-152);
  - INV-006's rule that evidence never upgrades through derivation.

  A workspace-wide origin-identity layer risks duplicating those domain models. Physical identity mapping needs site
  records (M2+). In the public lane, the only case is the same figure reprinted across Mine Stats, MD&A and release,
  which parity checks never treat as independent anyway.
- **What would make it better.** Ship "a reprint never narrows" now, as a no-laundering property test (AGENTS.md
  testing policy). Defer the shared-error schema to Corebox's first site import.
- **Duplication.** COD-26 depends on it. It is not in Appendix C or D.

### COD-04: decision recall after evidence changes — **580**

- **Strongest point.** It turns staleness (INV-013; `ClaimDag::invalidate_and_propagate`) into a decision-level work
  list. It separates loss of admissibility from numeric change, and it never carries a sign-off forward. It also
  rightly identifies the incomplete `compute_claim_id` as a prerequisite (`crates/fmine-claim/src/lib.rs:312`).
- **Biggest weakness.** It needs reviewed DecisionCards, which do not exist before M1.
  - For the small recipes it targets, full recomputation is cheap.
  - The conservative change-bound optimization therefore adds risk for little gain, since it must handle
    validity-domain exits and singularities.
  - "Does this change the ranking?" is already answered for linear sensitivities by flip margins (§22; `FlipMargin`
    in `crates/fmine-decide/src/lib.rs:11`).
- **What would make it better.** Drop the bounds. Ship the recall list, an exact rerun, and a before/after flip-margin
  comparison.
- **Duplication.** It overlaps my idea 14 (provenance-exact delta pack) and Codex's own COD-11 (QP re-review). It is
  partly planned through INV-013 and §22.

### COD-05: metal-to-available-cash bridge — **475**

- **Strongest point.** It catches a real category error: attributable production or a receivable is not cash the
  parent can spend. The example is right: the year-end balance looks fine while the floor is breached mid-period. It
  matters for JV cash that leaves only as distributions (Kibali, §23.2) and under host-state restrictions (RISK-26).
- **Biggest weakness.** Nothing in it can be tested on public data. Contract terms and treasury data are confidential
  and arrive late, if ever, and treasury systems already produce dated cash forecasts. Most of the ingredients are
  planned:
  - custody and outturn (§17.6);
  - settlement and provisional pricing (§21.5, part 8);
  - distributions and DSCR locks (BD-314);
  - the minimum-cash floor (§23).

  The idea sits uncomfortably close to FA's role as the book of record.
- **What would make it better.** Reduce it to an "entity accessibility" attribute on BD-314's entity graph, with one
  synthetic known-answer test.
- **Duplication.** Mostly planned.

### COD-06: answerability before data sharing — **530**

- **Strongest point.** It aims at the binding constraint, P(site data by month 7) ≈ 0.3 (E599), with a reviewable,
  smaller data offer that reads no site values. Its distinction between reproducing a published recovery and
  attributing a sampler bias is exactly right.
- **Biggest weakness.** Structural answerability is necessary but far from sufficient.
  - Whether a smaller offer can attribute the recovery gap is a power and identifiability question (§20.6, BD-153,
    BD-159). Enumerating recipes does not answer it.
  - Sponsors' objections are more often about security and law (pre-mortem F2) than about field count.
  - Two hand-authored recipes is a thin catalogue.
- **What would make it better.** Rank each field by its marginal detection power on a twin calibrated to the site's
  public series, so every requested file earns its place. That is my idea 5.
- **Duplication.** It overlaps GATE-DATA-03, §34.4, §37.3 and my idea 5. COD-14 is the in-operation analogue.

### COD-08: forecast survival scoreboard — **700**

- **Strongest point.** It closes a real defect in today's code.
  - `FrozenForecast` holds one `realized_outcome: Option<f64>`, and `evaluate_model` silently skips unresolved
    entries (`crates/fmine-calib/src/lib.rs:16–125`).
  - It adds outcome vintages, which matter because Barrick restates (E474).
  - Whole-cohort coverage bounds are exact and cheap (7/8 observed, so 7/10 to 9/10 for the whole cohort).
  - It protects the flagship freeze-and-score promise and the FY2026 forecasts M0 must hash.
- **Biggest weakness.** It is partly planned. PO-HEAD-5 and PO-HELM-3 already require every forecast to resolve or
  carry a declared non-resolution. So this is a refinement of BD-047–BD-049, not something a user sees. Executives
  will not ask for it, but without it they would be misled.
- **What would make it better.** Make it the acceptance criterion for BD-068 and BD-076, and for any scoring of
  issuer guidance. Absorb COD-16's intervention-affected resolution status.
- **Duplication.** None in my list as a separate idea; it is a prerequisite for my idea 1. Codex ranks it too low.

### COD-28: bounds on what untested ore could do to the decision — **590**

- **Strongest point.** Manski-style bounds on aggregate recovery from untested ore answer the question a coverage
  badge cannot: does the gap matter for this decision? The care points are right:
  - weights are contained metal, not tonnes;
  - additivity is a validity condition;
  - selection is never assumed random.

  It bears directly on decisions like Lumwana's transitional-ore recovery gap (E569) during a $2bn expansion.
- **Biggest weakness.** There is no public-lane test beyond synthetic data, and real bounds need site testwork (M2+).
  The physical 0–100% bound is vacuous, so usefulness rests on elicited bounds with weak evidence class. BD-300's
  `OutsideValidity` coverage report may already drive the testwork decision.
- **What would make it better.** Couple the bound with the VOI of the separating test (§22.4), so the output is a
  priced testwork recommendation.
- **Duplication.** It extends BD-300 and BD-289; it is not elsewhere in the plan or my list.

### COD-07: JV disagreement localization — **455**

- **Strongest point.** It reduces a JV disagreement to a named difference in basis, version or input, without new
  data sharing. It is careful not to claim zero-knowledge guarantees or legal causation.
- **Biggest weakness.**
  - It needs both partners to agree on an export format, and that agreement is the hard part.
  - "First comparable divergence in a deterministic order" is order-dependent, as Codex admits.
  - It has no public oracle and is synthetic-only until partners engage.
  - After Newmont's 2026 default notice and the settlement (E557, E558; F13), partners may not want a tool that
    pre-digests disputes.
- **What would make it better.** Start from the public version: reconcile the same JV asset as reported in both
  partners' public filings (NGM 61.5/38.5, E457; Pueblo Viejo 60/40; Kibali 45/45/10, E560). That is testable now.
  Then offer the private protocol on top.
- **Duplication.** It overlaps §31.2 bundles, my idea 6, and Codex's own COD-09.

### COD-10: controlled IR sentences bound to Claims — **520**

- **Strongest point.** It carries basis, period and calibration qualifiers into the sentence readers remember. It
  gives agents a deterministic writing task with typed slots. It is right to keep words like "safe" and "on track"
  out of the grammar.
- **Biggest weakness.** Three sentence forms cover a small part of a release. IR and legal drafting happens in office
  documents under counsel's review, so adoption is doubtful. A template does nothing for the free-text paragraphs,
  where most errors live. Checking any text against the Claim store covers more ground for less code.
- **What would make it better.** Invert it into a checker for arbitrary drafts, with templates only as the fix-it
  suggestion.
- **Duplication.** It overlaps §30.5, BD-072, BD-123, G-BULL-6 and my idea 4 (Disclosure Lint). It leans on COD-04.

### COD-26: decision fragility to removing an evidence family — **445**

- **Strongest point.** A leave-one-source-family-out check can expose a recommendation that looks stable under small
  sensitivities only because every input came from one campaign. Codex is careful not to call withdrawal a
  probability of error.
- **Biggest weakness.** It is a jackknife over provenance families. It needs COD-03's family definitions plus
  refittable models, so nothing useful exists before M2. Plan §22's flip margins and robustness views, and §9.4's
  evidence ranking, cover much of it. Reviewers may still read it as suspicion of a team or laboratory.
- **What would make it better.** Fold it into the §22 robustness view as one more stress case.
- **Duplication.** It duplicates COD-03's machinery (acknowledged).

### COD-14: measurement-outage observability — **440**

- **Strongest point.** Linear observability (q must annihilate the null space of the stacked observation and
  conservation system) is the right mathematics. The "estimable sum, not the split" example is exactly right.
- **Biggest weakness.**
  - Observability and redundancy classification is standard in metallurgical data reconciliation. Established
    metallurgical-accounting practice and tools already do it.
  - In the plan, it belongs inside Touchstone's reconciliation and §20.6 identifiability (BD-153, FMINE-REF-104, the
    `not_observable` state).
  - Only synthetic tests are possible before M2.
- **What would make it better.** Add it to BD-153 as an "outage mode" acceptance case, not as a separate feature.
- **Duplication.** Largely planned. It overlaps COD-06.

### COD-17: VOI against the last useful date — **465**

- **Strongest point.** Information that arrives after the commitment is worth nothing to that commitment. The
  exhaustive event tree is easy to verify. Today's `VoiCampaign` indeed has no timing fields
  (`crates/fmine-decide/src/lib.rs:49`).
- **Biggest weakness.**
  - The novelty is modest. This is standard decision analysis, and §22.4, GB-29 and BD-227 already include cost and
    delay.
  - Deadlines are often negotiable.
  - Turnaround distributions are site data.
- **What would make it better.** Add arrival-time and deadline fields to `VoiCampaign`, with the exhaustive-tree test.
  That is a change to one bead, not a feature. Separately, `VoiCampaign` carries money as a bare `f64`
  (`cost_usd_m`), an L-6 concern worth fixing at the same time.
- **Duplication.** Mostly planned.

### COD-09: comparability frontier — **500**

- **Strongest point.** "The strongest comparison the bases permit" is the honest form of peer comparison. Its caveat
  is correct: adding credits does not turn by-product AISC into co-product AISC.
- **Biggest weakness.** It is largely an extension of planned work (BD-060–BD-063, BD-273). With no source for
  calibrating bridge sizes, most cross-issuer results will be wide or refused, and the common-component view may not
  answer the question executives actually ask.
- **What would make it better.** Calibrate the bridges on same-asset JV pairs, where two issuers report one physical
  asset, and test them on a held-out pair before applying them to different assets. Add rank intervals so "lowest
  quartile" claims can be checked.
- **Duplication.** It overlaps my idea 12 and Codex's own COD-07.

### COD-16: intervention benefit contracts — **480**

- **Strongest point.** It names the prevention paradox correctly. A maintenance prediction followed by preventive
  action is neither a false alarm nor a proven saving. That is a real flaw in naive vendor scoring, and it matters
  now that an AI vendor is deploying at Barrick sites (E590, E591).
- **Biggest weakness.** The plan already covers most of it:
  - vendor predictions are registered and resolved (§16.5, PO-HEAD-5);
  - vendor benefit claims need prior forecasts (G-CRUC-5);
  - preregistered difference-in-differences exists (§19.6).

  The causal-benefit product needs site experiments, adoption records and a cooperative sponsor. None of that can be
  tested in the public lane.
- **What would make it better.** Keep only the "intervention-affected" resolution status, inside COD-08.
- **Duplication.** Mostly planned. Its useful core belongs in COD-08.

---

## The 15 table-only candidates (not scored)

- **COD-30 (KPI gameability tests)** deserved a top-15 slot. §15.6, §23.2, §23.6 and RISK-22 name AISC gaming by
  deferral, high-grading and reclassification, but give the remuneration committee no instrument. This is close to
  my idea 7.
- **COD-29 (fixed-cohort bridges across perimeter changes)** is useful after the Hemlo and Tongon sales (E552–E554).
- **COD-22 (auditor-chosen spot checks after the producer commits)** is a cheap, credible audit protocol.
- **COD-11** is a COD-04 consumer, and **COD-15** folds into §10 calibration.

---

## Ranked table (by my score)

| My rank | Idea | Title | Codex's rank | Score | Main reason |
|---|---|---|---|---|---|
| 1 | COD-02 | Rounding-aware contradiction finder | 2 | 830 | Exact and falsifiable at M0 on E113–E115; strengthens true findings and suppresses false ones (≈ my idea 2) |
| 2 | COD-08 | Forecast survival scoreboard | 7 | 700 | Closes a real defect in `fmine-calib`; cheap; protects every track-record claim |
| 3 | COD-03 | Independent-evidence accounting | 3 | 620 | Real laundering guard; generic layer partly duplicates domain bias models |
| 4 | COD-01 | Guidance completion test | 1 | 610 | Good framing; public output is mostly subtraction (E587); envelopes are M2+ |
| 5 | COD-28 | Bounds from untested ore | 8 | 590 | Right mathematics for a real metallurgical decision; site-gated |
| 6 | COD-04 | Decision recall | 4 | 580 | Useful work list; bounds add risk; needs reviewed cards |
| 7 | COD-06 | Answerability before data sharing | 6 | 530 | Targets E599, but structural answerability is not power |
| 8 | COD-10 | Controlled IR sentences | 10 | 520 | Narrow template coverage; checking direction is stronger |
| 9 | COD-09 | Comparability frontier | 14 | 500 | Honest, but largely planned and uncalibrated |
| 10 | COD-16 | Intervention benefit contracts | 15 | 480 | Prevention paradox is real; mostly planned; site-gated |
| 11 | COD-05 | Metal-to-available-cash bridge | 5 | 475 | Real category error; untestable in public; mostly BD-314 |
| 12 | COD-17 | VOI against the last useful date | 13 | 465 | Correct but a field on `VoiCampaign`, not a feature |
| 13 | COD-07 | JV disagreement localization | 9 | 455 | Needs partner-agreed exports; public mirror version is stronger |
| 14 | COD-26 | Evidence-family removal fragility | 11 | 445 | Duplicates COD-03 machinery and §22 robustness |
| 15 | COD-14 | Measurement-outage observability | 12 | 440 | Standard reconciliation practice; inside BD-153 |
