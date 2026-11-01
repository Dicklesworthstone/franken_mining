# CONTRACT.md — `fmine-io`

## 1. Charter & Layer
- **Crate:** `fmine-io`
- **Layer:** `V` (Verification, Ingestion & Assurance)
- **Subsystem:** `Ingestion`
- **LOC Budget:** `16k planned / 18k ceiling` (`registries/loc_budget.toml`)
- **Allowed Module Families:** `["xlsx", "zip_inflate", "styles", "numfmt", "numtok"]`

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

## 3. Authority, Epistemics & Representation
Every workbook carries the SHA-256 of its bytes; `Workbook::make_anchor` gives a `SourceAnchor`
(`artifact_path`, sha256, `sheet!cell`).

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
  percentage points. A range keeps both endpoint precisions. The token span includes its prefix and suffix.
  `$` alone remains an unspecified dollar currency; `US$` explicitly names USD.
- Basis and period tokens are finite lexical matches, not a sentence interpretation. Explicit ownership,
  per-ounce denominator, credit convention and definition words stay separate. Only `ownership basis unprinted`
  or `unprinted ownership` supplies the unprinted marker. Quarter/half tokens without a year keep `None`;
  no current year is supplied. Explicit period and ownership metadata do not become extra numerical amounts.

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

## 6. Proof Obligations (`PO-*`)
- `PO-IO-1`: on the 18 registered Barrick workbooks, the per-workbook census (visible and hidden sheets; numeric,
  placeholder, error and text cells) and numeric digest equal an independent reader's (MOR_CASE E820; asserted in
  `fmine-public`'s ignored real-file test).
- `PO-IO-2`: writer-to-parser round trip on a synthetic workbook reproduces typed cells, quarantine, census and a
  digest computed from the fixed listing format (unit test).
- `PO-IO-3` (BD-345.S1): synthetic known answers check the documented decimal/step examples, accounting negatives,
  range endpoint precision, each basis/period class, explicit currency and unit distinctions, UTF-8 spans and
  malformed/overflowing numbers. Seeded Philox text checks totality and non-overlapping character-boundary spans.

## 7. Failure Modes & Deterministic Fallbacks
Missing files, corrupt ZIP or DEFLATE streams and malformed workbooks fail closed with `IoError`. No fallbacks.

## 8. Gates & Kill Criteria (`GATE-*` / `KILL-*`)
- `GATE-BULL-01` (part 1, extraction fidelity)

## 9. Lifted Code & Foundation Dependencies
- Depends on `fmine-core`, `fmine-evidence`.

## 10. Non-Goals & Scope Boundary
Reads cells; does not interpret tables (`fmine-public`) or compute metrics (`fmine-metrics`).
The draft tokenizer recognises a bounded ASCII-number and lexical-context grammar, not arbitrary mathematical
notation or prose semantics. It makes no publicness, calibration or legal conclusion. The two-reader issuer
sentence corpus and its precision/recall study remain separate work; synthetic lexer tests do not stand in for it.
