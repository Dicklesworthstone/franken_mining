# C2 — Nevada Gold Mines technical reports (Carlin 2025, Cortez 2022, Turquoise Ridge 2024)

Agent C2, evidence block E151–E199. Read-only pass, 2026-10-28.

**Ownership basis (applies everywhere below).** Nevada Gold Mines LLC (NGM) is a JV: Barrick 61.5% (operator),
Newmont 38.5%. **Every figure in this file is a 100% NGM-basis figure unless the row says "Barrick attributable
(61.5%)".** All three reports are Barrick-issued NI 43-101 reports signed by NGM-employed QPs. NGM is
JV-sensitive in the plan (E034, F13). Nothing below uses partner-only data.

**Page convention.** "p.N" means the PDF page, the same as the `<!-- p.N -->` anchor in the markdown mirror. The
printed page number in the footer is lower by 1 (Carlin) or by 1–2 (Cortez, Turquoise Ridge).

---

## 1. RESULT_STATUS: MATERIAL_INCREMENT

---

## 2. Inventory

| Document | Effective date / signed | What it contains (PDF pages) | Extraction usability |
|---|---|---|---|
| `Carlin_Complex_TechnicalReport_14032025.pdf` (338 pp) | Effective 31 Dec 2024; dated 14 Mar 2025 | §1 summary p.13–29. §6 history and production p.63–65. §10 drilling (twin and spacing studies) p.93–106. §11 QA/QC with counts p.107–120. §12 data verification and RSC audit p.121–123. §13 metallurgy (blend targets, recovery equations) p.124–151. §14 resources (domaining, OK, classification, statements, 2023→2024 bridge, RSC audit) p.152–196. §15 reserves (dilution, costs, BCOG, pseudoflow, **2024 plant reconciliation**) p.197–222. §16 mining, geotechnics (GQ7CN failure), dewatering, LOM p.223–261. §17 plants p.262–281. §18 TSFs and water p.282–293. §20 closure and bonds p.296–308. §21 capex/opex p.309–311 | Text is good. Tables are mostly clean. **Garbled:** UG CAPEX column of Table 15-4 (markdown shows "$707"; PDF says $7.07); all figure-embedded data (monthly reconciliation charts, swath plots, variograms) is OCR noise. |
| `Barrick_Cortez_Technical_Report_2022.pdf` (304 pp) | Effective 31 Dec 2021 | §11 QA/QC rules and history p.144–157. §12 data verification and bias studies p.158–160. §13 recovery equations (oz/st) p.161–174. §14 resources by deposit (ID2/ID3/OK, classification) p.175–205. §15 reserves, BCOG tables, stockpile tracking p.206–218. §16 grade control and ore control p.219–259. §18 TSF and water p.269–278. §20 closure p.280–284. §21 costs p.285–288 | Good text. Tables are mostly clean, with some split cells. **Stale:** four years older than the Carlin report and superseded by later Barrick statements. Use for method evidence, not as current reserves. |
| `Turquoise-Ridge-Technical-Report-March2024.pdf` (305 pp) | Effective 31 Dec 2023; dated 15 Mar 2024 | §11 QA/QC rules (no counts) p.121–130. §12 p.131–132. §13 metallurgy: preg-rob variability, **BTAC-vs-plant monthly lots 2008–2014**, TCM/Au recovery equations, blend constraints p.133–149. §14 resources (OK, dynamic anisotropy, COG fixed point) p.150–195. §15 reserves (acid cost/credit routing, formula versions) p.196–214. §16 mining p.215–246. §18 TSF and water p.254–264. §20 closure p.267–271. §21 costs p.272–274 | Good text. Variogram and capping figures are OCR noise. Statement tables are clean. |

---

## 3. Demonstrator data (all verified against the PDF page)

### 3.1 Reserve and resource statements (Hallmark §24 demo)

All grades are in g/t Au and metal is contained Moz Au (before process recovery). Resources are inclusive of
reserves.

| Site | Statement | Category | Mt | g/t | Moz | Basis | Price | Period | Doc p. |
|---|---|---|---|---|---|---|---|---|---|
| Carlin | Resource | M+I | 230 | 3.43 | 25 | 100% | $1,900/oz | 31 Dec 2024 | C p.191 |
| Carlin | Resource | Inferred | 78 | 3.7 | 9.3 | 100% | $1,900 | 31 Dec 2024 | C p.191 |
| Carlin | Resource | M+I | 140 | 3.43 | 15 | **Barrick attrib. 61.5%** | $1,900 | 31 Dec 2024 | C p.192 |
| Carlin | Reserve | P+P | 130 | 3.62 | 15 (attrib. 9.5) | 100% (attrib. column given) | $1,400 | 31 Dec 2024 | C p.20 |
| Carlin | Reserve | Proven | 6.7 | 1.66 | 0.36 | 100% | $1,400 | 31 Dec 2024 | C p.20 |
| Carlin | Reserve | UG P+P | 32 | 7.69 | 7.9 | 100% | $1,400 | 31 Dec 2024 | C p.20 |
| Turquoise Ridge | Resource | M+I | 110 | 5.42 | 20 | 100% | $1,700 | 31 Dec 2023 | T p.193 |
| Turquoise Ridge | Resource | Inferred | 16 | 3.2 | 1.6 | 100% | $1,700 | 31 Dec 2023 | T p.193 |
| Turquoise Ridge | Reserve | P+P | 69 | 6.29 | 14 (attrib. 8.6) | 100% (attrib. column given) | $1,300 | 31 Dec 2023 | T p.198 |
| Turquoise Ridge | Reserve | UG P+P | 33 | 10.66 | 11 | 100% | $1,300 | 31 Dec 2023 | T p.198 |
| Cortez | Resource | M+I | 210 | 2.75 | 18 | 100% | $1,500 | 31 Dec 2021 | Z p.203 |
| Cortez | Resource | Inferred | 120 | 1.6 | 6.4 | 100% | $1,500 | 31 Dec 2021 | Z p.203 |
| Cortez | Reserve | P+P | 110 | 4.17 | 14 | 100% | $1,200 | 31 Dec 2021 | Z p.216 |
| Cortez | Reserve | Goldrush P+P | 33 | 7.29 | 7.8 | 100% | $1,200 | 31 Dec 2021 | Z p.216 |

Statement conventions worth encoding:
- Reserve tonnes and grades are "ROM grades and tonnage delivered to the primary crushing facilities" (C p.19–20).
- Tonnes and ounces are rounded to two significant digits. Grades take two decimals for M&I and one decimal for
  Inferred (C p.191).
- Cortez notes that ounces "do not include allowances for processing losses" (Z p.204).
- **Price-deck drift across the three statements:** reserves $1,200 (2021) → $1,300 (2023) → $1,400 (2024);
  resources $1,500 → $1,700 → $1,900.

