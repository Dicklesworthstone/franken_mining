# CONTRACT.md — `fmine-cli`

## 1. Charter & Layer
- **Crate:** `fmine-cli`
- **Layer:** `U` (User Surfaces & CLI)
- **Subsystem:** `Surfaces`
- **LOC Budget:** `14k planned / 16k ceiling` (`registries/loc_budget.toml`)
- **Allowed Module Families:** `["cli", "envelope", "packet"]`

## 2. Mathematical & Domain Formulation
The `fmine` binary and library surface (`WP-06`, `BD-111`–`BD-117`, `BD-071`).

- **Envelope:** `FMT-ENV-1` (`schema_version: "fmine.env.v1"`).
- **Verbs:**
  - `assure public --issuer <issuer> [--through YYYYQn]`: the gate-eligible run over the filings in
    `registries/sources/<issuer>.toml`, optionally only those at or before a quarter;
  - `assure public --synthetic`: the known-answer run, never gate-eligible;
  - `assure challenges --issuer <issuer> [--through YYYYQn]` or `--synthetic`: content-addressed internal-review
    packets from visible assurance exceptions, with JSON or readable `--format markdown` output (BD-349);
  - `guidance completion --issuer <issuer> --site <site> --through YYYYQn --as-of YYYY-MM-DD` or `--synthetic`:
    cited remaining-production and AISC requirements under an explicit historical-sales scenario (BD-342);
  - `doctor`: per registered issuer, `sources_registered`, `sources_present`, `sources_verified` and one row
    per file, `{file, present, bytes_ok, digest_ok}` against the registry's size and sha256 (BD-339.S2). A
    mismatch is reported, never refused: doctor stays exit 0, status ok;
  - `claim show <id>`: the canonical public Claim;
  - `claim why <id>`: its recorded evidence, interpretation, computed color and admission guards;
  - `claim trace <id>`: its source anchors and the available provenance boundary;
  - `robot-docs`.

All three Claim commands accept `--issuer <issuer> [--through YYYYQn]` or `--synthetic` to select the same
verified source snapshot as assurance. Without a selector they search the current canonical projections.

## 3. Authority, Epistemics & Representation
- Every JSON invocation prints exactly one `FMT-ENV-1` envelope on stdout, a usage error included (`BD-329.S3`).
  Markdown wraps that envelope in a JSON fence, except successful challenge exports, which render the cited
  packets as readable sections with the source root, packet-set digest and internal-review boundary.
- After `anchor_id` every envelope has, in this order:
  - `flags`: the four INV-002 flags, all false, from one constant (no field or setter can set one);
  - `certificate`: the report's ledger state root (hex), or null when no ledger snapshot is registered;
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
- `packet::{packets_from_report, ChallengePacket}`; immutable packet accessors, `validate`, `to_json`,
  `to_markdown`. The existing assurance engine computes the states; this module packages their evidence.

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
- `claim show|why|trace <id>` resolve public Mine Stats and AISC Claims by re-reading the registered, hash-verified workbook
  inputs (BD-332.S3, BD-115). It returns the semantic key, both stored and displayed text, ownership and
  status evidence, and the source locator/digest. Mine Stats uses its direct reader; AISC uses the existing
  assurance projection. Exact IDs emitted by `assure public --synthetic` also resolve by rebuilding that
  existing fixture projection; their evidence remains synthetic, and no real claim is replaced by one.
  No view creates a parallel claim store. An invalid or absent id refuses with
  `FMINE-REF-201`; missing or altered source files retain
  their typed input refusals. Source assertions remain issuer assertions and are not decision-ready.
- DESIGN (BD-115): an explicit `--issuer` or `--synthetic` selector rebuilds that source projection after verifying every registered
  workbook's size and SHA-256. Its `fmine.claim_read.v1` payload includes the original keyed Claim, lane,
  `claims_digest`, staleness and calibration-failure flags, and whether a decision UU term exists. The envelope
  carries the original ledger anchor and state root; the separate projection digest binds interpretation.
  Selected `why` and `trace` views add their inspection object to that same payload. Their next actions retain
  the issuer, through bound or explicit synthetic selector. A Claim absent from the selected snapshot refuses
  with `FMINE-REF-201`, without searching another lane. Source failures retain their original refusal.
  This is a current projection read, not a durable historical store or a decision-calibration claim.
