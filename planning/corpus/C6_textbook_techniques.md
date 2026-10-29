# C6: Mining-geology and engineering-geology textbooks (techniques, corrections, fixtures)

Agent C6 · 2026-10-28 · Evidence block E351–E399 (all 49 ids used) · Read-only pass: the plan, MOR_CASE.md and
the corpus were not edited.

**Citation convention.** Every cite is a **PDF page number of the corpus file** (`pdftotext -f N -l N`), not a
printed page. Abzalov's printed page numbers are lower by an offset that varies through the book. Waltham's printed
page = PDF page − 3.

Abbreviations:
- **AMG** = `Applied-Mining-Geology.pdf`
- **FEG** = `foundations-of-engineering-geology.pdf`
- **LAC** = `Introduction-to-Geology-and-Hard-Rock-Mining-2015.pdf`
- **LN** = `lecturenote_17136599874_5836676541685171723.pdf`

---

## 1. RESULT_STATUS: MATERIAL_INCREMENT

The four textbooks contain **no Barrick demonstrator data**. They do change the plan in three ways:
- **Three corrections:**
  - the provenance and meaning of the LUC "slope ≈ 0.1" stress case;
  - the GB-11 "slope ≥ 0.95" flip;
  - the plan's CRM ±2/±3 SD rule, which does not name its SD basis.
- **Additions with owners and known-answer fixtures:**
  - Gy calibration;
  - duplicate statistics;
  - pattern and tampering monitors;
  - twin holes;
  - core-recovery and survey checks;
  - geometallurgical sample coverage;
  - Highwall card rules.

---

## 2. Inventory

| Doc | Identity (verified) | Content by PDF page | Extraction usability | Notes |
|---|---|---|---|---|
| AMG (443 pp) | Marat Abzalov, *Applied Mining Geology*, Springer International Publishing Switzerland 2016, Modern Approaches in Solid Earth Sciences vol. 12, ISBN 978-3-319-39263-9, DOI 10.1007/978-3-319-39264-6 (PDF 4–5; chapter footer e.g. PDF 351) | Ch. 4 drilling and sampling (core recovery PDF 65–68; blastholes 79–83); Ch. 6 rock-mass classification (104–110); Ch. 7 density (112–126); Ch. 8 collar and downhole survey (128–131); Ch. 9 sampling theory (133–146); Ch. 10 QA/QC (147–171); Ch. 11 twin holes (173–182); Ch. 15 EDA (≈208–230); Ch. 19 kriging diagnostics (≈278–292); Ch. 22 change of support, UC and LUC (≈300–312); Ch. 23 model validation (≈313–318); Ch. 24 reconciliation with new data (319–326); Ch. 25 conditional simulation (331–335); Ch. 27–28 classification (351–362); Ch. 29 geometallurgy (365–372); Ch. 30 nugget split (373–375); Ch. 31 gold (381–388) | Good born-digital text. Equations extract as garbled glyphs (σ, ≤ and Greek letters are lost) and were read in context. Tables are good with `-layout`. The markdown mirror is fine for navigation. | **Rights risk:** every page is watermarked `https://telegram.me/Geologybooks`, so this is probably an unauthorised copy. Use it as a reference only. Do not ship its tables, figures or exercise files in the demonstrator; cite them and re-derive. |
| FEG (96 pp) | Tony Waltham, *Foundations of Engineering Geology*, Second Edition, Spon Press (Taylor & Francis), London and New York (title page, PDF 2, from the image). **The year is not printed on any scanned page viewed** (probably 2002; UNVERIFIED). | Two-page topic sections. Groundwater and permeability (PDF 39–40); geophysics (47); rock strength (51–52); rock-mass strength, RMR, Q and the class table (53); slope failure, the planar-slide equation and Vaiont (67–70); stabilisation (75–76); excavation (79); Q appendix (85) | **No text layer: the PDF is scanned images.** The markdown mirror is OCR that interleaves the two columns and has glyph errors. **Every FEG value in this report was read from page images rendered at 110 dpi.** | Commercial textbook scan; reference only. |
| LAC (147 pp) | Willard C. Lacy, *An Introduction to Geology and Hard Rock Mining*, Rocky Mountain Mineral Law Foundation, Science and Technology Series, reformatted edition, February 2015 (PDF 1, 4–5). It was written for mining lawyers and landmen. | Basic geology and deposit classification (PDF 12–52); exploration (53–72); evaluation, with an overview of resource-estimation methods (78–92); costs and financing (92–106); mining methods and beneficiation (107–147) | Good born-digital text. Figure pages (e.g. PDF 81–89) have no text. | Low value for the plan (introductory material). |
| LN (63 pp) | "Mining Geology" lecture note. PDF metadata: author "Ashebir", created 2015 in Word 2010. Ethiopian examples (Kenticha, Adola). | Part 1 exploration (PDF 1–22); Part 2 deposit evaluation (23–30); Part 3 mining and processing (31–55); glossary (55–63) | Good born-digital text. | Low value. One taxonomy is used (dilution sources, PDF 58–59). |

---

## 3. Demonstrator data

**There are no values usable as Barrick demonstrator data.**
- None of the four documents mentions Barrick, Newmont, Nevada Gold Mines or any Barrick-operated asset. This was checked by text search of every page; FEG could only be searched through its OCR mirror.
- AMG Table 24.1 (PDF 323) names Geita (Tanzania; not a Barrick or NGM asset) as running RC grade control at 10×5 m. That is a description of practice, not data.
- Ownership basis: not applicable.

What the books do provide is **known-answer fixtures and policy defaults** for test harnesses and registries. None of these is company data.

