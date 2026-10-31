# Wizard Rebuttal — Gemini (AGY): Two Hills Defended, Two Peer Ideas Attacked

> **Session:** Three-Model Dueling Idea Wizards (`franken_mining`) — Part 2: Rebuttal
> **Author:** Gemini (`AGY`)
> **Structure:** Four structured briefs — **2 Defenses** of our top ideas that `CC`/`COD` scored lowest (`AGY-01` and `AGY-04`), followed by **2 Attacks** on the ideas from `CC` and `COD` that we scored lowest (`CC-15` and `COD-16`). Each brief states **The Claim**, **The Evidence**, and **The Verdict (with `0–100%` confidence)**.

---

## PART A — TWO HILLS WE DEFEND

### 1. DEFENSE #1: `AGY-01` — `GuidanceCredibilityBridge` (`fmine guidance-bridge`)
- **Standing:** Ranked `#1` in `WIZARD_IDEAS_AGY.md`; scored **`540 / 1000` by `CC`** and **`430 / 1000` by `COD`** (Peer Mean: `485.0`).

#### The Claim
`CC` and `COD` scored our `#1` proposal in the `430–540` range based on three objections:
1. **Math objection:** Our guidance-coherence paragraph contained a typo in the variance formula (`μ²_L_corp` instead of `σ²_L_corp`) and stated a linear sum inequality `Σ w_s L_s ≤ L_tot ≤ U_tot ≤ Σ w_s U_s` that does not hold for general non-elliptical tail quantiles (`VaR`).
2. **Novelty objection (`CC`):** The price-normalized guidance band and multi-factor variance bridge are allegedly "mostly planned" in `§21.6` (LMDI variance waterfalls) and `GB-26` (`BD-064`).
3. **Public-lane identifiability objection (`CC` & `COD`):** Because quarterly Mine Stats workbooks do not disclose site-level *guided* tonnes, grade, and recovery (`T_0, G_0, R_0`), factors 3–5 of the 6-factor post-print bridge (`ΔA_throughput`, `ΔA_grade`, `ΔA_recovery`) cannot be separated on public data alone.

We concede the `σ²_L_corp` typo, the non-subadditivity of general tail quantiles, and the collapse of factors 3–5 in the public lane (which our own kill criterion in `WIZARD_IDEAS_AGY.md` explicitly stated!). We contend that dismissing `GuidanceCredibilityBridge` over those points misses the highest-leverage M0/M1 executive and investor workflow in FrankenMining.

#### The Evidence
1. **Neither `§21.6` nor `GB-26` (`BD-064`) models forward guidance brackets or royalty-price sensitivity slopes (`FACT`):**
   - Plan `§21.6` and `GB-26` compute *period-over-period historical actuals* (`Q2 vs. Q1`) using logarithmic LMDI (`ln(V_1 / V_0)`). There is **not a single struct, table, or equation** in `§21.6`, `§25` (`Portal`), `crates/fmine-public/src/lib.rs`, or `crates/fmine-metrics/src/lib.rs` that ingests an issuer's **annual guidance table** (`[L_s, U_s]`), its **stated gold-price assumption** (`P_guide`, e.g., `$2,400/oz`), or its **published royalty sensitivity** (`β_royalty`, e.g., `+$5/oz` to `+$15/oz` AISC per `+$100/oz` ΔAu).
   - Why does this matter? Primary research note `R6 §4.2` (*Cost Inflation*, lines 79–88) establishes that Q1 2026 industry-average AISC jumped `+16%` YoY to `$1,695/oz` largely because realized gold prices (`~$3,000–$3,290/oz`) sat `$600–$890/oz` above guidance decks, mechanically adding `$30–$90/oz` in ad-valorem royalties (`E183`, `E551`). Without `A_guide_norm(P_realized) = A_guide(P_guide) + β_royalty · (P_realized − P_guide) / 100`, every naive guidance tracker (`OPEN-BULL-1`) confuses a mechanical gold-price royalty pass-through with an operational cost overrun, or, when gold prices fall, shows a price-driven "cost beat" as operational (`RISK-22`, `R7 C139`, `C141`).
