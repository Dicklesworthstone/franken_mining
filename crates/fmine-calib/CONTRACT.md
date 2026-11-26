# CONTRACT.md — `fmine-calib`

## 1. Purpose
Pure calibration, forecast pre-registration (`INV-008`), strictly proper scoring rules, reference-class priors, spatial front PIT monitoring (`G-CAL-1`), and guidance-score assessment kernel (Plumb, layer `K0`; no I/O per `L-3`).

- **Crate:** `fmine-calib`
- **Layer:** K0
- **Subsystem:** Plumb
- **LOC budget:** 27,000 planned / 31,000 ceiling
- **Allowed Module Families:** `["ledger", "scoring", "prior", "pit", "record", "guidance_scores", "spatial"]`

## 2. Inputs
### Mathematical & Domain Formulation

- DESIGN (BD-334/BD-068): `record::scoring::assess` validates the frozen site/target universe and scores each
  live forecast exactly once with exact rational interval loss. Unknown, duplicate and omitted resolutions
  refuse; explicit unresolved members remain in the frozen denominator. Per-target output separates observed
  coverage and mean loss from whole-cohort completion bounds. Unresolved intervals contribute at least their
  width to the full mean loss; no finite upper loss is invented. Not-published frozen sites remain separately
  visible. Exact effective sample-size endpoints use the declared rho range as sensitivity, not an estimated
  correlation. The generalized certified `clopper_pearson_exact` uses the declared confidence level with the
  existing outward integer-binomial/dyadic bounds. Its iid Bernoulli reference is not a calibration verdict.
  This kernel reads no files, authenticates no timestamps and selects no public sources; the xtask adapter
  must verify the record and bind original-release source Claims before calling it.
  `record::scoring::compare` computes forecast, issuer-guidance and persistence losses on one identical
  complete member set. Every live forecast needs an explicit benchmark row; omitted, duplicate or foreign
  rows refuse, and inverted intervals refuse even when actuals are missing. Signed and point comparator
  intervals are valid. Every excluded member retains all applicable missing-actual/guidance/persistence
  flags. Empty common sets have no means or score differences. Negative forecast-minus-baseline differences
  favor the forecast under the frozen loss, without asserting skill, calibration or probability semantics
  for an issuer's range. The ordinary assessment retains the full frozen population independently.
  Each comparator also has exact full-frozen-cohort mean-loss-difference completion bounds. For shared
  alpha, `interval_score_difference_bounds` evaluates the four endpoint knots: the difference is piecewise
  linear and both tails are constant. Known outcomes contribute their exact difference; unresolved outcomes
  contribute the global extrema over all real values, divided by the full live-member count. Missing
  comparators leave bounds absent with named members; arithmetic refusal is local to its completion and
  preserves observed means and the other comparator. Different alpha values cannot use this cancellation.
  These arithmetic completion bounds are neither confidence intervals nor predictive-skill claims.
Implements the append-only pre-registration `ForecastLedger` (`INV-008`, `FMINE-REF-206`), strictly proper
scoring rules (`gaussian_crps`, `gaussian_log_score`, `winkler_interval_score`), `ReferenceClassPrior` and
`SpatialFrontPitMonitor` (`GB-08` Vecchia/Rosenblatt conditional PIT; plan §10.4's location score `tanh(z/2)`
and centred dispersion score `tanh((z^2 - 1)/2) - c0`, each on a `ScoreBettingEProcess` with aGRAPA stakes
at alpha/2).

FACT (the `GREENFIELD_PORPHYRY_CAPEX` constant in `src/lib.rs`). The built-in example declares a reference
mean of 1.28 times the baseline, a standard deviation of 0.35 times its magnitude and a mixture weight of
0.25. Historical fitting and transfer to a specific project remain open in `bd-2jow`.

## 3. Outputs
DESIGN (bd-2jow.1). `ReferenceClassPrior::blend` reports the mean and standard deviation of a
two-component mixture with declared parameters. A zero or unit mixture weight returns the active
component. The within- and between-component variances preserve small spreads at large cost scales.
These moments alone do not identify a cost-overrun probability or a project contingency percentile.

Enforces pre-registration of forecasts prior to outcome observation (`INV-008`) and monitors spatial front miscalibration (`G-CAL-1`), emitting typed assessment receipts, proper scores, calibration-gate verdicts, and exact rational completion bounds.

## 4. Invariants
- `FrozenForecast`, `ForecastPayload`, `ForecastLedger`
- DESIGN (BD-048/181, plan §10.2): `compare_supplied_assessments(left, right, PairedScore)` compares
  two existing immutable supplied-outcome assessments through the same exact interval and finite CRPS
  owners as public comparison. This explicit lane matches the unique prospectively frozen
  `(domain, site, horizon, quantity, unit)` tuple; matched targets also require the same outcome deadline.
  Model family and author remain visible but do not determine physical identity. Every cohort must have
  complete frozen metadata and at most one forecast for each target. Resolution rule, eligible sources,
  quantity, unit, alpha, assessment cut and outcome selector must agree. Public-bound and untagged legacy
  cohorts refuse rather than receiving an inferred target mapping.
  Observed pairs require both equal numeric outcomes and identical complete citations. Missing forecasts,
  unresolved outcomes and selection mismatches remain in the union. Unresolved compatible pairs use the
  existing exact global interval/CRPS difference extrema; whole-union averages remain absent when any
  target lacks a forecast, has an incompatible selection or requires unrepresentable arithmetic.
  CRPS requires finite empirical or exactly weighted empirical distributions on every member, including
  unmatched targets. These are supplied, unverified target and source declarations, not authenticated
  public outcomes or empirical calibration. Separate `SuppliedPairedAssessment` receipts use SINT/v1
  for interval loss and SCRP/v1 for finite CRPS; public PAIR/PCRP/GCRP encodings and durable journal bytes
  remain unchanged. Six independent kernel controls cover exact weighted scores, missingness, prospective
  identity, full citations, compatibility refusals and unsupported distributions.
  DESIGN (BD-048): `PairedScore::Brier` adds exact paired binary-event losses to the same supplied-target
  path. Every forecast in both complete cohorts must be Bernoulli, including unmatched members. Original
  probabilities, including zero and one, are retained without a log-score floor. The difference
  `(p_left - p_right) * (p_left + p_right - 2y)` has its binary completion extrema at zero and one;
  equal probabilities cancel even when their individual rational representation exceeds capacity.
  Missing outcomes remain in the denominator, while missing forecasts, source mismatches and arithmetic
  failures keep affected aggregates unavailable. SBRI/v1 binds this separate comparison; existing
  interval, CRPS, forecast and assessment encodings remain unchanged.
  DESIGN (BD-048/181): `compare_supplied_assessments_full_crps` dispatches homogeneous finite or Gaussian
  populations; `compare_supplied_assessments_gaussian_crps` exposes the Gaussian owner directly.
  Every union member is checked before scoring, including unmatched forecasts. Gaussian scores and
  missing-outcome extrema reuse the public Gaussian calculation with approximate arithmetic, while the
  full prospective target, deadline, source-selection and compatibility checks above remain in force.
  `SuppliedGaussianAssessment` and SGCR/v1 keep these numerical diagnostics separate from exact receipts.
  Missing pairs and unrepresentable arithmetic make the corresponding aggregate unavailable; no subset
  is substituted for the whole union. Gaussian extrema are not certified numerical enclosures or
  confidence intervals. Existing public Gaussian and exact supplied encodings remain unchanged.
- DESIGN (BD-067/BD-334): `record::candidate::build_candidate` generates a complete draft live-site × target
  record from already selected quarterly observations. The named `pooled-quarterly-ratio-empirical-v1` model
  uses each target's frozen persistence pool and inverse empirical CDF: the latest observation times
  the pooled median, with ordered scaled tail bounds. Only the canonical gold AISC/TCC and copper C1/AISC
  cost targets admit signed or zero live observations; production and unsupported targets still require a
  positive point. Negative multipliers reverse the tail endpoints, while the point remains the multiplier
  times the ratio-pool median. Its exact interval equals that comparator's interval on identical
  inputs, so it has the same interval score before export rounding; the changed point estimate is no evidence
  of independent forecasting skill. Historical missing or nonpositive observations omit only their adjacent
  pairs, with both endpoints retained in an exclusion audit. Missing live members or point observations,
  duplicate/foreign members, malformed pool dates, empty ratio pools and exact overflow refuse the entire
  candidate. Named not-published sites remain explicit. Output is deterministic in frozen rules order.
  The exact rational rows and actual serialized decimal rows are separate: at a declared 0–12 decimal places,
  lower bounds round down, upper bounds round up and points round to nearest with ties toward positive infinity.
  Signed decimal formatting uses exact arithmetic and emits no negative zero. Every CSV row
  says `uncalibrated=true` and passes the existing complete-record parser. Export rounding can change scores;
  it never silently replaces the exact calculation. Limits are 4,096 live members and 65,536 history values.
  The pure kernel authenticates no source, availability time, author attestation or external time proof and
  does not create an issued or registered forecast. Its adapter must bind every history and point value to
  the original-release cell, source digest, unit, ownership basis and information cut before publication.
  `build_candidate_with_model` additionally accepts the explicit
  `CandidateModel::SeasonalQuarterlyRatio`, named `seasonal-quarterly-ratio-empirical-v1`.
  It derives the destination calendar quarter from the common validated outcome horizon and admits only
  positive adjacent-quarter ratios ending in that quarter, pooled across the frozen live sites and years.
  Q4-to-next-year-Q1 transitions retain their original adjacent dates. The frozen pool and complete history
  slots remain unchanged: other-season pairs retain both endpoints in `off_season_pairs`, while missing
  or nonpositive selected pairs remain in `excluded_pairs`. Gaps are never bridged. An empty selected
  pool refuses the complete candidate without falling back to other seasons. The same exact quantiles,
  signed/zero cost treatment, rounding and resource bounds apply. The existing `build_candidate` wrapper
  keeps the all-season model; the registered persistence comparator and its encodings are unchanged.
  Seasonal intervals may differ from that comparator, but model selection supplies no evidence of
  seasonality, calibrated coverage or independent forecasting skill.
  DESIGN (BD-067/334): the explicit `PooledQuarterlyChange` and `SeasonalQuarterlyChange` models
  instead use exact adjacent `after - before` differences in the target's unit. Their identifiers are
  `pooled-quarterly-change-empirical-v1` and `seasonal-quarterly-change-empirical-v1`. Each latest value
  receives the pool's median and tail quantiles additively. Signed and zero cost/capital history remains
  usable; production history and its live point must be nonnegative. An explicitly numeric zero point
  can therefore receive a nonzero restart interval under these additive assumptions; ratio models
  continue to require positive production points. Missing or
  inadmissible pairs remain explicit, with no bridging or seasonal fallback. A negative exact production
  bound refuses before decimal export rather than rounding or clamping to zero. Supported field/outcome
  pairs, not target aliases, control admission. `CandidatePool::kind` separates dimensionless ratios from
  unit-bearing changes; the pure API's change fields cannot mislabel one as the other. Frozen CSV/rules
  and the ratio comparator are unchanged. Pooling absolute changes across sites assumes transferable
  changes at that target's scale; it estimates no site effect, relative growth or calibrated probability.
  Direct controls in `record::candidate::tests` cover signed/zero capital and all sold-metal cost targets,
  seasonal selection, missing quarters, physical-domain refusal, exact rounding, translation and overflow.