| ID | Fixture | Values | Unit | Basis | Doc + page | Use |
|---|---|---|---|---|---|---|
| FX-1 | Gy "30-pieces" calibration | Relative variance 9.41; mass 2889; sphere diameter 12.25 → d_N = 12.25/0.25^(1/3) = 19.45; α = 1.5 → **K = 9.41 × 2889 / 19.45^1.5 = 316.9** (arithmetic re-checked) | –; g; cm; –; g·cm^−1.5 | Textbook worked example, gold | AMG PDF 141 | §12.3 known answer |
| FX-2 | CRM accuracy (CANMET) | μ = 60.73, σ_C = 0.09 (certified within-lab), σ_L = 0.20 (certified between-lab); n = 10 results: 60.94, 60.99, 61.04, 61.06, 61.06, 61.09, 61.10, 61.14, 61.21, 61.24; m = 61.09, S_W = 0.092; \|m − μ\| = 0.36 ≤ 4·S_W = 0.368 → "accurate" | % Fe | Textbook exercise, iron-ore CRM, one batch | AMG PDF 150 | §12.4 SD-basis fixture (impact 3) |
| FX-3 | Smoothing vs slope (Table 19.4) | Data on a 40×40 m grid; exhaustive "true" values. Columns: block (m) / true SD / OK SD / corr(Z,Z*) / empirical b / theoretical slope. 1×1: 10.0 / 4.5 / 0.41 / 0.92 / 0.95. 5×5: 8.1 / 4.5 / 0.51 / 0.92 / 0.95. 10×10: 7.6 / 4.5 / 0.54 / 0.90 / 0.95. 15×15: 7.2 / 4.4 / 0.57 / 0.93 / 0.95. 20×20: 6.9 / 4.5 / 0.58 / 0.89 / 0.95. 25×25: 6.6 / 4.2 / 0.62 / 0.98 / 0.95. 30×30: 6.5 / 4.2 / 0.64 / 0.98 / 0.95. 35×35: 6.1 / 4.0 / 0.68 / 1.05 / 0.94. 40×40: 5.9 / 3.7 / 0.65 / 1.00 / 0.93. Means: true 9.7–9.8, OK 9.9–10.0. | g/t | Synthetic orogenic-gold model. The extraction drops the "×" in block sizes ("11", "55"); sizes were read from the 5 m progression. | AMG PDF 290 (text PDF 286) | §13.3 ValidationStats; GB-11 flip |
| FX-4 | Precision reference levels (Table 10.3), best / acceptable | Gold, coarse to medium: coarse rejects 20/30, pulp 10/20. Very coarse gold: coarse rejects "20 (?)"/40. Cu-Mo-Au porphyry, coarse rejects: Cu 5/10, Mo 10/15, Au 10/15; pulp: Cu 3/10, Mo 5/10, Au 5/10. Cu-Au-Fe skarn and IOCG, coarse rejects: Cu 7.5/15, Au 15/25; pulp: Cu 5/10, Au 7.5/15 | Precision error, % (the book recommends CV_AVR%, PDF 166) | Abzalov 2008 reference levels, reprinted with CIM permission | AMG PDF 167 (`-layout` verified) | `PolicyDefault` (impact 5) |
| FX-5 | QC insertion defaults | Field duplicates 5–10; pulp duplicates 3–5; CRMs 3–5; ≥ 5 of duplicates (pulp and coarse reject) to an external laboratory | % | Textbook recommendation | AMG PDF 169 | §12.1 TARGET defaults |
| FX-6 | Optimal sampling grids (Table 7.2) | Au orogenic: grade Proved 40×40, Probable 70×70; DBD Proved 250×250, Probable 450×450; DBD grade control 60×60. Cu porphyry: 40×40, 80×80; 300×300, 550×550; 40×40 | m | Geostatistical optimisation (Abzalov 2013) | AMG PDF 125 | Density coverage (impact 12) |
| FX-7 | Classification policy triples | Measured: quarterly volume ±15% at 90%. Indicated: annual ±15% (or ±30%) at 90%. Variant: ±10% at 95% (bauxite) | % / confidence | Author's proposal | AMG PDF 357 | §24.4 (impact 11) |
| FX-8 | Twin spacing vs precision | Al2O3, blasthole–RC pairs: CV_AVR 23.8% at 1 m → 35.7% at 10 m | % | Iron-ore case study | AMG PDF 175 | `TwinPair` policy (impact 7) |
| FX-9 | Vaiont back-analysis | Factor of safety by (reservoir level, rainfall): (none, low) 1.21; (none, high) 1.12; (710 m, low) 1.10; (710 m, high) 1.00; (722 m, low) 1.00. Failed at 701 m in a wet period. | – | Published back-analysis (textbook summary) | FEG PDF 70 (image) | §14.4 illustration (impact 15) |
| FX-10 | Rock-mass classes | Class I–V: RMR 80–100 / 60–80 / 40–60 / 20–40 / <20; Q >40 / 10–40 / 4–10 / 1–4 / <1; φ >45 / 35–45 / 25–35 / 15–25 / <15°; cohesion >400 / 300–400 / 200–300 / 100–200 / <100 kPa; "safe cut slope" >70 / 65 / 55 / 45 / <40° | as stated | Guideline table | FEG PDF 53 (image) | Card priors only (impact 14) |

---

## 4. Plan impacts (ranked by value)

### 1. LUC "b ≈ 0.1": wrong expansion, unpinned here, and better stated as a mechanism

**Where:** §13.1 GB-11 (plan line 1448), §15.2 (line 1942), MOR_CASE E029. **Type:** CORRECTION + VALIDATION.
**Evidence:** E352–E355.

1. **The expansion is wrong.** E029 expands LUC as "local-uncertainty-correction". LUC is *Localised Uniform Conditioning*, developed by Abzalov in 2006 (AMG PDF 307).
2. **The figure is not in the originator's book.** Abzalov's book contains no b ≈ 0.1 case. His own LUC check (Fig. 23.5, iron oxide, PDF 317–318) reports an **RMA** slope of 1 against drill-hole intersections, with r = 0.85 for LUC against 0.86 for OK. It appears to be an in-sample check against the conditioning holes (INFERENCE).
3. **The figure cannot be page-pinned in this corpus.** b ≈ 0.1 traces to Assibey-Bonsu & Muller (2014) through `planning/research/R7_technical_methods_sota.md` line 639. That paper is not in the corpus, so the plan's "FACT" label cannot be checked against a page here.
4. **The two results do not conflict.**
   - The RMA slope is σ_Z/σ_Z*.
   - GB-11's b is the regression of truth on the estimate: b = ρ·σ_Z/σ_Z*.
   - Any estimator that restores SMU-scale variance (LUC, a single realization) has σ_Z* ≈ σ_Z, and therefore **b ≈ ρ** (INFERENCE, E355).
   - Abzalov's own LUC case therefore implies b ≈ 0.85, not 1.
   - A b of about 0.1 means the within-panel ranking had ρ ≈ 0.1 against grade control. That is the failure Abzalov's validity conditions are meant to exclude: low nugget, and data "not too sparse or remote from the panel" (E353).

