# FrankenMining — Dueling Idea Wizards (Next Best 10: Ideas #6–#15, AGY / Gemini)

> **Scope & Compliance Note.** All ten ideas below (`#6–#15`, ranked in order from best `#6` to `#15`) follow the exact
> ground rules of Round 1: zero write paths to ISA-95 Levels 0–2 (`INV-011`, `NONGOAL-002`), the four human-authority
> envelope flags (`reserve_disclosure_claim`, `execution_authorized`, `geotechnical_signoff`,
> `regulatory_compliance_claim`) hard-wired `false` (`INV-002`), zero new crates outside the 59-crate topology
> (`§28.3`, `registries/loc_budget.toml`), downward-only layer edges (`L-1`: `T → U → V → E → K4..K0 → S1 → S0`),
> `franken_accounting` (`FA`) reached strictly via subprocess (`L-4`), public-lane-only fixtures in this repository,
> and zero personal data (`§31.2`). All mathematics uses Unicode notation (`§0.7`), and every claim cites its
> epistemic label and `E###` evidence register entry.

---

## Summary Table of Ideas #6–#15

| Rank | Idea Name | Subsystems & Crates (§0.5, §28.3) | Primary Users & Workflows | First Shippable Slice |
|---:|---|---|---|---|
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

## Idea 6: `SmelterOfftakeOptimizer` — Multi-Smelter Concentrate Lot Blending, Penalty-Cliff Avoidance & IFRS 9 / ASC 815 Quotational-Period (QP) Subledger

### 6.1 What Is Already in the Plan vs. What Is Materially New
- **Already in the plan:**
  - `§15.6` writes a single block/parcel NSR expression `NSR_sp = Σ g · R · payable · (price − TC/RC) − penalties_p(s) − royalty`, and `§17.6` / `§21.5` note that copper/tin/float concentrates have moisture limits (`FMINE-REF-403`), assay exchange, and provisional pricing (`IFRS 9` whole-receivable FVTPL vs `ASC 815` bifurcated derivative). Appendix C `I-25` places a single contract TOML in `fmine-acctpack`.
- **What is missing (and materially new here):**
  1. **Multi-Smelter Port-Lot Blending & Contract Allocation Optimization:** A copper-gold or tin producer (Lumwana,
     Jabal Sayid, Reko Diq, or refractory float concentrates) ships into **multiple simultaneous smelter and trader
     contracts** with sharply different deleterious penalty tiers and hard rejection cliffs (e.g., Custom Smelter A
     rejects above `0.20% As` or `50 ppm Hg`, while Custom Smelter B accepts up to `0.50% As` at `$3.00/dmt` per
     `0.10% As` above `0.20%`, and Trader C has higher TC/RC but a `0.80% As` blending tolerance). Treating `NSR_sp`
     as a static per-block property ignores **post-mill concentrate bin blending and contract routing**, which
     regularly captures `$15M–$50M/yr` by blending high-`As`/high-`Au` concentrate lots with clean lots to sit *just*
     under a contract's step-cliff or minimum payable threshold (e.g. `1.0 g/t Au` payable floor)!
  2. **Coupled Quotational-Period (QP) Exposure & Provisional-Pricing Subledger:** When a shipment sails in October
     with `QP = M+3` (January average LME price) or buyer-declared `M+1 / M+4` option, the open receivable fluctuates
     across quarter-ends. The plan names the accounting rule (`§17.6`) but has no state machine linking physical
     lot assays, moisture splits, forward curve marks, and assay-exchange finalization into `FMT-POST-1` slots!

### 6.2 Subsystems, Crates, Users & Workflows
- **Subsystems:** `Crucible`, `Bullion`, `Pushback` (`§0.5`).
- **Crates (`§28.3`, zero new crates):**
  - `fmine-acctpack` (E layer, budget 22k LOC): implements `SmelterContractCard`, piecewise-stepped penalty/payable
    evaluator, and `ProvisionalShipmentLedger` (`IFRS 9` vs `ASC 815` mark-to-market generator).
  - `fmine-plan` (K3 layer, budget 30k LOC): adds `fmine-plan::offtake` (a small MILP over `fmine-lp::milp`, `≤ 1,500`
    variables, allocating filter-cake batches `b` to port storage bins `k` and shipping parcels `v` bound to smelter
    contract `c`, and passing exact shadow prices `λ_As, λ_Hg, λ_Bi, λ_Sb, λ_AuFloor` back to the mill-feed LP!).
  - `fmine-plant` (K2 layer): enforces `FMINE-REF-403` (moisture vs TML) and `FMINE-REF-405` (`OffSpecSmelterReject`)
    before any shipment release effect (`EFFECT-SHIPMENT-RELEASE`) is prepared.
- **Users & Workflows:**
  - **Concentrate Marketing & Commercial Team:** Runs `fmine plan offtake` weekly to blend port filter-cake lots into
    upcoming vessel parcels and assign each parcel to the highest-netback contract.
  - **CFO & Mine Controller:** Runs `fmine bullion qp-revalue --as-of 2026-12-31` at period close to generate exact
    provisional-pricing revaluation adjustments, separating price-settlement variance from assay/moisture outturn
    variance for the quarterly MD&A bridge and `franken_accounting` posting slots.

### 6.3 Technical & Mathematical Design (`DESIGN`)
For shipping parcel `v` formed by blending filter-cake lots `b ∈ B` in dry tonnes `y_{b,v} ≥ 0` (with `W_v = Σ_b y_{b,v}`
and blended assay `ā_{v,e} = (Σ_b y_{b,v} a_{b,e}) / W_v`) assigned to contract `c` (`z_{v,c} ∈ {0, 1}`):
1. **Payable Metal with Minimum Deduction & Floor Cliff:**
   ```text
   Payable_{v,c,m} = W_v · max(0,  1[ā_{v,m} ≥ g^min_{c,m}] · min(p_{c,m} · ā_{v,m},  ā_{v,m} − d^min_{c,m}))
   ```
   Because `1[ā_{v,m} ≥ g^min_{c,m}]` is a step at the payable floor (e.g. `Au ≥ 1.0 g/t` in Cu concentrate), blending
   a `1.3 g/t Au` lot with a `0.8 g/t Au` lot at a `60:40` ratio brings the entire parcel to `1.10 g/t ≥ 1.0 g/t`,
   making the `0.8 g/t` lot's gold payable instead of zeroed!
2. **Tiered Penalty Schedule & Hard Rejection Guard (`FMINE-REF-405`):**
   ```text
   Penalty_{v,c,e} = W_v · Σ_{tier j} r_{c,e,j} · max(0,  min(ā_{v,e}, u_{c,e,j}) − l_{c,e,j})
   Constraint:       ā_{v,e} ≤ RejectMax_{c,e}  whenever z_{v,c} = 1   (else FMINE-REF-405)
   ```
   Since `W_v · ā_{v,e} = Σ_b y_{b,v} a_{b,e}`, **every mass-times-concentration term `W_v · max(0, ā_{v,e} − l)`
   rewrites as `max(0, Σ_b y_{b,v} (a_{b,e} − l))`, which is 100% linear in `y_{b,v}`**! Thus the concentrate
   blending and contract-assignment problem has **zero bilinear non-convexities** when parcel size `W_v` is bounded
   by vessel stem `[W_min, W_max]` (`PO-CRUC-OFFTAKE-1`)!

### 6.4 Worked Galaxy-Brain Card (`GB-49`, Illustrative Values)
- **Scenario:** Two `5,000 dmt` Cu-Au concentrate lots sit at port:
  - Lot 1: `26% Cu`, `1.4 g/t Au`, `0.34% As`.
  - Lot 2: `28% Cu`, `0.7 g/t Au`, `0.06% As`.
  - Contract rules: Au is payable (`90%` above `1.0 g/t`, `0%` below `1.0 g/t`); As is free up to `0.20%`, penalized
    at `$30/dmt` per `0.10%` in `[0.20%, 0.30%]`, and `$80/dmt` per `0.10%` above `0.30%`. Gold is `$3,000/oz`.
- **Unblended shipping (2 separate `5,000 dmt` parcels):**
  - Lot 1 pays `$30 + 0.4×$80 = $62/dmt` As penalty (`−$310k`), and gets paid on `1.4 × 0.90 = 1.26 g/t Au` (`+$608k`).
  - Lot 2 pays `$0` As penalty, but its `0.7 g/t Au` falls below the `1.0 g/t` floor → **`$0` Au revenue**!
  - Net Au minus As penalty across both lots = `+$608k − $310k = +$298k`.
- **50/50 Blended Parcel (`10,000 dmt` at `1.05 g/t Au` and `0.20% As`):**
  - Blended `As = 0.20%` → **`$0` As penalty (saves `$310k`)**!
  - Blended `Au = 1.05 g/t ≥ 1.0 g/t` → `10,000 dmt × 1.05 × 0.90 = 303.8 oz` payable Au (`+$911k`, **unlocking
    `+$303k` of previously lost gold**)!
  - **Net gain from blending = `+$613k` on a single `10,000 dmt` vessel (`+$61.30/dmt`)!**

### 6.5 First Shippable Slice, Testing Strategy & Kill Criterion
- **First shippable slice (M2–M3 / WP-10 & WP-13):**
  - Implement `SmelterContractCard`, the exact linear reformulation `fmine-plan::offtake`, and the `IFRS 9`/`ASC 815`
    QP revaluation engine in `fmine-acctpack` (~700 LOC total).
- **Testing strategy (§33.1 Four Claims):**
  - **Parity:** Reproduces standard trader/smelter settlement sheets and IFRS 9 vs ASC 815 QP revaluation examples to
    `$0.01` (`ExactQty`).
  - **Known answers:** On synthetic 20-lot / 4-contract instances, the linear reformulation matches brute-force
    enumeration and HiGHS MILP oracle (`GATE-PUSH-BLEND`).
- **Kill criterion (`KILL-BULL-OFFTAKE-1`):**
  - If a site produces only doré (no concentrate shipments) or sells 100% into a single tolling agreement without
    blending bins, run the `SmelterContractCard` evaluator in pass-through mode without the MILP.

---

## Idea 7: `StreamingRoyaltyTheoremProver` — 3D Cadastral Royalty, Sliding-Scale & Fixed-Recovery Metal-Stream Compiler for Block-Level Optimization

### 7.1 What Is Already in the Plan vs. What Is Materially New
- **Already in the plan:**
  - `§15.6` includes `- royalty_m(price)` in the block NSR formula and reproduces seven published cut-offs in M0
    (`BD-274`, noting Cortez cut-offs reproduce only with the Nevada `AB495` tax omitted, `E183`).
  - `§21.5` mentions IFRS 15 / ASC 606 deferred revenue for metal streams, and `§23.2` (`BD-296`) models country
    fiscal regimes (sliding-scale royalties in Mali and DRC).
