# CONTRACT.md — `fmine-cli`

## 1. Charter & Layer
- **Crate:** `fmine-cli`
- **Layer:** `U` (User Surfaces & CLI)
- **Subsystem:** `Surfaces`
- **LOC Budget:** `26k planned / 30k ceiling` (`registries/loc_budget.toml`)
- **Allowed Module Families:** `["cli", "envelope", "claim_inspection", "cost", "geo", "worksheet_input", "html", "decision", "capital", "packet", "guidance", "guidance_capacity", "track_record", "track_record_as_of", "track_record_binding", "track_record_data", "track_record_price", "lint", "lint_publicness", "lint_guidance", "lint_html", "mirror", "forecast", "reliance", "reliance_binding", "reliance_explicit", "reserve", "issued", "tables"]`

## 2. Mathematical & Domain Formulation
The `fmine` binary and library surface (`WP-06`, `BD-111`–`BD-117`, `BD-071`).

- DESIGN (BD-341): `track_record_binding` composes agreed, dated guidance transcriptions with annual
  Mine Stats Claims loaded by `fmine-public`. Supported quantity suffixes are `production_koz`,
  `production_kt`, `aisc_usd_per_oz`, `aisc_usd_per_lb`, `tcc_usd_per_oz` and `c1_usd_per_lb`, under
  `guidance_<site_key_with_underscores>_<suffix>` and period `YYYYFY`. The unit must match the suffix.
  Ownership is explicit `100%` or `attributable:n/d`; cost bases additionally name
  `;aisc=issuer-reported-per-sold-oz`, `;aisc=issuer-reported-per-sold-lb`,
  `;tcc=issuer-reported-per-sold-oz` or `;c1=issuer-reported-per-sold-lb`. No ownership, unit or metric
  conversion is implicit. Annual rates are read directly, never summed or averaged.
  The original outcome is the corresponding year's Q4 release. Later appearances keep their own Claims
  and scores; they do not fill a missing original. Guidance must precede the annual close, and supplied
  workbook availability must precede the requested cohort date. Each metric, year, canonical basis and guidance
  publication date forms a separate descriptive cohort, so revisions cannot increase the original cohort's member count. Exact
  normalized range positions, printed hits and rounding-aware classifications come from the calibration
  kernel. Missing originals remain in whole-cohort bounds; a boundary-straddling outcome remains explicit.
  Retrospective cards use this same binder through `assess_snapshot`, with no asserted as-of date.
  Their per-site, cross-year summaries keep guidance ordinals and comparative populations separate.
  Negative production is unusable. A degenerate printed range is interpreted as a point target.
- **Envelope:** `FMT-ENV-1` (`schema_version: "fmine.env.v1"`).
- **Verbs:**
  - `metrics geo --input <worksheet.toml>` (BD-062, GB-27): reads a regular UTF-8 file bounded to 1 MiB,
    hashes the actual bytes and evaluates `fmine-metrics::definitions::geo::worksheet`. Schema
    `fmine.geo-worksheet.v1` requires explicit physical scope, included metals and reference deck/version;
    unknown, duplicate or missing fields refuse. Two to sixty-four named/versioned decks compare the
    same quantities, with gold/silver in oz, copper in lb and positive prices in matching major currency
    units per metal unit. Every included metal needs its price in every deck; no mass, ownership or FX
    conversion is implicit. JSON/Markdown retain exact contributions, totals and signed reference
    differences, complete input/output Claims, source hashes, field locators and a deterministic digest.
    Worksheet Claims remain Assumed, Internal and uncalibrated; omitted metals are outside the declared
    population. Differences describe price-ratio effects on unchanged physical production and supply no
    source audit, reserve classification or disclosure authority. The invented example at
    `crates/xtask/testdata/production_geo.toml` yields 3,860,000 and 3,640,000 GEO oz, a −220,000 oz difference.
  - `metrics cost --input <worksheet.toml> --definition <registered-id>` (BD-061): reads the actual
    worksheet and workspace metric-registry bytes from regular UTF-8 files bounded to 4 MiB, binds their
    SHA-256 hashes, and evaluates `fmine-metrics::definitions::worksheet`. The explicit registered AISC/AIC
    rule determines each line's inclusion, exclusion or credit, including inherited rules. JSON schema
    `fmine.cost-worksheet.v1` retains exact numerator, sold ounces, USD/oz result, complete canonical Claim
    receipts, source metadata and definition successor; Markdown renders the same calculation. Imported
    values, cost classifications and completeness stay Assumed, Internal and uncalibrated. The adapter
    creates no ledger anchor or issuance certificate. The supplied `tests/costs/declared_aisc.toml` is an
    invented worksheet, not an issuer filing or independently verified complete cost population.
  - `html claims --issuer <registered> --definitions <mapping.toml> [--as-of YYYY-MM-DD]` (BD-057/BD-059):
    delegates to `fmine-public::html::run` to load registered HTML bytes and strict, value-free label mappings.
    The `fmine.html_claims.v1` payload returns full canonical Claim bodies and their semantic keys, exact
    decimal tokens/steps, value and label citations with original byte ranges and merged origins, mapping
    and registry hashes, selected source metadata and a separate Claims digest. Markdown renders the same
    values and evidence. The input mapping schema is `fmine.html-claims.v1`; it requires `id`, `file`,
    1-based `table`, A1 `cell` and the five `*_cell` / `*_label` pairs for subject, period, metric, unit and
    ownership. Unsupported semantic labels, missing or changed source bytes, duplicate numeric origins and
    duplicate semantic keys refuse. The optional inclusive publication cut is applied before any HTML
    source read; later mapped sources remain in `excluded_sources`, without fallback. No identity-ledger
    anchor, certificate, calibrated measurement or publication-timestamp authentication is created. HTML
    output is not gate-eligible, uses separate `html:` metric keys, and currently has synthetic test coverage
    rather than a registered live HTML corpus. Existing standalone Claim lookup selectors are unchanged;
    these Claim bodies and source interpretations are available directly in this command's receipt.
  - `decide card <model-file>`: a conditional monetary comparison on declared finite scenarios (BD-225).
    The adapter reads at most 4 MiB of UTF-8 from a regular file, hashes the actual bytes, and delegates parsing
    and exact evaluation to `fmine-decide::card`. The model supplies all weights, actions, payoffs and UU terms;
    every imported Claim remains Assumed and internal. JSON includes the source path/hash, complete core card,
    exact rankings, risk and regret. Optional information options add exact preposterior EVSI, EVPI, net value
    after declared test and delay costs, and every signal's posterior action ties (BD-227). Each option needs
    a complete state-by-signal likelihood matrix. Impossible signals remain explicit; no joint signal model
    or multi-test policy is inferred. All option evidence contributes to the card, including losing options.
    Exact enumeration reports Monte Carlo error as not applicable and supplies no Monte Carlo gate result.
    Markdown renders the same result and evidence. No issuance certificate,
    historical cut, calibration verdict, operational feasibility or execution authority is manufactured.
  - `decide program <model-file>` (BD-229): constructs all subsets of at most eight declared candidates
    on at most 256 shared states through `fmine-capital`. The bounded local model supplies exact USD costs,
    complete additive net delta-NAV payoffs, budget, prerequisites, exclusion groups, an exact tail mass
    and an optional signed-loss CVaR ceiling. Costs constrain spending without being subtracted again from
    net payoffs. Every bundle and rejection remains visible; risk is computed from the joint scenario sums.
    JSON and Markdown report exact co-winners, budget slack and the expected-value cost of the risk limit,
    with complete source-bound Claims and explicit reference-class adjustment bases. This is a conditional
    static comparison; staged recourse, entity waterfalls, variable share counts, empirical selection
    frequencies and execution authority are not supplied.
  - `lint <draft> --issuer <issuer> --html-definitions <mapping.toml> [--discover] [--as-of YYYY-MM-DD]`
    (BD-059/BD-345): checks only the mapped, registered HTML Claims through the existing exact equality
    kernel. The receipt retains full value, label, mapping and registry evidence. A separate source-publication
    assessment reports the registered dates; it does not establish historical availability of the current
    mapping. Future source bytes are excluded before reads. HTML cost-credit treatment is unstated, so an
    explicit by-product or co-product assertion remains unchecked. This source selector is exclusive with
    `--synthetic` and `--through`; ordinary workbook lint remains available through its existing selector.
  - `assure public --issuer <issuer> [--through YYYYQn]`: the gate-eligible run over the filings in
    `registries/sources/<issuer>.toml`, optionally only those at or before a quarter;
  - `assure public --synthetic`: the known-answer run, never gate-eligible;
  - either assurance selector with `--output <new-path>` and the host-clock issuance options below: save and
    register the exact report artifact, returning a separate issuance receipt;
  - `assure issue`: owner-authorized issuance of a freshly verified public or explicitly synthetic assurance
    report to a new local file, binding exact JSON bytes, canonical Claims, ledger anchor, source manifest,
    build-time revision, executable digest, generic recipient role and purpose (BD-347);
  - `reliance init|check|withdraw|supersede|refresh`: create, inspect and append local reliance events with
    explicit owner key, expected head and bounded time where the operation changes the journal;
  - `reliance export`: retrieve an existing artifact only after a pinned current-use check, keeping the
    journal lock through the exact-byte write. It does not create a new recipient record;
  - `recall --revision <revision>|--claim <id>`: list known affected copies and generic recipient roles,
    without sending messages or asserting receipt;
  - `assure challenges --issuer <issuer> [--through YYYYQn]` or `--synthetic`: content-addressed internal-review
    packets from visible assurance exceptions, with JSON or readable `--format markdown` output (BD-349);
  - `guidance completion --issuer <issuer> --site <site> --through YYYYQn --as-of YYYY-MM-DD`
    with optional `--capacity-document <registered file>`, or `--synthetic`:
    cited remaining-production and AISC requirements plus joint historical-window comparisons under an explicit
    historical-sales scenario, in JSON or readable `--format markdown` (BD-342);
  - `track-record --issuer <issuer> [--site <site>] [--through YYYYQn | --as-of YYYY-MM-DD] [--price-subject <subject>]` or `--synthetic`: cited issuer
    guidance against first-published annual outcomes, with separate comparative assessments, retained missing
    members, exact descriptive scores and JSON or readable Markdown output (BD-341);
  - `track-record --issuer <issuer> --cohort-as-of YYYY-MM-DD [--site <site>] [--through YYYYQn]` or
    `track-record --synthetic --cohort-as-of YYYY-MM-DD`: separate cohorts for each guidance publication,
    retaining original-outcome denominators and separate later appearances. This mode is distinct from
    `--as-of`, which selects earliest registered guidance with first-published and latest-at-cut assessments.
    The two date selectors are mutually exclusive; only `--cohort-as-of` rejects `--price-subject`;
  - `recovery gap --issuer <issuer> [--through YYYYQn]` or `--synthetic`: two cited historical
    grade-normalised recovery comparisons for Lumwana, with exact points and input-rounding enclosures
    (bd-srqc). The bound filters workbook releases for a retrospective computation using current verified
    historical documents; it does not establish a historical information cutoff;
  - `lint <draft> --issuer <issuer> [--through YYYYQn] [--discover]` or `--synthetic [--discover]`: bounded
    equalities checked against one verified canonical Claim snapshot, with JSON or readable Markdown output.
    Discovery searches for uncited matches while preserving ambiguous candidate sets (BD-345);
  - `mirror --asset <slug> --period <YYYY|YYYYQn|YYYYHn> --as-of YYYY-MM-DD
    --left-issuer <issuer> --left-document <file> --right-issuer <issuer> --right-document <file>
    [--metric production|sold]` or `--synthetic`: exact gold-production or ounces-sold compatibility under
    independently cited ownership and a common dated perimeter; production is the default (BD-346);
  - `forecast assess --ledger <path> --cohort <name> --as-of YYYY-MM-DDTHH:MM:SSZ
    [--selector first-published|latest-as-of] [--provenance <path>]`: read-only ordered transcript replay and
    cohort assessment through `fmine-calib`; first-published is the default (BD-343). An optional provenance
    bundle checks all declared ancestors at each frozen forecast's registration cut (BD-344);
  - `forecast journal init|register|freeze|outcome|resolve|restate|assess|inspect|report --journal <path>`: durable forecast admission,
    cohort freezing, append-only resolution and persisted assessment seals with runtime-assigned times (BD-047);
  - `forecast record score --record <directory> --as-of YYYY-MM-DDTHH:MM:SSZ --tsa-roots <pem>`:
    authenticated frozen-record assessment and common-population baseline comparison in JSON or Markdown (BD-076);
  - `reliance init|issue|reissue|check|reuse|withdraw|supersede|refresh|recall --journal <path>`: local issued-output
    status and exact artifact reuse, with capability-authorized mutations and generic recipient roles (BD-347);
  - `doctor`: per registered issuer, `sources_registered`, `sources_present`, `sources_verified` and one row
    per file, `{file, present, bytes_ok, digest_ok}` against the registry's size and sha256 (BD-339.S2). A
    mismatch is reported, never refused: doctor stays exit 0, status ok;
  - `claim show <id>`: the canonical public Claim;
  - `claim why <id>`: its recorded evidence, interpretation, computed color and admission guards;
  - `claim trace <id>`: its source anchors and the available provenance boundary;
  - `robot-docs`.

