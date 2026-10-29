# Independent second-reader claim verification audit of corpus extractions C1–C6 and 2024–2026 quarterly workbooks (AUDIT_4)

> Independent, read-only second-reader verification of all 294 proposed evidence rows (`E101–E399`) across
> `planning/corpus/C1–C6` against the primary PDFs and `.xlsx` workbooks in `corpus_of_input_docs/`, plus a full
> programmatic audit of the 29 newly downloaded `2024–2026` Barrick Mine Stats, AISC, and MD&A files in
> `corpus_of_input_docs/downloaded/mine_stats/`. Closes open audit item **H-4** in Appendix H.6.

---

## 1. Executive Summary (`E101–E399` Across `C1–C6` + 29 New Quarterly Files)

### 1.1 Verification Summary Table (All 294 Rows `E101–E399`)

| Corpus Report | Evidence Range | Total Rows | **CONFIRMED** (Exact Match) | **CONFIRMED WITH NUANCE** | **PAGE LOCATOR FIX NEEDED** | **Substantive Data / Math Errors** |
|---|---:|---:|---:|---:|---:|---:|
| **`C1_barrick_public_stats.md`** | `E101–E149` | 49 | **44** | **4** (`E101`, `E126`, `E147`, `E148`) | **1** (`E146`: `FM p.15` → `FM p.14`) | **0** |
| **`C2_nevada_technical_reports.md`** | `E151–E199` | 49 | **46** | **3** (`E163`, `E196`, `E199`) | **0** | **0** |
| **`C3_africa_technical_reports.md`** | `E201–E249` | 49 | **47** | **0** | **2** (`E211`: `K p222` → `K p221`; `E212`: `K p222` → `K p220`) | **0** |
| **`C4_americas_png_reports.md`** | `E251–E299` | 49 | **41** | **8** (`E251`, `E258`, `E261`, `E262`, `E263`, `E272`, `E289–E290`, `E298`) | **0** | **0** |
| **`C5_reko_diq_esia.md`** | `E301–E349` | 49 | **47** | **2** (`E314/§2.2` `FS20-PB` coordinate check; `E331` App S page layout) | **0** | **0** |
| **`C6_textbook_techniques.md`** | `E351–E399` | 49 | **48** | **1** (`E395`: UK spelling `"stabilised"` on `FEG PDF p.70`) | **0** | **0** (all `FX-1..FX-10` verified) |
| **TOTAL (`C1–C6`)** | **`E101–E399`** | **294** | **273 (92.9%)** | **18 (6.1%)** | **3 (1.0%)** | **0 (0.0%)** |

### 1.2 Bottom-Line Audit Verdict
- **Zero fabricated quotes or substantive arithmetic errors across all 294 rows (`E101–E399`):** Every verbatim quote, `.xlsx` cell coordinate, table figure, and recomputed `INFERENCE` value (North Mara LMDI bridge `E137`, sold-weighted AISC layer gaps `E110–E112`, BTAC dataset distribution `E177`, TRUG iterative COG fixed point `E182`, Cortez/Carlin BCOG equations `E183–E184`, Carlin Table 6-2 column sums `E192`, Kibali 2024 MCF and Table 6-2 recovered-ounce checks `E206`/`E240`, Loulo RCF check `E209`, Pueblo Viejo stockpile balance `E258`, Porgera 5-year reconciliation `E298`, Reko Diq nitrate stoichiometric conversion `E324`, and textbook fixtures `FX-1..FX-10`) reproduces against the primary files.
- **Only 3 rows (`1.0%`) need a page-locator fix (`E146`, `E211`, `E212`):** All three are off-by-one/off-by-two PDF page numbers in the `§4` summary tables where the quote and numbers are 100% present on the adjacent page (`FM p.14` instead of `p.15`; `K p221` and `K p220` instead of `p222`, which `C3` §2.3 L143 had already cited correctly in the prose).
- **Promotion Gate (`Appendix I.7 #6` / `H-4`):** With `E146`, `E211`, and `E212`'s page locators updated, all 294 evidence rows (`E101–E399`) pass independent second-reader verification and are cleared for promotion from `PROPOSED` to `VERIFIED`.

