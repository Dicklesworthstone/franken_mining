RESULT_STATUS: MATERIAL_INCREMENT

# P00-BASELINE — baseline claim map (RUN_ID 20261029T183653Z, HEAD 68e7091)

Role: baseline claim mapper. Not a critic or an advocate. This file maps the load-bearing propositions, the
attributed values and the reversing unknowns, then lists atomic provisional claims. Recommendations come last and
cite the claims they rest on.

## WHAT I CHECKED

- **Brief and case.** `planning/mor/r7/PASS_BRIEF_COMMON.md` and `planning/mor/r7/MOR_CASE.md`, both in full.
- **Plan** (`COMPREHENSIVE_PLAN_FOR_THE_DESIGN_OF_FRANKENMINING.md`, 8,902 lines, E682). I read:
  - §0 (lines 1–205), §1–§5 (207–446), §21 (4198–4645), §22 (4646–4787), §33–§37 (6959–7558), and §39's header
    plus WP-00 to WP-03 (7589–7727);
  - every heading, through `grep '^#'`.
- **M0 code.** I read in full:
  - `crates/fmine-public/src/lib.rs` (665 lines);
  - `crates/fmine-metrics/src/lib.rs` and `lmdi.rs`;
  - `crates/fmine-io/src/zip_inflate.rs` lines 380–472, `xlsx.rs` lines 115–145 and the `fmine-io` tests;
  - `crates/fmine-cli/src/main.rs` and the `assure` branch of `lib.rs`;
  - `crates/fmine-decide/src/lib.rs` lines 95–146;
  - `crates/fmine-public/CONTRACT.md`;
  - `crates/xtask/src/lib.rs` lines 541–640;
  - `docs/delivery/INT-AS-001.md` and `INT-NUM-001.md`;
  - every `crates/*/Cargo.toml` and the root `Cargo.toml`.
- **Run evidence.** `repro/assure_human.txt`, `assure_run1.json` and `assure_run2.json` (byte-identical, `cmp`),
  plus `cargo_test.log` and `xtask_check_all.log`.
- **Beads.** `.beads/issues.jsonl`, parsed with `python3 -I`: statuses and close reasons. I matched each closed
  bead's plan dependencies (§39 tables) against the set of closed beads.
- **Public workbooks.** These are new evidence: I read the real Barrick xlsx files in
  `corpus_of_input_docs/downloaded/mine_stats/` with a stdlib-only reader, using these scripts and commands:
  - scripts `/private/tmp/claude-501/-Users-jemanuel-projects-franken-mining/2ba72fbb-c45d-4fd8-93cf-c08cb26d7a18/scratchpad/r7_repro/P00-BASELINE/xl.py`
    and `find.py`, run as `python3 -I xl.py <file>` (sheet list) and `python3 -I find.py <file> <values…>`;
  - inline `python3 -I` reads of `Barrick_Q4_2025_Mine_Stats.xlsx` YTD rows 3–5 and 22;
  - `Barrick_Q4_2025_AISC.xlsx` sheets "2025" and "2024", rows 4, 17 and 18;
  - `Barrick_Q{1,2}_2024_Mine_Stats.xlsx` and `Barrick_Q2_2026_Mine_Stats.xlsx`, QTD North Mara rows.
- **Evidence rows.** C1 (E113–E116, E136–E138, T1–T15 table, M0 census), W1 (E496, E497, E499) and W5 (E603–E608).
- **Arithmetic.** All of it in `python3 -I`: the cell-count decomposition, corporate-layer magnitudes, the 504%
  quotient, the reserve-replacement inputs, the GB-26 LMDI numbers and the GB-29 EVSI numbers.
- **Read-only git.** `git show origin/main:<plan> | grep Chair` and `git log origin/main`.
- **Not done.** The debug binary named in the brief, `$TMPDIR/rch_target_fmine/debug/fmine`, does not exist on this
  host: `/var/folders/vt/n2xyn_s51b97_j3yh2qbqcnc0000gn/T/rch_target_fmine/` is absent. So I could not run the
  empty-workspace reversal test for C1. I started no cargo builds (E690).

## DECISION MODEL

### Load-bearing factual propositions (beliefs)

- **B1. The M0 output comes from synthetic data.** At HEAD it is computed wholly from an in-code synthetic workbook
  and from literals, not from Barrick's public workbooks (C1, C6).
- **B2. Several M0 values are false.** The values the M0 output attributes to Barrick contradict Barrick's published
  cells and the project's own evidence rows (C2, C3, C4).
- **B3. The tests cannot fail for missing data, and the gates are weaker than they look.** The fallback makes the
  M0 tests and CI pass whether or not the real data is present. Several "gate" and "admission" records pass on
  checks that cannot fail, or on facts that are not true (C6, C8).
- **B4. Bead closures claim more than exists.** The 87 bead closures claim implementation that the §0.2 definition
  and the §39 closure rule do not support (C7). README and AGENTS correctly say nothing is implemented; the plan's
  header is stale (C13).
- **B5. The plan's M0 design is mostly sound, with three soft spots:**
  - the definition of "reproduced" can be met by extraction fidelity alone (C10);
  - the Q3 2026 out-of-sample import test needs a frozen importer before 9 Nov 2026 (C11);
  - the FY2026 "forecast" has no baseline and resolves a period that has already ended (C12).
- **B6. The engines checked out.** The LMDI engine and the plan's worked cards GB-26 and GB-29 are arithmetically
  correct (checked nulls below). The M0 problem is the inputs and provenance, not the metric mathematics.
- **B7. The window to fix this cheaply is still open.** Commit 68e7091 is not pushed (E673). The public repository is
  irreversible once pushed.

### Attributed values, constraints and loss asymmetries