### 3.2 Reconciliation: Carlin 2024, plant level (Touchstone §20)

The only quantitative reconciliation in the three reports is three annual 2024 tables on a 100% basis
(C p.216–218). Monthly values exist only as charts (Figures 15-11 to 15-16) and cannot be extracted as numbers.

| Line | GS Roaster Mt / g/t / koz | GQ Roaster Mt / g/t / koz | GS Autoclave Mt / g/t / koz |
|---|---|---|---|
| Mined | 5.38 / 6.09 / 1,053 | 2.86 / 6.53 / 600 | 3.83 / 1.44 / 178 |
| Stockpile change (feeder pad only) | −0.03 / 0.55 / −1 | −0.01 / −0.19 / −2 | 0.41 / −0.86 / −10 |
| GC theoretical feed | 5.41 / 6.22 / 1,054 | 2.86 / 6.72 / 601 | 3.42 / 1.71 / 188 |
| GC adjustment | −0.15 / −0.27 / −49 | 0.07 / −0.15 / 20 | 0.29 / 0.08 / 25 |
| GC call | 5.25 / 5.95 / 1,006 | 2.94 / 6.57 / 621 | 3.71 / 1.79 / 213 |
| Mills check-in | 5.74 / 5.68 / 1,049 | 2.92 / 6.50 / 610 | 3.67 / 1.72 / 203 |
| Mills check-out | 5.74 / 5.59 / 1,031 | 2.92 / 6.37 / 598 | 3.67 / 1.76 / 208 |
| **"MCF out"** (tonnes / grade / metal) | **109% / 94% / 102%** | **99% / 97% / 96%** | **99% / 98% / 97%** |

- **Definition (INFERENCE, from the table).** MCF out = Mills check-out ÷ GC call, for each of tonnes, grade and
  metal. For example, 5.74/5.25 = 1.093, 5.59/5.95 = 0.939 and 1,031/1,006 = 1.025.
- **Scope (FACT, C p.216).** The plant totals include third-party ores from Cortez and concentrates from Golden
  Sunlight and Phoenix. Sources are only back-allocated.
- **What is missing.** No resource-model-vs-GC (F1) figure is disclosed at any site. Cortez and Turquoise Ridge
  give no reconciliation numbers at all; Turquoise Ridge says only "acceptable reconciliation results on a monthly
  and quarterly basis" (T p.107).

### 3.3 QA/QC programme statistics: Carlin, 1 Jan 2022 – 31 Dec 2024 (Corebox §12, Hallmark §24.5)

| Item | Value | Doc p. |
|---|---|---|
| QA/QC share of sample stream | ≈20% (blanks, duplicates, CRMs, check assays) | C p.112 |
| Target insertion | CRM 5% (1:20); coarse blank 5%; RC field duplicate 5%; coarse duplicate 2.5%; pulp duplicate 2.5%; umpire 5% | C p.112 |
| CRMs | 14,182 submitted (4.38% achieved vs 5% target); 375 failures (2.64%); 2,122 routine re-assays | C p.113 |
| CRM failure rule | outside ±3 SD, **or** two consecutive outside ±2 SD on the same side | C p.113 |
| Re-assay window | <0.5 ppm: geologist discretion; 0.5–3 ppm: CRM ± 5 routine samples (max 11); >3 ppm: CRM ± 10 (max 21) | C p.113 |
| Per-lab CRM failures (OREAS 264, 0.307 ppm) | ALS 45/940; BVL 1/56; AAL 0/40; GS internal 12/94; **GQ internal 10/37** | C p.113 |
| Blanks | 14,365 (4.44%); 440 (3.06%) >5×DL; 23 (0.16%) >1% carryover | C p.115 |
| Duplicates | 4,966 field, 4,557 coarse, 2,626 pulp (3.76% overall). Acceptance: 80% of coarse within ±20%, 90% of pulp within ±10%. Achieved 96% coarse and 95% pulp | C p.116–117 |
| Umpire | 16,282 (5.03%); total bias −0.3%; acceptance: bias and RMA error < ±5% | C p.118 |
| QP caveat | Samples from the internal production lab are fit for purpose, but sending them to an independent accredited lab "would be a better practice" | C p.120 |

Turquoise Ridge states the same rules but gives **no counts** (T p.127–128). Cortez states rules and history and
also gives no counts (Z p.152–154).

### 3.4 Recovery cards as NGM actually models them (Crucible §17)

| Plant / source | Equation or value (as published) | Input units | Doc p. |
|---|---|---|---|
| Goldstrike Roaster, "All Others" and West Leeville | 92.03 − 37.36·e^(−12.94·(Au·0.029167)) | Au in g/t, converted inside to oz/st | C p.144 |
| GS Roaster, Turf and Four Corners | 87.53 − 37.36·e^(−12.94·(Au·0.029167)) | same | C p.144 |
| GS Autoclave, acid POX-CIL | Au > 45: 96.23. 9.6–45: cubic in Au·0.029167 + 83.48. ≤9.6: cubic + 66.344 | g/t → oz/st | C p.146 |
| GS Autoclave, alkaline POX-CIL | Goldstar 50%. Au > 45: 88.14. Otherwise as acid with intercepts 75.387 and 58.254 | g/t → oz/st | C p.146 |
| GQ Roaster | Fixed by source: Exodus 84.0, Gold Quarry 85.4, West Leeville/Turf 85.1, Rita K 85.38. Four Corners: tails model in Au and As (see §4 item 3c) | g/t, As ppm | C p.147 |
| Sage autoclave, TRUG | −155.7·(TCM/Au) + 100.79 | Au g/t, TCM % | T p.145 |
| Sage, TR Surface | −224.6·(TCM/Au) + 100.79 (Section 13); **+97.14 (Vista 8 reserve) and +101.79 (Cut 40 reserve)** | same | T p.145, p.207–208 |
| Sage, Vista UG | −457.0·(TCM/Au) + 97.68 | same | T p.145 |
| Juniper oxide mill | Mega: 0.0754·ln(Au) + 0.72708. Vista: 0.919 − 0.06514/Au (outputs are fractions though labelled "%") | g/t | T p.145 |
| Cortez oxide mill | e.g. CHUG: 27.6·AAFA − 42.5·S2S + 68.4 | **oz/st** (stated) | Z p.172 |
| Cortez ore at Goldstrike Roaster | ((92.027536 − 37.35906·EXP(−12.94386·grade))/100.0) + [Arsenic Impact]; As impact (>1,200 ppm) = −4e-7·As² − 0.0005·As + 1.176 | **oz/st**, As ppm | Z p.173 |
| LOM averages | Carlin 83.5% (132.5 Mt, 15.43 Moz contained → 12.89 Moz recovered, 2025–2044); TRUG 91.24%; TR Surface 68.20%; Vista UG 92.15% | — | C p.260; T p.144 |