- DESIGN (BD-047/BD-049, plan §10.1): `register_payload_with_metadata` freezes a complete
  `ForecastMetadata` beside the predictive distribution: family, author role, domain, horizon, site,
  quantity and unit. Every label is nonempty, trimmed, at most 256 bytes and free of control characters.
  These are declared role/model identifiers, not personal names or independently verified identities.
  The opt-in uses FOR2/v2; `register_payload` and compatibility `register` retain their exact prior identities
  and an absent metadata field. No existing registration can acquire labels retrospectively.
  A tagged cohort requires the same exact family, quantity and unit for every member and refuses mixed
  tagged/legacy membership. This prevents dimensioned aggregate scores knowingly pooling incompatible or
  unknown quantities. The new COH1/v5 frame binds the complete metadata and an explicit policy-layout
  discriminator; all legacy COH1/v1–v4 and assessment bytes remain unchanged. Public-bound cohorts additionally
  require site to equal the bound subject, quantity to equal the public metric key and unit to equal its
  canonical unit. Ownership remains separately frozen in the public policy; grouping labels cannot override it.
- DESIGN: `segmented_assessment(saved_digest)` derives a deterministic `SegmentedAssessment` only from an
  already saved immutable assessment. Each group retains its full model/family/author/domain/horizon/site/
  quantity/unit key, member IDs, resolved and unresolved counts, missing reasons, observed scores and
  full-group completion bounds under the original policy. No new outcome selection, time sample or authority
  seal occurs. `DescriptiveSummary` has no calibration verdict or effective-sample-size field. Legacy-only
  reports list every member as unclassified without inferring labels or pooling scores. SGM1/v1 supplies a
  separate digest binding the exact parent report, complete partition and descriptive calculations;
  later outcomes and restatements cannot rewrite an earlier report. These groups are neither confidence
  intervals nor an author ranking, and they do not establish independent observations or predictive skill.
- DESIGN (BD-048/049, plan §10.2): `compare_assessments(left_digest, right_digest)` compares two saved public
  assessments without selecting outcomes again. The frozen issuer, metric/unit, ownership, calendar period,
  vintage permission and alpha must match, as must the assessment cut and outcome selector. Subjects are
  paired by their frozen mappings; resolved pairs require identical numeric values and complete citations.
  The union retains missing forecasts, unresolved outcomes and publication mismatches. Exact rational interval
  losses use the frozen binary values and interval endpoints; negative left-minus-right differences favor
  the left cohort. The observed mean includes only identical selected publications. Whole-population bounds
  require both forecasts and compatible selections for every subject; unresolved pairs use the existing exact
  four-endpoint global extrema at common alpha. Checked arithmetic failure keeps members explicit and makes
  the affected result unavailable. PAIR/v1 separately binds both parent assessments and the comparison;
  existing forecast, cohort, assessment and journal identities remain unchanged. Registration times and
  information sets may differ. These completion bounds are not confidence intervals or a predictive-skill
  claim. Independent examples and source-selection controls are in `xtask/tests/forecast_comparison.rs`.
- DESIGN (BD-048/049, plan §10.2): `compare_assessments_crps(left_digest, right_digest)` uses the same
  immutable public-target and selected-publication pairing to compare complete empirical or exactly
  weighted empirical distributions. Gaussian, quantile-only and interval-only payloads refuse, including
  on unmatched subjects; no supported subset is silently selected. CRPS differences use checked exact
  rationals of the frozen binary support values and original exact weights. Uniform draws retain mass
  `1/n`, duplicates retain their probability, and weights are never normalized or resampled. Sorting
  and two linear CDF sweeps compute each pair in O(n log n) time and O(n) space. The derivative of the
  left-minus-right difference is `2(F_left-F_right)`, so it is linear between the union's support atoms
  and constant in both tails. Its exact global extrema supply unresolved-pair completion bounds without
  an invented outcome domain. The complete union, arithmetic failures, source mismatches and missing
  forecasts retain the original comparison semantics. PCRP/v1 binds the parent assessments, method,
  member results and aggregates separately; default interval PAIR/v1 bytes are unchanged. These are
  descriptive arithmetic bounds, not confidence intervals, prospective model-race evidence, independence,
  calibration or predictive skill. Direct analytical, pair-distance-oracle, invariance, unsupported-payload
  and source/capacity controls are in `xtask/tests/forecast_comparison.rs`.
- DESIGN (BD-048/049, plan §10.2): `compare_assessments_gaussian_crps` uses the identical public-target
  and selected-source pairing for a complete Gaussian population. `compare_assessments_full_crps`
  dispatches to this separately typed projection or the unchanged exact finite-distribution owner;
  mixed Gaussian/finite populations and unsupported distributions refuse, including unmatched subjects.
  Gaussian values are explicitly approximate. The difference derivative is twice the CDF difference:
  equal standard deviations give monotonicity; otherwise one standardized CDF crossing plus the two
  infinite-tail limits supply all global extrema. The kernel evaluates these without constructing an
  overflowing physical crossing or subtracting huge nearly equal CRPS values. Scaled tail corrections
  retain density contributions that would underflow before multiplication by a large standard deviation.
  Missing-outcome extrema are approximate analytic completion diagnostics, not certified enclosures,
  confidence intervals or calibration evidence. Every missing/mismatched subject and finite-capacity
  failure remains explicit; no partial population bound is substituted. GCRP/v1 binds the unchanged
  saved parent digests and new projection. Existing PAIR/v1 and PCRP/v1 identities remain unchanged.
  Controls: `ledger/comparison/gaussian/tests.rs` and `xtask/tests/forecast_comparison.rs` cover independent
  closed-form answers, stationary extrema, distant tails, common location/unit transformations, complete
  membership, source mismatch and representability boundaries.
- DESIGN (BD-049, plan §10.3): `correction_proposal(saved_digest, raw_forecast_id)` makes an explicit,
  unissued correction for a later unresolved Gaussian forecast from one complete saved assessment.
  At least two positive forecast/outcome pairs are required; every model, family, author, domain, horizon,
  site, quantity and unit must match. No unresolved or incompatible member can be silently excluded.
  The target registration must follow the saved cut and all training outcome boundaries; any target outcome
  event or conflicting target-cohort alpha refuses. Selected observations are copied from the saved report,
  never reselected after later publications. The factor is `exp(mean(log(actual/raw_mean)))`; sample log-ratio
  dispersion is descriptive. At the frozen alpha, inflation is the inverse empirical `(1-alpha)` quantile
  of `abs(actual-factor*mean)/(factor*std*z)`, floored explicitly at one and rounded outward when inflated.
  Corrected Gaussian mean is `factor*raw_mean` and standard deviation is `factor*inflation*raw_std`.
  Finite positive products, noncollapsed Gaussian interval endpoints and actual fitted interval coverage
  are checked; no arithmetic success is fabricated.
  Zero log dispersion preserves scaled predictive width. Inflation never narrows the bias-rescaled width;
  a bias factor below one can still reduce absolute width relative to the raw forecast.
  COR1/v1 binds the method and no-claim text, parent report, full segment,
  unchanged raw forecast, selected source outcomes, corrections and proposal. No existing record is changed.
  This is a pooled within-segment proposal, not hierarchical fitting, calibration attainment or independent
  coverage evidence; prospective registration and later scoring remain separate.
- DESIGN (BD-049, plan §10.3): `correction_series_proposal(saved_digests, raw_forecast_id)` applies that same
  fitting arithmetic to an explicitly selected series of complete saved assessments. COR2/v1 binds every
  parent report/cohort, every selected member and source event, and all frozen target cohorts. Repeated
  reports, cohorts/revisions, forecasts or identical cited observations refuse; no member is filtered out.
  Selectors, exact alpha, model and all segment labels must match. Public series additionally require one
  issuer, metric, ownership, vintage permission and calendar period kind, with advancing non-overlapping
  training supports and a later compatible frozen public target. Quarter/year lengths may differ with the
  calendar; cumulative and quarterly supports are never pooled. Supplied series require the same resolution
  rule and eligible sources. Target registration must follow every training cut, forecast deadline and
  retained outcome's availability/record time, with no target outcome event. Canonical parent ordering makes
  input-list permutations identical. Later restatements cannot alter earlier selected reports. The original
  single-assessment API and COR1/v1 bytes are unchanged. The series retains unissued/uncalibrated status and
  does not establish independence, a complete historical record, hierarchical fitting or future coverage.
