# CONTRACT.md — `xtask`

## 1. Purpose
The T-layer verifier of the workspace (GATE-LAYERS-00, BD-015). `check-all` checks the structural rules over the source tree and `registries/*.toml`: L-1 to L-7, PO-NUM-2, GATE-ORACLE-01 and the §28.4 LOC budget. Other commands do five jobs:
- generate the refusal catalogue;
- capture oracle fixtures;
- check contracts and beads;
- hash, verify, score and order the FY2026 forecast record;
- write the `fmine` binary's bill of materials (`sbom`, BD-073).

`tests/` (29 files) also holds the public lane's cross-crate integration tests. No production crate links it.

- **Crate:** `xtask`
- **Layer:** T
- **Subsystem:** Testpit
- **LOC budget:** not budgeted (dev-only tool; no row in registries/loc_budget.toml)
- **Allowed Module Families:** `["loc", "layers", "deps", "caps", "lifts", "scope", "claims", "det", "oracle", "forecast", "refusals", "int_as", "beads", "contracts", "sbom"]`

## 2. Inputs
- **The tree.** Member `Cargo.toml` files, `crates/**/*.rs` and `crates/*/CONTRACT.md`. Also:
  - `fixtures/` and their capture records;
  - `docs/delivery/` (BUDGET-AMEND and GATE records);
  - `.beads/issues.jsonl`, `deny.toml` and `franken-stack.lock`;
  - `Cargo.lock` and `rust-toolchain.toml`, for `sbom`;
  - the plan's §8.4 codes and its Appendix F.1 rows.
- **Registries.** `registries/`: `loc_budget.toml`, `lifts.toml`, `lift_drift.toml`, `refusals.toml`, `l6_allow.toml`, `sources/*.toml`, `calendars.toml` and `zones.toml`.
- **External programs, read-only:**
  - `cargo tree`, for `check-int-as-001a`, `check-oracle` and `sbom`;
  - `git rev-parse`, `diff`, `ls-files`, `ls-tree` and `show`, for `check-amend`, `check-lifts --upstream`, `check-lift-drift` and `forecast-rules-order`;
  - `openssl ts`, for `forecast-record-verify` and `forecast-record-score`.
- **Library dependencies:**
  - `fmine-registry`, for the source registries, its TOML subset and the calendars;
  - `fmine-calib`, for the pure forecast record and its scoring;
  - `fmine-public`, for the label-anchored Mine Stats reader that `forecast-baseline` and `forecast-record-score` use;
  - `fmine-core` and `fmine-time`.
