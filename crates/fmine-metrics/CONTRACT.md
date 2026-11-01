# CONTRACT.md — `fmine-metrics`

## 1. Charter & Layer
- **Crate:** `fmine-metrics`
- **Layer:** `E` (Enterprise & Decision)
- **Subsystem:** `Enterprise`
- **LOC Budget:** `16k planned / 18k ceiling` (`registries/loc_budget.toml`)
- **Allowed Module Families:** `["lmdi", "cutoff", "definitions", "guidance", "capacity", "recovery", "reserves", "roll_forward", "mirror"]`

## 2. Mathematical & Domain Formulation
Implements `SiteAiscRow` and `AiscConsolidationBridge` (`L_corp`, `L_JV`, `G-BULL-3`), versioned `PriceDeck` (Barrick reserve decks 2023–2025 as printed in the 2026 Circular, E606; no silver price is stated, so none is carried) and `compute_geo` (`GB-27`), `ReserveReplacementBridge` on one stated basis (the Circular's 190% and 504% differ by metal basis, E603–E604), a spend-based Logarithmic Mean Divisia Index (`lmdi`, `GB-26`, BD-333.S1), and fixed-point marginal cut-off grade solver (`solve_cutoff_grade`; synthetic known answer only, the published cut-offs E182–E184 are not yet reproduced).

The LMDI bridge recomputes each endpoint as `A = supplied total_aisc_spend / oz_sold`. Its six bars are
`L(A1,A0)·Δln(C)` and `−L(A1,A0)·Δln(X)` for `X` in tonnes, grade, recovery, metal balance
`B = produced/(tonnes·grade·recovery)` and sales timing `S = sold/produced`. The printed rates never set the
weight or a bar. Each printed-minus-recomputed endpoint gap is retained, including when the two gaps are
equal, and `printed_rate_gap = (P1−P0)−(A1−A0)` is a separate reconciliation item. The measured floating-point
closure residual is retained rather than forced to zero or allocated to a factor.

The typed public kernel (`lmdi::waterfall`) uses exact displayed rationals with source locators and rounding
half-steps. Its units are fixed: cost in USD millions, sold/produced in koz, ore in thousand reported tonnes, grade
in g/t and recovery as a fraction. It computes `A = 1000·C/Q` exactly. The dimensionless absolute metal-balance
factor would be `B = produced·31.1034768/(tonnes·grade·recovery)` with the exact statutory troy conversion;
only its temporal logarithmic change is evaluated, where that constant cancels. No absolute B is emitted.
For physical inputs whose rounding boxes are not wholly in the positive domain, it supplies the exact signed
additive fallback: line `j` is `1000·Δc_j·mean(1/Q)`, and volume is `1000·mean(C)·Δ(1/Q)`. The supplied named lines
must sum exactly to supplied C in both periods. A rounded midpoint footing discrepancy is not a balancing line.

DESIGN (BD-342): `guidance` computes exact remaining production and the conditional AISC frontier. Annual AISC
is `(r_ytd * sold_ytd + numerator_remaining) / (sold_ytd + sold_remaining)`. The remaining sales interval is a
separate, cited scenario; production never substitutes for sales. Requirements use exact closed intervals and
retain the common YTD-sales variable when enclosing the numerator and rate. Negative computed requirements
remain signed. These are necessary arithmetic conditions, not forecasts or probabilities.

DESIGN (BD-342, COD-01): `capacity` bounds remaining gold production from cited maximum dry feed in million
metric tonnes, gold head grade in g/t and recovery as a fraction. Their product is converted with the exact
troy-ounce constant `31.1034768` g/oz and scaled only by an explicit attributable share. The product of maxima
is an optimistic upper ceiling even when those maxima cannot occur together; it is not a feasible plan.

DESIGN (BD-345.S4): `roll_forward::quarter` derives Q2 = H1 − Q1, Q3 = 9M − H1 and Q4 = FY − 9M
for additive gold production/sales in koz and copper production in kt. A printed value `v` with positive
step `s` supplies the closed enclosure `[v−s/2, v+s/2]`. Subtraction uses `[a.lo−b.hi, a.hi−b.lo]`;
both operands' displayed uncertainty is retained. Every admitted cumulative/prior pair is assessed.
A draft is `Derived` only when its own displayed enclosure contains every pair's enclosure and the
competing assertions for each cumulative period share a common interval. Numeric agreement never selects
a preferred source vintage. These are exact arithmetic enclosures, not forecasts or confidence intervals.

DESIGN (bd-srqc, W4 §3.3): `recovery` fits exactly two descriptive annual relations and applies them to later
quarters. Head grade `h` and recovery `R` are explicitly in percent, and implied tails grade is
`t = h·(1−R/100)`. M-a uses mean tails; M-b is OLS `t = a+b·h`. Pairwise sums of squared head-grade
differences and head/tails cross-products form the slope, avoiding cancellation in the design denominator.
Quarterly M-b projection uses the equivalent centred form `mean(t)+b·(h−mean(h))`, retaining the shared fit
centering when propagating rounding. All displayed-input estimates are exact rationals; closed enclosures
cover the supplied rounding boxes, not statistical confidence or prediction intervals.

DESIGN (BD-346.S3): `mirror` compares gold produced or sold in koz over one explicitly declared asset,
operation perimeter and calendar period. Each reported box has a fixed full or attributable basis. Scale
identities express one common 100% quantity; the existing exact feasibility solver and independent checkers
produce a witness or exclusion certificate. Fixed tolerances and inferred partner shares are absent.
At least two reporters are required, but their shares need not sum to one. The nonnegative physical domain
is explicit: a printed zero keeps its original negative-to-positive rounding box, while the common interval
is intersected with `x >= 0` and the reported witness is checked there. A negative printed midpoint refuses.

## 3. Authority, Epistemics & Representation
Pure `E`-layer metric kernel (`L-3`): enforces `G-BULL-3` (`FMINE-REF-203` when `L_corp` is omitted) and checks
the multiplicative-to-additive LMDI accounting identity (`GB-26`). Its public-data interpretation is descriptive;
arithmetic closure does not establish causation or identify an operational explanation (plan §21.6, E699).

## 4. Public API Surface
- `reserves::{assess, IssuerReserveRow, ReserveDeck, ReserveKey, ReserveReport}` (BD-335): a source-bound
  projection of declared reserve assertions. Rows retain separate economic, quoted-price-deck and filing-vintage
  identities, their exact printed bounds and all source/price readings. Private row fields and a constant
  `EvidenceClass::ISSUER` accessor prohibit promotion by relabelling. The economic key includes issuer, subject,
  reserve year, item, metal, unit, ownership and contained/recoverable support; it deliberately excludes the deck.
  Only registered MD&A/AIF documents supply current rows, selected by the latest declared publication date.
  The selection considers declared rows before checking reading availability: an unusable newer filing blocks
  fallback to an older one. Same-date filings remain ambiguous. Investor decks only cross-check; partner filings
  and technical reports are refused as implicit issuer filings. All supplied earlier vintages remain visible.
  Quoted decks bind positive agreed gold/copper price readings in the same document, including their period,
  basis, units and citations. Different or unstated decks produce named comparison states without asserting a
  reserve restatement. Comparable rows intersect their exact declared rounding intervals, retaining open ends.
  JSON/Markdown and digests include every admitted or refused row, selection, price and document metadata.
  A registry-wide refusal also binds all supplied declarations, documents and raw/admitted readings in a
  canonical input digest, retaining duplicates and making rejected-input mutations distinguishable.
  This projection is not a durable append-only store, a complete filing census or an `EstimateStatement`.
- `roll_forward::quarter::{Quantity, Printed, Observation, Report, Status, assess}`: source IDs identify
  actual operands; a caller-authenticated series key binds issuer, subject, year, scope/perimeter version,
  additive metric, unit, occurrence and explicit normalized ownership. Each observation states its
  cumulative ending quarter and either its displayed value/step or an unusable value. The report retains
  every evaluated pair, exact interval, relation and rule (`H1-Q1`, `9M-H1`, `FY-9M`). JSON bounds are
  exact rational strings. The source adapter enriches these fields with full source/date evidence.
  `WARNING` distinguishes derived information from an issuer disclosure. No target-period Claim is
  manufactured and no input value is promoted to a physically established fact.
- `roll_forward::{assess, TableReport, SourceCell, ComponentCheck, IdentityRefusal}` (BD-340.S5,
  BD-331.S7): binds explicit PDF identity declarations to agreed point readings and their registered document
  metadata. Units, basis, member periods, raw-token precision, distinct reader identities, page bounds and
  source hashes must agree. Missing, ambiguous or incompatible members retain a refused identity and its
  available evidence; independent complete identities still run. Shared cells within a document are solved
  jointly by `exact_feas`; each feasible witness and infeasible certificate is checked again before emission.
  Conflicts carry the kernel certificate digest and minimum or inclusion-minimal correction sets with exact
  implied intervals. Feasible components carry the narrowed ranges satisfying all declared identities.
  Different documents remain separate vintages. The JSON report and its SHA-256 digest bind all declarations,
  admitted source readings, rejected candidate readings, reader roles, filing metadata and results. No file
  I/O occurs here: callers authenticate actual registered bytes and reader agreement before public use.
- `SiteAiscRow`, `AiscConsolidationBridge`
- `PriceDeck` (`barrick_reserve(year)`), `GeoBreakdown`, `compute_geo`
- `ReserveReplacementBridge`
- `solve_cutoff_grade` → `Result<CutoffSolution, CutoffError>`
- `definitions::{all_in_cost_per_oz, is_non_sustaining, bundled_registry, AllInCost, CostLine, MetricFailure, Project}`
  (BD-060): AISC and AIC from `registries/metrics.toml`. A line the definition neither counts nor excludes is refused,
  and every result names its definition and any successor.
- `cutoff::{parse_cases, reproduce, CutoffCase, CutoffReport, CutoffState, Printed, Recovery}` (BD-274): the
  published cut-offs in `registries/published_cutoffs.toml`, recomputed on exact rationals from the printed inputs.
  Each comes back `Reproduced`, `WithinInputRounding` or `Conflict`; `CutoffReport::to_json` prints exact results to
  six places, never through a float.
- `cutoff::{section, CutoffSection, CUTOFFS_REGISTRY}`: every row reproduced, as the JSON object an issuer run of
  `fmine assure public` carries as `published_cutoffs`: the registry's sha256, the count in each state and the rows in
  registry order, with the registry's `issuer`. A row that cannot be computed fails the whole section.
- `lmdi::{LmdiState, LmdiWaterfall, decompose_aisc_lmdi, logarithmic_mean}`
  - `LmdiState` keeps the existing floating-point API. Monetary spend and sold ounces must have compatible
    scales so their quotient is USD/oz: USD/oz or thousand USD/koz. `$M`/`koz` inputs require the explicit
    factor 1,000 on spend before this call. Each variable's unit and ownership basis must agree across periods;
    this legacy input does not encode or infer them.
  - `LmdiWaterfall::{aisc_0, aisc_1, total_delta}` now describe the recomputed rates. `printed_aisc_0/1` retain
    the observed endpoints; `printed_rate_gap_0/1` retain each discrepancy; `printed_rate_gap` reconciles the
    changes. `closure_residual = total_delta − sum_of_factors()` is measured and checked to at most 1e-9
    USD/oz. `sum_of_factors()` includes only the six factor bars.
  - Physical product/ratio changes are evaluated in log space. A temporal quotient that underflows,
    overflows or is subnormal uses the difference of portable logarithms; a rounded subnormal quotient can
    otherwise invent a material closure discrepancy. No input is clamped.
- `lmdi::{PrintedQty, Factor, BasisTag, WaterfallPeriod, BarPoint, Bar, RateReconciliation, Bridge,
  decompose_aisc_waterfall}` (also in `lmdi::waterfall`):
  - `PrintedQty` carries exact `value`, non-negative `half_step` and source `at`. A `Factor::NotPublished`
    retains its token and source, including `n/a`; it is never converted to zero. `BasisTag` distinguishes
    consolidated 100% from an explicit attributable fraction, including attributable 100%. The caller must
    authenticate one common basis within each period; the pure kernel checks that both period tags agree.
  - `Bridge::Lmdi` has six bars (`cost`, `tonnes`, `grade`, `recovery`, `balance`, `sales_timing`) and a measured
    midpoint closure residual. `Bridge::AdditiveFallback` records every physical/domain reason, sorted named
    signed line bars and a `volume` bar. Its points and their closure are exact rationals. Missing line
    identities are not implicitly zero; duplicate/empty identities and a cost/line-sum mismatch refuse.
  - Both variants have an exact `RateReconciliation` with recomputed and printed endpoints, recomputed
    change, both endpoint gaps and the gap change. The gap is never a bar. Each bar names its rule and all
    direct source dependencies, including the C/Q cells that set an LMDI weight. The adapter retains the
    corresponding source Claim identities and evidence; this kernel does not authenticate workbook bytes.
  - Every bar exposes a midpoint and an outward rounding enclosure. Integer-to-float conversion is
    bracketed before division, elementary operations round outwards, and portable-log endpoints widen by
    four ulps. Monotonicity bounds L using the lower/lower and upper/upper rate endpoints. Each endpoint L
    is enclosed by interval evaluation of its definition, intersected with the proven `[min,max]` bound;
    cancellation or an overflowing quotient retains that proven bound. No guessed ulp margin on an
    approximate logarithmic mean narrows an enclosure. Shared dependencies remain an explicit conservative
    over-enclosure. A box containing zero sets `sign_not_determined`, while its midpoint remains visible.
  - `WATERFALL_RULES_VERSION = "m0-waterfall/1"` and `WATERFALL_NO_CLAIM` state the descriptive interpretation.
    The corrected legacy API remains available; its deterministic regression digests are unchanged by this API.
- `lmdi::{golden_digest_logarithmic_mean_10k, golden_digest_decompose_aisc_lmdi_10k}` (bd-hp2j.3): BLAKE3 over the
  output bits on 10,000 inputs from `LMDI_DIGEST_KEY` (series branch, both branches, ratios to e^300, magnitudes
  near the subnormals; seeded positive state pairs). The v2 decomposition draws independently supplied spend
  and binds all recomputed/printed endpoints, gaps, measured residual and six effects. The corrected
  logarithmic-mean branch is relative at every positive scale, so its tiny-wide-ratio digest changes too.
  These intentional mathematical/semantic changes invalidate the v1 snapshots; independent high-precision
  known answers establish correctness, while the v2 bit snapshots remain regression and host-agreement
  checks. `lmdi.rs` declares `BitwisePortable`: both digests agreed on ubuntu-latest (x86_64) and macos-15
  (aarch64) in CI run 41237993223 at 84f32ac (bd-hp2j.3). `lib.rs` and the other modules stay `DeterministicPerIsa`.
- `guidance::{Quantity, QuantityKind, Ownership, Period, CompletionInputs, complete}` (BD-342): all physical and
  monetary inputs/outputs are `Claim<Quantity>`, with exact rational bounds, subject, month support, ownership,
  quantity/unit, and a named AISC definition. `Quantity::canonical_tag` binds every interpretation field.
  `CompletionReport` carries remaining production, YTD spending, remaining spending/rate, and the frontier at
  both annual AISC guidance limits. The exact set lives in the Claim value; `Representation::Deterministic`
  describes the calculation, not a probability distribution. Derived evidence is no stronger than the weakest
  parent and at most `Derived`; all source anchors, parent ids and confidentiality are preserved.
- `guidance::{HistoricalContinuation, compare_history, check_witness}`: T1 is the finite union of supplied
  same-period historical rounding boxes. Production, sales and AISC must come from the same earlier year and
  remaining-month span, on the same basis. Each box is intersected with the explicitly declared remaining-sales
  scenario. `WithinEnvelope` needs a single exact witness satisfying both guidance targets; separate marginal
  successes across different periods do not suffice. `ExceedsEnvelope` excludes only this named envelope, not
  physical capacity. No history is T3/`Indeterminate`. The witness checker re-evaluates source membership and
  both production and annual-cost equations independently of witness construction.
- `guidance::quarters::{QuarterlyHistory, compare_quarters, aggregate_quarters, check_quarter_witness}`:
  T1 also accepts complete contiguous original quarters whose count equals the remaining duration, including
  cross-year windows. Source support remains distinct from hypothetical replay support. Exact Sum/Ratio
  constraints retain each quarter's sales/rate/numerator relationship and the common YTD-sales variable.
  Aggregate rate/sales intervals are marginal display projections, never independent boxes used to decide
  joint feasibility. Positive witnesses are checked against every original quarter and both annual targets;
  exclusions carry independently checked exact-feas certificates bound to input Claim ids and value tags.
  Missing history is T3; solver caps and unresolved numerical checks remain indeterminate.
- `capacity::{PhysicalKind, PhysicalLimit, CapacityLimits, CapacityVerdict, CapacityReport,
  compare_production}` (BD-342, COD-01): accepts a signed remaining-production Claim and three cited physical
  limit Claims over the same subject and remaining months. Physical inputs must be full-mine quantities;
  attributable scaling applies to the resulting production ceiling, never to grade or recovery. The output
  includes the exact ceiling, the signed grade required at maximum feed/recovery, and one of
  `ExceedsStatedProductionCeiling`, `NotExcluded` or `AlreadySatisfied`. `NotExcluded` supplies neither a
  physical feasibility witness nor a joint production-cost witness. Source anchors, parent identities,
  evidence, epistemics, confidentiality and UU terms propagate into both derived Claims.
- `recovery::{Quantity, QuantityKind, TrainingRow, OperatingRow, Estimate, Fit, Model, ModelProjection,
  Projection, SquareRoot, fit, project}` (bd-srqc): every input is `Claim<Quantity>`, with subject, calendar
  period, full-mine ownership, explicit percent/kt unit, exact printed point and closed rounding interval.
  `fit` returns `Claim<Fit>` from distinct complete annual observations. It retains mean head grade, mean
  tails, intercept and slope with rounding enclosures. `project` returns separate model Claims with expected
  recovery in percent, expected-minus-reported recovery in percentage points, signed copper difference in
  kt (`processed_kt·head_percent·gap_pp/10000`) and that difference divided by reported copper production.
  Production is the reported denominator, never a modelled substitute. Neither gaps nor extrapolated
  recoveries are clamped; the kernel does not annualize, monetize or attribute the differences.
  - `residual_sample_variance_at_printed` uses `SSE/(n−1)`, reproducing W4's descriptive sample spread.
    `regression_error_variance_at_printed` uses `SSE/(n−2)`, the conventional fitted-line error estimate.
    The corresponding explicitly named `SquareRoot` summaries enclose numerical conversion and the
    portable square root of each exact printed-input variance. They do not enclose historical input
    rounding or describe a confidence interval. The two denominator conventions are never conflated.
  - Derived evidence is no stronger than its weakest source and at most `ModelledUncalibrated`; issuer
    assertions therefore remain assertions. Outputs retain the sources' anchors, immediate parent Claim
    identities, weaker epistemics, confidentiality and existing UU terms. Their model UU explicitly names
    omitted ore type, grind, throughput effects, concentrate mass pull and annual-versus-quarterly support.
    Its zero monetary field is explicitly unquantified, not an estimated zero downside.
  - `RULES_VERSION = "m0-recovery/1"`, `NO_CLAIM` and `MODEL_CAVEAT` identify a descriptive model comparison.
    `Quantity::canonical_tag`, `Fit::canonical_tag` and `ModelProjection::canonical_tag` bind the exact
    calculation, interpretation and support. Sorting annual observations leaves the result unchanged.

- `mirror::{Perimeter, MirrorReading, MirrorCommon, MirrorReport, MirrorVerdict, compare}` (BD-346):
  source inputs are `Claim<MirrorReading>` under `READING_METRIC`. Canonical tags bind reporter identity,
  numeric source anchor, operation membership, inclusive validity dates and the exact typed quantity.
  Inputs, normalized Claims and proof cells share one sorted order; minimal-source indices address that
  order. `Consistent` carries a checked source-basis witness and a common full-basis Claim. `Conflict`
  carries the checked certificate/digest, blame status and inclusion-minimal source sets, whose disjoint
  normalized intervals independently establish minimality. An incomplete blame search remains explicit.
  `Indeterminate` has no common quantity. All source boxes, Scale identities and Closed rules are exposed.
  Normalized/common Claims retain immediate source parents, anchors, weaker evidence/knowledge/epistemics,
  and every distinct UU term, including different terms sharing a scenario id. No assertion is promoted.
- `mirror::transcriptions::{BoundReadings, bind, keys, period_label}` interprets already-agreed S0
  `Accepted` rows and a `DocumentRow`, without IO. `bind(subject, period, kind, issuer, document, rows,
  evidence)` returns the separately cited ownership Claim, when needed, and source reading Claims.
  Numeric and ownership rows must name the selected document/hash and valid pages; printed tokens,
  periods, units, exact equity percentages and complete dated perimeter context remain explicit.
  The supplied evidence class is preserved. The caller checks bytes, availability and all reader results;
  accepted rows alone cannot prove that an omitted rejection did not exist.

## 5. Guards & Typed Refusals

- `roll_forward::quarter` refuses malformed or duplicate source identities, unequal semantic series,
  unusable admitted values, nonpositive precision, unsupported cumulative periods, checked arithmetic
  overflow, and more than 4,096 candidate pairs. Missing either period yields `NotAvailable`; overlapping
  but insufficient precision yields `PrecisionNotEstablished`. Mutually inconsistent period assertions
  or differing pair conclusions remain `ConflictingDerivations`; all disjoint pair enclosures yield
  `Contradicted`. A dash is never zero, signed differences remain signed, and no failure selects a
  numerically convenient subset. Source identity and publication-cut admission are adapter obligations.

- The mirror refuses unsupported cost definitions/kinds, unequal assets and invalid shares (`105`),
  unequal periods or invalid dates (`102`), missing/unequal operation perimeters or validity straddles
  (`104`), missing numeric/context citations (`201`), private sources (`301`), placeholders (`208`),
  stale/unknown/conflicted/failed-calibration inputs (`502`/`202`/`205`), and mutated canonical Claims (`507`).
  Reversed bounds or negative reported midpoints refuse (`109`); open source bounds refuse (`203`).
  Arithmetic overflow/checker failures refuse (`207`); solver limits remain `Indeterminate`.
- `FMINE-REF-203` (`ConsolidationBridgeMissing`, `G-BULL-3`)
- `FMINE-REF-109` (outside physical domain, proposed; exit 3) on non-positive or non-finite LMDI state inputs,
  or recovery above one, naming the period and field.
- `FMINE-REF-207` (numerical certificate failed, proposed; exit 3) on an unrepresentable recomputed rate,
  a non-finite effect or reconciliation, or a measured closure residual above 1e-9 USD/oz. It supplies no
  forced closure. The bare logarithmic-mean helper returns NaN outside finite positive arguments; the
  decomposition checks its domain and result before returning a waterfall.
- The typed waterfall additionally uses `FMINE-REF-104` for a sold-quantity rounding box touching or crossing
  zero, unmatched/ambiguous signed line identities, or a fallback line sum that differs from supplied cost;
  `105` for unequal ownership bases; `107` for unprinted basis; `201` for a missing input source locator;
  `109` for negative half-steps or an attributable share outside `(0,1]`; and `207` for checked-rational
  overflow, invalid/non-finite numerical enclosures, a midpoint outside its enclosure, or failed closure.
  Missing, non-positive or recovery-above-one physical boxes select the labelled fallback rather than a forced
  logarithm. A non-positive total cost likewise selects the signed fallback. Its Q interval must still be positive.
- `CutoffError` (non-positive cost, price or guess; recovery outside (0, 1]; no convergence in 100 iterations): no input is clamped (bd-8wrk).
- `guidance` refuses unknown/conflicted inputs (`FMINE-REF-202`), stale anchors (`502`), missing citations (`201`),
  incompatible support (`102`), additivity (`103`), unit/metal-stage/ownership/definition mismatches (`105`), and
  undeclared subjects/definitions (`107`). Production inputs may be zero but not negative; this first slice needs
  strictly positive sales and AISC throughout each input interval (`109`). Open-ended intervals are outside the
  declared closed-rounding construction (`203`); arithmetic overflow or a failed witness is `207`. A historical
  period at or after the guidance year refuses (`108`). Failed-calibration claims refuse (`205`).
- `capacity` additionally requires explicit full-mine dry-metric-tonne/gold/fraction units, matching subject,
  calendar support and additivity. It refuses absent citations (`201`), placeholders (`208`), stale inputs
  (`502`), failed calibration (`205`), unknown states (`202`), mismatched basis (`105`), support (`102`) or
  additivity (`103`), open bounds (`203`), invalid physical domains (`109`) and exact overflow (`207`). Zero
  maximum feed or recovery refuses because required grade is undefined; zero grade remains a valid zero ceiling.
- `recovery` requires a positive head-grade box, a recovery box within `[0,100]`, nonnegative processed mass
  and strictly positive reported production. It refuses missing source anchors (`201`), stale Claims (`502`),
  failed calibration (`205`), unknown/conflicted inputs (`202`), placeholders (`208`), unit/full-mine/subject
  mismatches (`105`), support mismatches (`102`), additivity mismatches (`103`), open bounds (`203`), invalid
  domains or a printed point outside its bounds (`109`), duplicate/insufficient annual observations or a
  design-variance enclosure touching zero (`104`), and a quarter at or before the last training year (`108`).
  Exact-rational overflow and invalid numerical square-root enclosures refuse with `207`; no fallback fit,
  substituted point precision or fabricated zero gap is emitted.
- `cutoff::CutoffFailure`: a malformed registry row, a non-positive net value or recovery, or a float solver that
  disagrees with the exact fixed point. A printed figure beyond its inputs' rounding is a result (`Conflict`), not a
  failure.

## 6. Proof Obligations (`PO-*`)
- FACT (BD-345.S4, `roll_forward::quarter::tests`): four focused tests cover independently calculated
  decimal half-step subtraction for Q2/Q3/Q4, signed and zero results, narrower draft precision,
  all competing pairs, input-order invariance, internally conflicting assertions even under a coarse
  draft, missing/invalid operands, ambiguous series, duplicate IDs, unsupported periods and exact overflow.
  These pure known answers establish arithmetic/refusal behavior. The T-layer source-to-lint controls
  separately exercise workbook display precision, source identity, dates, ownership and perimeter admission.
- `PO-MET-1`: `LmdiWaterfall::sum_of_factors() == total_delta` to `< 1e-10` in the synthetic two-period case
  (`+$1,285/oz`; its numbers were once mislabelled as Barrick's, R7-C054). Its independently calculated sales
  timing bar is −536.9148672729303 USD/oz, and its printed gaps are zero because the supplied cost/quantity
  endpoints reconcile. Every accepted waterfall exposes its actual closure residual, at most 1e-9 USD/oz.
- `PO-MET-3`: `ReserveReplacementBridge::compute` returns `None` for non-positive or non-finite depletion; it never clamps the denominator.
- `PO-MET-2`: `AiscConsolidationBridge::verify_rollup(false)` refuses with `FMINE-REF-203` when `L_corp != 0`.
- `PO-MET-4` (bd-hp2j.2): `logarithmic_mean` takes its logarithm from `fmine_core::det`, never the platform libm. On
  10,000 Philox-seeded pairs each, it lies in [min, max], `L(a, b)` and `L(b, a)` are the same bits, and its series and
  logarithm branches agree to < 1e-12 relative at the switch (1.1e-14 measured; the old 1e-7 switch on `ln a − ln b`
  jumped 1.1e-8).
  - A relative near-equality test replaces the incorrect `mean.max(1e-15)` branch selector. Independent
    80-digit Decimal answers cover `L(1e-18,1e-21) = 1.4462006247378286e-19`, the same ratio at 1e-300,
    a subnormal result, and near-maximum positive endpoints. Scale invariance holds across 1e-300 to 1e300;
    equal smallest-subnormal and largest-finite inputs return themselves. An overflowing `a+b` uses a safe
    half-sum while ordinary finite-sum arithmetic retains its previous evaluation order.
- `PO-MET-5` (BD-333.S1 correctness slice, `lmdi::tests`):
  - Hand identities isolate a cost-only +100 USD/oz change and a throughput-only −50 change. Altering only
    printed rates changes no factor bits; changing supplied spend with unchanged printed rates changes the
    cost bar. Two nonzero endpoint gaps survive even when the gap change is zero.
  - The North Mara rate-reconciliation inputs quoted in BD-333.S1 (64 $M/50 koz/1,292 and
    93 $M/37 koz/2,577) give recomputed delta `45640/37` and printed gap `1905/37`, within 1e-12 in this
    floating-point API; bars plus gap give 1,285. That test uses explicitly invented physical factors and
    establishes only endpoint reconciliation, not a real-site operational attribution.
  - A grade-only change reallocates between grade and B without changing total cost per ounce. All seven
    input fields in both periods refuse zero, negatives, NaN and infinities by name; recovery above one,
    rate underflow/overflow and non-finite factor effects refuse. Log-space tests cover overflowing physical
    products and temporal subnormal ratios whose rounded logarithms would create a false residual.
- `PO-MET-6` (BD-333.S1, `lmdi::waterfall::tests`):
  - The historical invented GB-26 case gives +1,285 and sales timing −536.9148672729303 with exact zero gap.
    A hand-computable signed-line case gives bars +675, −225, −112.5 and +537.5, summing exactly to +875;
    endpoint gaps 3 and 5 give a separate +2 gap change. Negative total costs also produce exact signed bars.
  - Hand-entered North Mara published C/Q/P and physical inputs are checked against an independent
    80-digit Decimal oracle (`repro/barrick/waterfall_census.py`, calculated before the new Rust was read).
    The exact change is `45640/37`, gap is `1905/37`, and bars plus gap give +1,285. A counterfactual grade
    1.80→1.98 moves 174.21850713853705 USD/oz between grade and B without changing the total or gap.
    This demonstrates non-identification. The pure test does not stand in for source/vintage/basis verification.
  - 10,000 Philox-seeded signed line sets close exactly and match separate closed-form rational answers;
    reordering input lines gives the same result. Seeded LMDI bar permutations close within 1e-9 USD/oz.
    All 4,096 corners of a twelve-input LMDI rounding box and all 64 corners of a six-input additive box lie
    within their original bar enclosures. Independent known answers cover the logarithmic-mean bound near
    equality and at tiny scales. Straddling intervals flag the sign, with positive midpoint still shown.
  - Mutating only printed endpoints cannot move a bar. Equal nonzero endpoint gaps remain visible.
    Invalid basis, precision, source, Q interval, missing/duplicate lines and arithmetic overflow refuse;
    an interval-feasible but unequal rounded line sum cannot become a plug in the additive fallback.
- `PO-MET-6` (BD-342, `guidance::tests`, synthetic known answers):
  - The target 200–230 minus 68 yields exactly 132–162. The bead-shaped cost example uses explicitly synthetic
    sold ounces; sold quantities differing from production produce the independently calculated weighted rate.
  - Equal annual and YTD AISC cancel their shared uncertain YTD sales exactly; separately enclosing the numerator
    and denominator would introduce spurious width. Exhaustive small interior values lie within the computed
    frontier and attain each extremum. Signed negative remainders and budgets are preserved.
  - Three hundred discrete historical continuations agree with direct integer production/cost inequalities.
    Separate production and cost successes cannot produce a joint witness. An interior rational witness reaches
    an exact guidance boundary; corrupting it fails the independent checker.
  - Domain, zero-denominator, ownership, definition, support, missing/stale/evidence, and overflow refusals are
    checked. Historical ordering leaves the report identical and every output retains its input provenance.
- FACT (BD-342, `guidance::quarters::tests`): eight focused regressions cover exact source-quarter witnesses,
  cross-year ordering, one/two/three-quarter remainders, source/witness corruption and explicit solver caps.
  A 144-case integer oracle checks joint verdicts. The sales/rate counterexample uses quarters with sales
  `[1,2]` and rates 10 and 100: marginal weighted rates span `[40,70]`, but total sales 2 forces rate 55.
  The quarterly model correctly excludes the false witness admitted by independent aggregate boxes.
- `PO-MET-7` (BD-342, COD-01, `capacity::tests`): the independently calculated 600 koz remaining requirement
  against 10 Mt, 2 g/t and 90% recovery exceeds the exact ceiling `3750000000/6479891` koz and requires
  `6479891/3125000` g/t at maximum feed/recovery. Eight focused tests also cover attributable scaling, signed
  already-delivered requirements, equality at the ceiling, rounded physical limits, zero/invalid denominators,
  unit/support/ownership refusals, exact overflow and complete source/uncertainty propagation.
- `PO-MET-8` (bd-srqc, `recovery::tests`): independent synthetic annual heads `[1,2,4]` and tails
  `[1/5,3/10,1/2]` give mean tails `1/3`, intercept `1/10` and slope `1/10`. A quarter with head `2%`,
  recovery `82%`, processed `1000 kt` and produced `82/5 kt` gives M-a gap `4/3` pp, difference `4/15 kt`
  and output fraction `2/123`; M-b gives `3` pp, `3/5 kt` and `3/82`. A separate nonzero-residual case
  distinguishes the `n−1` and `n−2` variances. All 1,024 combinations of six training and four operating
  input-box corners are enclosed; widening the boxes cannot narrow the model outputs. Signed negative
  gaps, unclamped extrapolation, deterministic input ordering, source mutation, weaker evidence,
  confidentiality/UU propagation and the admission/domain/overflow refusals have direct tests. No historical
  issuer training rows are embedded in these synthetic fixtures.

FACT (BD-346, `mirror::tests`): ten focused tests cover the exact three-source tie at 100 koz; the
1000/615/385 case; independent witness/certificate checks and corrupted proofs; minimal conflict pairs;
zero's explicit domain; two 45% reporters and sold quantities; reporter-order/full-basis metamorphisms;
dated scope and evidence/UU preservation; same document basenames across issuers; semantic, source,
canonical-mutation and numerical-cap refusals. A separate integer-rounding oracle keeps the known truth
inside the common interval for 256 synthetic worlds. This is not the full BD-346.S6 power study.
Direct transcription-binding tests additionally check exact source/share Claims and refuse mismatched
numeric or ownership document names, hashes and pages before construction.

## 7. Failure Modes & Deterministic Fallbacks

The legacy floating API refuses non-positive physical inputs. The typed public API attempts the exact signed
additive fallback when physical factor boxes cannot support logarithms; if the supplied lines cannot justify
that fallback it refuses explicitly. Missing consolidation adjustments and numerical failures refuse. Neither
path uses a printed-rate discrepancy as a balancing factor.

## 8. Gates & Kill Criteria (`GATE-*` / `KILL-*`)
- `GATE-BULL-01`

## 9. Lifted Code & Foundation Dependencies
- Depends on `fmine-core`, `fmine-units`, `fmine-time`, `fmine-evidence`, `fmine-claim`, `fmine-numerics`,
  and the S0 rows in `fmine-registry`.

## 10. Non-Goals & Scope Boundary
The additive quarter kernel authenticates no workbook bytes, publication dates or ownership registry.
Its caller must admit all eligible evidence before assessment. It supplies no probabilistic confidence,
source ranking, implicit unit conversion, subtraction of non-additive rates, or disclosure permission.
A non-obvious arithmetic consequence can itself be new information. This bounded feature does not close
the general derived-identity or independently labelled disclosure-lint assessment gates.

DESIGN (BD-340.S5): the PDF identity adapter verifies declared arithmetic of issuer assertions, not their truth
or completeness. It never emits reserve classifications, selects a preferred filing/deck vintage, silently
changes a printed sign, or supplies an issuer-attributed corrected figure. Synthetic E692-shaped arithmetic is
a known-answer test of the binding; the authentic historical transcription and independent census gates remain
open. Closed intervals are agnostic about ties; a named half-open convention is never changed to rescue a row.

The mirror kernel authenticates no filing bytes, human readership, publication date or issuer affiliation;
its caller must bind those facts before supplying source Claims. It does not infer missing perimeters,
aggregate incomparable operations, select vintages, invent an absent partner share, bridge costs/capex,
transfer bridges between assets, or decide which reporter is physically correct. Actual FY2023–FY2025
partner-source reproduction and the full synthetic power gate remain separate BD-346 work.

Pure metric kernel in layer `E`; workbook parsing, first-vintage pairing, per-period basis authentication,
workbook source Claim bindings and public rendering live in `fmine-io` / `fmine-public` (`V`, BD-333.S2/S3).
The mirror's pure transcription binding consumes already-agreed S0 rows, without authenticating bytes or
publication availability. The typed kernel supplies the numerical and provenance-locator contract, not
real-file eligibility counts or causal claims.

BD-342's kernel is exact arithmetic on supplied, cited gold/koz and USD/oz quantities, with a declared positive
cost domain. It does not acquire or transcribe guidance, register future forecasts, infer missing inputs, equate
sales with production, estimate probabilities, authenticate T2 technical-report capacity evidence, or infer
dependence inside a historical rounding box. The capacity kernel checks supplied cited limits; a registered
technical-report adapter must establish their public eligibility before a report can claim T2. Source selection
and historical-box construction belong to the public adapter.
Real North Mara parity and the full bead require independently read guidance and actual-source cells; synthetic
known answers establish only the arithmetic and refusal behavior.