- DESIGN (BD-049, plan §10.3): `correction_proposal_with_method` and
  `correction_series_proposal_with_method` take an explicit `CorrectionMethod` as their final argument.
  `MultiplicativeLog` is the unchanged default of the original wrappers. `AdditiveResidual` admits finite
  signed and zero Gaussian means and outcomes without changing population, source, chronology or segment
  admission. It fits `shift = mean(actual - raw_mean)` in the target unit; sample residual dispersion is
  descriptive. At the frozen alpha, inflation is the inverse empirical `(1-alpha)` quantile of
  `abs(actual - (raw_mean + shift))/(raw_std*z)`, floored at one and rounded outward when inflated.
  Corrected mean is `raw_mean + shift`; standard deviation is `raw_std*inflation`. Zero residual
  dispersion never collapses width, and additive inflation never narrows the raw width. Checked finite
  arithmetic, representable endpoints and actual fitted coverage remain mandatory. No automatic fallback
  changes the requested method. Reports expose the method, mean error, sample error dispersion and
  adjustment; member errors are log ratios or dimensioned residuals according to that method.
  Additive COR1/v2 and COR2/v2 bind distinct method/no-claim text and all unchanged raw/source/parent/target
  evidence. Exact legacy COR1/v1 and COR2/v1 regression digests remain fixed. This does not establish
  independence, hierarchical fitting, calibrated future coverage or an observed public-data track record.
- DESIGN (BD-048/049, plan §10.2): `assessment_diagnostics(saved_digest)` derives Gaussian PIT values and
  fixed-decile histograms, Bernoulli reliability bins, and exact empirical CDF rank intervals from the
  selected outcomes in an immutable saved assessment. Every member and event/source identity is retained.
  Missing Gaussian outcomes widen each bin's whole-population completion bounds; missing event outcomes
  stay in their original probability bins. Full-population and resolved-only mean probabilities are separate,
  and empty-bin averages remain absent. The normal CDF retains its existing approximate tail saturation.
  DESIGN (BD-042, NL-8). For a Gaussian cohort with the same explicit model and full metadata key
  at a frozen 80% nominal level, the supplemental score is
  `1 − min(1, max(|cov80 − 0.8| / 0.2, KS_PIT))` under the saved calibrated flag. Other flags
  yield zero. The two-sided empirical CDF distance uses both limits at every sorted PIT value
  and requires every outcome. The score is recomputed from the saved assessment and its bound
  PIT values. DGN1 identities and the existing calibration flag remain unchanged.
  Empirical forecasts yield exact counts below, equal to and above the selected outcome, with closed bounds
  `[F(y-), F(y)] = [below/n, (below+equal)/n]`. One linear scan uses comparisons without sorting or subtraction;
  signed zeros are equal and ties remain intervals. Separate empirical decile bins count resolved intervals
  wholly contained (definite) or intersecting (possible). Edges are exact integer fractions `i/10`, left-inclusive
  and right-exclusive except at 1; exact edge points go right. Whole-population bounds use definite counts as
  lower numerators and possible plus unresolved counts as upper numerators over every empirical member.
  No randomized PIT, continuous interpolation, uniformity claim or pooling with Gaussian PIT is introduced.
  Weighted empirical forecasts have a separate rank population and retain exact probability mass below,
  equal to and above the selected outcome. Their CDF bounds are `[below, 1-above]`; weights are never
  replaced with counts or floating-point probabilities. Exact decile comparisons avoid multiplication
  overflow even at the largest admitted denominator. Rational summation capacity failures refuse the
  projection without modifying its saved parent. DGN1/v3 binds these weighted masses, populations and
  rules only when weighted members exist. Reports without them retain their DGN1/v1-v2 bytes.
  Sparse quantiles and standalone intervals remain unsupported for this diagnostic projection.
  Forecast, cohort and assessment identities are unchanged. The projection supplies no calibration verdict,
  independence estimate, confidence interval or retrospective outcome selection.
- DESIGN (BD-048/049): `coverage_diagnostics(saved_digest)` separately reports central 50%, 80% and 95%
  interval coverage from that same saved selection. Gaussian and empirical distributions supply their
  central quantiles; explicit intervals and quantile tables require exactly matching declared levels,
  without interpolation or tolerance-based substitution. Bounds are inclusive; empirical ties may produce
  more than the nominal mass. Each level retains supported, unsupported, resolved and unresolved counts,
  hits and both miss sides. Completion fractions use all supported members, including unresolved outcomes;
  empty denominators remain absent. Every member retains its bounds or explicit unavailable reason.
  CVG1/v1 binds the parent, selected-event/source evidence and these results without changing DGN1 or earlier
  forecast/cohort/assessment identities. Pure JSON and Markdown renderers expose this descriptive projection;
  no calibration, independence or confidence claim follows. Public-API acceptance tests live in
  `xtask/tests/forecast_diagnostics.rs`; private bin/count edge tests remain beside the kernel.
- DESIGN (BD-048/049): `gaussian_log_diagnostics(saved_digest)` exposes the existing Gaussian negative
  log-density score for each saved Gaussian member, including valid negative losses. Its named Gaussian
  population retains unresolved members in full-population completion bounds; other distributions remain
  explicit. The frozen outcome domain determines convex minimum/endpoint maximum bounds; without one the
  upper bound is unbounded. Arithmetic failure retains the member and invalidates affected averages.
  Values use the existing floating-point kernel and are not certified outward enclosures. Scores depend on
  the frozen target unit: scaling it by a positive factor adds that factor's logarithm. GLS1/v1 binds the
  exact parent, parameters, selected events/sources and projection without changing earlier durable bytes.
  Assessment JSON and saved-report Markdown expose these descriptive scores without a calibration verdict.
  Independent density answers, missing/domain controls, numerical failure and immutable replay controls
  live in `xtask/tests/forecast_diagnostics.rs`.
- DESIGN (BD-048/049): `quantile_diagnostics(saved_digest)` supplements the saved assessment with twice-pinball
  loss at **every supplied quantile**, including interior levels. The preregistered central interval score
  remains unchanged. At level 1/2 this loss is absolute error; for exactly complementary levels p and 1-p,
  the pair's sum divided by 2p equals the interval score. No CDF, interpolated level or CRPS is inferred.
  Checked i128 rationals score the exact frozen IEEE-754 values, not their decimal display strings; JSON
  numerators and denominators are strings to preserve their full precision. Representation, multiplication,
  summation or mean overflow is explicit arithmetic indeterminacy, never a rounded value or dropped member.
  Each exact probability has its own supplied/resolved/unresolved population, with absent levels and other
  distributions visible. Observed means require every resolved loss; completion means retain every supplied
  member and use only the frozen outcome domain. Their convex loss minima occur at the quantile clamped to
  that domain and maxima at an endpoint; without a domain the minimum is zero and the upper bound unbounded.
  QNT1/v1 binds the parent assessment, frozen domain, full source-bound membership, table and exact results.
  Later outcomes cannot rewrite it; parent forecast/cohort/assessment, DGN1 and CVG1 identities are unchanged.
  JSON and journal Markdown expose both individual losses and per-level completion bounds. This supplemental
  descriptive projection changes no calibration flags and establishes no independence or predictive skill.
  Known-answer, precision, missing-outcome, arithmetic-capacity and source-tamper controls live in
  `xtask/tests/forecast_quantiles.rs`; transcript/journal/reopened-report agreement is in `forecast_live.rs`.
- BD-343: `freeze_cohort(CohortSpec)` freezes sorted membership, the resolution rule, eligible sources and
  `AssessmentPolicy` before outcomes. The digest binds the complete payload, including exact float bits.
  Payloads are Gaussian, a uniform empirical distribution, an interval with a stated or explicitly absent
  level, quantiles, or an event probability.
  Both legacy Gaussian and exact-payload registrations require record time strictly before the outcome
  boundary. Equal-time and late attempts refuse before changing the population or time watermark;
  `ledger::tests::legacy_registration_requires_strict_precedence_without_advancing_history` covers this
  boundary and the valid one-nanosecond-before case. Numerical probes preserve their historical reduction
  order without admitting invalid timestamps or changing the committed cross-host golden values.
  Forecast registrations, cohort freezes and outcomes share a monotone record-time watermark: an already
  observed miss cannot be excluded by backdating a new cohort containing only other, unresolved members.
