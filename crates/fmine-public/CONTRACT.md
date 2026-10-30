# CONTRACT.md — `fmine-public`

## 1. Charter & Layer
- **Crate:** `fmine-public`
- **Layer:** `V` (Verification, Ingestion & Assurance)
- **Subsystem:** `Assurance`
- **LOC Budget:** `14k planned / 16k ceiling` (`registries/loc_budget.toml`)
- **Allowed Module Families:** `["sources", "aisc", "mine_stats", "synthetic"]`

## 2. Mathematical & Domain Formulation
The public lane of M0 (`fmine assure public`).

- `sources`: the source registry (`BD-055`, `BD-322`, bd-s9j3.1), read at run time from
  `<workspace>/registries/sources/<issuer>.toml` (schema `fmine.sources.v1`, checked by `fmine_registry::sources`
  and by `xtask registry-check`). It pins each public workbook by file name, URL, byte size and SHA-256 (Barrick:
  Mine Stats 2024Q1–2026Q2 and AISC reconciliations 2024Q3–2026Q2, from W1, E451, E452), and holds the reporting
  subjects printed in the AISC table headers and the registered reconstructions (E816). Registering a vintage is
  a TOML edit, never a code change; `through(YYYYQn)` keeps only rows at or before a quarter, so an earlier run
  reproduces after later rows are added.
- `aisc`: a label-anchored reader for the AISC reconciliation tables. It finds rows by column-A label and the
  header row by registered subjects, so no coordinate is fixed. It also builds the identity ledger
  (`GATE-BULL-01` part 2, plan §21.6).
  - **Declared construction:** component sum. TCC = Σ block-1 lines; AISC = TCC + Σ block-2 lines. Each rate is
    r = 1000·N/Q, checked on intervals (N ± k/2, Q ± 1/2, r ± 1/2). All arithmetic is exact (`i128`).
  - **States:** (i) consistent; (ii) explained by a registered reconstruction, re-verified on the run;
    (iii) issuer discrepancy, with an exact certificate and any single-line sign-flip witness;
    (iv) undecidable; (v) refused; not published (excluded by name).
  - **Alternative construction:** subtotal rounded once. It is reported, never scored.
- `mine_stats` (bd-gwf1.1): a label-anchored reader for the quarterly Mine Statistics workbooks (W1 §3, §4, §13).
  It finds blocks by `For the (three|six|nine|twelve) months`, takes site names from the row above (carried left
  across merged blanks), and strips trailing footnote numbers from labels. The `QTD`/`YTD` summary columns are
  mapped to field ids. Each data cell becomes a typed value (`Number` with its stored text, `Nil`,
  `NotApplicable`, `Missing`) with locator `<quarter>:<sheet>!<cell>`, period, site, printed ownership share,
  label and section. Numeric zero stays `Number`. Cells outside the token set, unknown periods and labels
  repeated within a block are refused by locator. The metric-label registry (labels, units, region maps,
  corrections) is not built yet.
- `synthetic`: invented workbooks in the real layout, seeded with one known answer per state. They are never
  gate-eligible.
- `lib`:
  - `load_registered_sources` verifies every registered file;
  - `assess` builds the ledger and the state root;
  - `PublicAssuranceReport::to_json` is the envelope payload;
  - `cell_to_public_claim` is the public-basis `Claim` builder (`BD-058`).

## 3. Authority, Epistemics & Representation
- Every reported exception cites file, file SHA-256, `sheet!cell`, subject, family and ownership basis
  (`IdentityFamily::basis`).
- The `LedgerAnchor` state root (BLAKE3 over an `M0AS` v2 frame) binds:
  - every input's SHA-256, numeric digest and census;
  - every instance's state and detail;
  - unregistered headers and table refusals.

  Changing any input cell moves the root.
- The report carries `lane`, `gate_eligible`, a `no_claim` line, and the four INV-002 flags, all false.

## 4. Public API Surface
- `sources::{SourceKind, RegisteredSource, RegisteredReconstruction, IssuerProfile, load_issuer_profile,
  registry_path, registered_issuers, is_issuer_name, is_through_bound}`; `IssuerProfile::{from_registry, through}`