- **What is missing (and materially new here):**
  1. **3D Spatially Partitioned Cadastral Encumbrances:** At complexes like NGM (Cortez Crossroads vs Pipeline vs
     Goldstrike claim blocks), a single open pit or underground mine straddles **dozens of distinct private royalty
     polygons (`NVR1–NVR5`, Franco-Nevada, Royal Gold)** where adjacent 3D blocks have Net Smelter Return (NSR) or
     Gross Smelter Return (GSR) encumbrances differing by `1%` to `9%`! Applying a deposit-average royalty % in
     pseudoflow (`pitflow`) over-mines high-royalty claims and under-mines low-royalty claims.
  2. **Fixed-Recovery & Tier-Stepping Metal Streams (The By-Product Stream Trap):** Some streams (e.g., a
     by-product stream where silver delivery is calculated on a **fixed 70% contractual silver
     recovery** until `X Moz` are delivered, then steps down by half!) decouple stream liability from actual plant
     by-product recovery. When actual Ag recovery on a refractory domain is `42%` while contractual Ag delivery is
     `70% × 75% = 52.5%` of contained Ag, **mining that block creates a net silver deficit that must be subsidized out
     of gold revenue**!
  3. **Automated Cut-Off Table vs. Royalty Footnote Consistency Prover (`G-ECON-ROY-1`):** Mechanically proves whether
     a Technical Report's cut-off grade table (`§15` / `§19` of NI 43-101 / S-K 1300) includes the royalties
     and streams disclosed in `§4` (Property Description & Royalties), flagging basis questions like `E183`.

### 7.2 Subsystems, Crates, Users & Workflows
- **Subsystems:** `Helm`, `Bullion`, `Hallmark`, `Pushback` (`§0.5`).
- **Crates (`§28.3`, zero new crates):**
  - `fmine-econ` (K1 layer, budget 20k LOC): implements `EncumbranceGraph`, `RoyaltyPolygonRule`, `StreamStepRule`,
    and `block_effective_netback(block_id, cum_delivered, price_draw)`.
  - `fmine-plan` (K3 layer): injects exact spatial/cumulative encumbrance wedges into `pitflow` block values and
    multi-period schedule LP coefficients (`fmine-plan::sched`).
  - `fmine-reserves` (K4 layer): runs `G-ECON-ROY-1` comparing disclosed cut-off parameters against `EncumbranceGraph`.
- **Users & Workflows:**
  - **Mine Planners & QPs:** Ensures `pitflow` shells and stope boundaries respect 3D claim-polygon royalty boundaries
    and contractual stream recovery wedges.
  - **Corporate Development, M&A & CFO:** Evaluates the exact distortion cost (lost reserve ounces from royalty/stream
    overhang) and prices royalty buy-downs or stream restructurings as `CapitalCandidate`s (`§23`).

### 7.3 Technical & Mathematical Design (`DESIGN`)
For block `b` located in 3D claim prism `Π(b)` with contained primary metal `M_{b,1}` (Au) and by-product `M_{b,2}`
(Ag), physical recoveries `R_{1}(b), R_{2}(b)`, and a metal stream on metal `2` with contractual recovery `R^contract_2`,
stream share `s_stream(Q_cum) ∈ {s_tier1, s_tier2}`, and cash delivery price `p_cash = min(P_cap, α P_2)`:
```text
StreamObligation_2(b) = M_{b,2} · R^contract_2 · s_stream(Q_cum)
OperatorNetByproductRev(b) = M_{b,2} · [ R_2(b) · P_2 − R^contract_2 · s_stream(Q_cum) · (P_2 − p_cash) ]
```
Notice that when `R_2(b) < R^contract_2 · s_stream(Q_cum) · (1 − p_cash / P_2)`, **`OperatorNetByproductRev(b) < 0`
—higher silver grade `g_{b,2}` *reduces* the block's value to the operator!**
Furthermore, across cumulative tier threshold `Q_tier1` (where `s_stream` drops from `0.75` to `0.375`), `fmine-plan`
introduces a single auxiliary cumulative-balance variable per period `t`, pricing the **Shadow Value of Crossing the
Stream Step-Down Tier Earlier (`λ_tier`)**!

### 7.4 Worked Galaxy-Brain Card (`GB-50`, Illustrative Values Anchored to Fixed-Recovery Stream Mechanics)
- **Scenario:** A refractory gold-silver block has `2.0 g/t Au` (`85%` recovery) and `40.0 g/t Ag`, but high sulphide/
  preg-robbing encapsulates the silver so physical plant Ag recovery is `R_Ag = 38%`.
  - Stream terms: Deliver `75%` of payable Ag calculated at a **fixed `70%` contractual Ag recovery** for `p_cash =
    $4.00/oz` when spot silver is `P_Ag = $32.00/oz` and gold is `$3,000/oz`.
- **Naive planner (ignores fixed-recovery stream wedge or treats Ag as `+38% × $32/oz` credit):**
  - Assumes Ag credit = `40 g/t ÷ 31.1035 × 0.38 × $32 = +$15.64/t`.
- **True operator economics (`StreamingRoyaltyTheoremProver`):**
  - Physical Ag recovered = `1.286 oz/t × 0.38 = 0.489 oz/t` (`+$15.64/t`).
  - Contractual stream delivery = `1.286 oz/t × 0.70 × 0.75 = 0.675 oz/t`!
  - Stream cost = `0.675 oz/t × ($32 − $4) = −$18.90/t`!
  - **Net Ag contribution to operator = `+$15.64 − $18.90 = −$3.26/t` (silver is a penalty element on this ore type,
    and the naive model overstated block value by `$18.90/t`, equal to `0.22 g/t Au` in cut-off grade!)**

### 7.5 First Shippable Slice, Testing Strategy & Kill Criterion
- **First shippable slice (M1–M3 / WP-03 & WP-14):**
  - Implement `EncumbranceGraph`, 2.5D/3D claim-polygon point-in-prism lookup (`fmine-geom::winding`), fixed-recovery
    stream wedge math, and `G-ECON-ROY-1` cut-off table auditor in `fmine-econ` (~550 LOC).
- **Testing strategy (§33.1 Four Claims):**
  - **Parity:** Reproduces Cortez (`E183`), Carlin, Turquoise Ridge (`E182`), Kibali, and Loulo (`BD-296`) cut-off
    and royalty calculations to `1e-6`.
  - **Known answers:** Synthetic pit straddling a `2%` vs `8%` NSR boundary shifts ultimate pit closure(`pitflow`)
    inward on the `8%` side and increases operator post-royalty NPV by `+4.8%` vs a uniform `5%` average assumption.
- **Kill criterion (`KILL-ECON-ROY-1`):**
  - If digitized 3D claim boundary polygons are unavailable for a site, fall back to domain-level `EncumbranceGraph`
    assignments with an `Assumed(UniformDomainRoyalty)` banner and flip margin.

---

## Idea 8: `CryptographicDataRoom` — Zero-Leakage Verifiable M&A, JV Vend-In (Fourmile → NGM) & Project-Finance Data-Room Compiler (`FMT-VDR-1`)

### 8.1 What Is Already in the Plan vs. What Is Materially New
- **Already in the plan:**
  - `§11.7` notes that Barrick anticipates vending Fourmile into the 61.5/38.5 NGM joint venture at fair market value
    (`E139`, `E658`) and that Fourmile's valuation is walled from NGM partner outputs until vend-in (`§31.2`).
  - `§31.2` defines per-venture physical projection separation and `.fab` / `FMT-CERT-1` partner bundles for *existing*
    joint ventures, plus sovereign partner principals for Reko Diq (`E302`).
- **What is missing (and materially new here):**
  - During a **JV vend-in negotiation (Fourmile → NGM), a project-finance technical due diligence (Reko Diq lender
    syndicate `E302`), an earn-in milestone audit, or an asset divestment (Tongon / Hemlo)**, the seller and
    buyer sit across a confidential information boundary:
    - The buyer/partner's QP and Independent Technical Consultant (ITC) must be able to verify that the disclosed
      resource block summaries, QA/QC pass rates, and LOM schedule were computed from complete raw assays (with
      every failed CRM batch and every drilled hole accounted for);
    - Yet the seller **cannot expose** drillholes outside the vend-in boundary, proprietary regional `Lodestar`
      posteriors (`basis = Full`), internal corporate cost cards, or Stage-2 raw block realizations before Stage-1
      price/term milestones are signed!
  - `CryptographicDataRoom` compiles a self-contained, cryptographically redacted bundle (`FMT-VDR-1`) verified
    offline by `fmine-verify` (`std`-only) using **Merkle Mountain Range (MMR) prefix/inclusion proofs, Homomorphic /
    Interval Conservation Witnesses, and Staged CAS Chunk Unsealing**!

### 8.2 Subsystems, Crates, Users & Workflows
- **Subsystems:** `Bedrock`, `Helm`, `Hallmark`, `Testpit` (`§0.5`).
- **Crates (`§28.3`, zero new crates):**
  - `fmine-authz` (S1 layer, budget 14k LOC): defines `DataRoomPolicy { spatial_clip: Polygon3D, min_support: Support,
    redacted_walls: Vec<WallId>, stage: u8 }`.
  - `fmine-ledger` & `fmine-object` (S1 layer): emits `FMT-VDR-1` bundles with MMR inclusion proofs, blinded-leaf
    subtree digests, and stage-keyed salted chunk manifests.
  - `fmine-verify` (T layer, `std`-only, budget 12k LOC): verifies `FMT-VDR-1` offline on the counterparty's laptop
    (`fmine verify data-room <bundle.vdr>`).
- **Users & Workflows:**
  - **Corporate Development, General Counsel & JV Negotiation Team:** Exports Stage-1 and Stage-2 verifiable data-room
    bundles for Fourmile vend-in or project-finance lenders with mathematical proof of zero MNPI/outside-tenement
    leakage (`PO-SEC-1`).
  - **Counterparty QP / Lender's Independent Engineer:** Runs `fmine-verify` offline to verify conservation, QA/QC
    completeness, and Stage-1 → Stage-2 commitment binding independently of the seller's own systems.

### 8.3 Technical & Mathematical Design (`DESIGN`)
1. **Spatial & Support Redaction with Conservation Proofs:**
   Given a full `CELL-DRILL` and `CELL-BELIEF` anchor `A_full`:
   - Every brick `B_i` inside the vend-in polygon `Π_vend` is disclosed at summary support `S_min` (e.g. `20m × 20m ×
     15m` panel summaries in Stage 1; `5m` SMUs and raw intervals in Stage 2).
   - Fine-scale CAS chunks `Chunk_k` for Stage 2 are included in the Stage-1 manifest only by their salted BLAKE3
     commitment `H(Chunk_k ∥ salt_k)`.
   - `FMT-VDR-1` carries a **Merkle Sum-Tree Witness** over the bricks inside `Π_vend`: each internal Merkle node
     commits to `(Hash_left ∥ Hash_right, ExactTonnes_sum, ExactMetal_sum, Count_Conflicted_Assays)`.
   - The counterparty's `fmine-verify` checks in `O(#disclosed panels)` that:
     1. The sum of disclosed panel tonnes and metal equals the root commitment in the signed `TonnageGradeCertificate`
        to `1e-12` (`PO-STRATA-3`);
     2. The count of `Conflicted` / `CommittedFlagged` QA/QC batches (`§12.4`, `E223`) inside `Π_vend` is disclosed
        explicitly in the root header (proving zero silent exclusion of failed batches!);
     3. Every brick outside `Π_vend` is represented solely by an opaque sibling hash `H_ext` (zero information leak
        beyond the root hash).
