# CONTRACT.md — `fmine-decide`

## 1. Charter & Layer
- **Crate:** `fmine-decide`
- **Layer:** `E` (Enterprise & Decision)
- **Subsystem:** `Helm`
- **LOC Budget:** `20k planned / 23k ceiling` (`registries/loc_budget.toml`)
- **Allowed Module Families:** `["bounds", "card", "voi_rank", "one_pager"]`

## 2. Mathematical & Domain Formulation
Implements `DecisionCard` (`FMT-CARD-1`), `FlipMargin`, `rank_voi_campaigns` (`GATE-HELM-02`), and the
CEO/CFO/Mine-GM 1-Page Executive Decision Card Markdown renderer (`render_one_pager_markdown`, `§9.6`).

`rank_voi_campaigns` evaluates finite-state information experiments against one validated decision problem.
Each experiment uses a state-by-signal likelihood matrix and a finite nonnegative cost in the payoff unit
(USD millions). Ranking is by descending net VOI, with campaign identifier breaking ties. The result retains
the prior action values and all optimal action indices, plus each signal's model probability, posterior state
probabilities, action expected payoffs, tied optimal actions and EVSI contribution. Impossible signals have no
posterior or action payoff vector, no optimal action and zero contribution. Zero-cost campaigns have no defined
benefit/cost ratio, including a zero-EVSI campaign; positive costs are never rounded down to zero for this ratio.
The compatibility `summary` field is copied from the retained analysis, without repeating the evaluation.

The Markdown policy names zero-based state and action indices in the supplied problem. It does not invent a
mapping from those alternatives to the card's action labels. All probabilities and information values remain
conditional on the declared finite model; rendering them does not promote evidence, calibration or authority.
A prior-optimal action that remains optimal for every possible signal is reported explicitly.