**Paste (GB-11 stress-case bullet):**
> - Stress case: estimators that restore SMU-scale variance (LUC, single realizations) have b ≈ ρ(Z, Z*) by
>   construction, so where local correlation is poor the slope collapses. One published check of Localised Uniform
>   Conditioning against grade control reported b ≈ 0.1 (Assibey-Bonsu & Muller 2014; FACT[U] until the page is
>   pinned; E029). Then it is **1.1 g/t (−45%)**. The originator restricts LUC to low-nugget domains whose data grid
>   is "not too sparse or remote from the panel" (AMG PDF 311).

**Paste (E029 text):** "Kriging conditional bias; Localised Uniform Conditioning (LUC; Abzalov 2006) with slope ≈ 0.1
against grade control in one critical study (Assibey-Bonsu & Muller 2014; page not pinned)."

**Paste (§13.3/§13.4, `Estimator::Luc`):** "`Estimator::Luc` carries a ValidityDomain: nugget/sill ratio, data spacing
vs variogram range, data-to-panel distance, and a border-effect flag (AMG PDF 311). Outside it, the projection
renders `OutsideValidity`."

### 2. "Slope" must be typed, and the GB-11 flip needs correlation and cut-off tonnage

**Where:** §13.1 GB-11 flip (line 1451), §13.3 `ValidationStats` (line 1498), §13.11 and §24.4 support pack.
**Type:** CORRECTION. **Evidence:** E356–E358, E355; FX-3.

- **The textbook warning.** Slope values of 0.95–1.00 "can mislead", and correlation can be as low as 0.5 while the slope is 0.9–1.0 (AMG PDF 282).
- **The worked case (Table 19.4, PDF 290).**
  - Empirical b is 0.89–1.05 and the theoretical slope 0.93–0.95 for every block size from 1×1 to 40×40 m.
  - Correlation is only 0.41–0.68, and the OK SD at 1×1 m is 4.5 against a true SD of 10.0.
  - Yet the tonnage error at the cut-off is "excessively large" until block size equals the data spacing (PDF 286).
- **What b ≥ 0.95 does and does not guarantee.**
  - It **does** guarantee that blocks selected on the model are, on average, as rich as the model says.
  - It **does not** guarantee recoverable tonnage and metal when mining selects on later, denser information (grade control). That needs ρ and the grade–tonnage curve as well.

**Paste (flip):**
> - **Flip:** in a domain where, against grade-control truth, (i) b ≥ 0.95, (ii) ρ(Z, Z*) ≥ the policy minimum and
>   (iii) tonnage and metal above the operating cut-off are within policy tolerance, `DeclaredOk` at SMU is adequate
>   there and simulation buys little. Slope alone is not enough: in Abzalov's synthetic gold case b = 0.89–1.05 at
>   every block size, while tonnage error is excessive below the 40×40 m data spacing (AMG PDF 286, 290).

**Paste (§13.3 `ValidationStats` comment):**
`/* swath, NN-vs-estimate bias, kriging efficiency, slope {kind: RegressionTrueOnEstimate | Rma}, rho(Z,Z*), variance ratio σ_Z*/σ_Z, OK-vs-DGM grade–tonnage deviation at registered cut-offs (AMG PDF 314–315) */`

Add a lint rule: no rule may compare an `Rma` slope with a regression-slope threshold. Add FX-3 as a known-answer
test: the stats must flag 1×1 m blocks (σ_Z*/σ_Z = 0.45, ρ = 0.41) even though b = 0.92.

### 3. CRM rules must name their SD basis; a CANMET "pass" is tolerance, not unbiasedness

**Where:** §12.1 `QaqcInsertion` (line 1341), §12.4 (line 1387). **Type:** CORRECTION.
**Evidence:** E364–E366; FX-2.

- **Abzalov's two tests.**
  - The single-value test is |X − μ| ≤ 2σ_C, where σ_C is the certified *within-lab* SD (PDF 151).
  - The batch test is the CANMET mean test (PDF 150).
- **His worked example passes as "accurate (unbiased)":** |m − μ| = 0.36 ≤ 4·S_W = 0.368 (PDF 150).
- **The same ten values under the plan's rule (INFERENCE, E366).**
  - On the σ_C basis: all 10 are outside μ ± 2σ_C and 8 are outside ± 3σ_C.
  - On the σ_L basis: 3 are outside ± 2σ_L and none outside ± 3σ_L.
  - The bias of +0.36 % Fe is about 12 standard errors (0.092/√10 = 0.029).
- **The consequence.** The plan's "±2 SD warning, ±3 SD failure" gives 10 warnings and 8 failures, or 3 warnings and 0 failures, or a pass, for the same batch, depending on an SD it never names.

**Paste (§12.4, first CRM bullet):**
> - A CRM rule is policy data that names (a) its SD basis: certified within-lab σ_C, certified between-lab σ_L, or
>   the lab's running S_W; and (b) its test: single-value |X − μ| ≤ k·SD, or the batch-mean CANMET test
>   |m − μ| ≤ 2√(σ_L² + S_W²/n) (AMG PDF 150–151). The default renders a warning at k = 2 and a failure at k = 3
>   **on the named basis**. Bias magnitude with its interval is always reported beside pass/fail, because a tolerance
>   test can pass a statistically certain bias (AMG Ex. 10.1.1.a).

**Paste (§12.1 `QaqcInsertion`):** "CRM (certified mean, σ_C and σ_L, certificate source)".

**Paste (§12.6 fixture):** "Known answer: AMG Ex. 10.1.1.a gives a CANMET pass, 10/10 warnings on the σ_C basis and 3/10
on the σ_L basis."

### 4. QA/QC monitors: add the textbook multi-rules and a dispersion-collapse (tampering) monitor

**Where:** §12.4, GATE-CORE-02 (line 1428). **Type:** ADDITION. **Evidence:** E367–E368.

