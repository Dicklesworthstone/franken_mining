# CONTRACT.md — `fmine-metrics`

## 1. Purpose
The public-lane metric kernels. Each one takes supplied numbers and computes a mining metric or a check on one:
- AISC and AIC per ounce under a definition read from `registries/metrics.toml` (`definitions`);
- the published cut-off grades recomputed from their printed inputs (`cutoff`);
- an AISC accounting waterfall, as a legacy float LMDI and as a typed exact kernel with an additive fallback (`lmdi`, `lmdi::waterfall`);
- the production and AISC a year still needs to meet guidance, and whether the site's own history reaches it (`guidance`, `guidance::quarters`);
- an optimistic production ceiling from stated feed, grade and recovery (`capacity`);
- a grade-normalised recovery comparison (`recovery`);
- the declared arithmetic identities of a PDF table, such as a roll-forward, over agreed readings (`roll_forward`);
- whether the reporters of one asset agree on the gold it produced or sold, on one 100% basis (`mirror`);
- at the crate root: an AISC consolidation check, GEO under a price deck, reserve replacement and a float cut-off solver.

The results are descriptive arithmetic. None identifies an operational cause, forecasts, or proves physical feasibility. The crate does no I/O at run time.

- **Crate:** `fmine-metrics`
- **Layer:** E
- **Subsystem:** Bullion
- **LOC budget:** 18,000 planned / 21,000 ceiling
- **Allowed Module Families:** `["lmdi", "cutoff", "definitions", "guidance", "capacity", "recovery", "roll_forward", "mirror"]`

## 2. Inputs
- **Crate root.** Bare `f64` values: site AISC rows and bridge totals, gold koz and copper Mlb with a `PriceDeck`, reserve movements in Moz, and a cost, net price, starting grade and recovery closure for `solve_cutoff_grade`.
- **`definitions`.** A `MetricRegistry`, a definition id, `CostLine { item, usd: ExactQty }` rows, ounces sold, and a gold share for a co-product definition. `is_non_sustaining` takes the registry's `ProjectTest` and a `Project`. `bundled_registry` parses `registries/metrics.toml`, which `include_str!` embeds at compile time.
- **`cutoff`.** The text or bytes of `registries/published_cutoffs.toml`: schema `fmine.published_cutoffs.v1`, a top-level `issuer`, and `[[cutoff]]` rows of printed decimal tokens. The caller reads the file.
- **`lmdi`.** Two `LmdiState` periods of bare `f64`, or two `WaterfallPeriod`s. A `WaterfallPeriod` holds `PrintedQty { value, half_step, at }` cells (an exact value, half its last printed digit, and a source locator), `Factor::Value` or `Factor::NotPublished`, and a `BasisTag`.
- **`guidance`.** `CompletionInputs`: six `Claim<Quantity>`, for the production and AISC guidance, the YTD production, sales and AISC, and a separately declared remaining-sales scenario. `compare_history` takes `HistoricalContinuation`s; `quarters::compare_quarters` takes `QuarterlyHistory` windows.
- **`capacity`.** A signed remaining-production `Claim<Quantity>`, and `CapacityLimits`: three `Claim<PhysicalLimit>` for dry feed, head grade and recovery.
- **`recovery`.** `TrainingRow`s and an `OperatingRow` of `Claim<recovery::Quantity>`, in percent and kt units.
- **`roll_forward`.** Explicit PDF identity declarations, agreed point readings and their registered document metadata. The caller authenticates the actual registered bytes and the reader agreement before public use.
- **`mirror`.** `Claim<MirrorReading>`s under `READING_METRIC`: gold produced or sold, in koz, over one declared asset, operation perimeter and calendar period, each box on a fixed full or attributable basis. `mirror::transcriptions::bind` builds them from already-agreed S0 `Accepted` rows and a `DocumentRow`.
- **Every `Quantity`** carries exact rational bounds, a subject, a calendar-month `Period`, an `Ownership` and a kind. Units are fixed by the kind, never inferred from magnitudes.

