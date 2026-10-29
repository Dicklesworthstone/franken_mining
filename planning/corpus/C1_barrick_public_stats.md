# C1: Barrick public operating data (Mine Stats Q1/Q2 2026 and four site decks)

Agent C1 (evidence block E101–E149), read-only pass, 2026-10-28. Arithmetic was run with `python3 -I` in the
session scratchpad. Every xlsx value below was read with openpyxl from the cell named. Every deck value was checked
with `pdftotext -layout -f N -l N`. Chart-only values were also checked on a 90 dpi render of the page.

**Ownership basis (applies everywhere).**
- The summary sheets (`QTD`, `YTD`) carry one "100% basis" production column. Everything else on them is at
  **Barrick's attributable share**.
- The site sheets (`Minestats1–4`) are **attributable share for every physical and money quantity**: tonnes,
  ounces, pounds and capex (INFERENCE proved in §3.2, E109).
- Per-unit costs are **per attributable ounce (or pound) sold**.
- **Nevada Gold Mines** (Carlin, Cortez, Turquoise Ridge, Phoenix) figures are Barrick's **61.5%** of an NGM JV
  in which Newmont holds 38.5%. This is JV-sensitive in the plan (E034, F13). Nothing here uses partner-only data.
- **Other non-100% sites:**
  - Pueblo Viejo 60%, Veladero 50%, Porgera 24.5%;
  - Loulo-Gounkoto 80%, Kibali 45%, Tongon 89.7% (sold 1 Dec 2025);
  - North Mara and Bulyanhulu 84% (16% Government of Tanzania through Twiga);
  - Zaldívar 50%, Jabal Sayid 50%.
- **100% sites:** Hemlo (sold 26 Nov 2025) and Lumwana. Lumwana's 100% basis is shown by 100% = attributable in
  every period, since its label prints no percentage.
- Deck figures carry their own basis tag in every row.

**File ids used in locators.**

| file_id | File (corpus_of_input_docs/) | sha256 (first 16) |
|---|---|---|
| `Q1_2026` | Barrick_Q1_2026_Mine_Stats.xlsx | ddef08dd544f0df5 |
| `Q2_2026` | Barrick_Q2_2026_Mine_Stats.xlsx | cf0d3f4fea9c96f9 |
| `LUM` | Lumwana_Site_Visit_25092025.pdf (43 pp.) | eca7a1026ae3e6ce |
| `TZ` | Tanzania_Q2_2025_Media_Day.pdf (30 pp.) | a3b499be3cef5fce |
| `FM` | Fourmile_Feb2026.pdf (15 pp.) | 89c22cd73d2fcc94 |
| `MA21` | Mine-Africa-2021-Tanzania.pdf (12 pp.) | 3c59b1affaebb382 |

---

## 1. RESULT_STATUS: MATERIAL_INCREMENT

---

## 2. Inventory

| Document | Range | Content | Extraction usability |
|---|---|---|---|
| `Q1_2026` | sheets `QTD` (A1:Q42), `Minestats1` (A1:O61), `Minestats2` (A1:F49), `Minestats3` (A1:L43), `Minestats4` (A1:M35), `Endnote` (A1:E16) | Three months to 31 Mar 2026, with the 2025 comparative. `QTD` holds per-site production (100% and attributable), sales, CoS, TCC/C1 and AISC, plus regional and total rows. `Minestats1–4` hold per-site physicals, unit costs, grade, recovery, per-oz cost build and capex. `Endnote` holds the QPs and Endnotes #1–#4. No YTD sheet (Q1 = YTD). | **High.** Values are hard-coded (no formulas). Stored precision equals displayed precision in 3,246/3,246 cells. Read the xlsx directly. |
| `Q2_2026` | sheets `QTD` (A1:N42); `YTD` (declared A1:WMQ42, real data A1:N42); `Minestats1` (A1:O126: three-month block rows 2–57, six-month block rows 64–120); `Minestats2` (A1:F101: 2–46 / 52–97); `Minestats3` (A1:V90: 2–41 / 46–86); `Minestats4` (A1:R74: 2–32 / 38–69); `Endnote` | Three and six months to 30 Jun 2026, with 2025 comparatives. Adds a **"Revenue ($ millions)" row** to every Minestats block (absent in Q1). | **High after hygiene.** About 60 empty merged ranges in `YTD` row 39 run out to column WMQ (15,903 columns). **1,092 broken formula cells** (`=B4-#REF!` style) sit in `Minestats3!M4:V81` (660) and `Minestats4!K4:R67` (432) and evaluate to `#REF!`/`#VALUE!`. They are outside the data columns and must be excluded. |
| `markdown_extractions/Barrick_Q2_2026_Mine_Stats.md` | whole file | pdf2md mirror of the Q2 xlsx | **Do not use for M0.** It renders `YTD` as 15,903 columns (37 lines over 5,000 characters) and carries 120 `#REF!` strings. Recovery and Cu grade appear as raw fractions (0.81, 0.0054) with the `%` format lost. It has no cell addresses. |
| `LUM` | pp.1–2 cover and cautionary; 3–5 Barrick strategy; 6–9 overview, safety, milestones, **p.9 H1 2025 actuals and 2025 guidance (100%)**; 10–11 mining and drill-and-blast (p.10 charts unlabelled; p.11 monthly drilling accuracy); 12–14 Zambia economics; 15–25 expansion (**p.19 geology and reconciliation claim**, **p.23 Super Pit economics**, pp.24–25 charts); 26–31 execution (**p.28 milestones**, pp.29–30 power); 32–34 community and REDD+; 35–37 exploration; 38 contacts; **p.39 Appendix A production 2009–2024**; p.40 outlook assumptions; **pp.41–42 endnotes (Tier One definitions, reserve statement)**; p.43 QPs | Site-visit deck, 25 Sep 2025 | **High** for text and tables (p.39 table extracts exactly and sums exactly). Charts on pp.10, 24, 25 have no data labels. |
| `TZ` | pp.1–7 intro, Q2 2025 highlights, safety, malaria; **p.8 North Mara OP mining and throughput, Q1 2024–Q1 2025 (100%)**; p.9 NM pipeline; **p.10 Bulyanhulu UG mining, ore/waste and development (100%)**; p.11 Bulyanhulu Inlier; p.12 environment (NM water); pp.13–26 economic contribution, local content, CSR; p.27 QPs; p.28 assumptions (GEO basis); **pp.29–30 NM and Bulyanhulu reserve histories 2019–2023 and Twiga 16%** | Media day deck, Q2 2025 | **Medium-high.** Chart values are labelled and text-extractable. The p.8 axis is in tonnes while the title says kt. |
| `FM` | pp.1–3 intro and location; **p.4 timeline and 2025 resource**; **p.5 resource 2024 → 2025**; **p.6 PFS metallurgy and drill spacing**; p.7 blank; **p.8 intercepts table**; p.9 2026 assumptions; **p.10 QPs and PEA basis, NGM fair-market-value transfer**; p.11 non-GAAP endnotes; **p.12 NGM reserve/resource table**; **p.13 CoS definition and GEO conversion factors 2019–2025**; **pp.14–15 reserve roll-forward 2019–2025 (gold, copper, GEO)** | Fourmile investor deck, Feb 2026 | **High** (text tables). The row label "2025g" extracts as "20256" in the copper and GEO tables. |
| `MA21` | pp.1–4 intro and timeline (p.3 Twiga 2019); p.5 UG ore tonnes (labels hidden in the render); p.6 investment; **p.7 2020 quarterly gold production (100%)**; p.8 NM water treatment; pp.9–12 workforce, investment, CSR | Mining Indaba-style deck, 2021 | **Low value** for M0 (2020 data only). |

---

## 3. Demonstrator data

### 3.1 Locator scheme and import rules (for `fmine-public`, BD-055/BD-057/BD-059)

- **Locator** = `<file_id>:<sheet>!<cell>` plus the file's sha256, e.g. `Q2_2026:QTD!G27` = Lumwana AISC, three
  months to 30 Jun 2026, 3.83 $/lb.
  - The xlsx is not itself an EDGAR filing. The Endnote cites the filed report that carries the reconciliations:
    "starting on page 36 in the First Quarter 2026 Report" and "page 44 in the Second Quarter 2026 Report", both on
    SEDAR+ and EDGAR.
  - A Claim should carry both the xlsx locator and the filing page reference (E107).
- **Anchor rows on labels, not row numbers.** Q2 inserts "Revenue ($ millions)" at `Minestats1!A38`, which moves
  CoS from row 38 (Q1) to row 40 (Q2). The same shift happens on every Minestats sheet.
- **Strip footnote digits from labels** with `^(.*?[^\d\s])(\d{1,2})$`, e.g. "Hemlo3", "Total Gold7". Footnote
  numbers differ by sheet for the same metric: "Total capex" is footnote 6 on MS1, 3 on MS2, 5 on MS3 and 2 on MS4.
- **Parse ownership from the label** with `\((\d+(?:\.\d+)?)%\)`. When no percentage is printed (Lumwana, Hemlo,
  regions, totals), record `own = unprinted` and do not default to 100%.
- **Period blocks:**
  - In Minestats, a block starts at a row reading "For the three months ended …" or "For the six months ended …".
    The site header row sits directly above it as merged column pairs (first column = current year, second = prior
    year).
  - In `QTD`/`YTD`, row 3 holds the year header for gold and row 24 for copper. Columns B–G are the current year
    and I–N the prior year.
- **Value tokens** (both workbooks, parsed metric rows):

  | Token | Count | Meaning | Import as |
  |---|---|---|---|
  | `'—'` | 223 | nil **or** not reported (overloaded) | `Nil{reason: unknown}` |
  | `'— '` | 49 | same, trailing space (summary sheets) | `Nil{reason: unknown}` |
  | `'n/a'` | 450 | not applicable (e.g. no open pit; heap leach has no recovery) | `NotApplicable` |
  | `None` | 2 | blank (`Q2_2026:Minestats3!F23`, `F68`, Tongon revenue) | `Missing` |
  | numeric 0 shown `0%` | 7 | placeholder for a divested site (§3.6, DR-07) | `Nil`, never 0 |

- **Units as stored:**
  - Recovery is a fraction (0.82) with format `0%`.
  - Copper grade is a fraction (0.0059) with format `0.00%`.
  - Gold grade is g/t with format `0.00`.
  - Strip ratio uses `0.00`, except 26 cells with `General` format, for example `Q1_2026:Minestats3!H8` = 514.14.
  - Tonnes are thousands of tonnes; gold is 000s oz; copper is thousand tonnes on the summary sheets and in
    Minestats4.
  - Unit costs are $/oz or $/lb; capex and revenue are $ millions.
- **Exclusions:**
  - `Q2_2026:YTD` columns beyond N;
  - `Q2_2026:Minestats3!M:V` and `Q2_2026:Minestats4!K:R` (formula residue);
  - footnote rows. Detect footnote rows as column-A text starting with a digit and a space.

### 3.2 Field dictionary

**Summary sheets** (`Q1_2026:QTD`, `Q2_2026:QTD`, `Q2_2026:YTD`):
- Gold sites are in rows 5–11, 13–14 and 16–20. Regions are rows 12 (North America), 15 (South America & Asia
  Pacific) and 21 (Africa & Middle East); row 22 is Total Gold.
- Copper sites are rows 26 (Zaldívar), 27 (Lumwana) and 28 (Jabal Sayid); row 29 is Total Copper.
- Per-site gold rows: Carlin 5, Cortez 6, Turquoise Ridge 7, Phoenix 8, NGM 9, Pueblo Viejo 10, Hemlo 11,
  Veladero 13, Porgera 14, Loulo-Gounkoto 16, Kibali 17, Tongon 18, North Mara 19, Bulyanhulu 20.

| field_id | Col (cur / prior) | Label as printed | Unit | Basis |
|---|---|---|---|---|
| `au_prod_100_koz` / `cu_prod_100_kt` | B / I | "Gold produced (000s ozs 100% basis)" / "Copper produced (kt 100% basis)" | koz / kt | **100%** |
| `au_prod_attr_koz` / `cu_prod_attr_kt` | C / J | "Gold produced (000s ozs attributable share)" / "Copper produced (kt attributable share)" | koz / kt | attributable |
| `au_sold_attr_koz` / `cu_sold_attr_kt` | D / K | "Gold sold (000s ozs attributable share)" / "Copper sold (kt attributable share)" | koz / kt | attributable |
| `cos_usd_per_oz` / `cos_usd_per_lb` | E / L | "Cost of sales ($/oz)" / "($/lb)" | $/oz, $/lb | per attributable unit **sold** |
| `tcc_usd_per_oz` / `c1_usd_per_lb` | F / M | "Total cash costs ($/oz)1" / "C1 cash costs ($/lb)8" | $/oz, $/lb | per unit sold; non-GAAP, Endnote #1 / #2 |
| `aisc_usd_per_oz` / `aisc_usd_per_lb` | G / N | "All-In sustaining costs ($/oz)1" / "All-In sustaining costs ($/lb)8" | $/oz, $/lb | per unit sold; non-GAAP, Endnote #1 / #2 |

**Site sheets** (Minestats1 = NGM mines, NGM, PV, Hemlo; Minestats2 = Veladero, Porgera; Minestats3 =
Loulo-Gounkoto, Kibali, Tongon, North Mara, Bulyanhulu; Minestats4 = Copper Total, Lumwana, Zaldívar, Jabal
Sayid). All quantities are at attributable share.