- DESIGN: `ledger::binding::PublicResolutionPolicy` freezes registered-public resolution semantics in the
  existing `CohortSpec.resolution_rule` string. Its reserved `fmine.public-resolution.v1:` prefix contains a
  canonical `PRP1`/v1 frame encoded as lowercase hexadecimal. It binds issuer, metric, explicit unit and source
  field, calendar FY/H1/nine-month/quarter support, original release, exact ownership fraction and every
  forecast-id-to-subject mapping. `YYYYH1` is January through June and first resolves from `YYYYQ2`;
  `YYYY9M` is January through September and first resolves from `YYYYQ3`. A cumulative target remains
  distinct from the quarter ending on the same date, including in its policy digest and shared-target check.
  Existing FY/quarter frame bytes remain unchanged. Cumulative policies use the same original-only or
  explicitly permitted comparative-vintage rules and period-end deadline checks.
  Metrics are gold production (`production_koz`, koz), copper production (`production_kt`, kt), and
  issuer-reported AISC and total cash cost per sold gold ounce in USD/oz, and AISC and C1 cash cost per sold
  copper pound in USD/lb. `aisc_usd_per_oz`, `tcc_usd_per_oz`, `aisc_usd_per_lb` and `c1_usd_per_lb` freeze
  distinct metric/field
  identities; their costs remain intensive and may be signed. Copper production binds the printed `cu_prod_100_kt` or
  `cu_prod_attr_kt` source field without any conversion to pounds. `PublicMetric::is_production` identifies
  both extensive, nonnegative production quantities for admission and replay. Adding copper AISC and cash-cost targets
  leaves prior target encodings and original/comparative permissions unchanged. Full production and
  attributable production remain distinct even at an attributable fraction of one. Adding copper preserves
  the existing gold policy bytes and does not change original-only or comparative-vintage permissions.
  Site physical targets add `processed_kt`, `processed_grade_g_per_t`, `processed_grade_fraction` and
  `recovery_fraction`. Each freezes one complete Minestats section/row key and its exact `kt`, `g_per_t` or
  `mass_fraction` unit; circuit recovery and mined-grade rows cannot replace site recovery or processed grade.
  Tonnes remain extensive on the declared printed ownership basis. Grade and recovery remain intensive and
  are never multiplied by a share. The two fraction targets preserve percent display as a source token and
  score its exact fraction (for example `91%` becomes `91/100`); unformatted percent quantities need a distinct
  unsupported target. The source adapter requires matching printed ownership context and validates both
  stored and displayed physical domains. Existing target frames retain their exact bytes.
  Reported capital targets add `capex_usd_m`, `capex_sustaining_usd_m` and `capex_projects_usd_m`.
  They freeze the total-capital row or the minesite-sustaining/project row under the capital-expenditures
  section, in `usd_m`. These are distinct extensive monetary amounts, including signed credits; they never
  enter the nonnegative physical-domain guard. The exact printed ownership remains mandatory. Total capital
  is not reconstructed from the components, and a label alias cannot change the reported scope. No cash/accrual,
  lease, FX or ownership bridge is inferred. Original and permitted comparative receipts retain their source
  Claims and exact displayed amounts, and old target frames and identities are unchanged.
  `validate`, `to_resolution_rule`, `parse_resolution_rule`, period and ownership helpers perform no I/O.
  `PublicVintagePolicy::OriginalOnly` keeps those exact v1 bytes and permissions. An opt-in
  `RegisteredComparatives` policy is encoded by `to_resolution_rule_with_vintages` as a
  `fmine.public-resolution.v2:`/`PRP1`-v2 rule with an explicit mode string. Its first admission still requires
  the original release; `validate_release` allows separate restatements only from later canonical calendar
  quarters. `parse_resolution_rule_with_vintages` returns both target and permission; the original parser
  retains its target-only API. No v1 record is reinterpreted as permitting a comparative. Authentication,
  the existence of an original outcome and the latest predecessor remain admission responsibilities.
  Legacy free-text rules retain their existing behavior; malformed reserved rules refuse.
  `freeze_cohort` requires the policy's members to equal the cohort exactly and its issuer to be the sole eligible
  source. Each forecast deadline must be at or before the final nanosecond of the target calendar period,
  while registration and cohort freeze remain strictly before that deadline. A forecast shared by multiple
  public cohorts must retain the same issuer, metric, period, ownership, subject and outcome-vintage permission
  in each; cohort populations
  may differ. Within one public cohort, each physical subject can occur only once, so multiple models of the
  same outcome must use separate cohorts and cannot inflate the calibration denominator.
  Binary events cannot be reinterpreted as production or cost quantities. Source authentication,
  original-release selection, printed rounding and publication availability remain adapter responsibilities;
  neither the period end nor this pure policy authenticates when a forecast or source became available.
- `append_outcome` records hash-chained `Published`, `Restated` and `Missing` events with separate availability
  and recording times. A restatement must name the latest numeric predecessor and a new cited source vintage.
  Every typed cohort's frozen vintage permission is rechecked before appending a numeric outcome, including
  when callers use the kernel directly. A typed public restatement must strictly advance the latest numeric
  release. Refused publications or restatements do not advance authority time.
  The original forecast is never mutated. Eligibility and a declared physical outcome domain are checked.
- `assess(cohort, as_of_ns, FirstPublished | LatestAsOf)` retains an immutable assessment. Both event times must
  be at or before the cut. Later events cannot be backdated into an already assessed cut. Every member remains
  visible, including Pending, Overdue, NotPublished, Withdrawn, DefinitionChanged and Divested outcomes.
  A successful assessment seals its cut across the ledger, including records outside that cohort. Historical
  queries remain permitted and never lower the sealed cut; equal-time mutation batches are allowed only above it.
- An `AssessmentSummary` carries all denominator counts; observed coverage and whole-cohort coverage bounds;
  observed and whole-cohort score bounds; the effective sample range under the frozen correlation assumptions.
  Brier bounds are finite. Interval-score upper bounds need a registered physical outcome domain. Unresolved
  Gaussian CRPS preserves its legacy conservative bounds unless finite-domain scoring is explicitly frozen;
  empirical CRPS can use a finite frozen domain as described below.
  These are completion bounds, never confidence intervals. Guidance with no
  nominal level remains descriptive. A positive calibration flag requires complete resolution and a conservative
  effective sample at least the frozen threshold (minimum 35; stricter thresholds are allowed). The default
  correlation range is [0, 0.5].
- DESIGN (BD-048/BD-343): `AssessmentPolicy.gaussian_crps_bounds` opts into Gaussian CRPS completion bounds
  over the frozen `outcome_bounds`. The domain must be finite; members must be Gaussian or empirical with
  at least one Gaussian. A missing Gaussian contributes the minimum score at its mean, or the nearer
  domain endpoint, and the maximum of its two endpoint scores. The original Gaussian distribution and
  observed scores are unchanged; no predictive distribution is truncated or renormalized. Missing outcomes
  remain in the whole-cohort denominator, including mixed Gaussian/empirical cohorts. Out-of-domain
  observations and unrepresentable score arithmetic refuse without advancing authority time or sealed cuts.
  This opt-in selects `COH1`/v4 and `ASM1`/v4, including an explicit optional-monitor presence marker.
  Omission adds no bytes and preserves every old v1/v2/v3 identity, even for a legacy finite-domain Gaussian
  cohort with unresolved outcomes. These are floating-point completion bounds, not confidence intervals.
  Independent normal known answers, dense domain grids, mixed populations, failed admissions and sealed
  assessments are covered by the `scoring::tests::gaussian_completion_*` and `ledger::tests::gaussian_domain_*`
  regressions; the existing Gaussian score kernels and golden digests are unchanged.
- DESIGN (BD-048, plan §10.2): `AssessmentPolicy.event_log_floor` optionally freezes a probability floor in
  `(0, 0.5)` for all-Bernoulli cohorts. `scoring::bernoulli_log_score` returns
  `-ln(max(P(observed outcome), floor))` in nats without changing the forecast. The explicit cap removes
  strict propriety in the floored tails; it is reported beside the unchanged proper Brier score. The floor
  is not selected retrospectively, and a score never establishes event calibration. Invalid floors,
  continuous members and simultaneous continuous-coverage monitoring refuse before the freeze mutates time.
  `AssessmentSummary.event_log` retains the floor, observed mean and whole-frozen-cohort completion bounds.
  Each missing member contributes the minimum/maximum of its losses at zero and one, keeping every missing
  outcome in the denominator. Zero/one probabilities have finite capped losses; tiny non-event losses remain
  positive despite cancellation in `1-p`. First-publication and restated assessments stay separate.
  This opt-in selects `COH1`/v3 and `ASM1`/v3; absent log scoring preserves all v1/v2 bytes and identities.
  Independent logarithms, exhaustive binary completions, floor/digest mutations and unchanged legacy goldens
  are covered by `scoring::tests::event_log_scores_match_independent_logs_and_retain_the_declared_cap` and
  the `ledger::tests::event_log_*` regressions. The arithmetic remains `DeterministicPerIsa`.
- DESIGN (BD-051, plan §10.4): `AssessmentPolicy.coverage_monitor` optionally freezes a
  `CoverageMonitorPolicy`: alarm alpha, nonempty support declaration, nonempty conditional-null explanation,
  and an explicit observation order containing every member exactly once. The null is
  `P(hit_t | all preceding observations) = 1 - policy.alpha`. The declaration is an assumption; this kernel
  does not establish it from marginal coverage, correlation sensitivity or effective sample size. Both excess
  hits and excess misses can reject this point null; it is not a test of coverage being at least the target.
  The supported probability-bearing inputs are Gaussian forecasts, intervals whose explicit nominal level
  exactly equals `1-alpha`, and exact central quantile levels `alpha/2` and `1-alpha/2`. Nearby descriptive
  levels, absent nominal levels, Bernoulli events and empirical/discrete forecasts refuse: a discrete
  distribution's inclusive interval need not carry the requested probability. No randomization is invented.
- DESIGN: `Assessment.coverage_monitor` retains a `CoverageMonitorAssessment` with the frozen declaration,
  null probability, consumed `CoverageObservation` entries, first unresolved `blocked_by` member, counts of
  all deferred members and their already-resolved originals, and the numerical `CoverageSnapshot`. Each
  observation retains its original `MemberAssessment` (value, citation, sequence and event digest), exact
  frozen interval and inclusive hit indicator. Monitoring always selects `FirstPublished` originals at the
  same bitemporal cut, even when ordinary scores use `LatestAsOf`. It consumes only the contiguous resolved
  prefix of the frozen order. Missing members are never skipped, reordered, imputed or treated as misses.
  The first blocker retains its missing-event identity or inferred Pending/Overdue status.
  `BernoulliCoverageProcess` supplies the uniform Beta(1,1) log-mixture, running maximum and persistent first
  crossing. A restatement cannot replace a consumed original or erase a prior alarm; later observations can
  lower current evidence without lowering its running maximum. The alarm is diagnostic and does not mutate
  the forecast or retrain a model. A persistent alarm downgrades an otherwise positive `Calibrated` summary
  to `Descriptive`; it preserves existing `Overconfident` and `Biased` findings and cannot establish either
  classification by itself. An aggregate hit-rate recovery cannot erase the earlier diagnostic rejection.