**Blend constraints (hard feed windows), 100% basis:**
- GS Roaster: SS 0.8–1.8%, CO3 >14%, TCM 0.8–1.2% (optimum 0.95), As <1,800 ppm (C p.143).
- GQ Roaster: SS 1.4–2.5%, CO3 10–18%, TCM 0.4–0.75%, As <1,800 ppm, Hg <25 ppm (C p.143).
- GS Autoclave: preg-rob <40% (Goldstrike, South Arturo sources) or <60% (Gold Quarry, Goldstar) (C p.143).
- Sage: SS 2.6–2.9%, CO3 3.5–4.3%, TCM <0.50%, As <10,000 ppm, Sb <500 ppm, Fe/As >3.5, Ag <10 g/t (T p.147).
- Sage annual caps by metallurgical type, e.g. O (TCM >1%) <120,000 t/y and A <1,200,000 t/y (T p.148).

**Lab-to-plant calibration set (T p.137–139, Table 13-2).** There are 83 monthly TRUG lots (#54–#136, Feb 2008 –
Dec 2014): 71 have both a BTAC (bench-top autoclave) and a plant recovery, and 12 are plant-only. Derived
statistics (INFERENCE):
- plant − BTAC has mean **+2.35 pts**, SD 4.49 and median +1.7;
- 53 of 71 lots fall within ±5 pts, **17 lots are > +5** and 1 lot is < −5 (max +15.2);
- the published "difference" column was checked against plant − BTAC for every row (all within 0.15).

### 3.5 Cut-off grades and their inputs (Pushback §15)

| Item | Value | Basis / note | Doc p. |
|---|---|---|---|
| Carlin in-pit BCOG | GQ Roaster 1.08; GS Roaster 1.05–1.09; Autoclave refractory 1.37–1.51; Autoclave oxide 0.52; Leach 0.27 g/t | $1,400/oz | C p.213 |
| Carlin UG SO-COG (stope-optimizer seed, "not traditional breakeven") | LHS 3.15 (Exodus) to 5.19 (Goldstrike UG); D&F 4.67–6.61 g/t | Pseudoflow selects the stopes | C p.213 |
| Carlin resource COG | OP non-refractory 0.21–0.36, refractory 0.78–0.80; UG 3.13–6.88 g/t | $1,900 | C p.189 |
| TRUG resource COG | 5.66 g/t from total cost $243.89/t, recovery 79.6% at COG (TCM 0.771%), AB495 1.02%, refining $0.38/oz | $1,700 | T p.168 |
| CHUG BCOG | C&F 0.118 / 0.152 oz/st (oxide / roaster) = 4.05 / 5.21 g/t; LHS 0.099 / 0.133 oz/st = 3.39 / 4.56 g/t | $1,200; "cut-off grades are calculated in US customary units" | Z p.212 |
| Goldrush BCOG | 0.117 oz/st = 4.0 g/t; total operating cost $133.61/t (includes $24.31/t transport to a roaster) + UG sustaining $1.71/t; recovery 88.6%; royalty $19.76/oz | $1,200 | Z p.214 |
| Carlin revenue | Price × recovered oz × (1 − royalty% − AB495%) − refining. Refining and selling $0.38/oz; G&A 10.9% of direct opex | — | C p.209–210 |
| AB495 effective rates (Carlin) | 0.68% (South Arturo) to 1.07% (GQ, Leeville, Exodus, Pete Bajo, Rita K) | Tiered: first $20M exempt, 0.75%, then 1.1% | C p.57 |

**Reproduction tests (INFERENCE; script in the C2 scratchpad):**
- **TRUG:** solving g = 243.89 / (net$/g × R(g)) with R(g) = 100.79 − 155.7·0.771/g gives **g = 5.665 g/t and
  R = 79.6%**. The published cut-off (5.66) is the fixed point of a grade-dependent recovery.
- **GQ Roaster in-pit BCOG:** 41.20 / ((1,400·(1 − 0.0107) − 0.38)/31.1035 × 0.854) = **1.084 → 1.08 g/t**.
- **Cortez:** (opex + sustaining) $/t × 0.90718 / ((price − royalty $/oz) × recovery) reproduces 0.118, 0.152,
  0.099 and 0.117 oz/st exactly; CHUG LH roaster gives 0.1336 against a published 0.133. The metric values are
  the rounded oz/st × 34.2857, so they are double-rounded. AB495 (0.96%, listed in the text) is **not** in the
  BCOG arithmetic.

### 3.6 Mining factors and unit costs (Pushback §15, Bullion §21, Helm §23)

| Item | Value | Basis | Doc p. |
|---|---|---|---|
| Carlin OP dilution | none beyond the SMU block; "No additional mining recovery or dilution assumptions are applied" | — | C p.208 |
| Carlin UG dilution / recovery | e.g. Goldstrike UG 10% dilution with 93% recovery (stopes) and 85% (floor pull); Leeville secondary stope 3–12% dilution, 95% recovery; development 5–16% dilution, 99% recovery | by method and mine | C p.209 |
| TRUG dilution / recovery | LHS primary 3.0% / 95%; secondary 10.0% / 95%; undercut 3.5% / 100%; topcut 4.7% / 100%; development 12% / 100%. **Dilution carries 0 g/t Au but in-situ TCM** | — | T p.201 |
| Cortez Hills UG | C&F: 5% handling dilution + CRF dilution 3/5/10%. LH recovery 95% (primary), 97% (secondary) | — | Z p.211 |
| Carlin processing cost | GS Roaster $30.43/t; GQ Roaster $41.20/t; GS Autoclave refractory $48.11/t, oxide $15.41/t; Leach $9.35/t (opex + capex + G&A) | 100%, $/t processed | C p.210 |
| Carlin UG mining cost | West Leeville LHS $107.88/t, D&F $157.87/t; Goldstrike UG LHS $148.35/t, D&F $214.42/t; Exodus LHS $74.45/t | $/t ore | C p.211 |
| Carlin OP mining cost | $3.85/t mined (opex 3.17 + capex 0.34 + G&A 0.34) | $/t mined | C p.211 |
| Carlin LOM capex | $2,429.3M: sustaining 865.5, capital drilling 40.3, OP stripping 779.9, UG development 743.6 | 100%, real Q4 2024 | C p.309 |
| Carlin LOM opex | $12,558.3M: OP 1,746.8, UG 5,134.8, processing 3,841.0, G&A 1,650.5, production taxes 180.3, freight and refining 4.9 | 100% | C p.310 |
| TR LOM capex / opex | $1,083M / $10,149M; UG $137.48/t; Sage $41.20/t; freight and refining **$95M** | 100%, real Q4 2023 | T p.272–273 |
| Cortez LOM capex | $2,019.7M (includes Goldrush) | 100% | Z p.286 |
| Reserve price sensitivity | TRUG reserve Moz: 7.89 at $1,000/oz; 11.20 at $1,300; 12.57 at $1,500 | 100% | T p.202 |

