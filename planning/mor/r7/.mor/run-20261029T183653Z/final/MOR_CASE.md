# MOR Case — Round 7 review of the FrankenMining plan and M0 code

RUN_ID `20261029T183653Z` · case built 2026-10-29 by the lead agent (SwiftBasin) · repository HEAD `68e7091`.
This file is the neutral case for one modes-of-reasoning run. It records what the artifacts and executions show;
it holds no thesis. The project's standing evidence register is `planning/mor/MOR_CASE.md` (E001–E672); this
case imports those ids by reference and adds E673–E699 for this run.

## Decision Contract

- **Primary question.** Before the M0 gate (GATE-BULL-01; Q3 2026 workbook released 9 Nov 2026; FY2026 forecasts
  to be hashed before the February 2027 release), which material claims, numbers, gates, dependencies and design
  choices in the FrankenMining plan and its committed M0 code are wrong, unsupported, inconsistent or
  mis-sequenced, and which changes most raise the chance that M0–M2 produce outputs that a Barrick-class
  executive committee, qualified persons (QPs) and site teams would trust and use?
- **Decision owner.** The repository owner (the user). Plan edits are theirs to accept; agents propose and
  implement. The plan says the plan wins over AGENTS.md on technical rules (AGENTS.md "Project Truth Sources").
- **Horizon.** Now to the program gate at month 12 (plan §34.5), weighted toward M0–M2 (plan §37).
- **Scope.** `COMPREHENSIVE_PLAN_FOR_THE_DESIGN_OF_FRANKENMINING.md`, `planning/` (research, corpus notes, Brenner
  passes, audits, evidence register), `README.md`, `AGENTS.md`, `crates/`, `registries/`, `docs/`, `.beads/`,
  `.github/workflows/ci.yml`. **Excluded:** changing the substrate (Rust, asupersync, the FrankenSuite siblings),
  implementing code, and new feature ideation (a separate idea duel runs in parallel).
- **Success criterion.** A ranked set of independently audited corrections and recommendations that can be
  written into the plan with evidence ids, plus checked nulls where the plan holds up.
- **Loss function (attributed to the owner's stated goals).** Costliest: a false or unsupported statement reaching
  Barrick-class management, and a gate that
  passes vacuously or on fabricated inputs. Next: a gate that cannot be passed as written; a dependency that
  silently blocks M0–M2. Then: missed high-value capability. Delay is a moderate cost.
- **Constraints.** Public lane only (no MNPI, no site data, no personal data); source documents never committed;
  Rust 2024 + asupersync `=0.5.0`; franken_accounting by subprocess; frankensim lift-only; AI agents as the
  development team; local disk on the build host is low (about 18 GB free, E690).
- **Changeability.** The plan is fully reversible. The code is a first slice (about 8,900 LOC, E675). Bead state is
  reversible. The public GitHub repository is not: what is pushed is published (E673).

## Project Identity and Non-Negotiables

- Evidence-native system of record for integrated gold, copper, silver and tin miners; every number carries
  support, evidence class, calibration flag, UU term and provenance (AGENTS.md; plan §6–§10).
- Non-goals: not a control system (INV-011), never classifies reserves or signs dams or slopes (INV-002), not a
  general ledger, not a replacement for SAP/Pitram/FMS/Leapfrog/Whittle/Deswik in years 1–2 (plan §4.2).
- Honesty rules: no number without a source; register before resolve (INV-008); refuse, never clamp or plug;
  no evidence upgrade through derivation (INV-006); "implemented" is defined in plan §0.2.
- RULE 0.1 in AGENTS.md: no faked tests, no mocks or fixtures presented as live proof, no hard-coded success paths.

## Deployment and Use Context

- No deployment yet. The first deliverable is M0, `fmine assure public --issuer barrick`, on public Barrick Mine
  Stats and AISC workbooks (plan §37; README).
- Intended readers: executive committee and board, CFO/treasury, QPs, mine planners, metallurgists, tailings
  engineers, site GMs, exploration, investor relations, regulators, JV partners, and their AI agents (plan §3).
- The repository is public on GitHub (`Dicklesworthstone/franken_mining`); `main` is mirrored to `master`.

## Evidence Index

New evidence for this run (artifacts and executions on 2026-10-29):

- E673 — Git: HEAD `68e7091` "feat(m0): implement WP-00..WP-03 and WP-06 workspace, S0/K0/E/V/U crates, and
  Barrick M0 public assurance demonstrator" (author Jeff Emanuel, 2026-10-29 13:42 -0400). Adds 15 crates plus
  `xtask`, registries, ADR-RNG-01, CI, and `.beads/` (320 issues). The message claims beads bd-001..bd-066,
  bd-071, bd-111..bd-117, bd-225, bd-227, bd-257, bd-271..bd-275, bd-277 and bd-308..bd-311 are implemented.
  `git status -sb` shows `main...origin/main [ahead 1]`: the commit is not yet pushed.
- E674 — Execution: `rch exec -- env CARGO_TARGET_DIR=$TMPDIR/rch_target_fmine cargo test --workspace` (built
  locally by the rch speed gate). 59 unit tests pass, 0 fail, 0 ignored, over 16 test binaries; doc-tests 0.
  Per crate: array 2, calib 4, claim 4, cli 2, core 11, crs 3, decide 2, evidence 3, io 2, metrics 4, numerics 6,
  public 2, registry 1, time 3, units 5, xtask 5. `grep -rn '#\[test\]' crates | wc -l` = 59. Log:
  `.mor/run-20261029T183653Z/repro/cargo_test.log`.
- E675 — Execution: `cargo run -p xtask -- check-all` (run on rch worker ts1) printed
  "GATE-LAYERS-00 PASS: 60 crates registered, 8859 actual Rust LOC (planned 1128000, ceiling 1298000)". The
  wrapper exited 102 after the gate printed PASS because rch rejected syncing a foreign-architecture artifact
  (RCH-E327). Log: `.mor/run-20261029T183653Z/repro/xtask_check_all.log`.
- E676 — Execution: the debug binary `fmine assure public --issuer barrick` exits 0 and prints one JSON
  envelope (human mode and `--json` print the same bytes). Two `--json` runs are byte-identical (1,108 bytes).
  Payload: `workbooks_verified` 18, `visible_sheets_verified` 36, `cells_verified` 43,992, `numeric_cells` 43,938,
  `placeholder_cells` 36, `quarantined_items` 36, gold FY2024 computed AISC 1,480.49 vs reported 1,484,
  gold FY2025 1,638.10 vs 1,638, North Mara LMDI Q1→Q2 2024 (1,297 → 2,582), reserve replacement 2024
  organic 190% / headline 504%, `discrepancy_count` 41, a `state_root` hash. Outputs:
  `.mor/run-20261029T183653Z/repro/assure_human.txt`, `assure_run1.json`, `assure_run2.json`.
- E677 — Filesystem: none of the 18 file names in `BARRICK_18_WORKBOOKS` (`crates/fmine-public/src/lib.rs`
  lines 36–) exists under `corpus_of_input_docs/` (all 18 checked: absent). The Barrick workbooks present
  locally are `corpus_of_input_docs/downloaded/mine_stats/Barrick_Q{1–4}_{2024–2026}_{Mine_Stats|AISC}.xlsx`
  (20 files) plus `corpus_of_input_docs/Barrick_Q1_2026_Mine_Stats.xlsx` and `Barrick_Q2_2026_Mine_Stats.xlsx`.
- E678 — Code: `BarrickCorpus::load_from_workspace` (`crates/fmine-public/src/lib.rs` lines 224–265) reads each
  named file and, when the read fails, substitutes `fmine_io::zip_inflate::build_canonical_barrick_xlsx_bytes()`
  (`crates/fmine-io/src/zip_inflate.rs` line 411–), an in-code workbook with sheets "Gold AISC", "Copper AISC" and
  a hidden "YTD Q3 2025".
- E679 — Code: `run_barrick_m0_assurance` (`crates/fmine-public/src/lib.rs` lines 425–) builds the North Mara LMDI
  states from literal constants (lines 437–455) with `.expect("valid North Mara LMDI")`, and computes reserve
  replacement from literals `ReserveReplacementBridge::compute(3.91, 7.429, 12.277)` (line 456).
- E680 — Beads: `.beads/issues.jsonl` holds 320 issues; `br list --status=closed` returns 87.
- E681 — Config: root `Cargo.toml` has `license = "MIT"` while `LICENSE` is "MIT License (with OpenAI/Anthropic
  Rider)"; workspace lints set `unsafe_code = "forbid"`, clippy `all = warn`, `correctness = deny`, and do not
  deny `unwrap_used`, `expect_used` or `panic`.
- E682 — Plan file at HEAD: 8,902 lines, sha256 `e8429e48d43ca739b72d4e49d03c92981d2991ae2bc338146d3262d9e5451390`.
  The whole-plan integrity script used in earlier rounds reports 291 headings, 320 beads, 596 known E ids, one
  unresolved "§0" reference (line 41) and 76 subsection references whose parent section exists.
- E683 — CI: `.github/workflows/ci.yml` runs `cargo run -p xtask -- check-all`, `cargo check --workspace
  --all-targets` and `cargo test --workspace` on ubuntu-latest and macos-14 in dev and release; no fmt or clippy
  step.
- E684 — Registries at HEAD: `registries/{calendars,crs,lifts,loc_budget,refusals,units,verbs,workspace_topology}.toml`.
- E685 — Plan text: GATE-BULL-01 acceptance (plan line 7397 and §21 table line 4590): "≥ 95% of site-quarter
  values reproduced to published rounding", the rest explained or refused, zero uncited numbers, byte-identical
  reruns, and FY2026 per-site forecasts hashed before the February 2027 release; Q3 2026 (9 Nov 2026) is the
  only open DATA-GATE (plan lines 4504, 7384–7385, 8672).
- E690 — Execution: `sbh check --need 20G` on the build host reports 18.3 GB free, status critical.

Imported evidence (defined in the standing register and the corpus and research notes; ids only):

