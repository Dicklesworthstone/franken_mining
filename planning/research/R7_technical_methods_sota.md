# R7: Technical Methods, State of the Art (Exploration to Closure)

**Project:** FrankenMining, a pure-Rust, evidence-first operating system for integrated metal miners (Au, Cu, Ag, Sn)
**Report date:** 2026-10-28
**Purpose:** Ground the normative design sections. For each technical domain this report covers the state-of-the-art methods, key references, known limitations and pitfalls, and what a rigorous software implementation needs (data structures, algorithms, validation tests).
**Emphasis:** Topic 1 (Bayesian prospectivity with knowledge transfer and negative evidence) and Topic 9 (uncertainty propagation and epistemics).

---

## How to read this report: verification legend

Every factual claim carries one of three tags.

| Tag | Meaning |
|---|---|
| **[V]** | Verified online during this research session. Checked against a publisher, DOI registry (Crossref/OpenAlex), arXiv, USGS, regulator or vendor page, or full text. A DOI or URL is given. |
| **[V-sec]** | Verified only through a secondary source, such as a reference list, abstract or search snippet. The primary text was not read. |
| **[U]** | UNVERIFIED. From the author's background knowledge. Believed correct, but bibliographic details (year, venue, pages) or numbers may be wrong. Re-check before using normatively. |

Untagged sentences labelled *Design implication* or *Recommendation* are the author's own synthesis. They are not literature claims.

**Research-process caveat.** The session's shared web-search quota ran out partway through. Later verification used direct lookups of known identifiers only: Crossref, OpenAlex and Semantic Scholar by DOI, and direct fetches of known publisher, USGS, arXiv and company pages. Some well-known classical references therefore remain **[U]** only because they were never looked up. Two research sub-agents covered topics 2–8 and checked about 200 further citations. Their verified items are tagged [V] and their unverifiable items [U].

---

## 0. Executive summary (decision-relevant points)

1. **Undiscovered-resource assessment.** The USGS three-part method (permissive tracts + grade–tonnage models + number of undiscovered deposits) is the accepted public-sector baseline. It is a special case of a Bayesian marked point-process model.
   - In MapMark4, the deposit-count distribution is a negative binomial fitted to elicited 90/50/10 percentiles from several weighted experts [V]. That is a Poisson–Gamma mixture, i.e. a Poisson process with an uncertain intensity.
   - FrankenMining should implement the three-part method as a degenerate configuration of a more general model. That gives backward compatibility and regulator familiarity.
2. **The principled generalization is a hierarchical, marked, thinned spatial point process.**
   - Latent intensity: a log-Gaussian Cox process on mineral-system covariates.
   - Marks: tonnage and compositional grade.
   - Thinning: an explicit *detection/discovery* model that depends on exploration effort, cover depth, deposit size and era.
   - **Negative evidence enters only through the thinning model.** "No deposit found here" is informative only in proportion to how hard and how well people looked. This is Bayesian search theory (Stone et al., Air France 447 search [V]), and ecology has a mature equivalent for presence-only data with observer bias [V].
   - **Without exploration-effort data, absence of evidence is confounded with maturity.** Hronsky & Kreuzer (2019) name exactly this as the field's practical failure: "bias to mature, well-known areas" [V].
3. **Knowledge transfer from explored to unexplored ground should be partial pooling, not model copying.**
   - Use terrane- or tract-level parameters drawn from deposit-type-level hyperpriors, with similarity-weighted borrowing.
   - **Pooled data are not safe as-is.** USGS (Ellefsen 2026) found pooled worldwide data versus tract-local data gave practically non-equivalent predictions in 14 of 16 past assessments, with ratios from 26.6× smaller to 5.5× larger [V].
   - Transferability is real but incomplete:
     - Porphyry-Cu models trained in North vs South America differ markedly [V].
     - Fine-tuning national models with regional ground truth changes local outcomes [V].
     - Leave-one-cluster-out validation shows performance drops [V].
4. **Validation of any prospectivity model must be spatially honest.**
   - Required: blocked, cluster-held-out and *temporal hindcast* validation (train on deposits known at year T, score discoveries after T).
   - Use PR-AUC and proper scores rather than ROC-AUC, because positive rates are about 1% or less.
   - Add calibration and coverage checks, e.g. conformal methods.
   - Random cross-validation is demonstrably over-optimistic [V].
5. **Decision-theoretic exploration is now demonstrated in practice, not just in theory.**
   - Stanford Mineral-X with KoBold Metals: Intelligent Prospector v1.0, a POMDP with a particle-filter belief and POMCPOW, in GMD 2023 [V].
   - v2.0 adds multiple geological hypotheses plus a max-entropy "null hypothesis" that detects when *all* human hypotheses are falsified. It was solved with SARSOP and was credited with helping characterize the ultra-high-grade Mingomba Cu deposit, Zambia, 2023 [V].
   - In the synthetic test, the POMDP needed **fewer than half as many holes as grid drilling** for the same decision accuracy [V].
   - Caers' 2025 essay sets out a Popper–Bayes protocol:
     1. Multiple hypotheses.
     2. Falsification by prior-predictive tests.
     3. Stochastic, not deterministic, inversion.
     4. VOI- or EOI-driven acquisition.
     5. The ordering "hypotheses → volume → grade" [V].
   - This protocol fits FrankenMining's evidence-first mandate closely.
6. **Uncertainty propagation in industry is typically broken at every hand-off.** The chain: deterministic kriged block model → single pit at one price → deterministic schedule → DCF with one-at-a-time sensitivities. Documented consequences:
   - Bertisen & Davis (2008), 63 projects:
     - As-built capex averaged **+14%** over the bankable feasibility study estimate.
     - **About half** fell outside the claimed ±15% accuracy.
     - **About 1 in 13** overran by at least 100%.
     - The bias has not attenuated over time, and the authors argue it is *intentional* (financing-driven).
     - They recommend a shifted lognormal for capex in Monte Carlo [V].
   - Conventional pit designs evaluated against simulations miss their forecast NPV, tonnage and grade (Dimitrakopoulos et al. 2002) [V-sec].
   - In Bond et al. (2007), only **21%** of 412 geoscientists correctly interpreted a synthetic seismic section [V]. Conceptual (model-structure) uncertainty dominates.
7. **Reporting codes are moving toward explicit uncertainty, but slowly.**
   - JORC 2012 is still in force (Oct 2026). The JORC draft (Aug 2024; "provisional JORC Code 2026" being finalized) would:
     - Define Risk as "the effect of uncertainty."
     - Drop "eventual" from RPEE in favour of "more likely than not."
     - Mandate reconciliation disclosure.
     - Require rounding that reflects uncertainty [V].
   - CRIRSCO Template, June 2024 update: "eventual" removed [V].
   - NI 43-101 repeal-and-replace proposed in June 2025; not final [V].
   - Resource classes (Measured/Indicated/Inferred) remain *qualitative confidence labels*, not probabilities. FrankenMining must map between the two explicitly and must never pretend they are the same thing.
