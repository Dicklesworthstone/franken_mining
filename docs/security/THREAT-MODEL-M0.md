# Threat model: the M0 `fmine` binary

Part of the M0 security pack (BD-073, plan §37 and §39). The other parts are the bill of materials (`cargo run -p xtask -- sbom`) and the offline runs recorded on bd-073. This document covers what the binary does today, as of 2026-11-01. Plan §31 describes the target design: MNPI walls, venture walls, residency and OT separation. None of that is in this binary, and this document does not claim it.

**Scope of the tree.** This document was checked against fa408dd. That tree merges `origin/main` at 7752612 (merge f27740a), which added the forecast journal, `assure issue`, `reliance reissue`, `reliance export`, `recall` and the `--output` issuance path of `assure public`. An earlier version, checked against c2466d6 before that merge, said issued artifacts take the process umask. That was wrong for that tree too: they are created with mode 0600.

Each control below says how it is known: **enforced** (a check or the compiler fails without it), **tested** (a named test), **observed** (a recorded run) or **by search** (true of the source at this date, with nothing to stop a regression).

## 1. What the binary is

- **One executable, `fmine`**, built from the `fmine-cli` crate. The released copy comes from GitHub Actions, built from a pushed commit (`.github/workflows/blind-binary.yml`). `repro/barrick/blind_record.json` records the run, commit and sha256 of the Q3 2026 copy (E854). The same workflow uploads the bill of materials, which carries the binary's sha256.
- **Dependencies.** The 15 workspace crates of its closure, and no crate from crates.io or git (the SBOM; `Cargo.lock` has no `source` line). The toolchain is pinned to `nightly-2026-08-31` (`rust-toolchain.toml`), and builds use `--locked`.
- **Verbs.** `assure public|challenges|issue`, `guidance completion`, `track-record`, `recovery gap`, `lint`, `forecast assess`, `forecast journal init|register|freeze|outcome|assess|inspect`, `claim show|why|trace`, `recall`, `doctor`, `robot-docs` and the `reliance` family (`init`, `issue`, `reissue`, `check`, `reuse`, `export`, `withdraw`, `supersede`, `refresh`, `recall`). `repair plan`, `repair apply` and `robot schema` are registered in `registries/verbs.toml` but not implemented: they return a usage error (exit 2; observed at fa408dd).
- **Output.** Each run prints one JSON envelope on stdout and exits with a code from the §30.1 dictionary. If stdout cannot be written, it exits 5 with one line on stderr (`crates/fmine-cli/src/main.rs`).
- **Workspace.** The binary reads its registries and source files under one workspace directory: the current directory, or `--workspace <dir>`.

## 2. Assets

| Asset | Why it matters |
|---|---|
| The verdicts, state roots and anchors the binary prints | A reviewer or board relies on them; a wrong one that looks right is the worst outcome |
| The registered source bytes and the registries that pin them (`registries/sources/*.toml`, `registries/transcriptions/`) | They decide which bytes are evidence |
| The reliance journal and the artifacts issued under it | They record what was released and whether it may still be relied on |
| The host | Other files on the machine, its network position and its availability |

## 3. Inputs and how each is handled