## 3. Outputs
- **`AiscConsolidationBridge::verify_rollup`** gives `Ok(())` or a `Refusal`.
- **`compute_geo`** gives `GeoBreakdown`. Copper GEO koz is copper Mlb × 1,000 × the copper price ÷ the gold price.
- **`ReserveReplacementBridge::compute`** gives organic and headline replacement as percentages of depletion, or `None`.
- **`solve_cutoff_grade`** gives `CutoffSolution { grade, iterations, residual }` or a `CutoffError`.
- **`all_in_cost_per_oz`** gives `AllInCost { definition, superseded_by, usd_per_oz, counted, left_out }`, or a `MetricFailure`.
- **`cutoff::reproduce`** gives a `CutoffReport`: the exact grade at the printed inputs, its low and high over their rounding, a `CutoffState`, the solver's iterations for a grade-dependent recovery, and a metric-rounding flag. `CutoffReport::to_json` prints six places, rounded half up from the exact value.
- **`cutoff::section`** gives `CutoffSection { issuer, json }`. The JSON holds the registry path, its SHA-256, the count in each state and every row in registry order. `fmine-cli` carries it as the published cut-offs of an `assure public` issuer run, and in `reliance_binding`.
- **`decompose_aisc_lmdi`** gives `LmdiWaterfall`: recomputed and printed endpoints, each endpoint gap, the gap change, six effects and the measured closure residual.
- **`decompose_aisc_waterfall`** gives `Bridge::Lmdi { bars, gap, closure_residual }` or `Bridge::AdditiveFallback { reason, bars, gap }`. Each `Bar` names its rule and source locators, and has a midpoint, an outward enclosure and a `sign_not_determined` flag. The `RateReconciliation` is exact. `fmine-public`'s `waterfall.rs` is the caller.
- **`guidance::complete`** gives a `CompletionReport` of derived `Claim<Quantity>`: remaining production, YTD spend, remaining spend and rate, a frontier at both guidance limits, and the sign of the spending requirement. `fmine-public`'s `guidance.rs` calls it.
- **`compare_history` and `quarters::compare_quarters`** give an `EnvelopeTier` (`T1` or `T3`), a `HistoryVerdict` (`WithinEnvelope`, `ExceedsEnvelope` or `Indeterminate`) and one check per history. A positive check carries a witness Claim. A quarterly exclusion carries checked `exact_feas` certificates. `fmine-public`'s `guidance/history.rs` calls the quarterly path.
- **`capacity::compare_production`** gives a `CapacityReport`: two derived Claims (the ceiling and the grade needed at maximum feed and recovery) and a `CapacityVerdict`. `fmine-cli`'s `guidance_capacity.rs` calls it.
- **`recovery::fit` and `recovery::project`** give `Claim<Fit>` and a `Projection` of two model Claims, `ConstantTails` and `GradeDependentTails`. `fmine-public`'s `recovery.rs` calls them.
- **`roll_forward::assess`** gives a `TableReport` (schema `fmine.pdf-table-check.v1`). Conflicts carry the kernel certificate digest and minimum or inclusion-minimal correction sets with exact implied intervals. Feasible components carry the narrowed ranges satisfying all declared identities. The JSON report and its SHA-256 digest bind all declarations, admitted source readings, rejected candidate readings, reader roles, filing metadata and results. `fmine-cli`'s `tables.rs` calls it.
- **`mirror::compare`** gives a `MirrorReport` with a `MirrorVerdict`. `Consistent` carries a checked source-basis witness and a common full-basis Claim. `Conflict` carries the checked certificate/digest, blame status and inclusion-minimal source sets. `Indeterminate` has no common quantity. `fmine-cli`'s `mirror.rs` calls it.

## 4. Invariants
- **Refuse, never clamp.**
  - `solve_cutoff_grade`: `test_cutoff_refuses_out_of_domain_inputs` and `test_cutoff_refuses_non_convergence`.
  - Cut-off reproduction: `test_no_positive_net_is_refused_not_clamped`.
  - Signed results stay signed: `signed_derived_requirements_are_never_clamped`, `signed_requirements_and_closed_ceiling_boundaries_are_preserved`, `negative_gaps_and_extrapolated_recovery_are_not_clamped` and `test_signed_nonpositive_cost_is_not_forced_into_a_logarithm`.
