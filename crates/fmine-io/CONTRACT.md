# CONTRACT.md — `fmine-io`

## 1. Charter & Layer
- **Crate:** `fmine-io`
- **Layer:** `V` (Verification, Ingestion & Assurance)
- **Subsystem:** `Ingestion`
- **LOC Budget:** `40k planned / 46k ceiling` (`registries/loc_budget.toml`)
- **Allowed Module Families:** `["xlsx", "zip_inflate", "styles", "numfmt", "numtok", "reliance", "forecast", "omf", "minelib_report"]`

## 2. Mathematical & Domain Formulation
Pure-Rust ZIP and RFC 1951 DEFLATE (`zip_inflate`); `.xlsx` workbook, shared-string and worksheet extraction
(`xlsx`); `G-BULL-1` quarantine of formula errors and hidden sheets; `G-BULL-2` placeholder preservation (`"-"`,
`"N/A"` become `CellValue::Placeholder`, never `0.0`); byte-bound HTML table extraction (`HtmlDocument`).

`Workbook::cell_census` counts cells by kind over visible sheets and computes the numeric-cell digest: SHA-256 over
`"{sheet}\t{cell_ref}\t{value}\n"` for every numeric cell, sheets in workbook order, cells in `(row, col)` order,
values in `f64` `Display` form. The format is fixed so an independently written reader can reproduce it
(`GATE-BULL-01` part 1).

`build_minimal_xlsx` writes a minimal valid workbook (stored ZIP, inline strings) so tests and the synthetic lane
can use a real filing's layout without shipping any issuer's bytes. `build_minimal_xlsx_styled` (bd-jg43.3) adds a
styles part (`MinimalStyles`: custom formats and `cellXfs`) for `MinimalCell::Styled` cells; with `None` its bytes
are exactly `build_minimal_xlsx`'s (golden test).

`omf` (BD-194) reads Open Mining Format 2 files: block models with regular, octree, full or free-form sub-blocks,
surfaces, line sets, point sets, grid surfaces and composites, with every attribute type. An OMF 2 file is a ZIP of
stored members whose comment names the format; its index is gzipped JSON, each array is a Parquet file and each
image a PNG or JPEG. omf-rust needs the `parquet` crate and imports `chrono`, both banned (L-5, E858), so the
reader is its own: `omf::json` (RFC 8259, numbers kept as written), `omf::gzip` (RFC 1952 over `inflate_raw`) and
`omf::parquet`, the subset omf-rust writes. That subset is a Thrift compact footer; a flat schema or one level of
groups; definition levels 0 or 1; BOOLEAN, INT32, INT64, FLOAT, DOUBLE and BYTE_ARRAY; the string, date, UTC
microsecond timestamp and integer logical types; v1 and v2 data pages; PLAIN values; RLE/bit-packed levels; and
no codec or GZIP. `read` decodes every array the index names and returns typed values: 32-bit floats are widened
exactly, nulls are `None`, dates are days and date-times microseconds since 1970. Every check the vendor reader
makes when reading, and every error its validator raises, refuses the file (§5).

`omf::proportions_in_surface` (bd-aqfl, plan §13.9) gives each parent block of a BlockModel element its proportion
inside a closed Surface element, as `fmine_geom::volcert` sure and band sub-cell counts at a chosen depth. Elements
are found by name, at the top level or inside composites. A surface's positions are its element origin plus its
vertices; a block model's grid starts at its frame origin. The project origin, which both share, is left out: it
changes no proportion. `surface_mesh` certifies the surface closed and outward (`ClosedMesh::certify`), and
`parent_grid` takes a regular grid (`BlockGrid::new`). Sub-blocks are not considered.
- Proportions are computed in the block model's own frame (bd-wn57), so models turned, tilted or mirrored are
  taken. `parent_grid` places the blocks from zero along u, v and w. `surface_mesh_in` maps the surface there:
  `M⁻¹(p − origin)`, with `M`'s columns the axes and the inverse the adjugate over the determinant. For axes x, y and
  z that is one subtraction. Otherwise each coordinate is rounded a few times more, and the certificate is for the
  surface so placed. A mirrored frame reverses each triangle so the surface stays outward.

`omf::pit::PitModel` (bd-joiv, plan §15.4) turns a regular OMF block model into `fmine_optimize::pitflow`'s
problem: precedence arcs from slope templates (`slope_arcs`), with each block's slope domain taken from a Category
attribute, then a certified ultimate pit (`solve`) or nested shells by revenue factor (`nested_shells`).
- Values come from Number attributes on the parent blocks (`Primitives`). Integer attributes scale exactly;
  float attributes use binary64 multiplication by `per_unit` before nearest-integer rounding, halves away from
  zero. The certificate proves optimality for these integer weights; no raw-float optimality bound is supplied.
  `value_attribute` admits the unique model, attribute, parent-block location and numeric type before a caller
  allocates per-block slope domains or arcs; `PitModel::weights` checks nulls, finiteness and scaled range.
- OMF orders parent blocks U first, then V, then W upward (omf-rust `block_model.rs`); `pitflow` puts layer 0 on
  top. So the layers are reversed on the way in, and `omf_order` reverses them back.
- W must be (0, 0, 1) with V counterclockwise of U. A model turned about W is taken, and its slope sectors are then
  measured from U toward V.
