# CONTRACT.md — `fmine-metrics`

## 1. Purpose
The public-lane metric kernels. Each one takes supplied numbers and computes a mining metric or a check on one:
- AISC and AIC per ounce under a definition read from `registries/metrics.toml` (`definitions`), and as an exact cost receipt over declared Claims or a strict cost worksheet (`definitions::claims`, `definitions::worksheet`, BD-061);
- exact production GEO under two to sixty-four named price decks, from declared Claims or a strict worksheet (`definitions::geo`, BD-062, GB-27);
- complete source-bound annual peer comparisons with exact competition-rank ranges and explicit comparative claims (`comparison`, BD-355);
- cut-off grades: the reference rows of `registries/published_cutoffs.toml` recomputed from their registered tokens (`cutoff`), and cut-offs bound to the readings of one registered technical report (`cutoff::source`, BD-274);
- an AISC accounting waterfall, as a legacy float LMDI and as a typed exact kernel with an additive fallback (`lmdi`, `lmdi::waterfall`);
- the production and AISC a year still needs to meet guidance, and whether the site's own history reaches it (`guidance`, `guidance::quarters`);
- an optimistic production ceiling from stated feed, grade and recovery (`capacity`);
- a grade-normalised recovery comparison (`recovery`);
- the declared arithmetic identities of a PDF table, including gold/copper sold-unit cost rates and roll-forwards, over agreed readings, and reserve-replacement ratios bound to a cited definition (`roll_forward`, `roll_forward::assess_with_metrics`);
- reserve replacement over a complete declared annual window, on attributable gold or GEO (`reserve_replacement`, BD-063);
- a quarter derived from cumulative disclosures (Q2 = H1 − Q1, Q3 = 9M − H1, Q4 = FY − 9M) and checked against a draft's displayed figure (`roll_forward::quarter`, BD-345.S4);
- H1, nine months and FY as sums of complete quarters, and full-mine amounts scaled by explicit ownership, carried as Claims (`public_derivation`, BD-345.S4);
- gold and copper cost-per-sold-unit enclosures from independently printed cost and sales amounts, with source-parent Claims retained (`public_derivation`, BD-345.S4);
- a source-bound projection of an issuer's declared reserve rows, with their filing vintages and quoted price decks, and an exact check of gold, copper and GEO reserve rows against their quoted prices (`reserves`, `reserves::geo`, BD-335, BD-070);
- whether the reporters of one asset agree on the gold or copper it produced or sold, or on its sustaining, project and total capital, on one 100% basis (`mirror`, `mirror::capex`);
- at the crate root: an AISC consolidation check, GEO under a price deck, reserve replacement and a float cut-off solver.

The results are descriptive arithmetic. None identifies an operational cause, forecasts, or proves physical feasibility. The crate does no I/O at run time.

- **Crate:** `fmine-metrics`
- **Layer:** E
- **Subsystem:** Bullion
- **LOC budget:** 18,000 planned / 21,000 ceiling
- **Allowed Module Families:** `["lmdi", "cutoff", "definitions", "guidance", "capacity", "recovery", "reserve_replacement", "reserves", "roll_forward", "mirror", "public_derivation", "consolidation", "comparison"]`

## 2. Inputs

- DESIGN (BD-355): `comparison::compare` takes a complete declared set of 2–64 issuer Claims on
  identical annual gold AISC/production or copper C1/AISC/production definitions. Each input retains its agreed metric,
  company scope, ownership, operations, metal, denominator, own-document definition citation,
  source SHA-256, publication evidence and readers. Costs also require allocation, credit location,
  inventory, G&A, sustaining leases, minesite exploration, reclamation, stripping and accounting
  treatment. Copper C1 and AISC additionally require explicit matching royalty treatment. Copper
  production is kt on a produced basis; copper costs are USD/lb on a sold basis. Signed cost readings
  remain admissible, while production has a nonnegative domain. Existing gold bindings are unchanged.
  Missing, extra or duplicate peers and differing fields refuse; no bridge or conversion
  is inferred. `comparison::source::bind` admits two-reader-agreed printed actuals from a selected
  registered financial or annual filing. The I/O owner authenticates bytes and complete reader agreement.
- DESIGN (BD-346.S5): `mirror::cost::compare` accepts canonical `Claim<CostReading>`s for cash costs,
  AISC or cost of sales in USD millions. Each original and signed adjustment retains its raw printed
  token, agreeing reader IDs, exact half-step bounds, own-document definition locator, publication
  evidence, fixed ownership and complete dated asset perimeter. All eight definition dimensions are
  explicit: primary metal, sold/produced denominator, by-/co-product allocation, credit location,
  inventory, G&A, reclamation and accounting treatment. `mirror::cost::bind::bind` consumes already
  agreed source rows; authenticity, complete reader agreement and source availability remain the caller's duty.
- DESIGN (BD-273): `consolidation::compare` accepts one bound `Claim<Table>` with complete named
  component rows, one aggregate column, explicitly matched member columns and membership rules. Each
  amount retains its signed exact value and display step; numerator components are USD millions and sold
  quantities are thousand ounces. Source scope, public evidence, knowledge state, support, shape and Claim
  identity are checked. No ownership conversion or inference of company membership occurs.
- **Crate root.** Bare `f64` values: site AISC rows and bridge totals, gold koz and copper Mlb with a `PriceDeck`, reserve movements in Moz, and a cost, net price, starting grade and recovery closure for `solve_cutoff_grade`.
- **`definitions`.** A `MetricRegistry`, a definition id, `CostLine { item, usd: ExactQty }` rows, ounces sold, and a gold share for a co-product definition. `is_non_sustaining` takes the registry's `ProjectTest` and a `Project`. `bundled_registry` parses `registries/metrics.toml`, which `include_str!` embeds at compile time.
- **`definitions::claims`.** The metric registry and `Claim<CostInput>`s: exact USD `Money` lines, gold ounces sold and an optional separately stated co-product allocation, all deterministic period aggregates on one issuer, subject, calendar support, ownership and declared accounting basis. A rule Claim must declare that the lines form the complete cost population.
- **`definitions::worksheet`.** The text of a `fmine.cost-worksheet.v1` worksheet, a `SourceAnchor` for it, and the metric-registry bytes, whose SHA-256 is checked. Costs are USD fixed-point amounts with at most four fractional digits. Sold quantities and ownership or allocation shares are exact decimals or rationals. Optional `[[driver_cost]]` rows declare `item`, `driver`, `unit`, `quantity`, `usd_per_unit` and optional `fixed_usd` (explicitly zero by default), with source anchors for each operand. At most 4,096 rows of at most 4,096 bytes a field.
- **`definitions::geo`.** `Claim<GeoInput>`s for one physical population (issuer, subject, calendar period, ownership, basis and currency) and two to sixty-four named, versioned price decks, one of them the reference. Gold and silver are in oz and copper in lb. Deck prices are positive major currency units per metal unit, and every deck prices gold, the numeraire. `definitions::geo::worksheet` parses a `fmine.geo-worksheet.v1` worksheet of at most 1 MiB with its `SourceAnchor`.
- **`cutoff`.** The text or bytes of `registries/published_cutoffs.toml`: schema `fmine.published_cutoffs.v1`, a top-level `issuer`, and `[[cutoff]]` rows of decimal tokens. The caller reads the file.
- **`cutoff::source`.** Cut-off declarations from the table registry, agreed point readings from one registered `technical_report`, and the document metadata. The caller authenticates the document bytes and complete reader agreement.
- **`lmdi`.** Two `LmdiState` periods of bare `f64`, or two `WaterfallPeriod`s. A `WaterfallPeriod` holds `PrintedQty { value, half_step, at }` cells (an exact value, half its last printed digit, and a source locator), `Factor::Value` or `Factor::NotPublished`, and a `BasisTag`.
- **`guidance`.** `CompletionInputs`: six `Claim<Quantity>`, for the production and AISC guidance, the YTD production, sales and AISC, and a separately declared remaining-sales scenario. `compare_history` takes `HistoricalContinuation`s; `quarters::compare_quarters` takes `QuarterlyHistory` windows.
- **`capacity`.** A signed remaining-production `Claim<Quantity>`, and `CapacityLimits`: three `Claim<PhysicalLimit>` for dry feed, head grade and recovery.
- **`recovery`.** `TrainingRow`s and an `OperatingRow` of `Claim<recovery::Quantity>`, in percent and kt units.
  DESIGN (bd-srqc): `recovery::sensitivity` accepts a canonical quarterly `Claim<ModelProjection>`
  and a separately declared `Claim<CopperPrice>`. The named, nonnegative USD/lb price is an exact constant
  nominal assumption, with its own declaration anchor; it is never presented as a market observation.
