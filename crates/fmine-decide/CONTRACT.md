# CONTRACT.md — `fmine-decide`

## 1. Purpose
The decision card and what feeds it:
- `card::evaluate` (BD-225, plan §22.5) compares declared alternatives over one finite, exactly weighted state population, from exact monetary Claims. It gives the mean, P10/P50/P90, CVaR95 of the signed loss, regret and rankings for every action, plus exact flip sensitivities and, optionally, exact information options (BD-227, §22.4);
- `card::model::parse` admits a `fmine.decision-model.v1` TOML model for it. `fmine decide card` in `fmine-cli` (`decision.rs`) is the caller;
- `DecisionCard::build` puts an expected-NPV `Claim<f64>` beside a CVaR loss, flip margins and ranked VOI campaigns, and takes its colour from the Claim;
- `render_one_pager_markdown` renders the §9.6 epistemic one-pager (BD-054);
- `DecisionCard::build_as_of` seals a retrospective card whose expected-NPV ancestry passed a temporal-cone check and whose output was replayed (BD-344, bd-r76w; G-HELM-4);
- `FlipMargin::from_linear_sensitivity` gives a linear break-even value;
- `rank_voi_campaigns` ranks campaigns by net VOI from `fmine_numerics::voi`'s validated finite evaluator, and keeps each campaign's conditional policy.

It does no I/O and never actuates anything. `card::evaluate` emits a draft Claim (§7); nothing else here emits one.

- **Crate:** `fmine-decide`
- **Layer:** E
- **Subsystem:** Helm
- **LOC budget:** 20,000 planned / 23,000 ceiling
- **Allowed Module Families:** `["card", "voi_rank", "one_pager"]`

## 2. Inputs
- **`card::evaluate`.** `DecisionInputs`: a `WeightSet` of strictly positive named state weights that sum exactly to one; at least two `ActionPayoffs`, exactly one with the do-nothing `ActionRole`, each a set of exact monetary Claims over the same states and one model, currency and valuation basis; optional `information_options`, each a `Claim<InformationOption>` with a complete exact `Likelihood`, explicit signals, test and delay costs and delay days.
- **`card::model::parse(text, source)`.** The model text and a `SourceAnchor` whose SHA-256 must equal the text's, with a non-empty path and locator. Weights and likelihoods are exact rationals, money has at most four decimal places, and every UU row needs a positive finite exposure.
- **`DecisionCard::build`.** A title, the recommended and runner-up actions as free text, the expected NPV as a `Claim<f64>` in USD millions, `cvar95_loss_usd_m` as a bare `f64`, `Vec<FlipMargin>` and `Vec<VoiRankedItem>`.
- **`FlipMargin::from_linear_sensitivity`.** A parameter name, its baseline value, the NPV advantage (USD millions), the NPV's derivative in the parameter, a unit string and the action it switches to. The numbers are bare `f64`.
- **`rank_voi_campaigns`.** A `fmine_numerics::voi::VoiProblem`, and `VoiCampaign { campaign_id, description, cost_usd_m, likelihood }` for each campaign. The cost is in the payoff unit (USD millions).
- **`build_as_of`.** A `fmine_claim::dag::TemporalGraph`, a `ConeCut`, and a `ReplayCertificate` whose generator returned `temporal_replay_bytes`.
- **No files.** The crate reads no files. `fmine decide card` reads the model file, at most 4 MiB, and binds its actual bytes by SHA-256.

## 3. Outputs
- **`EvaluatedCard`**, or a `Refusal`. It has immutable input and output accessors, the output `Claim<DecisionSummary>`, its weakest input colour, and JSON and Markdown renderers.
  - Per action: the exact weighted mean, inverse-CDF P10/P50/P90 and fractional-mass CVaR95 of the signed loss `−NPV`; regret against each state's best payoff, its mean and maximum; expected-value and minimax rankings, with every exact co-winner kept. Action-id order breaks presentation ties only.
  - `summary.sensitivity` compares the act-now expected-value primary with every rival. It gives the exact uniform extra cost to a tie and the minimum total-variation change in the state weights, with canonical weight transfers, a checked primal tie and a matching dual bound proving minimality.
  - `InformationSummary` gives EVPI and, per option, the exact preposterior EVSI, signal probabilities, posterior weights, conditional action values and co-winners. Unreachable signals stay, with null posteriors. Test and delay costs are charged once per option; net values stay signed. The report says `method=exact_finite_enumeration`, with the Monte Carlo standard error null.
  - The Claim's value tag and the versioned report digest also bind metadata that generic Claim identities omit: support, representation, UU values, supersession, current flags and computed input colours.