2. **Commitment-Bound Staged Unsealing:**
   When the deal advances from Stage 1 (panel summaries) to Stage 2 (full drillhole & SMU disclosure), the seller
   transmits only the 32-byte salts `{salt_k}` and the raw chunks `{Chunk_k}`. `fmine-verify` checks that every newly
   revealed hole and SMU hashes to the exact Stage-1 commitment `H(Chunk_k ∥ salt_k)` anchored months earlier!

### 8.4 Worked Galaxy-Brain Card (`GB-51`, Illustrative Values Anchored to Fourmile Vend-In `E139`, `E658`)
- **Scenario:** Seller prepares a Stage-1 JV vend-in package for a `15.6 Moz` deposit (`2.6 Moz` Indicated + `13.0 Moz`
  Inferred, `E140`) while keeping adjacent 100%-owned district exploration holes (`Wall = W-DIST-EXP`) and Stage-2
  fine-scale `VdbGrid` realizations confidential during initial valuation framing.
- **Traditional VDR risk:** Sending an acQuire database dump or Leapfrog project either leaks collar coordinates/assays
  from adjacent tenements or forces the seller to hand-edit CSV files (breaking chain of custody and leaving
  the counterparty unable to verify completeness).
- **`CryptographicDataRoom` (`FMT-VDR-1`) result:**
  - Stage-1 bundle is `18 MB`: proves to `fmine-verify` that `100%` of the `412` holes intersecting `Π_vend` up to
    `record_seq = 88,410` are included, `0` failed CRM batches were dropped, and panel totals match the signed
    certificate to `0.001 oz`, while leaking `0 bits` about the `190` holes outside `Π_vend`.
  - When Stage 2 opens, `fmine-verify` confirms in `1.8 s` that the `2.4 GB` of raw core intervals and SMU bricks match
    the Stage-1 Merkle commitments bit-for-bit.

### 8.5 First Shippable Slice, Testing Strategy & Kill Criterion
- **First shippable slice (M1–M3 / WP-04 & WP-07):**
  - Implement Merkle Sum-Tree commitment nodes in `fmine-object` / `fmine-ledger` and the `FMT-VDR-1` offline checker
    in `fmine-verify` (~600 LOC).
- **Testing strategy (§33.1 Four Claims):**
  - **Parity / Security:** Differential wall test (`PO-SEC-1`, `GATE-WALL-01`): two worlds differing only in drillholes
    outside `Π_vend` (with fresh per-subtree salts) produce computationally indistinguishable Stage-1 sibling hashes.
  - **Known answers:** Dropping a single `CommittedFlagged` assay interval or altering a Stage-2 chunk by 1 bit causes
    `fmine-verify` to exit with code `8` (`TamperOrCommitmentMismatch`).
- **Kill criterion (`KILL-BED-VDR-1`):**
  - If counterparties refuse CLI verification, `fmine-report` emits a human-readable PDF/Markdown audit trail alongside
    the `.vdr` file with printed SHA-256/BLAKE3 roots that can be checked with standard `sha256sum`.

---

## Idea 9: `StockpileSegregationAndReclaimPhysics` — Gravity Rill-Slope Size/Grade Segregation & Kinematic Funnel-Flow Draw-Cone Operator on `StockpileVoxelTwin`

### 9.1 What Is Already in the Plan vs. What Is Materially New
- **Already in the plan:**
  - `§20.8` and `BD-312` introduce `StockpileVoxelTwin` on `VdbGrid`, tracking dumped parcel provenance, sulphide
    oxidation age, and Pueblo Viejo's plant-cone latent inventory (`E666`: a `70 kt` swing between crushers and
    mills that the monthly balance must carry).
- **What is missing (and materially new here):**
  - `BD-312` places dumped truckloads into static 3D voxels and tracks oxidation age, but **how do dumped parcels
    distribute spatially across a 30 m high stockpile face, and which voxels actually enter the loader bucket or apron
    feeder during reclaim?**
  - In real mining stockpiles and surge cones (`FACT`, `R7 §6–§7`), two physical mechanisms dominate short-term grade
    and mill throughput:
    1. **Gravity Rill-Slope Segregation on Dump:** When a truck end-dumps at a stockpile crest (or a belt stacks a
       surge cone), coarse rocks (`+100 mm`) roll down the `37°` angle-of-repose rill slope to the **toe**, while fine
       particles (`−12 mm`, which in gold and copper ores carry **1.3× to 2.5× higher metal grade**, higher moisture,
       and higher clay/sulphide surface area!) arrest in the **upper crest**.
    2. **Funnel-Flow vs. Toe-Loader Reclaim Kinematics:** Reclaiming the toe with a front-end loader pulls the coarse,
       hard, lower-grade rock first (spiking SAG mill `F80` and dropping head grade!), whereas drawing from an
       under-pile apron feeder carves a steep **Beverloo/Nedderman kinematic
       draw ellipsoid** directly above the feeder throat, leaving a stagnant perimeter "dead zone" of fine sulphides
       that oxidize for months until a dozer push dumps a slug of oxidized, acid-consuming, low-recovery ore into the
       mill!

### 9.2 Subsystems, Crates, Users & Workflows
- **Subsystems:** `Touchstone`, `Crucible`, `Pushback`, `Strata` (`§0.5`).
- **Crates (`§28.3`, zero new crates):**
  - `fmine-recon` (K2 layer, budget 18k LOC): extends `StockpileVoxelTwin` (`BD-312`) with `RillSegregationOperator`
    and `KinematicDrawConeOperator`.
  - `fmine-plant` (K2 layer): couples reclaimed voxel `(F80_reclaimed, grade_reclaimed, oxidation_extent)` to the SAG
    specific-energy card (`§17.3`) and flotation/POX/CIL recovery cards (`§18.5` / `§20.8`).
- **Users & Workflows:**
  - **Plant Metallurgist & Short-Term Planner:** Predicts the exact 12-hour grade, `F80` particle size, and oxidation
    spike when switching from direct tipping to apron-feeder surge-cone draw or dozer pushdown (`E666`).
  - **Reconciliation Geologist (`Touchstone`):** Separates physical segregation/funnel-flow timing transients from true
    `F2` bias so a toe-reclaim week is not misdiagnosed as a mine call factor collapse!

### 9.3 Technical & Mathematical Design (`DESIGN`)
1. **Mass- and Metal-Conserving Rill-Slope Segregation (`RillSegregationOperator`):**
   When a parcel `P` of dry mass `M`, element masses `Q_m = M · g_m`, and particle-size class shares `w_c` (`c ∈
   {Fines, Mid, Coarse}`, `Σ_c w_c = 1`) with size-by-grade enrichment factors `η_{c,m}` (`Σ_c w_c η_{c,m} = 1`,
   calibrated from Corebox/Crucible screen-fire or size-by-size assays `§17.3`) is dumped at crest voxel `x_0` above a
   rill trajectory `γ(s)` (`s ∈ [0, L_rill]` from crest `s=0` to toe `s=L_rill`):
   - Each size class `c` deposits along `γ(s)` with normalized Beta density `f_c(s / L_rill; α_c, β_c)` (`Coarse`
     skewed to `s → L_rill`, `Fines` skewed to `s → 0`).
   - Voxel `v_k` at arc position `s_k` receives exact dry mass and metal:
     ```text
     ΔM(v_k)   = M · Σ_c w_c · p_{c,k},        where  Σ_k p_{c,k} = 1  exactly (via ExactQty rounding residual to toe)
     ΔQ_m(v_k) = M · g_m · Σ_c w_c · η_{c,m} · p_{c,k}
     ```
     By construction (`PO-TOUCH-SEG-1`), summing across the rill voxels `k` conserves dry mass `Σ_k ΔM(v_k) = M` and
     every element's metal `Σ_k ΔQ_m(v_k) = Q_m` to **`1e-12` relative (`ExactQty` exact)**!
2. **Kinematic Funnel-Flow Draw Ellipsoid (`KinematicDrawConeOperator`):**
   For an apron feeder at `(x_f, y_f, z_f)` drawing mass `ΔM_draw`:
   - Active draw velocity field follows the kinematic Nedderman–Tüzün parabolic stream function with eccentricity `κ`
     (a material card parameter of moisture and fines content): horizontal variance at height `z` above throat is
     `σ²_r(z) = 4 D_k (z − z_f)`.
   - Voxels are depleted in order of arrival time `τ(x, y, z) = (z − z_f) · exp((r²)/(4 D_k (z − z_f)))`, followed by
     surface avalanching down the induced crater at the dynamic angle of repose `θ_dyn`.
   - Any voxel outside the critical funnel radius `r > r_funnel(z)` stays in the **stagnant annulus**, where its
     sulphide oxidation state `Ox(t) = 1 − exp(−k_ox(Fines, T, O2) · Δt)` ticks upward every day until a recorded
     dozer-push event (`asbuilt` survey or FMS dozer log) shears the annulus into the throat!

### 9.4 Worked Galaxy-Brain Card (`GB-52`, Illustrative Values Anchored to Surge-Cone Mechanics `E666`)
- **Scenario:** A `100,000 dmt` ROM stockpile (`30 m` high) is built from ore averaging `2.00 g/t Au` (`6,430 oz`).
  Screen assays show the `−12 mm` fines (`30%` of mass) carry `3.20 g/t Au` (`η_fines = 1.60`), mid-size (`40%`)
  carries `1.85 g/t`, and `+100 mm` coarse rock (`30%`) carries `1.00 g/t` (`η_coarse = 0.50`).
- **Week 1 (Loader reclaims `25,000 dmt` from the outer toe):**
  - Toe consists of `65% Coarse`, `25% Mid`, `10% Fines` → Reclaimed grade is **`1.43 g/t Au` (`−28.5%` below average
    pile grade!)**, and `F80` is `145 mm` (dropping SAG throughput by `−11%`).
  - A segregating-blind FIFO model predicts `2.00 g/t` and raises a false `F2 = 0.715` alarm!
  - `StockpileSegregationAndReclaimPhysics` predicts `1.43 g/t` `[1.34, 1.52]` and knows that the remaining `75,000 dmt`
    crest core now holds **`2.19 g/t Au`** (`+9.5%` enriched)!

### 9.5 First Shippable Slice, Testing Strategy & Kill Criterion
- **First shippable slice (M1–M2 / WP-08 `BD-312`):**
  - Add `RillSegregationOperator` and `KinematicDrawConeOperator` to `fmine-recon::stockpile` (~450 LOC on `VdbGrid`).
