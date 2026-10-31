# R6 — Industry Landscape: Barrick, Newmont, Mining Economics, Software Stack, and Failure Patterns

**Prepared for:** FrankenMining planning (AI-native, pure-Rust operating system for integrated Au/Cu/Ag/Sn miners)
**Research date / access date for every URL below:** 2026-10-28
**Status of the record:** Barrick's Q3 2026 results are not out yet. They are scheduled for **9 Nov 2026** ([Barrick notice](https://www.barrick.com/English/news/news-details/2026/barrick-to-report-third-quarter-2026-results-on-november-9/default.aspx), VERIFIED-S). The latest Barrick figures here are therefore Q2 2026 (released 10 Aug 2026). Newmont's latest figures are Q3 2026 (released 22 Oct 2026).

## How to read this document

Every material statement carries one of these tags:

| Tag | Meaning |
|---|---|
| **[V-P]** | VERIFIED against a primary source that was read: SEC/SEDAR filing, company release, regulator or standards body, vendor page. URL given. |
| **[V-S]** | VERIFIED only through secondary sources: reputable trade press, or a search snippet that names the source. URL given. Treat as likely, not certain. |
| **[UNV]** | UNVERIFIED. This means my recall, a claim with conflicting sources, or something I could not reach. Do not repeat it to executives without checking it first. |
| **[INF]** | INFERENCE. This is my analysis for FrankenMining and is not a sourced fact. |

**Method and limits:**
- Sections C and B/D were researched in parallel by two sub-agents and folded into this report. I re-checked the most decision-relevant items myself.
- The session's web-search quota (200 searches) ran out partway through. After that I used direct reads of known primary URLs, mostly SEC EDGAR.
- A few planned checks therefore remain [UNV] and are listed in §9.

---

## 0. Executive summary: the 20 facts that matter most