| field_id | Label (column A) | Unit | Present on | Note |
|---|---|---|---|---|
| `mined_total_kt` | "Tonnes mined (thousands)" | kt | MS1–4 | = OP ore + OP waste + UG (T6: 101/101) |
| `mined_op_ore_kt`, `mined_op_waste_kt`, `mined_ug_kt` | "Open Pit Ore", "Open Pit Waste", "Underground" | kt | MS1–4 | |
| `strip_ratio` | "Strip Ratio" | waste:ore | MS1–4 | = waste/ore (T7: 82/82) |
| `cost_per_t_mined_op_usd`, `cost_per_t_mined_ug_usd` | "Open Pit", "Underground" under "Cost per tonne mined ($/tonne)3" | $/t | MS1–3 | Endnote #4: "before capitalization of waste stripping and allocation to inventory" |
| `processed_total_kt` and circuits `_oxide_mill`, `_roaster`, `_autoclave`, `_heap_leach` | "Tonnes processed (thousands)" + "Oxide Mill", "Roasters", "Autoclave", "Heap Leach" | kt | total MS1–4; circuits MS1–2 | = sum of circuits (T6b: 51/51) |
| `cost_per_t_processed[_circuit]_usd`, `ga_cost_per_t_processed_usd` | "Cost per tonne processed ($/tonne)3" (+ circuits), "G&A cost per tonne processed ($/tonne)3" | $/t | MS1–3 | Endnote #4 |
| `grade_op_mined_gpt`, `grade_ug_mined_gpt`, `grade_processed_gpt` | "Average grade (grams per tonne)4" → "Open Pit mined", "Underground mined", "Processed" | g/t | MS1–3 | Phoenix grade "relate[s] to material slated for processing at the oxide mill" (`Minestats1!A55`) |
| `grade_*_frac` | "Average grade" → "Open Pit mined", "Underground mined", "Processed" | fraction (format `0.00%`) | MS4 | copper |
| `recovery_frac`, `recovery_{oxide_mill,roaster,autoclave}_frac` | "Recovery rate (percent)" | fraction (format `0%`) | MS1–4 | heap leach and Zaldívar = `n/a` |
| `au_prod_attr_koz`, `au_prod_{circuit}_koz` | "Production (thousands of ounces)" + circuits | koz | MS1–3 | = summary column C (T4) |
| `cu_prod_attr_kt`, `cu_sold_attr_kt` | "Production (Thousands of tonnes)", "Sales (Thousands of tonnes)" | kt | MS4 | |
| `au_sold_attr_koz` | "Sales (thousands of ounces)" | koz | MS1–3 | |
| `revenue_usd_m` | "Revenue ($ millions)" | $M | **Q2 file only** | includes by-product revenue (Phoenix 2026Q2 $151M on 27 koz sold = $5,593/oz) |
| `cos_usd_per_oz`, `royalties_usd_per_oz`, `depreciation_usd_per_oz`, `tcc_usd_per_oz`, `aisc_usd_per_oz` | "Cost of sales per ounce ($/oz)"; under "Costs per ounce ($/oz)": "Royalties", "Depreciation", "Total cash costs5", "All-in sustaining costs5" | $/oz sold | MS1–3 | `royalties_usd_per_oz` is per-oz only; no G&A, lease, exploration or reclamation rows |
| `cos_usd_per_lb`, `c1_usd_per_lb`, `depreciation_usd_per_lb`, `aisc_usd_per_lb` | "Cost of sales per pound ($/lb)"; under "Costs per pound": "C1 cash costs1", "Depreciation", "All-in sustaining costs1" | $/lb sold | MS4 | |
| `capex_sustaining_usd_m`, `capex_project_usd_m`, `capex_total_usd_m` | "Minesite sustaining5", "Projects5", "Total capital expenditures ($ millions)6" (MS1 footnote numbers) | $M | MS1–4 | "presented on a cash basis and … at Barrick's share. Includes capitalized interest where applicable." total ≥ sust + project (§3.6 DR-11) |

**Basis proofs.**
- **Minestats physicals are attributable** (E109). North Mara Q1 2025 OP ore + waste
  (`Q1_2026:Minestats3!I5` 1,057 + `I6` 2,398) / 0.84 = 4,113.1 kt, against the 100%-basis chart value 4,112 kt
  (`TZ` p.8). Throughput `I12` 672 / 0.84 = 800.0 against 800 (`TZ` p.8). Bulyanhulu UG `K7` 304 / 0.84 = 361.9
  against 362 (`TZ` p.10). NGM-mine production on MS1 equals summary column C (attributable), not B.
- **Per-unit costs are per unit sold** (E110). Sold-weighting the sites reproduces every regional CoS, TCC and AISC
  in 2026 within 0.1% (T3). `FM` p.13 endnote 10 defines gold CoS/oz as cost of sales "divided by ounces sold (both on an attributable basis using Barrick's ownership share)".

**Endnote definitions** (`Q2_2026:Endnote`; Q1 is identical except for the page reference):
- **#1, TCC and AISC (A5).** TCC starts from cost of sales, removes depreciation and the NCI share, and includes
  by-product credits. AISC adds minesite sustaining capex, sustaining leases, G&A, minesite exploration and
  evaluation, and reclamation accretion and amortization. Reconciliation is in the Q1 2026 Report from p.36 and
  the Q2 2026 Report from p.44.
- **#2, C1 and copper AISC (A8).** C1 is cost of sales excluding depreciation and royalties and including TC/RCs.
  Copper AISC adds sustaining capex, sustaining leases, G&A **and royalties**.
- **#3, sustaining vs project capex (A11).**
- **#4, cost per tonne (A14).**
- **QPs (A2):**
  - Q1 lists Jesse Clark, Richard Peattie and Joel Holliday.
  - Q2 lists Richard Peattie, Sam Baldwin, Joel Holliday and Jesse Clark.
  - The reserve and resource as-of date is 31 Dec 2025.

### 3.3 Row and column maps (generated from the parsed workbooks)

#### Column map (which column holds which site-period)

| File | Sheet | Period | Site → column |
|---|---|---|---|
| Q1_2026 | Minestats1 | 2026Q1 | Carlin=B; Cortez=D; Turquoise Ridge=F; Phoenix=H; Nevada Gold Mines LLC=J; Pueblo Viejo=L; Hemlo=N |
| Q1_2026 | Minestats1 | 2025Q1 | Carlin=C; Cortez=E; Turquoise Ridge=G; Phoenix=I; Nevada Gold Mines LLC=K; Pueblo Viejo=M; Hemlo=O |
| Q1_2026 | Minestats2 | 2026Q1 | Veladero=B; Porgera=D |
| Q1_2026 | Minestats2 | 2025Q1 | Veladero=C; Porgera=E |
| Q1_2026 | Minestats3 | 2026Q1 | Loulo-Gounkoto=B; Kibali=D; Tongon=F; North Mara=H; Bulyanhulu=J |
| Q1_2026 | Minestats3 | 2025Q1 | Loulo-Gounkoto=C; Kibali=E; Tongon=G; North Mara=I; Bulyanhulu=K |
| Q1_2026 | Minestats4 | 2026Q1 | Copper - Total=B; Lumwana=D; Zaldívar=F; Jabal Sayid=H |
| Q1_2026 | Minestats4 | 2025Q1 | Copper - Total=C; Lumwana=E; Zaldívar=G; Jabal Sayid=I |
| Q2_2026 | Minestats1 | 2026Q2 | Carlin=B; Cortez=D; Turquoise Ridge=F; Phoenix=H; Nevada Gold Mines LLC=J; Pueblo Viejo=L; Hemlo=N |
| Q2_2026 | Minestats1 | 2025Q2 | Carlin=C; Cortez=E; Turquoise Ridge=G; Phoenix=I; Nevada Gold Mines LLC=K; Pueblo Viejo=M; Hemlo=O |
| Q2_2026 | Minestats1 | 2026H1 | Carlin=B; Cortez=D; Turquoise Ridge=F; Phoenix=H; Nevada Gold Mines LLC=J; Pueblo Viejo=L; Hemlo=N |
| Q2_2026 | Minestats1 | 2025H1 | Carlin=C; Cortez=E; Turquoise Ridge=G; Phoenix=I; Nevada Gold Mines LLC=K; Pueblo Viejo=M; Hemlo=O |
| Q2_2026 | Minestats2 | 2026Q2 | Veladero=B; Porgera=D |
| Q2_2026 | Minestats2 | 2025Q2 | Veladero=C; Porgera=E |
| Q2_2026 | Minestats2 | 2026H1 | Veladero=B; Porgera=D |
| Q2_2026 | Minestats2 | 2025H1 | Veladero=C; Porgera=E |
| Q2_2026 | Minestats3 | 2026Q2 | Loulo-Gounkoto=B; Kibali=D; Tongon=F; North Mara=H; Bulyanhulu=J |
| Q2_2026 | Minestats3 | 2025Q2 | Loulo-Gounkoto=C; Kibali=E; Tongon=G; North Mara=I; Bulyanhulu=K |
| Q2_2026 | Minestats3 | 2026H1 | Loulo-Gounkoto=B; Kibali=D; Tongon=F; North Mara=H; Bulyanhulu=J |
| Q2_2026 | Minestats3 | 2025H1 | Loulo-Gounkoto=C; Kibali=E; Tongon=G; North Mara=I; Bulyanhulu=K |
| Q2_2026 | Minestats4 | 2026Q2 | Copper - Total=B; Lumwana=D; Zaldívar=F; Jabal Sayid=H |
| Q2_2026 | Minestats4 | 2025Q2 | Copper - Total=C; Lumwana=E; Zaldívar=G; Jabal Sayid=I |
| Q2_2026 | Minestats4 | 2026H1 | Copper - Total=B; Lumwana=D; Zaldívar=F; Jabal Sayid=H |
| Q2_2026 | Minestats4 | 2025H1 | Copper - Total=C; Lumwana=E; Zaldívar=G; Jabal Sayid=I |

Summary sheets (QTD/YTD, both files): rows 5–22 gold sites/regions, rows 26–29 copper; columns B–G = current year (B 100% production, C attributable production, D attributable sold, E CoS/unit, F TCC or C1/unit, G AISC/unit), I–N = prior-year comparative in the same order.

#### Row map (metric → row) per sheet block

| field_id | Q1 MS1 three | Q1 MS2 three | Q1 MS3 three | Q1 MS4 three | Q2 MS1 three | Q2 MS1 six | Q2 MS2 three | Q2 MS2 six | Q2 MS3 three | Q2 MS3 six | Q2 MS4 three | Q2 MS4 six |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| mined_total_kt | 4 | 4 | 4 | 4 | 4 | 67 | 4 | 55 | 4 | 49 | 4 | 41 |
| mined_op_ore_kt | 5 | 5 | 5 | 5 | 5 | 68 | 5 | 56 | 5 | 50 | 5 | 42 |
| mined_op_waste_kt | 6 | 6 | 6 | 6 | 6 | 69 | 6 | 57 | 6 | 51 | 6 | 43 |
| mined_ug_kt | 7 | 7 | 7 | 7 | 7 | 70 | 7 | 58 | 7 | 52 | 7 | 44 |
| strip_ratio | 8 | 8 | 8 | 8 | 8 | 71 | 8 | 59 | 8 | 53 | 8 | 45 |
| cost_per_t_mined_op_usd | 10 | 10 | 10 |  | 10 | 73 | 10 | 61 | 10 | 55 |  |  |
| cost_per_t_mined_ug_usd | 11 | 11 | 11 |  | 11 | 74 | 11 | 62 | 11 | 56 |  |  |
| processed_total_kt | 12 | 12 | 12 | 9 | 12 | 75 | 12 | 63 | 12 | 57 | 9 | 46 |
| processed_oxide_mill_kt | 13 |  |  |  | 13 | 76 |  |  |  |  |  |  |
| processed_roaster_kt | 14 |  |  |  | 14 | 77 |  |  |  |  |  |  |
| processed_autoclave_kt | 15 | 13 |  |  | 15 | 78 | 13 | 64 |  |  |  |  |
| processed_heap_leach_kt | 16 | 14 |  |  | 16 | 79 | 14 | 65 |  |  |  |  |
| cost_per_t_processed_oxide_mill_usd | 18 |  |  |  | 18 | 81 |  |  |  |  |  |  |
| cost_per_t_processed_roaster_usd | 19 |  |  |  | 19 | 82 |  |  |  |  |  |  |
| cost_per_t_processed_autoclave_usd | 20 | 16 |  |  | 20 | 83 | 16 | 67 |  |  |  |  |
| cost_per_t_processed_heap_leach_usd | 21 | 17 |  |  | 21 | 84 | 17 | 68 |  |  |  |  |
| ga_cost_per_t_processed_usd | 22 | 18 | 14 |  | 22 | 85 | 18 | 69 | 14 | 59 |  |  |
| grade_op_mined_gpt | 24 | 20 | 16 |  | 24 | 87 | 20 | 71 | 16 | 61 |  |  |
| grade_ug_mined_gpt | 25 | 21 | 17 |  | 25 | 88 | 21 | 72 | 17 | 62 |  |  |
| grade_processed_gpt | 26 | 22 | 18 |  | 26 | 89 | 22 | 73 | 18 | 63 |  |  |
| recovery_frac | 27 | 23 | 19 | 14 | 27 | 90 | 23 | 74 | 19 | 64 | 14 | 51 |
| recovery_oxide_mill_frac | 28 |  |  |  | 28 | 91 |  |  |  |  |  |  |
| recovery_roaster_frac | 29 |  |  |  | 29 | 92 |  |  |  |  |  |  |
| recovery_autoclave_frac | 30 | 24 |  |  | 30 | 93 | 24 | 75 |  |  |  |  |
| au_prod_attr_koz | 31 | 25 | 20 |  | 31 | 94 | 25 | 76 | 20 | 65 |  |  |
| au_prod_oxide_mill_koz | 32 |  |  |  | 32 | 95 |  |  |  |  |  |  |
| au_prod_roaster_koz | 33 |  |  |  | 33 | 96 |  |  |  |  |  |  |
| au_prod_autoclave_koz | 34 | 26 |  |  | 34 | 97 | 26 | 77 |  |  |  |  |
| au_prod_heap_leach_koz | 35 | 27 |  |  | 35 | 98 | 27 | 78 |  |  |  |  |
| au_sold_attr_koz | 36 | 28 | 21 |  | 36 | 99 | 28 | 79 | 21 | 66 |  |  |
| cos_usd_per_oz | 38 | 30 | 23 |  | 40 | 103 | 32 | 83 | 25 | 70 |  |  |
| royalties_usd_per_oz | 41 | 33 | 26 |  | 43 | 106 | 35 | 86 | 28 | 73 |  |  |
| depreciation_usd_per_oz | 42 | 34 | 27 |  | 44 | 107 | 36 | 87 | 29 | 74 |  |  |
| tcc_usd_per_oz | 43 | 35 | 28 |  | 45 | 108 | 37 | 88 | 30 | 75 |  |  |
| aisc_usd_per_oz | 44 | 36 | 29 |  | 46 | 109 | 38 | 89 | 31 | 76 |  |  |
| capex_sustaining_usd_m | 47 | 39 | 32 | 26 | 49 | 112 | 41 | 92 | 34 | 79 | 28 | 65 |
| capex_project_usd_m | 48 | 40 | 33 | 27 | 50 | 113 | 42 | 93 | 35 | 80 | 29 | 66 |
| capex_total_usd_m | 49 | 41 | 34 | 28 | 51 | 114 | 43 | 94 | 36 | 81 | 30 | 67 |
| cost_per_t_processed_usd |  |  | 13 |  |  |  |  |  | 13 | 58 |  |  |
| grade_op_mined_frac |  |  |  | 11 |  |  |  |  |  |  | 11 | 48 |
| grade_ug_mined_frac |  |  |  | 12 |  |  |  |  |  |  | 12 | 49 |
| grade_processed_frac |  |  |  | 13 |  |  |  |  |  |  | 13 | 50 |
| cu_prod_attr_kt |  |  |  | 15 |  |  |  |  |  |  | 15 | 52 |
| cu_sold_attr_kt |  |  |  | 16 |  |  |  |  |  |  | 16 | 53 |
| cos_usd_per_lb |  |  |  | 18 |  |  |  |  |  |  | 20 | 57 |
| c1_usd_per_lb |  |  |  | 21 |  |  |  |  |  |  | 23 | 60 |
| depreciation_usd_per_lb |  |  |  | 22 |  |  |  |  |  |  | 24 | 61 |
| aisc_usd_per_lb |  |  |  | 23 |  |  |  |  |  |  | 25 | 62 |
| revenue_usd_m |  |  |  |  | 38 | 101 | 30 | 81 | 23 | 68 | 18 | 55 |

