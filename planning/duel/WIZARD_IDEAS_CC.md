# WIZARD_IDEAS_CC: Claude's fifteen best ideas for FrankenMining

| Field | Value |
|---|---|
| Author | Claude (Opus 5.5), dueling idea wizards, round 1 |
| Date | 2026-10-29 |
| Read before writing | AGENTS.md; README.md; plan §0–§5, §9–§10, §20–§24, §30, §33–§37, §39, Appendices B, C, D, H; every file in `crates/` and `registries/` |
| Status | Every capability below is DESIGN or HYPOTHESIS. No code was written. Facts cite E-ids or `file:line`. Example outputs marked *illustrative* are not findings. |
| Layout | §0–§7: the top five, best first. §8–§19: ideas 6–15, best first. Appendix: every idea considered, with its disposition. |

---

## 0. Starting point: what the five must build on

- **FACT.** `fmine assure public --issuer barrick` has not yet read a real workbook.
  - The 18 filenames it requests (`crates/fmine-public/src/lib.rs:36`) do not exist. The public files on disk are
    `corpus_of_input_docs/downloaded/mine_stats/Barrick_Q{n}_{yyyy}_{Mine_Stats|AISC}.xlsx`.
  - On a missing file the loader substitutes a workbook built in code (`crates/fmine-io/src/xlsx.rs:123`,
    `crates/fmine-public/src/lib.rs:236`, `crates/fmine-io/src/zip_inflate.rs:408`).
- **FACT.** The gold AISC bridge does not reconcile anything:
  - it hard-codes the JV sales layer (`cons_sales - 17.0`, `crates/fmine-public/src/lib.rs:343`);
  - it takes the corporate layer as the residual (`:344`);
  - it reads missing cells as zero (`number_or_zero`, `:369`).

  No Mine Stats value is compared with its published figure, no forecast is frozen, and GATE-BULL-01 has no
  scorer (BD-272 is open).
- **FACT.** Useful seams already exist:
  - the xlsx parser keeps formula text (`ExtractedCell.formula`, `crates/fmine-io/src/xlsx.rs:54`);
  - `ExactQty` provides exact i128 rationals (`crates/fmine-units/src/lib.rs:275`);
  - `ForecastLedger`, CRPS and Winkler scores (`crates/fmine-calib/src/lib.rs`);
  - `BettingEProcess` (`crates/fmine-numerics/src/eproc.rs:10`);
  - `FlipMargin` and `DecisionCard` (`crates/fmine-decide/src/lib.rs`);
  - `extract_html_tables` and `write_stored_zip` (`crates/fmine-io`).
- **DESIGN consequence.** Every first slice below runs on the real public files. Each one starts with a fail-closed
  source manifest: a missing file is FMINE-REF-502, never a substitute (appendix idea 29). Ideas 1 and 2 also
  replace the plugged bridge with engines that can explain a gap only by named layers, or prove that no such
  explanation exists (G-BULL-3).

## 1. The five at a glance

| # | Idea | Subsystems | Primary users | First slice | Gate claim type |
|---|---|---|---|---|---|
| 1 | **Public Track Record.** Score the guidance, revisions, technical-report schedules and capex estimates the issuer has already published as pre-registered forecasts | Plumb, Portal, Bullion, Helm | CFO, IR, board, COO, site GMs | Barrick FY2023–FY2026 per-site guidance vs the Q4 workbooks (M0) | Reconciliation for the scores; parity for the extraction |
| 2 | **Proof-carrying discrepancies.** A consistency solver for published tables that is exact about rounding and emits Farkas certificates | Portal, Bullion, Hallmark, Touchstone | Controller, IR, auditor, QP, regulator, JV partner | Mine Stats identities on the ten real workbooks (M0) | Parity |
| 3 | **Spreadsheet Lifter.** Turn workbook formula graphs into typed, evidence-classed Claim DAGs | Portal, Plumb, Bullion, Touchstone, Helm | Site finance, planners, QPs, JV and M&A teams, board | Barrick's public AISC and Mine Stats workbooks, plus seeded synthetic workbooks (M1) | Known answers; parity against Excel's cached values |
| 4 | **Disclosure Lint.** Check every number in human- or agent-written prose against the Claim store | Adit, Plumb, Bullion, Portal | Disclosure committee, IR, board secretary, AI agents | Cited mode plus six metric keys over Barrick's public text (M1) | Known answers; parity against a set labelled by two readers |
| 5 | **Pilot Protocol Card.** A pre-registered pilot design and minimal data ask, built on a twin calibrated to public data | Testpit, Touchstone, Crucible, Helm, Plumb | Executive sponsor, site GM, metallurgist, QP, IT security, counsel | Plant-only Lumwana twin: four seeded causes × five data subsets (M1) | Known answers; parity of the twin against the published series |

---

## 2. Idea 1: the Public Track Record

### What the user sees

`fmine track-record --issuer barrick`, plus a guidance-setting section on the CFO's DecisionCard. It shows:
- for each site × metric (production, TCC or C1, AISC, capex) and year: the published guidance range, its
  publication date and price assumptions, the outcome, and whether the outcome landed inside the range;
- across site-years:
  - how often ranges contained the outcome, and which way they missed;
  - how much of each miss came from the metal price (royalties, step royalties, super-profit taxes) and how much
    from things management controls;
  - which sites show a persistent bias beyond the common year shock;
  - what range would have achieved a stated coverage;
- each quarter: the probability that each full-year outcome lands inside guidance, given year-to-date actuals.

*Illustrative output on synthetic numbers, not a finding about any issuer:* "FY2023–25 AISC: 17 of 39
site-years inside guidance, 29 above the midpoint. The ranges behave like about 45% intervals. To be an 80% range,
centre them 4% higher and widen them 2.1×. Price explains about a third of the bias."

### What is new against the plan

The plan already contains four related pieces:
- FrankenMining freezes and scores its *own* FY2026 forecasts (BD-067, BD-068, BD-076);
- Loulo's technical-report assumptions are registered as frozen forecasts (BD-276; one site);
- calibrated guidance ranges arrive at M3/M5, on private data (NS-04);
- W4 §6.4 suggests single-site "guidance-assurance cards" for North Mara and Lumwana, which the plan did not adopt.

New here:
1. **The issuer's own forward-looking statements become a forecast family.** The record time is the publication
   date of a filed or posted document, which nobody can backdate, FrankenMining included. INV-008 therefore holds
   for forecasts made years before FrankenMining existed. This answers the small-n problem of §10.3 with data that
   were pre-registered when they were published.
2. **Implied coverage, bias and dispersion of guidance ranges**, each with a confidence interval.
3. **A hierarchical split** of every miss into a common year shock and a persistent site effect.
4. **Price-conditional scoring**, which separates price-driven misses from controllable ones.
5. **A guidance-at-risk nowcast** each quarter, itself registered and scored.
6. **A race** of FrankenMining's own forecasts against guidance and naive baselines, on the same outcomes.

Together these move NS-04 from M3/M5 to M0–M1, and give BET-02 ("scored beats persuasive") its first test at M1
instead of M3.

### How it works

- **Record.** `GuidanceRecord{issuer, site, metric_key, basis, period, range | point, assumptions{Au, Cu, Ag, FX,
  oil}, vintage: Initial | Revised{n}, locator{doc, page, table, sha256}, record_time}`.
  - It is stored as a `Claim` whose content is `Assertion{source: issuer}` and whose forecast author is
    `issuer:barrick/guidance`.
  - The basis is explicit (attributable vs 100%, per ounce sold, by- vs co-product), so a 100%-basis outcome can
    never be scored against attributable guidance.
  - FACT: per-site guidance is published in the AIF and the MD&A; for 2026, AIF p.31 (E586, E587).
- **Outcome.** Full-year per-site actuals come from the full-year blocks of the Q4 workbooks: FY2023 comparatives,
  FY2024 and FY2025 (E451).
  - The first-published vintage is the default resolution source.
  - Restatements (E474) are kept, and the segment names which vintage was used.
- **Scores.** For a range [L, U], let m = (L + U)/2, h = (U − L)/2 and z = (y − m)/h. Then:
  - hit rate, with Clopper–Pearson intervals;
  - implied coverage ĉ = P̂(|z| ≤ 1);
  - bias μ̂ = mean z, plus the dispersion of z;
  - the interval score at the implied level;
  - z_{s,y} = μ + a_y + b_s + ε, where a_y is the common year shock (inflation, price) and b_s the persistent site
    effect, partially pooled. When a_y dominates, the effective n reported is the number of years, not of
    site-years.
- **Recalibration.** Find the smallest (shift, scale) of [L, U] that reaches 80% leave-one-year-out coverage, by
  split conformal with year blocks. The result is emitted as the `Correction{family, bias, κ}` claim of §10.3,
  computed from public data.
- **Price-conditional score.** Guidance is conditional on its stated price deck.
  - Mine Stats publish royalties per ounce by site (FACT, §21.6). For ad valorem royalties, the price-driven part
    of an AISC miss is r·(1 − P_assumed/P_realized).
  - Step schedules come from the fiscal-term rows (BD-228, BD-296): Mali's TAV (E422–E424) and Kibali's
    super-profits tax (E242).
  - **HYPOTHESIS, per site:** royalty per ounce is proportional to price. Test: a regression over the ten
    quarters. Where it fails, nothing is adjusted and the miss stays unsplit.
  - Both scores are always shown side by side: unconditional (what investors experienced) and conditional (what
    management controls).
- **Guidance-at-risk.** After each quarter, compute P(FY ∈ [L, U] | YTD) from the distribution of the remaining
  quarters: empirical H2/H1 ratios per site and metric, plus the run rate. Each value is registered as a forecast
  before the next release.
  - Live case: North Mara delivered 68 koz in H1 2026 against FY guidance of 200–230 koz, so H2 needs 1.9–2.4× the
    H1 rate (E587).
  - This lives in the public lane only. A guidance-at-risk built on private data would be MNPI and sits behind
    the wall (§31.1).
- **Race.** FrankenMining's frozen forecasts run against two rivals on bounded, normalized interval scores, with a
  pairwise betting e-process (`BettingEProcess`). The output is an e-value; "beats guidance" is never claimed
  without one. The rivals:
  - guidance, centred at its implied level;
  - naive baselines: the prior-year actual and the trailing run rate.
