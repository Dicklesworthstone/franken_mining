# What a mining executive could decide with FrankenMining

Product study · 23 November 2026 · repository baseline `b3c6854`

**Recommendation.** Give the group investment committee a one-page answer to *“What is the smallest commitment we need to make at this meeting to keep the project on schedule?”* Start with one actual long-lead equipment package. Put the supplier’s expiry date, cash at risk on cancellation, commissioning impact and effect on the parent’s cash floor above the fold. Show the alternative project that loses funding if there is one. The group CFO sponsors the comparison, procurement supplies the terms and the projects executive owns the schedule. Attach the calculations behind the page the committee already reads.

**Duel result.** Three models generated 90 candidates, advanced 15 proposals and delivered 30 independent opponent scores. Buying time on a long-lead order ranked first by mean opponent score (720/1,000). Sequencing rehabilitation to release collateral was second (707.5). Retaining, partnering or relinquishing exploration rights was third (700). Zero of 15 met the pre-specified 700-from-every-model consensus threshold. The best next investment is a test with a willing operator. No operator interview, vote or realized saving was observed in this study.

## What the first page should say

**DESIGN · illustrative supplier terms.** A mill drive must be ordered by Friday to retain a delivery slot. An immediate order exposes the company to a $5 million cancellation charge. The supplier also offers to hold the same slot through the engineering review for a non-refundable $250,000. In this deliberately simple example, the purchase price and commissioning date are identical if the project proceeds, the hold fee is additional, and financing and other commitments are excluded.

| Choice at Friday’s committee | If engineering proceeds | If engineering cancels |
|---|---|---|
| Order now | Pay the agreed purchase price | Lose $5 million |
| Buy the hold | Pay the same purchase price plus $250,000 | Lose $250,000 |
| Let the slot lapse | Procurement must confirm the next delivery date. Project controls must trace any delay through commissioning. | Avoid the order and the hold fee |

**INFERENCE from those terms.** Between ordering and buying the hold, the break-even cancellation probability is 5% ($250,000 ÷ $5 million). The committee can use that threshold without assigning a probability to cancellation. If Procurement cannot get the hold in writing, remove it from the choices. If the delivery date has schedule float, charge only the delay that reaches first production or another binding milestone. On the page, name who can authorize each option, the last date they can do so, which source term could reverse the choice, and the next decision if they buy time.

**DESIGN.** The first build needs a small input for dated order terms, a mapping to the existing capital cash-flow engine, and a short rendering of the decisive rows. Retain the existing evidence appendix. Test cancellation, credited deposits, a hold that expires before the review, a nonbinding delivery date, and the same cash counted at a JV and its parent. A procurement lead should recognize each term and a project controller should be able to reproduce the result from their own records. [Existing staged decisions](../../../crates/fmine-capital/src/recourse.rs) · [Dated funding fixture](../../../tests/capital/cash_schedule.toml)

## Where the same approach earns its way across the mine life

