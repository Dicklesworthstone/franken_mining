# CONTRACT.md — `fmine-plant`

## 1. Purpose

DESIGN: Crucible accounts for elemental inventory and declared conservative plant flowsheets across
physical boundaries and intervals (plan §§17.2–17.5; BD-175/177/313). It keeps stocktake timing separate
from integrated flows and refuses unclosed unit or whole-plant balances. This is original code, using
the existing claim-admission pattern; no FrankenSim solver is copied or linked.
Inventory and joint-flowsheet adapters perform conditional measurement reconciliation through the shared
K0 numerical owner, preserving the original observations and a separately declared error model.

- **Crate:** `fmine-plant`
- **Layer:** K2
- **Subsystem:** Crucible
- **LOC budget:** 32,000 planned / 37,000 ceiling
- **Allowed Module Families:** `["inventory", "flowsheet", "grinding", "reconciliation", "measurements", "raw_measurements", "cards", "rtd"]`

## 2. Inputs

Six `Claim<MetalBalanceInput>` values: an explicit complete-population declaration, opening stock, feed,
product, other outflow and closing stock. Every physical quantity is a nonnegative `ExactQty` of kilograms
of the same total elemental measurand in all phases, at the full physical boundary. Explicit zero totals
are required. Ore mass, grade, concentration, mass rate and ownership shares are not input units.

Every input names the same `BalanceScope`: boundary version, case-sensitive element identity, measurand
identity and positive-duration `ValidTime`. Flow totals and the declaration cover that exact interval;
stocktakes are instants at its endpoints. `PeriodAggregate` identifies their common accounting support.
Each input has a source artifact SHA-256 and locator, deterministic nominal representation, correct
additivity, usable state and a matching semantic value tag. The kernel performs no IO.

`flowsheet::account_flowsheet` accepts `Claim<FlowsheetInput>` values: one complete declaration of units,
directed streams and elemental measurands; one integrated observation for every declared stream; and
opening and closing stocktakes for every unit. Explicit zero amounts are required. Each observation
contains exact kg of non-water dry matter (including dissolved matter), H2O, and every tracked element
in all phases. Elements overlap the carriers and are not added to them. Mechanical unit labels select
no equipment prediction. An internal edge has one shared observation and an explicit local outflow role.

## 3. Outputs

`account_interval` returns an immutable `MetalBalanceReceipt`, containing original admitted inputs,
a `Claim<MetalBalanceSummary>` and a BLAKE3 receipt digest. The summary reports all five mass totals,
signed inventory change, zero exact residual and optional product / same-interval feed diagnostic.
That ratio may exceed one during drawdown; zero feed produces `None`. It is not a recovery estimate.
The composite output claim is non-additive. A nonzero residual returns a refusal containing its exact
signed numerator/denominator, without a closed receipt or balancing stream.

The immutable `FlowsheetReceipt` retains original inputs and a `Claim<FlowsheetSummary>` over every unit
and the full boundary. Each element is projected through the existing `account_interval` owner; dry
matter and H2O close independently with exact arithmetic. Internal streams enter both endpoint accounts
but cancel from external feed/outflow totals. Every original source still participates in the final
evidence and confidentiality, including an internal observation that cancels from the outer balance.

## 4. Invariants

- INV-001: physical inputs and the output use Claims; sources remain parents and anchors
  (`exact_known_answer_retains_physical_inventory_and_source_claims`).
- INV-005: opening + feed = product + other outflow + closing exactly; neither residual sign is plugged
  (`exact_decimals_close_and_even_a_small_unbalanced_residual_refuses`).
- INV-006 / NL-2: arithmetic is at most Derived and meets every input, including completeness
  (`evidence_never_strengthens_and_assertions_keep_their_colour_refusal`).
- INV-007: all input confidentiality labels join; public export still refuses restricted results
  (`confidentiality_joins_every_input_including_the_completeness_declaration`).
- INV-014: mixed boundary, measurand or temporal support refuses; stock and flow supports differ explicitly
  (`boundary_element_measurand_and_interval_cannot_be_mixed`,
  `stocktakes_are_endpoint_instants_and_flows_cover_the_exact_interval`).
- Input values are independently rebound to their semantic tags, covering generic Claim hash omissions
  (`typed_value_mutation_is_caught_even_when_generic_claim_id_still_matches`).
- Unusable source states and failed calibration cannot supply totals
  (`unknown_refused_stale_and_uncertain_sources_cannot_supply_numeric_totals`).
- Flowsheet units close independently; offsetting hidden unit gains/losses cannot be masked by outer
  closure. Recycle uses one original observation at both endpoints, never an extra external feed.
- Every declared stream and both stocks per unit are required. Undeclared, duplicated, missing or
  temporally incompatible observations refuse; no missing channel becomes a measured zero.
- Nonnegative elemental masses sum to no more than dry matter plus H2O; non-H/O elements sum to no more
  than dry matter. Water cannot supply metal mass. Phase/species allocation is not inferred.

## 5. Refusals

- `FMINE-REF-101` (exit 3): any nonzero exact conservation residual.
- `FMINE-REF-102` (exit 3): boundary, measurand, interval, stocktake instant or accounting-support mismatch.
- `FMINE-REF-103` (exit 3): mass totals not extensive, or an additive completeness declaration.
- `FMINE-REF-105` (exit 3): duplicate/missing roles within a six-input set.
- `FMINE-REF-107` (exit 3): absent scope names or malformed case-sensitive elemental symbol.
- `FMINE-REF-109` (exit 3): negative physical total or nonpositive reporting interval.
- `FMINE-REF-201` (exit 3): incomplete role population, no completeness declaration, or missing source anchors.
- `FMINE-REF-202` (exit 3): unavailable knowledge state or a representation needing uncertainty reconciliation.
- `FMINE-REF-204` (exit 3): invalid or conflicting UU metadata.
- `FMINE-REF-205` (exit 3): failed calibration or invalid colour metadata.
- `FMINE-REF-207` (exit 3): unrepresentable exact arithmetic, including a diagnostic ratio.
- `FMINE-REF-502` (exit 6): stale input.
- `FMINE-REF-507` (exit 6): mutated claim identity, typed value tag mismatch or incompatible metric identity.