All three Claim commands accept `--issuer <issuer> [--through YYYYQn]` or `--synthetic` to select the same
verified source snapshot as assurance. Without a selector they search the current canonical projections.
`--json` is an alias for `--format json`.

DESIGN (BD-115): an explicit selector also resolves the recovery report's retained ownership, training,
fit, operating and projection Claims. Canonical identity and public clearance are checked before emission.
Typed values, canonical body bytes, value tags, parents, source anchors, recorded evidence and actual color/UU
guards remain visible. The trace walks the retained closure and names unresolved parents or unanchored leaves.
The recovery digest is separate from `source_identity_anchor_id` and `source_identity_state_root`;
`recovery_in_identity_certificate` is false, and the derived read's envelope anchor and certificate stay null.
This is a current verified projection, not a historical Claim store, calibration or freshness assessment.
Workbook Claim reads retain their established payload and identity-ledger certificate.

### Durable forecast registration (BD-047)

DESIGN: `forecast journal init` binds a cell id and operator-declared clock uncertainty to a new journal.
`register`, `freeze`, `outcome`, `resolve`, `restate` and `assess` require an exact predecessor `--head`; all mutations require
`--trust-host-clock`. The runtime clock is sampled under the journal's exclusive lock. The captured nanoseconds
remain in the journal frame's `observed_ns`; new admissions assign `ForecastLedger` registration, freeze and
recording times to the upper bound, `observed_ns + clock_skew_ns`. A caller cannot supply those times.
A registration or cohort freeze must precede every affected deadline even at that upper bound. Outcome
availability and assessment cuts must be at or before the lower bound. Clock rollback is refused, never clamped.

Requests still contain one permitted TOML table with no top-level fields, using the frozen
`fmine.forecast-ledger.v1` row grammar with the three assigned-time fields prohibited. The writer validates
an ordinary request, then adds `schema = "fmine.forecast-operation.v2"` to the stored operation; the public-bound
freeze and resolution operations described below use v3, and explicit vintage policies and comparative
restatements use v4. The `FJR1`
frame format is unchanged. Replay dispatches per operation: legacy rows without top-level fields retain
their original observed-time semantics and identities; v2 rows use the upper bound. Unknown schemas or extra
top-level fields refuse.
Replay also requires v4 for explicit `public_vintages` freezes; v4 admits only that freeze grammar or a
`registered_restatement`. Relabeling an ordinary operation or downgrading a new permission field refuses. Mixed journals preserve both policies without rewriting earlier operations.

DESIGN: a public-bound freeze adds `public_issuer`, `public_metric`, `public_period`, `public_ownership`
and `public_members` to the cohort request, with `resolution_rule = "registered-public-cell"`.
The member string maps existing aliases to exact source subjects (`alias=Subject,other=Other Subject`).
The adapter converts aliases to immutable forecast IDs and embeds the canonical
`fmine_calib::ledger::binding::PublicResolutionPolicy` in the cohort's hashed resolution rule.
The complete member set, sole eligible issuer, metric/unit, original FY/quarter release and ownership
basis are fixed before any outcome. Supported metrics are site-level gold `production_koz`, copper
`production_kt` and gold `aisc_usd_per_oz`, over calendar years (`YYYYFY`) or individual calendar quarters;
ownership is `100%` or `attributable:n/d`. AISC means the issuer's reported per-sold-ounce cost,
with no subtraction of annual and year-to-date rates or implicit ownership conversion.
Copper production retains the printed kilotonnes and the same exact source-Claim, original-release and
ownership checks. Gold figures, pounds, and unsupported copper cost definitions never substitute for that
target. Both production metrics refuse negative values during admission and semantic replay; an unmarked
printed zero remains valid. Copper adds no durable encoding change for existing gold targets or receipts.

`forecast journal resolve --journal <path> --head <hex> --cohort <name> --forecast <alias>
--trust-host-clock` takes no outcome value, target override, source locator or availability timestamp.
Under the exclusive writer lock it loads the original registered Mine Stats release, verifies source
bytes and the canonical Claim, and matches the exact frozen issuer, subject, period, metric, unit, scope
and ownership. It uses the displayed decimal, retaining the stored value and exact rational separately;
the existing score kernel receives that rational's deterministic floating-point projection.
Ambiguous, missing, placeholder, status-marked, stale, altered and incompatible sources refuse without
appending. Candidate cardinality is checked before canonical-key projection, and a reader refusal on either
QTD/YTD summary sheet blocks admission: an unusable competing row cannot disappear and leave a false singleton.
Unrelated detailed-sheet refusals do not replace the target. A later comparative cannot fill a missing original.
An unmarked printed zero remains valid. Cross-crate forecast replay controls retain their CLI harness and all
assertions in `crates/xtask/testdata/forecast_replay_controls.rs`.
The six cross-crate forecast-record source fixture tests likewise retain their CLI harness and unchanged
assertions in `crates/xtask/testdata/forecast_record_sources.rs`.
Publication date and its registered FACT/INFERENCE basis are mandatory; that date must fall after the
complete target period. Retrieval never substitutes for a missing publication date, and the stated basis
is retained metadata rather than independent proof of publication time. Conservative availability is
midnight after that date, checked against the sampled clock's lower bound before admission.

Public-bound freezes and generated `registered_outcome` operations use
`schema = "fmine.forecast-operation.v3"`; their authority time uses the same upper-bound policy as v2.
The raw `outcome` route refuses every public-bound member, including members shared with other cohorts.
The generated operation retains the frozen cohort digest, source Claim ID and canonical body, exact and
displayed values, ownership provenance, source anchor and publication evidence. Replay reconstructs the
original issuer Claim from the frozen policy and retained evidence, checking its complete canonical body
and identity before restoring the event. This prevents value, subject, period, basis or anchor substitution.
`source_admissions` and each selected member's `source_admission` distinguish recorded source verification
from unverified supplied outcomes. Replay does not re-read source files or assert present file availability.
The journal head binds admission evidence; the existing assessment digest binds the cohort, numeric event
and citations. Refused resolutions leave members unresolved in the frozen denominator.

DESIGN (BD-343): `public_vintages = "registered_comparatives"` is an optional pre-freeze permission for
source-bound restatements. Omission or `original_only` retains the existing version-one policy bytes;
opting in uses the canonical version-two policy. A shared forecast ID cannot acquire different vintage
permissions through another cohort. `forecast journal restate --journal <path> --head <hex>
--cohort <name> --forecast <alias> --release YYYYQn --trust-host-clock` requires an already admitted,
source-verified original outcome. Under the writer lock it selects the latest numeric predecessor and
resolves a strictly later registered reporting quarter against the same physical target, subject, metric,
unit, ownership and sold-ounce cost definition. The registered publication day must follow both the target
period and selected reporting quarter; availability remains midnight after that day. Neither the numeric
value, source Claim, publication evidence nor superseded sequence is accepted from the caller.

