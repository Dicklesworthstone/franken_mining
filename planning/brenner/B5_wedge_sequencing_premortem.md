# B5 — Wedge, sequencing and pre-mortem (PQ-6)

RUN_ID: fm-2026-10-28-a · Pass: B5 (independent Brenner-style hypothesis pass) · Date: 2026-10-28
Inputs: `planning/mor/MOR_CASE.md` (E001–E033), R4 §0–2 and §7–10, R5 (kill gates, translations, anti-patterns),
R6 (all), R7 §4.2–4.3, §7.6, §8, §9, the franken_accounting plan §0–1, the alien-artifact-coding skill, and
web checks run on 2026-10-28 (§1). Nothing outside this file was modified.

**Labels.** [FACT] sourced (E### or URL). [DESIGN] a choice this pass recommends. [HYP] a testable belief.
[TARGET] a number to hit, not a result. [OPEN] undecided or unknown. Web items also carry V-P (primary read),
V-S (secondary or wire copy) or UNV. Numbers in this file are targets, budgets or public figures.
None is a FrankenMining result.

---

## 0. Verdict in brief

- **Recommended wedge [DESIGN]: H-f, "Guidance & Reserve Assurance" (GRA).** It narrows H-b from a system of
  record to a read-only assurance overlay. It has three parts:
  - a mine-to-mill-to-market reconciliation posterior (joint state estimation, E029);
  - a versioned metric engine for AISC/GEO/C1/reserve replacement (RR) with certified causal waterfalls;
  - a forecast-and-calibration ledger (E032) that scores every forecast it makes.
  Its outputs reach executives as DecisionCards (H-e). It is built on a deliberately narrow H-d evidence spine.
- **Why this wedge.**
  - It speaks to about half of Barrick's company bonus (GEO 25%, gold AISC 18.75%, copper AISC 6.25%) and 15%
    of the long-term incentive (RR) (E025).
  - It needs neither frankensim (E001) nor franken_accounting (E019) to reach month 12.
  - It shows value in three states of the world: no data access (public lane), synthetic truth, and one site.
  - It positions FrankenMining as the evidence substrate that agents consume, which complements Avathon (E024).
    It does not replace Avathon.
- **Not the wedge.** H-c (full twin) collides with Avathon, the OEMs and frankensim maturity all at once. H-a
  (prospectivity) is the second wedge, at M4. Its proof horizon is long, and a Nevada hindcast has few
  positives to score against.
- **Twelve-month demonstrator [TARGET].** M2 is a shadow pilot at one site, preferably 100%-owned and outside
  the NGM JV and the Avathon perimeter; Lumwana is the candidate [HYP]. Its proof is at least one attested
  material finding the site did not already have, plus pre-registered forecasts. If there is no site data by
  month 7, the fallback M2′ is parent-level public assurance plus the Nevada hindcast.
- **Top pre-mortem risks.** Avathon politics and the Chair's link to Avathon. No data access. MNPI and lane
  contamination. QP and legal refusal. "Yet another dashboard". Key-person risk on both sides. Details in §7.

---

## 1. Web checks that change or sharpen MOR_CASE (2026-10-28)

| # | Finding | Label | Effect on MOR_CASE |
|---|---|---|---|
| W1 | The Avathon–Barrick NA release went out on PR Newswire on **23 Sep 2026** (R6 used the 25 Sep trade-press date). The release lists six use cases. One is "Production and recovery: linking ore flow, processing, maintenance, and operating constraints from mine to mill". Others are mine planning, and "exploration and growth" (ML on geological data). No KPIs, terms or timelines are given. Barrick staff keep "operational judgment, accountability and control". ([PR copy](https://finviz.com/news/394742/avathon-selected-to-power-an-ai-native-mining-operating-model-for-barricks-north-american-business)) | FACT V-P (wire copy) | E024: Avathon's stated scope **includes mine-to-mill**, which overlaps H-b, H-c and H-f. Date fix. |
| W2 | Reported terms: a 5-year deal with a base fee plus outcome fees, worth $10–20M/yr. Collaboration since Dec 2025. **Live at select US sites, including the Nevada complex, since Q2 2026.** Full NGM deployment within 1–2 quarters; the Dominican Republic (Pueblo Viejo) next. ([Deep News via Tiger](https://www.itiger.com/news/1177296302), unnamed source) | FACT V-S / terms UNV | Avathon is not just announced; it is being deployed at NGM. Pilots at NGM or PV are contested ground. |
| W3 | Avathon–Draslovka alliance (Feb 2026) combines Blue Cube in-line slurry analysers, the MetOptima AI metallurgical advisor and Avathon's platform for leach recovery and reagent use. Draslovka has prior Barrick work: a glycine-leach pilot at Bulyanhulu and commercial use announced in Feb 2024. ([search summary](https://www.globalminingreview.com/mining/13022026/draslovka-and-avathon-partner-to-deliver-ai-powered-solutions-for-mining-through-autonomy-metoptima-and-blue-cube-combined-offering/amp/); [Draslovka–Barrick 2024](https://www.businesswire.com/news/home/20240229113006/en/Draslovka-and-Barrick-Gold-Commence-Commercial-Use-of-Glycine-Leaching-Technology-for-Gold-Mining)) | FACT V-S | Avathon has a processing channel into Barrick **outside NA** as well. Plant optimisation is crowded. |
| W4 | **Newmont served Barrick a notice of default on 3 Feb 2026.** It alleged NGM mismanagement and diversion of resources to the wholly owned Fourmile. The 10 Aug 2026 settlement (R6 fact 4) resolved it. ([Bloomberg](https://www.bloomberg.com/news/articles/2026-02-20/newmont-spars-with-barrick-over-nevada-venture-as-notice-sent); [Mining Weekly](https://www.miningweekly.com/article/newmont-flags-mismanagement-at-ngm-sends-default-notice-to-barrick-2026-02-20); [N. Am. Mining](https://northamericanmining.com/index.php/2026/02/23/newmont-accuses-barrick-of-ngm-mismanagement/)) | FACT V-S | Closes R6 §9 item 5 in part. **NGM operating data has recently been litigation-adjacent.** A variance-attribution tool at NGM produces JV evidence (pre-mortem F13). |
| W5 | The NA Barrick team includes Wessel Hamman (CFO) and Tim Cribb (COO), reporting to Mark Hill. The IPO is reported at about 10–15% of Barrick's interests and is still "targeted by end-2026". I found no public F-1. ([mining.com](https://www.mining.com/barrick-names-north-america-team-ahead-of-ipo/); [Barrick 28 Apr 2026](https://www.barrick.com/English/news/news-details/2026/barrick-advances-IPO-of-north-american-gold-assets/default.aspx)) | FACT V-S; "no F-1" is a checked null (a confidential draft may exist: UNV) | E024: name the NA CFO and COO as personas. The IPO calendar is a bandwidth risk (F12). |
| W6 | The Reko Diq review was reported to run **to mid-2027**. Phase 1 capex $5.6–6.0B; Phase 2 $3.3–3.6B (Pakistan ministry, May 2026). ([Mining Weekly Apr 2026](https://www.miningweekly.com/article/reko-diq-coppergold-project-pakistan-update-2026-04-10)) | FACT V-S; mid-2027 UNV | Partly verifies R6 §9 item 1. A live DecisionCard candidate, but politically sensitive. |
| W7 | JORC 2012 is still operative. The provisional 2026 code is still under parent-body, ASX and ASIC review, with about a one-year transition after finalisation. The NI 43-101 replacement is not final. The June 2025 proposal **requires specific disclosure of QP data verification per technical-report item**. ([JORC Mar 2026](https://www.jorc.org/code-update); [OSC](https://www.osc.ca/en/securities-law/instruments-rules-policies/4/43-101/csa-notice-and-request-comment-proposed-repeal-and-replacement-national-instrument-43-101)) | FACT V-S | Confirms E031. A data-verification evidence index is a regulatory tailwind for H-f, but it is not yet a mandate. |
| W8 | **What decisive industry proofs look like:** Freeport "TROI" at Bagdad. McKinsey reports throughput up about 10% quarter on quarter and copper recovery up about 1 percentage point, with no new capital. ([McKinsey](https://www.mckinsey.com.br/capabilities/tech-and-ai/our-insights/freeport-mcmoran-turns-data-into-value)) | FACT V-S (consultant-reported) | Refuter for H-f (§3): the proofs the industry believes are **operational uplifts** at one asset, not assurance. |
| W9 | **How twins actually stall:** Metso's Geminex metallurgical twin at Newmont Lihir. "Technical deployment … completed", but "full operational commissioning is still in progress due to recent personnel changes at the plant". No measured benefits are published. ([Metso, updated Nov 2025](https://metso.com/insights/case-studies/mining-and-metals/optimizing-from-mine-to-mill-to-mine-with-digital--twin-technology-at-newmont-lihir-gold-mine)) | FACT V-P (vendor page) | Supports rejecting H-c as the wedge, and pre-mortem F5 and F12. |
| W10 | The Anglo–Teck merger was still pending regulatory approval in Aug 2026, with completion expected Sep 2026–Mar 2027. No digital programme announced. ([GMR Dec 2025](https://www.globalminingreview.com/finance-business/10122025/anglo-american-and-teck-shareholders-approve-merger-of-equals-to-form-anglo-teck)) | FACT V-S | Competitor context only. Integration years tend to freeze new platform bets [HYP]. |

Checked nulls:
- Barrick Q3 2026 results are not out yet (due 9 Nov 2026, per R6).
- No Barrick digital-twin or ML-exploration announcement for Fourmile was found.
- No 2026 Newmont twin or AI announcement beyond the AI Standard (R6) was found.

---

## 2. Decision frame (alien-artifact discipline)

- **Goal.** Within about 12 months, convince Tier-One executives that FrankenMining changes a decision they care
  about. The test is that an executive uses a FrankenMining number or card in a decision or guidance meeting.
- **Constraints.**
  - Rust and `Cx`; a closed dependency universe; consume, never fork (MOR_CASE).
  - frankensim is unconverged (E001). FA is immature (E019).
  - No Barrick data is assumed (MOR_CASE unknowns).
  - Software never signs anything (E023, E031).
- **Asymmetric failure costs, from MOR_CASE.**
  1. A false-certainty number reaching a disclosure, a capital decision or a dam. This is the worst outcome.
  2. A wedge that management cannot see value in.
  3. An architecture that cannot scale. This matters, but not yet for the wedge.
- **Observables available now.**
  - Public filings (EDGAR HTML exhibits, E018 sidesteps PDF).
  - Public geoscience for Nevada.
  - A synthetic-truth mine (E032).
  - Executive pay metrics (E025).
- **Method budget.** At most three method families for the wedge:
  - Bayesian data reconciliation (state estimation on a flow graph);
  - calibration and conformal coverage (forecast scoring);
  - decision theory (DecisionCards, VOI, reference-class priors).
  Everything else waits for its milestone. The skill's anti-pattern is "advanced terms with no compiled artifact".

---

## 3. Hypotheses (mechanism, owner, falsifier, 4-week test, dependencies, overlap)

### H-a — Exploration prospectivity and capital-allocation engine first

- **Mechanism [HYP].** Calibrated, effort-aware prospectivity (E028) raises the hit rate per exploration dollar.
  Deal-structure optimisation (earn-in, stake, option; R6 §4.7) raises the value per dollar committed. Both feed
  reserve replacement and Tier-One pipeline value (R6 fact 7).
- **Who feels it.** The Head of Exploration first, then the CEO (Hill was "integral" to Fourmile, R6 §1.1) and the
  capital committee. The CFO feels it only through the exploration budget.
- **Falsifier.** A temporal hindcast (train on deposits known at year T, score later discoveries) does no better
  than the USGS three-part parity baseline (E028), or too few positives leave the CIs unable to separate them.
  Or exploration leaders say the binding constraint is land access, permitting or deal flow, not target ranking.
- **4-week test.** A Nevada public-data hindcast: deposits known at 2000 versus discoveries 2000–2025 (USGS MRDS /
  NBMG, availability [UNV]); a log-linear Poisson intensity versus a three-part baseline; PR-AUC with bootstrap CIs
  and the **count of positives**. With fewer than about 15 in-tract post-2000 discoveries the test cannot
  discriminate. That would itself show H-a cannot prove value fast [HYP].
- **Data dependency.** Public data suffices for the demo. Effort data is the binding gap (R7 §11 Q1). Real value
  needs Barrick's exploration database, which is MNPI-heavy (E023).
- **Suite dependency.** Moderate to high, though not on frankensim's unconverged core. LGCP/SPDE, samplers and
  GeoTIFF/EPSG are absent from the suite (E018, E010); sparse Cholesky would need lifting (E003).
- **Overlap.** Avathon's "exploration and growth" (W1); Seequent/Bentley Target/Evo; Fleet Space (Barrick listed as
  a user, V-S); KoBold, VerAI, Earth AI. Crowded at "AI targeting"; less so at effort-aware calibration plus deal
  structuring.

### H-b — Reconciliation / metal-accounting / AISC-and-reserve system of record first

- **Mechanism [HYP].** One authority per number (R5 #5), from block to bullion to books. F-factors become
  posteriors (E029). AISC and RR are computed once under versioned definitions and certified, so disclosure
  inconsistencies (E025) disappear.
- **Who feels it.** The CFO and controller, the Head of MRM and QPs, the audit committee, and the COO through grade
  variance.
- **Falsifier.** Barrick's SAP-plus-site-spreadsheet process already reconciles within tolerance and the E025
  differences are deliberate bases (R-b2), or finance will not move any system of record during the IPO
  carve-out (W5).
- **4-week test.** The H-f public lane below. A system-of-record claim cannot be tested in 4 weeks.
- **Data dependency.** Internal: SAP S/4HANA, site metallurgical accounting, dispatch/FMS tonnages, survey (E024).
- **Suite dependency.** **High.** A system of record means FA as the book, and FA lacks metal units, CloseRun,
  proven performance and an active INV-12 (E019, E021; R4 §9 P0). Plus the 17-part domain pack (E021).
- **Overlap.** SAP (the incumbent system of record); Datamine Reconcilor; Maptek PETRA/MAXTA (R6 §5.1); Avathon
  mine-to-mill (W1).

### H-c — Full operating digital twin first

- **Mechanism [HYP].** A live state of pit, fleet, plant and TSF enables optimisation and what-ifs across the chain.
- **Who feels it.** The COO, site GMs and plant managers. Executives only through operational KPIs, months later.
- **Falsifier.** Live feeds sit behind OEM APIs (Komatsu FMS/FrontRunner, Pitram, PI, DISPATCH) or IT/OT security
  that will not open within 12 months, or the twin deploys but is not adopted (W9, Lihir).
- **4-week test.** A paper integration inventory listing every live feed with its owner and vendor. If more than
  half need OEM or OT-network cooperation, H-c cannot ship in 12 months [HYP]. Expected: it fails; NGM alone runs
  Komatsu AHS with a new FMS, Pitram underground, and SAP (E024).
- **Data dependency.** Real-time OT: the highest of any hypothesis.
- **Suite dependency.** **Highest.** frankensim physics and geometry, all `Cx`-coupled (E004); ICP, Bingham
  rheology, Mohr-Coulomb and DEM missing (E007, E010); fgdb lacks valid time and a spatial index (E013).
- **Overlap.** **Direct:** Avathon (W1, W2), Komatsu/Modular, Cat MineStar, Hexagon, Maptek, Metso Geminex,
  Draslovka+Avathon (W3), and OEM consolidation (E027).

### H-d — Evidence spine plus a thin vertical slice (synthetic-truth mine plus one public district)

- **Mechanism [HYP].** Prove every layer composes, geology → estimate → plan → operate → process → reconcile →
  account, with every number carrying evidence (E008, E032). Architecture risk (loss #2) is retired early.
- **Who feels it.** Engineers, QPs as sceptical reviewers, the CTO/CIO. **Executives mostly do not**: a thin slice
  through ten layers looks like ten toys.
- **Falsifier (for executive value).** Three outside mining professionals see the slice and would not take a
  follow-up meeting on their own data.
- **4-week test.** Run the synthetic-truth mine v0 through reconciliation with injected biases; ingest one public
  district; count layers with known-answer tests passing; run the "would you take a meeting" test with three
  ex-mining reviewers.
- **Data dependency.** None (synthetic plus public).
- **Suite dependency.** Moderate if every layer is touched (geometry is `Cx`-coupled, E004); low if narrowed.
- **Overlap.** None. Internal infrastructure.

### H-e — "Decision assurance for the board": a DecisionCard with an epistemic map on every material decision

- **Mechanism [HYP].** Every material capital, reserve or dam decision carries a card with alternatives and a loss
  matrix (E012 DecisionContract); reference-class priors (Bertisen & Davis +14% mean, about half outside ±15%;
  McKinsey >40%; R7 §9.1, E026); a KK/KU/UU map (E032); VOI on what would change the decision; and sign-offs.
  Decisions improve at the margin and the process becomes auditable.
- **Who feels it.** The CEO, the board, the audit and risk committee (which oversees AI risk, R6 §1.7), and the CFO
  for capital.
- **Falsifier.** Decision quality is unobservable within 12 months; directors read the cards as consultant paper;
  counsel blocks discoverable probabilistic records (F4).
- **4-week test.** Two or three retrospective cards built **as known at the decision date** (bitemporal discipline
  to control hindsight) on public cases, e.g. Lumwana Super Pit, the Reko Diq 2026 slowdown, an external case.
  Blind review by two ex-mining executives: did a card surface a consideration absent from the public rationale?
- **Data dependency.** Public for retrospective cards. Board papers for real ones, which are highly sensitive.
- **Suite dependency.** Low: fs-evidence (E008), franken-decision (E012).
- **Overlap.** Strategy consultancies (a director ran McKinsey's AI for mining, R6 §1.1). Low software overlap.

### H-f (constructed) — "Guidance & Reserve Assurance": H-b narrowed to a read-only overlay, delivered through H-e cards, built on a narrowed H-d spine

- **Mechanism [HYP].** Executives do not lack data; they lack **numbers with a track record**. GRA provides three
  things:
  1. a reconciliation posterior showing where metal goes between block model and bullion, with uncertainty and
     attribution to model bias, dilution, stockpile, sampler or recovery (E029, R7 §4.3);
  2. a metric engine that reproduces every published AISC/GEO/RR figure from cited components under named
     definitions and explains the differences between bases (E025);
  3. a calibration ledger in which every forecast is frozen before the outcome and scored after (E032).
  The executive artifact is a **certified causal waterfall** (why AISC or GEO moved, by cited driver) plus a
  forecast with a visible hit rate.
- **Who feels it.** The CFO (guidance credibility, AISC waterfall); the COO ("lower grades processed" is the most
  repeated explanation in both majors' 2026 releases, R6 §6.1); the Head of MRM and QPs (reconciliation evidence
  ahead of JORC's coming requirement, W7); the audit committee (reproducibility); the CEO (fewer guidance
  surprises).
- **Falsifiers.** (i) The pilot posterior yields **no attested finding** the site lacked. (ii) Public reproduction
  fails for more than 20% of site-quarters. (iii) Executives see the waterfall as already available from FP&A.
- **4-week test.** Reproduce Barrick's per-site AISC and GEO for 8 or more quarters from public MD&A components,
  every cell cited to an EDGAR accession and table (per-site tables believed present [HYP], verify in week 1).
  Reconstruct the reserve roll-forward and RR under each disclosed basis (504% versus ~190%, E025). **Freeze hashed
  FY2026 per-site forecasts** before the Feb 2027 release. Plus a synthetic-truth reconciliation with injected
  biases.
- **Data dependency.** None for M0–M1. For M2, one site's **read-only snapshots**, never live: OMF block models, GC
  model, dispatch tonnages, surveys, mill sampler, metallurgical balance, SAP cost centres.
- **Suite dependency.** Low, with **no frankensim and no FA before M3**: liftable fs-evidence, fs-eproc and
  fs-robust (E003); franken-evidence and franken-decision (E012); exact decimals; dense linear algebra for WLS or
  Gaussian reconciliation.
- **Overlap.** Datamine Reconcilor; Maptek PETRA; SAP for AISC; Avathon mine-to-mill. Differentiation is
  **epistemic** (calibrated posteriors, scored forecasts, certificates, refusals). "We also do reconciliation" is
  not a differentiator.

### H-g (constructed, for completeness) — TSF evidence state / GISTM traceability or critical-control verification first

- **Mechanism [HYP].** Show evidence state per failure mode (R7 §8.4): stale monitors, last EoR and ITRB review.
  GISTM has 77 requirements across 64 facilities (R6 §1.8). Critical-control verifications count for 5% of the
  bonus (E025).
- **Falsifier.** Barrick's gap is Reko Diq security, not tooling (14 of 18 E/VH facilities conform, R6 §1.8). CCV
  is a field-workflow problem, and Avathon's safety computer vision is first in line (W1).
- **Verdict preview.** A strong M5 module and a poor first impression for an unknown vendor in a safety-critical
  domain.

---

## 4. Refuter-first: the strongest case against each hypothesis

Each refuter is stated as strongly as I can make it. Where it lands, the verdict in §5 changes shape.

**Against H-a (prospectivity first).**
- R-a1 **Proof horizon.** Discovery to production takes 15.7–17.9 years (E026). A 12-month demo can only show
  a hindcast, and a hindcast on about 10–20 positives has CIs wide enough to make any lift ambiguous [HYP].
  Executives have seen AI-targeting decks since 2018. The best-funded AI explorers capture value by **owning
  assets**, not by selling software (R6 §5.2 [INF]).
- R-a2 **Culture.** Barrick's growth narrative credits geologists (Fourmile, "geologically driven additions",
  R6 §1.3). A model that "would have found Fourmile" reads as hindsight. A model that would not have found it
  reads as useless.
- R-a3 **Missing primitives.** It needs samplers, SPDE, GeoTIFF and EPSG. None exist in the suite (E018). The
  pure-Rust tax is highest here.
- R-a4 **MNPI.** Real value needs live drill results, which are MNPI (E023). That is the worst place to start
  with an unknown vendor.
- *What survives:* exploration **capital allocation** at the parent level (earn-ins, stakes, options; PQ-7) is
  not in Avathon's announced scope (R6 §7.2c). It needs decision theory more than geostatistics. → M4.

**Against H-b (system of record first).**
- R-b1 **SAP is the system of record and the IPO freezes finance.** Barrick runs S/4HANA in more than 15
  countries (E024). NA Barrick must carve out its SAP and reporting for a new SEC registrant during 2026–27 (W5).
  No CFO swaps the book of record mid-carve-out. FA itself is not ready to be one: no metal units, no CloseRun,
  118–439 ms per post (E019).
- R-b2 **The "inconsistency" may not be an error.** RR "29% / 3-yr 504%" versus "~190%", and comp AISC $1,423
  versus reported $1,637, are probably **deliberate, defensible bases** (R6 §1.2, §1.5). Two hypotheses:
  - the 504% may include gold-equivalent Reko Diq additions [HYP];
  - comp AISC may normalise price-linked royalties to the plan price [HYP].
  Pitching "your numbers are inconsistent" to the CFO who signs them is an insult with a citation. **Checked
  null:** R6 could not reconcile them from public documents. That does not mean they are unreconciled
  internally [OPEN].
- R-b3 **Reconciliation products exist.** Datamine Reconcilor, Maptek PETRA (bought Mar 2026), metallurgical-
  accounting packages, and now Avathon's mine-to-mill use case (W1). "Another recon tool" is a procurement
  line, not a strategic conversation.
- R-b4 **Assurance is not uplift.** The proofs the industry believes are operational uplifts: TROI at Bagdad
  (+10% throughput, +1 pt recovery, W8) and autonomous haulage with an IRR above 35% (R6 §6.8). Assurance
  saves embarrassment, not ounces. Executives pay for ounces.
- *What survives:* R-b1 kills "system of record" but not "assurance overlay". R-b2 changes the framing from
  "you are wrong" to "here is the bridge, published once, replayable". R-b3 and R-b4 force the wedge to produce
  an **attested material finding** (an ounce-level discovery inside reconciliation, not just tidiness) and a
  **scored forecast track record**. Neither is something Reconcilor or SAP provides [HYP].

**Against H-c (full twin first).**
- R-c1 Every live feed is owned by an OEM or OT network (E027, R6 §1.7). The Lihir twin stalled on people after
  technical deployment (W9). Avathon is being deployed at NGM now (W2). frankensim does not build cleanly, and
  only one capability is at L3 (E001). This is the most direct collision possible on every axis.
- *What survives:* nothing as a wedge. The "twin of record" (a certified read substrate) is the M6 form.

**Against H-d (spine plus thin slice first).**
- R-d1 **Executives cannot see infrastructure.** Loss #3 in MOR_CASE is "a wedge management cannot see value
  in". The FrankenTextiles lessons are that scope grew 2.7× and the plan turned into a log (E022, R5 §8). A
  slice through every layer invites both.
- R-d2 **The slice proves composition on synthetic data, which QPs discount.** "It fits the synthetic, not the
  orebody" (R5 §3.2 data-readiness gate).
- *What survives:* the spine is mandatory, but it must be **narrowed to the wedge's layers**: evidence, units,
  material-flow graph, reconciliation, metrics, calibration ledger and DecisionCard. The synthetic-truth mine
  covers only the chain from geology to grade control to stockpile to mill to reconciliation.

**Against H-e (board decision assurance first).**
- R-e1 Its value cannot be observed in 12 months. The cards look like consulting deliverables, and the board
  already buys those. Probabilistic records the board ignores become exhibits later (F4).
- R-e2 Retrospective cards on Barrick's own disasters (Pascua-Lama, a $5.2B write-down in Q2 2013, R6 §6.3) are
  politically toxic. Using a competitor's (Tanami Expansion 2, R6 §6.2) is safer but less relevant.
- *What survives:* the card is the right **delivery format** for executives. It is not a standalone wedge.

**Against H-f (the recommended composite).** This is the refuter I take most seriously.
- R-f1 **Data access is the whole bet.** Without one site's data by month 7, H-f is a public-filings parlour
  trick: anyone with EDGAR can reproduce AISC [HYP]. Countermeasure: M2′ (§5) and the pre-registration track
  record, which an EDGAR-reader does not have.
- R-f2 **Good sites produce null results.** If the pilot site reconciles well, the posterior agrees with the
  site and there is no finding. That would be a correct, non-decisive outcome. Countermeasure: choose a site
  with recorded grade or recovery variance, and pre-agree that a calibrated null is reported as a null.
- R-f3 **Avathon's mine-to-mill use case (W1) can claim the same space** at NA sites. Countermeasure: pilot
  outside the NA perimeter, and expose a read API so that agents cite FrankenMining certificates (bet B3).
- R-f4 **Guidance forecasts on internal data are MNPI.** Countermeasure: lane separation (F3).

---

## 5. Verdicts and sequencing logic

### 5.1 Verdicts

| Hyp | Verdict | Role in the plan |
|---|---|---|
| H-a | **Deferred, not rejected** [DESIGN] | Second wedge at M4. Lead with deal-structure capital allocation (PQ-7) and an effort-aware hindcast. ML claims only if the hindcast beats parity. |
| H-b | **Rejected as system of record; kept as assurance** | System of record waits for FA convergence, at M3 at the earliest, via the R4 §7 adapter. |
| H-c | **Rejected for 24 months** | Becomes the M6 "twin of record" read substrate. Never a control system. |
| H-d | **Mandatory but narrowed** | The engineering spine beneath H-f. A budget protocol from day one (E022). |
| H-e | **Adopted as the delivery format** | Every executive output is a DecisionCard. Retrospective public cards at M1. |
| H-f | **Recommended wedge** | M0–M3. |
| H-g | **Deferred to M5** | TSF evidence state after trust is earned. Never "safe" or "unsafe" (R7 §8.4). |

### 5.2 EV matrix (skill format: Impact × Confidence / Effort, each scored 1–5) [HYP; scores are this pass's elicitation]

| Candidate | Impact | Confidence | Effort | EV score | Relevance (0.35/0.25/0.20/0.20) |
|---|---|---|---|---|---|
| H-f GRA overlay plus cards | 4 | 3.5 | 2.5 | **5.6** | 4.4 (symptom 5, architecture 4, proof 4, operability 4) |
| H-e cards alone | 3 | 2.5 | 1.5 | 5.0 | 3.3 |
| H-d full thin slice | 2 | 4 | 3 | 2.7 | 3.0 |
| H-g TSF / CCV | 3 | 2.5 | 3 | 2.5 | 3.0 |
| H-a prospectivity first | 4 | 2 | 4 | 2.0 | 2.9 |
| H-b system of record | 4 | 2 | 5 | 1.6 | 2.4 |
| H-c full twin | 5 | 1 | 5 | 1.0 | 1.6 |

### 5.3 Galaxy-brain card: the choice of wedge under data-access uncertainty

- **Equation.** EV(h) = p·V(h | S1) + (1−p)·V(h | S2) − λ·T(h).
  - S1: site data access by month 7. S2: none in year 1.
  - V is the executive-perceived decisive value at month 12 (0–10).
  - T is the tail exposure to loss #1 (a false-certainty number reaching a disclosure, capital decision or dam).
- **Substituted values** [HYP, elicited]: p = 0.5 [OPEN]; λ = 1.

  | h | V(S1) | V(S2) | T | EV |
  |---|---|---|---|---|
  | H-f | 8 | 4 | 0.5 | **5.5** |
  | H-a | 4 | 3 | 1.0 | 2.5 |
  | H-e | 4 | 3 | 1.0 | 2.5 |
  | H-d | 3 | 2 | 0 | 2.5 |
  | H-b | 3 | 1 | 1.0 | 1.0 |
  | H-c | 2 | 0 | 2.0 | −1.0 |

- **Minimax regret.** H-f's worst regret is 0 in both states under these numbers, so it dominates.
- **Intuition.** H-f is the only option that is useful with **no data** (public lane plus synthetic truth) and
  decisive with **some data** (one site). Every other option is either data-starved (H-c, H-b) or slow to
  show value (H-a, H-e).
- **Assumptions that bound validity.**
  - Executives care about guidance credibility, which is high-weight in pay (E025).
  - Public filings contain per-site components [HYP, verify in M0 week 1].
  - A pre-registered scored forecast is persuasive [B2].
- **What would change the decision.**
  - V_f(S2) < 3: the M0 public lane impresses no one. Then shift weight to H-a plus PQ-7 for the parent.
  - The sponsor's stated pain is exploration or capital allocation, not guidance. Then start M4 at month 3,
    in parallel.
  - Barrick says reconciliation is solved and shows the evidence. Then H-f loses its finding mechanism. Pivot
    to H-e plus PQ-7.

### 5.4 Dependency firewall (how the wedge avoids unconverged siblings) [DESIGN]

| Milestone | frankensim (E001) | FA (E019) | fgdb (E013) | Lifted only (E003) |
|---|---|---|---|---|
| M0–M2 | **none** | **none** (local exact ledger; FA-compatible IDs and idempotency keys per R4 §7.1) | none (frankensqlite, E014) | fs-evidence, fs-eproc, fs-robust, franken-evidence, franken-decision |
| M3 | none | `fa` robot CLI on a **draft book**, summaries only (R4 §7.11–7.12); local fallback | optional | + fs-voi |
| M4 | none | none | optional | + fs-voi, fs-oed-e2e patterns; native SPDE / sparse (fsci-sparse caveat, E017) |
| M5–M6 | lifted crates at L3 only (G2 oracle discipline) | CloseRun if live | if spatial needs are met | + fs-uq, fs-assimilate (dense cap 256, E006) |

Rule [DESIGN]: **no milestone's executive deliverable may be blocked by a sibling repo.** A sibling capability
enters only behind an adapter with a working local fallback. This applies "subset-never-substitute" (E015) to
schedule risk.

---

## 6. Sequencing ladder M0–M6 (about 36 months from Nov 2026)

Each milestone has an artifact, acceptance criteria [TARGET], a kill gate with a fallback, and the pay metrics it
touches (E025). Honest wording: software does not *move* AISC. It moves the accuracy, timeliness and attribution
of AISC, which management then acts on. "Touches" means the decision inputs behind that metric.

**M0 — Public Mirror plus Spine (months 0–3, Nov 2026–Jan 2027).**
- *Artifact:* `fmine assure public --issuer barrick` (robot-first CLI, E015), emitting:
  (a) per-site AISC/TCC/GEO/grade/recovery for Q1 2024–Q3 2026, every cell citing an EDGAR accession and table
  locator; (b) metric-definition registry v1 (WGC 2013/2018 AISC/AIC, R6 §4.1; by-product versus co-product; GEO
  price ratio; named RR bases); (c) a discrepancy register for the E025 items, each bridged with citations or
  marked OPEN with the document that would close it; (d) **FY2026 per-site forecasts frozen and hashed before the
  Feb 2027 release**, with 80% intervals and flag `uncalibrated`; (e) the evidence spine (claim object R7 §9.5,
  ledger, refusal codes); (f) a security pack (SBOM, threat model, offline single binary).
- *Acceptance:* ≥95% of site-quarter values reproduced to published rounding, the rest explained or refused;
  byte-identical reruns; zero uncited numbers; the forecast hash timestamped before the release.
- *Kill gate:* reproduction below 80%, or two finance reviewers (Barrick or an ex-major advisor) rate it "nothing
  new". **Fallback:** the public metric engine becomes test fixtures only; weight shifts to M1 recon and an early M4.
- *Pay metrics:* gold AISC 18.75%, copper AISC 6.25%, GEO 25% (API); RR 15% (PGSU). Measurement integrity only.

**M1 — Synthetic truth, reconciliation known answers, retrospective cards (months 3–6, Feb–Apr 2027).**
- *Artifact:* a synthetic-truth generator (one open-pit Au, one Cu–Au) with seeded biases in the resource model,
  dilution, stockpile survey, mill sampler and density; a reconciliation engine (Bayesian/WLS data reconciliation
  on the material-flow graph, F-factors as hierarchical bias posteriors; E029, R7 §7.6); calibration ledger v1;
  scoring of the FY2026 public forecasts at release, **reported whatever they show**; two retrospective
  DecisionCards built as known at decision time (H-e test).
- *Acceptance:* over ≥200 replicates, 90% intervals on each injected bias cover truth 85–95% of the time; metal
  balance closes within stated measurement uncertainty; non-identifiable bias pairs (F1/F2 compensation, R7 §4.3)
  return a typed refusal, not a number; detection power for a 5% sampler bias over one quarter is stated.
- *Kill gate:* coverage outside 80–97% after one fix cycle, or identifiability not characterised. **Fallback:** no
  site pitch on reconciliation; lead the site conversation with the metric engine and H-a/PQ-7.
- *Pay metrics:* none directly. Builds the COO and MRM credibility M2 needs.

**M2 — Site Assurance Pilot in shadow mode (months 6–12, May–Oct 2027). The 12-month demonstrator.**
- *Preconditions:* a signed data agreement; an insider list and blackout policy (F3); a passed IT security
  review; a named site sponsor plus QP and metallurgist. Site preference: 100% owned, outside NGM and PV (W2, W4),
  with recorded grade or recovery variance. Lumwana is the candidate [HYP]; North Mara/Bulyanhulu (84%) alternates.
- *Artifact:* an on-prem single binary ingesting read-only snapshots for 8–12 historical quarters (OMF block and GC
  models, E027; dispatch tonnages; surveys; mill sampler; metallurgical balance; SAP cost-centre extracts).
  Outputs: **parity reproduction of the site's own monthly recon pack**; the posterior and attribution; a
  per-quarter AISC waterfall from tonnes, grade and recovery to unit cost; a rolling-origin hindcast; two forward
  quarters pre-registered **inside the tenancy**; a Site Assurance DecisionCard.
- *Acceptance:*
  1. Parity with the site's F1/F2/F3 from the same inputs, every difference explained.
  2. **At least one material finding** that the site QP or metallurgist attests `Reviewed{against}` as new and
     plausible. "Material" is pre-registered: ≥1% of contained metal, or ≥ a $M/yr threshold set with the sponsor.
  3. Hindcast coverage reported with a small-sample caveat (8–12 quarters cannot establish calibration).
  4. No data egress, verified by Barrick IT.
  5. Recon-pack time down ≥50%, or the site elects to co-sign.
- *Kill gate (month 12):* no data by month 7 → **M2′** (parent-level metric and reserve assurance on public data,
  the Nevada H-a hindcast, PQ-7 deal cards). Data but zero attested findings → report the null honestly and offer
  **one** second site with known pain. A second null → H-f is falsified as the wedge; pivot to H-e plus PQ-7.
- *Pay metrics:* GEO and AISC (site guidance accuracy and variance attribution); RR indirectly (reserve-model bias
  feeds RPEE).

**M3 — Group metric and reserve assurance plus the FA bridge (months 12–18).**
- *Artifact:* a certified AISC/AIC/C1/TCC engine reading SAP extracts at the pilot and consolidating published
  group figures; a reserves and resources register (roll-forward: opening, depletion, revisions, acquisitions,
  disposals; RR under every named basis; QP attestation workflow, R4 §5.15); a data-verification evidence index
  per technical-report item (W7); a JORC-style reconciliation section drafted for QP review if the code is in
  force; an FA draft book with statistical roll-forward and summary metal subledger via the `fa` robot CLI
  (R4 §7.5, §7.11).
- *Acceptance:* three years of published reserve statements reproduced; an auditor replays the AISC certificate
  offline; FA dry-run plan certificates for one month of summary posts, or a documented FA blocker list (R4 §9 P0).
- *Kill gate:* FA lacks metal units or acceptable performance at month 15. **Fallback:** a FrankenMining-local
  certified ledger with FA as a draft adapter. No executive deliverable slips.
- *Pay metrics:* RR 15% (PGSU), AISC, ROCE (reproducible capital-employed definition, R6 §8).

**M4 — Exploration and capital allocation, the second wedge (months 15–24).**
- *Artifact:* a Nevada public-district model (thinned marked point process with effort, E028, plus a USGS
  three-part parity mode with MapMark4 as lab oracle); a temporal hindcast; one Barrick region on internal data,
  outside NA if the Avathon conflict persists; junior earn-in, stake and option DecisionCards with
  distance-to-Tier-One (R6 §1.3), which is the PQ-7 object.
- *Acceptance:* three-part parity on a published tract; hindcast PR-AUC versus baseline with CIs and the count of
  positives; PIT/coverage reported; at least one live deal evaluated **before** the decision, recorded as a
  decision-ledger entry (R7 §9.5 item 4).
- *Kill gate:* the hindcast cannot beat parity or has too few positives. **Fallback:** ship parity, effort
  accounting and the deal-structure engine without ML lift claims.
- *Pay metrics:* RR (3-year rolling), ROCE, strategic execution (15% of the PGSU).

**M5 — Multi-site rollout, TSF evidence state, group calibration ledger (months 24–30).**
- *Artifact:* 3–5 sites; a group calibration ledger with a surprise monitor (CUSUM on F-factors that opens a
  hypothesis review and never retrains silently, R7 §9.5 item 5); guidance ranges derived from calibrated
  intervals; a GISTM evidence-state map for E/VH facilities (H-g).
- *Acceptance:* ≥3 sites use it as, or co-sign it as, their recon pack of record; calibration reported over ≥24
  site-quarters; the TSF map covers all 77 requirements for ≥3 facilities, with staleness and **no safety claim**
  (`geotechnical_signoff` never set, E023).
- *Kill gate:* fewer than 2 sites adopt after 6 months. **Fallback:** freeze the rollout; keep it as an audit and
  assurance tool.
- *Pay metrics:* AISC, GEO, ESG and licence to operate (TSF), Class 1 incidents (indirect).

**M6 — Closing the loop: planning under calibrated uncertainty and a twin of record (months 30–36).**
- *Artifact:* a stochastic LOM evaluation at one site using simulations whose bias and variance are **calibrated
  by M2–M5 reconciliation** (E029 → E030); VSS measured on the site's own plan; a read-only "twin of record" API
  that any agent, Avathon's included, can cite; frankensim crates only where they are at L3.
- *Acceptance:* G2 known answers (pseudoflow equals MineFlow/MineLib optima exactly, R5 §3.19); VSS reported with
  CIs; at least one external agent consumes certificates.
- *Kill gate:* frankensim not converged, or the optimiser differs from the oracle. **Fallback:** keep the vendor
  optimiser as the oracle. "Do not ship an optimizer that is merely different" (R5).
- *Pay metrics:* ROCE, GEO, AISC, RR.

**Program-level gate (month 12) [DESIGN].** If neither M2 nor M2′ yields an executive-recognised use, the program
is re-scoped before M3 spending. "Executive-recognised use" means a FrankenMining number or card cited in a
guidance, capital or reserve meeting minute, or the sponsor's written equivalent. This is the skill's "stop when
the bottleneck has shifted" rule applied to the program.

---

## 7. Pre-mortem: "It is 2029 and FrankenMining failed at Barrick. Why?"

Each story gives the failure, a leading indicator visible by about month 6, a countermeasure to bake in now, and
the residual risk. They are ordered roughly by likelihood × impact [HYP].

**F1 — Avathon absorbed the space.** The Chair is Avathon's lead director (E024). Avathon was live at NGM from
Q2 2026 (W2), and its mine-to-mill use case (W1) became the default home for anything "AI". FrankenMining was
routed to the Avathon program office and asked to "integrate into the knowledge graph", then starved.
- *Indicator:* meetings get redirected to NA digital leadership; the pilot is offered only at NGM or PV; the
  question becomes "how is this different from Avathon?" instead of "when can we start?"
- *Countermeasure:* position as the **evidence substrate agents cite** (B3); ship an M2 read API Avathon's agents
  can call; never pitch computer vision, maintenance or supply-chain agents; pilot outside the NA perimeter;
  never be the party that raises the Chair conflict.
- *Residual:* **high**. This is political and cannot be designed away. The sponsor's courage is the variable.

**F2 — Data never arrived.** IT security (under the CFO's function, R6 §1.1) took 9 months on an unknown pure-Rust
vendor with no SOC 2 and no references.
- *Indicator:* no signed data agreement or named data steward by month 4; the security questionnaire stalls.
- *Countermeasure:* the M0 security pack (SBOM, reproducible offline binary, no egress, read-only snapshot
  imports); run on Barrick hardware; the public and synthetic lanes keep delivering value with no data (M2′).
- *Residual:* medium. Policy may forbid any non-panel software. Then the only channel is a partner (F15).

**F3 — MNPI breach or lane contamination.** A team member with site access also maintained the public forecasts.
A forecast leaked, or someone traded in a blackout. Drill and reconciliation data are MNPI (E023), and the
period overlapped an IPO.
- *Indicator:* no insider list; the public and private lanes share a repo or machine; forecasts are discussed
  outside NDA.
- *Countermeasure:* **two physically separate lanes**; people with private access never author public-lane
  forecasts; private forecasts never leave the tenancy; insider list and trading blackout for the whole team;
  counsel reviews any external use of public-lane forecasts (default: share with the sponsor only).
- *Residual:* low probability, **fatal impact** (reputational and legal).

**F4 — QPs refused, and legal agreed.** QPs saw probabilistic block metrics as automation of their judgment and
as discoverable contradictions of signed classifications. Counsel kept the system out of technical reports.
- *Indicator:* the QP declines `Reviewed{against}` attestations; legal asks to remove intervals from QP-facing
  views.
- *Countermeasure:* never classify (E031); a CP-act record makes the QP the author; "evidence for" labels with
  `no_claim` lines (R5 #20); `reserve_disclosure_claim` never set (E023); co-design with the Head of MRM from
  M1; counsel designs the retention and privilege map.
- *Residual:* medium. Discoverability is a governance choice FrankenMining cannot make.

**F5 — The site ignored it.** A respected senior geologist's spreadsheet stayed the recon of record. Barrick
"empowers site leadership" (R6 §1.1), so the site simply said no. Lihir's twin was technically deployed and
stalled on personnel (W9).
- *Indicator:* after 3 shadow months, the monthly recon meeting still uses the old pack; usage logs show only the
  project team.
- *Countermeasure:* **parity first** (reproduce the site's own pack exactly, adopting its definitions as a
  versioned site profile); the site geologist is the named author; reduced pack time is the hook; embed a
  person on site.
- *Residual:* medium. Champion turnover.

**F6 — Yet another dashboard.** Executives saw charts, said "interesting", and changed nothing. EY's "proofs of
concept rather than scalable solutions" (R6 §6.4) held true.
- *Indicator:* no executive reuses a FrankenMining number in a meeting; requests are for UI polish, not decisions.
- *Countermeasure:* the unit of delivery is a **DecisionCard plus a scored forecast** (a claim with a track record,
  not a view); success is measured by decisions influenced and the §6 program gate; CLI and certificates first,
  UI downstream.
- *Residual:* medium.

**F7 — Overclaimed probabilities.** A "90%" interval missed 4 of 8 quarters in front of the CFO. Or the board read
"P(NPV>0)=0.83" as a fact. Or "more likely than not" (W7) was automated into an RPEE threshold.
- *Indicator:* calibration-ledger coverage below nominal; intervals narrower than reference-class spreads.
- *Countermeasure:* default flag `uncalibrated`, with the track record beside every forecast; UU budget and
  reference-class widening (R7 §9.3); refuse decision-grade probabilities without calibration evidence; state
  small-sample limits explicitly.
- *Residual:* medium. Readers' misuse is cultural.

**F8 — The value was not there (checked null).** The pilot site reconciled well. The posterior agreed with the
site, and there was nothing new to attest.
- *Indicator:* by month 9 the attribution matches the site's F-factor commentary.
- *Countermeasure:* pre-select a site with recorded grade or recovery variance; report the null honestly; allow
  one second site (§6 M2 gate).
- *Residual:* medium. A calibrated null is a valid result, but it is not a wedge.

**F9 — The suite never converged.** The team spent 30–40% of its time on frankensim, FA, the asupersync 0.5/0.6
skew (E011) and fgdb fixes. Milestones slipped behind upstream beads.
- *Indicator:* more than 20% of engineering hours on sibling repos; blocked beads pointing outside the repo.
- *Countermeasure:* the §5.4 dependency firewall; lift with provenance (E002, E003); FA through the CLI behind an
  adapter (R4 §7.11); no executive deliverable may depend on a sibling.
- *Residual:* low to medium. Duplicated effort if lifted code drifts.

**F10 — Purity cost the year.** Rebuilding HTML and table parsing, plotting, CRS handling, OMF readers and
samplers in pure Rust ate M0–M1, and the 12-month demonstrator missed its window.
- *Indicator:* M0 slips more than 4 weeks; infrastructure exceeds 40% of LOC.
- *Countermeasure:* a wedge chosen for minimal missing primitives (no MILP; no kriging, since vendor models are
  consumed; no PDF, since EDGAR is HTML); reuse MIT-licensed `omf-rust` (E027) if admitted [OPEN]; foreign tools
  as lab oracles; a budget protocol from day one (E022).
- *Residual:* medium.

**F11 — Key-person risk on both sides.** The owner was the only relationship and the lead architect. On Barrick's
side, Hill moved to NA Barrick, the parent's CEO was unnamed, the CFO had been new since Mar 2026, and the GC left
in Jun 2026 (R6 §1.1).
- *Indicator:* one person answers every executive question; the sponsor changes role.
- *Countermeasure:* multi-thread sponsors (CFO plus Head of MRM plus one site GM); plan-first documentation;
  DecisionCards make the reasoning transferable; a deputy for the relationship.
- *Residual:* high for a small team.

**F12 — The IPO ate the calendar.** The NA carve-out (SAP, reporting, S-K 1300 for a new registrant) consumed
finance, IT and MRM through 2027. "After the IPO" became forever.
- *Indicator:* repeated deferrals; perimeter changes.
- *Countermeasure:* stay off the IPO critical path; pilot in the remaining-parent perimeter; offer
  carve-out-relevant evidence indexes only if asked.
- *Residual:* medium. Timing.

**F13 — JV partner conflict.** NGM's 2026 default notice (W4) showed that operating data can become dispute
evidence. A variance-attribution waterfall at a JV site became an exhibit, or needed partner consent that never
came. Partners include Newmont at NGM and PV, Shandong at Veladero, and AngloGold at Kibali.
- *Indicator:* legal asks whether partner consent is needed; requests to "not write that down".
- *Countermeasure:* pilot at a 100%-owned site; partner-facing outputs only through governed channels; a
  privilege structure designed by counsel.
- *Residual:* low once the site choice is made.

**F14 — OEM lock-in.** Block models, FMS cycles and Pitram data could not be exported with the needed fields.
Vendors slow-rolled exports as paid services.
- *Indicator:* export requests need vendor professional services; OMF exports drop sub-blocks or attributes.
- *Countermeasure:* OMF v2 from day one (E027); import-only snapshot publishers (R5 #50); flat-file fallbacks;
  never require write or real-time APIs; Barrick invokes its data-ownership clauses.
- *Residual:* medium for real-time; low for monthly snapshots.

**F15 — Procurement and vendor viability.** A tiny pure-Rust shop failed vendor onboarding: insurance, financial
standing, support SLA, source escrow, references.
- *Indicator:* procurement asks for three references and a SOC 2 report.
- *Countermeasure:* a delivery or assurance partner (an audit or engineering firm) as the contracting wrapper;
  source escrow; a single binary with a support runbook.
- *Residual:* medium.

**F16 — Scale broke trust.** At M5–M6, 10⁸–10⁹ blocks and 100–1,000 realizations (R7 §10) overwhelmed the stack:
fgdb has no spatial index (E013), FA posts at 118–439 ms (E019), and the dense Belief cap is 256 (E006).
- *Indicator:* the M2 site's mined-volume subset takes hours to load; ensembles miss the monthly cycle.
- *Countermeasure:* the wedge needs only mined SMUs per period (about 10⁵–10⁶ blocks) [HYP]; content-addressed
  objects (E016); performance figures are budgets until benchmarked (R5 #19); FA receives summaries only
  (R4 §7.12).
- *Residual:* deferred, not removed. Loss #2 returns at M6.

---

## 8. Front matter for the plan

### 8.1 Thesis: the fossils

The mining software stack is a chain of capable tools held together by one epistemic habit: **each stage passes
the next a single number and discards how it knows it.** The tools mostly support better practice. Below,
[FACT] marks documented practice (R7 §9.1). Product-specific limits are [HYP], because no authoritative public
critique of named products exists (R6 §5.1).

- **Estimation** (Leapfrog, Vulcan, Datamine, Surpac) is fossilized around *one estimated block model*.
  - Uncertainty survives only as a class label assigned afterwards.
  - Kriging's conditional bias goes unreported (LUC slope about 0.1 against grade control, E029).
  - Capping and domaining are hidden choices.
- **Strategic planning** (Whittle, Deswik, XPAC, Evolution) is fossilized around *one shell at one price* and a
  deterministic schedule. Spatial schedule compliance is reported as low as 30% (E026).
- **Fleet and operations** (DISPATCH, MineStar, FrontRunner, Pitram) is fossilized around *telemetry owned by
  the company that sells the machine* (E027). Integrations run one way.
- **Reconciliation** (Reconcilor, spreadsheets) is fossilized around *F-factors as retrospective fudge
  multipliers* (R7 §4.3).
  - F1 and F2 errors compensate each other.
  - Mill head grade, itself a back-calculation, is treated as truth.
  - Nothing feeds back into the next estimate.
- **Plant metal accounting** (historians, AMIRA P754 balances) is rigorous but *stops at the plant gate*. It is
  disconnected from the model that predicted the feed.
- **Finance** (S/4HANA, XERAS, spreadsheets) is fossilized around *cost centres, not material flows*.
  - The sustaining versus non-sustaining split in AISC is a spreadsheet judgment (R6 §4.1).
  - The same pay metric appears on different bases in different documents (E025).
- **Capital allocation** is fossilized around *spreadsheet DCF*.
  - Sensitivities are run one at a time, and risk is buried in the discount rate.
  - Capex overruns average +14%, half fall outside the claimed ±15%, and there is no learning over time (R7 §9.1).
  - The fiscal, security and JV shocks that actually moved Barrick's capital in 2025–26 are absent from the
    models (R6 §6.6).
- **Exploration** is fossilized around *uncalibrated scores validated by random cross-validation*, where
  absence of evidence is read as absence (E028).
- **Tailings** is fossilized around *conformance checklists*. An offline piezometer reads as "normal"
  (R7 §8.4).
- **The newest layer, AI operating platforms** (knowledge graph plus agents), adds fluent reasoning over
  unreconciled, unversioned inputs. Its outcome fees are not tied to a published, scored baseline (W1, W2)
  [HYP characterisation].

Every hard sub-problem has been solved somewhere in isolation:
- geostatistical simulation;
- P754 metal accounting;
- Bayesian data reconciliation;
- reference-class forecasting;
- proper scoring and conformal coverage;
- Bayesian search theory;
- the JORC draft's mandatory reconciliation (E031).

**No shipping system composes them** into one ledger where:
- every number carries its support, evidence class and calibration flag;
- every forecast is frozen and scored;
- every material decision cites both.

FrankenMining is that composition. Its first product sits at the one place where every model in a mine meets
reality: **reconciliation**.

### 8.2 Bets (each with its falsifier)

| # | Bet | Falsifier |
|---|---|---|
| B1 | **Reconciliation is the truth signal.** A calibrated recon posterior is the highest-leverage object in a mine. | At two pilot sites the posterior yields no attested finding beyond the existing F-factor report (M2). |
| B2 | **Scored beats persuasive.** Executives value a forecast with a visible track record over a better-looking model. | After four scored quarters, no guidance or capital discussion cites the calibration record. |
| B3 | **Assurance complements agents.** AI operating layers raise demand for a certified evidence substrate. | Barrick and Avathon decline a read-API integration, and sponsors say the knowledge graph suffices. |
| B4 | **One definition, many renderings.** Versioned rule-sets for AISC, RR, GEO, C1 and reserve codes remove a recurring class of disclosure friction. | Finance shows the E025 bases are already bridged internally, at no measurable cost. |
| B5 | **Honest uncertainty is adoptable** if the system never classifies, signs or decides (E023, E031). | QPs or counsel refuse to let the system's probabilistic records exist, even after the design mitigations. |
| B6 | **Synthetic truth buys time.** A synthetic-truth mine plus a public lane establish credibility before data access. | M0–M1 artifacts do not move the data-access decision. |
| B7 | **Narrow purity is affordable.** The wedge needs only primitives the suite can supply in 12 months. | M0 plus M1 slip by more than 50% because of missing infrastructure. |
| B8 | **Exploration is an options portfolio.** Effort-aware prospectivity plus deal structuring improves capital allocation at the margin. | The Nevada temporal hindcast cannot beat the three-part parity baseline with calibrated intervals (M4). |

### 8.3 Readers and their questions

| Reader | Their question |
|---|---|
| CEO (parent; NA Barrick: Hill, W5) | "Which of my numbers can I trust, and which decision this quarter would change if I knew more?" |
| CFO (Cai; NA: Hamman) | "Why did AISC move against guidance, which parts are controllable, and can I replay every published non-GAAP figure from source?" |
| COO (NA: Cribb) and regional COOs | "Where is metal lost between the block model and the bullion: geology or execution?" |
| Head of Exploration | "Own drilling, earn-in, stake or walk away, and how sure are we?" |
| Head of MRM and QPs | "Does reconciliation confirm my model, does the evidence support my classification, and will this system ever speak for me?" |
| Audit and risk committee; external auditor | "Is every disclosed number reproducible, and who attested what, when?" |
| Accountable Executive (tailings) and EoR | "What is the evidence state of each failure mode, and what is stale?" |
| Site GM and resource geologist (the adopters) | "Does this make my monthly pack faster and more defensible?" |
| CLO / counsel (McGuire) | "What does this create that is discoverable, and does it ever make a disclosure claim?" |
| Group IT / security | "What leaves our network, and what runs where?" |
| AI agents (internal or third-party) | "Give me certified, cited numbers, or a typed refusal I can act on." |

### 8.4 North-Star scenarios (before → after) [TARGET]

- **NS-1 AISC waterfall.** Before: the CFO asks why gold AISC moved from $1,637 (FY2025) to $1,866 (Q2 2026), and
  FP&A returns a narrative citing "lower grades, fuel, royalties" three days later. After: a certified causal
  waterfall (grade, recovery, tonnes, price-linked royalty, fuel, FX, sustaining capex), each bar citing its
  evidence, replayable offline.
- **NS-2 RR bridge.** Before: "504%" and "~190%" sit in different documents. After: one page renders RR under each
  named basis from one cited roll-forward.
- **NS-3 Monthly reconciliation.** Before: a five-day spreadsheet exercise. After: a posterior in hours, with
  F1/F2/F3 intervals and a gross error localised to the mill sampler, which the metallurgist attests.
- **NS-4 Guidance setting.** Before: ranges set by negotiation. After: ranges derived from calibrated site
  intervals, with the record shown alongside (e.g. "5 of the last 6 quarters inside the 80% band").
- **NS-5 QP support.** Before: classification rationale sits in the QP's head. After: per-domain evidence (data
  spacing, slope of regression, simulation probability) beside the QP's signed act, and a reconciliation
  section drafted for the QP's review.
- **NS-6 Expansion card.** Before: a single capex point estimate. After: a reference-class-shifted capex
  distribution, P(overrun > 25%), VOI of further geotechnical drilling, and "what would change our mind".
- **NS-7 Junior deal.** Before: a spreadsheet for one structure. After: earn-in, stake, option and walk-away
  compared on the same prospectivity posterior and stage gates, with distance-to-Tier-One.
- **NS-8 Hindcast honesty.** "Would we have flagged Goldrush or Fourmile with what was known in 2005?" The answer
  is shown with its CI and its count of positives, including a null.
- **NS-9 Stockpile truth.** Before: survey, dispatch and weightometer are averaged. After: tonnage is admitted
  only when the three agree within tolerance; otherwise it is `unknown` (R5 #51).
- **NS-10 Price-deck change.** Before: the reserve price deck moves from $1,400 to $1,500 (R6 fact 7) and weeks of
  re-runs follow. After: reserves, DD&A inputs, impairment headroom and Tier-One filters recompute with new
  digests and no stale caches.
- **NS-11 Surprise monitor.** CUSUM on F2 trips, and a hypothesis-review case opens with the evidence attached.
  Nothing retrains silently.
- **NS-12 TSF evidence state.** Before: a green dashboard. After: "4 of 11 failure modes monitored; 2 piezometers
  stale more than 14 days; last ITRB review 7 months ago". The system never says "safe".
- **NS-13 Auditor replay.** The external auditor verifies the AISC certificate and its FA bundle offline (R4 §2.8)
  without access to the live system.
- **NS-14 Agent query.** An agent, Avathon's or internal, asks for metal in circuit last week and receives a
  certified figure or `NoClaimInDomain: mill sampler offline 2 days` (E008).

### 8.5 Anti-goals

- No replacement of SAP, Pitram, DISPATCH, FrontRunner, Leapfrog or Whittle in years 1–2. No fleet or plant
  control. FrankenMining never commands equipment.
- Never set `reserve_disclosure_claim`, `execution_authorized`, `geotechnical_signoff` or
  `regulatory_compliance_claim` (E023). Never classify resources or reserves (E031).
- No computer-vision safety, predictive-maintenance or procurement agents. Do not compete with Avathon on
  agents.
- No dashboard-first product. No probability without a calibration flag. No single number without an interval,
  an evidence class and a source.
- No private-data forecast leaves the tenancy. No public-lane author holds private access.
- Nothing on the IPO critical path. No JV-site pilot before a 100%-owned site.
- No full digital twin before M6. No optimiser that is "merely different" (R5).
- No hidden aggregate "ESG score" (R5 #45). No hindsight claims ("we would have saved Pascua-Lama").
- No forks of sibling repos (E002). No executive deliverable blocked on frankensim or FA (§5.4).

---

## 9. Recommended MOR_CASE updates and open questions

**Proposed evidence rows.** For the lead to adopt; this pass did not edit MOR_CASE.
- **E024+:**
  - The Avathon release is dated 23 Sep 2026 and its scope includes "production and recovery … mine to mill"
    (W1).
  - Avathon was reported live at NGM from Q2 2026, with PV next, on a 5-year term with outcome fees (W2, V-S).
  - The Draslovka alliance gives Avathon a leach-optimisation channel (W3).
  - The NA team includes Hamman (CFO) and Cribb (COO). No public F-1 was found (W5).
- **E034 (new):** Newmont's 3 Feb 2026 NGM default notice, settled 10 Aug 2026. NGM operating data is
  JV-sensitive (W4).
- **E035 (new):** the industry's believed proofs are single-asset operational uplifts (Freeport TROI, W8).
  Twins stall on people (Lihir, W9).
- **E031+:** JORC 2026 is still not in force. The NI 43-101 proposal adds QP data-verification disclosure per
  item (W7).

**Open questions [OPEN].**
1. p(site data by month 7). This is the dominant uncertainty in §5.3. The sponsor should be asked directly.
2. Do Barrick's MD&As carry per-site components sufficient for the M0 reproduction? Verify in week 1.
3. Who sponsors: the CFO (assurance), the Head of MRM (reconciliation) or the CEO (capital)? Which perimeter
   (NA or parent) does the sponsor sit in after the IPO?
4. Is `omf-rust` (MIT) admissible under the closed dependency universe, or is it a lab oracle only? (E027, E015)
5. What retention and privilege design for DecisionCards and probabilistic records does counsel accept (F4)?
6. Can the owner's relationship with Barrick be formalised (NDA, insider status, contracting partner) before
   M2 without triggering F3 or F15?
