# B2 — Prospectivity (PQ-2) and Epistemics (PQ-5): independent Brenner-style hypothesis pass

RUN_ID: fm-2026-10-28-a · Pass: B2 · Date: 2026-10-28 · Case file: `planning/mor/MOR_CASE.md` (E### IDs).

**Inputs read:**
- `planning/research/R7_technical_methods_sota.md` (all of it);
- `planning/research/R6_industry_landscape.md` §0, §4.4–4.7, §5–7;
- `~/.claude/skills/alien-artifact-coding/` SKILL plus references 00, TECHNIQUES, 02, 10, 15, 16, 18, 21, 23, GALAXY-BRAIN, FAILURE-CASES;
- `franken_surveillance_system/COMPREHENSIVE_PLAN_FOR_FRANKEN_SURVEILLANCE_SYSTEM.md` §§4.3, 4.5, 17.4–17.5, 20.4–20.5, 20.9, 23.8;
- sibling code (read-only): `frankensim/crates/{fs-evidence,fs-voi,fs-eproc,fs-oed-e2e,fs-bo,fs-robust,fs-regime,fs-sparse}`, `asupersync/{franken_evidence,franken_decision}`, `frankenscipy/crates/fsci-stats`.

**Labels:**

| Label | Meaning |
|---|---|
| FACT[V] | I checked it in a file and cite the path. |
| FACT[R7] | Verified literature claim carried from R7. |
| FACT[U] | From my background knowledge; verify before normative use. |
| DESIGN | Proposed design. |
| HYPOTHESIS | Testable claim. |
| TARGET | Gate threshold. |
| OPEN | Unresolved question. |

All numbers in galaxy-brain cards are **illustrative** unless they are tagged FACT.

---

## 0. Verdicts at a glance

| PQ | Hypothesis | Verdict | Role in the survivor |
|---|---|---|---|
| PQ-2 | (a) Pixel-wise ML prospectivity (RF/GBT/CNN) | **REFUTED as the object of record.** It has no probability semantics. It treats "unlabelled" as negative (SCAR), so it collapses all absence classes. It fails LOCO validation (E028; R7 §1.6). | **SURVIVES** as a *cross-fitted covariate supplier* and a proposal generator. |
| PQ-2 | (b) Hierarchical marked thinned Cox process (LGCP/SPDE, grade–tonnage marks, effort × detectability thinning, terrane partial pooling) | **SURVIVES** as the regional object of record, conditional on the effort-data gate G-P4. | Core of `fm-ppm`. |
| PQ-2 | (c) Mineral-systems causal DAG (source–pathway–trap–preservation) | **REFUTED standalone**: it has no calibration and no discovery-bias model. | **SURVIVES** as the *factorization of log-intensity* ("noisy-AND" prior). It is also the transportability argument for dissimilar terranes. |
| PQ-2 | (d1) Search-theoretic POMDP over competing geological hypotheses (Intelligent Prospector / Popper–Bayes) | **SURVIVES at prospect scale only** (≲10² km², ≲10² holes). It is intractable regionally. | Phase-2 `fm-pomdp`, fed by (b)'s local posterior as its prior. |
| PQ-2 | (d2) Hybrid: (a)+(c) → (b) → (d1), with typed search modes | **RECOMMENDED ARCHITECTURE.** | — |
| PQ-2 | N0: MapMark4-degenerate tract model (Poisson–Gamma count × G-T pdfs) | Checked null and baseline. | Parity oracle and deterministic fallback. |
| PQ-5 | (a) One coherent Bayesian posterior everywhere | **REFUTED globally.** Under M-open misspecification it concentrates on the wrong model, and coverage decays as data grow. It cannot represent CP/QP/controller judgment. | **SURVIVES** *inside* models (layer L1). |
| PQ-5 | (b) Imprecise probability / credal sets / Dempster–Shafer | **REFUTED as the stored representation.** It has dilation, the DS conflict paradox, no strictly proper scoring and poor executive legibility. | **SURVIVES narrowly** as the ε-contamination *decision envelope* for irreversible actions, and as Manski bounds for missing effort. |
| PQ-5 | (c) Layered epistemic stack (Bayes in models → stacking + catch-all → reference-class discrepancy → UU budget → e-process surprise → conformal where exchangeable → knowledge-state lattice) | **SURVIVES (winner).** | `fm-claim`, `fm-calib`, `fm-surprise`, `fm-refclass`, `fm-epimap`. |
| PQ-5 | (d) Forced alternative: info-gap robust satisficing ("state flip margins, never probabilities, for UU") plus calibration-only trust (track record) | **REFUTED as the whole system** (no probabilities on KU, slow feedback). | **ABSORBED.** Flip margins are the *third admissible UU form*. fs-evidence already implements them (§1). |

---

## 1. Verified substrate: what exists to consume (FACT[V] unless marked)

| Asset | Path | What it gives FrankenMining | Gap / caution |
|---|---|---|---|
| `fs_evidence::color::Color::{Verified{lo,hi}, Validated{regime,dataset}, Estimated{estimator,dispersion}}` | `frankensim/crates/fs-evidence/src/color.rs:209` | Three colors with conservative `compose` (`color.rs:848`). It is "an estimate can never be laundered into a certificate". Validation is *regional* and auto-demotes on regime exit (`check_regime`, `color.rs:974`). | Three colors are too coarse for R7's eight evidence classes. FM keeps its own `EvidenceClass` and *projects* to `Color` (§3.7). |
| `TermValue::Unknown{reason}`, `UnknownPlausibilityBound{kind,maximum_abs_effect,provenance}`, `FlippingUnknown{required_magnitude, bound: Unbounded\|Bounded}`, `ComplianceVerdict::{Compliant,NonCompliant,Indeterminate}` | `fs-evidence/src/uncertainty.rs:444,788,838,847,892` | **The "what would change our mind" primitive already exists.** It gives the smallest effect from an unknown source that flips a verdict, and the plausibility bound must carry provenance. This is info-gap robustness, implemented. | `EngineeringUncertaintyKind` is fixed at eight engineering kinds and is `#[non_exhaustive]`. Mining needs a mapping (§3.7). |
| `falsify.rs` `DOUBT_FLOOR = 0.05`, `DOUBT_COLD_START = 1.0` | `fs-evidence/src/falsify.rs:23,27` | Precedent for "doubt never reaches zero". It is the analogue of a catch-all floor. | Catalog only; not release authority (its own doc). |
| `useful_bound::NoUsefulBoundCause::{HorizonTooLong,LipschitzBlowup,DomainExit,BudgetExhausted}` | `fs-evidence/src/useful_bound.rs:18` | Typed "valid but useless" refusals. | — |
| `fs_voi::{expected_opportunity_loss, ranking_flip_probability, recommend, heuristic_choice, recommend_unknown_resolutions}` | `fs-voi/src/lib.rs:153–698` | Decision-relevant VOI. It values a design only if measuring it could *flip* the decision. It also has a baseline that VOI must beat. | **Gaussian closed form** (in-house normal CDF, lib.rs header). That is unfit for heavy-tailed, discrete exploration outcomes. FM needs Monte Carlo preposterior VOI (§2.8). |
| `fs_eproc::{BettingEProcess, PairwiseRace, LossSpan, GaussianMixtureCs, bernoulli::BernoulliMixtureCs, e_benjamini_hochberg}` | `fs-eproc/src/lib.rs:60–483`, `bernoulli.rs:68` | Anytime-valid monitors (Ville). `BernoulliMixtureCs` is a confidence sequence for coverage rates. `PairwiseRace` races models. | `BettingEProcess` takes outcomes in [0,1] only. `PairwiseRace` **refuses out-of-span losses**. So an unbounded log score cannot race: use Brier/CRPS-normalized or a floored log score (DESIGN, §3.8). |
| `fs_robust::{cvar, empirical_cvar, weakest_color, ColoredObjective::headline_color, robust_optimum}` | `fs-robust/src/lib.rs:124–371` | "Headline color = weakest input color". "No optimization against an un-colored objective". CVaR robust optimum. | — |
| `fs_oed_e2e::run_campaign` | `fs-oed-e2e/src/lib.rs:3148` | Kalman-fused Gaussian candidates with EVPI-stopped sensing. | Depends on `fs-exec::Cx` and fs-assimilate (`Cargo.toml`). It is **not in the E003 liftable list** and uses the wrong belief shape for discovery search. Use as a pattern only. |
| `fs-sparse` supernodal multifrontal LDLᵀ, AMD ordering, `SymbolicLdlt::analyze` reusable across same pattern | `fs-sparse/src/direct.rs:1–20` | **The INLA-style kernel exists.** The Laplace iterations of an SPDE/GMRF reuse the symbolic factorization. The log-det comes from D. | No nested-dissection ordering and no selected inversion (Takahashi) for marginal variances. That makes it OPEN-4. |
| `franken_evidence::EvidenceLedger{posterior, expected_loss_by_action, chosen_expected_loss, calibration_score∈[0,1], fallback_active, top_features}` | `asupersync/franken_evidence/src/lib.rs:62` | Suite-mandated decision trace ("no forking allowed"). It has renderers at levels 0–3, plus HTML and markdown (`render.rs`). | It is a *decision* trace, not a claim. `calibration_score` is a free scalar, so FM must *define* it from the calibration ledger (§3.8). |
| `franken_decision::{DecisionContract, LossMatrix, Posterior::bayesian_update, FallbackPolicy{calibration_drift_threshold=0.7, e_process_breach_threshold=20, confidence_width_threshold=0.5}}` | `asupersync/franken_decision/src/lib.rs:247–916` | Discrete-state decisions with fallback. e > 20 ↔ α = 0.05 by Ville. | Discrete, finite states only. Fits hypothesis-set and stage-gate decisions, not spatial fields. |
| `fsci_stats::{NegBinomial, Lognormal, GenPareto, Pareto, TruncPareto, Zipf, Dirichlet, Poisson, CensoredData, brier_score, kstest, cramervonmises, anderson, SobolSampler}` | `frankenscipy/crates/fsci-stats/src/lib.rs:6361,15443,20223,…; censored.rs:15; qmc.rs:314` | All distribution families needed for marks (lognormal body + GPD tail), MapMark NB counts, censored detection limits and QMC. | No samplers (E018). HMC/NUTS, ESS and particle filters must be built (`fm-infer`). |

The FrankenSurveillance pattern was read directly (FACT[V], plan file):
- Evidence classes `assertion < derived < observed < corroborated < verified` sit on *edges or claims*, not objects (§4.3).
- `GOAL-006`/`INV-016`: degraded observability invalidates absence-of-detection evidence.
- Knowledge states (§20.5): `known/estimated/unknown/conflicted/stale/not_observable/redacted/indeterminate/not_applicable`. "`unknown` is not false; `not_observable` is not no detection; `redacted` is not absent".
- §17.4: "no naked point confidence".
- §20.4.1 WorldEnvelope: nominal / certified core / material alternative worlds / protected adversarial residuals.
- §20.9: probe selection by "expected decision-loss reduction per resource, not information volume".

---

## 2. PQ-2 — Bayesian spatio-temporal reasoning about undiscovered deposits

### 2.1 Decision framing

- **Decisions served:**
  - which ground to hold, acquire, earn into or drop;
  - which survey or drill program to fund next;
  - when to stop on a target;
  - how much undiscovered endowment backs a district's option value (PQ-7 interface).
- **Loss asymmetry (E-loss #1 vs #5).** A falsely "cleared" area (overconfident negative evidence) and a falsely "hot" area (overconfident analogue transfer) are both false certainty. Missed upside is the cheapest error. So *absolute* probabilities must be refused or bounded wherever effort or transfer is unidentified.
- **Base rates that bound every output:**
  - $218M per discovery (2011–20);
  - value per exploration dollar 0.69;
  - 2–3 Tier-1 discoveries per year globally;
  - 15.7–17.9 years from discovery to production (FACT[R6 §4.4–4.5], E026).
- A model that implies a materially better hit rate than these base rates, over a broad area, is presumptively miscalibrated until its hindcast says otherwise.

### 2.2 Hypotheses, mechanisms, falsifiers, cheap tests

**(a) Pixel-wise prospectivity ML** (RF/GBT/CNN; PU variants; R7 §1.4)

- **Mechanism.** A discriminative score s(x(cell)) is learned from evidence rasters, with known deposits as positives and random or "geologically constrained" pseudo-absences as negatives. It is then thresholded or ranked.
- **Falsifier.** In a temporal hindcast the score is a proxy for *past effort*:
  - partial dependence of s on the (withheld) effort field is strong;
  - capture efficiency of post-T discoveries in low-effort or deep-cover cells is ≤ random;
  - decile calibration of "probabilities" against post-T discovery rates fails.
  - A second falsifier: change-of-support. Halving the pixel size changes the implied per-area rate (Baddeley 2010, FACT[R7]).
- **Cheap test.** Yilgarn-style hindcast (§2.5) stratified by effort tertile × cover class. Prediction: (a) wins only in the high-effort/shallow stratum (local analogue matching; Oh et al. 2026, FACT[R7]).

**(b) Hierarchical marked thinned Cox process** (R7 §§1.5, 1.7, 1.8; E028)

- **Mechanism:**
  - true deposits of type k: a marked Cox process with log λ_k(s) = α_{k,τ(s)} + β_{k,τ}ᵀx(s) + w_k(s), with w_k an SPDE-Matérn field;
  - marks m = (tonnage, ilr-grade, depth, style);
  - discovery by time t is an independent thinning π(s,m,t) = 1 − exp(−Σ_j ∫ effort_j(s,τ) q_j(m, cover(s), τ) dτ);
  - observed = PPP(λπf), undiscovered = independent PPP(λ(1−π)f).
- **Falsifiers:**
  - (i) Simulation-based calibration (SBC) on a synthetic district fails: rank histograms non-uniform, or undiscovered-count coverage off by more than 3 pts.
  - (ii) In hindcast, (b) does not beat its own thinning ablation (b0) in low-effort strata. Then the effort data or q-model are not doing work.
  - (iii) Metamorphic: doubling barren effort raises λ.
  - (iv) (b) does not beat N0 on held-out tract counts.
- **Cheap test.** The same hindcast plus MapMark4 parity in degenerate mode.

**(c) Mineral-systems causal model** (McCuaig et al. 2010, FACT[R7])

- **Mechanism.** Latent ingredient fields I_i(s) ∈ {source, energy/driver, pathway, trap, preservation} are each informed by proxies. A deposit forms where they co-occur, so λ(s) = λ₀ Π_i p_i(s) and log λ = log λ₀ + Σ_i log p_i(s). This is a noisy-AND / product of experts.
- Cover affects *detection* (q) and *economics*, not existence. Preservation and erosion affect existence. The DAG forces this separation.
- **Falsifier.**
  - (i) d-separation: given pathway and trap proxies, a source proxy that should be screened off still predicts the deposits. Then an edge is missing or a proxy is mislabelled.
  - (ii) Noisy-AND loses to additive-logit on likelihood in-terrane *and* in LOCO.
  - (iii) Deposits occur where one ingredient's proxy is confidently absent. That means the conjunction is wrong or the proxy is.
- **Cheap test.** Likelihood comparison of the noisy-AND vs additive link inside (b), in-terrane vs leave-one-terrane-out. Transportability predicts that the mechanism-structured prior wins *more* under LOCO than in-terrane.

**(d1) Forced alternative: search-theoretic POMDP over competing hypotheses** (Mern & Caers 2023; Mern et al. 2024; Caers 2025 — FACT[R7 §1.10])

- **Mechanism.**
  - Belief over {conceptual hypotheses H ∪ max-entropy null h₀} × geometry × grade field.
  - Actions: drill(x), survey, MINE, ABANDON. Reward: economics minus cost.
  - Online POMCPOW or offline SARSOP.
  - h₀'s likelihood overtaking all named H is the "all hypotheses falsified" alarm.
- **Falsifier.** On ≥17 synthetic truths it does not beat myopic VOI-greedy by ≥20% fewer holes at equal go/no-go accuracy. Mern reports >2× vs grid, but grid is a weak baseline.
- **Cheap test.** Port the MIT-licensed v1 problem as a lab oracle (test-time only).
- **Scale ceiling.** Belief particles × actions × depth make it a prospect-scale tool.

**(d2) Hybrid (my construction).**
- (a) supplies learned covariates *cross-fitted* on disjoint labels, or self-supervised (no labels).
- (c) supplies the factorization of log λ and the transport argument.
- (b) is the object of record.
- (d1) consumes (b)'s local posterior as its prior at prospect scale.
- Two **typed** search modes govern negative-evidence updates (§2.8-D).

**N0 (checked null).** The MapMark4 configuration (FACT[R7 §1.2]):
- tract-level NB count from expert N90/N50/N10;
- lognormal or KDE tonnage;
- compositional grades;
- no spatial field, no thinning.

### 2.3 Handling matrix (what each hypothesis does with the eight hard issues)

| Issue | (a) pixel ML | (b) thinned Cox | (c) MS causal DAG | (d1) POMDP | Survivor (d2) |
|---|---|---|---|---|---|
| **Negative evidence, 4 absence classes** | Collapses all four into "0" (SCAR). That is pitfall P1. | Through the thinning integral only. Not-looked → no effort term. Insensitive → q≈0. Sensitive-nothing → hazard Σe·q. Sub-economic → an *observed* point with a censored mark. | Not modelled. | Per-hole observation model inside the prospect. | (b), plus a **fifth class**: *effort suspected but unrecorded* → Manski bounds (§2.8, F1). |
| **Analog transfer / dissimilar terranes** | Copy-paste the model. LOCO drops (Oh 2026; Alfonso 2024). | θ_{k,τ} ~ GP over terrane descriptors; variance widens with dissimilarity; AOA flag. | Mechanism invariance gives the best *structural* transfer. | Hypothesis priors from analogues. | (b) hierarchy + (c) factorization. Absolute P is refused beyond the dissimilarity threshold (F3). |
| **Cover depth** | Covariate; confounded with effort. | Enters q_j(m,cover) (detection) and the economic filter, *not* λ. | Separates preservation (existence) from cover (detection). | Hole depth vs target depth. | (b)+(c): cover never suppresses λ. |
| **Deposit-type uncertainty** | One model per type. | Multitype process; occurrences carry a type-probability vector. | Type-specific ingredient sets. | Hypotheses = types/geometries. | Multitype plus a **novel-type intensity slot** with a Good–Turing UU budget (GB-9). |
| **Grade–tonnage marks** | None. | Hierarchical marks: lognormal body + GPD tail, compositional grades, resource-growth model, reporting truncation. | None. | Grade field in state. | (b). Tail model chosen by hindcast (P9). |
| **Data heterogeneity** | Rasterize everything (deterministic interpolation halves variance; Caers 2025). | Integrated likelihood: several observation processes share λ (Isaac 2020); covariates as *realizations*. | Proxies per ingredient with interpretation uncertainty. | Native multi-modal likelihoods. | (b) + covariate realizations + evidence-class tags on every input. |
| **MNPI** | Model weights trained on drill results are MNPI (membership leakage). | A posterior conditioned on MNPI is MNPI. | Priors are usually public. | Beliefs are MNPI. | **Dual-basis posterior** (public-only vs full) plus monotone taint (§2.8, PO-12). |
| **Computational scale** | Trivially parallel. | 2D SPDE with 10⁶ nodes is feasible with sparse LDLᵀ (fs-sparse). 3D under cover is district-only (OPEN-3). | Cheap. | Prospect-scale only. | Continental 2D coarse field + nested district meshes. |

### 2.4 Refuter-first: the strongest attacks on the survivor

1. **"Effort data do not exist, so (b) is a beautiful machine with no fuel."** (R7 §11 O1; Hronsky & Kreuzer 2019.)
   - *Response:* this is the decisive risk. G-P4 and kill gate K2.
   - Without effort, (b) must *refuse* absolute negative-evidence updates and emit bounds. The product then becomes "rank the digitization of unknown knowns by VOI" (§3.10). That is still valuable, because legacy reports are the cheapest information in mining.
2. **"Effort is endogenous: explorers looked where they saw signs not in your covariates."**
   - *Response:* correct, and absent from R7. The thinned likelihood is valid only if effort is **ignorable**, meaning it depends on recorded information only (Rubin 1976 logic; FACT[U]).
   - If prospectors used unrecorded favourability, then unexplored ground was *judged* poorer, and (b)'s prior there is too high.
   - Test (A2): add lagged effort as a covariate of λ in hindcast. If it predicts post-T discoveries beyond the covariates, effort is non-ignorable. Then model effort jointly as a preferential-sampling process (Diggle et al. 2010, FACT[U]) or widen with Manski bounds.
3. **"λ and q are not separately identified."**
   - *Response:* partly true. Three things identify them:
     - each discovered deposit records its **discovery method**, which identifies the *relative* q_j as competing hazards;
     - forward-modelled signatures (an EM response vs depth) bound the q_j;
     - hindcasts calibrate the scale.
   - What remains goes into the hierarchical prior on q and is reported as a KU-p driver ("detection efficiency dominates this cell").
4. **"Ecology analogies break: deposits are huge-tailed and clustered."**
   - *Response:* clustering lives in w(s) (LGCP). The tail lives in the GPD mark. Inhibition below ~2 km is handled by the versioned aggregation rule (R7 §1.2).
5. **"The heavy tail makes every expectation unstable."**
   - *Response:* yes. Report P(≥ Tier-1 size) and quantiles alongside means, and never value with medians (GB-5).

### 2.5 Discriminating experiment (cheap, one district, one quarter of effort)

**Design (DESIGN):**
- **District.** A mature public district with dated deposits and open-file effort. Candidates: Yilgarn (Guj et al. 2011 precedent, FACT[R7]); Abitibi; Nevada. Data licensing is OPEN.
- **Freeze.** Evidence and labels as known at T (bitemporal "as-of" query). Score discoveries in (T, T+20 yr].
- **Models:** (a) RF with pseudo-absences; (a′) bagging-PU; (b) full; (b0) = (b) without thinning; (c)-structured (b); N0.
- **Scores:**
  - point-process log score of held-out discoveries;
  - PR-AUC on cells;
  - capture efficiency at the top 1%, 5% and 10% of area;
  - expected-vs-observed discoveries per intensity decile;
  - 80% coverage of tract counts.
- **Validation splits:** blocked CV with block size from the variogram range, LOCO, and temporal.

| Prediction if (b)/(d2) is right | If instead… | …then |
|---|---|---|
| (b) − (b0) log-score gain is concentrated in **low-effort / deep-cover** strata | Gain is uniform or zero | Thinning does no work. Check effort coverage before concluding anything about the model. |
| (a) ≥ (b) in high-effort/shallow cells; (b) > (a) in low-effort and LOCO cells | (a) wins everywhere, including LOCO | Kill gate K1: (b) is demoted to the counting layer. |
| (c)-structured gains more in LOCO than in-terrane | No difference | The MS factorization is ornamental. Keep flat covariates. |
| (b) decile calibration within the 80% band; N0 beaten on tract counts | (b) not better than N0 | (b) is an expensive way to say N0. Fall back to N0 + effort-adjusted tracts. |

### 2.6 Verdicts (PQ-2)

- (a) is refuted as the object of record and survives as a covariate.
- (b) survives, gated on G-P4.
- (c) survives as structure.
- (d1) survives at prospect scale (phase 2).
- **Recommended: (d2).**
- This is a **checked null on novelty**:
  - the thinned point process is R7 §1.7's proposal;
  - the POMDP is Mern/Caers.
- The *new* contributions here are:
  1. typed Cox vs single-target search modes;
  2. the fifth absence class with Manski bounds;
  3. the effort-ignorability test;
  4. cross-fitting discipline for ML covariates;
  5. the dual-basis MNPI posterior;
  6. the Ellefsen-derived hyperprior scale (GB-7).

### 2.7 Survivor specification: the generative model (DESIGN)

```text
Types         k ∈ K_named ∪ {k*}       k* = "novel/unclassified type" slot (UU, GB-9)
Terranes      τ(s) from a versioned tract map (each version is a Claim; map uncertainty = KU-s alternatives)
Intensity     log λ_k(s) = α_{k,τ} + Σ_i log p_{k,i}(s) + w_k(s)
                  p_{k,i}(s) = logistic(γ_{k,i,τ}ᵀ z_i(s))   ingredient i's proxies z_i (c)
                  z_i may include cross-fitted ML scores from (a), stored as realizations, never as labels
                  w_k ~ SPDE-Matérn(range ρ_k, sd σ_k), mesh M (2D continental, nested 2D/3D district)
Hierarchy     θ_{k,τ} = (α, γ) ~ N(θ_k + GP_terrane(d(τ,τ′)), Σ_k)      σ_τ hyperprior ≈ 1–1.5 (GB-7)
Marks         log T ~ spliced{ lognormal body | GPD tail above u_k }, ilr(G) | T ~ N(·), depth z, style
              observed tonnage = true × growth(era, age) (Golden Mile 36→72 Moz, FACT[R7]); truncated below report threshold
Thinning      π(s,m,t) = 1 − exp(−H(s,m,t)),  H = Σ_j ∫_0^t e_j(s,τ) q_j(m, cover(s), τ) dτ
              q_j hierarchical: q_j = q̄_j(era) · g_j(footprint(T), cover/DOI_j); discovery-method labels → competing hazards
Observation   (1) deposit catalogue (presence-only, thinned, marked)
              (2) drill intercepts with assays (presence/absence at known support, prospect scale)
              (3) geochem/geophys as proxy realizations z_i^{(r)}, r=1..R, from stochastic interpolation/inversion
              (4) elicited N90/N50/N10 per tract as a pseudo-likelihood on ∫λ (enables MapMark parity)
Likelihood    ℓ = Σ_obs log[λ π f] − ∫∫ λ π f dm ds  (+ intercept and elicitation terms)  — the integral is where absence lives
Inference     Laplace/INLA-lite: Newton on latent field per hyperparameter point (log-concave, PO-4), fs-sparse LDLᵀ
              with symbolic reuse; hyperparameters via CCD/grid; covariate uncertainty by mixing over r=1..R;
              HMC fallback on district scale (fm-infer); SBC mandatory (PO-7)
Posterior     IntensityPosterior{basis: Public | Full}: samples of (θ, w at mesh nodes, q, mark params)
```

**Effort record → absence class mapping** (DESIGN; extends R7 §1.7 with a fifth class):

| Absence class | Typed as | Enters likelihood as | Knowledge state | Example |
|---|---|---|---|---|
| A0 Not looked | no `EffortRecord` intersecting s | nothing | `not_observable` | Unmapped tenement under 60 m transported cover |
| A1 Looked, insensitive | `EffortRecord` with q_j(m,cover) ≤ q_min for that style/depth | ≈ nothing (a computed hazard, logged) | `not_observable` for that style/depth | Soil geochem over transported cover; pre-2008 ASTER SWIR (R7 §2.3) |
| A2 Looked, sensitive, nothing | hazard H > 0 | −∫λπ penalty, scaled by q | `estimated` (absence of detection, with π) | Close-spaced aircore through thin cover |
| A3 Found sub-economic | occurrence with censored mark | positive point plus a mark upper bound | `known` occurrence | Drill intercept 0.3 g/t over 20 m |
| A4 Effort suspected, unrecorded | `EffortRecord{coverage_confidence: Low/Unknown}` or report-mention without geometry | **Manski bounds**: π ∈ [0, π_era-typical] | `unknown` (an unknown known) | Old company report cites "extensive RAB" with no collars |

### 2.8 Decision outputs and their compiled artifacts

Every output is a `Claim` (§3.6). Each carries a representation, evidence class, calibration flag, UU term, validity (price deck, economic filter version), basis (Public or Full) and confidentiality.

**(A) P(≥1 economic deposit in area A | E, t)**

```text
Λ_econ,undisc(A) = ∫_A ∫_{M_econ(deck)} λ(s) f(m|s) (1 − π(s,m,t)) dm ds
P(N ≥ 1) = 1 − E_post[ exp(−Λ_econ,undisc(A)) ]      ← posterior expectation OUTSIDE the exponential (GB-3)
```

- `M_econ(deck)` is an economic-mark filter: a minimum contained metal and grade at depth z under a declared price/cost/recovery deck. It is versioned, so the claim's validity names the deck.
- Output carries:
  - the P10/P50/P90 of P itself (from the hyperparameter posterior);
  - an AOA flag;
  - an A4 bound-pair when unknown-effort area > 10% of A;
  - the limiting ingredient argmin_i p_i (the explanation: "trap proxy is the binding constraint here");
  - the top uncertainty driver (ρ, q, θ_τ, mark tail).
- Artifact: `UndiscoveredSummary` (§4.2) plus a galaxy-brain card.
- **Refusal:** if dissimilarity(τ) > d_max or G-P4 fails → `Representation::Interval{kind: ManskiBounds}` or `Refused(NoClaimInDomain)`. Never a point P.

**(B) Distribution of undiscovered contained metal**
- A compound Cox sum Σ_{n∈N_undisc(A)} T_n·G_n, by Monte Carlo over posterior draws.
- Report P10/P50/P90, the mean, and P(at least one Tier-1-size deposit), stating *which* Tier-1 definition: Barrick vs MinEx, "do not mix" (FACT[R6 §1.3]).
- Express as an `Ensemble` claim indexed by shared realization IDs, so PQ-7 can join it with price paths (common random numbers).

**(C) EV and VOI of the next survey or drill program** (preposterior; Howard 1966; Eidsvik 2015)

```text
EV(a | E)   = E_post[ U(a, θ) ] ;   U includes lead-time discount (15.7–17.9 yr, E026), P(develop | discovered), deal terms
VOI(o | E)  = E_{y~p(y|E,o)} [ max_a E[U(a)|E,y] ] − max_a E[U(a)|E]          (≥ 0, PO-14)
net(o)      = VOI(o) − cost(o) − delay_cost(o)
y-model     = thinned-process prediction of survey/drill outcomes: detect / anomaly-false-positive / sub-economic / dry
```

- Estimator: nested MC with common random numbers, plus control variates (`fm-explore-voi`). It reports MC standard error.
- `VoiStatus::Indeterminate` when the sign of net(o) is not resolved at 2·SE. In that case it falls back to `fs_voi::heuristic_choice`-style uncertainty-proportional ranking, labelled as such.
- EOI (prior entropy minus expected posterior entropy; Caers et al. 2022) is shown as a *secondary, decision-agnostic* column, never as the ranking key. This follows FSS §20.9: "decision-loss reduction per resource, not information volume".

**(D) Bayesian search updates after negative drilling — two typed modes (DESIGN; refines R7 §1.7)**

| Mode | When valid | Update | Effect elsewhere |
|---|---|---|---|
| `SearchMode::Cox` | Unknown number of deposits (regional, district) | λ_post(s,m) = λ(s,m)·(1 − d_prog(s,m)) for the undiscovered part. Hyperparameters θ_τ are updated through the likelihood integral. | **No renormalization upward.** Elsewhere changes only via (i) θ_τ shrinking slightly (a shared-level effect) and (ii) hypothesis reweighting when a hypothesis predicted this spot. |
| `SearchMode::SingleTarget{anomaly}` | One causative body is asserted to exist (e.g., the source of a boulder train, a geochem dispersion fan, an EM conductor). This assertion is itself a Claim with P(exists). | Koopman: p′_i = p_i(1−d_i)/(1 − Σ p_j d_j). | Other cells are renormalized upward by 1/(1−pD) (GB-2). |

- The hole detection model: d_hole(s,m) = P(hole trace intersects the *footprint* (alteration/geochem halo, not ore) of a deposit at s with mark m) × P(recognized | intersected, logging/assay protocol). Detection is never 1.
- d is hierarchical, and its overstatement is the AF447 failure (R7 §1.7). The posterior over d is shown in GB-2.
- Artifact: `SearchUpdate{mode, before, after, d_posterior, hypothesis_reweights, mass_moved}`. It is an append-only claim pair with a galaxy-brain card.

**(E) Drill-target ranking under budget**
- **Objective.** Expected detection-weighted value F(S) = E_post[ Σ_deposits V(m)·(1 − Π_{h∈S}(1 − d_h(deposit))) ] − cost(S).
  - For each fixed deposit realization the coverage term is monotone submodular in the set of holes S. A non-negative mixture over the posterior preserves submodularity.
  - So F(S) + cost(S) is monotone submodular (FACT[U]: Krause & Guestrin, Nemhauser–Wolsey–Fisher).
- **Algorithm.** Lazy-greedy (CELF). Under a cardinality constraint the bound is ≥ (1 − 1/e). Under a knapsack budget use cost-benefit greedy plus the best singleton, for ≥ (1−1/e)/2 ≈ 0.316 (FACT[U]). Emit `PortfolioCertificate` (§4.2).
- **Not claimed:** the *VOI* objective is generally not submodular. So sequential, adaptive choice at prospect scale goes to `fm-pomdp` (phase 2) or myopic VOI with lookahead 1. Portfolio ranking uses F.
- **Deal structures.** The same ranking accepts options whose cost and value are shaped by earn-in, stake or option terms (R6 §4.7). The deal economics belong to PQ-7, and the interface is `ProgramOption{deal}`.

**MNPI artifact.**
- Every posterior is computed twice when MNPI inputs exist: `basis = Public` (public evidence only) and `basis = Full`.
- Only the Public posterior may leave the information barrier.
- The *difference* between them is itself MNPI (it encodes drill results).
- Any Claim whose provenance contains an MNPI leaf is MNPI until that leaf's public-release valid-time (PO-12).
- A capability gate forbids junior-equity decisions (PQ-7) from consuming `basis = Full` claims on that junior's ground (E023; OPEN-6 with legal).

---

## 3. PQ-5 — KK/KU/UU natively, with honest, calibratable, auditable probabilities

### 3.1 Framing

- **Scope.** The whole chain: exploration → resources → reserves → plans → cost forecasts → tailings → accounting estimates.
- **Most costly error** (E-loss #1): a false-certainty number reaching a reserve disclosure, a capital decision or a dam.
- **Fixed constraints:**
  - software never signs (E023): `reserve_disclosure_claim`, `geotechnical_signoff` and the other flags are never set true;
  - CP/QP classification is never automated (E031);
  - controllers own GAAP judgment (E019);
  - FrankenMining-generated evidence is self-sourced and needs third-party documents or a human `Reviewed` attestation (E020).

### 3.2 Hypotheses, falsifiers, discriminating tests

| Hyp. | Mechanism | Falsifier (what would kill it) | Discriminating test |
|---|---|---|---|
| (a) One coherent posterior | A joint generative model; every uncertainty is a prior; propagation by sampling. | Under M-open misspecification the posterior concentrates on the KL-nearest wrong model, so 80% coverage → 0 as n grows (Grünwald & van Ommen 2017, FACT[R7]). It also cannot host CP/QP/controller judgments, which are not likelihood terms. | T5-1 synthetic-truth mine with an out-of-model fault offset and a financing-biased capex process: plot coverage vs number of resolutions. |
| (b) Credal / DS | Lower/upper probabilities; vacuous ignorance; DS combination. | Dilation: conditioning widens every interval (Seidenfeld & Wasserman 1993, FACT[U]). The DS conflict paradox (Zadeh, FACT[U]). No strictly proper scoring rule for imprecise forecasts (Seidenfeld–Schervish–Kadane 2012, FACT[U]), so it cannot be calibrated by a ledger. Decision regret from indecision. | T5-1: interval width over time, decision regret vs oracle, and scoring feasibility. |
| (c) Layered stack | Seven layers (§3.5); UU is made explicit and bounded, monitored and converted. | Monitors alarm at more than α in a *correctly specified* control mine. Or post-alarm variance inflation does not restore coverage. Or sharpness loss exceeds 10% without a coverage gain. Or any probability is emitted without a model or reference-class basis (fabrication count > 0). | T5-1 plus T5-3 control mine plus T5-2 historical hindcasts: Bertisen & Davis capex set (63 projects), a site's F3 history, and Yilgarn discoveries. |
| (d) Info-gap only + track record | UU is never a probability. Decisions use robustness margins (Ben-Haim, FACT[U]). Trust is earned only through the scoring record (Cooke weighting; IDEA protocol). | KU questions that do have good probabilistic models get needlessly conservative decisions. Long-horizon claims (discovery, reserve life, dam PoF) resolve too slowly to earn trust (feedback lag 5–17 years). | T5-1 regret, plus count of claims with 0 resolutions within 5 years. |

**Predictions** (HYPOTHESIS, to be confirmed by T5-1):
- (a): coverage decays monotonically as data accumulate, and it stays confident.
- (b): coverage high, width grows, decisions stall.
- (c): coverage dips, the alarm fires, inflation brings coverage back within ±5 pts.
- (d): robust but highest opportunity loss on KU-dominated decisions.

### 3.3 Verdicts

- **(c) wins.**
- (a) lives inside models as L1.
- (b) lives only as an ε-contamination *envelope at decision time* for irreversible actions, and as Manski bounds for unidentified inputs. Credal sets are never the stored belief.
- (d) supplies the third admissible UU form (flip margins, "probability not stated"). Its track-record principle becomes the calibration ledger and performance-weighted expert pooling.
- Checked null on novelty: the stack assembles R7 §9.3–9.5's mechanisms. The contributions here are:
  1. the three-form UU admissibility rule;
  2. the catch-all floor with reset;
  3. bounded-score racing;
  4. the vocabulary reconciliation;
  5. lifting fs-evidence's flip-margin machinery.

### 3.4 The layered stack (DESIGN), with timescale separation

| Layer | Content | Update cadence | Artifact |
|---|---|---|---|
| L0 Measurement | QA/QC-passed measurements with error models; KK is *revocable* (a CRM failure cascades) | per event | `Claim{epistemic: KnownKnown, evidence: Measured}` |
| L1 Within-model Bayes | Geostat simulations, LGCP, recovery and cost models. Tempered posterior power η ≤ 1 where misspecification is suspected; η is calibrated on hindcast, not hand-set. | per data event | Ensembles with a shared realization index |
| L2 Across-model | **Stacking** (Yao et al. 2018) over KU-s alternatives, plus a catch-all h₀ with floor ε_ref. BMA only with an explicit M-closed justification. | per model revision | `HypothesisSet` with weights and h₀ |
| L3 Discrepancy and reference class | Hierarchical bias / variance-inflation per forecast family, learned from the ledger (F-factors as bias priors, R7 §4.3), plus outside-view priors (Bertisen & Davis capex). | per reconciliation period | `Correction{family, bias, κ}` claims, *visible* |
| L4 UU term | One of three admissible forms (§3.9) | per review or alarm | `UuTerm` on every decision-facing claim |
| L5 Surprise | e-process coverage confidence sequences, model races, null race, CUSUM pairing, conformal coverage (Mondrian) where exchangeable, AOA, staleness | continuous | `Monitor` automata (§4.2) |
| L6 Projection | FSS knowledge state, fs-evidence Color, franken-evidence `EvidenceLedger` | on read | `KnowledgeCell`, executive map |
| L7 Decision | franken-decision contracts for discrete gates. CVaR / ε-envelope robust choice (fs-robust) for irreversible actions. Human attestation gates. | per decision | `DecisionRecord` + `EvidenceLedger` |

**Interference rule.** The ledger scores *both* the raw L1/L2 forecast and the published, L3-corrected forecast as separate segments. So corrections are themselves tested and cannot hide model drift. L3 never reads a resolution that L5 has not yet logged.

### 3.5 Coverage across the whole system

| Domain | Dominant KU | UU form (default) | Fast resolvable forecasts (calibration fuel) | Representation and who signs |
|---|---|---|---|---|
| Exploration | λ, q, θ_τ, mark tail | Novel-type slot (Good–Turing) plus A4 Manski bounds | Drill-hole outcomes at prospect scale. Discoveries are slow (hindcast backfill). | Ensembles; no signer |
| Resources | Grade/tonnage simulations, domaining, variogram | Catch-all conceptual model h₀ (Mern 2024) | **F1 per bench per month**: the richest stream | Quantitative evidence beside a CP `Category`; CP signs |
| Reserves | Modifying factors, price, recovery | Reference class (reserve-to-production grade bias: OPEN-7, sources [U]) | F2/F3 quarterly | "P(economic extraction) > 0.5" as *evidence* for the JORC-2026 RPEE (R7 §9.2); QP signs |
| Plans/schedules | Productivity, availability, sequencing | Reference class (schedule compliance as low as 30%, E026) | Tonnes and grade to mill per period | Ensembles; planner attests |
| Cost forecasts | Unit costs; capex by WBS | **Reference class**: Bertisen & Davis mixture (GB-6), with a jurisdiction/security residual as a flip margin | Monthly actuals per WBS | Shifted-mixture claims; project controls attest |
| Tailings | Pore pressure, strength, failure modes | **Flip margin only.** PoF is not stated unless a vetted reference class exists. Evidence state per failure mode. | Piezometer and InSAR series; an offline sensor is `not_observable` | Never "safe" (R7 §8.4); EoR signs; `geotechnical_signoff` never software-set |
| Accounting estimates | Recoverable amount, ARO, UoP reserves, stockpile NRV, provisional pricing | Flip margins feed the IAS 1.125-style "sources of estimation uncertainty" (FACT[U]; OPEN-9) | Settlements, final assays, actual closure costs (slow) | `Projected{policy}` point for the FA books; the distribution stays in the workpaper; controller attests (E019, E020) |

### 3.6 The universal `Claim` (Rust sketch; DESIGN, crate `fm-claim`)

```rust
#![forbid(unsafe_code)]
pub struct Claim {
    pub id: ClaimId,                      // blake3 of canonical body; append-only, never edited (supersede instead)
    pub subject: SubjectRef,              // DepositId | TractId | BlockSetId | PlanPeriodId | WbsId | TsfModeId | EstimateId
    pub quantity: QuantityKind,           // ContainedMetal{metal} | Tonnage | Grade(Compositional) | Count | Probability{event} | Npv{deck} | Ratio ...
    pub unit: UnitSpec,                   // fs-qty backed; adds metal/mass/money (gap E019)
    pub support: Support,                 // Spatial{Point|Interval|Composite|Smu|Panel|Polygon|Tract} × Temporal{Instant|Window}
    pub repr: Representation,
    pub epistemic: EpistemicStatus,       // what kind of not-knowing
    pub evidence: EvidenceClass,          // how it is known
    pub color: fs_evidence::color::Color, // COMPUTED by fm-claim::project_color, never caller-set
    pub state: KnowledgeState,            // FSS lattice, computed from the above plus validity and monitors
    pub calibration: CalibrationFlag,
    pub uu: UuTerm,                       // mandatory on decision-facing claims (guard G-UU)
    pub validity: Validity,               // fs_evidence::ValidityDomain + price deck + tech + valid-time window + review_by
    pub provenance: ProvenanceDag,        // input ClaimIds, transforms {code hash, params, seed}, human acts, DocRefs
    pub confidentiality: Confidentiality, // join-semilattice; taint is monotone (PO-12)
    pub authority: Authority,             // orthogonal to evidence: Computed | Elicited | Attested
    pub forecast: Option<ForecastRegistration>, // if resolvable: source, horizon, scoring rule, segment
    pub time: Bitemporal,                 // valid_time and record_time (hindcast "as-of", E015)
    pub supersedes: Option<ClaimId>,
}
pub enum Representation {
    Closed(DistSpec),                                    // fsci-stats family + params
    Ensemble { index_space: RealizationSpaceId, artifact: ContentHash }, // common random numbers across modules
    Quantiles(Vec<(f64, f64)>),
    Interval { lo: f64, hi: f64, kind: IntervalKind },   // Enclosure | Credible{lvl} | Conformal{lvl, calib} | Manski
    Category(CodeClass),                                 // e.g. JORC "Indicated": a label, NEVER mapped to a probability
    Projected { value: f64, policy: ProjectionPolicyId, of: ClaimId }, // book/disclosure point; no naked numbers
    Refused(Refusal),                                    // NoClaimInDomain | NotObservable | NoUsefulBound(cause) | MnpiBarrier
}
pub enum EpistemicStatus { KnownKnown, KnownUnknown(KuKind), UnmodelledBudget }
pub enum KuKind { Aleatory, Parametric, Structural { set: HypothesisSetId } }
pub enum EvidenceClass {                  // R7 §9.3 list, minus `attested` (moved to Authority), plus FSS `Assertion`
    Measured, Derived, ModelledCalibrated, ModelledUncalibrated, Elicited,
    Transferred { aoa: AoaStatus }, Assumed, Assertion /* LLM-extracted text, imported unverified statements */ }
pub enum CalibrationFlag {
    Calibrated { record: CalibRecordId, n: u32, cov80: f64 },
    Uncalibrated,
    Miscalibrated { record: CalibRecordId, bias: f64, inflation: f64 },
    NotCalibratable { reason: NotCalibReason },          // e.g. horizon > ledger history; unique event
}
pub enum UuTerm {
    ReferenceClass { epsilon: f64, shape: ContaminationShape, refclass: RefClassId }, // ε-contamination
    CatchAll { set: HypothesisSetId, floor: f64, refclass: RefClassId },
    FlipMargin { flips: Vec<fs_evidence::uncertainty::FlippingUnknown>, probability: NotStated },
    NotDecisionRelevant { justification: NonEmptyString },
}
pub enum Authority { Computed { module: ModuleId, version: SemVer },
    Elicited { expert: PersonId, protocol: ElicitationProtocol /* IDEA */ },
    Attested { role: AttestRole /* CP|QP|EoR|Controller|ITRB */, person: PersonId, sig: Signature, scope: ClaimScope } }
```

### 3.7 Vocabulary reconciliation and the no-laundering rule

| R7 evidence class (FM) | Projected `fs_evidence::Color` | FSS evidence class | Default knowledge state |
|---|---|---|---|
| Measured (QA/QC-passed, characterized error) | `Validated{regime: measurement-system envelope, dataset: QA/QC record}`. `Verified` is reserved for interval enclosures, e.g., mass-balance bounds. | observed / corroborated (independent labs) | known (revocable) |
| Derived | `compose(inputs)`: weakest-link | derived | inherits the weakest input |
| ModelledCalibrated | `Validated{regime: calibration domain, dataset: CalibRecordId}`; auto-demotes outside it | derived | estimated |
| ModelledUncalibrated | `Estimated{estimator, dispersion}` | derived | estimated |
| Elicited | `Estimated{estimator: ElicitationId, dispersion}` | assertion → derived after scoring | estimated |
| Transferred | `Estimated` outside AOA; inside AOA of a validated regime, `Validated` with the intersected regime | derived | estimated, with the AOA flag |
| Assumed | `Estimated{dispersion: ∞}` ("no spread claim", color.rs) | assertion | unknown |
| Assertion | refused as a `Color` leaf until reviewed | assertion | unknown |

Mining mapping for fs-evidence's eight `EngineeringUncertaintyKind`s (DESIGN):

| fs-evidence kind | Mining meaning |
|---|---|
| Geometry | Geological interpretation and domaining |
| Parameters | Variogram, recovery, q_j |
| ModelForm | Conceptual model, i.e., KU-s plus the UU residual |
| Measurement | Assay, FSE and density |
| Discretization | Block or mesh support |

**No-laundering rules** (DESIGN; each is a property-tested invariant, PO-9):

| Rule | Statement |
|---|---|
| NL-1 | The color of any derived claim = `fs_evidence::color::compose` over its inputs. Its rank never exceeds the minimum input rank (`fs_robust::weakest_color` for headlines). |
| NL-2 | Evidence class of a derived claim = the weakest input class under the partial order Measured > Derived > ModelledCalibrated > {ModelledUncalibrated, Transferred, Elicited} > Assumed > Assertion. |
| NL-3 | **Attestation is orthogonal.** A CP/QP/EoR signature never upgrades class, color or calibration. It changes `authority` only. This keeps "the CP signed it" from becoming "it is known". |
| NL-4 | **Calibration is not inherited.** A derived claim is `Uncalibrated` until *its own* forecast family has resolved outcomes at its own support. |
| NL-5 | Knowledge states never upgrade under conjunction (unknown ∧ known = unknown). `redacted` stays redacted downstream. `not_observable` never becomes "absent". |
| NL-6 | Refusals are absorbing unless a *declared* fallback exists (MapMark-degenerate, Manski bounds). |
| NL-7 | Confidentiality is a join (MNPI ∨ public = MNPI). |
| NL-8 | `franken_evidence::EvidenceLedger.calibration_score` is never hand-entered. It is computed as `1 − min(1, max(|cov80 − 0.8|/0.2, KS_PIT))` from the claim family's ledger record, and is 0 when Uncalibrated. `fallback_active` mirrors `franken_decision::FallbackPolicy::should_fallback`. |

### 3.8 Forecast & calibration ledger (`fm-calib`; DESIGN)

- **Register before resolve.** Every resolvable claim is registered with `record_time < outcome.valid_time`. The ledger is append-only and hash-chained, and supports bitemporal as-of replay (PO-8).
- **Proper scores by family:**
  - CRPS for continuous quantities (grade, tonnes, capex ratio);
  - Brier and log score for events (P(≥1 deposit), "within ±10%");
  - point-process log score for spatial discoveries;
  - interval score (Gneiting & Raftery 2007) for quantile claims;
  - PIT histograms;
  - coverage at 50/80/95%.
- Segmented by model × domain × horizon × author, where author includes agents.
- **Racing models** uses `fs_eproc::PairwiseRace` with a declared `LossSpan`. Only *bounded* scores qualify: Brier, CRPS normalized by a declared range, or a log score floored at p_min (the floor is a recorded design choice). The race refuses out-of-span inputs, which is correct (fs-eproc lib.rs:165).
- **Coverage monitors** use `fs_eproc::bernoulli::BernoulliMixtureCs` on 1{outcome ∈ 80% interval} at the **conditionally independent support** (monthly reconciled totals per domain, not per block). This matters because within-month spatial correlation would void the martingale argument.
- Pair each monitor with a CUSUM for fast large shifts (FAILURE-CASES F7). GB-8 shows why: monthly coverage at 60% instead of 80% needs about 5 years to reach e ≥ 20.
- **Feedback.** A hierarchical model per family: log(actual/forecast) ~ N(μ_f + u_site, σ_f²). The coverage-restoring inflation is κ_f. Both feed L3 as visible `Correction` claims.
- **Small-n remedies, in order:**
  1. score at the finest resolvable support;
  2. hindcast backfill from the bitemporal record;
  3. synthetic-truth SBC;
  4. reference classes.

### 3.9 Stating probabilities on unknown unknowns without fabrication

A decision-facing claim must carry exactly one `UuTerm`. Only three forms are admissible:

1. **Reference-class ε-contamination.** The belief set is P_ε = {(1−ε)P_model + εQ}.
   - ε and Q come from a `RefClass` record: population, n, period, inclusion rule, similarity justification, verification tag [V]/[U] and source document.
   - Q = the reference-class residual distribution when one exists, otherwise worst case over the support.
   - Robust choice for irreversible actions: min_a max_{Q} E[L] or CVaR via `fs_robust::cvar`.
   - Example: the capex mixture (GB-6). A [U]-tagged reference class may inform internal decisions but is **blocked from disclosure-facing claims** (guard G-DISC).
2. **Catch-all hypothesis h₀** (max-entropy null; Mern et al. 2024) with floor ε_ref.
   - Naming a new hypothesis after a surprise debits h₀. Then h₀ is **reset to ε_ref** by renormalizing the named set (PO-11), because the base rate of conceptual surprise is a property of the domain and is not used up by naming one surprise.
   - The h₀-vs-best likelihood ratio is an e-value against the named model and is monitored.
   - For **novel types and novel failure modes**, ε_ref uses Good–Turing unseen mass n₁/N over the versioned type or failure-mode catalogue (GB-9).
3. **Flip margin, probability NOT STATED** (info-gap).
   - The type carries `NotStated`, so no code path can turn it into a number.
   - It uses `fs_evidence::uncertainty::FlippingUnknown{required_magnitude, bound}` against the decision's requirement. The tri-state `ComplianceVerdict::Indeterminate` is the honest result when an unbounded unknown can flip it.
   - The executive sees "unpriced unknown: decision flips if X exceeds Y; no admissible reference class".

**UU → KU conversion automaton:**

```text
Armed → Alarm(e ≥ 1/α | CUSUM | AOA exit | h₀ race) → InvestigationOpen{InvestigationCase per FSS §20.9}
      → {HypothesisAdded(debit h₀, reset ε) | Recalibrated(Correction claim) | FalseAlarmLogged} → Rearmed
```

There is never a silent retrain (R7 §9.5.5).

### 3.10 Unknown knowns (data held but not used)

- **Registry.** `fm-source` registers every data asset: report, database, survey, core library or expert. Each model declares the inputs it consumed.
- **The gap is computed, not remembered.** UnusedRelevant(claim) = {registered assets intersecting the claim's support and validity} − provenance(claim).
- **Ranking.** Unknown knowns are ranked by EVOI(ingest) per digitization cost, with ingest modelled as a survey whose "detection" is the expected effort and deposit information recovered.
- Expect the top items to be legacy effort records: they convert A4 (Manski) cells into A1/A2.
- Other unknown knowns:
  - failed past programs (the negative-evidence ledger, FSS §23.8);
  - tacit geologist knowledge, through IDEA-protocol elicitation, which is stored and scored.
- LLM-extracted facts from reports enter as `Assertion` with document provenance. Promotion requires deterministic re-derivation (e.g., collars re-georeferenced) or a human `Reviewed` attestation (E020).

### 3.11 How executives see it

**One-page epistemic map** (illustrative values; one row per decision-relevant claim):

```text
EPISTEMIC MAP · Region R · as-of record 2026-Q4 · basis: PUBLIC              colour = fs-evidence Color
Claim                            P10 / P50 / P90         Evidence · Calib (track)        UU                         Top driver → cheapest move (net VOI)
P(≥1 econ Au deposit, tract T7)  0.06 / 0.11 / 0.19      Transferred · Uncal (hindcast:1) novel-type ε=0.025 (GT)    effort coverage 38% → digitize open-file (+$1.1M)
Undiscovered Au, T7              0.4 / 2.1 / 9.8 Moz     Modelled · Uncal                 A4 bounds [1.6, 3.4] Moz P50 GPD tail ξ → hindcast district 2
Reserve Au, Site A (QP)          QP: 8.1 Moz | model P(±10%)=0.71  ModelCal · cov80 15/20 qtrs  catch-all h₀ 0.05     F1 bias −4% domain D3 → infill (+$0.6M)
Capex, Project P                 $2.06B / mean 2.31 / $3.2B  RefClass[V] B&D · n/a      jurisdiction: FLIP @R=1.7, P not stated  scope maturity → FEL-3 review
TSF-3 static liquefaction        4/6 failure modes monitored · 2 piezometers stale >14 d (NOT OBSERVABLE)  PoF: NOT STATED   EoR review 2026-08
Impairment headroom, CGU-2       −$0.1B / +$0.4B / +$0.9B  ModelUncal · n/a              FLIP: Au < $2,900/oz        price deck → none (exogenous)
```

**"What would change our mind" card** (one per decision). Fields:
- the decision and the current action, with expected loss by action (from the `EvidenceLedger`);
- each flip condition with its `required_magnitude`, its probability *if priced*, the evidence that would reveal it, and that evidence's cost, time and net VOI;
- monitors armed, with thresholds;
- the kill gate;
- who must attest.

It renders through `franken_evidence::render::{level1, markdown, html}` plus the FM-specific rows. Every number is clickable down to its Claim, then its provenance DAG, then its source documents.

### 3.12 How agents use it

- Agents read `KnowledgeCell`s through an FSS-style `SituationCapsule` and `WorldEnvelope`:
  - certified core: Measured/Validated claims;
  - material alternatives: the hypothesis set plus h₀;
  - protected residuals: UU flip margins and `not_observable` regions.
- Agents may **propose**, never **upgrade**. Agent-authored claims are `Assertion` or `Elicited{expert: agent}`.
- Agent forecasts are registered and scored in the same ledger under `author = agent:model@version`, so trust is earned by track record.
- Agents open `InvestigationCase`s with VOI-ranked probes (FSS §20.9). They cannot cross MNPI barriers (capability-scoped `basis`). They cannot construct `Authority::Attested`, because that is gated by a `HumanAct` capability.
- Every agent answer must cite ClaimIds and include the UU term. Unsupported numbers are refused at the response envelope.

---

## 4. Alien-artifact compilation (both PQs)

### 4.1 EV matrix (Impact × Confidence / Effort; natural fits only)

| Family / method | I | C | E | Score | Verdict | Compiled artifact |
|---|---|---|---|---|---|---|
| Info-gap flip margins (fs-evidence `FlippingUnknown`, exists) | 4 | 4 | 1 | 16 | **ACCEPT** | `UuTerm::FlipMargin`, WWCOM card |
| Bayesian search / thinning (Koopman–Stone; ecology PPP) | 5 | 5 | 2 | 12.5 | **ACCEPT** | `SearchUpdate`, thinning integrator, `AbsenceClass` |
| Proper scoring + calibration ledger (Gneiting) | 5 | 5 | 2 | 12.5 | **ACCEPT** | `fm-calib` ledger, PIT/coverage reports |
| Reference-class ε-contamination (Flyvbjerg; Huber) | 5 | 4 | 2 | 10 | **ACCEPT** | `RefClass`, `UuTerm::ReferenceClass` |
| Good–Turing unseen mass | 3 | 3 | 1 | 9 | **ACCEPT** | novel-type and novel-failure-mode ε |
| e-processes (Ville; fs-eproc exists) | 4 | 4 | 2 | 8 | **ACCEPT** (paired with CUSUM) | coverage confidence sequences, `PairwiseRace`, null race |
| Stacking + catch-all h₀ | 4 | 4 | 2 | 8 | **ACCEPT** | `HypothesisSet{weights, h0_floor}` |
| Partial identification (Manski) for missing effort | 4 | 4 | 2 | 8 | **ACCEPT** | `Interval{kind: Manski}` |
| Preposterior VOI (Howard; Eidsvik) | 5 | 4 | 3 | 6.7 | **ACCEPT** | `VoiEstimate{status}` |
| EVT/GPD mark tail | 4 | 3 | 2 | 6 | **ACCEPT** | spliced mark model; tail diagnostics |
| Submodular lazy-greedy portfolio | 3 | 4 | 2 | 6 | **ACCEPT** | `PortfolioCertificate` |
| Hierarchical LGCP/SPDE/GMRF + Laplace | 5 | 4 | 4 | 5 | **ACCEPT** (core, effortful) | `fm-ppm`, log-concavity certificate |
| Conformal (Mondrian by terrane or domain; CRC) | 3 | 3 | 2 | 4.5 | ACCEPT where exchangeable | coverage sets, per-stratum dashboard |
| Tempered / generalized posteriors (SafeBayes-calibrated η) | 3 | 3 | 2 | 4.5 | ACCEPT (L1 option) | η per model family, ledger-calibrated |
| Causal DAG / transportability (mineral systems, noisy-AND) | 4 | 3 | 3 | 4 | ACCEPT as structure | DAG + d-separation test report |
| Mutual-information EOI | 2 | 4 | 2 | 4 | secondary display only | EOI column (never the ranking key) |
| POMDP (POMCPOW/SARSOP) | 4 | 3 | 5 | 2.4 | **DEFER** (phase 2, kill K4) | `fm-pomdp` |
| Optimal transport / MMD terrane similarity | 2 | 3 | 3 | 2 | REJECT for v1 | use the Meyer–Pebesma dissimilarity index |
| Deep CNN as object of record | 3 | 2 | 3 | 2 | REJECT (keep as covariate) | — |
| Dempster–Shafer combination | 2 | 2 | 3 | 1.3 | REJECT (conflict paradox, not proper) | — |
| Full credal propagation | 3 | 2 | 5 | 1.2 | REJECT | — |
| Differential privacy for MNPI | 2 | 2 | 4 | 1 | REJECT (access control + dual basis instead; OPEN for JV sharing) | — |
| Sheaf cohomology / TDA / game theory | 2 | 2 | 4 | ≤1 | REJECT as ornamental here (game theory → PQ-7) | — |

**Composition and timescale statement** (25-COMPOSITION rule):

| Layer | Updates | Reads |
|---|---|---|
| L1 inference | per event | — |
| L3 corrections | per period | resolved outcomes only |
| L5 monitors | continuous | registered forecasts only |
| L7 decisions | per decision | — |

No two adaptive controllers write the same parameter. Interference test: on the T5-3 control mine with all layers on, false alarms must be ≤ α and sharpness loss < 10%.

### 4.2 Compiled runtime artifacts (DESIGN)

```rust
// fm-explore-data
pub struct EffortRecord { id: EffortId, method: ExploreMethod, geom: Geom, valid: TimeInterval,
    spec: MethodSpec /* line spacing, DOI, detection limits, hole depth */, source: DocRef, coverage: CoverageConfidence }
pub enum AbsenceClass { NotLooked, LookedInsensitive { method: ExploreMethod, q_upper: f64 },
    LookedSensitiveNothing { hazard: f64 }, FoundSubEconomic { occ: OccurrenceId }, EffortUnrecorded { bounds: (f64, f64) } }
// fm-ppm
pub struct IntensityPosterior { mesh: MeshId, latent: LatentRepr, hyper: HyperDraws, q: DetectionDraws,
    marks: MarkDraws, basis: EvidenceBasis /* Public | Full */, cert: LaplaceCertificate }
pub struct LaplaceCertificate { grad_inf_norm: f64, ldlt_all_pivots_positive: bool, newton_iters: u16,
    sbc_run: Option<SbcRunId>, psis_khat_max: f64 }          // refuse if khat > 0.7 or pivots not positive
impl IntensityPosterior {
    pub async fn undiscovered(&self, cx: &Cx, area: &Polygon, econ: &EconFilter, t: ValidTime)
        -> Outcome<UndiscoveredSummary, PpmRefusal>;      // asupersync Outcome{Ok,Err,Cancelled,Panicked}
}
pub struct UndiscoveredSummary { p_ge1: Claim, count: Claim, contained_metal: Claim, p_tier1: Claim,
    limiting_ingredient: IngredientId, top_driver: UncertaintyDriver, aoa: AoaStatus, unknown_effort_frac: f64 }
// fm-search / fm-explore-voi
pub enum SearchMode { Cox, SingleTarget { anomaly: AnomalyId, p_exists: ClaimId } }
pub struct SearchUpdate { mode: SearchMode, before: ClaimId, after: ClaimId, d_posterior: DistSpec,
    hypothesis_reweights: Vec<(HypothesisId, f64, f64)>, mass_moved_elsewhere: f64 }
pub struct VoiEstimate { gross: Claim, net: Claim, mc_se: f64, status: VoiStatus /* Positive|Negative|Indeterminate */ }
pub struct PortfolioCertificate { ratio_bound: f64 /* 1-1/e or (1-1/e)/2 */, submodularity_test: PropTestId,
    marginal_gains: Vec<(TargetId, f64)>, best_singleton: TargetId, budget_used: Money }
// fm-surprise
pub enum Monitor { Coverage(fs_eproc::bernoulli::BernoulliMixtureCs), Race(fs_eproc::PairwiseRace),
    NullRace { log_lr: f64 }, Cusum { s: f64, h: f64 }, Aoa { di: f64, threshold: f64 }, Staleness { max_age: Duration } }
```

**Guard tables** (deterministic; evaluated before render or export):

| Guard | Condition to PASS | On FAIL |
|---|---|---|
| G-UU | Decision-facing claim has exactly one admissible `UuTerm`. Any ε cites a `RefClass`. | Refuse render: "unbudgeted unknowns" |
| G-EXEC | Not `Refused`; not `stale`; calibration shown verbatim; Color shown | Show the knowledge state *instead of* a number |
| G-DISC (disclosure-facing) | Evidence ∈ {Measured, Derived, ModelledCalibrated}; no `Assertion`/`Assumed` in provenance; RefClass tags all [V]; CP/QP `Attested` present | Block export. Software never sets `reserve_disclosure_claim` (E023). |
| G-MNPI | Viewer capability ≥ claim confidentiality; `basis = Full` never crosses the barrier | Serve the `basis = Public` twin or `Refused(MnpiBarrier)` |
| G-ABS | Negative-evidence update only if effort has coverage ≥ Medium and q ≥ q_min (FSS `INV-016` analogue) | Treat as A0/A1 (no update), or A4 (Manski) |
| G-AOA | Dissimilarity ≤ d_max for absolute P | Interval or refusal (F3) |

**Automata.**

Hypothesis lifecycle:
```text
Proposed → PriorPredictiveChecked(pass) → Active → {Falsified(test, e) | Superseded | Dormant}
```
A failed prior-predictive check goes to `Rejected` (Box 1980; Caers 2025).

Target stage-gate (a franken-decision contract over states {econ, sub-econ, barren}):
```text
Generated → Ranked → Surveyed → Drilled → {Discovery | Downgraded | Dropped}
```
Kill if P(≥1) < p_drop and there is no A3 intercept.

### 4.3 Proof obligations

| ID | Statement | Verification | Fallback if violated |
|---|---|---|---|
| PO-1 | For fixed hyperparameters, adding effort with no discoveries never increases λ_undisc(s). | Metamorphic property tests | Block release |
| PO-2 | E[N_obs(A)] + E[N_undisc(A)] = E[N(A)] (thinning split) within quadrature tolerance 1e-6 relative. | Exact identity test | Block |
| PO-3 | Polygon-integrated counts are invariant under mesh refinement within 2% (change of support). | Refinement ladder | Coarse-mesh claim + Discretization term |
| PO-4 | For fixed (θ, q), the latent posterior is strictly log-concave (Poisson log-link + Gaussian prior), so the Newton mode is unique. Certificate: ‖∇‖∞ < tol and all LDLᵀ pivots > 0. | `LaplaceCertificate` | HMC (district) or N0 |
| PO-5 | Degenerate config matches MapMark4 endowment quantiles within MC error (KS < 0.02 at 10⁵ draws). | Lab oracle (R, test-time only) | Block parity claim |
| PO-6 | Single-target update equals p(1−d)/(1−pd). Total mass is conserved over cells. | Unit test | Block |
| PO-7 | SBC rank histograms uniform (χ² p > 0.01). Undiscovered-count and metal coverage within ±3 pts of nominal over ≥200 replicates (R7 §1.12). | Synthetic district | Demote to `Uncalibrated` |
| PO-8 | The ledger is append-only and hash-chained. Every scored forecast has record_time < outcome valid_time. As-of replay is bit-identical. | Chain audit | Quarantine the segment |
| PO-9 | NL-1…NL-8 hold over 10⁵ random provenance DAGs. | Proptest with fs-evidence `compose` | Block |
| PO-10 | Monitor false-alarm ≤ α under a calibrated null at the declared conditionally independent support (Ville). | Control-mine replay | Re-specify support |
| PO-11 | P(h₀) ≥ ε_ref after every revision, including after hypothesis additions. | Invariant test | Block |
| PO-12 | conf(derived) ≥ ⊔ conf(inputs); no Full-basis bytes in Public exports. | Taint test plus export fuzz | Block export |
| PO-13 | The greedy portfolio achieves the stated ratio. Submodularity of F is checked on samples. | Marginal-gain trace | Report as "heuristic, no bound" |
| PO-14 | VOI ≥ −2·SE. The sign is reported only when resolved at 2·SE. | MC CI | `Indeterminate` + heuristic baseline |
| PO-15 | `Refused` never becomes a number downstream without a declared fallback. | Type-level plus test | — |
| PO-16 | Mondrian conformal coverage ≥ 1−α within each exchangeable stratum. | Held-out strata | Drop the conformal claim |

### 4.4 Assumptions ledger

| ID | Assumption | Class | Test / monitor | If false |
|---|---|---|---|---|
| A1 | Independent thinning given λ (no deposit-to-deposit detection interaction) | structural | District shutdown effects in hindcast | Add a district-maturity covariate to q |
| A2 | **Effort ignorability**: effort depends on recorded information only | distributional | Lagged effort in λ predicts post-T discoveries? | Preferential-sampling joint model or Manski widening |
| A3 | Poisson/Cox with clustering in w; inhibition via the aggregation rule | structural | Pair-correlation residuals | Add an inhibition term |
| A4 | Marks independent of location given covariates | structural | Mark–covariate residual test | Mark regression |
| A5 | Deposit identity and aggregation applied identically to labels, marks and outputs (USGS consistency rule) | operational | Versioned rule hash on all three | Block fit |
| A6 | Covariate realizations represent interpolation uncertainty (Caers 2025) | computational | Variance-ratio check vs deterministic | Flag `Estimated`, widen |
| A7 | Reported tonnage = true × growth(era) | parametric | Inventory time series | Wider growth prior |
| A8 | Conditional independence of scored outcomes at the monitor support | distributional | Residual autocorrelation | Coarsen support |
| A9 | Reference classes are relevant (similarity justified) | structural | Reviewer sign-off on `RefClass` | `FlipMargin` instead |
| A10 | Reconciliation references are themselves noisy (mill head grade is estimated, R7 §4.3) | measurement | AMIRA P754 error model | Errors-in-variables scoring |

### 4.5 Failure modes and deterministic conservative fallbacks

| # | Failure | Detection | Deterministic fallback |
|---|---|---|---|
| F1 | Effort coverage below G-P4 | Coverage metric | Manski bounds for absolute P; relative ranking only; *digitize-first* VOI list |
| F2 | Laplace or sampler failure (pivots, k̂ > 0.7, R̂ > 1.01) | `LaplaceCertificate` | N0 MapMark-degenerate with a widened hyperprior, `Estimated` |
| F3 | Outside AOA or terrane dissimilarity > d_max | AOA monitor | Hyperprior predictive interval only; no point P |
| F4 | Hindcast gate fails | G-P3 | Demote to `Uncalibrated`; executive rows show N0 |
| F5 | h₀ wins the null race (e ≥ 20) | NullRace | Freeze hypothesis-conditioned drill recommendations; open an InvestigationCase; recommend falsification probes |
| F6 | Coverage alarm | e-process or CUSUM | Apply κ inflation (visible `Correction`); flag `Miscalibrated`; human review |
| F7 | No reference class for a UU | G-UU | `FlipMargin`, P not stated |
| F8 | MNPI path detected in an export | G-MNPI | Public twin or refusal |
| F9 | VOI sign unresolved | PO-14 | Uncertainty-proportional heuristic, labelled as such |
| F10 | Compute budget exhausted (continental) | `Cx` budget (E012) | Coarser mesh plus an explicit Discretization uncertainty term, never silent |

### 4.6 Verification plan: synthetic-truth district and mine (`fm-synth`)

- **District generator.** Ingredient fields → λ (noisy-AND) → Cox deposits with spliced marks and depths → cover-thickness field → era-wise exploration simulator.
  - The simulator places effort by method with known q_j.
  - It has an *ignorable* mode and a *non-ignorable* mode (follows unrecorded gossans).
  - Discovery catalogue with growth and truncation.
  - Public/MNPI split.
- **District tests:** PO-1…7, PO-12–14, plus these ablations:
  - (i) delete 50% of effort records → bounds must contain truth;
  - (ii) overstate q by 2× → detect the AF447 failure via the hindcast;
  - (iii) inject a novel type → time-to-alarm of the novel-type slot or h₀;
  - (iv) non-ignorable effort → the A2 test must fire.
- **Mine generator.** Geology with an out-of-model late fault → drillholes with FSE and QA/QC defects → estimates → plans → blasting, dilution, stockpiles → plant with measurement error → reconciliation. Also a capex process drawn from the GB-6 mixture against a "±15%" estimate, and a preg-robbing domain absent from testwork.
- **T5-1:** run PQ-5 strategies (a)–(d) and compare coverage over time, time-to-alarm, regret and fabrication count.
- **T5-3 control mine:** correctly specified; it bounds false alarms.
- **Real hindcasts:** Yilgarn discoveries (§2.5), the Bertisen & Davis capex set (needs dataset access; OPEN-7), and one site's F3 history (needs Barrick data; MOR unknowns).

### 4.7 Galaxy-brain cards (illustrative numbers; each card is reproducible from logged evidence)

**GB-1 · Discovery-thinning integral (`thinning`)**
- **Equation:**
  - Λ_undisc = Σ_cells Λ·e^{−H};
  - H = Σ_j e_j q_j;
  - log-likelihood penalty = −Σ Λ(1−e^{−H}).
- **Substituted:** one 400 km² cell, prior 0.5 deposits/1000 km², so Λ = 0.1 per half.
  - Shallow half: soil q = 0.9 + magnetics 0.1 gives H = 1.0, so π = 0.632 and Λ_undisc = 0.037.
  - Deep half (40–80 m transported cover): soil is *insensitive* (A1, q = 0), magnetics 0.1 + aircore 0.15 gives H = 0.25, so π = 0.221 and Λ_undisc = 0.078.
- **Result:**

  | Treatment | P(≥1 undiscovered) |
  |---|---|
  | Thinning model | 1 − e^{−0.115} = **0.108** |
  | Ignore effort | 0.181 |
  | "Explored = barren" (pseudo-absence) | ≈ 0 |

  **68%** of what remains lies under cover. The likelihood penalty is −0.085.
- **Intuition:** you only learn from looking where looking could have worked.
- **Assumptions:** A1, A2, and q at that cover depth.
- **Flip:** if deep-cover aircore q is really 0.40, the deep-half share drops to 0.067 and the ranking of the halves barely moves. If the effort records are incomplete (A4), report Manski bounds [0.108, 0.181].

**GB-2 · Bayesian search after 3 dry holes (`search_update`)**
- **Equation:**
  - Cox mode: Λ′ = Λ·Π(1−d_h).
  - Single-target mode: p′ = p(1−D)/(1−pD).
- **Substituted:** prior P(≥1) = 0.20, so Λ = 0.223; d_h = 0.35 per hole, so the miss probability = 0.65³ = 0.275.
  - **Cox:** Λ′ = 0.0613, so **P′ = 0.059**. Elsewhere is unchanged, apart from a small θ_τ shrink.
  - **Single-target** (D = 0.725): p′ = **0.064**, and every other cell is scaled ×1/(1−0.145) = **×1.17**.
- **AF447 check:** if the true d_h = 0.15 (holes too shallow beneath cover), the truth is P′ = 0.128, so the target was "cleared" 2.2× too hard. A hierarchical d that is 50/50 on {0.35, 0.15} gives P′ = **0.094**.
- **Intuition:** dry holes clear ground only as well as the holes could see. They push probability elsewhere only if you assert that the body exists.
- **Flip:** the d posterior (d < 0.2 → keep the target), or an A3 sub-economic intercept (positive evidence).

**GB-3 · Plug-in vs posterior P(≥1) (`posterior_core`)**
- **Equation:** P = 1 − E[e^{−Λ}]. For Λ ~ Gamma(a, b), P = 1 − (b/(b+1))^a.
- **Substituted:** posterior mean Λ = 0.3, shape a = 0.5, b = 1.667.
  - Posterior: P(≥1) = 1 − 0.625^{0.5} = **0.209**. Plug-in: 1 − e^{−0.3} = **0.259** (overstates by 24%).
  - But P(≥2) is 0.061 (posterior) vs 0.037 (plug-in).
- **Intuition:** uncertainty about the rate makes a *first* discovery less likely and a *camp* more likely.
- **Flip:** an Λ-shape a > 5 makes plug-in adequate (< 3% error).

**GB-4 · EVOI of an IP survey before a 6-hole program (`voi`)**
- **Equation:** VOI = Σ_y P(y)·max(EV_drill|y, 0) − max(EV_drill, 0).
- **Substituted:**
  - Inputs: P(present) = 0.10; program detection 0.70; V_net = $125M (already lead-time-discounted, net of $25M definition); drill $3M; survey $0.4M with sensitivity 0.85 and false-positive rate (FP) 0.30.
  - Drill now: EV = 0.07·125 − 3 = **$5.75M**.
  - P(anomaly) = 0.355. P(present | anomaly) = 0.239, so EV = $17.95M. P(present | no anomaly) = 0.023, so walk away.
  - Survey policy: 6.37 → **VOI $0.62M gross, +$0.22M net. Survey first.**
- **Flip:** VOI_gross = 1.4325 − 2.7·FP, so above **FP ≈ 0.38** you skip the survey and drill. Graphite and clays are chargeable (R7 §2.1), so a black-shale host flips the decision.
- **Assumption:** independence of survey and drill errors given the deposit.

**GB-5 · Heavy-tailed marks: never value with the median (`tail`)**
- **Equation:**
  - V ~ LN(μ, σ) with σ = √(2 ln(mean/median));
  - share of value held by the top decile = 1 − Φ(z₀.₉ − σ).
- **Substituted:** median $40M, mean $150M, so σ = 1.63.
  - Valued at the median, GB-4's drill EV = 0.07·(40−25) − 3 = **−$1.95M (drop)**. At the mean it is **+$5.75M (drill)**.
  - 79% of discoveries are worth less than the mean, and the top decile holds 64% of the value.
- **Singer (1995, FACT[R7]):** the top 10% of deposits hold 71–86% of metal. That implies a lognormal σ of ≈1.8–2.4, and that the tail is probably still under-stated (so use GPD).
- **Flip:** only portfolio-level risk limits (CVaR), not medians, should overrule EV.

**GB-6 · Unknown-unknown budget for capex (`robust_risk`)**
- **Equation:** R = actual/estimate ~ (1−w)·LN(0, s₁) + w·LN(ln r_b, s₂). Fitted to Bertisen & Davis (FACT[R7]: mean +14%, ~½ outside ±15%, ~1/13 ≥ +100%), this gives w = 0.094, s₁ = s₂ = 0.20, r_b = 2.4.
- **Substituted (BFS $2.0B, "±15%"):**

  | Statistic | Value |
  |---|---|
  | P50 | $2.06B |
  | Mean | $2.31B |
  | P90 | $3.2B |
  | P95 | ≈ $4.7B |
  | P(within ±15%) | 0.50 |
  | P(≥ 2×) | 0.077 |

  If NPV = 0 at R = 1.7, then **P(NPV < 0 from capex alone) ≈ 0.094**, against ≈ 0 under the "±15%" claim.
- **Residual not in this reference class** (jurisdiction or security, cf. Reko Diq; regulatory, cf. Pascua-Lama, R6 §6.2–6.3): a **FlipMargin, P not stated**.
- **Intuition:** most of the "unknown unknown" in capex is a *known* miscalibration, which the outside view converts.
- **Flip:** a project-specific reference class (e.g., FEL-3 maturity strata) with n ≥ 30.

**GB-7 · Ellefsen (2026) → terrane hyperprior and partial pooling (`shrinkage`)**
- **Equation:**
  - expected range of 16 normal draws ≈ 3.53σ, so σ_τ ≈ (ln 5.53 + ln 26.6)/3.53;
  - pooling weight on local data = σ_τ²/(σ_τ² + 1/n).
- **Substituted:** σ_τ ≈ (1.71 + 3.28)/3.53 = **1.41**, so an unexplored terrane's intensity is uncertain by ×**16** either way at 95%. The local weight is 0.67 at n = 1 and 0.91 at n = 5 deposits.
- **Intuition:** the honest analogue prior is wide, and a few well-explored local deposits dominate it fast.
- **Assumption:** the ratio statistics proxy tract log-intensity differences (HYPOTHESIS).
- **Flip:** a direct hierarchical fit on ≥30 tracts replaces this.

**GB-8 · Anytime-valid coverage alarm (`e_fdr`)**
- **Equation:** e = B(k+1, n−k+1) / (0.8^k·0.2^{n−k}), a uniform-mixture Bernoulli confidence sequence, alarming at e ≥ 1/α = 20 (franken-decision default).
- **Substituted:** monthly 80% intervals held in 7 of 12 months gives e = **1.45**; 14/24 gives **4.5**; 36/60 gives **84 → alarm**.
- **Intuition:** "7 of 12 looks bad" is not yet evidence. Monthly resolution is too slow, so score at a finer conditionally independent support and pair with CUSUM.
- **Flip:** within-month dependence (A8) voids finer scoring, so coarsen.

**GB-9 · Novel deposit-type mass via Good–Turing (`info_budget`)**
- **Equation:** P(next discovery is of an unseen type) ≈ n₁/N.
- **Substituted:** N = 120 regional discoveries in the versioned type catalogue; n₁ = 3 types seen exactly once, so **ε_type = 0.025** of the discovery rate goes to the k* slot. Precedent: IOCG was a new type at Olympic Dam in 1975 (FACT[U]).
- **Intuition:** the catalogue's own singletons price its incompleteness.
- **Flip:** a catalogue revision that reclassifies the singletons.

---

## 5. Plan-ready material

### 5.1 Crates (`fm-` prefix; phase 1 unless noted) and consumption posture

| Crate | Owns | Consumes |
|---|---|---|
| `fm-claim` | `Claim`, classes, states, `UuTerm`, no-laundering algebra, Color/EvidenceLedger projection, guards G-UU/G-EXEC/G-DISC/G-MNPI | lifted fs-evidence (E002/E003 provenance-header pattern), franken-evidence (dependency, never forked) |
| `fm-refclass` | Reference-class registry and fits (capex mixture, schedule, ramp-up) with [V]/[U] tags | fsci-stats |
| `fm-calib` | Forecast & calibration ledger, scoring, PIT, corrections | lifted fs-eproc; fm-claim |
| `fm-surprise` | Monitors, null race, UU→KU automaton, InvestigationCase hooks | fs-eproc; franken-decision |
| `fm-epimap` | Executive epistemic map, WWCOM card, galaxy-brain cards | franken-evidence render |
| `fm-source` | Data-asset registry, unknown-known gap and digitization VOI | fm-claim |
| `fm-explore-data` | `DepositRecord`, `EffortRecord`, aggregation rules, bitemporal catalogue | fsqlite R-tree (E014) |
| `fm-ppm` | Mesh/SPDE, thinned marked Cox likelihood, Laplace, marks (spliced/compositional), dual basis | lifted fs-sparse, fm-infer, fsci-stats |
| `fm-infer` | HMC/NUTS, elliptical slice, SMC, SBC harness (none exist in the suite, E018) | frankenjax autodiff (optional) |
| `fm-explore-voi` | Search modes, preposterior VOI, portfolio greedy plus certificate | fm-ppm |
| `fm-hypothesis` | Hypothesis sets, h₀ floor, prior-predictive checks | fm-claim |
| `fm-synth`, `fm-hindcast` | Synthetic district/mine; as-of hindcast harness | everything (test-only) |
| `fm-pomdp` (phase 2) | Prospect-scale POMCPOW/SARSOP | fm-explore-voi |

### 5.2 Gates (TARGET) and kill gates

**Gates:**
- **G-P1 / PO-7:** synthetic coverage within ±3 pts (≥200 replicates); SBC p > 0.01.
- **G-P2:** MapMark4 parity (KS < 0.02).
- **G-P3:** in ≥1 public district, (b) beats N0 and (a) on held-out point-process log score, with the paired race e ≥ 20 or a 95% bootstrap CI excluding 0; the gain is concentrated in low-effort strata; decile calibration within the 80% band.
- **G-P4:** ≥60% area-weighted effort digitized at coverage ≥ Medium before any *absolute* P leaves the lab.
- **G-P5:** metamorphic suite 100%.
- **G-P6:** 10⁶-node 2D Laplace fit < 1 h on one workstation; polygon query < 1 s.
- **G-E1:** no-laundering proptests 100%.
- **G-E2:** 100% of resolvable decision claims registered before resolution; as-of replay bit-identical.
- **G-E3:** T5-1 — the stack restores 80% coverage to ±5 pts within 12 resolutions after alarm, while strategy (a) documentably fails.
- **G-E4:** zero ε without a `RefClass`; zero [U] reference classes on disclosure-facing claims.
- **G-E5:** T5-3 — false alarms ≤ α; sharpness loss < 10%.

**Kill gates:**

| Gate | Trigger | Deterministic fallback |
|---|---|---|
| K1 | (b) fails G-P3 in 2 districts | N0 + (a) ranking labelled `Uncalibrated`; thinning only as a tract-level effort adjustment |
| K2 | G-P4 unreachable for any target district within 6 months | Ship Manski bounds + an unknown-knowns digitization VOI list as the exploration product |
| K3 | PO-4/G-P6 fail | Vecchia or coarse mesh, or tract mode |
| K4 | `fm-pomdp` < 20% hole saving vs myopic VOI at equal accuracy | Myopic VOI |
| K5 | Executive user test rejects the map | Three-field card: range · evidence class · flip margin |
| K6 | Monitors > 1 false alarm per site per quarter in the control | Re-specify the support, CUSUM-only for fast shifts |

### 5.3 Risks and OPEN questions

| ID | Question |
|---|---|
| OPEN-1 | Effort-data availability and licensing (WAMEX/MINEDEX, OGS, NBMG, Barrick archives). This is the decisive risk. |
| OPEN-2 | Effort endogeneity (A2): the magnitude in real districts is unknown. |
| OPEN-3 | 3D under-cover LGCP scale. |
| OPEN-4 | Pure-Rust nested-dissection ordering + selected inversion for marginal variances. fs-sparse has AMD + LDLᵀ only. |
| OPEN-5 | The value function V(m) for undiscovered deposits, including lead time, P(develop) and deal terms. This is the PQ-7 interface. |
| OPEN-6 | MNPI and JV data-room policy with Barrick legal; whether DP noise is ever needed for partner sharing. |
| OPEN-7 | Verified reference-class datasets (Bertisen & Davis microdata; McNulty ramp-up, Gypton and others are [U]). |
| OPEN-8 | Whether CPs/QPs and the ITRB accept quantitative evidence beside their categories. |
| OPEN-9 | IAS 1.125 / IAS 37 projection policies with controllers (FACT[U]). |
| OPEN-10 | Whether the franken-evidence schema should gain an optional claim-reference field upstream, or FM keeps projecting. The default is to project (consume, never fork, E015). |
| OPEN-11 | Elicitation programme staffing (IDEA); how to score named experts with care. |
| OPEN-12 | frankensim convergence (E001). The crates FM needs (fs-evidence, fs-eproc, fs-voi, fs-robust, fs-sparse) are all on the E003 liftable list; fs-oed-e2e is not, and is not needed. |

**Checked nulls** (deliberately not novel): the thinned point process (R7 §1.7), the POMDP (Mern/Caers), MapMark parity, the claim object (R7 §9.5), and the calibration ledger (R7 §9.5). This pass adds the decisions, guards, typed modes, fallbacks and numbers that make them buildable and honest.