| Input | Handling | How known |
|---|---|---|
| Workbooks (`.xlsx`) | Size and sha256 are checked against the source registry **before** parsing. A missing file refuses (FMINE-REF-506, exit 5), and an altered one refuses (FMINE-REF-505, exit 8), naming each file (`fmine_public::load_registered_sources`). The parser therefore sees only registered bytes | Tested: `fmine-public` live-dependency proof (BD-338) |
| The zip container inside a workbook | Pure-Rust reader. Declared sizes are capped at 64 MiB per entry and 256 MiB per archive; inflation stops at the declared size; a name that appears twice is refused; a stored entry must declare its own length. CRC-32 is not checked, because integrity is the registered sha256 | Tested: `test_inflation_stops_at_the_declared_size`, `test_declared_sizes_past_the_bounds_are_refused`, `test_a_name_that_appears_twice_is_refused`, `test_a_stored_entry_must_declare_its_own_length` |
| Workbook XML | Character entities are decoded once, so `&amp;lt;` stays `&lt;` (tested: `test_xml_entities_decode_once`). The reader has no DTD processing, so no declared or external entity is expanded (by search). Hidden sheets are held apart from the visible ones, and formula-error cells are quarantined; both are listed in the output (G-BULL-1/2). One AISC path reads hidden sheets on purpose (`fmine-public/src/aisc.rs`) | Tested and by search, as marked |
| PDFs | Hashed, never parsed. Values from a PDF come from human transcription registries keyed by the PDF's sha256 | By search |
| A draft (`lint`) | Read up to 4 MiB, then refused. It never leaves the process; only number spans and their verdicts are printed | By search (`crates/fmine-cli/src/lint.rs`) |
| The reliance journal | Bounded at 64 MiB and 100,000 records. Each line carries BLAKE3 of the previous one. A truncated tail, a non-canonical line, an illegal transition or a wrong head refuses, and the journal is never repaired or rewritten. A writer holds an exclusive OS lock through replay, check, append and `sync_all` | Tested: `fmine-io` reliance tests |
| The forecast journal and its request files | The same bounds and chain as the reliance journal; a record's time may not go backwards. A torn tail refuses without repair. A removed suffix refuses when the caller pins the head, which every writer must do; `inspect` without `--head` cannot detect one. Request files are read only if they are regular files of at most 64 KiB | Tested: `fmine-claim` `forecast_journal` and `fmine-io` `forecast` tests, e.g. `corruption_torn_tail_and_prefix_rollback_refuse_without_repair`, `held_locks_exclude_writers_and_create_never_replaces` |
| Registries (TOML) | Read by a small TOML subset parser (`fmine_registry::toml_subset`); anything outside the subset refuses | Tested: `test_toml_subset_refuses_inline_table_with_line` and its neighbours |
| Arguments | Parsed by hand; an unknown verb or flag is a usage error (exit 2), still in one envelope | Tested: `test_cli_usage_errors`, `test_usage_error_from_parse_args_is_envelope` |

## 4. What the binary does not do

| Property | How known |
|---|---|
| No network access | **Observed** on macOS: the GitHub-built binary gives the same state root and anchor inside `sandbox-exec '(deny network*)'` as outside, on all rows and `--through 2026Q2`. In the control, `curl` inside the same profile cannot resolve a host (E854; bd-073, 2026-10-31). **By search**: no production crate uses `std::net` or an HTTP library. `xtask check-caps` enforces the ban only in the S0, K and E crates. `fmine-io`, `fmine-public` (layer V) and `fmine-cli` (layer U) are covered by search alone. The Linux check (`repro/barrick/ci_real_files.sh`, stage `offline`: a fresh network namespace) is written but has not run yet |
| No unsafe code | **Enforced**: `unsafe_code = "forbid"` in the workspace lints, which every member inherits |
| No subprocesses and no environment variables | **By search**: no production crate calls `std::process::Command` or reads `std::env::var`. Only `xtask`, a development tool that is not in the binary, runs programs |
| Writes only through two journals | **By search**: outside test code, every file the binary creates goes through `fmine-io` (`reliance.rs`, `forecast.rs`); the file writers in `fmine-public` (`synthetic.rs`, `live.rs`) are reached only from tests and `xtask`. Journals are created with `create_new` and mode 0600, and are only appended to after that. Issued artifacts and copies are created with `create_new`, mode 0600 and `sync_all`, so nothing is overwritten or truncated. The verbs that write: `reliance init`, `issue`, `reissue`, `withdraw`, `supersede` and `refresh`; `assure issue` and `assure public --output`, which record an issuance; `reliance reuse` and `reliance export`, which write a copy and append nothing; and `forecast journal init`, `register`, `freeze`, `outcome` and `assess`. All other verbs only read and print |
| The host clock is read only by the two journals | **Enforced**: `xtask check-caps` bans `SystemTime::now` and `Instant::now` in production code outside `fmine-runtime` and `fmine-lab`. **By search**: only `crates/fmine-cli/src/reliance.rs` and `crates/fmine-cli/src/forecast/live.rs` use `fmine_runtime::HostWallClock`. A reliance append refuses (FMINE-REF-202) unless that time was explicitly trusted. A forecast journal record stores the host time with the skew bound declared at `init`, with no trust step (T9) |

## 5. Threats