- E001 — imported; defined in `planning/mor/MOR_CASE.md`
- E002 — imported; defined in `planning/mor/MOR_CASE.md`
- E003 — imported; defined in `planning/mor/MOR_CASE.md`
- E004 — imported; defined in `planning/mor/MOR_CASE.md`
- E005 — imported; defined in `planning/mor/MOR_CASE.md`
- E006 — imported; defined in `planning/mor/MOR_CASE.md`
- E007 — imported; defined in `planning/mor/MOR_CASE.md`
- E008 — imported; defined in `planning/mor/MOR_CASE.md`
- E009 — imported; defined in `planning/mor/MOR_CASE.md`
- E010 — imported; defined in `planning/mor/MOR_CASE.md`
- E011 — imported; defined in `planning/mor/MOR_CASE.md`
- E012 — imported; defined in `planning/mor/MOR_CASE.md`
- E013 — imported; defined in `planning/mor/MOR_CASE.md`
- E014 — imported; defined in `planning/mor/MOR_CASE.md`
- E015 — imported; defined in `planning/mor/MOR_CASE.md`
- E016 — imported; defined in `planning/mor/MOR_CASE.md`
- E017 — imported; defined in `planning/mor/MOR_CASE.md`
- E018 — imported; defined in `planning/mor/MOR_CASE.md`
- E019 — imported; defined in `planning/mor/MOR_CASE.md`
- E020 — imported; defined in `planning/mor/MOR_CASE.md`
- E021 — imported; defined in `planning/mor/MOR_CASE.md`
- E022 — imported; defined in `planning/mor/MOR_CASE.md`
- E023 — imported; defined in `planning/mor/MOR_CASE.md`
- E024 — imported; defined in `planning/mor/MOR_CASE.md`
- E025 — imported; defined in `planning/mor/MOR_CASE.md`
- E026 — imported; defined in `planning/mor/MOR_CASE.md`
- E027 — imported; defined in `planning/mor/MOR_CASE.md`
- E028 — imported; defined in `planning/mor/MOR_CASE.md`
- E029 — imported; defined in `planning/mor/MOR_CASE.md`
- E030 — imported; defined in `planning/mor/MOR_CASE.md`
- E031 — imported; defined in `planning/mor/MOR_CASE.md`
- E032 — imported; defined in `planning/mor/MOR_CASE.md`
- E033 — imported; defined in `planning/mor/MOR_CASE.md`
- E034 — imported; defined in `planning/mor/MOR_CASE.md`
- E035 — imported; defined in `planning/mor/MOR_CASE.md`
- E101 — imported; defined in `planning/corpus/C1_barrick_public_stats.md`
- E102 — imported; defined in `planning/corpus/C1_barrick_public_stats.md`
- E103 — imported; defined in `planning/corpus/C1_barrick_public_stats.md`
- E104 — imported; defined in `planning/corpus/C1_barrick_public_stats.md`
- E105 — imported; defined in `planning/corpus/C1_barrick_public_stats.md`
- E106 — imported; defined in `planning/corpus/C1_barrick_public_stats.md`
- E107 — imported; defined in `planning/corpus/C1_barrick_public_stats.md`
- E108 — imported; defined in `planning/corpus/C1_barrick_public_stats.md`
- E109 — imported; defined in `planning/corpus/C1_barrick_public_stats.md`
- E110 — imported; defined in `planning/corpus/C1_barrick_public_stats.md`
- E111 — imported; defined in `planning/corpus/C1_barrick_public_stats.md`
- E112 — imported; defined in `planning/corpus/C1_barrick_public_stats.md`
- E113 — imported; defined in `planning/corpus/C1_barrick_public_stats.md`
- E114 — imported; defined in `planning/corpus/C1_barrick_public_stats.md`
- E115 — imported; defined in `planning/corpus/C1_barrick_public_stats.md`
- E116 — imported; defined in `planning/corpus/C1_barrick_public_stats.md`
- E117 — imported; defined in `planning/corpus/C1_barrick_public_stats.md`
- E118 — imported; defined in `planning/corpus/C1_barrick_public_stats.md`
- E119 — imported; defined in `planning/corpus/C1_barrick_public_stats.md`
- E120 — imported; defined in `planning/corpus/C1_barrick_public_stats.md`
- E121 — imported; defined in `planning/corpus/C1_barrick_public_stats.md`
- E122 — imported; defined in `planning/corpus/C1_barrick_public_stats.md`
- E123 — imported; defined in `planning/corpus/C1_barrick_public_stats.md`
- E124 — imported; defined in `planning/corpus/C1_barrick_public_stats.md`
- E125 — imported; defined in `planning/corpus/C1_barrick_public_stats.md`
- E126 — imported; defined in `planning/corpus/C1_barrick_public_stats.md`
- E127 — imported; defined in `planning/corpus/C1_barrick_public_stats.md`
- E128 — imported; defined in `planning/corpus/C1_barrick_public_stats.md`
- E129 — imported; defined in `planning/corpus/C1_barrick_public_stats.md`
- E130 — imported; defined in `planning/corpus/C1_barrick_public_stats.md`
- E131 — imported; defined in `planning/corpus/C1_barrick_public_stats.md`
- E132 — imported; defined in `planning/corpus/C1_barrick_public_stats.md`
- E133 — imported; defined in `planning/corpus/C1_barrick_public_stats.md`
- E134 — imported; defined in `planning/corpus/C1_barrick_public_stats.md`
- E135 — imported; defined in `planning/corpus/C1_barrick_public_stats.md`
- E136 — imported; defined in `planning/corpus/C1_barrick_public_stats.md`
- E137 — imported; defined in `planning/corpus/C1_barrick_public_stats.md`
- E138 — imported; defined in `planning/corpus/C1_barrick_public_stats.md`
- E139 — imported; defined in `planning/corpus/C1_barrick_public_stats.md`
- E140 — imported; defined in `planning/corpus/C1_barrick_public_stats.md`
- E141 — imported; defined in `planning/corpus/C1_barrick_public_stats.md`
- E142 — imported; defined in `planning/corpus/C1_barrick_public_stats.md`
- E143 — imported; defined in `planning/corpus/C1_barrick_public_stats.md`
- E144 — imported; defined in `planning/corpus/C1_barrick_public_stats.md`
- E145 — imported; defined in `planning/corpus/C1_barrick_public_stats.md`
- E146 — imported; defined in `planning/mor/MOR_CASE.md`
- E147 — imported; defined in `planning/corpus/C1_barrick_public_stats.md`
- E148 — imported; defined in `planning/corpus/C1_barrick_public_stats.md`
- E149 — imported; defined in `planning/corpus/C1_barrick_public_stats.md`
- E151 — imported; defined in `planning/corpus/C2_nevada_technical_reports.md`
- E152 — imported; defined in `planning/corpus/C2_nevada_technical_reports.md`
- E153 — imported; defined in `planning/corpus/C2_nevada_technical_reports.md`
- E154 — imported; defined in `planning/corpus/C2_nevada_technical_reports.md`
- E155 — imported; defined in `planning/corpus/C2_nevada_technical_reports.md`
- E156 — imported; defined in `planning/corpus/C2_nevada_technical_reports.md`
- E157 — imported; defined in `planning/corpus/C2_nevada_technical_reports.md`
- E158 — imported; defined in `planning/corpus/C2_nevada_technical_reports.md`
- E159 — imported; defined in `planning/corpus/C2_nevada_technical_reports.md`
- E160 — imported; defined in `planning/corpus/C2_nevada_technical_reports.md`
- E161 — imported; defined in `planning/corpus/C2_nevada_technical_reports.md`
- E162 — imported; defined in `planning/corpus/C2_nevada_technical_reports.md`
- E163 — imported; defined in `planning/corpus/C2_nevada_technical_reports.md`
- E164 — imported; defined in `planning/corpus/C2_nevada_technical_reports.md`
- E165 — imported; defined in `planning/corpus/C2_nevada_technical_reports.md`
- E166 — imported; defined in `planning/corpus/C2_nevada_technical_reports.md`
- E167 — imported; defined in `planning/corpus/C2_nevada_technical_reports.md`
- E168 — imported; defined in `planning/corpus/C2_nevada_technical_reports.md`
- E169 — imported; defined in `planning/corpus/C2_nevada_technical_reports.md`
- E170 — imported; defined in `planning/corpus/C2_nevada_technical_reports.md`
- E171 — imported; defined in `planning/corpus/C2_nevada_technical_reports.md`
- E172 — imported; defined in `planning/corpus/C2_nevada_technical_reports.md`
- E173 — imported; defined in `planning/corpus/C2_nevada_technical_reports.md`
- E174 — imported; defined in `planning/corpus/C2_nevada_technical_reports.md`
- E175 — imported; defined in `planning/corpus/C2_nevada_technical_reports.md`
- E176 — imported; defined in `planning/corpus/C2_nevada_technical_reports.md`
- E177 — imported; defined in `planning/corpus/C2_nevada_technical_reports.md`
- E178 — imported; defined in `planning/corpus/C2_nevada_technical_reports.md`
- E179 — imported; defined in `planning/corpus/C2_nevada_technical_reports.md`
- E180 — imported; defined in `planning/corpus/C2_nevada_technical_reports.md`
- E181 — imported; defined in `planning/corpus/C2_nevada_technical_reports.md`
- E182 — imported; defined in `planning/corpus/C2_nevada_technical_reports.md`
- E183 — imported; defined in `planning/corpus/C2_nevada_technical_reports.md`
- E184 — imported; defined in `planning/corpus/C2_nevada_technical_reports.md`
- E185 — imported; defined in `planning/corpus/C2_nevada_technical_reports.md`
- E186 — imported; defined in `planning/corpus/C2_nevada_technical_reports.md`
- E187 — imported; defined in `planning/corpus/C2_nevada_technical_reports.md`
- E188 — imported; defined in `planning/corpus/C2_nevada_technical_reports.md`
- E189 — imported; defined in `planning/corpus/C2_nevada_technical_reports.md`
- E190 — imported; defined in `planning/corpus/C2_nevada_technical_reports.md`
- E191 — imported; defined in `planning/corpus/C2_nevada_technical_reports.md`
- E192 — imported; defined in `planning/corpus/C2_nevada_technical_reports.md`
- E193 — imported; defined in `planning/corpus/C2_nevada_technical_reports.md`
- E194 — imported; defined in `planning/corpus/C2_nevada_technical_reports.md`
- E195 — imported; defined in `planning/corpus/C2_nevada_technical_reports.md`
- E196 — imported; defined in `planning/corpus/C2_nevada_technical_reports.md`
- E197 — imported; defined in `planning/corpus/C2_nevada_technical_reports.md`
- E198 — imported; defined in `planning/corpus/C2_nevada_technical_reports.md`
- E199 — imported; defined in `planning/corpus/C2_nevada_technical_reports.md`
- E201 — imported; defined in `planning/corpus/C3_africa_technical_reports.md`
- E202 — imported; defined in `planning/corpus/C3_africa_technical_reports.md`
- E203 — imported; defined in `planning/corpus/C3_africa_technical_reports.md`
- E204 — imported; defined in `planning/corpus/C3_africa_technical_reports.md`
- E205 — imported; defined in `planning/corpus/C3_africa_technical_reports.md`
- E206 — imported; defined in `planning/corpus/C3_africa_technical_reports.md`
- E207 — imported; defined in `planning/corpus/C3_africa_technical_reports.md`
- E208 — imported; defined in `planning/corpus/C3_africa_technical_reports.md`
- E209 — imported; defined in `planning/corpus/C3_africa_technical_reports.md`
- E210 — imported; defined in `planning/corpus/C3_africa_technical_reports.md`
- E211 — imported; defined in `planning/mor/MOR_CASE.md`
- E212 — imported; defined in `planning/mor/MOR_CASE.md`
- E213 — imported; defined in `planning/corpus/C3_africa_technical_reports.md`
- E214 — imported; defined in `planning/corpus/C3_africa_technical_reports.md`
- E215 — imported; defined in `planning/corpus/C3_africa_technical_reports.md`
- E216 — imported; defined in `planning/corpus/C3_africa_technical_reports.md`
- E217 — imported; defined in `planning/corpus/C3_africa_technical_reports.md`
- E218 — imported; defined in `planning/corpus/C3_africa_technical_reports.md`
- E219 — imported; defined in `planning/corpus/C3_africa_technical_reports.md`
- E220 — imported; defined in `planning/corpus/C3_africa_technical_reports.md`
- E221 — imported; defined in `planning/corpus/C3_africa_technical_reports.md`
- E222 — imported; defined in `planning/corpus/C3_africa_technical_reports.md`
- E223 — imported; defined in `planning/corpus/C3_africa_technical_reports.md`
- E224 — imported; defined in `planning/corpus/C3_africa_technical_reports.md`
- E225 — imported; defined in `planning/corpus/C3_africa_technical_reports.md`
- E226 — imported; defined in `planning/corpus/C3_africa_technical_reports.md`
- E227 — imported; defined in `planning/corpus/C3_africa_technical_reports.md`
- E228 — imported; defined in `planning/corpus/C3_africa_technical_reports.md`
- E229 — imported; defined in `planning/corpus/C3_africa_technical_reports.md`
- E230 — imported; defined in `planning/corpus/C3_africa_technical_reports.md`
- E231 — imported; defined in `planning/corpus/C3_africa_technical_reports.md`
- E232 — imported; defined in `planning/corpus/C3_africa_technical_reports.md`
- E233 — imported; defined in `planning/corpus/C3_africa_technical_reports.md`
- E234 — imported; defined in `planning/corpus/C3_africa_technical_reports.md`
- E235 — imported; defined in `planning/corpus/C3_africa_technical_reports.md`
- E236 — imported; defined in `planning/corpus/C3_africa_technical_reports.md`
- E237 — imported; defined in `planning/corpus/C3_africa_technical_reports.md`
- E238 — imported; defined in `planning/corpus/C3_africa_technical_reports.md`
- E239 — imported; defined in `planning/corpus/C3_africa_technical_reports.md`
- E240 — imported; defined in `planning/corpus/C3_africa_technical_reports.md`
- E241 — imported; defined in `planning/corpus/C3_africa_technical_reports.md`
- E242 — imported; defined in `planning/corpus/C3_africa_technical_reports.md`
- E243 — imported; defined in `planning/corpus/C3_africa_technical_reports.md`
- E244 — imported; defined in `planning/corpus/C3_africa_technical_reports.md`
- E245 — imported; defined in `planning/corpus/C3_africa_technical_reports.md`
- E246 — imported; defined in `planning/corpus/C3_africa_technical_reports.md`
- E247 — imported; defined in `planning/corpus/C3_africa_technical_reports.md`
- E248 — imported; defined in `planning/corpus/C3_africa_technical_reports.md`
- E249 — imported; defined in `planning/corpus/C3_africa_technical_reports.md`
- E251 — imported; defined in `planning/corpus/C4_americas_png_reports.md`
- E252 — imported; defined in `planning/corpus/C4_americas_png_reports.md`
- E253 — imported; defined in `planning/corpus/C4_americas_png_reports.md`
- E254 — imported; defined in `planning/corpus/C4_americas_png_reports.md`
- E255 — imported; defined in `planning/corpus/C4_americas_png_reports.md`
- E256 — imported; defined in `planning/corpus/C4_americas_png_reports.md`
- E257 — imported; defined in `planning/corpus/C4_americas_png_reports.md`
- E258 — imported; defined in `planning/corpus/C4_americas_png_reports.md`
- E259 — imported; defined in `planning/corpus/C4_americas_png_reports.md`
- E260 — imported; defined in `planning/corpus/C4_americas_png_reports.md`
- E261 — imported; defined in `planning/corpus/C4_americas_png_reports.md`
- E262 — imported; defined in `planning/corpus/C4_americas_png_reports.md`
- E263 — imported; defined in `planning/corpus/C4_americas_png_reports.md`
- E264 — imported; defined in `planning/corpus/C4_americas_png_reports.md`
- E265 — imported; defined in `planning/corpus/C4_americas_png_reports.md`
- E266 — imported; defined in `planning/corpus/C4_americas_png_reports.md`
- E267 — imported; defined in `planning/corpus/C4_americas_png_reports.md`
- E268 — imported; defined in `planning/corpus/C4_americas_png_reports.md`
- E269 — imported; defined in `planning/corpus/C4_americas_png_reports.md`
- E270 — imported; defined in `planning/corpus/C4_americas_png_reports.md`
- E271 — imported; defined in `planning/corpus/C4_americas_png_reports.md`
- E272 — imported; defined in `planning/corpus/C4_americas_png_reports.md`
- E273 — imported; defined in `planning/corpus/C4_americas_png_reports.md`
- E274 — imported; defined in `planning/corpus/C4_americas_png_reports.md`
- E275 — imported; defined in `planning/corpus/C4_americas_png_reports.md`
- E276 — imported; defined in `planning/corpus/C4_americas_png_reports.md`
- E277 — imported; defined in `planning/corpus/C4_americas_png_reports.md`
- E278 — imported; defined in `planning/corpus/C4_americas_png_reports.md`
- E279 — imported; defined in `planning/corpus/C4_americas_png_reports.md`
- E280 — imported; defined in `planning/corpus/C4_americas_png_reports.md`
- E281 — imported; defined in `planning/corpus/C4_americas_png_reports.md`
- E282 — imported; defined in `planning/corpus/C4_americas_png_reports.md`
- E283 — imported; defined in `planning/corpus/C4_americas_png_reports.md`
- E284 — imported; defined in `planning/corpus/C4_americas_png_reports.md`
- E285 — imported; defined in `planning/corpus/C4_americas_png_reports.md`
- E286 — imported; defined in `planning/corpus/C4_americas_png_reports.md`
- E287 — imported; defined in `planning/corpus/C4_americas_png_reports.md`
- E288 — imported; defined in `planning/corpus/C4_americas_png_reports.md`
- E289 — imported; defined in `planning/corpus/C4_americas_png_reports.md`
- E290 — imported; defined in `planning/corpus/C4_americas_png_reports.md`
- E291 — imported; defined in `planning/corpus/C4_americas_png_reports.md`
- E292 — imported; defined in `planning/corpus/C4_americas_png_reports.md`
- E293 — imported; defined in `planning/corpus/C4_americas_png_reports.md`
- E294 — imported; defined in `planning/corpus/C4_americas_png_reports.md`
- E295 — imported; defined in `planning/corpus/C4_americas_png_reports.md`
- E296 — imported; defined in `planning/corpus/C4_americas_png_reports.md`
- E297 — imported; defined in `planning/corpus/C4_americas_png_reports.md`
- E298 — imported; defined in `planning/corpus/C4_americas_png_reports.md`
- E299 — imported; defined in `planning/corpus/C4_americas_png_reports.md`
- E301 — imported; defined in `planning/corpus/C5_reko_diq_esia.md`
- E302 — imported; defined in `planning/corpus/C5_reko_diq_esia.md`
- E303 — imported; defined in `planning/corpus/C5_reko_diq_esia.md`
- E304 — imported; defined in `planning/corpus/C5_reko_diq_esia.md`
- E305 — imported; defined in `planning/corpus/C5_reko_diq_esia.md`
- E306 — imported; defined in `planning/corpus/C5_reko_diq_esia.md`
- E307 — imported; defined in `planning/corpus/C5_reko_diq_esia.md`
- E308 — imported; defined in `planning/corpus/C5_reko_diq_esia.md`
- E309 — imported; defined in `planning/corpus/C5_reko_diq_esia.md`
- E310 — imported; defined in `planning/corpus/C5_reko_diq_esia.md`
- E311 — imported; defined in `planning/corpus/C5_reko_diq_esia.md`
- E312 — imported; defined in `planning/corpus/C5_reko_diq_esia.md`
- E313 — imported; defined in `planning/corpus/C5_reko_diq_esia.md`
- E314 — imported; defined in `planning/corpus/C5_reko_diq_esia.md`
- E315 — imported; defined in `planning/corpus/C5_reko_diq_esia.md`
- E316 — imported; defined in `planning/corpus/C5_reko_diq_esia.md`
- E317 — imported; defined in `planning/corpus/C5_reko_diq_esia.md`
- E318 — imported; defined in `planning/corpus/C5_reko_diq_esia.md`
- E319 — imported; defined in `planning/corpus/C5_reko_diq_esia.md`
- E320 — imported; defined in `planning/corpus/C5_reko_diq_esia.md`
- E321 — imported; defined in `planning/corpus/C5_reko_diq_esia.md`
- E322 — imported; defined in `planning/corpus/C5_reko_diq_esia.md`
- E323 — imported; defined in `planning/corpus/C5_reko_diq_esia.md`
- E324 — imported; defined in `planning/corpus/C5_reko_diq_esia.md`
- E325 — imported; defined in `planning/corpus/C5_reko_diq_esia.md`
- E326 — imported; defined in `planning/corpus/C5_reko_diq_esia.md`
- E327 — imported; defined in `planning/corpus/C5_reko_diq_esia.md`
- E328 — imported; defined in `planning/corpus/C5_reko_diq_esia.md`
- E329 — imported; defined in `planning/corpus/C5_reko_diq_esia.md`
- E330 — imported; defined in `planning/corpus/C5_reko_diq_esia.md`
- E331 — imported; defined in `planning/corpus/C5_reko_diq_esia.md`
- E332 — imported; defined in `planning/corpus/C5_reko_diq_esia.md`
- E333 — imported; defined in `planning/corpus/C5_reko_diq_esia.md`
- E334 — imported; defined in `planning/corpus/C5_reko_diq_esia.md`
- E335 — imported; defined in `planning/corpus/C5_reko_diq_esia.md`
- E336 — imported; defined in `planning/corpus/C5_reko_diq_esia.md`
- E337 — imported; defined in `planning/corpus/C5_reko_diq_esia.md`
- E338 — imported; defined in `planning/corpus/C5_reko_diq_esia.md`
- E339 — imported; defined in `planning/corpus/C5_reko_diq_esia.md`
- E340 — imported; defined in `planning/corpus/C5_reko_diq_esia.md`
- E341 — imported; defined in `planning/corpus/C5_reko_diq_esia.md`
- E342 — imported; defined in `planning/corpus/C5_reko_diq_esia.md`
- E343 — imported; defined in `planning/corpus/C5_reko_diq_esia.md`
- E344 — imported; defined in `planning/corpus/C5_reko_diq_esia.md`
- E345 — imported; defined in `planning/corpus/C5_reko_diq_esia.md`
- E346 — imported; defined in `planning/corpus/C5_reko_diq_esia.md`
- E347 — imported; defined in `planning/corpus/C5_reko_diq_esia.md`
- E348 — imported; defined in `planning/corpus/C5_reko_diq_esia.md`
- E349 — imported; defined in `planning/corpus/C5_reko_diq_esia.md`
- E351 — imported; defined in `planning/corpus/C6_textbook_techniques.md`
- E352 — imported; defined in `planning/corpus/C6_textbook_techniques.md`
- E353 — imported; defined in `planning/corpus/C6_textbook_techniques.md`
- E354 — imported; defined in `planning/corpus/C6_textbook_techniques.md`
- E355 — imported; defined in `planning/corpus/C6_textbook_techniques.md`
- E356 — imported; defined in `planning/corpus/C6_textbook_techniques.md`
- E357 — imported; defined in `planning/corpus/C6_textbook_techniques.md`
- E358 — imported; defined in `planning/corpus/C6_textbook_techniques.md`
- E359 — imported; defined in `planning/corpus/C6_textbook_techniques.md`
- E360 — imported; defined in `planning/corpus/C6_textbook_techniques.md`
- E361 — imported; defined in `planning/corpus/C6_textbook_techniques.md`
- E362 — imported; defined in `planning/corpus/C6_textbook_techniques.md`
- E363 — imported; defined in `planning/corpus/C6_textbook_techniques.md`
- E364 — imported; defined in `planning/corpus/C6_textbook_techniques.md`
- E365 — imported; defined in `planning/corpus/C6_textbook_techniques.md`
- E366 — imported; defined in `planning/corpus/C6_textbook_techniques.md`
- E367 — imported; defined in `planning/corpus/C6_textbook_techniques.md`
- E368 — imported; defined in `planning/corpus/C6_textbook_techniques.md`
- E369 — imported; defined in `planning/corpus/C6_textbook_techniques.md`
- E370 — imported; defined in `planning/corpus/C6_textbook_techniques.md`
- E371 — imported; defined in `planning/corpus/C6_textbook_techniques.md`
- E372 — imported; defined in `planning/corpus/C6_textbook_techniques.md`
- E373 — imported; defined in `planning/corpus/C6_textbook_techniques.md`
- E374 — imported; defined in `planning/corpus/C6_textbook_techniques.md`
- E375 — imported; defined in `planning/corpus/C6_textbook_techniques.md`
- E376 — imported; defined in `planning/corpus/C6_textbook_techniques.md`
- E377 — imported; defined in `planning/corpus/C6_textbook_techniques.md`
- E378 — imported; defined in `planning/corpus/C6_textbook_techniques.md`
- E379 — imported; defined in `planning/corpus/C6_textbook_techniques.md`
- E380 — imported; defined in `planning/corpus/C6_textbook_techniques.md`
- E381 — imported; defined in `planning/corpus/C6_textbook_techniques.md`
- E382 — imported; defined in `planning/corpus/C6_textbook_techniques.md`
- E383 — imported; defined in `planning/corpus/C6_textbook_techniques.md`
- E384 — imported; defined in `planning/corpus/C6_textbook_techniques.md`
- E385 — imported; defined in `planning/corpus/C6_textbook_techniques.md`
- E386 — imported; defined in `planning/corpus/C6_textbook_techniques.md`
- E387 — imported; defined in `planning/corpus/C6_textbook_techniques.md`
- E388 — imported; defined in `planning/corpus/C6_textbook_techniques.md`
- E389 — imported; defined in `planning/corpus/C6_textbook_techniques.md`
- E390 — imported; defined in `planning/corpus/C6_textbook_techniques.md`
- E391 — imported; defined in `planning/corpus/C6_textbook_techniques.md`
- E392 — imported; defined in `planning/corpus/C6_textbook_techniques.md`
- E393 — imported; defined in `planning/corpus/C6_textbook_techniques.md`
- E394 — imported; defined in `planning/corpus/C6_textbook_techniques.md`
- E395 — imported; defined in `planning/corpus/C6_textbook_techniques.md`
- E396 — imported; defined in `planning/corpus/C6_textbook_techniques.md`
- E397 — imported; defined in `planning/corpus/C6_textbook_techniques.md`
- E398 — imported; defined in `planning/corpus/C6_textbook_techniques.md`
- E399 — imported; defined in `planning/corpus/C6_textbook_techniques.md`
- E401 — imported; defined in `planning/research/W2_host_state_events.md`
- E402 — imported; defined in `planning/research/W2_host_state_events.md`
- E403 — imported; defined in `planning/research/W2_host_state_events.md`
- E404 — imported; defined in `planning/research/W2_host_state_events.md`
- E405 — imported; defined in `planning/research/W2_host_state_events.md`
- E406 — imported; defined in `planning/research/W2_host_state_events.md`
- E407 — imported; defined in `planning/research/W2_host_state_events.md`
- E408 — imported; defined in `planning/research/W2_host_state_events.md`
- E409 — imported; defined in `planning/research/W2_host_state_events.md`
- E410 — imported; defined in `planning/research/W2_host_state_events.md`
- E411 — imported; defined in `planning/research/W2_host_state_events.md`
- E412 — imported; defined in `planning/research/W2_host_state_events.md`
- E413 — imported; defined in `planning/research/W2_host_state_events.md`
- E414 — imported; defined in `planning/research/W2_host_state_events.md`
- E415 — imported; defined in `planning/research/W2_host_state_events.md`
- E416 — imported; defined in `planning/research/W2_host_state_events.md`
- E417 — imported; defined in `planning/research/W2_host_state_events.md`
- E418 — imported; defined in `planning/research/W2_host_state_events.md`
- E419 — imported; defined in `planning/research/W2_host_state_events.md`
- E420 — imported; defined in `planning/research/W2_host_state_events.md`
- E421 — imported; defined in `planning/research/W2_host_state_events.md`
- E422 — imported; defined in `planning/research/W2_host_state_events.md`
- E423 — imported; defined in `planning/research/W2_host_state_events.md`
- E424 — imported; defined in `planning/research/W2_host_state_events.md`
- E425 — imported; defined in `planning/research/W2_host_state_events.md`
- E426 — imported; defined in `planning/research/W2_host_state_events.md`
- E427 — imported; defined in `planning/research/W2_host_state_events.md`
- E428 — imported; defined in `planning/research/W2_host_state_events.md`
- E429 — imported; defined in `planning/research/W2_host_state_events.md`
- E430 — imported; defined in `planning/research/W2_host_state_events.md`
- E431 — imported; defined in `planning/research/W2_host_state_events.md`
- E432 — imported; defined in `planning/research/W2_host_state_events.md`
- E433 — imported; defined in `planning/research/W2_host_state_events.md`
- E434 — imported; defined in `planning/research/W2_host_state_events.md`
- E435 — imported; defined in `planning/research/W2_host_state_events.md`
- E436 — imported; defined in `planning/research/W2_host_state_events.md`
- E437 — imported; defined in `planning/research/W2_host_state_events.md`
- E438 — imported; defined in `planning/research/W2_host_state_events.md`
- E439 — imported; defined in `planning/research/W2_host_state_events.md`
- E440 — imported; defined in `planning/research/W2_host_state_events.md`
- E441 — imported; defined in `planning/research/W2_host_state_events.md`
- E442 — imported; defined in `planning/research/W2_host_state_events.md`
- E443 — imported; defined in `planning/research/W2_host_state_events.md`
- E444 — imported; defined in `planning/research/W2_host_state_events.md`
- E445 — imported; defined in `planning/research/W2_host_state_events.md`
- E446 — imported; defined in `planning/research/W2_host_state_events.md`
- E447 — imported; defined in `planning/research/W2_host_state_events.md`
- E448 — imported; defined in `planning/research/W2_host_state_events.md`
- E449 — imported; defined in `planning/research/W2_host_state_events.md`
- E451 — imported; defined in `planning/research/W1_barrick_mine_stats.md`
- E452 — imported; defined in `planning/research/W1_barrick_mine_stats.md`
- E453 — imported; defined in `planning/research/W1_barrick_mine_stats.md`
- E454 — imported; defined in `planning/research/W1_barrick_mine_stats.md`
- E455 — imported; defined in `planning/research/W1_barrick_mine_stats.md`
- E456 — imported; defined in `planning/research/W1_barrick_mine_stats.md`
- E457 — imported; defined in `planning/research/W1_barrick_mine_stats.md`
- E458 — imported; defined in `planning/research/W1_barrick_mine_stats.md`
- E459 — imported; defined in `planning/research/W1_barrick_mine_stats.md`
- E460 — imported; defined in `planning/research/W1_barrick_mine_stats.md`
- E461 — imported; defined in `planning/research/W1_barrick_mine_stats.md`
- E462 — imported; defined in `planning/research/W1_barrick_mine_stats.md`
- E463 — imported; defined in `planning/research/W1_barrick_mine_stats.md`
- E464 — imported; defined in `planning/research/W1_barrick_mine_stats.md`
- E465 — imported; defined in `planning/research/W1_barrick_mine_stats.md`
- E466 — imported; defined in `planning/research/W1_barrick_mine_stats.md`
- E467 — imported; defined in `planning/research/W1_barrick_mine_stats.md`
- E468 — imported; defined in `planning/research/W1_barrick_mine_stats.md`
- E469 — imported; defined in `planning/research/W1_barrick_mine_stats.md`
- E470 — imported; defined in `planning/research/W1_barrick_mine_stats.md`
- E471 — imported; defined in `planning/research/W1_barrick_mine_stats.md`
- E472 — imported; defined in `planning/research/W1_barrick_mine_stats.md`
- E473 — imported; defined in `planning/research/W1_barrick_mine_stats.md`
- E474 — imported; defined in `planning/research/W1_barrick_mine_stats.md`
- E475 — imported; defined in `planning/research/W1_barrick_mine_stats.md`
- E476 — imported; defined in `planning/research/W1_barrick_mine_stats.md`
- E477 — imported; defined in `planning/research/W1_barrick_mine_stats.md`
- E478 — imported; defined in `planning/research/W1_barrick_mine_stats.md`
- E479 — imported; defined in `planning/research/W1_barrick_mine_stats.md`
- E480 — imported; defined in `planning/research/W1_barrick_mine_stats.md`
- E481 — imported; defined in `planning/research/W1_barrick_mine_stats.md`
- E482 — imported; defined in `planning/research/W1_barrick_mine_stats.md`
- E483 — imported; defined in `planning/research/W1_barrick_mine_stats.md`
- E484 — imported; defined in `planning/research/W1_barrick_mine_stats.md`
- E485 — imported; defined in `planning/research/W1_barrick_mine_stats.md`
- E486 — imported; defined in `planning/research/W1_barrick_mine_stats.md`
- E487 — imported; defined in `planning/research/W1_barrick_mine_stats.md`
- E488 — imported; defined in `planning/research/W1_barrick_mine_stats.md`
- E489 — imported; defined in `planning/research/W1_barrick_mine_stats.md`
- E490 — imported; defined in `planning/research/W1_barrick_mine_stats.md`
- E491 — imported; defined in `planning/research/W1_barrick_mine_stats.md`
- E492 — imported; defined in `planning/research/W1_barrick_mine_stats.md`
- E493 — imported; defined in `planning/research/W1_barrick_mine_stats.md`
- E494 — imported; defined in `planning/research/W1_barrick_mine_stats.md`
- E495 — imported; defined in `planning/research/W1_barrick_mine_stats.md`
- E496 — imported; defined in `planning/research/W1_barrick_mine_stats.md`
- E497 — imported; defined in `planning/research/W1_barrick_mine_stats.md`
- E498 — imported; defined in `planning/research/W1_barrick_mine_stats.md`
- E499 — imported; defined in `planning/research/W1_barrick_mine_stats.md`
- E501 — imported; defined in `planning/research/W3_data_residency.md`
- E502 — imported; defined in `planning/research/W3_data_residency.md`
- E503 — imported; defined in `planning/research/W3_data_residency.md`
- E504 — imported; defined in `planning/research/W3_data_residency.md`
- E505 — imported; defined in `planning/research/W3_data_residency.md`
- E506 — imported; defined in `planning/research/W3_data_residency.md`
- E507 — imported; defined in `planning/research/W3_data_residency.md`
- E508 — imported; defined in `planning/research/W3_data_residency.md`
- E509 — imported; defined in `planning/research/W3_data_residency.md`
- E510 — imported; defined in `planning/research/W3_data_residency.md`
- E511 — imported; defined in `planning/research/W3_data_residency.md`
- E512 — imported; defined in `planning/research/W3_data_residency.md`
- E513 — imported; defined in `planning/research/W3_data_residency.md`
- E514 — imported; defined in `planning/research/W3_data_residency.md`
- E515 — imported; defined in `planning/research/W3_data_residency.md`
- E516 — imported; defined in `planning/research/W3_data_residency.md`
- E517 — imported; defined in `planning/research/W3_data_residency.md`
- E518 — imported; defined in `planning/research/W3_data_residency.md`
- E519 — imported; defined in `planning/research/W3_data_residency.md`
- E520 — imported; defined in `planning/research/W3_data_residency.md`
- E521 — imported; defined in `planning/research/W3_data_residency.md`
- E522 — imported; defined in `planning/research/W3_data_residency.md`
- E523 — imported; defined in `planning/research/W3_data_residency.md`
- E524 — imported; defined in `planning/research/W3_data_residency.md`
- E525 — imported; defined in `planning/research/W3_data_residency.md`
- E526 — imported; defined in `planning/research/W3_data_residency.md`
- E527 — imported; defined in `planning/research/W3_data_residency.md`
- E528 — imported; defined in `planning/research/W3_data_residency.md`
- E529 — imported; defined in `planning/research/W3_data_residency.md`
- E530 — imported; defined in `planning/research/W3_data_residency.md`
- E531 — imported; defined in `planning/research/W3_data_residency.md`
- E532 — imported; defined in `planning/research/W3_data_residency.md`
- E533 — imported; defined in `planning/research/W3_data_residency.md`
- E534 — imported; defined in `planning/research/W3_data_residency.md`
- E535 — imported; defined in `planning/research/W3_data_residency.md`
- E536 — imported; defined in `planning/research/W3_data_residency.md`
- E537 — imported; defined in `planning/research/W3_data_residency.md`
- E538 — imported; defined in `planning/research/W3_data_residency.md`
- E539 — imported; defined in `planning/research/W3_data_residency.md`
- E540 — imported; defined in `planning/research/W3_data_residency.md`
- E541 — imported; defined in `planning/research/W3_data_residency.md`
- E542 — imported; defined in `planning/research/W3_data_residency.md`
- E543 — imported; defined in `planning/research/W3_data_residency.md`
- E544 — imported; defined in `planning/research/W3_data_residency.md`
- E545 — imported; defined in `planning/research/W3_data_residency.md`
- E546 — imported; defined in `planning/research/W3_data_residency.md`
- E547 — imported; defined in `planning/research/W3_data_residency.md`
- E548 — imported; defined in `planning/research/W3_data_residency.md`
- E549 — imported; defined in `planning/research/W3_data_residency.md`
- E551 — imported; defined in `planning/research/W4_m2_site_and_data_access.md`
- E552 — imported; defined in `planning/research/W4_m2_site_and_data_access.md`
- E553 — imported; defined in `planning/research/W4_m2_site_and_data_access.md`
- E554 — imported; defined in `planning/research/W4_m2_site_and_data_access.md`
- E555 — imported; defined in `planning/research/W4_m2_site_and_data_access.md`
- E556 — imported; defined in `planning/research/W4_m2_site_and_data_access.md`
- E557 — imported; defined in `planning/research/W4_m2_site_and_data_access.md`
- E558 — imported; defined in `planning/research/W4_m2_site_and_data_access.md`
- E559 — imported; defined in `planning/research/W4_m2_site_and_data_access.md`
- E560 — imported; defined in `planning/research/W4_m2_site_and_data_access.md`
- E561 — imported; defined in `planning/research/W4_m2_site_and_data_access.md`
- E562 — imported; defined in `planning/research/W4_m2_site_and_data_access.md`
- E563 — imported; defined in `planning/research/W4_m2_site_and_data_access.md`
- E564 — imported; defined in `planning/research/W4_m2_site_and_data_access.md`
- E565 — imported; defined in `planning/research/W4_m2_site_and_data_access.md`
- E566 — imported; defined in `planning/research/W4_m2_site_and_data_access.md`
- E567 — imported; defined in `planning/research/W4_m2_site_and_data_access.md`
- E568 — imported; defined in `planning/research/W4_m2_site_and_data_access.md`
- E569 — imported; defined in `planning/research/W4_m2_site_and_data_access.md`
- E570 — imported; defined in `planning/research/W4_m2_site_and_data_access.md`
- E571 — imported; defined in `planning/research/W4_m2_site_and_data_access.md`
- E572 — imported; defined in `planning/research/W4_m2_site_and_data_access.md`
- E573 — imported; defined in `planning/research/W4_m2_site_and_data_access.md`
- E574 — imported; defined in `planning/research/W4_m2_site_and_data_access.md`
- E575 — imported; defined in `planning/research/W4_m2_site_and_data_access.md`
- E576 — imported; defined in `planning/research/W4_m2_site_and_data_access.md`
- E577 — imported; defined in `planning/research/W4_m2_site_and_data_access.md`
- E578 — imported; defined in `planning/research/W4_m2_site_and_data_access.md`
- E579 — imported; defined in `planning/research/W4_m2_site_and_data_access.md`
- E580 — imported; defined in `planning/research/W4_m2_site_and_data_access.md`
- E581 — imported; defined in `planning/research/W4_m2_site_and_data_access.md`
- E582 — imported; defined in `planning/research/W4_m2_site_and_data_access.md`
- E583 — imported; defined in `planning/research/W4_m2_site_and_data_access.md`
- E584 — imported; defined in `planning/research/W4_m2_site_and_data_access.md`
- E585 — imported; defined in `planning/research/W4_m2_site_and_data_access.md`
- E586 — imported; defined in `planning/research/W4_m2_site_and_data_access.md`
- E587 — imported; defined in `planning/research/W4_m2_site_and_data_access.md`
- E588 — imported; defined in `planning/research/W4_m2_site_and_data_access.md`
- E589 — imported; defined in `planning/research/W4_m2_site_and_data_access.md`
- E590 — imported; defined in `planning/research/W4_m2_site_and_data_access.md`
- E591 — imported; defined in `planning/research/W4_m2_site_and_data_access.md`
- E592 — imported; defined in `planning/research/W4_m2_site_and_data_access.md`
- E593 — imported; defined in `planning/research/W4_m2_site_and_data_access.md`
- E594 — imported; defined in `planning/research/W4_m2_site_and_data_access.md`
- E595 — imported; defined in `planning/research/W4_m2_site_and_data_access.md`
- E596 — imported; defined in `planning/research/W4_m2_site_and_data_access.md`
- E597 — imported; defined in `planning/research/W4_m2_site_and_data_access.md`
- E598 — imported; defined in `planning/research/W4_m2_site_and_data_access.md`
- E599 — imported; defined in `planning/research/W4_m2_site_and_data_access.md`
- E601 — imported; defined in `planning/research/W5_figure_verification.md`
- E602 — imported; defined in `planning/research/W5_figure_verification.md`
- E603 — imported; defined in `planning/research/W5_figure_verification.md`
- E604 — imported; defined in `planning/research/W5_figure_verification.md`
- E605 — imported; defined in `planning/research/W5_figure_verification.md`
- E606 — imported; defined in `planning/research/W5_figure_verification.md`
- E607 — imported; defined in `planning/research/W5_figure_verification.md`
- E608 — imported; defined in `planning/research/W5_figure_verification.md`
- E609 — imported; defined in `planning/research/W5_figure_verification.md`
- E610 — imported; defined in `planning/research/W5_figure_verification.md`
- E611 — imported; defined in `planning/research/W5_figure_verification.md`
- E612 — imported; defined in `planning/research/W5_figure_verification.md`
- E613 — imported; defined in `planning/research/W5_figure_verification.md`
- E614 — imported; defined in `planning/research/W5_figure_verification.md`
- E615 — imported; defined in `planning/research/W5_figure_verification.md`
- E616 — imported; defined in `planning/research/W5_figure_verification.md`
- E617 — imported; defined in `planning/research/W5_figure_verification.md`
- E618 — imported; defined in `planning/research/W5_figure_verification.md`
- E619 — imported; defined in `planning/research/W5_figure_verification.md`
- E620 — imported; defined in `planning/research/W5_figure_verification.md`, where it is WITHDRAWN (no design use)
- E621 — imported; defined in `planning/research/W5_figure_verification.md`
- E622 — imported; defined in `planning/research/W5_figure_verification.md`
- E623 — imported; defined in `planning/research/W5_figure_verification.md`
- E624 — imported; defined in `planning/research/W5_figure_verification.md`
- E625 — imported; defined in `planning/research/W5_figure_verification.md`
- E626 — imported; defined in `planning/research/W5_figure_verification.md`
- E627 — imported; defined in `planning/research/W5_figure_verification.md`
- E628 — imported; defined in `planning/research/W5_figure_verification.md`
- E629 — imported; defined in `planning/research/W5_figure_verification.md`
- E630 — imported; defined in `planning/research/W5_figure_verification.md`
- E631 — imported; defined in `planning/research/W5_figure_verification.md`
- E632 — imported; defined in `planning/research/W5_figure_verification.md`
- E633 — imported; defined in `planning/research/W5_figure_verification.md`
- E634 — imported; defined in `planning/research/W5_figure_verification.md`
- E635 — imported; defined in `planning/research/W5_figure_verification.md`
- E636 — imported; defined in `planning/research/W5_figure_verification.md`
- E637 — imported; defined in `planning/research/W5_figure_verification.md`
- E638 — imported; defined in `planning/research/W5_figure_verification.md`
- E639 — imported; defined in `planning/research/W5_figure_verification.md`
- E640 — imported; defined in `planning/research/W5_figure_verification.md`
- E641 — imported; defined in `planning/research/W5_figure_verification.md`
- E642 — imported; defined in `planning/research/W5_figure_verification.md`
- E643 — imported; defined in `planning/research/W5_figure_verification.md`
- E644 — imported; defined in `planning/research/W5_figure_verification.md`
- E645 — imported; defined in `planning/research/W5_figure_verification.md`
- E646 — imported; defined in `planning/research/W5_figure_verification.md`
- E647 — imported; defined in `planning/research/W5_figure_verification.md`
- E648 — imported; defined in `planning/research/W5_figure_verification.md`
- E649 — imported; defined in `planning/research/W5_figure_verification.md`
- E651 — imported; defined in `planning/mor/MOR_CASE.md`
- E652 — imported; defined in `planning/mor/MOR_CASE.md`
- E653 — imported; defined in `planning/mor/MOR_CASE.md`
- E654 — imported; defined in `planning/mor/MOR_CASE.md`
- E655 — imported; defined in `planning/mor/MOR_CASE.md`
- E656 — imported; defined in `planning/mor/MOR_CASE.md`
- E657 — imported; defined in `planning/mor/MOR_CASE.md`
- E658 — imported; defined in `planning/mor/MOR_CASE.md`
- E659 — imported; defined in `planning/mor/MOR_CASE.md`
- E660 — imported; defined in `planning/mor/MOR_CASE.md`
- E661 — imported; defined in `planning/mor/MOR_CASE.md`
- E662 — imported; defined in `planning/mor/MOR_CASE.md`
- E663 — imported; defined in `planning/mor/MOR_CASE.md`
- E664 — imported; defined in `planning/mor/MOR_CASE.md`
- E665 — imported; defined in `planning/mor/MOR_CASE.md`
- E666 — imported; defined in `planning/mor/MOR_CASE.md`
- E667 — imported; defined in `planning/mor/MOR_CASE.md`
- E668 — imported; defined in `planning/mor/MOR_CASE.md`
- E669 — imported; defined in `planning/mor/MOR_CASE.md`
- E670 — imported; defined in `planning/mor/MOR_CASE.md`
- E671 — imported; defined in `planning/mor/MOR_CASE.md`
- E672 — imported; defined in `planning/mor/MOR_CASE.md`

