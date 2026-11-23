# Executive Decisions — AGY (`b3c6854`)

Audience: CEO, CFO, COO, IC of multi-mine, multi-commodity majors. All outputs remain advisory (`INV-002`).

## AGY1: Pre-FID Sponsor De-Biasing & Scaled Capital Gate (930/1000)

**Decision/Owner:** Approve FID, require paid stage-gate (testwork/FEED), restructure (JV/debt/stream), or reallocate to buybacks across 20–100 candidates (CFO, IC Chair, CEO).
**See/Do:** One card stacks four waterfalls on sponsor cash flows: sponsor ΔNAV/share, fiscal/stepped-royalty/deck-mix audit, overrun/slip haircut, and trapped JV cash/DSCR/dilution, CVaR, KPI opportunity cost, plus exact pre-FID test VOI. IC votes and freezes forecasts in `forecast journal`.
**Reuse:** Unwired `fmine-econ` (`fiscal`, `regime`, `price_deck`, `npv`), `fmine-calib::ReferenceClassPrior` (`bd-2jow`), `fmine-capital` (`entity`, `recourse`, `robustness`, `kpi`), `fmine-decide` VOI, `fmine-lp`, `decide program`.
**New:** Wire `fmine-econ` and overrun priors into `fmine-capital`; lift `MAX_CANDIDATES=8` to 100+ via `fmine-lp` + exact top-K replay; commodity-agnostic fiscal/KPI registry (Fe, Cu, Au, Li, Ni, coal, potash).
**Epistemic:** *Site-only:* pre-tax cash flows, JV/debt terms. *Model:* overrun priors, scenario weights, fiscal shocks. *Customer evidence:* AFE-vs-actual overruns, tax laws, signed JV/loan contracts, IC votes.
**Pilot:** Offline `as_of_decision` replay on 5 past FIDs + 1 live IC gate.
**Kill:** Sponsors withhold raw pre-tax tables, or de-biased cards never flip an IC vote.

## AGY2: Multi-Mine Bottleneck & Commercial-Spec Margin Switch (890/1000)

**Decision/Owner:** Quarterly/monthly pit-to-plant routing, product-spec blending, stockpile holdback, or spot blending across mines sharing rail, port, smelter, roaster, power, or water caps (COO, Commercial/CFO).
**See/Do:** Hub Switch Card compares discrete blend/route/spec regimes on quarterly cash vs LOM NPV shadow cost, bottleneck shadow prices ($/t, $/m³), penalty-cliff/TML flip margins, and untested-ore holdback bounds. Leaders switch feed/spec targets or hold back risky lots.
**Reuse:** `fmine-optimize` (`plan schedule` PCPSP signed blends, splits, Lagrangian bounds), `fmine-plant::production`, `fmine-plant::custody` (`FMINE-REF-403` TML), `fmine-decide::bounds` (`decide recovery-bounds`), `fmine-metrics::valuation` (`decide production`), `fmine-econ::fiscal`.
**New:** Generalize `fmine-econ` beyond `Au/Cu/Ag/Sn` to registry-driven commercial specs (Fe/SiO2/Al2O3/P, As/Bi, Li2O, ash/CSR, moisture, TC/RC, freight) coupled to multi-mine PCPSP shared caps.
**Epistemic:** *Site-only:* block/stockpile grades, hub caps, assay intervals. *Model:* signed-row blend additivity, price/penalty states. *Customer evidence:* executed offtake/smelter terms, port/rail outturns, certified TML/assay settlements.
**Pilot:** 2-quarter offline replay on one multi-mine hub using OMF/CSV schedules and offtake terms.
**Kill:** Marketing withholds penalty curves, nonlinear blending breaks signed rows, or stockpile decoupling (`G-TOUCH-3`) breaks batch tracking.

## AGY3: Closure-Cliff, TSF-Runway & Divest-vs-Rehab Switch (860/1000)

**Decision/Owner:** 3–7 years pre-exhaustion, choose dry-stack/TSF capex vs cut-off throttling across permit delays vs progressive rehab vs divestment indemnity pricing (CFO, COO, CEO).
**See/Do:** Compares regulator bond vs IFRS/IAS 37 provision vs P90 closure PV (perpetual water-treatment flip margins, unmapped hazardous liabilities), parent revolver collateral impact, and exact $/m³ TSF/PAG storage shadow value. Never certifies dam "safe" (`INV-002`). Leaders fund dry-stack/permits, reprice indemnities, or throttle feed.
**Reuse:** `fmine-econ::npv` PV enclosures, `fmine-optimize` volume-capped scheduling, `fmine-capital` entity collateral floors/permit recourse, `fmine-decide` VOI and asymmetric PAG/NAF loss (`GB-22`, `FMINE-REF-402`).
**New:** Lightweight closure/runway adapter coupling bond/ARO/perpetual-treatment intervals, hazardous-inventory completeness (`G-FREE-11`), and TSF volume shadow prices without 3D geotech FEM.
**Epistemic:** *Site-only:* closure studies, waste/TSF volumes, ARD assays. *Model:* rate/duration ranges, permit delays, buyer-default risk. *Customer evidence:* regulator bond orders, audited ARO notes, surety/revolver covenants.
**Pilot:** One aging or TSF-tight mine using closure study, bond schedule, and block-model tailings volumes.
**Kill:** Legal/tailings blocks closure dossiers (`KILL-FREE-1`), or leadership suppresses bond-vs-provision gaps.

