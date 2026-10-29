# C4: Pueblo Viejo, Porgera, Veladero and Zaldívar technical reports

RESULT_STATUS: MATERIAL_INCREMENT

- **Agent:** C4.
- **Evidence block:** E251–E299.
- **Scope:** read-only. The only file written is this one.

## Conventions

**Page numbers.** "p.N" is the PDF page index, as used by `pdftotext -layout -f N -l N`. Printed page labels differ:
- Pueblo Viejo (PV) prints "Page N−1"; for example, PDF p.139 is printed "Page 138".
- The RPA reports print section-page labels ("Page 13-4").

**Verification.** Every number marked FACT was read from the per-page `pdftotext -layout` text of the PDF itself, not
from the markdown mirror. Porgera's equation page p.82 was also checked visually.

**Ownership basis.**
- Every figure is the **100% (project) basis**, unless marked otherwise.
- The ownership stated is the ownership at each report's effective date.
- Today's labels come from `Barrick_Q1_2026_Mine_Stats.xlsx`, read with openpyxl.

---

## 1. Inventory

| Document | Effective date, author, QPs | Ownership basis at effective date | Content by PDF page | Extraction usability |
|---|---|---|---|---|
| `Barrick_PuebloViejo_NI43-101_TechnicalReport_February2026.pdf` (301 pp) | 31 Dec 2025; report dated 27 Feb 2026; Barrick-employee QPs (P. Jones resources, P. Lee reserves) | 100%. Barrick 60% / Newmont 40% through PVD (p.208, p.213) | 16–24 summary, production history, risk table; 106–108 legacy and current QA/QC; 124–126 database (acQuire, Seequent); 127–139 metallurgy (ore types, stockpile variability, recovery models, plant history); 142–143 data verification; 195 classification; 203–204 stockpile model; 206–210 cut-off, F1A/F1B, resource statement, resource waterfall; 212–225 reserves, optimisation inputs, shells, MCF, mill vs reserve, reserve waterfall; 236–238 LOM schedule and fleet; 240–246 plant, power, water, reagents; 253–258 tailings (El Llagal, Naranjo); 261–269 environment, permits, social; 276 closure and assurance; 277–279 capex and opex, Item 22 omitted | Good text layer. Pages 33, 199–202, 250, 293 and 295 are figure-only. One source typo: SAG media consumption printed "0.7–0.12" (p.246). |
| `veladero-03232018.pdf` (183 pp) | 31 Dec 2017; RPA, dated 19 Mar 2018; independent QPs | 100%. MAGSRL is 50% Barrick / 50% Shandong Gold since 30 Jun 2017 | 9–28 summary, including Trigger Limits and incidents; 75–79 metallurgy and production (Tables 13-1 to 13-3) and the Transition Period; 110 GC vs RM; 112–117 classification, reserves, pit limit; 124 cut-off and routing; 136–145 VLF, mining, process; 152 doré sales; 154–160 permits, Trigger Limits, incidents, closure; 161–166 capex, opex, AISC, manpower, Item 22 omitted | Good. Tables are clean in both the PDF text and the markdown mirror. |
| `zaldivar-03162012.pdf` (114 pp) | 31 Dec 2011; RPA, dated 16 Mar 2012 | 100%. CMZ is 100% Barrick at that date (p.7) | 7–16 summary; 30–31 history and ore movements 2007–2011; 50 encapsulation; 70–72 reconciliation and classification; 74–82 reserves, cut-off, MEL layback, schedule; 84–92 process and recovery model; 96 market; 98–102 environment, capex, opex, Item 22 omitted | Good. There are two internal inconsistencies (see §2.5). |
| `porgera-03162012.pdf` (210 pp) | 31 Dec 2011; RPA, dated 16 Mar 2012 | 100%. Unincorporated JV: Barrick (Niugini) 95% / Mineral Resources Enga 5% (p.33, p.138) | 7–17 summary; 25, 33–35 tenure and royalty; 70–80 sampling, QA/QC, data verification; 81–87 metallurgy; 138–147 reserves, Whittle, underground design, 2007–2011 reconciliation; 172–175 process; 179–185 environment, social, closure; 186–191 capex, opex, manpower; 192 Item 22 omitted | Text is good. The recovery equations (p.82–84) are typeset math: glyph soup ("���") in the markdown mirror and in pdftotext. The equation *form* on p.82 was verified visually, but the constant tables were **not** re-verified and are not reported as FACT. The actual-vs-budget ounces table (p.86) appears to have swapped column labels. |

---

## 2. Demonstrator data

### 2.0 Ownership basis register (record with every figure)

| Site | Basis of the report figures | Ownership at the report's effective date | Barrick label in the Q1 2026 mine stats (xlsx cell) |
|---|---|---|---|
| Pueblo Viejo | 100% (p.139, p.208, p.213) | Barrick 60% / Newmont 40% (PVD) | "Pueblo Viejo (60%)", QTD!A10 |
| Veladero | 100% | MAGSRL 50/50 Barrick/Shandong from 30 Jun 2017. Earlier years in the same tables were under different ownership. | "Veladero (50%)", QTD!A13 |
| Porgera | 100%: "These represent 100% of the Mineral Reserves, and not the 95% attributable to Barrick" (p.138) | Barrick (Niugini) 95% / MRE 5% | "Porgera (24.5%)", QTD!A14. The change from 95% to 24.5% happened outside this corpus slice. |
| Zaldívar | 100% | CMZ 100% Barrick (2011) | Not checked: it is a copper asset. Any later ownership change is outside this corpus slice. |

**INFERENCE (for C1 to confirm).** In the Q1 2026 Minestats1 sheet (column L), PV "Tonnes processed" of 1,574 kt
appears to be on a 60% basis:
- 1,574 ÷ 0.6 = 2,623 kt per quarter, or about 10.5 Mtpa.
- That is close to the technical report's 2025 throughput of 10,868 kt, which is on a 100% basis.

Technical-report rows and mine-stats rows must never be joined without the ownership factor.

### 2.1 Pueblo Viejo (100% basis; effective 31 Dec 2025)

**Reserves.** Price $1,500/oz Au and $21/oz Ag. Point of reference: "ROM grades and tonnage delivered to the primary
crushing facilities", excluding process inventory (p.212). Source: p.213.

| Row | Mt | Au g/t | Ag g/t | Au Moz | Ag Moz |
|---|---|---|---|---|---|
| Open pit Proven | 89 | 2.22 | — | 6.4 | — |
| Open pit Probable | 130 | 1.95 | 11.58 | 8.1 | 48 |
| Stockpiles Probable (no Proven) | 92 | 2.04 | 13.59 | 6.0 | 40 |
| Total P&P | 310 | 2.06 | 12.30 | 20 | 120 |

**Resources.** Price $2,000 Au and $25 Ag. Resources are **inclusive** of reserves and rounded to two significant
digits (p.208).

| Class | Mt | Au g/t | Ag g/t | Au Moz | Ag Moz |
|---|---|---|---|---|---|
| Measured | 110 | 2.07 | 11.15 | 7.2 | 39 |
| Indicated | 300 | 1.82 | 11.16 | 18 | 110 |
| Indicated includes stockpiles | 94 | 2.02 | 13.47 | 6.1 | 41 |
| M&I | 410 | 1.89 | 11.16 | 25 | 150 |
| Inferred | 16 | 1.5 | 8.3 | 0.77 | 4.2 |

