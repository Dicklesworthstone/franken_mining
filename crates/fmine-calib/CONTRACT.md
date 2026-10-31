# CONTRACT.md — `fmine-calib`

## 1. Charter & Layer
- **Crate:** `fmine-calib`
- **Layer:** `K0` (Kernel Foundation)
- **Subsystem:** `Kernels`
- **LOC Budget:** `14k planned / 16k ceiling` (`registries/loc_budget.toml`)
- **Allowed Module Families:** `["ledger", "scoring", "prior", "pit", "record"]`

## 2. Mathematical & Domain Formulation
Implements the append-only pre-registration `ForecastLedger` (`INV-008`, `FMINE-REF-206`), strictly proper scoring rules (`gaussian_crps`, `gaussian_log_score`, `winkler_interval_score`), `ReferenceClassPrior` (`GB-32`: capex `+28%/+43%`, schedule `+31%`, ramp-up `74%` Yr-1), and `SpatialFrontPitMonitor` (`GB-08` Vecchia/Rosenblatt conditional PIT + `BettingEProcess`).

## 3. Authority, Epistemics & Representation
Enforces pre-registration of forecasts prior to outcome observation (`INV-008`) and monitors spatial front miscalibration (`G-CAL-1`).

## 4. Public API Surface
- `FrozenForecast`, `ForecastPayload`, `ForecastLedger`
- BD-343: `freeze_cohort(CohortSpec)` freezes sorted membership, the resolution rule, eligible sources and
  `AssessmentPolicy` before outcomes. The digest binds the complete payload, including exact float bits.
  Payloads are Gaussian, an interval with a stated or explicitly absent level, quantiles, or an event probability.
- `append_outcome` records hash-chained `Published`, `Restated` and `Missing` events with separate availability
  and recording times. A restatement must name the latest numeric predecessor and a new cited source vintage.
  The original forecast is never mutated. Eligibility and a declared physical outcome domain are checked.
- `assess(cohort, as_of_ns, FirstPublished | LatestAsOf)` retains an immutable assessment. Both event times must
  be at or before the cut. Later events cannot be backdated into an already assessed cut. Every member remains
  visible, including Pending, Overdue, NotPublished, Withdrawn, DefinitionChanged and Divested outcomes.
- An `AssessmentSummary` carries all denominator counts; observed coverage and whole-cohort coverage bounds;
  observed and whole-cohort score bounds; the effective sample range under the frozen correlation assumptions.
  Brier bounds are finite. Interval-score upper bounds need a registered physical outcome domain; an unresolved
  CRPS has no finite upper bound. These are completion bounds, never confidence intervals. Guidance with no
  nominal level remains descriptive. A positive calibration flag requires complete resolution and a conservative
  effective sample at least the frozen threshold (default 35). The default correlation range is [0, 0.5].
- `gaussian_crps`, `gaussian_log_score`, `winkler_interval_score` → `Result<f64, ScoreError>` (non-finite inputs, non-positive std_dev, alpha outside (0, 1) and inverted intervals refuse; nothing is clamped, bd-8wrk)
- `golden_digest_gaussian_crps_10k`, `golden_digest_gaussian_log_score_10k` (bd-hp2j.3): BLAKE3 over the score bits
  on 10,000 `SCORE_DIGEST_KEY` triples (|z| up to 12).
- `golden_digest_calib_rest_10k` (bd-hp2j.3): the rest of `lib.rs`, so one class covers the file: Winkler scores,
  reference-class blends, a ledger's report, and 1,000 samples through `SpatialFrontPitMonitor`, whose bias flips
  to reach both betting limits and the alarm (this also runs `BettingEProcess`). A refusal hashes as `u64::MAX`.
- All three committed values were first observed on aarch64-macos; `lib.rs` declares `BitwisePortable` only after
  ubuntu-latest and macos-15 agree in one CI run.
- `ForecastLedger::evaluate_model` → `Result<CalibrationReport, ScoreError>` delegates to the assessment engine
  and exposes `n_members`, `n_scored`, `n_unresolved` and whole-cohort coverage bounds. This compatibility snapshot
  is always Unscored or Descriptive, since it has no frozen membership. Existing Gaussian `register` ids are
  retained; new `register_payload` ids hash exact canonical bytes. `record_outcome` is an uncited internal
  compatibility path and cannot supply a frozen public cohort.
- `ProjectArchetype`, `ReferenceClassPrior`
- `SpatialFrontPitMonitor`
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

## 5. Guards & Typed Refusals
`ForecastLedger` refusals, by condition (`test_ledger_refusals_by_condition`):
- `FMINE-REF-206` (late registration, exit 3): exact payload registration at or after the outcome time; cohort
  freeze before registration, at or after the outcome time, or after any recorded outcome; an early or backdated
  outcome; an assessment before its cohort. Compatibility `register` retains equality at the freeze boundary;
  such a forecast cannot enter a cohort, which needs a strictly later freeze before the outcome.
- `FMINE-REF-507` (frozen record mutation, exit 6): a second registration of one forecast id, a reused cohort
  name, a second one-shot outcome, an invalid restatement predecessor or reused source vintage, or reinterpretation
  of a withdrawn, changed-definition or divested target as its old quantity without a new resolution rule.
- `FMINE-REF-109` (outside physical domain, exit 3): a non-finite mean or outcome, or `std_dev <= 0`.
- `FMINE-REF-201` (no claim in domain, exit 3): an unknown forecast/cohort, or an ineligible outcome source.

206 and 507 are proposed codes (`docs/adr/ADR-REFUSALS-01.md`).

## 6. Proof Obligations (`PO-*`)
- `PO-CAL-1`: Expected Gaussian CRPS is uniquely minimized at the true data-generating $(\mu_0, \sigma_0)$.
- `PO-CAL-2`: Retroactive forecast registration (`t_registered >= t_outcome_avail`) is refused with `FMINE-REF-206`.
- BD-343: exhaustive binary completions through 12 members verify coverage and Brier bounds; the E474 test uses
  independently captured source cells and registered hashes beside a synthetic forecast; delayed record times,
  restatement replay, source eligibility, frozen membership, interval bounds and minimum-effective-n gates have
  focused tests in `ledger::tests`.

## 7. Failure Modes & Deterministic Fallbacks
Miscalibrated spatial fronts trip `BettingEProcess` at `E_t >= 20.0` (`alpha = 0.05`) and downgrade downstream claims.

## 8. Gates & Kill Criteria (`GATE-*` / `KILL-*`)
- `GATE-HELM-01`

## 9. Lifted Code & Foundation Dependencies
- Depends on `fmine-core`, `fmine-time`, `fmine-evidence`, `fmine-claim`, `fmine-numerics`, and `fmine-registry` (S0)
  for the TOML subset the record's manifest is written in.

## 10. Non-Goals & Scope Boundary
Pure calibration and scoring kernel; does not perform file I/O (`L-3`).

The cohort store is in memory. Its canonical assessment bytes can be retained by an adapter; this slice adds no
durable ledger or external timestamp authority. Caller-supplied times are ordering inputs, not time proofs
(BD-047/BD-068). It does not infer a fallback after a target's definition changes, estimate correlation, or claim
calibration for an issuer guidance range with no stated level. Clopper–Pearson intervals remain part of the public
forecast-score report work; cohort completion bounds do not substitute for them.