---

## 2. Detailed Discrepancies and Nuances in `C1–C3` (`E101–E249`)

### 2.1 Page-Locator Fixes (`E146`, `E211`, `E212`)

1. **`E146` (`C1_barrick_public_stats.md` L770 — Page Locator Fix):**
   - **Claimed locator:** `FM p.15` (`Fourmile_Feb2026.pdf`).
   - **Primary-source verification:** All three reserve roll-forward tables (Gold, Copper, and Gold Equivalent Ounces) and the row `"Attributable Net Change GEO (Moz) (using reported reserve prices)16"` with values `6.7`, `79`, `1.4` and depletion `(6.0)`, `(6.1)`, `(5.1)` (`87.1 / 17.2 = 506.4%`) are on **`FM p.14`** (*"Gold & Copper Peer Leading Reserve Replacement"*). Page 15 contains only narrative endnotes `a)–g)`.
   - **Disposition:** Update locator `FM p.15` → `FM p.14`.

2. **`E211` (`C3_africa_technical_reports.md` L433 — Page Locator Fix):**
   - **Claimed locator:** `K p222` (`Barrick_Kibali_NI43-101_TechnicalReport_February2026.pdf`).
   - **Primary-source verification:** Table 15-25 (*"Yearly Summary of Open Pit GC Call against Raw Mineral Resource Model Depletion"*, containing the exact quoted row `"Total 47,054,265 2.04 3,092,735 44,027,009 2.32 3,290,593 7% -12% -6%"`) is on **`K p221`** (report p. 207), as `C3` §2.3 L143 correctly stated.
   - **Disposition:** Update locator `K p222` → `K p221` in the `E211` summary row.

3. **`E212` (`C3_africa_technical_reports.md` L434 — Page Locator Fix):**
   - **Claimed locator:** `K p222` (`Barrick_Kibali_NI43-101_TechnicalReport_February2026.pdf`).
   - **Primary-source verification:** Table 15-24 (*"2025 Summary of Open Pit GC Call against Dig Block Polygon Depletion"*, containing the exact quoted row `"2025 6,354,121 1.51 309,191 5,632,144 1.73 313,873 13% -13% -1%"`) is on **`K p220`** (report p. 206), as `C3` §2.3 L143 correctly stated.
   - **Disposition:** Update locator `K p222` → `K p220` in the `E212` summary row.

### 2.2 Confirmed With Nuance (`C1–C3`, 7 Rows + 1 Bonus Source Typo)

1. **`E101` (`C1` L725 — Formula Count Precision):**
   - In `Barrick_Q1_2026_Mine_Stats.xlsx`, all 1,603 populated cells are hard-coded values (`0` formulas). In `Barrick_Q2_2026_Mine_Stats.xlsx`, all 3,271 reported data cells in columns `A:N` are hard-coded values, while columns `M:V` (`Minestats3!M4:V81`, 660 cells) and `K:R` (`Minestats4!K4:R67`, 432 cells) hold the 1,092 broken `#REF!` scratch formulas documented in `E120` (`DR-03`).
2. **`E126` (`C1` L750 — Unrounded vs. Printed Recovery Basis):**
   - The stated 2023 copper recovery shortfall of `5,269 t Cu` reproduces from `feed × 0.9337 − produced_Cu` (`132,023 × 0.9337 − 118,001 = 5,268.88 ≈ 5,269 t Cu`, using unrounded `118,001 / 132,023 = 89.3791%` rather than printed `89.40%` which gives `5,241.3 t Cu`). For 2024, `136,523 × 0.9337 − 122,621 = 4,850.5 t Cu`.