- **`roll_forward`.** Explicit PDF identity declarations, agreed point readings and their registered document metadata. `assess_with_metrics` also takes the metric registry. The caller authenticates the actual registered bytes and the reader agreement before public use.
- **`reserve_replacement`.** Bindings for a complete declared annual window, the metric registry, agreed PDF point readings and document metadata. Each GEO year names its reserve-price deck and definition evidence.
- **`roll_forward::quarter`.** A `Quantity` (additive gold production or sales in koz, or copper production in kt), the target quarter, a draft `Printed` value and step, and `Observation`s. Source ids name the actual operands. A caller-authenticated series key binds issuer, subject, year, scope and perimeter version, metric, unit, occurrence and an explicit normalised ownership. Each observation states its cumulative ending quarter and either its displayed value and step or an unusable value.
- **`public_derivation`.** A `Rule` and `Claim<Amount>`s: exactly Q1 and Q2, or Q1 to Q4, of one extensive `AmountKind`. Rates and grades cannot enter the enum. The caller authenticates the source bytes and publication availability.
  DESIGN: `cost_rate_bounds` separately accepts `CostRateUnit::GoldKoz` or `CopperMillionPounds` and two
  `roll_forward::quarter::Printed` operands. A US$M numerator divided by sold koz uses factor 1000;
  a US$M numerator divided by sold millions of pounds uses factor 1. Each operand retains its own positive
  displayed step, signed cost intervals remain signed, and the complete sales interval must be positive.
  `derive_cost_rate` also accepts the named cost family and canonical source-parent Claims, returning a
  derived interval Claim with their evidence, uncertainty terms, source anchors and parent identities.
  Gold cost of sales is a distinct `cos_per_oz` family on the same explicit sold-koz scale; the kernel
  does not establish whether a consolidated cost numerator matches an attributable sales denominator.
  Source selection, publication-date admission, same-table scope and ownership compatibility remain the
  adapter's responsibility; neither arithmetic entry point authenticates a disclosure or infers unit conversions.
- **`reserves`.** A `TableIdentityRegistry`, `Accepted` readings and `DocumentRow`s. `reserves::geo` takes explicit gold, copper and GEO row memberships, the primary GEO row's quoted prices and declared convention, and optional candidate decks with independently bound price readings.
- **`mirror`.** `Claim<MirrorReading>`s under `READING_METRIC`: gold produced or sold in koz, copper produced or sold in kt, or sustaining, project or total capital in USD millions, over one declared asset, operation perimeter and calendar period, each box on a fixed full or attributable basis. Every capital amount carries its scope, cash or accrual timing, lease treatment and a citation to that reporter's definition page. `mirror::transcriptions::bind` builds them from already-agreed S0 `Accepted` rows and a `DocumentRow`. Copper has distinct metric-bearing canonical tags; existing gold Claim identities are unchanged. No implicit metal, production/sales or mass-unit conversion is applied.
- **Every `Quantity`** carries exact rational bounds, a subject, a calendar-month `Period`, an `Ownership` and a kind. Units are fixed by the kind, never inferred from magnitudes.

## 3. Outputs

- DESIGN (BD-355): `comparison::Report` retains all source Claims plus the derived rank Claim and
  explicit lowest/highest rank-at-most-N verdict. Exact closed half-step intervals feed competition
  ranks: ascending minimum is `1 + count(other.upper < self.lower)` and maximum is
  `1 + count(other.lower < self.upper)`, excluding self; descending reverses the inequalities.
  Ties share a rank. A claim is supported, unsupported or indeterminate over all admitted rounding
  values. Every source parent and anchor, weakest evidence and uncertainty term remains bound.
  These are arithmetic rank bounds, not probabilities or proprietary industry quartiles. Real peer
  filing reproduction remains a data gate. Seven shared exact controls include the bead's known
  answer, ties, touching endpoints, exhaustive small-box extrema and source-definition refusals.
- DESIGN (BD-346.S5): `CostReport` keeps all source Claims, one exact adjusted full-basis Claim per
  reporter, and a common interval Claim when the intervals intersect. The status is `consistent`
  without adjustments and `explained` with them. Disjoint intervals produce `conflict`, no common
  Claim, and a rational separation certificate naming the greater lower bound and lesser upper bound.
  This is an arithmetic compatibility report, not an accounting reconciliation or an implied per-ounce rate.
- DESIGN (bd-srqc): `recovery::sensitivity::assess` returns a derived `Claim<Sensitivity>` retaining the
  model, subject, full-site calendar quarter, signed copper difference in kt and exact signed USD-million
  value enclosure under the named price. The conversion uses the statutory avoirdupois gram/pound ratio
  from `fmine-units`. Both parent identities, sources, weaker evidence and uncertainty terms are retained.
  It prices the descriptive model difference without claiming recoverable copper, revenue, profit,
  monetary downside, a forecast, annualization or an implicit FX conversion.
- DESIGN (BD-273): `consolidation::compare` returns a derived comparison Claim with each observed
  aggregate-minus-member component difference, the summed differences, numerator and sold-quantity gaps,
  unexplained remainder and every column's own footing residual. Exact marginal display-rounding bounds
  remain separate from the nominal `printed_arithmetic_closed` result. Closure is not an accounting
  reconciliation or evidence identifying corporate/JV allocations. The CLI binds verified AISC workbook
  Claims to this kernel for its explicit region and subtotal selections and retains unavailable members.
  The nested `rates` result adds recomputed aggregate and pooled-member USD/oz rates under
  `aisc.component-rate-bridge/1`. Named component differences and the unexplained remainder use aggregate
  sales as their denominator. The separate denominator term is `1000 * member numerator *
  (1 / aggregate sales - 1 / member sales)`. These terms close exactly to the recomputed rate gap;
  printed per-ounce rows are never substituted. Every term retains its marginal display-rounding bounds,
  including signed costs and decimal source precision. Shared operands do not imply independent errors.
  A rate denominator touching zero refuses with 104; rate arithmetic overflow refuses with 207. Either
  nested refusal preserves the valid dollar comparison and is bound, alongside successful rate outputs,
  by the comparison's version-two `ACOU` identity. Neither arithmetic closure nor a valid denominator
  resolves unexplained amounts or identifies corporate/JV allocations.
- **`AiscConsolidationBridge::verify_rollup`** gives `Ok(())` or a `Refusal`.
- **`compute_geo`** gives `GeoBreakdown`. Copper GEO koz is copper Mlb × 1,000 × the copper price ÷ the gold price.
- **`ReserveReplacementBridge::compute`** gives organic and headline replacement as percentages of depletion, or `None`.
- **`solve_cutoff_grade`** gives `CutoffSolution { grade, iterations, residual }` or a `CutoffError`.
- **`all_in_cost_per_oz`** gives `AllInCost { definition, superseded_by, usd_per_oz, counted, left_out }`, or a `MetricFailure`.
- **`definitions::claims::evaluate`** gives an immutable `CostReceipt`: every input, each line's exact signed or allocated contribution, the excluded items, the numerator, USD per sold ounce, the effective rule and its successor, and a derived `Claim<CostSummary>`. `definitions::worksheet::evaluate` parses a worksheet into the same receipt, rendered as JSON and Markdown with a source trace. `fmine-cli`'s `cost.rs` (`fmine metrics cost`) calls both.
- **`definitions::geo::evaluate`** gives a `GeoReceipt`: exact contributions and totals under each deck, and signed differences from the reference deck on unchanged quantities, as JSON and Markdown. `definitions::geo::worksheet::evaluate` parses a worksheet into it. `fmine-cli`'s `geo.rs` (`fmine metrics geo`) calls the worksheet path.
- **`cutoff::reproduce`** gives a `CutoffReport`: the exact grade at the registered inputs, its low and high over their rounding, a `CutoffState`, the solver's iterations for a grade-dependent recovery, and a metric-rounding flag. `CutoffReport::to_json` prints six places, rounded half up from the exact value.
- **`cutoff::section`** gives `CutoffSection { issuer, json }`. The JSON holds the registry path, its SHA-256, the count in each state and every row in registry order, and says `"mode":"reference_input_arithmetic","source_verified":false,"gate_eligible":false`. `fmine-cli` carries it in an `assure public` issuer run, and in `reliance_binding`.
- **`cutoff::source::assess`** gives a `SourceReport` of `SourceRow`s: every declaration, the source each one used and any refusal, with JSON, Markdown and a digest. `SourceRow::claims()` rebuilds the input Claims and one derived result Claim. `roll_forward::assess` carries the report as `TableReport.cutoffs`.
- **`decompose_aisc_lmdi`** gives `LmdiWaterfall`: recomputed and printed endpoints, each endpoint gap, the gap change, six effects and the measured closure residual.
- **`decompose_aisc_waterfall`** gives `Bridge::Lmdi { bars, gap, closure_residual }` or `Bridge::AdditiveFallback { reason, bars, gap }`. Each `Bar` names its rule and source locators, and has a midpoint, an outward enclosure and a `sign_not_determined` flag. The `RateReconciliation` is exact. `fmine-public`'s `waterfall.rs` is the caller.
- **`guidance::complete`** gives a `CompletionReport` of derived `Claim<Quantity>`: remaining production, YTD spend, remaining spend and rate, a frontier at both guidance limits, and the sign of the spending requirement. `fmine-public`'s `guidance.rs` calls it.
- **`compare_history` and `quarters::compare_quarters`** give an `EnvelopeTier` (`T1` or `T3`), a `HistoryVerdict` (`WithinEnvelope`, `ExceedsEnvelope` or `Indeterminate`) and one check per history. A positive check carries a witness Claim. A quarterly exclusion carries checked `exact_feas` certificates. `fmine-public`'s `guidance/history.rs` calls the quarterly path.
- **`capacity::compare_production`** gives a `CapacityReport`: two derived Claims (the ceiling and the grade needed at maximum feed and recovery) and a `CapacityVerdict`. `fmine-cli`'s `guidance_capacity.rs` calls it.
- **`recovery::fit` and `recovery::project`** give `Claim<Fit>` and a `Projection` of two model Claims, `ConstantTails` and `GradeDependentTails`. `fmine-public`'s `recovery.rs` calls them.
- **`recovery::render`** serializes retained recovery values for Claim inspection, including exact rational
  bounds, signed differences and explicitly numerical square-root enclosures. The codecs preserve the CLI's
  established JSON bytes; callers remain responsible for Claim identity, source and public-clearance checks.
