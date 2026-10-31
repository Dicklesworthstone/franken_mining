# WIZARD_REBUTTAL_CC: two hills defended, two ideas attacked

| Field | Value |
|---|---|
| Author | Claude (Opus 5.5), dueling idea wizards, rebuttal round |
| Date | 2026-10-29 |
| Basis | `WIZARD_SCORES_COD_ON_CC.md`, `WIZARD_SCORES_AGY_ON_CC.md`, `WIZARD_SCORES_COD_ON_AGY.md`, my reactions (`WIZARD_REACTIONS_CC.md`), the plan, the evidence register |
| Ids | CC-nn = my ideas; AGY-nn = Gemini's; COD = Codex |

## How the four were chosen

- **Hills.** Among my ideas that still score at least 600 after my own revision, these are the two with the largest
  gap between my revised score and the lower reviewer score:
  - CC-07: COD 450, my revision 630;
  - CC-05: COD 625, my revision 680.

  I did not pick CC-09, CC-11 or CC-15, though the reviewers scored them lower. Their critiques hold, and defending
  them would be advocacy, not argument (reactions §3–§4).
- **Attacks.** The two ideas from the other lists that I scored lowest:
  - AGY-14, scored 300;
  - AGY-9, scored 320.

  Both happen to be Gemini's. Codex's lowest in my scoring was COD-14, at 440.
- **Bias warning.** I proposed the two hills, and authors over-defend their own ideas. The confidences below are
  meant to be checked against outcomes, not taken on trust.

---

## Hill 1. CC-07, the intertemporal-shift ledger, narrowed to a schedule-gap ledger

### The claim

- **COD (450):** gaps against a baseline do not identify deferral, debt or repayment; grade above plan can be
  planned sequencing; later reversal cannot tell deferral from inflation or a changed plan.
- **AGY (825):** valuable, but pushback phasing makes deviations normal, and 8–10 quarters cannot support a
  statistical baseline.
- **My position.**
  - Both are right about the package as submitted: a hierarchical baseline, a "debt" in $M, and a $/oz effect read
    as deferral.
  - A narrower instrument survives both objections and is worth building at M1. It measures each site's gaps
    against its own **dated, filed, definition-matched annual schedule**. Each gap carries a forecast that is
    conditional and registered.
  - The dispute is whether that narrower instrument is worth about 630, or 450.

### The evidence

1. **The user and the measurement question are documented.**
   - FACT (E605): gold AISC carries 18.75% and copper AISC 6.25% of Barrick's 2025 company scorecard weight. The
     scored gold AISC was $1,423/oz.
   - FACT (E607): reported FY2025 AISC was $1,637/oz, above guidance because of price-driven royalties. So the
     scorecard metric is already an adjusted AISC, and the remuneration committee decides on it.
   - FACT (plan RISK-22, a generic Goodhart risk): "Goodhart: a metric used in pay can drift from the quantity it
     measures (AISC deferral …)". The named mitigation is "deferral detection (GB-26)".
   - FACT (§15.6, §23.2, §23.6): the plan names high-grading, deferral and reclassification as ways to lower AISC
     while destroying NPV.
   - GB-26 shows a deferral only when someone has already labelled it. The plan has committed to a mitigation it has
     no instrument for.
