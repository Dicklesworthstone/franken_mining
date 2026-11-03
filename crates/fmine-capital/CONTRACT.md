# CONTRACT.md — `fmine-capital`

## 1. Charter & Layer

E-layer Helm capital-program comparison, within the planned 20k/23k LOC budget. These bounded BD-229/BD-314 slices
enumerate declared static subsets and two-stage policies, using the permitted `fmine-capital -> fmine-decide` edge.
- **Allowed Module Families:** `["model", "render", "recourse", "entity"]`
The static kernel lives in the crate root; `recourse` composes its state-level results into complete policies.
`entity` supplies separately conserved cash ledgers and declared ownership distributions as feasibility constraints.

## 2. Mathematical & Domain Formulation

For each subset, spending is the exact sum of declared candidate costs. Joint scenario delta-NAV is the sum
of selected candidates' **net** payoffs at the same named state. Costs constrain spending and are not
deducted from those payoffs again. Prerequisites, exclusive groups and budget limits are applied before risk.
Every binary subset remains visible, including structurally rejected subsets with named rejection reasons.

Optional funding declares an inclusive cash window and separate cash basis, available cash in every shared
state, a nonnegative minimum cash floor, and the complete candidate-by-state matrix of nonnegative cash
requirements (including explicit zeroes). Each subset sums requirements within the same shared state, then
reports remaining cash and slack to the floor. Every negative slack produces a named liquidity shortfall
before risk ranking. Requirements and planned costs are not deducted again from already-net delta-NAV.
Cash results remain visible even for subsets rejected by budget, prerequisites or exclusions. This is
aggregate fundability for the declared window, not a guarantee of every intra-period cash balance or a
staged funding schedule.

Alternatively, a `FundingSchedule` declares state-specific opening cash and complete signed baseline and
candidate cash movements over contiguous inclusive calendar periods. Positive movements are receipts;
negative movements are expenditures. Baseline movements exclude the selected program. At each period,
closing cash equals opening cash plus baseline flow plus the selected candidates' flows in that same state.
The closing balance carries forward exactly. The first period's floor applies at initial opening, and every
period's floor applies at its close. Every negative slack produces a dated structural rejection, including
earlier breaches followed by later recovery. All checkpoint rows remain visible even for rejected subsets.
There is no implicit within-period payment ordering, discounting, credit facility or adaptive recourse.
Aggregate funding and scheduled funding are mutually exclusive; neither changes the already-net NAV input.

DESIGN (BD-314, §23.2): optional `EntityFunding` is a third, mutually exclusive funding mode. Every named
entity has explicit opening cash in each shared state, a floor for every period, and complete external
baseline and candidate cash-flow matrices. Fixed funding transfers have two modeled endpoints, one period,
a complete state-amount vector and an optional activating candidate. Outside partner funding appears in
native cash inputs unless separately declared as modeled capital-call cash below; ownership alone never
implies a partner contribution or a parent top-up.

Each state starts with opening-floor checks. Every period then applies native external flows and all fixed
transfers, checks each entity's pre-distribution floor, processes ownership distributions child first, and
checks closing floors. An earlier failure remains a rejection even if a later distribution repairs it.
Put a dividend in an earlier period when it is intended to fund a subsequent expenditure. No payment order
inside the native-flow/transfer phase is inferred. Fixed transfers debit and credit the same exact amount.

Each distributing entity has one parent, an explicit ownership fraction and distribution basis. The graph
must be acyclic. A complete period/state permission and cap determine the maximum payout: a closed gate
pays zero; an open gate pays the lesser of its cap and positive cash above the current entity floor. Only
the parent's fraction reaches its ledger. The remainder is explicitly paid outside the modeled graph.
Blocked or capped cash remains with its entity. A parent's onward distribution includes child receipts
through deterministic child-first ordering. All amounts and carry balances remain rational, including
sub-minor-unit ownership splits; no cash is rounded through fixed-decimal `Money`.

Without modeled debt or capital calls, every period proves closing cash plus external distributions equals
opening cash plus native external flows. Internal transfers and parent receipts cancel exactly. A conservation failure
refuses the comparison. Source-bound NAV remains attributable to `objective_entity`; cash movements do
not create another NAV gain or deduction. When equity is declared, its entity must equal that objective
entity. The model does not derive another entity's NAV or infer dilution, taxes or actual lending terms.

DESIGN (BD-314, §23.2): optional `EntityCapitalCallFunding` supports the explicitly declared
`pre_money_subscription` method. Its `native_excludes_modeled_capital_calls` convention keeps these
contributions out of native cash and fixed transfers. Each call names a distributing JV, one period,
an optional activating candidate, an explicit sequence, and a basis. Every shared state supplies positive
pre-money equity value `V`, nonnegative required funding `Q`, parent contribution `P` and outside contribution
`X`. With prior parent ownership `s`, the new fraction is `(s*V+P)/(V+P+X)`; the outside fraction is its exact
complement. No JV penalty, subscription price, top-up, actual agreement or terminal NAV is inferred.

Full planned contributions settle even on infeasible paths: the parent pays `P` and the JV receives `P+X`.
They are never clipped to affordable cash. Signed funding surplus `P+X-Q` below zero causes a named
call/state structural rejection independently of cash floors, so existing JV cash cannot mask an unmet
raise. Oversubscription is permitted when explicitly supplied. Partial subscriptions still change ownership
on rejected paths; no rollback, arrears, remedy or default recovery is fabricated. Inactive calls retain
their declared terms, report zero actual funding/surplus and leave ownership unchanged.