- **`roll_forward::assess`** gives a `TableReport` (schema `fmine.pdf-table-check.v1`, `gate_eligible:false`). Conflicts carry the kernel certificate digest and minimum or inclusion-minimal correction sets with exact implied intervals. Feasible components carry the narrowed ranges satisfying all declared identities. The JSON report and its SHA-256 digest bind all declarations, admitted source readings, rejected candidate readings, reader roles, filing metadata, cut-off rows and results. **`assess_with_metrics`** also binds each reserve-replacement ratio to exactly one cited percentage definition in the metric registry, and the definition's full metadata enters the report digest. `fmine-cli`'s `tables.rs` calls `assess_with_metrics`.
- **`reserve_replacement::assess`** gives a `ReplacementReport` (schema `fmine.reserve-replacement-report.v1`): per series, a `SeriesAssessment` with annual results, totals, aggregate results, the joint ratio bounds and each headline's `HeadlineState`, or a `SeriesRefusal` with its evidence. The digest binds selections, rules, decks, source metadata, admitted readings and results. `fmine-cli`'s `reserve.rs` (`fmine reserve replacement`) calls it.
  `ReplacementReport::claims` and `SeriesResult::claims` recheck the calculation before producing canonical
  source Claims and one cumulative result Claim per computed series. `SeriesResult::check_claims` compares
  complete typed values and envelopes, including Claim v2's unhashed fields. Refused series receive no
  numeric Claim and remain in the original report. The CLI renders these identities and ancestry.
- **`roll_forward::quarter::assess`** gives a `Report`: a `Status` (`Derived`, `PrecisionNotEstablished`, `Contradicted`, `ConflictingDerivations`, `NotAvailable` or `Refused`), and every evaluated `Pair` with its exact interval, relation and rule (`H1-Q1`, `9M-H1` or `FY-9M`). JSON bounds are exact rational strings. `WARNING` says that derived information is not an issuer disclosure. No target-period Claim is made. `fmine-cli`'s `lint_publicness.rs` calls it.
- **`public_derivation::derive`** gives the H1, nine-month or FY `Claim<Amount>`, or a `Refusal`. `fmine-cli`'s `lint_publicness/sums.rs` calls it.
- DESIGN (BD-345.S4): `public_derivation::ownership_scale` accepts one full-mine `Claim<Amount>` and
  `Claim<OwnershipShare>` witnesses for the same issuer, subject, perimeter and physical period. Every
  exact share must agree and lie in (0, 1]. The result multiplies both closed amount bounds by that share,
  preserves every independent witness, source anchor and inherited Claim metadata, and never scales a rate.
  The CLI ownership adapter admits only printed shares from the amount's own dated source, retains other
  vintages as context, and rejects competing amounts or conflicting shares without selecting by draft value.
  These are arithmetic enclosures; no disclosure permission, reverse ownership conversion or gate pass is claimed.
- **`reserves::assess`** gives a `ReserveReport`: every admitted or refused `IssuerReserveRow`, the `CurrentSelection`, each `VintageComparison`, the `ReserveDeck` prices, the document metadata and `geo_checks`, with JSON, Markdown and digests over all of them. Each `reserves::geo::GeoCheckReport` gives the stated-deck result, the exact converted and reported bounds, every strict-convention fit, the declared and candidate sources and content digests. `fmine-cli`'s `tables.rs` calls it.
- **`mirror::compare`** gives a `MirrorReport` with a `MirrorVerdict`. `Consistent` carries a checked source-basis witness and a common full-basis Claim. `Conflict` carries the checked certificate/digest, blame status and inclusion-minimal source sets. `Indeterminate` has no common quantity. **`compare_capex`** gives a `CapexReport`: independent single-line checks in `lines`, and the separate `ComponentSum` assessment in `joint`. `fmine-cli`'s `mirror.rs` calls both.

## 4. Invariants

- DESIGN (BD-346.S5): each dollar adjustment field needs both reporters' own signed amount and the
  same target treatment; absence never becomes zero. All adjustments share their reporter's original
  source document, definition and ownership basis. Metal, denominator and allocation policies cannot
  be bridged with a dollar amount. Accounting adjustments apply only to cost of sales, never AISC or
  cash costs. Duplicate source rows, competing original totals and cross-asset or cross-period bridges
  refuse. Exact source boxes are summed before positive ownership division, without a zero clamp.
  Source tags bind raw tokens and readers; the kernel rechecks the token's half-step interval. Derived
  Claims preserve source parents, anchors, weaker evidence/knowledge and the complete UU-term union.
- DESIGN: cost and GEO declared worksheets share strict quoted-field, exact-quantity and calendar-period
  parsing, input-envelope admission, immutable receipt storage, derived Claim metadata and JSON rendering
  in the private `definitions::declared` module.
  Each metric keeps its own population, ownership, unit and allocation rules. Successful receipt schemas,
  field ordering and canonical identities are preserved, including their distinct colour/UU representations.
  Every source anchor, parent, exact metadata bit and typed refusal guard remains present.
- **Refuse, never clamp.**
  - `solve_cutoff_grade`: `test_cutoff_refuses_out_of_domain_inputs` and `test_cutoff_refuses_non_convergence`.
  - Cut-off reproduction: `test_no_positive_net_is_refused_not_clamped`.
  - Signed results stay signed: `signed_derived_requirements_are_never_clamped`, `signed_requirements_and_closed_ceiling_boundaries_are_preserved`, `negative_gaps_and_extrapolated_recovery_are_not_clamped` and `test_signed_nonpositive_cost_is_not_forced_into_a_logarithm`.
