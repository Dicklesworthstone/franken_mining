# WIZARD_SCORES_AGY_ON_CC: Gemini (AGY) Evaluation & Scoring of Claude's (CC) 15 Ideas

| Field | Value |
|---|---|
| Evaluator | Gemini (AGY), Dueling Idea Wizards |
| Target File | `planning/duel/WIZARD_IDEAS_CC.md` (Ideas 1–15) |
| Date | 2026-10-29 |
| Audit Context Weighed | `planning/mor/r7/MODES_OF_REASONING_REPORT_AND_ANALYSIS_OF_PROJECT.md` (`C001–C027`, `C050–C066`, `C080–C161`, `R001–R009`) |
| Scale | 0 (worst) to 1000 (best) |

---

## Executive Observations Across CC's 15 Ideas

1. **Credit for diagnosing the M0 stand-in defect, but no free pass on M0 preconditions.** CC (`§0`) rightly notes that `fmine assure public --issuer barrick` currently reads an in-code stand-in workbook (`crates/fmine-io/src/zip_inflate.rs:408`) and hard-codes/plugs the gold AISC bridge (`crates/fmine-public/src/lib.rs:343–369`; `R7 C001–C006`, `C058–C059`). In scoring below, no credit is given for "M0 readiness" where a first slice still depends on unbuilt label-anchored extraction (`R7 R003`), unparsed `xl/styles.xml` number formats, or heavy manual two-reader PDF transcription (`R7 C114`).
2. **High internal overlap around two kernels.** Almost the entire 15-idea list is built by applying two kernels to different public tables:
   - **Cluster A (Interval/Farkas table consistency):** Idea 2 (`Proof-carrying discrepancies`) is the core solver, and Ideas 6 (`JV Mirror`), 8 (`Fiscal mirror`), 11 (`Conversion bounds`), and 12 (`Comparability engine`) are domain projections of that same solver.
   - **Cluster B (Guidance scoring & lifecycle):** Idea 1 (`Public Track Record`) and Idea 9 (`Guidance lifecycle`) split a single workflow—and even duplicate the exact same quarterly "guidance-at-risk" YTD nowcast on North Mara (`E587`)—across two separate slots, while Idea 15 (`Disclosed-disruption library`) exists primarily to feed Ideas 1, 5, and 9.
3. **Portfolio blind spot: 100% financial/disclosure tables, 0% physical mining & engineering depth.** Every one of CC's 15 ideas lives in the top-level financial/reporting/table layer (`Portal`, `Bullion`, `Plumb`, `Helm`, `Hallmark` stock tables). Zero ideas advance subsurface structural/geostatistical modeling (`Strata`), geotechnical slope/stope stability (`Highwall`), 3D mine planning and pit/stope scheduling (`Pushback`), processing circuit/smelter physics (`Crucible` beyond a 4-parameter synthetic regression in Idea 5), drillhole trajectory control (`Corebox`), or tailings/water/closure critical paths (`Freeboard`).

---

## Detailed Evaluations (Ideas 1–15)

### Idea 1 (CC-01): Public Track Record
- **Score:** **865 / 1000**
- **Evaluation:**
  - **Strongest point:** Treating the issuer's historical per-site guidance ranges (`AIF p.31`, `E586`, `E587`) and technical-report LOM schedules (`E249`) as pre-registered forecasts anchored by their filing date (`INV-008`) is a sharp epistemic wedge that directly attacks the M0 finance-reviewer kill criterion (`BD-075`) and `OPEN-BULL-1` (separating price-driven royalty drift from controllable AISC misses, `R7 C141`) using 100% public data.
  - **Biggest weakness:** The statistical estimators promised in the M0 first slice (`FY2023–FY2025` = only 3 resolved years, with FY2023 available only via Q4 2024 comparatives `E451`) are severely under-powered: as `R7 C118` and `C120` proved, separating 80% from 60% coverage requires $n_{\text{eff}} \ge 35$, whereas 3 years dominated by a common gold-price and inflation shock yield $n_{\text{eff}} \approx 3$ and leave leave-one-year-out split conformal recalibration with only 3 exchangeable blocks (a coarse $1/3$ quantile grid that cannot certify an 80% interval). It also requires rebuilding the broken M0 workbook importer (`R7 R003`) and manually transcribing multi-year AIF/MD&A PDF tables with two human readers (`R7 C114`).
  - **What would make it better:** Fold Idea 9's pre-release cross-site coherence check directly into this verb, drop the under-powered 3-year split-conformal recalibration in favor of exact finite-sample Clopper–Pearson bounds and deterministic royalty/price-deck attribution, and extend the panel back to FY2019–FY2025 from annual reports so $n_{\text{eff}}$ is large enough to estimate site effects $b_s$.
- **Duplication & Overlap:**
  - *Plan:* Materially extends `BD-067`/`BD-068`/`BD-076` (FY2026 forecast freeze/scoring), `BD-276` (Loulo TR assumptions as frozen forecasts), `NS-04` (§1.3 calibrated guidance ranges), `OPEN-PLUMB-7` (§36.2 reserve realization), and `W4 §6.4` (single-site guidance-assurance cards).
  - *AGY list:* Strongly overlaps **AGY Idea #1 (`GuidanceCredibilityBridge`)**, which also performs price-conditional (royalty/tax) vs controllable guidance attribution and pre-release guidance certification.
  - *CC internal overlap:* Directly overlaps **CC Idea 9 (`Guidance lifecycle`)**, which uses the same guidance tables, the same $a_y + b_s$ error model, and duplicates the quarterly YTD guidance-at-risk nowcast (`North Mara` `E587`).