- **Trend rules.** Two successive points beyond 2 SD, or four successive points beyond 1 SD (Leaver et al. 1997). The book also expects about 95% of points within 2 SD, balanced either side of the mean (AMG PDF 154).
- **Tampering.** A rapid fall in CRM dispersion is read as possible data tampering (Sketchley 1998; PDF 154).
- **The gap.** Neither the plan's threshold rules nor a mean-drift e-process detects a collapse in dispersion.

**Paste (§12.4, new bullet):**
> - **Pattern monitors** run beside the e-process per (CRM, lab, method): two successive results beyond 2 SD; four
>   successive beyond 1 SD on one side; sign imbalance; and a **dispersion-collapse** monitor (running SD well below
>   the certified SD), which can indicate tampering (AMG PDF 154). A dispersion-collapse alarm opens an
>   InvestigationCase with integrity scope, not a recalibration.

**Paste (GATE-CORE-02, appended):** "...and on streams with injected variance collapse (TARGET: running SD at 0.3 of
certified), the dispersion monitor alarms within its documented delay."

### 5. Duplicates: one canonical precision statistic, with reference levels as policy data

**Where:** §12.3 (line 1374), §12.1, §24.5. **Type:** CORRECTION + ADDITION.
**Evidence:** E369–E374; FX-4, FX-5.

- **HARD and AMPD add nothing.** For a pair, CV = √2·|a−b|/(a+b), so HARD = CV/√2 and AMPD = √2·CV. The book says they "offer no more information" than CV, and recommends CV_AVR% as the universal measure (AMG PDF 158–159, 166).
- **Thompson–Howarth is the optimistic statistic.** It needs ≥ 50 pairs and assumes normal errors, and it gave the lowest precision error in every case study (PDF 157, 166).
- **Reference values.** Reference levels (Table 10.3) and insertion rates (PDF 167, 169).
- **Duplicate mass.** A quarter-core duplicate of a half-core sample has half the mass and is unfit (PDF 170).

**Paste (§12.3 duplicates bullet):**
> - field, coarse-reject and pulp duplicate pairs give the variance at each stage. The canonical statistic is
>   CV_AVR% = 100·√((2/N)·Σ(a−b)²/(a+b)²) per (stage, analyte, grade band). HARD (= CV/√2) and AMPD (= √2·CV) are
>   derived views (AMG PDF 158–159, 166). Thompson–Howarth plots show their N (≥ 50 required) and are labelled
>   "assumes normal errors; optimistic" (AMG PDF 157, 166). A duplicate's mass must match the original's: no
>   quarter-core duplicates of half-core samples (AMG PDF 170).

**Paste (§12.1 `QaqcInsertion` note):**
"Default TARGETs, which QP policy can override: field duplicates 5–10%, pulp duplicates 3–5%, CRMs 3–5%, and ≥ 5% of
duplicates to an external laboratory (AMG PDF 169). Reference precision levels (AMG Table 10.3, PDF 167) are
registered as `PolicyDefault`, never as compliance."

### 6. Gy: the calibrated form, calibration provenance, grade-level validity and a fixture

**Where:** §12.3 (lines 1366–1371). **Type:** VALIDATION + ADDITION. **Evidence:** E359–E363; FX-1.

- **The plan's FSE formula is confirmed**, with units: d in cm, M in g, c in g/cm³ (AMG PDF 135–136).
- **The practice form.** Practice uses σ²_FSE = K·d_N^α·(1/M_S − 1/M_L), with K = f·g·c·d_L^(3−α).
  - K and α must be calibrated: in lode gold, K spans 1 to more than 10,000 (PDF 137).
  - The uncalibrated default is K = 470 and α = 1.5 (François-Bongarçon 1993; PDF 383).
- **Validity and acceptance.**
  - A nomogram is valid only at the grade level it was built for (PDF 136).
  - Gy's safety line is a relative variance of 0.01 (PDF 142).
  - A sample needs at least 30 increments (PDF 144).
- **Ingamells is not mentioned in AMG.** The plan's "Ingamells formulations apply" is neither confirmed nor refuted by this corpus.

**Paste (§12.3 Gy bullet):**
> - the general form is σ²_FSE = (1/M_S − 1/M_L)·c·f·g·ℓ·d³ (d in cm, M in g, c in g/cm³); the calibrated form is
>   σ²_FSE = K·d_N^α·(1/M_S − 1/M_L). K and α are Claims with a calibration method (`Mste`, `ThirtyPieces`,
>   `HeterogeneityTest`, `DrillData`) or `Assumed` (François-Bongarçon default K = 470, α = 1.5; AMG PDF 383). In lode
>   gold K spans four orders of magnitude (AMG PDF 137), so an `Assumed` K makes the nomogram `Speculative`.
> - A nomogram records the grade at which K was calibrated and is `OutsideValidity` at other grades (AMG PDF 136).
>   The default acceptance line is Gy's safety line, relative variance 0.01 (10% relative error; AMG PDF 142),
>   registered as policy data. A sample made of fewer than 30 increments or cuts is flagged (AMG PDF 144).

**Fixture (§12.6):** FX-1, K = 316.9.

### 7. Twin-hole studies as a first-class record, with a disclosure guard

**Where:** §12.1, §12.4, §24.5. **Type:** NEW-IDEA. **Evidence:** E375–E377, E391; FX-8.

- **Spacing.**
  - Twins should be no more than 5 m apart (AMG PDF 175).
  - Precision degrades with spacing: CV_AVR rose from 23.8% to 35.7% as spacing went from 1 m to 10 m (PDF 175).
  - The Busang fraud was exposed by seven twins drilled 1.5 m from the originals (PDF 175).
- **Study size.** Studies of 20–30 twins are "not uncommon" (PDF 177).
- **RC contamination.** Down-hole smearing in RC holes shows as a systematic grade decrease down the hole, and twinning RC holes with diamond holes is recommended as routine (PDF 385).
- **Disclosure.** Corrections derived from twins are "unlikely" to be accepted by a regulator or in public reporting (PDF 174).

**Paste (§12.1 new record row):**
`TwinPair` | original hole, twin hole, separation (m, from desurveyed traces), drilling techniques, matched intervals,
per-pair CV_AVR%, RMA, paired t-test | A separation above 5 m (policy) renders `WeakTwin`. A twin study is a Claim over
its pairs (AMG PDF 174–177).