**Plant history, Table 13-7 (p.139).** The table states "All production figures presented are 100% basis."

| Year | 2012 | 2013 | 2014 | 2015 | 2016 | 2017 | 2018 | 2019 | 2020 | 2021 | 2022 | 2023 | 2024 | 2025 |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| Throughput kt | 740 | 4,429 | 6,712 | 6,917 | 7,545 | 7,984 | 8,347 | 8,606 | 8,829 | 9,111 | 9,448 | 8,886 | 9,551 | 10,868 |
| Au g/t | 5.05 | 6.14 | 5.53 | 4.94 | 5.29 | 4.57 | 4.04 | 3.91 | 3.61 | 3.18 | 2.68 | 2.39 | 2.46 | 2.38 |
| Au recovery % | 92.9 | 93.0 | 92.9 | 86.8 | 91.0 | 92.3 | 89.4 | 89.8 | 88.5 | 87.6 | 87.4 | 81.3 | 78.6 | 75.1 |
| Ag recovery % | 48.1 | 34.5 | 56.3 | 33.0 | 63.4 | 74.6 | 73.9 | 59.3 | 47.7 | 47.9 | 49.6 | 33.0 | 24.3 | 30.3 |
| Au koz | 112 | 813 | 1,109 | 954 | 1,167 | 1,083 | 968 | 983 | 903 | 814 | 713 | 559 | 586 | 630 |
| Ag koz | 454 | 2,084 | 3,854 | 2,496 | 3,385 | 4,457 | 5,006 | 3,202 | 2,746 | 2,391 | 2,179 | 1,347 | 1,374 | 1,526 |

**Recovery cards (p.127, p.136–137).**
- Five metallurgical ore types: MO-BSD, MO-VCL, MN-BSD, MN-VCL and MN-SP.
- **Pit POX card:** R_POX = 0.9·Au + 87 for Au ≤ 3.3 g/t; otherwise 90%.
- **Pit flotation card:** R_FLT = a·[S²⁻] + b·[S²⁻]/Au + c/Au + d.

  | Ore type | a | b | c | d |
  |---|---|---|---|---|
  | BSD | 0.586 | 0.202 | −7.50 | 78.25 |
  | VCL | 0.184 | 0.202 | −7.50 | 93.18 |
  | SP | 0.152 | 0.139 | −5.18 | 94.38 |

- Flotation-tail CIL recovery is 34%.
- **Overall recovery:**
  - R_OVL = (1−γ)·R_POX + γ·R_FLT·R_POX + γ·(1−R_FLT)·R_FTail, with γ ≈ 66%.
  - Stage "operational factors" can be applied in business plans.
- **Stockpile cards** use a matrix of ore type × Stot class, replacing the earlier single curve (p.133). The classes
  are L1 < 7.0% Stot, L2 7.0–8.5% and L3 > 8.5%.

  | Ore group | POX L1 | POX L2 | POX L3 | Flotation L1 | Flotation L2 | Flotation L3 |
  |---|---|---|---|---|---|---|
  | BSD | 87 | 87 | 74 | 77 | 77 | 74 |
  | VCL / SP | 88 | 82 | 82 | 74 | 79 | 76 |

**Reconciliation, gold (p.207, p.222–223).** 2025 F1A and F1B cover January to October.

| Factor | 2023 (t / g / metal) | 2024 | 2025 |
|---|---|---|---|
| F1A, GCM vs RM, block by block | 97 / 104 / 101% | 98 / 102 / 100% | 99 / 106 / 105% |
| F1B, GCM polygons vs RM | 102 / 100 / 103% | 101 / 98 / 99% | 95 / 100 / 96% |
| Mill vs reserve model | 94 / 94 / 89% | 104 / 95 / 99% | 96 / 101 / 96% |

**2025 MCF stock-and-flow, Table 15-10 (p.222).**

| Line | kt | Au g/t | Au koz |
|---|---|---|---|
| Opening stocks | 95,811 | 2.07 | 6,382 |
| Stocks to crusher | 8,030.5 | 2.16 | 557.1 |
| Closing stocks | 92,340 | 2.03 | 6,035 |
| Pits to crusher | 3,346.8 | 2.79 | — |
| Crusher feed | 11,377.3 | 2.34 | 857.7 |
| GC mine call | 11,221.9 | 2.36 | 849.8 |
| Mill check-in | 10,715.3 | 2.42 | 835.1 |
| Mill check-out | 10,715.3 | 2.44 | 839.6 |
| **MCF** | **95%** | **103%** | **99%** |

- The MCF also gives Ag grade 101% and Ag oz 97%. The cone change was 70.3 kt.
- INFERENCE: stockpiles supplied 70.6% of 2025 crusher feed.
- INFERENCE: stockpile additions of about 4,560 kt at about 1.43 g/t (about 210 koz) are implied by the balance but
  not reported.

**Costs, cut-off and price (p.206, p.215–219, p.278).**
- AuEq cut-off is 1.17–1.29 g/t.
- Mining cost: ore $3.20/t; waste $3.68/t.
- Fixed costs are $290.0M per year, or $21.48/t at 13.5 Mtpa.
- Royalty is 3.2%.
- Total ore cost is $46.10/t at 9.04% Stot.
- Bullion transport and refining is $0.50/oz.
- The reserve optimisation used the "conservative" flotation+POX route for all ore. LOM recovery is 80.1% Au and
  52.1% Ag (p.218).
- LOM opex per tonne of ore (p.278):

  | Item | $/t ore |
  |---|---|
  | Mining | 8.37 |
  | Processing | 35.34 |
  | G&A | 3.12 |
  | Direct | 46.83 |
  | Royalty | 3.04 |
  | **Total, "without by-product credits"** | **50.12** |

**Plant inputs (p.243–246).**
- POX: 225°C, 3,100 kPaG, 60 min.
- Hot cure > 90°C for 12 h; lime boil > 85°C for 4.5 h.
- Power: 210 MW demand, 124.2 kWh/t.
- Reagents per tonne: limestone 307–400 kg/t; lime 51–63 kg/t; cyanide 0.89–1.2 kg/t.
- Oxygen: 7,000 tpd.
- LOM quarry tonnage: quality limestone 321,561 kt, against pit ore of 217,649 kt (p.237).

**Tailings, closure and capital (p.20, p.253–258, p.264, p.276–278).**
- **El Llagal** ends in 2030.
- **Naranjo design:**
  - 500 Mm³ total: 344.7 Mt of tailings plus 452.7 Mt of PAG.
  - Downstream-raised, geomembrane-lined rockfill dam.
  - **"Extreme"** consequence class, "in accordance GISTM".
  - Seismic design: 1/10,000-year event, PGA 0.92 g. The seepage recovery dams are "High".
- **Naranjo cost, permits and land:**
  - Capex $986.4M, within expansion capex of $1,503.8M and LOM capex of $4,177.8M.
  - The starter-dam permit application was due in Q1 2026, with the permit expected "within 12 months".
  - About 680 households are affected.
- **Closure:**
  - $569M LOM. Of this, $273M is in the PER ($126M discounted) and $296M is not yet in it.
  - Assurance is $250M: $70M escrow plus a $180M insurance bond.

### 2.2 Veladero (100% basis; effective 31 Dec 2017; MAGSRL 50/50 since 30 Jun 2017)

