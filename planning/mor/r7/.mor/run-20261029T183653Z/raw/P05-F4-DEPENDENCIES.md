RESULT_STATUS: MATERIAL_INCREMENT

# P05-F4-DEPENDENCIES — failure-mode and known-at analysis of dependencies, data access, data terms, timing and capacity (PU-5)

Operator: F4 failure-mode analysis with E3 temporal (known-at) reasoning. For each credible failure:
cause → consequence for M0–M2 → how it would be detected → the plan's control (cited) or its absence.
Every claim below is provisional and is meant for the independent claim audit.

## WHAT I CHECKED

Read in full: `PASS_BRIEF_COMMON.md`; `MOR_CASE.md` (Decision Contract, E673–E690, Pivotal Unknowns, Source
Access). Plan (HEAD `68e7091`, 8,902 lines): §27 (lines 6003–6218), §28.5 L-4/L-5 (6380–6406), §31 (6681–6870,
including §31.4 at 6819–6847 and §31.5 at 6849–6858), §34.3–§34.4, §35 (7188–7247), §36 (7249–7313), §37
(7317–7557), Appendix F (8392–8455), Appendix H row at 8540, §10.1–§10.2 (867–900), §21 public lane (4486–4510),
§39 rows BD-005, BD-067/068/069, BD-075, BD-257, BD-308. Also `franken-stack.lock`, `deny.toml`,
`rust-toolchain.toml`, `Cargo.toml`, `Cargo.lock`, `.gitignore`, `.github/workflows/ci.yml`, AGENTS.md lines
244–268, `registries/lifts.toml`, `docs/delivery/INT-AS-001.md` and `INT-NUM-001.md`, `crates/xtask/src/lib.rs`
(check_deps 300–352, check_caps 361–422, check_lifts 424–486, check_all 645–685, tests 844–886),
`crates/fmine-public/src/lib.rs` (descriptor list 36–, loader 224–265), `crates/fmine-io/src/zip_inflate.rs`
(411–), W1 (§1–§16, evidence rows E451–E499), W5 rows E615/E616, standing register E024/E658, the bead JSONL.

Sibling repositories (read-only git queries only: `git cat-file`, `git rev-parse`, `git show <rev>:<path>`,
`git grep <rev>`, `git tag --points-at`, `git ls-tree`):
- Every pin in `franken-stack.lock` resolves to a commit: asupersync `78b64636e` (= tag `v0.5.0`, Cargo version
  0.5.0); frankensim `20463525d…`; franken_accounting `41363acb`; frankenpandas `37bb94101`; franken_numpy
  `d32c4e04`; frankenjax `fa93fb77`; frankensympy `9d785d0`; frankengraphdb `b323bd17`; franken_networkx `9d710b1c`.
  frankensqlite has tag `v0.4.12`; frankensearch has `crates-v0.7.1`; frankenscipy has `v0.4.0`.
- crates.io API (`https://crates.io/api/v1/crates/<name>`, accessed 2026-10-29, user agent as the case requires):
  asupersync max 0.5.0 (license `LicenseRef-MIT-OpenAI-Anthropic-Rider`); franken-decision and franken-evidence
  0.5.0 (same LicenseRef); fsci-linalg and fsci-stats 0.4.0 (MIT); fsqlite 0.4.12, frankensearch 0.7.1,
  fastmcp-rust 0.13.0, fp-frame 0.6.0 (all "non-standard", i.e. `license-file`); ftui 0.9.0 (LicenseRef rider);
  the crate named **`fastmcp`** is an unrelated 0.0.0 Unlicense package.
- Lift provenance: a `python3 -I` script read each `registries/lifts.toml` row, ran
  `git -C frankensim show <commit>:<source>`, and compared sha256. **8 of 8 match** `sha256_at_lift`
  (det.rs, fs-blake3, fs-rand, fs-evidence, fs-eproc, fs-voi, fs-robust, fs-tropical). Every Wave 0/1 source path
  in §27.1 exists at the pin, as do `fs-qty`, `fs-airflow`, `fs-conduction` and `fs-solid`.
- APIs at the pins. asupersync v0.5.0 has `franken_evidence::EvidenceLedger` (lib.rs:62) and a public `render`
  module (`level0`…`level3`, `html`, `markdown`). It has `franken_decision::LossMatrix` (lib.rs:247), the trait
  `DecisionContract` (:679) and `DecisionAuditEntry` (:731), `Cx::open_child_region` (src/cx/cx.rs:4086),
  `cap::None` (a type alias, src/cx/cap.rs:102), `Cx::checkpoint` (cx.rs:2508), `franken_kernel::Budget` and
  `MacaroonKeyRing::verify` (src/cx/macaroon.rs:612). frankenscipy v0.4.0 has `trust_constr`, `least_squares`,
  `KDTree`, `brentq`, `SobolSampler`, `chi2`, `maximum_flow`, `linprog`, `milp` and `RbfInterpolator`.
- Suite-pin coherence: fgdb `b323bd17` pins asupersync by git rev `78b64636e…` and fnx `9d710b1c…`; FA `41363acb`
  pins the same, with a `[patch.crates-io]` that keeps one asupersync. fsqlite v0.4.12 and fsci v0.4.0 take
  asupersync `0.5.0` from crates.io (fsci only in `fsci-conformance`).