**Paste (§12.4 guard):** "G-TWIN: a correction derived from a twin study may feed a `Full`-basis internal projection
with its uncertainty. It never enters a `Public` or disclosure-basis estimate without a recorded QP acceptance
(AMG PDF 174)."

**Paste (§12.4 monitor):** "RC smearing monitor: for each RC hole, a significant negative trend of grade with
down-hole depth inside mineralized intervals opens an InvestigationCase proposing diamond twins (AMG PDF 385)."

### 8. Core recovery, data generations and downhole surveys: three missing checks

**Where:** §12.1 `Interval` and `Survey` (lines 1336–1337), §12.2. **Type:** ADDITION. **Evidence:** E378–E381.

- **Core recovery.**
  - Poor recovery is "the main source" of diamond-drilling sampling error (AMG PDF 65).
  - Recovery can be measured by length, mass or volume (PDF 65–66), and broken and solid core should be sampled separately (PDF 67–68).
  - In one case, Al2O3 under-estimation was traced to a low-recovery 2000 campaign by grouping data by campaign (PDF 66, 224).
- **Data generations.** Every sample should be dated and the data compared by "data generation" (PDF 224).
- **Downhole surveys.**
  - Magnetic surveys in magnetite-bearing rock were shifted more than 30 m relative to gyro, even in 150–200 m holes (PDF 129).
  - Holes should be surveyed both going in and coming out (PDF 130).
  - Collar elevation should be checked against the DTM (PDF 128).

**Paste (§12.1 `Interval`):** "recovery % **with method** (length | mass | volume | unknown)".

**Paste (§12.2, new laws):**
> - Recovery–grade bias test: per (domain, campaign, drilling method), report the dependence of grade on recovery.
>   A significant dependence opens an InvestigationCase and marks the affected intervals `Conflicted`
>   (AMG PDF 65–68).
> - Data-generation comparison: each campaign is compared with the others (Q–Q plot, means by domain) before it
>   joins an estimate (AMG PDF 224).
> - Survey checks: in/out run consistency and collar-vs-DTM residuals are recorded. A magnetic survey in a domain
>   with logged magnetite or high susceptibility is `Suspect` until a gyro check exists (AMG PDF 128–130).

### 9. Geometallurgical sample coverage against the SMU distribution

**Where:** §17.4 OPEN-STRATA-8, §13.6. **Type:** NEW-IDEA. **Evidence:** E389.

**Paste (§17.4, after OPEN-STRATA-8):**
> **Geomet coverage report.** For each metallurgical domain, Crucible lists which percentile bands of Strata's
> SMU-support distribution of each primary variable are covered by tested samples. The target is every 5th
> percentile; with fewer than ten samples, at least the 5th, 25th, 50th, 75th and 95th (AMG PDF 368). Transfer cards
> evaluated outside the covered bands render `OutsideValidity`. This turns OPEN-STRATA-8 into a measurable coverage
> gap before the forecast comparison runs.

### 10. The nugget split should drive a protocol-versus-infill decision

**Where:** §12.3 (nugget split), §22 H3c information options (line 1927). **Type:** ADDITION. **Evidence:** E390.

The plan already uses the duplicate decomposition to split the nugget into geological and sampling parts. It does
not yet use the split to drive a decision.

**Paste (§12.3, appended to the decomposition bullet):** "When the sampling share of the nugget (duplicate relative
variance at composite support ÷ the pairwise-relative-variogram nugget) exceeds a policy share, Helm's information
options include 'improve the sampling protocol' alongside 'infill drilling', priced by the same EVSI
(AMG PDF 373–374)."

### 11. Classification support pack: thresholds as a policy triple, with the margin caveat

**Where:** §13.11, §24.4 (lines 4430–4441). **Type:** VALIDATION + ADDITION. **Evidence:** E387–E388; FX-7.

The plan's labelled heuristic (±15% at 90% over a quarter or year) matches Abzalov's proposal (AMG PDF 357).
Abzalov also gives variants and warns that ±15% "can be too loose" because operating margins rarely exceed 15%.

**Paste (§24.4 bullet):** "simulation-based P(metal within ±t over a production period at confidence c), where
(period, t, c) is a registered policy triple with named variants: quarterly/annual ±15% at 90%; ±10% at 95% (AMG PDF
357). The pack prints the margin caveat (AMG PDF 357). This remains an industry heuristic, labelled as such."
G-HALL-1 is unchanged.

### 12. Density: measurement coverage and the laboratory check rate

**Where:** §13.6 density field, §24.5 ("density and survey verification"). **Type:** ADDITION.
**Evidence:** E382–E383; FX-6.

**Paste (§24.5, new sub-item):** "Density verification: the laboratory check fraction (textbook default: 5–10% of
measurements; AMG PDF 116), and the density measurement grid against a geostatistically optimal grid per confidence
class (e.g. orogenic Au: density 250×250 m for Proved and 450×450 m for Probable, against grade grids of 40×40 m and
70×70 m; AMG Table 7.2, PDF 125)."

### 13. Reconciliation: two validations and one guard

**Where:** §20.4, §14.3 (caving). **Type:** VALIDATION + guard. **Evidence:** E384–E386.

- **Two validations.**
  - Reconciling reserves against production "must be used strictly together with" reconciliation against grade control (AMG PDF 321). This validates computing F1 and F2 separately and the plan's compensation detector.
  - Grade-control grids of 5×5 to 20×20 m (PDF 322) support using grade-control data as the "truth" for measuring slope in GB-11.
- **Guard (Phase 2 caving).** Draw-point grade-control samples at block caves "cannot be used for verification reserves" (PDF 324).

**Paste (§20.11 guard):** "G-TOUCH-CAVE: draw-point grade samples are admissible as observations of cave-draw grade,
never as the grade-control term of F1 for reserve verification (AMG PDF 324)."

### 14. Highwall cards: water counted twice, "safe" wording, Q's floor and the Is50 factor

**Where:** §14.1, §14.2, §13.6, §32 importers. **Type:** RISK. **Evidence:** E392, E396–E398; FX-10.