| Decision and owner | First usable output | Test that would justify a second site |
|---|---|---|
| **Exploration · VP Exploration and land manager.** Renew, farm out or surrender a package before its expiry. | Show discretionary spend, linked corridor and water rights, what past drilling could actually detect, and the minimum value required to keep the rights. | Replay one renewal docket using contemporaneous offers and obligations. Ask if any material, legally available choice survives the geologists’ plausible detection ranges. [Existing absence classifier](../../../crates/fmine-explore/src/absence.rs) |
| **Development · CFO and investment committee.** Set the contingency and size of the next commitment. | Put the sponsoring team’s estimate beside actual outcomes from comparable approvals. Keep scope changes, cancellations and incomplete projects in the population with their status visible. | Hold out a completed project and compare against the project office’s existing estimate. If the projects are too different, present the individual comparisons and leave the adjustment open. [Assumption journal](../../../README.md#register-and-later-score-a-technical-report-assumption) |
| **Operations · COO and CFO.** Support a recovery plan, commission a specific check or take a revised outlook to the disclosure committee. | Identify the site constraint that would have to change for the group to recover, the offset available from other sites on the same ownership basis, and the last date to act. | Replay a dated plan including a genuine ramp-up and a genuine miss. Check whether the page prompts a defensible earlier action than the existing pack. A historical outlier alone cannot establish impossibility. [Current guidance comparison](../../../crates/fmine-cli/src/guidance.rs) |
| **Closure · CFO, Treasurer and closure lead.** Which eligible rehabilitation package should be funded first? | Show work cost, formal acceptance conditions and the earliest date cash, a letter-of-credit line or surety capacity becomes available. Report each separately. | Reconcile one completed release to the regulator’s instrument and the treasury ledger, including review lag and any increase in assurance on the remaining disturbance. If timing and usable headroom never change, end the pilot. [Existing entity funding](../../../crates/fmine-capital/src/entity.rs) |

## Three decisions from real operators

**FACT.** At Quebrada Blanca, Teck reported that managing the rate of tailings rise required concentrator downtime. A group production review could show how much copper was missing. The decision facing management also depended on which alternatives remained available under the site engineers’ operating constraints. This is a reason to carry those constraints into a group forecast and show when management can still respond. [Teck, third-quarter 2025 results](https://www.teck.com/news/news-releases/2025/teck-reports-unaudited-third-quarter-results-for-2025)

**FACT.** Rio Tinto approved Brockman Syncline 1 as a replacement iron ore mine that would use existing plants. In the announcement, first ore moved forward from 2028 to 2027. A replacement project presents a specific capital question. How much would management pay to keep a shared plant fed if its next source of ore arrives late? [Rio Tinto, Brockman investment decision](https://www.riotinto.com/en/news/releases/2025/rio-tinto-to-invest-1_8-billion-to-develop-brockman-mine-extension-in-western-australias-pilbara)

**FACT.** At Jansen Stage 1, BHP’s January 2026 review raised the total potash investment estimate to US$8.4 billion from the preliminary US$7.0–7.4 billion range announced in July 2025. Scope changes, design development, escalation and productivity all contributed. An investment committee facing a revision needs to know which commitments it can still change and what delay would cost. These issuer disclosures establish the decisions and their stakes. Operator willingness to use this product is an open question. [BHP FY2026 Form 20-F, Jansen](https://www.sec.gov/Archives/edgar/data/811809/000119312526354647/bhp-20260630.htm)

## What already works

**FACT.** Four checked-in synthetic examples ran to completion with the local `fmine` binary. Each command below used `--format markdown`. The output sizes count all lines, including the evidence. [Decision engine contract](../../../crates/fmine-decide/CONTRACT.md) · [Capital program contract](../../../crates/fmine-capital/CONTRACT.md)

| Command after `fmine` | What the example actually answers | Output lines |
|---|---|---:|
| `guidance completion --synthetic` | What must the remainder of the year deliver, and did any supplied historical window achieve that combination? | 100 |
| `decide investment --manifest tests/decisions/investment_testwork.toml` | Would learning the ore type before committing change the investment? | 143 |
| `decide program tests/capital/production_investments.toml` | Which investments fit together when two alternatives compete for the same circuit? | 1,118 |
| `decide program tests/capital/debt_funding.toml` | Which staged parent choices remain funded under the stated JV debt and distribution rules? | 4,078 |

**INFERENCE.** The investment example illustrates the executive sentence the product can already support. Under its invented assumptions, pay $100 and accept five days of delay valued at $25. A hard-ore result favors the higher-power investment. A soft-ore result favors the process alternative. The model values the test at $3,361.40 over the best immediate choice, after both costs. A sponsor can challenge the usefulness of the test, the cost of waiting, or the cash flows. All three are identifiable inputs. The example assumes a perfect test and two equally weighted states. [Synthetic investment worksheet](../../../tests/decisions/investment_testwork.toml)

## How to tell whether a major would use it

**DESIGN.** Ask the group head of capital projects for the last investment paper where Treasury, the site GM and project controls disagreed, plus the date of the next comparable meeting. Have each identify the number they challenged, what they could authorize that day and which source would have settled it. If the existing spreadsheet answered the question in time, choose a different case. This also identifies the budget owner and the people who can supply the inputs.

**DESIGN.** Start with read-only copies of the original approval paper, baseline estimate and schedule, the approved scope and change history, and actuals by comparable scope. Add procurement clauses and dated entity cash floors only for a decision that depends on them. The project controller owns the estimate and the scope bridge. Treasury owns cash availability. Procurement verifies order terms. A site specialist owns any physical constraint. Keep the operator’s files in its environment.

**TARGET.** Replay one contentious decision using only the versions available before the meeting. Have the controller reproduce the material numbers independently and flag any difference from the paper the committee saw. For a live decision, give the same committee a one-page insert in its normal pack, before the order or allocation expires. Record what it chose, which condition or amount changed, and what it wants brought back next meeting. A recorded decision to hold can also count when the committee cites the evidence and defines its next trigger. Measure preparation time against its own prior process.

**TARGET.** Expand to a second asset or commodity after a sponsor cites the work in a real decision and requests it again. Rebuild or stop if the company cannot recover the original decision inputs, if timing or scope uncertainty swamps every alternative, or if the incumbent pack already delivers the same answer in time. Distinguish money committed, money theoretically at risk, and realized cash effects when reporting a pilot. The project already defines executive use through a citation in a meeting minute or the sponsor’s written equivalent. [Program gate, §34.5](../../../COMPREHENSIVE_PLAN_FOR_THE_DESIGN_OF_FRANKENMINING.md#345-the-program-gate-month-12)

## What survived the challenge

**FACT about the exercise.** Claude Opus 5.5 (CC), GPT-6.2-Sol (COD) and Gemini 4 Neon (AGY) independently authored 30 candidates apiece. Each chose five, scored both opponents’ slates on a 0–1,000 scale and then read both reviews of its own ideas. Each author revised its five scores and made the case for a rival proposal. The table ranks original proposals by the mean of the two opponent scores. The last column shows how the author’s assessment changed after review. The dash identifies its own proposal. No proposal met the mutual-rejection threshold of an opponent score below 300 plus a concession. [Claude slate](WIZARD_IDEAS_CC.md) · [Codex slate](WIZARD_IDEAS_COD.md) · [Gemini slate](WIZARD_IDEAS_AGY.md)

| Origin and original rank | Decision | CC | COD | AGY | Opponent mean | Author before → after |
|---|---|---:|---:|---:|---:|---:|
| COD2 | Stage the next irreversible purchase | 610 | — | 830 | 720 | 856 → 801 |
| COD3 | Sequence rehabilitation to release collateral | 640 | — | 775 | 707.5 | 828 → 728 |
| COD4 | Renew, partner or surrender exploration rights | 640 | — | 760 | 700 | 804 → 694 |
| CC2 | Adjust a project estimate against the company’s own record | — | 719 | 650 | 684.5 | 780 → 700 |
| AGY4 | Compare a production catch-up plan with the steady plan | 660 | 697 | — | 678.5 | 825 → 760 |
| CC5 | Use survey sensitivity in a tenure review | — | 561 | 750 | 655.5 | 660 → 620 |
| CC1 | Hold or revise group guidance | — | 676 | 610 | 643 | 860 → 680 |
| CC3 | Compare how much commodity-price upside the parent retains | — | 455 | 815 | 635 | 730 → 600 |
| AGY2 | Allocate scarce hub capacity by contractual margin | 470 | 764 | — | 617 | 890 → 740 |
| AGY3 | Compare tailings runway, closure and divestment choices | 620 | 602 | — | 611 | 860 → 730 |
| CC4 | Compare source control with long-term water treatment | — | 492 | 695 | 593.5 | 680 → 560 |
| AGY1 | Combine capital structuring and portfolio selection | 630 | 529 | — | 579.5 | 930 → 780 |
| AGY5 | Combine tenure renewal with pricing a junior earn-in | 600 | 552 | — | 576 | 790 → 700 |
| COD1 | Reserve emergency mill capacity | 560 | — | 575 | 567.5 | 879 → 628 |
| COD5 | Pool a critical spare between sites | 470 | — | 540 | 505 | 782 → 485 |

**INFERENCE.** The two reviewers of the winning purchase idea differed by 220 points. One thought project controls already answers it. The other saw value in tracing the *same* dated commitment through commissioning and into another project’s funding. The first pilot should therefore require a case where a credible cross-project effect exists. Routine spare pooling ranked last. A useful choice for a maintenance manager can still be a weak reason for a group executive to buy a product. [Claude on Codex](WIZARD_SCORES_CC_ON_COD.md) · [Gemini on Codex](WIZARD_SCORES_AGY_ON_COD.md)

**INFERENCE.** The biggest disagreement was over selling price upside through a stream or prepayment (455 vs. 815). The company’s share of an additional dollar of commodity price can be calculated. A counterparty may already reflect the fiscal regime in its bid. Two comparable indicative quotes, valued on the same after-tax basis, would establish whether the exposure changes a financing choice. The hub idea split reviewers 470 vs. 764. Reduce it to one contested shipment, two feasible routes and a signed contract. A settlement that reverses the tonnage ranking would justify further work. [Codex on Claude](WIZARD_SCORES_COD_ON_CC.md) · [Gemini on Claude](WIZARD_SCORES_AGY_ON_CC.md) · [Claude on Gemini](WIZARD_SCORES_CC_ON_AGY.md) · [Codex on Gemini](WIZARD_SCORES_COD_ON_AGY.md)

**INFERENCE.** The reveal narrowed three potentially misleading promises. The guidance screen can raise a question about catch-up, while a genuine ramp-up can exceed every historical window. A sensitive survey with no discovery still leaves option value outside its tested footprint. Tailings runway can give a COO a capital deadline only when responsible engineers supply admissible operating limits and surveyed remaining capacity. These revisions are more important to the product than a few points of rank. [Claude’s response](WIZARD_REACTIONS_CC.md) · [Codex’s response](WIZARD_REACTIONS_COD.md) · [Gemini’s response](WIZARD_REACTIONS_AGY.md)

**HYPOTHESES raised after scoring.** My follow-up priorities below use the same scale. The other models have not scored them. Care and maintenance had already appeared on the longlists. The new emphasis was the different costs of stopping and restarting.

| Priority | Executive question | Cheap way to challenge it |
|---:|---|---|
| 760 | Will hedge margin calls exhaust available cash before a delayed shipment settles? | Replay one price spike with Treasury’s actual daily margin calls, shipment receipts and credit headroom. Test whether the apparent cash shortfall survives the actual settlement calendar. |
| 730 | Should the group continue at a loss, suspend, restart or close? | Reconstruct two past decisions with contemporary price scenarios, care costs and restart commitments. Compare with the company’s existing rule and retain the outcomes unseen during the original decision. BHP’s temporary suspension of Western Australia Nickel gives a public starting case. [BHP FY2026 Form 20-F, §4.4](https://www.sec.gov/Archives/edgar/data/811809/000119312526354647/bhp-20260630.htm) |
| 640 | Would comparing projects on one commodity-price deck reverse the committee’s ranking? | Find two proposed investments in the same meeting pack, trace the original decks and rerun the affected cash flows on a common basis. Different prices can be appropriate for different purposes. A changed like-for-like comparison is the useful finding. |