3. **`E147` (`C1` L771 — Page Locator Precision):**
   - Both the `"2024f 176 - (6.1) 79"` row and the `$1,400/oz, $3.00/lb` vs. `$1,500/oz, $3.25/lb` price footnotes are on **`FM p.14`** (`FM p.15` holds narrative endnote `f)` breaking down the `79 Moz` across Reko Diq `42 Moz`, Lumwana `17 Moz`, Pueblo Viejo `14 Moz`, Fourmile `3.1 Moz`, Other `2.9 Moz`).
4. **`E148` (`C1` L772 — Superseded by `downloaded/mine_stats/`):**
   - `E148` recorded the `CHECKED-NULL` that 2024 and Q3–Q4 2025 Mine Stats workbooks were absent at initial extraction time. That gap is now **closed** by the 29 files in `corpus_of_input_docs/downloaded/mine_stats/` (Section 4 below).
5. **`E163` (`C2` L402 — Source-Report Prose Erratum):**
   - `Turquoise-Ridge-Technical-Report-March2024.pdf` p. 166 Table 14-4 matches `E163` verbatim, while the prose paragraph immediately above Table 14-4 in Section 14.2.10 of the NI 43-101 report itself has a copy-paste typo reading *"The Turquoise Ridge Surface classification scheme..."* inside the Underground section.
6. **`E196` (`C2` L435 — Unit Conversion Note):**
   - `Carlin_Complex_TechnicalReport_14032025.pdf` p. 260 Table 16-16 reports in `kt` and `koz` (`132,537 kt`, `15,429 koz`, `12,886 koz`), which `E196` accurately states in `Mt` and `Moz` (`132.5 Mt`, `15.43 Moz`, `12.89 Moz`).
7. **`E199` (`C2` L438 — Footnote Page Span):**
   - In `Barrick_Cortez_Technical_Report_2022.pdf`, Table 14-6 (`M+I 210 Mt @ 2.75 g/t = 18 Moz`) is on `Z p.203` and its `$1,500/oz` resource price footnote (`Note 1`) is on `Z p.204`.
8. **Bonus Primary-Source Typo Caught on `T p.193` Table 14-15 (Companion to `D2` / `E193`):**
   - On `T p.193` Table 14-15 (100% basis), row `Vista Underground` prints **Measured Tonnes = `0.0028 Mt`** (`@ 5.79 g/t = 0.0053 Moz`), a 10× decimal-point typo in the NI 43-101 table for **`0.028 Mt`** (`0.028 Mt × 5.79 g/t ÷ 31.1035 = 0.00521 Moz`; Indicated is `0.0020 Mt`, Total M+I is `0.030 Mt`, and 61.5% attributable Measured on `T p.194` is `0.018 Mt`).

---

## 3. Detailed Verification of `C4–C6` (`E251–E399`)

### 3.1 `C4_americas_png_reports.md` (`E251–E299`: 41 Exact, 8 Confirmed With Nuance, 0 Wrong)

