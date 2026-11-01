# CONTRACT.md — `xtask`

## 1. Charter & Layer
- **Crate:** `xtask`
- **Layer:** `T` (Build/Verification tooling)
- **Subsystem:** `Testpit`
- **LOC Budget:** Dev-only verification binary (`GATE-LAYERS-00`)
- **Allowed Module Families:** `["loc", "layers", "deps", "caps", "lifts", "scope", "claims", "det", "oracle", "forecast", "refusals", "int_as", "beads"]`

## 2. Mathematical & Domain Formulation
Enforces the structural invariants of the workspace (`L-1`..`L-7`, `PO-NUM-2`, `GATE-ORACLE-01`, `§28.4` LOC budget ceilings) over the repository source tree and `registries/*.toml`.

## 3. Authority, Epistemics & Representation
Pure static verifier over repository source files and TOML registries; produces deterministic pass/fail diagnostics (`GATE-LAYERS-00`).
The `forecast-*` commands are the exception: they write a record's digest files and run `openssl ts` and `git`
(BD-334.S1, S2). `gen-refusals` is the other: it writes `crates/fmine-evidence/src/refusal_catalog.rs`.
`check-int-as-001a` and `check-oracle`, both also run by `check-all`, run `cargo tree`. `check-amend`, also run by `check-all`, runs
`git rev-parse`, `git diff`, `git ls-files`, `git ls-tree` and `git show`, read-only.

## 4. Public API Surface
- `check_loc_budget(root: &Path) -> Result<LocReport, Vec<String>>`
- `loc::{check_amend, evaluate, measure, read_amendments, budget_rows}` (BD-012, §28.4; command `check-amend
  [--base <rev>]`, and `check-all` runs it against `HEAD` and prints its line
  `{"check":"check-amend","measured","changed","over_planned","amendments","approved","warnings"}`):
  - A change is the diff from the merge-base of the base and `HEAD` to the working tree, untracked files included,
    over the `.rs` files git lists, counting non-blank lines as `check_loc_budget` does.
  - A change that adds LOC to a crate over its planned budget fails unless a `docs/delivery/BUDGET-AMEND-nnn….md`
    record for that crate reads `Approval: approved by <who>, YYYY-MM-DD` with `New planned` at least the crate's
    actual LOC. Net LOC ≤ 0 needs no record (§28.4.4).
  - A change that adds LOC to a crate over its ceiling fails, record or not; net LOC ≤ 0 passes with a warning
    (§28.4.4). With no change measured, a crate over its ceiling fails.
  - A crate's row in `registries/loc_budget.toml` that differs from the base's fails unless an approved record for
    it has `New planned` equal to the new number, so a change cannot move budget between crates.
  - A crate over planned that nothing adds to, or one at 80% of planned, is a warning. §28.4.2 puts the 80% warning
    in `fmine doctor`; `check-all` prints it until that verb reads the budget. An approval added by the change
    under test is a warning naming the record: the check cannot tell who wrote it.
  - A malformed record name, a missing field, a repeated id, or a record for a crate the budget does not list,
    fails. A root that is not a git work tree's top level is a warning in `check-all`; with `--base`, which CI
    passes over the pushed range, it fails. A crate directory renamed in the change counts as wholly added.
  - The form is `docs/templates/BUDGET-AMEND.md`. `check_loc_budget` reports `over_ceiling` and no longer fails on
    it.
- `check_layers(root: &Path) -> Result<(), Vec<String>>`
- `check_deps(root: &Path) -> Result<(), Vec<String>>` (BD-005): no member names a `deny.toml` crate in any
  dependency table (normal, build and dev, each also under `target.<spec>`, inline or dotted, by key or through
  `package = "..."`, quoted either way). The crates are the `[bans] deny` rows, one per line; a row with `wrappers`
  still bans its crate here, and `skip` rows ban nothing. A `deny.toml` with no readable rows, or one unreadable
  deny row, fails. Transitive closures are B(i), and `cargo deny` in CI (BD-326).
- `check_caps(root: &Path) -> Result<(), Vec<String>>`: besides `L-3`, bans `thread::spawn`, `thread::Builder`,
  `thread::sleep`, `Instant::now` and `SystemTime::now` in production code outside `fmine-runtime` and `fmine-lab`.
- `int_as::{check_int_as_001a, evaluate, parse_tree_line}` (BD-325; command `check-int-as-001a [--json]`, and
  `check-all` prints its line): on each production closure, one package id for each of `SINGLE_ID`, none of
  `BANNED_RUNTIMES` (naming the path that brought it in), and asupersync, when present, through
  `[patch.crates-io]` at `franken-stack.lock`'s rev. Prints `NOT_APPLICABLE` while asupersync is absent, and the
  two other checks still fail. A `cargo tree` that does not run is an error, never a pass. Criteria 1 to 8 are
  INT-AS-001b.
