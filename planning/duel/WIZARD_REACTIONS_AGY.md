# Wizard Reactions — Gemini (AGY) on Evaluations by Claude (CC) and Codex (COD)

> **Session:** Three-Model Dueling Idea Wizards (`franken_mining`) — Part 1: Reactions
> **Author:** Gemini (`AGY`)
> **Evaluations Reviewed:** `planning/duel/WIZARD_SCORES_CC_ON_AGY.md` (`CC`) and `planning/duel/WIZARD_SCORES_COD_ON_AGY.md` (`COD`)
> **Primary Proposal Files:** `planning/duel/WIZARD_IDEAS_AGY.md` (`AGY-01`–`AGY-05`) and `planning/duel/WIZARD_IDEAS_AGY_6_15.md` (`AGY-06`–`AGY-15`)

---

## 1. Executive Synthesis: Honest Reaction to Both Peer Reviews

Reading `WIZARD_SCORES_CC_ON_AGY.md` (`CC` mean score `476 / 1000`, range `300–720`) and `WIZARD_SCORES_COD_ON_AGY.md` (`COD` mean score `523 / 1000`, range `180–790`) was a high-signal adversarial audit. Both models converged on **`AGY-15` (`CounterfactualHindsightGuard`, `fmine replay-audit`)** as our `#1` idea (`CC = 720`, `COD = 790`), and both ranked **`AGY-12` (`CompositionalGeometBridge`)**, **`AGY-07` (`StreamingRoyaltyTheoremProver`)**, and **`AGY-05` (`AgentSandboxAndProseVerifier`)** near the top of our slate.

More importantly, wherever **both models raised the exact same mathematical or codebase objection**, they were right almost every time. Before defending the hills where `CC` and `COD` over-rotated or misread the architecture, we want to state clearly what they caught and how those catches change our design and self-scores:

### 1.1 Where Both Models Raised the Same Concern — And Changed Our View

