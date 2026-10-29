# FrankenMining — Dueling Idea Wizards (Round 1: AGY / Gemini)

> **Scope & Compliance Note.** All 30 candidate ideas and the Top 5 detailed below respect FrankenMining's hard
> architectural and legal boundaries: zero write paths to ISA-95 Levels 0–2 (`INV-011`, `NONGOAL-002`), the four
> human-authority flags (`reserve_disclosure_claim`, `execution_authorized`, `geotechnical_signoff`,
> `regulatory_compliance_claim`) hard-wired `false` (`INV-002`), no new crates outside the 59-crate topology
> (`§28.3`, `registries/loc_budget.toml`), downward-only layer edges (`L-1`: `T → U → V → E → K4..K0 → S1 → S0`),
> `franken_accounting` (`FA`) reached strictly as an external subprocess (`L-4`), public-lane-only fixtures in this
> repository, and zero personal data (`§31.2`). Every number is a `Claim<T>` (`INV-001`), every forecast is frozen
> before resolution (`INV-008`), and every load-bearing statement carries its epistemic label (`FACT`, `INFERENCE`,
> `DESIGN`, `HYPOTHESIS`, `TARGET`, `OPEN`) and `E###` evidence register citation.

---

## Executive Summary of the Top 15 Ideas (Top 5 + Next Best 10, Ranked Best to #15)

| Rank | Idea Name | Subsystems & Crates (§0.5, §28.3) | Primary Users & Workflows | First Shippable Slice |
|---:|---|---|---|---|
| **1** | **`GuidanceCredibilityBridge`** — Pre-Release Multi-Site Guidance Envelope Certifier & Post-Print 6-Factor Guidance Attribution | **Bullion, Helm, Pushback, Portal, Adit** (`fmine-metrics`, `fmine-public`, `fmine-decide`, `fmine-plan`, `fmine-cli`) | Executive Committee, CFO, Investor Relations, Board Audit Committee, Equity Analysts & Agents | **M0 / Phase A** (`fmine assure guidance --issuer barrick --year 2025` over published guidance + Mine Stats `2024Q1–2026Q2`) |
| **2** | **`WorkbookForensicAutopsy`** — Pure-Rust `.xlsx` Formula-DAG Decompiler, Dimensional Type-Checker & Automated Discrepancy Root-Cause Explainer | **Portal, Bullion, Plumb, Adit** (`fmine-io`, `fmine-units`, `fmine-public`, `fmine-cli`) | CFO/Controllers, QPs, Metallurgists, Site Analysts importing `.xlsx` sheets, M0 Public Assurance | **M0 / Phase A** (~1.1k LOC in `fmine-io` + `fmine-public` explaining `E113–E121`, `E144–E153` formula defects automatically) |
| **3** | **`MineToMillShadowPriceLoop`** — Shift-Scale Bottleneck-Regime Classifier, Refractory Feed-Stability Chance Envelope & Live Dual-Price Broadcast | **Pushback, Crucible, Headframe, Touchstone, Adit** (`fmine-plan`, `fmine-plant`, `fmine-ops`, `fmine-recon`, `fmine-tui`) | Site General Manager, Plant Metallurgist, Short-Term Mine Planner, Grade-Control Geologist, Maintenance Planner | **M2–M4** (`fmine-plan::bottleneck` + `fmine-plant` shadow-price broadcast on Carlin/PV/Lumwana fixtures) |
| **4** | **`RpeeeAndBlockModelForensics`** — 4-Factor 3D Block-Model Version Shapley Attribution, Parametric Break-Even Shell Field `λ*(b)` & Sterilization Certifier | **Hallmark, Strata, Pushback, Portal** (`fmine-reserves`, `fmine-geom`, `fmine-optimize`, `fmine-io`) | Qualified Persons (QPs), Chief Resource Geologist, Corporate Development, Long-Term Planners, JV Partners | **M2–M3** (3D `VdbGrid` 4-factor model-to-model diff on imported OMF/CSV models + `pitflow` `λ*(b)` fragility curve) |
| **5** | **`AgentSandboxAndProseVerifier`** — Zero-Copy CoW Counterfactual Ledger Overlay (`ClaimColor::Hypothetical`) & Hallucination-Proof Prose Citation Verifier | **Adit, Bedrock, Plumb, Testpit** (`fmine-mcp`, `fmine-ledger`, `fmine-claim`, `fmine-report`, `fmine-verify`) | AI Agents (internal & Avathon), Executive Committee, IR, QPs, Planners running multi-subsystem what-ifs & drafting memos | **M0–M1** (`fmine verify-prose` CLI/MCP gate in M0; `SandboxedOverlay<AnchorHash>` in M1 with `fmine-ledger`) |
| **6** | **`SmelterOfftakeOptimizer`** — Multi-Smelter Concentrate Lot Blending, Penalty-Cliff Avoidance & IFRS 9 / ASC 815 Quotational-Period (QP) Subledger | **Crucible, Bullion, Pushback** (`fmine-acctpack`, `fmine-plan`, `fmine-plant`, `fmine-metalacct`) | Concentrate Marketing & Commercial Team, CFO/Treasury, Plant Metallurgist, Mine Planner | **M2–M3** (`SmelterContractCard` evaluator + port-bin blending MILP + QP mark-to-market subledger in `fmine-acctpack`) |
| **7** | **`StreamingRoyaltyTheoremProver`** — 3D Cadastral Royalty, Sliding-Scale & Fixed-Recovery Metal-Stream Compiler for Block-Level Optimization | **Helm, Bullion, Hallmark, Pushback** (`fmine-econ`, `fmine-acctpack`, `fmine-reserves`, `fmine-plan`) | Mine Planners, Corporate Development / M&A, General Counsel, CFO, QPs auditing cut-off grade tables | **M1–M3** (`EncumbranceGraph` in `fmine-econ` reproducing Cortez `AB495` `E183` and Pueblo Viejo Ag stream wedges) |
| **8** | **`CryptographicDataRoom`** — Zero-Leakage Verifiable M&A, JV Vend-In (Fourmile → NGM) & Project-Finance Data-Room Compiler (`FMT-VDR-1`) | **Bedrock, Helm, Hallmark, Testpit** (`fmine-authz`, `fmine-ledger`, `fmine-object`, `fmine-verify`) | Corporate Development, JV Partners (Newmont/Sovereigns), Project-Finance Lenders' Technical Consultants, QPs | **M1–M3** (`FMT-VDR-1` MMR redaction & staged CAS chunk unsealing verified offline by `fmine-verify`) |
| **9** | **`StockpileSegregationAndReclaimPhysics`** — Gravity Rill-Slope Size/Grade Segregation & Kinematic Funnel-Flow Draw-Cone Operator on `StockpileVoxelTwin` | **Touchstone, Crucible, Pushback, Strata** (`fmine-recon`, `fmine-plant`, `fmine-geom`) | Plant Metallurgist, Short-Term Planner, Reconciliation Geologist tracking ROM piles & surge cones (`E666`) | **M1–M2** (Segregation + Beverloo/Nedderman draw-cone operator on `StockpileVoxelTwin` in `fmine-recon`, `BD-312`) |
| **10** | **`DrillholeReferee`** — In-Progress Drillhole ISCWSA Trajectory-Covariance Projector, Magnetite-Survey Guard & Mid-Hole Wedge/Redrill Controller | **Corebox, Lodestar, Strata, Helm** (`fmine-drill`, `fmine-qaqc`, `fmine-prospect`, `fmine-decide`) | Exploration Geologist, Resource Geologist, Drill Program Manager, QP managing deep parent/wedge fans (`E656`) | **M2–M4** (`fmine-drill::live_referee` computing pierce-point miss probability & mid-hole EVSI before rig demob) |
| **11** | **`StructuralAdversary`** — Popperian Fault-Kinematics & Contact-Topology Challenger, En-Echelon Counter-Hypothesis Generator & 1-Hole Falsification VOI | **Strata, Corebox, Highwall, Lodestar** (`fmine-belief`, `fmine-geom`, `fmine-geotech`, `fmine-prospect`) | Structural & Resource Geologists, QPs, Geotechnical Engineers checking highwall daylighting (`E195`, `E639`) | **M3–M4** (`fmine-belief::adversary` kinematic checker + fault-offset vs pinch-out falsification VOI) |
| **12** | **`CompositionalGeometBridge`** — Schedule-Weighted Multivariate Area-of-Applicability (AOA) Scanner & D-Optimal Core-Composite Selector | **Strata, Crucible, Corebox, Pushback** (`fmine-materials`, `fmine-plant`, `fmine-drill`, `fmine-plan`) | Chief Metallurgist, Geometallurgist, Resource Geologist selecting composite samples (`OPEN-STRATA-8`) | **M2–M3** (Multivariate Mahalanobis/simplicial-depth AOA over scheduled feed + D-optimal core selector in `fmine-plant`) |
| **13** | **`PermitAndCommitmentCriticalPathCompiler`** — Coupled Permit-Dependency DAG, Volumetric TSF/Water Runway & Trigger-Curtailment Shadow Pricer | **Freeboard, Helm, Pushback** (`fmine-tailings`, `fmine-capital`, `fmine-plan`) | Executive Committee, Head of Sustainability/Permitting, Tailings Engineer, Strategic Mine Planner (`E190`, `E278`) | **M3–M5** (`fmine-tailings::permit_dag` coupling permit clocks & trigger curtailments to `Pushback` capacities) |
| **14** | **`UndergroundStopeSequenceStressAndFillCoupler`** — Boundary-Influence Stress-Shadow Matrix, Anti-Pillar-Trap MILP Cuts & Paste-Binder Cure Co-Optimizer | **Highwall, Pushback, Headframe** (`fmine-geotech`, `fmine-plan`, `fmine-ops`) | Underground Mine Planner, Rock Mechanics Engineer, Backfill Engineer (`E187`, `E233`, `E234`, `BD-307`) | **M4–M5** (Elastic stress-transfer influence matrix + anti-pillar-trap cuts + binder-recipe trade-off in `fmine-plan`) |
| **15** | **`CounterfactualHindsightGuard`** — Transitive Temporal-Cone Prover (`verify_decision_cone`) & Future-Zeroed Ledger Replay Oracle (`PO-TIME-CONE`) | **Plumb, Bedrock, Testpit** (`fmine-claim`, `fmine-time`, `fmine-verify`, `fmine-lab`) | Auditors, QPs, Data Scientists, CI Gate (`GATE-PLUMB-E2`, `BD-161`, `BD-226`, `BD-230`, `BD-309`) | **M0–M1** (`verify_decision_cone` over `ClaimDag` in `fmine-claim` + physical byte-truncation replay test in `fmine-lab`) |