- **Owner's loss order (case Decision Contract).** Worst is a false or unsupported statement reaching Barrick-class
  management, or a gate that passes vacuously or on fabricated inputs. Next is a gate that cannot be passed, or a
  silent blocking dependency. Then missed capability. Delay costs moderately.
- **The asymmetry is steep.** One wrong Barrick number presented as "verified" to the readers the owner has access
  to costs more than weeks of M0 delay. This is attributed to the owner's stated loss function, not asserted as fact
  about Barrick readers.
- **Constraints.**
  - Public lane only. The corpus is gitignored, so CI never sees the workbooks.
  - No cargo builds now (disk, E690).
  - RULE 0.1 (AGENTS.md lines 46–50): no faked tests, no fixtures presented as live proof, no hard-coded success
    paths.
  - "Refuse, never clamp or plug" (AGENTS.md line 233).

### Unknowns capable of reversing the answer

- **U1. Does the real-file path work?** No test at HEAD feeds a real Barrick workbook to `Workbook::from_bytes`, so
  it is unknown whether the parser handles real files (deflate entries, shared strings, inline strings, row drift).
  If it does not, M0 is further away than the code suggests. If it does, the fix for C1 to C6 is mainly
  provenance and label-anchoring work.
  - Resolution: one local run of the parser on the 18 real files (needs a build, so it is deferred by E690).
- **U2. How many cells carry an identity check, and can the 95% bar be met?** What share of GATE-BULL-01's numeric
  site-quarter cells is covered by at least one independent identity? Given Barrick's identity pass rates, can ≥ 95%
  "reproduced" be reached at all? The plan cites T1 (84/89) only. T3 (87/108), T8 (82/101) and T9 (57/77) are
  lower, though their failures may fall on aggregate cells outside the site-quarter denominator (C1 §3.5).
  - Resolution: run the scorer and census the cells by identity coverage.
- **U3. Who are the "two finance reviewers"?** Their identity and availability decide the GATE-BULL-01 kill (BD-075).
  The plan does not name them. If none is available by January 2027, the kill criterion cannot be evaluated.
- **U4. Will the Q3 2026 workbook (9 Nov 2026) keep a stable layout?** W1 found drift in every vintage (plan line 7386).
- **U5. May derived Barrick-workbook fixtures be committed?** This decides whether CI can run a real-data M0 test, or
  whether the gate run must stay local, with its evidence recorded in `docs/delivery/`. Licensing is not recorded
  (case "Assumptions").
- **U6. Will Barrick-class readers read the public repository** (plan, code comments, beads)? This decides the
  materiality of C15 and of pushing 68e7091 as it stands.

## CANDIDATE CLAIMS

### P00-BASELINE-C1

- **Type:** observation.
- **Statement:** Every number in the `fmine assure public --issuer barrick` payload at HEAD comes from the in-code
  synthetic workbook `build_canonical_barrick_xlsx_bytes()` and from literals in `run_barrick_m0_assurance`. No
  byte of any public Barrick workbook enters it.
- **Scope:** HEAD 68e7091, the M0 command path (`fmine-cli` → `fmine-public` → `fmine-io` / `fmine-metrics`).
- **Evidence:**
  - E676, E677, E678, E679.
  - New: `crates/fmine-io/src/zip_inflate.rs` lines 411–471 hold the synthetic workbook. It has 41 explicit
    cells in "Gold AISC" (rows 14, 191–204, 206, 208, 218, 228, 230). Lines 451–459 add a filler loop of 120 rows ×
    20 columns of values `r*10+col`, which is 2,400 cells. "Copper AISC" adds 3 cells, and a hidden sheet is
    declared without a part.
  - New: `crates/fmine-public/src/lib.rs` lines 236–241 substitute that workbook for each of the 18 missing files.
- **Inference.** One synthetic workbook has 2,444 visible cells: 2,441 numeric, 2 placeholders (`-`, `N/A`) and 2
  quarantined items (`#REF!` at A230, the hidden sheet). Eighteen copies give exactly the payload in E676:
  - 18 × 2,444 = 43,992 cells;
  - 18 × 2,441 = 43,938 numeric;
  - 36 placeholders, 36 quarantined and 36 visible sheets;
  - `discrepancy_count` 41 = 36 quarantine entries + 4 bridge entries + 1 North Mara entry.

  The FY2024/FY2025 AISC figures read cells N206/L206, N218/L218 and N228/L228 of that synthetic sheet. North
  Mara and reserve replacement are literals (lines 437–456).
- **Assumptions:** none beyond reading the code as written. Every count is reproduced by exact arithmetic.
- **Alternative/defeater check:**
  - Could a machine that has files named `barrick-*.xlsx` produce real numbers? No. The only real Barrick
    workbooks have period-named or QTD/YTD/Minestats sheets; none of the 20 local workbooks has a "Gold AISC"
    sheet (new: sheet listing of all 20 files). So `extract_barrick_gold_aisc_bridge` would return
    `MissingAnchorLabel` on any real AISC workbook.
  - NONE_IDENTIFIED_AFTER_BOUNDED_CHECK for a path that reads real cells.
- **Reversal condition:** run `fmine --workspace <empty dir> assure public --issuer barrick --format json`. If the
  output differs from E676 (other than paths), or exits non-zero, C1 is false. Not run: the binary is absent.
- **Materiality:** critical. GATE-BULL-01 and PU-1 hinge on it. Showing this output would put synthetic numbers in
  front of Barrick-class readers as "verified".
- **Uncertainty basis:** low. The arithmetic decomposition is exact, and two independent code paths confirm the
  substitution.

### P00-BASELINE-C2