- **Exact arithmetic that overflows refuses with 207, and no partial result is returned.** `exact_overflow_is_a_numerical_refusal`, `exact_arithmetic_overflow_is_a_typed_refusal`, `zero_denominators_invalid_domains_and_overflow_refuse`, `test_invalid_precision_source_and_overflow_refuse` and `exact_sum_overflow_refuses_in_both_directions_without_partial_output`. In `roll_forward::quarter`, overflow gives `Refused`, never a success status: `exact_overflow_is_a_refusal_with_no_success_status`.
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
- **A source-bound cut-off keeps its source and is never corrected (BD-274).** Roles bind point-only readings from one registered technical report, with exact units, period, separate physical and price bases, hashes, locators and reader identities. The gold price is an exact stated parameter. The other inputs carry closed half-last-place boxes, and the published grade is rounded half up. Grades, price and total cost must be positive, cost and deduction tokens nonnegative, TCM in [0, 100] percent and effective recovery in (0, 100]. A TCM intercept above 100 is admitted while effective recovery stays valid. No unit or ownership conversion is implicit. One refused row keeps its evidence and the complete rows still run. `SourceRow::check_claims()` checks the typed values and envelope against the rebuilt Claims, since `Claim::check_id()` alone does not authenticate mutable typed values. `exact_constant_and_fixed_point_answers_keep_all_source_evidence`, `short_ton_output_and_multiple_named_deductions_use_exact_units`, `unphysical_domains_refuse_but_a_valid_intercept_above_one_hundred_is_admitted`, `partial_refusals_are_retained_and_all_relevant_inputs_remain_digest_bound`, `source_change_moves_computation_and_declared_conflicts_are_not_corrected` and `typed_claim_and_result_mutations_refuse_instead_of_minting_false_provenance`.
- **A cost definition counts only its registered lines.** An unregistered line refuses, and excluded lines are named in `left_out`. `test_aisc_counts_its_lines_and_names_what_it_left_out`, `test_non_sustaining_capital_is_in_aic_and_refused_in_aisc`, `test_co_product_needs_a_share_and_drops_credits` and `test_a_definition_change_moves_the_result_and_is_named`.
- **A cost receipt is arithmetic over a declared complete population (BD-061).** Omitted components are not verified zeros. Duplicate roles or items, missing sold quantities, unsupported currencies and unregistered lines refuse. Credits are nonnegative and subtracted once. Signed cost reversals and negative resulting rates stay signed. Neither ownership nor FX conversion is implicit: an ownership label never rescales a declared amount, and co-product allocation is a separate parameter. The existing AISC refusals for growth lines hold. Excluded inputs still affect evidence and identity, and reordering rows keeps the result and identity. The receipt binds representation, support, additivity, supersession, current flags, UU values and colour metadata that generic Claim identities omit. Floating metadata is bound by its exact bits and never enters the cost arithmetic. `crates/xtask/tests/cost_worksheet.rs` (`selected_versions_and_explicit_allocation_compute_distinct_useful_costs`, `pennies_and_actual_source_or_registry_bytes_change_the_receipt_identity`, `stated_calendar_and_attributable_basis_do_not_rescale_supplied_amounts` and `missing_or_ambiguous_cost_inputs_refuse_without_a_result`).
- **Declared drivers enter that same cost engine (BD-061, plan §21.6).** Each `DriverLine` computes exact quantity × USD rate + fixed USD before definition classification and allocation. Supported pairs are tonnes mined/milled in `dry_t` or `wet_t`, haulage in `dry_t_km` or `wet_t_km`, electricity in `kWh`, reagent in `kg` and labour in `hour`. Operands must be nonnegative; incompatible units refuse, and costs outside the USD 0.0001 lattice refuse instead of rounding. Direct and driver lines share item uniqueness. Canonical receipts retain every operand even when different inputs give the same dollar amount. Existing direct-cost encodings remain unchanged. These are declared assumptions, not fitted or calibrated rates. Direct known-answer/refusal tests and the independent Python Fraction oracle cover gold, copper, credits and co-product allocation.
- **Production GEO changes only the price ratio (BD-062).** Every included metal needs one quantity and a price in every deck. Contributions, totals and reference differences use unchanged quantities, and omitted metals stay outside the declared population. No unit, ownership or FX conversion is implicit. The worksheet parser refuses unknown, duplicate or missing fields. Receipts bind the full canonical inputs, source SHA-256 and field locators, metadata, ancestry and results. `crates/xtask/testdata/production_geo_contract.rs`: `gb27_preserves_physical_quantities_and_explains_only_the_price_ratio_difference`, `declaration_population_and_units_are_never_completed_or_converted_implicitly`, `all_decks_must_be_usable_and_share_currency_without_silent_partial_output` and `receipts_are_order_stable_but_bind_sources_deck_versions_and_unrendered_metadata`.
- **Guidance never substitutes production for sales, and never averages quarterly rates.** `sold_and_produced_are_separate_and_rates_are_not_averaged`, `correlated_sales_and_numerator_do_not_create_false_uncertainty` and `rounding_extrema_enclose_every_small_interior_combination`.
- **A history check needs one joint witness.** Separate production and cost successes do not count, and every witness is re-checked against its sources.
  - `separate_marginal_successes_are_not_a_joint_history_witness`, `fractional_history_witness_hits_a_boundary_exactly` and `historical_verdict_matches_exhaustive_discrete_continuations`.
  - Quarterly: `quarter_witness_reaches_both_targets_and_checks_every_original_source`, `aggregate_sales_rate_boxes_cannot_manufacture_a_quarterly_witness`, `separate_production_and_cost_successes_do_not_make_one_joint_history` and `small_point_histories_match_independent_integer_inequalities`.
  - No history, or a solver cap, gives `Indeterminate`: `missing_history_and_solver_caps_remain_indeterminate`.
- **The capacity ceiling scales production by the attributable share, never grade or recovery.** `ownership_scales_production_not_the_physical_limits` and `rounding_uses_upper_physical_bounds_and_never_averages_rates`.
- **The two recovery variances keep their denominators apart** (`SSE/(n−1)` and `SSE/(n−2)`): `residual_sample_sd_and_regression_error_use_distinct_denominators`. Wider input boxes never narrow an output: `widening_rounding_never_narrows_model_outputs`.
- **Recovery price scenarios retain signs and source rounding.** The exact USD-million multiplier is
  `100000000 / 45359237` times the declared USD/lb price for each kt. The independent known answer
  `0.45359237 kt = 1,000,000 lb` gives exactly USD 4 million at USD 4/lb, with `[-4, 8]` for the
  corresponding cross-zero quantity enclosure. `recovery::sensitivity::tests` checks this answer, negative
  and zero scenarios, tampered typed values, stale Claims and invalid declarations. Claim identities,
  canonical typed value tags, deterministic support and a complete full-site quarter are checked before
  multiplication. Exact arithmetic overflow remains a refusal.
- **Derived Claims keep their provenance and never gain strength.** `derived_claims_preserve_provenance_and_never_upgrade_evidence`, `derived_claims_keep_uncertainty_confidentiality_and_source_identity`, `ordering_is_deterministic_and_evidence_cannot_be_strengthened`, `fit_and_both_models_preserve_every_registered_venture_and_wall`, `summation_preserves_source_identity_weaker_metadata_and_unknown_terms`, `arithmetic_inherits_the_weakest_evidence_state_and_strongest_confidentiality` and `canonical_claims_preserve_issuer_evidence_and_complete_ancestry`.
- **A roll-forward identity binds only agreeing members (BD-340.S5, BD-331.S7).** Units, basis, member periods, raw-token precision, distinct reader identities, page bounds and source hashes must agree. Shared cells within a document are solved jointly by `exact_feas`; each feasible witness and infeasible certificate is checked again before emission. Different documents remain separate vintages.
- **A reserve-replacement ratio is exact and bound to one definition (BD-063).** `assess_with_metrics` uses the definition's declared numerator and denominator signs and scaling. The signed depletion interval must be strictly positive and bounded away from zero. Shared amounts enter the same joint solve as the sums and roll-forwards. Original source tokens are unchanged, and gold and GEO definitions, windows and deck keys stay distinct. There is no implicit metal or price conversion and no shared-rate nonlinear solve. `crates/xtask/tests/reserve_replacement.rs` covers it.
- **Declared PDF unit costs use the same joint exact solver (BD-331/340).** `gold_cost_per_sold_oz`
  binds `usd_m`, sold `koz` and `usd_per_oz` with factor 1000; `copper_cost_per_sold_lb` binds `usd_m`,
  sold `Mlb` and `usd_per_lb` with factor 1. All three agreed source assertions must match the declared
  period and basis in one document. Costs and rates retain negative printed values, while the complete
  sold interval must be strictly positive and bounded away from zero. Shared cost amounts participate
  jointly in sums and rates; quoted rates cannot be reused in unsupported bilinear identities.
  JSON records the native units and exact factor. The existing CLI retains citations, source-byte checks,
  evidence-date cuts and unresolved readings. These are declared arithmetic checks, not verification of
  an AISC/C1 accounting definition. `crates/xtask/tests/pdf_cost_rates.rs` covers exact witnesses, joint
  conflicts, tie boundaries and source admission; `table_oracle.py` exercises both kinds through the binary.