### 3.7 Tailings, water and closure (Freeboard §18)

| Item | Value | Basis | Doc p. |
|---|---|---|---|
| Carlin NBTDF | Zoned rockfill, downstream; 145 m high; design 198 Mm³, ~169 Mm³ stored | — | C p.290 |
| Goldstrike tailings runway | NBTDF + TSF3 last "through 2028". New 180 Mm³ TSF permitting began in 2023; deposition from 2029 | — | C p.290 |
| Gold Quarry TSFs | 56 West 43.5 Mm³ (≈41 stored); 56 East 71 Mm³ at final configuration | — | C p.290 |
| Turquoise Ridge TSFs | Juniper "upstream centerline", permitted to 159 Mt; Getchell 52 m downstream; Sage TSF planned at 73 Mt | — | T p.262–263 |
| Cortez Cell 4 | Ultimate 56 Mt; 69 m maximum embankment height | — | Z p.275 |
| Carlin dewatering | Goldstrike UG 70,030 L/min now, 117,726 L/min LOM maximum; Gold Quarry 42,396 → 63,689 L/min | — | C p.248 |
| Carlin closure | ~$889M financial assurance posted ($956M pending); trusts and LTMM $13.9M; **IFRS provision ~$328M**; ~2,560 ha (25%) reclaimed; SRCE model | 100% | C p.307–308 |
| Turquoise Ridge closure | Bonds ~$217M; "closure model" cost ~$98M, which feeds the Reserve cash-flow model (not labelled IFRS) | 100% | T p.268 |
| Cortez closure | Bond ~$279M; "calculation model" closure cost ~$183M (not labelled IFRS) | 100% | Z p.280–281 |
| Carlin GISTM status | "four active TSFs" in conformance; closed sites targeted for August 2025; no consequence class stated | — | C p.289–290 |

### 3.8 Discrepancy-register seeds (for GATE-BULL-01 practice)

All four were verified in the PDF; none is an extraction artefact.

| # | Where | What | Check |
|---|---|---|---|
| D1 | Carlin Table 14-22, p.192 (Barrick attributable) | Leeville row: Indicated contained 4.0 Moz but M+I 4.3 Moz for the identical 17 Mt at 7.82 g/t | 17 × 7.82 / 31.1035 = 4.27 Moz, so the 4.0 is wrong. The 100% row is 7.0 × 0.615 = 4.3 |
| D2 | Turquoise Ridge Table 14-16, p.194 (attributable) | Mega Cut 55 Indicated tonnes printed as "1" Mt, while M+I shows 12 Mt and 1.1 Moz | 20 Mt × 0.615 = 12.3 Mt |
| D3 | Carlin Table 6-2, p.65 ("Historic Gold Production", heading "Contained Gold (Moz)") | Columns do not sum to the totals: 2020 rows sum to 3.91 Moz against a total of 1.8. Pete Bajo 0.88 and South Arturo UG 0.62 in 2020 against ~0.09 / ~0.06 in other years. 2021 repeats 2020 for four mines. The 2010–2019 column does sum (26.06 → 26) | Basis is also ambiguous: contained, not recovered, despite the "production" title |
| D4 | Turquoise Ridge Table 14-1 note, p.152 | "Resource block model dimensions of 10 m x 10 m x 10 m" against the stated TRUG parent blocks of 3.0 × 3.0 × 4.6 m (p.153) and the Mega pit's 9.1 × 9.1 × 6.1 m | Note/text inconsistency |

Two further unit traps:
- TR Table 13-4 (p.142) is headed ">g/t Au", but its values (0.090 / 0.075 / 0.040) match the stated g/t classes
  only if read as **oz/st** (INFERENCE).
- The GQ Four Corners equation (C p.147) has **unbalanced parentheses** (three opening, four closing before
  "/(Au·0.029167)") and evaluates to a fraction, not a percentage.

---

## 4. Plan impacts, ranked by value

**1. §13.1 GB-11, §24.4, Part I line 207 (incumbent table): CORRECTION + DATA-GATE.**
- **Evidence.**
  - None of the three reports discloses kriging efficiency or slope of regression (a full-text search found no
    hits).
  - Classification is geometric drill spacing:
    - Carlin: average distance to the nearest 3 holes, converted to a nominal grid (C p.186; spacings p.187);
    - TRUG: Measured ≤7.6 m, Indicated 7.6–61 m, Inferred >61 m (T p.166);
    - Cortez: three-hole distance ÷ 0.7, with Indicated at the variogram range to 80% of sill and Inferred at
      90% of sill (Z p.179, p.182).
  - Estimators at Cortez (2021) were mostly ID2/ID3, and Cortez Hills ID3 "was selected based on previous model
    performance compared to grade control" (Z p.181).
  - The RSC 2024 audit of Carlin recommends more composites "to mitigate the potential for conditional bias"
    (C p.195).
- **Change.** Line 207: replace "Conditional bias is reported as kriging efficiency and slope of regression" with
  "Conditional bias is reported as kriging efficiency and slope of regression **where reported at all**. NGM's
  NI 43-101 reports for Carlin (2025), Cortez (2022) and Turquoise Ridge (2024) report neither; classification
  there is drill-spacing geometry (E158, E162, E163)."
- **Add to GB-11.** "Public filings cannot populate per-domain slopes. A measured slope against GC needs
  blasthole and GC data, which is a pilot data gate. The nearest public practice is estimator choice tuned
  against grade control (E161)."

**2. §20.4 F-factors and §20.1 material-flow graph: VALIDATION + ADDITION + NEW-IDEA.**
- **(a) Register the NGM variant.**
  `NGM.MCF_out = mill check-out ÷ GC call, per tonnes, grade and metal, per plant, per year` (E154–E156). This
  sits closer to F2 or the South African MCF than to F1, and **no F1 is public** (DATA-GATE).
- **(b) The "GC adjustment" line is a booked plug** between theoretical feed (mined − feeder-pad stockpile change)
  and GC call: −49 koz (−4.6%), +20 koz (+3.3%) and +25 koz (+13.3%) (INFERENCE from C p.216–218). Touchstone
  should model it as an explicit latent term (feeder-pad survey and grade error), never absorb it.
- **(c) Extend the compensation detector to tonnes-versus-grade inside one factor.** The Goldstrike Roaster shows
  tonnes 109% and grade 94% netting to metal 102%. The current rule only compares F1 with F2.
