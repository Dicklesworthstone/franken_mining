# Dueling Idea Wizards Report: FrankenMining (round 7, 2026-10-29)

| Field | Value |
|---|---|
| Agents | Claude Code (Opus 5.5), Codex (GPT-6.1-Sol, xhigh), Gemini (Gemini 4 Argon); NTM session `franken_mining--duel` |
| Scope | New features, functionality and supported workflows for FrankenMining, read against the plan, the M0 code at `68e7091` and the round-7 audit |
| Phases | Study → 30 ideas each, kept 5 → expanded to 15 → six cross-scorings (0–1000) → reveal, reactions and rebuttals → steelman of the rivals' first-ranked ideas → blind spots |
| Outcome | 20 capabilities adopted into the plan (Appendix K.5; beads BD-338 to BD-357), the rest merged, parked or dropped with reasons |
| Status | DESIGN and HYPOTHESIS. Nothing here is implemented, and no score is a probability |

## Executive summary

Forty-five ideas were scored by both rival models. Nine reached 700 or more from both rivals; three were killed;
the rest split. The three strongest conclusions:

1. **All three models' own first-ranked ideas were about the issuer's published guidance.** Claude proposed scoring
   it as a pre-registered forecast family (CC-01), Codex a deductive test of what must happen for it to be met
   (COD-01), Gemini a price-normalized credibility bridge (AGY-01). After steelmanning, every rival raised its score
   of both other first-ranked ideas: CC-01 to 790 and 905, COD-01 to 720 and 900, AGY-01 to 640 and 610. The
   insight that survived: securities filings are a timestamped forecast ledger that nobody, FrankenMining
   included, can backdate. That gives M0 something "new" to show finance reviewers without relying on
   FrankenMining's own unscored forecasts, and it uses only released documents, so it fits the pre-release
   quiet-period rule.
2. **Two pairs converged independently on testing the evidence chain itself.** Claude (CC-02) and Codex (COD-02)
   both proposed an exact rounding-aware consistency solver with checkable certificates; it is the consensus #1
   (mean 892 and 852). In the blind-spot round, Claude (B1) and Gemini (blind spot 1) both proposed per-value
   provenance tests: perturb each cited cell and delete each registered file, then check that the output moves or
   refuses. That is the direct answer to round 7's finding that the M0 slice emitted stand-in figures as Barrick's.
3. **Three blind spots recurred across models:** recall of issued outputs (Claude B3 and Codex blind spot 1), the
   compensation committee's exposure to metric definitions (Codex blind spot 2, Gemini's steelman, the round-7
   reader table), and insurance recovery evidence (Gemini blind spot 5 and Codex blind spot 3).

## Consensus winners (both rivals ≥ 700)

| Rank | Idea | Rival scores | Mean | Plan |
|---:|---|---|---:|---|
| 1 | CC-02 Proof-carrying discrepancies | COD 875, AGY 910 | 892 | BD-340, with COD-02 |
| 2 | COD-02 Rounding-aware contradiction finder | CC 830, AGY 875 | 852 | BD-340 |
| 3 | CC-06 JV Mirror | COD 830, AGY 850 | 840 | BD-346 |
| 4 | CC-04 Disclosure Lint | COD 810, AGY 810 | 810 | BD-345 |
| 5 | CC-01 Public Track Record | COD 730 (790 after steelman), AGY 865 (905) | 798 | BD-341 |
| 6 | CC-10 Target Disclosure Auditor | COD 760, AGY 785 | 772 | BD-350 |
| 7 | AGY-15 CounterfactualHindsightGuard | CC 720, COD 790 | 755 | BD-344 |
| 8 | COD-08 Forecast cohorts and outcome vintages | CC 700, AGY 735 | 718 | BD-343 |
| 9 | CC-12 Comparability engine | COD 705, AGY 705 | 705 | BD-355 |

What the rivals required of each before adoption (all carried into the beads):
- **CC-02 / COD-02.** Linear identities first; certificates verified in exact rationals; conflict sets called
  inclusion-minimal unless the exact search ran; bilinear identities only as refutation; the ±1-digit rule kept
  beside it until the solver proves it finds more.
- **CC-06.** Agreement between two reporters is not physical truth; start with metrics untouched by purchase-price
  allocation; reconcile at the coarsest common perimeter.
- **CC-04.** Cited mode only; a named list of supported predicates; all other prose labelled unchecked. B7 adds
  that the lint must check value and basis, not the presence of a link (E696).
- **CC-01.** No recalibration at n = 3 years; descriptive scores with honest intervals; transcription by two
  readers; delivered privately to the sponsor first.
- **AGY-15.** Add the data-cone rule and split observation time from policy time; narrow "proof" to the checks
  actually run.

## Contested ideas (rival gap ≥ 200, or one rival below 600)

