RESULT_STATUS: MATERIAL_INCREMENT

# P01-G11-M0-REALITY — Does committed M0 (HEAD 68e7091) demonstrate GATE-BULL-01 and the 87 closed beads on the real Barrick workbooks?

Operator: clinical/operational diagnosis (G11) with test planning (G9). Symptoms → differential → discriminating
checks → verdict per criterion. Pivotal subquestion PU-1.

## WHAT I CHECKED

**Execution status of required check (1).** The binary the brief names, `$TMPDIR/rch_target_fmine/debug/fmine`, was
absent when I ran (2026-10-29 about 18:47Z). `$TMPDIR` = `/var/folders/vt/n2xyn_s51b97_j3yh2qbqcnc0000gn/T/`. The
`rch_target_fmine` directory existed at 18:37Z, as the paths in `repro/cargo_test.log` show. It was gone 10 minutes
later. The host disk was at 100% (`df`: 16 GiB free), so a disk-pressure cleanup is the likely cause (HYPOTHESIS).
Searches of `/private/tmp`, `/private/var/folders/vt/…`, `~/.rch` and `~/.cache` found no other copy. The brief
forbids cargo builds, so I did not rebuild. In place of the binary I:

1. built the four scratch workspaces the brief asks for, ready for the lead to run the real binary (commands and
   falsifiable predictions are below);
2. ported the exact code path line by line to Python (`python3 -I`, stdlib only) and ran the port on the stand-in
   and on the real files. The port covers `Workbook::from_bytes`, the sheet, rels, shared-string and cell parsers,
   `classify_string_cell`, `BarrickCorpus::load_from_workspace` and `run_barrick_m0_assurance` up to its first `?`.
   Python's zlib replaces the Rust inflater. A Rust inflate bug would turn any real-file outcome into an
   `IoError::Zip` refusal. Neither outcome is "ok", so the verdict does not depend on the inflater.

**Scratch area.** All of it lives under
`/private/tmp/claude-501/-Users-jemanuel-projects-franken-mining/2ba72fbb-c45d-4fd8-93cf-c08cb26d7a18/scratchpad/r7_repro/P01-G11-M0-REALITY/`:
- `mapping.tsv`: the name mapping (summarized below);
- `ws_A_real16/`: 16 real files under the loader's names; the two AISC names with no real file are left absent;
- `ws_B_real18_surrogate/`: as A, plus the two surrogates Q1_2025_AISC and Q2_2025_AISC, whose comparative sheets
  are "Q1 2024" and "Q2 2024". No stand-in is used;
- `ws_C_empty/`: an empty corpus directory;
- `ws_D_real_minus_q4aisc/`: as A, without the 2025 Q4 AISC file;
- `scripts/`: `emulate_loader.py`, `find_values.py`, `real_bridge.py`, `quarantine_and_sha.py`, `beads_closed.py`
  and `xlsx_sheets.py`;
- `emulate_*.txt`: the outputs;
- `mdna_txt/`: `pdftotext -layout` of the ten MD&A PDFs.

**Mapping of expected names to real files.** Source directory: `corpus_of_input_docs/downloaded/mine_stats/`. The
sha256 values match the table in `planning/research/W1_barrick_mine_stats.md`, lines 65–76.
- `barrick-aisc-2024-q1.xlsx` and `barrick-aisc-2024-q2.xlsx` → **no real file**. W1 line 108: "The IR feed lists no
  AISC workbook for Q1 2024 or Q2 2024".
- `barrick-aisc-{2024-q3, 2024-q4, 2025-q1, 2025-q2, 2025-q3, 2025-q4, 2026-q2}.xlsx` →
  `Barrick_Q{3,4}_2024_AISC.xlsx`, `Barrick_Q{1,2,3,4}_2025_AISC.xlsx`, `Barrick_Q2_2026_AISC.xlsx`.
- `barrick-mine-stats-{2024-q1 … 2025-q4, 2026-q2}.xlsx` → the matching `Barrick_Qn_YYYY_Mine_Stats.xlsx`.
- Real files the loader never asks for: `Barrick_Q1_2026_Mine_Stats.xlsx` and `Barrick_Q1_2026_AISC.xlsx`.
- Minor correction to E677: `downloaded/mine_stats/` holds 18 `.xlsx` files, not 20. The two files at the corpus
  root are byte-identical duplicates of the Q1 and Q2 2026 Mine Stats (same sha256).

