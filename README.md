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
| Is there code? | **Yes, and the public lane runs.** `fmine assure public --issuer barrick` runs end to end on the 18 registered workbooks (not in git) and reproduces the state root pinned in `repro/barrick/expected.toml`. The M0 gate is not passed: the M0 report, the security pack and the hashed FY2026 forecasts are still open |
| Has anything been benchmarked? | **No.** Every speed and scale number in the plan is a TARGET |

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

The same engine compares the AISC workbooks under `issuer_consistency.aisc_vintages`: 9,541 appearances,
5,819 series and 3,722 later comparisons, all unchanged in the registered corpus. This includes 359 explicitly
labelled co-product cash-cost rates, kept distinct from the by-product rates used by the identity ledger.

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

DESIGN boundary: these are arithmetic checks on issuer assertions. Different filing/deck vintages remain
separate documents. The command does not determine which vintage is true, classify reserves or establish
GATE-HALL-01. Its PDF report digest binds the declarations, readings and results; it is not a ledger anchor or
external timestamp. Authentic historical PDF readings and complete historical coverage remain open.

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

Complete quarter sets can also establish **H1 = Q1 + Q2** and **FY = Q1 + Q2 + Q3 + Q4** for those
same additive quantities. For example, `Example Mine H1 2030 100% basis gold production is 300 koz.`
can be supported by dated Q1 and Q2 disclosures of `100.0` and `200.0`: their exact sum enclosure is
`[299.9, 300.1]`. A draft stating `300.0` would claim more precision than those inputs establish.
The `quarter_sum` report preserves the interpreted input Claims, canonical derived Claim and complete
source ancestry. Every quarter must be present on the same subject, metric, ownership and scope basis;
competing eligible vintages stay ambiguous. Missing or undated quarters cannot become zero, and a
supporting sum cannot conceal a directly disclosed contradiction. Rates and ownership scaling remain
outside these arithmetic rules.

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
fraction as though they were the same unit. Bare/mined grade, circuit recovery and reserve replacement
remain outside discovery's supported grammar. Independently labelled general-prose accuracy remains unmeasured.

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
`production_kt`, `aisc_usd_per_oz`, `aisc_usd_per_lb`, `tcc_usd_per_oz` and `c1_usd_per_lb`.
Production basis is `100%` or `attributable:n/d`. Cost basis also declares its definition, for example
`attributable:4/5;aisc=issuer-reported-per-sold-oz`; the analogous per-sold-lb, `tcc` and `c1` definitions are
explicit. Registered runs require complete two-reader agreement across the selected guidance namespace.
The retrospective command also refuses unsupported guidance quantities; the dated comparison modes retain
them as diagnostics. The pure source-binding API can retain pending guidance readings alongside admitted rows.
Missing or changed source bytes refuse. The synthetic command is an invented known-answer demonstration
through the same binding and scoring code.

#### Restrict original-outcome cohorts to an evidence date

```bash
cargo run -p fmine-cli -- track-record --issuer barrick --cohort-as-of 2026-10-31 --site north-mara
cargo run -p fmine-cli -- track-record --issuer barrick --cohort-as-of 2026-10-31 --through 2026Q2 --format markdown
cargo run -p fmine-cli -- track-record --synthetic --cohort-as-of 2032-12-31 --format markdown
```

The optional `--cohort-as-of` selects a dated projection through the same checked source binder. It uses registered
document publication dates and conservative workbook-retrieval availability. The retrospective command above
does not claim an information-availability date. Neither mode authenticates an external timestamp or registers
a forecast. Units, ownership bases, canonical Claim identities, and metric definitions are checked in both.
An unprinted ownership basis is never inferred to mean full ownership.

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

### Mirror two reporters' gold production or sales

```bash
cargo run -p fmine-cli -- mirror --synthetic
cargo run -p fmine-cli -- mirror --synthetic --format markdown
```

DESIGN (BD-346): the mirror checks whether two reporters' printed gold quantities admit a common 100%
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
`--synthetic` is exclusive with source selectors. JSON is the default; `--workspace` selects the local root.

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

