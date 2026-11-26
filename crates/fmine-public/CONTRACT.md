# CONTRACT.md — `fmine-public`

## 1. Purpose
The public-filings assurance, five-state identity ledger, keyed public Claim index, cross-vintage comparison, guidance track-record, public waterfall, and disclosure-lint crate (Portal, layer `V`, M0 `fmine assure public`).

- **Crate:** `fmine-public`
- **Layer:** V
- **Subsystem:** Portal
- **LOC budget:** 24,000 planned / 28,000 ceiling
- **Allowed Module Families:** `["sources", "aisc", "mine_stats", "mine_ledger", "synthetic", "live", "citation", "key", "consistency", "transcribe", "forecast", "guidance", "waterfall", "recovery", "lint", "html", "mirror"]`

## 2. Inputs
### Mathematical & Domain Formulation
The public lane of M0 (`fmine assure public`).

- DESIGN (BD-273/BD-332): visible company `Total` or `Total Gold` columns in an explicitly labelled gold
  AISC reconciliation enter the shared public Claim index under `Total Gold` and `Scope::Company`.
  Admission requires the gold reconciliation title, USD-million heading and attributable thousand-ounce
  denominator. Printed headers, signed amounts, placeholders, repeated rows, original locators, hashes
  and every reporting vintage survive; no numeric ownership share is inferred. Duplicate company columns
  remain separate source assertions. The existing registered-subject identity population is unchanged.
  These Claims participate in supplemental AISC vintage inspection and the CLI's company-versus-regions
  comparison, without claiming a complete company perimeter or an accounting reconciliation.

- DESIGN (BD-067/BD-334): `forecast::build(workspace, rules_bytes, as_of, decimal_places)` produces an
  unissued quarterly-ratio candidate through `fmine_calib::record::candidate`. It reads only the original
  Mine Stats releases required by the declared historical pools and their common persistence point. The supported
  target meanings are 100%-basis gold production in koz and copper production in kt (`fy_minus_9m`),
  printed gold AISC or total cash cost per ounce sold, and copper C1 or AISC per pound sold (`q4_qtd`). Rates are never
  differenced across cumulative periods. Explicit `quarter_qtd` targets support those same six metrics
  for the immediately following quarter, including Q4-to-next-year-Q1. Every target in a quarterly record
  uses that outcome and the same point quarter. Ordinary candidate cuts precede the scored quarter's end.
  Explicit `quarter_qtd_nowcast` candidates instead require a cut at or after quarter end and before the
  frozen proof deadline, with that deadline strictly before the declared earliest outcome release.
  `nowcast_publications` retains registered scored-quarter workbook and HTML publication dates with
  FACT-cited bases; absent/inferred dates refuse and every known publication day starts an exclusive cut
  boundary. Earlier preliminary announcements therefore tighten the bound even when a workbook arrives
  later. The registry is not assumed complete. Drafts may lack an outcome release; authenticated scoring
  requires the original workbook registry entry and preserves missing outcome bodies as unresolved.
  Publication boundaries remain separate from historical model inputs and are bound by the registry digest.
  Original FY rules keep their common Q3 point and existing output bytes.
  `fmine_calib::record::TargetMetric` supplies the same exact
  field/outcome/unit interpretation used by the record scorers; metal quantities and cost families are
  separate pools. Every input uses the exact displayed
  decimal and the canonical source Claim. A missing original release, ambiguous cell, unreadable or changed
  source, unsupported target, unprinted or changing cost ownership, or nonzero observation overlapping a
  registered site status refuses. No source value is rescaled or filled from a later comparative.
  Historical placeholders, absent rows and nonpositive printed values remain named observations; excluded
  ratio pairs and every `not_published` site remain visible. Live production points must be positive and usable.
  Canonical gold AISC/TCC and copper C1/AISC cost points may be signed or zero, retaining their exact displayed
  value and printed ownership. Their negative multipliers reverse the scaled tail endpoints; zero remains
  a complete zero candidate. The historical positive-ratio pool is unchanged. A live cost point overlapping
  a registered site status refuses even when its raw cell contains zero; status-derived Nil and placeholders
  never become zero forecasts. All admitted cost points participate in the ownership-consistency check.
  The receipt binds the exact rules bytes and FMRUL1 digest, raw source-registry bytes, declared information
  cut, source digests, source Claims, model version and exported candidate. Source-specific `retrieved_utc`
  overrides take precedence over the legacy registry batch value, and retrieval intervals use their upper
  endpoint; date-only publication claims are admitted only through the end of that day. Required
  quarters must have ended by retrieval, and all availability bounds must precede the declared cut. Synthetic
  source registrations retain a synthetic report label. This checks a declared information cut against the
  captured registry; it supplies no independent proof of the registry's historical existence, author access,
  attestation, pre-registration, calibration or forecast skill. It does not freeze or issue the candidate.
  Historical positive 100%-gold production remains eligible under a canonical `Withheld` flag only when
  the same original workbook contains exactly one subject-matched, same-year note explicitly withholding
  per-ounce data. The original Claim, status evidence and displayed production remain unchanged; the
  receipt records the note locator as an inclusion reason. This exception never supplies missing or zero
  production, costs or live points, and never overrides disposal, care-and-maintenance, conflicting
  statuses or broader withholding notes. Direct controls are
  `forecast::tests::original_historical_production_survives_source_proven_per_ounce_withholding` and
  `withholding_exception_never_supplies_missing_notes_costs_points_or_disposal_support`; the ignored
  genuine-source test checks the original Loulo-Gounkoto Q1 2025 production cell and independent pools.
  Known-answer and negative controls: `forecast::tests::exact_displayed_observations_produce_complete_source_bound_candidate`,
  `missing_rows_placeholders_and_nonpositive_history_are_audited_without_imputation`,
  `availability_is_conservative_and_completed_quarters_cannot_look_ahead`,
  `incompatible_targets_ownership_changes_and_status_conflicts_refuse`, and
  `receipt_binds_rules_registry_information_cut_and_source_values`.
  Copper controls additionally cover three simultaneous target pools, exact CSV round trips, kt versus
  pound/ounce refusals, attributable-production targets and changing cost ownership:
  `copper_candidates_keep_kt_and_both_per_pound_costs_in_separate_source_bound_pools` and
  `copper_candidate_refuses_unit_aliases_attributable_targets_and_ownership_changes`.
  `forecast::build_with_model` also accepts the explicit seasonal empirical candidate model. The original
  `build` API keeps the all-season model and its output encoding. Seasonal selection occurs only after
  authenticating and reading every original release and observation required by the frozen historical
  pools and point quarter. Other-season sources remain mandatory, including their timing, ownership,
  status and byte-authentication checks; selecting a season never reduces the source corpus.
  Only positive adjacent transitions ending in the validated outcome's calendar quarter enter the
  seasonal ratio pool. Every other-season pair and unusable selected pair remains auditable, and an
  empty selected pool refuses. The receipt binds the model identifier even when two models produce the
  same numeric CSV; edits to other-season source data also change the receipt while leaving seasonal
  quantiles unchanged when their selected transitions are unaffected. Signed cost points, information
  cuts, source Claims and unissued status retain the same admission rules. The registered all-season
  persistence comparator remains a separate unchanged baseline; seasonal selection establishes neither
  calibrated coverage nor forecasting skill.

- DESIGN (BD-057/BD-059): `html::run(workspace, issuer, definitions, as_of)` turns registered HTML
  table cells into canonical public Claims. It parses the exact captured source registry and mapping
  bytes, verifies selected source lengths and SHA-256 digests, and uses `HtmlDocument`'s exact numeric
  tokens, decimal values and printed steps. The floating compatibility projection is never an input.
  A mapping has no amount field. Gold/copper AISC and cost of sales, gold TCC, copper C1, gold production
  and sales, copper production, gold-equivalent production, processed/head grade and recovery use bounded
  explicit metric/unit labels, exact source-label equality and a registered non-region site.
  Ownership must be printed as `100%` / `100% basis` or `Attributable (<percentage>%)`; percentages are
  reduced exactly. Periods come from canonical period codes or the existing explicit sheet-period forms,
  never table position. Source registration supplies the issuer and release; original and comparative
  periods keep that release independently. HTML line keys have their own `html:` namespace: importing a
  printed metric does not assert equivalence to a workbook definition, resolve a GEO price deck or establish
  a TCC credit treatment.
  - Source citations retain the requested slot, merged-cell origin, original UTF-8 byte range, decoded
    text and spans. The same data origin cannot become two observations; repeated semantic keys within
    a vintage refuse. Labels may share merged origins. Numeric, nil, not-applicable and empty-cell
    missing values remain distinct. Registered disposal/withholding makes a printed zero Nil and flags
    nonzero figures; the exact numeric token remains available alongside that interpretation.
  - Each Claim binds its value origin, all captured semantic-label origins, the mapping-file SHA and
    declaration locator, and the captured source-registry SHA. The registry supplies site classification,
    release/publication metadata and any status rule. These are cited interpretations of issuer assertions,
    never calibrated measurements or evidence of publisher authenticity. Existing Workbook Claim IDs retain
    their original single-anchor encoding through `key::public_claim_anchored`.
  - Optional publication cuts exclude later mapped sources before any file access. Excluded sources remain
    listed and are never replaced by earlier files. An entirely excluded selection has zero Claims and
    explicit excluded-source metadata. Mappings/registries are bounded to 1 MiB, declarations to 4096 rows,
    and HTML files retain the reader's limits; source and mapping paths must remain inside the workspace.
    No browser, network request, inline-XBRL transform, unit conversion or table-layout heuristic is used.
  - Mapping schema `fmine.html-claims.v1` requires top-level `schema` and `issuer`; each `[[claim]]` has
    `id`, registered HTML `file`, 1-based `table`, A1 `cell`, and the five pairs `subject_cell`/`subject_label`,
    `period_cell`/`period_label`, `metric_cell`/`metric_label`, `unit_cell`/`unit_label`,
    `ownership_cell`/`ownership_label`. Labels must match decoded source text exactly before their bounded
    grammar is interpreted. Cost units explicitly name USD per ounce sold or per pound sold; named gold
    and copper cost labels cannot cross those denominators. Production/sales labels name the metal:
    gold and GEO use koz, copper production uses kt; no ounces/pounds/tonnes rescaling is performed.
    Grade uses g/t or percentage points; recovery uses percentage points. All supported aliases are
    checked by the adapter. Generic production/sales, copper sales and produced-denominator costs refuse.
    Synthetic source fixtures cover the executable path; real issuer corpus coverage is not asserted.
    The optional `accounting_close_cell` explicitly cites the adjacent closing parenthesis when
    `cell` holds the opening parenthesis and digits. The joined token retains its printed precision
    and negative sign. Both origins enter the canonical Claim, and the receipt retains a separate
    byte range for each. Either fragment missing, annotated or out of position refuses.

- DESIGN (BD-345): `lint::lint_cited(draft, claims)` checks a bounded equality grammar against canonical
  source assertions. It indexes Claim IDs and latest series once, then compares the named subject, supported
  metric, period, ownership, denominator and unit. Exact rational half-step intervals decide containment,
  disjoint values and excessive precision; no floating-point tolerance or implicit unit conversion is used.
  Dated nine-month cumulative equalities use the existing canonical `NineMonths` period in cited checks,
  discovery and publicness queries. They stay distinct from H1, Q3 and FY; no year is supplied for undated prose.
  AISC, total cash costs, copper C1, cost of sales, gold/copper production, gold sales, processed/head grade
  and whole-site recovery are supported. AISC and cost of sales include canonical copper summary/site
  rows with explicit pound-sold denominators and `usd_per_lb` units; their rounding, source identity and
  ownership checks are the same as for gold, with no ounce/pound conversion. The HTML source keys join
  these same checks for gold/copper costs and production, and gold sales. Explicitly printed copper-sales
  Mlb rows supply typed extensive operands for conditional quarterly copper rates; copper tonnes and
  production rows are not converted into sold pounds. Printed HTML AISC/TCC/C1 labels
  never establish by-/co-product treatment: an explicit credit-basis assertion remains unchecked.
  Explicitly printed gold co-product TCC and AISC stay distinct from by-product rates. Their cited and
  discovery checks require the co-product qualifier and retain original publication cuts, source hashes
  and ownership evidence. A direct printed co-product rate is used without inventing a credit add-back.
  Canonical copper
  C1 summary/site rows require explicit `$/lb` or `US$/lb` and per-pound-sold denominators; by/co-product,
  gold-ounce and produced-denominator assertions are not inferred from C1. Copper production requires an
  explicit copper metric and canonical `kt`; historical millions-of-pounds site rows stay unchecked.
  Grade requires explicit `g/t`
  (or `grams per tonne`) or percent units; recovery requires percent. Percent-formatted source fractions
  retain their stored values and use their explicitly displayed percentage-point rounding intervals;
  `mass_fraction` without a percent display stays unchecked. Bare/mined grade and circuit recovery are
  outside this grammar. Unsupported prose and source values remain unchecked, and ambiguous citations or
  semantic context never produce `verified`. Each candidate's Claim ID, metric and canonical value tag are
  rechecked against its current key and value before verification; altered Claims stay unchecked.
  The caller supplies the source-verified snapshot. Staleness means a later release of the same canonical
  series in that snapshot or explicit Claim supersession; it is not inferred from a wall clock. The report
  binds exact draft bytes, canonical Claims, rules version and findings. It exposes numeric spans and source
  Claims without copying entire draft sentences. It does not decide publication availability, disclosure
  permission, qualitative truth or calibration of source numbers.
- DESIGN (BD-345.S3): `lint::lint_discover(draft, claims)` accepts the same bounded equality grammar for
  AISC, gold TCC, copper C1, cost of sales, gold/copper production, gold sales, processed/head grade and
  whole-site recovery. Exact subject, dated period,
  ownership, denominator, definition and unit select candidates before any numeric comparison. One candidate
  receives the cited-mode checks; multiple
  candidates, including repeated vintages or conflicting source assertions, stay ambiguous and are listed.
  Values never break ties. Explicit citations keep cited-mode checking, and every finding records its mode.
  The source snapshot remains the caller's responsibility; discovery does not infer publication dates.
  This executable slice has synthetic known-answer and adversarial checks. Reserve replacement and the
  independent labelled-set precision/recall assessment remain open; no general-prose accuracy is claimed.
