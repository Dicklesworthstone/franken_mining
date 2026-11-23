# FrankenMining executive decisions
Independent COD study at `b3c6854`. Scores reflect decision value, pilot cost, code reuse and how decisively a pilot can falsify the idea. **FACT.** The checked-in examples below are synthetic. Public filings and solver oracles provide source and arithmetic evidence. Customer demand, adoption and operating benefits remain open. [Delivery record](../../../docs/delivery/README.md)

## COD1 · Reserve an emergency mill slot · 879/1000

**DESIGN.** The group COO decides whether to reserve callable capacity at a sister or third-party mill for the next outage season. The card shows break-even reservation fees by scenario, who gets the slot if both mines call, and payable margin after freight, tolls and displaced host feed. The COO can reserve one slot, change priority or rely on stockpiles.

Site inputs are stockpile assays, compatible routes, host bookings and notice terms. Outage overlap and marginal recovery are assumptions. Reuse `plan schedule` and feed-limited `plant production`. Add notification deadlines and a common capacity row for all callers. BD-319 covers tolling arcs. The new decision prices the right to call capacity before knowing the outage. Replay one outage plus a coincident-outage case against the stockpile plan. Kill if audited feasible routes cannot cover the fee at conservative margins. [Schedule checker](../../../crates/fmine-optimize/src/toposort/pcpsp/candidate.rs) · [Feed fixture](../../../tests/plant/production_supply_limited.toml)

## COD2 · Release only the next irreversible purchase · 856/1000

**DESIGN.** The investment committee decides whether to place a long-lead equipment order, pay to hold its production slot, or wait. It sees dated cash exposure on cancellation, the effect on commissioning, and which other project hits its funding floor. It can cap the commitment and specify the milestone required for the next release.

Site inputs are purchase terms, dated commitments, the critical path and treasury balances. Delivery and milestone dates are scenario assumptions. Translate cancellation, deposit credit and expiry clauses into dated cash flows for `decide program`, reusing funding and recourse. Replay one prior approval from its original package. Reconcile cancellation exposure to an independent procurement calculation. Kill if a staged commitment buys no schedule or funding advantage within the verified terms. [Recourse evaluator](../../../crates/fmine-capital/src/recourse.rs) · [Funding fixture](../../../tests/capital/cash_schedule.toml)

## COD3 · Sequence rehabilitation to free collateral · 828/1000

**DESIGN.** The CFO and closure lead choose which eligible rehabilitation package to fund first. The card shows when work is paid, earliest auditable collateral release, net cash by entity and the approval delay that erases the advantage. Required closure work and monitoring constrain every option.

Site inputs are signed assurance instruments, restricted balances, accepted milestones and bids. Approval timing and acceptance are assumptions until evidenced. Add milestone-to-release cash mapping to the entity funding evaluator. Track surety headroom and funded cash separately, alongside the closure measurands in open BD-304. Replay one completed partial release against the treasury ledger and approval record. Stress an alternative sequence using explicit approval lags. Kill if cash remains locked through both schedules or acceleration consumes the release benefit. [Entity funding](../../../crates/fmine-capital/src/entity.rs) · [Fixture](../../../tests/capital/entity_funding.toml)

## COD4 · Renew, farm out or surrender exploration tenure · 804/1000

**DESIGN.** The CEO and exploration EVP choose which expiring packages to retain before a renewal cycle. Each package shows committed spend, linked rights, and the minimum remaining option value needed to justify renewal. A map shows where sensitive dry holes support walking away and where survey coverage leaves the ground unresolved.

Site inputs are tenure obligations, survey footprints, cover and contemporaneous offers. Detection sensitivity and development economics are assumptions. Reuse `fmine-explore` absence classes and Helm's action comparison. Add package-level linked obligations and dated renew/partner/surrender choices. Open BD-315 selects holes. Reconstruct one renewal cycle from its original pack. Compare the result with the actual retained packages and minimum spend. Kill if plausible detection assumptions reverse every actionable ranking or linked rights leave no discretionary spend. [Absence classifier](../../../crates/fmine-explore/src/absence.rs) · [Decision fixture](../../../tests/decisions/expansion.toml)

## COD5 · Pool a compatible critical spare · 782/1000

**DESIGN.** The COO decides where to hold one shared repairable spare versus buying a second. The card shows whose restoration deadline goes unmet, the overlap that makes the second unit worthwhile, and preferred stocking location. The COO can move inventory or fund redundancy.

Site inputs are verified part revisions, repair histories, transfer times and bottleneck margins. Joint failures and lead-time ranges are assumptions. Add a small inventory replay with dispatch, downtime and return-to-stock events, then value the choices with `decide card`. Replay one component family across two sites against current stocking practice. Stress simultaneous failures and include the donor’s added exposure. Kill if verified fit, installation or transport leaves neither site with a usable transfer window. [Decision code](../../../crates/fmine-decide/src/card.rs) · [Decision fixture](../../../tests/decisions/expansion.toml)

## Appendix · 30 candidates

1. Renew, farm out or surrender exploration tenure.
2. Pay for faster assays before rig demobilization.
3. Combine pad campaigns to avoid duplicate mobilization.
4. Purchase archive access before commissioning new surveys.
5. Trade a peripheral royalty for better district data.
6. Release only the next irreversible purchase.
7. Choose modular capacity under uncertain ore hardness.
8. Site a mill between competing feed sources.
9. Buy a grid connection or lease temporary generation.
10. Secure a corridor easement before redesigning access.
11. Time underground ramp-up against surface depletion.
12. Choose shared versus duplicate construction camps.
13. Reserve an emergency mill slot.
14. Pool a compatible critical spare.
15. Stagger shutdowns around contractor scarcity.
16. Buy the measurement that resolves a metal imbalance.
17. Route ore by payable margin.
18. Secure dry-season water or reduce planned throughput.
19. Redeploy surplus haul trucks across sites.
20. Prioritize loads during utility curtailments.
21. Sequence rehabilitation to free collateral.
22. Compare treatment upfront against perpetual pumping.
23. Set a price for retained divestiture indemnities.
24. Choose care and maintenance or permanent closure.
25. Fund monitoring evidence for a shorter aftercare term.
26. Time parent commitments around JV distributions.
27. Test capital exposure to shared commodity shocks.
28. Refinance between construction and production receipts.
29. Compare buybacks with brownfield sustaining projects.
30. Revise guidance or consume strategic stockpiles.