| Idea | Low side | High side | Resolution |
|---|---|---|---|
| CC-03 Spreadsheet Lifter (580, 895) | Public workbooks hold almost no formulas; the engine is oversized | Formula graphs are where site finance lives | Author narrowed it to a structural audit when site workbooks arrive (M2) |
| COD-01 Guidance completion test (610, 860) | Public output is mostly subtraction | Ratio AISC, sustaining-capital floors and envelope tiers are not subtraction; n = 1 deduction where statistics have no power | Steelmen raised both scores (720, 900); adopted as BD-342 |
| CC-07 Intertemporal-shift ledger (450, 825) | Baseline deviations do not identify deferral | Remuneration committees need it | Parked; author narrowed it to a schedule-gap ledger |
| CC-11 Conversion bounds (410, 775) | The cohort-flow formulation is unsound | An honest partial-identification question | Parked until the formulation is repaired |
| CC-09 Guidance lifecycle (385, 740) | The sequential guarantee is not established | Fast coherence diagnostics | Diagnostics merged into BD-341 |
| COD-28 Untested-ore bounds (590, 815) | Site-gated | Right mathematics for a real metallurgical decision | Adopted for M2 (BD-351) |
| COD-03 Evidence-origin accounting (620, 805) | Partly duplicates domain bias models | A real laundering guard | Adopted narrowly (BD-352) |
| AGY-06 Offtake optimizer (380, 675) | Confidential inputs; the subledger belongs to FA | Real copper-division money | Parked |
| AGY-12 Geomet bridge (550, 735) | Its generic greedy guarantee is false | Correct critique of BD-300 | Critique folded into BD-300 |
| AGY-04 Block-model forensics (450, 340) | Wrong Shapley identity; classification-like labels | Author defends with a sign fix (rebuttal, 88% confidence) | Parked: both rivals below 500 |

## Killed

- **AGY-08 CryptographicDataRoom** (400, 180): commitments prove neither source completeness nor zero leakage; a
  plaintext Merkle sum tree leaks sibling sums. The author conceded (revised self-score 310).
- **AGY-14 Stope sequence stress and fill coupler** (300, 260): the anti-pillar cut forbids primary–secondary
  stoping and the point-dipole stress model is unsupported. Only the two-sided paste-strength window survives, in
  BD-307.
- **AGY-03 Mine-to-mill shadow-price loop** (340, 300): an unproven chance-constraint envelope and operational
  precision the data cannot support; deferred to M2–M3 at best.
- Dropped by their own authors after the reveal: CC-15 as an idea (a cited fixture of quantified events stays),
  COD-05, COD-07, COD-09, COD-14, COD-16, COD-17 and COD-26 as standalone features (each folded into an existing
  bead or section).

## Score matrix

Rival A and Rival B are the two other models' scores; "Revised self" is the author's own score after the reveal.
Sorted by the rivals' mean.

