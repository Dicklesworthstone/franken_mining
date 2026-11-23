# WIZARD_IDEAS_CC: executive decisions (2026-11-23, at `b3c6854`)

Inputs split into **S** (site-only data), **M** (model assumptions) and **C** (customer evidence). **C is none
throughout:** no operator has used any output. "Reuse" cites code at `b3c6854` (FACT); value claims are HYPOTHESIS.

## CC1: Revise or hold guidance, each quarter (score 860)

- **Decision and owner.** Whether to revise annual guidance now, and which site gets help. The CFO decides with
  the CEO; the COO directs recovery.
- **Sees and does.** Per site: what the rest of the year must deliver, and whether any of the site's own past
  windows delivered that production and cost *together* (exclusion certificate or witness), beside a
  technical-report ceiling. The CFO revises early where excluded and holds where witnessed.
- **Reuse.** `guidance completion` (coupled T1 windows, T2 ceilings, `--as-of`), `track-record --cohort-as-of`,
  `forecast journal`.
- **New.** Attributable portfolio roll-up, joint production-cost T2, declared cost of revising late.
- **Pilot.** Run every site at the 2026Q3 cut, freeze the verdicts in the journal, and score them at the February
  2027 release. Needs agreed guidance transcriptions (bd-bi9f.2 open).
- **Kill.** Verdicts fail to separate sites that miss from sites that hit, or most rows stay indeterminate.
- **S** monthly actuals, plan. **M** sales continuation, window comparability. **C** none.

## CC2: FID haircut from your own scored studies (score 780)

- **Decision and owner.** The capex, ramp-up and recovery haircuts applied at FID, and which projects still clear
  the hurdle. The CFO and the investment committee decide.
- **Sees and does.** "Our studies' year-two throughput ran x–y% high over n resolved assumptions," shown beside the
  industry class (Bertisen & Davis mean +14%; McKinsey 83% with cost or schedule problems, E641). The committee
  applies its own haircut visibly, never inside the discount rate.
- **Reuse.** `forecast journal register-assumption`/`resolve-assumption` (BD-276), `track-record` capex scopes,
  and the reference-class fields in `decide investment` and `decide program`.
- **New.** A hierarchical company effect (BD-050, BD-230 open), assumption families beyond capex, and an FID card.
- **Pilot.** Register the 2027 assumptions in current technical reports now; score 2024–2026 capex guidance
  retrospectively, labelled as retrospective.
- **Kill.** Under about ten resolvable assumptions leaves the company interval wider than the industry class, or
  the project office already keeps this record.
- **S** internal studies, approvals, final costs. **M** reference-class hierarchy. **C** none.

## CC3: Price-upside retention map (score 730)

- **Decision and owner.** Where to place streams, prepays, royalty sales or hedges, and whose growth to favour at
  high prices. Treasury and the CFO decide, with the investment committee.
- **Sees and does.** Per asset and price, the share of a marginal price move the parent keeps after price-stepped
  royalties, super-profits tax, state equity and ownership. Upside is sold first where the parent keeps least.
- **Reuse.** In fmine-econ: `compute_royalty_rate`, `compute_fiscal_breakdown` and `evaluate_regime_fiscal_terms`.
  `registries/country_regimes.toml` (FiscalShock, NotPublished, Restricted). Ownership basis from public Claims.
- **New.** A verb (fmine-cli does not depend on fmine-econ), a marginal-retention curve and a stream candidate in
  `decide program`.
- **Pilot.** Statute-only curves for the registry's DRC and Mali rows, USD 2,000–4,500/oz.
- **Kill.** Spread across assets under 10 points, or counterparties price the same fiscal take. Unclear tax
  treatment of streamed ounces refuses.
- **S** stability agreements, tax losses. **M** statute reading, price grid. **C** none.

## CC4: Closure flip-rate card (score 680)

- **Decision and owner.** Fund source control (covers, diversions) now, or accept perpetual water treatment; also
  the form of financial assurance. The COO and CFO decide.
- **Sees and does.** The bond, the provision and a stochastic estimate side by side, never reconciled. Beside them,
  the real discount rate at which "build now" and "treat forever" swap, against the provision's rate.
- **Reuse.** In fmine-econ: `find_all_bracketed_irrs`, `count_cash_flow_sign_changes` (which refuses multiple roots)
  and `discover_discounting_convention`. Also `decide card`.
- **New.** Closure measurand types (BD-304 open), treatment-duration bounds.
- **Pilot.** Recover the discount conventions from public disclosures: Pueblo Viejo's USD 273M provision is
  USD 126M discounted (E271), and Nevada bond-to-estimate ratios run 1.5–2.7× (INFERENCE, E188). Then one synthetic
  flip card.
- **Kill.** Discounted/undiscounted pairs are missing, or every flip rate lies far outside plausible rates.
- **S** activity costs, flows. **M** duration cutoff, inflation. **C** none.

## CC5: Typed-absence tenure review (score 660)

- **Decision and owner.** At the annual land review, renew, drop, farm out or drill each tenement. The Head of
  Exploration decides with the CFO.
- **Sees and does.** Holding cost by absence class: A0 never looked; A1 looked but blind (for example, geochemistry
  under cover); A2 ruled out to detection; A3 sub-economic hit; A4 unrecorded effort, with Manski bounds. A2 ground
  is dropped and A1 ground farmed out.
- **Reuse.** In fmine-explore: `classify_site_absence`, `assess_site`, `effective_detection_q` and the `three_part`
  USGS baseline. None of these has a verb yet.
- **New.** Tenure register (costs, commitments), economic filter, verb.
- **Pilot.** In one public district archive, measure how much drilled ground is A1; hindcast later discoveries
  (§11.6).
- **Kill.** BET-08/KILL-LODE-1 fires, or statutory spending commitments fix the decision.
- **S** proprietary drilling, commitments. **M** detection q, depth of investigation. **C** none.

**Ranking.** CC1 runs on existing verbs and public data, quarterly. CC2's journal path exists. CC3–CC5 need a verb
over existing kernels and operator-only data for full value.

## Appendix: 30-item longlist

1. Revise-or-hold guidance (CC1)
2. Own-record FID haircut (CC2)
3. Price-upside retention map (CC3)
4. Closure flip-rate card (CC4)
5. Typed-absence tenure review (CC5)
6. Country-regime common-shock capital program
7. Shadow prices of scorecard ratio constraints
8. Tailings runway against the permit clock
9. Junior stakes priced by implied conversion
10. Pre-FID testwork EVSI with delay cost
11. JV capital call: fund or dilute
12. Fragile-project flag from selection stability
13. Cross-site within-factor compensation escalation
14. Measurement-audit VOI: which sampler to fix
15. Recovery below each site's own grade relation
16. Reserve price-deck choice from shell membership
17. Sustaining-capex deferral detector
18. Stream as tail insurance on shared scenarios
19. Buyback versus project on NAV per share
20. Divest versus operate to closure
21. Care-and-maintenance restart trigger
22. Dry-hole stop rule per target
23. Partner-filing mirror before JV disputes
24. Disclosure pre-flight lint
25. Site, corporate and agent forecasts compared
26. Past approvals re-carded as known at the time
27. Insurance-recovery claim card
28. Stockpile holdback for recovery targets
29. Peer cost-rank claim check
30. Hazardous inventory to ARO completeness
