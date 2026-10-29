# C3: African gold technical reports (Kibali, Loulo-Gounkoto)

Agent C3 · evidence block E201–E249 · written 2026-10-28.

RESULT_STATUS: MATERIAL_INCREMENT

**How the numbers were checked.** Every value marked FACT below was read from the PDF's own text layer
(`pdftotext -layout`, and `-raw` where layout mode misaligned table columns). The markdown mirrors were used only to
find pages. Page references are **PDF page numbers**:

- `K pN` = `Barrick_Kibali_NI43-101_TechnicalReport_February2026.pdf`. Printed page = PDF page − 14.
- `L pN` = `Loulo_Gounkoto_Technical_Report_2023.pdf`. Printed page = PDF page − 17.

Arithmetic marked INFERENCE was recomputed with `python3 -I` from the printed cells.

**Ownership basis (applies to everything below).**

- Kibali Goldmines SA is owned 45% by Barrick, 45% by AngloGold Ashanti and 10% by SOKIMO, whose shares are held
  by the DRC Ministry of Portfolio (K p44). Barrick is the operator.
- SOMILO (Loulo permit) and Gounkoto SA (Gounkoto permit) are each owned 80% by Barrick and 20% by the State of
  Mali (L p59). Barrick is the operator.
- Both reports state figures on a **100% basis** unless a column says "Attributable". Barrick's quarterly
  statistics label these sites "Kibali (45%)" and "Loulo-Gounkoto (80%)"
  (`Barrick_Q1_2026_Mine_Stats.xlsx`, sheet QTD).

---

## 1. Inventory

| Document | Page ranges (PDF) | Content | Extraction usability |
|---|---|---|---|
| Kibali NI 43-101. Issuer "Barrick Mining Corporation"; report dated 27 Feb 2026; effective 31 Dec 2025; 367 pp | p15–37 summary; p43–51 property, permits, fiscal terms; p52–53 history and production 2013–2025; p82–108 drilling, sampling, QA/QC (CPA vs fire assay); p112–135 metallurgy; p136–192 resources (estimation p139–160, stockpiles p165–166, COG p167–168, F1 p183–187); p193–230 reserves (SMU p197, dilution p199, IRA p200, UG dilution matrices p213–215, COG p217–218, reconciliation p220–224); p231–281 mining (OP geotech p235–238, blasting p245–246, UG p254–276, LOM p277–280); p282–291 processing; p292–316 infrastructure (power p294, TSFs p305–316); p319–335 environment, social, closure; p336–340 costs; p344–348 conclusions and risks; p349–350 recommendations | Text layer clean. Tables reproduce well in `-raw` mode. Charts (monthly MCF, recovery predicted vs actual) carry values that are not in the text and were not used. |
| Loulo-Gounkoto Technical Report (NI 43-101 format). Issuer "Barrick Gold Corporation"; report dated 17 Mar 2023; effective 31 Dec 2022; 429 pp | p18–45 summary (Table 1-3 LOM p29, risks p44); p55–63 property and fiscal; p95–101 drilling and GC classes; p102–157 sample prep and QA/QC (56 pp); p160–173 metallurgy; p174–260 resources (estimation p218–237); p261–309 reserves (COG p265–268, recon p274–301); p310–347 mining (geotech p272–273/p287, paste p333–334, ventilation p342); p348–362 processing (metal accounting p358–361); p363–374 infrastructure (TSF p366–370); p377–401 environment; p402–405 costs; p409–415 conclusions and risk register | Text layer clean except where noted. Table 15-8 is misaligned in `-layout` but correct in `-raw`. **The p360 plant-call-factor formula is glyph-encoded in every extraction and cannot be verified.** Table 11-1 contains a printed error (blanks "38%"). |

The report effective date (31 Dec 2022) **predates** both the 2023 Mali Mining Code and the 2024–2025 dispute. The
only corpus source for those events used here is the Barrick Q1 2026 xlsx footnote (§4, item 5).

---

## 2. Demonstrator data

### 2.1 Resource and reserve statements

| Item | Kibali (K p20/p22; 31 Dec 2025) | Loulo-Gounkoto (L p24/p27; 31 Dec 2022) |
|---|---|---|
| Price: resources / reserves | $2,000 / $1,500 per oz | $1,700 / $1,300 per oz |
| Measured | 46 Mt @ 3.04 g/t = 4.5 Moz (attributable 2.0) | 37 Mt @ 3.61 = 4.3 Moz (3.4) |
| Indicated | 150 Mt @ 2.71 = 13 Moz (6.1) | 55 Mt @ 4.02 = 7.1 Moz (5.7) |
| M+I | 200 Mt @ 2.79 = 18 Moz (8.1) | 92 Mt @ 3.85 = 11.4 Moz (9.1) |
| Inferred | 49 Mt @ 2.1 = 3.3 Moz (1.5) | 28 Mt @ 2.6 = 2.3 Moz (1.9) |
| Proven | 30 Mt @ 3.13 = 3.0 Moz (1.4) | 25 Mt @ 3.54 = 2.8 Moz |
| Probable | 82 Mt @ 2.92 = 7.7 Moz (3.5) | 42 Mt @ 4.08 = 5.5 Moz |
| P+P | 110 Mt @ 2.97 = 11 Moz (4.8) | 67 Mt @ 3.87 = 8.3 Moz (6.7) |
| UG P+P | 50 Mt @ 3.86 = 6.2 Moz | Proven 11 Mt @ 4.86 = 1.7; Probable 24 Mt @ 5.04 = 3.9 |
| Resource cut-off | OP 0.59–0.82 (average 0.71); UG incremental 0.91 g/t | OP average 0.79; UG average 1.43 g/t |
| Reserve cut-off | OP 0.75–0.99; UG 2.06 g/t | UG Yalea 2.60, Gara 2.39, Gounkoto 2.78 g/t |

Both reports state resources **inclusive of** reserves (K p20; L p24), report contained metal, and round to two
significant figures. Kibali's M+I contained metal rose 11.6% and Inferred rose 60.1% against 2024, partly because
the resource price rose from $1,900 to $2,000 (K p190).

### 2.2 Kibali mill production, Table 6-2 (K p53; 100% basis)

| Year | Tonnes milled (kt) | Grade (g/t) | "Contained Gold" (oz)¹ | Recovery (%) |
|---|---|---|---|---|
| 2018 | 8,218 | 3.45 | 807,251 | 88.6 |
| 2019 | 7,513 | 3.80 | 814,027 | 88.7 |
| 2020 | 7,632 | 3.68 | 808,134 | 89.4 |
| 2021 | 7,783 | 3.62 | 812,152 | 89.8 |
| 2022 | 7,769 | 3.30 | 749,590 | 88.5 |
| 2023 | 8,222 | 3.19 | 762,851 | 89.9 |
| 2024 | 8,504 | 2.82 | 686,417 | 89.2 |
| 2025 | 8,322 | 2.79 | 673,520 | 90.3 |