1. **Barrick is now "Barrick Mining Corporation."** Shareholders approved the name on 6 May 2025 with 98.14% in favour. The NYSE ticker changed from GOLD to **B** on 9 May 2025; the TSX ticker is still ABX. [V-S] ([Engineering News](https://www.engineeringnews.co.za/article/barrick-completes-name-change-elects-directors-2025-05-07); [Barrick](https://www.barrick.com/English/news/news-details/2025/barrick-announces-name-change-to-barrick-mining-corporation-and-election-of-directors/default.aspx)). Primary filings now use the new name. [V-P]
2. **Barrick has a new leadership team.**
   - The previous President and CEO left on 28/29 Sep 2025.
   - The Group COO became interim CEO on 29 Sep 2025 and permanent President and CEO on **4 Feb 2026**, and joined the Board.
   - A new CFO took office on 1 Mar 2026.
   - A new Chief Legal, Compliance and Policy Officer and a new Chief Global Affairs Officer were appointed.
   - The Chairman is non-executive.
   - [V-P] (Q2 2026 MD&A; 2026 Circular)
3. **Barrick is separating its North American business.**
   - "**North American Barrick**" will hold Carlin, Cortez and Turquoise Ridge (all within NGM), Pueblo Viejo, and the Fourmile, Mike and Fiberline projects.
   - Barrick plans an IPO of a minority stake: primary listing in New York, secondary in Toronto, **targeted by end-2026**. Barrick keeps a controlling majority.
   - **The Group CEO will be CEO of North American Barrick.** Who will lead the remaining parent has not been disclosed. [V-P]
4. **Barrick and Newmont have settled their NGM disputes (10 Aug 2026).**
   - Fourmile (Barrick) and Mike and Fiberline (Newmont) are contributed to NGM. The combined complex is described as "nearly 100 million oz" of gold.
   - Newmont pays Barrick a **$1.95B** cash top-up, expected Q4 2026.
   - Newmont consented to the IPO.
   - [V-P] (Barrick Q2 2026 release; Newmont Q3 2026 release; 6-K filing the NGM Second A&R LLC Agreement)
5. **The Mali dispute is resolved.**
   - The settlement was announced 24 Nov 2025. Operational control of Loulo-Gounkoto returned on 16 Dec 2025. [V-P] (Q1 2026 MD&A)
   - The reported payment is about **$430M** (244bn CFA) plus 50bn CFA in VAT-credit offsets. [V-S, Bloomberg via [BNN](https://www.bnnbloomberg.ca/business/2025/11/24/barrick-mining-reaches-deal-to-settle-dispute-with-government-in-mali/)]
   - Q2 2026 results include retrospective royalties, penalties and interest under Mali's **2023 Mining Code** for 2024–2025. [V-P]
6. **Reko Diq has been slowed down.**
   - On 5 Feb 2026 Barrick began reviewing "all aspects" of the project after a rise in security incidents.
   - On **2 Apr 2026** it said it would "slow development activity and extend the project review" covering security, capital, financing, scope and timeline.
   - Capitalised spend to date is $1,083M (100% basis).
   - Barrick's tailings page says it "cannot disclose conformance to GISTM in 2026" for the two Reko Diq facilities.
   - [V-P]
7. **Barrick's Tier One definition is published.** The reserve price in it moves with the price deck.
   - **Gold:** reserve at **$1,400/oz** (2026 Circular) or **$1,500/oz** (Q2 2026 release); at least 10-year life; at least **500 koz/yr**; costs in the **lower half of the industry cost curve**; located in a world-class district with potential for organic reserve growth.
   - **Copper:** reserve at $3.00/lb (Circular) or $3.25/lb (Q2 release); at least 5 Mt contained Cu; at least 20-year life; at least **200 ktpa**; lower-half costs.
   - [V-P]
8. **Barrick's executives are paid on these metrics (2025 scorecard).** [V-P] (2026 Circular)
   - **Annual bonus:** 70% company-wide and 30% individual strategic priorities. The company-wide part is:
     - ESG 20%: LTIFR, close-out of incident-investigation actions, **critical control verifications**, and Class 1 environmental incidents, each 5%.
     - Attributable gold-equivalent ounces 25%.
     - Gold AISC 18.75%.
     - Copper AISC 6.25%.
     - Fatalities trigger an extra deduction.
   - **Long-term incentive (PGSU):** relative TSR vs **GDX** 35%; ROCE 15% (long-term range 10–15%); 3-year rolling reserve replacement 15%; strategic execution 15%; ESG and licence to operate 20%.
9. **Newmont's executives are paid on these metrics.** [V-P] (2026 Proxy)
   - **Annual bonus:**
     - **Adjusted cash sustaining cost per GEO, 35%.**
     - **Adjusted free cash flow, 35%.**
     - Safety 20%: Significant Potential Event Frequency Rate 14% and Fatality Risk Management 6%.
     - Water efficiency 5% and culture 5%.
     - The 2025 payout was 155.15% of target.
   - **PSUs (2026 design):** rTSR 60% and ROCE 30%.
10. **Newmont has a new leadership team.**
    - **Natascha Viljoen** became CEO on 1 Jan 2026, succeeding Tom Palmer.
    - From 1 Jul 2026: CFO **Brian Tabolt**, COO **Mark Rodgers**, CTO **David Thornton**.
    - CTO Francois Hardy retired on 30 Jun 2026. He was also the accountable executive for tailings.
    - [V-P]
11. **Newmont's portfolio and results.**
    - The 2026 framing is **Managed Core**:
      - Lihir, Cadia, Tanami, Boddington.
      - Ahafo South and Ahafo North.
      - Merian (75%), Cerro Negro, Yanacocha, Peñasquito.
      - Red Chris (70%), Brucejack.
    - **Non-Managed Core** is NGM at 38.5%, Pueblo Viejo at 40%, and Lundin Gold (Fruta del Norte) at 32%.
    - 2026 guidance is about **5.26 Moz** with by-product AISC of about **$1,520/oz**.
    - Q3 2026 FCF was $1.9B.
    - The non-core divestitures (CC&V, Musselwhite, Éléonore, Porcupine, Akyem, Coffee) are complete and raised about $4.5B after tax.
    - [V-P]
12. **Newmont's capital-allocation order (Feb 2026)**, in priority sequence: [V-P]
    1. Sustaining capital, elevated by tailings work at Cadia and Boddington.
    2. A **$1.1B/yr** fixed cash dividend.
    3. Development capital.
    4. Balance sheet: a **$1B net cash target** (±$2B) and a **$5B minimum cash** floor.
    5. Ratable buybacks.
13. **Both companies now discuss AI publicly.**
    - Barrick's Chairman, in the 2026 Circular: "We will continue to use technology, **particularly AI**, to make us safer, more efficient, and more responsible." The Board held an "Autonomous Mining" education session on 7 Nov 2025. [V-P]
    - Newmont's 10-K says AI "is increasingly leveraged by Newmont for decision-making, mineral extraction optimization, and autonomous operations." Newmont adopted an enterprise-wide **AI Standard in Nov 2025**. [V-P]
14. **An incumbent already holds the "AI-native operating model" position at Barrick North America.**
    - On 25 Sep 2026, **Avathon** said Barrick's North American business had chosen its Autonomy Platform as "strategic technology partner." Avathon describes its product as a "Computational Knowledge Graph" plus AI agents across exploration, planning, safety, production and recovery, reliability, and supply chain. [V-S] ([Global Mining Review](https://www.globalminingreview.com/mining/25092026/avathon-selected-to-power-an-ai-native-mining-operating-model-for-barricks-north-american-business/))
    - This is the most important integration fact in this report: FrankenMining must be designed to complement that platform.
15. **Autonomy at NGM is now real and recorded in the MD&A.** [V-P] (Q2 2026 MD&A; [Komatsu](https://www.komatsu.jp/en/newsroom/2026/20260422))
    - The NGM Autonomous Haulage Program uses Komatsu FrontRunner.
    - **Cortez:** 23 trucks retrofitted by May 2026, a new fleet management system for the primary fleet, and site-wide rollout in Q3 2026. Budget about $50M (100% basis).
    - **Carlin:** 40 truck retrofits started in Q2 2026, with autonomy starting Q1 2027. Program spend about $65M through 2027.
    - Komatsu commissioned its **1,000th autonomous truck at NGM on 22 Apr 2026**.
16. **Industry costs are at records, and so are margins.**
    - WGC: industry-average AISC was **$1,785/oz in Q1 2026**, up 16% year on year. [V-P]
    - The Q2 2026 LBMA PM gold price averaged $4,506/oz. [V-P]
    - Gold went above $5,400/oz in Jan 2026. [V-S]
    - Barrick's 2026 AISC guidance is $1,760–1,950/oz. Newmont's by-product AISC is about $1,520/oz. Newmont's by-product figure is flattered by copper and silver credits; its co-product AISC was $1,963/oz in Q3. [V-P]
17. **Discovery economics are poor, which is why majors buy, partner and earn in.** [V-P/V-S] (MinEx/Schodde; S&P)
    - Cost per discovery tripled, from **$65M (1975–2005) to $218M (2011–2020)**.
    - Value created per exploration dollar fell from 1.29 to **0.69**.
    - Only **2–3 Tier-1 discoveries happen per year** worldwide.
    - It takes **15.7–17.9 years** to get from discovery to production.
18. **Megaprojects routinely overrun.** [V-S/V-P]
    - McKinsey: 83% of recent major mining projects had cost or schedule problems, with capex overruns of more than 40%.
    - Newmont's Tanami Expansion 2 went from about $395M in 2022 to **$1.7–1.8B**, and from early 2025 to H1 2027.
    - Barrick wrote off **$5.2B** on Pascua-Lama in Q2 2013 and about $6.1B for the full year.
19. **Tailings conformance is still incomplete at the majors.** [V-P]
    - GISTM was published in Aug 2020 and has 77 requirements. Deadlines were Aug 2023 (Extreme/Very High consequence) and Aug 2025 (all others).
    - Newmont said in Aug 2025 that it meets "the majority" of requirements, with gaps on an internal action plan.
    - Barrick: 14 of 18 Extreme/Very High facilities are in full conformance; Reko Diq has a disclosure gap.
20. **The tool stack is being absorbed by equipment makers (OEMs).** [V-P/V-S]
    - RPMGlobal went to **Caterpillar** (Feb 2026). Micromine went to **Weir** (Apr 2025, £624M EV). Deswik went to **Sandvik** (2022). Datarock went to **IMDEX** (2026). PETRA went to **Maptek** (Mar 2026). Modular Mining sits with **Komatsu**. Wenco sits with **Hitachi**.
    - The industry's open 3D interchange format, **OMF v2**, already has a **Rust reference implementation**: `gmggroup/omf-rust`, MIT licence, 2.0-beta. [V-P]

---

## 1. Barrick Mining Corporation (TSX: ABX; NYSE: B)

### 1.1 Identity, governance, leadership

| Item | Fact | Tag / source |
|---|---|---|
| Name | Barrick Gold Corp became **Barrick Mining Corporation**. Approved 6 May 2025 (98.14% for); NYSE ticker became "B" on 9 May 2025; TSX stays ABX. | [V-S] [Engineering News](https://www.engineeringnews.co.za/article/barrick-completes-name-change-elects-directors-2025-05-07); 6-K [SEC](https://www.sec.gov/Archives/edgar/data/756894/000119312525114445/d936939dex991.htm) |
| Former CEO | The previous President and CEO left effective 28 Sep 2025. | [V-P] Circular; [V-S] [Kitco](https://www.kitco.com/news/off-the-wire/2025-09-29/barrick-names-operations-chief-interim-ceo-after-sudden-bristow-exit) |
| CEO | Former Group COO; interim CEO from 29 Sep 2025; **President and CEO from 4 Feb 2026**; joined the Board as a non-independent director. Joined Barrick in 2006; previously ran LATAM and Asia Pacific. | [V-P] Q2 2026 MD&A; 2026 Circular |
| CFO | New CFO from 1 Mar 2026. | [V-P] Q2 2026 MD&A |
| Other executive changes | A new Chief Legal, Compliance and Policy Officer (from 1 Jun 2026) and a new Chief Global Affairs Officer. | [V-P] Q2 2026 MD&A |
| Chairman | Non-executive Chairman. | [V-P] 2026 Circular |
| Lead Director | A new independent Lead Director succeeded the previous one; the board renewal added directors with mining-technology and AI experience. | [V-P] 2026 Circular |
| Operating model | Regional leadership teams for North America, LatAm & Asia Pacific, and Africa & Middle East, with delegated authority. Functional teams (finance/risk/business assurance/**IT** under one leader; strategic matters; exploration; mineral resource management; metallurgy, engineering and projects; HSE; and others) report to the CEO. Barrick cites "an agile management structure that empowers our site based leadership teams." | [V-P] 2026 Circular; Q1/Q2 2026 MD&A |
| Activism | Press reports linked the Dec 2025 IPO evaluation to proposals from Elliott Investment Management to separate the North American assets. | [V-S] [mining.com](https://www.mining.com/barrick-eyes-ipo-of-north-american-gold-assets/). The 2026 Circular does not name Elliott. [V-P] |

**[INF] Implication.** IT sits under the CFO's finance, risk and business-assurance group, and authority is delegated to regions and sites. A top-down design that replaces existing systems would not fit that structure. The scopes that fit are:
- an enterprise layer for the finance function: capital allocation, reserves and AISC integrity, risk; or
- a regional or site deployment.

### 1.2 Portfolio and 2026 guidance (attributable)

Source: Barrick Q2 2026 MD&A guidance table [V-P]. Gold uses $4,500/oz and copper $5.50/lb as price assumptions.

| Asset (Barrick share) | Region | 2026 production guidance | 2026 AISC guidance | Notes |
|---|---|---|---|---|
| Carlin (61.5%, NGM) | Nevada | 600–670 koz | $1,900–2,100/oz | Tier One. Planned retrofit of 40 autonomous trucks; autonomous operations from Q1 2027. |
| Cortez (61.5%, NGM, incl. Goldrush) | Nevada | 430–480 koz | $1,690–1,870/oz | Tier One. Autonomous haulage live (23 trucks). |
| Turquoise Ridge (61.5%, NGM) | Nevada | 300–330 koz | $1,490–1,650/oz | Tier One. |
| Phoenix (61.5%, NGM) | Nevada | 80–100 koz | $1,180–1,310/oz | Gold–copper. |
| **NGM total (61.5%)** | Nevada | 1,420–1,580 koz | $1,720–1,900/oz | JV with Newmont (38.5%); Barrick operates. |
| Pueblo Viejo (60%) | Dominican Rep. | 350–400 koz | $1,590–1,760/oz | Tier One. Plant expansion; Naranjo tailings facility starter-dam permit targeted Q1 2027; 570 families resettled. |
| Veladero (50%) | Argentina | 180–200 koz | $1,460–1,620/oz | JV with Shandong Gold [UNV partner name]. |
| Porgera (24.5%) | PNG | 80–100 koz | $1,610–1,780/oz | Equity-accounted; New Porgera structure (§6.7). |
| Loulo-Gounkoto (80%) | Mali | 260–290 koz | **$2,640–2,900/oz** | Restarted after the dispute; carries the 2023 Mining Code cost burden. |
| Kibali (45%) | DRC | 270–310 koz | $1,330–1,470/oz | Equity-accounted; Barrick operates [UNV operator status this session]; 16 MW solar plus battery project. |
| North Mara (84%) | Tanzania | 200–230 koz | $1,520–1,680/oz | Twiga partnership with the Government of Tanzania [UNV detail]. |
| Bulyanhulu (84%) | Tanzania | 140–160 koz | $1,870–2,070/oz | |
| **Total gold** | | **2.90–3.25 Moz** | **$1,760–1,950/oz** | |
| Lumwana (100%) | Zambia | 130–150 kt Cu | $3.40–3.75/lb | **Super Pit Expansion: 240 ktpa Cu from a 52 Mtpa plant, life above 30 years, first copper end Q1 2028.** |
| Zaldívar (50%) | Chile | 30–35 kt | $5.40–5.70/lb | Partner Antofagasta [UNV]; EIA approved to extend life to 2051 [V-P Circular]. |
| Jabal Sayid (50%) | Saudi Arabia | 25–30 kt | $1.45–1.65/lb | Partner Ma'aden [UNV]. |
| **Total copper** | | **190–220 kt** | **$3.45–3.75/lb** | |
| Reko Diq (50% basis) | Pakistan | — | — | Development slowed (§1.6). |
| Fourmile (100%, being contributed to NGM) | Nevada | — | — | 2025 PEA: capex about $1.5–1.7B, 600–750 koz/yr, AISC about $650–750/oz at $2,585/oz gold. Resource 2.6 Moz Indicated and 13 Moz Inferred (end-2025). 20 rigs active. PFS 2028. Barrick says it has the "potential to be a standalone Tier One Gold Asset." [V-P] |

**Disposals since 2025** [V-P] (Q2 2026 release; 2026 Circular):
- 50% of Donlin Gold (gain $745M).
- Hemlo.
- The Tongon interest.
- Alturas.
- Total disposal cash: more than **$2.1B**.

**Reserves at 31 Dec 2025** [V-P] (FY2025 release):
- **Gold:** **85 Moz P&P at 0.98 g/t**, down from 89 Moz. Depletion of 3.7 Moz and divestments of 2.2 Moz outweighed 1.8 Moz of additions.
- **Copper:** **18 Mt P&P at 0.46%**, flat.
- **Replacement:** the three-year rolling gold reserve replacement is "close to 190%" (more than 24 Moz added, excluding M&A).

**Metric bases worth knowing.** The Circular's long-term scorecard shows reserve replacement of "2025: 29%; 3-year average 504%," while the results release says about 190%. These are evidently different bases (gold-only vs gold-equivalent, gross vs net, treatment of M&A). [V-P both; reconciliation UNV]. **[INF]** Headline KPIs published on more than one basis need a bridge. An auditable metrics engine renders each basis from one cited roll-forward.

### 1.3 What "Tier One" means at Barrick (verbatim)

- **2026 Circular glossary** [V-P]:
  - "**Tier One Gold Asset**: An asset with a **$1,400 per ounce reserve** with potential to deliver a **minimum 10-year life**, **annual production of at least 500,000 ounces** of gold and with costs per ounce in the **lower half of the industry cost curve**. Tier One Gold Assets must be located in a world-class geological district with potential for organic reserve growth and long-term geologically driven addition[s]."
  - "**Tier One Copper Asset**: An asset with a **$3.00 per pound reserve** with potential for **five million tonnes or more of contained copper** in support of **at least 20 years life**, **annual production of at least 200,000 tonnes**, with costs per pound in the lower half of the industry cost curve…"
- **Q2 2026 results release, endnote 5** [V-P]: the same structure, but the gold reserve price is **$1,500/oz** and the copper reserve price is **$3.25/lb**. The reserve price deck is updated over time.
- **Which assets qualify** [V-P] (Q1/Q2 2026 MD&A):
  - The North American Barrick perimeter contains "**four Tier One Gold Assets**": Carlin, Cortez, Turquoise Ridge and Pueblo Viejo.
  - Fourmile is a Tier One candidate.
  - Loulo-Gounkoto and Kibali have historically been called Tier One by Barrick [UNV for 2026 status].
- **MinEx (Schodde) uses a different Tier 1 definition** for discoveries [V-P] ([MinEx IMARC 2023](https://minexconsulting.com/wp-content/uploads/2023/11/IMARC-Presentation-31-Oct-2023.pdf)): life above 20 years, more than 250 koz/yr Au or more than 200 ktpa Cu, bottom-quartile costs, NPV above $1B. **Do not mix the two.**

**[INF]** A capital-allocation module should treat "Tier One" as a **parameterised filter**, with price deck, minimum life, minimum rate, cost-curve percentile and district-growth potential as inputs. It should also show *distance-to-Tier-One* for every asset and target. That mirrors how Barrick screens opportunities ("strategic filters driving investment," per the Circular's board-education list).

### 1.4 Strategy and capital allocation

- **Strategy statement** (Chairman's letter, 2026 Circular) [V-P]:
  - "We concentrate our capital on Tier One assets that can deliver low-cost production over long lives. We grow the business organically through exploration, disciplined project development, and selective acquisitions… We maintain strict capital discipline to grow per-share value… We continue to optimize the portfolio and pursue structural initiatives that unlock greater value."
  - "We will continue to use technology, particularly AI, to make us safer, more efficient, and more responsible. We will maintain unwavering discipline in capital allocation, with an uncompromising eye on growing **free cash flow per share**."
- **Dividend policy** (adopted Q4 2025) [V-P]: targets **50% of attributable FCF**. It consists of a **$0.175/quarter base** plus a year-end performance top-up. The Q4 2025 dividend was $0.42.
- **Buybacks** [V-P]: $1.5B in 2025, part of a record $2.39B returned that year. A new **$3B, 12-month program** was authorised on 8 May 2026; $1.209B was repurchased in Q2 2026.
- **Capex 2026** [V-P]: total attributable capex was cut to **$3.8–4.2B** from $4.0–4.45B, mainly because of lower Reko Diq spend.
- **Balance sheet at Q2 2026** [V-P]: net cash about $1.245B; net leverage −0.1x.
- **ROCE** [V-P]: Barrick's "long-term performance range" is **10%–15%**. 2025 ROCE was 32.5% and the three-year average 21.0%. Barrick says it reallocates capital "to only those projects that meet target returns and support the Company's long-term investment filters."
- **FY2025 financials** [V-P]:

  | Metric | 2025 |
  |---|---|
  | Gold production | 3.26 Moz (−17% y/y) |
  | Copper production | 220 kt (+13%) |
  | Gold AISC | $1,637/oz |
  | Copper AISC | $3.20/lb |
  | Operating cash flow | $7.69B |
  | Free cash flow | $3.87B |
  | Net earnings | $4.99B |

- **Q2 2026** [V-P]:

  | Metric | Q2 2026 |
  |---|---|
  | Gold production | 796 koz |
  | Copper production | 56 kt |
  | Gold AISC | $1,866/oz |
  | Copper AISC | $3.95/lb |
  | Revenue | $5.29B |
  | Attributable FCF | **$141M** (Q1: $1.21B) |

  Drivers cited: lower grades, fuel and royalties.

### 1.5 KPIs that drive Barrick executive pay (2025 scorecards, 2026 Circular) [V-P]

**Annual Performance Incentive (API).** 70% is company-wide and 30% is individual strategic priorities. The individual share was cut from 70% (2023) to 50% (2024) to 30% (2025).

| Company API measure | Weight (of 70%) | Threshold | Maximum | 2025 actual |
|---|---|---|---|---|
| Safety: LTIFR | 5% | 0.20 | 0.15 | 0.09 |
| Safety: Actions close-out from all incident investigations | 5% | 75% | 90% | 97% |
| Safety: **Critical Control Verifications** | 5% | 75% | 85% | 84% |
| Environment: Class 1 environmental incidents | 5% | 2 | 0 | 0 |
| Production: attributable gold-equivalent ounces | 25% | 3,471 koz | 4,050 koz | 3,675 koz |
| Gold AISC | 18.75% | $1,565/oz | $1,352/oz | $1,423/oz |
| Copper AISC | 6.25% | $2.87/lb | $2.48/lb | $2.62/lb |

- **Outcome:** 63.9% of maximum. Four fatalities in 2025 triggered a 5% reduction, giving 56.7%. The Africa & Middle East regional scorecard was cut 10%.
- **Caveat:** the AISC "actuals" here ($1,423/oz) differ from the $1,637/oz reported for FY2025. This presumably reflects compensation adjustments, such as price-linked royalties normalised to plan. The adjustment basis was not located [UNV].

**Long-Term Company Scorecard (PGSU).** For 2025 the result was **60%**.

| Measure | Weight | Basis | 2025 outcome |
|---|---|---|---|
| Relative TSR vs **GDX** constituents | 35% | 50th–75th percentile | 31st percentile, so nil |
| ROCE | 15% | 10%–15% | 21.0% three-year average, full payout |
| Reserve replacement (three-year rolling) | 15% | above 75% to above 100% | full payout |
| Strategic execution | 15% | milestones | full payout |
| ESG and licence to operate | 20% | Grade C to A | Grade A, capped at 75% for fatalities |

**Safety reporting trend** [V-P] (Q1/Q2 2026 MD&A):

| Period | TRIFR | LTIFR |
|---|---|---|
| Q1 2025 | 0.71 | 0.10 |
| Q4 2025 | 0.79 | 0.08 |
| Q1 2026 | 0.92 | 0.05 |
| Q2 2026 | 0.77 | 0.15 (six LTIs) |

- There were no fatalities in H1 2026. Two died in Q4 2025: one at Bulyanhulu on 21 Oct and one at Kibali on 15 Dec.
- Barrick is "continuing to shift toward the use of **leading indicators** to better understand and manage fatal risk exposure": whether critical controls are in place and effective, and whether they are routinely verified.
- $90M is being "invested in safety technology this year" (2026).

**[INF]** The safety-technology and critical-control-verification language maps directly onto a "critical control" data product: field verifications, close-out workflows and leading indicators. That metric is already in Barrick's published scorecard, so the module serves a measure the company already tracks.

### 1.6 Major events, 2025–2026

| Event | Facts | Tag |
|---|---|---|
| **Mali / Loulo-Gounkoto** | **Jan 2025:** Barrick suspended operations. **Mid-2025:** Mali imposed provisional administration and seized about 3 t of gold [V-S]. **24 Nov 2025:** settlement; charges dropped, arbitration withdrawn [V-S]. **16 Dec 2025:** control handed back [V-P]. Reported payment about $430M [V-S]. **Q1 2026:** ramp-up [V-P]. **Q2 2026:** restart "ahead of schedule"; retrospective 2023 Mining Code royalties, penalties and interest booked [V-P]. Loulo-Gounkoto 2026 AISC guidance of $2,640–2,900/oz is the highest in the portfolio [V-P]. | [mining.com](https://www.mining.com/barrick-shares-surge-as-it-resolves-dispute-with-mali/); Q1/Q2 2026 MD&A |
| **Reko Diq** (Cu–Au, Balochistan) | **5 Feb 2026:** review of "all aspects… in light of the escalation of security risks." **2 Apr 2026:** "slow development activity and extend the project review" to assess "security situation, capital requirements, project financing, project scope and timeline." Capitalised spend to Q1 2026: $1,083M (100% basis). The prior 2026 guidance of $600–700M attributable capex is under review. Forward-looking statements now warn of "potential significant increases to previously disclosed total estimated capital budget and timeline." | [V-P] Q1/Q2 2026 MD&A |
| Reko Diq pre-review plan | Phase 1 first production about end-2028, Phase 1 capex about $5.6B (100%), Phase 2 expansion to about 90 Mtpa [UNV, recall of the Feb 2025 technical report]. Ownership: Barrick 50%, Pakistan federal SOEs 25%, Balochistan 25% [UNV]. | — |
| **North American Barrick IPO** | **Dec 2025:** Board approved evaluating an IPO of "NewCo" [V-S]. **Feb 2026:** decision to prepare the IPO [V-P]. **Apr 2026:** dedicated NA executive appointments [V-P]. **Aug 2026:** Newmont consent [V-P]. Target: complete by end-2026, primary NYSE listing [V-P]. | Q4 2025 release; Q1/Q2 2026 MD&A |
| **NGM settlement with Newmont** | Second Amended and Restated LLC Agreement of Nevada Gold Mines dated **10 Aug 2026** (6-K filed 17 Aug 2026). Barrick contributes Fourmile; Newmont contributes Mike and Fiberline. Newmont pays $1.95B. "Resolves all outstanding disputes related to NGM." The nature of the disputes was not researched [UNV]. | [V-P] [6-K](https://www.sec.gov/Archives/edgar/data/756894/000119312526353144/d182096d6k.htm); Q2 2026 release |
| **Copper growth** | Lumwana Super Pit (240 ktpa, first copper end Q1 2028, on schedule). Zaldívar extended to 2051. Reko Diq slowed. Copper reserves 18 Mt. | [V-P] |
| **Exploration and junior deals** | Barrick earn-in on GFG Resources' Pen project, Ontario (25 Aug 2026): 60% for $1.2M cash plus a minimum $4.0M of exploration over 3 years; up to 70% by funding a PEA. Older earn-ins: Osisko Kan, Precipitate Pueblo Grande, MetalCorp Hemlo East, Melkior White Lake [V-S, undated]. Greenfield drill-testing in 2025 in Canada, Nevada, Peru and Tanzania [V-P Circular]. | [GFG release](https://s21.q4cdn.com/712049373/files/doc_news/GFG-and-Barrick-Sign-Strategic-Earn-In-to-Accelerate-Exploration-at-the-Pen-Gold-Project-2026.pdf) [V-P] |

### 1.7 How Barrick talks about technology, digital, AI and autonomy

- **Chairman (2026 Circular)** [V-P]: "continue to use technology, particularly AI, to make us safer, more efficient, and more responsible."
- **Board** [V-P] (2026 Circular):
  - The Audit & Risk Committee oversees "risks and opportunities relating to artificial intelligence."
  - In 2025 it received a briefing from the VP Group IT on cybersecurity and "artificial intelligence risks and opportunities."
  - Board education included "**Autonomous Mining**" on 7 Nov 2025: "technological advances in mining logistics and autonomous haulage; potential impacts on safety; productivity, cost reduction, and sustainability."
  - Board education also covered CFO-area "**digital initiatives**" and HR "digital cost-reduction initiatives."
- **NGM Autonomous Haulage Program** [V-P] (Q2 2026 MD&A):
  - Described as "a strategic NGM initiative to improve safety, productivity, and operating consistency," deployed in phases.
  - **Cortez:** 23 haul-truck retrofits completed May 2026; "transition to the new Fleet Management System for the primary production fleet has been completed"; Q3 2026 site-wide rollout; $37M spent of about $50M.
  - **Carlin:** 40 truck retrofits from Q2 2026; autonomous operations planned for Q1 2027; about $30M in 2026 and about $65M through 2027.
  - The FMS vendor is not named in the MD&A [UNV]. It is likely Komatsu/Modular, given the FrontRunner partnership.
- **Komatsu partnership** [V-P]:
  - NGM and Komatsu announced FrontRunner AHS across NGM surface operations (300 t and 230 t trucks) on 31 Jul 2025, with a private 5G network by Sedna/Nokia ([NGM–Komatsu release](https://s25.q4cdn.com/322814910/files/doc_news/2025/07/NGM-and-Komatsu-Launch-First-of-Its-Kind-Autonomous-Haulage-Partnership_Final.pdf)).
  - Komatsu's **1,000th autonomous ultra-class truck** was a 930E-5AT at NGM on 22 Apr 2026 ([Komatsu](https://www.komatsu.jp/en/newsroom/2026/20260422)).
  - Earlier, ASI Mining retrofitted trucks at Arturo [V-S].
- **Avathon at North American Barrick (25 Sep 2026)** [V-S] (Global Mining Review):
  - Barrick's North American business selected Avathon's **Autonomy Platform** as "strategic technology partner."
  - Avathon describes "a common intelligence layer" in which a "proprietary **Computational Knowledge Graph** connects assets, processes, people, constraints, and operational data" and "AI agents reason across that context."
  - The six initial use cases are:
    1. Safety (computer vision).
    2. Production and recovery.
    3. Asset reliability.
    4. Supply-chain intelligence.
    5. Mine planning.
    6. Exploration and growth.
  - The CEO: "AI has the potential to change how a mining company safely operates at its core."
  - **No KPIs, timelines or commercial terms were disclosed.**
  - Avathon's own site positions it as "The knowledge and AI platform for autonomous operations," built on ontology, agentic AI and governance [V-P] ([avathon.com](https://avathon.com/)).
  - That Avathon was formerly SparkCognition is [UNV]. The site does not say so.
- **ERP and planning systems** [V-P/V-S]:
  - Barrick chose **SAP S/4HANA** in Jan 2020 for finance, capital, supply chain and asset management in more than 15 countries ([SAP News](https://news.sap.com/uk/?p=132589)).
  - In 2025 the CFO "led the implementation of key **SAP and Xeras** systems at Porgera" [V-P Circular]. Xeras is understood to be RPMGlobal's (now Caterpillar's) financial-modelling and budgeting tool [UNV attribution].
  - Micromine **Pitram** runs at NGM Goldrush and has been used at Cortez, Goldstrike and Leeville for about 20 years [V-S].
  - Leapfrog has been used since 2005 [V-S, dated]. Maptek Vulcan was chosen for Pascua-Lama in 2010 [V-S, dated].
- **Missing from the public record:** Barrick's results releases and MD&As contain no AI programme, no digital KPIs and no quantified digital benefits [V-P, absence checked in Q1/Q2 2026 MD&A and FY2025 release].

### 1.8 Tailings (GISTM) at Barrick [V-P]

- **Portfolio:** 64 facilities, 18 of them Extreme (6) or Very High (12) consequence.
- **Status of those 18:** 14 in full conformance, 1 partial, 1 in safe closure, and **2 at Reko Diq** for which Barrick "cannot disclose conformance to GISTM in 2026." Those two were conforming in 2025; work on them is paused under the project review.
- **Safe closure:** 12 facilities overall.
- In 2025 Barrick could not disclose for Loulo-Gounkoto while it was under provisional administration.
- Sources: [Barrick tailings disclosures](https://www.barrick.com/tailings-disclosures/); [2025 GISTM release](https://www.barrick.com/English/news/news-details/2025/barrick-publishes-GISTM-TSF-disclosures/default.aspx).

---

## 2. Newmont Corporation (NYSE/ASX/PNGX: NEM)

### 2.1 Leadership [V-P unless noted]

- **CEO:** **Natascha Viljoen**, President and CEO from **1 Jan 2026**. She was previously COO, then President and COO. She succeeded **Tom Palmer**, who retired after more than 12 years ([2026 Proxy](https://www.sec.gov/Archives/edgar/data/1164727/000110465926035211/nem-20260512xdef14a.htm)). The succession was announced 29 Sep 2025 [V-S] ([BusinessWire](https://www.businesswire.com/news/home/20250929730136/en/Newmont-Announces-Retirement-of-CEO-Tom-Palmer-Names-President-COO-Natascha-Viljoen-as-Successor/)).
- **Appointments from 1 Jul 2026** ([8-K Ex. 99.1](https://www.sec.gov/Archives/edgar/data/1164727/000110465926074137/tm2617970d1_ex99-1.htm)):
  - **CFO:** Brian Tabolt.
  - **COO:** Mark Rodgers.
  - **Chief Technical Officer:** David Thornton.
  - **EVP Project Development:** David Fry.
- **Departure:** CTO **Francois Hardy** retired effective 30 Jun 2026. Erin Workman, Group Head Exploration & Geosciences, was acting CTO from May 2026 ([8-K](https://www.sec.gov/Archives/edgar/data/1164727/000110465926050355/tm2612965d1_8k.htm)). Hardy had signed Newmont's 2025 GISTM letter as Accountable Executive.
- **Board:** Peter David Beaven joined as an independent director on 1 Sep 2026.
- **Workforce** (FY2025 10-K): about **17,500 employees and 26,600 contractors** at 31 Dec 2025.

### 2.2 Portfolio after Newcrest and after divestment [V-P] (Q3 2026 release)

| Segment | Assets (Q3 2026 attributable koz) |
|---|---|
| **Managed Core** | Boddington 150; Lihir 135; Yanacocha 117; Ahafo South 104; Tanami 81; Brucejack 72; Ahafo North 69; Cadia 61 (recovering from an Apr 2026 seismic event); Cerro Negro 55; Merian (75%) 49; Peñasquito 46; Red Chris (70%) 9 |
| **Non-Managed Core** | NGM (38.5%) 251; Pueblo Viejo (40%) 65; Fruta del Norte via 32% of Lundin Gold 38 |
| **Non-Core (sold)** | CC&V, Musselwhite, Éléonore (Q1 2025); Porcupine, Akyem (Q2 2025); Coffee (Q4 2025). About $4.5B total after-tax proceeds, including $134M from Greatland shares in Jan 2026 |

- The Newcrest acquisition closed Nov 2023 [UNV this session, widely known].
- Newmont's 2026 documents do **not** use "Tier 1." The FY2025 10-K has zero hits for "Tier 1"/"Tier One" [V-P, absence checked]. Its earlier "Tier 1 Portfolio" framing is historical [UNV wording].
- **Junior stakes being monetised** [V-P] (Q1 2026 release): about $321M net from selling SolGold and Greatland stakes. Newmont Ventures still does earn-ins, for example Awalé in Côte d'Ivoire: up to $15M for up to 75% [V-S].

### 2.3 Results and guidance [V-P]

- **FY2025** ([Q4 release](https://www.sec.gov/Archives/edgar/data/1164727/000116472726000009/newmontq42025earningsand20.htm)):

  | Metric | FY2025 |
  |---|---|
  | Attributable gold | 5.9 Moz (5.7 Moz core) |
  | Silver | 28 Moz |
  | Copper | 135 kt |
  | By-product AISC | $1,358/oz |
  | Co-product AISC | $1,609/oz |
  | Free cash flow | **$7.3B (record)** |
  | Returned to shareholders | $3.4B |
  | Debt reduced | $3.4B |
  | Net cash | $2.1B |
  | Gold reserves | **118.2 Moz** |
  | Gold resources | 148.7 Moz |
  | Copper reserves | 12.5 Mt |
  | Silver reserves | 442 Moz |

  Longer-term outlook: "approximately **6 million ounces of gold and 150 thousand tonnes of copper** annually."
- **2026 guidance:**
  - Feb 2026: about 5.3 Moz at by-product AISC of $1,680/oz.
  - Oct 2026 update: **5,260 koz, by-product AISC $1,520/oz**, sustaining capex $1.95B, development capex $1.4B.
- **Q3 2026** ([release](https://www.sec.gov/Archives/edgar/data/0001164727/000116472726000041/newmontq32026earningsrelea.htm)):

  | Metric | Q3 2026 |
  |---|---|
  | Attributable gold | 1,302 koz |
  | By-product AISC | $1,454/oz |
  | Co-product AISC | $1,963/oz |
  | Free cash flow | $1.912B |
  | Net cash (before Barrick's $1.95B payment) | $3.2B |
  | Realised gold price | $4,317/oz |

  - Newmont "expects unit costs to trend higher in 2027" because of inflation above its 3% assumption, diesel prices and FX.

### 2.4 Capital allocation framework (Feb 2026, in priority order) [V-P]

1. **Sustaining capital** "in world-class portfolio," including "tailings solutions, primarily at Cadia and Boddington… to produce well into the middle of the century." This means elevated sustaining capex for several years.
2. **A fixed cash dividend of $1.1B/yr.** The per-share amount rises as buybacks shrink the share count. The 2026 rate is $1.04/share annualised.
3. **Disciplined development capital:** about $1.4B in 2026.
4. **Optimised capital structure:** a **$1B net cash target ±$2B** and a **minimum cash balance of $5B**.
5. **Ratable share repurchases:** $6.0B authorisation, $2.8B remaining at Q3 2026. The share count is down more than 115M (about 10%) since Feb 2024.

### 2.5 Executive pay metrics (2026 Proxy) [V-P]

- **2025 annual incentive (six metrics):**

  | Metric | Weight | 2025 range |
  |---|---|---|
  | **Adjusted Cash Sustaining Costs per GEO** | 35% | threshold $1,683, target $1,603, max $1,373 |
  | **Adjusted Free Cash Flow** | 35% | — |
  | Significant Potential Event Frequency Rate | 14% | — |
  | Fatality Risk Management | 6% | — |
  | Operating-site water consumption efficiency | 5% | — |
  | "Focusing on Our Culture" | 5% | — |

  Overall payout: 155.15% of target.
- **2026 changes:**
  - A leading-indicator safety metric for timely closure of critical safety actions was added.
  - The Newcrest synergy metric was removed.
  - The threshold payout rose from 20% to 35%.
- **PSUs:** relative TSR 60% (target at the 55th percentile; capped at 100% if TSR is negative); **ROCE raised to 30%**; long-term incentives also include Scope 1/2 emissions-project milestones. RSUs are 33% of LTI.

### 2.6 Projects (status Oct 2026) [V-P unless noted]

- **Red Chris block cave (70%, B.C.):**
  - Feasibility study nearly done; **full-funds decision early 2027**.
  - First phase: 3.0 Moz Au and more than 850 kt Cu over a 13-year cave life.
  - Construction from 2027; commercial production **2033**.
  - Development capex **$4.2–4.8B** (real, net of a C$500M Government of Canada contribution).
  - Potential life up to 70 years.
  - Amended B.C. Environmental Assessment Certificate received Jun 2026 through a consent-based process with the Tahltan Nation.
- **Tanami Expansion 2:**
  - Shaft complete. Commercial production now "first half of 2027", ahead of the earlier H2 2027.
  - Total capex **$1.7–1.8B**. The 2022 estimate was about $395M with an early-2025 target [V-S] ([Mining Weekly 2022](https://www.miningweekly.com/article/newmont-warns-of-project-cost-increases-in-australia-ghana-2022-07-25)).
- **Cadia panel caves:**
  - PC2-3: 1.0 Moz Au and more than 400 kt Cu over 2024–2034.
  - PC1-2: 4.0 Moz Au and more than 700 kt Cu over 2027–2042; capex $1.0–1.2B.
  - A seismic event in Apr 2026 hit Q2 output; PC1-2 has resumed.
- **Ahafo North:** commercial production 24 Oct 2025, with a 13-year initial life.
- **Lihir:** Nearshore Barrier approved, giving access to more than 5 Moz and life beyond 2040.
- **Yanacocha:** water treatment plants about $1.8B in total ($769M spent at Feb 2026).

### 2.7 How Newmont talks about technology and AI

- **FY2025 10-K** [V-P] ([10-K](https://www.sec.gov/Archives/edgar/data/1164727/000116472726000010/nem-20251231.htm)):
  - "AI, for example, is increasingly leveraged by Newmont for decision-making, mineral extraction optimization, and autonomous operations."
  - "In November 2025, we implemented an enterprise-wide **Artificial Intelligence Standard** that governs AI adoption and use, model lifecycle management, and associated cybersecurity and privacy controls."
  - Boddington runs "a fleet of **41 production autonomous haulage trucks**."
  - The 10-K lists AI, fleet electrification and autonomous vehicles as cyber-risk expanders.
- **Board:** the Audit Committee oversees "cybersecurity and artificial intelligence risks." "Innovation / Technology / Cybersecurity" is a tracked director skill, held by 7 directors [V-P Proxy].
- **Quarterly releases are silent on technology.** The Q4 2025, Q1 2026 and Q3 2026 releases contain no technology, automation or AI statements [V-P, absence checked].
- **Boddington autonomous haulage (Cat MineStar Command)** [V-P/V-S]:
  - Conversion started Mar 2021.
  - **$150M** investment; IRR above 35% [V-S].
  - More than 100 Mt hauled autonomously by Oct 2022 [V-P Cat].
  - The 2021 ramp-up was hurt by weather, shovel reliability and "bench hygiene," leaving output about 140 koz below 830 koz guidance [V-S].
  - Sources: [Cat](https://www.cat.com/en_US/news/machine-press-releases/newmonts-boddington-mine-safely-hauls-more-than-100-million-tonnes-of-material-autonomously-using-cat-minestar-command-for-hauling.html); [GMR 2021](https://www.globalminingreview.com/environment-sustainability/11102021/newmont-provides-update-on-boddington-mine/).
- **Newmont–Caterpillar alliance (Nov 2021):** $100M toward battery-electric autonomous haulage at CC&V and Tanami [V-S]. CC&V has since been sold, so the current status is [UNV].
- **Other systems:** Leapfrog and Seequent Central since 2020 [V-S, vendor-sourced]. SAP S/4HANA consolidation including Newcrest [V-S]. "Full Potential" savings were not verified [UNV].

### 2.8 Tailings (GISTM) at Newmont [V-P]

- Newmont's letter to ICMM of 1 Aug 2025 says it is "meeting the majority of GISTM requirements across all tailings storage facilities." Gaps are tracked in an internal action plan, and independent validation is pending ([PDF](https://s24.q4cdn.com/382246808/files/doc_downloads/2025/08/Newmont-GISTM-ICMM-2025.pdf)).
- It lists **11 priority Very High/Extreme facilities**:
  - Operating: Ahafo South; Boddington ×2; Merian; Peñasquito; Red Chris; Yanacocha ×2.
  - Legacy: Equity Silver, Golden Giant, Marlin.
- **Cadia** (the 2018 NTSF slump legacy; not re-checked) and **Boddington** tailings are explicit drivers of 2026 sustaining capex.

---

## 3. Side by side: the two companies' published scorecards [V-P inputs; synthesis INF]

| Dimension | Barrick | Newmont | FrankenMining design response [INF] |
|---|---|---|---|
| Unit cost KPI | AISC/oz (WGC basis), gold and copper separately; 25% of bonus | Adjusted **CSC/GEO**; 35% of bonus | A single, auditable cost engine with WGC-compliant AISC/AIC, by-product vs co-product views, and comp-adjusted variants with explicit reconciliation. |
| Volume | Attributable GEO; 25% of bonus | (implicit in CSC/GEO and FCF) | Plan-to-actual GEO with probabilistic forecasts. |
| Cash | FCF per share (strategy); dividend = 50% of attributable FCF | Adjusted FCF, 35% of bonus; fixed $1.1B dividend | FCF bridge from mine plan to treasury, run under price-deck scenarios. |
| Capital efficiency | ROCE, 15% of LTI (10–15% target range) | ROCE, 30% of PSUs | Project and portfolio ROCE attribution; capital-allocation optimiser with Tier One filters. |
| Market | rTSR vs GDX, 35% of LTI | rTSR, 60% of PSUs | Out of scope, but every module should roll up to per-share value. |
| Resource base | Three-year reserve replacement, 15% of LTI | (not in comp) | Reserve and resource pipeline with conversion probabilities; exploration portfolio optimiser. |
| Safety | LTIFR, close-out rate, **critical control verifications** | SPEFR, fatality risk management, critical-action close-out (2026) | Critical-control verification and close-out system. |
| ESG | Class 1 environmental incidents; ESG/licence-to-operate grade | Water efficiency; emissions milestones | GISTM evidence vault; water balance; licence-to-operate register. |
| Structure | Splitting NA from the rest; regional autonomy | Post-integration "one Newmont"; centralised standards (e.g. AI Standard) | NA Barrick IPO means a carve-out of systems (SAP, data, reporting); FrankenMining stays off that critical path [INF]. |

---

## 4. Industry economics (research question B)

### 4.1 WGC AISC / AIC definitions [V-P unless noted]

- **Source:** WGC non-GAAP guidance note, issued **26 Jun 2013**, updated **Nov 2018** (with a FAQ and lease accounting under IFRS 16/ASC 842) ([WGC](https://www.gold.org/goldhub/research/guidance-note-non-gaap-metrics-%e2%80%93-all-sustaining-costs-and-all-costs); [Mining Weekly 2018](https://www.miningweekly.com/article/wgc-publishes-update-to-2013-all-in-sustaining-cost-guidance-note-2018-11-14) [V-S]).
- **AISC includes:**
  - on-site costs;
  - corporate G&A, including share-based pay;
  - lease principal and financing components;
  - R&D on existing operations;
  - production-phase stripping and underground development;
  - current-inventory net-realisable-value write-downs;
  - sustaining exploration and capital;
  - reclamation at operating sites.
- **AISC excludes:**
  - interest and financing;
  - M&A;
  - impairments;
  - one-off severance;
  - exceptional non-current inventory write-downs;
  - discontinued operations;
  - income tax;
  - most working-capital changes.
- **AIC** = AISC plus non-sustaining items: growth projects and greenfield, non-sustaining exploration, and community, permitting and reclamation costs not tied to current operations [V-S].
- **The sustaining vs non-sustaining test:**
  - A project is non-sustaining (a "major project") if it raises annual or life-of-mine production, NPV or reserves by at least 10% at the operation level, **and** it generates no incremental net cash within 12 months.
  - Pushbacks, laybacks and underground extensions count as sustaining unless they take at least 12 months **and** the ore phase is longer than 5 years.
  - By-product credits reduce AISC.
- Barrick explicitly cites the 2013 WGC definition in its 2026 Circular [V-P].
- **[INF]** The classification rules are judgement-heavy and the two companies' bases diverge (Newmont by-product vs co-product, Barrick comp-adjusted AISC). That makes this a natural home for a rules engine with audit trail.

### 4.2 Cost levels and cost curves

- **Industry average AISC:** **$1,785/oz in Q1 2026**, up 5% q/q and 16% y/y, driven by royalties and corporate G&A [V-P] ([WGC Gold Demand Trends Q2 2026, supply](https://www.gold.org/goldhub/research/gold-demand-trends/gold-demand-trends-q2-2026/supply)).
  - Q2 2026 mine production: 966 t, a Q2 record. H1 2026: 1,867 t, a record.
  - Q2 2026 LBMA PM average: $4,506/oz.
  - Producer de-hedging in Q2: about 23 t.
  - WGC expects the energy-price impact of the Middle East conflict to show from Q2 2026.
- **FY2025 industry AISC:** conflicting secondary figures of about $1,552/oz (Metals Focus) and about $1,709/oz (WGC cost-curve median) [UNV].
- **Cost-curve framing:** Barrick's Tier One filter requires the "lower half of the industry cost curve" [V-P]. MinEx's Tier 1 requires the bottom quartile [V-P]. Proprietary cost curves (Metals Focus for WGC, S&P, Wood Mackenzie) are paywalled; I did not access them [UNV].

### 4.3 Capital intensity

- **Copper (FactSet, 16 Jul 2026, 96 pipeline projects)** [V-P] ([FactSet](https://insight.factset.com/metals-mining-copper-capex-watch)):

  | Category | US$ per tpa Cu |
  |---|---|
  | Median | **$18,048** |
  | Production-weighted | $25,228 |
  | Greenfield median | $18,200 |
  | Brownfield | $13,302 |
  | Restarts | $11,114 |
  | Expansions | $22,086 |
  | Copper M&A median, 2020–26 | **$25,182** |

  - FactSet: "a greenfield build plus a typical 40% overrun (about $25,480) ≈ the cost of buying production." That is the build-vs-buy parity executives use.
- **Copper (other):** an S&P sample of 26 projects gave about $22,359/tpa; Teck's QB2 was about $28,000/t [V-S, not checked at S&P].
- **Gold:** no authoritative current benchmark was found.
  - Perpetua Stibnite: about $2.58B for about 463 koz/yr, roughly $5,600 per annual oz [V-S].
  - Ahafo North: about $950M for about 300 koz/yr, roughly $3,000–3,500 per annual oz [INF from V-S inputs].
  - Fourmile PEA: $1.5–1.7B for 600–750 koz/yr, roughly $2,000–2,800 per annual oz [INF from V-P].
  - Treat about **$2,000–6,000 per annual oz** as the current range [UNV].

### 4.4 Exploration-to-production lead times

- **S&P Global Market Intelligence** [V-S/V-snip]:
  - **15.7 years** average from discovery to production for 127 mines started 2002–2023 (range 6–32 years); gold about 15.2 years ([S&P](https://www.spglobal.com/marketintelligence/en/news-insights/research/discovery-to-production-averages-15-7-years-for-127-mines)).
  - **17.9 years** for gold, copper, nickel and lithium mines started 2020–23, versus 12.7 years for 2005–09 ([S&P](https://www.spglobal.com/marketintelligence/en/news-insights/research/average-lead-time-almost-18-years-for-mines-started-in-2020-23)).
  - 17.8 years in a 2025 update for 2020–24 starts.
  - The extra time sits in studies and permitting, and in the gap between feasibility and construction.
- **MinEx (Schodde):** discovery-to-mine averages about 13 years and is lengthening [V-P] ([CSIRO](https://www.csiro.au/en/news/all/articles/2019/april/21st-century-gold)).

### 4.5 Discovery success and cost

Primary source: [MinEx IMARC, 31 Oct 2023](https://minexconsulting.com/wp-content/uploads/2023/11/IMARC-Presentation-31-Oct-2023.pdf) [V-P].

- **Global, all commodities, 2002–11 vs 2012–21:**

  | Measure | 2002–11 | 2012–21 |
  |---|---|---|
  | Spend | $172.7B | $196.2B |
  | Discoveries | 1,504 | 840 |
  | Tier 1 & 2 discoveries | 147 | 66 |
  | Value per dollar spent | **1.29** | **0.69** |

  MinEx: exploration "is (at best) a break-even proposition."
- **Cost per discovery** (moderate size or larger): **$65M (1975–2005) to $218M (2011–2020)**.
- **Tier 1 discoveries:** only 2–3 per year globally. They are about 3% of discoveries by count but about 45% of NPV.
- **Gold unit discovery cost:** about $23/oz in 2007 and about $45/oz in 2017 [V-P CSIRO]; about $62/oz GEO for 2009–18 vs $27/oz for 2000–09 [V-S].
- **EY (citing S&P):** "Since 2020, only six major gold discoveries have been made, with none occurring in 2023–24" [V-P] ([EY Top 10 risks 2026](https://www.ey.com/content/dam/ey-unified-site/ey-com/en-us/insights/mining-metals/documents/ey-gl-top-ten-business-risks-and-opportunities-10-2025.pdf)).

### 4.6 Exploration budgets (S&P Corporate Exploration Strategies) [V-snip]

- **2025:** nonferrous total **$12.4B** (−0.6%). Gold **$6.15B** (+11%). The grassroots share hit a record low of 18% ([S&P Nov 2025](https://www.spglobal.com/market-intelligence/en/news-insights/research/2025/11/ces-2025-overview-exploration-in-numbers)).
- **2026 preliminary (20 Oct 2026):** total **$16.5B (+33%)**. Gold **$8.5B (+38%)**. Copper **$4.3B (+32%)**, the highest since 2012. The full release is due 4 Nov 2026 ([S&P Oct 2026](https://www.spglobal.com/market-intelligence/en/news-insights/research/2026/10/global-nonferrous-exploration-budgets-growth-2026-gold-copper)).

### 4.7 Junior financing and how majors invest in juniors

- **TMX** [V-P] ([TSX mining](https://www.tsx.com/en/listings/listing-with-us/sector-and-product-profiles/mining)):
  - About **40% of the world's public mining companies** list on TSX/TSXV.
  - Over five years they raised **32% of global mining equity** (about $52B) across more than 6,400 financings.
  - In 2025: $16B raised; 54 new mining listings; $1.1T market cap.
- **Mechanisms majors use:**
  1. **Strategic equity stakes,** typically 9.9–19.9% with participation and top-up rights [UNV typical ranges]. Newmont is currently selling these stakes [V-P].
  2. **Earn-ins and option JVs:** the major funds exploration or studies to earn 51–80%. Barrick–GFG Pen (60%, then 70%) and Newmont–Awalé (51%, then 75%) are examples [V-S/V-P].
  3. **Generative or alliance programmes:** for example, VerAI monetises AI-generated targets through earn-in JVs and royalties [V-S].
  4. **Royalties and streams** as non-dilutive funding (Franco-Nevada, Wheaton, Royal Gold). Market size and terms were not verified [UNV].
  5. **Canadian flow-through shares and METC/CMETC:** mechanics not verified this session [UNV].
- **[INF]** Given 0.69 value per exploration dollar and about $218M per discovery, a major's exploration budget behaves like a **portfolio of real options**: many cheap earn-in options, staged funding, and kill or advance gates. FrankenMining's Bayesian prospectivity and capital-allocation layer should optimise over **deal structures**, meaning stake vs earn-in vs option vs own-drill. Single-project NPV is the wrong unit.

---

## 5. Software and data landscape (research question C)

### 5.1 Ownership map, Oct 2026

| Family | Products | Parent | Tag / source |
|---|---|---|---|
| Seequent | Leapfrog Geo/Edge, Central, Oasis montaj, Target, Imago, MX Deposit, **Evo** (2025 cloud platform: BlockSync, Driver ML) | **Bentley Systems** (closed 17 Jun 2021) | [V-P] [Bentley](https://investors.bentley.com/news-releases/news-release-details/bentley-systems-completes-acquisition-seequent-global-leader-3d/); [Evo](https://www.seequent.com/introducing-seequent-evo/) |
| Datamine | Studio RM, **Fusion** (drillhole, sample, QA/QC, LIMS database), Discover, Supervisor (Snowden), Reconcilor; Geovariances (2022) | **Constellation Software / Vela** (from CAE, 2015) | [V-S] [E&MJ](https://www.e-mj.com/leading-developments/cae-sells-datamine-to-constellation-software/); [Fusion docs](https://docs.dataminesoftware.com/Fusion/Latest/index.htm) [V-P] |
| Snowden Optiro | Supervisor (geostatistics: variography, kriging neighbourhood analysis, simulation, validation), consulting | Datamine / Constellation | [V-S]; [Supervisor docs](https://docs.dataminesoftware.com/Supervisor/9.2/index.htm) [V-P] |
| acQuire | GIM Suite (geoscience data management; v5.5 Nov 2025; claims more than 450 sites) | Vela / Constellation (2018) | [V-S] |
| GEOVIA | Surpac (geology and modelling), **Whittle** (pit optimisation: LG/Pseudoflow nested shells, strategic schedules), MineSched (tactical scheduling) | **Dassault Systèmes**, moving onto 3DEXPERIENCE | [V-P] [3DS](https://www.3ds.com/newsroom/press-releases/dassault-systemes-unveils-innovative-collaborative-mining-solutions-geovia-within-3dexperience-platform-pdac-2024) |
| Hexagon Mining | MinePlan, HxGN MineOperate (fleet and operations), MineProtect (collision avoidance, fatigue), MineEnterprise | **Hexagon AB**. The Octave spin-off completed 22 May 2026; whether Mining stayed in Hexagon is [UNV]. | [V-S]; Octave [V-P SEC](https://www.sec.gov/Archives/edgar/data/0002083632/000162828026040622/octv-20260331.htm) |
| Maptek | Vulcan, Evolution (multi-objective strategic scheduling), PointStudio, Sentry; **PETRA / MAXTA** (100% from Mar 2026; geomet and process digital twins; "agentic AI" MAXTA 6.0 due 2026) | Private (Adelaide) | [V-S] [GMR](https://www.globalminingreview.com/finance-business/05032026/maptek-acquires-petra-strengthening-orebody-intelligence-and-mine-value-chain-optimisation/) |
| Deswik | Design, scheduling, operations planning, mining data management | **Sandvik** (completed 4 Apr 2022) | [V-S] |
| Micromine | Origin (geology), Beyond (strategic planning), Alastri (scheduling), **Pitram** (underground fleet management and mine control) | **Weir Group** (30 Apr 2025, £624M EV; combined with Motion Metrics) | [V-S] [GMR](https://www.globalminingreview.com/mining/30042025/weir-completes-acquisition-of-micromine/) |
| RPMGlobal | XPAC scheduling, AMT asset management, XERAS financial modelling [product list UNV] | **Caterpillar** (A$5.00/share; court approval 3 Feb 2026) | [V-P] [RPM](https://rpmglobal.com/important-customer-update-caterpillar-completes-acquisition-of-rpmglobal/) |
| Modular Mining | DISPATCH FMS, Replenish, Predict, **Adapt** (AI fleet optimisation), ProVision guidance, MineCare | **Komatsu** | [V-P] [Komatsu](https://www.komatsu.com/en-br/technology/smart-mining/modular) |
| Komatsu FrontRunner | Surface AHS: 1,000th truck Apr 2026 (at NGM); more than 11.5B t hauled | Komatsu | [V-P] |
| Caterpillar MineStar | Fleet, Terrain, Detect, Health, **Command** (autonomy). 560+ autonomous trucks (CES 2023); current count [UNV] | Caterpillar | [V-S] [Cat](https://www.cat.com/en_US/news/machine-press-releases/caterpillar-surpasses-3-billio-tonnes-hauled-autonomously-by-cat-command-for-hauling-trucks.html) |
| Wenco | FMS; "open autonomy" (ISA-95); co-founded **OpenAutonomy.com** Sep 2025 | **Hitachi Construction Machinery** (2009) | [V-S] [Wenco](https://www.wencomine.com/news/wenco-openautonomy-international-mining-2025-09-11) |
| Sandvik AutoMine | Underground autonomy | Sandvik | [V-S] |
| Epiroc / RCT | Brand-agnostic underground automation | Epiroc (bought RCT 2022) | [V-P] [Epiroc](https://www.epiroc.com/en/products/digital-solutions/agnostic-automation) |
| IMDEX | IMDEXHUB-IQ (cloud drilling and sensor data), BLAST DOG, **Datarock** (core-image computer vision; 100% from Feb 2026) | IMDEX (ASX) | [V-P] [IMDEX](https://www.imdex.com/news-knowledge/news/imdex-completes-datarock-acquisition-accelerating-digital-earth-knowledge-expansion) |
| AVEVA PI System | Plant historian; claims "**9 of the top 10 mining companies**" | **Schneider Electric** (100% from Jan 2023) | [V-P vendor claim] [AVEVA](https://www.aveva.com/en/products/aveva-pi-system/) |
| ERP | SAP S/4HANA at **Barrick** (2020) and **Newmont** (Newcrest consolidation). Oracle use at either: no evidence [UNV] | SAP | [V-P] [SAP](https://news.sap.com/uk/?p=132589); [V-S] [SAPinsider](https://sapinsider.org/articles/newmonts-sap-roadmap-and-journey-achieve-one-consolidated-erp) |

**Known limitations (publicly stated):**
- **Proprietary formats and lock-in.** Deswik and Seequent both say so in their own marketing [V-S].
- **Block models are hard to exchange.** GMG's 2018 end-user survey flagged block-model interoperability as a critical pain point. GMG targets OMF v2 adoption across "30-odd" packages [V-S] ([IM](https://im-mining.com/?p=52518)).
- **Integrations are one-way.** For example, Datamine Fusion to Discover import is one-way [V-P].
- **Deterministic planning ignores geological uncertainty.** Stochastic optimisation claims about +10% NPV and +25% schedule value (Dimitrakopoulos/COSMO). That figure is advocacy-grade, and an independent study found 7.5% [V-S] ([CIM](https://magazine.cim.org/en/news/2023/a-stochastic-approach-to-mine-planning-en/)).
- **No authoritative public critique** of specific products (Leapfrog implicit modelling, DISPATCH and so on) was found [UNV]. Treat anecdotes as anecdotes.

### 5.2 AI-exploration startups: what they claim

| Company | Claim | Funding / evidence | Tag |
|---|---|---|---|
| **KoBold Metals** | AI-guided explorer turned owner and developer. Calls Mingomba (Zambia) the "largest high-grade copper discovery in a century." Targets more than 300 ktpa Cu in the early 2030s with capex above $2.3B. | Series C $537M at a $2.96B valuation (Jan 2025). No formal current resource estimate was found; 2026 start dates conflict. | [V-S] [Mining Weekly](https://www.miningweekly.com/article/gates-backed-kobold-metals-raises-537m-in-funding-round-2025-01-06); [mining.com](https://www.mining.com/gates-bezos-backed-kobold-says-zambia-copper-find-largest-in-a-century/) |
| **Earth AI** | AI targeting plus its own drilling. Claims a **75% discovery success rate**, "4x faster, up to 80% cheaper." | $20M Series B (Jan 2025). Success rate is self-reported and unaudited. | [V-S]; claim [UNV] |
| **VerAI** | AI asset generator: stakes 100%, then monetises through earn-in JVs and royalties; 60+ projects. | $24M Series B (Feb 2025, Insight Partners) | [V-S] |
| **GoldSpot Discoveries** | Renamed **EarthLabs** (Sep 2022). 2026 status [UNV]. | — | [V-S] |
| **Minerva Intelligence** | Knowledge-driven, explainable AI (semantic networks matched to deposit models); DRIVER multi-element zoning. | Current status [UNV] | [V-S] |
| **Fleet Space (ExoSphere)** | "Integrated subsurface decision platform": seismic-led multiphysics plus ML with quantified uncertainty. A secondary source lists Barrick as a user. | Series D US$100M at a US$525M valuation (Dec 2024) | [V-P] [OTPP](https://www.otpp.com/en-ca/about-us/news-and-insights/2024/fleet-space-closes-usd-100m-series-d-with-usd-525m-valuation); Barrick as user [V-S] |

**[INF]** The best-funded AI explorers own the assets. That suggests AI-for-exploration is hard to sell to majors as software alone. Value capture comes through equity in discoveries, and that aligns with the earn-in mechanics in §4.7.

### 5.3 Open standards

- **Open Mining Format (OMF), GMG** [V-P] ([GMG OMF](https://gmggroup.org/omf/); [omf-rust](https://github.com/gmggroup/omf-rust)):
  - **v2.0-beta** has a **Rust reference library**: crate 0.2.0-beta.1, MIT licence, minimum Rust 1.85, with C (`omf-c`) and Python bindings.
  - Elements: points, line segments, triangulated surfaces, grid surfaces, block models (regular or tensor, with sub-blocks), and composites. It reads and translates OMF v1.
  - Supported by Seequent, GEOVIA and Mira Geoscience. Maptek publishes `maptekomf`.
- **GeoSciML 4.1** (OGC 16-008r1, from OGC and CGI-IUGS): geological maps, boreholes, analyses. Lite, Basic and Extended profiles; builds on O&M (ISO 19156) [V-S] ([OGC](https://www.ogc.org/?p=2748)). The OGC Borehole Interoperability Experiment is ongoing [UNV outcome].
- **ISO 17757:2019:** safety of autonomous and semi-autonomous earth-moving and mining machines [V-S] ([ISO](https://www.iso.org/standard/76126.html)).
- **ISA-95:** enterprise–control integration. Wenco uses it for open autonomy [V-S].
- **GMG guidelines** [V-P] ([GMG library](https://gmggroup.org/category/library/guidelines/)):
  - Implementation of Autonomous Systems in Mining **v2** (Jul 2024).
  - Applying System Safety in Autonomous Mining (Jul 2024).
  - Sharing Open Data Sets in Mining (Dec 2023).
  - Functional Safety for Autonomous Systems (Aug 2020).
  - Time Usage Model and KPIs for surface mobile equipment (Aug 2020).
  - **Foundations of AI in Mining** white paper v2.
  - Safe mixed-fleet underground framework.
  - Electric equipment KPIs.
  - Geometallurgy data reporting.
  - Working groups: Data & AI, Autonomous Mining, Cybersecurity.
  - An underground short-interval-control guideline (2019) [V-S].

---

## 6. Pain points and failure patterns (research question D)

### 6.1 Mine-to-mill reconciliation

- **Factors** (AusIMM convention) [V-S] ([AusIMM APCOM 2011](https://ausimm.com/publications/conference-proceedings/35th-apcom-symposium-2011/reconciliation-principles-for-the-mining-industry)):
  - **F1** = grade-control model ÷ depleted reserve model.
  - **F2** = mill received ÷ mine delivered.
  - **F3** = F1 × F2.
  - "Mine call" and "mill call" factors are used inconsistently from site to site.
- **Error sources:** blast movement, dilution, stockpiles, density and moisture, sampling. One gold case had 18 months of poor reconciliation caused by sampling.
- **A 2022 AusIMM abstract** argues that standard F2 misstates tonnes and grade because in-situ models ignore blast swell, and proposes a post-blast F2′ [V-S] ([AusIMM 2022](https://www.ausimm.com/globalassets/conferences-and-events/international-mining-geology/mg-2022/abstract-96---a-novel-innovation-for-grade-control.pdf)).
- **No published industry benchmark** for typical F1/F2/F3 tolerance bands was found. The ±5–10% figure is practitioner lore [UNV].
- **In current disclosures:** both majors' 2026 results repeatedly explain AISC movements through "lower grades processed" and "mine sequencing" [V-P]. That is the language of plan-vs-actual grade variance.

### 6.2 Megaproject cost and schedule

- **McKinsey, "The capex crystal ball"** [V-snip]: 83% of recent major mining and metals projects had cost or schedule problems, with capex overruns above 40% and delays of 20–30%. A separate study of 58 projects found a 37% average overrun and 1.2 years of delay ([McKinsey](https://www.mckinsey.com/industries/metals-and-mining/our-insights/the-capex-crystal-ball-beating-the-odds-in-mining-project-delivery)).
- **EDC, 78 projects:** 37% average overrun; copper projects 40% [V via FactSet].
- **Oyu Tolgoi underground:** about $7.06B (Jun 2022 reforecast) vs about $5.3B originally. Shafts were 15 months late [V-P] ([Rio 6-K](https://www.sec.gov/Archives/edgar/data/863064/000162828022019482/rio-20220630_d2.htm)).
- **Newmont Tanami Expansion 2:** about $395M (2022) to $1.7–1.8B; early 2025 to H1 2027. Scope changes are not separated out [V-P/V-S; INF on ratio].
- **Barrick Reko Diq:** now warns of "potential significant increases to previously disclosed total estimated capital budget and timeline" [V-P].

### 6.3 Write-downs

- **Barrick Pascua-Lama (2013)** [V-P] ([SEC](https://www.sec.gov/Archives/edgar/data/0000756894/000119312514123835/d693534dex993.htm)):
  - $5.2B in Q2 for "regulatory challenges… schedule delays and associated capital expenditure increase," plus $896M in Q4.
  - About $6.1B for the year. Total Barrick impairments in 2013 were $12.7B.
  - A further $429M followed in Q4 2017 after the Chilean regulator SMA ordered closure [V-snip].
- **Newmont:** held-for-sale write-downs in 2024 (Q1 2024 $485M; Q4 2024 $268M loss on held for sale) [V-snip]. No impairment above $1B in 2025 was found [UNV].

### 6.4 Data silos and integration: the industry's own words

Source: [EY, Top 10 risks for 2026](https://www.ey.com/content/dam/ey-unified-site/ey-com/en-us/insights/mining-metals/documents/ey-gl-top-ten-business-risks-and-opportunities-10-2025.pdf) [V-P].

- **The ranking:**
  1. Operational complexity (new #1)
  2. Costs and productivity
  3. Capital
  4. Reserve depletion
  5. Licence to operate
  6. Workforce
  7. Geopolitics
  8. Digital and innovation
  9. Sustainability
  10. Changing business models
- **Quotes worth reusing:**
  - "Some reports suggest **spatial compliance to optimal schedules is as low as 30%**."
  - "Functional silos undermine schedule predictability and asset productivity."
  - "The integration gap: a siloed operating model, with little integration between operations and maintenance."
  - "**Data maturity gap**: the sector is quick to adopt technology but slow to become truly digital. Without trusted data, digital transformation gains are episodic and fragile."
  - Value will come from "an end-to-end approach that leverages a **unified data and AI backbone**."
  - AI to date is mostly "proof of concepts… rather than scalable solutions."
- **Avathon's announcement for Barrick NA** opens with the same diagnosis: "Mining has historically been organised around individual functions, systems, and processes" [V-S].

### 6.5 Tailings failures and GISTM

| Event | Date | Facts | Tag |
|---|---|---|---|
| **Mount Polley** (Imperial Metals, B.C.) | 4 Aug 2014 | No deaths. About 10 Mm³ water plus 4.5 Mm³ tailings (government/IRP); Imperial said about 25 Mm³ in total. Engineers disciplined about 8 years later. | [V-S] [IRP](https://www.mountpolleyreviewpanel.ca/sites/default/files/IRP00001_2014-10-07_IRP_Update%20on%20Mt%20Polley%20Failure%20Presentation.pdf); [Global News](https://globalnews.ca/news/8679942/engineers-disciplined-eight-years-after-mount-polley-mine-disaster) |
| **Fundão / Mariana** (Samarco, Vale 50% / BHP 50%) | 5 Nov 2015 | 19 deaths [V-S]. About 40 Mm³ released [UNV]. **Settlement about R$170B** at 100%: R$38B already spent, R$100B to governments over 20 years, R$32B Samarco obligations. Signed 25 Oct 2024; ratified by the Supreme Court 6 Nov 2024. UK, Australian and Dutch claims are outside it. | [V-P] [Vale 6-K](https://www.sec.gov/Archives/edgar/data/917851/000129281424003801/vale20241018_6k.htm); [V-S] [Northern Miner](https://secure.northernminer.com/news/bhp-vale-and-samarco-reach-30-billion-fundao-dam-settlement/1003872632) |
| **Brumadinho** (Vale, Córrego do Feijão) | 25 Jan 2019 | About 270 deaths; about 12 Mm³. **R$37.7B** (about US$7B) settlement, 4 Feb 2021. | [V-S] [Al Jazeera](https://www.aljazeera.com/economy/2021/2/4/vale-reaches-7bn-settlement-over-deadly-dam-collapse-in-brazil) |

- **GISTM** [V-P] ([ICMM](https://www.icmm.com/our-principles/conformance-protocols-gistm)):
  - Launched **Aug 2020** by ICMM, UNEP and PRI.
  - **15 principles** [V-S] and **77 requirements**, mapped to **219** conformance-protocol criteria.
  - ICMM members committed to conformance by **Aug 2023** for Extreme/Very High facilities and **Aug 2025** for all others.
  - Self-assessment is the minimum; third-party validation is expected "as soon as reasonably practicable."
  - The Global Tailings Management Institute provides assurance.
- **Status at the two majors:** see §1.8 (Barrick) and §2.8 (Newmont).
- **[INF]** GISTM is a requirements-traceability problem: 77 requirements × dozens of facilities × evidence, accountable-executive sign-off and independent review. It is a natural, bounded, high-trust first module that both companies must disclose against every August.

### 6.6 Megaproject and jurisdiction risk in 2025–26 (Barrick examples) [V-P]

- **Mali:** retrospective application of a new mining code, detention of staff, seizure of gold, and provisional administration. Resolved Nov–Dec 2025 at a reported cost of about $430M plus ongoing higher royalties.
- **Pakistan (Reko Diq):** security escalation. Development slowed with more than $1B already spent; GISTM disclosure suspended.
- **[INF]** For a capital-allocation engine, **political, security and fiscal-regime risk** must be modelled as first-class stochastic variables rather than folded into the discount rate. Events of this kind feature prominently in Barrick's 2025–26 disclosures.

### 6.7 Social licence incidents

- **Porgera (PNG)** [V-P/V-S]:
  - Care and maintenance from **25 Apr 2020** after the lease was not renewed.
  - Mar 2023 agreement created **New Porgera**: 51% PNG stakeholders; economic benefits split 53/47 with Barrick Niugini ([Barrick](https://www.barrick.com/English/news/news-details/2023/barrick-steers-porgera-gold-mine-back-towards-world-class-production/default.aspx)).
  - Restart Dec 2023 [V-S].
  - Tribal violence in Sep 2024 killed at least 20 [V-S] ([Mining Weekly](https://www.miningweekly.com/article/barrick-suspends-papua-new-guinea-mine-after-deadly-clashes-2024-09-19)).
  - Now Barrick 24.5% equity-accounted, with 2026 guidance of 80–100 koz attributable [V-P].
- **North Mara (Tanzania):** legacy community and security-related claims have been litigated in the UK and Ontario [V-S] ([RAID](https://raid-uk.org/?p=3474)).
- **Peñasquito (Newmont):** strike Jun–Oct 2023 over raising profit-sharing from 10% to 20%; no Q3 2023 production; about $1M/day in standby costs plus about $2.7M/day in lost revenue [V-snip] ([10-Q](https://www.sec.gov/Archives/edgar/data/1164727/000116472723000039/nem-20230930.htm)).
- **Cobre Panamá (First Quantum, industry example):** closed since late 2023 after the Supreme Court ruling. On 1 Oct 2026 a ministerial commission recommended talks on operation without expansion to fund closure [V-S].
- **Pueblo Viejo resettlement:** 95% package acceptance; 632 homes; 570 families resettled [V-P Q2 2026].

### 6.8 What Barrick and Newmont say about digital transformation results

- **Quantified results disclosed:** essentially only the autonomous-haulage cases.
  - Newmont Boddington: $150M investment; IRR above 35% [V-S]; 41 autonomous trucks now [V-P].
  - Barrick NGM: about $50M at Cortez and about $65M at Carlin, with no benefit figures disclosed yet [V-P].
- **No quantified AI, digital-twin or data-platform benefits** appear in either company's 2026 filings that I read [V-P, absence checked].
- **Avathon at Barrick NA:** no KPIs [V-S].
- **[INF]** The bar for credibility is specific. Executives have seen PoCs (EY: "proof of concepts… rather than scalable solutions") and autonomy business cases with IRR. FrankenMining's outputs should be stated in **the metrics the companies already publish**: AISC/oz, GEO, FCF/share, ROCE, reserve replacement, critical-control verification rate. Generic "AI transformation" claims are not enough.

---

## 7. Implications for FrankenMining (all [INF])

1. **Use the published metrics.** Every module should state which published metric it informs (§3):
   - Barrick: AISC (WGC basis), attributable GEO, LTIFR, critical-control verifications, ROCE, three-year reserve replacement, rTSR vs GDX.
   - Newmont: CSC/GEO, adjusted FCF, SPEFR, ROCE.
2. **Coexisting with Avathon.** Barrick's North American business, the IPO vehicle, has publicly chosen a "knowledge-graph + agents" operating-model platform. Options:
   - **(a) Complement:** position FrankenMining as the deterministic, auditable *system of record and computation kernel*. That covers reserves and resource lineage, WGC-compliant cost accounting, GISTM evidence, capital-allocation optimisation, and OMF-native geodata. Avathon-style agents could read from it.
   - **(b) Start outside the NA perimeter:** Africa & Middle East (Loulo-Gounkoto, Kibali, North Mara, Bulyanhulu), LatAm/APAC, and copper (Lumwana, Reko Diq), where no AI-platform partner has been announced.
   - **(c) Focus on capital allocation** across the post-IPO parent. Exploration, earn-ins and Tier One screening are not in Avathon's six announced use cases.
   - Do **not** propose replacing or duplicating the platform.
3. **Stay off the carve-out's critical path.** The NA Barrick IPO (by end-2026) requires separation of SAP, financial reporting, reserves reporting and data for a newly listed SEC registrant. FrankenMining should offer carve-out evidence only if asked.
4. **Capital allocation must model what actually moved capital in 2025–26:**
   - fiscal-regime shocks (Mali 2023 Code);
   - security (Reko Diq);
   - portfolio structure (the IPO);
   - JV governance (the NGM dispute and $1.95B settlement);
   - build vs buy at parity (FactSet).
   - Geology and NPV alone are insufficient.
5. **Exploration as an options portfolio.** Optimise over deal structures (earn-in, stake, option, royalty) and stage-gates. Use MinEx base rates (2–3 Tier-1 discoveries a year globally; $218M per discovery; 15.7–17.9 years to production) as priors for Bayesian prospectivity.
6. **Adopt OMF v2 via `omf-rust` from day one.** It is the industry's open 3D interchange format, is MIT-licensed, and its reference implementation is already Rust. Also support GeoSciML for borehole and geology exchange, and ISA-95 for operations integration.
7. **Co-exist with the OEM tool stack.** Planning and fleet software now belongs to OEMs: Cat (MineStar, RPM), Komatsu (Modular, FrontRunner), Sandvik (Deswik, AutoMine), Hitachi (Wenco), Weir (Micromine). FrankenMining should be the vendor-neutral layer above them, connecting through DISPATCH, MineStar, Pitram, PI and SAP adapters, not replacing them in v1. NGM already runs Komatsu AHS with a new FMS, Pitram underground, and SAP.
8. **Early high-trust modules:**
   - (i) GISTM requirements traceability and evidence;
   - (ii) critical-control verification and close-out;
   - (iii) WGC AISC/AIC and reserves reconciliation, with reproducible metric definitions that render the 190% and 504% reserve-replacement bases and the $1,423 and $1,637 AISC bases from one cited source;
   - (iv) mine-to-mill F1/F2/F3 reconciliation with uncertainty.

---

## 8. Glossary of metrics used by these executives

| Term | Definition | Source |
|---|---|---|
| AISC | WGC all-in sustaining cost (§4.1) | [V-P] WGC |
| AIC | AISC plus non-sustaining capex, exploration, etc. | [V-S] |
| TCC | Total cash costs (Barrick, gold) | [V-P] |
| C1 | Copper cash cost (Barrick) | [V-P] |
| CSC/GEO | Newmont adjusted cash sustaining costs per gold-equivalent ounce (comp metric) | [V-P] |
| GEO | Gold-equivalent ounces: production × price ratio | [V-P] |
| TRIFR | (reportable injuries, including fatalities, LTIs, restricted-duty and medically treated) × 1,000,000 ÷ hours worked (Barrick) | [V-P] Circular |
| LTIFR | Lost-time injury frequency rate | [V-P] |
| SPEFR | Newmont Significant Potential Event Frequency Rate | [V-P] |
| ROCE | Barrick: Adjusted EBIT ÷ average capital employed, where capital employed is total assets excluding cash (adjusted for CIP) minus liabilities excluding debt | [V-P] Circular |

---

## 9. UNVERIFIED and open items (check before executive use)

1. **Reko Diq's pre-review plan:** Phase 1 capex about $5.6B, first production about end-2028, Phase 2 about 90 Mtpa, and the ownership split. This is from recall of the Feb 2025 technical report.
2. **Mali settlement payment** of about $430M: Bloomberg, via secondary sources. Barrick's releases do not state an amount.
3. **Avathon's former identity** (SparkCognition?).
4. **Who will lead the remaining Barrick parent** after the current CEO moves to North American Barrick. Not disclosed in the sources read.
5. **The nature of the NGM disputes** settled on 10 Aug 2026.
6. **Basis of Barrick's compensation AISC** ($1,423/oz vs $1,637/oz reported) and of reserve replacement (504% vs about 190%).
7. **Kibali operatorship, and the Veladero, Zaldívar and Jabal Sayid partner names**: from recall.
8. **FY2025 industry-average AISC** ($1,552 vs $1,709/oz) and proprietary cost-curve positions.
9. **Current Cat autonomous truck count;** whether Hexagon Mining stayed in Hexagon after the Octave spin-off; the Deswik price; RPMGlobal's product list including XERAS.
10. **Which FMS NGM moved to at Cortez** (likely Komatsu/Modular).
11. **Status of the Newmont–Caterpillar battery-electric alliance;** Newmont "Full Potential" results.
12. **Typical F1/F2/F3 tolerance bands;** the Fundão release volume; the Cobre Panamá ruling date (believed 28 Nov 2023).
13. **Flow-through/METC mechanics;** royalty and stream market size; typical strategic-stake percentages.
14. **Independent validation of AI-startup discovery claims** (Earth AI's 75%, KoBold's Mingomba scale). None was found.
15. **Barrick Q3 2026 results** (due 9 Nov 2026) will supersede the Q2 figures in §1. A German blog's pre-release Q3 numbers were ignored as unreliable.

---

## 10. Source register (all accessed 2026-10-28)

**Barrick: primary**
- Q2 2026 results release (10 Aug 2026): https://www.sec.gov/Archives/edgar/data/756894/000119312526343857/d67611dex991.htm
- Q2 2026 MD&A: https://www.sec.gov/Archives/edgar/data/756894/000119312526343857/d67611dex992.htm
- Q1 2026 MD&A (8 May 2026): https://www.sec.gov/Archives/edgar/data/756894/000119312526216613/d145525dex992.htm
- Q4/FY2025 results release (5 Feb 2026): https://www.sec.gov/Archives/edgar/data/756894/000119312526039501/d10712dex991.htm
- 2026 Management Information Circular (27 Mar 2026): https://www.sec.gov/Archives/edgar/data/756894/000119312526148005/d858698dex991.htm
- 6-K, NGM Second A&R LLC Agreement (10 Aug 2026): https://www.sec.gov/Archives/edgar/data/756894/000119312526353144/d182096d6k.htm
- EDGAR filing index: https://www.sec.gov/cgi-bin/browse-edgar?action=getcompany&CIK=0000756894
- Tailings disclosures: https://www.barrick.com/tailings-disclosures/
- NGM–Komatsu release (Jul 2025): https://s25.q4cdn.com/322814910/files/doc_news/2025/07/NGM-and-Komatsu-Launch-First-of-Its-Kind-Autonomous-Haulage-Partnership_Final.pdf
- Pascua-Lama 2013 impairment exhibit: https://www.sec.gov/Archives/edgar/data/0000756894/000119312514123835/d693534dex993.htm
- Porgera 2023: https://www.barrick.com/English/news/news-details/2023/barrick-steers-porgera-gold-mine-back-towards-world-class-production/default.aspx

**Barrick: secondary**
- Name change: https://www.engineeringnews.co.za/article/barrick-completes-name-change-elects-directors-2025-05-07
- Leadership transition (Sep 2025): https://www.kitco.com/news/off-the-wire/2025-09-29/barrick-names-operations-chief-interim-ceo-after-sudden-bristow-exit
- Mali deal: https://www.mining.com/barrick-shares-surge-as-it-resolves-dispute-with-mali/ ; https://www.bnnbloomberg.ca/business/2025/11/24/barrick-mining-reaches-deal-to-settle-dispute-with-government-in-mali/
- IPO evaluation (Dec 2025): https://www.mining.com/barrick-eyes-ipo-of-north-american-gold-assets/ ; https://www.globalminingreview.com/mining/03122025/barrick-evaluating-ipo-of-its-north-american-gold-assets/
- Avathon partnership: https://www.globalminingreview.com/mining/25092026/avathon-selected-to-power-an-ai-native-mining-operating-model-for-barricks-north-american-business/
- Q3 2026 results date: https://www.barrick.com/English/news/news-details/2026/barrick-to-report-third-quarter-2026-results-on-november-9/default.aspx

**Newmont: primary**
- Q3 2026 results (22 Oct 2026): https://www.sec.gov/Archives/edgar/data/0001164727/000116472726000041/newmontq32026earningsrelea.htm
- Q4/FY2025 results and capital allocation framework (19 Feb 2026): https://www.sec.gov/Archives/edgar/data/1164727/000116472726000009/newmontq42025earningsand20.htm
- Q1 2026 results: https://www.sec.gov/Archives/edgar/data/0001164727/000116472726000017/newmontq12026earningsrelea.htm
- FY2025 10-K: https://www.sec.gov/Archives/edgar/data/1164727/000116472726000010/nem-20251231.htm
- 2026 Proxy Statement: https://www.sec.gov/Archives/edgar/data/1164727/000110465926035211/nem-20260512xdef14a.htm
- Executive appointments (15 Jun 2026): https://www.sec.gov/Archives/edgar/data/1164727/000110465926074137/tm2617970d1_ex99-1.htm
- CTO retirement 8-K (28 Apr 2026): https://www.sec.gov/Archives/edgar/data/1164727/000110465926050355/tm2612965d1_8k.htm
- GISTM letter to ICMM (Aug 2025): https://s24.q4cdn.com/382246808/files/doc_downloads/2025/08/Newmont-GISTM-ICMM-2025.pdf
- Cat/Boddington: https://www.cat.com/en_US/news/machine-press-releases/newmonts-boddington-mine-safely-hauls-more-than-100-million-tonnes-of-material-autonomously-using-cat-minestar-command-for-hauling.html

**Industry, standards, vendors:** the URLs are inline in §§4–6. Key ones:
- WGC AISC guidance: https://www.gold.org/goldhub/research/guidance-note-non-gaap-metrics-%e2%80%93-all-sustaining-costs-and-all-costs
- WGC GDT Q2 2026: https://www.gold.org/goldhub/research/gold-demand-trends/gold-demand-trends-q2-2026/supply
- MinEx IMARC 2023: https://minexconsulting.com/wp-content/uploads/2023/11/IMARC-Presentation-31-Oct-2023.pdf
- EY Top 10 risks 2026: https://www.ey.com/content/dam/ey-unified-site/ey-com/en-us/insights/mining-metals/documents/ey-gl-top-ten-business-risks-and-opportunities-10-2025.pdf
- FactSet copper capex: https://insight.factset.com/metals-mining-copper-capex-watch
- ICMM GISTM: https://www.icmm.com/our-principles/conformance-protocols-gistm
- GMG OMF: https://gmggroup.org/omf/ ; https://github.com/gmggroup/omf-rust
- GMG guidelines: https://gmggroup.org/category/library/guidelines/
- Komatsu 1,000th AHS truck: https://www.komatsu.jp/en/newsroom/2026/20260422
- RPMGlobal/Caterpillar: https://rpmglobal.com/important-customer-update-caterpillar-completes-acquisition-of-rpmglobal/
- IMDEX/Datarock: https://www.imdex.com/news-knowledge/news/imdex-completes-datarock-acquisition-accelerating-digital-earth-knowledge-expansion
- Seequent Evo: https://www.seequent.com/introducing-seequent-evo/
- AVEVA PI: https://www.aveva.com/en/products/aveva-pi-system/
- SAP–Barrick: https://news.sap.com/uk/?p=132589
- Vale Fundão settlement 6-K: https://www.sec.gov/Archives/edgar/data/917851/000129281424003801/vale20241018_6k.htm