- Block sizes are scaled by the smallest power of two up to 2^32 that makes them integers below 2^32. This is exact;
  the cone test is homogeneous in size.
- `PitModel::schedule` (bd-fr8x) builds `toposort`'s `ScheduleProblem` from the values, the arcs and capacities
  whose per-block uses are Number attributes, rounded up so that a schedule within the scaled limits is within the
  true ones. It places blocks in the caller's rank order with TopoSort (`greedy`) and improves the schedule by local
  search at the discount rate (`improve`); both refuse a schedule that fails `toposort::check`. The result is
  feasible and locally improved, not proved best.
- DESIGN: `omf::scheduling::evaluate` reads a strict `fmine.omf-schedule.v1` JSON declaration and exact
  OMF bytes, checks both source hashes and composes the existing schedule/candidate kernels. It retains
  the empty schedule and only feasible incumbents, checks the selected schedule again, and replays the
  integer Lagrangian bound at its reported prices. Objective, bound and gap use the same floored `2^40`
  discount factors. Capacity limits are explicitly scaled integers; uses round up, values nearest with
  halves away from zero. Original candidate findings and OMF block order survive rendering. Bounded
  periods, resources and time-expanded nodes are admitted before slope allocation. The retained-arc
  allowance is passed into `PitModel::new_bounded`; each CSR append and target-buffer reservation checks
  that allowance before growth. This admits reduced graphs that a full rectangular-cone estimate would
  reject, without allowing excess allocation. Independent archive, schedule and price-witness controls
  are in `xtask/tests/omf_schedule_cli.rs`. This is a
  deterministic declared-model certificate, with no LP-guidance, multi-destination or operational claim.
  `omf::pit_report` similarly owns the source-bound ultimate-pit report used by the CLI.
- DESIGN: `omf::shells_report::evaluate` binds an OMF archive and strict `fmine.omf-shells.v1`
  declaration, admits matching nonempty revenue/cost unit labels, and runs `PitModel::shells` for
  1..64 strictly increasing exact positive rational factors. No unit conversion or factor sorting is
  inferred. Scaled revenue is nonnegative; costs may be signed. Each objective is
  `sum(num * scaled_revenue - den * scaled_cost) / (per_unit * den)`. The existing solver verifies
  each flow, then checks that the running union is closed and preserves that factor's optimum.
  The report retains both source hashes, per-factor counts and exact objectives; optional `first_shell`
  uses original OMF order with null for never-mined blocks. The shared admission guard bounds
  blocks times factors at one million and `(retained_arcs + blocks) * factors` at eight million;
  `PitModel::new_bounded` enforces the retained-arc allowance before CSR appends and reservations.
  Tied shells need not be minimal or maximal; flow witnesses are checked internally, not exported.
  No raw-float optimum, geotechnical approval, designed pushback or operational claim is supplied.
- `minelib_report::evaluate` owns the existing MineLib UPIT report and optional candidate comparison.
  All source hashes are rechecked; final flow verification, exact decimal rounding bounds, candidate
  closure witnesses and optimum gaps retain the existing CLI payload semantics.
- `PitModel::lp_schedule` (bd-djb7) takes its starts from an LP instead of the caller's ranks. It builds the same
  `ScheduleProblem` and its relaxation (`toposort::lp::relaxation` at decimals 0, so the LP is in units of
  `1 / per_unit`, discounted by `(1 + rate)^-t` in `f64`), solves it with fmine-lp's PDLP, then searches from
  `toposort::lp`'s six TopoSort starts and keeps the best (`starts`, `incumbent`). `LpSchedule::upper_bound` is
  minus PDLP's certified dual bound: no schedule of the scaled problem is worth more at those factors, so the
  schedule's gap to the best possible is known, though not closed.
- Outside solvers (bd-06kg, plan §15.4 `UntrustedCandidate`). `PitModel::schedule_lp` hands out the same relaxation
  as an `LpProblem`. Column `t·n + b` is block `b`'s share, in `pitflow`'s order, mined by period `t`.
  `fmine_lp::mps::write` gives it as MPS, and an outside solver's solution comes back under `write`'s names through
  `fmine_lp::mps::read_values`. `guided_schedule` runs `lp_schedule`'s starts and search from any shares `x`.
  `schedule_bound` is minus `certified_bound` of any row multipliers `y`. Neither trusts the solver. `x` only ranks
  and releases blocks, and TopoSort and the search refuse a schedule that fails `check`. Every `y` bounds by weak
  duality, so a poor one only bounds loosely.
- Candidates (bd-jfg4, plan §15.4 `UntrustedCandidate`). A client's own schedule, such as the mining period its
  planning tool exported as a Number attribute on the parent blocks, comes in through `PitModel::periods`. The value
  `first` is period 0, and a null leaves a block out. `PitModel::candidate_schedule` checks it with
  `toposort::check`: the requirements and limits it breaks, with witnesses (`omf_index` names them in OMF's
  numbering), and the exact value it mines in each period, with its discounted value. A candidate that breaks
  nothing is improved by `improve`; one that breaks something is reported, not repaired. Uses are rounded up as for
  any schedule. So a candidate within the scaled limits is within the true ones. With uses from floats, one over a
  scaled limit by less than the number of blocks it mines that period may still be within the true limit.