- DESIGN: opt-in monitoring selects `COH1`/v2 and `ASM1`/v2 frames. Cohort hashes bind the complete declaration
  and original sequence; assessment hashes additionally bind every consumed original, interval and hit,
  blocker, deferred counts, and every snapshot field. An absent monitor adds no bytes or presence marker,
  preserving all existing v1 cohort and assessment identities exactly. Monitoring admission and replay are
  pure, with failed freezes/assessments leaving authority times, sealed cuts and caches unchanged. Arithmetic
  uses finite log evidence and is `DeterministicPerIsa`; exponentiated displays may be absent when they cannot
  represent a finite positive value. The numerical kernel caps monitoring at 1,000,000 observations.
- DESIGN (BD-051): `AssessmentPolicy.cusum_monitor` optionally freezes a two-sided Gaussian residual
  change diagnostic. `CusumMonitorPolicy` requires positive finite `reference_value` k and `threshold` h,
  explicit support, reference assumptions, tuning basis and a complete prospective member order.
  Each first-published residual is standardized with that member's frozen mean and standard deviation.
  Page's recurrences are `upper = max(0, upper + z - k)` and `lower = max(0, lower - z - k)`;
  reaching h records the first directional crossing. Later recovery cannot erase directional maxima or
  first crossings; no automatic reset, rearming or retraining occurs. Only Gaussian members are admitted.
  Coverage monitoring and finite-domain CRPS may coexist; event-log policies require a different cohort.
- DESIGN: `Assessment.cusum_monitor` retains original source/event evidence, the per-member reference
  mean/scale, residual and snapshot, the first unresolved blocker and deferred counts. It consumes only
  the contiguous first-published prefix at the same bitemporal cut, even under latest-as-of scoring.
  An alarm can suppress an otherwise positive `Calibrated` label but cannot itself establish bias,
  overconfidence, a false-alarm probability, average run length or a calibration verdict. The declared
  tuning basis is not verification of KILL-PLUMB-6. The control-mine and investigation workflow remain open.
  CUSUM uses `COH1`/v6 and `ASM1`/v5; both bind the previous optional policy layout and all new evidence.
  Omitting CUSUM preserves every prior frame. Invalid freezes and failed numerical assessments leave
  authority times and caches unchanged. Direct controls in `ledger::cusum::tests` cover standardization,
  ordered missingness, restatements, aggregate recovery, metadata/coverage/CRPS composition, identity and
  transactional numerical refusal. Arithmetic is `DeterministicPerIsa`, not an exact certificate.
- DESIGN (BD-051, GATE-PLUMB-E5): `DualSurpriseMonitor`, `DualSurpriseConfig`, `StandardizedResidual`, and
  `DualSurpriseSnapshot` pair the location and
  centred-dispersion `ScoreBettingEProcess`es (`eproc_alpha / 2` each, `BitwisePortable`) with a standardized
  two-sided Page `TwoSidedCusum` (`cusum_k`, `cusum_h`) and a causal Layer-3 predictive scale multiplier
  (`l3_scale()`, `1.0` before alarm and `alarm_scale >= 1.0` after). Out-of-domain parameters and non-finite
  residuals refuse without mutating monitor state; sticky first crossings are never erased by later quiet
  observations. Control-mine `GATE-PLUMB-E5` verification lives in `crates/fmine-gen/tests/gate_plumb_e5.rs`
  and `docs/delivery/GATE-PLUMB-E5.md`.
- `gaussian_crps`, `gaussian_log_score`, `winkler_interval_score` → `Result<f64, ScoreError>` (non-finite inputs,
  non-positive std_dev, alpha outside (0, 1), inverted intervals and unrepresentable results refuse; nothing
  is clamped, BD-048/bd-8wrk). Opposite-sign Gaussian endpoints are scaled before an overflowing subtraction;
  CRPS retains representable absolute-error limits at tiny forecast scales. Winkler penalties divide the miss
  before multiplying when the reciprocal level overflows. Ordinary finite paths retain their pinned digests.
- DESIGN (BD-047/BD-048): `ForecastPayload::Empirical { samples }` freezes a uniform univariate predictive
  distribution with 1–4,096 finite samples. Repeated values retain their probability mass; supplied sample
  order and exact bits participate in the forecast and cohort identities. The new payload tag is 4; existing
  tags, frames and golden identities remain unchanged. Validation precedes canonical-frame allocation.
  The journal adapter independently enforces its 64 KiB operation-byte limit, so a count-valid long textual
  request can still exceed that limit.
- DESIGN: `scoring::{MAX_EMPIRICAL_SAMPLES, empirical_crps, empirical_interval}` supplies the shared empirical
  arithmetic. A sorted scratch copy gives order-invariant scores and central intervals. CRPS integrates
  positive squared-ECDF gaps in O(n log n) time and O(n) scratch space, with compensated summation; it does not
  subtract large absolute-deviation expectations or fit a Gaussian. Unrepresentable positive contributions,
  sums or results return `ScoreError::Arithmetic`; no score is clamped or replaced with zero.
  Quantiles select the first sorted value with computed f64 CDF fraction `k/n >= p`, using binary search,
  including equality at a jump. Central levels are `alpha/2` and `1-alpha/2`; rounding to zero or one selects
  the minimum or maximum. There is no interpolation or implicit distribution smoothing.
  The assessment sorts each empirical forecast once for its interval, score and completion bounds. CRPS is
  reported for all-Gaussian, all-empirical and mixed Gaussian/empirical cohorts, retaining the complete frozen
  population. A cohort containing interval-only or quantile-only members does not report a partial CRPS mean.
  An unresolved empirical member contributes its CRPS minimum at a median, or at the minimizing endpoint
  of a disjoint declared outcome domain. A finite frozen domain also supplies the maximum of its endpoint
  scores; otherwise the upper bound remains absent. Legacy Gaussian unresolved bounds retain their behavior;
  a frozen `gaussian_crps_bounds` opt-in supplies their finite-domain extrema alongside empirical members.
  Standardized bias remains Gaussian-only. These are floating-point completion bounds, not confidence
  intervals, calibrated predictive coverage or shared-realization portfolio semantics. Empirical scoring
  is `DeterministicPerIsa`; this slice supplies no cross-host empirical-score digest.
- DESIGN (BD-047/BD-048, plan §10.1–§10.2): `ForecastPayload::WeightedEmpirical { samples, weights }`
  retains 1–4,096 finite univariate scenario values with corresponding `fmine_units::ExactQty` probabilities.
  Every weight must be strictly positive and their exact sum must be one. Mismatched lengths, invalid mass,
  nonfinite values and checked rational capacity failures refuse before registration changes authority state.
  There is no normalization, resampling, inferred equal weighting or omission. Duplicate values retain their
  probability mass. New payload tag 5 encodes the supplied order, exact value bits and reduced i128 weight
  numerators/denominators; all prior payload tags and forecast/cohort/assessment frames keep their bytes.
- DESIGN: `scoring::{weighted_empirical_crps, weighted_empirical_interval}` uses one scratch sort, exact
  cumulative and remaining probability masses, and compensated positive-gap CRPS integration. Remaining mass
  is subtracted exactly before floating projection, retaining a small tail even when its complementary CDF
  rounds to one. Losses are floating-point `DeterministicPerIsa` quantities, not rational certificates.
  Weighted central quantiles interpret the frozen `alpha` through its canonical shortest decimal spelling:
  `0.2` means exact 1/5, giving levels 1/10 and 9/10. The first sorted value whose exact CDF reaches a level
  wins, including equality. No interpolation or tolerance selects an adjacent jump. Rational capacity
  failures remain explicit. This rule applies only to the new weighted payload; legacy uniform empirical
  interval semantics and identities are unchanged.
- DESIGN: weighted, uniform empirical and Gaussian forecasts can share a complete CRPS assessment.
  Every unresolved weighted member contributes its minimum at a weighted median or the nearest frozen-domain
  endpoint; a finite outcome domain also supplies a maximum at an endpoint. No missing member is dropped,
  and any interval-only/quantile-only member still prevents a partial CRPS mean. Coverage diagnostics expose
  central 50/80/95% intervals under the exact-weight rule. Discrete inclusive intervals need not carry exactly
  the nominal mass, so any assessment containing the new weighted payload remains `Descriptive` or `Unscored`
  and conditional coverage monitoring refuses it. Weighted CDF-rank and supplied-quantile diagnostics remain
  explicitly unavailable. This is a marginal distribution, not a shared-realization portfolio, probability
  calibration or independent-observation claim.
  Direct controls in `scoring::weighted::tests` compare 567 cases against the independent pairwise CRPS
  definition, exact jump boundaries, tiny tails, repeated values, permutations, affine changes, domain extrema
  and arithmetic/domain refusals. `ledger::tests::weighted_*` checks complete mixed-population denominators,
  exact identities, unchanged historical reports, descriptive status and transactional refusals.
- `golden_digest_gaussian_crps_10k`, `golden_digest_gaussian_log_score_10k` (bd-hp2j.3): BLAKE3 over the score bits
  on 10,000 `SCORE_DIGEST_KEY` triples (|z| up to 12).
- `golden_digest_calib_rest_10k` (bd-hp2j.3): the rest of `lib.rs`, so one class covers the file: Winkler scores,
  reference-class blends, a ledger's report, and 1,000 samples through `SpatialFrontPitMonitor`, whose bias flips
  to take the location stake to both of its limits and the pair to the alarm. A refusal hashes as `u64::MAX`.
- All three committed values were first observed on aarch64-macos. `lib.rs` declares `BitwisePortable`: all three
  agreed on ubuntu-latest (x86_64) and macos-15 (aarch64) in CI run 41237993223 at 84f32ac (bd-hp2j.3). bd-311
  changed the monitor's scores and re-pinned the calib-rest value on aarch64-macos (the probe's PIT bits are
  unchanged; its wealth and alarm bits are not). An x86_64-linux rch worker gave the same value; no CI run of it
  exists yet.
