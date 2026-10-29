# Codex scores of Claude's fifteen FrankenMining proposals

**Review date:** 2026-10-29. **Input:** the complete 1,867-line
[WIZARD_IDEAS_CC.md](WIZARD_IDEAS_CC.md), including its expansion and appendix.
**IDs:** CC-01 through CC-15 below identify the file's Idea 1 through Idea 15; these are review labels.

**INFERENCE — Scoring.** These are overall judgments of usefulness, soundness, public-lane deliverability,
maintenance cost and incremental value, rather than novelty scores alone. Overlap with a good idea does not
make it bad, but already-funded functionality does not earn credit as a new feature. Scores of 800–899 mean
strong candidates with bounded implementation conditions; 700–799 mean worthwhile after narrowing; 500–699
mean substantial unresolved trade-offs; below 500 means the submitted mechanism needs material revision.
No score here means that another feature should precede repairing M0.

**FACT — Baseline.** The [round-7 audit, Bottom Line and C001–C006/C058–C059](../mor/r7/MODES_OF_REASONING_REPORT_AND_ANALYSIS_OF_PROJECT.md#bottom-line)
finds substituted workbook inputs, wrong issuer-attributed figures and refusal on real inputs at committed
revision `68e709122ebb0c6b50fb6c1b5dbe558c3921a864`; the fallback remains visible in
[`fmine-public/src/lib.rs:224–267`](../../crates/fmine-public/src/lib.rs#L224).
CC acknowledges this explicitly, which is a strength, but a fail-closed manifest alone does not implement
label-anchored extraction, source/basis verification, unblocked arithmetic or a trustworthy gate scorer.
I charge those missing prerequisites to affected first slices and do not treat existing scoring helpers as a
complete admitted forecast service.

**DESIGN — Boundaries.** All fifteen can be framed as read-only evidence tools while retaining INV-002,
INV-011 and FA's ownership of actual accounting (§21.1–§21.3, §31.3).
The particular hazards are false compliance implications in CC-08, reserve-equivalent or sensitivity outputs
escaping their intended scope in CC-10/CC-11/CC-14, and unsupported certainty in CC-01/CC-05/CC-09/CC-15.
Public filings are admissible inputs, not proof of causal identification, independent measurement or compliance.
Scores and critiques below are **INFERENCE**; proposed changes are **DESIGN**, and adoption benefits remain
**HYPOTHESIS** pending user tests.
References to “the plan” mean the [technical constitution](../../COMPREHENSIVE_PLAN_FOR_THE_DESIGN_OF_FRANKENMINING.md),
including its current working-tree text; COD references identify my [existing proposal file](WIZARD_IDEAS_COD.md).

## CC-01 — The Public Track Record

**Score: 730 / 1000.**

A versioned record of what management actually guided, when it revised guidance and what subsequently happened
would give CFOs and IR a useful public-data product beyond reproducing familiar tables.
The proposed statistical expansion outruns its first dataset: three resolved year blocks cannot support the
claimed finite distribution-free 80% split-conformal recalibration, whose required rank is
⌈(3 + 1) × 0.8⌉ = 4 among only three calibration scores, and ordinary binomial intervals do not automatically
handle the acknowledged dependence between sites (§10.3–§10.4).
Removing a modeled royalty effect does not make the remaining miss “controllable,” and historical publication
must be preserved as evidenced availability rather than backdated into this system's ledger record time
(§10.1, INV-016).
Ship cited initial/revised guidance, first-eligible and restated outcomes, fixed-cohort denominators and
descriptive misses after real M0 extraction works, then earn hierarchy, recalibration and forecast-race claims
with more independent years and predeclared scoring levels.
**Duplication:** substantial extension of Appendix C's forecast ledger/public mirror, §10, NS-04, BD-047–049,
BD-067–068/076/276; overlaps COD-01's guidance workflow and COD-08's outcome vintages, and CC-09 should be its
later extension rather than a separate forecasting product.

## CC-02 — Proof-carrying discrepancies

**Score: 875 / 1000.**

This is the strongest proposal: an exact, inspectable contradiction between named public cells is useful to a
controller, QP or JV reviewer, and the E113–E116 fixtures give it an immediate test beyond synthetic success.
The rational certificate checker is tractable, but the full solver, complete minimum-correction search,
nonlinear relaxations and parameter discovery are substantially more work than the “elementary” mathematics
and 2–3k LOC estimate suggest, especially with rounding ties, i128 growth and multiple valid explanations.
A minimum correction set identifies constraints that must change under the assumed identities, rather than
proving which published cell is erroneous, and feasibility alone cannot replace extraction fidelity or silently
redefine BD-272's reproduced/explained/refused denominator (§21.6, §33.1, §37.3).
Start with exact linear sum/ownership/half-year contradictions and an independently checked certificate,
explicitly separate model/basis errors from cell errors, and preserve the proposal's sound distinction between
nonlinear infeasibility and a merely feasible relaxation.
**Duplication:** essentially the same core addition as COD-02, materially extending Appendix C's discrepancy
register, BD-066/275/279 and GB-26; CC-06, CC-08, CC-10 and CC-11 reuse this kernel rather than constituting
independent solver innovations.

## CC-03 — The Spreadsheet Lifter

**Score: 580 / 1000.**

Reading the spreadsheet users already trust is a compelling adoption strategy, and a graph of overrides,
external links and broken references could save real finance and QP review time.
The submitted scope is effectively a new spreadsheet engine plus semantic inference: roughly 60 functions,
reference semantics, coercion, rounding, unit scales, basis inference and sensitivity propagation are not a
credible 3–4k LOC increment, while unknown units and unsupported upstream formulas can invalidate a headline
even when most cells are covered (§25, §28.4–§28.5).
Cached values need independently captured recalculation provenance and may themselves be stale, and lifting a
pack's formulas does not establish R0 parity “by construction” when macros, manual adjustments or conventions
are outside the lifted graph (GATE-TOUCH-03, §33.2).
Ship structural audit first, followed by a tiny explicitly supported formula subset with human-confirmed units
and an independent spreadsheet oracle, and report dependency-complete coverage for each output instead of a
reassuring workbook-wide percentage.
**Duplication:** formula lifting is materially new beyond §32 and BD-186/187/287, with no close COD equivalent;
it overlaps CC-02's table checks, CC-04's claim verification and CC-14's recipe comparisons, which should share
the supported expression representation rather than grow separate evaluators.

## CC-04 — Disclosure Lint

**Score: 810 / 1000.**

Strict cited mode has an excellent cost-to-value ratio for IR, board-paper authors and agents because it can
catch stale values, wrong periods and denominator mistakes at the point a statement leaves its source table.
Matching a numeral to a Claim does not verify the surrounding assertion, so a correct cited amount can still
appear in a false comparison, an unsupported “on track” sentence or a statement about the wrong subject.
Discovery mode adds difficult ambiguity and negation problems, and even perfect matching would faithfully
endorse bad M0 Claims unless source integrity and complete Claim identity are repaired first
(`fmine-claim/src/lib.rs:312–339`; audit C001–C006).
Ship strict references with explicit span, metric, basis and period binding, narrowly check supported sentence
predicates, label remaining prose unchecked and test false semantic associations as aggressively as digit
mutations (§30.4–§30.5, INV-002).
**Duplication:** strong convergence with COD-10, extending planned rendering and BD-072/123 rather than
inventing basic provenance; CC-12's comparative assertions should be another supported predicate, and CC-13
can consume the same checker.

## CC-05 — The Pilot Protocol Card

**Score: 625 / 1000.**

A concrete, justified smaller data request and a registered null-result plan directly address the sponsor and
IT bottleneck in §34.4 and §37.3, which is a more useful product than another readiness checklist.
Matching Lumwana's quarterly public aggregates cannot identify sampler-error covariance, ore-type effects and
measurement bias well enough to establish real-site detection power, so accurate simulation power under the
chosen twin is not power demonstrated for Lumwana (§33.1–§33.3).
The proposed fallback of retaining relative data-subset rankings after experts reject the twin is also unsafe,
because changing the causal or measurement structure can reverse those rankings.
Keep the first slice as synthetic protocol rehearsal with power and subset rankings across several explicitly
misspecified models, show robust versus model-dependent data requests, and retire the card if it changes
neither a sponsor's decision nor the data agreement.
**Duplication:** overlaps COD-06's answerability/data-minimization workflow and COD-14's observability tests,
and materially packages planned BD-127–133/149–159, GATE-DATA-03 and GATE-TOUCH-04 rather than supplying new
reconciliation machinery; CC-15's event library does not validate the twin.

## CC-06 — The JV Mirror

**Score: 830 / 1000.**

Comparing the same asset across two public reporters is a particularly persuasive public-lane workflow for JV
controllers, auditors and agents, with a small positive first slice in attributable production and ownership
rounding (§3, §24.3, E457, E672).
However, separately audited filings may reuse the same operator measurements, so agreement is a reporting
cross-check rather than an independent physical oracle or evidence stronger than synthetic truth for its
different claim type (§33.1).
Cost, capex and reserve figures can have legitimate differences that public data cannot identify, and a partner's
previous filing is not a same-period pre-release comparator without a clearly matched reporting cut.
Begin with one verified asset-period pair, preserve both source lineages and the dated perimeter, and require
an independently cited bridge before saying “explained,” without promising recovered bridge parameters from
unidentified public inputs.
**Duplication:** substantial convergence with COD-07, with a valuable emphasis on independent public reporters
instead of authorized partner exports, extending §31.2 and BD-314; CC-02 supplies its arithmetic and CC-12
should not treat its asset-specific adjustments as universally transferable.

## CC-07 — The intertemporal-shift ledger

**Score: 450 / 1000.**

The remuneration committee genuinely needs protection against apparent AISC improvement bought by deferral,
and the proposal correctly connects this to §15.6, §23.2, §23.6 and RISK-22.
But spending below a hierarchical or LOM baseline does not identify deferred work, an economic debt or its
eventual repayment, while high processed grade can reflect planned sequencing, stockpiles and improved
selectivity rather than depletion of future value.
Calling the result a shift instead of intent does not cure that identification problem, and later spending or
grade reversal cannot by itself distinguish deferral from inflation, changed plans or new investment; an
attested project also does not automatically satisfy §21.6's sustaining classification test.
Reduce the public slice to named differences from a dated, definition-matched schedule and explicitly
conditional reversal forecasts, avoid causal dollar sub-bars or accumulated “debt” without evidence of the
obligation, and reserve mechanism attribution for an adequately observed M2 deployment.
**Duplication:** a material attempted extension of GB-26 and the planned Goodhart mitigations, overlapping
COD-30's KPI transformation tests and COD-16's attribution discipline; CC-01 already supplies the forecasting
ledger, so it needs no separate authoritative debt ledger and must not compete with FA.

## CC-08 — The fiscal mirror

**Score: 655 / 1000.**

An evidence-backed bridge between public operating royalties and reported cash payments could be valuable to
tax teams, state JV partners and government relations, and is more distinctive than another forward fiscal
scenario (§23.4, BD-228/296).
The proposed first slice depends on expressly unverified source granularity, while cash/accrual timing,
controlled-entity perimeter, taxable versus realized revenue, settlements and state dividends can make the
four views different flows rather than four measurements of one flow.
Without the relevant opening/closing balances and adjustments, an unexplained gap or effective-rate mismatch
cannot establish underpayment, validate the legal schedule or support regulatory compliance, even if a
synthetic regime makes those conclusions easy (INV-002, §21.3, §33.1).
Capture one public project-year with compatible categories and a fully cited cash-to-accrual bridge before
adding jurisdictions, keep rates descriptive or conditional on registered terms, and leave actual accounting
and tax positions with FA and authorized human reviewers.
**Duplication:** new historical cross-reporter reconciliation beyond the planned fiscal rows, adjacent to
COD-05's cash-access bridge but not the same workflow; considerable overlap with CC-06's pairing and CC-02's
constraint checks means this should be another small adapter rather than another engine.

## CC-09 — The guidance lifecycle

**Score: 385 / 1000.**

Checking site/group arithmetic and constructing forecasts from shared scenarios would help a CFO avoid
incompatible guidance, but the proposal's headline revision guarantee is not established by its mechanism.
Equal nominal coverage does not generally make quantile half-widths proportional to standard deviations, so
the implied-correlation formula requires stronger common-distribution assumptions, and a value above one is
not a distribution-free proof of incoherence.
Likewise, split-likelihood e-values do not automatically provide conditionally valid sequential increments for
reused quarterly nowcasts: multiplying the same mean-one e-value that is two or zero with equal probability
gives a product of mean two, illustrating why marginal validity is insufficient for the claimed guarantee
(compare the explicit conditional joint-law requirements in §10.4).
Ship deterministic sum/ratio diagnostics and model-conditional shared-scenario ranges, then require a specified
null family, filtration and proven sequential construction—or a valid finite-look alternative—before offering
an anytime-valid recommendation to revise guidance.
**Duplication:** mostly a forward extension of CC-01, NS-04, §10.4 and §23.4, with COD-01 overlap in remaining-year
feasibility; the small public history and unfinished M0 inputs make a separate guaranteed revision service
especially premature.

## CC-10 — The Target Disclosure Auditor

**Score: 760 / 1000.**

Exact intercept arithmetic and transparent cash-flow replay give BD and exploration teams useful, decision-linked
checks before opening a data room, and can demonstrate value entirely on public or synthetic documents.
The full TargetCard bundles several products whose inputs are not yet secured: omitted hole identifiers do
not establish selective reporting without the program universe, nested inclusions require overlap-aware
interval algebra, and repricing disclosed cash flows cannot recompute reserves or an undisclosed mine plan
(INV-014, §24.1, §24.7).
The proposal commendably identifies `brent_root` returning a value after exhausting iterations
(`fmine-numerics/src/lib.rs:149`), but IRR also needs an explicit policy for multiple roots and ambiguous
cash-flow sign changes, while inverse price/convention fits can have several feasible explanations.
Ship remainder-grade checks on explicit non-overlapping intervals and one independently captured developer
cash-flow replay, carry missing width/reporting-universe flags without accusations, and defer conversion
priors, buyer repricing and Fourmile valuation use until their identification and wall gates are satisfied
(§11.7, §23.5, §31.2).
**Duplication:** materially extends planned junior/M&A candidates and GB-31/32 rather than duplicating a COD
feature, but its resource arithmetic reuses CC-02 and its conversion priors depend on the much weaker CC-11;
§37 places the broader capital-allocation workflow at M4, not automatically M1.

## CC-11 — Conversion bounds

**Score: 410 / 1000.**

Recognizing that annual stock changes do not identify conversion flows is an important correction to how
OPEN-HELM-1 might otherwise be implemented, although §24.7 already explicitly acknowledges the public-data
decomposition gap.
The submitted flow equations do not yet bound a fraction of the opening cohort: with opening/closing Inferred
stocks 10/12 and Measured-plus-Indicated stocks 5/5, conversion f, Inferred additions 2 + f and M&I losses f
satisfy them for any nonnegative f, including f / 10 greater than one.
New additions converted during the year need cohort accounting, negative re-estimates and inclusive/exclusive
reserve semantics need complete treatment, and “no category both gains and loses” is a disjunctive restriction
rather than an ordinary LP constraint; synthetic recovery under the same restricted generator cannot establish
that those assumptions describe public issuers (§23.5, §24.3, §33.1).
Start with independently reviewed stock-flow semantics, constructive feasible histories for every claimed
endpoint and assumption-only bounds clearly separated from observations, and stop before pooling whenever
the identified set is vacuous or the target rate itself is undefined.
**Duplication:** a potentially useful material extension of §23.5, §24.7 and OPEN-HELM-1, with a partial-identification
family resemblance to COD-28 but a different target; it depends on CC-02 and should not silently supply CC-10
with precise Beta priors or expose walled Fourmile sensitivities (§11.7, G-HALL-5).

## CC-12 — The comparability engine

**Score: 705 / 1000.**

Comparisons that name their peer set and refuse incompatible bases would be useful to CFOs, IR and agents,
and bounded ranks are a practical improvement over a falsely precise cost league table (§21.6).
The weak point is transferring a JV asset's measured adjustment to another asset: corporate allocation,
joint-product economics and portfolio composition are not necessarily samples from one portable bridge
distribution, even when the transferred evidence class is displayed.
There is also a concrete error in the illustrative result: rank [2, 3] among four peers does not by itself
support “below the median,” because the issuer can occupy the third rank, so comparative predicates need
tests across the whole admitted set and an explicit tie convention.
Start with exact definition matches and one fully disclosed common-component transformation, propagate
shared uncertainty into the comparison, and permit no empirical bridge transfer until a meaningful target
population and enough independent held-out cases justify it.
**Duplication:** strong overlap with COD-09, materially extending planned BD-060/273 definition layers; comparative
wording belongs in CC-04 and same-asset bridge observations come from CC-06, so this should remain a small
Bullion/Portal workflow rather than a separate metric ontology.

## CC-13 — The sealed-answer agent benchmark

**Score: 605 / 1000.**

A fixed public-task regression set with citation checking would be useful for agents and AI procurement, and
future-release tasks offer a sensible test of generalization beyond familiar workbook layouts.
Hashing tasks before a release does not prove that an agent evaluated afterward was never trained on that
release: model version, evaluation time, allowed retrieval and answer-key handling must also be fixed, and
the current M0 pipeline cannot serve as the trusted answer generator (audit C001–C006, §33.2).
The +1/−k/0 rule has the stated selective-answer threshold, but is a decision utility rather than a strictly
proper probability score, and giving every refusal zero without measuring answerability and coverage can
favor an unhelpful abstention policy or fail to reward the very refusals the benchmark is meant to test.
Fold a small independently keyed set into GATE-ROBOT-01 first, distinguish correct refusal from avoidable
abstention, and make any contamination claim conditional on a frozen model/evaluation protocol rather than
the task hash alone.
**Duplication:** substantial extension of §30.4, GATE-ROBOT-01 and planned oracle/golden tests, with no direct COD
equivalent; it reuses CC-02's traps and CC-04's checker, and the maintenance burden includes human answer-key
review beyond the proposed roughly 1k LOC.

## CC-14 — The board delta pack

**Score: 520 / 1000.**

“What changed since the previous pack?” is an excellent board question, and expanding provenance changes into
a compact table is a credible later consumer of the ledger.
However, the novelty claim misses §24.7, which already specifies exact order-independent reserve changes from
depletion, model, deck, cut-off, conversion, acquisitions and ownership, and explicitly says public bridges
cannot reproduce that decomposition.
Shapley values add an allocation convention rather than causal identification, while mixing old/new recipes,
definitions and perimeters may produce undefined counterfactual states; refusing one factor cannot generally
leave an exact complete decomposition, and multiplicative Shapley allocations need not equal LMDI's.
Ship an anchored observed-delta and definition-drift view first, allow Shapley only for recipes whose entire
hybrid evaluation set is admissible, label its interaction allocation, and do not promise public reserve
deck-versus-geology attribution without the missing evidence (E143/E147 do not supply a block model).
**Duplication:** substantial duplication of §24.7 and GB-26, adjacent to COD-04's changed-evidence workflow and
COD-29's fixed-cohort bridge; the genuinely new increment is a general provenance-factor view plus a definition
diff, which should integrate with those existing consumers.

## CC-15 — The disclosed-disruption library

**Score: 320 / 1000.**

A cited catalog of disclosed outages could usefully define historical segments and retrieve precedents for a
site GM, but the proposed hazard and production-loss estimates are not identified by that catalog.
Issuer disclosures select events by severity and materiality, so quarters without a mention are not known
event-free exposure, pooling does not remove that selection, and two readers agreeing on a narrative cannot
measure the missing-event process.
LMDI can decompose a quarter's throughput/recovery change but cannot assign an event's causal share of it,
so labeling that share `Derived` does not make the attribution defensible and using it to validate CC-01,
CC-05 or CC-09 risks circulating the same untested assumptions (§21.6, §33.1).
Keep only cited event dates, stated effects and explicit unknowns as segmentation inputs, and require a
defensible reporting/detection model plus exposure data before calling reported-event frequencies operational
hazards or forecasting losses from them.
**Duplication:** new as a public disclosure catalog but adjacent to planned §16–§18 hazards and §37.3's existing
Lumwana event segmentation, with only loose overlap with COD-19's shared-dependency stress; its useful residue
is modest curated input data, not the proposed new risk engine.

## Ranked table

| Rank | ID | Idea | Score / 1000 | Judgment |
|---:|---|---|---:|---|
| 1 | CC-02 | Proof-carrying discrepancies | 875 | Strongest; narrow to exact linear certificates first |
| 2 | CC-06 | The JV Mirror | 830 | Strong public workflow; agreement is not independent physical truth |
| 3 | CC-04 | Disclosure Lint | 810 | Ship strict binding; do not certify unrestricted prose |
| 4 | CC-10 | The Target Disclosure Auditor | 760 | Valuable arithmetic/replay slice; defer the integrated valuation bundle |
| 5 | CC-01 | The Public Track Record | 730 | Ship descriptive vintages; small-n calibration promises need revision |
| 6 | CC-12 | The comparability engine | 705 | Useful bounded comparisons; bridge transfer needs evidence |
| 7 | CC-08 | The fiscal mirror | 655 | First prove a compatible public cash/accrual bridge |
| 8 | CC-05 | The Pilot Protocol Card | 625 | Useful sponsor artifact; simulated power is model-conditional |
| 9 | CC-13 | The sealed-answer agent benchmark | 605 | Fold into robot tests; repair contamination and refusal semantics |
| 10 | CC-03 | The Spreadsheet Lifter | 580 | Structural audit is credible; the proposed spreadsheet engine is oversized |
| 11 | CC-14 | The board delta pack | 520 | Significant planned overlap and unavailable hybrid evidence |
| 12 | CC-07 | The intertemporal-shift ledger | 450 | Baseline differences do not identify deferral debt |
| 13 | CC-11 | Conversion bounds | 410 | Valuable question; cohort/flow model is not yet sound |
| 14 | CC-09 | The guidance lifecycle | 385 | Useful diagnostics bundled with an unestablished sequential guarantee |
| 15 | CC-15 | The disclosed-disruption library | 320 | Keep event citations; public reporting does not identify hazards |