Real partner-filing coverage for NGM, Pueblo Viejo and Kibali remains open, including independently read
ownership and dated perimeter evidence. The unresolved NGM vend-in dates and shares are not supplied by
defaults. Capex, cost-definition and accounting bridges, and full BD-346 acceptance, remain open.

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

### Compute the frozen issuer-guidance baseline

DESIGN (BD-334): the forecast record fixes two comparators. The existing `forecast-baseline` command computes
persistence; an explicit publication-date cut also evaluates issuer guidance from the registered files:

```bash
cargo run -p xtask -- forecast-baseline --as-of 2026-11-01
```

The command retains every site and target in the frozen rules. Production guidance is divided by the share
printed with that guidance, then the original Q3 release's nine-month production on the 100% basis is
subtracted. AISC guidance remains its published per-sold-ounce interval: unit costs are not subtracted across
periods. For example, synthetic attributable guidance `[240, 300]` at a 60% share and nine-month production
of `300` gives an exact Q4 comparator of `[100, 200]`. The calculation uses the displayed workbook number,
while its citation retains both stored and displayed values. Negative remainders are reported unchanged.

The latest agreed guidance on or before the cut is selected. Competing values on its latest publication date,
newer pending readings, missing original actuals, or unsupported labels remain explicit unavailable rows.
Excluded sites remain named `not_published` members. Before the Q3 workbook is registered, production
comparators remain unavailable; their guidance evidence is still shown. AISC comparators can be available
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
Q4 AISC uses its printed quarterly rate directly. Outcomes use the exact **displayed** numeric value while
retaining the stored token and source Claim for audit; printed zero counts, and a displayed dash stays missing.
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
reduced by the original nine-month displayed total; the AISC range is used directly. Signed remainders and
point ranges remain unchanged, and issuer guidance is assigned no nominal probability.

Persistence uses the frozen original Q3 observation and original consecutive-quarter ratio pool, with
displayed numbers, source Claims, pool exclusions and manifest membership retained. Those fixed benchmark
observations can follow the forecast cutoff; the report makes no claim that the methods had the same
information. Missing or unreadable required historical sources refuse the comparator instead of silently
shortening its pool. Printed zero is a valid persistence point.

For each target, all three interval-score means and both forecast-minus-comparator differences use exactly
the same eligible sites. Every excluded site lists all missing inputs. A negative difference favors the
forecast under this loss, without establishing skill or calibration. Optional comparator failures, including
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
freezes and generated source resolutions use v3. Callers must not include this marker in their request files.
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
Supported metrics are gold production (`production_koz`) and issuer-reported AISC per sold gold ounce
(`aisc_usd_per_oz`), at site level over a calendar year (`YYYYFY`) or individual calendar quarter.
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
Missing original releases, altered bytes, placeholders, ambiguous cells and ownership mismatches refuse
without changing the journal. Caller-supplied `outcome` requests cannot bypass a frozen public binding.
Historical reports reconstruct the recorded source Claim without requiring the workbook to remain on disk.

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
later revisions. Automatic missing-status events remain future work.

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

The report includes the guidance vintage, as-of selection and all source hashes. Workbook availability uses its
verified retrieval date conservatively where no publication date is registered. JSON preserves exact Claims,
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
pull, and apply annual training to quarterly observations. Operational attribution, recoverable production,
forecasts and dollar sensitivity are outside this output.

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
  - reproduce every per-site value in Barrick's published quarterly Mine Stats workbooks (2024Q1–2026Q2, plus
    Q3 2026 when it is released), each cell cited by `<file-period>:<Sheet>!<Cell>` and file hash;
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

## Workspace and budget

The crate map has 59 crates in 7 layers, each with a LOC budget (`registries/loc_budget.toml`; ceiling 1.15× plan).
A crate joins the workspace only with its first real slice. The first slice covers:
- the S0 substrate: `fmine-core`, `-units`, `-time`, `-crs`, `-evidence`, `-claim`, `-registry`, `-array`;
- `fmine-numerics` and `fmine-calib`;
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