---

## Idea 1 (Best): `GuidanceCredibilityBridge` — Forward Guidance Envelope Compiler, Pre-Release Multi-Site Coherence Certifier & Post-Print 6-Factor Guidance Attribution

### 1.1 What Is Already in the Plan vs. What Is Materially New
- **Already in the plan:**
  - `§21.6` and `crates/fmine-public` (`BD-059`, `BD-061`, `BD-065`) reproduce historical site-quarter actuals from the
    18 Barrick Mine Stats and Q4 AISC workbooks (`2024Q1–2026Q2`, `E101–E138`).
  - `§21.6` (`GB-26`, `BD-064`) decomposes period-over-period *actual-to-actual* AISC changes (`Q_{t-1} → Q_t`) into
    5 or 6 LMDI factors (`crates/fmine-metrics/src/lib.rs:146–198`).
  - `§10` (`BD-067`, `BD-068`) registers FrankenMining's *own* statistical per-site FY2026 forecasts with 80% intervals.
- **What is missing (and materially new here):**
  - The plan has **no representation, verification gate, or attribution engine for the issuer's own published
    forward guidance envelopes** (`[G_lo, G_hi]` per site, division, and consolidated group for production `koz` /
    `Mlb`, TCC `$/oz`, AISC `$/oz`, sustaining capex `$M`, and growth capex `$M`, plus the 5-year/10-year outlook
    decks) or the issuer's **published commodity-price royalty sensitivity slopes** (`ΔAISC / ΔP_Au`).
  - Yet in the corpus (`R6 §1.4`, `§4.2`, `E108`, `E278`, `E601`, `E602`), **guidance misses and unattributed
    guidance revisions are the #1 source of executive credibility loss, multi-billion-dollar equity drawdowns, and
    securities-litigation exposure** for Barrick and Newmont. For example, in Q1 2026, industry AISC rose +16%
    largely because higher realized gold prices mechanically inflated price-linked royalties (`FACT`, `R6 §4.2`),
    while operational misses at Carlin, Pueblo Viejo, and Loulo-Gounkoto had completely different physical roots
    (roaster/autoclave availability, plant-cone inventory swings `E666`, sovereign suspension `E602`).

### 1.2 Subsystems, Crates, Users & Workflows
- **Subsystems:** `Bullion`, `Helm`, `Pushback`, `Portal`, `Adit` (`§0.5`).
- **Crates (`§28.3`, zero new crates):**
  - `fmine-metrics` (E layer, budget 18k LOC, currently ~250 LOC): adds `GuidanceEnvelope`, `GuidanceCoherenceCert`,
    and the 6-factor `GuidanceVsActualBridge`.
  - `fmine-public` (V layer, budget 18k LOC, currently ~665 LOC): ingests published annual/quarterly guidance tables,
    multi-year outlook tables, and gold/copper price-sensitivity footnotes from filed MD&As and releases.
  - `fmine-decide` (E layer, budget 20k LOC, currently ~304 LOC): renders the `GuidanceReleaseDecisionCard`.
  - `fmine-cli` (U layer, budget 26k LOC, currently ~328 LOC): adds `fmine assure guidance` verb to
    `registries/verbs.toml`.
- **Users & Workflows:**
  1. **CFO, Investor Relations & Executive Committee (Pre-Release Workflow):** Before publishing annual or updated
     quarterly guidance, run `fmine assure guidance --draft <bundle>` to verify that (a) site-level guidance intervals
     mathematically aggregate to the consolidated guidance interval under `L_corp` / `L_JV` ownership and consolidation
     rules (`G-BULL-3`), (b) the joint probability of hitting all guided rows simultaneously is quantified under
     `Pushback`'s schedule-compliance kernel `K(plan, state)` (`§15.8`), and (c) the gold/copper price deck and royalty
     sensitivity slope are explicitly anchored.
  2. **Board Audit Committee, Equity Analysts & AI Agents (Post-Print Assurance Workflow):** On release day of each
     quarterly Mine Stats workbook, run `fmine assure guidance --issuer barrick --year 2025` to score every site and
     consolidated metric against the start-of-year frozen guidance envelope and decompose every beat or miss into
     six exact, non-overlapping drivers.

### 1.3 Technical & Mathematical Design (`DESIGN`)

#### (A) Pre-Release Multi-Site Guidance Coherence & The Sidak Trap
Let an issuer publish guidance intervals `I_s = [L_s, U_s]` for `s = 1..S` sites and a consolidated interval
`I_tot = [L_tot, U_tot]` on attributable basis `w_s ∈ (0, 1]`.

1. **Interval Consolidation Coherence (`G-BULL-8`, `FMINE-REF-109`):**
   For extensive quantities (produced/sold ounces, sustaining capex), any coherent joint distribution requires:
   ```text
   Σ_s w_s L_s  ≤  L_tot  ≤  U_tot  ≤  Σ_s w_s U_s    (for positive correlation ρ ≥ 0)
   ```
   and under the site covariance matrix `Σ_sites` (incorporating shared price/FX shocks and independent operational
   compliance `SC_s`), the implied consolidated `10%–90%` interval from site-level distributions `D_s` is:
   ```text
   μ_tot = Σ_s w_s μ_s,    σ²_tot = wᵀ Σ_sites w + μ²_L_corp
   ```
   For ratio metrics (`AISC = Cost / Oz_sold`), `fmine-metrics` checks whether `[L_tot^AISC, U_tot^AISC]` is
   reachable by any feasible vector `(Oz_s ∈ I_s^Oz, AISC_s ∈ I_s^AISC)` plus corporate G&A layer `L_corp`:
   ```text
   AISC_tot_min = min_{Oz_s ∈ I_s^Oz} [ (Σ_s w_s Oz_s L_s^AISC + L_corp_lo) / Σ_s w_s Oz_s ]
   AISC_tot_max = max_{Oz_s ∈ I_s^Oz} [ (Σ_s w_s Oz_s U_s^AISC + L_corp_hi) / Σ_s w_s Oz_s ]
   ```
   This fractional linear program over a box has an exact `O(S log S)` greedy solution (sorting sites by `L_s^AISC`).
   If `[L_tot^AISC, U_tot^AISC]` lies outside `[AISC_tot_min, AISC_tot_max]`, `fmine-metrics` refuses with
   `FMINE-REF-109 (IncoherentGuidanceEnvelope)`.

2. **Joint vs. Marginal Guidance Hit Rate (The Multi-Site Sidak Trap):**
   If management sets each of `S = 12` independent sites' guidance ranges `[L_s, U_s]` to cover a `70%` marginal
   probability (`P(Y_s ∈ I_s) = 0.70`), then the probability that **all 12 sites hit guidance in the same year** is:
   ```text
   P(∩_{s=1}^{12} {Y_s ∈ I_s}) = 0.70¹² = 0.0138 (1.4%!)
   ```
   And the expected number of sites missing guidance is `12 × 0.30 = 3.6 sites/year`! `GuidanceCoherenceCert`
   computes both the per-site marginal coverage `p_s = P(Y_s ∈ I_s | K(plan, state))`, the expected count of site
   misses `E[N_miss] = Σ_s (1 − p_s)`, and the portfolio probability `P(Y_tot ∈ I_tot)`.

#### (B) Exact 6-Factor Post-Print Guidance Attribution (`GuidanceVsActualBridge`)
When actual AISC `A_act` diverges from the guidance midpoint `A_guide = ½(L_AISC + U_AISC)` formed at guidance
price deck `P_guide` (e.g., `$2,400/oz` or `$3,000/oz`), the naive difference `ΔA = A_act − A_guide` confounds
market price windfalls (which raise royalties and production taxes) with operational execution.

`fmine-metrics` first shifts the guidance baseline onto the **Price-Normalized Guidance Benchmark**:
```text
A_guide_norm(P_act) = A_guide + β_royalty · (P_act − P_guide) + β_fx · (FX_act − FX_guide)
```
where `β_royalty` is the issuer's published royalty/tax sensitivity slope (e.g., `+$5/oz AISC per +$100/oz Au price`,
cited from the guidance footnote `SourceAnchor`). The total variance `A_act − A_guide` then decomposes **without
residual** (`ExactQty`-verified) into 6 additive terms via logarithmic mean `L(x, y) = (x − y)/(ln x − ln y)`:

```text
A_act − A_guide = ΔA_price_royalty          (1. Realized metal price & FX mechanical effect on royalties/taxes)
                + ΔA_sovereign_scope        (2. Force-majeure / suspension / divestment scope change, e.g. Loulo E602)
                + ΔA_tonnage_compliance     (3. Mined/milled dry tonnage vs guided throughput: ln(T_guide / T_act))
                + ΔA_head_grade_F1          (4. Feed grade & F1 dilution/selectivity vs guided grade: ln(g_guide / g_act))
                + ΔA_recovery_inventory_F2  (5. Plant recovery & GIC/heap/cone inventory swing: ln((R·B·S)_guide / (R·B·S)_act))
                + ΔA_unit_cost_spend        (6. Controllable cash + sustaining capex spend per tonne: ln(c_act / c_guide))
```