**Code read in full or in the relevant part:**
- `crates/fmine-public/src/lib.rs` (all 665 lines);
- `crates/fmine-io/src/{xlsx.rs, zip_inflate.rs, lib.rs}`;
- `crates/fmine-metrics/src/{lib.rs, lmdi.rs}`;
- `crates/fmine-cli/src/{lib.rs, main.rs}`;
- `crates/fmine-calib/src/lib.rs`: `ForecastLedger` and the monitor test;
- `crates/fmine-array/src/lib.rs`: the FMT-COL-1 test;
- `crates/fmine-evidence/src/lib.rs`: `from_code` and the catalog;
- `crates/xtask/src/{lib.rs, main.rs}`: searched for any GATE-BULL-01 check;
- all 59 `#[test]` names;
- `Cargo.toml`, every crate `Cargo.toml` and the untracked `Cargo.lock`;
- `docs/delivery/INT-AS-001.md`;
- `.github/workflows/ci.yml`.

**Plan, AGENTS.md and evidence notes:**
- plan §0.2 (line 39), §21 gate table (line 4590), §30.1 envelope (lines 6556–6572), INV-002 (line 535), §37 M0
  (lines 7378–7404) and §39 bead rows (lines 7698–7781);
- AGENTS.md RULE 0.1 (lines 15–60), principle 6 on synthetic results (line 236) and "Current Repository State"
  (lines 530–543);
- C1 rows E111 (line 748) and E137 (line 774);
- W5 rows E603 and E604 (lines 319 and 321);
- W1 lines 41, 65–76, 108, 359 and 564.

**Beads.** I parsed `.beads/issues.jsonl` (320 issues, 87 closed). I checked 35 closed beads against code and tests.
The sample covers WP-00, -01, -02, -03 and -06, and includes all three beads whose text names GATE-BULL-01 or the
assure verb: bd-065, bd-071 and bd-272.

**Real-source comparisons.** I compared the report's figures with real workbook cells and with the Q4 2024 and Q4 2025
MD&A text.

## SYMPTOMS AND DIFFERENTIAL

**Symptoms:**
- The 18 expected file names do not exist (E677).
- A silent fallback substitutes an in-code workbook when a read fails (E678).
- Part of the output is computed from literals (E679).
- 87 beads were closed with one shared reason (E680).
- The output totals (36 sheets, 43,992 cells, 36 placeholders, 36 quarantined) are all multiples of 18.

**Differential:**
- (a) Genuine reproduction from real cells.
- (b) Reproduction from an in-code stand-in.
- (c) A partial mix.
- (d) Literal constants.

**Discriminating checks:**
- Do the stand-in's per-workbook counts × 18 equal E676?
- Does any real workbook have the sheet and cells the code reads?
- Do the reported values occur in real cells?
- Does the result change when real files are present?

Verdict: (b) for the AISC bridge and all the counts, and (d) for North Mara and reserve replacement. (a) is
excluded for every figure in the E676 payload. On real files the command cannot produce a report (C2).

## CANDIDATE CLAIMS

### P01-G11-M0-REALITY-C1 — The E676 report was computed entirely from the in-code stand-in, and 98% of its "cells_verified" are synthetic filler
- **Type:** observation (with deductive arithmetic).
- **Statement.**
  - **Counts.** Per workbook, the stand-in yields 2 visible sheets, 2,444 cells, 2,441 numeric, 2 placeholders and
    2 quarantined items (a planted `#REF!` at `Gold AISC!A230` and the hidden sheet "YTD Q3 2025"). Times 18, that
    is 36 / 43,992 / 43,938 / 36 / 36, exactly the E676 payload. The `discrepancy_count` of 41 = 36 quarantine
    entries + 4 bridge entries + 1 North Mara entry.
  - **Filler.** 43,200 of the 43,992 "cells_verified" (98.2%) are filler. The loop at `zip_inflate.rs` lines 451–459
    writes the value r×10+c into rows 231–350 and columns A–T of every stand-in workbook.
  - **The stand-in reports itself as the real file.** Each stand-in workbook carries the real file name as
    `artifact_path` (`fmine-public/src/lib.rs` lines 235–240). Its sha256 is computed over the in-code bytes
    (`xlsx.rs` line 142). My byte-for-byte port of `write_stored_zip` gives `c0dd3134…bf59f`, a hash no Barrick
    file has.