¹ The column is headed "Contained Gold", but the values are **recovered** ounces. Printed ÷ (t × g ÷ 31.1035 ×
recovery) = 0.998–1.006 for 2018–2021 and 2023–2025, and 1.028 for 2022 (INFERENCE). The table also lists
2013–2017.

### 2.3 Kibali reconciliation series (K p222; 100% basis)

**Table 15-27: GC call vs Mills Check Out ("MCF (out)").** The last three columns are as printed; the bracketed
values are recomputed from the row's own cells.

| Year | GC call t | g/t | oz | Check-out t | g/t | oz | MCF t / g / oz |
|---|---|---|---|---|---|---|---|
| 2016 | 7,091,057 | 3.17 | 721,583 | 7,296,162 | 3.08 | 722,497 | 103 / 97 / 100 |
| 2017 | 7,829,349 | 2.92 | 735,021 | 7,618,932 | 2.92 | 715,267 | 97 / 100 / 97 |
| 2018 | 8,295,114 | 3.37 | 898,759 | 8,218,074 | 3.48 | 919,476 | 99 / 103 / 102 |
| 2019 | 7,532,915 | 3.78 | 915,473 | 7,513,496 | 3.78 | 912,513 | 100 / 100 / 100 |
| 2020 | 7,677,149 | 3.66 | 902,194 | 7,631,867 | 3.68 | 903,786 | 99 / 101 / 100 |
| 2021 | 7,848,475 | 3.68 | 929,076 | 7,783,337 | 3.61 | 902,613 | 99 / 98 / 97 |
| 2022 | 7,854,035 | 3.43 | 866,978 | 7,814,641 | 3.35 | 842,421 | 100 / 98 / 97 |
| 2023 | 8,284,019 | 3.25 | 864,684 | 8,222,323 | 3.21 | 849,746 | 99 / 99 / 99 [oz 98.3] |
| 2024 | 8,541,084 | 2.94 | 808,423 | 9,145,188 | 2.78 | 817,769 | 100 / 96 / 95 **[107.1 / 94.6 / 101.2]** |
| 2025 | 8,343,402 | 2.77 | 742,630 | 8,322,270 | 2.79 | 746,649 | 100 / 101 / 101 |
| Total | 79,296,598 | 3.29 | 8,384,820 | 79,566,290 | 3.26 | 8,332,737 | 100 / 99 / 99 |

The 2024 check-out tonnes (9,145,188 t) also disagree with Table 6-2 (8,504 kt milled in 2024). The 2022 values
disagree as well (7,814,641 t @ 3.35 here vs 7,769 kt @ 3.30 in Table 6-2).

**Table 15-25: open-pit GC call vs raw block GC model at marginal cut-off** (variance = GC call ÷ model − 1):

| Year | 2016 | 2017 | 2018 | 2019 | 2020 | 2021 | 2022 | 2023 | 2024 | 2025 | Total |
|---|---|---|---|---|---|---|---|---|---|---|---|
| Tonnes | −11% | −5% | 8% | 4% | 0% | −6% | 3% | 18% | 31% | 27% | 7% |
| Grade | 9% | 2% | −9% | −6% | −8% | −7% | −16% | −21% | −27% | −27% | −12% |
| Ounces | −4% | −3% | −2% | −3% | −8% | −13% | −13% | −7% | −4% | −8% | −6% |

The totals are 47,054,265 t @ 2.04 = 3,092,735 oz (GC call) vs 44,027,009 t @ 2.32 = 3,290,593 oz (model). Called
metal is lower than the model's metal in every one of the ten years.

**Table 15-24 (2025):** OP GC call 6,354,121 t @ 1.51 = 309,191 oz vs dig-block polygon depletion
5,632,144 t @ 1.73 = 313,873 oz, a variance of +13% t / −13% g / −1% oz. The reserve assumes 10% dilution and 3%
ore loss for pits not yet mined (K p199).

**Table 15-26: 2025 MCF detail (K p222).** Tonnes, grade (g/t) and ounces:

| Line | Tonnes | g/t | oz |
|---|---|---|---|
| GC mined ore UG+OP (EoY declared survey) | 9,299,052 | 2.67 | 798,760 |
| GC theoretical feed (mined − stockpile change) | 8,333,645 | 2.81 | 752,662 |
| GC actual feed (UG conveyor and OP stock change) | 8,344,111 | 2.77 | 742,180 |
| **GC adjustment (difference)** | −10,466 | **−31.15** | 10,482 |
| GC call (actual feed adjusted for cone and scats) | 8,343,402 | 2.77 | 742,630 |
| Mills check-in (flowmeter, reconstituted CIL and gravity sampling) | 8,322,270 | 2.72 | 727,674 |
| Mills check-out (flowmeter, bullion plus inventory change) | 8,322,270 | 2.79 | 746,649 |
| MCF (out) | 100% | 101% | 101% |

INFERENCE: MCF (in) for ounces = 727,674 ÷ 742,630 = 98.0%. Check-out exceeds check-in by 2.6%; the report calls
this "unaccounted gains" (K p224). The monthly 2025 ranges are tonnes −3% to +2%, grade −8% to +12% and ounces −8%
to +14%, against a stated tolerance of ±5% (K p221).

**Kibali F1** (short-range ÷ long-range model on areas converted from Indicated to Measured, year on year; K p183–187),
tonnes / grade / ounces in %:

| Domain | 2022 | 2023 | 2024 | 2025 |
|---|---|---|---|---|
| KCD 5000 | 83 / 115 / 98 | 100 / 99 / 96 | 98 / 117 / 115 | 91 / 105 / 95 |
| Gorumbwa | 116 / 110 / 127 | 131 / 92 / 121 | 106 / 121 / 129 | 99 / 98 / 98 |
| Pamao | 94 / 113 / 106 | 95 / 101 / 96 | 90 / 110 / 100 | 100 / 110 / 110 |

The Kombokolo and Rhino 2024 blocks print identical F1 values (122 / 75 / 92), which is probably a copy error
(K p186–187). For KCD 5000, F1 t × F1 g does not equal F1 oz in 2022 (0.83 × 1.15 = 0.95 vs 98) or 2023
(1.00 × 0.99 vs 96) (INFERENCE).

### 2.4 Loulo-Gounkoto reconciliation (100% basis; Loulo and Gounkoto feed the same plant)