8. **Epistemic accounting needs machinery for "unknown unknowns," not just labels.**
   - Explicit catch-all hypotheses with non-zero prior mass.
   - Model-discrepancy terms (Kennedy & O'Hagan) [V].
   - Generalized or tempered posteriors under misspecification [V].
   - Reference-class "outside view" priors (Flyvbjerg; Bertisen & Davis) [V].
   - Continuous **forecast-scoring against reconciliation**: a calibration ledger that turns F1/F2/F3 factors into a learned model of estimator bias.
9. **Software implications common to all domains:**
   - Every quantity is a random variable carrying *support* (volume or scale), units, provenance and evidence class.
   - Realization ensembles are the lingua franca between modules.
   - Every model ships with synthetic-truth tests, simulation-based calibration and hindcast scoring.
   - Open reference implementations exist for parity testing, including MineFlow (pseudoflow, MIT licence) [V], GemPy, LoopStructural, SimPEG [V] and MapMark4 [V].

---

## 1. Mineral prospectivity and undiscovered-resource assessment

### 1.1 The problem stated in Bayesian terms

For a deposit type *k* (e.g., porphyry Cu–Au, orogenic Au, sediment-hosted Cu, Sn–W greisen/skarn, epithermal Ag–Au) and a region *R*, we want:

1. The posterior distribution of the **number, locations and sizes** (tonnage, grade vector) of *undiscovered* deposits in *R*.
2. The **probability that a given cell or target contains an economic deposit** above a size/grade threshold, given all evidence.
3. The **value of the next measurement** (survey, geochem campaign, drillhole), given the decision context (walk away, acquire, drill, develop).

The core difficulties:

- **Rare events.** Positive rates are about 1% of cells or less. Ahn et al. (2026) had 818 positive cells out of 70,200, i.e. 1.17% [V].
- **Presence-only labels.** Known deposits are positives, but "no known deposit" is not a negative.
- **Discovery bias.** Labels concentrate where people looked, where cover is thin, and where deposits are big.
- **Mixed-support covariates.** Geophysics at about 100 m line spacing vs core at 15 cm; Caers (2025) shows that deterministic interpolation halves variance and *induces spurious label–predictor correlation* [V].
- **Non-stationarity** across terranes.
- **Heavy-tailed size distributions.** The top 10% of deposits hold 71–86% of discovered metal, depending on commodity (Singer 1995) [V].

### 1.2 USGS three-part quantitative assessment (public-sector baseline)

**Method** [V: USGS pubs; Wikipedia summary with references]:
1. Delineate **permissive tracts** for each deposit type, guided by descriptive deposit models.
2. Build a **grade–tonnage model** for each type from frequency distributions of well-explored deposits.
3. Estimate the **number of undiscovered deposits** per tract, either:
   - by expert judgement (multiple assessors give N90/N50/N10, i.e. counts at elicitation probabilities 0.9/0.5/0.1), or
   - by **deposit-density models** built from well-explored control tracts (Singer 2008, *Math. Geosci.* 40:33–46, doi:10.1007/s11004-007-9127-3 [V]).

**Key references:**
- Singer & Menzie (2010), *Quantitative Mineral Resource Assessments: An Integrated Approach*, Oxford UP [V-sec].
- Cox & Singer (1986), *Mineral Deposit Models*, USGS Bull. 1693 [V-sec].
- Root, Menzie & Scott (1992), the MARK3 Monte Carlo simulator [V-sec].

**MapMark4** (Ellefsen 2017, USGS TM 7-C14, doi:10.3133/tm7C14) [V, user guide read]:
- An R package that replaced MARK3. It has six classes: metadata, tonnage pdf, grade pdf, NDeposit pmf, and the simulation and summary classes.
- **Deposit-count pmf:** a *negative binomial* fitted to the weighted N90/N50/N10 estimates of team members. The fitted pmf is recast as elicitation percentiles for a visual check against each expert's numbers.
- **Tonnage pdf:** "normal" (on log scale) or "kde"; kde is suggested when there are about 50 or more discovered deposits; optional truncation.
- **Grades:** treated as **compositional data** (metal grades + "gangue" summing to 100%), with a compositional mean and variation matrix. The correlation between ore tonnage and metal tonnage is reported (often about 0.9).
- **Output:** Monte Carlo distribution of total undiscovered ore and metal per tract.

**Consistency rule** [V, USGS "Basic concepts"]:
- "Prevention of bias requires that deposits of each type be defined the same way" in tract delineation, grade–tonnage models and density models.
- USGS uses spatial aggregation rules for this. The commonly cited porphyry example merges deposits within 2 km [U].
- *Design implication:* deposit identity and aggregation rules must be **versioned first-class objects**, applied identically to training labels, size models and outputs.

**New evidence on transfer (Ellefsen 2026)** [V]:
- Source: USGS SIR 2026-5031, doi:10.3133/sir20265031.
- **Design:** 16 past assessments recomputed using *pooled* (worldwide) vs *tract-local* deposit data, with an equivalence margin of a factor of 1.5.
- **Result:** only 2 of 16 were practically equivalent. In the other 14, pooled-data statistics ranged from 26.6× smaller to 5.53× larger. Differences ran in both directions.
- This is direct evidence that **naive pooling is a large, unmodelled source of uncertainty**. The principled fix is hierarchical partial pooling (§1.8).

**Scale of stakes** [V]:
- USGS global assessment (Johnson, Hammarstrom, Zientek & Dicken 2014, FS 2014-3004, doi:10.3133/fs20143004): mean undiscovered Cu of about **3,500 Mt** across 225 tracts, for porphyry and sediment-hosted types (about 80% of world Cu supply).
- Other three-part applications: Finland (Rasilainen et al. 2017), the Urals (Hammarstrom et al. 2017) and Victoria (Lisitsin 2010) [V-sec].

**Limitations:**
- **Subjective deposit counts.** Calibration of the experts is rarely scored; MapMark4 weights experts but does not score them.
- **Coarse tracts.** Tracts carry no within-tract spatial resolution.
- **Implicit treatment of exploration history.** Exploration maturity enters only through expert judgement, and there is no explicit detection model.
- **Grade–tonnage models conflate deposits of different completeness.** Resources of known deposits grow with time. Guj et al. (2011) document Kalgoorlie's Golden Mile doubling from 36 to 72 Moz [V].
- **Economic truncation.** Small deposits are under-reported, so the observed size distribution is left-truncated [U, discovery-process literature, e.g. Schuenemeyer & Drew 1983].

### 1.3 Mineral systems approach (knowledge structure for priors)

- **Origin.** Wyborn, Heinrich & Jaques (1994), "Australian Proterozoic mineral systems: essential ingredients and mappable criteria" [U]. Operationalized by McCuaig, Beresford & Hronsky (2010), *Ore Geol. Rev.* 38:128–138, doi:10.1016/j.oregeorev.2010.05.008 [V].
- **Core idea.** A deposit requires co-occurrence of:
  - **source** (fluids, metals, ligands),
  - **energy/driver**,
  - **pathway/architecture** (lithosphere-scale structures),
  - **trap/depositional** physical or chemical gradients,
  - **preservation**.
  Each is translated into **mappable proxies** at a given scale.
- **National-scale implementations:**
  - Lawley et al. (2022), *Ore Geol. Rev.* 141:104635 [V]: sediment-hosted Zn–Pb across Canada, the USA and Australia on a discrete global grid. Surface-wave tomography and satellite-gravity derivatives map favourable paleo-tectonic settings, and **the most prospective combination of geophysical datasets varies by region and deposit type**, an explicit non-stationarity finding.
  - Lawley et al. (2021) magmatic Ni (Canada) [V].
  - Skirrow et al. (2019), knowledge-driven IOCG potential for Australia [V-sec].
  - Ford et al. (2023), carbonatite REE (Australia) [V-sec].
- **Design implication.** The mineral-system decomposition is the natural **factorization of the prior**. Each critical ingredient becomes a latent field, and the deposit intensity is (roughly) their conjunction. This keeps the model interpretable and lets expert knowledge enter as priors on ingredient presence, not on opaque ML weights.

### 1.4 Method families for prospectivity mapping

| Family | Representative refs | What it is | Key pitfalls |
|---|---|---|---|
| Weights of evidence (WofE) | Bonham-Carter, Agterberg & Wright (1989) [U]; Bonham-Carter (1994) GIS book [V-sec]; Agterberg & Cheng (2002), *NRR* 11:249–255, conditional independence test [V]; Carranza (2004) small-sample WofE [V] | Bayesian log-odds update per binary or multiclass evidence layer, assuming **conditional independence** given deposit presence | Violated conditional independence inflates posterior probabilities and is common with correlated geophysical layers. Thresholding of continuous predictors is arbitrary (Baddeley et al. 2020 [V] analyse optimal thresholding). Training points must be "coherent" deposits (Carranza et al. 2007 [V-sec]). |
| Logistic regression and its point-process equivalent | Baddeley et al. (2010), *EJS* 4:1151–1201: spatial logistic regression ≈ Poisson point process, with change-of-support issues [V] | Models deposit intensity as a function of covariates | Results depend on pixel size unless formulated as a point process |
| Fuzzy logic (knowledge-driven) | Porwal, Carranza & Hale (2003) [V-sec]; Lisitsin, Porwal & McCuaig (2014) probabilistic fuzzy logic with Monte Carlo [V]; Burkin et al. (2019) interpretation-uncertainty grading [V] | Expert membership functions combined by fuzzy operators (AND, OR, gamma) | Outputs are not probabilities. Operator choice is ad hoc. The Monte Carlo variants partly address this. |
| Random forest / gradient boosting | Rodriguez-Galiano et al. (2015), *Ore Geol. Rev.* 71:804–818 [V]; Carranza & Laborte (2015), RF with few prospects [V]; Yin & Li (2022), ensembles with Bayesian hyperparameter optimization [V] | Discriminative classifiers on pixels or voxels | Need negatives, usually random "pseudo-absences". Class imbalance (Prado et al. 2020 [V-sec]). Overfitting to local analogues. Uncalibrated scores. |
| Deep learning (CNN, GNN, transformers, foundation models) | Zuo & Xu (2022) graph DL [V]; Yang, Zuo & Kreuzer (2024) review, *Earth-Sci. Rev.* 258:104941 [V]; Daruna et al. (2024) GFM4MPM self-supervised pretraining (MVT and CD Zn–Pb, N. America and Australia) [V]; Parsa et al. (2025) LLMs and geoscience transformers for Canadian critical minerals [V-sec] | Representation learning from multi-modal rasters | Over-parameterized relative to a handful of positives. Self-supervision helps robustness [V]. Explanations (SHAP and similar) are not causal. |
| Bayesian neural nets / MC dropout / ensembles | Kirkwood et al. (2022), *Math. Geosci.* 54:507–531 [V]; Huang, Zuo & Wang (2022) Bayesian CNN for geochemical anomalies [V]; Veshchezerova et al. (2026), arXiv 2609.09246, 3D Bayesian DL of Cu grade, chargeability and resistivity at Kogodai [V] | Approximate epistemic uncertainty | MC-dropout "uncertainty" is often miscalibrated, so it must be checked empirically (§1.6) |
| PU learning | Elkan & Noto (2008), KDD [V]; Bekker & Davis (2020) survey, *Mach. Learn.* 109:719–760 [V]; Xiong & Zuo (2021), *Comput. Geosci.* 147:104667 [V]; Zhang, Wang & Carranza (2022) bagging-PU with cost-sensitive Bayesian LR [V]; Alfonso, Müller & Mather (2024), *GSA Bull.*, spatio-temporal porphyry Cu PU [V]; Ahn et al. (2026), PU + nested spatial CV + conformal [V] | Treats unlabeled cells as a mixture of negatives and undiscovered positives | Standard PU assumes labels are "selected completely at random" (SCAR). Discovery is *not* random: it depends on access, cover and size. SAR or SNAR extensions or explicit detection models are needed (§1.7). |
| Uncertainty and consensus methods | Zuo et al. (2021), uncertainty types in MPM, *NRR* 30:3059–3079 [V]; Zhang, Lawley et al. (2024), *workflow-induced uncertainty*, *NRR* [V]; Nwaila et al. (2024, 2025), consensus national maps and robust uncertainty [V]; Yousefi, Lindsay & Kreuzer (2024), majority voting and confidence index [V-sec] | Ensembles over workflows, algorithms and feature sets | Zhang et al. show that "typical geodata-science MPM workflows contain substantial local minima": arbitrary workflow choices yield highly discriminating but spatially different maps [V]. |

**Critical assessments every implementer should read:**
- **Hronsky & Kreuzer (2019),** *Ore Geol. Rev.* 107:647–653 [V, abstract read]. Prospectivity modelling is "not yet widely used or accepted" because of "bias to mature, well-known areas" and "excessively large areas of high-prospectivity." The authors attribute this mainly to *input data*: non-uniform representation of the search space, omission of deep ore-controlling structures, and "a large degree of unrecognised dependence." They advocate *intelligence amplification* (human + machine) over autonomous AI.
- **Caers (2025),** "The future of AI in critical mineral exploration," arXiv 2512.02879 [V, full text read]. Common errors he lists:
  - upscaling core labels to the geophysical scale;
  - deterministic interpolation of geophysics/geochemistry, which cuts variance (about ½ in his REE example) and inflates correlation;
  - merging zones that cannot be explained geologically;
  - applying a model trained in area A to area B "without testing the validity of doing so."
  Prospectivity maps "have a significant number of false positives."

### 1.5 Labels: presence-only data, point processes and the ecology equivalence

Statistical ecology has worked through exactly FrankenMining's problem for 15 years. Mineral deposits are "species occurrences": presence-only records collected with heterogeneous effort.

- **Warton & Shepherd (2010),** *Ann. Appl. Stat.* 4:1383–1402 [V]. Poisson point-process models "solve the pseudo-absence problem." Logistic regression with pseudo-absences converges to a PPP as the number of background points grows.
- **Fithian & Hastie (2013),** *Ann. Appl. Stat.* 7:1917–1939 [V]. Finite-sample equivalence of MaxEnt, PPP and logistic regression for presence-only data.
- **Renner et al. (2015),** *Methods Ecol. Evol.* 6:366–379 [V]. A practical guide to PPP for presence-only data (Baddeley is a co-author, and he also works on mineral prospectivity).
- **Fithian, Elith, Hastie & Keith (2015),** *MEE* 6:424–438 [V]. **Bias correction by pooling** survey (presence/absence, known effort) and opportunistic presence-only data across multiple species: a shared observer-bias field is learned jointly. Mineral analogue: deposit types share an "exploration effort" field.
- **Dorazio (2014),** *Global Ecol. Biogeogr.* 23:1472–1484 [V]. Imperfect detection and survey bias in presence-only data.
- **Chakraborty et al. (2011),** *JRSS C* 60:757–776 [V]. Point-pattern modelling for "degraded" presence-only data over large regions, with explicit thinning by sampling effort and land transformation.
- **Isaac et al. (2020),** *TREE* 35:56–67 [V]. Integrated models combining heterogeneous data sources with a shared latent process.
- **Log-Gaussian Cox process:** Møller, Syversveen & Waagepetersen (1998), *Scand. J. Stat.* 25:451–482 [V].
- **INLA:** Rue, Martino & Chopin (2009), *JRSS B* 71:319–392 [V].
- **SPDE/GMRF representation of Matérn fields:** Lindgren, Rue & Lindström (2011), *JRSS B* 73:423–498 [V].
- **Grid-free LGCP inference:** Simpson et al. (2016), *Biometrika* 103:49–70 [V].
- **Mineral-specific point-process work:** Baddeley et al. (2010) [V] and Baddeley et al. (2020) [V]; Coolbaugh, Raines & Zehner (2007), "Assessment of exploration bias in data-driven predictive models and the estimation of undiscovered resources," *NRR* 16, doi:10.1007/s11053-007-9037-6 [V, title only].

**Design implication.** Model deposits as a **marked point process**, not a pixel classifier. Pixel classifiers can remain as fast *feature extractors* or *proposal generators*. The probabilistic object of record should be an intensity surface with a declared support, whose integral over any polygon gives an expected deposit count with uncertainty. This lets the system:
- answer "expected number of undiscovered ≥ X Mt deposits in this licence" consistently across scales (no change-of-support paradox, per Baddeley et al. 2010);
- connect directly to the USGS three-part formalism (§1.12);
- add a principled detection/thinning layer for negative evidence (§1.7).

### 1.6 Validation: spatially honest, temporally honest, calibrated

**Spatial dependence breaks random cross-validation:**
- Roberts et al. (2017), *Ecography* 40:913–929 [V]: blocking strategies for structured data.
- Ploton et al. (2020), *Nat. Commun.* 11:4540: "spatial validation reveals poor predictive performance" of large-scale mapping models [V].
- Wadoux, Heuvelink, de Bruin & Brus (2021), *Ecol. Model.* 457:109692 [V]: spatial CV is not unbiased for *map accuracy* either. Under probability sampling, design-based validation is preferred. The fair conclusion is that the validation scheme must match the *prediction task* (interpolation vs extrapolation).
- Meyer & Pebesma (2021), *MEE* 12:1620–1633 [V]: the **area of applicability (AOA)**, a dissimilarity index in feature space flagging where a model is extrapolating.

**Mineral-specific evidence:**
- **Oh, Bang & Kim (2026),** *Ore Geol. Rev.* 107313 [V]. Compare random, spatially blocked and convex-hull **leave-one-cluster-out (LOCO)** CV for Canadian magmatic Ni:
  - Apparent performance falls as validation gets stricter.
  - Conventional schemes "may reward local analogue matching rather than more transferable geological relationships."
  - First-order maps are similar, but **the highest-priority hotspot cells change**, which are exactly the cells where money gets spent.
- **Ahn et al. (2026),** *NRR* [V]: nested fivefold spatial block CV with 50 km blocks from an empirical variogram, Yilgarn Ni:
  - Outer-test PR-AUC is 0.195 against ROC-AUC 0.911. This illustrates why **ROC-AUC flatters rare-event models**.
  - Cross-conformal calibration achieves empirical coverage above the nominal 85%.
  - BaggingPU gives more stable top-1% predictions (median relative IQR 18% vs 48%).
- **Nykänen et al. (2015)** use ROC as a validation tool for prospectivity [V]. Treat it as a minimum, not a sufficient, check.

**Temporal hindcast (the gold standard for discovery models):**
- Guj, Fallon, McCuaig & Fagan (2011), *Econ. Geol.* 106:241–259 [V], is the template. A Zipf model fitted to Yilgarn gold deposits known in 1973, 1989, 2003 and 2008 "would have provided remarkably realistic predictions" of later discoveries.
- The same study shows the model is sensitive to growth of the rank-1 deposit, and that predictions were conservative because resources grow.

**Calibration and coverage tools:**
- Proper scoring rules: Gneiting & Raftery (2007), *JASA* 102:359–378 [V].
- Calibration/sharpness paradigm: Gneiting, Balabdaoui & Raftery (2007), *JRSS B* 69:243–268 [V].
- Conformal prediction: Angelopoulos & Bates (2023) [V]; spatial conformal: Mao, Martin & Reich (2024), *JASA* 119:904–914 [V].
- Bayesian model comparison: PSIS-LOO, Vehtari, Gelman & Gabry (2017) [V]; stacking, Yao et al. (2018) [V].

**Required validation suite for FrankenMining prospectivity models (normative proposal):**
1. **Synthetic-world tests.** Simulate a known intensity, detection process and size marks. Check posterior coverage of the true undiscovered count and endowment, with simulation-based calibration (Talts et al. 2018, arXiv:1804.06788 [U for bibliographic detail]).
2. **Blocked and LOCO cross-validation,** with block size justified by the empirical spatial correlation range.
3. **Temporal hindcast.** Freeze the evidence and label set at year T; score held-out discoveries T→T+Δ with the point-process log score, the expected count, and the rank of the discovery cell.
4. **Rare-event metrics.** PR-AUC, capture-efficiency curves (fraction of discoveries vs fraction of area), and expected count calibration per decile of predicted intensity.
5. **AOA/extrapolation flags.** Every prediction outside the AOA is marked "transferred (extrapolated)" with a widened posterior or a separate evidence class.
6. **Workflow-perturbation ensembles** (after Zhang et al. 2024). Report variability over defensible workflow choices as an explicit uncertainty component.

### 1.7 Discovery bias, detectability and negative evidence ("absence of evidence")

**Principle.** An observation of "no deposit" at location *s* updates belief only through the probability that a deposit there *would have been found* had it existed.

**Bayesian search theory.** Koopman, and Stone's *Theory of Optimal Search* [U for book details], give the canonical form. With prior probability *p* that a target is in cell *i* and detection probability *d* for the search done there, an unsuccessful search updates to:

> p′ = p(1 − d) / (1 − p·d)

and renormalizes all other cells upward. Stone, Keller, Kratzke & Strumpfer (2014), *Stat. Sci.* 29:69–80 [V], describe how this located Air France 447 after earlier searches had failed. Part of the success came from re-examining the assumed detection probabilities of earlier searches, which turned out to be wrong [U for this detail: the beacon-failure hypothesis that lowered prior-search detection probability].

**Point-process generalization (proposed FrankenMining core model):**

- True deposits of type *k*: a marked point process with intensity λₖ(s), marks *m* = (tonnage T, grade vector G, depth z, style).
- Discovery by time *t*: an independent thinning with probability

  > π(s, m, t) = 1 − exp(−∫₀ᵗ Σⱼ effortⱼ(s, τ) · qⱼ(m, cover(s), τ) dτ)

  where *j* indexes exploration methods (outcrop prospecting, stream geochem, soil, airborne mag/EM, ground IP, drilling); effortⱼ is the recorded intensity of method *j* (line-km, samples/km², metres drilled); and qⱼ is the per-unit-effort detection efficiency. qⱼ depends on deposit size and footprint (bigger is more detectable), depth of cover, and method era (e.g., the arrival of deep-penetrating EM).
- Observed deposits form a PPP with intensity λₖ(s)·π(s, m, t). **Undiscovered deposits** form an independent PPP with intensity λₖ(s)·(1 − π(s, m, t)), because thinned Poisson processes split into independent parts.
- Log-likelihood:

  > Σ_obs log[λ(sᵢ) π(sᵢ, mᵢ, t) f(mᵢ)] − ∫_R ∫_M λ(s) π(s, m, t) f(m) dm ds

  The integral term is where **negative evidence lives**. Heavily explored, shallow-cover areas with no discoveries contribute a large expected count with zero observations, which pulls λ down there. Unexplored or deep-cover areas contribute almost nothing, so their posterior stays near the (transferred) prior.
- This also gives the **"biggest deposits found first" effect** without ad hoc corrections. Detection increases with size, so early discoveries are size-biased. This is the mineral analogue of the Kaufman–Balcer–Kruyt (1975) successive-sampling discovery-process model, "sampling proportional to size without replacement" [U], used for decades in petroleum play assessment (e.g., Drew 1997 [U]).

**Practical issues:**
- **Effort data are the bottleneck.** They come from drillhole databases (collar, depth, date), geophysical survey footprints and specifications, geochemical sample locations and detection limits, tenement and title history, and report archives. Hronsky & Kreuzer's "non-uniform representation of the search space" [V] is, formally, missing effort covariates.
- **Detection efficiencies qⱼ are themselves uncertain.** Calibrate them by:
  - hindcasting (what fraction of deposits now known under cover *X* would method *j* have flagged?);
  - forward modelling of deposit signatures (e.g., the EM response of a given conductor at depth *z*);
  - expert elicitation.
  Treat them as hierarchical parameters, not constants. The AF447 lesson [U for detail] is that over-confident detection probabilities in earlier searches made those areas look wrongly "cleared."
- **Economic and reporting truncation.** Small deposits and showings are under-reported. Model a reporting probability that depends on size and era.
- **Ecological templates** for the "shared bias field across species" are Fithian et al. (2015) [V] and Chakraborty et al. (2011) [V]. In mining terms, the effort field is shared across deposit types, so it can be learned jointly from all types' discovery records.
- **The PU-learning literature's "selection bias"** (SAR labels; Bekker & Davis 2020 [V]) is the same issue in ML vocabulary. A SCAR-PU model implicitly assumes uniform detection, which is precisely wrong in exploration.

**Design implication: evidence classes for absence.** FrankenMining must distinguish:
- **Not looked.** No effort recorded, so no update.
- **Looked, method insensitive.** Effort recorded, but q ≈ 0 at that depth or for that style, so a negligible update.
- **Looked, method sensitive, nothing found.** Strong negative update, scaled by q.
- **Looked, found sub-economic mineralization.** A positive update for the mineral system with a size-mark constraint.

These are four different claims with four provenance trails. They must never collapse into one "0" in a training raster.

### 1.8 Hierarchical Bayesian and GP models; transfer across terranes

**Hierarchical structure (proposed):**

> log λₖ(s) = αₖ,τ(s) + βₖ,τ(s)ᵀ x(s) + wₖ(s)

- τ(s) is the terrane or tract containing *s*. x(s) are mineral-system proxies. wₖ is a Matérn/SPDE Gaussian random field.
- Terrane-level coefficients are drawn from deposit-type hyperpriors, θₖ,τ ~ N(θₖ, Σₖ). Optionally a **similarity kernel** over terrane descriptors (tectonic setting, age, crustal thickness, metamorphic grade, erosion level) correlates θ across terranes, i.e. a GP over "terrane space."
- An unexplored terrane gets a posterior for θₖ,τ equal to its prior predictive from the hyperprior and similar terranes. Its **uncertainty widens automatically with dissimilarity** from the explored set. This is principled knowledge transfer: borrowing strength where analogues are close, and honest ignorance where they are not.
- Marks (grade–tonnage) are pooled hierarchically the same way. This directly addresses the Ellefsen (2026) pooled-vs-tract discrepancy [V]: the model learns how much tracts deviate from the global type.

**Evidence that transfer is partial:**
- Alfonso et al. (2024) [V]: "significantly different results between models trained on data from North or South America suggest the existence of extensive variety among porphyry copper provinces."
- Bourdeau, Parsa, Lawley et al. (2026), *NRR* [V]: national Canadian models fine-tuned for Nunavut. They explicitly test and support the hypothesis that "spatial transferability of trained ML models is incomplete," and frame national frameworks as "foundational" for fine-tuning.
- Lauzon & Gloaguen (2024), *Ore Geol. Rev.* 105918 [V]: transfer from five separated mineral belts plus geologically constrained negative sampling improved prediction of known Cu deposits and "significantly reduc[ed] uncertainties" (Superior Craton, Quebec).
- Wu et al. (2021) [V-sec]: transfer learning plus siamese networks for geochemical anomalies.
- Caers (2025) [V]: "Learning in area A may not apply to area B."

**GP and geostatistical tooling:**
- Diggle & Ribeiro (2007), *Model-based Geostatistics*, Springer [V].
- Handcock & Stein (1993) Bayesian kriging [V-sub].
- Kirkwood et al. (2022) [V]: Bayesian deep learning for spatial interpolation with auxiliary information. A deep model with a learned covariance structure gives calibrated predictive distributions on UK geochemical data [U for the specific result].
- Wang & Zuo (2024), *G-Cubed*: UQ in geochemical mapping, review and recommendations [V].

**Computational notes:**
- The LGCP plus SPDE route gives sparse precision matrices. INLA-style Laplace approximations or HMC/NUTS over the latent field are feasible at regional scale (10⁵–10⁶ mesh nodes with sparse Cholesky).
- Rust has the needed sparse linear-algebra building blocks (e.g., faer). INLA-equivalent machinery would have to be built in-house.

### 1.9 Deposit-size distributions: Zipf/Pareto, endowment and maturity

- **Concentration of metal** (Singer 1995, *Econ. Geol.* 90:88–104) [V]:
  - World-class deposits (the top 10% by contained metal) hold over 86% of gold, 79% of silver, 84% of copper, 71% of zinc and 73% of lead.
  - The largest 10% of deposits hold 47–79% of metal.
  - Over 96% of each metal is in above-median-size deposits.
  - **Consequence:** the economic value of exploration is dominated by the tail. Models must get the *upper tail of the size mark* right, and the thin-tailed lognormals often fitted to grade–tonnage data can understate it.
- **Zipf's law as a maturity measure** (Guj et al. 2011) [V]: rank–size Zipf curves anchored on the rank-1 deposit estimate total "natural" endowment and therefore *exploration maturity*. For the Yilgarn, the estimated share of endowment discovered was 17% (1973), 33% (1989), 62.5% (2003) and 75% (2008).
  - Pitfalls: anchoring on the rank-1 deposit makes estimates sensitive to that deposit's resource growth (the Golden Mile went from 36 to 72 Moz). Zipf is a phenomenological regularity, not a law. Merriam, Drew & Schuenemeyer (2004), "Zipf's law: a viable geological paradigm?", *NRR* [U for DOI], is the standard critique.
- **Tectonic-diffusion endowment model** (Kesler & Wilkinson 2008, *Geology* 36:255–258) [V]: total porphyry Cu endowment in the crust is estimated from deposit numbers and ages through a time–space model of formation, exhumation, erosion and preservation. This is an independent, physically motivated prior for "how many are left at depth."
- **Recommendation.** Treat Zipf/Pareto and discovery-process models as **alternative structural hypotheses** for the size mark and the discovery sequence, compared by hindcast score (§1.6). Do not hard-code any one of them. The size-biased thinning model of §1.7 *derives* the "biggest-first" pattern, so a district's apparent Zipf slope becomes a *diagnostic* of the detection process, not an input.

### 1.10 Decision-theoretic exploration: VOI, EOI, Bayesian evidential learning, POMDPs

**Foundations:**
- Howard (1966), "Information value theory," *IEEE TSSC* 2:22–26 [V]; Howard (1968), "The foundations of decision analysis" [V-sec].
- **VOI in spatial problems:** Bhattacharjya, Eidsvik & Mukerji (2010), *Math. Geosci.* 42:141–163 [V]; Eidsvik, Mukerji & Bhattacharjya (2015), *Value of Information in the Earth Sciences*, Cambridge UP [V-sec].
- **Bayesian Evidential Learning (BEL):** Scheidt, Li & Caers (2018), *Quantifying Uncertainty in Subsurface Systems*, AGU/Wiley, doi:10.1002/9781119325888 [V].
  - Workflow: (1) prior falsification by prior-predictive checks; (2) Monte Carlo from the prior; (3) learn a direct data→prediction relation (regression or ABC) in a reduced dimension; (4) posterior prediction without full inversion.
  - AutoBEL (Yin, Strebelle & Caers 2020, *GMD* 13:651–672) [V] automates geological-uncertainty updating with borehole data.

**Efficacy of information (EOI).** Caers, Scheidt, Yin, Wang, Mukerji & House (2022), *NRR* 31:1157–1173, doi:10.1007/s11053-022-10030-1 [V metadata; definition V-sec via Caers 2025].
- EOI measures how much future data will, *on average*, reduce uncertainty on a quantity of interest. It compares the prior f(x) with the posteriors under possible data outcomes, e.g. prior entropy minus expected posterior entropy [V-sec].
- **Distinction from VOI:** EOI is decision-agnostic (uncertainty reduction), whereas VOI weights uncertainty reduction by its effect on decisions and payoffs. Kurt House (KoBold CEO) is a co-author.
- A 2026 Scheidt, Neto, Yin & Caers paper ranking drillholes by EOI on grade, tonnage or volume was seen only in a search snippet [U].

**Intelligent Prospector v1.0.** Mern & Caers (2023), *GMD* 16:289–313, doi:10.5194/gmd-16-289-2023 [V, full method read].

| Element | Detail |
|---|---|
| State | Mineralization "bump" with uncertain width, plus a GP background; ore = field above a threshold; value = ore volume |
| Belief | Hierarchical: a particle filter over the shape parameter and a conditional GP for the background |
| Solver | POMCPOW with progressive widening and a modified UCB; 10,000 trajectories per step |
| Actions | Drill a grid location, MINE, or ABANDON |
| Reward | −0.1 per hole; MINE pays volume − 150 |
| Results | Matched a 16-hole grid's accuracy in about 7–10 holes; about 95% of profitable ore mined in the single-body case |
| Limitations | About 32% of profitable multi-body cases wrongly abandoned (particle degeneracy); 2D synthetic; noiseless observations. Code is MIT-licensed. |

**Intelligent Prospector v2.0.** Mern, Corso, Burch, House & Caers (2024), arXiv:2410.10610 [V, full text read]. Affiliations: KoBold Metals, Stanford and Terra AI.
- **States:** geological state (grade, thickness, surface depth on a grid), a *hypothesis state* (latent categorical: which conceptual model, plus domain geometry), and drill-rig states.
- **Prior:** a Bayesian network f(H, D_chem, D_grab, Th, g) = P(H) f(D_chem|H) f(D_grab|H) f(Th|D_grab) f(g|D_chem), with GPs whose means are controlled by domains. The posterior over geometry is sampled by elliptical slice sampling, implemented in Turing.jl.
- **Falsification:** a **null hypothesis h₀** is a maximum-entropy, spatially uncorrelated mixture model matched to the histograms. "Geology is not random," so if h₀'s likelihood overtakes every human hypothesis, all of them are falsified and humans must propose new ones. "There is currently no AI that can magically invent new hypotheses." This is the most concrete published mechanism for an "unknown-unknown alarm" in exploration.
- **Solver:** offline SARSOP on a discretized belief (state ensemble; observations clustered with k-means), re-solved after each continuous belief update.
- **Results** (17 synthetic truths):
  - For the same go/no-go accuracy, the POMDP needed **fewer than half as many boreholes** as 6×6 grid drilling.
  - It falsified wrong hypotheses faster.
  - Grid drilling "is not able to detect that all hypotheses are false."
- **Field use:** the method "has aided in the characterization of an ultra high-grade deposit in Zambia in 2023" (Mingomba; FT report, Feb 2024).
- **Open issue the authors flag:** "Decision science does not tell us what reward should be included." Reward design, covering uncertainty reduction vs economics and junior-company incentives, remains subjective.

**Caers (2025) protocol** [V, full text]. A "Popper–Bayes" scientific method:
1. Domain experts generate conceptual hypotheses.
2. Modellers translate them into parameterized priors, with discrete probabilities on hypotheses and distributions on parameters.
3. Falsification tests (forward-model the data; statistical test with p-values) reject inconsistent priors.
4. Monte Carlo Bayesian inversion follows.
5. Decisions use the Bayesian UQ.

He also argues three points:
- Inversion should invert *for geology* (geologically realistic priors), not use vague smoothness regularization that "double-dips" geological knowledge.
- The barriers are organizational: disciplinary silos, the junior-company funding model, and "no comprehensive software."
- Drill for **hypotheses, then volume, then grade**.

**KoBold Metals' public description** [V, koboldmetals.com/science]:
- **Hyperpod™:** airborne sensing.
- **TerraShed®:** a data platform that digitizes legacy reports, maps and notes into one cross-language searchable store.
- **Machine Prospector®:** predictive models of grade, thickness, lithology, conductivity and susceptibility, each "with a quantified uncertainty" and updated from field observations.
- **KoBoMine™:** mine design over many geological realizations, simulating and optimizing "thousands of scenarios" under uncertain costs and ground conditions.
- Mingomba (Zambia) is presented as a tier-one Cu discovery developed with ZCCM-IH. The page discloses no resource figures or methods. The peer-reviewed methodological trail is the Caers-group papers above.

**Software for sequential decisions:**
- POMDP algorithms: POMCP, POMCPOW (Sunberg & Kochenderfer 2018) and SARSOP (Kurniawati et al. 2008) [V-sec via Mern papers].
- Kochenderfer, Wheeler & Wray (2022), *Algorithms for Decision Making*, MIT Press [V-sec].
- Reinforcement-learning dispatch and planning benchmarks exist (§5), but exploration-specific RL results remain synthetic.

### 1.11 Pitfalls catalogue (prospectivity and undiscovered resources)

| # | Pitfall | Consequence | Mitigation in FrankenMining |
|---|---|---|---|
| P1 | Treating unlabeled cells as negatives | Prospectivity collapses onto explored areas | Point-process likelihood with explicit thinning (§1.7); PU only with SAR/SNAR detection modelling |
| P2 | Random cross-validation | Over-optimistic skill, wrong hotspots | Blocked, LOCO and hindcast CV mandatory; report all three |
| P3 | ROC-AUC as headline metric | Masks poor precision at about 1% prevalence | PR-AUC, capture curves, expected-count calibration |
| P4 | Conditional-independence violation (WofE, naive Bayes) | Over-confident posteriors | Joint models; CI tests (Agterberg & Cheng 2002); shrinkage |
| P5 | Deterministic interpolation of covariates | Variance loss and spurious correlation (Caers 2025) | Use covariate *realizations*; propagate interpolation uncertainty |
| P6 | Support mismatch (core vs geophysics) | Inflated correlation; wrong scale of prediction | Explicit support metadata; change-of-support operators (§4) |
| P7 | Naive pooling across terranes | Factor ~0.04–5.5× errors (Ellefsen 2026) | Hierarchical partial pooling; terrane-similarity kernel; AOA flags |
| P8 | Workflow-choice sensitivity | Arbitrary maps (Zhang et al. 2024) | Workflow ensembles; report between-workflow variance |
| P9 | Size-model tail underestimation | Undervalues exploration; wrong option values | Heavy-tail alternatives (Pareto/Zipf vs lognormal) compared by hindcast |
| P10 | Expert counts never scored | Unknown calibration of expert inputs | Store every elicitation; score against later discoveries; performance-weighted pooling (Cooke [U]; IDEA protocol, Hemming et al. 2018 [V]) |
| P11 | Single conceptual model | Exploration drilled on a falsified premise | Multiple hypotheses plus a null hypothesis (Mern et al. 2024) |
| P12 | Resource growth of known deposits ignored | Biased grade–tonnage and Zipf anchors | Version deposit inventories through time; model "growth" as part of the size mark |

### 1.12 Implementation requirements (topic 1)

**Data structures:**
- `DepositRecord`: identity under a *versioned aggregation rule*, type and classification confidence, location geometry, discovery date and method, resource and production history as a *time series* of reported estimates with code and category, and provenance.
- `EffortRecord`: method, geometry (line, polygon, point), date, specification (line spacing, depth of investigation, detection limits), source document, plus a *coverage-confidence* flag for incomplete archives.
- `CovariateField`: values or realizations, support (cell size and averaging kernel), uncertainty model, lineage.
- `Hypothesis`: a conceptual-model descriptor plus a parameterized prior plus a status (`active`, `falsified(evidence, test, date)`, `superseded`).
- `IntensityPosterior`: a mesh-based latent field (SPDE nodes) with posterior samples or Laplace moments; an integrator over arbitrary polygons and mark thresholds.
- `ElicitationRecord`: expert, date, question, quantiles, rationale, and later score.

**Algorithms:**
- Sparse GMRF/SPDE construction; sparse Cholesky; Laplace/INLA-style or HMC inference.
- Thinned-PPP likelihood with mark integration, via quadrature over the mesh.
- Negative-binomial/MapMark-compatible tract roll-up as a degenerate mode.
- Monte Carlo endowment simulation.
- EOI/VOI estimators by nested Monte Carlo (with variance-reduction warnings).
- A POMDP planner interface: particle belief, POMCPOW-class online solver, and an optional point-based offline solver.
- Hypothesis-falsification monitor (null-hypothesis likelihood race; prior-predictive p-values).

**Validation tests:**
1. **Synthetic world.** Known λ, π and marks.
   - Coverage of 50/80/95% credible intervals for undiscovered counts and metal within ±2–3% of nominal over at least 200 replicates.
   - Simulation-based calibration rank histograms uniform.
2. **MapMark4 parity.** With a degenerate configuration (no spatial field, tract-level Poisson–Gamma, the same grade–tonnage pdfs), the endowment distribution matches MapMark4 outputs on its bundled datasets within Monte Carlo error.
3. **Search-theory unit test.** A single-cell thinning update reproduces p′ = p(1 − d)/(1 − pd). Many-cell renormalization conserves total prior mass.
4. **Metamorphic tests:**
   - Doubling the effort in a region with no discoveries must not increase λ there.
   - Adding a discovery must not decrease the posterior expected count within its neighbourhood.
   - Coordinate rotation or translation leaves results invariant.
   - Refining the mesh or grid leaves polygon-integrated counts invariant within tolerance (change-of-support consistency).
5. **Hindcast benchmark.** At least one public district hindcast, e.g. reproducing the Guj et al. 2011 Yilgarn audit design, using public deposit and effort data where licensable.

---

## 2. Geophysical, geochemical and remote-sensing data; geological modelling

### 2.1 Geophysical methods: what each constrains (orientation)

The methods, their physical properties and their typical Au/Cu/Ag/Sn targets are standard background [U, textbook-level].

| Method | Property | Typical targets / use | Main ambiguity |
|---|---|---|---|
| Magnetics (airborne/ground) | Magnetic susceptibility, remanence | Intrusions, magnetite-destructive alteration (porphyry phyllic zones), structures, Fe-oxide (IOCG) | Remanence; non-uniqueness in depth/extent; magnetite destruction vs absence |
| Gravity (incl. airborne gradiometry) | Density | Basin architecture, intrusions, massive sulphides, lithosphere-scale structures | Depth–density trade-off |
| IP / resistivity (DC, 2D/3D) | Chargeability, resistivity | Disseminated sulphides (porphyry pyrite haloes), clay alteration | Graphite and clays also chargeable; coupling effects |
| EM (airborne TEM/FEM, ground TEM, MT) | Conductivity | Massive sulphides, conductive cover thickness, deep structure (MT) | Conductive cover masking; superparamagnetic and IP effects in TEM |
| Seismic (2D/3D reflection) | Impedance contrasts | Deep structural framework, cover thickness; increasingly hard-rock 3D | Cost; weak impedance contrasts in crystalline rock |
| Radiometrics (K, eTh, eU) | Surface (~30 cm) radioelement concentrations | K-alteration (potassic), regolith mapping, Sn granites (U/Th-rich, highly fractionated) | Surface only; regolith transport |

### 2.2 Joint, cooperative and stochastic inversion

- **Structural coupling:**
  - Haber & Oldenburg (1997), "Joint inversion: a structural approach," *Inverse Problems* 13:63–77 [V-sec].
  - Gallardo & Meju (2003), cross-gradient joint inversion of DC resistivity and seismic, *GRL* 30(13):1658 [V]; the 2004 *JGR* extension adds Lagrange-multiplier constraints [V-sec].
  - The cross-gradient forces property gradients to be parallel without assuming a petrophysical relation. **Limitation:** it assumes co-located boundaries [V-sub].
- **Petrophysically and geologically constrained inversion:**
  - Giraud et al. (2019), *GJI*: sensitivity of constrained joint inversions to geological and petrophysical uncertainty [V].
  - Blockworlds (*GMD* 15:3641, 2022): full posterior inference over implicit geological models with kinematic elements, using gravity and magnetic forward models [V-sub].
- **Stochastic/Bayesian inversion:**
  - Trans-dimensional MCMC (Sambridge et al. 2006, *GJI* 167:528–542) treats the number of parameters as unknown [V-sub].
  - Variational alternatives such as boosting variational inference (Zhao & Curtis 2024, *JGR*) [V].
  - Hessian-informed MCMC for 3D gravity geometry, from the Caers group [U for exact citation].
  - Caers (2025) shows multiple stochastic TEM inversions that fit the data equally well as the deterministic one but imply very different intrusion geometries [V].
- **Open-source frameworks:** SimPEG (Cockett et al. 2015, *Comput. Geosci.*) [V-sub] is a modular deterministic/gradient inversion framework, with v0.26.0 released Oct 2026 [V-sub]. It provides no posterior sampling out of the box.
- **Pitfall.** Deterministic inversion followed by a single interpretation is the dominant industry workflow, and Caers argues it is the main cause of false-positive drilling [V]. Regularization (smoothness, small model) is *not* a geological prior. It imposes a solution style, and interpreters then re-impose geology, giving circular reasoning.

### 2.3 Remote sensing (multispectral and hyperspectral)

All [V-sub] unless noted:
- **ASTER:** SWIR bands 4–9 failed in April 2008. SWIR alteration mapping (clays, micas, carbonates) must use **pre-April-2008** archives. VNIR and TIR still work.
- **WorldView-3:** 8 SWIR bands at high spatial resolution. Kruse & Perry (2013) simulated mineral mapping at Cuprite, and on-orbit validation followed in 2015.
- **PRISMA (ASI):** launched March 2019; operational; second generation targeted for 2029.
- **EnMAP (DLR):** launched 1 April 2022; 420–2450 nm, 30 m GSD; operational into year 5 with more than 5,000 users. Critical raw materials are an explicit application.
- **EMIT (NASA, ISS):** 380–2500 nm at 60 m. **L2B mineral products ship per-pixel uncertainty**, which suits uncertainty propagation. Extended mission through at least 2026.
- **Design implication.** Remote-sensing products must carry sensor, scene date, atmospheric-correction version and per-pixel uncertainty where provided. Band failures (ASTER SWIR) make "absence of alteration signal" an *evidence-class* question (sensor insensitive vs alteration absent), consistent with §1.7.

### 2.4 Geochemistry and compositional data analysis

- **Theory:**
  - Aitchison (1986), *The Statistical Analysis of Compositional Data* [V-sub].
  - Isometric log-ratio: Egozcue et al. (2003), *Math. Geol.* 35:279–300 [V-sub].
  - Pawlowsky-Glahn, Egozcue & Tolosana-Delgado (2015), *Modeling and Analysis of Compositional Data*, Wiley [V-sub].
  - For geochemistry: Grunsky's GSC regional-survey work, and the GeoCoDA workflow (Grunsky, Greenacre & Kjarsgaard 2024, *Appl. Comput. Geosci.* 22) [V-sub].
- **Facts:**
  - The clr transform keeps D parts but has a singular covariance.
  - The ilr transform gives D−1 orthonormal coordinates.
  - Aitchison distance equals Euclidean distance in clr space [V-sub].
  - Raw concentrations are closed data: correlations between raw percentages are spurious. MapMark4 itself treats grades compositionally [V].
- **Practical issues** [U]:
  - Censored values below detection limit need multiplicative or model-based replacement, never zero.
  - Mixed analytical packages and digestions (aqua regia vs four-acid vs fusion) are not interchangeable and must be harmonized or treated as different measurements.
  - Sample media (stream sediment, soil horizon, till, lag) have different dispersion physics.
  - Anomaly thresholds are deposit-context dependent.
- **Uncertainty in geochemical mapping:** Wang & Zuo (2024), *G-Cubed* review [V]; Liu et al. (2018), geostatistical simulation plus singularity analysis [V-sec].

### 2.5 Implicit and probabilistic 3D geological modelling

- **RBF implicit modelling:** Cowan et al. (2003), "Practical implicit geological modelling," 5th Int. Mining Geology Conf., AusIMM pp. 89–99 [V-sub]. The author list differs between sources, so check before citing. This is the foundation of Leapfrog-style volume-function modelling. Hardy (1971) multiquadrics is the mathematical ancestor [V].
- **Potential-field co-kriging implicit models:** GemPy 1.0 (de la Varga, Schaaf & Wellmann 2019, *GMD* 12:1–32) [V-sub].
  - Originally differentiable through Theano, enabling Bayesian inference.
  - GemPy v3 uses calendar versioning (2026.0.3 stable, June 2026), PyTorch support and the EUPL-1.2 licence [V-sub].
- **LoopStructural** (Grose et al. 2021, *GMD* 14:3915) [V-sub]: time-aware modelling with multiple interpolators. Faults and folds are modelled in reverse chronological order. v1.10.0 released Oct 2026.
- **Uncertainty:**
  - Wellmann & Regenauer-Lieb (2012), information entropy over model ensembles, *Tectonophysics* [V-sub].
  - de la Varga & Wellmann (2016), "Structural geologic modeling as an inference problem: a Bayesian perspective," *Interpretation* 4:SM1–SM16, doi:10.1190/INT-2015-0188.1 [V].
  - Wellmann & Caumon (2018), *Adv. Geophys.* 59:1–121, a comprehensive review [V].
  - Graph neural networks for structural modelling: Hillier et al. (2021) [V].
- **Conceptual uncertainty:** Bond et al. (2007) [V]. Only 21% of 412 geoscientists identified the correct tectonic setting of a synthetic seismic section. Model-structure uncertainty dominates parameter uncertainty in interpretation.
- **Implementation requirements:**
  - Implicit fields as **parameterized functions** (RBF or co-kriging weights plus drift) with uncertain interface/orientation data and uncertain topology (fault–horizon relations, chronology).
  - Sampling over geometry with MCMC/ABC (Schaaf et al. 2021, ABC in GemPy [V-sub]).
  - Ensembles exported as categorical voxel realizations with entropy maps.
  - **Tests:**
    - Exact interpolation of hard contacts.
    - Orientation honouring within tolerance.
    - Rotation/translation invariance.
    - Reproduction of GemPy/LoopStructural reference models (parity).
    - Topological validity: no self-intersecting surfaces; consistent stratigraphic order.

---

## 3. Drilling, sampling, assaying and QA/QC

### 3.1 Drilling and logging

- **RC vs diamond core** [U]:
  - RC is cheaper and faster and yields chips. It risks down-hole contamination and smearing, especially below the water table or in wet ground, and sample loss in voids.
  - Diamond core preserves structure, lithology and geotechnical data and enables orientation measurements, but is costly.
  - RC is standard for grade-control and infill. Diamond is needed for geology, geotechnics and metallurgy.
  - *Data-model implication:* the drilling method is a covariate of measurement error and bias, and wet RC intervals deserve their own flag.
- **Hyperspectral core scanning:**
  - CSIRO HyLogging licensed to Corescan (2017). HyLogger-4 (Corescan/Epiroc) was installed at GSWA in 2025 (VNIR–SWIR–MWIR–LWIR, 25 µm/pixel imagery) [V-sub].
  - The Australian NVCL holds more than 1.6 Mm of scanned core [V-sub].
- **pXRF and ML core-photo logging:**
  - SRK notes that ML photo-logging models need retraining when geology or image quality changes [V-sub]. No peer-reviewed 2023–2026 review was found [V-sub].
  - Automated lithology extraction from core photos dates back to Thomas et al. (2011), *First Break* [V].
- **Design implication.** Logged attributes from humans, scanners and ML are three evidence classes with different error models. ML-derived logs must carry model version and confidence, and must be re-derivable.

### 3.2 Theory of Sampling (Pierre Gy) and fundamental sampling error

- **Gy's FSE formula** [V-sub, from Minnitt & Assibey-Bonsu 2010, *JSAIMM* 110]:

  > σ²_FSE = c·f·g·l·d³ · (1/M_S − 1/M_L)

  - c: mineralogical factor; f: shape factor; g: granulometric factor; l: liberation factor; d: top size (about the 95% passing size); M_S, M_L: sample and lot masses.
- **François-Bongarçon calibration:** l = (d_ℓ/d)^b with 0 ≤ b ≤ 3, so σ² ∝ d^α with α = 3 − b.
  - **b ≈ 1.5 in most gold ores.** Pitard cites b ≈ 0.5 for a single constituent.
  - K and α are calibrated by duplicate-series analysis or a heterogeneity test [V-sub].
  - "Why the overall exponent in Gy's formula almost never gets close to 2.5" (Sampling 2010, AusIMM) [V-sub].
- **Sampling nomogram:** log σ²_FSE vs log mass across each crushing and splitting stage. The safety line (e.g., a relative standard deviation target) defines allowable mass reductions [U for conventional targets, e.g. ≤10% relative standard deviation (RSD) for Au].
- **Other TOS errors** [U]: grouping and segregation, delimitation, extraction, preparation and analytical errors. The increment delimitation and extraction errors are *biases* that statistics cannot fix after the fact.
- **Standard:** DS 3077 "Representative sampling – Horizontal standard." The 2013 edition was **superseded by DS 3077:2024** (13 Oct 2024, 94 pp.) [V-sub].
- **Coarse gold:**
  - Dominy et al. (2019), *Minerals* 9:238 [V-sub], integrate TOS into underground grade control and decompose the nugget effect into a *geological* part and a *sampling-induced* part.
  - Dominy, Xie & Platten (2008): coarser gold needs larger samples [V-sub].
- **Implementation requirements:**
  - Each assay links to a **sampling-protocol object** (stages, masses, top sizes, splitters) from which an *a priori* FSE variance is computed.
  - That variance becomes part of the measurement-error model used by geostatistics, separated from the geological nugget.
  - Duplicates at each stage empirically calibrate K and α. Tests: unit tests of the formula; reproduction of published nomograms; consistency checks between predicted FSE and duplicate-pair variance.

### 3.3 Assay methods

- **Fire assay** (Pb collection, gravimetric or AAS/ICP finish) measures *total* Au. It is the global default [V-sub, ALS].
- **Screen fire assay** for coarse gold: assays the coarse fraction entirely and the fines in replicate [U for protocol details].
- **Cyanide leach / LeachWELL (bottle roll):**
  - Measures *recoverable* Au, often reconciles better with production [V-sub].
  - At Tarmoola, a 1-hour LeachWELL leach under-called fire assay by up to 18% and a 2-hour leach over-called by up to 8% [V-sub].
  - The method is a different *measurand*, not a noisier version of the same one.
- **AAS and ICP-OES/MS:** multi-element packages; digestion strength defines what fraction is measured [U].
- **Chrysos PhotonAssay** [V-sub]:
  - Physics: gamma activation of a ~500 g crushed sample in a jar using an 8.5 MeV X-ray source. Detects the ¹⁹⁷ᵐAu isomer (7.73 s half-life; 279 keV line), about 2 minutes per jar.
  - Non-destructive, with a large sample mass, which reduces FSE for nuggety gold.
  - 46 units across 34 sites at 30 June 2026. Runs in ISO/IEC 17025-accredited labs (e.g., NATA), but there is no PhotonAssay-specific ISO method standard.
  - **Design implication:** method-switching mid-project (fire assay → PhotonAssay) is a *measurement-system change* requiring paired-sample bias studies, with both values kept.

### 3.4 QA/QC

- **Programme elements** [V-sub refs; U for thresholds]:
  - Certified reference materials (CRMs), inserted blind, monitor *accuracy*.
  - Blanks monitor *contamination*.
  - Field, coarse-reject and pulp duplicates monitor *precision* at each stage.
  - Umpire-lab checks monitor inter-lab bias.
- **References:**
  - Abzalov (2008), "Quality control of assay data: a review of procedures for measuring and monitoring precision and accuracy," *Explor. Min. Geol.* 17:131–144 [V-sub].
  - Abzalov (2016), *Applied Mining Geology*, Springer [V-sub].
  - Thompson & Howarth (1976, 1978): precision as a function of concentration [V-sub].
  - Lawie & Stanley (2012): tie all relative-error measures to the coefficient of variation [V-sub].
- **Conventional control rules** [U]:
  - CRM results outside ±2 SD are a warning; outside ±3 SD, or two consecutive results beyond 2 SD on the same side, are a failure that triggers batch re-assay.
  - Blanks above about 5–10× the detection limit are a contamination failure.
  - Duplicate precision is assessed by HARD/HRD or CV, with typical targets: pulp duplicates within ±10% for 90% of pairs; coarse within ±20%; field within ±30%.
  - These are industry conventions, not standards. FrankenMining should make them **configurable policies with provenance**, not constants.
- **Implementation requirements:**
  - **QA/QC as a statistical process-control service.** Westgard-style rules over CRM time series by lab, batch and method. Every assay carries a `qaqc_state` (`passed`, `failed`, `pending`, `overridden(by, reason)`).
  - **Batch lineage.** The ability to propagate a CRM failure to all affected samples and downstream estimates, giving "invalidation cascades."
  - **Precision models.** Thompson–Howarth style precision–concentration fits feed measurement-error variance into estimation.
  - **Tests:** synthetic batches with injected bias, drift and contamination must trigger the right rules (detection power and false-alarm rate measured).

### 3.5 Compositing, nugget effect, capping/top-cutting

- **Compositing** [U]:
  - Composite to equal length within geological domains to regularize support. Handle residual short intervals by distribution or exclusion with documented rules.
  - Never composite across domain boundaries.
  - Length-weight, plus density-weight where density varies strongly (e.g., massive sulphide).
- **Nugget effect:**
  - Part geological (micro-scale heterogeneity) and part sampling/analytical (Dominy et al. 2019 [V-sub]).
  - Only the sampling part can be reduced by protocol, which is why the separation matters.
- **Capping/top-cutting:**
  - Methods include probability-plot breaks, percentile caps, Parrish's decile analysis (1997), metal-at-risk (Parker) and cutting-curve analysis [U for bibliographic details].
  - Alternatives to capping: the indicator/truncation decomposition with cokriging ("Capping and Kriging Grades with Long-Tailed Distributions") [V-sub]; Rivoirard et al.'s top-cut model [U].
  - **Pitfall:** capping is an *estimator bias choice* made to reduce variance. It must be recorded as an explicit modelling decision with sensitivity results (metal removed, by domain), never as data cleaning.

---

## 4. Geostatistics, resource estimation and reporting

### 4.1 Estimation and simulation methods

| Method | Key refs | Role | Known limitations |
|---|---|---|---|
| Ordinary / simple kriging | Krige (1951) [V-sub]; Matheron [U]; Journel & Huijbregts (1978) *Mining Geostatistics* [V-sec]; Chilès & Delfiner (2012) [V] | BLUE local estimate; block kriging | Smoothing; **conditional bias**. The slope of regression below 1 diagnoses it (Krige 1996; CCG note) [V-sub]. Kriged grade–tonnage curves misstate tonnage above cut-off. |
| Indicator kriging (IK, MIK) | Journel (1983) [U] | Non-parametric local distributions | Order-relation violations; many variograms; change of support not handled by IK alone |
| Change of support: discrete Gaussian model (DGM) | Matheron (1976) [U]; Rivoirard (1994) [U]; consistency conditions (Emery 2007) [U] | Block (SMU) distributions from point data | Permanence-of-distribution assumptions; information effect handled separately |
| Uniform conditioning (UC) and localized UC (LUC) | Rivoirard (1994) [U]; Abzalov (2006), *Math. Geol.* 38:393–411 [V-sub] | SMU-scale recoverable resources in panels; LUC localizes them | **Assibey-Bonsu & Muller (2014):** LUC checked against an 8×5 m grade-control grid gave a **slope of regression ≈ 0.1**, i.e. severe conditional bias for short- and medium-term planning [V-sub] |
| Sequential Gaussian simulation (SGS) / sequential indicator simulation (SIS) | Deutsch & Journel (1998), *GSLIB* [U]; Goovaerts (1997) [U] | Realizations reproducing histogram and variogram, used for uncertainty and transfer functions | Ergodic fluctuations; normal-score back-transform; trend handling; multi-Gaussian assumptions (no connectivity of extremes) |
| Turning bands | Matheron (1973) [U]; Emery & Lantuéjoul (2006) TBSIM, *Comput. Geosci.* 32:1615–1628 [V-sub] | Fast unconditional simulation plus kriging conditioning | Banding artefacts with few lines |
| Multiple-point statistics (MPS) | Strebelle (2002), *Math. Geol.* 34:1–21 [V-sub]; Mariethoz & Caers (2014/15) [V-sub] | Curvilinear and categorical geology from training images | Training-image choice is itself a hypothesis (Popper–Bayes again) |
| SPDE / GMRF and non-stationary GRFs | Lindgren, Rue & Lindström (2011) [V] | Sparse-precision Matérn fields; non-stationarity via spatially varying SPDE coefficients | Mesh design; boundary effects |
| Bayesian kriging / GP with hyperparameter uncertainty | Handcock & Stein (1993) [V-sub]; Diggle & Ribeiro (2007) [V] | Propagates variogram-parameter uncertainty | Rarely used in industry; computational cost (mitigated by SPDE) |
| ML and ensembles | Mery & Marcotte (2021), *Math. Geosci.* [V]; Lindi et al. (2024) UQ review, *NRR* 33:2503–2526 [V] | Non-linear trends; auxiliary data | Spatial leakage in validation; support ignorance; calibration |

**Key structural facts:**
- **Uncertainty accumulates through the estimation chain:** sampling → domaining → compositing → variography → estimation or simulation → change of support → classification. Lindi et al. (2024) review stepwise propagation [V].
- **Volume–variance relation.** Block variance is lower than point variance. The *information effect* means that future selection will rely on imperfect grade-control estimates, not true block grades [U]. Recoverable-resource methods must model both. Many reported resources are "kriged at SMU size with too little data," which is exactly what UC/LUC and simulation try to address.
- **Variogram uncertainty** is usually ignored, yet it dominates in sparse-data (Inferred) areas. Bayesian treatment, or at least ensembles over defensible variogram models, is needed.

### 4.2 Reporting codes and classification (status verified to Oct 2026)

**JORC (Australasia)** [V-sub, jorc.org]:
- **JORC 2012 remains in force.**
- Timeline:
  - Exposure draft released 1 August 2024; comments closed 31 October 2024.
  - March 2026: "provisional JORC Code 2026" being finalized for CRIRSCO, ASX, ASIC and parent-body review.
  - August 2026: webinar on the direction of changes.
  - Remaining steps: parent-body approval, then a mandatory ASX Listing Rules consultation, then a transition period. No commencement date was found.
- Draft content (2024; final wording **unverified**):
  - A new "Specialist" role.
  - Mineral Resources require reasonable prospects for economic extraction, "i.e., more likely than not." **"Eventual" is dropped.**
  - A documented Reasonable Prospects Assessment.
  - **A new mandatory Reconciliation section** (prior-estimate and production reconciliations in Table 1 and the public report).
  - **"Risk" defined as the effect of uncertainty** (Opportunities and Threats).
  - Rounding of tonnage and grade to reflect relative uncertainty.
  - The "CV of Record" renamed "Record of Relevant Experience."

**CRIRSCO International Reporting Template, June 2024 update** [V-sub]:
- "Eventual" removed; "Mineral" deleted as a defined term; "Life of Mine Plan" added.
- Competent Person: member of a Recognised Professional Organisation with enforceable discipline, plus at least 5 years' relevant experience in the style of mineralization and the activity.
- Classes:
  - Inferred: "limited geological evidence," with continuity inferred, not verified, and not convertible to Reserve.
  - Indicated: continuity demonstrated; converts to Probable.
  - Measured: continuity confirmed; converts to Proved (or Probable).
- Related: CRIRSCO "Beyond Inferred" position paper, May 2025 [V-sub].

**NI 43-101 (Canada)** [V-sub]:
- CSA proposed repeal and replacement on **12 June 2025**; comments closed 10 October 2025. **Not final** as of October 2026.
- Proposals:
  - A single term, "mineral project."
  - QP definition drops the formal-education requirement and requires 5 years' relevant experience.
  - Alignment with CIM and CRIRSCO.
  - "Relevant" replaces "material."
  - More Indigenous, community and environmental disclosure.
  - A royalty-issuer exemption.
- CIM Definition Standards 2014 remain current. In July 2025 CIM proposed new definitions for exploration target, scoping study and life-of-mine plan; adoption is unverified.

**SEC Regulation S-K subpart 1300 (US)** [V-sub]:
- Adopted 31 October 2018; mandatory for fiscal years beginning on or after 1 January 2021; Industry Guide 7 rescinded.
- QP has at least 5 years' experience and membership in a recognized organization.
- Cut-off grade and price must be disclosed.
- A technical report summary is filed on first or materially changed disclosure.
- Uses "proven/probable."

**Others:** PERC 2021 (mandatory from 1 January 2023) and SAMREC 2016 (rewrite discussed, no new edition found) [V-sub].

**Competent/Qualified Person (CP/QP).** A *named, accountable professional* signs off public estimates. The codes are principles-based (transparency, materiality, competence), and **none prescribes an estimation method or a probabilistic definition of the classes** [U for exact wording; consistent with the template content above].

**Design implications:**
- **Resource classes are qualitative labels assigned by a CP/QP.** FrankenMining should compute *quantitative* supporting metrics per block or volume and present them as *evidence for* the CP's classification, not as an automated classification. Examples:
  - kriging variance or slope of regression;
  - conditional simulation probability that the local grade lies within ±15% at 90% confidence over a quarterly or annual production volume [U, a common industry heuristic];
  - data spacing vs variogram range.
- **The classification decision is a signed, versioned human act** with rationale: the "Competent Person act" record.
- **Code-agnostic reporting.** Store estimates once with full uncertainty, and render JORC 2012, JORC 2026 (when in force), NI 43-101 (current or new), S-K 1300, PERC or SAMREC views by versioned rule sets. This matters because the codes are in flux (2024–2026).
- **The JORC draft's mandatory reconciliation and "Risk = effect of uncertainty" align directly with FrankenMining's calibration ledger** (§9.5). It is a regulatory tailwind, not merely good practice.

### 4.3 Reconciliation (F-factors and the mine call factor)

- **Common definitions** [U; the precise variants differ by company]:
  - **F1** = grade-control (short-term) model / resource (long-term) model, over the mined volume. Tonnes, grade and metal are each tracked.
  - **F2** = mill-received (by metallurgical accounting back-calculation) / grade-control model (mine-claimed).
  - **F3** = F1 × F2 = mill-received / resource model.
  - Additional factors exist for reserve vs resource, dilution and ore loss, and stockpile transfers.
- **Mine call factor (MCF)** in South African gold mining [U]: the ratio of gold accounted for at the plant (recovered plus residue) to gold "called for" by the mine's sampling-based measurement. Persistent MCF below 100% signals sampling bias, unrecorded losses or theft.
- **Philosophy:** "Beyond Reconciliation – A Proactive Approach to Using Mining Data" (AusIMM 2003) argues for continuous, forward-looking calibration rather than retrospective correction factors [V-sub, abstract].
- **Regulation:** the JORC 2024 draft would make reconciliation disclosure mandatory [V-sub].
- **Pitfalls:**
  - **Mutual compensation.** F1 and F2 errors can cancel, hiding problems.
  - **Error in the reference.** Mill head grade is itself estimated (a back-calculation through the metal-accounting system), not "truth." AMIRA P754 is relevant here (§7).
  - **Temporal misalignment.** Stockpiles and blending decouple mining from milling in time.
  - **Factor creep.** Factors get applied as fudge multipliers to future estimates without understanding.
- **Implementation:**
  - Reconciliation is **joint state estimation over the material-flow graph** (mine → stockpiles → plant), with each node's measurements carrying error models.
  - F-factors become *posterior estimates of systematic bias* by domain and period, which feed back as **hierarchical bias priors** into the next estimates. This closes the loop in §9.5.
  - **Tests:**
    - Synthetic mines with injected biases at known nodes must have those biases recovered.
    - Mass and metal balance must close within stated measurement uncertainty.

---

## 5. Mine planning and optimization

### 5.1 Ultimate pit limits: Lerchs–Grossmann, pseudoflow, nested shells

**Problem.** Maximum-weight closure on a precedence digraph. Each block has a value, and arcs encode slope precedence.

**Lerchs & Grossmann (1965)**, "Optimum design of open-pit mines," *CIM Bulletin / Trans. CIM* 58 [V-sub].
- Graph algorithm plus a 2D dynamic-programming method.

**Equivalence to min-cut / max-flow and pseudoflow:**
- Hochbaum & Chen (2000), *Oper. Res.* 48:894–914 [V-sub].
- Hochbaum (2001), *Networks* 37:171–193 [V-sub].
- Hochbaum (2008), "The pseudoflow algorithm," *Oper. Res.* 56:992–1009, O(mn log n) [V-sub].

**MineFlow** (Deutsch, Dağdelen & Johnson 2022, *NRR* 31:1175–1187) [V-sub]:
- Open-source C++ (MIT licence), modified pseudoflow.
- Location- and direction-varying slope precedence, sub-blocks, and minimum search patterns after Caccetta & Giannini (1988).
- Author benchmark: 16 M blocks in 9 s, versus more than 3 min for a commercial tool.
- It is the obvious **parity oracle** for a Rust implementation.

**Nested pit shells.**
- Parametric analysis: re-solve at revenue factors λ·price (Whittle-style). Shell sequences can jump in size (the "gap" problem) [V-sub].

**Pitfalls** [V-sub]:
- LG and pseudoflow are *undiscounted and single-price*.
- Block-precedence approximations of slope angles introduce error. Beretta & Marinho (APCOM 2015) report up to 10.4% variation in reserves and up to 7.8° slope error [V-sec].
- Under grade uncertainty, a pit optimized on a smoothed kriged model is generally not optimal in expectation. Pit value is a convex function of block values, so Jensen's inequality applies [U, standard argument]. See also Dimitrakopoulos et al. 2002 (below).

### 5.2 Cut-off grade theory

**Lane (1964)**, "Choosing the optimum cut-off grade," *Colorado School of Mines Quarterly* 59(4) [V-sub; page range disputed].
**Lane (1988)**, *The Economic Definition of Ore*, Mining Journal Books [V-sub].
- Cut-off that maximizes NPV subject to mine, mill and market capacities, including the **opportunity cost of time**.
- Leads to declining cut-off policies.
- Balancing cut-offs between pairs of constraints [U for detailed mechanics].

**Limitation** [U]:
- Deterministic grade–tonnage curves.
- Stockpile options and uncertainty are handled poorly. Stochastic and real-options extensions exist.

### 5.3 Production scheduling at scale

**Direct block scheduling.** The precedence-constrained production scheduling problem (PCPSP) is a MILP.

| Reference | Contribution |
|---|---|
| Bienstock & Zuckerberg (2010), IPCO, LNCS 6080:1–14 [V-sub] | Solves LP relaxations with millions of variables; beats Dantzig–Wolfe |
| Muñoz et al. (2018), *Comput. Optim. Appl.* [V-sub] | Analysis of the BZ algorithm |
| Chicoisne et al. (2012), *Oper. Res.* 60:517–528 [V-sub] | LP relaxation for up to 5 M blocks × 20 periods in minutes, then rounding and local search. Limitation: one capacity constraint per period |
| MineLib (Espinoza, Goycoolea, Moreno & Newman 2013), *Ann. OR* 206:93–114 [V-sub] | Benchmark instances for UPIT, CPIT and PCPSP; the **required regression suite** |

**Stockpiles and blending:**
- Exact mixing models are non-convex (Bley, Boland, Froyland & Zuckerberg) [V-sub].
- Linear approximations: Moreno, Rezakhah, Newman & Ferreira (2017), *EJOR* 260:212–221 [V-sub]; Rezakhah et al. (2020) [V-sub].

### 5.4 Stochastic and integrated mining-complex optimization (McGill COSMO)

**Key papers:**
- **Dimitrakopoulos, Farrelly & Godoy (2002)**, *Trans. IMM A* 111:A82–A88 [V-sub]:
  - A conventional design tested against equally probable simulations **does not achieve its forecast NPV, ore tonnage or grade**.
  - This is the foundational demonstration that deterministic planning is biased.
- Dimitrakopoulos (1998), conditional simulation for orebody uncertainty in pit optimization, *Int. J. Surf. Min.* 12:173–179 [V].
- Ramazan & Dimitrakopoulos (2013), *Optim. Eng.* 14:361–380 [V-sub]: stochastic integer programming with a geological-risk discounting term.
- Goodfellow & Dimitrakopoulos (2016), *Appl. Soft Comput.* 40:292–304 [V-sub].
- Goodfellow & Dimitrakopoulos (2017), "Simultaneous stochastic optimization of mining complexes and mineral value chains," *Math. Geosci.* 49:341–360 [V-sub].
- Montiel & Dimitrakopoulos (2015), *EJOR* 247:166–178 [V-sub].
- Dimitrakopoulos (2011) decade review [V-sub].
- Kumar & Dimitrakopoulos (2021, 2022): updating simulations and schedules with new information [V-sub].

**Concepts:**
- Optimize directly over a set of geological (and price) realizations.
- Penalize deviations from targets.
- Model the whole complex jointly: mines → stockpiles → processing streams → products.

**Pitfalls** [V-sub]:
- Results are only as good as the simulations, and simulations are rarely validated for calibration.
- Large instances are solved by metaheuristics, with **no optimality certificate**.

**Design implication.** FrankenMining's planning layer should be *realization-native*:
- Accept an ensemble.
- Report the distribution of NPV, metal, and attainment of targets.
- Compute *value of the stochastic solution* versus the deterministic plan evaluated on the same ensemble.

### 5.5 Underground: stope optimization and caving

**Mineable Shape Optimiser (MSO)** [V-sub]:
- Origin: Alford Mining Systems; Datamine; commercialised 2011.
- Heuristic: seeding slices, then annealing shapes, given cut-off value (NSR), geometry and dilution.
- Alford, Brazil & Lee, "Optimisation in underground mining," in *Handbook of OR in Natural Resources* (2007) [V-sub].

**Block caving:**
- PCBC (Diering 2000, MassMin) models draw-column depletion and material mixing [V-sub].
- Cave planning hinges on draw strategy, dilution entry, and fragmentation (§6).

**Limitation:**
- Stope optimizers are deterministic on a single block model.
- Stochastic stope design (evaluating shapes over realizations) is an open opportunity [U].

### 5.6 Valuation under uncertainty

**Real options:**
- Brennan & Schwartz (1985), "Evaluating natural resource investments," *J. Business* 58:135–157 [V-sub]. Critiqued as too simple for mining realities (Maier 2021) [V-sub].
- Trigeorgis (1996) [U].
- Dimitrakopoulos & Abdel Sabour (2007), *Resources Policy* [U].

**Robust optimization:**
- Kumral (2010), "Robust stochastic mine production scheduling," *Eng. Optim.* 42:567–579 [V-sub].

**Design implication:**
- Separate **price/market uncertainty** (exogenous stochastic processes) from **geological and technical uncertainty** (endogenous, reducible by information).
- Options exist on both, e.g. delay, expand, switch cut-off, and drill more before committing.

### 5.7 Geotechnical constraints, haulage, fleet and dispatch

**Slopes:**
- Slope angles are uncertain inputs. They come from rock-mass characterization, structural domains and stability analyses (limit equilibrium, FoS / probability of failure) [U for standard practice].
- Precedence-based implementations approximate them [V-sub, Beretta & Marinho].

**Dispatch:**
- Alarie & Gamache (2002) review dispatch strategies [V-sub].
- White & Olson (1986/87), the DISPATCH lineage [V-sub].
- Hazrathosseini & Moradi Afrapoli (2024): RL-based fleet management review; 60% of 29 dispatching features neglected in prior work [V-sub].
- OpenMines (2024) and Mining-Gym (2025) RL benchmark environments [V-sub]. Results are mostly simulation-only.

**Fleet sizing:**
- Queueing and discrete-event simulation (match factor, cycle-time distributions) [U].

---

## 6. Operations

### 6.1 Drill-and-blast and fragmentation

**Kuz-Ram:**
- Cunningham (1983, 1987, 2005: "The Kuz-Ram fragmentation model – 20 years on") [V-sub].
- Kuznetsov mean-size equation plus a Rosin–Rammler distribution. The uniformity index is derived from blast design (burden, spacing, accuracy, charge length).
- The 2005 version adds electronic-delay timing.
- Ouchterlony (2016) argues all versions effectively predict the median x50 [V-sub].
- Commonly cited form [U]: x50 = A·K^−0.8·Q^(1/6)·(115/RWS)^(19/20), with rock factor A, powder factor K and charge mass Q.

**Swebrec:**
- Ouchterlony (2005), *Mining Technology* 114:29–44 [V-sub].
- Fixes Rosin–Rammler's poor fines tail.
- Form [U]: P(x) = 1 / {1 + [ln(x_max/x) / ln(x_max/x50)]^b}.

**Mine-to-mill:**
- Fragmentation affects digging rates, crusher throughput and SAG throughput (§7).
- Parameters must be *learned per domain* from image-based fragmentation measurement [U].

**Blast movement and grade control:**
- Blast Movement Technologies (Thornton, founded 2005) monitors 3D displacement with buried transmitters [V-sub].
- Blasting both displaces and *mixes* material (AusIMM 2014) [V-sub].
- If ore/waste polygons are not translated for movement, dilution and ore loss follow directly.

### 6.2 Grade control and dig-line optimization

**Methods:**
- **Verly (2005)**, *Math. Geol.* 37:451–475 [V-sub]: a critical review of estimation- vs simulation-based ore/waste classification. Simulation plus expected-loss (profit) classification outperforms kriging plus a cut-off when loss functions are asymmetric.
- **Isaaks, Treloar & Elenbaas (2014)** [V-sub]: optimum dig lines that optimize a loss function under minimum mining-width constraints.
- **Norrena & Deutsch** [V-sub]: dig-limit optimization (equipment cannot select single blocks).

**Dilution and ore loss:**
- *Planned* dilution comes from design.
- *Unplanned* dilution comes from blast movement, edge effects, overbreak (underground) and misclassification.
- Measure via reconciliation (§4.3) and model as part of the material-flow posterior.

### 6.3 Fleet, autonomy, OEE, reliability

**Autonomous haulage** [V-sub]:
- Komatsu commissioned its **1,000th FrontRunner** autonomous ultra-class truck on 21 April 2026 (Nevada Gold Mines). More than 11.5 Bt moved since 2008.
- Caterpillar MineStar Command: more than 8.6 Bt cited at MINExpo 2024 (snippet only); Fortescue renewal, March 2026.

**OEE (overall equipment effectiveness)** = availability × performance × quality [U, Nakajima].
- Elevli & Elevli (2010): loading-time-based OEE overstates performance; calendar-time basis preferred [V-sub].
- Time-usage models (GMG categories) standardize time accounting.
- Fleets can show 95.7% availability with 28.1% utilization [V-sub]. A single headline metric hides the constraint.

**Reliability:**
- Weibull life models; NHPP/power-law models for repairable systems that are *not* renewal processes [V-sub, Kumar & Klefsjö 1992].
- RCM: Nowlan & Heap (1978) [V-sub]; SAE JA1011 (1999; revised 2009, 2024) [V-sub].
- **Tyres:** TKPH limits; one Ghana study found 93.6% of 455 tyres scrapped before wear-out [V-sub].

**Short-interval control (SIC):**
- Practitioner literature only (Deswik white paper; ABM 2019) [V-sub].
- No peer-reviewed review found.

**Design implication.** Operations data are high-frequency, noisy and *the main source of reconciliation evidence*.
- Event-sourced telemetry with a strict time model (sensor time vs ingest time) is foundational.
- Reliability models must be hierarchical over components, fleets and sites to borrow strength across sparse failure records.

---

## 7. Processing

### 7.1 Comminution and ore hardness

**Bond:**
- Bond (1952), "The third theory of comminution," *Trans. AIME* 193:484–494 [V-sub].
- Bond's law [U, standard]: W = 10·Wi·(1/√P80 − 1/√F80) kWh/t.
- Wi depends on the closing screen size and feed preparation; correction methods exist [V-sub].

**SAG and SMC:**
- Morrell (2004), *Minerals Eng.* 17:447–451: specific energy from small drill-core samples (SMC test, DWi) [V-sub].
- JK drop-weight and JKSimMet: Napier-Munn, Morrell, Morrison & Kojovic (1996), *Mineral Comminution Circuits*, JKMRC [V-sub].

**Hardness variability:**
- Throughput is often hardness-limited.
- Hardness indices are *non-additive*: blends do not average linearly in energy terms [U, a well-known geomet caveat].
- Hardness must therefore be modelled as a spatial variable with its own support and blending rules, not kriged and averaged naively.

### 7.2 Geometallurgy

**Reviews:**
- Dominy et al. (2018), "Geometallurgy—a route to more resilient mine operations," *Minerals* 8:560 [V-sub].
- Lishchuk, Koch, Ghorbani & Butcher (2020), *Minerals Eng.* 145:106072 [V-sub].

**Core idea:**
- Spatially model *process-relevant* properties: hardness, mineralogy, liberation, recovery, deleterious elements (e.g., As, Sb, Hg, F, carbonaceous preg-robbing matter).
- Feed them into planning and plant models.

**Pitfalls** [U]:
- Response variables measured on few, large composite samples, giving support mismatch and sparse data.
- Non-additivity.
- Recovery models extrapolated beyond tested ranges.

### 7.3 Flotation (Cu, Ag-bearing sulphides)

**Kinetics:**
- First-order: R(t) = R∞(1 − e^(−kt)).
- Klimpel model: rectangular distribution of k over [0, k_max] [U form, standard].
- An IMPC 2024 study found rectangular and gamma rate-distribution models more stable than a single k, but 71% of rectangular-model R∞ estimates fell below the measured 32-minute recovery [V-sub]. Model choice affects extrapolation.

**Grade–recovery trade-off.**
- The concentrate grade vs recovery curve is the plant's operating frontier [U].

### 7.4 Gold

**Reference:**
- Marsden & House (2006), *The Chemistry of Gold Extraction*, 2nd edn, SME [V-sub].

**Circuits:**
- Gravity concentration for free coarse gold (also reduces nugget risk in leach circuits).
- Cyanidation via CIL (leach and adsorption together, the standard response to preg-robbing ores) or CIP (sequential) [V-sub].
- Heap leach for low-grade oxide ores.

**Refractory ores:**
- Gold locked in sulphides (arsenopyrite, pyrite) or preg-robbing carbon.
- Treated by pressure oxidation (POX), roasting or bio-oxidation (BIOX).
- No independent head-to-head review was found, only site comparisons [V-sub].

**Design implication:** recovery is a function of mineralogy domain × circuit × operating conditions, and needs uncertainty from testwork.

### 7.5 Copper, tin, silver

**Copper:**
- Sulphides: flotation to concentrate, then smelting (third-party, with payables and penalties).
- Oxides and secondary sulphides: heap leach, then SX-EW.
- Petersen (2016), *Hydrometallurgy* 165:206–212 [V-sub].
- SX-EW was about 18–21% of mine output in 2000–2009 [V-sub].

**Tin:**
- Cassiterite is dense and brittle: gravity concentration, with significant **losses in fines and slimes** [V-sub, Angadi et al. 2015, *Minerals Eng.* 70:178–200].
- Concentrates are smelted.
- *Design implication:* recovery is strongly size-distribution dependent, so it couples to the comminution model.

**Silver:**
- Mostly a by-product of Pb–Zn, Cu and Au mines [V-sub, USGS MCS 2026].
- Recovered via flotation concentrates or cyanidation (Merrill–Crowe or carbon) [U].
- By-product accounting and payables matter.

### 7.6 Plant digital twins, control and metal accounting

**Digital twins:**
- Qu, Kizil, Yahyaei & Knights (2023) review digital twins in minerals, *Mining Technology* 132:267–289 [V-sub].
- Vendor examples combine soft sensors with model predictive control (e.g., Metso Geminex at Ada Tepe) [V-sub, vendor].

**AMIRA P754 Code of Practice for Metal Accounting** [V-sub]:
- Origin: a 2001 workshop; releases from 2005, with Release 3 cited as February 2007.
- Gaylard, Randolph, Wortley & Ralston (2006), *JSAIMM* 106; Morrison & Gaylard (2008) [V-sub].
- Principles:
  - Accounting must be unbiased and precise.
  - Check-in/check-out measurement.
  - Stated precision for every mass, sampling and assay stream.
  - Data reconciliation.
  - Stocktakes.
  - Competent-person sign-off; independent audits.
  - Primary (whole-plant) vs secondary (circuit) accounting.

**Implementation:**
- Metal accounting = **weighted least-squares or Bayesian data reconciliation** over the plant flowsheet graph, with measurement-error covariances. Classical mass-balance reconciliation is standard [U].
- Outputs: reconciled flows with posterior uncertainties, plus gross-error detection (statistical tests on residuals).
- **Tests:**
  - Synthetic flowsheets with known true flows plus noise.
  - Reconciled estimates are unbiased, with correct covariance.
  - Gross errors are detected at the stated power.

---

## 8. Tailings and water

### 8.1 Construction types and failure evidence

**Construction types** [U, standard]:
- Upstream raises build on deposited tailings. This is the cheapest option and the most vulnerable to static liquefaction.
- Centreline is intermediate.
- Downstream is the most robust and needs the most fill.
- Filtered ("dry-stack") tailings reduce water and liquefaction risk. Operating plants run at about 2,500–30,000 t/d [V-sub].
- Dry stacks *can* fail, e.g. Turmalina, Minas Gerais, December 2024 [V-sub].

**Failure review:**
- Rico et al. (2008), *J. Haz. Mat.* 152:846–852, 147 cases [V-sub].
- Failures concentrate in dams up to 30 m high. The authors recommend avoiding upstream construction.

**Expert-panel findings** [V-sub]:

| Failure | Panel / report | Finding |
|---|---|---|
| Mount Polley (2014) | Panel report 2015 | Foundation failure in a glaciolacustrine unit; design was the dominant cause |
| Fundão/Samarco (2015) | Morgenstern et al. 2016 | Liquefaction flowslide; a design-concept change allowed saturation; embankment built over slimes |
| Brumadinho/Feijão Dam I (2019) | Robertson et al. 2019 | Static liquefaction from creep and loss of suction; **no specific external trigger needed**; upstream construction with poor drainage |
| Jagersfontein (2022) | DWS final report, released late 2025 | Toe movement visible in satellite imagery from February 2019; weak foundation on old tailings; no functioning decant; no evidence of registered-engineer supervision |

### 8.2 GISTM and governance

**Global Industry Standard on Tailings Management** (launched 5 August 2020 by ICMM, UNEP and PRI) [V-sub]:
- Structure: 6 topics, 15 principles, 77 auditable requirements.
- Roles:
  - Accountable Executive.
  - Responsible Tailings Facility Engineer.
  - **Engineer of Record (EoR).**
  - Independent Tailings Review Board (ITRB) or senior independent reviewer. An ITRB is required for Very High and Extreme consequence classes [V-sec].
- Consequence classes: Low, Significant, High, Very High, Extreme.
- ICMM commitment: Very High/Extreme facilities to conform by 5 August 2023; all others by 5 August 2025.

**Conformance status** (ICMM progress report, 5 November 2025, self-assessed) [V-sub]:
- 836 facilities: 67% fully conformant, 33% partially.
- More than 80% of Extreme/Very High facilities fully conformant.

**Global Tailings Management Institute (GTMI):**
- Launched 21 January 2025 for third-party audit and certification.
- No public audits found as of April 2026 [V-sub].

**National guidelines:**
- CDA Technical Bulletin on mining dams (2014) [V-sub].
- ANCOLD (2019) tailings guidelines [V-sub].

### 8.3 Monitoring, water and ARD

**InSAR:**
- Grebby et al. (2021), *Commun. Earth Environ.* 2:2: ISBAS analysis of Sentinel-1 data shows deformation precursors at Brumadinho [V-sub]. It was contested as possibly consolidation settlement (Petley) [V-sub].
- Rana et al. (2024), *Bull. Eng. Geol. Environ.* 83: InSAR is a **screening** tool. "Most potential failure modes may not exhibit InSAR-detectable accelerations," and line-of-sight geometry limits it [V-sub].
- Lumbroso et al. (2021): even short warnings at Brumadinho would have reduced deaths [V-sub].

**Piezometers** [U]:
- Pore-pressure monitoring is the primary liquefaction-relevant measurement.
- Vibrating-wire piezometers need maintenance and drift checks.

**Water balance:**
- Probabilistic Monte Carlo site-wide models (e.g., GoldSim) [V-sub].
- Climate non-stationarity is a key unknown [U].

**Acid rock drainage:**
- GARD Guide (INAP, 2009) [V-sub].
- MEND Prediction Manual 1.20.1 (Price 2009) [V-sub].
- Static tests (ABA, NAG) and kinetic tests (humidity cells, columns). Prediction uncertainty is large and long-tailed in time [U].

**Closure:**
- ICMM Integrated Mine Closure Good Practice Guide, updated February 2025 [V-sub].

### 8.4 Implementation implications

- **Model each tailings facility as a safety-critical asset** with:
  - a hazard and consequence classification (versioned by standard: GISTM, CDA, ANCOLD, local regulation);
  - named role holders (Accountable Executive, EoR, ITRB) with *signed* decisions;
  - trigger-action response plans (TARPs) tied to monitored variables with alert thresholds.
- **Evidence classes are critical here:**
  - InSAR "no acceleration detected" is weak evidence for many failure modes [V-sub].
  - A piezometer that is offline gives *no* evidence, not "pressure normal."
  - The Jagersfontein precursor visible in imagery [V-sub] shows that evidence existed and was not acted on. That is a workflow and accountability failure, not a sensing failure.
- **Never let the system present a TSF as "safe."** Present the current *evidence state*:
  - which failure modes are monitored, by what, with what detection capability;
  - which monitors are stale;
  - the last EoR review and ITRB opinion.

---

## 9. Uncertainty communication and epistemics in mining

### 9.1 How uncertainty is propagated today, and where it breaks

The typical industry value chain, and the point at which each stage throws uncertainty away:

| Stage | Typical practice | What is lost | Evidence |
|---|---|---|---|
| Exploration | Deterministic inversion → single interpretation → drill the anomaly | Geometry and hypothesis uncertainty. Failures are not diagnosed ("one simply moves on to the next anomaly") | Caers (2025) [V]; Bond et al. (2007): 21% correct among 412 interpreters [V] |
| Prospectivity | Uncalibrated scores; random cross-validation; pooled analogues | Calibration; transfer error; workflow variance | Hronsky & Kreuzer (2019) [V]; Zhang et al. (2024) [V]; Oh et al. (2026) [V]; Ellefsen (2026) [V] |
| Sampling/assay | QA/QC as pass/fail gate; FSE rarely propagated | Measurement-error variance and method bias in the estimate | Gy/TOS literature [V-sub]; LeachWELL vs fire-assay biases [V-sub] |
| Resource | One kriged (often ID²) block model; capping and domaining as hidden choices; classification as a qualitative label | Local and global grade/tonnage uncertainty; conditional bias; variogram uncertainty | LUC slope ≈ 0.1 vs grade control (Assibey-Bonsu & Muller 2014) [V-sub]; Caers (2025) on IDW still in use [V] |
| Reserve | Modifying factors as point values; one pit shell at one price; dilution and recovery as constants | Joint geological–technical–economic uncertainty | Dimitrakopoulos et al. (2002): designs miss forecast NPV, tonnage and grade [V-sub] |
| Schedule and finance | Deterministic schedule → DCF; one-at-a-time ±% sensitivities; risk loaded into the discount rate | Dependence between variables; non-linearity (Jensen); tail risk | Standard practice [U]; Bertisen & Davis (2008) recommend Monte Carlo with shifted-lognormal capex [V] |
| Execution | Capex/opex estimates with claimed ±15% accuracy | Systematic optimism bias | **Bertisen & Davis (2008), 63 projects:** mean +14% overrun; about half outside ±15%; about 1 in 13 at least +100%; no learning over time; argued to be *intentional*, driven by financing needs [V] |
| Ramp-up | Nameplate assumed within months | Learning-curve and technology risk | McNulty (1998) ramp-up curve classes (some plants never reach design) [U]; Burmeister (1988) and Clow (1991) on gold-project under-performance [U] |
| Operations | F-factors used as fudge factors | Root-cause attribution; calibrated feedback | "Beyond Reconciliation" (2003) [V-sub]; JORC draft mandates reconciliation disclosure [V-sub] |

**Structural reasons, beyond technical ones:**
- **Incentives.** Bertisen & Davis argue the bias is rational under scarce project finance [V]. Junior explorers are funded on "hits," not on hypothesis falsification (Mern et al. 2024) [V].
- **Disciplinary silos.** Caers (2025) argues that "no agreed upon scientific method exists" across geology, geophysics and geochemistry, and that comprehensive software is missing [V].
- **Cognition:**
  - Inside-view "bold forecasts" (Kahneman & Lovallo 1993, *Mgmt. Sci.* 39:17–31) [V].
  - Planning fallacy and the remedy of **reference-class forecasting** (Flyvbjerg 2006, *PMJ* 37:5–15 [V]; Flyvbjerg 2014, *PMJ* 45:6–19 [V]).
  - Heuristics and biases (Tversky & Kahneman 1974) [V-sec].
- **Discovery economics context.** Exploration spending rose while major discoveries declined. Schodde (2017) is cited for this by Mern et al. (2024) and Caers (2025) [V-sec]; the original figures are [U].

### 9.2 Regulatory and code direction (2024–2026)

- **JORC draft:**
  - Defines **Risk as "the effect of uncertainty"** (ISO 31000 language) [V-sub].
  - Requires uncertainty-reflecting rounding [V-sub].
  - Mandates reconciliation disclosure [V-sub].
  - Replaces "eventual" with "more likely than not" for reasonable prospects, which is an implicitly *probabilistic* threshold [V-sub].
- **CRIRSCO (June 2024)** made the same RPEE change [V-sub]. NI 43-101 replacement and CIM definition proposals are pending [V-sub].
- **Implication for FrankenMining.** "More likely than not" invites a quantitative, auditable *Reasonable Prospects Assessment*: P(economic extraction) > 0.5 under stated price, cost and technology scenarios. FrankenMining can compute this as **evidence for** the CP's judgement. The codes still assign the decision to the CP, so the system must not automate classification.

### 9.3 A principled known / unknown / unknown-unknown accounting

The proposed formal taxonomy below is the author's synthesis, grounded in the cited statistics literature. Each claim in the system gets an **epistemic status** that is orthogonal to its **evidence class** and **provenance**.

**Epistemic status (what kind of not-knowing):**

1. **Known knowns (KK): measured, with characterized error.**
   - Direct measurements from a validated measurement system: QA/QC-passed assays with precision models, surveyed volumes, weighbridge tonnes, calibrated flowmeters.
   - Representation: value plus error model plus measurement-system identity.
   - KK status is *revocable*. A later CRM failure or bias study demotes it, and the invalidation cascades.
2. **Known unknowns (KU): modelled, with explicit probability.**
   - **KU-a, aleatory/spatial:** grade between holes, fragmentation per blast.
   - **KU-p, parametric:** variogram ranges, recovery-curve parameters, detection efficiencies qⱼ.
   - **KU-s, enumerated structural alternatives:** competing geological hypotheses, Zipf vs lognormal size models, alternative domaining.
   - Each KU carries a **calibration flag**:
     - *calibrated*: its predictive distributions have passed hindcast or reconciliation coverage tests in comparable conditions;
     - *uncalibrated*: model-asserted only;
     - *miscalibrated*: failed, with a measured correction applied.
3. **Unknown unknowns (UU): possibilities outside the model space.**
   - Cannot be enumerated, but can be:
     - **budgeted** via a catch-all hypothesis with non-zero prior mass, model-discrepancy terms, and reference-class widening;
     - **detected** via surprise monitors;
     - **hedged** via robust decision rules and option value;
     - **converted** into KU when a surprise triggers new hypotheses.
4. **Unknown knowns: the fourth quadrant, often forgotten.**
   - Knowledge the *organization* holds but the *model* does not: legacy reports, tacit geologist knowledge, unrecorded effort, prior failed drill programmes.
   - Mitigation: systematic digitization (KoBold's TerraShed is the commercial example [V]), structured expert elicitation, and making *effort* data first-class (§1.7).

**Evidence class (how it is known; orthogonal to the above):**
- `measured`, `derived` (deterministic transform of measured), `modelled-calibrated`, `modelled-uncalibrated`, `elicited` (expert, with elicitation record), `transferred` (analogue or pooled from elsewhere; AOA status), `assumed` (default or policy), `attested` (signed by CP/QP/EoR; a human accountability act, not a measurement).

**Mechanisms with literature support:**

| Mechanism | Purpose | Key refs |
|---|---|---|
| Prior-predictive falsification | Reject priors and hypotheses that cannot reproduce observed data before using them | Box (1980), *JRSS A* 143:383–430 [V]; Scheidt, Li & Caers (2018) [V]; Caers (2025) [V] |
| Null-hypothesis race | Detect when *all* human hypotheses are wrong | Mern et al. (2024) [V] |
| Model discrepancy | Prevent over-confident, biased parameters when the simulator or model is imperfect | Kennedy & O'Hagan (2001), *JRSS B* 63:425–464 [V]; Brynjarsdóttir & O'Hagan (2014), *Inverse Problems* 30:114007 [V] |
| Generalized / tempered posteriors | Robustness when the likelihood is misspecified | Grünwald & van Ommen (2017), *Bayesian Anal.* 12:1069–1103 [V]; Bissiri, Holmes & Walker (2016), *JRSS B* 78:1103–1130 [V] |
| Stacking over model averaging | Prediction under an "M-open" world, where Bayesian model averaging over-concentrates on one wrong model | Yao, Vehtari, Simpson & Gelman (2018) [V] |
| Proper scoring and calibration | Measure forecast quality and drive learning | Gneiting & Raftery (2007) [V]; Gneiting, Balabdaoui & Raftery (2007) [V]; geostatistical accuracy plots: Goovaerts (2001), *Geoderma* 103:3–26 [V] |
| Coverage guarantees | Distribution-free intervals; detect miscalibration | Angelopoulos & Bates (2023) [V]; spatial conformal: Mao, Martin & Reich (2024) [V]; mineral application: Ahn et al. (2026) [V] |
| Extrapolation detection | Flag transferred or out-of-domain predictions | Meyer & Pebesma (2021) AOA [V] |
| Structured expert elicitation | Make subjective inputs auditable and scorable | O'Hagan et al. (2006), *Uncertain Judgements* [V]; IDEA protocol (Hemming et al. 2018) [V]; Cooke's classical model [U] |
| Reference-class (outside-view) priors | Correct systematic optimism in costs, schedules and ramp-up | Flyvbjerg (2006, 2014) [V]; Kahneman & Lovallo (1993) [V]; Bertisen & Davis (2008) for mining capex [V] |
| Value of information | Decide whether reducing an uncertainty is worth it | Howard (1966) [V]; Bhattacharjya et al. (2010) [V]; Eidsvik et al. (2015) [V-sec]; Caers et al. (2022) EOI [V] |
| Entropy maps for geometry | Show where structural uncertainty sits | Wellmann & Regenauer-Lieb (2012) [V-sub] |
| Hindcast audits | Score long-horizon forecasts after the fact | Guj et al. (2011) Zipf audit [V]; Ellefsen (2026) USGS re-analysis [V] |

### 9.4 Communicating uncertainty (display rules)

Recommendations [synthesis, partly U]:
- Never show a single number without (a) an interval or distribution summary, (b) its evidence class, and (c) its calibration flag.
  - Example: "Contained Au: 2.1 Moz (P10–P90: 1.6–2.7), modelled-uncalibrated, 31% of the volume is outside the variogram range of data."
- Use the decision's own language: probability of meeting the mill-feed target per quarter, probability NPV > 0, expected shortfall (CVaR) in the bad decile.
- Show **what would change the answer.** VOI/EOI rankings of candidate data acquisitions and the hypotheses currently in play with their probabilities, including the catch-all.
- Show **staleness and coverage.** Last reconciliation, last QA/QC failure, last model validation, monitors offline (critical for TSFs).
- Keep the CP/QP's qualitative classification visually *separate* from the system's quantitative metrics, and show both. Codes classify, and the system provides evidence.

### 9.5 Implementation requirements (topic 9)

**1. The claim object (a universal record):**
- `id`, `quantity` (typed: tonnage, grade vector, recovery, NPV, probability…), `units`, and `support` (spatial volume or footprint, time window).
- `representation`: one of
  - a closed-form distribution;
  - an ensemble of realizations sharing a *realization index* with upstream claims;
  - an interval or bounds;
  - a qualitative category (code class).
- `epistemic_status` (KK / KU-a / KU-p / KU-s / UU-budget), `evidence_class`, `calibration_flag` with a link to its validation record.
- `provenance`: a DAG of input claims, transforms (code version, parameters, random seed), human acts (elicitations, sign-offs) and source documents.
- `uu_budget`: catch-all mass, discrepancy scale, reference-class widening applied.
- `validity`: conditions of validity (price deck, technology, domain), a valid-time window, and expiry or review date.
- `supersedes` / `superseded_by`: append-only history; claims are never edited in place.

**2. The forecast and calibration ledger:**
- Every forecast that will later be observable is *immutably* stored with its conditioning information at forecast time:
  - block grades for a mining period;
  - quarterly metal to mill;
  - plant recovery;
  - capex by WBS;
  - drill-hole outcome predictions;
  - prospectivity predictions for areas later explored.
- When outcomes arrive (reconciliation, production, actuals, discoveries), compute proper scores, PIT histograms and coverage, segmented by model, domain, horizon and author.
- The calibration results feed **hierarchical bias and variance-inflation models** that adjust future forecasts. This is the formal version of "learning from F-factors." The same ledger produces the JORC-draft-style reconciliation disclosures.

**3. Propagation semantics:**
- Pass **realization ensembles with a shared index** (common random numbers) across geology → plan → plant → finance, so that dependencies are preserved. Avoid moment propagation through non-linear operators (pit optimization, scheduling, comminution, flotation).
- Exogenous uncertainties (prices, FX, costs) are joined via explicit dependence models (copulas or joint stochastic processes) [U for specific choices].
- Reference-class priors (e.g., the shifted-lognormal capex overrun of Bertisen & Davis [V]) are applied as *explicit, visible* adjustments with their own claims.

**4. The decision ledger:**
- Each irreversible decision (drill programme, mine/abandon, pit selection, TSF raise) is recorded with the belief state, the alternatives considered, the VOI estimates and the sign-offs at decision time.
- This enables process-vs-outcome audits: a good decision can have a bad outcome, and vice versa.

**5. The surprise monitor service:**
- Continuously evaluates:
  - posterior-predictive p-values of incoming data;
  - the null-hypothesis race;
  - conformal coverage on rolling windows;
  - AOA status of new predictions;
  - reconciliation drift (CUSUM-type detection on F-factors).
- On alarm, it opens a *hypothesis review* workflow. It does not silently retrain.

**6. System-level validation: the synthetic-truth mine.**
- Build a generator for complete synthetic districts and mines: geology → deposits → drillholes with realistic sampling errors → assays with QA/QC defects → estimates → plans → operations with blasting and dilution → plant with measurement error → reconciliation.
- Run the whole FrankenMining chain on it, and test that realized outcomes fall within forecast distributions at nominal rates.
- This is the only way to validate end-to-end calibration before real reconciliation data accumulate. It mirrors simulation-based calibration in Bayesian workflow (Talts et al. 2018 [U]).
- Public building blocks:
  - MineLib instances for planning [V-sub];
  - the Walker Lake dataset for geostatistics (Isaaks & Srivastava 1989) [U];
  - published synthetic exploration problems (Mern & Caers 2023 code, MIT) [V].

---

## 10. Cross-cutting implementation requirements

**Core typing and data model:**
- **Units and dimensions** are typed; grades are compositional, so closure must be enforced.
- **Support/scale** is a first-class attribute of every spatial value: point, core interval, composite, SMU block, panel, stope, or pit shell. Operators that change support must be explicit (§4.1).
- **Coordinate reference systems:** EPSG plus local mine grids with versioned transformations; downhole desurveying (minimum curvature) [U] with survey-error propagation.
- **Bitemporal records:** "valid time" (when it was true in the world) vs "record time" (when the system learned it). This is essential for hindcasts, which require "what did we know at T?"
- **Event sourcing and immutability** for assays, QA/QC decisions, estimates, plans, sign-offs and decisions. Corrections are new events.

**Reproducibility and numerics:**
- **Reproducibility:** deterministic random-number streams per realization (counter-based RNG); model artefacts content-addressed by hash.
- **Numerical engines needed natively in Rust** (pure-Rust mandate):
  - sparse Cholesky and LDLᵀ for GMRF/SPDE;
  - dense linear algebra for kriging systems;
  - HMC/NUTS and SMC samplers, plus elliptical slice sampling;
  - particle filters;
  - max-flow/pseudoflow;
  - LP/MILP. A pure-Rust MILP solver of commercial grade is a significant gap [U]. Options: implement BZ-style decomposition plus specialized heuristics natively, or accept a solver dependency at the boundary.
  - Monte Carlo tree search (POMCPOW-class) and point-based POMDP solvers.

**Differential testing against open oracles (test-time only):**
These can be used at test time without violating runtime purity:
- MineFlow (C++, MIT) for UPIT [V-sub];
- MapMark4 (R) for three-part assessments [V];
- GemPy and LoopStructural (Python) for implicit modelling [V-sub];
- SimPEG (Python) for forward and inverse geophysics [V-sub];
- GSLIB-style programs for kriging and simulation [U];
- Mern & Caers' Julia code for POMDP exploration [V].

**Interoperability formats** [U]:
- Collar/survey/assay/lithology drillhole tables.
- OMF (Open Mining Format).
- GeoTIFF/COG rasters.
- LAS/DLIS logs.
- Industry block-model CSVs.
- RESQML is optional.

**Performance targets:**
- Ensembles of 100–1,000 realizations × 10⁷–10⁸ blocks are the stress case (Monte Carlo pit and schedule evaluation).
- Design for streaming, out-of-core blocks and parallel realization evaluation.

---

## 11. Open questions and research gaps (for the planning team)

1. **Effort data availability.** Can we assemble exploration-effort histories (drilling, surveys, sampling) for target districts? Without them, negative evidence cannot be used rigorously (§1.7). Candidate sources:
   - government drillhole databases and open-file reports;
   - company archives (unknown knowns);
   - tenement histories.
2. **Detection-efficiency calibration.** How do we estimate qⱼ(size, cover, method, era) credibly? Combine forward modelling of deposit signatures with hindcasts and elicitation; expect large uncertainty.
3. **Reward design for exploration POMDPs.** Information vs economics vs funding constraints is unresolved even in the leading papers (Mern et al. 2024) [V].
4. **Scalability of LGCP + thinning + marks** at continental scale with 3D (depth) structure. The 2D case is tractable with SPDE; 3D under-cover prospectivity is open.
5. **Validation of conditional simulations for calibration.** Stochastic mine planning assumes calibrated simulations, but published reconciliation-based calibration studies are scarce. The LUC study (slope ≈ 0.1) [V-sub] shows how wrong local estimates can be.
6. **Pure-Rust MILP/LP at mining scale.** A build-vs-boundary decision is needed.
7. **Codes in flux.** The final JORC 2026 content and the new NI 43-101/CIM definitions must be tracked. The rendering layer must be rule-versioned.
8. **Empirical base rates for mining-specific reference classes:**
   - capex overrun is verified (Bertisen & Davis 2008);
   - ramp-up, recovery shortfall, reserve-to-production grade bias and discovery-to-production timelines need verified datasets.
   - The McNulty, Burmeister, Clow and Gypton studies are cited from memory and must be retrieved and verified before use as priors [U].

---

## Appendix A. Key references with verified identifiers (topics 1 and 9 emphasis)

**Prospectivity, undiscovered resources, discovery bias:**
- Agterberg F., Cheng Q. (2002) Conditional independence test for weights-of-evidence modeling. *NRR* 11:249–255. doi:10.1023/A:1021193827501 [V]
- Ahn S. et al. (2026) Uncertainty-aware MPM with PU learning and conformal calibration: Ni, Yilgarn. *NRR*. doi:10.1007/s11053-026-10778-w [V]
- Alfonso C., Müller R.D., Mather B., Anthony M. (2024) Spatio-temporal copper prospectivity in the American Cordillera predicted by PU machine learning. *GSA Bull.* doi:10.1130/B37614.1 [V]
- Baddeley A. et al. (2010) Spatial logistic regression and change-of-support in Poisson point processes. *EJS* 4:1151–1201. doi:10.1214/10-EJS581 [V]
- Baddeley A. et al. (2020) Optimal thresholding of predictors in mineral prospectivity analysis. *NRR* 30:923–969. doi:10.1007/s11053-020-09769-2 [V]
- Bekker J., Davis J. (2020) Learning from positive and unlabeled data: a survey. *Mach. Learn.* 109:719–760. doi:10.1007/s10994-020-05877-5 [V]
- Bourdeau J.E., Parsa M., Lawley C.J.M. et al. (2026) Bridging the scale gap: regional refinement of national MPM for Nunavut. *NRR*. doi:10.1007/s11053-026-10699-8 [V]
- Caers J. (2025) The future of AI in critical mineral exploration. arXiv:2512.02879 [V]
- Caers J., Scheidt C., Yin Z., Wang L., Mukerji T., House K. (2022) Efficacy of information in mineral exploration drilling. *NRR* 31:1157–1173. doi:10.1007/s11053-022-10030-1 [V]
- Carranza E.J.M., Laborte A. (2015) Random forest predictive modeling of mineral prospectivity… *Comput. Geosci.* 74:60–70. doi:10.1016/j.cageo.2014.10.004 [V]
- Chakraborty A. et al. (2011) Point pattern modelling for degraded presence-only data over large regions. *JRSS C* 60:757–776. doi:10.1111/j.1467-9876.2011.00769.x [V]
- Coolbaugh M.F., Raines G.L., Zehner R.E. (2007) Assessment of exploration bias in data-driven predictive models and the estimation of undiscovered resources. *NRR* 16. doi:10.1007/s11053-007-9037-6 [V]
- Daruna A. et al. (2024) GFM4MPM: towards geospatial foundation models for MPM. ACM SIGSPATIAL. doi:10.1145/3678717.3691268 [V]
- Dorazio R. (2014) Accounting for imperfect detection and survey bias in presence-only data. *Global Ecol. Biogeogr.* 23:1472–1484. doi:10.1111/geb.12216 [V]
- Elkan C., Noto K. (2008) Learning classifiers from only positive and unlabeled data. KDD '08:213–220. doi:10.1145/1401890.1401920 [V]
- Ellefsen K.J. (2017) User's guide for MapMark4. USGS TM 7-C14. doi:10.3133/tm7C14 [V]
- Ellefsen K.J. (2026) Effect of mineral deposit data on predictions from the three-part approach… USGS SIR 2026-5031. doi:10.3133/sir20265031 [V]
- Fithian W., Elith J., Hastie T., Keith D. (2015) Bias correction in species distribution models: pooling survey and collection data for multiple species. *MEE* 6:424–438. doi:10.1111/2041-210X.12242 [V]
- Fithian W., Hastie T. (2013) Finite-sample equivalence in statistical models for presence-only data. *AoAS* 7:1917–1939. doi:10.1214/13-AOAS667 [V]
- Guj P., Fallon M., McCuaig T.C., Fagan R. (2011) A time-series audit of Zipf's law as a measure of terrane endowment and maturity in mineral exploration. *Econ. Geol.* 106:241–259. doi:10.2113/econgeo.106.2.241 [V]
- Hronsky J.M.A., Kreuzer O.P. (2019) Applying spatial prospectivity mapping to exploration targeting: fundamental practical issues… *Ore Geol. Rev.* 107:647–653. doi:10.1016/j.oregeorev.2019.03.016 [V]
- Johnson K.M., Hammarstrom J.M., Zientek M.L., Dicken C.L. (2014) Estimate of undiscovered copper resources of the world, 2013. USGS FS 2014-3004. doi:10.3133/fs20143004 [V]
- Kesler S.E., Wilkinson B.H. (2008) Earth's copper resources estimated from tectonic diffusion of porphyry copper deposits. *Geology* 36:255–258. doi:10.1130/G24317A.1 [V]
- Kirkwood C. et al. (2022) Bayesian deep learning for spatial interpolation in the presence of auxiliary information. *Math. Geosci.* 54:507–531. doi:10.1007/s11004-021-09988-0 [V]
- Lauzon D., Gloaguen E. (2024) Quantifying uncertainty and improving prospectivity mapping in mineral belts using transfer learning and random forest. *Ore Geol. Rev.* 105918. doi:10.1016/j.oregeorev.2024.105918 [V]
- Lawley C.J.M. et al. (2022) Data-driven prospectivity modelling of sediment-hosted Zn–Pb mineral systems… *Ore Geol. Rev.* 141:104635. doi:10.1016/j.oregeorev.2021.104635 [V]
- Lindgren F., Rue H., Lindström J. (2011) An explicit link between Gaussian fields and GMRFs: the SPDE approach. *JRSS B* 73:423–498. doi:10.1111/j.1467-9868.2011.00777.x [V-sub]
- McCuaig T.C., Beresford S.W., Hronsky J.M.A. (2010) Translating the mineral systems approach into an effective exploration targeting system. *Ore Geol. Rev.* 38:128–138. doi:10.1016/j.oregeorev.2010.05.008 [V]
- Mern J., Caers J. (2023) The Intelligent Prospector v1.0. *GMD* 16:289–313. doi:10.5194/gmd-16-289-2023 [V]
- Mern J., Corso A., Burch D., House K., Caers J. (2024) Intelligent prospector v2.0: exploration drill planning under epistemic model uncertainty. arXiv:2410.10610 [V]
- Meyer H., Pebesma E. (2021) Predicting into unknown space? Estimating the area of applicability of spatial prediction models. *MEE* 12:1620–1633. doi:10.1111/2041-210X.13650 [V]
- Møller J., Syversveen A.R., Waagepetersen R.P. (1998) Log Gaussian Cox processes. *Scand. J. Stat.* 25:451–482. doi:10.1111/1467-9469.00115 [V]
- Oh S., Bang M., Kim B. (2026) Validation design and cross-cluster transferability in ML-based prospectivity mapping of magmatic Ni sulphide systems. *Ore Geol. Rev.* 107313. doi:10.1016/j.oregeorev.2026.107313 [V]
- Ploton P. et al. (2020) Spatial validation reveals poor predictive performance of large-scale ecological mapping models. *Nat. Commun.* 11:4540. doi:10.1038/s41467-020-18321-y [V]
- Renner I.W. et al. (2015) Point process models for presence-only analysis. *MEE* 6:366–379. doi:10.1111/2041-210X.12352 [V]
- Roberts D.R. et al. (2017) Cross-validation strategies for data with temporal, spatial, hierarchical, or phylogenetic structure. *Ecography* 40:913–929. doi:10.1111/ecog.02881 [V]
- Rodriguez-Galiano V. et al. (2015) Machine learning predictive models for mineral prospectivity… *Ore Geol. Rev.* 71:804–818. doi:10.1016/j.oregeorev.2015.01.001 [V]
- Rue H., Martino S., Chopin N. (2009) Approximate Bayesian inference for latent Gaussian models by using INLA. *JRSS B* 71:319–392. doi:10.1111/j.1467-9868.2008.00700.x [V]
- Scheidt C., Li L., Caers J. (2018) *Quantifying Uncertainty in Subsurface Systems*. AGU/Wiley. doi:10.1002/9781119325888 [V]
- Simpson D. et al. (2016) Going off grid: computationally efficient inference for log-Gaussian Cox processes. *Biometrika* 103:49–70. doi:10.1093/biomet/asv064 [V]
- Singer D.A. (1995) World class base and precious metal deposits—a quantitative analysis. *Econ. Geol.* 90:88–104. doi:10.2113/gsecongeo.90.1.88 [V]
- Singer D.A. (2008) Mineral deposit densities for estimating mineral resources. *Math. Geosci.* 40:33–46. doi:10.1007/s11004-007-9127-3 [V]
- Singer D.A., Menzie W.D. (2010) *Quantitative Mineral Resource Assessments: An Integrated Approach*. Oxford UP [V-sec]
- Stone L.D., Keller C.M., Kratzke T.M., Strumpfer J.P. (2014) Search for the wreckage of Air France flight AF 447. *Stat. Sci.* 29:69–80. doi:10.1214/13-STS420 [V]
- Wadoux A.M.J.-C. et al. (2021) Spatial cross-validation is not the right way to evaluate map accuracy. *Ecol. Model.* 457:109692. doi:10.1016/j.ecolmodel.2021.109692 [V]
- Warton D.I., Shepherd L.C. (2010) Poisson point process models solve the "pseudo-absence problem" for presence-only data in ecology. *AoAS* 4:1383–1402. doi:10.1214/10-AOAS331 [V]
- Xiong Y., Zuo R. (2021) A positive and unlabeled learning algorithm for mineral prospectivity mapping. *Comput. Geosci.* 147:104667. doi:10.1016/j.cageo.2020.104667 [V]
- Yang F., Zuo R., Kreuzer O.P. (2024) Artificial intelligence for mineral exploration: a review and perspectives on future directions from data science. *Earth-Sci. Rev.* 258:104941. doi:10.1016/j.earscirev.2024.104941 [V]
- Yin Z., Strebelle S., Caers J. (2020) AutoBEL v1.0. *GMD* 13:651–672. doi:10.5194/gmd-13-651-2020 [V]
- Zhang S.E., Lawley C.J.M. et al. (2024) Workflow-induced uncertainty in data-driven mineral prospectivity mapping. *NRR*. doi:10.1007/s11053-024-10322-8 [V]
- Zuo R. et al. (2021) Uncertainties in GIS-based mineral prospectivity mapping: key types, potential impacts and possible solutions. *NRR* 30:3059–3079. doi:10.1007/s11053-021-09871-z [V]

**Decision analysis, epistemics and uncertainty quantification:**
- Angelopoulos A.N., Bates S. (2023) Conformal prediction: a gentle introduction. *Found. Trends ML* 16:494–591. doi:10.1561/2200000101 [V]
- Bertisen J., Davis G.A. (2008) Bias and error in mine project capital cost estimation. *Eng. Economist* 53:118–139. doi:10.1080/00137910802058533 [V]
- Bhattacharjya D., Eidsvik J., Mukerji T. (2010) The value of information in spatial decision making. *Math. Geosci.* 42:141–163. doi:10.1007/s11004-009-9256-y [V]
- Bissiri P.G., Holmes C.C., Walker S.G. (2016) A general framework for updating belief distributions. *JRSS B* 78:1103–1130. doi:10.1111/rssb.12158 [V]
- Bond C.E., Gibbs A.D., Shipton Z.K., Jones S. (2007) What do you think this is? "Conceptual uncertainty" in geoscience interpretation. *GSA Today* 17(11):4–10. doi:10.1130/GSAT01711A.1 [V]
- Box G.E.P. (1980) Sampling and Bayes' inference in scientific modelling and robustness. *JRSS A* 143:383–430. doi:10.2307/2982063 [V]
- Brynjarsdóttir J., O'Hagan A. (2014) Learning about physical parameters: the importance of model discrepancy. *Inverse Problems* 30:114007. doi:10.1088/0266-5611/30/11/114007 [V]
- Dimitrakopoulos R. (1998) Conditional simulation algorithms for modelling orebody uncertainty in open pit optimisation. *Int. J. Surf. Min. Reclam. Environ.* 12:173–179. doi:10.1080/09208118908944041 [V]
- Flyvbjerg B. (2006) From Nobel Prize to project management: getting risks right. *PMJ* 37(3):5–15. doi:10.1177/875697280603700302 [V]
- Flyvbjerg B. (2014) What you should know about megaprojects and why. *PMJ* 45(2):6–19. doi:10.1002/pmj.21409 [V]
- Gneiting T., Raftery A.E. (2007) Strictly proper scoring rules, prediction, and estimation. *JASA* 102:359–378. doi:10.1198/016214506000001437 [V]
- Gneiting T., Balabdaoui F., Raftery A.E. (2007) Probabilistic forecasts, calibration and sharpness. *JRSS B* 69:243–268. doi:10.1111/j.1467-9868.2007.00587.x [V]
- Goovaerts P. (2001) Geostatistical modelling of uncertainty in soil science. *Geoderma* 103:3–26. doi:10.1016/S0016-7061(01)00067-2 [V]
- Grünwald P., van Ommen T. (2017) Inconsistency of Bayesian inference for misspecified linear models, and a proposal for repairing it. *Bayesian Anal.* 12:1069–1103. doi:10.1214/17-BA1085 [V]
- Hemming V. et al. (2018) A practical guide to structured expert elicitation using the IDEA protocol. *MEE* 9:169–180. doi:10.1111/2041-210X.12857 [V]
- Howard R.A. (1966) Information value theory. *IEEE TSSC* 2:22–26. doi:10.1109/TSSC.1966.300074 [V]
- Kahneman D., Lovallo D. (1993) Timid choices and bold forecasts. *Mgmt. Sci.* 39:17–31. doi:10.1287/mnsc.39.1.17 [V]
- Kennedy M.C., O'Hagan A. (2001) Bayesian calibration of computer models. *JRSS B* 63:425–464. doi:10.1111/1467-9868.00294 [V]
- Lindi O.T. et al. (2024) Uncertainty quantification in mineral resource estimation. *NRR* 33:2503–2526. doi:10.1007/s11053-024-10394-6 [V]
- Mao H., Martin R., Reich B.J. (2024) Valid model-free spatial prediction. *JASA* 119:904–914. doi:10.1080/01621459.2022.2147531 [V]
- O'Hagan A. et al. (2006) *Uncertain Judgements: Eliciting Experts' Probabilities*. Wiley. doi:10.1002/0470033312 [V]
- Rossi M.E., Deutsch C.V. (2014) *Mineral Resource Estimation*. Springer. doi:10.1007/978-1-4020-5717-5 [V]
- Sinclair A.J., Blackwell G.H. (2002) *Applied Mineral Inventory Estimation*. Cambridge UP. doi:10.1017/CBO9780511545993 [V]
- Vehtari A., Gelman A., Gabry J. (2017) Practical Bayesian model evaluation using LOO-CV and WAIC. *Stat. Comput.* 27:1413–1432. doi:10.1007/s11222-016-9696-4 [V]
- Yao Y., Vehtari A., Simpson D., Gelman A. (2018) Using stacking to average Bayesian predictive distributions. *Bayesian Anal.* 13:917–1007. doi:10.1214/17-BA1091 [V]

**Topics 2–8:** references and their verification tags are given inline in §§2–8 (mostly [V-sub], from the topic-specific sub-agent verification passes).

**Items explicitly UNVERIFIED and needing retrieval before normative use:**
- Wyborn et al. (1994).
- Bonham-Carter, Agterberg & Wright (1989).
- Kaufman, Balcer & Kruyt (1975); Drew (1997); Schuenemeyer & Drew (1983).
- Merriam, Drew & Schuenemeyer (2004).
- Stone's *Theory of Optimal Search*.
- Talts et al. (2018).
- Cooke (1991).
- McNulty (1998); Burmeister (1988); Clow (1991); Gypton (2002); Mackenzie & Cusworth (2007).
- Parker (2012) reconciliation principles (a guessed DOI was wrong).
- F-factor and MCF definitions as cited.
- Discrete Gaussian model and UC primary references (Matheron; Rivoirard 1994); GSLIB.
- Standard formula statements for Bond, Kuz-Ram, Swebrec and Klimpel (forms believed standard).
- QA/QC threshold conventions.
- The Scheidt, Neto, Yin & Caers (2026) EOI drillhole-ranking paper.
