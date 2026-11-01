# CONTRACT.md — `fmine-decide`

## 1. Purpose
The decision card and what feeds it:
- `DecisionCard::build` puts an expected-NPV `Claim<f64>` beside a CVaR loss, flip margins and ranked VOI campaigns, and takes its colour from the Claim;
- `render_one_pager_markdown` renders the §9.6 epistemic one-pager (BD-054);
- `DecisionCard::build_as_of` seals a retrospective card whose expected-NPV ancestry passed a temporal-cone check and whose output was replayed (BD-344, bd-r76w; G-HELM-4);
- `FlipMargin::from_linear_sensitivity` gives a linear break-even value;
- `rank_voi_campaigns` ranks campaigns by closed-form net VOI from `fmine_numerics::voi`.

It does no I/O, never actuates anything, and emits no Claim of its own.

- **Crate:** `fmine-decide`
- **Layer:** E
- **Subsystem:** Helm
- **LOC budget:** 20,000 planned / 23,000 ceiling
- **Allowed Module Families:** `["card", "voi_rank", "one_pager"]`

## 2. Inputs
- **`DecisionCard::build`.** A title, the recommended and runner-up actions as free text, the expected NPV as a `Claim<f64>` in USD millions, `cvar95_loss_usd_m` as a bare `f64`, `Vec<FlipMargin>` and `Vec<VoiRankedItem>`.
- **`FlipMargin::from_linear_sensitivity`.** A parameter name, its baseline value, the NPV advantage (USD millions), the NPV's derivative in the parameter, a unit string and the action it switches to. The numbers are bare `f64`.
- **`rank_voi_campaigns`.** A `fmine_numerics::voi::VoiProblem`, and `VoiCampaign { campaign_id, description, cost_usd_m, likelihood }` for each campaign.
- **`build_as_of`.** A `fmine_claim::dag::TemporalGraph`, a `ConeCut`, and a `ReplayCertificate` whose generator returned `temporal_replay_bytes`.
- **No files.** It reads no files or registries.

## 3. Outputs
- **`DecisionCard`**, or a `Refusal`. Its `decision_id` is a `Decision` `ObjectId` over `title:recommended_action:claim_id`, and its `color` is the expected-NPV Claim's.
- **`RetrospectiveDecisionCard`**, or a `Refusal`. It has read-only `card`, `cone` and `replay` accessors and its own renderer, which puts a scoped "Historical check" section before the card.
- **`render_one_pager_markdown`** gives Markdown. Money is shown to 2 decimals, flip values to 3, and UU downsides to 1.
- **`temporal_replay_bytes`** is a `DCRP` version 1 frame. It holds the rendered card, the Claim's canonical body, and the exact bits of every displayed number and VOI total.
- **`rank_voi_campaigns`** gives `Vec<VoiRankedItem>` (EVSI summary and `benefit_cost_ratio`), sorted by descending `net_voi` with ties broken by `campaign_id`, or a `VoiError`.

## 4. Invariants
- **A card needs a budgeted unknown and a colour.** `test_decision_card_uu_and_flip_margin_guards` refuses a missing UU term (204) and an unreviewed assertion (205). It also shows that a calibrated class without a named domain is `Estimated`, and that naming the domain and its calibration record makes it `Validated`.
- **A retrospective card binds its actual root and its replayed output.**
  - `retrospective_card_seals_actual_root_and_replayed_output`.
  - `retrospective_card_refuses_unrelated_root_and_omitted_parents`.
  - `retrospective_card_refuses_late_ancestor_and_changed_cut_or_cone`.
  - `retrospective_card_refuses_output_or_claim_edits_after_replay`.
  - A checked card cannot be replaced: the `compile_fail` doctest on `RetrospectiveDecisionCard`.
- **The replay bytes see edits the Markdown hides.** `temporal_replay_projection_binds_subdisplay_numeric_edits`.
- **VOI ranking refuses rather than ranking bad input.** `test_voi_campaign_ranking_gate_helm_02` checks the order and `0 ≤ EVSI ≤ EVPI` on two campaigns, and refuses a likelihood row summing to 1.1 and a negative cost (bd-a482).

## 5. Refusals
- **`FMINE-REF-204`** (unbudgeted unknowns, `G-UU`; exit 3), from `build` and `build_as_of`: a non-`KnownKnown` Claim with no `UuTerm`.
- **`FMINE-REF-205`** (calibration required, `NL-4`; exit 3), from `build` and `build_as_of`: the Claim has no colour because calibration failed or it is an unreviewed `Assertion`. `build_as_of` also refuses a stored card colour that differs from the Claim's.
- **`FMINE-REF-507`** (proposed; exit 6), from `build_as_of`: the Claim's hashed body no longer matches its identity.
- **`FMINE-REF-110`** (proposed; exit 3), from `build_as_of`. It covers a missing or mismatched root or parents, temporal ancestry that is incomplete or after the cut, and a replay certificate for a different root, cut, cone or output. Missing node times are never inferred. Admissibility uses the per-cell record anchor, not wall-clock order between cells.
- **`VoiError`** (no refusal code), from `rank_voi_campaigns`: see `fmine-numerics`.

## 6. Determinism class
**DeterministicPerIsa**, from the `//! Determinism:` header of `src/lib.rs`. There is no cross-host digest test. The replay bytes carry exact `f64` bits, so a replay is compared on one host.

## 7. Evidence class of outputs
The card shows the expected-NPV Claim's evidence class, epistemic status, state and colour. It assigns none of them. The CVaR, flip margins and VOI fields carry no evidence class.