**Table 15-8 tonnage MCF (tails tonnes ÷ GC call tonnes) and Table 15-9 grade MCF (check-out ÷ GC call grade),
L p274 (verified in `-raw`):**

| Year | GC call kt | Tails kt | MCF t % | GC grade | Check-out grade | MCF g % |
|---|---|---|---|---|---|---|
| 2015 | 4,728 | 4,543 | 96 | 5.11 | 4.78 | 93 |
| 2016 | 5,094 | 4,875 | 96 | 4.99 | 4.91 | 98 |
| 2017 | 5,177 | 4,918 | 95 | 5.19 | 4.96 | 96 |
| 2018 | 5,298 | 5,154 | 97 | 4.50 | 4.40 | 98 |
| 2019 | 5,066 | 4,931 | 97 | 4.82 | 4.92 | 102 |
| 2020 | 5,012 | 4,895 | 98 | 4.39 | 4.74 | 108 |
| 2021 | 5,067 | 5,019 | 99 | 4.63 | 4.80 | 104 |
| 2022 | 5,090 | 5,087 | 100 | 4.43 | 4.61 | 104 |

The series runs 2005–2022 in the source. Over 2005–2022: Σ tails ÷ Σ GC call = 98.3%, and the tonne-weighted metal
ratio is 96.9% (INFERENCE). The grade-MCF row is mislabelled "kt". The printed grade MCF differs from the
recomputed value by 1 point in 2009, 2014 and 2015, which rounding explains. The "tails tonnes" equal Table 17-2
"Total Ore to Plant" for 2015–2022 (L p352). Recovered ounces 2015–2022 were 630,167; 707,116; 730,372; 660,234;
714,802; 680,215; 700,133; 684,225 (L p352).

**2022 year-end reconciliation, Table 15-19 (L p297)** in tonnes / g/t / oz:

- mine 6,587,618 / 3.29 / 695,847;
- stockpile change 1,485,487 / **−0.54** / −25,935;
- theoretical feed 5,102,131 / 4.40 / 721,782;
- actual feed 5,093,709 / 4.43 / 726,227;
- GC adjustment 30,084 / **−0.97** / −940;
- scats −18,404 / 2.98 / −1,762;
- cone 21,662 / 5.03 / 3,505;
- GC call 5,090,451 / 4.43 / 724,484;
- plant check-out 5,087,313 / 4.61 / **753,220**;
- MCF 100 / 104 / 104.

The +4% grade is attributed to higher-grade, more variable Gounkoto FGO stockpile feed (L p296, p299).

**Resource Call Factor, Table 15-20 (L p301):**

- mine (resource model) 5,648,115 / 4.27 / 775,196;
- resource call 5,077,706 / 4.45 / 726,169;
- plant check-out 5,087,313 / 4.61 / **683,184**;
- RCF printed 100 / 104 / 104.

INFERENCE: 5,087,313 t × 4.61 g/t ÷ 31.1035 = 754,0xx oz, which matches Table 15-19's 753,220 and not 683,184. On
683,184 oz the ounce RCF would be 94%, not the printed 104%.

### 2.5 Cut-off inputs, costs, closure and fiscal terms

| Item | Value | Basis / period | Page |
|---|---|---|---|
| Kibali UG resource BCOG / ICOG | 1.56 / 0.91 g/t at $2,000 (total costs $84.45 / $49.38 per t) | 100%, 2025 | K p168 |
| Kibali UG reserve BCOG / ICOG | 2.06 / 1.21 g/t at $1,500; selling cost $0.38/oz | 100%, 2025 | K p217 |
| Kibali OP COG table | Dilution 10%, ore loss 3%, G&A $10.25/t, "Gold Royalty (5.7%) $85.5/oz" at a $2,000 resource price | 2025 | K p167 |
| Kibali MSO (resource) | 25 × 35 m stope, 40% waste, width 5–50 m, 10 m pillar | 2025 | K p168 |
| Loulo OP COG (Table 15-2) | Royalty 6%, selling 0%, net price $1,222/oz at $1,300; dilution 10%, loss 3%; full COG 1.36–3.58, marginal 0.75–0.98 g/t by pit | 2022 | L p265–266 |
| Loulo OP material classes | Waste < 0.5 g/t; mineralised waste 0.5 g/t to marginal COG (excluded from reserve); marginal ore stockpiled (in reserve) | 2022 | L p265 |
| Loulo Yalea UG COG | BCOG 2.59 g/t (2.56 in 2021); incremental 1.27 (development) and 1.47 (stoping) at $1,300 and 86.61% recovery | 2022 | L p267 |
| Kibali LOM unit opex | OP $4.69/t mined, $51.30/t ore; UG $63.93/t mined, $72.83/t ore; processing $19.51/t; G&A $10.58/t; total $91.11/t milled | Real Q4 2025 $; 100% | K p336–339 |
| Kibali LOM capex | $3,017M (sustaining 1,089; capitalised stripping 960; UG development 190; growth 777) | 100% | K p336–337 |
| Loulo LOM unit opex | OP $2.84/t mined, $38.42/t ore; UG $50.73/t mined, $52.86/t ore; processing $19.63/t; G&A $7.81/t; total $75.99/t milled | Real Q4 2022 $; 100% | L p37, p405 |
| Loulo processing cost | 2022 actual $24.68/t vs planned $17.49/t; the LOM uses $19.63/t | 100% | L p405 |
| Loulo LOM capex | $1,636M (capitalised stripping 404; UG development 371; TSF 143; plant expansion 150; GC drilling 50; other) | 100% | L p36, p402 |
| Kibali closure | $41.5M undiscounted; $25.8M discounted under IFRS (IAS 37); financial surety ("sureté financière") lodged with a regulator-designated bank | 100%, 31 Dec 2025 | K p30, p335 |
| Kibali royalty and charges | 5.7% of gold revenue, including 2% shipment fees | DRC Code 2002 as amended 2018 | K p46 |
| Kibali super-profits tax | 50% of the increase in gross operating surplus above $2,000/oz (trigger = BFS $1,600 + 25%); deductible; **not applied** at reserve ($1,500) or resource ($2,000) prices | | K p46, p347 |
| Kibali CIT | 30% ("initial accelerated depreciation allowances have been depleted") | | K p46 |
| Kibali permit tenure | 10 permits expire in 2029 and 2030; renewable for 15 years if obligations are met; LOM runs to 2043 | | K p44–45 |
| Loulo fiscal terms | Establishment Conventions under the **1991** Mining Code: 6% royalty, 30% CIT, minimum 0.75% of gross revenue if loss-making, fuel-duty exoneration | As of Mar 2023 | L p59 |