Explicit `public_vintages` fields require a v4 cohort operation during both admission and replay; an older
operation version cannot acquire the newer policy grammar. Omitting the field preserves existing v3 freezes.
The generated `registered_restatement` operation uses `fmine.forecast-operation.v4` and retains the
predecessor sequence alongside the same complete source evidence as an original admission. Replay checks
the frozen permission, original admission, chronological predecessor and reconstructed canonical source
Claim before appending `OutcomeKind::Restated`. The kernel retains the original event for `first-published`
and selects a later event for `latest-as-of` only within both its availability and recording cuts. Saved
reports retain their first checkpoint's prefix, operation-version counts and selected source admission.
No source files are re-read during report replay. Unknown versions, repeated or earlier releases, changed
ownership/denominators, missing originals and altered source bytes refuse without a journal append.
Automatic status events remain outside the supported domain.

The existing calibration kernel requires an outcome's availability and recording time to be at or before the
assessment cut. For example, an observed recording at `12:00:01.500Z` with a one-second skew bound is assigned
`12:00:02.500Z`; an otherwise eligible outcome remains unresolved at a later assessment's `12:00:02Z` cut.
This closes the interval in which an outcome's recording uncertainty could cross its historical information cut.

Every new assessment operation binds the actual computed result in its existing `expected_digest` field,
even when the caller supplies no expectation; that field is mandatory for v2 replay. The digest is computed
under the writer lock; a caller's conflicting expectation refuses, and the append callback still checks clock
bounds and replays the complete semantic transition before any write. The callback also prepares the complete
receipt and report against the candidate record and head, so a rendering refusal leaves the journal unchanged.
Success is returned only after append and sync. Reopening verifies each bound result, global seals, frozen
membership, missing outcomes and original/restated selection. Legacy unbound checkpoints remain readable and
retain their explicit weaker binding status; existing frame bytes and semantic identities do not change.

The `fmine.forecast-journal.v1` payload reports the cell, exact head, clock bounds, operation schema, inventories
and checkpoints. `operation_schema` continues to identify the row grammar. `operation_version_counts` contains
`legacy_v1` and `v2` counts excluding genesis, plus `v3` and `v4` when present; their sum is `n_operations`. `record_time_policy` is `none`,
`legacy_observed`, `clock_upper_bound` or `mixed`. An assessment adds `report` with mode `durable_journal`;
its version counts and time policy describe the prefix through its first persisted checkpoint. A v2 checkpoint
over legacy operations remains mixed and does not upgrade the earlier time evidence. `inspect` is read-only
and can verify an independently retained head; an unpinned read establishes internal consistency only.
`report --assessment <digest> [--head <hex>] [--provenance <path>]` retrieves one existing persisted assessment
under the journal's shared lock. It accepts no new cohort/cut/selector and samples no clock. The saved tuple
selects the existing kernel result, whose digest must match; missing assessments refuse `FMINE-REF-201`, wrong
head pins retain `FMINE-REF-502`, and digest mismatches retain `FMINE-REF-505`. It appends nothing and creates no
new persistent seal. The payload identifies `persisted_assessment` retrieval, the selected digest, read-only status and
`result_digest_binding` (`persisted_checkpoint` or `caller_requested_digest_only`), using the first matching
checkpoint's binding even if a later checkpoint pins the same digest. Its `record_time_basis` is
`observed_host_clock_with_declared_uncertainty`, `clock_upper_bound_with_declared_uncertainty` or
`mixed_versioned_record_times`, according to that first checkpoint's prefix.

Without a supplemental provenance bundle, the complete nested `payload.report` is reconstructed at the first
matching checkpoint, retaining its original `journal_head`, `durable_journal` mode, checkpoint prefix,
time-policy labels and `expected_digest_matched` values. Under the same implementation, this JSON is byte-identical
to the original checkpoint's report after later appends, restatements and reopening. Repeating an assessment
does not replace its first report. The outer
payload's head and inventories describe the currently opened journal, including any later checkpoints; its
retrieval metadata remains separate from the historical report. The stored digest binds the assessment result;
this contract does not freeze report presentation across software versions. Source admissions retain their
recorded verification evidence; other supplied citations remain unverified;
`--trust-host-clock` declares an operator's bound and provides no independent timestamp proof. Without an
explicit timestamp receipt, external time proof is not verified. Envelope authority anchors/certificates remain null.
An optional provenance bundle checks every frozen member at its exact recorded registration time, including
unresolved members, using the existing temporal-cone verifier. Provenance timestamps alone accept strict UTC
fractions of one through nine digits with checked nanosecond conversion. A leakage refusal retains the structured
frontier and shortest paths. The supplied bundle digest and temporal result are supplemental: the saved
assessment digest and journal bytes remain unchanged. This does not bind the bundle into the original
registration, authenticate its declared times or perform generator replay.

DESIGN (BD-047, BD-068): every journal receipt exposes `timestamp_commitment` with schema
`fmine.forecast-journal-prefix.v1`, head, sequence and `imprint_sha256`. The imprint is SHA-256 of exact ASCII
`fmine.forecast-journal-prefix.v1\nhead=<64 lowercase BLAKE3 hex>\nsequence=<unpadded decimal>\n`, including final LF;
it is not the BLAKE3 head relabelled as SHA-256. Mutations prepare this commitment before append and release it
only after sync. It is suitable for `openssl ts -query -digest <imprint> -sha256 -cert`.

`report` additionally accepts the complete group `--timestamp-head <hex> --timestamp-receipt <path>
--timestamp-roots <pem>`, with optional `--timestamp-intermediates <pem>`. Partial, empty, duplicate or
mutation-side proof options are usage errors. No caller-provided sequence or imprint is accepted. Verification
holds the journal's shared lock, locates the exact retained prefix, authenticates its SHA-256 commitment using
the existing offline RFC 3161 verifier and replays that prefix with its original operation-version semantics.
The selected assessment's exact cohort and every member must exist there, with no selected-member outcome
events (including Missing) yet recorded. The authenticated upper bound must strictly precede all frozen-member
deadlines and all current recorded Published/Restated availability times, including events beyond the saved
assessment cut. Comparisons retain nanosecond precision through i128; equality refuses `FMINE-REF-206`.
An absent prefix refuses `FMINE-REF-502`; invalid receipt/imprint/trust fails `FMINE-REF-505`. Explicit proof
failure never falls back to success without proof.

Proof success adds outer `timestamp_verification` and `external_time_proof: verified_caller_selected_roots`,
including the exact prefix, cohort, member count, receipt/root fingerprints and authenticated time bounds.
The historical nested report is unchanged by timestamp verification, and verification reads no clock and
writes no journal bytes. Evidence is scoped to caller-selected roots and current journal declarations; host
timestamps, publication completeness and source truth remain unverified. The attested prefix is independent
of optional `--head`, which pins the final opened history and still detects suffix rollback.

Malformed inputs, stale heads and semantic refusal cannot append. A failed write/sync poisons the writer and
retains unconfirmed bytes for inspection. No command repairs, truncates or deletes journal history.

### Issued-output reliance (BD-347)

`assure public --output <new-path>` requires `--journal`, `--head`, `--release-key`, `--trust-host-clock`,
`--recipient-role`, `--purpose` and `--code-revision`. Its `--format json|markdown` controls both the saved
artifact and stdout receipt rendering, with JSON as the default; the receipt is separate from the stored
report bytes. `--artifact-format` belongs only to `reliance issue`, which retains independent artifact formatting.
Both host-clock issuance routes render the existing assurance envelope and bind its exact bytes, sorted output Claim IDs, actual
ledger anchor, source/transcription/cutoff registry bytes and optional-file absence, consumed workbook/PDF
hashes, lane and public label. The runtime executable is hashed; `--code-revision` is explicitly caller-declared.
Public clearance, explicit staleness and canonical Claim IDs are checked without declassification. `init` binds
a private raw 32-byte release key to the journal's role and freshness policy; capability possession proves no
human review or regulatory authority. Host-clock mutations require that key, an exact independently trusted `--head`
and `--trust-host-clock`. That clock flag declares the operator's bound; it supplies no external time proof.
Missing, mismatched or stale trust inputs leave `check` unknown and refuse current `reuse` or `reissue`.
`reissue` takes `--artifact`, `--output`, `--source-issuance`, `--recipient-role` and `--purpose`, plus the
journal/key/head/clock options. It selects one existing issuance by its canonical `issuance_id`, checks exact
bytes and current freshness before creating a destination or appending, and preserves the original code revision,
binary, input manifest, Claims, anchor, lane and taint; only recipient, purpose and issuance time change.
Authorization failure, unknown status, staleness, withdrawal or supersession refuses before destination creation.
Issuance and recall results expose `issuance_id` (BLAKE3 of the canonical IssuanceRecord frame), distinct from
`output_digest` (exact artifact bytes) and `journal_head` (the status history). Explicit-time results retain the
equal `issuance_digest` compatibility field. Identical artifact bytes need not identify one producer issuance.
All artifact writes use create-new semantics. `reuse` holds the journal read lock through its status check and
local copy; it appends no issuance and cannot track that copy. `recall` covers recorded issuances only and sends
no notifications. Ordinary stdout report emissions and other report families remain unregistered; this is not
complete BD-347 coverage or proof of delivery.
Withdrawal names affected Claim IDs, or explicitly declares that all statements are affected; it preserves
historical digest verification. Supersession names an already recorded replacement;
refresh records a new status cut without reviving withdrawn or superseded outputs. If artifact creation succeeds
but the final journal append fails, no confirmed new issuance receipt or recipient registration exists. Retain
any destination artifact and resolve the journal before using that copy; an earlier issuance of the same digest
may still be current.

In the host-clock grammar, `check --digest ... --require-current` returns `FMINE-REF-202` (exit 3) when current
reuse is denied, retaining the complete status and byte-verification payload.
`check --digest ... --as-of YYYY-MM-DDTHH:MM:SSZ` inspects historical status under
the same fresh trust inputs and never grants current reuse; the two flags are mutually exclusive. The payload
includes the effective `reliance.as_of_ns`. Without a trusted clock, or if that clock fails, historical artifact
bytes can still be verified while reliance remains unknown. No historical timestamp can enter a mutation.
Issuance and reuse delegate to the canonical I/O adapter: they keep the journal lock through create-new writes,
use private file permissions, and sync both the artifact and its parent directory. Artifact checks reject
symlinks, FIFOs and other special files before opening them, so they cannot block a withdrawal behind a reader.
FACT (BD-347, `tests/reliance.rs`): actual-binary regressions exercise exact report bytes, two recipient copies,
withdrawal and replacement, refused current reuse, stale heads, wrong keys, tampered artifacts, private file modes,
and a FIFO that must refuse before it can hold a journal lock indefinitely.