**Reserves (p.11, p.116).** $1,200 Au, $16.50 Ag, ARS 20. Cut-offs are 0.18 and 0.32 g/t.

| Row | Mt | Au g/t | Ag g/t | Au Moz | Note |
|---|---|---|---|---|---|
| Proven open pit | 8.0 | 0.82 | 15.8 | 0.21 | |
| Proven stockpiles | 7.0 | 0.52 | — | 0.12 | |
| Proven leach inventory | 13.5 | 0.78 | — | 0.34 | Labelled "contained". Numerically it equals the recoverable-basis book (see E276). |
| Probable open pit | 199.4 | 0.78 | — | 4.97 | 94.7 Moz Ag |
| P&P | 227.8 | 0.77 | — | 5.63 | 100.8 Moz Ag. The silver grade excludes leach-inventory tonnes. |

**Resources** (exclusive of reserves; $1,500, p.10):
- M&I: 140.1 Mt @ 0.57 g/t, 2.55 Moz.
- Inferred: 67 Mt @ 0.43 g/t, 0.93 Moz.

**Heap-leach production, Table 13-3 (p.78; 100%).**

| | 2017 | 2016 | 2015 | 2014 | 2013 |
|---|---|---|---|---|---|
| Total ore placed (dmt) | 28,848,621 | 28,028,112 | 28,384,940 | 29,499,744 | 29,086,337 |
| Placed contained Au (oz) | 942,169 | 723,654 | 751,918 | 945,655 | 876,133 |
| Placed recoverable Au (oz) | 725,003 | 561,328 | 588,241 | 739,923 | 671,344 |
| Total production Au (oz) | 641,103 | 544,155 | 601,444 | 722,418 | 640,697 |
| Leach-pad inventory Au (oz) | 336,310 | 252,410 | 235,236 | 248,439 | 230,935 |
| Reported "Recovery" Au | 77.0% | 77.6% | 73.3% | 72.8% | 72.3% |
| *INFERENCE: placed recoverable ÷ placed contained* | *76.95%* | *77.57%* | *78.23%* | *78.24%* | *76.63%* |
| *INFERENCE: Δinventory − (placed recoverable − production)* | *0* | *+1* | *0* | *−1* | — |
| Mercury produced (kg) | 54,469 | 134,186 | 59,481 | 52,292 | 55,125 |
| Mercury sold (kg) | 56,973 | 0 | 0 | 0 | 0 |

- 2017 production was 641,103 oz against a budget of 820,542 oz (−21.87%, p.77).
- Cumulative since 2005: 8.2 Moz Au from 319 Mt at 1.09 g/t (p.9). INFERENCE: about 73% recovery against contained.

**Recovery card (Table 13-1, p.76).** Recovery is a piecewise function of grade by ore type and route. Examples:
- Type 1 CRUSH: 2.4·Au + 77 for 0.5–1.6 g/t.
- Type 2 CRUSH: 14·Au + 28 for 1–3 g/t; capped at 70.
- ROM Type 1: 60%.
- Silver: 10.6% for CRUSH ores; 0 for ROM.
- Test basis: 96 h bottle rolls; 30-day columns; a 60-day heap cycle (p.75).

**VLF Trigger Limits (permit amendment 2014; p.26, p.145, p.154).**

| Limit | Value |
|---|---|
| LCRS level | ≤ 3,914.7 MASL |
| PSSA level | ≤ 3,927 MASL |
| LCRS pumping | ≤ 270 m³/day |

- An exceedance triggers the contingency plan and restricts fresh-water make-up and cyanide addition.
- Snowmelt is a named driver.
- Operating parameters: 85,000 tpd crushed; irrigation 20 L/m²/h; maximum VLF capacity 700 Mt.

**Incidents (p.14, p.27–28, p.155–157).**

| Date | Event | Consequence |
|---|---|---|
| 2013 | Excess solution in the collection system | $1.2M fine (paid March 2014). The April 2014 permit amendments created the Trigger Limits. |
| September 2015 | Valve failure; release through a diversion-channel gate "that was open at the time" | About $10M fine (paid 14 April 2016) |
| 8 September 2016 | Ice damaged a pipe | Suspension |
| 28 March 2017 | Pipe rupture "detected" by the monitoring system | Cyanide-addition restriction, 29 March to 15 June 2017; $5.6M fine covering 2016 and 2017 |

**Costs and AISC (p.162; 100%).**

| | 2014 | 2015 | 2016 | 2017 |
|---|---|---|---|---|
| Total $/t processed | 15.66 | 16.18 | 13.47 | 17.32 |
| Gold sold koz | 724 | 629 | 532 | 660 |
| AISC $/oz | 815 | 946 | 769 | 987 |

- Notes to the table: "Selling Costs includes silver credits" and "AISC includes adjustments for inventory".
- LOM opex is $13.58/t processed.
- Closure: $112M discounted; $132M undiscounted.

**Grade control vs resource model, 2017 (p.110).** Tonnes 101.6%; Au grade 106.8%; Au oz 108.5%; Ag oz 111.3%.

**Sales (p.152).**
- All doré is sold to Barrick, "for and on behalf of itself and as agent for" Shandong.
- No hedging.

### 2.3 Zaldívar (100% basis; effective 31 Dec 2011; 100% Barrick)

**Reserves, Table 15-1 (p.8, p.74).** Price $2.75/lb Cu.

| Class | Route | Mt | Cu % | Mlb |
|---|---|---|---|---|
| Proven | Heap leach | 245.0 | 0.616 | 3,328.0 |
| Proven | Dump leach | 75.4 | 0.263 | 436.8 |
| Proven | Stockpile | 65.8 | 0.504 | 731.6 |
| Probable | Heap leach | 151.6 | 0.558 | 1,865.1 |
| Probable | Dump leach | 40.1 | 0.272 | 240.4 |
| P&P | All | 578.0 | 0.518 | 6,602.0 |

**Copper recovery card (p.87–91).**
- Form: TCu recovery = F1·ASCu/TCu + F2·CNCu/TCu + F3·SCu/TCu, with **F1 = 96, F2 = 78, F3 = 40**.
- Acid-soluble recovery: ASCu recovery = 98.47 − 2.42/ASCu.
- Insoluble (SCu) recovery is **capped at 55%**.
- A correction factor of 0.788 applies to Geol_Min codes 7, 8 and 13.
- Route averages: dynamic pad 62.6%; dump 36.0%; overall 59.7%.
- The 2011 cap cut average remaining recovery by about 9% (p.92).
- Dynamic pad: P80 13 mm, 360-day cycle. Spent ore moves to a secondary-leach dump (p.15, p.84–85).

**Reconciliation, January–September 2011 (p.70–71).**
- Grade control: 23,720,733 t @ 0.528% TCu.
- Resource model: 22,608,926 t @ 0.526%.
- The resource model under-calls tonnes and contained Cu by about 5%.

**Costs (p.99–101).**
- Mining: $1.468/t mined.
- Process: $7.35/t, of which the leach facility is $6.24/t.
- G&A: $1.245/t.
- Sustaining capital: $440.8M. Pre-stripping of $193M is treated as opex.
- Closure: $36.2M.

**Products and sales (p.30, p.85, p.96).**
- EW capacity is 150,000 t/y of cathode.
- A fines flotation plant (1,850 tpd) makes concentrate of about 35% Cu for La Negra.
- p.96: "no smelting and refining contracts, no forward sales, and no hedging."

