# B3 — Planning & Optimization Under Uncertainty (PQ-3) and Executive Capital Allocation (PQ-7)

RUN_ID `fm-2026-10-28-a` · Brenner pass B3 (independent) · 2026-10-28

**Inputs read:**
- `planning/mor/MOR_CASE.md` (E001–E033);
- R7 §0, §1.10, §4–§7, §9–§11;
- R3 §0, §2.2, §4.4, §4.9, §5, §6;
- R6 §0, §1.2–§1.5, §2.4–§2.5, §3, §4, §6, §7, §9;
- R4 §5.13–§5.14, §7.1; R2 §5.4;
- alien-artifact-coding `SKILL.md` and refs 01, 10, 15, 16, 22, 26, 27, 31, 33, GALAXY-BRAIN.

**Labels:**
- **FACT**: from R1–R7 or MOR (with their tags), or checked in a sibling repo this session. **FACT[U]** is literature recall that was not re-verified this session.
- **DESIGN**: a proposal.
- **HYPOTHESIS**: a testable belief.
- **TARGET**: a numeric goal or gate.
- **OPEN**: unresolved.
- All galaxy-brain numbers are **ILLUSTRATIVE substitutions**. They are not assessments of any real asset or company.

---

## 0. Bottom line

1. **PQ-3 verdict: adopt (d), the hierarchical certified decomposition, as the architecture.** It absorbs the others:
   - (a), the deterministic chain, is retained as the parity/baseline mode and the geometry generator. It is rejected as a decision basis.
   - (b), COSMO-style stochastic optimization, is admitted only through an out-of-sample VSS gate on calibrated ensembles.
   - (c), information as a decision, is adopted narrowly, for discrete commitments.
   - (e), a new forced alternative of policy search over a frozen skeleton, becomes the tactical cut-off/stockpile layer.
   - Novelty claim (checked null): every algorithm is standard. The contribution is a **three-gap ledger** on every plan: optimization gap, sampling gap, calibration gap. It makes "certified optimal" impossible to confuse with "true". This is loss #1 in the MOR contract.
2. **The solver is buildable in pure Rust because mining LPs are mostly closure problems** (DESIGN, HYPOTHESIS):
   - Bienstock–Zuckerberg (BZ) needs only (i) a max-closure oracle (our pseudoflow) and (ii) a *small* master LP.
   - fsci-opt's in-universe simplex/IPM already covers (ii) (FACT, verified below).
   - We do **not** need a commercial-grade general MILP. Build a small branch-and-bound core for subproblems of at most about 10⁴ variables, and keep HiGHS/CBC/Gurobi/MineFlow as lab oracles only.
3. **PQ-7 verdict: ONE decision object.** It is a per-share, two-stage stochastic `CapitalProgram` with recourse, over a shared scenario index (price × geology × country × execution).
   - Mean-CVaR is its risk layer, and real options are its recourse.
   - NPV/IRR is a reporting column only. Gittins/regret *guarantees* are rejected because their assumptions are violated.
   - Buybacks are a **scenario-dependent** benchmark candidate, not a risk-free hurdle.
   - Pay metrics enter as constraints with **published shadow prices** (the "KPI price list"), never as hidden objective weights.
4. **Surprises:**
   - A plain lognormal **cannot** reproduce Bertisen & Davis's three published capex statistics together; a core-plus-blowout mixture can (card G7).
   - At illustrative inputs, a junior's market price implies **about 93% inferred→indicated conversion** (card G6).
   - Unit-cost KPIs (AISC/oz) are ratios. Optimizing them directly is anti-correlated with NPV in high-grading and deferral cases.
   - fsci-sparse max-flow *refuses* capacities outside int32, so block values in cents above $21.4M are out of range. It is an oracle, not a kernel.

---

## 1. Sibling-repo verification log (read-only, this session)

| ID | Claim checked | Finding | Path | Label |
|---|---|---|---|---|
| V1 | fsci-opt MILP scale | `MilpProblem` uses **dense rows** (`a_ub: &[Vec<f64>]`). `MilpOptions{max_nodes, lp_maxiter}` defaults to **2,048 nodes**. Branch-and-bound over `linprog`, no cuts or presolve (R3). | `frankenscipy/crates/fsci-opt/src/lib.rs:1260–1430` | FACT; confirms E017 |
| V2 | fsci-opt LP engine | `LinprogMethod::{Highs, HighsDs, HighsIpm}` are **names only**. fsci runs its own bounded (primal) revised simplex and a Mehrotra IPM with crossover. No HiGHS dependency. It suits small dense master LPs (BZ master, blend LPs). | `fsci-opt/src/linprog.rs:83–120` | FACT |
| V3 | fsci-sparse max-flow | Capacities are SciPy int32. Values outside int32 are **refused** at input (not wrapped), and wrap-around only near 2³¹ in arithmetic. E017's "wraps" is imprecise. | `fsci-sparse/src/linalg.rs:41341–41369` | FACT (correction to E017 wording) |
| V4 | Certified first-order LP precedent | `fs-truss` PDHG has `verified_dual_lower_bound()` / `verified_primal_upper_bound()`, built from an independently verified dual witness and a repaired feasible primal with outward interval bounds (`fs-ivl`). Limits: `MAX_PDHG_ITERS = 1_000_000`, default `gap_tol 1e-6`. It is a *layout-LP specialization* that depends on `fs-exec`, `fs-solid` and fnx path deps. | `frankensim/crates/fs-truss/src/lp.rs:673–820`, `certificate.rs` | FACT. Lift the **pattern**, not the crate |
| V5 | CVaR substrate | `fs-robust::empirical_cvar(samples, alpha)`, `ColoredObjective`, `weakest_color`; it refuses to optimize an un-colored objective. Its only dependency is `fs-evidence`, so it is liftable (E003). | `fs-robust/src/lib.rs:80–500` | FACT |
| V6 | DRO substrate | `fs-dfo::dro::wasserstein_worst_case(losses, costs, n, rho)` uses the exact Mohajerin Esfahani–Kuhn dual on discrete support. fs-dfo also has CMA-ES/BIPOP, DE, DIRECT and MOO, but depends on `fs-exec`, so it is **not** in E003's liftable list. | `fs-dfo/src/dro.rs:1–180`, `Cargo.toml` | FACT |
| V7 | VOI substrate | `fs-voi::{ranking_flip_probability, top_two_evpi_surrogate, expected_opportunity_loss, recommend_unknown_resolutions}` assume **Gaussian objectives**. Dependencies are `fs-evidence` and `fs-math`. | `fs-voi/src/lib.rs:150–663` | FACT |
| V8 | Max-plus | `fs-tropical` has no dependencies: CPM critical path, Karp max-cycle-mean, `bottleneck`, `tune_next`. `fs-schedule-e2e` adds max-plus makespan plus VOI. | `fs-tropical/src/lib.rs` | FACT |
| V9 | Decision substrate | `franken_decision`: `LossMatrix`, `Posterior`, `FallbackPolicy`, `DecisionContract` trait, `DecisionAuditEntry`, `evaluate`. | `asupersync/franken_decision/src/lib.rs:247–916` | FACT (E012) |
| V10 | Graph flows | `fnx-algorithms`: `network_simplex_int`, `min_cost_flow`, `preflow_push`, on string-keyed graphs (R3). Dispatch-scale and oracle use only. | `franken_networkx/crates/fnx-algorithms/src/` | FACT |
| V11 | SDP/SOS | `fs-sos` exists (interval-Cholesky certificates). **Checked null:** no natural fit in PQ-3 or PQ-7. | `fs-sos/Cargo.toml` | FACT |

Nothing in the suite implements pseudoflow, BZ, TopoSort, PDLP for general sparse LP, a sparse MILP, SAA, or NPV/IRR (E018 confirmed by these reads).

---

## 2. Framing: what exactly is optimized

**Objective hierarchy (DESIGN):**
- **Mine level:** E[discounted NSR cash flow] over the realization ensemble, subject to physical, geotechnical and permit constraints, with CVaR or target-attainment constraints.
- **Enterprise level:** E[ΔNAV per share] subject to liquidity, CVaR and KPI constraints.
- "Maximize profit" is interpreted as *discounted free cash flow per share*, not accounting profit. Accounting profit is downstream of DD&A, IFRIC 20 and impairment policy (E021), and FA renders it (E019).

**Mapping to the MOR loss function:**
- **Loss #1, false certainty:** the three-gap ledger and calibration gating (§3.4).
- **Loss #2, scale to 10⁸–10⁹ blocks:** pseudoflow plus BZ on implicit arcs, with regularized aggregation beyond 10⁸ (§4).
- **Loss #3, value management cannot see:** the KPI price list and DecisionCards in their scorecard language (§7.7–§7.8).
- **Loss #4, duplicating substrate:** consume fsci, fs-robust, fs-voi, fs-tropical and franken_decision; build only what is absent (§1).

**The three gaps (DESIGN).** Every plan or decision carries all three, never only the first:

```text
reported_value − true_value = (z_model − z*_model)   [optimization gap: certified by dual bound]
                            + (z*_model − z*_SAA_pop)  [sampling gap: SAA statistical CI]
                            + (z*_pop − z_truth)        [calibration/model gap: synthetic-truth + reconciliation]
```

The third term dominates in mining. Evidence: the LUC slope ≈ 0.1 (E029); 21% interpretation accuracy (Bond 2007, R7 §0.6); and the fact that simulations are rarely validated (R7 §11.5). A 0.1% optimization gap on a miscalibrated model is a false-certainty number. It must therefore be rendered `Color::Estimated` with an "uncalibrated" headline, using the fs-evidence no-laundering rule (E008).

---

## 3. PQ-3: hypotheses (refuter-first)

### 3.1 H3a — The deterministic chain (industry standard)

**Mechanism:**
1. A kriged (smoothed) block model gives block economic values at one price deck.
2. LG/pseudoflow solves the ultimate pit (UPIT).
3. Parametric revenue-factor shells generate nested pits, from which pushbacks are chosen.
4. MILP or heuristic scheduling (CPIT/PCPSP) runs at a fixed cut-off.
5. A DCF with one-at-a-time ±% sensitivities values the result (R7 §5.1–§5.3, §9.1).