- **A replacement window is the whole declared window (BD-063).** `reserve_replacement` computes cumulative net changes over depletion in exact rationals. Negative net changes stay signed, and depletion follows the explicit source sign convention. The bases are attributable gold or GEO excluding acquisitions and divestments, in matching `oz`, `koz` or `Moz` with no implicit conversion. Each GEO year keeps its named deck, and already-converted GEO is never converted again. Source periods, hashes, publication chronology, unique cells and reader roles are rechecked. Annual printed sums and separately cited aggregate totals keep their own points and closed half-step enclosures, and their bounds intersect into the joint ratio enclosure. Disjoint constraints refuse with all the evidence, without replacing either point. Rounded headlines are assessed for interval compatibility; approximate language stays unassessed, with no invented tolerance. Missing, duplicate, mismatched, zero-crossing-denominator or overflowing inputs refuse. `crates/xtask/testdata/reserve_replacement_unit_tests.rs` (`ratio_of_cumulative_amounts_preserves_negative_net_changes`, `separate_rounded_totals_narrow_bounds_without_overwriting_annual_points`, `contradictory_totals_refuse_with_all_source_evidence`, `approximate_language_gets_no_invented_tolerance`, `sign_convention_is_explicit_and_zero_crossing_denominator_refuses` and `geo_retains_each_named_deck_without_reconverting_reported_values`) and `crates/xtask/tests/reserve_replacement_report.rs`.
- **A derived quarter keeps both operands' displayed uncertainty (BD-345.S4).** A printed value `v` with positive step `s` is the closed enclosure `[v−s/2, v+s/2]`, and subtraction is `[a.lo−b.hi, a.hi−b.lo]`. Every admitted cumulative/prior pair is assessed. A draft is `Derived` only when its own displayed enclosure contains every pair's enclosure and the competing assertions for each cumulative period share a common interval. Numeric agreement never selects a source vintage. `cumulative_rules_preserve_exact_rounding_and_signed_zero_results`, `every_competing_pair_is_retained_and_a_matching_vintage_never_wins_a_tie` and `missing_or_unusable_members_and_incompatible_series_refuse_positive_derivation`.
- **A quarter sum needs complete, disjoint quarters of one series (BD-345.S4).** Issuer, subject, scope, metric and unit, explicit ownership and calendar year must match. Closed source bounds add in exact rationals and keep their signs. `quarter_sums_require_complete_disjoint_half_or_full_year`, `different_basis_unit_scope_and_year_do_not_mix`, `addition_propagates_rounding_and_keeps_zero_as_a_real_observation` and `canonical_but_unusable_and_placeholder_inputs_never_become_summed_amounts`.
- **The current reserve row is the latest eligible filing, and nothing falls back silently (BD-335).** Only registered MD&A and AIF documents supply current rows, chosen by the latest declared publication date. Declared rows are considered before reading availability, so an unusable newer filing blocks fallback to an older one, and same-date filings stay ambiguous. Investor decks only cross-check. Partner filings and technical reports are refused as implicit issuer filings. All supplied earlier vintages stay visible. The economic key (issuer, subject, reserve year, item, metal, unit, ownership and contained or recoverable support) excludes the deck. A quoted deck binds positive agreed gold and copper price readings from the same document, with their period, basis, units and citations. A different or unstated deck gives a named comparison state and asserts no reserve restatement. Comparable rows intersect their exact declared rounding intervals and keep open ends. `latest_filed_row_wins_and_later_deck_never_replaces_it`, `missing_new_value_or_deck_price_blocks_silent_old_filing_fallback`, `same_date_filings_remain_ambiguous_even_with_one_unusable_row`, `deck_change_updates_current_without_claiming_a_reserve_restatement` and `half_open_tie_bounds_do_not_report_false_compatibility`.
- **A GEO reserve check keeps the stated deck (BD-340.S4, BD-070).** Three source-verified amounts share document vintage, subject, reserve item and year, period, ownership and support. Gold and GEO are in Moz and copper in Mlb, with no implicit conversion. The declared convention controls the stated-deck result, and proper fits, tie-only contact and empty intersections stay distinct. Candidates need independently bound prices but not their unrelated reserve amounts. Missing candidate prices prevent discovery and keep every named source and any usable stated-deck result. Identical parameter keys keep every provider, and distinct equal-ratio decks stay ambiguous. A found key depends on the supplied candidate set. It is never a correction, a claim about the issuer's calculation or a document-wide convention. `crates/xtask/testdata/reserve_geo_unit_tests.rs` (`cited_conversion_and_alternative_deck_findings_do_not_replace_the_stated_deck`, `equal_ratios_remain_ambiguous_and_identical_parameters_keep_every_source_provider`, `missing_or_unusable_amounts_and_prices_never_create_a_unique_finding` and `alternate_convention_does_not_override_the_declared_document_convention`).
- **A reserve report binds everything it was given.** A registry-wide refusal binds all supplied declarations, documents and raw and admitted readings in a canonical input digest, keeping duplicates, so a change to a rejected input changes the digest. `inputs_are_order_invariant_and_rejected_metadata_remains_digest_bound`, `registry_refusal_binds_invalid_declarations_documents_and_raw_and_admitted_values`, `registry_refusal_fingerprint_includes_identities_and_preserves_duplicates_and_order` and `geo_checks_are_order_invariant_and_bind_candidate_evidence_and_membership`.
- **The mirror works on one common 100% quantity (BD-346.S3).** Scale identities express it; the existing exact feasibility solver and independent checkers produce a witness or exclusion certificate. Fixed tolerances and inferred partner shares are absent. At least two reporters are required, but their shares need not sum to one. The nonnegative physical domain is explicit for gold: a printed zero keeps its original negative-to-positive rounding box, while the common interval is intersected with `x >= 0` and the reported witness is checked there. Canonical tags bind reporter identity, numeric source anchor, operation membership, inclusive validity dates and the exact typed quantity. Inputs, normalized Claims and proof cells share one sorted order, and minimal-source indices address that order. Conflict sets' disjoint normalized intervals independently establish minimality; an incomplete blame search remains explicit.
- **Capital compares only on equal stated bases (BD-346.S4).** Missing or unequal definitions need an explicit bridge and refuse here. Signed capital corrections are kept, with no physical-zero floor. `compare_capex` keeps the three independent half-step line checks apart from the declared `ComponentSum` joint assessment. The joint model needs each reporter's complete, unambiguous triple on one ownership basis, checks displayed total = displayed sustaining + displayed project, and takes the true total's bounds from the component boxes. It never selects a construction to rescue an instance. A missing triple or an ambiguous line on any reporting ownership basis leaves the joint assessment explicitly unavailable, even when another complete triple exists for that reporter. Gold value tags are unchanged; capital tags also bind the stated basis and its source. `mirror::capex::tests`: `same_basis_capex_retains_units_citations_and_signed_amounts`, `capital_basis_is_required_cited_and_cannot_be_silently_bridged`, `capex_sum_identity_sustaining_plus_project_has_checked_witness`, `independent_line_agreement_cannot_hide_a_joint_footing_conflict`, `component_sum_uses_component_rounding_not_the_total_half_step`, `incomplete_ownership_triple_is_explicit_and_input_order_is_immaterial`, `independently_rounded_capital_components_preserve_known_truth` and `a_complete_attributable_triple_cannot_hide_ambiguous_full_basis_lines`.

## 5. Refusals
`lmdi`, `lmdi::waterfall`, `guidance`, `guidance::quarters`, `capacity`, `recovery`, `public_derivation`, `definitions::claims` and `definitions::geo` refuse with these codes:
- **`FMINE-REF-102`** (support mismatch; exit 3): a non-aggregate or invalid month support, or periods that do not line up. From `guidance`, `quarters`, `capacity`, `recovery`, `public_derivation`, `definitions::claims` and `definitions::geo`.
- **`FMINE-REF-103`** (non-additive average; exit 3): additivity that does not match the quantity. From `guidance`, `capacity`, `recovery`, `public_derivation`, `definitions::claims` and `definitions::geo`.
- **`FMINE-REF-104`** (non-identifiable attribution; exit 3). From the waterfall: a sold-ounce box that touches zero, unmatched or ambiguous line identities, or lines that do not sum to the cost. From `recovery`: a denominator box that touches zero, fewer than three annual observations, or a duplicate year. The zero-denominator and too-few-rows cases use the nearest code.
- **`FMINE-REF-105`** (basis mismatch; exit 3): a kind, unit, subject, ownership or AISC-definition mismatch, or repeated history ids. From every Claim kernel, and from the waterfall for unequal bases. `guidance` also uses it for an attributable share outside (0, 1].
- **`FMINE-REF-107`** (undeclared unit, basis or time zone; exit 3): an undeclared subject or definition, or an unprinted waterfall basis. From `guidance`, `capacity`, `recovery`, `public_derivation`, `definitions::claims`, `definitions::geo` and the waterfall.
- **`FMINE-REF-108`** (assay lookahead; exit 3), used for time order: a history at or after the guidance year, or a quarter that does not follow every training year. From `guidance`, `quarters` and `recovery`.
- **`FMINE-REF-109`** (outside physical domain; proposed, exit 3). From the legacy LMDI: a non-positive or non-finite input, or recovery above one. From the waterfall: a negative half-step, or a share outside (0, 1]. From the Claim kernels: reversed bounds, or a value outside its domain.
- **`FMINE-REF-201`** (no claim in domain; exit 3): a missing source locator or digest, a history with no id or no quarters, or no operands. From the waterfall, `guidance`, `quarters`, `capacity`, `recovery`, `public_derivation`, `definitions::claims` and `definitions::geo`.
- **`FMINE-REF-202`** (not observable; exit 3): an input whose state is neither `Known` nor `Estimated`. From `guidance`, `capacity`, `recovery`, `definitions::claims` and `definitions::geo`.
- **`FMINE-REF-203`** (outside AOA; exit 3).
  - From `guidance`, `capacity` and `recovery`, for an open interval bound. The kernels need closed rounding bounds. This is the nearest code.
  - From `AiscConsolidationBridge::verify_rollup`, when the numerator gap exceeds 0.5 $M (Validity domain).
- **`FMINE-REF-204`** (unbudgeted unknowns; exit 3): a UU term with an unnamed field or a negative or non-finite downside, or one UU id with two definitions. From `definitions::claims` and `definitions::geo`.
- **`FMINE-REF-205`** (calibration required; exit 3): an input that failed calibration. From `guidance`, `capacity`, `recovery`, `definitions::claims` and `definitions::geo`.
- **`FMINE-REF-207`** (numerical certificate failed; proposed, exit 3): exact overflow, a failed witness or certificate, an invalid enclosure, or a failed closure. From every Claim kernel, both LMDI paths, `quarters` and `mirror::capex`.
- **`FMINE-REF-208`** (source cell unusable; proposed, exit 3): a placeholder input. From `guidance`, `capacity`, `recovery` and `public_derivation`.
- **`FMINE-REF-301`** (MNPI wall; exit 7): a private source. From `public_derivation` and `mirror`.
- **`FMINE-REF-502`** (stale anchor; exit 6): a stale input. From `guidance`, `capacity`, `recovery`, `public_derivation`, `definitions::claims` and `definitions::geo`.
- **`FMINE-REF-507`** (frozen record mutation; exit 6): a source Claim whose metric or value differs from its canonical identity. From `public_derivation`, `mirror`, `definitions::claims` and `definitions::geo`.