- **Exact arithmetic that overflows refuses with 207, and no partial result is returned.** `exact_overflow_is_a_numerical_refusal`, `exact_arithmetic_overflow_is_a_typed_refusal`, `zero_denominators_invalid_domains_and_overflow_refuse` and `test_invalid_precision_source_and_overflow_refuse`.
- **A printed rate never moves a waterfall bar.** The factors close on supplied spend ÷ ounces sold, and the printed-rate gap is a separate item. `test_supplied_cost_drives_factors_and_printed_rate_only_drives_gaps`, `test_printed_gaps_never_change_bars_and_equal_gaps_survive` and `test_equal_endpoint_gaps_remain_visible_when_gap_change_is_zero`.
- **The legacy LMDI closes or refuses.** A non-finite effect, an unrepresentable rate, or a closure residual above 1e-9 USD/oz refuses with 207. `test_synthetic_two_period_exact_lmdi`, `test_unrepresentable_rates_refuse_instead_of_returning_nan`, `test_invalid_inputs_name_the_period_and_field` and `test_log_space_avoids_product_overflow_and_subnormal_ratio_rounding`.
- **The typed waterfall.**
  - The additive fallback closes exactly, and no rounding difference becomes a plug line: `test_property_fallback_exact_random_lines_with_negatives` (10,000 seeded line sets) and `test_fallback_never_invents_missing_lines_or_duplicate_identities`.
  - A missing or non-positive physical factor selects the fallback: `test_recovery_na_exact_signed_fallback_known_answer` and `test_straddling_bar_flagged_and_nonpositive_physical_box_falls_back`.
  - Enclosures hold every input corner: `test_lmdi_rounding_enclosures_contain_all_4096_input_corners` and `test_additive_rounding_enclosures_contain_all_64_input_corners`.
  - The bars do not depend on evaluation order: `test_property_lmdi_order_independent_and_enclosure_contains_point`.
  - A grade change moves between grade and metal balance without naming a cause: `test_grade_change_reallocates_into_balance_without_identifying_a_cause` and `test_north_mara_grade_control_is_non_identifying`.
- **The logarithmic mean** takes its logarithms from `fmine_core::det`. It lies in [min, max], gives the same bits for `L(a, b)` and `L(b, a)`, and is continuous at the series switch. `test_logarithmic_mean_between_min_and_max`, `test_logarithmic_mean_symmetric`, `test_logarithmic_mean_continuous_at_series_switch`, `test_logarithmic_mean_scale_invariance_for_widely_separated_rates` and `test_logarithmic_mean_high_precision_tiny_and_large_known_answers`.
- **A cut-off's state is exact.** The grade is monotone in every input, so the low and high corners bound it exactly. A printed figure outside its inputs' rounding is a `Conflict` result, not a failure.
  - `test_reaches_respects_the_half_open_rounding_interval` and `test_range_holds_the_printed_inputs_value`.
  - Planted errors flip the state: `test_planted_published_figure_conflicts_both_ways`, `test_planted_tonne_for_short_ton_conflicts_on_every_cortez_row` and `test_planted_ab495_moves_a_cortez_row`.
  - A section never shows fewer rows than are registered, and one bad row fails it: `test_section_counts_every_row_by_state` and `test_registry_refusals`.
- **A cost definition counts only its registered lines.** An unregistered line refuses, and excluded lines are named in `left_out`. `test_aisc_counts_its_lines_and_names_what_it_left_out`, `test_non_sustaining_capital_is_in_aic_and_refused_in_aisc`, `test_co_product_needs_a_share_and_drops_credits` and `test_a_definition_change_moves_the_result_and_is_named`.
- **Guidance never substitutes production for sales, and never averages quarterly rates.** `sold_and_produced_are_separate_and_rates_are_not_averaged`, `correlated_sales_and_numerator_do_not_create_false_uncertainty` and `rounding_extrema_enclose_every_small_interior_combination`.
- **A history check needs one joint witness.** Separate production and cost successes do not count, and every witness is re-checked against its sources.
  - `separate_marginal_successes_are_not_a_joint_history_witness`, `fractional_history_witness_hits_a_boundary_exactly` and `historical_verdict_matches_exhaustive_discrete_continuations`.
  - Quarterly: `quarter_witness_reaches_both_targets_and_checks_every_original_source`, `aggregate_sales_rate_boxes_cannot_manufacture_a_quarterly_witness`, `separate_production_and_cost_successes_do_not_make_one_joint_history` and `small_point_histories_match_independent_integer_inequalities`.
  - No history, or a solver cap, gives `Indeterminate`: `missing_history_and_solver_caps_remain_indeterminate`.