**Strengths (FACT):**
- The UPIT step is exact and fast: MineFlow handles 16M blocks in 9 s (R7 §5.1).
- The MineLib regression set exists.
- It matches single-deck reserve disclosure (S-K 1300 requires cut-off and price; R7 §4.2).
- QPs and boards understand it.

**Predicts:**
- Forecast NPV, tonnes and grade are biased high when scored against calibrated simulations. Pit value is convex in block values, so Jensen's inequality applies; see also Dimitrakopoulos 2002 (R7 §5.4, [V-sub]).
- Plans are fragile to execution, with compliance as low as 30% (E026).

**Falsifier.** On a synthetic-truth mine with calibrated ensembles, both hold:
- VSS_oos has a CI that includes 0; and
- (a)'s own P10–P90 (from its sensitivities) covers truth at nominal rates.

**Discriminating tests:**
- T3-2 (VSS) and T3-3 (coverage), §3.6.
- MineLib parity (T3-1) does **not** discriminate. It tests only solver quality.

**Refuter-first (strongest case for a):**
- Published stochastic gains are mostly evaluated in-sample, on the same simulations.
- Simulations are unvalidated.
- With 30% compliance, plan precision beyond the execution noise floor is wasted.
- Deterministic MILP is certifiable and auditable.

This is *partially accepted*: (a) remains mandatory as the baseline, because VSS is undefined without it.

**Verdict.**
- REJECT (a) as the decision basis.
- RETAIN it as:
  - the parity/baseline mode in every `PlanCertificate`;
  - the shell and pushback skeleton generator;
  - the single-deck reserve rendering that the codes require (E031).
- Confidence: HYPOTHESIS, high that the bias exists and medium on its magnitude.

### 3.2 H3b — Simultaneous stochastic optimization of the mining complex (COSMO-style)

**Mechanism:**
- Optimize over S geological realizations (and price paths) jointly.
- The first stage, the extraction sequence, is non-anticipative. The second stage, destinations, stockpiles and processing, is recourse per scenario.
- Deviations from targets are penalized, and the whole mines→stockpiles→plants→products complex is modelled together (Ramazan & Dimitrakopoulos 2013; Goodfellow & Dimitrakopoulos 2016/2017; R7 §5.4).

**Predicts:**
- VSS > 0.
- Higher target attainment.
- Risk managed by sequencing: certain ore is mined early and uncertain ore deferred.

**Falsifiers:**
1. On calibrated synthetic truth, the CI of VSS_oos includes 0.
2. VSS collapses under *realistic* miscalibration (under-dispersion of 30–50%, or a biased mean). That is the very failure R7 §11.5 calls typical.
3. Large instances can only be solved by metaheuristics with no bound (R7 §5.4), so optimization error and model value cannot be separated.

**Discriminating test.** T3-2, three-arm (§3.6).

**Refuter-first:**
- SAA optimal values are biased in the optimizer's favour: E[max of the SAA] ≥ the true max (FACT[U], Mak–Morton–Wood 1999).
- Realizations encode KU-a but rarely KU-s structural uncertainty (domaining, conceptual model), which dominates (E032; Bond 21%).
- Cost scales roughly S× and could exceed a closed-universe solver.

**Verdict: CONDITIONAL ADOPT.**
- It enters only through the VSS gate (out-of-sample, calibrated, certified).
- The default strategic stochastic engine is SAA over the BZ LP relaxation, with non-anticipativity via Lagrangian or progressive hedging, followed by rounding.
- Unbounded metaheuristic COSMO solves are allowed only when an SAA statistical bound is attached.

### 3.3 H3c — Robust, real-options and adaptive planning, with information acquisition as a decision

**Mechanism:**
- Actions include infill drilling, grade-control spacing, metallurgical testwork, and "defer".
- They are valued by EVSI = E_data[max_a E(U|a, data)] − max_a E(U|a) (Howard; R7 §1.10).
- Policies are re-optimized as information arrives (Kumar & Dimitrakopoulos updating; R7 §5.4).
- Robust counterparts handle contested distributions.

**Predicts.** For irreversible commitments (pushback start, plant expansion, cave footprint, TSF raise):
- targeted EVSI exceeds cost;
- drill-before-commit reorders decisions;
- robust plans trade a small mean loss for a large tail cut.

**Falsifiers:**
- On calibrated synthetic truth, EVSI is below drill cost for the top three commitments.
- Or the fs-voi surrogate EVPI (Gaussian-objective, V7) fails to predict realized decision flips in hindcast.

**Discriminating test.** T3-4 (§3.6).

**Refuter-first:**
- Published POMDP successes are 2D, synthetic and noiseless.
- In the multi-body case, 32% of cases were wrongly abandoned (Mern & Caers 2023, R7 §1.10).
- Reward design is subjective, and a whole-mine POMDP is intractable.
- VOI needs the calibrated model whose absence is the problem.

**Verdict: ADOPT NARROWLY.**
- Attach an `InformationOption` to each discrete commitment on its DecisionCard. Use the fs-voi/fs-oed-e2e patterns (E006) and the E033 `InvestigationCase`.
- Whole-mine POMDP is REJECTED for v1.

### 3.4 H3d — Hierarchical decomposition with certified gaps at every level (forced alternative)

**Mechanism (DESIGN):**
- **Strategic, LOM, annual or on trigger:**
  - pseudoflow UPIT with parametric shells;
  - the BZ LP relaxation of PCPSP, whose closure subproblems use the same pseudoflow kernel and whose small master LP uses fsci-opt;
  - Lagrangian prices on capacity and blend constraints, giving (i) a dual bound and (ii) $/t prices passed down;
  - TopoSort plus local search for the incumbent.
- **Tactical, quarterly/monthly:** SAA over the realization ensemble inside the strategic envelope, plus the policy layer (H3e).
- **Short-term, weekly/shift:** receding-horizon MPC (blend, dig sequence, dispatch targets), fed by grade-control data. It tracks tactical targets inside hysteresis bands.
- **Every plan** carries a `PlanCertificate`: dual bound, optimization gap, SAA CI, VSS_oos, regret table, calibration status, and color.

**Predicts:**
- Gaps of a few percent on MineLib PCPSP (FACT[U], Chicoisne et al. 2012 report small gaps for single-capacity instances).
- Dual prices readable as $/t of mill capacity and $/t of blend relaxation.
- Coherent cross-horizon targets.

**Falsifiers:**
1. BZ or Lagrangian gaps above 5% on side-constrained instances, because blend ratios and multiple processors break the closure structure.
2. Decomposition loss above 3% against a monolithic lab-oracle solve on mid-size instances.
3. Lagrangian prices oscillate enough to destabilize MPC.

**Discriminating tests.** T3-1 and T3-5.

**Refuter-first:**
- "Certificate theatre": bounds certify the *model*, not the mine.
- Hierarchical planning plus LP bounds is textbook, so this is not novel.
- BZ is strongest with few side constraints (Chicoisne 2012: one capacity per period, R7 §5.3).

All three points are accepted. The response is the three-gap ledger, plus a Lagrangian fallback whose bounds are always valid but looser.

**Verdict: ADOPT as the architecture.** The novelty claim is limited to the certificate discipline (checked null on algorithms).

### 3.5 H3e — Policy search over a frozen geometric skeleton (second forced alternative)

**Mechanism:**
- Freeze the geometry (shells, pushbacks, stope envelopes) from the H3d strategic layer.
- Optimize low-dimensional *decision rules* by simulation over the ensemble, using CMA-ES or BO (fs-dfo, fs-bo; E006). The rules cover:
  - cut-off by period and state;
  - stockpile reclaim thresholds;
  - blend targets;
  - pushback-start triggers.
- The value is out-of-sample policy NPV with a CI.