`DecisionCard::build_as_of` consumes an ordinary card and returns an immutable `RetrospectiveDecisionCard`
(`BD-344`, `bd-r76w`, §22.6; the plan's guard `G-HELM-4`). It checks the expected-NPV Claim's canonical identity, graph node kind and immediate parents,
then checks every declared ancestor against a decision timestamp and per-cell record anchor. A successful
`fmine_claim::dag::replay::ReplayCertificate` must bind the same root, cut, checked cone and card output. The replay
generator returns `DecisionCard::temporal_replay_bytes()`: rendered Markdown, the NPV Claim body and exact bits of
displayed numeric values, VOI calculation totals and the complete retained conditional policy. Thus changing a
value beneath the displayed decimal precision cannot reuse a proof. Unrendered Claim metadata is subject to the
existing Claim canonical-body limits.
The projection is an internal versioned replay input (version 2), not a new durable `FMT-CARD-1` encoding.

## 3. Authority, Epistemics & Representation
`FMINE-REF-204` refuses a non-`KnownKnown` Claim with no UU term. The card type has no `authoritative`, `may_actuate`, `is_safe_certification` or `is_signing_basis` field. Those flags are not enforced here: they are not fields of `DecisionCard`, and this tree does not define them in `fmine-claim` or `fmine-evidence`.

## 4. Public API Surface
- DESIGN (BD-225): `card::{WeightSet, ActionRole, ActionPayoffs, DecisionInputs, evaluate, EvaluatedCard}`
  computes a complete finite-scenario draft comparison from exact monetary Claims. Strictly positive named
  state weights sum exactly to one; at least two alternatives, exactly one explicit do-nothing role, complete
  identical state populations and one model/currency/valuation basis are required. `canonical_tag` and the
  Claim identity are both checked; stale, unusable, uncited, unsupported or unbudgeted inputs refuse the
  entire comparison. No action or state is dropped and no weight is normalized implicitly.
  The exact weighted mean, inverse-CDF P10/P50/P90 and fractional-mass CVaR95 of signed loss `-NPV` are computed
  for every action. Regret is each state's best-in-hindsight payoff minus that action's payoff over the same
  full matrix. Mean and maximum regret, expected-value and minimax rankings, and every exact co-winner remain
  visible. Action-id ordering breaks presentation ties only. Negative tail loss is retained for all-gain cases.
  `EvaluatedCard` has immutable input/output accessors and JSON/Markdown renderers. Its output Claim meets the
  evidence of all inputs and retains every parent, source anchor, confidentiality label and consistent UU
  definition. The derived Claim value tag and versioned report digest additionally bind metadata omitted by generic Claim identities,
  including support, representation, UU values, supersession, current flags and computed input colours.
  Arithmetic is Exact; inherited floating UU/colour metadata is retained by its exact bits, not used as payoff
  inputs. This draft comparison supplies no calibrated probabilities, Monte Carlo VOI accuracy, retrospective evidence proof,
  physical feasibility, sign-off or authorization, and is not a durable decision-ledger entry.
- DESIGN (BD-229): `card::evaluate_feasible` compares the nonempty action set retained by an external
  constraint evaluator, with the same complete-state, canonical Claim, metadata and exact arithmetic
  admission. It permits zero or one declared do-nothing role and refuses information options. It neither
  proves physical feasibility nor supplies a missing alternative. The ordinary `card::evaluate` contract
  remains at least two alternatives and exactly one do-nothing role. A sole feasible action retains its
  actual monetary mean, quantiles and tail loss, with itself as hindsight benchmark and zero regret.
  `DecisionSummary::runner_up` is optional: a singleton serializes it as null and reports sensitivity
  unavailable because there is no challenger. Existing comparisons retain their runner-up string and
  canonical JSON bytes. Multiple feasible actions compute regret only against that supplied set, even
  when every action has a negative payoff and doing nothing was excluded by the caller's constraints.
- DESIGN (BD-225/BD-227): the serialized `summary.sensitivity` and Markdown flip-margin section compare the
  displayed act-now expected-value primary with every rival, not only the runner-up. They compute the exact
  uniform extra cost to a tie and the minimum total-variation change in the shared state weights, retaining
  every equal weakest challenger. Stress weights may reach zero but remain nonnegative and sum to one;
  payoffs and the declared state population do not change. Each positive-radius result carries canonical
  weight transfers and a checked primal tie with a matching dual lower bound proving minimality. Baseline
  ties, strict pointwise dominance, and ties without possible strict overtaking are distinct. The complete
  sensitivity result participates in the existing output Claim value tag and report digest. An exact
  arithmetic limit produces a visible `FMINE-REF-207` sensitivity refusal with its remedy, retaining the
  underlying comparison rather than substituting an approximate or saturated margin. These thresholds are
  conditional ties, not probabilities of reversal, calibrated confidence bounds, Wasserstein radii or
  protection against omitted states and unknown unknowns. Information policies and minimax-regret choices
  are not reoptimized under this stress. No input is mutated or new decision-ledger authority created.
- DESIGN (BD-227, §22.4): optional `DecisionInputs::information_options` contains source-bound
  `Claim<InformationOption>` values with a complete exact `Likelihood` matrix, explicit signals, nonnegative
  test and delay costs, and independently declared delay days. All options, including losers, must pass the
  same canonical identity, citation, evidence, support, state and UU admission as the base inputs. The model,
  currency and valuation basis must match; each state's conditional signal probabilities lie in [0,1] and
  sum exactly to one, with every zero supplied explicitly. Missing cells or malformed options refuse the
  complete comparison. Work is bounded to 64 options, 256 signals per option and 4 million total
  state/signal/action terms; exact arithmetic overflow refuses rather than approximating or dropping data.
  `InformationSummary` computes EVPI and each option's exact preposterior EVSI, signal probabilities,
  posterior state weights, conditional action values and all co-winners. Unreachable signals remain visible
  with null posterior/policy values. Strict change is relative to the fixed act-now presentation primary;
  a posterior tie retaining it is not a change. Exact checks require 0 ≤ EVSI ≤ EVPI and zero EVSI iff that
  baseline stays optimal after every reachable signal. Test and delay costs are charged once per option;
  net VOI and net policy value remain signed. Delay duration implies no discounting or state transition.
  Overall expected-value rankings compare acting now with buying at most one option; all exact ties remain
  visible, with act-now first only for presentation. The same admitted marginal payoff laws also produce a
  separate signed-loss CVaR95 ordering across every act-now action and one-test policy, plus an exact
  expected-value/CVaR95 Pareto frontier. Lower CVaR95 loss is better and every exact tail-risk tie remains.
  A choice leaves the frontier only when another admitted choice has weakly higher expected value and weakly
  lower CVaR95 loss with at least one strict improvement. Exact loss probability is retained beside each
  comparison point but never combined into an invented scalar utility or lexicographic recommendation.
  No joint signal distribution, sequential/batched tests, or cross-option regret is inferred. Existing base
  action regret remains separate and unchanged.
  Every option's complete Claim metadata participates in output evidence, parents, sources, UU, value tag
  and digest. JSON/Markdown report `method=exact_finite_enumeration`, Monte Carlo standard error as null/not
  applicable, and the uncalibrated-model boundary. Determinism is Exact; this does not pass `GATE-HELM-02`.
- `card::information` types. `InformationSummary` holds `baseline_primary`, `baseline_winners`,
  `baseline_value`, `perfect_information_value`, `evpi`, `options` (one `InformationStats` each), `option_order`,
  `option_winners`, `overall_order` and `overall_winners` (`ExpectedChoice`: a `ChoiceKind` of `ActNow` or
  `InformationOption`, an id and an expected value), and `risk_comparison`.
  - `InformationStats` carries an option's costs, delay days, gross and net value, EVSI, net VOI, `NetStatus`,
    strict-change probability, one `SignalPolicy` per signal (posterior weights and `PosteriorActionValue`s, `None`
    for an unreachable signal), and `policy_risk`.
  - `PolicyRisk` is the exact law of that option's net payoff when each reachable signal's `selected_action` is
    followed (`POLICY_RULE`, `RISK_SCOPE`). It holds the mean, P10/P50/P90, signed-loss CVaR95, loss probability,
    the ascending `PolicyPayoffMass` distribution, and the reachable signals whose posterior ties could change the
    tail.
  - `risk_comparison` is a `RiskComparison` over every act-now action and every one-test policy, each a
    `RiskChoice` (kind, id, expected value, CVaR95 loss, loss probability). It holds `tail_risk_order` (lower
    CVaR95 loss first), `tail_risk_winners` (every exact co-winner) and `expected_value_cvar95_frontier`. The JSON
    carries the ranking rule beside them; the Markdown has an "Expected value versus realized tail risk" section.
  - Constants: `MAX_INFORMATION_OPTIONS` (64), `MAX_SIGNALS` (256), `MAX_INFORMATION_WORK` (4,000,000),
    `METHOD`, `STANDARD_ERROR_NOTE` and `SCOPE`.
- `card` result types: `ExactMoney`, `StateResult`, `AlternativeStats` and `DecisionSummary`, under
  `EvaluatedCard`; constants `MAX_STATES` (4,096), `MAX_ACTIONS` (256), `NO_CLAIM` and `TIE_RULE`.
  `EvaluatedCard` has `inputs`, `claim`, `summary`, `weakest_input_color`, `to_json` and `to_markdown`.
  `ActionRole`, `NetStatus` and `ChoiceKind` have `as_str`.
- `card::model::parse` admits the strict `fmine.decision-model.v1` TOML subset with explicit `state`, `action`,
  `payoff` and `uu` rows plus optional `information_option`, `signal` and `likelihood` rows. It checks the
  supplied source SHA-256 against the actual text, exact rational weights and likelihoods,
  at most four decimal money places, complete action/state membership and explicit positive finite UU exposure.
  All parsed Claims remain internal, Assumed and uncalibrated. Unknown fields cannot upgrade those labels.
- `FlipMargin`, `DecisionCard` (`build`, `build_as_of`, `temporal_replay_bytes`)
- `RetrospectiveDecisionCard`: immutable `card`, `cone` and `replay` accessors; scoped Markdown renderer
  (`render_one_pager_markdown`), which puts a "Historical check" section before the card
- `VoiCampaign`, `VoiRankedItem`, `rank_voi_campaigns -> Result<Vec<VoiRankedItem>, Refusal>`
- `render_one_pager_markdown`
- `card::stability::{SelectionStabilityDesign, SelectionStabilityProblem, StabilityAlternative,
  StabilityTailLimit, SelectionScope, SelectionStabilitySummary}`: DESIGN (BD-229, §22.5), source-bound
  conditional bootstrap robustness for complete comparison populations. `evaluate` admits a Claim of
  the original model, structural eligibility, named state law, objective unit and complete outcome laws.
  It returns a Claim retaining source ancestry, confidentiality, uncertainty and the weakest evidence;
  its tag binds the complete input metadata, source model, conditional result and scope boundary.
  It reports exact co-winner frequencies, nominal runner-up ties, fragile nominal choices below 4/5,
  candidate inclusion bounds and every replicate's counts/winners. Capital static/adaptive adapters are
  the first consumer. `ordered_values` aligns named outcomes to the original support without dropping
  missing, duplicate or extra rows. The result never changes nominal economics or asserts authorization.

  Structural eligibility is fixed over the complete original support; zero-count states do not waive
  cash, share or contractual constraints. Every replicate recomputes mean and all signed tail-loss
  constraints from shared count weights, using pre-sorted numerical payoff laws. Additional entity laws
  constrain separately and never enter the primary objective. All exact ties survive. Alternative
  inclusion frequencies may sum above one; candidate lower/upper bounds mean inclusion in every/any
  nonempty co-winner set. Policy initial and realized-state selections are distinct scopes, including
  continuations on unsampled states. Definite/possible flip bounds describe choices outside the nominal
  co-winner set, not arbitrary tie-break changes within that set. No-feasible replicates remain in every
  denominator, are reported separately and never become an invented zero-value winner.

  The declared `weighted_multinomial` experiment has an unsigned 64-bit seed, 200–2,000 replicates,
  1–4,096 draws each, at most 8,000,000 total draws and 32,000,000 state/law/alternative/replicate
  evaluations. State/candidate/alternative identifiers are at most 128 ASCII letters, digits, dots,
  underscores or hyphens. The conservative replicate count × sum of all alternative identifier bytes
  cannot exceed 8,000,000, bounding retained winners even when every alternative ties. These bounds
  precede canonical hashing. The exact sampler uses existing Philox with
  independent replicate streams; no floating weight approximation or global denominator LCM is used.
  `SelectionStabilityDesign` has canonical tags and JSON; the complete summary has JSON and Markdown.
  Counts, sources, seed and experiment size remain explicit. Frequencies are conditional sensitivity,
  not calibrated optimality probabilities, confidence intervals or empirical model-uncertainty coverage.
  Known-answer and metamorphic tests cover dominant negative choices, fragile choices, every tie,
  changing primary/secondary risk, no feasible choice, original state retention, source admission,
  arithmetic/work refusals and positive-affine invariance of complete replicate results.

- `card::robustness::{RobustnessDesign, StateDistance, RobustAlternative, RobustnessSummary}`:
  DESIGN (BD-225, §22.5), optional finite-support Wasserstein-1 comparisons through the existing
  `card::evaluate` and `fmine decide card` command. One source-bound `[[robustness]]` declaration
  requires `method = "wasserstein_1"`, a nonnegative rational `radius`, and explicit `metric_basis`
  and `radius_basis`. `[[transport_distance]]` supplies every ordered `from`/`to`/`value` pair,
  including zero diagonals. The dimensionless metric is symmetric, strictly positive between
  different states and satisfies every triangle inequality. No metric, radius or calibration is inferred.
  All controls and original matrix-row anchors remain in an Assumed/Internal input Claim.

  For action a, DESIGN: solve max Σᵢⱼ γᵢⱼ (−vₐⱼ) with γ ≥ 0, Σⱼ γᵢⱼ = pᵢ and
  Σᵢⱼ γᵢⱼ dᵢⱼ ≤ radius; its negative is the worst-case mean. Separately solve the same problem
  for same-state regret max_b(v_bⱼ) − vₐⱼ. All original actions enter the hindsight baseline.
  The dual is min_{λ ≥ 0} λ·radius + Σᵢ pᵢ maxⱼ(lossⱼ − λdᵢⱼ), the finite-support
  specialization of the Wasserstein dual (Mohajerin Esfahani and Kuhn, 2017, §4.1, equation 16;
  https://arxiv.org/abs/1505.05116v3). This implementation obtains and checks both witnesses
  directly; it does not import that paper's empirical-sample confidence guarantees.

  Maximin mean and minimax expected regret have separate rankings, complete co-winner sets and
  adversarial distributions. Every certificate retains all destination weights (including zeros),
  all positive transport arcs, transport cost, dual multiplier/potentials, objective and exact zero
  primal/dual gap. Nominal choices, regrets, tail statistics and information-option assessments stay
  unchanged and visible. The original robust mean/regret fields retain their **act-now action** scope.

  When information options are present, `FixedPolicyComparison` adds a joint maximin-mean ranking of
  act-now actions and the already selected one-test policies. For original state s and fixed nominal
  signal policy π, its conditional net value is gπ(s) = Σ_z P(z|s) v(π(z),s) − test_cost − delay_cost.
  The input likelihood matrix and original action payoffs supply this value; posterior means and
  aggregated marginal risk laws do not. Every signal, including impossible signals, retains its
  nominal selected action. Strictly positive original state weights ensure an impossible signal has
  zero likelihood in every original state and cannot become reachable by reweighting that support.
  The existing transport plan maximizes −gπ with the same state metric and radius. Nominal expectation
  must agree exactly with both the information option's net value and its policy-risk mean.
  Each `RobustInformationPolicy` retains the signal actions, posterior-tied signals, conditional state
  values and complete primal/dual certificate. `FixedPolicyChoice` qualifies identifiers by kind so
  an action and an information option with the same name remain distinct, and every co-winner stays
  visible. The fixed posterior tie resolution is not reconsidered under the stressed distribution.
  Conditional state means are not realized signal outcomes. This view does not optimize signal
  policies, alter test accuracy, infer cross-test regret or joint signals, or certify robust CVaR,
  hard constraints, missing regimes or empirical uncertainty.

  Work is bounded before cloning or hashing: 1–64 states, at most 256 actions, a complete distance
  matrix and at most 2,097,152 objective/state-pair evaluations: two objectives per act-now action
  and one per information policy. The existing information work bounds also apply. State and action
  identifiers are at most 128 ASCII identifier bytes and the declared bases at most 4,096 bytes.
  Metric validation and cost ordering are reused across all actions and policies. Each solve uses exact
  concave hull segments and returns at most n+1 positive arcs, without convergence tolerances.
  Configuration metadata joins output ancestry, confidentiality, UU and weakest evidence/colour.
  When the declaration is absent, the prior card fields, scope, Claim identity and report bytes remain
  unchanged. With no information options, the original robust JSON/Markdown bytes remain unchanged.
  Tests include independent primal-vertex enumeration, direct source-admission mutations,
  zero/full-radius limits, distinct robust criteria and actual CLI/library JSON/Markdown parity.
  Fixed-policy tests independently enumerate state/signal outcomes and the feasible two-state laws,
  reproduce a nominal 67.5 versus robust 27.5 test value that loses to a safe 30 action, reject changed
  conditional models, retain fixed posterior ties and impossible signals, distinguish equal action
  and option identifiers, and exercise the additional transport work cap before Claim admission.

- DESIGN (BD-225/227, plan §22.4–5): `card::downside::{DownsideLimit, DownsideSummary}` adds an optional
  common downside screen through `DecisionInputs::downside_limit` and the existing `decide card` command.
  One `[[downside_limit]]` has exact monetary `floor` and rational `max_breach_probability` in [0,1].
  The parser binds both controls, model, currency and valuation basis in an Assumed/Internal Claim with
  the original row anchor. Normal canonical identity, evidence, staleness, UU and support admission apply
  before information or transport evaluation. No default threshold, normalization or currency change occurs.

  A breach is realized net payoff strictly below the floor; equality is not a breach and equality to the
  probability cap passes. For act-now actions each state contributes its event indicator. For fixed
  information policies each state contributes Σ_z P(z|s) 1[v(π(z),s) − test_cost − delay_cost < floor].
  Conditional expected payoffs are never thresholded. The existing source-admitted policy reconstruction
  retains nominal selected actions, posterior tie choices and unreachable signals, and costs apply once.
  Nominal probabilities average this vector under original weights. If a robustness declaration exists,
  the same already validated transport plan maximizes it and returns a complete probability-valued
  primal/dual certificate. A further objective per original action and information policy is counted in
  both parser and library work limits before solving. Geometry and cost ordering are reused.

  `DownsideSummary` retains every kind-qualified choice, each state's conditional breach probability,
  nominal/stressed probabilities and eligibility, and separate nominal/stressed eligible rankings.
  Both rankings maximize nominal expected value, preserve every exact co-winner and report the loss
  relative to the unconstrained maximum nominal value. An empty eligible set is `NoEligibleChoice`, with
  empty ranks and null best-value/cost fields; doing nothing is never inserted as a fallback. Ordinary
  action statistics, nominal information results and the separate maximin-value view remain unchanged.
  Omitting the limit preserves prior output bytes and Claim identities. This fixed-policy screen does not reoptimize policies,
  supply calibrated probabilities, bound loss severity/CVaR, certify physical feasibility or authorize action.
  Known answers include the synthetic 1/8 nominal and 3/16 stressed test-policy loss probabilities, the
  inclusive 3/16 limit, exact floor-neighbor changes, no eligible choice, zero/full-radius limits, source
  mutations, early work bounds and an independent original-TOML state/signal plus primal-vertex oracle.

- DESIGN (BD-225/227, plan §22.4–5): `card::downside::policies` optionally optimizes deterministic
  signal-to-action mappings for each one-test option. Add one `[[policy_optimization]]` row with
  `method = "exact_deterministic"`, `screen = "nominal"` or `"wasserstein"`, and explicit integer
  `max_policies` and `max_work` budgets. An existing `[[downside_limit]]` and at least one information
  option are required. The Wasserstein screen additionally requires the existing complete declared
  transport geometry. The configuration belongs to the limit Claim, including its original row
  locator, canonical identity, assumed evidence and confidentiality; no new authority is introduced.

  Every policy chooses one original action per reachable signal, before learning the true state.
  Its exact nominal expected net payoff is maximized subject to the original strict-floor event and
  inclusive probability cap. Nominal screening averages the realized event over state/signal pairs.
  Wasserstein screening maximizes the conditional breach vector over state weights, with the original
  signal likelihoods fixed, and retains the winning policy's exact transport certificate. Costs are
  paid once in every outcome, including do nothing. CVaR is reported for the representative's nominal
  realized law but is neither the optimized objective nor a constraint. Randomized and sequential
  policies are outside this slice.

  Before any policy enumeration, checked multiplication counts the complete population across every
  option. The limits are 65,536 total mappings and 4,000,000 conservative work cells. Work reserves
  each policy's reachable-signal/state cells, state-weight cells and, for the stressed screen,
  transport-objective state pairs, plus source-matrix and representative-risk cells. Exceeding either
  declared budget refuses the whole request with FMINE-REF-203; no partial optimum is emitted. The
  search streams action-index tuples instead of allocating the exponential population. Unreachable
  signals select no action and do not multiply the count. Exact co-optimal mappings are counted,
  with the lexicographically first action tuple in sorted reachable-signal order as representative.

  The optional `downside.policy_optimization` output retains per-option feasible/optimal counts,
  the complete representative mapping, exact objective and probabilities, realized payoff law,
  original fixed EV-policy comparison, and the gain over the best eligible act-now alternative.
  It separately ranks original act-now choices and optimized options. All-infeasible results have
  no fabricated mapping, winner or improvement. Existing fixed-policy statistics remain unchanged;
  omitting the opt-in preserves their prior output bytes and Claim identities. The invented
  `tests/decisions/constrained_testwork.toml` case gives an unconstrained mean 34 at breach 1/10,
  versus the feasible optimum 57/2 at breach 1/100 under a 1/20 cap, with act-now hold worth zero.
  Tests also cover strict floors, once-only costs, robust equality, unreachable signals, exact ties,
  no feasible choice, source identity and budget limits, plus independent twenty-atom exhaustive
  oracles over 24 distinct finite models. These conditional calculations do not establish calibration,
  physical feasibility, operational authority or completion of GATE-HELM-02.

## 5. Guards & Typed Refusals
`rank_voi_campaigns` validates the shared problem once, even for an empty campaign list. Every campaign must
have a distinct nonempty identifier, a well-formed likelihood matrix and a valid cost. Any failure returns a
refusal for the entire request; it never returns a partial ranking or substitutes a zero score:

- `FMINE-REF-109`: invalid problem, campaign identity, likelihood or cost.
- `FMINE-REF-207`: numerical evaluation failure, including an unrepresentable positive-cost benefit/cost ratio.

`DecisionCard::build` relays the claim's refusals:
- `FMINE-REF-204` (unbudgeted unknowns, `G-UU`), exit 3: a non-`KnownKnown` claim with no `UuTerm`.
- `FMINE-REF-205` (calibration required, `NL-4`), exit 3: an `expected_npv_usd_m` with no colour to show, because
  calibration failed or it is an unreviewed `Assertion`.

`build_as_of` repeats the UU and colour guards, rejects a changed stored colour, and additionally refuses:
- `FMINE-REF-507`, exit 6: the NPV Claim's current hashed body does not match its identity.
- `FMINE-REF-110`, exit 3: missing or mismatched Claim root/parents, incomplete or post-cut temporal ancestry,
  or a replay certificate for a different root, timestamp, anchor, cone or output. Missing node times are never
  inferred. Record admissibility uses the per-cell anchor, not wall-clock ordering between cells.

## 6. Proof Obligations (`PO-*`)
- `PO-DEC-1`: `DecisionCard::build` refuses a card whose claim has an empty `uu_terms` (`FMINE-REF-204`) or no colour
  (`FMINE-REF-205`).
- `PO-DEC-2`: `rank_voi_campaigns` relays the validated finite evaluator's `0 <= EVSI <= EVPI` guarantee, retains
  the posterior action policy and refuses invalid inputs instead of clamping or returning a partial ranking.
  The GB-29 closed-form test checks EVSI `$41M`, net VOI `$31.2M` with the declared `$9.8M` combined cost, and the
  high/low observation action switch. This is a finite-model known answer, not the Monte Carlo `GATE-HELM-02`.
- `PO-DEC-3`: retrospective construction binds the actual NPV Claim and the replayed output; neither an unrelated
  clean root nor an older certificate for edited output is accepted. The checked snapshot has no mutable accessor.
  Tests: the four `retrospective_card_*` tests in `lib.rs`, and the `compile_fail` doctest on
  `RetrospectiveDecisionCard`, which fails to compile an assignment to its private `card` field. This discharges
  the plan's guard `G-HELM-4` over the declared expected-NPV ancestry only, within the limits in §10.

## 7. Failure Modes & Deterministic Fallbacks
Invalid or unanchored decision inputs fail closed with typed `Refusal`.

## 8. Gates & Kill Criteria (`GATE-*` / `KILL-*`)
- `GATE-HELM-02`
- `GATE-HELM-03`

## 9. Lifted Code & Foundation Dependencies
- Depends on `fmine-core`, `fmine-units`, `fmine-time`, `fmine-evidence`, `fmine-claim`, `fmine-numerics`,
  `fmine-calib` and the pure `fmine-registry` TOML parser.

## 10. Non-Goals & Scope Boundary
Never actuates physical equipment (`INV-002`) or performs file/network I/O (`L-3`).

Finite-state VOI is not a Monte Carlo estimator and supplies no Monte Carlo standard error, Bayesian evidence
for the supplied prior/likelihood, or calibrated predictive probability. The K-problem / 100-seed statistical
`GATE-HELM-02` and real retrospective cards remain separate requirements. Campaign ranking does not model
portfolio interactions; a delay cost affects the result only if included in the supplied campaign cost.

The retrospective proof covers the **declared expected-NPV Claim ancestry**. The existing action text, CVaR,
flip margins and VOI fields have no individual Claim provenance. Their bytes are replayed, but this does not prove
complete historical sourcing for those fields or certify the whole decision. The renderer states this boundary.
The replay contract requires all evidence reads to use the supplied store view; captured values or external I/O
cannot be detected by a pure in-process checker. This slice supplies the guard and synthetic controls, not the
two real retrospective cards required by GATE-HELM-01. No cross-host replay result is asserted;
`DeterministicPerIsa` remains the crate's class. Ordinary `build` retains its existing Claim guards; its VOI
section now renders the retained conditional action policy.

## 11. Limits kept from the local outline

`registries/loc_budget.toml` gives this crate layer E, subsystem Helm, 20,000 planned lines and a 23,000 ceiling. The `DecisionCard` shows the expected-NPV Claim's evidence class, epistemic status, state and colour. It assigns none of them. The CVaR, flip margins and VOI fields carry no evidence class.

`FlipMargin::from_linear_sensitivity` is linear. A derivative whose magnitude is at most 1e-12 yields `flip_value = baseline_value`. The doc comment's non-negative NPV advantage is not checked, and non-finite inputs are not checked. `DecisionCard::build` does not check that `cvar95_loss_usd_m` is finite, and it does not tie the flip margins or the VOI items to the two named actions.

`DecisionCard::build` does not require a do-nothing alternative and does not compute regret. `card::evaluate` does both. Nothing in `DecisionCard::build` refuses a priced safety constraint, and there is no banner for an assumed top-3 sensitivity. VOI is shown without a Monte Carlo standard error. GATE-HELM-01 still needs two real retrospective cards. There is no golden card and no decision-ledger entry. The module docs label flip margins BD-227 and VOI ranking BD-225. The plan's BD-225 is the card schema and BD-227 is the VOI estimator.

The local outline names these tests for the properties above: `test_decision_card_uu_and_flip_margin_guards`, `retrospective_card_seals_actual_root_and_replayed_output`, `retrospective_card_refuses_unrelated_root_and_omitted_parents`, `retrospective_card_refuses_late_ancestor_and_changed_cut_or_cone`, `retrospective_card_refuses_output_or_claim_edits_after_replay`, `temporal_replay_projection_binds_subdisplay_numeric_edits`, `replay_projection_binds_subdisplay_policy_and_summary_edits`, `voi_campaigns_rank_by_net_value`, `gb29_ranking_exposes_the_observation_that_changes_the_action`, `campaign_and_action_ties_and_impossible_signals_are_retained`, `invalid_campaigns_refuse_the_whole_ranking`, `small_positive_cost_keeps_its_ratio_and_unrepresentable_ratios_refuse`, `gb29_exact_payoffs_regret_quantiles_and_signed_loss`, `overflow_refuses_instead_of_saturating_or_dropping_an_action`, `garbling_never_increases_information_value`, `invalid_material_information_never_drops_the_option`, and `exact_radius_bounds_every_independent_three_state_grid_stress`. The known-answer files named there are `crates/xtask/testdata/decision_card_oracle.py` and `decision_sensitivity_oracle.py`, driven by `crates/xtask/tests/decision_card.rs`.

The merge ec60a12 dropped two test families from this list; bd-coro restores them. `PolicyRisk` is tested in `card/information/policy_risk.rs` (5): `actual_gb29_policy_has_more_tail_loss_despite_positive_net_information_value`, `fractional_tail_boundary_and_inverse_cdf_endpoints_are_exact`, `all_gain_signed_losses_and_uniform_cost_translation_are_preserved`, `incomplete_mass_wrong_mean_and_unrepresentable_net_outcomes_refuse` and `full_model_retains_unreachable_signals_and_exposes_risk_relevant_posterior_ties`. `risk_comparison` is tested in `card/information/ranking.rs` (4): `frontier_keeps_expected_value_tail_tradeoffs_and_drops_dominated_choices`, `tail_ranking_retains_exact_ties_and_uses_value_only_for_presentation`, `equal_expected_value_with_strictly_lower_tail_loss_dominates` and `exact_loss_probability_distinguishes_zero_from_negative_payoff`. The other counts are `card.rs` 15, `card/model.rs` 10, `card/render/sensitivity.rs` 5 and `lib.rs` 12.

`registries/l6_allow.toml` in this tree has two `fmine-decide` rows, for `from_linear_sensitivity` and for `build` (`cvar95_loss_usd_m`).

## 12. Untested-ore recovery bounds (DESIGN; BD-351.S1, plan §17.4)

`bounds::evaluate` accepts one source-bound `Claim<RecoveryBoundModel>`. Tested and untested strata
have explicit closed recovery intervals, a named route and metal, an additivity basis, an untested-bound
basis, and one positive named UU scenario. Weights are either a contained-metal fraction interval or
explicit independent dry-tonne and grade intervals in grams per dry metric tonne. Tonnes alone are not
metal weights. A box admitting zero combined contained metal refuses.

Exact rational arithmetic enumerates all eight endpoint vertices and checks the closed-form extrema of
`R = (1-w) * R_tested + w * R_untested`. The card retains witnesses and reports stable proceed, stable
stop or unstable. Proceed means recovery is at least the declared threshold; stop is strictly below it.
Decisive test conditions narrow only the admitted untested-recovery interval. Zero untested weight is
handled without division, and unattainable tests are reported explicitly. The uncertain-weight example
has bounds [19/25, 917/1000], a decisive proceed lower bound >=19/25 and a decisive stop upper bound
strictly <34/75. Equality at 34/75 cannot guarantee stop. Declared interactions refuse additive averaging.

`bounds::model::parse` reads strict `fmine.recovery-bounds.v1` worksheets with exact quoted quantities,
no implicit physical-range defaults, verified source bytes and original-token anchors. JSON and Markdown
retain inputs, results, weights, threshold, witnesses, sources, UU and an immutable digest binding the
full Claim metadata. Outputs inherit confidentiality and UU; evidence meets ModelledUncalibrated.
Parsed declarations remain Assumed and Internal. Bounds are not probabilities or measured recovery;
no calibration, disclosure, reserve classification, VOI price or execution authority is supplied.

Tests cover independent known answers, the tonne-weight trap, threshold equality, interval narrowing,
zero-weight and impossible-test cases, enclosure over 10,000 Philox completions, source/metadata binding
and refusal of invalid inputs. The synthetic examples are `tests/decisions/untested_ore.toml` and
`tests/decisions/untested_ore_tonnes.toml`. Evidence-qualified testwork ingestion, BD-351.S2 coverage
wiring and the real metallurgical commitment/testwork choice remain open.

The CLI entry point is `fmine decide recovery-bounds <model-file> [--json|--format markdown]`.
`crates/xtask/tests/recovery_bounds.rs` independently checks endpoint arithmetic with Python Fraction,
strict threshold ties, source and UU identity, refusals and real-binary/library output agreement.