- **The capacity ceiling scales production by the attributable share, never grade or recovery.** `ownership_scales_production_not_the_physical_limits` and `rounding_uses_upper_physical_bounds_and_never_averages_rates`.
- **The two recovery variances keep their denominators apart** (`SSE/(n−1)` and `SSE/(n−2)`): `residual_sample_sd_and_regression_error_use_distinct_denominators`. Wider input boxes never narrow an output: `widening_rounding_never_narrows_model_outputs`.
- **Derived Claims keep their provenance and never gain strength.** `derived_claims_preserve_provenance_and_never_upgrade_evidence`, `derived_claims_keep_uncertainty_confidentiality_and_source_identity`, `ordering_is_deterministic_and_evidence_cannot_be_strengthened` and `fit_and_both_models_preserve_every_registered_venture_and_wall`.
- **A roll-forward identity binds only agreeing members (BD-340.S5, BD-331.S7).** Units, basis, member periods, raw-token precision, distinct reader identities, page bounds and source hashes must agree. Shared cells within a document are solved jointly by `exact_feas`; each feasible witness and infeasible certificate is checked again before emission. Different documents remain separate vintages.
- **The mirror works on one common 100% quantity (BD-346.S3).** Scale identities express it; the existing exact feasibility solver and independent checkers produce a witness or exclusion certificate. Fixed tolerances and inferred partner shares are absent. At least two reporters are required, but their shares need not sum to one. The nonnegative physical domain is explicit: a printed zero keeps its original negative-to-positive rounding box, while the common interval is intersected with `x >= 0` and the reported witness is checked there. Canonical tags bind reporter identity, numeric source anchor, operation membership, inclusive validity dates and the exact typed quantity. Inputs, normalized Claims and proof cells share one sorted order, and minimal-source indices address that order. Conflict sets' disjoint normalized intervals independently establish minimality; an incomplete blame search remains explicit.

## 5. Refusals
`lmdi`, `lmdi::waterfall`, `guidance`, `guidance::quarters`, `capacity` and `recovery` refuse with these codes:
- **`FMINE-REF-102`** (support mismatch; exit 3): a non-aggregate or invalid month support, or periods that do not line up. From `guidance`, `quarters`, `capacity` and `recovery`.
- **`FMINE-REF-103`** (non-additive average; exit 3): additivity that does not match the quantity. From `guidance`, `capacity` and `recovery`.
- **`FMINE-REF-104`** (non-identifiable attribution; exit 3). From the waterfall: a sold-ounce box that touches zero, unmatched or ambiguous line identities, or lines that do not sum to the cost. From `recovery`: a denominator box that touches zero, fewer than three annual observations, or a duplicate year. The zero-denominator and too-few-rows cases use the nearest code.
- **`FMINE-REF-105`** (basis mismatch; exit 3): a kind, unit, subject, ownership or AISC-definition mismatch, or repeated history ids. From every Claim kernel, and from the waterfall for unequal bases. `guidance` also uses it for an attributable share outside (0, 1].
- **`FMINE-REF-107`** (undeclared unit, basis or time zone; exit 3): an undeclared subject or definition, or an unprinted waterfall basis. From `guidance`, `capacity`, `recovery` and the waterfall.
- **`FMINE-REF-108`** (assay lookahead; exit 3), used for time order: a history at or after the guidance year, or a quarter that does not follow every training year. From `guidance`, `quarters` and `recovery`.
- **`FMINE-REF-109`** (outside physical domain; proposed, exit 3). From the legacy LMDI: a non-positive or non-finite input, or recovery above one. From the waterfall: a negative half-step, or a share outside (0, 1]. From the Claim kernels: reversed bounds, or a value outside its domain.
- **`FMINE-REF-201`** (no claim in domain; exit 3): a missing source locator or digest, or a history with no id or no quarters. From the waterfall, `guidance`, `quarters`, `capacity` and `recovery`.
- **`FMINE-REF-202`** (not observable; exit 3): an input whose state is neither `Known` nor `Estimated`. From `guidance`, `capacity` and `recovery`.
- **`FMINE-REF-203`** (outside AOA; exit 3).
  - From `guidance`, `capacity` and `recovery`, for an open interval bound. The kernels need closed rounding bounds. This is the nearest code.
  - From `AiscConsolidationBridge::verify_rollup`, when the numerator gap exceeds 0.5 $M (Validity domain).
- **`FMINE-REF-205`** (calibration required; exit 3): an input that failed calibration. From `guidance`, `capacity` and `recovery`.
- **`FMINE-REF-207`** (numerical certificate failed; proposed, exit 3): exact overflow, a failed witness or certificate, an invalid enclosure, or a failed closure. From every Claim kernel, both LMDI paths and `quarters`.
- **`FMINE-REF-208`** (source cell unusable; proposed, exit 3): a placeholder input. From `guidance`, `capacity` and `recovery`.
- **`FMINE-REF-502`** (stale anchor; exit 6): a stale input. From `guidance`, `capacity` and `recovery`.

