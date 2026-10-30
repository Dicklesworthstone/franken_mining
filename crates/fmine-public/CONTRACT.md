# CONTRACT.md — `fmine-public`

## 1. Charter & Layer
- **Crate:** `fmine-public`
- **Layer:** `V` (Verification, Ingestion & Assurance)
- **Subsystem:** `Assurance`
- **LOC Budget:** `14k planned / 16k ceiling` (`registries/loc_budget.toml`)
- **Allowed Module Families:** `["sources", "aisc", "mine_stats", "synthetic", "live", "citation", "key"]`

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
    r = 1000·N/Q, checked on intervals (N ± k/2, Q ± 1/2, r ± 1/2).
  - **Kernel** (BD-331.S6): every footing and rate decision is a run of `fmine_numerics::exact_feas` (BD-340)
    under the `Closed` tie convention, whose checkers re-verify each witness and certificate.
    - A footing is posed under `ComponentSum` and its alternative under `RoundedOnce`.
    - A placeholder line is an exact 0 (a zero step) and never counts in k.
    - In the AISC alternative, TCC stands for its k₁ numeric block-1 lines.
    - A (iii)'s residual and sign-flip witness come from the kernel's residual certificate. Its total and line
      cells are S3's one minimal conflict set.
    - The (iv) range is the kernel's implied interval, rounded outward to three places.
    - A kernel that makes no claim refuses the instance (`exact_kernel_indeterminate`).
  - **States:** (i) consistent; (ii) explained by a registered reconstruction, re-verified on the run and
    labelled `explained_stated` or `explained_reconstructed` by whether an issuer note states the rule;
    (iii) issuer discrepancy, with an exact certificate and any single-line sign-flip witness;
    (iv) undecidable, naming the missing fact (registered by subject and family, else the family default) and any
    closing document; (v) refused; not published (excluded by name).
  - **Alternative construction:** subtotal rounded once. It is reported, never scored.
  - **Hidden scope** (BD-331.S2): the same families run over hidden worksheets into `hidden_scope`. Each instance
    is labelled "sheet not displayed to readers". It is bound into the state root in its own section and never
    counted in the totals, `by_family` or `GATE-BULL-01`.
  - **One-cell check** (bd-prmn.8, Appendix K.4): a diagnostic, never a state and not in the state root. On
    every registered-subject column of the visible tables, cost of sales per ounce is checked against
    1000·s·N/Q at the column's basis. The basis is the registered, label-confirmed share where an NCI line is
    non-zero, and 100% otherwise. Each column reports the check at 100%, the check at the registered share,
    and the implied ounce interval. `one_cell_verdict` counts the failures outside a caller's K.4 items:
    "site-specific" if there are none, otherwise "basis rule".
- `mine_stats` (bd-gwf1.1): a label-anchored reader for the quarterly Mine Statistics workbooks (W1 §3, §4, §13).
  It finds blocks by `For the (three|six|nine|twelve) months`, takes site names from the row above (carried left
  across merged blanks), and strips trailing footnote numbers from labels. The `QTD`/`YTD` summary columns are
  mapped to field ids. Each data cell becomes a typed value (`Number` with its stored text, `Nil`,
  `NotApplicable`, `Missing`) with locator `<quarter>:<sheet>!<cell>`, period, site, printed ownership share,
  label and section. Numeric zero stays `Number`. Cells outside the token set, unknown periods and labels
  repeated within a block are refused by locator. With the issuer's `[[mine_stats_label]]` vocabulary (Barrick:
  56 pairs from W1's read, frozen before the Q3 2026 release), a data row whose `(section, label)` is not
  registered is refused too. Units, region maps and the W1-DR-01/02 corrections are not data yet.
- `synthetic`: invented workbooks in the real layout, seeded with one known answer per state. They are never
  gate-eligible. `write_issuer_workspace` writes them, plus a later title-only vintage, with a digest registry
  under the reserved issuer name `synthetic`; `run_public_assurance` runs that name in the synthetic lane.
- `lib`:
  - `load_registered_sources` verifies every registered file;
  - `assess` builds the ledger and the state root;
  - `PublicAssuranceReport::to_json` is the envelope payload;
  - `cell_to_public_claim` is the public-basis `Claim` builder (`BD-058`).

## 3. Authority, Epistemics & Representation
- Every reported exception cites file, file SHA-256, `sheet!cell`, subject, family and ownership basis
  (`IdentityFamily::basis`).