2. **Our exact `O(S log S)` fractional-LP ratio solver needs *zero* distributional or quantile assumptions (`FACT`):**
   - Consolidated AISC is a **weighted harmonic/ratio aggregation** across `S` sites:
     $$\text{AISC}_{\text{tot}}(Oz_1, \dots, Oz_S, A_1, \dots, A_S) = \frac{\sum_{s=1}^S w_s \, Oz_s \, A_s + L_{\text{corp}}}{\sum_{s=1}^S w_s \, Oz_s}$$
   - When an issuer publishes site production guidance intervals `Oz_s ∈ [L_s^Oz, U_s^Oz]` and site AISC guidance intervals `A_s ∈ [L_s^A, U_s^A]`, testing whether the stated consolidated AISC bracket `[L_tot^A, U_tot^A]` is mathematically reachable on the support box `∏_s [L_s^Oz, U_s^Oz] × [L_s^A, U_s^A]` is a **exact deterministic linear-fractional program (Charnes-Cooper transformation)** solvable in `O(S log S)` operations by sorting sites by `A_s` and sweeping `Oz_s` between `L_s^Oz` and `U_s^Oz`.
   - This check uses **support bounds, not tail quantiles**, so `COD`'s VaR counterexample does not apply to it at all. It deterministically accounts for unallocated corporate adjustments (`E496`, `R7 C003`), attributable-interest differences (`E061`, `E062`: NGM `61.5%`, PV `60%`, Kibali `45%`), and Loulo-Gounkoto scope exclusions (`E410–E413`, `E555`).
3. **In the public lane, the 3-factor decomposition is 100% identifiable and immediately answers the CFO's `#1` question (`DESIGN`):**
   - On public filings alone, `GuidanceCredibilityBridge` decomposes every site and portfolio guidance deviation `ΔA = A_actual − A_guide_mid` into three exact, orthogonal terms:
     $$\Delta A = \Delta A_{\text{price\_royalty}} + \Delta A_{\text{sovereign\_scope}} + \Delta A_{\text{operational\_volume\_and\_spend}}$$
   - When a pilot site connects its M1 budget workbook (`BD-064`), the third term seamlessly expands into the full 6-factor Shapley/LMDI split (`ΔA_throughput + ΔA_grade + ΔA_recovery + ΔA_spend`).

#### The Verdict
With the `σ²_L_corp` typo fixed, support-box coherence separated from Gaussian/copula P10–P90 bands, and the public-lane bridge scoped to the 3 exact public factors (`price_royalty + sovereign_scope + operational`), `GuidanceCredibilityBridge` (`fmine guidance-bridge`) is a **must-ship M0/M1 capability (`Revised Score: 840 / 1000`)**.
- **Confidence:** **`90%`**

---

### 2. DEFENSE #2: `AGY-04` — `RpeeeAndBlockModelForensics` (`fmine reserve-audit`)
- **Standing:** Ranked `#4` in `WIZARD_IDEAS_AGY.md`; scored **`450 / 1000` by `CC`** and **`340 / 1000` by `COD`** (Peer Mean: `395.0`).
- *(Note: We also defended `AGY-06` (`SmelterOfftakeOptimizer`, scored `380` by `CC` vs. `675` by `COD`) in `WIZARD_REACTIONS_AGY.md §1.2`, proving that `y_{b,v,c}` is a strictly linear MILP with zero bilinear terms and belongs in `fmine-acctpack` per `§21.2`/`§28.3`. Here we defend `AGY-04` because both models scored it below `450` over a 1-character sign error.)*

#### The Claim
`CC` (`450`) and `COD` (`340`) docked `RpeeeAndBlockModelForensics` by more than `450` points on three grounds:
1. **Sign error in the 3rd-order Shapley term:** In our symmetric 3-factor decomposition of `p₁ρ₁g₁ − p₀ρ₀g₀`, we wrote `+ ⅙ Δρ Δg` instead of `− ⅙ Δρ Δg`, leaving a `½ Δp Δρ Δg` residual.
2. **Overlap claim:** Both claimed that year-over-year reserve waterfalls (`BD-070`, `BD-222`) and parametric Boykov-Kolmogorov nested shells (`§15.3`, `BD-200`) already cover this territory.
3. **Boundary objection (`COD`):** Writing `"1.0 (Reserve), 1.25 (Resource)"` in a sample CLI header violates the spirit of `G-HALL-1` (`INV-002`).

We accept the 1-character sign fix (`− ⅙` in place of `+ ⅙`) and strip the words `"Reserve"` / `"Resource"` from shell headers (`Shell(λ = 1.00)`, `Shell(λ = 1.25)`). We contend that docking a 3D block-model forensic engine to `340` over a `+` vs. `−` typo ignores a fundamental mathematical and mining-audit breakthrough.