### 2.4 Porgera (100% basis; effective 31 Dec 2011; Barrick 95%)

**Reserves (p.138).** The open-pit price is $1,200. The selected shell is the "$575 Pit (Shell 39)" (p.140).

| Row | kt | Au g/t | Au koz |
|---|---|---|---|
| Open pit Proven | 13,944 | 3.00 | 1,343 |
| Open pit Probable | 31,105 | 2.11 | 2,115 |
| Underground Proven | 3,500 | 7.17 | 807 |
| Underground Probable | 3,624 | 7.56 | 880 |
| Stockpiles Probable | 19,802 | 2.29 | 1,455 |
| Process inventory | – | – | 102 |
| Total | 71,975 | 2.90 | 6,701 |

**Underground cut-off (p.144–145).**
- Calculated at 2.66 g/t ($1,000, $77.14/t, 92% recovery).
- 3.5 g/t was used "based on historical operating costs". RPA recommends a lower cut-off.

**Reconciliation, declared ore mined (DOM) vs ore reserve (OR), five years 2007–2011 (p.146–147).**

| Source | DOM | OR | Ounce variance (INFERENCE) |
|---|---|---|---|
| Underground | 3,141 kt / 850 koz | 2,123 kt / 626 koz | +36% oz |
| Open pit | 15,563 kt / 1,465 koz | 15,657 kt / 1,586 koz | −8% oz |

**2011 costs (p.190–191).**
- $841/oz produced against a budget of $543.
- $81.78/t milled against a budget of $62.56.
- 5.344 Mt milled.

**Recovery and production (p.12, p.81–86).**
- Historical recovery is about 86%.
- Gravity recovery is 22%.
- Concentrate is 14% S.
- Four autoclaves run at 1,750 kPa and 197°C (p.172–174).

**Tenure (p.33–34).**
- The SML "expires in 2019".
- There is "no formal application to extend".
- Mining was to end in 2020, with stockpiles processed through 2025.

**Tailings (p.179–183).**
- Riverine disposal to the Porgera, Lagaip, Strickland and Fly rivers.
- Monitoring: pH and WAD CN sampled every 2 h; sub-ppb trace metals; quarterly reports to DEC; CSIRO peer review.

**Closure (p.185).** $170M.

### 2.5 Source discrepancies (register them; never resolve silently)

**Pueblo Viejo**
1. Naranjo deposition: "until … 2048" (p.253) vs "2030 … until 2044" (p.258).
2. Environmental licence: 0101-23 (p.262) vs 0501-23 (p.263).
3. 2025 throughput: 10,868 kt (p.139) vs mill check-in of 10,715.3 kt (p.222). The definition is not stated.

**Veladero**
4. The "Recovery" row changes basis: for 2016–17 it equals placed recoverable ÷ placed contained; for 2013–15 it does
   not (p.78; INFERENCE).
5. Leach inventory is labelled "contained" in reserves (p.116), but equals the recoverable-basis book (p.78).

**Zaldívar**
6. SCu recovery constant: 91.06 (p.89) vs 94.24 (Table 17-1, p.91).
7. M&I contained copper: 1,225.0 Mlb (p.7 table) vs "1.255 billion" (p.13).
8. Concentrate grade:
   - The text gives about 35% Cu (p.85).
   - The flowsheet shows 30% Cu and "flotation of fines temporarily out of service" (p.86).
   - p.96 says there are no smelting or refining contracts.

**Porgera**
9. Reserve price: "US$575 per ounce" in the summary note (p.9) vs "US$1,200" in the Table 15-1 note (p.138). The $575
   figure is the revenue factor of the selected shell (p.140).
10. Underground declared ore vs reserve: "24% higher" in the text (p.145) vs +36% in the five-year table (p.146).
11. Underground mining recovery: 85–90% (p.139) vs 93% (p.144).
12. Smaller conflicts: SML area 2,350 ha (p.33) vs 2,240 ha (p.25); Hides power 72 MW (p.174) vs 62 MW (p.15).
13. In the actual-vs-budget ounces table (p.86), "recovered" exceeds "fed" in 2009. The labels are likely swapped.

---

## 3. Plan impacts (ranked by value)

### P1. CORRECTION and ADDITION: heap inventory is a recoverable-basis book, and reserves call it "contained"

**Plan sections:** §17.3 (heap card), GB-20, GATE-CRUC-03, §20.8, §25.2 `MetalStage`.

**Evidence.**
- Veladero p.78 (Table 13-3), p.116 and p.79.
- The site's ledger is a roll-forward: Inv_t = Inv_{t−1} + placed recoverable_t − production_t.
  - It reproduces the published leach-pad inventory to ±1 oz for every year 2014–2017.
  - "Placed recoverable" is tonnes × grade × the fixed Table 13-1 piecewise formula. It is not a fitted R∞.
- The reserve row "Leach Inventory 13.5 Mt @ 0.78 g/t, 0.34 Moz" is labelled contained gold (p.116).
  Arithmetically, 13.5 Mt × 0.78 g/t = 0.3385 Moz, which matches the 336,310 oz *recoverable-basis* book.
- In 2017 the book rose by 83,900 oz (+33%). That year production was 21.9% below budget, and a cyanide-addition
  restriction ran from 29 March to 15 June. The identity books a production shortfall as inventory automatically.
- About 0.5 Moz of residual leach "not yet thoroughly leached or flushed" was excluded from reserves (p.79). The
  physical tail can therefore sit above the book, not only below it.

**Proposed changes.**
- **§17.3, heap leach row (append):**

  > "Book convention (FACT, Veladero 2013–2017): sites carry heap WIP as a recoverable-basis roll-forward:
  > Inv_t = Inv_{t−1} + Σ T·g·R_card − produced, where R_card is a fixed grade/ore-type formula. The card exposes
  > R_card (book) and R∞ (posterior) as separate named parameters."

- **GB-20, Equation:**

  > "Book = Σ T·g·R_card − recovered (the site ledger). Latent = Σ T·g·R∞ − recovered − solution/channelling
  > losses. Report Δ = latent − book with its interval."

- **GB-20, Assumptions:** replace "Normal scale-up error" with:

  > "A declared, possibly skewed, scale-up prior. Production shortfalls under regulatory restriction enter the book
  > as inventory, so the restriction is a covariate."

- **GATE-CRUC-03:** add a known-answer fixture.

  > "Veladero 2013–2017 (PDF p.78): the roll-forward reproduces the published inventory to ±1 oz."

- **§25.2:**

  ```rust
  pub enum MetalStage {
      InSitu,
      Mined,
      Contained,
      Recoverable { card: CardRef },
      Recovered,
      Payable,
      Sold,
  }
  ```

  > "A `ProcessInventory` row must declare `Contained` or `Recoverable`. When a source label contradicts the
  > arithmetic, the importer files a discrepancy row and never relabels silently."

### P2. CORRECTION: the point of reference belongs on each statement row, not on the statement

**Plan section:** §24.3 `EstimateStatement`.

**Evidence.** Real reserve statements mix sources and bases inside one statement:
- PV reserves are ROM delivered to the crusher and exclude process inventory (p.212).
- Veladero includes pit, stockpile and leach-inventory rows (p.116).
- Porgera has a "Process Inventory 102 koz" row with no tonnes or grade (p.138).
- Zaldívar splits rows by process route: heap, dump and stockpile (p.74).