- **Testing strategy (§33.1 Four Claims):**
  - **Parity / Conservation:** `PO-TOUCH-SEG-1` property test over 1,000 random dump/reclaim/dozer sequences: when a
    pile is reclaimed to zero volume, cumulative reclaimed dry tonnes and metal per element equal cumulative dumped
    tonnes and metal to `0` milligram residual (`ExactQty`).
  - **Known answers:** Synthetic segregated pile with `η_fines = 1.6` reproduces analytical toe vs crest grade paths
    and cuts monthly `F2` attribution error by `> 70%` against a homogeneous voxel baseline.
- **Kill criterion (`KILL-TOUCH-SEG-1`):**
  - If a site lacks size-by-grade assay data (`η_{c,m}`) and dump-location GNSS coordinates, default `η_{c,m} = 1.0`
    (size segregation only for SAG `F80`, uniform grade) and widen the reclaim grade interval by the unsegregated
    heterogeneity prior.

---

## Idea 10: `DrillholeReferee` — In-Progress Drillhole ISCWSA Trajectory-Covariance Projector, Magnetite-Survey Guard & Mid-Hole Wedge/Redrill Controller

### 10.1 What Is Already in the Plan vs. What Is Materially New
- **Already in the plan:**
  - `§11.5(E)` and `BD-315` rank drill pads, parent holes, and daughter wedges *before* drilling using bundle-greedy
    monotone submodular coverage (`PortfolioCertificate`).
  - `§12.1–12.2` and `BD-286` ingest completed `Collar`, `Survey`, and `Interval` records, noting that magnetic surveys
    in magnetite-bearing ground are `Suspect` (`> 30 m` shifts in `150–200 m` holes, `E380`, `E381`) and poor core
    recovery is the main diamond-drilling error (`E378`).
- **What is missing (and materially new here):**
  - Deep diamond parent holes with directional daughter wedges (such as Fourmile's `800–1,200 m` holes `FM25-260DW1`,
    `E656`) cost `$250k–$600k` per hole and take `3–6 weeks` to drill. **Waiting until the hole is finished and the
    rig has demobilized to run Corebox checks is too late!**
  - While the hole is at depth `z_curr = 550 m` (on its way to target `z_target = 950 m`):
    1. Natural bit walk (foliation dip/azimuth lift and drop) can steer the projected pierce point `45 m` outside the
       target infill cell—leaving a gap in the Indicated spacing grid (`E142`) while redundantly twinning an existing
       hole!
    2. If the hole enters a magnetite-bearing skarn or alteration halo at `480 m`, magnetic multishot azimuths silently
       corrupt (`E380`) right when the driller needs to set a directional wedge kick-off!
    3. If the hole intersects the target mineralized contact with `52%` core recovery (`E378`), the geologist needs to
       know **immediately** (before rods are pulled and the rig leaves the pad) that a short `30 m` redrill wedge from
       `50 m` above the contact has `EVSI ≫ cost`!

### 10.2 Subsystems, Crates, Users & Workflows
- **Subsystems:** `Corebox`, `Lodestar`, `Strata`, `Helm` (`§0.5`).
- **Crates (`§28.3`, zero new crates):**
  - `fmine-drill` (K1 layer, budget 15k LOC): adds `fmine-drill::live_referee`, forward trajectory drift/diffusion
    integrator on the ISCWSA survey covariance model, and `MidHoleState`.
  - `fmine-qaqc` (K1 layer): runs real-time magnetic field total-intensity `|B|` and dip-angle anomaly checks (`G-DRILL-MAG`)
    on each downhole survey shot as it arrives.
  - `fmine-prospect` / `fmine-decide` (E layer): computes live **Mid-Hole Pierce-Point EVSI & Action Card**
    (`ContinueDrilling | RunGyroCheckNow | SetDirectionalWedge{kickoff_depth_m} | RedrillLostCoreZone{from_m}`).
- **Users & Workflows:**
  - **Exploration & Resource Geologists / Drill Supervisors:** After each daily driller shift report and downhole
    survey shot (`z_curr`), run `fmine drill referee <hole_id>` to get an instant trajectory cone to target and a
    priced decision card before the next shift starts.

### 10.3 Technical & Mathematical Design (`DESIGN`)
1. **Forward Trajectory Covariance & Pierce-Point Distribution (`z_curr → z_target`):**
   Given desurveyed position and covariance `(x(z_curr), Σ_pos(z_curr))` and tangent `(I(z_curr), A(z_curr))` at bit
   depth `z_curr`, the remaining path `s ∈ [z_curr, z_target]` follows a domain-calibrated tangential Ornstein–Uhlenbeck
   / constant-curvature walk `dI/ds = κ_I(domain) + σ_I dW_I`, `dA/ds = κ_A(domain)/sin I + σ_A dW_A`.
   Integrating to the target horizon surface `S_target` yields the predictive pierce-point Gaussian ellipse
   `X_pierce ~ N(μ_pierce(z_curr), Σ_pierce(z_curr))` on `S_target` (`PO-CORE-REF-1`).
2. **Real-Time Magnetic Interference Guard (`G-DRILL-MAG`):**
   A magnetic multishot tool measures `(g_x, g_y, g_z, B_x, B_y, B_z)`. Before trusting azimuth `A(z)`, `fmine-qaqc`
   checks the invariant:
   ```text
   | ‖B(z)‖ − B_IGRF(site) | ≤ τ_B    AND    | θ_dip(g(z), B(z)) − θ_IGRF(site) | ≤ τ_θ
   ```
   Any shot failing this invariant is immediately marked `SurveyState::MagneticSuspect` (`E380`, `E381`), inflating
   azimuth variance `σ²_A` to the un-surveyed bound and triggering `Action::RunGyroCheckNow` if `VOI_gyro > Cost_gyro`.
3. **Mid-Hole Wedge & Core-Loss Redrill Decision Rule:**
   - **Trajectory Steering:** If `P(X_pierce ∈ TargetCell | z_curr)` drops below threshold so that expected target
     coverage loss `ΔF = F(S_planned) − E[F(S_actual | z_curr)] > Cost_NaviCorrection`, recommends setting a
     navi-drill or wedge at `z_curr` (before drilling past the maximum dogleg-severity kick-off depth `z_max_kickoff`!).
   - **Core-Loss Redrill (`E378`):** When an interval in the target structure logs recovery `r_rec < r_min` (e.g.
     `< 70%`), the interval is degraded to `Conflicted(CoreLossBiasSuspect)` (cannot support Indicated classification!).
     `DrillholeReferee` compares the cost of kicking off a `40 m` daughter wedge from `z_curr − 45 m` *today* (`~$18k`
     while rods are in the hole) against the cost of re-mobilizing a rig and redrilling `800 m` next year (`~$320k`)!

### 10.4 Worked Galaxy-Brain Card (`GB-53`, Illustrative Values Anchored to Deep Wedge Drilling `E656`, `E378`, `E380`)
- **Scenario:** A `950 m` parent hole targeting a high-grade Carlin-type structural intersection (`35 m` Indicated grid
  spacing target, `E142`) is at `z_curr = 620 m`.
  - Shift survey at `620 m` shows inclination has lifted `+4.2°` faster than planned and `‖B‖` jumped `+2,800 nT`
    (entering a pyrrhotite/magnetite alteration halo).
  - Forward projection shows the hole will pierce the target horizon `48 m ± 19 m` east of target—leaving a `75 m`
    hole in the Indicated grid (preventing conversion of `85 koz` Inferred → Indicated worth `$4.2M` in reserve option
    value) and passing within `9 m` of an existing hole!
  - Maximum depth at which a `1.5°/30 m` directional wedge can still reach the target is `z_max_kickoff = 665 m`
    (**only `45 m` / 1.5 shifts of drilling away!**).
- **`DrillholeReferee` output:**
  - Flags `G-DRILL-MAG` at `620 m`; recommends **Run North-Seeking Gyro + Set Directional Wedge at `635 m`** (`cost =
    $28k`, net `EVSI = +$1.14M`). Waiting until `700 m` misses the geometric kick-off window and wastes `$190k` of
    drilling!

### 10.5 First Shippable Slice, Testing Strategy & Kill Criterion
- **First shippable slice (M2–M4 / WP-09 & WP-15):**
  - Implement `fmine-drill::live_referee` (forward ISCWSA trajectory ellipse, `G-DRILL-MAG` `‖B‖`/`θ_dip` guard,
    dogleg reachability cone, and core-loss redrill card) in `fmine-drill` + `fmine-qaqc` (~550 LOC).
- **Testing strategy (§33.1 Four Claims):**
  - **Parity:** At `z_curr = z_target` with zero remaining metres, matches the completed-hole minimum-curvature
    desurvey position and covariance (`PO-CORE-1`) to `< 1 mm`.
  - **Known answers:** On synthetic 100-hole directional fans with seeded magnetite anomalies and foliation drift,
    `DrillholeReferee` catches 100% of `> 1,000 nT` magnetic corruptions and increases target-cell hit rate from `64%`
    to `93%` at `< 6%` incremental meterage cost.
- **Kill criterion (`KILL-CORE-REF-1`):**
  - For shallow (`< 120 m`) vertical RC or aircore holes where directional wedging is physically inapplicable, disable
    mid-hole steering alerts and run only the post-hole collar/survey and `RC smearing` checks (`BD-285`).

---

## Idea 11: `StructuralAdversary` — Popperian Fault-Kinematics & Contact-Topology Challenger, En-Echelon Counter-Hypothesis Generator & 1-Hole Falsification VOI

### 11.1 What Is Already in the Plan vs. What Is Materially New
- **Already in the plan:**
  - `§13.2` defines `L1 Interpretation` (`StructuralModelRef`) as a versioned human act (`AttestedInterpretation`)
    holding a `Vec<(HypothesisId, Weight)>` plus a `catch_all` weight, and enforces the hard rule: **"Software never
    edits L1."**
  - `§13.7` (`G-STRATA-9`) raises a posterior-predictive surprise event if a new drillhole intercept contradicts an
    existing L1 wireframe beyond tolerance.
  - `§15.2` cites Bond et al. 2007 (`E639`: only 21% of 412 interpreters identified the right tectonic setting on a
    synthetic seismic image), and `§14.1` cites Carlin's GQ7CN `~1 Mt` highwall failure (`E195`: LOM slope designs
    rested on a 2008 geology model).
