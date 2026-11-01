# CONTRACT.md — `fmine-io`

## 1. Charter & Layer
- **Crate:** `fmine-io`
- **Layer:** `V` (Verification, Ingestion & Assurance)
- **Subsystem:** `Ingestion`
- **LOC Budget:** `40k planned / 46k ceiling` (`registries/loc_budget.toml`)
- **Allowed Module Families:** `["xlsx", "zip_inflate", "styles", "numfmt", "numtok", "reliance", "forecast"]`

## 2. Mathematical & Domain Formulation
Pure-Rust ZIP and RFC 1951 DEFLATE (`zip_inflate`); `.xlsx` workbook, shared-string and worksheet extraction
(`xlsx`); `G-BULL-1` quarantine of formula errors and hidden sheets; `G-BULL-2` placeholder preservation (`"-"`,
`"N/A"` become `CellValue::Placeholder`, never `0.0`); HTML table extraction (`extract_html_tables`).

`Workbook::cell_census` counts cells by kind over visible sheets and computes the numeric-cell digest: SHA-256 over
`"{sheet}\t{cell_ref}\t{value}\n"` for every numeric cell, sheets in workbook order, cells in `(row, col)` order,
values in `f64` `Display` form. The format is fixed so an independently written reader can reproduce it
(`GATE-BULL-01` part 1).

`build_minimal_xlsx` writes a minimal valid workbook (stored ZIP, inline strings) so tests and the synthetic lane
can use a real filing's layout without shipping any issuer's bytes. `build_minimal_xlsx_styled` (bd-jg43.3) adds a
styles part (`MinimalStyles`: custom formats and `cellXfs`) for `MinimalCell::Styled` cells; with `None` its bytes
are exactly `build_minimal_xlsx`'s (golden test).

`reliance` is the filesystem adapter for the S0 reliance model (BD-347). A journal is one canonical
`fmine_claim::reliance::JournalRecord` frame per lowercase hexadecimal line, ending in LF. Each record's
`previous` field binds BLAKE3 of the complete preceding line, including LF; genesis uses the zero digest.
The S0 codec and transition rules remain the durable-format authority.

DESIGN (BD-047): `forecast` stores S0 `forecast_journal::Record` lines without interpreting their operation
schema. A live append samples an injected `fmine_runtime::WallClock` while holding the exclusive journal lock,
assigns the next sequence and previous head, checks monotone observed time and declared skew bounds, then calls
the consumer's semantic validator. No caller-supplied record timestamp enters this API. Only after successful
validation, complete write and `sync_all` does the committed in-memory snapshot/head advance.

DESIGN (BD-076): `forecast::timestamp` is the shared offline RFC 3161 verifier for the CLI and xtask. It
authenticates a SHA-256 message imprint against an explicitly supplied PEM root bundle using OpenSSL 1.1 or 3.x.
Text decoding, embedded-certificate extraction and signature verification use the same bounded receipt snapshot.
`fmine-runtime::process` owns the scoped pipe workers, bounded stdout/stderr capture, and child cleanup;
the IO layer retains OpenSSL configuration, trust-input fingerprints and typed verification refusals.
The decoded generation time is provisional until verification succeeds; certificate validity is checked at
that signed time with `-attime`, without disabling certificate checks or sampling the host clock.

## 3. Authority, Epistemics & Representation
Every workbook carries the SHA-256 of its bytes; `Workbook::make_anchor` gives a `SourceAnchor`
(`artifact_path`, sha256, `sheet!cell`).

A reliance journal's header registers a release role and SHA-256 commitment to a private, raw 32-byte key.
The key is never written to the journal or exposed by Debug. Possession is a local release capability, not
proof of a human identity, signature, notification or attestation. Every append checks the capability and
explicit `TrustedTime` supplied under the owner's policy. The adapter never infers trusted time from a
system clock, journal timestamp or network service. An independently trusted exact final-head pin is required
for current reuse; an unpinned replay establishes only internal consistency of the inspected bytes.