- Dependency hygiene against FrankenMining's `deny.toml`: asupersync v0.5.0 `[dependencies]` and `[features]`, and
  a reverse-dependency scan of its v0.5.0 `Cargo.lock` (copy at
  `…/scratchpad/as050.lock`). FA's `deny.toml` at `41363acb` for comparison.

Data and timing:
- Real workbook structure: `…/scratchpad/r7_repro/P05-F4-DEPENDENCIES/scripts/count_cells.py` (`python3 -I`,
  zipfile and regex, read-only). `Barrick_Q2_2026_Mine_Stats.xlsx`: 7 sheets, 2,297 numeric `<v>` cells.
  `Barrick_Q1_2025_AISC.xlsx`: 3 sheets, 1,146. `Barrick_Q4_2024_Mine_Stats.xlsx`: 7 sheets, 2,308.
- `git status -sb` → `main...origin/main [ahead 1]`, with `?? Cargo.lock`. `git ls-tree -r origin/main` holds 37
  files and no `crates/`, `.github/`, `deny.toml`, `franken-stack.lock` or `rust-toolchain.toml`. All of those,
  plus `zip_inflate.rs` and `docs/delivery/INT-*.md`, exist only in the unpushed commit `68e7091`.
- Web, accessed 2026-10-29:
  - Barrick legal page `https://www.barrick.com/English/legal/default.aspx`. curl got a Cloudflare 403, so WebFetch
    was used.
  - Barrick's date notice for FY2025 results, on the issuer CDN:
    `https://s25.q4cdn.com/322814910/files/doc_news/2026/01/Barrick-to-Report-Full-Year-and-Fourth-Quarter-2025-Results-on-February-5.pdf`
    (notice 7 Jan 2026; results 5 Feb 2026; search-result summary of the issuer page).
  - The GlobeNewswire notice of 16 Jan 2025 that FY2024 results would come on 12 Feb 2025
    (`https://manilatimes.net/2025/01/16/tmt-newswire/globenewswire/notice-of-release-of-barricks-fourth-quarter-and-full-year-2024-results/2039956/amp`;
    FACT[secondary]).
  - Searches for a January preliminary Q4 production release in 2025 and 2026 found none (bounded check).
- Not done: rerunning `fmine` on an empty workspace. The debug binary is not on this host (no
  `$TMPDIR/rch_target_fmine`; it was built on an rch worker). No cargo builds were run (E690).

## CANDIDATE CLAIMS

### P05-F4-DEPENDENCIES-C1 — the M0 data-access path cannot fail, so the 9 Nov 2026 DATA-GATE cannot detect anything

- **Type:** observation + causal.
- **Statement.** In the committed M0 code, a missing or unreadable Barrick workbook is silently replaced by an
  in-code workbook. E676's counts match that replacement for all 18 files, not the real workbooks. Two of the 18
  compiled-in inputs cannot exist anywhere. So the Q3 2026 DATA-GATE (an importer meeting an unseen workbook) and
  the CI run of GATE-BULL-01 would both pass whether or not any Barrick data is present. The plan has no text that
  forces a fail-closed result on a missing or hash-mismatched source.
- **Scope / horizon.** `fmine assure public --issuer barrick` at HEAD `68e7091`; M0 (Nov 2026–Jan 2027).
- **Evidence.**
  - E677, E678, E676.
  - New: `crates/fmine-public/src/lib.rs`.
    - Lines 237–243 read `match std::fs::read(&full_path) { Ok(bytes) => …, Err(_) => { let bytes = fmine_io::zip_inflate::build_canonical_barrick_xlsx_bytes(); … } }`.
    - The compiled-in list (36–) names `barrick-aisc-2024-q1.xlsx` … `barrick-mine-stats-2026-q2.xlsx`: 9
      periods × 2 kinds. It omits 2026Q1, has no Q3 2026 slot and carries no sha256.
  - New: `zip_inflate.rs` 411– builds 2 visible sheets plus 1 hidden sheet.
    - Sheet 1 has 39 numeric cells and 2,400 synthetic cells, with value `r*10+col` for rows 231–350 × 20 columns.
    - Sheet 2 has 2 numeric cells and 1 placeholder.
  - E676: 36 visible sheets, 43,938 numeric cells and 36 placeholders for 18 files. That is exactly 2, 2,441 and 2
    per file, and 2,441 = 39 + 2,400 + 2.
  - New: the real workbooks have 7 sheets (Mine Stats) or 3 (AISC), and 2,297 / 1,146 / 2,308 numeric cells
    (count_cells.py).
  - W1 line 108 and E499: "the IR feed lists no AISC xlsx for Q1 or Q2 2024". Yet the code requires
    `barrick-aisc-2024-q1.xlsx` and `-q2.xlsx`.
  - `.gitignore` lines 27–35 exclude `/corpus_of_input_docs/` and `*.xlsx`, so a CI runner never has the files.
- **Inference.**
  - Uniform per-file counts equal to the in-code workbook's, against real files with 3–7 sheets and different
    counts, imply that the E676 run read no real workbook. The arithmetic is exact.
  - The descriptor names match neither the local corpus nor Barrick's naming (`Barrick_QN_YYYY_Mine_Stats.xlsx`,
    and `AISC_QN_YYYY.xlsx` from 2025, W1 line 95). So on any machine the run takes the `Err(_)` path for all
    18 files.
  - A new Q3 2026 file is not in the list, so it is never read. Layout drift in it therefore cannot cause a failure.