`mirror` refuses unsupported cost definitions/kinds, unequal assets and invalid shares (`105`), unequal periods or invalid dates (`102`), missing/unequal operation perimeters or validity straddles (`104`), missing numeric/context citations (`201`), private sources (`301`), placeholders (`208`), stale/unknown/conflicted/failed-calibration inputs (`502`/`202`/`205`), and mutated canonical Claims (`507`). Reversed bounds or negative reported midpoints refuse (`109`); open source bounds refuse (`203`). Arithmetic overflow/checker failures refuse (`207`); solver limits remain `Indeterminate`. `mirror::transcriptions::bind` refuses mismatched numeric or ownership document names, hashes and pages before construction.

`roll_forward` does not refuse a table for one bad identity. A missing, ambiguous or incompatible member keeps an `IdentityRefusal` with its available evidence, and independent complete identities still run.

The other modules have local errors with no refusal code:
- **`CutoffError`**: `NonPositiveCost`, `NonPositivePrice`, `NonPositiveGuess`, `RecoveryOutOfDomain` and `NotConverged`.
- **`MetricFailure`**: `UnknownDefinition`, `NotACostDefinition`, `UnregisteredItem`, `NonPositiveOunces`, `AllocationRequired`, `AllocationNotApplicable` and `Exact`.
- **`CutoffFailure`**: `Registry`, `Row`, `Exact`, `NoPositiveNet`, `NoPositiveRecovery` and `Solver`. `fmine-cli` maps it to `FMINE-REF-506`.
- **`ReserveReplacementBridge::compute`** returns `None` for a depletion that is not positive and finite.

## 6. Determinism class
**DeterministicPerIsa** for the crate, the weakest of its files' `//! Determinism:` headers:
- **Exact:** `capacity.rs`, `guidance.rs`, `guidance/quarters.rs`, `mirror.rs`, `mirror/transcriptions.rs` and `roll_forward.rs`. These use checked rational arithmetic, canonical input tags and sorted identities.
- **BitwisePortable:** `lmdi.rs`. Its two golden digests, `golden_digest_logarithmic_mean_10k` and `golden_digest_decompose_aisc_lmdi_10k`, agreed on x86_64-linux and aarch64-macos in CI run 41237993223 at 84f32ac (bd-hp2j.3). `test_golden_digest_logarithmic_mean_10k` and `test_golden_digest_decompose_aisc_lmdi_10k` pin them.
- **DeterministicPerIsa:**
  - `lib.rs` has no cross-host digest test (plan §29.2, bd-hp2j.1);
  - `cutoff.rs` grades are exact, but its solver cross-check uses floats;
  - `definitions.rs` is exact, but `PriceDeck` hands floats to `compute_geo`;
  - `lmdi/waterfall.rs` has exact rates and additive bars, with portable-log LMDI enclosures;
  - `recovery.rs` has no cross-host test of its whole output, which includes square-root summaries.
- **The test files** `lmdi/waterfall/tests.rs` and `recovery/tests.rs` declare DeterministicPerIsa.

## 7. Evidence class of outputs
- **`guidance` and `capacity`** give `Derived`, met with every parent's class. So a result is never stronger than its weakest input. An issuer `Assertion` stays an `Assertion`. Epistemic status and state take the weaker of their parents'. Confidentiality is joined, and anchors, parent ids and UU terms are kept. The exact set lives in the value, so the representation is `Deterministic`.
- **`recovery`** starts from `ModelledUncalibrated`, `KnownUnknown(Parametric)` and `Estimated`, then meets its sources'. A model output is never calibrated here (NL-4). It adds one model UU term, named by `RULES_VERSION` and `MODEL_CAVEAT`.
- **`mirror`**'s normalized and common Claims retain immediate source parents, anchors, weaker evidence/knowledge/epistemics, and every distinct UU term, including different terms sharing a scenario id. No assertion is promoted. `mirror::transcriptions::bind` preserves the supplied evidence class.
- **The other outputs carry no Claim and no evidence class.** These are the root items, `definitions`, `cutoff`, both LMDI paths and `roll_forward`. `WATERFALL_NO_CLAIM`, `WATERFALL_RULES_VERSION`, `recovery::NO_CLAIM`, `roll_forward::NO_CLAIM` and `mirror::NO_CLAIM` state the descriptive reading. A `Conflict` or `Reproduced` cut-off state speaks to the printed inputs only, not to whether they suit the mine.

## 8. Validity domain
- **The root items are bare `f64`.** Four L-6 rows in `registries/l6_allow.toml` cover them. Three are debt: `compute_geo`, `ReserveReplacementBridge::compute` and `solve_cutoff_grade`. One is generic: `logarithmic_mean`.
- **`verify_rollup` checks the numerator only, to a fixed 0.5 $M.**
  - It compares the caller's site sum, plus L_corp if asked, with the caller's consolidated numerator. Either mode refuses on an absolute gap above 0.5 $M. The tolerance is fixed, not taken from a declared rounding construction.
  - It does not re-add the site rows, and does not check the L_JV sales, the consolidated sales, either AISC or the rounding bridge.