### 2.6 Underground mining, geotechnics and tailings

| Item | Value | Page |
|---|---|---|
| Kibali shaft | 740 m deep, 8 m diameter, double-drum winder, 14.8 t skips, average 34 skips/h; "3.2 Mtpa at a 90% utilisation"; upgrade to 3.4 Mtpa planned for 2027 | K p265 |
| Kibali UG ore schedule (Table 16-13) | 2026 3.2 Mt @ 5.01; 2028 3.4; 2029 3.5; 2039 3.6 Mt; LOM 50.2 Mt @ 3.86 g/t | K p279 |
| Kibali plant | Design 7.2 Mtpa; peak 8.5 Mtpa; LOM target 8.2 Mtpa (UG 3.4 + OP 4.8); FTSF capacity constrains oxide feed until Pamao Main in-pit deposition in 2028 | K p277 |
| Kibali stopes | 20–40 m strike, 15–40 m wide, 25–35 m high (Modified Stability Graph); 35 m level interval; footwall drive offset 25–30 m (up to 60 m) | K p254–258 |
| Kibali UG unplanned dilution matrix (Table 15-20) | Primary transverse HW 1.8%; FW 1.1% + 1 exposure × 2.0% = 3.1%; secondary transverse HW 1.8% + 2 × 2.5% = 6.8%; FW 1.7% + 3 × 2.5% = 9.2% | K p215 |
| Kibali paste | Legacy 70/30 slag-cement fill has "degraded over time", raising paste dilution in adjacent secondary stopes; pillars are left against weak fill; Terraflow tested against the traditional 90/10 slag blend | K p214 |
| Loulo paste design | 0.3–0.5 MPa (not exposed below, overhand); 0.9–2.4 MPa (exposed) | L p333 |
| Loulo paste UCS by binder % and curing days (Table 16-11; unit not printed, MPa from p333 context) | 3.5% binder: 14 d 1.67, 28 d 2.15, 56 d 2.65, **112 d 1.10, 180 d 1.25**; 6.0%: 3.05 / 4.26 / 4.80 / 5.06 / 5.81 | L p334 |
| Kibali ventilation | About 1,420 m³/s; "a dilution rate of 0.05 m3/s" (unit basis not stated) | K p266 |
| Loulo ventilation by equipment (Table 16-12) | TH663i 565 kW → 28.25 m³/s; LH621i 352 kW → 17.6; AC3 150 → 7.5; CAT 962H 172 → 8.6. These equal 0.05 m³/s per kW, but that is INFERENCE; the factor is not printed | L p342 |
| Kibali development rate | Needs to rise from 700 m to 1,000 m per month | K p280, p350 |
| Kibali OP slope design | "Consequence-confidence" design acceptance criteria; lower DAC "e.g., factor of safety = 1.2"; **no PoF criterion stated** | K p236 |
| Kibali OP slope angles (Table 15-3, headed "Inter Ramp Angle") | Weathered/oxide 27–50°; fresh 30–59° by pit. The text calls the same angles "overall slope": 30–40° oxide, 35–45° transition, 45–60° fresh | K p200 |
| Kibali CTSF1 and CTSF2 | GISTM "High"; full at 31.78 Mt (July 2025); freeboard 3.28 m | K p305, p307 |
| Kibali CTSF3 | GISTM "Very High"; PGA 0.271g (1-in-10,000 years); FoS > 1.5 static and > 1.1 pseudo-static; EDF 1-in-100-year 24 h 153 mm; IDF 72 h PMP 487 mm; flood storage ≈ 1.90 million m³; freeboard minimum 1.5 m; 1.5 mm HDPE over compacted clay | K p308–309 |
| Kibali FTSF | Unlined, upstream, self-raised, with a downstream waste-rock buttress; 59.2 m current height, 61.5 m design maximum; **no GISTM class stated** | K p311 |
| Loulo TSF | Minimum freeboard 2.7 m for a 1-in-50-year 72 h rainfall (raised from 1.5 m under the GISTM-based strategy). Epoch (Oct 2022) found "an area of low Factor of Safety" needing a sacrificial buttress to deposit above the 43 m design height because the Southern Extension was delayed to Q2 2025. **No GISTM class stated** | L p366, p369 |

### 2.7 QA/QC

| Item | Kibali | Loulo-Gounkoto |
|---|---|---|
| CRM failure rule | Any single result outside 3SD; 98% within 3SD (K p102) | Outside 3SD, **or** three consecutive results beyond 2SD on the same side (L p109) |
| Insertion | ~10% QC overall. Oct 2024–Sep 2025 (Table 11-2): standards 5,971 (1.93%); blanks 6,085 (1.97%); field duplicates 6,694 (2.16%); coarse duplicates 6,330 (2.04%); pulp duplicates 3,022 (0.98%); umpire 1,217 (0.43%); CPA vs FA 4,719 (1.67%); total samples 316,820 (K p99) | CRMs 1 in 19 (5.26%), rotated quarterly; CRM bag IDs photographed to detect swaps (L p108–109) |
| Failed batch | Not described beyond the CRM rule | Whole batch re-assayed; if it fails again, the samples are "flagged but committed into the database" (L p109) |
| Method change | FA → Chrysos PhotonAssay (CPA). 62,545 tandem samples (May 2022–May 2024) show CPA 2.7% higher above 0.1 g/t; CPA only since May 2024 (K p98–99) | 50 g FA with AAS finish; gravimetric above 15 g/t (L p106) |
| Retention | CPA coarse-crush originals discarded after QC (jars reused up to 25 times); FA pulps kept about one year (K p100) | n/a |
| Printed inconsistencies | Umpire count 1,998 in the text (K p106) vs 1,217 in Table 11-2 (K p99), both "0.43%" | Table 11-1 Loulo blanks "5,556 38%" (≈6.5% by count); OREAS 205 min 1.24 > max 1.21 (L p110) |

---

## 3. Plan impacts (ranked by value)

**1. §20.4 F-factor definitions registry: ADDITION and CORRECTION.**
- Evidence: K p183, p221–222; L p274, p301.
- Problem: the plan's F1 is "GC model ÷ depleted resource over the mined volume" and its MCF is "South African". At
  Barrick's African mines, the same names mean different numerators and denominators.
- Proposed registry rows:

| Registry key | Numerator | Denominator | Support | Source |
|---|---|---|---|---|
| `F1.barrick_kibali.conversion` | Short-range (GC) model | Long-range (Ind/Prob) model | Areas converted Ind→Meas, year on year (not the mined volume) | K p183 |
| `MCF_out.barrick_africa` | Mills check-out (flowmeter t; bullion + inventory change) | GC call (actual feed ± cone/scats) | Plant year/month | K p221–222 |
| `MCF_in.barrick_africa` | Mills check-in (flowmeter t; grade reconstituted from CIL and gravity sampling) | GC call | Plant year | K p222 |
| `MCF_t.barrick_loulo` | Tails tonnes | GC call tonnes | Plant year | L p274 |
| `MCF_g.barrick_loulo` | Check-out grade | GC call grade | Plant year | L p274 |
| `RCF.barrick_loulo` | Plant check-out | Resource call (resource model over 3D mined volume) | Plant year | L p301 |

**2. §20.2 / §20.9 and the M0 discrepancy register: VALIDATION and DATA-GATE.**
- Evidence: K p53, p222; L p297, p301.
- Published reconciliation tables put grades on residual rows:
  - Kibali "GC Adjustment" −10,466 t @ **−31.15 g/t** for +10,482 oz;
  - Loulo stockpile change @ −0.54 g/t and GC adjustment @ −0.97 g/t.
- Some cells cannot be reproduced:
  - Kibali 2024 MCF row: printed 100/96/95 vs recomputed 107.1/94.6/101.2;
  - Kibali Table 6-2 vs Table 15-27, same year: 2022 and 2024 tonnes differ;
  - Loulo RCF plant ounces: 683,184 vs 753,220.
- These validate the rule "no plugs; no grade on a difference row" and give the M0 discrepancy register real
  fixtures.
- Proposed change: "Importers store printed reconciliation cells verbatim, recompute every ratio and t·g→oz
  identity, and file mismatches beyond rounding as DiscrepancyRecords citing both pages. A residual row is imported
  as (Δt, Δoz) with grade = NOT_DEFINED."

**3. §14.2 GB-39 and acceptance-policy data: CORRECTION.**
- Evidence: K p236, p308; L p369.
- The only slope and dam criteria in these reports are deterministic FoS:
  - Kibali open pits use a consequence-confidence DAC with an example lower DAC of FoS = 1.2;
  - Kibali CTSF3 uses FoS > 1.5 static and > 1.1 pseudo-static.
- Neither report states any PoF criterion.
- Proposed change: replace the GB-39 example with "Acceptance (policy data): e.g. Kibali OP DAC = f(consequence,
  confidence) with lower DAC FoS 1.2 (FACT, K p236). PoF ≤ 5% remains FACT[U]; no PoF criterion was found in the
  Kibali or Loulo reports." Policy rows need a field `dac_basis ∈ {FoS, PoF, FoS+PoF}`, with `pof_threshold`
  nullable.

**4. §20.5 bias priors and the §20.4 compensation detector: VALIDATION of E029.**
- Evidence: K p221–224.
- In every year 2016–2025, Kibali OP called ounces fell below the raw GC model (−2% to −13%).
- From 2022 the pattern is more dilution and lower grade: tonnes +3% to +31%, grade −16% to −27%.
- In the same year (2025), the mine-to-mill MCF (out) is 100/101/101. That figure is helped by +2.6% check-out over
  check-in "unaccounted gains".
- This is a public case of the plan's "compensating errors" flag, with a 10-year series to fit a signed bias prior.
- Proposed change: add it as known-answer fixture KA-TOUCH-KIBALI-2016-2025 (100% basis, Barrick 45%).

**5. §35.2 RISK register, §10 forecast ledger and GATE-BULL-01: RISK and DATA-GATE.**
- Evidence: L p59, p415; `Barrick_Q1_2026_Mine_Stats.xlsx` QTD!A34.
- Loulo-Gounkoto timeline:
  - operations suspended 14 Jan 2025;
  - "loss of control" 16 Jun 2025;
  - agreement with Mali 24 Nov 2025;
  - provisional administration ended 16 Dec 2025.
- Barrick published no operating data for Q1–Q3 2025.
- The 2023 report rated Country & Political risk "Possible / Major / Moderate" and Fiscal Stability "Possible /
  Moderate / Moderate". Its QP was "not aware of any risks that could result in the loss of ownership".
- Proposed rows:
  - `RISK-26 | Host-state action at a demonstration site (suspension, provisional administration, retroactive code) | High | Site-quarter states NOT_PUBLISHED; CountryRegime as-of dating; no silent fill | Owner + counsel`.
  - GATE-BULL-01 excludes NOT_PUBLISHED site-quarters from its 95% denominator and lists them by name.
- Registering technical-report risk ratings as forecasts needs resolution criteria written in advance. "Loss of
  operational control" is not "loss of ownership", and only the first happened.

**6. §23.4 CountryRegime / H-FISC and §21 Bullion: ADDITION.**
- Evidence: K p44–46, p167, p347; L p59.
- Rows ready to paste:
  - `DRC/Kibali | royalty+charges 5.7% of revenue incl. 2% shipment fees | super-profits tax 50% × ΔGOS above $2,000/oz, deductible | CIT 30% | state 10% via SOKIMO | permits expire 2029/2030, 15-yr renewal`.
  - `Mali/Loulo-Gounkoto (as of 2023-03) | 1991-Code Establishment Conventions | royalty 6% | CIT 30% | min 0.75% gross revenue | state 20%`.
- The super-profits tax is excluded from reserve economics because $1,500 is below the trigger. Helm must model the
  tax as price-contingent, because at any price above $2,000 after-tax value departs from the technical report.
- The Kibali resource COG table charges "$85.5/oz" royalty at a $2,000 price. That equals 5.7% of $1,500, not of
  $2,000 ($114), so the importer should flag it.
- The plan's "Mali's 2023 Code" step-up is absent from this report. Each regime row must carry the instrument
  (convention vs code) and an as-of date.

**7. §18 Freeboard, GB-21 and closure: ADDITION and VALIDATION.**
- Evidence: K p30, p277, p305–311, p335; L p366–369.
- GISTM classes are per facility: Kibali CTSF1/2 "High", CTSF3 "Very High". Kibali FTSF and the Loulo TSF have no
  stated class, so store NULL and never infer one.
- Flood criteria differ by facility:
  - CTSF3: EDF 1:100 24 h 153 mm; IDF PMP 72 h 487 mm; 1.5 m freeboard;
  - Loulo: 2.7 m for 1:50 72 h.
- GB-21's S = 1.8 Mm³ is close to CTSF3's printed flood storage of ≈ 1.90 Mm³, which validates the magnitude.
- Two tailings-to-plan couplings for Pushback/Crucible:
  - TSF capacity is a binding plan constraint at Kibali (FTSF limits oxide feed until 2028);
  - at Loulo, a delayed TSF expansion forced over-height deposition behind a sacrificial buttress.