- `ForecastLedger::evaluate_model` → `Result<CalibrationReport, ScoreError>` delegates to the assessment engine
  and exposes `n_members`, `n_scored`, `n_unresolved` and whole-cohort coverage bounds. This compatibility snapshot
  is always Unscored or Descriptive, since it has no frozen membership. Existing Gaussian `register` ids are
  retained; new `register_payload` ids hash exact canonical bytes. `record_outcome` is an uncited internal
  compatibility path and cannot supply a frozen public cohort.
- `ProjectArchetype`, `ReferenceClassPrior`
- `SpatialFrontPitMonitor` (BD-311): `new` (nugget and sill variances, the range as a `fmine_units::Qty<Length>`,
  the neighbourhood size and alpha), `observe_sample` (PIT and alarm), and the pub `location_eproc` and
  `dispersion_eproc`. Private to the module: `predictive` (the registered model's conditional mean and standard
  deviation for a polygon, recording nothing), the two scores, and c0 (`scripts/oracle/front_pit_constants.py`,
  `test_front_pit_score_constants_known_answers`). Nothing outside the crate calls them yet, and L-6 refuses bare
  `f64` coordinates in a public fn. `test_front_pit_predictive_known_answers` checks the conditional against one-
  and two-neighbour hand algebra.
  `test_spatial_front_pit_detects_58pct_coverage_shift_and_keeps_ville_bound` is the acceptance run. Synthetic truth
  is drawn from the monitor's own predictive, so it is an exact draw from the registered Vecchia joint law, and the
  PIT equals Phi(z) at every polygon. On 20,000 null runs of 600 residuals the false-alarm rate is 0.0347
  (α = 0.05), with a one-sided 95% Clopper–Pearson upper bound of 0.0369; that bound's assertion was added after
  the first run. Over 200 runs each, the median first alarm is 21 polygons for the location shift that takes 80%
  coverage to 58%, and 40 for the dispersion shift (σ × 1.589). The plan's ceilings are 25 and 40, so the
  dispersion median sits on its ceiling. The planted negatives are the uncentred dispersion score (false-alarm
  rate 0.79) and the scores this monitor used before bd-311 (dispersion median 94). INFERENCE from simulation;
  HYPOTHESIS for any real pit. Truth from a full Gaussian process, rather than its Vecchia approximation, is not
  tested.
- `guidance_scores` (BD-341.S4, descriptive slice): exact `Range`, `Point`, `AtMost` and `AtLeast` guidance
  scores against cited printed outcomes and their declared rounding intervals. `score` reports the printed
  hit/miss side, whether rounding makes the hit ambiguous, and range-only midpoint, half-width and normalized
  bias `z = (y - midpoint) / half_width`. One-sided and point guidance have no `z`. `score_range` is a
  convenience adapter that maps equal endpoints to `Point`; the canonical `Range` remains strictly positive-width.
  `summarize` preserves every declared member, refuses duplicate ids and invalid ranges even when unresolved,
  and reduces in stable id order. It gives exact observed hit rates, whole-cohort completion bounds, separate
  rounding-aware bounds, mean `z` with its arithmetic rounding enclosure, and the share above the midpoint.
  `clopper_pearson_90` retains its floating-point 90% IID Bernoulli reference interval API, using a bounded,
  mode-centred binomial recurrence and the existing refusing root solver. Empty/invalid counts, more than
  4096 resolved outcomes and nonconvergence refuse. `CP_ASSUMPTION` states that neither reference calculation
  adjusts for common year/site shocks, selective missingness or outcome rounding. Numeric JSON renderers
  keep exact rational numerators/denominators and open interval ends; effective sample size and bias confidence
  intervals remain null. Outputs are always Descriptive or Unscored.
- DESIGN (BD-341.S5): `guidance_scores::price` checks whether the supplied cited quarterly royalty/price
  rounding intervals admit one common proportionality coefficient. It returns the exact feasible interval
  or two conflicting quarters. Only supported gold/copper AISC ranges with a stated price assumption and
  a passing gate receive the signed adjustment `royalty * (1 - assumed_price / realized_price)`.
  The scored period's exact royalty/price ratio must also intersect that coefficient interval; a contradictory
  period remains unsplit with its ratio and historical gate visible. Cost ownership is checked within the
  outcome/royalty family, and commodity-price basis within the price family; the two are never relabelled equal.
  Exact rounding enclosures, source identities, unit/basis checks and an explicit company-price proxy are
  retained. The immutable conditional report always includes the unconditional score and binds both to its
  inputs. It makes no causal claim and does not establish that the supplied history is complete.
- DESIGN: `guidance_scores::clopper_pearson_90_exact` returns an outward enclosure of the iid Bernoulli model's
  two-sided confidence interval. It bisects a dyadic probability grid with spacing `2^-40`, using outward
  integer evaluation of the exact binomial survival polynomial on a `2^-80` probability scale. Each endpoint
  is at most one grid step beyond the model's exact endpoint. Count, iteration and trial-resource limits
  refuse; a tail whose sign cannot be certified refuses. `clopper_pearson_90_exact_with_limit` exposes the iteration
  budget, and `MAX_CP_TRIALS` is 4,096. Zero trials return the vacuous exact interval. The private
  `guidance_scores::certified` evaluator uses no floating-point
  or transcendental operations. Summaries expose canonical `iid_reference_interval_90_exact` rationals and
  retain `iid_reference_interval_90` as an exactly representable dyadic float projection for compatibility.
  The iid assumption is explicit and is not asserted for correlated sites or years.
- `guidance_scores::claims` wraps those same primitives in canonical, cited Claims. Its `score` checks matching
  subject, metric, annual period, unit and ownership/definition basis, source usability, and both the hashed body
  and typed payload's canonical tag. Its `summarize` refuses duplicate subjects in one publication cohort and
  mixed metric/year/unit groups. All score and aggregate arithmetic delegates to `guidance_scores`; the wrapper
  adds provenance rather than a second formula. Derived Claims retain every source, including unresolved
  guidance, and never upgrade evidence or reduce confidentiality. Effective sample size and bias confidence
  interval remain absent. Summary Claims retain the certified rational `binomial_90_exact` bounds and their
  exact float projections; the `GDA1` version-2 value frame binds both representations. Publication selection
  and source-byte authentication belong to the public adapter.
- `guidance_scores::claims::summarize_history` binds a single site's multi-year population on one metric,
  unit and ownership/definition basis. Duplicate years and mixed subjects or bases refuse; unresolved years
  stay in the denominator. `GuidanceSummary.scope` distinguishes `Annual` from `SiteHistory`, including the
  latter's exact ordered years. Existing annual summaries retain their byte-identical `GDA1` version-2 value
  encoding; the new history scope uses version 3. Existing numeric score and summary structures can be
  projected from these Claims without repeating arithmetic or source selection.
- Explicit deterministic `Assumed` numerical inputs may remain `Unknown` during hypothetical arithmetic;
  the result retains that state and evidence. This exception does not admit unknown issuer assertions,
  placeholders, stale inputs, failed calibration or explicit negative knowledge states.