- **`compute_geo` checks nothing.** A zero or non-finite price gives an infinity or a NaN.
- **`ReserveReplacementBridge::compute` checks only depletion.** Non-finite additions pass into the percentages. The basis is whatever the caller supplies.
- **`solve_cutoff_grade`** stops when the undamped step falls below 1e-10 in grade units, within 100 iterations. It reports the residual and does not bound it. `cutoff` holds it to the exact fixed point within 1e-6 g/t.
- **`definitions` takes the amounts as given.** The registry says what counts, not that an amount is right.
- **The legacy LMDI** is float. Its spend and sold ounces must share a scale. It encodes no units or basis, and its closure tolerance is 1e-9 USD/oz absolute.
- **The typed waterfall** fixes its units: $M, koz, thousand tonnes, g/t and a fraction. It checks that both periods carry the same basis tag, but cannot authenticate that basis. Its enclosures treat inputs as independent, so they over-enclose shared ones.
- **Guidance** works on closed intervals, with sales and AISC strictly positive and production at least zero. Its results are necessary arithmetic conditions, not forecasts or probabilities. `T1` is the supplied history only. `ExceedsEnvelope` says nothing about physical capacity.
- **Capacity** multiplies the three maxima. That gives an optimistic ceiling, not a plan. `NotExcluded` is not a feasibility witness.
- **Recovery** fits two descriptive annual models and applies them to quarters. Its enclosures cover input rounding, not sampling error. An extrapolated recovery outside 0–100% is kept as computed.
- **The PDF identity adapter (`roll_forward`, BD-340.S5)** verifies declared arithmetic of issuer assertions, not their truth or completeness. It never emits reserve classifications, selects a preferred filing/deck vintage, silently changes a printed sign, or supplies an issuer-attributed corrected figure. Synthetic E692-shaped arithmetic is a known-answer test of the binding; the authentic historical transcription and independent census gates remain open. Closed intervals are agnostic about ties; a named half-open convention is never changed to rescue a row. No file I/O occurs here: callers authenticate actual registered bytes and reader agreement before public use.
- **The mirror** authenticates no filing bytes, human readership, publication date or issuer affiliation; its caller must bind those facts before supplying source Claims. It does not infer missing perimeters, aggregate incomparable operations, select vintages, invent an absent partner share, bridge costs/capex, transfer bridges between assets, or decide which reporter is physically correct. Its transcription binding consumes already-agreed S0 rows, without authenticating bytes or publication availability; accepted rows alone cannot prove that an omitted rejection did not exist.

## 9. Oracles
- **Exact hand known answers**, all synthetic:
  - `synthetic_completion_known_answers_are_exact`;
  - `cod_01_ceiling_and_required_grade_are_exact`;
  - `independent_exact_fit_and_two_model_known_answers`;
  - `test_recovery_na_exact_signed_fallback_known_answer`;
  - `test_fixed_point_cutoff_grade_solver`.
- **Exhaustive and independent checks.**
  - Integer enumerations: `historical_verdict_matches_exhaustive_discrete_continuations` and `small_point_histories_match_independent_integer_inequalities`.
  - Corner sweeps: `rounding_enclosures_contain_every_synthetic_training_and_operating_corner` and the two waterfall corner tests.
- **High-precision known answers** for the logarithmic mean: `test_logarithmic_mean_high_precision_tiny_and_large_known_answers` and `test_logarithmic_mean_enclosure_known_answers_near_equality_and_tiny_scale`.
- **Seeded populations** from Philox streams: `test_cutoff_property_seeded`, `test_property_fallback_exact_random_lines_with_negatives`, `test_property_lmdi_order_independent_and_enclosure_contains_point` and the two golden digests.
- **The mirror (BD-346, `mirror::tests`).** Ten focused tests cover the exact three-source tie at 100 koz; the 1000/615/385 case; independent witness/certificate checks and corrupted proofs; minimal conflict pairs; zero's explicit domain; two 45% reporters and sold quantities; reporter-order/full-basis metamorphisms; dated scope and evidence/UU preservation; same document basenames across issuers; semantic, source, canonical-mutation and numerical-cap refusals. A separate integer-rounding oracle keeps the known truth inside the common interval for 256 synthetic worlds. This is not the full BD-346.S6 power study. Direct transcription-binding tests additionally check exact source/share Claims.
- **The registries.**
  - `test_seven_corpus_cutoffs_e182_e184` runs every row of `registries/published_cutoffs.toml` and pins each row's state (E182–E184).
  - The `definitions` tests read `registries/metrics.toml`, including `test_price_decks_read_from_the_registry`.
