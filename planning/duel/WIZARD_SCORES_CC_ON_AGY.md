# WIZARD_SCORES_CC_ON_AGY: Claude's scores for Gemini's fifteen ideas

| Field | Value |
|---|---|
| Scorer | Claude (Opus 5.5), dueling idea wizards, scoring round |
| Date | 2026-10-29 |
| Files scored | `planning/duel/WIZARD_IDEAS_AGY.md` (ideas 1–5, plus the 30-title appendix) and `planning/duel/WIZARD_IDEAS_AGY_6_15.md` (ideas 6–15), both read in full |
| Basis | The plan at `68e7091`; current crate code; the evidence register (plan text, `planning/research/W5`, `R6`); the corpus workbooks (formula elements counted, no content quoted); the Bottom Line of `planning/mor/r7/MODES_OF_REASONING_REPORT_AND_ANALYSIS_OF_PROJECT.md` |
| Ids used here | AGY-1 … AGY-15, in Gemini's own rank order |

## How I scored

**Scale.** The same scale as my scores of Codex's ideas, so the two files can be compared.

| Score | Meaning |
|---|---|
| 800–1000 | Build now. Correct, testable in the public lane before M2, and clearly worth its cost |
| 650–799 | Strong. Build in M0–M1, or fold into a planned bead as its acceptance criterion |
| 500–649 | Worthwhile, but gated on site data, a sponsor or a fix, or largely an extension of planned work |
| 300–499 | Marginal: mostly duplicates planned machinery, cannot be tested before M2, has a design error at its core, or costs more than it returns |
| < 300 | Reject |

**The M0 condition.**
- FACT (r7 Bottom Line; C001, C011, C059): `fmine assure public --issuer barrick` substitutes an in-code workbook and
  emits wrong figures labelled as Barrick's (C002–C006, C159).
- No idea gets credit for a first slice that runs inside `run_barrick_m0_assurance` or assumes the importer reads the
  real workbooks. That affects AGY-1, AGY-2 and AGY-5.

**What I checked.** Where a score depends on a factual claim, I checked it against code, plan text or evidence:
- the corpus workbooks' formula elements (AGY-2);
- the reader's formula field (AGY-2);
- the cited E-ids (AGY-1, AGY-9);
- the plan's effect types and Appendix C rows (AGY-3, AGY-6);
- the plan's `as_of_decision` rule (AGY-15);
- the arithmetic of every worked card.

The errors I found are listed under each idea.

**Conflict of interest.** Four of these ideas overlap my own list:
- AGY-2 is nearly my idea 3 (Spreadsheet Lifter);
- the prose half of AGY-5 is my idea 4 (Disclosure Lint);
- AGY-1 overlaps my ideas 1 (Public Track Record) and 9 (guidance lifecycle);
- AGY-7 touches my ideas 6 (phase 2, streamer mirrors) and 8 (fiscal mirror).

I scored them on their own text. Discount my view on those four.

## Overall assessment

- **Strengths.**
  - The file is ambitious and technically literate.
  - It respects the crate topology: no new crates, downward-only edges, FA reached only as a subprocess.
  - Most ideas name the planned bead they extend.
  - There are good domain insights:
    - the fixed-recovery stream wedge (AGY-7);
    - the multivariate testwork corner that BD-300's one-variable check misses (AGY-12);
    - the transitive hindsight leak and its truncation oracle (AGY-15).
- **Weakness 1: confident exactness that does not survive checking.**
  - "Exact … zero residual": the Shapley identity in AGY-4 has the wrong sign.
  - "Linear chance constraint": the version in AGY-3 is optimistic, not conservative.
  - "Certified (1 − 1/e)": AGY-12 claims it for variance reduction, which is not generally submodular.
  - "0 bits leaked": AGY-8's Merkle sum tree leaks sibling sums.
  - "fmine-io ignores `<f>`": the reader stores formula text (AGY-2).
  - Several citations are wrong: E601/E602 (AGY-1), E666 (AGY-9), an Appendix C "I-25" row and `EFFECT-PLANT-SETPOINT` (neither exists in the plan).
  - Much of this is checkable in minutes. Agents implementing from this text would ship the errors.