Evidence discovered by the generation passes (indexed by the normalizer):

- E700 — Code: `crates/fmine-io/src/zip_inflate.rs` lines 405–472 (`build_canonical_barrick_xlsx_bytes`, `write_stored_zip`; filler loop lines 451–459; doc comment lines 408–409) — as cited, builds the in-code stand-in workbook: explicit cells in sheet "Gold AISC" (rows 14, 191–204, 206, 208, 218, 228, 230; e.g. L218 = 1638, N218 = 1484, L228 = 3255, N228 = 3973, J208 = 2582, A230 `#REF!`), 3 cells in "Copper AISC", a hidden "YTD Q3 2025" sheet, and a loop writing r×10+c into rows 231–350, columns A–T; the doc comment names remote workers without the gitignored corpus as the reason [P00, P01, P04, P05; extends E678]
- E701 — Code: `crates/fmine-public/src/lib.rs` lines 36–146 (`BARRICK_18_WORKBOOKS`) and lines 160–170 — compiled-in list of 18 names `barrick-aisc-2024-q1.xlsx` … `barrick-mine-stats-2026-q2.xlsx` (9 periods × 2 kinds, no 2026 Q1, no sha256, a `const`); lines 165–166 label each workbook cell `EvidenceClass::Exact` + `KnownKnown` [P00, P01, P02, P05]
- E702 — Code: `crates/fmine-io/src/xlsx.rs` line 95 (`Sheet::find_rows_by_label`) and lines 115–145 (`Workbook::from_file`; fallback at lines 121–137 for an unreadable path containing "corpus_of_input_docs"; sha256 over the bytes at line 142), and `crates/fmine-io/src/lib.rs` lines 160–187 (test `test_parse_real_barrick_xlsx_workbooks`, asserting "Gold AISC"/"Copper AISC" sheets, a `#REF!` and > 2,000 cells) [P00, P01]
- E703 — Execution: P01 stdlib Python port of the M0 loader path, `/private/tmp/claude-501/-Users-jemanuel-projects-franken-mining/2ba72fbb-c45d-4fd8-93cf-c08cb26d7a18/scratchpad/r7_repro/P01-G11-M0-REALITY/scripts/emulate_loader.py` on workspaces `ws_A_real16`, `ws_B_real18_surrogate`, `ws_C_empty`, `ws_D_real_minus_q4aisc` → `emulate_standin.txt`, `emulate_ws_*.txt` (stand-in per workbook 2 sheets, 2,444 cells, 2,441 numeric, 2 placeholders, 2 quarantined; ws_A 44,420 cells, 1,100 quarantined, stand-ins for 2024 Q1/Q2 AISC; ws_D 45,700 cells, status ok), plus `quarantine_and_sha.py` (stand-in sha256 `c0dd3134…bf59f`) [P01]
- E704 — Execution: `/private/tmp/claude-501/-Users-jemanuel-projects-franken-mining/2ba72fbb-c45d-4fd8-93cf-c08cb26d7a18/scratchpad/r7_repro/P05-F4-DEPENDENCIES/scripts/count_cells.py` — Q2 2026 Mine Stats 7 sheets / 2,297 numeric cells; Q1 2025 AISC 3 / 1,146; Q4 2024 Mine Stats 7 / 2,308 [P05]
- E705 — Workbook: `corpus_of_input_docs/downloaded/mine_stats/Barrick_Q4_2025_Mine_Stats.xlsx` sheet YTD row 22 (Total Gold) — C22 3,255, D22 3,318, G22 1,637 (FY2025 produced koz, sold koz, AISC $/oz); J22 3,911, K22 3,798, N22 1,484 (FY2024) [P00, P01, P03]
- E706 — Workbook: `corpus_of_input_docs/downloaded/mine_stats/Barrick_Q4_2024_Mine_Stats.xlsx` YTD!C23 = 3,911 and D23 = 3,798 (FY2024 gold produced, sold) [P03]
- E707 — PDF extraction: `corpus_of_input_docs/downloaded/mine_stats/Barrick_Q4_2025_MD-A.pdf` via `pdftotext -layout` (copies `/private/tmp/claude-501/-Users-jemanuel-projects-franken-mining/2ba72fbb-c45d-4fd8-93cf-c08cb26d7a18/scratchpad/r7_repro/P01-G11-M0-REALITY/mdna_txt/Barrick_Q4_2025_MD-A.txt`, `/private/tmp/claude-501/-Users-jemanuel-projects-franken-mining/2ba72fbb-c45d-4fd8-93cf-c08cb26d7a18/scratchpad/r7_repro/P04-I4-READERS/q4_2025_mda.txt`) — summary table (page 6, lines 352–358): FY2025 gold produced 3,255 koz, sold 3,318 koz, AISC $1,637/oz; FY2024 3,911 / 3,798 / $1,484; non-GAAP reconciliation (page 47, lines 4060–4067): gold AISC $5,428M FY2025, $5,636M FY2024 [P01, P03, P04]
- E708 — PDF extraction: `/private/tmp/claude-501/-Users-jemanuel-projects-franken-mining/2ba72fbb-c45d-4fd8-93cf-c08cb26d7a18/scratchpad/r7_repro/P01-G11-M0-REALITY/mdna_txt/Barrick_Q4_2024_MD-A.txt` line 357 — FY2024 gold sold 3,798 koz; per P01, 3,973 appears in the ten MD&As only as an ore-tonnes figure [P01]
- E709 — Execution: `/private/tmp/claude-501/-Users-jemanuel-projects-franken-mining/2ba72fbb-c45d-4fd8-93cf-c08cb26d7a18/scratchpad/r7_repro/P01-G11-M0-REALITY/scripts/real_bridge.py` and `find_values.py` (±0.5 searches over Q4 2025 AISC, Q4 2024 AISC and Mine Stats workbooks and ten MD&A texts) — only 1,484 and 3,255 match payload figures; derived corporate layer ≈ +$229M (FY2025), +$140M (FY2024), ±2 $M [P01]
- E710 — Workbook: `corpus_of_input_docs/downloaded/mine_stats/Barrick_Q4_2025_AISC.xlsx` — sheets "Q4 2025", "Q3 2025", "Q4 2024" (hidden), "2025", "2024"; site-by-column layout rows 5–21; sheets "2025"/"2024" carry site columns C–P (no Total Gold column), row 17 attributable AISC $M, row 18 ounces sold [P00, P01, P04]
- E711 — Execution: `/private/tmp/claude-501/-Users-jemanuel-projects-franken-mining/2ba72fbb-c45d-4fd8-93cf-c08cb26d7a18/scratchpad/r7_repro/P03-B11-QUANT/calc.py` → `calc_out.txt` — card and figure recomputations (GB-03, GB-08, GB-28, GB-35–GB-38, §10.4 first passage, §34.4, Lumwana, corporate layer) [P03]
- E712 — Plan text: plan (`COMPREHENSIVE_PLAN_FOR_THE_DESIGN_OF_FRANKENMINING.md` at HEAD `68e7091`, E682) §21.6 lines 4430–4472 (FY2025 AISC 1,637 at 4430; L_corp/L_JV layers 4433–4439; per attributable ounce sold 4444; LMDI factor set 4465–4472) and Appendix I.2 item 3 line 8676 [P03, P04]
- E713 — Plan text: plan (`COMPREHENSIVE_PLAN_FOR_THE_DESIGN_OF_FRANKENMINING.md` at HEAD `68e7091`, E682) §21.6 lines 4519–4524 (definition of "reproduced"; "84/89 (94.4%) … so the 95% bar"), §37.3 lines 7401–7402, Appendix I.2 item 2 lines 8673–8675 [P00, P03]
- E714 — Code: `crates/fmine-public/src/lib.rs` lines 556–664 — North Mara `DiscrepancyEntry` text ("72 koz sold vs 54 koz produced", anchor "Gold AISC!J208"; lines 556–570), `state_root` over workbook count, cell count and discrepancy tags (lines 572–579), and tests (gate test at line 655; assertions > 35,000 cells, > 20,000 numeric, ≥ 15 discrepancies, constants 5882.0, −49.0, 5332.0, 18 at lines 631–632 and 659); `crates/fmine-cli/src/lib.rs` line 297 (`test_cli_assure_public_barrick_end_to_end`) [P00, P01]
- E715 — Code: `crates/fmine-metrics/src/lmdi.rs` lines 70 and 75–125 (LMDI engine with log-mean weights; `.ln()` calls at lines 70 and 101–112) and lines 131–169 (test; comment at line 133: "Real Barrick corpus case E137 (barrick-mine-stats-2025-q4.xlsx & barrick-aisc-2024-q2.xlsx)") [P00, P01]
- E716 — Workbook: `corpus_of_input_docs/downloaded/mine_stats/Barrick_Q1_2024_Mine_Stats.xlsx` QTD row 20 (North Mara C20 46, D20 46, G20 1,753) and `corpus_of_input_docs/downloaded/mine_stats/Barrick_Q2_2024_Mine_Stats.xlsx` QTD row 20 (C20 54, D20 50, G20 1,491) [P00, P01, P03, P04]
- E717 — Workbook: `corpus_of_input_docs/downloaded/mine_stats/Barrick_Q2_2026_Mine_Stats.xlsx` QTD row 19 (A19 "North Mara (84%)", G19 2,577, N19 1,292; Q2 2025 produced 62, sold 50) and sheet Minestats3 columns H/I (tonnes 698→688, grade 3.16→1.80, recovery 0.88→0.85, produced 62→35, sold 50→37) [P00, P01, P03, P04]
- E718 — Research note: `planning/research/W1_barrick_mine_stats.md` lines 65–76 (18 workbook names with sha256), line 95 (Barrick naming), line 108 ("The IR feed lists no AISC workbook for Q1 2024 or Q2 2024"), line 112 (`w1/manifest.tsv` with URLs and timestamps in a scratch folder) [P00, P01, P05]
- E719 — Plan text: plan (`COMPREHENSIVE_PLAN_FOR_THE_DESIGN_OF_FRANKENMINING.md` at HEAD `68e7091`, E682) §39 WP-03 rows lines 7690–7727 (BD-055 7698; BD-056 7699; BD-062–BD-064 7705–7707; BD-067–BD-069 7710–7712; BD-072 7715; BD-075; BD-076 7719; BD-271 7722; BD-272–BD-275) [all passes]
- E720 — Code: `crates/fmine-metrics/src/lib.rs` lines 86–107 (price decks `barrick_reserve_2024` $1,300/oz Au, $3.00/lb Cu; `barrick_reserve_2025` $1,400/oz Au, $4.00/lb Cu), lines 133–167 (`ReserveReplacementBridge`; doc comment lines 148–149 "Reko Diq +13.0 Moz + Lumwana +1.7 Moz = 12.28 Moz"), test lines 237–239 (`compute(3.91, 7.429, 12.277)`), line 122 `compute_geo(gold_koz: f64, copper_mlb: f64, deck: &PriceDeck)` [P00, P01, P04]
- E721 — Command: searches over planning and repository prose — 7.429/12.277/7.43 in `planning/` (no hits); "Gold AISC" in plan, C1, W1 (prose only); "quiet period", "51-201", "Reg FD", "tipping", "selective disclosure" in plan, `planning/` (outside r7), README, AGENTS (no hits); "terms of use", "copyright", "redistribut" in plan, AGENTS, README (no hits); "RFC 3161", "OpenTimestamps", "TSA", "external timestamp" in the plan (no hits beyond line 7711) [P01, P04, P05]
- E722 — Plan text: plan (`COMPREHENSIVE_PLAN_FOR_THE_DESIGN_OF_FRANKENMINING.md` at HEAD `68e7091`, E682) §21.6 lines 4481–4486 (line 4484 "504% in gold-equivalent ounces (87 Moz ÷ 17.2 Moz)") and OPEN-BULL-4 lines 4639–4645 ("506%, 499–514% under rounding" at 4642) [P00, P01, P03, P04]
- E723 — Filesystem: `corpus_of_input_docs/downloaded/mine_stats/` holds 18 `.xlsx` (10 Mine Stats Q1 2024–Q2 2026, 8 AISC Q3 2024–Q2 2026; sha256 as in W1 lines 65–76); `corpus_of_input_docs/Barrick_Q1_2026_Mine_Stats.xlsx` and `Barrick_Q2_2026_Mine_Stats.xlsx` reported byte-identical to the Q1/Q2 2026 files there (`/private/tmp/claude-501/-Users-jemanuel-projects-franken-mining/2ba72fbb-c45d-4fd8-93cf-c08cb26d7a18/scratchpad/r7_repro/P01-G11-M0-REALITY/mapping.tsv`) [P00, P01]
- E724 — Plan text: plan (`COMPREHENSIVE_PLAN_FOR_THE_DESIGN_OF_FRANKENMINING.md` at HEAD `68e7091`, E682) §37.3 M0 lines 7378–7405 (window 7378; artifact 7379–7396; ten quarters 7381; per-number citation 7382; Q3 2026 "must pass on" 7384–7386; FY2026 forecasts 7393–7394; kill and "test fixtures" fallback 7403–7404; crates line 7405) [all passes; extends E685]
- E725 — Code: `crates/fmine-public/src/lib.rs` lines 301–375 — AISC bridge reads sheet "Gold AISC" of `barrick-aisc-2025-q4.xlsx` (line 301) at fixed coordinates; `SITE_ROWS` lines 306–321 (shared rows); line 343 `site_sales_sum = cons_sales - 17.0`; `number_or_zero` lines 369–375 [P00, P01, P05]
- E726 — Command: searches over `crates/` — `grep -rn -iE "scorer|NOT_PUBLISHED|reproduc|site.quarter" crates/*/src` and grep for "reproduced"/"NOT_PUBLISHED" (no scorer found); `grep -rn rules_version crates/` and `grep -rn -E 'reserve_disclosure_claim|no_claim|next_actions|regulatory_compliance_claim' crates` (no match outside CONTRACT.md); grep for "Determinism: BitwisePortable" (fmine-core only) and std transcendental calls (14 non-test calls in core/units/calib/numerics/metrics) [P00, P01, P04]
- E727 — Docs: `AGENTS.md` lines 15–60 (RULE 0.1; prohibited "mocks or fixtures presented as live proof", "hard-coded success paths" at lines 44–51), line 233 ("refuse, never clamp or plug"), line 236 ("A synthetic result is labelled synthetic") [P00, P01]
- E728 — Execution: sheet listings of the local Barrick workbooks — `/private/tmp/claude-501/-Users-jemanuel-projects-franken-mining/2ba72fbb-c45d-4fd8-93cf-c08cb26d7a18/scratchpad/r7_repro/P00-BASELINE/xl.py` (20 files) and `/private/tmp/claude-501/-Users-jemanuel-projects-franken-mining/2ba72fbb-c45d-4fd8-93cf-c08cb26d7a18/scratchpad/r7_repro/P01-G11-M0-REALITY/scripts/xlsx_sheets.py` (18 files in `corpus_of_input_docs/downloaded/mine_stats/`): no workbook has a sheet named "Gold AISC" [P00, P01]
- E729 — Config: `.gitignore` line 7 ("Keep Cargo.lock tracked") and lines 27–35 (`/corpus_of_input_docs/`, `*.xlsx`) [P00, P05]
- E730 — Docs: `crates/fmine-public/CONTRACT.md` §7 — states "Missing anchor labels or corrupted workbooks fail closed with `IoError`" [P00]
- E731 — Beads: `.beads/issues.jsonl` fields `status`, `close_reason`, `closed_at` — 87 closed issues share the reason "Implemented and verified by cargo test --workspace and xtask check-all (GATE-LAYERS-00, GATE-BULL-01)", closed 17:31:36.158Z–17:31:47.821Z (P00 inline `python3 -I`; `/private/tmp/claude-501/-Users-jemanuel-projects-franken-mining/2ba72fbb-c45d-4fd8-93cf-c08cb26d7a18/scratchpad/r7_repro/P01-G11-M0-REALITY/scripts/beads_closed.py`) [P00, P01, P02]
- E732 — Docs: `docs/delivery/` (contains `INT-AS-001.md`, `INT-NUM-001.md`, `README.md`) — INT-AS-001 Verdict PASS (item 2 confines franken-evidence/decision to fmine-decide); INT-NUM-001 Verdict PASS (item 1 "`fsci-* =0.4.0` is consumed with `default-features = false`"; item 2 bit-identical digests across x86_64-linux, aarch64-darwin, wasm32; fmine-metrics listed BitwisePortable); `docs/adr/` contains only `ADR-RNG-01.md` [P00, P01, P02, P05]
- E733 — Plan text: plan (`COMPREHENSIVE_PLAN_FOR_THE_DESIGN_OF_FRANKENMINING.md` at HEAD `68e7091`, E682) §39 lines 7589–7611 ("blocks" precedence 7596; critical path 7601; site selection waits for the M0 finance review 7603; ids stable when moved 7605; closure rule 7611) [P00, P02, P05]
- E734 — Execution: `/private/tmp/claude-501/-Users-jemanuel-projects-franken-mining/2ba72fbb-c45d-4fd8-93cf-c08cb26d7a18/scratchpad/r7_repro/P02-A1-CONSISTENCY/beads_parse.py` and `closure.py` (inputs `beads.json`, `gate_beads.json`) — §39 tables vs `.beads/issues.jsonl` (320 beads, 533 edges), closed beads with open blockers, dependency closures of M0 beads; P00 matched each closed bead's §39 dependencies against the closed set [P00, P02]
- E735 — Plan text: plan (`COMPREHENSIVE_PLAN_FOR_THE_DESIGN_OF_FRANKENMINING.md` at HEAD `68e7091`, E682) §39 rows lines 7619–7675 (BD-001 7619 incl. `[patch.crates-io]`; BD-002 7620; BD-005 7623; BD-014 7632; BD-257 7634; BD-308 7635 "or the FD-row fallback taken"; BD-038 7671; BD-039 7672; BD-042 7675) [P00, P01, P02, P05]
- E736 — Code: `crates/fmine-cli/src/lib.rs` lines 31–39 (`Envelope`: schema_version, verb, status, exit_code, anchor_id, payload_json, refusal), 141–160 (verb dispatch `match`), 176–229 (`assure` branch: unsupported issuer → FMINE-REF-401 at line 180; payload format string line 189; `MissingAnchorLabel` → FMINE-REF-502, exit 8), 247–262 (`claim show`; fixed `"Exact"`/`"KnownKnown"` for any `clm_` id at line 258; malformed ClaimId → 401 at line 252), 269 (robot-docs verb list literal) [P00, P01, P02, P04]
- E737 — Code: `crates/fmine-decide/src/lib.rs` line 73 (`compute_evsi`, closed form, no seeds) and lines 95–146 (`DecisionCard` lines 98–110: recommended and runner-up actions); `crates/fmine-decide/CONTRACT.md` line 14 (INV-002 via `authoritative`, `may_actuate`, `is_safe_certification`, `is_signing_basis`) [P00, P02, P03]
- E738 — Code: `crates/xtask/src/lib.rs` lines 300–352 (`check_deps`, direct `[dependencies]` names), 424–486 (`check_lifts`, headers), 575–590 (check-claims, five parameter names), 627–640 (check-det, files declaring "Determinism: BitwisePortable", `f64::ln(` syntax), 704–713 (LOC = non-blank `.rs` lines), 844–886 (tests); `crates/xtask/src/main.rs` line 58 (check-all runs GATE-LAYERS-00) [P00, P01, P02, P03, P05]
- E739 — Plan text: plan (`COMPREHENSIVE_PLAN_FOR_THE_DESIGN_OF_FRANKENMINING.md` at HEAD `68e7091`, E682) §39 rows lines 7765–7787 (BD-107 7765; BD-111 7775; BD-113 7777; BD-115 7779; BD-123 7787), BD-147 line 7824, BD-179 line 7877 [P01, P02]
- E740 — Plan text: plan (`COMPREHENSIVE_PLAN_FOR_THE_DESIGN_OF_FRANKENMINING.md` at HEAD `68e7091`, E682) §22 lines 4646–4787 (§22.2 decision-card requirements: alternatives incl. "do nothing", sign-offs, `as_known_at`, registered forecasts; G-HELM-2; GATE-HELM-02 line 4766) [P00, P03]
- E741 — Docs: `AGENTS.md` lines 530–543 ("Current Repository State"; lines 537, 542: nothing implemented in the §0.2 sense) and `README.md` lines 50 and 65 ("Nothing runs end to end yet") [P00, P01, P02]
- E742 — Plan text: plan (`COMPREHENSIVE_PLAN_FOR_THE_DESIGN_OF_FRANKENMINING.md` at HEAD `68e7091`, E682) line 14 (§0 status "PLAN. No FrankenMining code exists.") and §0.2 lines 39–48 ("implemented": item 2 CONTRACT.md sections line 44; item 4 no naked numbers lines 47–48; partial work "MUST be described as partial") [P00, P01, P02]
- E743 — Config: root `Cargo.toml` lines 42–43 (`sole_async_runtime = "asupersync"`, `asupersync_pin = "=0.5.0"` metadata), no `[patch.crates-io]`; `crates/*/Cargo.toml` path dependencies on `fmine-*` only; untracked `Cargo.lock` with 16 workspace-local packages [P00, P01, P02, P05]
- E744 — Git: `git status -sb` (`main...origin/main [ahead 1]`, `?? Cargo.lock`) and `git ls-tree -r origin/main` (37 files; no `crates/`, `.github/`, `deny.toml`, `franken-stack.lock`, `rust-toolchain.toml`) [P05]
- E745 — Plan text: plan (`COMPREHENSIVE_PLAN_FOR_THE_DESIGN_OF_FRANKENMINING.md` at HEAD `68e7091`, E682) §27 lines 6015–6018 (INT-* admission before first production use; FD fallback "keeps the product shippable and honest"), §27.1 lines 6050–6051 (lift header contract), line 6113 (frankenpandas pin condition), §27.5 line 6189 (one-package-id "a CI gate"), §28.5 L-5 lines 6389–6403, §28.6 line 6412 (`[patch.crates-io]`), §31.4 lines 6819–6847 (6831 QUIC/TLS), §31.5 lines 6849–6858 [P05]
- E746 — Plan text: plan (`COMPREHENSIVE_PLAN_FOR_THE_DESIGN_OF_FRANKENMINING.md` at HEAD `68e7091`, E682) §37.4 lines 7549–7554 (no executive deliverable blocked by a sibling; M0–M2 consumed column) and §38 lines 7567–7580 (INT-AS-001 at "Commit 1" 7567; WP-04 7571; WP-07 7574; WP-10 7577; WP-13 at M3 7580) [P02, P03, P05]
- E747 — Plan text: plan (`COMPREHENSIVE_PLAN_FOR_THE_DESIGN_OF_FRANKENMINING.md` at HEAD `68e7091`, E682) INV-002 line 535, INV-009 line 542, §30.1 lines 6556–6588 (envelope fields 6556–6572; no naked numbers in `data` 6570; exit codes 6576–6588, usage exit 2 at 6579, exit 6 conflict at 6583) [P01, P02]
- E748 — Plan text: plan (`COMPREHENSIVE_PLAN_FOR_THE_DESIGN_OF_FRANKENMINING.md` at HEAD `68e7091`, E682) §21.8 lines 4572–4592 (G-BULL-2 4572; G-BULL-6 4576; GATE-BULL-01 row 4590; GATE-BULL-02 at M1 4591; GATE-BULL-03 4592) [P00, P02, P03, P04]
- E749 — Plan text: plan (`COMPREHENSIVE_PLAN_FOR_THE_DESIGN_OF_FRANKENMINING.md` at HEAD `68e7091`, E682) §3 lines 306–324 (reader questions: CFO line 311 incl. "controllable"; QP 314; audit committee 320; counsel 321) and §37.1 lines 7336–7337 (wedge justified by measurement integrity behind published metrics) [P04]
- E750 — Execution: `/private/tmp/claude-501/-Users-jemanuel-projects-franken-mining/2ba72fbb-c45d-4fd8-93cf-c08cb26d7a18/scratchpad/r7_repro/P03-B11-QUANT/` `t3_scan.py`, `t3_scan_round.py`, `t8_scan.py`, `ceiling.py`, `ceiling_layers_excluded.py` → `t3_out.txt`, `t3_round_out.txt`, `t8_out.txt`, `ceiling_out.txt`, `ceiling_layers_excluded_out.txt` — identity census over the ten Mine Stats workbooks (denominator 18,860 cells) [P03]
- E751 — Plan text: plan (`COMPREHENSIVE_PLAN_FOR_THE_DESIGN_OF_FRANKENMINING.md` at HEAD `68e7091`, E682) §21.6 lines 4495–4496 (FY2026 per-site forecasts frozen and hashed before the February 2027 release) [P00, P03, P05]
- E752 — Plan text: plan (`COMPREHENSIVE_PLAN_FOR_THE_DESIGN_OF_FRANKENMINING.md` at HEAD `68e7091`, E682) §37.3 M1 lines 7407–7417 (window 7407; scoring 7412; acceptance 7413–7417 incl. GATE-SYNC-01 at 7417) [P00, P02, P03, P05]
- E753 — Plan text: plan (`COMPREHENSIVE_PLAN_FOR_THE_DESIGN_OF_FRANKENMINING.md` at HEAD `68e7091`, E682) §10.1 lines 867–886 (registration stores horizon and resolution source, lines 875–877; `author`) and §10.2 lines 886–900 (CRPS/coverage line 892; `PairwiseRace` line 897) [P04, P05]
- E754 — Plan text: plan (`COMPREHENSIVE_PLAN_FOR_THE_DESIGN_OF_FRANKENMINING.md` at HEAD `68e7091`, E682) §2 lines 294–297 (BET-02 incl. "four scored quarters" line 294; BET-04/05) [P00, P04]
- E755 — Plan text: plan (`COMPREHENSIVE_PLAN_FOR_THE_DESIGN_OF_FRANKENMINING.md` at HEAD `68e7091`, E682) §37.3 M2 lines 7429–7487 (Lumwana value 7429–7437; recovery card vs 92.7% 7436–7437; recovery-capable set 7446–7448; North Mara H1 2026 line 7457; site-decision calendar 7466–7469; waterfall 7473; acceptance 7476–7481; M2′ 7484–7487) [P00, P02, P03, P04]
- E756 — Withdrawn as not needed for the design, by owner decision on 2026-10-30 [P00]
- E757 — Plan text: plan (`COMPREHENSIVE_PLAN_FOR_THE_DESIGN_OF_FRANKENMINING.md` at HEAD `68e7091`, E682) §35 lines 7188–7247 (§35.1 F1; F3 7199; F4 7200; F7 7203; RISK-03 7220; RISK-22 7239) [P00, P04, P05]
- E758 — Docs: `AGENTS.md` lines 267–268 ("not … built for Barrick") and `README.md` lines 36–39 ("Not affiliated … not built for Barrick") [P00, P04]
- E759 — Plan text: plan (`COMPREHENSIVE_PLAN_FOR_THE_DESIGN_OF_FRANKENMINING.md` at HEAD `68e7091`, E682) §5 NS-01 lines 388–391 ([M0/M2]; bars for grade, recovery, tonnes, price-linked royalty, fuel, FX, sustaining capex) [P00, P04]
- E760 — Plan text: plan (`COMPREHENSIVE_PLAN_FOR_THE_DESIGN_OF_FRANKENMINING.md` at HEAD `68e7091`, E682) §21.6 lines 4525–4528 (TCC → AISC only a residual; component split stays OPEN) [P00, P04]
- E761 — Plan text: plan (`COMPREHENSIVE_PLAN_FOR_THE_DESIGN_OF_FRANKENMINING.md` at HEAD `68e7091`, E682) §31.1 lines 6703–6725 (public lane; cell cites sha256 6720–6722; "No public-lane author holds private access" 6723) and §4.3 line 376 [P04, P05]
- E762 — Plan text: plan (`COMPREHENSIVE_PLAN_FOR_THE_DESIGN_OF_FRANKENMINING.md` at HEAD `68e7091`, E682) Appendix F lines 8392–8455 (lock authoritative 8396; F.1 `FD-…-00` rows 8398–8416, franken_markdown fallback 8416; F.2 `FD-FA-01` 8425, `FD-AS-01` 8447) [P02, P05]
- E763 — Plan text: plan (`COMPREHENSIVE_PLAN_FOR_THE_DESIGN_OF_FRANKENMINING.md` at HEAD `68e7091`, E682) §21.6 lines 4501–4518 (discrepancy register over IR-host workbooks 4501; cell cites the file's sha256 4507–4508; label-anchored import, placeholders typed `Nil`, 4509–4518) [P00, P04, P05]
- E764 — Plan text: plan (`COMPREHENSIVE_PLAN_FOR_THE_DESIGN_OF_FRANKENMINING.md` at HEAD `68e7091`, E682) §34.4 lines 7149–7179 (GATE-DATA-01 7156, -02 7157, -03 7158 incl. "No data by month 7 → M2′"; P(site data) 7163–7179; M2′ ≈ 0.7 at 7176) [P02, P03, P04]
- E765 — Filesystem: `$TMPDIR/rch_target_fmine/` (`/var/folders/vt/n2xyn_s51b97_j3yh2qbqcnc0000gn/T/rch_target_fmine/`) absent when P00, P01, P02 and P05 ran; paths in `.mor/run-20261029T183653Z/repro/cargo_test.log` show it in use at 18:37Z; P01 `df` ≈ 16 GiB free [P00, P01, P02, P05]
- E766 — Case: `planning/mor/r7/MOR_CASE.md` lines 24–27 — Decision Contract loss function attributed to the owner's stated goals [P00]
- E767 — Web: Barrick legal page https://www.barrick.com/English/legal/default.aspx (accessed 2026-10-29 via WebFetch after a Cloudflare 403; files under `/private/tmp/claude-501/-Users-jemanuel-projects-franken-mining/2ba72fbb-c45d-4fd8-93cf-c08cb26d7a18/scratchpad/r7_repro/P05-F4-DEPENDENCIES/web_terms/`) — "Copyright and Trademarks" terms on copying and distribution and "prior written approval"; no governing-law clause reported [P05]
- E768 — Docs/config: `AGENTS.md` line 248 and `.gitignore` lines 40–41 — fixtures are synthetic (fmine-gen) or built from public filings; issuer-website workbooks treated as a fixture source [P05]
- E769 — Plan text: plan (`COMPREHENSIVE_PLAN_FOR_THE_DESIGN_OF_FRANKENMINING.md` at HEAD `68e7091`, E682) cards GB-03 lines 856–865, GB-08 962–964, GB-26 4596–4618, GB-29 4770–4786, GB-35 5960–5973, GB-36 5975–5989, GB-37 6529–6549, GB-38 7360 and 7365 [P00, P03]
- E770 — Code: `crates/fmine-evidence/src/lib.rs` line 10 (INV-011 cited for the Refusal type), lines 26–38 (`Refusal::from_code` reads only `REFUSAL_CATALOG`), lines 70–202 (`REFUSAL_CATALOG`; e.g. line 90 code 104 "Density or specific gravity missing", line 138 code 301 exit 5, line 150 code 303 "Tailings GISTM TARP red threshold exceeded", lines 162–166 code 401 exit 6) [P01, P02]
- E771 — Workbook: `corpus_of_input_docs/downloaded/mine_stats/Barrick_Q2_2026_Mine_Stats.xlsx` — Minestats1!C49/C50/C51 (78 + 18 vs 98, footnote "Includes capitalized interest"), QTD rows 5–28 (14 reporting sites: 11 gold, 3 copper), Minestats3/Minestats4 error cells (953 `#REF!`, 139 `#VALUE!`) [P01, P03]
- E772 — Code: refusal use sites and checks — `crates/fmine-crs/src/lib.rs` lines 10, 28 (101 for CRS mismatch); `crates/fmine-numerics/src/lib.rs` lines 21, 35 (301 for non-SPD Cholesky); `crates/fmine-calib/src/lib.rs` lines 41–98 (`ForecastLedger` lines 41–60, in-memory, caller-supplied `registered_ns`/`freeze_time_ns`; mutation → 403); `crates/fmine-registry/src/lib.rs` lines 90–98 (checks catalog ids exist in the registry only) [P01, P02]
- E773 — Code: `crates/fmine-evidence/src/lattice.rs` lines 9–20 (`EvidenceClass` Placeholder < Assumed < Historical < Calibrated < Exact), 63–70 (`KnowledgeState` KnownKnown/KnownUnknown/UnknownUnknown), 93–100 (`ClaimColor` Green/Amber/Red) [P02]
- E774 — Research note: `planning/research/W1_barrick_mine_stats.md` §5 lines 230–282 (identity tallies; table 248–265, T3 612/94, T8 593/69) and lines 359, 564 (AISC-workbook per-unit values match Mine Stats in 420 of 432 cells) [P01, P03]
- E775 — Execution: `/private/tmp/claude-501/-Users-jemanuel-projects-franken-mining/2ba72fbb-c45d-4fd8-93cf-c08cb26d7a18/scratchpad/r7_repro/P02-A1-CONSISTENCY/cratemap.py` → `cratemap.txt` and `wp_crates.py` → `wp_crates.txt` — §28.3 vs `registries/loc_budget.toml`, `registries/workspace_topology.toml` and Cargo members; §38 crate sets vs §28.3 phases [P02]
- E776 — Plan text: plan (`COMPREHENSIVE_PLAN_FOR_THE_DESIGN_OF_FRANKENMINING.md` at HEAD `68e7091`, E682) §28.3 lines 6338–6349 (budgets; phase A 390k at 6340), §28.4 budget triggers, F10 line 7206, RISK-25 line 7242 [P03]
- E777 — Registry: `registries/refusals.toml` — header calls it the canonical registry (§8.4, §25.6, §31.3, BD-031, BD-039); line 83 code 301 "MNPI wall boundary violation or non-positive-definite covariance matrix"; 304 "Schedule LP/MILP infeasible"; 501 "direct actuation attempt"; 502 "digest mismatch" [P02]
- E778 — Docs: `docs/templates/CONTRACT.md` headings and the 15 product `crates/*/CONTRACT.md` (P02 `contracts.py` → `/private/tmp/claude-501/-Users-jemanuel-projects-franken-mining/2ba72fbb-c45d-4fd8-93cf-c08cb26d7a18/scratchpad/r7_repro/P02-A1-CONSISTENCY/contracts.txt`) — section set Charter & Layer … Non-Goals; LOC budgets (e.g. fmine-cli 14k/16k, fmine-io 16k/18k); `crates/fmine-cli/CONTRACT.md` §2/§5 exit codes 0, 10, 20, 30, 40, 50; contract-only refusal codes `crates/fmine-claim/CONTRACT.md` lines 22–23 (109, 110), `crates/fmine-units/CONTRACT.md` line 29 (206); `crates/fmine-crs/CONTRACT.md` line 22 (G-CRS-1 → 105); `crates/fmine-time/CONTRACT.md` line 22 (104 CalendarWindowMismatch) [P02]
- E779 — Execution: `/private/tmp/claude-501/-Users-jemanuel-projects-franken-mining/2ba72fbb-c45d-4fd8-93cf-c08cb26d7a18/scratchpad/r7_repro/P02-A1-CONSISTENCY/refusals.py` → `refusals_table.txt` — 25 plan refusal codes vs `registries/refusals.toml`, `REFUSAL_CATALOG` and use sites [P02]
- E780 — Plan text: plan (`COMPREHENSIVE_PLAN_FOR_THE_DESIGN_OF_FRANKENMINING.md` at HEAD `68e7091`, E682) §8.4 lines 693–702 (refusal families and codes) and guard tables at lines 2462, 3226–3229, 3561–3565, 5599–5608 (G-CRS-1 → 106 at 5604), 6714–6816 [P02]
- E781 — Plan text: plan (`COMPREHENSIVE_PLAN_FOR_THE_DESIGN_OF_FRANKENMINING.md` at HEAD `68e7091`, E682) §20.10 lines 4105–4124 (GATE-TOUCH-02 line 4120; GATE-TOUCH-03 parity "from the same inputs" line 4121; line 4123) [P02, P03]
- E782 — Sibling repo: `/Users/jemanuel/projects/frankensim` at `20463525d` — `crates/fs-evidence/src/lib.rs` (copy `/private/tmp/claude-501/-Users-jemanuel-projects-franken-mining/2ba72fbb-c45d-4fd8-93cf-c08cb26d7a18/scratchpad/r7_repro/P02-A1-CONSISTENCY/fs_evidence_lib_20463525.rs`; reported to contain no `EvidenceClass`, `KnowledgeState`, `ClaimColor`, `Placeholder`, `Green`, `project_color`) and `crates/fs-evidence/src/color.rs` lines 209–226 (`Color` Verified/Validated/Estimated) [P02]
- E783 — Plan text: plan (`COMPREHENSIVE_PLAN_FOR_THE_DESIGN_OF_FRANKENMINING.md` at HEAD `68e7091`, E682) §7.1 lines 553–566 (eight evidence classes), G-EXEC line 589, NL-1/NL-2 lines 595–596, §8.1 lines 621–624 (Claim `color`, `epistemic`, `state`), G-DISC line 684 [P02]
- E784 — Plan text: plan (`COMPREHENSIVE_PLAN_FOR_THE_DESIGN_OF_FRANKENMINING.md` at HEAD `68e7091`, E682) GATE-PLUMB table lines 1017–1026 (E1 1017, E3 1019, E5 1021, KILL-PLUMB-6 1026), GATE-CORE-02 line 1575, GATE-CRUC-02 line 3244 [P02, P03]
- E785 — Registry: `registries/lifts.toml` — 8 rows lifted from frankensim `20463525d` with `sha256_at_lift` (lattice.rs ← `crates/fs-evidence/src/lib.rs`, `3c62314c…`), guard-test names, header template line 3 "licence MIT" [P02, P05]
- E786 — Execution: `/private/tmp/claude-501/-Users-jemanuel-projects-franken-mining/2ba72fbb-c45d-4fd8-93cf-c08cb26d7a18/scratchpad/r7_repro/P02-A1-CONSISTENCY/lifts.py` → `lifts.txt` — each `registries/lifts.toml` row vs target file, provenance header and guard test [P02]
- E787 — Sibling repos (read-only `git cat-file`/`rev-parse`/`show`/`grep`/`ls-tree`): every `franken-stack.lock` pin resolves; 8 of 8 `lifts.toml` sha256 match `git show <commit>:<source>`; plan-named APIs at pins (asupersync v0.5.0 `franken_evidence::EvidenceLedger` lib.rs:62, `franken_decision::LossMatrix` :247, `Cx::open_child_region` cx.rs:4086, `MacaroonKeyRing::verify` macaroon.rs:612; frankenscipy v0.4.0 solvers); fgdb `b323bd17` and FA `41363acb` pin asupersync by git rev `78b64636e` (FA with `[patch.crates-io]`), fsqlite v0.4.12 and fsci v0.4.0 take asupersync 0.5.0 from crates.io; FA HEAD `a68ef963`; asupersync v0.5.0 `MacaroonKeyRing::verify` compares against an all-zero sentinel when `retired` is `None`; frankensim's LICENSE at the pin byte-identical to FrankenMining's MIT + OpenAI/Anthropic rider; `rust-toolchain.toml` nightly-2026-08-31 [P05]
- E788 — Plan text: plan (`COMPREHENSIVE_PLAN_FOR_THE_DESIGN_OF_FRANKENMINING.md` at HEAD `68e7091`, E682) §34.2 lines 7081 and 7096–7107 (INT-AS-001 admitted "only when" nine criteria hold) [P02]
- E789 — Plan text: plan (`COMPREHENSIVE_PLAN_FOR_THE_DESIGN_OF_FRANKENMINING.md` at HEAD `68e7091`, E682) §39 rows lines 7915–7942 (BD-199 7915; BD-200 7916; BD-201; BD-204 7920; BD-221 7942) [P02, P04]
- E790 — Plan text: plan (`COMPREHENSIVE_PLAN_FOR_THE_DESIGN_OF_FRANKENMINING.md` at HEAD `68e7091`, E682) line 6952 (GATE-PORT-02: one full acQuire/Datamine export) [P02]
- E791 — Docs: `AGENTS.md` line 142 and lines 439–450 — each crate's CONTRACT.md and `docs/templates/CONTRACT.md` described as having the §0.2 sections [P02]
- E792 — Plan text: plan (`COMPREHENSIVE_PLAN_FOR_THE_DESIGN_OF_FRANKENMINING.md` at HEAD `68e7091`, E682) Appendix H line 8540 (L-5 time/chrono allow-list recorded "Fixed") and H.4 row 6 line 8561 (gate-milestone fixes GATE-SYNC-02, GATE-HALL-02) [P02, P05]
- E793 — Execution: `/private/tmp/claude-501/-Users-jemanuel-projects-franken-mining/2ba72fbb-c45d-4fd8-93cf-c08cb26d7a18/scratchpad/r7_repro/P02-A1-CONSISTENCY/gate_timing.py` → `gate_timing.txt` and `gates.py` → `gates_table.txt` — gate milestones vs the WP milestone of each closing bead [P02]
- E794 — Code: `crates/fmine-units/src/lib.rs` lines 95, 194, 228, 560 — comments cite INV-012 for grade non-additivity and BD-026 for the chemical basis (line 560) [P02]
- E795 — Docs: `AGENTS.md` line 289 — `unwrap_used`, `expect_used`, `panic` described as denied [P02]
- E796 — Execution: `/private/tmp/claude-501/-Users-jemanuel-projects-franken-mining/2ba72fbb-c45d-4fd8-93cf-c08cb26d7a18/scratchpad/r7_repro/P03-B11-QUANT/sibling_loc.py` → `sib1.jsonl`, `sib2.jsonl` (25 FrankenSuite siblings, read-only git: non-blank Rust LOC per day), `rg -c` test-annotation counts in five siblings, `stat` times of `crates/*/src/*.rs`, and frankensqlite `crates/fsqlite-ext-rtree` [P03]
- E797 — Execution: `/private/tmp/claude-501/-Users-jemanuel-projects-franken-mining/2ba72fbb-c45d-4fd8-93cf-c08cb26d7a18/scratchpad/r7_repro/P03-B11-QUANT/gates.py` → `gates_out.txt` — exact binomial pass probabilities for statistical gates and small-n calibration power [P03]
- E798 — Plan text: plan (`COMPREHENSIVE_PLAN_FOR_THE_DESIGN_OF_FRANKENMINING.md` at HEAD `68e7091`, E682) §10.4 lines 934–951 ("3–5 years to alarm when 80% coverage slips to about 58%", line 934; simulated false-alarm rate 3.8%) [P03]
- E799 — Plan text: plan (`COMPREHENSIVE_PLAN_FOR_THE_DESIGN_OF_FRANKENMINING.md` at HEAD `68e7091`, E682) card GB-28 lines 4553–4566 and G-BULL-2 line 4572 [P03]
- E800 — Brenner pass: `planning/brenner/B4_substrate_and_integration.md` lines 622–645 (card G1 review-load assumptions: 15 sites per book, 4 families × 3 books, 16 close steps per site per month) [P03]
- E801 — Plan text: plan (`COMPREHENSIVE_PLAN_FOR_THE_DESIGN_OF_FRANKENMINING.md` at HEAD `68e7091`, E682) GATE-ROBOT-01 (line 6678) and GATE-WALL-01 (line 6868) [P03, P04]
- E802 — Plan text: plan (`COMPREHENSIVE_PLAN_FOR_THE_DESIGN_OF_FRANKENMINING.md` at HEAD `68e7091`, E682) §29 lines 6430–6550 (performance budgets, incl. §29.4 interval throughput target) [P03]
- E803 — Web: 17 CFR 243.101(b), https://www.law.cornell.edu/cfr/text/17/243.101, and EDGAR index CIK 0000756894, https://www.sec.gov/cgi-bin/browse-edgar?action=getcompany&CIK=0000756894&type=40-F&dateb=&owner=include&count=10 (both accessed 2026-10-29) — Regulation FD's "issuer" excludes foreign private issuers; Barrick files Form 40-F annually (latest 2026-02-27) [P04]
- E804 — Web: National Policy 51-201 Disclosure Standards (BCSC copy, amended 31 May 2013), https://www.bcsc.bc.ca/Securities_Law/Policies/Policy5/PDF/51-201__NP__May_31__2013 (accessed 2026-10-29; local text under `/private/tmp/claude-501/-Users-jemanuel-projects-franken-mining/2ba72fbb-c45d-4fd8-93cf-c08cb26d7a18/scratchpad/r7_repro/P04-I4-READERS/np51201/`) — ss.3.1, 3.2(c), 5.2, 6.8, 6.9, 6.10 [P04]
- E805 — Brenner pass: `planning/brenner/B5_wedge_sequencing_premortem.md` line 388 (M0 reviewers "Barrick or an ex-major advisor") and lines 502–510 (F3; counsel reviews external use of public-lane forecasts, default share with the sponsor only, 507–509) [P04]
- E806 — Plan text: plan (`COMPREHENSIVE_PLAN_FOR_THE_DESIGN_OF_FRANKENMINING.md` at HEAD `68e7091`, E682) §36.2 lines 7262–7275 (OPEN-B4-07 "before M2" 7272; OPEN-HALL-4 "before M3" 7273) [P04]
- E807 — Web (secondary copies): NI 43-101 s.2.1 via https://cassels.com/insights/national-instrument-43-101-who-is-a-qualified-person-and-what-are-their-responsibilities/ and the CSA June 2025 draft amendments https://fcaa.gov.sk.ca/public/CKeditorUpload/Securities/CSA_Consultation_Papers/2025/43_101_et_al/Annex_A_43-101_NI_Amendments_June_12,_2025.pdf (accessed 2026-10-29) — QP basis for scientific and technical disclosure on a material property [P04]
- E808 — Case: `planning/mor/r7/MOR_CASE.md` lines 48–49 — "Intended readers" list includes investor relations and regulators, attributed to plan §3 [P04]
- E809 — Config: `deny.toml` — licence allow list MIT, Apache-2.0, BSD-2-Clause, BSD-3-Clause, ISC, Unicode-3.0, Zlib (no exceptions/clarify); `multiple-versions = "deny"` (no skip); bans `time`, `chrono`; no petgraph, fastembed or tantivy ban [P05]
- E810 — Sibling repo: asupersync at `78b64636e` (tag v0.5.0) — `Cargo.toml` line 25 (`license = "LicenseRef-MIT-OpenAI-Anthropic-Rider"`), non-optional `nkeys = "0.4.5"`, `tls` feature line 248 (`dep:rustls`, …, `dep:x509-parser`); its `Cargo.lock` (copy `/private/tmp/claude-501/-Users-jemanuel-projects-franken-mining/2ba72fbb-c45d-4fd8-93cf-c08cb26d7a18/scratchpad/as050.lock`: getrandom 0.2.17/0.4.3, rand 0.8.7, rand_core 0.6.4/0.10.1, sha2 0.10.9/0.11.0, digest 0.10.7/0.11.3, x509-parser → time) [P05]
- E811 — Web: crates.io API `https://crates.io/api/v1/crates/<name>` (accessed 2026-10-29) — asupersync max 0.5.0, franken-decision/franken-evidence 0.5.0, ftui 0.9.0 (LicenseRef rider); fsci-linalg/fsci-stats 0.4.0 (MIT); fsqlite 0.4.12, frankensearch 0.7.1, fastmcp-rust 0.13.0, fp-frame 0.6.0 (license-file); `fastmcp` an unrelated 0.0.0 Unlicense crate [P05]
- E812 — Sibling repo: franken_accounting at `41363acb`, `deny.toml` lines 23–80 — per-crate licence exceptions allowing `LicenseRef-MIT-OpenAI-Anthropic-Rider` for asupersync, asupersync-macros and fgdb-* [P05]
- E813 — Web: Barrick FY2025 results-date notice https://s25.q4cdn.com/322814910/files/doc_news/2026/01/Barrick-to-Report-Full-Year-and-Fourth-Quarter-2025-Results-on-February-5.pdf (notice 7 Jan 2026; results 5 Feb 2026) and GlobeNewswire notice of 16 Jan 2025 via https://manilatimes.net/2025/01/16/tmt-newswire/globenewswire/notice-of-release-of-barricks-fourth-quarter-and-full-year-2024-results/2039956/amp (FY2024 results 12 Feb 2025; secondary) (accessed 2026-10-29); bounded search found no January preliminary Q4 production release in 2025 or 2026 [P05]
- E814 — Config: `franken-stack.lock` — foundation rows labelled `FD-AS-01`, `FD-FSIM-01`, `FD-FA-01`, …; row `name = "fastmcp", pin = "=0.13.0"`; sibling pins [P05]

## Directly Observed Facts

- The plan is §0–§39 plus Appendices A–J (E682); §39 lists 320 beads with 533 dependency edges and a 29-bead
  critical path starting at BD-001 (plan §39; AGENTS.md).
- The committed code is 15 product crates plus `xtask` (E673), about 8,859 Rust LOC (E675), with 59 unit tests
  (E674) and a `CONTRACT.md` per crate.
- `fmine assure public --issuer barrick` runs, exits 0 and reproduces its own bytes (E676). The 18 workbook paths
  it names do not exist locally (E677); the loader substitutes an in-code workbook when a read fails (E678).
- Parts of the M0 report are computed from literals in code rather than from workbook cells (E679).
- 87 of 320 beads are closed (E680).

## Executions and Reproductions

E674 (tests), E675 (layering gate), E676 (M0 command, twice) and E690 (disk). Raw logs and outputs are under
`.mor/run-20261029T183653Z/repro/`. The debug binary is at
`$TMPDIR/rch_target_fmine/debug/fmine` on the build host; agents may run it read-only. Do not start new cargo
builds: the host is short of disk (E690) and the results above are already recorded.

## Owner Facts, Values, and Authorities

- The owner wants a plan good enough for Barrick-class executives, built on public Barrick data as the demo,
  general to Barrick/Newmont-class gold, copper, silver and tin miners (owner messages, this project).
- The owner authorized the other agent (ProudCompass) whose plan edits and M0 code are under review.
- Owner rules in AGENTS.md govern: no file deletion, no destructive git, no branches, public lane only,
  RULE 0.1 (no process theatre; real code and real tests together).

## Known Limitations and Negative Evidence

- Earlier review rounds: AUDIT_1–AUDIT_5 (`planning/mor/audits/`), Appendix H (audit fold-in) and H.5b
  (corrections to the concurrent revision). Evidence rows E401–E655 (public research) have not yet had an
  independent second reader (plan §36.4 item H-4).
- `planning/corpus/C5_reko_diq_esia.md` is held back from the public repository (its source carries a
  confidentiality notice); it may be read locally but must not be quoted in outputs.
- The plan says nothing is "implemented" in the §0.2 sense yet (AGENTS.md "Current Repository State"), while the
  M0 commit closes 87 beads (E673, E680).

## Assumptions Requiring Validation

- That the plan's M0–M6 dates and the month-12 program gate are achievable with an AI-agent team and the stated
  crate budgets (E675 reports 1,128,000 planned LOC).
- That the Q3 2026 workbook appears on 9 Nov 2026 in the same layout (plan §37.3).
- That the public Barrick workbooks may be read and redistributed as derived fixtures (licensing not recorded).
- That asupersync `=0.5.0`, franken_accounting and frankensim are usable at the pinned revisions (plan §27, §31.4).

## Pivotal Unknowns

- PU-1: Does the committed M0 code demonstrate what the plan's GATE-BULL-01 and the closed beads claim, on the
  real public workbooks? (E673–E680, E685)
- PU-2: Are the plan's internal commitments mutually consistent (invariants ↔ gates ↔ milestones ↔ work packages
  ↔ beads ↔ crate map ↔ registries ↔ code)? (E682, E684)