- **Plan control, or its absence.**
  - §31.1 (6720–6722) and §21 (4507–4508) say each cell cites "the file's sha256".
  - §37.3 (7384–7386) says GATE-BULL-01 "must pass on" Q3 2026.
  - GATE-BULL-01 acceptance (7397–7402) requires "zero uncited numbers" and byte-identical reruns. Both are
    satisfied by a deterministic substitute.
  - No line says a missing, unregistered or hash-mismatched source must be refused.
  - Appendix F (8398–8416) has foundation rows only for software siblings. The single most critical external
    dependency of M0, the issuer's IR host and its files, has no FD row, pin or fallback rule.
  - §27 line 6017–6018 ("each `FD-` row has a fallback that keeps the product shippable and honest") is the
    principle. The code's fallback keeps M0 shippable but not honest.
- **Alternative / defeater check.**
  - (a) A machine with files under the 18 exact names could read real data. Defeated for 2 of the 18, which were
    never published (E499), and no such names exist locally (E677).
  - (b) The fallback might be test-only. Defeated: it sits in `load_from_workspace`, the production path, not under
    `#[cfg(test)]`.
  - (c) Overlap with PU-1 passes, which will likely report E677–E679. The increment here is that the plan's own
    timing control (the DATA-GATE) and its CI evaluation are structurally unable to fire. That is a PU-5 finding.
- **Resolution / reversal.** Reversed if a run on a workspace without the corpus exits nonzero with a typed refusal,
  or if a reproduction shows the E676 counts arising from real files.
- **Materiality.** High. Per the loss function, this is a gate that passes vacuously on substituted inputs. If shown
  to Barrick management as "18 workbooks, 43,992 cells verified", it becomes a false statement. 98.3% of the
  counted numeric cells (2,400 of 2,441 per file) are synthetic filler.
- **Uncertainty basis.** Deductive from code and counts. I could not rerun the binary because it is not on this
  host.

### P05-F4-DEPENDENCIES-C2 — recommendation: make the issuer data source a pinned, fail-closed dependency (FD row + refusal + CI fetch)

- **Type:** recommendation. **Supports:** C1.
- **Plan edits.**
  - **§37.3 M0 (after line 7386) and GATE-BULL-01 (7397–7402; mirror in the §21 table at 4590).** Add: "Every input
    workbook is listed in a source registry with issuer URL, retrieval time and sha256. `fmine assure public`
    refuses with a typed refusal and nonzero exit when any registered file is absent or its sha256 differs. No
    built-in substitute exists outside `#[cfg(test)]`. GATE-BULL-01 is evaluated only on a run whose input-manifest
    hashes equal the registry, and the run record names that manifest. The Q3 2026 workbook enters by adding a
    registry row from the IR feed, not by a code change."
  - **§31.1 (after 6722).** Add the same refusal rule for the public lane.
  - **Appendix F.1.** Add `FD-DATA-BARRICK-00`:
    - posture: fetch by URL and pin by sha256; bytes never committed (`.gitignore` 27);
    - admission: registry-check;
    - fallback: refuse the affected site-quarters as `NOT_AVAILABLE`, never substitute.
  - **CI.** Either a job that fetches the registered URLs and verifies sha256 before `assure` (fetching is not
    redistribution; see C9), or a statement that GATE-BULL-01 is evaluated only on a named machine holding the
    corpus, with that run's manifest committed.
- **Cost.** Small: a registry file, about 20 lines of loader change, and one CI step. W1's `w1/manifest.tsv` already
  holds URLs and timestamps but lives in a scratch folder (W1 line 112); it can seed the registry.
- **Reversibility.** Full.
- **Acceptance test.**
  1. Run on an empty workspace: exit is nonzero with a refusal naming each missing file.
  2. Flip one byte of one workbook: refusal naming that file.
  3. Run on the ten registered real quarters: the per-file sheet counts equal the real files (7 or 3), not 2.

### P05-F4-DEPENDENCIES-C3 — the committed `deny.toml` would reject the M1 substrate if enforced, and nothing enforces it

- **Type:** deductive + observation.
- **Statement.**
  - The committed dependency constitution contradicts the plan's own L-5 rule.
  - Run as written, it would fail the first build that admits asupersync `=0.5.0`, on four counts: licence, duplicate
    versions, `time` via `tls`, and missing L-5 bans.
  - It is also not run anywhere. The controls the plan relies on to stop a second asupersync, the one-package-id
    rule and the transitive ban inversion, do not exist at HEAD.
- **Scope / horizon.** HEAD `68e7091`. Becomes live at M1, when `fmine-runtime`/`-ledger`/`-sync` consume asupersync
  (GATE-STORE-01 and GATE-SYNC-01 are M1 gates, §37.3 line 7417).