- **`DecisionCard`**, or a `Refusal`. Its `decision_id` is a `Decision` `ObjectId` over `title:recommended_action:claim_id`, and its `color` is the expected-NPV Claim's.
- **`RetrospectiveDecisionCard`**, or a `Refusal`. It has read-only `card`, `cone` and `replay` accessors and its own renderer, which puts a scoped "Historical check" section before the card.
- **`render_one_pager_markdown`** gives Markdown. Money is shown to 2 decimals, flip values to 3, and UU downsides to 1. Its VOI section renders each campaign's conditional policy by zero-based state and action index. It does not map those to the card's action labels, and it says when the prior action stays optimal after every signal.
- **`temporal_replay_bytes`** is a `DCRP` version 2 frame, an internal replay input and not a durable `FMT-CARD-1` encoding. It holds the rendered card, the Claim's canonical body, and the exact bits of every displayed number, the VOI totals and the whole retained policy.
- **`rank_voi_campaigns`** gives `Vec<VoiRankedItem>`, sorted by descending `net_voi` with ties broken by `campaign_id`, or a `Refusal`. Each item keeps its `VoiAnalysis` (prior action values, every tied optimal action, and per signal its probability, posterior, action values, tied optimal actions and EVSI contribution) and the `summary` copied from it. `benefit_cost_ratio` is `None` for a zero-cost campaign, including one with zero EVSI.

## 4. Invariants
- **A card needs a budgeted unknown and a colour.** `test_decision_card_uu_and_flip_margin_guards` refuses a missing UU term (204) and an unreviewed assertion (205). It also shows that a calibrated class without a named domain is `Estimated`, and that naming the domain and its calibration record makes it `Validated`.
- **A retrospective card binds its actual root and its replayed output.**
  - `retrospective_card_seals_actual_root_and_replayed_output`.
  - `retrospective_card_refuses_unrelated_root_and_omitted_parents`.
  - `retrospective_card_refuses_late_ancestor_and_changed_cut_or_cone`.
  - `retrospective_card_refuses_output_or_claim_edits_after_replay`.
  - A checked card cannot be replaced: the `compile_fail` doctest on `RetrospectiveDecisionCard`.
- **The replay bytes see edits the Markdown hides.** `temporal_replay_projection_binds_subdisplay_numeric_edits` and `replay_projection_binds_subdisplay_policy_and_summary_edits`.
- **VOI ranking refuses rather than ranking bad input.** The problem is validated once, even for an empty list, and every campaign needs a distinct non-empty id, a valid likelihood and a valid cost. Any failure refuses the whole ranking; nothing is clamped or scored 0. Tests: `voi_campaigns_rank_by_net_value`, `gb29_ranking_exposes_the_observation_that_changes_the_action` (EVSI $41M, net $31.2M at $9.8M), `campaign_and_action_ties_and_impossible_signals_are_retained`, `invalid_campaigns_refuse_the_whole_ranking` and `small_positive_cost_keeps_its_ratio_and_unrepresentable_ratios_refuse`.
- **`card::evaluate` is exact and refuses whole.** No action or state is dropped and no weight is normalised. Stale, unusable, uncited, unsupported or unbudgeted inputs, and arithmetic overflow, refuse the comparison. Tests in `card.rs` (15, among them `gb29_exact_payoffs_regret_quantiles_and_signed_loss`, `overflow_refuses_instead_of_saturating_or_dropping_an_action`, `garbling_never_increases_information_value` and `invalid_material_information_never_drops_the_option`), `card/model.rs` (10), `card/information/policy_risk.rs` (5) and `card/render/sensitivity.rs` (5, among them `exact_radius_bounds_every_independent_three_state_grid_stress`).
- **Information options.** Each state's signal probabilities lie in `[0, 1]` and sum exactly to one, with every zero given. Work is bounded to 64 options, 256 signals per option and 4 million state × signal × action terms. EVSI lies in `[0, EVPI]` exactly, and is 0 exactly when the act-now primary stays optimal after every reachable signal; a posterior tie that keeps it is not a change.
- **Sensitivity stress.** Stressed weights stay non-negative and sum to one; payoffs and the state population do not change. Baseline ties, strict pointwise dominance and ties with no possible strict overtaking are told apart. Information policies and minimax-regret choices are not re-optimised under the stress.
- **The binary and the library agree.** `crates/xtask/tests/decision_card.rs` checks the known answers against an independent Python oracle (`crates/xtask/testdata/decision_card_oracle.py`) and that `fmine decide card` renders the same JSON and Markdown as the library, with and without information options.