- Designs (bd-hq5y, plan §15.5 shell-to-design loss). An engineer's designed pit, as a closed Surface in the same
  project, comes in through `PitModel::design`, which uses `proportions`' frame and `volcert::block_proportions`.
  Each parent block has between `inside` and `inside + band` of `8^depth` sub-cells inside the design.
  `Design::against` compares the design with any shell (one flag per block, such as `PitSolution::in_pit`) for any
  per-block quantity in integers (`weights`: value, tonnes or metal). It gives four totals, each an exact integer
  enclosure in units of a weight over `8^depth`:
  - the shell's total;
  - the design's total;
  - `left`, the part of the shell the design leaves;
  - `added`, the part of the design outside the shell.
  The loss is `shell − design`, which is `left − added`. For a positive weight the least is `inside` sub-cells and
  the most `inside + band`, and the other way round for a negative one. So the true total, with the design as
  placed and each block's quantity spread evenly through it, lies within the enclosure. A design reaching outside
  the block model's blocks is refused, since nothing values that part. The design is not checked against the
  slopes, so its loss can be negative when it breaks them.

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

DESIGN (BD-068): `forecast::prefix::{PrefixCommitment, commitment, verify, VerifiedPrefix}` binds a retained
FJR1 prefix to SHA-256 of the exact ASCII frame `fmine.forecast-journal-prefix.v1\nhead=<head>\nsequence=<seq>\n`.
The head is 64 lowercase BLAKE3 hex digits; the sequence is unpadded unsigned decimal, derived from the matching
validated record. `PrefixCommitment::for_record` can prepare a receipt before sync but establishes no journal
membership or durability. `commitment` and `verify` require a healthy retained snapshot and refuse an absent
prefix with `FMINE-REF-502`. Verification delegates to the existing RFC 3161 verifier under caller-selected
roots and returns its conservative time bounds and receipt/root fingerprints. It neither authenticates the
original host-clock times nor asserts forecast preregistration: the caller must check exact cohort/member
definitions, absence of selected outcomes in the prefix, and all applicable deadline/availability bounds.

## 3. Authority, Epistemics & Representation
Every workbook carries the SHA-256 of its bytes; `Workbook::make_anchor` gives a `SourceAnchor`
(`artifact_path`, sha256, `sheet!cell`).

DESIGN (BD-057, BD-059): `HtmlDocument::from_bytes` binds the complete original UTF-8 byte snapshot to its
SHA-256 and artifact path. Immutable tables retain each source cell once, its original inner HTML and byte
range, decoded text, exact numeric token and printed step when supported, and its row/column span. Logical
covered slots refer to the same origin instead of creating repeated observations. `make_anchor` uses
`Table1!B3`-style locators and returns that origin's anchor even when queried through a covered slot.
Table indices are zero-based in the API; row and column coordinates are one-based.

A reliance journal's header registers a release role and SHA-256 commitment to a private, raw 32-byte key.
The key is never written to the journal or exposed by Debug. Possession is a local release capability, not
proof of a human identity, signature, notification or attestation. Every append checks the capability and
explicit `TrustedTime` supplied under the owner's policy. The adapter never infers trusted time from a
system clock, journal timestamp or network service. An independently trusted exact final-head pin is required
for current reuse; an unpinned replay establishes only internal consistency of the inspected bytes.

## 4. Public API Surface
- `IoError`, `HtmlTable`, `extract_html_tables`
- `HtmlDocument::{from_bytes, artifact_path, bytes, sha256, tables, make_anchor}`;
  `HtmlSourceTable::{table_index, rows, columns, cells, cell}`;
  `HtmlSourceCell::{row, column, row_span, column_span, byte_range, raw_html, text, value, number}`.
  The rich types expose no mutable fields or mutable cell access. A numeric cell retains `numtok::NumberToken`
  with its exact `Dec`, printed precision and lexeme; `CellValue::Number` remains only a floating-point
  compatibility projection. Only a single full-cell bare number is promoted; explicit currencies, units,
  percentages, scale words, annotations and split text runs remain text for a semantic adapter. Token spans index decoded cell text; `byte_range` indexes the original document.
  `HTML_MAX_BYTES` (8 MiB), `HTML_MAX_CELLS` (100,000 origins) and `HTML_MAX_SLOTS` (1,000,000 expanded grid slots)
  bound document ingestion. The slot budget is global and includes padding.
- `zip_inflate::{read_zip_entries, write_stored_zip, ZipError, MAX_ENTRY_BYTES, MAX_ARCHIVE_BYTES}`. The inflater,
  CRC-32, stored-member reader and archive comment are crate-private, for `omf`.
- `omf::{read, read_path, Project, Element, Geometry, Orient2, Orient3, Grid2, Grid3, BlockModel, Subblocks,
  SubblockMode, RegularSubblock, FreeformSubblock, Location, Attribute, AttributeData, Numbers, Vectors,
  Boundaries, Colormap, MAX_MEMBER_BYTES, MAX_ARCHIVE_BYTES, MAX_INDEX_BYTES}` and `omf::json::{parse, Json,
  MAX_DEPTH}` (BD-194). `Project` holds every element with its arrays decoded; metadata and a continuous
  colormap's range stay `Json` as written. `Grid3::count` and `blocks`, `Subblocks::len` and
  `AttributeData::item_count` give the counts the location lengths follow from.
