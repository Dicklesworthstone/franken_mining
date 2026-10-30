# Independent readers for the public lane

These Python scripts (standard library only) read the registered Barrick workbooks without using `fmine-io` or
`fmine-public`. `fidelity.py` and `census2.py` were written and their outputs pinned before the Rust code was
written against them (MOR_CASE, "M0 machine run", E820–E823). The others are research checks, and their headers
say what they are independent of.

Rule: a mismatch between a script here and the Rust output is a defect in one of the two until proven otherwise.
Never resolve it by editing a script to agree with the Rust output.

The workbooks are not in git (OPEN-R7-1). `repro/barrick/fetch.sh` (BD-339) will fetch and verify them; until
then they sit in `corpus_of_input_docs/downloaded/mine_stats/`.

| Evidence | Command (repository root) | Expected (2026-10-29) |
|---|---|---|
| E820 extraction fidelity | `python3 -I scripts/oracle/fidelity.py corpus_of_input_docs/downloaded/mine_stats/*.xlsx` | one line per file: visible and hidden sheets, numeric, placeholder, formula-error and text counts, numeric digest; totals 101, 6, 30388, 3952, 1092, 6537 |
| E821 and E822 identity ledger | `python3 -I scripts/oracle/census2.py .` | `AISC tables (visible): 34 instances: 2595`; footing i 1004, iii 4, v 6, np 24; per ounce i 1116, ii 3, iv 17, v 385, np 36 |
| E826 stated share rule (2026-10-30) | `python3 -I scripts/oracle/share_census.py .` | 379 lines, then `{"sites": 328, "fit": 263, "miss": 65, "unconfirmed": 0, "regions_v": 51, "no_footnote": 0, "no_registered_share": 0}` (Long Canyon registered on E827) |
| E828 hidden scope (2026-10-30) | `python3 -I scripts/oracle/hidden_census.py .` | 435 lines, then `{"hidden_sheets": 6, "instances": 435, "footing": {"i": 169, "iii": 1, "np": 4}, "per_oz": {"i": 235, "iv": 17, "np": 6, "v": 3}}` |
| bd-jg43.1 number formats (2026-10-30) | `python3 -I scripts/oracle/formats.py corpus_of_input_docs/downloaded/mine_stats/*.xlsx` | 18 file lines, then 10 formats, `"numeric_total":30388,"non_integer_total":6256` |
| bd-prmn.8 K.4 one-cell check (2026-10-30) | `python3 -I scripts/oracle/one_cell_cos.py --quiet .` | `{"columns": 519, "pass": 374, "fail": 82, "refused": 63, "k4_fail": 17, "other_fail": 65, "verdict": "basis rule", ...}` |
| bd-jg43.2 displayed values (2026-10-30) | `python3 -I scripts/oracle/formats.py --displayed corpus_of_input_docs/downloaded/mine_stats/*.xlsx` | 18 file lines with `displayed_digest`, then `{"files":18,"shown":27367,"token":2873,"refused":148}` |
| bd-prmn.4 region against members (2026-10-30) | `python3 -I scripts/oracle/region_census.py .`, then with `--hidden` | 51 lines, then `{"regions": 51, "refuted": 10, "not_refuted": 41, "refused": 0}`; hidden: 3 lines, then `{"regions": 3, "refuted": 1, "not_refuted": 2, "refused": 0}` |
| E838 ledger on displayed values (2026-10-30) | `python3 -I scripts/oracle/census2.py --displayed .` | `AISC tables (visible): 34 instances: 2595`; footing i 1010, iii 4, np 24; per ounce i 1122, ii 3, iv 17, v 379, np 36; alt construction 1014 and 1000 |

| Script | What it is |
|---|---|
| `fidelity.py` | Extraction-fidelity reader; its docstring is the digest spec shared with `numeric_cells_digest`. |
| `census2.py` | Identity-ledger census, visible sheets only; the reference for `fmine-public`'s ledger. `--displayed` (bd-jg43.3) reads each numeric cell as `formats.py --displayed` shows it; it was added after the Rust change ran, by the agent that wrote both, so it is a same-author check (E838). |
| `census.py` | Version 1 of the census. It counts hidden sheets too (superseded by E821). Kept because `census2.py` imports its subject list. |
| `xl.py` | Minimal xlsx loader used by both censuses. |
| `share_census.py` | BD-331.S3 stated-share census over `census2.py`'s "attributable share needed" instances; written by a separate agent that did not read `crates/` or `rate_census.py`. |
| `region_census.py` | BD-331.S4 region against members: each region's cost of sales per ounce that `share_census.py` leaves (v), checked against its member sites weighted by attributable ounces sold, under the W1 region map keyed by release. It decides the exact problem by the weighted mean's range over the members' ounce-box vertices, a different method from the kernel's McCormick relaxation. Written by a separate agent from the rule as stated, without reading `crates/`, before the Rust regional rule. Its docstring lists its eight judgement calls. |
| `hidden_census.py` | BD-331.S2 hidden scope. It runs `census2.py` and `share_census.py` unchanged on the hidden AISC sheets and adds no ledger logic. Written by the agent that implemented the Rust hidden scope, before that code ran on the corpus. |
| `format_census.py` | Number-format census (cell-number-formats bead); research check. |
| `formats.py` | bd-jg43.1 number-format census per file: format code to [numeric, non-integer], with non-integer decided on the exact stored decimal. Written and run before the Rust styles reader, by the agent that then wrote it. It counts numeric cells as `fidelity.py` does. `--displayed` (bd-jg43.2) adds each numeric cell's displayed text and a per-file digest; it was written before the Rust interpreter, by the same agent, and its two rounding rules are hypotheses until the LibreOffice cross-check runs. |
| `as_shown_check.sh`, `as_shown_check.py` | bd-jg43.2 cross-check: LibreOffice's "as shown" CSV text against `formats.py --displayed`, on a fixture of hypothesis cases and 50 stratified cells per file. Runs in the `as-shown` job of `.github/workflows/real-files.yml`; issuer values are logged only with `--values`. |
| `one_cell_cos.py` | bd-prmn.8, Appendix K.4's cheapest common test: the one-cell cost-of-sales-per-ounce check on every registered-subject column of the visible AISC tables, at 100% and at the registered share, with the implied ounces and the K.4 verdict. It reuses `census2.py`'s layout and `share_census.py`'s shares, footnote and label rules. Written before the Rust diagnostic, by the agent that then wrote it (same origin). |
| `vintage_census.py` | Cross-vintage stored-value comparison; research check. |
| `rate_census.py` | Per-ounce check that mirrors `aisc.rs`; NOT independent evidence. |