(Q1 = Q1_2026 file, Q2 = Q2_2026 file; 'three' = three-month block, 'six' = six-month block.)


### 3.4 Per-site tables, verified from xlsx cells

**How to read these tables.**
- **Values are verbatim.** Locators follow from §3.3: summary columns come from `QTD`/`YTD` (columns B–G for the
  current year, I–N for the prior year), and Minestats columns from the column map.
- **Period sources.** 2026Q1 and 2025Q1 come from `Q1_2026`. 2026Q2 and 2025Q2 come from `Q2_2026` (three-month
  blocks). 2026H1 and 2025H1 come from `Q2_2026:YTD` and the six-month blocks.
- **Revenue** is blank for Q1 periods because the Q1 file has no revenue row.
- **Recovery "0%"** for Hemlo and Tongon in 2026 is a numeric-zero placeholder, not a measurement (DR-07).

#### 2026Q1 — gold sites (Barrick attributable basis except the '100%' column)

| Site (Barrick share) | Au prod 100% koz | Au prod attr koz | Au sold attr koz | CoS $/oz | TCC $/oz | AISC $/oz | Mined kt | Strip | Processed kt | Grade proc g/t | Recovery | Royalty $/oz | Dep $/oz | Sust capex $M | Project capex $M | Total capex $M | Revenue $M |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| Carlin (61.5%) | 283 | 173 | 173 | 1794 | 1315 | 1838 | 10054 | 8.63 | 1500 | 4.42 | 82% | 145 | 445 | 87 | 25 | 115 |  |
| Cortez (61.5%) | 145 | 89 | 89 | 2149 | 1569 | 1957 | 14058 | 2.04 | 4425 | 0.87 | 81% | 213 | 556 | 32 | 36 | 68 |  |
| Turquoise Ridge (61.5%) | 148 | 91 | 96 | 1327 | 1011 | 1153 | 887 | 126.95 | 713 | 4.7 | 86% | 1 | 299 | 13 | 4 | 17 |  |
| Phoenix (61.5%) | 36 | 23 | 22 | 2485 | 574 | 862 | 4948 | 2.05 | 2281 | 0.69 | 80% | — | 424 | 5 | — | 5 |  |
| Nevada Gold Mines LLC (61.5%) | 612 | 376 | 380 | 1800 | 1255 | 1645 | 29947 | 3.03 | 8919 | 1.86 | 83% | 116 | 433 | 140 | 65 | 207 |  |
| Pueblo Viejo (60%) | 135 | 81 | 82 | 1702 | 1019 | 1457 | 4365 | 11.3 | 1574 | 2.29 | 74% | 161 | 487 | 35 | 26 | 61 |  |
| Hemlo (100%; sold 26 Nov 2025) | — | — | — | — | — | — | — | n/a | — | — | 0% | — | — | — | — | — |  |
| Veladero (50%) | 110 | 55 | 54 | 1816 | 1037 | 1253 | 7131 | 2.08 | 3227 | 0.55 | n/a | 270 | 661 | 10 | 15 | 25 |  |
| Porgera (24.5%) | 79 | 19 | 22 | 1665 | 1346 | 1740 | 1532 | 2.98 | 314 | 2.12 | 91% | 171 | 300 | 7 | 3 | 10 |  |
| Loulo-Gounkoto (80%) | 80 | 64 | 69 | 2801 | 1918 | 1933 | 390 | n/a | 689 | 3.13 | 92% | 684 | 439 | — | — | — |  |
| Kibali (45%) | 129 | 58 | 69 | 1906 | 1418 | 1614 | 6834 | 8.34 | 910 | 2.24 | 89% | 255 | 473 | 11 | 27 | 38 |  |
| Tongon (89.7%; sold 1 Dec 2025) | — | — | — | — | — | — | — | — | — | — | 0% | — | — | — | — | — |  |
| North Mara (84%) | 39 | 33 | 36 | 2287 | 1758 | 1990 | 3952 | 514.14 | 664 | 1.81 | 85% | 314 | 489 | 7 | 37 | 44 |  |
| Bulyanhulu (84%) | 39 | 33 | 36 | 2008 | 1379 | 1922 | 349 | n/a | 204 | 5.15 | 96% | 286 | 418 | 19 | 17 | 36 |  |
| **North America** | 747 | 457 | 462 | 1783 | 1213 | 1612 |  |  |  |  |  |  |  |  |  |  |  |
| **South America & Asia Pacific** | 189 | 74 | 76 | 1773 | 1126 | 1393 |  |  |  |  |  |  |  |  |  |  |  |
| **Africa & Middle East** | 287 | 188 | 210 | 2281 | 1633 | 1836 |  |  |  |  |  |  |  |  |  |  |  |
| **Total Gold** | 1223 | 719 | 748 | 1922 | 1327 | 1708 |  |  |  |  |  |  |  |  |  |  |  |

#### 2026Q1 — copper sites

| Site (Barrick share) | Cu prod 100% kt | Cu prod attr kt | Cu sold attr kt | CoS $/lb | C1 $/lb | AISC $/lb | Mined kt | Strip | Processed kt | Grade proc % Cu | Recovery | Dep $/lb | Sust capex $M | Project capex $M | Total capex $M | Revenue $M |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| Zaldívar (50%) | 16 | 8 | 8 | 4.87 | 3.7 | 5.02 | 6954 | 1.6 | 2421 | 0.60% | n/a | 1.1 | 20 | 10 | 30 |  |
| Lumwana (100%, not printed) | 32 | 32 | 30 | 3.31 | 2.67 | 3.83 | 31064 | 4.37 | 5911 | 0.59% | 91% | 0.65 | 45 | 104 | 155 |  |
| Jabal Sayid (50%) | 18 | 9 | 7 | 2.07 | 0.79 | 0.94 | 427 | n/a | 384 | 2.36% | 92% | 0.4 | 2 | 1 | 3 |  |
| Total Copper (mixed) | 66 | 49 | 45 | 3.41 | 2.57 | 3.67 | 38445 | 3.5 | 8716 | 0.67% | 94% | 0.7 | 67 | 115 | 188 |  |

#### 2026Q2 — gold sites (Barrick attributable basis except the '100%' column)

| Site (Barrick share) | Au prod 100% koz | Au prod attr koz | Au sold attr koz | CoS $/oz | TCC $/oz | AISC $/oz | Mined kt | Strip | Processed kt | Grade proc g/t | Recovery | Royalty $/oz | Dep $/oz | Sust capex $M | Project capex $M | Total capex $M | Revenue $M |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| Carlin (61.5%) | 280 | 172 | 173 | 1856 | 1405 | 1971 | 14764 | 30.36 | 2016 | 3.23 | 81% | 128 | 421 | 96 | 23 | 120 | 769 |
| Cortez (61.5%) | 169 | 104 | 104 | 2267 | 1713 | 2028 | 12841 | 4.49 | 3698 | 1.16 | 83% | 189 | 542 | 29 | 45 | 74 | 457 |
| Turquoise Ridge (61.5%) | 130 | 80 | 82 | 1657 | 1319 | 1552 | 1496 | 55.7 | 572 | 4.88 | 86% | 1 | 320 | 18 | 6 | 24 | 363 |
| Phoenix (61.5%) | 44 | 26 | 27 | 2343 | 640 | 917 | 4688 | 1.04 | 2168 | 0.69 | 71% | — | 381 | 6 | — | 6 | 151 |
| Nevada Gold Mines LLC (61.5%) | 623 | 382 | 386 | 1961 | 1418 | 1843 | 33789 | 5.43 | 8454 | 1.93 | 82% | 108 | 430 | 155 | 74 | 231 | 1740 |
| Pueblo Viejo (60%) | 186 | 112 | 108 | 1683 | 1016 | 1322 | 6261 | 3.81 | 1862 | 2.28 | 75% | 147 | 473 | 32 | 30 | 62 | 463 |
| Hemlo (100%; sold 26 Nov 2025) | — | — | — | — | — | — | — | n/a | — | — | 0% | — | — | — | — | — | — |
| Veladero (50%) | 72 | 36 | 45 | 2098 | 1176 | 1487 | 6955 | 1.28 | 2879 | 0.65 | n/a | 226 | 801 | 12 | 8 | 20 | 211 |
| Porgera (24.5%) | 94 | 23 | 23 | 1899 | 1385 | 1811 | 1620 | 2.26 | 329 | 2.24 | 92% | 134 | 498 | 9 | 5 | 14 | 104 |
| Loulo-Gounkoto (80%) | 109 | 88 | 86 | 2352 | 1897 | 2240 | 2738 | 13.77 | 914 | 3.25 | 92% | 585 | 383 | 28 | 2 | 30 | 385 |
| Kibali (45%) | 168 | 76 | 71 | 1873 | 1381 | 1702 | 6701 | 8.35 | 923 | 2.82 | 90% | 234 | 479 | 20 | 40 | 60 | 314 |
| Tongon (89.7%; sold 1 Dec 2025) | — | — | — | — | — | — | — | — | — | — | 0% | — | — | — | — | — | (blank) |
| North Mara (84%) | 41 | 35 | 37 | 2736 | 2164 | 2577 | 5043 | 42.46 | 688 | 1.8 | 85% | 298 | 536 | 14 | 39 | 53 | 163 |
| Bulyanhulu (84%) | 53 | 44 | 45 | 1854 | 1249 | 1748 | 401 | n/a | 236 | 6.03 | 97% | 270 | 387 | 23 | 20 | 43 | 198 |
| **North America** | 809 | 494 | 494 | 1900 | 1330 | 1729 |  |  |  |  |  |  |  |  |  |  |  |
| **South America & Asia Pacific** | 166 | 59 | 68 | 2031 | 1247 | 1597 |  |  |  |  |  |  |  |  |  |  |  |
| **Africa & Middle East** | 371 | 243 | 239 | 2175 | 1662 | 2039 |  |  |  |  |  |  |  |  |  |  |  |
| **Total Gold** | 1346 | 796 | 801 | 1993 | 1426 | 1866 |  |  |  |  |  |  |  |  |  |  |  |

#### 2026Q2 — copper sites

| Site (Barrick share) | Cu prod 100% kt | Cu prod attr kt | Cu sold attr kt | CoS $/lb | C1 $/lb | AISC $/lb | Mined kt | Strip | Processed kt | Grade proc % Cu | Recovery | Dep $/lb | Sust capex $M | Project capex $M | Total capex $M | Revenue $M |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| Zaldívar (50%) | 17 | 9 | 9 | 4.89 | 3.71 | 5.5 | 8549 | 2.4 | 2293 | 0.43% | n/a | 1.12 | 30 | 23 | 53 | 123 |
| Lumwana (100%, not printed) | 41 | 41 | 40 | 3.14 | 2.35 | 3.83 | 41542 | 5.74 | 6946 | 0.64% | 91% | 0.8 | 87 | 146 | 242 | 505 |
| Jabal Sayid (50%) | 14 | 6 | 5 | 2.63 | 1.08 | 1.42 | 435 | n/a | 372 | 2.05% | 91% | 0.65 | 2 | 2 | 4 | 69 |
| Total Copper (mixed) | 72 | 56 | 54 | 3.39 | 2.47 | 3.95 | 50526 | 4.77 | 9611 | 0.64% | 94% | 0.84 | 119 | 171 | 299 | 697 |