- **`repro/barrick/waterfall_census.py` and `repro/barrick/recovery_census.py`** are independent readers for hand-entered public cells. No workflow runs them.
- **No fixture** under `fixtures/`.

## 10. Known gaps
- **The old contract had the wrong registry lines.** It said Subsystem Enterprise and 16k/18k. The registry says Bullion and 18,000/21,000.
- **The old dependency list was wrong.** It omitted `fmine-registry`, which `cutoff` and `definitions` use. It listed `fmine-time`, which `Cargo.toml` declares but no source file uses.
- **The old contract named PO-MET-1 to PO-MET-8**, with PO-MET-6 used twice. The plan has none of them. The plan's rows are in Proof obligations.
- **The old contract said `verify_rollup(false)` refuses when L_corp ≠ 0. That is false.** It refuses on the gap between the site sum and the consolidated numerator, whatever L_corp holds. An L_corp of 0.5 $M or less never refuses. The doc comment on `verify_rollup` says the same false thing.
- **The old contract labelled 203 `ConsolidationBridgeMissing`.** No such key exists. The catalogue's 203 is `outside_aoa`, used here as the nearest code.
- **The old contract said the published cut-offs were not yet reproduced.** `cutoff` reproduces them, and its own API list said so.
- **The old refusal lists were incomplete.** `guidance` also refuses 208, and `capacity` and `recovery` also refuse 107.
- **G-BULL-3 is only partly enforced.** `verify_rollup` checks one numerator layer, with the limits in Validity domain, and nothing calls it.
- **Made-up values.**
  - The rollup's 0.5 $M absolute tolerance.
  - `recovery`'s UU term sets `downside_exposure_usd_m` to 0.0 to mean "unquantified". The field is a bare `f64` in `fmine-claim`, so a consumer that sums downsides reads it as zero. Only the trigger text says otherwise.
  - `logarithmic_mean` returns NaN for a non-positive or non-finite input, rather than an error.
- **Silent `None`s.** `PriceDeck::barrick_reserve` turns a registry load failure into `None`, the same answer as an unregistered year. `ReserveReplacementBridge::compute` returns `None`, not a typed refusal.
- **No unit types at the root.** Three L-6 debt rows (Validity domain).
- **The module docs in `lib.rs` carry shifted BD ids.** In the plan:
  - L_corp and L_JV aggregation is BD-273, not BD-060–BD-062 or BD-272 (BD-272 is the GATE-BULL-01 scoring rule);
  - GEO is BD-062, not BD-063 or BD-273;
  - reserve replacement is BD-063, not BD-064 (BD-064 is the LMDI waterfall);
  - cut-off reproduction is BD-274, not BD-275 (BD-275 is the discrepancy fixtures).
- **Most of plan §21.6 is absent.** There is no C1 and no ROCE, though the registry charter lists both. There is no single `aisc(rules_version, inputs)` function for plan and actual. There are no definition flags (by-product credit location, inventory adjustments, per-ounce denominator) and no `CostDriverCard`.
- **Code with no caller outside this crate's tests:**
  - `AiscConsolidationBridge`, `PriceDeck`, `compute_geo` and `ReserveReplacementBridge`;
  - the whole `definitions` module;
  - the legacy `LmdiState` and `decompose_aisc_lmdi`;
  - the single-period `compare_history` and `check_witness`.
  - `solve_cutoff_grade` runs only inside `cutoff`'s cross-check.
- **Codes are not used the same way.** An attributable share outside (0, 1] gives 105 in `guidance` but 109 in the waterfall.
- **No cross-host digest** for `lib.rs`, `cutoff`, the typed waterfall or `recovery`.
- **The mirror's real-data work is open.** Actual FY2023–FY2025 partner-source reproduction and the full synthetic power gate (BD-346.S6) remain separate BD-346 work.