- `lifts::{check_lifts, check_upstream}` (bd-5vex; command `check-lifts`, run by `check-all`, and
  `check-lifts --upstream <path>`, local only): `registries/lifts.toml` schema 2. `check_lifts` fails on (a) a
  missing target; (b) a header without all four §27.1 fields, or disagreeing with its row; (c) a `guard_test` not
  in the target's test module; (d) a malformed `commit` or `sha256_at_lift`, or a licence other than
  `UPSTREAM_LICENCE`; (f) a provenance line with no `[[lift]]` row, or an `[[analogue]]` not opening with
  `ORIGINAL_HEADER`. `check_upstream` (e) recomputes each sha256 through `git show` and flags a target under
  `MIN_OVERLAP_PERCENT`; CI has no frankensim checkout, so it never runs there.
- `lifts::{check_drift, parse_drift}` (bd-d2l5, GATE-FSIM-DRIFT; command `check-lift-drift --upstream <path>
  [--head <rev>]`, run by `.github/workflows/lift-drift.yml` each quarter against frankensim main): each `[[lift]]`
  source's blob at its pinned commit against the same path at `<rev>` (default HEAD), from committed objects only.
  One JSON line per lift: `unchanged`, `changed` (lines added and deleted, the hunk ranges, the commits that touched
  it) or `missing`.
  It fails on a changed or missing source without a `registries/lift_drift.toml` decision (`relift` with its bead,
  or `no-op` with a reason) whose `upstream` commit holds the same blob, and on a decision naming no `[[lift]]` row.