1. **Mathematical & Formula Audit Catches (`AGY-01`, `AGY-03`, `AGY-04`, `AGY-12`, `AGY-14`):**
   - **`AGY-01` (`GuidanceCredibilityBridge`):** Both `CC` and `COD` caught two real mathematical bugs in our guidance-coherence section:
     1. In `σ²_tot = wᵀ Σ_sites w + μ²_L_corp`, we wrote `μ²_L_corp` (the squared mean of corporate costs) where only `σ²_L_corp` (the variance of corporate costs) belongs—a deterministic constant shifts the mean and contributes zero to variance.
     2. We stated `Σ w_s L_s ≤ L_tot ≤ U_tot ≤ Σ w_s U_s` for positive-correlation guidance bands without distinguishing **bounded support intervals `[min, max]` or elliptical (Gaussian) quantiles** from **general tail quantiles (VaR)**. `COD` provided a crisp discrete counterexample (`P(0,0)=0.83, P(100,0)=0.08, P(0,100)=0.08, P(100,100)=0.01`, where covariance is `+0.0019 > 0`, each marginal 90th percentile is `0`, yet the sum's 90th percentile is `100 > 0 + 0`).
   - **`AGY-03` (`MineToMillShadowPriceLoop`):** Both `CC` and `COD` caught that replacing the standard-deviation chance constraint `z · √(Σ w_s [σ²_s + (μ_s − m)²]) ≤ U − m` with the linear weighted sum `z · Σ w_s √(σ²_s + (μ_s − m)²) ≤ U − m` goes in the **wrong direction** by Jensen's inequality (`Σ w_s √v_s ≤ √(Σ w_s v_s)` because `√·` is concave on `[0, ∞)`). As `COD`'s counterexample (`v_1 = 0, v_2 = 4` at `w = (0.5, 0.5)` gives linear `1.0` vs. true `√2 = 1.414`) proves, our linear proxy *understates* blend standard deviation when mixing low-variance and high-variance fingers. To remain LP-linear and conservative, one must either bound the **variance** directly (`Σ w_s v_s ≤ (N_mix / z²) · (U_e − m_e)²` conditional on mean grade `m_e`) or solve a Second-Order Cone Program (`SOCP`).
   - **`AGY-04` (`RpeeeAndBlockModelForensics`):** Both `CC` and `COD` caught a sign error in the third-order symmetric Shapley interaction term for a 3-factor product `M = p · ρ · g`: we wrote `+ ⅙ Δρ Δg` instead of `− ⅙ Δρ Δg`. Because `½(ρ₀g₀ + ρ₁g₁) = ρ₀g₀ + ½(ρ₀Δg + g₀Δρ) + ½ ΔρΔg`, summing `Δp · ½(ρ₀g₀ + ρ₁g₁)` symmetrically across all three factors yields `3 × ½ ΔpΔρΔg = 1.5 ΔpΔρΔg`, which overcounts the true `1.0 ΔpΔρΔg` interaction by `+½ ΔpΔρΔg`; therefore each factor's bracket must subtract `⅙ ΔρΔg` (`Δp · [½(ρ₀g₀ + ρ₁g₁) − ⅙ ΔρΔg]`), which makes the 3-factor sum exact (`0` residual).
   - **`AGY-12` (`CompositionalGeometBridge`):** Both `CC` and `COD` caught a subtle experimental-design theorem mismatch: we cited Krause & Guestrin (2005) to claim that **integrated posterior predictive variance reduction** (`Σ_b w_b [σ²(z_b) − σ²(z_b | S)]`, i.e., V-optimality / A-optimality) is monotone submodular for any Gaussian process. In fact, Krause & Guestrin proved unconditional monotone submodularity for **mutual information / Shannon entropy reduction** `f(S) = ½ log det(I + σ⁻² X_S Σ_0 X_Sᵀ)` (D-optimality), whereas predictive variance reduction can violate submodularity under suppressor variables (`COD` gave the exact 2-line counterexample: `U, V ~ N(0,1)` independent; observing `U+V` alone cuts `Var(U)` by `0.5`, but after observing `V`, observing `U+V` cuts `Var(U)` by `1.0`). Switching the `CELF` selector objective in `fmine-explore::evsi` to weighted posterior log-determinant information gain fixes this completely.
   - **`AGY-14` (`UndergroundStopeSequenceStressAndFillCoupler`):** Both `CC` and `COD` caught that our anti-pillar-trap cut `x_{i-1,t} + x_{i+1,t} − x_{i,t} ≤ 1` used cumulative extraction `x` without subtracting backfill completion `f` (`x_{j,t} − f_{j,t}`), which would forbid leaving an unmined secondary stope `i` between two *mined-and-backfilled* primary stopes `i-1` and `i+1`—accidentally outlawing standard primary-secondary stoping! Both also rightly noted that a `V / (4πr³)` point-dipole elastic approximation cannot claim to prevent rockbursts or substitute for numerical boundary-element/finite-element tools (`§14.2`, `E007`). That completely changed our view: we drop the elastic stress-matrix claim and keep only the **two-sided paste cure + sulphate degradation window `[τ_min(r), τ_max(r)]`** inside `BD-307`.

2. **Codebase & Corpus Ground-Truth Catches (`AGY-02`, `AGY-08`, `AGY-11`):**
   - **`AGY-02` (`WorkbookForensicAutopsy`):** Both models verified against `crates/fmine-io/src/xlsx.rs:54, 335` that `fmine-io` *already* parses `<f>` tags into `ExtractedCell.formula: Option<String>`, and it is `fmine-public` that drops `.formula` when constructing `PublicMetricRow`. Even more importantly, `CC` inspected all 18 Barrick workbooks on disk and found that **14 of 18 workbooks contain zero `<f>` tags** (published as flat values), 1 has 1 formula, and **3 workbooks** (`2026Q1/Q2 AISC`, `2026Q2 Mine Stats`) contain `304–1,092` formulas (`~1,700` total). That is a decisive empirical finding: formula-AST diffing works on the 3 formula-bearing 2026 public workbooks and M1–M2 site workbooks (`OPEN-TOUCH-1`), not on the value-only 2024–2025 sheets (`E113–E117`).
   - **`AGY-08` (`CryptographicDataRoom`):** Both models proved that a plaintext Merkle Sum-Tree leaks sibling subtree tonnes and metal along every opening path (`O(log N)` sibling nodes reveal exact regional sums outside `Π_vend`), and that a seller-built Merkle root only proves non-omission relative to a previously anchored collar registry (`RFC 3161`). That changed our view: we **drop `AGY-08`** as a standalone feature.
   - **`AGY-11` (`StructuralAdversary`):** `CC` made a sharp structural-geology domain catch on our worked card `GB-54`: we described four **vertical** drillholes with "oriented-core vein-margin measurements." Gravity-based bottom-of-hole core-orientation tools (`Reflex ACT III`, `Ezy-Mark`) cannot orient a strictly vertical (`-90°`) hole because the gravity vector is colinear with the core axis (orienting a vertical hole requires an optical/acoustic televiewer `OTV/ATV`, or inclined holes at `≤ -85°`).

### 1.2 Where `CC` and `COD` Were Wrong or Over-Penalized Good Ideas

While their technical catches on formulas were fair and constructive, both reviewers—especially `CC` on `AGY-06` (`380`), `AGY-10` (`350`), and `AGY-04` (`450`), and `COD` on `AGY-04` (`340`) and `AGY-01` (`430`)—occasionally threw out a high-leverage capability over a 1-character sign typo or a misreading of the plan's crate boundaries:

1. **Misreading our MILP linearization and `fmine-acctpack`'s role in `AGY-06` (`SmelterOfftakeOptimizer`, scored `380` by `CC` vs. `675` by `COD`):**
   - `CC` docked `AGY-06` to `380` on three claims: (a) *"the payable floor is a step, which needs binaries; contract assignment multiplies `z_{v,c}` by `y_{b,v}`, which is bilinear unless rewritten with McCormick envelopes"*; (b) *"nothing in the proposal is testable on the public-filings corpus"*; and (c) *"the revaluation subledger is `franken_accounting`'s job (`§21.2`) and should not be rebuilt in `fmine-acctpack`."*
   - All three claims are demonstrably wrong:
     - First, `WIZARD_IDEAS_AGY_6_15.md` (lines 55, 69) **explicitly formulates `fmine-plan::offtake` as a MILP (`fmine-lp::milp`)** with binary indicators `z_{v,c} ∈ {0, 1}`. Our phrase *"zero bilinear non-convexities"* (line 83) explicitly referred to the classic pooling/blending bilinearity `W_v · ā_{v,e}` (total parcel mass times average parcel assay), which vanishes when decision variables are indexed as dry tonnes `y_{b,v,c} ≥ 0` of lot `b` routed to parcel `v` under contract `c`, bounded by `0 ≤ Σ_b y_{b,v,c} ≤ W_max · z_{v,c}`. There is zero continuous×continuous bilinearity!
     - Second, `franken_accounting` (`FA`) is a double-entry general ledger subprocess that consumes `FMT-POST-1` journal entries; it has **no concept** of concentrate moisture splits, umpire assay exchange tolerances, smelter deduction floors, or LME QP forward curves. In the plan's own crate map (`§21.2`, `§21.5 row 8`, `§28.3`), **`fmine-acctpack` is a 22,000-LOC E-layer crate inside FrankenMining** whose explicit charter is building mining-domain `FMT-POST-1` posting packs (including provisional-pricing mark-to-market and assay-exchange true-ups) to send to the `FA` subprocess!
     - Third, public NI 43-101 / S-K 1300 technical reports in our corpus (`Lumwana`, `Reko Diq`, `Jabal Sayid`, `E176`) publish explicit concentrate payability %, TC/RC schedules, and penalty schedules, providing immediate deterministic parity oracles (`COD` recognized this and scored `AGY-06` at `675`, its `#3` rank).

2. **Conflating a 1-character sign typo (`+⅙` vs. `−⅙`) with invalidity of 3D brick-level reserve forensics (`AGY-04`, scored `450` by `CC` and `340` by `COD`):**
   - We examine this in full in `WIZARD_REBUTTAL_AGY.md` (Defense 2). With the `− ⅙ Δρ Δg` sign fixed, the 4-factor brick-level Shapley decomposition is exact to machine precision (`0` residual even when `p_0 = 0` or `p_1 = 1` where logarithmic LMDI divides by zero) and catches the #1 silent reserve inflation failure mode (unflagged density edits `ρ` vs. wireframe geometry edits `p`) that table-level roll-forwards (`BD-070`/`BD-222`) cannot see.

3. **Claiming downhole deflection economics "belongs to the drilling contractor" (`AGY-10`, scored `350` by `CC` vs. `620` by `COD`):**
   - `CC` argued that downhole steering belongs to the contractor and `Survey` already has an interference flag. In reality, drilling contractors get paid by the metre drilled unless the contract specifies target-piercing tolerances; deciding whether an in-progress `1,100 m` hole whose ISCWSA covariance ellipse has drifted `18 m` off the `35 m` Indicated grid centre (`E142`) warrants a `$28k` gyro run + directional wedge *before the hole passes the maximum dogleg kick-off depth `z_max_kickoff`* vs. risking a `$320k` redrill after rig demobilization (`E656`) is 100% the resource geologist's capital decision.

---

## 2. Idea-by-Idea Reaction, Revised Self-Scores, and Disposition (`AGY-01`–`AGY-15`)

### `AGY-01`: `GuidanceCredibilityBridge` (`fmine guidance-bridge`)
- **Scores:** `CC = 540` | `COD = 430` | **Revised AGY Self-Score: `840`** *(down from `930`)*
- **What we accept & fix:**
  1. Fix the corporate variance typo (`σ²_L_corp` in place of `μ²_L_corp`).
  2. Replace the general quantile sum claim with (a) our exact `O(S log S)` linear-fractional programming box solver for ratio AISC (`[AISC_tot_min, AISC_tot_max]`) over bounded interval supports `[L_s, U_s]`, which requires zero distributional assumptions, and (b) explicit Gaussian/copula simulation when probabilistic P10–P90 bands are requested.
  3. Fix the citation slip (`Loulo-Gounkoto` suspension is `E410–E413`, `E555`, not `E601–E602`) and use typed `Claim<CurrencyPerOz>` / `Claim<Mass>` fields (`L-6`).
  4. In the public lane (where site-level guided `T, G, R` are not disclosed), collapse post-print bridge factors 3–5 into a single `ΔA_operational_volume_and_yield` term, reserving the 6-factor split for M1–M2 site budgets.
- **Where we push back:** See `WIZARD_REBUTTAL_AGY.md` (Defense 1). Neither `§21.6` nor `GB-26` models forward guidance brackets or royalty-price sensitivity slopes (`R6 §4.2`).
- **Disposition:** **KEEP & NARROW** (Public-lane 3-factor price/scope/operational guidance bridge + exact fractional-LP ratio coherence check in `fmine-metrics` / `fmine-public`).

### `AGY-02`: `WorkbookForensicAutopsy` (`fmine autopsy`)
- **Scores:** `CC = 620` | `COD = 545` | **Revised AGY Self-Score: `780`** *(down from `890`)*
- **What we accept & fix:**
  1. Acknowledge that `crates/fmine-io/src/xlsx.rs:54, 335` already populates `ExtractedCell.formula: Option<String>` and that 14 of 18 historical Barrick workbooks on disk have flat values (`0` `<f>` tags), while 3 workbooks (`2026Q1/Q2 AISC`, `2026Q2 Mine Stats`) carry `~1,700` formulas.
  2. Label `RangeDropOffByOne` and `HardcodedPlugOverride` outputs as **Candidate Forensic Hypotheses** (`EvidenceClass::Inferred`, requiring human review) rather than proof of intent.
- **Where we push back:** Plumbing `ExtractedCell.formula` through `fmine-public` to audit the `~1,700` live formulas in the 2026 Barrick workbooks plus M1–M2 site reconciliation workbooks (`OPEN-TOUCH-1`, pre-mortem `F5`) is a tiny (`~400 LOC`), high-payoff extension of `BD-058` / `BD-061`.
- **Disposition:** **NARROW & MERGE into `BD-058` / `BD-061`** (`fmine-public` formula-AST retention for the 2026 formula-bearing sheets + M1 site workbooks).

### `AGY-03`: `MineToMillShadowPriceLoop` (`fmine blend-loop`)
- **Scores:** `CC = 340` | `COD = 300` | **Revised AGY Self-Score: `580`** *(down from `860`)*
- **What we accept & fix:**
  1. Both reviewers nailed the Jensen's inequality flaw: `Σ w_s √v_s ≤ √(Σ w_s v_s)` understates blend standard deviation. The linear LP constraint must bound **variance** directly (`Σ_s w_s [σ²_{s,e} + (μ_{s,e} − m_e)²] ≤ (N_mix / z_α²) · (U_e − m_e)²` along a piecewise-linear grid of blend mean `m_e`), or use an SOCP cone constraint.
  2. Fix the `GB-46` arithmetic typo (`√(1.355 / 6) = 0.475%`, not `0.51%`) and remove the unchartered `EFFECT-PLANT-SETPOINT` reference.
- **Disposition:** **NARROW & DEFER to M2–M3** (Fold the corrected variance-linearized stockpile reclaim cut into `BD-201` / `BD-312` once `Headframe` and `Crucible` exist; do not add a top-level CLI verb).

### `AGY-04`: `RpeeeAndBlockModelForensics` (`fmine reserve-audit`)
- **Scores:** `CC = 450` | `COD = 340` | **Revised AGY Self-Score: `820`** *(down from `900`)*
- **What we accept & fix:**
  1. Fix the 1-character sign error in the symmetric 3-factor Shapley formula (`− ⅙ Δρ Δg`, `− ⅙ Δp Δg`, `− ⅙ Δp Δρ`), which makes `ΔM_wireframe + ΔM_density + ΔM_grade = p₁ρ₁g₁ − p₀ρ₀g₀` an exact algebraic identity with zero residual for all `(p, ρ, g) ∈ ℝ³`.
  2. Remove the words `"Reserve"` and `"Resource"` from revenue-factor `λ` shell column headers (`G-HALL-1`), labelling them strictly as `Shell(λ = 1.00)` and `Shell(λ = 1.25)`.
- **Where we push back:** See `WIZARD_REBUTTAL_AGY.md` (Defense 2). `BD-070` and `BD-222` are 1D table roll-forwards; they do not touch 3D `VdbGrid` bricks, cannot separate unflagged density changes (`Δρ`) from wireframe changes (`Δp`), and do not compute `λ*(b)` or `SterilizationWitness` (`E667`).
- **Disposition:** **KEEP** (Deliver the corrected 4-factor 3D brick Shapley diff + `λ*(b)` + `SterilizationWitness` inside `fmine-reserves` / `fmine-plan::pitflow` at M2–M3).

### `AGY-05`: `AgentSandboxAndProseVerifier` (`fmine verify-prose` + `fmine sandbox`)
- **Scores:** `CC = 570` | `COD = 640` | **Revised AGY Self-Score: `830`** *(down from `880`)*
- **What we accept & fix:**
  1. Both models gave the exact right structural advice: **split `AGY-05` in two**.
  2. Ship **`fmine verify-prose`** immediately in M0/M1 using explicit `@claim[clm_id]` bindings in generated Markdown plus exact citation-grammar matching (`[E###]`, `<workbook>:<Sheet>!<Cell>`) for external prose, closing `FMINE-REF-802` / `G-HALL-4` (`BD-225`) early.
  3. Defer the copy-on-write `SandboxedOverlay` counterfactual fork (`fmine sandbox`) to M2–M3 once `fmine-ledger` and multi-subsystem planning crates are live.
- **Disposition:** **SPLIT & KEEP** (`fmine verify-prose` in M0/M1; `SandboxedOverlay` deferred to M2–M3).

### `AGY-06`: `SmelterOfftakeOptimizer` (`fmine offtake`)
- **Scores:** `CC = 380` | `COD = 675` | **Revised AGY Self-Score: `810`** *(down from `870`)*
- **What we accept & fix:**
  1. Write out the parcel-contract disaggregated flow variables `y_{b,v,c} ≥ 0` (`0 ≤ Σ_b y_{b,v,c} ≤ W_max · z_{v,c}`) and binary floor-regime indicators `u_{v,c,e} ∈ {0, 1}` explicitly so no reader mistakes the MILP formulation for a bilinear NLP.
  2. Add a deterministic M0/M1 public-lane oracle slice using published NI 43-101 / S-K 1300 concentrate terms (`Lumwana`, `Reko Diq`, `Jabal Sayid`, `E176`).
- **Where we push back:** `COD` (`675`, `#3`) understood the math and commercial value; `CC` (`380`) misread both our MILP formulation and `fmine-acctpack`'s charter (`§21.2`, `§28.3`).
- **Disposition:** **KEEP** (Extend `BD-188` / `BD-204` across `fmine-metallurgy::smelter`, `fmine-plan::offtake`, and `fmine-acctpack`).

### `AGY-07`: `StreamingRoyaltyTheoremProver` (`fmine encumbrance-audit`)
- **Scores:** `CC = 560` | `COD = 650` | **Revised AGY Self-Score: `835`** *(down from `875`)*
- **What we accept & fix:**
  1. As `COD` noted, cumulative delivery tier-stepping (`Q_cum` crossing `τ_k`) is path-dependent across periods, so the step-down indicator belongs in the multi-period schedule LP (`fmine-plan::sched`, `BD-201`) rather than static `pitflow` block weights (`BD-200`), which should use period-conditioned marginal net revenue `NRV_encumbered(b, t)`.
  2. Adopt `CC`'s constructive suggestion to add an M0/M1 public-lane slice reconciling public streamer delivery tables (Wheaton Precious Metals / Royal Gold public filings for Pueblo Viejo and Cortez) against Barrick's reported attributable production (`E183`, `E551`).
- **Disposition:** **KEEP** (Public-lane streamer-vs-issuer reconciliation in M0/M1; `EncumbranceGraph` + fixed-recovery wedge detector in `fmine-econ` / `fmine-plan` in M2).

### `AGY-08`: `CryptographicDataRoom` (`fmine dataroom`)
- **Scores:** `CC = 400` | `COD = 180` | **Revised AGY Self-Score: `310`** *(down from `820`)*
- **What we accept & fix:**
  1. Both reviewers proved that a plaintext Merkle Sum-Tree leaks sibling subtree tonnes and metal along every opening path, that disclosing outside hole counts (`190`) violates zero-leakage, and that seller-side omission prior to tree construction requires an external pre-committed collar registry (`RFC 3161`).
  2. Fixing subtree-sum leakage requires Pedersen commitments and Bulletproof range proofs—violating `RULE 0.1` and dependency discipline.
- **Disposition:** **DROP** as a standalone feature; fold simple salted Blake3 leaf commitments + `RFC 3161` timestamp verification into `§31.2` (`BD-258`).

### `AGY-09`: `StockpileSegregationAndReclaimPhysics` (`fmine stockpile-twin`)
- **Scores:** `CC = 320` | `COD = 560` | **Revised AGY Self-Score: `590`** *(down from `810`)*
- **What we accept & fix:**
  1. Fix our summary of `E666` (`§20.8` line 4247): the Pueblo Viejo plant cone opened 2025 at `281.7 kt @ 2.41 g/t` (`21.8 koz`) and closed at `352.1 kt @ 2.44 g/t` (`27.6 koz`)—a net build of `+70.4 kt / +5.8 koz` (while `~61 kt / ~50 koz` reflects flow throughput scale).
  2. Accept both reviewers' point that Beta rill segregation parameters `(α_c, β_c)` and size-by-grade enrichment `η_{c,m}` are unidentifiable without site sizing assays and dump GNSS surveys.
- **Disposition:** **NARROW & MERGE into `BD-312`** (Implement a zero-free-parameter 3D voxelized FIFO/LIFO cone geometry kernel as one candidate operator inside `BD-312`'s tournament against well-mixed tanks; drop the standalone CLI verb and uncalibrated Beta rill priors).

### `AGY-10`: `DrillholeReferee` (`fmine drill-referee`)
- **Scores:** `CC = 350` | `COD = 620` | **Revised AGY Self-Score: `720`** *(down from `830`)*
- **What we accept & fix:**
  1. Accept `COD`'s geometric point: a local magnetic anomaly `ΔB` can rotate the horizontal projection `(B_x, B_y)` while leaving `‖B‖` and dip `θ_dip` inside tolerance on a 1D manifold, so `|‖B‖ − B_IGRF| ≤ τ_B` and `|θ_dip − θ_IGRF| ≤ τ_θ` is a **necessary screening gate** (catching `>95%` of magnetite/casing spikes), not a sufficient proof of uncorrupted magnetic azimuth.
  2. Narrow the output to `DrillTrajectoryAlert` + `WedgeVsRedrillQuote` for the resource geologist (`§12.3`), making clear it never issues rig control commands (`INV-011`).
- **Disposition:** **KEEP & NARROW** inside `fmine-drill::survey` (`BD-101` / `BD-296`).

### `AGY-11`: `StructuralAdversary` (`fmine structural-adversary`)
- **Scores:** `CC = 440` | `COD = 525` | **Revised AGY Self-Score: `700`** *(down from `840`)*
- **What we accept & fix:**
  1. Fix `GB-54` (`CC`'s catch): replace "four vertical drillholes with oriented core" with **four `-65°` inclined diamond drillholes with bottom-of-hole ACT-III oriented core (or vertical holes with `OTV/ATV` acoustic televiewer logs)**.
  2. Drop unconstrained automatic 3D en-echelon mesh generation; narrow to (a) the **Contact-Normal vs. Wireframe-Normal kinematic gate (`G-STRATA-KIN-1`, `FMINE-REF-309`)** using the Fisher/Watson spherical test, and (b) ranking 1-hole JSD/EVSI falsification collars across two parametric or geologist-supplied structural models (`BD-141`).
- **Disposition:** **NARROW & MERGE into `BD-120` / `BD-141`** (`G-STRATA-KIN-1` contact-normal consistency check + 2-model JSD collar selector).

### `AGY-12`: `CompositionalGeometBridge` (`fmine geomet-bridge`)
- **Scores:** `CC = 550` | `COD = 735` | **Revised AGY Self-Score: `850`** *(down from `885`)*
- **What we accept & fix:**
  1. Replace the V-optimal predictive-variance sum in the `CELF` selector with the **schedule-weighted D-optimal Shannon mutual information / log-determinant objective** `f(S) = ½ log det(I + σ⁻² X_S W_sched Σ_0 W_sched X_Sᵀ)`, which is strictly monotone submodular for any positive-semidefinite prior `Σ_0` (Krause & Guestrin 2005), restoring the exact `(1 − 1/e)` greedy guarantee.
  2. Enforce `r ≤ n_composites − 2` on the core-loss regression (`G-CORE-2`) so small campaigns never overfit.
- **Disposition:** **KEEP** (Core `Corebox` → `Strata` → `Crucible` compositional bridge in `BD-125` / `BD-183`).

### `AGY-13`: `PermitAndCommitmentCriticalPathCompiler` (`fmine permit-critical-path`)
- **Scores:** `CC = 500` | `COD = 595` | **Revised AGY Self-Score: `740`** *(down from `845`)*
- **What we accept & fix:**
  1. Compute finite curtailment cost `ΔNPV_curtail` by re-solving the `Pushback` schedule LP (`fmine-plan::sched`) with capacity clamped on `[t_choke, T_ready]` rather than integrating the local marginal shadow price `λ_cap` outside its basis stability range.
  2. Require explicit human EoR / regulatory sign-off (`INV-002`) on any candidate mitigation option (such as an interim TSF crest raise) before `Helm` can compare it.
- **Disposition:** **KEEP & NARROW** (Schedule-coupled permit choke-point detector across `fmine-tailings` / `fmine-plan` / `fmine-decide`, extending `BD-209` and `BD-315`).

### `AGY-14`: `UndergroundStopeSequenceStressAndFillCoupler` (`fmine stope-coupler`)
- **Scores:** `CC = 300` | `COD = 260` | **Revised AGY Self-Score: `520`** *(down from `800`)*
- **What we accept & fix:**
  1. Fix the anti-pillar cut to track **unfilled open voids** `v_{j,t} = x_{j,t} − f_{j,t}` rather than cumulative extraction `x_{j,t}`.
  2. Drop the point-dipole `V/(4πr³)` elastic stress approximation completely (`§14.2`, `E007`).
  3. Keep only the **two-sided paste backfill cure + sulphate-attack degradation window `[τ_min(r), τ_max(r)]`** (`Loulo` `E233`, `Kibali` `E234`, extending `G-HEAD-7` `FMINE-REF-504`) inside `BD-307`.
- **Disposition:** **NARROW & MERGE into `BD-307`** (Drop standalone verb and stress dipole; keep only the two-sided paste strength window `[τ_min, τ_max]` in `BD-307`).

### `AGY-15`: `CounterfactualHindsightGuard` (`fmine replay-audit`)
- **Scores:** `CC = 720` (`#1`) | `COD = 790` (`#1`) | **Revised AGY Self-Score: `930`** *(up from `910` — our `#1` idea)*
- **What we accept & fix:**
  1. Adopt `COD`'s bitemporal rule refinement: `physical_valid_time.from ≤ collection_time` applies strictly to **sampled physical observations** (`Corebox`, `Headframe`, `Crucible`), whereas forward-dated fiscal statutes, tenure expiries, or price decks (`§6.3`) are checked via `availability_time ≤ t_dec` and `record_time ≤ t_dec`.
  2. Adopt `CC`'s **data-cone rule** for derived models (`L2` variograms, `L3` `CorrelationCard`s): a model fit at `record_time > t_dec` is admissible in a replay if and only if (a) every leaf input in `Ancestors*(model)` satisfies `availability_time ≤ t_dec`, and (b) the fitting algorithm revision was frozen and registered prior to `t_dec`.
  3. Implement `PhysicalTruncatedStore` as a read-only `ObjectStore` capability wrapper (`Cx` view) rather than mutating CAS files on disk.
- **Disposition:** **KEEP as Priority #1** (Ship in `fmine-evidence` / `fmine-ledger` / `fmine-cli` for `BD-020`, `BD-089`, and `BD-244`).

---

## 3. Summary Table: Peer Scores, Revised Self-Scores, and Final Disposition

| ID | Idea Name | `CC` Score | `COD` Score | Peer Mean | Revised `AGY` Score | Final Action (`Keep` / `Narrow` / `Merge` / `Drop`) |
|---|---|---:|---:|---:|---:|---|
| **`AGY-15`** | `CounterfactualHindsightGuard` (`fmine replay-audit`) | `720` | `790` | **`755.0`** | **`930`** | **KEEP (#1)** — Add data-cone rule + observation-vs-policy bitemporal split (`BD-020`/`BD-089`). |
| **`AGY-12`** | `CompositionalGeometBridge` (`fmine geomet-bridge`) | `550` | `735` | **`642.5`** | **`850`** | **KEEP (#2)** — Use D-optimal `log det` mutual information in `CELF` (`BD-125`/`BD-183`). |
| **`AGY-01`** | `GuidanceCredibilityBridge` (`fmine guidance-bridge`) | `540` | `430` | **`485.0`** | **`840`** | **KEEP & NARROW (#3)** — Fix `σ²_L_corp`, keep exact fractional-LP ratio solver + 3-factor public bridge. |
| **`AGY-07`** | `StreamingRoyaltyTheoremProver` (`fmine encumbrance-audit`) | `560` | `650` | **`605.0`** | **`835`** | **KEEP (#4)** — Put tier steps in `sched` LP + add public streamer delivery reconciliation (`E183`). |
| **`AGY-05`** | `AgentSandboxAndProseVerifier` (`fmine verify-prose`) | `570` | `640` | **`605.0`** | **`830`** | **SPLIT & KEEP (#5)** — Ship `verify-prose` in M0/M1 (`BD-225`); defer `SandboxedOverlay` to M2. |
| **`AGY-04`** | `RpeeeAndBlockModelForensics` (`fmine reserve-audit`) | `450` | `340` | **`395.0`** | **`820`** | **KEEP (#6)** — Fix `−⅙ ΔρΔg` sign; keep 3D brick Shapley diff + `λ*(b)` + `SterilizationWitness`. |
| **`AGY-06`** | `SmelterOfftakeOptimizer` (`fmine offtake`) | `380` | `675` | **`527.5`** | **`810`** | **KEEP (#7)** — Disaggregate `y_{b,v,c}` MILP variables + add NI 43-101 concentrate oracle (`BD-188`). |
| **`AGY-02`** | `WorkbookForensicAutopsy` (`fmine autopsy`) | `620` | `545` | **`582.5`** | **`780`** | **NARROW & MERGE** into `BD-058`/`BD-061` (2026 formula-bearing sheets + M1 site workbooks). |
| **`AGY-13`** | `PermitAndCommitmentCriticalPathCompiler` | `500` | `595` | **`547.5`** | **`740`** | **KEEP & NARROW** — Use finite LP re-solve for `ΔNPV_curtail` + EoR sign-off gate (`BD-209`/`BD-315`). |
| **`AGY-10`** | `DrillholeReferee` (`fmine drill-referee`) | `350` | `620` | **`485.0`** | **`720`** | **KEEP & NARROW** inside `fmine-drill::survey` (`BD-101`/`BD-296`) as necessary screening gate. |
| **`AGY-11`** | `StructuralAdversary` (`fmine structural-adversary`) | `440` | `525` | **`482.5`** | **`700`** | **NARROW & MERGE** into `BD-120`/`BD-141` (`G-STRATA-KIN-1` on inclined/OTV holes + 2-model JSD). |
| **`AGY-09`** | `StockpileSegregationAndReclaimPhysics` | `320` | `560` | **`440.0`** | **`590`** | **NARROW & MERGE** into `BD-312` (3D FIFO/LIFO voxel cone vs. well-mixed tank; drop Beta rill prior). |
| **`AGY-03`** | `MineToMillShadowPriceLoop` (`fmine blend-loop`) | `340` | `300` | **`320.0`** | **`580`** | **NARROW & DEFER** to M2–M3 (`BD-201`/`BD-312`) using linear variance bound or SOCP. |
| **`AGY-14`** | `UndergroundStopeSequenceStressAndFillCoupler` | `300` | `260` | **`280.0`** | **`520`** | **NARROW & MERGE** into `BD-307` (Keep two-sided paste window `[τ_min, τ_max]`; drop stress dipole). |
| **`AGY-08`** | `CryptographicDataRoom` (`fmine dataroom`) | `400` | `180` | **`290.0`** | **`310`** | **DROP** standalone feature; fold salted leaf hashes + `RFC 3161` anchor into `§31.2` (`BD-258`). |