- **What is missing (and materially new here):**
  - In practice, resource geologists almost always upload **a single deterministic wireframe set** (`hypotheses.len() ==
    1`), because drawing three competing 3D structural interpretations by hand in Leapfrog or Vulcan takes weeks! If
    L1 holds only one human wireframe, `BeliefSpec`'s multi-hypothesis mixture (`§13.3`) is starved of alternatives
    until after a stope or pit wall fails!
  - Furthermore, the single most common geological disaster in gold and copper mining is **geometric aliasing between
    drill sections**: linking high-grade intercepts across `50 m`-spaced sections into one continuous steeply dipping
    sheet when the reality is a stack of short **en-echelon flat/oblique lenses** (or an unmapped post-mineral fault
    offset)—causing `30%–50%` F1 tonnage wipeouts and exposing unfavorable bedding/fault planes in the highwall!
  - Without ever editing L1 authority (`§13.2` preserved!), `fmine-belief::adversary` acts as a **Popperian Structural
    Challenger**: it audits any human L1 wireframe against L0 structural/assay data, synthesizes candidate
    `QuarantinedCounterHypothesis` alternatives in `CELL-QUAR` for the geologist to review, computes the **Single-Hole
    Popperian Falsification VOI** (the exact drillhole that maximally kills one of the competing geometries), and
    checks whether any counter-hypothesis creates a kinematic daylighting hazard in `Highwall` (`§14.2`)!

### 11.2 Subsystems, Crates, Users & Workflows
- **Subsystems:** `Strata`, `Corebox`, `Highwall`, `Lodestar` (`§0.5`).
- **Crates (`§28.3`, zero new crates):**
  - `fmine-belief` (K3 layer, budget 25k LOC): implements `fmine-belief::adversary` (kinematic admissibility checker,
    alias/en-echelon counter-hypothesis generator, and structural Bayes-factor / JSD discriminator).
  - `fmine-geom` (K0 layer): checks mesh watertightness, fault-chronology (`cut/onlap/erode`) topological consistency,
    and oriented-core tangent residuals.
  - `fmine-geotech` (K2 layer): evaluates whether a candidate structural counter-hypothesis introduces a planar/wedge
    daylighting orientation in any open-pit design sector (`§14.2`).
- **Users & Workflows:**
  - **Resource Geologist & QP:** Before signing an L1 interpretation or infill drill campaign, runs `fmine strata
    challenge <spec_hash>` to inspect kinematic violations, see the top aliased counter-hypotheses, and locate the
    single highest-leverage scissors/oblique test hole to falsify geometric aliasing.
  - **Geotechnical Engineer (`Highwall`):** Checks whether an unmapped fault or alternative contact dip compatible with
    current drillholes could daylight in an active highwall (`E195`).

### 11.3 Technical & Mathematical Design (`DESIGN`)
1. **Three Structural Admissibility Audits on L1 Wireframes (`G-STRATA-KIN-1..3`):**
   - **Oriented-Core Tangent Discrepancy (`G-STRATA-KIN-1`):** At every drillhole contact pierce point `x_i` where
     `Corebox` holds an oriented-core `Structure` measurement (`α, β` true normal `n_obs,i` with concentration `κ_i`,
     `§12.1`), computes the angular discrepancy `cos θ_i = |n_wireframe(x_i) · n_obs,i|`. If the Watson/Fisher test
     rejects alignment across a domain (`p < 0.01`), flags **Systematic Contact Dip/Strike Aliasing**!
   - **Cross-Hole Stair-Step Offset Detector (`G-STRATA-KIN-2`):** When a wireframe bends by an inflection `Δz > h_crit`
     between adjacent holes `H_a, H_b` without a mapped fault in L1, compares the bending energy `∫ ‖∇²φ‖² dx` of the
     continuous flexing surface against a **Discontinuous Fault-Offset Counter-Hypothesis** (a planar fault with throw
     `Δz` cutting a planar horizon).
   - **Chronology & Volume-Conservation Audit (`G-STRATA-KIN-3`):** Verifies that fault displacements conserve
     hanging-wall vs footwall stratigraphic thickness and obey declared `cut/onlap/erode` ordering.
2. **Single-Hole Popperian Falsification VOI:**
   Given human baseline `H_0` (e.g. one continuous `70°`-dipping vein linking intercepts in `H_1, H_2`) and
   challenger `H_1` (three `25°`-dipping en-echelon tension veins consistent with the oriented-core `α/β` data), a
   candidate test hole `h` (defined by collar, azimuth, dip, length) has predictive contact-depth distributions
   `p(y | H_0, h)` and `p(y | H_1, h)`. `fmine-belief::adversary` ranks candidate holes by:
   - **Epistemic Falsification Power:** Jensen–Shannon divergence `JSD(p(y | H_0, h) ∥ p(y | H_1, h))` (maximized by a
     "scissors" hole drilled parallel to `H_0` and perpendicular to `H_1`, or through the predicted gap between
     en-echelon lenses!); and
   - **Decision-Theoretic EVSI (`§22`):** Expected reduction in Pushback stope/pit misclassification loss (`ore loss +
     unplanned dilution`) when designing stopes under `H_0` vs `H_1`!

### 11.4 Worked Galaxy-Brain Card (`GB-54`, Illustrative Values Anchored to Structural Aliasing & `E639`, `E195`)
- **Scenario:** Four vertical drillholes (`50 m` spacing) each hit a `4 m @ 8.0 g/t Au` quartz-carbonate interval at
  depths stepping down `50 m` per hole (`100 m, 150 m, 200 m, 250 m`).
  - **Human L1 Wireframe (`H_0`):** Connects all four intercepts into a single continuous `45°`-dipping vein (`220 m`
    dip length, `450 koz` Indicated target).
  - **`StructuralAdversary` audit:** Notices that oriented-core vein-margin measurements (`Structure` records in
    `Corebox`) in all four holes dip at **`15° ± 6°`**, not `45°` (`G-STRATA-KIN-1` triggers with `p = 4 × 10⁻⁶`)!
  - **Synthesized Challenger (`H_1` in `CELL-QUAR`):** Four stacked, `15°`-dipping en-echelon tension gashes (`60 m`
    wide each, separated by `35 m` of barren wall rock), containing `195 koz` (`−57%` less metal than `H_0`!), and a
    stope layout built on `H_0` would mine **62% barren waste rock**!
  - **Falsification Hole Recommendation:** One `180 m` hole inclined at `−60°` opposite to `H_0` between Holes 2 and 3
    has `P(hit vein | H_0) = 0.96` vs `P(hit vein | H_1) = 0.04` (`Cost = $36k` to resolve a `$48M` stope-design error!).

### 11.5 First Shippable Slice, Testing Strategy & Kill Criterion
- **First shippable slice (M3–M4 / WP-16):**
  - Implement `G-STRATA-KIN-1..3` (oriented-core normal vs wireframe normal test and stair-step fault-vs-flexure test)
    and the 1-hole falsification scorer in `fmine-belief::adversary` (~650 LOC).
- **Testing strategy (§33.1 Four Claims):**
  - **Known answers:** On synthetic `fmine-gen` deposits with seeded en-echelon lenses and unmapped `20 m` normal
    faults, `StructuralAdversary` flags 100% of `> 20°` oriented-core vs wireframe mismatches and selects a scissors
    hole that resolves the true topology in 1 hole across `≥ 95%` of replicates.
  - **Invariant check:** Type-level verification that `fmine-belief::adversary` has zero write path to `L1` authority
    and outputs only to `CELL-QUAR` / `ClassificationSupportPack`.
- **Kill criterion (`KILL-STRATA-ADV-1`):**
  - If a deposit has zero oriented-core `Structure` records (`§12.1`), disable `G-STRATA-KIN-1` and run only the
    cross-hole stair-step fault-offset vs flexure discriminator (`G-STRATA-KIN-2`).

---

## Idea 12: `CompositionalGeometBridge` — Schedule-Weighted Multivariate Area-of-Applicability (AOA) Scanner & D-Optimal Core-Composite Selector