#### (C) Rust Type Sketch (`crates/fmine-metrics/src/lib.rs`)
```rust
#[derive(Debug, Clone, PartialEq)]
pub struct GuidanceRow {
    pub site_id: String,
    pub period_label: String,
    pub ownership_share: f64,               // 1.00 or attributable share (e.g. 0.615 NGM, 0.60 PV, 0.45 Kibali)
    pub prod_oz_range: (f64, f64),          // [L_oz, U_oz]
    pub tcc_usd_oz_range: (f64, f64),       // [L_tcc, U_tcc]
    pub aisc_usd_oz_range: (f64, f64),      // [L_aisc, U_aisc]
    pub gold_price_assumption_usd_oz: f64,  // e.g. 2400.0 or 2800.0
    pub royalty_aisc_slope_per_100usd: f64, // e.g. 5.0 $/oz per $100/oz Au move
    pub source_anchor: SourceAnchor,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GuidanceAttributionResult {
    pub site_id: String,
    pub guided_aisc_mid: f64,
    pub actual_aisc: f64,
    pub price_normalized_guidance_aisc: f64,
    pub delta_price_royalty: f64,
    pub delta_sovereign_scope: f64,
    pub delta_tonnage_compliance: f64,
    pub delta_grade_f1: f64,
    pub delta_recovery_inventory_f2: f64,
    pub delta_controllable_spend: f64,
    pub residual_error: f64, // PO-BULL-GUID-1: |residual_error| < 1e-9
    pub status: GuidanceOutcomeStatus, // Beat | WithinRange | OperationalMiss | PriceRoyaltyMissOnly
}
```

### 1.4 Worked Galaxy-Brain Card (`GB-44`, Illustrative Values Anchored to `R6 §4.2`)
- **Scenario:** A Complex guides FY production of `950–1,050 koz` (mid `1,000 koz`) and AISC of `$1,350–$1,450/oz`
  (mid `$1,400/oz`) at a `$2,400/oz` gold price assumption, with a published royalty sensitivity of `+$5.50/oz` per
  `+$100/oz` gold price.
- **Actual outcome:** Realized gold price averages `$3,200/oz` (`+$800/oz`). Production is `965 koz` (inside guidance,
  `−3.5%` below midpoint due to a 4% roaster shutdown), and actual AISC prints at `$1,472/oz` (`+$22/oz` above the
  top of guidance!).
- **Naive headline:** "Management missed AISC guidance (`$1,472` vs `$1,350–$1,450`)."
- **`GuidanceCredibilityBridge` output:**
  - `ΔA_price_royalty = (+800 / 100) × $5.50 = +$44.00/oz` → Price-normalized guidance range is `$1,394–$1,494/oz`
    (midpoint `$1,444/oz`).
  - Operational variance vs price-normalized midpoint is `+$28.00/oz`, decomposed as:
    - `ΔA_tonnage_compliance = +$36.40/oz` (roaster downtime reduced throughput);
    - `ΔA_grade_F1 = −$12.10/oz` (slightly higher grade fed);
    - `ΔA_recovery_inventory_F2 = +$6.20/oz` (GIC build);
    - `ΔA_controllable_spend = −$2.50/oz` (site unit spending was *below* budget!).
  - **Classification:** `PriceRoyaltyMissOnly` (actual `$1,472/oz` sits `$22/oz` *inside* the `$1,394–$1,494/oz`
    price-normalized guidance band, and unit cost spending beat budget by `$2.50/oz`).

### 1.5 First Shippable Slice, Testing Strategy & Kill Criterion
- **First shippable slice (M0 / WP-03):**
  - Add `GuidanceRow` registry entries for Barrick's published FY2024, FY2025, and FY2026 guidance tables (citing
    the Q4/annual MD&A locators and `sha256` hashes) in `fmine-public`, implement `GuidanceCoherenceCert` and
    `GuidanceVsActualBridge` in `fmine-metrics`, and expose `fmine assure guidance --issuer barrick --year 2025` in
    `fmine-cli`. Fits in **~650 LOC** across `fmine-metrics` and `fmine-public`.
- **Testing strategy (§33.1 Four Claims):**
  - **Parity:** Reproduces every published site and consolidated guidance range and price-sensitivity footnote in
    Barrick's 2024–2026 disclosures to `0` last-digit error.
  - **Known answers:** Synthetic `fmine-gen` multi-site company with injected `+$600/oz` gold price jump and `−8%`
    mill throughput drop recovers all 6 attribution terms to `< 1e-9` residual (`PO-BULL-GUID-1`), and refuses
    a seeded incoherent consolidated guidance range with `FMINE-REF-109`.
  - **Reconciliation:** Scores start-of-year guidance intervals as registered interval forecasts (`WinklerScore`,
    `crates/fmine-calib/src/lib.rs:205–221`) alongside FrankenMining's own `bd-067` forecasts.
- **Kill criterion (`KILL-BULL-GUID-1`):**
  - If two finance reviewers in `BD-075` find that price-normalized guidance attribution does not clarify quarterly
    variance reviews, or if `< 70%` of guided sites in public filings disclose enough denominator detail to separate
    volume from unit spend, demote factors 3–5 into a single `ΔA_volume_and_yield` term on the public lane and reserve
    the full 6-factor split for the M2 internal site lane.

---

## Idea 2: `WorkbookForensicAutopsy` — Pure-Rust `.xlsx` Formula-DAG Decompiler, Dimensional Type-Checker & Automated Discrepancy Root-Cause Explainer

### 2.1 What Is Already in the Plan vs. What Is Materially New
- **Already in the plan:**
  - `§21.6`, `§32.1`, `BD-271`, and `crates/fmine-io/src/lib.rs` implement a pure-Rust ZIP/DEFLATE `.xlsx` reader that
    extracts `<v>` cell values, checks if the raw XML string contains `"#REF!"` (`G-BULL-1`), and checks
    `workbook.xml` for `state="hidden"` (`G-BULL-2`).
  - `crates/fmine-public/src/lib.rs` compares extracted cell numbers against expected arithmetic identities (e.g.
    `TCC = OpCost + Royalties`, `AISC = TCC + SustCapex + ...`) and logs a `DiscrepancyEntry` when `|diff| > tol`.
- **What is missing (and materially new here):**
  - Right now, when an `.xlsx` cell fails an identity check in `fmine-public` (such as the corpus discrepancies
    `E113–E121`, `E144`, `E147`, `E153`, `E193`, or client workbooks in M2), `fmine-io` **ignores the `<f>...</f>`
    formula tags** that sit right above `<v>...</v>` inside `xl/worksheets/sheet*.xml`!
  - Because it discards `<f>`, FrankenMining can only report *that* cell `G42` is off by `4.12 koz`—it cannot tell
    the CFO, controller, or QP **why** `G42` is wrong!
  - By adding a ~1.1k LOC pure-Rust **Spreadsheet Formula AST Parser, Dependency-Graph Builder, and Dimensional
    Theorem Checker (`fmine-io::xlsx_forensics`)**, FrankenMining automatically decompiles every `.xlsx` workbook
    on ingest, propagates `fmine-units` dimensions and bases through the cell DAG, detects the exact structural
    spreadsheet bugs that plague mining finance and geology teams, and attaches a machine-verified **Formula Root
    Cause** to every `DiscrepancyEntry`!

### 2.2 Subsystems, Crates, Users & Workflows
- **Subsystems:** `Portal`, `Bullion`, `Plumb`, `Adit` (`§0.5`).
- **Crates (`§28.3`, zero new crates):**
  - `fmine-io` (V layer, budget 40k LOC, currently ~450 LOC): adds `CellFormulaAst`, `SheetDependencyDag`, and the
    5 structural formula detectors in `xlsx_forensics`.
  - `fmine-public` (V layer, budget 18k LOC): enriches `DiscrepancyEntry` with `FormulaAutopsyReport`.
  - `fmine-cli` (U layer): adds `fmine assure workbook <path.xlsx>` for standalone forensic auditing of any mining
    workbook (Mine Stats, LOM cash-flow model, monthly metallurgical balance sheet, reserve roll-forward table).
- **Users & Workflows:**
  1. **M0 Public Assurance & Equity/IR Teams:** Automatically diagnoses why published Mine Stats or TR tables break
     identities (`E113–E121`) without manual reverse-engineering.
  2. **CFO / Mine Controller / Metallurgist / QP (M2 Site Pilot):** Before signing a monthly metallurgical balance,
     reserve table, or board pack built in Excel, run `fmine assure workbook` to catch omitted rows, hardcoded plug
     numbers, dragged-cell reference shifts, and `oz/st` vs `g/t` unit mixing in `< 200 ms`.

### 2.3 Technical & Mathematical Design (`DESIGN`)

#### (A) Extracting and Parsing `<f>` Tags in `xl/worksheets/sheet*.xml`
In OpenXML (`ECMA-376`), every formula cell in `sheetN.xml` has the form:
```xml
<c r="H24" s="12"><f>SUM(H14:H22)+14.2</f><v>412.8</v></c>
```
or shared-formula form `<f t="shared" ref="D14:M14" si="3">D12*D13</f>`. `fmine-io` expands shared formulas using
relative R1C1 offsets and parses each formula into a compact, zero-unsafe recursive-descent AST:

```rust
#[derive(Debug, Clone, PartialEq)]
pub enum FormulaExpr {
    CellRef { sheet: Option<String>, col: u32, row: u32, abs_col: bool, abs_row: bool },
    RangeRef { sheet: Option<String>, start: (u32, u32), end: (u32, u32) },
    LiteralNumber(f64),
    LiteralText(String),
    ErrorLiteral(String), // #REF!, #DIV/0!, #VALUE!, #NAME?
    Unary { op: UnaryOp, expr: Box<FormulaExpr> },
    Binary { op: BinaryOp, lhs: Box<FormulaExpr>, rhs: Box<FormulaExpr> },
    FuncCall { name: String, args: Vec<FormulaExpr> }, // SUM, AVERAGE, IF, ROUND, VLOOKUP, INDEX, MATCH, SUMPRODUCT
}
```