## 4. Public API Surface
- `IoError`, `HtmlTable`, `extract_html_tables`
- `zip_inflate::{read_zip_entries, inflate_raw, write_stored_zip, ZipError, MAX_ENTRY_BYTES, MAX_ARCHIVE_BYTES}`
- `xlsx::{CellValue, ExtractedCell, QuarantineItem, Sheet, Workbook, CellCensus, FormatCount, MinimalCell,
  MinimalSheet, MinimalStyles, build_minimal_xlsx, build_minimal_xlsx_styled, col_index_to_letters,
  parse_a1_ref}`; `Workbook::format_census`;
  `ExtractedCell::{style, format}` (bd-jg43.1). `Sheet` and `Workbook` implement `PartialEq` over every field
  (bd-r7lp), so two parsed workbooks are equal only when every cell is equal, whatever their `sha256`.
- `styles::{Styles, NumberFormat, BUILTIN_FORMATS, builtin_code, parse_styles, styles_part_name,
  decimal_is_non_integer}` (bd-jg43.1): a cell's `s` resolves through `cellXfs` to a `numFmts` code or an
  ECMA-376 §18.8.30 built-in. Without `s` it uses xf 0, and with no styles part every cell is General. An index
  past `cellXfs` is `BadStyleIndex` and an id neither listed nor built in is `Unknown`; neither is clamped. A
  styles part that does not parse, or an `s` that is not an unsigned integer, refuses the workbook.
- `numfmt::{parse_format, display, FormatSpec, Section, Displayed, DisplayRefusal, Unsupported}` and the refusal
  codes; `ExtractedCell::displayed`, `Workbook::displayed_census`, `DisplayedCensus` (bd-jg43.2). The displayed
  value comes from the stored decimal text by exact digit arithmetic, never `f64`. The section is chosen by the
  stored sign. The value, x100 for `%`, is rounded half away from zero to 15 significant digits and then to the
  section's decimals. Section choice and both roundings are hypotheses that the LibreOffice cross-check decides.
  Every format construct outside the supported subset refuses the code by name, as do General non-integers.
- `numtok::{tokenize, Token, NumberToken, Dec, BasisTok, PeriodTok, UnitTok, Currency, ScaleWord, Hedge, Span}`
  (BD-345.S1): pure lexical inputs for cited disclosure lint. Numbers use exact `i128` mantissa and decimal
  scale, with their own printed step and UTF-8 byte spans into the original text. Comma grouping, accounting
  parentheses, signs, percentages, explicit scale words and ranges retain their printed information. An explicit
  scale word scales the number and step; a unit such as `koz` never converts its value. Percent values remain
  percentage points. `g/t` and `grams per tonne` retain grade values and their printed precision unchanged.
  A range keeps both endpoint precisions. The token span includes its prefix and suffix.
  `$` alone remains an unspecified dollar currency; `US$` explicitly names USD.
- Basis and period tokens are finite lexical matches, not a sentence interpretation. Explicit ownership,
  per-ounce/per-pound sold or produced denominators, credit convention and definition words stay separate.
  `BasisTok` classifies ownership, denominator and credit tokens lexically without supplying a missing basis.
  Only `ownership basis unprinted`
  or `unprinted ownership` supplies the unprinted marker. Quarter/half tokens without a year keep `None`;
  no current year is supplied. Explicit period and ownership metadata do not become extra numerical amounts.
- `reliance::{Journal, ReleaseKey, ReissueRequest, MAX_JOURNAL_BYTES, MAX_JOURNAL_RECORDS}`. `Journal::create` uses
  `create_new`, validates genesis, writes and syncs it, and on Unix syncs its existing parent directory.
  `open_read(path, optional_pin)` retains a shared OS file lock; `open_write(path, expected_head)` retains
  an exclusive lock through replay, expected-head comparison and all subsequent appends. `ledger` and `head`
  expose the committed snapshot. `validate_append` stages the same checks without writing; `append` publishes
  its new head only after the complete line is written and synced. A new head is not automatically marked
  externally pinned. Dropping the handle releases the lock.
- DESIGN (BD-347): `Journal::issue_bytes` verifies the exact artifact digest and validates the owner capability,
  clock and append before creating a new output. It syncs the output and parent directory before committing
  the issuance, retaining its exclusive journal lock throughout. `export_current` reads bounded bytes once,
  verifies their issuance and current-use status, and creates a byte-identical local copy while retaining
  an independently pinned journal lock. Retrieval does not register a new recipient or assert receipt.