## 11. Public API
- **Root:** `SiteAiscRow`, `AiscConsolidationBridge::verify_rollup`, `PriceDeck` with `barrick_reserve(year)`, `GeoBreakdown`, `compute_geo`, `ReserveReplacementBridge::compute`, `CUTOFF_MAX_ITER`, `CUTOFF_TOL`, `CutoffSolution`, `CutoffError` and `solve_cutoff_grade`. The root re-exports `LmdiState`, `LmdiWaterfall`, `decompose_aisc_lmdi` and `logarithmic_mean`.
- **`definitions`:** `bundled_registry`, `CostLine`, `MetricFailure`, `AllInCost`, `all_in_cost_per_oz`, `Project` and `is_non_sustaining`.
- **`cutoff`:** `CUTOFFS_SCHEMA`, `CUTOFFS_REGISTRY`, `CutoffFailure`, `Printed`, `GradeUnit`, `Recovery`, `CutoffCase`, `CutoffState`, `CutoffReport` with `to_json`, `reproduce`, `parse_cases`, `CutoffSection` and `section`.
- **`lmdi`:**
  - `LmdiState`, `LmdiWaterfall` with `sum_of_factors`, `logarithmic_mean` and `decompose_aisc_lmdi`;
  - `LMDI_DIGEST_KEY`, `logarithmic_mean_digest_inputs`, `digest_logarithmic_mean`, `golden_digest_logarithmic_mean_10k`, `digest_decompose_aisc_lmdi` and `golden_digest_decompose_aisc_lmdi_10k`;
  - from `waterfall`: `PrintedQty`, `Factor`, `BasisTag`, `WaterfallPeriod`, `BarPoint`, `Bar`, `RateReconciliation`, `Bridge` with `bars` and `gap`, `decompose_aisc_waterfall`, `WATERFALL_RULES_VERSION` and `WATERFALL_NO_CLAIM`.
- **`guidance`:**
  - `Ownership`, `Period`, `QuantityKind`, `Quantity` with `canonical_tag`, `CompletionInputs`, `FrontierPoint`, `SpendingRequirement`, `CompletionReport` and `complete`;
  - `HistoricalContinuation`, `CompletionWitness`, `ContinuationCheck`, `EnvelopeTier`, `HistoryVerdict`, `HistoryReport`, `check_witness` and `compare_history`;
  - `quarters`: `QuarterlyHistory`, `WindowTotals`, `QuarterWitness`, `QuarterlyWitness`, `CheckedCertificate`, `QuarterlyCheck`, `QuarterlyHistoryReport`, `aggregate_quarters`, `check_quarter_witness` and `compare_quarters`.
- **`capacity`:** `PhysicalKind`, `PhysicalLimit` with `canonical_tag`, `CapacityLimits`, `CapacityVerdict`, `CapacityReport` and `compare_production`.
- **`recovery`:** `RULES_VERSION`, `NO_CLAIM`, `MODEL_CAVEAT`, `QuantityKind`, `Quantity`, `TrainingRow`, `OperatingRow`, `Estimate`, `SquareRoot`, `Fit`, `Model`, `ModelProjection`, `Projection`, `fit` and `project`. It re-exports `Ownership` and `Period` from `guidance`.
- **`roll_forward`:** `SCHEMA`, `NO_CLAIM`, `assess`, `TableReport`, `SourceCell`, `ComponentCheck` and `IdentityRefusal` (BD-340.S5, BD-331.S7).
- **`mirror`:** `READING_METRIC`, `NO_CLAIM`, `Perimeter`, `MirrorReading`, `MirrorCommon`, `MirrorReport`, `MirrorVerdict` and `compare` (BD-346). `mirror::transcriptions`: `BoundReadings`, `bind`, `keys` and `period_label`. `bind(subject, period, kind, issuer, document, rows, evidence)` returns the separately cited ownership Claim, when needed, and source reading Claims.

## 12. Proof obligations
The plan defines no `PO-MET-*`. The old contract's PO-MET-1 to PO-MET-8 were local labels, and their properties are tested here (Invariants). The plan's §21.8 rows that touch this crate:
- **PO-PUSH-9** (plan and actual AISC use one `aisc()` and `rules_version`): not discharged. No such function exists.
- **PO-PUSH-10** (the plan-to-actual bridge sums exactly, and LMDI effects are order-independent): the LMDI half has property tests, `test_property_lmdi_order_independent_and_enclosure_contains_point` and `test_property_fallback_exact_random_lines_with_negatives`. There is no plan-to-actual bridge.
- **PO-BULL-1 to PO-BULL-4** belong to the FrankenAccounting bridge and GATE-BULL-02. None is discharged here.

Guards and gates:
- **G-BULL-3** is partly met by `verify_rollup` (Known gaps).
- **G-BULL-6** (no metric without `rules_version` and basis) is not enforced here. Rendering happens in `fmine-public`.
- **GATE-BULL-01** has no scorer in this crate. The crate supplies kernels only.
- **GATE-BULL-02 and GATE-BULL-03** need the FrankenAccounting book and a site pilot. Neither exists here.