**Predicts.**
- It captures most adaptive value (Lane's declining cut-off, stockpile options, recourse) at a fraction of COSMO cost.
- The rules are legible to operators.

**Falsifier:**
- The policy's value falls short of full-(b) VSS_oos by more than the CI;
- or the out-of-sample value is much lower than in-sample (overfitting).

**Verdict: ADOPT** as the tactical cut-off, stockpile and blend policy layer. Lift CMA-ES under the frankensim consumption posture, because fs-dfo depends on fs-exec (V6, E002/E003).

### 3.6 Cheap discriminating tests

| ID | Test | Discriminates | Cost | Kill/keep rule (TARGET) |
|---|---|---|---|---|
| T3-1 | MineLib UPIT/CPIT/PCPSP parity: objectives and bounds vs best-known and lab oracles | Solver quality; H3d viability | Low | UPIT exact; PCPSP gap ≤3% on ≥80% of instances, ≤5% on all |
| T3-2 | **Three-arm VSS on a synthetic-truth mine.** Arms: calibrated, under-dispersed (σ×0.6), biased-mean (+8% grade). Optimize on ensemble A (S = 20–50); evaluate on a disjoint ensemble B (S = 200) **and** on K = 30 independent truths; paired common random numbers (CRN) | (a) vs (b) vs (e); calibration dependence | Medium (needs `fm-gen`) | (b) admitted only if the CI of VSS_oos excludes 0 in the calibrated arm **and** the calibration monitor fires in the other arms before any VSS claim is published |
| T3-3 | Coverage of each plan's P10–P90 for NPV, metal and tonnes against K truths | (a) vs the rest | Low once T3-2 exists | Nominal 80% ± binomial CI |
| T3-4 | EVSI flip calibration: on 3 commitments × 3 drill programmes, compare predicted with realized decision flips on the truth | (c) | Medium | Brier score better than the no-information baseline |
| T3-5 | Decomposition loss: hierarchical vs monolithic lab-oracle solve on 10⁴–10⁵-block synthetic instances | (d) | Low–medium | Loss ≤3% |
| T3-6 | Execution-noise robustness: apply the compliance kernel K (§5.4) to each plan; measure realized NPV loss | All. Penalizes brittle plans (E026) | Low | Report rank changes; a plan that wins only without K is flagged |

### 3.7 Horizon coupling and timescale separation (DESIGN; control family 33)

- **Downward:** each level passes *targets* (tonnes, metal, blend bands, face sequence) and *prices* (duals for mill hours, blend tolerances and stockpile capacity) to the level below.
- **Upward:** each level reports *attainment* (compliance, reconciliation residuals, F-factors as hierarchical bias posteriors per E029).
- **Separation:**
  - strategic ≥ 12× tactical, and tactical ≥ 4× short-term, in cadence;
  - inner MPC loops run per shift, with a prediction horizon of 1–2 weeks.
- **Hysteresis:** the tactical plan is re-solved only when one of these fires:
  - (i) an e-process (`fs-eproc`) on compliance or reconciliation drift crosses its threshold;
  - (ii) the price leaves its band;
  - (iii) a geotechnical or plant event occurs.
- **Plan churn is itself a cause of low compliance** (HYPOTHESIS). Re-planning on noise must be measurably avoided: churn is tracked as a KPI.

---

## 4. Solver: build vs boundary, under "closed dependency universe; foreign tools are lab oracles only" (E015)

### 4.1 Key structural fact (HYPOTHESIS, high confidence)

Strategic mine scheduling is overwhelmingly **max-closure plus a few coupling rows**. Three methods exploit this:
- **BZ** alternates between (i) a max-closure subproblem and (ii) a *small* master LP over aggregated partitions (R7 §5.3).
- **Lagrangian relaxation** of the coupling rows also yields closure subproblems.

So the hard kernel is one pseudoflow implementation, which also serves UPIT and nested shells. The master LP is small enough for fsci-opt's in-universe simplex or IPM (V2). A general commercial-grade MILP is *not* on the critical path.

### 4.2 Build in pure Rust (DESIGN; all `#![forbid(unsafe_code)]`, `Cx`-threaded, budgets and cancellation per E012)

| Crate | What | Algorithm notes | Scale TARGET | Oracle (lab only) |
|---|---|---|---|---|
| `fm-pitflow` | UPIT, parametric nested shells, closure oracle | Hochbaum HPF pseudoflow on i64 CSR. Overflow guard: Σ\|v_i\| < 2⁶² or refuse. **Implicit precedence arcs** from slope templates (azimuth sectors × geotech domain) with Caccetta–Giannini minimum search patterns. Parametric λ (revenue factor) warm-started across λ, using the monotone nesting of parametric min-cut. Returns the cut **and** the flow, so the certificate is checkable | MineLib UPIT exact. 10⁷ blocks / ~2×10⁸ implicit arcs in ≤120 s, ≤32 GB. 10⁸ via regularized aggregation (OPEN) | MineFlow (MIT C++), fsci-sparse `maximum_flow` (small, scaled int32, V3), fnx `preflow_push` |
| `fm-bz` | PCPSP / multi-period LP relaxations | BZ partitions; master solved by fsci-opt `linprog`; subproblem by `fm-pitflow` | MineLib PCPSP LP within 1e-6 relative of the oracle. 10⁶ blocks × 20 periods ≤30 min | HiGHS |
| `fm-lag` | Lagrangian/bundle relaxation of side constraints (blend, multi-processor) | Subgradient or proximal bundle; **every iterate gives a valid dual bound** | Always-valid bound; target gap ≤5% | HiGHS |
| `fm-topo` | Incumbents from LP solutions | TopoSort (Chicoisne 2012) plus local search and repair; feasible by construction; verified in `fm-verify` | Gap ≤3% (T3-1) | Best-known MineLib values |
| `fm-pdlp` | Large *non-closure* sparse LPs: multi-site blend networks, SAA extensive forms, CVaR LPs | Restarted PDHG with diagonal preconditioning and adaptive restarts. **Certified bound** by dual repair plus outward interval rounding, following the fs-truss pattern (V4) | 10⁷ nnz to 1e-4 relative gap in ≤30 min, bound attached | HiGHS IPM, OR-Tools PDLP |
| `fm-milp` | **Small** MILP: blend with on/off, fleet assignment, pushback selection, capital programme | Sparse bounded dual simplex with warm start, bound-tightening presolve, best-bound plus diving; cover/MIR cuts only if a gate demands them; MPS I/O. **Explicit non-goal: general commercial MILP** | ≤10⁴ variables, ≤500 binaries, ≤1% gap in ≤10 min | HiGHS, CBC; fsci-opt `milp` (tiny, V1) |
| `fm-stoch` | SAA, scenario reduction, Mak–Morton–Wood statistical bounds, VSS_oos, progressive hedging | CRN across candidates; disjoint optimization and evaluation sets enforced by type | S ≤ 200 in-optimization; ≥10³ evaluation | Brute-force enumeration on toys |
| `fm-mpc` | Shift/week receding-horizon blend and dig targets | LP or QP over 1–2 weeks; feasibility fallback = last feasible plan plus slack | ≤5 s per solve | HiGHS |
| `fm-haul` | Haulage DES and fleet sizing | Deterministic virtual time (asupersync `LabRuntime`, E012); `fs-tropical` max-cycle-mean as cycle bound (V8); Kingman / M/G/c match-factor sanity checks (family 31) | 1 shift of 100 trucks in ≤10 s | OpenMines-style benchmarks (R7 §5.7) |

### 4.3 Lab oracles only (DESIGN)

- **Home:** they live only in `fm-oracles`.
- **Pinning and fixtures:** each oracle is version-pinned. Outputs are frozen as fixtures, **never regenerated from the candidate** (E022).
- **Not in the build graph:** none of them appears in the runtime or CI dependency graph. Fixtures are produced by a separate lab runner.
- **The oracles:**
  - HiGHS (MIT) and CBC (EPL) for LP and MILP;
  - MineFlow (MIT) for UPIT;
  - OR-Tools PDLP (Apache) for first-order LP;
  - Gurobi/CPLEX only where the customer's lab licenses them.
- **In-universe small oracles:** fsci-opt `linprog`/`milp`, fsci-sparse `maximum_flow`, and fnx flows (V1–V3, V10).

**Boundary escape hatch: `UntrustedCandidate` import (DESIGN; OPEN for owner sign-off).**
1. FM exports MPS plus a certificate request.
2. Any external solution can be imported as an `UntrustedCandidate`.
3. `fm-verify` (std-only) independently checks feasibility and recomputes the objective.
4. The candidate may become the **incumbent**. Its claimed bound is ignored unless a dual witness verifies.

This permits a customer's own accelerator without a dependency. It also keeps the "one authority per number" rule: our verifier is the authority.

### 4.4 Kill gates and fallbacks (TARGET)

| Gate | Threshold | By | Fallback if killed (deterministic, conservative) |
|---|---|---|---|
| K-PIT | `fm-pitflow` within 5× MineFlow wall-time on the largest MineLib UPIT, and 10⁷ blocks in ≤10 min | Month 4 | Regularize or aggregate blocks (bench/panel super-blocks), with the pit re-verified at fine scale inside the shell. **Never** a runtime foreign dependency |
| K-BZ | PCPSP LP within 1% of the oracle in ≤1 h on side-constrained instances | Month 6 | `fm-lag` bound plus rolling-horizon aggregation (phase-bench units). Publish the gap honestly |
| K-MILP | Blend, fleet and capital subproblems to ≤1% in SLA | Month 8 | LP plus rounding with a reported gap; MPS export to the lab for offline benchmarking |
| K-PDLP | 1e-4 relative gap on real-size non-closure LPs | Month 9 | BZ/Lagrangian where structure allows; otherwise scenario reduction to sizes fsci can handle |
| K-STO | T3-2 calibrated-arm VSS_oos CI excludes 0 for realistic configurations | Month 10 | Drop COSMO-scale (b). Keep (a) plus a robust overlay plus (e) policy search. Use stochastic only for stockpile and cut-off policy |
| K-CAL | Ensemble PIT coverage demonstrated on synthetic truth (and later on reconciliation, E029) | Continuous | Stochastic outputs stay `Estimated` and "uncalibrated". Capital decisions use reference-class widening (§7.3) |

---

## 5. "Optimal ore mix at best unit cost"

### 5.1 Blending and destination LP (DESIGN)

Indices: source/parcel s (block, bench-phase or stockpile bin), processor p (CIL, flotation, heap leach, POX, waste, stockpile bin k), period t.

```text
max Σ_t δ_t [ Σ_{s,p} x_spt (NSR_sp − c^proc_p − c^haul_sp) − Σ_s c^mine_s m_st − f_t ]
s.t.  Σ_s x_spt ≤ Cap_p,t                         (tonnes)
      Σ_s x_spt · E_s ≤ P_p · H_p,t               (kWh: specific energy E_s = Bond/SMC-derived)
      Σ_s x_spt (a_s,e − A^max_p,e) ≤ 0            (deleterious e: As, Hg, CN-soluble Cu, organic C, F)
      Σ_p x_spt = m_st ;  precedence/closure on m  (from fm-pitflow / fm-bz envelope)
      I_k,t+1 = I_k,t + inflow_k,t − reclaim_k,t ;  0 ≤ I ≤ Î_k   (grade-binned stockpiles)
NSR_sp = Σ_metal g_s,m · R_p,m(domain_s) · payable_p,m · (price_m − TC/RC/refining_m) − penalties_p(s)
```

**Design choices:**
1. **Recovery is per source/geomet domain, not per blend.** This keeps the recovered metal linear. Blend-dependent recovery is approximated by piecewise-linear domain-mix classes.
2. **Hardness enters as specific energy (kWh/t) under a mass-weighted additivity assumption.** R7 §7.1 warns that hardness indices are non-additive, so this is **HYPOTHESIS H-ADD**: "kWh/t additivity within ±8% over the operating blend range". It is validated on plant data. If falsified, use piecewise blend classes.
3. **Stockpiles use grade bins.** Exact mixing is bilinear and non-convex (Bley et al., R7 §5.3). Fixed-grade bins make the model linear.
   - Certificate: the McCormick relaxation of the bilinear model gives an upper bound.
   - TARGET: binned-LP vs McCormick gap ≤1%.
4. **Throughput–recovery trade-off.**
   - Grind modes m (fine, medium, coarse) carry pairs (Q_m t/h, ΔR_m), and the model picks a convex combination per period.
   - The shadow price on mill hours says whether to "push tonnes". The first-order condition is
     `∂Q/∂P80 · (g·R·(p−k) − h) + Q·g·(p−k)·∂R/∂P80 = 0`.
   - Tin is strongly size-dependent and loses metal in fines (R7 §7.5), so it needs the coupled comminution model.
5. **By-product credits.**
   - The planning objective is NSR, which contains *all* payable metals (Au, Cu, Ag, Sn).
   - By-product vs co-product treatment is a **presentation layer** for AISC/C1 (§5.3). It never changes the optimization.

### 5.2 Lane's cut-off theory with stockpile options (DESIGN plus FACT)

- **The cut-off formulas** (Lane 1964/1988, R7 §5.2):
  - Mine-limited: `g_m = h / ((p−k)y)`.
  - Mill-limited: `g_h = (h + (f+F)/H) / ((p−k)y)`, with opportunity cost `F = δ·V`.
  - Market-limited: `g_k = h / ((p − k − (f+F)/K) y)`.
  - Balancing cut-offs come from the grade–tonnage curve. The operative cut-off is the median of the limiting and balancing values for each capacity pair, iterated on V. This produces a *declining* policy.
- **The two extensions:**
  - **(i) A stockpile destination.** Material between the marginal and opportunity-cost cut-offs is stockpiled when its discounted end-of-life value exceeds the rehandle cost (card G1).
  - **(ii) Grade uncertainty.** Classification uses expected loss over the local posterior (Verly 2005, R7 §6.2), not the kriged grade versus cut-off.
- **Implementation (fm-cutoff):**
  - Lane serves as the closed-form **policy seed** and the explainability card.
  - H3e policy search refines the seed over the ensemble.
  - The LP of §5.1 is the arbiter. Its shadow price on mill capacity *is* the (f+F)/H term, made consistent: a duality cross-check of Lane against the LP (PO-15).

### 5.3 Unit cost and AISC consistent with the accounting truth (DESIGN; FA is the book of record, E019–E021)

1. **One authority per number** (E022, R4 §7.1):
   - FA owns actual costs: certified trial balance and statement rows.
   - FM owns physical drivers, plans and the *metric definitions*. This is the R4 domain-pack item 14, `fm-metrics`.
   - FM never computes an authoritative balance.
2. **`CostDriverCard` per site and cost pool:**
   - Contents: driver (t mined, t·km hauled, t milled, kWh, reagent kg/t, labour hours); fixed/variable split; rate with uncertainty; FA certificate IDs of the cost-category registers it was fitted on; valid-time window.
   - Color: `Estimated`.
   - Plan cost = Σ driver quantities × rates.
   - Price-linked lines are functions of the **same scenario index** as revenue. These are royalties, profit-sharing and by-product credits. Royalties drove Q1 2026's +16% industry AISC (R6 §4.2).
3. **The same function on two inputs (PO-9).** `aisc(rules_version, inputs)` runs identically on:
   - (i) plan quantities × driver rates (forecast); and
   - (ii) FA-certified actuals (actual).
   
   The rules version pins the WGC 2013/2018 taxonomy (R6 §4.1): the sustaining vs non-sustaining test (≥10% uplift **and** no net cash within 12 months; the pushback rule) and by-product credits.
4. **Variants as named adjustment layers with reason codes:**
   - by-product vs co-product (Newmont: $1,520 vs $1,963/oz, R6);
   - comp-adjusted (Barrick: $1,423 vs $1,637/oz, R6 §1.5);
   - attributable vs consolidated (FA party ownership, R4 §5.13).
   
   A reported gap that cannot be decomposed is a **refusal**, not a footnote.
5. **The plan→actual bridge closes exactly.** Its components are volume, grade/mix, unit rate, price-linked items, by-product price, classification and FX, and they must sum to the total variance (PO-10). That variance enters the forecast/calibration ledger (E032).
6. **AISC/oz is never the objective.** It is a ratio: high-grading, deferring sustaining capex or reclassifying capex as growth can all lower it while destroying NPV.
   - It may enter as a linear constraint, `Σ cost_t − A·oz_t ≤ 0`.
   - Its **shadow price is published**: the "$ NPV per $/oz of AISC target" (§7.7).

### 5.4 Schedule compliance as a forecastable, calibrated quantity (DESIGN; E026 "as low as 30%")

**Definitions:**
- Spatial compliance: `SC_t = tonnes mined in t from blocks scheduled in t / tonnes scheduled in t`.
- Also tracked: sequence compliance, volume compliance, metal compliance, and **sequence debt** (scheduled-but-unmined tonnes carried forward, including stripping debt).

**Model (`fm-compliance`):**
- `k_t ~ BetaBinomial(n_t, α_site, β_site)`, with a hierarchy by site, pit and horizon.
- Covariates: fleet availability, plan churn, geotechnical events, blast delays, weather, active-face count.
- Prior: a reference class that is *wide*, because "as low as 30%" is a lower anecdote, not a base rate (OPEN; R6 §6.4 [V-P quote]).

**Calibration and monitoring:**
- Each period's forecast is written immutably to the calibration ledger.
- It is scored by PIT and coverage.
- An `fs-eproc` e-process watches drift.

**Use in planning:**
- **Compliance kernel K.** Realized extraction is drawn from K(plan, state): substitution to available adjacent blocks with a grade ratio ρ.
- **Scoring.** Plans are scored on `E_K[·]` (T3-6).
- **Executability features.** Active faces and precedence slack are rewarded.
- **Card G4** shows that the honest metal forecast differs from the plan number by the *execution* term alone, before any grade uncertainty.

---

## 6. PQ-7: hypotheses (refuter-first)

| | Mechanism | Predicts | Falsifier | Refuter-first (strongest case *for* it) | Verdict |
|---|---|---|---|---|---|
| **H7a** NPV/IRR + hurdle | Base-case DCF at a risk-loaded rate; rank by NPV, IRR or PI under the budget | Overrun bias uncorrected (+14%, B&D; >40%, McKinsey; E026, R6 §6.2). Portfolio concentrated in one price beta. Exploration looks NPV-negative (0.69 value per $, R6 §4.5). IRR misorders projects of different scale (FACT[U]) | Hindcast: approval-time NPVs of past approvals are unbiased vs realized, with high rank correlation. R6 contradicts this: Tanami 2 went from $395M to $1.7–1.8B; Pascua-Lama was written down $5.2B | E[NPV] *is* the right risk-neutral statistic; most bias is in the inputs, not the rule | **REJECT as the rule; RETAIN as a reporting column** and as the risk-neutral term |
| **H7b** Mean-CVaR over shared scenarios | Candidate cash-flow ensembles on the shared index. Choose x to maximize E[ΔNAV/share] subject to CVaR_α ≤ τ, budget and liquidity, using the Rockafellar–Uryasev LP. `fs-robust` for reporting (V5) | Diversification across price beta and country; an explicit "price of risk appetite" | (i) Selection unstable under scenario bootstrap. (ii) Tail shaped by unidentifiable correlations. (iii) CVaR on NAV misaligned with real harm (covenants, minimum cash) | Coherent risk measure; LP-tractable; boards already speak "downside" | **ADOPT as the risk layer**. Always report selection frequency, never a single "optimal portfolio" |
| **H7c** Real options + bandit/Gittins | Staged compound options (gates, information, abandon, expand) by backward induction or LSM. Budget across plays via Gittins/Whittle/Thompson/knowledge gradient | Early plays carry option value despite negative static NPV. Earn-ins beat acquisition when inferred ounces dominate (G6) | **Gittins optimality needs independent arms, one arm played at a time, frozen idle arms, geometric discounting and no budget coupling** (FACT[U], Gittins 1979; Whittle 1988). All are violated: shared price, regional geology, parallel programmes, tenure expiry, competitors (restless). Regret bounds O(√T) are **vacuous at T ≈ 10–30 decisions per decade** (ref 22) | Options are real; a stage gate is a real decision; ignoring option value under-explores | **ADOPT staged-option valuation; REJECT Gittins and regret *guarantees*.** Indices are triage heuristics only. Option value is shown *separately* from static NPV and the base rate, to prevent "option-value laundering" |
| **H7d** Per-share two-stage stochastic `CapitalProgram` with recourse (forced alternative) | One object. First stage: this cycle's commitments across *all* candidate types. Recourse y_ω (expand, defer, abandon, ROFR, draw stream) per scenario. Objective E[ΔNAV_ω/shares_ω] (buybacks and issuance change the share count). Constraints: budget, liquidity, CVaR, KPI soft constraints with shadow prices. Robustness check: minimax regret plus Wasserstein DRO over contested scenario-family weights (V6) | Robust first stage; KPI shadow prices expose Goodhart pressure; buybacks chosen exactly when the own P/NAV discount beats risk-adjusted project spreads | (i) Indistinguishable from b+c separately: fine, then it is packaging. (ii) Decision flips with P/NAV ±10%: report the P/NAV-conditional hurdle. (iii) Too slow at board scale: unlikely (§6.2) | n/a (forced) | **ADOPT as THE decision object.** It contains (a) as a column, (b) as the risk layer, and (c) as recourse |

**Discriminating tests:**
- **T7-1:** G5 reproduced by brute force (exact).
- **T7-2:** bootstrap scenario stability. TARGET: chosen items selected in ≥80% of 200 resamples, else flagged "fragile".
- **T7-3:** bitemporal hindcast of past approvals using "as known at t" (E015, E013) to score H7a vs H7d decisions.
- **T7-4:** synthetic exploration district from the PQ-2 prior (E028), 50 synthetic decades. Compare static NPV rank, Gittins heuristic, Thompson, knowledge gradient and staged-option policies on discoveries and NAV per $.
- **T7-5:** decision sensitivity to P/NAV, α and country-hazard multipliers.

### 6.1 Every capital alternative mapped into one `CapitalCandidate` (DESIGN)

| Type | First-stage action | Recourse | Cash-flow source | Key priors | Typical evidence class |
|---|---|---|---|---|---|
| Sustaining capex | fund / defer | catch-up later at higher cost; failure hazard | PQ-3 plan with vs without; hazard model (TSF, fleet, plant) | failure hazards, overrun | modelled-uncalibrated |
| Brownfield expansion | study → FEED → build gate | delay, rescale, abandon | PQ-3 plan delta under the same scenario index | overrun, ramp-up, conversion | modelled |
| Greenfield exploration play | fund stage k | advance, abandon, JV-out | PQ-2 posterior (E028) × size marks (Zipf/lognormal) × conversion × development path; 15.7–17.9 y lead (R6 §4.4) | discovery probability, size, lead time | elicited / transferred |
| Junior equity stake | buy % | top-up, ROFR, sell | Junior's public NI 43-101/JORC resource, *haircut* (§6.4); market price process | h_pub, conversion, P_dev, dilution | transferred |
| Earn-in / option JV | stage spend | walk away / complete | as above, then own-drill data | conversion | transferred → measured |
| M&A | bid | integrate, divest | Target assets as candidates on public data, plus synergies | synergy realization (OPEN), premium; build-vs-buy parity: greenfield $18,048/tpa + ~40% overrun ≈ M&A $25,182/tpa (R6 §4.3) | transferred |
| Stream / royalty sale | sell x% for deposit D | buy-back clauses | Own production × price paths: payout is *scenario-dependent* (a put-like hedge) | price paths | modelled |
| Share buyback | repurchase $C | — | ΔNAV/share = C·(NAV_ω/P − 1)/shares: **correlated with gold price**, not risk-free | P/NAV, own NAV ensemble | modelled |
| Dividend top-up | pay | — | Policy, e.g. Barrick's 50% of attributable FCF (R6 §1.4) | — | policy (attested) |

**Sustaining capex is not an NPV project. It is risk reduction.**
- Its payoff is the reduction in expected loss **and** in CVaR of catastrophic events, such as TSF loss of containment (GISTM; R6 §6.5).
- Deferral lowers current AISC, because AISC includes sustaining capex (R6 §4.1).
- The KPI therefore *rewards* deferral. The DecisionCard must show that hazard–AISC trade-off explicitly.

### 6.2 Scale (TARGET)

- **Candidates:** 50–500, typically ≤200 binaries after mutually exclusive stage-variant grouping.
- **Scenarios:** 2,000 for optimization, after Kantorovich scenario reduction; ≥10⁴ for evaluation.
- **Solver:**
  - `fm-milp` targets ≤0.5% gap in ≤10 min.
  - Fallback: greedy plus a Lagrangian bound with the gap reported.
- **Bottleneck:** input validity, not compute (diminishing-returns rule, SKILL §"Diminishing Returns").

### 6.3 Reference-class priors for capex overruns and schedule slips (FACT inputs; DESIGN mechanics)

**Classes:**
- Capex: B&D 63 projects, mean +14%, ~½ outside ±15%, ~1/13 ≥ +100% (R7 §9.1 [V]).
- Megaprojects: McKinsey 83% with problems and >40% overrun; EDC 37% (copper 40%).
- Schedule: 20–30% delay, 1.2 y mean in 58 projects (R6 §6.2 [V-snip]).
- Lead time: 15.7–17.9 y (R6 §4.4).
- Ramp-up: McNulty classes [U] (R7 §11.8).

**Mechanics:**
- Hierarchical: industry class → company random effect → project.
- Updated from the company's **own** approvals in its bitemporal record (E013, E015).
- Applied as **explicit, visible adjustment claims** with evidence class `transferred` (R7 §9.5.3), never buried in the discount rate.

**Fit check (card G7):** a single lognormal fails to reproduce B&D's three statistics together, so `fm-refclass` must use a core-plus-blowout mixture or B&D's shifted lognormal. That fit is a gate (G-CAP-2).

### 6.4 Resource conversion and public-resource haircuts (HYPOTHESIS priors; OPEN data)

- **Ounce paths:** inferred → indicated (`c_FI`) → reserve (`c_IR`), plus a public-report bias haircut `h_pub` (junior-reported vs the major's re-estimate) and a development probability `P_dev`.
- **Reserve-equivalent:**
  - `R = h_pub · (c_IR·M_ind + c_FI·c_IR·M_inf)`.
  - Priors: Beta per deposit style × drill density × reporting code × price deck.
- **Learning source:**
  - Statement-to-statement flows in the miner's *own* resource and reserve history ("as known at t" in fgdb, E013).
  - Juniors' filing histories (SEDAR+/ASX).
  - Post-acquisition re-estimates.
  - OPEN: ounce-level lineage is rarely published, so start with deposit-level flows.
- **Price decks differ:** Barrick reserves at $1,400 or $1,500/oz (R6 §1.3); juniors often use higher. Re-price before applying any haircut.
- **Fourmile scale reference:** Barrick's Fourmile is 2.6 Moz Indicated vs 13 Moz Inferred (R6 §1.2). This shows that inferred mass can dominate even Tier-One candidates. *(Context only; no assessment.)*

### 6.5 Country and political risk as a stochastic process, not a discount-rate fudge (DESIGN; R6 §6.6, §7.4)

- **Process:** per-country `CountryRegime` Markov chain with states {Stable, FiscalShock, SecurityDisruption, Suspension/Expropriation}.
- **Impacts** are cash-flow transforms:
  - royalty or tax step-ups, including retroactive ones (Mali 2023 Code);
  - suspension (Porgera 2020; Cobre Panamá 2023);
  - slowdown with capex escalation (Reko Diq 2026).
- **Shared state:** one regime path per country, shared by *all* assets in that country (North Mara and Bulyanhulu move together).
- **HYPOTHESIS H-FISC:** fiscal-shock hazard rises with metal price (windfall-seeking). This creates a negative price–regime correlation that caps upside, and a hurdle-rate model cannot express it.
- **Priors:** structured elicitation (IDEA protocol, R7 §9.3) plus a reference class.
- **Robustness:** Wasserstein DRO over hazard multipliers (V6), because these probabilities are *contested*.

### 6.6 Correlation through the shared scenario index (DESIGN)

```rust
pub struct ScenarioIndex {          // one draw ω of the enterprise world
    pub market: PricePathId,        // Au, Cu, Ag, Sn, FX, diesel, power: joint process + copula
    pub inflation: CostPathId,      // industry-wide cost factor (common cause for overruns)
    pub country: CountryPathId,     // one regime path per country
    pub geology: RealizationSetId,  // per-asset realization ids from PQ-1/PQ-2 ensembles
    pub execution: ExecDrawId,      // per-project overrun/slip/ramp draws (+ common factor)
}
```

- **Common random numbers** across all candidates make comparisons paired and low-variance. Use Sobol/Philox streams keyed by logical identity (`fs-rand`, R3 §2.2).
- **Portfolio CVaR** is computed **only** on joint draws, never by combining marginal distributions.
- **Price-linked costs** (royalties, profit-sharing) share the `market` draw (§5.3).

### 6.7 The Tier One screen and executive pay metrics

**Tier One screen:**
- `TierOneScreen{deck_version, min_life, min_rate, cost_percentile, district_growth}` is versioned:
  - gold at $1,400 (Circular) or $1,500/oz (Q2 2026 release), ≥10 y, ≥500 koz/y, lower-half cost;
  - copper at $3.00 or $3.25/lb, ≥5 Mt, ≥20 y, ≥200 ktpa (R6 §1.3).
- It reports **P(Tier One)** over the ensemble plus a *distance vector*, not pass/fail.
- MinEx's Tier 1 definition is a separate object. **Never mix the two.**

**Pay metrics (DESIGN): constraints with shadow prices, never hidden weights.**

| Metric (weight, R6 §1.5, §2.5) | Treatment | Why |
|---|---|---|
| AISC; Newmont CSC/GEO 35% | Soft constraint per site-year; shadow price published | Ratio metric. Gameable by high-grading, deferral or reclassification |
| ROCE (Barrick 15% LTI, 10–15% range; Newmont 30% PSU) | 3-year floor constraint; shadow price | Penalizes long-gestation growth (capital employed rises years before EBIT), a **structural bias against greenfield**. The shadow price makes it visible |
| Reserve replacement (3-yr, 15% LTI) | Computed by `fm-metrics` under **one** definition; constraint only after the definition is reconciled (190% vs 504%, R6 §1.2) | Rewards booking ounces (M&A, a higher deck), so Goodhart pressure |
| GEO (25% API) | Soft floor | Volume can conflict with NPV |
| rTSR vs GDX (35% Barrick; 60% Newmont) | **Excluded** from optimization; context only | Not controllable; market-relative |
| FCF/share; dividend policy | Liquidity constraints, e.g. a minimum-cash floor (Newmont $5B, R6 §2.4) | Real harm when breached |
| Safety, critical controls | **Hard** constraints; never priced against $ | E023 refusals; not tradeable |

**The KPI price list (board artifact; DESIGN).** It answers questions of the form:
- "Holding 2027 Au AISC ≤ $X/oz costs $Y M of E[NAV]."
- "Relaxing the ROCE floor from 12% to 10% admits greenfield G, worth +$Z/share."

This is the dual of each KPI constraint, with a validity range from post-optimality analysis.

### 6.8 How a board sees it: the `DecisionCard` (DESIGN; over `franken_decision`, V9)

```text
┌ DC-2027-CAP-014  "FY27 growth envelope $800M"   color: ESTIMATED (weakest input: country hazard, elicited) ┐
│ Alternatives (incl. do-nothing, buyback): A+E | A+D | E+D | A+E_stream | hold cash                        │
│ E[ΔNAV] $M: 372 | 241 | 253 | 376 | 0      CVaR20: −210 | −131 | −283 | −150 | 0   P(Tier One E): 0.31    │
│ Selection frequency (200 bootstraps): A 0.97, E 0.62, D 0.41, E_stream 0.58  → "E is fragile"            │
│ Regret (mean/max): A+E 91/377 | A+D 222/701 | E+D 210/550                                                  │
│ Chosen under board risk appetite CVaR20 ≥ −150: A+E_stream (binding). Without the stream: A+D, −$131M     │
│ What would change it: E's S1 loss > $150M after stream; stream PV cost > $135M; P/NAV < 0.66 → A+D       │
│ KPI prices: AISC FY27 cap $1,650/oz → −$38M; ROCE floor 12% excludes a greenfield G (−$0.21/sh)           │
│ Reference-class adjustments shown: capex ×1.14 mean (B&D mixture G7), slip +1.2y; conversion h_pub 0.85   │
│ Best next information: EVSI(E metallurgy testwork) $41M vs cost $6M → fund before FID                     │
│ Sign-offs required: QP/CP (resources), CFO (financing), Board. execution_authorized = false (E023)        │
└────────────────────────────────────────────────────────────────────────────────────────────────────────────┘
```

All numbers in the card above are illustrative (G5).

The card's fields map onto suite types:

| Card field | Source |
|---|---|
| alternatives | `LossMatrix` rows |
| regret | per-scenario best-in-hindsight, over the *same* scenario set |
| "what would change it" | break-evens plus `fs-voi` `ranking_flip_probability` |
| headline color | `fs-robust` `weakest_color` (no laundering, E008) |
| audit record | `DecisionAuditEntry` in `EvidenceLedger` (E012) |

**Outcome loop.** Later actuals (spend, slip, conversion) are scored in the calibration ledger and update `fm-refclass` priors. This is the process-vs-outcome audit of R7 §9.5.4.

---

## 7. Alien-artifact compilation (PQ-3 + PQ-7)

**Objective (one sentence).** Choose plans and capital commitments that maximize E[discounted FCF per share] under geological, price, cost, country and execution uncertainty, where every published number carries its optimization, sampling and calibration gaps.

**Failure-cost asymmetry.** A false-certainty number reaching a reserve, capital or dam decision costs far more than a conservative plan (MOR loss #1).

**Observables now:**
- MineLib instances;
- synthetic-truth mines (`fm-gen`);
- FA certified actuals;
- compliance and reconciliation history;
- public filings.

### 7.1 EV matrix (natural fits only; score = Impact × Confidence / Effort)

| # | Family (ref) | Fit here | I | C | E | Score | Decision |
|---|---|---|---|---|---|---|---|
| 1 | Calibration + anytime monitoring (02; fs-eproc) | Ensemble PIT, compliance forecasts, F-factor drift, reference-class updating; this gates loss #1 | 5 | 4 | 2 | **10.0** | ADOPT |
| 2 | Graph flows / max-closure (32) | UPIT, shells, BZ and Lagrangian subproblems | 5 | 5 | 3 | 8.3 | ADOPT |
| 3 | Convex duality / LP (26) | BZ, Lagrangian prices, certified bounds, shadow prices, KPI price list | 5 | 5 | 3 | 8.3 | ADOPT |
| 4 | Tail risk CVaR + light Wasserstein DRO (10) | Capital CVaR, contested country hazards (V5, V6) | 4 | 4 | 2 | 8.0 | ADOPT |
| 5 | Bayesian decision / VOI (TECHNIQUES; fs-voi) | EVSI information options, ranking flips, DecisionCards (V7) | 4 | 4 | 2 | 8.0 | ADOPT |
| 6 | Queueing + max-plus (31; fs-tropical) | Fleet match factor, cycle bounds, project critical path (V8) | 3 | 4 | 2 | 6.0 | ADOPT (cheap) |
| 7 | Optimal transport (24) | Kantorovich scenario reduction, DRO radius | 3 | 4 | 2 | 6.0 | ADOPT as a tool |
| 8 | Stochastic programming / SAA + Markov regimes (38) | SAA bounds, VSS, country chains, LSM options | 4 | 3 | 3 | 4.0 | ADOPT (gated by K-STO) |
| 9 | Control / MPC (33) | Short-term receding horizon, hysteresis | 3 | 3 | 3 | 3.0 | ADOPT in phase 2 |
| 10 | Submodular (27) | Batch infill selection | 2 | 2 | 2 | 2.0 | DEFER. VOI is not submodular in general (FACT[U], Krause & Guestrin); use greedy EVSI with an adaptivity check |
| 11 | Game theory (29) | Junior, JV and host-government bargaining | 3 | 2 | 4 | 1.5 | REJECT for v1 (OPEN) |
| 12 | Online learning / regret (22) | Exploration budget bandits | 2 | 1 | 3 | 0.7 | REJECT (T too small; see H7c) |

**Checked nulls (no natural fit):** sheaf, K-theory, TDA, coding theory, differential privacy, SOS/SDP (V11), spectral risk.

**Budget.** More than 100K LOC and safety-adjacent means 5–8 families are allowed (SKILL). Families 1–8 are used; 9 is phase 2.

### 7.2 Compiled artifacts: Rust type sketches (DESIGN)

```rust
pub struct PlanCertificate {                 // attached to every published plan, all horizons
    pub plan: PlanId, pub model: ModelDigest, pub opt_set: RealizationSetId,
    pub primal: Money, pub bound: Option<CertifiedBound>, pub opt_gap: Option<Ratio>,
    pub saa: Option<SaaGap>, pub vss: Option<VssCertificate>, pub execution: Option<ComplianceAdjusted>,
    pub calibration: CalibrationStatus,      // Calibrated{ledger_ref} | Uncalibrated | Miscalibrated{correction}
    pub color: Color, pub refusals: Vec<Refusal>,  // fs-evidence colors, no laundering (E008)
}
pub struct CertifiedBound { pub value: Interval<Money>, pub kind: BoundKind, pub witness: ContentHash, pub verified_by: VerifierId }
pub enum BoundKind { ClosureExact, BzLpRelaxation, Lagrangian { iters: u32 }, PdlpDualRepair, SaaStatistical { conf: f64, batches: u16 } }
pub struct VssCertificate {                  // constructor refuses eval_set == opt_set or unpaired draws
    pub det_plan: PlanId, pub sto_plan: PlanId, pub eval_set: RealizationSetId, pub n_eval: u32,
    pub vss_oos: Interval<Money>, pub vss_in_sample: Money, pub optimism: Money, pub truth_vss: Option<Interval<Money>>,
}
pub struct ShadowPrice { pub constraint: ConstraintId, pub units: UnitExpr, pub dual: f64, pub valid_range: (f64, f64) }
pub struct CostDriverCard { pub pool: CostPoolId, pub driver: DriverKind, pub rate: Dist<Money>, pub fixed_share: Ratio, pub fa_certs: Vec<FaCertificateId>, pub valid: ValidTime }
pub struct CapitalCandidate {
    pub id: CandidateId, pub kind: CandidateKind, pub stages: Vec<StageGate>,
    pub cashflows: CashflowEnsemble,         // keyed by ScenarioIndex (§6.6), CRN across candidates
    pub kpis: KpiEnsemble, pub tier_one: Option<TierOneAssessment>,
    pub adjustments: Vec<ReferenceClassAdjustment>, pub evidence: EvidenceSummary,
}
pub struct StageGate { pub cost: CashflowEnsemble, pub reveals: InformationSpec, pub recourse: Vec<RecourseAction> }
pub struct CapitalProgram { pub candidates: Vec<CapitalCandidate>, pub budget: Money, pub liquidity_floor: Money,
    pub cvar: CvarConstraint, pub kpis: Vec<KpiConstraint>, pub objective: PerShareNav }
pub struct DecisionCard {                    // emitted via franken_decision DecisionAuditEntry (V9)
    pub id: DecisionId, pub alternatives: Vec<AltStats>,  // mean, P10/50/90, CVaR, regret mean/max, selection freq
    pub flips: Vec<FlipCondition>, pub kpi_prices: Vec<ShadowPrice>, pub info_options: Vec<InformationOption>,
    pub color: Color, pub signoffs: Vec<SignoffRequirement>,  // no authorization field exists (E023)
}
```

### 7.3 Guard table (deterministic; evaluated in order; first match wins)

| # | Condition | Action |
|---|---|---|
| 1 | Any E023 flag would be set, or a safety constraint would be priced | Refuse and route to a human. No card is emitted |
| 2 | Bound not verified by `fm-verify` | No gap shown; label "heuristic, unbounded" |
| 3 | opt_gap > 5% | Label "heuristic plan"; the word "optimal" is banned |
| 4 | VSS with eval_set = opt_set, unpaired draws, or n_eval < 200 | Refuse the VSS claim |
| 5 | VSS_oos CI includes 0 | The deterministic baseline is the default plan; the stochastic plan is advisory |
| 6 | Calibration ≠ Calibrated | Color ≤ Estimated; refuse the P(target) headline; widen ranges by reference class |
| 7 | AISC forecast with drivers lacking FA certificates | Watermark "plan-only metric" |
| 8 | AISC variant gap not decomposable into named layers | Refuse |
| 9 | A top-3 sensitivity input has evidence class `assumed` | Banner: "decision rests on assumption X" |
| 10 | Selection frequency < 0.8 | Flag "fragile"; show the runner-up and the flip probability |

### 7.4 Monitors

- **M1.** Ensemble PIT/coverage: on synthetic truth first, then on reconciliation (E029).
- **M2.** Schedule-compliance e-process.
- **M3.** F-factor drift e-process.
- **M4.** Price-band re-plan trigger.
- **M5.** Capex/slip tracking: alarm when P(final > AFE) > 0.5 under the project's posterior.
- **M6.** CI solver regression on MineLib gaps and times vs pinned fixtures (E022).
- **M7.** Plan churn.
- **M8.** Goodhart alarm: a KPI shadow price exceeds 5% of programme E[NAV].

### 7.5 Proof obligations (statement → verification)

| ID | Statement | Verification |
|---|---|---|
| PO-1 | Closure feasible | O(arcs) independent check in `fm-verify` |
| PO-2 | Max-flow = min-cut; flow is conserved; no i64 overflow | Pre-solve Σ\|v\| < 2⁶² guard |
| PO-3 | Nested shells: λ₁ ≤ λ₂ ⇒ pit(λ₁) ⊆ pit(λ₂) (parametric monotonicity, when block values are monotone in λ) | Property test |
| PO-4 | Dual bound valid | Dual feasibility verified with outward interval rounding, then weak duality (V4 pattern) |
| PO-5 | Schedule feasible | Precedence across periods and capacities, independent checker (`fm-reference`, R2 §5.4) |
| PO-6 | SAA bound CIs at nominal coverage | Toys with a known optimum |
| PO-7 | VSS sets disjoint and paired | Type-enforced |
| PO-8 | CVaR LP equals the sorted-tail average | Property test vs `fs-robust::empirical_cvar` |
| PO-9 | Plan and actual AISC use the same `aisc()` and `rules_version` | — |
| PO-10 | Plan→actual bridge sums exactly | — |
| PO-11 | Regret ≥ 0, from best-in-hindsight over the same scenario set | — |
| PO-12 | Calibration claims hold | Binomial CI or e-process |
| PO-13 | Same inputs and seeds ⇒ same plan digest | Replay test |
| PO-14 | Metamorphic: price ↑ ⇒ pit ⊇ (positive-metal blocks); capacity ↑ ⇒ LP value non-decreasing; duplicated scenario ⇒ SAA solution invariant; money-unit rescale ⇒ same plan | Metamorphic tests |
| PO-15 | Lane–LP consistency: in a single-mill toy, the mill-hour dual equals (f+F)/H | — |

### 7.6 Assumptions ledger

| ID | Assumption | Class | Measured by | If false |
|---|---|---|---|---|
| A1 | Ensembles are calibrated at decision support (block, period) | distributional | M1, T3-3 | Guard 6; K-CAL |
| A2 | kWh/t is additive over blends within ±8% (H-ADD) | structural | Plant data | Piecewise blend classes |
| A3 | Few coupling rows per period (BZ-friendly) | computational | T3-1, K-BZ | `fm-lag` plus aggregation |
| A4 | Compliance kernel K is stationary within a quarter | distributional | M2 | Shorter windows; widen intervals |
| A5 | Scenario index captures the dominant correlations (price–cost, country–price H-FISC) | structural | T7-2, T7-5 | DRO over family weights |
| A6 | Reference classes transfer to this company | transfer | Company random effect, M5 | Widen; company-only after n ≥ 10 projects |
| A7 | P/NAV model is adequate for buyback valuation | modelling | T7-5 | Report the P/NAV-conditional hurdle |
| A8 | Conversion priors are learnable from statement histories | data | Data audit (OPEN) | Elicited priors, flagged `elicited` |
| A9 | FA certified reads are available at close cadence | operational | R4 interim posture | Plan-only watermark (guard 7) |
| A10 | Recovery is per source domain, not per blend | structural | Plant reconciliation | Blend-class recovery |

### 7.7 Failure modes and deterministic conservative fallbacks

| Failure | Detection | Fallback |
|---|---|---|
| Miscalibrated realizations make the stochastic plan overconfident | M1, T3-2 arms | Deterministic baseline is the default; reference-class widening; Estimated color |
| Solver stall or timeout | `Cx` budget exhaustion (E012) | Last certified plan plus the incumbent with its current bound; never an uncertified "optimal" |
| Lagrangian oscillation destabilizes MPC | Price-variance monitor | Freeze prices at the last stable value; tactical targets only |
| Execution far below plan | M2 | Re-plan with an updated K; short-term MPC on available faces |
| Capex blowout | M5 | Stage-gate hold; recompute the DecisionCard with posterior overrun |
| Country shock | Regime state change | Recompute the programme; liquidity floor binds first |
| Pay-metric gaming pressure | M8 | Board sees the KPI price; no auto-adjustment |
| FA unavailable | Read failure | Plan-only metrics, watermarked |
| Lab-oracle drift | Fixture hash mismatch | Fail CI; never regenerate from the candidate (E022) |

### 7.8 Verification plan

1. **MineLib** (UPIT/CPIT/PCPSP; instance set per Espinoza et al. 2013, R7 §5.3): exact UPIT parity with MineFlow; PCPSP bounds and gaps vs best-known and HiGHS fixtures.
2. **Synthetic-truth mine** (`fm-gen`, R7 §9.5.6): T3-2 three-arm VSS, T3-3 coverage, T3-4 EVSI flips, T3-6 execution, plus injected-bias recovery in reconciliation.
3. **Lab-oracle parity** (`fm-oracles`, pinned): LP to 1e-6 relative; MILP objectives to 1e-6 on small instances; blend LP vs HiGHS.
4. **Capital:**
   - T7-1 brute force on 20 or fewer binaries;
   - T7-2 bootstrap stability;
   - T7-3 bitemporal hindcast of past approvals;
   - T7-4 synthetic district;
   - G-CAP-2 reference-class fit.
5. **Accounting:** a synthetic FA book where `fm-metrics` reproduces AISC by-product, co-product and comp-adjusted variants exactly from certified rows (PO-9, PO-10).
6. **Determinism:** replay digests across `LabRuntime` seeds (PO-13); metamorphic suite (PO-14).

### 7.9 Galaxy-brain cards (ILLUSTRATIVE numbers; each card has an equation, substitution, intuition, assumptions and flips)

```text
G1 lane_cutoff_with_stockpile
 Eq   g_m = h/((p−k)y);  g_h = (h + (f+F)/H)/((p−k)y);  F = δ·V
 Sub  p=$3,000/oz=$96.45/g, k=$6/oz, y=0.90 → (p−k)y=$86.63/g; h=$22/t; f=$60M/y; H=10 Mt/y; δ=8%; V=$2.0B → F=$160M/y
      g_m = 22/86.63 = 0.254 g/t;  g_h = (22+220/10)/86.63 = 0.508 g/t;  late life V=$0.5B → (22+10)/86.63 = 0.369 g/t
      0.40 g/t parcel stockpiled 12 y: (0.40·86.63 − 22 − 1.8)·1.08⁻¹² − 0.6 = 10.85·0.397 − 0.6 = +$3.7/t → stockpile, not waste
 Why  Early in life, the opportunity cost of time doubles the mill cut-off; the 0.25–0.51 g/t band is option inventory, not waste.
 Asm  Mill binds (else balancing cut-off); V iterated from the plan; grades are posterior means (expected-loss class under uncertainty).
 Flip Recovery loss on the stockpile ≥27% (oxidation) or price −27% → parcel value ≤ placement cost → waste.

G2 vss_out_of_sample (three-arm, synthetic-truth mine)
 Eq   VSS_oos = E_B[NPV(x_sto)] − E_B[NPV(x_det)], B disjoint from opt set A, paired CRN; SE = sd(diff)/√n_B
 Sub  calibrated: 1,905 − 1,812 = $93M (5.1%); sd 110, n_B 200 → SE 7.8 → 95% CI [78, 108]
      in-sample on A: $140M → optimism $47M; truth arm (K=30 truths): $71M, sd 140 → CI [21, 121]
      under-dispersed arm (σ×0.6): $12M, CI [−5, 29] → stochastic value NOT demonstrated
 Why  A third of the in-sample "stochastic value" is optimizer's curse; the rest survives only with calibrated realizations.
 Asm  Generator representative; S_A = 50; compliance kernel off (on: VSS_oos = $58M).
 Flip Truth-arm CI includes 0, or M1 fails → (b) demoted to advisory (guards 5–6).

G3 three_gap_ledger (duality certificate)
 Eq   gap_opt = (UB* − z_inc)/UB*, UB* = outward-rounded verified dual bound; sampling gap = UB_SAA − LB_eval; calibration = PIT coverage
 Sub  deterministic schedule: BZ LP 2,140.0 → verified UB* 2,140.4; TopoSort+LS 2,091.0 → gap_opt = 49.4/2,140.4 = 2.31%
      stochastic plan: UB_SAA 1,948 ± 18 (10 batches), LB_eval 1,905 ± 8 → sampling gap 43 ± 20 (2.2% [1.2, 3.3])
      calibration: 90% NPV intervals covered 24/30 truths; P(X ≤ 24 | Bin(30, 0.9)) = 0.073 → AMBER
 Why  "2.3% from optimal" is a statement about the model; the model is amber-calibrated, so the headline stays Estimated.
 Flip Coverage ≤ 22/30 (p < 0.01) → Miscalibrated → widen (guard 6); gap_opt > 5% → "heuristic plan".

G4 schedule_compliance_forecast
 Eq   c ~ BetaBinomial posterior predictive; metal = plan·(c + (1−c)·ρ)
 Sub  12 months: 2,520/4,800 planned blocks mined as planned (0.525), between-month sd 0.07 → next c: 0.53, 80% PI [0.44, 0.62]
      plan 95 koz, substitution grade ratio ρ = 0.82 → 95·(0.53 + 0.47·0.82) = 87.0 koz [85.4, 88.5]; past 80% PIs covered 9/12
 Why  The plan number is not a forecast: execution alone costs ~8 koz/month (≈$36M at $4,500/oz) before any grade uncertainty.
 Asm  ρ stable; months exchangeable within a quarter (A4); equal block tonnage.
 Flip ρ → 1 (substitutes as rich): metal gap vanishes but sequence debt grows; an M2 alarm resets the prior window.

G5 cvar_capital_allocation (5 joint scenarios stand in for ≥2,000)
 Eq   max E[ΔNAV] s.t. CVaR_20%(ΔNAV) ≥ τ (Rockafellar–Uryasev LP); regret_s(x) = max_x' V_s(x') − V_s(x)
 Sub  S1 low price+overrun, S2 low, S3 base, S4 high, S5 country shock; budget $800M = two $400M packages of:
      A brownfield [−60, 40, 220, 480, 220] μ 180;  E greenfield, fiscal-risk country [−150, 60, 420, 960, −330] μ 192
      D buyback at P/NAV 0.85, own-NAV multipliers [.70, .85, 1.0, 1.4, .95] → 400(m/.85 − 1) = [−70.6, 0, 70.6, 258.8, 47.1] μ 61.2
      A+E μ 372, CVaR −210 | A+D μ 241.2, CVaR −130.6 | E+D μ 253.2, CVaR −282.9
      NPV rank and minimax regret (A+E max regret 377 vs A+D 701) pick A+E; with τ = −150 → A+D: price of risk appetite $130.8M
      Stream on E (deposit $120M; scenario cost [60, 80, 130, 260, 50], ≈ fair) → E_s [−90, 100, 410, 820, −260]; A+E_s μ 376, CVaR −150 → chosen
 Why  The buyback is a levered bet on our own NAV, not a risk-free hurdle; the stream is bought as tail insurance, not cheap money.
 Asm  Stream ≈ fair (real streams cost more: OPEN); P/NAV fixed per scenario; equiprobable scenarios.
 Flip Stream PV cost > $135M → A+D; P/NAV < 0.66 → A+D overtakes on mean; S5 hazard ×2 under DRO → re-rank.

G6 junior_earn_in_with_conversion_haircut
 Eq   R = h_pub(c_IR·M_ind + c_FI·c_IR·M_inf); NAV = R·v·P_dev; staged = Σ p·max(0, 0.51·NAV_state − K₂)/(1+r) − K₁
 Sub  M_ind 1.0, M_inf 3.0 Moz; h_pub .85, c_IR .75, c_FI .45, v $300/oz, P_dev .65 → R = .85(.75 + 1.0125) = 1.498 Moz; NAV $292M
      market cap $350M (≈$87.5 per resource oz): acquire at +35% (472.5) → −$180M; 19.9% at market: .199(292 − 350) = −$11.5M
      earn-in 51%: K₁ $35M reveals c_FI ∈ {.75 p .35 → NAV 497 (P_dev .80); .29 p .65 → NAV 197 (P_dev .55)}; K₂ $110M at t=1
      good .51·497 − 110 = 143.6; bad .51·197 − 110 = −9.7 → walk; value = .35·143.6/1.08 − 35 = +$11.5M (precommit +$5.7M; staging option $5.8M)
      acquisition break-even: .85(.75 + 2.25·c_FI)·300·.65 = 472.5 → c_FI = 0.93
 Why  The market price implies ~93% conversion of inferred ounces; buy information (earn-in), not ounces.
 Asm  Priors illustrative (§6.4 OPEN); NAV net of capex; no dilution, control-premium or bargaining dynamics.
 Flip Own-history c_FI prior ≥ 0.6 with tight spread, or P_dev ≥ 0.85 → gap narrows; junior refuses terms → stake + information rights.

G7 reference_class_capex_prior
 Eq   X = final/BFS capex. Targets (Bertisen & Davis 2008): E[X] = 1.14; P(|X−1| > .15) ≈ .5; P(X ≥ 2) ≈ 1/13
 Sub  lognormal fitted to mean and tail: μ = .020, σ = .472 → P(|X−1| > .15) = .349 + .400 = .75 ✗ (vs ≈ .5)
      mixture .923·N(1.043, .20) + .077·Blowout(mean 2.3): mean .963 + .177 = 1.140 ✓; outside ±15% .923·.464 + .077 = .505 ✓; P(X ≥ 2) ≈ .077 ✓
      BFS $1.6B → mean $1.82B; P90 ≈ 1.44× = $2.31B; P(≥ $3.2B) ≈ 7.7%
 Why  Overruns are a well-behaved core plus a separate blowout regime; one lognormal over-spreads the core to fit the tail.
 Asm  B&D 63-project class; megaproject classes (McKinsey >40%) are heavier, so the class choice is itself a decision (OPEN).
 Flip The company's own approvals (≥10 projects) show a lighter blowout share → narrower posterior.
```

---

## 8. Plan-ready material

### 8.1 Crates

| Crate | Status | Contents | Depends on (V-refs) |
|---|---|---|---|
| `fm-pitflow` | DESIGN | Pseudoflow, shells, closure oracle | — |
| `fm-bz` | DESIGN | BZ relaxations | fm-pitflow, fsci-opt (V2) |
| `fm-lag` | DESIGN | Lagrangian / bundle bounds | — |
| `fm-topo` | DESIGN | TopoSort + local search incumbents | — |
| `fm-pdlp` | DESIGN | Certified PDHG | fs-truss pattern (V4) |
| `fm-milp` | DESIGN | Small branch-and-bound | — |
| `fm-stoch` | DESIGN | SAA, VSS, MMW, PH | fs-rand |
| `fm-sched` | DESIGN | Strategic and tactical models | — |
| `fm-blend` | DESIGN | Blend LP, stockpile bins | — |
| `fm-cutoff` | DESIGN | Lane + H3e policy search | CMA-ES lift (V6) |
| `fm-mpc` | DESIGN | Short-term receding horizon | — |
| `fm-haul` | DESIGN | DES fleet simulation | fs-tropical (V8) |
| `fm-compliance` | DESIGN | Compliance forecasts | fs-eproc |
| `fm-cost` | DESIGN | CostDriverCards | FA certified reads |
| `fm-metrics` | DESIGN | AISC/AIC/C1 rules engine (R4 item 14) | — |
| `fm-capital` | DESIGN | CapitalCandidate / CapitalProgram | — |
| `fm-options` | DESIGN | Stage gates, LSM | — |
| `fm-refclass` | DESIGN | Reference-class priors | — |
| `fm-country` | DESIGN | Regime chains | — |
| `fm-decision` | DESIGN (R2 name) | DecisionCards | franken_decision (V9), fs-voi (V7), fs-robust (V5) |
| `fm-oracles` | lab only | Oracle fixtures | — |
| `fm-gen` | DESIGN | Synthetic-truth mines and districts | — |
| `fm-verify` | DESIGN | std-only certificate and feasibility checker | — |

### 8.2 Gates (TARGET)

| Gate | Threshold |
|---|---|
| G-PIT-1 | MineLib UPIT exact (integer-scaled) with verified closure |
| G-PIT-2 | 10⁷ blocks in ≤120 s, ≤32 GB |
| G-BZ | PCPSP LP within 1e-6 relative of HiGHS fixtures |
| G-TOPO | Gap ≤3% on ≥80% of PCPSP instances, ≤5% on all |
| G-STO | T3-2 calibrated-arm VSS_oos CI excludes 0 **and** the miscalibrated arms trip M1 before publication |
| G-BLEND | HiGHS parity 1e-7; bins vs McCormick ≤1% |
| G-COST | Synthetic FA book: AISC variants reproduced to $0.01/oz; bridge closes exactly |
| G-COMP | ≥24-month hindcast; 80% PI coverage in [0.70, 0.90] |
| G-CAP-1 | Brute-force parity on ≤20 binaries; CVaR LP = sorted tail |
| G-CAP-2 | Reference-class prior reproduces B&D's three statistics within ±0.03 |
| G-CAP-3 | Bootstrap selection frequency reported on every card |

Kill gates and fallbacks are in §4.4.

### 8.3 Risks

1. **The calibration gap swamps everything.** Mitigation: the K-CAL guard; publish deterministic plus reference-class widening.
2. **BZ weakens with many side constraints.** Mitigation: `fm-lag` plus aggregation.
3. **Pay-metric politics:** shadow prices that embarrass the scorecard (E025). Mitigation: show, never auto-tune; board-owned τ and KPI caps.
4. **The Avathon incumbent at Barrick NA (E024).** Mitigation: position `fm-capital` and the certificates as the auditable kernel that agents can read (R6 §7.2a/c).
5. **Data access for conversion and haircut priors.** Mitigation: elicited priors flagged `elicited`.
6. **Scope creep into a general MILP.** Mitigation: the explicit non-goal plus the `UntrustedCandidate` escape hatch.

### 8.4 OPEN

1. Does the `UntrustedCandidate` import respect the suite rule? (Owner decision.)
2. Real distribution of schedule compliance. (30% is "as low as".)
3. Conversion and h_pub datasets (§6.4).
4. Is H-FISC (price-dependent fiscal hazard) supported by data?
5. Reference-class choice for megaprojects vs B&D.
6. Is P/NAV valuation of the company's own shares acceptable to the CFO?
7. 10⁸–10⁹ blocks: are aggregation error bounds provable?
8. Stream and royalty market cost of capital (R6 §9.13).
9. Who signs the risk appetite τ? (Board; mechanism OPEN.)

### 8.5 Top six decisions for the owner

1. **Adopt H3d plus the three-gap `PlanCertificate` as the planning contract.** No plan is published without bound, gap, calibration status and color.
2. **Build a pseudoflow-centred solver stack.**
   - Build `fm-pitflow`, `fm-bz`, `fm-lag`, `fm-topo` and a *small* `fm-milp` in pure Rust.
   - Explicitly do not build a general commercial MILP.
   - Oracles stay in `fm-oracles`.
3. **Gate COSMO-style stochastic planning on the T3-2 three-arm out-of-sample VSS test.** Until it passes, the deterministic baseline plus H3e policy search is the default.
4. **Use one `fm-metrics` AISC function for plan and actual, fed by FA-certified rows.** AISC is never an objective, and its shadow price is always published.
5. **Make the per-share two-stage `CapitalProgram` the single capital decision object.** It runs on a shared `ScenarioIndex`, includes buybacks and streams as scenario-dependent candidates, and exposes KPIs as priced constraints.
6. **Build `fm-refclass` and conversion/haircut priors as explicit, visible adjustments.** They are learned from the company's own bitemporal approval and resource history, and stand before any capital card reaches a board.