2. **A gap against a filed schedule is an observation, not an inference of intent.**
   - Example: "2025 waste mined was W Mt below the technical report's 2025 schedule, and processed grade was r × the
     scheduled grade".
   - That statement needs no baseline model, and so no identification. It is INFERENCE from two cited FACTs.
   - COD's objection applies to the next step, reading the gap as deferral. The narrowed ledger does not take that
     step. Its wording is "below schedule", never "deferred".
   - AGY's fix (year-by-year LOM or refuse) and COD's fix ("named differences from a dated, definition-matched
     schedule") describe the same instrument. Two reviewers reaching the same narrowing independently is evidence
     the narrowed form is the sound one.
3. **The definitional bridge is the hard part, and neither reviewer's list has it.**
   - INFERENCE (E196, E197; 100% basis): Carlin's technical-report "sustaining" line is about $67 per recovered
     ounce. With capital drilling, stripping and underground development included (all counted as sustaining under
     WGC AISC, §21.6), it is about $188/oz.
   - A naive comparison of reported AISC capital with a TR schedule would flag every quarter by roughly $120/oz. The
     ledger's first output, the bridge itself, is useful to the controller even if no gap is ever flagged.
4. **COD's reversal objection becomes a resolution rule, not a fatal flaw.**
   - Each gap carries a forecast conditional on the schedule standing: "if the schedule is unchanged, cumulative
     waste in years t+1 … t+k exceeds schedule by ≥ W".
   - A new technical report, plan revision or perimeter change resolves the forecast as `PlanChanged`. It is never
     scored as deferral or as efficiency.
   - What gets scored is whether gaps against an unchanged schedule close. That is an ordinary Reconciliation claim
     (§33.1). It cannot prove intent, and it does not claim to.
5. **I concede the parts that fail:**
   - the "debt" in $M;
   - the hierarchical and trailing sustaining baseline (8–10 quarters; r7 C118);
   - the $/oz effect bar;
   - any dollar figure for reclassification, which is reduced to a flag on "an unregistered shift in sustaining
     share".
6. **Coverage will be partial, and the ledger says so.**
   - Annual schedules exist for some sites: Carlin (E196, E197), Kibali (C3, p336–337) and Loulo (E249).
   - Some producing-asset reports omit the economic analysis (C4 §5 nulls). Those sites are refused, not estimated.

### The verdict

- **COD's 450 is right for the package as submitted, and too low for the narrowed ledger.** I put the narrowed
  version at **630**:
  - a cited gap table per site-year for sites with annual schedules;
  - the WGC-to-TR definitional bridge;
  - conditional forecasts with a `PlanChanged` resolution.
- **Confidence that the narrowed ledger is worth building at M1 for at least two sites: 65%.**
- **Confidence that COD is right that the submitted causal version should not be built: 85%.**
- **What would change my mind:** fewer than two Barrick sites with a filed annual schedule, or a remuneration
  committee that has no use for "below schedule" without causal attribution. The first is checkable in the week-1
  data gate.

---

## Hill 2. CC-05, the Pilot Protocol Card

### The claim

- **The reviewers' view.** Both reviewers say power computed on a twin fitted to Lumwana's public quarterly
  aggregates does not identify sampler-error covariance, autocorrelation or ore-type kinetics, so it is not power
  for Lumwana (COD 625, AGY 765). COD adds that my fallback, keeping the data-subset ranking after experts reject
  the twin, is unsafe.
- **My position.**
  - Both points are right, and I adopt both fixes.
  - But the reviewers scored the card as a power estimator. Most of its decision value does not depend on the
    absolute power numbers.
  - It is the only idea in all three lists aimed squarely at the plan's binding constraint, and it is worth about
    680.

### The evidence

1. **The constraint is real and documented.**
   - FACT (E599): P(site data by month 7) ≈ 0.3.
   - BET-06's falsifier is "M0–M1 artifacts do not move the data-access decision".
   - The plan's critical path runs through the pilot (§39).
   - Pre-mortem F2: security and legal review, not modelling, is what stalls data access.
   - Codex independently proposed an answer to the same constraint (COD-06, answerability-first data offers). Two
     models converging on the target is evidence the target matters.
2. **Three of the card's five parts do not depend on how accurate the twin is.**
   - **(a) The full analysis plan is hashed before the data arrive.** It covers the finding definitions, the
     materiality threshold, the code revision, the seeds, the stopping rule and what counts as a null. The plan
     hashes forecasts and the threshold (GATE-TOUCH-04), not the analysis plan. Hashing the plan is what makes a
     later QP attestation that a finding is "new" credible.
   - **(b) A justification for each requested file.** This is the form IT security and counsel review in (F2).
   - **(c) An explicit early kill.** If no feasible subset has power under any credible twin, switch to the
     alternate site at month 1 instead of month 12.
3. **The envelope answers the identification objection with the fix both reviewers proposed.**
   - Fit K twins that agree on the public aggregates and differ in what those aggregates cannot identify:
     - sampler-error covariance;
     - intra-quarter autocorrelation;
     - transitional-ore kinetics.
   - Request the files that rank in the top set under every twin. Mark the files whose rank depends on the twin as
     **model-dependent**, to be decided with the site.
   - No absolute power is stated without its envelope. My illustrative "82%" and "less than 2 points" go.
   - The "keep the ranking" fallback is dropped. A structural rejection ends the card.
4. **The status quo is no better identified.**
   - Today GATE-DATA-03's list goes to the site with no per-file reasons.
   - Per AGY (r7 C138), its definition also conflicts with §37.3's recovery-capable subset.
   - The card is where that conflict gets resolved in writing before a sponsor sees it.
5. **The test is cheap and decisive.** Known answers: on sealed-truth replicates (GATE-GEN-01), the claimed detection
   rate per subset must match the empirical rate across the envelope. Retirement: after the site decision, if the
   card changed neither the data agreement nor the site choice (BET-06).

### The verdict

- **COD's 625 undervalues the card, because it scores the power arithmetic and not the decision the card serves.**
  With the envelope and without the fallback, I put it at **680**.
- **Confidence that the narrowed card is worth building at M1: 60%.**
- **Confidence that COD is right about the fallback: 90%.**
- **What would change my mind:** an envelope so wide that no file is robust, so every request is model-dependent.
  The card then reduces to the hashed analysis plan, which is worth about 450 alone.

---

## Attack 1. AGY-14, the underground stope-sequence stress and fill coupler

### The claim

- **The scores.** I scored AGY-14 at 300 and Codex at 260.
- **My position.** It should be rejected as an optimizer:
  - its central cut contradicts the plan's own stoping design;
  - its stress model cannot support the claims made for it;
  - its one valuable piece is already in the plan.

### The evidence

1. **The plan already has the two-sided fill window.**
   - FACT (§16.3, G-HEAD-7): "A secondary or tertiary stope that exposes a wall of filled primary stope gets no
     ring-charge plan or extraction step if the fill's `FillStrength(t)` is below `UCS_design` … `FillStrength(t)`
     covers early-age cure and late-age binder loss from sulphate attack."
   - FACT (§16.3): "Cure time to the required strength is a precedence constraint in the stope sequence (§15)".
   - So both the lower bound (cure) and the upper bound (sulphate loss, E233, E234) are planned. The only increment
     is putting the upper bound into the scheduler as a constraint, not only as a refusal. That is an acceptance case
     on BD-307, not an idea.
2. **The anti-pillar cut forbids the method the plan is built around.**
   - The cut x_{i−1,t} + x_{i+1,t} − x_{i,t} ≤ 1 has no fill term, so it bans every secondary stope between two
     mined primaries.
   - G-HEAD-7's premise is exactly that: secondaries mined against filled primaries.
   - The 1-3-5-then-2-4 schedule AGY calls "naive" is primary–secondary stoping, the standard sequence for filled
     stopes.
   - Real diminishing-pillar risk (closing sill or crown pillars in high stress) depends on geometry and stress
     state. A topological cut cannot detect it.
3. **AGY's description of the plan is wrong.** AGY says §14.3/§15.5 treat stability and dilution as "static
   constants independent of the extraction order". FACT (§16.3): planned dilution is a
   `DilutionMatrix(stope_type × sequence × wall × n_exposures)`, with Kibali planning 2.0–2.5% per paste wall
   exposed (E235).
4. **The stress model cannot carry the claims.**
   - A V/(4πr³) elastic superposition kernel ignores yield, fill stiffness, stope shape and the stress path.
   - The parity test, "Kirsch/Eshelby within 2%", checks a single opening. It does not check interacting openings,
     which are the whole claim.
   - Codex makes the same point independently. §14.2 records that 3D FEM is not available (E007).
5. **One premise has no evidence.**
   - τ_max = ∞ for a 5% sulphate-resistant binder has no support. E233 covers Loulo's 3.5% paste, and Kibali's
     slag-cement fill has degraded (E234).
   - Codex raised this; I missed it in my own scoring.
6. **It crosses an authority boundary.** Stress limits and stope sequence are rock-mechanics sign-offs (INV-002,
   `geotechnical_signoff`). Hard-coding them as MILP cuts presents engineering judgment as model output.
7. **The reviewers converge.** Two independent scorers put it at 260 and 300, and both name the primary–secondary
   conflict.

### The verdict

- **Reject AGY-14.** Keep only the late-age upper bound as a scheduler constraint in BD-307, with an engineer-approved
  `FillStrength(t)` card per binder and no infinite windows.
- **I lower my score from 300 to 250**, because §16.3 already holds the part I credited.
- **Confidence: 90%.**

---

## Attack 2. AGY-9, stockpile segregation and reclaim physics

### The claim

- **The scores.** I scored AGY-9 at 320, Codex at 560.
- **My position.**
  - It adds one candidate operator to a framework the plan already specifies, and that operator's parameters cannot
    be identified.
  - Its evidence is misquoted or miscited.
  - Codex's 560 credits a conservation invariant and a goal (separating timing from bias) that BD-312 already holds.
  - It is worth at most about 350 before M2.

### The evidence

1. **The plan already has most of it.** FACT (§20.8):
   - `StockpileVoxelTwin` places each dump "with its source polygon and angle-of-repose spreading";
   - each reclaim cut "removes voxels and passes their parcel mix and age to the crusher feed";
   - each voxel integrates oxidation exposure;
   - without dump and reclaim positions, declared mixing models (well-mixed, layered/FIFO, campaign-separated) are
     compared on predictive performance, "and the voxel model is one more candidate".

   AGY's premise that BD-312 holds "static 3D voxels" is wrong. Its increment is a Beta deposition kernel per size
   class along the rill, plus a kinematic draw cone: one more candidate.
2. **Its evidence is misquoted or miscited.**
   - **E666.** FACT (§20.8): Pueblo Viejo's cone opens at 281.7 kt at 2.41 g/t and closes at 352.1 kt at 2.44 g/t.
     INFERENCE: a 70 kt swing and about 5.8 koz contained. AGY writes "61 kt / ~50 koz", overstating the metal about
     ninefold.
   - **R7.** AGY labels its segregation mechanism and its 1.3–2.5× enrichment of fines "FACT, R7 §6–§7". FACT: R7
     contains no passage on rill segregation, draw cones or the enrichment of fines. Its only size-related method is
     screen fire assay for coarse gold (R7 §3). There the coarse fraction carries the gold, the opposite direction
     to AGY's assumption.
3. **The parameters cannot be identified from what sites measure.**
   - The model needs per-class Beta shapes (α_c, β_c), a draw-cone diffusivity D_k and size-by-grade enrichment
     η_{c,m}.
   - Its observation channel is coarse. FACT (§20.8, E194): Cortez sonic-drills piles on a 15–18 m grid and scores
     each pile annually.
   - INFERENCE: a 30 m pile at a 37° angle of repose has a rill about 40 m across in plan. A 15–18 m grid puts two
     or three holes across it, too few to fit a Beta shape per size class.
   - Annual scoring cannot resolve a 12-hour reclaim sequence at all.
   - So the posterior is essentially the prior.
4. **Its known-answer test is circular.** "Cuts F2 attribution error by > 70%" is measured on a generator built with
   the same physics. Per §33.1 and AGENTS.md's four claims, synthetic recovery does not show that a real pile
   behaves like the generator.
5. **It overclaims precision.** It promises "exact" 12-hour grade and F80 predictions. Exact mass and metal
   conservation (`ExactQty`) is necessary for a stockpile operator, not evidence that its spatial allocation is
   right.
6. **The steelman, from Codex's 560.**
   - The case for it: separating stockpile timing transients from F2 bias matters to reconciliation, and a
     conservation-checked operator is a sound start.
   - I agree, and that is what §20.8's mixing-model comparison already does. Codex's own recommended fix is to add
     "one mass/metal-conserving candidate operator … compare it with homogeneous/FIFO candidates … and require M2
     measured dump/reclaim and size-by-grade evidence".
   - That fix concedes the point: the idea is a candidate, gated on M2 data.

### The verdict

- **AGY-9 should not be built before M2**, and never as a default mixing model.
- **Once a site supplies dump and reclaim positions plus size-by-grade assays**, it enters BD-312's comparison as one
  candidate and earns weight only through predictive performance.
- **Score: 320, unchanged.** Codex's 560 overstates the increment.
- **Confidence: 75%.** It is lower than for AGY-14 because a careful reviewer scored this one far higher, and our
  difference is mostly about how much credit a planned invariant deserves.

---

## Summary

| # | Idea | Disagreement | Scores (COD / AGY / mine) | Final position | Confidence |
|---|---|---|---|---|---|
| Hill 1 | CC-07 schedule-gap ledger | Is a narrowed, non-causal ledger worth building? | 450 / 825 / 630 | Yes, for sites with filed annual schedules; drop the debt and statistical baselines | 65% |
| Hill 2 | CC-05 Pilot Protocol Card | Does model-conditional power sink the card? | 625 / 765 / 680 | No: the hashed analysis plan, per-file reasons and early kill survive; power only as an envelope | 60% |
| Attack 1 | AGY-14 stope sequencer | Is it a planning tool or a contradiction of the plan? | 260 / — / 250 | Reject; keep only the late-age bound inside BD-307 | 90% |
| Attack 2 | AGY-9 stockpile physics | Is it a new twin or one unidentifiable candidate? | 560 / — / 320 | One candidate in BD-312 after M2 data; misquoted evidence | 75% |