- Closure: carry both the undiscounted ($41.5M) and the IAS 37 discounted ($25.8M) figures. They answer different
  questions.

**8. §12.4 QA/QC policy and §12 acceptance states: CORRECTION and ADDITION.**
- Evidence: K p98–106; L p108–110.
- CRM failure rules differ by site: Kibali uses a single 3SD rule; Loulo also fails three consecutive results beyond
  2SD on the same side. Rules must be policy data that supports multiple rules.
- Add a batch state `COMMITTED_FLAGGED` (Loulo commits re-failed batches). Strata must decide explicitly whether
  flagged assays enter estimation.
- Model the assay method (FA vs CPA, +2.7% mean bias) as a covariate across the 2022–2024 changeover.
- Make sample retention a data-gate. Kibali CPA originals are discarded, so those assays cannot be re-assayed.
- Require explicit denominators on insertion rates (Kibali Table 11-2 mixes 309,587 and 282,782 bases; INFERENCE).

**9. §16.4 hoisting: CORRECTION.**
- Evidence: K p255, p265, p279.
- 34 skips/h × 14.8 t × 8,760 h × 0.90 = 3.97 Mt, against a stated 3.2 Mtpa. The stated pair implies about 6,360
  effective hours, or a further ≈ 0.81 factor (INFERENCE).
- Proposed change: the hoist formula carries `availability`, `utilisation` and `fill_factor` as separate registered
  parameters, each with the site's own definition.
- UG ore of 3.4–3.6 Mt/yr from 2028 exceeds today's 3.2 Mtpa hoist. The schedule depends on the 2027 upgrade
  (3.4 Mtpa) and on decline trucking that "supplements" the shaft. Register this as a schedule dependency.

**10. §16.3 backfill and §16 dilution: ADDITION.**
- Evidence: K p214–215; L p333–334.
- Paste strength is not monotone in time. At Loulo, 3.5% binder drops from 2.65 at 56 d to 1.10 at 112 d. At
  Kibali, legacy 70/30 slag-cement fill has "degraded over time".
- Dilution is planned per exposure: 2.0–2.5% for each paste wall exposed.
- Proposed change: add a typed `DilutionMatrix(stope_type × sequence × wall × n_exposures)` and a `FillStrength(t)`
  curve with a degradation flag. Cure-time precedence alone is not enough.

**11. §13 Strata and §24.4 Hallmark: VALIDATION and a checked null for GB-11.**
- Evidence: K p157–158; L p222, p237.
- Kibali classifies "across zones large enough to represent a quarter or a year of production". This validates the
  support volume in §24.4.
- Both sites display SR (and KE at Loulo) visually and publish **no numeric threshold**, so GB-11's b ≈ 0.1 cannot
  be calibrated from these reports.
- Loulo resets negative-grade blocks "to the block grade of the closest sample". Strata must refuse or flag this,
  never apply it silently.
- Loulo estimates recovery per block by ID, which validates §13.4: recovery is NonAdditive and must be
  metal-weighted.
- Kibali re-reports old models (Pakaka 2019; Megi-Marakeke-Sayi 2020) under a new shell and COG, so model age
  should be a metadata field.

**12. §17 Crucible: ADDITION.**
- Evidence: K p222, p277; L p352, p361.
- Plant capacity in the LOM is empirical: design 7.2, target 8.2, peak 8.5 Mtpa. Store nameplate and demonstrated
  capacity separately.
- Expose MCF (in) and MCF (out) separately; their gap (2.6% in 2025) is the plant's unaccounted gain.
- Loulo's tonnage MCF uses the same flowmeter tonnes as plant feed, so it cannot detect a flowmeter bias.
- The Loulo team applies P754 "without necessarily going through formal training" (L p361). That is an input to
  metal-accounting assurance scoring.

**13. §16.6 ventilation and §16 regulation registry: ADDITION.**
- Evidence: K p256, p266; L p342.
- The 0.05 m³/s per kW diesel factor appears at both sites (derived at Loulo; stated without units at Kibali).
- Kibali, in the DRC, adopts Western Australian Regulation 10.28. The regulation registry needs `adopted_standard`,
  separate from jurisdiction.

**14. §10 forecast ledger and §21: NEW-IDEA.**
- Evidence: L p29, p30, p36, p329, p405.
- Technical-report assumptions are scoreable frozen forecasts:
  - Loulo LOM processing $19.63/t, against 2022 actual $24.68/t and plan $17.49/t;
  - Loulo LOM recovered gold 2023–2025 of 680 / 680 / 666 koz (100%).
- The Loulo report is also internally inconsistent: the text expands the plant to 6.2 Mtpa from 2029/2030 (capex
  $150M), but Table 1-3 processes 4.5 Mt/yr in 2029–2036.

**15. §32 Portal basis labels: DATA-GATE.**
- Evidence: K p53, p99.
- Header semantics cannot be trusted: Kibali Table 6-2 "Contained Gold" holds recovered ounces, and QA/QC
  percentages use mixed denominators.
- Every imported column needs an asserted basis (contained/recovered; 100%/attributable) that is checked against an
  identity test, not the header text.

---

## 4. Proposed evidence rows (E201–E249)

Page numbers are PDF pages (K = Kibali, L = Loulo; xlsx = `Barrick_Q1_2026_Mine_Stats.xlsx`).