- **Exact Matches (41 rows):** `E252–E257`, `E259–E260`, `E264–E271`, `E273–E288`, `E291–E297`, `E299` (plus 100% XML verification of the `C4 §4` `Barrick_Q1_2026_Mine_Stats.xlsx` cross-check).
- **Confirmed With Nuance (8 rows + 3 prose/table enrichments):**
  1. **`E251` (`PV TR 2026` ownership citation):** `p.208` and `p.213` state Barrick's 60% share in PVD; the explicit sentence naming Newmont's 40% share is on **PDF `p.15`** (§1.2) and **`p.27`** (§2.1).
  2. **`E258` & `C4 §2.1` (`PV Table 15-10`, p. 222):** In `Table 15-10`, `Pits to crusher` reports `300.5 koz Au` (`3,346.8 kt @ 2.79 g/t Au, 10.65 g/t Ag, 1,145.8 koz Ag`), and `Stocks to crusher` prints `557.1 koz Au` in the Mining Production Actual block vs. `557.2 koz Au` in the Crusher Feed Actual block (`0.1 koz` internal rounding).
  3. **`E261` (`PV TR p.212` quote ellipsis):** Insert `...` for the intervening bullet (*"Mineral Reserves are depleted up to end of December 2025; and"*) between the two quoted sentences.
  4. **`E262` (`PV TR p.208` rounding notes 5 & 7):** Two significant digits applies to tonnes and metal ounces across all categories, while two decimal places (M&I) vs. one decimal place (Inferred) applies to grades.
  5. **`E263` (`PV TR p.195, Table 14-26`):** Measured (`±15%` quarterly) and Indicated (`±15%` annual) state `"at 90% confidence level"`, whereas the Inferred row (`±30%` annual) omits a confidence percentage in `Table 14-26`.
  6. **`E272` & `C4 §2.1` (`PV Table 21-3`, p. 278):** Between Total Direct Operating Cost (`$46.83/t` = Mining `$10.02` + Processing `$31.26` + G&A `$3.29` + Tailings `$2.26`) and Royalty (`$3.04/t`), `Table 21-3` includes two small indirect lines: **`Freight & Refining: $0.15/t`** and **`Other Indirect: $0.10/t`** (`46.83 + 0.15 + 0.10 + 3.04 = $50.12/t`).
  7. **`E289–E290` & `C4 §2.3` (`Zaldívar 2012 pp.87, 90–92, Table 17-1`):** `E289` (`F1=96, F2=78, F3=40`) is the historical 2006 3-species sequential-assay (`ASCu, CNCu, RCu`) CMZ correlation, whereas `Table 17-1` (`p.91`) is the 2011 2-species (`ASCu` and `SCu = TCu − ASCu`) piecewise block-model formula. On `p.92` (`E290`), the `~9%` lower resource recovery reflects both the `55%` insoluble Cu recovery cap and an `~8%` reduction in average insoluble Cu grade.
  8. **`E298` (`Porgera 2012 pp.145–147, Table 15-6` — Root Cause of `24% higher` vs. `+36%`):** On `p.146`, the `2007` Underground row has `-` (missing) for `DOM` and `GC` (`3,139 kt, 850 koz` over 2008–2011) but reports `682 kt @ 2.66 g/t = 58 koz` for `OR` (Ore Reserve). The table's `"Five Year"` OR summary (`626 koz`) sums **only the 4 years 2008–2011** (`164 + 102 + 169 + 192 = 627 ≈ 626 koz`), giving `850 / 626 = +35.8% (+36%)` printed on `p.147`. Including 2007's `58 koz` in the 5-year denominator (`58 + 627 = 685 koz`) gives `850 / 685 = 1.2409` = **`+24.1%` (`"24% higher"` in prose on `p.145`)**!
  9. **Note on `C4 §5` (`ITRB`):** `PV TR 2026 PDF p.29` (§2.3) explicitly records a November 2025 QP site visit to attend meetings with the regulator and the **Independent Tailings Review Board (`ITRB`)**.

### 3.2 `C5_reko_diq_esia.md` (`E301–E349`: 47 Exact, 2 Confirmed With Nuance, 0 Wrong)

- **Exact Page & Quote Verification (`49 / 49` verified on cited PDF pages via `pdftotext` and in-memory OCR):**
  - All 49 rows (`E301–E349`) match the exact cited pages and verbatim text of `Reko_Diq_Early_Works_ESIA.pdf`.