- `omf::{surface_mesh, parent_grid, proportions_in_surface}` (bd-aqfl) and `omf::surface_mesh_in` (bd-wn57),
  returning `fmine_geom::volcert`'s `ClosedMesh`, `BlockGrid` and `Proportions`.
- `omf::pit::{PitModel, value_attribute, MAX_SIZE_SCALE}` (bd-joiv). `PitModel::{new, new_bounded, grid, size_scale, arc_count, weights,
  ultimate_pit, shells, omf_order}` return `fmine_optimize::pitflow`'s `BlockGrid`, `PitSolution` and
  `NestedShells`.
- `omf::pit::{Capacity, ScheduleSpec, OmfSchedule}` and `PitModel::{uses, schedule}` (bd-fr8x).
  `OmfSchedule::{problem, factors, improvement}` give `toposort`'s `ScheduleProblem`, the discount factors and
  `search::Improvement`.
- `omf::pit::LpSchedule` and `PitModel::lp_schedule` (bd-djb7). `LpSchedule::{problem, factors, lp, upper_bound,
  incumbent}` give the `ScheduleProblem`, the discount factors, fmine-lp's `PdlpSolution`, minus its dual bound,
  and `toposort::lp::Incumbent`.
- `PitModel::{schedule_lp, guided_schedule, schedule_bound}` and `omf::pit::GuidedSchedule` (bd-06kg).
  `GuidedSchedule::{problem, factors, incumbent}` give the same as `LpSchedule`'s.
- `PitModel::{periods, candidate_schedule, omf_index}` and `omf::pit::CandidateSchedule` (bd-jfg4).
  `CandidateSchedule::{problem, factors, schedule, check, value, improvement}` give the `ScheduleProblem`, the
  discount factors, the candidate in `pitflow`'s order, `toposort::ScheduleCheck`, its discounted value, and
  `search::Improvement` when it is feasible.
- `PitModel::design` and `omf::pit::{Design, ShellToDesign}` (bd-hq5y). `Design::{cells_per_block, cells, total,
  against}`. `ShellToDesign`'s public fields are `shell`, `design`, `left`, `added` and `loss`.
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
- `numtok::equality::{Context, Metric, context}` is the shared bounded one-number equality grammar for
  cited checks and derived-value queries. Callers supply known subjects and citation spans; the parser
  retains the same explicit metric, period, ownership and unit requirements and refuses residual prose.
  It creates no Claim, selects no source vintage and supplies no missing semantic or evidence metadata.
- Basis and period tokens are finite lexical matches, not a sentence interpretation. Explicit ownership,
  per-ounce/per-pound sold or produced denominators, credit convention and definition words stay separate.
  `BasisTok` classifies ownership, denominator and credit tokens lexically without supplying a missing basis.
  Only `ownership basis unprinted`
  or `unprinted ownership` supplies the unprinted marker. Quarter/half and spelled nine-month tokens without a year keep `None`;
  no current year is supplied. Explicit period and ownership metadata do not become extra numerical amounts.
  Nine-month abbreviations require an explicit year (`9M 2030`, `9M2030` or `20309M`); an undated `9M`
  retains its existing meaning as the numeric amount nine million. `nine months of 2030` and
  `first nine months of 2030` identify the same cumulative calendar support, never Q3 or a full year.
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
- OMF files (BD-194) refuse with `FMINE-REF-506` and a detail beginning `OMF: ` that names the element and attribute
  path. A member is at most `MAX_MEMBER_BYTES` (1 GiB), the members together `MAX_ARCHIVE_BYTES` (2 GiB) and the
  inflated index `MAX_INDEX_BYTES` (64 MiB); Parquet pages, decoded bytes and rows are bounded too, and JSON
  nests at most 128 deep. Refused:
  - a comment other than "Open Mining Format 2.x"; a compressed member; a member whose CRC-32 is wrong; no index;
  - gzip with a bad header, CRC-32 or size; JSON with duplicate keys, a bad number, escape or surrogate, or trailing
    text;
  - Parquet outside the subset: dictionary pages, other codecs, repetition, deeper nesting, encryption, INT96 and
    fixed-length types, or a page CRC that does not match;
  - an array whose columns, types or nullability are not those OMF gives it, or whose length is not the index's
    `item_count`; a member the index names that is absent;
  - the vendor reader's checks: category indices below the names count, segment and triangle indices below the
    vertex count, sizes above zero, boundaries that do not decrease, sub-block parents inside the grid, corners
    with `min < max <= count` (free-form in [0, 1]), and sub-block sizes of the mode, octree ones aligned;
  - the vendor validator's errors: finite origins and sizes, unit axes, orthogonal axes, counts above zero, octree
    counts that are powers of two, an attribute at a location its element does not have, a length other than the
    location's, and a projected texture anywhere but `Projected`.
  The vendor validator compares u with v three times (omf-rust 64d3057); this reader checks all three pairs, and
  refuses a NaN size or boundary, which the vendor lets through. Its warnings are not checked: two elements may
  share a name. Images are checked by signature only, and members the index never names are not read.