- **Water counted twice.**
  - RMR includes a groundwater rating (dry 15 down to flowing 0), and Q includes Jw tied to joint water pressure (FEG PDF 53, 85).
  - If an index-derived strength card feeds a limit-equilibrium analysis that also applies §14.5 pore pressure, water is counted twice (INFERENCE).
- **"Safe" wording.** Waltham's class table has a column headed "Safe cut slope (°)" (FEG PDF 53). An imported textbook table must never render "safe" (plan doctrine).
- **Q details.**
  - Q has a floor rule: "If RQD <10, use 10" (FEG PDF 85).
  - The Q tables come from tunnel-support design (Barton et al. 1974, cited on FEG PDF 85).
- **UCS from point load.** Is ≈ UCS/20, and strength tests carry ±20% error (FEG PDF 52).
- **Classification as design.** AMG says classification indices should not be the sole design tool (PDF 109). This validates §13.6.

**Paste (§14.1, card rule):** "An index-derived strength card declares `water_basis: dry_normalised | as_logged`. An LEM
run with explicit pore pressure accepts only `dry_normalised` inputs (groundwater rating set to dry, Jw = 1).
Otherwise lint raises 'water double-counted'."

**Paste (§32 importer rule):** "Guideline tables from textbooks are imported with column names mapped to neutral
terms (e.g. 'guideline cut-slope angle'). The word 'safe' never reaches a rendered label."

**Paste (Q card):** "Q = (max(RQD, 10)/Jn)·(Jr/Ja)·(Jw/SRF) (FEG PDF 85). Validity domain: underground openings
(tunnel-support basis). Open-pit use is `OutsideValidity` without site calibration (INFERENCE from the stated
basis)."

**Paste (UCS(Is50) card):** "UCS = k·Is50, where k is a calibrated parameter per domain. The textbook prior is k ≈ 20
(FEG PDF 52), with ±20% test error. A prior alone is `Assumed`."

### 15. TARPs and time-of-failure: antecedent rainfall, and the "creep will stabilise" fallacy

**Where:** §14.4, §14.5. **Type:** ADDITION. **Evidence:** E394–E395; FX-9.

- **The main trigger.** A rise in groundwater is "by far the most important single trigger" of landslides (FEG PDF 68).
- **Vaiont (FEG PDF 70).**
  - Movement correlated with reservoir level and with rainfall over the previous 60 days.
  - The back-analysed factor of safety falls from 1.21 to 1.00 on water alone.
  - The slide failed at 701 m in a wet period, although the reservoir was designed to fill to 722 m.
  - Borehole data showed no movement at the toe while the surface was creeping.
  - In Waltham's words, "The error was to assume slide would creep until it stabilized".

**Paste (§14.4, after the TARP automaton bullet):**
> - Trigger predicates may use antecedent rainfall over a registered window (e.g. 60 days, set by site
>   calibration) and reservoir or pit-lake level, not only piezometers (FEG PDF 68, 70). A creep forecast that assumes
>   deceleration is a registered hypothesis scored like any other; "creep will stabilise" is never a default.

### 16. LEM known answer: the planar-slide closed form (low)

**Where:** §14.2, GATE-HIGH-01. **Type:** ADDITION. **Evidence:** E393.

**Paste:** "Metamorphic test: on a single planar slip surface, every LEM method must reproduce
FoS = (c·A + (W cos α − U) tan φ)/(W sin α) (FEG PDF 68; Waltham writes c for the cohesive force across the plane, so
multiply by area A)."

### 17. Dilution attribution taxonomy (low)

**Where:** §20.5–§20.6. **Type:** ADDITION. **Evidence:** E399.

**Paste (§20.5 table row note):** "Residual dilution and ore loss are attributed to four registered sources: internal
(waste inside the model), planned (design; LN cites 10–40%), accidental (overbreak and sloughing → Highwall and
Headframe) and geologic (contact uncertainty → Strata) (LN PDF 58–59)."

### Validations needing no change

- **Krige's additivity relation** (AMG PDF 302) supports PO-STRATA-4. The change-of-support conditions (Cartier's relation) are on PDF 303.
- **Blasthole errors** (cone segregation, sub-drill contamination, delimitation; AMG PDF 79–83) support the §12.3 blasthole bullet.
- **Method = measurand** is supported in two places:
  - fire assay of high-sulphide samples needs roasting, and untreated assays "can be seriously biased" (AMG PDF 383);
  - sulphide nickel should be assayed by bromine-methanol to exclude silicate nickel (LN PDF 59).
- **Conditional simulation for plant tolerance** (Corridor Sands, AMG PDF 331–335) supports the P(>c) projections.
- **The NN/geometric cross-check:** "Computer-generated estimates must always be compared against geometric-generated block estimates" (LAC PDF 92) supports the NN-vs-estimate check in `ValidationStats`.

---

## 5. Proposed evidence rows (E351–E399)

