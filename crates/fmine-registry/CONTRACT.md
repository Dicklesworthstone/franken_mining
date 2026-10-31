# CONTRACT.md — `fmine-registry`

## 1. Charter & Layer
- **Crate:** `fmine-registry`
- **Layer:** `S0` (Substrate)
- **Subsystem:** `Bedrock`
- **LOC Budget:** `10k planned / 12k ceiling` (`registries/loc_budget.toml`)
- **Allowed Module Families:** `["catalog", "parser", "sources", "toml_subset", "transcriptions"]`

## 2. Mathematical & Domain Formulation
Parses and validates the 8 canonical TOML registries in `registries/` (`loc_budget`, `workspace_topology`, `lifts`, `units`, `crs`, `calendars`, `refusals`, `verbs`) and verifies 1:1 alignment between `registries/refusals.toml` and `fmine_evidence::CATALOG` (generated from that file; `xtask check-refusals` checks the generation).

## 3. Authority, Epistemics & Representation
Serves as the schema and registry catalog authority for `fmine doctor` and `xtask`.

## 4. Public API Surface
- `RegistryCatalog`, `RegistryError`
- `toml_subset::{parse_subset, TomlDoc, TomlRow, TomlField, TomlValue, SubsetError}`: a strict TOML subset (`[[table]]` rows, basic strings, integers, comments); anything else refuses naming the line (bd-s9j3.1)
- `sources::{parse_sources_registry, load_sources_registry, SourcesRegistry, SourceRow, ReconstructionRow, BasisRow, MissingFactRow, MineStatsLabelRow, ShareRow, RegionMemberRow, SourcesError, is_quarter, is_url_basis, URL_BASES, URL_BASIS_FETCHED}`, `SourcesRegistry::members_of`, `RegionMemberRow::holds_for`: issuer source registries (each `url_basis` is `"W1 table"`, `"W1 naming rule"` or `"fetched and verified YYYY-MM-DD"` with a calendar date, BD-339.S3; each `[[reconstruction]]` declares `basis = "reconstructed"`, or `"stated"` with `quote_doc` (a registered file), `quote_locator` (`sheet!cell`) and `quote_prefix`, BD-331.S2; optional `[[missing_fact]]` rows, BD-331.S2: `subject`, `family`, `text` (prose with no digits), `evidence`, optional `closing_document`, once per subject and family; optional `[[mine_stats_label]]` rows: `label`, optional `section`, duplicates refused; optional `kind = "site" | "region"` on `[[subject]]`; optional `[[share]]` rows, BD-331.S3: `subject` (a registered non-region subject, once), positive `num` ≤ `den`, `evidence`, optional Mine Stats `label`; optional `[[region_member]]` rows, BD-331.S4: `region` (a registered region), `member` (a registered non-region subject), release quarter `first` and optional `last` ≥ `first`, no two rows of one member over overlapping releases) `registries/sources/<issuer>.toml` (`fmine.sources.v1`), checked on load
- `sources::PrintedShareCorrection`, `SourcesRegistry::printed_share_corrections` (bd-prmn.4): optional
  `[[printed_share_correction]]` rows, the share to use where a Mine Stats label prints a stale one: `subject`
  (a registered site, not a region), `printed` (the decimal inside `(…%)`, e.g. `"47.5"`), positive `num` ≤
  `den`, `evidence`; one row per subject and printed share. It is not a `[[share]]` row: the registered share
  holds for the AISC tables, while this one replaces only what the label prints, which stays reported beside.
- `sources::MineStatsLabelAlias`, `MineStatsLabelAlias::holds_for`, `SourcesRegistry::mine_stats_label_aliases`
  (BD-331.S4, W1-DR-02): optional `[[mine_stats_label_alias]]` rows, a `Minestats*` row label printed with a
  stale unit and read as a registered one in files released from `first` to `last`: `label` and `reads_as`
  (both registered `[[mine_stats_label]]` labels, never the same one), release quarters `first` and `last` ≥
  `first`, `evidence`; no two rows of one label over overlapping releases. Values are never converted.
- `sources::{DocumentRow, DOCUMENT_KINDS, PUBLISHED_AT_BASES}`, `SourcesRegistry::documents`, `sources::is_date`
  (bd-plx1): optional `[[document]]` rows, one PDF each: a bare `file` and a sha256 unique across workbooks and
  documents, optional `dir` (a plain relative path; default `corpus_dir`), `kind` (`mda`, `aif`,
  `investor_deck` or `partner_filing`), an `https://` `url` naming a file, a registered `url_basis`,
  `published_at` (a calendar date) with `published_at_basis` starting `"FACT: "` or `"INFERENCE: "` and citing
  its source, positive `pages` and `bytes`. No value is ever read from a document.
- `transcriptions::{parse_transcriptions, load_transcriptions, TranscriptionRegistry, EntryRow, Locator, Token,
  Decimal, parse_token, agree, Agreement, Accepted, Disagreement, TRANSCRIPTIONS_SCHEMA, MAX_SCALE}` (bd-plx1):
  `registries/transcriptions/<issuer>.toml` (`fmine.transcriptions.v1`; git-ignored pending OPEN-R7-1). Each
  `[[entry]]` is one reader's read of one cell: `doc` (a registered document) and `doc_sha256` (its registered
  digest, so a re-posted document is a new vintage), `page` (at most the document's `pages`), `table`, `row`,
  `col` (labels as printed), `token` (exactly as printed), `quantity` (a key two documents share when they print
  the same quantity), `unit`, `basis`, `period`, `reader` (a role id, never a name); one entry per reader and
  locator, and per reader, document and quantity. `parse_token` reads "(708)" as −708, "6.7" as 67 at scale 1
  (step 0.1), "1,423" as 1423, and "200 - 230" or "200–230" as a range; a dash, "n/a" or bad grouping refuses
  with a reason that names no value. `agree` accepts a locator only when two or more readers entered it with
  the same token, quantity, unit, basis and period; a disagreement lists the readers and the differing fields
  (never their values) and is never resolved, a third reader included; a lone reader's locator is pending; one
  quantity agreed at two locators of one document accepts neither. `Agreement::digest` is the sha256 of the
  accepted rows without readers, so a third reader who agrees reproduces it. Locators are cited
  `<doc>:p<page>:<table>:<row>/<col>`: bd-plx1's form with the table added, since one page can print two tables
  with the same labels.

## 5. Guards & Typed Refusals
- `RegistryError::MissingEntry`, `RegistryError::CatalogMismatch`, `RegistryError::Io`.

## 6. Proof Obligations (`PO-*`)
- `PO-REG-1`: `RegistryCatalog::load_from_root` verifies all 8 workspace TOML registries and 100% refusal-code parity, in order, with `CATALOG` (32 codes).

## 7. Failure Modes & Deterministic Fallbacks
Missing or out-of-sync registry entries fail closed with `RegistryError`.

## 8. Gates & Kill Criteria (`GATE-*` / `KILL-*`)
- `GATE-LAYERS-00`

## 9. Lifted Code & Foundation Dependencies
- Depends on `fmine-core`, `fmine-units`, `fmine-time`, `fmine-evidence`.

## 10. Non-Goals & Scope Boundary
Read-only TOML validation; never mutates registry files at runtime.