- `check_stack_lock(root: &Path) -> Result<StackLockReport, Vec<String>>` and the pure
  `compare_lock_to_f1(lock, plan)` (BD-326.S1; command `check-stack-lock`, and `check-all` prints its line
  `{"check":"stack-lock","lock_ids","f1_ids",…}`): the `id` set of `franken-stack.lock` equals the `| FD-… |` rows
  from plan heading `### F.1 ` to the next heading (F.2's `-01` upstream asks are outside it). It fails on a missing
  or extra id, a lock id not ending `-00`, a malformed or repeated id, or an empty set.
- `check_scope(root: &Path) -> Result<(), Vec<String>>`
- `beads::{check_beads, evaluate}` (BD-324; command `check-beads`, which prints
  `{"check":"check-beads","rows","closed","distinct"}`): over `.beads/issues.jsonl`, a closed bead fails when its
  normalised reason is shared with another closed bead (R-SHARED), holds no `test:`/`record:`/`ci:`/`decision:`/
  `commit:` reference (R-TYPED), or cites one that does not resolve (R-RESOLVE). A `test:` reference's last `::`
  segment must be a `fn` anywhere under `crates/`; it is not looked up in the named crate, because tests have
  moved between crates since they were cited. A `record:` reference must be a file, and `ci:`/`commit:` are
  format-checked only. A bead also fails when it cites a `GATE-…` token with no `docs/delivery/<token>*.md`
  record holding a `Verdict: PASS` line (R-GATE), or when it is closed over a `blocks` dependency that is not
  closed (R-BLOCKED). Ten closes within 60 s is a warning (R-WINDOW). The JSONL reader is hand-written (no serde),
  and a malformed line is an error naming its number.
- `check_claims(root: &Path) -> Result<(), Vec<String>>`
- `check_det(root: &Path) -> Result<DetReport, Vec<String>>` (bd-hp2j.1; command `check-det`, and `check-all`
  prints its line `{"check":"check-det","files","headers_missing":0,"violations":0,"bitwise_files"}`): every
  production file of a `K`/`E` crate declares `//! Determinism: <class>` with a §29.2 class (`DET_CLASSES`); a
  `BitwisePortable` file calls none of `BANNED_TRANSCENDENTALS` as `.name(` or `f64::name(` outside comments and
  test code (`banned_transcendental_calls`); only `DET_EXEMPT` (`crates/fmine-core/src/det.rs`) is exempt; an
  unreadable file is an error.
- `check_oracle(root: &Path) -> Result<oracle::Report, Vec<String>>` and `oracle::{check, closure_findings,
  check_capture_records, record_errors}` (GATE-ORACLE-01, BD-013, §33.2; command `check-oracle`, and `check-all`
  runs `oracle::check` and prints `{"check":"gate-oracle-01","closure_packages","fixtures","oracles_in_closure":0}`):
  - `check_deps`' direct check, then `cargo tree -e normal,build` over the workspace without `xtask` and the
    member oracle crates: any `ORACLE_CRATES` package at any depth fails, naming the path. This catches an oracle
    reached through a crate outside the workspace, which the direct check cannot see. Dev-dependencies are not in
    the closure.
  - Every file under `fixtures/` needs `<file>.capture.toml` beside it with the nine §33.2 fields once each; its
    `outputs_digest` must be the fixture's sha256, and a record with no fixture fails. The form is
    `docs/templates/CAPTURE-RECORD.md`.
- `oracle::{capture, parse_capture_args, inputs_digest, host_platform, utc_date_today, Capture}` (BD-136; command
  `capture-fixture <fixture> --oracle --version --captured-by --licence --input <path>... [--note] [--replace] --
  <command>...`): runs the command from the root without a shell and writes its stdout as the fixture and the record
  beside it, with `platform` as `<arch>-<os>`, `date` as today in UTC, `inputs_digest` as one input's sha256 or the
  sha256 of the sorted `sha256:<hex> <path>` manifest of several, and the command as run. The record must pass
  `record_errors` before either file is written. It writes nothing when the command fails or prints nothing, when a
  value holds a quote, a backslash or a line break, or, without `--replace`, when the fixture or its record exists.
- `check_sources_registry(root: &Path) -> Result<Vec<String>, Vec<String>>` (`registry-check`; on success one
  `{"stage":"registry",…}` JSON log line per `registries/sources/*.toml`)
- `check_calendars(root: &Path) -> Result<String, Vec<String>>` (BD-029; `check-all` runs it and prints
  `{"check":"calendars","sites","tzdata"}`): `fmine_registry::calendars::load_site_calendars` must join
  `registries/calendars.toml` with the zone snapshot `registries/zones.toml`
- `refusals::{check_refusals, gen_refusals}` (BD-328; commands `check-refusals`, `gen-refusals`, and `check-all`
  runs the check and prints its line `{"check":"check-refusals","rows","active","proposed","reserved",…}`):
  - `gen_refusals` renders `registries/refusals.toml` (schema 2) into the `RefusalCode` enum and `CATALOG`, and
    writes the file only when its text changes.
  - `check_refusals` fails unless (e) the committed catalogue equals the rendered text; (a) every `active` code
    is in plan §8.4 and every §8.4 code is `active`; (b) each exit code follows the §30.1 family rule or a listed
    exception (504→7, 506→5), and no exception is unused; (c) every `FMINE-REF-nnn` token in `crates/**/*.rs`
    and `crates/*/CONTRACT.md` is a registry code, named `path:line`; (d) no fix text says "clamp".
- `check_all(root: &Path) -> Result<LocReport, Vec<String>>`
- `forecast::{rules_hash, record_hash, record_verify}` (BD-334.S1; commands `forecast-rules-hash <rules.toml>`,
  `forecast-record-hash <dir>`, `forecast-record-verify <dir>`): the IO around the pure `fmine_calib::record`.
  They write `rules.sha256` and `record.sha256`, and replace a different digest only while no proof (`.tsr`,
  `.ots`) stands beside it. `record_verify` takes the record's time only from a signed `record.tsr`
  verified against the compiled SHA-256 pins for the checked-in DigiCert or FreeTSA root PEM. A supplied
  `tsa_chain.pem` contributes only untrusted intermediates. Certificate validity is evaluated at the token's
  signed time, and a separately authenticated rules proof must strictly precede the record proof including
  both accuracy bounds. The shared `fmine_io::forecast::timestamp` implementation verifies one receipt snapshot
  and refuses changed root/intermediate fingerprints; xtask also matches the returned root fingerprint to
  its compiled pin. Its text parser is reexported as `proof_times_from_text`; duplicate or malformed fields
  and overflowing intervals refuse. `rules_hash` refuses malformed rules or an `OPEN` marker; the record is
  validated against the dates its rules state and the CSV against their universe and targets.
  The manifest's advisory registry-hash binding includes registered documents as well as workbooks.
  This authenticates the frozen declarations and detects conflicting registered digests; it does not prove
  actual input use or replay a generator. Source adapters separately verify the bodies they consume.
- DESIGN (BD-334/BD-068): `forecast::record_score` (command `forecast-record-score <dir>`) shares that verifier
  and scores the already loaded frozen manifest/CSV/rules against registered original-release Mine Stats
  Claims. It verifies time proofs before opening outcome workbooks. Production is the signed exact difference
  between original FY and original nine-month totals on the explicit 100% basis; AISC uses the original Q4
  QTD rate directly. Later comparatives never fill missing originals. Published numeric observations,
  including zero and source-status flags, retain their declared measurement meaning and source evidence.
  A Q2 ownership-label reference is used only when its exact registered bytes occur in the frozen manifest.
  Every frozen site/target remains visible, with unresolved and not-forecast states. The deterministic JSON
  assessment includes exact losses, observed and full-cohort coverage/loss bounds, the declared iid reference
  interval and rho sensitivity, source Claims/digests/locators, proof bounds and an assessment digest.
  It creates no forecast or timestamp and claims no calibration.
  `forecast::scoring::benchmarks` supplies the frozen issuer-guidance and persistence comparators to
  `fmine_calib::record::scoring::compare`. Guidance uses the latest agreed pre-cut document; same-day
  publication metadata needs exact manifest-bound bytes and an earlier declared retrieval, whose declared
  status is retained. No older guidance substitutes for an uncertain latest candidate. Original nine-month
  displayed production supplies the signed annual-guidance remainder; cost rates are never subtracted.
  The fixed original Q3 point and consecutive-quarter persistence pool are separate registered benchmark
  information sets, with manifest membership disclosed. Missing/unreadable required history refuses the
  comparator rather than shortening its pool. Printed zero and signed points remain valid; only historical
  ratio pairs carry the frozen positive-number restriction. Source Claims retain stored and displayed values.
  All three means and both differences use one identical complete member set, retaining every exclusion.
  Per-target arithmetic refusal retains that population and the other target; the final optional adapter
  boundary preserves unconditional forecast/outcome scores. `benchmark_assessment` serializes statuses,
  exact results, source/timing evidence and pool exclusions. Issuer guidance has no inferred nominal
  probability, and no same-information-set, skill or calibration claim is made.
- `forecast::persistence_baselines` (command `forecast-baseline`): each `run_rate_persistence` baseline of the
  FY2026 rules from the registered Mine Stats workbooks (read through `fmine-public` under the registered
  vocabulary; a workbook with a refused cell refuses): the pool's size, the pairs left out, the two ratio
  quantiles and, once the point quarter's workbook is registered, each live site's exact interval. It needs the
  corpus, so it never runs in CI (RISK-31).
  `forecast-baseline --as-of YYYY-MM-DD` adds `forecast::guidance::run`'s issuer-guidance baseline to that
  persistence output. It selects the latest registered two-reader annual guidance at the inclusive date cut;
  ties and pending newer readings stay unavailable. Production divides by its stated attributable share before
  subtracting the original Q3-release nine-month Claim; AISC retains its per-sold-ounce interval unchanged.
  All frozen sites and targets remain visible, including excluded sites and missing originals. Negative
  remainders are retained. The report binds rules, exact intervals, source Claims, PDF locators/digests/readers
  and availability evidence. This baseline uses workbook retrieval as its conservative availability bound,
  including when per-workbook publication dates are registered; earlier cuts refuse. No forecast, time proof or calibration
  claim is created. Actual registered-file integration controls are `tests/forecast_guidance_baseline.rs`.
  `scripts/forecast_record_e2e.sh` checks synthetic digest/mutation failures and verifies that a throwaway
  local TSA (`test_tsa`) cannot authenticate a public record. The committed rules proof is independently
  checked under the pinned public authority in unit tests, including wrong-digest refusal.
  `scripts/verify_rules_timestamp.sh` checks the rules' proofs with openssl/ots alone.
- `forecast::rules_order` (BD-334.S2; command `forecast-rules-order`): the commit adding
  `forecasts/fy2026/rules.toml` is a strict ancestor of every commit touching `forecasts/fy2026/forecasts.csv`;
  refuses a shallow clone, whose history cannot show the order.

## 5. Guards & Typed Refusals
Fails CI (`exit 2`) on any upward layer edge (`L-1`), banned dependency (`L-5`), `open_child_region` escalation outside `fmine-runtime` (`L-3`), lift provenance mismatch (`L-7`), undeclared module family (`§28.4`), bare `f64` physical quantity in `K`/`E` public signatures (`L-6`), a `K`/`E` file without a determinism class or a `std` transcendental in a `BitwisePortable` module (`PO-NUM-2`), a change adding LOC to a crate over its ceiling, or over planned without an approved amendment (`§28.4`), oracle crate in a production closure or a fixture without its capture record (`GATE-ORACLE-01`), or a `franken-stack.lock` id set that differs from plan Appendix F.1 (`BD-326.S1`).

## 6. Proof Obligations (`PO-*`)
Each check has a positive workspace test and a seeded negative violation unit test (`BD-003`..`BD-015`).

## 7. Failure Modes & Deterministic Fallbacks
Deterministic error collection across all files in lexicographical path order.

## 8. Gates & Kill Criteria (`GATE-*` / `KILL-*`)
- `GATE-LAYERS-00` (`BD-015`)
- `GATE-ORACLE-01` (`BD-013`)

## 9. Lifted Code & Foundation Dependencies
`fmine-registry` (the source registries and TOML subset), `fmine-calib` (the pure forecast record),
`fmine-io` (the shared offline RFC 3161 verifier) and `fmine-public` (the label-anchored Mine Stats reader,
for `forecast-baseline`); OpenSSL 3.x on `PATH` for `forecast-record-verify`, through the I/O adapter,
and `git` for `forecast-rules-order` only.

## 10. Non-Goals & Scope Boundary
Never linked by any production crate.