Calls follow chronological periods and explicit sequence within each JV/period; identifiers and source row
order do not determine economics. Multiple candidate-triggered alternatives may occupy distinct sequences,
and even an excluded combined subset retains its declared full ledger. Ownership starts at the distribution
edge's initial fraction, carries across calls and periods, and governs the current and later dividends.
All call cash settles after native flows/fixed transfers and before modeled debt service, pre-distribution
floor checks and child-first distributions. Cash is checked after this complete phase, without inferred
intermediate payment ordering. Internal parent contributions cancel; outside call contributions enter the
period conservation equation as a separate external inflow. Call cash, ownership fractions and separately
declared primary/secondary equity NAV/share counts remain distinct quantities.

DESIGN (BD-314, §23.2): optional `EntityDebtFunding` derives debt service, coverage locks and principal
sweeps for one external senior facility per debtor entity. The explicit `native_excludes_modeled_debt`
cash convention separates native receipts/expenditures from modeled draws, interest, principal service
and sweeps. CFADS is a separately declared covenant metric with its own basis; it is never another cash
receipt. Opening principal, baseline debt flows and selected-candidate debt increments require complete
facility/state, facility/period/state and facility/period/candidate/state matrices, including zeroes.

For opening principal `B`, current draw `D`, declared full-period rate `r`, and principal-payment cap `A`,
interest is `(B+D)*r`, scheduled principal is `min(A,B+D)`, and scheduled service is their sum. This is an
explicit draw-before-interest convention; no annual rate, accrual calendar or day-count rule is inferred.
The full scheduled service leaves the debtor's cash even when its balance becomes negative. The ordinary
entity floor checks retain that planned funding failure; service is not reduced to affordable cash and
the calculation does not invent arrears, default remedies or emergency finance.

For positive scheduled service, DSCR is declared baseline plus selected-candidate CFADS divided by that
service. The discretionary sweep is excluded from its denominator. Exact equality with the declared
positive minimum DSCR passes. When service is zero, the ratio is absent and the result explicitly says
`NoServiceDue`; the facility's `allow_distributions_without_service` declaration controls eligibility.
A coverage failure locks an ownership distribution but is not itself a cash-floor or default rejection.
The original distribution permission and cap remain binding alongside the calculated coverage gate.

Timing is explicit. All native flows, fixed transfers, loan draws and mandatory service precede every
entity's pre-distribution floor check. The graph then visits all entities child first, including roots.
Immediately before an entity's own dividend, its facility sweeps
`min(remaining principal, sweep_fraction * max(0,current cash - floor))`. Current child dividends are
therefore included in their parent's sweep base, even when that parent has no outgoing ownership edge.
They cannot erase the earlier mandatory-service/floor failure. Sweeps preserve the cash floor; closing
checks follow the complete graph traversal. Debt after the sweep carries into the next period's interest.

Both conservation equations must be exactly zero: closing principal plus scheduled principal and sweep
minus opening principal and draw; and modeled closing cash plus outside dividends, interest, scheduled
principal and sweeps minus modeled opening cash, native flows and loan draws. Every facility/period row
is retained on every state and rejected path. Loan draws, repayments, coverage and sweeps constrain cash;
they do not add to or deduct from separately supplied already-net objective NAV again.

DESIGN (BD-229, PO-CAP-2): optional `EquityBasis` switches to the explicit
`additive_equity_nav_with_share_changes` convention. Candidate payoffs then represent changes in **total
equity NAV**, including buyback cash paid/fees, net issuance proceeds or already-net project value. They
must not contain a precomputed per-share accretion benefit. For each shared state, baseline NAV `N`, initial
share units `u`, actual shares per unit `s`, and the selected changes `dN` and `du`, the objective is
`(N + dN) / ((u + du) * s) - N / (u * s)` in USD per actual share. Initial units and scale are positive;
NAV and candidate share changes may be signed. Each whole bundle is valued once in each state, preserving
the dependence between NAV and share changes. The ratio of expected NAV to expected shares and the sum of
standalone per-share benefits are not substitutes. Funding remains a separate USD constraint.

Structurally feasible subsets use `fmine-decide::card` for expected value, inverse-CDF P10/P50/P90,
CVaR95 of signed loss `-delta-NAV`, and same-state regret against all structurally feasible subsets. A
configurable exact positive tail mass additionally integrates the lowest-payoff states, including fractional
boundary mass, and negates their mean. An optional signed tail-loss ceiling retains equality. Risk-rejected
subsets retain their numerical outcomes. All expected-value co-winners remain visible before and after risk;
the difference of their maximum means is the expected-value cost of the risk limit.

In equity mode those total-USD statistics remain separate reporting columns. `ExactPerShare` and
`PerShareStats` carry the USD/share objective, quantiles, CVaR95, same-state regret and configurable tail
loss, using `fmine-numerics::risk::exact::ExactDistribution` without rounding through fixed-decimal `Money`.
Co-winners and risk cost use the per-share mean and the optional signed `max_tail_loss_per_share` ceiling;
the legacy USD `max_tail_loss` is forbidden. Each bundle retains baseline/change/ending NAV and normalized
actual shares. Nonpositive ending shares cause a named state-level structural rejection with undefined
ending and delta NAV/share, retaining all state rows. No statistics or ranking are invented for that bundle.

DESIGN (BD-314, §23.2): optional secondary `EntityValuation` Claims evaluate the same selected candidates on
additional entity/share-class denominators. Existing `EquityBasis` remains the sole primary objective.
Each secondary perimeter declares its own valuation basis, complete baseline NAV, candidate/state already-net
NAV changes, candidate/state share changes and optional signed USD/share tail-loss ceiling. It shares the
program model, currency, terminal states, weights and tail mass, but never receives inferred NAV from an
ownership percentage, cash transfer or another entity's valuation. A valuation perimeter need not be a cash
graph node. Terminal share changes do not alter dated cash-waterfall ownership fractions.