- DESIGN (BD-345.S4): `lint::equality_target(draft, span, prototype)` parses the same bounded equality
  against a prototype's subject, metric, ownership, definition and unit without using its source amount
  or requiring a Claim for the requested period. The returned `EqualityTarget` carries that explicit
  period and the draft's exact decimal value/step. It supplies no evidence that the target is published
  or derivable. The caller must retain competing source populations, resolve ownership/perimeter ambiguity
  and authenticate the publication cut. Cited checking and this semantic query share the pure
  `fmine_io::numtok::equality` context grammar, parameterized by known subjects and citation spans;
  unsupported residual prose, ambiguous/malformed links and missing required basis never become matches.
- `sources`: the source registry (`BD-055`, `BD-322`, bd-s9j3.1), read at run time from
  `<workspace>/registries/sources/<issuer>.toml` (schema `fmine.sources.v1`, checked by `fmine_registry::sources`
  and by `xtask registry-check`). It pins each public workbook by file name, URL, byte size and SHA-256 (Barrick:
  Mine Stats 2024Q1–2026Q3 and AISC reconciliations 2024Q3–2026Q3, from W1, E451, E452, E859), and holds the reporting
  subjects printed in the AISC table headers and the registered reconstructions (E816). Registering a vintage is
  a TOML edit, never a code change; `through(YYYYQn)` keeps only rows at or before a quarter, so an earlier run
  reproduces after later rows are added. Document-only partner profiles use the same schema and document
  verification path; workbook assurance and workbook Claims require at least one registered workbook.
  Per-source `retrieved_utc` overrides preserve later-release timing without changing older batch evidence.
  Source inventories, Claim traces, forecast availability and public-record retrieval bounds use the same
  effective value. The numeric identity root remains independent of timing metadata; candidate receipts
  and public-record comparison digests retain their relevant timing evidence. Retrieval and publication
  remain distinct, including for undated sources and intervals that cross midnight.
  FACT (BD-055). The source inventory reports each loaded workbook's registered filed reconciliation
  page, PDF digest and optional SEC accession. These references apply to the selected workbook
  vintage and do not enter the numeric state root. The workbook run verifies the workbook bytes.