The worksheet parsers, `definitions::worksheet` and `definitions::geo::worksheet`, also refuse an oversized, malformed or incomplete worksheet with `FMINE-REF-506` (input missing or unreadable), and a digest that does not match its bytes with `FMINE-REF-505` (digest mismatch).

`mirror` refuses unsupported cost definitions/kinds, unequal assets and invalid shares (`105`), unequal periods or invalid dates (`102`), missing/unequal operation perimeters or validity straddles (`104`), missing numeric/context citations (`201`), private sources (`301`), placeholders (`208`), stale/unknown/conflicted/failed-calibration inputs (`502`/`202`/`205`), and mutated canonical Claims (`507`). Reversed bounds or negative physical-gold midpoints refuse (`109`); signed capital amounts stay valid. Missing, unstated, uncited or mismatched capital timing or lease definitions refuse (`104`), and mismatched capital scope or monetary units refuse (`105`). There is no accounting-basis adjustment in the capital API. Open source bounds refuse (`203`). Arithmetic overflow/checker failures refuse (`207`); solver limits remain `Indeterminate`. `mirror::transcriptions::bind` refuses mismatched numeric or ownership document names, hashes and pages before construction.

`roll_forward` does not refuse a table for one bad identity. A missing, ambiguous or incompatible member keeps an `IdentityRefusal` with its available evidence, and independent complete identities still run. `cutoff::source` keeps each refused row in `TableReport.cutoffs` with its available evidence, and a refused row makes no numeric Claim; a registry-wide failure refuses every cut-off row. `reserve_replacement` keeps a refused series as a `SeriesRefusal` with all its evidence, and `reserves::geo` keeps a refused check inside its `GeoCheckReport`.

`roll_forward::quarter` reports, rather than raises:
- **`Refused`** for malformed or duplicate source identities, unequal semantic series, unusable admitted values, non-positive precision, unsupported cumulative periods, checked arithmetic overflow, or more than 4,096 candidate pairs;
- **`NotAvailable`** when either period is missing;
- **`PrecisionNotEstablished`** when the enclosures overlap but the draft's precision is not reached;
- **`ConflictingDerivations`** for mutually inconsistent period assertions or differing pair conclusions;
- **`Contradicted`** when every pair's enclosure is disjoint from the draft's.

A dash is never zero, signed differences stay signed, and no failure selects a numerically convenient subset. Source identity and publication-cut admission are the adapter's duty.

`reserves` refuses a row as a `ReserveRefusal` kept in the report: an unsupported source role, a wrong price, unit, period, reader or document metadata (`unsupported_sources_are_refused_and_deck_only_has_no_current_row`, `source_guards_reject_wrong_prices_units_periods_readers_and_metadata`).

The other modules have local errors with no refusal code:
- **`CutoffError`**: `NonPositiveCost`, `NonPositivePrice`, `NonPositiveGuess`, `RecoveryOutOfDomain` and `NotConverged`.
- **`MetricFailure`**: `UnknownDefinition`, `NotACostDefinition`, `UnregisteredItem`, `NonPositiveOunces`, `AllocationRequired`, `AllocationNotApplicable` and `Exact`.
- **`CutoffFailure`**: `Registry`, `Row`, `Exact`, `NoPositiveNet`, `NoPositiveRecovery` and `Solver`. `fmine-cli` maps it to `FMINE-REF-506`.
- **`ReserveReplacementBridge::compute`** returns `None` for a depletion that is not positive and finite.

## 6. Determinism class
**DeterministicPerIsa** for the crate, the weakest of its files' `//! Determinism:` headers:
- **Exact:** `capacity.rs`, `definitions/claims.rs`, `definitions/geo.rs`, `definitions/geo/worksheet.rs`, `definitions/worksheet.rs`, `guidance.rs`, `guidance/quarters.rs`, `mirror.rs`, `mirror/capex.rs`, `mirror/transcriptions.rs`, `public_derivation.rs`, `reserve_replacement.rs`, `reserves.rs`, `reserves/geo.rs` and `roll_forward/quarter.rs`. These use checked rational arithmetic, canonical input tags and sorted identities. The Claim and GEO receipts bind inherited colour and UU metadata by its exact floating-point bits.
- **BitwisePortable:** `lmdi.rs`. Its two golden digests, `golden_digest_logarithmic_mean_10k` and `golden_digest_decompose_aisc_lmdi_10k`, agreed on x86_64-linux and aarch64-macos in CI run 41237993223 at 84f32ac (bd-hp2j.3). `test_golden_digest_logarithmic_mean_10k` and `test_golden_digest_decompose_aisc_lmdi_10k` pin them.
- **DeterministicPerIsa:**
  - `lib.rs` has no cross-host digest test (plan §29.2, bd-hp2j.1);
  - `cutoff.rs`, `cutoff/source.rs` and `cutoff/source/claims.rs` have exact grades and bounds, but their solver cross-check uses floats;
  - `roll_forward.rs` table checks are exact, but its optional cut-off rows inherit that float cross-check;
  - `definitions.rs` is exact, but `PriceDeck` hands floats to `compute_geo`;
  - `lmdi/waterfall.rs` has exact rates and additive bars, with portable-log LMDI enclosures;
  - `recovery.rs` has no cross-host test of its whole output, which includes square-root summaries.
- **The test files** `lmdi/waterfall/tests.rs` and `recovery/tests.rs` declare DeterministicPerIsa.

## 7. Evidence class of outputs
- **`guidance` and `capacity`** give `Derived`, met with every parent's class. So a result is never stronger than its weakest input. An issuer `Assertion` stays an `Assertion`. Epistemic status and state take the weaker of their parents'. Confidentiality is joined, and anchors, parent ids and UU terms are kept. The exact set lives in the value, so the representation is `Deterministic`.
- **`public_derivation`** keeps the parents' identities, source anchors, evidence, knowledge, confidentiality and UU terms. The result is arithmetic over assertions, never a new issuer disclosure or a physical truth. `WARNING` says so.
- **`definitions::claims` and `definitions::geo`** derive a Claim that inherits the weakest evidence and knowledge state and the joined confidentiality, includes every admitted parent and source anchor, and keeps consistent UU definitions. Claims imported from either worksheet stay `Assumed` and `Internal`, and GEO ones uncalibrated. `claims::NO_CLAIM` and `geo::NO_CLAIM` state the conditional reading.
- **`cutoff::source`** Claims stay issuer assertions. The result Claim keeps every input Claim as a parent with its source anchors, and rounding bounds are not confidence intervals.
- **`reserve_replacement`** Claims use an explicit `ClaimBasis`: registered issuer assertions remain
  `ISSUER`, while invented evidence remains `Assumed` and `Unknown`. The derived result retains every
  source Claim and locator/hash, the original signed amounts, named definitions/decks, distinct annual
  and aggregate points, joint rounding bounds and approximate-headline status. No calibration or reserve
  truth is asserted. Changing precision, dates or source bytes changes the derived identity even when
  the printed percentage is unchanged. Document authentication remains at the public CLI boundary.
- **`recovery`** starts from `ModelledUncalibrated`, `KnownUnknown(Parametric)` and `Estimated`, then meets its sources'. A model output is never calibrated here (NL-4). It adds one model UU term, named by `RULES_VERSION` and `MODEL_CAVEAT`. Price sensitivity additionally meets an explicitly `Assumed` price Claim; a declaration digest does not authenticate a commodity-market price.
- **`reserves`** rows keep their fields private and report a constant `EvidenceClass::ISSUER`, so relabelling cannot promote them. `reserves::NO_CLAIM` states that they are issuer assertions, not classified reserves; `reserves::geo::NO_CLAIM` says the same of the GEO checks.
- **`mirror`**'s normalized and common Claims retain immediate source parents, anchors, weaker evidence/knowledge/epistemics, and every distinct UU term, including different terms sharing a scenario id. No assertion is promoted. `mirror::transcriptions::bind` preserves the supplied evidence class.
- **The other outputs carry no Claim and no evidence class.** These are the root items, `definitions`, `cutoff`, both LMDI paths, `roll_forward` and `roll_forward::quarter`. `WATERFALL_NO_CLAIM`, `WATERFALL_RULES_VERSION`, `recovery::NO_CLAIM`, `roll_forward::NO_CLAIM`, `reserve_replacement::NO_CLAIM`, `quarter::WARNING` and `mirror::NO_CLAIM` state the descriptive reading. A `Conflict` or `Reproduced` cut-off state speaks to the registered inputs only, not to whether they suit the mine.

## 8. Validity domain
- **The root items are bare `f64`.** Four L-6 rows in `registries/l6_allow.toml` cover them. Three are debt: `compute_geo`, `ReserveReplacementBridge::compute` and `solve_cutoff_grade`. One is generic: `logarithmic_mean`.
- **`verify_rollup` checks the numerator only, to a fixed 0.5 $M.**
  - It compares the caller's site sum, plus L_corp if asked, with the caller's consolidated numerator. Either mode refuses on an absolute gap above 0.5 $M. The tolerance is fixed, not taken from a declared rounding construction.
  - It does not re-add the site rows, and does not check the L_JV sales, the consolidated sales, either AISC or the rounding bridge.