Every entity's whole-bundle same-state ending NAV and actual shares are calculated before division. A
nonpositive secondary ending count is a named entity/share-class/state structural rejection, retaining its
undefined ratios. All primary and secondary statistics and hindsight regrets use the common population
satisfying every structural constraint. Each entity then applies its own optional tail ceiling to its complete
unconditional distribution; exceeding any ceiling rejects the choice for risk, while equality passes. All
statistics remain visible for risk-rejected choices. Entity USD/share values are never summed or weighted
together. Unconstrained winners maximize the primary mean before every risk cap; constrained winners maximize
that same mean subject to all caps, so the reported risk cost stays in primary USD/share.

The empty subset has zero spending and payoffs. Any sole structurally feasible subset retains its actual
payoff statistics, with itself as hindsight benchmark and zero regret. A negative loss limit may exclude
hold; no risk-feasible program is an explicit
result with empty constrained winners and no invented opportunity cost. Declared cash below the floor can
reject even the empty subset, while signed financing receipts may make a nonempty subset feasible at later
checkpoints. An initial opening breach rejects every subset because later flows cannot repair it. Comparison
uses `card::evaluate_feasible`; no fictitious hold action enters the statistics or regret benchmark.
If no subset is structurally feasible, both winner sets are empty, statistics
and tail losses are absent, and no risk cost or hindsight baseline is invented. This is distinct from
having structurally feasible choices that all violate the risk limit.

DESIGN (BD-229, §23.1–23.3, G-CAP-D): `recourse::evaluate` adds a separate exact two-stage comparison while
retaining the complete static result. Every candidate has one declared stage; every terminal state has one
declared observation. A policy fixes its initial subset before the observation and chooses one continuation
subset per observed class. If the information candidate is not selected, all states receive the same
unobserved continuation. Thus terminal states sharing an observation cannot receive different decisions.
Existing prerequisite links separately express whether an action requires purchase of the gate.

The information candidate must be initial, have positive planned cost and negative declared net NAV in every
state, and incur negative cumulative cash before the observation in every state. The observation is at the
start of a noninitial scheduled cash period. Recourse candidates have explicit zero cash flows before it.
All initial commitments, NAV/share changes and later contractual cash flows persist in every continuation,
including abandonment. No cash flow, study cost or share effect disappears when the policy changes course.
Initial candidates' later capital calls also persist after abandonment. A recourse candidate cannot
activate a call before observation, including a zero-contribution call with a contractual funding target.

Every realized path must satisfy budget, prerequisites, exclusions, cash floors and positive ending shares.
The union of mutually exclusive branch actions is not a realized portfolio and is not charged as one.
Cash and equity results are reused from the same state's static subset, preserving scenario dependence.
Risk and regret are calculated over the complete unconditional policy distribution, after structural
screening. Neither posterior-mean branch selection nor nodewise tail-risk constraints replace global CVaR.
Secondary entity ledgers follow the same realized subset/state lookup. Their initial NAV and share effects
survive abandonment. State-specific secondary share failures are copied only onto the paths that realize
them. Whole-subset risk flags are never copied: every entity's policy statistics, regret and tail test are
recomputed from the complete realized policy population before constrained winners and adaptive value.

Net adaptive value is the best policy mean minus the best static-subset mean, reported separately before
and after the same risk limit. This is a global comparison that may change the initial commitment; it is
not a same-initial-commitment information value. Study costs are already in those means, and the difference
must never be added to policy NAV again. Every static subset embeds as a constant-continuation policy.
If either comparison population has no admissible winner, the difference is unavailable, not measured
against an invented zero. Per-share objectives retain separate USD/share and total-USD quantities.

## 3. Authority, Epistemics & Representation

All imported values remain Assumed, Internal, Estimated and parametrically uncertain. A complete input
snapshot includes separate policy, shared-weight and candidate Claims, including every losing candidate,
plus the complete aggregate funding or scheduled funding Claim when declared. Funding values, cash
basis/window, opening date, period dates/floors, signed flow matrices, source anchors and full
metadata participate in the same parent, uncertainty and receipt binding.
An equity Claim similarly binds the entity, share class, unit scale, initial count, complete baseline-NAV
and candidate/state share-change matrices, per-share risk limit, valuation/payoff conventions and metadata,
including every never-selectable candidate. The selected scope statement is part of the same digest.
The output meets evidence, epistemic and knowledge states, joins confidentiality, and retains all input
parents, source locators and consistent UU definitions. Generic Claim metadata omitted from its standard
identity (UU exact bits, computed colours, support, representation, supersession and current flags) is bound
into the program digest together with all typed values and constraints. The unanchored derived output never
acquires a stronger validated regime. Every authority flag remains false.
The two-stage result additionally binds the complete information Claim, candidate stages, observation
partition/date/basis and the full static result. Its source assumptions remain uncalibrated; the model does
not infer signal accuracy, information rights, approval or a calendar of economic NAV realization.
Entity funding adds its complete graph, matrices, fixed-transfer activation, ownership, gate and cap Claim
to the same evidence and digest binding. Losing candidates and closed distribution gates remain bound.
Optional debt inputs remain inside that same entity Claim: every facility, cash/CFADS basis, opening
principal, rate, draw, payment cap, CFADS increment, coverage threshold, sweep fraction and explicit
zero-service permission is bound, including terms for losing candidates. Debt-specific scope text is
selected only when debt is declared; models without debt retain their existing receipts and report bytes.
Capital-call method, cash convention, sequence, activation, basis and complete state terms bind into that
same entity Claim, including inactive calls and losing candidates. Optional cash components and call arrays
are omitted when calls are absent, preserving legacy report bytes and scope text.
Each secondary valuation has its own Claim binding its complete equity declaration and candidate NAV matrix,
including never-selectable candidates and the entity's loss ceiling. Its original source hash, row anchors,
UU definitions and evidence metadata enter the same complete result binding. Secondary entities and rows are
canonicalized by identifier. When no secondary valuation is declared, all legacy tags, scopes and reports
remain unchanged.

