# W5: Primary-source verification of the plan's Barrick and industry figures

RESULT_STATUS: COMPLETE. There are 72 claims: 48 VERIFIED, 2 CORRECTED, 15 PARTLY, 4 SECONDARY-ONLY and 2 UNVERIFIABLE; one further claim (#19) was withdrawn as having no design use. Nothing has been edited in the plan or in `MOR_CASE.md`. The patches in §3 and the register rows in §4 are proposals only.

- **Verified on:** 2026-10-28/29.
- **Snapshot verified:** `COMPREHENSIVE_PLAN_FOR_THE_DESIGN_OF_FRANKENMINING.md` at **8,262 lines**. All 25 Find strings in §3 were re-checked against it after this file was written; each matches exactly once.
  - The plan was edited by someone else while this pass ran. It grew from 8,174 lines, and line 217 gained new Loulo-Gounkoto and Tongon text.
  - All line numbers below refer to the 8,262-line version. Before pasting a patch, re-locate it by its quoted "Find" text.
- **Primary documents** were downloaded to `/private/tmp/claude-501/-Users-jemanuel-projects-franken-mining/2ba72fbb-c45d-4fd8-93cf-c08cb26d7a18/scratchpad/w5/` (folders `docs/` and `dl/`). They include SEC filings, PDFs and the HTML of vendor pages.
  - SEC HTML was converted to text with a stdlib parser. PDFs were converted with `pdftotext -layout`.
  - Quotes from those documents were taken with grep, not paraphrased.
  - Quotes obtained with WebFetch are marked "(WF)". They passed through a summarising fetcher, so treat their punctuation as approximate.
- **Pages that could not be reached:**
  - spglobal.com: HTTP 403.
  - tandfonline and researchgate: HTTP 403. The Bertisen & Davis abstract was read through OpenAlex instead.
  - mckinsey.com Freeport/TROI article: timed out.
  - globaltailingsreview.org: unreachable. GISTM text was read through ICMM and through Newmont's verbatim requirement table.
  - The Weir newsroom.
  - Mining Weekly: HTTP 403.

**Verdict key.**
- VERIFIED: matches a primary source.
- CORRECTED: the primary source gives a different value.
- PARTLY: the figure is right but the wording, basis or label overstates it.
- SECONDARY-ONLY: only trade press, consultant material or search snippets were found.
- UNVERIFIABLE: no source was found.
- WITHDRAWN: removed from the register; no design use.

---

## 1. Headline findings for the executive deck

1. **Tanami 2 "rose from $395M to $1.7–1.8B" is wrong (CORRECTED).**
   - The $395M was development *spend to 30 Jun 2022*, not an estimate.
   - The capex estimate was $850–950M in Feb 2022, with production in 2024. It is now $1.7–1.8B (Feb 2026), with production in H1 2027.
   - That is roughly a 1.8–2.1× rise, not 4.5×.
2. **The 190% vs 504% reserve-replacement question is answered by primary sources (CORRECTED/resolved).**
   - Both figures cover the same three years (2023–25), exclude acquisitions and divestments, and divide additions by depletion.
   - 190% is in **gold ounces**. 504% is in **gold-equivalent ounces**: 87 Moz ÷ 17.2 Moz.
   - Both figures appear in the **same** 2026 Circular. The plan's candidate bridge ("190% = five-year 2020–24") does not describe the results-release figure.
3. **Barrick's scorecard GEO uses fixed reserve-price ratios**: $1,500/oz Au and $3.25/lb Cu (Circular, scorecard note 5).
   - GB-27's flip condition is therefore met. In Barrick's API the gold price does not move GEO.
4. **The North American IPO target has softened (PARTLY).**
   - The company's official target is still end-2026 (Q2 release, 10 Aug 2026).
   - On 29 Sep 2026 the CEO said it "may slide into January" (trade press).
5. **Newmont's default notice of 3 Feb 2026 is now primary-sourced** in Newmont's FY2025 10-K. The register row can move from V-S to FACT.
6. **Several labels overstate their evidence (PARTLY):**
   - Bond et al. reports 21% of "participants", not of experts.
   - McKinsey's 83% / >40% applies to all major projects, not megaprojects.
   - The FactSet parity applies 40% to the *greenfield* median of $18,200, not $18,048.
   - Lihir's twin *does* claim benefits, but without numbers.
   - Cortez's 23 trucks were retrofitted, with rollout planned for Q3 2026. They were not confirmed "live".
   - "Modular DISPATCH" at NGM is unverified.
7. **Two figures have no primary source:**
   - Freeport's TROI uplift is SECONDARY-ONLY.
   - "1.2-year mean across 58 projects" is UNVERIFIABLE.
   - Plan line 2667 should cite Freeport's own 10-K instead (214 Mlb of incremental copper from leach analytics in each of 2024 and 2025).

---

## 2. Claim table

| # | Plan line (8,262-line snapshot) | Claim as written | Verdict | Primary value | Source URL (date) | Quote (≤ 25 words) |
|---|---|---|---|---|---|---|
| 1 | 212, 2302 (E026) | Spatial schedule compliance "reported as low as 30%" | VERIFIED | EY's own wording ("some reports suggest"); EY does not name the underlying reports. The plan's "lower anecdote" framing at 2302 is right. | https://www.ey.com/content/dam/ey-unified-site/ey-com/en-us/insights/mining-metals/documents/ey-gl-top-ten-business-risks-and-opportunities-10-2025.pdf (Oct 2025, p.5) | "Some reports suggest spatial compliance to optimal schedules is as low as 30%" |
| 2 | 216 (E025) | "The same pay metric appears on different bases in different documents" | PARTLY | The different bases appear in the **same** document. The 2026 Circular gives reserve replacement as ~190% (gold) and 504% (GEO), and 2025 gold AISC as $1,423 (scorecard) and $1,637 (non-GAAP table). | https://www.sec.gov/Archives/edgar/data/756894/000119312526148005/d858698dex991.htm (27 Mar 2026; pp.41–42, 45, 85) | "replacing 504% of attributable gold equivalent ounces(3)(4) net of depletion at a higher grade" |
| 3 | 217 | Capex overruns average +14%, about half outside ±15% (Bertisen & Davis) | VERIFIED | 63 projects; mean +14% vs bankable feasibility study; about half outside ±15% even after allowing for bias. The 2007 SME conference version reports 25% (OneMine abstract). | https://api.openalex.org/works/doi:10.1080/00137910802058533 (abstract of *Eng. Economist* 53(2):118–139, 2008) | "as-built capital costs are, on average, 14% higher than as estimated in the bankable feasibility study" |
| 4 | 217 | "…with no institutional learning" | PARTLY | Bias shows "little attenuation over time", but the authors argue it is intentional and rational, driven by financing scarcity. | same as #3 | "We argue that this persistence of bias is instead intentional and rational, driven by a scarcity of project financing" |
| 5 | 217 (new text) | Loulo-Gounkoto: control regained 16 Dec 2025; H1 2026 152 koz (Q2 88 koz); "$585/oz Q2 royalty ($629/oz H1 2026; TCC $1,897/oz, AISC $2,240/oz…)" | PARTLY | Date and production VERIFIED (Barrick 80% table). **TCC $1,897 and AISC $2,240 are Q2-only**; H1 2026 is TCC $1,906 and AISC $2,104. Royalty per ounce comes from Mine Stats xlsx and was not re-verified. | https://www.sec.gov/Archives/edgar/data/756894/000119312526343857/d67611dex992.htm (Q2 2026 MD&A, Loulo-Gounkoto table); https://www.sec.gov/Archives/edgar/data/756894/000119312526216613/d145525dex992.htm (Q1 2026 MD&A) | "The provisional administration of the Loulo-Gounkoto complex was terminated on December 16, 2025" |
| 6 | 217 (new text) | Tongon divestiture closed 1 Dec 2025, "$305M to Atlantic Group" | PARTLY | "Up to $305M" (consideration includes contingent elements); closing date VERIFIED. | Q1 2026 MD&A (as #5) | "to the Atlantic Group for total consideration of up to $305 million. The transaction closed on December 1, 2025." |
| 7 | 997 (E026) | ~$218M exploration spend per discovery (2011–20) | VERIFIED | $218M in 2023 US$ for deposits of "Moderate" size or larger; $65M in 1975–2005. | https://minexconsulting.com/wp-content/uploads/2023/11/IMARC-Presentation-31-Oct-2023.pdf (31 Oct 2023, slide 13) | "Between 2011-2020 the average discovery cost tripled to $218m" |
| 8 | 998 | ~0.69 of value per exploration dollar | VERIFIED | $196b spent, $135b of value (2012–21). | same, slide 30 | "generated $135b of value … giving a "bang-per-buck" of 0.69" |
| 9 | 999 | 2–3 Tier-1 discoveries per year globally | VERIFIED | MinEx Tier 1 is >20 years, >200 ktpa Cu or >250 koz/y Au. This is not Barrick's definition. | same, slide 18 | "Tier-1 discoveries are rare … only 2-3 per year" |
| 10 | 1000, 4509, 4620 (E026; MOR "~16–18 years") | 15.7–17.9 years from discovery to production | SECONDARY-ONLY | S&P pages return HTTP 403. The figures come from two different samples: 15.7 years for 127 mines started 2002–23, and 17.9 years for mines started 2020–23. S&P later gave 17.8 years for 2020–24. This is **not** a MinEx figure; MinEx/CSIRO gives about 13 years. | https://www.spglobal.com/marketintelligence/en/news-insights/research/discovery-to-production-averages-15-7-years-for-127-mines ; …/average-lead-time-almost-18-years-for-mines-started-in-2020-23 (S&P; read via search snippets) | Article titles: "Discovery to production averages 15.7 years for 127 mines"; "Average lead time almost 18 years for mines started in 2020-23" |
| 11 | 2075 (E026) | "expert interpretation accuracy of about 21% in Bond et al. 2007" | PARTLY | 21% of 412 **participants** (mixed cohort) identified the correct tectonic setting of one synthetic seismic image. | https://doi.org/10.1130/gsat01711a.1 (GSA Today 17(1), 2007; abstract via OpenAlex) | "Only 21% of the participants interpreted the "correct" tectonic setting of the original model" |
| 12 | 2220 | "+16% industry AISC rise in Q1 2026 was driven by royalties and corporate G&A" | VERIFIED | WGC industry average $1,785/oz, +5% q/q, +16% y/y. This is an **industry** figure. Barrick's own Q1 2026 AISC moves were driven mainly by sustaining capex (Q1 2026 MD&A). | https://www.gold.org/goldhub/research/gold-demand-trends/gold-demand-trends-q2-2026/supply (30 Jul 2026) (WF) | "This was underpinned by a rise in royalties and corporate G&A expenses." |
| 13 | 2458 | "Cortez: 23 autonomous trucks live" | PARTLY | 23 retrofits were completed in May 2026 and systems were in test. A Q3 2026 site-wide rollout was *planned*. Barrick reports Q3 on 9 Nov 2026. | Q2 2026 MD&A (as #5), "Autonomous Haulage Program" | "At Cortez, the retrofit of 23 haul trucks was completed in May 2026." "Systems are being tested ahead of a Q3 2026 site-wide rollout." |
| 14 | 2458 | "Modular DISPATCH" at NGM | UNVERIFIABLE | The MD&A names a "new Fleet Management System" but not its vendor. No Barrick, NGM or Komatsu source names DISPATCH at NGM. Searched: Q1/Q2 2026 MD&A, NGM–Komatsu release, Komatsu 1,000th-truck release. | Q2 2026 MD&A | "The transition to the new Fleet Management System for the primary production fleet has been completed." |
| 15 | 2458–2459, 3530 | Carlin: 40 retrofits from Q2 2026; staggered Cortez/Carlin rollouts | VERIFIED | Carlin autonomous operations planned for Q1 2027; program spend about $65M (100%) through 2027. | Q2 2026 MD&A | "At Carlin, we plan to undertake 40 truck retrofits with the work having commenced in Q2 2026." |
| 16 | 2460 | Boddington: 41 autonomous trucks | VERIFIED | The count comes from the 10-K. The 10-K does not name the vendor. Cat MineStar Command attribution comes from Cat's 2022 release; that page timed out and was not re-read. | https://www.sec.gov/Archives/edgar/data/1164727/000116472726000010/nem-20251231.htm (FY2025 10-K, Properties, Boddington) | "with a fleet of 41 production autonomous haulage trucks" |
| 17 | 2461 (E024) | Micromine Pitram underground at NGM Goldrush | SECONDARY-ONLY | A 2023 vendor-sourced trade article. No Barrick filing names Pitram. | https://www.globalminingreview.com/technology-digitalisation/04072023/worlds-largest-gold-mining-complex-deploys-micromine-fms/ (4 Jul 2023) (WF) | "Micromine has successfully implemented its innovative fleet management system (FMS) and mine control solution" |
| 18 | 2465, 6479, 6738 (E024) | Avathon is Barrick North America's strategic tech partner | VERIFIED | The release is Avathon-issued, via PR Newswire, Pleasanton, **23 Sep 2026**. R6's "25 Sep" is the date of the GMR reprint. No fees, KPIs or term were disclosed. | https://www.stocktitan.net/news/B/avathon-selected-to-power-an-ai-native-mining-operating-model-for-73hh6u7orvof.html (PR Newswire, 23 Sep 2026) (WF) | "Barrick's North American business has selected Avathon's Autonomy Platform as a strategic technology partner" |
| 19 | 6479, 6738 (E024) | A board-affiliation statement (withdrawn: no design use) | WITHDRAWN | Not carried forward. | n/a | n/a |
| 20 | 2484, 3681 | AusIMM 2022: standard F2 ignores blast swell; F2′ proposed | VERIFIED | Hunt et al., IMG 2022, abstract 96. The authors are OreControl, the vendor of OrePro Recon. | https://www.ausimm.com/globalassets/conferences-and-events/international-mining-geology/mg-2022/abstract-96---a-novel-innovation-for-grade-control.pdf | "nor has the swell for each individual blast been used to determine mass inside of any grade control polygon" |
| 21 | 2667 | "(FACT, R6 §6.8: the disclosed quantified results are almost entirely autonomous-haulage cases)" | PARTLY | This is an absence finding over the Barrick and Newmont filings R6 read, so it is INFERENCE, not FACT. Counter-example for the wider industry: Freeport's 10-K quantifies 214 Mlb of incremental Cu from leach data analytics in each of 2024 and 2025. | https://www.sec.gov/Archives/edgar/data/831259/000083125926000012/fcx-20251231.htm (FCX FY2025 10-K, MD&A) | "Incremental copper production from these initiatives totaled 214 million pounds in both 2025 and 2024." |
| 22 | 2826–2827, 7951 (E035) | Lihir Geminex "technically deployed… not operationally commissioned, with no published benefits" | PARTLY | Deployment status VERIFIED. Metso *does* claim benefits, but they are unquantified (energy, water, chemicals). | https://metso.com/insights/case-studies/mining-and-metals/optimizing-from-mine-to-mill-to-mine-with-digital--twin-technology-at-newmont-lihir-gold-mine (published 24 Oct 2024, updated Nov 2025) | "the system reduced the plant's energy and water consumption and optimized the use of chemicals" |
| 23 | 2828–2829, 7951 (E035) | Freeport TROI at Bagdad: about +10% throughput, about +1 pt recovery, no new capital | SECONDARY-ONLY | The figures are consultant-reported (McKinsey, Feb 2020, "Inside a mining company's AI transformation"). They were read only via a Harvard D3 MBA-student post. The McKinsey page timed out, and Freeport's 10-K does not mention TROI. "No new capital" was not found in any source read. | https://d3.harvard.edu/platform-digit/submission/freeport-pioneers-data-analytics-in-the-mining-industry-to-drive-operational-performance/ (modified 4 Oct 2022) (WF) | "which resulted in a 10% increase in throughput and 1ppt increase in recoveries" |
| 24 | 3132–3133, 7583 (E026) | GISTM: Aug 2020, ICMM/UNEP/PRI, 6 topics, 15 principles, 77 requirements, 219 criteria | VERIFIED | Standard dated 5 Aug 2020. | https://www.icmm.com/our-principles/gistm ; https://www.icmm.com/our-principles/conformance-protocols-gistm (WF) | "Comprising six topic areas, 15 principles and 77 auditable requirements" / "They map to the Standard's 77 requirements using 219 clear and concise criteria." |
| 25 | 3135 | ITRB required for Very High/Extreme | VERIFIED | GISTM Requirement 8.7, reproduced verbatim in Newmont's self-assessment table. | https://s24.q4cdn.com/382246808/files/doc_downloads/2025/08/Newmont-GISTM-ICMM-2025.pdf (1 Aug 2025) | "For tailings facilities with Consequence Classification of 'Very High' or 'Extreme', appoint an Independent Tailings Review Board (ITRB)." |
| 26 | 3137 | ICMM conformance deadlines: Aug 2023 (VH/Extreme) and Aug 2025 (others) | VERIFIED | The second deadline applies to facilities "not in a state of safe closure". | https://www.icmm.com/our-principles/gistm (WF) | "be in conformance with the Standard by 5 August 2023" |
| 27 | 3140 (R6 §1.8) | Barrick: 64 facilities, 18 Extreme (6) / Very High (12) | VERIFIED | The 64 includes facilities "in design phase". Twelve facilities are separately in safe closure. | https://www.barrick.com/tailings-disclosures/ (2026 GISTM disclosure page) (WF) | "only eighteen (18) are classified as 'Extreme' (six facilities) or 'Very High' (twelve facilities) under the GISTM" |
| 28 | 3141 | Of 18: 14 conformant, 1 partial, 1 safe closure, 2 Reko Diq "cannot disclose" | VERIFIED | Reko Diq facilities were conformant in 2025; work is paused for 2026. | same (WF) | "Barrick cannot disclose conformance to GISTM in 2026, but the tailings storage facilities were fully confirming to GISTM in 2025." [sic] |
| 29 | 3143 | Newmont: "meeting the majority of GISTM requirements…", 11 priority facilities | VERIFIED | Letter of 1 Aug 2025, signed by F. Hardy (since retired). It lists 11 priority Very High/Extreme facilities: 8 operating and 3 legacy. | Newmont letter (as #25) | "Newmont has assessed itself as meeting the majority of GISTM requirements across all tailings storage facilities." |
| 30 | 3497–3498 | CCV: 5% of annual incentive, 75% threshold, 85% maximum, 2025 actual 84% | VERIFIED | The weighting is "of 70%", i.e. 5 points of the total API. Payout was 90%. The metric **counts verifications against internal targets**, which is why the plan audits verification quality as well as counts. | 2026 Circular p.45 (scorecard), p.46 note (3) | "It measures the number of Critical Control Verifications undertaken by responsible leadership … against internal targets." |
| 31 | 3498 | Barrick "continuing to shift toward the use of leading indicators" | VERIFIED | Q1 2026 MD&A, safety section. | Q1 2026 MD&A (as #5) | "As an organization, we are continuing to shift toward the use of leading indicators to better understand and manage fatal risk exposure." |
| 32 | 3530 | Autonomy rollouts at Cortez and Carlin are staggered | VERIFIED | Cortez rollout Q3 2026; Carlin operations Q1 2027. | Q2 2026 MD&A | "autonomous operations planned to commence in Q1 2027" |
| 33 | 3713 | "the ±5–10% figure is practitioner lore (FACT, R6 §6.1)" | PARTLY | R6 tagged this [UNV]. It is a checked null (no benchmark found), so the right label is INFERENCE, not FACT. No primary source exists by construction. | R6 §6.1 / §9 item 12 | — (absence claim) |
| 34 | 4127 | WGC guidance note issued June 2013, updated November 2018 | VERIFIED | 26 Jun 2013 date and a "2018-updated-guidance-note.pdf" file are on the WGC page. The **November** month is secondary only (Mining Weekly, 14 Nov 2018). | https://www.gold.org/goldhub/research/guidance-note-non-gaap-metrics-%e2%80%93-all-sustaining-costs-and-all-costs (WF) | "The Guidance Note on Non-GAAP Metrics – all-in sustaining costs and all-in costs was first issued in 2013." |
| 35 | 4136 | Barrick cites the 2013 definition in its 2026 Circular | VERIFIED | Scorecard note (6). | 2026 Circular p.46 | "Although a standardized definition was published in 2013 by the World Gold Council" |
| 36 | 4138–4139, audit fix 2 | Newmont Q3 2026 by-product AISC $1,454 vs co-product $1,963; $1,520 is full-year guidance | VERIFIED | Release dated **22 Oct 2026**. Full-year by-product AISC guidance improved to $1,520. | https://www.sec.gov/Archives/edgar/data/1164727/000116472726000041/newmontq32026earningsrelea.htm (8-K Ex.99.1, 22 Oct 2026) | "All-In Sustaining Costs (AISC) was $1,454 per ounce3; improved full-year cost guidance to $925 and $1,520 per ounce" |
| 37 | 4140–4141 (E025) | Barrick API "actual" $1,423/oz vs $1,637/oz reported for FY2025; adjustment basis not located | VERIFIED | Both figures are in the Circular (p.45 scorecard; p.85 non-GAAP table). The Circular says adjustments "will be fully disclosed", yet none is shown for AISC. The FY2025 release attributes the above-guidance AISC to price-driven royalties, which is consistent with a royalty-normalised scorecard basis (INFERENCE, unconfirmed). OPEN-BULL-1 stays open. | 2026 Circular pp.37, 45, 85; FY2025 release https://www.sec.gov/Archives/edgar/data/756894/000119312526039501/d10712dex991.htm (5 Feb 2026) | "AISC2 of $1,637 per ounce—all slightly above guidance due to higher royalites driven by the higher realized gold price" [sic] |
| 38 | 4187, 4658 | GEO is 25% of Barrick's API | VERIFIED | Threshold 3,471 koz, maximum 4,050 koz, actual 3,675 koz. **The scorecard GEO uses fixed long-term reserve prices** (see #41). | 2026 Circular p.45 and note (5) | "Gold equivalent ounces from Barrick's copper assets are calculated using long-term mineral reserve commodity prices of $1,500/oz of gold and $3.25/lb of copper." |
| 39 | 4188, 4655 | Newmont CSC/GEO is 35% of its bonus | VERIFIED | 2026 Proxy. | https://www.sec.gov/Archives/edgar/data/1164727/000110465926035211/nem-20260512xdef14a.htm | "35% of the annual incentive program is tied to Adjusted Cash Sustaining Costs per Gold Equivalent Ounce (CSC/GEO)" |
| 40 | 4190–4191 | Results release "close to 190%" vs Circular "3-year average 504%": "evidently different bases" | VERIFIED | Both statements are correct. The bases can now be stated as FACT: 190% is **gold oz**, 504% is **GEO**, both cover 2023–25 and exclude A&D. The Circular repeats both figures. | FY2025 release (Reserves; endnote 7); 2026 Circular pp.41–42, 86 | "the three-year rolling average gold mineral reserve replacement stands close to 190% adding more than 24 million ounces" |
| 41 | 4320–4329 (GB-27) | GEO falls 220 koz with gold price, "38% of an illustrative threshold-to-maximum pay range of 579 koz"; "names which one pay uses" | PARTLY | 579 koz = 4,050 − 3,471, which *is* Barrick's API range. But Barrick's scorecard GEO uses fixed reserve-price ratios, so the **Flip applies**: no scorecard effect, only a plan-vs-actual "ratio basis" layer. | 2026 Circular p.46 note (5); p.86 GEO note gives $1,500/$3.25 (2025), $1,400/$3.00 (2024), $1,300/$3.00 (2023) | (as #38) |
| 42 | 4337–4341 (OPEN-BULL-4), 7207 (BD-063), 8197 | Candidate bridge: "190% is gold-only, 5-year cumulative 2020–2024 (pooled 186%; E145)"; "Which basis does the Circular use for '504%'?" | **CORRECTED** | The results-release and Circular 190% is **three-year 2023–25**, gold ounces, excluding A&D. The release's own endnote defines it. 504% = 87 Moz GEO net change ÷ 17.2 Moz GEO depletion (2023–25), with 2024 contributing 79 Moz. E145's 2020–24 definition must come from a different footnote (the Fourmile deck, per C1); it was not re-read here. | FY2025 release endnote 7; 2026 Circular p.86 table | "calculated from the cumulative net change in attributable reserves in ounces from the three most recently completed years divided by the cumulative depletion" |
| 43 | 4498 | "Tanami 2 rose from $395M to $1.7–1.8B" | **CORRECTED** | Feb 2022 estimate: **$850–950M**, production 2024. Jul 2022: about 25% above that, production early 2025; **$395M was spend to 30 Jun 2022**. Feb 2026: **$1.7–1.8B**, production H2 2027. Oct 2026: production "towards the end of the first half of 2027". | Newmont Q4 2021 release https://www.sec.gov/Archives/edgar/data/1164727/000115752322000249/a52584477ex99_1.htm (24 Feb 2022); Q2 2022 https://www.sec.gov/Archives/edgar/data/1164727/000115752322000888/a52789249ex991.htm (25 Jul 2022); Q4 2025 https://www.sec.gov/Archives/edgar/data/1164727/000116472726000009/newmontq42025earningsand20.htm (19 Feb 2026) | "Development costs (excluding capitalized interest) since approval were $395 million" |
| 44 | 4498 | "Pascua-Lama was written down by $5.2B" | VERIFIED | $5.2B in Q2 2013, plus $896M in Q4 2013, for **$6.1B for the year**. | https://www.sec.gov/Archives/edgar/data/0000756894/000119312514123835/d693534dex993.htm (Barrick 2013 MD&A) | "including a $5.2 billion impairment loss related to the carrying value of the PP&E at Pascua-Lama" |
| 45 | 4512 | "greenfield at $18,048/tpa plus ~40% overrun ≈ M&A at $25,182/tpa" | PARTLY | $18,048 is the **all-project** median (96 projects). FactSet applies 40% to the **greenfield** median of **$18,200**, giving about $25,480. The 2020–26 M&A median is $25,182. FactSet's "3% below" is internally inconsistent: $25,480 is about 1% *above* $25,182. | https://insight.factset.com/metals-mining-copper-capex-watch (16 Jul 2026) (WF) | "Applying a 40% overrun adjustment supported by industry studies to the active pipeline greenfield median of $18,200/tpa" |
| 46 | 4515 | Barrick dividend policy: 50% of attributable FCF | VERIFIED | Base dividend $0.175 per quarter plus a year-end top-up, from Q4 2025. | FY2025 release; 2026 Circular p.V | "targets a total payout of 50% of attributable free cash flow on an annualized basis" |
| 47 | 4557 | Newmont minimum-cash floor of $5B | VERIFIED | Restated in the Q3 2026 release. | Newmont Q3 2026 release (as #36) | "while maintaining a minimum cash balance of $5 billion through the cycle" |
| 48 | 4617, 745, 4788 | Bertisen & Davis: 63 projects, mean +14%, about half outside ±15%, about 1 in 13 at ≥ +100% | VERIFIED | All three statistics are in the abstract. Note: the authors recommend a **shifted** lognormal. GB-32 says a single lognormal fails, which is a different claim. | as #3 | "Cost overruns of 100% or more happen in roughly 1 out of 13 projects." |
| 49 | 4618 | "McKinsey megaprojects: 83% with problems, > 40% overrun" | PARTLY | 83% and >40% apply to *recent major* projects (dataset of 80 projects). **Megaprojects (≥ $1B):** overruns ≥ 79% and delays averaging 52%. | https://www.mckinsey.de/industries/metals-and-mining/our-insights/the-capex-crystal-ball-beating-the-odds-in-mining-project-delivery (27 Nov 2024) | "cost and scheduling challenges affect 83 percent of recent major mining and metals projects, with capital expenditure overruns of more than 40 percent" |
| 50 | 4619 | Schedule delays of 20–30% | VERIFIED | McKinsey (same sentence as #49). | as #49 | "and schedule delays of 20 to 30 percent" |
| 51 | 4619 | "with a 1.2-year mean across 58 projects" | UNVERIFIABLE | Not in the McKinsey article (80-project set; delays given in %). EDC's study is 78 projects with a 37% overrun and no year figure. Searched: McKinsey full text, "58 projects" "1.2 years" overrun, EDC/Mining Weekly 2016. | — | — |
| 52 | 4636 | "Barrick reserves at $1,400 or $1,500/oz" | VERIFIED | 2025 reserves at $1,500 (2024: $1,400). The Tier One gold definition uses $1,400 in the Circular glossary and $1,500 in the Q2 2026 release. | FY2025 release (Reserves); Q2 2026 release endnote 5 https://www.sec.gov/Archives/edgar/data/756894/000119312526343857/d67611dex991.htm | "calculated using a gold price assumption of $1,500 and $2,000 per ounce, increased from $1,400 and $1,900 in 2024" |
| 53 | 4696 | Tier One copper: $3.00 or $3.25/lb, ≥ 5 Mt, ≥ 20 years, ≥ 200 ktpa | VERIFIED | $3.00/lb in the Circular (p.3); $3.25/lb in Q2 2026 endnote 5. | Q2 2026 release endnote 5; Circular glossary | "A Tier One Copper Asset/Project is an asset with a $3.25/lb reserve with potential for +5Mt contained copper" |
| 54 | 4656 | ROCE: Barrick 15% of LTI, 10–15% range; Newmont 30% of PSUs | VERIFIED | Barrick 3-year ROCE 21.0%, paid at 100%. | Circular p.41; 2026 Proxy | "30% of the Performance Share Unit (PSU) program is tied to Return on Capital Employed (ROCE)" |
| 55 | 4657 | Reserve replacement: 3-year, 15% of LTI | VERIFIED | Threshold >75%, maximum >100%; paid at 100%. | Circular pp.40–41 | "Reserve Replacement (net of depletion) (15% weighting, assessment: 100%)" |
| 56 | 4659 | rTSR vs GDX: 35% Barrick, 60% Newmont | VERIFIED | Barrick at the 31st percentile, paid nil. | Circular p.41; 2026 Proxy | "Relative TSR (35% weighting, assessment: Nil)" |
| 57 | 6874 (E025) | Pay weights: gold AISC 18.75%, copper AISC 6.25%, GEO 25%, reserve replacement 15% | VERIFIED | The first three are API weights (of 70%). Reserve replacement is an **LTI** weight. | Circular pp.41, 45 | "Gold All-in Sustaining Costs(6) 18.75% $1,565/oz $1,352/oz $1,423/oz" |
| 58 | 6461 (E027) | omf-rust is MIT; HYPOTHESIS that it links arrow/parquet | VERIFIED | MIT; crate 0.2.0-beta.1; "pre-release code"; MSRV 1.85; last push 15 May 2025. **`parquet` 54.2 is a default feature** (`omf1` requires it). The `arrow` feature is not requested. `chrono` is mandatory. | https://github.com/gmggroup/omf-rust (LICENSE, Cargo.toml, README, read 2026-10-29) | "A library for reading and writing files in Open Mining Format 2.0." / "Crate version: 0.2.0-beta.1" |
| 59 | 7950 (E034) | Newmont's NGM default notice (3 Feb 2026); settlement 10 Aug 2026 | VERIFIED | Primary source is Newmont's 10-K risk factor, which upgrades the row from V-S. An earlier notice of 26 Jan 2026 preceded it. The settlement is in Barrick's Q2 release dated 10 Aug 2026. | Newmont FY2025 10-K (as #16); Barrick Q2 2026 release | "On February 3, 2026, we sent Barrick a notice of default under the Nevada JV Agreement related to this conduct." |
| 60 | MOR E024 | Mark Hill permanent CEO from February 2026 | VERIFIED | Appointed 4 Feb 2026. He will be CEO of North American Barrick on separation (Q2 2026 release). | 2026 Circular p.15 | "on February 4, 2026, the Board appointed him as President and Chief Executive Officer." |
| 61 | MOR E024 | North American Barrick IPO targeted for end-2026 | PARTLY | The official target is still end-2026 (10 Aug 2026). On 29 Sep 2026 the CEO said it may slip into January 2027 (trade press, FACT[U]). No public registration statement was found. | Barrick Q2 2026 release; https://mining.com.au/barrick-flags-possible-north-american-ipo-delay/ (30 Sep 2026) (WF) | "Barrick continues to expect to complete the IPO by the end of 2026, subject to market and other conditions and necessary approvals." |
| 62 | MOR E024 | Komatsu FrontRunner AHS runs at NGM | VERIFIED | Partnership launched 31 Jul 2025. Komatsu's 1,000th AHS truck was commissioned at NGM (release dated 22 Apr 2026). | NGM release https://s25.q4cdn.com/322814910/files/doc_news/2025/07/NGM-and-Komatsu-Launch-First-of-Its-Kind-Autonomous-Haulage-Partnership_Final.pdf ; https://www.komatsu.jp/en/newsroom/2026/20260422 | "deployment of Komatsu's FrontRunner Autonomous Haulage System (AHS). NGM will be automating their fleet of 300 and 230 tonne haul trucks" |
| 63 | MOR E024; plan 216, 353 | Barrick uses SAP S/4HANA | VERIFIED | Selected in 2020 for more than 15 countries. Current state was not re-checked; the 2026 Circular mentions SAP and Xeras at Porgera. | https://news.sap.com/uk/?p=132589 (30 Jan 2020) (WF) | "Barrick selected SAP S/4HANA covering Finance, Capital, Supply Chain and Asset Management." |
| 64 | MOR E026 | "discovery to production takes ~16–18 years" (task brief attributes this to MinEx) | PARTLY | This rounds S&P's two different-sample figures (#10). MinEx does not publish it; MinEx/CSIRO gives about 13 years. | as #10 | — |
| 65 | MOR E027 | Cat–RPM | VERIFIED | Completion page; court approval date not re-read. | https://rpmglobal.com/important-customer-update-caterpillar-completes-acquisition-of-rpmglobal/ (WF) | "Caterpillar Inc. (NYSE: CAT) has completed its acquisition of RPMGlobal Holdings Limited (ASX: RUL)" |
| 66 | MOR E027 | Weir–Micromine | SECONDARY-ONLY | Completed 30 Apr 2025 at £624M EV. Read via GMR's reprint of the RNS; the Weir newsroom URL returned 404. | https://www.globalminingreview.com/mining/30042025/weir-completes-acquisition-of-micromine/ (30 Apr 2025) (WF) | "Weir has completed the acquisition of Mining Software Holdings Pty Ltd (Micromine), effective 30 April 2025." |
| 67 | MOR E027 | Sandvik–Deswik | VERIFIED | Stockholm, 4 Apr 2022. | https://news.cision.com/sandvik/r/sandvik-completes-the-acquisition-of-the-leading-mine-planning-software-company-deswik,c3538987 (WF) | "Sandvik has completed the previously announced acquisition of the Australian-based Deswik" |
| 68 | MOR E027 | Komatsu–Modular | VERIFIED | Komatsu presents DISPATCH as its own product. The 1996/2003 ownership dates were not primary-checked. | https://www.komatsu.com/en-us/newsroom/2024/komatsu-launches-interoperable-mine-management-platform (24 Sep 2024) (WF) | "Building upon its industry-leading DISPATCH fleet management system, Komatsu is excited to announce the launch of the new" |
| 69 | MOR E027 | Hitachi–Wenco | VERIFIED | Wholly owned. The 2009 date was not primary-checked. | https://www.wencomine.com/about | "Wenco is a wholly owned subsidiary of Hitachi Construction Machinery headquartered in Vancouver, Canada." |
| 70 | MOR E027 | IMDEX–Datarock | VERIFIED | 100% from 2 Feb 2026 for about A$31M; 51% held previously. | https://announcements.asx.com.au/asxpdf/20260202/pdf/06vv4712m7nlrm.pdf (2 Feb 2026) | "The purchase of the remaining equity to take IMDEX's ownership to 100% was acquired for ~A$31 million" |
| 71 | MOR E027 | Maptek (PETRA) | VERIFIED | Stake raised from 25% to 100%, Mar 2026. | https://www.maptek.com/news/maptek-acquires-petra-strengthening-orebody-intelligence-and-mine-value-chain-optimisation/ (6 Mar 2026) (WF) | "Maptek has increased its shareholding in PETRA from 25 to 100 per cent, following a strategic partnership that began in 2019." |
| 72 | MOR E027 (framing) | "OEMs are consolidating mining software (… IMDEX-Datarock, Maptek)" | PARTLY | Only Cat, Weir, Sandvik, Komatsu and Hitachi are equipment OEMs. IMDEX is a drilling-sensor and data company; Maptek is a private software house. Komatsu–Modular and Hitachi–Wenco are long-standing subsidiaries, not 2020s deals. | #65–#71 | — |

**Verdict counts:** 48 VERIFIED (#1, 3, 7–9, 12, 15, 16, 18, 20, 24–32, 34–40, 44, 46–48, 50, 52–60, 62, 63, 65, 67–71), 2 CORRECTED (#42, 43), 15 PARTLY (#2, 4–6, 11, 13, 21, 22, 33, 41, 45, 49, 61, 64, 72), 4 SECONDARY-ONLY (#10, 17, 23, 66) and 2 UNVERIFIABLE (#14, 51); #19 is WITHDRAWN.

**Other findings outside the plan text but in R6** (for whoever maintains R6):
- **Mali settlement.** Barrick's own Q1 2026 MD&A gives a **$253M** settlement payment in November 2025. R6 §0 item 5 gives about $430M, from Bloomberg. Use $253M as the primary-sourced figure; the $430M may include the CFA VAT offsets.
- **Avathon release date** is 23 Sep 2026, not 25 Sep.
- **R6 §6.2 Tanami line** needs the same correction as #43.

---

## 3. Replacement text (CORRECTED and PARTLY items; optional upgrades marked)

Each patch is find/replace against the 8,262-line snapshot. Labels follow the plan's FACT / FACT[U] / INFERENCE / HYPOTHESIS style. The E6xx IDs refer to §4.

**P-01 (#2, line 216).**
- Find: `The same pay metric appears on different bases in different documents (E025).`
- Replace: `The same pay metric appears on different bases in one disclosure: Barrick's 2026 Circular gives 2023–25 reserve replacement as ~190% (gold ounces) and 504% (gold-equivalent ounces), and 2025 gold AISC as $1,423/oz (pay scorecard) and $1,637/oz (non-GAAP table) (FACT, E603, E604, E605, E607).`

**P-02 (#4, line 217).**
- Find: `Capex overruns average +14% and about half fall outside the claimed ±15% (FACT, Bertisen & Davis, R7 §9.1), with no institutional learning.`
- Replace: `Capex overruns average +14% against the bankable feasibility estimate, about half fall outside ±15% and about 1 in 13 reach +100% or more; the bias barely attenuates over time, which the authors read as intentional, financing-driven optimism rather than a failure to learn (FACT, Bertisen & Davis 2008, E637).`

**P-03 (#5, line 217).**
- Find: `($629/oz H1 2026; TCC $1,897/oz, AISC $2,240/oz;`
- Replace: `($629/oz H1 2026; Q2 2026 TCC $1,897/oz and AISC $2,240/oz, H1 2026 TCC $1,906/oz and AISC $2,104/oz, Barrick 80% basis, E638;`

**P-04 (#6, line 217).**
- Find: `($305M to Atlantic Group;`
- Replace: `(up to $305M to the Atlantic Group; E638;`

**P-05 (#10 optional, #64; line 1000).**
- Find: `  - 15.7–17.9 years from discovery to production.`
- Replace: `  - 15.7 years from discovery to production for 127 mines started 2002–23, and 17.9 years for mines started 2020–23 (FACT[U], S&P Global Market Intelligence, two different samples; E636).`

**P-06 (#10 optional, lines 4509 and 4620).**
- Find: `15.7–17.9-year lead time (FACT, R6 §4.4)`
- Replace: `15.7–17.9-year lead time (FACT[U], S&P; E636)`
- Find: `  - lead time: 15.7–17.9 years (R6 §4.4, §6.2; R7 §9.1).`
- Replace: `  - lead time: 15.7 years (127 mines, 2002–23 starts) to 17.9 years (2020–23 starts) (FACT[U], S&P; E636).`

**P-07 (#11, line 2075).**
- Find: `- expert interpretation accuracy of about 21% in Bond et al. 2007 (R7 §0.6);`
- Replace: `- only 21% of 412 interpreters identified the correct tectonic setting of one synthetic seismic image in Bond et al. 2007 (FACT, E639);`

**P-08 (#13, #14; lines 2458–2459).**
- Find:
  ```
    - Komatsu FrontRunner and Modular DISPATCH (Cortez: 23 autonomous trucks live, Carlin: 40 retrofits from Q2
      2026; FACT R6);
  ```
- Replace:
  ```
    - Komatsu FrontRunner (Cortez: 23 haul-truck retrofits completed May 2026, site-wide rollout planned for Q3 2026;
      Carlin: 40 retrofits from Q2 2026, autonomous operations planned for Q1 2027; FACT, E618, E619), on a new
      fleet management system whose vendor Barrick does not name (Modular DISPATCH is HYPOTHESIS);
  ```

**P-09 (#21, lines 2667–2668).**
- Find: `(FACT, R6 §6.8: the disclosed quantified
  results are almost entirely autonomous-haulage cases)`
- Replace: `(INFERENCE, R6 §6.8: in the Barrick and Newmont
  filings read, the only quantified digital results are autonomous-haulage cases; Freeport, by contrast, reports
  214 Mlb of incremental copper from leach analytics in each of 2024 and 2025, FACT, E645)`

**P-10 (#22, lines 2826–2827).**
- Find: `with no
    published benefits (FACT, B5 W9).`
- Replace: `with no
    quantified benefits published; Metso asserts only unquantified energy, water and reagent savings (FACT, E643).`

**P-11 (#23, lines 2828–2829).**
- Find: `about +10% throughput and about +1 point copper
    recovery without new capital (FACT, B5 W8, E035).`
- Replace: `about +10% throughput and about +1 point copper
    recovery (FACT[U], consultant-reported by McKinsey in 2020 and seen only second-hand; absent from Freeport's
    filings; E644).`

**P-12 (#33, line 3713).**
- Find: `figure is practitioner lore (FACT, R6 §6.1).`
- Replace: `figure is practitioner lore (INFERENCE: a checked null in R6 §6.1).`

**P-13 (#34, optional precision; line 4127).**
- Find: `issued in June 2013 and updated in November 2018 (FACT, R6 §4.1).`
- Replace: `issued on 26 June 2013 and updated in 2018 (FACT, E611; the November 2018 month is FACT[U]).`

**P-14 (#40, optional upgrade; lines 4190–4191).**
- Find:
  ```
    - Barrick's results release says "close to 190%", while the Circular's long-term scorecard shows "3-year average
      504%" (FACT, R6 §1.2): evidently different bases.
  ```
- Replace:
  ```
    - Barrick's FY2025 release and its 2026 Circular give three-year (2023–25) reserve replacement as "close to 190%"
      in gold ounces and as 504% in gold-equivalent ounces (87 Moz ÷ 17.2 Moz), both excluding acquisitions and
      divestments (FACT, E603, E604): the bases differ by metal coverage, not by window.
  ```

**P-15 (#41, lines 4328–4329).**
- Find:
  ```
  - **Flip.** If pay uses fixed budget ratios, the effect disappears from pay but reappears in the plan-vs-actual
    bridge as a "ratio basis" layer.
  ```
- Replace:
  ```
  - **Flip (met for Barrick).** Barrick's 2025 API computes GEO at fixed long-term reserve prices ($1,500/oz Au,
    $3.25/lb Cu; FACT, E606), so the effect disappears from the scorecard and reappears in the plan-vs-actual
    bridge as a "ratio basis" layer. The 579 koz scorecard range above is therefore insulated from the gold price.
  ```

**P-16 (#42, lines 4337–4341).**
- Find: the whole `- **OPEN-BULL-4.** …` bullet, from `Reserve-replacement basis reconciliation` to `before BD-063 passes.`
- Replace:
  ```
  - **OPEN-BULL-4 (resolved from primary sources, W5).** Both figures are three-year (2023–2025), exclude
    acquisitions and divestments, and divide reserve additions by depletion. 190% is gold ounces (FY2025 release
    endnote 7; Circular p.41); 504% is gold-equivalent ounces at reserve prices, 87 Moz ÷ 17.2 Moz, with 2024
    contributing 79 Moz (Circular p.86) (FACT, E603, E604). The 2020–2024 pooled 186% (E145) fits a differently
    footnoted "close to 190%" and is not the results-release basis. BD-063 reproduces both from the Circular table.
  ```

**P-17 (#42, line 7207).**
- Find: `(candidate: gold 5-yr pooled vs GEO 3-yr pooled; OPEN-BULL-4)`
- Replace: `(gold 3-yr 2023–25 vs GEO 3-yr 2023–25, both ex-A&D; OPEN-BULL-4 resolved)`

**P-18 (#42, line 8197).**
- Find: `2. Which basis does the Circular use for "504%" (OPEN-BULL-4)?`
- Replace: `2. How does the scorecard's $1,423/oz gold AISC bridge to the $1,637/oz reported (OPEN-BULL-1)? (The "504%" basis is answered: GEO, 2023–25, ex-A&D.)`

**P-19 (#43, #44; line 4498).**
- Find: `Tanami 2 rose from $395M to $1.7–1.8B; Pascua-Lama was written down by $5.2B (FACT, R6).`
- Replace: `Tanami 2's estimate rose from $850–950M (Feb 2022, production 2024) to $1.7–1.8B (Feb 2026, production H1 2027), roughly 1.8–2.1×; Pascua-Lama was written down by $5.2B in Q2 2013 and $6.1B for the year (FACT, E631, E632, E633).`

**P-20 (#45, line 4512).**
- Find: `Build-vs-buy parity: greenfield at $18,048/tpa plus ~40% overrun ≈ M&A at $25,182/tpa (FACT, R6 §4.3)`
- Replace: `Build-vs-buy parity: greenfield median $18,200/tpa plus a 40% overrun ≈ $25,480/tpa, about equal to the 2020–26 copper M&A median of $25,182/tpa (FACT, FactSet 16 Jul 2026, E642; $18,048 is the all-project median)`

**P-21 (#49, #50, #51; lines 4618–4619).**
- Find:
  ```
    - McKinsey megaprojects: 83% with problems, > 40% overrun;
    - schedule: 20–30% delay, with a 1.2-year mean across 58 projects;
  ```
- Replace:
  ```
    - McKinsey (Nov 2024, 80 projects): 83% of recent major projects with cost or schedule problems, overruns
      > 40%, delays 20–30%; megaprojects ≥ $1B: overruns ≥ 79%, delays 52% (FACT, E641);
    - schedule in years: no sourced reference class yet (the earlier "1.2-year mean across 58 projects" has no
      located source; OPEN);
  ```

**P-22 (#22, #23; line 7951; register index).**
- Find: `| E035 | The industry's believed proofs are single-asset uplifts (Freeport TROI at Bagdad); twins stall on people | B5 §9 |`
- Replace: `| E035 | The industry's believed proofs are single-asset uplifts (Freeport TROI at Bagdad, consultant-reported, FACT[U]); twins stall on people (Lihir) | B5 §9; W5 E643, E644 |`

**P-23 (#59; line 7950; optional upgrade).**
- Find: `| E034 | Newmont's NGM default notice (3 Feb 2026) and settlement (10 Aug 2026); NGM data JV-sensitive | B5 §9 plus corpus E139 |`
- Replace: `| E034 | Newmont's NGM default notice (3 Feb 2026) and settlement (10 Aug 2026); NGM data JV-sensitive | Newmont FY2025 10-K and Barrick Q2 2026 release (W5 E628, E629); corpus E139 |`

**For the `MOR_CASE.md` owner.** These are text proposals only; the file was not edited.
- **E024:** replace `the North American Barrick IPO is targeted for end-2026` with `the North American Barrick IPO is officially targeted for end-2026 (Q2 2026 release); on 29 Sep 2026 the CEO said it may slip into January 2027 (FACT[U])`.
- **E025:** replace `Published reserve-replacement and AISC figures are internally inconsistent across disclosures.` with `Reserve replacement is published on two bases (gold ~190%, GEO 504%) and gold AISC on two bases ($1,423 pay, $1,637 reported); the RR bases are disclosed, the AISC pay adjustment is not.`
- **E026:**
  - Replace `discovery to production takes ~16–18 years` with `discovery to production averages 15.7 years (127 mines) to 17.9 years (2020–23 starts) (S&P, FACT[U])`.
  - Replace `Bond et al. 21%` with `Bond et al.: 21% of 412 interpreters found the correct setting`.
- **E027:** replace `OEMs are consolidating mining software (…, IMDEX-Datarock, Maptek)` with `OEMs own most mine-planning and fleet software (Cat–RPM 2026, Weir–Micromine 2025, Sandvik–Deswik 2022, Komatsu–Modular, Hitachi–Wenco); non-OEM tech firms are consolidating too (IMDEX–Datarock 2026, Maptek–PETRA 2026)`.

---

## 4. Proposed evidence rows E601–E649

The format is `E### | claim | source + locator | quote ≤ 25 words | class`.

Class meanings:
- **FACT:** a primary source read in this pass.
- **FACT[U]:** secondary, consultant or snippet-only.
- **INFERENCE:** derived from FACT rows.

E601 | Critical-control verifications are 5% (of 70%) of Barrick's 2025 API, with a 75% threshold, 85% maximum and 84% actual; the metric counts verifications against targets | Barrick 2026 Management Information Circular (6-K Ex.99.1, 27 Mar 2026), Company API Scorecard p.45 and note (3) p.46 | "It measures the number of Critical Control Verifications undertaken by responsible leadership … against internal targets." | FACT

E602 | Barrick is shifting toward leading indicators of fatal risk | Barrick Q1 2026 MD&A (6-K Ex.99.2, 8 May 2026), Safety | "As an organization, we are continuing to shift toward the use of leading indicators to better understand and manage fatal risk exposure." | FACT

E603 | Barrick's 3-year (2023–25) gold reserve replacement is about 190% (gold ounces, excluding A&D), defined over the three most recent years | Barrick FY2025 results release (6-K Ex.99.1, 5 Feb 2026), Reserves section and endnote 7; repeated in Circular p.41 | "calculated from the cumulative net change in attributable reserves in ounces from the three most recently completed years divided by the cumulative depletion" | FACT

E604 | Barrick's 504% is GEO-basis (2023–25: 87 Moz net change ÷ 17.2 Moz depletion), with copper at reserve prices | 2026 Circular pp.41–42 and Reserve Replacement table p.86 | "replacing 504% of attributable gold equivalent ounces(3)(4) net of depletion at a higher grade" | FACT

E605 | Barrick 2025 API weights: GEO 25% (3,471/4,050/3,675 koz), gold AISC 18.75% ($1,565/$1,352/$1,423), copper AISC 6.25% | 2026 Circular, Company API Scorecard p.45 | "Gold All-in Sustaining Costs(6) 18.75% $1,565/oz $1,352/oz $1,423/oz" | FACT

E606 | Barrick scorecard GEO uses fixed long-term reserve prices, not realised ratios | 2026 Circular p.46 note (5); p.86 GEO note (2025 $1,500/$3.25; 2024 $1,400/$3.00; 2023 $1,300/$3.00) | "Gold equivalent ounces from Barrick's copper assets are calculated using long-term mineral reserve commodity prices of $1,500/oz of gold and $3.25/lb of copper." | FACT

E607 | Barrick FY2025 reported gold AISC was $1,637/oz, above guidance because of price-driven royalties | FY2025 results release, 2025 gold summary; also Circular p.85 non-GAAP table | "AISC2 of $1,637 per ounce—all slightly above guidance due to higher royalites driven by the higher realized gold price" | FACT

E608 | The Circular promises disclosure of scorecard adjustments, but no AISC adjustment is shown, so the $1,423 to $1,637 bridge is undisclosed | 2026 Circular p.37 (API design) vs p.45 and p.85 | "Any such adjustments will be fully disclosed in our Information Circular each year." | INFERENCE

E609 | Barrick LTI weights: rTSR vs GDX 35% (31st percentile, paid nil); ROCE 15% (range 10–15%); reserve replacement 15% | 2026 Circular pp.40–41 | "Relative TSR (35% weighting, assessment: Nil)" | FACT

E610 | Barrick dividend policy targets 50% of attributable FCF ($0.175 base per quarter plus year-end top-up) | FY2025 results release; Circular p.V | "targets a total payout of 50% of attributable free cash flow on an annualized basis" | FACT

E611 | The WGC AISC/AIC guidance note was first issued 26 Jun 2013 and updated in 2018; Barrick's Circular cites the 2013 definition | gold.org guidance page (date and 2018 file name); Circular p.46 note (6) | "Although a standardized definition was published in 2013 by the World Gold Council" | FACT (November 2018 month: FACT[U])

E612 | Barrick Tier One: gold $1,500/oz reserve, ≥ 10 years, ≥ 500 koz/y, lower-half cost; copper $3.25/lb, +5 Mt, ≥ 20 years, ≥ 200 ktpa (Circular uses $1,400/$3.00) | Barrick Q2 2026 release (10 Aug 2026), endnote 5; Circular glossary p.3 | "A Tier One Gold Asset is an asset with a $1,500/oz reserve with potential to deliver a minimum 10-year life" | FACT

E613 | Barrick 2025 reserves used $1,500/oz (2024: $1,400) | FY2025 results release, Reserves and Resources | "calculated using a gold price assumption of $1,500 and $2,000 per ounce, increased from $1,400 and $1,900 in 2024" | FACT

E614 | Mark Hill became permanent President and CEO on 4 Feb 2026 | 2026 Circular p.15 | "on February 4, 2026, the Board appointed him as President and Chief Executive Officer." | FACT

E615 | The official North American Barrick IPO target is end-2026 | Barrick Q2 2026 release (10 Aug 2026), "North American IPO" | "Barrick continues to expect to complete the IPO by the end of 2026, subject to market and other conditions and necessary approvals." | FACT

E616 | The CEO said on 29 Sep 2026 that the IPO may slip into January 2027 | mining.com.au, 30 Sep 2026 (Mining Forum Americas remarks); Bloomberg headline of 29 Sep 2026 | "Maybe it will slide into January, but there won't be a significant delay to that" | FACT[U]

E617 | Barrick North America selected Avathon's Autonomy Platform as strategic technology partner; no terms disclosed | Avathon release, PR Newswire, Pleasanton, 23 Sep 2026 (StockTitan reprint) | "Barrick's North American business has selected Avathon's Autonomy Platform as a strategic technology partner" | FACT (vendor-issued)

E618 | NGM runs Komatsu FrontRunner AHS (launched Jul 2025); Komatsu's 1,000th AHS truck was commissioned at NGM | NGM/Barrick release, 31 Jul 2025; Komatsu newsroom, 22 Apr 2026 | "deployment of Komatsu's FrontRunner Autonomous Haulage System (AHS). NGM will be automating their fleet of 300 and 230 tonne haul trucks" | FACT

E619 | Cortez: 23 retrofits done May 2026, rollout planned Q3 2026. Carlin: 40 retrofits from Q2 2026, operations Q1 2027. The FMS vendor is not named | Barrick Q2 2026 MD&A, Autonomous Haulage Program | "At Cortez, the retrofit of 23 haul trucks was completed in May 2026." "Systems are being tested ahead of a Q3 2026 site-wide rollout." | FACT

E620 | (withdrawn: board affiliation with no design use) | n/a | n/a | WITHDRAWN

E621 | Barrick selected SAP S/4HANA (2020) for finance, capital, supply chain and asset management | SAP News UK, 30 Jan 2020 | "Barrick selected SAP S/4HANA covering Finance, Capital, Supply Chain and Asset Management." | FACT (current state not re-checked)

E622 | Micromine Pitram FMS and mine control was deployed at NGM Goldrush (2023) | Global Mining Review, 4 Jul 2023 (vendor-sourced) | "Micromine has successfully implemented its innovative fleet management system (FMS) and mine control solution" | FACT[U]

E623 | Barrick: 64 TSFs (including those in design), 18 Extreme (6) or Very High (12); 14 conformant, 1 partial, 1 safe closure, 2 Reko Diq undisclosed for 2026 | barrick.com/tailings-disclosures (2026 page) | "only eighteen (18) are classified as 'Extreme' (six facilities) or 'Very High' (twelve facilities) under the GISTM" | FACT

E624 | Newmont self-assesses as meeting the majority of GISTM requirements; 11 priority Very High/Extreme facilities | Newmont letter to ICMM, 1 Aug 2025, p.1 and attachment | "Newmont has assessed itself as meeting the majority of GISTM requirements across all tailings storage facilities." | FACT

E625 | GISTM (5 Aug 2020; ICMM, UNEP, PRI): six topics, 15 principles, 77 requirements, 219 protocol criteria; ICMM deadlines Aug 2023 and Aug 2025 | icmm.com GISTM page and conformance-protocols page | "Comprising six topic areas, 15 principles and 77 auditable requirements" | FACT

E626 | GISTM Requirement 8.7: an ITRB is required for Very High/Extreme facilities | Requirement text reproduced in Newmont's GISTM self-assessment table (1 Aug 2025) | "For tailings facilities with Consequence Classification of 'Very High' or 'Extreme', appoint an Independent Tailings Review Board (ITRB)." | FACT

E627 | Newmont Q3 2026: by-product AISC $1,454, co-product $1,963; full-year guidance $1,520; $5B minimum cash | Newmont Q3 2026 release (8-K Ex.99.1, 22 Oct 2026), highlights, AISC table, capital-allocation section | "All-In Sustaining Costs (AISC) was $1,454 per ounce3; improved full-year cost guidance to $925 and $1,520 per ounce" | FACT

E628 | Newmont sent Barrick a notice of default under the NGM agreement on 3 Feb 2026, after an earlier notice of 26 Jan 2026 | Newmont FY2025 10-K, Risk Factors (joint ventures) | "On February 3, 2026, we sent Barrick a notice of default under the Nevada JV Agreement related to this conduct." | FACT

E629 | The Barrick–Newmont agreement of 10 Aug 2026 resolves all NGM disputes; Newmont pays $1.95B and consents to the IPO | Barrick Q2 2026 release, 10 Aug 2026, headline bullets | "The agreement resolves all outstanding disputes related to NGM." | FACT

E630 | Newmont pay: CSC/GEO 35% of annual bonus; ROCE 30% and rTSR 60% of PSUs | Newmont 2026 Proxy (DEF 14A), CD&A summary | "35% of the annual incentive program is tied to Adjusted Cash Sustaining Costs per Gold Equivalent Ounce (CSC/GEO)" | FACT

E631 | Tanami Expansion 2 estimate history: $850–950M with 2024 production (Feb 2022); $1.7–1.8B with H2 2027 production (Feb 2026); H1 2027 (Oct 2026) | Newmont Q4 2021 release (24 Feb 2022); Q4 2025 release (19 Feb 2026); Q3 2026 release | "Capital costs for the project are estimated to be between $850 and $950 million with a commercial production date in 2024." | FACT

E632 | The often-quoted Tanami "$395M" was spend to 30 Jun 2022, when the estimate rose about 25% and production moved to early 2025 | Newmont Q2 2022 release (25 Jul 2022), projects | "Development costs (excluding capitalized interest) since approval were $395 million" | FACT

E633 | Pascua-Lama impairment was $5.2B in Q2 2013 and $6.1B for 2013 | Barrick 2013 Annual MD&A (Ex.99.3 to 40-F) | "bringing the total impairment loss for Pascua-Lama to $6.1 billion for the full year" | FACT

E634 | Industry average AISC hit a record $1,785/oz in Q1 2026 (+5% q/q, +16% y/y), driven by royalties and corporate G&A | WGC Gold Demand Trends Q2 2026, Supply (30 Jul 2026) | "This was underpinned by a rise in royalties and corporate G&A expenses." | FACT

E635 | MinEx: $218M per discovery (2011–20, 2023 US$, ≥ Moderate size); 2–3 Tier-1 discoveries a year; 0.69 value per dollar (2012–21) | MinEx Consulting, IMARC presentation, 31 Oct 2023, slides 13, 18 and 30 | "Between 2011-2020 the average discovery cost tripled to $218m" | FACT

E636 | Discovery-to-production lead time: 15.7 years (127 mines, 2002–23 starts); 17.9 years (2020–23 starts); 17.8 years (2020–24) | S&P Global Market Intelligence articles (HTTP 403; titles and search snippets only) | "Average lead time almost 18 years for mines started in 2020-23" (article title) | FACT[U]

E637 | Bertisen & Davis (63 projects): +14% mean overrun vs BFS; about half outside ±15%; about 1 in 13 at ≥ +100%; bias intentional; shifted-lognormal model | *The Engineering Economist* 53(2):118–139 (2008), abstract via OpenAlex | "as-built capital costs are, on average, 14% higher than as estimated in the bankable feasibility study" | FACT

E638 | Loulo-Gounkoto (Barrick 80%): H1 2026 152 koz (Q2 88 koz); Q2 TCC $1,897/AISC $2,240; H1 $1,906/$2,104. Tongon sold for up to $305M, closed 1 Dec 2025 | Barrick Q2 2026 MD&A, Loulo-Gounkoto table; Q1 2026 MD&A, discontinued operations note | "to the Atlantic Group for total consideration of up to $305 million. The transaction closed on December 1, 2025." | FACT

E639 | Bond et al. 2007: 21% of 412 interpreters identified the correct tectonic setting of a synthetic seismic image | GSA Today 17(1):4–10, doi:10.1130/GSAT01711A.1, abstract | "Only 21% of the participants interpreted the "correct" tectonic setting of the original model" | FACT

E640 | EY reports spatial compliance to optimal schedules "as low as 30%", citing unnamed reports | EY, *Top 10 business risks and opportunities for mining and metals in 2026* (Oct 2025), p.5 | "Some reports suggest spatial compliance to optimal schedules is as low as 30%" | FACT (EY's underlying source: FACT[U])

E641 | McKinsey (80 projects): 83% of recent major projects hit cost or schedule problems; overruns > 40%; delays 20–30%; ≥ $1B megaprojects overruns ≥ 79%, delays 52% | McKinsey, "The capex crystal ball", 27 Nov 2024 | "cost and scheduling challenges affect 83 percent of recent major mining and metals projects, with capital expenditure overruns of more than 40 percent" | FACT

E642 | FactSet: copper pipeline median $18,048/tpa (96 projects); greenfield $18,200 × 1.4 ≈ $25,480; 2020–26 M&A median $25,182/tpa | FactSet Insight, "Copper capex watch", 16 Jul 2026 | "Applying a 40% overrun adjustment supported by industry studies to the active pipeline greenfield median of $18,200/tpa" | FACT

E643 | Lihir Geminex: technically deployed; commissioning stalled on personnel changes; only unquantified benefit claims | Metso case study (published 24 Oct 2024, updated Nov 2025) | "full operational commissioning is still in progress due to recent personnel changes at the plant" | FACT (vendor page)

E644 | Freeport TROI at Bagdad: about +10% throughput and +1 pt recovery (2018–19), consultant-reported | Harvard D3 student post (Oct 2022) citing McKinsey, "Inside a mining company's AI transformation" (Feb 2020) | "which resulted in a 10% increase in throughput and 1ppt increase in recoveries" | FACT[U]

E645 | Freeport's own filings quantify 214 Mlb of incremental copper from leach data analytics in each of 2024 and 2025 | FCX FY2025 10-K, MD&A, leach initiatives | "Incremental copper production from these initiatives totaled 214 million pounds in both 2025 and 2024." | FACT

E646 | OEM ownership of mine software: Cat–RPMGlobal (2026), Weir–Micromine (30 Apr 2025, £624M EV), Sandvik–Deswik (4 Apr 2022), Komatsu–Modular/DISPATCH, Hitachi–Wenco (wholly owned) | RPMGlobal completion page; GMR reprint of Weir RNS; Sandvik/Cision; Komatsu newsroom 24 Sep 2024; wencomine.com/about | "Caterpillar Inc. (NYSE: CAT) has completed its acquisition of RPMGlobal Holdings Limited (ASX: RUL)" | FACT (Weir: FACT[U])

E647 | Non-OEM consolidation: IMDEX to 100% of Datarock (2 Feb 2026, about A$31M); Maptek to 100% of PETRA (Mar 2026) | IMDEX ASX announcement, 2 Feb 2026; Maptek news, 6 Mar 2026 | "The purchase of the remaining equity to take IMDEX's ownership to 100% was acquired for ~A$31 million" | FACT

E648 | omf-rust: MIT, crate 0.2.0-beta.1 (pre-release), MSRV 1.85; `parquet` 54.2 is a default feature, `chrono` is mandatory, `arrow` feature not requested | github.com/gmggroup/omf-rust: LICENSE, Cargo.toml, README (read 2026-10-29; last push 15 May 2025) | "A library for reading and writing files in Open Mining Format 2.0." | FACT

E649 | AusIMM IMG 2022 (Hunt et al., vendor authors): in-situ F2 ignores blast movement and swell; F2′ (post-blast) proposed | AusIMM International Mining Geology 2022, abstract 96 | "nor has the swell for each individual blast been used to determine mass inside of any grade control polygon" | FACT

**Not given a row:**
- Newmont Boddington's 41 trucks (FY2025 10-K, Properties: "with a fleet of 41 production autonomous haulage trucks"). Cite the 10-K directly or append it as E650.
- The Mali settlement payment ($253M, Q1 2026 MD&A).

---

## 5. Method and limits

- **Claim list.** It was built from:
  - plan Part I (lines about 198–440);
  - every line matching `R6|E024|E025|E026|E027|E035`;
  - Barrick, Newmont and industry numbers in §21.6 and §23;
  - `MOR_CASE.md` rows E024–E027, E034 and E035;
  - the new Loulo-Gounkoto and Tongon text inserted at line 217 during this pass.
- **Not covered.** These cite corpus, B5 or R7 sources rather than R6 and were not re-verified:
  - Draslovka–Avathon (line 2963);
  - E1xx–E3xx corpus figures;
  - the Mine Stats per-ounce royalty values at line 217;
  - the LUC slope (E029).
- **Barrick Q3 2026 results** are due 9 Nov 2026. They will supersede the Cortez "rollout planned for Q3 2026" status and the IPO timing. Re-check both before the presentation if it falls after that date.
- **Disclosure about this pass.** One early SEC download sent a User-Agent string containing the account email from the session context at the time. Later requests used a generic contact string.