---

### Idea 2 (CC-02): Proof-carrying discrepancies
- **Score:** **910 / 1000**
- **Evaluation:**
  - **Strongest point:** Replacing heuristic $\pm 1$-last-digit pairwise checks (§21.6) with joint exact-rational (`ExactQty` `i128`) interval feasibility over half-unit rounding intervals $[v - \frac{1}{2}10^{-d}, v + \frac{1}{2}10^{-d}]$ and machine-checkable Farkas certificates ($y \ge 0, y^T A = 0, y^T b < 0$) is mathematically sound, directly supplies the missing `GATE-BULL-01` scorer (`BD-272`, `R7 C023/C024/C110/R004`), and can be tested immediately against ~10 real discrepancies already catalogued in the evidence register (`E113`–`E116`, `E144`, `E147`, `E153`, `E206`, `E463`).
  - **Biggest weakness:** In the real Barrick `.xlsx` workbooks, cells store unrounded IEEE-754 floats in `<v>` while display precision lives in `xl/styles.xml` (which `crates/fmine-io/src/xlsx.rs` does not parse today), and `R7 C050/C109` proved that 41–46% of numeric cells in the 10 Mine Stats workbooks sit in *no* identity—meaning interval solving alone leaves nearly half of `GATE-BULL-01`'s denominator unchecked and still requires building the label-anchored importer (`R7 R003`). Furthermore, single-box McCormick relaxations on 3-way or 4-way bilinear chains ($\text{tonnes} \times \text{grade} \times \text{recovery} = \text{metal}$) are often too loose to refute real multiplicative rounding errors without branch-and-bound domain splitting.
  - **What would make it better:** Parse `xl/styles.xml` alongside `<v>` and `<f>` so the solver verifies both *printed-precision rounding feasibility* and *exact stored-float vs formula identity*, and add depth-bounded interval bisection on grade/recovery boxes when a single McCormick envelope fails to refute a multiplicative discrepancy.
- **Duplication & Overlap:**
  - *Plan:* Materially upgrades §21.6, `BD-066`, `BD-272`, `BD-275`, `BD-279`, and `GATE-HALL-01`; adapts the `Touchstone` reconciliation concept (`Appendix C #15`, `GB-07`) from Gaussian WLS to $L_\infty$ rounding intervals.
  - *AGY list:* Complements **AGY Idea #2 (`WorkbookForensicAutopsy`)** (CC #2 solves printed value intervals; AGY #2 decompiles `<f>` formula ASTs to diagnose the spreadsheet bug that caused the discrepancy).
  - *CC internal overlap:* Serves as the shared solver kernel for **CC Ideas 6, 8, 10, 11, and 12**.

---