- **Weakness 2: timing.**
  - Ten of the fifteen first slices need site data, block models, contracts or telemetry: AGY-3, 4, 6, 9, 10, 11, 12, 13, 14, and the coupling in 7.
  - Only AGY-15 and the prose half of AGY-5 have a clean public-lane slice in M0–M1.
  - AGY-1 and AGY-2 propose M0 additions eleven days before the 9 November release (E454), on top of an M0 that r7
    says does not yet work.
- **Boundaries.** There is no write path to control (INV-011 holds), but four ideas lean toward authority the plan reserves for others:
  - AGY-3 gives plant set-point advice and shift-scale dispatch duals (§4.2 says FrankenMining does not replace dispatch);
  - AGY-6 has a revaluation subledger beside franken_accounting;
  - AGY-13 prices a TSF crest raise;
  - AGY-14 sets stope sequence and stress limits.

  The last two are geotechnical calls behind INV-002's `geotechnical_signoff`.
- **Its ranking.** Its #15 (hindsight guard) is its best idea. Its #3 (shift shadow prices) is among its weakest.

---

## The fifteen, in Gemini's order

### AGY-1: GuidanceCredibilityBridge (pre-release guidance coherence and six-factor guidance attribution) — **540**

- **Strongest point.** It normalizes guidance for the realized price before anyone calls a beat or a miss. That
  answers the question the plan already gives the CFO (§3 readers' table: why AISC moved against guidance, and
  which parts are controllable). R6 §4.2 shows why it matters: industry AISC rose 16% year on year in Q1 2026,
  driven by royalties and corporate G&A (FACT, V-P grade in R6).
- **Biggest weakness.**
  - **Novelty is overstated.**
    - §21.6 already scores the FY2026 forecasts against "Barrick's last stated per-site guidance" as a
      pre-registered baseline.
    - GB-26/BD-064 already give the exact LMDI waterfall.
    - Appendix C (1) and (22) cover the ledger and the waterfall.
  - **The split cannot be identified from public data.** In the public lane, factors 3–5 cannot be separated,
    because guided tonnes, grade and recovery per site are not published. AGY's own kill criterion concedes this, so
    the slice reduces to a price adjustment plus one operational residual.
  - **The mathematics has errors.**
    - σ²_tot = wᵀΣw + μ²_L_corp adds a squared mean to a variance.
    - The coherence inequality is only a necessary condition.
    - The 0.70¹² "Sidak trap" assumes independent sites and ranges with 70% coverage. The issuer states neither.
  - **The guidance rows would be hand-entered literals** in `fmine-public`, the failure mode r7 found (C002–C006).
  - **Sourcing.**
    - The royalty sensitivity β is not in the evidence register.
    - E601 and E602 are miscited. FACT (W5): they concern critical-control verifications and leading indicators of
      fatal risk, not guidance misses or Loulo-Gounkoto.
- **What would make it better.**
  - Load guidance from anchored source pages (page and sha256).
  - Replace independence with my idea 9's implied site correlation.
  - Ship only the price-normalized band, and only where the issuer publishes a sensitivity.
- **Duplication.** My ideas 1 and 9; COD-01; §21.6; Appendix C (1) and (22).

### AGY-2: WorkbookForensicAutopsy (formula-graph decompiler, unit checker and root-cause explainer) — **620**

- **Strongest point.** Site LOM, cut-off and metallurgical-balance workbooks hide plugs, skipped rows and unit
  slips in their formula graphs. The five detectors are concrete and testable. "The parsed formula evaluates to
  Excel's cached value" is a real Parity claim.
