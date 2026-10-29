# W4: M2 site choice and P(site data by month 7)

Agent W4 (evidence block E551–E599), read-only pass, 2026-10-28/29. Scope: plan §37.3 M2 "Site preference", §34.4
"The probability that matters most", §37.2 GB-38, and C1 §3.7 and plan impact 8.

**Method and provenance.**
- Primary per-site numbers were read cell by cell from Barrick Mine Stats workbooks with a stdlib xlsx reader
  (`python3 -I`, scratchpad `w4/scripts/xl.py`). Q1 and Q2 2026 are the corpus files C1 used. Q1 2024–Q4 2025 are
  in `corpus_of_input_docs/downloaded/mine_stats/`, downloaded by a parallel agent and **not yet in the C1 source
  registry**. Their sha256 prefixes are listed in §8.
- Quarterly MD&A narrative was read from the PDFs in the same folder (`pdftotext`). The Barrick 2025 Annual
  Information Form (AIF, dated 27 Feb 2026) was downloaded from Barrick's site to the scratchpad
  (sha256 `d8bc75c700aa8892`).
- Web evidence came from WebSearch/WebFetch. Both tools hit a usage limit mid-run and the work resumed after the
  reset. WebFetch truncates long SEC pages, so MD&A site sections were read from the local PDFs instead.
- **Process note.** While working I removed two 4.7 KB SEC "undeclared user agent" error pages that I had just
  written into my own scratchpad. No project file was touched except this output.

**Ownership basis (applies everywhere).**
- Lumwana is 100%, so its attributable and 100% figures are identical.
- North Mara and Bulyanhulu `Minestats3` physicals are **attributable 84%** (C1 E109). The Tanzania deck values are 100%.
- NGM figures are Barrick's 61.5%. NGM appears here only as ownership and governance facts. No NGM operating data
  is used.

---

## 1. RESULT_STATUS: MATERIAL_INCREMENT

1. **Keep Lumwana as the M2 primary, and change the reason.**
   - Lumwana is Barrick's **only wholly owned operating mine** in October 2026. Hemlo was sold on 26 Nov 2025 and
     Tongon on 1 Dec 2025 (E551–E554).
   - It sits outside the NGM/Newmont JV and outside the North American Barrick IPO perimeter, which is where
     Avathon is deploying (E563, E590–E591).
2. **The recovery gap did not close in 2025. Grade masked it.**
   - Headline recovery rose from 85–88% (2024H1) to 91–92% (2025–2026H1).
   - Head grade also rose, from 0.41–0.45% to 0.57–0.67% Cu.
   - Measured against the 2013–22 grade–recovery relation, every 2025–26 quarter is still **1.7–2.7 points
     below expectation**: 2.4–3.4% of output, about **$30–57M a year** at $4.00–5.50/lb (INFERENCE, E566–E567).
   - Barrick calls 2021–25 recovery (91.15%, fresh plus transitional ore) "reasonable" and publishes no mechanism
     (E569, E573).
3. **Recovery-led variance helps the pilot, with conditions** (§5.2).
   - *Supported:* the gap is material, persistent and forward-relevant. The $2bn Super Pit assumes 92.7%
     fresh-ore recovery in a parallel plant with a similar flowsheet, and one point is worth about 2.6 kt Cu a
     year at 240 ktpa (E570–E572).
   - *Not supported:* that the finding will be *new to the site*. To count as new, it must be an **attribution**
     (sampling/assay vs ore type vs operations) and a calibrated card, not "recovery is low".
   - *Hurts:* plant events in every year of the window (E580–E584), and new-plant commissioning from H2 2027
     (E131, E578).
4. **Replace P(site data by month 7) = 0.5 with 0.3 for Lumwana** (plausible range 0.15–0.45; §6).
   - Decomposition: sponsor 0.6 × legal 0.8 × IT 0.6.
   - The fastest Barrick analogue, Avathon, needed about 4–6 months to reach operating sites, and it had a
     Chair-level link (E591).
   - The other sites score lower: North Mara about 0.15–0.2, Kibali about 0.1–0.15, NGM and Pueblo Viejo below
     0.1.
   - GB-38's choice of H-f is unchanged (EV 4.7 vs 2.3 at p = 0.3). M2′ becomes the modal path.
5. **Ranked alternates:**
   1. North Mara with Bulyanhulu, under one Twiga data-governance agreement.
   2. Kibali, if AngloGold Ashanti consent can be had.

   The decision rule is in §5.3.

---

## 2. Barrick portfolio and ownership, October 2026

