# W1: Barrick quarterly Mine Statistics, Q1 2024 – Q2 2026 (Q3 2026 pending)

**Question.** Get every Barrick quarterly Mine Statistics workbook from Q1 2024 to Q3 2026 and verify each one. Then rerun the C1 public-data reproduction tests (T1–T15, plus three new tests) on all of them, and settle five site questions: Loulo-Gounkoto, Hemlo, Tongon, Lumwana and North Mara.

**Baseline.** `planning/corpus/C1_barrick_public_stats.md` §3.1–3.6 (Q1 2026 and Q2 2026 only).

**Dates.** Researched 2026-10-28/29. All downloads were made 2026-10-29 between 03:35:00Z and 03:36:24Z (UTC).

**Handling rules.**
- Downloads were treated as untrusted data.
- Workbooks were parsed only with `openpyxl` (`data_only=True`) from scripts outside the download directory, run as `python3 -I`. PDFs were read only with `pdftotext`.
- Nothing from the downloads was executed.
- No cookies or authenticated sessions were used.
- No existing file was edited or overwritten.

**Ownership basis (applies to every figure below unless a row says otherwise).** All of this is FACT, read from the files.
- **Summary sheets (`QTD`/`YTD`).** Column B is "Gold produced (000s ozs 100% basis)". Columns C and D are "attributable share" (`Q2_2026:QTD!B4:D4`).
- **Per-unit costs (CoS, TCC/C1, AISC).** Per attributable ounce or pound sold.
- **`Minestats1–4` physicals.** Barrick's attributable share. The site header prints the percentage, e.g. "Carlin (61.5%)" at `Q2_2026:Minestats1!B2`.
- **Capex.** "reported at Barrick's share" (`Q1_2025:Minestats1!A54`).
- **NGM.** NGM (Carlin, Cortez, Turquoise Ridge, Phoenix) is **61.5% Barrick / 38.5% Newmont**, so every NGM or NGM-mine figure here is 61.5%-attributable unless it is tagged 100%.
- **Sites with no printed percentage** (Hemlo, Lumwana) are 100%-owned and have no percentage in the label.

---

## 1. RESULT_STATUS: MATERIAL_INCREMENT

- **Coverage: 10 of 11 quarters obtained**, Q1 2024 to Q2 2026, each with its MD&A PDF and, from Q3 2024 on, the AISC reconciliation workbook. The Q1 2026 and Q2 2026 downloads are byte-identical (sha256) to the existing corpus copies.
- **Q3 2026 is not released.** Barrick's notice (9 Oct 2026) schedules it for **Monday 9 Nov 2026, 06:00 ET**. At 2026-10-29T03:36:32Z the CDN returned 404 for every candidate path, and the IR feed holds an empty "Third Quarter 2026" entry.
- **Tests.** All 15 C1 tests were rerun on all 10 files, together with 3 new tests:
  - T16: cross-vintage stability;
  - T17: ownership-label consistency;
  - T18: AISC workbook vs Mine Stats.
- **Structural identities hold everywhere:** T2 612/0, T6 596/0, T6b 296/0, T6c 303/0, T7 475/0, T15 18,627/0.
- **The failures cluster** in a small number of cells. Section 6 gives 17 new discrepancy candidates with exact locators and reconfirms C1 DR-01/02/04/05/07/09/11/12 across vintages.
- **Most consequential findings:**
  - **(a)** The Copper-Total recovery is a constant 94% in all 20 quarterly cells. That includes 2024Q2, when both reporting mines were below 94% (C1 DR-06 extended and strengthened).
  - **(b)** Porgera is labelled **47.5%** in the Q2 2024 and Q3 2024 summary sheets, but the values are at 24.5%.
  - **(c)** The Q3 2025 file dates the Loulo-Gounkoto loss of control as **September 16**, while every other vintage says June 16.
  - **(d)** North Mara processed grade fell from 4.29 g/t (2024Q4) to 1.80 g/t (2026Q2), and AISC rose from $1,098 to $2,577/oz.
  - **(e)** The Total-Gold AISC sits above the sold-weighted regions in every one of the 16 periods tested, by +$23.57 to +$88.66/oz. This is an unallocated layer and must be modelled.

---

## 2. Coverage

### 2.1 Mine Statistics workbooks (one per quarter)

**Download location.** Directory `/Users/jemanuel/projects/franken_mining/corpus_of_input_docs/downloaded/mine_stats/`.

**Source URLs.** `https://s25.q4cdn.com/322814910/files/doc_financial/quarterly_results/<path>`. Q1 2025 uses folder `2025/01/`, not `2025/q1/`.

**HTTP status.** Every file returned HTTP 200.

**Recon-page column.** The four numbers are the MD&A PDF pages for:
1. the endnote-cited page ("provided starting on page N");
2. the first TCC/AISC reconciliation page;
3. the first per-site gold table;
4. the copper C1 reconciliation.

In every MD&A the printed page number equals the PDF page index.

| Quarter | File | URL path (after `quarterly_results/`) | Bytes | sha256 | Retrieved (UTC) | Blocks (current + comparative) | Recon pages |
|---|---|---|---|---|---|---|---|
| Q1 2024 | `Barrick_Q1_2024_Mine_Stats.xlsx` | `2024/q1/Barrick_Q1_2024_Mine_Stats.xlsx` | 43,668 | `e2e1243ec0549f4c487c773869876e2acade401a0b6cd1141adf4b9296aec840` | 03:35:00Z | 2024Q1, 2023Q1 | 54 / 56 / 59 / 65 |
| Q2 2024 | `Barrick_Q2_2024_Mine_Stats.xlsx` | `2024/q2/Barrick_Q2_2024_Mine_Stats.xlsx` | 109,832 | `1a73d6d7d4658f923757e873eec147ccc74aa8aada6a439f3c98da8540ff68d5` | 03:35:00Z | 2024Q2, 2023Q2, H1 2024, H1 2023 | 58 / 60 / 63 / 72 |
| Q3 2024 | `Barrick_Q3_2024_Mine_Stats.xlsx` | `2024/q3/Barrick_Q3_2024_Mine_Stats.xlsx` | 110,573 | `d8786ce96f7129bf4cc240e01d2fe7840bfde0df4082517975578e69d73ead3b` | 03:35:01Z | 2024Q3, 2023Q3, 9M 2024, 9M 2023 | 46 / 48 / 50 / 59 |
| Q4 2024 | `Barrick_Q4_2024_Mine_Stats.xlsx` | `2024/q4/Barrick_Q4_2024_Mine_Stats.xlsx` | 110,614 | `f5e7db7c4657927f96e8684b40f9b50c0061fa2e0c22912aa480babdb49ef4cd` | 03:35:01Z | 2024Q4, 2023Q4, FY2024, FY2023 | 61 (capex endnote A11 cites 60) / 61 / 63 / 72 |
| Q1 2025 | `Barrick_Q1_2025_Mine_Stats.xlsx` | `2025/01/Barrick_Q1_2025_Mine_Stats.xlsx` | 45,695 | `ec5158f8b4624643ee83d0099e4f3354dc550729ebb443a2dbfacfc29315998b` | 03:36:18Z | 2025Q1, 2024Q1 | 38 / 40 / 42 / 47 |
| Q2 2025 | `Barrick_Q2_2025_Mine_Stats.xlsx` | `2025/q2/Barrick_Q2_2025_Mine_Stats.xlsx` | 107,135 | `599864f9958b5a109530c321e9481fa7d59c53ed2c730be96f51035390a2d822` | 03:35:01Z | 2025Q2, 2024Q2, H1 2025, H1 2024 | 44 / 46 / 48 / 57 |
| Q3 2025 | `Barrick_Q3_2025_Mine_Stats.xlsx` | `2025/q3/Barrick_Q3_2025_Mine_Stats.xlsx` | 108,497 | `b55b857766195fe90464cf813e7a71a5112439d606a6fc96ec3b6c5f01b58715` | 03:35:01Z | 2025Q3, 2024Q3, 9M 2025, 9M 2024 | 43 (copper endnote A8 cites 44) / 45 / 47 / 56 |
| Q4 2025 | `Barrick_Q4_2025_Mine_Stats.xlsx` | `2025/q4/Barrick_Q4_2025_Mine_Stats.xlsx` | 117,396 | `94f6692acfa5f133b97ae028f172cc9c5e7527945788da7bd9a06ffd11c7eec4` | 03:35:01Z | 2025Q4, 2024Q4, FY2025, FY2024 | 60 (capex endnote A11 cites 59) / 59 / 61 / 66 |
| Q1 2026 | `Barrick_Q1_2026_Mine_Stats.xlsx` | `2026/q1/Barrick_Q1_2026_Mine_Stats.xlsx` | 55,644 | `ddef08dd544f0df511fc30f53c1dd889bb6878257d394b8baad7ac3dbfa8c5d2` | 03:35:01Z | 2026Q1, 2025Q1 | 36 / 38 / 40 / 43 |
| Q2 2026 | `Barrick_Q2_2026_Mine_Stats.xlsx` | `2026/q2/Barrick_Q2_2026_Mine_Stats.xlsx` | 131,296 | `cf0d3f4fea9c96f967c138d0aca549f4ff4f4a233bd2c5946c4035bf96c3df8e` | 03:35:01Z | 2026Q2, 2025Q2, H1 2026, H1 2025 | 44 / 46 / 48 / 53 |
| Q3 2026 | — | not published (404) | — | — | probed 03:36:32Z | — | — |

**Notes on the table.**
- **Q1 files** carry only `QTD` (there is no `YTD` sheet). Every other file has `QTD`, `YTD`, `Minestats1–4` and `Endnote`.
- **Non-GAAP section page.** The cited endnote page is the start of the "Non-GAAP Financial Measures" section. That section starts on the same page in all files except Q4 2024 (section p.59) and Q4 2025 (section p.57).
- **Q4 MD&As.** The gold-reconciliation heading falls one page before the first per-site table: p.62 in Q4 2024 and p.60 in Q4 2025.
- **Endnote wording.** `Q1_2024:Endnote!A5` reads "provided starting on page 54 in the First Quarter 2024 Report filed on SEDAR+".

### 2.2 Supporting files (same directory, same CDN, all HTTP 200)