- **Root.** The workspace is the checkout xtask was built from (`CARGO_MANIFEST_DIR`'s grandparent), not the current directory.

## 3. Outputs
- **Lines and exit codes.** `<check>: PASS` or `<check> FAIL: <path:line …>` lines, and the JSON log lines named under Public API. The exit code is 0 on a pass and 2 on any failure; a usage error is also 2. `sbom` prints its CycloneDX document on stdout and nothing else there; its failures go to stderr.
- **Files written** (every other command writes nothing):
  - `gen-refusals`: `crates/fmine-evidence/src/refusal_catalog.rs`;
  - `capture-fixture`: a fixture and its capture record;
  - `forecast-rules-hash` and `forecast-record-hash`: `rules.sha256` and `record.sha256`;
  - `contracts --write-budget`: the three registry lines of each contract's section 1.
- **No Claims.** It emits none.

## 4. Invariants
- **A writer writes all or nothing**, and a second run changes nothing:
  - `gen-refusals` writes only when the text changes: `test_stale_or_hand_edited_catalogue_fails`.
  - `capture-fixture` writes nothing on any refusal: `test_capture_refusals_write_nothing`.
  - `contracts --write-budget` checks every contract before writing one: `test_write_budget_is_idempotent`.
  - A forecast digest is replaced only while no proof stands beside it: `test_digest_frozen_only_once_stamped`.
- **A failed program is an error, never a pass.** An external program that does not run (`cargo tree`, `git`, `openssl`) fails its check, with one exception: inside `check-all`, a root that is not a git work tree's top level makes `check-amend` warn instead.
- **Each check has a seeded negative test**, beside the workspace pass `test_workspace_check_all_gate_layers_00_passes`. Seeded negatives:
  - `check_layers`: `test_seeded_upward_layer_edge_fails`;
  - `check_caps`: `test_seeded_open_child_region_fails`, `test_seeded_detached_worker_or_timer_fails_outside_the_runtime`;
  - `check_deps`: `test_seeded_tokio_fails_check_deps_in_every_declaration_form`;
  - `check_claims`: `test_seeded_bare_f64_quantity_fails`, `test_l6_any_bare_f64_parameter_fails_on_one_line_or_several`, `test_l6_allow_rows_excuse_their_sites_and_refuse_stale_or_empty_ones`;
  - `check_det`: `test_seeded_std_transcendental_in_bitwise_portable_fails`;
  - `check_scope`: `test_seeded_smuggled_module_fails_check_scope`;
  - `check_oracle`: `test_seeded_oracle_outside_the_workspace_fails_the_closure_but_not_check_deps`;
  - `check_sources_registry`: `test_registry_check_on_workspace_and_on_a_broken_copy`;
  - `loc::check_amend`: `test_synthetic_overrun_blocks_a_merge`;
  - `beads`: `test_golden_negative_control`;
  - `sbom`: `undescribable_closures_are_refused`.
  `check_calendars` has none here.

## 5. Refusals
Exit 2, naming each finding. The findings are:
- an upward layer edge (L-1);
- a banned dependency (L-5);
- `open_child_region` outside `fmine-runtime`, or a banned capability (L-3);
- a lift provenance mismatch (L-7);
- an undeclared module family (§28.4);
- a parameter typed bare `f64` in a K/E `pub fn` that no `registries/l6_allow.toml` row names, or a row that names no such parameter (L-6);
- a K/E file without a determinism class, or a std transcendental in a BitwisePortable module (PO-NUM-2);
- a change adding LOC to a crate over its ceiling, or over planned without an approved amendment (§28.4);
- an oracle crate in a production closure, or a fixture without its capture record (GATE-ORACLE-01);
- a `franken-stack.lock` id set that differs from plan Appendix F.1 (BD-326.S1);
- the refusal, lift, bead and contract rules under Public API.

No `FMINE-REF-nnn` code is emitted: `check-refusals` checks those codes, it does not raise them.

## 6. Determinism class
Undeclared: `xtask check-det` requires a header only in the K and E layers. A check's result depends on the tree and, for `check-amend`, `check-lift-drift`, `check-lifts --upstream` and `forecast-rules-order`, on git history. Source files (`collect_rs_files`) and source registries are visited in sorted path order. `capture-fixture` records today's UTC date and the host's `<arch>-<os>`. `sbom` writes no timestamp or serial number, sorts its components and edges, and writes paths relative to the root, so its bytes depend only on the closure, `Cargo.lock`, `rust-toolchain.toml` and the binary (`rendering_is_cyclonedx_and_independent_of_lock_order`).

## 7. Evidence class of outputs
Not applicable: no Claims are emitted.

## 8. Validity domain
- The static checks read text, not a Rust syntax tree:
  - `check-det` matches `.name(` and `f64::name(` outside comments and test code.
  - `check_scope` matches the `mod` lines of `src/lib.rs`.
  - `check_claims` (L-6) reads each `pub fn` signature, across lines, in the production code of the K and E crates except `fmine-numerics`. Test code and line comments are skipped. A parameter fails when its type is exactly `f64`. It does not check return types, which the plan's L-6 also names, nor `pub(crate)` or private functions, slices, tuples, closures' arguments or struct fields. A `pub fn` written inside a string literal is read as code.
  - An allow row matches by crate, file, function name and parameter, not by `impl` type, so two `pub fn new` in one file share their rows.
- `check-amend` needs the root to be a git work tree's top level. Without one it is a warning in `check-all`, and an error with `--base`.
- `check-lifts --upstream` and `check-lift-drift` need a frankensim checkout. CI runs only the drift check, each quarter (`.github/workflows/lift-drift.yml`). `test_upstream_known_answers_when_a_checkout_is_given` passes without running when `FRANKENSIM_PATH` is unset.
- `sbom` describes the closure over all targets (`--target all`), so a crate used only on another platform is listed too. A path package has no purl, since `pkg:cargo/<name>` would name a crates.io crate. A licence is the manifest's declared `license` field: the workspace declares MIT, but `LICENSE` reads "MIT License (with OpenAI/Anthropic Rider)" (E681), so the bill's MIT is the declaration, not necessarily the full terms, until the owner decides (RISK-17). The output is checked by its tests and parses as JSON; it has not been validated against the official CycloneDX schema.
- `forecast-record-verify` and `forecast-record-score` need OpenSSL 3 or later as `openssl` on PATH. OpenSSL 1.1 cannot build the chain from the committed DigiCert rules proof to its pinned root, and refuses it (macos-15 CI, run 41322629628). LibreSSL 3.3.6 (`/usr/bin/openssl` on macOS) has no `ts -verify -attime`. OpenSSL 3.6.5 and 4.0.3 verify the proof. A refusal under an older `openssl` fails closed; it says nothing about the proof.
- `forecast-baseline` needs the registered workbooks under `corpus_of_input_docs/`, so it never runs in CI (RISK-31). Its test is `#[ignore]` for that reason. So are 30 tests in `tests/`, which need registered workbooks, PDFs or the local transcription registry.

## 9. Oracles
- The plan text: §8.4 for `check-refusals` (`test_committed_registry_catalogue_and_plan_agree`) and Appendix F.1 for `check-stack-lock` (`test_lock_ids_match_appendix_f1`).
- The frankensim checkout, whose blobs `check-lifts --upstream` and `check-lift-drift` hash through `git show` (`test_rule_e_upstream_digest_and_overlap`, `test_rule_g_drift_against_a_seeded_upstream`).
- `openssl ts -verify` against the checked-in DigiCert or FreeTSA root, whose sha256 is pinned in `TSA_ROOTS` (`tsa_chain.pem` adds only untrusted intermediates), for a forecast record's time (`test_proof_times_from_openssl_text`).
- The other checks have no external oracle: their tests are seeded violations.

## 10. Known gaps
- **L-6 debt (bd-u56b).** `registries/l6_allow.toml` holds five `debt` rows, twelve bare physical or money parameters that existed when the deny-by-default rule landed. They are in `fmine-decide` (`from_linear_sensitivity`, `build`) and `fmine-metrics` (`compute_geo`, `compute`, `solve_cutoff_grade`). The sixth, `fmine-calib`'s `SpatialFrontPitMonitor::new`, now takes its range as a `Qty<Length>`; its two variances are a `generic` row, since the monitor is unit-agnostic. Each should take a typed quantity or a Claim. Bare `f64` return types are not checked at all.
- **Gaps in check-all and CI:**
  - `check-all` does not run `contracts`, which waits for bd-2a73.2 to rewrite every contract, nor `check-beads`. CI runs `check-all`, `check-amend --base`, `loc` and the quarterly drift check, so neither of the two runs there.
  - `check_scope` still holds the older rule of sections 1 to 10, which a twelve-section contract also meets.
- **No calendars negative test.** `check_calendars` has no seeded negative test in xtask.
- **BD-015 (bd-015) stays open.** It waits behind its blockers, bd-012 (an owner decision) first.

## 11. Public API
- `check_loc_budget(root: &Path) -> Result<LocReport, Vec<String>>` (BD-003, BD-012; commands `loc` and `check-budget`).
- `loc::{check_amend, evaluate, measure, read_amendments, budget_rows}`. These implement BD-012 (§28.4) through the command `check-amend [--base <rev>]`. `check-all` runs it against `HEAD` and prints its line `{"check":"check-amend","measured","changed","over_planned","amendments","approved","warnings"}`.
  - **Measuring a change.** A change is the diff from the merge-base of the base and `HEAD` to the working tree, untracked files included. It covers the `.rs` files git lists, and counts non-blank lines as `check_loc_budget` does.
  - **Over planned.** A change that adds LOC to a crate over its planned budget fails, unless the crate has an approved `docs/delivery/BUDGET-AMEND-nnn….md` record:
    - the record reads `Approval: approved by <who>, YYYY-MM-DD`;
    - its `New planned` is at least the crate's actual LOC.
    A change with net LOC ≤ 0 needs no record (§28.4.4).
  - **Over the ceiling.** A change that adds LOC to a crate over its ceiling fails, record or not. Net LOC ≤ 0 passes with a warning (§28.4.4). With no change measured, a crate over its ceiling fails.
  - **Budget rows.** A crate's row in `registries/loc_budget.toml` that differs from the base's fails, unless an approved record for that crate has `New planned` equal to the new number. So a change cannot move budget between crates.
  - **Warnings.** These are warnings, not failures:
    - a crate over planned that nothing adds to;
    - a crate at 80% of planned. §28.4.2 puts this warning in `fmine doctor`, and `check-all` prints it until that verb reads the budget;
    - an approval added by the change under test, which names the record: the check cannot tell who wrote it.
  - **Failures on records.** A malformed record name, a missing field, a repeated id, or a record for a crate the budget does not list, fails.
  - **Git root and renames.** A root that is not a git work tree's top level is a warning in `check-all`; with `--base`, which CI passes over the pushed range, it fails. A crate directory renamed in the change counts as wholly added.
  - The form is `docs/templates/BUDGET-AMEND.md`. `check_loc_budget` reports `over_ceiling` and no longer fails on it.
- `check_layers(root: &Path) -> Result<(), Vec<String>>` (L-1, BD-004): dependencies point only down the layers.
- `check_deps(root: &Path) -> Result<(), Vec<String>>` (BD-005): no member names a `deny.toml` crate in any dependency table.
  - The tables are normal, build and dev, each also under `target.<spec>`. A crate counts whether named inline or dotted, by key or through `package = "..."`, quoted either way.
  - The banned crates are the `[bans] deny` rows, one per line. A row with `wrappers` still bans its crate here, and `skip` rows ban nothing.
  - A `deny.toml` with no readable rows, or one unreadable deny row, fails.
  - Transitive closures are B(i), and `cargo deny` in CI (BD-326).
- `check_caps(root: &Path) -> Result<(), Vec<String>>` (L-3, BD-006): bans `open_child_region` outside `fmine-runtime`, and bans `std::fs`, `std::net`, `std::process` and `SystemTime::now` in S0, K and E production files. It also bans `thread::spawn`, `thread::Builder`, `thread::sleep`, `Instant::now` and `SystemTime::now` in production code outside `fmine-runtime` and `fmine-lab`.
- `int_as::{check_int_as_001a, evaluate, parse_tree_line}` (BD-325; command `check-int-as-001a [--json]`, and `check-all` prints its line). Each production closure must have:
  - one package id for each crate in `SINGLE_ID`;
  - none of `BANNED_RUNTIMES`, with a finding naming the path that brought one in;
  - asupersync, when present, through `[patch.crates-io]` at `franken-stack.lock`'s rev.
  It prints `NOT_APPLICABLE` while asupersync is absent, and the two other checks still fail. A `cargo tree` that does not run is an error, never a pass. Criteria 1 to 8 belong to INT-AS-001b.
- `lifts::{check_lifts, check_upstream}` (bd-5vex; command `check-lifts`, run by `check-all`, and `check-lifts --upstream <path>`, local only) check `registries/lifts.toml` schema 2. `check_lifts` fails on:
  - (a) a missing target;
  - (b) a header without all four §27.1 fields, or one disagreeing with its row;
  - (c) a `guard_test` not in the target's test module;
  - (d) a malformed `commit` or `sha256_at_lift`, or a licence other than `UPSTREAM_LICENCE`;
  - (f) a provenance line with no `[[lift]]` row, or an `[[analogue]]` not opening with `ORIGINAL_HEADER`.
  `check_upstream` (e) recomputes each sha256 through `git show`, and flags a target under `MIN_OVERLAP_PERCENT`. CI has no frankensim checkout, so it never runs there.
- `lifts::{check_drift, parse_drift}` (bd-d2l5, GATE-FSIM-DRIFT; command `check-lift-drift --upstream <path> [--head <rev>]`). `.github/workflows/lift-drift.yml` runs it each quarter against frankensim main.
  - It compares each `[[lift]]` source's blob at its pinned commit with the same path at `<rev>` (default HEAD), from committed objects only.
  - It prints one JSON line per lift: `unchanged`, `changed` (lines added and deleted, the hunk ranges, the commits that touched it) or `missing`.
  - It fails on a changed or missing source without a `registries/lift_drift.toml` decision whose `upstream` commit holds the same blob. A decision is `relift`, with its bead, or `no-op`, with a reason.
  - It also fails on a decision naming no `[[lift]]` row.
- `check_stack_lock(root: &Path) -> Result<StackLockReport, Vec<String>>` and the pure `compare_lock_to_f1(lock, plan)` (BD-326.S1; command `check-stack-lock`, and `check-all` prints its line `{"check":"stack-lock","lock_ids","f1_ids",…}`).
  - The `id` set of `franken-stack.lock` must equal the `| FD-… |` rows from plan heading `### F.1 ` to the next heading. F.2's `-01` upstream asks are outside it.
  - It fails on a missing or extra id, a lock id not ending `-00`, a malformed or repeated id, or an empty set.
- `check_scope(root: &Path) -> Result<(), Vec<String>>` (BD-002, BD-008): every crate has a `CONTRACT.md` with sections 1 to 10. Every module `src/lib.rs` declares, at any visibility and outside `#[cfg(test)]`, is listed in its Allowed Module Families.
- `contracts::{check_contract_text, findings, rewrite_budget_lines, check_contracts, write_budget}` (BD-327.S1). The commands are `contracts` and `contracts --write-budget`.
  - `contracts` prints `{"crate","headings_ok","budget_ok","layer_ok","subsystem_ok","unregistered_codes"}` per contract, then `{"contracts","errors","warnings"}`.
  - A contract fails when its level-2 headings are not exactly the twelve of `docs/templates/CONTRACT.md`, in order, or when a section is empty.
  - It fails when its section 1 `Layer`, `Subsystem` or `LOC budget` line differs from the crate's `registries/loc_budget.toml` row (`26,000 planned / 30,000 ceiling`), or names a subsystem outside §0.5's eighteen.
  - `xtask` has no row, so its LOC budget line reads `not budgeted (dev-only tool; no row in registries/loc_budget.toml)`.
  - A `FMINE-REF-nnn` with no row in `registries/refusals.toml` is a warning until BD-328 closes.
  - `--write-budget` rewrites those three lines from the registry and no other byte, and a second run changes nothing. A contract without exactly one of each line in section 1 is refused, with nothing written.
- `beads::{check_beads, evaluate}` (BD-324; command `check-beads`, which prints `{"check":"check-beads","rows","closed","distinct"}`) runs over `.beads/issues.jsonl`. A closed bead fails when:
  - its normalised reason is shared with another closed bead (R-SHARED);
  - it holds no `test:`, `record:`, `ci:`, `decision:` or `commit:` reference (R-TYPED);
  - it cites one that does not resolve (R-RESOLVE);
  - it cites a `GATE-…` token with no `docs/delivery/<token>*.md` record holding a `Verdict: PASS` line (R-GATE);
  - it is closed over a `blocks` dependency that is not closed (R-BLOCKED).
  A `test:` reference's last `::` segment must be a `fn` anywhere under `crates/`. It is not looked up in the named crate, because tests have moved between crates since they were cited. A `record:` reference must be a file, and `ci:` and `commit:` are format-checked only. Ten closes within 60 s is a warning (R-WINDOW). The JSONL reader is hand-written (no serde), and a malformed line is an error naming its number.
- `check_claims(root: &Path) -> Result<(), Vec<String>>` (L-6, BD-009, bd-u56b): deny by default. Every bare `f64` parameter of a K/E `pub fn` fails unless `registries/l6_allow.toml` names it.
  - A row is `[[allow]]` with `crate`, `file` (relative to the crate), `fn`, `params` (comma-separated), `kind` and `reason`.
  - `kind` is `generic` (dimensionless, or a statistical function over values in the caller's own unit) or `debt` (a physical quantity or money passed bare).
  - A row fails when a field is missing, unknown or not a string, the kind is neither, the reason or `params` is empty, a parameter is listed twice in the file, or a parameter matches no site. A key or table outside `[[allow]]` fails too, and a missing file reads as empty.
  - `check_bare_quantity_signature(path, prod_code)` fails on the first bare `f64` parameter of one source, with no allow-list.
- `claims::{bare_f64_params, parse_allow, evaluate, BareParam, AllowRow, Site, ALLOW_PATH, KINDS}`: the scanner, the allow-list reader and the matcher behind `check_claims`.
- `check_det(root: &Path) -> Result<DetReport, Vec<String>>` (bd-hp2j.1; command `check-det`, and `check-all` prints its line `{"check":"check-det","files","headers_missing":0,"violations":0,"bitwise_files"}`).
  - Every production file of a K or E crate declares `//! Determinism: <class>`, with a §29.2 class (`DET_CLASSES`).
  - A `BitwisePortable` file calls none of `BANNED_TRANSCENDENTALS`, as `.name(` or `f64::name(`, outside comments and test code (`banned_transcendental_calls`).
  - Only `DET_EXEMPT` (`crates/fmine-core/src/det.rs`) is exempt, and an unreadable file is an error.
- `check_oracle(root: &Path) -> Result<oracle::Report, Vec<String>>` and `oracle::{check, closure_findings, check_capture_records, record_errors}` (GATE-ORACLE-01, BD-013, §33.2). The command is `check-oracle`; `check-all` runs `oracle::check` and prints `{"check":"gate-oracle-01","closure_packages","fixtures","oracles_in_closure":0}`.
  - **Closure.** It runs `check_deps`' direct check, then `cargo tree -e normal,build` over the workspace without `xtask` and the member oracle crates. Any `ORACLE_CRATES` package at any depth fails, naming the path. This catches an oracle reached through a crate outside the workspace, which the direct check cannot see. Dev-dependencies are not in the closure.
  - **Capture records.** Every file under `fixtures/` needs `<file>.capture.toml` beside it, with the nine §33.2 fields once each. Its `outputs_digest` must be the fixture's sha256, and a record with no fixture fails. The form is `docs/templates/CAPTURE-RECORD.md`.
- `oracle::{capture, parse_capture_args, inputs_digest, host_platform, utc_date_today, Capture}` (BD-136). The command is `capture-fixture <fixture> --oracle --version --captured-by --licence --input <path>... [--note] [--replace] -- <command>...`.
  - It runs the command from the root without a shell, writes its stdout as the fixture, and writes the record beside it.
  - The record holds:
    - `platform` as `<arch>-<os>`;
    - `date` as today in UTC;
    - `inputs_digest`, which is one input's sha256, or for several inputs the sha256 of their sorted `sha256:<hex> <path>` manifest;
    - the command as run.
  - The record must pass `record_errors` before either file is written.
  - It writes nothing when:
    - the command fails or prints nothing;
    - a value holds a quote, a backslash or a line break;
    - without `--replace`, the fixture or its record exists.
- `check_sources_registry(root: &Path) -> Result<Vec<String>, Vec<String>>` (`registry-check`): on success, one `{"stage":"registry",…}` JSON log line per `registries/sources/*.toml`.
- `check_calendars(root: &Path) -> Result<String, Vec<String>>` (BD-029). `check-all` runs it and prints `{"check":"calendars","sites","tzdata"}`. `fmine_registry::calendars::load_site_calendars` must join `registries/calendars.toml` with the zone snapshot `registries/zones.toml`.
- `refusals::{check_refusals, gen_refusals}` (BD-328). The commands are `check-refusals` and `gen-refusals`; `check-all` runs the check and prints `{"check":"check-refusals","rows","active","proposed","reserved",…}`.
  - `gen_refusals` renders `registries/refusals.toml` (schema 2) into the `RefusalCode` enum and `CATALOG`, and writes the file only when its text changes.
  - `check_refusals` fails unless all of these hold:
    - (e) the committed catalogue equals the rendered text;
    - (a) every `active` code is in plan §8.4, and every §8.4 code is `active`;
    - (b) each exit code follows the §30.1 family rule or a listed exception (504→7, 506→5), and no exception is unused;
    - (c) every `FMINE-REF-nnn` token in `crates/**/*.rs` and `crates/*/CONTRACT.md` is a registry code, named `path:line`;
    - (d) no fix text says "clamp".
- `check_all(root: &Path) -> Result<LocReport, Vec<String>>` (BD-015): every check above except `contracts`, `check-beads`, `check-lifts --upstream`, `check-lift-drift` and the forecast commands.
- `forecast::{rules_hash, record_hash, record_verify}` (BD-334.S1). The commands are `forecast-rules-hash <rules.toml>`, `forecast-record-hash <dir>` and `forecast-record-verify <dir>`: the IO around the pure `fmine_calib::record`.
  - They write `rules.sha256` and `record.sha256`, and replace a different digest only while no proof (`.tsr`, `.ots`) stands beside it.
  - `record_verify` takes the record's time only from a signed `record.tsr` verified against the compiled SHA-256 pins for the checked-in DigiCert or FreeTSA root PEM (`TSA_ROOTS`). A supplied `tsa_chain.pem` contributes only untrusted intermediates. Certificate validity is evaluated at the token's signed time, and a separately authenticated rules proof must strictly precede the record proof including both accuracy bounds. Changed proof/root bytes refuse. `forecast::proof_times_from_text` reads signed time and accuracy; an overflowing interval refuses.
  - `rules_hash` refuses rules that do not parse or still carry an `OPEN` marker.
  - The record is validated against the dates its rules state, and its `forecasts.csv` against their universe and targets.
  - The manifest's advisory registry-hash binding includes registered documents as well as workbooks. This authenticates the frozen declarations and detects conflicting registered digests; it does not prove actual input use or replay a generator. Source adapters separately verify the bodies they consume.
- DESIGN (BD-334/BD-068): `forecast::record_score` (command `forecast-record-score <dir>`) shares that verifier and scores the already loaded frozen manifest/CSV/rules against registered original-release Mine Stats Claims. It writes nothing.
  - It verifies time proofs before opening outcome workbooks. Production is the signed exact difference between original FY and original nine-month totals on the explicit 100% basis; AISC uses the original Q4 QTD rate directly. Later comparatives never fill missing originals. Published numeric observations, including zero and source-status flags, retain their declared measurement meaning and source evidence. A Q2 ownership-label reference is used only when its exact registered bytes occur in the frozen manifest.
  - Every frozen site/target remains visible, with unresolved and not-forecast states. The deterministic JSON assessment includes exact losses, observed and full-cohort coverage/loss bounds, the declared iid reference interval and rho sensitivity, source Claims/digests/locators, proof bounds and an assessment digest. It creates no forecast or timestamp and claims no calibration.
  - `forecast::scoring::benchmarks` supplies the frozen issuer-guidance and persistence comparators to `fmine_calib::record::scoring::compare`. Guidance uses the latest agreed pre-cut document; same-day publication metadata needs exact manifest-bound bytes and an earlier declared retrieval, whose declared status is retained. No older guidance substitutes for an uncertain latest candidate. Original nine-month displayed production supplies the signed annual-guidance remainder; cost rates are never subtracted.
  - The fixed original Q3 point and consecutive-quarter persistence pool are separate registered benchmark information sets, with manifest membership disclosed. Missing/unreadable required history refuses the comparator rather than shortening its pool. Printed zero and signed points remain valid; only historical ratio pairs carry the frozen positive-number restriction. Source Claims retain stored and displayed values.
  - All three means and both differences use one identical complete member set, retaining every exclusion. Per-target arithmetic refusal retains that population and the other target; the final optional adapter boundary preserves unconditional forecast/outcome scores. `benchmark_assessment` serializes statuses, exact results, source/timing evidence and pool exclusions. Issuer guidance has no inferred nominal probability, and no same-information-set, skill or calibration claim is made.
- `forecast::persistence_baselines` (command `forecast-baseline`) computes each `run_rate_persistence` baseline of the FY2026 rules from the registered Mine Stats workbooks.
  - The workbooks are read through `fmine-public` under the registered vocabulary, and a workbook with a refused cell refuses.
  - Each baseline gives the pool's size, the pairs left out and the two ratio quantiles. Once the point quarter's workbook is registered, it also gives each live site's exact interval.
  - It needs the corpus, so it never runs in CI (RISK-31).
  - `forecast-baseline --as-of YYYY-MM-DD` adds `forecast::guidance::run`'s issuer-guidance baseline to that persistence output. It selects the latest registered two-reader annual guidance at the inclusive date cut; ties and pending newer readings stay unavailable. Production divides by its stated attributable share before subtracting the original Q3-release nine-month Claim; AISC retains its per-sold-ounce interval unchanged. All frozen sites and targets remain visible, including excluded sites and missing originals. Negative remainders are retained. The report binds rules, exact intervals, source Claims, PDF locators/digests/readers and availability evidence. This baseline uses workbook retrieval as its conservative availability bound, including when per-workbook publication dates are registered; earlier cuts refuse. No forecast, time proof or calibration claim is created. Actual registered-file integration controls are `tests/forecast_guidance_baseline.rs`.
  - `scripts/forecast_record_e2e.sh` checks synthetic digest/mutation failures and verifies that a throwaway local TSA (`test_tsa`) cannot authenticate a public record. The committed rules proof is independently checked under the pinned public authority in unit tests, including wrong-digest refusal. `scripts/verify_rules_timestamp.sh` checks the rules' proofs with openssl and ots alone.
- `sbom::{sbom, build, render, parse_lock}` (BD-073; command `sbom [--binary <path>]`). The bill is CycloneDX 1.5 JSON for `fmine-cli`'s normal and build closure, as `cargo tree --locked -e normal,build --target all` resolves it, so dev-dependencies and `xtask` stay out.
  - Each component has its name, version and licence expression. A path package has its relative path, a crates.io package its purl and the lock's SHA-256 checksum, and a git package a purl whose `vcs_url` holds the lock's full commit.
  - `dependencies` lists every component's direct edges. The metadata records the toolchain channel, `Cargo.lock`'s sha256 and the `cargo tree` command. With `--binary`, the root component also carries the binary's file name and sha256.
  - It refuses a package the lock lacks or holds more than once, a path outside the workspace, a registry other than crates.io, a crates.io package without a checksum, a git source without a commit, and a tree whose single root is not `fmine-cli`.
  - `the_workspace_bill_is_the_manifest_closure` checks the real bill against the closure read from the member manifests without cargo. `.github/workflows/blind-binary.yml` uploads the bill beside the blind binary.
- `forecast::rules_order` (BD-334.S2; command `forecast-rules-order`): the commit adding `forecasts/fy2026/rules.toml` is a strict ancestor of every commit touching `forecasts/fy2026/forecasts.csv`. It refuses a shallow clone, whose history cannot show the order.

## 12. Proof obligations
- PO-NUM-2 (no std transcendental in a BitwisePortable module): `check_det`. Tests:
  - `test_seeded_std_transcendental_in_bitwise_portable_fails`;
  - `test_seeded_method_call_ln_in_bitwise_portable_fails`;
  - `test_seeded_ufcs_f64_exp_in_bitwise_portable_fails`;
  - `test_workspace_check_det_passes_and_logs`.
- PO-UNIT-2 (no K or E public function takes a bare `f64` for a physical quantity) is enforced for parameters by `check_claims` in `check-all`, and is not discharged while the five debt rows remain and returns go unchecked (Known gaps). Tests:
  - `test_l6_any_bare_f64_parameter_fails_on_one_line_or_several`;
  - `test_l6_typed_private_and_commented_parameters_pass`;
  - `test_l6_allow_rows_excuse_their_sites_and_refuse_stale_or_empty_ones`;
  - `test_seeded_bare_f64_quantity_fails`;
  - `test_workspace_check_all_gate_layers_00_passes`.
- GATE-ORACLE-01 (no oracle crate in a production closure; every fixture has its capture record). Tests:
  - `test_seeded_oracle_outside_the_workspace_fails_the_closure_but_not_check_deps`;
  - `test_every_fixture_needs_a_record_with_its_digest`.
- §28.4 (a change cannot pass a crate's ceiling, or its planned budget without an approved amendment). Tests:
  - `test_synthetic_overrun_blocks_a_merge`;
  - `test_over_planned_change_needs_an_approved_amendment`;
  - `test_ceiling_blocks_only_changes_that_add_to_the_crate`;
  - `test_budget_moved_between_crates_in_a_change_refuses`.