## 4. Public API Surface

- `ProgramPolicy`, `CapitalCandidate`, `ProgramFunding`, `FundingSchedule`, `ProgramInputs`: typed monetary
  declarations wrapped in Claims; funding is optional and explicit. A schedule uses `CashPeriod`,
  `StateCashFlow` and `CandidateCashFlow`; assessments retain `CashScheduleCheckpoint` rows and minimum slack.
- `EquityBasis`, `CandidateShareChange`, `ExactPerShare`, `PerShareStats`, `EquityState`, `EquityAssessment`:
  optional source-bound equity inputs and exact per-share outcomes, distinct from total-USD quantities.
- `entity::valuation::{EntityValuation, CandidateNavChange, EntityValuationAssessment}`:
  `ProgramInputs.entity_valuations` holds zero to three secondary valuation Claims alongside required primary
  equity. Secondary entity names are distinct from each other and the primary entity. Each assessment retains
  the named entity/share class, complete `EquityAssessment`, exact per-share statistics, tail loss, cap and
  risk result. Optional report arrays are omitted when empty. With secondary valuations, the complete static
  plus policy population times shared states times primary-plus-secondary entities must not exceed 65,536
  valuation-state rows. This independent bound and all matrix bounds precede canonical tagging/enumeration;
  existing cash-work limits continue to apply separately.
- `entity::{EntityFunding, Entity, EntityPeriod, EntityFloor, EntityOpeningCash, EntityStateCashFlow,
  EntityCandidateCashFlow, EntityTransfer, EntityDistribution, EntityDistributionGate}`: the complete
  entity funding declaration in `ProgramInputs.entity_funding`. `EntityFundingAssessment` contains a
  state-indexed ledger, exact minimum slack, all fixed transfers, distributions and conservation rows.
  `EntityCashPhase` distinguishes opening, pre-distribution and closing checks. Every rejected choice
  retains the full assessment. Optional entity JSON keys are omitted for legacy models.
  At most four entities, sixteen periods and sixty-four transfers are admitted. With `E` entities,
  `P` periods, `T` transfers and `D` distributing entities, work per state is
  `E*(2P+1) + T + D*P + P`. All subsets times states times this work must fit 65,536 ledger rows.
  For recourse the bound covers static subsets plus complete policies. These bounds precede canonical
  admission/enumeration; output is never pruned to fit them.
- `entity::debt::{EntityDebtFunding, DebtFacility, DebtOpeningPrincipal, DebtBaselineFlow,
  DebtCandidateFlow}`: optional declared external senior debt, with at most one facility per entity and
  four facilities total. Full-period rates and minimum DSCR are exact ratios; CFADS may be signed;
  draws, principal and payment caps are nonnegative, and sweep fractions lie in `[0,1]`.
  `DebtServiceResult` retains the full principal/service/coverage/sweep calculation.
  `DebtCoverageStatus` distinguishes covered, below minimum and no service due, with no fabricated ratio.
  `DebtCashFlows` supplies optional checkpoint and conservation components; `DebtDistributionGate`
  preserves the declared permission beside the calculated effective gate. `EntityFundingState.debt`
  retains every facility/period result in canonical period/facility order. Debt adds `facilities*periods`
  rows per state to the same 65,536 total-work bound before canonical tagging or enumeration.
- `entity::capital_call::{EntityCapitalCallFunding, CapitalCall, CapitalCallState}`: optional pre-money
  subscription declarations. Each call requires a distributing entity and its existing modeled parent,
  a known period, optional known candidate, explicit basis and a sequence in `1..=64` unique within its
  entity/period. At most 64 calls are admitted, each with all shared states. `CapitalCallResult` retains
  declared and actual contributions, active status, funding surplus, prior/ending ownership and its exact
  conservation residual. `CapitalCallCashFlows` separates internal parent receipts/payments from outside
  contributions on checkpoints and period conservation. Every declared call adds one row per state to
  the existing combined static/policy work bound before canonical tagging or enumeration.
- `evaluate(&ProgramInputs) -> Result<EvaluatedProgram, Refusal>`: exact exhaustive comparison, bounded to
  1–8 candidates and 1–256 shared states (at most 256 subsets). Schedules admit 1–16 periods and at most
  65,536 checkpoints across all subsets: `2^candidates × states × (periods + 1)`. Bounds apply before schedule
  canonical serialization and enumeration. Oversized work refuses; no state, period or subset is truncated.
- `EvaluatedProgram::{inputs, summary, claim, digest, no_claim, to_json, to_markdown}`: immutable input/result access
  and source-complete reports. Monetary output is rational major USD, without implicit rounding.
  Without a schedule or equity declaration, no new optional JSON keys are inserted and the existing scope
  text and receipts remain unchanged. Scheduled and equity models bind their explicit scope into the receipt.
  Equity JSON labels the objective and all total-USD versus USD/share statistics; Markdown shows all equity
  inputs and every bundle/state ledger, including undefined ratios and rejected bundles.
- `recourse::{RecourseInputs, RecourseBasis, DecisionStage, CandidateStage, StateObservation}`: a source-bound
  information declaration plus the existing `ProgramInputs`. Static public types and receipt paths remain
  unchanged. `recourse::evaluate(&RecourseInputs)` returns `EvaluatedRecourseProgram`, with immutable inputs,
  static comparator, policy summary, Claim, digest, JSON and Markdown.