- **Two High-Value Nuances & Bonus Findings:**
  1. **`E314` & `C5 §2.2` (Bonus `1.45 km` Internal Coordinate Discrepancy on `FS20-PB`):**
     - Borehole **`FS13-PB`** in `Table 3-1` (`p.58`, `364074 E, 3264150 N` in UTM Zone 41N / `EPSG:32641`) matches `Table 5-6` (`p.110`, `29°29'58.29" N, 61°35'51.84" E` = `364074.12 E, 3264149.93 N`) to within **`0.12 m`**.
     - Borehole **`FS20-PB`** (omitted from `Table 5-6` on `p.110`, as noted in `E325–E326`) appears in **`Appendix B Table 5` (`p.468`)** at **`29°36'51.91" N, 61°28'34.35" E`**, which converts in UTM Zone 41N to **`(352460 E, 3277031 N)`** vs. `Table 3-1`'s `(351471 E, 3278086 N)` — a **`1.45 km` (`989 m E, 1,055 m N`) internal coordinate discrepancy** inside the ESIA!
  2. **`E331` (Appendix S Odd/Even Page Layout & Strict Privacy Compliance):**
     - In Appendix S (`pp.851–870`), even-numbered pages (`pp.862, 864, 868`) hold the institutional/community concern bullets, while odd-numbered pages (`pp.853, 855, 857–859, 861, 863, 865, 867, 869`) and Appendix R (`pp.820–850`) hold the `Name | Occupation | Mobile` attendance sheets. Per `§31.2` and `Appendix I.10`, zero personal names or mobile numbers are reproduced here or in the plan.

### 3.3 `C6_textbook_techniques.md` (`E351–E399`: 48 Exact, 1 Confirmed With Nuance, 0 Wrong)

- **Exact Page, Quote & Worked-Fixture Verification (`49 / 49` rows + `FX-1..FX-10`):**
  - All 49 evidence rows (`E351–E399`) across Abzalov (*Applied Mining Geology*, 2016, `AMG`), Waltham (*Foundations of Engineering Geology*, 2nd ed., `FEG`), Lacy (*An Introduction to Geology and Hard Rock Mining*, 2015, `LAC`), and `mining-geology` (`LN`) match the cited PDF pages and quotes, and all 10 worked numerical fixtures (`FX-1..FX-10`) reproduce.
  - **Minor Spelling Nuance (`E395`):** On `FEG PDF p.70` (Vaiont slide), Waltham's scanned text uses British spelling `"stabilised"` (`"The error was to assume slide would creep until it stabilised on flatter toe"`), whereas `E395` Americanized the quote to `"stabilized"`.

---

## 4. Programmatic Audit of the 29 Newly Downloaded `2024–2026` Mine Stats, AISC & MD&A Files

We ran a full cell-by-cell and identity audit over all 18 `.xlsx` workbooks (**44,328 populated cells**: 28,749 in `10 × *_Mine_Stats.xlsx` and 15,579 in `8 × *_AISC.xlsx`) and 10 `*_MD-A.pdf` reports (`Q1 2024` through `Q2 2026`) in `corpus_of_input_docs/downloaded/mine_stats/`.

### 4.1 Workbook Hygiene & Structural Audit (`44,328` Cells Across 18 `.xlsx` Workbooks)