## 5. Refusals
- **`FMINE-REF-204`** (unbudgeted unknowns, `G-UU`; exit 3), from `build` and `build_as_of`: a non-`KnownKnown` Claim with no `UuTerm`.
- **`FMINE-REF-205`** (calibration required, `NL-4`; exit 3), from `build` and `build_as_of`: the Claim has no colour because calibration failed or it is an unreviewed `Assertion`. `build_as_of` also refuses a stored card colour that differs from the Claim's.
- **`FMINE-REF-507`** (proposed; exit 6), from `build_as_of`: the Claim's hashed body no longer matches its identity.
- **`FMINE-REF-110`** (proposed; exit 3), from `build_as_of`. It covers a missing or mismatched root or parents, temporal ancestry that is incomplete or after the cut, and a replay certificate for a different root, cut, cone or output. Missing node times are never inferred. Admissibility uses the per-cell record anchor, not wall-clock order between cells.
- **`FMINE-REF-109`** (proposed), from `rank_voi_campaigns`: an invalid problem, campaign id, likelihood or cost.
- **`FMINE-REF-207`** (proposed), from `rank_voi_campaigns`: a numerical failure of the VOI evaluator (`VoiError::is_numerical_failure`), or a positive-cost benefit/cost ratio that cannot be represented. From `card::evaluate`: an exact arithmetic limit in the sensitivity, shown as a visible sensitivity refusal with its remedy, beside the comparison it leaves intact.
- **`card::evaluate` and `card::model::parse`** refuse malformed, incomplete or unadmissible models and Claims whole; see their tests (§4).

## 6. Determinism class
- **`src/lib.rs`: DeterministicPerIsa**, from its `//! Determinism:` header. There is no cross-host digest test. The replay bytes carry exact `f64` bits, so a replay is compared on one host.
- **`card` and its submodules: Exact**, from their headers: rational weights, likelihoods, payoffs and costs, no simulation and no clamping. Inherited floating UU and colour metadata is kept by its exact bits and is not used as payoff input.

## 7. Evidence class of outputs
- The `DecisionCard` shows the expected-NPV Claim's evidence class, epistemic status, state and colour. It assigns none of them. The CVaR, flip margins and VOI fields carry no evidence class.
- `card::evaluate`'s output Claim meets the evidence of all its inputs and keeps every parent, source anchor, confidentiality label and consistent UU definition. Parsed model Claims stay internal, `Assumed` and uncalibrated, and no field can upgrade them. The comparison is a draft: no calibrated probability, Monte Carlo VOI accuracy, retrospective proof, physical feasibility, sign-off or authority, and no decision-ledger entry.

## 8. Validity domain
- **`FlipMargin` is linear.** A sensitivity with |d| ≤ 1e-12 gives `flip_value = baseline_value`, a made-up "flips at the baseline". The doc comment's `npv_advantage ≥ 0` is not checked, nor are non-finite inputs. One test case checks a value (0.78).
- **`build` takes the ancillary fields as given.** It does not check that `cvar95_loss_usd_m` is finite. Nothing ties the flip margins or VOI items to the two actions, and the runner-up may be any text.
- **VOI here is finite-model arithmetic**, conditional on the supplied prior and likelihood. It gives no Monte Carlo standard error and no calibration. Campaigns are ranked independently: no portfolio interaction, and a delay cost counts only if it is in the campaign's cost. Information options are single tests: no joint signals and no sequential or batched tests.
- **Sensitivity thresholds are conditional ties**, not probabilities of reversal, calibrated bounds, Wasserstein radii or protection against omitted states.
- **`build_as_of` covers the declared expected-NPV ancestry only.** The action text, CVaR, flip margins and VOI have no Claim provenance of their own. Their bytes are replayed, but that does not prove their historical sourcing or certify the whole decision; the renderer says so. All evidence reads must go through the supplied store view. A pure in-process checker cannot detect captured values or external I/O.
- **Colour is the NPV Claim's alone** on a `DecisionCard`. It inherits `fmine-evidence`'s `project_color`, including bd-wiaq (a Verified basis passes through for anchored model classes).

## 9. Oracles
- **Independent Python oracles.** `crates/xtask/testdata/decision_card_oracle.py` and `decision_sensitivity_oracle.py`, run by `crates/xtask/tests/decision_card.rs` against the library and the binary. `tests/decisions/expansion.toml` and `tests/decisions/metallurgical_test.toml` are the examples the CLI's robot docs name.
- **Hand values.** The flip value 0.78 in `test_decision_card_uu_and_flip_margin_guards`; GB-29 (plan §22.8: EVSI $41M, net $31.2M) in `gb29_ranking_exposes_the_observation_that_changes_the_action` and `gb29_exact_payoffs_regret_quantiles_and_signed_loss`.
- **Exhaustive.** `exact_radius_bounds_every_independent_three_state_grid_stress` checks the sensitivity radius over a grid.
- **Synthetic temporal controls.** The four `retrospective_card_*` tests and the two replay-projection tests build their graphs, cuts and stores in the test.
- No golden card.