- **Biggest weakness: the M0 premise is wrong twice.**
  - **Most published workbooks have no formulas.** FACT (counted this session):
    - of the 18 distinct Barrick workbooks in the corpus, 14 contain no `<f>` element;
    - one contains a single formula;
    - three (Q1 and Q2 2026 AISC, Q2 2026 Mine Stats) carry 304–1,092 formulas each.

    The seeds it promises to explain are mostly value-only 2024–2025 cells (E117, E468–E477) or filing tables
    (E144, E147, E153). Formula autopsy cannot explain those.
  - **The reader already keeps formulas.** FACT: `ExtractedCell.formula` holds the `<f>` text
    (`crates/fmine-io/src/xlsx.rs:54`, `:335`). Wiring the detectors into `run_barrick_m0_assurance` presumes the
    loader r7 found substituting a stand-in workbook.
  - **The detection itself is established spreadsheet-audit practice.** The new part is binding findings to Claims
    and units.
- **What would make it better.**
  - Retarget it at M1–M2 site and developer workbooks.
  - Use the roughly 1,700 public formula cells as the parity corpus, and seeded synthetic workbooks as known answers.
  - Refuse to evaluate `OFFSET`/`INDIRECT`, rather than guessing.
- **Duplication.** Nearly identical to my idea 3, including the findings list. It extends BD-271, BD-275 and
  G-BULL-1/2.

### AGY-3: MineToMillShadowPriceLoop (shift-scale bottleneck regimes, refractory feed-variance constraint, live dual broadcast) — **340**

- **Strongest point.** Shift-to-shift swings in refractory feed chemistry trip roasters and autoclaves even when
  the average is in range. Adding a heterogeneity term to BD-319's two-sided windows is a real improvement.
- **Biggest weakness.**
  - **The "linear chance constraint" is optimistic, not conservative.** INFERENCE: √ is concave, so
    Σ w_s √v_s ≤ √(Σ w_s v_s). The linear coefficient therefore understates the blend's standard deviation and
    admits plans that break the stated chance level. The worked card also miscomputes σ: its inputs give
    √(1.355/6) = 0.475, not 0.51.
  - **It is the wrong product.** Shift duals broadcast to dispatch, and grind and residence-time set-point advice,
    move FrankenMining into dispatch and advanced process control, which §4.2 says it does not replace.
    `EFFECT-PLANT-SETPOINT` does not exist in the plan; only `EFFECT-DIGLINE` does (§15.7). It is advisory, so
    INV-011 holds.
  - **Its premise is unregistered.** Regime flips "inside a week or shift" is a HYPOTHESIS with no evidence entry.
    Everything needs site telemetry (M2–M4).
- **What would make it better.** Fold a correct feed-variance constraint into BD-319 and drop the live broadcast.
  The constraint should be a second-order cone, or conservative, using the largest per-source σ.
- **Duplication.** §15.3/§15.6 duals, BD-319, BD-320, GB-19 and M-PUSH-5. It overlaps AGY-13 (TSF dual), AGY-9
  (feed variance from `StockpileVoxelTwin`) and AGY-12 (the same chemistry windows).

### AGY-4: RpeeeAndBlockModelForensics (brick-level model-version attribution, break-even revenue factor field, sterilization check) — **450**

- **Strongest point.** Locating a resource change by brick, and separating depletion, wireframe, density and grade,
  is what a QP needs before signing. The card picks the case that matters: an unflagged density change.
- **Biggest weakness.**
  - **The "exact" three-factor Shapley identity has the wrong sign.** INFERENCE: averaging the six orderings gives
    Δp·[½(ρ₀g₀+ρ₁g₁) − ⅙ΔρΔg]. With "+⅙", the three terms over-count by ΔpΔρΔg. Example: if every factor doubles
    from 1 to 2, the true change is 7 and AGY's formula gives 8. Its ≤ 1e-12 property (PO-HALL-DIFF-1) would fail
    on its first test.
  - **λ*(b) is not new.** It is the nested-shell index that parametric pit analysis already produces (§15
    `pitflow`, "parametric nested shells"; BD-250).
  - **Orphan-pod detection belongs to mineable-shape tools.** §4.2 says FrankenMining is not a Deswik replacement.
  - **The grade bucket is too coarse.** "Grade estimator" lumps together infill, capping, variogram and search, the
    split a QP most needs.
  - **Data.** Site block models only (M2+).