**MD&A / quarterly report PDFs** (`Barrick_QN_YYYY_MD-A.pdf`). Columns give bytes / sha256 / page count:

| Q1 2024 | Q2 2024 | Q3 2024 | Q4 2024 | Q1 2025 |
|---|---|---|---|---|
| 1,702,165 / `09a50ed4ce1eb71351692ba5beaaa851c472ac1a979b582280190d1812a5583d` / 86 | 1,842,743 / `3ddfc7dcbc1721f7c54bea7f309979d29972ffbee98348a71644c4edcadcdd24` / 98 | 2,598,324 / `3ca53761402bf20e0e1419e3c1ab546dc0e134e1015c5435f9def4dda9ed4bf4` / 86 | 2,338,137 / `43dcfca7aadc48b47ade474b60405db0c027434f68095823e5e9adbac6cf4924` / 147 | 1,890,648 / `a8734b98dddf941624a17223c0e3a9000bdb77de962f2c2fff0d79b9ff728259` / 67 |

| Q2 2025 | Q3 2025 | Q4 2025 | Q1 2026 | Q2 2026 |
|---|---|---|---|---|
| 2,424,176 / `849b54b343e2109e82aa75caa3aaf11492e150325b9c65f1487dfa49f10435ce` / 80 | 2,190,616 / `076819436e309d82698314c47ab848062b6e6061a6d76dbbd6bdc828d7eb0311` / 79 | 2,286,439 / `63b3af0b98436900159f6ddac7f2920a0178f0e8651f1ee262d07bbaa690d281` / 140 | 1,551,260 / `2bbc4b75b00b440a81b6d782f68b869abf1ac47ec3018069f06a39493551e1bc` / 64 | 1,650,987 / `08ee992f8a98886c79ef79909bf7dc690d16f0f7d169b5b8d470e9bc22e05139` / 76 |

**AISC reconciliation workbooks.** Saved as `Barrick_QN_YYYY_AISC.xlsx`. The source names are `Barrick_Q3_2024_AISC.xlsx` and `Barrick_Q4_2024_AISC.xlsx`, then `AISC_QN_YYYY.xlsx` from 2025 on.

| Quarter | Bytes | sha256 |
|---|---|---|
| Q3 2024 | 40,977 | `d063ea7c1cd667a6db9c9de1b3aa4cb0ff2dba4a8cb24f6ce889e56af2fd0ef7` |
| Q4 2024 | 42,141 | `f842696915b7a44d00194e01a3bdf4a76d1e52d0e31c2610a91be70afb4b1b4d` |
| Q1 2025 | 32,587 | `06951434c3a3e325553dbe25018544c7329bc1a8b6e01ab0d3fcdeba2bb4388e` |
| Q2 2025 | 41,599 | `1cbd6b739a276d64e4752ebbc67c369720ccb8eec16845a4858dccd04fd6d178` |
| Q3 2025 | 41,020 | `62901aaab21d64057422a950e29e0ad2ecebad525e0c2b8b9185e4843d9ecfd3` |
| Q4 2025 | 40,336 | `2ce7b8e46b36d745228487e683b221101c4db3019ba67acedea96967486d6755` |
| Q1 2026 | 43,718 | `dbe567e90730b4ef87330b1eaa539c7289a92781e452b949c24f7b705464ef28` |
| Q2 2026 | 49,738 | `66515a4ae4305de130b72fa1a3a78bf6aa51ac5a10350fc43732bb621c6d0e99` |

The IR feed lists no AISC workbook for Q1 2024 or Q2 2024.

**Release notice.** `Barrick-to-Report-Third-Quarter-2026-Results-on-November-9.pdf`: 93,851 bytes, sha256 `4f98457a2b6f358b87468ce300e92a44b65d50967d102e0c857b0731f09e5809`. Source: `https://s25.q4cdn.com/322814910/files/doc_news/2026/10/`.

**Manifests.** The full download manifest (file, URL, timestamp, HTTP code) is `w1/manifest.tsv`, and the hashes are in `w1/hashes.tsv`, both in the scratch folder (§16).

### 2.3 Q3 2026 status

- **Release date.** The notice is dated "TORONTO, October 9, 2026" and states that Barrick "will release its third quarter 2026 results before markets open on Monday, November 9, 2026, at 6:00 a.m. ET". The webcast is at 11:00 a.m. ET.
- **CDN probes.** HEAD requests at 2026-10-29T03:36:32Z returned **404** for:
  - `2026/q3/Barrick_Q3_2026_Mine_Stats.xlsx`;
  - `2026/q3/Barrick_Q3_2026_MD-A.pdf`;
  - `2026/q3/AISC_Q3_2026.xlsx`;
  - the same names under the folders `2026/03/`, `2026/10/` and `2026/11/`.
- **IR feed.** The feed (`/feed/FinancialReport.svc/GetFinancialReportList?…&year=2026`) has a "Third Quarter 2026" entry (ReportId 2733, ReportDate 11/09/2026) with **no documents**.
- **Access note.** `barrick.com` returns 403 to curl, so the feed was read with WebFetch. The files themselves came from the q4cdn host.
- **Expected path.** Based on the pattern, the Q3 2026 workbook should appear on 2026-11-09 at `quarterly_results/2026/q3/Barrick_Q3_2026_Mine_Stats.xlsx`. This is an INFERENCE.

---

## 3. Layout drift against C1 §3.1–3.2

The parser anchors on labels, not row numbers: blocks are found from column-A text "For the …", with the site header on the row above. With that approach, every file parsed without per-file configuration. These are the drifts an importer must tolerate.

### 3.1 Summary sheets (`QTD`/`YTD`)

**Columns.** B–G hold the current year and I–N the prior year in every file.

**Gold-row order.** Three versions:

| Version | Files | Row map |
|---|---|---|
| v0 | 2024 files | Long Canyon 9, NGM 10, Hemlo 11, North America 12, PV 13, Veladero 14, Porgera 15, "Latin America & Asia Pacific" 16, L-G 17, Kibali 18, Tongon 19, North Mara 20, Bulyanhulu 21, AME 22, Total Gold 23. Copper header row 25, labels row 26, sites 27–29, Total Copper 30 |
| v1 | Q1–Q3 2025 | Long Canyon row dropped, so everything moves up one: NGM 9 … Total Gold 22; copper 26–29 |
| v2 | Q4 2025 → | NGM 9, **PV 10**, Hemlo 11, North America 12, Veladero 13, Porgera 14, "**South America** & Asia Pacific" 15, L-G 16, Kibali 17, Tongon 18, NM 19, Bulyanhulu 20, AME 21, Total Gold 22 |

The v2 footnote reads "Starting Q4 2025, we have presented Pueblo Viejo as part of North America instead of South America & Asia Pacific. Comparative information has been restated." (`Q4_2025:QTD!A33`). Region aggregates are therefore **not comparable across the Q3 2025 / Q4 2025 boundary**. T16 skips 30 region rows for this reason.

**Label changes.**
- The cost label changes from "Total Cost of sales ($/oz)" (2024) to "Cost of sales ($/oz)".
- The copper label "C1 Cash Costs" (to Q2 2025) becomes "C1 cash costs" (Q3 2025 on).
- Double spaces appear inside the Q2 and Q3 2025 YTD period headers.

### 3.2 Minestats sheets

**Minestats1 columns.**
- **2024:** Carlin B, Cortez D, TR F, Phoenix H, Long Canyon J, NGM L, Hemlo N.
- **Q1–Q3 2025:** NGM J, Hemlo L.
- **Q4 2025 on:** NGM J, **PV L**, Hemlo N.

**Minestats2.** PV, Veladero and Porgera to Q3 2025; Veladero and Porgera from Q4 2025.

**Second-block start rows.**

| Sheet | Start row by vintage |
|---|---|
| MS1 | 63, then 64 (Q4 2025), then 66 (Q2 2026) |
| MS2 | 52, then 54 (Q2 2026) |
| MS3 | 44 (2024), 45 (Q2–Q3 2025), 46 (Q4 2025), 48 (Q2 2026) |
| MS4 | 38, then 40 (Q2 2026) |

**New row in Q2 2026 only.** A **"Revenue ($ millions)"** row (e.g. `Q2_2026:Minestats3!A23`), which pushes the cost rows down two. For example, L-G total capex is at `B34` up to Q1 2026 and at `B36` in Q2 2026.

**Footnote numbering.** The superscript numbers on labels change between files (e.g. "Total cash costs3" vs "Total cash costs4"), so labels must be matched with the trailing 1–2 digits stripped.

**Copper unit label (2024).** In all four 2024 files `Minestats4!A15`/`A16` read **"Production (millions of pounds)1"** and "Sales (millions of pounds)1". The values are kilotonnes: they equal the summary kt values, and footnote 1 says quantities moved to tonnes. From Q1 2025 the label is "Production (Thousands of tonnes)1". See W1-DR-02.

**Header typo.** `Q4_2025:Minestats1!A64` reads "For the Twelve months ended **Decemer** 31,". The parser must match on "twelve", not on the month.

**QP lists** (`Endnote`) change from quarter to quarter. They do not affect parsing.

---

## 4. Workbook hygiene and token census

**Formulas and `#REF!`.** No file contains formulas except **Q2 2026**, which has **1,092 `#REF!` formula cells** (`Minestats3!M4:V81` 660, `Minestats4!K4:R67` 432). They lie outside the data columns. The cached values are errors, so `data_only` readers must ignore columns beyond the block.

**Oversized YTD dimensions** (the sheet "used range" runs to the far right of the sheet):

| File | Range | Merged ranges | Merged ranges beyond column 40 |
|---|---|---|---|
| Q2 2024 | `A1:XFD41` (all 16,384 columns) | 1,184 | 1,169 |
| Q3 2024, Q4 2024 | `A1:XEM41` | 1,183 | 1,167 |
| Q2 2025, Q3 2025 | `A1:WSF40` | 1,160 | 1,145 |
| Q4 2025 | `A1:WSF42` | 1,162 | 1,145 |
| Q2 2026 | `A1:WMQ42` | 1,152 | 1,134 |

An importer must bound its scan (columns ≤ 20).

**GEO.** No per-site GEO (gold-equivalent ounces) appears in any workbook (0 hits). See §14.