## 3. Authority, Epistemics & Representation
- Every JSON invocation prints exactly one `FMT-ENV-1` envelope on stdout, a usage error included (`BD-329.S3`).
  Markdown wraps that envelope in a JSON fence, except successful challenge exports, which render the cited
  packets as readable sections with the source root, packet-set digest and internal-review boundary. Successful
  guidance reports also render readable requirements, historical comparisons, checked replays and source citations.
  Track-record Markdown presents guidance vintages, original outcomes, later comparatives, missing members,
  descriptive aggregates and source citations.
- After `anchor_id` every envelope has, in this order:
  - `flags`: the four INV-002 flags, all false, from one constant (no field or setter can set one);
  - `certificate`: the report's ledger state root (hex), or null when no ledger snapshot is registered;
  - `warnings` (an array) and `next_actions` (`[{command, why}]`; every command parses with `parse_args`);
  - `no_claim`, on report envelopes only;
  - then `payload` and `refusal`.
- A successful assurance run carries the run's `anc_…` anchor id, and the report as payload: `lane`,
  `gate_eligible`, sources with SHA-256, the extraction census, the identity ledger with cited exceptions, the
  quarantine and the state root. The payload holds no flags, no `no_claim` and no `next_actions`.
- The `waterfall` payload (BD-333) reports first-published year-on-year public AISC bridges, signed additive
  fallbacks and named refusals. Bars carry rounding intervals and source citations; exact printed-rate gaps
  remain separate. It is descriptive and non-identifying, and its digest is bound into the M0AS v8 certificate.
- A successful `recovery gap` envelope carries a `fmine.recovery.gap.v1` payload and the model's explicit
  `no_claim` text. Its own digest binds source rows, source Claims, the fit and quarterly outcomes. Both
  `anchor_id` and `certificate` remain null; no ledger snapshot or forecast registration is created. Each
  quarter retains its earliest registered release and comparative status, both model outputs or a named
  refusal, citations and input-rounding bounds. The residual sample and regression-error SDs retain their
  separate denominator conventions and numerical-evaluation bounds.
- A successful `mirror` emits `fmine.jv_mirror.v1` with both reporters' raw readings, source metadata,
  ownership Claims, source/perimeter anchors, normalized 100% Claims and an exact common interval or a
  checked exclusion certificate. The proof retains original closed rounding cells and Scale identities;
  checked minimal source sets are explicitly non-exhaustive, and indeterminate attribution keeps its reason.
  Source-set indices address the canonical `inputs` array; proof cells retain their scope for digest replay.
  `physical_nonnegative` constrains the common quantity without erasing a printed zero's negative half-step.
  The canonical payload digest binds evidence and selection; `anchor_id` and envelope `certificate` stay null,
  `gate_eligible` is false, and the four authority flags stay false. Markdown renders the same sources and
  result. Synthetic inputs and their derived Claims remain Assumed; no affiliation or physical-truth claim is made.
- DESIGN (BD-341): the retrospective `track-record` payload retains `fmine.guidance.track-record.v1`; optional
  `--cohort-as-of` selects `fmine.guidance.track_record.v1` dated cohorts through `track_record_binding` and the
  Claim scoring path. Its digest binds evidence and computation; `anchor_id` and `certificate` remain null.
  Synthetic runs reject issuer, site and release selectors and may take an explicit date for the dated example.
  Dated summaries preserve missing original outcomes and separate later scores; retrospective summaries retain
  per-site histories and comparative populations. Exact normalized positions, rounding bounds and certified
  rational IID reference endpoints remain exact in JSON; any display approximation is marked.
- An issuer run's payload ends with `published_cutoffs` (BD-274), `fmine_metrics::cutoff::section` over
  `registries/published_cutoffs.toml` when that registry's `issuer` is the run's: the registry's sha256, the count
  of cut-offs reproduced, within input rounding and in conflict, and one row per cut-off. It is not in the state
  root, so the certificate is the same with or without it. No registry means no field; a registry that does not
  read, or a row that cannot be computed, refuses the run with `FMINE-REF-506` before any workbook is read. The
  synthetic lane never carries it.
- A refusal carries `{code, exit_code, what, fix}` and a null payload.
- Field names are this crate's. Renaming them to FMT-ENV-1's (`v`, `event`, `node`, `anchor`, `data`) and
  bumping `schema_version` are open for the FMT-ENV-1 owner.

## 4. Public API Surface
- `OutputFormat`, `CliCommand`, `Envelope`, `NextAction`
- `run` (what `fmine` prints, and its exit code), `invoke`, `parse_args`, `execute_command`
- `forecast_journal::{Command, execute, execute_with_clock}`: the actual host-clock adapter or an explicitly
  injected clock capability for callers/tests. Trust in an injected clock belongs to its caller, not request data.
- `forecast_record::{Command, execute}`: a read-only adapter over exact frozen record bytes, explicit RFC 3161
  trust roots and registered original sources; numerical scoring delegates to `fmine-calib`.
- `packet::{packets_from_report, ChallengePacket}`; immutable packet accessors, `validate`, `to_json`,
  `to_markdown`. The existing assurance engine computes the states; this module packages their evidence.
- `track_record_binding::{run, assess, assess_snapshot, synthetic, synthetic_as_of, TrackRecordInput,
  TrackRecordSnapshot, TrackRecordReport, TrackRecordRow, OutcomeAssessment, TrackRecordSummary,
  NO_CLAIM}` (BD-341). `run(root, issuer, site, through, as_of)` uses `fmine-public` loaders to check
  registered source bytes and the local two-reader transcription registry. Registered runs require complete
  agreement across the guidance namespace; optional price reader failures remain separate. `assess` performs
  the same projection over caller-supplied checked evidence and can retain pending guidance diagnostics,
  without establishing byte or timestamp authenticity.
  `to_json` emits `fmine.guidance.track_record.v1` with source Claims, original and later outcomes, distinct
  guidance vintages, pending/unsupported rows, sparse-data gate counts and a content digest. The digest
  commits to the selection, publication-date evidence, source/basis interpretation, outputs and limitations.
  `assess_snapshot(profile, transcriptions, claims, site)` returns only source-bound rows and pending or
  unsupported transcriptions; its internal unbounded date is never emitted as a user information cut.
  Both operators reuse `TrackRecordRow::to_json`; the dated report also carries the full input Claim digest
  and source mapping refusals. Rules version `guidance.track_record/2` exposes the certified rational
  `binomial_90_exact` interval beside its approximate display. `synthetic_as_of(date)` applies an explicit
  cut to the same invented fixture; `synthetic()` retains its fixed deterministic date.
  A canonical numeric source under the reserved synthetic issuer may remain Assumed/Unknown through its
  interpreted outcome and descriptive score; arithmetic never promotes its evidence state. The source Claim
  remains unchanged and parented in the score chain. Guidance targets retain their published-statement parent
  and publication metadata. Synthetic handling never applies to real issuers, placeholders or explicit negative
  evidence states and does not bypass identity, support, ownership, confidentiality, staleness or calibration checks.
  The binder rejects mismatched synthetic/issuer labels, mixed issuers and any non-public input before retaining
  rows. Unmatched document hashes and conflicting guidance readers refuse. Unusable, ambiguous, divested or
  partially withheld annual outcomes remain unresolved; later releases cannot erase the original reason.

## 5. Guards & Typed Refusals
- DESIGN (BD-346): `mirror` selects two distinct issuer profiles and explicit documents, without default
  partners, shares or asset boundaries. Documents-only profiles use the existing sources/transcription
  schemas and required `[[subject]]`; workbook consumers retain their no-workbook refusal. Selected documents
  must be `mda`, `aif` or `partner_filing`, with a `FACT: ` publication basis and a publication date between
  the reporting-period end and the requested as-of cut, inclusive. Registered byte/hash failures retain
  `FMINE-REF-506`/`505`; incomplete selected reader agreement retains `202`, including a differing quantity
  key at a selected locator. All supplied selected assertions participate; unrelated documents are excluded.
  Keys are `mirror_<asset_underscored>_<lowercase_period>_gold_produced_100_koz`, `_gold_produced_attr_koz`
  and `_ownership_percent`, replacing `gold_produced` with `gold_sold` for sold ounces. Entry periods are
  uppercase `YYYYFY`, `YYYYQn` or `YYYYHn`; numeric units are `koz`, ownership is an exact point in `percent`.
  Attributable inputs require a separately cited ownership row; full-mine inputs require no invented share.
  Every basis has the fixed grammar
  `<100%|attributable|equity>;perimeter=<slug>;members=<slug+slug>;valid=<YYYY-MM-DD/YYYY-MM-DD>;scope_page=<page>;scope_table=<label>;scope_row=<label>;scope_col=<label>`.
  Operation lists are complete and duplicate-free, both reporters' perimeter IDs and member sets must match,
  and validity covers the entire requested period. Scope locators reference the same checked document.
  The ownership row must match its attributable row's dated perimeter. Missing scope/share evidence refuses
  `104`; incompatible units, bases or non-point quantities refuse `105`; no calendar, perimeter or cost bridge
  is inferred. Quantity intervals retain closed half-last-place precision; exact ownership scaling and the
  independently checked feasibility/certificate come from `fmine_metrics::mirror`.
- DESIGN (BD-347): the explicit-time grammar never samples the host clock or trusts a journal head it just
  read. The existing host-clock grammar requires its separate `--trust-host-clock` declaration; flags from
  the two grammars cannot be mixed. Unpinned, stale, future or insufficiently bounded status is `unknown`, never permission to reuse.
  A historical `--as-of` result never grants current reuse. Check and recall emit metadata, not artifact bytes.
  Mutation keys are exact nonzero private binary files. Append uses a caller-supplied expected head under an
  exclusive lock; wrong keys, stale writers and invalid transitions cannot overwrite an artifact or journal.
  `assure issue` uses the same verified source projection and JSON envelope as `assure public`, including its final
  newline; its `--format` affects only receipt rendering. Inputs are bound separately from exact output bytes.
  Source errors retain their original refusal.
  For `assure issue`, build revision is embedded by `build.rs`; a dirty build is marked and an unknown or
  unrelated enclosing-repository revision refuses issuance. The opened Linux `/proc/self/exe` image is
  hashed with BLAKE3, preserving identity across pathname replacement. DESIGN: executable hashing streams
  through a fixed-size buffer and verifies the opened file's exact metadata length, including an EOF check.
  Report-artifact byte limits do not constrain the executable; empty, shortened or growing images refuse.
  Platforms without a supported executing-image handle refuse this command with `FMINE-REF-202`.
  A runtime checkout cannot relabel an installed executable. For `assure issue`, no supplied revision, Claim list
  or artifact digest substitutes for the producer's verified report. Owner-key possession is local release capability,
  not a human signature, disclosure clearance or an independent time proof.
  New-report issuance routes share `reliance_binding` for the exact envelope, Claim closure and input manifest,
  and use `Journal::issue_bytes`; `reliance reuse` and `reliance export` use `Journal::export_current`. This preserves
  cross-command artifact identity, private create-new writes, parent-directory synchronization and journal
  locking throughout the status check and copy. Shared record rendering labels revision evidence as
  recorded; only the immediate `assure issue` receipt attests build-time capture.