| E### | Claim | Source + page | Quote (≤ 25 words) | Class |
|---|---|---|---|---|
| E351 | AMG is Abzalov, *Applied Mining Geology*, Springer 2016, MASES vol. 12 | AMG PDF 351 (also PDF 4–5) | "© Springer International Publishing Switzerland 2016 M. Abzalov, Applied Mining Geology, Modern Approaches in Solid Earth Sciences 12" | FACT |
| E352 | LUC = Localised Uniform Conditioning, developed by Abzalov in 2006 | AMG PDF 307 | "A new approach, called Localised Uniform Conditioning (LUC), has been developed in 2006 (Abzalov 2006)" | FACT |
| E353 | The originator's LUC validity conditions: low nugget, continuity at the variogram origin, data not too sparse or remote | AMG PDF 311 | "low nugget effect and exhibit good continuity at their variogram origins ... data grid is not too sparse or remote from the panel" | FACT |
| E354 | The originator's LUC validation (iron oxide, Fig. 23.5) reports RMA slope = 1 against drill-hole intersections; r = 0.85 (LUC) vs 0.86 (OK) | AMG PDF 317–318 | "eliminated the conditional bias where slope of the data distribution on the diagram is equal to 1" | FACT |
| E355 | b = ρ·σ_Z/σ_Z* and RMA slope = σ_Z/σ_Z*, so a variance-restoring estimator has b ≈ ρ: Abzalov's LUC case implies b ≈ 0.85, and b ≈ 0.1 implies ρ ≈ 0.1 | Derived from E354 and the slope definitions, AMG PDF 280–282 | — | INFERENCE (algebra above; assumes a linear relationship) |
| E356 | A slope of regression of 0.95–1.00 can give false confidence; use it with kriging variance and efficiency | AMG PDF 282 | "values of 0.95–1.00 suggesting lacking of a conditional bias can mislead geologists creating them a false impression of the reliable estimates" | FACT |
| E357 | Correlation can be as low as 0.5 while the slope is 0.9–1.0 | AMG PDF 282 | "when slope of regression lay in the rage of 0.9–1.0 ... the correlation between Z and Z* can be as low as 0.5" | FACT |
| E358 | Synthetic gold, 40×40 m data: b 0.89–1.05 and r 0.41–0.68 at every block size; tonnage error excessive below the data spacing | AMG PDF 286, 290 | "According to this study, the only acceptable block size is 40 × 40 m, which is matching to the data grid" | FACT |
| E359 | A Gy formula or nomogram is valid only at the grade level of its calibration | AMG PDF 136 | "any use of a formula and the sampling nomogram derived from is relevant only to the grade level at which it is established" | FACT |
| E360 | Gy's safety line is a relative variance of 0.01 (10% relative error) | AMG PDF 142 | "draw line corresponding to 10 % relative error, or in other words the relative variance equal to 0.01" | FACT |
| E361 | 30-pieces example: rel. var. 9.41, mass 2889 g, d_N 19.45 cm, α 1.5 → K = 316.9 | AMG PDF 141 | "gives the nominal particle size (dN) equal to 19.45 cm" | FACT (K re-computed: 9.41 × 2889 / 19.45^1.5 = 316.9) |
| E362 | K in lode gold ranges from 1 to more than 10,000 | AMG PDF 137 | "at the lode gold deposits K-constant values vary from 1 to more than 10,000 (Table 9.2)" | FACT |
| E363 | Default sampling constants when uncalibrated: K = 470, α = 1.5 (François-Bongarçon 1993) | AMG PDF 383 | "suggested to use as a default values the K-constant equal to 470 and α-constant equal to 1.5" | FACT |
| E364 | Single-CRM test: \|X − μ\| ≤ 2σ_C, with σ_C the certified (within-lab) SD | AMG PDF 151 | "when decision on the analytical accuracy should be made using a single value (X) of the certified standard, the commonly used statistical test is" | FACT |
| E365 | Worked example (FX-2): \|61.09 − 60.73\| = 0.36 ≤ 4·S_W = 0.368, declared accurate | AMG PDF 150 | "This result shows that analytical method is accurate (unbiased) as required." | FACT |
| E366 | For FX-2: 10/10 values exceed 2σ_C and 8/10 exceed 3σ_C; 3/10 exceed 2σ_L and 0 exceed 3σ_L; bias ≈ 12 SE. A ±k SD rule without a named basis is ambiguous. | Derived from AMG PDF 150 values | — | INFERENCE (deviations 0.21–0.51 vs σ_C 0.09, σ_L 0.20; SE = 0.092/√10) |
| E367 | CRM trend rules: 2 successive beyond 2 SD or 4 successive beyond 1 SD | AMG PDF 154 | "two successive points lie outside the two standard deviations or four successive points lie outside the one standard deviation (Leaver et al. 1997)" | FACT |
| E368 | A rapid fall in CRM dispersion indicates possible tampering | AMG PDF 154 | "Such decrease in the standards variability is commonly interpreted (Sketchley 1998) as indicative of the data tampering." | FACT |
| E369 | The Thompson–Howarth method requires ≥ 50 duplicate pairs | AMG PDF 157 | "to use when 50 or more duplicated results are available" | FACT |
| E370 | Thompson–Howarth gives the lowest precision error of all methods (it assumes normal errors) | AMG PDF 166 | "precision errors produced by the Thompson-Howarth technique are consistently the lowest among all the reviewed methods" | FACT |
| E371 | CV_AVR% is recommended as the universal precision measure; HARD and AMPD are proportional to CV | AMG PDF 159, 166 | "the author concurs with the proposal of Stanley and Lawie (2007a) that CVAVR (%) be used as the universal measure of relative precision error" | FACT |
| E372 | Reference best/acceptable precision levels by deposit type and sample type (FX-4) | AMG PDF 167 | "Best and acceptable levels of the precision errors proposed as reference for evaluating the mining projects" | FACT |
| E373 | Insertion rates: field duplicates 5–10%, pulp duplicates 3–5%, CRMs 3–5%, ≥ 5% of duplicates external | AMG PDF 169 | "approximately 5–10 % of field duplicates, and 3–5 % of the pulp duplicates" | FACT |
| E374 | Quarter-core duplicates of half-core samples are unfit (half the mass) | AMG PDF 170 | "it is unwise to collect quarter core as a duplicate as this will produce a duplicate samples of a twice smaller weight" | FACT |
| E375 | Twins at most 5 m apart; at Yandi, CV_AVR rose from 23.8% to 35.7% from 1 m to 10 m spacing | AMG PDF 175 | "good practice is not to drill matching pairs of holes more than 5 m apart" | FACT |
| E376 | Busang fraud exposed by seven twins drilled 1.5 m from the originals | AMG PDF 175 | "Fraud was revealed by seven twinned holes (Lawrence 1997). Each of the seven new holes was drilled only 1.5 m away" | FACT |
| E377 | Twin-based geostatistical corrections are unlikely to be accepted for public reporting | AMG PDF 174 | "It is unlikely that the new resource or reserve estimates based on geostatistically corrected data will be accepted by a formal regulator" | FACT |
| E378 | Poor core recovery is the main diamond-drilling sampling error | AMG PDF 65 | "Poor core recovery is the main source of sampling errors in diamond drilling." | FACT |
| E379 | Samples must be dated and compared by data generation; a biased 2000 campaign was found this way | AMG PDF 224 (case also PDF 66) | "All samples should be assigned the date when sample was collected and processed which is used for grouping the data into the data generations" | FACT |
| E380 | Magnetic survey shifted > 30 m from gyro in magnetite-bearing rock | AMG PDF 129 | "The drill hole coordinates based on the magnetic down hole survey were shifted by more than 30 m in comparison with the Gyro survey results" | FACT |
| E381 | Survey each hole twice, going in and coming out | AMG PDF 130 | "it is a good practice to survey the holes twice" | FACT |
| E382 | Check 5–10% of density measurements in a laboratory | AMG PDF 116 | "in total 5–10 % of original measurements should be checked and confirmed by the laboratory tests" | FACT |
| E383 | Optimal density grids are much sparser than grade grids (FX-6) | AMG PDF 125 | "Geostatistically estimated optimal sampling grids for estimation reserves and for the production grade control (Abzalov 2013)" | FACT |
| E384 | Grade-control samples are typically at 5×5 to 20×20 m spacing | AMG PDF 322 | "requires large amount of additional samples usually taken at the distances of 5 × 5 m to 20 × 20 m" | FACT |
| E385 | Reconciliation of reserves against production must be paired with reconciliation against grade control | AMG PDF 321 | "reconciliation of reserves with a mine production must be used strictly together with reconciliation with the grade control data" | FACT |
| E386 | Block-cave draw-point samples cannot verify reserves | AMG PDF 324 | "validity of such grade control samples is seriously questionable and this data cannot be used for verification reserves" | FACT |
| E387 | Measured = quarterly production volume within ±15% at 90% confidence (Indicated: annual) | AMG PDF 357 | "representing the quarterly production (3 months), is estimated with an error ±15 % at the 90 % confidence level" | FACT |
| E388 | ±15% can be too loose because margins rarely exceed 15% | AMG PDF 357 | "using ±15 % error can be too loose because profit margin of the mining operations is rarely exceeding 15 %" | FACT |
| E389 | Geomet or bulk samples should span the percentiles of the SMU histogram | AMG PDF 368 | "if the number of samples is less than ten, they should match at least the 5th, 25th, 50th, 75th and 95th percentiles" | FACT |
| E390 | If sampling error dominates the nugget, improve the protocol rather than infill | AMG PDF 374 | "when sampling error constitutes most of the nugget effect ... can be achieved by updating the sampling protocols" | FACT |
| E391 | RC smearing shows as a down-hole grade decrease; twin RC holes with diamond holes | AMG PDF 385 | "systematic decrease of the gold grade in the down-the-hole direction, which is indicative of the down-the-hole smearing" | FACT |
| E392 | Rock-mass classification is not a sole design tool | AMG PDF 109 | "use of these systems as the sole design tool cannot be supported on scientific grounds" | FACT |
| E393 | Planar slide: D = W sin α; R = c + (W cos α − u) tan φ; SF = R/D | FEG PDF 68 (image) | "Safety factor = R / D = resistance/driving force." | FACT |
| E394 | Rising groundwater is the dominant landslide trigger | FEG PDF 68 (image) | "rise in groundwater pressure is by far the most important single trigger factor behind landslides" | FACT |
| E395 | Vaiont: SF 1.21 → 1.00 with reservoir level and rainfall; movement correlated with the previous 60 days' rainfall; the slide failed, contrary to the assumption that creep would stabilise | FEG PDF 70 (image) | "The error was to assume slide would creep until it stabilised on flatter toe." | FACT |
| E396 | Point-load strength Is ≈ UCS/20; strength tests carry ±20% error | FEG PDF 52 (image) | "Point Load Strength (Is) is then close to UCS/20." | FACT |
| E397 | Q calculation floors RQD at 10 | FEG PDF 85 (image) | "If RQD <10, use 10 to calculate Q" | FACT |
| E398 | Waltham's RMR/Q class table includes a "Safe cut slope" column (FX-10) | FEG PDF 53 (image) | "Guideline Properties of Rock Mass Classes ... Safe cut slope (°) >70 65 55 45 <40" | FACT |
| E399 | Dilution sources: internal, planned (usually 10–40%), accidental, geologic | LN PDF 58–59 | "Dilution may come from internal waste, planned waste (intentional dilution), accidental waste or geologic waste." | FACT |