**Display format.** 148 numeric cells use `General` format, and all of them are strip ratios (T15a).

**Tokens in parsed data cells:**

| File | `n/a` | `'— '` (trailing space) | `'—'` | blank | numeric | numeric 0 |
|---|---|---|---|---|---|---|
| Q1 2024 | 190 | 16 | 92 | 0 | 1,082 | 0 |
| Q2 2024 | 373 | 24 | 159 | 0 | 2,204 | 0 |
| Q3 2024 | 373 | 24 | 153 | 0 | 2,210 | 0 |
| Q4 2024 | 372 | 0 | 173 | 2 | 2,213 | 0 |
| Q1 2025 | 145 | 8 | 44 | 0 | 1,095 | 0 |
| Q2 2025 | 288 | 10 | 72 | 1 | 2,213 | 0 |
| Q3 2025 | 281 | 10 | 72 | 1 | 2,220 | 0 |
| Q4 2025 | 306 | 0 | 44 | 0 | 2,266 | 0 |
| Q1 2026 | 152 | 16 | 74 | 0 | 1,063 | 3 |
| Q2 2026 | 298 | 33 | 149 | 2 | 2,202 | 4 |

**Blank data cells (`None`).**
- `Q4_2024:Minestats2!G79`, `G85`: Porgera 2023FY CoS and AISC.
- `Q2_2025:Minestats3!B8` and `Q3_2025:Minestats3!B8`: L-G strip ratio.
- `Q2_2026:Minestats3!F23`, `F68`: Tongon revenue.

**Numeric-zero placeholders.** There are 7, all in 2026 and all recovery cells of the divested Hemlo and Tongon. This is C1 DR-07, unchanged.

**Q4 files** use `'—'` with no trailing space in the summary; other files mix both forms.

**Cumulative vs quarterly tokens.** Cumulative blocks often use `—` where the matching quarterly cells use `n/a` for the same circuit, e.g. Phoenix heap leach `Q2_2024:Minestats1!H35` `n/a` vs `H95` `—`. This causes 24 of the 32 Minestats T5 "fails" (W1-DR-12). The importer should map both to Nil and keep the original token.

**M0 target-cell census.** Five per-site fields (CoS, TCC/C1, AISC, processed grade, recovery), current vintage of every quarter, aggregates excluded: **820 cells = 744 numeric, 33 `'— '`, 11 `'—'`, 28 `n/a`, 4 numeric-zero.**

---

## 5. Reproduction tests on all ten files

**Scripts.** `w1/extract3.py` → `long_all3.json` (22,732 rows), then `w1/tests3.py` → `tests3_out.txt`. These are copies adapted from the C1 scripts; the originals in `scratchpad/scripts/` were not edited.

**Tolerances.** Same as C1:

| Test | Tolerance |
|---|---|
| T1 | \|100% × own − attr\| ≤ 0.5·own + 0.5 |
| T3, T5b | 1% |
| T5 | ±1 (two-part sums) or ±2 (four-part sums) |
| T8 | ±1 $M |
| T9 | ±3 percentage points of implied recovery |
| T11 | ≥ −25 $/oz, or ≥ −0.05 $/lb |

**Columns.** Each column is the quarterly release that holds the cells (current plus comparative blocks).

| Test | Q1 24 | Q2 24 | Q3 24 | Q4 24 | Q1 25 | Q2 25 | Q3 25 | Q4 25 | Q1 26 | Q2 26 | **Total** |
|---|---|---|---|---|---|---|---|---|---|---|---|
| T1 attr = 100% × own | 33/1 | 59/9 | 64/4 | 67/1 | 34/0 | 62/5 | 65/2 | 65/3 | 30/2 | 58/5 | **537/32** |
| T2 Σ sites = region/total (flows) | 36/0 | 72/0 | 72/0 | 72/0 | 36/0 | 72/0 | 72/0 | 72/0 | 36/0 | 72/0 | **612/0** |
| T3 sold-weighted unit-cost recombination | 31/5 | 61/11 | 62/10 | 64/8 | 30/6 | 58/14 | 62/10 | 63/9 | 30/6 | 57/15 | **518/94** |
| T4 Minestats = summary | 190/0 | 380/0 | 380/0 | 380/0 | 180/0 | 360/0 | 360/0 | 360/0 | 180/0 | 359/1 | **3,129/1** |
| T6 mined = ore + waste + UG | 35/0 | 70/0 | 70/0 | 70/0 | 36/0 | 71/0 | 71/0 | 72/0 | 34/0 | 67/0 | **596/0** |
| T6b processed = Σ circuits | 17/0 | 34/0 | 34/0 | 34/0 | 18/0 | 36/0 | 36/0 | 36/0 | 17/0 | 34/0 | **296/0** |
| T6c production = Σ circuits | 18/0 | 36/0 | 36/0 | 36/0 | 18/0 | 36/0 | 36/0 | 36/0 | 17/0 | 34/0 | **303/0** |
| T7 strip = waste/ore | 27/0 | 54/0 | 56/0 | 56/0 | 28/0 | 55/0 | 58/0 | 59/0 | 27/0 | 55/0 | **475/0** |
| T8 sust + proj = total capex | 32/2 | 67/3 | 67/3 | 64/5 | 32/3 | 62/10 | 59/12 | 59/12 | 28/5 | 54/14 | **524/69** |
| T9 metal balance (±3 pt) | 17/10 | 37/17 | 41/13 | 42/12 | 20/8 | 40/15 | 45/10 | 44/12 | 19/7 | 38/13 | **343/117** |
| T11 AISC − TCC − sust capex ≥ −tol | 34/0 | 68/0 | 70/0 | 70/0 | 34/0 | 68/0 | 70/0 | 70/1 | 32/0 | 66/0 | **582/1** |
| T14 no numeric-zero recovery | 86/0 | 172/0 | 172/0 | 172/0 | 78/0 | 156/0 | 156/0 | 164/0 | 79/3 | 160/4 | **1,395/7** |
| T15 stored = displayed precision | 1,069/0 | 2,186/0 | 2,191/0 | 2,197/0 | 1,083/0 | 2,198/0 | 2,206/0 | 2,251/0 | 1,055/0 | 2,191/0 | **18,627/0** |
| T15a display format defined | 0/13 | 0/18 | 0/19 | 0/16 | 0/12 | 0/15 | 0/14 | 0/15 | 0/11 | 0/15 | **0/148** |
| T16 (new) first publication = later comparative | – | – | – | – | 645/1 | 1,288/4 | 1,291/1 | 1,268/0 | 634/0 | 1,265/3 | **6,391/9** |
| T17 (new) ownership label: summary = Minestats | 34/0 | 64/4 | 64/4 | 68/0 | 32/0 | 64/0 | 64/0 | 64/0 | 32/0 | 64/0 | **550/8** |
| T18 (new) AISC xlsx = Mine Stats (per-oz, sold) | – | – | 56/0 | 56/0 | 52/4 | 52/4 | 52/4 | 56/0 | 48/0 | 48/0 | **420/12** |

**T5 additivity across files.** Each result is counted under the file that holds the cumulative figure.

| Identity | Summary | Minestats |
|---|---|---|
| Q1 + Q2 = H1 | 269/4 | 862/6 |
| H1 + Q3 = 9M | 203/1 | 648/7 |
| 9M + Q4 = FY | 193/2 | 647/8 |
| Q1 + Q2 + Q3 + Q4 = FY | 194/1 | 646/11 |

**T5b unit costs (1%).** Q1 + Q2 = H1: 257/2. H1 + Q3 = 9M: 195/0. 9M + Q4 = FY: 185/4.

**Not rerun.** T10 is diagnostic only and has no pass/fail. C1 §3.5 defines no T12 or T13.

**Cross-check against C1.** On the two files C1 covered, the T15 count reproduces exactly: 1,055 + 2,191 = 3,246, the same as C1. T1 reads 88/7 here against C1's 84/5. The difference comes from the wider field set and the stricter tolerance: for example, NGM Q2 2026 623 × 0.615 = 383.14 vs 382 fails here, while C1 passed it.

### 5.1 What the failures are

**T1 (32 fails).**
- 4 are the Porgera "(47.5%)" label (W1-DR-01).
- The remaining 28 are edges of 0.75–2.14 koz where the attributable value cannot be reproduced from the rounded 100% value. Examples:
  - Carlin 2024Q2 `Q2_2024:QTD!B5` 327 → 201.1 vs `C5` 202;
  - Carlin 2024Q4 301 → 185.1 vs 186;
  - Carlin 2025Q4 `Q4_2025:QTD!B5` 335 → 206.0 vs 207;
  - Carlin 2026Q1 = DR-01;
  - North Mara 2024Q2 63 → 52.9 vs 54 and 2025Q3 75 → 63.0 vs 64;
  - L-G FY2025 `Q4_2025:YTD!B16` 35 → 28.0 vs `C16` 29;
  - TR H1 2026 = DR-02;
  - NGM H1 2026 1,236 → 760.1 vs 758;
  - Jabal Sayid 2026Q2 = DR-04.
- Several of these coincide with T5 fails at the same cell (W1-DR-05 for Carlin), which points to a keying error rather than rounding.

**T3 (94 fails).** All are positive gaps at aggregation layers, never at a single mine:

| Layer | Fails | Gap |
|---|---|---|
| Total Gold AISC | 34 | +$23.57 to +$88.66/oz |
| Total Copper AISC | 28 | |
| AME | 14 | |
| Latin America | 9 | |
| NGM AISC | 3 | |
| Copper C1/CoS | 6 | |

This is C1 DR-12 confirmed in every vintage. See §11.

**T4 (1 fail).** DR-05: Hemlo H1 2026 CoS `Q2_2026:YTD!E11` = 1,691 vs `Q2_2026:Minestats1!N103` `—`.

**T8 (69 fails).** Every residual is positive: total > sustaining + projects.

| Site | Fails | Residual |
|---|---|---|
| PV | 24 | +2 to +25 $M |
| NGM | 19 | +2 to +4 |
| Carlin | 8 | |
| Lumwana | 7 | +3 to +15 |
| Total Copper | 7 | |
| L-G | 4 | +2 |