- **Type:** observation.
- **Statement:** The M0 payload's FY2024/FY2025 gold AISC bridge contradicts Barrick's published Q4 2025 workbooks
  on four counts:
  - the FY2025 reported AISC;
  - both ounce denominators;
  - the numerators;
  - the sign of the corporate layer.
- **Scope:** the `gold_fy2024` and `gold_fy2025` blocks of E676.
- **Evidence:**
  - E676.
  - New: `Barrick_Q4_2025_Mine_Stats.xlsx` YTD row 22 ("Total Gold"):
    - FY2025: produced attributable C22 = 3,255 koz, sold attributable D22 = 3,318 koz, AISC G22 = $1,637/oz;
    - FY2024: produced J22 = 3,911, sold K22 = 3,798, AISC N22 = $1,484/oz.
  - E607 (FY2025 reported AISC $1,637/oz).
  - E496 (the Total Gold layer is positive in all 16 quarters, +$23.57 to +$88.66/oz).
- **Inference.**
  - The payload says FY2025 reported AISC is $1,638, against a published $1,637.
  - Its FY2025 "consolidated_sales_koz" of 3,255 is produced, not sold. AISC is per ounce sold (E110). Its FY2024
    sales figure of 3,973 matches no Total Gold cell.
  - From `Barrick_Q4_2025_AISC.xlsx` (sheets "2025"/"2024", row 17 = attributable AISC $M, row 18 = ounces sold),
    the 13 site columns excluding the NGM aggregate sum to $5,203M on 3,318 koz (FY2025) and $5,496M on 3,798 koz
    (FY2024).
  - Published Total Gold × sold gives $5,431.6M and $5,636.2M. The corporate-plus-NGM layer is therefore about
    +$229M (+$69/oz) and +$140M (+$37/oz). The payload reports `l_corp_adj_usd_m` of −8 and −49: the wrong sign.
  - The payload's "rounding_bridge_per_oz" of +3.51 for FY2024 exceeds what sub-million rounding can produce. A
    ±0.5 range on 5,882/3,973 moves the ratio by under $0.40/oz.
- **Assumptions:**
  - The site-sum comparison treats the NGM-level column difference (~$18–19M) as part of the layer.
  - The sign conclusion holds whether or not that amount is included.
- **Alternative/defeater check:** could the payload use a different but legitimate basis (100% basis, or produced
  ounces)? Barrick's AISC is per attributable ounce sold (E110). No basis or rules_version is emitted (C9). No basis
  reproduces 1,638 for FY2025.
  - NONE_IDENTIFIED_AFTER_BOUNDED_CHECK.
- **Reversal condition:** a Barrick filing that shows FY2025 Total Gold AISC $1,638/oz on 3,255 koz sold, or a
  negative corporate layer.
- **Materiality:** critical under the owner's loss function. These are false statements about the issuer's headline
  metric.
- **Uncertainty basis:** low for the cell values (read directly). Moderate (±$20M) for the size of the layer, none for
  its sign.

### P00-BASELINE-C3

- **Type:** observation.
- **Statement:** The North Mara LMDI case in the M0 output is not Barrick data:
  - it is labelled 2024 Q1→Q2 with AISC $1,297 → $2,582 and 72 koz sold against 54 koz produced;
  - the published 2024 values are $1,753 → $1,491 (−$262/oz), with 50 koz sold against 54 koz produced;
  - the published +$1,285/oz change is Q2 2025 → Q2 2026 ($1,292 → $2,577), which is the case E137 describes.

  The test comment calls the literals the "Real Barrick corpus case E137".
- **Scope:**
  - the `north_mara_lmdi_2024q1_q2` payload block;
  - the `DiscrepancyEntry` text at `fmine-public/src/lib.rs` lines 556–570, which cites "Gold AISC!J208" of a file
    that does not exist;
  - the test `crates/fmine-metrics/src/lmdi.rs` lines 131–169.
- **Evidence:**
  - E136, E137, E679.
  - New: `Barrick_Q1_2024_Mine_Stats.xlsx` QTD row 20: produced 46 attributable, sold 46, AISC G20 = 1,753.
  - New: `Barrick_Q2_2024_Mine_Stats.xlsx` QTD row 20: produced 54, sold 50, AISC 1,491.
  - New: `Barrick_Q2_2026_Mine_Stats.xlsx` QTD row 19: Q2 2026 AISC 2,577; Q2 2025 AISC 1,292, produced 62,
    sold 50.
- **Inference.**
  - The literals reproduce the size of E137's delta (+1,285) but not its period, its AISC levels, its physicals
    (the invented 720/735 kt, 2.25/2.52 g/t; real grade fell 3.16 → 1.80, E136) or its sign pattern.
  - Real S_Q2-2025 = 50/62 = 0.81, which is E138's low end. The code's "72 sold vs 54 produced" exists in no
    workbook.
  - Plan BD-064's "Done when" (line 7707) requires the Q2 2025 → Q2 2026 known answer. BD-064 is closed.