- PU-3: Are the plan's quantitative commitments (schedule, budgets, LOC, performance targets, statistical
  guarantees, gate thresholds) correct and feasible? (E675, plan §28, §33–§37, Appendix D/E)
- PU-4: Would the actual readers of M0–M2 outputs (EC/CFO, QPs, site GMs, IR, regulators, JV partners) be able
  to use and trust them as specified, given disclosure rules (NI 43-101, S-K 1300, JORC) and their documented
  needs? (plan §3, §21–§24; E025 and the corpus notes)
- PU-5: Which credible failures in dependencies, data access, licensing or team capacity could reverse the M0–M2
  plan, and are they controlled in the plan? (plan §27, §31, §35)

## Source Access

- Read freely: the whole repository (plan, `planning/`, `crates/`, `registries/`, `docs/`, `.beads/issues.jsonl`,
  README, AGENTS), and the local corpus `corpus_of_input_docs/` (PDF, XLSX; markdown extractions under
  `corpus_of_input_docs/markdown_extractions/`).
- Executions: the built debug binary (read-only); `python3 -I` for arithmetic and for reading `.xlsx` files;
  `br show`/`br list` (read-only). No new cargo builds (E690).
- Web: allowed for primary sources (issuer filings, standards) with the user agent
  "OpenAI File Downloader, XaiImageApiFetch/1.0"; cite URL and access date.
- Not allowed: editing any project file other than your own raw output; quoting C5 or personal data.