#### 2026H1 — gold sites (Barrick attributable basis except the '100%' column)

| Site (Barrick share) | Au prod 100% koz | Au prod attr koz | Au sold attr koz | CoS $/oz | TCC $/oz | AISC $/oz | Mined kt | Strip | Processed kt | Grade proc g/t | Recovery | Royalty $/oz | Dep $/oz | Sust capex $M | Project capex $M | Total capex $M | Revenue $M |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| Carlin (61.5%) | 561 | 345 | 346 | 1825 | 1360 | 1904 | 24818 | 15.53 | 3516 | 3.74 | 81% | 136 | 433 | 183 | 48 | 235 | 1629 |
| Cortez (61.5%) | 314 | 193 | 193 | 2213 | 1647 | 1995 | 26899 | 2.86 | 8123 | 1 | 82% | 200 | 548 | 61 | 81 | 142 | 899 |
| Turquoise Ridge (61.5%) | 281 | 171 | 178 | 1479 | 1153 | 1337 | 2383 | 68.89 | 1285 | 4.78 | 86% | 1 | 308 | 31 | 10 | 41 | 838 |
| Phoenix (61.5%) | 80 | 49 | 49 | 2407 | 611 | 892 | 9636 | 1.46 | 4449 | 0.69 | 75% | — | 400 | 11 | — | 11 | 294 |
| Nevada Gold Mines LLC (61.5%) | 1236 | 758 | 766 | 1881 | 1337 | 1745 | 63736 | 4.03 | 17373 | 1.89 | 82% | 112 | 432 | 295 | 139 | 438 | 3665 |
| Pueblo Viejo (60%) | 321 | 193 | 190 | 1691 | 1018 | 1380 | 10626 | 5.42 | 3436 | 2.28 | 74% | 153 | 479 | 67 | 56 | 123 | 858 |
| Hemlo (100%; sold 26 Nov 2025) | — | — | — | 1691 | — | — | — | n/a | — | — | — | — | — | — | — | — | — |
| Veladero (50%) | 183 | 91 | 99 | 1945 | 1101 | 1360 | 14086 | 1.63 | 6106 | 0.6 | n/a | 250 | 725 | 22 | 23 | 45 | 480 |
| Porgera (24.5%) | 172 | 42 | 45 | 1786 | 1366 | 1777 | 3152 | 2.58 | 643 | 2.18 | 92% | 152 | 402 | 16 | 8 | 24 | 209 |
| Loulo-Gounkoto (80%) | 189 | 152 | 155 | 2551 | 1906 | 2104 | 3128 | 13.77 | 1603 | 3.2 | 92% | 629 | 408 | 28 | 2 | 30 | 726 |
| Kibali (45%) | 297 | 134 | 140 | 1889 | 1399 | 1659 | 13535 | 8.35 | 1833 | 2.53 | 90% | 245 | 476 | 31 | 67 | 98 | 656 |
| Tongon (89.7%; sold 1 Dec 2025) | — | — | — | — | — | — | — | — | — | — | 0% | — | — | — | — | — | (blank) |
| North Mara (84%) | 80 | 68 | 73 | 2511 | 1961 | 2283 | 8995 | 70.92 | 1352 | 1.81 | 85% | 306 | 512 | 21 | 76 | 97 | 346 |
| Bulyanhulu (84%) | 91 | 77 | 81 | 1923 | 1307 | 1826 | 750 | n/a | 440 | 5.62 | 97% | 278 | 401 | 42 | 37 | 79 | 385 |
| **North America** | 1557 | 951 | 956 | 1843 | 1274 | 1673 |  |  |  |  |  |  |  |  |  |  |  |
| **South America & Asia Pacific** | 355 | 133 | 144 | 1896 | 1183 | 1490 |  |  |  |  |  |  |  |  |  |  |  |
| **Africa & Middle East** | 657 | 431 | 449 | 2225 | 1649 | 1944 |  |  |  |  |  |  |  |  |  |  |  |
| **Total Gold** | 2569 | 1515 | 1549 | 1959 | 1378 | 1790 |  |  |  |  |  |  |  |  |  |  |  |

#### 2026H1 — copper sites

| Site (Barrick share) | Cu prod 100% kt | Cu prod attr kt | Cu sold attr kt | CoS $/lb | C1 $/lb | AISC $/lb | Mined kt | Strip | Processed kt | Grade proc % Cu | Recovery | Dep $/lb | Sust capex $M | Project capex $M | Total capex $M | Revenue $M |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| Zaldívar (50%) | 34 | 17 | 17 | 4.88 | 3.71 | 5.27 | 15503 | 1.99 | 4714 | 0.51% | n/a | 1.11 | 50 | 33 | 83 | 233 |
| Lumwana (100%, not printed) | 73 | 73 | 70 | 3.21 | 2.49 | 3.83 | 72606 | 5.08 | 12857 | 0.62% | 91% | 0.74 | 132 | 250 | 397 | 852 |
| Jabal Sayid (50%) | 30 | 15 | 12 | 2.3 | 0.9 | 1.13 | 862 | n/a | 756 | 2.21% | 92% | 0.5 | 4 | 3 | 7 | 168 |
| Total Copper (mixed) | 137 | 105 | 99 | 3.4 | 2.52 | 3.82 | 88971 | 4.14 | 18327 | 0.66% | 94% | 0.77 | 186 | 286 | 487 | 1253 |

#### 2025Q1 — gold sites (Barrick attributable basis except the '100%' column)

| Site (Barrick share) | Au prod 100% koz | Au prod attr koz | Au sold attr koz | CoS $/oz | TCC $/oz | AISC $/oz | Mined kt | Strip | Processed kt | Grade proc g/t | Recovery | Royalty $/oz | Dep $/oz | Sust capex $M | Project capex $M | Total capex $M | Revenue $M |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| Carlin (61.5%) | 236 | 145 | 142 | 1720 | 1459 | 2570 | 16710 | 123.33 | 1377 | 3.97 | 81% | 39 | 254 | 156 | 18 | 174 |  |
| Cortez (61.5%) | 149 | 92 | 96 | 1541 | 1172 | 1536 | 14397 | 9.02 | 1782 | 1.87 | 85% | 114 | 365 | 32 | 28 | 60 |  |
| Turquoise Ridge (61.5%) | 121 | 74 | 78 | 1605 | 1227 | 1408 | 201 | n/a | 655 | 4.08 | 85% | — | 370 | 13 | 1 | 14 |  |
| Phoenix (61.5%) | 50 | 31 | 29 | 1686 | 747 | 1012 | 5653 | 1.3 | 2329 | 1.02 | 72% | — | 357 | 6 | — | 6 |  |
| Nevada Gold Mines LLC (61.5%) | 556 | 342 | 345 | 1643 | 1269 | 1899 | 36961 | 7.94 | 6143 | 2.51 | 82% | 48 | 320 | 209 | 48 | 257 |  |
| Pueblo Viejo (60%) | 123 | 74 | 76 | 1863 | 1189 | 1668 | 1382 | 14.53 | 1294 | 2.17 | 82% | 93 | 582 | 36 | 8 | 46 |  |
| Hemlo (100%; sold 26 Nov 2025) | 38 | 38 | 39 | 1730 | 1458 | 1692 | 416 | n/a | 345 | 3.77 | 93% | 350 | 250 | 8 | 1 | 9 |  |
| Veladero (50%) | 143 | 71 | 68 | 1141 | 753 | 1271 | 8785 | 1.63 | 3042 | 0.96 | n/a | 147 | 368 | 33 | 15 | 48 |  |
| Porgera (24.5%) | 85 | 21 | 21 | 1675 | 1336 | 1684 | 1611 | 5.35 | 360 | 1.87 | 86% | 83 | 330 | 6 | 2 | 8 |  |
| Loulo-Gounkoto (80%) | 22 | 18 | — | — | — | — | 1709 | 23.96 | 169 | 3.54 | 91% | — | — | 10 | 3 | 14 |  |
| Kibali (45%) | 141 | 63 | 67 | 1691 | 1212 | 1426 | 5246 | 11.41 | 931 | 2.36 | 90% | 243 | 473 | 12 | 20 | 32 |  |
| Tongon (89.7%; sold 1 Dec 2025) | 30 | 27 | 29 | 2154 | 1971 | 2144 | 7405 | 13.58 | 727 | 1.34 | 87% | 236 | 178 | 3 | — | 3 |  |
| North Mara (84%) | 80 | 67 | 68 | 1257 | 986 | 1258 | 3843 | 2.27 | 672 | 3.56 | 88% | 204 | 258 | 17 | 17 | 34 |  |
| Bulyanhulu (84%) | 44 | 37 | 38 | 1714 | 1212 | 1831 | 304 | n/a | 237 | 5.27 | 93% | 186 | 364 | 23 | 12 | 35 |  |
| **North America** | 717 | 454 | 460 | 1687 | 1272 | 1843 |  |  |  |  |  |  |  |  |  |  |  |
| **South America & Asia Pacific** | 228 | 92 | 89 | 1267 | 890 | 1368 |  |  |  |  |  |  |  |  |  |  |  |
| **Africa & Middle East** | 317 | 212 | 202 | 1639 | 1244 | 1602 |  |  |  |  |  |  |  |  |  |  |  |
| **Total Gold** | 1262 | 758 | 751 | 1629 | 1220 | 1775 |  |  |  |  |  |  |  |  |  |  |  |

#### 2025Q1 — copper sites

| Site (Barrick share) | Cu prod 100% kt | Cu prod attr kt | Cu sold attr kt | CoS $/lb | C1 $/lb | AISC $/lb | Mined kt | Strip | Processed kt | Grade proc % Cu | Recovery | Dep $/lb | Sust capex $M | Project capex $M | Total capex $M | Revenue $M |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| Zaldívar (50%) | 18 | 9 | 10 | 4.11 | 2.99 | 3.38 | 6863 | 0.36 | 4241 | 0.41% | n/a | 1.11 | 5 | 2 | 7 |  |
| Lumwana (100%, not printed) | 27 | 27 | 34 | 2.8 | 2.22 | 3.2 | 30310 | 4.05 | 5237 | 0.57% | 91% | 0.81 | 50 | 20 | 70 |  |
| Jabal Sayid (50%) | 17 | 8 | 7 | 1.96 | 1.44 | 1.55 | 434 | n/a | 370 | 2.50% | 90% | 0.4 | 2 | 2 | 4 |  |
| Total Copper (mixed) | 62 | 44 | 51 | 2.92 | 2.25 | 3.06 | 37607 | 2.36 | 9848 | 0.57% | 94% | 0.81 | 57 | 24 | 81 |  |

#### 2025Q2 — gold sites (Barrick attributable basis except the '100%' column)

| Site (Barrick share) | Au prod 100% koz | Au prod attr koz | Au sold attr koz | CoS $/oz | TCC $/oz | AISC $/oz | Mined kt | Strip | Processed kt | Grade proc g/t | Recovery | Royalty $/oz | Dep $/oz | Sust capex $M | Project capex $M | Total capex $M | Revenue $M |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| Carlin (61.5%) | 277 | 170 | 166 | 1589 | 1330 | 1826 | 16042 | 39.19 | 1432 | 4.5 | 81% | 57 | 254 | 78 | 18 | 98 | 552 |
| Cortez (61.5%) | 176 | 108 | 107 | 1687 | 1326 | 1774 | 14639 | 11.92 | 1553 | 2.69 | 82% | 121 | 357 | 45 | 30 | 75 | 356 |
| Turquoise Ridge (61.5%) | 123 | 76 | 75 | 1761 | 1394 | 1621 | 221 | n/a | 570 | 4.74 | 89% | — | 358 | 16 | — | 16 | 252 |
| Phoenix (61.5%) | 44 | 27 | 28 | 2033 | 1010 | 1376 | 6402 | 1.54 | 2386 | 0.87 | 75% | — | 399 | 8 | — | 8 | 111 |
| Nevada Gold Mines LLC (61.5%) | 620 | 381 | 376 | 1685 | 1319 | 1749 | 37304 | 7.95 | 5941 | 2.97 | 82% | 59 | 314 | 151 | 48 | 201 | 1272 |
| Pueblo Viejo (60%) | 158 | 95 | 93 | 1715 | 1147 | 1552 | 3876 | 4.76 | 1611 | 2.34 | 77% | 110 | 496 | 37 | 16 | 56 | 306 |
| Hemlo (100%; sold 26 Nov 2025) | 32 | 32 | 32 | 1837 | 1512 | 1766 | 353 | n/a | 293 | 3.38 | 94% | 242 | 298 | 7 | 1 | 8 | 106 |
| Veladero (50%) | 124 | 62 | 67 | 1234 | 751 | 1295 | 7615 | 1.99 | 3369 | 0.68 | n/a | 158 | 459 | 34 | 7 | 41 | 221 |
| Porgera (24.5%) | 93 | 23 | 24 | 1354 | 1041 | 1406 | 1688 | 3.5 | 350 | 2.41 | 90% | 99 | 303 | 8 | 2 | 10 | 80 |
| Loulo-Gounkoto (80%) | — | — | — | — | — | — | — | — | — | — | — | — | — | 3 | — | 4 | — |
| Kibali (45%) | 166 | 75 | 69 | 1565 | 1094 | 1273 | 5421 | 7.04 | 946 | 2.73 | 90% | 175 | 466 | 10 | 20 | 30 | 226 |
| Tongon (89.7%; sold 1 Dec 2025) | 33 | 29 | 28 | 2397 | 2204 | 2390 | 7009 | 16.01 | 790 | 1.33 | 87% | 267 | 190 | 3 | 1 | 4 | 93 |
| North Mara (84%) | 74 | 62 | 50 | 1430 | 1073 | 1292 | 3271 | 4.91 | 698 | 3.16 | 88% | 247 | 330 | 10 | 33 | 43 | 168 |
| Bulyanhulu (84%) | 45 | 38 | 31 | 1722 | 1189 | 1885 | 377 | n/a | 231 | 5.37 | 95% | 215 | 370 | 22 | 14 | 36 | 112 |
| **North America** | 810 | 508 | 501 | 1701 | 1299 | 1714 |  |  |  |  |  |  |  |  |  |  |  |
| **South America & Asia Pacific** | 217 | 85 | 91 | 1266 | 828 | 1325 |  |  |  |  |  |  |  |  |  |  |  |
| **Africa & Middle East** | 318 | 204 | 178 | 1718 | 1277 | 1577 |  |  |  |  |  |  |  |  |  |  |  |
| **Total Gold** | 1345 | 797 | 770 | 1654 | 1239 | 1684 |  |  |  |  |  |  |  |  |  |  |  |