#### The Evidence
1. **With the single sign character fixed (`− ⅙`), the 3D brick-level symmetric Shapley decomposition is an exact algebraic identity with zero residual everywhere—including at zero/one boundary cells where `§21.6` LMDI blows up (`FACT`):**
   - For any brick `b` with wireframe ore fraction `p ∈ [0, 1]`, dry bulk density `ρ > 0`, and grade `g ≥ 0`, let `Δp = p₁ − p₀`, `Δρ = ρ₁ − ρ₀`, `Δg = g₁ − g₀`. Elementary algebra verifies that:
     $$\Delta M_{\text{wireframe}}(b) = V_b \cdot \Delta p \cdot \left[\tfrac{1}{2}(\rho_0 g_0 + \rho_1 g_1) - \tfrac{1}{6}\Delta\rho\,\Delta g\right]$$
     $$\Delta M_{\text{density}}(b) = V_b \cdot \Delta \rho \cdot \left[\tfrac{1}{2}(p_0 g_0 + p_1 g_1) - \tfrac{1}{6}\Delta p\,\Delta g\right]$$
     $$\Delta M_{\text{grade}}(b) = V_b \cdot \Delta g \cdot \left[\tfrac{1}{2}(p_0 \rho_0 + p_1 \rho_1) - \tfrac{1}{6}\Delta p\,\Delta\rho\right]$$
     satisfies **`ΔM_wireframe(b) + ΔM_density(b) + ΔM_grade(b) = V_b (p₁ρ₁g₁ − p₀ρ₀g₀)` identically (`0` residual to machine precision)** for all `(p₀, ρ₀, g₀, p₁, ρ₁, g₁) ∈ ℝ⁶`!
   - Why is this strictly superior to logarithmic LMDI (`§21.6`) on a 3D `VdbGrid`? Because along every geological wireframe boundary, thousands of sub-blocks transition from waste outside the wireframe (`p₀ = 0`) to ore inside the updated wireframe (`p₁ > 0`), or vice versa (`p₁ = 0`). **Logarithmic LMDI (`(M₁ − M₀) / (ln M₁ − ln M₀)`) divides by `ln(0) = −∞` on every single boundary brick!** The symmetric Shapley identity handles `p₀ = 0 ↔ p₁ > 0` analytically without epsilon hacks or residuals.
2. **Table roll-forwards (`BD-070`, `BD-222`) cannot detect unlogged bulk-density changes (`Δρ`) or offsetting spatial shifts (`FACT`):**
   - `BD-070` and `BD-222` read 2D summary disclosure tables (`Tonnes`, `Grade`, `Ounces` by category). A 2D table has **no bulk density column (`ρ`)** and **no 3D brick coordinates `(i, j, k)`**.
   - In real QP reserve audits (`§22`, `E031` S-K 1300, `E032` NI 43-101, `E163` Lumwana density `2.72 t/m³`), a known way a model change can mask a wireframe contraction (`Δp < 0`) or grade drop (`Δg < 0`) is a change of dry bulk density `ρ` from `2.68 t/m³` to `2.78 t/m³` in an unlogged domain script, leaving table-level tonnes unchanged! Only a brick-by-brick 3D `VdbGrid` diff (`fmine-reserves::forensics`) separates `Σ_b ΔM_density(b)` from `Σ_b ΔM_wireframe(b)` and `Σ_b ΔM_depletion(b)`.