## 6. Determinism class

`lib`, `inventory` and `flowsheet`: Exact. Conserved quantities use reduced i128 rationals; inputs are ordered by role,
parents/anchors by ordered sets, and UU scenarios by identity. IEEE bits are retained only for supplied
epistemic metadata. `receipt_identity_is_input_order_independent_and_immutable` checks input permutations.
Flowsheet units, streams and elements have canonical identity order; declaration/input permutations
preserve the graph receipt. Generic claim-admission helper extraction preserves original inventory receipts.
`reconciliation` and `flowsheet::reconciliation`: DeterministicPerIsa. Their typed binary64 estimates and covariance come from the
shared LDLT-based numerical owner; source observations retain their original exact rational values.

## 7. Evidence class of outputs

The output class is `Derived` met with all six input classes. Knowledge state and epistemic status conjoin;
confidentiality joins; compatible UU scenarios are retained and duplicate identities with conflicting
definitions refuse. Assertions remain assertions with no colour. The composite balance receives no
validated regime or measurement-accuracy claim from successful arithmetic. It is not a decision output.

## 8. Validity domain

One full physical boundary, one declared conserved elemental measurand, one positive-duration interval,
two endpoint stocks and integrated nonnegative nominal mass totals. Element symbols are validated for
case/shape, not looked up or chemically interpreted. The completeness declaration is conditional input;
its truth is not independently audited. `inventory` and `flowsheet` closure is exact, with no measurement
tolerance or error-model adjustment. No sequence continuity, as-of availability or authority-ledger check is inferred.
The flowsheet is restricted to conservative mechanical transfer on the declared dry-matter/H2O basis.
Cycles and held inventory are admitted; reactions, phase changes that alter carrier definitions,
process prediction and estimated recoveries are outside this nominal-accounting interface.
Declared unit stocks must partition the outer inventory without overlap; geometry and source coverage
are conditional declarations rather than an independently audited partition.

## 9. Oracles

Hand-accounted synthetic totals in `inventory/tests.rs`: 10 + 100 = 72 + 8 + 30 kg; the decimal case
0.1 + 0.2 = 0.25 + 0.01 + 0.04 kg; and pulse/drawdown/shutdown/release cases. Perturbing the final stock
by 0.000000001 kg refuses with the signed exact residual. These are known-answer arithmetic checks,
not empirical plant observations. The runnable `inventory_pulse` example cites its own constructed
source schedule and labels every output synthetic.

`flowsheet/tests.rs` and runnable `flowsheet_recycle` construct a mill/classifier recycle with changing
inventories: external dry feed 100 kg, product 80 kg, other outflow 5 kg and stocks increasing 30→45 kg;
external H2O feed 200 kg, product 150 kg, other outflow 20 kg and stocks increasing 60→90 kg; Au feed 1 kg,
product 0.8 kg, other outflow 0.05 kg and stocks increasing 0.3→0.45 kg. Internal dry recycle is 60 kg,
not new external feed. `--hidden-gain` perturbs the shared mill-discharge observation and refuses with
no successful output although the outer account is unchanged. Source mutations, missing zero stocks,
impossible metal-in-water composition, identity order and internal confidentiality have regressions.

## 10. Known gaps

BD-175 now has a nominal conservative plant-flow graph and a joint conditional reconciliation adapter;
process-model prediction and shared cross-domain flow adapters remain. BD-177 now has covariance-aware
reconciliation of fully measured carrier and elemental totals, plus identifiable explicitly unmeasured
streams with measured endpoint stocks (§17), including cross-element and cross-stream covariance.
Nonlinear uncertainty propagation, missing-stock inference, sensor bias estimation, plant error-model
calibration and full P754 reconciliation remain.
BD-287 now has source-bound, versioned nominal recovery-card evaluation (§18), with declared joint
input populations propagated per realization (§20); report ingestion, source-box qualification,
fitting and calibration remain. BD-313 now has source-bound forward RTD
transport and conditional lag alignment (§19); kernel fitting and noisy inverse deconvolution remain. No real plant fixture, independent source verification, durable receipt decoder, site-format importer,
forecast registration, sequence authority or permission-to-operate claim is supplied. The declared CLI
worksheets expose nominal material conversion, elemental inventory accounting/reconciliation and joint flowsheet reconciliation;
their byte-bound declarations remain Assumed rather than verified plant measurements.
Generic Claim hashes omit some fields; this slice independently checks values and binds admitted
support, representation, UU, supersession and calibration metadata into its receipt.

## 11. Public API

`inventory::{BalanceScope, BalanceValue, MetalBalanceInput, MetalBalanceSummary, MetalBalanceReceipt,
account_interval, INPUT_METRIC, OUTPUT_METRIC, MASS_BASIS, NO_CLAIM}`. `MetalBalanceInput::canonical_tag`
binds typed source projections. `MetalBalanceReceipt::{inputs, summary, claim, digest}` exposes immutable
views. No physical public signature uses bare f64.