| E### | Claim | Source + page | Quote (≤ 25 words) | Class |
|---|---|---|---|---|
| E201 | Kibali is a 45/45/10 JV (Barrick / AngloGold Ashanti / SOKIMO); Barrick operates; TR figures are 100% basis | K p44 | "Kibali Goldmines is owned 45% by Barrick and 45% by AngloGold. The remaining 10% interest in Kibali Goldmines is held by SOKIMO" | FACT |
| E202 | Loulo and Gounkoto permit holders are each 80% Barrick, 20% State of Mali | L p59 | "The Loulo Permit is owned by SOMILO, which is held 80% by Barrick and 20% by the state of Mali." | FACT |
| E203 | Kibali "F1" compares short-range and long-range models over Ind→Meas conversion areas, not the mined volume | K p183 | "F1 factors compare long-range models (Indicated Mineral Resources/Probable Mineral Reserves used for LOM planning) against short-range models (GC drilled areas used for annual budget planning)" | FACT |
| E204 | Kibali's primary reconciliation is MCF (out) = Mills Check Out ÷ GC Call; tolerance ±5% | K p221 | "the primary reconciliation comparison made is the Mills Check Out divided by the GC Call, referred to as the Mine Call Factor" | FACT |
| E205 | Loulo tonnage MCF = tails tonnes ÷ GC call tonnes; grade MCF = check-out ÷ GC call grade | L p274 | "tracking of tonnes to check dilution between GC call tonnes and tails tonnes from the plant and the mine call factor (MCF)" | FACT |
| E206 | Kibali 2024 MCF row does not reproduce: cells give 107.1/94.6/101.2% vs printed 100/96/95%; check-out t also ≠ Table 6-2 8,504 kt | K p222, p53 | "2024 8,541,084 2.94 808,423 9,145,188 2.78 817,769 100% 96% 95%" | INFERENCE (ratios recomputed from printed cells) |
| E207 | Kibali's published MCF table assigns a grade (−31.15 g/t) to a residual "GC Adjustment" row | K p222 | "GC Adjustment (difference between Theoretical and Actual Feed) -10,466 -31.15 10,482" | FACT |
| E208 | Loulo's published reconciliation assigns a negative grade to the stockpile-change row | L p297 | "Stockpile Change 1,485,487 -0.54 -25,935" | FACT |
| E209 | Loulo RCF table plant ounces (683,184) contradict the same t and g elsewhere (753,220) and the printed 104% | L p297, p301 | "Plant Check Out 5,087,313 4.61 683,184" | INFERENCE (5,087,313 × 4.61 ÷ 31.1035 ≈ 754,0xx oz) |
| E210 | Kibali 2025 check-out exceeds check-in by 2.6% (MCF (in) oz 98.0%), attributed to unaccounted gains | K p222, p224 | "driven by unaccounted gains, averaged 101% MCF (out) for the year." | INFERENCE (746,649 ÷ 727,674; 727,674 ÷ 742,630) |
| E211 | Kibali OP GC call held fewer ounces than the raw GC model every year 2016–2025 (−2% to −13%; total −6%) | K p221 | "Total 47,054,265 2.04 3,092,735 44,027,009 2.32 3,290,593 7% -12% -6%" | FACT |
| E212 | 2025 Kibali OP GC call vs dig-block depletion: +13% t, −13% g, −1% oz | K p220 | "2025 6,354,121 1.51 309,191 5,632,144 1.73 313,873 13% -13% -1%" | FACT |
| E213 | Loulo 2022 +4% grade MCF attributed to Gounkoto FGO stockpile feed | L p296 | "The positive grade reconciliation is a function of the higher-grade portion feed from Gounkoto FGO stockpile" | FACT |
| E214 | Kibali OP slopes use consequence-confidence DAC; example lower DAC is FoS 1.2; no PoF criterion stated | K p236 | "utilises a consequence-confidence approach to determine the design acceptance criteria (DAC) … permit a lower DAC (e.g., factor of safety = 1.2)" | FACT |
| E215 | Kibali CTSF3 stability criteria: FoS > 1.5 static and > 1.1 pseudo-static; PGA 0.271g (1:10,000) | K p308 | "Factors of Safety > 1.5 (static) and > 1.1 (pseudo-static) for all critical sections." | FACT |
| E216 | GISTM classes at Kibali: CTSF1/2 "High", CTSF3 "Very High" | K p307–308 | "Under the GISTM consequence classification framework, CTSF3 is classified as “Very High”" | FACT |
| E217 | CTSF3 flood design: EDF 1:100 24 h 153 mm; IDF 72 h PMP 487 mm; flood storage ≈ 1.90 Mm³ | K p308–309 | "corresponds to approximately 1.90 million m³ of floodwater storage." | FACT |
| E218 | Kibali FTSF is unlined, upstream and self-raised (59.2 m, max 61.5 m); no GISTM class stated | K p311 | "The FTSF is an unlined, upstream, self-raised, paddock, open ring dyke facility with a downstream waste rock buttress" | FACT |
| E219 | Loulo TSF has a low-FoS area; a sacrificial buttress is needed to exceed the 43 m design height because the expansion is delayed | L p369 | "an area of low Factor of Safety, which require a sacrificial buttress to enable tailings deposition above the current designed final height of 43 m" | FACT |
| E220 | Loulo TSF freeboard 2.7 m for 1:50-yr 72 h rainfall, raised from 1.5 m under the GISTM-based strategy | L p366, p369 | "minimum freeboard of 2.7 m to provide sufficient storage to contain a 1 in 50-year rainfall event over a 72-hour period" | FACT |
| E221 | Kibali CRM failure = any single result outside 3SD; 98% within | K p102 | "classified as a failure if any individual result fell outside three standard deviations (3SD) from the certified mean" | FACT |
| E222 | Loulo CRM failure is multi-rule (3SD, or three consecutive beyond 2SD on the same side) | L p109 | "outside of three standard deviations (3SD) from the certified mean, or three consecutive samples fall outside of two standard deviations (on the same side)" | FACT |
| E223 | Loulo commits re-failed batches to the database with a flag | L p109 | "when a batch is re-assayed and fails again, the samples are flagged but committed into the database" | FACT |
| E224 | Kibali CPA originals are discarded after QC, so those assays cannot be re-assayed from original material | K p100 | "CPA coarse crush original samples are routinely discarded following QC checks, as the jars are reused up to 25 times." | FACT |
| E225 | Kibali CPA reads 2.7% higher than fire assay above 0.1 g/t (62,545 tandem samples, 2022–2024) | K p98 | "Above a threshold of 0.1 g/t Au, results showed a bias of 2.7%, with CPA having a higher mean than fire assay" | FACT |
| E226 | The Kibali umpire sample count conflicts within the report (1,998 in text vs 1,217 in Table 11-2) | K p99, p106 | "A total of 1,998 umpire samples (0.43% of samples) were submitted across the grade range" | FACT (both values printed) |
| E227 | Kibali classification uses SR displayed visually; no numeric SR threshold is stated | K p157 | "carried out by displaying the estimated blocks (SR), together with the supporting data as a guide." | FACT |
| E228 | Kibali classifies at quarterly or annual production support | K p158 | "classification done across zones large enough to represent a quarter or a year of production in a realistic mining context." | FACT |
| E229 | Loulo resets negative kriged grades to the nearest sample's grade | L p237 | "Blocks estimated with negative grades are reset to the block grade of the closest sample." | FACT |
| E230 | Loulo estimates metallurgical recovery per block by inverse distance | L p222 | "gold, arsenic, and copper grades are estimated using OK. Only metallurgical recovery is estimated using ID." | FACT |
| E231 | Kibali shaft: 34 skips/h of 14.8 t; 3.2 Mtpa at 90% utilisation | K p265 | "The shaft hoists an average of 34 skips per hour and has a capacity of 3.2 Mtpa at a 90% utilisation." | FACT |
| E232 | Kibali stated hoist figures imply a further ≈ 0.81 factor beyond 90% utilisation; LOM UG ore (3.4–3.6 Mt) exceeds the current 3.2 Mtpa | K p265, p279 | "Mt 50.2 3.2 3.2 3.4 3.5 3.4 3.3 3.3 3.4 3.3 3.4 3.4 3.4 3.0 3.6" | INFERENCE (34 × 14.8 × 8,760 × 0.9 = 3.97 Mt; 3.2 ÷ 3.97 = 0.81) |
| E233 | Kibali legacy slag-cement paste has degraded, raising paste dilution in secondary stopes | K p214 | "adjacent to legacy stopes which were backfilled with a 70/30 slag-cement blend, the strength of which has degraded over time" | FACT |
| E234 | Loulo paste UCS is non-monotone with curing time (3.5% binder: 2.65 at 56 d, 1.10 at 112 d) | L p334 | "3.50% 1.67 2.15 2.65 1.10 1.25" | FACT (MPa unit from p333 context) |
| E235 | Kibali plans UG dilution per paste exposure (secondary transverse FW: 1.7% + 3 × 2.5% = 9.2%) | K p215 | "Footwall 1.7 3 2.5 7.5 9.2" | FACT |
| E236 | Loulo paste design strength depends on exposure (0.3–0.5 MPa unexposed; 0.9–2.4 MPa exposed) | L p333 | "0.3 MPa to 0.5 MPa compressive strength for stopes that are not exposed below" | FACT |
| E237 | Kibali (DRC) adopts Western Australian UG regulations | K p256 | "The mine has adopted the Western Australian Mines Safety and Inspection Regulations (1995) and specifically looks to comply with Regulation 10.28." | FACT |
| E238 | Loulo diesel ventilation = 0.05 m³/s per kW engine rating; Kibali cites "0.05 m3/s" without a unit basis | L p342; K p266 | "Truck SANDVIK TH663i 565 28.25" | INFERENCE (28.25 ÷ 565 = 17.6 ÷ 352 = 0.05) |
| E239 | Kibali plant runs above nameplate; LOM target 8.2 Mtpa | K p277 | "While it has a design capacity of 7.2 Mtpa, it has consistently exceeded that capacity up to a peak of 8.5 Mtpa." | FACT |
| E240 | Kibali Table 6-2 "Contained Gold" values are recovered ounces | K p53 | "2025 8,322 2.79 673,520 90.3" | INFERENCE (printed ÷ (t·g/31.1035·rec) = 0.998–1.006, except 2022 at 1.028) |
| E241 | Kibali same-year mill figures differ between Table 6-2 and Table 15-27 (2022, 2024) | K p53, p222 | "2022 7,854,035 3.43 866,978 7,814,641 3.35 842,421" | FACT (both tables printed; Table 6-2 2022 = 7,769 kt @ 3.30) |
| E242 | Kibali super-profits tax: 50% of ΔGOS above $2,000/oz; not applied at reserve or resource prices | K p46, p347 | "The rate is 50% and is applied to the increase in gross operating surplus driven by the gold price exceeding $2,000/oz." | FACT |
| E243 | Kibali royalty and charges total 5.7% of gold revenue | K p46 | "A total royalty and other charges payable to the DRC government of 5.7% of gold revenue inclusive of 2% shipment fees" | FACT |
| E244 | Kibali resource COG royalty ($85.5/oz) equals 5.7% of $1,500, not of the stated $2,000 price | K p167 | "Gold Royalty (5.7%) $/oz Au 85.5 85.5 85.5" | INFERENCE (0.057 × 2,000 = 114.0; 0.057 × 1,500 = 85.5) |
| E245 | Kibali permits expire 2029/2030 while the LOM runs to 2043; renewal is conditional, for 15 years | K p44 | "The next renewal dates for the Exploitation Permits are May 11, 2029 and June 3, 2030" | FACT |
| E246 | Loulo fiscal terms are 1991-Code Establishment Conventions (6% royalty, 30% CIT, 0.75% minimum) | L p59 | "based on the 1991 Mining Code. A 6% royalty is payable to the Malian government based upon production" | FACT |
| E247 | The 2023 Loulo QP saw no ownership-loss risk; country/political risk was rated Possible/Major/Moderate | L p59, p415 | "not aware of any risks that could result in the loss of ownership of the deposits or loss of the permits" | FACT |
| E248 | Loulo-Gounkoto: suspension 14 Jan 2025, loss of control 16 Jun 2025; no Q1–Q3 2025 data; control returned 16 Dec 2025 | xlsx QTD!A34 | "temporary suspension of operations at Loulo-Gounkoto starting January 14, 2025, and subsequent loss of control on June 16, 2025" | FACT |
| E249 | Loulo 2022 processing cost overran plan by 41%; the LOM assumes $19.63/t | L p405, p37 | "The actual processing costs for 2022 were $24.68/t, compared to a planned cost of $17.49/t." | FACT (41% = 24.68 ÷ 17.49 − 1, INFERENCE) |