- **(d) The material-flow graph must carry cross-complex sources.** Cortez ore and third-party concentrates
  (Golden Sunlight, Phoenix) enter Carlin plants, and "it is not possible to break out individual sources beyond
  what is back allocated" (C p.216). Source attribution is therefore not identifiable from plant data alone; this
  feeds §20.6 identifiability.

**3. §17.3 cards and §17.4 geomet chain: VALIDATION + CORRECTION + ADDITION.**
- **(a) PRI(TOC, domain) is validated.**
  - At identical organic carbon, the preg-rob index ranges from 0–40% to 75–100% depending on source (T p.134).
  - The source effect is "confirmed using Raman spectroscopy" (T p.144).
  - Add an optional `carbon_type` covariate (Raman or source domain) to the PRI card.
  - Carlin estimates TCM, sulphide S, preg-rob, As, Hg, CO3 and cyanide-soluble gold in the block model, with
    "alternate methods" for preg-rob because it is non-linear (C p.180). This validates §13.6.
- **(b) CORRECTION to the validity-enforcement example.**
  - NGM's preg-rob is a percentage from a spike test (T p.134), with feed caps of <40% or <60% by source
    (C p.143).
  - Replace "PRI > 2.1 … no testwork covers PRI > 1.6" with: "Recovery falls below the plan's break-even if
    preg-rob > 60% in the Q3 blend; testwork covers only < 40% for this source."
- **(c) Card ingestion must be unit-aware.** The same company publishes three conventions:
  - Carlin: g/t input converted inside the formula by ×0.029167;
  - Cortez: oz/st input (Z p.172–173), sometimes /100 into a fraction, plus an additive "As impact" term of
    unclear scale;
  - Turquoise Ridge: g/t input, some outputs fractions labelled "%".
  
  One formula is unparseable as printed (unbalanced parentheses, C p.147). Add `input_units`, `output_scale` and
  `parse_check` to the `CorrelationCard` schema, plus a gate that evaluates every imported card at three
  reference grades and compares it with the published LOM average.
- **(d) Card versioning is real and mixed within one statement.** The TR Surface intercept is 100.79 in §13, 97.14
  for the Vista 8 reserve and 101.79 for the Cut 40 reserve (+1% screen upgrade). A "+3% adjustment … based on
  plant CIL improvements" was applied in 2023 (T p.133, p.207–208). Every Reserve element must cite its card
  version.
- **(e) Preg-robbing mitigation in practice.**
  - Blend windows on TCM, SS, CO3 and As (C p.143; T p.147).
  - Annual tonnage caps by metallurgical type (T p.148).
  - Organic carbon (TCM) >1% was "considered waste" when selecting Mega Pit test samples (T p.142). Yet the Sage
    mill still takes ore type O (TCM >1%) under a cap of <120,000 t/y (T p.141, p.148). The same threshold is
    therefore waste in one context and capped feed in another, so a TCM rule must be scoped to its use.
  - Double-refractory ore (sulphide + TCM) routed to roasters, single-refractory to POX-CIL (C p.124).
  - CIL carbon management, where the inter-tank screen change raised recovery (T p.145).
  - Goldstrike's calcium thiosulphate RIL circuit was **converted to CIL in Q1 2023 for $30M** (C p.22).
  
  The plan's generic route list stays correct. **RISK:** any demo text must not present Goldstrike as running
  thiosulphate after Q1 2023.
- **(f) Fitting hygiene as a `ModellingDecision`.** The GS Roaster curve was fitted on 12-hour shifts, dropping
  shifts below 90% availability and their neighbours; "many factors are held constant for LOM" (C p.144).
- **(g) Calibration-ledger demo (§10).** TR Table 13-2 provides 71 registered lab-vs-plant pairs. BTAC is biased
  low (mean +2.35 pts) with a one-sided tail (17 lots > +5). The report attributes this to active TCM and
  halogens in BTAC (T p.139). This makes a ready public dataset for a recovery-forecast calibration card.

**4. §15.2–15.6 Pushback: VALIDATION + NEW-IDEA.**
- **(a) Pseudoflow is the production algorithm** at Carlin (2024) and Turquoise Ridge (2023), for open-pit
  shells and for underground stope net-value selection (C p.213; T p.196, p.202). Blocks are routed to the
  highest-margin destination. Cortez (2021) still used Whittle 4X Lerchs–Grossmann shells (Z p.206). This
  validates the E030 MineFlow parity oracle, and shows the oracle must cover both LG and pseudoflow because the
  algorithm changes between filings.
- **(b) Blend constraints enter pit optimisation as linearised per-block penalties.** An acid cost applies when
  CO3 > 4.3% and an acid credit when CO3 < 3.7%, mirroring the 3.5–4.3% feed window (T p.202). Hard annual caps
  are enforced only in scheduling. §15.6 should support both forms and record which one a plan used.
- **(c) Cut-off is a fixed point when recovery depends on grade or on TCM/Au.** TRUG 5.66 g/t reproduces only as
  the fixed point (E182).
- **(d) Add a "published cut-off reproduction" check to M0** (seven cut-offs reproduced; E182–E184). It catches
  the $/t versus $/st trap: the Cortez cut-offs reproduce only with the 0.90718 conversion, and AB495 is omitted.
- **(e) ADDITION (§15/§17 coupling).** TRUG dilution tonnes carry in-situ TCM at 0 g/t Au (T p.201), so dilution
  raises TCM/Au and lowers recovery. Dilution material must carry its deleterious attributes, not only zero
  grade.
- **(f) Open-pit dilution "inherent in the SMU"** (Carlin and Cortez: C p.208, Z p.211) and 100% mining recovery
  (TR Surface, T p.207). This makes GB-11's change-of-support point load-bearing: the SMU block size carries all
  dilution assumptions.

**5. §12.4 Corebox and §24.5 Hallmark data-verification index: ADDITION + VALIDATION.**
- **(a) Register NGM's rules as policy data.**
  - CRM: ±3 SD, plus a Westgard-style "two consecutive outside ±2 SD on the same side" bias rule. §12.4 lists
    only the ±2/±3 thresholds.
  - Re-assay windows tiered by grade.
  - Duplicate and umpire acceptance limits (C p.113–118; T p.128).
- **(b) The same blank rule is worded "and" in the Carlin report (C p.115) and "either/or" in the Turquoise Ridge
  report (T p.128).** Rule text must be versioned per report; this is a metric-definition registry case.
- **(c) Achieved insertion is below target** (CRM 4.38% and blanks 4.44% against 5%; duplicates 3.76%). §12.1's
  "insertion rates are TARGETs" is right; Hallmark should show target versus achieved.