`flowsheet::{UnitId, StreamId, UnitKind, UnitSpec, OutflowRole, StreamRoute, StreamSpec, ElementKey,
MaterialAmounts, FlowsheetScope, StocktakeKind, FlowsheetValue, FlowsheetInput, CarrierBalance,
MaterialBalance, FlowsheetSummary, FlowsheetReceipt, account_flowsheet}`. `FlowsheetInput::canonical_tag`
binds the entire typed source projection; receipt accessors expose immutable original inputs and results.

## 12. Proof obligations

PO-UNIT-2: quantities enter as `Claim<MetalBalanceInput>` using exact kg values; `xtask check-claims`
checks public signatures. INV-005 is demonstrated by positive closure and both signs of a nonzero
residual; INV-007 by the label-join/export refusal test. Exact arithmetic establishes no calibration
or reconciliation proof obligation for a real plant.

## 13. Fully measured elemental inventory reconciliation (DESIGN)

`reconciliation::reconcile_inventory` takes the same six `Claim<MetalBalanceInput>` roles plus a
`Claim<InventoryErrorModel>`. The model binds the exact five observation claim IDs, boundary/element/
measurand/interval, a full 5×5 covariance in kg², an explicit unbiased-Gaussian assumption, a global
chi-square rejection threshold, an optional simultaneous normalized-adjustment policy and a kg closure
allowance. Physical public quantities use `Qty<Mass>` and `Qty<SquaredMass>`; test statistics are
dimensionless. The deterministic model envelope describes supplied parameters; the covariance inside
it describes measurement uncertainty. No independent-error assumption or variance is manufactured.

All source admission, source anchors, semantic tags, stale/calibration checks and exact role/time
checks still apply. The adapter retains the exact original imbalance and compares exact source
quantities with their binary64 representations. Conversion loss exceeding the smaller of one-millionth
of a declared sigma and one-fifth of the closure allowance refuses; so does an unrepresentable fitted
adjustment or loss of the original exact residual. Fitted values are never rounded into an exact
`MetalBalanceReceipt`. Full covariance remains present in the composite result; fitted coordinates
must not be interpreted as independent measurements.

`InventoryReconciliationOutcome::Admitted` contains an immutable `ReconciledInventoryReceipt` with
original observations, original error-model claim, result claim and digest. Output evidence meets
every source, confidentiality joins, compatible UU metadata survives, and knowledge is capped at
`Estimated`. The result is a conditional model calculation, not a calibrated observation.
`Rejected` retains the same originals, numerical diagnostics, refusal and digest but exposes no
admitted balance claim. Negative fitted mass rejects with `FMINE-REF-109`; excess global/normalized
discrepancy or domain closure error rejects with `FMINE-REF-101`. Malformed sources or numerically
unresolved covariance/constraints return a refusal without a candidate. No measurement is deleted,
no fitted negative is clamped, and a single balance cannot identify the uniquely faulty instrument.

`examples/reconcile_inventory.rs` declares synthetic observations [10,100,72,8,31] kg and diagonal
variances [1,4,1,1,9] kg². It obtains a -1 kg original residual, chi-square 0.0625, and fitted totals
[10.0625,100.25,71.9375,7.9375,30.4375] kg. Replacing closing stock by 50 kg produces chi-square 25
and rejection against the explicitly illustrative threshold 9. The example prints both outcomes.
`reconciliation/tests.rs` independently checks these weights and full covariance, correlated errors,
gross-error rejection, a negative candidate, exact-to-float precision loss, source/model tampering,
scoped observation identities and participation of model evidence/confidentiality.

This is plan §20.3's fully measured linear R1 slice using `fmine_numerics::reconciliation`; it does not
claim a real plant fixture, empirical confidence calibration, latent-state/bias R2 model,
reconciliation of grades as independent extensive totals, or P754 compliance. Joint flowsheet
reconciliation is a separate adapter described in §15. Original source families and receiver-side
permissions remain authoritative.

## 14. Nominal material projection (DESIGN; BD-177/336)

`measurements::project_material` converts Claims over one full physical material and time into exact
`MaterialAmounts`: all non-water dry matter, H2O and total elemental kilograms. Source masses explicitly
declare kg or metric tonnes; dry-basis assays declare mass fraction, mass percent or grams per tonne.
Two exclusive routes are admitted: dry mass plus explicit water mass, or wet mass plus wet-basis moisture
(H2O / wet mass). Missing water never defaults to zero. An instant may identify a stocktake. Assays refer
to all non-water dry matter, including dissolved residue; separated-solid, partial or speciated assays
cannot acquire that meaning through this arithmetic. H/O refuse because their total elemental content
would require an explicit water-stoichiometry model. Fractions outside the physical domain, overlapping
elemental assay identities, incompatible supports, conflicting routes and unrepresentable arithmetic refuse.

The immutable receipt retains original Claims and declared units. Source semantic tags and admitted
envelopes participate in its digest; evidence meets, confidentiality joins and compatible UU scenarios
survive. Every output is nominal arithmetic. No variance, independence, Gaussian law, measurement
accuracy, recovery or uncertainty interval is inferred from multiplying a mass and an assay.

FACT (constructed unit tests in `measurements/tests.rs`): 2.5 dry tonnes at 4 g/t Au and 2% Cu produce
0.01 kg Au and 50 kg Cu. A 1,000 wet-tonne input at 8% wet-basis moisture and 0.6% dry-basis Cu produces
920,000 kg dry matter, 80,000 kg H2O and 5,520 kg Cu. The BD-336 monthly example preserves its 25,000 kg
Cu imbalance; projection does not reconcile it. Tests cover declared unit equivalents, metadata and
source mutations, input-order determinism, exact overflow and physical/support refusals.