- OMF block proportions (bd-aqfl) refuse with `FMINE-REF-506` a name that no element, or more than one, carries
  (composites searched too), and an element of the wrong kind; with `FMINE-REF-109` a tensor grid, or axes whose
  determinant is zero or not finite (the reader already holds axes to unit length and right angles within 1e-6,
  so only a hand-built `Project` reaches it); then with `ClosedMesh::certify`'s, `BlockGrid::new`'s and
  `block_proportions`' refusals (`109`, `207`), among them a surface that is not closed and outward.
- OMF pits (bd-joiv) refuse with `FMINE-REF-506`:
  - a block model name that no element, or more than one, carries, or an element of another kind;
  - a value or domain attribute that is missing or ambiguous, is not on the parent blocks, is the wrong type
    (values are integer or float Numbers, domains are Categories), or has a null.
- They refuse with `FMINE-REF-109`:
  - a tensor grid, a W that is not straight up, or a mirrored frame;
  - sizes no power of two up to 2^32 makes integers;
  - `per_unit` 0, a value that is not finite, or a weight outside `i64`;
  - a category above 65,535.
- Then `pitflow`'s own refusals (`207`) apply: among them a domain the slope table lacks, a negative revenue for
  shells, and the value-sum guard.
- Schedules (bd-fr8x) refuse a capacity attribute as values are refused. `toposort`'s refusals (`207`) cover limits
  that are not one per period or are negative, a rank that is not one per block, a negative discount rate, and a
  schedule that fails `check`.
- LP schedules (bd-djb7) refuse as schedules do, then pass on the refusals of `relaxation`, `LpProblem::new`,
  PDLP's `solve` (among them options out of range), `Shares::new`, `starts` and `incumbent`.
- Outside solutions (bd-06kg): `guided_schedule` refuses through `Shares::new` (`FMINE-REF-207`) a share outside
  `[0, 1]`, even by one ulp, rather than clamp it, and a count that is not one per block and period.
  `schedule_bound` passes on `certified_bound`'s refusal of a `y` that is not one finite value per row.
- Candidates (bd-jfg4): `periods` refuses an attribute as values are refused (`506`), but a null is a block left
  out. It refuses (`109`), naming the block, a value that is not a whole number (`2.5`, NaN, infinity), or lies
  below `first` or more than `u32::MAX` above it. It never rounds or clamps. `candidate_schedule` refuses as
  schedules do (`207`), and refuses a candidate that is not one entry per parent block or puts a block past the last
  period, naming the block. Breaking a requirement or a limit is not a refusal: `check` reports it.