## 8. Validity domain
- **`FlipMargin` is linear.** A sensitivity with |d| ≤ 1e-12 gives `flip_value = baseline_value`, a made-up "flips at the baseline". The doc comment's `npv_advantage ≥ 0` is not checked, nor are non-finite inputs. One test case checks a value (0.78).
- **`build` takes the ancillary fields as given.** It does not check that `cvar95_loss_usd_m` is finite. Nothing ties the flip margins or VOI items to the two actions, and the runner-up may be any text.
- **VOI is closed-form only**, over the inputs `fmine_numerics::voi` accepts. A campaign costing at most 1e-9 $M gets a `benefit_cost_ratio` of 0, a made-up value that the replay bytes encode.
- **`build_as_of` covers the declared expected-NPV ancestry only.** The action text, CVaR, flip margins and VOI have no Claim provenance of their own. Their bytes are replayed, but that does not prove their historical sourcing or certify the whole decision; the renderer says so. All evidence reads must go through the supplied store view. A pure in-process checker cannot detect captured values or external I/O.
- **Colour is the NPV Claim's alone.** It inherits `fmine-evidence`'s `project_color`, including bd-wiaq (a Verified basis passes through for anchored model classes).

## 9. Oracles
No fixture under `fixtures/` and no golden card.
- **Hand values.** The flip value 0.78 in `test_decision_card_uu_and_flip_margin_guards`, and the two-campaign ranking in `test_voi_campaign_ranking_gate_helm_02`.
- **Synthetic temporal controls.** The four `retrospective_card_*` tests and `temporal_replay_projection_binds_subdisplay_numeric_edits` build their graphs, cuts and stores in the test.

## 10. Known gaps
- **Most `G-HELM` guards are not enforced.**
  - Only G-HELM-4 is, through `build_as_of`.
  - Nothing refuses a priced safety constraint (G-HELM-1), a card with no do-nothing alternative (G-HELM-2), or a colour above the weakest material input (G-HELM-3).
  - There is no banner for an `Assumed` top-3 sensitivity (G-HELM-5).
  - VOI is shown without a Monte Carlo SE or status, which G-HELM-6 refuses.
  - There is no regret (G-HELM-7).
- **GATE-HELM-01 is not met.** It needs two real retrospective cards and an executive panel (bd-226). This crate has only synthetic controls.
- **GATE-HELM-02 is not met.** It needs a Monte Carlo VOI estimator that recovers closed-form EVSI by a χ² test (bd-227). Only the closed form exists.
- **BD-225 is open (bd-225).** There are no golden cards and no decision-ledger entries.
- **No INV-002 flags.** The old contract said every card enforces `authoritative`, `may_actuate`, `is_safe_certification` and `is_signing_basis` false. No such fields exist in this crate, `fmine-claim` or `fmine-evidence`.
- **Two L-6 debt rows** in `registries/l6_allow.toml`, for `from_linear_sensitivity` (3 parameters) and `build` (`cvar95_loss_usd_m`).
- **The module doc swaps two BD ids.** It labels flip margins BD-227 and VOI ranking BD-225, but the plan's BD-225 is the card schema and BD-227 the VOI estimator.
- **The registry charter promises more than the crate has.** It calls this "the only franken-evidence/franken-decision adapter and renderer". There is no adapter, and robustness views are not built.
- **No caller.** `fmine-cli` depends on the crate but does not use it.
- **The old contract named GATE-HELM-03**, which the plan does not have.

## 11. Public API
- **`FlipMargin { parameter_name, baseline_value, flip_value, units, switches_to_action }`** and `FlipMargin::from_linear_sensitivity(name, baseline, npv_advantage_usd_m, d_npv_d_param, units, switches_to_action)`.
- **`VoiCampaign { campaign_id, description, cost_usd_m, likelihood }`**, `VoiRankedItem { campaign_id, description, cost_usd_m, summary, benefit_cost_ratio }` and `rank_voi_campaigns(problem, campaigns) -> Result<Vec<VoiRankedItem>, VoiError>`.
- **`DecisionCard { decision_id, title, recommended_action, runner_up_action, expected_npv_usd_m, cvar95_loss_usd_m, flip_margins, top_voi_items, color }`**, with:
  - `build(...) -> Result<Self, Refusal>`;
  - `build_as_of(self, graph, cut, replay) -> Result<RetrospectiveDecisionCard, Refusal>`;
  - `temporal_replay_bytes() -> Vec<u8>`;
  - `render_one_pager_markdown() -> String`.
- **`RetrospectiveDecisionCard`**, with `card()`, `cone()`, `replay()` and `render_one_pager_markdown()`.

## 12. Proof obligations
The plan defines no `PO-DEC-*`. The old contract's PO-DEC-1 to PO-DEC-3 were local labels, and their properties are tested here:
- the card's UU and colour refusals: `test_decision_card_uu_and_flip_margin_guards`;
- VOI ranking refuses invalid input, and `0 ≤ EVSI ≤ EVPI` holds on valid input up to rounding and the likelihood row-sum tolerance (no clamp since bd-a482): `test_voi_campaign_ranking_gate_helm_02`;
- a retrospective card binds its actual NPV Claim and its replayed output, and neither an unrelated clean root nor an older certificate is accepted: the four `retrospective_card_*` tests.

The plan's guard **G-HELM-4** is discharged by `build_as_of` over the declared expected-NPV ancestry, within the limits in Validity domain.