- DESIGN (BD-347): `Journal::reissue_current` registers a new generic recipient and purpose for the exact
  `ReissueRequest::source_issuance` identity, not an arbitrary issuance with the same output digest. It requires
  a healthy, independently pinned exclusive writer, reads the artifact once, verifies its selected output
  digest and gates current status before creating a destination or advancing the status cut. It preserves
  every selected producer field, changing only recipient, purpose and issuance time, then uses `issue_bytes`
  for capability/transition validation, private `create_new`, sync and append. Neither a stale status cut nor
  withdrawn/superseded bytes can be refreshed by forwarding. The new record establishes a local registered
  copy; it does not establish delivery or carry forward a human sign-off.
- `ReleaseKey::read` accepts an exact-size, nonzero regular, non-symlink file. On Unix it refuses group/other access;
  on platforms where this private-file policy is unsupported it refuses. `sha256` exposes only its public
  header commitment. The adapter does not generate keys or choose an operator's state directory.
- `forecast::{Journal, MAX_JOURNAL_BYTES, MAX_JOURNAL_RECORDS, MAX_REQUEST_BYTES}`. `create(path, cell_id,
  clock_skew_ns, clock)` uses create-new semantics, a private Unix mode, exclusive lock, runtime-stamped genesis
  and file/parent-directory sync. `open_read(path, optional_head)` retains a shared lock; `open_write(path,
  expected_head)` retains an exclusive lock through complete replay, final-head comparison and append.
  `records`, `header`, `head` and `is_pinned` expose the committed snapshot. `append(request, clock, validate)`
  invokes the validation closure before writing; its caller replays `ForecastLedger` semantics on staged state.
  A successful append clears the old pin status; the newly returned head must be retained independently.
- `forecast::timestamp::{verify_rfc3161, verify_rfc3161_with_intermediates, VerifiedTimestamp,
  proof_times_from_text}`. Verification takes
  receipt and trust-root paths plus the expected digest, returning `fmine_calib::record::ProofTimes` and
  SHA-256 fingerprints of both input files. The text parser is exposed for known-answer tests and performs
  no authentication by itself. Each stated accuracy component rounds upward to whole seconds; a nonzero
  fractional generation time widens the upper endpoint by one second. Unspecified components add zero,
  preserving the existing frozen-record verifier's rule.
  The intermediate-aware entry point combines the receipt's embedded certificates with an optional bounded
  PEM file, passed only as `-untrusted`. An exclusively created private temporary input supplies that chain
  while the receipt stays on stdin; its owned files are cleaned up after use. These certificates can complete
  a chain but never become additional trust anchors. Supplied intermediate fingerprints are checked before
  and after verification as for the explicitly selected root bundle.

## 5. Guards & Typed Refusals
- `G-BULL-1`: formula-error cells and `hidden`/`veryHidden` sheets go to `Workbook::quarantined`. Hidden sheets are
  parsed into `Workbook::hidden_sheets` (BD-331.S2, for the hidden scope), never into `sheets`, `sheet()`,
  `cell_census` or the numeric digest; their formula errors are not logged.
- `G-BULL-2`: placeholders are typed, never zero. A placeholder keeps its token exactly as stored, surrounding
  whitespace included (`"— "` and `"—"` stay distinct); a number keeps its stored text in
  `ExtractedCell::number_text`.
- `Workbook::from_file` on a missing or unreadable path returns `IoError::FileRead` naming the path. There is no
  fallback content of any kind (`BD-321`).
- Hostile archives (BD-073). `read_zip_entries` refuses, before allocating, an entry declaring more than
  `MAX_ENTRY_BYTES` (64 MiB) uncompressed or an archive declaring more than `MAX_ARCHIVE_BYTES` (256 MiB):
  `ZipError::Limit`. Inflation stops at the declared size, and an entry that inflates to any other length is refused,
  so an 840-to-1 stream declared at 1,000 bytes never grows past 1,000. A stored entry whose two sizes differ, and a
  name that appears twice, are `ZipError::InvalidArchive`: a spreadsheet program and this reader cannot take
  different parts under one name. The registered workbooks peak at 0.4 MB an entry. CRC-32 is not checked; a
  registered file's integrity is its SHA-256 against the source registry.