#### (B) Five Structural Spreadsheet Defect Detectors (`G-PORT-XLSX-1..5`)
Once the workbook's `SheetDependencyDag` is built and row/column labels are anchored (`BD-271`), `fmine-io` runs
five deterministic graph/AST analyzers:

1. **`HardcodedPlugInFormula` (`G-PORT-XLSX-1`):**
   Flags any cell in an aggregation row/column (`Total`, `Subtotal`, `AISC`, `TCC`, `Contained Metal`, `Balance`)
   whose AST combines cell/range references with an unlabelled `LiteralNumber(k)` (where `k ∉ {0, 1, 100, 1000}` and
   `k` is not a registered exact unit constant from `fmine-units` such as `31.1034768` or `0.90718474`).
   *Catches silent plug numbers buried inside formulas (`=SUM(D10:D18) - 12.4`), enforcing `INV-005` at the
   spreadsheet import boundary!*
2. **`RaggedSumOmittedAdjacentRow` (`G-PORT-XLSX-2`):**
   When `SUM(C_{r1}:C_{r2})` sits in row `r_sum > r2 + 1` (or when `r1 > r_header + 1`), checks whether the
   omitted adjacent cells `C_{r2+1} .. C_{r_sum-1}` contain numeric values of the same semantic table block. If
   adding the omitted cell(s) closes a downstream identity break within rounding tolerance, emits an **Exact
   Omitted-Row Proof**!
3. **`HorizontalDragSymmetryBreak` (`G-PORT-XLSX-3`):**
   Across a time-series row `(col_1, col_2, ..., col_T)` representing quarters or years, canonicalizes each cell's
   `FormulaExpr` into relative `R[Δr]C[Δc]` normal form. Flags any cell `col_t` whose normalized AST differs from its
   neighbours (e.g., 9 quarters compute `=R[-2]C[0]*R[-1]C[0]`, but `2025Q3` has a literal number or points to
   `R[-2]C[-1]`, automatically catching copied-cell bugs like `E114`!).
4. **`NaiveUnweightedGradeAverage` (`G-PORT-XLSX-4`):**
   Propagates `fmine-units` types (`DryTonnes`, `WetTonnes`, `Grade`, `Recovery`, `MoneyPerOz`) from label-anchored
   rows through the `FormulaExpr` DAG:
   - `AVERAGE(Range)` where `Range` has inferred dimension `Grade` or `AiscPerOz` or `Recovery` (intensive
     quantities!) instead of `SUMPRODUCT(Tonnes, Grade) / SUM(Tonnes)` triggers `FMINE-REF-103` (`NonAdditiveAverage`)
     with the exact cell coordinate and the mass-weighted correction!
5. **`InexactUnitConversionConstant` (`G-PORT-XLSX-5`):**
   Inspects multiplication/division literals against `fmine-units` exact rationals (`§25.2`): flags approximate
   constants such as `34.29` instead of `240/7` (`34.285714...` for `oz/st → g/t`) or `0.02917` instead of
   `0.0291666...` (`E171`), quantifying the exact ounce drift caused by the truncated constant.

### 2.4 Worked Galaxy-Brain Card (`GB-45`, Illustrative Values)
- **Scenario:** A regional consolidation sheet reports Total Gold Produced across 6 mines as `482 koz` in cell `J28`,
  while summing the 6 published site rows `J21:J26` gives `501 koz` (`Δ = −19 koz`).
- **Standard importer:** Logs `Discrepancy: J28 expected 501, actual 482 (diff -19)`. An analyst spends 45 minutes
  opening Excel and tracing precedents.
- **`WorkbookForensicAutopsy` output (in `< 5 ms`):**
  ```text
  [G-PORT-XLSX-2: RaggedSumOmittedAdjacentRow]
  Cell:       2025Q2_MineStats.xlsx:Summary!J28
  Formula:    =SUM(J21:J25)
  Defect:     Range J21:J25 stops 1 row short of contiguous numeric row J26 ("Porgera (47.5%)": 19.0 koz).
  Counterfactual Replay: Evaluating =SUM(J21:J26) yields 501.0 koz (residual vs identity = 0.0 koz).
  ```

### 2.5 First Shippable Slice, Testing Strategy & Kill Criterion
- **First shippable slice (M0 / WP-03 & WP-11):**
  - Extend `crates/fmine-io/src/lib.rs` to extract `<f>` tags alongside `<v>`, parse the core Excel formula grammar
    (`CellRef`, `RangeRef`, `+`, `-`, `*`, `/`, `SUM`, `AVERAGE`, `IF`, `ROUND`, `SUMPRODUCT`), and run detectors
    `G-PORT-XLSX-1..5` during `run_barrick_m0_assurance` (`crates/fmine-public/src/lib.rs`).
- **Testing strategy (§33.1 Four Claims):**
  - **Parity:** All 18 Barrick workbooks (`2024Q1–2026Q2`) parse with 0 panics; every formula cell's AST-evaluated
    value matches Excel's cached `<v>` value to `< 1e-9` relative (`PO-PORT-XLSX-1`).
  - **Known answers:** Seeded synthetic workbooks containing each of the 5 defects (hidden `+14.2` plug, omitted row
    in `SUM`, dragged `C[-1]` typo, `=AVERAGE(grade_range)`, and `34.29` opt factor) are caught 100% of the time with
    exact counterfactual closure.
  - **Performance:** Full AST parse + DAG audit of all 18 workbooks (`~70,000` cells) completes in `< 150 ms` single-core.
- **Kill criterion (`KILL-PORT-XLSX-1`):**
  - If > 15% of formulas in target workbooks use opaque array/macro functions (`OFFSET`, `INDIRECT`, `LAMBDA`) that
    prevent static DAG resolution, restrict automated root-cause replay to the static subgraph and flag `INDIRECT`/
    `OFFSET` cells as `UnverifiableFormulaTopology`.

---

## Idea 3: `MineToMillShadowPriceLoop` — Shift-Scale Bottleneck-Regime Classifier, Refractory Feed-Stability Chance Envelope & Live Dual-Price Broadcast

### 3.1 What Is Already in the Plan vs. What Is Materially New
- **Already in the plan:**
  - `§15.3` and `§15.6` define strategic/tactical LP duals (`ShadowPrice` on mill hours and blend rows) and two-sided
    refractory windows (`BD-319`: Carlin roasters `E172`, Goldstrike autoclave acid/alkaline modes `E659`, Pueblo Viejo
    direct POX vs float+POX `E265`, `E664`).
  - `§16.8` (`BD-320`) defines `EnergyEnvelope` with a power shadow price `λ_MWh(t)` (`E668`, `E669`).
  - `§17.8` and `§17.10` (`GB-19`) note that whether a plant should push tonnes (coarse grind) or chase recovery (fine
    grind) depends on whether the mine or mill binds.
- **What is missing (and materially new here):**
  1. **Real-Time Bottleneck Regime Flips Across Subsystems:** In operating complexes (NGM Carlin/Cortez/Turquoise Ridge,
     Pueblo Viejo, Kibali, Lumwana), **the active bottleneck flips inside a week or shift**—from `MineHaulageLimited`
     (shovel/AHS rain slowdown) to `MillComminutionPowerLimited` (hard ore + dry-season hydro/diesel tariff `E669`) to
     `RefractorySulphurOxidationLimited` (autoclave/roaster `Cap_S` or `O2` plant limit `E265`) to
     `CarbonateOrPregRobWindowLimited` (`CO3/SS` or `TCM` spike `E172`, `E173`) to `TailingsRateOfRiseLimited` (`B̄ >
     B̄_crit` in `Freeboard`, `GB-42`). Currently, `Pushback` computes duals in a strategic/monthly LP, leaving shift
     grade-control geologists, dispatchers, and plant metallurgists flying blind when a constraint shifts mid-week!
  2. **High-Frequency Stoichiometric Variance in Refractory Feeds:** `BD-319` enforces two-sided windows (`SS ∈
     [0.8%, 1.8%]`, `CO3 > 14%`, `TCM ∈ [0.8%, 1.2%]`) on **period-average** LP tonnes. But a roaster or autoclave
     has a **2-to-8 hour thermal/stoichiometric residence time**: a weekly average of `1.5% SS` made by alternating
     `0.5% SS` and `2.5% SS` shifts **extinguishes the roaster bed on Monday and trips the autoclave on high
     temperature/pressure on Tuesday**!

### 3.2 Subsystems, Crates, Users & Workflows
- **Subsystems:** `Pushback`, `Crucible`, `Headframe`, `Touchstone`, `Adit` (`§0.5`).
- **Crates (`§28.3`, zero new crates):**
  - `fmine-plan` (K3 layer, budget 30k LOC): adds `BottleneckRegimeClassifier`, `ShiftDualBroadcastCard`, and the
    short-horizon **Variance-Constrained Refractory Blend QP/SOCP-outer-LP** in `fmine-plan::mpc`.
  - `fmine-plant` (K2 layer, budget 32k LOC): maps live dual vector `λ_shift` onto the `(Q_tph, P80, ΔR)` grind-mode
    frontier (`GB-19`) and computes high-frequency surge-tank feed variance after RTD damping (`BD-313`).
  - `fmine-recon` (K2 layer, budget 18k LOC): supplies parcel-level chemical covariance `(Var(SS), Var(CO3),
    Var(TCM))` from `StockpileVoxelTwin` (`BD-312`).
  - `fmine-tui` / `fmine-cli` (U layer): displays the live Shift Shadow-Price & Bottleneck Console (`FMT-DUAL-1`).
- **Users & Workflows:**
  - **Short-Term Planner & Grade-Control Geologist:** Sees live `$/t` marginal penalties/credits for hardness, `S%`,
    `CO3%`, `TCM%`, and moisture at every shovel face and stockpile finger before issuing shift dig lines
    (`EFFECT-DIGLINE`).
  - **Plant Metallurgist:** Sees the exact `P80*` grind target and residence-time set-point recommendation
    (`EFFECT-PLANT-SETPOINT`) induced by today's active bottleneck duals.
  - **Maintenance Planner & Site GM:** Sees the exact live opportunity cost (`$/hour`) of taking any shovel, crusher,
    ball mill, roaster, or autoclave offline in the next 48 hours.