- `aisc`: a label-anchored reader for the AISC reconciliation tables. It finds rows by column-A label and the
  header row by registered subjects, so no coordinate is fixed. It also builds the identity ledger
  (`GATE-BULL-01` part 2, plan §21.6).
  - **Declared construction:** component sum. TCC = Σ block-1 lines; AISC = TCC + Σ block-2 lines. Each rate is
    r = 1000·N/Q, checked on intervals (N ± k/2, Q ± 1/2, r ± 1/2).
  - **Kernel** (BD-331.S6): every footing and rate decision is a run of `fmine_numerics::exact_feas` (BD-340)
    under the `Closed` tie convention, whose checkers re-verify each witness and certificate.
    - A footing is posed under `ComponentSum` and its alternative under `RoundedOnce`.
    - A placeholder line is an exact 0 (a zero step) and never counts in k.
    - In the AISC alternative, TCC stands for its k₁ numeric block-1 lines.
    - A (iii)'s residual and sign-flip witness come from the kernel's residual certificate. Its total and line
      cells are S3's one minimal conflict set.
    - The (iv) range is the kernel's implied interval, rounded outward to three places.
    - A kernel that makes no claim refuses the instance (`exact_kernel_indeterminate`).
  - **States:** (i) consistent; (ii) explained by a registered reconstruction, re-verified on the run and
    labelled `explained_stated` or `explained_reconstructed` by whether an issuer note states the rule;
    (iii) issuer discrepancy, with an exact certificate and any single-line sign-flip witness;
    (iv) undecidable, naming the missing fact (registered by subject and family, else the family default) and any
    closing document; (v) refused; not published (excluded by name).
  - **Alternative construction:** subtotal rounded once. It is reported, never scored.
  - **Hidden scope** (BD-331.S2): the same families run over hidden worksheets into `hidden_scope`. Each instance
    is labelled "sheet not displayed to readers". It is bound into the state root in its own section and never
    counted in the totals, `by_family` or `GATE-BULL-01`.
  - **One-cell check** (bd-prmn.8, Appendix K.4): a diagnostic, never a state and not in the state root. On
    every registered-subject column of the visible tables, cost of sales per ounce is checked against
    1000·s·N/Q at the column's basis. The factor is the registered, label-confirmed share where an operating
    NCI line before TCC is non-zero, and one otherwise. A sustaining-only NCI adjustment never rescales the
    earlier cost-of-sales numerator. Factor one preserves the printed numerator; it does not establish
    100% ownership. Each column reports the unscaled check, the check at the registered share,
    and the implied ounce interval. `one_cell_verdict` counts the failures outside a caller's K.4 items:
    "site-specific" if there are none, otherwise "basis rule".
  - **Construction discovery** (bd-4wvl.4, BD-340.S4 part 1): a report, never a state and not in the state root.
    For the TCC and AISC footings in states (i) and (iii), visible and hidden scopes apart, it asks which
    construction they fit (`fmine_numerics::exact_feas::discover`). Each footing's k is its numeric lines and
    its residual is Σ lines − total. A footing printed again with the same subject and numbers (a later
    release's comparative) counts once.
- `mine_stats` (bd-gwf1.1): a label-anchored reader for the quarterly Mine Statistics workbooks (W1 §3, §4, §13).
  It finds blocks by `For the (three|six|nine|twelve) months`, takes site names from the row above (carried left
  across merged blanks), and strips trailing footnote numbers from labels. The `QTD`/`YTD` summary columns are
  mapped to field ids from populated header groups and their explicit year anchors, without fixed column
  ranges or a first-sixty-rows limit. Site-sheet year headers likewise have no fifteen-column cutoff.
  Blank year cells preserve merged-header support; missing or ambiguous year support and unknown fields
  produce cited refusals rather than inheriting a neighboring year's meaning. Each data cell becomes a
  typed value (`Number` with its stored text, `Nil`,
  `NotApplicable`, `Missing`) with locator `<quarter>:<sheet>!<cell>`, period, site, printed ownership share,
  label and section. Numeric zero stays `Number`. Cells outside the token set, unknown periods and labels
  repeated within a block are refused by locator; the first row of a repeated label is listed
  (`MineStatsRead::repeated`). With the issuer's `[[mine_stats_label]]` vocabulary (Barrick:
  56 pairs from W1's read, frozen before the Q3 2026 release), a data row whose `(section, label)` is not
  registered is refused too. W1-DR-01 is data (`[[printed_share_correction]]`, read by `mine_ledger`), and so is
  W1-DR-02 (`[[mine_stats_label_alias]]`, read by `mine_ledger`). The keyed Claim adapter also applies these
  evidenced corrections and resolves explicit units without rescaling the stored values. The region map is
  (`[[region_member]]`, BD-331.S4); raw reader values do not carry its version, while the keyed projection
  records the release's membership digest in each region's scope.
- `mine_ledger` (BD-331.S4, bd-prmn.4): six Mine Stats identity families anchored on the `QTD`/`YTD` summary
  sheets, decided in exact arithmetic. Ownership: attributable = s × 100% per site row and column group, s the
  share the label prints or its registered correction (the printed share kept beside); regions and metal totals
  pose none; a row with no printed share is refused, never read as 100%. Quarter sums: H1 = Q1 + Q2,
  9M = H1 + Q3, FY = 9M + Q4 on the 100%, attributable and sold columns, anchored on each `YTD` cell. Summary
  against site sheet (family 7): each summary value of attributable production and sales, cost of sales, TCC or
  C1 and AISC equals the same site's value on a `Minestats*` sheet of the metal (A1 title `(Gold)` or
  `(Copper)`), same release, period and year (`SITE_FIELDS`), with a registered label alias applied by release;
  subjects with no site-sheet column pose none. Sites to region (family 3): a region row = Σ its registered
  members in the release's map on the gold flow columns, a member found by its subject or its registered Mine
  Stats label (refused at the first row when printed under both); a region with no member in the map is refused
  (`region_not_in_map`). Region to total (family 4): `Total Gold` = Σ the map's regions, `Total Copper` = Σ the
  copper sites in row order. Site-sheet quarter sums (family 2 on the `Minestats*` sheets): the same quarter sums
  on each additive row (`SITE_QUARTER_ROWS`: tonnes mined and processed with their sub-rows, ounces or pounds,
  revenue, capital expenditures; never rates, grades, recovery, strip ratio or unit costs), anchored on each
  cumulative block's cell, the previous release's block (Q1 for H1, else its cumulative) plus the anchor
  release's quarter block, a site found by name on whichever sheet of its metal it prints in each release. A
  sum's nil token is exactly 0; a sum is decided alone by |Σ parts − total| ≤
  Σ half-steps (the kernel's answer for one rounded-once sum, checked by a test) and never posed jointly. The
  `sites_to_subtotal` appends to the stable family tags: a registered subtotal's three gold flows equal its
  component mines under the distinct `subtotal_member` registry. Membership must cover the operand's whole
  physical period, not its release: NGM's 2023 comparative includes Long Canyon; 2024 onward excludes it
  (W1 §10). Partial-period membership refuses (`cross_map_subtotal`), and no applicable members refuses
  (`subtotal_not_in_map`). Subtotal and member aliases, missing rows, placeholders and withheld statuses
  follow the sum rules; both a subtotal's and its members' withheld statuses apply. Regional maps are unchanged.
  Operands follow registry order; the subtotal instance keeps its source's printed subject.
  The report retains the cited membership declarations in `mine_stats_ledger.subtotal_members`; M0AS v9
  binds every subtotal, member, period bound and evidence string even when the numeric states do not change.
  Profiles without subtotal declarations retain their v8 certificate encoding.
  Sold-weighted rate (family 5): a
  region's cost of sales, TCC and AISC per ounce (`RATE_FIELDS`) = its members' rates weighted by attributable
  gold sold, members found as in family 3, in row order. A member whose sold cell is the nil token leaves the
  identity; a placeholder left in it refuses (a rate is never read as 0), as do no member left
  (`weights_all_nil`) and a weight at or below 0 (`weight_not_positive`). It is decided alone and exactly: two
  separable affine bounds select weight vertices in linear work without a member-count ceiling, and a feasible
  answer carries a witness checked exactly. Empty/reversed boxes, weights whose bounds reach zero, arithmetic
  overflow or a failed witness check yield no answer. The identity is never posed jointly. A conflict is (iv)
  `cost_totals_unpublished`, not (iii): the issuer may
  weight by quantities it does not print. Exclusions by name come first (a
  `withheld` status covering an operand's period, or a region member's; a region whose members differ between
  the operands' releases; a site on two site sheets of one metal), then a missing row or release, a label printed
  twice in a block, a blank, all placeholders (not published), some placeholders, an unreadable number. On a
  site sheet only `—`, `— ` and `n/a` are tokens; the nil token against a number reads as number = 0, decided
  alone (`Instance::nil_token`, E116). Each value as displayed stands for its closed box [v − step/2, v + step/2],
  the step from the displayed decimals (General non-integers are refused). Every decided instance but the sums
  and the sold-weighted rates is then posed jointly; each infeasible component reports every minimum correction
  set (an exhaustive search up to 32 cells within the kernel's run cap) and the range each of its cells can
  take, never a corrected value, and whether it is infeasible only jointly.
- DESIGN (BD-331.S4): `MineLedger::cost_layers` reports each `Total Gold` cost-of-sales, TCC and AISC rate
  minus its release-map regions' attributable-sold-weighted rate. Exact closed half-step rounding boxes
  supply the regional-mean and difference bounds; signed differences and intervals containing zero remain
  unresolved cost breakdowns. The residual identifies no corporate G&A, JV or other component and does not
  reconcile the company sales denominator. Nil regional sales are explicitly excluded with both operands
  retained; unavailable operands, nonpositive weights, missing maps, ambiguous releases, period mismatches
  and bounded-arithmetic failures stay visible without a numeric result. The separate projection digest
  binds workbook hashes, every operand/rounding step, release and period, active map declarations, results
  and limitations. JSON and Markdown expose it outside identity counts and the existing M0 certificate.
- `synthetic`: invented workbooks in the real layout, seeded with one known answer per state. They are never
  gate-eligible. `write_issuer_workspace` writes them, plus a later title-only vintage, with a digest registry
  under the reserved issuer name `synthetic`; `run_public_assurance` runs that name in the synthetic lane.
- `guidance` (BD-342): selects an agreed production/AISC guidance pair from the latest eligible registered
  document at the explicit as-of date, verifies its bytes and connects it to the requested Mine Stats YTD
  Claims. Units, period, ownership and the issuer-reported per-sold-unit AISC definition must agree. Gold
  uses production/sales in koz and AISC in USD/oz; copper uses production in kt, sales in million lb and
  AISC in USD/lb. Copper requires exactly one declared site sales row: million lb directly, or kt converted
  by the exact factor 100000000/45359237, including its full displayed rounding interval. The original
  source key, token, unit interpretation and anchors remain cited beside the derived conversion rule.
  Registered stale-label corrections remain visible; production and summary rows cannot replace site sales.
  The latest guidance must identify one complete gold or copper pair, with no mixed-metal selection. A pending
  row in an equally recent or later eligible document prevents silent fallback to an older agreed pair.
  Remaining sales explicitly assume the preceding year's same remaining-month sales, derived from its original
  FY and YTD releases. Production is never substituted for sales. Dated completion selects Mine Stats by
  cited workbook publication date, falling back to the verified retrieval upper bound only for undated rows.
  The cut precedes byte access; unrelated AISC inputs and positively future workbooks are not opened.
  `guidance::history` forms every complete contiguous original-vintage window of the remaining duration,
  including cross-year windows, strictly before the guidance year. A later comparative never fills an absent
  original release. Missing, unusable or incompatible windows are listed with their source refusal; same-season
  is context only. The exact quarter model retains sales/cost dependence and intersects the separate sales
  scenario before making a joint comparison. Checked witnesses preserve original and hypothetical supports;
  checked exclusion certificates apply only to the supplied finite windows. No usable window stays T3, and
  solver caps remain indeterminate.
  Net AISC rates retain their printed sign, including zero-crossing rounding boxes, through exact YTD
  spending, remaining requirements and joint historical comparisons. Sales must remain strictly positive.
  `xtask/tests/public_guidance_copper.rs` checks negative and printed-zero costs from authenticated invented
  filings through JSON/Markdown output, with independently calculated bounds and checked quarterly witnesses.
  `guidance::capacity` independently selects the latest eligible registered `technical_report` with agreed
  `technical_<site>_remaining_dry_feed_mt`, `technical_<site>_head_grade_g_per_t` and
  `technical_<site>_recovery_percent` rows. Units must be `dry_mt` (million dry metric tonnes),
  `g_per_dry_t` and `percent`; common full-mine basis must declare
  `100%;scenario=declared_upper_bound`, `100%;scenario=plan` or `100%;scenario=nameplate`.
  Support must exactly match the remaining months (`YYYYQ2-Q4`, `YYYYH2` or `YYYYQ4`). Recovery divides
  by 100 exactly; ownership scales the output only. Declared boundaries are exact, while plan/nameplate
  points preserve half-last-place rounding and remain conditional scenarios. No annual proration or
  implicit wet/dry, grade-average or ownership conversion is allowed. Missing, changed, disputed, pending or
  inapplicable optional technical evidence becomes a typed technical T3 without discarding the requirements
  or historical assessment. Selection never falls back over a newer incomplete or inapplicable document.
  DESIGN (BD-342): copper uses the same feed/recovery quantity suffixes, with `head_grade_percent`
  instead of `head_grade_g_per_t`, units `dry_mt`, `percent_dry_mass` and `percent`, and an explicit
  `;metal=copper` suffix on all three common scenario bases. Both percentages convert exactly to
  fractions; dry Mt times grade times recovery times 1000 yields kt before ownership scaling.
  JSON and Markdown retain the copper units, required dry-mass grade fraction, and source evidence.
  Gold factors cannot enter a copper calculation. Both grade keys participate in source selection so
  an incomplete or wrong-metal newer report cannot silently select older complete inputs.
  These source-to-report tests use invented
  public-format evidence and do not establish authentic issuer acceptance or prospective calibration.
- `guidance::track_record` (BD-341): `run(workspace, issuer, site, as_of)` verifies the source registry,
  workbook bytes and two-reader document transcriptions before comparing registered annual guidance with
  same-site, same-year Mine Stats full-year outcomes. `assess` accepts already authenticated projections;
  `synthetic` exercises the same adapter with invented evidence. `TrackRecordReport::to_json` and
  `to_markdown` retain guidance citations, outcome Claims, source digests, refusals and a deterministic report
  digest. The digest binds a descriptive report, not a forecast ledger or historical timestamp proof.
  The explicit information-cut mode emits `fmine.guidance.track-record.as-of.v1`, distinct from the CLI's
  retrospective `fmine.guidance.track-record.v1` payload and its workbook-release `--through` selection.
  Both entry paths select all `guidance_` quantities before checking document bytes and reader agreement.
  Unsupported guidance keys remain in scope; optional technical readings cannot block the guidance report.
  Every reader at a selected locator remains, including a conflicting quantity key. Registry syntax and
  document identity validation remain global.
  - Supported transcription quantities are `guidance_<site>_production_koz`, `production_kt`,
    `aisc_usd_per_oz`, `aisc_usd_per_lb`, `tcc_usd_per_oz`, `c1_usd_per_lb`, `capex_usd_m`,
    `capex_sustaining_usd_m` and `capex_projects_usd_m`. Each requires an explicit
    full calendar year and matching unit/ownership basis. Cost bases additionally name
    `;<aisc|tcc|c1>=issuer-reported-per-sold-<oz|lb>`. The supported, agreed registered population is
    explicit; unsupported quantities are named, and absent historical guidance is not invented.
    Capital rows require `usd_m` and the matching `;capex=issuer-reported-total`,
    `;capex=issuer-reported-minesite-sustaining` or `;capex=issuer-reported-projects` suffix. The shared
    `capex_definition` mapping selects original annual site rows for either metal, keeping the printed
    ownership and signed amount. Total is never reconstructed from components; missing originals and
    ambiguous source appearances remain unresolved. These reported-line comparisons infer no cash/accrual,
    lease, FX or ownership bridge and supply no royalty-price adjustment.
  - Every agreed guidance vintage remains visible. Summaries use the earliest registered document date per
    site/year/metric/normalized basis; tied earliest rows yield one unresolved denominator member. Later
    revisions cannot replace an initial miss or create extra members. Distinct metrics, units, ownership
    shares and years have separate summaries; regions and issuer totals do not enter site cohorts.
  - The original annual release and latest registered appearance have separate assessments. A comparative
    without the original remains `comparative_only_original_missing`; it never fills the first-published
    denominator. Missing, unreadable, withheld, disposed, stale and incompatible outcomes remain unresolved,
    retaining every selected source. Guidance whose registered exact-byte availability is after the target
    year cannot acquire a historical score. The explicit date-level cut uses each workbook's cited publication
    date or, when absent, its retrieval upper bound. Reports bind the actual per-source basis and exclusions;
    they do not prove historical registry existence. An empty eligible workbook projection preserves every
    guidance member unresolved and reports a null workbook availability bound. Supplied forged or unregistered
    future Claims remain unusable evidence; only authenticated source bindings can be excluded by the cut.
    Registered disposal or withholding overlapping any part of the target year also prevents scoring an
    unflagged annual stub. Disposal's closing month is recovered from the profile's first-full-month rule,
    including a December closure; the registered evidence remains in the refusal. Maintenance alone does
    not suppress a genuine annual zero. The source cell must equal its Claim anchor's file, locator and hash.
  - `fmine_calib::guidance_scores` computes exact printed-value hit classifications, rounding ambiguity,
    range midpoint/half-width and standardized distance. A declared `ScoringMember` exists for each earliest
    member, including missing outcomes. Completion bounds, rounding-aware completion bounds and the exact
    outward 90% IID Clopper-Pearson reference interval remain separate. Point guidance has no standardized
    distance. All results are `Descriptive` or `Unscored`; no calibration threshold is inferred.
- `recovery` (bd-srqc): fits constant implied tails and an OLS tails-on-head-grade relation to the agreed
  annual Lumwana observations for 2013–2022, using `fmine_metrics::recovery`. It projects both onto each
  later visible site-quarter's head grade, recovery, processed mass and reported copper production.
  Displayed percent units and closed half-last-place bounds survive conversion; stored Excel fractions are
  not mistaken for percent values. All four quarterly inputs come from the same earliest registered
  workbook appearance. A later comparative never repairs an unusable first appearance. Comparative-only
  periods retain that label, including the 2023 observations in the registered 2024 releases.
  - Full-mine interpretation requires a separate, agreed ownership citation from the checked AIF.
    It is explicit context on the derived physical Claims; original source keys, including unprinted basis,
    remain unchanged. Annual intensive ratios are never multiplied by an ownership share.
  - The model returns exact point estimates and conservative input-rounding enclosures for expected
    recovery, signed gap in percentage points, copper difference in kt and the fraction of reported output.
    Its marginal coefficient bounds and centred prediction bounds are not confidence or prediction intervals.
    The residual sample SD uses `SSE/(n−1)`; the conventional regression residual-error SD uses `SSE/(n−2)`.
    Both have separate names and numerical square-root enclosures at the printed inputs.
- `key` (BD-332.S2/S3): projects visible AISC arithmetic operands and Mine Stats reads into source-anchored
  `Claim<PublicValue>` values. Subject, period, release, basis, scope, unit and semantic line belong to the key;
  coordinates remain citations. The raw readers and numerical identity ledgers retain their existing behavior.
  `canonical_value_tag` exposes the existing key/raw/typed-value encoding so adapters can reject changes to
  displayed values or status interpretations without changing any existing Claim identity.
- `consistency` (bd-504e.1): groups visible AISC identities by the total or rate's `SeriesKey` and identity
  family. It reports the state at the earliest registered release once per identity, retains the all-files
  counts, and reports every adjacent appearance. "Earliest registered" never asserts first publication.
  Unknown keys and ambiguous earliest releases are named and excluded from the first-registered denominator.
  This implements the AISC diagnostics for part 3a and adjacent histories for part 3c; it does not implement
  part 3b's full restatement classification or a composite issuer-consistency pass score.
- DESIGN (BD-504e): `consistency::mine_stats` applies the same state-count and transition rules to all seven
  Mine Stats identity families. It resolves the ledger's release-relative locators against the complete
  Mine Stats Claim population before grouping by semantic anchor key and family. Current and comparative
  columns can join one history; distinct ownership, units and region-map versions cannot. Multiple source
  candidates remain unresolved, and missing operands cannot establish persistence or correction.
  Printed subject labels use only the exact registered share-label mapping already used by the Claim
  adapter. The projection retains raw subjects and binds that mapping in its digest; unregistered label
  differences remain unkeyed.
- `lib`:
  - `load_registered_sources` verifies every registered file;
  - `assess` builds the ledger and the state root;
  - `assess_memo` is `assess` with a `MineStatsMemo` (bd-r7lp). The memo reuses the Mine Stats reads and ledger
    of its last computed run when the profile and every Mine Stats source's name, quarter and parsed workbook
    are equal. The live proofs use it, because their perturbations move only AISC cells. A synthetic sweep of
    every one-cell edit checks that its anchor, reads and ledger equal `assess`'s;
  - `PublicAssuranceReport::to_json` is the envelope payload;
  - `cell_to_public_claim` is the public-basis `Claim` builder (`BD-058`).

## 3. Outputs
### Authority, Epistemics & Representation
- Every retained numeric issuer assertion is `Assertion { source: Issuer }`, state `Known`, status `KnownKnown`, and every reported
  instance says so (`"evidence":"Assertion","source":"issuer"`). A reproduced figure is `Derived` over assertions,
  so by NL-2 it is still an `Assertion`: its figures agree with each other, which does not show they are true.
  A placeholder cell, including a status-derived `Nil`, is `Assumed`, state `Unknown`. Keyed numbers for the
  reserved synthetic issuer are also `Assumed`, state `Unknown`, and labelled `synthetic` in JSON; they never
  acquire issuer authority. Nothing here labels an issuer figure stronger than that.
- This report is not the issuer's disclosure, so it does not call G-DISC (`ClaimDag::guard_disclosure_export`).
  No output implies that FrankenMining or Barrick attests a figure.
- Every reported exception cites file, file SHA-256, `sheet!cell`, subject, family and ownership basis
  (`IdentityFamily::basis`).
- The `LedgerAnchor` state root (BLAKE3 over an `M0AS` v8 frame) binds:
  - every input's SHA-256, numeric digest and census, and (v5, bd-jg43.3) its displayed-value digest with the
    shown, token and refused counts (`Workbook::displayed_census`);
  - every instance's state as codes, numbers and locators (`encode_state`), never its prose, with a tag before
    each optional part;
  - each instance's operands (role, locator, value);
  - unregistered headers and table refusals;
  - the same for the hidden scope, in its own section;
  - (v6, bd-prmn.4) the Mine Stats ledger in its own section: each instance's family, subject, field, group,
    period, anchor, cells, state code, reason and evidence, share and component, then each infeasible
    component's cells, flags and correction sets with their implied ranges. Its counts stay apart from the
    AISC totals. v7 adds the site-sheet family and each instance's nil-token certificate. The sum families
    (families 3–4) add instances in the v7 frame, with no change to its format. The sold-weighted rate
    (family 5) adds the first (iv) instances: after the state code `iv`, the frame binds the need's code where
    (i) and (iii) put a zero byte, so the state code already says which branch follows. The site-sheet quarter
    sums add instances in the v7 frame, with no change to its format;
  - (v8, BD-333) the public waterfall's canonical digest, including its pair outcomes, cited inputs,
    exact printed-rate reconciliation and outward bar enclosures. Previous root epochs remain recorded.

  Changing any input cell moves the root; rewording a message does not (bd-sjrh.1 part 4).
- The keyed projection has its own BLAKE3 digest over a `PCLM` v1 frame, sorted by key and locator. It binds
  every accepted cell's locator, Claim id, canonical key, typed value (including stored/displayed text or a
  display refusal and status), printed basis, basis and registered-status evidence, printed label and unit evidence. The Claim id
  itself includes a `PVAL` v1 digest of the typed interpretation, so changing a zero from a number to a
  registered `Nil` cannot reuse its numeric Claim id. This projection has its own digest; M0AS v8 additionally
  binds the waterfall's use of its source Claims. Keying refusals and unregistered all-zero columns
  are reported separately; they are not silently represented as accepted cells in this digest.
- Issuer-consistency counts and histories are a report over the existing ledger and keyed projection. They
  do not alter the numerical states or the identity-ledger root. Their binomial intervals describe the
  registered corpus counts, with no calibrated forecast claim or pass threshold.
- The report payload carries `lane` and `gate_eligible`. The no-claim line (`NO_CLAIM`), the four INV-002 flags
  and the next actions (`PublicAssuranceReport::next_actions`) are envelope fields, which `fmine-cli` prints.

## 4. Invariants
### Source binding, keying and public-lane invariants

- DESIGN (BD-355): `fmine compare` reuses registered issuer profiles and the existing authenticated
  transcription loader. Its bindings file contains only issuer/document/quantity selectors and must
  match the full selected population. The metrics kernel returns exact ranks and comparative-claim
  verdicts through canonical source/result Claims with JSON and Markdown provenance, without
  changing existing assurance identity totals or denominators. Two readers, document hashes, FACT
  publication evidence, annual period, units and matching definitions are prerequisites; a refusal
  retains the complete declared population. This bounded same-definition comparison supplies no
  transferred bridge, automated free-prose interpretation or proprietary cost-curve claim.
- `sources::{SourceKind, RegisteredSource, RegisteredReconstruction, RegisteredShare, RegisteredMissingFact,
  IssuerProfile, load_issuer_profile, registry_path, registered_issuers, is_issuer_name, is_through_bound}`;
  `IssuerProfile::{from_registry, through}`; `IssuerProfile::{regions, shares}` (BD-331.S3);
  `RegionMemberRow`, `IssuerProfile::{region_members, members_of}` (BD-331.S4: a region's registered members
  for a release, in registry order);
  `IssuerProfile::missing_facts`, `RegisteredReconstruction::basis` (BD-331.S2);
  `ExpectedRoot`, `expected_roots_path`, `load_expected_roots` (BD-339.S1);
  `kit_selectors_markdown`, `kit_readme_section`, `KIT_SELECTORS_BEGIN`, `KIT_SELECTORS_END` (BD-339.S4: the
  generated section of `repro/<issuer>/README.md`, from the row anchors and the registry; a golden test
  compares)
- `PublicAssuranceReport::{registry_epoch, census_digest_of_digests, check_expected}` (BD-339.S1)
- `PublicAssuranceReport::exception_json` exposes the existing cited exception projection for the CLI packet
  renderer without changing its numerical state or the assurance root.
- `guidance::{run_public, run_synthetic, GuidanceCompletionReport, NO_CLAIM}` (BD-342). The report carries exact
  input/output Claims, source and intermediate historical Claims, accepted guidance rows, source hashes,
  explicit sales assumption and a content digest. It exposes `history`, `history_sources` and
  `history_exclusions`, with T1 when usable original-quarter windows exist and T3 otherwise. Its independent
  `technical: capacity::TechnicalAssessment` is `Compared(Box<TechnicalComparison>)` (T2) or
  `Indeterminate { reason }` (T3). A comparison retains `TechnicalScenarioKind`, checked document, accepted
  source rows, `CapacityLimits` and the exact production/required-grade `CapacityReport`. Both JSON sections
  and their source metadata bind the content digest. This is not a forecast registration or ledger root and
  supplies no gate-eligibility claim. Source epistemics and unknown-unknown terms survive quantity conversion.
  DESIGN (BD-342): numeric conversion rechecks the source Claim ID, metric, full public value tag and
  matching document anchor before minting derived quantity Claims. Edited stored/displayed values,
  ownership/unit/vintage keys and hashed source metadata refuse instead of acquiring fresh ancestry.
  Guidance interpretations must still parse from the exact reader-agreed token. Newly issued, consistent
  source assertions remain admissible; stale and unavailable historical windows retain their exclusions.
- `recovery::{assess, run_public, run_synthetic, RecoveryReport, QuarterReport, RULES_VERSION}` and
  `PublicAssuranceReport::recovery` (bd-srqc). `RecoveryReport::{digest, to_json}` emits
  `fmine.recovery.gap.v1`, binding the accepted source rows, fit, selected quarterly source Claims, results
  and refusal codes with its own digest. The existing identity/waterfall state root is unchanged.
  The payload defines the ownership, annual and converted operating parent Claims with exact quantities,
  rounding bounds, parents and source anchors, alongside the unchanged raw workbook Claims. Inherited
  quantified UU exposures retain their cited values; the model's own unquantified term remains explicit.
  - `assess` returns no section when no recovery inputs are registered. Once registration is started,
    incomplete or conflicting historical/ownership evidence refuses explicitly. Valid training with an
    unusable operating quarter retains that quarter as a named refusal, with computed/refused counts.
  - `run_public(root, issuer, through)` reads hash-verified Mine Stats Claims and current verified model
    documents. Its optional `through` selects workbook releases only: this is a retrospective computation,
    not an historical as-of record or hindcast. The dedicated command requires model inputs; absence refuses
    with `FMINE-REF-201`. `run_synthetic` uses invented observations through the same adapter and kernel.
  - Unbounded ordinary assurance includes the optional section when the local transcription registry
    supplies the model inputs. Ordinary assurance's existing `--through` behavior still skips PDF
    transcriptions and therefore this section.
  - The local transcription registry uses `fmine.transcriptions.v1` with issuer `barrick`. For each year in
    2013–2022, `recovery_lumwana_<year>_head_grade_pct` has unit `wt_pct`, and
    `recovery_lumwana_<year>_recovery_pct` has unit `percent`; both use period `<year>FY`, basis `100%`,
    table `Appendix A: Past Production Records for Lumwana`, the year as row, and columns
    `Head Grade (% Cu)` and `Recovery (%)`, respectively. These fields cite the presentation's printed
    page 39, with each original printed token and its precision preserved.
  - `recovery_lumwana_ownership_pct` uses unit `percent`, basis `100%` and period `2025FY`, citing the
    `Copper Mineral Reserves` table on the checked AIF's PDF page 38 (printed page 37), row
    `Lumwana surface (100%)`, column `Based on attributable tonnes`. Every accepted field needs two distinct
    agreeing reader roles, a complete page/table/row/column locator and the registered PDF SHA-256.
    PDF bytes and real historical row values remain local and ignored under OPEN-R7-1; source URLs,
    exact-byte hashes, byte counts and availability qualifications are registered.
- `aisc::{IdentityFamily, IdentityState, IdentityInstance, FootingCertificate, UnregisteredHeader, AiscTable,
  RateMiss, ShareBasis, ExplanationBasis, DEFAULT_MISSING_FACT, SHARE_MISSING_FACT, RowAnchor, AnchorRule,
  ConsistentBasis, MemberWeighting, WeightedMember, REGION_MISSING_FACT (BD-331.S4),
  ROW_ANCHORS (the seven rows `locate_table` anchors on, BD-339.S4), locate_table,
  ledger_for_table, rate_holds, rate_range, admitted_rates, KernelIndeterminate, feasible_share,
  feasible_share_text, find_share_footnote, find_qtd_sheet, confirm_share_label, reconstruction_holds,
  match_subject, cell_text,
  OneCellCheck, OneCellOutcome, OneCellVerdict, FailGroup, implied_ounces, one_cell_checks, one_cell_verdict}`
  (bd-prmn.8); `PublicAssuranceReport::one_cell`, the payload's `one_cell_cos` section.
- `aisc::{FootingDiscovery, footing_discovery}`, `PublicAssuranceReport::construction_discovery` and
  `discovery_json` (bd-4wvl.4): the payload's `construction_discovery` section, after `hidden_scope`, holds one
  object per family and scope. It carries counts and log tails only, never a cell value; a tail of −∞ is
  `null`.
- `PublicAssuranceReport::copper_unit_discovery` and the payload's `unit_discovery` section, after
  `construction_discovery` (bd-4wvl.4 part 5, E463). The site-sheet instances on `cu_prod_attr_kt` and
  `cu_sold_attr_kt` whose site row is labelled "(millions of pounds)" pair the site value with the summary
  value, both as displayed (`mine_ledger::displayed_number`). Units kt and Mlb (1 Mlb = 45,359,237/10⁸ kt,
  from `AVDP_LB_TO_GRAMS_NUM`) are tried on those numbers alone, apart from the registered alias. The section
  holds counts only and is not in the state root.
- `transcribe::{load_transcription_report, load_transcription_report_as_of, load_transcription_quantities_as_of,
  load_transcription_quantities_for_period_as_of,
  load_transcription_prefixes, load_transcription_prefixes_for_period, load_guidance_transcription_report,
  transcription_json, TranscriptionReport, CheckedDocument,
  TieOutcome}`, `sources::{DocumentRow, transcriptions_path}`, `IssuerProfile::documents` and
  `PublicAssuranceReport::transcriptions` (bd-plx1, bd-4wvl.4 part 2). When
  `<workspace>/registries/transcriptions/<issuer>.toml` exists and the run has no `--through` bound (documents
  carry no release quarter), `run_public_assurance`:
  - checks the bytes and sha256 of every document the registry cites, and refuses naming each missing
    (`FMINE-REF-506`, exit 5) or altered (`FMINE-REF-505`, exit 8) file; uncited documents are never read, so
    CI, which caches only the workbooks, needs none;
  - applies the two-reader check and refuses (`FMINE-REF-202`, not observable, exit 3: the evidence conflicts)
    naming each locator where readers disagree, with
    the differing field names and never their values; a locator one reader has entered is pending and listed;
  - feeds the accepted point values that two or more documents print under one quantity key to
    `discover_tie_conventions`. A quantity whose prints no convention reconciles (its `Closed` meet is empty)
    is a conflict, listed with its locators, and the discovery re-runs without it.
  - The guidance-specific as-of loader filters future documents before byte checks, reader agreement and tie
    analysis; it still validates registry syntax and document identities. General assurance checks all cited
    documents as before.
  - `load_transcription_quantities_as_of` further scopes source verification to requested quantities and an
    optional selected document, retaining every reader at each selected locator, including a conflicting
    quantity assignment. Guidance and optional technical inputs use separate scopes so an unrelated or
    unusable optional quantity cannot erase valid required guidance. Explicit capacity selection binds only
    its three requested roles within the named document. Registry structure and document identity validation
    remain global.
  - `load_transcription_prefixes` uses the same single-parse selection, with an optional as-of date. Explicit-cut
    track record selects `guidance_`. Unsupported guidance remains visible while unrelated technical sources
    are excluded before byte checks and agreement. Every reader at each selected locator remains.
  - DESIGN (bd-067.1): `load_transcription_prefixes_for_period` selects an exact period at that same raw-locator
    boundary. Quarterly forecast scoring can use a filing containing both quarterly and annual guidance
    without unrelated annual quantities or pending readers suppressing its quarterly comparator. Every reader
    at a selected locator remains, so period or quantity disagreements still refuse. Registry syntax and
    document identities remain globally checked; selected source bytes and information-cut checks still apply.
    Explicit period selection uses the registry's period-scoped parser before quantity uniqueness, so normal
    current/comparative columns can coexist. `load_transcription_quantities_for_period_as_of` applies this
    same path to exact quantity keys and an optional selected document for capital-return filings. All readers
    at a selected locator remain; missing current operands never borrow from a comparative period.
  - DESIGN (BD-341.S5): `load_guidance_transcription_report` explicitly scopes the agreement gate to
    guidance, selecting the union of `guidance_`, `aisc_price_assumption_` and `realized_price_` quantities
    for retrospective track record. Rejected optional price readings remain in the returned report and cannot
    be used for a price split. Unrelated technical sources are excluded; all selected-locator readers remain.
    The optional evidence cut is validated before any absent-registry return and applied before agreement;
    all selected in-cut document byte/hash checks remain mandatory. Mixed guidance/optional identities or unknown
    rejection identities refuse. Both general loaders retain their stricter disagreement behavior.
  - The dedicated recovery loader deliberately uses the current verified historical documents even with a
    workbook `through` bound, and states that retrospective interpretation in its payload. It does not change
    the ordinary assurance loader's bound behavior or infer historical availability from a PDF cover date.
  - The payload's `transcriptions` section, after `unit_discovery`, holds citations (file, kind,
    `published_at` and its basis, sha256), counts, the accepted-rows digest, pending locators, conflicts and each
    document's convention finding (FMINE-REF-104 when ambiguous); never a transcribed value. It is `null` when
    the registry is absent and is not in the state root.
- `aisc::{Operand, OperandRole, OperandValue, operand_at}` and `IdentityInstance::operands` (bd-sjrh.1, part 2
  of BD-329.S1). Each instance cites every cell its state was computed from, with its role (total, line,
  rate, ounces, numerator, NCI line, component, label check, member rate, member ounces) and its value as read.
  - **Cited cells:**
    - the table cells of the instance's family;
    - every used cell, for an all-zero column;
    - the footnote and the confirmed share label, for a cost-of-sales rate with a non-zero operating NCI line
      before TCC; sustaining NCI rows remain cited but do not trigger ownership conversion;
    - each printed member's rate and ounces, for such a rate of a region with registered members;
    - a consulted reconstruction's quote, label-check and component cells.
  - **Not cited:** structure cells (row labels, subject headers, a sheet's title and period line).
  - The numerical operands are cited in the payload and bound into the state root; the separate keyed
    projection adds typed Claims for arithmetic cells without turning textual context into numerical Claims.
  - **Test:** `test_operands_cover_every_input_cell` perturbs every other synthetic cell, one at a time, and
    fails on any state change it causes that the operands do not cite.
  `IdentityState::Consistent` carries `basis`: `ConsistentBasis::Share` (the stated share, footnote and label
  it used), `ConsistentBasis::MembersExact` (a region's weighting, reached by a weighted mean checked exactly),
  `ConsistentBasis::Members` (a region's weighting, `relaxation_only`: no conflict shown, no value proved; the
  report counts these), or `None`;
  `IdentityState::Explained` carries `basis: ExplanationBasis`; `IdentityState::Undecidable` carries
  `missing_fact` and `closing_document`; `IdentityState::RefusedAt` is (v) naming a cell or file.
- `mine_stats::{View, MineStatsCell, MineStatsValue, CellRefusal, MineStatsRead, LabelVocabulary, read_mine_stats,
  strip_footnote, printed_share, site_name, is_m0_target}`
- `mine_ledger::{mine_ledger, MineLedger, LedgerRules, LedgerFile, Instance, Family, Group, Metal, State, Reason,
  Need, Component, Correction, FLOW_FIELDS, SITE_FIELDS, RATE_FIELDS, SITE_QUARTER_ROWS, range_text, rat_text}`;
  `PublicAssuranceReport::mine_ledger`, the payload's `mine_stats_ledger` section (counts by family and state,
  (v) by reason, discrepancies with their certificate, each (iv) with its need under `unresolved`, and each
  infeasible component with its correction sets and implied ranges).
- `synthetic::{synthetic_aisc_cells, synthetic_hidden_cells, synthetic_workbooks, synthetic_stats_workbook,
  synthetic_share_label_cells, synthetic_profile, write_issuer_workspace, SYNTHETIC_ISSUER, SYNTHETIC_SUBJECTS,
  SYNTHETIC_SHARES, SYNTHETIC_SHARE_FOOTNOTE, SYNTHETIC_STATED_QUOTE, SYNTHETIC_HIDDEN_SHEET,
  SYNTHETIC_LATER_STATS_FILE}`
- `live::{CheckKind, LiveCheck, LiveReport, Loader, prove_files}` (BD-338.S1): the file half of the
  live-dependency proof. For each registered file, the cases are absent, zero bytes and a foreign xlsx, each in
  its own scratch workspace. Each must refuse "1 of n registered", naming only that file under "missing" or
  "digest mismatch": absent is `FMINE-REF-506` (exit 5), the other two `FMINE-REF-505` (exit 8)
  (`CheckKind::expected`).
- `live::{prove_leaves, LeafCheck, Perturbation, leaf_summary, rate_flip_up, rate_flip_down}` (BD-338.S2): the
  leaf half. Each operand of each visible instance is perturbed alone, on a copy of the parsed sources with
  that file's census recomputed.
  - **Pass rule:**
    - the field at the leaf reads the new value, including a footing certificate's `printed`; otherwise
      "value did not move";
    - a field derived from the leaf (component sum, k, rate range) or the state moves; otherwise "dead
      ancestor";
    - the root moves.
  - **Perturbations:**
    - +1, or 1 for a placeholder;
    - a counted line to "-" (k − 1);
    - a plain-consistent rate to its flip margin, or 3r + 1000 for a stated-share or explained rate;
    - a decisive NCI line to 0;
    - a label to "x".
  - **Skips:** a state decided before arithmetic, a share refusal's non-cause leaves, and an unrounded value
    are skipped, each with its reason.
  - **Threads** (bd-r7lp): the checks are independent runs, spread over `available_parallelism()` scoped
    threads, so `run` must be `Sync`. The result is in instance and operand order, equal to a sequential pass
    (`test_live_leaves_threaded_matches_sequential`).
  - **Not yet:** hidden-scope instances.
- `live::{prove_one_leaf_per_instance, prove_labels, LabelCheck}` (BD-338.S3): the real-lane
  frame and the label sample.
  - The sample starts at the original modulo choice in each instance and scans cyclically until an
    operand has a perturbation plan. Selection happens before any mutation is run. A selected
    failure is reported, and if no operand has a plan the original skip is reported.
  - The run reports attempted perturbations by source file. The historical gate requires an attempted
    pass with a moved root for each of its 2,595 instances.
  - `prove_labels` mutates each of the seven anchor labels of a located table (append "x"; prepend it for the
    ounces prefix). Each must give a table refusal or changed instances.
  - **Real lane** (ignored, local runs):
    - files: 54 of 54;
    - leaves (one per instance): 2,595 pass, 0 fail and 0 skipped through 2026Q2 (E872). The previous
      modulo-only sample passed 2,492 and explicitly skipped 103 non-decisive NCI operands (E871);
    - labels: 56 of 56 table refusals;
    - the Kibali O11 literal mutant is caught.
  - **Finding, fixed:** a broken cost-of-sales label made `locate_table` return `Ok(None)`, so the table dropped
    out with no refusal. `assess` now refuses a reconciliation's own-quarter sheet by name when it has no table
    ("no cost-of-sales row on the release's own quarter sheet") or is missing ("the release's own quarter sheet
    is missing").
- `aisc::Calc` and `IdentityInstance::calc` (bd-sjrh.1): a footing's printed total, component sum and k; a
  rate's r, n, k and q, as the ledger read them. `citation` renders them in the payload, and the root binds the
  operands they come from. `OperandRole::Counted` marks lines read only for their count: rate lines, and block 1
  under the AISC footing.
- `citation::{Cited, RULES, RULES_VERSION, extracted, derived, cited_json, locator}` (bd-sjrh.1 part 3): the
  cited numbers of an instance.
  - **Extracted:** each operand is `{"role","v"|"token"|"blank"|"unrounded","at","sha256"}`.
  - **Derived** (`{"name","v","rule","rules_version","in"}`, rules version `m0-ledger/1`):
    - a footing gives `component_sum` (`aisc.footing.component_sum`, from its lines) and `residual`
      (`aisc.footing.residual`, from its total and lines);
    - a rate gives `k` (`ledger.count`, of its counted lines or the cost-of-sales row) and `range_lo` and
      `range_hi` (`aisc.rate_interval`, rounded outward).
  - **Payload:** every exception record carries `cited`, and an issuer discrepancy's certificate cites printed,
    component sum and residual. The top-level `instances` array holds one `{"at","family","state","cited"}`
    record per counted instance: 55 synthetic (column N, bd-jg43.3) and 2,595 real.
  - **Size:** the real payload is about 7.3 MB.
  - **Not yet:**
    - totals and census counts as count citations;
    - the uncited-number validator (S2).
- **Structured states** (bd-sjrh.1 part 1):
  - `aisc::{RefusedReason, NotPublishedReason}`: each has a unique snake_case `code()` and a `why()` text with no
    digits.
  - `aisc::{Undecided, ReconstructionCheck}`: a (iv) is `RateOutside(RateMiss)`, `ShareOutside` or
    `ReconstructionNotApplied { evidence_id, check }`. Each check (quote, file, sheet, label, whole number,
    closes) carries its own fields.
  - `aisc::StatedShare` is a stated-share basis, and `aisc::CitedCell` an explained component.
  - `IdentityState::reason_code()` gives the code that the payload's exception records and refused and
    not-published groups carry as `reason_code`. `detail()` renders the same prose as before.
    The visible `all_zero_column` group includes `status_citations` when a registered subject status covers
    the full period of a column. Each citation gives the column locator, workbook SHA-256, period, subject,
    status and registry evidence. The presence of a status citation leaves the ledger verdict and state root unchanged.
- `Lane`, `LoadedSource`, `TableRefusal`, `AlternateCounts`, `HiddenScope`, `HIDDEN_SCOPE`,
  `PublicAssuranceReport`, `load_registered_sources`, `assess`, `assess_memo`, `MineStatsMemo` (`hits`,
  `misses`), `run_public_assurance`, `run_barrick_m0_assurance`,
  `run_synthetic_assurance`, `cell_to_public_claim`, `NO_CLAIM`; `PublicAssuranceReport::next_actions`
- `key` (bd-a28f.1/.2/.3, BD-332.S1/S2/S3): the key of a printed public figure, and the value typed with its raw token.
  - `PublicKey { issuer, subject, valid_period, reporting_vintage, ownership_basis, scope, unit, line }`.
    `series_key()` drops the vintage and share provenance: `SeriesOwnershipBasis` retains the exact effective
    attributable fraction, with `Consolidated100`, `Attributable` and `Unprinted` still distinct. A registered
    correction and the same share later printed explicitly can therefore belong to one series. Provenance
    remains on `PublicKey` and in the Claim id. `canonical_tag(raw_token)` is `fmk1` plus every field and the
    token, length-prefixed. `public_claim` combines it with the typed-value digest for `Claim::new`, so a
    different key, token or typed interpretation has a different claim id. `series` retains and sorts every
    vintage rather than overwriting comparative figures.
    AISC cost of sales per ounce receives an exact attributable share when the same sheet states the applicable
    footnote and the registered share matches the Mine Stats ownership label in the release and in every quarter
    of the printed period. The rate Claim anchors the footnote and those workbook cells. A missing or conflicting
    label leaves the rate basis `Unprinted`. The rate arithmetic verdict and published value stay separate.
    Comparative and cumulative AISC sold-ounce Claims use the same release and period checks and anchor the
    row label and corroborating workbooks. An original-quarter row with an explicit attributable label uses
    its registered share. The historical waterfall independently checks and binds its same-release label.
  - `parse_sheet_period`: `Qn YYYY`, `YTD Q2 YYYY` (H1), `YTD Q3 YYYY` and `YTD Sept 30 YYYY` (nine months),
    `YTD Dec 31 YYYY` and `YYYY` (full year). Anything else refuses. `parse_period_code` reads the Mine Stats
    reader's corresponding `YYYYQn`, `YYYYH1`, `YYYY9M` and `YYYYFY` codes without inferring a calendar.
  - `parse_ownership`: `100%` first, then a final `(x%)` after a one- or two-digit footnote run. No `%` gives
    `Unprinted`, never 100%.
  - `classify`: `—` and other dashes give `Nil { Unknown }`, `n/a` gives `NotApplicable`, and no cell gives
    `Missing`, each keeping the token as stored. A number keeps its stored text and its displayed text, or the
    display refusal code. A numeric zero where a registered disposal or withholding applies is respectively
    `Nil { reason: Disposed }` or `Nil { reason: Withheld }`; the JSON value is `kind: "nil"` with that reason
    in lower case. A non-zero number there stays a `Number`, flagged `after_disposal` or `withheld`. Without
    a matching status, zero remains a number. Care and maintenance leaves zeros numeric. Text, a formula
    error or a number without stored text refuses.
  - `SubjectStatus`, `status_for`: the `[[subject_status]]` rows of the issuer registry, carried on
    `IssuerProfile::subject_statuses`. A disposal covers the periods that begin after its closing date.
  - `scope_for`: region scope hashes sorted, distinct members of the map effective at the reporting release;
    no registered members gives a release-specific unknown map. Company totals are company scope; other
    subjects are site scope. No joint-venture kind is inferred without a registered classification.
  - `KeyedCell { locator, key, claim, printed_basis, basis_evidence, status_evidence, label_as_printed, unit_evidence }`,
    `public_claim`, `canonical_value_tag`, `raw_token`, `series`, `claims_digest`, `KeyedCell::to_json` and
    `PublicAssuranceReport::public_claims`: the existing public-assurance JSON includes `public_claims` with
    schema `fmine.public_claims.v1`, its separate digest, cells, refusals and `unregistered_all_zero`.
    Each accepted cell carries its Claim id, `file:sheet!cell`, file SHA-256, complete key, original and
    effective basis with evidence, typed value and authority. This is part of the existing command, not a
    new mode or endpoint.
    Visible instance and exception operands carry `claim_id` when the canonical inventory accepts their
    exact locator and source digest. The optional field is absent from label checks, derived values and
    hidden-scope records. Review packets still use their canonical evidence projection.
  - `from_mine_stats`: binds the reader's release/cell locator and exact stored numeric bits/text or token to
    the workbook. With a source registry it also requires matching filename, SHA-256 and release. It retains
    the printed ownership basis while applying the registered correction, keeps the original row label and
    alias evidence, and uses the alias only within its registered releases. Units describe the stored number:
    a supported percent display over a stored fraction uses `mass_fraction`, preserving both texts; an
    explicitly printed percent-unit number without that display uses `percent`. Corrections never silently
    rescale a value. Unknown units, periods or source bindings produce named keying refusals.
  - `key::mine_stats::{KeyedClaims, build_claims, load_claims}` is a thin inventory over that same adapter,
    not another Claim implementation. `mine_stats_claims` reports its typed-value census, named refusals and
    canonical `claims_digest`. The `by_file` member includes each loaded Mine Stats workbook's SHA-256, accepted
    value-kind counts, the `after_disposal` subset of numeric Claims and the count of refusals associated with
    that workbook. Missing kinds count as zero. Even a verified workbook with no accepted Claims is listed.
    Refusals without a verified workbook stay in the named global list. This reporting projection leaves Claim
    identities and the underlying evidence root unchanged. The direct reader supports `fmine claim show` without
    running the identity solver.
- `key::consistency::{VintageReport, VintageState, compare_vintages_with_disclosures}` and
  `PublicAssuranceReport::{mine_stats_vintages, aisc_vintages}` add separate value-comparison populations
  under `issuer_consistency`. Every keyed appearance is counted. Comparisons retain both the earliest registered and
  previous release, match semantic keys rather than cell coordinates, and compare displayed decimals exactly.
  Percentage glyphs are normalized to the key's stored-number unit using exact rationals. Unknown conversions
  and refused displays remain `not_comparable`; no stored precision is substituted for displayed precision.
  - Changed values or token classes are `restated_disclosed` only when an applicable verified registry note
    covers them, otherwise `restated_silent` after a complete check. An omitted or incomplete check leaves
    `changed_without_registered_explanation`. These findings never select a true vintage or establish that
    no disclosure exists elsewhere. Same-release ambiguity and actual printed ownership disagreements remain
    conflicts; registered share provenance alone is not a conflict. Region-map changes remain `not_comparable`,
    with both assertions and any verified explanatory note retained.
  - The real-source regression checks E474 and all eight W1-DR-09 changes. W1 T16's 6,391/9 is passes/failures:
    the six per-release populations equal 6,400 in total after excluding the 30 cited region-map changes.
    The full classification differs from that pass/fail table because basis conflicts and display refusals
    remain visible. Comparison JSON binds the canonical input-claims digest and disclosure-registry digest;
    the comparison does not alter the numerical-ledger states. The waterfall's addition advances the
    assurance frame to v8 below.
- `key::disclosure::VerifiedDisclosureRegistry` checks `restatement_note` and `restatement_scope` rows
  once per run. It verifies registered file identity, size and SHA-256, then matches the complete quote
  (modulo whitespace) at a visible non-formula source cell. Every exact subject/line/occurrence scope must
  bind to an observed canonical period and a unique unit. Coverage includes the entire reporting period;
  future notes never explain an earlier release. The `--through` cut drops later notes before verification.
  Missing sources, mismatched bytes and unusable quotes/scopes yield typed refusals and prohibit silent
  classification for uncovered changes. E461 is registered at Q4 2025 `QTD!A33`, covering 18 exact regional
  summary selectors for 2024–2025. Existing `region_member` rows remain the sole membership map.
- `key::inspect::{why_json, trace_json}` exposes actual Claim evidence, guards and source provenance for
  the CLI's show/why/trace views. Absent decision, calibration and clock assessments stay absent; unresolved
  parent IDs mark a partial trace. No upstream measurement history or arbitrary derived-Claim store is inferred.
- `aisc::{AiscClaimIndex, KeyingRefusal, key_aisc_operands}` (BD-332.S2): one keyed Claim per distinct arithmetic
  operand and per explicitly labelled co-product cash-cost or AISC rate, with the subject read from its own column
  (including regional member inputs), canonical
  metric labels, repeated-label occurrence indices and units `usd_m`, `koz` or `usd_per_oz`. An operand must
  still agree with its loaded source cell. Co-product rows retain their distinct line and `usd_per_oz` unit;
  they do not add numerical identity instances. Their row positions are discovered once per worksheet.
  Textual label checks remain context references; consulted Mine
  Stats cells use `from_mine_stats`. All-zero columns lacking a registered subject status are named in
  `unregistered_all_zero`; their zeros do not become evidence of withholding or disposal.
  - **Explicitly partial ownership wiring:** a row stays `Unprinted` unless its printed label states an
    attributable basis and the subject has a registered share; that supplied share cites both its registry
    evidence and the row locator. The missing row-basis registry is not invented. In particular, an
    arithmetic family's stated basis is not automatically assigned to every operand, and BD-332.S2's full
    row-basis obligation remains open.
- `consistency::{AiscConsistency, FamilyCounts, Appearance, IdentityHistory, Transition, TransitionKind,
  UnkeyedIdentity, aisc_consistency, clopper_pearson_95, FIRST_REGISTERED_CAVEAT}` and
  `PublicAssuranceReport::issuer_consistency`: the public JSON's `issuer_consistency` section has schema
  `fmine.issuer_consistency.aisc.v1`.
  - `within_vintage.first_registered` and `all_files` give counts by family and state `(i, ii, iii, iv, v, np)`.
    The denominator includes `np`; rates and equal-tailed 95% Clopper–Pearson intervals are descriptive.
    An empty family gives a null rate/interval; intervals also return null beyond the 100,000-trial budget.
  - Identities group by semantic total/rate key and family, so moved rows/columns and equivalent period names
    do not create new identities. Hidden sheets are excluded. An unkeyed total remains in `all_files` and is
    listed under `unkeyed`; multiple distinct assertions at the earliest registered release are retained in
    `ambiguous_vintages` and excluded from the first-registered counts. The history's `first_registered` and
    `total` fields are both null in that case, with every assertion still cited in `sequence`.
  - `transitions` keeps every appearance's release, file/cell, SHA-256 and state, and an adjacent `steps` list.
    `persisted` requires equal state, label, reason and complete arithmetic operand signatures; `corrected`
    requires `(iii) → (i or ii)` with changed operands; `broken` records `(i) → (iii)`. Other transitions,
    including ambiguous releases, are `reclassified`; an only appearance is `single_vintage`. Signatures
    include operand role, semantic key and displayed arithmetic value/token, so swapping inputs is not
    persistence and moving a cell is not a restatement. An unkeyed operand makes `operands_changed` null and
    cannot prove persistence or correction.
  - This is the earliest **registered** appearance, not necessarily the issuer's first publication. Value
    comparisons and registered-note classification are reported separately under `mine_stats_vintages`
    and `aisc_vintages`; a composite issuer-consistency gate result remains open.
- `consistency::mine_stats::{assess, MineStatsConsistency}` and
  `PublicAssuranceReport::mine_stats_consistency`: JSON under `issuer_consistency.mine_stats_identities`,
  schema `fmine.issuer_consistency.mine-stats.v1`, with first-registered and all-file family counts,
  cited adjacent histories, unkeyed instances and a digest of the complete projection. The existing
  assurance root binds the underlying ledger and source bytes; this derived report adds no new authority.
  Displayed arithmetic values and precision, operand roles, ownership shares and nil-token rules determine
  whether the operands changed. Hidden sheets are excluded; `np`, undecidable and refused rows stay in the
  all-file denominator. The synthetic source-to-report contracts in
  `crates/xtask/tests/public_mine_consistency.rs` exercise the actual XLSX reader and identity engine;
  they do not establish an independent census of the registered issuer corpus.

- `waterfall::{pair_sites, WaterfallReport}` (BD-333): the existing assurance payload includes same-quarter
  year-on-year AISC bridges, with first-published sources only. A missing first-release reconciliation is
  a named refusal; later comparative sheets never substitute for it. Copper, aggregate scopes, ambiguous
  sources, undeclared or conflicting ownership and affected (iii)/(v) operands are named explicitly.
  - `aisc::bridge_basis` checks the live gold-reconciliation title, attributable thousand-ounce label and
    same-release Mine Stats summary share. The net reconciliation has this denominator; its gross cost and
    NCI components retain their original Claims and are never all relabelled attributable.
  - Positive physical factors yield six LMDI bars. Unavailable or non-positive factors permit the exact
    signed cost-line/reciprocal-sales fallback only when the displayed component midpoints foot exactly.
    Its declared frontier is reported TCC as an aggregate plus signed block-2 sustaining adjustments;
    changing gross-cost row names never creates an alias or an assumed-zero component. Cost uncertainty
    sums the exact displayed half-steps of all components; the TCC aggregate sums its block-1 half-steps.
    These reduce to `k/2` and `k1/2` for integer displays. Negative amounts retain positive rounding radii,
    while literal placeholder zeros contribute no radius. This adapter arithmetic does not widen the AISC
    identity ledger's whole-display admission rule: an amount shown with decimals still produces state (v)
    and refuses the full-assurance bridge. The decimal precision regression exercises the adapter below that
    boundary and separately verifies the end-to-end refusal.
    Every bar carries its input citations, rounding enclosure and `sign_not_determined`; endpoint gaps and
    their change are separate exact quantities. Public bridges are descriptive and non-identifying.
  - Derived USD/oz endpoint fields use `waterfall.rate_reconciliation`; the measured residual uses
    `waterfall.closure` and cites the union of bar and rate inputs, both at `m0-waterfall/1`. The `WFLW` v1
    digest binds structured values, rule identifiers, Claim digests and source contexts. Refusal prose is
    excluded; changing a refusal code or interpretation changes the digest.
  - `pair_sites_between` selects any two distinct increasing calendar quarters on the same three-month
    support kind, including Q4 to Q1 and nonadjacent years. It reuses the original-release source selection,
    basis checks, identity-state guards and arithmetic kernel, including earlier originals needed to check
    cumulative identities. The full subject union is retained: a missing
    original endpoint produces a named refusal rather than a dropped subject or a borrowed comparative.
    Equal, reversed, invalid or nonquarter periods refuse; a selection with neither endpoint represented
    refuses explicitly. This optional projection has its own period-bound digest and does not change the
    automatic year-on-year assurance population or certificate. It remains descriptive, with no seasonal
    normalization or causal attribution.
  - DESIGN (BD-064/BD-333): `pair_sites_between_for_metric` accepts `WaterfallMetric::TotalCashCost`.
    It selects signed block-1 components, the TCC total and the printed TCC rate. Component uncertainty
    includes only those block-1 rounding radii; gross cost and negative ownership adjustments keep their
    original signs and source Claims. TCC checks its footing and per-ounce identity families alongside
    the unchanged physical-input guards, without borrowing sustaining adjustments or unrelated rate
    verdicts. Missing selected inputs and incompatible ownership refuse; an additive fallback also refuses
    changed component populations rather than inferring an absent component's value.
    The TCC `WFLW` v2 frame binds `metric:"tcc"` and `m0-tcc-waterfall/1`; JSON v2 names its endpoint
    fields `tcc_0`, `tcc_1` and `printed_tcc_*`. Existing AISC wrappers, v1 bytes and the automatic
    assurance population are unchanged. Whole-workbook hashes remain bound even when an edited
    sustaining cell does not change TCC arithmetic. Standalone TCC-only and copper layouts remain outside
    this adapter's accepted source grammar.
  - Cross-crate checks live in `xtask/tests/public_waterfall.rs`; the independent reader and high-precision
    known answers are in `repro/barrick/waterfall_census.py`. The existing synthetic CLI profile has no
    year-on-year source pair; the four-outcome synthetic bridge fixture is an integration test.

## 5. Refusals
- A run names every registered file that is missing, digest-mismatched or unparseable, and never substitutes
  content. The code is `FMINE-REF-505` (digest mismatch, exit 8) when every file is present and readable, and
  `FMINE-REF-506` (registered input missing or unreadable, exit 5) otherwise (`files_refusal`).
- `FMINE-REF-506` also refuses a missing or malformed registry, naming the file and line, and an unregistered
  Mine Stats quarter.
- `FMINE-REF-201` (no claim in domain, exit 3): a `through` bound that leaves no rows, or a workbook loader
  called on a profile without registered workbooks. Document-only profiles remain usable by document consumers.
- `FMINE-REF-208` (source cell unusable, exit 3) from `cell_to_public_claim` on text or formula-error cells.
- 505, 506 and 208 are proposed codes (`docs/adr/ADR-REFUSALS-01.md`).
- The ledger scores displayed values (bd-jg43.3, E838): the declared construction sums the numbers as printed,
  so a numeric cell enters as `fmine-io`'s displayed value under its number format, never as the stored double.
  A whole displayed number (d = 0) enters as itself and a zero section's token (the dash) as 0. An operand
  with no whole displayed number is still a printed number, and its instance is refused (`RefusedAt`), naming
  the cell and the format code, for a General non-integer, a format with decimals or a percentage shown, or a
  format the interpreter refuses (a date, a scaling comma, a condition, an unknown id).
- Instance-level refusals (state (v)) carry a reason:
  - an operand with no whole displayed number (above);
  - a non-numeric line;
  - one of rate and ounces being zero.
- Keying refusals are separate from numerical identity states. `KeyRefusal` distinguishes a sheet period,
  printed share, unusable cell token, source binding or explicit unit that could not be read; the projection
  names the failing locator and reason. A refused Claim is never supplied with an inferred period, unit,
  ownership share or numeric replacement. The ledger's already computed state remains available.
- Cost of sales per ounce in a column with a non-zero operating non-controlling-interest line before TCC
  (BD-331.S3, E860) is checked at the
  subject's registered share, s: 1000·s·N/Q on exact corners. A fit is (i), and its `basis` cites the share, the
  footnote and the label. A miss is (iv), giving the feasible share interval, rounded outward, and the missing
  fact. The instance is (v) instead when:
  - the sheet has no attributable-basis footnote, found by text;
  - the subject is a registered region with no members registered for the release ("regional numerator needs
    member weighting");
  - the subject has no registered share;
  - the same-release Mine Stats quarter-to-date label is missing, repeated or prints another share
    (`RefusedAt`, naming the cell);
  - an operand is not positive.
- Such a rate of a region with members registered for the AISC file's release (BD-331.S4), on a sheet with the
  footnote, is checked against its members, weighted by attributable ounces sold. A member printed on the sheet
  is weighed when its rate and ounces are whole displayed numbers with ounces not zero, and left out by name when
  both are zero (a displayed 0 or a zero-section token) or both placeholders. A registered member the sheet does
  not print is named as absent. The weighed members' ounces must sum to the region's (component sum). Then
  `exact_feas::region_against_members` poses the weighted mean with no numerator (the table prints consolidated
  cost of sales, not the attributable numerator). Its McCormick relaxation either refutes the rate with a
  checked certificate (iv, `region_outside_members`, carrying the weighting and the certificate digest, with
  `REGION_MISSING_FACT`), or does not. Then the exact decision (`mine_ledger::weighted_mean_holds`, the
  members' ounces free of the region's box as under the component sum) gives (i) with
  `ConsistentBasis::MembersExact` when weights and rates in their boxes reach the rate, the witness checked
  exactly; (iv) `region_outside_members` with the certificate `EXACT_REGION_REFUTATION` when none do (a
  safety net: no synthetic case was found where the relaxation misses a refutation); and (i) with
  `ConsistentBasis::Members`, `relaxation_only`, only when it gives no answer (overflow or a failed witness check).
  The rounded-once alternative is reported, never scored. The instance is (v) instead, naming a cell where it
  has one, when:
  - a member's rate or ounces is anything else, or its ounces are negative (`region_member_operand`, at the
    rate's cell when the rate is not whole, else the ounces'), or a member heads two columns (at the second
    header);
  - no member is weighed (`region_no_members`);
  - the members' ounces do not sum to the region's (`region_ounces_do_not_foot`, at the region's ounces).
- A stated (ii) whose quote cell no longer starts with the registered prefix is refused as an explanation: the
  instance stays (iv), naming the quote cell and what it reads.

## 6. Determinism class
Pure report assembly, `M0AS` v8 state-root encoding, `PCLM` v1 keyed-claim encoding, and exact rational identity checks are deterministic and byte-identical on reruns over identical registered inputs (`INV-010`).

## 7. Evidence class of outputs
Every retained numeric issuer assertion is `Assertion { source: Issuer }` (`Known` / `KnownKnown`); derived checks remain `Assertion` (`NL-2`). Placeholders and reserved synthetic issuer inputs remain `Assumed` / `Unknown` (`synthetic`), never acquiring issuer authority.

## 8. Validity domain
- A sheet with a cost-of-sales row but no locatable structure becomes a `TableRefusal`; an unregistered header becomes an `UnregisteredHeader`; neither is guessed.
- Depends on `fmine-core`, `fmine-units`, `fmine-time`, `fmine-evidence`, `fmine-claim`, `fmine-registry`, `fmine-numerics`, `fmine-calib`, `fmine-io`, and `fmine-metrics`.

## 9. Oracles
- `crates/xtask/tests/public_exact_feas_oracle.rs` (closed-form rate/footing oracle), `public_exact_feas_blame.rs`, `public_laap_2023.rs`, `public_registry.rs`, `public_mine_stats_inventory.rs`, `public_mine_stats_vintages.rs`, `public_transcriptions.rs`, `public_mine_ledger.rs`, `public_report_contract.rs`, `public_citations.rs`, `public_track_record.rs`.
- `GATE-BULL-01` (parts 1–3a/3b/3c): Mine Stats population reproduces T16 and all nine known changes with 30 region-map exclusions citing E461.
- E866–E867 independently match all registered cross-vintage workbook pairs and dispositions through 2026Q3.

## 10. Known gaps
- DESIGN (BD-504e): original-quarter gold cost/sold figures now receive a separate same-release workbook
  comparison under `workbook_comparisons`. Canonical AISC and Mine Stats source Claims retain both locators,
  raw values and ownership evidence. Exact equality, closed rounding overlap, differences and unavailable
  counterparts are distinct; missing or duplicate assertions never choose a fallback. An unprinted
  reconciliation basis may use the existing gold-table bridge with its title, attributable denominator and
  same-release share label retained as hash-bound citations. Sold-ounce comparisons also require the
  Mine Stats header to state attributable share. Cost-of-sales rates require the explicit attributable
  numerator footnote, cited with its workbook hash. Unproven bases stay unavailable. Before emitting either
  report format, the assurance gate matches comparison inputs to canonical Claims and original-quarter keys,
  verifies the cited ownership text against the loaded workbooks, and recomputes the displayed differences.
  Inconsistent fields name their paths in `FMINE-REF-208`. The projection has its own digest. The authentic
  T18 census and complete gate passage remain open.
- Complete `GATE-BULL-01` issuer-consistency delivery record and prospective Q4 blind-layout run remain open.
- Full all-verb JSON uncited-number validator (`BD-329`) and two-way cluster inference (`BD-341.S4`) remain open.
- See scope notes below under Public API / Proof obligations.

## 11. Public API
- `run_public_assurance`, `run_synthetic_assurance`, `PublicAssuranceReport`, `LoadedSource`, `cell_to_public_claim`
- `sources`, `aisc`, `mine_stats`, `mine_ledger`, `synthetic`, `live`, `citation`, `key`, `consistency`, `transcribe`, `forecast`, `guidance`, `waterfall`, `recovery`, `lint`, `html`.

## 12. Proof obligations
### Proof obligations and controls (`PO-*`)
- `PO-PUB-1` (real files through 2026Q2, ignored test; recorded in MOR_CASE E820–E823): the census and digest of
  every registered workbook equal an independent reader's. The test also checks the run against the last row of
  `repro/barrick/expected.toml` (root, anchor, totals, registry epoch, digest of digests). The root is a
  regression value, not independent; the last two can be recomputed without this code (E829). Rows are
  append-only, and a default-CI test checks each row's form and that its anchor derives from its root.
  - **Ledger:** 34 visible AISC tables, 2,595 instances. Footing: 1,010 (i), 4 (iii), 24 not published.
    Per ounce, after the region rule (E840): 1,426 (i), 3 (ii), 92 (iv), 36 not published, none (v); 41 of the
    (i) are regions, each proved by the exact decision since E845 (before it, `relaxation_only`), none
    `relaxation_only`. Before the stated share rule (E826) the counts were 1,116 (i), 17 (iv) and 385
    (v); E838 (displayed values) and E840 took the rest out of (v).
  - **Alternative construction:** 1,014 footing and 1,304 rate instances consistent.
  - **Identities:** each (ii) and (iii) instance is identified.
- `PO-PUB-2` (synthetic): every seeded answer is reproduced, including the sign-flip witness. Perturbing one cell
  moves the state root and the affected state. A wrong component label leaves the instance in (iv). Column G's
  stored 100.4 under `#,##0` is scored as the displayed 100, and column N's General non-integer and date-format
  ounces are refused by cell and format code (bd-jg43.3).
- `PO-PUB-3`: a workspace without the files refuses, naming every registered file; the count follows the registry
  rows (a row added to a copy of the TOML changes it with no code edit).
- `PO-PUB-4` (Mine Stats reader, MOR_CASE E825, E847):
  - **Real files** (ignored test through 2026Q2): all ten reads equal W1's openpyxl read, digest for digest
    (22,732 cells, no refusals). Q1 and Q2 2026 hold 3,996 cells, and the seven numeric zeros sit at their
    locators with stored text `0`. The W1 §4 M0 census is 820 cells. Given the oracle's per-file lines
    (`FMINE_MINE_STATS_ORACLE`), the test logs one JSON line per differing cell with both reads and the
    differing fields, never the token (0 on the ten files). Mutation: a reader without footnote stripping
    (436 labels changed) and one fixed to Q2 2025's coordinates (1,683 of 2,567 cells misread, 17 refused)
    both miss the Q2 2026 digest.
  - **Synthetic** (default suite): an inserted site, an inserted row, renumbered footnotes and a misspelled
    month give the same values. Each refusal names its cell: an unknown token, a repeated label, an unknown
    period and a formula error. The mismatch log names the cell and the differing field, and prints no token.
    A block header misspelling its month ("Decemer", as in `Q4_2025:Minestats1!A64`) is read by its period word.
- `PO-PUB-5` (stated share rule, BD-331.S3, MOR_CASE E826):
  - **Real files** (ignored test `barrick_share_rule_matches_census`): of the 379 instances refused before, the 328
    site instances split into 263 (i) and 65 (iv). The 51 region instances go to the region rule (PO-PUB-10).
    The counts and per-subject fit/miss equal the independent census. On every instance, Rust also equals it on
    state, subject, footnote cell and label cell; that check was run outside the test.
  - **Synthetic:** K14 fits at 3/4, and L14 misses with the interval [0.8210, 0.8290]. Further tests cover a
    mismatched or missing label, a moved footnote letter and a removed footnote, a region, and a dropped share.
- `PO-PUB-6` (state-type completion and hidden scope, BD-331.S2, MOR_CASE E828):
  - **Real files** (ignored tests `barrick_hidden_scope_matches_census`, `barrick_every_iv_names_missing_fact`):
    6 hidden tables, 435 instances. Footing: 169 (i), 1 (iii, North America Q4 2023 TCC, 541 against 540), 4 not
    published. Per ounce: 237 (i), 18 (iv), 6 not published. The counts equal the hidden census, whose three
    regional (v) the region census decides (PO-PUB-10); on every instance, Rust also equals the hidden census, a
    check run outside the test. Each of the 92 visible and 18 hidden (iv) names a missing fact. E816's three (ii)
    are reconstructed.
  - **Synthetic:** M16 is a stated (ii), and a changed or empty quote keeps it (iv). The hidden `Q4 2029` table's
    (iii) is reported and never counted. A changed hidden cell moves the root and no visible state. Hidden sheets
    leave the census and its digest unchanged.
- `PO-PUB-8` (live dependency, files; BD-338.S1; default suite,
  `xtask/tests/public_report_contract.rs::tests::live_dependency`):
  - **Positive control:** on the three-file synthetic registry, 3 files × 3 cases give 9 checks, and every
    one refuses as required.
  - **Mutants:**
    - M3, a loader that restores the intact bytes on refusal, fails each file with "file deletion did not
      refuse: <file>".
    - M4, re-raising under the wrong code (`FMINE-REF-501`, exit 8), fails a deleted file with "refusal exit 8
      is not 5" and the other cases with "refusal code FMINE-REF-501 is not FMINE-REF-505".
  - **Unreadable file:** a chmod 000 file is named under "missing" (`FMINE-REF-506`, exit 5). As root, the test
    says it skipped.
  - **Lane:** the synthetic registry never reports the public lane or gate eligibility.
- `PO-PUB-9` (live dependency, leaves; BD-338.S2; default suite,
  `xtask/tests/public_report_contract.rs::tests::live_dependency`):
  - **Positive control:** every leaf of the 50 synthetic instances passes or is skipped with a reason. Skips
    fall only on Site F's unrounded column and on E14's three non-cause leaves (hand-read from
    `synthetic.rs`).
  - **Mutants:**
    - M1, C8's certificate keeping the literal 350, is named "value did not move";
    - M2, B8 citing the unused P5, is named "dead ancestor".
  - **Flip margins (hand-derived):**
    - (79, 1, 68) flips at 1145 and 1179;
    - B8 401 gives residual 1;
    - E7 → 0 evaluates E14 as (i) and E8 as (iii);
    - F5 → 1 ends the all-zero column;
    - a broken `Minestats3!B2` makes H16 (iv) naming the cell.
  - **Edits:** a byte-level edit and a parsed-level edit give the same census and instances.
- `PO-PUB-7` (K.4 one-cell check, bd-prmn.8, MOR_CASE E836):
  - **Real files** (ignored test `barrick_one_cell_matches_independent_reader`): 519 columns give 374 pass,
    82 fail and 63 refused. On every column, every field equals `scripts/oracle/one_cell_cos.py` (same origin),
    including the summary. The failing columns are exactly the 82 cost-of-sales-per-ounce (iv) other than the ten
    regions refuted by their members (PO-PUB-10), which the check leaves unbased. Of the 82, 17 are K.4 items and
    65 are not, so the verdict is "basis rule". The root does not move.
  - **Synthetic:** 10 columns, and Site I fails at both bases with known implied ounces. A share registered
    wrong for Site H (3/5, labelled 60%) fails at that basis and gives "basis rule".
- `PO-PUB-10` (region against members, BD-331.S4, MOR_CASE E840, E845):
  - **Real files** (ignored test `barrick_regions_match_region_census`): the 51 visible and 3 hidden regional
    rates the stated-share census leaves to member weighting are decided: visible 41 consistent (i,
    `MembersExact`, each with a witness checked exactly) and 10 refuted (iv, each by a McCormick certificate);
    hidden 2 and 1. None is `relaxation_only` (the test pins 41 and 2 exact, 0 relaxation-only). Each instance's
    verdict, reason, cell and used, absent and excluded members equal `scripts/oracle/region_census.py`
    (57a5aa5, an exact vertex check written before the Rust and without reading it), run in the test. The 41 are
    also consistent under the rounded-once alternative (the kernel's relaxation; no independent reader computes
    it).
  - **Synthetic** (default suite): Region North over Site H and Site I is (i), exactly, at 2099, 2100 and 2101,
    and refuted at 2098, 2102, 1900 and 2300, where the exact bound is [2099⅙, 2100⅚]. Over Site H at 1000 and
    Site I at 2000 on one ounce each the bound is [1249½, 1750½]: 1249 and 1751 touch it and are (i), exactly;
    1248 and 1752 are refuted. Every refusal and exclusion names its cell or member; a footnote-less sheet and a
    release before the members' first keep the earlier refusals.
- `PO-PUB-11` (Mine Stats ownership, quarter sums on both sheets, summary against site sheet, the sum families
  and the sold-weighted rate, BD-331.S4, bd-prmn.4, MOR_CASE E841, E842, E843, E844, E846):
  - **Real files** (ignored test `barrick_mine_ledger_matches_census`): 10 releases, 9,000 instances. Ownership
    592: 470 (i), 29 (iii), 73 (v), 20 np; quarter sums 942: 883 (i), 9 (iii), 26 (v), 24 np; site sheet 3,130:
    2,982 (i), 1 (iii), 42 (v: 40 withheld, 2 blank), 105 np; sites to region 306: 282 (i, 41 by the nil token),
    24 (v: withheld, E818); region to total 204: 204 (i); sold-weighted rate 306: 252 (i), 30 (iv), 24 (v:
    withheld, E818), 45 of them with a nil-sold member; site-sheet quarter sums 3,520: 2,742 (i), 8 (iii), 87
    (v: 41 withheld, 43 missing row, 3 placeholder), 683 np, 61 decided by the nil token. 2,091 components, 27
    infeasible (2 only jointly), every one with all its minimum correction sets. Every instance line, component
    line, summary and implied range equals `scripts/oracle/mine_stats_census.py` (e66f7dc; family 7 a736c26;
    families 3–4 c697b13; family 5 ac07fee; site-sheet quarter sums 9854b6c, each written first by a separate
    reader, exact Fourier–Motzkin, exact sums over fractions or a
    separable exact test of the weighted mean), run in the test, which compares a sum's or a rate's member cells
    as a set (the census orders a map's members by its rule, the ledger by row) and pins the sum and rate
    families' counts. The plan's known answers hold:
    E113, E114 and E115 are each a pair of (iii) on one component whose one minimum set is the shared cell,
    E113's B5 can take 281, and E116 is one (iii) site-sheet instance decided by the nil token, alone.
  - **Synthetic** (default suite): the E113 shape (one shared cell, the one minimum set); a component
    infeasible only jointly, with its three minimum sets and ranges derived by hand ([201/4, 201/4] twice,
    [202, 203]); an unprinted share refused where 100% would hold; a registered share with the printed one
    beside; `withheld` and region-map exclusions; placeholders, blanks and a repeated row; the same stored text
    decided by its displayed format; row and file order changing no state; a sign flip, a dropped share and a
    missing release moving the state. Family 7: a print pairing by label as matched, (iii) posed jointly with
    both prints as its sets; the nil token as zero ((iii) at 5, (i) at 0) and `n/a`; another nil spelling
    unreadable; the refusal order (two sheets, withheld, missing row, blank, unreadable); the W1-DR-02 alias by
    release, and beside its registered row; a label printed twice; a site print making an ownership component
    infeasible only jointly, its two minimum sets and ranges ([399/4, 401/4], [201, 201]) derived by hand. Sum
    families: the slack bound on both sides (148–152 against 150 with slack 1.5; Total Copper 54–60 against 57
    with slack 2, the bound included), a region discrepancy that neither reaches the total nor forms a
    component; the nil token as exactly 0, all nil (np), `n/a` and a blank; a member found by its registered
    label, missing without it, and refused at the first row under both names in either order; a region outside
    the map, a missing member and a map region with no row; a withheld member by period; the closed form
    against the kernel on 400 pseudo-random sums of mixed steps (both outcomes at least 50 times). Sold-weighted
    rate: a region rate held between its weighted-mean bounds and (iv) on either side, cells in row order when the
    registry lists members in reverse; a nil-sold member dropped, all nil (`weights_all_nil`), a rate placeholder
    refused, a zero or negative weight; refusals by name, missing row and both names; the exact decision against a
    separable test written apart from it on 600 pseudo-random cases (both outcomes at least 50 times), and the
    kernel's McCormick relaxation, which refutes at least 50 of them and never a feasible one. Site-sheet quarter
    sums: H1 and 9M across three releases on the additive rows only (`Underground` posed under tonnes mined, not
    under a unit cost; no strip ratio), cells as [previous, quarter, total], a (iii) past the slack and 35 against
    36 inside it; a row the Q3 release does not print; the nil token as exactly 0, held and past the slack, all
    nil (np), `n/a` and a blank at their cells; a site withheld for January–March refused for H1 by its Q1
    operand, its prior year still decided; a site that moved sheets matched; a missing previous release; a site
    on two sheets of its metal; a row the previous release does not print.
- `PO-PUB-12` (construction discovery, bd-4wvl.4, MOR_CASE E848):
  - **Real files** (ignored test `test_construction_discovery_real_aisc_tables`): `ComponentSum` in all four
    family and scope pairs. Visible: TCC 505 of 507 instances (370 of 371 distinct), AISC 505 of 507 (306 of
    307). Hidden: TCC 84 of 85 (57 of 58), AISC 85 of 85 (58 of 58).
  - The five (iii) are E815's cells: Kibali `'Q2 2024'!O11` (sign-flip witness O7) and Porgera L18, each in
    the Q3 2024 and Q2 2025 files, and North America `'Q4 2023'!J11`, hidden only. On every decided footing
    the numeric lines sum to the ledger's component sum.
  - **Default suite:** on the synthetic ledger every pair is `Ambiguous` (FMINE-REF-104), and the counts are
    the (i) and (iii) footings. A doubled instance list leaves the report unchanged; the same numbers under
    another subject double the distinct count.
  - **Unit** (ignored test `test_e463_copper_unit_real_files`, MOR_CASE E849): 112 pairs, kt fits all 112 and
    Mlb none, so kt is identified. The synthetic Mine Stats print no row in pounds: 0 pairs, no fit.
  - **Share** (`aisc::tests::test_feasible_share_matches_kernel_share_interval`): `feasible_share` equals the
    kernel's `share_interval` on all 15,800 grid points with 2n − k ≥ 1.
- `PO-PUB-13` (transcriptions, bd-plx1; synthetic documents and rows, `crates/xtask/tests/public_transcriptions.rs`):
  - E698's shape signed as printed ((3.6) and 1.7 against (3.7) and 1.8, step 0.1): 3 of 16 assignments fit
    properly, the `Closed` check fits at points only, the deck's convention is `truncate` and the MD&A's is any
    of `half_up`, `half_down`, `half_even` (FMINE-REF-104). The section names no transcribed value, and the
    state root is unchanged without the registry.
  - E692's shape (6.7 against 79): a conflict naming both locators; the re-run without it finds the same deck
    convention.
  - Refusals: a reader's planted different token (`FMINE-REF-202`, exit 3; the locator and `token` named, no
    value); one altered byte of a cited document (`FMINE-REF-505`, exit 8); a missing cited document
    (`FMINE-REF-506`, exit 5). A `--through` run reads no
    transcriptions. A lone reader's locator is pending, not refused.
- `PO-PUB-14` (keyed public Claims, BD-332.S2/S3): cross-crate contracts live in the T layer at
  `crates/xtask/tests/public_key_contract.rs`, `public_keys_aisc.rs` and `public_keys_mine_stats.rs`.
  - **Default suite:** every synthetic arithmetic operand is keyed or refused by name; stored and displayed
    numbers and unsupported-format refusal codes survive; registered statuses alone turn zeros into Nil,
    changing the Claim id when the interpretation changes. A non-zero disposed/withheld figure is retained
    with its flag. Source/release rebinding is refused. Printed shares and unit aliases remain beside their
    evidenced corrections, without numerical conversion. An explicit percent format's literal zero section
    does not change a stored fraction's unit. Unknown AISC row bases remain unprinted. Shifted rows/columns,
    repeated labels and reordered files preserve semantic series and distinct vintage Claim ids.
  - **Real files** (ignored tests `barrick_aisc_claims_known_answers`, `barrick_mine_stats_claims_known_answers`):
    the seven 2026 Hemlo/Tongon numeric-zero cells become disposed Nil; Loulo-Gounkoto's registered withheld
    zeros become withheld Nil; `n/a` and the em dash retain distinct types. The known Q1 2024 Total Gold AISC
    comparative shares its series across the 2024Q1 and 2025Q1 releases with distinct Claim ids. Porgera's
    printed 47.5% and corrected 24.5% remain separately visible. Pueblo Viejo's moved AISC column and Kibali's
    repeated total preserve their series. These are keyed projection checks; the existing ledger-root
    regression remains unchanged.
- `PO-PUB-15` (AISC issuer consistency, bd-504e.1, E861 and T-layer `crates/xtask/tests/public_consistency.rs`):
  - **Real files** (ignored `test_part3_real_aisc_dedup_and_persisted`): 2,595 all-file instances become 1,565
    first-registered identities, 313 per family, with no unkeyed totals or ambiguous releases. The independent
    aggregation in `repro/barrick/first_vintage_aisc_census.py` uses `census2.py` on displayed values plus
    `share_census.py` and `region_census.py`, grouping
    normalized periods, subjects and families by earliest registry quarter before reading the consistency
    implementation; adding region-map scope gives the same counts. First-registered states are:

    | Family | i | ii | iii | iv | v | np |
    | --- | ---: | ---: | ---: | ---: | ---: | ---: |
    | TCC footing | 306 | 0 | 1 | 0 | 0 | 6 |
    | AISC footing | 306 | 0 | 1 | 0 | 0 | 6 |
    | Cost of sales per ounce | 250 | 0 | 0 | 57 | 0 | 6 |
    | TCC per ounce | 307 | 0 | 0 | 0 | 0 | 6 |
    | AISC per ounce | 305 | 2 | 0 | 0 | 0 | 6 |

    All-file counts remain 2,436 (i), 3 (ii), 4 (iii), 92 (iv), 0 (v), 60 np. Kibali TCC
    `'Q2 2024'!O11` and Porgera AISC `'Q2 2024'!L18` both retain their `(iii) → (iii)` sequence from the
    2024Q3 to the 2025Q2 registered release, with both source digests and locators.
  - **Default suite:** equivalent sheet-period forms, earliest rather than latest selection, hidden-sheet
    exclusion, missing keys, ambiguous earliest releases and reversed input order. Seeded adjacent histories
    distinguish persistence, correction, break and reclassification, including a correction followed by a
    later break. Binomial intervals match independent known answers and their edge cases; empty/invalid and
    over-budget samples return no interval. No pass threshold is inferred from these counts.

- FACT (BD-342, `guidance::tests`): synthetic sources exercise the complete registry-to-report path, exact
  production and sales intervals, latest eligible vintage, exclusion of future guidance, missing/incomplete
  pairs, pending newer evidence, incompatible ownership and definition, altered documents, unusable source
  support and metadata preservation. Real values remain external inputs, not embedded source constants.
- `PO-PUB-RECOVERY` (bd-srqc): the public adapter must preserve the independent historical fit and all
  selected quarterly point answers, exact rounding boxes, source locators, source hashes and separate
  ownership evidence. Synthetic and registered-input tests cover missing/disagreeing historical readings,
  altered source bytes, missing ownership context, unusable first appearances, explicit comparative
  selection, input mutation changing the result/digest, deterministic reruns and uncited-number checks.
  Historical point answers are checked against the independent source-only oracle; real training rows are
  supplied through local inputs rather than embedded test fixtures.
- FACT (BD-342, `xtask/tests/public_guidance_history.rs`): six integration regressions cover the positive
  synthetic T1 path, rolling and cross-year windows, original-vintage-only selection, missing history, guidance-year
  cutoffs, incompatible source Claims and a separately restricted sales scenario. Every emitted source-quarter
  witness is checked against its original source intervals; missing history cannot produce a positive verdict.
- FACT (BD-342, `xtask/tests/public_guidance_capacity.rs`): eight end-to-end regressions exercise the complete
  registered synthetic workbook/PDF/transcription path, exact ownership and percent conversion, conditional
  plan/nameplate rounding, latest-vintage and as-of selection, altered or missing documents, reader conflicts
  (including conflicting quantity keys), inapplicable support and preservation of T1 when technical inputs
  fail. Independent rational answers pin the scenario ceiling and optimistic required-grade endpoints.
- FACT (BD-341, `xtask/tests/public_track_record.rs`): source-selector controls cover original versus later
  outcomes, comparative-only history, missing members, stale and incompatible Claims, retained revisions,
  normalized equivalent shares, ambiguous earliest guidance, duplicate outcomes, mutated bindings,
  unregistered digests, pending readings, late guidance and future outcomes. A synthetic on-disk registry
  verifies real workbook/document loading and two-reader agreement, retains a placeholder as unresolved, and
  refuses an altered cited document. Exact hand-derived scores and completion bounds, deterministic input
  order, site filtering and source-bearing JSON/Markdown are exercised through the public adapter.

### Scope boundary, dependencies and failure modes
- A sheet with a cost-of-sales row but no locatable structure becomes a `TableRefusal`.
- A header that names no registered subject becomes an `UnregisteredHeader`.
- Neither stops the run, and neither is ever guessed.
- `GATE-BULL-01`: parts 1 and 2 retain their existing numerical ledger. Part 3a reports AISC and Mine Stats
  first-registered within-vintage counts, and part 3c reports adjacent histories for both. Part 3b compares keyed AISC and Mine Stats
  values with verified registered-note coverage. The Mine Stats population reproduces T16 and all nine
  known changes, with 30 region-map exclusions citing E461. A complete issuer-consistency gate record remains
  outstanding; these projections alone do not establish a full M0 gate pass.
- Depends on `fmine-core`, `fmine-evidence`, `fmine-claim`, `fmine-io`, `fmine-metrics`, `fmine-numerics`, `fmine-calib`, `fmine-registry`, `fmine-units`, `fmine-time`.

### Gold and copper cost guidance

- DESIGN (BD-342): explicit `CostMetric::Tcc` selects gold guidance assertions named
  `guidance_<site>_tcc_usd_per_oz` with `;tcc=issuer-reported-per-sold-oz`, and the matching
  `summary:tcc_usd_per_oz` YTD and original-quarter observations. Missing or incompatible TCC
  evidence cannot fall back to AISC. Exact signed requirements and historical witnesses retain
  source hashes, ownership, sold-ounce units and separate TCC Claim identities. TCC reports use
  `fmine.guidance.completion.v3` and TCC-labelled fields; AISC/C1 report versions are unchanged.
  Registered-source controls use invented evidence. Authentic issuer transcription and the full
  BD-342 acceptance population remain separate work.
- DESIGN (BD-342): `guidance::run_public_with_metric` explicitly selects `CostMetric::Aisc` or copper
  `CostMetric::C1`. C1 requires agreed `guidance_<site>_c1_usd_per_lb` assertions with
  `;c1=issuer-reported-per-sold-lb`, the same document's copper-production target, and original-vintage
  `summary:c1_usd_per_lb` actuals/history. AISC cannot fill any missing C1 observation. Exact signed
  C1 spending and rate requirements retain their own typed Claims, source hashes, sales assumptions,
  historical witnesses and refusals. Optional technical scenarios still constrain production only.
  The C1 report uses `fmine.guidance.completion.v2` with `cost_metric: "c1"` and C1-labelled fields;
  existing AISC reports keep v1. Synthetic registered-source controls in `public_guidance_copper.rs`
  exercise both metrics in the same sources, rounding, signed credits, incompatible evidence and CLI
  selection. Authentic issuer C1 transcription, calibration and full BD-342 acceptance remain separate.
- DESIGN (BD-341.S4): `guidance::track_record::clusters` wraps the unchanged dated comparison in
  `fmine.guidance.track-record.clusters.v1` / `GTCL` v1. Its digest binds the entire original report,
  both first-published and latest-registered selectors, every guidance/source link, both exact cluster
  partitions and the explicitly selected bootstrap axis. Earliest-guidance ties remain one unresolved
  site-year, with all competing readings retained; a cached numerical outcome cannot override its bound
  score. Compatible metric/unit/basis families stay separate. Missing originals never acquire a later
  outcome in their place. The kernel's independent-cluster assumption is unverified, its equal-cluster
  estimand differs from the existing pooled mean, and it supplies no two-way robustness, automatic
  effective sample size, calibration or site ranking. Legacy report bytes and digests are unchanged.
- DESIGN (BD-329.S2, bounded runtime citation integrity): both public and synthetic assurance entry points
  validate canonical source Claims and the AISC ledger's emitted source operands and registered derivations
  before returning a report. The check includes rendered hidden-sheet exceptions. It binds each footing
  discrepancy's printed total, component sum, residual, cell list and sign-flip witness to the calculation
  and its operands. A mismatch produces `FMINE-REF-208` with the affected report path. Hidden-sheet lookup
  applies only to hidden exceptions. The full all-verb JSON uncited-number validator remains open.
- DESIGN (BD-331.S8): the cross-vintage ledger projects each later source appearance against its earliest
  registered release into a separate numeric denominator. Unchanged numbers are state i; verified stated
  and reconstructed silent restatements are distinct state ii explanations; source assertion conflicts
  are state iii; changes without complete disclosure coverage are state iv. Token-only and non-comparable
  cases are named exclusions. Both source Claim ids, locators and hashes survive even for unchanged
  comparisons; ambiguous releases retain all assertions without a minimal-set claim. Existing within-vintage
  counts, first/single appearances and source-value classifications are unchanged. This is accounting of
  registered assertions, not an arithmetic conflict certificate or a choice of the true vintage. The independent
  census in E866–E867 matched 7,670 Mine Stats and 4,498 AISC pairs in the registered through-2026Q3 corpus.
- DESIGN (BD-340.S5): `transcribe::check_docs` exposes the existing byte/digest verification for explicitly
  declared PDF tables. A consumer can require a document even when it has no transcription rows, so missing
  evidence cannot hide a missing or changed file. The consumer still owns identity definitions and arithmetic.
- Operates only on public disclosures (`Confidentiality::public()`).
- Makes no reserve, resource, geotechnical, regulatory or investment claim.
- Implies no issuer affiliation.
- Guidance uses only the registered evidence population, not an assertion that no later issuer guidance
  exists. Historical sales persistence is an assumption, not a production/sales equality or a capacity bound.
  The T1 result excludes or admits only supplied historical windows under that assumption. T2 compares
  production only under its cited technical scenario; it neither proves physical impossibility nor supplies
  a joint production/cost witness. An issuer's T2 result requires actual applicable registered evidence, and
  no historical exclusion establishes that a mine cannot meet guidance.
  The separately selected explicit-limit interface lives in `fmine-cli::guidance_capacity` and delegates to
  the same exact production-ceiling kernel; it retains its distinct input schema and report.
- The ordinary guidance track record does not establish a complete historical population, authentic prior
  forecast registration, calibrated coverage, independence, effective sample size, a bias sampling interval or
  a site ranking. Its separate `clusters` projection supplies nominal 90% one-axis sampling intervals under
  explicit unverified assumptions; simultaneous site/year inference and empirical qualification remain open.
  Ordinary cost scores remain unconditional, with separate price sensitivities rather than attribution to
  controllable operations. These report slices do not close the full BD-341 data gate.
- Recovery comparison is descriptive and non-identifying. Ore type, grind, throughput, concentrate mass
  pull and annual-to-quarter support remain explicit model limitations. Signed differences do not establish
  causation, recoverable production, investment value or forecasting skill. This slice emits no dollar
  sensitivity or annualized economic estimate, and makes no first-publication claim for comparative inputs.

### Quarterly capital expenditure forecast records

DESIGN (BD-059/067/076): `forecast` admits `capex_usd_m`, `capex_sustaining_usd_m`
and `capex_projects_usd_m` only for explicit quarterly/quarterly-nowcast rules, each in `usd_m`.
The source owner matches complete canonical Mine Stats line identities for reported total,
minesite sustaining and project expenditures, respectively. It preserves every raw competitor
before admission, original quarter, display token, cell, hash and printed ownership basis.
Summary aliases, cumulative amounts and other capital scopes cannot supply these targets.

Signed and zero unflagged numeric persistence points are permitted; training requires positive
adjacent quarter pairs with missing/nonpositive slots retained. Changing ownership among eligible
history and point values refuses candidate generation. No capital values are scaled to a different
ownership basis. Frozen-record scoring and comparators retain the same scope and basis requirements.
Status-derived zeros cannot become numeric capex observations. Existing six production/cost targets
retain their prior receipts and admission rules.

### JV mirror pair registry (`BD-346.S2`, `bd-xzvb.2`)

DESIGN (`mirror`, `BitwisePortable`): code-constant dual-reporter pair registry for NGM (`615/1000` Barrick /
`385/1000` Newmont, `E457`, plus dated 2026-08-09 Fourmile + Mike/Fiberline vend-in entry `E658`/`E558` with
unresolved effective date and shares), Pueblo Viejo (`3/5` Barrick / `2/5` Newmont, `E672`; Q4 2025 North
America region move `E461` is strictly a reporting-scope change via `barrick_pv_reporting_region`, never a
perimeter change), and Kibali (`9/20` Barrick / `9/20` AngloGold Ashanti + non-reporter SOKIMO `1/10` = `1`,
`E560`). `resolve_pair` refuses straddling periods, unresolved post-vend-in parameters, and `Unprinted` /
`Consolidated100` JV shares with `FMINE-REF-104`, naming the missing parameter. `check_pair_registry` validates
non-overlapping chronological perimeters and exact share sums `<= 1`, pinning the canonical `MPR1` SHA-256
digest (`PAIR_REGISTRY_GOLDEN_SHA256`).