3. **`SterilizationWitness` (`Closure(λ) ∩ Cone_down(F)`) checks infrastructure placement against ore sterilisation before construction (`E667`):**
   - While `§15.3` (`BD-200`) computes nested pit shells on unconstrained grids, it has no concept of **surface facility sterilization cones** (`Cone_down(F)` under a proposed waste dump, crusher pad, or TSF embankment like Pueblo Viejo's Naranjo TSF, `E667`). Computing the exact marginal break-even price multiplier `λ*(b)` and emitting a `SterilizationWitness` before an irreversible surface facility is built is a textbook high-value `Pushback`/`Hallmark` capability.

#### The Verdict
With the `− ⅙` sign restored and `G-HALL-1`-clean `Shell(λ)` headers enforced, `RpeeeAndBlockModelForensics` is a mathematically exact, zero-residual 3D block-model audit engine that meets a core QP need (`Revised Score: 820 / 1000`).
- **Confidence:** **`88%`**

---

## PART B — TWO PEER IDEAS WE ATTACK

### 3. ATTACK #1: `CC-15` — `Disclosed-disruption library` (`fmine disruptions --issuer barrick`)
- **Standing:** Proposed as `#15` in `WIZARD_IDEAS_CC.md`; scored **`620 / 1000` by `AGY`** (lowest score in our evaluation of `CC`'s 15 ideas).

#### The Claim
`CC-15` proposes a new CLI verb `fmine disruptions --issuer barrick` and registry workflow where two human readers manually scan quarterly MD&As and technical reports to log disclosed operational disruptions across 8 cause classes (`power`, `weather`, `geotech`, `plant_unplanned`, `labour`, `permit_sovereign`, `supply_chain`, `grade_surprise`), attribute unquantified metal losses to those events using quarterly LMDI throughput/recovery waterfalls (`BD-064`), and fit pooled Poisson / negative-binomial hazard rates with Empirical-Bayes shrinkage to parameterize M1 synthetic twins (`CC-05`) and guidance-at-risk (`CC-01`, `CC-09`).

We contend that `CC-15` is **statistically invalid as a hazard estimator**, **ecologically confounded as an attribution engine**, and **violates `RULE 0.1` (Value Delivery Over Process)** by turning manual text coding on a tiny, outcome-selected sample into a top-level product verb.

#### The Evidence
1. **Severe endogenous reporting bias (truncation on the dependent variable) violates `INV-006` (`FACT` / `INFERENCE`):**
   - In public securities filings (MD&A quarterly commentary), there is no regulatory threshold requiring an issuer to disclose every 12-hour SAG mill trip, grid sag, or heavy rainstorm. A disruption is **more likely to be described in a quarter where it explains a deviation from guidance**; a 24-hour crusher trip in a quarter where higher head grades (`+8%`) offset it may not be mentioned in the MD&A.
   - Fitting a Poisson or negative-binomial arrival rate `λ_cause` to **outcome-selected narrative mentions** conditions on the tail outcome (`Miss`), severely undercounting arrival frequency in beat/in-line quarters while inflating severity per event. Labelling an outcome-selected narrative frequency as a calibrated hazard prior (`EvidenceClass::EmpiricalPrior`) violates `INV-006` (*Evidence never upgrades through derivation*).
2. **Ecological confounding in attributing 90-day LMDI residuals (`BD-064`) to a multi-day narrative event (`FACT`):**
   - `CC-15` states: *"When a disclosure names a disruption without quantifying its metal impact, `BD-064`'s LMDI decomposition supplies the quarterly throughput or recovery delta attributable to the affected driver, tagged `EvidenceClass::Inferred`."*
   - A quarterly Mine Stats row spans **90–92 days** of continuous mining and milling across multiple pushbacks and ore types (`E172–E175`). If an MD&A mentions a 3-day grid power curtailment in Q2 alongside a `-6%` quarterly throughput delta in `BD-064`, assigning the 90-day aggregate LMDI throughput delta to the 3-day power outage conflates 87 days of ore-hardness variation (`BWi`), planned liner changes, and blend shifts with the outage (`§33.1`).
3. **Extreme small-`$n$` sparsity and two-reader process overhead (`RULE 0.1`, `R7 C114`, `C118`):**
   - Across `~10` sites and `11–14` quarters (`2024Q1–2026Q2`), splitting disclosed narrative events across `10 sites × 8 cause classes = 80 cells` yields an average of **0.2 to 1.5 events per cell**. Even pooled across all sites, classes like `supply_chain` or `geotech` have `n ≤ 3` public mentions (`R7 C118`). Building a two-reader manual curation protocol (`R7 C114`), Empirical-Bayes hierarchical shrinkage engine, and top-level CLI verb (`fmine disruptions`) around `n ≈ 2–5` outcome-selected prose anecdotes fails all three prongs of the `RULE 0.1` Value Test.

#### The Verdict
**Drop `CC-15` (`fmine disruptions`) as a CLI verb and statistical hazard-fitting feature.** If `fmine-gen` (`BD-078`) wants historical shock magnitudes for synthetic stress tests, store the `~15` *explicitly quantified* public outages (such as Loulo-Gounkoto's sovereign suspension `E410–E413` or Pueblo Viejo's conveyor failure `E173`) as a static, 50-line cited TOML fixture—with zero Poisson/NegBin hazard inference and zero LMDI residual attribution.
- **Confidence:** **`95%`**

---

### 4. ATTACK #2: `COD-16` — `Measure intervention benefit without confusing prevention with a bad forecast`
- **Standing:** Proposed as `#16` (15th idea) in `WIZARD_IDEAS_COD.md`; scored **`670 / 1000` by `AGY`** (lowest score in our evaluation of `COD`'s 15 ideas).

#### The Claim
`COD-16` proposes `Causal benefit contracts for operational and process interventions` (`Touchstone`, `Headframe`, `Crucible`, `Plumb`, `Helm`) as a top-15 new capability that uses parallel-trend Difference-in-Differences (`DiD`: `(130 − 100) − (120 − 100) = 10`) to estimate the causal economic uplift of adopted operational/process recommendations and links that contract to an `intervention-affected` forecast resolution state.

We contend that `COD-16` is **95% already in the plan (`§16.5`, `§17.8`, `§19.6`)**, relies on a **parallel-trends identification assumption that fails physically in mineral processing circuits**, and has **zero public-lane testability before M3–M4**.

#### The Evidence
1. **Triple duplication with `§16.5`, `§17.8` (`G-CRUC-5`), and `§19.6` (`FACT`):**
   - `COD`'s own proposal (`WIZARD_IDEAS_COD.md` lines 1164–1167) admits:
     - Plan `§16.5` *already* registers and scores external predictive-maintenance and dispatch vendor predictions.
     - Plan `§17.8` and gate `G-CRUC-5` *already* require every processing/reagent vendor benefit claim to be evaluated against a frozen pre-intervention baseline forecast (`INV-008`).
     - Plan `§19.6` *already* specifies preregistered Difference-in-Differences (`DiD`) and staggered rollout evaluation for workforce and operational interventions.
   - Repackaging three existing plan sections (`§16.5`, `§17.8`, `§19.6`) under the name "Causal benefit contracts" adds ceremony (`RULE 0.1`), not a new capability.
2. **Why naive parallel-trends DiD (`(130 − 100) − (120 − 100) = 10`) fails in `Crucible` and `Headframe` (`FACT` / `INFERENCE`):**
   - `COD-16`'s worked example assumes we have a "treated unit" (`100 → 130`) and an independent "parallel-trend comparison unit" (`100 → 120`).
   - In a real gold or copper concentrator/hydromet plant (`§17`, `Crucible`, e.g., Pueblo Viejo autoclaves `E172–E175`, Carlin roasters `E183`, Lumwana flotation `E176`), **what is the untreated parallel-trend comparison unit?**
     - Most mills have a **single primary crushing / grinding / CIL / autoclave train**, or two parallel trains fed from a **common coarse-ore stockpile and shared process-water / tailings thickening loop**.
     - Even when two parallel ball mills or autoclave trains exist, changing the grind target, reagent dosage, or blend on Train A alters the shared cyclone overflow, thickener underflow, or autoclave oxygen/steam header for Train B—violating the **Stable Unit Treatment Value Assumption (`SUTVA`, no interference between units)**!
     - Worse, day-to-day recovery and throughput in mineral processing are driven by **non-stationary geological feed shifts** (preg-robbing organic carbon `TOC`, arsenic/sulphide ratio, Bond Ball Mill Work Index `BWi`, clay content `E172–E175`), which cause massive time-varying confounding whenever ore feed is not randomized between parallel lines.
   - Claiming a `DiD` estimate is a `CausalBenefit` without controlling for feed mineralogy (`CompositionalGeometBridge`) or interference violates `§33.1`.
3. **The only genuinely novel piece is a 15-line enum variant that belongs inside `COD-08` (`DESIGN`):**
   - The single useful observation in `COD-16` is that when an operator acts on an early-warning alarm (e.g., slowing down an autoclave feed pump after a high-sulphide alert), the pre-intervention baseline forecast (`FrozenForecast`) should resolve as `ResolutionDisposition::InterventionAffected { action_id }` rather than scoring the prevented failure as a false alarm.
   - That is a **15-line enum variant in `crates/fmine-calib/src/lib.rs`** (`BD-047`, and already proposed in `COD-08`!), not a standalone multi-subsystem causal-inference product.

#### The Verdict
**Drop `COD-16` as a standalone proposal.** Fold the 15-line `ResolutionDisposition::InterventionAffected` variant into `fmine-calib::ForecastLedger` (`BD-047` / `COD-08`), and leave intervention scoring to the existing gates in `§16.5`, `§17.8` (`G-CRUC-5`), and `§19.6`.
- **Confidence:** **`92%`**