#### 2025Q2 — copper sites

| Site (Barrick share) | Cu prod 100% kt | Cu prod attr kt | Cu sold attr kt | CoS $/lb | C1 $/lb | AISC $/lb | Mined kt | Strip | Processed kt | Grade proc % Cu | Recovery | Dep $/lb | Sust capex $M | Project capex $M | Total capex $M | Revenue $M |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| Zaldívar (50%) | 14 | 7 | 7 | 4.59 | 3.46 | 4.34 | 8188 | 0.86 | 3381 | 0.51% | n/a | 1.12 | 10 | 3 | 13 | 72 |
| Lumwana (100%, not printed) | 44 | 44 | 39 | 2.25 | 1.58 | 2.79 | 37481 | 3.89 | 7082 | 0.67% | 92% | 0.8 | 78 | 72 | 151 | 340 |
| Jabal Sayid (50%) | 15 | 8 | 8 | 2.11 | 1.29 | 1.46 | 454 | n/a | 370 | 2.29% | 91% | 0.43 | 2 | 2 | 4 | 72 |
| Total Copper (mixed) | 73 | 59 | 54 | 2.56 | 1.8 | 2.9 | 46123 | 2.79 | 10833 | 0.67% | 94% | 0.79 | 90 | 77 | 168 | 484 |

#### 2025H1 — gold sites (Barrick attributable basis except the '100%' column)

| Site (Barrick share) | Au prod 100% koz | Au prod attr koz | Au sold attr koz | CoS $/oz | TCC $/oz | AISC $/oz | Mined kt | Strip | Processed kt | Grade proc g/t | Recovery | Royalty $/oz | Dep $/oz | Sust capex $M | Project capex $M | Total capex $M | Revenue $M |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| Carlin (61.5%) | 513 | 315 | 308 | 1649 | 1390 | 2169 | 32752 | 60.47 | 2809 | 4.24 | 81% | 49 | 254 | 234 | 36 | 272 | 969 |
| Cortez (61.5%) | 325 | 200 | 203 | 1618 | 1253 | 1662 | 29036 | 10.3 | 3335 | 2.25 | 83% | 118 | 360 | 77 | 58 | 135 | 637 |
| Turquoise Ridge (61.5%) | 244 | 150 | 153 | 1682 | 1310 | 1513 | 422 | n/a | 1225 | 4.39 | 87% | — | 364 | 30 | — | 30 | 476 |
| Phoenix (61.5%) | 94 | 58 | 57 | 1856 | 876 | 1190 | 12055 | 1.42 | 4715 | 0.94 | 73% | — | 377 | 14 | — | 14 | 217 |
| Nevada Gold Mines LLC (61.5%) | 1176 | 723 | 721 | 1665 | 1295 | 1821 | 74265 | 7.94 | 12084 | 2.73 | 82% | 54 | 318 | 360 | 96 | 458 | 2302 |
| Pueblo Viejo (60%) | 281 | 169 | 169 | 1781 | 1166 | 1605 | 5258 | 5.9 | 2905 | 2.26 | 79% | 102 | 535 | 73 | 24 | 102 | 534 |
| Hemlo (100%; sold 26 Nov 2025) | 70 | 70 | 71 | 1779 | 1482 | 1725 | 769 | n/a | 638 | 3.59 | 94% | 301 | 285 | 15 | 2 | 17 | 221 |
| Veladero (50%) | 267 | 133 | 135 | 1187 | 752 | 1283 | 16400 | 1.79 | 6411 | 0.82 | n/a | 152 | 413 | 67 | 22 | 89 | 421 |
| Porgera (24.5%) | 178 | 44 | 45 | 1503 | 1179 | 1535 | 3299 | 4.25 | 710 | 2.14 | 88% | 91 | 316 | 14 | 4 | 18 | 141 |
| Loulo-Gounkoto (80%) | 22 | 18 | — | — | — | — | 1709 | 23.96 | 169 | 3.54 | 91% | — | — | 13 | 3 | 18 | — |
| Kibali (45%) | 307 | 138 | 136 | 1627 | 1152 | 1348 | 10667 | 8.73 | 1877 | 2.55 | 90% | 208 | 469 | 22 | 40 | 62 | 418 |
| Tongon (89.7%; sold 1 Dec 2025) | 63 | 56 | 57 | 2272 | 2084 | 2264 | 14414 | 14.67 | 1517 | 1.34 | 87% | 251 | 184 | 6 | 1 | 7 | 178 |
| North Mara (84%) | 154 | 129 | 118 | 1330 | 1022 | 1272 | 7114 | 3.1 | 1370 | 3.36 | 88% | 222 | 288 | 27 | 50 | 77 | 366 |
| Bulyanhulu (84%) | 89 | 75 | 69 | 1718 | 1201 | 1856 | 681 | n/a | 468 | 5.32 | 94% | 199 | 367 | 45 | 26 | 71 | 235 |
| **North America** | 1527 | 962 | 961 | 1694 | 1286 | 1776 |  |  |  |  |  |  |  |  |  |  |  |
| **South America & Asia Pacific** | 445 | 177 | 180 | 1266 | 859 | 1346 |  |  |  |  |  |  |  |  |  |  |  |
| **Africa & Middle East** | 635 | 416 | 380 | 1676 | 1260 | 1591 |  |  |  |  |  |  |  |  |  |  |  |
| **Total Gold** | 2607 | 1555 | 1521 | 1641 | 1229 | 1728 |  |  |  |  |  |  |  |  |  |  |  |

#### 2025H1 — copper sites

| Site (Barrick share) | Cu prod 100% kt | Cu prod attr kt | Cu sold attr kt | CoS $/lb | C1 $/lb | AISC $/lb | Mined kt | Strip | Processed kt | Grade proc % Cu | Recovery | Dep $/lb | Sust capex $M | Project capex $M | Total capex $M | Revenue $M |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| Zaldívar (50%) | 32 | 16 | 17 | 4.32 | 3.2 | 3.79 | 15051 | 0.59 | 7622 | 0.46% | n/a | 1.11 | 15 | 5 | 20 | 167 |
| Lumwana (100%, not printed) | 71 | 71 | 73 | 2.51 | 1.88 | 2.98 | 67791 | 3.96 | 12319 | 0.63% | 92% | 0.8 | 128 | 92 | 221 | 645 |
| Jabal Sayid (50%) | 32 | 16 | 15 | 2.03 | 1.36 | 1.51 | 888 | n/a | 739 | 2.40% | 91% | 0.41 | 4 | 4 | 8 | 145 |
| Total Copper (mixed) | 135 | 103 | 105 | 2.74 | 2.02 | 2.98 | 83730 | 2.58 | 20681 | 0.63% | 94% | 0.8 | 147 | 101 | 249 | 957 |


### 3.5 Reproduction tests (run now; `python3 -I`)

| Test | Identity | Result | Notes |
|---|---|---|---|
| T1 | attributable = 100% × ownership (±0.5 + rounding) | **84 pass / 5 fail** (94.4%) | fails are DR-01, DR-02, DR-03 (2 cells), DR-04. NGM Q2 2026 623 × 0.615 = 383.1 vs 382 is within ±1 |
| T2 | regional and total flows = Σ sites (production, sales) | 108 / 108 | |
| T3 | regional/total unit costs = sold-weighted Σ of components (1%) | 87 / 108 | the 21 misses are all aggregation layers (DR-12), not errors |
| T4 | Minestats value = summary value (production, sales, CoS, TCC, AISC) | 539 / 540 | DR-05 |
| T5a/T5c | Q1 + Q2 = H1 for flows (summary / Minestats) | 129 / 132; 421 / 424 | DR-01, DR-02, DR-04, DR-08 |
| T5b | H1 unit cost = sold-weighted Q1, Q2 (1%) | 122 / 123 | Jabal Sayid C1 0.91 vs 0.90 (−1.2%, small-volume rounding) |
| T6, T6b, T6c | mined = ore + waste + UG; processed = Σ circuits; production = Σ circuits | 101/101, 51/51, 51/51 | |
| T7 | strip = OP waste / OP ore | 82 / 82 | |
| T8 | sustaining + project = total capex | **82 / 101**; all 19 residuals positive (+1 to +15 $M) | DR-11 |
| T9 | processed × grade × stated recovery ≈ production (±3 pts implied recovery) | 57 / 77 | single-circuit mills close (Lumwana, North Mara, Bulyanhulu, Kibali, Loulo-Gounkoto). Fails are explained by Phoenix grade basis, Cortez/Veladero heap leach and timing at PV/Porgera/Jabal Sayid/Hemlo (§3.6) |
| T10 | CoS − depreciation − TCC = by-product credits and other | (diagnostic) | $4–40/oz at most sites; Phoenix $1,322–1,487 (copper credits); Pueblo Viejo about $194; Bulyanhulu $138–218; Loulo-Gounkoto Q1 2026 **$444** (unexplained, OPEN) |
| T11 | AISC − TCC − sustaining capex/oz ≥ −25 | 98 / 98 | residual "other" $5–80/oz; Bulyanhulu −12.1 (2026Q2) and −13.7 (2025Q2), within cash-vs-accrual rounding |
| T14 | no numeric zero where siblings are nil | 239 / 246 | DR-07 |
| T15 | stored value = displayed precision | 3,246 / 3,246 | 26 strip cells have `General` format |

**M0 target-cell census.** Five per-site fields (CoS, TCC/C1, AISC, processed grade, recovery) were counted
across six periods, excluding the NGM and Copper-Total aggregates. Of 480 cells:

- 427 are numeric;
- 36 are `—`;
- 12 are `n/a`;
- 5 are numeric-zero placeholders.

### 3.6 Discrepancy-register candidates (exact cells)