- **Phase 2: technical-report schedules and capex.**
  - Life-of-mine (LOM) schedules in technical reports (production, cost and recovery by year) are scored the same
    way. Sources: Loulo (E249) and the Kibali, Pueblo Viejo, Carlin and Cortez reports already in the corpus.
  - Project capex and schedule histories are scored too (E631–E633, plus Barrick's own approvals). They become the
    company random effect that the §23.5 reference class asks for, from public data.
- **Phase 3: reserve-grade realization.** The reserve-statement grade is compared with the grade processed in
  later years, with caveats for stockpiles and blending. This is the first public evidence toward OPEN-PLUMB-7.

### First shippable slice (M0, before the February 2027 release)

1. Transcribe Barrick's FY2023–FY2026 per-site guidance (production, TCC or C1, AISC; gold and copper sites) from
   the AIFs and MD&As, with page citations.
   - Two independent readers do the transcription (the AUDIT_4 second-reader rule).
   - The result is stored as a cited registry.
2. Load FY2023–FY2025 outcomes from the Q4 workbooks, under the versioned region map (E461) and unit rules (E463).
3. Ship `fmine track-record --issuer barrick --json`. It reports:
   - hit rates, implied coverage and bias;
   - year and site effects;
   - unconditional and price-conditional scores;
   - `NotCalibratable` wherever n is too small.
4. Register the FY2026 guidance with its publication date. It then resolves on the same release day as
   FrankenMining's own FY2026 forecasts (BD-068), and the two are scored side by side.

TARGET size: 2.5–4k LOC across `fmine-public`, `fmine-calib` and `fmine-cli`, inside their budgets.

### Tests

- **Parity (extraction).** Every guidance and outcome value reproduces its cited page or cell. Fixtures carry
  capture records and are never regenerated from the candidate (§33.2).
- **Known answers (estimators).**
  - A generator draws ranges with known coverage c, bias μ, and year and site effects.
  - Over ≥ 200 replicates, the 90% intervals for c and μ must cover the truth 85–95% of the time (the
    GATE-TOUCH-02 standard).
- **Properties.**
  - Scores are invariant under exact unit changes.
  - The price adjustment is the identity when P_realized = P_assumed.
  - Recalibrated ranges reach nominal coverage on exchangeable synthetic data.
- **No hindsight.**
  - A vintage is admissible only if its record time precedes the outcome's availability time.
  - The forecast horizon (days into the period at publication) is part of the segment.
  - G-TIME-2 applies.

### What could kill it

| Risk | Response |
|---|---|
| Data gate in week 1: fewer than three resolved years × ten sites retrievable with citations | Fall back to single-site guidance cards (the W4 form) until history accumulates |
| Guidance turns out to be well calibrated | The output is then a certified clean bill for IR, and the engine stays as the scorer for FrankenMining's own forecasts |
| The scoring is misread as a judgement on the teams that set the ranges | Deliver privately to the sponsor first. Show the controllable score beside the unconditional one. FrankenMining publishes nothing about the issuer |
| Common shocks dominate (effective n ≈ number of years) | Refuse site rankings with a typed refusal in the REF-104 pattern, and report only the year-shock finding |

### Why I am confident

- It is useful with no site data at all (GB-38), and it attacks the M0 kill criterion head-on. Finance reviewers
  already know their own numbers, so reproducing them risks a "nothing new" rating. A scored track record of their
  guidance, split into price-driven and controllable misses, is new. That claim is a HYPOTHESIS, tested by the
  two-reviewer assessment (BD-075).
- The data are public and were already pre-registered by their publication dates.
- Most of the scoring code exists: `ForecastLedger`, Winkler and CRPS, `BettingEProcess`, `Claim`.
- It is falsifiable in public on a fixed date: the FY2026 release in February 2027.
- It is what the wedge is named for ("Guidance & Reserve Assurance"). Its premise, that executives care about
  guidance credibility, is GB-38's own stated assumption.

---

## 3. Idea 2: proof-carrying discrepancies

### What the user sees

`fmine check tables <workbook | document>` returns one of these outcomes for a set of published tables:
- **Consistent** under the stated rounding.
- **Conflict, with proof.** The output is:
  - a short arithmetic certificate an auditor can check by hand, e.g. "rows 5 + 7 + 9 − total = 3.5, but rounding
    allows at most 1.5";
  - the minimal set of cells that must be wrong;
  - each culprit's implied value interval.
- **Ambiguous.** Two or more equally small explanations exist. The solver refuses to pick one and names the
  document that would decide, for example the filed Quarter Report page (E107).

It also returns:
- **un-rounded intervals** for every cell, tighter than the printed precision;
- **hidden parameters:** which rounding convention, ownership share, price deck or unit makes the table
  consistent.

### What is new against the plan

Today's plan checks identities one at a time, against a heuristic tolerance:
- §21.6 defines "reproduced" as each applicable identity holding to ±1 unit of the last published digit;
- BD-066 and BD-275 file discrepancies;
- BD-279 recomputes reconciliation identities;
- GATE-HALL-01 checks roll-forward closure.

New here:
1. exact half-unit rounding semantics instead of ±1 digit;
2. joint solving across chained identities, so one bad cell is not reported as three failures;
3. machine-checkable Farkas certificates, verified in exact rationals;
4. minimum correction sets, with implied values;
5. typed blame ambiguity;
6. un-rounding;
7. discovery of hidden parameters by feasibility;
8. a pre-publication workflow for the issuer itself.

It also gives BD-272, the GATE-BULL-01 scorer, precise semantics:
- *reproduced* = feasible;
- *explained* = a certified conflict with a bridge;
- *refused* = ambiguous.

### How it works

- **Cells are intervals.** A cell printed as v, with its last digit at 10^−d, has a true value
  x ∈ [v − ½·10^−d, v + ½·10^−d].
  - Displayed precision comes from the xlsx number format (`xl/styles.xml`, which is not parsed today), or from
    the printed digits of a PDF or HTML table.
  - A stored full-precision value is recorded separately. What readers saw and what the file says are both
    evidence.
- **Identities are registry data.**
  - Σ sites = region = total, under the versioned region map (E461).
  - Q1 + Q2 = H1.
  - Attributable = share × 100%.
  - Metal = tonnes × grade × k.
  - Cost = AISC × ounces sold.
  - Opening − depletion + additions = closing.
  - The same quantity agrees across the Mine Stats workbook, the AISC workbook and the Quarter Report.
- **The rounding convention is a hypothesis.** Two conventions are tested per table, and the result is reported:
  - sum-then-round: the identity holds on the true values;
  - round-then-sum: the printed total is the rounded sum of the printed components.
- **Solving.**
  - Linear identities are handled exactly.
  - Bilinear identities (tonnes × grade, AISC × ounces) use McCormick envelopes over the interval box. An
    infeasible relaxation proves the original infeasible, so refutation is sound. A feasible relaxation is
    reported as "not refuted", never as "proved consistent".
- **Certificate.** Multipliers y ≥ 0 with yᵀA = 0 and yᵀb < 0.
  - They are checked exactly with `ExactQty`, even when a floating-point LP found them.
  - On i128 overflow the verdict is `Indeterminate`, never a claim.
- **Blame.**
  - Irreducible infeasible subsets come from a deletion filter.
  - The minimum correction set comes from a hitting set over those subsets: exact up to about 20 candidates;
    beyond that, greedy and labelled approximate.
  - Each culprit's implied value is its feasible range once its own constraint is freed.
- **Un-rounding.** The min and max of each x over the feasible set. These give exact error bars for the LMDI
  waterfall (BD-064), and answer "is this quarter's change bigger than rounding?".
- **Hidden-parameter discovery.** The table is solved under each registered alternative (ownership share, price
  deck, unit, rounding convention), and the feasible alternatives are reported.
- **Placement.**
  - The kernel sits in K: `fmine-numerics`, std-only, over `fmine-units::ExactQty`.
  - Table binding sits in V: `fmine-public`.
  - The verb sits in U: `fmine-cli`.
  - Connected components have tens of variables, so compute cost is negligible.

### Public known answers already in the evidence register

| Evidence | What it is | What the solver should report |
|---|---|---|
| E113 | A Carlin 100%-basis cell breaks two identities; one corrected value satisfies both | A size-1 minimum correction set, with that value as its implied value |
| E114, E115 | Half-year and ownership identity breaks (Turquoise Ridge, Jabal Sayid) | Certified conflicts |
| E116 | A divested site's cell equals another site's value | A conflict located on that cell |
| E144 | The sign of the 2025 copper roll-forward net change | A roll-forward closure conflict |
| E147 | The GEO net change closes only at a deck other than the one its row states | Hidden-parameter discovery: the deck that closes it |
| E463 | Copper rows labelled in pounds hold kilotonnes | Unit discovery |
| E153, E193 | Carlin's attributable column | Ownership-identity conflicts |
| E206 | Kibali's MCF row recomputes away from its printed ratios | A ratio-identity conflict |
| E209, E241 | Loulo and Kibali tables that disagree within one report | Cross-table conflicts |

### Workflows

- **Controller and IR pre-flight.** Run on draft workbooks before release; the issuer itself gains most. The
  items a pre-flight would flag are already on record:
  - the Q2 2026 workbook contains 1,092 `#REF!` cells (E466);
  - a value was restated without a footnote (E474);
  - two AISC workbooks carry hidden external-link sheets (AUDIT_4; Appendix H.5b).
- **Auditor and audit committee.** Proofs, not opinions.
- **QP and regulator.** Technical-report tables.
- **JV partner.** The operator's attributable statements.
- **Touchstone.** The verbatim reconciliation-table importer (BD-279) emits certified conflicts.

### First shippable slice (M0)

1. Check sum, half-year and ownership identities on the ten real Mine Stats workbooks.
2. Reproduce E113–E115 as certified conflicts with the correct minimum correction sets.
3. Report the share of identities that are consistent under exact rounding, beside the ±1-digit result.
4. Extend to the AISC workbooks, then to the reserve roll-forward (E144, E147).

TARGET size: 2–3k LOC.

### Tests

- **Known answers: soundness.**
  - A generator draws exact values on a random identity graph and rounds them at random display precisions.
  - Property: zero certified conflicts over 10⁴ tables. Any certified conflict on clean data is a bug.
- **Known answers: power.**
  - Inject transpositions, copied cells, sign flips, wrong ownership shares and stale vintages.
  - Report detection power against error size ÷ rounding slack.
  - Report blame accuracy: the share of injected cells that fall inside the unique minimum correction set.
- **Oracle.** LP fixtures from HiGHS, run in the lab lane only (GATE-ORACLE-01).
- **Parity.** Each register item in the table above is a pinned fixture with its cell locators.
- **Metamorphic.** These transformations must leave the verdicts unchanged:
  - permuting rows;
  - exact unit scaling;
  - splitting one site row into two with the same totals.
- **Determinism.** Certificate digests are stable (INV-010).

### What could kill it

| Risk | Response |
|---|---|
| Identity graphs too sparse to localise blame | Ship detection with proofs only |
| Display precision cannot be read | Require declared precision in the import dialect (BD-186); never guess |
| Certified conflicts never exceed what the ±1-digit rule already finds on the ten vintages | It is then a better scorer, not a product. Keep it only as BD-272's implementation |

### Why I am confident

- The mathematics is elementary and exact.
- The outputs are proofs, which suits the auditor's question in §3 ("Is every disclosed number reproducible?") and
  the refuse-never-plug rule.
- About ten public known answers are already in the register, so the first slice cannot be faked with a hand-built
  workbook.
- It runs at M0 on files already on disk.

---

## 4. Idea 3: the Spreadsheet Lifter

### What the user sees

`fmine lift xlsx <workbook>` takes a budget, LOM, cut-off, reconciliation-pack or JV-statement workbook and
returns:
- **a typed dependency graph;**
- **an assumption register:** every literal input, with its label, inferred unit and basis, location, and the
  number of outputs that depend on it;
- **findings:**
  - dimension and basis mismatches;
  - overrides inside formula rows;
  - embedded constants;
  - `#REF!` chains, and what they reach;
  - dependencies on hidden or external-link sheets;
  - circular references;
  - range totals that skip rows;
  - stale cached values;
- **headline numbers as Claims**, each carrying the evidence class of its weakest input (NL-2);
- **the top sensitivities**, as inputs to DecisionCard flip margins.

*Illustrative output:* "AISC rests on 14 literal inputs, 9 of them unlabeled. Row 41 applies a $/st rate to metric
tonnes. Column M overrides the formula pattern in 3 rows. The NPV is `Assumed`-grade."

### What is new against the plan

What the plan has today:
- Portal reads exports (§32; BD-187 CSV/Excel readers);
- xlsx import ignores formula cells (§21.6);
- G-UNIT-4 and BD-287 parse-check `CorrelationCard` formulas.

Nothing lifts spreadsheet logic. New here:
- formula-graph lifting;
- dimensional analysis through formulas;
- detection of overrides and pattern breaks;
- the automatic assumption register;
- evidence propagation;
- sensitivity extraction.

Two strategic links:
- **It inverts pre-mortem F5,** "a senior geologist's spreadsheet stayed the recon of record". FrankenMining
  ingests that spreadsheet as a typed candidate instead of competing with it.
- **It resolves OPEN-TOUCH-1** ("the pilot site's F-factor definitions and pack conventions"). Lifting the site's
  own reconciliation pack gives R0 parity (GATE-TOUCH-03) by construction, because the pack's formulas *are* its
  conventions. They are read from the pack itself rather than from documentation.

### How it works

- **Parser.** A formula tokenizer and parser covering:
  - A1 and R1C1 references, ranges and sheet references;
  - defined names and operators;
  - about 60 common functions: SUM, SUMPRODUCT, IF, ROUND, MIN, MAX, INDEX/MATCH, VLOOKUP, IFERROR, and so on.

  Array formulas, structured references, volatile INDIRECT/OFFSET and macros are refused cell by cell, with a
  coverage percentage. Refuse, never guess.
- **Dimension inference.** A label grammar over `registries/units.toml`, which nothing reads today, plus basis
  tokens: "100%", "attributable", "wet", "dry", "per oz sold". Any other label gives `unit unknown`, never a
  default.
- **Dimension propagation.**
  - Addition and subtraction need equal dimension and basis.
  - Multiplication and division compose dimensions.
  - SUM needs a homogeneous range.
  - Each mismatch is reported with its cell path. The $/t vs $/st trap is a corpus finding (E182–E184).
- **Pattern breaks.** Formula signatures are normalised to R1C1 along each run of cells. A literal, or a different
  signature, inside a run is an override finding.
- **Evaluation.** An exact evaluator for the supported subset, over `ExactQty` where possible, re-computes the
  values.
  - Excel's own cached values are an independent oracle: Excel produced them, not the candidate.
  - A mismatch flags a stale cache or unsupported semantics.
  - The same evaluator gives finite-difference sensitivities.
- **Evidence.**
  - Literals become `Assumed`, or `Assertion` pending review if the cell cites a source.
  - Formula cells are `Derived`, through `compose`.
  - The lifter never computes an authoritative balance (G-BULL-1).

### First shippable slice (M1)

1. Lift Barrick's public AISC and Mine Stats workbooks:
   - the dependency graph;
   - the reach of the `#REF!` cells (1,092 in Q2 2026, E466);
   - the hidden external-link sheets AUDIT_4 found;
   - dimensions inferred from labels.
2. Build a synthetic workbook generator, on the existing `write_stored_zip`, that seeds each fault class at known
   cells.

TARGET size: 3–4k LOC: the parser in `fmine-io`, a K-layer dimension checker in `fmine-units`, and the verb.

### Tests

- **Known answers.**
  - Recall and precision per fault class on the seeded synthetic workbooks.
  - Property: zero dimension findings on generated models that carry no faults.
- **Oracle parity.** For the supported subset, the lifter's re-evaluation equals Excel's cached values on the public
  workbooks.
- **Metamorphic.**
  - parse → print → parse is the identity.
  - Inserting a row, with references adjusted, leaves the findings unchanged.
- **Public fixtures.** The external-link and `#REF!` counts recorded by AUDIT_4 and W1 are reproduced.

### Boundaries

- Private workbooks never enter this repository. The lifter runs on-premises.
- It only reads, and the INV-002 flags stay false.
- FA remains the book of record.

### What could kill it

| Risk | Response |
|---|---|
| Formula coverage below about 90% of cells on real models | Ship the structural audit only (graph, links, `#REF!`, patterns), without evaluation |
| Label-based unit precision below about 95% on a labelled set | Report units only where the label is unambiguous |
| Site packs turn out to be macro-driven | Refuse, with a coverage report |

### Why I am confident

- The spreadsheet is where mining numbers are actually made.
- The seam already exists: formula text is parsed today and then discarded.
- The work is deterministic, and it has a free external oracle in Excel's cached values.
- It answers a named pre-mortem failure (F5) and an OPEN item on the M2 critical path (OPEN-TOUCH-1).
- Its first slice runs on public files.

---

## 5. Idea 4: Disclosure Lint

### What the user sees

`fmine lint <draft>`, also exposed as an MCP tool. It takes a board paper, an MD&A draft, an IR script, an analyst
note or an agent's output, and gives every number in it one verdict:

| Verdict | Meaning |
|---|---|
| `verified` | Matches a Claim; the claim id and citation are attached |
| `value_mismatch` | The value differs from the Claim |
| `basis_mismatch` | Sold vs produced; attributable vs 100%; by- vs co-product; scorecard vs non-GAAP; gold ounces vs GEO |
| `period_mismatch` | The period differs |
| `rounding_beyond_tolerance` | Differs by more than its rounding allows |
| `stale` | The Claim has been superseded (INV-013) |
| `uncalibrated_probability` | A probability or interval with no calibration flag |
| `uncited` | No Claim found |
| `ambiguous` | More than one Claim fits. Lint never guesses |

Lint also applies to any text the wording rules the plan imposes on its own renders:
- ranges, not points;
- the evidence class beside each number;
- "evidence state", never "safe" (G-FREE-1);
- no class labels from software (G-HALL-1);
- no compliance claim.

### What is new against the plan

The plan guards only FrankenMining's own output:
- G-BULL-6 refuses FrankenMining's own metric renders without `rules_version` and basis;
- §30.5 sets language rules for FrankenMining's reports;
- GATE-ROBOT-01 tests agents that use robot-docs.

Nothing checks text written outside FrankenMining. Lint extends those guards to any document. It also gives "the
evidence substrate agents cite" (§32.3; the countermeasure to pre-mortem F1) a verb that third-party AI layers can
call.

### How it works

- **Cited mode: strict and deterministic.**
  - Numbers carry inline claim references, e.g. `[$1,637/oz](fmine:clm_…)`.
  - Lint checks the value at its rendered rounding, the basis, the period and staleness, and flags every number
    without a reference.
  - This is the mode for FrankenMining's reports and for agents. The agent proposes the mapping from text span to
    Claim; the substrate verifies it.
- **Discovery mode: human prose.**
  - A deterministic matcher over:
    - the metric-definition registry, with synonyms per key (BD-060);
    - the site and entity registry;
    - the fiscal calendar;
    - units.
  - It returns ranked candidates, or `ambiguous` when more than one Claim fits.
  - No LLM sits in the verdict path (INV-010).
- **Output.** NDJSON verdicts with `fix` hints, e.g. "write 'non-GAAP AISC' or cite clm_…".

### Public demonstration

Lint Barrick's MD&A and results-release text against its Mine Stats and AISC workbooks. The register already holds
real basis collisions:
- 2025 gold AISC: $1,423/oz on the scorecard vs $1,637/oz non-GAAP (E605, E607);
- 2023–25 reserve replacement: about 190% in gold ounces vs 504% in GEO (E603, E604);
- attributable vs 100% basis at NGM (61.5% Barrick).

### Workflows

- The disclosure committee, before release.
- The board secretary, assembling a pack.
- IR, preparing a call script.
- The CFO's office, reviewing analyst notes.
- Agents, checking their own drafts before a human sees them. Lint is the gate between "agents prepare" and
  "humans authorize" (§30.4).

### First shippable slice (M1)

- Cited mode.
- Discovery mode for six metric keys over Barrick's public text: AISC, TCC or C1, production, grade, recovery and
  reserve replacement.

TARGET size: 2–3k LOC. A number-and-unit tokenizer goes in `fmine-io`, the matcher in `fmine-public` and the verb in
`fmine-cli`. The MCP projection comes later, with `fmine-mcp`.

### Tests

- **Known answers.**
  - A generator renders documents from a claim set and injects mutations:
    - a wrong digit;
    - rounding beyond tolerance;
    - a swapped basis;
    - a wrong period;
    - a wrong ownership basis;
    - a stale claim;
    - an uncited number;
    - a probability without a calibration flag.
  - Report precision and recall per mutation class.
  - Property: never `verified` unless a Claim's rounding interval contains the number, with matching basis and
    period.
- **Parity.** Public MD&A paragraphs are labelled by two readers, with capture records, and never regenerated from
  the candidate. Discovery-mode precision and recall are reported against them.
- **Metamorphic.** Formatting variants ($1.6k/oz, US$1,637, 1,637) and sentence reordering leave the verdicts
  unchanged.

### What could kill it

| Risk | Response |
|---|---|
| Discovery-mode precision below about 90% on the labelled set | Ship cited mode only. It still covers every agent and FrankenMining output |
| Human authors will not adopt the citation syntax | Cited mode serves agents only |

### Why I am confident

- The failure it targets is named in the plan's own diagnosis (§1.1: "reasoning over inputs that carry no
  reconciliation state or version").
- The basis collisions are real and already in the register.
- Cited mode is small and fully deterministic.
- It is the cheapest way to make every number an agent writes traceable to a Claim.

---

## 6. Idea 5: the Pilot Protocol Card

### What the user sees

`fmine pilot design --site lumwana` produces one card before any data are requested. It holds:
- the finding mechanism;
- the minimal data list, with a reason for each file and that file's marginal power;
- the analysis plan, hashed and timestamped;
- the plan for a null result;
- a go/no-go for the site, based on power.

*Illustrative output:* "The monthly metallurgical balance, head and tail sampler records, and fresh/transitional
tags reach 82% power to attribute a 2-point grade-normalised recovery gap. Settlement assays add 6 points. Historian
data add less than 2 points and are not requested."

### What is new against the plan

What the plan already has:
- §20.6 states detection power per measurement node;
- BD-159 states power for a 5% sampler bias;
- GATE-TOUCH-04 pre-registers the materiality threshold;
- GATE-DATA-03 lists the data;
- §37.3 sets the site decision rule.

New here:
1. the generator is calibrated to one named site's public disclosures;
2. curves of power and identifiability by data subset minimise the data ask and justify each file;
3. the whole analysis plan is hashed before the data arrive, not only the forecasts and the threshold;
4. power drives an early kill.

It aims at the plan's binding constraint, P(site data by month 7) ≈ 0.3 (E599). It also aims at BET-06's
falsifier: "M0–M1 artifacts do not move the data-access decision". And it hands IT and counsel a per-file
justification, which is what security reviews ask for (pre-mortem F2).

### How it works

- **Twin.** A plant-level synthetic twin reproduces Lumwana's public quarterly series: throughput, head grade,
  recovery, ore-type mix and outage segments (E566–E573, E580–E584).
- **Seeded causes.** The candidate causes the plan pre-registers for attribution (§37.3):
  - head-sampler and tail-sampler bias;
  - the share of transitional ore;
  - throughput and downtime;
  - assay bias.
- **Power and identifiability.**
  - Computed for each subset of the GATE-DATA-03 list:
    - the monthly metallurgical balance;
    - sampler and assay records;
    - shipment and settlement assays;
    - fresh/transitional tags;
    - the reconciliation pack;
    - historian data.
  - The GB-25 linear-Gaussian machinery is used first, then the R2 engine once it lands.
  - FMINE-REF-104 appears exactly where the rank condition fails.
- **Minimal ask.** Greedy forward selection by marginal power per unit of extraction cost. It is reported as a
  ranking with no optimality claim, because power is not submodular in general.
- **Registration.**
  - The analysis plan is hashed into the ledger like a forecast. It covers:
    - the finding definitions;
    - the materiality threshold;
    - the code revision;
    - the seeds;
    - the stopping rule;
    - what counts as a null.
  - A later QP attestation that a finding is "new and plausible" is then credible.
- **No personal data.** Rosters and other personal data are never on the list.

### First shippable slice (M1)

It rides on the planned generator and reconciliation beads, BD-127–BD-133 and BD-149–BD-159.
1. The plant-only Lumwana twin, four seeded causes × five data subsets, and the rendered card.
2. Then North Mara's grade and ore-delivery mechanism (E587–E589), for the alternate site.

TARGET size: about 1.5–2.5k LOC beyond the planned generator and reconciliation work.

### Tests

- **Parity.** The twin's quarterly aggregates reproduce the published Mine Stats series within published rounding,
  with cells cited.
- **Known answers.** On sealed-truth replicates (GATE-GEN-01), the power the card claims for each subset must match
  the empirical detection rate over ≥ 200 replicates, within Monte Carlo error.
- **Determinism.** The card digest is a pure function of (public inputs, seeds, code revision), as in PO-HELM-2.

### What could kill it

| Risk | Response |
|---|---|
| Site experts reject the twin's structure | Keep the relative ranking of data subsets; drop the absolute power claims |
| No feasible data subset has enough power | An early kill of the Lumwana mechanism: switch to the alternate site at month 1, not month 12 |
| The card does not move the data-access decision (BET-06 falsified for this artifact) | Retire it after the site decision |

### Why I am confident

- It reuses machinery the plan already builds for M1, so its own cost is a small increment.
- Either outcome is worth months: a higher P(data), or an early switch of site. The plan's 29-bead critical path
  runs through the pilot (§39).

---

## 7. How the five fit together, and why this order

- **One spine.**
  - Idea 2 certifies the published numbers.
  - Idea 1 scores the published forecasts against those numbers.
  - Idea 4 checks the prose built on both.
  - Ideas 3 and 5 carry the same discipline into the M2 site: Idea 3 lifts its spreadsheets of record, and Idea 5
    designs the pilot that needs them.
- **Why this order.**
  - **Idea 1 first.** It is the most visible to executives, it works with no site data, and it attacks the M0 kill
    criterion directly.
  - **Idea 2 second.** It is the most certainly correct of the five and it supplies the M0 gate's scorer, but its
    audience is narrower.
  - **Idea 3 above Idea 4.** It serves more user types and sits on the M2 critical path (OPEN-TOUCH-1, F5).
  - **Idea 4.** Its strict mode is cheap, but discovery mode carries matching risk.
  - **Idea 5 last.** It depends on M1 machinery that does not exist yet.
- **Boundaries all five respect.**
  - None writes to OT or sets an INV-002 flag.
  - None classifies a resource or reserve, signs anything, or claims regulatory compliance.
  - All repository work stays in the public lane.
  - None touches personal data.
  - FA stays the book of record.

---

## 8. Ideas 6–15 at a glance

Same ground rules as the top five. Ideas 6–15 are ranked best first. Five come from the original 30, and five are
new ideas found while checking those against the plan (appendix rows 31–41).

| # | Idea | Subsystems | Primary users | First slice | Gate claim type |
|---|---|---|---|---|---|
| 6 | **JV Mirror.** Reconcile one JV asset as reported in both partners' public filings | Portal, Bullion, Hallmark, Touchstone | JV partners, CFO, controller, audit committee, auditors, regulators | NGM and Pueblo Viejo, Barrick vs Newmont filings, FY2023–FY2025 (M0–M1) | Parity |
| 7 | **Intertemporal-shift ledger.** How much of each site's AISC came from deferred sustaining capital, deferred stripping, grade draw or reclassification, with a registered forecast that it reverses | Bullion, Plumb, Pushback, Helm | Remuneration and audit committees, board, CFO, COO, JV partners | Carlin, Kibali, Loulo-Gounkoto from Q3 2024 against technical-report LOM baselines (M1) | Known answers now; Reconciliation later |
| 8 | **Fiscal mirror.** Reconcile royalties and taxes across operating disclosures, statutory payments reports, host-government receipts and the registered fiscal terms | Bullion, Helm, Portal | CFO and tax, government relations, host-state regulators, state JV partners | Loulo-Gounkoto, Kibali, Tanzania, Lumwana, FY2023–FY2025 (M1) | Parity |
| 9 | **Guidance lifecycle.** Check that site and consolidated ranges are probabilistically coherent; an anytime-valid rule for when to revise | Plumb, Helm, Bullion | CFO, IR, disclosure committee, board | Coherence diagnostics on FY2023–FY2026 guidance; revision-rule replay (M1) | Known answers; Reconciliation for the replay |
| 10 | **Target Disclosure Auditor.** Check a junior's or developer's drill results, resource statement and technical-report economics before a stake or bid | Lodestar, Hallmark, Helm, Portal | BD and M&A, exploration, executive committee, securities regulators | Synthetic juniors, Barrick's own exploration disclosures, one developer report (M1) | Known answers; Parity for the replay |
| 11 | **Conversion bounds.** Partially identify resource-to-reserve conversion rates from published stock tables (OPEN-HELM-1) | Hallmark, Helm, Lodestar, Plumb | Exploration, QPs, BD, executive committee | Barrick per-site annual statements 2019–2025, plus Fourmile (M1–M2) | Known answers (soundness of bounds) |
| 12 | **Comparability engine.** Compare issuers only where definitions match or a calibrated bridge exists; check "lowest-cost" claims | Bullion, Portal, Adit | CFO, IR, remuneration committee, board, BD | AISC and production for four large gold issuers, FY2024–FY2025 (M1) | Parity; Known answers for bridges |
| 13 | **Sealed-answer agent benchmark.** Tasks hashed before the documents that answer them exist | Testpit, Adit, Portal | CIO/CDO, AI procurement, the agents themselves | 100 open tasks on the ten workbooks and a sealed set on the Q3 2026 release (M0–M1) | Known answers |
| 14 | **Board delta pack.** Exact, order-independent attribution of every change between two board packs to evidence, model, definition, deck, perimeter and restatement | Plumb, Helm, Hallmark, Adit | Board, audit committee, CFO, QPs | Public vintages: restatements, region and perimeter changes, reserve deck changes (M1) | Known answers; Parity on public cases |
| 15 | **Disclosed-disruption library.** Typed, cited operational disruptions from issuer disclosures, with pooled hazard rates | Headframe, Crucible, Plumb, Helm | COO, site GMs, risk committee, planners | Barrick 2023–2026 quarterly MD&As; Lumwana events as fixtures (M1) | Parity (two readers); Known answers |

---

## 9. Idea 6: the JV Mirror

### What the user sees

`fmine mirror --asset ngm --period 2025` returns a MirrorCard for one JV asset and period. Each metric gets one
verdict. The metrics are gold produced and sold, unit cost lines, sustaining and growth capex, and reserves and
resources.
- **Consistent.** Both partners' figures imply the same 100% value within their published rounding.
- **Explained.** They differ, and named bridge layers close the gap: ownership share, metric definition,
  accounting basis, period, perimeter, price deck.
- **Conflict, with proof.** No registered bridge closes the gap. The certificate and the minimal culprit set come
  from idea 2.
- **Refused.** Closing the gap needs a bridge parameter that the public filings do not identify.

*Illustrative output, not a finding:* "Asset J, year Y, gold produced: consistent; both partners' shares imply the
same 100% total within 2 koz. AISC: explained by two layers, by-product vs co-product credits and corporate G&A
allocation; residual inside rounding. Reserves: refused; the partners use different price decks, and no public
grade–tonnage data separate deck from model."

### What is new against the plan

- The plan treats a JV partner as a *viewer* of walled projections and verifiable bundles. The partner's question
  in §3 is "What do I see, and can I verify it without seeing my partner's private data?" (§31.2).
- NGM data are JV-sensitive (E034; pre-mortem F13).
- Newmont appears in the plan only as a source of definition examples and scorecard weights (§21.6, §23.6).
- Nothing reconciles one physical asset as reported by two independent, separately audited issuers.

New here:
1. **Second-reporter reconciliation** of the same asset, from public filings only.
2. **Typed bridge layers** for the ways two honest reporters can differ, each cited to that issuer's own non-GAAP
   reconciliation.
3. **A natural experiment for metric definitions.** Both partners report the same tonnes, grade and cash, so every
   difference is either definitional or an error. The mirror therefore calibrates the definition bridges that
   idea 12 needs.
4. **Counterparty mirrors (phase 2):** streamers and royalty holders that publish deliveries from a named site.

### How it works

- **Pairing.** `MirrorPair{asset, perimeter@as_of, reporters: [(issuer, share, basis)], period}`, from the
  `CapitalEntityGraph` (BD-314) or a registry row. The pairs:
  - FACT: NGM is 61.5% Barrick / 38.5% Newmont (E457).
  - FACT: Pueblo Viejo is 60/40 (E672).
  - FACT: Kibali is 45% Barrick, 45% AngloGold Ashanti, 10% SOKIMO (E560).
  - FACT: Veladero is 50/50 Barrick and Shandong Gold (C4 source table).
  - Perimeter changes are dated. Fourmile, Mike and Fiberline were vended into NGM in 2026 (E558).
- **Identity.** Suppose a = s_a·x and b = s_b·x are the two attributable values of one 100% quantity x.
  - Consistency means a/s_a = b/s_b.
  - Each published value is a half-unit rounding interval (idea 2), so the check is exact interval feasibility,
    not a tolerance.
  - INFERENCE: for koz printed to 1 koz at 61.5/38.5, the implied 100% values can differ by up to
    0.5/0.615 + 0.5/0.385 ≈ 2.1 koz with both reporters correct.
- **Bridges.** `BridgeLayer{kind, parameter, evidence}`, where kind is one of:
  - `Ownership`: the share, and whether the figure is attributable or 100%.
  - `Definition(flag)`: the §21.6 flags (`byproduct_credit_location`, `per_oz_denominator`,
    `includes_inventory_adjustments`), plus G&A allocation and reclamation scope.
  - `AccountingBasis`: each partner carries the asset at its own purchase-price allocation, so DD&A and cost of
    sales differ while cash costs need not. DESIGN: AISC excludes DD&A, so this layer applies to cost of sales and
    not to AISC.
  - `Period` and `Perimeter`, both dated as-of.
  - `Deck`: reserves and resources at each partner's price deck. Barrick's decks by year are E143.
- **Solver.** Idea 2's kernel:
  - feasibility under the registered bridges;
  - a Farkas certificate when infeasible;
  - the minimal set of unexplained metrics;
  - a refusal in the REF-104 pattern when a bridge parameter is not identifiable from public data.
- **Placement.** Pairing and bridges in V (`fmine-public`), the solver in K (`fmine-numerics`), the verb in U
  (`fmine-cli`).

### Workflows

- **Pre-release cross-check.** The operator's controller runs the mirror on draft figures against the partner's
  last filing, before release.
  - INFERENCE: both partners report the asset in separately audited public filings (Newmont's side is FACT[U],
    below), so each can check the other's attributable figures.
  - A mismatch that both partners' public filings would reveal is cheaper to find first.
- **The partner's own check.** A JV partner verifies the operator's public numbers from public data, with no access
  to the operator's private data. This is the zero-trust layer underneath §31.2's verifiable bundles.
- **Auditors, audit committees and securities regulators.** Two audited issuers disagreeing about one asset is a
  finding whichever of them is right.
- **Analysts.** Attributable roll-ups across issuers, without double counting.

### First shippable slice (M0–M1)

1. NGM and Pueblo Viejo, FY2023–FY2025 annual and 2025–2026 quarterly: gold produced and sold, unit cost lines,
   sustaining and growth capex.
   - Barrick's side comes from Mine Stats (attributable, E457).
   - Newmont's side comes from its 10-K and 10-Q segment tables. FACT[U]: Newmont reports NGM and Pueblo Viejo as
     segments on an attributable basis. To be verified with E-ids in week 1.
2. Kibali, against AngloGold Ashanti's attributable reporting.
3. Then reserves and resources, where the deck bridge is expected to refuse.

TARGET size: 1.5–2.5k LOC beyond idea 2's kernel.

### Tests

- **Parity.** Every value from both issuers reproduces its cited page or cell, with capture records.
- **Known answers.** A JV generator produces one physical asset series and two reporters that differ in share,
  definitions, rounding, accounting basis and calendar.
  - Zero conflicts on clean synthetic pairs over 10⁴ draws.
  - An injected misreport at least as large as the rounding slack is detected; power is reported against size.
  - Recovered bridge parameters match the generator's within their intervals.
- **Metamorphic.** Swapping the reporters, or restating both on a 100% basis, leaves every verdict unchanged.
- **Determinism.** MirrorCard digests are stable (INV-010).

### What could kill it

| Risk | Response |
|---|---|
| The partner reports a coarser perimeter (a region, not the asset) | Reconcile at the coarsest common perimeter, and say so |
| Cost definitions are too thinly disclosed to bridge | Most cost lines refuse. Production and capex mirrors still ship |
| Legal sensitivity after the 2026 dispute (E557, E558; F13) | Public filings only. Deliver privately to the sponsor first. FrankenMining publishes nothing about either partner |
| Everything reconciles | A certified clean bill for both audit committees, plus calibrated bridges for idea 12 |

### Why it ranks first of the ten

- A second, independently audited reporter of the same physical quantity is the strongest public oracle there is.
  It is stronger than any synthetic generator.
- JV partners are a named user (§3), and ownership basis is already a hard rule (AGENTS.md), so the question
  matters to both partners.
- It reuses idea 2's kernel almost entirely.

---

## 10. Idea 7: the intertemporal-shift ledger

### What the user sees

`fmine shifts --issuer barrick --site carlin` shows how much of each site-year's AISC per ounce came from four
mechanisms that move cost or metal between periods:
1. **Sustaining deferral.** Sustaining capital per ounce below its baseline.
2. **Stripping deferral.** Waste mined below the plan's strip ratio, which leaves the waste for later years.
3. **Grade draw.** Processed grade above the reserve or LOM-plan grade for that period.
4. **Reclassification.** Spending moved from sustaining to project capital with no registered project.

For each mechanism it shows:
- its effect in $/oz;
- the cumulative "debt", in $M or Mt;
- its evidence class;
- a registered forecast of whether, and when, it reverses.

The ledger never infers intent. It says "AISC includes an estimated −$x/oz from below-baseline sustaining capital",
not "management deferred". At flag time, deferral and genuine efficiency look the same. Only the scored reversal
forecasts, years later, can tell them apart.

*Illustrative output:* "Site S, 2025: AISC $1,540/oz includes −$38/oz (90% interval −$61 to −$12) from sustaining
capital below baseline, and −$22/oz from a strip ratio 0.6 below plan. Cumulative deficit $95M. Registered: P(the
next eight quarters repay the deficit) = 0.62."

### What is new against the plan

The plan names the problem in four places:
- AISC/oz is never the objective, because "high-grading, deferring sustaining capital or reclassifying capital as
  growth can all lower it while destroying NPV" (§15.6);
- deferral lowers AISC, so the KPI rewards it, and every sustaining card shows the hazard–AISC trade-off (§23.2);
- the KPI price list says high-grading, deferral or reclassification can move AISC without changing value (§23.6);
- RISK-22 (Goodhart) lists "deferral detection (GB-26)" as a mitigation.

But GB-26 shows a deferral only when someone has already labelled it (a synthetic −$20M line). Nothing in the plan
detects deferral, stripping deferral, grade draw or reclassification when no one labels them. Detection needs a
baseline, and the plan has none.

New here:
1. **Baselines** from three sources: technical-report LOM schedules, the site's own history, and a hierarchical model
   across sites.
2. **A debt ledger** that accumulates each mechanism over time.
3. **Reversal forecasts**, registered before resolution and scored. They turn "was it deferral?" into a
   Reconciliation claim.
4. **A public-lane slice.** It is possible because the per-site inputs below are all published.

### How it works

- **Inputs (FACT).**
  - Per-site AISC components in $M, from the AISC workbooks, Q3 2024 onward (W1 §12).
  - From Mine Stats, per site: sustaining vs project capex, tonnes mined (ore and waste), strip ratio, mined and
    processed grade, and recovery (Appendix I.9).
  - LOM capex splits and schedules from technical reports, e.g. Carlin (E196, E197) and Kibali (C3, K p336–337).
  - Reserve grades from the reserve statements, and price decks by year (E143).
- **The definitional trap, handled first.** WGC AISC counts production-phase stripping and underground development
  as sustaining (§21.6), but technical reports list them separately.
  - INFERENCE (E196, E197; 100% basis, real Q4 2024; assumes all four lines pass the WGC sustaining test):
    Carlin's "sustaining" line alone is about $67 per recovered ounce over the LOM. Adding capital drilling,
    open-pit stripping and underground development gives about $188/oz.
  - A baseline that skipped this bridge would flag every quarter. The baseline is therefore a `Definition` bridge
    layer, as in idea 6.
- **Mechanism estimates.**
  - *Sustaining.* deficit_t = (b_t − a_t)·oz_t.
    - b_t is the posterior baseline per ounce, from a hierarchical model: site effect, mine phase, LOM average and
      trailing mean. a_t is the actual.
    - The AISC effect is −(b_t − a_t). It is flagged only where its 90% interval excludes zero.
  - *Stripping.* The waste deficit is W_t = s̄_t·ore_t − waste_t.
    - s̄_t is the LOM-schedule strip ratio for that period, where an annual schedule exists. Otherwise the result is
      `NotCalibratable`, because a LOM average hides pushback phasing.
    - The deferred cost is W_t × the published cost per tonne mined.
  - *Grade draw.* r_t = g_processed / g_plan. Its consequence is registered as a forecast: the next reserve
    statement's grade change, conditional on r_t.
  - *Reclassification.* A shift in the sustaining share of total capex with no attested project classification
    (G-BULL-3).
- **Reversal forecasts.** For each flagged deficit, the ledger registers P(repayment within k quarters) and the
  expected repayment path.
  - After resolution, it learns per site whether past deficits were deferrals or efficiencies.
  - The baseline's evidence class can rise above `ModelledUncalibrated` only through that record (INV-006).
- **Waterfall integration.** The four mechanisms become named sub-bars of the cost and grade effects in the LMDI
  waterfall (BD-064, GB-26), so the waterfall still sums exactly.

### Workflows

- **Remuneration committee.** A reproducible decomposition of AISC-linked scorecard figures,
  including any intertemporal shifts.
- **Audit committee and CFO.** The sustaining card of §23.2, filled from data instead of assumed.
- **COO and site GMs.** Stripping and grade-draw debt as an operating risk, with a forecast.
- **JV partners.** An operator-neutral view of intertemporal shifts in the asset's AISC.

### First shippable slice (M1)

1. Carlin, Kibali and Loulo-Gounkoto, Q3 2024 onward: mechanisms 1, 2 and 4 against the technical-report LOM
   baselines, with the definitional bridge.
2. Mechanism 3 against the technical-report reserve grade (e.g. Carlin's LOM processed grade, E196).
3. Register the first reversal forecasts before the FY2026 release.

TARGET size: 2–3k LOC in `fmine-metrics` and `fmine-calib`, plus the verb.

### Tests

- **Known answers.** `fmine-gen` mines that follow their plan exactly, and mines with injected deferrals, stripping
  shortfalls and grade draw.
  - The false-flag rate is ≤ α on the plan-following mines.
  - Injected magnitudes are recovered, with 90% intervals covering 85–95% of the time over ≥ 200 replicates.
  - Detection power against effect size is reported.
- **Properties.**
  - The mechanism sub-bars plus the rest of the waterfall sum exactly to the AISC change (PO-PUSH-10).
  - Reclassification never fires when an attested project classification exists.
- **Parity.** Every input cites its cell or page. The Carlin bridge arithmetic reproduces from E196 and E197.
- **Reconciliation, later.** The ledger scores the reversal forecasts.

### What could kill it

| Risk | Response |
|---|---|
| Most sites have only LOM averages, no annual schedule | Mechanisms 2 and 3 return `NotCalibratable`. Mechanism 1 runs on the site's own history and the hierarchical model |
| Baseline uncertainty swamps the signal | Report the intervals, which include zero, and no flags. That is still a correct answer for the committee |
| The output is read as an accusation | Wording rules: "shift", "debt", "baseline". Never "gaming" or "manipulation". Intent is never inferred |
| Reversal forecasts take years to resolve | The first scores arrive at the eight-quarter horizon. Until then, only Known-answer claims are made |

### Why it ranks here

- The plan's own analysis notes that AISC as a KPI can move with these shifts (§15.6, §23.2, §23.6). It names the
  risk (RISK-22) but gives the remuneration committee no instrument.
- The public data it needs are published per site today.
- It turns a judgment call into a scored forecast, which is the project's core move.

---

## 11. Idea 8: the fiscal mirror

### What the user sees

`fmine fiscal --site loulo-gounkoto --years 2023-2025` reconciles four independent views of what a site paid its host
government:
1. **The operating disclosure.** Royalties per ounce × ounces sold, from Mine Stats. Accrual basis, per attributable
   ounce sold (E110).
2. **The statutory payments report.** Payments by country, project and category under Canada's ESTMA. Cash basis,
   covering entities the reporter controls. FACT[U]: Barrick, as a Canadian-listed extractive issuer, files annual
   ESTMA reports. To be verified with E-ids in week 1.
3. **The host government's receipts.** EITI country reports, where the country implements EITI (FACT[U], per
   country).
4. **The statutory schedule.** The registered fiscal terms and their price-contingent steps (BD-228, BD-296).

It also shows the effective royalty rate per site-quarter, royalties ÷ (realized price × ounces sold). Every gap
between views is either explained by named bridges or flagged.

*Illustrative output, with placeholders for numbers:* "Loulo-Gounkoto 2026: cash royalties paid exceed the
royalties-per-ounce accrual by $x M. Explained: retroactive 2023-Code royalties, penalties and interest for 2024–2025,
paid in April 2026 and booked in other expense, not in royalties per ounce (E412, E413). Effective 2026 royalty
rate y%, against a registered schedule of z%: consistent."

### What is new against the plan

The plan models fiscal terms forward, for valuation:
- versioned price decks and fiscal terms (BD-228);
- `CountryRegime` rows with price-contingent terms (BD-296);
- a Markov chain over regime states (§23.4);
- jurisdiction packs for mining taxes in FA (M4).

It never reconciles payments as published by independent parties. ESTMA never appears in the plan. EITI appears
once, in W2, and only as the host of a DRC legal text (S28).

New here:
1. a four-way mirror of the same flow;
2. effective rates backed out of public data and compared with the registered schedule;
3. a validation set for BD-296's own acceptance test ("Kibali and Loulo rows reproduce"), made of amounts actually
   paid rather than terms as read;
4. typed bridges for the legitimate ways the views differ.

### How it works

- **Bridges.**
  - `Timing`: accrual vs cash, and payment lags.
  - `Ownership`: Mine Stats per-ounce costs are on an attributable basis; payments are 100% of the paying entity.
  - `ReportingPerimeter`: controlled vs jointly controlled entities. Kibali is equity-accounted (§23.2), so whether
    its payments appear in the operator's payments report is OPEN until checked.
  - `Category`: royalties vs production taxes vs dividends to a state shareholder vs fees.
  - `Classification`: amounts booked outside royalties per ounce, as with Mali's retroactive settlement (E412).
  - `Currency`, at stated rates, and `InKind` payments.
- **Effective-rate engine.**
  - With realized price p_t and royalties r_t per ounce, the effective rate is ρ_t = r_t / p_t.
  - It is compared with the registered schedule ρ*(p_t), a step function of price where the terms are
    price-contingent.
  - Registered facts to reproduce:
    - the DRC's statutory precious-metal royalty of 3.5% (E445);
    - Kibali's royalties and charges of about 5.7% of revenue (§23.4);
    - Mali's average royalty of about 18% after the 2023 Code (§23.4; E410–E413).
- **Solver.** Idea 2's kernel over the four views: rounding intervals, the bridges as layers, and refusals where a
  bridge parameter is not public.
- **Placement.** Fiscal rows already belong in `fmine-econ` (K, a planned crate). Import and pairing go in
  `fmine-public` (V).

### Workflows

- **CFO and tax.** A pre-filing consistency check across the operating disclosure and the statutory payments report.
- **Government relations and the CEO, before fiscal discussions.** What has been paid, at what effective rate,
  under which terms. RISK-26 (host-state action: suspension, provisional administration, retroactive code) covers
  events such as those at Loulo-Gounkoto in 2025.
- **Host-state regulators and revenue authorities.** Verify receipts against the operator's public operating data.
  Regulators are named users (§3).
- **State JV partners.**
  - The Government of Tanzania's 16% through Twiga, with its 50/50 economic-benefit true-up (E448).
  - Mali's state interest.
  - SOKIMO at Kibali.
- **Audit committee.** Confirmation that the payments report and the operating disclosure tell one story.

### First shippable slice (M1)

1. Loulo-Gounkoto, Kibali, North Mara/Bulyanhulu and Lumwana, FY2023–FY2025, views 1, 2 and 4.
2. View 3 for whichever host countries have an EITI report covering those years.
3. The Mali retroactive settlement (E412, E413) as the first known-answer fixture for the `Classification` and
   `Timing` bridges.

TARGET size: 1.5–2.5k LOC beyond idea 2's kernel.

### Tests

- **Parity.** Every value in every view reproduces its cited source.
- **Known answers.** A synthetic fiscal regime with price-step royalties, payment lags, currency and a state
  shareholder.
  - Inject an under-payment, a misclassification and a perimeter error.
  - Measure detection and blame accuracy.
- **Properties.**
  - ρ_t is invariant under exact unit changes.
  - The schedule ρ*(p) is monotone wherever the law says it is.
  - The bridges sum exactly.
- **Determinism.** Stable digests.

### What could kill it

| Risk | Response |
|---|---|
| The payments report aggregates several sites into one "project", e.g. a country | Reconcile at the coarser level, and say so |
| EITI reports lag by two or more years | View 3 serves the historical record only |
| ESTMA reports cannot be obtained for the issuer | Drop view 2. Views 1 and 4 still give the effective-rate check |
| Government relations judge the output too sensitive | Deliver privately. Public filings only. Nothing is published about any government |

### Why it ranks here

- Host-state action is a registered plan risk (RISK-26), and payment figures are central to any fiscal
  discussion.
- As in idea 6, independent public reporters exist for the same flow.
- It gives BD-296's fiscal rows a test made of money actually paid.
- It sits below ideas 6 and 7 only because its key source, ESTMA, is FACT[U] until week 1.

---

## 12. Idea 9: the guidance lifecycle

### What the user sees

Two outputs on the CFO's guidance DecisionCard.

1. **A coherence report, before guidance is published.**
   - *Implied correlation.* Suppose the site ranges and the consolidated range share one coverage level. Then the
     consolidated half-width h_T implies an average correlation between site outcomes:
     ρ̄ = (h_T² − Σh_s²) / ((Σh_s)² − Σh_s²).
     - ρ̄ = 1: the consolidated range is the worst-case sum of the site ranges.
     - ρ̄ < 0: the consolidated range is narrower than independent sites would give.
     - ρ̄ > 1: no joint probability model is consistent with the ranges.
   - *Implied corporate layer, for AISC.* Consolidated AISC guidance minus the ounce-weighted site midpoints equals
     the corporate layer the guidance assumes. Compare it with history: the published layer was positive in all
     sixteen quarters 2023Q1–2026Q2, from +$23.57 to +$88.66/oz (INFERENCE, E496).
   - *A coherent proposal.* Site and consolidated ranges drawn from one joint error model, so that each range has
     its stated coverage and they add up. Idea 1's year shock a_y supplies the correlation.
2. **A revision rule, during the year.**
   - After each quarter (monthly in the private lane) it gives P(full-year outcome ∈ published range | year to date)
     and an e-value against the null "the outcome lands in the range".
   - A revision is recommended when the e-value crosses 1/α.
   - The guarantee: across all looks, P(ever recommending a revision when the outcome would have landed in range)
     ≤ α.

*Illustrative output:* "Year Y: the consolidated gold range implies ρ̄ = 0.93 between sites, against an empirical
year-shock correlation of 0.41 ± 0.25. The consolidated range is about 1.6× wider than a coherent one. The AISC
guidance implies a corporate layer of $31/oz, below every quarter since 2023."

### What is new against the plan

What exists:
- NS-04 sets calibrated guidance ranges at M3/M5, on private data;
- idea 1 scores published guidance and nowcasts guidance-at-risk.

Neither checks whether site and consolidated ranges are coherent with each other. The plan also has no logic for
revising guidance: "revision" never appears beside "guidance".

New here:
1. the implied-correlation and implied-corporate-layer diagnostics;
2. coherent range construction from a joint model;
3. an anytime-valid revision rule;
4. a public-lane replay that shows how early each historical revision could have been made.

This promotes appendix ideas 6 and 30, and builds on idea 1's error model.

### How it works

- **Coherence.**
  - A Gaussian approximation in z units comes first; ratio metrics then use the joint simulation.
  - AISC is a ratio of sums, so its coherence runs through the §21.6 identity: company AISC = Σ sold-weighted site
    AISC + L_corp + L_JV.
  - The equicorrelation diagnostic assumes common coverage across ranges. The card states that assumption, and idea
    1's per-site implied coverage relaxes it.
- **Coherent construction.**
  - Simulate the joint distribution of site outcomes from idea 1's hierarchical model: common year shock, site
    effects and residual.
  - Sum each draw to the consolidated outcome, and take the marginal quantiles.
  - Point forecasts are reconciled by MinT-style projection onto the identity graph, the same graph idea 2 builds.
- **Revision rule.**
  - After quarter k, the predictive distribution of Y = YTD_k + R_k comes from site-specific remaining-quarter ratios
    (empirical H2/H1 and Q4/9M ratios) and the run rate.
  - The null "Y ∈ [L, U]" is composite. The e-value therefore uses universal inference, a split likelihood ratio
    valid under composite nulls (Wasserman, Ramdas and Balakrishnan 2020).
  - E-values computed conditionally at each look multiply into a valid e-process (§10.4).
  - With only three quarterly looks this is close to alpha-spending. The anytime property matters for monthly or
    weekly monitoring in the private lane.
- **MNPI.** On private data, the rule's outputs are MNPI. They live behind the wall (§31.1), visible only to the
  disclosure committee. The repository holds only the public-lane replay and the synthetic tests.

### First shippable slice (M1)

1. The diagnostics on Barrick's FY2023–FY2026 published site and consolidated ranges: gold production, AISC,
   copper. The consolidated rows come from the same AIF and MD&A guidance tables as E586 and E587, and are cited in
   the slice.
2. Replay the revision rule over the FY2023–FY2025 quarterly releases. Compare when it would have fired with when
   guidance was actually revised (idea 1's `Revised{n}` vintages).
3. After the Q3 2026 release (9 Nov 2026, E454), register the rule's FY2026 outputs before the February 2027
   outcome. That includes North Mara, where H2 must deliver 1.9–2.4× the H1 rate (E587).

TARGET size: 1–1.5k LOC on top of idea 1.

### Tests

- **Known answers.**
  - Synthetic multi-site years with a known correlation: ρ̄ is recovered with 85–95% coverage.
  - Coherent ranges reach nominal coverage at both the site and the consolidated level.
- **Properties.**
  - In every joint draw, the site outcomes sum exactly to the consolidated outcome.
  - Over 10⁴ synthetic in-range years, the revision rule's false-trigger rate is ≤ α across all looks.
- **Power.** Lead time against miss size on out-of-range synthetic years, reported.

### What could kill it

| Risk | Response |
|---|---|
| The issuer's convention is "consolidated = sum of site ranges", so ρ̄ = 1 by construction | The finding is then that the consolidated range is a worst-case sum, and the coherent proposal is the product |
| Three resolved years are too few to estimate the year-shock correlation | Report only the ρ̄ implied by the published ranges, with no empirical comparison |
| Revision timing is a management judgement that weighs factors beyond the statistics | The card is advisory. It shows the statistical evidence beside the decision |
| MNPI handling in the private lane | Public-lane replay only, until the wall (§31.1) exists |

### Why it ranks here

- Guidance credibility is GB-38's premise and idea 1's market. This idea is the forward half: it changes the next
  guidance, not just the score of the last one.
- Its diagnostics need only the published ranges, so it can ship as soon as idea 1's transcription exists.
- It sits below ideas 6–8 because it depends on idea 1, and its public replay has a small n.

---

## 13. Idea 10: the Target Disclosure Auditor

### What the user sees

`fmine screen target --docs <releases> <resource statement> <technical report>` produces a TargetCard for a junior
or developer, before a stake, earn-in or bid. The card has four parts.

1. **Drill-result arithmetic.**
   - For "L m @ g including l m @ g_i", the implied remainder grade is g_r = (L·g − l·g_i)/(L − l). This
     generalizes to several inclusions.
   - It flags:
     - headlines carried by an inclusion (the remainder grade is below the stated composite cut-off);
     - downhole lengths with no true-width statement;
     - composites with an unstated cut-off, internal dilution or top cut;
     - metal-equivalent grades with unstated prices or recoveries;
     - gaps in the hole sequence when a release reports "results from n holes". This is a HYPOTHESIS signal of
       unreported holes, tested against later full releases.
2. **Resource-statement checks.**
   - Idea 2's solver checks tonnes × grade = metal and the category sums.
   - The cut-off is reproduced as the fixed point g = cost/(net price·R(g)), the method that reproduces E182.
   - The "reasonable prospects" price is compared with the buyer's reserve deck (E143).
3. **Technical-report economics replay**, for developers whose report includes the economic analysis.
   - Rebuild NPV, IRR and payback from the annual cash-flow table.
   - Discover the discounting convention (mid-year or end-year) by finding which one reproduces the published NPV.
   - Check the sensitivity tables against the cash-flow structure.
   - Re-price at the buyer's deck and the buyer's cost reference class: GB-32's blowout mixture, and the company
     effect from its own approvals (§23.5).
4. **A `CapitalCandidate` row** (§23.2, "Junior equity stake" and "M&A").
   - The h_pub and c_FI priors come from idea 11.
   - Every input is evidence-classed `Transferred` or `Assertion`.

*Illustrative output:* "Release R: 3 of 7 headline intervals carry their grade in the inclusion (remainder below
0.3 g/t). True width stated for 0 of 7. Resource: tonnes × grade = metal holds; the cut-off reproduces only at
$2,300/oz, against a stated $1,900. NPV₅ reproduces with mid-year discounting, not end-year. After-tax NPV refused:
no tax schedule given."

### What is new against the plan

What exists:
- §23.2 values junior stakes and acquisitions on "the junior's public NI 43-101/JORC resource, haircut", and M&A on
  "target assets as candidates on public data";
- GB-31 shows a junior's share price implying 93% conversion of inferred ounces;
- Lodestar treats intercepts as data, for example the A3 sub-economic intercept (§11), not as claims to audit.

Nothing ingests or checks a target's disclosures. Drill-result arithmetic, third-party resource-statement checks and
technical-report economics replay are all absent.

FACT: Barrick's own Americas technical reports omit the economic analysis under the producing-issuer exemption, so
they contain no NPV or cash-flow tables to reproduce (C4 §5, checked nulls). Economics replay therefore belongs
here, on developer targets, and not on the issuer's own reports (appendix row 34).

New here: all four parts, and the TargetCard as the single artifact of the BD workflow.

### How it works

- **Parser.** A deterministic grammar for intercept phrases ("X m @ Y g/t Au", "including", "inside", "from–to").
  - Units go through `ExactQty` (feet ↔ metres, oz/st ↔ g/t; `crates/fmine-units/src/lib.rs:275`).
  - Anything outside the grammar goes to cited-mode manual entry. It is never guessed.
- **Economics.**
  - `fmine-econ` (K, a planned crate) builds exact-money cash flows.
  - IRR uses a bracketed root finder that refuses on non-convergence. FACT: today's `brent_root` returns `Some` after
    `max_iter` iterations without converging (`crates/fmine-numerics/src/lib.rs:149`). That defect must be fixed
    before any IRR is published.
- **Re-pricing.** The buyer's deck and reference classes are registry rows. Every adjustment is a visible layer,
  never buried in the discount rate (§23.5).
- **Boundaries.**
  - Public documents only, never embargoed results (MNPI).
  - The card never classifies a resource or reserve (INV-002). It reports what the target's own statement implies,
    and where that statement does not reproduce.

### Workflows

- **BD and M&A.** Screen dozens of juniors and developers quickly and identically, before any data room opens. Once
  there is a data room, idea 3 lifts the target's models.
- **Exploration: earn-ins and option JVs.** Decide which targets deserve a stage-1 spend. GB-31's lesson: "buy
  information, not ounces".
- **Executive committee and board.** A bid recommendation whose every input is cited.
- **Securities regulators.** The same arithmetic over issuers' drill-result disclosures.

### First shippable slice (M1)

1. Drill-result arithmetic and resource checks on synthetic juniors: `fmine-gen` drill programs rendered into
   release text, with seeded selective reporting and missing true widths.
2. The same checks on Barrick's own public exploration disclosures: the Fourmile deck's intercepts and resource
   growth (E140–E142, E656).
3. One public developer feasibility report with an annual cash-flow table, replayed. Week-1 data gate: choose the
   report and capture it.

TARGET size: 2–3k LOC across `fmine-public`, `fmine-econ` (its first slice) and the verb.

### Tests

- **Known answers.**
  - Remainder grades are exact in rationals.
  - Selective-reporting detection power on seeded releases is reported.
  - Zero flags on clean synthetic releases.
- **Parity.** Published NPV, IRR and payback are reproduced to their rounding from the published table, with the
  discovered convention named.
- **Property and metamorphic.**
  - Unit changes and reordering of intervals leave the verdicts unchanged.
  - The identity L·g = l·g_i + (L − l)·g_r holds exactly.

### What could kill it

| Risk | Response |
|---|---|
| Release formats are too irregular to parse | Cited mode: a human or agent enters the intervals, and the arithmetic still runs |
| Developer cash-flow tables omit the tax detail | Replay pre-tax only; refuse after-tax |
| Deal flow is episodic, so the card is used rarely | It is cheap to keep. The same code serves the junior-stake and earn-in decisions of §23.2 |

### Why it ranks here

- Acquisitions and stakes are among the largest capital decisions an executive committee makes. The plan already
  has the valuation machinery (§23.2, GB-31, GB-32), but nothing that checks what goes into it.
- The arithmetic is exact, cheap and testable.
- It ranks below ideas 6–9 because its use is episodic.

---

## 14. Idea 11: conversion bounds

### What the user sees

`fmine conversion --issuer barrick --site <s>` reports two conversion rates as *bounds*, per site-year and pooled:
- c_FI, the fraction of Inferred ounces that became Indicated or better;
- c_IR, the fraction of Indicated ounces that became reserves.

The pooling is by deposit style, drill spacing, reporting code and price deck. Every bound is reported under an
explicit ladder of assumptions.

*Illustrative output:* "Deposit style D, 2019–2025, c_FI per year: [0.08, 0.61] with no assumptions; [0.19, 0.44] if
additions enter only as Inferred; [0.24, 0.38] if no category both gains and loses in one year. GB-31's illustrative
acquisition break-even needs c_FI = 0.93, which lies outside every set."

### What is new against the plan

What exists:
- §23.5 defines R = h_pub·(c_IR·M_ind + c_FI·c_IR·M_inf). It says the priors are "learned from statement-to-statement
  flows in the company's own reserve history, junior filing histories and post-acquisition re-estimates";
- GB-31 uses illustrative values, c_IR = 0.75 and c_FI = 0.45;
- OPEN-HELM-1 lists "Conversion and h_pub datasets" as open.

What the plan does not say is that **those flows are not published.** Statements publish stocks by category each
year. Conversion, attrition, discovery additions, model changes and deck effects are netted together inside the
stocks, so the flows are only partially identified.

New here:
1. the identification analysis;
2. LP bounds that respect the printed rounding;
3. a stated assumption ladder that shows how much each assumption buys;
4. pooled estimates, labelled with the assumptions they rest on;
5. the first construction of OPEN-HELM-1's dataset.

### How it works

- **A stock–flow system per site-year.** The categories are Inferred (I), Measured+Indicated (M) and reserves (P):
  - I_{t+1} = I_t + a_I − f_IM − x_I;
  - M_{t+1} = M_t + a_M + f_IM − f_MP + f_PM − x_M. When resources are reported inclusive of reserves, the f_MP and
    f_PM terms drop out. This is a basis flag, `resources_inclusive_of_reserves`, recorded per issuer and year and
    never assumed;
  - P_{t+1} = P_t + f_MP − f_PM − d_t + m_P;
  - every flow is ≥ 0. Processed contained ounces from Mine Stats bound the depletion d_t. The deck for each year is
    known (E143).
- **Bounds.**
  - c_FI(t) = f_IM / I_t. Its identified set is the [min, max] of that ratio over the feasible set.
  - That is a linear-fractional program, solved as an LP through the Charnes–Cooper transform.
  - Published stocks enter as rounding intervals (idea 2), so the bounds are sound.
- **The assumption ladder.**
  - A0: no assumptions.
  - A1: discovery additions enter only as Inferred. A deck increase would violate this, because it can add
    Indicated material directly.
  - A2: no category both gains and loses within one year.
  - A3: downgrades happen only in years when the deck or costs worsen.
  - A4: depletion equals processed contained ounces × a published reconciliation factor.

  Each rung is DESIGN, with a HYPOTHESIS test wherever the issuer publishes a roll-forward (the company-level
  roll-forward used by GATE-HALL-01).
- **Worked datum (INFERENCE from E140; illustrates the method).**
  - Fourmile, Dec 2024 → Dec 2025: Indicated 1.4 → 2.6 Moz, Inferred 6.4 → 13 Moz, no reserves and no depletion.
  - Under A1, f_IM ≥ 1.2 Moz, so c_FI ≥ 0.19 for that year, and Inferred additions were ≥ 7.8 Moz.
- **Pooling.**
  - A hierarchical model on interval-valued data over deposit style × drill spacing (E142's 35 m and 80–90 m
    targets) × reporting code × deck.
  - Posteriors are `ModelledUncalibrated` and are always shown beside the assumption-free bounds.

### Workflows

- **Exploration.** Budget priors for converting Inferred ounces at a given drill spacing. They feed drill VOI
  (§11.5).
- **QPs.** A reference class for the confidence wording around Inferred material. The system never classifies
  (INV-002).
- **BD (idea 10).** c_FI and c_IR priors, with honest widths, for every target.
- **Executive committee.** What the company's own history says inferred ounces are worth.

### First shippable slice (M1–M2)

1. Capture Barrick's per-site annual reserve and resource tables, 2019–2025, from the annual reports. This is a
   week-1 data gate.
2. Compute bounds per site-year under A0–A4, pooled bounds by deposit style, and Fourmile (E140) as a fixture.
3. Recompute GB-31 with these bounds in place of its illustrative values.
4. Then add other issuers' per-deposit statements to the pool.

TARGET size: 1–1.5k LOC on idea 2's kernel.

### Tests

- **Known answers: soundness.** A generator with known flows, aggregated to rounded stocks. The identified set
  contains the true rate in 100% of ≥ 10⁴ replicates; any miss is a bug.
- **Known answers: sharpness.** The bounds equal the generator's own min and max over its feasible flows.
- **Coverage.**
  - When the assumptions hold, the pooled posterior's 90% intervals cover the truth 85–95% of the time.
  - Miscoverage when the assumptions are violated is reported.
- **Parity.** The stocks reproduce the published tables, and the roll-forward closure agrees with GATE-HALL-01.

### What could kill it

| Risk | Response |
|---|---|
| The bounds are vacuous ([0, 1]) even at A4 | Publish only the net category changes, and drop the rates. That is itself the answer to OPEN-HELM-1 |
| The deck changes every year (E143), confounding economic and geological change | Keep the deck as an unknown with a sign constraint. Refuse to separate the two where the data cannot |
| Per-site annual tables cannot be obtained for enough years | Pool across issuers, or stop at single data points like Fourmile |

### Why it ranks here

- It is the honest answer to an OPEN item that sits under every exploration and acquisition decision in §23.
- The method is small, exact and sound by construction, and it reuses idea 2.
- It ranks below idea 10 because its value arrives through other decisions, and its bounds may turn out wide.

---

## 15. Idea 12: the comparability engine

### What the user sees

`fmine compare --metric aisc --issuers <a,b,c,d> --period 2025` returns a comparison only where one is valid:
- values restated on one basis through definition bridges, with each layer cited;
- a rank *interval* per issuer, from rounding and bridge uncertainty;
- typed refusals, e.g. "not comparable: co-product vs by-product credits cannot be bridged without the by-product
  revenue split".

It also checks comparative claims such as "lowest-cost quartile" or "industry-leading AISC". The verdicts:
- supported;
- unsupported;
- indeterminate: the rank interval straddles the threshold;
- refused: the claim names no peer set or basis.

*Illustrative output:* "2025 AISC, by-product basis, per ounce sold. Rank intervals: issuer A [2, 3], B [1, 2], C [1,
3], D [3, 4]. 'Lowest quartile among these four': indeterminate. 'Below the median': supported for A."

