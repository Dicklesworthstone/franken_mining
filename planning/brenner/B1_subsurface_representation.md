# B1 — PQ-1: Canonical subsurface representation (Brenner pass)

- RUN_ID: fm-2026-10-28-a · Pass: B1 · Pivotal question: **PQ-1** (MOR_CASE.md)
- Inputs: `planning/mor/MOR_CASE.md` (E001–E033), `research/R1_frankensim_survey.md`, `research/R7_technical_methods_sota.md` §2.5, §3–4, §6.2, §7.1–7.2, §9–10, `research/R3_numerics_ml_perception_survey.md` §2.2, §4.3, §4.5, §5, and the alien-artifact-coding skill (router, 03, 15, 34, 38, GALAXY-BRAIN).
- frankensim checked read-only at HEAD `20463525d` (2026-10-26). Nothing was built and no file outside this one was modified.
- Labels: **FACT** (verified in code or a cited source), **DESIGN** (proposed here), **HYPOTHESIS** (testable claim), **TARGET** (measurable goal), **OPEN** (unresolved). Literature not verified in R7 is marked [U].

---

## 0. Verdict in one screen

| # | Hypothesis | Verdict | Role in the final design |
|---|---|---|---|
| H-a | The kriged block model is the authority | **KILL as authority** | Survives as a mandatory *declared-estimator projection* and a parity mode, so the QP signs on familiar ground |
| H-b | An ensemble of conditional simulations is the authority | **DOWNGRADE** | Becomes the *exchange currency*: realizations are regenerable, indexed by common random numbers (CRN) and cached, not stored as truth |
| H-c | A generative hierarchical belief object is the authority | **SURVIVE, amended** | Core of the design, with staged ("cut") inference, a hypothesis set plus catch-all, and procedure certificates in place of truth certificates |
| H-d | Event-sourced data plus recipes are the only authority | **SURVIVE, merged into H-c** | Storage and provenance substrate. A recipe without a probabilistic model collapses back to H-a. |
| H-e | Support-indexed authority with a gluing (sheaf) condition (forced alternative) | **DOWNGRADE** | Kept as the reconciliation consistency layer: aggregation maps, with F-factor residuals as obstruction witnesses |
| H-f | Explicit QP wireframes plus tabulated grades are the authority (legacy site practice) | **DOWNGRADE** | Wireframes become *attested interpretations*: inputs with an uncertainty model, never the belief itself |

**Recommended design (DESIGN): the Subsurface Belief Ledger (SBL).** Authority = immutable bitemporal **evidence** + a versioned, content-addressed **BeliefSpec**. The BeliefSpec holds:
- the structural framework: implicit fields, fault chronology, several domain partitions;
- per-domain latent random fields with hyperparameter particles;
- measurement-error and bias priors;
- geometallurgical and geotechnical transfer cards;
- a seed root.

Everything a person sees or a planner consumes is a **projection** keyed by (spec hash, evidence cut, frame, support, estimator, realization set). Each projection carries a receipt with conservation, regeneration-hash and staleness witnesses. The operating twin adds an epoch-indexed **depletion geometry** and **anthropogenic material bodies** (stockpiles, heaps, TSFs). Both reuse the field machinery, but neither is ever kriged as in-situ rock.

---

## 1. Framing: three meanings of "authority" that the question conflates

1. **Authority of record.** What a QP/CP signs and a regulator can re-perform. (E031: classification is never automated. E023: software never sets `reserve_disclosure_claim`.)
2. **Authority of belief.** The single object from which every number (tonnes, grade, P(>cut-off), dig lines, F-factors) is derived, so no two modules can disagree silently. (E015: "one authority per number"; "canonical ledger plus derived projections".)
3. **Operational authority.** What the fleet digs against today: a dig line, a GC model, a polygon in the dispatch system (E024: FrontRunner and Pitram at NGM).