- **(d) Per-lab disaggregation matters.** Internal mine labs fail OREAS 264 at 12.8% (GS) and 27% (GQ) against
  4.8% at ALS (C p.113), and the QP prefers an accredited lab (C p.120). This is a natural per-lab e-process demo.
- **(e) "Control absent" periods belong in the index.** Cortez stopped outside check assays for resource
  conversion and infill drilling from 2015 to 2021 (Z p.154). Cortez also used assay bias-adjustment factors
  until 2007 (Z p.158).
- **(f) Disclosure depth differs by report.** Carlin gives counts and failure rates; Turquoise Ridge and Cortez
  give rules only. That gap is exactly what §24.5 fills.
- **(g) Audit findings to index:**
  - RC vs DD bias at Leeville and Goldstrike;
  - RC contamination below the water table, which drives twin drilling (C p.103, p.195);
  - a global 5% porosity assumption;
  - a "lack of Data Quality Objectives (DQO)" (C p.123).

**6. §24.4 classification support pack: VALIDATION.** Carlin's spacing studies use single-block kriging over a
production volume. NGM itself says the result is "highly sensitive to chosen quarterly or annual production
volumes" and "assumes stationarity" (C p.104). This supports labelling the ±15%/quarter metric a heuristic. Add
the production-volume choice as a declared parameter of that metric.

**7. §24.6–24.7 statements and demo: DATA.**
- **Statements.** Tables 3.1 and 3.8 supply a Barrick-attributable versus 100% rendering test. Carlin's table
  gives an attributable column whose rows can be recomputed at 61.5%, and that recomputation catches D1 and D2.
- **Year-over-year bridge.** Carlin's 2023→2024 bridge (Table 14-23, p.194) gives only net change per deposit.
  The drivers are listed qualitatively (p.152): depletion, model updates, domaining, method, optimisation inputs,
  South Arturo +40% acquisition and Emigrant removal. §24.7's exact decomposition therefore cannot be reproduced
  from public text (DATA-GATE).

**8. §18.6 Freeboard closure: ADDITION + DATA-GATE.**
- **(a) At least three different closure numbers exist per site.**
  - Regulator bond (SRCE, regulator unit-cost data file);
  - the operator's own estimate. Carlin labels it the IFRS provision; Turquoise Ridge and Cortez call it the
    "closure model" or "calculation model" cost and do not say whether it is the IFRS figure;
  - the plan's stochastic estimate.
  
  Bond ÷ operator estimate is 2.7× at Carlin ($889M / $328M IFRS), 2.2× at Turquoise Ridge ($217M / $98M) and
  1.5× at Cortez ($279M / $183M) (INFERENCE from E188). The plan's "financial assurance compared against P90"
  should name each measurand with its basis label. It should never reconcile a bond to an operator estimate as if
  they were the same quantity, and never assume that an unlabelled "closure model" cost is the IFRS provision.
- **(b) No consequence classification is disclosed for any TSF**, only GISTM-conformance claims. Consequence
  class is a data gate for the GISTM traceability engine.
- **(c) Tailings runway as a monitored quantity.** Goldstrike's storage lasts "through 2028", and the new 180 Mm³
  TSF is still being permitted (C p.290). This suits a §18 capacity-runway monitor with permitting lead time.

**9. §14 Highwall (outside the C2 focus; recorded because it is strong): RISK + VALIDATION.**
- The GQ7CN highwall failed on 11 Feb 2024 (~1 Mt) (C p.245).
- An independent review board found that the LOM slope designs rested on "a 2008 geology model and 2018 internal
  geotechnical analyses that was not independently reviewed" (C p.244).
- This supports versioned, attested interpretations (L1) and model-currency flags.

**10. §20.8 stockpiles: VALIDATION.**
- **Cortez tracking.** Fleet management records each truck's majority block ID, grade and constituents. Tonnages
  are corrected to survey weekly or monthly. An annual data-quality assessment scores each pile, and sonic
  drilling is done on a 15–18 m grid with 0.6 m samples. Adjustments are booked at end-of-month (Z p.214–215).
- **Carlin.** Stockpiles count as Resources on a net-value test that includes TCM-driven recovery, rehandle
  cost and AB495 (C p.188). Stockpiles carry 2.8 Moz of the 100% Reserve (C p.199).

---

## 5. Proposed evidence rows (E151–E199)

The basis is 100% NGM unless the row says otherwise. C = Carlin 2025, Z = Cortez 2022, T = Turquoise Ridge 2024.
Page numbers are PDF pages.

