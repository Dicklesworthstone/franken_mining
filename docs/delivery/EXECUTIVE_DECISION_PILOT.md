# North Basin Metals investment committee

**Synthetic meeting paper · 8 March 2030 · copper replacement circuit.** North Basin Metals, its supplier, contracts, schedule, meeting, and subsequent outcomes are fictional. All figures are whole nominal USD and concern unrestricted parent cash.

**Decision requested.** Authorize procurement to pay **$250,000 by 8 March** to retain the November drive slot through the 22 March engineering review. Authorize no equipment purchase today. Return on 22 March to exercise at the agreed price or release the slot. The hold is the smallest upfront payment and cancellation exposure among the choices that meet both treasury floors, preserve the substation commitment and keep first ore on schedule. [Decision worksheet](../../tests/capital/executive_purchase.toml)

| Choice today | If the project proceeds | If engineering cancels | Consequence for the committee |
|---|---|---|---|
| Order now | $3m deposit today, credited against $20m total | $5m total, including the deposit | Breaches today’s $18m parent cash floor by $1.5m |
| Hold through review | $250k today and $20m at review | Lose the $250k fee | Preserves first ore and leaves $250k over the floor after funding the substation |
| Let the slot lapse | $21.5m at review, delivery 30 days later | No supplier payment | After 12 days of float, first ore slips 18 days. Retaining the substation would leave parent cash $1m below floor |

**Controller’s independent calculation.** Treasury starts the meeting with $19.5m. Ordering leaves $16.5m; holding leaves $19.25m. The assumed 22 March counterfactual balance is $45m before the drive and substation. Exercising the hold leaves $45m − $20.25m − $6.5m = $18.25m. Under lapse it leaves $45m − $21.5m − $6.5m = $17m. Delay exposure is 18 × $80,000 = **$1.44m**, shown separately from cash due on 22 March. The break-even cancellation probability for hold versus order is $250k ÷ $5m = **5%**. The committee assigned no cancellation probability.

**Before signing.** Procurement must confirm the hold in writing under VD-17. Project controls must confirm the 12-day float against B07. Treasury must reserve the substation allocation under T09. Reopen the decision if the supplier changes the hold or the later slot fits inside the revised float.

---

## Independent replay and follow-on use

**Negotiating room at the original meeting.** In the fictional copper case, Treasury can spend
$45m − $18m − $6.5m = **$20.5m** in total at review. The reserved drive costs $20m and its fee buys
no exercise credit, leaving **$500k** as the largest affordable hold fee. Today's cash also permits
that fee. For waiting to work, Procurement needs a replacement quote at or below **$20.5m**, with delivery
by **27 November** (15 November plus 12 days). Against the existing offer this requires **both** a
$1m reduction and delivery 18 days earlier. The supplier has offered neither concession. These figures
measure the budget and schedule limits, without assigning a value to the vendor's willingness to move.

**Fictional minute and subsequent outcome.** On 8 March the group committee approved only the fee and set the review date. In this scenario the engineering team approved the drive on 22 March and procurement exercised the hold. The fee was spent. No realized saving is credited. This resolution is outside the original meeting insert and its input worksheet.

**DESIGN.** In the simulated original meeting, procurement’s stand-alone comparison favored the $20m order over the later $21.5m quote. Treasury’s dated floor and the substation commitment changed the simulated committee choice. The CLI reads only the decision-time worksheet. The later fictional minute above is deliberately outside its inputs. JSON preserves the original worksheet and digest so a reviewer can challenge a date, credit or basis.

**SYNTHETIC follow-on · 12 April 2030.** The same committee requested the comparison for a South Shaft potash gearbox. Delivery in the alternate slot is nine days later. The schedule has 15 days of float. The alternate quote holds the $8m price through the review. Waiting preserves the shutdown date and costs nothing to reserve. The simulated committee let the slot lapse. The runner-up hold costs $150k with $100k credited on exercise. Its hold-versus-order threshold is $50k ÷ $1.7m = 1/34. This second case tests whether the tool can advise the opposite choice. [Potash worksheet](../../tests/capital/executive_purchase_potash.toml)