The capex footnote says capex "Includes capitalized interest where applicable" (e.g. `Q4_2025:Minestats1!A55`), which supports C1 DR-11 (capitalized interest is counted in the total but in neither component; INFERENCE).

**T9 (117 fails).** By site: Phoenix 34, Cortez 23, Carlin 13, PV 13, Porgera 13, Jabal Sayid 11, Hemlo 5, TR 5. There are **zero fails at Lumwana, North Mara, Bulyanhulu, Kibali, L-G or Tongon** (single-circuit mills). Phoenix is explained by its grade basis: "At Phoenix, stated grades relate to material slated for processing at the oxide mill." (`Q1_2025:Minestats1!A52`). Cortez reflects heap-leach timing (INFERENCE, as in C1).

**T11 (1 fail).** Hemlo 2025Q4: AISC 1,976 − TCC 1,707 − sustaining capex/oz (8 $M / 27 koz = 296) = −27, from `Q4_2025:Minestats1!N44`, `N43`, `N47`, `N36`. This is a stub quarter that ends at the 26 Nov 2025 sale closing (W1-DR-13).

**T14 (7 fails).** DR-07 (2026 only).

**T15a (148).** All are `General`-format strip ratios.

**T5 summary (8 fails).**
- Carlin 2024: H1 `Q2_2024:YTD!B5` 663 vs 334 + 327 = 661, and FY `Q4_2024:YTD!B5` 1,261 vs quarter sum 1,258 (W1-DR-05).
- L-G 9M 2024 `Q3_2024:YTD!B17` 527 vs 349 + 180 = 529.
- NGM and North America 2023 FY +2.
- DR-01, DR-02 and DR-04 in 2026.

**T5 Minestats (32 fails).**
- 24 are token mixes (`n/a` vs `—`, W1-DR-12).
- The 8 numeric fails are:
  - PV 2024 processed (total and autoclave) −130 kt, ×4 (W1-DR-04);
  - Hemlo 2023 capex +8, ×2 (W1-DR-06);
  - Tongon 2023 mined −2;
  - Total Copper 2025 processed −2.

**T5b (6 fails).**
- L-G FY2025 CoS +2.81% and AISC +9.67% (W1-DR-07).
- Long Canyon 2023 TCC/AISC −1.2 to −1.3%.
- DR-05.
- Jabal Sayid H1 2026 C1 −1.2%.

**T16 (9 fails).** Listed in W1-DR-08/09.

**T17 (8 fails).** All are the Porgera label (W1-DR-01).

**T18 (12 fails).** All are L-G 2025Q1–Q3 per-ounce and sold cells, where the AISC workbook stores numeric 0 (shown as "-" by accounting format) and Mine Stats shows `'— '` (W1-DR-14). Every other per-site per-unit and ounces-sold cell (gold $/oz, copper $/lb) on the current-quarter sheet of the eight AISC workbooks matches Mine Stats within rounding (≤ $1/oz, ≤ $0.01/lb).

---

## 6. Discrepancy candidates (new; IDs W1-DR-nn; C1 DR-nn reconfirmed at the end)

Locators are `<file>:<Sheet>!<Cell>`, where `<file>` is the quarterly release (e.g. `Q3_2024` means `Barrick_Q3_2024_Mine_Stats.xlsx`). For AISC workbooks the form is `Q1_2025_AISC:<sheet>!<cell>`.