- **Exit 0:** ok.
- **Exit 2:** a usage error with no refusal code. This covers:
  - an unknown verb or flag, a surplus word, or a flag the verb does not take (none is ignored);
  - a missing `--issuer`;
  - an unregistered issuer (no `registries/sources/<issuer>.toml` in the workspace);
  - a malformed `--through` bound, or `--through` with `--synthetic`;
  - `--issuer` together with `--synthetic`.
- **Exit 3–8:** the catalog refusal's own exit code. For example, `FMINE-REF-506` (exit 5) means a registered
  file is missing or unreadable, and `FMINE-REF-505` (exit 8) that one is altered.
- **Exit 5, no envelope** (BD-112): stdout could not take the envelope (a closed pipe, as under `| head -c 1`,
  or a full disk). `fmine` writes one line to stderr and exits 5, the I/O-error code, instead of panicking
  with exit 101. Exit 1 is never emitted.
- `claim show|why|trace <id>` resolve public Mine Stats and AISC Claims by re-reading the registered, hash-verified workbook
  inputs (BD-332.S3, BD-115). It returns the semantic key, both stored and displayed text, ownership and
  status evidence, and the source locator/digest. Mine Stats uses its direct reader; AISC uses the existing
  assurance projection. Exact IDs emitted by `assure public --synthetic` also resolve by rebuilding that
  existing fixture projection; their evidence remains synthetic, and no real claim is replaced by one.
  No view creates a parallel claim store. An invalid or absent id refuses with
  `FMINE-REF-201`; missing or altered source files retain
  their typed input refusals. Source assertions remain issuer assertions and are not decision-ready.
- DESIGN (BD-115): an explicit `--issuer` or `--synthetic` selector rebuilds that source projection after verifying every registered
  workbook's size and SHA-256. Its `fmine.claim_read.v1` payload includes the original keyed Claim, lane,
  `claims_digest`, staleness and calibration-failure flags, and whether a decision UU term exists. The envelope
  carries the original ledger anchor and state root; the separate projection digest binds interpretation.
  Selected `why` and `trace` views add their inspection object to that same payload. Their next actions retain
  the issuer, through bound or explicit synthetic selector. A Claim absent from the selected snapshot refuses
  with `FMINE-REF-201`, without searching another lane. Source failures retain their original refusal.
  This is a current projection read, not a durable historical store or a decision-calibration claim.
- `why` delegates to `fmine_public::key::inspect::why_json`. It calls `Claim::color`,
  `verify_uu_slot_for_decision` and `verify_export_clearance(Public)`, preserving their typed refusals in
  the successful inspection payload. The decision result is scoped to the color and UU guards used by
  `DecisionCard::build`, not a completed decision or authority grant. Actual UU terms are shown; an absent
  term, calibration assessment, decision context or flip margin is never invented. Infinite estimated
  dispersion is represented as `null` with `dispersion_status: unbounded`, not as zero. The recorded stale
  flag is shown separately from freshness, which has not been assessed by a clock or dependency replay.
- `trace` delegates to `fmine_public::key::inspect::trace_json`. Public projection leaves carry exact
  artifact paths, SHA-256 digests and cell locators, plus matching registered source URLs and release metadata.
  Built-in synthetic anchors have no invented registry row. Parent IDs remain explicit and unresolved if
  supplied: the trace is then partial and never claims a complete derived provenance DAG. Both views return
  the unchanged canonical Claim alongside their explanation and link to the other inspection commands.
- Claim kinds not produced by the registered public projection refuse; the verb never
  fabricates a claim.
- `doctor` refuses `FMINE-REF-506` when the registries cannot be read.
- DESIGN (BD-349): challenge packets cover visible AISC states ii–v, Mine Stats states iii–v and infeasible
  joint components, preserving each finding's original state, operands, Claim references, source hashes,
  certificates, alternatives and missing-fact request. Source resolution or operand mismatches refuse.
  Packet ids bind their content, and `packet_set_sha256` binds the deterministic ordered set separately from
  the unchanged assurance root. `human_check` is null and external use is hardwired false; generation cannot
  manufacture review or identify a culprit from a sign-flip witness or correction alternative.
  - Unresolved Mine Stats/AISC vintage changes and ownership or ambiguous-release conflicts also produce
    packets. Each finding retains first registered, immediately previous and current canonical Claims,
    both comparison states and any verified disclosure note. Grouping keeps period, unit, ownership,
    scope and line occurrence distinct. The builder recomputes each document family's comparison once
    from canonical Claims, refusing forged verdicts, predecessor chronology or disclosure metadata.
    Disclosed-only changes, unchanged/first/single appearances and non-comparable scope boundaries remain
    in the assurance report without becoming challenges. No vintage is selected as true; silence means
    no applicable note in registered documents. Packet IDs and immutable draft restrictions apply unchanged.
  - `identity_transition` packets cover exceptional adjacent AISC identity histories, including corrected,
    broken and reclassified calculations with unchanged printed operands. The builder recomputes the histories
    from canonical Claims and the original identity ledger, rejecting forged states, classifications or chronology.
    Both original appearances retain their states, certificates, operands and source Claims. An ambiguous earliest
    release has `first_registered: null` and every `earliest_registered_assertions` entry is retained. Closing
    requests name both compared locators and the missing rule or evidence; the transition itself assigns no
    disclosure classification. Persisted and single-vintage histories add no transition packet.
  - `mine_stats_identity_transition` packets cover exceptional Mine Stats identity histories through the same
    `assure challenges` command. The builder recomputes the complete projection from canonical Claims and the
    original ledger, including counts, bindings and registered subject labels. Findings retain earliest sources,
    the previous bound identity, current identity and all intervening ambiguous source assertions. Ambiguity
    before or at the earliest bound appearance leaves `first_registered` null; ambiguity between appearances
    prevents a persistence or correction conclusion. Recorded missing inputs and typed refused/unresolved
    states remain visible; absent source bytes or altered operands refuse export. The transition supplies no
    issuer pass score or disclosure classification.
  - `transcription_conflict` packets preserve agreed PDF assertions with page/table/row/column locators,
    raw tokens, exact decimals, units, basis, period, reader roles and checked document metadata. The exporter
    recomputes the registered rounding-convention comparison and checks the retained conflict population.
    A mismatched quantity context is reported as such, never as a numerical issuer discrepancy. Truncation
    remains among the candidate conventions; nearest-rounding failure alone does not establish a conflict.
    The packet digest binds this evidence separately from the workbook ledger root. Reader agreement does
    not supply the packet's independent human review. Quarter-bounded assurance runs retain their existing
    behavior of excluding PDF transcriptions; the unbounded issuer run includes the local registry when present.
- DESIGN (BD-342): guidance delegates acquisition to `fmine_public::guidance` and arithmetic to
  `fmine_metrics::guidance`. Its payload digest binds the cited inputs and explicit scenario. `anchor_id` and
  `certificate` remain null because this is not a registered forecast or ledger snapshot. Missing two-reader
  guidance, incompatible ownership/support, future evidence and unusable sales refuse; synthetic evidence is
  available only through the explicit synthetic lane.
  T1 output retains exact source-quarter/replay-quarter points and checked exclusion certificates under the
  separately declared sales scenario. No usable historical window remains T3. Markdown retains exact fractions
  when a decimal would round, labels display approximations and escapes source text; it never adds authority.
  The independent `technical` section renders a T2 production-only comparison when registered, agreed
  same-period technical inputs exist; otherwise it retains a typed T3 reason without hiding the requirements
  or history. Declared upper bounds, plan and nameplate scenarios remain distinct. Exact input limits,
  scenario production ceiling, optimistic required grade, evidence classes and page citations survive both
  JSON and Markdown. The historical tier is labelled separately; neither technical non-exclusion nor a
  historical witness establishes joint technical production/cost feasibility.
  Optional `--capacity-document` binds a selected registered `technical_report` through
  `guidance_capacity`: verified bytes, publication at/before the cut and two agreeing readings for the same
  mine and remaining months. Source checks select its three capacity roles within the named document while
  retaining every reader at each selected locator; unrelated technical inputs cannot block that comparison.
  The full-mine gold limits must explicitly state maximum remaining dry feed,
  grade and recovery, with exact units and `100%;bound=explicit-upper;metal=gold` basis; ordinary averages or
  annual nameplate values do not qualify. The adapter delegates ceiling and required-grade arithmetic to
  `fmine_metrics::capacity`. The T2 result is a production-only exclusion test; NotExcluded supplies no
  feasibility witness or probability. Its Claims retain source anchors and interpretation. A composite digest
  binds both the arithmetic/history and capacity reports; the envelope still has no authority certificate.
- DESIGN (bd-srqc): recovery delegates source acquisition and selection to `fmine_public::recovery` and
  fitting/projection to `fmine_metrics::recovery`. The public path needs registered, byte-verified PDFs,
  agreed local historical/ownership transcriptions and verified workbook Claims. Missing model inputs
  refuse with `FMINE-REF-201`; document disagreement, source failures and incompatible support retain
  their typed refusals. With valid training, unusable quarters remain explicit refused rows in an otherwise
  successful report. The explicit synthetic selector runs invented inputs through the same adapter/kernel.
  `--synthetic` is exclusive with `--issuer` and `--through`; recovery accepts neither `--site` nor `--as-of`.
  The existing `assure public --through` path continues to skip PDF transcriptions, independently of
  the dedicated recovery command's retrospective workbook bound.
