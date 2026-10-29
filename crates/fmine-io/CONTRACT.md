# CONTRACT.md — `fmine-io`

## 1. Charter & Layer
- **Crate:** `fmine-io`
- **Layer:** `V` (Verification, Ingestion & Assurance)
- **Subsystem:** `Ingestion`
- **LOC Budget:** `16k planned / 18k ceiling` (`registries/loc_budget.toml`)
- **Allowed Module Families:** `["xlsx", "zip_inflate"]`

## 2. Mathematical & Domain Formulation
Implements pure-Rust ZIP + RFC 1951 DEFLATE decompression (`zip_inflate`), `.xlsx` OpenXML workbook/sharedStrings/worksheet extraction (`xlsx`), `G-BULL-1` formula-error (`#REF!`) and hidden-sheet quarantine (`QuarantineItem`), `G-BULL-2` placeholder preservation (`"-"`, `"N/A"` -> `CellValue::Placeholder`), and SEC EDGAR HTML table extraction (`extract_html_tables`).

## 3. Authority, Epistemics & Representation
Tags every extracted `.xlsx` cell and HTML table cell with a BLAKE3 `SourceAnchor` (`artifact_path#sheet!cell_ref`).

## 4. Public API Surface
- `IoError`, `HtmlTable`, `extract_html_tables`
- `zip_inflate::{read_zip_entries, inflate_raw, build_canonical_barrick_xlsx_bytes}`
- `xlsx::{CellValue, ExtractedCell, QuarantineItem, Sheet, Workbook, col_index_to_letters, split_cell_ref}`

## 5. Guards & Typed Refusals
- `G-BULL-1`: Quarantines `#REF!` formula errors and `state="hidden"` / `state="veryHidden"` worksheets into `Workbook::quarantined`.
- `G-BULL-2`: Preserves `"-"`, `"N/A"` as `CellValue::Placeholder` (never `0.0`).

## 6. Proof Obligations (`PO-*`)
- `PO-IO-1`: Parses all 18 Barrick `.xlsx` workbooks (`2024Q1–2026Q2`), extracting `>35,000` cell anchors and quarantining `Gold CAS!#REF!` and `Copper CAS (2)` (`veryHidden`).

## 7. Failure Modes & Deterministic Fallbacks
Corrupted ZIP/DEFLATE streams fail closed with `IoError::CorruptZip` or `IoError::CorruptDeflate`.

## 8. Gates & Kill Criteria (`GATE-*` / `KILL-*`)
- `GATE-BULL-01`

## 9. Lifted Code & Foundation Dependencies
- Depends on `fmine-core`, `fmine-units`, `fmine-time`, `fmine-evidence`, `fmine-claim`.

## 10. Non-Goals & Scope Boundary
Does not compute AISC consolidation bridges or LMDI decompositions (`fmine-metrics`).
