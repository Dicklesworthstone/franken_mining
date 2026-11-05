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
- **Allowed Module Families:** `["inventory", "flowsheet", "reconciliation", "measurements", "raw_measurements"]`

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
reconciliation of fully measured carrier and elemental totals, including cross-element and cross-stream
covariance. Nonlinear uncertainty propagation, missing observations, sensor bias estimation, plant
error-model calibration and full P754 reconciliation remain.
BD-287 process-card ingestion and BD-313 RTD inversion/deconvolution remain unimplemented
here. No real plant fixture, independent source verification, durable receipt decoder, site-format importer,
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
Missing measurements, correlated systematic-bias inference, chemical conversion, process prediction,
error-model calibration and empirical P754 qualification require their own supported models.

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