### 3.3 Technical & Mathematical Design (`DESIGN`)

#### (A) The Shift Bottleneck-Regime Classifier & Dual Broadcast (`FMT-DUAL-1`)
At each shift boundary (or whenever a state-transition event lands in `CELL-TEL` / `CELL-FLOW`, such as an autoclave
derate, shovel down, or rain speed cap), `fmine-plan::mpc` solves the 14-day receding-horizon LP over available
mining faces and `StockpileVoxelTwin` fingers `s ∈ S_avail` in `< 2 s`:

```text
max  Σ_{τ=1}^{28 shifts} δ_τ Σ_{s,p} x_{spτ} · Margin_{sp}(P_deck)
s.t. Σ_s x_{spτ}                  ≤ Cap_mass_{p,τ}       [dual: λ_tonnes_{p,τ}   $/t]
     Σ_s x_{spτ} · e_grind(s, p)  ≤ P_mill_{p,τ} · Δt    [dual: λ_power_{p,τ}    $/MWh]
     Σ_s x_{spτ} · S_s            ≤ Cap_S_{p,τ}          [dual: λ_sulphur_{p,τ}  $/t-S]
     Σ_s x_{spτ} · (L_e − a_{s,e}) ≤ 0                   [dual: λ_floor_{p,e,τ}  $/unit-e]
     Σ_s x_{spτ} · (a_{s,e} − U_e) ≤ 0                   [dual: λ_ceil_{p,e,τ}   $/unit-e]
     Σ_p x_{spτ}                  ≤ HaulCap_{s,τ}        [dual: λ_mine_{s,τ}     $/t]
     Σ_{s,p} x_{spτ} · v_tail(s)  ≤ Cap_TSF_rise_τ       [dual: λ_tsf_τ          $/m³]
```
From the optimal basis and its parametric sensitivity intervals `[rhs_lo, rhs_hi]`, `fmine-plan` classifies the
shift into one of six **Canonical Operational Regimes**:
1. `MineSupplyLimited` (`λ_mine > 0`, `λ_power = λ_sulphur = 0`) → Plant runs **fine grind** to maximize recovery
   (`GB-19`); haul-road grading and extra trucks at the ore shovel have maximum ROI (`GB-17`).
2. `ComminutionEnergyLimited` (`λ_power > 0` binds first) → Plant runs **coarse grind** (`P80 ↑`) and diverts
   high-`BWi` parcels to stockpile while pulling soft/oxidized fingers.
3. `SulphurOxidationLimited` (`λ_sulphur > 0` at POX/roaster) → Block and stockpile ranking switches from `$/t` to
   **`$/kg of Sulphide Sulphur`** (`NSR_s / S_s`)! High-Au/low-S parcels (e.g. Fourmile single-refractory silicified
   zone `E656`) jump to the front of the queue.
4. `StoichiometricWindowPinched` (`λ_floor_{CO3}` or `λ_ceil_{TCM}` or `λ_floor_{SS}` binds) → "Sweetener" stockpile
   fingers (high-carbonate or low-TCM ore) earn a quantified `$/t` blending premium even if their gold grade is near
   the marginal cut-off!
5. `GridPowerOrTariffPinched` (`EnergyEnvelope` `λ_MWh` spikes in dry-season diesel/BESS tier `E668`, `E669`) →
   Defers hoisting/dewatering to solar/off-peak hours and feeds lowest-specific-energy ore during peak tariff hours.
6. `TailingsOrWaterPinched` (`λ_tsf > 0` from `B̄` rate-of-rise or pond freeboard `GB-21`, `GB-42`) → Maximizes
   grade per tonne of tailings volume; halts low-grade marginal stockpile reclaim.

#### (B) High-Frequency Refractory Feed-Stability Chance Constraint
To prevent shift-to-shift feed swings from tripping a roaster or autoclave while satisfying the monthly average,
let each source parcel `s` have chemical mean `μ_{s,e}` and within-parcel truckload variance `σ²_{s,e}` (from
`Strata` / `StockpileVoxelTwin`), damped by the feed-surge & grinding circuit residence-time kernel `h` (`BD-313`)
with effective mixing number of truckloads `N_mix = (τ_surge / t_truck) · H_bed`:
```text
Var_surge(a_e) = (1 / N_mix) · Σ_s (x_{spτ} / X_{pτ}) · [ σ²_{s,e} + (μ_{s,e} − ā_{p,e,τ})² ]
```
The second term `(μ_{s,e} − ā)²` is the **inter-finger heterogeneity penalty**: blending a `0.2% SS` oxide finger with
a `3.2% SS` sulphide finger to hit a `1.7% SS` average creates massive truck-by-truck variance unless homogenized on a
chevron bed (`H_bed ≫ 1`)! Because `(x_{spτ} / X_{pτ})` is linear in the blend shares `w_s = x_{spτ} / X_{pτ}`, the
chance constraint `P(a_e(t) ∈ [L_e, U_e]) ≥ 1 − α` (with `z_{1-α/2}` quantile) is **linear in the blend shares `w_s`**
around the target window midpoint `m_e`:
```text
Σ_s w_s · [ μ_{s,e} + (z_{1-α/2} / √N_mix) · √(σ²_{s,e} + (μ_{s,e} − m_e)²) ] ≤ U_e
Σ_s w_s · [ μ_{s,e} − (z_{1-α/2} / √N_mix) · √(σ²_{s,e} + (μ_{s,e} − m_e)²) ] ≥ L_e
```
This keeps the receding-horizon problem a **pure LP** (`fmine-lp::pdlp` / simplex) while penalizing erratic two-end-member
blends that would trip the roaster or autoclave!

### 3.4 Worked Galaxy-Brain Card (`GB-46`, Illustrative Values Anchored to Carlin/PV `E172`, `E265`)
- **Scenario:** Goldstrike Roaster target window is `SS ∈ [0.8%, 1.8%]` (`E172`). The planner needs `12,000 t/day`
  at `1.5% SS`.
  - **Plan A (Direct 2-Shovel Blend):** `6,000 t` from Face 1 (`0.4% SS ± 0.2%`) + `6,000 t` from Face 2 (`2.6% SS ± 0.5%`),
    dumped directly into the crusher (`N_mix = 6` truckloads per surge-bin turnover).
  - **Plan B (Bed-Blended Stockpile + Moderated Face):** Reclaim via chevron bed (`N_mix = 48`) or substitute Face 3
    (`1.4% SS ± 0.3%`) at `0.3 g/t` lower gold grade.
- **Result:**
  - Plan A has mean `1.50% SS`, **passing a naive LP**. But its surge-bin SD is:
    `σ_surge = √((1/6) · [0.5·(0.04 + 1.21) + 0.5·(0.25 + 1.21)]) = 0.51% SS`!
  - Under Plan A, `P(SS > 1.8% thermal overload) = 27.8%` and `P(SS < 0.8% bed extinguish) = 8.5%`—nearly **1 in 3
    four-hour windows trips or derates the roaster** (losing `$180k` per derate)!
  - With the linear chance-constraint coefficient active, `fmine-plan::mpc` refuses Plan A unless routed through the
    homogenisation bed (`σ_surge = 0.18%`) or blended with Face 3 (`σ_surge = 0.14%`), saving `+$1.4M/month` net of
    the `0.3 g/t` grade trade-off.

### 3.5 First Shippable Slice, Testing Strategy & Kill Criterion
- **First shippable slice (M2–M4 / WP-10 & WP-19):**
  - Implement `BottleneckRegimeClassifier`, the linear chance-constrained refractory window coefficients, and
    `ShiftDualBroadcastCard` in `fmine-plan` + `fmine-plant` (~850 LOC), with fixtures for Carlin (`Tables 13-7..13-16`,
    `E172`, `E659`) and Pueblo Viejo (`E265`, `E664`).
- **Testing strategy (§33.1 Four Claims):**
  - **Parity:** When `σ²_{s,e} = 0` and `N_mix → ∞`, reproduces the `BD-319` deterministic LP objective and duals to
    `1e-7`.
  - **Known answers:** On a synthetic 30-day shift simulation with stochastic truck arrivals and an autoclave `Cap_S`
    derate on Day 12, the classifier switches from `ComminutionEnergyLimited` to `SulphurOxidationLimited` within 1
    shift and reduces roaster window violations from `31%` to `< 3%` (at nominal `95%` chance level).
  - **Performance:** 28-shift × 50-parcel receding-horizon LP solves in `< 250 ms` on one core.
- **Kill criterion (`KILL-PUSH-DUAL-1`):**
  - If shift-level duals exhibit excessive Basis Flip Churn (`M-PUSH-5` plan-churn monitor > 2 regime flips per week
    from sensor noise), apply hysteresis deadbands on RHS capacities and broadcast daily rather than per-shift duals.

---

## Idea 4: `RpeeeAndBlockModelForensics` — 4-Factor 3D Block-Model Version Shapley Attribution, Parametric Break-Even Shell Field `λ*(b)` & Sterilization Certifier

### 4.1 What Is Already in the Plan vs. What Is Materially New
- **Already in the plan:**
  - `§15.4` (`BD-250`) plans `fmine-optimize::pitflow` (Hochbaum pseudoflow on `i64` CSR warm-started across revenue
    factors `λ`) to compute nested pit shells.
  - `§24.2` (`BD-070`, `BD-222`) tracks table-level annual reserve roll-forwards (`depletion, infill, economics,
    methodology, M&A`) and `BD-223` builds a `ClassificationSupportPack` (kriging efficiency, slope of regression).
  - `§13.9` and `BD-191` lift `VdbGrid` into `fmine-geom::vdb` with conservative cell remapping (`fmine-geom::remap`).