**HYPOTHESIS H0.** A design that gives these three meanings to *one* artifact fails the loss function. A single kriged model as authority of belief produces false certainty (loss #1). A full ensemble as authority of record is unauditable and unscalable (loss #2). The answer must separate them: one authority of belief, with certified projections serving as the authority of record and the operational authority.

**FACT (E029, R7 §4.1).** Industry kriged models carry conditional bias. Assibey-Bonsu & Muller (2014) measured an LUC slope of regression ≈ 0.1 against an 8×5 m grade-control grid. Variogram uncertainty is "usually ignored". Bond et al. (2007): 21% of 412 interpreters got a synthetic section right, so structural uncertainty dominates.

---

## 2. Hypotheses: mechanism, falsifier, discriminating test, stress dimensions

The seven stress dimensions are:
- **CoS:** change of support plus the information effect
- **Scale:** 10⁸–10⁹ blocks, 10⁵ holes, 100+ realizations
- **Recon:** reconciliation
- **ME:** multi-element and deleterious elements
- **GG:** geometallurgy and geotechnics
- **Audit:** QP auditability
- **Incr:** incremental update

### H-a — Kriged block model is the authority (industry practice)

**Mechanism.** Domain wireframes are filled with a (sub-)blocked grid. Composites are estimated with ordinary kriging (OK) or inverse distance squared (ID²) per domain, using QP-fitted variograms. Classification is drawn from kriging variance, slope of regression and drill spacing. The QP signs the file.

| Dim | Handling | Weakness |
|---|---|---|
| CoS | Block kriging at the SMU, or UC/LUC/DGM post-processing for recoverables | Kriging at SMU with sparse data smooths, so tonnage above cut-off is wrong. The information effect is ignored unless LUC is applied, and LUC slope ≈ 0.1 (E029). |
| Scale | Good. One value per block per attribute: 10⁹ × 20 attributes × 4 B = 80 GB. | No uncertainty is carried, so scaling is cheap because it is incomplete |
| Recon | F1/F2/F3 against the model | Factors become fudge multipliers (R7 §4.3 "factor creep"); there is nothing to recalibrate |
| ME | Independent OK per element | Cross-element consistency (Au–As, Cu–S) is lost, and ratios such as CuAS/CuT come out biased |
| GG | Kriges hardness and recovery like grades | **Wrong for non-additive variables** (R7 §7.1). RQD and GSI are ordinal or judgement indices. |
| Audit | Excellent. This is what QPs know. | Hidden choices (capping, domaining) are not recorded as decisions (R7 §3.5) |
| Incr | Re-run the neighbourhoods inside the search radius | A variogram refit forces a global rerun. No paired before/after comparison exists. |

- **Falsifier.** In a decision-relevant domain, the slope of regression of true or GC SMU grade on kriged SMU grade is < 0.9, **or** the value of the stochastic solution (VSS, E030) for pit or schedule exceeds materiality (TARGET: > 2% NPV). Either makes a single-estimate authority a source of loss #1.
- **Cheap test (≤ 1 week).** Generate a synthetic-truth deposit, or use an exhaustive dataset of the Walker Lake type [U]. Sample it on a realistic drill grid. Compare tonnage and metal above three cut-offs at the SMU, after simulated GC selection, between (i) the OK block model and (ii) the e-type of a simulation ensemble with simulated GC. H-a loses if its error against truth is outside what it would claim, which is zero.
- **Refuter-first attack.** "H-a is what every regulator accepts, and Barrick's QPs sign it today. Replacing it is loss #3 (no wedge)." *Response:* that argues for keeping H-a as a **projection**, not as the authority. Nothing in the codes prescribes a method (R7 §4.2). The JORC 2026 draft mandates reconciliation and defines "Risk = effect of uncertainty" (E031), which H-a cannot express.
- **Verdict: KILL as authority. SURVIVE as `Estimator::DeclaredOk` projection with bit-for-bit parity.**

### H-b — Ensemble of conditional simulations is the authority

**Mechanism.** Per domain, conditional realizations (SGS, turning bands, FFT-MA) are generated at fine support and averaged to the SMU. The ensemble of N realizations *is* the resource. All downstream modules consume realizations by index (R7 §9.5 "realization ensembles are the lingua franca").

| Dim | Handling | Weakness |
|---|---|---|
| CoS | Correct for linear variables: simulate at point or fine support, then average. Simulated GC captures the information effect. | Non-linear variables still need declared blend rules |
| Scale | 10⁹ blocks × 100 realizations × 10 attributes × 4 B = **4 TB per model version**, ×versions | Storage and I/O dominate. Each update is a full new ensemble. |
| Recon | Coverage and PIT of panel outcomes are computable | Bias learned by reconciliation has nowhere to live except a new ensemble |
| ME | Co-simulation (LMC, PPMT/MAF decorrelation [U]) | Same as H-c |
| GG | Simulate proxies; apply transfer functions per realization | Fine |
| Audit | Weak. A pile of 100 realizations is not a model a QP can interrogate. | "Which variogram, which domains, why?" is not answerable from the realizations themselves |
| Incr | Regenerate everything, or reweight realizations (weights degenerate fast) | Realization identity breaks across updates, so VOI audits cannot pair outcomes |

- **Key structural defect (FACT-level reasoning).** An ensemble is a *sample conditioned on fixed hyperparameters and a fixed interpretation*. KU-p (variogram) and KU-s (structural alternatives, E032) are absent unless they are added generatively. Adding them generatively turns H-b into H-c.
- **Falsifier.** On the synthetic-truth mine, 80% intervals of panel metal from a fixed-hyperparameter, fixed-interpretation ensemble cover truth less than 70% of the time in sparse (Inferred-like) areas.
  - HYPOTHESIS: this happens, because variogram and contact uncertainty dominate there (R7 §4.1).
- **Cheap test.** Run the same ensemble two ways, with the hyperparameters fixed and with them sampled from a posterior. Compare panel coverage on held-out truth.
- **Refuter-first attack.** "Stochastic mine planning (COSMO, E030) consumes realizations directly, so an ensemble *is* what the planner needs." *Response:* the planner needs realizations, not ensemble authority. Regenerable realizations with stable indices serve the planner better and are smaller.
- **Verdict: DOWNGRADE to exchange currency.**

### H-c — Generative hierarchical belief object is the authority

**Mechanism.** The authority is one object. It holds:
- raw data;
- the structural and implicit geological model: domains, contacts and faults as implicit functions with chronology;
- per-domain spatial random fields with hyperparameter posteriors;
- measurement-error models;
- transfer functions.

Block models, realizations, wireframes, grade shells, entropy maps and classification-support metrics are *content-addressed projections* at a declared support and resolution.

| Dim | Handling | Risk |
|---|---|---|
| CoS | Support is a type. Change of support goes through explicit operators: regularization for additive variables, declared blend rules otherwise, and simulated GC for the information effect. | Operators must be correct and complete (guard table §6.3) |
| Scale | Store data plus spec plus summaries; regenerate realizations per tile from keyed RNG (galaxy card 1) | Regeneration throughput is a TARGET, not a fact |
| Recon | Reconciliation updates *bias hyperpriors* by domain and period (E029). Forecasts are immutable and scored. | Joint inference over the material-flow graph is PQ-3/PQ-4 territory |
| ME | Multivariate anamorphosis plus decorrelation; censored likelihood for below-detection values; heterotopic sampling | Model complexity |
| GG | Additivity is declared per attribute; geomet and geotech are fields over their *own* domain partitions | Sparse geomet samples on a large support (R7 §7.2) |
| Audit | QP signs a snapshot = (evidence cut, spec hash, projection receipt). Declared-OK parity plus the standard validation pack. | A "Bayesian black box" perception |
| Incr | Conditioning by kriging with CRN: new data changes only the local correction term, and realization indices persist (galaxy card 3). Hyperparameter particles are reweighted, with a refit on ESS collapse. | Structural contradictions need a human act |

- **Falsifier.** On the synthetic-truth mine, H-c's 80% panel intervals cover no better than H-b's (difference within Monte Carlo noise) while costing > 3× the compute. **Or** an independent QP cannot reproduce the declared-OK projection and validation report from the snapshot hash. **Or** staged inference understates uncertainty so badly that coverage stays below 0.7 after structural hypotheses are included.
- **Cheap test.** The same synthetic deposit as H-a/H-b, with 50 additional holes appended. Measure:
  1. coverage;
  2. the fraction of tiles recomputed;
  3. bitwise equality between the incremental and from-scratch results (PO-7);
  4. QP replay.
- **Refuter-first attacks, with amendments:**
  1. *"Full joint inference of structure, fields and hyperparameters at 10⁵ holes is intractable."* Correct. **Amend:** staged "cut" inference [U, Plummer 2015]: structure → domains → fields. Structure is not updated by grade data except through enumerated hypotheses (KU-s) plus a catch-all mass. The cut is recorded as assumption A-4.
  2. *"SPDE/GMRF at 10⁹ nodes is impossible."* Correct. Nested-dissection fill for a 3D GMRF grows about as n^(4/3), roughly 10¹² at n = 10⁹ [U, standard]. **Amend:** use SPDE only for hyperparameter inference and coarse latent fields (≤ 10⁶–10⁷ nodes). Fine-support realizations come from turning bands or FFT-MA plus kriging correction.
  3. *"'Certified projection' overclaims."* Correct. fs-evidence only certifies Exact/Enclosure numerics (FACT: `crates/fs-evidence/src/lib.rs:1182-1230` refuses `Estimate`). **Amend:** projections carry *procedure* certificates (regeneration hash, conservation, volume enclosure) and *statistical* summaries with Estimated colour. They are promoted to Validated only through reconciliation calibration.
  4. *"Legacy sites hold hand-digitized wireframes, not implicit models."* **Amend:** admit wireframes as attested interpretations (absorbing H-f), converted to implicit sign via generalized winding number.
- **Verdict: SURVIVE, amended.**

### H-d — Event-sourced "data + recipes are the only authority"

**Mechanism.** Every assay, survey, log and QA/QC decision is an immutable bitemporal event (E015, R7 §10). Every model is a pure function `recipe(events@T)`. Nothing else is authoritative.
- **Strength.** Perfect provenance, and hindcasts answer "what did we know at T" (R7 §10).
- **Refuter-first attack.** A recipe can be "ID² with these parameters": perfectly reproducible and still wrong. **Reproducibility is not validity** (fs-evidence doctrine: "verification is not validation", R1 §1.6). H-d specifies *where truth is stored*, not *what is believed*. It also cannot answer P(panel metal < plan) unless the recipe is a probabilistic model, which is H-c. A purist replay of 10⁹ blocks per query is also infeasible.
- **Falsifier.** A full replay from events to a decision-ready projection exceeds the operating budget (TARGET: resource update ≤ 24 h, GC update ≤ 1 h) without materialized caches.
- **Verdict: SURVIVE as substrate, MERGED.** Authority = events + BeliefSpec; caches are projections with receipts.

### H-e — Support-indexed authority with gluing (forced alternative beyond the binary)

**Mechanism.** There is no single representation. Each support and horizon has its own authority:
- panel and long-term support: the resource model;
- SMU in active benches: the GC model;
- mined volumes: the as-mined record;
- plant: metallurgical accounting.

Authorities must agree under *restriction maps* (aggregation of tonnes and metal), a sheaf-style gluing condition (skill ref 03). Disagreements are cocycle defects, which are exactly F1 and F2.
- **Strength.** It matches how operations already run: distinct teams own the long-term model, GC and metallurgical accounting. Gluing makes reconciliation a first-class *consistency* computation.
- **Refuter-first attack.** Several authorities mean several truths. A defect is *detected* but nothing *learns* from it. The F1 bias does not flow back as a prior into the next resource model. This is the silo the R7 §9.1 table blames.
- **Falsifier.** On the synthetic mine, inject a +8% resource-model bias in one domain. After 12 reconciliation periods, H-e alone must reduce the forecast bias of the next periods to below 2%.
  - HYPOTHESIS: H-e cannot do this. H-c with a bias hyperprior can.
- **Verdict: DOWNGRADE to the consistency layer.** The restriction maps and obstruction witnesses are kept (§6), and the GC model becomes an *observation set* on the single belief (blastholes with their own measurand, support and bias), not a peer authority.

### H-f — Explicit wireframes plus tabulated grades (legacy site practice)

**Mechanism.** QP-digitized solids (ore domains, oxidation surfaces, faults) are the authority, and the block model is a fill.
- **Refuter-first attack.** A wireframe has no uncertainty, no chronology and no re-derivation. Bond et al. (2007) shows the interpretation is the *least* certain element.
- **Verdict: DOWNGRADE to `AttestedInterpretation` input.** Retain the bytes, author and date. Convert to implicit sign through the winding number. Attach a contact-uncertainty model (a displacement field or an alternative-hypothesis set).

### Checked nulls

- A neural or implicit continuous field as authority (fs-rep-neural) adds nothing over H-c's latent fields and is unauditable. Null.
- A multiple-point-statistics-centred authority is a sub-case of H-c, with the training image as one KU-s hypothesis (R7 §4.1). Null.

---
## 3. The surviving design: Subsurface Belief Ledger (SBL)

### 3.1 Layers and authority (DESIGN)

| Layer | Content | Mutability | Authority? |
|---|---|---|---|
| L0 Evidence | Collars, surveys (with instrument error), intervals, assays (method = *measurand*, QA/QC state, batch lineage), logs (human/scanner/ML as separate evidence classes, R7 §3.1), structural measurements (orientation + confidence), density determinations, geophysics grids, mapping, blasthole samples, survey epochs (DTM, CMS scans), plant reconciliation | Append-only, bitemporal (E015, R7 §10); corrections are new events | **Yes (data)** |
| L1 Interpretation | Structural framework: stratigraphic/intrusive potential fields, fault surfaces with offset and chronology (cut/onlap/erode rules, after GemPy/LoopStructural, R7 §2.5), vein/lode surfaces, weathering surfaces (BOCO/TOFR [U]), **several domain partitions** (grade, oxidation, geomet, geotech, hydro), alternative hypotheses H₁..Hₖ with weights + catch-all mass | Versioned; every version is a signed human act (`AttestedInterpretation`) | **Yes (spec)** |
| L2 Property fields | Per (partition, domain, variable-group): anamorphosis, decorrelation, nested anisotropic covariance, trend, **hyperparameter particles + weights**, measurement-error model (FSE + analytical, R7 §3.2), method-bias terms (fire assay vs PhotonAssay vs LeachWELL), reconciliation bias hyperprior | Versioned; reweighting is a logged event | **Yes (spec)** |
| L3 Transfer cards | Recovery(Cu, CuAS/CuT, S, As...), throughput(A×b, BWi...), PRI(TOC), UCS(Is50), density(S, Fe) — `CorrelationCard` pattern with `ValidityDomain` + colour (R1 §4.7) | Versioned | **Yes (spec)** |
| L4 Projections | Block models (any frame, support, estimator), realizations, wireframes/grade shells, entropy maps, classification-support metrics, recoverable resources, domain summary material cards | Immutable, content-addressed, **regenerable**, cached with eviction | **No** — each carries a `ProjectionReceipt` |
| L5 Twin geometry | Depletion epochs (as-mined solids), design solids (plans, not belief), anthropogenic bodies (stockpiles, ROM, heaps, TSF) | Epoch-indexed events + projections | Data: yes; current-state: projection |

The `BeliefSpec` = L1+L2+L3 + evidence cut + seed root is a small, canonical, hash-addressed document (KB–MB). **DESIGN:** the QP/CP snapshot is the triple `(EvidenceCut, SpecHash, Vec<ProjectionReceiptHash>)` plus a human `CompetentPersonAct` record. Software never sets `reserve_disclosure_claim` (E023).

### 3.2 Change of support and information effect (DESIGN)

- **Support is a type, not metadata.** `Support::{Point, Interval{len}, Composite{len}, Block{dx,dy,dz}, Smu{..}, Panel{id}, Stope{id}, Bench{..}, Parcel{id}}`. Any operation mixing supports without a declared operator is refused (guard G2).
- **Linear (extensive/mass-weighted) variables:** simulate at fine support (target ≤ SMU/4 per axis, e.g. 2.5 m for a 10 m SMU), regularize by **density-weighted** averaging: `g_v = Σ ρᵢ vᵢ gᵢ / Σ ρᵢ vᵢ`. Density is itself a field. In massive sulphide, density error can rival volume error in tonnage (assumption A-6; first-order: CV_T² ≈ CV_V² + CV_ρ²).
- **Information effect:** `Estimator::SimulatedGc{pattern, dig}` (the pattern carries spacing and the blasthole sampling-error model). On each realization, sample a virtual blasthole grid with blasthole measurand error, re-estimate, classify by the operating rule (incl. minimum mining width), and account *true* tonnes/metal of what is sent where. This is the simulation route to recoverable resources (Journel & Kyriakidis 2004 [U]; Verly 2005 per R7 §6.2).
- **Fast parity paths:** DGM/UC/LUC (R7 §4.1) as `Estimator::{Dgm, Uc, Luc}` projections — needed because QPs and reviewers know them; never the authority (E029 slope≈0.1).
- **Non-additive variables:** declared `Additivity::NonAdditive{blend_rule}`; projection engine refuses naive averaging (G1). Hardness: aggregate an additive proxy (specific energy at a declared circuit, or 1/throughput) and transform at the end [U standard geomet caveat, R7 §7.1]. Recovery: applies at a declared *processing* support (shift/day blend), not per block.

### 3.3 Scale (DESIGN + TARGET)

- **Store:** evidence (10⁵ holes ≈ 2×10⁷ intervals × ~30 fields ≈ a few GB), spec, hyperparameter particles, and per-block *summaries* at SMU: e-type, conditional variance, P(>c) at ≤4 cut-offs, P10/P50/P90 of metal, domain-membership probabilities, distance-to-data (AOA-style, R7 §9.3). TARGET ≤ 64 B/block → 10⁸ SMU blocks ≈ 6.4 GB per version.
- **Regenerate:** any realization tile bit-identically from `(SpecHash, realization r, tile key)` via keyed counter-based RNG (fs-rand Philox `StreamKey`, FACT `crates/fs-rand/src/lib.rs:131-140`) + conditioning by kriging. Cache tiles under an LRU/cost policy; the cache is never authoritative (G5 regeneration hash).
- **Grid substrate:** sparse bricks (8³ leaf / 32³ internal, after `VdbGrid`) for topology + **columnar per-attribute chunks** (not AoS — see §4 FACT on `Leaf<T>`), Merkle root per projection over brick hashes for incremental diff.
- **Kriging cost:** OK with 24–48 neighbours ≈ 5×10⁴ flop/block → 5×10¹³ flop at 10⁹ blocks: minutes–hour on 64 cores (TARGET, unbenchmarked).
- **SPDE ceiling:** coarse latent mesh ≤ 10⁶–10⁷ nodes (sparse LDLᵀ), not block resolution (H-c amendment 2).

### 3.4 Multi-element and deleterious elements (DESIGN, domain specifics)

| Element / attribute | Representation issue | Design response |
|---|---|---|
| Au (nuggety, coarse) | High nugget; FSE dominates; capping is an estimator choice | Measurement-error model separates geological vs sampling nugget (R7 §3.2, §3.5); capping is a logged `ModellingDecision` with metal-at-risk sensitivity; consider indicator/mixture fields where multiGaussianity fails |
| Ag | Correlated with Au (epithermal) or base metals (Pb–Zn) | Co-simulation within the decorrelated factor space |
| Cu (CuT, CuAS, CuCN) | Sequential-leach ratios define oxide/transition/primary routing [U] | Model CuT plus *ratios* in log-ratio space with closure (ratios ∈ [0,1]); never krige CuAS and CuT independently |
| Sn | Recoverable Sn depends on mineralogy (cassiterite vs stannite) [U] | Distinct measurands `Sn_total` vs `Sn_cassiterite`; geomet card for recovery |
| As, S, Hg | Deleterious/penalty and ARD (PAG/NAF, E023 refusal "unknown PAG → no NAF dumping"); Hg often below detection | **Censored likelihood** for <DL (G10; never DL/2 substitution); heterotopic sampling (As/Hg on subsets) via co-simulation; S feeds ARD classification (NAPP/NPR projection) |
| Carbonaceous / preg-robbing (TOC, PRI) | Few PRI tests on large composites; TOC as proxy | `CorrelationCard` PRI(TOC, domain) with validity box; spatial field on TOC; recovery card consumes PRI per realization |
| Major components (Cu %, S %, Fe %) | Closed compositions | ilr coordinates where closure is material (R7 §2.4); trace (g/t, ppm) modelled in log space |
| Decorrelation | Multivariate Gaussianity | PPMT or MAF [U] then independent factor simulation; back-transform; reproduction checks of cross-variograms and scatterplots |

### 3.5 Geometallurgy and geotechnics (DESIGN)

- **Multiple domain partitions** over one structural framework: geotech domains ≠ grade domains ≠ weathering profile ≠ hydro units. `Partition{id, kind, boundary_rules}` × `Domain`; fields bind to `(partition, domain)`.
- **RQD:** length-ratio over an interval — support-dependent and orientation-biased (Terzaghi weighting [U]); store per-interval with run length; aggregate length-weighted only within declared support.
- **RMR / GSI / Q:** judgement indices, ordinal → `Additivity::Ordinal`, Estimated colour only; modelled as categorical/indicator fields or truncated-Gaussian with elicitation record (R7 §9.3 `elicited`).
- **UCS:** sparse lab tests + many point-load Is50 → `CorrelationCard` UCS(Is50) per domain; field on UCS with measurement-error per method.
- **Structural fabric:** orientation data as directional statistics (Fisher) per structural domain → DFN later (OPEN); not a scalar field.
- Geotech *material models* (Hoek–Brown parameters) are **cards** consuming field projections, not fields themselves (E007/E010: no MC/HB in fs-solid).

### 3.6 Reconciliation and incremental update (DESIGN)

- **GC is an observation, not a peer model.** Blasthole assays enter L0 with their own measurand, support (cone/full-hole), and bias prior; the "GC model" is a projection of the belief at T_gc.
- **F-factors** = ratios of projections over the *as-mined* volume (L5): F1 = proj(spec@T_gc)/proj(spec@T_resource), F2 = plant back-calculated/proj(spec@T_gc). Each period's forecast is registered immutably before the outcome (R7 §9.5 forecast ledger). Residuals update **domain-period bias hyperpriors** (E029) — the learning loop H-e lacks. No multiplicative "factor creep".
- **Incremental drilling:** `realization_r = Z_uc(r) + K·(data − Z_uc(r)@data)` with `Z_uc(r)` fixed by keyed RNG. New holes change only kriging corrections inside the neighbourhood → recompute only dirty bricks (Merkle diff); indices r persist (CRN) → paired pre/post comparisons for VOI audits (galaxy card 3).
- **Hyperparameters:** particle weights updated by importance reweighting; ESS < 0.3·N triggers refit (G8).
- **Interpretation contradiction** (ore logged outside domain beyond tolerance, unexpected fault offset): surprise event (posterior-predictive e-value), affected volume labelled `Conflicted` (E033 knowledge states), human re-interpretation required (G9). Software never edits L1.

### 3.7 Operating-mine twin geometry (DESIGN)

| Object | Representation | Note |
|---|---|---|
| Pit / stope / development as-mined | Epoch-indexed excavation solids from survey events (pit pickups, drone/LiDAR DTMs, CMS stope scans [U]); `mined(T) = ∪ epochs ≤ T` | Each survey is L0 evidence with registration accuracy; overbreak/underbreak = as-mined vs **design** solid (plan object, not belief) |
| Current topography | Projection: latest epoch per area + per-area survey-date staleness map | Never edited in place |
| In-situ remaining | `belief ∖ mined(T)` evaluated at projection time | Depletion is a mask, not a model edit |
| Stockpiles, ROM pads | **Anthropogenic body**: survey volume + parcel ledger (dig block → truck → pile, with provenance mixture) | Grade is a material-flow state (PQ-3/PQ-4), *not* kriged from grab samples as if in situ |
| Heaps | Anthropogenic body + engineering fields (moisture, permeability, temperature) via the same `fm-field` machinery, `DomainKind::Anthropogenic` | Leach kinetics are cards (R1 §5 #12) |
| TSF | Deposition-history layers (implicit by time) + CPTu/piezometer-conditioned geotech fields | GISTM (E026); software never sets `geotechnical_signoff` (E023) |

### 3.8 Exploration stage (interface to PQ-2)

- Regional prospectivity is a marked point process over *deposits* (E028), not a rock field. **DESIGN interface:** when a prospect is drilled, PQ-2's deposit hypothesis instantiates a `BeliefSpec` whose priors (global mean, volume, grade–tonnage) come from the deposit-type model as hierarchical hyperpriors; L1 starts hypothesis-heavy (many KU-s, large catch-all). Same types from first hole to closure — HYPOTHESIS, falsified if exploration-stage objects need fields the spec cannot express (test: instantiate on a public early-stage dataset, OPEN which).

---
## 4. frankensim mapping (verified against HEAD `20463525d`)

### 4.1 Code facts that change the plan (FACT, read-only verification)

1. **`VdbGrid` itself is Cx-free and std-only; the coupling is crate-level.** `crates/fs-rep-sdf/src/vdb.rs` (310 LOC) imports only `std::collections::BTreeMap` and `std::fmt::Write`; the crate's `Cargo.toml` pulls fs-exec/fs-geom (E004). → liftable as one file.
2. **`VdbGrid` leaves are dense AoS:** `Leaf<T>{ mask: [u64; 8], values: [T; 512] }` (vdb.rs:20-31). A leaf costs `512·size_of::<T>()` regardless of occupancy — a 40-attribute f32 struct = 80 KB per leaf even for one active block (thin veins). No frame/rotation, no serialization, no parallel or out-of-core iteration; root and internal maps are `BTreeMap`. The 10⁹ target is "scaled in CI (~12.5k actives)" (`fs-rep-sdf/CONTRACT.md:203`). → topology only; attributes must be columnar.
3. **`fs-rep-voxel::DensityField` is a SIMP volume *fraction* in [0,1]** (`field.rs:340-395`, refuses values outside [0,1]), not bulk density. False friend — R1 §3.1's "densities" row should not be read as rock density. `MaterialField = VdbGrid<u16>` with id 0 reserved for empty.
4. **`fs-geocon::volume_certified` cannot certify any realistic orebody geometry.** It refuses unless `chart.trace_step_claim() == ExactDistance` (`primitives.rs:783-793`); `MeshChart` (`fs-rep-mesh/src/chart.rs:395-430`), `TiledSdf` and `OccupancyChart` keep the default `NoClaim`; `Frep` claims exact only for sphere/cylinder/box/torus/**axis-aligned** half-spaces (`fs-rep-frep/src/lib.rs:853-868`) — an oblique fault plane does not qualify. It also caps work at `VOLUME_MAX_CELLS = 16_777_216` (256³) on a serial dense grid. → tonnage certificates must be built in FrankenMining.
5. **EnSRF core is Cx-free:** `crates/fs-assimilate/src/nonlinear/ensemble.rs` (273 LOC, feature `ensemble`) has no crate imports, takes a cancellation closure `FnMut() -> bool`, supports per-component localization weights. Its state is member-major dense `Vec<f64>` (dim × members) → 10⁸ × 100 × 8 B = 80 GB: unusable for whole block models, good for low-dimensional states and local patches.
6. **`fs-evidence` pulls nightly:** it depends on `fs-qty` (`QuantitySpec` in `ValidityDomain`, lib.rs:474-626), and `fs-qty` has `#![feature(generic_const_exprs)]` (lib.rs:20). "Asupersync-free" (E003) ≠ "stable-Rust". fs-matdb inherits the same.
7. **`winding_exact` is liftable** (`fs-rep-mesh/src/winding.rs`, 214 LOC, needs only `Point3/Vec3`) but uses std `atan2`, not `fs_math::det` → not cross-ISA deterministic as-is.
8. **`conservative_cell_remap` does not compute overlaps** (`fs-mesh/src/adaptivity.rs:1171`): the caller supplies canonical `(source, target, fraction)` triples; it conserves an *extensive* quantity. Grades must be remapped as metal and tonnes, never directly.
9. **`fs-uq::KlExpansion::build`** builds a dense n×n covariance (kl.rs:46-58), `CovarianceKind::{Exponential, SquaredExponential}` only — no spherical, nested, or anisotropic models.
10. **`fs-sparse`** has supernodal LDLᵀ (`direct.rs:658 factor`, `LdltFactor`) but no selected inversion (no Takahashi) → marginal variances of GMRFs need sampling-based estimates or a built selected inverse.
11. Matdb seed corpus: no rock/ore/soil/tailings bundles (grep of `data/matdb/seed-v1` returns only false positives) — confirms E009.

### 4.2 Per-crate disposition

Postures: **Lift** (copy with provenance header + golden hash, E002), **Wrap** (feature-gated dependency behind the spike gate), **Pattern** (copy design, not code), **Oracle** (dev-only), **Don't**.

| Crate | Posture | Why | What FrankenMining builds itself |
|---|---|---|---|
| fs-rep-sdf `VdbGrid` | **Lift** `vdb.rs` (topology/index only) | Std-only, deterministic iteration, measured footprint (Fact 1) | `fm-blockgrid`: columnar per-attribute chunks keyed by leaf coord; rotated/offset grid frames; sub-blocking (parent + variable sub-cells, OPEN which scheme); Merkle hash per brick; out-of-core paging; parallel brick iteration |
| fs-rep-sdf `TiledSdf/AdaptiveSdf/NarrowBand` | Don't (now) | Cx-coupled, SDF-of-shape tools; geology fields are potentials, not distances | Implicit potential-field interpolator (below) |
| fs-rep-voxel | Pattern | `FrameMismatch` refusal and u16 domain ids are good patterns; DensityField is a false friend (Fact 3); fnx + Cx deps | Domain-code bricks; refusal on mixed frames (G3) |
| fs-rep-mesh `winding_exact`, `WindingOctree` | **Lift** `winding.rs` (swap `atan2` → `det::atan2`) | Robust inside tests on dirty legacy wireframes (H-f input); block-proportion estimation | Mesh ingest (OMF/DXF), proportion-in-wireframe per block with error bound |
| fs-rep-mesh `dual_contour`, `mesh_to_sdf`, `MeshChart` | Wrap (after spike) or reimplement | Cx-coupled; MeshChart is Estimate/NoClaim (Fact 4) | Wireframe/grade-shell extraction from FM implicit fields (marching cubes / dual contouring on bricks) |
| fs-rep-frep | Pattern | Hard/blend CSG + half-spaces fit engineering envelopes (pit/stope design solids, exclusion zones); geological chronology (fault offsets, erode/onlap) is *not* min/max CSG | `fm-struct` composition rules; oblique half-spaces need own exactness claim |
| fs-topo `manifold_certificate`, `self_intersection_certificate` | Wrap (spike) else Pattern | Wireframe validity is a sign-off check (watertight, no self-intersection) | Mesh validation report in QP pack |
| fs-mesh `delaunay`, `conservative_cell_remap` | Pattern now; Wrap later for geotech meshes | Remap is extensive-only, caller-supplied overlaps (Fact 8) | Re-blocking with exact overlap fractions for rotated/sub-blocked grids; tonnes+metal conservation witness (G4) |
| fs-geocon `volume_certified` | **Don't** | Refuses all realistic charts; 256³ cap (Fact 4) | `fm-volcert`: divergence-theorem volume of fs-topo-certified closed meshes in interval arithmetic; sure/band block-proportion enclosures |
| fs-geocon `min_thickness_soft` | Pattern (later) | Minimum mining width analogue | PQ-3 |
| fs-query `closest_point`, `raycast` | Don't; Pattern | Cx-coupled, chart-generic; drillhole–wireframe intersection needs exact segment–triangle tests | BVH (FrankenTextiles already lifted `bvh.rs` from fs-query, R1 §2.2) + exact predicates |
| fs-asbuilt | Pattern (Kabsch/Umeyama + pose covariance) | Correspondences only, no ICP, `Point2` profiles, Estimated colour (E005) | Survey-epoch ingest (LAS/LAZ, DTM), ICP, surface–surface volumes (R3 §5 #7) |
| fs-uq `KlExpansion` | **Oracle** only | Dense O(n²)/O(n³), isotropic, unconditional (Fact 9) | Turning bands, FFT-MA, SGS, conditioning by kriging |
| fs-uq Sobol/QMC/MLMC | Lift later | Propagation tools for PQ-3 | — |
| fs-assimilate dense `Belief` | Don't | 256-state cap (E006) | — |
| fs-assimilate EnSRF (`nonlinear/ensemble.rs`) | **Lift** | Cx-free, localization, deterministic (Fact 5) | Use for low-dim states: domain bias factors, density-regression coefficients, GC-zone patches ≤ 10⁵–10⁶ nodes |
| fs-sparse (`direct.rs`, AMG, pcg) | **Lift** (AF) | Supernodal LDLᵀ + AMD for SPDE/GMRF coarse meshes and reconciliation systems | Selected inversion or Hutchinson/RBMC variance estimates (Fact 10); SPDE mesh builder |
| fs-fft | Don't; prefer `fsci-fft` (crates.io 0.4.0, R3) | Cx-coupled, power-of-two only | FFT-MA / circulant embedding with padding |
| fs-bo `Gp` (Matérn ARD) | **Oracle** | GP ≡ simple kriging under the same kernel; lacks rotated anisotropy, nested, spherical, block support, OK constraint (R3 §2.2) | Full kriging engine (SK/OK/UK/co-kriging/block, KE, slope of regression) |
| fs-rand (Philox `StreamKey`, checkpoints, Sobol) | **Lift** (AF; FrankenTextiles precedent) | Keyed streams = CRN + tile-local regeneration | Key schema `(seed_root, partition, domain, var_group, r, brick)` |
| fs-evidence | **Lift** types; strip/shim `QuantitySpec` for stable Rust | `Evidence`, `Certified` (Exact/Enclosure only), `Color` no-laundering, `ModelCard`, typed refusals (E008, Fact 6) | Mapping evidence colour ≠ M/I/I classification (R1 §6.3) |
| fs-ivl | **Lift** (AF) | Interval enclosures for volume certificates | — |
| fs-qty | Don't (nightly); Pattern | No tonne/oz/g/t/ppm/day/kWh·t⁻¹ (E010) | `fm-units` on stable |
| fs-matdb / fs-matdb-store | **Pattern + format-compatible export**; sealed corpus pattern | Pack v7 frozen (R1 §1.8); nightly via fs-qty | `fm-matpack` emitting `frankensim.matdb-manifest.v1` bundles (§4.4) |
| fs-convection `CorrelationCard` | Pattern | Validity box + discrepancy basis + colour | `fm-geomet` transfer cards |
| fs-eproc | **Lift** (AF) | Anytime-valid monitors for F-factor drift and surprise | Monitor wiring |
| fs-voi, fs-oed-e2e | Lift / Pattern (PQ-2/PQ-3) | Conditioning by kriging makes preposterior VOI cheap (variance independent of values) | Line-support drillhole observation operator |
| fs-blake3 | **Lift** | Content addressing | — |
| fs-vvreg | Pattern | Benchmark registry with pinned editions | `fm-geostat-oracles` registry |
| fs-cutfem, fs-solid, fs-conduction | Wrap later via bridge | Consumers of projections (pit-wall stress, seepage), not part of the representation | `fm-fsim-bridge` adapters |
| fs-rep-nurbs, fs-rep-neural, fs-xform, fs-motion | Don't (checked null) | No representational increment for subsurface | — |

### 4.3 `Chart::eval(&Cx)` coupling and non-convergence (E001, E002, E004, E011)

- **DESIGN rule R-1:** no FrankenMining *authority* type (evidence, BeliefSpec, receipts, brick format) may mention a frankensim type. Lifted code is re-owned under `fm-*` with provenance headers + golden hashes.
- **DESIGN rule R-2:** FM numerical kernels take a cancellation/budget closure (`FnMut() -> bool`, as EnSRF does — Fact 5) and are driven from asupersync regions at the orchestration layer. This keeps kernels testable in `LabRuntime` and avoids two `Cx` types (`fs_exec::Cx` wraps asupersync and is built from `CancelGate, Arena, StreamKey, Budget, ExecMode`, R1 §2.6; FACT `crates/fs-exec/src/cx.rs:579`). With asupersync 0.5.0 vs 0.6.0 (E011), a direct `fs_exec::Cx` dependency risks a second runtime type.
- **DESIGN rule R-3:** a single optional crate `fm-fsim-bridge` implements `fs_geom::Chart` for FM implicit fields and block projections so frankensim solvers (CutFEM, conduction-as-seepage) can consume them. It is the only place `fs_exec::Cx` appears, behind feature `fsim`, pinned to a frankensim commit and gated by the spike (gate G0). Kill → lift-only fallback.
- Consequence: frankensim non-convergence (15 failing targets, E001) can delay *engineering consumers* (geotech/seepage) but cannot block the resource representation.

### 4.4 Where material properties live (DESIGN)

| Kind of knowledge | Home | Examples | Colour / authority |
|---|---|---|---|
| Located measurements | L0 evidence (spatial) | Assays, density determinations, UCS/Is50 tests, Bond/SMC/A×b tests on located composites, PRI tests | Measured; feed fields |
| Spatially varying attributes | L2 fields → L4 block projections | Grades, density, hardness proxies, RQD, UCS, TOC, oxidation state | Estimated → Validated after reconciliation |
| Empirical laws | L3 correlation cards (`CorrelationCard` pattern) | UCS(Is50), density(S,Fe), Hoek–Brown, Bond law, PRI(TOC), recovery(CuAS/CuT) | Validated(regime, dataset) or Estimated; never promoted by verification (D6) |
| Non-spatial material classes | `fm-matpack` sealed private corpus in `frankensim.matdb-manifest.v1` | Liner steels, reagents, process water, CRMs/reference ores, TSF construction-zone design parameters, geomembranes | Per-claim provenance; `NoClaimInDomain` refusal |
| Domain summary cards (export) | L4 projection → matdb bundle | "Domain 12 fresh granodiorite: UCS distribution, E, ν" for FrankenSim solvers | Projection receipt hash in `citation`; Estimated |

Client test data is confidential and MNPI-adjacent (E023): only sealed corpora (`seal_corpus`, `StoreError::CorpusChanged` — FACT `fs-matdb-store/src/lib.rs:494,1198`), never seed bundles. The same located test result lives once in L0; matdb cards are *projections* of it, so there is one authority per number (E015).

### 4.5 Stage participation summary

| Stage | frankensim participation | FM builds |
|---|---|---|
| Exploration | fs-voi/fs-oed-e2e patterns (hole placement); winding (legacy interpretations) | Hypothesis-heavy L1; geophysics forward operators (R3 §5 #9) |
| Resource estimation | Lifted fs-rand, fs-sparse, fs-ivl, fs-evidence, vdb.rs, winding.rs, EnSRF; fs-bo/fs-uq as oracles | Desurvey, compositing, variography, kriging, TB/FFT-MA/SGS, conditioning, anamorphosis, implicit modelling, projections |
| Operating twin | fs-asbuilt pattern; fs-mesh/fs-cutfem/fs-conduction via bridge (gated) | Survey epochs, depletion, anthropogenic bodies, GC-as-observation, reconciliation |

---
## 5. Alien-artifact compilation

**Scaffold.** Goal: one auditable belief from which every subsurface number is derived. Constraints: Rust, closed universe, no unsafe, consume-never-fork, software never signs. Failure-cost asymmetry: false certainty in disclosure/capital/dam ≫ compute cost. Observables now: synthetic truth, public benchmarks; later GC and plant reconciliation. Budget: resource update ≤ 24 h, GC update ≤ 1 h (TARGET).

### 5.1 EV matrix (Impact × Confidence / Effort, 1–5 each; natural fits only)

| Candidate | I | C | E | Score | Keep? | Note |
|---|---|---|---|---|---|---|
| Keyed-RNG regeneration + conditioning by kriging (CRN) | 5 | 5 | 2 | 12.5 | ✔ | Scale + incremental update + paired VOI audits in one mechanism |
| Interval-arithmetic divergence-theorem volume enclosure | 3 | 5 | 1 | 15.0 | ✔ | Replaces unusable `volume_certified` (Fact 4) |
| Merkle-tiled content addressing of projections | 4 | 5 | 2 | 10.0 | ✔ | Snapshot identity, model diff, dirty-brick recompute |
| Support-sheaf gluing as mass-balance witnesses (ref 03, simplest level) | 4 | 4 | 2 | 8.0 | ✔ | Only level 1–2 (overlap consistency); no spectral sequences |
| Importance-reweighted hyperparameter particles + ESS guard (ref 38) | 4 | 4 | 2 | 8.0 | ✔ | Variogram uncertainty without full MCMC per update |
| Anytime-valid e-processes for F-factor drift / surprise (lift fs-eproc) | 4 | 4 | 2 | 8.0 | ✔ | Optional stopping is the real reconciliation regime |
| PIT / interval-coverage calibration of panel forecasts | 4 | 4 | 2 | 8.0 | ✔ | Gate G3; JORC-draft reconciliation tailwind |
| Simulated-GC change of support (information effect) | 5 | 4 | 3 | 6.7 | ✔ | The decisive correction to H-a |
| Multivariate anamorphosis + PPMT/MAF decorrelation | 4 | 4 | 3 | 5.3 | ✔ | Needed for Au–As, Cu ratios |
| SPDE/GMRF on coarse meshes (ref 34: sparse Cholesky) | 4 | 4 | 4 | 4.0 | ✔ (coarse only) | Hyperparameter likelihoods, non-stationarity |
| Partition-of-unity RBF / dual kriging implicit modelling | 4 | 3 | 3 | 4.0 | ✔ | Structural L1 (GemPy/LoopStructural parity) |
| Low-rank (randomized SVD) ensemble compression | 3 | 3 | 3 | 3.0 | ✘ v1 | Summaries + regeneration already solve storage |
| Joint MCMC over implicit structure (GemPy-style) | 4 | 2 | 5 | 1.6 | ✘ v1 | Phase 2 research bet; enumerated hypotheses first |
| Optimal-transport realization selection | 2 | 3 | 3 | 2.0 | ✘ | Ranking/selecting realizations destroys calibration |
| Persistent homology of grade connectivity | 2 | 2 | 3 | 1.3 | ✘ | Union-find connected components suffice (checked null) |
| MPS with training images | 2 | 2 | 4 | 1.0 | ✘ v1 | Later as a KU-s hypothesis generator |

Families retained (5, within budget for a >100K-LOC safety-relevant system): **(A)** Gaussian-field numerics (34/38), **(B)** change-of-support decision layer, **(C)** local-to-global consistency (03, level 1–2), **(D)** sequential validity and calibration (02/eproc), **(E)** certified geometry + provenance. Timescale separation (ref 25): D monitors at period cadence; A reweights at drilling cadence; C runs per reconciliation period; no two controllers act on the same parameter.

### 5.2 Type sketches (DESIGN; Rust, stable, `#![forbid(unsafe_code)]`)

```rust
pub struct SpecHash([u8; 32]);                       // blake3 over canonical bytes
pub struct EvidenceCut { record_seq: u64, valid_time: TimeInterval,
                         qaqc_policy: PolicyHash, exclusions: Vec<ExclusionActId> }
pub struct BeliefSpec {
    cut: EvidenceCut, seed_root: SeedRoot,
    structure: StructuralModelRef,                   // L1, attested
    hypotheses: Vec<(HypothesisId, Weight)>, catch_all: Weight,
    partitions: Vec<PartitionSpec>,                  // grade, oxidation, geomet, geotech, hydro
    fields: Vec<FieldSpec>, transfers: Vec<CardRef>,
}
pub enum Additivity { Extensive, MassWeighted, VolumeWeighted,
                      NonAdditive { blend: BlendRuleId }, Ordinal, Categorical }
pub struct AttributeSpec { id: AttrId, unit: UnitId, measurand: MeasurandId,
                           additivity: Additivity, censoring: Option<DetectionLimitModel> }
pub struct FieldSpec {
    partition: PartitionId, domain: DomainId, vars: Vec<AttributeSpec>,
    anamorphosis: AnamorphosisRef, decorrelation: Decorrelation,     // None | Maf | Ppmt
    covariance: NestedAnisotropic, trend: Option<TrendSpec>,
    hyper: ParticleSet<Hyper>,                        // particles + log-weights
    meas_error: MeasErrorModelRef, bias_prior: BiasPriorRef,
}
pub enum Support { Point, Interval{len_m: f64}, Composite{len_m: f64},
                   Block{d: [f64;3]}, Smu{d: [f64;3]}, Panel(PanelId),
                   Stope(StopeId), Bench(BenchId), Parcel(ParcelId) }
pub enum Estimator { DeclaredOk(OkParams), Sk, Etype, Quantiles(Vec<f64>),
                     ProbAbove(Vec<Cutoff>), Realization(u32),
                     SimulatedGc { pattern: GcPattern, dig: DigRule }, Luc, Uc, Dgm }
pub struct ProjectionRequest { spec: SpecHash, frame: GridFrame, support: Support,
                               estimator: Estimator, realizations: RealizationSet,
                               attrs: Vec<AttrId>, region: RegionSel, det: DetClass }
pub struct ProjectionReceipt {
    request: RequestHash, spec: SpecHash, code: CodeVersion, det: DetClass,
    merkle_root: [u8; 32], conservation: ConservationWitness,
    validation: ValidationStats,       // swath, NN-vs-estimate bias, KE, slope, var. reproduction
    color: Color, staleness: Staleness, mnpi: MnpiLabel,
}
pub enum Staleness { Fresh, Stale { since: EventSeq, cause: StaleCause }, Superseded(RequestHash) }
```

### 5.3 Certificates (DESIGN)

| Certificate | Content | Verifiable by |
|---|---|---|
| `TonnageGradeCertificate` | region, support, cut-off spec; tonnes and metal per element as distributions (P10/P50/P90 + MC half-width); grade derived as metal/tonnes; `procedure`: regeneration hash OK, conservation OK, optional volume enclosure; `calibration` flag (uncalibrated / calibrated(period set) / miscalibrated(correction)); colour | `fm-check` replay from snapshot |
| `VolumeEnclosure` | `[lo, hi]` from interval divergence theorem on an fs-topo-certified closed mesh | Interval re-evaluation; `Certified<f64>` admissible (Enclosure kind) |
| `ConservationWitness` | Σtonnes, Σmetalₑ before/after every support change; max relative residual | Recompute |
| `RegenerationWitness` | per-brick blake3 of regenerated bytes; DetClass | Regenerate on reference ISA |
| `ClassificationSupportPack` | per block/volume: KE, slope of regression, drill spacing vs range, P(±15% over a quarterly volume at 90%) [U heuristic, R7 §4.2], AOA distance | Evidence **for** CP act; never a class label |
| `ReconciliationWitness` | per period/domain: forecast distribution (registered pre-outcome), outcome, PIT value, e-process state | Forecast ledger |

### 5.4 Guard table (deterministic, typed refusals with fixes)

| ID | Condition | Action |
|---|---|---|
| G1 | Aggregating `NonAdditive`/`Ordinal` attribute across supports without blend rule | Refuse `NonAdditiveAggregation{attr, fix}` |
| G2 | Mixing supports without declared operator | Refuse `SupportMismatch` |
| G3 | Mixing grid frames | Refuse `FrameMismatch` (fs-rep-voxel pattern) |
| G4 | Conservation residual > 1e-9 relative (Extensive attrs) | Refuse publish |
| G5 | Regenerated brick hash ≠ receipt | Refuse; mark artifact `Corrupt`; re-materialize on reference ISA |
| G6 | QA/QC invalidation or new evidence in region after cut | Mark `Stale`; block use in disclosure packages |
| G7 | Block beyond k × range from data (AOA) | Label `Extrapolated`; exclude from classification-support metrics |
| G8 | Particle ESS < 0.3·N | Flag `HyperDegenerate`; schedule refit |
| G9 | Intercept contradicts interpretation beyond tolerance | Surprise event; volume `Conflicted`; human re-interpretation |
| G10 | Below-detection values substituted by constants | Refuse; require censored model |
| G11 | Any path setting `reserve_disclosure_claim` / `geotechnical_signoff` | Absent from API (E023) |
| G12 | MNPI-labelled evidence in a projection | Projection inherits MNPI label; export refused without release event (E023) |

**Monitors:** e-process per (domain, period) on F1/F2 log-ratios for tonnes and metal (lift fs-eproc); rolling PIT/coverage of panel forecasts; posterior-predictive e-value of each new composite against belief@T⁻ (surprise); ESS per FieldSpec; staleness heat map.

### 5.5 Proof obligations (ref 15 template, compressed)

| PO | Statement | Verification | Failure → mitigation |
|---|---|---|---|
| PO-1 Regeneration determinism | Same (spec, request, code, DetClass) ⇒ bit-identical bricks | Golden hashes across runs/machines (same ISA); cross-ISA if all transcendentals via `det` | G5; reference-ISA rebuild |
| PO-2 Conditioning | Zero-nugget realizations honour data exactly; with error model, residuals ~ declared error | Unit + statistical tests | Refuse spec |
| PO-3 Conservation | Extensive totals invariant under every support change | G4 witness | Refuse publish |
| PO-4 Dispersion | Variance of regularized realizations ≈ γ̄(D,D) − γ̄(v,v) (Krige's relation [U]) | MC test with tolerance from #realizations | Flag covariance/regularization bug |
| PO-5 Parity | `DeclaredOk` = reference OK (GSLIB kt3d-class [U]) within 1e-6 rel. | Oracle harness | Block QP pack |
| PO-6 Calibration | On synthetic truth, nominal 80% panel intervals cover 0.72–0.88 | Gate G3 | Downgrade to "uncalibrated", H-a fallback |
| PO-7 Incremental equivalence | Incremental update ≡ from-scratch (bitwise with identical neighbourhoods); indices preserved | Replay test | Force full recompute |
| PO-8 Volume enclosure | True volume ∈ [lo, hi] for certified closed mesh | Analytic shapes | Refuse certificate |
| PO-9 No laundering | colour(projection) ≤ min colour(inputs) | Type-level + property test | — |
| PO-10 Staleness monotone | Fresh → Stale → Superseded only | State-machine test | — |
| PO-11 Reweighting | Self-normalized importance weights consistent; ESS guard enforced | Known-posterior test (conjugate case) | Refit |

### 5.6 Assumptions ledger

| ID | Assumption | Class | Runtime check |
|---|---|---|---|
| A-1 | Multi-Gaussianity after anamorphosis within a domain | distributional | Bivariate h-scatter / indicator-variogram checks; fallback indicator/mixture fields |
| A-2 | Stationarity within domain (or declared trend) | structural | Swath plots; trend residual tests |
| A-3 | Domain boundaries hard/soft as declared | structural | Contact analysis across boundaries |
| A-4 | Staged ("cut") inference: grade data does not update structure except via hypothesis weights | structural | Coverage gap vs joint inference on synthetic (Phase 2) |
| A-5 | Measurement-error model from QA/QC (FSE + analytical + method bias) | distributional | Duplicate-pair variance vs model (R7 §3.2) |
| A-6 | Density field adequate (sparse determinations + regression card) | distributional | Density coverage map; tonnage-variance attribution |
| A-7 | Blasthole measurand bias modelled | distributional | Paired BH–RC/DD studies |
| A-8 | Transfer cards valid in their boxes | computational | `NoClaimInDomain` refusal outside box |
| A-9 | Ergodic fluctuation acceptable for domain size | computational | Realization-statistics spread vs target |
| A-10 | Survey epochs registered within stated accuracy | operational | Control-point residuals |

### 5.7 Failure modes and deterministic conservative fallbacks

| Failure | Detection | Fallback (deterministic, conservative) |
|---|---|---|
| Simulation infeasible in budget | Budget exhaustion | Publish `DeclaredOk` + LUC/DGM only, labelled `UncertaintyUncharacterized`; no intervals shown |
| Hyperparameter inference degenerate | G8 | Ensemble over ≥3 QP-declared defensible variograms (equal weights), labelled `elicited` |
| Structural contradiction | G9 | Freeze last attested L1; affected volume `Conflicted`; excluded from classification-support |
| Calibration fails on reconciliation | e-process alarm | Widen by measured variance-inflation (explicit, visible claim); open hypothesis review; never silent F-multiplier |
| Regeneration mismatch | G5 | Refuse artifact; rebuild on reference ISA |
| frankensim spike fails | G0 | Lift-only; no bridge; FM wireframe extraction and volumes |
| Non-additive attribute requested at wrong support | G1 | Refuse with fix |

### 5.8 Verification plan

1. **Known-answer oracles:** closed-form SK/OK for small configurations; dispersion variance identities; GP ≡ SK vs `fs-bo::Gp` (oracle); small-n unconditional covariance reproduction vs `fs-uq::KlExpansion`; analytic volumes (sphere/box meshes); minimum-curvature desurvey on textbook examples [U].
2. **Synthetic-truth mine** (R7 §9.5 #6): fine-scale truth with ≥3 domains, a faulted contact, Au–As correlation, coarse-gold nugget, oxide/fresh profile, density contrast; realistic drill patterns, FSE/analytical errors, injected QA/QC defects and blasthole bias; full chain to F-factors. Tests: PO-6 coverage, recovery of injected biases (±2%), PO-7 under staged drilling campaigns.
3. **Public datasets / tools as lab oracles only** (E015 closed universe): Walker Lake exhaustive data [U]; GSLIB kt3d/sgsim [U]; GemPy and LoopStructural reference models (R7 §2.5); OMF v2 files via `omf-rust` (E027) for I/O round-trips.
4. **Metamorphic tests:** rotation/translation invariance; unit change (g/t ↔ ppm) invariance; input-order permutation invariance; adding a datum beyond the neighbourhood leaves distant bricks bit-identical.
5. **Performance:** brick regeneration throughput; 10⁸-SMU end-to-end projection; memory under out-of-core paging.

### 5.9 Galaxy-brain cards

**Card 1 — Store or regenerate realizations?**
- Equation: `S_store = N_b · R · V · b`; `T_regen = N_b · R · c_node / P`.
- Substituted (illustrative): N_b = 10⁹, R = 100, V = 10 attrs, b = 4 B → **4 TB per model version**. Regenerating an SMU projection of N_b = 10⁸ at R = 100 with c_node = 2 µs (TB + kriging correction, TARGET) on P = 64 cores → 10¹⁰ × 2×10⁻⁶ / 64 ≈ **5.2 min**; at 10⁹ ≈ 52 min.
- Intuition: realizations are cheap to recreate and expensive to keep; store summaries (≤ 64 B/block) and seeds, regenerate the rest.
- Assumptions: c_node holds at scale; determinism class fixed; cache hit-rate modest.
- What would change it: c_node > ~50 µs (e.g. MPS), or > ~20 full-ensemble queries/day, or a regulator requiring retained realizations (OPEN Q4) → materialize at SMU support.

**Card 2 — Why the kriged SMU model misstates recoverables (conditional bias)**
- Equation (multi-Gaussian regression): `E[Z | Z* = z*] = m + b·(z* − m)`, with b the slope of regression.
- Substituted: m = 1.0 g/t; mean estimate of selected blocks E[Z*|Z*>c] = 2.0 g/t. b = 0.8 → 1.8 g/t expected (−10%); b = 0.1 (LUC case, E029) → **1.1 g/t (−45%)**.
- Intuition: selecting on an estimate selects its errors; the plan expects 2.0 and the mill gets less.
- Assumptions: linear regression of truth on estimate (holds under multi-Gaussianity), unbiased global mean.
- What would change it: measured slope ≥ 0.95 against GC truth in a domain → `DeclaredOk` at SMU is adequate there, and simulation buys little.

**Card 3 — Common random numbers make "did the new drilling change the decision?" answerable**
- Equation: `Var(X_new − X_old) = Var(X_new) + Var(X_old) − 2ρ·σ_new·σ_old`.
- Substituted: σ_new = σ_old = 10 koz panel metal; independent seeds (ρ = 0) → SD of difference 14.1 koz; CRN with ρ = 0.95 → **3.2 koz**. Detecting a 5 koz shift needs ~20× fewer realizations with CRN.
- Intuition: same seeds, same "geological dice"; only the data-driven correction differs, so paired differences are sharp.
- Assumptions: conditioning by kriging with fixed unconditional fields; hyperparameters unchanged (else ρ falls).
- What would change it: an interpretation change (L1) or a large hyperparameter shift decorrelates realizations → report as a structural update, not a paired delta.

---
## 6. Plan-ready material

### 6.1 Proposed crates (DESIGN)

| Crate | Responsibility | Lifts / deps |
|---|---|---|
| `fm-units` | Stable-Rust dimensions + mining units (t, kt, Mt, troy oz, g/t, ppm, ppb, %, kWh/t, day/year), wet/dry and in-situ/loose as semantic tags | Pattern fs-qty |
| `fm-frame` | Mine grids, rotated block frames, minimal EPSG subset, versioned transforms (CRS gap E010) | — |
| `fm-evidence-core` | Evidence/Certified/Color/ModelCard/refusals | Lift fs-evidence (QuantitySpec shim), fs-blake3 |
| `fm-drill` | Collar/survey/interval/sample schema, minimum-curvature desurvey with survey-error propagation, domain-respecting compositing, measurand registry, QA/QC states + invalidation cascade | — |
| `fm-struct` | Implicit potential fields (dual kriging / partition-of-unity RBF), fault chronology, multiple partitions, hypothesis set + catch-all, `AttestedInterpretation` (legacy wireframes via winding) | Lift winding.rs |
| `fm-field` | Variography, anamorphosis, PPMT/MAF, nested anisotropic covariances, SK/OK/UK/co-kriging/block kriging, TB/FFT-MA/SGS, conditioning by kriging, coarse SPDE, hyperparameter particles, censored likelihood | Lift fs-rand, fs-sparse; `fsci-fft`, `fsci-spatial` KDTree |
| `fm-blockgrid` | Sparse brick topology + columnar attribute chunks, sub-blocking, Merkle tiles, out-of-core paging | Lift vdb.rs |
| `fm-project` | Projection engine, change-of-support operators, simulated GC, guard table, receipts, staleness | — |
| `fm-volcert` | Interval volume enclosures, mesh validity checks, block-proportion enclosures | Lift fs-ivl |
| `fm-geomet` | Additivity declarations, blend rules, transfer/correlation cards | Pattern CorrelationCard |
| `fm-asmined` | Survey epochs, depletion masks, design vs as-mined, anthropogenic bodies | Pattern fs-asbuilt |
| `fm-recon-monitor` | Forecast registration, PIT/coverage, F-factor e-processes, bias-hyperprior updates (state estimation itself: PQ-3/PQ-4) | Lift fs-eproc, EnSRF |
| `fm-matpack` | Geomaterial packs in `frankensim.matdb-manifest.v1`, sealed corpora | Pattern fs-matdb(-store) |
| `fm-fsim-bridge` (optional) | `fs_geom::Chart` adapters for solvers | Wrap, gated by G0 |
| `fm-geostat-oracles` (dev-only) | Parity harness and benchmark registry | Pattern fs-vvreg |

### 6.2 Durable formats (DESIGN)

- `fm.beliefspec.v1`: canonical CBOR/JSON with sorted keys, blake3 `SpecHash`; references evidence by `EvidenceCut`, not copies.
- `fm.brick.v1`: per-brick (8³ default; 32³ super-brick for paging) columnar chunks; per-attribute codec (f64, f32, f16, log-quantized u16 for grades with declared max quantization error, dictionary u16 for categories); brick hash = blake3(codec header ‖ bytes); projection Merkle root.
- `fm.receipt.v1`: `ProjectionReceipt` canonical JSON.
- Exchange (projections only): OMF v2 via `omf-rust` (MIT, E027) for block models/wireframes; CSV block model; vendor formats via lab-only converters.
- Storage authority mapping (event log in fgdb vs fsqlite, R-tree index) defers to PQ-4 (E013, E014, E016).

### 6.3 Gates (TARGET thresholds; kill gates marked ✖)

| Gate | When | Pass criterion | On fail |
|---|---|---|---|
| ✖ G0 frankensim spike | M2 | Separate clone at pinned commit; scratch crate depending on fs-rep-mesh + fs-topo + fs-sparse builds on the pinned nightly with one unified asupersync; one `Chart` round-trip from an FM field | Kill the bridge; lift-only (no impact on the resource path) |
| G1 parity | M4 | `DeclaredOk` vs reference OK: estimates and kriging variances ≤ 1e-6 relative; desurvey ≤ 1 mm per 1,000 m vs reference; compositing identical intervals | Block QP pack until fixed |
| G2 regeneration + throughput | M6 | Bit-identical bricks across 3 runs × 2 machines (same ISA); 10⁷ blocks × 100 conditional realizations ≤ 1 h on 64 cores; summaries ≤ 64 B/block | Optimize; if > 4× over, materialize SMU ensembles for active zones only |
| ✖ G3 calibration | M8 | Synthetic-truth mine: 80% panel-metal intervals cover 0.72–0.88 over ≥ 50 panels; injected 5% F2 bias recovered within ±2%; injected 8% resource bias reduced to < 2% after 12 periods | Ship only `DeclaredOk` + ensembles labelled "uncalibrated"; no probabilistic claims in management views |
| G4 scale + incremental | M10 | 10⁸ SMU blocks end-to-end with out-of-core paging, peak RAM ≤ 50% of host; +100 holes recomputes ≤ 5% of bricks and satisfies PO-7 | Restrict v1 to ≤ 10⁷ blocks per domain-region |
| ✖ G5 QP replay | M12 | An independent QP reproduces `DeclaredOk`, the validation pack and the TonnageGradeCertificate from the snapshot triple alone; ≤ 1 day of effort | Auditability failure → H-c loses its falsifier test; revert to H-a authority with H-c as advisory |

### 6.4 Risks

1. **QP acceptance / "Bayesian black box"** (loss #3): mitigated by parity mode and the standard validation pack; the probabilistic layer is evidence *for* the CP, not a replacement.
2. **Structural uncertainty is the dominant term and the hardest to infer** (Bond et al. 2007): v1 relies on enumerated hypotheses with elicited weights + catch-all; joint inference is a Phase-2 research bet (one bet per lane, D12).
3. **Multi-Gaussian failure in coarse-gold domains:** needs indicator or mixture fields; detect via A-1 checks.
4. **Scale numbers are unbenchmarked** (c_node, brick throughput): G2/G4 are the measurement.
5. **frankensim drift and nightly coupling** (E001, Fact 6): lifted files need golden hashes; the resource path depends on no frankensim crate.
6. **Determinism vs performance:** cross-ISA bitwise reproduction requires `det` transcendentals everywhere (Fact 7 shows lifted code may not comply).
7. **Data reality** (MOR unknowns): legacy databases with mixed assay methods, missing surveys and unrecorded capping decisions will dominate early effort.
8. **Scope creep / LOC ceiling** (E022): `fm-field` is the largest risk to the budget; set a LOC ceiling and a delivery log separate from the plan from day one.

### 6.5 OPEN questions

- **Q1** Sub-blocking scheme Barrick sites use (octree vs per-parent variable sub-cells), and required interoperability.
- **Q2** Will QPs sign the `DeclaredOk` projection, the simulation e-type, or both side by side? (Codes do not prescribe; company policy does.)
- **Q3** Which GC models at NGM-class operations are produced in other software and must be ingested as observations vs regenerated?
- **Q4** Do regulators or auditors require *retained* realizations rather than regenerable ones? (If yes, card 1 flips for disclosed snapshots only.)
- **Q5** Operational authority under several live hypotheses: dispatch needs one dig line — the decision rule (expected-loss dig lines, R7 §6.2) belongs to PQ-3; the representation must expose per-hypothesis projections.
- **Q6** Licensing: lifted frankensim files (MIT + rider) and OMF dependency in a client-facing product.
- **Q7** How much does the staged "cut" understate uncertainty on realistic deposits? Measurable only on synthetic truth with known structure.
- **Q8** Minimum geomet sampling density for transfer cards to be better than domain averages.

---

## 7. Evidence index (case-file IDs used)

E001–E006 (frankensim state, consumption, geometry coupling, representation and UQ crates), E008–E010 (evidence types, matdb, gaps), E011 (two-Cx risk), E013–E016 (storage substrates), E022–E023 (claim discipline, refusals, MNPI), E026–E027 (economics, OMF), E028–E033 (prospectivity interface, geostatistics and reconciliation, planning, codes, epistemics, knowledge states). Code citations are to `/Users/jemanuel/projects/frankensim/crates/...` at `20463525d`; all are read-only observations.