- DESIGN (BD-345): lint bounds its local UTF-8 draft read, verifies the selected sources once, and delegates
  every finding to `fmine_public::lint`. Its certificate identifies the source assurance snapshot; the lint
  payload separately binds the draft and canonical Claim population. Only number spans and findings are
  emitted, never the draft's full prose. Drafts are never written or sent over a network. A completed check
  exits zero even when findings report mismatches; invalid syntax is exit two and source failures retain
  their typed refusals. Explicit `--discover` delegates uncited equalities to the public discovery engine;
  existing links retain their cited checks. Multiple semantic matches are exposed and never resolved by the
  draft's amount. The selected source snapshot is verified in either mode; supported discovery grammar is
  not a general-prose accuracy claim.
- DESIGN (BD-345.S4): `lint --issuer <issuer> --as-of YYYY-MM-DD` adds a dated public-record companion
  report without changing the equality assessment. Optional source publication dates and their cited
  FACT/INFERENCE bases come from the same single registry parse as the numerical source projection.
  Earliest means earliest matching dated registered assertion. Every finding retains exact canonical
  source Claims, byte spans, locators, file hashes, date bases and rejected or later evidence. Ambiguous
  attributable shares stay unresolved even when one amount matches. Unknown publication dates remain
  unknown; retrieval dates never establish publication. Changing dates or their basis changes the report
  digest while the numerical Claim remains unchanged. JSON and Markdown retain both assessments, and
  neither grants disclosure permission or a regulatory-compliance claim. Synthetic/unknown evidence cannot
  become a public disclosure assertion.
  `lint_publicness` also delegates Q2 = H1 − Q1, Q3 = 9M − H1 and Q4 = FY − 9M to the exact
  `fmine_metrics::roll_forward::quarter` kernel for gold production/sales (`koz`) and copper production (`kt`).
  The shared equality target binds the original draft's subject, metric, year, unit and ownership syntax;
  no target-period Claim is invented. All relevant cumulative candidates retain their provenance. Only
  dated, usable public issuer Claims available by the cut can supply values, and every admitted pair must
  share explicit normalized ownership, occurrence and registered perimeter. Missing display precision,
  placeholders, changing scope, incompatible bases and conflicting vintages cannot establish a derivation.
  Closed displayed-number intervals propagate exactly; rates are excluded by the typed quantity API.
  The `derivation` object retains every pair and exact enclosure, input evidence and the new-information
  warning. `derivable` never overrides a direct disclosed or contradictory assertion. Derived values
  remain arithmetic assessments rather than new issuer Claims, forecasts, calibration or disclosure authority.
  `lint_publicness::sums` additionally binds complete Q1/Q2 sums for H1 and Q1/Q2/Q3/Q4 sums for FY
  to `fmine_metrics::public_derivation`. The same extensive quantity, source-admission and dated-cut
  constraints apply. Exactly one eligible source is required per quarter; competing vintages stay
  ambiguous without selecting by their amounts. Unusable operands with unknown publication dates
  block the derivation; only positively post-cut operands may be excluded before usability checks.
  The `quarter_sum` object retains every candidate, exact bounds, canonical interpreted input Claims,
  derived Claim bodies and source ancestry. Its full enclosure must fit the draft's displayed interval
  before `derivable` is reported. Direct disclosures and contradictions retain priority, while missing
  quarters, unknown dates, insufficient precision and arithmetic refusals remain visible. These Claims
  establish arithmetic over source assertions, never an additional issuer disclosure or permission.
  Both modes include processed/head grade (`g/t`, `grams per tonne` or percent) and whole-site recovery
  (percent), with explicit subject, dated period and ownership. Percent-format fractions use the source's
  displayed percentage-point interval while retaining the stored fraction. Bare/mined grade, circuit recovery,
  and reserve replacement remain outside the discovery grammar. Copper C1 requires explicit per-pound-sold
  and `US$/lb` or `$/lb` tokens; copper production requires `kt`. C1 does not inherit gold TCC's credit
  convention, and historical site production in millions of pounds remains unchecked.
- DESIGN (BD-345.S4): `lint --as-of` also supplies an independent `forward_guidance` assessment for a
  standalone line explicitly marked `forecast` or `guidance`, with one equality, registered site, fiscal
  year, ownership basis and unit. Gold/copper production, gold/copper AISC, TCC and C1 reuse the canonical
  guidance transcription binder. The latest matching publication at or before the cut supplies the
  comparison; same-date ties, distinct attributable shares and pending readers remain unresolved.
  Draft point/range endpoints are compared exactly, with no implied probability or rounding tolerance.
  Canonical source and interpreted Claims, ancestry, date basis, hash and page locator remain in the report.
  Guidance is selected by quantity and date before document bytes are read. Its source failures remain
  local findings and cannot discard the workbook equality result. A document-only registry is supported;
  it emits no workbook state root, authority anchor or certificate. Pending readers conservatively block
  the same quantity even across other years or ownership bases. Unrestricted prose, explicit share syntax,
  real issuer coverage and disclosure permission remain outside this predicate.
- DESIGN (BD-343): forecast assessment replays transcript rows in their supplied order using the existing
  calibration kernel. No alternate scoring formula, default time proof or hidden denominator is introduced.
  Saved assessment rows enforce the kernel's sealed-cut rules across replay. Both envelope authority fields
  remain null: transcript and assessment digests identify content, not a durable authority ledger or an
  external timestamp. The adapter does not verify supplied source citations or issue a forecast.
- DESIGN (BD-047/BD-048): transcript and live journal registration also accept `kind = "empirical"` with
  `samples` as a quoted comma-separated list of one through 4096 finite numbers. Samples have uniform
  weights, repeated values retain their mass, and original order remains in the frozen identity. JSON retains
  every sample and names the inverse-ECDF quantile convention. CRPS, central interval scores and missing-member
  completion bounds come from the calibration kernel; no Gaussian fit or interpolation is introduced.
  Registered gold/copper source resolution and immutable assessment retrieval use the existing target and
  receipt checks unchanged. The 64 KiB live operation limit applies independently of the sample-count limit.
  This representation is univariate and supplies neither joint-realization semantics nor calibration evidence.
- DESIGN (BD-076): `forecast record score` binds the manifest, forecast CSV and rules to their exact framed
  SHA-256 digests and verifies both timestamp signatures through `fmine_io::forecast::timestamp`. Complete
  proof intervals enforce rules-before-record, the frozen registration deadline, the scored-year boundary
  and the explicit assessment cut. Trust roots are caller-selected and fingerprinted; a successful signature
  check does not grant public authority to a local root. This path does not weaken xtask's compiled root pins.
  Original FY-minus-nine-month production retains signed residuals; unit costs use Q4 QTD values directly.
  Source hashes, exact displayed values, raw tokens and canonical Claims remain bound to the report.
  Guidance uses the latest pre-cut publication and two-reader agreement; fixed Q3/history benchmark inputs
  follow the frozen rule and may postdate the forecast cut. Their availability and manifest membership are
  explicit. Missing originals, unknown guidance and incomplete history retain the full cohort and named
  unavailable comparators. Only historical ratio pairs require strictly positive values; a printed zero
  point remains valid. Every mean in the paired comparison uses the same complete member set.
  Publication-date availability is bounded at end of day, with an explicitly labelled retrieval fallback;
  frozen source retrieval times come from the manifest. JSON and Markdown expose completion bounds, the
  conditional iid reference and rho sensitivity separately. The assessment digest binds record/rules hashes,
  proof metadata, source evidence and scores. All authority flags remain false, anchor/certificate stay null,
  and no sample size promotes the descriptive report to a calibration claim.
- DESIGN (BD-344): `--provenance` reads a strict `fmine.forecast-provenance.v1` bundle, binds exact forecast
  IDs, and checks every cohort member's complete declared ancestry at its registration time. The scoring
  `--as-of` time cannot admit later evidence. Forecast roots must be derived, name inputs and have a performed
  time equal to registration. Availability and cell/sequence anchor checks delegate to `fmine_claim::dag`.
  Missing members, missing times/parents, cycles, backdating and future ancestors refuse. Future-input
  refusals include structured frontier nodes and shortest paths in the envelope payload, without a score
  report. Successful payloads bind the bundle's SHA-256 and each cone digest. Without a bundle the status
  is `not_checked`; the CLI always reports generator replay as `not_performed`. Supplied metadata remains
  unauthenticated evidence. The pure replay API is a separate caller-mediated operation.
- DESIGN (BD-341): `track-record --as-of` delegates source selection to `fmine_public::guidance::track_record` and
  all scoring to `fmine-calib`. The explicit calendar-date cut prevents an implicit wall-clock information
  set. The payload digest binds guidance, source Claims, selection and results; the envelope's authority
  anchor and certificate remain null. Only the earliest registered agreed guidance per subject, metric,
  fiscal year and ownership basis enters each denominator; all revisions remain visible. First publication
  and comparatives are reported separately, and unresolved outcomes retain their members and reasons.
  Unknown issuers, empty sites, non-date cuts and conflicting selectors are usage errors. Source integrity
  and transcription failures keep their typed refusals. The explicit-cut schema is
  `fmine.guidance.track-record.as-of.v1`; the existing retrospective command and its synthetic lane retain
  `fmine.guidance.track-record.v1`. `--through` and `--as-of` are mutually exclusive; synthetic selection
  remains retrospective and accepts neither filter. Both real-source modes accept `--price-subject`.
  The explicit-cut report retains the base digest as `comparison_digest`; its top-level `digest` binds the
  base comparison and separately selected first-published/latest-registered price assessments in a framed
  `GTAP` version 1 encoding. Price documents are restricted to the same evidence cut; annual royalty Claims
  must belong to the corresponding selected outcome release. Required quarterly history ends at the last
  completed quarter within the declared 2024Q1–2026Q2 range; unavailable members remain explicit. Later price
  documents cannot change the earlier-cut result. The borrowed pricing view reuses the exact public selector
  and preserves canonical Claim chains without manufacturing replacement inputs.
- `robot-docs` lists every catalogue row as `{code, exit_code, status, summary}`, `status` being `active` or
  `proposed` (`docs/adr/ADR-REFUSALS-01.md`).