| ID | Cells | Observation | Candidate explanation | Status |
|---|---|---|---|---|
| DR-01 | `Q1_2026:QTD!B5` = 283 (Carlin, 100%, 2026Q1) | 283 × 0.615 = 174.0 ≠ `C5` 173. Also 283 + `Q2_2026:QTD!B5` 280 = 563 ≠ `Q2_2026:YTD!B5` 561 | 281 satisfies both identities, which points to a keying error in B5 | PLAUSIBLE |
| DR-02 | `Q2_2026:YTD!B7` = 281 (Turquoise Ridge, 100%, H1 2026) | `Q1_2026:QTD!B7` 148 + `Q2_2026:QTD!B7` 130 = 278. Also 281 × 0.615 = 172.8 ≠ `YTD!C7` 171, while 278 × 0.615 = 171.0 | YTD cell error; 278 fits | PLAUSIBLE |
| DR-03 | `Q1_2026:QTD!B8` 36 / `C8` 23; `Q2_2026:QTD!B8` 44 / `C8` 26 (Phoenix) | Quarterly 100% × 0.615 = 22.1 and 27.1. H1 80 / 49 and the quarter sums are consistent | rounding allocated between quarters | PLAUSIBLE (low) |
| DR-04 | `Q2_2026:QTD!B28` 14 / `C28` 6 (Jabal Sayid 50%); `Q1_2026:QTD!B28` 18 / `C28` 9; `Q2_2026:YTD!B28` 30 / `C28` 15 | 14 × 0.5 ≠ 6, and 18 + 14 = 32 ≠ 30. Attributable values sum correctly. Metal balance gives Q2 attributable 372 × 2.05% × 0.91 = 6.9 kt (favours 7) and Q1 384 × 2.36% × 0.92 = 8.3 kt (favours 8, stated 9) | unresolved; closer: Q2 2026 Report, p.44 onward | OPEN |
| DR-05 | `Q2_2026:YTD!E11` = 1691 (Hemlo CoS, H1 2026) | Hemlo was sold 26 Nov 2025, and every other Hemlo 2026 cell is `—` (including `Q2_2026:Minestats1!N103`). 1691 equals Pueblo Viejo `YTD!E10` | copy error from the row above | PLAUSIBLE (high) |
| DR-06 | `Q1_2026:Minestats4!B14`, `C14`; `Q2_2026:Minestats4!B14`, `C14`, `B51`, `C51` (Copper Total recovery) | 94% in all six periods, while Lumwana is 91–92%, Jabal Sayid 90–92% and Zaldívar `n/a`. Any weighting of the stated site recoveries gives 91–92%. Metal balance gives 93.3% (2026Q1) and 90.2% (2026Q2) | stale value or a different definition | PLAUSIBLE |
| DR-07 | `Q1_2026:Minestats1!N27`, `N28`; `Q1_2026:Minestats3!F19`; `Q2_2026:Minestats1!N27`, `N28`; `Q2_2026:Minestats3!F19`, `F64` | numeric 0 displayed "0%" for divested Hemlo and Tongon, where sibling cells are `—` | placeholder; import as Nil | CONFIRMED (format) |
| DR-08 | `Q2_2026:Minestats1!H97` = 1 (Phoenix autoclave koz, H1 2026) vs `Q1_2026:Minestats1!H34` `n/a` and `Q2_2026:Minestats1!H34` `—`. Also `H98`/`I98` `—` vs `n/a` in the quarters | H1 shows production that neither quarter shows. Placeholder semantics flip between `n/a` and `—` | rounding of sub-0.5 koz, plus inconsistent tokens | PLAUSIBLE (low) |
| DR-09 | footnote `Q1_2026:QTD!A34` vs `Q1_2026:QTD!I16` 22, `J16` 18 and `Q1_2026:Minestats3!C4` 1,709, `C12` 169, `C18` 3.54, `C19` 0.91 | The footnote says "no operating or per ounce data was provided for Q1 2025 to Q3 2025", but the Q1 2025 column holds operating data. Per-ounce cells are `—` | the data likely cover 1–14 Jan 2025, before the suspension (INFERENCE) | PLAUSIBLE (low) |
| DR-10 | `Q2_2026:Minestats3!B53` = 13.77 = `B8` (Loulo-Gounkoto strip ratio, H1 = Q2 2026) | consistent only if Q1 2026 had no open-pit mining. `Q1_2026:Minestats3!B8` is `n/a` and `B4` is 390, so it holds | not an error; listed as a pre-checked false positive | CLOSED |
| DR-11 | 19 capex triples, e.g. Lumwana `Q1_2026:Minestats4!D26:D28` (45 + 104 vs 155), `Q2_2026:Minestats4!D28:D30` (87 + 146 vs 242), `D65:D67` (132 + 250 vs 397) | total exceeds sustaining + project by +1 to +15 $M, never negative. Others: Carlin +2 to +4, NGM +2 to +4, PV +2 to +5, Loulo-Gounkoto 2025H1 +2 | capitalized interest is in the total but in neither component (footnote wording; INFERENCE) | PLAUSIBLE; closer: Q-report capex note |
| DR-12 | Total Gold AISC `Q1_2026:QTD!G22` 1708, `Q2_2026:QTD!G22` 1866, `Q2_2026:YTD!G22` 1790 | exceeds sold-weighted regions by **+$55.36, +$55.71, +$55.46/oz** (2026Q1, Q2, H1) and +$53.11, +$47.64, +$49.11 (2025). TCC +$3.8–4.9. NGM AISC (`QTD!G9`) +$8.69 / +$19.38 / +$14.57 (2026) and +$10.40 / +$12.20 / +$11.35 (2025) over its four mines. Total Copper AISC +$0.05–0.11/lb. AME 2025 CoS +$23–33 and AISC +$16–53 | unallocated layers (corporate and JV-level G&A, other; INFERENCE). Not errors | needs a named layer (G-BULL-3) |
| DR-13 | `LUM` p.41 endnote 2 vs p.8 | p.41: probable "1,500million tonnes grading 0.53% representing 8.3 million tonnes". But 1,500 × 0.53% = 7.95 Mt, and proven 0.686 + probable 7.95 = 8.64 Mt at 0.527% vs the p.8 P&P total "1.6Bt @ 0.52% for 8.3Mt Cu" (1,600 × 0.52% = 8.32) | the p.41 probable contained-metal or grade is misstated | OPEN; closer: Lumwana Expansion technical report, 19 Feb 2025 |
| DR-14 | `FM` p.14, copper roll-forward, 2025 row | net change printed **131** (positive). 40,201 − 708 + 131 = 39,624 ≠ closing 39,361 (gap −263). With −131: 39,362 (−1, rounding). GEO 2025 net change 1.4 reproduces only with −131 (1.42; +131 gives 1.98). So the "2019–2025 Total" 30,252 should be 29,990 | sign error (missing parentheses) | PLAUSIBLE (high); verified on page render |
| DR-15 | `FM` p.14–15, GEO 2024 row ("Reported Reserve Price" $1,400 / $3.00) | net change GEO 79 is not reproducible at $1,400/$3.00: 17.0 + 28,542 × 3.00/1,400 = 78.16 (78.21 max within rounding). At $1,500/$3.25 it is 78.84 ≈ 79. Every other year fits its stated basis | 2024 GEO row priced at the 2025 deck: a mixed basis inside one table | PLAUSIBLE |
| DR-16 | `FM` p.14 endnote 16 text vs table | the text says "from year-end 2019 to 2024", while the table runs to 2025 | stale endnote text | CONFIRMED (text) |
| DR-17 | `TZ` p.8 | chart title "(kt)" but the axis is labelled in tonnes ("6,000" scale) | label inconsistency | low |

### 3.7 Site-deck data (M2 site choice, Lodestar §11, Helm §23)

**Lumwana (100% Barrick; `LUM` unless noted)**

| Item | Value | Basis | Source |
|---|---|---|---|
| H1 2025 actuals | Cu 70.6 kt; CoS 2.51, C1 1.88, AISC 2.98 $/lb | 100% | p.9. Matches `Q2_2026:YTD!I27` 71, `L27` 2.51, `M27` 1.88, `N27` 2.98 |
| 2025 guidance | 125–155 kt; CoS 2.30–2.60; C1 1.60–1.90; AISC 2.80–3.10 $/lb | 100% | p.9 |
| Production history 2009–2024 | 352,133,027 t milled @ 0.57%; 2,008,485 t Cu in feed; 1,844,683 t produced; 91.84% recovery (table sums exactly) | 100% | p.39 |
| Recovery history | 2013–2022 mean 93.37% (range 92.5–94.7); **2023 89.4%, 2024 89.82%**; head grade 0.49% and 0.53% | 100% | p.39 |
| Recovery shortfall vs the 2013–22 mean | 5,269 t Cu (2023), 4,850 t Cu (2024): about 4.5% and 4.0% of output; about $46.5M and $42.8M at the deck's $4.00/lb outlook price | INFERENCE | p.39, p.40 |
| 2025–26 quarterly recovery | 91%, 92%, 91%, 91%; metal-balance implied 0.904–0.927 | attributable = 100% | `Minestats4` row 14 (Q2 H1 row 51) |
| Processed grade 2025Q1 → 2026Q2 | 0.57%, 0.67%, 0.59%, 0.64% | 100% | `Minestats4` row 13 |
| Sold vs produced | 2025Q1 34 vs 27 kt (1.26); 2025Q2 39 vs 44 | 100% | `Q1_2026:QTD!K27`/`J27`; `Q2_2026:QTD!K27`/`J27` |
| Mining rate and strip | 2026Q1 31,064 kt (≈124 Mt/y), strip 4.37; 2026Q2 41,542 kt (≈166 Mt/y), strip 5.74 (2025Q2 3.89) | 100% | `Minestats4!D4`, `D8` (both files) |
| Expansion capex | project capex 104 / 146 / 250 $M (2026Q1 / Q2 / H1) vs 92 (H1 2025) | 100% | `Minestats4!D27`; `D29`; `D66`; `E66` |
| Copper cost build (per lb sold) | AISC − C1 = 1.16 / 1.48 / 1.34; sustaining capex/lb 0.68 / 0.99 / 0.86; residual (royalties, G&A, leases) 0.48 / 0.49 / 0.48 | INFERENCE | 2026Q1 / Q2 / H1 cells |
| Super Pit | "average of 240kt of copper p.a. from 2028"; 52 Mtpa (54 peak); LOM 7.7 Mt Cu 2025–2057 (+16 yr); initial capital ~US$2.0bn; peak funding and payback ~US$0.5bn and 2028 | 100% | p.23 |
| Super Pit LOM costs | CoS ~$2.37, C1 ~$1.59, AISC ~$2.51 /lb; incremental IRR ~25%; incremental NPV8 ~US$2.3bn; project IRR ~99%; consensus NPV8 ~US$5.0bn. Prices: consensus Sep 2025 ($4.37 / 4.48 / 4.58 / 4.68 / 4.32 per lb, 2025 → 2029+) | 100% | p.23 |
| Milestones | H1 2025 FS complete; H2 2026 "200Mt Mining Run Rate"; H2 2027 commissioning starts, "215Mt Mining Run Rate"; H1 2028 commissioning complete; H2 2028 "52Mtpa Plant Ramp Up" | | p.28 (render checked) |
| Site reconciliation claim | "Reconciliation results confirm the metal correlation between geological model and production"; derisk drilling coverage to June 2028 | | p.19 |
| Reserves | P&P "1.6Bt @ 0.52% for 8.3Mt Cu"; "+180% since 2019"; as of 31 Dec 2024 | 100% | p.8, p.41 (DR-13) |
| Drill-and-blast | XY / Z hole accuracy Jan → Jun 2025: 45/42 → 75/86% | | p.11 |
| Tier One definitions | Gold "$1,400/oz reserve", ≥ 10 yr, ≥ 500 koz/y; Copper "$3.00/lb reserve", +5 Mt, ≥ 20 yr, ≥ 200 ktpa; lower half of the cost curve | | p.41 endnote 1 |
| Outlook assumptions | gold $2,400/oz, copper $4.00/lb; GEO at $1,400/oz and $3.00/lb | | p.40 |

**North Mara and Bulyanhulu (84% Barrick, 16% GoT through Twiga)**

| Item | Value | Basis | Source |
|---|---|---|---|
| Twiga | "the GoT acquired a 16% freecarried interest in these sites, effective as of January 1, 2020" | | `TZ` pp.29–30 |
| NM OP mining Q1 2024 → Q1 2025 | 3,791 / 3,993 / 5,226 / 5,562 / 4,112 kt; throughput 774 / 852 / 812 / 861 / 800 kt; 410 t/h | **100%** | `TZ` p.8, p.5 |
| Bulyanhulu UG mining (same quarters) | 362 / 374 / 361 / 394 / 362 kt (ore 276 / 298 / 269 / 307 / 269); development 1,015 / 790 / 991 / 980 / 1,017 m | **100%** | `TZ` p.10 |
| NM processed grade | 3.56 → 1.81 g/t (Q1 2025 → Q1 2026); 3.16 → 1.80 (Q2) | attributable | `Q1_2026:Minestats3!I18` / `H18`; `Q2_2026:Minestats3!I18` / `H18` |
| NM AISC | 1,258 → 1,990 (Q1); 1,292 → 2,577 (Q2) $/oz sold | attributable | `Minestats3!I31`/`H31` (Q2), `I29`/`H29` (Q1) |
| **NM LMDI bridge Q2 2025 → Q2 2026** | +$1,285/oz = grade **+1,047**, cost +725, recovery +65, tonnes +27, metal-balance B −75, sold/produced S **−504** (sums exactly) | INFERENCE | `Q2_2026:Minestats3!H/I` rows 12, 18, 19, 20, 21, 31 |
| NM LMDI bridge Q1 2025 → Q1 2026 | +$732/oz = grade +1,080, cost −283, recovery +55, tonnes +19, B −24, S −115 | INFERENCE | `Q1_2026:Minestats3!H/I` rows 12, 18, 19, 20, 21, 29 |
| Reserve histories 2019–2023 | tabulated by site; combined 2023 P&P 7.6 Moz (p.4 says ">7Moz") | 100% | `TZ` pp.29–30 |
| 2020 production | NM 310 koz, Bulyanhulu 52, Buzwagi 100, total 462 (sums) | 100% | `MA21` p.7 |

**Fourmile (100% Barrick today; `FM`)**

| Item | Value | Source |
|---|---|---|
| Resource at 31 Dec 2025 | Indicated 4.6 Mt @ 17.59 g/t = 2.6 Moz; Inferred 25 Mt @ 16.9 g/t = 13 Moz (31 Dec 2024: 3.6 Mt @ 11.76 = 1.4 Moz; 14 Mt @ 14.1 = 6.4 Moz) | p.4, p.5 |
| Conceptual upside | 30–34 Mt @ 15–16 g/t | p.4 |
| Drill spacing | "Targeting 35m spacing for Indicated and 80-90m for Inferred" | p.6 |
| Metallurgy | p.6 splits the deposit into a roaster-amenable refractory domain and a single-refractory domain "Amenable Roaster or Autoclave" | p.6 |
| Timeline | Q4 2025 16 rigs; 2026 ramp to 20 rigs and Bullion Hill decline; 2027 conversion drilling and UG drilling from the Goldrush MPD; PFS, then independent FS; first ore; UG connection to Goldrush | p.4 |
| Economic basis | PEA on a "$1,900/oz mineable stope optimizer" | p.10 |
| **JV transfer** | "Barrick anticipates Fourmile will be incorporated into the Nevada Gold Mines joint venture, at fair market value, if certain criteria are met." | p.10, p.12 |
| Intercept reporting | 3.4 g/t cutoff, uncapped, minimum 2.4 m, ALS Minerals; e.g. FM25-300D 16.0 m @ 38.35 g/t; FM25-321D 26.9 m @ 33.71 g/t | p.8, pp.4–5 |
| 2026 assumptions | gold $4,500/oz, copper $5.50/lb | p.9 |

**Reserve roll-forward (`FM` pp.14–15, attributable)**