- **What is missing (and materially new here):**
  1. **3D Brick-Level Block-Model Version Forensics (`BlockModelVersionDiff`):** When a site imports its 2026 vendor
     block model (from Leapfrog, Vulcan, Datamine, or Surpac in M2, or a new `Strata` `BeliefSpec` in M3+) and total
     contained gold drops by `380 koz` relative to the 2025 model, table-level roll-forwards cannot verify *where* or
     *why* the ounces moved! Currently, QPs rely on manual spreadsheet adjustments between categories. Using
     `fmine-geom::remap` on `VdbGrid`, FrankenMining can compute an **Exact 4-Factor Spatial Shapley Decomposition
     Brick** (`Depletion + ΔVolume_Wireframe + ΔDensity + ΔGrade_Capping`) for every `(x, y, z)` brick in the deposit!
  2. **Block-Level RPEEE Break-Even Revenue Factor Field `λ*(b)` & Orphan-Pod / Sterilization Certifier:** Under
     S-K 1300, NI 43-101, and JORC (`§24`), every Mineral Resource block must have Reasonable Prospects for Eventual
     Economic Extraction (RPEEE). Rather than running one static `$1,900/oz` resource shell (`E141`), parametric
     pseudoflow immediately yields for **every block `b` in the 3D grid** its exact **Minimum Break-Even Revenue
     Factor `λ*(b)`**—the lowest price/cost ratio at which `b` is paid for by its own net value plus the cone of
     overlying waste/ore it must carry! Storing `λ*(b)` as a 3D `VdbGrid` attribute enables:
     - **Resource Fragility Curves:** Exact ounces at risk for every `5%` cost inflation or recovery drop;
     - **Underground Orphan-Pod Detection (`G-HALL-7`):** Flags isolated above-COG blocks that cannot form a
       contiguous minimum-stope volume (`E186`);
     - **Infrastructure Sterilization Proofs (`SterilizationWitness`):** Proves whether a proposed waste dump, TSF
       (e.g. Naranjo `E667`), or plant site sits inside the closure cone of any `λ*(b) ≤ 1.5` resource or `Lodestar`
       endowment anomaly!

### 4.2 Subsystems, Crates, Users & Workflows
- **Subsystems:** `Hallmark`, `Strata`, `Pushback`, `Portal` (`§0.5`).
- **Crates (`§28.3`, zero new crates):**
  - `fmine-geom` (K0 layer, budget 30k LOC): implements `remap_and_decompose_bricks(grid_t0, grid_t1, as_mined_solid)`.
  - `fmine-optimize` (E layer, budget 36k LOC): emits `LambdaStarField` (`u16` quantized `λ*(b)` in `0.001` steps)
    directly from parametric `pitflow` (`BD-250`) at zero extra asymptotic cost.
  - `fmine-reserves` (K4 layer, budget 20k LOC): emits `BlockModelDiffCert`, `RpeeeFragilityCurve`, `OrphanPodReport`,
    and `SterilizationWitness` into the QP `ClassificationSupportPack`.
- **Users & Workflows:**
  - **Qualified Person (QP) & Resource Audit Team:** Runs `fmine reserves diff <model_2025.omf> <model_2026.omf>
    --depletion <survey_2025.laz>` to get a mathematically exact, brick-by-brick 3D audit of every ounce change
    before signing the annual statement (`INV-002` preserved: supplies evidence for the QP act, never classifies).
  - **Mine Planner, Tailings Engineer & Corporate Development:** Runs `fmine reserves check-sterilization
    --footprint <naranjo_tsf.dxf>` to verify what `λ*(b)` shells lie beneath a proposed surface facility.

### 4.3 Technical & Mathematical Design (`DESIGN`)

#### (A) Exact 4-Factor Spatial Shapley Attribution on `VdbGrid` Bricks
For any spatial control volume (a `VdbGrid` 8³ brick, bench, or domain polygon) between model version `0` and
model version `1` (after remapping rotated/sub-blocked cells of model `0` onto model `1`'s frame via exact polyhedral
intersection `fmine-geom::remap`), contained metal in version `k ∈ {0, 1}` is:
```text
M_k = V_avail,k · V_ore_prop,k · ρ_k · g_k
```
where `V_avail,k` is unmined volume (after intersecting with the `L5` depletion solid `§16.2`), `p_k = V_ore_prop,k`
is the wireframe proportion (`fmine-geom::winding`), `ρ_k` is dry bulk density (`t/m³`), and `g_k` is dry grade.
First, mining depletion is separated exactly on the version-`0` state:
```text
ΔM_depletion = − (V_avail,0 − V_avail,1) · p_0 · ρ_0 · g_0
```
On the remaining unmined volume `V_avail,1`, the 3-factor product change `p_1 ρ_1 g_1 − p_0 ρ_0 g_0` is decomposed
by the **Exact Symmetric 3-Factor Shapley / Sun Identity** (or LMDI where all factors `> 0`), which is **100%
path-independent, symmetric, and has zero residual even when `p_0 = 0` or `p_1 = 0`** (where logarithmic LMDI is
singular because wireframe boundaries shift!):
```text
Δ(p·ρ·g) = Δp · [ ½(ρ_0 g_0 + ρ_1 g_1) + ⅙ Δρ Δg ]     (1. ΔM_wireframe_volume: contact/shell geometry shift)
         + Δρ · [ ½(p_0 g_0 + p_1 g_1) + ⅙ Δp Δg ]     (2. ΔM_density: bulk density model change)
         + Δg · [ ½(p_0 ρ_0 + p_1 ρ_1) + ⅙ Δp Δρ ]     (3. ΔM_grade_estimator: infill/variogram/capping change)
```
Every brick satisfies `M_1 − M_0 = ΔM_depletion + ΔM_wireframe_volume + ΔM_density + ΔM_grade_estimator` to machine
precision (`PO-HALL-DIFF-1: max |residual| ≤ 1e-12`), and summing over all bricks in the `VdbGrid` reproduces the
deposit-wide reserve roll-forward table automatically!

#### (B) Parametric Break-Even Field `λ*(b)` & Sterilization Cone
In parametric maximum closure (`Hochbaum 2008` pseudoflow), block net value at revenue factor `λ > 0` is:
```text
v_b(λ) = λ · Rev_b(P_0) − Cost_b
```
Because `v_b(λ)` is strictly affine and increasing in `λ` for any block with `Rev_b > 0`, the nested closure family
`C(λ₁) ⊆ C(λ₂)` for `λ₁ < λ₂` (`PO-PUSH-3`) guarantees that each block `b` has a unique **Break-Even Entry Factor**:
```text
λ*(b) = inf { λ ∈ [λ_min, λ_max] : b ∈ Closure(λ) }
```
For a Proposed Surface Facility Footprint `F` (e.g., a waste dump or TSF polygon at surface plus its geotechnical
offset cone `Cone_down(F)` at slope angle `θ_setback`), any block `b ∈ Cone_down(F)` cannot be mined without
relocating facility `F` at cost `C_reloc`. The **Sterilized Resource Curve** `S_F(λ)` is therefore:
```text
S_F(λ) = Σ_{b ∈ Closure(λ) ∩ Cone_down(F)} Metal(b)
```
emitted as a certified table across `λ ∈ {0.8, 1.0 (Reserve), 1.25 (Resource), 1.5, 2.0}`!

### 4.4 Worked Galaxy-Brain Card (`GB-47`, Illustrative Values)
- **Scenario:** A QP reviews an annual update where Domain 4 Indicated Gold fell from `1,200 koz` to `1,040 koz`
  (`−160 koz`). The site table labels the entire `−160 koz` as "Infill drilling & model update".
- **`BlockModelVersionDiff` output on the 3D `VdbGrid`:**
  - `ΔM_depletion = −42.0 koz` (mined in Q1–Q4);
  - `ΔM_wireframe_volume = +18.5 koz` (infill widened the shear zone on benches 4100–4200);
  - `ΔM_density = −94.2 koz` (someone changed Domain 4 default dry density from `2.78` to `2.58 t/m³` without
    new pycnometer measurements!);
  - `ΔM_grade_estimator = −42.3 koz` (top-cut lowered from `60 g/t` to `40 g/t`);
  - **Sum = `−160.0 koz` (residual `0.000 koz`).**
- **Impact:** Immediately surfaces that **59% of the resource loss (`−94.2 koz`) was an unflagged bulk-density
  parameter change**, not infill drilling!

### 4.5 First Shippable Slice, Testing Strategy & Kill Criterion
- **First shippable slice (M2 / WP-11 & WP-13):**
  - Implement the 4-factor Shapley brick decomposition in `fmine-geom` + `fmine-reserves` (~500 LOC) over two imported
    block models (`CSV`/`OMF`), and emit `LambdaStarField` + `SterilizationWitness` once `pitflow` lands in `BD-250`.
- **Testing strategy (§33.1 Four Claims):**
  - **Parity:** `λ*(b) ≤ 1.0` set matches the `λ = 1.0` MineLib UPIT closure set bit-for-bit (`GATE-PUSH-PIT1`).
  - **Known answers:** On synthetic `fmine-gen` block models with known injected wireframe shift (`+5 m`), density
    shift (`−5%`), and grade bias (`−10%`), the 3-factor Shapley formula recovers the exact analytical attribution
    and sums to total `ΔMetal` within `1e-12` even when blocks enter/exit the wireframe (`p = 0 ↔ p = 1`).
  - **Performance:** 4-factor diff over `10⁷` active `VdbGrid` cells runs in `< 1.5 s` single-core.
- **Kill criterion (`KILL-HALL-DIFF-1`):**
  - If vendor exports at the M2 pilot lack block-proportion attributes (`p_k ∈ [0, 1]`) and export only binary centroid
    flags, evaluate `p_k ∈ {0, 1}` at sub-block resolution and flag sub-block boundary discretization width in the
    certificate.

---

## Idea 5: `AgentSandboxAndProseVerifier` — Zero-Copy CoW Counterfactual Ledger Overlay (`ClaimColor::Hypothetical`) & Hallucination-Proof Prose Citation Verifier