- `recourse::{PolicyBranch, PolicyState, PolicyRejection, PolicyChoice, RecourseSummary}`: distinct policy
  outputs retain initial choices/spending, each observation's probability and continuation, every state's
  selected candidates/spending/slack/NAV, all cash/equity rows, structural reasons, global risk and co-winners.
  No public scalar subset selection or cost is repurposed as a union or expectation across branches.
  At most four observation classes and 256 policies are admitted. With `f` initial candidates, `r` recourse
  candidates and `k` classes, policy count is `2^(f-1) * (2^r + 2^(r*k))`. The combined static and policy
  checkpoint count `(2^(f+r) + policies) * states * (periods + 1)` must not exceed 65,536. Bounds precede
  canonical tagging/enumeration; rejected policies are retained, never pruned to satisfy work limits.
  Entity-funded policies reuse the entire same-state entity ledger of each realized subset. Recourse native
  flows and candidate-triggered transfers must be zero before observation. The information gate must incur
  negative net native external cash across the entities in every state before observation; an internal
  transfer alone does not buy information. Initial obligations remain in every realized entity path.
  Debt-funded recourse additionally requires every pre-observation candidate debt draw, scheduled-principal
  cap increment and CFADS increment to be individually zero. Offsetting increments cannot hide an early
  decision. The catalog copies the selected subset's same-state debt ledger, retaining future obligations
  from initial commitments even after abandonment.
- `model::parse(text, SourceAnchor)`: verifies SHA-256 against actual bytes, then reads the strict
  static-only `fmine.capital-program.v1` TOML subset; staged tables refuse. Top-level strings are `schema`,
  `model_key`, `question`, `currency`,
  `valuation_basis`, `budget`, `tail_mass`, `payoff_convention`, `reference_class_basis`, with optional
  `max_tail_loss`. Candidate rows require `id`, `label`, `cost`, `reference_class_basis` and optionally
  `exclusion_group`; separate `requires` rows name candidate/prerequisite pairs. State rows name id/weight;
  payoff rows name candidate/state/value; UU rows name id/description/trigger/downside_usd_m.
  Optional `[[funding]]` requires `cash_period`, `cash_basis`, `minimum_cash`; `[[available_cash]]` requires
  `state`, `value`; `[[cash_requirement]]` requires `candidate`, `state`, `value`. At most one funding
  declaration is allowed. `cash_period` is an ordered valid `YYYY-MM-DD/YYYY-MM-DD` range, without an
  inferred calendar. Cash amounts and their basis apply to that window independently of NAV valuation.
  Instead, `[[funding_schedule]]` requires `cash_basis`, `opening_date`; `[[cash_period]]` requires `id`,
  `cash_period`, `minimum_cash`; `[[opening_cash]]` requires `state`, `value`; `[[baseline_cash_flow]]` requires
  `period`, `state`, `value`; `[[candidate_cash_flow]]` requires `period`, `candidate`, `state`, `value`.
  Every opening state, period/state baseline and period/candidate/state flow is explicit, including zeros.
  Periods start at `opening_date` and continue without date gaps or overlaps. Rows may be supplied in any
  order; canonical ordering follows chronological period, then candidate and state identifiers.
  Alternatively, `[[entity_funding]]` requires `cash_basis`, `opening_date`, `objective_entity`.
  `[[entity]]` requires `id`, `label`; `[[entity_period]]` requires `id`, `cash_period`;
  `[[entity_floor]]` requires `entity`, `period`, `value`; `[[entity_opening_cash]]` requires `entity`,
  `state`, `value`; `[[entity_baseline_cash_flow]]` requires `entity`, `period`, `state`, `value`;
  `[[entity_candidate_cash_flow]]` requires `entity`, `period`, `candidate`, `state`, `value`.
  `[[entity_transfer]]` requires `id`, `from_entity`, `to_entity`, `period`, `basis`, with optional
  activating `candidate`; `[[entity_transfer_amount]]` requires `transfer`, `state`, `value` for every
  transfer/state. `[[entity_distribution]]` requires `entity`, `parent_entity`, `parent_share`, `basis`;
  the ownership share is an exact quoted `n/d` in `[0,1]`. `[[entity_distribution_gate]]` requires
  `entity`, `period`, `state`, quoted `allowed = "true" | "false"`, `maximum_distribution` for every
  distributing entity/period/state. All entity cash matrices include explicit zeros. The three funding
  declarations are mutually exclusive. The separate entity Claim retains the original source hash,
  line anchors and UU metadata; unknown, duplicate and orphan entity rows refuse. Matrix/output bounds
  are checked before canonical tagging, and finalization invokes the same typed core admission rules.
  Optional `[[entity_debt]]` requires `cash_flow_convention = "native_excludes_modeled_debt"` within entity
  funding. `[[debt_facility]]` requires `id`, `entity`, `basis`, `cfads_basis`, `minimum_dscr`,
  `sweep_fraction`, `allow_distributions_without_service`; the ratios are quoted `n/d` and the last field
  is exactly quoted `"true"` or `"false"`. `[[debt_opening_principal]]` requires `facility`, `state`,
  `value`; `[[debt_service]]` requires `facility`, `period`, `state`, `interest_rate`, `draw`,
  `principal_payment_cap`, `cfads`; `[[candidate_debt_service]]` requires `facility`, `period`,
  `candidate`, `state`, `draw`, `principal_payment_cap`, `cfads`. All matrices are complete, including
  explicit zeroes. Debt rows require both entity/debt declarations and retain their original line anchors
  in the existing entity Claim; unknown, duplicate, orphan and malformed rows refuse.
  Optional `[[equity_basis]]` requires `entity`, `share_class`, `shares_per_unit`, `initial_share_units` and
  accepts `max_tail_loss_per_share`. `[[baseline_equity_nav]]` requires `state`, `value` for every state;
  `[[candidate_share_change]]` requires `candidate`, `state`, `value` for every candidate/state pair,
  including zeroes. Share units, their scale and the per-share limit use exact signed quoted `n/d` strings;
  a decimal or bare integer is not accepted. Baseline NAV uses the existing monetary input format.
  Money uses quoted major USD decimals with at most four fractional digits, weights and tail mass exact
  positive `n/d`. The parser bounds text to 4 MiB, rows to 45,056 and UU terms to 64. Core admission and
  parsing bound identifiers to 128 bytes and descriptions, bases, source paths and locators to 4096 bytes,
  preventing one large identifier from expanding into every enumerated subset's state statistics.
  Optional secondary `[[entity_valuation]]` requires `entity`, `share_class`, `valuation_basis`,
  `shares_per_unit`, `initial_share_units`, with optional `max_tail_loss_per_share`. Its independently named
  valuation basis accompanies the shared model/currency. `[[entity_baseline_nav]]` requires `entity`,
  `state`, `value`; `[[entity_candidate_payoff]]` and `[[entity_candidate_share_change]]` each require
  `entity`, `candidate`, `state`, `value`. NAV values use quoted USD decimals; share quantities and caps use
  exact quoted signed ratios. Every matrix entry is explicit, including zeroes. Primary equity is required;
  unknown, duplicate, foreign, orphan or incomplete secondary inputs refuse. Each entity's Claim retains
  only its own original valuation row anchors plus shared metadata/UU anchors and the complete file hash.