```
E151 | Carlin M+I 230 Mt @3.43 g/t = 25 Moz; Inferred 78 Mt @3.7 = 9.3 Moz; 100% basis; $1,900/oz; 31-Dec-2024 | C p.191 Table 14-21 | "Barrick's attributable share of the Mineral Resource is based on its 61.5% interest in NGM." | FACT
E152 | Carlin P+P 130 Mt @3.62 g/t = 15 Moz (100%), 9.5 Moz Barrick-attributable; $1,400/oz | C p.20 Table 1-2 | "Mineral Reserves are reported at a gold price of US$1,400/oz." | FACT
E153 | Carlin attributable table internal error: Leeville Indicated 4.0 Moz vs M+I 4.3 Moz for the same 17 Mt @7.82 g/t (correct ≈4.27) | C p.192 Table 14-22 | "Leeville - - - 17 7.82 4.0 17 7.82 4.3" | FACT (values) / INFERENCE (which is wrong: 17×7.82/31.1035)
E154 | Goldstrike Roaster 2024: GC call 5.25 Mt @5.95 g/t 1,006 koz; check-out 5.74 Mt @5.59 1,031 koz; MCF out 109/94/102% | C p.216 Table 15-13 | "MCF out 109% 94% 102%" | FACT
E155 | Gold Quarry Roaster 2024: GC call 2.94 Mt @6.57 621 koz; check-out 2.92 @6.37 598 koz; MCF out 99/97/96% | C p.217 Table 15-14 | "MCF out 99% 97% 96%" | FACT
E156 | Goldstrike Autoclave 2024: GC call 3.71 Mt @1.79 213 koz; check-out 3.67 @1.76 208 koz; MCF out 99/98/97% | C p.218 Table 15-15 | "MCF out 99% 98% 97%" | FACT
E157 | Carlin plant reconciliation commingles Cortez ore and third-party concentrates; sources only back-allocated | C p.216 | "it is not possible to break out individual sources beyond what is back allocated from the processing plants" | FACT
E158 | Carlin classification is drill-spacing based; no KE or slope of regression in any of the three reports (full-text search) | C p.186 | "Resource classification at Carlin is based on the underlying philosophy that data density is the primary driver for confidence" | FACT (method) / INFERENCE (absence, searched)
E159 | NGM spacing studies use single-block kriging; NGM states sensitivity to the chosen production volume and stationarity | C p.104 | "The biggest limitation is that results are highly sensitive to chosen quarterly or annual production volumes." | FACT
E160 | Carlin estimates Au by OK; TCM, SS, preg-rob, As, Hg, CO3, AuCN also estimated; preg-rob by non-linear methods | C p.180 | "For elements where linear estimation methods are not appropriate (e.g. preg-rob), alternate methods of estimation are applied." | FACT
E161 | Cortez 2021 estimators: ID2 Cortez Pits, ID3 Cortez Hills (chosen against GC), OK Goldrush with ID2/ID3/NN checks | Z p.178, p.181, p.192 | "ID3 was selected based on previous model performance compared to grade control." | FACT
E162 | Cortez spacing = three-hole distance / 0.7; Indicated at variogram range to 80% sill, Inferred at 90% | Z p.179, p.182 | "Drill spacings were obtained by dividing the three-hole distance and two-hole distance by 0.7, assuming a squared grid for the drill holes." | FACT
E163 | TRUG classification: Measured ≤7.6 m; Indicated >7.6–61 m; Inferred >61 m (mean distance to 3 nearest holes) | T p.166 Table 14-4 | "Note: average distance to closest three drill holes." | FACT
E164 | RSC 2024 audit of Carlin: RC-vs-DD bias (Leeville, Goldstrike), 5% global porosity, composite counts vs conditional bias | C p.195 | "Consider the use of increase composite numbers to improve the estimation and mitigate the potential for conditional bias." | FACT
E165 | Carlin QA/QC 2022–2024: 14,182 CRMs, 375 failures (2.64%); 14,365 blanks; 16,282 umpires, bias −0.3% | C p.113–118 | "Of the 14,182 CRMs submitted during the review period, 375 (2.64%) were classified as failures." | FACT
E166 | NGM CRM rule includes a two-consecutive ±2 SD same-side bias failure; grade-tiered re-assay windows (±5 / ±10 samples) | C p.113; T p.128 | "if two consecutive samples fall outside of ±2 certified standard deviations (on the same side) of the mean." | FACT
E167 | Internal mine labs show higher CRM failure (OREAS 264: GQ 10/37, GS 12/94) than ALS (45/940); QP prefers an accredited lab | C p.113, p.120 | "the ability to send samples to an independent accredited lab with sufficient turn around time and quality would be a better practice." | FACT (counts) / INFERENCE (rates)
E168 | Cortez stopped outside check assays for growth, conversion and infill drilling 2015–2021 (re-implemented at 5% in 2021) | Z p.154 | "In 2015, growth, resource conversion, and infill drilling at the Cortez Complex discontinued the use of outside check samples" | FACT
E169 | Cortez used assay bias-adjustment factors (exploration vs blasthole vs mill) from a 2003 study; none after 2007 | Z p.158 | "No bias adjustment factors for assay data have been used since 2007." | FACT
E170 | GS Roaster recovery curve fitted on 12-h shifts excluding <90%-availability shifts and adjacent shifts | C p.144 | "In order to avoid upset plant conditions and false CIL retention times, shifts with less than 90% availability were removed" | FACT
E171 | NGM recovery equations use mixed units: Carlin g/t converted ×0.029167 inside; Cortez oz/st; TR g/t, some fraction outputs | C p.144; Z p.172–173; T p.145 | "Note that head grade values in above table are in US Standard units (ounce per short ton)." | FACT
E172 | Roaster feed windows: GS TCM 0.8–1.2%, SS 0.8–1.8%, CO3 >14%; GQ TCM 0.4–0.75%, Hg <25 ppm | C p.143 Tables 13-7, 13-8 | "TCM (%) 0.8 - 1.2 0.95" | FACT
E173 | Goldstrike autoclave preg-rob feed cap is source-dependent | C p.143 | "Preg-rob % maximum depends on the source: <40% for Goldstrike and South Arturo OP sources, <60% for Gold Quarry and Goldstar OP sources" | FACT
E174 | Goldstrike autoclave converted from calcium thiosulphate RIL to CIL in Q1 2023 ($30M) | C p.22 | "A $30M conversion from the calcium thiosulphate RIL process to standard cyanide leaching with CIL was completed in Q1 2023." | FACT
E175 | Same organic carbon yields preg-rob index 0–40% to 75–100% by source; Raman confirms the carbon-structure difference | T p.134, p.144 | "Ores with the same amount of organic carbon can vary in the preg-rob index from insignificant pregrob (0-40%) to severe preg-rob (75-100%)." | FACT
E176 | Sage recovery = a·(TCM/Au) + b per source (TRUG −155.7/100.79; Vista UG −457.0/97.68), stored per block | T p.145 | "Sage autoclave recoveries were calculated from gold grade and TCM percentages and are stored for each refractory block in the Resource block model." | FACT
E177 | TRUG BTAC vs plant, 71 monthly lots 2008–2014: plant − BTAC mean +2.35 pts, SD 4.49; 17 lots > +5, 1 < −5 | T p.137–139 Table 13-2 | "The BTAC results were generally within a few percent of actual plant gold recoveries" | INFERENCE (statistics computed from the table)
E178 | TRUG plant recovery modelled as linear in TCM/Au, R² 0.557 (monthly, 2014–Oct 2023) | T p.139 | "It shows a correlation coefficient (R2) of 0.557" | FACT
E179 | TR Mega Pit testwork: organic carbon >1% treated as waste for sample selection; recovery 34–96% (mean 75.1%) by preg-robbing carbon; Sage still caps type O (TCM >1%) at <120 kt/y | T p.142, p.148 | "Material with organic carbon >1% would be considered waste because of the preg-robbing potential of the organic carbon" | FACT
E180 | Sage blend window (SS 2.6–2.9%, CO3 3.5–4.3%, TCM <0.50%) enters pit optimisation as per-block acid cost or credit | T p.147, p.202 | "applying an acid cost to those blocks with carbonates values >4.3% and an acid credit to those blocks with carbonate values <3.7%." | FACT
E181 | One TR reserve statement uses three intercepts of the surface recovery card (100.79 §13; 97.14 Vista 8; 101.79 Cut 40) | T p.145, p.207–208 | "the Cut 40 Reserves recovery formula includes a nominal 1% increase in sulfide mill recovery." | FACT
E182 | TRUG resource COG 5.66 g/t reproduces as the fixed point g = cost/(net·R(g)): g = 5.665, R = 79.6% | T p.168 Table 14-5 | "TR Autoclave Recovery is calculated at the cutoff grade and assumes LOM plan average of 0.771% TCM" | FACT (inputs) / INFERENCE (reproduction)
E183 | Cortez BCOGs reproduce only with $/t × 0.90718 → $/st; AB495 omitted; 4 of 5 exact, 1 off by 0.001 oz/st | Z p.212, p.214 | "Note: cut-off grades are calculated in US customary units." | INFERENCE (derivation in §3.5)
E184 | Carlin GQ Roaster in-pit BCOG 1.08 g/t reproduces from $41.20/t, 85.4% recovery, 1.07% AB495, $0.38/oz | C p.57, p.147, p.210, p.213 | "External refining and selling costs are $0.38 per recovered gold ounce." | INFERENCE (1.084 g/t computed)
E185 | Carlin open-pit dilution and recovery are taken as inherent in the SMU; UG factors are tabulated by method and mine | C p.208–209 | "No additional mining recovery or dilution assumptions are applied for the optimization and block value calculations." | FACT
E186 | Carlin and TR use pseudoflow for OP shells and UG stope net-value selection; Carlin SO cut-offs are seeds, not breakeven; Cortez 2021 used Whittle LG | C p.213; T p.196; Z p.206 | "It is important to note that these SO-COGs are not traditional breakeven cut off grades (BCOGs)." | FACT
E187 | TRUG dilution carries in-situ TCM at zero gold, so dilution lowers TCM-dependent recovery | T p.201 | "Dilution tonnes (0 g/t Au and insitu TCM% grade material)" | FACT (rule) / INFERENCE (effect)
E188 | Closure: Carlin bonds ~$889M vs IFRS provision ~$328M; TR bonds ~$217M vs closure-model cost ~$98M; Cortez bond ~$279M vs calculation-model cost ~$183M (TR/Cortez not labelled IFRS) | C p.308; T p.268; Z p.280–281 | "NGM currently has posted approximately $889 million in financial assurances in the form of letters of credit and surety bonds" | FACT
E189 | Carlin dewatering: Goldstrike UG 70,030 L/min current, 117,726 L/min LOM max; Gold Quarry 42,396 → 63,689 L/min | C p.248 Table 16-9 | "Goldstrike Underground 969 737 70,030 117,726" | FACT
E190 | Goldstrike tailings capacity lasts to 2028; a new 180 Mm³ TSF (permitting from 2023) is needed from 2029 | C p.290 | "NBTDF and TSF3 provide tailings storage for the Goldstrike Autoclave and Roaster through 2028." | FACT
E191 | No TSF consequence classification is disclosed in any of the three reports, only GISTM conformance claims; Juniper is upstream-centerline | C p.289–290; T p.262 (Juniper TSF); Z p.274–275 | "It is constructed using an upstream centerline method" | INFERENCE (absence, searched) / FACT (Juniper method)
E192 | Carlin Table 6-2 "historic gold production" columns do not sum to totals (2020 rows 3.91 vs total 1.8 Moz) | C p.65 | "Pete Bajo 0.59 0.88 0.088 0.09 0.11 0.085" | FACT (values) / INFERENCE (sums)
E193 | TR attributable table prints Mega Cut 55 Indicated as 1 Mt (12 Mt implied by 20 Mt × 61.5%); 100% table prints Vista UG Measured as 0.0028 Mt (0.028 Mt implied by 0.0053 Moz @ 5.79 g/t) | T p.193–194 Table 14-15, 14-16 | "Mega Cut 55 - - - 1 2.79 1.1 12 2.79 1.1" | FACT (value) / INFERENCE (error)
E194 | Cortez stockpiles: per-truck block ID, grade and constituents; survey correction; sonic drilling on 15–18 m grid | Z p.214–215 | "Geochemical/metallurgical constituents (cyanide soluble gold, preg rob index, sulfide sulfur, organic carbon, and carbonate) are tracked alongside ounces." | FACT
E195 | Carlin GQ7CN highwall failed 11-Feb-2024 (~1 Mt); review board found designs on a stale, unreviewed basis | C p.244–245 | "a 2008 geology model and 2018 internal geotechnical analyses that was not independently reviewed" | FACT
E196 | Carlin reserves LOM: 132.5 Mt processed @3.62 g/t, 15.43 Moz contained, 83.5% recovery, 12.89 Moz recovered, 2025–2044 | C p.260 Table 16-16 | "Recovery % 83.5 84.6" | FACT
E197 | Carlin LOM capex $2,429.3M, opex $12,558.3M (real Q4-2024, reserves only, 100%) | C p.309–310 | "All capital and operating costs are based on Mineral Reserves only and presented in real USD as of Q4 2024." | FACT
E198 | TR P+P 69 Mt @6.29 = 14 Moz (8.6 attributable) at $1,300; M+I 110 Mt @5.42 = 20 Moz at $1,700 | T p.193, p.198 | "Mineral Reserves are reported at a gold price of US$1,300/oz." | FACT
E199 | Cortez (31-Dec-2021) P+P 110 Mt @4.17 = 14 Moz at $1,200; M+I 210 Mt @2.75 = 18 Moz at $1,500 (100%) | Z p.203–204, p.216–217 | "Barrick's and Newmont's attributable shares of the Mineral Reserves are 61.5% and 38.5%, respectively." | FACT
```