| Year | Au P&P (Moz) | Au depletion | Au net change | Cu P&P (Mlb) | Cu net change | GEO P&P | GEO net change | Reserve price |
|---|---|---|---|---|---|---|---|---|
| 2019 | 71 | – | – | 13,494 | – | – | – | – |
| 2020 | 68 | (5.5) | 4.2 | 12,691 | 31 | 97 | 4.2 | $1,200 / $2.75 |
| 2021 | 69 | (5.4) | 8.1 | 12,233 | 178 | 97 | 8.5 | $1,200 / $2.75 |
| 2022 | 76 | (4.8) | 12.0 | 12,252 | 642 | 104 | 13 | $1,300 / $3.00 |
| 2023 | 77 | (4.6) | 5.0 | 12,391 | 728 | 105 | 6.7 | $1,300 / $3.00 |
| 2024 | 89 | (4.6) | 17.0 | 40,201 | 28,542 | 176 | 79 (DR-15) | $1,400 / $3.00 |
| 2025 | 85 | (3.6) | 1.7 | 39,361 | 131 (DR-14: should be (131)) | 171 | 1.4 | $1,500 / $3.25 |

**Reserve-replacement bases computed from this table** (pooled = Σ net change / Σ depletion):

| Basis | Value | Note |
|---|---|---|
| Gold, 2020–2024, pooled (endnote 16 definition) | 46.3 / 24.9 = **186%** | consistent with "close to 190%" |
| Gold, 2020–2025, pooled | 168% | |
| Gold, 2023–2025, pooled | 185% | |
| GEO, 2023–2025, pooled | 87.1 / 17.2 = **506%** (rounding envelope 499–514%) | ≈ "3-year average 504%" |
| GEO, 2023–2025, mean of annual ratios | 478% | does not match |
| GEO, 2022–2024, pooled | 536% | |
| GEO, 2025 alone | 27% | |
| Gold, 2025 alone | 47% | |

**GEO price bases found in the corpus** (GB-27):
1. the annual reserve prices above;
2. the GEO cost-metric conversion factors 2019–2025 (`FM` p.13, gold $1,373 … $3,569/oz; copper $6,751 …
   $10,406/t);
3. the outlook GEO at $1,400/oz and $3.00/lb (`LUM` p.40);
4. `TZ` p.28 GEO 2024 at $2,352/oz and $4.16/lb.

DR-15 shows two bases mixed inside one table.

---

## 4. Plan impacts, ranked by value

**1. §37.3 M0 artifact, BD-055/BD-059. DATA-GATE + CORRECTION.**
- **Evidence.**
  - The corpus's only per-site AISC/TCC/grade/recovery source is the two Mine Stats workbooks.
  - They cover **2025Q1–2026Q2** (six period blocks), not Q1 2024–Q3 2026.
  - **There is no per-site GEO field** in any sheet; GEO appears only at company level in decks.
  - The xlsx has no filing accession. Its Endnote points to the filed Quarter Report (p.36 for Q1, p.44 for Q2).
- **Change.** Replace the first M0 artifact bullet with:
  > "per-site AISC/TCC (C1 for copper)/CoS/grade/recovery/capex for every quarter whose Mine Stats workbook is in
  > the source registry (corpus today: Q1 2025–Q2 2026). Every cell cites `<file_id>:<sheet>!<cell>` + sha256 and
  > the filed Quarter Report page that reconciles it. GEO is company-level only, on a named price basis (GB-27).
  > Q1–Q4 2024 and Q3 2026 are DATA-GATE: workbooks not yet in the registry."

**2. BD-057, §31.1. ADDITION.**
- **Evidence.** The Mine Stats are xlsx, not HTML. §3.1 lists the hygiene rules:
  - label anchoring, because of row drift (`Minestats1!A38`);
  - footnote stripping;
  - ownership parsing;
  - a token map (`—`/`n/a`/0/blank);
  - excluding 1,092 `#REF!` residue cells and the 15,903-column `YTD`.
- **Change.** Add a row:
  > "BD-057b | Pure-Rust xlsx table extractor (label-anchored, placeholder-typed, residue-excluding) with
  > `<file_id>:<sheet>!<cell>` provenance | BD-056 | Fixtures: the 3,996 parsed cells of Q1/Q2 2026 Mine Stats
  > reproduce exactly; the 7 zero placeholders import as Nil."

  Also add to BD-055: "the registry entry holds a sha256 for website-published workbooks, which have no accession."

**3. §21.6 item 4, G-BULL-3, GB-26. ADDITION.**
- **Evidence.** DR-12. Total Gold AISC sits **$47.6–55.7/oz** above the sold-weighted regions in all six periods,
  while regions reconcile to 0.1%. NGM sits $8.7–19.4/oz above its four mines. Total Copper sits $0.05–0.11/lb
  above.
- **Change.** Add to item 4:
  > "aggregation layers: company AISC = Σ sold-weighted site AISC + L_corp (Barrick Total Gold: +$48–56/oz in
  > 2025–H1 2026, INFERENCE from Mine Stats) + L_JV (NGM: +$9–19/oz). Each is a named layer citing the Quarter
  > Report reconciliation, or the gap is refused under G-BULL-3."

**4. GB-26 and BD-064 (LMDI). CORRECTION.**
- **Evidence.** Public AISC is per oz **sold**. Sold/produced runs from 0.81 (North Mara 2025Q2) to 1.26 (Lumwana
  2025Q1, copper).
- **North Mara Q2 2025 → Q2 2026 (+$1,285/oz):**
  - grade +$1,047;
  - cost +$725;
  - **sold/produced −$504**;
  - metal balance −$75.
- Without an S factor the bridge does not close.
- **Change.** Replace the GB-26 identity with:
  > "AISC = C / oz_sold, with oz_sold = T·g·R·B·S. B = produced / (T·g·R), the metal-balance and in-circuit
  > inventory factor. S = sold / produced, the finished-goods inventory factor. Both are named LMDI effects. The
  > public lane computes them from Mine Stats rows (processed, grade, recovery, produced, sold)."

**5. §21.6 public lane, GATE-BULL-01, BD-065. ADDITION (scoring definition).**
- **Evidence.**
  - Barrick's own identities fail strict rounding in about 5% of cases: T1 84/89 = 94.4%.
  - T5 shows 3 Q1 + Q2 ≠ H1 breaks.
  - The 480-cell M0 census has 53 non-numeric cells.
- **Change.** Add to GATE-BULL-01:
  > "Denominator = numeric site-quarter cells only (`—`/`n/a`/placeholder excluded and listed). 'Reproduced' =
  > extracted value equals the published cell, and every applicable identity (attributable = 100% × own, Σ
  > components, Q1 + Q2 = H1, sold-weighted roll-up) holds to ±1 unit of the last published digit or is a register
  > item. A register item with a bridge counts as explained, not reproduced."

**6. BD-063, OPEN-BULL-4. VALIDATION (INFERENCE).**
- **Evidence.**
  - The `FM` p.14 table and endnote 16 definition give gold 2020–2024 pooled = **186%**, which fits "close to
    190%".
  - GEO 2023–2025 pooled = **506%** (499–514% under rounding), which fits "3-year average 504%".
  - The mean of annual ratios gives 478% and does not fit.
- **Change.** Add to OPEN-BULL-4:
  > "Candidate bridge: 190% = gold-only, 5-year cumulative (2020–24); 504% = GEO at reported reserve prices, 3-year
  > pooled (2023–25), driven by the 2024 Reko Diq copper addition (+28,542 Mlb). Confirm with the Circular's
  > definition before BD-063 passes."

**7. BD-070, GATE-HALL-01, GB-27. CORRECTION / NEW-IDEA (fixture).**
- **Evidence.**
  - DR-14: the copper 2025 net change is printed positive, which breaks closure by 263 Mlb and overstates the
    total by 262.
  - DR-15: the 2024 GEO row was priced at the 2025 deck.
  - DR-16: the endnote text is stale.
- **Change.** Add to GATE-HALL-01:
  > "First fixture: Barrick roll-forward 2019–2025 (Fourmile deck p.14). The engine must flag the 2025 copper sign
  > and the 2024 GEO price-basis mismatch as discrepancy items with closure arithmetic, never silently correct
  > them."

**8. §37.3 M2 site preference, BD-199. VALIDATION + RISK.**
- **Validation.**
  - Lumwana is 100% (100% = attributable in every period).
  - It has a recorded recovery variance: 89.4% in 2023 and 89.82% in 2024 vs a 93.37% 2013–22 mean, about 5 kt
    Cu/yr.
  - Its public metal balance closes within 1.5 pts.
- **Risk (a): the site says its grade reconciliation is good** ("Reconciliation results confirm the metal
  correlation…", p.19). The variance on record is recovery, not grade.
- **Risk (b): 2026–2028 are non-stationary.**
  - Strip ratio rose from 3.89 to 5.74.
  - Mining ran at 124–166 Mt/y against a 200 Mt H2 2026 milestone.
  - The new 52 Mtpa plant commissions from H2 2027.
- **North Mara** shows the larger grade signal (3.56 → 1.81 g/t; LMDI grade +$1,047/oz) but is 84/16 Twiga.
- **Change.** Append to the M2 site preference:
  > "Lumwana's recorded variance is metallurgical recovery (2023–24, about 4–4.5% of output), and the site
  > reports its grade reconciliation as confirming. Pilot quarters must predate or be segmented from Super Pit
  > commissioning (H2 2027–2028), and the H-f finding mechanism is recovery-led at Lumwana and grade-led at North
  > Mara."

**9. Lodestar §11.7, Helm §23.8, F13. RISK.**
- **Evidence.**
  - "Barrick anticipates Fourmile will be incorporated into the Nevada Gold Mines joint venture, at fair market
    value, if certain criteria are met" (`FM` pp.10, 12).
  - The PEA is on a $1,900/oz MSO while 2025 reserves use $1,500.
- **Change.** Add to §11.7 / §23.8:
  > "Any Fourmile valuation is a potential JV transfer-price input: walled lane, insider-listed, never in
  > NGM-partner-visible outputs. Re-price PEA-basis ($1,900/oz) inventories to the reserve deck before
  > haircutting (§23)."
- **New prior.** Fourmile Indicated went from 1.4 to 2.6 Moz and Inferred from 6.4 to 13 Moz in one year, at
  35 m / 80–90 m target spacing. This is a c_FI prior datum.

**10. BD-058. VALIDATION (partial answer).**
- **Published per site in Mine Stats:**
  - CoS, royalties/oz, depreciation/oz, TCC, AISC;
  - sustaining and project capex (cash basis);
  - cost per tonne mined and processed, and G&A per tonne processed.
- **Not published per site:** G&A $/oz, sustaining leases, minesite exploration, reclamation accretion and
  by-product credits.
- **Change.** Record in CELL-DEC:
  > "Mine Stats give TCC→AISC only as a residual (AISC − TCC − sustaining capex/oz = $5–80/oz 'other'; Bulyanhulu
  > −12 to −14 from cash-vs-accrual). The per-site component split needs the Quarter Report reconciliation (Q1
  > p.36, Q2 p.44): still OPEN."

**11. §23.8 Tier One, §23 price decks, §37 Twiga. VALIDATION.**
- **Tier One.** `LUM` p.41 endnote 1 confirms gold at $1,400/oz, ≥ 10 yr, ≥ 500 koz and copper at $3.00/lb,
  +5 Mt, ≥ 20 yr, ≥ 200 ktpa, as of Sep 2025.
- **Reserve prices.** `FM` p.14 gives $1,200 → $1,300 → $1,400 → $1,500/oz (2020–25).
- **Twiga.** `TZ` p.29 confirms 16% free-carried from 1 Jan 2020.
- No change is needed. Optionally, add "(2020–21 $1,200; 2022–23 $1,300)" to the §23 price-deck sentence.

**12. Hallmark §24. RISK.**
- **Evidence.** DR-13: the deck's reserve components do not sum to its own total.
- **Change.**
  > "Reserve Claims cite the technical report or annual reserve table, never investor-deck endnotes."

---

## 5. Proposed evidence rows (E101–E149)