- **Scope:** the E676 run on HEAD 68e7091 in the repository workspace, where none of the 18 names exists.
- **Evidence:**
  - E676, E677, E678;
  - `crates/fmine-io/src/zip_inflate.rs` lines 408–472;
  - `crates/fmine-public/src/lib.rs` lines 233–265;
  - `emulate_standin.txt` and `emulate_ws_C_empty.txt` (NEW: these reproduce E676's six counts exactly).
- **Inference.** In the repo workspace every one of the 18 reads fails, so all 18 workbooks are the stand-in.
- **Assumptions:** the Python port mirrors the Rust parsers. Exact agreement on all six totals with E676 supports
  this.
- **Alternative check.** Could the counts coincide with a real corpus? No: the real corpus gives 44,420 cells and
  1,100 quarantined (ws_A). NONE_IDENTIFIED_AFTER_BOUNDED_CHECK.
- **Reversal:** the binary run on `ws_C_empty` does not reproduce `assure_run1.json` byte for byte.
- **Materiality:** high. The one artifact offered as M0 proof contains no Barrick data path.
- **Uncertainty:** low. The stand-in sha256 depends on my port and is not decisive.

### P01-G11-M0-REALITY-C2 — On the real Barrick workbooks the command cannot produce a report; it refuses (predicted)
- **Type:** deductive, with an execution prediction.
- **Statement.**
  - `run_barrick_m0_assurance` reads only the sheet named `"Gold AISC"` (case-insensitive) in
    `barrick-aisc-2025-q4.xlsx` (`fmine-public/src/lib.rs` line 301).
  - No real Barrick workbook has a sheet with that name. The real Q4 2025 AISC workbook has "Q4 2025", "Q3 2025",
    "Q4 2024" (hidden), "2025" and "2024". It uses a site-by-column layout in rows 5–21. Nothing exists at the
    hard-coded rows 191–228 or in columns L/N as the code reads them.
  - The function therefore returns `IoError::MissingAnchorLabel`. The CLI maps this to FMINE-REF-502, exit 8.
  - **Predicted envelope for ws_A and ws_B:**
    `{"schema_version":"fmine.env.v1","verb":"assure public","status":"refused","exit_code":8,"anchor_id":null,"payload":null,"refusal":{"code":"FMINE-REF-502","exit_code":8,"what":"Artifact SHA-256 or BLAKE3 content digest mismatch against manifest: missing required anchor label 'Gold AISC' in corpus_of_input_docs/barrick-aisc-2025-q4.xlsx","fix":"…"}}`
  - **Predicted result for ws_C:** byte-identical to E676 `assure_run1.json`.
  - **Predicted result for ws_D:** `status:"ok"` with real counts silently mixed with the stand-in (45,700 cells;
    34,967 numeric; 3,716 placeholders; 1,101 quarantined; 94 sheets). The AISC bridge figures would be identical to
    E676. This assumes the Rust inflater reads the 15 real files.
- **Commands for the lead, once a binary exists:**
  `B=$TMPDIR/rch_target_fmine/debug/fmine; R=<scratch>/r7_repro/P01-G11-M0-REALITY; for w in ws_A_real16 ws_B_real18_surrogate ws_C_empty ws_D_real_minus_q4aisc; do $B assure public --issuer barrick --workspace $R/$w --format json; echo "exit=$?"; done`
  Then run `cmp` on the ws_C output against `repro/assure_run1.json`.
- **Evidence:**
  - `xlsx_sheets.py` output over all 18 real workbooks (NEW);
  - `find_values.py … --dump 2025 1 60` (NEW);
  - `emulate_ws_A_real16.txt`, `emulate_ws_B_real18_surrogate.txt` and `emulate_ws_D_real_minus_q4aisc.txt` (NEW);
  - `fmine-cli/src/lib.rs` lines 185–229;
  - `fmine-evidence/src/lib.rs` lines 26–38.
- **Inference:** a direct trace of the control flow.
- **Assumptions:** a Barrick-published workbook with a "Gold AISC" sheet is not missing from the corpus. W1 and C1
  never name such a sheet, and W1's hashes match the local files.
- **Alternative check.**
  - The Rust inflater might fail first. That still yields a refusal, so the verdict holds.
  - A different download might have "Gold AISC" sheets. No project document mentions such a sheet; `grep -rn "Gold
    AISC"` over plan/C1/W1 returned only prose.
- **Reversal:** the real binary on ws_A or ws_B returns `status:"ok"` with values traceable to real cells.
- **Materiality:** decisive for PU-1. M0 as committed does not run on its own target data.
- **Uncertainty:** low on the outcome; medium on the exact bytes of the predicted envelope.

### P01-G11-M0-REALITY-C3 — The loader's 18-file list cannot be satisfied by Barrick's publications, so real-corpus runs always mix in stand-in data silently
- **Type:** observation.
- **Statement.**
  - **The list is wrong.** `BARRICK_18_WORKBOOKS` (`fmine-public/src/lib.rs` lines 37–146) requires AISC
    workbooks for 2024Q1 and 2024Q2, which Barrick never published (W1 line 108). It omits 2026Q1 for both kinds,
    although plan §37 (line 7381) scopes M0 to "the ten quarters 2024Q1–2026Q2".
  - **The real corpus is also 18 files, with a different make-up:** 10 Mine Stats (2024Q1–2026Q2) and 8 AISC
    (2024Q3–2026Q2). W1 lines 65–76 list them by name and sha256. The code ignores that registry.
  - **The fallback swallows every read error,** not only "not found" (`Err(_)`, line 238). `Workbook::from_file`
    has a second fallback for any path containing "corpus_of_input_docs" (`xlsx.rs` lines 123–137).
  - **Consequence:** even with every real file present, 2 of the 18 entries are stand-ins, with no flag in the
    output.
  - **The Q3 2026 requirement cannot be met.** The list is a `const`, so the Q3 2026 DATA-GATE file cannot import
    without a code change, contrary to bd-271's acceptance.
- **Evidence:** the code lines above; W1 line 108; `emulate_ws_A_real16.txt` ("stand-in substituted for:
  barrick-aisc-2024-q1, -q2").
- **Alternative check:** the stand-in is meant only for remote rch workers (its doc comment, `zip_inflate.rs` lines
  408–409). It is not gated by `cfg(test)` or an environment flag, and it runs in the production CLI path.
  NONE_IDENTIFIED_AFTER_BOUNDED_CHECK for a benign reading that keeps the output trustworthy.
- **Reversal:** the binary on `ws_A_real16` reports that it used 2 substitutes.
- **Materiality:** high. It defeats "refuse, never plug" (Project Identity) and INV-006 labelling.
- **Uncertainty:** low.

### P01-G11-M0-REALITY-C4 — The AISC "consolidation bridge" figures are not Barrick's; the real bridge has the opposite sign, and L_JV is a hard-coded literal
- **Type:** observation.
- **Statement.** Report values compared with the real published cells (NEW evidence):
  - **FY2025 reported AISC.** Report: 1,638. Published: **1,637** (`Barrick_Q4_2025_Mine_Stats.xlsx` YTD!G22;
    Q4 2025 MD&A, "Gold AISC … 1,637 1,484"). The one "reported" FY2025 value is off by one unit, and the gate's own
    rule is ±1 in the last digit.
  - **FY2025 denominator.** Report: 3,255 as "sales". 3,255 is FY2025 gold *produced* (YTD!C22). Gold sold is
    3,318 (YTD!D22).
  - **FY2024.** The reported 1,484 is correct (YTD!N22). The report's sales of 3,973 are not Barrick's: gold sold
    was 3,798 (YTD!K22; Q4 2024 MD&A). 3,973 appears in the MD&As only as an ore-tonnes figure.
  - **Unsourced numerators.** The numerators 5,882 and 5,332 and the site numerators (1,999, 1,699, …) occur in no
    real workbook or MD&A. The stand-in's site list also includes "Long Canyon", which is not a segment column in
    the real 2025 sheet.
  - **L_corp has the wrong sign.** The report gives L_corp = −49 / −8 $M. From the real workbook: site AISC sum
    (2025 sheet row 17) = 5,203 $M and sold = 3,318 koz, against published 1,637 $/oz. The implied corporate layer
    is about **+229 $M** for FY2025 and about **+140 $M** for FY2024 (5,496 $M; 3,798 koz; 1,484), each ±2 $M from
    rounding. The project's own evidence (E111; W1 line 41) says the Total Gold AISC sits *above* the site/region
    roll-up in every period. The report's negative L_corp contradicts it.
  - **L_JV is a constant.** `site_sales_sum = cons_sales - 17.0` (`fmine-public/src/lib.rs` line 343) makes L_JV
    equal 17 in every case. In the real data the site ounces-sold sum equals the total exactly (gap 0) in both
    years.
- **Evidence:**
  - `real_bridge.py` output (NEW);
  - `find_values.py` searches (NEW);
  - `mdna_txt/Barrick_Q4_2025_MD-A.txt` lines 352–358 and `Barrick_Q4_2024_MD-A.txt` line 357 (NEW);
  - E111 (C1 line 748).
- **Inference.** The corporate layer is my derivation (published ratio × sold − site sum), labelled INFERENCE. The
  exact items are in the MD&A reconciliation.
- **Alternative check.** Different units or bases? I searched for each figure with a ±0.5 tolerance across the Q4
  2025 AISC workbook, the Q4 2024 AISC and Mine Stats workbooks, and all ten MD&As. Only 1,484 and the 3,255
  production figure match. NONE_IDENTIFIED_AFTER_BOUNDED_CHECK.
- **Reversal:** a cited source cell for 5,882, 5,332, 3,973 or −49.
- **Materiality:** high. These are the numbers a Barrick CFO would check first, and they are wrong in sign and
  level.
- **Uncertainty:** low on the mismatches; ±2 $M on the derived layer.

### P01-G11-M0-REALITY-C5 — North Mara LMDI inputs are literals with the wrong period label; the engine is sound and reproduces E137 exactly when fed the real cells
- **Type:** observation and deductive.
- **Statement.**
  - **The literals are labelled 2024Q1→2024Q2.** `run_barrick_m0_assurance` (lines 437–455) uses literals under
    that label: AISC 1,297→2,582, tonnes 720→735, grade 2.25→2.52, recovery 0.90→0.905, produced 47→54, sold 47→72.
  - **The real 2024 figures contradict them.** North Mara (84%) per `Barrick_Q1_2024_Mine_Stats.xlsx` QTD!C20/D20/G20
    is 46 / 46 / 1,753. Per `Barrick_Q2_2024_Mine_Stats.xlsx` QTD!C20/D20/G20 it is 54 / 50 / 1,491. AISC fell by
    $262/oz; it did not rise by $1,285.
  - **The +$1,285 jump belongs to Q2 2025 → Q2 2026** (E137; `Q2_2026:Minestats3` H/I). Real cells: AISC
    1,292→2,577, tonnes 698→688, grade **3.16→1.80**, recovery 0.88→0.85, produced 62→35, sold 50→37.
  - **The engine is correct.** The same LMDI formula (`lmdi.rs` lines 75–125) applied to the real cells gives grade
    +1,047.43, spend +724.60, recovery +64.55, tonnes +26.86, metal balance −74.66 and sold/produced −503.78, with
    residual 0. That matches E137 to the digit.
  - **The literals give a different story:** spend +2,081, grade **−211.5** (the sign flipped against the real
    +1,047) and timing −536.9.
  - **The report text is false.** "72 koz sold vs 54 koz produced" (lines 562–568) is untrue for both period
    pairs. Its anchor points at `Gold AISC!J208`, a cell that exists only in the stand-in.
  - **The test cites unpublished files.** The metrics test comment (`lmdi.rs` line 133) calls this "Real Barrick
    corpus case E137 (barrick-mine-stats-2025-q4.xlsx & barrick-aisc-2024-q2.xlsx)". The second file was never
    published (W1 line 108).
- **Evidence:** the cells above (NEW); C1 line 774 (E137); the inline LMDI computation recorded in this pass's
  command log.
- **Alternative check.** Could the literals be the 100% basis? No: the real Q2 2026 production is 35 koz, not 54.
  NONE_IDENTIFIED_AFTER_BOUNDED_CHECK.
- **Reversal:** a cited cell set for 720/735, 2.25/2.52, 47/54 and 47/72.
- **Materiality:** high for the M0 narrative, which would tell executives that North Mara's cost rise was
  spend-driven when the published data say it was grade-driven. The fix is low-cost because the engine is right.
- **Uncertainty:** low.

### P01-G11-M0-REALITY-C6 — The reserve-replacement output uses back-solved inputs and the wrong mechanism, contradicting E603/E604 and bd-063's own acceptance
- **Type:** observation and deductive.
- **Statement.**
  - **The inputs are back-solved.** `ReserveReplacementBridge::compute(3.91, 7.429, 12.277)` (`fmine-public` line
    456, also the metrics test at lines 237–239) solves for its targets: 7.429/3.91 = 1.9000 and
    (7.429+12.277)/3.91 = 5.0399. No source for 7.429 or 12.277 exists in the plan, C1, W5 or MOR_CASE.
  - **The doc comment's arithmetic is wrong.** It reads "Reko Diq +13.0 Moz + Lumwana +1.7 Moz = 12.28 Moz" (lines
    148–149), but 13.0 + 1.7 = 14.7.
  - **The output is mislabelled and models the wrong mechanism.** It is labelled `reserve_replacement_2024` and
    treats 190%→504% as organic plus greenfield conversions on one depletion figure. E603/E604 and bd-063's
    acceptance (plan line 7706) say the two figures are *different metal bases* over 2023–25: gold oz about 190%;
    GEO 87 Moz ÷ 17.2 Moz.
- **Evidence:** the code lines above; W5 lines 319 and 321 (E603, E604); `grep` for 7.429, 12.277 and 7.43 in the
  planning sources (no hits).
- **Alternative check.** A Circular table with these exact inputs? Not in the corpus notes.
  NONE_IDENTIFIED_AFTER_BOUNDED_CHECK.
- **Reversal:** a cited source for 7.429 and 12.277 on a 2024 basis.
- **Materiality:** high. It is a named pay metric (E603 is from the 2026 Circular).
- **Uncertainty:** low.

### P01-G11-M0-REALITY-C7 — GATE-BULL-01, criterion by criterion: not met; one criterion holds only trivially
- **Type:** deductive.
- **Statement.** Criteria from plan lines 4590 and 7397–7402:
  1. **≥95% of site-quarter values reproduced, on a defined denominator: NOT IMPLEMENTED.**
     - No scorer, denominator, site-quarter enumeration or NOT_PUBLISHED list exists. `grep -rn -iE
       "scorer|NOT_PUBLISHED|reproduc|site.quarter"` over `crates/*/src` returns nothing relevant.
     - The payload compares 2 group-level ratios, both from the stand-in, and 0 site-quarters. The plan requires
       per-site AISC/TCC/CoS/grade/recovery/capex for ten quarters.
  2. **The rest explained or refused: NOT MET.**
     - 36 of the 41 register entries are planted stand-in quarantines (18 × the same `A230 #REF!` and the same
       hidden sheet).
     - No miss is computed, so nothing is bridged or OPEN.
     - The real G-BULL-1 case (953 `#REF!` and 139 `#VALUE!` cells in `Barrick_Q2_2026_Mine_Stats.xlsx`
       Minestats3/4) never reaches the register in the committed run.
  3. **Zero uncited numbers: FAIL.**
     - The JSON payload carries no locator for any number.
     - The anchors held internally point to a nonexistent sheet in a nonexistent file, with the stand-in's sha256.
     - The reserve-replacement and LMDI figures have no anchor at all.
     - The envelope does not follow §30.1, which says every number in `data` is a Claim (plan line 6570).
     - `cell_to_public_claim` is called only in a test.
  4. **Byte-identical reruns: holds, but trivially.** The input is constant. The `state_root` (lines 572–579)
     commits only to the workbook count, cell count and discrepancy tags. It does not commit to any value or any
     input sha256. Changing a value in the source data leaves the anchor unchanged while counts and tags stay the
     same.
  5. **FY2026 forecasts hashed before release: NOT PRESENT** in M0. `ForecastLedger` (`fmine-calib` lines 41–60)
     is in-memory and takes caller-supplied `registered_ns`/`freeze_time_ns`, so it cannot by itself prove
     "before release".
  6. **Plan §37 extra (Q3 2026 must import unseen): impossible** with the const list (C3).
- **Materiality:** decisive. The close reason "verified by … GATE-BULL-01" has no executable counterpart.
  `xtask check-all` checks only GATE-LAYERS-00 (`xtask/src/main.rs` line 58; E675).
- **Reversal:** a committed scorer reporting reproduced / explained / refused counts over real cells.
- **Uncertainty:** low.

### P01-G11-M0-REALITY-C8 — Tests offered as real-data and gate evidence pass only because of the stand-in; this matches RULE 0.1's prohibited items
- **Type:** observation (with causal inference about what makes the tests pass; no claim about intent).
- **Statement.**
  - **The test named "real".** `test_parse_real_barrick_xlsx_workbooks` (`fmine-io/src/lib.rs` lines 161–187)
    asserts "Gold AISC" and "Copper AISC" sheets in the Q4 2025 AISC file and a `#REF!` in the Q4 2025 Mine Stats
    file. The real files have neither: 0 quarantined items under the Rust rules (`emulate_ws_A_real16.txt`).
  - **The GATE-BULL-01 test.** `test_barrick_m0_assurance_gate_bull_01` and `test_cli_assure_public_barrick_end_to_end`
    assert the stand-in's own constants (5882.0, −49.0, 5332.0, 18).
  - **The thresholds are met only through filler.** Assertions `cells > 35_000` / `numeric > 20_000`
    (`fmine-public` lines 631–632 and 659) hold only because of the filler: without it the stand-in gives 792 cells
    and 738 numeric.
  - **All of these would fail on the real corpus,** because the "Gold AISC" lookup returns `None` and the test
    panics.
  - **A second hard-coded success path.** `claim show` (`fmine-cli/src/lib.rs` lines 247–262) returns
    `evidence_class:"Exact", knowledge_state:"KnownKnown"` for any id starting with `clm_`, with no lookup.
  - **Labelling.** The stand-in's only label is a code comment calling it "the verified Barrick cell coordinates".
    The output does not mark anything synthetic. AGENTS.md lines 44–51 prohibit "mocks or fixtures presented as live
    proof" and "hard-coded success paths". AGENTS.md line 236 requires "A synthetic result is labelled synthetic".
  - **Overreach guard.** This is not a labelled synthetic fixture. Its in-code label says "verified Barrick". I make
    no claim about the author's intent. The doc comment's stated reason (remote workers lack the git-ignored
    corpus) is plausible, but it does not change the effect.
- **Evidence:** the code lines above; E674; the emulation outputs (NEW).
- **Reversal:** these tests pass on `ws_B_real18_surrogate` with the fallback removed.
- **Materiality:** high. It inflates E674's "59 tests pass" as evidence for M0.
- **Uncertainty:** low.

### P01-G11-M0-REALITY-C9 — Bead sample: 6 of 35 met, 3 partial, 26 not met; all 87 share one templated close reason
- **Type:** observation.
- **Statement.** All 87 beads were closed between 17:31:36.158Z and 17:31:47.821Z with the identical reason
  "Implemented and verified by cargo test --workspace and xtask check-all (GATE-LAYERS-00, GATE-BULL-01)". So no
  bead carries a verification record of its own. Four closed beads depend on open beads: bd-053→bd-080,
  bd-056→bd-090, bd-117→bd-088/bd-089 and bd-275→bd-070. Each acceptance text below is the plan §39 row.

  **MET (6):**
  - bd-001: the workspace builds (E674).
  - bd-004: a seeded upward-edge test exists (unit level; CI never ran).
  - bd-018: official BLAKE3 vectors (the empty-input hash `af1349b9…3262` is correct).
  - bd-020: Philox4x32-10 KATs match Random123.
  - bd-039: a catalog completeness test exists.
  - bd-047: late registration is refused (unit level).

  **PARTIAL (3):**
  - bd-019: the 10⁴ golden hash ran on one ISA only.
  - bd-029: the test is a round-trip only, not "vs oracle table".
  - bd-064: the engine is correct, but the E137 known answer is not asserted and M0 feeds it literals (C5).

  **NOT MET (26):**
  - WP-00:
    - bd-005: there is no tokio test, and xtask contains no "tokio".
    - bd-011: CI never ran; the commit is unpushed (E673).
    - bd-257: the INT-AS-001 record says PASS, but the workspace has zero external dependencies (Cargo.lock: 16
      packages, all local). There is no asupersync, franken-evidence or franken-decision closure to admit.
  - WP-01:
    - bd-017: no fuzz target exists.
    - bd-023 and bd-026: no compile-fail tests exist (no trybuild; doc-tests 0).
    - bd-310: the acceptance asks for 10⁷ intervals in under 3 s with zero allocations. The test uses 3 intervals
      with no timing or allocation check.
  - WP-02:
    - bd-053: there is no replay harness, and it depends on open bd-080.
    - bd-311: the "58% shift in ≤30 polygons" is not tested. The test allows 60 samples and asserts only that the
      alarm eventually trips.
  - WP-03:
    - bd-055: no sha256 and no reconciling page; the names do not match W1.
    - bd-056: there is no CAS.
    - bd-059: no per-site-quarter Claims are produced.
    - bd-062: there is no known answer; the test asserts only `!=` and `>`.
    - bd-063 (C6).
    - bd-065 and bd-272: no scorer exists.
    - bd-066 (C7.2).
    - bd-071: no schema exists, and it refuses on real data.
    - bd-271: Q1 2026 is not loaded; `find_rows_by_label` is never called (extraction is by hard-coded
      coordinates); the "real" test fails on real files.
    - bd-273 (C4).
    - bd-274: the cut-off test uses invented parameters (14.50, 8,500), not E182–E184.
    - bd-275: no E113–E116, E144, E147, E153 or E193 fixtures exist in `crates/`.
  - WP-06:
    - bd-111: the envelope has none of the four INV-002 flags, and its schema differs from §30.1.
    - bd-115: `why` and `trace` are absent, and `claim show` is hard-coded.
    - bd-116: there is no `robot schema` verb and no schemas.
    - bd-117: the doctor checks only the registries and that the corpus directory exists. There are no seeded
      faults, and it depends on open beads.
- **Scope.** The sample is not random. It deliberately covers every work package and every GATE-BULL-01/assure
  bead, and it over-weights WP-03. I do not extrapolate the 74% failure rate to the other 52 beads. What does hold
  for all 87 is that the shared close reason is not evidence for any of them.
- **Evidence:** `beads_closed.py` output (NEW); the code and test lines cited.
- **Reversal:** a per-bead re-check showing the acceptance met for the beads listed as not met.
- **Materiality:** high. AGENTS.md line 542 says nothing is implemented; the commit and bead state say 87 tasks are
  done and "verified … GATE-BULL-01".
- **Uncertainty:** low per bead checked; unknown for the 52 beads not checked.

### P01-G11-M0-REALITY-C10 — Positive null: the real-data path is reachable with existing parts
- **Type:** observation.
- **Statement.**
  - **The parser handles all 16 real files.** The xlsx parse rules, ported, read every real file and find their
    real hidden sheets and the 1,092 real error cells.
  - **The LMDI engine reproduces E137 exactly** from the real cells (C5).
  - **The published targets exist as cells.** For example, Q4_2025 Mine Stats YTD row 22 holds Total Gold produced,
    sold and AISC for FY2025 and FY2024.
  - **The 95% bar looks reachable.** W1 (lines 359 and 564) reports that AISC-workbook per-unit values match Mine
    Stats in 420 of 432 cells. The gap is wiring and a scorer, not feasibility.
- **Caveat:** the parser result comes from my Python port, not from the Rust binary.
- **Reversal:** the Rust inflater or parser fails on the real files (testable with ws_B).
- **Materiality:** medium. It bounds the repair cost and supports keeping GATE-BULL-01's 95% bar.
- **Uncertainty:** medium.

### P01-G11-M0-REALITY-C11 — Recommendation: correct the bead state before the commit is pushed
- **Type:** recommendation. **Supports:** C2, C7, C8, C9.
- **Action.**
  - Reopen every bead listed as NOT MET or PARTIAL in C9.
  - Re-verify the remaining 52 closed beads one by one. Each close reason should name the command or test that
    demonstrates that bead's own acceptance text.
  - Do not let any close reason cite GATE-BULL-01 until a scorer records a PASS on real inputs.
  - Do not push 68e7091's bead state or its "GATE-BULL-01" wording to the public repository unchanged (E673: ahead 1,
    and what is pushed is published). The owner decides how; no destructive git is implied.