- `model::evaluate(text, SourceAnchor) -> Result<EvaluatedModel, Refusal>`: source-bound static or two-stage
  dispatch with `mode`, `no_claim`, `to_json` and `to_markdown` accessors. The same strict schema accepts
  one optional `[[recourse]]` with `information_candidate`, `observation_date`, `information_basis`;
  complete `[[candidate_stage]]` rows with `candidate`, `stage` (`initial` or `recourse`); and complete
  `[[state_observation]]` rows with `state`, `observation`. The information declaration becomes a separate
  Assumed Claim with its original line anchors and UU. Parsing checks the original complete source bytes,
  without stripping staged tables or manufacturing another file hash. Unknown, duplicate and orphan rows
  refuse. Static models retain their existing report bytes. Staged output uses `fmine.capital-recourse.v1`.

## 5. Guards & Typed Refusals

Every candidate is admitted before enumeration, including candidates that cannot be selected. Missing or
duplicate states, incomplete payoffs, foreign references, prerequisite cycles, duplicate prerequisites,
currency/basis mismatch, negative costs/budgets, nonpositive or non-unit-sum weights, and invalid tail mass
refuse. Funding additionally refuses partial/duplicate/foreign state matrices, negative cash amounts,
currency mismatch, invalid or reversed cash dates, absent bases, and orphan cash rows without a complete
funding declaration. Even never-selectable candidates require their complete explicit cash requirements.
Scheduled funding additionally refuses missing/duplicate/foreign period rows, incomplete signed flow
matrices, negative opening cash/floors, invalid dates, gaps/overlaps, and conflicts with aggregate funding.
Every candidate's cash flows remain required even when another constraint prevents its selection.
Equity additionally refuses missing/duplicate/foreign matrix entries, nonpositive initial shares or unit
scale, incompatible currency/model/valuation metadata, absent entity/share class, an incompatible payoff
convention, a legacy total-USD risk ceiling, and orphan equity rows without a complete declaration.
Secondary valuations also refuse absent primary equity, repeated primary/secondary entity names, absent
perimeter valuation bases, incomplete candidate NAV matrices, incompatible model/currency metadata, and
population limits exceeded by the combined static and policy results. Every secondary Claim must pass the
same source, freshness, uncertainty and evidence admission as the primary inputs.
Entity funding refuses incomplete/duplicate/foreign matrices, missing or mismatched objective entities,
invalid periods and currencies, negative opening balances/floors/transfer amounts/distribution caps,
unknown/self/cyclic distribution edges, ownership outside [0,1], absent transfer/distribution bases,
unknown activation candidates and gates without a distribution rule. It excludes both legacy funding modes.
Debt additionally refuses an unrecognized cash convention, more than one facility per debtor entity,
incomplete/duplicate/foreign facility matrices, negative opening principal/draws/payment caps/rates,
nonpositive coverage thresholds, sweep fractions outside `[0,1]`, currency mismatch or absent loan/CFADS
bases. Recourse debt draws, principal caps and CFADS increments must each be zero before observation.
Capital calls additionally refuse unknown methods/cash conventions, empty or oversized call populations,
unknown distributors/periods/candidates, repeated call ids or entity/period sequences, missing or repeated
states, nonpositive pre-money value, negative funding/contributions, currency mismatch and absent bases.
Declared calls activated by a recourse candidate must occur at or after the observation boundary.
Recourse additionally refuses incomplete/duplicate/foreign stage and observation mappings, an unknown or
noninitial information candidate, a missing paid gate, absent or mistimed cash schedules, pre-observation
recourse cash flows, and initial prerequisites that depend on a later decision. It admits at least one
initial and one recourse candidate. Information metadata must agree with the program's model/currency/basis.
Stale, altered, unobservable, uncited, unsupported, calibration-failed or unbudgeted Claims refuse.
Conflicting UU definitions refuse. Exact arithmetic overflow returns `FMINE-REF-207`; no alternative is
dropped to obtain a plausible result. Input/source errors use the existing refusal catalog, including
`FMINE-REF-505` for source SHA mismatch and `FMINE-REF-507` for altered typed Claims.