| Idea | Title | Own rank | Rival A | Rival B | Mean | Gap | Revised self | Verdict |
|---|---|---:|---:|---:|---:|---:|---:|---|
| CC-02 | Proof-carrying discrepancies | 2 | COD 875 | AGY 910 | 892 | 35 | 850 | Consensus #1 → BD-340 |
| COD-02 | Rounding-aware contradiction finder | 2 | CC 830 | AGY 875 | 852 | 45 | 860 | Consensus; converges with CC-02 → BD-340 |
| CC-06 | JV Mirror | 6 | COD 830 | AGY 850 | 840 | 20 | 800 | Consensus → BD-346 |
| CC-04 | Disclosure Lint | 4 | COD 810 | AGY 810 | 810 | 0 | 790 | Consensus → BD-345 |
| CC-01 | Public Track Record | 1 | COD 730 | AGY 865 | 798 | 135 | 760 | Consensus; steelmen raised it to 790 and 905 → BD-341 |
| CC-10 | Target Disclosure Auditor | 10 | COD 760 | AGY 785 | 772 | 25 | 740 | Consensus → BD-350 |
| AGY-15 | CounterfactualHindsightGuard | 15 | CC 720 | COD 790 | 755 | 70 | 930 | Consensus → BD-344 |
| CC-03 | Spreadsheet Lifter | 3 | COD 580 | AGY 895 | 738 | 315 | 540 | Contested; narrowed to an M2 structural audit |
| COD-01 | Guidance completion test | 1 | CC 610 | AGY 860 | 735 | 250 | 715 | Contested; steelmen raised it to 720 and 900 → BD-342 |
| COD-08 | Forecast cohorts and outcome vintages | 7 | CC 700 | AGY 735 | 718 | 35 | 790 | Consensus → BD-343 |
| COD-03 | Independent-evidence accounting | 3 | CC 620 | AGY 805 | 712 | 185 | 735 | Contested → BD-352 |
| CC-12 | Comparability engine | 12 | COD 705 | AGY 705 | 705 | 0 | 640 | Consensus (marginal) → BD-355 |
| CC-08 | Fiscal mirror | 8 | COD 655 | AGY 750 | 702 | 95 | 600 | Parked to M2 (one project-year) |
| COD-28 | Untested-ore decision bounds | 8 | CC 590 | AGY 815 | 702 | 225 | 710 | Contested → BD-351 |
| CC-05 | Pilot Protocol Card | 5 | COD 625 | AGY 765 | 695 | 140 | 680 | Near-consensus → BD-356 |
| COD-04 | Decision recall | 4 | CC 580 | AGY 770 | 675 | 190 | 690 | Merged into BD-347 |
| COD-06 | Answerability before sharing | 6 | CC 530 | AGY 755 | 642 | 225 | 610 | Merged with recipes |
| AGY-12 | CompositionalGeometBridge | 12 | CC 550 | COD 735 | 642 | 185 | 850 | Contested; critique folded into BD-300 |
| CC-07 | Intertemporal-shift ledger | 7 | COD 450 | AGY 825 | 638 | 375 | 630 | Contested; parked |
| CC-13 | Sealed-answer agent benchmark | 13 | COD 605 | AGY 660 | 632 | 55 | 520 | Folded into GATE-ROBOT-01 |
| CC-14 | Board delta pack | 14 | COD 520 | AGY 720 | 620 | 200 | 480 | Parked (planned overlap) |
| COD-05 | Metal to spendable cash | 5 | CC 475 | AGY 745 | 610 | 270 | 595 | Dropped standalone; into BD-314 |
| COD-10 | Bound numerical sentences | 10 | CC 520 | AGY 695 | 608 | 175 | 695 | Merged into BD-345 |
| AGY-05 | AgentSandboxAndProseVerifier | 5 | CC 570 | COD 640 | 605 | 70 | 830 | verify-prose merged into BD-345 |
| AGY-07 | StreamingRoyaltyTheoremProver | 7 | CC 560 | COD 650 | 605 | 90 | 835 | Folded into BD-314 |
| COD-07 | JV disagreement localization | 9 | CC 455 | AGY 750 | 602 | 295 | 575 | Dropped standalone; public mirror first |
| COD-09 | Comparability frontier | 14 | CC 500 | AGY 690 | 595 | 190 | 600 | Into BD-355 |
| CC-11 | Conversion bounds | 11 | COD 410 | AGY 775 | 592 | 365 | 450 | Contested; formulation to repair |
| COD-17 | Information's last useful date | 13 | CC 465 | AGY 715 | 590 | 250 | 625 | Into BD-227 |
| COD-14 | Measurement-outage observability | 12 | CC 440 | AGY 730 | 585 | 290 | 580 | Into BD-153 |
| AGY-02 | WorkbookForensicAutopsy | 2 | CC 620 | COD 545 | 582 | 75 | 780 | Narrowed; merged into BD-271 |
| COD-26 | Evidence-family withdrawal stress | 11 | CC 445 | AGY 710 | 578 | 265 | 605 | A §22 robustness case |
| COD-16 | Action-aware forecasts | 15 | CC 480 | AGY 670 | 575 | 190 | 590 | Into BD-343 |
| CC-09 | Guidance lifecycle | 9 | COD 385 | AGY 740 | 562 | 355 | 420 | Contested; merged into BD-341 |
| AGY-13 | PermitAndCommitmentCriticalPathCompiler | 13 | CC 500 | COD 595 | 548 | 95 | 740 | Parked to M3+ |
| AGY-06 | SmelterOfftakeOptimizer | 6 | CC 380 | COD 675 | 528 | 295 | 810 | Contested; parked (confidential inputs) |
| AGY-01 | GuidanceCredibilityBridge | 1 | CC 540 | COD 430 | 485 | 110 | 840 | Contested; steelmen raised it to 640 and 610; narrowed into BD-342 |
| AGY-10 | DrillholeReferee | 10 | CC 350 | COD 620 | 485 | 270 | 720 | Contested; parked |
| AGY-11 | StructuralAdversary | 11 | CC 440 | COD 525 | 482 | 85 | 700 | Narrowed into Strata epics |
| CC-15 | Disclosed-disruption library | 15 | COD 320 | AGY 620 | 470 | 300 | 330 | Dropped by author; cited fixture kept |
| AGY-09 | StockpileSegregationAndReclaimPhysics | 9 | CC 320 | COD 560 | 440 | 240 | 590 | One candidate operator in BD-312 |
| AGY-04 | RpeeeAndBlockModelForensics | 4 | CC 450 | COD 340 | 395 | 110 | 820 | Both rivals low; author defends; parked |
| AGY-03 | MineToMillShadowPriceLoop | 3 | CC 340 | COD 300 | 320 | 40 | 580 | Killed for now (both rivals < 350) |
| AGY-08 | CryptographicDataRoom | 8 | CC 400 | COD 180 | 290 | 220 | 310 | Killed (author concedes) |
| AGY-14 | UndergroundStopeSequenceStressAndFillCoupler | 14 | CC 300 | COD 260 | 280 | 40 | 520 | Killed; paste window kept in BD-307 |