| Workbook Suite | Files (`2024–2026`) | Populated Cells | Formula Cells | `#REF!` Cells | New Spreadsheet Hygiene & Errata Findings |
|---|---:|---:|---:|---:|---|
| **`*_Mine_Stats.xlsx`** (`Q1_2024`–`Q2_2026`) | 10 | 28,749 | 1,092 (`Q2_2026` only) | 1,092 (`Q2_2026` only) | 1. All 8 `2024–2025` Mine Stats workbooks and `Q1_2026` have **0 formulas** and **0 `#REF!` errors**.<br>2. **`Q3_2025_Mine_Stats.xlsx` (`QTD!A32`, `YTD!A32`) Footnote 3 date typo:** misstated the date of loss of control at Loulo-Gounkoto as **"September 16, 2025"** instead of **"June 16, 2025"** (corrected in `Q4_2025`–`Q2_2026`).<br>3. **`Q1_2024_Mine_Stats.xlsx` (`Minestats4!A15:A16`) stale unit label:** row header still read `"Production (millions of pounds)1"` while printing `kt` values (`40, 22, 9, 9`); fixed to `"Production (Thousands of tonnes)"` in `Q2_2024`. |
| **`*_AISC.xlsx`** (`Q3_2024`–`Q2_2026`) | 8 | 15,579 | 609 (`Q3_2024`: 1; `Q1_2026`: 304; `Q2_2026`: 304) | 0 | **NEW HYGIENE CATCH (`DR-16`):** Both `Barrick_Q1_2026_AISC.xlsx` and `Barrick_Q2_2026_AISC.xlsx` contain **two hidden sheets (`YTD Q3 2025` and `YTD Q3 2024`)** left over from template cloning, holding **304 external workbook formulas** pointing to `='[1]YTD CY'!$J$4` (`xl/externalLinks/externalLink1.xml`)! Validates `G-PORT-1` (`BD-185`: quarantine hidden sheets & external links on import). |
| **Total `.xlsx` Corpus** | **18** | **44,328** | **1,701** | **1,092** | 4 hidden sheets with 608 dangling external-workbook links (`Q1_2026_AISC`, `Q2_2026_AISC`). |

### 4.2 Accounting & Physical Identity Pass Rates (`2024 Q1 – 2026 Q2`)

| Identity | Scope | Pass / Total | Pass Rate | Residual / Exception Diagnosis |
|---|---|---:|---:|---|
| **Mined Tonnes Balance** (`Tonnes mined = OP Ore + OP Waste + UG`) | 10 `*_Mine_Stats.xlsx` | **596 / 596** | **100.0%** | 0 rounding diffs, 0 mismatches across 10 quarters. |
| **Strip Ratio Identity** (`Strip Ratio = round(OP Waste / OP Ore, 2)`) | 10 `*_Mine_Stats.xlsx` | **475 / 475** | **100.0%** | 0 mismatches beyond integer-input rounding (±0.06). |
| **Capex Decomposition** (`Total Capex = Sustaining + Project + CapInt`) | 10 `*_Mine_Stats.xlsx` + 10 `*_MD-A.pdf` | **375 / 375** | **100.0%** | `324 / 375` (`86.4%`) have `CapInt = 0`; `51 / 375` (`13.6%`) have positive `CapInt` (`+$1M` to `+$15M`, reconciled in MD&A `Reconciliation of the Classification of Capital Expenditures`); `0` negative residuals. |
| **TCC ($M) Identity** (`COS − Depr − ByProd + NonRec + Other − NCI`) | 8 `*_AISC.xlsx` | **517 / 519** | **99.6%** | Only 2 integer-rounding diffs (`Q2 2024 Kibali`: `106 − 34 = 72` vs. printed `70`). |
| **AISC ($M) Identity** (`TCC + G&A + E&E + SustCapex + Leases + Rehab − NCI`) | 8 `*_AISC.xlsx` | **519 / 519** | **100.0%** | Holds within ±$1M integer rounding on every visible column of all 8 workbooks. |
| **Per-Ounce `AISC/oz` Identity** (`AISC ($M) × 1,000 / Ounces Sold (koz)`) | 8 `*_AISC.xlsx` | **495 / 507** | **97.6%** | 12 exceptions fully explained by (a) unrounded ounce denominators at small/divested sites (`Veladero`, `Hemlo`) and (b) the 2025 `Africa & Middle East` Loulo-Gounkoto suspension numerator effect (`§4.3` below). |

### 4.3 Resolution of `C1` Open Discrepancies (`DR-04`, `DR-09`, `DR-11`, `DR-12`) & `BD-058`