`fmine plant project --input tests/plant/material_projection.toml` exposes this nominal owner. Its
strict worksheet requires an explicit total-element assay basis and whole-material support declaration.
Inputs remain Assumed, Internal and uncalibrated; the exact submitted bytes are hashed. Site source
auditing, sampled-support conversion, full P754 qualification and the complete BD-177/336 gates remain open.

## 15. Joint conservative-flowsheet reconciliation (DESIGN; BD-149/175/177)

`flowsheet::reconciliation::observation_order` admits the original complete graph and returns a
canonical source-bound coordinate order. `reconcile_flowsheet` uses one full kg² covariance over
all dry-matter, H2O and elemental stream/stock quantities. `ObservationKey` binds the exact source
ClaimId, transfer or endpoint stock location, and channel. The error-model Claim binds the declaration,
scope, complete ordered keys, explicit unbiased-Gaussian assumption, gross-error policy and one kg
closure allowance for every channel. No block-diagonal or independent-error assumption is supplied.

Every unit/channel contributes a conservation row. Internal streams share a single observation at
both endpoints, including recycles; no redundant outer-boundary row enters the solve. The existing
`fmine_numerics::reconciliation` owner fits the fully measured graph under its 128-coordinate and
64-independent-row bounds. Exact source admission is shared with the nominal graph. Exact-to-binary64
conversion and closure checks refuse loss that exceeds the declared measurement/precision budget.

The immutable receipt retains originals, the error model, fitted amounts, adjustments, full posterior
and adjustment covariance, original exact unit residuals, fitted residuals and discrepancy diagnostics.
External opening/feed/product/other/closing totals are projected with their full cross-channel and
cross-role covariance. A separate whole-boundary audit checks aggregate closure and precision.
Each nominal and fitted role total also receives an independent exact precision check, with one-fifth
of its channel tolerance, so equal opening/closing rounding losses cannot cancel in the residual.
Fitted negatives, physically impossible elemental composition and declared gross-error failures keep
the originals and numerical diagnostics but expose no admitted balance Claim. Arithmetic never plugs
a stream, deletes an observation or turns a fitted float into an exact nominal balance receipt.

Evidence meets all sources, confidentiality joins, compatible UU scenarios remain, and output knowledge
is capped at Estimated. The graph remains conservative on the declared non-water/H2O carrier basis.
This fully measured API requires every observation. Section 17 adds explicitly unmeasured streams
with measured stocks. Missing-stock and correlated systematic-bias inference, chemical conversion,
process prediction, error-model calibration and empirical P754 qualification require separate models.

FACT (constructed known answers): `examples/reconcile_flowsheet.rs` fits a two-unit recycle with
27 coordinates, six constraints and correlated dry feed/product errors. Its global statistic is 32/133;
opening-stock variance is 550/399 kg². `--gross-error` refuses before any successful stdout.
Graph tests include an independent two-metal correlated stocktake solution and cases whose
exact aggregate closes but binary64 loses 1 kg, including equal losses in opening and closing totals.
Existing exact-flow receipt identities remain unchanged. These constructed oracles do not establish
real plant calibration or P754 qualification.

`fmine plant flowsheet --input tests/plant/flowsheet_reconcile.toml` exposes this owner through a bounded,
byte-bound worksheet. The input explicitly supplies topology, all extensive kg coordinates, temporal
supports, every covariance pair and per-channel tolerances. The output preserves the full graph, original
Claims and model, fitted covariance, unit/outer diagnostics and admitted or rejected receipt identity.
Worksheet declarations remain Assumed, Internal and uncalibrated.

## 16. Lot-to-stream projection with first-order covariance (DESIGN; BD-151/155/177)

`raw_measurements::project_transfer` accepts an explicitly complete, nonoverlapping lot population
for one physical stream and interval. Each PlantLot supplies exact wet kg M, water/wet-mass fraction w
and every total dry-basis elemental fraction g. A named, matched moisture/assay dry state binds every
source. The declaration must identify moisture as H2O and assays as covering all non-water matter.
Drying mass loss alone establishes neither assumption. H/O assays require a separate water-composition
model and refuse here. This is distinct from `measurements`' single-material nominal unit conversion.

Each lot contributes exact D=M(1-w), W=Mw and E=M(1-w)g before any aggregation. Aggregate dry grade is
sum(E)/sum(D), never an average of assays or a product of separately averaged moisture and grade.
Missing roles, conflicting bases/times/supports, impossible fractions, duplicated elements and exact
overflow refuse. Every declared element is present for every lot; no missing value becomes zero.

A separate `RawTransferErrorModel` Claim binds the declaration and canonical observation IDs,
dimensioned mass/fraction standard uncertainties, and a full dimensionless correlation matrix including
cross-lot and cross-channel terms. All coefficients, including zero correlations and uncertainties,
are explicit. Correlation is finite, exactly symmetric, positive definite and unit diagonal; singular
raw correlations need a lower-dimensional declaration. The source must accept first-order applicability.
Exact derivatives are formed before conversion, preserving small 1-w values near complete moisture.
The shared numerical owner propagates J diag(sigma) R diag(sigma) J' into full extensive kg² covariance;
nonzero numerical range loss refuses. Output covariance may legitimately be singular.