## 6. Proof Obligations (`PO-*`)
- FACT (BD-345.S4, `xtask/tests/public_lint_derivations.rs`): actual minimal workbook bytes and registered
  source metadata exercise quarter differences, exact displayed rounding, all competing vintages, date cuts,
  unknown dates, tampering, signed/zero/placeholder behavior, direct contradictions and ownership/perimeter
  admission. JSON and Markdown preserve the source Claims and warning; authority flags remain false.
- DESIGN (BD-346, `tests/mirror.rs` and `xtask/testdata/mirror_oracle.py`): documents-only synthetic reporters
  exercise registered byte and reader
  verification, exact common intervals and point contact, checked witnesses/minimal conflict sets, 45%+45%
  ownership, physical zero, scope/date/unit refusals, isolated optional sources and same-named partner files.
  Python's standard rational arithmetic independently checks serialized normalization, witness equations and
  minimal interval conflicts. Real-binary JSON/Markdown, false flags, canonical ordering and evidence-bound
  digests are exercised without claiming real partner-filing coverage.
- FACT (BD-347, `tests/reliance_lifecycle.rs`): real-binary synthetic workflows cover issuance, exact ordinary
  report bytes, multiple recipient roles, pinned checks, recall, withdrawal, supersession and current retrieval.
  Wrong keys, stale writers, unsafe trust, altered registered workbooks and existing destinations refuse while
  preserving evidence. Withdrawn and superseded bytes still authenticate but cannot pass current reuse.
- `PO-CLI-1`: `assure public --synthetic` exits 0 with `lane` `synthetic`, `gate_eligible` false and the seeded
  totals.
- `PO-CLI-2`: `assure public --issuer barrick` without the registered files exits 8 and names them.
- `PO-CLI-3` (real files, ignored test): with the files it exits 0 with 34 tables, 2,595 instances and totals
  `{i 2120, ii 3, iii 4, iv 17, v 391, np 60}`.
- `PO-ADIT-1`: every verb's envelope matches a hand-written frame (`test_po_adit_1_golden_every_verb`; payload,
  anchor, certificate and refusal text masked), and `tests/e2e_envelope.sh` checks the printed line of each.
- `PO-CLI-4` (BD-115, T `public_claim_inspection`): `test_claim_show_why_trace_golden` compares all three synthetic inspection payloads
  with hand-written expected output. Default tests preserve guard failures, explicitly partial provenance,
  exact-ID lookup and altered-source refusal for every view. The ignored real source test checks E474's
  issuer assertion, refused color and registered source trace without upgrading its evidence.
- FACT (BD-115, T `public_claim_inspection`): `test_claim_show_reads_reported_claims_and_refuses_invented_ids` round-trips numeric
  and placeholder Claim ids, projection digest and ledger root through all three selected views. The corpus test
  `test_claim_show_reads_real_mine_stats_and_aisc_claims` covers both adapters. `test_claim_show_rechecks_registered_source_bytes`
  exercises the registered-file path, preserves a through bound despite a later altered source, and refuses
  altered in-scope sources and Claim IDs from another selected lane.
- FACT (BD-342/349, CLI and packet tests): known-answer end-to-end tests cover exact rational guidance,
  preserved assumptions/citations, deterministic JSON and Markdown, missing evidence, mixed selectors,
  seeded discrepancy witnesses, joint-only conflicts and immutable review restrictions. The ignored real-corpus
  packet test retains the known Kibali and Porgera findings across both registered vintages.
- `PO-ADIT-4` (BD-112, `tests/exit_codes.rs`): 400 seeded argument lists (Philox; half are edits of a
  well-formed command for each verb) and every verb in both formats exit 0 or 2–9, with the envelope's
  `exit_code` and `status` matching. The generator must reach ok, usage, exit-3 and other refusals. The built
  binary, given a closed stdout, exits 5 with a line on stderr. This is a seeded generative test, not a
  coverage-guided fuzzer, and no registry is present, so issuer runs past file loading are not reached.
- `PO-CLI-RECOVERY` (bd-srqc): the synthetic command must return both cited model outputs with deterministic
  JSON, false authority flags and null ledger anchor/certificate. Invalid selector combinations are usage
  errors. The registered command preserves missing-input and altered-source refusals and the explicit
  retrospective meaning of `--through`; successful per-quarter results retain their source and model bounds.
- FACT (BD-349, `packet::tests`): four transition regressions cover corrected/broken footings, unchanged values
  reclassified by a later source-bound reconstruction, forged history, ordering and ambiguous earliest releases.
  The real-corpus packet test asserts complete coverage of the existing exceptional identity-transition population.

- FACT (BD-341, `tests/track_record_binding.rs`): the invented `[100,120]` target against `125` yields
  exact normalized position `3/2`. Regressions distinguish printed hits from rounding-boundary ambiguity,
  retain missing originals beside usable comparatives, keep revisions in separate cohorts, reject mutated
  source identities and incompatible bases, and verify deterministic ordering and digest sensitivity.
  Further regressions retain Unknown synthetic sources without promoting their evidence state, reject
  negative production, preserve certified binomial bounds and omit future appearances in dated demos.
  The registered-workbook regression uses explicitly hypothetical guidance and synthetic assumed copies
  parented by authenticated workbook Claims, preserving their source anchors, to test real source binding:
  original North Mara production/AISC and Lumwana production, comparative-only FY2023, and the FY2025
  disposal/withholding exclusions. It does not establish actual issuer guidance history. Lumwana annual
  AISC's unprinted ownership remains a basis refusal rather than an inferred full-mine assertion.
- FACT (`tests/track_record_as_of.rs`): cohort command regressions cover explicit dates, incompatible selectors, deterministic
  synthetic output, false authority flags, absent ledger certificates, valid next actions, readable Markdown and
  machine-readable command discovery. Source binding is checked in `tests/track_record_binding.rs`; numerical
  known answers remain in the calibration crate.

## 7. Failure Modes & Deterministic Fallbacks
None: every failure is a usage error or a typed refusal.

## 8. Gates & Kill Criteria (`GATE-*` / `KILL-*`)
- `GATE-BULL-01`

## 9. Lifted Code & Foundation Dependencies
- Depends on `fmine-calib::guidance_scores::claims` for Claim-typed descriptive range scores and summaries
  (BD-341); `track_record_binding` supplies publication selection, source/basis interpretation and citations.
- Depends on `fmine-core`, `fmine-units`, `fmine-time`, `fmine-crs`, `fmine-evidence`, `fmine-claim`,
  `fmine-runtime` (explicit host-clock boundary),
  `fmine-registry`, `fmine-array`, `fmine-numerics`, `fmine-calib`, `fmine-decide`, `fmine-io`, `fmine-public`,
  `fmine-metrics`.

## 10. Non-Goals & Scope Boundary
- DESIGN (BD-063): `reserve replacement --issuer <issuer> --bindings <path> [--as-of YYYY-MM-DD]`
  authenticates explicitly selected PDF operands and delegates complete annual-window arithmetic to
  `fmine-metrics::reserve_replacement`. Exact ratios of sums retain independently printed aggregate points,
  joint closed rounding bounds, separately qualified headlines and each GEO year's registered deck.
  Approximate language supplies no tolerance. The command verifies the full declared document population
  before loading readings, rejects post-cut documents before file IO, and refuses accepted quantities with
  competing pending locators. Missing operands leave the declared series visible with a nonzero202 result;
  other complete series remain present. Incompatible rounded headlines are findings, not loader failures.
  JSON and Markdown retain signed tokens, definition metadata, source hashes/locators, publication evidence
  and reader roles. The wrapper digest also binds exact binding/rule-file bytes, the cut and pending readings.
  No workbook population or implicit source/basis conversion is needed. The report uses current rules,
  carries no ledger certificate, remains public and `gate_eligible:false`, and leaves all authority flags false.
  An explicit date checks selected document availability, not full hindcast closure. The committed Barrick
  bindings contain no values; PDF bytes and real transcription inputs remain ignored pending OPEN-R7-1.
  Source-bound arithmetic and AI reading checks do not close the authentic independently reviewed data gate.
- The mirror supports produced and sold gold in koz and sustaining, project or total capital in USD millions
  from explicitly selected, agreed document assertions. It neither selects a correct reporter nor establishes
  physical truth or disclosure permission. Actual NGM, Pueblo Viejo and Kibali filing coverage, resolved NGM
  vend-in boundaries and definition/cost/accounting bridges remain open; synthetic/source-adapter checks do
  not close full BD-346 acceptance.
- DESIGN (BD-346.S4): `mirror --metric capex-sustaining|capex-project|capex-total` binds each monetary row's
  explicit cash/accrual timing, included/excluded lease treatment and separately cited definition locator.
  Missing, unstated or unequal bases refuse instead of applying an inferred bridge. `--metric capex` requires
  all three scopes from both selected reporters and reports their independent rounding-box comparisons beside
  the joint component-sum assessment. Joint totals must foot exactly from printed components; their underlying
  bounds then come from those components. Every supplied complete ownership-basis triple participates, and an
  absent unambiguous triple keeps the joint result unavailable. Signed monetary values retain their sign;
  neither a physical nonnegativity clamp nor a currency conversion is applied. JSON `interval_usd_m` fields
  remain distinct from physical `interval_koz`, and definition citations survive into source and derived Claims.
  An explicit `--synthetic --metric capex` demonstration uses invented, Assumed evidence. Integration tests
  exercise the executable with generated source bytes and independently calculated rational expectations;
  it proves the adapter, not authentic issuer filing coverage or the full BD-346 gate.