| ID | Cells | Observation | Candidate explanation | Status |
|---|---|---|---|---|
| W1-DR-01 | `Q2_2024:QTD!A15`, `Q2_2024:YTD!A15`, `Q3_2024:QTD!A15`, `Q3_2024:YTD!A15` = "Porgera (47.5%)4" vs `Q2_2024:Minestats2!F2` "Porgera (24.5%)1"; `Q1_2024:QTD!A15` and `Q4_2024:QTD!A15` both "Porgera (24.5%)4" | Values fit 24.5%, not 47.5%: `Q3_2024:QTD!B15` 72 × 0.245 = 17.6 ≈ `C15` 18 (× 0.475 = 34.2) | 47.5% was Barrick's share before the 2020 shutdown (50% of Barrick Niugini's 95%). After the December 2023 restart it is 24.5% (50% of BNL's 49%). The Q2/Q3 2024 summary label is a stale pre-2020 template (INFERENCE, background knowledge) | CONFIRMED (label conflict); values at 24.5% |
| W1-DR-02 | `Q1_2024`…`Q4_2024:Minestats4!A15`/`A16` "Production/Sales (millions of pounds)1" (also the second block, e.g. `A50`/`A51` in H1/9M/FY files) | Values equal the summary kt values (T4 passes only under kt); `Q1_2025:Minestats4!A15` reads "Production (Thousands of tonnes)1"; footnote `Q1_2024:Minestats4!A29` says quantities moved to tonnes | stale unit label for the whole of 2024 | CONFIRMED (label); import as kt |
| W1-DR-03 | `Q3_2025:QTD!A32`, `Q3_2025:YTD!A32`, `Q3_2025:Minestats3!A35`, `A77` | "loss of control on **September 16**, 2025"; Q2 2025 (`Q2_2025:QTD!A32`) and Q4 2025 → (`Q4_2025:QTD!A34`) say **June 16**, 2025 | Q3 2025 footnote error (June 16 is in both the earlier and later vintages) | CONFIRMED (text conflict); June 16 preferred (INFERENCE) |
| W1-DR-04 | `Q3_2024:Minestats2!B12` (and `B13` autoclave) = 1,605 (PV 2024Q3 processed, 60%) | H1 `Q2_2024:Minestats2!B61` 2,878 + 1,605 = 4,483 ≠ 9M `Q3_2024:Minestats2!B61` 4,353 (−130). FY `Q4_2024:Minestats2!B61` 5,730 = 9M 4,353 + Q4 1,377, so the 9M figure is consistent onward. Implied Q3 = 1,475 kt. Metal balance (98 koz / (0.78 × 2.58 g/t / 31.1035)) ≈ 1,515 kt sits closer to 1,475 | Q3 quarterly cell overstated by 130 kt (keying) | PLAUSIBLE |
| W1-DR-05 | `Q2_2024:QTD!B5` = 327 (Carlin 2024Q2, 100%); repeated as comparative `Q2_2025:QTD!I5` | 327 × 0.615 = 201.1 ≠ `C5` 202; `Q1_2024:QTD!B5` 334 + 327 = 661 ≠ `Q2_2024:YTD!B5` 663; FY quarter sum 1,258 ≠ `Q4_2024:YTD!B5` 1,261. 329 satisfies T1 and both T5 checks (FY then 1,260; a 2024Q4 of 302 would close it too, and 2024Q4 301 is also a T1 fail) | keying error in the 100%-basis cell (same pattern as C1 DR-01) | PLAUSIBLE |
| W1-DR-06 | `Q4_2024:Minestats1!O49` `—` (Hemlo 2023Q4 total capex) vs `Q4_2024:Minestats1!O109` 41 (FY2023) | 2023 quarters 12 (`Q1_2024:Minestats1!O49`) + 9 + 12 + `—` = 33 ≠ 41 | 2023Q4 capex of ~8 $M suppressed as `—` | PLAUSIBLE |
| W1-DR-07 | `Q4_2025:YTD!E16` 4,271 and `G16` 1,603 (L-G FY2025 CoS, AISC; 80%) vs `Q4_2025:QTD!E16` 4,151 and `G16` 1,448 | FY sold `Q4_2025:YTD!D16` 91 = Q4 sold `QTD!D16` 91, yet the FY unit costs differ. Implied extra FY numerator: (1,603 − 1,448) × 91 = $14.1M AISC; (4,271 − 4,151) × 91 = $10.9M CoS | FY numerators include Q1–Q3 2025 costs, when per-ounce data were suppressed and no ounces were sold (INFERENCE) | PLAUSIBLE; closer: Q4 2025 MD&A p.61 per-site table |
| W1-DR-08 | `Q1_2024:QTD!G23` 1,479 (Total Gold AISC 2024Q1, first publication) vs `Q1_2025:QTD!N22` 1,474 (comparative) | silent restatement of −$5/oz; a search of all Q1 2025 footnotes for "restat/revis/recast/comparative" finds nothing about it | unannounced restatement | CONFIRMED (value change); cause OPEN |
| W1-DR-09 | T16 others: Tongon 9M 2024 OP-mined grade `Q3_2024:Minestats3!F57` 1.83 → `Q3_2025:Minestats3!G58` 1.86; TR H1 2025 sustaining/project capex `Q2_2025:Minestats1!F107` 29 / `F108` 1 → `Q2_2026:Minestats1!G112` 30 / `G113` `—`; Porgera 2024Q2/H1 heap-leach `Q2_2024:Minestats2!F14`, `F27`, `F63`, `F76` `—` → `n/a` in Q2 2025; L-G 2025Q2 strip `Q2_2025:Minestats3!B8` blank → `Q2_2026:Minestats3!C8` `—` | small comparative restatements and token flips | restatements without footnotes; token semantics drift | CONFIRMED (changes) |
| W1-DR-10 | Copper-Total recovery `Minestats4!B14` and comparative `C14` = **0.94 in all 10 files (20 quarterly cells)**, e.g. `Q2_2024:Minestats4!B14` 0.94 while Lumwana `D14` 0.85 and Jabal Sayid `H14` 0.93 (Zaldívar `F14` `n/a`) | no weighting of 85% and 93% can give 94%. Extends C1 DR-06 (2026 only) to every vintage since Q1 2024 | hard-coded or stale aggregate, or a definition that differs from site recovery | PLAUSIBLE (high) |
| W1-DR-11 | Hemlo 2024Q1 `Q1_2024:QTD!B11` 38 vs `C11` 37 (repeated `Q1_2025:QTD!I10`/`J10`); Lumwana 9M 2024 `Q3_2024:YTD!B28` 78 vs `C28` 77 | 100%-owned sites (no % printed) where 100% basis ≠ attributable | rounding at different stages, or a small non-attributable stream | OPEN (low) |
| W1-DR-12 | token mixes, e.g. Phoenix heap leach `Q2_2024:Minestats1!H35` `n/a` (Q) vs `H95` `—` (H1); Cortez autoclave `Q4_2024:Minestats1!D34` `n/a` vs `D94` `—`; TR roaster 2025 `n/a` ×3 then `Q4_2025:Minestats1!F14` 4 | `n/a` and `—` used for the same "nothing" in quarter vs cumulative blocks (24 T5 fails) | presentation convention, not data | CONFIRMED (format); import both as Nil, keep the token |
| W1-DR-13 | Hemlo 2025Q4 `Q4_2025:Minestats1!N44` 1,976, `N43` 1,707, `N47` 8, `N36` 27 | AISC − TCC − sustaining capex/oz = −27 (the only T11 fail) | stub period to 26 Nov 2025 closing; cash vs accrual timing | PLAUSIBLE (low) |
| W1-DR-14 | `Q1_2025_AISC:Q1 2025!N19`, `N20`, `N21`, `N23` = 0 (also `Q2_2025_AISC:Q2 2025!N19:N23` and `Q3_2025_AISC:Q3 2025!L19:L23`) vs Mine Stats `Q1_2025:QTD!D16:G16` `'— '` | AISC workbook stores numeric 0 (accounting format shows "-") for not-provided L-G values | format; zero would poison averages | CONFIRMED (format); import as Nil |
| W1-DR-15 | Text defects: `Q3_2024:QTD!A34` and `Q3_2024:Minestats2!A42` say "Refer to our Q2 2024 Report"; `Q4_2025:Minestats1!A64` "Decemer"; `Q2_2024:Endnote!A5` begins "“Total cash co“Total cash costs”"; the three endnotes (`A5` gold TCC/AISC, `A8` copper C1/AISC, `A11` capex) cite different start pages: Q4 2024 61/61/60, Q3 2025 43/44/43, Q4 2025 60/60/59. In Q3 2025 the copper endnote cites p.44, which is the "Capital Expenditures" page; the copper C1 reconciliation starts on p.56 | stale cross-reference, typo, garbled paste, imprecise page references | copy-forward drafting | CONFIRMED (text) |
| W1-DR-16 | Q2 2026: 1,092 `#REF!` formula cells (`Minestats3!M4:V81`, `Minestats4!K4:R67`); YTD ranges to `XFD`/`XEM`/`WSF`/`WMQ` with 1,134–1,169 merged ranges past column 40 | workbook residue | hygiene; parser must bound its columns | CONFIRMED |
| W1-DR-17 | `Q1_2025:QTD!B16` 22 / `C16` 18 and `Q1_2025:Minestats3!B12` 169, `B18` 3.54, `B19` 0.91, `B34` 14 vs the later footnote `Q4_2025:QTD!A34` "no operating or per ounce data was provided for Q1 2025 to Q3 2025" | The first-publication footnote `Q1_2025:Minestats3!A35` says only "no per ounce data is provided", so the operating data were published deliberately. The later footnote is wrong for Q1 2025 | extends and upgrades C1 DR-09 | CONFIRMED (text conflict) |

**C1 register reconfirmed on the full set.**

| C1 ID | Result here |
|---|---|
| DR-01 | Carlin 2026Q1: T1 and T5 fail |
| DR-02 | TR H1 2026: T1 and T5 fail |
| DR-03 | Phoenix 2026: T1 fails |
| DR-04 | Jabal Sayid 2026: T1 and T5 fail |
| DR-05 | Hemlo H1 2026 CoS 1,691: T4 and T5b fail |
| DR-06 | now W1-DR-10 |
| DR-07 | 7 zero recoveries: T14 fails |
| DR-08 | Phoenix autoclave token flip: T5 token mix |
| DR-09 | now W1-DR-17 |
| DR-11 | T8: 69 positive residuals across all vintages |
| DR-12 | T3: aggregation layer in every period, see §11 |

**Note on W1-DR-01.** The ownership history of Porgera (Barrick 47.5% before 2020; 24.5% after the 2023 restart, when PNG stakeholders took 51% and Barrick Niugini 49%) comes from background knowledge, not from these files, so it is INFERENCE. The workbooks only show that the two labels conflict and that the values fit 24.5%.

---

## 7. Lumwana series (copper; 100% owned, no percentage printed, so 100% = attributable)

**Sources.**
- Columns 2–8: `QN_YYYY:Minestats4!D9` (processed kt), `D13` (grade), `D14` (recovery), `D15` (production kt), `D16` (sales kt), `D4` (mined kt), `D8` (strip).
- C1 and AISC ($/lb): `QTD!F28`/`G28` in 2024 files and `F27`/`G27` from 2025.
- 2023 values come from the comparative column `E` (and summary `M`/`N`) of the 2024 files.
- Every value is first publication.

**Implied recovery** = production / (processed × grade).

| Period | Processed kt | Grade % Cu | Recovery | Cu prod kt | Cu sold kt | Mined kt | Strip | C1 $/lb | AISC $/lb | Implied rec. |
|---|---|---|---|---|---|---|---|---|---|---|
| 2023Q1 | 6,523 | 0.38 | 87% | 22 | 22 | 17,178 | 2.76 | 3.09 | 3.98 | 0.888 |
| 2023Q2 | 6,578 | 0.50 | 93% | 30 | 29 | 26,919 | 2.44 | 2.30 | 3.29 | 0.912 |
| 2023Q3 | 6,606 | 0.55 | 91% | 33 | 30 | 37,455 | 4.66 | 1.86 | 3.41 | 0.908 |
| 2023Q4 | 7,090 | 0.54 | 87% | 33 | 32 | 32,081 | 3.58 | 2.14 | 3.38 | 0.862 |
| 2024Q1 | 6,022 | 0.41 | 88% | 22 | 22 | 29,571 | 6.93 | 2.52 | 4.33 | 0.891 |
| 2024Q2 | 6,523 | 0.45 | 85% | 25 | 25 | 39,132 | 6.03 | 2.14 | 4.36 | 0.852 |
| 2024Q3 | 6,380 | 0.53 | 91% | 30 | 26 | 36,809 | 4.96 | 2.53 | 3.94 | 0.887 |
| 2024Q4 | 6,858 | 0.71 | 93% | 46 | 36 | 35,354 | 2.34 | 1.89 | 3.14 | 0.945 |
| 2025Q1 | 5,237 | 0.57 | 91% | 27 | 34 | 30,310 | 4.05 | 2.22 | 3.20 | 0.904 |
| 2025Q2 | 7,082 | 0.67 | 92% | 44 | 39 | 37,481 | 3.89 | 1.58 | 2.79 | 0.927 |
| 2025Q3 | 6,392 | 0.66 | 92% | 38 | 37 | 41,678 | 2.97 | 1.68 | 2.93 | 0.901 |
| 2025Q4 | 7,029 | 0.65 | 91% | 42 | 47 | 32,205 | 2.86 | 1.97 | 3.24 | 0.919 |
| 2026Q1 | 5,911 | 0.59 | 91% | 32 | 30 | 31,064 | 4.37 | 2.67 | 3.83 | 0.918 |
| 2026Q2 | 6,946 | 0.64 | 91% | 41 | 40 | 41,542 | 5.74 | 2.35 | 3.83 | 0.922 |

**Reading the series.**
- **Metal balance.** Lumwana passes T9 in every period: implied recovery is within 3 points of the stated figure.
- **Recovery regime (INFERENCE).** Stated recovery moves between **85% and 93%** in 2023–24 but stays at **91–92%** in 2025–26. This is consistent with steadier feed grade (0.57–0.67% vs 0.38–0.71%).
- **Sales vs production.** Sales above production in 2025Q1 (34 vs 27 kt) and 2025Q4 (47 vs 42), and well below it in 2024Q4 (36 vs 46), point to concentrate-shipment timing (INFERENCE).
- **Throughput.** It sits in the 5.2–7.1 Mt/quarter range, with a Q1 seasonal dip every year (wet season, INFERENCE).

---

## 8. North Mara series (gold; 84% attributable)

**Sources.**
- Columns 2–6: `QN_YYYY:Minestats3!H12` (processed kt), `H18` (processed grade g/t), `H19` (recovery), `H20` (attributable production koz), `H21` (attributable sales koz).
- 100%-basis production, TCC and AISC: summary `QTD!B20`/`F20`/`G20` in 2024 files and `B19`/`F19`/`G19` from 2025.
- 2023 values come from the comparative columns `I` (Minestats) and `I`/`M`/`N` (summary) of the 2024 files.
- Grade and recovery do not depend on basis. Tonnes and ounces are at 84%; the 100% column is total mine output.

**Implied recovery** = attr. production / (processed × grade / 31.1035).

| Period | Processed kt (84%) | Grade g/t | Recovery | Prod koz (84%) | Sold koz (84%) | Prod koz (100%) | TCC $/oz | AISC $/oz | Implied rec. |
|---|---|---|---|---|---|---|---|---|---|
| 2023Q1 | 716 | 3.25 | 92% | 68 | 70 | 81 | 759 | 1,137 | 0.909 |
| 2023Q2 | 698 | 3.08 | 92% | 64 | 64 | 77 | 942 | 1,355 | 0.926 |
| 2023Q3 | 715 | 2.91 | 92% | 62 | 59 | 73 | 999 | 1,429 | 0.927 |
| 2023Q4 | 719 | 2.85 | 91% | 59 | 61 | 71 | 1,103 | 1,449 | 0.896 |
| 2024Q1 | 651 | 2.43 | 90% | 46 | 46 | 55 | 1,339 | 1,753 | 0.904 |
| 2024Q2 | 715 | 2.61 | 89% | 54 | 50 | 63 | 1,266 | 1,491 | 0.900 |
| 2024Q3 | 682 | 3.84 | 90% | 75 | 78 | 89 | 850 | 1,052 | 0.891 |
| 2024Q4 | 724 | **4.29** | 90% | 90 | 89 | 107 | 771 | **1,098** | 0.901 |
| 2025Q1 | 672 | 3.56 | 88% | 67 | 68 | 80 | 986 | 1,258 | 0.871 |
| 2025Q2 | 698 | 3.16 | 88% | 62 | 50 | 74 | 1,073 | 1,292 | 0.874 |
| 2025Q3 | 729 | 2.99 | 89% | 64 | 72 | 75 | 1,069 | 1,268 | 0.913 |
| 2025Q4 | 683 | 2.87 | 90% | 56 | 56 | 67 | 1,237 | 1,546 | 0.889 |
| 2026Q1 | 664 | **1.81** | **85%** | 33 | 36 | 39 | 1,758 | 1,990 | 0.854 |
| 2026Q2 | 688 | **1.80** | **85%** | 35 | 37 | 41 | 2,164 | **2,577** | 0.879 |

**Reading the series.**
- **Throughput.** It is flat (651–729 kt/qtr attributable).
- **Grade and AISC.** The grade swing drives everything. It peaked in 2024Q3–Q4 at 3.84–4.29 g/t, then fell steadily to 1.80 g/t in 2026Q2, and AISC moved inversely ($1,052 → $2,577). In the 2026 quarters recovery also dropped to 85%, the lowest in the series.
- **Ore source (FACT, read from the cells).** Q2 2026 open-pit ore mined was only 109 kt against 4,628 kt of waste (strip 42.46, `Q2_2026:Minestats3!H8`). Q1 2026 had 7 kt ore and strip 514.14 (`Q1_2026:Minestats3!H5`, `H8`).
- **Interpretation (INFERENCE).** Mill feed in 2026 is therefore mostly underground ore plus low-grade stockpile while a pit stripping phase runs.
- **Metal balance.** T9 passes in every period. Note 2025Q2: sold 50 vs produced 62.
- **Consolidation.** North Mara sits within the 84%-owned Tanzanian structure. Consolidated reporting would need 100% figures (column B), which the summary provides.

---

## 9. Loulo-Gounkoto (80% attributable)

| Period | Status | Values (100% / attributable; unit costs per attributable oz) | Locators |
|---|---|---|---|
| 2024Q1–Q4 | full data | e.g. 2024Q4: produced 196 (100%) / 156 (attr.), **sold 47**, AISC 2,136 | `Q4_2024:QTD!B17`, `C17`, `D17`, `G17` |
| 2025Q1 | **operating data published, per-ounce data not** | produced 22 / 18; processed 169 kt, grade 3.54 g/t, recovery 91%, total capex 14 $M; sold, CoS, TCC and AISC are `'— '` | `Q1_2025:QTD!B16:G16`; `Q1_2025:Minestats3!B12`, `B18`, `B19`, `B34` |
| 2025Q2 | **NOT_PUBLISHED** (operating and per-ounce) | every operating cell is `—`/`'— '`, but **total capex 4 $M is published**; strip-ratio cell is blank | `Q2_2025:QTD!B16`; `Q2_2025:Minestats3!B34`, `B8` |
| 2025Q3 | **NOT_PUBLISHED** (including capex) | all `—` | `Q3_2025:QTD!B16`; `Q3_2025:Minestats3!B34` |
| 2025Q4 | resumed after handback | produced 13 / 11, sold 91, CoS 4,151, TCC 1,448, AISC 1,448; processed 97 kt, grade 3.84, recovery 89%; capex `—` | `Q4_2025:QTD!B16:G16`; `Q4_2025:Minestats3!B12`, `B18`, `B19`, `B34` |
| FY2025 | | produced 35 / 29, sold 91, CoS 4,271, AISC 1,603 (W1-DR-07) | `Q4_2025:YTD!B16:G16` |
| 2026Q1 | normal | produced 80 / 64, sold 69 (attr.), AISC 1,933; capex `—` | `Q1_2026:QTD!B16`, `C16`, `G16`; `Q1_2026:Minestats3!B21`, `B34` |
| 2026Q2 | normal | produced 109 / 88, AISC 2,240, total capex 30 $M (28 sustaining + 2 projects) | `Q2_2026:QTD!B16`, `C16`, `G16`; `Q2_2026:Minestats3!B34:B36` |

**Footnote chronology** (FACT unless marked):
- **Suspension.** Operations were suspended from **January 14, 2025** (`Q1_2025:Minestats3!A35`).
- **Loss of control.** Dated **June 16, 2025** in `Q2_2025:QTD!A32` and in all Q4 2025+ files, but **September 16, 2025** in all four Q3 2025 footnotes (W1-DR-03).
- **Settlement.** Agreement with the Government of Mali announced **November 24, 2025** (`Q4_2025:QTD!A34`).
- **Handback.** The provisional administration ended **December 16, 2025**, when operational control returned to Somilo and Gounkoto management (`Q4_2025:QTD!A34`).
- **INFERENCE.** The 2025Q4 figures (91 koz sold vs 11 koz produced, AISC = TCC = 1,448 with capex `—`) look like a post-handback stub that sold inventoried gold.

**NOT_PUBLISHED quarters for an importer.** 2025Q2 and 2025Q3 in full, and the per-ounce part of 2025Q1.

---

## 10. Hemlo, Tongon, Long Canyon and Porgera: exits and gaps

**Hemlo** (100% owned).
- **Sale.** Agreement signed September 10, 2025: "we reached an agreement to sell the Hemlo gold mine to Carcetti Capital Corp. for gross proceeds of up to $1.09 billion." The transaction closed on **November 26, 2025**, and the results run to that date (`Q4_2025:QTD!A32`).
- **Last quarter.** 2025Q4 (stub): 26 koz (`Q4_2025:QTD!B11`), AISC 1,976 (`G11`).
- **Disappearance.** From the **Q1 2026 file** every Hemlo summary cell is `'— '` (e.g. `Q1_2026:QTD!B11`, `G11`). The exceptions are the numeric-0 recoveries `Q1_2026:Minestats1!N27`/`N28` and `Q2_2026:Minestats1!N27`/`N28` (DR-07), and the H1 2026 CoS 1,691 at `Q2_2026:YTD!E11` (DR-05).
- **Column kept.** The Hemlo column still exists in 2026 files (`Q1_2026:Minestats1!N2` "Hemlo2").

**Tongon** (89.7%).
- **Sale.** Agreement signed October 6, 2025 "to sell our interest in the Tongon gold mine … to the Atlantic Group for total consideration of up to $305 million". It closed on **December 1, 2025**, with results to closing (`Q4_2025:QTD!A35`).
- **Last quarter.** 2025Q4 (stub): 20 koz 100% / 18 koz attr. (`Q4_2025:QTD!B18`, `C18`), AISC 2,844 (`G18`).
- **Disappearance.** All 2026 cells are `—`. Recovery is numeric 0 at `Q1_2026:Minestats3!F19`, `Q2_2026:Minestats3!F19` and `F64`. Revenue is blank at `Q2_2026:Minestats3!F23`, `F68`.

**Long Canyon** (61.5% via NGM).
- **Dropped from 2024.** From the first quarter of 2024, "we have ceased to include production or non-GAAP cost metrics for Long Canyon as it was placed on care and maintenance" (`Q1_2024:QTD!A32`).
- **Column history.** The column holds `—` throughout 2024 (e.g. `Q1_2024:QTD!A9` "Long Canyon (61.5%)2") and is removed in Q1 2025.
- **2023 comparatives.** NGM 2023 includes Long Canyon: "our 61.5% interest in Carlin, Cortez, Turquoise Ridge, Phoenix and Long Canyon until it transitioned to care and maintenance at the end of 2023" (`Q1_2024:QTD!A33`).

**Porgera** (24.5%).
- **Care and maintenance.** "Porgera was placed on care and maintenance from April 25, 2020 until December 22, 2023" (`Q1_2024:QTD!A34`).
- **Data gap.** No operating data exist for 2020Q3–2023Q4, so the 2023 comparatives are `—`.
- **Blank cells.** `Q4_2024:Minestats2!G79`/`G85` (2023FY CoS/AISC) are blank, not `—`.
- **Label.** See W1-DR-01.

**Pierina.** It is excluded from the totals: "producing incidental ounces until December 31, 2023 while in closure" (`Q1_2024:QTD!A35`).

---

## 11. Corporate / unallocated layer (C1 DR-12, extended to every period)

The layer is Total Gold AISC (summary column G, or N for comparatives) minus the regional AISC weighted by attributable ounces sold. Values are in $/oz, first publication. 2023 figures come from the 2024 files.

| Period | Layer | Period | Layer | Period | Layer |
|---|---|---|---|---|---|
| 2023Q1 | +33.64 | 2024Q1 | +35.29 | 2025Q1 | +53.19 |
| 2023Q2 | +27.46 | 2024Q2 | +23.57 | 2025Q2 | +47.54 |
| 2023Q3 | +24.31 | 2024Q3 | +44.47 | 2025Q3 | **+88.66** |
| 2023Q4 | +38.57 | 2024Q4 | +26.34 | 2025Q4 | +55.62 |
| FY2024 | +32.62 | FY2025 | +61.00 | 2026Q1 / Q2 | +55.36 / +55.71 |

**Reading the table.**
- The layer is always positive.
- It roughly doubled from 2024 (about $25–45) to 2025–26 (about $47–89).
- The 2025Q3 peak (`Q3_2025:QTD!G22` 1,538 on 837 koz sold) falls in the only 2025 quarter in which L-G published nothing at all, capex included. Any causal link is unproven (INFERENCE).
- **Modelling consequence.** It must be represented as an explicit corporate and JV-level layer (G&A, other, community, rehabilitation at non-operating sites; INFERENCE as to composition), not as a reconciliation error. The AISC workbooks (§12) list "General & administrative costs" as a separate AISC component row, which is where most of it is expected to sit (INFERENCE).

---

## 12. AISC reconciliation workbooks (T18)

**Layout.** Each workbook's first sheet is named after its quarter (e.g. "Q3 2025"). Further sheets hold the prior quarter, the prior-year quarter and YTD:
- Q4 2024 sheets: "Q4 2024 / Q3 2024 / Q4 2023 / YTD Dec 31 2024 / YTD Dec 31 2023".
- **Q1 2026 also carries "YTD Q3 2025 / YTD Q3 2024", and Q2 2026 carries them again.** These are stale sheets, a minor hygiene issue.

**Row structure** (`Q3_2025_AISC:Q3 2025!A5:A24`):
- Cost of sales applicable to gold production, Depreciation, By-product credits, Non-recurring items, Other, Non-controlling interests → **Total cash costs**;
- G&A, Minesite exploration and evaluation, Minesite sustaining capital, Sustaining capital leases, Rehabilitation (accretion and amortization, operating sites), Non-controlling interests → **All-in sustaining costs**;
- Ounces sold (attributable basis), the per-ounce rows, and co-product per-ounce rows.

**2026 changes.** "By-product credits" is renamed **"Costs allocated to by-products"**, "Non-recurring items" is dropped, and the per-ounce labels become "COS/oz", "TCC/oz" and "AISC/oz" (`Q2_2026_AISC:Q2 2026!A7`, `A19:A21`).

**Value for the demonstrator.** These workbooks give **per-site AISC components in $M**, which fills C1's checked null on per-site AISC build-up from Q3 2024 onward. On the first (current-quarter) sheet, per-unit values match Mine Stats within rounding (≤ $1/oz, ≤ $0.01/lb) in 420 of 432 cells. The only exceptions are the 12 L-G zero placeholders (W1-DR-14).

**Header suffix.** The site header for L-G carries a footnote letter ("Loulo-Gounkotok" at `Q1_2025_AISC:Q1 2025!N4`), so header matching must strip one trailing letter.

---

## 13. Import rules this adds to C1 §3.1 (for `fmine-public`)

1. **Anchor on labels.**
   - Find blocks by "For the (three|six|nine|twelve) months".
   - Find sites from the row above the block, carrying the name across merged blanks.
   - Strip trailing footnote digits (`^(.*?[^\d\s])(\d{1,2})$`) from labels. Detect "100%" before stripping digits.
2. **Version the region map by file.**
   - v1 (to Q3 2025): North America = NGM + Hemlo; LatAm & AP = PV + Veladero + Porgera.
   - v2 (Q4 2025 on): North America = NGM + PV + Hemlo; SA & AP = Veladero + Porgera.
   - Never compare region aggregates across the boundary.
3. **Copper units.** Treat 2024 `Minestats4` "millions of pounds" production and sales as **kt** (W1-DR-02). Cost metrics stay in $/lb.
4. **Missing-value tokens.** `—`, `'— '`, `n/a`, blank, and numeric 0 in recovery cells of divested sites all map to Nil with the token kept. In AISC workbooks, numeric 0 where Mine Stats shows `—` also maps to Nil (W1-DR-14).
5. **Bound the column scan** to ≤ 20 and ignore formula cells (Q2 2026 `#REF!`).
6. **Vintages.** Keep both first publication and later comparative values (bitemporal). T16 finds 9 unannounced changes (values or tokens) in 6,400 cells (0.14%).
7. **Ownership.**
   - Parse the printed `(x%)`. Where none is printed, record `unprinted` (Hemlo, Lumwana; 100%).
   - Treat the Porgera summary label in Q2/Q3 2024 as an error: use 24.5% (W1-DR-01).
   - All NGM figures are tagged 61.5%-attributable (Newmont 38.5%).

---

## 14. Still missing, and why

| Item | Status | Reason / next check |
|---|---|---|
| Q3 2026 Mine Stats, MD&A, AISC xlsx | **not available** | Release scheduled for **2026-11-09 06:00 ET**. CDN 404 and empty feed entry as of 2026-10-29T03:36Z. Re-probe `quarterly_results/2026/q3/` after the release |
| AISC reconciliation xlsx for Q1 2024 and Q2 2024 | not published | The IR feed lists none for those quarters; per-site AISC components for H1 2024 exist only in the MD&A PDFs (Q1 2024 p.56+, Q2 2024 p.60+) |
| Per-site GEO | absent from every workbook | Barrick reports GEO only at group level (C1 FM deck); per-site GEO must be derived from by-product credits (INFERENCE) |
| 2023 quarterly data | comparative columns only | 2023 quarterly workbooks were out of scope; 2023 values here are as restated in the 2024 files |
| L-G 2025Q2–Q3 operating data, 2025Q1 per-ounce data | NOT_PUBLISHED by Barrick | loss of control; see §9 |
| barrick.com HTML pages | 403 to curl | read with WebFetch (no cookies); all files came from the public q4cdn host |
| Cause of W1-DR-04/05/07/08 | OPEN | closer: the per-site tables in the MD&A pages listed in §2.1 |

---

## 15. Evidence rows (E451–E499)

**Ownership basis.** "100%" means 100% basis; "attr." means Barrick attributable share; NGM means 61.5% Barrick / 38.5% Newmont.

**Locators.** Every locator points to a file in `corpus_of_input_docs/downloaded/mine_stats/`. Unit costs are per attributable unit sold.

| E# | Claim | Source + locator | Quote (≤ 25 words) | Type |
|---|---|---|---|---|
| E451 | Mine Stats xlsx for all 10 quarters Q1 2024–Q2 2026 were retrieved over HTTP 200 from Barrick's public CDN | `w1/manifest.tsv`; e.g. `https://s25.q4cdn.com/322814910/files/doc_financial/quarterly_results/2025/q3/Barrick_Q3_2025_Mine_Stats.xlsx` | "quarterly_results/2025/q3/Barrick_Q3_2025_Mine_Stats.xlsx … 2026-10-29T03:35:01Z 200" | FACT |
| E452 | The Q1 2026 and Q2 2026 downloads are byte-identical to the existing corpus files | sha256 of `Barrick_Q1_2026_Mine_Stats.xlsx`, `Barrick_Q2_2026_Mine_Stats.xlsx` vs corpus copies | "ddef08dd544f…c5d2" / "cf0d3f4fea9c…df8e" | FACT |
| E453 | Q1 2025 files sit under folder `2025/01/`, not `2025/q1/` | manifest URL for `Barrick_Q1_2025_Mine_Stats.xlsx` | "quarterly_results/2025/01/Barrick_Q1_2025_Mine_Stats.xlsx" | FACT |
| E454 | Q3 2026 results are scheduled for 9 Nov 2026, 06:00 ET | `Barrick-to-Report-Third-Quarter-2026-Results-on-November-9.pdf` p.1 | "will release its third quarter 2026 results before markets open on Monday, November 9, 2026, at 6:00 a.m. ET" | FACT |
| E455 | As of 2026-10-29T03:36:32Z the Q3 2026 workbook, MD&A and AISC xlsx are not on the CDN | HEAD probes of `2026/q3/` (also `2026/03`, `/10`, `/11`) | "HTTP 404" | FACT |
| E456 | Summary column B is 100% basis; column C is attributable | `Q2_2026:QTD!B4`, `C4` | "Gold produced (000s ozs 100% basis)" / "Gold produced (000s ozs attributable share)" | FACT |
| E457 | NGM figures are Barrick's 61.5% share | `Q2_2026:QTD!A31` | "These results represent our 61.5% interest in Carlin, Cortez, Turquoise Ridge and Phoenix." | FACT |
| E458 | Minestats capex is at Barrick's share, cash basis, and includes capitalized interest | `Q1_2025:Minestats1!A54` | "Capital expenditures are presented on a cash basis and are reported at Barrick's share. Includes Capitalized Interest." | FACT |
| E459 | The Q2 2026 non-GAAP reconciliations start on MD&A p.44 | `Q2_2026:Endnote!A5`; `Barrick_Q2_2026_MD-A.pdf` p.44 | "provided starting on page 44 in the Second Quarter 2026 Report filed on SEDAR+" | FACT |
| E460 | The Q4 2024 endnotes cite p.61 for gold TCC/AISC and p.60 for capex | `Q4_2024:Endnote!A5`, `A11` | "starting on page 61 in the Q4 and Year End 2024 Report" … "starting on page 60" | FACT |
| E461 | From Q4 2025, PV is in North America and comparatives are restated | `Q4_2025:QTD!A33` | "Starting Q4 2025, we have presented Pueblo Viejo as part of North America instead of South America & Asia Pacific." | FACT |
| E462 | Copper quantities have been in tonnes since 2024 | `Q1_2024:QTD!A36` | "Starting in 2024, we have presented our copper production and sales quantities in tonnes rather than pounds" | FACT |
| E463 | 2024 Minestats4 still labels copper production in millions of pounds, though the values are kt | `Q1_2024:Minestats4!A15`; T4 passes only under kt | "Production (millions of pounds)1" | FACT (label); INFERENCE (stale) |
| E464 | Header typo in the Q4 2025 twelve-month block | `Q4_2025:Minestats1!A64` | "For the Twelve months ended Decemer 31," | FACT |
| E465 | The Q2 2024 endnote text is garbled | `Q2_2024:Endnote!A5` | "“Total cash co“Total cash costs” per ounce and “All-in sustaining costs” per ounce are non-GAAP" | FACT |
| E466 | Q2 2026 contains 1,092 `#REF!` formula cells outside the data blocks | `Q2_2026:Minestats3!M4:V81` (660), `Q2_2026:Minestats4!K4:R67` (432) | "#REF!" | FACT |
| E467 | The Q2 2024 YTD sheet's used range spans all 16,384 columns, with 1,184 merged ranges | `Q2_2024:YTD` dimensions | "dims=A1:XFD41 … merged=1184" | FACT |
| E468 | The Porgera label reads 47.5% in the Q2/Q3 2024 summary but 24.5% in the Minestats sheet | `Q2_2024:QTD!A15` vs `Q2_2024:Minestats2!F2` | "Porgera (47.5%)4" / "Porgera (24.5%)1" | FACT |
| E469 | Porgera Q3 2024 values fit 24.5% (72 × 0.245 = 17.6 ≈ 18), not 47.5% | `Q3_2024:QTD!B15` 72 (100%), `C15` 18 (attr.) | "72" / "18" | INFERENCE |
| E470 | PV 2024 processed (60%): H1 2,878 + Q3 1,605 ≠ 9M 4,353 | `Q2_2024:Minestats2!B61`, `Q3_2024:Minestats2!B12`, `Q3_2024:Minestats2!B61` | "2878" / "1605" / "4353" | FACT (cells); INFERENCE (Q3 overstated 130 kt) |
| E471 | Carlin 2024Q2 100% value 327 fails both T1 and H1 additivity (334 + 327 ≠ 663) | `Q2_2024:QTD!B5`, `C5`; `Q1_2024:QTD!B5`; `Q2_2024:YTD!B5` | "327" / "202" / "334" / "663" | FACT (cells); INFERENCE (keying) |
| E472 | Hemlo FY2023 capex 41 $M exceeds the sum of its quarters (12 + 9 + 12 + "—") | `Q4_2024:Minestats1!O109`, `O49`; `Q1_2024:Minestats1!O49` | "41" / "—" | FACT |
| E473 | L-G FY2025 CoS and AISC differ from Q4 although FY sold = Q4 sold = 91 koz (attr.) | `Q4_2025:YTD!D16`, `E16`, `G16` vs `Q4_2025:QTD!D16`, `E16`, `G16` | "4271" / "1603" vs "4151" / "1448" | FACT |
| E474 | Total Gold 2024Q1 AISC was restated from 1,479 to 1,474 with no footnote | `Q1_2024:QTD!G23` vs `Q1_2025:QTD!N22` | "1479" / "1474" | FACT |
| E475 | Copper-Total recovery is 94% in every quarterly cell Q1 2024–Q2 2026, including 2024Q2 when Lumwana was 85% and Jabal Sayid 93% | `Q2_2024:Minestats4!B14`, `D14`, `H14` (all 10 files, `B14`/`C14`) | "0.94" / "0.85" / "0.93" | FACT (cells); INFERENCE (stale aggregate) |
| E476 | The Q3 2025 file dates the L-G loss of control to September 16; Q2 2025 and Q4 2025+ say June 16 | `Q3_2025:QTD!A32` vs `Q2_2025:QTD!A32` | "subsequent loss of control on September 16, 2025, no operating or per ounce data is provided" | FACT |
| E477 | L-G 2025Q1 production was published: 22 koz (100%), 18 koz (80% attr.); per-ounce cells are "—" | `Q1_2025:QTD!B16`, `C16`, `D16:G16` | "22" / "18" / "— " | FACT |
| E478 | The Q1 2025 footnote withheld only per-ounce data | `Q1_2025:Minestats3!A35` | "As a result of temporary suspension of operations at Loulo-Gounkoto starting January 14, 2025, no per ounce data is provided" | FACT |
| E479 | L-G 2025Q2 operating data are not provided, yet capex of 4 $M (attr.) is published | `Q2_2025:QTD!B16` `'— '`; `Q2_2025:Minestats3!B34` | "— " / "4" | FACT |
| E480 | L-G 2025Q3 is fully NOT_PUBLISHED, capex included | `Q3_2025:QTD!B16`; `Q3_2025:Minestats3!B34` | "— " / "—" | FACT |
| E481 | L-G 2025Q4: 13 koz (100%) / 11 (attr.), 91 koz sold, CoS 4,151, TCC = AISC = 1,448 | `Q4_2025:QTD!B16`, `C16`, `D16`, `E16`, `F16`, `G16` | "13" / "11" / "91" / "4151" / "1448" / "1448" | FACT |
| E482 | L-G operational control returned on 16 Dec 2025 after the Mali agreement of 24 Nov 2025 | `Q4_2025:QTD!A34` | "The provision administration of the Loulo-Gounkoto complex was terminated on December 16, 2025" | FACT |
| E483 | L-G 2026: Q1 80 / 64 koz, AISC 1,933; Q2 109 / 88 koz, AISC 2,240, total capex 30 $M | `Q1_2026:QTD!B16`, `C16`, `G16`; `Q2_2026:QTD!B16`, `C16`, `G16`; `Q2_2026:Minestats3!B36` | "80" / "64" / "1933"; "109" / "88" / "2240" / "30" | FACT |
| E484 | L-G 2024Q4 sold only 47 koz against 156 koz produced (attr.); AISC 2,136 | `Q4_2024:QTD!C17`, `D17`, `G17` | "156" / "47" / "2136" | FACT |
| E485 | Hemlo was sold to Carcetti Capital for up to $1.09bn | `Q4_2025:QTD!A32` | "we reached an agreement to sell the Hemlo gold mine to Carcetti Capital Corp. for gross proceeds of up to $1.09 billion" | FACT |
| E486 | The Hemlo sale closed 26 Nov 2025; the last data are the 2025Q4 stub, 26 koz (100%), AISC 1,976 | `Q4_2025:QTD!A32`, `B11`, `G11` | "The transaction closed on November 26, 2025. Accordingly, operating and financial results provided are up to the closing date." | FACT |
| E487 | Tongon was sold to Atlantic Group (up to $305M) and closed 1 Dec 2025; 2025Q4 stub 20 koz (100%) / 18 (89.7%), AISC 2,844 | `Q4_2025:QTD!A35`, `B18`, `C18`, `G18` | "to the Atlantic Group for total consideration of up to $305 million. The transaction closed on December 1, 2025." | FACT |
| E488 | Hemlo and Tongon 2026 cells are "—" apart from numeric-zero recovery placeholders | `Q1_2026:QTD!B11`; `Q1_2026:Minestats1!N27`; `Q2_2026:Minestats3!F64` | "— " / "0" (fmt 0%) | FACT |
| E489 | Long Canyon is excluded from 2024 onward (care and maintenance) | `Q1_2024:QTD!A32` | "we have ceased to include production or non-GAAP cost metrics for Long Canyon as it was placed on care and maintenance" | FACT |
| E490 | Porgera was on care and maintenance 25 Apr 2020 – 22 Dec 2023 | `Q1_2024:QTD!A34` | "Porgera was placed on care and maintenance from April 25, 2020 until December 22, 2023" | FACT |
| E491 | Lumwana stated recovery ranged 85–93% in 2023–24 and 91–92% in 2025–26 (100%) | `Q2_2024:Minestats4!D14` 0.85; `Q4_2024:Minestats4!D14` 0.93; `Q1_2025`…`Q2_2026:Minestats4!D14` | "0.85" / "0.93" / "0.91" | FACT (cells); INFERENCE (regime) |
| E492 | Lumwana 2025Q1 sales (34 kt) exceeded production (27 kt) | `Q1_2025:Minestats4!D15`, `D16` | "27" / "34" | FACT |
| E493 | North Mara processed grade fell from 4.29 g/t (2024Q4) to 1.80 g/t (2026Q2); AISC rose from 1,098 to 2,577 $/oz | `Q4_2024:Minestats3!H18`, `Q2_2026:Minestats3!H18`; `Q4_2024:QTD!G20`, `Q2_2026:QTD!G19` | "4.29" / "1.8" / "1098" / "2577" | FACT |
| E494 | North Mara recovery was 85% in both 2026 quarters vs 88–92% in 2023–25 | `Q1_2026:Minestats3!H19`, `Q2_2026:Minestats3!H19` | "0.85" | FACT |
| E495 | North Mara 2026 mill feed is mostly non-pit: Q1 open-pit ore 7 kt, strip 514 | `Q1_2026:Minestats3!H5`, `H8`, `H12` | "7" / "514.14" / "664" | FACT (cells); INFERENCE (feed source) |
| E496 | Total Gold AISC exceeds sold-weighted regional AISC in every period tested (+23.57 to +88.66 $/oz) | e.g. `Q3_2025:QTD!G22` 1,538 on `D22` 837 koz; `tests3_out.txt` T3 detail | "1538" / "837" | FACT (arithmetic); INFERENCE (corporate layer) |
| E497 | AISC workbooks store numeric 0 for L-G 2025Q1–Q3, where Mine Stats shows "—" | `Q1_2025_AISC:Q1 2025!N19`, `N23` vs `Q1_2025:QTD!D16`, `G16` | "0" (fmt `_(* #,##0_);…"-"??`) / "— " | FACT |
| E498 | The 2026 AISC workbooks rename the by-product row and the per-ounce labels | `Q3_2025_AISC:Q3 2025!A7` vs `Q2_2026_AISC:Q2 2026!A7`, `A19:A21` | "By-product credits" → "Costs allocated to by-products"; "COS/oz", "TCC/oz", "AISC/oz" | FACT |
| E499 | No per-site GEO field appears in any of the 10 workbooks; the IR feed lists no AISC xlsx for Q1 or Q2 2024 | `w1/labels_out.txt` line 294 (regex `\bGEO\b`, "gold equivalent" and "equivalent ounce" over every string cell); 2024 feed listing | "== GEO hits: 0" | FACT |

---

## 16. Reproduction notes

**Scratch folder.** `/private/tmp/claude-501/-Users-jemanuel-projects-franken-mining/2ba72fbb-c45d-4fd8-93cf-c08cb26d7a18/scratchpad/w1/`. It is session scratch; copy it elsewhere if it needs to persist.

**Command form.** Every script was run as `UV_CACHE_DIR=/tmp/uv-cache uv run -q --with openpyxl python3 -I <script> <args>`, with the downloads passed as path arguments.

| Script | Input | Output |
|---|---|---|
| `survey.py` | all xlsx | `survey_out.txt` (dimensions, merged ranges, formulas, `#REF!`, tokens) |
| `layout.py`, `labels.py`, `footnotes.py` | all xlsx | `layout_all.txt`, `labels_out.txt` (label inventory and GEO search), `footnotes_all.txt` |
| `pdfpages3.py` (`pdftotext`) | MD&A PDFs | `pdfpages3_out.txt` (reconciliation pages) |
| `extract3.py` | all Mine Stats xlsx | `long_all3.json` (22,732 rows, each with `file:sheet!cell`, period, vintage, ownership and ownership basis) |
| `tests3.py` | `long_all3.json` | `tests3_out.txt` (matrix + every fail with locators), `long_all3_fields.json` |
| `series.py` | `long_all3_fields.json` | `series_out.txt` (Lumwana, North Mara, L-G/Hemlo/Tongon with locators; corporate layer) |
| `aisc_check2.py` | `long_all3_fields.json` + AISC xlsx | T18 (stdout; 420/12) |
| `cells.py` | xlsx + cell specs | spot checks used for every FACT row above |

**Superseded scripts.** `extract2.py`/`tests2.py` and their outputs are superseded, not deleted. Two bugs led to them being replaced:
- `extract2.py` stripped footnote digits before detecting "100%", so it dropped the 100%-basis columns.
- `tests2.py` treated the 2024 copper "millions of pounds" values as pounds.

**Files not modified.** The plan, `planning/mor/MOR_CASE.md`, `planning/corpus/*`, the existing corpus files and the C1 scripts in `scratchpad/scripts/` were not modified.