---

## 6. Checked nulls

- **Barrick, NGM and JV data.** Nothing on Barrick, Newmont, Nevada Gold Mines or any Barrick asset appears in AMG, LAC or LN (full text search). FEG was searched only through its OCR mirror, so no ownership basis is needed.
- **Plan formulas these books do not contain.** The following are neither confirmed nor refuted by this corpus:
  - Kuz-Ram and Swebrec (§16.3);
  - PPV scaled distance (§16.3);
  - Bond's law (§17.3; LAC's comminution pages have no work-index formula);
  - Hoek–Brown and GSI (no GSI in AMG; none in the FEG pages viewed);
  - Mathews/Potvin (§14.3);
  - NAPP (§18.5);
  - F1/F2/F3 and the mine call factor (§20.4);
  - Lane cut-off (AMG cites "Lane 2015" only, PDF 370).
- **Ingamells.** Not in AMG. The §12.3 statement is unverified here.
- **Kriging neighbourhood analysis.** AMG does not use the term "KNA". The diagnostics it does give (slope, kriging efficiency, Lagrange multiplier, weight of the mean; PDF ≈280–284) are already covered by impact 2.
- **The b ≈ 0.1 primary source.** Assibey-Bonsu & Muller 2014 is not in the corpus, so the value cannot be page-verified (impact 1).
- **LAC.** Only an overview of estimation methods (polygonal, triangular, sectional, contouring, inverse distance, kriging; PDF 90–92) and cost-estimate accuracy ranges (±40% preliminary, ±10% detailed; PDF 92, 101). It adds nothing the plan lacks.
- **LN.** Core recovery = l/L × 100% (PDF 20) and equivalent-grade formulas (PDF 58) are textbook-standard. Nothing beyond impact 17.
- **FEG groundwater and geophysics** (PDF 40, 47). Pumping-test and packer-test permeability formulas, and fractured-rock Vp ≈ unfractured Vp × RQD/100, are standard and site-investigation scale. No plan change.
- **Unverified and therefore omitted.** An aqua-regia vs fire-assay comparison was not found on AMG PDF 378–387, so no claim is made.
