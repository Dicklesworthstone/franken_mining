# CONTRACT.md — `fmine-plant`

## 1. Purpose

DESIGN: Crucible accounts for elemental inventory and declared conservative plant flowsheets across
physical boundaries and intervals (plan §§17.2–17.5; BD-175/177/313). It keeps stocktake timing separate
from integrated flows and refuses unclosed unit or whole-plant balances. This is original code, using
the existing claim-admission pattern; no FrankenSim solver is copied or linked.

- **Crate:** `fmine-plant`
- **Layer:** K2
- **Subsystem:** Crucible
- **LOC budget:** 32,000 planned / 37,000 ceiling
- **Allowed Module Families:** `["inventory", "flowsheet"]`

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

## 7. Evidence class of outputs

The output class is `Derived` met with all six input classes. Knowledge state and epistemic status conjoin;
confidentiality joins; compatible UU scenarios are retained and duplicate identities with conflicting
definitions refuse. Assertions remain assertions with no colour. The composite balance receives no
validated regime or measurement-accuracy claim from successful arithmetic. It is not a decision output.

## 8. Validity domain

One full physical boundary, one declared conserved elemental measurand, one positive-duration interval,
two endpoint stocks and integrated nonnegative nominal mass totals. Element symbols are validated for
case/shape, not looked up or chemically interpreted. The completeness declaration is conditional input;
its truth is not independently audited. Closure is exact, with no measurement tolerance or error-model
adjustment. No sequence continuity, as-of availability or authority-ledger check is inferred.
The flowsheet is restricted to conservative mechanical transfer on the declared dry-matter/H2O basis.
Cycles and held inventory are admitted; reactions, phase changes that alter carrier definitions,
process prediction and estimated recoveries are outside this nominal-accounting interface.

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

BD-175 now has a nominal conservative plant-flow graph; process-model prediction and shared cross-domain
flow adapters remain. BD-177 still needs raw-stream error models and P754 reconciliation.
BD-287 process-card ingestion and BD-313 RTD inversion/deconvolution remain unimplemented
here. No real plant fixture, source-file verification, assay/ore conversion, durable receipt decoder,
CLI importer, forecast registration, sequence authority or permission-to-operate claim is supplied.
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