- Designs (bd-hq5y): `design` passes on the refusals of `surface_mesh_in` (`506` for a missing element or one that
  is not a Surface, and `ClosedMesh::certify`'s for a surface that is not closed) and of `block_proportions`
  (`109` for a depth above 6). It refuses (`109`) a design whose bounding box leaves `[0, size · count]` on any
  axis of the block model's frame. In a turned frame, rounding may refuse a design that only touches the model's
  side. `against` and `total` refuse (`207`) a shell or weights that are not one per block.
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

- HTML requires an explicit, well-formed table subset. Malformed, ambiguous or unsupported table structure
  returns a typed refusal without returning partial numeric tables. Row and column spans retain their origins;
  overlapping spans, spans crossing row groups, and expansion beyond the bounded grid refuse. Comments and
  script content cannot create tables, and attribute quotes are respected while scanning tags.
- HTML numeric recognition never removes arbitrary separators or joins adjacent text runs into a new number.
  Footnote-marked or unsupported tokens remain text. Placeholders retain their printed text and never become
  zero. Entity decoding occurs once; decoded text is never reparsed as markup.
- The legacy `extract_html_tables` projection retains its original signature, emits each merged origin once,
  and leaves covered positions empty. On a refused document it returns an empty vector; consumers requiring
  diagnostics and source evidence must use the fallible `HtmlDocument::from_bytes` API.

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
- `PO-IO-5` (BD-073, threat T1): `tests/reader_mutations.rs` mutates two synthetic workbooks, one stored and one
  deflated with dynamic-Huffman, fixed-Huffman and stored blocks, under fixed Philox keys: 4,000 mutants each,
  split between byte edits to the archive, edits to one entry re-zipped intact, and overwrites inside the deflated
  payloads. Through `read_zip_entries`, `Workbook::from_bytes`, the three censuses and every displayed text, no
  mutant panics and an accepted archive stays within both bounds. Each seed must be refused and accepted at least
  one time in twenty and reach display refusals, and the deflated seed's inflater refusals meet the same floor, so
  the search is not vacuous. This is a bounded random search, not a proof and not a coverage-guided fuzzer. The
  coverage-guided targets for the same code are `fuzz/fuzz_targets/xlsx.rs` and `numfmt.rs` (`scripts/fuzz.sh`).
- `PO-IO-6` (BD-194): `omf::tests` builds OMF files member by member, with Parquet from the crate's own test
  writer (`omf::parquet::testkit`, v1 and v2 pages, GZIP and none, split pages and row groups, both level
  encodings). One file holding every geometry and attribute type reads back equal to the values written, and each
  refusal of §5 is reached by a file with that one defect. The test writer shares its author with the reader, so
  `tests/omf_samples.rs` reads the files omf-rust publishes (tree 64d3057, MIT), written by parquet-rs. The files
  are not committed; it is `#[ignore]`d and reads them from `FMINE_OMF_DIR`. On 2026-11-03 its five tests passed:
  - `one_of_everything.omf`: every element, attribute, value and null equals the vendor test's constants, and
    omf-python's copy of it equals it element for element;
  - pyramid and bunny: the vendor examples' values, and the bunny's 69,634 vertices and 69,630 triangles read;
  - the octree bunny: each of 5,123 sub-blocks gives its source CSV's centroid within 9.1e-13 m, and its size and
    category exactly; the 1,774 body centroids, computed in the vendor's order of operations, equal the vendor
    reader's output bit for bit;
  - omf-python's test files: nullable numbers, dates, date-times, categories, booleans and colors, continuous
    colormap ranges, element metadata and a duplicate name read as generated; `missing_parquet` and
    `array_length_mismatch` are refused, as the vendor refuses them.
  - The bunny surface against the octree bunny (bd-wn57, `omf::proportions_in_surface` at depth 3, in the model's
    frame turned 10° about Z).
    - The surface certifies closed. Its volume encloses 6,039,237,580.3241 m³ in both the model's frame and the
      world's.
    - Of 384 parent blocks, 170 are certified wholly outside and hold no Body sub-block, 23 wholly inside and hold
      no Air, and 191 are crossed.
    - The vendor's Body sub-blocks, a classification independent of `volcert` whose rule is not published, total
      6.044e9 m³ (0.08% above the volume). Every parent's Body share lies within its certified bounds. That was
      observed on the first run and is kept as a regression check, not derived.
  Two bugs planted in turn (nulls inverted, octree alignment unchecked) each failed the unit tests, and the first
  failed three of the four sample tests; both were reverted. `mutated_files_are_read_or_refused_and_never_panic`
  edits one member of the every-kind file at a time, the index as JSON text and other members as bytes, rezips it
  with true CRC-32s and reads it: 1,000 mutants in each of three page forms (GZIP v2, and v1 and v2 without a
  codec). None panicked and every refusal was `FMINE-REF-506`; the forms read 36, 40 and 55 and refused the rest,
  over a floor of one in fifty each way. Its floor was one in twenty until the GZIP form read 50 of 1,500, because
  the gzip CRC-32 refuses most edits to a page. It is a bounded random search, not a proof. The coverage-guided
  target is `fuzz/fuzz_targets/omf.rs`. It takes the members, not the archive, so that a mutation is not stopped by
  a CRC-32, and it checks every project it reads for in-range vertex, category and sub-block indices. Its seeds
  (`fuzz/seeds.py`) are the every-kind file in four uncompressed page forms, plus the omf-rust samples under
  64 KiB when `FMINE_OMF_DIR` is set. Its first 24 h run started 2026-11-03 03:07Z; its result belongs on BD-197.
  Not checked: files from writers other than omf-rust.
- `PO-IO-7` (bd-aqfl): `omf::tests::a_wireframe_against_a_block_model_gives_each_parent_block_its_proportion`
  writes a box Surface as an element origin plus vertices from zero, inside a composite, and 3 × 1 × 2 unit
  blocks that the box covers in quarters. The surface's volume enclosure holds the box's volume. At depths 0 to 3
  each block's exact proportion lies within its bounds;
  from depth 2 the bounds equal it, and the sure volume is the box's 1.96875. Depth 99 is refused. With the element
  origin dropped (planted, reverted) the first block's 0.375 fell outside its bounds [0, 0].
  `proportions_refuse_missing_ambiguous_and_unsuitable_elements` reaches each refusal of §5 on the every-kind file.
  `a_turned_or_mirrored_block_model_gives_the_box_its_proportions_in_its_own_frame` (bd-wn57) places the same box
  through the model's axes in three frames: x, y and z; a turn that also tilts; and a mirror. The x, y and z frame
  and the mirror give the exact proportions from depth 2. In the tilted frame, which rounds every coordinate, each
  proportion lies within its bounds and the band narrows with depth (total 3.25, 1.92, 0.98 at depths 1 to 3).
  Axes that span no volume are refused. Planted, reverted: without the mirror's triangle reversal the mirrored box
  was refused as inward; with the origin left out, the first block's 0.375 fell outside [0, 0].
  The real files are in PO-IO-6's `omf_samples.rs`.
- `PO-IO-8` (bd-joiv): `omf::tests::an_omf_block_model_gives_its_certified_ultimate_pit_and_shells` writes a
  5 × 1 × 3 section of 12.5 m blocks (sizes scale to 25) with its ore at the bottom centre.
  - At 45° and 4 per unit, ore at 9.25 pays for its 9-block cone (value 5 quarter-units), and ore at 7.75 does not
    (the pit is empty). At 1 per unit, 9.25 rounds to 9 (value 1).
  - Two domains, the lower ones at 2:1, narrow the cone to 5 blocks, which 7.75 pays for (value 15).
  - Shells at revenue factors 1/2, 3/2 and 2 hold 1, 9 and 9 blocks, worth 1, 19 and 16 in units of their
    denominators.
  - Every in-pit flag and first shell is checked block by block in OMF order, all against hand counts.
  - A model turned about W is taken.
  `omf_pits_refuse_unsuitable_models_and_attributes` reaches each refusal of §5. With the layer reversal dropped
  (planted, reverted), the ore sat on top and was mined alone, `(37, [2])`.
  Not checked: a real OMF block model; models larger than the test's 15 blocks.
- `PO-IO-9` (bd-fr8x): `omf::tests::an_omf_block_model_gives_a_feasible_discounted_schedule` schedules the same
  section over 3 periods at 10%, ranked by its ultimate pit.
  - Blocks of 1.2 t round up to 2 units, so 10 units a period hold five blocks.
  - The result is the hand optimum: four waste blocks in period 0, then the fifth, the three middle blocks and the
    ore in period 1, worth `-16 · 2^40 + 21 · ⌊2^40 · 10/11⌋`.
  - TopoSort filled period 0 with five blocks, and one "later" move deferred one of them.
  - An independent `toposort::check` passes, with period values [-16, 21, 0], uses [8, 10, 0] and the blocks
    outside the pit unmined.
  With uses rounded to nearest (planted, reverted), all nine blocks fit period 0 and the test failed. Limits for
  the wrong number of periods and a missing capacity attribute are refused.
- `PO-IO-10` (bd-djb7): `omf::tests::an_lp_guided_omf_schedule_lies_under_its_certified_bound` schedules PO-IO-9's
  section and capacities from PDLP's solution of the relaxation (ε = 1e-6).
  - The incumbent reaches the same hand optimum and passes an independent `toposort::check` with period values
    [-16, 21, 0].
  - The relaxation's optimum is `25/9 + 20/9.9`: 5/9 of the cone in period 0, all a period holds, and 4/9 in
    period 1. PDLP's certified bound lies at or above it and within 1e-3 of it (4.7979798173 against
    4.7979797980), and above the schedule's own `-16 + 21/1.1`.
  - Planted, reverted: shares from a zero vector in place of PDLP's `x` left every start empty and the search at
    0, and the bound without its sign change fell below the optimum; the test failed on each.
  Limits for the wrong number of periods are refused. Not checked: a real OMF block model; the gap on models
  larger than the test's 15 blocks.
