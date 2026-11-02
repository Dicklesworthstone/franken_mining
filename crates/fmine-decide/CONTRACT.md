# CONTRACT.md — `fmine-decide`

## 1. Charter & Layer
- **Crate:** `fmine-decide`
- **Layer:** `E` (Enterprise & Decision)
- **Subsystem:** `Enterprise`
- **LOC Budget:** `16k planned / 18k ceiling` (`registries/loc_budget.toml`)
- **Allowed Module Families:** `["card", "voi_rank", "one_pager"]`

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
(`BD-344`, §22.6). It checks the expected-NPV Claim's canonical identity, graph node kind and immediate parents,
then checks every declared ancestor against a decision timestamp and per-cell record anchor. A successful
`fmine_claim::dag::replay::ReplayCertificate` must bind the same root, cut, checked cone and card output. The replay
generator returns `DecisionCard::temporal_replay_bytes()`: rendered Markdown, the NPV Claim body and exact bits of
displayed numeric values, VOI calculation totals and the complete retained conditional policy. Thus changing a
value beneath the displayed decimal precision cannot reuse a proof. Unrendered Claim metadata is subject to the
existing Claim canonical-body limits.
The projection is an internal versioned replay input (version 2), not a new durable `FMT-CARD-1` encoding.

## 3. Authority, Epistemics & Representation
Enforces `INV-002` (`authoritative: false`, `may_actuate: false`, `is_safe_certification: false`, `is_signing_basis: false`) and `INV-015` (non-empty `uu_register`, `FMINE-REF-204`) on every `DecisionCard`.

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
  visible, with act-now first only for presentation. No joint signal distribution, sequential/batched tests,
  or test-policy risk/regret is inferred. Existing base action risk/regret remains separate and unchanged.
  Every option's complete Claim metadata participates in output evidence, parents, sources, UU, value tag
  and digest. JSON/Markdown report `method=exact_finite_enumeration`, Monte Carlo standard error as null/not
  applicable, and the uncalibrated-model boundary. Determinism is Exact; this does not pass `GATE-HELM-02`.
- `card::model::parse` admits the strict `fmine.decision-model.v1` TOML subset with explicit `state`, `action`,
  `payoff` and `uu` rows plus optional `information_option`, `signal` and `likelihood` rows. It checks the
  supplied source SHA-256 against the actual text, exact rational weights and likelihoods,
  at most four decimal money places, complete action/state membership and explicit positive finite UU exposure.
  All parsed Claims remain internal, Assumed and uncalibrated. Unknown fields cannot upgrade those labels.
- `FlipMargin`, `DecisionCard` (`build`, `build_as_of`, `temporal_replay_bytes`)
- `RetrospectiveDecisionCard`: immutable `card`, `cone` and `replay` accessors; scoped Markdown renderer
- `VoiCampaign`, `VoiRankedItem`, `rank_voi_campaigns -> Result<Vec<VoiRankedItem>, Refusal>`
- `render_one_pager_markdown`

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
