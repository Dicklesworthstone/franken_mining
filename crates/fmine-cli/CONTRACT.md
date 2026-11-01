# CONTRACT.md — `fmine-cli`

## 1. Charter & Layer
- **Crate:** `fmine-cli`
- **Layer:** `U` (User Surfaces & CLI)
- **Subsystem:** `Surfaces`
- **LOC Budget:** `26k planned / 30k ceiling` (`registries/loc_budget.toml`)
- **Allowed Module Families:** `["cli", "envelope", "packet", "guidance", "guidance_capacity", "track_record", "track_record_data", "track_record_price", "lint", "forecast", "reliance", "reliance_binding", "reliance_explicit", "issued"]`

## 2. Mathematical & Domain Formulation
The `fmine` binary and library surface (`WP-06`, `BD-111`–`BD-117`, `BD-071`).

- **Envelope:** `FMT-ENV-1` (`schema_version: "fmine.env.v1"`).
- **Verbs:**
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
  - `recovery gap --issuer <issuer> [--through YYYYQn]` or `--synthetic`: two cited historical
    grade-normalised recovery comparisons for Lumwana, with exact points and input-rounding enclosures
    (bd-srqc). The bound filters workbook releases for a retrospective computation using current verified
    historical documents; it does not establish a historical information cutoff;
  - `lint <draft> --issuer <issuer> [--through YYYYQn] [--discover]` or `--synthetic [--discover]`: bounded
    equalities checked against one verified canonical Claim snapshot, with JSON or readable Markdown output.
    Discovery searches for uncited matches while preserving ambiguous candidate sets (BD-345);
  - `forecast assess --ledger <path> --cohort <name> --as-of YYYY-MM-DDTHH:MM:SSZ
    [--selector first-published|latest-as-of] [--provenance <path>]`: read-only ordered transcript replay and
    cohort assessment through `fmine-calib`; first-published is the default (BD-343). An optional provenance
    bundle checks all declared ancestors at each frozen forecast's registration cut (BD-344);
  - `forecast journal init|register|freeze|outcome|assess|inspect --journal <path>`: durable forecast admission,
    cohort freezing, append-only resolution and persisted assessment seals with runtime-assigned times (BD-047);
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

### Durable forecast registration (BD-047)

DESIGN: `forecast journal init` binds a cell id and operator-declared clock uncertainty to a new journal.
`register`, `freeze`, `outcome` and `assess` require an exact predecessor `--head`; all mutations require
`--trust-host-clock`. The runtime clock is sampled under the journal's exclusive lock. The captured nanoseconds
reach `ForecastLedger` directly; a caller cannot supply registration, freeze or recording time. A registration
or cohort freeze must precede every affected deadline even at the clock's upper bound. Outcome availability
and assessment cuts must be at or before its lower bound. Clock rollback is refused, never clamped.

Requests contain one permitted TOML table with no top-level fields. `FJR1` version one operations use the frozen
`fmine.forecast-ledger.v1` row grammar, with the three assigned-time fields prohibited. Semantic replay uses the
existing calibration kernel. Assessment operations are persisted, retaining their optional expected digest,
global seals, frozen membership, missing outcomes and original/restated selection across reopening.

The `fmine.forecast-journal.v1` payload reports the cell, exact head, clock bounds, operation schema, inventories
and checkpoints. An assessment adds `report` with mode `durable_journal`. `inspect` is read-only and can verify
an independently retained head; an unpinned read establishes internal consistency only. Source citations remain
unverified, external time proof is not verified, and envelope authority anchors/certificates remain null.
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
  Track-record Markdown presents guidance vintages, outcome states, descriptive aggregates and source citations.
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
- `packet::{packets_from_report, ChallengePacket}`; immutable packet accessors, `validate`, `to_json`,
  `to_markdown`. The existing assurance engine computes the states; this module packages their evidence.

## 5. Guards & Typed Refusals
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
  hashed with BLAKE3, preserving identity across pathname replacement. Platforms without a supported
  executing-image handle refuse this command with `FMINE-REF-202`.
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
  draft's amount. The selected source snapshot is verified in either mode. Publication-time/publicness remains
  unassessed; supported discovery grammar is not a general-prose accuracy claim.
  Both modes include processed/head grade (`g/t`, `grams per tonne` or percent) and whole-site recovery
  (percent), with explicit subject, dated period and ownership. Percent-format fractions use the source's
  displayed percentage-point interval while retaining the stored fraction. Bare/mined grade, circuit recovery,
  copper C1 and reserve replacement remain outside the discovery grammar.
- DESIGN (BD-343): forecast assessment replays transcript rows in their supplied order using the existing
  calibration kernel. No alternate scoring formula, default time proof or hidden denominator is introduced.
  Saved assessment rows enforce the kernel's sealed-cut rules across replay. Both envelope authority fields
  remain null: transcript and assessment digests identify content, not a durable authority ledger or an
  external timestamp. The adapter does not verify supplied source citations or issue a forecast.
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
- `PO-CLI-4` (BD-115): `test_claim_show_why_trace_golden` compares all three synthetic inspection payloads
  with hand-written expected output. Default tests preserve guard failures, explicitly partial provenance,
  exact-ID lookup and altered-source refusal for every view. The ignored real source test checks E474's
  issuer assertion, refused color and registered source trace without upgrading its evidence.
- FACT (BD-115, CLI tests): `test_claim_show_reads_reported_claims_and_refuses_invented_ids` round-trips numeric
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

## 7. Failure Modes & Deterministic Fallbacks
None: every failure is a usage error or a typed refusal.

## 8. Gates & Kill Criteria (`GATE-*` / `KILL-*`)
- `GATE-BULL-01`

## 9. Lifted Code & Foundation Dependencies
- Depends on `fmine-core`, `fmine-units`, `fmine-time`, `fmine-crs`, `fmine-evidence`, `fmine-claim`,
  `fmine-runtime` (explicit host-clock boundary),
  `fmine-registry`, `fmine-array`, `fmine-numerics`, `fmine-calib`, `fmine-decide`, `fmine-io`, `fmine-public`,
  `fmine-metrics`.

## 10. Non-Goals & Scope Boundary
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