| Other committee use | Synthetic replay result |
|---|---|
| Exploration tenure | The cover exceeds the survey depth. Renewing ranges from −$150k to $1.05m and farming out from $210k to $740k. Both outperform surrender across the declared bounds. The overlapping renewal and farm-out estimates leave the partner negotiation open. [Land docket](../../tests/capital/executive_tenure.toml) |
| Quarter-end recovery | Baseline output is 12,000 attributable tonnes of copper against 13,000 required. The two sites can add up to 1,300 under the declared simultaneous limits. If Mesa misses the intervention date, the remaining options leave a 700-tonne deficit. The bounds carry no probability of success. [Operations docket](../../tests/capital/executive_group.toml) |
| Closure funding | North cover releases $1.51m net cash by year-end after costs and rebonding. East requires $575k net cash but frees $4.2m of credit-line capacity and $1m of surety capacity. The treasurer can select the package that addresses the actual constraint. [Assurance instruments](../../tests/capital/executive_rehabilitation.toml) |
| Original project estimates | Five original approvals stay visible. Only one comparable closeout was known before the latest approval. The original estimate misses by $2.5m and the illustrative adjustment misses by $0.5m. This single holdout cannot establish predictive performance. [Approval cohort](../../tests/capital/executive_estimates.toml) |

## Reproduce the meeting insert

```sh
cargo run -p fmine-cli -- decide program tests/capital/executive_purchase.toml --format markdown
cargo run -p fmine-cli -- decide program tests/capital/executive_purchase_potash.toml --json
```

**Measurement boundary.** The fixtures exercise the calculations, evidence cutoff and change of decision. Operator preparation time, adoption, actual contract enforceability, physical execution and realized business benefit require independent observations. None is measured by this fictional pilot.

## Delivery checklist

| Bead | Completed work | Verification |
|---|---|---|
| bd-4hg5 | Read quoted price, deposit, cancellation, option fee and exercise credit. Validate offer and review dates. Compare missing and expired offers. Charge schedule delay only after float. Check each branch against both parent floors and the competing commitment. Retain tied choices, source locators and the original worksheet. | Copper and potash binary examples; controls for credits, refunds through the payment convention, expired holds, future data, mismatched JV cash and float. |
| bd-3a2a | Admit eligible packages, date work and conditional releases, deduct replacement collateral and work cost, and report cash, line and surety separately. Preserve unknown release timing. | Net cash and line/surety known answers; late, pending and ineligible release controls. |
| bd-rn1q | Compare renew, farm out and surrender. Carry rights and obligations, dated offers and retained interest. Apply the existing cover-sensitive detection calculation. Report the economic ranges, dominated choices and unresolved overlap. | Blind survey versus sensitive negative, expired farm-out, legally unavailable choices, invalid retained share and detection inputs. |
| bd-jg7w | Require a common commodity, unit and remaining interval. Apply site interests once. Reconcile group and site targets, enforce lapsed deadlines, and identify each indispensable site increment. | Attributable group example, missed intervention, basis mismatch, duplicate site, over-ownership and allocation mismatch. |
| bd-zabr | Preserve all original approvals and their disposition. Admit only earlier comparable closeouts, retain scope changes and cancellations, and report original and adjusted holdout errors. | Five-project docket plus cohort timing, latest holdout, unknown actual and same-day outcome controls. |
| bd-srg1 | Construct the fictional source terms, original decision insert, controller reconciliation, simulated minute, later outcome and second-commodity reuse. Keep the future result outside the original insert. | Two independent purchase known answers, source-byte hashes, deterministic replay and opposite decisions. Real-world outcomes remain unobserved. |

The command-level regression cases are in [executive.rs](../../crates/fmine-cli/tests/executive.rs). The input controls run beside the [worksheet evaluator](../../crates/fmine-cli/src/capital/executive.rs).