- `aisc::{IdentityFamily, IdentityState, IdentityInstance, FootingCertificate, UnregisteredHeader, AiscTable,
  RateMiss, locate_table, ledger_for_table, rate_interval_holds, reconstruction_closes, match_subject, cell_text}`
- `mine_stats::{View, MineStatsCell, MineStatsValue, CellRefusal, MineStatsRead, read_mine_stats, strip_footnote,
  printed_share, site_name, is_m0_target}`
- `synthetic::{synthetic_aisc_cells, synthetic_workbooks, synthetic_profile, SYNTHETIC_SUBJECTS}`
- `Lane`, `LoadedSource`, `TableRefusal`, `AlternateCounts`, `PublicAssuranceReport`, `load_registered_sources`,
  `assess`, `run_public_assurance`, `run_barrick_m0_assurance`, `run_synthetic_assurance`, `cell_to_public_claim`

## 5. Guards & Typed Refusals
- `FMINE-REF-502` (exit 8) names every registered file that is missing, digest-mismatched or unparseable. A run never
  substitutes content. The same code (until BD-328 assigns one) refuses a missing or malformed registry, naming the
  file and line, and a `through` bound that leaves no rows.
- `FMINE-REF-401` from `cell_to_public_claim` on text or formula-error cells.
- Instance-level refusals (state (v)) carry a reason:
  - an unrounded stored value;
  - a non-numeric line;
  - "attributable share needed" when a non-zero non-controlling-interest line separates the bases of cost of sales
    and ounces;
  - one of rate and ounces being zero.

## 6. Proof Obligations (`PO-*`)
- `PO-PUB-1` (real files through 2026Q2, ignored test; recorded in MOR_CASE E820–E823): the census and digest of
  every registered workbook equal an independent reader's. The test also pins the state root as a regression
  value (not independent).
  - **Ledger:** 34 visible AISC tables, 2,595 instances. Footing: 1,004 (i), 4 (iii), 6 (v), 24 not published.
    Per ounce: 1,116 (i), 3 (ii), 17 (iv), 385 (v), 36 not published.
  - **Alternative construction:** 1,008 footing and 994 rate instances consistent.
  - **Identities:** each (ii) and (iii) instance is identified.
- `PO-PUB-2` (synthetic): every seeded answer is reproduced, including the sign-flip witness. Perturbing one cell
  moves the state root and the affected state. A wrong component label leaves the instance in (iv).
- `PO-PUB-3`: a workspace without the files refuses, naming every registered file; the count follows the registry
  rows (a row added to a copy of the TOML changes it with no code edit).
- `PO-PUB-4` (Mine Stats reader, MOR_CASE E825):
  - **Real files** (ignored test through 2026Q2): all ten reads equal W1's openpyxl read, digest for digest
    (22,732 cells, no refusals). Q1 and Q2 2026 hold 3,996 cells, and the seven numeric zeros sit at their
    locators with stored text `0`. The W1 §4 M0 census is 820 cells.
  - **Synthetic** (default suite): an inserted site, an inserted row, renumbered footnotes and a misspelled
    month give the same values. Each refusal names its cell: an unknown token, a repeated label, an unknown
    period and a formula error.

## 7. Failure Modes & Deterministic Fallbacks
- A sheet with a cost-of-sales row but no locatable structure becomes a `TableRefusal`.
- A header that names no registered subject becomes an `UnregisteredHeader`.
- Neither stops the run, and neither is ever guessed.

## 8. Gates & Kill Criteria (`GATE-*` / `KILL-*`)
- `GATE-BULL-01` (parts 1 and 2; part 3, issuer consistency across vintages, is not yet scored)

## 9. Lifted Code & Foundation Dependencies
- Depends on `fmine-core`, `fmine-evidence`, `fmine-claim`, `fmine-io` (and, in the manifest, `fmine-units`,
  `fmine-time`, `fmine-metrics`, not used by this crate since BD-321).

## 10. Non-Goals & Scope Boundary
- Operates only on public disclosures (`ConfidentialityTier::Public`).
- Makes no reserve, resource, geotechnical, regulatory or investment claim.
- Implies no issuer affiliation.