- **What would make it better.**
  - Fix the identity and test it on random factor vectors.
  - Attribute estimator parameters by rerunning with one parameter changed at a time.
  - Present λ*(b) as an output of BD-250, not a new idea.
- **Duplication.** BD-250, BD-070/BD-222 roll-forwards, BD-223 and GATE-HALL-01. It overlaps AGY-7, since both
  change pitflow block values.

### AGY-5: AgentSandboxAndProseVerifier (copy-on-write counterfactual overlay and `fmine verify-prose`) — **570**

- **Strongest point.** `verify-prose` attacks failure modes agents really show:
  - dropped intervals;
  - 100% versus attributable basis swaps;
  - "safe" or "compliant" creeping into prose (G-FREE-1, INV-002).

  Its Pueblo Viejo card makes a good first fixture: 650 koz on a 100% basis written as "for Barrick", where 60%
  gives 390 koz.
- **Biggest weakness.**
  - **It bundles two ideas.**
    - The copy-on-write sandbox has no consumers until Strata, Pushback, Crucible, Bullion and Helm exist (M3+).
    - `ClaimColor::Hypothetical` is a new type with a new refusal surface.
    - "20× fewer realizations" comes from an illustrative GB card (AGENTS.md rule 8).
  - **It checks consistency, not correctness.** A citation verifier checks prose against the Claim store, not
    against reality. Run on today's M0 report, it would pass the wrong figures r7 found (C002–C006).
  - **Some rules are too strict or too fragile.** Refusing every uncited quantity (G-PROSE-1) is too strict for
    human board memos. Basis detection from natural language (G-PROSE-3) is fragile, as its own kill criterion
    concedes.
- **What would make it better.** Ship `verify-prose` alone in M1, starting with structured citation macros. Defer
  the sandbox to the first multi-subsystem DecisionCard.
- **Duplication.** My idea 4 and COD-10; §30.4–§30.5, BD-072, BD-123, G-FREE-1; Appendix C (8), common random
  numbers.

### AGY-6: SmelterOfftakeOptimizer (multi-contract concentrate blending and provisional-pricing subledger) — **380**

- **Strongest point.** Blending lots around payable floors and penalty tiers is real money for the copper division:
  Lumwana, Jabal Sayid at 50%, and later Reko Diq. The worked card's arithmetic checks out: $613k on one
  10,000 dmt parcel.
- **Biggest weakness.**
  - **Nothing is testable in the public lane.** Contracts, assays and port data are commercially confidential. The
    "$15M–$50M/yr" figure is unlabelled and unsourced.
  - **"Zero bilinear non-convexities" is overstated.**
    - The payable floor is a step, which needs binaries.
    - Contract assignment multiplies z_{v,c} by y_{b,v} unless it is linearized with big-M constraints.
  - **The revaluation subledger is FA's job.** The IFRS 9/ASC 815 "subledger" computes accounting entries.
    franken_accounting is the book of record for those (§21.5 provisional pricing; FD-FA-07).
  - **Naming and sourcing.**
    - Calling the quotational period "QP" collides with "qualified person" throughout the plan.
    - The Appendix C "I-25" row it cites does not appear in the plan.
- **What would make it better.**
  - Keep a `SmelterContractCard` evaluator in Crucible as an NSR input.
  - Leave revaluation to FA.
  - Build the blending MILP only when a concentrate site sponsors it.
- **Duplication.** §15.6 NSR, §17.6, §21.5. It overlaps AGY-7, since both put contract terms into block NSR.

### AGY-7: StreamingRoyaltyTheoremProver (cadastral royalties and fixed-recovery streams in block values) — **560**