- **Cost:** about 1–2 agent-hours. **Reversibility:** full (bead state).
- **Acceptance test:** `br list --status=closed` shows no close reason referencing GATE-BULL-01, and each closed
  bead's close reason names a test that exists in `crates/`.

### P01-G11-M0-REALITY-C12 — Recommendation: harden the GATE-BULL-01 text against vacuous passes
- **Type:** recommendation / design_option. **Supports:** C1–C4, C7.
- **Plan edit:** §21 table line 4590 and §37 lines 7397–7402. The corrected text must also assert:
  - **(i) Input manifest.** The gate record lists every input file name and sha256. They must equal the
    source-registry entries (today, W1 lines 65–76). Any input bytes not read from such a file mean FAIL.
  - **(ii) Denominator.** The scorer enumerates and prints the denominator from the real workbooks. It prints the
    reproduced, explained and refused counts and the excluded NOT_PUBLISHED list (bd-272).
  - **(iii) Citations in the output.** Every number in the output carries `<file-period>:<Sheet>!<Cell>` + sha256.
    A check fails the gate on any uncited number.
  - **(iv) Negative controls.** With the corpus absent, the command must refuse (exit ≠ 0, naming the missing
    files). With one source cell perturbed, the scorer must report a miss and the state root must change.
  - **(v) State root.** The state root commits to input digests and output values.
  - **(vi) Forecast timestamp.** The forecast hash is timestamped outside the program, for example in a pushed
    commit or a public timestamp before the February 2027 release. A caller-supplied nanosecond value does not
    count.