- `record` (BD-334.S1, the public forecast record of plan §21.6), all pure:
  - `record_frame`/`record_digest` (`FMREC1`: manifest and forecasts hashed as one) and `rules_frame`/`rules_digest`
    (`FMRUL1`: the rules alone, stamped first);
  - `parse_manifest` → `RecordManifest` over `fmine-registry`'s TOML subset (no booleans, so `label` is
    `forecast`/`nowcast` and `contact` is `pre-contact`/`post-contact`); unknown keys or tables, missing keys and
    mistyped values refuse (`RecordError`);
  - `validate(manifest, registered digests, Option<ProofTimes>, RecordDates)` → `Vec<Finding>`: a digest that
    differs from the source registry, an input after the cut-off, a reworded attestation, an author that is not a
    role id, or a cut-off at/after the frozen proof deadline; with a proof, a cut-off not before it, a label that
    disagrees with it, a proof after the deadline. Draft validation refuses an impossible proof window before
    hashing: the cut-off must leave time for `cutoff < lower <= upper <= deadline`.
    Times come only from a proof (`ProofTimes`), never from a clock; `FY2026_DATES` holds the Q3 release
    (2026-11-09T11:00Z) and the 31 Dec 2026 deadline;
  - `parse_rules` → `RecordRules` (BD-334.S2): α, the quantile estimator, the n_eff correlation range, the
    coverage level, the dates, the targets (each with its Mine Stats summary field id and its `Outcome`,
    `fy_minus_9m`, `q4_qtd`, `quarter_qtd` or `quarter_qtd_nowcast`), basis rules, baselines (each on a declared target, every target with one; the id
    decides the `BaselineKind`, and `run_rate_persistence` carries its pool's `Quarter`s and the point quarter
    after them) and the named site universe (`live`/`not_published`, never dropped); refuses an `OPEN` marker in
    any value, a duplicate id, an unknown baseline id and every malformed or missing key;
  - `TargetMetric::from_target` gives candidate and scoring adapters one explicit source interpretation:
    `au_prod_100_koz` and `cu_prod_100_kt` require `fy_minus_9m`; `aisc_usd_per_oz`, `tcc_usd_per_oz`,
    `c1_usd_per_lb` and `aisc_usd_per_lb` require `q4_qtd`. It supplies the exact unit, guidance quantity key
    and cost-definition suffix. All six fields also accept `quarter_qtd`, meaning the original
    next-quarter QTD value. Explicit `quarter_qtd_nowcast` uses the same units and original QTD outcome,
    but all targets must share that mode. Its `nowcast_from_utc` equals quarter end, and `proof_by_utc`
    must be later and strictly before mandatory `earliest_outcome_release_utc`. A nonempty
    `outcome_release_basis` must state the declared earliest release, including preliminary announcements.
    These exact rule bytes are frozen; the declaration is not independent evidence of nonpublication.
    Adapters additionally enforce authenticated record timing and registered original publication bounds.
    `RecordRules::outcome_quarter` requires one common persistence point and refuses mixed annual/quarterly
    targets; annual records retain their Q3-to-Q4 meaning. Rules parsing, CSV admission and both direct
    scoring APIs enforce that shared horizon whenever a target uses `quarter_qtd`; constructing or mutating
    typed rules cannot bypass it. Legacy annual numerical admission is unchanged.
    `Quarter::end_utc` supplies the exclusive calendar
    boundary, including year rollover, and rejects invalid or unrepresentable quarters. Shared persistence
    pool validation is reused by the candidate and public source adapter.
    Production is additive on the 100% basis; sold-metal cost rates are neither subtracted nor
    ownership-scaled. Unsupported field/outcome pairs have no interpretation. This adds no durable tags
    or changes to existing rules/record frames;
  - `parse_forecasts(csv, rules)` → `Vec<ForecastRow>` (BD-334.S3): the header `forecasts_header` derives from α
    (`site,target,point,p10,p90,uncalibrated` at 0.2), exactly one row per live site × target, none for a
    `not_published` site, exact decimals with lower ≤ point ≤ upper, and `uncalibrated` = `true`.
    Gold AISC/TCC and copper C1/AISC sold-metal cost targets admit signed forecasts, including credit-only and
    cross-zero ranges. The exact field and `q4_qtd` or `quarter_qtd` outcome rule authorize this domain, never the target ID
    or prose. Production, unknown fields and incompatible field/outcome pairs retain nonnegative bounds.
    Both direct scoring APIs enforce the same target domain; signed costs are not clamped or excluded.
  - `interval_score` on exact rationals (refuses α outside (0, 1) and l > u), `quantile` (Hyndman–Fan types 1
    and 7), `pooled_ratios` (consecutive-quarter ratios per site; a pair with a value that is not a positive
    number is left out and counted, never imputed), `ratio_quantiles` (the ratios at α/2 and 1 − α/2) and
    `persistence_interval` (point × those ratios; inverse ECDF only: exact, since exp of a quantile of log ratios
    is then an observed ratio; the linear estimator's bounds are irrational and refuse with `NotExact`)
- DESIGN (BD-076): `record::scoring::paired::assess(rules, forecasts, outcomes, baselines)` consumes the parsed frozen
  rules, exactly one forecast for each live site × target, an explicit `OutcomeRow` for every declared site ×
  target, and a `BaselineRow` for every site × declared baseline. `Availability::Unavailable(reason)` retains
  missing outcomes and baselines; omission, duplicates and additions outside the universe refuse.
  Both canonical comparator kinds, `issuer_guidance` and `run_rate_persistence`, must be declared per target.
  `RecordAssessment` retains every declared member in rules order, including sites marked `NotPublished`
  before forecasting. Coverage and its hypothetical completion bounds use the frozen **live** population;
  pre-excluded sites have no invented forecast and are counted separately by name.
  Each target reports exact interval scores, method-specific availability/scored counts and observed means,
  coverage, an equicorrelation n_eff sensitivity over the frozen rho range, and a paired count/mean for the
  common population where the forecast, outcome and every declared baseline are available. Membership,
  paired means and forecast-minus-baseline differences reuse `record::scoring::compare`'s common calculation;
  both full-live-cohort loss-difference completion objects are retained in the typed target summary and JSON,
  including pending/missing names and local arithmetic refusal, independently of the common-set means.
  negative differences favor the forecast loss without a skill claim. Calibration remains `Descriptive` or,
  without resolved live outcomes, `Unscored`,
  even if the record's sample threshold is exceeded.
  `RecordAssessment::to_json()` emits `fmine.forecast-record.scores.v1`, preserving rational numerator and
  denominator strings. It never verifies sources, chooses an outcome vintage, transforms an ownership basis
  or authenticates the record's time proof; those are adapter responsibilities.
- DESIGN (BD-076): `guidance_scores::clopper_pearson_exact(k, n, coverage_level)` and its `_with_limit` form
  generalize the certified dyadic enclosure to the frozen confidence level, including FY2026's 95%. Existing
  CP90 entry points retain their endpoints. The interval applies to resolved members under the explicit iid
  reference assumption, without adjustment for missingness, site dependence or rounding. Completion bounds
  and n_eff sensitivity remain separate from this reference interval.

## 5. Refusals
`ForecastLedger` refusals, by condition (`test_ledger_refusals_by_condition`):
- `FMINE-REF-206` (late registration, exit 3): exact payload registration at or after the outcome time; cohort
  freeze before registration, at or after the outcome time, or after any recorded outcome; an early or backdated
  outcome; an assessment before its cohort. Compatibility `register` retains equality at the freeze boundary;
  such a forecast cannot enter a cohort, which needs a strictly later freeze before the outcome.
  A registration, cohort freeze or outcome behind the shared record watermark or at/before a sealed assessment
  cut also refuses. A refused mutation changes neither watermark nor stored records.
  A public-bound forecast deadline after its target calendar period ends also refuses, so an arbitrary later
  deadline cannot register a forecast for a target that has already ended.
- `FMINE-REF-507` (frozen record mutation, exit 6): a second registration of one forecast id, a reused cohort
  name, a second one-shot outcome, an invalid restatement predecessor or reused source vintage, or reinterpretation
  of a withdrawn, changed-definition or divested target as its old quantity without a new resolution rule.
  A second public cohort cannot change a shared forecast's semantic target.
- `FMINE-REF-109` (outside physical domain, exit 3): a non-finite mean or outcome, `std_dev <= 0`, or a policy
  attempting to lower the positive-calibration minimum below 35 independent outcomes.
- `FMINE-REF-201` (no claim in domain, exit 3): an unknown forecast/cohort, or an ineligible outcome source.
- Paired record scoring uses `FMINE-REF-107` for malformed rules or population rows, `FMINE-REF-109` for invalid
  intervals/probability parameters, and `FMINE-REF-207` for exact arithmetic overflow or an uncertified CP endpoint.
  Signed FY-minus-9M residuals, negative per-unit costs and negative Q4-implied issuer guidance ranges are not
  clamped or excluded after observation. The adapter validates underlying physical totals and flags a negative
  literal residual as a discrepancy; the frozen subtraction is not a claim about physical Q4 production.

206 and 507 are proposed codes (`docs/adr/ADR-REFUSALS-01.md`).

## 6. Determinism class
Every module declares its `//! Determinism:` header checked by `xtask check-det`: `lib.rs` and `ledger/binding.rs` are `BitwisePortable`; exact rational scoring/CRPS modules are `Exact`; floating-point scoring/prior/monitor paths are `DeterministicPerIsa`.

## 7. Evidence class of outputs
Calibration scoring never promotes evidence (`NL-1`, `NL-2`): `apply_calibration_gate` only lowers `ModelledCalibrated` and sets `calibration_failed` on `Overconfident`/`Biased` (irreversibly per `NL-4`). Descriptive guidance scores and completion bounds carry no predictive-skill or calibration verdict.

## 8. Validity domain
A miscalibrated spatial front alarms when either score's wealth reaches `2 / alpha` (40 at `alpha = 0.05`). The
source-bound `spatial` worksheet adapter exposes that alarm and its diagnostic Claims; it does not downgrade
other Claims or create an InvestigationCase (plan §10.5). Pure calibration and scoring kernel; does not perform file I/O (`L-3`). Depends on `fmine-core`, `fmine-time`, `fmine-evidence`, `fmine-claim`, `fmine-numerics`, `fmine-registry` (S0), and `fmine-units` (S0).

## 9. Oracles
- `scripts/oracle/score_known_answers.py`: numerical integration oracle for `gaussian_crps` and `gaussian_log_score` to `1e-12` relative.
- Independent 90-digit `Decimal` binomial sum oracle (`test_frozen_95_percent_level_against_independent_binomial_sum`), quadratic CRPS oracle over 81 four-sample models, and exhaustive binary completions through 12 members.
- `GATE-HELM-01` (tracking target; real retrospective cards remain open).

## 10. Known gaps
- Spatial front alarm does not yet emit an `InvestigationCase` (plan §10.5) or downgrade downstream Claims.
- Clustered bias interval and its 200-replicate coverage gate (`BD-341.S4`) remain open; `GATE-HELM-01` remains open.
- Cohort store is in memory; durable storage and external timestamp authority live in `fmine-io`/`fmine-ledger`.

### Forecast-monitor investigation lifecycle

DESIGN (BD-051, §10.5): `ledger::investigation::InvestigationLedger` consumes a retained assessment and
derives the first original-publication crossing of its explicitly selected coverage or CUSUM monitor.
The opening binds the assessment, source cohort, original forecast/event/citation, owner role, future
deadline, at least two distinct labelled competing hypotheses and an ordered list of proposed probes.
Probe ranking is supplied; this slice computes no VOI. A later assessment or restatement cannot reopen
the same original alarm under another case id. Rejected transitions do not reserve identities or time.

An explicit human-role review records one of `hypothesis_added`, `recalibrated`, `data_defect` or
`false_alarm_logged`, with separate rationale and evidence declarations. The role and evidence are
unverified inputs, not authenticated identity, causality or proof that a correction was applied.
Resolution preserves the opening and alarm. Optional rearming links a different cohort frozen after
resolution, with the same monitor kind and no outcomes recorded or due at rearm. The source monitor
is never reset. The authority adapter retains all events and checks any stronger physical support start.
The pure kernel supplies bounded deterministic replay state and canonical opening/response digests;
durability and persistence of the selected source assessment remain the adapter's responsibility.

Synthetic controls in `ledger::investigation::tests` independently identify the second-observation CUSUM
crossing and third-miss coverage crossing, preserve original evidence through restatements, reject
duplicate triggers and malformed transitions atomically, exercise all four explicit resolution kinds,
and enforce prospective rearming without changing frozen forecasts, assessments or alarm histories.
Spatial-front assessments and automatic downstream Claim invalidation remain outside this slice.

### Scope boundary and cluster projection notes
Pure calibration and scoring kernel; does not perform file I/O (`L-3`).

DESIGN (BD-341.S4): `guidance_scores::clusters` is a separate opt-in projection. It preserves exact
year/site membership, observed range means, rounding bounds and missing-outcome hit bounds. The caller
explicitly selects the assumed independent axis. The 1,999 keyed Philox block replicates estimate the
equally weighted mean of cluster means, distinct from the legacy observation-weighted mean. Inverse-ECDF
5th/95th percentiles have a nominal, approximate 90% interpretation conditional on independent,
exchangeable selected clusters; no two-way robustness or dependence estimate is implied. Twenty clusters
is a declared admission floor, not a coverage theorem. Incomplete eligible populations, clusters without
range targets, sparse populations and zero between-cluster variation have no sampling interval. Both
partitions and every unresolved member remain available. Exact integer-coordinate overflow refuses.
Direct controls live in `guidance_scores::clusters::tests`, including exact equal-cluster weighting,
whole-block replication invariance, unit invariance, rounding separation and a 200-panel-per-scenario
synthetic comparison of correct and deliberately misspecified axes. The broad BD-341.S4 gate remains open.

The cohort store is in memory. Its canonical assessment bytes can be retained by an adapter; this slice adds no
durable ledger or external timestamp authority. Caller-supplied times are ordering inputs, not time proofs

## 11. Public API
- `FrozenForecast`, `ForecastPayload`, `ForecastLedger`, `apply_calibration_gate`
- `gaussian_crps`, `gaussian_log_score`, `winkler_interval_score`, `ReferenceClassPrior`, `SpatialFrontPitMonitor`
- `record::{candidate::build_candidate, scoring::assess, TargetMetric}`
- `guidance_scores::{clopper_pearson_exact, claims, clusters, price}`
- `ledger::{binding, comparison::crps, correction}`.

## 12. Proof obligations
- `PO-CAL-1`: Expected Gaussian CRPS is uniquely minimized at the true data-generating $(\mu_0, \sigma_0)$.
- `PO-CAL-2`: Retroactive forecast registration (`t_registered >= t_outcome_avail`) is refused with `FMINE-REF-206`.
- BD-048: `gaussian_crps` and `gaussian_log_score` match eight known answers to 1e-12 relative. The CRPS values are
  integrated from the definition (`scripts/oracle/score_known_answers.py`), not taken from the closed form.
- DESIGN (BD-048): empirical scoring controls compare positive-gap integration with an independently expressed
  quadratic absolute-deviation oracle over 81 four-sample models and five outcomes each. Additional controls
  cover degenerate samples, exact CDF jumps, permutation and uniform-replication invariance, large offsets,
  representable wide gaps, arithmetic refusal and declared-domain extrema. Ledger controls retain unresolved
  members in mixed Gaussian/empirical CRPS, bind supplied draw order independently of scoring order, and prove
  refused empirical admissions and assessments do not seal authority time.
- BD-049: `apply_calibration_gate` per flag (only Overconfident and Biased mark a failure; every flag but Calibrated
  drops `ModelledCalibrated`), and over a run of flags it only lowers: a later Calibrated flag neither clears a
  failure nor gives the class back. The class is in the claim's id, so the lowered claim comes from
  `Claim::downgraded`: a new id, `supersedes` naming the old one (§8.1, BD-040); `calibration_failed` is set in place.
- BD-343: exhaustive binary completions through 12 members verify coverage and Brier bounds; the E474 test uses
  independently captured source cells and registered hashes beside a synthetic forecast; delayed record times,
  restatement replay, source eligibility, frozen membership, interval bounds and minimum-effective-n gates have
  focused tests in `ledger::tests`.
- Shared-chronology regressions prove that omitting an already observed miss cannot enable a backdated cohort;
  registration and outcomes outside an assessed cohort respect its closed cut; historical queries cannot reopen
  that cut; same-time batches remain valid; failed writes do not advance time or reserve a cohort name; and
  calibration policies cannot bypass the minimum independent sample. Existing frame and score goldens are retained.
- BD-341.S4's descriptive slice has known-answer `z = 3/2`, exact oz-to-gram invariance, open/closed rounding
  boundary, missing-member completion-bound, stable input-order, Clopper–Pearson closed-form edge, malformed
  input and arithmetic-overflow tests in `guidance_scores::tests`. Tests compare floating binomial bounds with
  the independent log-space tail implementation, verify maximum supported populations and one-sided boundaries,
  and force nonconvergence. The certified reference interval is checked
  against independent high-precision combinatorial-sum answers, exact small binomial polynomials, monotonicity
  and reflection symmetry in `guidance_scores::certified::tests`. Exhausted root iterations and oversized
  populations refuse. These tests do not establish the unimplemented clustered bias interval or its
  200-replicate coverage gate.
- The Claim wrapper's tests in `guidance_scores::claims::tests` check canonical tampering, basis and duplicate
  member refusals, evidence retention, source parents for unresolved members, stable Claim identities, and
  faithful conversion of exact scores and rounding/completion bounds through the numerical API. Certified
  binomial bounds survive the wrapper with their source parents and cannot be changed or removed without
  changing its canonical value tag; unresolved-only cohorts retain absent reference intervals.

- FACT (BD-341.S5, `guidance_scores::price::tests`): independent exact examples exercise a price contribution
  of twenty-five, unchanged prices, supported and refuted proportionality, open interval endpoints, missing
  assumptions, unsupported metrics, explicit proxies with distinct commodity-price bases, incompatible current
  periods, altered paired reports, scaling and arithmetic overflow.
- `record::scoring::paired::tests` supplies independent interval-score answers, common-population means, live-cohort
  completion bounds and n_eff fractions, and exercises retained unavailable/excluded members, input-order
  invariance, strict population rejection, signed original-release residuals, net cost credits and arithmetic refusal.
  `guidance_scores::certified::tests::test_frozen_95_percent_level_against_independent_binomial_sum` compares
  95% endpoints with an independent 90-digit Decimal binomial summation. A separate regression pins the
  previous CP90 grid endpoints and a dyadic exact-root case.
- `public_target_semantics_keep_metal_denominator_and_additivity_distinct` checks all six public target
  interpretations and rejects reversed outcome kinds and an attributable-production alias.
(BD-047/BD-068). It does not infer a fallback after a target's definition changes, estimate correlation, or claim
calibration for an issuer guidance range with no stated level. Guidance's IID Clopper–Pearson reference interval
does not account for common shocks. The opt-in block projection above supplies conditional one-axis
intervals; simultaneous site/year inference, an empirically justified effective sample-size rule and the
full BD-341 coverage qualification remain open. The exact guidance scorer consumes
already selected, citation-bound outcomes: it does not freeze forecasts, authenticate a publication date or
select a vintage. Its rational payloads and one-sided guidance do not extend the ledger's durable floating-point
forecast format. Adapters must retain the frozen membership and selector separately; completion bounds do not
substitute for statistical confidence intervals. Mean-z bounds propagate source rounding only.
The frozen-record scorer consumes already transformed, selected inputs. Its completion bounds do not make an
unavailable baseline into a published prediction; its confidence references do not establish forecasting skill.
The actual FY2026 record and outcomes remain subject to the Q3 source gate, external time proof and later release.

### Quarterly capital target semantics

DESIGN (BD-059/067/076): `record::TargetMetric` adds distinct total, minesite-sustaining
and project capex meanings for `capex_usd_m`, `capex_sustaining_usd_m` and
`capex_projects_usd_m`. All use `usd_m` and require `Outcome::QuarterQtd`, including
the existing explicit nowcast normalization. `Q4Qtd` and `FyMinus9m` do not admit capital targets.
The metric exposes its exact canonical source line and issuer-reported guidance suffix.
Capital points may be signed or zero; positive historical adjacency and exact pooled-ratio
arithmetic remain unchanged. This adds target meanings without modifying frozen rule bytes.

### Declared spatial-front assessments (BD-311)

DESIGN: `spatial::evaluate(text, source)` checks the supplied worksheet's SHA-256 before strict subset parsing.
It admits at most four MiB and 4,096 observations in strictly increasing declared availability order.
`registered_ns`, `prediction_registered_ns` and `available_ns` are nonnegative UTC Unix nanoseconds.
The policy precedes each prior-mean registration, which must precede its observation's availability. Every row
repeats and matches the policy's target unit, metre coordinate frame and support; conversion is never inferred.
The existing monitor's finite-parameter, neighbourhood and covariance checks remain in force. Unrepresentable
conditional quantities, squared distances, residuals or log wealth refuse the whole assessment.

The immutable receipt retains the policy and every observation as Assumed/Internal input Claims, each
diagnostic row as a derived Claim with its predecessor and input parents, and a bound summary. It reports
conditional means and standard deviations, standardized residuals, PIT values, both current and peak log
wealths, threshold, separate sticky monitor alarms and the first crossing. The original source identity,
policy, complete declared order, all row results and interpretive limits contribute to the receipt digest.
JSON and Markdown retain the complete population, input quantities and declared chronology. Markdown
separates observation values, conditional diagnostics and both monitor histories, and exposes each input,
result and summary Claim's identity, value tag, parents and bound worksheet locators.
A supplied locator remains a declaration attached to worksheet bytes, not authenticated field evidence.
Nanosecond timestamps render as exact decimal strings in JSON. The command does not freeze a policy in a
durable journal, prove prospective timing, infer calibration or causality, retrain, invalidate another Claim,
or implement the human-resolved investigation lifecycle. The existing monitor kernel and its goldens are unchanged.

DESIGN: direct tests in `spatial::tests` cover independent two-observation Gaussian conditioning answers,
source and semantic changes, inherited evidence, chronology and dimensional mismatches, numerical overflow,
population limits, sticky crossings through a separately evaluated prefix, deterministic JSON/Markdown,
and complete Markdown inputs, diagnostics and source/Claim bindings.
These are synthetic controls; no authentic-site or false-alarm qualification is asserted by this adapter.
