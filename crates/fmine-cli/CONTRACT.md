# CONTRACT.md — `fmine-cli`

## 1. Charter & Layer
- **Crate:** `fmine-cli`
- **Layer:** `U` (User Surfaces & CLI)
- **Subsystem:** `Surfaces`
- **LOC Budget:** `14k planned / 16k ceiling` (`registries/loc_budget.toml`)
- **Allowed Module Families:** `["cli", "envelope"]`

## 2. Mathematical & Domain Formulation
The `fmine` binary and library surface (`WP-06`, `BD-111`–`BD-117`, `BD-071`).

- **Envelope:** `FMT-ENV-1` (`schema_version: "fmine.env.v1"`).
- **Verbs:**
  - `assure public --issuer <issuer> [--through YYYYQn]`: the gate-eligible run over the filings in
    `registries/sources/<issuer>.toml`, optionally only those at or before a quarter;
  - `assure public --synthetic`: the known-answer run, never gate-eligible;
  - `doctor`: per registered issuer, `sources_registered`, `sources_present`, `sources_verified` and one row
    per file, `{file, present, bytes_ok, digest_ok}` against the registry's size and sha256 (BD-339.S2). A
    mismatch is reported, never refused: doctor stays exit 0, status ok;
  - `claim show <id> --issuer <issuer> [--through YYYYQn]` or `--synthetic`: read an existing AISC or
    Mine Stats Claim from the same verified source projection that the assurance report emits (BD-115);
  - `robot-docs`.

## 3. Authority, Epistemics & Representation
- Every invocation prints exactly one `FMT-ENV-1` envelope on stdout, a usage error included (`BD-329.S3`).
- After `anchor_id` every envelope has, in this order:
  - `flags`: the four INV-002 flags, all false, from one constant (no field or setter can set one);
  - `certificate`: the report's state root (hex), or null for a verb that writes no report;
  - `warnings` (an array) and `next_actions` (`[{command, why}]`; every command parses with `parse_args`);
  - `no_claim`, on report envelopes only;
  - then `payload` and `refusal`.
- A successful assurance run carries the run's `anc_…` anchor id, and the report as payload: `lane`,
  `gate_eligible`, sources with SHA-256, the extraction census, the identity ledger with cited exceptions, the
  quarantine and the state root. The payload holds no flags, no `no_claim` and no `next_actions`.
- An issuer run's payload ends with `published_cutoffs` (BD-274), `fmine_metrics::cutoff::section` over
  `registries/published_cutoffs.toml` when that registry's `issuer` is the run's: the registry's sha256, the count
  of cut-offs reproduced, within input rounding and in conflict, and one row per cut-off. It is not in the state
  root, so the certificate is the same with or without it. No registry means no field; a registry that does not
  read, or a row that cannot be computed, refuses the run with `FMINE-REF-506` before any workbook is read. The
  synthetic lane never carries it.
- A refusal carries `{code, exit_code, what, fix}` and a null payload.
- Field names are this crate's. Renaming them to FMT-ENV-1's (`v`, `event`, `node`, `anchor`, `data`) and
  bumping `schema_version` are open for the FMT-ENV-1 owner.

## 4. Public API Surface
- `OutputFormat`, `CliCommand`, `Envelope`, `NextAction`
- `run` (what `fmine` prints, and its exit code), `invoke`, `parse_args`, `execute_command`

## 5. Guards & Typed Refusals
- **Exit 0:** ok.
- **Exit 2:** a usage error with no refusal code. This covers:
  - an unknown verb or flag, a surplus word, or a flag the verb does not take (none is ignored);
  - a missing `--issuer`;
  - an unregistered issuer (no `registries/sources/<issuer>.toml` in the workspace);
  - a malformed `--through` bound, or `--through` with `--synthetic`;
  - `--issuer` together with `--synthetic`.
- **Exit 3–8:** the catalog refusal's own exit code. For example, `FMINE-REF-506` (exit 5) means a registered
  file is missing or unreadable, and `FMINE-REF-505` (exit 8) that one is altered.
- `claim show <id>` resolves public Mine Stats and AISC Claims by re-reading the registered, hash-verified workbook
  inputs (BD-332.S3, BD-115). It returns the semantic key, both stored and displayed text, ownership and
  status evidence, and the source locator/digest. Mine Stats uses its direct reader; AISC uses the existing
  assurance projection. Neither creates a parallel claim store. An invalid or absent id refuses with
  `FMINE-REF-201`; missing or altered source files retain their typed input refusals.
- DESIGN (BD-115): an explicit `--issuer` or `--synthetic` selector rebuilds that source projection after verifying every registered
  workbook's size and SHA-256. Its `fmine.claim_read.v1` payload includes the original keyed Claim, lane,
  `claims_digest`, staleness and calibration-failure flags, and whether a decision UU term exists. The envelope
  carries the original ledger anchor and state root; the separate projection digest binds interpretation.
  It refuses `FMINE-REF-201` if the id is absent. Source failures retain their original
  refusal, so altered workbook bytes cannot yield a previously emitted Claim. This is a current projection
  read, not a durable historical store or a decision-calibration claim; `why` and `trace` remain open.
- `doctor` refuses `FMINE-REF-506` when the registries cannot be read.
- `robot-docs` lists every catalogue row as `{code, exit_code, status, summary}`, `status` being `active` or
  `proposed` (`docs/adr/ADR-REFUSALS-01.md`).

## 6. Proof Obligations (`PO-*`)
- `PO-CLI-1`: `assure public --synthetic` exits 0 with `lane` `synthetic`, `gate_eligible` false and the seeded
  totals.
- `PO-CLI-2`: `assure public --issuer barrick` without the registered files exits 8 and names them.
- `PO-CLI-3` (real files, ignored test): with the files it exits 0 with 34 tables, 2,595 instances and totals
  `{i 2120, ii 3, iii 4, iv 17, v 391, np 60}`.
- `PO-ADIT-1`: every verb's envelope matches a hand-written frame (`test_po_adit_1_golden_every_verb`; payload,
  anchor, certificate and refusal text masked), and `tests/e2e_envelope.sh` checks the printed line of each.
- FACT (BD-115, CLI tests): `test_claim_show_reads_reported_claims_and_refuses_invented_ids` round-trips numeric
  and placeholder Claim ids, projection digest and ledger root; an invented id refuses. The corpus test
  `test_claim_show_reads_real_mine_stats_and_aisc_claims` covers both adapters. `test_claim_show_rechecks_registered_source_bytes`
  exercises the registered-file path and refuses a source changed after the report was built.

## 7. Failure Modes & Deterministic Fallbacks
None: every failure is a usage error or a typed refusal.

## 8. Gates & Kill Criteria (`GATE-*` / `KILL-*`)
- `GATE-BULL-01`

## 9. Lifted Code & Foundation Dependencies
- Depends on `fmine-core`, `fmine-units`, `fmine-time`, `fmine-crs`, `fmine-evidence`, `fmine-claim`,
  `fmine-registry`, `fmine-array`, `fmine-numerics`, `fmine-calib`, `fmine-decide`, `fmine-io`, `fmine-public`,
  `fmine-metrics`.

## 10. Non-Goals & Scope Boundary
- Computes nothing itself: all calculation is delegated to the `V`/`E`/`K`/`S0` crates.
- Never writes to any external system.