### What is new against the plan

What exists:
- §21.6 carries definition flags on every AISC-family metric, and names the by-product/co-product layer using
  Newmont's figures (W5 #36);
- §23.6 excludes rTSR vs GDX from optimization.

The plan never compares issuers. New here:
1. per-issuer definition rows, mapped from each issuer's own non-GAAP reconciliation into WGC components;
2. a comparison verb that refuses invalid comparisons;
3. rank intervals;
4. checking of comparative claims;
5. bridges calibrated on idea 6's JV mirrors. When the same asset is reported twice, the mirror shows how large each
   definitional difference actually is.

### How it works

- **Rank interval.**
  - For interval values [lo_i, hi_i], with rank 1 the lowest cost, issuer j's rank lies in
    [1 + #{i: hi_i < lo_j}, n − #{i: lo_i > hi_j}].
  - The interval is exact and needs no distributional assumption.
- **Bridge transfer.**
  - A bridge measured on a mirror asset is applied to a different asset only as a `Transferred` layer with its
    spread (INV-006).
  - Before use, it is tested on a held-out mirror pair.
- **Claims.**
  - A comparative claim must name a peer set, a metric, a basis and a period, or it is refused.
  - Proprietary industry cost curves are out of scope. Where a claim cites one, the checker says so and refuses.

### Workflows

- **CFO and board.** "Where do we stand?", on a basis that survives scrutiny.
- **IR.** A pre-flight for comparative statements in decks and releases, together with idea 4's lint.
- **Remuneration committee.** Peer context for cost metrics.
- **BD.** A target's costs compared with the buyer's own, on the same basis.

### First shippable slice (M1)

- AISC and gold production for four large gold issuers, FY2024–FY2025. Each issuer's definition row is cited to its
  own reconciliation.
- Rank intervals.
- Claim checks on a synthetic claim set, and on comparative statements from public decks, labelled by two readers.
- Every output is labelled "from public filings; no affiliation implied" (AGENTS.md).

TARGET size: 1.5–2k LOC in `fmine-metrics` and `fmine-public`.

### Tests

- **Parity.** Each issuer's values reproduce their cited sources.
- **Known answers.** Synthetic issuers with known definitions: the bridges are recovered, and the true rank always
  lies inside the rank interval.
- **Properties.**
  - Comparison is refused whenever definition flags differ and no bridge is registered.
  - Restating every issuer on one basis never turns a refusal into a verdict without a cited bridge.

### What could kill it

| Risk | Response |
|---|---|
| Peers disclose too little to map their definitions | Mostly refusals: honest, but of little use. Ship production and capex comparisons only |
| Bridges do not transfer between assets (they fail on the held-out mirror) | Compare only same-asset pairs and issuers with the same definitions |
| Contradicting a peer's public claim is sensitive | Private use; public filings only |

### Why it ranks here

- Every executive asks for peer comparisons, and the plan's definition flags make honest ones feasible.
- But it is closer to planned work than ideas 6–11, and it needs idea 6 to calibrate its bridges.

---

## 16. Idea 13: the sealed-answer agent benchmark

### What the user sees

`fmine bench run --agent <endpoint>` scores any AI agent or AI operating layer on mining-disclosure tasks whose
answers FrankenMining can verify:
- reproduce a site-quarter value, with its cell citation;
- find the inconsistency in a table (idea 2's fixtures, E113–E116);
- compute an AISC bridge on a named basis;
- convert between the attributable and 100% bases;
- **refuse** when the answer cannot be determined, e.g. a per-site GEO that is never published (E499, E148).

**Scoring.**
- A correct, correctly cited answer scores +1, a typed refusal 0, and a wrong or uncited number −k.
- An agent maximizes its expected score by answering only when P(correct) > k/(1 + k). With k = 3, that means only
  above 0.75.
- Honest refusal is rewarded and fabrication punished, in the spirit of the project's refusal discipline
  (AGENTS.md).

**The sealed set** is built from releases that do not exist yet.
- Before a release (Q3 2026: 9 Nov 2026, E454), FrankenMining hashes the task texts and their answer recipes into the
  ledger.
- After the release, the verified pipeline computes the answers and two readers confirm them.
- No model can have trained on answers that did not exist when the tasks were committed. This is register before
  resolve (INV-008) applied to evaluation.

### What is new against the plan

What exists:
- GATE-ROBOT-01 tests whether a fresh agent can operate FrankenMining from `robot-docs`: 20 scripted tasks, ≥ 18
  correct, at M1 (§30.4);
- agent *forecasts* are scored in the ledger (§30.4).

Nothing evaluates third-party agents' numerical honesty on mining disclosures, and nothing uses time-sealed tasks.

New here:
1. the benchmark;
2. a scoring rule that prices abstention;
3. sealed tasks, committed before the documents that answer them exist;
4. hard cases taken from the evidence register:
   - copper rows labelled Mlb that hold kilotonnes (E463);
   - a restatement without a footnote (E474);
   - two vintages that give different dates for the same event (E476);
   - comparatives restated after a region change (E461).

### Workflows

- **CIO/CDO and procurement.** Choose and monitor AI layers by measured numerical honesty.
  - The plan's diagnosis of AI operating layers is "reasoning over inputs that carry no
    reconciliation state or version" (§1.1).
  - The benchmark is vendor-neutral and complements the AI platforms issuers already use (E590, E591).
- **The agents.** Regression-test themselves before they touch real decisions.
- **FrankenMining.** The open set doubles as GATE-ROBOT-01's task pool, and lint (idea 4) checks the agents'
  free-text answers.

### First shippable slice (M0–M1)

1. 100 open tasks on the ten public workbooks (2024Q1–2026Q2). Answer keys come from pinned fixtures and two
   readers.
2. A sealed set of about 40 tasks on the Q3 2026 release, hashed before 9 Nov 2026 (E454).
3. A second sealed set, hashed before the FY2026 release in February 2027.

TARGET size: about 1k LOC: task format, scorer and verb.

### Tests

- **Known answers.** Answer keys come only from pinned fixtures confirmed by two readers, never from the candidate
  (AGENTS.md, honesty rule 1).
- **Properties.**
  - The scorer is a proper abstention rule: a truthful agent's best policy is to answer exactly when
    P(correct) > k/(1 + k).
  - A refusal is never scored as wrong. An uncited number is never scored as correct.
- **Determinism.** Scores are reproducible from the tasks, the answers and the agents' transcripts.

### What could kill it

| Risk | Response |
|---|---|
| Vendors will not run it | It remains the internal agent-regression suite and GATE-ROBOT-01's pool |
| Maintenance cost per release | Sealed sets only twice a year (Q3 and FY), generated from recipes, not hand-written |
| Once public, agents overfit to the task style | Rotate the templates. Sealed answers still cannot leak |

### Why it ranks here

- "Which AI can we trust with our numbers?" is a question every executive committee is asking now. The project's
  evidence register supplies the set of hard cases an honest benchmark needs.
- It is small.
- It is a tool for choosing AI, not a capability users run every day, so it ranks below ideas 6–12.

---

## 17. Idea 14: the board delta pack

### What the user sees

`fmine delta --from <board pack 2026-07> --to <board pack 2026-10>` breaks the change in each headline number
(reserves, guidance, a project's NPV, a site forecast) into provenance factors:
- new evidence (actuals, assays);
- model revision (recipe version);
- definition change (`rules_version`);
- price deck and FX;
- ownership or perimeter change, e.g. the Hemlo and Tongon sales (E552–E554) or Fourmile's vend-in (E558);
- restated inputs (E474);
- corrected errors (superseded authority records, INV-003).

The factor contributions sum exactly to the total change, and they do not depend on the order in which the factors
are considered. Physical drivers inside "new evidence" expand into the LMDI waterfall (GB-26).

*Illustrative output:* "Reserve GEO +4.1 Moz since the last pack: deck +2.7, new drilling +1.9, model revision −0.8,
depletion −1.6, perimeter +1.9 (vend-in). The sum is exact."

### What is new against the plan

What exists:
- vintages are kept;
- records are superseded, never overwritten (INV-003);
- dependants are marked stale on read (INV-013);
- physical drivers are decomposed exactly (GB-26).

The plan never attributes a change across *provenance* factors; it has no "what changed and why" verb.

New here:
1. Shapley attribution over provenance factors, run on the as-of machinery;
2. a definition-drift detector that flags when an issuer's own non-GAAP reconciliation structure changes between
   vintages.

### How it works

- **Shapley over factors.**
  - k factors each switch from old to new. v(S) is the recipe evaluated with the factors in S at their new state and
    the rest at their old state.
  - φ_i = Σ_{S ⊆ F∖{i}} |S|!(k − |S| − 1)!/k! · [v(S ∪ {i}) − v(S)].
  - Properties:
    - efficiency: Σφ_i = v(F) − v(∅), exact in `ExactQty` wherever the recipe is rational;
    - symmetry;
    - a zero for any factor that did not change.
  - With k ≤ 8, that is at most 256 deterministic recomputations.
- **Requirements.**
  - Recipes must be replayable with a mix of old and new inputs.
  - The cognition plane is rebuildable from the authority plane plus recipes (§6.1), and `as_of_decision` supplies
    the old inputs (INV-016).
  - Where an old input is missing, that factor is refused, not estimated.
- **Definition drift.** Diff each issuer's non-GAAP reconciliation lines between vintages. An added, removed or
  renamed line becomes a `rules_version` factor.

### First shippable slice (M1, public lane)

1. Mine Stats vintages:
   - the 2024Q1 AISC restatement (E474);
   - Pueblo Viejo's region move, with restated comparatives (E461);
   - the conflicting loss-of-control dates (E476).
2. Reserve statements, 2024 → 2025: the deck change (E143) against the physical change, for GEO. E147 shows a printed
   row that reproduces only at the following year's deck.

TARGET size: 1–1.5k LOC in `fmine-decide` and `fmine-cli`.

### Tests

- **Known answers.**
  - Additive recipes, where each factor's Shapley value equals its own effect.
  - Multiplicative recipes, compared with LMDI.
- **Properties.** Exact efficiency, symmetry, a zero for dummy factors, and invariance to factor order.
- **Parity.** The public restatement and deck cases reproduce their E-row arithmetic.

### What could kill it

| Risk | Response |
|---|---|
| Old recipes or inputs cannot be replayed | Refuse those factors and ship the rest |
| More factors than 2^k recomputations allow | Group the factors into at most eight classes |
| Boards prefer narrative | Generate the narrative from the table, and lint it (idea 4) |

### Why it ranks here

- Every board pack opens with "what changed since last time?", and today the answer is a narrative. This makes it
  an exact table.
- Private use needs the M1 ledger, so its value arrives later than that of ideas 6–13.

---

## 18. Idea 15: the disclosed-disruption library

### What the user sees

`fmine disruptions --issuer barrick` shows a cited library of the operational disruptions an issuer has itself
disclosed, quarter by quarter. Each event carries:
- site and period;
- cause class: plant outage, power, water and weather, geotechnical, security and community, labour, supply,
  regulatory and fiscal, or ore body;
- the affected circuit;
- the stated duration;
- its quantified effect, where it is stated or derivable.

On top sit pooled hazard rates by cause and site, and the expected effect on full-year outcomes.

*Illustrative output:* "Site S, 2023–2026: 5 plant or power events in 14 quarters. Pooled plant-outage hazard across
copper sites: 0.21 per quarter (90% interval 0.12–0.33). Expected full-year copper loss from plant events: 2.1%
(0.9–3.8%)."

### What is new against the plan

What exists:
- hazard models for TSFs, fleet and plant, on private data (§16–§18);
- a host-state event registry for `CountryRegime` (W2, E401–E449).

The plan has no library of the operational disruptions issuers themselves disclose. "Outage" and "disruption" each
appear once in it.

New here:
1. the library itself;
2. its pooled hazard rates;
3. its use as the event model behind three other ideas: idea 1's guidance-at-risk, idea 5's twin (the Lumwana outage
   segments) and idea 9's revision rule.

### How it works

- **Import.**
  - Events are curated from the issuer's own MD&As and releases, with page citations.
  - Agents may propose events as `Assertion`s; two human readers confirm them.
  - No LLM sits in the verdict path.
  - Vague narratives ("operational challenges") are refused, not coded.
- **Quantification.** Where a disclosure states no effect, the LMDI waterfall (BD-064) gives the quarter's throughput
  and recovery effects. The event's share of those effects is `Derived`, never `Measured`.
- **Hazards.** Poisson or negative-binomial rates with site random effects, pooled across sites and, later, across
  issuers.

### First shippable slice (M1)

- Barrick's 2023–2026 quarterly MD&As, all sites, read by two readers.
- The Lumwana events already in the register (E580–E584) as fixtures.
- Hazard estimates by cause class.

TARGET size: about 1k LOC.

### Tests

- **Parity.**
  - Two-reader agreement (Cohen's κ), reported per cause class.
  - The fixtures reproduce E580–E584.
- **Known answers.** Synthetic quarterly series with injected events. Effects are recovered with coverage, and
  hazards with calibrated intervals.
- **Property.** No event without a citation.

### What could kill it

| Risk | Response |
|---|---|
| Narratives are too vague (low κ) | Keep only quantified events |
| Too few events per cause class | Pool across issuers, or refuse to give rates |
| The library drifts into news monitoring | Issuers' own disclosures only. Engines never fetch |

### Why it ranks fifteenth

It is useful, but in a supporting role. Its main value is as an input to ideas 1, 5 and 9, and the extraction takes
a lot of human reading.

---

## 19. How ideas 6–15 fit with the top five

- **Mirrors (6, 8, 12).** The same quantity reported by independent parties is the strongest public oracle. Idea 6
  mirrors JV partners and idea 8 mirrors governments. Idea 12 uses what the mirrors reveal about definitions.
- **Forecasts and bounds (7, 9, 11, 15).** Each turns a judgment into a registered, scored forecast or an honest
  bound, in the same ledger as idea 1.
- **Decisions (10, 14).** The TargetCard and the delta pack put the evidence in front of the executive committee and
  the board.
- **Agents (13).** The benchmark measures whether the agents working for these users can be trusted with numbers;
  idea 4 checks what they write.
- **Shared kernels.**
  - Ideas 6, 8, 10, 11 and 12 need idea 2's solver.
  - Ideas 7, 9 and 15 need idea 1's error model or the LMDI waterfall (BD-064).
  - Building ideas 1 and 2 first makes most of the next ten cheap.
- **Boundaries.** The same as the top five (§7). In addition:
  - ideas 6 and 8 never infer private partner or government data;
  - idea 9's private-lane outputs are MNPI and stay behind the wall (§31.1);
  - idea 10 never touches embargoed results;
  - ideas 10 and 11 never classify a resource or reserve.

---

## Appendix: every idea considered

Rows 1–30 were the round-1 candidates for the top five. Rows 31–41 were added while choosing ideas 6–15.
Dispositions reflect both rounds.

| # | One-line title | Disposition |
|---|---|---|
| 1 | Public Track Record: guidance, revisions, technical-report schedules and capex estimates scored as pre-registered forecasts | **Top five, #1** |
| 2 | Implied coverage and bias of published guidance ranges, with conformal recalibration | Merged into #1 |
| 3 | Price-conditional ("controllable") scoring of guidance from published decks and royalties per ounce | Merged into #1 |
| 4 | Guidance-at-risk nowcast: P(full-year outcome inside guidance given YTD), each quarter | Merged into #1 |
| 5 | Quarterly per-site nowcasts frozen before every Mine Stats release, raced against guidance and naive baselines | Merged into #1 (the race) |
| 6 | Coherent forecasts: project site forecasts onto the disclosure identity graph (MinT-style), with the Touchstone WLS kernel | Promoted into idea 9 (coherent ranges); reuses #2's constraint graph |
| 7 | Rounding-exact disclosure consistency solver with Farkas proof certificates and minimum correction sets | **Top five, #2** |
| 8 | Hidden-parameter discovery by feasibility: rounding convention, ownership share, price deck, unit | Merged into #2 |
| 9 | Pre-publication "pre-flight" check of draft disclosure workbooks for the issuer | Merged into #2 (workflow) |
| 10 | Spreadsheet Lifter: workbook formula graphs → typed, evidence-classed Claim DAGs with an assumption register | **Top five, #3** |
| 11 | R0 parity by lifting the site's own reconciliation-pack formulas (resolves OPEN-TOUCH-1) | Merged into #3 |
| 12 | Disclosure Lint: number-level verification of human and agent prose against the Claim store | **Top five, #4** |
| 13 | Cross-document consistency: MD&A text vs Mine Stats vs AISC workbook vs results release | Merged into #4 |
| 14 | Pilot Protocol Card: pre-registered pilot design on a twin calibrated to public data | **Top five, #5** |
| 15 | Data-ask minimiser: power curves by data subset give the smallest justified file list | Merged into #5 |
| 16 | Reserve-realization reference class: reserve grade vs realized processed grade (OPEN-PLUMB-7) | Merged into #1 (phase 3) |
| 17 | Cross-issuer comparability engine (Newmont, Agnico Eagle, Kinross) that matches definition flags and refuses non-comparable comparisons | **Ideas 6–15, #12**, with bridges calibrated on idea 6's mirrors |
| 18 | Controllable AISC restated at the guidance price deck, as a hypothesis test for OPEN-BULL-1 | Merged into #1 |
| 19 | Public-lane sentinel for sustaining-capex deferral (AISC beats driven by sustaining cuts) | Promoted and widened into **idea 7** (four mechanisms, baselines, reversal forecasts) |
| 20 | Vintage lattice and restatement sentinel for every published value | Mostly planned (E474 handling); minor extension |
| 21 | Board delta pack: what changed between two anchors, and why (evidence, model, definition, deck, restatement) | **Ideas 6–15, #14**; public-lane slice first, private use needs the M1 ledger |
| 22 | Public forecast transparency log, with inclusion and consistency proofs and several independent timestamp anchors | Fold into BD-068's implementation |
| 23 | Forecast tournament for agents and analysts, with pairwise e-value races and performance-weighted pooling | Mostly planned (§10.2, §10.6, §30.4) |
| 24 | Public-lane GISTM evidence-state map from issuers' published conformance disclosures | Deferred to M5; high risk of being read as a safety statement |
| 25 | Sustainability metric assurance: GHG and water intensity, with LMDI decomposition and ownership basis | Deferred; outside the wedge's metrics |
| 26 | Auditor for junior press-release drill intercepts ("including" arithmetic, implied remainder grade, selective reporting) | Promoted into **idea 10** (part 1 of the TargetCard) |
| 27 | Host-state early-warning register of dated public events, feeding `CountryRegime` | Rejected for now; drifts toward news monitoring, and engines never fetch |
| 28 | Diff between technical-report editions, each change registered as a forecast revision | Merged into #1 (phase 2) |
| 29 | Fail-closed source manifest: sha256-pinned; a missing file is FMINE-REF-502 and never a substituted workbook | A precondition for all five; a defect fix, not a feature |
| 30 | Guidance-revision DecisionCard: when to revise guidance, weighing early vs late credibility cost | Promoted into **idea 9** (anytime-valid revision rule); private-lane outputs are MNPI |
| 31 | JV Mirror: reconcile one JV asset as reported in both partners' public filings, plus streamer and royalty-holder mirrors | **Ideas 6–15, #6** |
| 32 | Fiscal mirror: operating disclosure vs statutory payments report vs host-government receipts vs registered fiscal terms | **Ideas 6–15, #8** |
| 33 | Partial identification of resource-to-reserve conversion rates from published stock tables (OPEN-HELM-1) | **Ideas 6–15, #11** |
| 34 | Technical-report economics replay: NPV, IRR, payback and sensitivities rebuilt from published cash-flow tables | Merged into #10 for developer targets. Rejected for the issuer's own reports, which omit the economic analysis (C4 §5) |
| 35 | Sealed-answer benchmark for AI agents on mining disclosures, with an abstention-pricing scorer | **Ideas 6–15, #13** |
| 36 | Disclosed-disruption library with pooled hazard rates | **Ideas 6–15, #15** |
| 37 | Public-lane decomposition of closure-cost estimates and the ARO roll-forward | Mostly planned (§18.6, GOAL-014, G-FREE-11) |
| 38 | Concentrate settlement checker: payables, TC/RC, quotational periods, provisional pricing | Planned (§21 domain pack, row 8; FD-FA-07) |
| 39 | Reserve price-deck DecisionCard: reserve, impairment and scorecard effects of the deck choice | Deferred; estimating reserves at other decks needs block models (Pushback) |
| 40 | Definition-drift sentinel for issuers' non-GAAP reconciliations | Merged into #14 |
| 41 | Public check of the Twiga 50/50 economic-benefit true-up (E448) | Planned inside BD-314; public data too thin for an independent check |