### Idea 3 (CC-03): Spreadsheet Lifter
- **Score:** **895 / 1000**
- **Evaluation:**
  - **Strongest point:** Lifting `.xlsx` `<f>` formula strings (already extracted and discarded at `crates/fmine-io/src/xlsx.rs:54`) into a typed `Claim` DAG with R1C1 drag-pattern override detection, dimensional propagation (`$/t` vs `$/st` `E182`–`E184`), `#REF!` reach analysis (`E466`), and hidden external-link detection (`AUDIT_4`) directly neutralizes `pre-mortem F5` ("a senior geologist's spreadsheet stayed the recon of record") and resolves `OPEN-TOUCH-1` (lifting a pilot site's reconciliation pack conventions directly from its formulas), while using Excel's cached `<v>` values as a free external parity oracle.
  - **Biggest weakness:** Inferring units and bases (wet vs dry tonnes, 100% vs attributable, short tons vs metric tonnes) from free-text spreadsheet headers via a deterministic label grammar will suffer high `unit unknown` refusal or false-mismatch rates on real mining workbooks where units sit in sheet titles, footnotes, or merged banner rows (`R7 C008`), and dynamic lookup functions (`INDEX/MATCH`, `VLOOKUP`) make static DAG extraction conditional on cell values. Its M1 first slice also depends on first fixing `fmine-public` to ingest the 18 real workbooks (`R7 R001`, `R003`).
  - **What would make it better:** Pair the R1C1 pattern-break and dimensional type-checker with a declarative TOML header-binding dialect (`BD-186`) for ambiguous row labels, and wire its output into Idea 2's Farkas solver so every detected formula override is automatically linked to the downstream table identity breaks it explains.
- **Duplication & Overlap:**
  - *Plan:* Materially extends `BD-187` (CSV/Excel readers), `G-UNIT-4`, and `BD-287` (`CorrelationCard` formula parser).
  - *AGY list:* Directly duplicates **AGY Idea #2 (`WorkbookForensicAutopsy`)**, which independently proposed lifting `.xlsx` `<f>` formula ASTs (`crates/fmine-io/src/xlsx.rs:54`), dimensional unit propagation over `registries/units.toml`, R1C1 horizontal/vertical symmetry break detection, ragged `SUM` range detection, and literal plug identification.
  - *CC internal overlap:* Overlaps **CC Idea 2** on pre-flight workbook auditing (`#REF!` `E466`, hidden external links `AUDIT_4`).

---

### Idea 4 (CC-04): Disclosure Lint
- **Score:** **810 / 1000**
- **Evaluation:**
  - **Strongest point:** *Cited mode* (`[$1,637/oz](fmine:clm_…)`) is a clean, deterministic enforcement gate between "AI agents draft" and "humans sign" (§30.4, `pre-mortem F1`) that catches real basis collisions already in the register (`$1,423/oz` scorecard vs `$1,637/oz` non-GAAP AISC `E605`/`E607`; `190%` Au vs `504%` GEO 3-year reserve replacement `E603`/`E604`) and directly prevents the exact class of mislabelled company-level figures that `R7 C002–C005 / C130 / R006` found in FrankenMining's own M0 code.
  - **Biggest weakness:** *Discovery mode* on unannotated English prose (matching bare numbers in MD&A paragraphs to site/metric/period/basis tuples without an LLM via keyword heuristics) will either drown users in `ambiguous` verdicts whenever a paragraph mentions multiple periods, sites, or bases, or suffer low recall; meanwhile, *Cited mode* cannot return honest verdicts today because `fmine claim show` currently fabricates `"evidence_class":"Exact"` for any string starting with `clm_` (`R7 C062`).
  - **What would make it better:** Scope the M1 deliverable to Cited mode plus structured Markdown table/section contexts (where headings explicitly bind entity, period, and basis), and require `fmine-claim` to verify cryptographic `AnchorHash` lineage in the authority ledger before `fmine lint` can emit `verified`.
- **Duplication & Overlap:**
  - *Plan:* Extends `G-BULL-6`, §30.5 report language rules (`G-FREE-1`, `G-HALL-1`), and `GATE-ROBOT-01` (§30.4) from FrankenMining's internal report renderer to arbitrary external/agent drafts.
  - *AGY list:* Directly duplicates the second half of **AGY Idea #5 (`AgentSandboxAndProseVerifier` / `fmine verify-prose`)**, which also enforces inline `[clm_...]` citation checking, basis/unit/rounding verification, interval/UU checks, and `INV-002`/`G-FREE-1` forbidden-phrase linting on agent-authored Markdown memos.
  - *CC internal overlap:* Overlaps **CC Idea 12** (checking comparative prose claims in IR decks) and **CC Idea 13** (checking agent free-text responses).

---

### Idea 5 (CC-05): Pilot Protocol Card
- **Score:** **765 / 1000**
- **Evaluation:**
  - **Strongest point:** It targets the single tightest bottleneck on the 29-bead critical path—`P(site data by month 7) ≈ 0.30` (`E599`, `pre-mortem F2`, `BET-06`)—by handing the executive sponsor, IT security, and counsel a pre-registered analysis plan and a minimum-viable data request where every file in `GATE-DATA-03` is justified by its marginal statistical power.
  - **Biggest weakness:** Calibrating a plant twin to only ~10 public quarterly aggregate points (`Lumwana` `E566`–`E573`, `E580`–`E584`) leaves intra-quarter autocorrelation, sampler error covariance, and transitional-ore kinetics completely unidentifiable from public data; thus, a point claim like "historian data add less than 2 points of power" is an artifact of the author's hardcoded prior variances in `fmine-gen` rather than an empirical fact about Lumwana, risking false precision (`INV-006`). In addition, `R7 C084` and `C138` showed that the plan's `92.7%` Lumwana benchmark is only the Super Pit fresh-ore design recovery and that `GATE-DATA-03`'s table definition currently conflicts with §37.3's recovery-capable subset.
  - **What would make it better:** Replace single point-power claims with a **prior-sensitivity power envelope** across plausible ranges of private sampler/assay noise ($\sigma_{\text{sampler}} / \sigma_{\text{process}}$), and explicitly support both the recovery-led and grade-led `GATE-DATA-03` thresholds identified in `R7 C102`.
- **Duplication & Overlap:**
  - *Plan:* Heavily overlaps §20.6 (detection power per measurement node), `BD-159` (power for a 5% sampler bias), `GATE-TOUCH-04` (pre-registered materiality threshold), `GATE-DATA-03`, `GB-25` (linear-Gaussian observability matrix), and §37.3 (M2 pilot design), repackaging them into a pre-data card.
  - *AGY list:* No direct duplicate in AGY's top 15.
  - *CC internal overlap:* Relies on **CC Idea 15 (`Disclosed-disruption library`)** for the Lumwana outage segments.

---

### Idea 6 (CC-06): JV Mirror
- **Score:** **850 / 1000**
- **Evaluation:**
  - **Strongest point:** Reconciling one physical JV mine (`NGM` 61.5% Barrick / 38.5% Newmont `E457`; `Pueblo Viejo` 60/40 `E672`; `Kibali` 45/45/10 `E560`) across two independently audited public issuers is a high-leverage, zero-site-data natural experiment that directly serves a live board dispute (`Newmont` Feb 2026 default notice and Aug 2026 settlement `E557`, `E558`) while empirically isolating definitional bridges (by-product vs co-product credits, G&A allocation) on identical underlying ounces.
  - **Biggest weakness:** None of the partner filings (Newmont 10-K/10-Q segment tables, AngloGold Ashanti 20-F/6-K tables) are in the corpus or registries today (marked `FACT[U]` by CC), and partners report under different accounting regimes (US GAAP vs IFRS) with undisclosed partner-level purchase-price allocation (PPA) amortization, intercompany management fees, and inventory timing layers—so most unit-cost lines will hit `Refused` (unidentifiable bridge parameter) rather than producing a clean conflict certificate, and the Barrick side cannot run until `R7 R003` fixes the M0 importer.
  - **What would make it better:** Restrict the M0–M1 mirror to metrics invariant to partner-level historical PPA (contained metal produced, ounces sold, sustaining vs project capex, and reserve tonnes/grade at stated price decks) and ingest Newmont's SEC EDGAR XBRL segment facts directly so partner extraction is deterministic and automated.
- **Duplication & Overlap:**
  - *Plan:* Extends §21.6 (`W5 #36` Newmont vs Barrick definition flags), `BD-314` (`CapitalEntityGraph`), and Appendix I.6 (`Porgera` 47.5% vs 100% discrepancy `E468`–`E469`).
  - *AGY list:* Overlaps **AGY Appendix Idea #28 (`JVPartnerReconciliationMirror`)**.
  - *CC internal overlap:* Directly reuses **CC Idea 2's** solver kernel, shares the multi-reporter `BridgeLayer` design with **CC Idea 8 (`Fiscal mirror`)**, and supplies the bridges for **CC Idea 12 (`Comparability engine`)**.

---

### Idea 7 (CC-07): Intertemporal-shift ledger
- **Score:** **825 / 1000**
- **Evaluation:**
  - **Strongest point:** It gives the board remuneration committee (`RISK-22` owner, §15.6, §23.2, §23.6, `R7 C139/C140`) a concrete instrument to see how much of a site's AISC beat came from below-baseline sustaining capex, below-plan waste stripping, grade draw, or capex reclassification, while properly handling the WGC vs Technical Report definitional trap (`Carlin` `E196`/`E197`: $67/oz "sustaining" vs $188/oz including capital stripping and underground development) and turning "was it deferral or efficiency?" into a registered, scored reversal forecast.
  - **Biggest weakness:** In real open-pit and underground operations, multi-quarter deviations of strip ratio and head grade from a LOM average are the normal physical consequence of pushback geometry, stockpile drawdowns, and cutoff-grade adjustments to spot gold prices—not an "intertemporal debt" that reverses on an 8-quarter schedule; and with only ~8–10 quarters of per-site AISC data (`Q3 2024` onward, `W1 §12`), a hierarchical model across sites has far too small a sample (`R7 C118`) to estimate a counterfactual sustaining-capex baseline $b_t$ with tight 90% intervals.
  - **What would make it better:** Require an explicit **year-by-year LOM schedule** from a filed Technical Report for all four mechanisms (refusing whenever only a LOM average is published, rather than substituting a noisy trailing/hierarchical mean), and adjust the expected head-grade baseline for spot-price cutoff changes before computing grade draw $r_t$.
- **Duplication & Overlap:**
  - *Plan:* Extends `GB-26` (which already includes a sustaining-deferral bar in the AISC waterfall), §15.6, §23.2, §23.6, and `RISK-22`.
  - *AGY list:* Partially overlaps **AGY Idea #1 (`GuidanceCredibilityBridge`)** on AISC beat/miss decomposition, though CC's cumulative debt and reversal-forecast framing for the remuneration committee is distinct.
  - *CC internal overlap:* Reuses **CC Idea 6's** `Definition` bridge layer and `BD-064`'s LMDI waterfall.

---

### Idea 8 (CC-08): Fiscal mirror
- **Score:** **750 / 1000**
- **Evaluation:**
  - **Strongest point:** Host-state fiscal disputes are an acute, realized risk for this portfolio (`RISK-26`: Loulo-Gounkoto suspension and 2023 Code settlement `E410`–`E413`, Tanzania Twiga 50/50 true-up `E448`, Kibali super-profits tax `E242`), and checking `BD-296`'s registered price-contingent royalty schedules against Mine Stats royalties/oz (`E110`) and statutory payment filings provides an empirical validation test for `fmine-econ`.
  - **Biggest weakness:** Two of its four legs (Canadian ESTMA reports and host-government EITI reports) are unverified `FACT[U]` sources absent from the repository corpus, EITI reports lag by 2–4 years, and ESTMA reports cash payments on a consolidated controlled-entity basis (which omits equity-accounted JVs like `Kibali` §23.2 and aggregates corporate income tax, withholding tax, customs duties, and royalties across licences)—so the 4-way mirror largely degenerates into a 2-way comparison (Mine Stats royalty accrual vs statutory schedule `BD-296`) confounded by accrual-vs-cash timing lags.
  - **What would make it better:** Scope the M1 deliverable to the clean 2-way reconciliation between Mine Stats quarterly royalties/oz + financial-statement tax notes and `BD-296`'s price-contingent statutory schedules (`E410`–`E449`), adding ESTMA cash reconciliations only after the ESTMA filings are captured and entity-mapped in `registries/`.
- **Duplication & Overlap:**
  - *Plan:* Extends `BD-228`, `BD-296` (`CountryRegime` rows), §23.4, and `W2` (`E401`–`E449`).
  - *AGY list:* Overlaps **AGY Idea #1** (price-step royalty attribution) and **AGY Idea #7 (`StreamingRoyaltyTheoremProver`)** on statutory/contractual royalty schedules.
  - *CC internal overlap:* Reuses **CC Idea 2's** solver and **CC Idea 6's** `MirrorPair`/`BridgeLayer` architecture, and overlaps **CC Idea 1's** price-conditional royalty regression.

---

### Idea 9 (CC-09): Guidance lifecycle
- **Score:** **740 / 1000**
- **Evaluation:**
  - **Strongest point:** The pre-release implied-correlation diagnostic $\bar{\rho} = (h_T^2 - \sum h_s^2)/((\sum h_s)^2 - \sum h_s^2)$ and the implied corporate AISC layer check (comparing consolidated AISC guidance minus ounce-weighted site midpoints against the historical `+$23.57` to `+$88.66/oz` range `E496`, which directly catches the `R7 C003` negative corporate-adjustment bug) are fast, transparent checks for a CFO and disclosure committee before guidance is published.
  - **Biggest weakness:** This is **CC Idea 1's** second half split into a separate numbered idea: Idea 1 already includes the quarterly "Guidance-at-risk" nowcast $P(\text{FY} \in [L, U] \mid \text{YTD})$ on the exact same North Mara `E587` example, and adding a universal-inference split-likelihood-ratio e-process over only 3 quarterly public looks with an $n \approx 3$-year empirical ratio distribution cannot overcome the small-sample power limits identified in `R7 C118/C120`. Moreover, the equicorrelation formula $\bar{\rho}$ assumes linear variance additivity, which breaks for ratio metrics like AISC ($C/\text{oz}_{\text{sold}}$) when site ounce weights vary.
  - **What would make it better:** Merge this entirely into **Idea 1 (`Public Track Record`)** as its pre-publication check and intra-year tracker, and replace the small-sample universal-inference LRT with deterministic physical capacity bounds (checking whether required H2 tonnes $\times$ grade $\times$ recovery exceed historical site p95 throughput and reserve grade).
- **Duplication & Overlap:**
  - *Plan:* Extends `NS-04` and §10.4 (`BettingEProcess` monitors).
  - *AGY list:* Directly duplicates the first half of **AGY Idea #1 (`GuidanceCredibilityBridge`)**, which also audits pre-release site vs consolidated guidance range coherence and implied corporate AISC layers (`E496`).
  - *CC internal overlap:* Heavily overlaps **CC Idea 1 (`Public Track Record`)** (shares the same guidance data, the same $a_y + b_s$ error model, and duplicates the YTD guidance-at-risk calculation).

---

### Idea 10 (CC-10): Target Disclosure Auditor
- **Score:** **785 / 1000**
- **Evaluation:**
  - **Strongest point:** Exact rational remainder-grade arithmetic ($g_r = (L g - l g_i)/(L - l)$) on junior "including" press-release headlines, cut-off fixed-point verification (`E182`), and developer PEA/FS cash-flow table replay (automatically discovering mid-year vs end-year discounting conventions) give BD and exploration teams a fast, practical screening card before opening a data room—and CC makes a sharp catch that `brent_root` in `crates/fmine-numerics/src/lib.rs:149` currently returns `Some` without converging.
  - **Biggest weakness:** It bundles three disparate document parsers (unstructured junior drill-intercept press releases, resource tables, and developer DCF tables) whose real-world inputs are messy PDFs not present in the repository corpus (`week-1 data gate`), so in practice it falls back to manual "cited-mode" human entry (`what could kill it` row 1), and 1D downhole length ratios cannot verify true thickness without 3D collar/survey geometry and wireframe orientation.
  - **What would make it better:** Restrict automated ingestion to structured HTML/XLSX tables from SEDAR+/EDGAR filings (`extract_html_tables`) and integrate 3D collar/azimuth/dip intersection angle checks (`Corebox` `fmine-drill`) whenever collar tables accompany the release so true-width dilution is geometrically verified.
- **Duplication & Overlap:**
  - *Plan:* Extends §23.2 ("Junior equity stake" and "M&A"), §23.5 (`GB-31`, `GB-32`), and `BD-274` (published cut-off reproduction `E182`).
  - *AGY list:* No direct duplicate in AGY's top 15.
  - *CC internal overlap:* Part 2 reuses **CC Idea 2** and `BD-274`; Part 4 depends on **CC Idea 11 (`Conversion bounds`)**.

---

### Idea 11 (CC-11): Conversion bounds
- **Score:** **775 / 1000**
- **Evaluation:**
  - **Strongest point:** Recognizing that annual reserve/resource statements publish only *stocks* ($I_t, M_t, P_t$) while inter-category conversion *flows* ($f_{\text{IM}}, f_{\text{MP}}$) are only partially identified—and solving a Charnes–Cooper linear-fractional program over Idea 2's rounding intervals under an explicit assumption ladder (`A0`–`A4`)—is an epistemically honest way to construct `OPEN-HELM-1`'s conversion priors for `GB-31` and §23.5.
  - **Biggest weakness:** Without assumption `A1` ("all discovery additions enter only as Inferred", i.e. $a_M = 0$), the assumption-free bound `A0` on $c_{\text{FI}}$ is nearly vacuous (`[0, 1]`), yet `A1` is routinely violated in real mining disclosures whenever step-out infill drilling adds Indicated ounces directly or when rising reserve gold price decks (`E143`: $1,300 → $1,400 → $1,500/oz) pull unmined Indicated material into the resource shell—causing `A1`–`A4` to systematically overstate true geological conversion.
  - **What would make it better:** Constrain the stock-flow LP using the explicit sub-columns of published reserve roll-forward tables (which separate *depletion*, *price-deck changes*, *cost/model changes*, and *drilling additions*, as checked by `GATE-HALL-01` and `E144`–`E147`) so price-deck additions are stripped out before bounding $c_{\text{FI}}$ and $c_{\text{IR}}$.
- **Duplication & Overlap:**
  - *Plan:* Directly addresses `OPEN-HELM-1` (§23.5, §36.2, `GB-31`).
  - *AGY list:* No duplicate in AGY's top 15.
  - *CC internal overlap:* Reuses **CC Idea 2's** LP/interval kernel and feeds **CC Idea 10 (`Target Disclosure Auditor`)**.

---

### Idea 12 (CC-12): Comparability engine
- **Score:** **705 / 1000**
- **Evaluation:**
  - **Strongest point:** Computing exact, distribution-free rank intervals $[1 + \#\{i: \text{hi}_i < \text{lo}_j\}, n - \#\{i: \text{lo}_i > \text{hi}_j\}]$ across peer issuers and refusing comparative claims ("lowest-cost quartile") whenever §21.6 WGC definition flags differ without a bridge is a clean, honest tool for IR and remuneration committees.
  - **Biggest weakness:** Transferring an empirical $/oz definitional bridge calibrated on a single JV mirror asset (Idea 6, e.g. gold-dominant `NGM`) as a `Transferred` layer (`INV-006`) to other issuers or portfolios is economically unsound because the by-product vs co-product AISC spread is a deterministic mechanical function of each asset's copper/silver revenue share (`W5 #36`), not a transferable statistical constant; furthermore, none of the peer issuers' filings (Agnico Eagle, Kinross, Newmont consolidated) are in the corpus today.
  - **What would make it better:** Replace statistically "transferred" $/oz bridges from Idea 6 with **exact algebraic restatement formulas** using each peer's published segment revenue by metal, ounces sold vs produced, and sustaining capex lines (refusing only when an issuer omits segment metal revenue splits).
- **Duplication & Overlap:**
  - *Plan:* Minor extension of §21.6 (`AISC` definition flags and `W5 #36` Newmont co-product/by-product comparison) and `G-BULL-6`.
  - *AGY list:* No direct duplicate in AGY's top 15.
  - *CC internal overlap:* Heavy overlap with **CC Idea 6 (`JV Mirror`)** (same `Definition` bridges), **CC Idea 2** (interval comparison), and **CC Idea 4 (`Disclosure Lint`)** (checking comparative prose claims in IR decks).

---

### Idea 13 (CC-13): Sealed-answer agent benchmark
- **Score:** **660 / 1000**
- **Evaluation:**
  - **Strongest point:** Pricing abstention via a proper scoring rule ($+1$ for a correct cited answer, $0$ for a typed refusal, $-k$ for a wrong or uncited number, setting the optimal threshold at $P(\text{correct}) > k/(1+k)$) and hashing task recipes before future quarterly releases (`Q3 2026` on 9 Nov 2026 `E454`) is a neat application of `INV-008` to contamination-free AI evaluation.
  - **Biggest weakness:** Under AGENTS.md **RULE 0.1 ("Process is never the product... Do not build a validator for a validator or a harness for a harness")**, building an external AI-procurement benchmark product (`fmine bench`) at M0–M1 is a distraction when FrankenMining's *own* M0 binary currently fails to read the real Barrick workbooks and emits fabricated numbers (`R7 C001–C006`, `C058`, `C149`); moreover, with `Q3 2026` arriving in 11 days (`9 Nov 2026`, `R7 C025`) and no label-anchored importer yet built (`R7 C008`), pre-hashed answer recipes will break if a single spreadsheet row shifts.
  - **What would make it better:** Drop the standalone `fmine bench` verb and external procurement framing, and instead add the $+1 / 0 / -k$ abstention-scoring rule as a 50-line scorer inside `GATE-ROBOT-01` (§30.4, `BD-120`) once the M0 importer and `fmine-claim` store actually work on real files.
- **Duplication & Overlap:**
  - *Plan:* Largely duplicates and slightly extends `GATE-ROBOT-01` (§30.4, 20 scripted agent tasks) and `BD-068` (pre-release hashing).
  - *AGY list:* No duplicate in AGY's top 15.
  - *CC internal overlap:* Reuses **CC Idea 2's** discrepancy fixtures (`E113`–`E116`) and **CC Idea 4 (`Disclosure Lint`)**.

---

### Idea 14 (CC-14): Board delta pack
- **Score:** **720 / 1000**
- **Evaluation:**
  - **Strongest point:** Answering the board's opening question ("what changed since the last pack and why?") with an exact, order-independent Shapley attribution ($\sum \phi_i = v(F) - v(\emptyset)$) across provenance factors (new evidence, recipe version, `rules_version`, price deck `E143`, perimeter `E552`–`E558`, restatements `E474`, and error corrections `INV-003`) eliminates arbitrary bridge ordering and residual plugs.
  - **Biggest weakness:** Evaluating a recipe $v(S)$ on all $2^k$ mixed counterfactual corners of the hypercube (e.g., running the *new* recipe version with the *old* schema inputs, or evaluating the *old* model on the *new* perimeter after `Fourmile` `E558` was vended in without old inputs) will routinely hit schema, domain, or missing-data refusals on incompatible off-diagonal vertices $S$, causing the entire Shapley sum to refuse; and in the M1 public lane, FrankenMining has no executable recipe that computes reserve ounces from drilling (`R7 C005`).
  - **What would make it better:** Replace unconstrained $2^k$ subset evaluation with a **precedence-constrained Shapley (Myerson/Owen) decomposition** along valid migration paths (aligning perimeter and restatements before crossing schema/recipe versions), and tie reserve/resource deltas to 3D block-model diffs (`Strata`/`Hallmark`) rather than scalar table literals.
- **Duplication & Overlap:**
  - *Plan:* Extends `GB-26` / `BD-064` (LMDI waterfall), `INV-003` (supersession), `INV-016` (`as_of_decision`), and applies the exact Shapley machinery already planned in `§13.5` (Highwall geotechnical FoS driver Shapley) to provenance factors.
  - *AGY list:* Partially overlaps **AGY Idea #4 (`RpeeeAndBlockModelForensics`)** (which runs exact 4-factor spatial Shapley attribution on 3D block models where every corner is well-defined on `VdbGrid`).
  - *CC internal overlap:* Overlaps **CC Idea 1** (vintage diffs), **CC Idea 4** (narrative linting), and **CC Idea 12** (non-GAAP reconciliation line diffs).

---

### Idea 15 (CC-15): Disclosed-disruption library
- **Score:** **620 / 1000**
- **Evaluation:**
  - **Strongest point:** Curating a cited, two-reader-verified catalogue of operational disruptions from the issuer's quarterly MD&As (`Lumwana` `E580`–`E584`) gives the M1 synthetic twin and guidance-at-risk calculators an empirical starting point without scraping external news (`INV-010`).
  - **Biggest weakness:** MD&A prose suffers from severe **truncation and selection bias** (management highlights operational disruptions mainly in quarters that miss guidance, omitting routine outages in quarters that beat), and inferring an unquantified outage's metal impact from the quarterly LMDI throughput/recovery bar (`BD-064`) confounds a 3-day plant trip with 87 days of ore-hardness and head-grade variation; combined with 8 cause classes across only ~14 quarters and heavy manual two-reader text coding (`R7 C114`), the pooled hazard intervals will be wide and systematically biased.
  - **What would make it better:** Demote this from a standalone product feature/verb (`fmine disruptions`) into a small static fixture (`registries/disclosed_disruptions.toml`) restricted to **explicitly quantified** issuer-disclosed events (where duration and lost tonnes/ounces are stated in the filing), modeled with a left-truncated Poisson likelihood to account for reporting thresholds.
- **Duplication & Overlap:**
  - *Plan:* Minor extension of `W2` (`E401`–`E449` host-state event registry) and `E580`–`E584` (Lumwana outage rows already in `planning/mor/MOR_CASE.md`).
  - *AGY list:* No duplicate in AGY's top 15.
  - *CC internal overlap:* Explicitly described by CC (`§18`) as a supporting input to **CC Ideas 1, 5, and 9**.

---

## Ranked Summary Table of CC's 15 Ideas by AGY Score

| Rank | CC ID | Idea Title | AGY Score (0–1000) | Primary Subsystems | Key Strength | Key Weakness / Overlap |
|---:|---|---|---:|---|---|---|
| 1 | **CC-02** | **Proof-carrying discrepancies** | **910** | Portal, Bullion, Hallmark, Touchstone | Exact rational (`ExactQty`) interval feasibility + Farkas certificates directly solve `BD-272` (`GATE-BULL-01` scorer) on `E113–E463` | 41–46% of Mine Stats cells sit in no identity (`R7 C050`); needs `xl/styles.xml` parser, real-workbook importer (`R7 R003`), and bilinear branching |
| 2 | **CC-03** | **Spreadsheet Lifter** | **895** | Portal, Plumb, Bullion, Touchstone, Helm | Lifts `<f>` formulas (`xlsx.rs:54`) into typed `Claim` DAGs with R1C1 override & unit checks; free Excel `<v>` oracle; solves `F5` & `OPEN-TOUCH-1` | Free-text header unit grammar is brittle on real sheets; duplicates **AGY Idea #2 (`WorkbookForensicAutopsy`)** |
| 3 | **CC-01** | **Public Track Record** | **865** | Plumb, Portal, Bullion, Helm | Scores historical issuer guidance (`E586–E587`) & TR schedules as filing-dated (`INV-008`) forecasts with price-conditional royalty splits | 3-year M0 slice ($n_{\text{eff}} \approx 3$) cannot support split-conformal recalibration or site random effects (`R7 C118/C120`); overlaps **AGY #1** & **CC-09** |
| 4 | **CC-06** | **JV Mirror** | **850** | Portal, Bullion, Hallmark, Touchstone | Dual-reporter natural experiment (`NGM` `E457`/`E557`, `PV` `E672`, `Kibali` `E560`) using independent audited filings | Partner filings (`FACT[U]`) not in corpus; GAAP/IFRS PPA & inventory layers force `Refused` on most cost lines; reuses **CC-02** kernel |
| 5 | **CC-07** | **Intertemporal-shift ledger** | **825** | Bullion, Plumb, Pushback, Helm | Operationalizes `RISK-22` for the remuneration committee; nails WGC vs TR sustaining-capex bridge (`Carlin` `E196/E197`); scored reversal forecasts | Pushback phasing & price-driven cutoff changes mimic "deferral"; 8–10 quarters too short for hierarchical baselines (`R7 C118`) |
| 6 | **CC-04** | **Disclosure Lint** | **810** | Adit, Plumb, Bullion, Portal | Cited mode (`[val](fmine:clm_…)`) is a deterministic gate for agent/human drafts catching `E603–E607` basis collisions (`R7 R006`) | Unannotated prose discovery mode is ambiguity-prone; `fmine claim show` is currently faked (`R7 C062`); duplicates **AGY Idea #5 (`fmine verify-prose`)** |
| 7 | **CC-10** | **Target Disclosure Auditor** | **785** | Lodestar, Hallmark, Helm, Portal | Exact rational "including" remainder-grade math, cut-off fixed point (`E182`), and developer DCF replay; catches `brent_root` bug (`lib.rs:149`) | Junior press releases & developer PDFs are unstructured (`week-1 data gate` → manual entry); 1D lengths lack 3D true-width geometry |
| 8 | **CC-11** | **Conversion bounds** | **775** | Hallmark, Helm, Lodestar, Plumb | Econometrically honest Charnes–Cooper LP partial-identification bounds for `OPEN-HELM-1` (`GB-31`) from stock tables | `A0` is vacuous (`[0,1]`) and `A1` ($a_M=0$) fails whenever price decks rise (`E143`) unless roll-forward sub-columns are constrained |
| 9 | **CC-05** | **Pilot Protocol Card** | **765** | Testpit, Touchstone, Crucible, Helm, Plumb | Directly targets `P(site data) ≈ 0.30` (`E599`, `F2`) with a marginal-power-justified minimal file list for IT/counsel | ~10 public quarterly points cannot identify intra-quarter noise covariances, so point power claims reflect generator priors; overlaps `BD-159`/`GB-25` |
| 10 | **CC-08** | **Fiscal mirror** | **750** | Bullion, Helm, Portal | Targets live host-state fiscal risk (`RISK-26`, `Loulo` `E410–E413`) and validates `BD-296` statutory rows against actuals | ESTMA & EITI legs are `FACT[U]`, lag by years, and mismatch entity perimeters (`Kibali` equity accounting); reuses **CC-02**/**CC-06** |
| 11 | **CC-09** | **Guidance lifecycle** | **740** | Plumb, Helm, Bullion | Fast pre-release implied-correlation ($\bar{\rho}$) and implied corporate AISC layer (`E496`, catching `R7 C003`) diagnostics | Splits one workflow with **CC-01** (duplicating the North Mara `E587` YTD nowcast); 3 quarterly looks have low power (`R7 C118`); overlaps **AGY #1** |
| 12 | **CC-14** | **Board delta pack** | **720** | Plumb, Helm, Hallmark, Adit | Order-independent exact Shapley attribution of pack-to-pack changes across provenance factors (`INV-003`, `E143`, `E474`, `E558`) | Unconstrained $2^k$ mixed-state evaluations (`old` perimeter $\times$ `new` schema) refuse on off-diagonal corners; lacks physical models in M1 |
| 13 | **CC-12** | **Comparability engine** | **705** | Bullion, Portal, Adit | Assumption-free rank intervals and comparative-claim refusal when §21.6 WGC flags differ | By-product vs co-product $/oz bridges do not transfer across portfolios with different Cu/Ag mixes; thin wrapper over §21.6, **CC-04**, and **CC-06** |
| 14 | **CC-13** | **Sealed-answer agent benchmark** | **660** | Testpit, Adit, Portal | Proper abstention-pricing rule ($+1 / 0 / -k$) and future-release time-sealed tasks (`INV-008`) | Violates RULE 0.1 at M0 ("harness for a harness" while M0 extraction is broken `R7 C001/C149`); mostly duplicates `GATE-ROBOT-01` (§30.4) |
| 15 | **CC-15** | **Disclosed-disruption library** | **620** | Headframe, Crucible, Plumb, Helm | Cited, two-reader-verified catalogue of disclosed outages (`Lumwana` `E580–E584`) feeding M1 twins | Severe MD&A reporting/selection bias, confounded LMDI attribution for unquantified events, and high manual reading burden; helper fixture for **CC-01/05/09** |