| Asset | Barrick interest | Other holders | Operator | Post-IPO perimeter | Status, Oct 2026 | Vendor data-access friction (INFERENCE unless cited) | Sources |
|---|---|---|---|---|---|---|---|
| **Lumwana** (Zambia, Cu) | **100%** via Lumwana Mining Company (LMC) | none | Barrick (LMC) | Remaining Barrick ("RemainCo") | Operating; $2bn Super Pit under construction | **Lowest.** No partner consent. Zambia DPA s.70 localises *personal* data only (E596); an on-prem binary with no egress avoids it | E551, Q2_2026:QTD!A27 |
| Carlin, Cortez, Turquoise Ridge, Phoenix (NGM) | 61.5% | Newmont 38.5% | Barrick, under a board of 3 Barrick + 2 Newmont managers; technical committee with equal representation | North American Barrick | Operating; disputes settled 9 Aug 2026; Fourmile, Mike and Fiberline vended in; $1.95bn top-up | **Highest.** The Feb 2026 default notice alleged "operational and accounting issues" (E557); Avathon is live (E591) | E556–E558 |
| Fourmile (Nevada) | 100% today, being contributed to NGM | (Newmont after vend-in) | Barrick | North American Barrick | Exploration/PEA | Walled lane (plan §11.7); not a candidate | E558, C1 E139 |
| Pueblo Viejo (DR) | 60% | Newmont 40% | Barrick | North American Barrick | Operating; Naranjo TSF | High: Newmont JV; reported as Avathon's "next implementation location" (E591) | AIF p.31; Q2_2026:QTD!A10 |
| Veladero (Argentina) | 50% | Shandong Gold 50% | Shareholders' agreement | RemainCo | Operating (heap leach) | High (shared governance) | AIF p.30 |
| Porgera (PNG) | 24.5% | Zijin 24.5% (via PJL); PNG stakeholders 51% | Barrick Niugini (Barrick/Zijin 50/50) | RemainCo | Operating since Dec 2023 restart | High | E555 |
| Loulo-Gounkoto (Mali) | 80% | State of Mali 20% | Barrick. Control was lost 16 Jun 2025; the 24 Nov 2025 agreement ended the disputes | RemainCo | Operating again; AISC guidance $2,640–2,900/oz | High (political) | Q2_2026:QTD!A34 |
| Kibali (DRC) | 45% | AngloGold Ashanti 45%, SOKIMO 10% | **Barrick** | RemainCo | Operating | Medium-high: needs AngloGold consent | E560 |
| North Mara, Bulyanhulu (Tanzania) | 84% | Government of Tanzania 16%, free-carried, via Twiga | Barrick (Twiga) | RemainCo | Operating; Gokona pre-strip at North Mara | Medium: GoT board seats (E562); PDPA permit regime for personal data (E597) | C1 E135; Q2 2026 MD&A |
| Zaldívar (Chile) | 50% | Antofagasta 50% | **Antofagasta** (non-operated) | RemainCo | Operating | High: Barrick "will rely on the operators for accurate information" | E559; Q1 2024 MD&A |
| Jabal Sayid (Saudi Arabia) | 50% | Partner not named in the sources read (believed to be Ma'aden; UNVERIFIED) | Not determined | RemainCo | Operating | High | AIF p.31 |
| Reko Diq (Pakistan) | 50% | GoB 10% free-carried + 15% via an SPC; federal SOEs 25% | Barrick | RemainCo | **Under review since 5 Feb 2026** | High | E561 |
| *Divested* | Hemlo (to Carcetti Capital Corp, closed 26 Nov 2025); Tongon (to Atlantic Group, closed 1 Dec 2025); Donlin 50% and Alturas (2025, R6) | | | | | | E553, E554 |

**Conclusions.**
- **Lumwana is the only 100%-owned operating mine** (INFERENCE from the Q2 2026 labels and the disposals; E552).
- Every other operation has a partner or state shareholder with governance rights, and NGM and PV also sit in
  the Avathon perimeter.

**JV consent for vendor data sharing (Goal 1.1).**
- *Searched for:* any public text on whether the NGM, Kibali, PV or Twiga agreements require partner consent to
  share operating data with a vendor. *Found:* none (checked null).
- **Public facts:**
  - NGM is managed by its board, with "equal representation" on its technical committee (E556).
  - Newmont's Feb 2026 notice cited "operational and accounting issues" (E557).
  - Secondary reports say Newmont exercised inspection and audit rights, and that both CEOs cited the agreement's
    confidentiality terms when declining to comment ([Skillings](https://skillings.net/newmont-issues-default-notice-to-barrick-over-nevada-gold-mines-jv/),
    FACT[secondary], not quoted).
  - The settlement "includes revisions to the NGM joint venture agreement" (Q2 2026 MD&A). The revised text (6-K,
    17 Aug 2026; R6) was not read here.
- **INFERENCE:**
  - In 2026, NGM operating data is the most consent-sensitive data in the portfolio.
  - Any vendor touching it should assume a technical-committee or board-level disclosure.
  - Twiga and Kibali probably need a board paper rather than a contract amendment. This is OPEN.

---

## 3. Lumwana dossier

### 3.1 Identity

- **Pits and plant:** two pits (Chimiwungo and Malundwe) and a "conventional sulphide flotation processing
  plant", fed by two primary crushers through a SAG-ball circuit (AIF p.100–101, E574).
- **Mineralogy:** pyrite, chalcopyrite and occasional bornite in biotite- or muscovite-dominant schist (AIF
  p.101).
- **Workforce:** about 3,260 employees and 6,400 contractors at 31 Dec 2025 (AIF p.100).
- **Grade control:** RC drilling on a 12.5 × 25 m pattern (AIF p.105, E575).
- **Mining fleet:**
  - Komatsu fleet recapitalised in 2021;
  - contractor-to-owner mining transition in 2022;
  - a Fleet Management System "including High Precision GPS Implementation" (LUM p.8, p.10; E585).

### 3.2 Quarterly operating record, 2023Q1–2026Q2 (100% = attributable)

Cells are `Minestats4!D` rows 4, 8, 9, 13, 14, 15, 16 and 26–27 (rows +2 for cost and capex in Q2 2026) of each
quarter's workbook. 2023 values are the "E" comparative columns of the 2024 workbooks. Recovery and grade are
printed rounded (recovery to the whole percent).

| Quarter | Mined kt | Strip | Processed kt | Head Cu % | Recovery | Prod kt | Sold kt | CoS $/lb | C1 $/lb | AISC $/lb | Sust. capex $M | Project capex $M |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 2023Q1 | 17,178 | 2.76 | 6,523 | 0.38 | 87% | 22 | 22 | 3.56 | 3.09 | 3.98 | 26 | 26 |
| 2023Q2 | 26,919 | 2.44 | 6,578 | 0.50 | 93% | 30 | 29 | 2.80 | 2.30 | 3.29 | 44 | 27 |
| 2023Q3 | 37,455 | 4.66 | 6,606 | 0.55 | 91% | 33 | 30 | 2.48 | 1.86 | 3.41 | 85 | 17 |
| 2023Q4 | 32,081 | 3.58 | 7,090 | 0.54 | 87% | 33 | 32 | 2.95 | 2.14 | 3.38 | 68 | 13 |
| 2024Q1 | 29,571 | 6.93 | 6,022 | 0.41 | 88% | 22 | 22 | 3.41 | 2.52 | 4.33 | 75 | 12 |
| 2024Q2 | 39,132 | 6.03 | 6,523 | 0.45 | **85%** | 25 | 25 | 3.15 | 2.14 | 4.36 | 102 | 15 |
| 2024Q3 | 36,809 | 4.96 | 6,380 | 0.53 | 91% | 30 | 26 | 3.27 | 2.53 | 3.94 | 62 | 17 |
| 2024Q4 | 35,354 | 2.34 | 6,858 | 0.71 | 93% | 46 | 36 | 2.27 | 1.89 | 3.14 | 73 | 113 |
| 2025Q1 | 30,310 | 4.05 | 5,237 | 0.57 | 91% | 27 | 34 | 2.80 | 2.22 | 3.20 | 50 | 20 |
| 2025Q2 | 37,481 | 3.89 | 7,082 | 0.67 | 92% | 44 | 39 | 2.25 | 1.58 | 2.79 | 78 | 72 |
| 2025Q3 | 41,678 | 2.97 | 6,392 | 0.66 | 92% | 38 | 37 | 2.32 | 1.68 | 2.93 | 78 | 119 |
| 2025Q4 | 32,205 | 2.86 | 7,029 | 0.65 | 91% | 42 | 47 | 2.76 | 1.97 | 3.24 | 92 | 173 |
| 2026Q1 | 31,064 | 4.37 | 5,911 | 0.59 | 91% | 32 | 30 | 3.31 | 2.67 | 3.83 | 45 | 104 |
| 2026Q2 | 41,542 | 5.74 | 6,946 | 0.64 | 91% | 41 | 40 | 3.14 | 2.35 | 3.83 | 87 | 146 |

**Annual totals.**
- Production (LUM p.39 and AIF p.102):
  - 2023: 118.0 kt at 89.4%;
  - 2024: 122.7 kt at 89.82%;
  - 2025: 150.9 kt, the mine's record, at 25.74 Mt and 0.64%; metal-balance recovery 91.7% (INFERENCE);
  - H1 2026: 73 kt.
- **2026 guidance** is 130–150 kt, AISC $3.40–3.75/lb (AIF p.31).
- **H1 2026 AISC was $3.83/lb** (`Q2_2026:YTD!G27`). That is above the guidance top, and the Q2 2026 MD&A says
  copper cost guidance will be met "albeit at the top end" (E586).

**Workbook hygiene (for M0's DATA-GATE quarters).**
- The 2024 workbooks label row 15 "Production (millions of pounds)". Their own footnote says production is in
  tonnes, and the values are kt (`Q1_2024:Minestats4!A15`, `A29`).
- The AIF prints Lumwana grade as "Average grade processed (grams per tonne) 0.64%" (p.106).

Both are discrepancy-register candidates.

### 3.3 Is the recovery gap real once grade is accounted for? (INFERENCE; scratchpad `w4/scripts/tails.py`)

For a sulphide flotation plant, recovery rises with head grade, so a recovery drop at a lower grade is not a
finding by itself. Two models test whether Lumwana's gap survives a grade adjustment. Both use the 2013–22 annual
record (LUM p.39). Implied tails grade is t = h·(1 − R), ignoring concentrate mass pull, which is about 1–2%.

- **M-a (constant tails).**
  - 2013–22 mean t = 0.036% Cu (range 0.031–0.044).
  - 2023 = 0.052; 2024 = 0.054.
  - 2025–26 quarters = 0.051–0.059.
- **M-b (grade-dependent tails).** OLS on 2013–22 gives t = 0.0114 + 0.0453·h (%Cu), with residual SD 0.0029.

| Period | Expected recovery (M-b) | Actual | Gap (pts) | Shortfall kt Cu (M-a / M-b) | % of output | Value $M at $4.00 / $5.50 per lb |
|---|---|---|---|---|---|---|
| 2023 (annual) | 93.2% | 89.4% | 3.8 | 4.1 / 4.7 | 3.4–4.0% | 36–57 |
| 2024 (annual) | 93.3% | 89.8% | 3.5 | 4.5 / 4.7 | 3.6–3.8% | 39–57 |
| 2025 (quarters 1.7–2.7) | 93.5–93.8% | 91–92% | 1.7–2.7 | 4.6 / 3.6 | 2.4–3.1% | 31–56 |
| H1 2026 (2.5, 2.7) | 93.5–93.7% | 91% | 2.5–2.7 | 2.5 / 2.1 (half-year) | 2.9–3.4% | 18–30 (half-year) |

**Reading.**
1. The 2025 "recovery" improvement that Barrick reports ("higher grades processed and higher recoveries", Q4
   2025 MD&A p.40) is **mostly grade**. On a grade-adjusted basis, the gap roughly halved from 2024 and then
   stopped closing.
2. Every quarter from 2023Q3 to 2026Q2 lies below the 2013–22 relation (M-b) by more than the ±0.5-point
   rounding of the printed recovery. The smallest gap is 2024Q4, at 0.9 points on printed values, and it
   disappears on metal balance (46 kt produced ÷ 48.7 kt contained ≈ 94.5%).
3. Metal balance agrees: processed × grade × stated recovery reproduces production within ±1.5 points in every
   quarter (C1 E127). So the gap is not an artefact of the printed recovery.
4. **Caveats:**
   - annual and quarterly support differ (quarterly scatter is larger);
   - the model ignores ore type, grind, throughput and mass pull;
   - 2013–22 may not be the right baseline if the ore mix changed. Barrick's own wording points to transitional
     ore (E569).

That is why the M2 finding has to be an attribution, not a restatement of this table.

### 3.4 What Barrick says about recovery, ore types, reconciliation and grade control

- **Recovery is "reasonable" and in line with expectations.** "Operating data between 2021 and 2025 indicates
  that the plant has been able to consistently achieve reasonable recoveries, with an average of 91.15%, across
  both fresh ore and transitional material" (AIF p.102, E569).
- **The forward recovery basis is 92.7% fresh**, and Barrick expects no recovery difference for Super Pit ore
  (AIF p.102, E570, E571).
- **Recovery changes are attributed to the mine plan.**
  - Q2 2024 vs Q2 2023: "lower grades processed and recoveries, in line with the mine plan" (Q2 2024 MD&A p.37,
    E573).
  - 2025: "higher grades processed and higher recoveries" (Q4 2025 MD&A p.40).
  - No metallurgical mechanism is published in any 2024–26 MD&A read.
- **Grade reconciliation.** "Reconciliation results confirm the metal correlation between geological model and
  production" (LUM p.19, C1 E130). Derisk drilling covers the plan to June 2028, and phase two runs to the end of
  2026 (LUM p.19).
- **Narrative vs cells (E568).**
  - The Q2 2024 MD&A says Q2 copper rose "driven by higher grades and recoveries at Lumwana" (p.47, also p.7).
  - The cells show recovery **88% → 85%** (`Q1_2024:Minestats4!D14` → `Q2_2024:Minestats4!D14`). The metal
    balance gives 89.1% → 85.2%.
  - The segment narrative on p.37 mentions grade and throughput but not recovery.
  - This is a recovery-specific public discrepancy that M0 can file now (PLAUSIBLE).

### 3.5 Plant and operating events, 2024–2026 (segment boundaries)

| Period | Event | Source |
|---|---|---|
| Q1 2024 | Planned shutdown; lower throughput and grade | Q1 2024 MD&A |
| Q3 2024 | "lower throughput due to grid power instability"; diesel co-generation | Q3 2024 MD&A p.29, E584 |
| Q4 2024 | Sales below production: "prolonged shutdown at one of the third-party smelters"; long-lead down payments | Q4 2024 MD&A p.39 |
| Q1 2025 | "prolonged SAG mill shutdown" | Q2 2026 MD&A p.29, E581 |
| Q3 2025 | "prolonged September shut and crusher down time"; stockpiles built | Q3 2025 MD&A p.29, E582 |
| Q4 2025 | "premature failures" lowered fleet availability; higher power costs | Q4 2025 MD&A |
| Q1 2026 | Planned reline in Feb 2026; "premature component failures" being addressed with OEMs | Q1 2026 MD&A p.23, E583 |
| Q2 2026 | "pebble crusher tie in on the existing plant" | Q2 2026 MD&A p.28, E580 |
| H2 2026 | 200 Mt/y mining run-rate milestone | LUM p.28 (C1 E131) |
| 2027 | Two new primary crushing and overland conveying systems; Chimiwungo in-pit crusher | AIF p.102 |
| H2 2027 – H1 2028 | New-plant commissioning; first copper end Q1 2028; 52 Mtpa ramp-up H2 2028 | E131, E578 |

Two consequences for the pilot:
- The 12-quarter window has no clean steady state.
- Each event is a known covariate that Touchstone must segment on. This also makes a realistic test of the
  "explained rejections" in GATE-PORT-02.

### 3.6 Super Pit Expansion

| Item | Fact | Source |
|---|---|---|
| Pre-FS announcement | 11 Sep 2024: FS "expected to be completed by the end of the year", construction to start in 2025, new plant commissioning "second half of 2027" | Barrick release 11 Sep 2024 |
| FS / decision | "In 2024, Barrick completed a feasibility study". TR effective 31 Dec 2024, issued 19 Feb 2025. No standalone "FID" release was found. The Q2 2026 MD&A cites "$2 billion based on the approved feasibility study". Long-lead down payments began in Q4 2024 | AIF p.100, p.106; Q4 2024 MD&A; E576, E577 |
| Construction | 10 Jul 2025: "Site construction is underway, long-lead equipment orders have been placed"; 2025 was the "first full year of execution" | Barrick release 10 Jul 2025; Q4 2025 MD&A |
| Capex | About $2bn, 2025–2028. $254M spent at 31 Dec 2025; **$610M at 30 Jun 2026** ($120M in Q2). 2026 at the lower end of $750–850M; 2026–27 are the peak-spend years | AIF p.106; Q2 2026 MD&A p.33; E577, E579 |
| Plant | Parallel plant, 27 → 52 Mtpa (54 peak), "similar flow sheet"; mill building on the critical path; long-lead items on site (mill shells, trunnions, primary crusher, thickener) | AIF p.102; Q2 2026 MD&A p.33; E574, E578 |
| Schedule | "on schedule, with first copper production targeted for the end of Q1 2028" | Q2 2026 release (10 Aug 2026), E578 |
| Mine | Mining 141.7 Mt (2025) → 350 Mtpa; 240 ktpa average; LOM to 2057 | AIF p.102, p.106 |
| Economics | AIF base case: NPV8 $3.9bn, IRR 49% at $4.03/lb (whole mine) | AIF p.105 |

### 3.7 Digital and data maturity at Lumwana (public evidence only)

| Item | Evidence |
|---|---|
| Positive | FMS with high-precision GPS (LUM p.10). "High Precision drilling modules" with "real time data" for drill-and-blast evaluation; XY/Z accuracy rose 45/42% → 75/86% (Jan → Jun 2025) (LUM p.11). "Improved process control and automation" in the expansion (LUM p.18). "Real time data monitoring" (LUM p.21). RC grade control at 12.5 × 25 m (AIF). Group SAP S/4HANA since 2020 (R6). |
| Unknown | Plant historian and LIMS vendors; whether monthly metallurgical balances follow AMIRA P754; whether ore-type (fresh/transitional) tags are carried to the mill feed |
| Negative | Site IT and engineering capacity is shared with the $2bn build. Power instability forces generator switching (Q1 2026 MD&A), which degrades time-series continuity |

---

## 4. North Mara, Bulyanhulu and other candidates (brief)

**North Mara (84% attributable; `Minestats3!H/I`).**

| Quarter | Processed g/t | Recovery | Prod koz | AISC $/oz |
|---|---|---|---|---|
| 2024Q1–Q4 | 2.43 / 2.61 / 3.84 / 4.29 | 90 / 89 / 90 / 90% | 46 / 54 / 75 / 90 | 1,753 / 1,491 / 1,052 / 1,098 |
| 2025Q1–Q4 | 3.56 / 3.16 / 2.99 / 2.87 | 88 / 88 / 89 / 90% | 67 / 62 / 64 / 56 | 1,258 / 1,292 / 1,268 / 1,546 |
| 2026Q1–Q2 | 1.81 / 1.80 | 85 / 85% | 33 / 35 | 1,990 / 2,577 |

- **Guidance gap.** H1 2026 is 68 koz at AISC $2,283/oz (`Q2_2026:YTD!C19`, `G19`). FY guidance is 200–230 koz at
  AISC $1,520–1,680/oz, so H2 must deliver 132–162 koz, **1.9–2.4× the H1 rate** (INFERENCE, E587).
- **Cause, per Barrick.** "waste material movement in the Gokona pit while addressing delayed delivery of ore from
  the underground operations" (Q2 2026 MD&A p.24, E588). Higher underground tonnes are expected in H2 2026 (p.22).
- **Classification.** The grade decline is partly planned (the Gokona pre-strip, a project-capex line) and partly
  unplanned (underground ore delays). It is a **plan-execution and sequence variance**, not evidence of a
  model-vs-mill grade bias.
- **Recovery.** The fall to 85% is grade-driven: implied tails fell from 0.38 to 0.27 g/t between Q2 2025 and
  Q2 2026 (INFERENCE, E589).
- **Visibility.** It is a live **guidance-credibility case**, the H-f theme, and the Q3 results on 9 Nov 2026
  will show whether the H2 ramp is on track.

**Bulyanhulu (84%).**
- Underground; grade 4.9–6.0 g/t; recovery 92–97%; 33–45 koz a quarter; AISC $1,243–1,922/oz.
- A stable operation with a small recorded variance. Its value is as a second site under the same Twiga data
  agreement.

**Kibali (45%, Barrick operator).** This is the richest public reconciliation record in the corpus (C3 §2.3, K p222):
- open-pit GC call grade was 21–27% below the raw block model in 2023–25, and called ounces were below model
  metal in every one of ten years;
- 2025 mill check-out exceeded check-in by 2.6% ("unaccounted gains");
- monthly 2025 MCF ranged −8% to +14% in ounces against a ±5% tolerance.

It is a strong technical case, but it is a JV with AngloGold Ashanti and SOKIMO, in the DRC.

**Pueblo Viejo and NGM.** Both have recovery-led stories:
- PV's 2025 CIL recovery suffered from "higher than planned copper and preg-robbing ores" (AIF p.30);
- NGM has its preg-robbing caps (plan §17.3).

Both are excluded on perimeter grounds: Newmont JV, the IPO vehicle, and Avathon.

---

## 5. Criteria, the recovery question, and the recommendation

### 5.1 Criteria table (scores 1–5, 5 = best for the M2 pilot; weights are judgment)

| Criterion (weight) | Lumwana | North Mara + Bulyanhulu | Kibali | Pueblo Viejo | NGM |
|---|---|---|---|---|---|
| Ownership and data-access friction (25%) | **5**: 100%, no partner, RemainCo | 3: 84/16 Twiga; GoT board seats; PDPA permits | 2: AngloGold 45%, SOKIMO 10% | 1: Newmont 40%; IPO vehicle | 1: Newmont 38.5%; 2026 default notice |
| Recorded variance Touchstone/Crucible can explain (20%) | 4: persistent grade-adjusted recovery gap of $30–57M/yr; grade reconciliation reported good | 3: large grade/AISC swing, mostly planned sequence plus UG delay | **5**: GC below model ten years running; check-out vs check-in 2.6% | 4: CIL recovery, Cu and preg-rob | 4 |
| Data maturity, public evidence (15%) | 3: FMS/GPS, real-time drill data, RC GC; historian unknown | 2: little public evidence | 4: detailed MCF/F1 tables published | 4 | 5: new FMS, autonomy |
| Stationarity in the pilot window (10%) | 2: tie-ins, relines, commissioning from H2 2027 | 3: Gokona ramp, second crushing line | 4 | 2: expansion, TSF | 2: autonomy rollout, vend-in, IPO |
| Executive visibility and pay linkage (15%) | **5**: only 100% mine; $2bn copper flagship; copper AISC 6.25% of bonus; H1 AISC above guidance | 3: gold AISC 18.75%, but NM is about 7% of gold; guidance gap | 3 | 4 (NA Barrick) | 5 (NA Barrick) |
| Residency and regulatory (5%) | 4: Zambia DPA s.70 covers personal data only | 3: PDPA permits; state-dispute history | 2: DRC | 4 | 5 |
| Avathon and political perimeter, F1 (10%) | **5**: outside NA | 5 | 5 | 1 | 1 |
| **Weighted score** | **4.15** | **3.05** | **3.55** | 2.75 | 3.10 |
| P(site data by month 7), §6.4 | **≈0.30** | ≈0.15–0.20 | ≈0.10–0.15 | <0.10 | <0.10 |

On the weighted score, Kibali ranks above North Mara on technical value. On access probability, North Mara ranks
above Kibali. Their P × value is within noise, so the tie-break is whichever consent arrives first (§5.3).

### 5.2 Does recovery-led variance help or hurt the pilot?

The claim to test: *FrankenMining's metallurgical accounting and recovery-card calibration make recovery
variance a stronger demonstration than grade variance.*

| Test | Verdict | Evidence |
|---|---|---|
| T1 Materiality: above the plan's ≥1% contained-metal bar (GATE-TOUCH-04) | **Supported** | 2.4–4.0% of output a year, 2023–H1 2026 (E566, E567) |
| T2 Persistence: not a 2023–24 transient | **Supported** | 2025–26 quarters remain 1.7–2.7 points below the 2013–22 relation. The headline recovery "recovered" because grade rose (§3.3) |
| T3 Forward decision value | **Supported, and stronger than any grade story at Lumwana** | The Super Pit assumes 92.7% fresh recovery in a similar-flowsheet parallel plant, against 91.15% (2021–25, fresh plus transitional). One point at 240 ktpa ≈ 2.6 kt Cu/yr ≈ $23–31M/yr (E570–E572) |
| T4 Measurement anchors | **Supported** | Concentrate shipments and smelter settlements are high-precision check-out measurements, and the site already balances metal monthly (C1 E127). Sampler bias vs real loss is a classic identifiability test (FMINE-REF-104 when not identifiable) |
| T5 Novelty: "new and plausible" to the site QP or metallurgist | **Not supported / unknown** | Barrick calls recoveries "reasonable" and attributes them to the mine plan (E569, E573). The plant metallurgist almost certainly knows that transitional ore and downtime hurt recovery. A finding counts only as a *quantified attribution* (ore type vs sampling/assay vs throughput/downtime/power), or as a sampler or assay bias the site has not measured |
| T6 Data timing | **Partly hurts** | Monthly metallurgical balance and sampler records are in the GATE-DATA-03 set (month 7). Shift- or day-level attribution needs the historian/LIMS (GATE-DATA-04, month 9 of the engagement) |
| T7 Stationarity | **Hurts** | Eight plant events in ten quarters (§3.5) and new-plant commissioning from H2 2027 |
| T8 Is grade an alternative at Lumwana? | **No** | The site reports its grade reconciliation as confirming the model (E130). If the pilot needed a grade finding, Lumwana would be the wrong site. Recovery is not merely the stronger mechanism there; it is the only one on the public record |

**Verdict.** Recovery-led variance **helps**, provided the pilot is scoped to it:
1. **Pre-register the deliverable** as the attribution of the grade-normalised recovery gap, plus a recovery card
   with a validity box, both scored against the FS's 92.7%. Do not pre-register "recovery is low".
2. Make the minimal GATE-DATA-03 set recovery-capable:
   - monthly metallurgical balance;
   - head, tail and concentrate sampler and assay records;
   - concentrate shipment and settlement assays;
   - fresh/transitional tags on feed;
   - the monthly reconciliation pack.
3. Pre-register the forward quarters for the **existing circuit only**.
4. Report a calibrated null if the attribution lands entirely on known causes (R-f2).

It **hurts** if the pilot is judged on site novelty alone, with no historian data, or if its quarters straddle
commissioning.

### 5.3 Recommendation and decision rule

- **Primary: Lumwana**, scoped as in §5.2.
  - *Historical window:* 2024Q1–2027Q1; take the most recent 8–12 quarters.
  - *Segment boundaries:* the events in §3.5.
  - *Forward quarters:* Q3–Q4 2027, existing circuit only. New-plant tonnage is a separate circuit with
    `Estimated` cards.
- **Alternate 1: North Mara with Bulyanhulu**, under one Twiga data-governance paper.
  - *Mechanism:* plan-vs-actual on grade and ore delivery (Gokona; underground haulage) and a guidance-assurance
    card for the H2 2026 ramp.
  - *Condition:* explicit Twiga board sign-off.
- **Alternate 2: Kibali**, if AngloGold Ashanti consent is available.
  - *Mechanism:* grade-led model vs GC reconciliation and check-in/check-out closure.
  - It extends GATE-TOUCH-06's public Kibali known answers directly.
- **Decision rule (program months from Nov 2026):**
  1. **Month 1:** the executive sponsor is asked to name a Lumwana site sponsor (GM), metallurgist and QP, and to
     accept the recovery-capable data list.
  2. **Month 2:** if all of these are named and the list is accepted, the pilot is Lumwana. Stop.
  3. **Month 3:** if there is no Lumwana sponsor, open North Mara (Twiga paper) and Kibali (AngloGold consent) in
     parallel. Take the first to reach GATE-DATA-01. If both reach it, prefer Kibali for its technical value.
  4. **Month 4 (GATE-DATA-01):** if no site has a signed agreement, switch to M2′.
  5. **Standing rule:** no pilot quarter mixes new-plant commissioning tonnage with existing-circuit tonnage.

---

## 6. P(site data by month 7): evidence and decomposition

**S1 definition** (plan §34.4): GATE-DATA-03 passed by month 7 (about May–June 2027). That means 8–12 historical
quarters of reconciliation inputs imported with explained rejections.

### 6.1 Barrick-specific analogues: how fast has Barrick given operating data to outside technology partners?

| Analogue | First engagement → data/operation | Conditions | Source |
|---|---|---|---|
| **Avathon** (AI platform, NA Barrick) | "began collaborating in December" (2025) → software "operating in select U.S. mining sites since the second quarter" (2026) = **about 4–6 months**. Full Nevada deployment expected within 1–2 more quarters; DR next. Public announcement 23–25 Sep 2026 (about 9–10 months); five-year term reported | The Chair is Avathon's lead director (R6); NA perimeter; enterprise vendor | E590, E591 (FACT[secondary], unnamed sources) |
| **Komatsu FrontRunner** (OEM, NGM) | Partnership launched 31 Jul 2025 → 23 Cortez trucks retrofitted and new FMS in place by May 2026 = **about 10 months** to operational systems. Carlin autonomy is planned for Q1 2027 | OEM with incumbent hardware; JV operator; approved capital budgets | E592, E593 |
| **Cisco** (2016, group) | "interactive data rooms will be set up with real-time data"; flagship digital operation at Cortez | Chair-sponsored (Thornton); NA again. No timeline published | [Fortune, 12 Sep 2016](https://fortune.com/2016/09/12/barrick-gold-partners-with-cisco/) (FACT[secondary]) |

**Readings.**
- Barrick's digital flagships have always been in Nevada (2016, 2025, 2026).
- The fastest data-to-vendor path on record (about 4–6 months) had Chair-level proximity.
- FrankenMining needs about 7 months **to a complete 8–12-quarter historical import**, at an African site, as an
  unknown vendor. That is at the fast end of the Barrick distribution, not the middle.

### 6.2 Barrick's data and AI posture, 2026

- **Positive:**
  - Chair's letter: "We will continue to use technology, particularly AI, to make us safer, more efficient, and
    more responsible" (2026 Circular p.II, E594).
  - Audit & Risk Committee oversight of "risks and opportunities relating to artificial intelligence" (Circular
    p.16).
  - Mark Hill on Avathon: "AI has the potential to change how a mining company safely operates at its core"
    (E590).
- **Cautionary:**
  - The AIF says "Barrick and its third party vendors and service providers have experienced … cybersecurity
    incidents" and that digital initiatives may not meet "capital allocation objectives" (AIF p.127–128, E595).
    Expect a real security review.
- **Structural:**
  - Hill becomes CEO of North American Barrick. As of the Q2 2026 MD&A the leader of the remaining Barrick, which
    owns Lumwana, has not been disclosed (E563; R6 §9).
  - The IPO (target end-2026) loads group legal, finance and IT through months 0–3 of the program.
  - Barrick's model delegates authority to regions and sites (R6 §1.1). A Lumwana GM can sponsor, but needs
    group IT security sign-off.

### 6.3 Reference class (secondary and weak; no mining-specific "time to data access" dataset was found)

- Bain (13 Aug 2020): "only 25% of companies are getting beyond pilots to broader adoption" in energy and natural
  resources (E598).
- BCG (2022, cross-industry): about 30% scale digital solutions effectively; 70% stall at pilots or silos
  ([BCG PDF](https://services.google.com/fh/files/misc/bcg_the_keys_to_scaling_digital_value_global_2022.pdf),
  FACT[secondary], not quoted).
- McKinsey (2019, Asian metals and mining): about 85% of aware companies "haven't moved past initial pilot
  programs" (trade reprint; FACT[secondary], dated).
- Protegrity survey (2025–26, cross-industry, vendor-commissioned): 82.9% of organisations had AI projects
  delayed by security or compliance reviews
  ([SecurityBrief](https://securitybrief.com.au/story/enterprise-ai-deployments-slowed-by-security-reviews),
  FACT[secondary], not quoted).
- EY (16 Oct 2025, 500 executives): digital transformation re-entered the mining top-10 risks at #8
  ([Mining Weekly](https://www.miningweekly.com/article/operational-complexity-overtakes-external-pressures-as-minings-top-risk-for-2026-ey-2025-10-16),
  FACT[secondary]).
- Deloitte *Tracking the trends 2026* (27 Jan 2026) treats operational data and AI as trends 5–6. It gives no
  timing data.
- **Checked nulls:** no CSIRO, Austmine, METS Ignited, Unearthed, McKinsey or Deloitte source gave a measured
  first-engagement-to-data-access interval for mining pilots. METS Ignited named "access to decision makers" as a
  leading barrier (Mining News, undated).

These base rates concern *scaling* after a pilot. They bound the problem only loosely, and they are not used
numerically below.

### 6.4 Decomposition (Lumwana; conditional factors; judgment ranges anchored on §6.1–6.3)

| Factor | Central | Range | Evidence for | Evidence against |
|---|---|---|---|---|
| A. P(executive sponsor in RemainCo plus a named Lumwana site sponsor, metallurgist and QP, with the data list accepted, by month 4) | **0.60** | 0.45–0.75 | Public AI posture (E594, E590); site-level authority; Lumwana is the copper flagship; a $30–57M/yr grade-adjusted gap and H1 AISC above guidance give the sponsor a reason (E567, E586) | RemainCo leadership undisclosed and IPO churn (E563); site bandwidth consumed by the $2bn build (E577); Barrick calls recovery "reasonable" (E569), so the pain is not felt; F1 Avathon gravity; unknown vendor |
| B. P(data agreement, NDA, insider list and blackout in force by month 4–5, given A) | **0.80** | 0.65–0.90 | 100% owned: no JV consent (E551, E552). On-prem with no egress: Zambia's localisation covers personal data only (E596) | MNPI: dual-listed issuer; legal-team turnover in 2026 (R6); IPO workload to end-2026 |
| C. P(security review passed and 8–12 quarters imported with explained rejections by month 7, given A and B) | **0.60** | 0.40–0.75 | FMS/GPS, RC GC, monthly reconciliation exists (§3.7, E130); the single offline binary eases review; the fastest analogue, about 4–6 months (E591) | Vendor cyber history and caution (E595); 82.9% security-delay base rate (secondary); group IT stretched by the IPO carve-out; site IT shared with the build; historical "as-known" model versions are hard to reconstruct; F2 pre-mortem (9-month review) |
| **P(S1) = A × B × C** | **≈0.29 → 0.30** | factor extremes 0.12–0.51; **plausible 0.15–0.45** | | |

**Other sites**, with the same A and C structure and B adjusted for consent (INFERENCE):

| Site | A | B | C | Product |
|---|---|---|---|---|
| North Mara / Bulyanhulu | 0.6 | 0.55 (Twiga board, PDPA permits, state history) | 0.5 | ≈0.17 |
| Kibali | 0.5 | 0.45 (AngloGold, SOKIMO) | 0.6 | ≈0.14 |
| NGM | 0.4 (Avathon) | 0.35 (Newmont) | 0.6 | ≈0.08 |

**Robustness.** P(S1) stays below 0.5 unless all three factors sit at their upper bounds together.

**GB-38 re-substitution** at p = 0.3, with the plan's own V, T and λ:

| Option | EV at p = 0.3 |
|---|---|
| **H-f** | 0.3·8 + 0.7·4 − 0.5 = **4.7** |
| H-a, H-e | 0.3·4 + 0.7·3 − 1 = 2.3 |
| H-d | 0.3·3 + 0.7·2 − 0 = 2.3 |
| H-b | 0.3·3 + 0.7·1 − 1 = 0.6 |
| H-c | 0.3·2 + 0.7·0 − 2 = −1.4 |

- The ranking is unchanged. What changes is resourcing: with p ≈ 0.3, **M2′ is the modal path (≈0.7)**. Its
  preparation (the public metric engine, the PQ-7 cards, the guidance-assurance cards for North Mara and
  Lumwana) belongs in M1, not in reserve.
- The flip condition V_f(S2) < 2.5 now matters more and should be tested at the M0 kill gate.

### 6.5 Early signals, months 1–4, and how they move p

Likelihood ratios are judgment, applied to the odds (0.3 corresponds to odds of 0.43).

| Signal (when) | If observed | If absent or late |
|---|---|---|
| RemainCo CEO/COO named, and the executive sponsor confirms remit over Lumwana (month 1–2; the IPO closes by end-2026) | ×1.5 | IPO slips into 2027: ×0.7 |
| Lumwana GM named as site sponsor, plus metallurgist and QP (month 2) | ×2 | None by month 3: ×0.5 |
| Barrick IT issues its vendor security questionnaire within 30 days of the request (month 1–2) | ×1.5 | Review not started by month 3: ×0.4 |
| NDA in ≤6 weeks; data-agreement draft exchanged by month 3 | ×1.5 | No draft at GATE-DATA-01 (month 4): p ≤ 0.1, go to M2′ |
| One sample month of the reconciliation pack or met balance delivered (month 3–4) | ×3 | — |
| Avathon scope announced beyond North America (any month) | ×0.6 | — |
| Q3 2026 results (9 Nov 2026) or Q4 (Feb 2027) cite Lumwana recovery as an issue | ×1.2 for factor A | — |

**Worked example.**
- Sponsor named by month 2 (×2), questionnaire issued (×1.5) and a sample month delivered (×3): odds 3.9, p ≈ 0.8.
- No site sponsor by month 3: p ≈ 0.18.

---

## 7. Proposed replacement text (ready to paste)

### 7.1 §37.3 M2, replacing the "*Site preference:*" bullet and its sub-bullets

```markdown
- *Site preference:* **Lumwana** (Zambia; 100% through Lumwana Mining Company, E551), Barrick's only wholly
  owned operating mine after the Hemlo and Tongon sales (E552–E554). It is outside the NGM/Newmont JV and outside
  the North American Barrick perimeter where Avathon is deploying (E563, E590–E591). The finding mechanism is
  **recovery-led** (W4 §5.2):
  - Headline recovery went from 89.4–89.8% (2023–24) to 91–92% (2025–H1 2026), but mostly because head grade
    rose to 0.57–0.67% Cu. Against the 2013–22 grade–recovery relation, every 2025–26 quarter is still 1.7–2.7
    points short: about 2.4–4.0% of output a year, roughly $30–57M/yr at $4.00–5.50/lb (INFERENCE, E566–E567).
  - Barrick calls 2021–25 recovery (91.15%, fresh plus transitional ore) "reasonable" and attributes changes to
    grade and the mine plan; no mechanism is published (E569, E573). So the pre-registered deliverable is an
    **attribution** of the grade-normalised gap among measurement (head/tail sampling, assay), ore type
    (fresh vs transitional) and operations (throughput, downtime, power), not "recovery is low".
  - The same recovery card is scored against the feasibility study's 92.7% fresh recovery for the 52 Mtpa
    parallel plant, which uses a similar flowsheet. One point at 240 ktpa ≈ 2.6 kt Cu/yr (E570–E572).
  - The site reports its grade reconciliation as confirming the model (E130). A grade null is the expected
    result there and is reported as such (R-f2).
  - Non-stationarity: the window contains a prolonged SAG shutdown (Q1 2025), crusher downtime (Q3 2025), a
    reline (Feb 2026), the pebble-crusher tie-in (Q2 2026) and grid-power instability (E580–E584). Each is a
    segment boundary. New-plant commissioning runs from H2 2027, with first copper at end-Q1 2028 (E131, E578).
    Historical quarters are 2024Q1–2027Q1. The two forward quarters are pre-registered for the **existing
    circuit only**; new-plant tonnage is a separate circuit with `Estimated` cards.
  - The recovery-capable GATE-DATA-03 set is:
    - monthly metallurgical balance;
    - head, tail and concentrate sampler and assay records;
    - concentrate shipment and settlement assays;
    - fresh/transitional tags on feed;
    - the monthly reconciliation pack.
    Historian/LIMS (GATE-DATA-04) refines the finding but does not enable it.
- *Ranked alternates* (used only if Lumwana has no named site sponsor by month 3):
  1. **North Mara with Bulyanhulu** under one Twiga data-governance paper. The Government of Tanzania holds a 16%
     free carry and board seats (E135, E562). Mechanism: plan vs actual on grade and ore delivery (H1 2026 was
     68 koz attributable against 200–230 koz FY guidance; Gokona pre-strip and delayed underground ore; E587,
     E588). Its recovery fall to 85% is grade-driven (E589), not a separate finding.
  2. **Kibali** (45%; Barrick operates; AngloGold Ashanti 45%, SOKIMO 10%; E560), if AngloGold consent is
     obtainable. It has the richest public reconciliation record in the corpus (C3 §2.3: GC call below model
     metal in every year 2016–25; 2025 check-out 2.6% above check-in).
  - Not candidates:
    - NGM and Pueblo Viejo: Newmont JV, the IPO vehicle, and Avathon (E556–E558, E591);
    - Fourmile: moving into NGM (E139);
    - Loulo-Gounkoto, Porgera, Reko Diq (under review), and the non-operated JVs (E559, E561).
- *Site decision rule.*
  - Month 1: the executive sponsor is asked to name a Lumwana site sponsor, metallurgist and QP, and to accept
    the data list.
  - Month 2: if all are named, the pilot is Lumwana.
  - Month 3: otherwise, open North Mara (Twiga paper) and Kibali (AngloGold consent) in parallel and take the
    first to reach GATE-DATA-01, preferring Kibali if both do.
  - Month 4: no signed agreement means M2′.
```

### 7.2 §34.4, replacing the "**The probability that matters most.**" paragraph

```markdown
**The probability that matters most.** P(site data by month 7) was elicited at 0.5 (B5 §5.3). W4 replaces it
with an evidence-based **0.3 for Lumwana (plausible range 0.15–0.45)** (INFERENCE, E599). The decomposition is:
- P(executive and Lumwana site sponsor committed, data list accepted, by month 4) ≈ 0.6 (0.45–0.75);
- × P(agreement, insider list and blackout in force by month 4–5 | sponsor) ≈ 0.8 (0.65–0.9);
- × P(security review passed and 8–12 quarters imported by month 7 | both) ≈ 0.6 (0.4–0.75).

The fastest Barrick precedent, Avathon, went from first collaboration (December 2025) to software running at
selected U.S. sites in Q2 2026, with a Chair-level link and inside the North American perimeter (FACT[secondary],
E591). FrankenMining must match that pace without either advantage.

The alternates are lower: North Mara/Bulyanhulu about 0.15–0.2 (Twiga), Kibali about 0.1–0.15 (AngloGold
consent), NGM and Pueblo Viejo below 0.1 (Newmont, Avathon).

The wedge choice is unchanged. At p = 0.3, H-f's EV is 4.7 against 2.3 for the next options (GB-38, §37.2). What
changes is planning weight: M2′ is the modal path (≈0.7) and is resourced from M1, not held in reserve.

Update p only on the observable signals in W4 §6.5:
- raise it for a named Lumwana site sponsor by month 2 (×2 odds), a security questionnaire issued within 30 days
  (×1.5), or one sample month of the reconciliation pack by month 4 (×3);
- lower it if there is no site sponsor by month 3 (×0.5);
- no draft agreement at GATE-DATA-01 sets p ≤ 0.1 and triggers M2′.
```

---

## 8. Proposed evidence rows (E551–E599)

Locator file ids (sha256 first 16):
- Mine Stats: `Q1_2024` e2e1243ec0549f4c, `Q2_2024` 1a73d6d7d4658f92, `Q3_2024` d8786ce96f7129bf,
  `Q4_2024` f5e7db7c4657927f, `Q1_2025` ec5158f8b4624643, `Q2_2025` 599864f9958b5a10, `Q3_2025` b55b857766195fe9,
  `Q4_2025` 94f6692acfa5f133, `Q1_2026` ddef08dd544f0df5, `Q2_2026` cf0d3f4fea9c96f9 (all under
  `corpus_of_input_docs/downloaded/mine_stats/` except the two 2026 corpus files).
- MD&A PDFs (same folder): Q1-24 09a50ed4ce1eb713, Q2-24 3ddfc7dcbc1721f7, Q3-24 3ca53761402bf20e,
  Q4-24 43dcfca7aadc48b4, Q3-25 076819436e309d82, Q4-25 63b3af0b98436900, Q1-26 2bbc4b75b00b440a,
  Q2-26 08ee992f8a98886c.
- `AIF` = Barrick 2025 Annual Information Form, 27 Feb 2026,
  https://s25.q4cdn.com/322814910/files/doc_downloads/agm/2026/Barrick_AIF_2025.pdf (d8bc75c700aa8892). Page
  numbers are the printed ones.

```
E551 | Lumwana is wholly owned by Barrick through LMC | AIF p.101 | "Lumwana is owned by LMC, a Zambian registered exploration and mining company and wholly owned subsidiary of Barrick." | FACT
E552 | Lumwana is Barrick's only 100%-owned operating mine in Oct 2026 | Q2_2026:QTD!A5–A28 labels (every other site prints a %); E553, E554 | "Lumwana" (only copper/gold site label without a percentage after Hemlo's sale) | INFERENCE
E553 | Hemlo sold to Carcetti Capital Corp; closed 26 Nov 2025 | Q2_2026:QTD!A32 | "we reached an agreement to sell the Hemlo gold mine to Carcetti Capital Corp. … The transaction closed on November 26, 2025." | FACT
E554 | Tongon sold to the Atlantic Group; closed 1 Dec 2025 | Q2_2026:QTD!A35 | "sell our interest in the Tongon gold mine … to the Atlantic Group … The transaction closed on December 1, 2025." | FACT
E555 | Porgera: Barrick 24.5%; PNG stakeholders 51% | Q2_2026:QTD!A14; AIF p.117 | "ultimately increased the ownership interest of PNG stakeholders in Porgera to 51%" | FACT
E556 | NGM governance: 3 Barrick / 2 Newmont managers; equal-representation technical committee | AIF p.139 | "five members … three of which were appointed by Barrick and two of which were appointed by Newmont" | FACT
E557 | Newmont's Feb 2026 NGM default notice alleged misuse of assets and operational/accounting issues | AIF p.126 | "Newmont sent Barrick a notice of default … relating to an alleged misuse of assets as well as operational and accounting issues" | FACT
E558 | NGM disputes settled; Fourmile, Mike, Fiberline vended in; Newmont consents to the IPO | Q2 2026 release, 10 Aug 2026, https://www.sec.gov/Archives/edgar/data/756894/000119312526343857/d67611dex991.htm | "The agreement resolves all outstanding disputes related to NGM." | FACT
E559 | At non-operated JVs Barrick depends on the operator for information | AIF p.126 | "will rely on the operators for accurate information about the properties" | FACT
E560 | Kibali: Barrick 45% and operator; partners AngloGold Ashanti and SOKIMO | AIF p.84 | "Barrick is the operator of Kibali." | FACT
E561 | Reko Diq: 50% Barrick, 50% Pakistani stakeholders; Barrick operates; under review since 5 Feb 2026 | AIF p.92–93 | "on February 5, 2026, Barrick announced that it is undertaking a review of all aspects of the Reko Diq Project" | FACT
E562 | GoT appointed its own members to the Twiga board (2020) | The Citizen, 2020, https://www.thecitizen.co.tz/news/1840340-5501040-auebx4/index.html | "The two will represent the interests of the government, which owns 16 percent free carry shares" | FACT[secondary]
E563 | Mark Hill will be CEO of North American Barrick (NGM's Carlin/Cortez/TR plus PV); IPO by end-2026 | Q2 2026 MD&A p.7 (Q2-26 PDF) | "Mark Hill will be the CEO of North American Barrick." | FACT
E564 | Lumwana quarterly recovery 2023Q1–2026Q2: 87, 93, 91, 87, 88, 85, 91, 93, 91, 92, 92, 91, 91, 91% | Minestats4!D14 (and E14 for 2023) in Q1_2024…Q2_2026 | "Recovery rate (percent)" | FACT
E565 | 2024 Mine Stats label copper production "millions of pounds" while the footnote says tonnes; values are kt | Q1_2024:Minestats4!A15, A29 | "Starting in 2024, we have presented our copper production and sales quantities in tonnes rather than pounds" | FACT (cells) / INFERENCE (label error)
E566 | The grade-adjusted recovery gap persists: 1.7–2.7 pts below the 2013–22 relation in every 2025–26 quarter | LUM p.39 (2013–22) + Minestats4!D9/D13/D14/D15; W4 §3.3 | "2024 25,782,749 0.53 136,632 122,723 89.82" | INFERENCE (tails models M-a/M-b)
E567 | Grade-adjusted shortfall: 3.4–4.0% of output (2023–24), 2.4–3.1% (2025), 2.9–3.4% (H1 2026); $30–57M/yr at $4.00–5.50/lb | W4 §3.3 | "Copper Produced" (LUM p.39 column) | INFERENCE
E568 | Q2 2024 MD&A claims higher Lumwana recoveries Q2 vs Q1 2024; the cells show 88% → 85% (metal balance 89.1% → 85.2%) | Q2-24 MD&A p.47 (also p.7); Q1_2024:Minestats4!D14; Q2_2024:Minestats4!D14 | "3 thousand tonnes higher compared to the prior quarter driven by higher grades and recoveries at Lumwana" | INFERENCE (discrepancy; PLAUSIBLE)
E569 | Barrick: 2021–25 recovery averaged 91.15% across fresh and transitional ore, described as reasonable | AIF p.102 | "consistently achieve reasonable recoveries, with an average of 91.15%, across both fresh ore and transitional material" | FACT
E570 | Super Pit LOM basis: expected average fresh recovery 92.7% | AIF p.102 | "The expected average fresh recovery is 92.7% copper, based on the current life-of-mine plan and test work completed to date." | FACT
E571 | Barrick expects no recovery difference between current and Super Pit ore; the new plant uses a similar flowsheet | AIF p.102 | "No material difference is expected in recovery and concentrate grades between currently processed mineralization and expected mineralization" | FACT
E572 | One recovery point at 240 ktpa ≈ 2.6 kt Cu/yr ≈ $23–31M/yr | AIF p.102 (240 ktpa, 92.7%); arithmetic | "life-of-mine average of 240 kilotonnes per annum" | INFERENCE
E573 | Barrick attributes Lumwana recovery changes to the mine plan | Q2-24 MD&A p.37 | "mainly due to lower grades processed and recoveries, in line with the mine plan" | FACT
E574 | Lumwana flowsheet and expansion: two primary crushers, SAG-ball, flotation; parallel plant 27 → 52 Mtpa (54 peak) | AIF p.100, p.102 | "through the construction of a parallel processing plant. The new plant will use a similar flow sheet" | FACT
E575 | Lumwana grade control uses RC drilling at 12.5 × 25 m | AIF p.105 | "RC drilling is used for grade control. … RC drilling is the closest spaced at 12.5 meters by 25 meters." | FACT
E576 | Super Pit feasibility study completed in 2024; TR effective 31 Dec 2024, issued 19 Feb 2025 | AIF p.100, p.106 | "In 2024, Barrick completed a feasibility study for the Super Pit Expansion Project" | FACT
E577 | Super Pit spend $610M at 30 Jun 2026; $2bn per the approved FS; 2026 at the lower end of $750–850M | Q2-26 MD&A p.33 | "the total spend on the expansion project was $610 million (including $120 million in Q2 2026)" | FACT
E578 | Super Pit on schedule for first copper at end-Q1 2028; most long-lead equipment on site | Q2 2026 release, 10 Aug 2026 (SEC URL as E558) | "The Lumwana Super Pit Expansion remains on schedule, with first copper production targeted for the end of Q1 2028." | FACT
E579 | $254M spent by 31 Dec 2025; about $0.75–0.85bn expected in 2026; total about $2bn over 2025–2028 | AIF p.106 | "As at December 31, 2025, the total spend for the Super Pit Expansion Project was $254 million." | FACT
E580 | Q2 2026 throughput cut by the pebble-crusher tie-in on the existing plant | Q2-26 MD&A p.28 | "lower throughput due to down time for the pebble crusher tie in on the existing plant, as well as lower grades processed" | FACT
E581 | Q1 2025 had a prolonged SAG mill shutdown | Q2-26 MD&A p.29 | "primarily due to the prolonged SAG mill shutdown experienced in Q1 2025" | FACT
E582 | Q3 2025: prolonged September shut and crusher downtime | Q3-25 MD&A p.29 | "lower than planned throughput driven by a prolonged September shut and crusher down time" | FACT
E583 | Q1 2026: planned reline in Feb 2026; premature component failures being addressed with OEMs | Q1-26 MD&A p.23 | "higher maintenance costs from premature component failures which is being addressed with OEMs" | FACT
E584 | 2024 throughput hit by Zambian grid power instability | Q3-24 MD&A p.29 | "lower grades processed and lower throughput due to grid power instability" | FACT
E585 | Lumwana runs an FMS with high-precision GPS and real-time drill data | LUM p.10, p.11 (corpus deck, 25 Sep 2025) | "Leveraging technology to improve efficiencies through Fleet Management System including High Precision GPS Implementation" | FACT
E586 | Lumwana H1 2026 AISC $3.83/lb vs 2026 guidance $3.40–3.75/lb | Q2_2026:YTD!G27; AIF p.31 | "All-in sustaining costs are expected to be in the range of $3.40 to $3.75 per pound" | FACT (cells) / INFERENCE (comparison)
E587 | North Mara H1 2026: 68 koz attributable at AISC $2,283/oz vs FY guidance 200–230 koz at $1,520–1,680; H2 needs 1.9–2.4× the H1 rate | Q2_2026:YTD!C19, G19; AIF p.30 | "equity share of 2026 gold production to be in the range of 200 - 230 thousand ounces" | INFERENCE
E588 | North Mara's 2026 grade fall: Gokona pre-stripping plus delayed underground ore | Q2-26 MD&A p.24 | "waste material movement in the Gokona pit while addressing delayed delivery of ore from the underground operations" | FACT
E589 | North Mara's recovery fall to 85% is grade-driven: implied tails 0.38 → 0.27 g/t (Q2 2025 → Q2 2026) | Q2_2026:Minestats3!H18, I18, H19, I19 (attributable) | "Recovery rate (percent)" | INFERENCE
E590 | Barrick's North American business selected Avathon as strategic technology partner (Sep 2026) | Global Mining Review, 25 Sep 2026, https://www.globalminingreview.com/mining/25092026/avathon-selected-to-power-an-ai-native-mining-operating-model-for-barricks-north-american-business/ | "AI has the potential to change how a mining company safely operates at its core" (Mark Hill) | FACT[secondary] (republished release)
E591 | Avathon timeline: collaboration from December (2025); software at select U.S. sites since Q2 (2026); Nevada full deployment in 1–2 quarters; DR next | Deep News via Tiger Brokers, 23 Sep 2026, https://www.itiger.com/news/1177296302 | "the software already operating in select U.S. mining sites since the second quarter" | FACT[secondary] (unnamed sources)
E592 | NGM–Komatsu FrontRunner AHS partnership launched 31 Jul 2025 | NGM release, https://s25.q4cdn.com/322814910/files/doc_news/2025/07/NGM-and-Komatsu-Launch-First-of-Its-Kind-Autonomous-Haulage-Partnership_Final.pdf | "Nevada Gold Mines (NGM) and Komatsu have officially launched a pioneering partnership" | FACT
E593 | Cortez: 23 trucks retrofitted May 2026; new FMS for the primary fleet completed | Q2-26 MD&A p.32 | "The transition to the new Fleet Management System for the primary production fleet has been completed." | FACT
E594 | Barrick Chair commits to AI in the 2026 Circular | 2026 Circular p.II, https://www.sec.gov/Archives/edgar/data/756894/000119312526148005/d858698dex991.htm | "We will continue to use technology, particularly AI, to make us safer, more efficient, and more responsible." | FACT
E595 | Barrick and its vendors have had cyber incidents; digital initiatives carry no assured value | AIF p.127–128 | "Barrick and its third party vendors and service providers have experienced … cybersecurity incidents and cybersecurity breaches" | FACT
E596 | Zambia DPA 2021 s.70 requires personal data to be stored and processed in Zambia, with exceptions | Afriwise, 8 Jun 2022, https://www.afriwise.com/blog/data-localisation-requirements-under-the-data-protection-act | "on servers or data centres that are located in Zambia" | FACT[secondary]
E597 | Tanzania PDPA 2022 (in force 1 May 2023): cross-border personal-data transfers need a Commission permit | FPF overview, https://fpf.org/blog/tanzanias-personal-information-protection-act-overview-key-takeaways-and-context/ (via search extract; page not fetched) | "the transfer will be subject to obtaining a permit from the Personal Data Protection Commission" | FACT[secondary]
E598 | Only 25% of energy and natural-resources companies get beyond digital pilots | Bain, 13 Aug 2020, https://www.bain.com/insights/scaling-digital-operations-in-energy-and-natural-resources/ | "only 25% of companies are getting beyond pilots to broader adoption" | FACT[secondary]
E599 | P(site data by month 7) for Lumwana ≈ 0.3 (0.15–0.45) = sponsor 0.6 × legal 0.8 × IT 0.6; NM ≈ 0.17, Kibali ≈ 0.14, NGM ≈ 0.08 | W4 §6.4 | "S1: site data by month 7" (plan §37.2 GB-38) | INFERENCE
```

---

## 9. Checked nulls and open items

- **Super Pit FID.** No standalone FID release was found. "Approved feasibility study" (Q2 2026 MD&A) and the
  Q4 2024 long-lead down payments are the decision evidence. The exact board-approval date is OPEN.
- **JV data-sharing clauses.** No public text was found for NGM, Kibali, Twiga or PV. The revised NGM agreement
  (6-K, 17 Aug 2026) was not read in this pass. It is the closer for NGM.
- **Lumwana metallurgical mechanism.** No public 2024–26 document explains the recovery shortfall (transitional
  ore, grind, reagents, sampling). The only hints are "transitional material" (AIF) and the event log (§3.5).
- **Lumwana historian and LIMS vendors.** Unknown. Job advertisements cite Hexagon, Modular, MineStar and Wenco
  generically (secondary; not used).
- **RemainCo leadership** after the IPO: undisclosed in the sources read.
- **Mining-specific reference class** for time-to-data in vendor pilots: none found (§6.3).
- **Q3 2026 results** are due 9 Nov 2026. They will update Lumwana recovery (2026Q3), North Mara's H2 ramp, and
  possibly the IPO timing. Re-run §3.3 and §6.5 then.
- **Jabal Sayid partner and operator:** not confirmed in the sources read.
