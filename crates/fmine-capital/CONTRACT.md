# CONTRACT.md — `fmine-capital`

## 1. Charter & Layer

E-layer Helm capital-program comparison, within the planned 20k/23k LOC budget. These bounded BD-229 slices
enumerate declared static subsets and two-stage policies, using the permitted `fmine-capital -> fmine-decide` edge.
- **Allowed Module Families:** `["model", "render", "recourse"]`
The static kernel lives in the crate root; `recourse` composes its state-level results into complete policies.

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

Every realized path must satisfy budget, prerequisites, exclusions, cash floors and positive ending shares.
The union of mutually exclusive branch actions is not a realized portfolio and is not charged as one.
Cash and equity results are reused from the same state's static subset, preserving scenario dependence.
Risk and regret are calculated over the complete unconditional policy distribution, after structural
screening. Neither posterior-mean branch selection nor nodewise tail-risk constraints replace global CVaR.

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

## 4. Public API Surface

- `ProgramPolicy`, `CapitalCandidate`, `ProgramFunding`, `FundingSchedule`, `ProgramInputs`: typed monetary
  declarations wrapped in Claims; funding is optional and explicit. A schedule uses `CashPeriod`,
  `StateCashFlow` and `CandidateCashFlow`; assessments retain `CashScheduleCheckpoint` rows and minimum slack.
- `EquityBasis`, `CandidateShareChange`, `ExactPerShare`, `PerShareStats`, `EquityState`, `EquityAssessment`:
  optional source-bound equity inputs and exact per-share outcomes, distinct from total-USD quantities.
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
- `model::parse(text, SourceAnchor)`: verifies SHA-256 against actual bytes, then reads the strict
  `fmine.capital-program.v1` TOML subset. Top-level strings are `schema`, `model_key`, `question`, `currency`,
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
  Optional `[[equity_basis]]` requires `entity`, `share_class`, `shares_per_unit`, `initial_share_units` and
  accepts `max_tail_loss_per_share`. `[[baseline_equity_nav]]` requires `state`, `value` for every state;
  `[[candidate_share_change]]` requires `candidate`, `state`, `value` for every candidate/state pair,
  including zeroes. Share units, their scale and the per-share limit use exact signed quoted `n/d` strings;
  a decimal or bare integer is not accepted. Baseline NAV uses the existing monetary input format.
  Money uses quoted major USD decimals with at most four fractional digits, weights and tail mass exact
  positive `n/d`. The parser bounds text to 4 MiB, rows to 45,056 and UU terms to 64. Core admission and
  parsing bound identifiers to 128 bytes and descriptions, bases, source paths and locators to 4096 bytes,
  preventing one large identifier from expanding into every enumerated subset's state statistics.

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
- Feasible-set statistics retain actual negative-NAV survivors, with no invented zero-payoff competitor.
- All ties survive, and arithmetic risk cost is nonnegative whenever constrained winners exist.
- Source, losing candidate, constraint, adjustment-basis and evidence metadata changes affect identity.
- A continuation is constant within each information set, and all first-stage commitments survive in every state.
- Policy funding and structural constraints hold on each realized path; global risk uses the original state weights.
- The static feasible set embeds in the policy set, so every available net adaptive value is nonnegative.

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
Recourse known answers cover paid study/build/abandon decisions, indistinguishable observations, sunk costs,
global rather than branchwise tail limits, path-specific prerequisites and exclusions, adaptive-only funding,
and equity issuance chosen by USD/share rather than total NAV. State-specific cash and share-count failures
remain on the paths that realize them. Admission, exact work limits, input permutation, losing-input and
metadata binding, and unavailable static comparisons have direct regressions.
The complete `tests/capital/equity.toml` example selects issuance plus project at exactly 1/2 USD per share;
its rejected standalone issuance loses 1/3 USD per share and its buyback adds 2/9 USD per share.
The complete `tests/capital/cash_schedule.toml` example selects bridge plus plant at USD 62.4M expected net delta-NAV.
This bounded slice does not claim the full GATE-CAP-01/03/04 scale, selection-frequency or historical evidence.

## 9. Lifted Code & Foundation Dependencies

No new external dependencies. Reuses the exact units/rational arithmetic, strict registry TOML parser,
evidence Claim types, `fmine-decide::card` total-USD statistics and the bounded exact weighted-distribution
kernel in `fmine-numerics::risk::exact` for per-share statistics. No IO, spawning or time APIs occur in this crate.

## 10. Non-Goals & Scope Boundary

Static declared net-NAV comparison with optional aggregate or dated-checkpoint funding and exact equity NAV
per-share optimization, plus bounded two-stage policies with purchased declared information. No additional
decision stages, undeclared project interactions, inferred observation likelihoods, price/fiscal model,
reference-class fitting, entity waterfalls, portfolio safety, calibrated probabilities or
physical authorization is supplied. Reference-class bases must be explicitly declared, including an honest
unadjusted basis; the system does not apply an unnamed adjustment. Selection frequency is explicitly
unavailable. The broad BD-229 CapitalProgram and production-scale gates remain open.
