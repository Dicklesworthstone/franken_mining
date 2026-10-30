# CONTRACT.md — `fmine-io`

## 1. Charter & Layer
- **Crate:** `fmine-io`
- **Layer:** `V` (Verification, Ingestion & Assurance)
- **Subsystem:** `Ingestion`
- **LOC Budget:** `16k planned / 18k ceiling` (`registries/loc_budget.toml`)
- **Allowed Module Families:** `["xlsx", "zip_inflate", "styles", "numfmt"]`

## 2. Mathematical & Domain Formulation
Pure-Rust ZIP and RFC 1951 DEFLATE (`zip_inflate`); `.xlsx` workbook, shared-string and worksheet extraction
(`xlsx`); `G-BULL-1` quarantine of formula errors and hidden sheets; `G-BULL-2` placeholder preservation (`"-"`,
`"N/A"` become `CellValue::Placeholder`, never `0.0`); HTML table extraction (`extract_html_tables`).

`Workbook::cell_census` counts cells by kind over visible sheets and computes the numeric-cell digest: SHA-256 over
`"{sheet}\t{cell_ref}\t{value}\n"` for every numeric cell, sheets in workbook order, cells in `(row, col)` order,
values in `f64` `Display` form. The format is fixed so an independently written reader can reproduce it
(`GATE-BULL-01` part 1).

`build_minimal_xlsx` writes a minimal valid workbook (stored ZIP, inline strings) so tests and the synthetic lane
can use a real filing's layout without shipping any issuer's bytes.

## 3. Authority, Epistemics & Representation
Every workbook carries the SHA-256 of its bytes; `Workbook::make_anchor` gives a `SourceAnchor`
(`artifact_path`, sha256, `sheet!cell`).

## 4. Public API Surface
- `IoError`, `HtmlTable`, `extract_html_tables`
- `zip_inflate::{read_zip_entries, inflate_raw, write_stored_zip}`
- `xlsx::{CellValue, ExtractedCell, QuarantineItem, Sheet, Workbook, CellCensus, FormatCount, MinimalCell,
  MinimalSheet, build_minimal_xlsx, col_index_to_letters, parse_a1_ref}`; `Workbook::format_census`;
  `ExtractedCell::{style, format}` (bd-jg43.1)
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

## 5. Guards & Typed Refusals
- `G-BULL-1`: formula-error cells and `hidden`/`veryHidden` sheets go to `Workbook::quarantined`. Hidden sheets are
  parsed into `Workbook::hidden_sheets` (BD-331.S2, for the hidden scope), never into `sheets`, `sheet()`,
  `cell_census` or the numeric digest; their formula errors are not logged.
- `G-BULL-2`: placeholders are typed, never zero. A placeholder keeps its token exactly as stored, surrounding
  whitespace included (`"— "` and `"—"` stay distinct); a number keeps its stored text in
  `ExtractedCell::number_text`.
- `Workbook::from_file` on a missing or unreadable path returns `IoError::FileRead` naming the path. There is no
  fallback content of any kind (`BD-321`).

## 6. Proof Obligations (`PO-*`)
- `PO-IO-1`: on the 18 registered Barrick workbooks, the per-workbook census (visible and hidden sheets; numeric,
  placeholder, error and text cells) and numeric digest equal an independent reader's (MOR_CASE E820; asserted in
  `fmine-public`'s ignored real-file test).
- `PO-IO-2`: writer-to-parser round trip on a synthetic workbook reproduces typed cells, quarantine, census and a
  digest computed from the fixed listing format (unit test).

## 7. Failure Modes & Deterministic Fallbacks
Missing files, corrupt ZIP or DEFLATE streams and malformed workbooks fail closed with `IoError`. No fallbacks.

## 8. Gates & Kill Criteria (`GATE-*` / `KILL-*`)
- `GATE-BULL-01` (part 1, extraction fidelity)

## 9. Lifted Code & Foundation Dependencies
- Depends on `fmine-core`, `fmine-evidence`.

## 10. Non-Goals & Scope Boundary
Reads cells; does not interpret tables (`fmine-public`) or compute metrics (`fmine-metrics`).