The immutable `TransferProjectionReceipt` retains original sources, the error model, exact extensive
amounts/aggregate grades, channel order, covariance and full-envelope digest. Evidence meets all sources,
confidentiality joins and knowledge is capped at Estimated. Its nominal `Claim<FlowsheetInput>` uses
the projection Claim as its parent, preserving the moisture/basis/model provenance into the graph.
The approximate covariance does not automatically become a Gaussian WLS model. f(x) is not asserted
to equal E[f(X)], and this method supplies no nonlinear confidence bounds or empirical calibration.
First-order propagation follows [NIST TN1297 Appendix A](https://www.nist.gov/pml/nist-technical-note-1297/nist-tn-1297-appendix-law-propagation-uncertainty).

FACT (independent constructed example): `cargo run -p fmine-plant --example raw_lot_transfer` combines
100 kg at w=0.1 and g=0.01 with 200 kg at w=0.2 and g=0.02. Totals are 250 kg dry matter, 50 kg water
and 4.1 kg Cu, giving 1.64% aggregate dry Cu. With sigmas [1 kg,0.01,0.001] per lot, weight correlation
0.5 and explicitly zero remaining off-diagonals, covariance in [D,W,Cu] order is
[[7.17,-4.62,0.1217],[-4.62,5.07,-0.0842],[0.1217,-0.0842,0.035881]] kg².
Tests independently check this full matrix, near-one moisture derivatives, direct use by the existing
exact graph, source/basis/model refusals, numerical range and provenance/confidentiality retention.
Nominal quantities are Exact; covariance arithmetic is DeterministicPerIsa.

`fmine plant transfer --input tests/plant/raw_transfer.toml` exposes this owner through a strict,
byte-bound worksheet with explicit whole-lot declarations and every raw uncertainty/correlation.
The output preserves original observation tokens, canonical model binding, exact aggregate totals,
full first-order covariance and the parent-linked nominal transfer. It does not automatically adopt
that approximate covariance as a Gaussian reconciliation error model.

## 17. Explicitly unmeasured flows (DESIGN; BD-149/175/177)

`flowsheet::reconciliation::unmeasured::reconcile_unmeasured_flowsheet` accepts the original graph
Claims and a separate `Claim<UnmeasuredFlowModel>`. The model binds the complete topology, scope,
canonical measured `ObservationKey`s, explicitly unmeasured stream IDs, full covariance of genuine
observations, Gaussian assumption, diagnostic policy and per-channel closure allowances.
`observation_order(inputs, &unmeasured_streams)` supplies the admitted measured ordering.

All carriers and elements of each named unmeasured stream are inferred together. Opening and closing
stocktakes for every unit remain measured. Missing stocktakes, undeclared omissions and a stream
supplied as both measured and unmeasured refuse. The original exact and fully measured entry points
retain their complete-observation requirements. No observation, variance or prior is supplied for
an unmeasured quantity.

The shared `fmine_numerics::reconciliation::partial` owner solves measured/latent GLS. Independent
measured stock coordinates support the measured-row covariance; the latent incidence must have full
column rank. Ambiguous unmeasured cycles, parallel unknown outlets and excess unknowns refuse
`FMINE-REF-104`. Bounds are 128 total measured/inferred coordinates and 64 unit/channel rows.
An internal inferred stream uses one coordinate at both endpoints.

The immutable admitted or rejected receipt retains original Claims, the model, measured adjustments,
full joint covariance in measured-then-inferred order, and source-bound diagnostics. Observed
contributions are distinguished from complete balance residuals. External totals involving an
unmeasured stream have no observed total; fitted totals retain full covariance and independent
precision/closure audits. Negative or impossible fitted compositions and gross-error failures expose
no admitted Claim. Evidence meets, confidentiality joins and output knowledge is capped at Estimated.

Redundancy is the number of unit/channel rows minus identifiable unmeasured coordinates.
`GrossErrorDiagnostic::NoRedundancy` explicitly supplies no gross-error test when this is zero;
a uniquely inferred flow does not establish that the measurements passed such a test.
The determinism class is DeterministicPerIsa. Missing-stock state estimation, temporal/bias R2
learning, bilinear grade/tonnage fitting, chemical conversion and empirical P754 qualification remain
outside this slice.

FACT (constructed known answers): unmeasured tails in a two-unit recycle infer 5.4 kg dry matter
with variance 4.2 kg², feed/tails covariance 0.4 kg², chi-square 0.2 and three redundant constraints.
`cargo run -p fmine-plant --example reconcile_unmeasured_flowsheet` infers product quantities
[30 kg dry matter, 15 kg H2O, 0.6 kg Au], variances [8, 2, 0.0008] kg² and dry/Au covariance
0.02 kg² from measured stock drawdown. It explicitly reports unavailable gross-error testing.
`--ambiguous` adds a second unmeasured outlet and refuses before successful stdout. Tests cover
these analytical answers, cross-scale correlated errors, observability, physical/policy rejection,
source/model mutation, missing roles and provenance/confidentiality retention.

`fmine plant infer --input tests/plant/flowsheet_infer.toml` exposes this owner through the shared graph
worksheet parser. The separate schema explicitly names unmeasured streams and supplies covariance only
for observed coordinates. The result retains source Claims, the full joint covariance, observed/inferred
distinctions and the explicit availability of gross-error testing. No missing stock or prior is invented.

## 18. Versioned nominal recovery cards (DESIGN; BD-176/287)

`cards::RecoveryModel::compile` admits four separately sourced layers: a test-work card,
an ordered sequence of operational factors, a correction with an optional explicit upper cap,
and an assumed processing route. Every dependent layer binds the exact test-work Claim ID.
Versions, source claims and intermediate values remain in the immutable model and evaluation
receipt. Updating one source requires compiling a new model; an old receipt retains its original
model and inputs.

A card names its circuit, material domain, dry basis and element, declares its valid period and
exact open or closed input bounds, and assigns each variable both a measurand and a numerical
unit. Supported coordinates distinguish grams per dry metric tonne, troy ounces per dry short
ton, dry mass percent/fraction, dry mass ppm, spike-test percent, micrometres and hours.
Evaluation never silently converts a coordinate or treats spike-test recovery as an assay.
Output scale is separately declared as fraction or percent. Exact bounds are checked before
conversion to binary64, including values whose floating-point representations coincide with a
boundary. Inputs require the same scope, complete declared variable set and period-aggregate
support. Physical input domains, failed calibration, stale or mutated source claims, missing
support and nonfinite or out-of-range results refuse.

The bounded formula compiler accepts arithmetic, parentheses, scientific numeric literals,
`exp`/`EXP`, `ln`/`LN` and integer powers from -16 through 16. It resolves declared variable
names once and uses the existing deterministic scalar numerical owner. The compiled program
evaluates on a fixed stack, without reparsing or allocating an expression tree on each call.
Limits are 4,096 ASCII formula bytes, 512 operations, 32 nesting levels, 32 inputs and eight
operational factors. Unknown names, unsupported syntax, domain errors, overflow and destructive
underflow refuse. This is a nominal scalar calculation with `DeterministicPerIsa` classification;
it is not an interval enclosure or a fitted response surface.

The evaluation receipt exposes the raw test-work fraction, the result after each nonnegative
operational multiplier, the result after the signed additive correction, whether the explicit cap
applied, the final recovery fraction and the selected route. Corrections use fractional units
(`0.02` means two percentage points). A positive raw test-work estimate may exceed one only
when the declared layers produce an admissible final fraction; an explicitly sourced cap can
supply that bound. No implicit clipping occurs. Source evidence, confidentiality, support and
unknown-unknown metadata are combined using the existing plant admission and receipt machinery;
outputs are never promoted beyond `Estimated`.

### Executable source-unit illustrations

```sh
cargo run -p fmine-plant --example process_recovery_cards
cargo run -p fmine-plant --example process_recovery_cards -- --wrong-unit
```

The example retains the Carlin, Cortez and Juniper Mega formula transcriptions already recorded
in corpus C2 section 3.4 (E171), including their different numeric input units and output scales.
Carlin retains its printed `0.029167` factor; this is not substituted for an exact statutory unit
conversion. Cortez explicitly selects the no-arsenic-impact regime. Juniper Mega's logarithmic
expression is interpreted as a fractional output despite its source heading. The original
technical reports have not been requalified by this example. All validity boxes, time intervals,
input claims, operational factors, corrections, caps and routes are authored illustrations,
classified `Assumed`; none establish plant applicability.

The normal command evaluates nine cases before emitting output. The wrong-unit variant refuses
with `FMINE-REF-107` and empty stdout. Thirteen tests include independent 60-digit Decimal
known answers for all three equations, exact boundary and unit/measurand checks, strict spike-test
bounds, four-layer intermediate values, explicit caps, source/version retargeting, metadata
retention, parser bounds, physical domains and mathematical refusal paths.

This completes typed nominal card evaluation, not all of BD-287. Report ingestion, source-box
qualification, parameter estimation, empirical uncertainty/covariance, forecast calibration,
automatic route selection and plant control remain outside this contract.

## 19. Throughput-volume residence time and lag alignment (DESIGN; bounded BD-313)

`rtd::propagate` admits a source-bound, equal-active-tank model, every initial tank inventory,
a declared nonexchanging dead stock and a complete contiguous feed history. Dry matter, water and
declared total elements are extensive channels. The caller explicitly assumes that all channels
share one passive transport response and that feed composition is constant per unit slurry volume
within each interval. Missing initial stocks never default to empty or steady state.

The model supports one through 64 active tanks. Bypass is a fraction of total slurry flow:
`Q_train = (1 - bypass) * Q_total`. Active volume is `(1 - dead_fraction) * V`. At constant
flow, active-path mean residence is active volume divided by actual train flow; the plan's
`V_active / Q` notation therefore uses `Q_train`. Bypass reduces flow through the train as
well as carrying its own immediate outlet. Dead stock remains present but does not exchange.

Propagation advances in cumulative throughput volume. Shutdown retains all stocks; changing the
duration of an interval with the same integrated volume does not silently reuse an old clock-time
kernel. Flow-domain admission applies to each observed interval mean. Integrated volume and elapsed
time do not establish pointwise bounds on unobserved within-interval flow excursions. The propagation
uses cumulative throughput volume under the declared fixed-kernel and constant-composition-per-volume
assumptions.

Positive Erlang/Poisson weights compute both retained inventory and integrated outlet independently.
Outlet is never filled from a balance remainder. All channels reuse the same transition weights.
Long intervals are subdivided into analytic steps of at most 32 single-tank turnovers; global limits
are 4,096 observed intervals and 65,536 kernel steps. Numerical range failure, exhausted work or
cancellation refuses the complete operation without a partial success receipt. At fixed tank and
channel counts the work and receipt storage grow linearly with interval count; the implementation
does not construct a dense whole-history convolution matrix.

Each interval records original feed-source identity, opening stock, incoming masses, bypass,
tank discharge, total outlet, closing stock and independently calculated conservation residuals.
The receipt additionally retains final individual tank stocks, dead stock, whole-window closure,
source claims and the source-envelope digest. Exact source masses are admitted through the existing
flowsheet amount validator, with explicitly bounded nominal conversion and channel-specific closure
allowances. Outputs preserve evidence, confidentiality and unknown-unknown metadata and cannot
exceed `Estimated`. This is `DeterministicPerIsa` nominal transport, not an uncertainty enclosure.

### Separate conditional recovery comparison

`rtd::recovery::compare_recovery` accepts a transport receipt, a source-bound declaration and
complete tail-element observations for its intervals. The declaration must bind that exact
transport Claim and element and state the delay-then-separation hypothesis, complete tail
population and the accounting assumption that all non-tail metal is recovered.

Under that hypothesis, observed tail metal is compared with transported feed-equivalent metal.
The receipt retains both the lag-aligned ratio and the ordinary same-interval ratio, together
with original tail sources and the immutable transport receipt. Zero denominators remain undefined;
a positive tail mass with no corresponding transported metal refuses. Tail mass exceeding aligned
metal also refuses. These transport inventories are passive feed-equivalent inventories; they do
not assert actual reactive CIL gold stocks or infer a spatial recovery process inside the tanks.

```sh
cargo run -p fmine-plant --example rtd_shift_pulse
cargo run -p fmine-plant --example rtd_shift_pulse -- --slow-shifts
cargo run -p fmine-plant --example rtd_shift_pulse -- --outside-flow
```

The GB-41 illustration supplies four independent literal tail observations from a separate
75-digit Decimal integration of the eight-tank Erlang pulse. Same-interval recovery appears as
95.932746025%, 81.703370314%, 76.201535391% and 83.198456772%; lag alignment returns the constructed
88% in every interval. Stretching alternating intervals retains the volume-based answer.
The outside-flow variant refuses with `FMINE-REF-203` and empty stdout.

Eight tests cover independent one-/two-tank solutions and the eight-tank pulse, channel
conservation, physical bypass, dead stock, complete bypass, shutdown, throughput-clock equivalence,
interval composition, source mutation, missing observations, conversion/domain refusal and
bounded work/cancellation. Together with §18 and existing plant tests, the actual plant library
passes 111 tests and all-target Clippy with warnings denied.

This is known-kernel forward transport and conditional lag alignment. RTD fitting, noisy inverse
deconvolution, mixed residence/reaction kinetics, empirical kernel qualification and recovery-card
calibration remain unimplemented. BD-313 remains open.

### Declared residence-time worksheet (DESIGN; BD-313)

`rtd::worksheet::evaluate` exposes §19's forward transport and optional separate recovery comparison
through strict `fmine.plant-rtd.v1` declarations. It is a pure parser/evaluator/renderer; the CLI owns
bounded file reads. `fmine plant rtd --input <worksheet.toml>` accepts one total elemental measurand plus
dry matter and water. Model dimensions, units, flow validity, work limits, every opening tank and dead
inventory, and complete contiguous feed intervals are explicit. Source bytes, their SHA-256, original
numeric tokens and admitted Claims remain in the receipt. A complete optional tail population must bind
the same intervals and declare all delay-then-separation assumptions; any recovery failure refuses the
entire operation without a successful transport receipt. Zero recovery denominators remain undefined.

Inputs remain Assumed, Internal and uncalibrated. JSON retains all three conservation channels, interval
and whole-window residuals, final inventories and both ordinary and lag-aligned recovery. The adapter
introduces no RTD fitting, inverse deconvolution, reactive-stock inference or plant qualification.
`xtask/tests/plant_rtd.rs` supplies independent one-tank and eight-tank pulse checks, bypass and
dead stocks, shutdown, changed clock durations at fixed throughput, atomic refusal and source identity.
The synthetic examples are `tests/plant/rtd_one_tank.toml` and `tests/plant/rtd_shift_pulse.toml`.

## 20. Joint recovery populations (DESIGN; plan §17.4, bounded BD-176/182)

`cards::RecoveryModel::evaluate_ensemble` propagates a `Claim<RecoveryPopulation>` through the
same compiled four-layer recovery model used by nominal evaluation. Each of 1–256 named scenarios
contains an exact positive probability and its original `Claim<RecoveryInputs>`. Every scenario
must supply the complete simultaneous processing-blend coordinates, units and measurands. All
scenarios share the same physical interval and the card's circuit, domain, dry basis and element.
The probabilities sum to one exactly; they are probabilities of alternative states, not weights
for physically blending lots or periods. Their source declaration remains distinct from each
scenario's input source. IDs preserve joint realization identity for downstream joins.

Every member passes the existing exact domain, source, time, version, physical and numerical
admission before any ensemble receipt is returned. One failed member refuses the whole population;
no probability mass is discarded, normalized or replaced with the nominal blend. No independent
coordinate draws, Gaussian approximation or stochastic sampling is inferred. Ordered operational
factors, corrections and explicit caps are applied separately in every realization before moments
are computed; a cap on the mean is not substituted for the mean of capped recoveries.

The immutable `RecoveryEnsembleReceipt` retains the compiled model, original population Claim,
canonical ID-ordered member receipts, result Claim and digest. Every member retains its complete
original inputs, source/version layers, intermediates and cap status through `RecoveryReceipt`.
The typed summary reports the weighted mean, population variance in squared recovery fractions,
minimum, maximum, P10/P50/P90 and exact probability of explicit cap application. Quantiles use the
inverse empirical CDF with exact cumulative probabilities, including exact boundary ties; there is
no interpolation. A nonnegative shifted compensated mean and a centered compensated second pass
avoid cancellation for constant populations and small spreads. Lost numerical range refuses.

The output uses `Representation::EnsembleSummary`, with evidence met across the population and
all member evaluations, confidentiality joined, compatible UU terms retained and knowledge capped
at `Estimated`. Sources and parents remain traceable through every member. Input envelopes and
typed values participate in canonical population identity; declaration permutation preserves
the result Claim and receipt digest. Fixed reduction order makes evaluation `DeterministicPerIsa`.
`to_json()` and `to_markdown()` expose the complete weighted results without printing; the nominal
`RecoveryReceipt::result_json()` is shared by individual and ensemble adapters.

The existing `plant recovery` worksheet accepts optional named scenario probabilities and complete
scenario input sets alongside its nominal processing inputs. Without scenarios its nominal schema
and output remain unchanged. Submitted numeric tokens and exact source bytes remain the CLI
adapter's responsibility; kernel rendering retains exact normalized probabilities and inputs.

Known-answer tests use authored finite populations, not plant measurements. For `R(g)=g²/4` at
`g={0,2}` with equal probabilities, mean recovery is 1/2 and variance is 1/4, while the card at
mean input returns 1/4. For `R(x,y)=xy`, paired `{(0,0),(1,1)}` gives mean 1/2 and anti-paired
`{(0,1),(1,0)}` gives zero despite identical coordinate marginals. With recoveries `{0.6,1.2}`
and an explicit cap of 0.8, mean recovery is 0.7 and cap-event probability is 1/2. Tests additionally
exercise exact weighted-quantile boundaries, very rare states, small nonzero variance, canonical
identity, source metadata, malformed weights and refusal of any incompatible member.

These are conditional probabilities under a user-supplied finite population, explicitly marked
uncalibrated. Neither population completeness in reality nor uncertain card parameters, fitted
likelihoods, empirical recovery calibration, sample-to-blend transfer, forecast registration or
operating permission is established. Authentic source and prospective outcome gates remain open.

## 21. Bond grinding energy and throughput (DESIGN; plan §17.3, PO-CRUC-4; BD-176/181)

`grinding::evaluate` evaluates a versioned `Claim<BondModel>` and 1–4096 complete simultaneous
`Claim<GrindingScenario>` rows at one named boundary, circuit, processing blend and positive interval.
Wi and specific energy use kWh per **dry metric tonne**, F80/P80 use micrometres, installed circuit
power uses kW, utilization is a fraction, and throughput uses dry metric tonnes/hour. Physical public
values use `Qty` dimension/basis types. The card records its work-index test basis, including the
declared closing-screen/feed-preparation basis, and a separately declared validity basis. No
sample-to-blend transfer, metric/short-ton conversion or implicit unit conversion occurs.

The conditional equations are `E = 10 Wi (1/sqrt(P80) - 1/sqrt(F80))` and
`Q = installed_power * utilization / E`. A cancellation-resistant equivalent evaluates the size
term for nearby positive sizes. Wi and sizes must be positive and finite, F80 must exceed P80,
power is nonnegative and utilization lies in [0,1]. All five inputs must also lie in their explicitly
declared closed validity ranges. Zero power and/or zero utilization retain positive specific energy
and return zero throughput with an explicit shutdown reason. Physical failures refuse with
`FMINE-REF-109`, extrapolation with `FMINE-REF-203`, and numerical overflow or lost nonzero values
with `FMINE-REF-207`. There is no numerical clamp or inferred circuit-efficiency factor.

Scenario IDs are unique; exact positive probabilities sum to one without normalization. Each row
contains every simultaneous input of the same processing support, preserving dependence between
hardness, size, power and utilization. These are probabilities over alternative states, not physical
blending weights. Every row must be admitted before a receipt exists. Weighted means are computed
**after** evaluating each row; inverse-CDF P10/P50/P90 use exact cumulative probabilities and no
interpolation. Both specific-energy and throughput distributions retain their extrema; shutdown
probability is exact. Statistics describe the declared population, not confidence intervals.

The immutable `GrindingReceipt` retains the model, canonically ordered source scenarios, every
point result, and the derived summary Claim. Canonical value checks and the shared plant admission
guard reject mutated, stale, failed-calibration or incompatible-support sources. Receipt identity
also binds the input envelope fields omitted from generic Claim hashes, including UU terms and
supersession. Evidence is capped by `ModelledUncalibrated` and met with all inputs, knowledge is at
most `Estimated`, confidentiality joins monotonically, and ancestry/anchors remain available.
The summary's scalar `EnsembleSummary` representation describes throughput; specific energy has
separately typed distribution fields. Evaluation and rendering are `DeterministicPerIsa`.

`grinding::worksheet::evaluate(text, source)` supplies a strict pure adapter for
`fmine.plant-grinding.v1`, exposed by `fmine plant grinding --input <worksheet.toml>`.
It checks the actual UTF-8 source bytes against the supplied SHA-256, enforces a 1 MiB bound,
requires every field and unit declaration, and rejects unknown fields/tables or missing scenarios.
Quoted finite numerical coordinates retain their original tokens in `submitted_worksheet`; exact
weights admit plain decimals or rational strings. Model and row Claims carry the original artifact
and source locator. Inputs remain Assumed and Internal. JSON and Markdown retain every input,
declared domain, source Claim, result, probability, units and digest. No failed member is dropped.

Synthetic examples `tests/plant/grinding_bond.toml` and `tests/plant/grinding_joint.toml` have authored
analytic answers: Wi10/F80400/P80100/power1000/utilization0.8 gives E5 and Q160. Equal states with
Wi5 and Wi20 give mean energy6.25 and mean throughput200; evaluating mean Wi12.5 gives128 instead.
Unit tests also cover joint dependence with unchanged marginals, weighted-quantile ties, shutdown,
near-equal size precision against an independently evaluated Decimal result, size scaling, source
mutation, taint, envelope binding, malformed populations, numerical failure and strict source parsing.

This is a conditional energy-limited estimate. It establishes no actual plant throughput, empirical
validity, grinding-circuit correction, other bottleneck, calibration, frozen forecast, ledger authority
or operating permission. BD-181 still needs actual prospective registration and outcomes; this slice
does not assert that the broader process-card or uncertainty-propagation gates have passed.
