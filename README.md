# FrankenMining

<div align="center">

[![Status](https://img.shields.io/badge/status-design%20plan%20%C2%B7%20scaffold%20in%20progress-orange)](#honest-status)
[![Runtime](https://img.shields.io/badge/runtime-asupersync%200.5.0-6f42c1)](#oracles-and-pins)
[![Core](https://img.shields.io/badge/core-Rust%202024-b7410e)](#architecture)
[![Plan](https://img.shields.io/badge/plan-485%20beads%20%C2%B7%20E001%E2%80%93E852-2ea44f)](COMPREHENSIVE_PLAN_FOR_THE_DESIGN_OF_FRANKENMINING.md)
[![License](https://img.shields.io/badge/license-MIT%20%2B%20AI%20rider-yellow)](LICENSE)

</div>

**FrankenMining is a system of record for how a mining company knows what it knows: from the first soil sample to
the board's capital decision, every number carries its evidence, its uncertainty and its track record.**

A miner's numbers are produced by a dozen systems that each pass along a single value and discard how sure they
were. The result is predictable:
- guidance that misses;
- reserves that reconcile badly;
- capital projects that overrun by a predictable amount;
- exploration budgets that cannot say what they have ruled out.

FrankenMining is built for integrated gold, copper, silver and tin producers.
- Every published metric is reproduced from cited sources.
- Reconciliation becomes a calibrated attribution of where metal goes.
- Every forecast is frozen before the outcome and scored after.
- Every material capital, reserve or dam decision gets a one-page DecisionCard. It shows:
  - the alternatives;
  - the probability-weighted outcomes;
  - the unknowns that would flip the call;
  - the cheapest way to learn more.

It never classifies a reserve, never signs a dam and never drives a truck. It makes the people who do those
things faster, better evidenced, and accountable to a track record.

It works alongside the systems a miner already runs (the ERP, fleet management, historians, laboratory and geology
tools), reading their exports and replacing none of them. It gives people and AI agents one checked, cited view
of the whole operation. See [Alongside your systems](#alongside-your-systems-and-a-nerve-centre-for-agents).

> **Not affiliated.** FrankenMining is an independent open-source project. It is not affiliated with, endorsed by
> or built for Barrick, Newmont or any other company named here. The demonstration lane uses public disclosures
> only: technical reports, quarterly Mine Stats workbooks and investor presentations, cited by file, page and
> cell.

---

## Honest status

**Status.** The design is written down and has been audited. The code is starting.
- [`COMPREHENSIVE_PLAN_FOR_THE_DESIGN_OF_FRANKENMINING.md`](COMPREHENSIVE_PLAN_FOR_THE_DESIGN_OF_FRANKENMINING.md)
  (about 8,900 lines) is the technical constitution.
- A Rust workspace scaffold (the S0 substrate, the first public-lane crates and the `xtask` layering checks) is
  being built in `crates/`.
- Nothing is *implemented* in the plan's sense (§0.2), which requires all of:
  - a pinned oracle;
  - a filled `CONTRACT.md`;
  - Claim-typed outputs;
  - a robot-CLI entry point;
  - a recorded gate PASS.

  Every capability below is DESIGN or TARGET until its gate says otherwise.

| Question | Answer |
|---|---|
| Is there a design? | **Yes.** §0–§39 plus Appendices A–J. Every load-bearing statement is labelled FACT, DESIGN, HYPOTHESIS, TARGET or OPEN, and every FACT cites an evidence row |
| Is it evidenced? | **Yes, with a register.** [`planning/mor/MOR_CASE.md`](planning/mor/MOR_CASE.md) holds E001–E852. The corpus rows (E101–E399) were re-read by a second reader against the PDFs and workbooks: 273 exact, 18 confirmed with nuance, 3 page-locator fixes, 0 substantive errors. The research rows (E401–E655) still await a second reader |
| Has it been reviewed? | **Five audits and six hypothesis passes.** AUDIT_1–AUDIT_5 covered domain accuracy, suite fit, internal consistency, a corpus second read and a second-wave review of equations, guards and the bead graph. Brenner passes B1–B6 covered subsurface, prospectivity, planning and capital, substrate, wedge and pre-mortem, and operations, plant, tailings and workforce. Their corrections are folded in (plan Appendix H) |
| Is there a work plan? | **Yes.** 21 work packages and 485 beads with 917 dependency edges, acyclic. Instantiated in `.beads/` for `br` |
| Is there code? | **Yes, and the public lane runs.** `fmine assure public --issuer barrick` runs end to end on the 20 registered workbooks through 2026Q3 (not in git), evaluating 2,915 AISC identities and reproducing the source-epoch pin in `repro/barrick/expected.toml`. `--through 2026Q2` retains the earlier 18-source root. Q3 admission and its two source-grounded COS ownership corrections are recorded in E859–E860. The full M0 gate remains open |
| Has anything been benchmarked? | **No benchmark suite.** One machine has timed `fmine-optimize`'s certified ultimate pit: MineLib's 2.1M-block mclaughlin in about 1.2 s, and a synthetic 10⁷-block model in under a minute (`crates/fmine-optimize/CONTRACT.md` §8). Every other speed and scale number in the plan is a TARGET |

### Run declared plant accounting

DESIGN: the plant CLI accepts explicit elemental inventory worksheets and preserves the submitted source
bytes, physical support, original observations and model assumptions in its output:

```bash
cargo run -p fmine-cli -- plant balance --input tests/plant/closed_inventory.toml
cargo run -p fmine-cli -- plant reconcile --input tests/plant/measured_inventory.toml
cargo run -p fmine-cli -- plant trajectory --input tests/plant/trajectory_survey.toml --format markdown
cargo run -p fmine-cli -- plant project --input tests/plant/material_projection.toml
cargo run -p fmine-cli -- plant transfer --input tests/plant/raw_transfer.toml
cargo run -p fmine-cli -- plant flowsheet --input tests/plant/flowsheet_reconcile.toml
cargo run -p fmine-cli -- plant infer --input tests/plant/flowsheet_infer.toml
cargo run -p fmine-cli -- plant infer --input tests/plant/inventory_infer.toml
cargo run -p fmine-cli -- plant recovery --input tests/plant/recovery_card.toml
cargo run -p fmine-cli -- plant production --input tests/plant/production_joint.toml --grinding tests/plant/grinding_joint.toml --recovery tests/plant/production_recovery.toml
```

The supplied examples are synthetic. `balance` checks exact nominal closure. `reconcile` uses the declared full
error covariance and reports fitted amounts, adjustments, posterior covariance and gross-error diagnostics.
Rejected candidates return a nonzero exit status and no admitted balance Claim while retaining the original
observations and available diagnostics. Inputs remain Assumed, Internal and uncalibrated. This calculation
does not audit a site's measurements, infer recovery or certify P754 compliance. `fmine robot-docs` documents
the strict worksheet fields, explicit kg basis, stocktake timing and all required covariance pairs.

`plant production` joins the complete grinding and recovery populations by their shared scenario IDs
and exact probabilities. A third worksheet declares the common physical support, linked recovery grade
and P80 coordinates, and scheduled exposure. The calculation keeps every state paired through dry tonnes,
contained metal and recovered metal before calculating conditional means and quantiles. In the supplied
synthetic case, the mean is 200 dry tonnes, 0.32 kg contained Au and 0.24 kg recovered Au; multiplying the
marginal means would incorrectly produce 0.375 kg recovered Au. The output preserves all three exact
worksheets, source hashes, model receipt identities and Claim ancestry. `--format markdown` shows the
same results. Mismatched populations, weights, scopes or linked model inputs refuse the whole calculation.

`forecast journal register-plant` registers a selected recovery, throughput, dry-tonnage, contained-metal
or recovered-metal target through the existing durable forecast journal. It takes the same three source
paths plus `--journal`, `--head`, `--id`, `--target`, `--author-role`,
`--time-basis utc-unix-ns` and `--trust-host-clock`. The explicit time basis requires whole-second UTC
Unix nanoseconds; the demonstration's relative times must not be treated as future calendar dates.
Registration and subsequent cohort freeze must precede the physical interval's start, while supplied
outcomes cannot be available before its end. The v9 operation retains both bounds and rechecks them on
replay. It records p10/p50/p90 with their exact-weight inverse-CDF interpretation, enabling the existing
80% interval score. It does not claim full-distribution CRPS or calibrated probabilities. Complete source
evidence is retained in the hashed operation; an operation exceeding 64 KiB refuses without advancing the
journal. Existing `freeze`, `outcome` and `assess` commands handle cohort membership, explicitly supplied
observations and scoring. Supplied plant outcomes remain unverified; this is no external timestamp proof
or claim that a real monthly forecasting gate has passed. `fmine robot-docs` documents the fields.

`plant trajectory` links 2–32 contiguous periods for one conserved elemental inventory. Adjacent periods
share one boundary stock; every stock is either measured once or explicitly unmeasured. The declared full
measurement covariance includes cross-period terms, and the output retains the full covariance of fitted
flows and stocks. A later survey can revise earlier stock estimates in a new receipt while the previous
receipt remains unchanged. The synthetic example infers the intermediate stock as 14.5 kg after admitting
the terminal survey; without that survey it is 12 kg. With only one observed stocktake there is no redundant
gross-error test, which is reported explicitly. This bounded linear estimator supplies no sensor-bias,
stockpile-mixing or hierarchical R2 model.

`plant project` converts explicit dry/wet masses, wet-basis moisture and total dry-basis assays into exact
material and elemental kilograms. It preserves the submitted values and units. It requires all measurements
to cover the same declared whole material; sampled assays and chemical species are not silently promoted to
total-material measurements. The nominal conversion does not invent an uncertainty model or close a balance.

`plant transfer` aggregates a declared complete lot population from wet kg, wet-basis moisture and dry-basis
elemental assays. It forms each lot's exact dry, water and elemental mass before aggregation, then propagates
explicit standard uncertainties and the full raw correlation matrix, including cross-lot terms. The example
yields 250 kg dry matter, 50 kg water and 4.1 kg Cu, with a weighted dry grade of 1.64%. Output preserves the
source Claims, exact totals, full first-order covariance and a parent-linked nominal graph transfer. The
first-order covariance remains distinct from a declared Gaussian reconciliation model.

`plant flowsheet` fits all declared unit balances together, with each internal transfer shared by its two
endpoints. Its worksheet explicitly supplies the graph, every stream and stocktake coordinate, every covariance
pair and the error policy. It retains cross-stream and cross-element covariance and independently audits the
whole boundary. The Rust owner is `fmine_plant::flowsheet::reconciliation::reconcile_flowsheet`.
Run its synthetic mill/classifier
example with `cargo run -p fmine-plant --example reconcile_flowsheet`; adding `-- --gross-error` demonstrates
rejection. This fully measured conditional model preserves source evidence and does not infer missing
measurements, sensor calibration or process performance.

`plant infer` admits explicitly declared unmeasured streams and, with the v2 worksheet schema, an unmeasured
opening or closing stocktake per unit. It uses the existing partial reconciliation owner to infer uniquely
identifiable flows and inventories, retaining full measured/inferred covariance and distinguishing observed
contributions from complete totals. A shared transfer's uncertainty can correlate stocks across units.
Ambiguous splits, unmeasured cycles or both endpoint stocks unknown refuse. A unique inference with no
redundant observations is labelled `no_redundancy`, with no gross-error statistic or claim that a consistency
test passed. Existing v1 worksheets retain their stream-only behavior.

`plant recovery` evaluates an explicitly declared recovery formula through the existing versioned model-card
engine. Its worksheet names the circuit, material domain, dry basis, element, validity bounds and processing
support, plus the test-work formula, ordered operational factors, correction, optional cap and assumed route.
Each input repeats its unit and measurand; no conversion or extrapolation is inferred. The output retains the
submitted source hash, exact bounds, original tokens, source Claims and every intermediate recovery fraction.
The synthetic example gives 0.8 from the formula, 0.7752 after its two operational factors, 0.7952 after the
correction and 0.79 after the explicitly declared cap. These are conditional nominal calculations with assumed,
uncalibrated inputs; they do not qualify a model for an actual plant or authorize an operating change.

Optional `[[scenario]]` rows add complete **joint alternatives** with named IDs and exact positive weights
that sum to one. Corresponding `[[input]]` rows use `scenario = "<id>"`; ordinary input rows continue to
define the nominal comparison. Each alternative must supply every coordinate on the same simultaneous
processing support and interval. The engine compiles the formula once, evaluates every alternative through
all layers and the explicit cap, and retains each result and source-bound receipt. The added ensemble
reports its weighted mean, population variance, exact cap probability and inverse-CDF p10/p50/p90;
quantiles use exact cumulative weights without interpolation. It refuses an invalid member rather than
dropping it or normalizing the remaining population.

```bash
cargo run -p fmine-cli -- plant recovery --input tests/plant/recovery_scenarios.toml
```

In this synthetic nonlinear example, `R(Au)=Au²/4`, nominal grade **1** gives recovery **0.25**, while
equally weighted grades **0 and 2** give mean recovery **0.5** and variance **0.25**. Joint scenarios preserve
declared coordinate dependence; the engine does not substitute marginal means or draw independent values.
These are conditional calculations under supplied scenarios, with no fitted distribution, model-error
estimate, empirical probability calibration or forecast registration.

### Compare transient feed and tail observations

A declared residence-time model can now be run from a worksheet:

```sh
cargo run -p fmine-cli -- plant rtd --input tests/plant/rtd_one_tank.toml --json
cargo run -p fmine-cli -- plant rtd --input tests/plant/rtd_shift_pulse.toml --format markdown
```

The strict `fmine.plant-rtd.v1` input names the tank train, bypass and dead volume, flow validity,
numerical work limit, every opening inventory and the complete feed intervals. Transport follows
integrated slurry volume. The report retains source bytes and their SHA-256, admitted Claims, dry-matter,
water and elemental conservation, residuals and final inventories.

An optional complete tail population permits a conditional recovery comparison when all
delay-then-separation assumptions are declared. The report keeps ordinary same-interval recovery
separate from lag-aligned recovery; a zero denominator stays undefined. The eight-tank synthetic example
constructs 88% lag-aligned recovery in each interval even though the ordinary ratios differ. Changed
clock durations with the same integrated throughput preserve the transport result within the declared
flow-validity domain.

These are assumed inputs and a known forward kernel. No residence-time fitting, inverse deconvolution,
reactive-stock inference or plant calibration is supplied. Invalid optional recovery observations refuse
the whole operation without a successful transport receipt.

### Reconstruct delayed feed with uncertain opening stocks

The separate `plant rtd-infer` command reconstructs explicitly unmeasured elemental feeds from integrated
outlet observations under a declared transport model:

```sh
cargo run -p fmine-cli -- plant rtd-infer --input tests/plant/rtd_inverse.toml --json
cargo run -p fmine-cli -- plant rtd-infer --input tests/plant/rtd_inverse_opening.toml --format markdown
```

DESIGN: the v1 worksheet fixes initial stocks. The v2 worksheet instead declares every active tank's
opening elemental stock as a measurement, with its variance and all opening/flow correlations. Opening
stocks, measured feeds and outlet masses are fitted jointly; absent feeds receive no invented observation
or prior. Dead stock and transport parameters remain fixed. Both modes keep original source Claims,
the full joint covariance, source bytes, adjustment diagnostics and independently replayed conservation.

The synthetic measured-opening example starts with a **2 kg** stock measurement and reconstructs
**10 kg and 4 kg** interval feeds. Its declared correlated errors give feed variances of approximately
**0.846854 and 0.886953 kg²**, including the opening-stock uncertainty. These known answers are checked
against independently derived one-tank equations in `rtd::inverse::tests` and `xtask/tests/plant_rtd_inverse.rs`.
The two unknown feeds consume all available redundancy; the report explicitly has no gross-error test.
Negative fitted stocks or flows and failed diagnostic limits retain investigation details without an
admitted result. These conditional calculations do not fit the kernel, identify sensor bias, qualify a
real plant or establish calibrated recovery.

### Calculate grinding energy and throughput across joint scenarios

`plant grinding` evaluates a declared Bond model at processing-blend support:

```sh
cargo run -p fmine-cli -- plant grinding --input tests/plant/grinding_bond.toml --json
cargo run -p fmine-cli -- plant grinding --input tests/plant/grinding_joint.toml --format markdown
```

The worksheet declares the circuit, blend, interval, model version, work-index test basis and closed
validity bounds for every input. Work index and specific energy use **kWh per dry metric tonne**;
F80/P80 use microns, power uses kW and utilization is a fraction. The calculation is
`E = 10 Wi (1/sqrt(P80) - 1/sqrt(F80))`, followed by `Q = power × utilization / E`.
The single-state synthetic example gives **5 kWh/dry tonne** and **160 dry tonnes/hour**.

For uncertainty, supply complete joint scenario rows with positive exact weights summing to one.
Each row keeps its paired hardness, sizes, power and utilization. The report retains every row result,
weighted means, inverse-CDF P10/P50/P90, source Claims and the original worksheet bytes. The equal-weight
Wi=5/Wi=20 example gives throughputs 320 and 80, hence a mean of **200 dry tonnes/hour**;
plugging mean hardness into the formula would instead give 128. These scenarios describe alternative
states of the blend, not a physical ore-blending rule.

Zero power or utilization produces explicit zero-throughput shutdown. Invalid units, incomplete weights,
nonfinite values, nonpositive energy or any coordinate outside the declared validity domain refuse the
complete calculation. The inputs, domain and scenario probabilities remain assumed and uncalibrated.
The model supplies conditional power-limited throughput, without qualifying circuit efficiency, other
plant bottlenecks, measured capacity or operational authority.

### Run the public lane

With the pinned Rust toolchain, fetch the registered public workbooks and run:

```bash
bash repro/barrick/fetch.sh
cargo run -p fmine-cli -- assure public --issuer barrick > run.json
```

**FACT, the registered-corpus integration tests:** the Mine Stats Claim inventory contains 22,732
observations, with no mapping refusals. Each Claim retains its publication vintage, period, ownership basis,
unit, raw token, displayed value and source cell/hash. Numeric zeros at registered disposed sites remain
`nil` with reason `disposed`; missing cells, dashes and `n/a` remain distinct.

The JSON payload includes `public_claims.cells` (the canonical AISC and Mine Stats Claims), `mine_stats_claims`
(the Mine Stats census and digest), and `issuer_consistency.mine_stats_vintages` (counts and cited comparisons
against both the earliest registered and previous releases). The comparison tests
reproduce E474 and all eight W1-DR-09 changes, while listing ownership conflicts and changed regional membership
separately. The comparison population reconciles with W1 T16: 6,391 passes plus nine changes, with 30
regional-membership changes listed separately. The nine changes are classified as `restated_silent` because
no applicable verified note is registered. All 30 region-map exclusions cite E461's exact workbook note and
remain `not_comparable`. A difference does not identify which publication is correct, and this classification
does not establish whether an explanation exists outside the registered evidence.

The same engine compares the AISC workbooks under `issuer_consistency.aisc_vintages`. Its frozen original
population through 2026Q2 contains 9,541 appearances, 5,819 series and 3,722 later comparisons, including
359 explicitly labelled co-product cash-cost rates. The original census remains pinned separately from
newly admitted rate families and later source vintages.

Explicitly printed **gold co-product AISC** is now also admitted as a distinct USD/oz source Claim.
An independent visible-OOXML read identified 359 additional source cells across 22 sheets in five registered
workbooks. Stored precision, displayed rounding, reporting vintage, original source hashes, unprinted
ownership and status-bearing zeros are preserved. Cited and discovery checks require the co-product
qualifier and keep co-product ownership comparisons separate from by-product ones. The direct printed
rate provides the evidence; no credit add-back or co-product numerator is inferred.

**DESIGN (BD-504e):** `issuer_consistency.mine_stats_identities` also follows the seven Mine Stats
arithmetic identity families across releases. It shows state counts at each identity's earliest registered
appearance beside the all-file counts, then reports whether each later appearance persisted, was corrected,
became broken or was reclassified. The comparison follows semantic keys through moved spreadsheet cells;
ownership, units and region-map versions remain separate. Missing or ambiguous source keys stay listed and
cannot prove persistence or correction. Counts retain not-published and refused instances. Each report
carries its own digest and cited source Claims; the existing assurance root still binds the underlying
identity ledger and registered workbook bytes. Earliest registered evidence does not establish first
publication, and these findings do not supply an issuer pass score.

Disclosure coverage comes from explicit `restatement_note` and `restatement_scope` registry rows. Each run
checks the source digest and complete quoted cell text, then matches exact subjects, lines, occurrences and
whole reporting periods. A future note cannot explain an earlier release. Missing, altered or unusable note
evidence leaves a change unassessed rather than silently declaring it unexplained. The JSON reports source
Claims, applicable note citations, the claim-population digest and the checked disclosure-registry digest.

Inspect a comparison's `current[].claim_id` or `first[].claim_id`, then resolve it directly:

```bash
cargo run -p fmine-cli -- claim show clm_REPLACE_WITH_THE_EMITTED_ID
cargo run -p fmine-cli -- claim why clm_REPLACE_WITH_THE_EMITTED_ID
cargo run -p fmine-cli -- claim trace clm_REPLACE_WITH_THE_EMITTED_ID
```

All three views recheck the registered source hashes and refuse missing or altered inputs. `show` returns
the complete source Claim; `why` explains its evidence, knowledge state, colour and actual admission guards;
`trace` follows its source anchors to the registered workbook, cell, URL and digest. Assessments that the
public projection has not made, including clock freshness and flip margin, remain explicitly absent.
Exact Claim IDs emitted by the built-in synthetic run also resolve, retaining their synthetic evidence.
These additions do not constitute a pass of the full M0 gate.

To resolve a Claim against a particular assurance snapshot, use the same issuer and release bound:

```bash
cargo run -p fmine-cli -- claim show clm_REPLACE_WITH_THE_EMITTED_ID --issuer barrick --through 2026Q2
```

For workbook Claims, the response includes both the assurance root and the keyed Claim projection digest.
The same explicit issuer selection also resolves the recovery report's ownership, training, fitted-model,
operating-input and projection Claims. `show` retains the typed value and canonical body; `why` runs the
recorded Claim guards; `trace` follows the retained parents through the model to the workbook or agreed PDF
source anchors. Missing parents remain explicit. Recovery reads carry their separate model-report digest;
their envelope has no identity-ledger certificate or issuance anchor. The source identity root is identified
separately and does not certify the model. Missing IDs and altered source files refuse instead of returning
a stale cached Claim. Recovery results remain descriptive and uncalibrated.

### Read the public assurance report

Use the same public-lane selection with `--format markdown` to produce a readable report:

```bash
cargo run -p fmine-cli -- assure public --issuer barrick --through 2026Q2 --format markdown > assurance.md
cargo run -p fmine-cli -- assure public --synthetic --format markdown > synthetic-assurance.md
```

The report presents exact printed values and all selected publication vintages by subject, reporting
period, scope and ownership basis. It retains the five identity states, with not-published instances
listed separately, and includes evidence-challenge packets with their operands, certificates and the
documents needed to resolve each visible exception. Consistency histories show original and comparative
filings, later corrections, verified disclosure notes and explicit comparability exclusions.

Source cells, document hashes and Claim IDs remain available in the report. Hidden/template scope has a
separate appendix. PDF transcriptions, recovery-model outputs and any published-cutoff reference
calculations carry their own evidence boundaries; they do not enlarge the identity-ledger certificate.
The JSON envelope and state root are the same for either presentation. A synthetic report remains
ineligible for a public-lane gate, and a readable report does not turn the current implementation into
a complete M0 gate pass.

### Compare registered AISC aggregation layers

DESIGN (BD-273): the same `assure public` run includes `aisc_component_comparisons` in JSON and an **AISC component
comparisons** section in Markdown. For explicitly registered region and subtotal memberships, it compares
the aggregate's printed USD-million component rows with every selected member in the same workbook table.
The report lists each named difference, the reported numerator and sold-quantity gaps, each column's
arithmetic footing, and any unexplained numerator remainder. Exact display-rounding bounds, original source
Claims, file hashes, cell locators and membership policies remain available alongside a separate report digest.

The report also compares the gold reconciliation's printed company `Total` with its declared regional
population at the same release and reporting period. Company amounts and rates use canonical `Total Gold`
Claims while retaining the printed header, source cells and unknown ownership where no share is stated.
Missing, stale, overlapping or ambiguous membership leaves this comparison unavailable, including two
subtotals that contain the same mine. Company observations also remain inspectable across filing vintages;
they do not enlarge the existing identity-check denominator.

The same comparison also explains the recomputed **USD/oz gap**. Each component difference and any
unexplained numerator remainder is divided by the aggregate's attributable ounces sold; a separate sales
denominator term retains any difference between aggregate and combined member sales. The terms sum exactly
to the gap between the two recomputed rates, and every term carries exact rounding bounds. For example, an
invented subtotal of $850 million on 100 thousand ounces versus members totalling $820 million on the same
sales gives a $300/oz gap; a $30 million G&A difference accounts for that gap arithmetically. These recomputed
rates are distinct from the issuer's printed per-ounce rates. A sales interval reaching zero refuses the
per-ounce calculation while preserving the usable dollar comparison.

Missing members, partial-period membership, incompatible source semantics and unlabelled numeric components
leave the comparison unavailable; cancelling omitted rows cannot make it pass. Printed arithmetic closes
only when the reported numerator is explained by the named differences, every column foots, and the printed
sold quantities agree. These checks never establish accounting reconciliation. Differences are not assigned
to corporate or JV costs, and ownership is not rescaled. Company-level reconciliation needs separately
source-bound company reconciliation evidence beyond the selected regional population. These comparisons
do not establish complete company coverage or complete BD-273 or M0.

### Calculate a cost metric under an explicit definition

`metrics cost` evaluates a declared cost worksheet under a selected AISC or AIC definition from
[`registries/metrics.toml`](registries/metrics.toml). Run the included invented example:

```bash
cargo run -p fmine-cli -- metrics cost --input tests/costs/declared_aisc.toml --definition aisc@wgc-2018 --format markdown
cargo run -p fmine-cli -- metrics cost --input tests/costs/declared_aisc.toml --definition aisc@wgc-2018 --json
```

The [example worksheet](tests/costs/declared_aisc.toml) declares USD 800.01 of mining costs, 300 of corporate
costs, 100 of lease payments and 200 of by-product credits, with 100 gold ounces sold. The selected definition
produces a **USD 1,000.01 numerator and USD 10.0001 per ounce sold**. Its USD 999 income-tax line remains
visible as excluded. Each input receives its inclusion, exclusion or credit treatment from the selected
definition; the receipt also retains inherited rules and any registered successor.

The worksheet uses schema `fmine.cost-worksheet.v1`, explicit issuer, subject, period, ownership, basis and
currency, `complete = "true"`, and `ounces_sold`. Each `[[cost]]` names a registered `item` and quoted `usd`
amount; `[[uu]]` rows retain named unknown-risk scenarios, their triggers and downside amounts. Monetary
inputs use major USD units with at most four decimal places. Quantities and shares are exact decimals or
fractions. The calculation does not pass through floating point, and JSON retains exact rational results
and complete canonical input/output Claims, bound to the actual worksheet and metric-registry hashes.

Planned costs can instead declare physical quantities and USD rates in `[[driver_cost]]` rows. Each row
computes **quantity × rate + fixed cost**, then enters the same definition and allocation calculation as a
direct cost. The [planned-driver example](tests/costs/planned_drivers.toml) replaces the USD 800.01 mining
line above with:

```toml
[[driver_cost]]
item = "on_site_mining_costs"
driver = "tonnes_mined"
unit = "dry_t"
quantity = "100.01"
usd_per_unit = "7.5"
fixed_usd = "49.935"
```

Run it with `fmine metrics cost --input tests/costs/planned_drivers.toml --definition aisc@wgc-2018`.
The result is the same USD 10.0001 per sold ounce, with the declared operands retained in the receipt.
Supported driver/unit pairs are `tonnes_mined` or `tonnes_milled` with `dry_t` or `wet_t`, `haulage` with
`dry_t_km` or `wet_t_km`, `electricity` with `kWh`, `reagent` with `kg`, and `labour` with `hour`.
Quantities and rates must be nonnegative exact decimals or fractions; `fixed_usd` is nonnegative and defaults
explicitly to zero. A computed amount that cannot be represented exactly in USD 0.0001 units refuses.
There is no implicit unit conversion or rounding. Direct and driver rows cannot repeat the same registered
item. These are supplied planning assumptions, with source locators for every operand; no rate is fitted from
historical accounts or promoted to calibrated evidence.

The worksheet's cost classification and completeness remain **Assumed, Internal and uncalibrated**. Declaring
a complete population is not independent evidence that omitted costs are zero. This command calculates the
chosen rule; it does not verify issuer filings or close the broader metric-reproduction work.

The same command accepts explicit copper pounds sold under issuer-specific C1 and AISC table rules:

```bash
cargo run -p fmine-cli -- metrics cost --input tests/costs/declared_copper.toml --definition c1.copper@barrick-q2-2026 --format markdown
cargo run -p fmine-cli -- metrics cost --input tests/costs/declared_copper.toml --definition aisc.copper@barrick-q2-2026 --json
```

The invented copper example uses 400 pounds sold and produces a C1 numerator of USD 980
(USD 2.45/lb) and an AISC numerator of USD 1,200 (USD 3/lb). Depreciation and the C1 royalty removal
are reconciliation deductions, distinct from by-product credits; the AISC royalty add-back remains
its own named input. Both calculations retain every excluded cost. These versioned rules describe
the disclosed table's lines; unlisted inventory or nonroutine adjustments refuse.

Specify exactly one of `ounces_sold` (gold) or `pounds_sold` (copper), in actual sold units.
The engine refuses an ounce/pound mismatch, a produced-unit substitution, ambiguous denominators
and an allocation for another metal. Values and labels remain declared assumptions; the example
does not reproduce a rounded issuer rate or establish an audited accounting population.

### Compare production GEO across named price decks

`metrics geo` applies each versioned price deck to the same declared production quantities, showing each
metal's contribution and the exact change from the named reference. Run the included invented example:

```bash
cargo run -p fmine-cli -- metrics geo --input crates/xtask/testdata/production_geo.toml --format markdown
cargo run -p fmine-cli -- metrics geo --input crates/xtask/testdata/production_geo.toml --json
```

The [GB-27 example worksheet](crates/xtask/testdata/production_geo.toml) holds gold production at 3,200,000 oz
and copper production at 440,000,000 lb. Both decks price copper at USD 4.50/lb:

| Deck / version | Gold price, USD/oz | Copper contribution, GEO oz | Total GEO oz | Difference from reference, oz |
|---|---:|---:|---:|---:|
| budget / example-v1 | 3,000 | 660,000 | 3,860,000 | 0 |
| comparison / example-v1 | 4,500 | 440,000 | 3,640,000 | −220,000 |

Schema `fmine.geo-worksheet.v1` requires explicit issuer, subject, period, ownership, physical basis,
population, currency, included metals and reference deck/version. Each `[[quantity]]` declares `metal`,
`unit` and `amount`; each `[[deck]]` declares `id`, `version`, `basis`, `currency` and `gold_price_per_oz`,
with `copper_price_per_lb` and `silver_price_per_oz` required when those metals are included. Values are
quoted strings; amounts are exact decimals or fractions. Gold and silver use oz, copper uses lb, and
prices are positive major currency units per stated metal unit. Two to sixty-four distinct deck/version
pairs must share the declared currency. Unknown, duplicate or missing fields refuse. Input files are
bounded to 1 MiB; no mass, ownership or FX conversion is implicit.

The receipt retains complete input/output Claims, the submitted file's SHA-256, field locators and a
deterministic digest. Imported inputs remain **Assumed, Internal and uncalibrated**. Omitted metals are
outside the declared population, not independently verified zero. The differences above measure price-ratio
effects on unchanged physical quantities; they establish neither physical production change nor source
accuracy, reserve classification or disclosure authority.

### Solve the ultimate pit of a MineLib UPIT pair

`plan pit` reads a block-value file and a precedence file in MineLib's UPIT format, solves the maximum-value
closed pit by pseudoflow, and checks the solver's certificate again before it answers:

```bash
cargo run --release -p fmine-cli -- plan pit --upit kd/kd.upit --prec kd/kd.prec --format markdown
cargo run --release -p fmine-cli -- plan pit --upit marvin/marvin.upit --prec marvin/marvin.prec --round --list-blocks --json
```

MineLib's files are not in this repository, because their licence is not recorded; download them from MineLib
and pass their paths. Values are scaled to integers at `--decimals` places (default 6, at most 9), so the
reported value is exact at those places. A value with more non-zero places refuses unless `--round` is given.
`--round` rounds half away from zero, counts the values rounded, and reports the bound on the effect: half a
unit per rounded value. Malformed files refuse with the line at fault; nothing is defaulted. The receipt binds
both files' SHA-256 hashes.

On the eleven MineLib UPIT instances, the verb's values equal the GATE-PUSH-PIT1 run
([docs/delivery/GATE-PUSH-PIT1.md](docs/delivery/GATE-PUSH-PIT1.md)), and each rounds to MineLib's published
optimum. The values and arcs are taken as given: no price, cost, recovery or slope is derived or checked, and
the pit is not a reserve, a schedule or a mine plan.

`--candidate <file>` checks a pit from another tool, such as Whittle, Deswik or a MIP solver, against the
certified optimum. The file lists the candidate's block ids. The verb recomputes the candidate's value itself,
counts every unmet precedence requirement (and names the first ten), and reports the exact gap to the optimum
with a verdict: `not_closed`, `optimal` or `suboptimal`. Nothing the candidate says about itself is read. On
MineLib's kd, for example, the optimal pit without its highest-numbered block is `suboptimal` by 132,781.5934, and
with block 0 removed it is `not_closed`, because eight pit blocks require block 0.

### Solve an ultimate pit directly from an OMF block model

`plan pit` also connects the OMF 2 reader to the certified pit solver. Select the regular parent-block
model and its numeric value attribute, and declare the integer value scale and uniform slope cone:

```bash
cargo run --release -p fmine-cli -- plan pit --omf model.omf \
  --block-model orebody --value-attribute value --per-unit 100 \
  --slope-rise 1 --slope-run 1 --slope-levels 3 --round --list-blocks --json
```

All selectors, scale and slope parameters are required. `--round` explicitly accepts binary64 scaling
followed by nearest-integer rounding, with halves away from zero. The certificate proves the optimum of
that **scaled integer model**; it does not prove the optimum of the original unrounded attribute.
The receipt binds the actual OMF bytes by SHA-256 and records the coordinate context, selected attribute,
declared slope, scale, exact scaled value and verification result. Listed block IDs follow OMF order:
U fastest, then V, then W from the bottom upward.

This path accepts regular parent grids with W pointing straight up. It refuses ambiguous model or
attribute names, missing/null values, unsupported geometry and mixed MineLib options. The finite-depth
uniform slope is an explicit modeling assumption; sector-specific slope tables and external OMF
candidate pits remain outside this command. OMF period scheduling is available below. The supplied values
are not independently
derived from prices, costs or recoveries, and the result supplies no reserve classification or slope approval.
The integration tests independently enumerate all closed sets for 24 small authored OMF models, check
source perturbations and signed rounding, and exercise the built executable; these are synthetic checks.

### Generate and check production schedules with destination splits

`plan schedule` makes the existing PCPSP scheduling and bound solvers available through the CLI.
It reads MineLib block/destination values, precedence arcs and per-period resource limits, constructs
a whole-block schedule, improves its periods and destinations, then tries profitable fractional
destination allocations at fixed mining periods. Every selected schedule is independently checked.
A global Lagrangian upper bound is recomputed from the returned prices over **every model
block**, including negative-value material that can enable a blend.

DESIGN (plan §15.4–15.6): when separate period and destination improvements stop, whole-block
search also considers changing both together. A profitable alternative route can move into an earlier
period even when the current route cannot fit there, and an unmined block can enter through a feasible
alternative route. Every accepted move checks predecessor and dependent periods and all signed resource
limits in both the departing and arriving periods before changing the schedule.

```bash
cargo run --release -p fmine-cli -- plan schedule \
  --pcpsp tests/planning/joint_route.pcpsp --prec tests/planning/joint_route.prec \
  --list-schedule --format markdown
```

The synthetic example starts with a route worth **10** in period 1, discounted to **5**. Its capacity
requirement prevents moving that route to period 0, while switching routes in period 1 would lose value.
A joint move selects the alternative route in period 0 for **6**. `--rounds 0` retains the initial value
of **5**. These bounded local moves improve the checked incumbent; the reported upper bound and gap
still determine what is known about optimality.

```bash
cargo run --release -p fmine-cli -- plan schedule \
  --pcpsp tests/planning/split.pcpsp --prec tests/planning/split.prec \
  --candidate tests/planning/split.sol --list-schedule --format markdown
```

The example is invented arithmetic: a half-dump/half-mill allocation gives period values **12 and 21**,
total **33**, while satisfying a signed blending row exactly. An outside candidate uses four columns:
`block destination period fraction`, with zero-based indices and `%` comments. Fractions never round;
each mined block must total exactly one, and every predecessor must be completely mined in the same
or an earlier period. Whole-block candidates can seed further search. A better feasible split candidate
is retained exactly; the solver does not silently round its destinations.

LP guidance can generate additional mining sets before local search and fractional polishing:

```bash
cargo run --release -p fmine-cli -- plan schedule \
  --pcpsp tests/planning/lp_guided.pcpsp --prec tests/planning/lp_guided.prec \
  --lp-guidance --list-schedule --format markdown
```

In this synthetic capacity trap, the default greedy start chooses a block worth **10** that uses all
capacity. LP-guided starts find two blocks worth **18** together. The relaxation supplies six bounded
threshold/release starts, reusing duplicates. Every retained schedule passes exact feasibility and
value checks, and only a strict improvement replaces the existing baseline or external split.
The final upper bound still comes from the full-model integer Lagrangian replay.

`--lp-iterations` requires `--lp-guidance`, defaults to 100,000 and accepts 1–1,000,000. The PDLP solve
uses one worker and tolerance `1e-6`. An iteration-limited point may guide search only after finite
`[0,1]` share admission; its row residuals or floating objective never certify a schedule. JSON records
termination, iterations, problem size, generated/searched starts, polished seeds and exact guided value;
the selected schedule records its origin. Explicit size limits refuse excessive LP expansion before
allocation. Omitting `--lp-guidance` retains the existing command behavior and output shape.

Fractional allocations can also be generated without an outside candidate:

```bash
cargo run --release -p fmine-cli -- plan schedule \
  --pcpsp tests/planning/fractional.pcpsp --prec tests/planning/fractional.prec \
  --list-schedule --format markdown
```

This synthetic model earns **5** by processing half a block where the best whole-block choice earns **0**.
The bounded pricing pass proposes destination columns; exact integer interpolation takes the largest
feasible step at the declared share precision and admits only strict value improvements. It checks
every signed resource row, so fractional rounding never buys an infeasible increment. Both the initial
and improved whole schedules are considered, retaining fractional opportunities in material that
whole-block search would otherwise discard. Admitted external whole schedules and their improved
versions can also seed fractional allocations, including mining sets the internal start cannot construct.
This is a heuristic over bounded pricing directions;
it does not claim an optimal fractional destination assignment or alter mining periods during polishing.

The JSON reports the input hashes, decimal scales, rounding counts, exact rational discounted value,
resource usage, candidate infeasibility witnesses, bound prices and gap. `--list-schedule` includes
canonical solution text that the same reader accepts. Structural candidate errors refuse. A structurally
valid but infeasible candidate remains a reported finding and cannot become the selected schedule.

Value and resource scales default to six decimal places; `--decimals`, `--use-decimals` and
`--share-decimals` accept 0–9. Shares default to nine places. Values with excess precision refuse unless
`--round` is requested; resource uses round up and limits down. `--rounds` defaults to 20 and
`--bound-steps` to 64. Period-zero discount is `2^40`, and later integer factors follow the disclosed
floor recurrence. All reported monetary comparisons use those same factors.

The certificate covers the supplied deterministic model and rounding convention. A positive gap is
distance to an upper bound, not proof that the incumbent can be improved by that amount. General side
constraints, lower/equality resource limits, stochastic sampling error and empirical input calibration
remain outside this slice. The independent Python Fraction oracle checks the actual binary against the
example and 16 generated models, including exhaustive feasible schedules and independent dual-price
replay; these are known answers, not operating-mine validation.

### Compare OMF pit shells across revenue factors

`plan shells` solves a nested family of pit limits from explicitly supplied revenue and cost attributes.
It checks each scaled integer optimum and verifies that every shell contains the preceding shell.
The OMF archive and the declaration both retain their SHA-256 identities in the result.

```bash
cargo run --release -p fmine-cli -- plan shells \
  --omf model.omf --spec shells.json --list-blocks --json
```

For numeric parent-block attributes `revenue` and `cost` with the same nonempty unit label on a regular
block model named `orebody`, `shells.json` can contain:

```json
{
  "schema": "fmine.omf-shells.v1",
  "block_model": "orebody",
  "revenue_attribute": "revenue",
  "cost_attribute": "cost",
  "per_unit": 100,
  "value_rounding": "nearest_integer_half_away_from_zero",
  "slope": {"rise": 1, "run": 1, "levels": 3},
  "factors": [{"num": 1, "den": 2}, {"num": 1, "den": 1}, {"num": 3, "den": 2}]
}
```

Every field is required. Factors are positive exact integer fractions, strictly increasing as written;
the command does not sort or deduplicate them. Revenue and cost are separately scaled and rounded to the
nearest integer, halves away from zero. At factor `num/den`, the reported objective is exactly
`sum(num * scaled_revenue - den * scaled_cost) / (per_unit * den)`. Scaled revenue must be nonnegative;
signed costs are supported. Different or missing unit labels refuse, and no conversion is inferred.

The report gives each shell's exact objective, total blocks and newly added blocks. `--list-blocks` adds
`first_shell`: one zero-based shell index per original OMF block, or null if absent at every factor.
Block order is U, then V, then W from bottom upward. The independent actual-binary oracle enumerates
small synthetic models, including tied optima, signed costs and exact rational factor boundaries.

Certificates cover the declared scaled integer model and finite-depth uniform slope cones. The existing
solver checks flow witnesses internally; this report does not export them. Shells are mathematical pit
limits, with no designed-pushback, reserve, approved-slope, raw-float optimum or execution claim.

### Compare an engineered pit design with its optimized shell

`plan design` measures the value and material changes between a closed OMF design surface and the
certified ultimate-pit shell of the same block model. It reports what the design leaves outside, what it
adds beyond the shell, and their signed difference, with geometric uncertainty retained as intervals.

```bash
cargo run --release -p fmine-cli -- plan design \
  --omf model.omf --spec design.json --list-blocks --json
```

For a regular block model named `orebody` and an engineer's closed surface named `designed_pit`, an
explicit declaration can be:

```json
{
  "schema": "fmine.omf-design.v1",
  "block_model": "orebody",
  "value_attribute": "value",
  "value_per_unit": 100,
  "value_rounding": "nearest_integer_half_away_from_zero",
  "surface": "designed_pit",
  "depth": 2,
  "slope": {"rise": 1, "run": 1, "levels": 3},
  "quantities": [
    {"attribute": "dry_tonnes", "per_unit": 10},
    {"attribute": "contained_gold_oz", "per_unit": 1000}
  ]
}
```

Each selected attribute must be a complete parent-block quantity with an explicit nonempty unit label.
The command does not derive metal from grade, convert wet to dry tonnes, or infer a valuation model.
It scales the declared block totals and assumes each is uniformly distributed within its block.
Every reported endpoint is an integer numerator over `per_unit * 8^depth`, so signed costs and material
fractions retain exact arithmetic after the declared scaling. `quantities: []` compares value alone.

Both input hashes, the certified shell objective, and the surface selector are retained. With
`--list-blocks`, the output also includes shell membership and the lower/upper number of design
sub-cells for every original OMF block. Block order is U, then V, then W from bottom upward.
Increasing depth can narrow the surface-intersection intervals; conservative work bounds refuse
requests that would require excessive triangle/cell processing.

The surface must be closed and lie entirely within the model. Geometry inherits the existing volcert
nominal sub-cell and winding-classification limits; the intervals are not statistical confidence
intervals. The design's slopes are not checked, so its value can exceed the slope-constrained shell's
value and the signed loss can be negative. This comparison supplies no geotechnical sign-off,
reserve, operational schedule or execution authority.

### Schedule an OMF block model under resource limits

`plan schedule` also accepts an OMF model and a separate JSON declaration. It constructs a whole-block
schedule, improves it by local search, checks every precedence and capacity row, and computes a global
Lagrangian upper bound on the **same integer-discount objective**. Both input files retain their SHA-256
identities in the report. This connects the OMF scheduling kernels to an executable workflow.

```bash
cargo run --release -p fmine-cli -- plan schedule \
  --omf model.omf --spec schedule.json --list-schedule --json
```

For example, `schedule.json` can declare the following assumptions for numeric parent-block attributes
named `value` and `tonnes` on a regular block model named `orebody`:

```json
{
  "schema": "fmine.omf-schedule.v1",
  "block_model": "orebody",
  "value_attribute": "value",
  "value_per_unit": 100,
  "value_rounding": "nearest_integer_half_away_from_zero",
  "slope": {"rise": 1, "run": 1, "levels": 3},
  "periods": 3,
  "discount": {"rate": 1, "scale": 10},
  "capacities": [
    {"attribute": "tonnes", "per_unit": 1, "limits_scaled": [10000, 12000, 12000]}
  ],
  "search_rounds": 20,
  "bound_steps": 64
}
```

The discount rate is `rate / scale` per period, here 10%. Period zero has factor `2^40`; each later
factor is `floor(previous * scale / (scale + rate))`. Values round to the nearest integer after scaling;
resource uses round upward. Capacity limits are **already scaled integers**, not unscaled tonnes or
implicitly converted quantities. Each row must have exactly one nonnegative upper limit per period.
Signed per-block uses are supported for declared blending rows. All declaration fields above are
required, and unknown or duplicate fields refuse.

To check a schedule exported by another planning tool, add
`"candidate": {"attribute": "mining_year", "first": 2027}`. The numeric OMF attribute then maps 2027 to
period zero, 2028 to period one, and null to unmined. The report preserves the candidate's original value,
capacity overruns and unmet-precedence witnesses. Only a feasible candidate can seed local search or
replace the selected schedule; a structurally valid but infeasible candidate remains visible as a finding.
The empty schedule remains an available zero-value incumbent.

JSON schema `fmine.plan-schedule.omf.v1` reports the selected origin, exact rational discounted value,
per-period values and resource usage, and independently replayed bound prices and gap. `--list-schedule`
adds one zero-based period or null per original OMF block, in U/V/W bottom-up order. A positive gap is
distance to a verified upper bound; it does not prove that much improvement is achievable. The certificate
covers the scaled deterministic model and finite-depth uniform slope cones. It supplies no reserve
classification, approved slope, execution authorization or optimum for the original unrounded attributes.

The declaration is bounded to 1 MiB and the OMF archive to 1 GiB. Work limits include 256 periods,
32 resource rows, 1,000 search rounds, 256 bound steps, one million block-period cells and a conservative
eight-million-arc bound before slope construction. Unsupported geometry, ambiguous selectors, null value
or capacity attributes, invalid candidate periods and oversized expansions refuse. OMF schedules use one
destination per block; the MineLib path above provides destination splits and optional LP guidance.

### Turn registered HTML tables into cited Claims

`html claims` reads local, registered HTML filings and explicit label mappings into canonical public Claims:

```bash
cargo run -p fmine-cli -- html claims --issuer example --definitions local/html-claims.toml --format markdown
cargo run -p fmine-cli -- html claims --issuer example --definitions local/html-claims.toml --as-of 2025-05-01 --json
```

The current supported population is site-level AISC, TCC, gold-equivalent production, processed/head grade
and recovery. It preserves printed numbers exactly, including large decimals and accounting parentheses.
Each result contains its full Claim receipt, period and publication vintage, ownership, unit, printed token,
exact decimal value and step, original table/cell anchor and byte range. Label citations and both the mapping
and source-registry hashes explain how the source became that Claim. Dashes, `n/a`, empty cells and numeric
zero remain distinct; an applicable registered disposal or withholding rule remains visible.

Register a filing in `registries/sources/<issuer>.toml` with a separate `[[html]]` row. This template shows the
required grammar; replace the example metadata, byte count and digest with the actual source evidence:

```toml
schema = "fmine.sources.v1"
issuer = "example"
corpus_dir = "local/filings"
retrieved_utc = "2025-05-01T12:00:00Z"
url_prefix = "https://example.invalid/"

[[subject]]
name = "Site A"

[[html]]
file = "filing.html"
release = "2025Q1"
url = "https://example.invalid/filing.html"
url_basis = "fetched and verified 2025-05-01"
published_at = "2025-05-01"
published_at_basis = "FACT: <citation establishing this publication date>"
bytes = 1234 # Replace with the actual file length.
sha256 = "<64 lowercase hexadecimal characters for the actual file bytes>"
```

An optional `dir` on `[[html]]` overrides `corpus_dir`. HTML registrations require no PDF page count. The
mapping names a data cell and the five labels establishing its interpretation; it never supplies an amount.
For a table whose rows are `Site A | 100%`, `AISC | 2025Q1`, and `US$/oz sold | 10.00`, the mapping is:

```toml
schema = "fmine.html-claims.v1"
issuer = "example"

[[claim]]
id = "site_a_aisc_q1"
file = "filing.html"
table = 1
cell = "B3"
subject_cell = "A1"
subject_label = "Site A"
period_cell = "B2"
period_label = "2025Q1"
metric_cell = "A2"
metric_label = "AISC"
unit_cell = "A3"
unit_label = "US$/oz sold"
ownership_cell = "B1"
ownership_label = "100%"
```

Table numbers start at one. Labels must match the decoded cell text exactly and satisfy the supported
grammar: for example `AISC` / `All-in sustaining costs`, `TCC` / `Total cash costs`, `Gold-equivalent
production`, `Head grade`, or `Recovery`. Cost units explicitly state USD per ounce sold; GEO uses `koz`,
grade uses `g/t` or `%`, and recovery uses `%`. Ownership is `100%`, `100% basis`, or an explicit
`Attributable (60%)`-style percentage. Subjects must be registered sites; regions and company totals are
outside this adapter. Merged label cells retain their original anchors, and one merged numeric origin
cannot be imported as multiple observations.

The optional publication cut excludes later mapped sources **before opening their files** and lists those
exclusions. Earlier releases and later comparatives keep separate Claims. These dates are the cited registry
dates, not independently authenticated timestamps. Inputs stay inside the selected workspace; HTML files
are bounded to 8 MiB and mapping files to 1 MiB. Extraction is offline and uses no browser, unit conversion
or inferred table layout. HTML metric keys remain separate from workbook definitions, so importing a TCC or
GEO figure does not establish its credit treatment or price-deck equivalence.

No live HTML filing is registered in the repository yet. The executable path is exercised with invented
filings; it does not establish real issuer coverage or completion of the broader BD-057/BD-059 acceptance
population. Full Claim receipts are returned directly by `html claims`.

### Check a disclosure draft against registered HTML evidence

The same HTML mapping can supply the Claims checked by disclosure lint:

```bash
cargo run -p fmine-cli -- lint draft.md --issuer example \
  --html-definitions local/html-claims.toml --format markdown
cargo run -p fmine-cli -- lint draft.md --issuer example \
  --html-definitions local/html-claims.toml --discover --as-of 2025-05-01 --json
```

Use actual registered sources and mappings as described above. A cited equality links the printed number
to its canonical Claim ID with `[10.00 USD/oz sold](fmine:clm_ID)` and states the subject, period, ownership
basis and metric. `--discover` searches the selected semantic population for an uncited equality; the
amount never chooses among competing vintages. The existing checker retains mismatches, excess precision,
stale citations and ambiguity. Current HTML cost mappings do not establish by-product or co-product credit
treatment, so a draft asserting either treatment remains unchecked rather than borrowing a workbook rule.

The report includes the full HTML value and label citations, source hashes, mapping and registry evidence.
An optional date filters future filings before their bytes are read and reports source publication separately
from numeric agreement. It does not prove that the current mapping or registry existed on that date.
This selector reads only the mapped HTML population and cannot be combined with `--through` or `--synthetic`.

### Select affordable capital bundles under shared scenario risk

DESIGN (BD-229, plan section 23): `decide program` constructs and compares static combinations of declared
capital candidates. Run the invented GB-30 exercise:

```bash
cargo run -p fmine-cli -- decide program tests/capital/gb30.toml --format markdown
cargo run -p fmine-cli -- decide program tests/capital/gb30.toml --json
```

The example has an USD 800M budget and three USD 400M packages: brownfield A, greenfield E and buyback D.
Every package supplies net delta-NAV under the **same five named scenarios**. Without the risk limit, A+E
has the highest mean at USD 372M. Its worst-20% signed loss is USD 210M. A USD 150M tail-loss ceiling selects
**A+D**, whose mean is USD 241.18M and tail loss USD 130.6M. The expected-value cost of that constraint is
USD 130.82M. These exact answers use the plan's displayed rounded buyback inputs; they reproduce its
one-decimal presentation of 241.2M and 130.8M without treating those rounded figures as exact inputs.

The strict `fmine.capital-program.v1` model declares `budget`, `tail_mass`, optional `max_tail_loss`, a common
USD valuation basis, the `additive_net_delta_nav` convention and explicit reference-class adjustment bases.
It contains `[[state]]`, `[[candidate]]`, complete `[[payoff]]` rows, optional `[[requires]]` edges and named
`[[uu]]` risks. Candidates sharing an `exclusion_group` cannot appear together. Monetary values are quoted
major USD amounts with at most four decimal places; weights and tail mass are exact rational strings.

The evaluator examines every subset of at most eight candidates on at most 256 states. It keeps every
structurally rejected bundle and reason, all risk-rejected comparisons, hold-cash and every exact co-winner.
Spending constrains the budget; it is **not deducted again** from already-net payoffs. Portfolio CVaR comes
from summed payoffs within each shared state, including fractional tail-boundary mass, so dependence remains
in the calculation. Full input/output Claims, source bytes, constraints and adjustment bases bind the receipt,
including candidates that never win.

#### Stress capital choices under declared scenario-weight shifts

DESIGN (BD-229, plan sections 22.5 and 23.3): an optional `[[robustness]]` declaration compares every
**nominally feasible static bundle** over a finite-support Wasserstein-1 ball. It reports worst-case mean
value and worst-case expected same-state regret, preserving every tied winner and a separate exact
primal/dual transport certificate for each objective. The nominal capital selection remains visible.

Run the entirely synthetic, two-state example:

```bash
cargo run -p fmine-cli -- decide program tests/capital/wasserstein.toml --format markdown
cargo run -p fmine-cli -- decide program tests/capital/wasserstein.toml --json
```

INFERENCE from the example's declared payoffs, weights and quarter-unit transport radius, in USD millions:

| Bundle | Nominal mean | Worst-case mean | Worst-case expected regret |
|---|---:|---:|---:|
| Hold cash | 0 | 0 | 20 |
| Risky project | 12 | 4 | 10 |
| Safe project | 8 | 8 | 12 |

The risky project wins on nominal mean and minimax expected regret; the safe project wins on maximin mean.
The joint bundle exceeds the budget and stays in the exclusions. These are synthetic known-answer
comparisons, not investment recommendations or empirically calibrated probability estimates.

The declaration supplies `method = "wasserstein_1"`, an exact rational `radius`, `metric_basis` and
`radius_basis`. Each `[[transport_distance]]` supplies `from`, `to` and rational `value`; all ordered pairs,
including zero diagonals, must be explicit and form a proper metric. No geometry or radius is inferred
from state names or payoff values. Input bytes and source-bound Claims retain those assumptions.

Structural, cash, debt, contractual and share-count constraints apply over the original complete state
support. The original nominal primary and secondary tail-loss screens determine the mean/regret comparison
population. Models with a per-share objective stress each state's exact whole-bundle NAV per actual share.

Every declared primary or secondary entity tail-loss cap also receives an exact worst-CVaR check over
the same finite-support ambiguity set. Each cap keeps its original tail mass and USD or actual USD/share
basis. Its certificate retains a feasible stressed law, worst-tail submass and matching transport/threshold
upper bound. Separate robust-tail feasible sets and maximin-mean/minimax-regret co-winners require every
declared cap to hold. Regret keeps the original nominally feasible benchmark; nominal selections and
adaptive value remain separate. Equality to a cap passes. If every choice fails, the robust-tail winner
lists are empty.

The example declares a zero cap on the worst-half loss. All amounts below are USD millions:

| Bundle | Nominal worst-half loss | Worst-case worst-half loss | Satisfies the cap throughout |
|---|---:|---:|---|
| Hold cash | 0 | 0 | Yes |
| Risky project | -4 | 12 | No |
| Safe project | -8 | -8 | Yes |

Risky remains the nominal expected-value winner. Safe wins both robust-tail filtered comparisons at the
declared radius of 1/4. These are conditional model calculations; geometry/radius calibration and omitted
regimes remain outside this slice.

The report exposes adversarial distributions, transported mass, transport cost, dual multipliers,
dual potentials and exact zero duality gaps. It retains empty eligible populations, rejected bundle IDs
and all co-winners. Geometry is limited to 64 states. Combined work is bounded before enumeration by
`(static subsets + policies) × (2 + 128 × declared tail limits) × states² ≤ 2,097,152`;
each tail check permits at most 128 exact transport calls and refuses unresolved convergence.
The same explicit declaration also supports two-stage policies, described below. The option can coexist
with the separate bootstrap diagnostic, and an absent declaration preserves the prior report shape.

#### Measure whether the selected program changes under scenario resampling

DESIGN (BD-229, plan sections 22.5 and 23.9): an optional `[[selection_stability]]` declaration adds
deterministic weighted bootstrap comparisons to either a static portfolio or a two-stage policy model:

```toml
[[selection_stability]]
method = "weighted_multinomial"
seed = "229"
replicates = "200"
draws_per_replicate = "5"
sampling_basis = "Synthetic sensitivity experiment on the declared five-state law; no empirical sample-size claim"
```

All fields are quoted strings. `seed` is an unsigned 64-bit integer, `replicates` is between 200 and 2,000,
and `draws_per_replicate` is between 1 and 4,096; combined work also has an explicit admission limit.
The sampling basis explains the chosen experiment size. It is not an inferred effective sample size or
evidence that scenario rows are independent historical observations. Each replicate draws from the original
declared weights, with replacement, and uses the resulting **same state-count vector across every alternative,
valuation entity, static portfolio and adaptive policy**. Sampling never combines marginal project outcomes.

Structural eligibility remains fixed over the complete original support. Omitting a Bad-state draw cannot
erase its cash-floor failure, unmet capital call, nonpositive share count or contractual obligation.
Within that population the engine recomputes expected value and every declared primary and secondary tail-loss
limit for each replicate. A nominal risk rejection may therefore become feasible under another weighting,
while a previously feasible choice may fail. Adaptive policies retain their original observation-based
decisions; bootstrap analysis does not choose branches with hindsight.

The report retains every replicate and all exact co-winners, gives selection frequencies with a fragility
threshold of `4/5`, and shows runner-ups and ranking changes without inventing a tie-breaking probability.
Replicates with no risk-feasible winner remain visible and in the denominator. Existing cash, debt, capital-call
and ownership ledgers are reused; the engine sorts each payoff vector once before reweighting it.
The seed, counts, method, basis and original source anchors bind the report alongside the economic inputs.

These are **conditional selection frequencies under the supplied finite scenario law**, not calibrated
probabilities that an investment is optimal or coverage for missing model mechanisms. Models without the
declaration keep their existing output and explicitly unavailable selection-frequency status.

Run the complete synthetic fragile-choice example with:

```sh
cargo run -p fmine-cli -- decide program tests/capital/selection_stability.toml --format markdown
```

#### Check whether the program can be funded in every declared scenario

The optional funding clause constrains cash separately from net NAV:

```bash
cargo run -p fmine-cli -- decide program tests/capital/liquidity.toml --format markdown
```

This invented example has a USD 80M planned budget, USD 110M available cash in each state and an explicit
USD 20M minimum cash balance. Expansion and maintenance each have USD 40M planned costs. Together their
declared expected net NAV is USD 108M, but expansion's cash requirement rises to USD 100M in the overrun
state. Expansion alone leaves USD 10M, below the floor; both projects leave minus USD 30M. Maintenance
alone remains fundable and has USD 40M expected net NAV. The report keeps the rejected choices and their
exact state-specific shortfalls alongside the fundable result.

Supply one `[[funding]]` row with `cash_period = "2027-01-01/2027-12-31"`, an explicit `cash_basis` and
`minimum_cash`, then complete `[[available_cash]]` rows (`state`, `value`) and `[[cash_requirement]]`
rows (`candidate`, `state`, `value`). Every state and every candidate/state pair must appear, including
explicit zeroes; cash values use the same exact USD input convention. Funding evidence and all rows bind
the result, including funding for candidates that cannot otherwise be selected. A scenario shortfall is
a structural rejection before risk ranking. If every subset fails, the result explicitly reports
`no_structurally_feasible_program`, with no invented winners or risk statistics.

This aggregate calculation tests the stated cash window. For dated financing and spending, supply a cash
schedule instead. Declared net-NAV payoffs already include their costs; enforcing either cash constraint
does not charge them again.

#### Fund construction before later operating receipts arrive

`funding_schedule` carries each scenario's cash balance through explicit periods, including signed financing
receipts and repayments. Run the complete invented bridge-financing example:

```bash
cargo run -p fmine-cli -- decide program tests/capital/cash_schedule.toml --format markdown
cargo run -p fmine-cli -- decide program tests/capital/cash_schedule.toml --json
```

The entity opens with USD 100M and needs USD 90M for baseline construction-period commitments, leaving only
USD 10M against a USD 20M floor. Holding fails that checkpoint even though operating receipts later restore
cash. Adding the plant requires a further USD 40M in the base state or USD 50M in the delay state, so it also
fails without financing. A bridge supplies USD 60M and later repays USD 65M or USD 68M. The bridge plus plant
holds construction-end cash at USD 30M or exactly USD 20M and produces **USD 62.4M expected net delta-NAV**.
Its operation-end balances are USD 135M and USD 52M. The report retains every rejected alternative and each
specific dated shortfall; the later surplus never erases an earlier breach.

Supply one `[[funding_schedule]]` row with an explicit `cash_basis` and `opening_date`, followed by:

- `[[cash_period]]`: `id`, inclusive `cash_period` date range, and nonnegative `minimum_cash` at its close.
- `[[opening_cash]]`: a nonnegative `value` for every `state`, before any program cash flow.
- `[[baseline_cash_flow]]`: signed `value` for every `period`/`state`, excluding the selected program.
- `[[candidate_cash_flow]]`: signed `value` for every `period`/`candidate`/`state`, including explicit zeroes.

Positive cash flows are receipts; negative flows are expenditures. The first period starts on `opening_date`,
subsequent periods are contiguous, and each closing balance becomes the next opening balance. The first
period's floor also applies at the initial opening checkpoint. Every scenario must meet every declared floor;
averaging across scenarios cannot hide a shortfall. A financing candidate can be the only feasible choice,
including one with negative NAV. Its actual statistics are retained, and regret compares only the feasible set.

Aggregate `funding` and `funding_schedule` are mutually exclusive. Schedules admit at most 16 periods and
65,536 total opening/period-end checkpoints across all enumerated subsets:
`2^candidates × states × (periods + 1) ≤ 65,536`. Larger requests refuse explicitly. Inputs remain bounded to
4 MiB, with exact USD decimals and complete matrices. The JSON and Markdown show all cash inputs and every
bundle's timeline, source Claims and receipt. Unscheduled models keep their existing receipts unchanged.

These checks establish opening and period-end balances only. If a payment must precede a receipt within one
period, declare separate periods to expose that timing; the engine does not infer an intra-period cash path.

#### Respect parent and joint-venture cash boundaries

DESIGN (BD-314, plan section 23.2): `entity_funding` gives each declared entity its own cash balance and
liquidity floor. It combines explicit funding calls with distributions governed by declared ownership,
permission and caps. A surplus in the JV cannot pay a parent obligation until the declared cash transfer
or distribution reaches the parent. Run the complete invented example:

```bash
cargo run -p fmine-cli -- decide program tests/capital/entity_funding.toml --format markdown
cargo run -p fmine-cli -- decide program tests/capital/entity_funding.toml --json
```

Parent opens with USD 25, the JV with USD 20, and both have USD 20 cash floors. A USD 5 study reveals Good
or Bad, each with declared weight 1/2. Good brings USD 100 into the JV in the second period. The declared
distribution pays 3/5 to Parent and 2/5 to outside shareholders. Parent can then spend USD 50 on a project
in the third period; Bad receives no JV cash and abandons that project.

| Observation | JV receipt | Paid to Parent | Paid outside the graph | Parent project | Parent closing cash | Parent NAV change |
|---|---:|---:|---:|---|---:|---:|
| Good | USD 100 | USD 60 | USD 40 | Build for USD 50 | USD 30 | USD 95 |
| Bad | USD 0 | USD 0 | USD 0 | Abandon | USD 20 | USD -5 |

The JV retains its USD 20 floor in both states. With 100 unchanged Parent shares, the winning policy has
exactly **9/20 USD/share expected improvement** and **1/20 USD/share tail loss**: USD 45 and USD 5 in total.
The best feasible fixed program is hold, so the net adaptive value is also 9/20 USD/share. Committing to
study plus construction in both states fails Parent's Bad-state floor by USD 50. Every alternative and its
complete entity ledger remain in the report.

Choose exactly one of `funding`, `funding_schedule` or `entity_funding`. The entity schema is:

| Table | Required fields | Complete population |
|---|---|---|
| `[[entity_funding]]` | `cash_basis`, `opening_date`, `objective_entity` | One declaration |
| `[[entity]]` | `id`, `label` | Every modeled entity |
| `[[entity_period]]` | `id`, `cash_period` | Contiguous inclusive date ranges from `opening_date` |
| `[[entity_floor]]` | `entity`, `period`, `value` | Every entity/period |
| `[[entity_opening_cash]]` | `entity`, `state`, `value` | Every entity/state |
| `[[entity_baseline_cash_flow]]` | `entity`, `period`, `state`, `value` | Every entity/period/state |
| `[[entity_candidate_cash_flow]]` | `entity`, `period`, `candidate`, `state`, `value` | Every entity/period/candidate/state |
| `[[entity_transfer]]` | `id`, `from_entity`, `to_entity`, `period`, `basis` | Each fixed internal transfer; optional `candidate` activates it only when selected |
| `[[entity_transfer_amount]]` | `transfer`, `state`, `value` | Every declared transfer/state, with nonnegative amounts |
| `[[entity_distribution]]` | `entity`, `parent_entity`, `parent_share`, `basis` | One declared parent per distributing entity; no cycles |
| `[[entity_distribution_gate]]` | `entity`, `period`, `state`, `allowed`, `maximum_distribution` | Every distributing entity/period/state |

All cash cells, including zeroes, are explicit quoted USD amounts. Ownership uses an exact quoted fraction
in `[0,1]`, such as `"3/5"`; permission is the quoted string `"true"` or `"false"`. `objective_entity` names
the entity whose declared NAV is being compared. If equity is present, `equity_basis.entity` must equal
that identifier. The other entities supply funding constraints, not additional NAV/share objectives.

Each period first applies native external cash flows and fixed internal transfers, then checks every
entity's floor. Distributions run from the deepest subsidiary toward its ancestors. An allowed distribution
is the smaller of its cap and cash above that entity's floor; a blocked one is zero. Parent receives its
exact ownership fraction, and the complement leaves the modeled graph. The engine checks closing floors
and carries cash forward, preserving even ownership receipts smaller than one monetary input unit.

Opening floors and every pre-distribution breach also remain binding. A dividend later in the same period
cannot erase an earlier shortfall; place that dividend in an earlier period if it must finance a subsequent
payment. Fixed transfers are never clipped to available cash. Internal movements cancel in each period's
conservation equation, while outside distributions explicitly reduce group cash:
`closing cash + outside distributions = opening cash + native external flows`.

For funding that leaves ownership unchanged, record an external partner's contribution as a native receipt
and a parent's call as an internal transfer; do not also record that transfer as native cash. Explicit
subscription calls below instead settle their own cash and update ownership. No contribution, top-up,
actual JV agreement or lending terms are inferred. Declared gate permissions and
caps remain assumptions; optional debt terms below add calculated coverage conditions. Candidate NAV remains separately
declared and already net of costs; cash is not added to or deducted from those values again.

The same entity model works for static portfolios and two-stage policies. Before observation, recourse
candidate flows and their activated transfers must explicitly be zero. The paid study must consume net
native external cash across the entities in every state; moving cash internally does not pay for information.
Each policy reuses only the ledger for its realized subset and state.

The bounded model admits at most four entities, 64 transfers and 16 periods. With `E` entities, `P` periods,
`T` transfers and `D` distribution edges, each state requires `E × (2P + 1) + T + D × P + P` ledger rows.
Optional debt adds one result per facility/period, so `F` facilities add `F × P` rows per state.
Optional subscription funding adds one row per declared call, including inactive calls.
Across every static subset and, when present, every policy, the total may not exceed 65,536. This includes
all checkpoints, inactive transfers, blocked distributions and conservation rows; oversized models refuse
before enumeration. The complete source, including losing inputs and original line anchors, binds the receipt.

#### Fund JV capital calls and carry changing ownership into dividends

DESIGN (BD-314, plan section 23.2): explicit subscription calls can change a parent's JV interest when the
parent and outside partner contribute different amounts. The engine settles the declared funding, checks
the required raise, and uses the resulting ownership for current and later distributions. Run the complete
invented example:

```bash
cargo run -p fmine-cli -- decide program tests/capital/capital_calls.toml --format markdown
cargo run -p fmine-cli -- decide program tests/capital/capital_calls.toml --json
```

The supported method is explicitly named `pre_money_subscription`. Given the parent's previous fraction
`s`, a declared positive 100% pre-money equity value `V`, parent funding `P` and outside funding `X`, the new
parent fraction is `(s × V + P) / (V + P + X)`. All arithmetic remains exact. Starting at 60% and a pre-money
value of USD 100, a USD 100 round preserves 60% if the parties fund 60/40, increases the parent to 80% if it
funds the whole round, or reduces it to 30% if the outside partner funds the whole round.

The example starts with Parent cash of USD 125, JV cash of USD 20 and a 60% Parent interest; each cash floor
is USD 20. Parent pays USD 5 for a study. That initial decision also commits USD 10 of later JV funding,
which remains payable after either observation. At a pre-money value of USD 100, this first subscription
changes Parent ownership to 7/11. The JV spends those USD 10 on its declared initial obligation.

After observing Good, the policy funds construction through a second USD 100 round at a pre-money value of
USD 110: Parent contributes USD 60 and the partner USD 40. After Bad, it abandons construction while paying
the initial obligation. The two possible realized paths are:

| Result | Good: construct | Bad: abandon |
|---|---:|---:|
| Initial future Parent contribution | USD 10 | USD 10 |
| Construction Parent / outside contribution | USD 60 / USD 40 | USD 0 / USD 0 |
| Final Parent ownership | 13/21 | 7/11 |
| Final JV dividend to Parent / outside partner | USD 130 / USD 80 | USD 0 / USD 0 |
| Ending Parent cash | USD 180 | USD 110 |
| Ending JV cash | USD 20 | USD 20 |
| Separately declared Parent NAV/share change | 19/20 USD/share | -1/20 USD/share |

Expected NAV/share improvement is **9/20**, tail loss is **1/20**, and net adaptive value is **9/20** over
the best feasible fixed program, hold. Always constructing would require Parent to fund the entire second
round after Bad, leaving only USD 10 against its USD 20 floor. That rejected path still shows the full
payment and its resulting 17/21 ownership. All four static portfolios and six policies remain visible.

Add these tables to a complete `entity_funding` model:

| Table | Required fields |
|---|---|
| `[[entity_capital_calls]]` | `method = "pre_money_subscription"`, `cash_flow_convention = "native_excludes_modeled_capital_calls"` |
| `[[capital_call]]` | `id`, `entity`, `period`, `sequence`, `basis`; optional activating `candidate` |
| `[[capital_call_state]]` | `call`, `state`, `pre_money_value`, `required_funding`, `parent_contribution`, `external_contribution` |

Each call uses the distributing entity's existing modeled parent and requires every shared state,
including zero contributions. Money uses quoted USD decimals. `sequence` is a quoted integer in `1..=64`,
unique within an entity/period; multiple calls follow that explicit order rather than identifier or input
order. Up to 64 calls are admitted within the complete ledger-work bound. Candidate alternatives may
activate different calls, and inactive calls retain their declared terms beside zero actual settlement.

Native cash and fixed transfers must exclude these modeled contributions. Parent pays its full declared
amount; the JV receives parent plus outside cash before debt service and pre-distribution floor checks.
Funding is never reduced to available cash. A raise below `required_funding` is a separate structural
failure even if the JV already holds enough cash. Partial contributions and their ownership effects stay
visible on rejected paths; the model supplies no automatic top-up or rollback. Oversubscription is allowed
when declared. Outside funding enters cash conservation separately, while internal Parent funding cancels.

Initial candidates' future calls survive abandonment. Recourse candidates cannot activate calls before
the observation boundary. The same-state call ledger composes with debt coverage/sweeps and independently
declared primary and secondary valuations. Subscription ownership does not manufacture another entity's
NAV or alter its separately supplied terminal share count. These synthetic examples demonstrate declared
subscription mechanics; they do not represent an issuer's actual JV agreement or penalty-dilution terms.

#### Calculate debt service, coverage locks and cash sweeps

DESIGN (BD-314, plan section 23.2): entity funding can include one external senior debt facility per debtor
entity. The engine carries principal, calculates interest and scheduled repayments, derives debt-service
coverage, and applies excess-cash sweeps before ownership distributions. Run the complete invented example:

```bash
cargo run -p fmine-cli -- decide program tests/capital/debt_funding.toml --format markdown
cargo run -p fmine-cli -- decide program tests/capital/debt_funding.toml --json
```

The example starts with Parent cash of USD 25, JV cash of USD 20 and JV debt of USD 100. Both cash floors
are USD 20. Parent pays USD 5 for a study before Good or Bad is observed. In the next period, the JV owes
USD 10 interest and USD 20 scheduled principal. Its declared coverage threshold is 3/2 and it sweeps half
of cash above its floor toward outstanding principal before paying any dividend.

| Observation | JV cash receipt and CFADS | Debt-service coverage | Principal sweep | Dividend received by Parent | Parent continuation |
|---|---:|---:|---:|---:|---|
| Good | USD 130 | 13/3 | USD 50 | USD 30 | Build the USD 20 project |
| Bad | USD 30 | 1/1 | USD 0 | USD 0 | Abandon construction |

The Bad-state coverage lock leaves the JV's cash available for its own obligations. It does not itself
declare a default or reject the program. The selected policy has net NAV changes of USD 95 and USD -5,
giving **USD 45 expected improvement and USD 5 tail loss**, or **9/20 and 1/20 USD/share** with 100 Parent
shares. Parent closes with USD 30 after Good and USD 20 after Bad. The best feasible fixed program is hold.

Debt commitments continue after the observation: the study also commits a final USD 1 principal payment
in both states. Interest in that final period is USD 3 after Good and USD 8 after Bad, reflecting the earlier
sweep. Both paths pay their remaining scheduled service, including the abandoned path, and close with debt
of USD 29 and USD 79 respectively. The report retains all six policies and four static portfolios.

Add these tables to a complete `entity_funding` model:

| Table | Required fields |
|---|---|
| `[[entity_debt]]` | `cash_flow_convention = "native_excludes_modeled_debt"` |
| `[[debt_facility]]` | `id`, `entity`, `basis`, `cfads_basis`, `minimum_dscr`, `sweep_fraction`, `allow_distributions_without_service` |
| `[[debt_opening_principal]]` | `facility`, `state`, `value` |
| `[[debt_service]]` | `facility`, `period`, `state`, `interest_rate`, `draw`, `principal_payment_cap`, `cfads` |
| `[[candidate_debt_service]]` | `facility`, `period`, `candidate`, `state`, `draw`, `principal_payment_cap`, `cfads` |

Opening principal is complete for every facility/state. Baseline service is complete for every
facility/period/state, and candidate increments for every facility/period/candidate/state, including zeroes.
Monetary values use the existing quoted USD format. Rates, minimum coverage and sweep fractions use exact
quoted `n/d`; the zero-service permission is explicitly `"true"` or `"false"`. All loan rows remain bound to
the original source file and the entity funding Claim, including terms for candidates that never win.

The period rate applies to **opening principal plus the current draw**. Scheduled principal is the smaller
of the outstanding amount and the declared payment cap. The engine debits full mandatory interest and
principal before checking cash floors. An unaffordable payment creates a retained cash shortfall; it is
never reduced to the amount available, and a later receipt cannot erase it.

CFADS means the separately declared **cash available for debt service** metric for the covenant. It is not
another cash receipt. Coverage is CFADS divided by mandatory interest plus scheduled principal, excluding
the sweep; equality with the threshold passes. Loan draws, reserves and internal funding do not silently
increase CFADS. When no service is due, the ratio is absent and the explicit zero-service permission applies.

After those mandatory-service checks, the graph processes entities child first. Each debtor first sweeps
the smaller of its remaining principal and `sweep_fraction × max(0, current cash - floor)`, then applies its
ownership distribution. That cash includes dividends just received from children. Root entities also sweep
their receipts, even when they have no outgoing ownership edge. A dividend requires both the original
permission/cap and the calculated coverage permission. Blocked cash carries forward.

The reports retain every loan input, principal balance, service component, coverage result, sweep and
effective gate, including rejected paths. They prove two exact identities each period:

```text
closing principal + scheduled principal + sweep = opening principal + draw
closing cash + outside dividends + interest + scheduled principal + sweep
    = opening cash + native cash flows + draw
```

Native cash rows must exclude those modeled debt movements, and CFADS must not be booked as an extra
receipt. Declared candidate NAV remains already net of its financing economics; this funding calculation
does not add proceeds to NAV or charge service again. Loan rates and covenant terms are explicit inputs,
with no inferred annualization, market pricing, waivers, arrears, default remedies or lender priorities.
Recourse debt draws, principal-cap increments and CFADS increments must each be zero before observation.
This mode shares the existing bounded static/adaptive evaluator and does not add an entity valuation objective.

#### Compare buybacks, issuance and projects on NAV per share

DESIGN (BD-229, PO-CAP-2): the optional equity mode evaluates each complete bundle using its resulting equity
NAV and share count. It can be combined with any declared funding model. Run the complete invented example:

```bash
cargo run -p fmine-cli -- decide program tests/capital/equity.toml --format markdown
cargo run -p fmine-cli -- decide program tests/capital/equity.toml --json
```

Set `payoff_convention = "additive_equity_nav_with_share_changes"` and declare:

- One `[[equity_basis]]` with `entity`, `share_class`, positive `shares_per_unit`, positive
  `initial_share_units`, and optional `max_tail_loss_per_share`.
- `[[baseline_equity_nav]]` with `state` and `value` for every shared state.
- `[[candidate_share_change]]` with `candidate`, `state` and signed `value` for every candidate/state pair,
  including explicit zeroes. Negative changes retire shares; positive changes issue shares.

Share counts, their unit scale and the per-share risk limit use exact quoted fractions such as `"100/1"`,
`"1000000/1"` and `"1/4"`. Baseline NAV and candidate payoffs use the existing exact USD decimal format.
For baseline NAV `N`, initial share units `u`, shares per unit `s`, and selected changes `dN` and `du`, the
state payoff in USD per actual share is `(N + dN) / ((u + du) * s) - N / (u * s)`.
The evaluator computes this once for the whole bundle in each shared state. It never sums standalone
per-share benefits or substitutes a ratio of expected NAV to expected shares.

Candidate payoffs in this mode must be **changes in total equity NAV**. A buyback records the cash paid,
including fees, as a negative NAV change and the retired shares as a negative share change. Issuance records
net proceeds and new shares. Ordinary projects declare already-net equity NAV changes. An existing
standalone buyback-accretion score cannot be reused as the NAV change: that would count its benefit twice.
Debt proceeds alone are not an equity NAV gain. Funding constraints consume the separate cash-flow inputs;
they do not deduct costs from declared net NAV again.

The synthetic fixture starts at USD 1,000 NAV and 100 actual shares. A USD 80 buyback retiring ten shares
adds exactly **2/9 USD per share**. Issuing twenty shares for USD 160 loses **1/3 USD per share** and fails
the declared **1/4 USD per share** tail-loss ceiling. The project cannot meet the cash floor alone, but
**issuance plus project wins at 1/2 USD per share**, with USD 1,260 ending NAV, 120 shares and USD 160 cash.
All eight subsets remain in the report, including the rejected combinations and their state-level ledgers.

Expected value, quantiles, same-state regret and tail loss remain exact rational values in USD/share.
Total-USD NAV statistics are reported separately; the old USD `max_tail_loss` is forbidden in equity mode.
Nonpositive ending shares reject the affected bundle and leave its undefined ratios explicit. Missing
matrix entries or arithmetic overflow refuse the whole comparison. The entity, share class, unit scale,
baseline NAV, share changes, risk limit, sources and unknown-risk terms all bind the result's receipt.
Models without an equity declaration retain their existing economics and receipts.

#### Evaluate parent and carve-out shareholders together

DESIGN (BD-314, plan section 23.2): a capital program can evaluate additional entity/share-class denominators
on the same scenarios while retaining one explicit primary objective. Each entity supplies its own net NAV
changes, share changes and optional per-share loss ceiling. The selected program must satisfy all declared
limits. Run the complete invented parent/carve-out example:

```bash
cargo run -p fmine-cli -- decide program tests/capital/entity_valuation.toml --format markdown
cargo run -p fmine-cli -- decide program tests/capital/entity_valuation.toml --json
```

Parent starts at USD 1,000 NAV and 100 shares; the carve-out starts at USD 600 NAV and 60 shares. Both have a
declared tail-loss limit of 1/10 USD/share. Good and Bad each have weight 1/2. Parent pays USD 10 for a study,
whose declared net NAV effects are -10 for Parent and -6 for the carve-out. It can then choose either an
aggressive deal or a fair deal after Good, abandoning further investment after Bad.

The aggressive deal adds more Parent NAV but issues thirty carve-out shares. The fair deal issues six.
Calculating each entity's ending NAV divided by its own actual shares gives:

| Policy | Parent expected change/share | Parent tail loss/share | Carve-out expected change/share | Carve-out tail loss/share | Joint result |
|---|---:|---:|---:|---:|---|
| Study; aggressive deal only after Good | 7/10 | 1/10 | -13/12 | 31/15 | Carve-out limit exceeded |
| Study; fair deal only after Good | 2/5 | 1/10 | 89/220 | 1/10 | All limits met |

The fair policy wins under the joint limits. Parent ends with NAV 1,090/990 and 100 shares after Good/Bad;
the carve-out ends with NAV 720/594 and 66/60 shares. The carve-out's Good-state gain is exactly 10/11
USD/share, and both entities' Bad-state loss is 1/10. Parent's expected-value cost of the joint risk limits
is 3/10 USD/share. The best feasible fixed program is hold, so constrained adaptive value is 2/5 USD/share.

Funding is checked independently. Parent opens with USD 30 and pays the study while keeping its USD 20 floor.
A Good-state child receipt of USD 50 produces a USD 30 Parent dividend in the next period, before Parent
spends USD 20 on the chosen deal. Parent closes at USD 30/20 and the child at USD 20 in both states. The
terminal share changes do not retroactively change that earlier dividend. All eight static portfolios and
twenty policies remain visible, including funding failures and risk-rejected alternatives.

Keep the existing primary `[[equity_basis]]`, `[[baseline_equity_nav]]`, `[[candidate_share_change]]` and
`[[payoff]]` inputs. Add these strictly entity-keyed tables for each secondary valuation:

| Table | Required fields |
|---|---|
| `[[entity_valuation]]` | `entity`, `share_class`, `valuation_basis`, `shares_per_unit`, `initial_share_units`; optional `max_tail_loss_per_share` |
| `[[entity_baseline_nav]]` | `entity`, `state`, `value` |
| `[[entity_candidate_payoff]]` | `entity`, `candidate`, `state`, `value` |
| `[[entity_candidate_share_change]]` | `entity`, `candidate`, `state`, `value` |

Every secondary entity needs every baseline state and every candidate/state NAV and share-change entry,
including explicit zeroes. NAV uses quoted USD decimals; share quantities, scales and loss limits use exact
quoted `n/d` fractions. Secondary entities must be distinct from each other and the primary entity. Each
declares its own valuation basis and retains a separate Claim with the original file hash, row anchors and
unknown-risk terms. A valuation perimeter need not be a node in the cash graph.

All entities use the shared state weights and program tail mass. Nonpositive ending shares are named
entity/share-class/state structural failures; undefined ratios remain visible. Statistics and hindsight
regret use the population satisfying every structural constraint, then each entity's optional tail ceiling
is applied. Equality passes, all caps bind jointly, and the primary mean alone ranks the surviving choices.
No combination of per-share values across overlapping entity perimeters is used as an objective.

Adaptive policies reuse each realized subset's same-state entity ledgers and retain initial NAV and share
commitments after abandonment. They recompute every entity's unconditional policy risk; they never inherit a
static subset's global risk rejection or a failure on an unrealized path. Reports retain all entity inputs,
state ledgers, exact statistics, individual cap results and complete static/adaptive populations.

Up to three secondary entities are admitted alongside the primary. With secondary valuations, the total
static-plus-policy population times shared states times all valuation entities must not exceed 65,536
valuation-state rows. Cash-work limits continue to apply independently. Valuations and transaction effects
are explicit source assumptions: the engine does not infer NAV from ownership cash, reconcile overlapping
perimeters, infer actual JV penalty-dilution terms, or change dated waterfall ownership from terminal
share changes. Without secondary declarations, existing report bytes and receipts remain unchanged.

#### Fund a study, then choose a response

DESIGN (BD-229, plan section 23): an explicit two-stage model can commit to a paid study, observe its declared
result, and then choose financing, construction or abandonment. Run the complete invented example:

```bash
cargo run -p fmine-cli -- decide program tests/capital/recourse.toml --format markdown
cargo run -p fmine-cli -- decide program tests/capital/recourse.toml --json
```

Add one `[[recourse]]` table naming `information_candidate`, `observation_date` and `information_basis`.
Map every candidate through `[[candidate_stage]]` (`candidate`, `stage = "initial"` or `"recourse"`) and
every shared state through `[[state_observation]]` (`state`, `observation`). The complete fixture shows
these tables alongside the required dated cash matrix and optional equity inputs.

The information candidate must be an initial commitment with positive planned cost, negative net NAV in
every state, and negative cumulative cash before the observation. The observation date starts a noninitial
cash period; every recourse candidate declares zero cash flow before that boundary. Without the study,
the policy must choose one common continuation across all states. With it, the policy may respond only to
the observation: states assigned the same observation always receive the same continuation. An explicit
`requires` relation can additionally make the study a prerequisite for construction or financing.

In the synthetic fixture, the company starts with USD 1,000 NAV, 100 shares and USD 30 cash. The study costs
USD 10, leaving exactly the USD 20 cash floor. Good and Bad each have declared weight 1/2. The chosen policy is:

| Observation | Continuation | Ending equity NAV | Ending shares | NAV/share change | Closing cash |
|---|---|---:|---:|---:|---:|
| Good | Issue 20 shares for USD 160, then fund construction | USD 1,250 | 120 | 5/12 USD/share | USD 80 |
| Bad | Abandon further investment | USD 990 | 100 | -1/10 USD/share | USD 20 |

The study cost remains in both outcomes. Expected improvement is exactly **19/120 USD/share**, with
**USD 120 expected total NAV change**. The declared tail-loss ceiling is met at equality, **1/10 USD/share**.
The best fixed program is hold, so the separately reported net adaptive value is **19/120 USD/share**.
Issuing in the Bad state would increase total NAV but dilute existing shareholders; the per-share objective
correctly declines that continuation.

The evaluator checks budget, prerequisites, exclusions, cash and ending shares on each realized path.
It evaluates CVaR on the complete policy with the original unconditional state weights. A separate risk
limit at each observation would answer a different question and can discard the correct global winner.
All initial commitments and their full declared cash, NAV and share effects remain in every continuation.

JSON and Markdown retain every policy, its initial choices, continuations, realized paths, cash checkpoints,
equity ledgers and rejection reasons, plus the complete static comparison. Net adaptive value is the best
policy mean minus the best static mean under the same objective and risk rule; it may involve a different
initial commitment. It is already reflected in policy outcomes and must never be added to NAV again.
If either population lacks a feasible winner, the comparison is explicitly unavailable.

This exact mode admits at most four observation classes and 256 policies. With `f` initial candidates,
`r` recourse candidates and `k` classes, the population is `2^(f-1) × (2^r + 2^(r×k))`; all rejected policies
remain visible. The combined static and policy cash-checkpoint count must be at most 65,536. Larger models
refuse explicitly. The evaluator reuses each static subset's same-state cash and equity results rather than
recalculating those ledgers for every policy, and sorts exact distributions without floating-point rounding.

Add the same `[[robustness]]` declaration and complete `[[transport_distance]]` matrix used by static
programs to assess conditional policies under declared scenario-weight shifts. The report retains separate
static-bundle and adaptive-policy comparisons; each has its own feasible population, hindsight-regret
reference, co-winners and exact transport certificates. The adaptive report uses `policy_id` and scope
`nominally_feasible_adaptive_policies`.

The stress calculation changes the **unconditional weights of complete states**. A policy's paid study,
observation classes and chosen continuations stay fixed. States with the same observation cannot acquire
different decisions, and the study cost stays in every resulting path. Branch probabilities in the policy
table remain nominal; each stress certificate displays its own adversarial distribution. Cash, contractual
and share-count failures remain binding even if an adversary gives the failing state zero weight.
The nominal tail-loss screens define the mean/regret comparison population. Every declared primary and
secondary entity cap also receives an exact worst-CVaR certificate for the complete policy outcomes.
Separate robust-tail feasible sets and filtered co-winners require all caps to hold over the declared
ambiguity set; the regret benchmark remains the original nominally feasible policy population.

INFERENCE from the independent synthetic study test: a USD 10 study followed by construction only in the
good state has complete policy payoffs `[90, -10]`, with equal nominal weights and unit state distance.
Construction in this fixture requires the study; building without it is structurally excluded from regret.
At radius `1/4`, its nominal mean is USD 40, worst-case mean is USD 15, and worst-case expected regret is
USD 7.50 against the feasible policy population. At radius `1/2`, hold-cash wins on worst-case mean while
the adaptive policy still wins on worst-case regret. These two criteria can choose different policies.
The same calculation uses realized whole-policy NAV per actual share in equity mode. Combined work is
bounded over all static subsets and policies before enumeration; the bootstrap diagnostic can run alongside it.

These are conditional comparisons of declared assumptions. The examples apply no fitted reference-class
correction, and the output supplies no calibrated signal probabilities, empirical selection frequency,
additional decision stages, inferred JV agreement terms, project feasibility approval or execution authority.
The wider CapitalProgram and its M4 gates remain open.

### Bound a recovery decision with untested ore

```sh
cargo run -p fmine-cli -- decide recovery-bounds tests/decisions/untested_ore.toml --json
cargo run -p fmine-cli -- decide recovery-bounds tests/decisions/untested_ore_tonnes.toml --format markdown
```

The strict `fmine.recovery-bounds.v1` worksheet declares tested and untested recovery intervals,
a contained-metal weight interval or explicit dry tonnes and grades, a decision threshold, an additivity
basis, the basis for the untested bound and one named positive UU exposure. All physical quantities use
quoted exact decimals or fractions. The card reports exact recovery bounds, all eight endpoint witnesses,
decision stability and the attainable test conditions that would settle the choice.

For the first synthetic worksheet, uncertain weights and tested recovery give an aggregate interval
of **[19/25, 917/1000]**. At an 85% threshold the choice is unstable: a valid test raising the untested
lower bound to at least **19/25** guarantees proceed; lowering its upper bound to strictly below
**34/75** guarantees stop. The strict inequality matters because equality with the decision threshold
counts as proceed.

The tonne example supplies 80 dry tonnes at 1 g/t and 20 dry tonnes at 3 g/t. Untested ore therefore
contains **3/7** of the metal, and the aggregate lower bound is **24/35**. A 20% tonne fraction would
produce the wrong recovery bound.

JSON and Markdown preserve the source, input/output Claims, evidence, confidentiality and UU. The
receipt binds that metadata as well as the numerical result. These are conditional identification
bounds over declared assumptions, without probabilities, calibrated predictions, measured recovery,
reserve classification or execution authority. Declared blending interactions refuse additive averaging.

### Compare explicit decision alternatives

DESIGN (BD-225, plan §22): `decide card` evaluates an explicitly declared finite scenario model and renders a
draft DecisionCard. Run the invented expansion exercise supplied in the repository:

```bash
cargo run -p fmine-cli -- decide card tests/decisions/expansion.toml --format markdown
cargo run -p fmine-cli -- decide card tests/decisions/expansion.toml --json
```

The model declares named states with exact rational probabilities, monetary payoffs for every action in every
state, one explicit do-nothing action, a common currency and valuation basis, and a named unknown-risk scenario
with its trigger and downside exposure. `payoff.value` is in **major currency units**, with at most four decimal
places. `uu.downside_usd_m` is explicitly in USD millions. Weights must be strictly positive and sum exactly to
one. Missing or duplicate states, incompatible currencies, missing unknown-risk terms and unsupported fields
refuse the comparison; the engine does not repair the model or drop alternatives.

The exercise has a 60% high state and a 40% low state. These are invented input assumptions. The computed results,
in USD millions, are:

| Alternative | Expected NPV | CVaR95 of loss | Mean regret | Maximum regret |
|---|---:|---:|---:|---:|
| Build the expansion | 100 | 200 | 102 | 255 |
| Smaller fixed-return option | 55 | −55 | 147 | 245 |
| Do nothing | 0 | 0 | 202 | 300 |

Expected value ranks the expansion first; minimax regret ranks the smaller option first. Regret compares each
action with the best payoff in the **same named state**. CVaR95 integrates exactly the worst 5% of signed loss
`-NPV`, including only the needed fraction of a boundary state's probability. An all-gain alternative can
therefore have negative tail loss. The card also reports weighted inverse-CDF P10/P50/P90, complete per-state
payoffs and regret, both rankings and every tied winner. All calculations use checked exact arithmetic; JSON
monetary results are rational strings in major currency units.

The command hashes the actual model bytes and retains the complete input Claims, source anchors, evidence
labels, confidentiality and unknown-risk terms in its output. The output Claim and report digest bind that full
input metadata as well as the computed statistics. Editing an assumption changes the identity even when it
leaves the numerical ranking unchanged. Model-file inputs remain **Assumed, internal and uncalibrated**; supplied
probabilities do not become calibrated by passing arithmetic checks. The explicit unknown-risk terms remain
outside the priced scenario matrix. This comparison supplies no feasibility approval, calibrated selection
frequency, historical replay proof or durable decision-ledger entry. It does not pass the full
BD-225 or Helm acceptance gates.

#### Retain a decision and its original evaluated evidence

DESIGN (bounded BD-225): `decide journal` records an explicit act-now choice, its rationale, and the exact
model and evaluated card that supported it. It uses the existing locked, append-only journal. Initialize
a new file, then set `FMINE_DECISION_HEAD` to the returned head and `FMINE_CODE_REVISION` to the declared
40- or 64-character lowercase Git revision of the evaluator:

```bash
cargo run -p fmine-cli -- decide journal init --journal decisions.journal \
  --cell planning-decisions --trust-host-clock

cargo run -p fmine-cli -- decide journal record --journal decisions.journal \
  --expected-head "$FMINE_DECISION_HEAD" --model tests/decisions/expansion.toml \
  --decision expansion-review --action smaller \
  --reason "Prefer the fixed-return alternative under the declared downside exposure" \
  --code-revision "$FMINE_CODE_REVISION" --trust-host-clock

cargo run -p fmine-cli -- decide journal show --journal decisions.journal \
  --head "$FMINE_DECISION_HEAD" --format markdown
```

After each successful record, replace `FMINE_DECISION_HEAD` with that operation's returned head and keep
the value independently of the journal. A stale expected head, reused decision id, unknown action or
oversized record refuses without appending. A later choice uses a new decision id, preserving the earlier
one. Any declared act-now alternative, including do nothing, can be recorded; selecting an action does
not remove the other alternatives from the retained comparison.

Each entry retains the original model bytes, evaluated card JSON, source path, SHA-256 digests, output
Claim id and declared code revision. `show` verifies the chain and retained bytes without reopening the
mutable model or recalculating the card with newer code. The complete encoded snapshot must fit the
existing 64 KiB operation limit. An independently saved `--head` detects rollback or rewriting; unpinned
inspection explicitly cannot establish the original history or evaluation provenance. Host-clock accuracy
and the code revision are declarations, not external time proof or authenticated human identity. Recording
a choice grants no approval, sign-off or execution authority and does not close the full BD-225 gate.

#### Compare choices when scenario weights are contested

DESIGN (BD-225, plan §22.5): an optional Wasserstein-1 comparison finds each original action's worst
expected value and worst expected same-state regret over an explicitly declared probability-transport
budget. The nominal decision remains visible, alongside the maximin-value and minimax-expected-regret
rankings and every exact co-winner. Run the complete synthetic example:

```bash
cargo run -p fmine-cli -- decide card tests/decisions/wasserstein.toml --format markdown
cargo run -p fmine-cli -- decide card tests/decisions/wasserstein.toml --json
```

The optional declaration has four quoted fields:

```toml
[[robustness]]
method = "wasserstein_1"
radius = "1/4"
metric_basis = "Synthetic dimensionless distance between the declared scenario regimes"
radius_basis = "Declared sensitivity budget; no statistical confidence claim"
```

Supply a `[[transport_distance]]` row with quoted `from`, `to` and `value` fields for **every ordered state
pair**, including diagonal `value = "0/1"`. Every off-diagonal distance must be positive, reverse distances
must agree, and the complete matrix must satisfy the triangle inequality. Distances and radius are exact
nonnegative fractions in the declared dimensionless metric. The engine does not infer a distance from a
state's name, normalize the metric, choose a radius, or drop a missing matrix cell. This comparison supports
at most 64 states and 256 actions, with an explicit bound on the total action/state-pair work.

Each result retains a sparse probability-transport witness, its stressed state weights and cost, and a dual
certificate whose value equals the primal value exactly. The worst-mean and worst-regret witnesses are
separate: their adversaries may choose different distributions. Every action retains its supplied payoff
in every original state. The configuration Claim binds the original declaration and every distance row,
including its source digest and original line locator; source assumptions and unknown-risk terms survive.

The metric and radius define a conditional sensitivity analysis. They supply no empirical confidence
coverage or protection from omitted regimes. Nominal tail statistics and information-option values remain
unchanged; this view does not reoptimize information policies or claim robust CVaR or physical feasibility.
Models without the declaration keep their existing output bytes.

With information options in the same model, the card also compares each **fixed signal policy** with
acting now under the same state-weight stress. Run the complete synthetic example:

```bash
cargo run -p fmine-cli -- decide card tests/decisions/wasserstein_information.toml --format markdown
cargo run -p fmine-cli -- decide card tests/decisions/wasserstein_information.toml --json
```

Its 75%-accurate test chooses the fixed-return alternative on a bad signal and expansion on a good signal.
After its $6M test cost and $4M delay cost, the policy has a nominal expected value of **$67.5M**. Under the
declared radius of 1/4, its worst expected value falls to **$27.5M**, while acting on the fixed-return
alternative retains **$30M**. The new `fixed_policy_comparison` reports that change, the policy's conditional
net value in every original state, its selected signal actions, and an independently checkable exact
transport certificate. Action and information-option identifiers remain distinct even when their names match.

This comparison varies only the scenario weights. It preserves the original test likelihoods, the displayed
signal actions (including the existing resolution of posterior ties), and both costs charged exactly once.
It does not choose a new policy after seeing the stressed law or treat conditional state means as realized
test outcomes. Existing nominal VOI, tail-risk and act-now regret results remain visible. All extra policy
transport solves count toward the declared work bound.

### Screen choices against an explicit downside limit

The optional source-bound limit specifies a monetary floor in the model's currency and valuation basis,
and an exact maximum probability of realized net payoff strictly below that floor:

```toml
[[downside_limit]]
floor = "0.0000"
max_breach_probability = "3/20"
```

Every act-now action and displayed fixed information policy retains its conditional breach probability
by original state, nominal probability and eligibility. With declared Wasserstein robustness, each
choice also has its worst-case breach probability, eligibility and an exact transport certificate.
Both eligibility screens rank the surviving choices by **nominal expected value**; the probability
constraint does not silently replace that objective with a worst-mean objective. All exact ties and
rejected choices remain visible. If every choice fails, the result explicitly says `no_eligible_choice`
with no fallback, winner or fabricated value.

```bash
cargo run -p fmine-cli -- decide card tests/decisions/downside_limit.toml --format markdown
```

In this synthetic example the test policy has nominal loss probability **1/8** and stressed loss
probability **3/16**. A limit of **3/20** therefore admits it nominally and rejects it under stress.
The nominal screen selects its **$67.5M** expected value; the stressed screen selects the **$30M**
fixed-return alternative, with **$37.5M** of nominal expected value forgone. Equality with the probability
limit passes, and payoff exactly at the floor is not a breach. Test and delay costs are charged once
in each realized signal outcome; a conditional mean is never substituted for that outcome's loss event.
The screen neither recalibrates probabilities nor reoptimizes policies, measures loss severity,
certifies physical feasibility or authorizes execution. Omitting the limit preserves existing card output.

### Optimize test responses within a downside limit

The fixed expected-value policy can fail a risk limit even when a different response to the same test
would be feasible and valuable. To search the complete finite set of deterministic responses, add:

```toml
[[policy_optimization]]
method = "exact_deterministic"
screen = "nominal"
max_policies = 4096
max_work = 1000000
```

This requires the existing `[[downside_limit]]` and at least one information option. Choose
`screen = "wasserstein"` to enforce the worst-case breach probability under the declared transport
geometry; the optimization objective remains **nominal expected net payoff** in either mode.

```bash
cargo run -p fmine-cli -- decide card tests/decisions/constrained_testwork.toml --format markdown
```

The synthetic example sets a floor of −10 and a maximum breach probability of 1/20:

| Response | Expected net value | Breach probability | Eligibility |
|---|---:|---:|---|
| Fixed EV policy: act after strong or medium signals | 34 | 1/10 | Rejected |
| Optimized policy: act only after a strong signal | **57/2 = 28.5** | **1/100** | Eligible |
| Best eligible act-now choice: hold | 0 | 0 | Eligible |

The optimizer selects one common action per observed signal, never an action chosen with knowledge of the
true state. Test and delay costs are charged once on every branch, including hold. Reports retain the
original fixed-policy comparison, the selected mapping, its exact realized payoff law and risk measures,
the exact number of co-optimal mappings, and the comparison with eligible act-now alternatives.
Canonical signal/action ordering chooses one reproducible representative when multiple mappings tie.
Unreachable signals select no action and do not inflate the policy count.

The complete population is bounded before enumeration: at most 65,536 mappings and 4,000,000 declared
work units across all options. Exhausted budgets refuse without a partial optimum. Infeasible options
have no fabricated winner or value. This is exact optimization over the supplied finite deterministic
policies; it does not optimize randomized or sequential policies, impose a CVaR limit, calibrate the
probabilities or authorize action. Omitting the opt-in preserves existing cards and their identities.

### Evaluate whether testwork is worth buying

DESIGN (BD-227, plan §22.4): the same command evaluates an optional information-gathering step before choosing
an action. The model supplies explicit test and delay costs, named signals, and the exact likelihood of each
signal in each state. Run the invented GB-29 exercise:

```bash
cargo run -p fmine-cli -- decide card tests/decisions/metallurgical_test.toml --format markdown
cargo run -p fmine-cli -- decide card tests/decisions/metallurgical_test.toml --json
```

The example assumes a 60% high state, build payoffs of +$300M or −$200M, and an 85%-accurate test in each state.
The do-nothing payoff is zero. The exact calculation gives:

| Quantity | Result |
|---|---:|
| Best expected value before testwork | $100M |
| Expected value of perfect information (EVPI) | $80M |
| Expected payoff from the optimal action after each test result | $141M |
| Gross expected value of sample information (EVSI) | $41M |
| Test cost | $6M |
| Declared monetary cost of delay | $3.8M |
| Net information value | $31.2M |
| Expected value after test and delay costs | $131.2M |

A high test result has probability `57/100` and raises the high-state posterior to `17/19`; build is then
optimal. A low result has probability `43/100` and lowers that posterior to `9/43`; do nothing is then optimal.
The output retains all posterior action ties, every named signal, and the complete posterior state weights.
A signal with zero marginal probability is shown as unreachable, with no invented posterior or action.

Each `information_option` declares `test_cost`, `delay_cost` and integer `delay_days`. Costs are nonnegative
amounts in the model's major currency units and valuation basis; duration does not imply a discount rate.
Both monetary costs are charged once for the chosen option, regardless of its result. The declared action
payoffs supply all other economics; the evaluator does not infer further effects from waiting.
`signal` rows name that option's possible results. Its `likelihood` rows must explicitly cover every state and
signal, including zero entries, with each state's probabilities summing exactly to one. The card refuses
missing cells, unknown references and malformed probability rows instead of filling or normalizing them.

The engine enumerates the finite model exactly and checks `0 ≤ EVSI ≤ EVPI`. It reports the calculation method
and exact net-value status; Monte Carlo standard error is explicitly not applicable. This is arithmetic
conditional on the supplied assumptions, not a calibration result. An uninformative test has zero EVSI; a
costly test can have negative net information value. At the GB-29 prior boundary `P(high) = 34/43`, the low-result
actions tie and EVSI is zero: no test result forces a change from the current optimal action.

Options are compared by expected net value against acting now. This is a choice of at most one test followed
by one action. The ordinary action CVaR and same-state regret remain separate: the model supplies no joint
signal distribution for counterfactual comparisons across different tests. Every option's source Claims,
assumptions and unknown-risk terms remain in the card's evidence and identity, including losing options.
This slice does not implement sequential testing or pass BD-227's Monte Carlo estimator acceptance gate.

### Check cited tables, reserve calculations and cutoffs

DESIGN (BD-340.S5/S8): inspect the existing workbook identities with their source citations and ledger
certificate, or supply explicit definitions for tables transcribed from registered PDFs:

```bash
cargo run -p fmine-cli -- check tables --synthetic --format markdown
cargo run -p fmine-cli -- check tables --issuer barrick --through 2026Q2 --json
cargo run -p fmine-cli -- check tables --issuer demo --definitions local/reserve_tables.toml --format markdown
cargo run -p fmine-cli -- check tables --issuer demo --definitions local/reserve_tables.toml --as-of 2025-03-01 --json
```

The definitions mode reads only its named documents and quantities. Each document must be registered under
the selected issuer and match its registered SHA-256 and size, even when no reading has yet been entered.
Values come from the existing two-reader transcription registry. Definitions contain membership and rules;
they contain no source values. Optional `--as-of YYYY-MM-DD` requires every declared document's registered
publication date to be on or before the cut. An unavailable document refuses the run; it does not disappear
from the declared population. Without the option, inspection is retrospective. JSON retains the cut, checked
document hashes and publication-date evidence; these dates do not authenticate an external timestamp or
prove when a transcription was entered. This invented example declares closing reserves as opening reserves plus a
signed net change minus a positively printed depletion:

```toml
schema = "fmine.table-identities.v1"
issuer = "demo"

[[identity]]
id = "gold_reserves_2025"
doc = "Synthetic_Reserves.pdf"
family = "gold_reserve_rollforward"
kind = "roll_forward"
total = "closing_reserves_2025"
total_period = "2025FY"
unit = "koz"
basis = "gold;full;contained"
construction = "rounded_once"
convention = "closed"

[[term]]
identity = "gold_reserves_2025"
quantity = "closing_reserves_2024"
period = "2024FY"
sign = 1
role = "opening"

[[term]]
identity = "gold_reserves_2025"
quantity = "depletion_2025"
period = "2025FY"
sign = -1
role = "move"

[[term]]
identity = "gold_reserves_2025"
quantity = "net_change_2025"
period = "2025FY"
sign = 1
role = "move"
```

The coefficient applies to the signed printed token: a depletion already printed in parentheses normally
uses `sign = 1`. A `kind = "sum"` identity uses `role = "member"` terms and explicitly declares each member's
period, including multi-year totals. One rounding construction applies to a family, and one tie convention
applies to a document. The parser rejects competing declarations. Shared cells are solved jointly; every
connected component reports a checked witness, a checked conflict certificate, or an explicit indeterminate
result. Conflict reports retain correction sets and implied intervals without selecting a corrected value.
Missing agreed operands remain listed identities. A mathematical conflict is a finding with exit zero;
unreadable/changed source files and reader disagreements preserve their typed refusals.

Reserve replacement uses `kind = "reserve_replacement"` in the same declarations file. It requires a named
`reserve_replacement.gold_oz@…` or `reserve_replacement.geo@…` definition from `registries/metrics.toml`,
`unit = "percent"`, `factor = 100`, and explicit, identical `numerator_unit` and `denominator_unit`.
Exactly two terms use `role = "numerator"` and `role = "denominator"`. The percentage and both cumulative
amounts must name the same three-year `YYYY-YYYY` window. For example, a gold declaration uses:

```toml
kind = "reserve_replacement"
definition = "reserve_replacement.gold_oz@barrick-fy2025"
total_period = "2023-2025"
unit = "percent"
numerator_unit = "moz"
denominator_unit = "moz"
factor = 100
price_deck = "none"
basis = "attributable;contained;metal=gold;ex_ad;deck=none"
```

These fields replace the corresponding identity fields in a complete declaration; they supply no numerical
inputs. GEO uses `metal=geo` and an explicit matching deck key in both `price_deck` and `basis`. It checks
already-converted GEO amounts and performs no implicit price or metal conversion. Each term's sign applies
to its original printed token; the resulting depletion interval must be strictly positive. Shared net-change
or depletion amounts can participate in sums or roll-forwards, and all those constraints are checked jointly.
Reusing the quoted percentage in another identity refuses because that would require a general nonlinear
constraint. JSON and Markdown retain the named definition, window, deck, tokens, locators and checked result.

Published gold and copper unit costs can use the same cited PDF workflow. Two explicit identity kinds fix
the native units and exact conversion factor:

| Kind | Cost numerator | Sold denominator | Reported rate | Exact formula |
|---|---|---|---|---|
| `gold_cost_per_sold_oz` | `usd_m` | `koz` | `usd_per_oz` | `1000 * cost / sold` |
| `copper_cost_per_sold_lb` | `usd_m` | `Mlb` | `usd_per_lb` | `cost / sold` |

For example, append this declaration to a definitions file after registering the named PDF and agreeing
the three source quantities through the existing two-reader registry:

```toml
[[identity]]
id = "gold_cost_rate_2025"
doc = "Synthetic_Costs.pdf"
family = "reported_cost_rate"
kind = "gold_cost_per_sold_oz"
total = "gold_cost_per_oz"
total_period = "2025FY"
unit = "usd_per_oz"
basis = "attributable;denominator=sold"
construction = "rounded_once"
convention = "closed"

[[term]]
identity = "gold_cost_rate_2025"
quantity = "gold_cost"
period = "2025FY"
sign = 1
role = "numerator"

[[term]]
identity = "gold_cost_rate_2025"
quantity = "gold_sold"
period = "2025FY"
sign = 1
role = "denominator"
```

All three assertions must match the declared document, period and basis. The selected kind requires the
numerator and denominator units shown above; it does not infer a cost definition or ownership conversion.
Costs retain their printed sign, and the sold interval must be strictly positive throughout its rounding
range. A cost subtotal can be shared with a sum identity so both relations are checked jointly. A quoted
rate cannot itself be reused in another identity, which would require unsupported bilinear reasoning.
The output retains the operands, exact factor, checked witness or conflict, and source hashes. Changing
PDF bytes, disagreeing readers or an unavailable publication date still refuses the inspection.

DESIGN boundary: these are arithmetic checks on issuer assertions. Different filing/deck vintages remain
separate documents. The command does not determine which vintage is true, classify reserves or establish
GATE-HALL-01. Its PDF report digest binds the declarations, readings and results; it is not a ledger anchor or
external timestamp. Authentic historical PDF readings and complete historical coverage remain open.

#### Report a complete annual reserve-replacement window

DESIGN (BD-063): the standalone command calculates a declared annual population, including when no
percentage headline is available. It preserves the annual printed sums, any separately printed aggregate
totals, and a separately qualified headline. Every year in the registered window must have both net-change
and depletion readings; a missing member cannot silently reduce the denominator.

```bash
fmine reserve replacement --issuer barrick \
  --bindings registries/reserve_replacement/barrick.toml --format markdown
fmine reserve replacement --issuer barrick \
  --bindings registries/reserve_replacement/barrick.toml --as-of 2026-11-01 --format json
```

The committed bindings contain selectors only. The command needs the registered PDF bytes and agreeing
readings in the local `registries/transcriptions/barrick.toml`; those inputs remain outside Git. Annual periods
use `YYYY`, aggregate/headline periods use `YYYY-YYYY`, and source strings must match the declared unit and
basis exactly. The Barrick report uses `Moz`, `attributable_gold_ex_ad` and `attributable_geo_ex_ad`. These are
explicit report-rule bindings; the table-checking command's `moz` and semicolon-separated basis grammar
is separate. Neither command silently relabels admitted readings from the other.

The calculation is the ratio of cumulative net changes to cumulative positive depletion, using the explicit
source sign convention. Exact rational results retain closed half-last-digit rounding bounds. Independently
printed aggregate constraints intersect the annual bounds without replacing either printed point. GEO keeps
its named annual reserve-price decks and already-converted amounts. A rounded headline is checked for
interval compatibility; an approximate headline supplies no numerical tolerance and stays unassessed.

JSON and Markdown retain definitions, every source token and locator, document hashes, publication evidence,
reader roles, deck dependencies and refused series. Computed series also expose canonical source Claims and
a cumulative result Claim. Their identities bind the original signed amounts, exact rounding enclosures,
definitions, annual price decks and publication evidence; the calculation is rechecked before those identities
are issued. Markdown names the same Claims as JSON. They remain issuer assertions and confer no calibration.
An unusable series returns `FMINE-REF-202` with the complete
report; an incompatible rounded headline is a cited finding. Source-byte changes and reader disagreements
retain their existing typed refusals. `--as-of` rejects every selected post-cut document before opening PDFs;
it uses current registered rules and does not establish a complete historical information set. The report
is retrospective, is not gate-eligible and conveys no reserve classification or disclosure authority.

#### Keep reserve filing vintages and their price decks separate

DESIGN (BD-335): optional `[[reserve_row]]` declarations add an issuer-reserve register projection to the same
`check tables --definitions` command. The following invented declaration can be appended to the example
above; a file containing only reserve declarations is also supported:

```toml
[[reserve_row]]
id = "closing_gold_2025"
doc = "Synthetic_Reserves.pdf"
quantity = "closing_reserves_2025"
subject = "Example Mine"
item = "closing"
reserve_year = 2025
period = "2025FY"
metal = "gold"
unit = "koz"
ownership = "full"
support = "contained"
convention = "closed"
deck = "quoted"
deck_period = "2025FY"
deck_basis = "reserve_prices"
gold_price_quantity = "reserve_gold_price_2025"
```

The reserve reading must independently agree on basis `gold;full;contained`, as well as its quantity, period,
unit, token and locator. An attributable basis explicitly names a fraction, for example
`gold;attributable:123/200;contained`. Opening rows use the preceding year's period. Supported items are
opening, acquisitions/divestments, depletion, net change and closing; signed printed adjustments remain signed.
The document's declared rounding convention also controls reserve-row comparisons.

A quoted price deck comes from independently agreed price readings in the **same document**, with the declared
deck period and basis. Gold prices use `usd_per_oz`; copper prices use `usd_per_lb`. GEO requires both price
quantities. No prices are entered into the definition file. Use `deck = "unstated"` and omit every price field
when the source does not state its deck; two unstated decks never establish equivalent assumptions.

JSON exposes `payload.issuer_reserves`, and Markdown adds reserve assertions, selections, comparisons and
unavailable rows. Every admitted row retains its source token, exact rounding interval, source hash, readers,
publication evidence, interpreted economic key and separately cited price deck. All rows are issuer assertions.
For the same issuer, subject, item, reserve year, metal, unit, ownership and support, the latest declared MD&A or
AIF filing is current. A later investor deck remains a cross-check. Competing filings on the latest date stay
ambiguous; a missing or unusable latest filing or price reading blocks fallback to an older filing.
An accepted value with a competing pending location refuses with both citations, even when the tokens match.

Earlier filings remain visible, including their disagreement with the current selection. Changed price decks
are reported separately from amount comparisons, and the latest filing still carries its own deck. Source
quantities on different units, ownership bases or contained/recoverable supports are never silently combined.
The projection retains the supplied declared population; it does not prove complete filing coverage, establish
an append-only history, normalize a changed price deck or create an `EstimateStatement` or reserve classification.

#### Check GEO against its printed price deck

DESIGN (BD-340.S4, BD-070): `[[geo_check]]` checks **GEO = gold + copper × (copper price / gold price)**
using the existing reserve rows and their cited prices. This invented addition assumes that `gold_net_2030`,
`copper_net_2030` and `geo_net_2030` have already been declared as reserve rows, with gold/GEO in `moz` and
copper in `mlb`. They must share their document, subject, item, year, period, ownership, support and convention.
The GEO row must declare `deck = "quoted"` and both `gold_price_quantity` and `copper_price_quantity`.

```toml
[[geo_check]]
id = "geo_conversion_2030"
gold_row = "gold_net_2030"
copper_row = "copper_net_2030"
geo_row = "geo_net_2030"

# Optional: another declared reserve row supplying both quoted price readings.
[[geo_candidate]]
check = "geo_conversion_2030"
row = "geo_net_2031"
```

The primary deck always comes from `geo_net_2030`. An optional candidate supplies a separate price hypothesis;
its unrelated reserve amount need not be available. Every price still requires agreed readings with its source
hash, locator, unit, period and basis. For an invented known answer, 10.0 Moz gold and 1,000 Mlb copper at
$1,500/oz and $3.00/lb give 12.0 Moz GEO. Those numbers belong in the source readings, never in these definitions.

`payload.issuer_reserves.geo_checks` retains the exact converted and reported intervals. A completed check has
`status = "checked"`; its `declared.fit` is `proper`, `points` for boundary-only contact, or `empty` for an
arithmetic mismatch. Alternative-deck discovery separately reports `found`, `ambiguous` or `no_fit` across the
explicit candidates and strict conventions. It never replaces the printed deck or the declared convention.
Distinct decks with the same price ratio remain ambiguous. Missing or unusable evidence gives `unavailable`;
a missing candidate price blocks discovery while retaining any usable primary-deck calculation.

#### Reproduce a technical report's cutoff from its cited assumptions

DESIGN (BD-274): the same command accepts `[[cutoff]]` declarations whose inputs come from one registered
`technical_report`. This complete, invented definition file selects four agreed quantities; it contains no
numerical assumptions:

```toml
schema = "fmine.table-identities.v1"
issuer = "demo"

[[cutoff]]
id = "example_cutoff"
doc = "Synthetic_Technical_Report.pdf"
subject = "Example Mine"
period = "effective_2030_12_31"
basis = "full;resource"
price_basis = "resource_price"
unit = "g_per_t"
recovery = "constant_percent"

[[cutoff_input]]
cutoff = "example_cutoff"
role = "published"
quantity = "reported_cutoff"

[[cutoff_input]]
cutoff = "example_cutoff"
role = "gold_price"
quantity = "resource_gold_price"

[[cutoff_input]]
cutoff = "example_cutoff"
role = "cost"
quantity = "processing_cost"

[[cutoff_input]]
cutoff = "example_cutoff"
role = "recovery"
quantity = "metallurgical_recovery"
```

Run it with `fmine check tables --issuer demo --definitions local/cutoffs.toml --format markdown`. The published
cutoff uses `g_per_t`, costs use `usd_per_t`, recovery uses `percent`, and the gold price uses `usd_per_oz`.
Every input must match the declared period. The gold price uses `price_basis`; the other inputs use `basis`.
Multiple `cost` rows explicitly add their separate costs. Optional `revenue_deduction` and `ounce_deduction`
inputs use `percent` and `usd_per_oz`, respectively.

The gold price is an exact quoted parameter; the other inputs retain their displayed rounding bounds.
For example, invented readings of $100.0/t cost, 50.0% recovery and $3,110.34768/oz gold reproduce 2.00 g/t.
`recovery = "tcm_over_gold"` instead requires `recovery_intercept` (`percent`), `recovery_slope` (`g_per_t`)
and `tcm` (`percent`) inputs. The alternative output unit is `oz_per_short_ton`; an optional `published_metric`
input checks its printed `g_per_t` equivalent. Cost inputs remain USD per metric tonne.

Results appear under `payload.checks.cutoffs.rows`: `reproduced`, `within_input_rounding`, or `conflict` for a
completed arithmetic comparison; `refused` with its reason when evidence or assumptions are unusable. JSON
and Markdown retain exact results, input bounds, citations and source hashes. A conflict remains a finding
with exit zero. Missing/altered document bytes and disagreeing readers retain the command's typed refusal.
Each successful case also carries `claims.inputs` and `claims.derived`. The derived Claim names every input
Claim as a parent; the JSON receipts include canonical bodies, value tags and source anchors. Original
tokens, precision, reader identities and declared semantics participate in those identities even when two
inputs have the same numerical value. The public Rust API `SourceRow::check_claims` reconstructs the complete
typed Claim chain, detecting changed values, ancestry or envelope fields. These Claims remain issuer
assertions. Refused cases retain available evidence and emit `claims: null`.
The result reproduces declared assumptions; it does not choose an operating cutoff or classify a reserve.
The existing seven `assure public` reference calculations remain explicitly `source_verified:false` until
their real documents and agreed readings are supplied. This command does not establish physical truth from
arithmetic agreement.

### Check cited numbers in a draft

```bash
cargo run -p fmine-cli -- lint draft.md --issuer barrick --through 2026Q2
cargo run -p fmine-cli -- lint draft.md --issuer barrick --through 2026Q2 --format markdown
cargo run -p fmine-cli -- lint draft.md --issuer barrick --through 2026Q2 --discover
cargo run -p fmine-cli -- lint draft.md --issuer barrick --through 2026Q2 --as-of 2026-08-11 --format markdown
```

DESIGN (BD-345): cited mode checks explicit equalities against canonical Claims from one hash-verified source
snapshot. Link the printed number and unit to its exact Claim ID, and state the subject, reporting period,
ownership basis and metric. For example, using the registered North Mara source Claim:

```markdown
North Mara Q2 2026 attributable AISC per ounce sold is [$2,577/oz](fmine:clm_f6e6c9b2ea703557f2a3b2f49b22b1aa).
```

The checker uses exact rounding intervals. It distinguishes value, period and basis mismatches, excessive
rounding, missing or ambiguous citations, and superseded Claims. It accepts only complete supported equality
predicates; negations, comparisons and other prose remain `unchecked`. A matching number cannot compensate for
a wrong site, period, metric or basis. `verified` means agreement with the cited source assertion, not that the
issuer assertion is true or that disclosure is permitted. `--through` selects registered source vintages.

DESIGN (BD-345.S4): `--as-of YYYY-MM-DD` adds an independent public-record assessment for each supported
equality. It reports the earliest matching dated registered assertion, its exact Claim, locator, file hash,
publication date and the date's stated basis. Other-basis matches, contradictory values, unsupported precision,
post-cut publications, undated sources and inadmissible evidence remain explicit. Competing attributable
shares cannot become an unambiguous disclosure merely because one amount matches. The equality verdict stays
visible beside this assessment; this does not authorize disclosure or establish legal compliance.

Workbook `[[source]]` rows may declare `published_at` and `published_at_basis` together, with a cited
`FACT: ` or `INFERENCE: ` basis. An omitted date stays unknown: the reporting quarter and retrieval date
cannot manufacture a publication date. Correcting these metadata changes the public-record report digest
without changing the workbook's numerical Claim.

The same assessment can derive additive quarters from dated cumulative disclosures: **Q2 = H1 − Q1**,
**Q3 = nine months − H1**, and **Q4 = full year − nine months**. Supported quantities are gold production
and sales in `koz`, and copper production in `kt`. For example, an uncited draft line
`Example Mine Q4 2030 100% basis gold production is 100 koz.` can be checked with:

```bash
cargo run -p fmine-cli -- lint draft.md --issuer <registered-issuer> --as-of 2031-03-01 --discover
```

Every required input must have a registered publication date at or before the cut and the same explicit
ownership and perimeter basis. The calculation uses displayed rounding intervals: subtracting `200.0`
and `100.0` gives the exact enclosure `[99.9, 100.1]`, so a draft claiming `100.0` has more precision than
those inputs establish. All competing source pairs, their Claims, hashes and publication evidence remain
in JSON and Markdown. Future and undated disclosures cannot fill a missing input; dashes cannot become zero.
Rates such as AISC, grades and recovery are not subtracted. A successful `derivable` result retains the
separate direct-publication assessment and warns that the derived figure may itself be new information.
It is arithmetic over issuer assertions, not permission to disclose them.

Complete quarter sets can also establish **H1 = Q1 + Q2**, **9M = Q1 + Q2 + Q3**, and
**FY = Q1 + Q2 + Q3 + Q4** for those
same additive quantities. For example, `Example Mine H1 2030 100% basis gold production is 300 koz.`
can be supported by dated Q1 and Q2 disclosures of `100.0` and `200.0`: their exact sum enclosure is
`[299.9, 300.1]`. A draft stating `300.0` would claim more precision than those inputs establish.
The `quarter_sum` report preserves the interpreted input Claims, canonical derived Claim and complete
source ancestry. Every quarter must be present on the same subject, metric, ownership and scope basis;
competing eligible vintages stay ambiguous. Missing or undated quarters cannot become zero, and a
supporting sum cannot conceal a directly disclosed contradiction. Rates remain outside these arithmetic rules.

DESIGN (BD-345.S4): explicit **attributable production and sales** can also be checked against a full-mine
amount and its printed ownership share. For example, a full-mine amount of `100 koz`, printed to the nearest
koz, and a 60% share imply `[59.7, 60.3] koz`. This supports a draft stating `60 koz`; it does not establish
the extra precision in `60.0 koz`. The `ownership_scaling` result retains the amount Claim, every admitted
share Claim, the exact derived Claim and the dated source evidence. Ownership must be printed for the same
site, period and perimeter in the exact source vintage supplying the full-mine amount. Conflicting shares
and competing amount vintages remain ambiguous; future, undated, missing or registry-corrected share evidence
cannot authorize the conversion. Direct disclosed matches and contradictions keep priority. This is a
full-mine-to-attributable arithmetic check for gold production/sales and copper production; reverse scaling,
unit-cost scaling and a new issuer disclosure are not inferred.

DESIGN (BD-345.S4): gold **AISC and explicitly by-product total cash cost per sold ounce** can be derived
from the same dated source table's cost numerator in US$ millions and attributable ounces sold in thousands:
`1000 * cost / sold`. Integer and decimal disclosures use their own displayed precision: each operand's
rounding interval extends by half its printed step. The entire sold-ounce interval must be positive.
The original printed and registered ownership evidence
stays attached; the declared identity interpretation never invents a printed ownership share.

For synthetic disclosed values of US$150M and 100 thousand ounces, the exact rate enclosure is
`[299000/201, 301000/199] USD/oz`. The draft sentence
`Example Mine Q1 2030 attributable AISC per ounce sold is $1.5k/oz.` is coarse enough for that enclosure;
`$1,500/oz` claims more precision than those inputs establish. The `cost_rate` section retains the formula,
identity basis, exact derived Claim and both original source Claims. The derived Claim retains their
issuer-assertion evidence class. Stored workbook values remain visible, while displayed values determine
the rounding bounds. Signed costs stay signed; zero, negative, missing or nonnumeric sales cannot provide
a denominator. Competing operands remain ambiguous, and direct disclosed matches or contradictions retain
priority. Co-product TCC, cost-of-sales bridges and produced-ounce denominators require their own explicit
constructions and are outside this rule.

Copper **AISC, C1 cash cost and cost of sales per pound sold** use the separate `N/Q` identity when the same dated table
explicitly supplies a cost numerator in US$ millions and sales in millions of pounds. Both operands must
carry the same explicit ownership basis. Decimal costs and sales retain their independently printed
rounding intervals, so a coarse disclosure cannot establish extra precision in a draft. The result names
`USD/lb`, its copper identity and both source Claims; gold's thousand-ounce scale is never applied to it.
Missing dollar numerators, sales in tonnes, unprinted ownership or competing source assertions remain
unavailable, refused or ambiguous. This support does not assert that a particular issuer publishes all
required numerator rows.

For example, the supported draft equality
`Example Mine Q1 2030 attributable cost of sales per pound sold is $3.00/lb.`
can be checked against the same column's printed `Costs ($ millions): Cost of sales` and
`Sales (millions of pounds)` rows. Printed values of `300.00` and `100.00` imply the exact enclosure
`[59999/20001, 60001/19999] USD/lb`, which supports the draft's cent precision. The output identifies
the `cos_per_lb` family and retains both source locators, hashes and original printed values.
Gold cost-of-sales derivation still requires an explicit bridge between its consolidated numerator
and attributable sales; the copper rule does not supply that bridge.

Nine-month cumulative disclosures can also be checked directly in cited or discovery mode. The grammar
recognizes `9M 2030`, `20309M`, and `nine months of 2030`, keeping them distinct from Q3, H1, and FY.
An undated `9M` remains a numeric nine-million amount. Every derived nine-month total requires all three
dated, compatible quarter assertions; a missing quarter or competing eligible vintage remains explicit.

DESIGN (BD-345.S3): `--discover` also searches for source Claims when a supported equality has no Claim link.
The subject, metric, reporting period, ownership basis and unit must still be stated explicitly. A unique
candidate goes through the same value, rounding and staleness checks as a cited Claim. Multiple matching
Claims remain ambiguous, with their source evidence retained; the draft's amount never selects a preferred
vintage. Existing links keep their cited-mode checks. This bounded grammar does not interpret arbitrary prose.

Discovery supports AISC, gold TCC, copper C1, gold/copper production, processed/head grade and whole-site recovery.
Copper C1 requires an explicit per-pound-sold denominator and `US$/lb` or `$/lb`; it does not inherit gold
TCC's by-product convention. Copper production requires `kt` and an explicit copper metric. Historical site
production in millions of pounds stays unchecked; no unit conversion is inferred. Grade uses
explicit `g/t` (or `grams per tonne`) or percent units; recovery uses percent. For example, an invented draft
`Site A Q1 2030 ownership basis unprinted recovery is 91.5%.` checks the source's displayed percentage-point
interval, including when the workbook stores `0.915` with a percent format. It never compares `91.5%` with a raw
fraction as though they were the same unit. Bare/mined grade and circuit recovery remain outside the workbook
grammar. Independently labelled general-prose accuracy remains unmeasured.

#### Check a named reserve-replacement percentage

DESIGN (bounded BD-345.S3/BD-063): reserve-replacement equalities use the authenticated annual selectors
from the standalone reserve report. Select the bindings and a publication date explicitly:

```bash
fmine lint draft.md --issuer barrick --as-of 2026-11-01 \
  --reserve-bindings registries/reserve_replacement/barrick.toml --discover --format markdown
```

Each supported standalone line states company scope, attributable gold or GEO, the complete year window,
exclusion of acquisitions and divestments, and the backtick-quoted reserve definition. GEO also names its
deck definition with `using`. For example, these statements use **invented** fixture definitions and values:

```text
Company attributable gold reserve replacement for 2030-2032 excluding acquisitions and divestments under `replacement.gold@fixture` is 200%.
Company attributable GEO reserve replacement for 2030-2032 excluding acquisitions and divestments under `replacement.geo@fixture` using `geo.deck@fixture` is 450%.
```

Cited mode uses `[200%](fmine:clm_...)`, with the exact cumulative Claim ID from `reserve replacement`.
Discovery retains every matching bound series and refusal; equal amounts never select a preferred source.
The entire exact source-rounding enclosure must fit the draft's printed half-step interval for `verified`.
Disjoint intervals give `value_mismatch`; overlap without containment gives `rounding_not_established`.
Approximate language and missing basis qualifiers stay unchecked.

The separate `reserve_replacement` result retains the full source report, annual decks, canonical Claims,
reader roles, hashes and publication cut. Missing or altered optional reserve evidence stays visible while
the workbook findings remain available. This checks arithmetic and declared precision; it supplies no
reserve classification or disclosure permission. The actual PDF bytes and agreeing local readings remain
required, and authentic issuer coverage is still an open gate.

The command reads the draft locally, writes no files, and emits number spans and verdicts rather than whole
sentences. JSON includes the draft and Claim-population digests and the source snapshot root. Findings are data:
a completed lint exits zero even when it finds mismatches. Missing or altered registered source files refuse.
Use `--synthetic` with Claims emitted by the built-in synthetic assurance run for an invented demonstration.
Relative draft paths are resolved against `--workspace`, which defaults to the current workspace.

With `--as-of`, explicitly marked forward statements also receive a separate `forward_guidance`
assessment. For example, these invented lines use the supported grammar:

```text
Example Mine FY2030 100% basis gold production forecast is 125 koz.
Copper Mine FY2030 100% basis AISC guidance is $3.40 - 3.75/lb per pound sold.
```

The comparison uses the latest agreed, registered guidance published by the date, retaining its page,
hash, ownership basis and canonical source Claims. Conflicting latest statements and incomplete readers
stay unresolved. Point and range endpoints are compared exactly; this does not verify a forecast or
permit disclosure. Document-only registries work, and an optional guidance failure is reported alongside
the historical numerical check.

### Compare issuer guidance with reported annual outcomes

```bash
cargo run -p fmine-cli -- track-record --issuer barrick --through 2026Q2
cargo run -p fmine-cli -- track-record --issuer barrick --site north-mara --format markdown
cargo run -p fmine-cli -- track-record --issuer barrick --site north-mara --price-subject barrick --format markdown
cargo run -p fmine-cli -- track-record --synthetic
```

DESIGN (BD-341): this retrospective report binds agreed, page-cited guidance to canonical annual source
Claims. It verifies registered document and workbook hashes, matches the site, fiscal year, unit and ownership
basis, and uses the original annual release when that release is registered. Later appearances remain visible
without replacing the selected outcome. When only a comparative publication is available, its results are
labelled and summarized separately. `--through` selects workbook releases; it is not an information-availability
cut or evidence that a forecast was registered before an outcome.

Every guidance document remains a separate vintage. Missing annual cells, withheld periods, divestments and
incompatible bases remain unresolved members. The report shows exact normalized deviations for ranges,
rounding-boundary ambiguity, hit and miss counts, and whole-cohort completion bounds. Its separate binomial
reference interval assumes independent, identically distributed hits; it does not estimate common year effects,
effective sample size, a bias confidence interval or forecasting skill. Guidance ranges receive no invented
probability level, and the report does not rank sites.

Retrospective scores and site-history summaries also carry canonical Claim IDs, source parents, evidence and
knowledge state. The JSON includes the original guidance statement, its interpreted target, the interpreted
outcome, each score and each summary as Claims, while preserving the existing numeric fields and row-level
`claims` object. Claims expose exact value tags, canonical body bytes and citations; interpreted outcomes point
to their original workbook Claims. Correcting a guidance publication date changes its evidence identity even
when the arithmetic is unchanged. Markdown shows the Claim links, included years and exact rational binomial
bounds; unresolved years remain in the denominator. Hypothetical arithmetic on an explicit Unknown/Assumed
number preserves that state. Other Claim admission failures retain their typed refusal.

The public command requires `registries/transcriptions/<issuer>.toml` with agreed readings from registered
documents. These transcriptions and source documents remain outside git. Quantity keys follow
`guidance_<site>_<metric>`, with site hyphens written as underscores; supported metrics are `production_koz`,
`production_kt`, `aisc_usd_per_oz`, `aisc_usd_per_lb`, `tcc_usd_per_oz`, `c1_usd_per_lb` and the three
capital-expenditure scopes below.
Production basis is `100%` or `attributable:n/d`. Cost basis also declares its definition, for example
`attributable:4/5;aisc=issuer-reported-per-sold-oz`; the analogous per-sold-lb, `tcc` and `c1` definitions are
explicit. Registered runs require complete two-reader agreement across the selected guidance namespace.
The retrospective command also refuses unsupported guidance quantities; the dated comparison modes retain
them as diagnostics. The pure source-binding API can retain pending guidance readings alongside admitted rows.
Missing or changed source bytes refuse. The synthetic command is an invented known-answer demonstration
through the same binding and scoring code.

Gold and copper site capital guidance uses `usd_m`, explicit ownership and one of these reported-line definitions:

| Metric | Required suffix on the ownership basis | Annual source row |
|---|---|---|
| `capex_usd_m` | `;capex=issuer-reported-total` | Total capital expenditures |
| `capex_sustaining_usd_m` | `;capex=issuer-reported-minesite-sustaining` | Minesite sustaining |
| `capex_projects_usd_m` | `;capex=issuer-reported-projects` | Projects |

For example, `attributable:3/4;capex=issuer-reported-total` compares total capital expenditure on the
same reported ownership basis. Ordinary, `--as-of` and `--cohort-as-of` reports all support these scopes.
They select the printed annual row directly and retain its sign, rounding interval, source hash and Claim
ancestry. A missing original total stays unresolved even when its components or a later comparative exist.
The three scopes remain separate; there is no inferred cash/accrual or lease-treatment bridge, currency
conversion, ownership conversion or royalty-price adjustment.

#### Restrict original-outcome cohorts to an evidence date

```bash
cargo run -p fmine-cli -- track-record --issuer barrick --cohort-as-of 2026-10-31 --site north-mara
cargo run -p fmine-cli -- track-record --issuer barrick --cohort-as-of 2026-10-31 --through 2026Q2 --format markdown
cargo run -p fmine-cli -- track-record --synthetic --cohort-as-of 2032-12-31 --format markdown
```

The optional `--cohort-as-of` selects a dated projection through the same checked source binder. It uses registered
document publication dates and each workbook's cited publication date. A workbook without a publication date
uses the strict upper end of its registered retrieval interval as a fallback availability bound. Later
workbooks are excluded before their files are opened, and unrelated AISC workbooks do not block Mine Stats
outcomes. A cut with no eligible outcome workbook still retains the guidance cohort and its unresolved members.
The report identifies the selected sources' availability evidence; a wholly unavailable workbook population
has no invented availability date. The retrospective command above does not claim an information-availability
date. Neither mode proves that today's registry existed at the historical cut, authenticates an external
timestamp or registers a forecast. Units, ownership bases, canonical Claim identities and metric definitions
are checked in both. An unprinted ownership basis is never inferred to mean full ownership.

Dated summaries group sites by metric, annual period, canonical basis and guidance publication date. They retain every admitted
member, including missing original outcomes. Later comparative appearances have separate scores and never fill
the original cohort. Revisions remain separate; an unchanged range on a changed ownership basis is a revision.
The retrospective view retains its per-site history summaries across years and separate comparative populations.
The separate `--as-of` mode selects the earliest registered guidance and compares first-published and latest
outcomes at its evidence cut. `--cohort-as-of` preserves each guidance publication as its own cohort; the two
date selectors cannot be combined.

For a guidance range `[L, U]`, the exact normalized position is `(outcome - midpoint) / half_width`.
The dated synthetic example has three sites with `[100, 120]` guidance: an original outcome of `125` gives exactly
`3/2`; another sits on a rounding boundary, and the third lacks an original outcome. Missing members remain in
whole-cohort completion bounds. Rounding-aware bounds additionally preserve boundary ambiguity. Point guidance
has no normalized range position. Exact rational IID reference bounds are retained alongside display projections;
they do not estimate dependence, effective sample size, bias confidence intervals or forecasting skill.

Inclusive one-sided guidance is also accepted from agreed printed tokens: `<= 120`, `≤120`,
`at most 120`, `>= 120`, `≥120` and `at least 120`. Hit and miss counts include these targets, with
exact inclusive-boundary and printed-rounding ambiguity checks. They have no midpoint, normalized deviation,
opposite endpoint or implied probability. Finite-interval forecast baselines still require a point or a
two-ended range; strict `<`/`>` and approximate wording refuse. Negative production thresholds refuse,
while signed cost thresholds remain valid. The original token and its direction must agree between readers.

Historical guidance transcriptions and the full M0/BD-341 data, dependence and coverage gates remain open.
This cohort mode keeps unconditional scores; the paired sensitivities below require their own cited price
and royalty evidence. JSON is the default (`--json` also selects it); Markdown includes comparisons and citations.

Track-record source loading selects guidance, price-assumption and realized-price inputs before checking document
bytes and reader agreement. Adding incomplete or unusable optional
technical readings does not invalidate these reports. Unsupported guidance keys and every reader at a
selected locator remain in scope; the complete registry still needs valid syntax and registered identities.

For AISC, the report can show a cited price sensitivity beside the unconditional score:
`p = royalty * (1 - assumed_price / realized_price)`. It requires every declared original quarter from
2024Q1 through 2026Q2, shortened by `--through` or the last completed quarter at `--as-of`, to admit a common
royalty/price coefficient.
The scored year's ratio must also be compatible. Missing quarters, incompatible evidence and a missing stated
price assumption leave the result unsplit with an explicit reason. TCC and C1 are never adjusted.

Two-reader price entries use quantity keys
`aisc_price_assumption_<site>_<yyyyfy>_<unit>` and
`realized_price_<subject>_<yyyyfy|yyyyqn>_<unit>`, with underscore-separated site/subject names,
lowercase period in the key, `usd_per_oz` or `usd_per_lb`, and the matching uppercase `YYYYFY` or `YYYYQn`
in the entry's `period` field. The price assumption must cite the same document as its guidance row.
An explicitly shared assumption uses `aisc_price_assumption_all_sites_<yyyyfy>_<unit>` and retains its
single physical citation. Exactly one site-specific or shared assumption may apply; overlapping entries
leave the sensitivity unsplit with both citations.
Realized-price documents must be published at or after the claimed quarter or year ends.
Prices retain their stated commodity-price basis; royalties retain their canonical site ownership basis.
Site prices are used by default. `--price-subject barrick` explicitly selects that company's price series as
a proxy and labels it in the result; no company fallback is inferred. JSON and Markdown include the paired
scores, exact rounding bounds, source cells/pages and hashes. This is a descriptive sensitivity with no causal
or controllability conclusion.
Incomplete optional price transcriptions leave the unconditional track record available. The price result
stays unsplit and names pending or conflicting locators, reader roles, quantity identities and checked
document hashes. Guidance must still have complete two-reader agreement, including when readers disagree
about whether a row is guidance. Missing or altered selected guidance/price document bytes retain their
existing refusals. Unrelated technical sources do not block either track-record mode.
The explicit `--as-of` report also supports price sensitivity. It bounds both the price documents and
completed quarterly history by the requested evidence cut, and assesses first-published and latest registered
outcomes separately.
`--cohort-as-of` keeps unconditional scores and cannot be combined with `--price-subject`.

### Mirror two reporters' gold, copper and capital expenditure

```bash
cargo run -p fmine-cli -- mirror --synthetic
cargo run -p fmine-cli -- mirror --synthetic --format markdown
cargo run -p fmine-cli -- mirror --synthetic --metric copper-production --format markdown
cargo run -p fmine-cli -- mirror --synthetic --metric copper-sold --json
cargo run -p fmine-cli -- mirror --synthetic --metric capex --format markdown
```

DESIGN (BD-346): the mirror checks whether two reporters' printed gold or copper quantities admit a common 100%
quantity under their independently cited ownership and complete dated operating perimeter. The synthetic
demo uses invented evidence. To use registered filings, explicitly select both issuers and documents;
the following names are placeholders for your local registries:

```bash
cargo run -p fmine-cli -- mirror --asset example-jv --period 2025 --as-of 2026-02-01 \
  --left-issuer issuer_a --left-document Left.pdf \
  --right-issuer issuer_b --right-document Right.pdf --metric production --format markdown
```

`--metric production` is the default; `--metric sold` checks ounces sold separately. Periods are calendar
years (`2025` or `2025FY`), quarters (`2025Q1` through `2025Q4`) or halves (`2025H1`, `2025H2`).
`--metric copper-production` and `--metric copper-sold` select copper in kilotonnes (`kt`). Copper outputs
carry `interval_kt` throughout, including the common quantity, while gold retains `interval_koz` and its
existing Claim identities. The command requires exact selected units; pounds and gold ounces cannot
substitute for copper tonnes. Both copper selectors also work with `--synthetic`.
Capital selections are `capex-sustaining`, `capex-project`, `capex-total`, or `capex` for all three scopes
and their joint footing check. `--synthetic` is exclusive with source selectors; the capex demo accepts
`--synthetic --metric capex`. JSON is the default; `--workspace` selects the local root.

Each issuer uses the existing `registries/sources/<issuer>.toml` and ignored
`registries/transcriptions/<issuer>.toml`. A sources registry can contain only validated `[[document]]`
rows plus its required metadata and `[[subject]]`; no dummy workbook is needed. Select a document of kind
`mda`, `aif` or `partner_filing`, with a cited `FACT: ` publication-date basis. Its `published_at` must be
on or after the requested period ends and on or before `--as-of`. The run verifies the selected bytes and
every reader at each selected locator, including readers who used a different quantity key. Missing,
altered, pending or conflicting selected evidence refuses. Unselected documents do not supply fallback data.

Transcription keys include the asset with hyphens replaced by underscores and a lowercase period. For
`example-jv`, FY2025 production uses:

| Quantity key | Unit | Basis prefix |
| --- | --- | --- |
| `mirror_example_jv_2025fy_gold_produced_100_koz` | `koz` | `100%` |
| `mirror_example_jv_2025fy_gold_produced_attr_koz` | `koz` | `attributable` |
| `mirror_example_jv_2025fy_ownership_percent` | `percent` | `equity` |

For sales, replace `gold_produced` with `gold_sold`; the ownership key stays the same. Every entry's
`period` is the corresponding uppercase `2025FY`, `2025Q1` or `2025H1` form. At least one full-mine or
attributable quantity is required from each issuer, and both assertions participate when supplied.
Attributable quantities require the separate ownership row at its own page/table/row/column. Its printed
point is an exact stated percent, converted rationally to a share; a range or missing share is not inferred
from the other partner. Two reporters need not own the entire venture.

For copper, use `copper_produced_100_kt` and `copper_produced_attr_kt` after the same asset/period prefix;
sales use `copper_sold_100_kt` and `copper_sold_attr_kt`. Their transcription unit is exactly `kt`, and
the separate `ownership_percent` row and dated operating-perimeter evidence use the same grammar.
For example, independently reported attributable quantities of `60.00 kt` at 60% and `40.00 kt` at 40%
can be checked against one full-mine quantity without assuming that the reporters use the same precision.
The comparison preserves each printed rounding interval and selects no corrected source value.

Each row's basis appends the following exact fields. This is an **invented grammar example**, not an
assertion about any mine:

```toml
basis = "attributable;perimeter=group;members=mine-a+mine-b;valid=2025-01-01/2025-12-31;scope_page=2;scope_table=Scope;scope_row=Operations;scope_col=2025"
```

Use `100%` or `equity` instead of `attributable` for the other roles. `members` is the complete operation
list, with distinct slug names separated by `+`. The inclusive `valid` dates must cover the whole reporting
period. The scope fields cite the page and labels in the same selected document that establish that
perimeter; both readers must agree on this context. An ownership row must match its attributable row's
dated perimeter. The reporters must name the same perimeter and complete member set; broader and narrower
sets are not silently intersected. A change of reporting region alone establishes no change in operations.

Printed quantities retain exact closed half-last-place rounding bounds. Dividing attributable intervals by
the cited shares produces the full-mine comparison. A common interval carries a checked witness; a conflict
carries a checked exclusion certificate and checked inclusion-minimal source sets, without claiming that
the set list is exhaustive. Indeterminate attribution remains explicit. Printed zero keeps its original
rounding interval, while the common physical quantity is constrained to be nonnegative. JSON and Markdown
retain both reporters, tokens, ownership and scope citations, source hashes and publication-date bases.
All authority flags remain false, with no issuance anchor or authority certificate. Agreement does not
establish physical truth, select a correct reporter or authorize disclosure.

**Capital expenditure (BD-346.S4).** Use the same explicit source selection with `--metric capex` to
check sustaining, project and total capital together. The three single-scope selections work when only
that line is available. Capital amounts use `USD_m`; no currency conversion is inferred. For example:

| Quantity key | Unit | Basis prefix |
| --- | --- | --- |
| `mirror_example_jv_2025fy_capex_sustaining_100_usd_m` | `USD_m` | `100%` |
| `mirror_example_jv_2025fy_capex_sustaining_attr_usd_m` | `USD_m` | `attributable` |
| `mirror_example_jv_2025fy_capex_project_attr_usd_m` | `USD_m` | `attributable` |
| `mirror_example_jv_2025fy_capex_total_attr_usd_m` | `USD_m` | `attributable` |

All scopes accept both `_100_usd_m` and `_attr_usd_m`. The separate ownership row uses the same key and
grammar as the gold mirror. Each capital amount appends explicit timing and lease treatment plus the
same-document page and labels supporting that definition. This remains an invented grammar example:

```toml
basis = "attributable;perimeter=group;members=mine-a+mine-b;valid=2025-01-01/2025-12-31;scope_page=2;scope_table=Scope;scope_row=Operations;scope_col=2025;timing=cash;leases=excluded;definition_page=2;definition_table=Capital policy;definition_row=Cash capital excluding leases;definition_col=2025"
```

Timing must be `cash` or `accrual`; leases must be `included` or `excluded`. Missing or unstated bases,
uncited definitions, and unequal bases refuse with the missing parameter identified. No cash/accrual,
lease, purchase-accounting or asset-to-asset adjustment is manufactured. Signed capital corrections retain
their sign and source rounding; they do not inherit gold's nonnegative physical constraint.

The all-capex result keeps `scope_checks` and `joint` separate. Scope checks compare each printed amount's
own half-last-place interval. The declared joint **component-sum** check requires displayed
`total = sustaining + project` on each complete reporting basis, then derives the total's interval from
those components while checking all ownership identities together. An individually plausible set of
capital lines can therefore have a conflicting joint footing. A missing complete triple leaves the joint
assessment explicitly unavailable. JSON retains each exact witness or exclusion certificate and the
definition citations; Markdown explains both constructions. Arithmetic conflicts are findings with exit
zero; unusable sources or incompatible bases retain their nonzero refusal exits.

Real partner-filing coverage for NGM, Pueblo Viejo and Kibali remains open, including independently read
ownership, capital definitions and dated perimeter evidence. The unresolved NGM vend-in dates and shares
are not supplied by defaults. The current capex adapter reads agreed registered document transcriptions;
automatic workbook-to-partner pairing, cited definition bridges, cost/accounting bridges and full BD-346
acceptance remain open.

### Assess a frozen forecast cohort

```bash
cargo run -p fmine-cli -- forecast assess --ledger tests/forecasts/cohort_synthetic.toml \
  --cohort synthetic-demo --as-of 2026-10-31T00:00:00Z
cargo run -p fmine-cli -- forecast assess --ledger tests/forecasts/cohort_synthetic.toml \
  --cohort synthetic-demo --as-of 2026-10-31T00:00:00Z --selector latest-as-of
```

DESIGN (BD-343): this read-only adapter replays an ordered transcript through the existing forecast ledger.
Registrations, cohort freezes, published or missing outcomes, restatements and saved assessments retain their
order. It reports every cohort member, selected outcome citations, missing reasons, observed scores and
whole-cohort completion bounds, plus the frozen assumptions behind the effective sample size. The default
selector preserves the first published outcome; `latest-as-of` reports restatements separately.

The committed example is explicitly synthetic. Transcript times and citations are supplied inputs, not
independently verified time proofs or source documents. This command neither issues the FY2026 forecast set
nor replaces `xtask forecast-record-verify`; it makes the cohort and scoring kernel usable while retaining those
boundaries. Relative transcript paths are resolved against the selected workspace.

Empirical forecasts preserve a supplied skewed or multimodal distribution without fitting a Gaussian.
Use `kind = "empirical"` and, for example, `samples = "350,350,450,650"` in a forecast row. Each sample has
equal probability; repeats retain their mass. The engine scores the complete distribution with CRPS and
uses inverse-ECDF central intervals without interpolation. A cohort containing Gaussian and empirical
forecasts gets one CRPS denominator, including unresolved members. The sample limit is 4096; live journal
operations also retain their 64 KiB byte limit. Sample order is frozen into the record identity, while
reordering has no effect on distribution scores. This is a univariate forecast, not a joint scenario model.

The following invented example has an observed CRPS of 37.5 and retains an uncalibrated, descriptive result:

```bash
cargo run -p fmine-cli -- forecast assess --ledger tests/forecasts/empirical_synthetic.toml \
  --cohort empirical-demo --as-of 2031-02-17T00:00:00Z
```

DESIGN (BD-049): forecasts can also freeze `family`, `author_role`, `domain`, `horizon`, `site`, `quantity`
and `unit`. All seven fields are required together at registration; the metadata becomes part of the forecast
identity. Use an agent/model identifier or a role for the author. Omission preserves legacy identities and
does not assign new labels to old forecasts. A tagged cohort requires one exact family, quantity and unit;
different units and legacy unclassified forecasts need separate cohorts. Public-source resolution additionally
requires the declared site, quantity and unit to match the prospectively frozen public target.

Assessment JSON includes a separate `segmented_assessment`, partitioned by the full model and metadata key.
Each group retains its forecast IDs, unresolved reasons, coverage and proper-score completion bounds.
These are descriptive comparisons, with no inferred independence, subgroup calibration verdict or ranking.
Saved journal reports render the same groups in Markdown and keep their original outcome selection after
later restatements. The optional registration uses journal operation v7; earlier operation versions reject
metadata instead of silently ignoring it.

DESIGN (BD-048/049): every saved assessment also has a separately digested `diagnostics` projection.
Gaussian forecasts retain each selected outcome's probability integral transform (PIT) and a fixed-decile
histogram. Bernoulli forecasts retain their original probability bins, complete and resolved-only mean
probabilities, observed event frequencies and completion bounds for pending outcomes. Compare the resolved
mean probability with the observed event frequency; those two quantities use the same population. Empty-bin
averages remain absent, and pending forecasts never disappear from the denominators. Empirical distributions
retain exact counts below, equal to and above the selected outcome, with CDF bounds
`[below/n, (below+equal)/n]`. Separate decile bins preserve ties through definite/possible counts and missing
outcome completion bounds; they do not invent a randomized PIT or pool empirical ranks with Gaussian PIT.
Sparse quantiles and standalone intervals remain unsupported for that CDF projection. JSON and saved-journal
Markdown preserve the selected sources and parent assessment digest, so later outcomes or restatements cannot
rewrite an earlier diagnostic report. These are descriptive diagnostics, not calibration verdicts, confidence
intervals or claims of independent observations.

DESIGN (BD-048/049): the sibling `interval_coverage` projection reports central **50%, 80% and 95%** coverage.
Gaussian and empirical distributions supply their central quantiles; explicit intervals and quantile tables
contribute only exactly declared levels. Each level shows its supported population, unresolved and unsupported
counts, hits, lower misses and upper misses. Completion bounds retain every supported unresolved forecast;
an empty denominator is unavailable. Every member remains visible with its inclusive interval or unavailable
reason. Empirical ties may give more than the nominal mass. The separate CVG1/v1 digest binds the original
assessment and selection without changing existing forecast, cohort, assessment or DGN1 diagnostic identities.

The separate `quantile_losses` projection scores **every supplied quantile**, including interior levels such
as the median. It reports twice-pinball loss, so the median's loss equals absolute error, while preserving the
original frozen central interval score. Each exact level keeps its own complete population and unresolved
outcome bounds under the frozen domain. Scores are checked rational numbers of the recorded binary values;
capacity failures remain `arithmetic_indeterminate` and never cause a partial average. An absent outcome
domain leaves the upper completion bound unbounded. QNT1/v1 binds the saved assessment and its selected
evidence; later restatements cannot alter an earlier projection. Assessment JSON and saved-journal Markdown
include the level summaries and individual scores without implying calibration or predictive skill.

The synthetic example below keeps an unresolved northern forecast in its two-member group: the observed
interval score is 10, while the final group mean can range from 10 to 260 under its declared outcome domain.
The southern forecast remains a separate group with an interval score of 110.

```bash
cargo run -p fmine-cli -- forecast assess --ledger tests/forecasts/segmented_synthetic.toml \
  --cohort synthetic-segments --as-of 2031-01-10T00:00:00Z
```

Sequential coverage monitoring is an optional policy frozen with the cohort. In the synthetic interval
fixture `tests/forecasts/cohort_synthetic.toml`, add these fields inside its `[[cohort]]` row, before the outcome rows:

```toml
monitor_alarm_alpha = "0.05"
monitor_support = "Synthetic quarterly unit-cost intervals"
monitor_conditional_null = "Assumed exact conditional 80% coverage at every frozen step"
monitor_order = "north-unit-cost,south-unit-cost,central-unit-cost"
```

All four fields are required. The order must contain every member exactly once. The mathematical null is
**P(interval hit | past) = 1-alpha**, at the declared support; the command retains this assumption without
claiming to verify it. Gaussian forecasts, explicitly probability-bearing intervals and exact central
quantiles are supported. Empirical and Bernoulli forecasts do not satisfy this continuous-coverage grammar.

The assessment reports log e-values, their running maximum, the first alarm, and each original value,
interval, hit, citation and event digest. Monitoring uses original publications even when `latest-as-of`
scoring selects a restatement. It stops at the first unresolved member rather than admitting later resolved
outcomes out of order, and reports the blocked member and deferred counts. An alarm does not recalibrate
the forecasts. In the example order, the missing south member blocks the already resolved central member.

DESIGN (BD-051): Gaussian cohorts can also freeze a **two-sided residual CUSUM** to detect sustained
positive or negative shifts. Supply all six fields in the cohort before outcomes:

```toml
cusum_reference_value = "0.5"
cusum_threshold = "3"
cusum_support = "Synthetic ordered site outputs"
cusum_reference_assumption = "Standardized original residuals follow the declared Gaussian reference model"
cusum_tuning_basis = "Illustrative only; no false-alarm or average-run-length validation"
cusum_order = "f1,f2,f3"
```

The residual is `(original outcome - frozen mean) / frozen standard deviation`. The directional sums are
`max(0, previous + residual - k)` and `max(0, previous - residual - k)`, with an alarm at either sum reaching
`h`. Reports retain each residual, source citation and event digest, both sums and maxima, and each first
crossing. Missing members stop the sequence; later resolved members remain deferred. Restatements can change
the surrounding score but cannot replace the monitor's original outcomes or erase an alarm. Thresholds and
reference assumptions are declared inputs: this diagnostic supplies no false-alarm probability, validated
average run length, anytime guarantee, cause or recalibration. Coverage monitoring can be enabled alongside it.

The invented example has residuals `[2, 2, 2]`, giving upper sums `[1.5, 3, 4.5]` and a first crossing at 2.
Its early assessment stops at the missing second observation; its later assessment retains the alarm after
the first outcome is restated to the forecast mean:

```bash
rch exec -- cargo run -p fmine-cli -- forecast assess --ledger tests/forecasts/cusum_synthetic.toml \
  --cohort synthetic-cusum --as-of 2031-01-13T00:00:00Z --selector latest-as-of --json
```

Live CUSUM freezes use operation v8, selected by the writer. Existing journals without the opt-in keep their
identities, and saved assessments replay the same monitoring evidence after later appends.

For a separate all-Bernoulli cohort, add `event_log_floor = "0.01"` to its cohort row to enable a bounded
event log score. The loss is `-ln(max(P(observed outcome), floor))`, in **nats**, with a finite floor strictly
between zero and 0.5 fixed before outcomes. The report retains the floor, observed mean and whole-cohort
completion bounds, including both possible losses for every unresolved member. The explicit floor caps
extreme losses and removes strict propriety in the floored tails; the original forecast probabilities and
proper Brier score are unchanged. This is descriptive scoring and supplies no calibration claim.

Both policies are available through replay and live journal freezes. The live writer records an opt-in
freeze as operation v5; clients continue to omit schema markers. Omitting the policies preserves existing
identities and output. `robot-docs` exposes the accepted fields and their interpretation.

Check every member's evidence ancestry at its own registration time with a provenance bundle:

```bash
cargo run -p fmine-cli -- forecast assess --ledger tests/forecasts/cohort_synthetic.toml \
  --cohort synthetic-demo --as-of 2026-10-31T00:00:00Z \
  --provenance tests/forecasts/provenance_synthetic.toml
```

DESIGN (BD-344): the bundle's `node`, `anchor` and `binding` tables declare complete parent links, availability
or performed times, and ledger cell/sequence positions. Bindings must match the exact frozen forecast IDs, with
one for every cohort member, including missing outcomes. A derived object's availability is the latest of its
inputs and its performed time; an earlier declared availability refuses. Record visibility is checked against
the named anchor's per-cell high-water marks. The later `--as-of` assessment date cannot admit inputs that were
unavailable when the forecast was registered.

Successful output carries a digest for each checked ancestry. A future input, backdated fit, cycle, missing
ancestor or missing time refuses with `FMINE-REF-110`; future-input diagnostics include the earliest offending
ancestors and shortest paths from the forecast. Without `--provenance`, temporal validation explicitly says
`not_checked`. These are checks of supplied metadata, not independent proof of publication or recording times.
The CLI does not execute a supplied forecast generator and reports `generator_replay: not_performed`.

The pure `fmine_claim::dag::replay` API separately reruns a generator against immutable content-addressed bytes
and a view that denies inadmissible reads. A replay certificate requires identical output and read receipts,
declared ancestry for every read, and no denied read even if the generator swallows its error. It can police only
reads through that view; the caller must rebuild the result and supply the same seeds. The retrospective
DecisionCard API requires this replay certificate and checks that it binds the actual expected-NPV Claim,
cut and exact rendered/numeric output. Legacy action, CVaR, flip-margin and VOI fields still lack their own
Claim provenance, so that wrapper explicitly limits its historical evidence claim to expected NPV.

### Build a source-bound draft forecast

DESIGN (BD-067/BD-334): `forecast record build` constructs a complete candidate from the registered original
Mine Stats releases at an explicit information cut. The model is selected by name:

```bash
cargo run -p fmine-cli -- forecast record build \
  --rules forecasts/fy2026/rules.toml \
  --model pooled-quarterly-ratio-empirical-v1 \
  --as-of 2026-11-10T23:59:59Z --decimal-places 6 > candidate.json
```

The FY2026 rules require the original Q3 2026 workbook to be registered, available by the supplied cut, and
present with its registered hash. A missing release or live site's latest observation refuses the whole
candidate. Each live site and target remains required; the rules' `not_published` sites remain named in the
report. Historical missing or nonpositive observations exclude their adjacent transitions and retain both
endpoints in the report. A missing quarter never creates a ratio across the gap.

Candidate generation and record scoring share these explicit target meanings:

| Rules `field` | Unit | Rules `outcome` and scored value |
|---|---|---|
| `au_prod_100_koz` | `koz` | `fy_minus_9m`: original FY gold production minus original nine-month production, both on the 100% basis |
| `cu_prod_100_kt` | `kt` | `fy_minus_9m`: original FY copper production minus original nine-month production, both on the 100% basis |
| `aisc_usd_per_oz` | `usd_per_oz` | `q4_qtd`: original Q4 AISC per ounce sold |
| `c1_usd_per_lb` | `usd_per_lb` | `q4_qtd`: original Q4 C1 per copper pound sold |
| `aisc_usd_per_lb` | `usd_per_lb` | `q4_qtd`: original Q4 AISC per copper pound sold |

Copper forecasts require rules that explicitly declare these copper targets, their site universe and
comparators. The sealed `forecasts/fy2026/rules.toml` remains the gold record. This support does not change
those rules or issue copper forecasts; production quantities are additive, while cost rates retain their
printed ownership evidence and are neither subtracted nor ownership-scaled.

The `pooled-quarterly-ratio-empirical-v1` model uses the historical pool fixed by each persistence rule. For
each target it multiplies a site's latest Q3 observation by the pool's lower, median and upper inverse-ECDF
ratios, keeping each target's pool separate. Production requires a positive latest observation. Supported
Q4 cost targets also accept signed or zero printed observations: a negative cost reverses the two interval
endpoint products, and zero produces an exact zero point and interval under this multiplicative model.
The point remains the latest cost multiplied by the pooled-ratio median. Source ownership and status checks
still apply; a disposed or withheld zero placeholder cannot become a forecast. Historical ratio pairs still
require two positive observations. The model assumes those pooled changes apply to the future site-quarter,
including when its latest cost is nonpositive. Its exact interval bounds equal the persistence comparator's
bounds on the same inputs,
so the different median point does not improve its interval score or establish independent forecasting skill.

Every operand uses the published display value and retains its source Claim, cell, source hash, period,
vintage, unit and ownership basis. Later comparative releases cannot replace originals. Availability uses
the later of the registry's retrieval upper bound and the end of any declared publication day, without
inventing a publication date. The receipt binds the exact rules, registry, information cut and source
observations. This is a check against supplied registry evidence, not external proof of a historical run.

The JSON envelope includes `payload.exact_forecasts`, `serialized_forecasts`, `forecasts_csv`, `pools`,
`input_manifest`, `observations` and `receipt_digest`. The complete CSV is accepted by the existing frozen-record
parser. Lower bounds round toward negative infinity, upper bounds toward positive infinity, and points to
nearest with ties toward positive infinity at the requested precision (0–12 decimal places, default 6).
For example, a point of `-1.25` becomes `-1.2` at one decimal place; zero never carries a minus sign.
Exact and serialized values remain separate because export rounding
can change the later score. `--format markdown` renders the forecasts, CSV, source manifest and receipt.

Output remains an **uncalibrated draft**. The command writes no files and supplies no author attestation,
forecast registration or timestamp proof. Its source manifest is review evidence; it does not replace the
complete issuance manifest. After reviewing a successful envelope, extract the CSV without adding a newline:

```bash
jq -ej '.payload.forecasts_csv' candidate.json > forecasts.csv
```

The existing record-hash, external timestamp and record-score workflow remains responsible for freezing and
scoring a reviewed forecast. Synthetic source fixtures are explicitly labelled in JSON and Markdown.

### Compute the frozen issuer-guidance baseline

DESIGN (BD-334): the forecast record fixes two comparators. The existing `forecast-baseline` command computes
persistence; an explicit publication-date cut also evaluates issuer guidance from the registered files:

```bash
cargo run -p xtask -- forecast-baseline --as-of 2026-11-01
```

The command retains every site and target in the frozen rules. Production guidance is divided by the share
printed with that guidance, then the original Q3 release's nine-month production on the 100% basis is
subtracted. Gold AISC and copper C1/AISC guidance retain their published sold-metal rate intervals. For
example, synthetic attributable guidance `[240, 300]` at a 60% share and nine-month production
of `300` gives an exact Q4 comparator of `[100, 200]`. The calculation uses the displayed workbook number,
while its citation retains both stored and displayed values. Negative remainders are reported unchanged.

Copper guidance uses `guidance_<site>_production_kt`, `guidance_<site>_c1_usd_per_lb` and
`guidance_<site>_aisc_usd_per_lb`. Each cost entry requires unit `usd_per_lb` and an explicit ownership basis
followed by `;c1=issuer-reported-per-sold-lb` or `;aisc=issuer-reported-per-sold-lb`, respectively.
These definitions keep C1 and AISC distinct throughout source selection, persistence and scoring.

The latest agreed guidance on or before the cut is selected. Competing values on its latest publication date,
newer pending readings, missing original actuals, or unsupported labels remain explicit unavailable rows.
Excluded sites remain named `not_published` members. Before the Q3 workbook is registered, production
comparators remain unavailable; their guidance evidence is still shown. Cost comparators can be available
independently. The report binds exact endpoints, rules, document hashes, reader roles, and canonical actual
Claims. It is followed by the existing persistence output.

This baseline uses the latest registered workbook retrieval date as a conservative availability bound and
refuses earlier cuts, including when individual source publication metadata is present. Retrieval is not a
publication date. This is a source calculation, not an externally timestamped forecast or a calibration claim.
It does not choose a model for FrankenMining's own forecasts or generate the final FY2026 forecast record.

### Score an externally timestamped public forecast record

```bash
cargo run -p xtask -- forecast-record-verify path/to/frozen-record
cargo run -p xtask -- forecast-record-score path/to/frozen-record
```

DESIGN (BD-334/BD-068): these commands require a complete frozen record: `manifest.toml`, `forecasts.csv`,
`rules.toml`, their digest files, an authenticated `record.tsr` and an earlier authenticated rules proof.
The verifier accepts the pinned checked-in DigiCert or FreeTSA roots; a supplied `tsa_chain.pem` contributes
only untrusted intermediate certificates. A locally generated TSA cannot establish the record's time.
Rules must precede the record even after accounting for both proofs' stated accuracy.

Scoring verifies the record before opening registered original-release outcome workbooks. Q4 production is
the exact signed difference between the original FY and nine-month totals on the explicit 100% basis.
Gold AISC and copper C1/AISC use their printed quarterly rates directly. Outcomes use the exact **displayed**
numeric value while retaining the stored token and source Claim for audit; printed zero counts, and a
displayed dash stays missing.
Later comparative releases cannot substitute for missing originals. Ownership-label changes remain visible
even when registry corrections make the effective ownership fractions equal.

Every frozen site/target remains in the report, with unresolved and not-forecast members listed separately.
The deterministic JSON includes exact interval losses, observed and whole-cohort coverage/loss bounds, the
declared iid coverage reference and correlation sensitivity, source hashes/cell citations, proof bounds and
an assessment digest. It remains explicitly uncalibrated.

The same assessment computes both preregistered comparators. Issuer guidance must be strictly before the
frozen information cutoff; date-only metadata on the cutoff day remains unresolved unless the exact document
is in the frozen manifest with an earlier declared retrieval. The report names that declaration and does not
present it as independent proof of publication time. Production guidance is converted to the 100% basis and
reduced by the original nine-month displayed total; cost ranges are used directly. Signed remainders and
point ranges remain unchanged, and issuer guidance is assigned no nominal probability.

Persistence uses the frozen original Q3 observation and original consecutive-quarter ratio pool, with
displayed numbers, source Claims, pool exclusions and manifest membership retained. Those fixed benchmark
observations can follow the forecast cutoff; the report makes no claim that the methods had the same
information. Missing or unreadable required historical sources refuse the comparator instead of silently
shortening its pool. Printed zero is a valid persistence point.

For each gold or copper target, all three interval-score means and both forecast-minus-comparator differences
use exactly the same eligible sites. Every excluded site lists all missing inputs. A negative difference
favors the forecast under this loss, without establishing skill or calibration. Optional comparator failures, including
arithmetic overflow, preserve the verified forecast/outcome scores and appear as explicit unavailable or
refused comparisons. The `benchmark_assessment` object carries the detailed statuses, evidence and results.

Each comparator also gets an exact full-cohort bound on the mean forecast-minus-comparator interval loss,
including every unresolved outcome. With the same frozen alpha, the two interval losses have matching tail
slopes, so their difference is bounded over all real outcomes even though either individual loss is unbounded.
The report keeps the full live-member denominator, names pending outcomes and missing benchmark intervals,
and isolates arithmetic refusals to the affected comparison. For example, an invented observed lead of
`-20` can become `[-20, 30]` after admitting one pending outcome: either method could finish ahead. A wholly
negative completion interval retains the forecast's lower loss across every admitted completion. These are
arithmetic completion bounds, with no probability, confidence or calibration claim. The paired scoring API
preserves the same bounds and missing-member states.

The timestamp authenticates the frozen record and its declarations. Consumed source bodies are separately
verified; actual input use and forecast generation are not replayed. This command creates no forecasts,
registrations or time proofs; an absent or incomplete FY2026 record refuses.

### Compare a frozen record at an explicit evidence cut

DESIGN (BD-076): the production CLI exposes the record assessment in JSON or Markdown, with an explicit
assessment time and caller-selected timestamp trust roots:

```bash
cargo run -p fmine-cli -- forecast record score --record path/to/frozen-record \
  --as-of 2027-02-15T00:00:00Z --tsa-roots forecasts/tsa/digicert-trusted-root-g4.pem
cargo run -p fmine-cli -- forecast record score --record path/to/frozen-record \
  --as-of 2027-02-15T00:00:00Z --tsa-roots forecasts/tsa/digicert-trusted-root-g4.pem \
  --format markdown
```

The directory must contain `manifest.toml`, `forecasts.csv`, `rules.toml`, `record.sha256`, `rules.sha256`,
`record.tsr` and `rules.tsr`. Both signatures and message imprints are verified using OpenSSL 3 at their signed
generation times. The complete rules proof must precede the complete record proof; the record must satisfy
its frozen deadline, precede the scored year's end, and exist by `--as-of`. The selected PEM's fingerprint
is reported. Selecting a local root expresses caller trust; it does not confer public timestamp authority or
replace the pinned-authority acceptance policy of `xtask forecast-record-verify`.

The command reads original releases with verified source bytes and displayed exact values. Production
retains the signed FY-minus-nine-month result; Q4 unit costs use the printed quarterly rate. Guidance is
selected before the forecast information cutoff. The rules' fixed Q3 observation and historical persistence
pool can become available later and are checked against the assessment cut, with that information-set
difference disclosed. Missing originals, ambiguous guidance and incomplete history retain named unavailable
rows. Registered publication dates bound availability conservatively through the end of that date; when absent,
the report labels its retrieval-time fallback explicitly. Frozen input timestamps stay those in the manifest.

Every declared site and target remains visible. JSON and Markdown show each method's exact loss and coverage,
the common-population mean comparison, whole-live-cohort completion bounds, the declared binomial reference
and correlation sensitivity. Missing outcomes and missing comparators cannot change the population used by
only one method. Source cells, canonical Claim IDs, raw values, document hashes, baseline calculations and
unavailable reasons accompany the scores. The assessment digest binds this evidence and the proof metadata.
The report is descriptive or unscored, with null authority fields and no claim of forecasting skill or calibration.
It does not generate the own-model forecasts still required for the final FY2026 record.

### Register forecasts with a live durable journal

DESIGN (BD-047, BD-343): `forecast journal` records live admissions, freezes cohort membership, appends
outcomes and restatements, and persists immutable assessments. Every mutation reads the runtime clock while
holding the journal's exclusive lock. Request files cannot choose `registered_at`, `frozen_at` or `recorded_at`.
The complete operation, assigned nanosecond time, sequence and previous head are hash-bound and synchronized
before the command reports success. For an appended operation, the candidate receipt and report are rendered
before writing, so a rendering refusal cannot follow a successful append; output is released only after sync.

The following example uses an **invented unitless forecast** with a distant deadline. Use an unused journal path:

```bash
mkdir -p runs/forecast-demo
cargo run -p fmine-cli -- forecast journal init \
  --journal runs/forecast-demo/history --cell synthetic-forecast-demo \
  --max-clock-skew-ms 1000 --trust-host-clock
```

Retain the returned `payload.head` independently of the journal. Supply the latest successful head to each
mutation; a stale head refuses before writing. The placeholders below stand for those complete hex digests:

```bash
cargo run -p fmine-cli -- forecast journal register \
  --journal runs/forecast-demo/history --head HEAD_FROM_INIT \
  --request tests/forecasts/journal_registration_synthetic.toml --trust-host-clock
cargo run -p fmine-cli -- forecast journal freeze \
  --journal runs/forecast-demo/history --head HEAD_FROM_REGISTRATION \
  --request tests/forecasts/journal_cohort_synthetic.toml --trust-host-clock
cargo run -p fmine-cli -- forecast journal inspect \
  --journal runs/forecast-demo/history --head HEAD_FROM_FREEZE
```

For an actual forecast, the registration request contains one `[[forecast]]` table with `id`, `model_key`,
`outcome_due`, `kind` and that kind's distribution fields. A freeze request contains one `[[cohort]]` table
using the membership, resolution-source and scoring-policy fields shown in the committed example. Each request
has no top-level fields. Decimal values are quoted strings. The latest possible time under the declared
clock bound must be **strictly before** every affected outcome deadline, including on an empty or quiet journal.
For ordinary new requests, the writer internally adds `schema = "fmine.forecast-operation.v2"`; public-bound
freezes and generated source resolutions use v3, explicit vintage policies and comparative restatements use v4,
and freezes opting into coverage monitoring or bounded event log loss use v5. Callers must not include this
marker in their request files.
The frame retains the sampled `observed_ns`, while registration,
cohort freeze and outcome recording times use the clock's upper bound (`observed_ns + clock_skew_ns`).

After the outcome becomes available, `forecast journal outcome` accepts one `[[outcome]]` table with the same
published, missing or restated vocabulary as the replay example above, omitting `recorded_at`. Its
`available_at` must be at or before the earliest possible current time. Restatements append to the original
history, and missing members remain in the frozen denominator.

For a forecast resolved from registered public workbooks, freeze its physical target with the cohort.
This example names a future target and assumes the forecast alias was already registered before its
deadline; it supplies no forecast values:

```toml
[[cohort]]
name = "fy2027-production"
members = "north-mara-production"
resolution_rule = "registered-public-cell"
eligible_sources = "barrick"
public_issuer = "barrick"
public_metric = "production_koz"
public_period = "2027FY"
public_ownership = "100%"
public_members = "north-mara-production=North Mara"
```

The frozen digest binds the exact issuer, subject, period, metric/unit and ownership basis for every member.
Supported metrics are gold production (`production_koz`), copper production (`production_kt`),
issuer-reported AISC (`aisc_usd_per_oz`) and total cash costs (`tcc_usd_per_oz`) per sold gold ounce,
and copper AISC (`aisc_usd_per_lb`) and C1 cash costs (`c1_usd_per_lb`) per sold pound, at site level over a
calendar year (`YYYYFY`)
or individual calendar quarter. Cost definitions and sold-unit denominators must match exactly; negative
printed costs remain signed. Copper production retains the printed kilotonnes and the declared full or attributable
ownership basis; resolution performs no implicit pounds conversion or ownership scaling.
A declared deadline after the target period ends
refuses. When the original workbook has been published and registered with its digest and dated evidence:

```bash
cargo run -p fmine-cli -- forecast journal resolve \
  --journal runs/forecast-demo/history --head LATEST_RETAINED_HEAD \
  --cohort fy2027-production --forecast north-mara-production --trust-host-clock
```

`resolve` obtains the value from the authenticated original-release cell. It accepts no replacement value,
target, locator or date. The receipt retains the source Claim, exact printed value, workbook hash and
publication basis. The registered publication date must fall after the complete target period; retrieval
never supplies a missing publication date. Date-only availability conservatively begins at midnight after
the publication day. The stated FACT/INFERENCE basis is retained metadata, not independent proof of that time.
Missing original-release files, altered bytes, ambiguous cells and ownership mismatches refuse without changing
the journal. When the authenticated original cell itself prints a source-native dash, `resolve` instead records a
source-verified `not_published` missing event. The dash token, canonical source Claim, workbook hash and publication
evidence remain attached, and the member stays in every cohort denominator. Status-derived placeholders and
`n/a` still refuse rather than guessing their meaning. Caller-supplied `outcome` requests cannot bypass a frozen
public binding. Historical reports reconstruct the recorded source Claim without requiring the workbook to remain
on disk.

To retain later comparative revisions as separately scored outcomes, include this field in the cohort
request **before freezing it**:

```toml
public_vintages = "registered_comparatives"
```

DESIGN: the default `original_only` policy retains its existing canonical bytes and meaning. Opting in
freezes a distinct version-two public resolution policy; another cohort cannot change this permission
for a shared forecast ID. First admit the original release with `resolve`. Then, after registering a later
workbook and its publication evidence, append the comparative with:

```bash
cargo run -p fmine-cli -- forecast journal restate \
  --journal runs/forecast-demo/history --head LATEST_RETAINED_HEAD \
  --cohort fy2027-production --forecast north-mara-production \
  --release 2028Q1 --trust-host-clock
```

The writer verifies the later workbook's exact bytes and the same frozen subject, physical period,
metric, unit and ownership. A changed cost denominator or ownership basis refuses. The reporting quarter
must follow the original and every admitted numeric vintage; publication must follow both the target period
and that reporting quarter. A later comparative cannot fill a missing original. The writer selects the
superseded event under lock; callers supply neither its sequence nor an outcome value. `first-published`
continues to score the original, while `latest-as-of` uses a restatement only once both its availability
and recording time fall within the assessment cut. Each event retains its own source Claim and publication
evidence. Previously saved reports retain their original result, journal prefix and source admission after
later revisions. An original source-native dash is already a durable `not_published` event and cannot be backfilled
by a comparative; automatic inference of other missing reasons remains outside this source-bound protocol.

For example, an outcome available at `12:00:00Z` and recorded at an observed `12:00:01.500Z` with a one-second
uncertainty has a recording upper bound of `12:00:02.500Z`. A later assessment cut at `12:00:02Z` leaves that
outcome unresolved; a cut at `12:00:03Z` can include it. This prevents a recording from entering a historical
information cut while its declared uncertainty still crosses that cut.

`forecast journal assess --journal <path> --head <latest-head> --cohort <name>
--as-of YYYY-MM-DDTHH:MM:SSZ --trust-host-clock` computes and persists an assessment and its closed information
cut. `--selector first-published` is the default; `latest-as-of` selects eligible restatements. An optional
`--expected-digest <hex>` refuses a mismatching result without appending. Every new assessment also stores its
actual result digest in the operation, including when the caller omits that option. Replay checks that binding
and refuses a different result; a v2 assessment without its stored digest is invalid. The cut must already
be in the past under the declared clock bound.
Reopening restores these seals and earlier assessment digests. `inspect`
replays recorded operations without writing; omitting its optional head pin leaves rollback authenticity unchecked.

Recover a complete saved assessment using its exact `payload.report.assessment.digest`:

```bash
cargo run -p fmine-cli -- forecast journal report \
  --journal runs/forecast-demo/history --assessment ASSESSMENT_DIGEST \
  --head LATEST_RETAINED_HEAD --format markdown
```

The digest placeholders stand for complete hex values from successful receipts. `report` retrieves the saved
cohort, cut and outcome selector, including original/restated citations and unresolved members. It samples no
clock and leaves the journal's bytes, head and assessment count unchanged. An absent assessment or mismatched
head refuses; the command never substitutes another assessment. Without a supplemental `--provenance` bundle,
JSON reconstructs the complete `payload.report` at the first checkpoint matching that digest, including its
original `journal_head`, checkpoint inventory, `durable_journal` mode, time policy and digest-binding status.
With the same implementation, later appends, restatements and reopening leave this nested report byte-identical.
The outer `payload.head` and inventories describe the currently opened history, and `read_only: true`
identifies the retrieval. Omitting `--head`
permits an unpinned read with that limitation disclosed. This is deterministic replay of the saved assessment;
it does not promise an unchanged presentation across software versions.
Older checkpoints without a stored result digest remain readable; `result_digest_binding` distinguishes their
caller-requested digest check from a result digest bound into the first checkpoint. A later checkpoint pinning
the same result cannot upgrade that original binding. Legacy operations without an internal schema marker
retain their observed recording times, existing identities and results. New v2 operations can follow them in
the same journal; replay applies each operation's original time policy.

`payload.operation_version_counts` reports `legacy_v1` and `v2` counts, plus `v3` and `v4` when present,
excluding genesis. Version three stores original public-bound freezes and source admissions. Version four
stores explicit vintage policies and generated comparative restatements, with the same bounded-clock policy.
`payload.record_time_policy` is `none`, `legacy_observed`, `clock_upper_bound` or `mixed`. The nested report
describes the operation prefix through that assessment's first saved checkpoint, so later appends do not
change its time-policy labels. A v2 assessment over legacy history remains explicitly mixed; adding a result
digest does not upgrade the earlier clock evidence.

Add `--provenance <path>` to check the saved cohort's declared ancestry at each forecast's recorded
registration time. The same `fmine.forecast-provenance.v1` bundle used by `forecast assess` applies, including
bindings for unresolved members. Its UTC timestamps accept one through nine fractional digits, so an exact
nanosecond registration can be represented without rounding. A future ancestor refuses with its leakage
frontier and shortest path. Successful output adds the supplied bundle's digest and temporal check to the
report while preserving the original assessment digest and journal bytes. This later-supplied metadata is
not an ancestry commitment made at registration or an external timestamp proof; generator replay remains
`not_performed`.

DESIGN (BD-047, BD-068): independently timestamp a frozen journal prefix before its outcomes resolve.
Every successful receipt and `inspect` response supplies `payload.timestamp_commitment`: a versioned
SHA-256 imprint, the exact BLAKE3 prefix head, and its sequence. After freezing a cohort, retain that receipt
and request an RFC 3161 timestamp for its `imprint_sha256`, for example by preparing a query with:

```bash
openssl ts -query -digest IMPRINT_SHA256 -sha256 -cert -out frozen-prefix.tsq
```

Submit that query to your selected timestamp authority and retain the returned receipt and trusted PEM roots.
After outcomes and assessments have been appended, verify the original frozen prefix while retrieving a report:

```bash
cargo run -p fmine-cli -- forecast journal report \
  --journal runs/forecast-demo/history --assessment ASSESSMENT_DIGEST \
  --head LATEST_RETAINED_HEAD --timestamp-head FROZEN_PREFIX_HEAD \
  --timestamp-receipt frozen-prefix.tsr --timestamp-roots trusted-tsa-roots.pem
```

All three timestamp options are required together. Optional `--timestamp-intermediates <pem>` supplies
untrusted chain certificates without adding trust roots. The verifier derives the sequence and imprint from
the retained journal, checks the receipt's signature and chain, and replays the exact attested prefix. That
prefix must contain the selected assessment's exact frozen cohort and all members, with none of their outcome
events recorded yet. The authenticated upper time bound must strictly precede every member's declared
deadline, including unresolved members, and every published/restated availability time recorded for those
members in the current journal. Equality and uncertainty crossing a boundary refuse.

Successful verification appears in outer `payload.timestamp_verification`, with the prefix, receipt and
root-file hashes and authenticated time bounds; `external_time_proof` becomes `verified_caller_selected_roots`.
The nested historical report and journal bytes stay unchanged. An invalid requested proof refuses rather than
falling back to an unverified result. Caller-selected roots express that caller's trust: this does not assert
public timestamp authority, source truth, completeness of recorded publications or accuracy of original host
times. Retain `--head` separately to detect rollback of the current journal beyond the attested prefix.

The response schema is `fmine.forecast-journal.v1`; assessment responses contain the familiar report under
`payload.report`. `--trust-host-clock` acknowledges an operator-declared clock bound, fixed at initialization;
it supplies no independent timestamp proof. Source admissions retain their recorded verification evidence;
other source citations remain supplied evidence. This journal workflow
does not issue the FY2026 forecast set or replace its separate frozen-record acceptance policy. Clock rollback,
corrupt or truncated bytes, and stale heads refuse. A failed write retains its bytes for inspection and never
silently repairs or truncates history.

### Compare two saved public forecast assessments

After persisting assessments for two public-bound cohorts in the same journal, compare their saved results:

```bash
cargo run -p fmine-cli -- forecast journal compare \
  --journal runs/forecast-demo/history \
  --left LEFT_ASSESSMENT_DIGEST --right RIGHT_ASSESSMENT_DIGEST \
  --head LATEST_RETAINED_HEAD --format markdown
```

Replace the digest placeholders with complete hexadecimal values from successful receipts. Both assessments
must use the same cut and outcome selector, with matching frozen issuer, metric/unit, ownership, calendar
period, vintage permission and interval alpha. Members are paired by their frozen public subjects. The
command retains both source selections and pairs numeric outcomes only when their values and complete
publication citations agree. It reads saved journal evidence without sampling a clock, reopening workbooks
or appending records.

The result reports exact rational interval loss on the frozen binary values, expressed as left minus right;
a negative difference favors the left cohort under that loss. The observed mean names its paired population.
Every missing forecast, unresolved outcome and publication mismatch remains visible in the union of subjects.
When every subject has both forecasts and compatible selections, whole-population completion bounds include
unresolved pairs using global loss-difference extrema at the common alpha. A missing forecast, selection
mismatch or unrepresentable calculation makes the corresponding aggregate unavailable. Completion bounds
are arithmetic bounds, not confidence intervals. Registration times and information sets may differ; this
comparison does not establish independent forecasting skill. JSON is available with `--format json`, including
both source admissions, exact numerator/denominator strings and a separate comparison digest.

### Issue reports with explicit time and build identity

DESIGN (BD-347): `assure issue` produces the same verified JSON envelope as `assure public`, including its
final newline, and records its exact bytes, Claim inventory, ledger anchor, input manifest, build identity,
generic recipient role and purpose in a local append-only journal. `--output` must name a new file. No command
sends the report or contacts a recipient. The explicit synthetic lane exercises the same issuance protocol.

The owner supplies a pre-existing private key file containing exactly thirty-two raw bytes, with no Unix
group or other access; an all-zero key is refused. `NOW` is a UTC timestamp such as
`2026-11-01T12:00:00Z`; `UNCERTAINTY_NS` is an
explicit bound supplied by the trusted caller. The CLI does not establish clock accuracy. For example:

```bash
fmine reliance init --journal reliance.log --key owner.key \
  --now "$NOW" --uncertainty-ns "$UNCERTAINTY_NS" \
  --max-age-ns 3600000000000 --max-skew-ns 1000000000

fmine assure issue --synthetic --output report.json --journal reliance.log \
  --expected-head "$EXPECTED_HEAD" --key owner.key --role sponsor --purpose 'internal review' \
  --now "$NOW" --uncertainty-ns "$UNCERTAINTY_NS"

fmine reliance check report.json --journal reliance.log --head "$TRUSTED_HEAD" \
  --now "$NOW" --uncertainty-ns "$UNCERTAINTY_NS"
fmine reliance export report.json --output current-copy.json --journal reliance.log \
  --head "$TRUSTED_HEAD" --now "$NOW" --uncertainty-ns "$UNCERTAINTY_NS"
```

Use `--issuer barrick [--through YYYYQn]` instead of `--synthetic` to issue a report from verified registered
documents. The mutation's `EXPECTED_HEAD` is the previously committed head; each successful mutation returns
its successor. A consumer's `TRUSTED_HEAD` must be obtained independently from the release owner. Reading the
head from an untrusted copy of the same journal supplies no authority. Missing trust or time, stale status,
clock rollback and excessive uncertainty produce `unknown` and never authorize current reuse. Historical
`--as-of` inspection also never authorizes reuse, even if the historical status was `current`.

Owner-authorized withdrawal and recall retain the original artifact:

```bash
fmine reliance withdraw "$OUTPUT_DIGEST" --journal reliance.log \
  --expected-head "$EXPECTED_HEAD" --key owner.key --reason 'source correction' \
  --defect source-restated --claim "$CLAIM_ID" \
  --now "$NOW" --uncertainty-ns "$UNCERTAINTY_NS"
fmine recall --journal reliance.log --claim "$CLAIM_ID"
fmine recall --journal reliance.log --revision "$BUILD_REVISION"
```

DESIGN: withdrawal blocks current reuse while historical bytes still verify. `reliance supersede` links an
issued digest to an existing issued replacement; the replacement's own status must be checked separately.
`reliance refresh` appends an owner-authorized freshness cut. Both require the same key, expected head and
bounded-time arguments as withdrawal. `reliance export` retrieves an existing issuance. Use `assure issue`
for a freshly generated report, or the host-clock `reliance reissue` below to register another recipient
while preserving one selected issuance's producer metadata. Only recorded issuances are recallable.
Recipient roles are `release-owner`, `sponsor`, `reviewer-pool-N` or `agent-session:<hex>`; no personal contact
fields are stored. Build revision is captured at compilation, with `+dirty` when applicable, and the executable
itself is hashed. An unavailable build revision refuses issuance. `robot-docs` lists the complete grammar.
`assure issue` currently requires Linux's executing-image handle so replacing a binary's pathname cannot change
the recorded binary identity. Other platforms refuse issuance explicitly; status inspection and local
retrieval remain available where their existing journal-lock support is present.

The explicit-time grammar above shares report metadata and journal storage with the host-clock grammar
below. Existing `reliance issue`, `reuse` and `recall` commands remain available. Choose one complete grammar
for each invocation; mixing its authority or time flags is a usage error.

### Issue reports with an explicitly trusted host clock

DESIGN (BD-347): a local reliance journal records exact assurance artifacts, their Claim dependencies and
generic recipient roles. A matching file digest proves the bytes still match; current reuse also requires a
fresh status cut and an independently trusted journal head. The default without trusted head/time inputs is
`unknown`. `--trust-host-clock` declares the operator's clock bound, not an external time proof. Possessing the
release key establishes local capability, not human review or regulatory authority.

This Bash example uses an invented report and ignored local files. Generate a private **raw 32-byte** key
(not hexadecimal text), without printing it or overwriting an existing key:

```bash
mkdir -p runs/reliance-demo
(umask 077; set -o noclobber; head -c 32 /dev/urandom > runs/reliance-demo/release.key)
FMINE_REVISION=$(git rev-parse HEAD) # Caller-declared metadata; the running executable is hashed separately.
cargo run -p fmine-cli -- reliance init --journal runs/reliance-demo/journal \
  --release-key runs/reliance-demo/release.key --release-role release-owner
```

Retain each successful mutation's returned `payload.journal_head` through an independently trusted channel.
Supply that trusted value below; reading a head from the journal being checked cannot authenticate it.

```bash
FMINE_HEAD=REPLACE_WITH_INDEPENDENTLY_TRUSTED_INIT_HEAD
cargo run -p fmine-cli -- assure public --synthetic --journal runs/reliance-demo/journal \
  --release-key runs/reliance-demo/release.key --head "$FMINE_HEAD" --trust-host-clock \
  --recipient-role reviewer-pool-1 --purpose review --code-revision "$FMINE_REVISION" \
  --output runs/reliance-demo/report.json

FMINE_HEAD=REPLACE_WITH_INDEPENDENTLY_TRUSTED_POST_ISSUE_HEAD
FMINE_DIGEST=REPLACE_WITH_RETURNED_ISSUANCE_OUTPUT_DIGEST
FMINE_ISSUANCE=REPLACE_WITH_RETURNED_ISSUANCE_ID
cargo run -p fmine-cli -- reliance check --journal runs/reliance-demo/journal \
  --head "$FMINE_HEAD" --trust-host-clock --digest "$FMINE_DIGEST" --artifact runs/reliance-demo/report.json
cargo run -p fmine-cli -- reliance reissue --journal runs/reliance-demo/journal \
  --release-key runs/reliance-demo/release.key --head "$FMINE_HEAD" --trust-host-clock \
  --artifact runs/reliance-demo/report.json --output runs/reliance-demo/sponsor-copy.json \
  --source-issuance "$FMINE_ISSUANCE" --recipient-role sponsor --purpose review

FMINE_HEAD=REPLACE_WITH_INDEPENDENTLY_TRUSTED_POST_REISSUE_HEAD
cargo run -p fmine-cli -- reliance reuse --journal runs/reliance-demo/journal \
  --head "$FMINE_HEAD" --trust-host-clock --artifact runs/reliance-demo/report.json \
  --output runs/reliance-demo/working-copy.json
cargo run -p fmine-cli -- reliance withdraw --journal runs/reliance-demo/journal \
  --release-key runs/reliance-demo/release.key --head "$FMINE_HEAD" --trust-host-clock \
  --digest "$FMINE_DIGEST" --defect code-defect --all-claims \
  --reason "All report statements require review after a producer defect"

FMINE_HEAD=REPLACE_WITH_INDEPENDENTLY_TRUSTED_POST_WITHDRAWAL_HEAD
cargo run -p fmine-cli -- reliance recall --journal runs/reliance-demo/journal \
  --head "$FMINE_HEAD" --code-revision "$FMINE_REVISION"
```

Replace each head/digest/issuance placeholder with its full 64-character lowercase hexadecimal value. The revision is
a full 40-character Git SHA declared by the caller. Use `--issuer barrick` instead of `--synthetic` to issue a
registered public report. Relative paths use `--workspace`; all new artifact paths must be unused.

With `assure public --output`, `--format json|markdown` controls both the saved artifact and the separate stdout
receipt, defaulting to JSON. `reliance issue` remains available; only that command accepts `--artifact-format`
to select the stored format independently. The explicit-time `assure issue` always stores the JSON report;
its `--format` selects only the receipt rendering. Each issuance and recall entry exposes `issuance_id`, the
BLAKE3 digest of the canonical issuance record; explicit-time entries also retain the equal `issuance_digest`
field. This identifies one producer/recipient record; `output_digest` identifies the exact report bytes, and
`journal_head` identifies the status history to pin independently.

`reissue` checks the selected issuance's exact artifact and current status before creating another recipient
copy or appending its record. It preserves the original code revision, executable digest, input manifest,
Claims, anchor, lane and taint; only recipient, purpose and issuance time change. Its new issuance appears in
recall. A wrong key, missing or stale trust, withdrawn/superseded output or unknown issuance refuses before
creating the destination. It does not rerun the report or attest a new build identity.

The default freshness window is 300 seconds with a declared clock-skew bound of 1,000 ms, configurable at
`init`. An authorized `refresh` appends a fresh status cut using the same key/head/clock options; withdrawals
remain effective. `supersede` additionally takes `--digest` and `--replacement`, both already recorded outputs.
After withdrawal or supersession, current reuse refuses even while the original bytes still verify.
For scripts that only need the current-use gate, add `--require-current` to the host-clock form
`reliance check --digest ...`; denied reuse exits 3 and retains the full status diagnostics.
`reliance check --digest ... --as-of YYYY-MM-DDTHH:MM:SSZ` reads historical status
under the same fresh head/clock inputs and always reports `current_reuse_allowed: false`. It cannot be combined
with `--require-current`. Checking an artifact without trust inputs still verifies its original bytes, while
reporting reliance as unknown. Special files and symlinks are refused before opening them; issuance and reuse
create private files and sync their parent directories as well as their contents.
Withdrawal requires `--claims clm_…,clm_…` or an explicit `--all-claims` declaration when every statement is affected.
`reuse` makes a gated local copy without recording a new issuance. `recall --claim clm_…` or the revision query
lists recorded issuances only; no command claims delivery or contacts recipients. Ordinary stdout emissions,
including shell redirection, and other report families remain unregistered. External copies are not tracked.
If artifact creation succeeds but the final journal append fails, no confirmed new issuance receipt or recipient
registration exists. Retain any destination artifact and resolve the journal before using that copy; an earlier
issuance of the same digest may still be current.

### Export evidence challenges

```bash
cargo run -p fmine-cli -- assure challenges --issuer barrick --through 2026Q2 > challenges.json
cargo run -p fmine-cli -- assure challenges --issuer barrick --through 2026Q2 --format markdown > challenges.md
```

DESIGN (BD-349): these packets turn visible AISC and Mine Stats exceptions, including joint inconsistencies,
into cited review material. Each preserves the original finding, source filenames and hashes, displayed values,
Claim references, checked arithmetic, correction alternatives and the evidence needed to close the question.
Packet ids and a packet-set digest bind their content separately from the unchanged assurance root. They are
internal-review drafts: human checks remain unfilled and external use remains disabled. Cross-vintage packets
retain the earliest, previous and current canonical assertions, both comparison results and any applicable
verified note. Periods, units and ownership bases remain distinct. The renderer rechecks the classifications
against the canonical comparison engine before exporting them. On the registered corpus through 2026Q2,
all nine known unexplained changes and all 60 ownership-conflict findings appear once in the exported packets;
E474 retains both the original 1,479 and later 1,474 USD/oz assertions with their source cells. Disclosed-only
changes and non-comparable region boundaries stay in the assurance report. Use `--synthetic` in place of the
issuer selector for an invented known-answer demonstration.

Identity-transition packets additionally preserve corrected, broken or reclassified AISC calculations across
releases, including a change in classification with identical printed values. They keep the original states,
operands, certificates and source Claims for both appearances, and request the evidence or inclusion rule that
explains the transition. Ambiguous earliest releases retain every assertion with no selected first assertion.
Transition classifications and chronology are rechecked against the canonical identity ledger before export.

`assure challenges --issuer barrick --through 2026Q2` also exports `mine_stats_identity_transition` packets
for corrected, broken and reclassified Mine Stats calculations. Each retains the earliest registered sources,
previous bound identity and current identity, plus competing assertions from intervening ambiguous releases.
Recorded missing inputs and original refused or unresolved states keep their typed reasons. The exporter
rechecks the complete history, registered subject-label interpretation and cited source operands. These
findings supply review questions, with no issuer pass score or selection of a true vintage.

PDF transcription conflicts also render as packets when the local two-reader transcription registry is present.
Run `assure challenges --issuer barrick` without `--through` to include that document evidence; the existing
quarter-bounded assurance snapshot excludes PDF transcriptions. Each packet retains the agreed printed tokens,
page, table, row and column labels, units, ownership basis, period, reader roles and verified document hashes.
The exporter recomputes the rounding-convention comparison before reporting a numerical conflict. Incompatible
quantity contexts are reported separately and never treated as evidence that two comparable numbers disagree.
Neither assertion is chosen as correct, and a later document is not assumed to be an issuer correction.

### Calculate guidance completion requirements

```bash
cargo run -p fmine-cli -- guidance completion --synthetic
cargo run -p fmine-cli -- guidance completion --issuer barrick --site north-mara --through 2026Q2 --as-of 2026-10-31
cargo run -p fmine-cli -- guidance completion --issuer barrick --site north-mara --through 2026Q2 --as-of 2026-10-31 --format markdown
```

DESIGN (BD-342): the public command requires registered, hash-verified documents and two agreeing reader
entries in the local, ignored `registries/transcriptions/<issuer>.toml`. Use the existing transcription schema
with quantities `guidance_<site_key_with_underscores>_production_koz` and
`guidance_<site_key_with_underscores>_aisc_usd_per_oz`, an annual period such as `2026FY`, units `koz` and
`usd_per_oz`, and an explicit ownership basis (`100%` or `attributable:n/d`). The AISC basis must also carry
`;aisc=issuer-reported-per-sold-oz`. Both guidance rows must come from the same latest eligible registered
document. Missing or conflicting evidence refuses; no real guidance values are bundled in the code.

The result reports exact remaining production and an AISC spending/rate frontier. AISC uses summed costs and
sold ounces; it never averages quarterly rates or treats production as sales. Remaining sales explicitly assume
the preceding year's sales for the same remaining months, with source Claims and rounding intervals retained.
The report also tests completion jointly against every usable contiguous historical quarter window of the
remaining duration, including windows crossing a year boundary. Production, sales and AISC come from each
quarter's own original release before the guidance year; later comparatives cannot replace a missing original.
The **T1 historical comparison** keeps each quarter's sales and costs coupled. Separate best-case production
and cost ranges cannot establish a feasible result. A positive result includes an independently checked exact
replay; an exclusion includes checked certificates and applies only to those historical windows under the
declared sales assumption. Missing or incompatible windows have explicit reasons, and no usable history stays
**T3 for the historical comparison**. Solver limits remain indeterminate.

A separate **T2 technical production comparison** binds the remaining requirement to a registered
`technical_report` PDF and three agreed, page-cited inputs. Use these quantity keys and units, with site
hyphens written as underscores:

| Quantity key | Unit | Meaning |
|---|---|---|
| `technical_<site>_remaining_dry_feed_mt` | `dry_mt` | Million dry metric tonnes over the remaining months |
| `technical_<site>_head_grade_g_per_t` | `g_per_dry_t` | Gold head grade per dry metric tonne |
| `technical_<site>_recovery_percent` | `percent` | Gold recovery in explicit percent units |

All three rows must come from the same latest eligible document and declare the same basis:
`100%;scenario=declared_upper_bound`, `100%;scenario=plan`, or `100%;scenario=nameplate`.
Their period must be the exact remaining support: `YYYYQ2-Q4` after Q1, `YYYYH2` after Q2, or `YYYYQ4`
after Q3. Full-mine inputs are converted exactly; the guidance ownership share scales production only.
No annual rate, average grade or life-of-mine figure is silently converted into a remaining-period bound.
Declared upper boundaries are exact; printed plan/nameplate points retain their rounding intervals.
The output includes the scenario production ceiling and optimistic required grade with exact Claims and
citations. Plan/nameplate comparisons remain conditional, and no T2 result supplies a joint production/AISC
witness. Missing, altered, disputed or inapplicable optional technical evidence gives a named **T3 technical
result** while preserving the arithmetic requirements and historical comparison. Newer incomplete inputs
cannot silently fall back to an older complete report. The synthetic command exercises the positive T2 path.

The report includes the guidance vintage, as-of selection and all source hashes. Each workbook uses its cited
publication date, with the registered retrieval upper date as a fallback where publication is unstated. Later
files are filtered before reading; missing eligible YTD inputs still prevent a completion calculation.
JSON preserves exact Claims,
source-quarter witnesses and certificates; Markdown presents requirements, comparison results and citations.
Its content digest creates no forecast registration or ledger certificate, and neither comparison gives a
probability or physical-impossibility conclusion. A real issuer's T2 result still needs applicable registered
technical-report evidence; this implementation does not bundle or invent those inputs. Prospective forecast
registration and the remaining BD-342/M0 acceptance gates stay open.

For a separately selected explicit upper-limit document, use `--capacity-document` and the schema below.
Its result appears alongside the automatic registered scenario; each retains its own input interpretation.

#### Add an explicit physical production ceiling

```bash
cargo run -p fmine-cli -- guidance completion --issuer barrick --site north-mara --through 2026Q2 --as-of 2026-10-31 --capacity-document Mine-technical-report.pdf --format markdown
```

DESIGN (BD-342): the optional `--capacity-document` selects a registered `kind = "technical_report"`
document. The filename above is illustrative; no new issuer facts or document values are bundled. Its bytes
and SHA-256, publication date at or before `--as-of`, and two agreeing readers are checked before use.
All three quantities must appear in that same document, for the selected mine and exactly the remaining
calendar-month period (for example `2026-07/2026-12`):

| Quantity key suffix after `capacity_<site_key_with_underscores>_` | Unit | Meaning |
|---|---|---|
| `remaining_dry_feed_mt` | `dry_mt` | Maximum remaining dry feed, in million metric tonnes |
| `head_grade_g_per_t` | `g_per_dry_t` | Upper gold head grade, grams per dry metric tonne |
| `recovery_fraction` | `fraction` | Upper gold recovery, greater than zero and at most one |

Each row requires basis `100%;bound=explicit-upper;metal=gold`, a single explicitly stated upper value,
and a printed locator label identifying a maximum, upper limit or at-most bound. Ordinary forecasts,
average grades, percent values, wet tonnes and annual nameplate values are not silently converted into
remaining-period physical limits. Missing, conflicting, incompatible or late evidence refuses the requested
capacity comparison.

The **T2 production-ceiling comparison** multiplies the explicit full-mine limits, uses the exact statutory
troy-ounce conversion and applies the guidance's stated attributable share when needed. It reports the
optimistic maximum production and required head-grade interval, with canonical Claims and citations.
A requirement above that optimistic ceiling is excluded by the stated limits. **Not excluded** establishes
neither simultaneous attainability of the maxima nor production-cost feasibility. Already delivered
requirements stay separately labelled. No probability, mine valuation or technical-report certification is
inferred. JSON binds the unchanged arithmetic/history report and the capacity report in one composite digest;
Markdown presents both sections. Full BD-342/M0 empirical and acceptance gates remain open.

### Assess guidance at an explicit evidence cut

```bash
cargo run -p fmine-cli -- track-record --issuer barrick --as-of 2026-11-01 --json
cargo run -p fmine-cli -- track-record --issuer barrick --site north-mara --as-of 2026-11-01 --format markdown
cargo run -p fmine-cli -- track-record --issuer barrick --site north-mara --as-of 2026-11-01 --price-subject barrick --format markdown
```

DESIGN (BD-341): `--as-of` selects a separate evidence-cut report, preserving the retrospective command and
its `--synthetic` demonstration above. It uses the same registered workbooks and local two-reader PDF
transcriptions. It keeps every registered guidance vintage, selects the earliest
agreed guidance for each site, fiscal year, metric and ownership basis, and compares it with the matching
annual outcome. A later comparative is displayed separately and cannot replace a missing first publication.
Divested, withheld and unavailable outcomes remain in the denominator with their reasons.

The quantities and basis declarations are the same as the retrospective report. Ownership, units, scope,
source hashes and exact source-cell anchors are checked before scoring. When original publication timing
is unestablished, the registered retrieval date is used conservatively. `--through` and `--as-of` are
mutually exclusive; neither invents an external timestamp proof.

The workbook publication cut is applied before opening source files. A future workbook remains visible
as an excluded registry declaration, but its absence or changed bytes cannot alter the historical report
or either report digest. Every eligible workbook still has to pass its registered size and hash checks.

The report gives exact range-normalized misses, their direction, source-rounding ambiguity, observed hit
rates and bounds that retain unresolved outcomes. Its Clopper–Pearson reference interval is explicitly
conditional on independent Bernoulli hits; common year or site shocks are not modelled. Guidance ranges
have no inferred probability level. The report is descriptive and makes no calibration or site-ranking
claim. Base cost comparisons remain unconditional on price: a miss alone does not establish an operating failure.
AISC rows add the same cited price sensitivity as the retrospective report, assessed separately against each
selected first-published and latest registered outcome. Annual royalty evidence must come from that outcome's
own release. Only price documents available by the cut can contribute; the quarterly population ends at the
last completed quarter within 2024Q1–2026Q2. Missing historical members remain explicit, even when their source
is unavailable. Site prices are the default; `--price-subject` explicitly selects and labels a proxy.

Each row retains its page or cell citation and file hash. The deterministic payload uses
`fmine.guidance.track-record.as-of.v1`, distinct from the retrospective `fmine.guidance.track-record.v1`.
The report retains its base comparison digest as `comparison_digest`; the top-level `digest` binds that
comparison and both price assessments. Coverage describes the registered transcription population; the
complete historical data gate, causal cost attribution and cluster-aware inference remain open. Source
documents and real transcription rows remain outside git.

### Compare Lumwana recovery with its historical grade relation

```bash
cargo run -p fmine-cli -- recovery gap --synthetic
cargo run -p fmine-cli -- recovery gap --issuer barrick --through 2026Q2 > recovery.json
```

The command fits two descriptive models to the annual head-grade and recovery observations in the
2013–2022 part of the Lumwana site-visit presentation's Appendix A, then compares both with the registered
quarterly Mine Stats. M-a holds implied tails grade constant; M-b fits a line relating implied tails grade
to head grade. Both use `tails = head_grade × (1 − recovery/100)`, with grade and recovery explicitly in
percent units. Their calculations and rounding propagation live in the
[recovery metric kernel](crates/fmine-metrics/src/recovery.rs).

The synthetic command runs without external data. To use the Barrick command:

1. Fetch the registered workbooks with `bash repro/barrick/fetch.sh`.
2. Download `Lumwana_Site_Visit_25092025.pdf` and `Barrick_AIF_2025.pdf` from their `[[document]]` URLs in
   [the source registry](registries/sources/barrick.toml), placing them in each row's registered `dir`.
   The command verifies their byte counts and SHA-256 digests. The workbook fetch script does not fetch PDFs.
3. Supply the two agreeing reader entries in the local, ignored `registries/transcriptions/barrick.toml`.
   Each annual head-grade and recovery field must cite the presentation's printed page 39, table
   `Appendix A: Past Production Records for Lumwana`, year row and printed column. The separate ownership
   entry cites PDF page 38 (printed page 37), table `Copper Mineral Reserves`, row
   `Lumwana surface (100%)`, column `Based on attributable tonnes`. The
   [public recovery contract](crates/fmine-public/CONTRACT.md) specifies the quantity keys, units and support;
   the [transcription schema](crates/fmine-registry/src/transcriptions.rs) defines the entry fields.

Only source metadata and hashes are committed. Historical row values and PDFs remain local while
OPEN-R7-1 is unresolved. Missing or conflicting evidence produces a typed refusal; the real command never
substitutes synthetic inputs.

For every available quarter, the report retains the earliest registered visible workbook appearance and
prints both models' expected recovery, signed gap in percentage points, implied copper difference in kt,
and that difference as a fraction of reported production. A fraction can be multiplied by one hundred to
express it as a percentage. The original source Claims, PDF citations, ownership interpretation and model
limitations accompany the results. An unusable quarter remains in the report with its refusal; a later
comparative does not replace it. The available 2023 observations are explicitly labelled as comparative
appearances in the 2024 releases, rather than first-published 2023 results.

The exact rational points have conservative enclosures over the declared input-rounding boxes. These bounds
do not assert statistical coverage. The fit also distinguishes the descriptive residual sample SD, using
`SSE/(n−1)`, from the conventional regression residual-error SD, using `SSE/(n−2)`; their square-root bounds
cover numerical evaluation only. The models omit ore type, grind, throughput effects and concentrate mass
pull, and apply annual training to quarterly observations. Operational attribution, recoverable production
and forecasts remain outside this output.

#### Apply explicit copper-price scenarios

DESIGN (bd-srqc): repeat `--price SCENARIO=USD_PER_LB` to value each signed quarterly model difference
under named constant nominal price assumptions. These example prices are illustrative declarations:

```bash
cargo run -p fmine-cli -- recovery gap --synthetic \
  --price low=4.00 --price high=5.50 --format markdown
cargo run -p fmine-cli -- recovery gap --issuer barrick --through 2026Q2 \
  --price low=4.00 --price high=5.50 --json
```

Both models and every declared scenario retain exact USD-million points and all input-rounding bounds.
The conversion uses the statutory avoirdupois pound, keeps negative and cross-zero differences, and
retains the original full-site quarter. For an independent unit example, `0.45359237 kt` is exactly one
million pounds, so USD 4/lb gives USD 4 million. This is a hypothetical gross metal-value sensitivity;
it does not estimate recoverable copper, sales revenue, profit or monetary downside and is not annualized.

The `fmine.recovery.price-sensitivity.v1` payload embeds the unchanged recovery report and base digest,
plus canonical price declarations and derived Claims with their parent identities and source anchors.
Scenario names are unique; up to sixteen nonnegative plain decimal prices are accepted, including zero.
No price is fetched or inferred. Reordering scenarios keeps output bytes unchanged, while changing a price
changes the sensitivity digest. Unusable quarters remain explicit refusals with the full result population.

**`--through` limits workbook releases for a retrospective comparison.** The command still reads the current
verified historical model documents, so the bound does not establish what was knowable at that earlier date.
Omit it to use all registered workbook releases. The payload's `fmine.recovery.gap.v1` digest binds the model,
source Claims and results; the CLI envelope has no ledger anchor or certificate. An unbounded
`assure public --issuer barrick` run also includes the optional `recovery` section when these transcriptions
are supplied. The existing bounded assurance command continues to skip PDF transcriptions.

---

## TL;DR

**The problem.** Mining runs on single numbers. A reserve is one tonnage at one grade, and a plan is one schedule.
A grade-control model silently replaces the resource model it was meant to test. Reconciliation factors are
reported but rarely attributed. Capex overruns average about +14%, with about half outside the claimed ±15%
(plan §1). The shocks that actually moved capital in 2025–26 sit outside single-entity spreadsheet models:
- host-state disputes and the loss and return of control of a mine;
- asset exits;
- JV disputes;
- an IPO carve-out.

**What already exists.** The methods are known: geostatistics, pit optimisation, metallurgical accounting,
Bayesian search and sequential testing. The enterprise systems are mature: ERP, fleet management, geological
modelling and mine planning.

**What nobody has composed.** One evidence substrate in which each of those numbers is a `Claim`:
- with its support and evidence class;
- with a calibration record and a typed unknown-unknown term;
- frozen and scored when it is a forecast;
- carried through reconciliation, accounting and the capital decision without losing any of that.

**FrankenMining** is that substrate and the engines that use it. It is written in Rust on the
[asupersync](https://github.com/Dicklesworthstone/asupersync) runtime. It reuses the
[FrankenSuite](https://github.com/Dicklesworthstone) siblings: frankensim for geometry and solvers, and
franken_accounting as the book of record. At a company that already runs an ERP, the ERP stays the statutory
ledger and franken_accounting reconciles to it.

---

## Alongside your systems, and a nerve centre for agents

A major miner has spent years, and often hundreds of millions of dollars, on SAP S/4HANA or another ERP, fleet
management, historians, laboratory systems and geology and planning tools. FrankenMining is designed to sit beside
them, not to replace them (plan §1.5, NONGOAL-001).
- **Read-only connectors.** ERP extracts (flat file, IDoc or OData), content-hashed fleet and historian snapshots,
  and modelling-tool exports. Nothing writes back to a transactional or control system.
- **The ERP stays the ledger.** franken_accounting rebuilds the ERP's trial balance from extracts and must agree
  with it, and every difference is a cited register item. Estimates that move money reach the controllers as
  workpapers for entries they post in their own ERP.
- **Incremental adoption.** The public demonstration needs no system access. A pilot needs a handful of read-only
  extracts, and each further source is one more registered dialect, never a migration.

On top of those systems, FrankenMining is designed to be the **agentic nerve centre**: the one place AI agents read
the operation from (plan §1.6).
- **One typed model of the business.** Sites, ventures, ore, metal, costs, plans, forecasts and decisions are typed
  Claims with units, ownership basis and provenance.
- **A robot-first surface.** One JSON envelope, typed refusals, `next_actions` and `robot-docs`, also served over
  MCP.
- **Safe by construction.** Agents read and prepare, and humans authorize. Agent forecasts are frozen and scored
  like anyone's, so trust in an agent is earned on its record.

The ERP keeps running the transactions. People and agents get one legible, checked and cited view of the business.

---

## How it knows what it knows

- **The universal Claim.** Every number that crosses a crate boundary is a Claim (INV-001). It carries:
  - support: point, interval, composite, block or SMU, and larger mining and accounting units;
  - evidence class: measured, derived, modelled (calibrated or not), elicited, transferred, assumed or asserted;
  - knowledge state and calibration flag;
  - validity domain;
  - provenance down to a sheet cell or a truck ticket;
  - exactly one admissible **unknown-unknown term**. There are three forms: a reference-class ε-contamination, a
    catch-all hypothesis with a reset floor, or a flip margin whose probability is typed as *not stated*.

  No probability is fabricated.
- **Known knowns, known unknowns, unknown unknowns.** A layered epistemic stack keeps:
  - what is measured;
  - what is modelled with a calibration record;
  - what is modelled without one;
  - what lies outside every model.

  An executive sees them on one page (plan §9).
- **Register before resolve.** Forecasts are hashed before their outcome exists and scored by proper rules after.
  Calibration is reported per family, segment and author, whether model, person or agent (§10). Anytime-valid
  monitors watch for drift: coverage e-processes, and a sequential PIT along the mining front that alarms in
  months rather than years (GB-08).
- **Refuse, never plug.** Conservation failures, unit or basis mismatches, averaging across supports and assay
  look-ahead are typed refusals (`FMINE-REF-*`), never silently clamped numbers.
- **Four time axes.** Every record carries:
  - when the physical state held;
  - when it was sampled;
  - when the result became visible to a decision-maker;
  - when the ledger committed it.

  A backtest therefore never sees an assay before its certificate existed (INV-016).

---

## Subsystems

| Subsystem | What it does | Plan |
|---|---|---|
| **Bedrock** | Append-only authority cells, content-addressed objects, certified sync, asupersync regions | §26 |
| **Plumb** | Claims, evidence classes, KK/KU/UU, the calibration ledger, surprise monitors | §7–§10 |
| **Lodestar** | Exploration: thinned point processes over exploration effort, typed negative evidence, drill-portfolio VOI | §11 |
| **Corebox** | Drilling, sampling theory, QA/QC as sequential evidence, assays | §12 |
| **Strata** | The subsurface belief ledger: regenerable, seeded realizations at a declared support | §13 |
| **Highwall** | Rock mass, slopes, ground control, TARP automata, pore-pressure sensitivity | §14 |
| **Pushback** | Plans with a three-gap certificate (optimization, sampling, calibration); ore mix at best unit cost | §15 |
| **Headframe** | The read-only operations twin: drill and blast, load and haul, autonomous-haulage queueing, energy | §16 |
| **Crucible** | Comminution, flotation, leach, refractory circuits, residence-time deconvolution, metallurgical accounting | §17 |
| **Freeboard** | Tailings evidence state (never "safe"), GISTM traceability, water balance, ARD, closure | §18 |
| **Muster** | Rosters, fatigue, critical-control verification, driver-based labour budgets | §19 |
| **Touchstone** | Reconciliation as a joint posterior over the material-flow graph; F-factors as posteriors | §20 |
| **Bullion** | Metal accounting over franken_accounting; AISC, AIC, C1, GEO and reserve replacement as versioned definitions | §21 |
| **Helm** | DecisionCards; one `CapitalProgram` over a shared scenario index, JV waterfalls, juniors and deal structures | §22–§23 |
| **Hallmark** | Resource and reserve register, QP/CP attestation, JORC/NI 43-101/S-K 1300 renderers | §24 |
| **Portal** | Vendor formats, CRS, quarantine on import, the public-filings lane | §32 |
| **Adit** | Robot CLI, daemon, MCP, TUI and reports | §30 |
| **Testpit** | Synthetic-truth mines and districts, pinned oracles, hindcasts, the offline verifier | §33 |

---

## The four claims

| Claim | Meaning | What establishes it |
|---|---|---|
| **Parity** | A published number or reference tool is reproduced from the same inputs | Pinned oracle fixtures; public filings |
| **Known answers** | Seeded truth is recovered on synthetic data, with calibrated intervals | `fmine-gen` replicates and coverage |
| **Reconciliation** | Forecasts agree with mined and processed reality, scored after the fact | The calibration ledger |
| **Performance** | A workload meets a target on named hardware | A benchmark with machine, data size and spread |

They are never averaged into one number. Reproducing a published AISC does not show the AISC is right. Recovering
a synthetic deposit does not show a real one behaves like the generator.

---

## Architecture

```text
 soil, geophysics, drilling ─► Lodestar ─► Corebox ─► Strata (belief ledger, regenerable projections)
                                                        │
                                  Highwall ◄────────────┤
                                                        ▼
 FMS, historian, LIMS exports ─► Headframe ─► Pushback (plans with three-gap certificates)
                                     │            │
                                     ▼            ▼
                                 Crucible ──► Touchstone (reconciliation posterior) ─► Bullion ─► franken_accounting
                                 Freeboard       Muster                                 Hallmark (register)
                                                   │
                     Helm: DecisionCards and the CapitalProgram over one scenario index ─► the board

 Plumb (Claims, KK/KU/UU, calibration ledger, monitors) and Bedrock (ledger, sync, runtime) sit under all of it.
```

Dependencies point downward only, checked by `xtask check-layers` from the first commit (GATE-LAYERS-00):

```text
T   Testpit: verification; reaches anything, depended on by nothing
U   Adit surfaces: fmine, fmined, fmine-mcp, fmine-tui
V   services and adapters: importers, projections, the franken_accounting subprocess
E   engines: seeded tiles and solvers; no IO, no clock
K   domain kernels K0–K4: pure functions under Cx<cap::None>
S1  runtime and authority: asupersync regions, ledger, CAS, sync
S0  substrate: std-only types and durable formats (FMT-*)
```

The design has hard boundaries:
- No process holds a write path to plant or fleet control (INV-011).
- Four envelope flags are false in every output and cannot be set: `reserve_disclosure_claim`,
  `execution_authorized`, `geotechnical_signoff` and `regulatory_compliance_claim` (INV-002).

---

## The wedge: Guidance & Reserve Assurance

The wedge pass (Brenner B5) tested seven candidate first products. The one that is useful with **no data** and decisive
with **some data** is assurance of the numbers behind guidance and disclosure: AISC, GEO, reserve replacement and
grade and recovery by site (plan §37.1).

- **M0, Public Mirror plus Spine (Nov 2026–Jan 2027).** `fmine assure public --issuer barrick` will:
  - reproduce every per-site value in Barrick's published quarterly Mine Stats workbooks (2024Q1–2026Q3),
    each cell cited by `<file-period>:<Sheet>!<Cell>` and file hash;
  - reproduce every cut-off grade stated in a technical report;
  - keep a discrepancy register;
  - hash FY2026 per-site forecasts, with 80% intervals, *before* the February 2027 release.

  Its gate, GATE-BULL-01, has three parts (plan §21.6): every numeric cell in a named denominator extracted equal
  to the published value against an independent read; every identity instance placed in one of five states; and
  issuer consistency across vintages. Zero uncited numbers and byte-identical reruns still apply.
- **M1–M2.** Synthetic truth, reconciliation known answers, then a site assurance pilot in shadow mode: the
  12-month demonstrator.
- **M3–M6.** Group metric and reserve assurance with the accounting bridge. Then exploration and capital
  allocation, multi-site rollout with a tailings evidence state, and planning under calibrated uncertainty.

The program gate at month 12 re-scopes everything if no executive meeting has cited a FrankenMining number or
card (§34.5).

---

## Read HTML filing tables with source provenance

The I/O layer provides a bounded HTML table reader for explicitly closed filing tables, including
`rowspan` and `colspan` headers. It retains the complete source hash, original cell markup and byte
locations, exact decoded numeric tokens, and merged-cell origins:

```rust
use fmine_io::HtmlDocument;

let bytes = br#"<table><tr><th>Production</th><td>1,250.50</td></tr></table>"#;
let document = HtmlDocument::from_bytes("filing.html", bytes)?;
let table = &document.tables()[0];
let cell = table.cell(1, 2).expect("the declared second cell");
let number = cell.number().expect("one supported exact number");
assert_eq!(number.value.mantissa, 125050);
assert_eq!(number.value.scale, 2);
let anchor = document.make_anchor(0, 1, 2).expect("source origin");
```

Covered positions in a merged cell resolve to one source origin and anchor. They do not create additional
numeric observations. The reader preserves placeholders and refuses malformed or overlapping spans and
excessive grid expansion. It does not concatenate footnote digits, separate text runs or malformed grouping
into a different quantity. Table indices are zero-based; cell coordinates are one-based.

This API records a cell's source bytes and location. Interpreting its metric, period, ownership, units and
disclosure basis remains the public-source adapter's responsibility. Only a single full-cell bare number is
promoted; explicit currencies, units, ranges and annotated values remain text. It supports a documented subset of HTML rather
than browser layout, CSS or inline XBRL transformations. Existing callers of `extract_html_tables`
receive a compatibility projection; use the fallible API for source evidence and refusal diagnostics.

---

## Workspace and budget

The crate map has 59 crates in 7 layers, each with a LOC budget (`registries/loc_budget.toml`; ceiling 1.15× plan).
A crate joins the workspace only with its first real slice. The first slice covers:
- the S0 substrate: `fmine-core`, `-units`, `-time`, `-crs`, `-evidence`, `-claim`, `-registry`, `-array`;
- `fmine-runtime`, which so far holds only the host wall clock;
- `fmine-numerics` and `fmine-calib`;
- `fmine-geom`, the Strata geometry kernel: exact predicates and the sparse `VdbGrid`, lifted from frankensim;
- `fmine-optimize`, so far only `pitflow`:
  - a certified ultimate pit by pseudoflow, which matches MineLib's published UPIT optima on the 11 instances
    tried, up to 2.1M blocks, and which `fmine plan pit` runs on a UPIT pair;
  - nested pit shells across revenue factors;
  - precedence arcs from slope angles by azimuth sector and geotechnical domain;
- the public lane: `fmine-io`, `fmine-public`, `fmine-metrics`;
- `fmine-decide` and `fmine-cli`;
- `xtask`.

The workspace runs under these rules:
- `unsafe_code = "forbid"` everywhere.
- asupersync is the only async runtime.
- tokio, arrow, polars, petgraph, rusqlite and C solver features are banned (plan §28.5).
- Numerical code lifted from [frankensim](https://github.com/Dicklesworthstone/frankensim) carries a provenance
  header per file.

---

## Command surface

```bash
fmine assure public --issuer barrick    # M0: reproduce, cite or refuse every published site metric
fmine claim show <id>                   # a Claim with its evidence class, knowledge state, UU term and provenance
fmine claim why <id>                    # evidence, colour, admission guards and unassessed fields
fmine claim trace <id>                  # source anchors, registered files and unresolved parent IDs
fmine doctor                            # hash chains, pins, LOC budgets, obligations
fmine robot-docs                        # machine-readable docs and NDJSON schemas for agents
```

Every verb is registered in `registries/verbs.toml`. Every command prints human text and, with `--format json`, a stable
NDJSON envelope with documented exit codes. Agents get certified numbers or typed refusals, never prose.

---

## Oracles and pins

| Reference | Pin | Role |
|---|---|---|
| asupersync | `=0.5.0` | Sole async runtime (known defects and mitigations: plan §31.4) |
| Rust toolchain | `nightly-2026-08-31` | Suite-wide pin |
| frankensim | commit per lifted file (`registries/lifts.toml`) | Geometry, sparse solvers, exact predicates |
| franken_accounting | revision in `franken-stack.lock` | Book of record, reached as a subprocess |
| Issuer filings | sha256 per file in the source registry | Public-lane parity fixtures |

Fixtures are captured from references and never regenerated from the candidate. A mismatch is a defect until
proven otherwise.

---

## Data, confidentiality and privacy

- **No site data, MNPI or personal data in this repository.** Confidentiality is monotone (INV-007): MNPI-derived
  bytes never reach a public export, and there is no global index over embargoed data.
- **Ownership basis is part of every figure.** 100% project basis vs attributable, at the stated interest, for
  example NGM 61.5/38.5 or Pueblo Viejo 60/40.
- **Source documents are not committed.** The repository carries evidence notes that cite them by file, page and
  cell. One note is held back pending owner review (`planning/corpus/C5`).

---

## Scope boundaries

Not a control system. Not a reserve classifier, a dam sign-off or a compliance certificate. Not a general ledger.
Not a replacement for ERP, fleet management, geological modelling or mine-planning packages in years 1–2: it reads
their exports. Not a general MILP solver. Not an aggregate ESG score.

---

## Repository layout

```text
COMPREHENSIVE_PLAN_FOR_THE_DESIGN_OF_FRANKENMINING.md   the technical constitution
AGENTS.md                                               the operating contract for agents
planning/mor/MOR_CASE.md                                the evidence register (E001–E852)
planning/corpus/                                        evidence notes on the source documents (C1–C6)
planning/research/                                      research R1–R8 and primary-source passes W1–W5
planning/brenner/                                       hypothesis passes B1–B6
planning/mor/audits/                                    AUDIT_1–AUDIT_5
crates/                                                 the fmine-* workspace crates (scaffold in progress)
fuzz/                                                   coverage-guided fuzz targets: decoders, CLI (scripts/fuzz.sh)
registries/                                             verbs, units, CRS, calendars, refusals, lifts, budgets, topology
docs/                                                   ADRs, delivery notes, CONTRACT template
.beads/                                                 the work graph (JSONL is truth)
```

---

## Reading paths

- **What and why:** this README, then plan §1–§5.
- **The epistemics:** §7–§10, then the galaxy-brain cards (Appendix D).
- **The first deliverable:** §21.6, §31.1 and §37.
- **A subsystem:** its section (§11–§24), its guards and proof obligations, then its beads in §39.
- **Implementing:** `AGENTS.md`, plan §28–§29, the crate's `CONTRACT.md`, then `br ready`.

---

## FAQ

**Is this a Barrick product?** No. It is independent, and it uses Barrick's public disclosures as its
demonstration data because they are detailed, public and auditable.

**Does it replace Leapfrog, Whittle, Deswik or the ERP?** No. It reads their exports and makes their outputs
evidence: cited, versioned, scored and reconciled.

**Will it tell me a dam is safe?** No. It reports the evidence state of each failure mode. Missing, stale or
conflicting instrumentation is shown as such, and humans sign.

**Why Rust and a custom runtime?** Every certificate must replay bit-identically offline (INV-010). One audited
async runtime with structured concurrency, cancellation and a deterministic lab makes that testable.

**Why so much planning before code?** The expensive mistakes in this domain are false certainty and wrong
architecture. Examples are a number that reaches a reserve disclosure without its uncertainty, or a substrate
that cannot scale to 10⁸–10⁹ blocks. The plan names its kill gates so those mistakes surface early.

---

## License

MIT with the OpenAI/Anthropic rider. See [LICENSE](LICENSE).