**Proposed change (§24.3).**

```rust
pub struct StatementRow {
    pub source: SourceKind,          // InSituOpenPit | InSituUnderground | Stockpile | ProcessInventory
    pub route: Option<ProcessRoute>, // HeapLeach | DumpLeach | Mill | FlotationPox | ...
    pub point_of_reference: PointOfReference, // moved here from EstimateStatement
    pub metal_stage: MetalStage,     // Contained, or Recoverable for process inventory (P1)
    pub tonnes: Option<DryTonnes>,   // None only for ProcessInventory (Porgera 2011)
    /* class, grades, metal, intervals as now */
}
// EstimateStatement gains: pub process_inventory: Included | Excluded,
```

### P3. ADDITION and NEW-IDEA: stockpiles degrade, so a stockpile state needs a metallurgical state

**Plan sections:** §20.8, §17.3, §21.5 NRV, §18.5, §24.4.

**Evidence (PV).**
- Stockpiles hold 6.0 of 20 Moz of reserves (about 30%; p.213).
- They supplied 70.6% of 2025 crusher feed (p.222; INFERENCE).
- "Variability in stockpiled material continued to increase due to weathering of exposed sulfide minerals" (p.133).
- Stockpile POX recovery falls to 74% for BSD above 8.5% Stot (p.136).
- Stockpiles are Probable only because of "sulfur degradation impacting process recoveries" (p.214).
- All stockpiles are assumed PAG (p.269).
- A separate grade-control model tracks Stot, S2, Ctot and Corg in the stockpiles to monitor weathering (p.204).
- Plant Au recovery fell from 92.9% to 75.1% between 2012 and 2025 (p.139). This is not attributed to weathering
  alone, because feed type also changed.

**Proposed change (§20.8, new bullet).**

> "**Stockpile metallurgical state.** Per pile or zone, carry Stot, S²⁻, Ctot/Corg and exposure age as posteriors
> (stockpile drilling, GC records) with a declared degradation model. Crucible's recovery card is indexed by that
> state (PV 2026 Tables 13-4/13-6). The same oxidation latent feeds Freeboard ARD (§18.5). The NRV for a long-term
> stockpile uses the recovery expected at its scheduled reclaim date. The §24.4 support pack shows the
> degradation uncertainty; the class remains the QP's call."

### P4. ADDITION: Freeboard must cover solution-holding heap and valley-fill leach facilities, not only TSFs

**Plan sections:** §18.2, §18.3, §23.4.

**Evidence (Veladero).**
- The permit Trigger Limits are PSSA ≤ 3,927 MASL, LCRS ≤ 3,914.7 MASL and LCRS pumping ≤ 270 m³/day (p.26, p.154).
- An exceedance restricts fresh-water make-up and cyanide addition. Snowmelt is a named driver (p.26).
- The facility's requirement set is the permit plus ICMC certification (p.26), not GISTM.

**Proposed changes.**
- **§18.3, add a facility class row:**

  > "Solution-holding heap/VLF: in-heap solution level vs permitted limit; LCRS level and pumped volume as
  > liner-leak indicators; a seasonal melt forecast registered as P(level > limit); a permit-derived requirement
  > set (§18.2 accepts permit-derived as well as GISTM requirement sets). The evidence state is rendered, never
  > 'within limits' from stale data (FMINE-REF-404)."

- **§23.4:** add an operation-level state `Restricted{reagent|water}`, distinct from suspension. The fixture is
  Veladero's cyanide-addition restriction of 29 March to 15 June 2017.

### P5. ADDITION (safety lesson): gate and valve position is safety-relevant state

**Plan sections:** §31.3, §18.7 (G-FREE-2).

**Evidence (Veladero).**
- 2015: a valve failure became an off-site release through a diversion-channel gate "that was open at the time"
  (p.14, p.27).
- 2016: ice from the pad slope damaged a pipe.
- 2017: the monitoring system detected a rupture.

**Proposed change.**

> "Manual gate and valve positions on solution and tailings circuits are held as attested observations with a
> freshness bound. A stale or unknown position renders as `position unknown`, never as 'contained', and widens
> G-FREE-2 to cover containment statements. FrankenMining still has no write path to these devices."

### P6. RISK and ADDITION: a reserve can depend on a tenure or permit that is not yet granted

**Plan sections:** §24.3 `ModifyingFactorClaim`, §23.4, §35.

**Evidence.**
- **Porgera:** the SML "expires in 2019"; there was "no formal application to extend the SML"; stockpiles were to
  be processed through 2025 (p.33–34).
- **PV:**
  - Naranjo tailings from about 2030 underpin the reserve LOM to 2048–2049.
  - The starter-dam permit application was due in Q1 2026, with the permit "expected within 12 months" (p.264).
  - About 680 households are to be resettled (p.20), under a "3 attempt" deal strategy (p.24).