### 5.1 What Is Already in the Plan vs. What Is Materially New
- **Already in the plan:**
  - `§30.4` and `§32.3` define the agent security model: agents (including Avathon, `E024`) read anchor-pinned
    projections, cannot mint `HumanAuthorityToken`s, and write only quarantined `AgentUpload` proposals (`CELL-QUAR`).
  - `§30.5` and `BD-123` define `fmine-report` rendering deterministic Markdown/Mermaid reports from templates.
- **What is missing (and materially new here):**
  1. **Multi-Subsystem Counterfactual Sandbox (`SandboxedOverlay<AnchorHash>`):** Because agents (and exploratory
     human sessions) are rightly barred from writing to authority cells (`§30.4`), how does an executive's AI agent
     answer a compound counterfactual question that spans five subsystems—such as:
     *"If gold drops to $2,300/oz, we delay the Goldrush ramp-up by 6 months, and we cap Goldstrike Roaster As at
     1,200 ppm, what happens to our 2027 AISC, NGM JV cash distributions, and covenant DSCR?"*
     Without a first-class **in-memory Copy-on-Write (CoW) ledger overlay**, an agent cannot chain hypothetical inputs
     through `Strata → Pushback → Crucible → Bullion → Helm`!
  2. **Mechanical Verification of Free-Form Agent Prose (`fmine verify-prose`):** While `fmine-report` generates
     fixed template tables, executives, IR teams, and QPs use LLMs over `fmine-mcp` to write free-form board memos,
     MD&A drafts, and technical-report narratives. LLMs routinely **drop uncertainty intervals, strip `Uncalibrated`
     flags, conflate `100%` project basis with attributable basis, or slip banned words ("safe", "compliant",
     "proven reserve") into prose** when summarizing tool calls!

### 5.2 Subsystems, Crates, Users & Workflows
- **Subsystems:** `Adit`, `Bedrock`, `Plumb`, `Testpit` (`§0.5`).
- **Crates (`§28.3`, zero new crates):**
  - `fmine-claim` / `fmine-evidence` (S0 layer): adds `HypotheticalWatermark { base_anchor: ObjectId, delta_hash:
    [u8; 32] }` on claims produced inside a sandbox, with compile-time and runtime refusal (`FMINE-REF-305:
    HypotheticalClaimInAuthorityPath`) if any hypothetical claim reaches `CELL-POST`, `CELL-RSV`, or public export.
  - `fmine-ledger` (S1 layer, budget 24k LOC): adds `SandboxedOverlay`, an ephemeral, bounded-RAM (`≤ 256 MiB`
    `Budget`), capability-stripped (`CapSet<0,0,0,0,0>`) CoW delta map keyed on an immutable `LedgerAnchor`.
  - `fmine-verify` / `fmine-report` (T/U layers): implements `verify_prose_markdown(doc, anchor)` (`fmine verify-prose`
    CLI verb and `verify_narrative_claims` MCP tool).
- **Users & Workflows:**
  - **AI Agents (Claude, GPT, Gemini, Avathon) & Human Planners:** Call `sandbox_fork(anchor_id, deltas[])` via MCP or
    CLI to run paired Common-Random-Number (`GB-12`) multi-subsystem sensitivity studies safely in memory, returning a
    `PairedCounterfactualDiff` against the base anchor.
  - **Board / IR / QP / Compliance Reviewers & Agents:** Every agent-drafted memo or report must pass
    `fmine verify-prose <memo.md> --anchor <anc_...>` (exit code `0`) before a human reviews it.

### 5.3 Technical & Mathematical Design (`DESIGN`)

#### (A) Zero-Copy CoW `SandboxedOverlay` & Paired CRN Differencing
Given a sealed `LedgerAnchor` `A_0` with seed root `S_0`:
1. `SandboxedOverlay::new(A_0, deltas, mem_quota)` constructs an immutable view over `A_0` plus an in-memory
   `BTreeMap<ObjectId, Claim<T>>` holding the overridden assumptions (`PriceDeck`, `CorrelationCard`, `ScheduleCap`,
   `SlopeAngle`).
2. Using `ClaimDag` dependency tracking (`crates/fmine-claim/src/lib.rs:429–507`), `SandboxedOverlay` identifies the
   exact minimal dirty subgraph `D(deltas) ⊆ ClaimDag` that depends on the overridden roots and recomputes **only**
   `D(deltas)` using the **identical Philox stream keys** `(S_0, partition, domain, var_group, r, brick)` (`§29.5`).
3. Every derived claim `c' ∈ D(deltas)` is tagged with `HypotheticalWatermark { base_anchor: A_0, delta_hash }` and
   paired with its baseline counterpart `c ∈ A_0` on realization index `r = 1..R`, emitting:
   ```text
   Δ_r = c'(r) − c(r),    E[Δ],    P10(Δ), P50(Δ), P90(Δ),    SE_CRN(Δ) = sd(Δ_r) / √R
   ```
   By `GB-12`, because `ρ(c', c) ≈ 0.95`, the sandbox resolves counterfactual differences with **20× fewer
   realizations** than independent runs, while `CapSet<0,0,0,0,0>` guarantees zero disk/network/authority side effects!

#### (B) Five Mechanical Prose-Verification Laws (`fmine verify-prose`, `G-PROSE-1..5`)
`fmine verify-prose` scans a Markdown document for numeric tokens with units/currencies and inline anchor citations
`[clm_<id>]` or `[src_<id>]`:
1. **`G-PROSE-1` (`UncitedNumericQuantity`, `FMINE-REF-601`):** Any sentence or table cell containing a physical or
   monetary quantity (`koz`, `Moz`, `Mt`, `g/t`, `$/oz`, `$M`, `%`) without an adjacent `[clm_...]` or `[src_...]`
   citation is refused.
2. **`G-PROSE-2` (`ValueOrRoundingMismatch`, `FMINE-REF-602`):** The number written in prose must equal the cited
   `Claim`'s value under its declared unit and rounding precision (e.g., writing `1,450 $/oz` when `clm_x` is
   `1,412 $/oz` refuses with the exact replacement token).
3. **`G-PROSE-3` (`BasisOrStageConflation`, `FMINE-REF-603`):** Checks local modifier nouns in the sentence against
   the cited `Claim`'s `(OwnershipBasis, MassBasis, MetalStage, ChemBasis)`:
   - Citing a `100%` project-basis claim in a sentence containing `"attributable"` or `"Barrick's share"` (or vice
     versa) refuses;
   - Citing `MetalStage::Contained` or `InSitu` as `"recovered"` or `"payable"` refuses;
   - Citing `ChemBasis::AsN` against an `AsNO3` limit (`E322–E324`) refuses.
4. **`G-PROSE-4` (`StrippedUncertaintyOrCalibrationBanner`, `FMINE-REF-604`):** If the cited `Claim` has
   `CalibrationFlag ∈ {Uncalibrated, Miscalibrated}` or `ClaimColor ∈ {Estimated, Speculative}`, the prose span must
   include its `[P10, P90]` (or `[lo, hi]`) interval and calibration tag within the same sentence/row.
5. **`G-PROSE-5` (`ForbiddenAuthorityAssertion`, `FMINE-REF-605`):** Enforces `INV-002` and `G-FREE-1` on free text:
   any unquoted assertion of `"safe"`, `"stable"`, `"GISTM conformant"`, `"compliant"`, or `"proven/probable reserve"`
   (unless quoting a signed `Assertion` locator in quotation marks per `§18.7 G-FREE-1`) is refused.

### 5.4 Worked Galaxy-Brain Card (`GB-48`, Illustrative Values)
- **Scenario:** An AI agent summarizes a Pueblo Viejo query into a draft board note:
  > *"Pueblo Viejo will produce 650 koz for Barrick in 2026 [clm_pv26_100pct] at an AISC of $1,380/oz, and the El
  > Llagal TSF is safe and fully conformant."*
- **`fmine verify-prose` output (exit code `3`, 3 structured refusals in `< 2 ms`):**
  1. `FMINE-REF-603 (BasisOrStageConflation)` at `line 1, col 27`: `[clm_pv26_100pct]` has `OwnershipBasis =
     Project100Pct` (`650 koz`), but sentence attributes it to `"for Barrick"` (`60%` share = `390 koz [clm_pv26_attr]`).
  2. `FMINE-REF-604 (StrippedUncertaintyOrCalibrationBanner)` at `line 1, col 48`: `[clm_pv26_100pct]` is
     `CalibrationFlag::Uncalibrated` with 80% interval `[590, 710] koz`; point value cannot appear without its
     interval and `[uncalibrated]` tag.
  3. `FMINE-REF-605 (ForbiddenAuthorityAssertion)` at `line 2, col 18`: Unquoted phrase `"is safe and fully
     conformant"` violates `INV-002` and `G-FREE-1`. Suggested replacement: `"has 74/77 GISTM requirements in state
     current and 3 in state stale as of anchor anc_9a1f"`.

### 5.5 First Shippable Slice, Testing Strategy & Kill Criterion
- **First shippable slice (M0–M1 / WP-02 & WP-06):**
  - Ship `fmine verify-prose` in `fmine-cli` / `fmine-report` (~450 LOC) during M0 so the M0 assurance report
    (`BD-072`) and `GATE-ROBOT-01` (`BD-125`, where a fresh agent completes 20 tasks) are mechanically verified
    against `G-PROSE-1..5`. Ship `SandboxedOverlay` in `fmine-ledger` (~400 LOC) alongside `BD-080`/`BD-084` in M1.
- **Testing strategy (§33.1 Four Claims):**
  - **Parity:** `fmine verify-prose` passes 100% of golden `fmine-report` outputs and rejects 100% of a 50-document
    adversarial corpus containing seeded hallucinations, `100%` vs `61.5%` basis swaps, dropped intervals, and
    `G-FREE-1` violations.
  - **Known answers:** `SandboxedOverlay` run with `deltas = []` produces bit-identical `FMT-CERT-1` digests to the
    base anchor, and a `LabRuntime` capability-mask test proves `SandboxedOverlay` cannot mutate the base ledger or
    perform I/O (`PO-BED-8`).