---

## 5. Checked nulls

- **Numeric SR or KE thresholds for classification.** Both reports display SR (and KE at Loulo) visually and
  classify by drill spacing (Kibali KCD UG M 25×10, I 40×40, Inf 80×80; Loulo M ≈ 30 m, I 30×80). There is nothing
  to calibrate GB-11's b ≈ 0.1 against.
- **PoF acceptance criteria.** Neither report states one for slopes or dams. Only FoS criteria appear.
- **Conditional simulation.** Neither site reports simulation for classification or grade control. Both use OK only.
- **Blasthole sampling.** Both use RC/GC drilling for grade control (Kibali GC spacing 10×5 to 5×5 m). Kibali
  tracks blast movement with RFID, flitch mapping and OrePro3D/Oretrack (K p221, p245–246).
- **Mali 2023 Code and the 2024–2025 events.** The Loulo report (effective 31 Dec 2022) is silent on both; the only
  corpus evidence used is the xlsx footnote (E248).
- **GISTM class for the Kibali FTSF and the Loulo TSF.** Not stated.
- **Per-site AISC.** Neither report gives AISC. That is left to C1's quarterly stats.
- **Loulo plant call factor formula (L p360).** Glyph-garbled in every extraction; not reported.
- **Chart-only values** (monthly MCF, predicted vs actual recovery, throughput plan vs actual). Not extracted,
  because the series pairing is ambiguous.
- **Hoist automation and cycle-time detail.** Beyond skip count and payload, none is given.