- **Strongest point.** The fixed-recovery stream wedge is a sharp insight. When physical by-product recovery falls
  below the contractual rate, higher by-product grade lowers a block's value to the operator, and an averaged
  royalty misses that. The card's arithmetic checks out. It also has a real public anchor: Cortez cut-offs
  reproduce only with the AB495 tax omitted (E183).
- **Biggest weakness.**
  - **The name oversells it.** "Theorem prover" describes an encumbrance-aware block value, and zone-based
    royalties are standard in planning tools.
  - **The new parts are site-gated.** 3D claim polygons, block models and schedule coupling are M2+.
  - **The stream terms are unsourced.** They are illustrative and not in the evidence register.
  - **The cut-off audit is mostly planned.** G-ECON-ROY-1 largely restates BD-274's seven cut-off reproductions.
- **What would make it better.** Add a public slice:
  1. register the stream terms from the counterparty's own filings;
  2. reconcile the streamer's published Pueblo Viejo deliveries against Barrick's 60% share of production (my idea
     6, phase 2).

  That tests the fixed-recovery mechanics on public data before any block model arrives.
- **Duplication.** BD-274, BD-296, §15.6 and §21.5; my ideas 6 (phase 2), 8 and 10. It overlaps AGY-6 and AGY-4.

### AGY-8: CryptographicDataRoom (staged, redacted, verifiable data-room bundles) — **400**

- **Strongest point.** Salted commitments that bind Stage-2 disclosures to Stage-1 hashes are a cheap, real defence
  against bait-and-switch. They build on the plan's existing CAS, Merkle/MMR ledger and GATE-WALL-01 differential
  test.
- **Biggest weakness.**
  - **Commitments prove consistency, not completeness.** "0 failed CRM batches dropped" and "100% of the 412 holes"
    hold only if the seller's ledger was externally time-anchored before the deal.
  - **The zero-leak claim fails.** Verifying a Merkle sum tree's root needs the sibling sums, so the outside
    subtrees leak aggregate tonnes and metal. The card itself also discloses the outside hole count (190). That is
    not "0 bits".
  - **Data rooms already work another way.** Real data rooms run on NDAs and raw access for the counterparty's
    independent engineer.
  - **Fixtures must be synthetic.** The idea is built around the live Fourmile → NGM vend-in, which §31.2 walls off.
- **What would make it better.** Pair staged commitments with periodic external time-stamping (the RFC 3161
  receipts §21.6 already uses), and drop the zero-leak and completeness claims.
- **Duplication.** §31.2 partner bundles, FMT-CERT-1, GATE-WALL-01; Appendix C (14), (18) and (19).

### AGY-9: StockpileSegregationAndReclaimPhysics (rill segregation and draw-cone reclaim on `StockpileVoxelTwin`) — **320**

- **Strongest point.** Reclaiming the toe of a segregated pile can raise a false F2 alarm. The mass- and
  metal-conservation property test is well specified, and the card's arithmetic checks out.
- **Biggest weakness.**
  - **The model's parameters are never measured.** The rill Beta densities and the draw-cone diffusion D_k are not
    measured at sites, and stockpile sampling is notoriously unreliable, so the result is mostly prior.
  - **Fines enrichment is deposit-specific.** Coarse-gold ores can invert it.
  - **Its known-answer test is circular.** The "> 70% less F2 attribution error" test uses the same physics as the
    generator, which shows nothing about real piles (§33.1).
  - **It misquotes E666.** FACT (§20.8): the cone opens at 281.7 kt at 2.41 g/t and closes at 352.1 kt at
    2.44 g/t. INFERENCE: that is a swing of about 70 kt and about 6 koz contained, not "61 kt / ~50 koz".
- **What would make it better.** Keep BD-312's declared mixing model with honest interval widths. Add segregation
  only where a site supplies size-by-grade assays and dump positions.
- **Duplication.** BD-312 and §20.8. It overlaps AGY-3.