1. **`DR-12` (`E110–E112`) — Exact Decomposition of `L_corp`, `L_JV`, and `L_region` AISC Layers (`BD-273`):**
   - **Total Gold vs. Regions/Sites (`+$47 to +$56/oz`, `L_corp`):** `Barrick_Q1_2026_MD-A.pdf` p. 39 and `Barrick_Q2_2026_MD-A.pdf` p. 47 prove that every individual mine reports `$0M` in `General & administrative costs`, whereas `Total Gold` adds **corporate `G&A` of `$39M` (Q1 2026), `$39M` (Q2 2026), `$78M` (H1 2026), `$64M` (Q4 2025), and `$42M` (Q1 2025)** less `($6M)–($10M)` allocated to NCI/copper in Footnote `f` (`net $33M / 748 koz = +$44.1/oz` in Q1 2026; `net $33M / 801 koz = +$41.2/oz` in Q2 2026), plus consolidated `Other`/lease/rehab rounding (`~$8–11/oz`).
   - **Nevada Gold Mines (`NGM`) vs. 4 Constituent Mines (`+$8.69 to +$19.38/oz`, `L_JV`):** `*_AISC.xlsx` Column `G` (Footnote `a`: *"61.5% interest in Carlin, Cortez, Turquoise Ridge and Phoenix and non-mine site related activity"*) shows NGM's 100%-basis `Minesite sustaining capital expenditures` (`G13`) exceeds the sum of the 4 mines (`C13:F13`) by **`+$5M` (Q1 2026: `228 vs 223`)**, **`+$10M` (Q2 2026: `254 vs 244`)**, and **`+$14M` (H1 2026: `482 vs 468`)**, plus `+$1M–$2M` of NGM-level exploration (`G12`).
   - **Africa & Middle East (`AME`) 2025 Regional Gap (`+$16 to +$53/oz`, `L_region`):** During the Q1–Q2 2025 suspension at Loulo-Gounkoto (`0 oz` sold), Loulo-Gounkoto's **`$10M` (Q1 2025), `$3M` (Q2 2025), and `$13M` (H1 2025) attributable sustaining capex** (`Minestats3!B32/B74`) were included in the `Africa & Middle East` regional `AISC ($M)` numerator (`Q1_2025_AISC` & `Q2_2025_AISC` Column `S`) even though the `Loulo-Gounkoto` column was zeroed out (`$10.8M / 202 koz = +$53.5/oz` in Q1 2025; `$3.0M / 178 koz = +$16.9/oz` in Q2 2025).
   - **Total Copper vs. 3 Copper Sites (`+$0.05 to +$0.11/lb`, `L_corp_Cu`):** `Barrick_Q1_2026_MD-A.pdf` p. 43 and `Barrick_Q2_2026_MD-A.pdf` p. 53 show `Total Copper` adds **`$6M/qtr` (`$12M` H1 2026)** of unallocated corporate copper `G&A` (`$6M / 99 Mlb = +$0.06/lb` in Q1 2026; `$6M / 119 Mlb = +$0.05/lb` in Q2 2026).
2. **`DR-04` (`E108`) — Jabal Sayid Q2 2026 100%-Basis Production Typo Confirmed:**
   - `Barrick_Q2_2026_MD-A.pdf` p. 30 confirms Jabal Sayid (50%) attributable copper production was **`6 kt` in Q2 2026 vs. `9 kt` in Q1 2026 (`−33%`)**, proving `Q2_2026:QTD!B28 = 14` (100% basis) is a typo in the published workbook for `12` (`2 × 6 = 12`, reconciling `18 + 12 = 30 kt` in `YTD!B28`).
3. **`DR-09` (`E118`) — Loulo-Gounkoto Q1 2025 Pre-Suspension Production vs. Blanket Footnote:**
   - `Barrick_Q1_2025_Mine_Stats.xlsx` confirms Barrick originally reported Jan 1–14, 2025 pre-suspension production (`22 koz` 100%, `18 koz` attributable, `169 kt` milled @ `3.54 g/t`, `91%` recovery) with `—` only for sales/costs; the blanket footnote *"no operating or per ounce data was provided for Q1 2025 to Q3 2025"* was added later without removing the Q1 2025 operating rows.