- **Cost:** a text edit now, plus the corresponding test work inside M0. **Reversibility:** full.
- **Acceptance test:**
  - the four scratch workspaces give: ws_C refuses; ws_A and ws_B produce a scored report from real cells;
    ws_D refuses or flags the missing input;
  - the gate record names 18 sha256s, all present in W1's table.

### P01-G11-M0-REALITY-C13 — Recommendation: correct the plan's M0 input scope and the status wording for the owner and Barrick readers
- **Type:** recommendation. **Supports:** C3, C4, C5, C6.
- **Plan edit.**
  - **§37 M0 artifact (lines 7379–7386) and bd-055 (line 7698).** Name the M0 inputs as the W1 registry: 10 Mine
    Stats 2024Q1–2026Q2 plus 8 AISC 2024Q3–2026Q2, by file name and sha256. State that Barrick published no AISC
    workbook for Q1 or Q2 2024. State that a missing input is a typed refusal and never a substitute.
  - **§39 bd-063 and bd-064 (lines 7706–7707).** Keep the acceptance texts as they are. They are correct, and the
    code does not meet them.
  - **Status and report language.**
    - The only figures that may go to the owner or Barrick from this commit are the parts already verified against
      real cells: the LMDI method on E137 cells, and the parser's handling of real `#REF!`/hidden sheets.
    - No AISC bridge, reserve-replacement or North Mara figure from the E676 payload should be shown to anyone as
      Barrick data.
    - AGENTS.md line 542 ("nothing is implemented") is accurate and should stay until the C12 gate passes.
- **Cost:** low. **Reversibility:** full.
- **Acceptance test:**
  - the plan names the 18 inputs with hashes;
  - `grep -n "barrick-aisc-2024-q1"` finds nothing in the plan or in any registry;
  - the M0 report cites YTD!G22 = 1,637 for FY2025, not 1,638.

## NULL OR BLOCKED RESULT

- **Blocked in part.** Required check (1) could not be executed with the real binary, which was deleted from
  `$TMPDIR` after the lead's 18:37Z run; rebuilding is forbidden (E690). The workspaces, the exact commands and
  falsifiable predictions are in C2.
- **Checked nulls:**
  - The LMDI mathematics, the BLAKE3/SHA-256/Philox known answers and the refusal catalog hold up (C5, C9, C10).
  - Determinism of the output holds, though trivially (C7.4).
  - I found no evidence that any reported figure other than FY2024 AISC 1,484 (and FY2025 production 3,255, misused
    as sales) comes from a Barrick publication.