- `PO-IO-11` (bd-06kg): the schedule LP goes out as MPS, and an outside solution comes back checked.
  - `omf::tests::the_schedule_lp_goes_out_as_mps_and_its_solution_comes_back_as_a_checked_schedule_and_bound`:
    - PO-IO-10's relaxation (111 rows, 45 columns) is written as MPS and read back to the same text.
    - PDLP's `x` and bound multipliers, written as solution files and read back, give `lp_schedule`'s incumbent,
      problem and factors, and its bound, bit for bit.
    - Shares of zero give a checked schedule worth no more. Multipliers of zero give a weaker bound (37 against
      4.7979798173).
    - Refused: a share one ulp above 1, and wrong counts of shares and multipliers.
  - `omf::tests::highs_solution_of_the_exported_schedule_lp_comes_back_as_a_checked_schedule_and_bound`:
    - The exported text equals the pinned `testdata/omf_section_schedule.mps`, the input HiGHS solved. A change to
      the relaxation fails here until both fixtures are captured again.
    - HiGHS's shares violate no row or bound (0). The guided schedule passes `check` and reaches the hand optimum.
    - HiGHS's multipliers certify 4.7979797979797985 against the hand optimum `25/9 + 20/9.9`
      (4.797979797979798), held within 1e-6. PDLP's bound at ε = 1e-6 is 4.7979798173.
  - Planted, reverted: the bound without its sign change. Both tests failed.
  - Not checked: a client's commercial solver; any model but this 15-block section; an outside primal solution
    whose shares lie outside `[0, 1]` by the solver's tolerance, which `guided_schedule` refuses (HiGHS's did not).
- `PO-IO-12` (bd-jfg4): a client's schedule in the OMF is checked, then improved.
  `omf::tests::a_clients_schedule_in_the_omf_is_checked_then_improved_and_a_broken_one_is_reported` reads the
  section's "plan" attribute, floats in years from 1: the top layer in year 1, the middle three and the ore in year 2.
  - `check` passes it, with period values [-20, 25, 0] and uses [10, 8, 0]. Its discounted value is
    `-20 · 2^40 + 25 · ⌊2^40 · 10/11⌋`.
  - Local search reaches PO-IO-9's hand optimum in one "later" move (2.7272727 to 3.0909091 quarter-units), and an
    independent `check` passes the result. Both lie under PDLP's certified bound, 4.7979798.
  - The ore alone in year 1 breaks its three requirements, with witnesses (2, 6), (2, 7) and (2, 8) in OMF's
    numbering. The whole cone in year 1 uses 18 of period 0's 10. Neither is improved.
  - An Integer attribute reads too. Refused: 9.25, a value below `first`, 1e19, a period past `u32::MAX` or
    overflowing `i64`, a Category, a missing attribute, one on Vertices, a short candidate, and a period past the
    last.
  - Planted, reverted: the candidate taken without reversing its layers. The test failed.
  - Not checked: a vendor's own export; any model but this 15-block section; the refusal of periods as dates.