- XML text decodes the five named entities and `&#10;`, `&#13;`, `&#160;`, each once (`&amp;` last); no DTD or
  entity declaration is read, so nothing expands.
- `numtok` keeps malformed grouping, unsupported attached notation, decimal overflow and ambiguous standalone
  dash signs as `UnsupportedNumber`, allowing the consumer to remain unchecked. It never rounds an overflowing
  input into range. Trailing zeros retain the finest printed step; hedge words are qualifiers and do not invent
  a wider tolerance. Claim IDs embedded in words are not quantities. Unrecognised prose remains for the caller's
  supported-predicate check; tokenisation alone never produces a verification verdict.
- Reliance reads are bounded to 64 MiB and 100,000 records; each decoded frame is bounded by the S0 4 MiB
  limit. Empty, odd-length, uppercase or overlong hex lines, incomplete final lines, invalid frames, illegal
  transitions and wrong final-head pins refuse. An ancestor pin never accepts a newer untrusted suffix.
  Journal integrity/head/locking failures use `FMINE-REF-502`; unreadable paths use `FMINE-REF-506`;
  missing release authority or unsupported private-key protection uses `FMINE-REF-304`.
- No repair, truncation, replacement or fallback journal is attempted. A failed write or sync poisons its
  writer handle and retains all bytes for inspection. OS locks coordinate cooperating local writers and
  readers; filesystem access controls and the external pin remain necessary against file replacement.
- Forecast journals enforce the same 64 MiB and 100,000-record storage limits with a 64 KiB operation limit.
  The S0 codec rejects corrupt/truncated/noncanonical chains. Runtime clock failures, rollback, wrong final heads,
  lock conflicts and changed file lengths refuse with `502`; unavailable paths use `506`, malformed fields `107`.
  A validator refusal preserves its original code and leaves disk bytes, sequence and committed head unchanged.
  A failed write/sync poisons the handle; no retry or repair is attempted. Creation failures may leave an
  uninitialized path, which is retained for inspection rather than overwritten by a second create.
- Timestamp inputs must be nonempty regular, non-symlink files of at most 4 MiB. Duplicate time/accuracy
  fields, malformed fractions, invalid subsecond accuracy components, overflow, wrong imprint algorithm,
  failed signature/chain checks and changed root fingerprints refuse. Missing/unreadable inputs use `506`,
  unavailable OpenSSL uses `202`, and invalid proofs use `505`. OpenSSL runs with `LC_ALL=C`, an empty local
  configuration and explicit `-CAfile`; `SSL_CERT_FILE` names that same file and `SSL_CERT_DIR`
  names the non-directory file, preventing an ambient hashed trust directory. No network store is selected.
  Both the original and resolved root paths are fingerprinted again before a verified result is returned.

## 6. Proof Obligations (`PO-*`)
- `PO-IO-1`: on the 18 registered Barrick workbooks, the per-workbook census (visible and hidden sheets; numeric,
  placeholder, error and text cells) and numeric digest equal an independent reader's (MOR_CASE E820; asserted in
  `fmine-public`'s ignored real-file test).
- `PO-IO-2`: writer-to-parser round trip on a synthetic workbook reproduces typed cells, quarantine, census and a
  digest computed from the fixed listing format (unit test).
- `PO-IO-3` (BD-345.S1): synthetic known answers check the documented decimal/step examples, accounting negatives,
  range endpoint precision, each basis/period class, explicit currency and unit distinctions, UTF-8 spans and
  malformed/overflowing numbers. Seeded Philox text checks totality and non-overlapping character-boundary spans.
- `PO-IO-4` (BD-347): synthetic filesystem tests preserve every previously committed byte through issuance
  and withdrawal, check exact line heads, reject wrong keys before append, verify shared/exclusive lock
  exclusion and expected-head CAS under concurrent writers, refuse corrupted/torn/reordered/replayed journals,
  retain committed state after an I/O failure, and refuse insecure or malformed key files. These tests exercise
  local filesystem behavior; they do not establish partner receipt or hardware power-loss durability.