- `why` delegates to `fmine_public::key::inspect::why_json`. It calls `Claim::color`,
  `verify_uu_slot_for_decision` and `verify_export_clearance(Public)`, preserving their typed refusals in
  the successful inspection payload. The decision result is scoped to the color and UU guards used by
  `DecisionCard::build`, not a completed decision or authority grant. Actual UU terms are shown; an absent
  term, calibration assessment, decision context or flip margin is never invented. Infinite estimated
  dispersion is represented as `null` with `dispersion_status: unbounded`, not as zero. The recorded stale
  flag is shown separately from freshness, which has not been assessed by a clock or dependency replay.
- `trace` delegates to `fmine_public::key::inspect::trace_json`. Public projection leaves carry exact
  artifact paths, SHA-256 digests and cell locators, plus matching registered source URLs and release metadata.
  Built-in synthetic anchors have no invented registry row. Parent IDs remain explicit and unresolved if
  supplied: the trace is then partial and never claims a complete derived provenance DAG. Both views return
  the unchanged canonical Claim alongside their explanation and link to the other inspection commands.
- Claim kinds not produced by the registered public projection refuse; the verb never
  fabricates a claim.
- `doctor` refuses `FMINE-REF-506` when the registries cannot be read.
- DESIGN (BD-349): challenge packets cover visible AISC states ii–v, Mine Stats states iii–v and infeasible
  joint components, preserving each finding's original state, operands, Claim references, source hashes,
  certificates, alternatives and missing-fact request. Source resolution or operand mismatches refuse.
  Packet ids bind their content, and `packet_set_sha256` binds the deterministic ordered set separately from
  the unchanged assurance root. `human_check` is null and external use is hardwired false; generation cannot
  manufacture review or identify a culprit from a sign-flip witness or correction alternative.
- DESIGN (BD-342): guidance delegates acquisition to `fmine_public::guidance` and arithmetic to
  `fmine_metrics::guidance`. Its payload digest binds the cited inputs and explicit scenario. `anchor_id` and
  `certificate` remain null because this is not a registered forecast or ledger snapshot. Missing two-reader
  guidance, incompatible ownership/support, future evidence and unusable sales refuse; synthetic evidence is
  available only through the explicit synthetic lane.
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
- `PO-CLI-4` (BD-115): `test_claim_show_why_trace_golden` compares all three synthetic inspection payloads
  with hand-written expected output. Default tests preserve guard failures, explicitly partial provenance,
  exact-ID lookup and altered-source refusal for every view. The ignored real source test checks E474's
  issuer assertion, refused color and registered source trace without upgrading its evidence.
- FACT (BD-115, CLI tests): `test_claim_show_reads_reported_claims_and_refuses_invented_ids` round-trips numeric
  and placeholder Claim ids, projection digest and ledger root through all three selected views. The corpus test
  `test_claim_show_reads_real_mine_stats_and_aisc_claims` covers both adapters. `test_claim_show_rechecks_registered_source_bytes`
  exercises the registered-file path, preserves a through bound despite a later altered source, and refuses
  altered in-scope sources and Claim IDs from another selected lane.
- FACT (BD-342/349, CLI and packet tests): known-answer end-to-end tests cover exact rational guidance,
  preserved assumptions/citations, deterministic JSON and Markdown, missing evidence, mixed selectors,
  seeded discrepancy witnesses, joint-only conflicts and immutable review restrictions. The ignored real-corpus
  packet test retains the known Kibali and Porgera findings across both registered vintages.

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
- Challenge packets do not yet cover cross-vintage comparisons, PDF transcription conflicts, hidden or
  not-published instances. Human review and external-release sequencing remain open.
- Guidance completion is a T3 necessary-arithmetic report under a declared scenario, not a probability of
  meeting guidance, a technical capacity envelope or a completed M0 gate.
