# WIZARD_STEELMAN_CC: the strongest case for COD-01 and AGY-1

| Field | Value |
|---|---|
| Author | Claude (Opus 5.5), dueling idea wizards, steelman round |
| Date | 2026-10-29 |
| Ideas | COD-01 "Prove what must happen to meet guidance" (`WIZARD_IDEAS_COD.md` §1); AGY Idea 1 `GuidanceCredibilityBridge` (`WIZARD_IDEAS_AGY.md` §1) |
| Scores so far | COD-01: mine 610, AGY 860, Codex's own revision 715. AGY-1: mine 540, COD 430, Gemini's own revision 840 |
| Checked this session | E493–E495, E587, E605–E608; plan §3 (readers), §21.6, §35.2 RISK-22, §37.3, Appendix I.7 OPEN-BULL-1; the row structure of the Mine Stats workbooks (labels and cell counts only; no content quoted) |

## How I approached this

- **I am arguing against my own earlier scores.** I scored COD-01 at 610 ("public output is mostly
  subtraction") and AGY-1 at 540 ("the split cannot be identified from public data"). A steelman that only
  repeated the authors' arguments would not test those verdicts, so each section below adds an argument the
  author did not make and checks it against the evidence register.
- **The arithmetic below is mine.** It is INFERENCE from cited FACTs, and each input carries its E-id. Ownership
  basis is attributable throughout, as in E587.
- **All three of us put guidance first.** My CC-01, COD-01 and AGY-1 are the three authors' #1 ideas, and all
  three target guidance. Three independent models converging on the same executive question is evidence that it
  is the right wedge. The question this round answers is which of the three framings serves it best.

---

## Part A. COD-01: prove what must happen to meet guidance

### A.1 Why the idea is better than it looks

1. **It is the only guidance product that is honest before any calibration exists.**
   - FACT (r7 C118): no positive real-world calibration claim is supported by month 12. Forward products (my CC-01
     guidance-at-risk, AGY-1's joint hit probability) need a calibration record that will not exist in time.
   - COD-01 inverts the question. A requirement such as "H2 must deliver X at no more than Y" is arithmetic on
     cited facts. It needs no probability model, no prior and no calibration flag, and it is exact in `ExactQty`.
   - So it is the guidance product that survives C118, and it plays to the substrate's strengths (exact
     arithmetic, provenance, typed refusal) instead of its weakness (no outcome history).
2. **The joint implication of two affirmed guidance lines is not subtraction.** This is where my 610 was wrong.
   Each line alone is subtraction; the pair implies a requirement that nobody states. Worked from public data
   alone (INFERENCE from E587; attributable basis):
   - **Inputs.**
     - North Mara H1 2026: 68 koz at AISC $2,283/oz, so H1 AISC spend was about $155.2M.
     - FY guidance: 200–230 koz at $1,520–1,680/oz.
   - **H2 production.** H2 must deliver 132–162 koz, 1.9–2.4× the H1 rate. That is E587's existing result.
   - **H2 unit cost.** Annual AISC is total spend over total ounces, so the required H2 AISC is
     (A* × Q* − 155.2M) / Q_H2:

     | FY production | At the top of AISC guidance ($1,680) | At the bottom ($1,520) |
     |---|---:|---:|
     | 200 koz (H2 = 132 koz) | $1,369/oz | $1,127/oz |
     | 230 koz (H2 = 162 koz) | $1,427/oz | $1,200/oz |

   - **Reading.**
     - Meeting both lines needs H2 AISC 37–51% below H1's $2,283/oz.
     - At the bottom of both ranges, H2 must produce 1.9× H1's ounces on *less* total AISC spend than H1
       ($148.8M against $155.2M).
     - Even at the loosest pair (230 koz, $1,680/oz), each H2 ounce above H1's run-rate can add at most about
       $808 of AISC spend. At 200 koz and $1,680/oz the limit is about $399.
   - **Reference point** (a labelled scenario, not a bound; FACT E493–E494):
     - in 2024Q4 North Mara ran at $1,098/oz, when processed grade was 4.29 g/t;
     - in 2026Q2, grade was 1.80 g/t, recovery 85%, and the quarter's AISC $2,577/oz.
     - So the affirmed guidance pair needs H2 costs near the 2024Q4 level, which was achieved at more than twice
       the current grade.
   - **Why it matters.** That sentence is decision-relevant, cited, and computed from two public cells and one
     guidance page. A finance worksheet computes each line; it does not usually state the implied marginal
     cost per extra ounce.
3. **A requirement can be owned and resolved.**
   - A forecast belongs to nobody. A requirement ("H2 AISC ≤ $1,427/oz on ≥ 162 koz") can be assigned to a site
     GM, tracked monthly, and resolved as met or not met at year-end.
   - That turns the guidance meeting from "do we believe the forecast?" into "who owns each requirement, and what
     evidence supports it?". That is the shape of a Helm DecisionCard (retain or revise guidance) with a resolution
     date.

### A.2 The strongest implementation path

- **Step 0: a captured fixture, not the importer.**
  - Take the inputs from cells already located in the register (E587: the Q2 2026 YTD sheet's production and
    AISC cells, and the AIF guidance page; E493–E495 for the reference points).
  - Capture them by an independent read, as r7 R004(1) requires, into a pinned oracle fixture with the file sha256
    from R003's registry.
  - The slice does not touch `fmine-public`'s loader, which still falls back to stand-in bytes
    (`crates/fmine-public/src/lib.rs:236`, `crates/fmine-io/src/xlsx.rs:124`; r7 C001, C059).
  - When the honest importer lands, its values for these cells must equal the fixture, so the fixture doubles as
    an R003 acceptance test.
- **Step 1: one pure function in `fmine-metrics`.**
  - It computes the remaining-quantity identities in `ExactQty`. Annual AISC is always a ratio of sums, never an
    average of halves.
  - It returns `Requirement { quantity, basis, bound, derivation, citations }` or a typed refusal, for example a
    missing sold-ounce cell, or a denominator that can reach zero.
- **Step 2: the joint table** above for every site with per-site guidance at the H1 checkpoint.
  - Each row shows required H2 production and AISC, their ratio to H1, the implied marginal cost per incremental
    ounce, and the distance to labelled reference points (the site's own best and median quarters in the last ten).
  - Reference points are never presented as capacity bounds.
- **Step 3: the verdict space at M0 is {Requirement, Indeterminate(missing input)}.**
  - "Excluded under declared envelope" becomes available only when Pushback and Crucible publish capability
    envelopes (M2+).
  - The conflict set, when it exists, is an infeasible subsystem of the same exact-rational constraint system as
    CC-02 and COD-02, so it reuses that kernel and needs no new solver.
- **Step 4: register each requirement as a Claim with a resolution date** (INV-008, register before resolve).
  The Q3 2026 release (9 Nov 2026, E454) resolves half of H2, and the FY2026 release resolves the rest.
- **Size.** About 300–400 LOC plus the fixture (INFERENCE): the arithmetic is small, and the work is in basis
  discipline.

### A.3 Second-order benefits Codex did not state

- **It speeds up the calibration record the forward products need.** Each checkpoint produces one resolvable
  requirement per site and guided line: for production and AISC alone, two per guided site per checkpoint, against
  one annual guidance outcome per site (INFERENCE). The question "when a site needs ≥ 1.5× its H1 rate, how often does it deliver?" becomes
  answerable years earlier. This is data accumulation, not calibration, which is Codex's own caution.
- **It gives my CC-07 ledger a schedule at every guided site.** The implied H2 spend and the sustaining layer it
  must catch up are exactly the "gap against a dated, filed schedule" that CC-07 needs. Guidance is that schedule,
  and it exists where technical reports do not.
- **It is the answer IR needs before the call.** "What must Q4 look like?" is the standard Q3-call question.
  COD-01 answers it on the issuer's own basis before the analysts compute it.
- **It gives AI agents a non-narrative object.** An agent asked "are we on track?" receives a requirement set with
  citations, not a story. The answer cannot be "on track" while a requirement sits 40% from the run-rate.
- **AGY-1's price conditioning makes it sharper** (B.3). At a higher gold price, royalties rise, so the required
  non-royalty cost cut is larger than the table shows.

### A.4 Defense against the two most likely objections

1. **"It only restates subtraction that finance already does"** (my own 610, and AGY's main weakness).
   - For one line, true. For two or more affirmed lines, false: the North Mara table in A.1 is an unstated joint
     requirement (H2 AISC 37–51% below H1, a marginal-cost ceiling per extra ounce) from public data only.
   - Codex's own kill criterion keeps this honest: kill the surface if it only restates subtraction, measured in
     three pre-selected historical checkpoints.
2. **"Without capacity envelopes it cannot prove anything, and nameplate or historical maxima are not bounds"**
   (Codex's own reaction to AGY's suggestion).
   - Agreed, and the steelmanned design never uses them as bounds. At M0 it states requirements and their
     distance to labelled reference points.
   - The decision value lies in the requirement itself: management, not the tool, judges whether $1,427/oz at
     1.8 g/t is credible. Exclusion verdicts wait for M2 engines.
   - A third objection, that it reads as a claim that guidance is unachievable, is answered by wording: it states
     conditional arithmetic, never feasibility. INV-002 flags stay false, and external use goes through counsel
     (§37.3).

### A.5 Residual concerns I could not argue away

- **Basis traps.**
  - AISC is per ounce *sold*, and E587 gives produced ounces. The table uses produced ounces as a proxy for sold
    ounces, which is OPEN until the sold-ounce cells are captured.
  - The YTD AISC must be the workbook's YTD ratio, not a mean of quarters.
- **Guidance vintage.** If North Mara's range was revised in the Q2 release, the requirement must be recomputed
  against each vintage. E587 cites the AIF range.
- **The issuer's CFO already has the internal version** with real envelopes. At M0 the public version mainly
  serves IR, the board and outside readers, and its value to the executive is as a demonstration (HYPOTHESIS).
- **The retrospective test is anecdotal.** Three checkpoints cannot measure "faster than the worksheet" with any
  precision.

### A.6 Revised score

**COD-01: 610 → 720.** Steelmanning changed my mind on the core critique. The two-line joint requirement is
public-lane information that is not subtraction, and it is the only guidance output that is exact before
calibration exists. It does not go higher because envelope-based verdicts are M2+, and because whether it beats
the finance worksheet is still HYPOTHESIS.

---

## Part B. AGY-1: GuidanceCredibilityBridge

### B.1 Why the idea is better than it looks

1. **Guidance is a conditional claim, and scoring it unconditionally is a type error.**
   - Barrick's AISC guidance is stated at a price deck. AISC contains price-linked lines (royalties, production
     taxes), so the guided number means "AISC if gold averages the deck price".
   - Comparing it with AISC at the realized price compares two different quantities. That is the same kind of
     error as averaging across supports, which the plan forbids by type (INV-014).
   - The fix is a type: guidance carries its conditioning (price deck, FX, perimeter), and a comparison either
     aligns the conditions or refuses.
   - This also exposes a systematic bias: in rising-price years the headline punishes management for the gold
     price, and in falling-price years it flatters them.
2. **The issuer already performs this normalization, privately, and pays on it.**
   - FACT (E605): gold AISC carries 18.75% of the 2025 company API scorecard, with a scored actual of $1,423/oz.
   - FACT (E607): reported FY2025 AISC was $1,637/oz, above guidance because of price-driven royalties.
   - INFERENCE (E608): the Circular promises to disclose adjustments, but none is shown for AISC.
   - The plan lists this gap as open (Appendix I.7, OPEN-BULL-1). It gives the compensation committee the
     question "are the scorecard metrics we pay on measured the same way every year, at the price deck and basis
     we fixed?" (§3).
   - So AGY-1 is not an analyst's nicety. It is the public, reproducible version of a computation that already
     sets part of executive pay, and its first output is the plan's own OPEN-BULL-1 answer.
3. **The price channel can be observed per site, so it needs no published sensitivity.**
   - I faulted AGY for relying on a royalty sensitivity β that is not in the register.
   - FACT (counted this session, row labels and counts only): both the Q4 2025 and Q2 2026 Mine Stats workbooks
     carry a royalties row with per-site numeric values on each of the three site sheets (Minestats1–3; 4 to 18
     values per row). Whether a row is in $M or $/oz is checked at capture; R_actual below is per ounce sold.
   - Where a royalty is ad valorem, the royalty per ounce at the deck price is the actual royalty scaled by
     P_deck / P_realized. The price-conditioned AISC is then

     AISC_deck = AISC_actual − R_actual × (1 − P_deck / P_realized),

     an exact identity under pure ad valorem terms (INFERENCE).
   - Synthetic example: a royalty of $120/oz, a $2,400 deck and a $3,200 realized price give a price effect of
     $30/oz.
4. **The coherence check survives Codex's counterexample once it is stated as sets, not quantiles.**
   - Codex showed the quantile version is false. The deterministic version is sound: if every site lands inside
     its guided ranges, consolidated AISC must lie in [min, max] of AGY's linear-fractional programme over the box.
   - The greedy O(S log S) solution is correct: at the optimum value, sites below it sit at their upper ounce bound.
   - Its useful output is less "incoherent guidance" (probably rare) than the **implied corporate layer per
     ounce**: the gap between consolidated guidance and the ounce-weighted site guidance. R6 §4.2 names corporate
     G&A as one of the two drivers of 2026's industry AISC rise.

### B.2 The strongest implementation path

- **Step 0: a captured FY2025 fixture, independent of the M0 loader.** By independent read, with page and
  sha256 (R004(1), R006):
  - per-site FY2025 guidance rows and their price deck (OPEN: the deck and ranges are not yet in the register);
  - FY2025 per-site royalty and AISC rows;
  - the FY2025 realized price;
  - E605 and E607.

  No guidance row is a literal in code; that was r7's failure (C002–C006).
- **Step 1: `price_condition(aisc, royalty, p_realized, p_deck, terms) → Claim<Interval>`** in `fmine-metrics`,
  in `ExactQty`. It refuses with typed codes:
  - where a royalty is sliding-scale or profit-based and its cited terms are absent (CC-08's fiscal mirror is
    where those terms would come from);
  - where a by-product credit line moves with another metal's price and is not published.
- **Step 2: the deterministic box check**, from site to consolidated, emitting the implied corporate-layer
  interval.
- **Step 3: the three-factor post-print bridge** that Gemini adopted in its reactions:
  - price, through the royalty channel;
  - scope (suspensions and divestments; the plan already lists Loulo-Gounkoto Q1–Q3 2025 as `NOT_PUBLISHED`,
    §37.3);
  - an operational residual that is explicitly *unattributed*.

  The six-factor split waits for site budgets (M2).
- **Step 4: the OPEN-BULL-1 table.** It shows reported $1,637/oz, the price-explained part computed from royalty
  rows, the scored $1,423/oz, and the remainder labelled "not explained by price; adjustment basis undisclosed
  (E608)". Wording: evidence state, never an allegation.
- **Size.** About 400–500 LOC plus the fixture (INFERENCE). Most of the work is in capturing fiscal terms.

### B.3 Second-order benefits Gemini did not state

- **It fixes a flaw in the plan's own forecast baseline.**
  - §21.6 scores the FY2026 forecasts against "Barrick's last stated per-site guidance", and r7 R005 repeats it.
  - Scored unconditionally, that baseline is penalized whenever the price moves away from the deck. FrankenMining's
    forecast would beat it for reasons unrelated to skill.
  - A price-conditioned guidance baseline makes the Reconciliation claim fair (§33.1).
- **It gives RISK-22 the instrument it lacks.**
  - The risk register names the remuneration committee as owner of "KPI-linked pay distorts the measured
    quantities" (§35.2).
  - A normalization method fixed in advance applies symmetrically: in a falling-price year it raises the
    normalized AISC above the reported figure. Applying the same rule both ways is the defense against one-sided
    adjustments.
- **It sharpens COD-01.** Required H2 costs can be stated at the deck price and at spot, separating "the price will
  make you miss" from "operations must improve" (A.3).
- **It gives agents the correct sentence.** An executive's agent asked "did we miss AISC guidance?" should say
  "above the range at the realized price; inside it at the deck price, by $x". The bridge makes that the default
  answer.
- **It gives a peer view through the same channel.** The royalty-row method applies to any issuer that publishes
  royalties per site. That gives the industry-wide price-conditioned cost trend R6 §4.2 asks about, and feeds
  CC-12's comparability engine.

### B.4 Defense against the two most likely objections

1. **"The split is not identifiable from public data; six factors are false precision"** (my 540, Codex's 430).
   - Granted for the submitted text. The steelmanned bridge has three factors:
     - one exact factor from an observed channel (the royalty rows);
     - one discrete factor from cited events (scope);
     - one residual it refuses to split.
   - The price term is the one in dispute (headline against pay), and it is the one that can be identified.
2. **"It is not new: §21.6 uses guidance, GB-26 has the waterfall, and CC-01 and COD-01 overlap"** (both
   reviewers).
   - §21.6 uses guidance unconditionally as a comparator, which is the flaw in B.3.
   - GB-26 decomposes actual against actual, not guidance against actual under aligned conditions.
   - COD-01 is the inverse problem, and CC-01 the track record. None represents guidance as a conditional claim.
   - The plan itself lists the question as open (OPEN-BULL-1), so it is not done.
   - Codex's quantile counterexample applies only to the version the steelman drops (B.1, point 4).

### B.5 Residual concerns I could not argue away

- **Royalties are often not purely ad valorem.** Sliding scales, net-smelter terms with deductions, profit-based
  taxes and price-linked community payments break the simple identity, and the royalty row may bundle them. Many
  sites may refuse until CC-08 captures fiscal terms.
- **The $214/oz gap may not be mainly price.** It may include perimeter or other items the bridge cannot see. The
  bridge can bound only the price part, and its "unexplained remainder" will read as an accusation however neutral
  the wording.
- **Inputs are still OPEN.** The FY2025 deck and per-site ranges are not in the register.
- **The symmetry test needs a falling-price year**, and 2025–2026 offers none.
- **The submitted package still has its errors:** the μ² term, the quantile claim, the 0.70¹² independence
  arithmetic and the miscited E601/E602. The steelman credits the narrowed idea, not that text.

### B.6 Revised score

**AGY-1: 540 → 640** for the narrowed bridge described here. Steelmanning changed my mind on two points:
- the price channel can be observed per site through the royalty rows, so it is identifiable without a published
  sensitivity;
- the idea answers a question the plan already lists as open and that already sets part of executive pay
  (OPEN-BULL-1, E605–E608).

It stays below COD-01 because its exact factor depends on fiscal-term capture, and its headline output (the
unexplained pay remainder) carries reputational risk.

---

## Part C. What changed

| Idea | My score before | After steelman | What moved it |
|---|---:|---:|---|
| COD-01 | 610 | **720** | The two-line joint requirement is not subtraction (North Mara: H2 AISC 37–51% below H1); it is exact before calibration exists |
| AGY-1 | 540 | **640** | The price channel can be observed in the per-site royalty rows; it answers OPEN-BULL-1 and fixes the §21.6 baseline |

- **The effect on my own idea 1.** After this round I would build guidance in this order:
  1. COD-01's inverse requirements;
  2. conditioned on price by AGY-1's royalty channel;
  3. recorded as resolvable claims in CC-01's ledger.

  That puts my CC-01's calibration component last, which is where C118 says it belongs before month 12.
- **The honest version of the ranking.** For an M0–M1 guidance product, Codex's framing is the strongest of the
  three, Gemini's adds the one adjustment that makes it fair, and mine supplies the record they write to.