## 10. Known gaps
- **Most `G-HELM` guards are not enforced on `DecisionCard`.**
  - G-HELM-4 is, through `build_as_of`.
  - `card::evaluate` requires a do-nothing alternative (G-HELM-2) and computes regret (G-HELM-7), and its output Claim meets all inputs' evidence. `DecisionCard::build` does none of these.
  - Nothing refuses a priced safety constraint (G-HELM-1). There is no banner for an `Assumed` top-3 sensitivity (G-HELM-5).
  - VOI is shown without a Monte Carlo SE or status, which G-HELM-6 refuses.
- **GATE-HELM-01 is not met.** It needs two real retrospective cards and an executive panel (bd-226). This crate has only synthetic controls.
- **GATE-HELM-02 is not met.** The Monte Carlo estimator and its χ² test exist in `fmine-numerics` (`voi::estimate_evsi_mc`, `test_mc_evsi_recovers_closed_form_chi2`), and the test passes locally. CI has not run it, there is no gate record, and bd-227 is open. This crate does not call the estimator: `rank_voi_campaigns` and `card` use exact finite arithmetic.
- **BD-225 is open (bd-225).** There are no golden cards and no decision-ledger entries.
- **No INV-002 flags.** An older contract said every card enforces `authoritative`, `may_actuate`, `is_safe_certification` and `is_signing_basis` false. No such fields exist in this crate, `fmine-claim` or `fmine-evidence`.
- **Two L-6 debt rows** in `registries/l6_allow.toml`, for `from_linear_sensitivity` (3 parameters) and `build` (`cvar95_loss_usd_m`).
- **The module doc swaps two BD ids.** It labels flip margins BD-227 and VOI ranking BD-225, but the plan's BD-225 is the card schema and BD-227 the VOI estimator.
- **The registry charter promises more than the crate has.** It calls this "the only franken-evidence/franken-decision adapter and renderer". There is no adapter.
- **The older contract named GATE-HELM-03**, which the plan does not have.

## 11. Public API
- **`card`**: `WeightSet`, `ActionRole`, `ActionPayoffs`, `DecisionInputs` (with `canonical_tag`), `evaluate(&DecisionInputs) -> Result<EvaluatedCard>`, `EvaluatedCard` (`inputs`, `claim`, `summary`, `weakest_input_color`, JSON and Markdown renderers), `ExactMoney`, `StateResult`, `AlternativeStats`, `DecisionSummary`, the `information` items it re-exports, and `card::model::parse`.
- **`FlipMargin { parameter_name, baseline_value, flip_value, units, switches_to_action }`** and `FlipMargin::from_linear_sensitivity(name, baseline, npv_advantage_usd_m, d_npv_d_param, units, switches_to_action)`.
- **`VoiCampaign { campaign_id, description, cost_usd_m, likelihood }`**, `VoiRankedItem { campaign_id, description, cost_usd_m, summary, analysis, benefit_cost_ratio: Option<f64> }` and `rank_voi_campaigns(problem, campaigns) -> Result<Vec<VoiRankedItem>, Refusal>`.
- **`DecisionCard { decision_id, title, recommended_action, runner_up_action, expected_npv_usd_m, cvar95_loss_usd_m, flip_margins, top_voi_items, color }`**, with:
  - `build(...) -> Result<Self, Refusal>`;
  - `build_as_of(self, graph, cut, replay) -> Result<RetrospectiveDecisionCard, Refusal>`;
  - `temporal_replay_bytes() -> Vec<u8>`;
  - `render_one_pager_markdown() -> String`.
- **`RetrospectiveDecisionCard`**, with `card()`, `cone()`, `replay()` and `render_one_pager_markdown()`.
- Depends on `fmine-core`, `fmine-units`, `fmine-time`, `fmine-evidence`, `fmine-claim`, `fmine-numerics`, `fmine-calib` and the pure `fmine-registry` TOML parser.

## 12. Proof obligations
The plan defines no `PO-DEC-*`. Older contracts' PO-DEC-1 to PO-DEC-3 were local labels, and their properties are tested here:
- the card's UU and colour refusals: `test_decision_card_uu_and_flip_margin_guards`;
- VOI ranking relays the validated evaluator's `0 ≤ EVSI ≤ EVPI`, keeps the posterior policy, and refuses invalid input whole instead of clamping or ranking part of it: the five VOI ranking tests (§4);
- a retrospective card binds its actual NPV Claim and its replayed output, and neither an unrelated clean root nor an older certificate is accepted: the four `retrospective_card_*` tests.

The plan's guard **G-HELM-4** is discharged by `build_as_of` over the declared expected-NPV ancestry, within the limits in Validity domain.