- `PO-IO-13` (bd-hq5y): a designed pit against the certified shell gives its loss in exact enclosures.
  `omf::tests::a_designed_pit_against_the_certified_shell_gives_its_loss_in_exact_enclosures` compares boxes
  through the section's full width with PO-IO-9's ultimate pit (values at 4 a unit, tonnes at 10).
  - "design" covers the middle three columns and half of each corner column. At depth 1 every block is exact
    (4 or 8 of 8 sub-cells). In quarter-units:
    - the pit is worth 5 and the design −7;
    - the design leaves −4 (half the pit's top corners) and adds −16;
    - so the loss is 12, with each enclosure a single value.
  - Tonnes: 14.4 t designed against the pit's 10.8 t.
  - "deep design" leaves the top layer out. It is worth 9, so its loss is −4: the slopes are not checked.
  - "rough design" has its sides off the sub-cell faces, so the corner columns are 0.52 inside, worth −7.48. Depths
    1 to 4 enclose that in [−19, −7], [−13, −7], [−10, −7] and [−8.5, −7], each half as wide as the one before.
  - At each depth `loss` equals `shell − design` as an interval.
  - Refused: a design 1 m past the model's side, a missing element, a BlockModel, depth 99, and shells and weights
    of the wrong length.
  - Planted, reverted: proportions left in OMF's layer order. The deep design failed; the full-height boxes, the
    same in every layer, did not.
  - Not checked: a vendor's own design export (an open pit surface must be closed before it is read); turned
    models; grade uncertainty.
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

- HTML synthetic controls cover merged origins and anchors, exact decimals and UTF-8 byte ranges, one-pass
  entity decoding, fake tables in comments/raw-text contexts, ambiguous numeric markup, malformed spans and
  row groups, hidden content, invalid encodings and global expansion including rectangular padding.

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
- `KILL-6` (OMF purity, plan §32.2) has not fired. It fires if omf-rust needs banned dependencies and the Parquet
  subset passes 8,000 lines. The first holds (E858). Measured at BD-194, `omf/parquet.rs` without its test writer
  and tests is 923 lines (838 of code); the whole reader, `omf.rs` and `omf/` without tests, is 2,877.

## 9. Lifted Code & Foundation Dependencies
- Depends on `fmine-core`, `fmine-evidence`, `fmine-claim` (V to S0), and `fmine-runtime` (V to S1, explicit clock
  and owned bounded subprocesses).
- `fmine-geom` supplies `volcert`'s certified closed meshes and block proportions (V to K0, bd-aqfl).
- `fmine-optimize` supplies `pitflow`'s slope arcs, certified ultimate pit and nested shells (V to E, bd-joiv),
  and `toposort`'s schedules (bd-fr8x, bd-djb7).
- `fmine-lp` supplies PDLP and its certified dual bound for LP schedules (V to E, bd-djb7), and MPS output and
  solution files for outside solvers (bd-06kg).
- Oracle fixtures (§33.2, bd-06kg): `fixtures/fmine-io/omf_section_schedule_highs_{columns,rows}.txt` are HiGHS's
  solution of `testdata/omf_section_schedule.mps`, from scipy 1.18.1's `linprog(method="highs")` as run by
  `scripts/oracle/highs_lp.py`, with capture records beside them. The script reads the MPS with its own parser and
  shares no code with fmine-lp. It was written by the agent that wrote the code under test.
- `fmine-calib` supplies the shared `ProofTimes` and UTC parser (V to K0); timestamp verification invokes
  OpenSSL 1.1 or 3.x on `PATH` with Unix `/dev/stdin` and `/dev/null`. Unsupported installations refuse.
- No omf-rust code is lifted. Its schemas, read checks and validator (tree 64d3057) are the specification the
  `omf` reader follows.

## 10. Non-Goals & Scope Boundary
Reads cells and adapts the reliance model to local files; does not interpret tables (`fmine-public`) or compute
metrics (`fmine-metrics`).
The OMF reader reads; it does not write OMF, decode images, transform geometry into world coordinates, or turn a
block model into the engines' types beyond the parent grid that block proportions take. It reads OMF 2 only: OMF 1
files and Parquet from other writers outside the subset are refused, not converted. Block proportions do not handle
tensor grids, sub-blocks, or surfaces that are open or intersect themselves (the last is not checked).
OMF pits take block values as given in attributes. They hold no price deck or cost model, do not value sub-blocks,
and do not write the pit back to OMF. Schedules have no destinations (PCPSP). `schedule` takes the caller's
ranking; `lp_schedule` searches from the LP's starts and reports its gap to the bound, but proves no schedule best.
No outside solver runs in CI or at run time: HiGHS ran once in the lab and its output is pinned. There is no CLI verb
for MPS export or LP-solution import. `plan schedule --omf` admits candidate period attributes. A candidate
comes only as periods per parent block, from an attribute or the caller. Periods as dates, a calendar, vendor
string files and partial blocks are not read, and a broken candidate is never repaired. The shell-to-design loss is
geometric: no grade uncertainty over realizations (no geostatistics here), no ramp-induced slope flattening, no
check of the design against the slopes, and no closing of an open design surface.
The bounded HTML reader is not a browser or DOM repair engine. It does not execute scripts, resolve CSS,
interpret inline XBRL transformations, or establish issuer, metric, period or ownership semantics. Its source
anchor records the inspected bytes and location, not the financial meaning, publisher authenticity or independent
publication time.
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