**Proposed changes.**
- **§24.3:**

  > "`ModifyingFactorClaim::Tenure{instrument, expiry, renewal_state: Granted | Applied | Pending{expected} |
  > NotApplied | Refused, depended_on_by: DateRange}`. New guard G-HALL-6: a scheduled reserve period after an
  > expiry, or one that depends on a permit whose state is not `Granted`, is highlighted in the QP pack and in the
  > §35 register. The software does not decide the class."

- **DATA-GATE:**
  - The plan's §23.4 citation "Porgera 2020" is not evidenced by this corpus. The 2012 report predates it.
  - The Q1 2026 label "Porgera (24.5%)" shows that the ownership changed, not why.
  - The plan should cite an external primary source for the 2020 events.

### P7. ADDITION: the tailings disposal method must include riverine disposal

**Plan section:** §18.3, "Construction | Method" row.

**Evidence (Porgera).**
- Riverine disposal was selected because "it is very difficult to ensure a stable foundation for a tailings
  storage facility" (p.179).
- The discharge route runs through the Porgera, Lagaip, Strickland and Fly rivers (p.182).
- Monitoring: sampling every 2 h; DEC reporting; CSIRO peer review (p.182–183).
- There was a historic downstream soluble-Cu exceedance (p.174).
- Paste backfill has been used since 2011 (p.179).
- PV co-disposes PAG in Naranjo (p.254).

**Proposed change.**

> "`TailingsDisposalMethod { Impoundment{raise: Upstream|Centreline|Downstream}, FilteredDryStack, InPit,
> PasteBackfill, CoDisposalWithWaste, Riverine, Submarine }`. For riverine disposal the twin of record is the
> downstream compliance-monitoring network and its exceedance history. There are no embankment failure modes, and
> GISTM's facility framing does not apply."

### P8. CORRECTION: recovery cards have layers, caps, versions and an assumed route

**Plan sections:** §17.3, §15.

**Evidence.**
- PV: stage "operational factors can be applied … in development of business plans" (p.137).
- PV: the reserves assume the "conservative" flotation+POX route for all ore (p.218).
- Zaldívar: insoluble Cu recovery is capped at 55%; a 0.788 factor applies; the cap cut recovery by about 9%
  (p.91–92).
- Zaldívar: the copper card is speciation-weighted, with F1/F2/F3 = 96/78/40 (p.91). It is not an R(t) curve.
- Porgera: the recovery equations "are not used in the cut-off-grade calculations or the Resource and Reserve
  models" (p.12).

**Proposed change (§17.3, after the table).**

> "Card layers: (1) test-work card with its validity box; (2) named stage operational factors, attested and
> versioned; (3) caps and correction factors as explicit parameters; (4) the route assumed for reserves as a
> ModellingDecision. Cu heap/dump card: TCu_rec = F1·ASCu/TCu + F2·CNCu/TCu + F3·SCu/TCu with residence time in
> the validity box. If Pushback or Hallmark use a recovery other than Crucible's current card, the statement shows
> the divergence."

### P9. VALIDATION and demonstrator data for Touchstone

**Plan sections:** §20.1, §20.5, §20.7, G-TOUCH-3.

**Evidence.**
- **PV Table 15-10 (p.222)** is a complete stock-and-flow fixture: opening and closing stocks, reclaim, pit feed,
  cone change, GC call, check-in and check-out.
  - Stockpile additions must be inferred (about 4,560 kt at about 1.43 g/t). That confirms why G-TOUCH-3 refuses a
    mined ÷ milled ratio across a stockpile.
- **Porgera (p.146–147)** shows source-specific biases of opposite sign over five years:
  - underground declared ore +36% oz against the reserve;
  - open pit −8% oz.
- Further fixtures: PV F1A/F1B (p.207); Veladero GC/RM 108.5% oz (p.110); Zaldívar resource model under-calling by
  about 5% (p.70–71).

**Proposed change.** Add the PV 2025 MCF and the Porgera 2007–2011 DOM/GC/OR tables to the §20.10 known-answer
set. Bias priors (§20.5) should be keyed by mining source (open pit, underground, stockpile).

### P10. ADDITION: ownership is bitemporal and carries an offtake arrangement

**Plan sections:** §24.3 `OwnershipRef`, §21.5 part 12, §20.1.

**Evidence.**
- Veladero went from 100% to 50% on 30 Jun 2017, inside the reported 2013–2017 series. Doré is sold to Barrick
  "as agent for" Shandong (p.152).
- Porgera parties "take … the gold production" in kind (p.33). Barrick's share was 95% in 2011 and is labelled
  24.5% in Q1 2026.
- Zaldívar's layback with MEL means ore on the other side of the boundary "would be stockpiled for MEL to process"
  (p.77–78).

**Proposed change.**

> "`OwnershipRef{valid_from, valid_to, interests, arrangement: InKind | Entity | MarketingAgent{agent}}`. Material
> lots carry `owner` when a boundary agreement exists. Series that cross an ownership change are rendered
> per-interval."

### P11. ADDITION: Hallmark renderer attributes and price-deck structure ("store once, render many")

**Plan sections:** §24.3, §24.6, §24.7.

**Evidence: 2012/2018 vs 2026 disclosure.**

| Item | 2012/2018 RPA reports | 2026 PV report |
|---|---|---|
| QPs | Independent | Barrick employees |
| Resources vs reserves | Exclusive | Inclusive; two significant digits (p.208) |
| F1A/F1B/MCF tables | No | Yes |
| Rated risk table | No | Yes |
| GISTM consequence class | No | Yes |
| IAS 37 closure split | No | Yes |
| Item 22 | Omitted under the producing-issuer exemption | Omitted under the producing-issuer exemption |

**Evidence: price.**
- Veladero's pit limit was set at $1,200 and a 15% discount rate, with the design taken from a $900 shell (p.117).
- Porgera selected the "$575 Pit (Shell 39)" at a $1,200 price (p.140), and its summary misstates the price as $575.

**Proposed changes.**
- **Statement attributes:**

  > "`resource_inclusion`, `rounding: RoundingRule`, `qp_relationship: Independent | Employee`, `item22:
  > Included | ProducingIssuerExemption`; `price_deck` → `{evaluation: DeckRef, shell_selection:
  > Option<{revenue_price, discount_rate}>}`."

- **Test:** render the PV 2026 rows from unrounded internal rows and match the published table to two significant
  digits.

### P12. ADDITION: Corebox data verification

**Plan sections:** §12.1, §12.4, §24.5.

**Evidence.**
- **Porgera:**
  - "There are no 'certificates' issued. Consequently, there is no way to validate the assay results" (p.79).
  - Sulphur values above 100% were found in the database (p.79–80).
  - No field duplicates are taken, and gold repeatability is poor (p.80).
- **PV:** Rosario-era and MIM data have no known QA/QC (p.106–108).

**Proposed changes.**
- **§12.1 `LabCertificate`:**

  > "The absent-certificate state `LimsDirectNoCertificate` is explicit; §24.5 reports its share per domain."

- **`AssayResult`:** add `qaqc_regime: None | Partial | Current` per campaign.
- **New proof obligation:** analyte mass fractions must lie in [0, 1] on import.
- **§12.3 flag:** `FieldDuplicateAbsent`, meaning the sampling-error decomposition is not identifiable.

### P13. VALIDATION: GB-33's ±15% at 90% heuristic is used verbatim

**Plan sections:** GB-33, §24.4.

**Evidence (PV p.195).**
- Measured: ±15% at 90% confidence on quarterly volumes.
- Indicated: ±15% at 90% confidence on annual volumes.
- Inferred: ±30% on annual volumes.
- Also a non-CIM "Inventory" class (≤ 300 m).

**Proposed change.** Cite this as the real-practice anchor in GB-33. The §24.4 support pack must allow internal
non-CIM classes that never render publicly.

### P14. ADDITION: the AISC metric registry needs definition flags

**Plan section:** §21.6.

**Evidence.**
- Veladero AISC was $815/$946/$769/$987 for 2014–2017. The notes say "Selling Costs includes silver credits" and
  "AISC includes adjustments for inventory" (p.162).
- PV's opex of $50.12/t is "without by-product credits" (p.278).
- Porgera's 2011 cost of $841/oz is a departmental cost per ounce produced, not AISC (p.190).

**Proposed change.** Add registry fields:
- `byproduct_credit_location: SeparateLine | InsideSellingCosts | Excluded`;
- `includes_inventory_adjustments: bool`;
- `per_oz_denominator: Produced | Sold`.

### P15. ADDITION: reagents can be a mined stream

**Plan section:** §17.7.

**Evidence (PV).**
- The LOM includes 321,561 kt of quality limestone against 217,649 kt of pit ore (p.237).
- Limestone use is 307–400 kg/t (p.246).

**Proposed change.**

> "Reagent supply can be a mined stream (quarry limestone at POX sites), scheduled in Pushback with its own cost
> and quality model."

### P16. ADDITION and RISK: closure needs three figures and a completeness check

**Plan sections:** §18.6, §21.5, §25.2 `MaterialOf`.

**Evidence.**
- PV closure has three figures: $569M LOM; $273M in the PER ($126M discounted); $250M of assurance (p.276).
- Veladero stores by-product mercury in 82 L flasks. Its removal "is not in the Closure Plan" (p.145). In 2016,
  134,186 kg was produced and none was sold (p.78).

**Proposed changes.**
- §18.6 renders the LOM undiscounted, provisioned discounted and assurance figures side by side.
- `MaterialOf` gains `HazardousByproduct(Hg)`, with custody records.
- Closure completeness check: every stored hazardous inventory must map to an ARO line.

### P17. CORRECTION (small): G-FREE-1 and source wording

**Plan sections:** §18.1, §18.7 G-FREE-1.

**Evidence.** The PV source says "The Naranjo TSF will safely store both tailings and PAG waste rock" (p.253) and
uses "safely pass the Inflow Design Flood" (p.257).

**Proposed change.**

> "G-FREE-1 applies to system-authored text. A source document's wording is imported as an attributed `Assertion`,
> rendered in quotation marks with author and locator, and never restated without attribution."

---

## 4. Proposed evidence rows (E251–E299)

`E### | claim | source + page | quote ≤ 25 words | class`