### 12.1 What Is Already in the Plan vs. What Is Materially New
- **Already in the plan:**
  - `§13.12` and `§17.4` highlight `OPEN-STRATA-8` ("the minimum geometallurgical sampling density at which transfer
    cards beat domain averages").
  - `§17.4` and `BD-300` add a **1D marginal percentile coverage report** checking whether each primary variable's 5th,
    10th, ..., 95th percentile in the SMU distribution is covered by tested `GeometTest` samples (`E389`).
- **What is missing (and materially new here):**
  1. **The Multivariate Corner Blind-Spot (Why 1D Percentile Coverage Fails):** In refractory gold and copper-gold
     circuits (Carlin roasters & autoclaves `E172`, `E173`, `E175`; Turquoise Ridge `E177`, `E181`; Pueblo Viejo `E265`),
     recovery shortfalls can arise from **joint multivariate interactions**—such as `moderate Au (50th pct) + high
     preg-robbing TOC (80th pct) + low carbonate CO3 (15th pct) + high As (85th pct)`. Every single variable sits inside
     its `[5th, 95th]` 1D marginal band (so `BD-300` passes green!), yet **zero metallurgical test composites exist in
     that joint corner**!
  2. **Schedule-Weighted Exposure (Year 2 Feed vs. Year 18 Feed):** A gap in a domain corner that won't be mined until
     Year 16 has low urgency, whereas an untested multivariate cluster constituting `35%` of Q3 2027 mill feed is an
     immediate operational cliff!
  3. **Optimal Core-Composite Selection from Physical `Corebox` Inventory:** Selecting 25 new metallurgical composites
     (`$20k–$50k` per autoclave/roaster/locked-cycle test) is currently done by hand. `CompositionalGeometBridge` solves
     a **Schedule-Weighted D-Optimal / Expected-Loss Composite Selection Optimization** over available, non-oxidized
     half-core and coarse-reject intervals in `Corebox`!

### 12.2 Subsystems, Crates, Users & Workflows
- **Subsystems:** `Strata`, `Crucible`, `Corebox`, `Pushback` (`§0.5`).
- **Crates (`§28.3`, zero new crates):**
  - `fmine-materials` (K0 layer, budget 12k LOC): adds `MultivariateGeometAoa` (Mahalanobis distance + directional
    convex-hull / isolation-depth enclosures in ILR/standardized feature space).
  - `fmine-plant` (K2 layer): weights AOA gaps by `Pushback`'s scheduled period feed `x_{spt}` (`§15.6`) and solves
    the greedy submodular / D-optimal core-interval selector over `Corebox` `Sample` records (`§12.1`).
- **Users & Workflows:**
  - **Chief Metallurgist, Geometallurgist & QP:** Runs `fmine plant geomet-aoa --schedule <plan_id> --select-samples 20`
    to see a period-by-period map of scheduled mill feed that falls outside multivariate testwork coverage, plus the
    exact list of 20 drillhole intervals (`hole_id, from_m, to_m, core_tray_id`) to pull from the core shed to close
    those gaps!

### 12.3 Technical & Mathematical Design (`DESIGN`)
Let the geometallurgical state vector of an SMU block or composite in domain `d` be `z = T_ilr(Au, Ag, S_sulphide,
CO3, TOC, As, CuCN, BWi, Clay) ∈ ℝ^p` (transformed to isometric log-ratio `ILR` for closed compositions `§13.6` and
standardized by the domain covariance `Σ_d`).
1. **Multivariate Dissimilarity Index `DI(z)` (`Meyer & Pebesma AOA` + Mahalanobis):**
   Given existing located `GeometTest` composites `Z_test = {z_1, ..., z_m}` in domain `d`:
   ```text
   d_min(z, Z_test) = min_{j=1..m} √( (z − z_j)ᵀ W_card (z − z_j) )
   DI(z)            = d_min(z, Z_test) / d̄_LOO(Z_test)
   ```
   where `W_card = diag(|∂Recovery/∂z_k|² + |∂Throughput/∂z_k|²)` weights dimensions by their local sensitivity on the
   `CorrelationCard` (`§17.3`), and `d̄_LOO(Z_test)` is the mean leave-one-out nearest-neighbour distance among the
   training composites. Any block with `DI(z) > τ_AOA` (the 95th percentile of LOO distances) is **multivariately
   outside the Area of Applicability** (`OutsideMultivariateAoa`), even if every 1D coordinate lies in `[P10, P90]`!
2. **Schedule-Weighted Predictive Variance & Core-Shed Sample Selection:**
   For candidate available drillcore composites `c ∈ C_shed` (filtered by `Corebox` retention state `!= Discarded`,
   `E224`, and sulphide core-storage oxidation age `t_now − collection_time ≤ τ_fresh_core` via `QuadTemporalStamp`!),
   adding sample set `S ⊆ C_shed` (`|S| ≤ K`) to the Gaussian-process / polynomial `CorrelationCard` design matrix `X`
   reduces the posterior predictive variance `σ²_card(z_b | Z_test ∪ S)` of scheduled blocks `b`.
   The objective:
   ```text
   max_{S ⊆ C_shed, |S| ≤ K}  Σ_{t=1}^{T} δ_t Σ_{b ∈ MillFeed(t)} Tonnes(b, t) · [ σ²_card(z_b | Z_test) − σ²_card(z_b | Z_test ∪ S) ]
   ```
   is **monotone submodular** in `S` under Gaussian/linearized card posteriors (`Krause & Guestrin`), so lazy greedy
   (`CELF`, reusing the exact pattern of `§11.5(E)`) achieves a certified `(1 − 1/e)` optimality bound (`PO-CRUC-AOA-1`)!

### 12.4 Worked Galaxy-Brain Card (`GB-55`, Illustrative Values Anchored to Carlin Refractory Ores `E172`, `E175`)
- **Scenario:** A refractory gold domain has `45` existing roaster/autoclave test composites.
  - Marginal 1D check (`BD-300`): `Au` (`1.5–12 g/t`), `SS` (`0.6–2.8%`), `CO3` (`4–22%`), and `TOC` (`0.2–1.6%`) are
    all `100%` covered across their 5th–95th percentiles.
  - **Multivariate check (`CompositionalGeometBridge`):** Discovers that in `Q3–Q4 2027`, Pushback schedules `1.8 Mt`
    (`165 koz Au`) from the lower footwall where **`SS = 2.1%` (75th pct) co-occurs with `CO3 = 5.2%` (15th pct) and
    `TOC = 1.1%` (80th pct)**.
  - In `Z_test`, high `SS` only ever occurred with high `CO3` (`> 14%`) or low `TOC` (`< 0.4%`)! The footwall blend has
    `DI(z) = 2.84 > τ_AOA = 1.15`: it is too low in carbonate for alkaline autoclave mode or roaster self-neutralization
    (`E172`), and too high in preg-robbing `TOC` for acid autoclave + CIL without blanking (`E173`, `E175`)!
  - **Core Selector output:** Identifies `8` preserved diamond-core intervals from the 2025 infill campaign sitting in
    the site core library that span this exact `(high SS, low CO3, high TOC)` corner (`Testwork cost = $160k`; protects
    `$72M` of Q3–Q4 2027 revenue from an unpredicted `-18 pp` recovery crash).

### 12.5 First Shippable Slice, Testing Strategy & Kill Criterion
- **First shippable slice (M2–M3 / WP-10 & WP-16 `BD-300`):**
  - Extend `BD-300` in `fmine-plant` / `fmine-materials` with `MultivariateGeometAoa` (`DI(z)` calculation) and the
    CELF greedy core-composite selector (~500 LOC).
- **Testing strategy (§33.1 Four Claims):**
  - **Known answers:** Synthetic 3-variable deposit where training samples lie on the diagonal `(x ≈ y)` while Year 2
    mining hits the off-diagonal `(x high, y low)`: 1D check reports `0%` out-of-domain, whereas `MultivariateGeometAoa`
    flags `100%` of the off-diagonal blocks and selects the exact off-diagonal core intervals.
- **Kill criterion (`KILL-CRUC-AOA-1`):**
  - When a domain has `< 10` total `GeometTest` samples (`E389`), covariance `Σ_d` is under-determined; fall back to
    axis-aligned hyperbox + L2 distance in standardized coordinates and label the card `InsufficientGeometSupport`.

---

## Idea 13: `PermitAndCommitmentCriticalPathCompiler` — Coupled Permit-Dependency DAG, Volumetric TSF/Water Runway & Trigger-Curtailment Shadow Pricer

### 13.1 What Is Already in the Plan vs. What Is Materially New
- **Already in the plan:**
  - `§18.2` (`BD-247`, `BD-301`) tracks `RequirementSet` (`GISTM, ESMP, PermitCondition, IFC_PS, EP4`) and
    `EsCommitment` rows with `OB-COMMIT` obligations (demonstrated on an ESIA, `E304–E348`).
  - `§18.6` (`BD-304`) notes that tailings capacity runway is a monitored quantity (e.g. Goldstrike's storage lasts
    "through 2028" while a 180 Mm³ TSF is in permitting, `E190`; Kibali's FTSF limits oxide feed until 2028, `E218`;
    Loulo's delayed TSF expansion forced over-height deposition behind a sacrificial buttress, `E219`).
  - `§23.2` (`BD-297`) attaches `PermitGate` to capital `StageGate`s.
- **What is missing (and materially new here):**
  - In the current plan, `Freeboard`'s permit/commitment register (`§18.2`), `Freeboard`'s water/TSF balances
    (`§18.3–18.4`), and `Pushback`'s production scheduler (`§15.6`) touch only through a single static TSF volume cap!
  - In reality, **operating mines face a coupled network of time-limited permits, volumetric exhaustion runways, and
    environmental trigger curtailments**:
    1. **Volumetric & Spatial Runway Exhaustion:** Pit-layback boundary permits, archaeological/species clearance
       buffers (`E345`), waste-dump lift height limits, and TSF stage-raise permits (`E190`, `E218`, `E219`) each
       impose a time-indexed capacity step function `Cap_k(t; T_approval_k)` where `T_approval_k` is a random variable
       governed by a regulatory agency dependency DAG and reference-class delay prior (`REFCLASS-PERMIT`).
    2. **Environmental Trigger-to-Production Curtailment Laws:** At Veladero (`E278`, `E280`, `E281`), crossing
       an environmental trigger (in-heap solution `PSSA > 3,927 MASL` or `LCRS > 270 m³/d`) **restricts water
       make-up and cyanide addition** (2017: `21.87%` below budget). Similarly, a borefield permit below installed
       capacity (`E312`, `E313`) directly caps mill ramp-up throughput!
  - `PermitAndCommitmentCriticalPathCompiler` compiles all active permits, renewal dates, agency prerequisite edges,
    and trigger-curtailment rules into a unified **Stochastic Permit–Operations Critical Path Graph** (`fmine-tailings::
    permit_dag`) that computes for every permit and ESIA commitment its **Live Production-At-Risk (`koz/month`) and
    Shadow Price (`$M per month of permitting delay`)**!

### 13.2 Subsystems, Crates, Users & Workflows
- **Subsystems:** `Freeboard`, `Helm`, `Pushback` (`§0.5`).
- **Crates (`§28.3`, zero new crates):**
  - `fmine-tailings` (K2 layer, budget 18k LOC): adds `OperationalPermitNode`, `TriggerCurtailmentRule`, and
    `PermitDependencyDag` (evaluated with lifted `fs-tropical` max-plus critical path `§13.9` + Monte Carlo over
    `REFCLASS-PERMIT` lead-time distributions).
  - `fmine-plan` (K3 layer) & `fmine-capital` (E layer): couples `Cap_k(t; T_approval_k)` across the shared scenario
    index `r = 1..R` (`§23.4`) to compute each permit's **Curtailment CVaR & Expediting/Mitigation ROI Card**.
- **Users & Workflows:**
  - **Executive Committee, VP Permitting/Sustainability & Site GM:** Runs `fmine tailings permit-runway` to see a
    ranked dashboard of every permit, raise approval, and water/discharge cap across the company sorted by
    **Expected Production Curtailment (`koz` / `$M` at risk over the next 36 months)** and the exact date `t_choke`
    when operations must curtail if an approval slips.

### 13.3 Technical & Mathematical Design (`DESIGN`)
1. **Coupled Tropical Lead-Time & Volumetric Depletion Equation:**
   For each constrained operational resource `k` (TSF stage volume `V_TSF`, pit phase polygon `Π_phase`, borefield
   abstraction `Q_water`, PAG dump cell `V_PAG`), let `R_k(t) = Cap^current_k − ∫_0^t u_k(τ) dτ` be the remaining
   permitted runway under the `Pushback` schedule consumption rate `u_k(τ)`. The **Choke Time** under realization `r` is:
   ```text
   t_choke,k(r) = inf { t > t_now : ∫_{t_now}^t u_k(τ, r) dτ ≥ R_k(t_now) }
   ```
   Meanwhile, the next permit expansion `Cap^next_k` sits at the end of a prerequisite DAG `G_permit` (baseline study →
   ESIA submission `OB-COMMIT` → public comment clock → agency review → construction of raise/liner → EoR sign-off).
   In max-plus (tropical) algebra (`fmine-numerics::tropical`), the earliest readiness date is:
   ```text
   T_ready,k(r) = max_{path π ∈ G_permit} Σ_{e ∈ π} d_e(r)
   ```
   Whenever `T_ready,k(r) > t_choke,k(r)`, the **Permit Deficit Window** `Δt_curtail,k(r) = T_ready,k(r) − t_choke,k(r)`
   forces `Pushback` into its **Curtailment Recourse Policy** (either throttling mill throughput by `1 − α_curtail`,
   stockpiling refractory/PAG ore with rehandle cost `c_rehandle`, or building a sacrificial buttress as at Loulo
   `E219`), losing cash flow:
   ```text
   Loss_k(r) = ∫_{t_choke,k(r)}^{T_ready,k(r)} λ_cap,k(τ, r) · u_k(τ, r) dτ
   ```
   where `λ_cap,k` is `Pushback`'s dual price on constraint `k`!

### 13.4 Worked Galaxy-Brain Card (`GB-56`, Illustrative Values Anchored to `E190`, `E218`, `E278`, `E312`)
- **Scenario:** A complex's existing TSF has `14.2 Mm³` of remaining permitted volume (`E190` pattern). Planned mill
  throughput consumes `6.8 Mm³/yr` of tailings storage (`t_choke = 2.09 years = 25.1 months`).
  - The new TSF requires: (1) final hydrogeological baseline (`3 months`), (2) EIS Record of Decision (`REFCLASS-PERMIT`
    median `14 months`, P80 `21 months`, P90 `26 months`), and (3) Phase-1 starter embankment & liner construction
    (`10 months`, dry-season gated).
- **Deterministic Gantt chart:** `3 + 14 + 10 = 27 months` (already `1.9 months` late at the median, which an
  `11-month` permitting assumption would not show).
- **`PermitAndCommitmentCriticalPathCompiler` output:**
  - Across `1,000` joint `(permitting × weather × production)` scenarios:
    - `P(T_ready > t_choke) = 78%`!
    - Expected curtailment window = `4.8 months` (P90 = `9.5 months`), risking **`215 koz Au` (`$410M` deferred FCF)**!
    - **Mitigation DecisionCard:** Pre-investing `$18M` *now* in an interim crest raise / in-pit tailings diversion or
      early-works liner procurement shrinks `E[Loss]` by `$195M` (`NPV / Cost = 10.8×`)!

### 13.5 First Shippable Slice, Testing Strategy & Kill Criterion
- **First shippable slice (M3–M5 / WP-14 & WP-18):**
  - Implement `PermitDependencyDag` (on `fmine-numerics::tropical` + Monte Carlo `REFCLASS-PERMIT`), volumetric
    `t_choke` intersection, and `TriggerCurtailmentRule` in `fmine-tailings` (~550 LOC), with fixtures for Goldstrike
    (`E190`), Kibali (`E218`), Veladero (`E278`, `E280`), and an abstraction permit (`E312`).
- **Testing strategy (§33.1 Four Claims):**
  - **Parity:** Deterministic critical path (`σ = 0`) matches `fmine-numerics::tropical` (`crates/fmine-numerics/src/
    lib.rs:283–347`) exactly.
  - **Known answers:** Injecting Veladero's 2017 cyanide/water restriction (`E278`) into the curtailment rule reproduces
    the `21.87%` production shortfall against budget.
- **Kill criterion (`KILL-FREE-PERM-1`):**
  - If agency review durations in a jurisdiction lack an admissible reference class (`< 5` historical permits), report
    only deterministic `t_choke − T_min_statutory` slack and the `$/month` curtailment shadow price (`G-HIGH-2` pattern:
    flip margin stated, probability omitted).

---

## Idea 14: `UndergroundStopeSequenceStressAndFillCoupler` — Boundary-Influence Stress-Shadow Matrix, Anti-Pillar-Trap MILP Cuts & Paste-Binder Cure Co-Optimizer

### 14.1 What Is Already in the Plan vs. What Is Materially New
- **Already in the plan:**
  - `§14.3` computes Mathews/Potvin stability number `N'` vs hydraulic radius per stope and tracks microseismic event
    rates.
  - `§15.5` schedules underground stopes via an activity network on `fmine-lp::milp` (`≤ 10⁴` variables) and notes that
    Turquoise Ridge underground dilution carries in-situ `TCM` at `0 g/t Au`, lowering recovery (`E187`).
  - `§16.3` (`BD-307`, `BD-320`) models `FillStrength(t)` with early-age cure and late-age sulphate degradation (`Loulo`
    `3.5%` binder falling from `2.65 MPa` at `56 d` to `1.10 MPa` at `112 d`, `E233`; `Kibali` slag-cement degradation,
    `E234`), `DilutionMatrix`, and refusal guard `G-HEAD-7` (`FMINE-REF-504: AdjacentFillUncured`).
- **What is missing (and materially new here):**
  1. **Sequence-Dependent Stress Transfer & Diminishing-Pillar Traps:** In `§14.3` and `§15.5`, each stope's stability
     `N'` and dilution (`ELOS`) are treated as **static constants independent of the extraction order**, and `§14.2`
     notes that full 3D elastoplastic FEM (`fs-solid`) is not yet available (`E007`). In real underground stoping
     (Turquoise Ridge, Goldrush, Kibali, Loulo/Yalea/Gara, Bulyanhulu), mining primary stopes sheds in-situ stress onto
     adjacent unmined secondary stopes! If a naive MILP schedule mines from both ends of an orebody toward the middle,
     it creates a **diminishing central rib or sill pillar** where induced tangential stress `σ_θ` triples—causing
     seismic lockouts, blasthole squeezing, and `3×` higher wall sloughing (at Turquoise Ridge, dilution carries
     in-situ `TCM` at zero gold into the autoclave feed, `E187`)!
  2. **Endogenous Paste-Fill Binder Recipe vs. Stope Exposure Window Co-Optimization:** Because `Loulo`-style paste
     fill *degrades* after `56–112 days` from sulphate attack (`E233`), a primary stope filled with a `3.5%` binder
     has a **two-sided time window `[t_cure_min, t_degrade_max]`** during which the adjacent secondary stope *must* be
     mined before `FillStrength(t)` falls back below `UCS_design` and trips `FMINE-REF-504` (`G-HEAD-7`)! Either the
     scheduler mines the secondary stope inside `[t_cure_min, t_degrade_max]`, or it must specify a higher-binder/slag-
     resistant paste recipe (`5.0%` binder, `+$6/t`) when pouring the primary stope!

### 14.2 Subsystems, Crates, Users & Workflows
- **Subsystems:** `Highwall`, `Pushback`, `Headframe` (`§0.5`).
- **Crates (`§28.3`, zero new crates):**
  - `fmine-geotech` (K2 layer, budget 22k LOC): adds `StopeStressInfluenceMatrix` (analytical 3D Eshelby / displacement-
    discontinuity elastic stress-transfer kernel `K_{i←j}` between stope centroids/facets, requiring **zero `fs-solid`
    dependency**!) and maps induced stress `σ_1(x_t)` onto Mathews stability factor `A(σ_1 / UCS)` → dynamic
    `ELOS_i(x_t)`.
  - `fmine-plan` (K3 layer): injects **Linear Stress-Cap & Anti-Pillar-Trap Cuts** and **Two-Sided Paste Cure/Degradation
    Window Constraints (`[τ_cure(recipe), τ_degrade(recipe)]`)** into `fmine-plan::sched` (`fmine-lp::milp`).
- **Users & Workflows:**
  - **Underground Mine Planner, Rock Mechanics Engineer & Backfill Engineer:** Co-optimizes stope extraction sequence
    and paste-fill binder recipes so that no stope exceeds its induced stress factor `A_min`, no diminishing pillar trap
    is created, and no secondary stope is stranded behind degraded fill (`E233`, `G-HEAD-7`).

### 14.3 Technical & Mathematical Design (`DESIGN`)
1. **Linearized Elastic Stress-Transfer Influence Matrix (`StopeStressInfluenceMatrix`):**
   When stope `j` of volume `V_j` and span `w_j` is extracted (`x_{j,t} = 1` if mined by period `t`, `f_{j,t} = 1` if
   backfilled by period `t`), it sheds stress onto unmined stope `i` at distance `r_{ij}` with elastic Boussinesq/McCreath
   influence coefficient `K_{ij} = γ_stress · V_j / (4π r_{ij}³) · Ψ(orientation_{ij})` (reduced by `(1 − β_fill)` once
   stope `j` is tight-filled):
   ```text
   σ_induced,i(t) = σ_0,i + Σ_{j ∈ Neighbours(i)} K_{ij} · (x_{j,t} − β_fill · f_{j,t})
   ```
   Notice that **`σ_induced,i(t)` is strictly linear in the binary schedule variables `(x_{j,t}, f_{j,t})`!**
   - **Hard Stress-Ratio Cut:** To prevent rockburst / brow squeeze when stope `i` is mined in period `t`
     (`Δx_{i,t} = x_{i,t} − x_{i,t-1} = 1`), the MILP enforces:
     ```text
     σ_0,i + Σ_j K_{ij} (x_{j,t} − β_fill f_{j,t}) ≤ σ_max,i + M_big · (1 − Δx_{i,t})
     ```
   - **Topological Anti-Pillar-Trap Cut:** For any contiguous triplet of stopes `(i−1, i, i+1)` along strike or dip,
     forbids leaving `i` as an isolated unmined pillar between two mined neighbours unless `i` is mined first or last in
     a monotone retreat order:
     ```text
     x_{i-1,t} + x_{i+1,t} − x_{i,t} ≤ 1    (∀t: never strand stope i between mined i−1 and i+1!)
     ```
2. **Two-Sided Paste Cure & Late-Age Degradation Window (`E233`, `G-HEAD-7`):**
   For primary stope `p` poured at period `t_pour` with binder recipe `r ∈ {Standard_3.5pct, SulphateRes_5.0pct}`, its
   strength satisfies `FillStrength(Δt; r) ≥ UCS_design` iff `Δt ∈ [τ_min(r), τ_max(r)]` (for `Loulo` `3.5%` binder,
   `τ_min = 28 d`, `τ_max = 84 d` before dropping toward `1.10 MPa` at `112 d`, `E233`!). Thus adjacent secondary stope
   `s` must either be extracted within `[t_pour + τ_min(r), t_pour + τ_max(r)]` or stope `p` must be poured with
   `r = SulphateRes_5.0pct` (`τ_max = ∞`)!

### 14.4 Worked Galaxy-Brain Card (`GB-57`, Illustrative Values Anchored to `E187`, `E233`, `G-HEAD-7`)
- **Scenario:** A 5-stope high-grade underground panel (`50,000 t` per stope at `9.0 g/t Au`, wall rock has `2.8% TCM`
  at `0 g/t Au`, `E187`).
  - **Naive MILP schedule:** Mines Stopes 1, 3, and 5 first (highest grades), leaving Stopes 2 and 4 as secondary
    pillars, and pours `3.5%` paste binder in Stopes 1, 3, 5; then mines Stope 2 at Day 120.
  - **What goes wrong physically:**
    1. At Day 120, the `3.5%` paste in Stopes 1 and 3 has degraded from `2.65 MPa` to `1.10 MPa` (`E233`) < `1.8 MPa`
       required for two exposed walls (`E236`) → **`G-HEAD-7` (`FMINE-REF-504`) refuses the Stope 2 blast plan**, or if
       mined, paste sloughing adds `12%` dilution;
    2. Stress shedding from Stopes 1 & 3 onto Pillar 2 drops Mathews stress factor `A` from `0.7` to `0.2`, increasing
       hanging-wall `ELOS` from `0.4 m` to `1.6 m` (`+20%` carbonaceous wall dilution, spiking blend `TCM` and cutting
       autoclave/CIL gold recovery by `-4.2 pp`, `E187`)!
- **`UndergroundStopeSequenceStressAndFillCoupler` schedule:**
  - Enforces centre-outward retreat (`3 → 2 → 1` and `3 → 4 → 5`, zero diminishing pillars) and upgrades Stope 3 to
    `5.0%` binder (`+$220k` cement cost), avoiding `FMINE-REF-504`, cutting `TCM` dilution by `65%`, and preserving
    `+$4.8M` of recovered gold!

### 14.5 First Shippable Slice, Testing Strategy & Kill Criterion
- **First shippable slice (M4–M5 / WP-18 & WP-19):**
  - Implement `StopeStressInfluenceMatrix`, the `x_{i-1,t} + x_{i+1,t} − x_{i,t} ≤ 1` anti-pillar-trap cuts, and the
    two-sided `[τ_min(r), τ_max(r)]` paste recipe constraints in `fmine-geotech` + `fmine-plan` (~600 LOC).
- **Testing strategy (§33.1 Four Claims):**
  - **Parity:** Analytic stress influence around a spherical/tabular opening matches Kirsch / Eshelby closed-form solutions
    within `2%`.
  - **Known answers:** On a synthetic 15-stope panel with `Loulo` `FillStrength(t)` degradation (`E233`), `fmine-verify`
    confirms `0` `FMINE-REF-504` violations and `0` isolated pillar states across all periods.
- **Kill criterion (`KILL-HIGH-UG-1`):**
  - If linear influence cuts increase MILP solve time above the `10 min` SLA (`§15.4`), apply the anti-pillar-trap
    topological cuts (`O(N)` rows) in the MILP and evaluate stress-induced `ELOS` as a post-solve certificate check.

---

## Idea 15: `CounterfactualHindsightGuard` — Transitive Temporal-Cone Prover (`verify_decision_cone`) & Future-Zeroed Ledger Replay Oracle (`PO-TIME-CONE`)

### 15.1 What Is Already in the Plan vs. What Is Materially New
- **Already in the plan:**
  - `§6.3`, `§25.4`, and `BD-309` define `QuadTemporalStamp` (`physical_valid_time`, `collection_time`,
    `availability_time`, `record_time`) and guard `G-TIME-2` (`FMINE-REF-108: AssayLagLookahead`), currently implemented
    in `crates/fmine-time/src/lib.rs:228–272` for individual timestamps.
  - `§9.4` (`BD-053`), `§20` (`BD-161`), `§22` (`BD-226`), and `§23` (`BD-230`) require bitemporal rolling-origin
    hindcasts and retrospective `DecisionCard`s built "as known at decision time `t_dec`".
- **What is missing (and materially new here):**
  - Checking `obs.availability_time <= t_dec` on raw L0 observations (`G-TIME-2`) is **necessary but NOT sufficient**
    to prevent hindsight leakage in a multi-layer DAG!
  - Why? Because in a complex system (`L0 Evidence → L1 Wireframes → L2 Variogram Hyperparameters → L3 Recovery
    CorrelationCards → Price/Cost Decks → Exclusion Lists → L4 Projections → DecisionCard`), **future information
    routinely leaks through five side channels**:
    1. **Hyperparameter Lookahead:** Filtering drillholes to `availability_time <= 2024-01-01`, but kriging or
       simulating them using a variogram range/nugget (`L2`) or `CorrelationCard` (`L3`, e.g. Turquoise Ridge recovery
       regression `E177`) that was fitted on `2024–2025` data!
    2. **Structural Wireframe Lookahead:** Projecting `2023` assays inside an `L1` domain wireframe drawn by a geologist
       in `2025` (`AttestedInterpretation` timestamp `> t_dec`)!
    3. **QA/QC Exclusion Lookahead:** Using an `EvidenceCut` exclusion list (`§13.3`) that drops a biased drill campaign
       only discovered via twin holes in `2025`!
    4. **Reference-Class / Bias-Prior Lookahead:** Conditioning a `2024` hindcast on an F-factor bias hyperprior updated
       by `2024Q4` reconciliation!
    5. **Code / Registry Lookahead:** Evaluating `as_of_decision(t_dec)` against a `PriceDeck` or `MetricDefinition`
       registered after `t_dec`!
  - `CounterfactualHindsightGuard` closes every single one of these leaks with a **Transitive DAG Temporal-Cone Prover**
    in `fmine-claim` / `fmine-verify` and a **Physical Byte-Zeroing Replay Oracle (`PO-TIME-CONE`)** in `fmine-lab`!

### 15.2 Subsystems, Crates, Users & Workflows
- **Subsystems:** `Plumb`, `Bedrock`, `Testpit` (`§0.5`).
- **Crates (`§28.3`, zero new crates):**
  - `fmine-claim` (S0 layer, budget 16k LOC, currently ~594 LOC): extends `ClaimDag` (`crates/fmine-claim/src/lib.rs:
    429–507`) with `verify_decision_cone(root_claim_id, t_dec) -> Result<TemporalConeCertificate, Refusal>`.
  - `fmine-verify` (T layer): verifies `TemporalConeCertificate` offline on every hindcast (`BD-161`, `BD-209`) and
    retrospective `DecisionCard` (`BD-226`, `BD-230`).
  - `fmine-lab` (S1/T layer, budget 10k LOC): implements the `assert_no_future_leakage_by_truncation` metamorphic test
    harness (`PO-TIME-CONE`).
- **Users & Workflows:**
  - **Auditors, QPs, Sponsor Reviewers & CI Gates (`GATE-PLUMB-E2`, `GATE-HELM-01`, `GATE-CAP-04`, `GATE-TOUCH-05`):**
    Guarantees mathematically and physically that no retrospective `DecisionCard` or hindcast in FrankenMining can ever
    cheat with future knowledge.

### 15.3 Technical & Mathematical Design (`DESIGN`)
1. **Transitive Temporal-Cone Invariant (`G-TIME-3`, `FMINE-REF-110: TransitiveHindsightLeak`):**
   Every node `u` in the extended provenance closure `Ancestors*(root)`—including L0 observations, L1
   `AttestedInterpretation`s, L2 `FieldSpec` hyperparameter particles, L3 `CorrelationCard` fitting datasets,
   `ExclusionActId`s, `PriceDeck`s, and `ReferenceClassPrior`s—carries a mandatory `QuadTemporalStamp`.
   `verify_decision_cone(root, t_dec)` traverses `Ancestors*(root)` in topological order and verifies:
   ```text
   ∀ u ∈ Ancestors*(root):
       u.physical_valid_time.from ≤ u.collection_time
                                  ≤ u.availability_time
                                  ≤ t_dec
   AND u.record_time.observed_at  ≤ t_dec_record_cutoff
   ```
   If any ancestor `u` (for example, a variogram fit or a recovery card) has `u.availability_time > t_dec`,
   `verify_decision_cone` refuses with `FMINE-REF-110 (TransitiveHindsightLeak)` and returns the **Exact Minimal
   Leakage Path** `root → ... → u` showing which intermediate model or card smuggled future data!
2. **Physical Future-Blinding Metamorphic Oracle (`PO-TIME-CONE` in `fmine-lab`):**
   To prove that an engine implementation does not bypass `ClaimDag` provenance (e.g. by iterating over an in-memory
   table containing post-`t_dec` rows without recording them as parents in `ClaimDag`), `fmine-lab` runs every
   hindcast closure `F(ledger, cas, t_dec)` twice:
   - **Run 1 (Logical `as_of_decision`):** Execute `out_1 = F(ledger_full, cas_full, t_dec)` on the full multi-year
     database.
   - **Run 2 (Physical Future-Zeroed Clone):** Construct `ledger_trunc` and `cas_trunc` by physically stripping every
     ledger batch and zeroing every CAS object whose `availability_time > t_dec` *before* invoking `out_2 =
     F(ledger_trunc, cas_trunc, t_dec)`.
   - **Assertion (`PO-TIME-CONE`):** `digest(out_1) == digest(out_2)` bit-for-bit! Any difference proves the engine
     read post-`t_dec` bytes that were not filtered by `as_of_decision(t_dec)`!

### 15.4 Worked Galaxy-Brain Card (`GB-58`, Illustrative Values)
- **Scenario:** A team runs a `2024Q1` rolling-origin hindcast (`t_dec = 2024-03-31`) of mill recovery on a refractory
  gold circuit (`BD-161`).
  - All shift feed assays are properly filtered to `availability_time ≤ 2024-03-31` (`G-TIME-2` passes!).
  - However, the recovery `CorrelationCard` (`card_pox_v3`) used in the hindcast was calibrated on `2023–2025` plant
    data (`card_pox_v3.fitting_cut_max_availability = 2025-06-30`).
  - With `card_pox_v3`, the `2024` hindcast shows a stellar `81%` interval coverage (`CRPS = 0.82%`).
- **`CounterfactualHindsightGuard` output:**
  - `verify_decision_cone` immediately refuses the hindcast with `FMINE-REF-110 (TransitiveHindsightLeak)`:
    ```text
    Leak Path: Forecast[fc_2024Q2] → Claim[clm_rec_pred] → CorrelationCard[card_pox_v3]
               → AssayResult[asy_2025_04_12] (availability_time = 2025-04-19 > t_dec = 2024-03-31)
    ```
  - Re-running with `card_pox_v1` (fitted strictly on `≤ 2024-03-31` data, verified by `PO-TIME-CONE` physical
    byte-zeroing) reveals true out-of-sample coverage was `54%`—catching an over-fitted card before it reaches a board
    `DecisionCard`!

### 15.5 First Shippable Slice, Testing Strategy & Kill Criterion
- **First shippable slice (M0–M1 / WP-02 `BD-053` & `BD-226`):**
  - Add `verify_decision_cone` to `crates/fmine-claim/src/lib.rs` (~180 LOC, deliverable immediately in M0!) and wire
    it into `ForecastLedger::register` (`crates/fmine-calib/src/lib.rs`) and `DecisionCard` rendering (`crates/
    fmine-decide/src/lib.rs`), followed by the `PO-TIME-CONE` physical truncation harness in `fmine-lab` (`BD-053`).
- **Testing strategy (§33.1 Four Claims):**
  - **Known answers:** Seeded 5-hop lookahead leak (where `L0` assays are `< t_dec` but an `L3` card or `L2` variogram
    has one parent assay with `availability_time = t_dec + 1 ns`) is caught 100% of the time with the exact 5-hop path.
  - **Parity / Determinism:** `PO-TIME-CONE` Run 1 (logical filter) and Run 2 (physically truncated store) produce
    bit-identical `FMT-CERT-1` digests across both reference ISAs (`x86_64-linux` and `aarch64-darwin`).
- **Kill criterion (`KILL-PLUMB-CONE-1`):**
  - None for the DAG check (`O(|V| + |E|)` takes `< 1 ms` for `10⁵` claim nodes). For `PO-TIME-CONE` physical store
    cloning on `> 100 GB` CAS stores, use a read-interceptor wrapper around `ObjectStore` that refuses reads of any
    object registered after `t_dec` in `O(1)` without copying bytes on disk.