E101 | Mine Stats Q1 file has sheets QTD, Minestats1–4, Endnote; Q2 adds YTD and six-month blocks; all values hard-coded | Q1_2026, Q2_2026 workbook structure (openpyxl) | "QTD; YTD; Minestats1; Minestats2; Minestats3; Minestats4; Endnote" | FACT
E102 | Summary columns: B 100% production, C attributable production, D attributable sold, E–G CoS/TCC/AISC | Q2_2026:QTD!B4:D4 | "Gold produced (000s ozs 100% basis) … Gold produced (000s ozs attributable share) … Gold sold (000s ozs attributable share)" | FACT
E103 | NGM figures are Barrick's 61.5% of four mines | Q2_2026:QTD!A31; Q2_2026:Minestats1!A52 | "These results represent our 61.5% interest in Carlin, Cortez, Turquoise Ridge and Phoenix." | FACT
E104 | Site capex is cash basis, Barrick share, includes capitalized interest | Q2_2026:Minestats1!A57 (also MS2!A46, MS3!A41, MS4!A32) | "Capital expenditures are presented on a cash basis and are reported at Barrick's share. Includes capitalized interest where applicable." | FACT
E105 | Phoenix grades refer to oxide-mill feed only | Q2_2026:Minestats1!A55 | "At Phoenix, stated grades relate to material slated for processing at the oxide mill." | FACT
E106 | Cost-per-tonne metrics are before stripping capitalization and inventory allocation | Q2_2026:Endnote!A14 | "period operating costs, before capitalization of waste stripping and allocation to inventory, divided by the number of tonnes mined/processed" | FACT
E107 | Non-GAAP reconciliations sit in the filed Quarter Reports: Q1 2026 from p.36, Q2 2026 from p.44 | Q1_2026:Endnote!A5; Q2_2026:Endnote!A5 | "provided starting on page 44 in the Second Quarter 2026 Report filed on SEDAR+ … and on EDGAR" | FACT
E108 | AISC components per Barrick Endnote #1 | Q2_2026:Endnote!A5 | "includes minesite sustaining capital expenditures, sustaining leases, general and administrative costs, minesite exploration and evaluation costs and reclamation cost accretion and amortization" | FACT
E109 | Minestats physicals are attributable share: NM Q1 2025 (1,057 + 2,398)/0.84 = 4,113 vs 4,112 kt (100%); 672/0.84 = 800.0 vs 800; Bulyanhulu 304/0.84 = 361.9 vs 362 | Q1_2026:Minestats3!I5, I6, I12, K7; TZ p.8, p.10 | "North Mara operational performance… Open Pit Mining (kt)" | INFERENCE (ratio to 100%-basis deck values)
E110 | Unit costs are per attributable unit sold: sold-weighted site costs reproduce every 2026 regional CoS/TCC/AISC within 0.1% | QTD rows 12, 15, 21 (both files); FM p.13 endnote 10 | "Gold sold (000s ozs attributable share)" | INFERENCE (T3)
E111 | Total Gold AISC exceeds sold-weighted regions by $55.36 / $55.71 / $55.46 per oz (2026Q1 / Q2 / H1), $53.11 / $47.64 / $49.11 (2025) | Q1_2026:QTD!G22 = 1708; Q2_2026:QTD!G22 = 1866; Q2_2026:YTD!G22 = 1790 | "All-In sustaining costs ($/oz)1" | INFERENCE (T3)
E112 | NGM AISC exceeds the sold-weighted AISC of its four mines by $8.69–19.38/oz | Q1_2026:QTD!G9 = 1645; Q2_2026:QTD!G9 = 1843; Q2_2026:YTD!G9 = 1745 | "Nevada Gold Mines LLC (61.5%)2" | INFERENCE (T3)
E113 | Carlin Q1 2026 100% value breaks two identities; 281 satisfies both | Q1_2026:QTD!B5 = 283, C5 = 173; Q2_2026:QTD!B5 = 280; Q2_2026:YTD!B5 = 561 | "Carlin (61.5%)" | INFERENCE (283 × 0.615 = 174.0; 283 + 280 = 563)
E114 | Turquoise Ridge H1 2026 100% production 281 ≠ Q1 + Q2 (278) and ≠ attributable/0.615 | Q2_2026:YTD!B7 = 281, C7 = 171; Q1_2026:QTD!B7 = 148; Q2_2026:QTD!B7 = 130 | "Turquoise Ridge (61.5%)" | INFERENCE
E115 | Jabal Sayid 100% values break Q1 + Q2 = H1 (32 vs 30) and the 50% identity (14 vs 6) | Q2_2026:QTD!B28, C28; Q1_2026:QTD!B28; Q2_2026:YTD!B28 | "Jabal Sayid (50%)" | INFERENCE
E116 | Hemlo H1 2026 CoS 1691 is a copy of Pueblo Viejo's (Hemlo sold 26 Nov 2025) | Q2_2026:YTD!E11 = 1691, E10 = 1691; Q2_2026:Minestats1!N103 = '—' | "The transaction closed on November 26, 2025." | INFERENCE
E117 | Total Copper recovery is 94% in all six periods; site recoveries are 90–92% (Zaldívar n/a) | Q1_2026:Minestats4!B14, C14; Q2_2026:Minestats4!B14, C14, B51, C51 | "Recovery rate (percent)" | INFERENCE (not reproducible by any weighting)
E118 | Capex total exceeds sustaining + project in 19 of 101 triples (+1 to +15 $M), largest at Lumwana | Q1_2026:Minestats4!D26:D28; Q2_2026:Minestats4!D28:D30, D65:D67 | "Includes capitalized interest where applicable." | INFERENCE (residual attributed to capitalized interest)
E119 | Divested Hemlo and Tongon recovery cells hold numeric 0 shown "0%" while siblings are '—' | Q1_2026:Minestats1!N27, N28; Q1_2026:Minestats3!F19; Q2_2026:Minestats1!N27, N28; Q2_2026:Minestats3!F19, F64 | cell value 0, format "0%" | FACT (cell) / INFERENCE (placeholder)
E120 | Q2 workbook carries 1,092 broken formula cells and a 15,903-column YTD dimension | Q2_2026:Minestats3!M4:V81; Q2_2026:Minestats4!K4:R67; Q2_2026:YTD (A1:WMQ42) | "=B4-#REF!" | FACT
E121 | Q2 adds a Revenue row, shifting CoS from row 38 to row 40 (Minestats1) | Q2_2026:Minestats1!A38; Q1_2026:Minestats1!A38 | "Revenue ($ millions)" | FACT
E122 | Loulo-Gounkoto footnote says no 2025 operating data, but Q1 2025 cells hold 18 koz attributable, 169 kt processed | Q1_2026:QTD!A34, I16, J16; Q1_2026:Minestats3!C12, C20 | "no operating or per ounce data was provided for Q1 2025 to Q3 2025" | FACT (both) / INFERENCE (conflict; likely 1–14 Jan 2025)
E123 | Lumwana is reported at 100%: production 100% = attributable every period; label shows no % | Q2_2026:QTD!A27, B27 = 41, C27 = 41; Q1_2026:QTD!B27 = C27 = 32 | "Lumwana" | INFERENCE
E124 | Lumwana H1 2025 deck figures equal the Mine Stats H1 2025 comparatives | LUM p.9; Q2_2026:YTD!I27 = 71, L27 = 2.51, M27 = 1.88, N27 = 2.98 | "Copper Produced (kt) 70.6 … Cost of Sales ($/lb) 2.51 … C1 … 1.88 … All-in Sustaining Costs ($/lb) 2.98" | FACT
E125 | Lumwana 2009–2024 history: 352.1 Mt milled @ 0.57%, 1,844,683 t Cu, 91.84% recovery | LUM p.39 | "Total 352,133,027 0.57 2,008,485 1,844,683 91.84" | FACT
E126 | Lumwana recovery fell to 89.4% (2023) and 89.82% (2024) vs a 93.37% 2013–22 mean; shortfall 5,269 and 4,850 t Cu | LUM p.39 | "2023 26,796,553 0.49 132,023 118,001 89.4" | INFERENCE (feed × 0.9337 − produced)
E127 | Lumwana quarterly metal balance closes: implied recovery 0.904–0.927 vs stated 0.91–0.92 (2025Q1–2026Q2) | Q1_2026 / Q2_2026:Minestats4!D/E rows 9, 13, 14, 15 | "Recovery rate (percent)" | INFERENCE (T9)
E128 | Super Pit: 240 ktpa from 2028; 52 Mtpa (54 peak); LOM 7.7 Mt Cu 2025–2057; ~US$2.0bn initial capital | LUM p.23 | "expansion on track to produce an average of 240kt of copper p.a. from 2028" | FACT
E129 | Super Pit LOM unit costs and returns (consensus Sep 2025 prices) | LUM p.23 | "Cost of Sales: ~$2.37/lb … C1: ~$1.59/lb … AISC: ~$2.51/lb" | FACT
E130 | Lumwana states its grade reconciliation confirms the geological model | LUM p.19 | "Reconciliation results confirm the metal correlation between geological model and production" | FACT
E131 | Lumwana milestones: 200 Mt mining run rate H2 2026; commissioning H2 2027–H1 2028; 52 Mtpa ramp-up H2 2028 | LUM p.28 | "200Mt Mining Run Rate … Commissioning Starts 215Mt Mining Run Rate … 52Mtpa Plant Ramp Up" | FACT
E132 | Lumwana mined 31.1 and 41.5 Mt in 2026Q1/Q2 (≈124 and 166 Mt/y); strip 4.37 → 5.74 | Q1_2026:Minestats4!D4, D8; Q2_2026:Minestats4!D4, D8 | "Tonnes mined (thousands)" | FACT (cells) / INFERENCE (annualised ×4)
E133 | Lumwana reserve endnote is internally inconsistent: 1,500 Mt × 0.53% = 7.95 Mt, not 8.3; components 8.64 Mt vs P&P 8.3 | LUM p.41, p.8 | "probable reserves of 1,500million tonnes grading 0.53% representing 8.3 million tonnes of copper" | INFERENCE
E134 | Barrick Tier One copper definition as of Sep 2025 | LUM p.41 endnote 1 | "A Tier One Copper Asset/Project is an asset with a $3.00/lb reserve with potential for +5Mt contained copper" | FACT
E135 | Twiga: GoT 16% free-carried interest in North Mara and Bulyanhulu from 1 Jan 2020 | TZ p.29, p.30 | "the GoT acquired a 16% freecarried interest in these sites, effective as of January 1, 2020" | FACT
E136 | North Mara processed grade halved and AISC roughly doubled year on year | Q1_2026:Minestats3!I18 = 3.56, H18 = 1.81, I29 = 1258, H29 = 1990; Q2_2026:Minestats3!I18 = 3.16, H18 = 1.8, I31 = 1292, H31 = 2577 | "North Mara (84%)" | FACT
E137 | NM AISC +$1,285/oz (Q2 2025 → Q2 2026) decomposes exactly into grade +1,047, cost +725, recovery +65, tonnes +27, metal balance −75, sold/produced −504 | Q2_2026:Minestats3!H/I rows 12, 18, 19, 20, 21, 31 | "All-in sustaining costs" | INFERENCE (LMDI, log-mean weights)
E138 | Sold/produced ranges 0.81–1.26 across sites and quarters | Q2_2026:QTD!J19/K19 (NM 2025Q2 62/50); Q1_2026:QTD!J27/K27 (Lumwana 2025Q1 27/34); Q2_2026:QTD!C13/D13 (Veladero 36/45) | "Gold sold (000s ozs attributable share)" | INFERENCE
E139 | Fourmile is expected to move into NGM at fair market value | FM p.10, p.12 | "Barrick anticipates Fourmile will be incorporated into the Nevada Gold Mines joint venture, at fair market value, if certain criteria are met." | FACT
E140 | Fourmile resource doubled: Indicated 1.4 → 2.6 Moz, Inferred 6.4 → 13 Moz (Dec 2024 → Dec 2025), 100% Barrick | FM p.5, p.4 | "4.6Mt @ 17.59 g/t … 2.6Moz … 25Mt @ 16.9 g/t … 13Moz" | FACT
E141 | Fourmile PEA uses a $1,900/oz MSO, above the $1,500 reserve deck | FM p.10; FM p.14 | "economic assessment for Fourmile is based upon $1,900/oz mineable stope optimizer" | FACT
E142 | Fourmile drill spacing targets | FM p.6 | "Targeting 35m spacing for Indicated and 80-90m for Inferred" | FACT
E143 | Barrick reserve prices by year: gold $1,200 (2020–21), $1,300 (2022–23), $1,400 (2024), $1,500 (2025); copper $2.75 / $3.00 / $3.00 / $3.25 | FM p.14 | "Reported Reserve Price USD/oz for GEO conversion" | FACT
E144 | Copper roll-forward 2025 net change printed +131; closure needs −131 (gap −263 Mlb); total 30,252 should be 29,990 | FM p.14 (render checked) | "2025 … 39,361 - (708) 131 $3.25" | INFERENCE (40,201 − 708 − 131 = 39,362)
E145 | Gold reserve replacement 2020–2024 pooled = 46.3/24.9 = 186%, matching "close to 190%" | FM p.14 endnote 16 | "calculated from the cumulative net change in reserves from 2020 to 2024 divided by the cumulative depletion" | INFERENCE
E146 | GEO reserve replacement 2023–2025 pooled = 87.1/17.2 = 506% (499–514% under rounding), matching "504%" | FM p.14 | "Attributable Net Change GEO … (using reported reserve prices)" | INFERENCE
E147 | GEO 2024 net change 79 is not reproducible at the row's $1,400/$3.00 (78.16) but is at $1,500/$3.25 (78.84) | FM p.14 | "2024f 176 - (6.1) 79" | INFERENCE
E148 | Public per-site data in the corpus cover 2025Q1–2026Q2 only; no 2024 or Q3 2026 workbook; no per-site GEO field | Q1_2026, Q2_2026 (all sheets) | "For the six months ended June 30," | FACT (checked)
E149 | Mine Stats name the QPs who approved the technical information; the reserve as-of date is 31 Dec 2025 | Q2_2026:Endnote!A2 | "each a “Qualified Person” as defined in National Instrument 43-101" | FACT

---

## 6. Checked nulls

- **Per-site GEO.** It is absent from every Mine Stats sheet. GEO appears only as company-level price bases in
  `FM` p.13, `LUM` p.40 and `TZ` p.28.
- **Per-site AISC components** (G&A, sustaining leases, exploration, reclamation, by-product credits). None are in
  the workbooks or decks. The Quarter Report reconciliation is the next source (BD-058).
- **2024 quarters and Q3 2026.** There is no workbook in the corpus. The decks give some 2024 site values
  (Lumwana annual 2024; NM and Bulyanhulu Q1–Q4 2024 at 100% on `TZ` pp.8, 10), but no AISC/TCC.
- **Lumwana monthly or plant reconciliation data.** Only the p.19 claim. No F1/F2/F3 factors, no
  grade-control-vs-mill table. Charts on pp.10, 24 and 25 have no data labels.
- **Fourmile costs.** There is no AISC, capex or recovery figure. The PEA economics are not disclosed in the deck.
- **`MA21`.** p.5 UG ore and development labels are hidden in the render. The 2020 production totals add nothing
  for M0.
- **Contradictions with plan text on Barrick.** None found:
  - Twiga 84/16, Lumwana 100% and the NGM 61.5/38.5 split all match.
  - The Tier One and reserve-price statements match.
  - The plan's FY2025 API and AISC figures cannot be checked here, because there is no FY data in the corpus.
- **Reko Diq, Kibali, Pueblo Viejo deck data.** No such decks are in this assignment's documents. Only the Mine
  Stats rows (§3.4) cover these sites.