E251 | PV technical-report figures are 100% basis; Barrick attributable 60% via PVD (Newmont 40%) | PV TR 2026 p.15, p.27, p.208, p.213 | "Barrick's attributable share of the Mineral Reserve is 60% based on its interest in PVD." | FACT

E252 | PV P&P at 31 Dec 2025 is 310 Mt @ 2.06 g/t (20 Moz Au) at $1,500. Stockpiles are Probable only: 92 Mt @ 2.04 g/t, 6.0 Moz (100%). | PV TR p.213 | "Stockpiles Probable 92 2.04 6.0 13.59 40" | FACT

E253 | PV stockpiles are classed Probable because of carbon uncertainty and sulphur degradation affecting recovery | PV TR p.214 | "Existing stockpiles have been classified as Probable Mineral Reserves due to uncertainty on carbon estimates and sulfur degradation impacting process recoveries." | FACT

E254 | PV stockpile metallurgical variability is increasing through weathering | PV TR p.133 | "variability in stockpiled material continued to increase due to weathering of exposed sulfide minerals" | FACT

E255 | PV stockpile POX recovery is a matrix of ore type × Stot class: BSD 87/87/74%, VCL/SP 88/82/82% | PV TR p.136 | "(L1 < 7.0% Stot, L2 between 7.0% to 8.5% Stot and L3 > 8.5% Stot)" | FACT

E256 | PV business plans apply operational factors to stage recoveries on top of the test-work cards | PV TR p.137 | "operational factors can be applied to individual stage recoveries in development of business plans to account for known differences in" | FACT

E257 | PV plant Au recovery fell from 92.9% (2012) to 75.1% (2025) as head grade fell from 5.05 to 2.38 g/t; 2025 production 630 koz Au (100%) | PV TR p.139 | "All production figures presented are 100% basis." | FACT

E258 | Stockpile reclaim was 70.6% of PV's 2025 crusher feed. Implied stockpile additions of about 4,560 kt @ about 1.43 g/t (about 210 koz) are not reported. | PV TR p.222 | "Opening Stocks 95,811 2.07 14.26 6,382" | INFERENCE (8,030.5 ÷ 11,377.3; additions = 92,340 − 95,811 + 8,030.5 kt and 6,035 − 6,382 + 557.1 koz; rounding ignored)

E259 | PV 2025 MCF (mine call vs plant check-out): tonnes 95%, Au oz 99%, Ag oz 97% | PV TR p.222 | "2025 Mine Call Factor (MCF) reconciliation between the mine call and plant check-out is 95% for tonnes, 99% for gold ounces" | FACT

E260 | PV F1A gold metal was 101%, 100% and 105% for 2023, 2024 and 2025. 2025 covers January–October only. | PV TR p.207 | "Note: 2025 results reflect reconciliation for the period January through October." | FACT

E261 | PV reserves are stated at ROM delivered to the crusher and exclude process inventory | PV TR p.212 | "Presented as ROM grades and tonnage delivered to the primary crushing facilities; Excludes material considered to be within process inventory." | FACT

E262 | PV resources are inclusive of reserves and rounded to two significant digits | PV TR p.208 | "Mineral Resources are inclusive of Mineral Reserves." | FACT

E263 | PV classification uses ±15% at 90% for quarterly volumes (Measured) and annual volumes (Indicated), and ±30% annual (Inferred) | PV TR p.195 | "Measured ±15% Error in tonnes, grade and metal at 90% confidence for quarterly volumes" | FACT

E264 | The PV LOM quarries 321,561 kt of quality limestone, against 217,649 kt of pit ore | PV TR p.237 | "Quality Limestone kt 321,561" | FACT

E265 | PV reserve optimisation assumes the costlier flotation+POX route for all ore; LOM recovery is 80.1% Au | PV TR p.218 | "assumed a "conservative" case using the more expensive stream of flotation and POX for costs and recoveries" | FACT

E266 | The Naranjo TSF is designed to the GISTM "Extreme" consequence class, with a 1/10,000-year seismic event at PGA 0.92 g | PV TR p.256–257 | "The Naranjo TSF dam will be designed for "Extreme" consequence classification" | FACT

E267 | The PV source itself uses "safely store" wording (relevant to G-FREE-1) | PV TR p.253 | "The Naranjo TSF will safely store both tailings and PAG waste rock." | FACT

E268 | Naranjo TSF capex is $986.4M of $1,503.8M expansion capex; LOM capex is $4,177.8M (100%) | PV TR p.277–278 | "Naranjo TSF 986.4" | FACT

E269 | The Naranjo starter-dam permit was not yet granted at the report date | PV TR p.264 | "the permit application is on track for submission in Q1 of 2026, with the permit expected to be received within 12 months" | FACT

E270 | Naranjo requires resettlement of about 680 households | PV TR p.20 | "for the project with approximately 680 households affected" | FACT

E271 | PV LOM closure is $569M: $273M is in the PER ($126M discounted) and $296M is not. Assurance is $250M. | PV TR p.276 | "Closure liabilities for the LOM are estimated to be US$569M with US$273M ($126M discounted" | FACT

E272 | PV LOM opex is $50.12/t ore, stated without by-product credits | PV TR p.278 | "Total Operating Costs (without by-product credits) 50.12" | FACT

E273 | Veladero (MAGSRL 50/50) sells all doré to Barrick, acting for itself and as agent for Shandong | Veladero TR 2018 p.152 | "sold to Barrick, for and on behalf of itself and as agent for Shandong Gold Mining (HongKong) Co., Limited" | FACT

E274 | Veladero reserves include leach inventory of 13.5 Mt @ 0.78 g/t (0.34 Moz), defined as "contained" gold on the heap | Veladero TR p.116 | "Leach inventory reported in reserves consists of contained gold in ore stacked on the heap leach that has yet to be" | FACT

E275 | Veladero's leach-pad inventory is a recoverable-basis roll-forward: Δinventory = placed recoverable − production, ±1 oz, 2014–2017 | Veladero TR p.78 | "Placed recoverable Au (oz) 725,003 561,328 588,241 739,923 671,344" | INFERENCE (for example, 2017: 336,310 − 252,410 = 83,900 = 725,003 − 641,103)

E276 | The "contained" reserve leach inventory equals the recoverable-basis book | Veladero TR p.116, p.78 | "Leach Inventory 13.5 0.78 - 0.34 -" | INFERENCE (13.5 Mt × 0.78 g/t = 0.3385 Moz ≈ book of 336,310 oz)

E277 | Veladero's reported "Recovery" changes basis: 2016–17 = placed recoverable ÷ placed contained; 2013–15 does not match | Veladero TR p.78 | "Gold 77.0% 77.6% 73.3% 72.8% 72.3%" | INFERENCE (ratios 76.95, 77.57, 78.23, 78.24, 76.63%)

E278 | In the 2017 cyanide-restriction year, book inventory rose 83,900 oz (+33%) while production was 21.87% below budget | Veladero TR p.77–78, p.28 | "Total Production Au (oz) 641,103 820,542 -21.87%" | INFERENCE (from FACT inputs; restriction 29 March–15 June 2017)