- DESIGN (BD-340.S5/S8): `check tables` exposes the existing workbook identity ledger, or checks a strict
  `--definitions` registry of PDF sums, reserve roll-forwards and named reserve-replacement ratios. The PDF mode verifies every declared
  document's bytes, even when its values have no readings, and selects only the declared quantities while
  retaining all readers at their locators. It delegates exact arithmetic, joint consistency, checked
  certificates and correction sets to `fmine-metrics::roll_forward`; it selects no corrected value.
  Infeasibility is a reported finding with exit zero. Missing or altered source bytes and disagreeing readers
  retain their typed refusals. Missing agreed operands remain explicit unresolved identities, including
  their available evidence. PDF report digests bind definitions, readings, results and pending readers;
  they are not authority-ledger anchors or external timestamps. Workbook mode retains its existing ledger
  anchor and certificate. All authority flags remain false. JSON and Markdown retain signed source tokens,
  period, unit, basis, document vintage and citations. Optional `reserve_row` declarations also collect their
  same-document price readings and render `fmine-metrics::reserves` under `payload.issuer_reserves`.
  Definitions and every selected source remain hash-bound. The arithmetic slice does not classify reserves,
  infer table membership or complete the real-PDF acceptance population. The separate declared reserve-row
  projection selects the latest dated MD&A/AIF filing, keeps older vintages and investor-deck cross-checks
  visible, and retains ambiguous or unusable latest filings rather than falling back to an older assertion.
  An accepted value or price with a competing pending location refuses `FMINE-REF-202` with both citations;
  ordinary missing second readings remain visible as unavailable rows and pending evidence.
  Replacement declarations additionally load the selected workspace's metric registry and bind the selected
  definition metadata into the report. Gold/GEO metal, attributable contained basis, ex-A&D scope, three-year
  window, operand units, signs and price-deck key are explicit. Signed depletion must remain strictly positive
  throughout its rounding interval. Shared amounts are checked jointly; shared quoted-rate reuse refuses as
  unsupported. Replacement ratios consume already-converted GEO amounts; they do not perform a conversion
  or establish the complete real reserve-replacement gate.
- DESIGN (BD-340.S4, BD-070): the same definitions path accepts `geo_check` and linked `geo_candidate`
  declarations over existing reserve rows. `payload.issuer_reserves.geo_checks` retains the exact stated-deck
  calculation under the declared convention, its proper/point-only/empty fit, and a separate strict-convention
  comparison across explicit candidate decks. Candidate prices are verified independently of their unrelated
  reserve amounts. An unusable candidate prevents discovery instead of disappearing from the population;
  any usable stated-deck result stays visible. Equal-ratio distinct decks remain ambiguous, and identical
  economic parameters retain every source provider. `checked` and `unavailable` are separate from arithmetic
  fit. No finding replaces an issuer value, registered deck, current filing or document convention.
- DESIGN (BD-274): `cutoff` and linked `cutoff_input` declarations use the same verified document/reader
  loader and delegate to `fmine-metrics::cutoff::source`. Every input comes from the declared technical report,
  with matching period, explicit role units, physical/model basis and separate gold-price basis. Constant
  recovery and TCM-over-gold recovery use the existing reproduction kernel; the gold price is an exact quoted
  parameter and other inputs retain displayed rounding bounds. `payload.checks.cutoffs.rows` distinguishes
  `reproduced`, `within_input_rounding` and `conflict` from `refused` inputs. Markdown renders the same evidence,
  exact results and bounds. Arithmetic conflicts are inspection findings with exit zero; source failures keep
  their typed refusals. This does not select an operating cutoff or classify a resource or reserve.
  `--as-of YYYY-MM-DD` is optional only with `--issuer` and `--definitions`. Every declared document must
  have a registered publication date at or before that cut; a later document refuses the command rather
  than silently reducing the declared population. Default inspection remains retrospective. The outer
  digest binds the selected cut, verified-document inventory and publication-date evidence as well as the
  arithmetic reports. These are current registered metadata, not external time proofs or a reconstruction
  of historical transcription entry dates. Pending second readings remain visible and leave their cases
  refused; source-byte failures and reader disagreements retain the existing command-level refusals.
  The seven reference-only `assure public` cases are not promoted to source-verified evidence.
- DESIGN (BD-341): `track-record --issuer <issuer> [--site <subject>] [--through YYYYQn] [--price-subject <subject>]` is a read-only
  adapter over agreed PDF transcriptions, canonical annual Mine Stats Claims and `fmine-calib::guidance_scores`.
  `--synthetic` uses invented inputs through the same adapter. JSON is the default (`--json` is an explicit
  alias); Markdown renders the same selected values, unresolved reasons, later appearances and source hashes.
  Original annual releases and comparative-only selections have separate populations, including unresolved
  members. Every guidance document is retained; its ordinal is registered order, not proof of initial guidance.
  Source-byte failures preserve their typed refusals. The payload digest binds the selected source inventory
  and report; it is not a ledger anchor, forecast registration or external timestamp. All authority flags stay
  false. Unsupported guidance metrics and incomplete guidance agreement refuse instead of disappearing from the report.
  Both retrospective and explicit-as-of entry paths load the `guidance_` population before source-byte and
  reader checks. Both CLI modes also retain `aisc_price_assumption_` and `realized_price_` for their
  paired price sensitivities. Missing, altered, pending or disputed optional technical rows do not change a valid
  guidance report. Conflicting guidance readers, unsupported guidance and invalid registered identities remain visible
  or refused according to each mode's existing rules; quantity selection never discards a reader at a selected locator.
  Unresolved outcomes retain every selected canonical source Claim, including simultaneous conflicting cells,
  so refusal does not erase its reported token, interpretation, locator or source hash. Later appearances stay
  separate. JSON and Markdown both expose that rejected evidence without scoring it or dropping its member.
  Before numerical use, the adapter rechecks the Claim identity, metric and complete canonical public value
  tag, including the semantic key and typed interpretation. Mutated source assertions remain unresolved
  members with `FMINE-REF-507`; numeric placeholders refuse with `FMINE-REF-208`.
  The retrospective adapter computes through canonical `guidance_scores::claims` inputs, scores and site-history
  summaries. Guidance statement IDs bind publication date, printed token, interpretation, locator and document
  hash. Interpreted outcomes retain the original source Claim as a parent. JSON v1 adds `guidance.source_claim`,
  `guidance.claim`, `interpreted_outcome_claim`, `score.claim` and each summary's `claim`; existing numeric fields
  and the row-level `claims` object are projections of those Claims under `guidance-track-record/2`. The Claims
  retain canonical body bytes, value tags, source ancestry, evidence, knowledge state, confidentiality and UU
  terms. Markdown exposes their links, included years and certified rational binomial bounds. Explicit numeric
  Unknown/Assumed synthetic inputs stay Unknown in derived Claims; other admission failures retain typed
  refusals. Unresolved years remain in the history denominator. This wiring creates neither a calibration
  verdict nor a probability for guidance.
  Registered-file integration tests construct invented workbooks and two-reader document entries, then check
  exact scores, original/comparative selection, ownership separation, source tampering, missing bytes,
  transcription disagreement, deterministic replay and actual JSON/Markdown executable output.
  The adapter supports gold/copper production and explicit issuer-reported per-sold-unit AISC, TCC and C1;
  automatic cost-definition reconciliation, transcription, year-effect estimation and full BD-341 data/coverage
  gates remain outside this slice. Actual calculations live in the calibration kernel.
- DESIGN (BD-341.S5): `track_record_price` binds agreed, hash-verified price readings and canonical original
  Mine Stats royalty Claims to the exact paired sensitivity kernel. Every expected quarter from 2024Q1 through
  2026Q2 is required, truncated by an explicit `--through` bound or the last completed quarter at `--as-of`;
  missing releases or unusable inputs leave
  the row unsplit with its reason and evidence. Price assumptions must come from the same guidance document;
  realized-price documents must be dated at or after the end of their stated period.
  An explicitly transcribed `all_sites` assumption can serve several guidance rows from that same document
  without duplicating its physical locator; overlapping site/shared assumptions are retained as ambiguous.
  Site prices are the default. `--price-subject` explicitly chooses a differently named price series and the
  result identifies that proxy. Commodity-price conventions are retained separately from site ownership.
  AISC sensitivity also requires the scored period's royalty/price interval to meet the historical coefficient.
  JSON and Markdown retain the unconditional score beside every conditional result, exact input-rounding
  bounds and source citations. TCC and C1 remain unsplit. No controllability or causal claim is created.
  Numeric royalty Claims must be Known or Estimated, except explicitly synthetic deterministic Assumed/Unknown
  inputs. That exception never overrides Conflicted, NotObservable, Redacted, Stale or other negative states.
  Incomplete optional price transcriptions preserve the unconditional report, while pricing returns
  `incomplete_transcription_agreement` with pending/conflicting locators, reader roles, quantity identities
  and checked source hashes. The complete report digest binds these diagnostics. Guidance completeness
  remains mandatory; mixed guidance/price identity disagreements and unknown rejection identities refuse.
  Missing or altered selected guidance/price document bytes still refuse before either report can be produced.
- Computes nothing itself: all calculation is delegated to the `V`/`E`/`K`/`S0` crates.
- Track records are descriptive projections. They do not issue forecasts, authenticate an external time proof,
  estimate dependence or effective sample size, provide bias confidence intervals, or satisfy the full BD-341
  data/coverage gates. Conditional binomial intervals are labelled with their independence assumption. Sparse
  evidence and unprinted outcome bases remain explicit; real guidance transcriptions are local inputs.
  Point targets have no normalized range position. Capex and one-sided transcription syntax are outside the
  cohort source binder's supported input schema.
- Writes only explicitly requested local reliance journals and artifact files; never sends messages or writes
  to an external system. Recall covers recorded copies; local retrieval does not prove remote delivery or
  recall every detached copy. The local journal is not a replicated authority service.
- Challenge packets do not cover hidden or not-published instances. Human review and external-release
  sequencing remain open for every packet, including PDF transcription conflicts.
- Guidance completion supplies a T1 finite historical comparison when usable original-quarter evidence exists,
  otherwise historical T3, under a declared sales scenario. Its independent T2 technical comparison is
  conditional on cited dry-feed, head-grade and recovery inputs for the exact remaining months. It supplies
  no probability of meeting guidance, physical-impossibility conclusion, joint technical production/AISC
  witness or completed M0 gate.
- Recovery outputs describe model differences with an annual historical relation. They make no causal,
  recoverable-production or forecast claim, emit no dollar sensitivity, and do not turn comparative-only
  observations into first-published outcomes. Input-rounding bounds are not confidence or prediction intervals.
- Track-record coverage is the registered two-reader transcription population, not a complete historical
  issuer census. Guidance has no inferred probability level. The binomial reference interval is conditional
  on independent trials; source-rounding and missing-outcome bounds have separate meanings. Evidence-cut
  reports retain their unconditional scores alongside separate first-published and latest-registered price
  sensitivities. Causal cost attribution, cluster inference, forecast registration and the full
  guidance data gate remain open.