- **Kill criterion (`KILL-ADIT-PROSE-1`):**
  - If natural-language basis detection (`G-PROSE-3`) produces `> 5%` false positives on human-edited prose, restrict
    `G-PROSE-3` to structured citation macros `@claim[clm_id]{basis=attr, stage=recovered}` that render cleanly to
    text while carrying explicit machine-checkable attributes.

---

## Appendix: All 30 Candidate Ideas Considered (One-Line Titles)

1. **`GuidanceCredibilityBridge` (Bullion/Helm/Pushback/Adit — Top 5 #1):** Pre-release multi-site guidance envelope coherence certifier (solving the Sidak multi-site miss trap) and post-print 6-factor guidance beat/miss attribution separating gold-price royalty drift from operational execution.
2. **`WorkbookForensicAutopsy` (Portal/Bullion/Adit — Top 5 #2):** Pure-Rust `.xlsx` `<f>` formula-AST decompiler, dimensional type-checker, and automated root-cause explainer for omitted `SUM` rows, hidden literal plugs, dragged-cell typos, and unweighted grade averages.
3. **`MineToMillShadowPriceLoop` (Pushback/Crucible/Headframe — Top 5 #3):** Shift-scale receding-horizon bottleneck-regime classifier, linear chance-constrained refractory feed-stability envelope, and live mine-to-mill `$/t` dual-price broadcast (`FMT-DUAL-1`).
4. **`RpeeeAndBlockModelForensics` (Hallmark/Strata/Pushback/Portal — Top 5 #4):** Exact 4-factor 3D spatial Shapley block-model diff (`Depletion + ΔWireframe + ΔDensity + ΔGrade`) on `VdbGrid` plus parametric pseudoflow break-even field `λ*(b)` and surface-facility sterilization certifier.
5. **`AgentSandboxAndProseVerifier` (Adit/Bedrock/Plumb/Testpit — Top 5 #5):** Zero-copy CoW ephemeral counterfactual ledger overlay (`ClaimColor::Hypothetical` with paired CRN diffs) and `fmine verify-prose` mechanical citation/basis/interval verifier for agent-authored memos.
6. **`SmelterOfftakeOptimizer` (Crucible/Bullion/Pushback):** Multi-smelter concentrate blending & contract allocation MILP (stepping around As/Hg/Bi/F/moisture penalty cliffs) coupled with an IFRS 9 / ASC 815 Quotational-Period (QP) mark-to-market subledger.
7. **`StreamingRoyaltyTheoremProver` (Helm/Bullion/Hallmark):** 3D cadastral royalty and metal-stream compiler that projects polygon-specific, tier-stepping, and fixed-recovery stream wedges (e.g. Pueblo Viejo Ag stream) into block-level `pitflow` and schedule optimization.
8. **`CryptographicDataRoom` (Bedrock/Helm/Hallmark):** Zero-leakage verifiable M&A, JV vend-in (Fourmile → NGM), and project-finance data-room bundle (`FMT-VDR-1`) using MMR redaction proofs and staged CAS chunk unsealing.
9. **`RefineryOutturnArbitrator` (Crucible/Bullion/Corebox):** Repeated-game e-process detector for systematic counterparty assay shading inside commercial splitting limits, paired with an optimal umpire-call EV decision rule.
10. **`DrillholeReferee` (Corebox/Lodestar/Strata):** In-progress drillhole ISCWSA trajectory-covariance projector that triggers real-time gyro checks, directional wedge kick-offs, or core-loss redrills *before* the drill rig demobilizes from the pad.
11. **`StructuralAdversary` (Strata/Corebox/Highwall):** Popperian structural challenger over L0 contacts and L1 wireframes that detects kinematic fault/stratigraphic violations, proposes minimal counter-hypotheses (en-echelon lenses vs steep vein), and prices 1-hole falsification VOI.
12. **`CompositionalGeometBridge` (Strata/Crucible/Corebox):** Multivariate simplicial-depth Area of Applicability (AOA) scanner over scheduled quarterly mill feed that selects D-optimal core intervals from non-oxidized `Corebox` inventory to eliminate joint `(TOC, As, S, clay, hardness)` blind spots.
13. **`LegacyDataExcavator` (Corebox/Portal/Lodestar):** Cross-campaign spatial near-neighbour forensic reconciler that jointly estimates historical campaign 3D collar shifts (`Δx, Δy, Δz`), assay bias, and censoring quantization for NI 43-101 Item 12 qualification.
14. **`StockpileSegregationAndReclaimPhysics` (Touchstone/Crucible/Pushback):** Gravity rill-slope size/grade segregation and kinematic funnel-flow draw-cone operator on `StockpileVoxelTwin` (`VdbGrid`) that predicts grade/hardness swings during toe vs feeder reclaim.
15. **`HaulRoadSurfaceAndWetWeatherTwin` (Headframe/Pushback):** Physics-informed inversion of truck rimpull/fuel telemetry into a live 50m-segment rolling-resistance posterior `RR_e(t)`, pricing motor-grader dispatch against AHS geo-fence interruption delays (`GB-40`).
16. **`UndergroundStopeSequenceStressAndFillCoupler` (Highwall/Pushback/Headframe):** Boundary-influence stress-shadow matrix and diminishing-pillar guard injected as linear cuts into the underground stope MILP alongside sequence-dependent `TCM` wall dilution and paste-fill cure curves.
17. **`PermitAndCommitmentCriticalPathCompiler` (Freeboard/Helm/Pushback):** Coupled permit-dependency DAG that links statutory approval clocks, TSF volumetric runways (`E190`), and environmental trigger curtailments (`E278`) directly to `Pushback` capacity constraints.
18. **`TailingsBeachAndWaterReclaimOptimizer` (Freeboard/Highwall/Crucible):** 2.5D spigot-deposition cycle and decant-pond position optimizer that maintains beach setback, bounds foundation `B̄(t)` rate-of-rise (`GB-42`), and maximizes clarify-zone water reclaim.
19. **`ClosureCashflowAndProgressiveRehabScheduler` (Freeboard/Pushback/Bullion):** LOM waste-dump scheduling extension that co-optimizes active haulage cost, progressive NAF encapsulation cover, ARD oxidation kinetics, IAS 37 ARO accretion, and financial-assurance bond release (`E188`).
20. **`BlastholeToSlopeDamageCoupler` (Headframe/Highwall/Pushback):** Empirical blast-damage skin field `D_actual(x)` fitted from near-wall charge weights and post-blast LiDAR half-barrel preservation, feeding live Hoek–Brown slope FoS updates and trim-blast ROI cards.
21. **`StatutoryAndCriticalControlInterlockMatrix` (Muster/Highwall/Freeboard/Headframe):** Shift-start operational readiness engine that cross-joins active headings against TARP freshness, seismic/fume re-entry, adjacent paste cure (`G-HEAD-7`), CCV quality (`G-MUST-6`), and unexpired statutory appointments.
22. **`CounterfactualHindsightGuard` (Plumb/Bedrock/Testpit):** Transitive temporal-cone prover (`verify_decision_cone`) and future-zeroed ledger replay oracle (`PO-TIME-CONE`) that catches hyperparameter, wireframe, and price-deck lookahead across multi-stage backtests.
23. **`SensorClockDriftAndPacketLineageAttestor` (Headframe/Bedrock):** Cross-stream physical impulse correlator (truck dump → crusher power spike → belt weightometer surge) that estimates and corrects historian/FMS clock skews (`ClockSkewCorrectionClaim`) on `FMT-TELSEG-1` ingest.
24. **`WaterQualityStoichiometricAndIonBalanceGuard` (Freeboard/Corebox/Bedrock):** Hydrochemical electroneutrality (cation–anion charge-balance error `≤ 5%`), TDS-conductivity, and `dissolved ≤ total` gate on environmental certificate ingest, catching `µg/L` vs `mg/L` 1000× errors automatically.
25. **`SparePartsAndCriticalComponentBufferOptimizer` (Headframe/Helm/Pushback):** Multi-site long-lead capital spares pooling optimizer (SAG gearless drives, 330 kV transformers `E668`, hoist motors `E231`, POX agitators) priced via NHPP survival hazard and Pushback bottleneck duals under CVaR.
26. **`ExplorationTenementDropOrHoldOptionEngine` (Lodestar/Helm):** Cadastral partial-relinquishment and minimum-commitment real-option optimizer over `Lodestar`'s thinned Cox posterior `Λ_undisc(s)` with `AF447` deep-cover protection (`GB-02`).
27. **`MultiSiteCarbonAndGridDecarbonizationScheduler` (Headframe/Pushback/Helm):** Hourly hydro/solar/BESS/diesel dispatch and trolley-assist/BEV haulage co-optimizer over `EnergyEnvelope` (`E668`, `E669`) pricing Scope 1/2 abatement `CapitalCandidate`s in `$/t CO₂e` vs `ΔNAV/share`.
28. **`CyanideAndReagentStoichiometricTwin` (Crucible/Freeboard/Bullion):** End-to-end ICMC cyanide & reagent mass-balance tracker linking ore `CuCN`/`S`/`TOC` speciation to leach consumption, detoxification reagent demand, and tailings discharge `WAD-CN` limits.
29. **`ContractMiningAndEquipmentLeaseArbitrageCard` (Helm/Headframe/Muster/Bullion):** Owner-mining vs contract-mining and OEM availability-guarantee contract compiler with IFRS 16 lease capitalization and counterfactual underperformance clawback scoring.
30. **`GeoPoliticalAndSovereignFiscalShockSimulator` (Helm/Bullion/Bedrock):** Coupled sovereign-regime Markov chain (`BD-296`) and trapped-cash/VAT-receivable/royalty-ratchet stress tester over `CapitalEntityGraph` (`BD-314`) for multi-jurisdiction capital allocation.