## 6. Proof Obligations

- Exhaustive output contains exactly `2^n` subsets; constraint rejection never edits the input population.
- Joint risk uses the shared scenario index, never marginal risk sums.
- Every feasible winner satisfies exact budget, dependency, exclusion, declared state-specific liquidity
  and optional tail-loss constraints. Equality with the cash floor remains feasible.
- Funding requirements are combined jointly by state, never averaged across states or charged to NAV again.
- Scheduled cash carries exactly within each same-state trajectory; later recovery never erases an earlier
  shortfall. Financing and repayment remain signed cash movements distinct from the declared NAV payoff.
- Equity valuation uses the selected bundle's same-state total NAV and actual shares; changing the declared
  share-unit scale with compensating counts leaves every per-share result unchanged (PO-CAP-2).
- USD funding and total-NAV reporting never enter the USD/share risk limit or objective without the explicit
  equity calculation. Share counts at or below zero remain visible structural rejections.
- Every secondary NAV/share calculation uses its own same-state numerator and actual-share denominator.
  All entity constraints share one structural comparison population; risk tests remain entity-specific and
  apply jointly, with no conversion or blending of denominators. Recourse copies state-specific values and
  reconstructs whole-policy risk instead of inheriting a subset's aggregate admission result.
- Feasible-set statistics retain actual negative-NAV survivors, with no invented zero-payoff competitor.
- All ties survive, and arithmetic risk cost is nonnegative whenever constrained winners exist.
- Source, losing candidate, constraint, adjustment-basis and evidence metadata changes affect identity.
- A continuation is constant within each information set, and all first-stage commitments survive in every state.
- Policy funding and structural constraints hold on each realized path; global risk uses the original state weights.
- The static feasible set embeds in the policy set, so every available net adaptive value is nonnegative.
- Cash cannot move between entities except through declared transfers, capital calls and distributions.
  Ownership alone never creates a funding obligation or credits outside shareholders' cash to the parent.
- Every period conserves native external cash after explicitly accounting for outside distributions and,
  when declared, outside call funding, external loan draws, interest, scheduled principal and sweeps.
  Principal conserves exactly.
- Every call preserves total ownership at one, carries the exact updated fraction into subsequent calls
  and dividends, and retains full contributions on underfunded or unaffordable paths. Required-raise
  shortfalls reject only policies that realize that same call/state, independent of existing JV cash.
- Coverage uses the declared same-state CFADS and scheduled service; cash reserves, sweeps and loan draws
  do not silently change that numerator or denominator. Zero service has no numerical ratio.
- Each entity's sweep consumes eligible child receipts before its own dividend, including roots with no
  outgoing ownership edge. Mandatory service/floor failures remain after subsequent child receipts.
- Entity floor failures and their phases remain on the realized policy state; another state's failure
  cannot reject an otherwise feasible adaptive continuation.

## 7. Failure Modes & Deterministic Fallbacks

Invalid input and arithmetic overflow refuse the complete comparison. Structural and risk infeasibility
are positive report states. No omitted candidate, state, cash flow, interaction or adjustment is inferred.
Internal temporary bundle Claims keep all input parents and one locator per source artifact to avoid
quadratic duplication of source metadata; the public result retains every original locator and input Claim.

## 8. Gates & Kill Criteria