- **`compute_geo` checks nothing.** A zero or non-finite price gives an infinity or a NaN.
- **`ReserveReplacementBridge::compute` checks only depletion.** Non-finite additions pass into the percentages. The basis is whatever the caller supplies.
- **`solve_cutoff_grade`** stops when the undamped step falls below 1e-10 in grade units, within 100 iterations. It reports the residual and does not bound it. `cutoff` holds it to the exact fixed point within 1e-6 g/t.
- **`definitions` takes the amounts as given.** The registry says what counts, not that an amount is right.
- **A cost receipt** is arithmetic over declared inputs. It is not an authenticated filing read, an independent completeness review, a cost-classification judgment or completion of the WGC real-data gate.
- **Production GEO** is conditional on one declared population and the named decks. A price-ratio difference establishes no physical production change, source accuracy, reserve classification or disclosure authority.
- **The legacy cut-off section** is source-unverified arithmetic over registered tokens: neither the report bytes nor independent readings are authenticated. **`cutoff::source`** reproduces stated technical-report assumptions. It is not an optimal operating cut-off, a reserve classification or completion of the real-data gate.
- **The legacy LMDI** is float. Its spend and sold ounces must share a scale. It encodes no units or basis, and its closure tolerance is 1e-9 USD/oz absolute.
- **The typed waterfall** fixes its units: $M, koz, thousand tonnes, g/t and a fraction. It checks that both periods carry the same basis tag, but cannot authenticate that basis. Its enclosures treat inputs as independent, so they over-enclose shared ones.
- **Guidance** works on closed intervals, with sales and AISC strictly positive and production at least zero. Its results are necessary arithmetic conditions, not forecasts or probabilities. `T1` is the supplied history only. `ExceedsEnvelope` says nothing about physical capacity.
- **Capacity** multiplies the three maxima. That gives an optimistic ceiling, not a plan. `NotExcluded` is not a feasibility witness.
- **Recovery** fits two descriptive annual models and applies them to quarters. Its enclosures cover input rounding, not sampling error. An extrapolated recovery outside 0–100% is kept as computed.
- **The PDF identity adapter (`roll_forward`, BD-340.S5)** verifies declared arithmetic of issuer assertions, not their truth or completeness. It never emits reserve classifications, selects a preferred filing/deck vintage, silently changes a printed sign, or supplies an issuer-attributed corrected figure. Synthetic E692-shaped arithmetic is a known-answer test of the binding; the authentic historical transcription and independent census gates remain open. Closed intervals are agnostic about ties; a named half-open convention is never changed to rescue a row. No file I/O occurs here: callers authenticate actual registered bytes and reader agreement before public use.
- **Reserve replacement** is not reserve validation. Actual file authentication and independent reader agreement remain the caller's duty.
- **The additive quarter kernel and the quarter sums** authenticate no workbook bytes, publication dates or ownership registry. The caller must admit all eligible evidence first. They give no probabilistic confidence, source ranking, implicit unit conversion, subtraction of non-additive rates, or permission to disclose. A non-obvious arithmetic consequence can itself be new information. These are exact arithmetic enclosures, not forecasts or confidence intervals.
- **`reserves`** is a projection, not a durable append-only store, a complete filing census or an `EstimateStatement`. A `reserves::geo` finding is conditional on the supplied candidate set.
- **The mirror** authenticates no filing bytes, human readership, publication date or issuer affiliation; its caller must bind those facts before supplying source Claims. It does not infer missing perimeters, aggregate incomparable operations, select vintages, invent an absent partner share, adjust capital definitions, infer cost-line bridges, transfer bridges between assets, or decide which reporter is physically correct. The independent total check and the joint total can differ, because `ComponentSum` uses the components' rounding bounds. Its transcription binding consumes already-agreed S0 rows, without authenticating bytes or publication availability; accepted rows alone cannot prove that an omitted rejection did not exist.

## 9. Oracles
- **Exact hand known answers**, all synthetic:
  - `synthetic_completion_known_answers_are_exact`;
  - `cod_01_ceiling_and_required_grade_are_exact`;
  - `independent_exact_fit_and_two_model_known_answers`;
  - `test_recovery_na_exact_signed_fallback_known_answer`;
  - `test_fixed_point_cutoff_grade_solver`;
  - `exact_constant_and_fixed_point_answers_keep_all_source_evidence` for `cutoff::source`;
  - the invented GB-27 fixture, `crates/xtask/testdata/production_geo.toml`: 3,860,000 and 3,640,000 GEO oz, a −220,000 oz difference from its reference (`gb27_preserves_physical_quantities_and_explains_only_the_price_ratio_difference`);
  - capital: a 101 total on 60 + 40 is a checked joint conflict although both reporters agree on every line. Conversely 1 + 1 = 2 and 2 + 2 = 4 admit true components 1.5 + 1.5 = 3 under `ComponentSum`, although the independently rounded total boxes do not overlap;
  - `roll_forward::quarter::tests`: four tests with independently calculated decimal half-step subtraction for Q2, Q3 and Q4, signed and zero results, a narrower draft precision, all competing pairs, input-order invariance, internally conflicting assertions even under a coarse draft, missing and invalid operands, ambiguous series, duplicate ids, unsupported periods and exact overflow. The T-layer source-to-lint controls separately exercise workbook display precision, source identity, dates, ownership and perimeter admission.
- **Independent readers.** `cost_receipts_pass_independent_fraction_and_source_checks` and `cost_library_and_binary_keep_identical_json_and_markdown` in `crates/xtask/tests/cost_worksheet_binary.rs` check cost receipts with independent fraction arithmetic and the CLI binary.
- **Public Claim derivations.** `crates/xtask/tests/public_lint_derivations.rs::derived_amount_claims` retains the seven direct public-API tests for exact quarter sums, overflow refusal, source admission and inherited evidence beside the source-to-command acceptance tests.
- **Exhaustive and independent checks.**
  - Integer enumerations: `historical_verdict_matches_exhaustive_discrete_continuations` and `small_point_histories_match_independent_integer_inequalities`.
  - Corner sweeps: `rounding_enclosures_contain_every_synthetic_training_and_operating_corner`, `all_synthetic_rounding_corners_are_enclosed_and_overflow_refuses` and the two waterfall corner tests.
  - `independently_rounded_capital_components_preserve_known_truth` compares 64 independently rounded attributable capital worlds with their generated exact truth.
- **High-precision known answers** for the logarithmic mean: `test_logarithmic_mean_high_precision_tiny_and_large_known_answers` and `test_logarithmic_mean_enclosure_known_answers_near_equality_and_tiny_scale`.
- **Seeded populations** from Philox streams: `test_cutoff_property_seeded`, `test_property_fallback_exact_random_lines_with_negatives`, `test_property_lmdi_order_independent_and_enclosure_contains_point` and the two golden digests.
- **The mirror (BD-346, `mirror::tests`).** Ten focused tests cover the exact three-source tie at 100 koz; the 1000/615/385 case; independent witness/certificate checks and corrupted proofs; minimal conflict pairs; zero's explicit domain; two 45% reporters and sold quantities; reporter-order/full-basis metamorphisms; dated scope and evidence/UU preservation; same document basenames across issuers; semantic, source, canonical-mutation and numerical-cap refusals. A separate integer-rounding oracle keeps the known truth inside the common interval for 256 synthetic worlds. This is not the full BD-346.S6 power study. Direct transcription-binding tests additionally check exact source/share Claims, and `mirror::transcriptions::tests` covers the capital keys.
- **The registries.**
  - `test_seven_corpus_cutoffs_e182_e184` runs every reference row of `registries/published_cutoffs.toml` and pins each row's state. It recomputes the registered tokens; it does not read the source report.
  - The `definitions` tests read `registries/metrics.toml`, including `test_price_decks_read_from_the_registry`.
- **`repro/barrick/waterfall_census.py` and `repro/barrick/recovery_census.py`** are independent readers for hand-entered public cells. No workflow runs them.
- **No fixture** under `fixtures/`.

## 10. Known gaps

- DESIGN (BD-346.S5): the bounded dollar-cost bridge has synthetic exact and source-loader controls in
  `metrics_mirror_cost_controls.rs` and `mirror_cost_oracle.py`. Authentic partner filings, complete
  accounting-definition equivalence, general policy bridges and the full BD-346 power/data gates
  remain separate work. Supplied common allocation policies are conditional inputs, not independently
  established equivalence, and bridges are never transferred between assets.
- DESIGN (BD-060/BD-061): the exact cost worksheet also admits copper pounds sold under the
  issuer-specific Q2 2026 C1 and copper AISC table definitions. Reconciliation deductions remain
  separate from by-product credits; co-product allocation removes only by-product credits.
  Explicit sold quantity and allocation metals must match the definition. Gold input tags and
  JSON field names remain unchanged; copper reports pounds and USD/lb. Source authentication,
  account classification, completeness and reproduction of rounded issuer rates remain separate.
  The invented example at `tests/costs/declared_copper.toml` gives C1 49/20 and AISC 3 USD/lb.