- The `LedgerAnchor` state root (BLAKE3 over an `M0AS` v4 frame) binds:
  - every input's SHA-256, numeric digest and census;
  - every instance's state as codes, numbers and locators (`encode_state`), never its prose, with a tag before
    each optional part;
  - each instance's operands (role, locator, value);
  - unregistered headers and table refusals;
  - the same for the hidden scope, in its own section.

  Changing any input cell moves the root; rewording a message does not (bd-sjrh.1 part 4).
- The report payload carries `lane` and `gate_eligible`. The no-claim line (`NO_CLAIM`), the four INV-002 flags
  and the next actions (`PublicAssuranceReport::next_actions`) are envelope fields, which `fmine-cli` prints.

## 4. Public API Surface
- `sources::{SourceKind, RegisteredSource, RegisteredReconstruction, RegisteredShare, RegisteredMissingFact,
  IssuerProfile, load_issuer_profile, registry_path, registered_issuers, is_issuer_name, is_through_bound}`;
  `IssuerProfile::{from_registry, through}`; `IssuerProfile::{regions, shares}` (BD-331.S3);
  `IssuerProfile::missing_facts`, `RegisteredReconstruction::basis` (BD-331.S2);
  `ExpectedRoot`, `expected_roots_path`, `load_expected_roots` (BD-339.S1);
  `kit_selectors_markdown`, `kit_readme_section`, `KIT_SELECTORS_BEGIN`, `KIT_SELECTORS_END` (BD-339.S4: the
  generated section of `repro/<issuer>/README.md`, from the row anchors and the registry; a golden test
  compares)
- `PublicAssuranceReport::{registry_epoch, census_digest_of_digests, check_expected}` (BD-339.S1)
- `aisc::{IdentityFamily, IdentityState, IdentityInstance, FootingCertificate, UnregisteredHeader, AiscTable,
  RateMiss, ShareBasis, ExplanationBasis, DEFAULT_MISSING_FACT, SHARE_MISSING_FACT, RowAnchor, AnchorRule,
  ROW_ANCHORS (the seven rows `locate_table` anchors on, BD-339.S4), locate_table,
  ledger_for_table, rate_holds, rate_range, admitted_rates, KernelIndeterminate, feasible_share,
  feasible_share_text, find_share_footnote, find_qtd_sheet, confirm_share_label, reconstruction_holds,
  match_subject, cell_text,
  OneCellCheck, OneCellOutcome, OneCellVerdict, FailGroup, implied_ounces, one_cell_checks, one_cell_verdict}`
  (bd-prmn.8); `PublicAssuranceReport::one_cell`, the payload's `one_cell_cos` section.