### AGY-10: DrillholeReferee (in-progress trajectory projection, magnetic-survey guard, mid-hole wedge decision) — **350**

- **Strongest point.** Deep wedge holes are expensive (E656). Pricing a wedge or redrill while the rods are still in
  the hole is a real decision, and the plan prices hole selection only before drilling (BD-315).
- **Biggest weakness.**
  - **The magnetic check already exists.** Checking total field and dip for magnetic interference is standard QC in
    modern survey tools. The plan's `Survey` record already carries an interference flag (§12.1).
  - **Steering belongs to the contractor.** The drilling contractor runs directional steering with its own software
    and real-time data. FrankenMining will not see that data before M2+.
  - **One headline figure is undefined.** The "$4.2M reserve option value" of 85 koz conversion has no definition.
- **What would make it better.** Reduce it to a post-shift VOI card in BD-315, updated as each survey arrives. Leave
  navigation to the contractor.
- **Duplication.** BD-315, BD-286, §12.1–§12.2. It overlaps AGY-11 and AGY-12: all three are experimental design
  over Corebox.

### AGY-11: StructuralAdversary (kinematic checks on L1 wireframes, counter-hypotheses, one-hole falsification) — **440**

- **Strongest point.** It respects "software never edits L1" (§13.2) and writes counter-hypotheses only to
  quarantine. Its cheapest check, comparing oriented-core normals with wireframe normals, is objective and aimed at
  a documented failure (E639, E195).
- **Biggest weakness.**
  - **Its showcase contains a domain error.** The worked card relies on oriented-core measurements from vertical
    holes. Standard bottom-of-hole orientation tools cannot orient vertical core; that needs inclined holes or a
    televiewer.
  - **Generating alternatives is open-ended.** The space of en-echelon interpretations is unbounded, and
    implicit-modelling tools already sample structural uncertainty.
  - **Its test is self-confirming.** "≥ 95% resolved in one hole" is measured on its own generator.
  - **Data.** Site data, M3–M4.
- **What would make it better.** Ship G-STRATA-KIN-1 alone, as a QA check on oriented core from inclined holes that
  feeds §13.3's hypothesis weights.
- **Duplication.** §13.2–§13.3, §13.7 (G-STRATA-9) and BD-315. It overlaps AGY-10.

### AGY-12: CompositionalGeometBridge (multivariate testwork coverage weighted by schedule; core-composite selection) — **550**

- **Strongest point.** It finds a real gap in a planned bead. BD-300's one-variable percentile check passes a feed
  whose joint corner (high sulphide, low carbonate, high TOC) has no testwork at all. The remedy is practical:
  - multivariate area-of-applicability in log-ratio space, weighted by scheduled tonnes;
  - a pick list from the core library that respects sample oxidation age.
- **Biggest weakness.**
  - **The guarantee does not hold in general.** The (1 − 1/e) bound is claimed for posterior-variance reduction,
    which is submodular only under extra conditions. The Krause–Guestrin result is for mutual information.
  - **The weighting is circular.** Dimensions are weighted by the card's own gradients, which are least reliable
    exactly where the card has no data.
  - **The check may flag almost everything.** With about 9 dimensions and about 45 composites, nearest-neighbour
    coverage is sparse.
  - **Data.** Site testwork only (M2+).
- **What would make it better.**
  - Make it BD-300's acceptance criterion, with a dimension-reduced check.
  - Pair each gap with a COD-28-style bound, so the output is a priced testwork list.
- **Duplication.** BD-300, OPEN-STRATA-8 and §17.4; COD-28. It overlaps AGY-3.

### AGY-13: PermitAndCommitmentCriticalPathCompiler (permit dependencies, capacity runways, curtailment pricing) — **500**

- **Strongest point.** Collisions between approval dates and capacity runways are an executive-level risk, and the
  record documents them:
  - Goldstrike's TSF capacity lasts "through 2028" (E190);
  - Kibali's FTSF (E218);
  - Loulo-Gounkoto's buttress (E219);
  - Veladero ran 21.9% below budget (E278).
