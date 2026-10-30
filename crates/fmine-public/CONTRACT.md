# CONTRACT.md — `fmine-public`

## 1. Charter & Layer
- **Crate:** `fmine-public`
- **Layer:** `V` (Verification, Ingestion & Assurance)
- **Subsystem:** `Assurance`
- **LOC Budget:** `14k planned / 16k ceiling` (`registries/loc_budget.toml`)
- **Allowed Module Families:** `["sources", "aisc", "mine_stats", "mine_ledger", "synthetic", "live", "citation", "key"]`

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
  repeated within a block are refused by locator; the first row of a repeated label is listed
  (`MineStatsRead::repeated`). With the issuer's `[[mine_stats_label]]` vocabulary (Barrick:
  56 pairs from W1's read, frozen before the Q3 2026 release), a data row whose `(section, label)` is not
  registered is refused too. W1-DR-01 is data (`[[printed_share_correction]]`, read by `mine_ledger`), and so is
  W1-DR-02 (`[[mine_stats_label_alias]]`, read by `mine_ledger`); other units are not. The region map is
  (`[[region_member]]`, BD-331.S4); values do not carry its version, and `mine_ledger` asks it by release.
- `mine_ledger` (BD-331.S4, bd-prmn.4): five Mine Stats identity families anchored on the `QTD`/`YTD` summary
  sheets, decided in exact arithmetic. Ownership: attributable = s × 100% per site row and column group, s the
  share the label prints or its registered correction (the printed share kept beside); regions and metal totals
  pose none; a row with no printed share is refused, never read as 100%. Quarter sums: H1 = Q1 + Q2,
  9M = H1 + Q3, FY = 9M + Q4 on the 100%, attributable and sold columns, anchored on each `YTD` cell. Summary
  against site sheet (family 7): each summary value of attributable production and sales, cost of sales, TCC or
  C1 and AISC equals the same site's value on a `Minestats*` sheet of the metal (A1 title `(Gold)` or
  `(Copper)`), same release, period and year (`SITE_FIELDS`), with a registered label alias applied by release;
  subjects with no site-sheet column pose none. Sites to region (family 3): a region row = Σ its registered
  members in the release's map on the gold flow columns, a member found by its subject or its registered Mine
  Stats label (refused at the first row when printed under both); a region with no member in the map is refused
  (`region_not_in_map`). Region to total (family 4): `Total Gold` = Σ the map's regions, `Total Copper` = Σ the
  copper sites in row order. A sum's nil token is exactly 0; a sum is decided alone by |Σ parts − total| ≤
  Σ half-steps (the kernel's answer for one rounded-once sum, checked by a test) and never posed jointly. The
  Nevada Gold Mines LLC sub-sum is not posed (it needs a registry table). Exclusions by name come first (a
  `withheld` status covering an operand's period, or a region member's; a region whose members differ between
  the operands' releases; a site on two site sheets of one metal), then a missing row or release, a label printed
  twice in a block, a blank, all placeholders (not published), some placeholders, an unreadable number. On a
  site sheet only `—`, `— ` and `n/a` are tokens; the nil token against a number reads as number = 0, decided
  alone (`Instance::nil_token`, E116). Each value as displayed stands for its closed box [v − step/2, v + step/2],
  the step from the displayed decimals (General non-integers are refused). Every decided instance but the sums is
  then posed jointly; each infeasible component reports every minimum correction set (an exhaustive search up to
  32 cells within the kernel's run cap) and the range each of its cells can take, never a corrected value, and
  whether it is infeasible only jointly.
- `synthetic`: invented workbooks in the real layout, seeded with one known answer per state. They are never
  gate-eligible. `write_issuer_workspace` writes them, plus a later title-only vintage, with a digest registry
  under the reserved issuer name `synthetic`; `run_public_assurance` runs that name in the synthetic lane.
- `lib`:
  - `load_registered_sources` verifies every registered file;
  - `assess` builds the ledger and the state root;
  - `assess_memo` is `assess` with a `MineStatsMemo` (bd-r7lp). The memo reuses the Mine Stats reads and ledger
    of its last computed run when the profile and every Mine Stats source's name, quarter and parsed workbook
    are equal. The live proofs use it, because their perturbations move only AISC cells. A synthetic sweep of
    every one-cell edit checks that its anchor, reads and ledger equal `assess`'s;
  - `PublicAssuranceReport::to_json` is the envelope payload;
  - `cell_to_public_claim` is the public-basis `Claim` builder (`BD-058`).

## 3. Authority, Epistemics & Representation
- Every reported exception cites file, file SHA-256, `sheet!cell`, subject, family and ownership basis
  (`IdentityFamily::basis`).
- The `LedgerAnchor` state root (BLAKE3 over an `M0AS` v7 frame) binds:
  - every input's SHA-256, numeric digest and census, and (v5, bd-jg43.3) its displayed-value digest with the
    shown, token and refused counts (`Workbook::displayed_census`);
  - every instance's state as codes, numbers and locators (`encode_state`), never its prose, with a tag before
    each optional part;
  - each instance's operands (role, locator, value);
  - unregistered headers and table refusals;
  - the same for the hidden scope, in its own section;
  - (v6, bd-prmn.4) the Mine Stats ledger in its own section: each instance's family, subject, field, group,
    period, anchor, cells, state code, reason and evidence, share and component, then each infeasible
    component's cells, flags and correction sets with their implied ranges. Its counts stay apart from the
    AISC totals. v7 adds the site-sheet family and each instance's nil-token certificate. The sum families
    (families 3–4) add instances in the v7 frame, with no change to its format.

  Changing any input cell moves the root; rewording a message does not (bd-sjrh.1 part 4).
- The report payload carries `lane` and `gate_eligible`. The no-claim line (`NO_CLAIM`), the four INV-002 flags
  and the next actions (`PublicAssuranceReport::next_actions`) are envelope fields, which `fmine-cli` prints.

## 4. Public API Surface
- `sources::{SourceKind, RegisteredSource, RegisteredReconstruction, RegisteredShare, RegisteredMissingFact,
  IssuerProfile, load_issuer_profile, registry_path, registered_issuers, is_issuer_name, is_through_bound}`;
  `IssuerProfile::{from_registry, through}`; `IssuerProfile::{regions, shares}` (BD-331.S3);
  `RegionMemberRow`, `IssuerProfile::{region_members, members_of}` (BD-331.S4: a region's registered members
  for a release, in registry order);
  `IssuerProfile::missing_facts`, `RegisteredReconstruction::basis` (BD-331.S2);
  `ExpectedRoot`, `expected_roots_path`, `load_expected_roots` (BD-339.S1);
  `kit_selectors_markdown`, `kit_readme_section`, `KIT_SELECTORS_BEGIN`, `KIT_SELECTORS_END` (BD-339.S4: the
  generated section of `repro/<issuer>/README.md`, from the row anchors and the registry; a golden test
  compares)
- `PublicAssuranceReport::{registry_epoch, census_digest_of_digests, check_expected}` (BD-339.S1)
- `aisc::{IdentityFamily, IdentityState, IdentityInstance, FootingCertificate, UnregisteredHeader, AiscTable,
  RateMiss, ShareBasis, ExplanationBasis, DEFAULT_MISSING_FACT, SHARE_MISSING_FACT, RowAnchor, AnchorRule,
  ConsistentBasis, MemberWeighting, WeightedMember, REGION_MISSING_FACT (BD-331.S4),
  ROW_ANCHORS (the seven rows `locate_table` anchors on, BD-339.S4), locate_table,
  ledger_for_table, rate_holds, rate_range, admitted_rates, KernelIndeterminate, feasible_share,
  feasible_share_text, find_share_footnote, find_qtd_sheet, confirm_share_label, reconstruction_holds,
  match_subject, cell_text,
  OneCellCheck, OneCellOutcome, OneCellVerdict, FailGroup, implied_ounces, one_cell_checks, one_cell_verdict}`
  (bd-prmn.8); `PublicAssuranceReport::one_cell`, the payload's `one_cell_cos` section.
- `aisc::{Operand, OperandRole, OperandValue, operand_at}` and `IdentityInstance::operands` (bd-sjrh.1, part 2
  of BD-329.S1). Each instance cites every cell its state was computed from, with its role (total, line,
  rate, ounces, numerator, NCI line, component, label check, member rate, member ounces) and its value as read.
  - **Cited cells:**
    - the table cells of the instance's family;
    - every used cell, for an all-zero column;
    - the footnote and the confirmed share label, for a cost-of-sales rate with a non-zero NCI line;
    - each printed member's rate and ounces, for such a rate of a region with registered members;
    - a consulted reconstruction's quote, label-check and component cells.
  - **Not cited:** structure cells (row labels, subject headers, a sheet's title and period line).
  - **Not yet in the payload or the state root.**
  - **Test:** `test_operands_cover_every_input_cell` perturbs every other synthetic cell, one at a time, and
    fails on any state change it causes that the operands do not cite.
  `IdentityState::Consistent` carries `basis`: `ConsistentBasis::Share` (the stated share, footnote and label
  it used), `ConsistentBasis::Members` (a region's weighting, `relaxation_only`: no conflict shown, no value
  proved; the report counts these), or `None`;
  `IdentityState::Explained` carries `basis: ExplanationBasis`; `IdentityState::Undecidable` carries
  `missing_fact` and `closing_document`; `IdentityState::RefusedAt` is (v) naming a cell or file.
- `mine_stats::{View, MineStatsCell, MineStatsValue, CellRefusal, MineStatsRead, LabelVocabulary, read_mine_stats,
  strip_footnote, printed_share, site_name, is_m0_target}`
- `mine_ledger::{mine_ledger, MineLedger, LedgerRules, LedgerFile, Instance, Family, Group, Metal, State, Reason,
  Component, Correction, FLOW_FIELDS, SITE_FIELDS, range_text, rat_text}`; `PublicAssuranceReport::mine_ledger`,
  the payload's `mine_stats_ledger` section (counts by family and state, (v) by reason, discrepancies with their
  certificate, and each infeasible component with its correction sets and implied ranges).
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
  - **Threads** (bd-r7lp): the checks are independent runs, spread over `available_parallelism()` scoped
    threads, so `run` must be `Sync`. The result is in instance and operand order, equal to a sequential pass
    (`test_live_leaves_threaded_matches_sequential`).
  - **Not yet:** hidden-scope instances.
- `live::{prove_leaves_in, one_leaf_per_instance, prove_labels, LabelCheck}` (BD-338.S3): the real-lane
  frame and the label sample.
  - `prove_labels` mutates each of the seven anchor labels of a located table (append "x"; prepend it for the
    ounces prefix). Each must give a table refusal or changed instances.
  - **Real lane** (ignored, local runs):
    - files: 54 of 54;
    - leaves (one per instance): 2,492 pass, 0 fail, 103 skipped, all `nci_not_decisive` (2026-10-30; it was
      2,444, 0 and 151 at 38a62ae, and the change is not attributed commit by commit);
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
    record per counted instance: 55 synthetic (column N, bd-jg43.3) and 2,595 real.
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
  `PublicAssuranceReport`, `load_registered_sources`, `assess`, `assess_memo`, `MineStatsMemo` (`hits`,
  `misses`), `run_public_assurance`, `run_barrick_m0_assurance`,
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
- The ledger scores displayed values (bd-jg43.3, E838): the declared construction sums the numbers as printed,
  so a numeric cell enters as `fmine-io`'s displayed value under its number format, never as the stored double.
  A whole displayed number (d = 0) enters as itself and a zero section's token (the dash) as 0. An operand
  with no whole displayed number is still a printed number, and its instance is refused (`RefusedAt`), naming
  the cell and the format code, for a General non-integer, a format with decimals or a percentage shown, or a
  format the interpreter refuses (a date, a scaling comma, a condition, an unknown id).
- Instance-level refusals (state (v)) carry a reason:
  - an operand with no whole displayed number (above);
  - a non-numeric line;
  - one of rate and ounces being zero.
- Cost of sales per ounce in a column with a non-zero non-controlling-interest line (BD-331.S3) is checked at the
  subject's registered share, s: 1000·s·N/Q on exact corners. A fit is (i), and its `basis` cites the share, the
  footnote and the label. A miss is (iv), giving the feasible share interval, rounded outward, and the missing
  fact. The instance is (v) instead when:
  - the sheet has no attributable-basis footnote, found by text;
  - the subject is a registered region with no members registered for the release ("regional numerator needs
    member weighting");
  - the subject has no registered share;
  - the same-release Mine Stats quarter-to-date label is missing, repeated or prints another share
    (`RefusedAt`, naming the cell);
  - an operand is not positive.
- Such a rate of a region with members registered for the AISC file's release (BD-331.S4), on a sheet with the
  footnote, is checked against its members, weighted by attributable ounces sold. A member printed on the sheet
  is weighed when its rate and ounces are whole displayed numbers with ounces not zero, and left out by name when
  both are zero (a displayed 0 or a zero-section token) or both placeholders. A registered member the sheet does
  not print is named as absent. The weighed members' ounces must sum to the region's (component sum). Then
  `exact_feas::region_against_members` poses the weighted mean with no numerator (the table prints consolidated
  cost of sales, not the attributable numerator). Its McCormick relaxation either refutes the rate with a
  checked certificate (iv, `region_outside_members`, carrying the weighting and the certificate digest, with
  `REGION_MISSING_FACT`), or does not: (i) with `ConsistentBasis::Members`, `relaxation_only`. The rounded-once
  alternative is reported, never scored. The instance is (v) instead, naming a cell where it has one, when:
  - a member's rate or ounces is anything else, or its ounces are negative (`region_member_operand`, at the
    rate's cell when the rate is not whole, else the ounces'), or a member heads two columns (at the second
    header);
  - no member is weighed (`region_no_members`);
  - the members' ounces do not sum to the region's (`region_ounces_do_not_foot`, at the region's ounces).
- A stated (ii) whose quote cell no longer starts with the registered prefix is refused as an explanation: the
  instance stays (iv), naming the quote cell and what it reads.

## 6. Proof Obligations (`PO-*`)
- `PO-PUB-1` (real files through 2026Q2, ignored test; recorded in MOR_CASE E820–E823): the census and digest of
  every registered workbook equal an independent reader's. The test also checks the run against the last row of
  `repro/barrick/expected.toml` (root, anchor, totals, registry epoch, digest of digests). The root is a
  regression value, not independent; the last two can be recomputed without this code (E829). Rows are
  append-only, and a default-CI test checks each row's form and that its anchor derives from its root.
  - **Ledger:** 34 visible AISC tables, 2,595 instances. Footing: 1,010 (i), 4 (iii), 24 not published.
    Per ounce, after the region rule (E840): 1,426 (i), 3 (ii), 92 (iv), 36 not published, none (v); 41 of the
    (i) are `relaxation_only`. Before the stated share rule (E826) the counts were 1,116 (i), 17 (iv) and 385
    (v); E838 (displayed values) and E840 took the rest out of (v).
  - **Alternative construction:** 1,014 footing and 1,304 rate instances consistent.
  - **Identities:** each (ii) and (iii) instance is identified.
- `PO-PUB-2` (synthetic): every seeded answer is reproduced, including the sign-flip witness. Perturbing one cell
  moves the state root and the affected state. A wrong component label leaves the instance in (iv). Column G's
  stored 100.4 under `#,##0` is scored as the displayed 100, and column N's General non-integer and date-format
  ounces are refused by cell and format code (bd-jg43.3).
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
    site instances split into 263 (i) and 65 (iv). The 51 region instances go to the region rule (PO-PUB-10).
    The counts and per-subject fit/miss equal the independent census. On every instance, Rust also equals it on
    state, subject, footnote cell and label cell; that check was run outside the test.
  - **Synthetic:** K14 fits at 3/4, and L14 misses with the interval [0.8210, 0.8290]. Further tests cover a
    mismatched or missing label, a moved footnote letter and a removed footnote, a region, and a dropped share.
- `PO-PUB-6` (state-type completion and hidden scope, BD-331.S2, MOR_CASE E828):
  - **Real files** (ignored tests `barrick_hidden_scope_matches_census`, `barrick_every_iv_names_missing_fact`):
    6 hidden tables, 435 instances. Footing: 169 (i), 1 (iii, North America Q4 2023 TCC, 541 against 540), 4 not
    published. Per ounce: 237 (i), 18 (iv), 6 not published. The counts equal the hidden census, whose three
    regional (v) the region census decides (PO-PUB-10); on every instance, Rust also equals the hidden census, a
    check run outside the test. Each of the 92 visible and 18 hidden (iv) names a missing fact. E816's three (ii)
    are reconstructed.
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
    including the summary. The failing columns are exactly the 82 cost-of-sales-per-ounce (iv) other than the ten
    regions refuted by their members (PO-PUB-10), which the check leaves unbased. Of the 82, 17 are K.4 items and
    65 are not, so the verdict is "basis rule". The root does not move.
  - **Synthetic:** 10 columns, and Site I fails at both bases with known implied ounces. A share registered
    wrong for Site H (3/5, labelled 60%) fails at that basis and gives "basis rule".
- `PO-PUB-10` (region against members, BD-331.S4, MOR_CASE E840):
  - **Real files** (ignored test `barrick_regions_match_region_census`): the 51 visible and 3 hidden regional
    rates the stated-share census leaves to member weighting are decided: visible 41 not refuted (i,
    `relaxation_only`) and 10 refuted (iv); hidden 2 and 1. Each instance's verdict, reason, cell and used,
    absent and excluded members equal `scripts/oracle/region_census.py` (57a5aa5, an exact vertex check written
    before the Rust and without reading it), run in the test. The 41 are also consistent under the rounded-once
    alternative (the kernel's result; no independent reader computes it).
  - **Synthetic** (default suite): Region North over Site H and Site I is not refuted at 2099, 2100 and 2101,
    and refuted at 2098, 2102, 1900 and 2300, where the exact bound is [2099⅙, 2100⅚]. Every refusal and
    exclusion names its cell or member; a footnote-less sheet and a release before the members' first keep the
    earlier refusals.
- `PO-PUB-11` (Mine Stats ownership, quarter sums, summary against site sheet and the sum families, BD-331.S4,
  bd-prmn.4, MOR_CASE E841, E842, E843):
  - **Real files** (ignored test `barrick_mine_ledger_matches_census`): 10 releases, 5,174 instances. Ownership
    592: 470 (i), 29 (iii), 73 (v), 20 np; quarter sums 942: 883 (i), 9 (iii), 26 (v), 24 np; site sheet 3,130:
    2,982 (i), 1 (iii), 42 (v: 40 withheld, 2 blank), 105 np; sites to region 306: 282 (i, 41 by the nil token),
    24 (v: withheld, E818); region to total 204: 204 (i). 2,091 components, 27 infeasible (2 only jointly),
    every one with all its minimum correction sets. Every instance line, component line, summary and implied
    range equals `scripts/oracle/mine_stats_census.py` (e66f7dc; family 7 a736c26; families 3–4 c697b13, each
    written first by a separate reader, exact Fourier–Motzkin or exact sums over fractions), run in the test,
    which compares a sum's member cells as a set (the census orders a map's members by its rule, the ledger by
    registry row) and pins the sum families' counts. The plan's known answers hold:
    E113, E114 and E115 are each a pair of (iii) on one component whose one minimum set is the shared cell,
    E113's B5 can take 281, and E116 is one (iii) site-sheet instance decided by the nil token, alone.
  - **Synthetic** (default suite): the E113 shape (one shared cell, the one minimum set); a component
    infeasible only jointly, with its three minimum sets and ranges derived by hand ([201/4, 201/4] twice,
    [202, 203]); an unprinted share refused where 100% would hold; a registered share with the printed one
    beside; `withheld` and region-map exclusions; placeholders, blanks and a repeated row; the same stored text
    decided by its displayed format; row and file order changing no state; a sign flip, a dropped share and a
    missing release moving the state. Family 7: a print pairing by label as matched, (iii) posed jointly with
    both prints as its sets; the nil token as zero ((iii) at 5, (i) at 0) and `n/a`; another nil spelling
    unreadable; the refusal order (two sheets, withheld, missing row, blank, unreadable); the W1-DR-02 alias by
    release, and beside its registered row; a label printed twice; a site print making an ownership component
    infeasible only jointly, its two minimum sets and ranges ([399/4, 401/4], [201, 201]) derived by hand. Sum
    families: the slack bound on both sides (148–152 against 150 with slack 1.5; Total Copper 54–60 against 57
    with slack 2, the bound included), a region discrepancy that neither reaches the total nor forms a
    component; the nil token as exactly 0, all nil (np), `n/a` and a blank; a member found by its registered
    label, missing without it, and refused at the first row under both names in either order; a region outside
    the map, a missing member and a map region with no row; a withheld member by period; the closed form
    against the kernel on 400 pseudo-random sums of mixed steps (both outcomes at least 50 times).

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