## AGY4: Mid-Year Hockey-Stick Guidance Check & Anti-High-Grading Gate (825/1000)

**Decision/Owner:** At Q2/Q3 review for lagging mines, choose early guidance reset vs cross-site shift vs one separating audit—blocking panic Q4 high-grading/capex cuts (COO, CFO).
**See/Do:** Per lagging mine (any commodity): remaining volume/grade/cost frontier vs physical ceilings, compensating grade-for-tonnes variance flags, cheapest separating audit for confounded causes, and LOM NPV lost to panic high-grading. COO orders the audit or CFO resets guidance early.
**Reuse:** `guidance completion`, `recon run`, `fmine-flow::compensation`, `fmine-calib` CUSUM/e-process (`bd-051`), `track-record`, `fmine-plant` (`reconcile-raw`, `rtd-infer`), `registries/biases.toml`, `fmine-gen::pilot_power`, `plan schedule`.
**New:** Commodity-agnostic internal monthly completion adapter; promote `pilot_power` identifiability from `fmine-gen` to K/E; link high-grading counterfactual to `plan schedule` LOM delta.
**Epistemic:** *Site-only:* monthly recon, plant, cost, mine-call packs. *Model:* error covariances, envelope stationarity. *Customer evidence:* realized Q4 actuals, weighbridge/belt-cut audits, past guidance revisions.
**Pilot:** 1-quarter shadow run on 3–5 mines using HQ monthly packs, scored against Q4 actuals.
**Kill:** Sites sanitize/delay monthly recon feeds, or leaders high-grade for quarterly optics despite LOM loss.

## AGY5: Cover-Aware Drill-or-Drop & Junior Earn-In Hurdle Inverter (790/1000)

**Decision/Owner:** Ahead of relinquishment, statutory spend, or M&A deadlines, drop condemned ground, fund high-VOI cover-penetrating tests, or offer staged earn-ins over outright bids (CEO, VP Exploration, IC).
**See/Do:** Two IC views across any commodity: (1) zero-discovery tenements split into cover-masked/untested (`A1`/`A0`/`A4`) vs sensitively condemned (`A2`) with net VOI/$ for next surveys; (2) junior/M&A asks re-priced on the major's price/fiscal deck exposing implied inferred-conversion & capex hurdles (`GB-31`), separating earn-in option value from static NPV (`G-CAP-D`).
**Reuse:** Unwired `fmine-explore` (`absence` A0–A4, `three_part`, `ElicitationRecord`), `fmine-econ` (`fiscal`, `regime`, `price_deck`), `fmine-decide` VOI, `fmine-capital::recourse`, `forecast journal`.
**New:** Wire `fmine-explore` into CLI/Helm; commodity-agnostic asking-price inverter over public `NI 43-101`/`JORC`/`S-K 1300` tables; pre-registered kill-gate templates.
**Epistemic:** *Site-only:* collar/survey metadata, cover depths. *Model:* detection thinning `q_j`, conversion haircuts, lead times. *Customer evidence:* drill/relinquishment logs, public junior filings, post-M&A re-estimates.
**Pilot:** 1 belt (15 tenements) + 5 live/recent junior targets from public filings.
**Kill:** Geologists reject pre-registered kill gates, or hot markets refuse earn-ins for cash bids.

## Appendix: 30-Idea Longlist

1. Pre-FID sponsor de-biasing & stage-gate VOI (**AGY1**)
2. 100+ candidate multi-commodity capital allocator (**AGY1**)
3. Cross-commodity KPI shadow-price/Goodhart audit
4. Wall-safe JV dilution/carve-out simulator
5. Junior/M&A implied-conversion inverter (**AGY5**)
6. Stream/royalty/prepay as CVaR insurance
7. Host-state fiscal-reset settlement vs arbitration
8. Counter-cyclical buyback/debt/capex trigger
9. Cover-aware drill-or-drop relinquishment gate (**AGY5**)
10. Infill vs step-out pit-flip VOI
11. Belt-entry fiscal + data-residency screen
12. Junior-stake ROFR/top-up exercise trigger
13. Historic-core/tailings by-product re-assay VOI
14. IDEA-elicitation exploration calibration gate
15. Pre-permit early-works real-option sizer
16. Modular phased vs megaproject overrun tradeoff
17. EPCM contract & AFE-overrun descope alarm
18. Shared rail/port/power/desal build vs toll/JV
19. Pre-commissioning untested-geomet bulk-pilot hedge
20. Multi-mine bottleneck & spec switch (**AGY2**)
21. Mid-year hockey-stick & anti-high-grading (**AGY4**)
22. Sustaining-capex deferral vs outage-CVaR card
23. Annual mine-plan optimality/stale-deck audit
24. Confounded mine-to-mill audit dispatcher (**AGY4**)
25. Multi-port TML liquefaction & smelter diversion
26. TSF runway, PAG & permit-cliff (**AGY3**)
27. Late-life divest vs rehab/bond-cliff (**AGY3**)
28. PAG/NAF expected-loss & S/ANC VOI
29. Multi-site closure bond/surety revolver optimizer
30. Perpetual water-treatment vs source-control tradeoff