- **Biggest weakness.**
  - **Most of it exists or is planned:**
    - §18.2's requirement register, BD-247/BD-301;
    - §18.6's runway monitor, BD-304;
    - BD-297's permit gates;
    - Appendix C (25).
  - **The probabilistic layer rests on weak ground.** The "tropical" readiness date is just a longest path.
    REFCLASS-PERMIT does not exist, and permit-duration reference classes are thin and jurisdiction-specific.
  - **Its known-answer test is circular.** The Veladero "known answer" fits the curtailment rule to the drop it then
    reproduces.
  - **It prices a dam-engineering decision.** An "$18M interim crest raise" needs the Engineer of Record (INV-002
    `geotechnical_signoff`). The card must present it as evidence only.
- **What would make it better.** Ship the deterministic slack (t_choke minus the statutory minimum) on the public
  cases, using dates from filings. Add probabilities only once a reference class exists.
- **Duplication.** As listed above. It overlaps AGY-3's TSF dual.

### AGY-14: UndergroundStopeSequenceStressAndFillCoupler (stress-shadow matrix, anti-pillar cuts, paste-window constraints) — **300**

- **Strongest point.** The two-sided paste window is cheap and valuable. A secondary stope must be mined after the
  adjacent fill has cured but before sulphate attack weakens it (E233), which extends G-HEAD-7.
- **Biggest weakness.**
  - **The anti-pillar cut forbids the standard method.** The cut x_{i−1,t} + x_{i+1,t} − x_{i,t} ≤ 1 ignores fill,
    so it rules out primary–secondary stoping, the standard sequence for filled stopes. The "naive" schedule it
    condemns (primaries 1, 3 and 5 first, then the secondaries) is that method.
  - **The stress model is too crude.** A V/(4πr³) superposition kernel is no substitute for the numerical stress
    modelling that rock-mechanics teams use (§14.2; E007).
  - **The decisions belong to others.** Sequence and stress limits are geotechnical calls (INV-002).
  - **Data.** Site data, M4–M5.
- **What would make it better.** Keep only the paste-window constraint, inside BD-307 and G-HEAD-7.
- **Duplication.** §14.3, §15.5, §16.3, BD-307 and BD-320.

### AGY-15: CounterfactualHindsightGuard (transitive temporal-cone check and future-truncation replay oracle) — **720**

- **Strongest point.** It names a real gap and gives two cheap tests that can run now on synthetic and public data:
  1. **A data-cone check over ancestors.** Every input a result depends on, directly or indirectly, must have been
     available at the decision time.
  2. **A metamorphic oracle.** It compares a run on the full store with a run on a physically truncated one.

  The oracle catches leaks that bypass the DAG entirely, which no provenance check can. It protects every FY2026
  hindcast and every retrospective DecisionCard.
- **Biggest weakness.**
  - **It is partly planned:** INV-016, G-TIME-2, PO-TIME-2, BD-309 and Appendix C (13).
  - **The real gaps are not where AGY puts them.** §6.3 already excludes any record whose record time is after the
    anchor, so a card fitted in 2025 is already excluded if it is stored as a ledger record. The real gaps are:
    - artifacts kept outside the ledger: registry TOML, code constants, in-memory tables;
    - the reverse error: a legitimate refit today on pre-decision data is excluded too.
  - **AGY's rule inherits the over-strictness.** It keeps a record-time cutoff beside its data cone.
  - **Its prerequisite is missing.** Every card must record its training set as parents. The current `ClaimDag` and
    the incomplete Claim identity (`crates/fmine-claim/src/lib.rs:312`) do not support that yet.
  - **It is overconfident.** "Kill criterion: none" overstates it.
- **What would make it better.**
  - For derived artifacts, replace the record-time rule with the data-cone rule.
  - Truncate data only, since code and registries necessarily postdate the decision time.
  - Run the oracle first on the public Mine Stats vintages, with release dates as availability times and
    restatements as vintages (E474).