---

## 6. Checked nulls

- **Slope of regression, kriging efficiency, kriging variance, conditional simulation results.** A full-text
  search of all three mirrors found no reported values. Methods are only named: SBK (C p.104); "single block
  kriging, various Quantitative Kriging Neighbourhood Analyses, or conditional simulation studies" (C p.186); an
  SBK study for Goldrush (Z p.193). GB-11 cannot be populated from these filings.
- **F1, F2 and F3 by name, and resource-model-vs-GC reconciliation numbers.** Absent at all three complexes.
  Only the Carlin plant-level "MCF out" tables (2024 annual) exist. Monthly values are chart-only.
- **TSF consequence or hazard classes, dam-break studies and FoS.** Not disclosed. Factors of safety appear only
  for pit slopes, where design acceptance criteria are consequence-tiered ("e.g., factor of safety = 1.2" for
  low-consequence sectors, C p.243), and for UG ground support (≥1.5, C p.239).
- **QA/QC counts for Turquoise Ridge and Cortez.** Rules only; no submitted or failed counts.
- **Recovery uncertainty.** No intervals or covariance are given on any recovery equation. The only fit statistic
  is TRUG's R² = 0.557.
- **Closure cost probability distributions.** Point values only (bond, provision). Nothing is usable as a P90.
- **Per-source plant reconciliation.** Explicitly impossible from plant data at Carlin (commingling).
- **AISC / TCC per site.** Not in these reports. That data belongs to the Barrick quarterly mine statistics (other
  agents' documents).
- **Thiosulphate leaching anywhere in NGM's current flowsheets.** None after the Q1 2023 Goldstrike conversion
  (C p.22).
