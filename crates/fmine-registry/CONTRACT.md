# CONTRACT.md — `fmine-registry`

## 1. Charter & Layer
- **Crate:** `fmine-registry`
- **Layer:** `S0` (Substrate)
- **Subsystem:** `Bedrock`
- **LOC Budget:** `10k planned / 12k ceiling` (`registries/loc_budget.toml`)
- **Allowed Module Families:** `["calendars", "catalog", "metrics", "parser", "sources", "table_identities", "toml_subset", "transcriptions"]`

## 2. Mathematical & Domain Formulation
Parses and validates the 8 canonical TOML registries in `registries/` (`loc_budget`, `workspace_topology`, `lifts`, `units`, `crs`, `calendars`, `refusals`, `verbs`) and verifies 1:1 alignment between `registries/refusals.toml` and `fmine_evidence::CATALOG` (generated from that file; `xtask check-refusals` checks the generation).

## 3. Authority, Epistemics & Representation
Serves as the schema and registry catalog authority for `fmine doctor` and `xtask`.

## 4. Public API Surface
- `table_identities::{parse_table_identities, load_table_identities, TableIdentityRegistry, TableIdentity,
  Term, IdentityKind, TermRole}` (BD-340.S5): strict `fmine.table-identities.v1` declarations for source-bound
  PDF arithmetic, without source values. Every identity names its registered document, family, sum or
  roll-forward kind, total quantity and period, unit, basis, construction and tie convention. Linked term rows
  explicitly name each member quantity, period, sign and role. A roll-forward has exactly one positive opening
  and at least one movement; signed printed values are multiplied by the explicit coefficient. Multi-year
  membership is declared rather than inferred. Duplicate quantities, unknown documents/fields, orphan terms,
  missing periods and invalid signs/roles refuse. One construction applies per family and one tie convention
  per document, preventing selection of a rounding rule per instance. `validate` rechecks caller-built structs.
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
- `sources::{RestatementNoteRow, RestatementScopeRow}`, `SourcesRegistry::{restatement_notes,
  restatement_scopes}` (BD-504e.2/.3): optional `[[restatement_note]]` rows bind an `id` to a registered
  workbook `file_name`, one `locator` (`sheet!cell`), exact `quoted_text`, release `from_quarter` and
  `evidence`. The row's `sha256` is derived from that workbook's registered source digest. Activation
  cannot precede the source's release. Each `[[restatement_scope]]` names a registered `note` and
  `subject`, an exact canonical `line`, inclusive `period_from` / `period_to` quarters, and optional
  zero-based `occurrence` (default `0`). Unknown fields, malformed or missing bindings, unused notes,
  duplicate note ids or cells, and overlapping scopes for the same note and selector refuse. Wildcards
  are not supported; registries without these rows retain empty vectors. Runtime additionally verifies
  the exact quote against the digest-bound workbook and each selector against observed canonical claims
  with one unit. A scope covers a claim only when its full period lies within the quarter bounds.
  `region_members` remains the sole membership map. The E461 golden test pins the source digest and quote
  plus all 18 region-summary selectors for 2024Q1 through 2025Q4.
- `sources::{DocumentRow, DOCUMENT_KINDS, PUBLISHED_AT_BASES}`, `SourcesRegistry::documents`, `sources::is_date`
  (bd-plx1): optional `[[document]]` rows, one PDF each: a bare `file` and a sha256 unique across workbooks and
  documents, optional `dir` (a plain relative path; default `corpus_dir`), `kind` (`mda`, `aif`,
  `investor_deck`, `partner_filing` or `technical_report`), an `https://` `url` naming a file, a registered `url_basis`,
  `published_at` (a calendar date) with `published_at_basis` starting `"FACT: "` or `"INFERENCE: "` and citing
  its source, positive `pages` and `bytes`. A profile requires at least one validated `[[source]]` or
  `[[document]]`; document-only partner profiles use the same schema and still require `[[subject]]` rows.
  Document validation and cross-kind file/digest uniqueness are unchanged. No value is ever read from a document.
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
  DESIGN (BD-341.S5): `Agreement::rejected_quantities` retains every reader's quantity identity at each
  pending, disagreeing or clashing locator, without raw tokens. `is_complete_for_prefix` checks a named
  quantity namespace while treating missing, invalid or orphan identity metadata as incomplete. A mixed
  guidance/price identity disagreement still blocks guidance; the empty prefix requires global completeness.
  `is_consistent` and the accepted-row digest retain their existing meanings.
- `calendars::{load_site_calendars, parse_site_calendars, SiteCalendars}` (BD-029): `registries/calendars.toml`
  (`site_id`, IANA `tz_name`, `day_start_local_hour` 0..=23, `shift_hours` 1..=24, `roster_pattern`) joined with
  the zone snapshot `registries/zones.toml` (header `tzdata`, span `[from_utc_s, to_utc_s)`; one `[[zone]]` per
  zone with its offset at the span's start and `tzif_sha256`; `[[transition]]` rows `tz_name`, `at_utc_s`,
  `offset_minutes`) into `fmine_time::SiteCalendar`s with `ZoneRules`. Every field is required and no other is
  accepted (a fixed `utc_offset_minutes` refuses); a calendar's zone must have a `[[zone]]` row and every zone
  must be named by a calendar. Outside `REQUIRED_FILES`, like `metrics`; `xtask check-all` runs it. The snapshot
  is generated by `scripts/oracle/zone_snapshot.py registry`.

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