- **The old contract had the wrong registry lines.** It said Subsystem Enterprise and 16k/18k. The registry says Bullion and 18,000/21,000.
- **The old dependency list was wrong.** It omitted `fmine-registry`, which `cutoff` and `definitions` use. It listed `fmine-time`, which `Cargo.toml` declares but no source file uses.
- **The old contract named PO-MET-1 to PO-MET-8**, with PO-MET-6 used twice. The plan has none of them. The plan's rows are in Proof obligations.
- **The old contract said `verify_rollup(false)` refuses when L_corp ≠ 0. That is false.** It refuses on the gap between the site sum and the consolidated numerator, whatever L_corp holds. An L_corp of 0.5 $M or less never refuses. The doc comment on `verify_rollup` says the same false thing.
- **The old contract labelled 203 `ConsolidationBridgeMissing`.** No such key exists. The catalogue's 203 is `outside_aoa`, used here as the nearest code.
- **The published cut-offs E182–E184 are not yet reproduced from the source.** `cutoff` recomputes the registered tokens and says `source_verified:false`. `cutoff::source` supplies the source binding, but the authentic reproduction remains a separate data gate.
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
- **Two GEO paths and two reserve-replacement paths.** The root `PriceDeck` and `compute_geo` are float and unchecked beside the exact `definitions::geo`. `ReserveReplacementBridge` is float beside `reserve_replacement` and `assess_with_metrics`.
- **Much of plan §21.6 remains open.** The copper C1 table rule and the shared exact cost calculation
  accept supplied plans or actuals, including declared quantity/rate/fixed-cost drivers, but do not fit rates
  or authenticate accounting inputs. ROCE, broader definition flags (credit location and inventory
  adjustments) and a fitted `CostDriverCard` remain absent.
- **Code with no caller outside this crate's tests:**
  - `AiscConsolidationBridge`, `PriceDeck`, `compute_geo` and `ReserveReplacementBridge`;
  - `is_non_sustaining` (`all_in_cost_per_oz` is reached through `definitions::claims` and `fmine metrics cost`);
  - the legacy `LmdiState` and `decompose_aisc_lmdi`;
  - the single-period `compare_history` and `check_witness`.
  - `solve_cutoff_grade` runs only inside the cut-off cross-checks.
- **Codes are not used the same way.** An attributable share outside (0, 1] gives 105 in `guidance` but 109 in the waterfall.
- **No cross-host digest** for `lib.rs`, `cutoff`, `cutoff::source`, the typed waterfall or `recovery`.
- **The real-data gates are open.** Actual FY2023–FY2025 partner-source reproduction, the full synthetic power gate (BD-346.S6) and partner-filing capital coverage remain separate BD-346 work. The cost receipt does not complete the WGC real-data gate.
- **The quarter work does not close the wider gates.** The additive quarter kernel and the quarter sums do not close the general derived-identity or the independently labelled disclosure-lint assessment gates.

## 11. Public API
- **`consolidation`:** `RATE_RULES_VERSION`, `Printed`, `Column`, `Table`, `Difference`,
  `RateComparison`, `Comparison` and `compare`. `Table` and `Comparison` expose `canonical_tag`;
  comparisons cross the API in a Claim. Direct known-answer and signed source-box enclosure tests are
  `consolidation::tests::signed_rate_bridge_has_exact_known_answers_and_rounding_bounds`,
  `marginal_rate_bounds_enclose_every_signed_source_box_corner` and
  `unavailable_rate_domains_keep_the_dollar_comparison_and_bind_the_refusal`.
- **Root:** `SiteAiscRow`, `AiscConsolidationBridge::verify_rollup`, `PriceDeck` with `barrick_reserve(year)`, `GeoBreakdown`, `compute_geo`, `ReserveReplacementBridge::compute`, `CUTOFF_MAX_ITER`, `CUTOFF_TOL`, `CutoffSolution`, `CutoffError` and `solve_cutoff_grade`. The root re-exports `LmdiState`, `LmdiWaterfall`, `decompose_aisc_lmdi` and `logarithmic_mean`.
- **`definitions`:** `bundled_registry`, `CostLine`, `MetricFailure`, `AllInCost`, `all_in_cost_per_oz`, `Project` and `is_non_sustaining`.
  - `CostUnit`, `UnitCost` and `cost_per_sold_unit` use explicitly typed gold-ounce or copper-pound
    sold denominators; negative credits/deductions and incompatible registered units refuse.
    `claims::CostSummary` retains `unit`, `quantity_sold`, `primary_metal_share` and `usd_per_unit`.
  - `claims`: `INPUT_METRIC`, `OUTPUT_METRIC`, `NO_CLAIM`, `Scope`, `DeclaredDriver`, `CostValue`, `CostInput`, `Treatment`, `Contribution`, `CostSummary`, `CostReceipt`, `definition_tag` and `evaluate` (BD-061).
  - `worksheet`: `SCHEMA` (`fmine.cost-worksheet.v1`) and `evaluate`, with JSON and Markdown rendering (BD-061).
  - `geo`: `INPUT_METRIC`, `OUTPUT_METRIC`, `NO_CLAIM`, `MAX_DECKS`, `Metal`, `Scope`, `DeckKey`, `PriceDeck`, `GeoValue`, `GeoInput`, `Contribution`, `DeckResult`, `GeoSummary`, `GeoReceipt` and `evaluate`; `geo::worksheet`: `SCHEMA` (`fmine.geo-worksheet.v1`) and `evaluate` (BD-062, GB-27).
- **`cutoff`:** `CUTOFFS_SCHEMA`, `CUTOFFS_REGISTRY`, `CutoffFailure`, `Printed`, `GradeUnit`, `Recovery`, `CutoffCase`, `CutoffState`, `CutoffReport` with `to_json`, `reproduce`, `parse_cases`, `CutoffSection` and `section`. `cutoff::source`: `NO_CLAIM`, `ROUNDING_RULE`, `SourceRow` with `claims` and `check_claims`, `SourceReport` with `digest`, `to_json` and `to_markdown`, and `assess` (BD-274).
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
- **`recovery::sensitivity`:** `RULE`, `NOTICE`, `CopperPrice`, `Sensitivity` and `assess`; prices provide
  `canonical_tag` and `declared_claim`, and derived sensitivities provide `canonical_tag`.
- **`roll_forward`:** `SCHEMA`, `NO_CLAIM`, `assess`, `assess_with_metrics`, `TableReport` (with `cutoffs`), `SourceCell`, `ComponentCheck` and `IdentityRefusal` (BD-340.S5, BD-331.S7, BD-063).
- **`reserve_replacement`:** `SCHEMA`, `NO_CLAIM`, `Bounds`, `ExactValue`, `Reading`, `AnnualResult`, `Totals`, `AggregateResult`, `JointBounds`, `HeadlineState`, `HeadlineResult`, `SeriesResult`, `SeriesRefusal`, `SeriesAssessment`, `ReplacementReport` with `digest` and `to_json`, and `assess` (BD-063).
  Canonical APIs: `ClaimBasis`, `InputRole`, `InputClaim`, `ReplacementValue`, `ReplacementClaims::to_json`,
  `SeriesResult::{claims, check_claims}` and `ReplacementReport::claims`.
- **`roll_forward::quarter`:** `WARNING`, `Quantity`, `Printed`, `Observation`, `Status`, `Pair`, `Report` and `assess` (BD-345.S4).
- **`public_derivation`:** `INPUT_METRIC`, `SHARE_METRIC`, `OWNERSHIP_SCALE_RULE`, `WARNING`, `AmountKind`, `Amount`, `OwnershipShare`, `Rule`, `derive` and `ownership_scale` (BD-345.S4).
- **`reserves`:** `NO_CLAIM`, `ReserveKey`, `SourceRole`, `ReserveDeck`, `IssuerReserveRow`, `ReserveRefusal`, `CurrentSelection`, `VintageComparison`, `ReserveReport` and `assess` (BD-335). `reserves::geo`: `NO_CLAIM`, `GeoAmount`, `GeoDeckInput` and `GeoCheckReport` with `digest`, `to_json` and `to_markdown` (BD-340.S4, BD-070).
- **`mirror`:** `READING_METRIC`, `NO_CLAIM`, `Perimeter`, `MirrorMetric`, `MirrorQuantity`, `MirrorReading`, `MirrorCommon`, `MirrorReport`, `MirrorVerdict` and `compare` (BD-346). Capital (BD-346.S4): `CapexScope`, `CapexTiming`, `LeaseTreatment`, `CapexBasis`, `CapexReport`, `CapexJointReport` and `compare_capex`. `mirror::transcriptions`: `BoundReadings`, `bind`, `keys` and `period_label`. `bind(subject, period, kind, issuer, document, rows, evidence)` returns the separately cited ownership Claim, when needed, and source reading Claims. Capital keys are `capex_sustaining`, `capex_project` or `capex_total` with `_100_usd_m` or `_attr_usd_m`, the source unit is exactly `USD_m`, and the context needs `timing`, `leases` and the definition's own page, table, row and column.

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