Direct GB-30 known answers use the plan's explicitly illustrative, displayed inputs: A+D mean $241.18M
and risk cost $130.82M before adding streamed E, without treating rounded plan prose as an exact oracle.
Tests also cover fractional tail mass, dependence, exact boundaries, prerequisite and exclusion constraints,
do-nothing, infeasible risk, metadata and source identity, input permutation and overflow. Funding tests cover
shared-state demand dependence, cost overruns changing the selected subset, exact cash-floor boundaries,
preexisting cash shortfalls rejecting do-nothing, complete matrices, independent cash basis, unchanged legacy
economics, losing-candidate requirements and complete funding metadata binding.
Schedule tests cover early shortfalls versus aggregate solvency, carry, financing with hold rejected,
singleton and multiple negative-NAV survivors, exact minor-unit equality, changing floors, shared-state
dependence, chronology, complete matrices, source/metadata binding, overflow and combined work limits.
An independent Python Fraction oracle checks 44 legacy, scheduled and equity models through the actual binary;
JSON/Markdown integration checks retain every bundle's timeline/equity ledger and library/binary byte parity.
Equity known answers cover buyback accretion, issuance dilution, shared-state pairing, fractional tail mass,
share-unit invariance, nonpositive ending counts, negative singleton survivors, source identity and overflow.
Secondary valuation known answers cover a parent gain masking carve-out share dilution, simultaneous primary
and multiple-secondary caps, exact cap equality, fractional tail mass, share-unit invariance, and common
structural hindsight populations. The funded two-stage example chooses a fair Good-only deal under the
carve-out cap: Parent mean 2/5 USD/share, carve-out mean 89/220 and both tail losses 1/10. The aggressive
Good-only policy retains Parent mean 7/10 but carve-out tail loss 31/15, so its separate cap rejects it.
Regressions retain initial secondary NAV/share effects after abandonment and admit a Good-only continuation
despite a secondary share failure on its unrealized Bad path. Matrix, source, losing-input, metadata,
canonical-order, overflow and complete static/recourse work-bound checks accompany these known answers.
An independent Python Fraction oracle checks twelve secondary-valuation scenarios through the actual binary,
including global versus observation-conditional tail risk, coarse information, multiple entity caps and
composition with debt-funded policies. It derives each denominator from declared inputs, reconstructs every
static and adaptive path, and compares full ledgers, statistics, rejections and winners. Three CLI integrations
check JSON/Markdown parity and the original source bytes of `tests/capital/entity_valuation.toml`; that example
retains eight portfolios, twenty policies and Parent closing cash of 30/20 under the joint entity limits.
Recourse known answers cover paid study/build/abandon decisions, indistinguishable observations, sunk costs,
global rather than branchwise tail limits, path-specific prerequisites and exclusions, adaptive-only funding,
and equity issuance chosen by USD/share rather than total NAV. State-specific cash and share-count failures
remain on the paths that realize them. Admission, exact work limits, input permutation, losing-input and
metadata binding, and unavailable static comparisons have direct regressions.
An independent Python Fraction policy oracle exercises 15 two-stage scenarios through the actual binary,
including deferred initial obligations after abandonment. It computes every realized path directly from
the inputs and compares both the complete policy population and the static population. Integration tests
also check library/binary JSON and Markdown parity and execute the checked-in source-bound example.
The complete `tests/capital/equity.toml` example selects issuance plus project at exactly 1/2 USD per share;
its rejected standalone issuance loses 1/3 USD per share and its buyback adds 2/9 USD per share.
The complete `tests/capital/cash_schedule.toml` example selects bridge plus plant at USD 62.4M expected net delta-NAV.
The complete `tests/capital/recourse.toml` example buys the study, then issues shares and builds only after
Good, abandoning after Bad. Its exact mean is 19/120 USD/share with USD 120 total expected NAV change;
closing cash is USD 80 and USD 20 respectively. The best feasible fixed program is hold.
Entity known answers cover trapped cash reversing the selected investment, explicit partner/capital-call
shortfalls, caps and blocked distributions carried into later periods, exact one-third ownership cash,
multilevel child-first payouts and retained pre-distribution failures. The three-period policy pays for a
study, receives a conditional JV distribution, then builds only after Good: mean NAV 45, tail loss 5 and
adaptive value 45, with parent cash 30/20. Direct guards cover matrix completeness, cycles, units, source and
losing-input identity, permutation, overflow, pre-observation transfers and combined ledger-row bounds.
Capital-call known answers cover a parent top-up from 3/5 to 4/5, outside-only dilution to 3/10,
current and later dividends, exact ownership carry, explicit sequence versus source order, and inactive
terms retained beside zero settlement. Independent required-raise failures remain visible despite surplus
JV cash; full parent payments remain visible despite a floor breach and subsequent dividend recovery.
The two-stage regression keeps an initial later call after abandonment and excludes an unrealized state's
call shortfall from a feasible policy. Invalid terms/matrices/sequences, altered Claims and calls activated
by future decisions before observation refuse.
Debt known answers cover exact principal carry and falling interest after sweeps, coverage equality and
locked-cash carry, mandatory-service shortfalls, draw-before-interest and independent CFADS, final payoff
caps, explicit zero-service permissions, and nested intermediary/root sweeps of child receipts. The
two-period loan starts at 100: interest is 10 then 7, sweeps are 10 then 23/2, final principal is 77/2,
and Parent cash is 329/10. A coverage lock reverses the selected investment despite surplus group cash.
Static and adaptive comparisons retain full same-state debt paths; initial future principal/CFADS effects
persist after abandonment. Direct guards cover source/losing-input identity, all matrix fields, early
recourse debt effects, exact overflow and debt-inclusive static/combined output bounds.
An independent Python Fraction entity oracle checks 11 scenarios through the actual binary. It derives
native balances and fixed transfers directly from declared inputs and recursively solves each ownership
waterfall, comparing every checkpoint, transfer, distribution, conservation row, rejection and complete
static/policy statistic. CLI integrations compare full JSON and Markdown with the library and verify the
checked-in `tests/capital/entity_funding.toml` example's original source hash. That example combines the
three-period policy with 100 Parent shares: mean/adaptive value 9/20 USD/share and tail loss 1/20 USD/share.
The independent Python Fraction debt oracle derives principal/service recurrences and recursively solves
child receipts, sweeps and distributions from the inputs. It compares every debt field, checkpoint,
coverage gate, principal/cash residual, structural rejection and static/adaptive statistic. The complete
`tests/capital/debt_funding.toml` example observes Good/Bad, pays JV service, sweeps 50/0 and distributes
30/0 to Parent. The selected policy builds only after Good and retains initial final principal obligations
in both states: mean/adaptive value 9/20 USD/share, tail loss 1/20 USD/share, Parent cash 30/20, debt 29/79.
Library/binary integrations retain identical JSON/Markdown and the example's unchanged complete source bytes.
This bounded slice does not claim the full GATE-CAP-01/03/04 scale, selection-frequency or historical evidence.

## 9. Lifted Code & Foundation Dependencies

No new external dependencies. Reuses the exact units/rational arithmetic, strict registry TOML parser,
evidence Claim types, `fmine-decide::card` total-USD statistics and the bounded exact weighted-distribution
kernel in `fmine-numerics::risk::exact` for per-share statistics. No IO, spawning or time APIs occur in this crate.

## 10. Non-Goals & Scope Boundary

Static declared net-NAV comparison with optional aggregate or dated-checkpoint funding and exact equity NAV
per-share optimization, entity-specific cash/ownership distributions, and bounded two-stage policies with
purchased declared information. No additional
decision stages, undeclared project interactions, inferred observation likelihoods, price/fiscal model,
reference-class fitting, inferred JV dilution penalties or loan agreements,
blended or simultaneous optimization objectives, portfolio safety, calibrated probabilities or
physical authorization is supplied. Reference-class bases must be explicitly declared, including an honest
unadjusted basis; the system does not apply an unnamed adjustment. Selection frequency is explicitly
unavailable. The broad BD-229 CapitalProgram and production-scale gates remain open.