- **Duplication.** As listed above. It complements COD-08 and the "no hindsight" test in my idea 1.

---

## Where Gemini's ideas overlap each other

| Ideas | Shared ground |
|---|---|
| AGY-4, AGY-6, AGY-7 | All three change block values: pitflow shells, or NSR with smelter, stream or royalty terms. They should share one block-value model |
| AGY-3, AGY-13 | TSF and water constraints as Pushback duals (AGY-3's "tailings or water pinched" regime is AGY-13's curtailment) |
| AGY-3, AGY-9 | Feed variance taken from `StockpileVoxelTwin` (BD-312) |
| AGY-3, AGY-12 | The same joint refractory-chemistry windows (sulphide, carbonate, TOC), once for shift variance and once for testwork coverage |
| AGY-10, AGY-11, AGY-12 | Experimental design over Corebox (wedge, falsification hole, composite pick), all adjacent to BD-315 |
| AGY-5, AGY-15 | Both traverse `ClaimDag`: AGY-5 the affected descendants, AGY-15 the ancestors. One implementation should serve both |
| AGY-8, AGY-15 | Both rely on the offline verifier (Appendix C (18)) |

---

## Ranked table (by my score)

| My rank | Idea | Title | Gemini's rank | Score | Main reason |
|---|---|---|---|---|---|
| 1 | AGY-15 | CounterfactualHindsightGuard | 15 | 720 | Real gap; cheap metamorphic oracle testable now; needs the data-cone rule to replace the record-time cutoff |
| 2 | AGY-2 | WorkbookForensicAutopsy | 2 | 620 | Valuable for site workbooks (≈ my idea 3), but 14 of 18 public workbooks have no formulas and the reader already keeps them |
| 3 | AGY-5 | AgentSandboxAndProseVerifier | 5 | 570 | `verify-prose` is strong for agents (≈ my idea 4); the sandbox has no consumers before M3 |
| 4 | AGY-7 | StreamingRoyaltyTheoremProver | 7 | 560 | Fixed-recovery stream wedge is a sharp insight; E183 anchor; terms unregistered |
| 5 | AGY-12 | CompositionalGeometBridge | 12 | 550 | Correct critique of BD-300; guarantee overstated; site-gated |
| 6 | AGY-1 | GuidanceCredibilityBridge | 1 | 540 | Price-normalized guidance is useful; mostly planned; public split unidentifiable; math and citation errors |
| 7 | AGY-13 | PermitAndCommitmentCriticalPathCompiler | 13 | 500 | Executive-relevant; mostly planned; circular known answer; prices a dam decision |
| 8 | AGY-4 | RpeeeAndBlockModelForensics | 4 | 450 | Useful QP diff, but its Shapley identity is wrong and λ*(b) is the nested-shell index |
| 9 | AGY-11 | StructuralAdversary | 11 | 440 | Good quarantine discipline; vertical-hole orientation error; open-ended generator |
| 10 | AGY-8 | CryptographicDataRoom | 8 | 400 | Staged commitments are good; completeness and zero-leak claims fail |
| 11 | AGY-6 | SmelterOfftakeOptimizer | 6 | 380 | Real copper-division money; confidential inputs; subledger belongs to FA |
| 12 | AGY-10 | DrillholeReferee | 10 | 350 | Survey tools already flag magnetics; steering is the contractor's |
| 13 | AGY-3 | MineToMillShadowPriceLoop | 3 | 340 | Optimistic "chance" constraint; drifts into dispatch and process control |
| 14 | AGY-9 | StockpileSegregationAndReclaimPhysics | 9 | 320 | Unidentifiable parameters; circular test; misquotes E666 |
| 15 | AGY-14 | UndergroundStopeSequenceStressAndFillCoupler | 14 | 300 | Anti-pillar cut forbids primary–secondary stoping; keep only the paste window |