## Steelman results

| Steelman by | Of | Before | After | What changed the scorer's mind |
|---|---|---:|---:|---|
| Claude | COD-01 | 610 | 720 | The joint requirement (production with ratio AISC) is not visible by subtraction |
| Claude | AGY-01 | 540 | 640 | A narrowed bridge over the publicly identified price and royalty factors is useful |
| Codex | CC-01 | 730 | 790 | An auditable history of commitments has value without proving forecasting skill |
| Codex | AGY-01 | 430 | 610 | A common-basis guidance review is useful with partial identification |
| Gemini | CC-01 | 865 | 905 | It fills the calibration ledger on day 1 from timestamped filings and fits the quiet-period rule |
| Gemini | COD-01 | 860 | 900 | Deductive conflict proofs need no distributional prior; it maps to the MD&A duty to discuss changes in previously disclosed forward-looking information (NI 51-102 s.5.8; FACT[U], to verify) |

## Blind spots (not cross-scored; scored here by the orchestrator, with extra scrutiny)

| Blind spot | Proposed by | Self | Orchestrator | Plan |
|---|---|---:|---:|---|
| Per-value live-dependency proof and source byte witness | Gemini 1, Claude B1 (independent) | 945, 760 | 850 | BD-338 |
| Reproduction kit with no source bytes in git | Gemini 1 part C | — | 720 | BD-339 |
| Recall and reliance status of issued outputs; outbound register | Codex 1, Claude B3 (independent) | 810, 590 | 720 | BD-347 |
| Non-interference check for outputs crossing the wall; disclosure calendar | Gemini 2 | 915 | 680 | BD-348 (M2) |
| Is this number already public, and since when? | Claude B2 | 640 | 650 | Folded into BD-345 |
| Compensation-formula exposure to metric definitions | Codex 2 | 740 | 650 | BD-353 |
| Salted review: measure whether sign-off catches errors | Claude B4 | 540 | 620 | BD-357 (internal reviews only) |
| Insurance recovery and business-interruption evidence | Gemini 5, Codex 3 (independent) | 865, 620 | 560 | BD-354 (M3) |
| Statutory mining workpapers (IFRIC 20, IAS 2, IAS 36, IAS 37) | Gemini 3 | 890 | 520 | Parked to M3 with FA |
| Project-finance completion tests and covenants | Gemini 4 | 880 | 450 | Parked: agreements are confidential |

Orchestrator scrutiny. The non-interference check is sound only for deterministic outputs; an agent's prose is
not covered and must pass the lint. Planted errors in a review packet must never reach an issuer insider as the
issuer's own figure (RISK-32), so salted review starts with internal reviews. Gemini's statutory-workpaper and
lender ideas assume data (per-pushback stripping ratios, credit agreements) that the public lane does not hold.

## Meta-analysis

- **Judge severity.** Mean score given: Claude 509 (COD 549, AGY 469), Codex 570 (CC 617, AGY 523), Gemini 767
  (CC 778, COD 755). Claude was the harshest judge and Gemini the most lenient; Gemini's scores separate less.
- **Self-assessment.** Mean revised self-score against mean received: Claude 615 against 698 (rates its own ideas
  lower than its rivals do), Codex 665 against 652 (calibrated), Gemini 724 against 496 (+228, the largest
  self-inflation; its idea ranked 15th by itself, AGY-15, became its best-received).
- **What each model favoured.** Claude: public-data products that run at M0 on files on disk. Codex: inverse and
  identification problems, with careful claim language. Gemini: broad mathematical machinery across many
  subsystems, which drew most of the kills (wrong identities, unsupported guarantees).
- **Where adversarial pressure helped.** Every consensus winner came back narrower and more testable than it went
  in. The rebuttals produced concessions rather than entrenchment, apart from AGY-04.
- **What the steelman round revealed.** Hidden value lay in *who can use* an idea before it is statistically
  mature: guidance scoring and completion tests help the M0 review and the disclosure committee long before any
  calibration claim is possible.

## Recommended next steps (carried into the plan, Appendix K.5)

1. M0: BD-338 (live-dependency proof) and BD-339 (reproduction kit) with the pre-push corrections; BD-340's linear
   slice; the guidance transcription for BD-341 as a target.
2. M1: BD-341 scores, BD-342, BD-343, BD-344, BD-345, BD-346, BD-347, each judged by its K.5 test and kill.
3. M2 onward: BD-348 with GATE-DATA-01; BD-351; BD-354 only after the sponsor gate.
