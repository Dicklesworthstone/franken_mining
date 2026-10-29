# Corpus mining brief (shared by agents C1–C6)

## Why this exists

FrankenMining is an evidence-first software system for integrated gold, copper, silver and tin miners
(Barrick/Newmont class). Its design plan is
`/Users/jemanuel/projects/franken_mining/COMPREHENSIVE_PLAN_FOR_THE_DESIGN_OF_FRANKENMINING.md` (~6,900 lines).
The owner supplied a corpus of public documents (Barrick technical reports, Barrick quarterly mine statistics,
site-visit decks, an ESIA, and mining-geology textbooks). The product is general, but **the demonstration uses
public Barrick data**. Your job is to mine your assigned documents for (a) data the demonstrator can use, and
(b) facts and ideas that improve or correct the plan.

## Where things are

- Corpus PDFs/XLSX: `/Users/jemanuel/projects/franken_mining/corpus_of_input_docs/`
- Markdown mirror (mechanical extraction, NOT human-verified):
  `/Users/jemanuel/projects/franken_mining/corpus_of_input_docs/markdown_extractions/<same name>.md`.
  Each PDF page starts with an invisible anchor `<!-- p.N -->`. Tables were extracted mechanically and can be
  garbled; OCR'd pages can contain glyph errors.
- **Verify every number you report as FACT against the PDF page itself**:
  `pdftotext -layout -f N -l N <file.pdf> -` (poppler is installed). For spreadsheets, read the `.xlsx` directly
  with `uv run --with openpyxl python3 -I ...` if the markdown table is ambiguous (one sheet has ~15,900 junk
  empty columns).
- Plan navigation: `grep -n "^## \|^### " <plan>` gives the table of contents. Key sections:
  - §0.3 identifier prefixes; Part I (§1–5) thesis and the "galaxy-brain" cards GB-xx;
  - Plumb §6–10 (evidence classes, Claims, KK/KU/UU, calibration ledger);
  - Lodestar §11 (exploration, Bayesian prospectivity), Corebox §12 (drilling, sampling, QA/QC),
    Strata §13 (geological/resource models, kriging parity, simulation), Highwall §14 (geotechnics),
    Pushback §15 (mine planning, cut-off, pit optimization), Headframe §16 (operations twin, drill/blast,
    load/haul, underground), Crucible §17 (processing, metallurgical accounting, refining), Freeboard §18
    (tailings, water, ARD, closure), Muster §19 (labour), Touchstone §20 (reconciliation, F-factors),
    Bullion §21 (accounting, AISC/AIC), Helm §22–23 (decisions, capital allocation, juniors),
    Hallmark §24 (reserve/resource disclosure, QP data verification);
  - §32 Portal (importers), §34.4 data-acquisition gates, §37 milestones.
  - **M0 (months 0–3)** is the public lane: `fmine assure public --issuer barrick` reproduces per-site AISC, TCC,
    GEO, grade and recovery from public filings, every cell citing a filing locator, plus a metric-definition
    registry, a discrepancy register and frozen per-site forecasts. Acceptance GATE-BULL-01: ≥ 95% of
    site-quarter values reproduced to published rounding.
- Evidence register: `/Users/jemanuel/projects/franken_mining/planning/mor/MOR_CASE.md` (rows E001–E035).
  Propose new rows only inside your own id block (see your assignment).

## Output (one file per agent)

Write `/Users/jemanuel/projects/franken_mining/planning/corpus/C<n>_<slug>.md` with these sections:

1. `RESULT_STATUS: MATERIAL_INCREMENT` or `NO_MATERIAL_INCREMENT` (a checked null is a valid result).
2. **Inventory.** What each document contains, by page range, and how usable the extraction is.
3. **Demonstrator data.** Tables of values the demo can use: value, unit, **basis** (100% vs attributable, wet vs
   dry, contained vs recovered, by-product vs co-product, $/oz sold vs produced), period, document and page.
   Every value verified against the PDF page or the xlsx cell.
4. **Plan impacts**, ranked by value. Each item: plan § reference; type (CORRECTION / ADDITION / VALIDATION /
   NEW-IDEA / RISK / DATA-GATE); evidence (document + page); and a concrete proposed change (a sentence or a
   table row ready to paste).
5. **Proposed evidence rows** in your id block: `E### | claim | source + page | quote ≤ 25 words | class`
   (FACT if stated in the document; INFERENCE if derived by you, with the derivation).
6. **Checked nulls**: areas you checked where the documents add nothing.

## Rules

- Read-only everywhere except your own output file. Do not edit the plan, MOR_CASE.md or the corpus.
- No fabrication. If extraction is garbled and you cannot verify against the PDF, say so and do not report the
  number as FACT. Quotes ≤ 25 words.
- Do not manufacture findings; a short list of verified, high-value items beats a long speculative one.
- These documents are public. Still, Nevada Gold Mines (Carlin, Cortez, Turquoise Ridge; 61.5% Barrick,
  38.5% Newmont) and other JVs are sensitive in the plan (JV dispute history, venture walls): record the
  ownership basis of every figure.
- Respect the plan's doctrine: software never declares anything "safe", never signs for a QP/CP or engineer, and
  states uncertainty with evidence classes. Flag places where a document shows how real practice works that the
  plan has wrong or missing.
- Your final message (returned to the orchestrator) is ≤ 500 words: the output path, the five most valuable
  findings, and anything that blocks use of the data.