E279 | About 0.5 Moz Au of residual leach (2024–2028) is excluded from Veladero's reserves | Veladero TR p.79 | "This residual leach would be designed to recover gold not yet thoroughly leached or flushed from the VLF" | FACT

E280 | Veladero VLF permit Trigger Limits: PSSA ≤ 3,927 MASL; LCRS ≤ 3,914.7 MASL and ≤ 270 m³/day | Veladero TR p.26 | "permitted operating level limited to 3,927 metres above sea level (MASL) was enacted in 2014 to control the hydraulic head" | FACT

E281 | A Trigger Limit exceedance (for example from snowmelt) restricts water make-up and cyanide addition | Veladero TR p.26 | "trigger the VLF contingency plan and restrictions that constrain fresh water make-up and cyanide addition to the leach pad" | FACT

E282 | The September 2015 release passed through an open diversion-channel gate; the fine was about $10M | Veladero TR p.27 | "release of cyanide-bearing process solution into a nearby waterway through a diversion channel gate" | FACT

E283 | 2016: ice damaged a pad pipe. 2017: the monitoring system detected a rupture; fine $5.6M for 2016–2017. | Veladero TR p.27–28 | "On March 28, 2017, the monitoring system at the Mine detected a rupture of a pipe carrying" | FACT

E284 | Veladero AISC (100%) was $815/$946/$769/$987 for 2014–2017, with silver credits inside selling costs | Veladero TR p.162 | "1. Selling Costs includes silver credits. 2. AISC includes adjustments for inventory." | FACT

E285 | Veladero by-product mercury is stored on site; its removal is not in the Closure Plan; 134,186 kg produced in 2016, none sold | Veladero TR p.145, p.78 | "The cost of removal at end of life will need to be addressed by operations, as it is not in the Closure Plan" | FACT

E286 | Veladero 2017 grade control vs resource model: tonnes 101.6%, grade 106.8%, Au oz 108.5% | Veladero TR p.110 | "Percent Differences 101.6% 106.8% 108.5% 109.5% 111.3%" | FACT

E287 | Veladero's pit limit used $1,200 and a 15% discount rate; the design came from a $900 shell | Veladero TR p.117 | "final pit limit for Veladero was determined based on analysis completed using a 15% discount rate and US$1,200/oz gold price" | FACT

E288 | Zaldívar reserves are split by route (heap, dump, stockpile): P&P 578.0 Mt @ 0.518% Cu, 6,602 Mlb, 100% Barrick (2011) | Zaldívar TR 2012 p.74, p.7 | "which is 100% owned by Barrick" | FACT

E289 | Zaldívar's copper leach card is speciation-weighted with F1 = 96, F2 = 78, F3 = 40 | Zaldívar TR p.87, p.91 | "The values used for the correlation parameters were: F1= 96, F2= 78, F3= 40." | FACT

E290 | Zaldívar's 2011 cap of 55% on insoluble Cu cut average remaining recovery by about 9% | Zaldívar TR p.91–92 | "recovery for the remaining resources at Zaldívar decreases by approximately 9%" | FACT

E291 | Zaldívar had no smelting or refining contracts, no forward sales and no hedging | Zaldívar TR p.96 | "Currently, there are no smelting and refining contracts, no forward sales, and no hedging." | FACT

E292 | Under the Zaldívar–MEL layback, ore found across the boundary is stockpiled for the other owner | Zaldívar TR p.77–78 | "if any ore grade material is intersected on the MEL side of the property boundary, this would be stockpiled for MEL to process" | FACT

E293 | Porgera report figures are 100% (Barrick 95% in 2011). Barrick's Q1 2026 stats label it "Porgera (24.5%)". | Porgera TR 2012 p.138; Q1 2026 xlsx QTD!A14 | "These represent 100% of the Mineral Reserves, and not the 95% attributable to Barrick." | FACT

E294 | Porgera reserves include process inventory of 102 koz with no tonnes. The pit is the $575 shell evaluated at $1,200. | Porgera TR p.138, p.140 | "Whittle Shell Selected $575 Pit (Shell 39)" | FACT

E295 | Porgera's recovery equations were not used in cut-off or reserve models | Porgera TR p.12 | "As a result the equations are not used in the cut-off-grade calculations or the Resource and Reserve models." | FACT

E296 | Porgera's reserve LOM ran past the SML expiry (2019) without a formal extension application | Porgera TR p.33–34 | "there has been no formal application to extend the SML" | FACT

E297 | Porgera uses riverine tailings disposal because a stable TSF foundation is very difficult to ensure | Porgera TR p.179 | "it is very difficult to ensure a stable foundation for a tailings storage facility, therefore, riverine tailings disposal is the preferred option" | FACT

E298 | Porgera 2007–2011: underground declared ore mined vs reserve is +36% oz in the table (which sums 2008–2011 = 626 koz OR); the text says 24% (including 2007's 58 koz OR → 685 koz) | Porgera TR p.145–147 | "declared ore ounces that are mined for the entire project, underground mining and open pit mining are 7% higher, 24% higher and 8% lower" | FACT (text/p.147) + INFERENCE (850 ÷ 626 = 1.358 vs 850 ÷ 685 = 1.241)

E299 | Porgera assay results arrived without certificates and could not be validated | Porgera TR p.79 | "there are no "certificates" issued. Consequently, there is no way to validate the assay results." | FACT

---

## 5. Checked nulls (looked for; not in these four documents)

**Sales terms and products**
- No copper cathode sales terms, premiums, LME grade data or cathode custody records (Zaldívar).
- No concentrate terms (TC/RC, payables) for the Zaldívar fines concentrate.
- The Veladero doré agreement gives no refining terms.

**Production series**
- Zaldívar gives no historical cathode production by year. Only ore movements (p.31) and EW capacity are given, so
  there is no cathode check-out anchor for §17.3.
- No acid consumption figure for Zaldívar (kg/t or t/y). Cut-offs depend on acid consumption (p.77), but no value
  is given.
- Veladero gives cyanide, lime and zinc only in $/t (p.163–164), with no kg/t.

**Heap leach**
- No measured field leach curves (R vs t) for either heap leach. Only bottle-roll and column test durations and
  fixed formula recoveries are given.
- No heap solution inventory, apart from Merrill-Crowe feed volumes and solution grades.

**Tailings, closure, cost metrics**
- No GISTM consequence class or classification for Veladero, Porgera or Zaldívar.
- No EoR or ITRB information in any of the four reports. PV mentions a Board of Consultants advising INDRHI (p.264).
- No AISC in the PV, Porgera or Zaldívar reports. There is no by-product vs co-product AISC split anywhere; the
  only by-product treatment is Veladero's silver-credit note and PV's "without by-product credits".

**Licence and ownership**
- Porgera's 2020 lease non-renewal and reopening are not in the corpus slice. The 2012 report predates them.
- The cause of Porgera's ownership change, and any later Zaldívar ownership change, are not in the corpus slice.

**Other**
- Section 22 economic analysis is omitted in all four reports under the producing-issuer exemption. There are no
  NPV or cash-flow tables to reproduce.
- Porgera's flotation, sulphur and CIP equation constants (p.82–84) are not usable as FACT, because the extraction
  is garbled and the tables were not re-verified visually.