- **Evidence (all new except E683).**
  1. **Licences.**
     - `deny.toml` `allow = ["MIT","Apache-2.0","BSD-2-Clause","BSD-3-Clause","ISC","Unicode-3.0","Zlib"]`, with
       no `exceptions` and no `clarify`.
     - asupersync v0.5.0 `Cargo.toml` line 25: `license = "LicenseRef-MIT-OpenAI-Anthropic-Rider"`. The same holds
       for franken-decision, franken-evidence and ftui on crates.io.
     - fsqlite, frankensearch, fastmcp-rust and fp-* declare only `license-file` (frankensqlite's Cargo.toml says
       the rider is "not an SPDX license expression").
     - FA's `deny.toml` at `41363acb` (lines 23–80) handles this with per-crate `exceptions` allowing
       `LicenseRef-MIT-OpenAI-Anthropic-Rider` for asupersync, asupersync-macros and every fgdb-* crate.
  2. **Duplicate versions.**
     - `deny.toml` sets `multiple-versions = "deny"` with no `skip`.
     - asupersync v0.5.0 non-optionally depends on `nkeys = "0.4.5"`.
     - Per its v0.5.0 `Cargo.lock`, nkeys pulls `getrandom 0.2.17` and `rand 0.8.7` (`rand_core 0.6.4`), and its
       `ed25519-dalek 2.2.0` pulls `sha2 0.10.9` (`digest 0.10.7`).
     - asupersync itself uses `getrandom 0.4.3`, `sha2 0.11.0` (`digest 0.11.3`) and, via `getrandom 0.4.3`,
       `rand_core 0.10.1`.
  3. **`time` via `tls`.**
     - `deny.toml` bans `{ name = "time" }` and `{ name = "chrono" }` graph-wide.
     - asupersync's `tls` feature is `["dep:rustls", …, "dep:x509-parser"]` (v0.5.0 Cargo.toml line 248), and
       `x509-parser → time` (lockfile).
     - §31.4(d) (line 6831) requires ATP and sync to run "only over QUIC/TLS".
     - L-5 (6395–6398) says the ban is for direct dependencies and public APIs only, with "a per-foundation
       allow-list for exactly those paths". Appendix H line 8540 records this as "Fixed". The committed file reverts
       the fix.
  4. **Missing L-5 bans.** L-5 also bans petgraph, fastembed and tantivy (6394, 6399–6400). None is in
     `deny.toml`.
  5. **Not enforced.**
     - CI runs only `xtask check-all`, `cargo check` and `cargo test` (E683; `ci.yml` lines 20–24), with no
       `cargo deny`.
     - `xtask::check_deps` (lib.rs 300–352) parses each member's direct `[dependencies]` names against the deny
       list. It does no transitive (`cargo tree`/metadata) inversion, no licence check and no one-package-id check.
     - That contradicts §31.5 (6851–6853: "`deny.toml` and the L-5 bans are CI gates. `xtask check-deps` inverts
       `cargo tree` …"), §27.5 (6189: the one-package-id check "is a CI gate, not a convention") and L-5 line 6403.
  6. **BD-005 closed early.** BD-005 ("`xtask check-deps`: deny.toml, banned list (L-5), one-package-id rule";
     acceptance "A test crate pulling tokio fails CI", plan line 7623) is closed. The xtask tests (lib.rs 844–886)
     contain no tokio or second-asupersync seed.
- **Inference.**
  - Once M1 adds asupersync, two outcomes are possible:
    - (i) someone wires `cargo deny`, and M1 is blocked on four independent counts that have nothing to do with
      product risk; or
    - (ii) the deny file stays decorative, and the very failure §27.5 names (asupersync arriving from both a git
      source and crates.io — two package ids, two `Cx` types) passes CI silently.
  - Path (ii) is reachable: fsqlite and fsci take asupersync from crates.io, while fgdb and FA take it by git rev.
    Without the plan's `[patch.crates-io]` (§28.6 line 6412; absent from `Cargo.toml`), Cargo links both.
- **Alternative / defeater check.**
  - "The duplicates are optional-feature only." Defeated: nkeys and asupersync's own `getrandom`/`sha2` are
    non-optional.
  - "cargo-deny filters by target." With no `[graph] targets` set, cargo-deny considers all targets, so this cannot
    help. Either way the getrandom and sha2 pairs are target-independent.
  - The exact closure FrankenMining would resolve may differ with feature choices. I did not run `cargo tree` (no
    builds), so the duplicate list is from asupersync's own lockfile. The two non-optional pairs above are
    sufficient.
- **Resolution / reversal.** Reversed if `cargo deny check` on a scratch workspace with
  `asupersync = { version = "=0.5.0", default-features = false }` and this `deny.toml` passes.
- **Materiality.** High-moderate. It creates a silent M1 blocker or a vacuous gate on the exact risk RISK-09 and
  RISK-18 rely on. The fix is cheap and the commit is unpushed.
- **Uncertainty basis.** Deductive from lockfile and configs. cargo-deny behaviour assumed per its documented
  semantics.

### P05-F4-DEPENDENCIES-C4 — recommendation: bring `deny.toml` and `check-deps` to L-5 before M1 and put them in CI

- **Type:** recommendation. **Supports:** C3.
- **Edits to the code and config (not the plan):**
  - `deny.toml`:
    - `[[licenses.exceptions]]` for each rider crate (copy FA's list);
    - `[[licenses.clarify]]` for `license-file` siblings;
    - `wrappers` on `time`/`chrono` naming exactly the foundation paths L-5 lists;
    - a `skip` list for known duplicate pairs, each with a reason;
    - add petgraph, fastembed and tantivy to the bans.
  - Add `cargo deny check` to CI.
  - Re-implement `check-deps` over `cargo metadata` (resolved graph), including one package id each for
    `asupersync`, `fgdb-types` and `fnx-classes`.
  - Add the seeded failing tests BD-005 promised.
- **Plan edits.**
  - §31.5 lines 6851–6853 must say the gate is "`cargo deny check` plus `xtask check-deps` over the resolved
    `cargo metadata` graph, run in CI on every push, with seeded failing fixtures (tokio; two asupersync sources)".
  - §28.5 L-5 (6389–6403) must name the cargo-deny mechanism (`wrappers`, `exceptions`) that implements the
    per-foundation allow-list.
  - Reopen BD-005 until its acceptance test exists.
- **Cost.** Under a day of agent work.
- **Reversibility.** Full.
- **Acceptance test.** In CI:
  - a scratch member depending on tokio fails;
  - a member set pulling asupersync by both git rev and crates.io without `[patch.crates-io]` fails;
  - `asupersync =0.5.0, default-features = false` with the patch passes `cargo deny check` and `check-deps`.

### P05-F4-DEPENDENCIES-C5 — the INT-AS-001 and INT-NUM-001 admissions are recorded PASS before any foundation is in the graph

- **Type:** observation.
- **Statement.** The plan's control for foundation failure is "an `INT-*` admission gate before its first
  production use" (§27 line 6015). Both admission records say **PASS** and their beads are closed, yet no foundation
  is in the build graph. The records assert facts the graph and CI do not show.
- **Evidence.**
  - `Cargo.lock` (working tree; untracked at HEAD per `git status`) lists only `fmine-*` and `xtask`. No member
    declares an external dependency. `Cargo.toml` has no `[patch.crates-io]`.
  - `docs/delivery/INT-NUM-001.md` item 1: "`fsci-* =0.4.0` is consumed with `default-features = false`". Item 2:
    bit-identical digests "across `x86_64-linux`, `aarch64-darwin`, and `wasm32`".
  - CI covers ubuntu-latest and macos-14 only, with no wasm32 job (E683). CI has never run on this code because
    `.github/` is absent from `origin/main`.
  - BD-257's acceptance is "Admission record in CELL-DEC" and its title includes "`[patch.crates-io]` single `Cx`".
    BD-308's title is "closures pass L-5 and the one-package-id rule; oracle parity on every consumed kernel". Both
    are closed with the generic reason "Implemented and verified by cargo test --workspace and xtask check-all".
  - `.gitignore` line 7: "Keep Cargo.lock tracked". It is untracked at HEAD.
- **Inference.**
  - When fsci, fp or asupersync first enter (M1), the ledger will already show them admitted.
  - The FD-row fallback logic (switch to the fallback if admission fails) can therefore never trigger.
  - This defeats the §37.4 firewall's admission step for the "Lifted / consumed" column (line 7554).
- **Alternative / defeater check.** The records could be read as desk checks of upstream facts. Part of INT-AS-001
  item 2 is true: the APIs exist at v0.5.0 (verified above). But the records label the verdict "PASS" against
  admission criteria (closure, one package id, parity) that were not measured. NONE_IDENTIFIED_AFTER_BOUNDED_CHECK
  for a reading under which the closure claims hold.
- **Resolution / reversal.** Reversed by a committed `Cargo.lock` containing the foundations plus CI logs of
  `cargo deny`, one-package-id and per-target determinism tests that match the records.
- **Materiality.** Moderate-high. A dependency control that has passed vacuously cannot catch the M1 failure it
  exists for.
- **Uncertainty basis.** Direct observation.

### P05-F4-DEPENDENCIES-C6 — recommendation: make INT-* admission records evidence-bound

- **Type:** recommendation. **Supports:** C5, C3.
- **Plan edit, §27 line 6015 (and §34.2 INT rows).** Add: "An `INT-*` record is valid only if it cites the
  committed `Cargo.lock` hash in which the foundation appears, the `cargo deny` and one-package-id output for that
  lock, and the CI job ids of the per-target tests it relies on. A record made before the foundation is in the graph
  is a pre-admission desk check and does not satisfy the gate."
- **Repository actions.**
  - Reopen BD-257 and BD-308, or relabel both records "PRE-ADMISSION (desk check)".
  - Strike the wasm32 bit-identity claim until a wasm32 test job exists.
  - Commit `Cargo.lock`.
- **Cost.** Trivial.
- **Reversibility.** Full.
- **Acceptance test.** `docs/delivery/INT-*.md` each name a lock hash. `git show <hash>:Cargo.lock` contains the
  foundation. The cited CI job ids exist.

### P05-F4-DEPENDENCIES-C7 — the FY2026 forecast as specified is mostly a nowcast at any feasible hash date, and its resolution basis is exposed to the NewCo IPO

- **Type:** causal + forecast (known-at).
- **Statement.**
  - Under the plan's wording, FY2026 per-site forecasts are hashed "before the February 2027 release"
    (7393–7394, 4495–4496, BD-067/BD-068). At any feasible hash date after the 9 Nov 2026 DATA-GATE, Q1–Q3 2026
    actuals are public, so roughly three-quarters of each FY2026 site value is already observed.
  - Scored "whatever they show" against FY outcomes, with no declared information cut-off, horizon, baseline,
    ownership basis or timestamp authority, the result will look good mechanically and carry little evidence of
    skill.
  - The resolution window also overlaps the North American NewCo IPO, which can change attributable shares and
    segments, and whose prospectus may publish Q4 information before the hash.
- **Scope / horizon.** Hash window 9 Nov 2026 → about 5–12 Feb 2027. Scoring in M1 (Feb–Apr 2027).
- **Evidence.**
  - E454: Q3 2026 results on 9 Nov 2026.
  - New (web, accessed 2026-10-29):
    - FY2025 results were 5 Feb 2026, announced 7 Jan 2026 (issuer CDN notice above);
    - FY2024 results were 12 Feb 2025, announced 16 Jan 2025 (FACT[secondary]);
    - no January preliminary Q4 production release was found for 2025 or 2026 (bounded search).
  - §10.1 (875–877) requires each registration to store "the horizon" and "the resolution source". §10.2 scores
    with CRPS/coverage (892) and races models with `PairwiseRace` (897). No baseline is named for the public-lane
    FY forecast.
  - BD-068: "externally timestamp", with no mechanism named anywhere in the plan (grep for RFC 3161,
    OpenTimestamps, TSA, "external timestamp": no hits beyond line 7711).
  - E615/E616/E658: NewCo (NGM and Pueblo Viejo JV interests plus Fourmile) minority IPO officially targeted for
    end-2026, and "may slide into January" (FACT[U]).
  - E456: Mine Stats column B is 100% basis and column C attributable.
- **Inference.**
  - FY2026 production is additive across quarters. A forecast made with Q1–Q3 known has its uncertainty
    concentrated in Q4. FY AISC per ounce is a ratio dominated by observed quarters.
  - Interval coverage and CRPS on FY values therefore mostly measure the Q4 component, divided by about 4.
    Reporting them to an executive committee as forecast skill would overstate it (BET-02, line 294, depends on
    this record).
  - Known-at hazards inside the window:
    - Q4 metal prices are fully known by 31 Dec. That is legitimate information but must be declared.
    - The release-date notice comes about 4 weeks before release, so the hard deadline is known only in January.
    - A NewCo prospectus filed in Dec 2026 or Jan 2027 may contain preliminary Q4 figures for NGM, PV or Fourmile.
      HYPOTHESIS: recent-developments disclosure of the latest quarter is common practice, but its content here is
      unknown.
  - If NewCo closes in the quarter, attributable (column C) figures for NA sites may be redefined, while 100%-basis
    figures (column B) are invariant to the ownership change.
- **Plan control, or its absence.**
  - F3/RISK-03 and §31.1 cover MNPI contamination.
  - The region map is versioned by file (W1 §13).
  - Nothing specifies the information cut-off, a baseline, the ownership basis of the forecast target, or the
    timestamp authority.
- **Alternative / defeater check.**
  - (a) Hash on H1-only data, before 9 Nov. That makes a genuine two-quarter forecast, but it requires the importer
    on real files within 11 days, and C1 shows it does not read them.
  - (b) Barrick may issue no Q4 preliminary information, so leakage is nil. This weakens the leakage half but not
    the nowcast and basis halves.
  - (c) The IPO may slip past FY2026 resolution. That removes the attributable risk only.
- **Resolution / reversal.** Reversed if the plan's forecast registration already fixes the cut-off, target basis,
  baseline and timestamp authority somewhere I did not find (searched §10, §21, §31.1, §37.3, §39).
- **Materiality.** Moderate-high. This is the first scored-forecast claim the program will show Barrick
  management, the loss function's costliest channel.
- **Uncertainty basis.**
  - Release-date precedents: two years, one secondary source.
  - IPO timing: FACT[U].
  - Prospectus content: HYPOTHESIS.

### P05-F4-DEPENDENCIES-C8 — recommendation: pre-register the public-lane forecast so that it is a test, not a nowcast

- **Type:** recommendation. **Supports:** C7, C1.
- **Plan edits: §37.3 lines 7393–7394, §21 lines 4495–4496, BD-067/BD-068 (7710–7711).** The text must assert the
  following.
  1. **Cut-off.** An information cut-off date and an input manifest listing every document used, with publication
     date and sha256.
  2. **Deadline.** An internal hash deadline fixed now, e.g. ≤ 15 Dec 2026. That is 36 days after the 9 Nov data,
     ahead of any January prospectus and the mid-January date notice. "Before the release" is a backstop only.
  3. **Basis.** Targets on the 100%-basis per-site fields (E456 column B), with a pre-written rule for
     re-segmentation or ownership change at FY2026 resolution.
  4. **Scoring.** Score on the Q4-implied quantity (FY − published YTD Q3). Race the forecast against two
     pre-registered naive baselines with `PairwiseRace` or the interval score:
     - YTD run-rate;
     - Barrick's own FY2026 guidance as last stated before the cut-off.
     "Reported whatever they show" then means the skill relative to the baselines.
  5. **Timestamp.** The named timestamp mechanism: the forecast-set hash pushed to the public repository, plus an
     RFC 3161 TSA receipt or an OpenTimestamps proof stored with it.
- **Design option.** If, and only if, C2's fail-closed loader exists and reads sha256-pinned real H1 files, also hash
  a vintage V0 conditioned on H1 2026 before 06:00 ET on 9 Nov 2026. That yields a genuine two-quarter-ahead record
  that resolves partly on 9 Nov and fully in February. Otherwise skip V0. A forecast built on substituted inputs is
  worse than none.
- **Timing verdict on the 9 Nov DATA-GATE.**
  - From 9 Nov 2026 to the M0 end (31 Jan 2027) is 83 days. To a 15 Dec hash deadline is 36 days.
  - That is enough calendar time if the importer reads real files, Barrick posts the workbook with the release, and
    the layout drift is of the kinds W1 already catalogued.
  - It is not enough if the importer must first be rebuilt to read real files (C1). That rebuild is the critical
    path, not the calendar.
- **Cost.** Small: text plus a baseline computation.
- **Reversibility.** Full until the hash. Irreversible after.
- **Acceptance test.**
  - The hashed forecast record contains the cut-off, the manifest, the basis, the baselines and a TSA/OTS proof
    whose time precedes the release.
  - The M1 scoring report shows skill against both baselines, on the Q4-implied quantity.

### P05-F4-DEPENDENCIES-C9 — question: may derived Barrick data be committed to a public repository? The plan has no decision, and the unpushed commit already embeds some

- **Type:** question (with an observation base).
- **Statement.**
  - Barrick's website terms restrict copying and distribution of site material outside non-commercial, personal or
    educational use where the material is not modified. Other uses need Barrick's "prior written approval".
  - The project's policy treats issuer-website workbooks as an approved fixture source.
  - The unpushed M0 commit embeds a reconstructed workbook carrying Barrick-published values at their source cell
    coordinates.
  - The M0 kill fallback turns the engine into "test fixtures", which presupposes committing derived Barrick data.
  - No plan line, RISK row or OPEN item addresses issuer data terms. RISK-17 covers only the code licence rider.
- **Evidence.**
  - New: `https://www.barrick.com/English/legal/default.aspx` (accessed 2026-10-29), "Copyright and Trademarks":
    - "Certain material found on this Website is protected by copyright";
    - "Except for non-commercial, personal or educational purposes, where the material is not modified";
    - "materials may not be copied, reproduced, modified, uploaded, posted, transmitted, or distributed in any way";
    - other uses need "Barrick's prior written approval";
    - the page contains no governing-law clause.
  - AGENTS.md line 248; `.gitignore` lines 40–41 ("fixtures are synthetic (fmine-gen) or built from public filings
    only").
  - `zip_inflate.rs` 411–: about 41 values identical to published Barrick cells (e.g. N218 = 1484, N228 = 3973,
    J208 = 2582), placed at Barrick's coordinates in sheets named "Gold AISC" and "Copper AISC".
  - Plan line 7403–7404 / 4590 (fixtures fallback).
  - grep of plan, AGENTS and README for "terms of use", "copyright", "redistribut": no hits.
  - E673 and `git ls-tree origin/main`: none of this code is public yet.
- **Inference.** Whether isolated numeric facts and small extracts are protected, and whether the terms bind, are
  legal questions I cannot settle. What the evidence does establish:
  - (i) the project may be used commercially, so the "non-commercial" exception is doubtful on its face;
  - (ii) the plan has no recorded decision;
  - (iii) the decision is still cheap, because `68e7091` is unpushed and E673 notes a push is irreversible.
- **Alternative / defeater check.**
  - Fair dealing for research or review, and the facts/expression distinction, may well permit locators, short
    quotations and computed checks. That is why this is a question, not a finding of breach.
  - The W1 and C1 notes already public quote short cell extracts with commentary. I do not claim those are
    infringing.
  - Written approval from Barrick IR is a plausible, cheap route.
- **Resolution.** Owner and counsel decide. Or Barrick IR grants written approval for derived fixtures in a public
  repository.
- **Materiality.** Moderate. If wrong, it surfaces at the worst moment, in front of the issuer whose data it
  uses. It is cheap to resolve.
- **Uncertainty basis.** Terms text retrieved through WebFetch (short verbatim fragments). Legal effect unknown.

### P05-F4-DEPENDENCIES-C10 — recommendation: add an issuer-data-terms risk with a fail-closed default, decided before pushing `68e7091`

- **Type:** recommendation. **Supports:** C9, C2.
- **Plan edits.**
  - **§35.2.** Add `RISK-31 Issuer data terms` (owner and counsel; deadline: before the next push to the public
    repository).
  - **§36.2.** Add the matching OPEN row. Default until decided: "commit only locators (`file:Sheet!Cell`), source
    URLs, sha256 and computed check results; never a reconstructed workbook or bulk extracted values; fixtures that
    need values are generated synthetically (fmine-gen) or fetched at test time from the pinned URL."
  - **§37.3 kill fallback (7403–7404).** Read "test fixtures (locator-and-hash form, §31.1)".
  - **AGENTS.md line 248 and `.gitignore` 40–41.** Align with the same rule.
  - Seek Barrick IR's written approval in parallel.
- **Cost.** Low.
- **Reversibility.** Full now. Not after a push.
- **Acceptance test.** `git grep` over the to-be-pushed tree finds no Barrick-sourced numeric literal outside
  `planning/` prose quotations. The in-code workbook is synthetic or removed.

### P05-F4-DEPENDENCIES-C11 — question: is the owner a public-lane author during the forecast window?

- **Type:** question.
- **Statement.**
  - §31.1 line 6723 says "No public-lane author holds private access", and BD-069 (open) requires access
    attestations.
  - The owner directs the agents that will author the FY2026 public forecasts. Whether the owner holds any
    private Barrick access is not recorded.
  - The hash window (Nov 2026–Jan 2027) coincides with Barrick's pre-results period and the NewCo IPO process
    (E615, E616).
  - Is the owner inside the lane? If yes, the forecast set needs the owner's attestation that no non-public Q4 2026
    or NewCo information was received before the cut-off. Otherwise the affected sites must be labelled
    contaminated.
- **Evidence.** §31.1 6719–6724; BD-069 line 7712; F3/RISK-03 ("fatal impact", 7199, 7220); E615/E616; Decision
  Contract.
- **Alternative.** The owner may have had no such contact. The attestation then costs nothing.
- **Materiality.** Low probability, but the plan rates the impact fatal. Resolving it costs one signed line in BD-069
  before BD-068.

### P05-F4-DEPENDENCIES-C12 — question: who are the "two finance reviewers" on the M0 kill gate, and by when?

- **Type:** question (team capacity).
- **Statement.**
  - The M0 kill criterion requires two finance reviewers' ratings (7403, 4590).
  - BD-075 is open and is on the 29-bead critical path (line 7601). No plan line names the reviewers, their
    independence from the owner and agents, or a recruitment deadline.
  - Without them the kill branch cannot fire, so GATE-BULL-01 can only pass or fail on reproduction.
- **Materiality.** Low-moderate. It is a vacuous-kill risk on the M0 gate.
- **Resolution.** Name the reviewers and a date (e.g., M0 month 1) in §37.3 or §34.3.

### P05-F4-DEPENDENCIES-C13 — minor dependency-ledger defects (traceability), with checked nulls

- **Type:** observation.
- **Statement / evidence.**
  - (a) **FD id collision.** `franken-stack.lock` labels the foundation rows `FD-AS-01`, `FD-FSIM-01`, `FD-FA-01`,
    `FD-FSCI-01`, `FD-FP-01`, `FD-FNP-01`, `FD-FJ-01`, `FD-FSYM-01`, `FD-FGDB-01`, `FD-FNX-01` and so on. Appendix
    F.1 says foundations are `FD-…-00` (8398–8416). F.2 uses the `-01` ids for upstream asks (e.g. `FD-AS-01` =
    "Protected third-party caveats", 8447; `FD-FA-01` = metal units, 8425). Line 8396 makes the lock authoritative,
    so an agent resolving "FD-FA-01" gets two different objects. This conflicts with §0.3's stable-id rule.
  - (b) **Package name.** The lock row `name = "fastmcp", pin = "=0.13.0"` names a crates.io package that is an
    unrelated 0.0.0 Unlicense crate. The suite's package is `fastmcp-rust` 0.13.0. This is low risk because
    `=0.13.0` cannot resolve to it and wildcards are denied, but the row should name the package.
  - (c) **Lift headers.** All eight say "licence MIT" (template in `registries/lifts.toml` line 3). frankensim's
    LICENSE at the pin is byte-identical to FrankenMining's MIT + OpenAI/Anthropic rider. That is compatible, but
    the header misstates the licence the plan's header contract asks for (§27.1, 6050–6051).
  - (d) **frankenpandas.** Plan line 6113 says "git `37bb94101` (or crates.io `0.6.0` if published)". fp-frame 0.6.0
    is now published on crates.io, so the condition can be resolved.
- **Checked nulls.**
  - Every pinned revision exists. All 8 lift sha256 values match. Every plan-named API exists at its pin. Suite pins
    are mutually consistent.
  - Defect (b) of §31.4 is present at v0.5.0 exactly as described: `verify` compares against an all-zero sentinel
    when `retired` is `None`. The plan's mitigation addresses it.
- **Materiality.** Low. Cheap text and lock fixes:
  - renumber the lock to `-00`;
  - name `fastmcp-rust`;
  - change the header licence id to `LicenseRef-MIT-OpenAI-Anthropic-Rider`;
  - resolve the fp pin.

## NULL OR BLOCKED RESULT (checked nulls within PU-5)

- **Site data access for M2 (F2/RISK-02).** Controlled. §34.4 GATE-DATA-01…03 with dated thresholds, an
  evidence-based p = 0.3 and M2′ resourced from M1 as the modal path. No increment.
- **Suite non-convergence and frankensim (F9/RISK-09, KILL-3).** Controlled for M0–M2. §37.4 assigns no frankensim,
  FA or fgdb dependency to M0–M2, and the lifts verify at the pin. FA's HEAD has moved again (`a68ef963`) but FA is
  off the M0–M2 path. No increment.
- **asupersync 0.5.0 defects (RISK-18).** The plan's description matches the v0.5.0 source for (a) and (b), and the
  mitigations are adequate as written. The only gap is enforcement of the closure rules (C3/C5).
- **Toolchain.** The `nightly-2026-08-31` pin matches §27.5. OPEN-SUITE-3 (fsym) is build-time only. No M0–M2
  impact found.
- **Build host disk (E690).** It affects local throughput, not plan control. GitHub CI is independent of it. Not
  treated as a reversal.
- **Pivotal unknowns left open.**
  - Whether Barrick posts the Q3 2026 workbooks with the 9 Nov release (E455 shows them absent as of 29 Oct, as
    expected).
  - What a NewCo prospectus would disclose about Q4 2026.
  - The legal effect of Barrick's terms on derived numeric fixtures.
