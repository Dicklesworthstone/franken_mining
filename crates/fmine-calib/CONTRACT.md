# CONTRACT.md — `fmine-calib`

## 1. Charter & Layer
- **Crate:** `fmine-calib`
- **Layer:** `K0` (Kernel Foundation)
- **Subsystem:** `Kernels`
- **LOC Budget:** `18k planned / 21k ceiling` (`registries/loc_budget.toml`)
- **Allowed Module Families:** `["ledger", "scoring", "prior", "pit", "record", "guidance_scores"]`

## 2. Mathematical & Domain Formulation

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
Implements the append-only pre-registration `ForecastLedger` (`INV-008`, `FMINE-REF-206`), strictly proper scoring rules (`gaussian_crps`, `gaussian_log_score`, `winkler_interval_score`), `ReferenceClassPrior` (`GB-32`: capex `+28%/+43%`, schedule `+31%`, ramp-up `74%` Yr-1), and `SpatialFrontPitMonitor` (`GB-08` Vecchia/Rosenblatt conditional PIT; plan §10.4's location score `tanh(z/2)` and centred dispersion score `tanh((z^2 - 1)/2) - c0`, each on a `ScoreBettingEProcess` with aGRAPA stakes at alpha/2).

## 3. Authority, Epistemics & Representation
Enforces pre-registration of forecasts prior to outcome observation (`INV-008`) and monitors spatial front miscalibration (`G-CAL-1`).

## 4. Public API Surface
- `FrozenForecast`, `ForecastPayload`, `ForecastLedger`
- DESIGN (BD-067/BD-334): `record::candidate::build_candidate` generates a complete draft live-site × target
  record from already selected quarterly observations. The named `pooled-quarterly-ratio-empirical-v1` model
  uses each target's frozen persistence pool and inverse empirical CDF: the latest positive observation times
  the pooled lower, median and upper ratios. Its exact interval equals that comparator's interval on identical
  inputs, so it has the same interval score before export rounding; the changed point estimate is no evidence
  of independent forecasting skill. Historical missing or nonpositive observations omit only their adjacent
  pairs, with both endpoints retained in an exclusion audit. Missing live members or point observations,
  duplicate/foreign members, malformed pool dates, empty ratio pools and exact overflow refuse the entire
  candidate. Named not-published sites remain explicit. Output is deterministic in frozen rules order.
  The exact rational rows and actual serialized decimal rows are separate: at a declared 0–12 decimal places,
  lower bounds round down, upper bounds round up and points round to nearest with ties upward. Every CSV row
  says `uncalibrated=true` and passes the existing complete-record parser. Export rounding can change scores;
  it never silently replaces the exact calculation. Limits are 4,096 live members and 65,536 history values.
  The pure kernel authenticates no source, availability time, author attestation or external time proof and
  does not create an issued or registered forecast. Its adapter must bind every history and point value to
  the original-release cell, source digest, unit, ownership basis and information cut before publication.
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
- DESIGN (BD-048/049, plan §10.2): `assessment_diagnostics(saved_digest)` derives Gaussian PIT values and
  fixed-decile histograms, Bernoulli reliability bins, and exact empirical CDF rank intervals from the
  selected outcomes in an immutable saved assessment. Every member and event/source identity is retained.
  Missing Gaussian outcomes widen each bin's whole-population completion bounds; missing event outcomes
  stay in their original probability bins. Full-population and resolved-only mean probabilities are separate,
  and empty-bin averages remain absent. The normal CDF retains its existing approximate tail saturation.
  Empirical forecasts yield exact counts below, equal to and above the selected outcome, with closed bounds
  `[F(y-), F(y)] = [below/n, (below+equal)/n]`. One linear scan uses comparisons without sorting or subtraction;
  signed zeros are equal and ties remain intervals. Separate empirical decile bins count resolved intervals
  wholly contained (definite) or intersecting (possible). Edges are exact integer fractions `i/10`, left-inclusive
  and right-exclusive except at 1; exact edge points go right. Whole-population bounds use definite counts as
  lower numerators and possible plus unresolved counts as upper numerators over every empirical member.
  No randomized PIT, continuous interpolation, uniformity claim or pooling with Gaussian PIT is introduced.
  Sparse quantiles and standalone intervals remain unsupported for this diagnostic projection. DGN1/v2 binds
  empirical counts, bounds and rules when empirical members exist; all other reports retain DGN1/v1 bytes.
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
  Forecast registrations, cohort freezes and outcomes share a monotone record-time watermark: an already
  observed miss cannot be excluded by backdating a new cohort containing only other, unresolved members.
- DESIGN: `ledger::binding::PublicResolutionPolicy` freezes registered-public resolution semantics in the
  existing `CohortSpec.resolution_rule` string. Its reserved `fmine.public-resolution.v1:` prefix contains a
  canonical `PRP1`/v1 frame encoded as lowercase hexadecimal. It binds issuer, metric, explicit unit and source
  field, calendar FY/quarter, original release, exact ownership fraction and every forecast-id-to-subject mapping.
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
    role id; with a proof, a cut-off not before it, a label that disagrees with it, a proof after the deadline.
    Times come only from a proof (`ProofTimes`), never from a clock; `FY2026_DATES` holds the Q3 release
    (2026-11-09T11:00Z) and the 31 Dec 2026 deadline;
  - `parse_rules` → `RecordRules` (BD-334.S2): α, the quantile estimator, the n_eff correlation range, the
    coverage level, the dates, the targets (each with its Mine Stats summary field id and its `Outcome`,
    `fy_minus_9m` or `q4_qtd`), basis rules, baselines (each on a declared target, every target with one; the id
    decides the `BaselineKind`, and `run_rate_persistence` carries its pool's `Quarter`s and the point quarter
    after them) and the named site universe (`live`/`not_published`, never dropped); refuses an `OPEN` marker in
    any value, a duplicate id, an unknown baseline id and every malformed or missing key;
  - `parse_forecasts(csv, rules)` → `Vec<ForecastRow>` (BD-334.S3): the header `forecasts_header` derives from α
    (`site,target,point,p10,p90,uncalibrated` at 0.2), exactly one row per live site × target, none for a
    `not_published` site, exact decimals with 0 ≤ lower ≤ point ≤ upper, and `uncalibrated` = `true`;
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

## 5. Guards & Typed Refusals
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

## 6. Proof Obligations (`PO-*`)
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

## 7. Failure Modes & Deterministic Fallbacks
A miscalibrated spatial front alarms when either score's wealth reaches `2 / alpha` (40 at `alpha = 0.05`). Nothing
downstream reads the alarm yet: no claim is downgraded by it, and no InvestigationCase (plan §10.5) exists.

## 8. Gates & Kill Criteria (`GATE-*` / `KILL-*`)
- `GATE-HELM-01`

## 9. Lifted Code & Foundation Dependencies
- Depends on `fmine-core`, `fmine-time`, `fmine-evidence`, `fmine-claim`, `fmine-numerics`, `fmine-registry` (S0)
  for the TOML subset the record's manifest is written in, and `fmine-units` (S0) for the monitor's range.

## 10. Non-Goals & Scope Boundary
Pure calibration and scoring kernel; does not perform file I/O (`L-3`).

The cohort store is in memory. Its canonical assessment bytes can be retained by an adapter; this slice adds no
durable ledger or external timestamp authority. Caller-supplied times are ordering inputs, not time proofs
(BD-047/BD-068). It does not infer a fallback after a target's definition changes, estimate correlation, or claim
calibration for an issuer guidance range with no stated level. Guidance's IID Clopper–Pearson reference interval
does not account for common shocks. Clustered bias confidence intervals, an empirically justified effective
sample-size rule and BD-341's three-scenario coverage study remain open. The exact guidance scorer consumes
already selected, citation-bound outcomes: it does not freeze forecasts, authenticate a publication date or
select a vintage. Its rational payloads and one-sided guidance do not extend the ledger's durable floating-point
forecast format. Adapters must retain the frozen membership and selector separately; completion bounds do not
substitute for statistical confidence intervals. Mean-z bounds propagate source rounding only.
The frozen-record scorer consumes already transformed, selected inputs. Its completion bounds do not make an
unavailable baseline into a published prediction; its confidence references do not establish forecasting skill.
The actual FY2026 record and outcomes remain subject to the Q3 source gate, external time proof and later release.