| # | Threat | Present controls | Gap |
|---|---|---|---|
| T1 | A hostile or substituted workbook: code execution, memory exhaustion, or values that differ between readers | Digest gate before parsing; bounded zip reader; entities decoded once; quarantine; no unsafe code; duplicate part names refused, so this reader and a spreadsheet program cannot take different parts under one name | Coverage-guided fuzz targets (`fuzz/`, run by `scripts/fuzz.sh`) cover the zip reader, the inflater, the XML parts, the number formats, the TOML registries and the forecast record's text inputs. So far each has run for minutes, not the 24 h that plan §32.4 asks (PO-PORT-1), and none has found a defect. A seeded mutation test (`crates/fmine-io/tests/reader_mutations.rs`) also runs 8,000 mutants of two synthetic workbooks through the reader on every test run, and requires no panic. The registry that pins the digests is a file in the workspace, so whoever can edit the workspace can register a hostile file |
| T2 | A tampered binary | Built by GitHub from a pushed commit; its sha256 is in `blind_record.json` and in the SBOM; `scripts/register_vintage.sh --run` stops when the binary's sha256 differs from the record's | No signature (plan: minisign through DSR) and no reproducible-build check. A user must compare the sha256 by hand |
| T3 | A compromised dependency | No third-party crates in the product. The fuzz harness in `fuzz/` is its own workspace, and its one dependency, libfuzzer-sys (which compiles LLVM's libFuzzer from C++), never reaches the binary; `Cargo.lock` with `--locked`; `cargo deny` in CI (advisories, bans, licences, sources; unknown registries and git sources denied outside the suite's organisation) | The compiler and the platform C library are trusted. The toolchain is a nightly |
| T4 | Data leaving the host | No network code; the macOS sandbox run | The Linux run is pending. Nothing stops a future change from adding network code to the V or U crates: `check-caps` covers S0, K and E only |
| T5 | Exposure of non-public issuer data (MNPI) | The M0 lane uses only public filings and issuer-website workbooks. The repository holds locators and digests, not issuer cell values (RISK-31) | This binary has **no** embargo states, walls or access log (§31.1). It must not be pointed at non-public data. Doing so would give that data no protection beyond the file system's |
| T6 | A false or stale reliance on an issued output | Append-only, hash-chained journal; exact-byte verification before reuse, export and reissue; the consumer's pinned head is required for current reuse; a reissued copy keeps the source issuance's producer metadata and enters the recall population | OS locks coordinate cooperating processes only. They are not signatures, and anyone who can replace the files can replace the journal. A release capability names a local role, not a person who reviewed the output |
| T7 | Misuse by an AI agent driving the CLI | Only the verbs listed in §4 write. `registries/verbs.toml` marks the ones that change authority (`mutates_authority`). Envelopes carry fixed false flags (`reserve_disclosure_claim`, `execution_authorized`, `geotechnical_signoff`, `regulatory_compliance_claim`); `robot-docs` describes the surface | The writing verbs mutate. An agent with file access can run them as any role the workspace grants. `reliance reuse` and `reliance export` are not marked as changing authority, but they still write a file |
| T8 | Resource exhaustion | The caps above (zip, draft, both journals, forecast requests) | No cap on the number of registered sources or the size of a registry file. Such a file comes from the workspace's own owner |
| T9 | A backdated or postdated forecast record, which would make a forecast look frozen before its outcome was known | Each record stores the host time, read as uncertain by the skew bound declared at `init`. The chain refuses a time that goes backwards. A writer must pin the expected head, and the clock is read only after the exclusive lock is taken | The time is whatever the host clock says. Whoever controls the clock, or can rewrite the whole journal and its pinned head, controls the record times. Nothing external attests them: no timestamp authority and no published head |

## 6. Open items for a security review

1. The Linux offline run's first CI result (bd-073).
2. A signed release and a reproducible-build check (T2).
3. 24-hour runs of every fuzz target (T1, plan §32.4, PO-PORT-1). Each has run for minutes so far. The `cli` target covers the verb parser and the envelope (§33.4): parse errors and every free-text field must print as one line of JSON, and no envelope exits 1. It parses only. The `cli_run` target executes verbs: argument lists from a fixed vocabulary, run against a missing workspace, so each verb stops at its first file access and nothing is written.
4. A static network and process ban for the V and U crates, so that §4's "by search" rows become enforced (T4).
5. The licence. The manifests declare MIT, and the SBOM reports that, but `LICENSE` reads "MIT License (with OpenAI/Anthropic Rider)" (E681). The owner's decision is pending (RISK-17).
6. Review by someone other than the author. bd-073's done-when is "Pack reviewed".
7. Independent time for forecast-journal records, or a published head, if a forecast's freeze time is to be relied on by anyone who does not control the host (T9).

This document should be revised whenever the binary gains a verb that writes, reads a new input type, or links a new crate. It is retired when the WP-12 site-pilot security pack replaces it.
