# WIZARD_SCORES_AGY_ON_COD: Gemini (AGY) Evaluation & Scoring of Codex's (COD) 15 Ideas

| Field | Value |
|---|---|
| Evaluator | Gemini (AGY), Dueling Idea Wizards |
| Target File | `planning/duel/WIZARD_IDEAS_COD.md` (Ranks #1–#15: `COD-01..10`, `COD-14`, `COD-16`, `COD-17`, `COD-26`, `COD-28`) |
| Date | 2026-10-29 |
| Audit Context Weighed | `planning/mor/r7/MODES_OF_REASONING_REPORT_AND_ANALYSIS_OF_PROJECT.md` (`C001–C027`, `C050–C066`, `C080–C161`, `R001–R009`) |
| Scale | 0 (worst) to 1000 (best) |

---

## Executive Observations Across COD's 15 Ideas

1. **High epistemic discipline and honest M0 diagnosis, but pervasive mathematical and domain thinness.** COD (`lines 35–40`) accurately identifies the M0 stand-in workbook substitution (`crates/fmine-public/src/lib.rs:224`, `crates/fmine-io/src/xlsx.rs:122`), the `number_or_zero` and `-17.0` AISC bridge plugs (`fmine-public/src/lib.rs:326–374`), the North Mara literals (`:437–456`), and an incomplete `ClaimId` hash in `crates/fmine-claim/src/lib.rs:312–339`. However, across 1,306 lines, COD cites almost *zero* concrete mine mechanisms or quantitative evidence rows from the 672-item `MOR_CASE.md` register (beyond generic range citations like `E113–E115` and `E451–E499`), and every single worked example is a 2-line grade-school arithmetic toy (`1,000 - 400 = 600 koz`, `5.20 + 5.20 = 10.40`, `100 - 80 + 50`, `0.4 × $10M - $5M = -$1M`, `130 - 100 - (120 - 100) = 10`).
2. **Many slots are 50–150 LOC completions of existing plan beads rather than major new capabilities.** In its own appendix (`lines 1251–1260`), COD originally classified several of these items as *"Fold into forecast delivery rather than a separate product"* (`COD-08`) or *"Defer as a VOI extension"* (`COD-17`). Specifically, `COD-08` (tracking missing outcomes and restatements in `FrozenForecast`), `COD-14` (null-space check when removing one row of `Touchstone`'s observation matrix $H$ in `§20.6`/`BD-153`), `COD-17` (adding a completion-time table to `VoiCampaign` in `§22.4`/`BD-227`), `COD-09` (comparing matching sub-components in `§21.6`/`BD-060`), and `COD-10` (3 sentence templates on top of `§30.5`/`BD-072`) are incremental refinements of already-planned beads.
3. **Internal overlap and missing physical/engineering subsystems.** Within COD's 15 ideas, `COD-03` and `COD-26` share the same `origin_family` / batch-error grouping (`line 901`), `COD-04` and `COD-26` both re-evaluate `DecisionCard`s under `ClaimDag` node changes, `COD-06` and `COD-14` both test measurement-subset answerability, and `COD-07` and `COD-09` both diff `§21.6` metric basis definitions. Like CC, COD proposes zero capabilities in 3D subsurface structural/geostatistical modeling (`Strata`), geotechnical slope/stope stability (`Highwall`), mine planning and pit/stope scheduling (`Pushback`), drillhole trajectory control (`Corebox`), or tailings/water critical paths (`Freeboard`).

---

## Detailed Evaluations (Ranks #1–#15)

### 1. COD-01 (Rank #1): Prove what must happen to meet guidance (`Guidance completion test`)
- **Score:** **860 / 1000**
- **Evaluation:**
  - **Strongest point:** Framing mid-year guidance assurance as a deterministic **inverse feasibility problem**—computing the exact remaining H2 production, sales, and AISC numerator/denominator requirements and testing whether required `(tonnes, grade, recovery)` can jointly coexist inside a declared capacity envelope—cleanly avoids the small-$n$ statistical calibration failure identified in `R7 C118/C120` while applying directly to cases such as North Mara H1 2026 (`E587`: 68 koz delivered vs 200–230 koz guidance).
  - **Biggest weakness:** Because public Mine Stats workbooks do not state upper bounds on remaining H2 mill throughput, pushback grade, or stockpile drawdown, the M0 public-lane slice without external capacity bounds reduces to elementary subtraction ($1,000 - 400 = 600\text{ koz}$, which COD's own kill criterion warns risks *"only restating subtraction"*), and it cannot run on real Barrick actuals until the M0 workbook importer (`R7 R001`, `R003`) is rebuilt.
  - **What would make it better:** Populate the feasible operating envelope automatically from two public sources already in the corpus—each site's filed NI 43-101 Technical Report nameplate throughput/recovery curves (`E196`, `E249`, `E566`–`E571`) and the site's historical quarterly p95/max envelope in Mine Stats—and separate gold-price royalty drift from controllable H2 spend (`OPEN-BULL-1`, `R7 C141`).
- **Duplication & Overlap:**
  - *Plan:* Extends `BD-067`/`BD-068` (FY2026 guidance forecast) and `§21.6` (AISC ratio identity).
  - *AGY list:* Overlaps **AGY Idea #1 (`GuidanceCredibilityBridge`)** (which audits guidance feasibility and attributes misses across price vs operational drivers).
  - *CC list:* Overlaps the YTD "Guidance-at-risk" module in **CC Idea 1 (`Public Track Record`)** and **CC Idea 9 (`Guidance lifecycle`)**.

---

### 2. COD-02 (Rank #2): Find contradictions that rounding cannot explain (`Rounding-aware contradiction finder`)
- **Score:** **875 / 1000**
- **Evaluation:**
  - **Strongest point:** Requiring every occurrence of a displayed cell across multiple breakdowns (`5.20 + 5.20 \approx 10` vs `4.80 + 4.80 \approx 10`) to share a **single latent unrounded variable** inside its half-unit rounding interval both rules out a rounding explanation, where none fits, with a checkable witness (`E113`–`E115`, `E144`/`E147`) and suppresses false $\pm 1$-digit alarms when a common rounded pre-image exists.
  - **Biggest weakness:** Relying on local "bounded exact interval propagation and elimination" while deferring nonlinear `tonnes × grade × recovery` and ratio `AISC × oz` identities (`line 179`) means `COD-02` cannot solve coupled linear cycles that require full LP simplex/Farkas duals and skips the multiplicative identities that govern half of Mine Stats; moreover, `R7 C050/C109` showed that 41–46% of Mine Stats cells participate in zero identities, and `fmine-io` does not yet parse `xl/styles.xml` display precision or load the real workbooks (`R7 R003`).
  - **What would make it better:** Upgrade the mechanism from local interval propagation to a full exact-rational (`ExactQty` `i128`) LP feasibility solver emitting Farkas dual certificates ($y \ge 0, y^T A = 0, y^T b < 0$) plus McCormick/branch-and-bound enclosures for multiplicative `tonnes × grade` and `AISC × oz` identities, and parse `xl/styles.xml` number formats directly.
- **Duplication & Overlap:**
  - *Plan:* Materially upgrades `§21.6`, `BD-065`, `BD-066`, `BD-272`, and `BD-275`.
  - *AGY list:* Complements **AGY Idea #2 (`WorkbookForensicAutopsy`)** (which decompiles `.xlsx` `<f>` formulas to trace the formula behind the contradiction).
  - *CC list:* Directly duplicates **CC Idea 2 (`Proof-carrying discrepancies`)**, which solves the exact same rounding-interval feasibility problem with stronger math (Farkas LP certificates, minimum hitting sets, and bilinear McCormick envelopes).

---

### 3. COD-03 (Rank #3): Count independent evidence before narrowing uncertainty (`Independent-evidence accounting`)
- **Score:** **805 / 1000**
- **Evaluation:**
  - **Strongest point:** Adding explicit **observation origin identities** and shared systematic-error groups (`publication copy`, `superseding publication`, `technical replicate`, `shared batch calibration` $y_i = \mu + b + \epsilon_i$ with variance $\sigma_b^2 + \sigma_\epsilon^2/n$) to `ClaimDag` (`crates/fmine-claim/src/lib.rs`) prevents QPs, reconciliation models, and AI agents from treating three filings that quote the same assay—or 50 assays sharing one biased CRM calibration batch—as independent variance-reducing evidence.
  - **Biggest weakness:** The statistical kernel ($y_i = \mu + b + \epsilon_i$ via Cholesky) is elementary variance-components math already covered by the plan's domain models (`§12.4` Corebox batch CRM bias, `§13` Strata nugget/sill covariance, `§20.3` Touchstone correlated sensor errors), while the hard practical problem—detecting when two disclosures or historical drill tables refer to the same physical measurement without fuzzy matching—is left to manual human declaration (`line 240`).
  - **What would make it better:** Automatically propagate `ObservationOriginId` through `Portal` workbook `<f>` formula references and across quarterly Mine Stats comparative columns (`E451`), and bind `Corebox` laboratory certificate and CRM batch IDs directly into the covariance constructor on ingest.
- **Duplication & Overlap:**
  - *Plan:* Partially overlaps `§7.3` (`ClaimDag` lineage and `SharedRealizationIndex` `BD-043`), `§12.4` (Corebox QA/QC batch error), and `§20.3` (Touchstone error covariance), though explicit deduplication of publication copies in `ClaimDag` is a genuine improvement.
  - *AGY list:* No duplicate in AGY's top 15.
  - *COD internal overlap:* Directly overlaps **COD-26 (Rank #11)**, which reuses the exact same `origin_family` grouping to test removing an entire observation family (`line 901`).

---

### 4. COD-04 (Rank #4): Recall decisions when their evidence changes (`Decision recall after evidence changes`)
- **Score:** **770 / 1000**
- **Evaluation:**
  - **Strongest point:** Extending passive `Stale` propagation (`INV-013`, `BD-046`) into a **decision-level recall filter**—distinguishing upstream numerical revisions (`E474`) that stay strictly inside a `DecisionCard`'s `FlipMargin` from changes that flip the action ranking or invalidate an attestation/wall permission—reduces review noise while never carrying an old human sign-off forward, and COD rightly catches that `Claim::new` (`crates/fmine-claim/src/lib.rs:312–339`) currently omits semantic fields from `ClaimId`.
  - **Biggest weakness:** The constitution *already* provides `ClaimDag::invalidate_and_propagate` (`INV-013`, `§6.4`, `BD-046`), `FlipMargin` on every `DecisionCard` (`§22.2`, `crates/fmine-decide/src/lib.rs`), and incremental spatial/schedule recomputation (`§13.4`, `§15`), so `COD-04` is largely glue between `BD-046` and `FlipMargin`; moreover, COD's own kill section (`line 356`) admits that conservative analytical change bounds over nonlinear/ratio recipes will routinely fail and fall back to ordinary full recomputation.
  - **What would make it better:** Skip the fragile analytical bounding algebra and instead run **exact incremental DAG recomputation** coupled with an exact Shapley factor diff (`CC-14`) that reports how much of the card's `FlipMargin` was consumed by each updated input.
- **Duplication & Overlap:**
  - *Plan:* Heavily overlaps `INV-013`, `§6.4`, `BD-046` (`invalidate_and_propagate`), and `§22.2` (`FlipMargin`).
  - *AGY list:* No direct duplicate in AGY's top 15.
  - *CC list:* Overlaps **CC Idea 14 (`Board delta pack`)** on anchor-to-anchor delta attribution.
  - *COD internal overlap:* Overlaps **COD-26 (Rank #11)** (re-evaluating a `DecisionCard` after removing an evidence family) and appendix candidate `COD-11`.

---

### 5. COD-05 (Rank #5): Show when metal becomes spendable cash (`Metal-to-available-cash bridge`)
- **Score:** **745 / 1000**
- **Evaluation:**
  - **Strongest point:** Tracking the **dated, entity-level conversion of physical metal into unrestricted parent cash**—and computing the reverse stress frontier of `(metal price, payment delay)` pairs that breach a near-term liquidity floor (`day 10` $20M shortfall despite `day 15` $50M receipt)—targets a real treasury timing gap where positive quarterly EBITDA is trapped in concentrate quotational periods (`QP`) or JV/host-country sweep locks (`Kibali`, `Pueblo Viejo`, `Loulo`).
  - **Biggest weakness:** Almost the entire capability is *already planned* across `§17.6` (doré/concentrate custody to outturn), `§21.5 row 8` / `FD-FA-07` (provisional pricing and settlement true-ups), `§23.2` (liquidity floors), and `BD-314` (`CapitalEntityGraph` with JV distribution waterfalls, DSCR locks, and withholding taxes); and because shipment-level payment terms and daily entity cash balances are private, `COD-05`'s first slice is just a trivial event fold over one synthetic doré batch (`100 - 80 + 50`) with no real public-lane data.
  - **What would make it better:** Anchor the M0–M1 public-lane slice in real cash-trapping data published in Barrick's quarterly financial statements—reconciling Mine Stats attributable free cash flow against actual JV dividends/distributions received from `NGM`, `Pueblo Viejo`, and `Kibali` and quarterly receivables movements—and connect the private-lane slice to multi-smelter penalty/QP allocation (`AGY Idea #6`).
- **Duplication & Overlap:**
  - *Plan:* Largely duplicates `BD-314` (`CapitalEntityGraph`), `§17.6` (custody and outturn), `§21.5 row 8` (settlement and provisional pricing), and `§23.2` (liquidity floors).
  - *AGY list:* Partially overlaps **AGY Idea #6 (`SmelterOfftakeOptimizer`)** on concentrate QP provisional-to-final settlement accounting.

---

### 6. COD-06 (Rank #6): Answer the owner's question with less shared data (`Answerability-first data offers`)
- **Score:** **755 / 1000**
- **Evaluation:**
  - **Strongest point:** Giving a prospective pilot site GM and data steward an interactive preview of all **minimal schema field subsets** that can compute a specific assurance question (e.g., `dry_feed_mass` directly vs `wet_feed_mass + lot_moisture`, while explicitly warning when monthly totals can reproduce headline recovery but cannot identify a sampler bias) lowers the `GATE-DATA-01`/`03` access barrier (`pre-mortem F2`) without making uncalibrated statistical power claims from unseen noise variances.
  - **Biggest weakness:** The proposed first slice—"enumerate all field subsets for two hand-authored recipes for one material-balance question"—is a ~25-line boolean dependency check over ~6 column names that amounts to a static documentation table, and as COD concedes (`line 508`), column-name presence does not check whether the actual site exports have valid support, time alignment, or non-missing values.
  - **What would make it better:** Unify `COD-06` with `COD-14`'s linear-algebraic null-space observability engine (`Touchstone` $\begin{bmatrix} H \\ A \end{bmatrix}$) so the tool automatically computes *all minimal measurement cut-sets* over an arbitrary plant flowsheet DAG rather than enumerating 2 hardcoded recipes by hand.
- **Duplication & Overlap:**
  - *Plan:* Minor extension of `§34.4` (`GATE-DATA-03`), `§37.3` (M2 pilot data ask), `BD-185`/`BD-186` (import dialects), and `§20.6` (`GB-25`).
  - *AGY list:* No duplicate in AGY's top 15.
  - *CC list:* Directly overlaps **CC Idea 5 (`Pilot Protocol Card`)**, which targets the exact same M2 minimal-data-ask problem (`GATE-DATA-03`, `E599`, `F2`).
  - *COD internal overlap:* Closely overlaps **COD-14 (Rank #12)** (testing which measurement subsets can identify a period-close quantity).

---

### 7. COD-08 (Rank #7): Keep missing outcomes and restatements in the forecast record (`Forecast survival scoreboard`)
- **Score:** **735 / 1000**
- **Evaluation:**
  - **Strongest point:** Fixing `evaluate_model` (`crates/fmine-calib/src/lib.rs:16–155`) so that unresolved, suspended (`Loulo-Gounkoto`), or redefined forecasts cannot silently drop out of the cohort denominator—and reporting exact `[min, max]` whole-cohort coverage bounds (`7/8` observed on 10 forecasts $\implies [7/10, 9/10]$) alongside separate scores for initial vs restated (`E474`) outcome vintages—prevents survivorship bias in forecast track records.
  - **Biggest weakness:** This is a ~120-line data-model fix to `fmine-calib` implementing requirements that are *already in the plan and R7 audit* (`§10.2`, `§22.7` declared non-resolution, `BD-047`–`BD-049`, `BD-076`, `E474`, and `R7 R005/C155` re-segmentation rules), which is why COD's own appendix (`line 1251`) originally stated: *"Fold into forecast delivery rather than a separate product."*
  - **What would make it better:** Fold this directly into `fmine-calib::ForecastLedger` (`BD-047`) and pair it with pre-registered historical guidance and technical-report cohorts (`CC-01`) so it ships with multi-year public data rather than a 10-entry synthetic array.
- **Duplication & Overlap:**
  - *Plan:* Directly overlaps `§10.2`, `§22.7`, `BD-047`–`BD-049`, `BD-076`, and `E474` restatement handling.
  - *AGY list:* No duplicate in AGY's top 15.
  - *CC list:* Overlaps **CC Idea 1 (`Public Track Record`)** (which already tracks initial vs `E474` restated resolution vintages).
  - *COD internal overlap:* Overlaps **COD-16 (Rank #15)** (which adds `intervention-affected` resolution status to the same cohort scoreboard).

---

### 8. COD-28 (Rank #8): Bound what untested ore could do to the decision (`Metallurgical sample-selection bounds`)
- **Score:** **815 / 1000**
- **Evaluation:**
  - **Strongest point:** Extending `§17.4` and `BD-300` (which emit an `OutsideValidity` refusal when geometallurgical samples miss an ore domain) into **contained-metal-weighted Manski partial-identification bounds** ($0.8 \times 90\% + 0.2 \times [40\%, 90\%] = [80\%, 90\%]$, carefully weighting by contained metal $T_i G_i$ rather than dry tonnes $T_i$) tells the metallurgist, QP, and board whether the untested $20\%$ ore fraction can actually reverse a capital or processing decision.
  - **Biggest weakness:** Linear parcel-wise recovery additivity fails in real gold and copper plants whenever the untested domain carries **non-linear blending poisons**—such as preg-robbing organic carbon (`TCM`), arsenic/antimony fouls, or high-clay/talc viscosity spikes (`Carlin`, `Pueblo Viejo`, `Kibali`, `Lumwana` `E172`–`E175`) where $20\%$ bad ore depresses recovery across the *entire $100\%$ blended feed*—and COD's first slice is just a 15-line 2-stratum interval weighted average without spatial `Strata` or `Corebox` integration.
  - **What would make it better:** Add a **non-linear blending contamination bound** for preg-robbing/deleterious species (`G-CRUC-1`) where a minority parcel can degrade bulk circuit recovery, and connect the sensitivity bound to an automated `Corebox` drill-core interval selector (`AGY Idea #12`) that picks the exact archived core boxes needed to narrow the decisive untested domain.
- **Duplication & Overlap:**
  - *Plan:* Extends `§17.4`, `BD-289`, `BD-300` (geometallurgical coverage and `OutsideValidity`), and `OPEN-STRATA-8`.
  - *AGY list:* Complements and partially overlaps **AGY Idea #12 (`CompositionalGeometBridge`)**, which also targets `BD-300` and `OPEN-STRATA-8` (multivariate untested-ore domain detection across scheduled mill feed and optimal `Corebox` testwork selection).

---

### 9. COD-07 (Rank #9): Locate the first comparable difference in a JV disagreement (`Bilateral JV disagreement localization`)
- **Score:** **750 / 1000**
- **Evaluation:**
  - **Strongest point:** Providing a deterministic bilateral diff over the mutually authorized fields of two JV partners' statements (`period`, `basis`, `interest` + effective date, `metric_definition`, `recipe_version`, `visible_inputs`) without crossing `§31.2` confidentiality walls addresses a real reconciliation need (`NGM` 61.5/38.5 `E457`/`E557`, `Pueblo Viejo` 60/40 `E672`) when two internally valid partner bundles produce different numbers.
  - **Biggest weakness:** Comparing two structured export structs in a fixed list order and returning the "first comparable divergence" (`line 712`) is path-dependent whenever multiple differences interact (e.g., a change in ownership effective date combined with by-product vs co-product credit location), and unlike `CC-06` (`JV Mirror`), `COD-07`'s first slice runs only on two hand-crafted synthetic copies rather than extracting and reconciling the real Barrick vs Newmont (`NGM`, `Pueblo Viejo`) and AngloGold (`Kibali`) public filings.
  - **What would make it better:** Replace the order-dependent "first divergence" check with an order-independent **Shapley / telescoping basis bridge**, and test it directly on the real Barrick 40-F/Mine Stats vs Newmont 10-K/10-Q segment disclosures (`CC-06`).
- **Duplication & Overlap:**
  - *Plan:* Extends `§31.2` (JV verifiable bundles), `§21.6` (metric definition flags), and `BD-314`.
  - *AGY list:* Overlaps **AGY Appendix Idea #28 (`JVPartnerReconciliationMirror`)**.
  - *CC list:* Directly overlaps **CC Idea 6 (`JV Mirror`)** (which applies exact interval feasibility and bridge layers to the real Barrick/Newmont/AngloGold public filings).
  - *COD internal overlap:* Overlaps **COD-09 (Rank #14)** (comparing metric definitions across basis differences) and **COD-04 (Rank #4)**.

---

### 10. COD-10 (Rank #10): Keep the final numerical sentence bound to its evidence (`Controlled IR sentences`)
- **Score:** **695 / 1000**
- **Evaluation:**
  - **Strongest point:** Enforcing a typed sentence-slot grammar that distinguishes relative-percent change (`+15.2%`) from percentage-point change (`+2.1 pp`), blocks percentage changes on zero/negative baselines, and binds mandatory ownership, period, basis, and calibration qualifiers directly to the rendered sentence prevents last-mile editorial stripping of critical caveats.
  - **Biggest weakness:** Restricting authors and agents to **three rigid Mad-Libs sentence templates** (a level, an absolute change, and a percent change) covers only a tiny fraction of real IR releases, board memos, or QP packs, and `§30.5` (`BD-072`, `BD-123`) plus `G-BULL-6` *already* mandate deterministic template rendering with attached basis, `rules_version`, intervals, and `no_claim` lines—making `COD-10` a very narrow increment over planned report rendering.
  - **What would make it better:** Broaden the 3-template renderer into a **full Markdown document & inline-citation verifier** (`AGY Idea #5` `fmine verify-prose` / `CC Idea 4` `Disclosure Lint`) that checks every number, delta, percentage/pp change, unit, basis, and forbidden safety/compliance phrase across arbitrary human- or agent-written prose.
- **Duplication & Overlap:**
  - *Plan:* Heavily overlaps `§30.5` (`fmine-report`), `G-BULL-6`, `BD-072`, and `BD-123`.
  - *AGY list:* Overlaps the prose-verification half of **AGY Idea #5 (`AgentSandboxAndProseVerifier` / `fmine verify-prose`)**.
  - *CC list:* Directly overlaps **CC Idea 4 (`Disclosure Lint`)**.

---

### 11. COD-26 (Rank #11): Find the evidence family whose failure would change the choice (`Decision fragility to removal of an observation family`)
- **Score:** **710 / 1000**
- **Evaluation:**
  - **Strongest point:** Running **leave-one-family-out (LOFO) decision refits**—withdrawing an entire registered observation campaign or shared calibration batch (`COD-03`) rather than perturbing a continuous parameter by $\epsilon$ (`FlipMargin`)—exposes when a seemingly robust `DecisionCard` actually rests on a single laboratory batch or drill campaign whose removal collapses identifiability or flips the preferred action.
  - **Biggest weakness:** As COD admits (`line 901`), this is a direct companion to `COD-03` and `COD-04` that counts the same `origin_family` and decision-rerun machinery twice; and when observations are grouped into coarse families (e.g., "all surface drilling" vs "all underground channel samples"), dropping one family out of two almost always causes a trivial identifiability refusal (`FMINE-REF-104`) without requiring a refit engine.
  - **What would make it better:** Fold this into `COD-03` and `fmine-decide::FlipMargin` as a **Minimal Decision-Flipping Observation Subset (MDFOS)** solver (computing the smallest number or weight of individual assays/drillholes whose removal flips the card) rather than only coarse all-or-nothing family deletion.
- **Duplication & Overlap:**
  - *Plan:* Extends `§22.2` (`FlipMargin` and robustness views) and `§9.4`.
  - *AGY list:* No duplicate in AGY's top 15.
  - *COD internal overlap:* Directly overlaps **COD-03 (Rank #3)** (same `origin_family` definitions) and **COD-04 (Rank #4)** (recomputing a `DecisionCard` after removing evidence nodes).

---

### 12. COD-14 (Rank #12): Show what remains measurable when an instrument is lost (`Measurement-outage observability`)
- **Score:** **730 / 1000**
- **Evaluation:**
  - **Strongest point:** Testing whether each requested period-close linear combination $q^T x$ annihilates the null space of the surviving observation and conservation matrix $\begin{bmatrix} H_{\text{surv}} \\ A \end{bmatrix}$ (`inflow = outflow + stock change`) when a weightometer, sampler, or survey is down tells the metallurgical accountant which composite totals remain defensible during an outage and which single stocktake restores the lost split.
  - **Biggest weakness:** `§20.6` (`GB-25`, `BD-153`, `FMINE-REF-104`) *already* requires `Touchstone` to construct the exact observability matrix $\begin{bmatrix} H \\ A \end{bmatrix}$ and check rank/identifiability before every reconciliation solve, so zeroing out the rows of $H$ for a stale/missing sensor and checking $q \perp \ker \begin{bmatrix} H_{\text{surv}} \\ A \end{bmatrix}$ is a ~30-line linear-algebra helper inside `BD-153` rather than a distinct product feature.
  - **What would make it better:** Extend the linear null-space test to the **local Jacobian of bilinear metal balances** ($\text{mass} \times \text{grade} \times \text{recovery}$) and merge it with `COD-06` so pre-pilot schema negotiation and operational sensor-outage triage are handled by one unified `Touchstone` observability module.
- **Duplication & Overlap:**
  - *Plan:* Heavily overlaps `§20.6`, `GB-25` (observability matrix), `BD-153` (`FMINE-REF-104`), and `G-CRUC-2` (`FMINE-REF-404` stale sensor refusal).
  - *AGY list:* No duplicate in AGY's top 15.
  - *COD internal overlap:* Overlaps **COD-06 (Rank #6)** (both test which subsets of measurements identify a requested quantity).

---

### 13. COD-17 (Rank #13): Price information against the last date it can be used (`Last useful date for information`)
- **Score:** **715 / 1000**
- **Evaluation:**
  - **Strongest point:** Modeling discrete campaign completion-time distributions against a hard commitment/option-expiry deadline (`0.4 × $10M − $5M = −$1M` standard vs `0.9 × $10M − $7M = +$2M` expedited)—and noting that lab turnaround can be **state-dependent** (refractory or high-grade samples taking longer, invalidating naive $P(\text{on time}) \times \text{EVSI}$ multiplication)—makes exploration and testwork VOI genuinely operational for earn-in and board gates.
  - **Biggest weakness:** `§22.4`, `GB-29`, and `BD-227` already include campaign cost, delay cost, and staged recourse, and adding a discrete completion-time table to `VoiCampaign` (`crates/fmine-decide/src/lib.rs:49–97`) is a ~60-line struct and loop extension (`line 1260`: *"Defer as a VOI extension"*).
  - **What would make it better:** Wire the turnaround distribution directly to `Corebox`'s empirical `QuadTemporalStamp` assay delay history (`availability_time - collection_time`, `§6.3`, Stratified by ore type/grade) and `Lodestar` tenement/earn-in expiry calendars so the parameters are learned from operational data rather than hand-typed.
- **Duplication & Overlap:**
  - *Plan:* Minor extension of `§22.4`, `GB-29`, and `BD-227` (`VoiCampaign`).
  - *AGY list:* No duplicate in AGY's top 15.
  - *COD internal overlap:* Overlaps appendix candidate `COD-13` (informative assay-delay sensitivity).

---

### 14. COD-09 (Rank #14): State the strongest comparison the metric bases actually permit (`Cross-issuer metric comparability frontier`)
- **Score:** **690 / 1000**
- **Evaluation:**
  - **Strongest point:** When a total-AISC peer comparison must be refused because an adjustment layer (such as by-product credits or corporate G&A) is unbridged or only interval-bounded (`$100–200/oz`), automatically isolating the **deepest matching WGC common component** (e.g., gross cost before credits per attributable ounce sold) and checking whether the interval ordering holds across the entire bounded set gives CFOs and IR teams a defensible alternative to unbridged peer tables.
  - **Biggest weakness:** `§21.6`, `BD-060`–`BD-063`, and `BD-273` (`E605`–`E607`, `E611`) *already* define AISC as a composable hierarchy of named WGC layers with explicit basis flags and refusals, so comparing two metrics at their matching sub-layers or propagating a 1D interval bound (`[$1,700, $1,800] < $1,900`) is a tiny helper inside `fmine-metrics` (`line 1152`).
  - **What would make it better:** Merge this with `COD-07` (`JV disagreement localization`) and populate a real multi-issuer WGC reconciliation registry from Barrick, Newmont, Agnico Eagle, and Kinross 40-F/10-K filings (`CC-12`) so it runs on actual peer disclosures rather than a 2-number synthetic example.
- **Duplication & Overlap:**
  - *Plan:* Minor extension of `§21.6`, `BD-060`–`BD-063`, and `BD-273`.
  - *AGY list:* No duplicate in AGY's top 15.
  - *CC list:* Directly overlaps **CC Idea 12 (`Comparability engine`)**.
  - *COD internal overlap:* Overlaps **COD-07 (Rank #9)** (both compare metric evaluations across `§21.6` basis differences).

---

### 15. COD-16 (Rank #15): Measure intervention benefit without confusing prevention with a bad forecast (`Causal benefit contracts for adopted interventions`)
- **Score:** **670 / 1000**
- **Evaluation:**
  - **Strongest point:** Explicitly solving the **performative prediction / preventive-action paradox**—where a forecast of equipment failure or recovery loss under `no_action` triggers a preventive intervention, making the raw forecast look like a false alarm if scored naively while naive scoring credits the entire baseline as an "avoided loss"—by introducing an `intervention-affected` resolution status (`COD-08`) and requiring a pre-registered counterfactual comparison design is a sharp conceptual addition to `§16.5` and `§17.8` (`G-CRUC-5`).
  - **Biggest weakness:** As COD concedes (`lines 1164–1167, 1218–1221`), `§16.5` (vendor maintenance scoring), `§17.8`/`G-CRUC-5` (vendor benefit forecasts), and `§19.6` (preregistered DiD / staggered rollout) *already* exist in the plan, parallel-trend comparison units rarely exist inside a single mineral processing plant (where grinding/leach trains are coupled or unique), and no public data exist before M2/M4 to test this beyond a 4-number synthetic subtraction (`(130 - 100) - (120 - 100) = 10`).
  - **What would make it better:** Fold the `InterventionAffected` resolution variant directly into `fmine-calib::ForecastLedger` (`COD-08`), and for single-plant `Crucible` interventions where no parallel control line exists, support **pre-registered randomized ABAB / crossover setpoint schedules** stratified by feed ore hardness and head grade.
- **Duplication & Overlap:**
  - *Plan:* Heavily overlaps `§16.5`, `§17.8` / `G-CRUC-5`, and `§19.6` (preregistered DiD / staggered rollout).
  - *AGY list:* No duplicate in AGY's top 15.
  - *COD internal overlap:* Directly ties into **COD-08 (Rank #7)** (`FrozenForecast` cohort resolution states).

---

## Ranked Summary Table of COD's 15 Ideas by AGY Score

| AGY Rank | COD ID (COD Rank) | Idea Title | AGY Score (0–1000) | Primary Subsystems | Key Strength | Key Weakness / Overlap |
|---:|---|---|---:|---|---|---|
| 1 | **COD-02** (#2) | **Find contradictions that rounding cannot explain** | **875** | Portal, Bullion, Hallmark, Adit | Shared latent variable across multiple breakdowns rules out a rounding explanation (`E113–E147`) and suppresses false $\pm 1$-digit alarms | Local interval propagation misses LP/Farkas cycles & defers bilinear `tonnes × grade`; 41–46% of Mine Stats cells sit in no identity (`R7 C050`); duplicates **CC-02** |
| 2 | **COD-01** (#1) | **Prove what must happen to meet guidance** | **860** | Bullion, Helm, Portal, Adit | Deterministic inverse H2 completion test avoids small-$n$ calibration failure (`R7 C118`) and applies to North Mara (`E587`) | Without TR/historical capacity bounds in public Mine Stats, reduces to subtraction (`1000 - 400 = 600`); needs M0 importer fix (`R7 R003`); overlaps **AGY #1** & **CC-01/09** |
| 3 | **COD-28** (#8) | **Bound what untested ore could do to the decision** | **815** | Crucible, Strata, Helm, Hallmark | Contained-metal-weighted Manski bounds turn binary `OutsideValidity` (`BD-300`) into an actionable decision-flip test | Linear parcel additivity fails on preg-robbing/refractory poisons (`E172–E175`); 2-stratum toy lacks spatial `Strata`/`Corebox` selection; overlaps **AGY #12** |
| 4 | **COD-03** (#3) | **Count independent evidence before narrowing uncertainty** | **805** | Plumb, Corebox, Touchstone, Strata | `ObservationOriginId` + batch error ($\sigma_b^2 + \sigma_\epsilon^2/n$) stops repeated citations & correlated batches from faking precision | Gaussian variance-components math already in `§12.4`/`§13`/`§20.3`; origin mapping left manual; overlaps **COD-26** |
| 5 | **COD-04** (#4) | **Recall decisions when their evidence changes** | **770** | Plumb, Helm, Hallmark, Adit | Triages `INV-013` staleness by `FlipMargin` vs lost admissibility (`E474`); catches incomplete `ClaimId` hash (`fmine-claim:312`) | Mostly glue between planned `BD-046` (`invalidate_and_propagate`) and `§22.2` (`FlipMargin`); nonlinear change bounds collapse to full rerun; overlaps **CC-14** & **COD-26** |
| 6 | **COD-06** (#6) | **Answer the owner's question with less shared data** | **755** | Portal, Touchstone, Plumb, Adit | Separates headline-reproduction columns from bias-attribution columns to lower M2 data-sharing friction (`F2`) | 2-recipe subset enumeration is a ~25-line checklist; ignores value-level support/null defects; overlaps **CC-05** & **COD-14** |
| 7 | **COD-07** (#9) | **Locate the first comparable difference in a JV disagreement** | **750** | Bullion, Plumb, Portal, Bedrock | Deterministic bilateral diff over authorized JV fields (`§31.2`) without leaking walled data (`NGM` `E457`/`E557`) | Fixed-order "first difference" is path-dependent under interacting changes; synthetic-only first slice vs **CC-06**'s real Barrick/Newmont filings; overlaps **COD-09** |
| 8 | **COD-05** (#5) | **Show when metal becomes spendable cash** | **745** | Bullion, Crucible, Helm, Touchstone | Dated entity cash-availability fold + reverse `(price, delay)` liquidity-floor frontier catches trapped JV/QP cash | Almost entirely planned in `BD-314` (`CapitalEntityGraph`), `§17.6`, `§21.5 row 8`, and `§23.2`; toy synthetic doré slice lacks public JV dividend data; overlaps **AGY #6** |
| 9 | **COD-08** (#7) | **Keep missing outcomes and restatements in the forecast record** | **735** | Plumb, Portal, Adit | Prevents survivorship bias in `fmine-calib` via whole-cohort `[min, max]` bounds (`[7/10, 9/10]`) & `E474` vintage splits | ~120-line completion of planned `§10.2`, `§22.7`, `BD-047`, `BD-076`, and `R7 R005`; COD's own appendix says *"Fold into forecast delivery"*; overlaps **CC-01** & **COD-16** |
| 10 | **COD-14** (#12) | **Show what remains measurable when an instrument is lost** | **730** | Touchstone, Crucible, Plumb, Freeboard | Null-space test ($q \perp \ker [H_{\text{surv}}; A]$) identifies surviving composite totals and separating stocktakes during sensor outages | ~30-line row-deletion helper inside planned `§20.6` / `GB-25` / `BD-153` observability check; linear-only; overlaps **COD-06** |
| 11 | **COD-17** (#13) | **Price information against the last date it can be used** | **715** | Helm, Corebox, Lodestar, Portal | Prices expedited vs routine lab turnaround against option expiry and catches state-dependent assay delays | ~60-line completion-time extension to `VoiCampaign` (`fmine-decide:49`, `§22.4`, `GB-29`, `BD-227`); COD's own appendix says *"Defer as a VOI extension"* |
| 12 | **COD-26** (#11) | **Find the evidence family whose failure would change the choice** | **710** | Plumb, Helm, Hallmark, Corebox | Leave-one-family-out (LOFO) refitting exposes single-campaign or single-lab fragility missed by local `FlipMargin`s | Double-counts **COD-03** (`origin_family`) and **COD-04** (`DecisionCard` rerun) (`line 901`); coarse family removal trivially triggers `FMINE-REF-104` |
| 13 | **COD-10** (#10) | **Keep the final numerical sentence bound to its evidence** | **695** | Adit, Portal, Bullion, Hallmark | Typed slot binding distinguishes `%` vs `pp`, blocks zero-baseline `%` changes, and locks basis/calibration qualifiers | Only 3 rigid Mad-Libs sentence templates; minor increment over `§30.5` (`BD-072`/`BD-123`) and `G-BULL-6`; narrower than **AGY #5** & **CC-04** |
| 14 | **COD-09** (#14) | **State the strongest comparison the metric bases actually permit** | **690** | Bullion, Portal, Adit, Helm | Isolates deepest matching WGC sub-component and checks interval ordering when one adjustment layer is bounded | Minor helper over planned `§21.6` / `BD-060..063` / `BD-273` layered AISC tree; synthetic 2-number first slice; overlaps **CC-12** & **COD-07** |
| 15 | **COD-16** (#15) | **Measure intervention benefit without confusing prevention with a bad forecast** | **670** | Helm, Plumb, Headframe, Crucible | `InterventionAffected` resolution status prevents penalizing preventive action as a false alarm or crediting baseline as avoided loss | DiD/staggered rollout (`§19.6`) and vendor forecast gates (`§16.5`, `§17.8`/`G-CRUC-5`) are already planned; plants lack parallel control lines; overlaps **COD-08** |