- FACT (BD-347, `reliance::tests`): exact artifact issuance and current-use retrieval preserve bytes and
  refuse after withdrawal; invalid digest, key, time, stale or absent trust, and an existing destination
  cannot create an authorized output. A failed final append retains the unissued artifact and poisons its
  writer. Artifact reads refuse special files and inputs over the separate 64 MiB bound.
- `reliance::tests` checks that forwarding selects the requested producer when different revisions emitted
  identical bytes, preserves its full provenance and adds only that revision's new recipient to recall.
  Missing/wrong receipt identity, pin, writer authority, key, bounded clock, current status, exact bytes or
  an available destination cannot create a new forwarding record or alter the previous head.
- BD-047 direct filesystem tests reopen exact operations and pinned heads, preserve prefixes across appends,
  exercise shared/exclusive lock contention, reject stale pins, corrupt/truncated/rolled-back files and clock
  rollback, and prove validator failures do not write. A real read-only descriptor triggers append failure and
  verifies poisoning while retaining the previous committed snapshot. Oversized and special files refuse.
- Timestamp parser tests retain exact whole/fractional-second and per-component rounding behavior, refusing
  ambiguous, malformed and overflowing fields. The separately invoked OpenSSL test checks the committed
  DigiCert rules receipt against its expected digest and selected root, and refuses a changed digest or
  unrelated FreeTSA root, including when the correct root is supplied only as an untrusted certificate.
  It is an offline integration test, not evidence that caller-chosen roots are trusted.

## 7. Failure Modes & Deterministic Fallbacks
Missing files, corrupt ZIP or DEFLATE streams and malformed workbooks fail closed with `IoError`. No fallbacks.
Reliance failures return registered `Refusal`s. A caller issuing a new artifact holds the writer lock, validates
its entry first, creates and syncs the exact artifact with `create_new`, then appends the issuance. If that final
append fails, a previously unissued digest cannot pass the current-use gate. Failed forwarding of an already
issued digest adds no recipient record or successful issuance receipt; its existing status remains unchanged.
A current-use export keeps its
pinned shared journal handle alive through its exact-byte copy; checking and dropping the handle before copying
would permit a concurrent withdrawal race.

## 8. Gates & Kill Criteria (`GATE-*` / `KILL-*`)
- `GATE-BULL-01` (part 1, extraction fidelity)

## 9. Lifted Code & Foundation Dependencies
- Depends on `fmine-core`, `fmine-evidence`, `fmine-claim` (V to S0), and `fmine-runtime` (V to S1, explicit clock
  and owned bounded subprocesses).
- `fmine-calib` supplies the shared `ProofTimes` and UTC parser (V to K0); timestamp verification invokes
  OpenSSL 1.1 or 3.x on `PATH` with Unix `/dev/stdin` and `/dev/null`. Unsupported installations refuse.

## 10. Non-Goals & Scope Boundary
Reads cells and adapts the reliance model to local files; does not interpret tables (`fmine-public`) or compute
metrics (`fmine-metrics`).
The draft tokenizer recognises a bounded ASCII-number and lexical-context grammar, not arbitrary mathematical
notation or prose semantics. It makes no publicness, calibration or legal conclusion. The two-reader issuer
sentence corpus and its precision/recall study remain separate work; synthetic lexer tests do not stand in for it.
The reliance file is a bounded local seam until the authority ledger is available. It is not a replicated
authority service, authentication signature, clock synchronisation service, key manager or notification system.
It cannot establish that all detached copies were recalled; it records only known issuances.
The forecast store provides local durable ordering, not external time authenticity, replication or a substitute
for forecast semantic replay. Its declared host-clock bound is an operator assumption. Advisory locks coordinate
cooperating processes; independently retained pins detect a changed final head but do not authenticate the writer.
The separate timestamp adapter authenticates the TSA's assertion under the caller's selected roots. It does
not select or endorse a TSA, fetch revocation information, perform network verification or establish that a
local test authority is an independent time witness. Its input fingerprints permit the trust choice to be audited.