- `aisc::{Operand, OperandRole, OperandValue, operand_at}` and `IdentityInstance::operands` (bd-sjrh.1, part 2
  of BD-329.S1). Each instance cites every cell its state was computed from, with its role (total, line,
  rate, ounces, numerator, NCI line, component, label check) and its value as read.
  - **Cited cells:**
    - the table cells of the instance's family;
    - every used cell, for an all-zero column;
    - the footnote and the confirmed share label, for a cost-of-sales rate with a non-zero NCI line;
    - a consulted reconstruction's quote, label-check and component cells.
  - **Not cited:** structure cells (row labels, subject headers, a sheet's title and period line).
  - **Not yet in the payload or the state root.**
  - **Test:** `test_operands_cover_every_input_cell` perturbs every other synthetic cell, one at a time, and
    fails on any state change it causes that the operands do not cite.
  `IdentityState::Consistent` carries `basis` (the stated share, footnote and label it used, or `None`);
  `IdentityState::Explained` carries `basis: ExplanationBasis`; `IdentityState::Undecidable` carries
  `missing_fact` and `closing_document`; `IdentityState::RefusedAt` is (v) naming a cell or file.
- `mine_stats::{View, MineStatsCell, MineStatsValue, CellRefusal, MineStatsRead, LabelVocabulary, read_mine_stats,
  strip_footnote, printed_share, site_name, is_m0_target}`
- `synthetic::{synthetic_aisc_cells, synthetic_hidden_cells, synthetic_workbooks, synthetic_stats_workbook,
  synthetic_share_label_cells, synthetic_profile, write_issuer_workspace, SYNTHETIC_ISSUER, SYNTHETIC_SUBJECTS,
  SYNTHETIC_SHARES, SYNTHETIC_SHARE_FOOTNOTE, SYNTHETIC_STATED_QUOTE, SYNTHETIC_HIDDEN_SHEET,
  SYNTHETIC_LATER_STATS_FILE}`
- `live::{CheckKind, LiveCheck, LiveReport, Loader, prove_files}` (BD-338.S1): the file half of the
  live-dependency proof. For each registered file, the cases are absent, zero bytes and a foreign xlsx, each in
  its own scratch workspace. Each must refuse `FMINE-REF-502`, exit 8, "1 of n registered", naming only that
  file under "missing" or "digest mismatch".
- `live::{prove_leaves, LeafCheck, Perturbation, leaf_summary, rate_flip_up, rate_flip_down}` (BD-338.S2): the
  leaf half. Each operand of each visible instance is perturbed alone, on a copy of the parsed sources with
  that file's census recomputed.
  - **Pass rule:**
    - the field at the leaf reads the new value, including a footing certificate's `printed`; otherwise
      "value did not move";
    - a field derived from the leaf (component sum, k, rate range) or the state moves; otherwise "dead
      ancestor";
    - the root moves.
  - **Perturbations:**
    - +1, or 1 for a placeholder;
    - a counted line to "-" (k − 1);
    - a plain-consistent rate to its flip margin, or 3r + 1000 for a stated-share or explained rate;
    - a decisive NCI line to 0;
    - a label to "x".
  - **Skips:** a state decided before arithmetic, a share refusal's non-cause leaves, and an unrounded value
    are skipped, each with its reason.
  - **Not yet:** hidden-scope instances.
- `live::{prove_leaves_in, one_leaf_per_instance, prove_labels, LabelCheck}` (BD-338.S3): the real-lane
  frame and the label sample.
  - `prove_labels` mutates each of the seven anchor labels of a located table (append "x"; prepend it for the
    ounces prefix). Each must give a table refusal or changed instances.
  - **Real lane** (ignored, local runs):
    - files: 54 of 54;
    - leaves (one per instance): 2,444 pass, 0 fail, 151 skipped;
    - labels: 56 of 56 table refusals;
    - the Kibali O11 literal mutant is caught.
  - **Finding, fixed:** a broken cost-of-sales label made `locate_table` return `Ok(None)`, so the table dropped
    out with no refusal. `assess` now refuses a reconciliation's own-quarter sheet by name when it has no table
    ("no cost-of-sales row on the release's own quarter sheet") or is missing ("the release's own quarter sheet
    is missing").
- `aisc::Calc` and `IdentityInstance::calc` (bd-sjrh.1): a footing's printed total, component sum and k; a
  rate's r, n, k and q, as the ledger read them. `citation` renders them in the payload, and the root binds the
  operands they come from. `OperandRole::Counted` marks lines read only for their count: rate lines, and block 1
  under the AISC footing.
- `citation::{Cited, RULES, RULES_VERSION, extracted, derived, cited_json, locator}` (bd-sjrh.1 part 3): the
  cited numbers of an instance.
  - **Extracted:** each operand is `{"role","v"|"token"|"blank"|"unrounded","at","sha256"}`.
  - **Derived** (`{"name","v","rule","rules_version","in"}`, rules version `m0-ledger/1`):
    - a footing gives `component_sum` (`aisc.footing.component_sum`, from its lines) and `residual`
      (`aisc.footing.residual`, from its total and lines);
    - a rate gives `k` (`ledger.count`, of its counted lines or the cost-of-sales row) and `range_lo` and
      `range_hi` (`aisc.rate_interval`, rounded outward).
  - **Payload:** every exception record carries `cited`, and an issuer discrepancy's certificate cites printed,
    component sum and residual. The top-level `instances` array holds one `{"at","family","state","cited"}`
    record per counted instance: 50 synthetic and 2,595 real.
  - **Size:** the real payload is about 7.3 MB.
  - **Not yet:**
    - totals and census counts as count citations;
    - the uncited-number validator (S2).
- **Structured states** (bd-sjrh.1 part 1):
  - `aisc::{RefusedReason, NotPublishedReason}`: each has a unique snake_case `code()` and a `why()` text with no
    digits.
  - `aisc::{Undecided, ReconstructionCheck}`: a (iv) is `RateOutside(RateMiss)`, `ShareOutside` or
    `ReconstructionNotApplied { evidence_id, check }`. Each check (quote, file, sheet, label, whole number,
    closes) carries its own fields.
  - `aisc::StatedShare` is a stated-share basis, and `aisc::CitedCell` an explained component.
  - `IdentityState::reason_code()` gives the code that the payload's exception records and refused and
    not-published groups carry as `reason_code`. `detail()` renders the same prose as before.
- `Lane`, `LoadedSource`, `TableRefusal`, `AlternateCounts`, `HiddenScope`, `HIDDEN_SCOPE`,
  `PublicAssuranceReport`, `load_registered_sources`, `assess`, `run_public_assurance`, `run_barrick_m0_assurance`,
  `run_synthetic_assurance`, `cell_to_public_claim`, `NO_CLAIM`; `PublicAssuranceReport::next_actions`
- `key` (bd-a28f.1, BD-332.S1): the key of a printed public figure, and the value typed with its raw token.
  - `PublicKey { issuer, subject, valid_period, reporting_vintage, ownership_basis, scope, unit, line }`.
    `series_key()` drops only the vintage. `canonical_tag(raw_token)` is `fmk1` plus every field and the
    token, length-prefixed, and goes into `Claim::new` as the canonical value tag, so the claim id differs
    whenever any field or the token differs.
  - `parse_sheet_period`: `Qn YYYY`, `YTD Q2 YYYY` (H1), `YTD Q3 YYYY` and `YTD Sept 30 YYYY` (nine months),
    `YTD Dec 31 YYYY` and `YYYY` (full year). Anything else refuses.
  - `parse_ownership`: `100%` first, then a final `(x%)` after a one- or two-digit footnote run. No `%` gives
    `Unprinted`, never 100%.
  - `classify`: `—` and other dashes give `Nil { Unknown }`, `n/a` gives `NotApplicable`, and no cell gives
    `Missing`, each keeping the token as stored. A number keeps its stored text and its displayed text, or the
    display refusal code. A numeric zero where a registered disposal or withholding applies is `Nil`; a
    non-zero number there stays a `Number`, flagged. Text, a formula error or a number without stored text
    refuses.
  - `SubjectStatus`, `status_for`: the `[[subject_status]]` rows of the issuer registry, carried on
    `IssuerProfile::subject_statuses`. A disposal covers the periods that begin after its closing date.
  - **Not yet:** no run reads claims through the key. The Mine Stats and AISC readers adopt it in S2 and S3.

## 5. Guards & Typed Refusals
- `FMINE-REF-502` (exit 8) names every registered file that is missing, digest-mismatched or unparseable. A run never
  substitutes content. The same code (until BD-328 assigns one) refuses a missing or malformed registry, naming the
  file and line, and a `through` bound that leaves no rows.
- `FMINE-REF-401` from `cell_to_public_claim` on text or formula-error cells.
- Instance-level refusals (state (v)) carry a reason:
  - an unrounded stored value;
  - a non-numeric line;
  - one of rate and ounces being zero.
- Cost of sales per ounce in a column with a non-zero non-controlling-interest line (BD-331.S3) is checked at the
  subject's registered share, s: 1000·s·N/Q on exact corners. A fit is (i), and its `basis` cites the share, the
  footnote and the label. A miss is (iv), giving the feasible share interval, rounded outward, and the missing
  fact. The instance is (v) instead when:
  - the sheet has no attributable-basis footnote, found by text;
  - the subject is a registered region ("regional numerator needs member weighting");
  - the subject has no registered share;
  - the same-release Mine Stats quarter-to-date label is missing, repeated or prints another share
    (`RefusedAt`, naming the cell);
  - an operand is not positive.
- A stated (ii) whose quote cell no longer starts with the registered prefix is refused as an explanation: the
  instance stays (iv), naming the quote cell and what it reads.

## 6. Proof Obligations (`PO-*`)
- `PO-PUB-1` (real files through 2026Q2, ignored test; recorded in MOR_CASE E820–E823): the census and digest of
  every registered workbook equal an independent reader's. The test also checks the run against the last row of
  `repro/barrick/expected.toml` (root, anchor, totals, registry epoch, digest of digests). The root is a
  regression value, not independent; the last two can be recomputed without this code (E829). Rows are
  append-only, and a default-CI test checks each row's form and that its anchor derives from its root.
  - **Ledger:** 34 visible AISC tables, 2,595 instances. Footing: 1,004 (i), 4 (iii), 6 (v), 24 not published.
    Per ounce, after the stated share rule (E826): 1,379 (i), 3 (ii), 82 (iv), 57 (v), 36 not published. Before
    it, the counts were 1,116 (i), 17 (iv) and 385 (v).
  - **Alternative construction:** 1,008 footing and 1,257 rate instances consistent.
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
- `PO-PUB-5` (stated share rule, BD-331.S3, MOR_CASE E826):
  - **Real files** (ignored test `barrick_share_rule_matches_census`): of the 379 instances refused before, the 328
    site instances split into 263 (i) and 65 (iv). The 51 region instances stay (v).
    The counts and per-subject fit/miss equal the independent census. On every instance, Rust also equals it on
    state, subject, footnote cell and label cell; that check was run outside the test.
  - **Synthetic:** K14 fits at 3/4, and L14 misses with the interval [0.8210, 0.8290]. Further tests cover a
    mismatched or missing label, a moved footnote letter and a removed footnote, a region, and a dropped share.
- `PO-PUB-6` (state-type completion and hidden scope, BD-331.S2, MOR_CASE E828):
  - **Real files** (ignored tests `barrick_hidden_scope_matches_census`, `barrick_every_iv_names_missing_fact`):
    6 hidden tables, 435 instances. Footing: 169 (i), 1 (iii, North America Q4 2023 TCC, 541 against 540), 4 not
    published. Per ounce: 235 (i), 17 (iv), 3 (v), 6 not published. The counts equal the hidden census; on every
    instance, Rust also equals it, a check run outside the test. Each of the 82 visible and 17 hidden (iv) names a
    missing fact. E816's three (ii) are reconstructed.
  - **Synthetic:** M16 is a stated (ii), and a changed or empty quote keeps it (iv). The hidden `Q4 2029` table's
    (iii) is reported and never counted. A changed hidden cell moves the root and no visible state. Hidden sheets
    leave the census and its digest unchanged.
- `PO-PUB-8` (live dependency, files; BD-338.S1; default suite, `live::tests`):
  - **Positive control:** on the three-file synthetic registry, 3 files × 3 cases give 9 checks, and every
    one refuses as required.
  - **Mutants:**
    - M3, a loader that restores the intact bytes on refusal, fails each file with "file deletion did not
      refuse: <file>".
    - M4, re-raising under an uncatalogued code, fails every check with "refusal exit 2 is not 8".
  - **Unreadable file:** a chmod 000 file is named under "missing". As root, the test says it skipped.
  - **Lane:** the synthetic registry never reports the public lane or gate eligibility.
- `PO-PUB-9` (live dependency, leaves; BD-338.S2; default suite, `live::tests`):
  - **Positive control:** every leaf of the 50 synthetic instances passes or is skipped with a reason. Skips
    fall only on Site F's unrounded column and on E14's three non-cause leaves (hand-read from
    `synthetic.rs`).
  - **Mutants:**
    - M1, C8's certificate keeping the literal 350, is named "value did not move";
    - M2, B8 citing the unused P5, is named "dead ancestor".
  - **Flip margins (hand-derived):**
    - (79, 1, 68) flips at 1145 and 1179;
    - B8 401 gives residual 1;
    - E7 → 0 evaluates E14 as (i) and E8 as (iii);
    - F5 → 1 ends the all-zero column;
    - a broken `Minestats3!B2` makes H16 (iv) naming the cell.
  - **Edits:** a byte-level edit and a parsed-level edit give the same census and instances.
- `PO-PUB-7` (K.4 one-cell check, bd-prmn.8, MOR_CASE E836):
  - **Real files** (ignored test `barrick_one_cell_matches_independent_reader`): 519 columns give 374 pass,
    82 fail and 63 refused. On every column, every field equals `scripts/oracle/one_cell_cos.py` (same origin),
    including the summary. The failing columns are exactly the 82 cost-of-sales-per-ounce (iv). Of the 82, 17 are
    K.4 items and 65 are not, so the verdict is "basis rule". The root does not move.
  - **Synthetic:** 10 columns, and Site I fails at both bases with known implied ounces. A share registered
    wrong for Site H (3/5, labelled 60%) fails at that basis and gives "basis rule".

## 7. Failure Modes & Deterministic Fallbacks
- A sheet with a cost-of-sales row but no locatable structure becomes a `TableRefusal`.
- A header that names no registered subject becomes an `UnregisteredHeader`.
- Neither stops the run, and neither is ever guessed.

## 8. Gates & Kill Criteria (`GATE-*` / `KILL-*`)
- `GATE-BULL-01` (parts 1 and 2; part 3, issuer consistency across vintages, is not yet scored)

## 9. Lifted Code & Foundation Dependencies
- Depends on `fmine-core`, `fmine-evidence`, `fmine-claim`, `fmine-io` (and, in the manifest, `fmine-units`,
  `fmine-time`, `fmine-metrics`, not used by this crate since BD-321).
- Depends on `fmine-numerics` (BD-331.S6): the ledger's exact kernel (`exact_feas`, BD-340).
  `tests/exact_feas_oracle.rs` keeps the old closed-form corner rule as an independent oracle for rates (at a
  share, and the (iv) range exactly); `test_property_footings_match_closed_form` does the same for footings.

## 10. Non-Goals & Scope Boundary
- Operates only on public disclosures (`ConfidentialityTier::Public`).
- Makes no reserve, resource, geotechnical, regulatory or investment claim.
- Implies no issuer affiliation.