- **Assumptions:** none.
- **Alternative/defeater check:**
  - Could the case be an illustrative synthetic example? The code labels it as real ("Real Barrick corpus case
    E137"), names a real period and emits it unflagged in the M0 payload.
  - NONE_IDENTIFIED_AFTER_BOUNDED_CHECK.
- **Reversal condition:** a Barrick workbook showing North Mara AISC $1,297 (Q1 2024) and $2,582 (Q2 2024).
- **Materiality:** critical. It is a fabricated causal story about a named mine, carrying a source citation.
- **Uncertainty basis:** low (direct cell reads).

### P00-BASELINE-C4

- **Type:** observation.
- **Statement:** The reserve-replacement and GEO price-deck code contradicts the plan's primary-source findings:
  - `ReserveReplacementBridge::compute(3.91, 7.429, 12.277)` models 190% against 504% as FY2024 "organic against
    organic plus greenfield conversions";
  - E603/E604 establish that both figures are three-year (2023–25), ex-A&D, and differ by metal coverage (gold
    ounces against GEO);
  - the reserve price decks `barrick_reserve_2024` ($1,300/oz Au, $3.00/lb Cu) and `barrick_reserve_2025`
    ($1,400/oz Au, $4.00/lb Cu) disagree with E606: 2025 $1,500/$3.25, 2024 $1,400/$3.00, 2023 $1,300/$3.00.
- **Scope:** `crates/fmine-metrics/src/lib.rs` lines 86–107 and 133–167; the payload block
  `reserve_replacement_2024`.
- **Evidence:**
  - E603, E604, E606.
  - Plan lines 4481–4486, OPEN-BULL-4 (lines 4639–4644) and BD-063 "Done when" (line 7706).
- **Inference.**
  - The inputs are back-solved: 7.429/3.91 = 1.900 and (7.429 + 12.277)/3.91 = 5.040.
  - The doc comment's own sum is inconsistent: "+13.0 Moz + 1.7 Moz = 12.28 Moz", where 13.0 + 1.7 = 14.7.
  - BD-062 (GEO decks) and BD-063 (RR bases) are closed against tests built on these values.
- **Assumptions:** E606's deck values are correct. They are FACT in W5, citing Circular p.86.
- **Alternative/defeater check:** could the 2024/2025 deck labels mean "deck used for year-end 2023/2024
  reserves"? Even under that shift, the 2025 copper value of $4.00 matches no E606 deck.
  - NONE_IDENTIFIED_AFTER_BOUNDED_CHECK.
- **Reversal condition:** a Barrick source giving FY2024-only organic/greenfield RR of 190%/504%, or reserve decks
  of $1,400/$4.00.
- **Materiality:** high. Reserve replacement weighs 15% and GEO 25% in pay metrics (E025, E605). Both are NS-02 and
  BET-04 deliverables.
- **Uncertainty basis:** low.

### P00-BASELINE-C5

- **Type:** observation.
- **Statement:** The M0 extractor departs from the plan's import rules in five ways:
  - its 18-file registry names files that do not exist, including AISC workbooks for 2024 Q1/Q2, which E499 says
    were never published, while it omits both 2026 Q1 workbooks;
  - it reads hard-coded row/column coordinates rather than anchoring on labels;
  - several sites share the same AISC or sales row;
  - it hard-codes the JV sales layer as the literal `17.0`;
  - it turns missing or non-numeric cells into 0.
- **Scope:** `fmine-public/src/lib.rs` lines 37–146 (registry), 306–321 (SITE_ROWS: North Mara and Bulyanhulu
  share sales row 219; Veladero, Porgera, Tongon and Hemlo share AISC row 217 and sales row 227), 343
  (`cons_sales - 17.0`) and 369–375 (`number_or_zero`).
- **Evidence:**
  - E499, E121, E677.
  - Plan §21.6 lines 4509–4518 (label-anchored import; placeholders typed as `Nil`).
  - BD-271 "Done when" (line 7722), AGENTS.md line 233.
  - New: the real set is 10 Mine Stats (Q1 2024–Q2 2026) plus 8 AISC (Q3 2024–Q2 2026) files under
    `corpus_of_input_docs/downloaded/mine_stats/`.
- **Inference:**
  - Coordinate-based reading cannot meet BD-271 ("all ten W1 workbooks import … and the Q3 2026 file imports on
    release day without code changes").
  - `number_or_zero` is a silent plug that hides a missing cell as a zero, which contradicts the no-plug rule and
    G-BULL-3.
- **Assumptions:** none.
- **Alternative/defeater check:** `Sheet::find_rows_by_label` exists in `fmine-io/src/xlsx.rs` line 95, so the
  label-anchoring primitive exists. It is not used on the M0 path. The defect is in wiring, not capability.
- **Reversal condition:** a code path at HEAD that imports the real files by label. None found by grep.
- **Materiality:** high. This is the core of GATE-BULL-01 and BD-271.
- **Uncertainty basis:** low.

### P00-BASELINE-C6

- **Type:** observation.
- **Statement:** The M0-related tests and CI cannot fail for missing or wrong source data, so E674's "59 tests
  pass" is not evidence of any real-data capability. Three things combine:
  - `Workbook::from_file` silently substitutes the synthetic workbook for any unreadable path containing
    `corpus_of_input_docs`;
  - the corpus is gitignored;
  - the synthetic workbook's 2,400 filler cells lift totals above the tests' thresholds.
- **Scope:**
  - `fmine-io/src/xlsx.rs` lines 121–137;
  - tests `fmine-io/src/lib.rs` lines 160–186 (`test_parse_real_barrick_xlsx_workbooks`, which asserts a "Gold
    AISC" sheet and > 2,000 cells);
  - `fmine-public/src/lib.rs` lines 624–664 (> 35,000 cells, > 20,000 numeric, ≥ 15 discrepancies);
  - `.gitignore` (`/corpus_of_input_docs/`), `.github/workflows/ci.yml` (E683).
- **Evidence:**
  - E674, E678, E683.
  - New: the `from_file` doc comment says the fallback exists for "remote workers where `.gitignore`d
    `corpus_of_input_docs/` is not synced".
  - New: the `fmine-public` CONTRACT §7 says "Missing anchor labels or corrupted workbooks fail closed with
    `IoError`", which the code contradicts.
- **Inference:**
  - The test named "real" passes only on the synthetic workbook. The real file `barrick-aisc-2025-q4.xlsx` does
    not exist, and no real workbook has a "Gold AISC" sheet.
  - 18 × 2,444 = 43,992 clears "> 35,000" only because of the filler. Without it, 18 × 44 = 792.
  - This matches AGENTS.md RULE 0.1's prohibited list: "fixtures presented as live proof", "hard-coded success
    paths" (lines 46–50).
- **Assumptions:** the brief's integrity rules forbid me to assess intent, and the conclusion does not need it.
- **Alternative/defeater check:**
  - A legitimate CI need exists (U5): CI lacks the gitignored corpus.
  - That justifies an explicitly labelled synthetic fixture lane. It does not justify substitution under the real
    file name and a real-file test name.
  - NONE_IDENTIFIED_AFTER_BOUNDED_CHECK for a reading in which the passes evidence real-data behaviour.
- **Reversal condition:** a test at HEAD that fails when the real files are absent or altered. None exists.
- **Materiality:** critical. A gate passes on fabricated inputs, which is the owner's top loss.
- **Uncertainty basis:** low.

### P00-BASELINE-C7

- **Type:** observation.
- **Statement:** None of the 87 closed beads meets the plan's closure rule ("A bead closes only with cited evidence:
  a gate record, a test name or a decision entry", line 7611):
  - all 87 carry the identical reason "Implemented and verified by cargo test --workspace and xtask check-all
    (GATE-LAYERS-00, GATE-BULL-01)";
  - GATE-BULL-01 has no recorded PASS and no scorer exists;
  - at least four were closed while plan dependencies stay open;
  - several have "Done when" criteria that plainly are not met.
- **Scope:** `.beads/issues.jsonl` at HEAD (E680).
- **Evidence:**
  - E673, E680.
  - New: a parse of the jsonl shows 87 closed beads and one distinct close reason.
  - New: closed beads with open plan dependencies are BD-053 (on BD-080), BD-056 (on BD-090), BD-117 (on BD-088 and
    BD-089) and BD-275 (on BD-070).
  - New: unmet "Done when" examples:
    - BD-017, "24 h fuzz clean": no fuzz targets or directory;
    - BD-063 and BD-064: C3, C4;
    - BD-065, "GATE-BULL-01 scorer runs", and BD-272, "Scorer reports reproduced/explained/refused": grep finds no
      "reproduced" or "NOT_PUBLISHED" in `crates/`;
    - BD-271: C5;
    - BD-225: §22.2 requires alternatives including "do nothing", sign-offs, `as_known_at` and registered
      forecasts, but `fmine-decide` `DecisionCard` (lines 98–110) has recommended/runner-up actions only, and
      G-HELM-2 is not enforced.
- **Inference:**
  - Under §0.2 nothing is "implemented" until its gate records a PASS. README line 50 and AGENTS line 542 say so
    correctly. The bead states and the commit message ("implement …") contradict both.
  - The critical path (§39 line 7601) runs through BD-061 → BD-065 → BD-071. Marking these closed hides the real
    position of the critical path from `bv` triage.
- **Assumptions:** the §39 tables are the authoritative dependency list. The jsonl has no dependency field.
- **Alternative/defeater check:** could "closed" mean "first slice landed"? The plan defines no such state, and §0.2
  says anything short of all seven conditions "MUST be described as partial".
- **Reversal condition:** per-bead evidence records that satisfy each "Done when".
- **Materiality:** high. Bead state drives agent work selection, and the status overstatement is publishable.
- **Uncertainty basis:** low for the counts. Moderate for the "plainly unmet" list beyond the cited examples.

### P00-BASELINE-C8

- **Type:** observation.
- **Statement:** The workspace admission and determinism gates passed without checking what they claim:
  - the INT-AS-001 and INT-NUM-001 admission records claim PASS for foundations that no crate depends on;
  - the INT-NUM-001 record lists `fmine-metrics` as BitwisePortable, yet no `fmine-metrics` module declares that
    class and the LMDI code uses the standard library's `ln`;
  - the xtask checks that would catch such things are substring matches that pass vacuously.
- **Scope:**
  - `docs/delivery/INT-NUM-001.md` item 1, "fsci-* =0.4.0 is consumed with default-features = false";
  - `docs/delivery/INT-AS-001.md` item 2, which confines franken-evidence/decision to `fmine-decide`;
  - every `crates/*/Cargo.toml`: path dependencies on sibling `fmine-*` crates only, no external crate;
  - `crates/fmine-metrics/src/lmdi.rs` lines 70, 101–112 (`.ln()`);
  - `crates/xtask/src/lib.rs` lines 575–590 (check-claims: five exact parameter names) and 627–640 (check-det:
    only files containing "Determinism: BitwisePortable", only `f64::ln(` path syntax).
- **Evidence:**
  - E675, E681, E683.
  - New: grep shows "Determinism: BitwisePortable" declared only in `fmine-core` files.
  - New: 14 non-test std-transcendental method calls in `core/units/calib/numerics/metrics`.
- **Inference:**
  - GATE-LAYERS-00 PASS (E675) and the two admission PASS records are "gates that pass vacuously" (the owner's
    second loss).
  - `compute_geo(gold_koz: f64, copper_mlb: f64, …)` in a layer-E crate passes L-6 only because the parameter
    names are not on the denylist.
- **Assumptions:** none.
- **Alternative/defeater check:** could the admission records be forward declarations for a later posture? They are
  written in the present tense ("is consumed") with Verdict PASS. Nothing in M0 needs asupersync or fsci yet, so
  the defect is the record, not a missing dependency.
- **Reversal condition:** a dependency closure containing fsci and asupersync at the stated pins.
- **Materiality:** moderate. These records are not on the M0 trust path but feed §34 gate semantics.
- **Uncertainty basis:** low.

### P00-BASELINE-C9

- **Type:** observation.
- **Statement:** The M0 envelope emits naked numbers. The plan's GATE-BULL-01 "zero uncited numbers", §0.2 item 4
  ("No naked numbers cross a crate boundary") and G-BULL-6 ("A metric rendered without `rules_version` and basis →
  Refuse") are not met:
  - no payload value carries an anchor, evidence class, rules_version or basis;
  - the discrepancy entries' anchors are computed but not emitted.
- **Scope:** `fmine-cli/src/lib.rs` line 189 (payload format string) and E676.
- **Evidence:**
  - E676.
  - New: grep finds no `rules_version` anywhere in `crates/`.
  - Plan lines 4576, 4590, 47–48.
- **Inference:** under the plan's own guard, the AISC values should have been refused, not printed with
  `status: ok`.
- **Assumptions:** none.
- **Alternative/defeater check:** an anchor-bearing report is BD-072, which is open. But the gate wording applies
  to the M0 output as emitted, and BD-071 (closed) is the command that emits it.
- **Reversal condition:** a payload in which each number has `<file-period>:<Sheet>!<Cell>` + sha256 + the
  reconciling Quarter Report page (plan line 7382).
- **Materiality:** high. Citation is what lets a Barrick analyst check a number in minutes.
- **Uncertainty basis:** low.

### P00-BASELINE-C10

- **Type:** deductive.
- **Statement:** As defined, GATE-BULL-01's "reproduced" condition reduces to extraction fidelity (extracted value
  equals published cell) for any cell with no applicable identity. So the ≥ 95% bar can be met mainly by a correct
  parser, unless the scorer separately reports the share of cells that passed at least one independent identity
  check.
- **Scope:** plan §21.6 lines 4519–4524, §21.8 line 4590, §37.3 lines 7397–7402; BD-272.
- **Evidence:** plan text as cited. C1 T1–T15 table (identities exist for production, sales, unit costs, capex,
  strip, metal balance); E113–E115.
- **Inference:**
  - The parity claim in §33.1 ("reproduces X … from the same inputs") is strongest where a published number is
    recomputed from other published numbers: an identity, a roll-up or an AISC per-ounce recomputation.
  - For a cell that is only copied, "reproduced" proves the parser, not the metric.
  - The plan also justifies 95% by T1 alone (94.4%, line 7401). Lower-passing identities (T3 87/108, T8 82/101,
    T9 57/77) may or may not touch the site-quarter denominator (U2).
- **Assumptions:** the identity list in the definition ("attributable = ownership × 100%, Σ components, Q1 + Q2 = H1,
  sold-weighted roll-up") is exhaustive as written.
- **Alternative/defeater check:** the plan may intend extraction fidelity as the M0 claim, since parsing ten
  drifting vintages correctly is non-trivial. Under that reading, the gate is honest only if its report says so.
- **Resolution condition:** the scorer's census of numeric site-quarter cells by identity coverage (U2).
- **Materiality:** moderate. It touches the "vacuous gate" loss and what finance reviewers will be told.
- **Uncertainty basis:** moderate. It depends on U2.

### P00-BASELINE-C11

- **Type:** deductive (sequencing).
- **Statement:** The plan's out-of-sample test can only happen if a label-anchored importer exists and is frozen
  (hashed) before 9 Nov 2026. The test is that GATE-BULL-01 "must pass on" the Q3 2026 workbook, "a workbook it was
  not developed against", and that it "imports on release day without code changes". No such importer exists at
  HEAD (C5). If the importer is written after 9 Nov, the Q3 2026 file becomes in-sample, and the plan's only blind
  test of layout robustness is lost unless it is moved to the Q4 2026 workbook (February 2027).
- **Scope:** plan lines 7384–7386, BD-271 (line 7722), E454, E685.
- **Evidence:** as cited, plus C5. The date gap is 11 days from the case date (2026-10-29).
- **Inference:** a frozen importer digest (code revision plus binary sha256), recorded before release, is what makes
  the release-day run a real test. Plan text does not require recording it.
- **Assumptions:** an importer can be made label-anchored for the ten known vintages within 11 days. If not, the
  fallback (Q4 2026 as blind test) applies.
- **Alternative/defeater check:** the developer could avoid opening the Q3 file before running the frozen importer.
  That honour-system alternative is weaker than a recorded digest, and it fails once anyone opens the file.
- **Resolution condition:** a recorded importer digest dated before 9 Nov 2026, or a plan edit naming the Q4 2026
  workbook as the blind test.
- **Materiality:** moderate-high. It decides whether M0 shows robustness or only fit.
- **Uncertainty basis:** low for the logic. Moderate for the 11-day feasibility.

### P00-BASELINE-C12

- **Type:** deductive and value.
- **Statement:** The M0 "FY2026 per-site forecasts frozen and hashed before the February 2027 release" will be
  hashed during M0 (Nov 2026–Jan 2027):
  - by then FY2026 has ended and Q1–Q3 2026 are published, so the forecast is in effect a Q4 2026 nowcast;
  - the plan names no baseline it must beat;
  - as specified, its score can show register-before-resolve discipline but not forecasting skill.
- **Scope:** plan lines 4495–4496, 7393–7394, 7412; BD-067, BD-068, BD-076; BET-02 (line 296).
- **Evidence:**
  - Plan text.
  - Barrick publishes per-site guidance: North Mara H1 2026 at 68 koz against 200–230 koz guidance, plan line 7457,
    E587/E588.
  - The FY2025 release was dated 5 Feb 2026 (W5 E607 URL).
- **Inference:** a finance reader (the BD-075 kill reviewers) will judge a forecast against guidance and naive
  baselines. Without them, "reported whatever they show" carries no comparative information.
- **Assumptions:** the reviewers value skill relative to guidance. This is a value assumption, attributed to the
  plan's BET-02 framing, not established.
- **Alternative/defeater check:** the plan may intend only to demonstrate the protocol at M0. If so, its wording
  should say that, to avoid an overclaim (F7, RISK-07).
- **Resolution condition:** the plan names baselines (Barrick per-site guidance midpoint; Q4 = Q3 persistence) and
  scores against them, or it relabels the M0 forecast as a protocol demonstration.
- **Materiality:** moderate. It feeds BET-02/BET-06 credibility at M1.
- **Uncertainty basis:** moderate. It rests on the value assumption.

### P00-BASELINE-C13

- **Type:** observation.
- **Statement:** The plan's document-control status is stale. Line 14 says "PLAN. No FrankenMining code exists."
  HEAD has 15 product crates, about 8,859 LOC (E675), and a running M0 command.
- **Scope:** plan §0 table, lines 12–14.
- **Evidence:** E673, E675, E676, E682. README line 65 and AGENTS line 537 are current.
- **Inference:** the plan is the declared technical constitution (AGENTS "Project Truth Sources"), so its status line
  should match the repository and state partial status per §0.2.
- **Alternative/defeater check:** none. The text is unambiguous.
- **Reversal condition:** not applicable. It is fixed by an edit.
- **Materiality:** low-moderate.
- **Uncertainty basis:** none.

### P00-BASELINE-C14

- **Type:** observation.
- **Statement:** Plan line 4484 presents "504% in gold-equivalent ounces (87 Moz ÷ 17.2 Moz)". That quotient is
  505.8%, so a reader who recomputes it gets 506%, not 504%. The plan explains the rounding only later, in
  OPEN-BULL-4 (line 4642: "506%, 499–514% under rounding").
- **Scope:** §21.6 line 4484 (same pattern in E604's gloss).
- **Evidence:** E604, E146; `python3 -I`: 87/17.2 = 5.0581.
- **Inference:** the figures in the text do not reproduce the headline number they are cited for. This is the
  pattern FrankenMining's own G-BULL-3 treats as a defect.
- **Alternative/defeater check:** the Circular's unrounded inputs presumably give 504%. That is the explanation the
  text should state where the number is given.
- **Reversal condition:** not applicable. It is fixed by an edit.
- **Materiality:** low-moderate. It is small, but it sits in an executive-facing section.
- **Uncertainty basis:** none for the arithmetic.

### P00-BASELINE-C15

- **Type:** observation, with a HYPOTHESIS about its effect.
- **Statement:** The public plan raises the Barrick Chair / Avathon lead-director link at lines 6923, 7170 and
  7197, and it is already on `origin/main`. Its own F1 countermeasure is "never be the party that raises the Chair
  conflict". Separately, AGENTS.md lines 267–268 say FrankenMining is "not … built for Barrick", while the plan's
  pre-mortem and M2 are framed around a Barrick pilot.
- **Scope:** the public repository text.
- **Evidence:** new: `git show origin/main:COMPREHENSIVE_PLAN_FOR_THE_DESIGN_OF_FRANKENMINING.md | grep -n Chair`
  returns lines 6923, 7170 and 7197. AGENTS.md lines 267–268. Plan §35.1 F1.
- **Inference:**
  - Observation: the plan does what its own countermeasure forbids.
  - HYPOTHESIS: if Barrick-class readers follow a link to the repository, the passage could weigh on the
    relationship the owner relies on.
  - Test of the hypothesis: none available to this pass (U6).
- **Assumptions:** the repository link would be shared with, or found by, the intended readers. Not established.
- **Alternative/defeater check:** the owner may judge the public discussion acceptable or already known to the
  readers. That is a value call for the owner, not an error.
- **Resolution condition:** owner decision on public positioning.
- **Materiality:** moderate, conditional on U6.
- **Uncertainty basis:** high for the effect. None for the observation.

### P00-BASELINE-C16

- **Type:** observation.
- **Statement:** NS-01 is tagged [M0/M2] and promises bars for "grade, recovery, tonnes, price-linked royalty,
  fuel, FX, sustaining capex". The plan's own §21.6 says public Mine Stats do not publish G&A, leases, exploration,
  accretion or credits per site, and that TCC → AISC is only a residual (lines 4525–4528). At M0 the waterfall can
  therefore show T, g, R, B, S, sustaining capex, royalties/oz and a residual, but no fuel or FX bar.
- **Scope:** plan line 388 against lines 4525–4528.
- **Evidence:** plan text; C1 T10/T11.
- **Inference:** the executive-facing scenario overstates what M0 can demonstrate. The fuel and FX bars first become
  possible at M2/M3 with site cost extracts.
- **Alternative/defeater check:** "[M0/M2]" may mean "partially at M0". The scenario text does not say which bars
  are M0.
- **Reversal condition:** a per-site public source for fuel and FX cost lines.
- **Materiality:** low-moderate.
- **Uncertainty basis:** low.

### P00-BASELINE-C17 (recommendation)

- **Statement:** Before commit 68e7091 is pushed:
  - remove both silent fallbacks (`BarrickCorpus::load_from_workspace` lines 236–241 and `Workbook::from_file`
    lines 121–137) and replace them with a typed refusal (non-zero exit) when a source file is absent;
  - remove, or explicitly relabel as synthetic, every literal attributed to Barrick: the North Mara literals, the
    RR inputs, the price decks, and the "Real Barrick corpus case" and "verified Barrick cell coordinates"
    comments;
  - move any synthetic workbook into a clearly named fixture lane whose outputs carry `evidence: Synthetic` and
    cannot count toward GATE-BULL-01;
  - reopen the beads whose "Done when" is unmet.
- **Supporting claims:** C1, C2, C3, C4, C6, C7.
- **Belief, value and action:**
  - Belief: the output is synthetic and partly false (C1–C3).
  - Value: the owner ranks a false statement to Barrick-class readers worst.
  - Action: stop it at the last reversible point (B7).
- **Cost:** small code edits and bead state changes. M0 will then visibly not run end to end, which is the true
  state (README line 65 already says so).
- **Reversibility:** fully reversible before push; irreversible after.
- **Acceptance test:**
  - `fmine --workspace <empty dir> assure public --issuer barrick` exits non-zero with a FMINE-REF refusal.
  - `grep -rn "build_canonical_barrick_xlsx_bytes" crates/*/src` returns hits only under `#[cfg(test)]` or a
    `synthetic` fixture module.
  - CI on a clean clone passes without claiming any Barrick figure.
- **Plan edit:** add a §21.8 guard row stating: "A public-lane source file is absent, unreadable or of unexpected
  layout → refuse (FMINE-REF); never substitute bytes or values." In §33.2, state that in-code or generated
  workbooks are synthetic fixtures and never stand in for a published source.

### P00-BASELINE-C18 (recommendation)

- **Statement:** Make the GATE-BULL-01 evidence a local real-data run recorded in `docs/delivery/` with:
  - the sha256 of each of the 18 real files (10 Mine Stats Q1 2024–Q2 2026, 8 AISC Q3 2024–Q2 2026);
  - a label-anchored importer;
  - per-number citations (`<file-period>:<Sheet>!<Cell>` + sha256 + Quarter Report page).

  Keep CI on labelled synthetic fixtures until U5 (licensing of derived fixtures) is decided.
- **Supporting claims:** C5, C6, C9; U5.
- **Cost:** moderate; this is the BD-271 work the plan already scopes.
- **Reversibility:** high.
- **Acceptance test:**
  - the run's payload reproduces Total Gold FY2025 = $1,637/oz on 3,318 koz sold (Q4_2025 Mine Stats YTD!G22,
    D22);
  - it reproduces North Mara Q2 2025 → Q2 2026 = +$1,285/oz (E137);
  - every payload number resolves to a cell that a reviewer opens and confirms.

### P00-BASELINE-C19 (recommendation, plan edits)

- **Supporting claims:** C7, C10, C11, C12, C13, C14, C16.
- **Plan edits:**
  1. **§39 rules (line 7611).** Add: "A closure reason must name the bead's own 'Done when' evidence (test name,
     gate record id or decision entry). Closing a bead with an open dependency is refused by tooling. A generic
     multi-bead reason is not evidence." (C7)
  2. **§21.6 lines 4521–4524 and §37.3 lines 7397–7402.** The scorer must report three things separately:
     - cells reproduced with at least one independent identity check;
     - cells matched by extraction only;
     - explained and refused cells.

     State the share of identity-checked cells next to the 95% headline. (C10)
  3. **§37.3 lines 7384–7386 and BD-271.** Require the importer's revision and binary digest to be recorded before
     9 Nov 2026. If that is missed, the Q4 2026 workbook becomes the blind test. (C11)
  4. **§21.6 line 4495 and §37.3 line 7393.** Name the baselines (Barrick per-site guidance midpoint; Q4 = Q3
     persistence) and state that the M0 forecast demonstrates the protocol, and is not evidence of skill. (C12)
  5. **Line 14.** Correct the status. **Line 4484:** state that the quotient of the rounded inputs is 506% and that
     504% is the published figure. **Line 388:** mark which NS-01 bars are possible at M0. (C13, C14, C16)
- **Cost:** low (text).
- **Reversibility:** full.
- **Acceptance test:** the plan-integrity script (E682) passes, and each edit cites its claim's evidence ids.

### P00-BASELINE-C20 (question)

- **Question:** Who are the two finance reviewers whose "nothing new" verdict kills GATE-BULL-01 (BD-075), and are
  they available by January 2027?
- **Why it matters:** the M0 kill is unevaluable without them (U3). Their profile (Barrick-internal, ex-issuer
  finance, sell-side) also decides which discrepancy-register items count as "new".
- **Evidence:** plan line 4590 and 7403; BD-075 (open).
- **Resolution:** owner names them, or names the selection rule, in §37.3.

## Checked nulls (where the plan or code holds up)

- **LMDI engine** (`fmine-metrics/src/lmdi.rs` lines 75–125). The identity AISC = C/oz_sold, with
  oz_sold = T·g·R·B·S, is implemented with log-mean weights, and the factor effects sum exactly to the delta. The
  defect is the inputs (C3), not the engine.
- **GB-26 synthetic waterfall** (plan lines 4596–4611) recomputes exactly. L(1,744.98, 1,600) = 1,671.5, which
  gives:
  - grade +68.23;
  - cost +41.27;
  - recovery +18.68;
  - tonnes +16.80;
  - sum +144.98.
- **GB-29 VOI card** (lines 4770–4786) recomputes:
  - P(test high) = 0.57; EV with test = 141.0; EVSI = 41.0;
  - the accuracy flip is 0.730;
  - the prior flip is 0.791 (the plan says "about 0.78", which is close but slightly low).
- **E676 internal arithmetic is correct as arithmetic.** 5,882,000/3,973 = 1,480.49 and 5,332,000/3,255 = 1,638.10.
  The inputs are what is wrong (C2).
- **Q3 2026 date and M0 window** are mutually consistent (E454, E685, plan line 7378).

## NULL OR BLOCKED RESULT

Not a null result. One reversal test was blocked: running the M0 binary against an empty workspace (C1). The binary
directory named in the brief is absent on this host, and the brief forbids new builds. The exact cell-count
decomposition in C1 carries that claim without it.
