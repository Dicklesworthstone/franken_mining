# R4: Accounting fit for FrankenMining on franken_accounting

**Question.** FrankenMining (`franken_mining`) is an operating system for an integrated gold/copper/silver/tin miner (Barrick/Newmont class), and its accounting is to be done **using franken_accounting** (FA). This report covers four things. It sets out FA's exact vocabulary and how FA expects domain systems to integrate. It records the state of FA's code as of 2026-10-28. It assesses how well FA fits mining-specific accounting. It proposes a grounded integration design.

**Sources.** All 2,096 lines of `/Users/jemanuel/projects/franken_accounting/COMPREHENSIVE_PLAN_FOR_THE_DESIGN_OF_FRANKENACCOUNTING.md` (cited below as "plan §x" or "App. X"). I also read the workspace `Cargo.toml`, the registries (`units.toml`, `conversion_laws.toml`, `lot_laws.toml`, `allocation_laws.toml`, `intents.toml`, `entry_classes.toml`, `fx_laws.toml`, `reason_codes.toml`, `relationship_bases.toml`, `account_roles.toml`, `evidence_policy.toml`, `verbs.toml`), the README, and these source files: `fa-canon/src/ir.rs`, `fa-model/src/rea.rs`, `fa-model/src/ids.rs`, `fa-statements/src/concordat.rs`, `fa/src/consolidation.rs`, `fa/src/fx.rs`, `fa/src/external_subledger.rs`, `fa-registers/src/lots.rs`, `fa-intents/src/lib.rs` and `fa/src/book.rs`. I modified nothing in the surveyed repo.

**Surveyed HEAD.** `53c6e953` (2026-10-28).

---

## 0. Verdict in one paragraph

FA is a strong fit as FrankenMining's **book of record**. It provides an immutable, bitemporal, evidence-native double-entry journal with exact arithmetic, period law, restatement lineage, certified and replayable reports, offline-verifiable proof bundles, and an agent-first CLI/MCP. FA describes itself as **only the mechanical layer**: "Questions that require professional judgment ... are the province of agent skills and human professionals that *act on* this system" (plan §0.2). It is "deliberately **not** an ERP (no procurement, inventory, payroll ...) — though their subledgers can be modeled as books that post into it" (plan §0.3).

Almost every mining-specific accounting topic is therefore one of two things:

- **(a)** expressible with FA's general mechanisms, with FrankenMining supplying the computation, the estimate and the evidence; or
- **(b)** absent, so FrankenMining must own it as a domain pack.

FA has **no plugin or domain-pack concept**. Its extension points are:

- journaled in-book data: chart, dimensions values, Canon `RuleVersion`s, `StatementDefinition`s, policies, parties, instruments and ownership edges;
- compiled registries that live in the FA repo: units, conversion/allocation/lot/FX laws, reason codes, document kinds, event kinds, dimension kinds, entry classes and procedures.

The second group means several mining needs require **upstream changes to FA**. The most urgent are: metal and mass units, custom REA event kinds, custom dimension kinds, tiered-NCI and cross-book consolidation, commodity price documents, and a more expressive Canon.

The code is real but not finished (~793K lines of Rust, ~4,700 `#[test]` functions). FA's own reality check (plan §20.1) still records a large share of the plan as **PARTIAL / UNPROVEN / NOT_STARTED**. That includes multi-book adjustment layers, cross-book consolidation with translation and CTA, the CloseRun DAG, real-book-scale operability, and every performance target.

---

## 1. What franken_accounting is (plan §0–§3, condensed)

FA is "a universal, evidence-native, double-entry accounting system of record in memory-safe Rust, built on FrankenGraphDB and the Franken/asupersync ecosystem" (plan title block). It composes **six bets** (plan §0 table):

| Bet | Name | One-line meaning |
|---|---|---|
| A1 | **One Posting Universe** | No balance rows; every balance, register, TB and statement is a deterministic **fold** over the immutable entry stream at a `(book, branch, known-at, effective)` coordinate. |
| A2 | **Bitemporal by Construction, Period Law Mechanized** | Effective civil date vs. recorded `CommitSeq`. "As known at" and "as currently restated" are selectors. Close, reopen and restate are journaled controls, never flag flips. |
| A3 | **Evidence-Native and Outsider-Verifiable** | Every entry carries a content-addressed provenance closure. Bundles verify offline with `fa-verify`. |
| A4 | **The Ledger Is a Graph (REA, not rows)** | Accounts, entities, parties, instruments, documents and findings are vertices. Forensics are graph queries plus `fnx` algorithms. |
| A5 | **Exact and Deterministic as a Product Feature** | `Amount { unit, mantissa: i128 }`. Every rounding, allocation and FX conversion names a registered law. Reports carry replayable certificates. |
| A6 | **Agent-Native with Counterfactual Branches** | Every write is a typed intent with a dry-run plan. Agents work on drafts and scenarios. One contract drives the CLI (robot mode) and MCP. |

**The five readers** (plan §0.1; worked workflows in §15.6):

1. the CPA auditor (financial-statement audit, SOX 404);
2. the regulator and examiner;
3. the litigation consultant and expert witness;
4. the forensic accountant and investigator;
5. the AI agent, "the day-to-day operator of the books".

For a miner, these map onto the external auditor, the securities regulators (SEC S-K 1300, OSC/NI 43-101), the tax authorities and host governments, JV partners exercising audit rights, streaming and royalty counterparties and lenders, and FrankenMining's own agents.

**Boundary with judgment** (plan §0.2): "FrankenAccounting never silently applies a judgment (it never auto-balances, never picks an account, never rounds without a law ...)". Skills "plug in through Canon posting rules (§10), typed intents, and the MCP surface (§15), and the system records *which* rule version and *which* principal made each call." **This sentence is the integration contract FrankenMining must design to.**

**Anti-goals** (plan §0.3) that bound what FrankenMining can expect: no ERP, no inventory workflow, no tax engine, no GAAP interpreter, no payments, no UI in core, no floats, no mutable balances, no edit-in-place, no deletes, and no closing a period by setting a flag.

---

## 2. Exact vocabulary FrankenMining's plan should use

### 2.1 Subsystem (codename) names

| Codename | Plan section | What it is |
|---|---|---|
| **Folio** | §5 | Posting kernel, the "only writer of the ledger graph". Pipeline: `ADMIT → RESOLVE → CANON → ABACUS → LAWS → PLAN → COMMIT → ACK → DERIVE` (§5.1). |
| **Abacus** | §6 | Exact money algebra: amounts, rounding laws, allocation laws, FX laws, quantities and lots. |
| **Almanac** | §7 | Time, fiscal calendars, the period state machine, selectors, the close. |
| **Palimpsest** | §8 | Entry classes, supersession lineage, restatement cases, erasure and holds, migration. |
| **Docket** | §9 | Documents, provenance closure, chain of custody, attestations, bank lines. |
| **Canon** | §10 | Posting-rule language and typed intents. |
| **Lantern** | §11 | Forensic and audit procedures, findings, tracing laws. |
| **Concordat** | §12 | Multi-book, intercompany, consolidation, FX translation and remeasurement (with Statements). |
| **Notary** | §13 | Journal accumulator, checkpoints, bundles (`.fab`), certificates, `fa-verify`. |
| **Steward** | §14 | Capabilities, caveats, SoD, profiles, access audit. |
| **Teller** | §15 | Agent and human surface: embedded API, `fa` CLI, `fad` daemon, `fa-mcp`. |

### 2.2 Crates (`fa-*`, plan §18.1) and what exists today

| Layer | Planned crates | Present at `53c6e953`? |
|---|---|---|
| Foundation | `fa-types`, `fa-money`, `fa-time`, `fa-registry`, `fa-claim`, `fa-sim-harness` | `fa-money`, `fa-time`, `fa-registry` present; **`fa-types`, `fa-claim`, `fa-sim-harness` absent** |
| Model | `fa-model` | present |
| Laws (pure, stripped `Cx`) | `fa-canon`, `fa-policy`, `fa-intents` | present |
| Kernel | `fa-journal`, `fa-registers`, `fa-escrow`, `fa-palimpsest` | present (`fa-escrow` 240 LOC, `fa-palimpsest` 1.5K LOC) |
| Evidence | `fa-docket`, `fa-notary` | present |
| Analytics | `fa-statements`, `fa-concordat`, `fa-lantern`, `fa-query` | **`fa-concordat` absent**: Concordat lives in `fa-statements/src/concordat.rs` plus `fa/src/consolidation.rs` |
| Governance | `fa-steward` | present (107 LOC; evaluation lives in `fa-policy`) |
| Interop | `fa-formats` | present |
| Surface | `fa-verbs`, `fa` (embedded facade), `fa-demo`, `fa-cli` (binary `fa`), `fad` (daemon), `fa-mcp` | present |
| Verification | `fa-reference`, `fa-sim`, `fa-oracles`, `fa-gen`, `fa-bench`, `fa-conformance`, `fa-fuzz`, `fa-verify` | **`fa-sim` absent**; others present |
| Tooling | `tools/registry-check`, `tools/xtask` | present |

### 2.3 Core objects (plan §4.1, App. A)

**Ledger graph vertices:**

- `Book`, with `base_unit` (one functional currency per book, §0.4 decision 1), `gaap: {USGAAP, IFRS, Tax, Statutory, Management, Custom}` and `mode: Full | AdjustmentLayer(of)`;
- `LegalEntity`, `Account`, `Dimension`/`DimensionValue`, `Unit`, `FiscalCalendar`/`Period`;
- `Entry` (the atom), `Leg` (derived projection), `Register` (derived fold), `Party`, `Instrument`;
- `Event` (REA economic event), `Subledger`;
- control objects: `RestatementCase`, `DraftSet`, `CloseRun`, `Scenario`, `StatementIssuance`, `MigrationRun`, `Engagement`, `ProposedAdjustment`, `ProductionProfile`, `TimeEvidence`;
- versioned data: `StatementDefinition`, `RegisterDefinition`, `RuleVersion`, `Policy`.

**Evidence graph:** `Document`, `Attestation`, `BankLine`, `ImportReceipt`, `Principal`, `Approval`. **System graph:** `Capability`, `AccessEvent`, `Finding`, `Certificate`, `Bundle`, `DecisionCard`.

**Edges:** `HAS_LEG`, `POSTS_TO`, `FOR`, `IN_UNIT`, `CHILD_OF`, `CONTROLS`, `REPRESENTS`, `SUPERSEDES {kind}`, `RESOLVES {disposition}`, `DERIVED_FROM` (Entry→Event), `GENERATED_BY` (Entry→RuleVersion), `EVIDENCED_BY {role}`, `PREPARED_BY`/`APPROVED_BY`/`REVIEWED_BY`, `ATTESTS`, `SETTLES`/`MATCHES`/`UNSETTLES`, `OWNS {pct}`/`CONTROLS {basis}`/`RELATED_TO {basis}` (valid-time), `HELD_AT`, `MIRRORS`, `INTERCOMPANY_WITH {pair_key}`, `PAID` (derived), `CITES`, `IN_CASE`, `READ`.

**Handles** (§4.9): `EntryRef { vid, entry_digest }`, `ProposalRef`, `AccountRef { book, code }`, `PeriodRef`, `DocumentRef { oid }`, `FindingRef`, `CertificateRef`, `BundleRef`. Identities are **derived, never allocated** (for example `vid(Entry) = H(book ‖ principal_scope ‖ idempotency_key)`).

**Coordinate** (§4.8): `BookCoordinate { book, branch, known_at: Head | AsOfSeq | AsOfTime, effective: All | Period | Range | AsOf | YearToDate | InceptionToDate }`.

**Selectors** (§7.4): `AS KNOWN AT`, `FOR PERIOD`, `EFFECTIVE AT|BETWEEN`, `AT BRANCH`, `AS ORIGINALLY REPORTED`, `AS CURRENTLY STATED`, `AS ISSUED TO <audience>`, `AS REPORTED IN <certificate>`.

### 2.4 Entry classes, supersession, periods

- **Entry classes** (§8.1, `entry_classes.toml`): `Original`, `Pending`, `Resolution`, `Reversal`, `Replacement`, `Reclassification`, `Adjusting`, `Closing`, `Allocation`, `Revaluation`, `Elimination`, `Migration`, `Revision`, `Restatement`, `PriorPeriodAdjustment`, `PeriodControl` / `SchemaControl` / `PolicyControl`. The implementation adds further control classes (`TimeControl`, `SweepControl`, `ScheduleControl`, `MigrationControl`, `ImportControl`, `HoldControl`, `DraftControl`, `ScenarioControl`, `ProductionControl`, `IssuanceControl`, `EngagementControl`, `CaseControl`).
- **Supersession kinds** (§8.2): `Reverses`, `Replaces`, `Reclassifies`, `Restates`, `Revises`, `Corrects`, `Resolves(Pending)`.
- **Correction reason codes** (`reason_codes.toml`): `DataEntryError`, `WrongAccount`, `WrongPeriod`, `WrongAmount`, `Duplicate`, **`EstimateChange`**, `PolicyChange`, `Fraud`, `AuditAdjustment`, `RegulatoryDirective`, `Other`, `ScheduledAccrualReversal`. There are also ManualAssertion reasons (`NoDocumentIssued`, `ManagementEstimate`, ...).
- **Period states** (§7.3): `Future → Open → SoftClosed → Closed → Locked`, with `Reopen(case)`.
- **Period kinds:** `Normal`, `Adjustment`, `Opening`, `Closing`, `Historical`.
- **Calendars:** `Monthly12`, `Weeks445|454|544`, `Week5253`, `Custom`.
- **Admissibility:** App. C, `Period.state × EntryClass`. The headline rule: an `Original` is never admissible in `Closed` or `Locked`.
- **Restatement case lifecycle** (§8.3): `Proposed → Determined → Rehearsed → Approved (dual control) → Posted → Disclosed → Closed`. Case kinds: `Revision | Restatement | PriorPeriodAdjustment | PolicyChangeRetrospective`.

### 2.5 Intents (plan §10.2, App. B)

The common header is `{ book, branch?, idempotency_key, dry_run, confirm_plan?, evidence?, description?, tags? }`. The 1.0 catalog:

- **Posting:** `PostEntry`, `PostBatch`, `RecordEvent`, `RecordInvoice` / `RecordBill`, `ApplyPayment` / `ApplyReceipt` (with typed `differences: Discount | ShortPay | Writeoff | BankCharge | RealizedFx`), `Reserve` / `PostReserve` / `VoidReserve`, `Accrue` / `Defer` (with `auto_reverse_on`), `Reclass`, `Reverse` / `Replace`, `Allocate`, `Revalue` / `Translate`, `Depreciate` / `Amortize`.
- **Reconciliation:** `MatchBank` / `ConfirmMatch` / `AssertBalance`.
- **Chart control:** `OpenAccount` / `CloseAccount` / `PromoteToSummary` / `ImportChart`.
- **Periods and close:** `OpenPeriod` / `SoftClose` / `ClosePeriod` / `LockPeriod` / `ReopenPeriod`, `RunClose`.
- **Restatement:** `OpenRestatementCase` / `RehearseCase` / `ApproveCase` / `PostCase` / `DiscloseCase`.
- **Evidence and migration:** `AttachEvidence` / `Attest` / `ImportDocument`, `Migrate`, `IssueStatements`.
- **Audit engagement:** `ProposeAdjustment` / `BookAdjustment` / `PassAdjustment`, `OpenEngagement` / `CompleteDocumentation` / `Represent`, `MarkExhibit` / `Produce`.
- **Governance:** `IssueCapability` / `Revoke` / `LegalHold` / `ReleaseHold` / `ErasureCut`.
- **Drafts and scenarios:** `ForkDraft` / `PromoteDraft` / `DiscardDraft` / `ForkScenario` / `DiffBranches`.

Every intent returns `{outcome, entry_refs[], plan_certificate, register_deltas[], warnings[], next_actions[]}`. `LockPeriod`, `PostCase`, `ErasureCut` and `Migrate` require `confirm_plan`. Amounts cross the wire as exact strings (`"1234.56 USD"`); a JSON number is refused.

### 2.6 Canon rule language (plan §10.3, as implemented)

The plan sketch is `rule <name> v<N> for event <Kind> { inputs {...} let ... debit "<acct>" <expr> dims {...} credit ... effective <date> evidence ... }`.

The plan's design rules: total, pure, no I/O, no clock, no entropy; the only loops are folds over declared inputs; every arithmetic operation names a rounding law; account references are by code, resolved at the effective date; units are type-checked; the compiler proves debit equals credit per unit. Rules are content-addressed `RuleVersion`s, **activated** by a `PolicyControl` entry with an effective date range under dual control, golden-tested, and backtested (`fa rule propose|check|eval|activate|derive|backtest|test`).

The implemented grammar is much narrower than the plan's sketch:

```text
rule sale_with_tax v1 for event Sale {
  inputs { as_of: date }
  let net = event.resource("net", "USD")
  let tax = net * 7 / 100 round HalfEven
  debit "1200" net + tax dims { cost_center = "CC10" }
  credit "4000" net
  credit "2300" tax
  effective as_of
}
```

Implemented IR limits:

- inputs are only `Date` or `Amount(unit)`;
- the amount nodes are `Literal`, `Resource`, `Input`, `Slot`, `Add`, `Sub`, and `Scale` by a **constant** rational;
- `x * y` with two variables is refused (see the `parse.rs` test vectors);
- there are no tables, no folds, no `account_map[...]` lookups;
- `produces` is only `Original | Adjusting` (`fa-canon/src/ir.rs:151`).

### 2.7 Embedded API, CLI, MCP

**Plan shape** (§15.2): `fa::Book::open(&cx, dir, keys)`, then `book.session(&cx, capability_token)`, then `sess.plan(&cx, Intent::...)` and `sess.commit(&cx, plan)`. The plan also lists `sess.trial_balance`, `sess.explain_balance`, `sess.fork_draft`, `sess.diff` and `sess.produce_bundle`, plus `fa_verify::verify(bundle, checkpoint)`.

**Actual shape today:**

- `fa::Book::{create_memory, create_dir, open_dir(dir, BookKeys, Entropy)}` and `fa::AsyncBook<V: Vfs, B: Vfs>`, which take `&PurposeContexts` and `&Capability`;
- `Book::post(PostRequest) -> Outcome<Posted>`, `Book::plan(..)`, `record_event(&EventSpec, principal, effective_date, idempotency_key, &Capability)`, `derive_rule_entry`, `open_draft`, `open_scenario`, `open_case`, `create_party|instrument|legal_entity`, `create_native_subledger`, `create_recurring`;
- `type Outcome<T> = Result<Result<T, Refusal>, BookError>` (`fa/src/book.rs:569`).

The Rust facade is still moving. **The stable contract is the verb registry.**

**CLI:** `fa <verb> [subverb] [args] [--book] [--as-known-at] [--for-period] [--branch] [--json|--robot] [--dry-run]`. The verb families are listed in §15.3. `fad serve|status|stop` is the single long-running process; it holds the book's writer lease and hosts MCP.

**Robot envelope** (App. D):

```text
{"v":1,"event":"<schema-id>","book":..,"coordinate":..,"data":..,"certificate":..,"warnings":[..],"next_actions":[{"command","why"}]}
```

**Exit codes:** `0` ok · `2` usage · `3` accounting-law refusal · `4` environment · `5` foundation/I/O · `6` conflict · `7` safety block · `8` verification failure. `1` is never emitted.

**Refusal object:** `{refusal_id, code: "FA-REF-NNN", exit, what_failed, where, why: {law|policy|matrix}, fix: {command|or_field_change}, alternatives, docs}`. Examples: `FA-REF-001 UNBALANCED`, `-002 NOT_A_LEAF`, `-005 DIMENSION_MISSING`, `-010 PERIOD_LAW`, `-013 AGENT_TRUNK`, `-032 IDEMPOTENCY_CONFLICT`, `-034 PLAN_INVALIDATED`, `-100 STRUCTURE`, `-154 UNIT_UNREGISTERED`.

**MCP clusters** (§15.4): `fa.orient`, `fa.book`, `fa.chart`, `fa.govern`, `fa.io`, `fa.post`, `fa.read`, `fa.time`, `fa.evidence`, `fa.forensics`, `fa.draft`, `fa.notary`, `fa.case`. The implementation also registers `fa.rea`, `fa.rules`, `fa.subledger`, `fa.schedule`, `fa.fx`, `fa.dimensions`, `fa.roles` and others.

MCP resources are `resource://fa/...`. The macros are `macro_record_with_evidence`, `macro_month_end_dry_run`, `macro_investigate_party` and `macro_reconcile_instrument`. Amounts cross MCP only as strings.

### 2.8 Evidence, claim, proof and governance vocabulary

- **Claim classes** (§1 constraint 11, §16.1): `invariant > proof > bounded_model > statistical > slo > benchmark`. Registries: `invariants.toml` (FA-INV-01…16), `evidence.toml` (`FA-EVID-*`, `FA-CAL-*`) and `slo.toml` (`FA-CFG-*`). A Lantern score is never an invariant (§11.9).
- **Invariants** (App. F):
  - FA-INV-01 Balance law (`per-unit`, `per-entity`, `base`, `base-residual`, `realized-fx`, `memo`, `leaf-only`, `unit-policy`, `dimensions`);
  - -02 Append-only;
  - -03 Period law (`nominal-zero`);
  - -04 Registers are folds (`mapping-partition`);
  - -05 Evidence closure (`evidence-minimum`, `external-retained`, `supplemental`);
  - -06 Exactness;
  - -07 Bitemporal visibility (`migration-disjoint`);
  - -08 Supersession lineage;
  - -09 Idempotent intents;
  - -10 Authorization before observation and SoD (`approval-binds-proposal`, `post-evaluation`, `ceiling`, `whole-entry`);
  - -11 Determinism and replay;
  - -12 Subledger–control reconciliation;
  - -13 Cross-book and consolidation conservation;
  - -14 Tamper evidence;
  - -15 Two-phase and limits;
  - -16 Projections equal payloads.
- **Document kinds** (`evidence_policy.toml`): `Invoice`, `Bill`, `Receipt`, `Contract`, `BankStatement`, `Check`, `WireConfirmation`, `Remittance`, `PurchaseOrder`, `ReceivingReport`, `Rate`, `Confirmation`, `Representation`, `BoardMinutes`, `CourtOrder`, `Memo`, `Email`, `Spreadsheet`, `Image`, `Disclosure`, `Workpaper`, `Other`.
- **Evidence roles:** `Source`, `Support`, `Rate`, `Contract`, `Statement`, `Approval`, `Other`.
- **Provenance grades** (§9.3): `ThirdPartySigned`, `Connector(transcript)`, `HumanUpload`, `AgentUpload`, `Generated`. An entry whose only evidence is `AgentUpload`/`Generated` is **self-sourced**: the Agent profile may commit it only on a draft, and promotion needs a `Reviewed { against }` attestation.
- **ImportReceipt sources:** `Upload{principal} | Connector{name, version} | Email{message_id} | Scan{device}`.
- **Attestation claims:** `Prepared | Reviewed{against} | Reconciled | RegisterEquals{...}` (the balance assertion), `Confirmed | Examined | Custom`, and in engagements `Represented`.
- **Proof scopes** (§13.1): `inclusion`, `consistency`, `recomputation`, `replay`, `membership_only`.
- **Completeness scope:** `SeqRangeComplete | ScopeIndexComplete | FilterAttested`.
- **Certificate completeness grades:** `Redacted` and `VerifiableIfArtifactsSupplied` (downgrades).
- **Capability `ops`** (§14.2): `read, post, post_softclosed, post_closed, approve, reconcile, close, reopen, restate, migrate, schema_change, rule_change, issue_capability, witness, verify, produce, hold, erase, privileged, receivership, partial_view`.
- **Caveats:** `books`, `entities`, `accounts ⊆ subtree`, `periods`, `dimensions`, `classes`, `amount ≤`, `daily_total ≤`, `known_at ≤`, `branch =`, `redaction`, `expires_at`, `requires_dry_run`.
- **Profiles** (§14.4): `Agent`, `Preparer`, `Approver`, `Controller`, `Auditor`, `Regulator/Examiner`, `Investigator`, `Receiver/Trustee`, `Production`, `Witness`.

---

## 3. How the plan expects external domain systems to integrate

The plan never says "domain pack" or "plugin". It does define six integration mechanisms. FrankenMining should use exactly these names.

1. **Typed intents through Teller** (§10.2, §15, App. B). Domain systems submit intents (plan, then commit), idempotently, from their own `Principal`, under capabilities. `PostEntry` is the escape hatch; "everything below could be expressed as it".
2. **REA Events plus Canon rules** (§4.1 `Event`, §4.6, §10.3). `RecordEvent` journals "the thing that happened" (kind, `occurred_on`, resources `{label, quantity/amount, instrument?}`, participants, evidence). Activated rule versions derive the entries, with `DERIVED_FROM`/`GENERATED_BY` lineage. Re-running the rule version over the event reproduces the entry byte for byte (FA-INV-05 rule-reproduction; implemented in `fa-journal/src/rule.rs`). The plan describes this as "subledger accounting rules as the ancestor of Canon" (§3.2, after Oracle SLA and SAP account determination). **This is FA's intended "domain system emits events, accounting policy derives entries" path.**
3. **Subledgers and control accounts** (§5.6, FA-INV-12). FA has two forms.
   - The *native* subledger is "one control leaf with a required `party` dimension plus the open-item graph — never an account per customer".
   - The *externally fed* subledger: "an external AR system posts a summary entry that must name the detail batch digest it summarizes; a batch whose detail does not fold to the summary is refused ... an externally fed subledger's *detail* is not in the book at all (it is an attested document whose digest the summary entry names)".

   This is implemented as `fa::external_subledger`. The detail is a canonical external-detail `Workpaper` reviewed by an independent principal, and an `Original` summary is posted at its effective date (refusal text in `fa/src/external_subledger.rs`). **This is the natural seam for FrankenMining's metal and inventory subledger.**
4. **Multi-book** (§12.2). There is a `Book` per basis (US GAAP, IFRS, tax, statutory, management). "An `Event` posts to every book whose rules bind it." A basis may be an `AdjustmentLayer(of)` book holding only reason-coded differences. `recon.book_to_book` explains differences by rule version.
5. **Docket evidence and custody** (§9). Every document a book ingests gets an `ImportReceipt`; `Connector{name, version}` is the external-system source. Entries cite documents with `EVIDENCED_BY {role}`. Evidence minimums are registry policy (`evidence_policy.toml`).
6. **Drafts and promotion for agents** (§14.4, §15.2). Agent-profile principals are branch-confined and dry-run-required. A human (or a separately scoped approver) promotes drafts to trunk.

**Extension points, and where each lives (the pack-versus-upstream decision):**

| Extension point | Where it lives | Can FrankenMining add it without an FA change? |
|---|---|---|
| Chart of accounts, account types, `closes_to`, `unit_policy`, `dimension_requirements` | Journaled `SchemaControl` (`OpenAccount`, `ImportChart`) | **Yes.** `fa chart open <json>` per account. Chart templates (`us-gaap-smb`, `ifrs-sme`, `us-bank-call-report`) are embedded in the binary, so a mining template would be an upstream addition. |
| Dimension *values* and hierarchies | Journaled (`fa dimension add`) | **Yes** |
| Dimension *kinds* | Compiled: 8 fixed kinds (`cost_center`, `department`, `project`, `product`, `region`, `fund`, `grant`, `segment`) plus `legal_entity` and `party` (`fa-model/src/ids.rs`, `DIMENSION_KINDS`). The plan's `Custom` kind is not implemented. | **No.** Upstream needed for mining-specific axes. |
| Parties, instruments, legal entities, `OWNS {pct}`/`CONTROLS`/`RELATED_TO` | Journaled | **Yes.** `InstrumentKind` is a closed enum (`BankAccount, CreditFacility, Card, Security, Loan, Lease, Contract, CustodyWallet`); the plan's `FixedAsset` and `Derivative` are absent. |
| Canon rule versions | Journaled `RuleVersion` plus `PolicyControl` activation (dual control) | **Yes**, within the current Canon expressiveness (§2.6) |
| Statement definitions (incl. `Custom`) | Journaled, content-addressed (`custom_statement`) | **Yes** |
| REA event *kinds* | Compiled closed enum: `Sale, Purchase, Receipt, Payment, Shipment, Return, Accrual, Depreciation` (`fa-model/src/rea.rs:696`). The plan's `Custom(id)` is not implemented. | **No** |
| Units, scales, balance policy | Compiled `registries/units.toml` (13 units; no metal or mass units). `fa unit add` is planned (§15.3) but only `fa unit list` exists. | **No** |
| Conversion, allocation, lot, FX, rounding, tracing laws | Compiled registries | **No** (append-only upstream rows) |
| Reason codes, relationship bases, document kinds, account roles, application-difference kinds | Compiled registries | **No** |
| Lantern procedures | Compiled `procedures.toml` plus code | **No.** FrankenMining runs its own procedures over FA certified reads. |
| SoD policy, admissibility matrix | Compiled registries | **No.** Ceilings and caveats are per-capability and journaled. |

---

## 4. Codebase state (2026-10-28)

### 4.1 Git and work graph

```text
$ git -C /Users/jemanuel/projects/franken_accounting log -5 --format='%h %ad %s' --date=short
53c6e953 2026-10-28 merge origin/main: keep the mixed equity-view rejection with the remote case and cash-flow work
1fa33bdf 2026-10-28 fix(policy): revalidate retained read authority before historical reads
49194ff3 2026-10-28 fix(fa-cli): reject mixed equity roll-forward view selectors
fe63bbf5 2026-10-28 fix(fad): fail closed on unreadable journaled session authority
5d949847 2026-10-28 feat(reports): independently verify indirect cash flow across all cash bases
```

- 2,279 commits in total.
- The beads graph (`.beads/issues.jsonl`) has 461 issues: 155 open, 61 in progress, 245 closed.
- Pins: toolchain `nightly-2026-08-31`; `asupersync` git rev `78b64636…` (=0.5.0); `frankengraphdb` rev `b323bd17…`; `fastmcp-rust =0.12.0` (confined to `fa-mcp`).

### 4.2 Workspace members, size and tests

There are 31 crates plus 2 tools. LOC counts all `.rs` lines including tests; test counts are `#[test]` attributes.

| Crate | LOC | `#[test]` | | Crate | LOC | `#[test]` |
|---|---:|---:|---|---|---:|---:|
| `fa-cli` | 162,010 | 715 | | `fa-mcp` | 18,684 | 108 |
| `fa` (facade) | 142,685 | 922 | | `fa-registry` | 17,348 | 82 |
| `fa-journal` | 125,808 | 656 | | `fa-formats` | 16,854 | 51 |
| `fa-verify` | 72,428 | 659 | | `fa-statements` | 15,091 | 56 |
| `fa-registers` | 44,473 | 222 | | `fa-reference` | 12,749 | 30 |
| `fa-lantern` | 35,236 | 180 | | `fad` | 9,478 | 60 |
| `fa-oracles` | 33,937 | 404 | | `fa-model` | 9,043 | 50 |
| `fa-notary` | 21,933 | 181 | | `fa-policy` | 8,939 | 52 |
| `fa-money` | 8,167 | 98 | | `fa-canon` | 3,846 | 17 |
| `fa-docket` | 3,655 | 10 | | `fa-bench` | 3,605 | 28 |
| `fa-time` | 3,475 | 43 | | `fa-verbs` | 3,340 | 20 |
| `fa-query` | 3,205 | 15 | | `fa-demo` | 2,930 | 3 |
| `fa-gen` | 2,587 | 5 | | `fa-conformance` | 2,513 | 15 |
| `fa-fuzz` | 1,960 | 9 | | `fa-palimpsest` | 1,513 | 6 |
| `fa-intents` | 1,138 | 4 | | `fa-escrow` | 240 | 3 |
| `fa-steward` | 107 | 1 | | tools (2) | 4,013 | 28 |

**Total: about 793K lines of Rust and about 4,733 `#[test]` functions.** The verb registry (`verbs.toml`, 635 KB) is large, and the CLI surface is broad. **Canon (3.8K LOC) and Concordat (about 4.2K LOC) are small relative to the kernel and surfaces.** Those are the two subsystems FrankenMining leans on hardest.

### 4.3 What §20.1 says is real versus aspirational

FA's own reality check (plan §20.1, the 2026-10-23 audit plus the 2026-10-26 "StormyJay" refresh) is explicit.

**Demonstrated** (synthetic inputs, real binary):

- all 21 `### Try it today` commands exit 0, and `fa replay` reproduces the certified TB;
- the Genesis negatives hold: unbalanced is exit 3 `FA-REF-001`, Original into Closed is exit 7 `FA-REF-010`, a float amount is exit 2, no evidence is exit 3 `FA-REF-080`;
- `fa-verify` accepts prefix, account and TB bundles, and a one-byte tamper exits 8;
- self-approval is refused (`FA-REF-008`);
- `fa demo init --scenario worldcom` plus the JET, Benford and related-party procedures run.

The goal rows rated DEMONSTRATED are V01 (immutable N-leg posting), V02 (registers and projections), V05 (sequence-pinned TB and replay), V14 (an original-cited document bundle subset), V17 (book-key signed checkpoints) and V24 (teaching refusals).

**PARTIAL**, with source present and acceptance incomplete:

- V04 period and calendar law;
- V06 restatement (disclosure binds no diff);
- V08 holds;
- V09 accrual and recurrence;
- V10 batches;
- V11 migration;
- V12 exact arithmetic, FX and units ("unit evolution ... remain");
- V13 dimensions and subledgers (FA-INV-12 clauses and `FA-INV-01.dimensions` **dormant**);
- V15 reconciliation and aging;
- V18 privacy;
- V19 authority and SoD ("SoD binds asserted names; approvals unsigned"; "Dual control on the owner CLI is satisfied by typing a second `--as` name");
- V20 access audit;
- **V21 Canon** ("full intent rule families and caller caveats intersected with RuleAuthority remain; FA-INV-10.post-evaluation is dormant");
- V22 drafts;
- V23 surfaces;
- V25 query;
- V26 statements;
- **V27 multi-book and consolidation** ("real CLI/native certified single-book consolidation from retained shared-functional-unit entities, eliminations and NCI; this read posts nothing. Cross-book basis layers, commit-time intercompany pairing, branch-posted eliminations, foreign-functional-currency translation and CTA remain");
- V28–V31 forensics, audit and standards;
- V32–V33 simulation and formal methods;
- **V37 quantity lots and scheduled asset costs** ("lot certification explicitly refuses. Depreciation/amortization rule families remain owned by Canon").

**NOT_STARTED / UNPROVEN / REGRESSED:**

- **V07 CloseRun DAG: NOT_STARTED**; YearEndClose is real;
- V16 independent portable report verification: UNPROVEN;
- **V34 performance: UNPROVEN** (all 20 SLO rows inactive);
- V35 exact-tree gates: REGRESSED;
- V36 release: NOT_STARTED;
- **V38 "Operable at real-book scale through the CLI and fad": NOT_STARTED**;
- V39 public-record case studies: zero real cases.

**Verdict and scale findings** (2026-10-26):

- "No genuine full verdict exists."
- The last complete workspace run was `24c64e79` (3,407 passed / 15 failed); main has since taken 404 commits.
- Single posts slow from 118 ms to 439 ms across 300 posts in a debug build, about 1.1 ms more per entry already in the book.
- `fa verify book` "exits 8 on a healthy book whenever a period holds entries but was never materialized."

**Bridge freeze that affects FrankenMining's upstream asks:** "truth (`fa-0hs4.1`–`.4`), then operability (`fa-0hs4.6`, `.5`, `.11`), then authority (fa-z46b, fa-kae, fa-yok), then breadth. **Until the first three hold, no new verbs, MCP tools, Lantern procedures or case-harness machinery land.**"

---

## 5. Mining-accounting fit analysis

Legend:

- **N**: native. The plan names the mechanism as first-class.
- **G**: general mechanism. Expressible with FA primitives; FrankenMining supplies the computation, policy and evidence.
- **D**: domain pack. FA has no concept for it; FrankenMining must build it, and FA only records the results.

"Today" is the code status at `53c6e953`.

### 5.1 Multi-entity consolidation, JVs and joint operations

*Example: Nevada Gold Mines, Barrick 61.5% / Newmont 38.5%. Barrick consolidates with a 38.5% NCI. The 38.5% holder can use proportionate consolidation (permitted under US GAAP for undivided interests in extractive operations) or, for an incorporated JV, the equity method. Verify the actual elections against current filings.*

| Need | Verdict | FA mechanism | Today |
|---|---|---|---|
| Full consolidation with NCI | **N** | §12.3 `ConsolidationRun` on a branch of the consolidation book; valid-time `OWNS {pct}`; `Elimination`-class entries by rule; NCI and CTA; FA-INV-13. Multi-entity book: `legal_entity` is a balancing dimension, the balance law holds per `(unit, legal_entity)`, and due-to/due-from are paired by `INTERCOMPANY_WITH` (§4.1, §5.2). | **PARTIAL.** `fa consolidate` is a **read**: certified, nothing posted. It works for a single book with a shared functional unit, using OWNS edges plus account roles (`contributed_capital`, `intercompany`, `investment_in_subsidiary`, `noncontrolling_interest`, ...). Ownership is in **basis points** (`OwnershipSpec.ownership_bps: u16`; 6150/10000 is representable). The code also refuses three cases. **One group parent per subsidiary**: a member owned by two group entities is refused. **Tiered NCI** is refused: "consolidate groups whose partly owned subsidiaries own no other member; tiered NCI is not available yet (fa-76hy)". **Cross-book translation and CTA, and posted eliminations**, remain (V27). |
| Effective ownership and attributable share | **N** | `fa party ownership` returns the exact effective stake: the sum over simple OWNS paths of the product of stakes, as of a day and at a known-at (`effective_ownership` event in `verbs.toml`). | Implemented |
| Equity method (ASC 323 / IAS 28) | **D** | No law or rule family. The `relationship_bases.toml` rows `significant_influence_associate` and `joint_venture` exist only as `RELATED_TO` bases for related-party closure. | Absent |
| Proportionate consolidation / IFRS 11 joint operations (line-by-line share of assets, liabilities, revenue and expenses) | **D** | Can be *posted* with general mechanisms. FrankenMining computes the share from the JV's certified statements and posts `Original`/`Adjusting` entries citing the JV's certificate or bundle as evidence. `Elimination` is "currently unsupported by the posting kernel" (`intents.toml` comment). | Absent |
| JV operator accounting (cash calls, joint-interest billing, operator overhead, partner audits) | **G** | Native AR/AP subledgers keyed by `party` (§5.6); `RecordInvoice`/`RecordBill`; `Allocate` by participating interest (`ProRataResidualToNamed`, `LargestRemainder`); `pair_key` intercompany when both partners are in the group. Partner-verifiable JV reporting via `fa bundle produce` plus `fa-verify` (§13.2–13.3) is a genuine differentiator for JV audit rights. | AR/AP native subledgers implemented. Allocation is implemented with literal source amounts (`AllOfPeriod` unsupported). |

**Gap to close:** FrankenMining needs tiered NCI (mining groups routinely have partly owned holding companies with mine-owning subsidiaries, and host-government free-carried interests). It also needs multi-book consolidation with translation, and equity or proportionate methods. The first two are FA backlog (fa-76hy and V27). The methods should be either an FA Concordat extension or explicitly FrankenMining-owned.

### 5.2 Inventory: stockpiles, WIP, finished goods, NRV, multi-unit quantities

| Need | Verdict | FA mechanism | Today |
|---|---|---|---|
| Carrying inventory cost by account and dimension | **G** | Asset accounts with `unit_policy: AnyUnit` (§4.3). An "inventory" account role exists. | Implemented |
| Contained or recoverable metal quantity as a ledger quantity | **G** | Units of `kind: Commodity` with their own scale (§4.4, §6.5). **Commodity units balance per unit** (FA-INV-01 `per-unit`; `fa-money` `BalancePolicy::Balanced`). Every quantity debit therefore needs a quantity credit, which gives FrankenMining a **metal balance enforced by the kernel**: ounces in the ground, then stockpile, then pad, then circuit, then doré, then refinery, then sold. | **No metal or mass units registered.** `units.toml` has 13 rows (USD, EUR, GBP, JPY, CHF, CAD, AUD, KWD, BTC, ETH, SHARE, HOUR, EACH). `conversion_laws.toml` already has `TroyOuncesToGrams` (`XAU_OZ`→`GRAM`, exact factor 19439673/625000), `AvoirdupoisOuncesToGrams` and `GramsToKilograms`, yet `XAU_OZ`, `GRAM` and `KG` are not in `units.toml`. `fa unit add` is unimplemented. |
| Cost-flow formula | **G** | `Lot` = derived open item `(quantity leg, cost leg)` on an `AnyUnit` account; `RELIEVES {quantity, cost}`; `lot_laws.toml`: `FIFO`, `LIFO` (refused under IFRS per IAS 2.25), `SpecificId`, `WeightedAverage` (pooled) (§6.5). | **PARTIAL** (V37). `fa gl lots` plans relief; "lot certification explicitly refuses". |
| Multiple simultaneous quantity attributes per lot: wet tonnes, dry tonnes, moisture, grade, contained metal (per metal), recoverable metal, payable metal | **D** | A Lot has **one** quantity unit and one cost. Grade and moisture are ratios, not additive quantities, and FA has no ratio unit. Wet-to-dry tonnes is a *measured* per-lot factor, so it must **not** be a `ConversionLaw` (those are fixed exact factors). | Absent |
| Heap-leach WIP (recoverable ounces placed, recovery curves, average cost per recoverable ounce, pad inventory true-ups); metal in circuit; long-term low-grade stockpiles (non-current classification) | **D** | No concept. FA only records the summary entries. | Absent |
| Survey reconciliations, stockpile true-ups, metal-accounting reconciliation (mine to mill to market) | **D**, posted as **G** | FrankenMining computes the true-up; FA records an `Adjusting` entry with reason `EstimateChange` (or `Reclassification`), with survey and assay workpapers as `EVIDENCED_BY {Support}`. | Implemented posting path |
| NRV / lower-of-cost write-downs, and reversals (permitted under IAS 2.33; under ASC 330 the write-down establishes a new cost basis) | **D** | No LCM/NRV law. FrankenMining computes NRV from long-term price assumptions and costs to complete and sell, and posts `Adjusting` entries. The reversal policy differs per book (IFRS book versus US GAAP book). | Absent |
| Materials and supplies, obsolescence | **G** | Ordinary inventory accounts with an `Adjusting` allowance; `aging.inventory` Lantern procedure (§11.7). | Procedure registered |

**Design consequence:** FrankenMining owns a **metal and inventory subledger** that holds all physical attributes and does the costing. It posts **summary entries** through FA's externally-fed-subledger form, where the detail is a canonical `Workpaper` digest reviewed by an independent principal (§5.6, FA-INV-12). Each summary carries contained and recoverable metal as balanced commodity-unit legs, so FA's balance law double-checks the metal balance. This requires upstream unit rows (§7.4).

### 5.3 Production cost accounting and allocation; by-products and co-products

| Need | Verdict | FA mechanism | Today |
|---|---|---|---|
| Cost pools by stage (mine, mill or leach, refinery), site, metal | **G** | Dimensions (`cost_center`, `region`, `product`, `project`, `segment`; §4.1) with hierarchies and valid time. `RegisterDefinition`s declare materialized cuts (§5.3). | Dimension values and hierarchies implemented. **Required-dimension and dimension-filtered TB certification are PARTIAL** (V13). `RegisterDefinition`s are not surfaced. |
| Allocation of costs by drivers | **N/G** | `Allocate` intent with a registered `AllocationLawId` and a `driver_document` (§6.3, App. B). The `Allocation` class records law and basis digest. | Implemented: `LargestRemainder`, `ProRataResidualToNamed`, `Waterfall`, `EqualSplitResidualFirst`. **`Tiered` and `ByUnitsThenValue` are absent** from `allocation_laws.toml`. `AllOfPeriod` is unsupported. |
| Process-costing flow (absorption into stockpile, WIP and FG; cost per tonne or ounce) | **D** | No process-costing engine. FrankenMining computes; FA records the transfers (`Reclass` or `PostEntry`). | Absent |
| By-product credit (silver in gold mines, gold in copper mines) | **D** (policy) | Under IFRS 15 / ASC 606, by-product sales are revenue. "By-product credit" is mainly a cost-metric and presentation convention (§5.13). FA records the revenue. | n/a |
| Co-product allocation (relative sales value, physical units) | **G** for posting, **D** for the weights | `Allocate` with `Rate` weights. Computing the weights needs price × quantity, which **Canon cannot do** (variable × variable is refused), so FrankenMining computes the weights and supplies them as a driver document. | n/a |

### 5.4 Deferred stripping (IFRIC 20; ASC 930 practice)

- **Verdict: D** for the computation, **G** for the posting.
- Under IFRIC 20, a stripping activity asset is recognized per identifiable ore-body component when future access is improved. Costs are allocated between inventory and the asset using a relevant production measure (for example, actual versus expected waste-to-ore ratio), and the asset is depreciated by units of production over the component's life. US GAAP (ASC 930-330-25-1) treats production-phase stripping as a variable production cost of inventory.
- **FA mechanism:** separate IFRS and US GAAP books, or an `AdjustmentLayer`, with different Canon rule activations bound to the same event (§12.2, §10.3 "the same event evaluated under two active books ... may bind different rule versions"). In the IFRS book, `Reclassification`/`Adjusting` moves cost from production cost to `Stripping activity asset` (one `project` dimension value per component). `recon.book_to_book` explains the basis difference.
- **Today:**
  - `AdjustmentLayer` and the book-to-book bridge exist only as **library types** (`fa-statements/src/concordat.rs` `BookKind::AdjustmentLayer`, `BookToBookBridge`). There is no CLI or facade surface (V27: "cross-book basis layers ... remain").
  - `BookToBookReasonCode` is a **closed Rust enum** of US tax Schedule M-1/M-3 codes plus `StatutoryReclassification` and `ManagementCostAllocation`. IFRS-versus-US GAAP mining differences cannot be reason-coded without an upstream change.

### 5.5 DD&A on units of production, with prospective reserve revisions

- **Verdict: G** for the posting family, **D** for the rate engine.
- **FA mechanism:**
  - The `Depreciate`/`Amortize` intents (`instrument|asset refs, period, schedule_rule`; App. B) and the "schedules" rule family post `Adjusting` entries.
  - A prospective change in estimate is ordinary current-period posting under a new rule input or activation (`PolicyActivation { effective: [from, to) }`, §10.3) with reason `EstimateChange` (§8.2). It is **not** a `Restatement`, which is reserved for errors under ASC 250 / IAS 8 (§8.3).
  - The reserve statement or QP report is the cited `Document`.
- **Today:**
  - `Depreciate`/`Amortize` are not intents in `fa-intents`, and the "depreciation/amortization rule families remain owned by Canon" (V37).
  - Canon can produce `Adjusting` entries, but cannot compute the UoP rate, which needs (net book value ÷ remaining reserves) × period production, that is, division and multiplication of variables.
  - `InstrumentKind` has no `FixedAsset` (the plan has it).
- **FrankenMining therefore owns:** the mineral-property and asset register, component accounting, the UoP rate engine (reserves and, where policy allows, a portion of resources), and straight-line or other bases for non-UoP assets. It passes the computed depletion amount to FA as an `Amount` input or an event resource, with the depletion workpaper as evidence.

### 5.6 Exploration and evaluation (IFRS 6) versus expense (US GAAP)

- **Verdict: G** for multi-book posting, **D** for the policy.
- **FA mechanism:**
  - Each basis is its own book with its own rule activations (§12.2), or a layer.
  - The IFRS book capitalizes E&E (`project` = exploration project) while the US GAAP book expenses, except acquisition costs and post-reserve development, per company policy.
  - IFRS 6.20 impairment triggers and the reclassification to development on technical feasibility are FrankenMining judgments, posted as `Reclassification`.
- **Today:** this works only as two **full** books maintained in parallel, because adjustment layers and the bridge are not surfaced (§5.4).

### 5.7 Asset retirement obligations and closure/rehabilitation provisions (IAS 37 + IFRIC 1; ASC 410-20)

- **Verdict: D** for measurement, **G** for posting.
- FA has **no** present-value, discounting or accretion law. Abacus has exact rational scale and rounding laws (§6.1–6.2), but nothing like `PV(cashflows, rate)`.
- **FrankenMining owns:**
  - closure cost estimates, inflation and discount rates;
  - IFRS whole-provision remeasurement at the current rate, versus US GAAP layering at the credit-adjusted risk-free rate (upward revisions as new layers at current rates, downward revisions at historical weighted rates);
  - unwinding or accretion schedules;
  - settlement;
  - the split of accretion and amortization between operating and non-operating sites, needed for AISC (§5.13).
- **FA mechanism:**
  - Initial recognition is `Original` (Dr asset retirement cost / Cr provision).
  - Monthly accretion is `Accrue` or `Adjusting` with the computed amount. `RecurringSchedule` (`fa/src/recurring.rs`) mints only **fixed** entries, so it does not fit an accretion amount that changes each period.
  - Revisions are `Adjusting` with reason `EstimateChange` and the closure study as evidence.
- **Note:** ASC 410 layers behave like lots (each with its own rate). FrankenMining can keep them in its own register; FA's `Lot` is quantity/cost-shaped and not rate-shaped.

### 5.8 Impairment of CGUs and asset groups (IAS 36; ASC 360-10)

- **Verdict: D** for testing, **G** for recording and analysis.
- The plan places this explicitly outside FA: "whether a reserve is adequate" and similar judgments (§0.2).
- **FrankenMining owns:** the CGU/asset-group registry, LOM plans, price and FX decks, FVLCD/VIU (IAS 36) and the ASC 360 recoverability-then-fair-value two-step, the allocation of impairment to assets, and reversal policy (permitted under IAS 36.114 except goodwill; prohibited under US GAAP).
- **FA provides:**
  - `Adjusting` entries with reason `EstimateChange`, citing the LOM plan, price deck and model certificate as documents;
  - **scenario branches** for sensitivity (`ForkScenario` / `fa scenario fork|run|diff` give a certified diff over a scratch replay, §15.2, §15.6 "but-for");
  - capability ceilings and human approvals for large write-downs (§14.2, §14.4);
  - a CGU mapped to a dimension value (`segment` or `region` hierarchy).

### 5.9 Provisionally priced concentrate sales

Context: under IFRS 9 the receivable is FVTPL until final pricing; under US GAAP the embedded derivative is bifurcated. Contracts include QP months, forward curves, payable percentages, TC/RCs, penalties, provisional and final assays, umpire assays, and moisture.

- **Verdict: D**, with some **G** pieces.
- **What fits:**
  - `RecordInvoice` (provisional invoice) or `RecordEvent(Shipment|Sale)` with a Canon rule;
  - native AR subledger by smelter or offtaker `party`;
  - `ApplyReceipt` with typed `differences` at final settlement;
  - `Adjusting` entries for month-end mark-to-market;
  - a `Rate` document kind for price fixings and forward curves (§6.4 "A rate is evidence, not configuration").
- **What does not fit today:**
  - **FX rate documents must name currencies.** `fa/src/fx.rs:46–56` (`currency_units`) refuses a rate whose units are not currencies, so FA's revaluation machinery cannot remeasure a metal-denominated receivable at a commodity price.
  - Revaluation posts to an unrealized-FX account. Mining presentation puts provisional-pricing adjustments in revenue (or "other revenue") lines.
  - Application difference kinds are a closed list (`Discount, ShortPay, Writeoff, BankCharge, RealizedFx`). There is no `ProvisionalPricing`, `AssayAdjustment`, `TreatmentCharge` or `Penalty` kind. Final-settlement deltas need explicit `PostEntry` legs, or upstream registry rows.
  - Two-phase `Pending`/`Reserve` (§5.4) is **not** the right tool. It models holds and reservations, not fair-value remeasurement.
- **FrankenMining owns:** the concentrate sales engine (contract terms, QP logic, forward-curve MTM, payable metal, TC/RC, penalties, assay exchange and umpire, title-transfer cutoff) and posts the results.

### 5.10 Streaming and royalty agreements

- **Verdict: D** for the contract model, **G** for posting.
- A stream deposit is a contract liability under IFRS 15 / ASC 606, settled by metal delivery, with a significant financing component (interest accretion). Revenue is recognized per ounce delivered at deposit ÷ expected deliveries. When reserves or LOM change, there is a **cumulative catch-up** adjustment.
- **FA:**
  - `Instrument { kind: Contract }` with the stream agreement as a `Contract` document;
  - delivery entries are `Original`; accretion is `Accrue`/`Adjusting`; catch-ups are `Adjusting` with reason `EstimateChange`;
  - `Defer` with `auto_reverse_on` does not model per-ounce drawdown.
- NSR, NPI and gross royalties payable are computed by FrankenMining and posted with `Accrue`.

### 5.11 Hedging and hedge accounting (metal, FX, diesel)

- **Verdict: D** for hedge accounting, **G** for posting.
- **FA has:**
  - OCI-capable account types: `Revenue(stmt: OCI)`, `Expense(stmt: OCI)`, and the `Equity(AOCI)` subtype (§4.1);
  - `YearEndClose` closes OCI to AOCI (§7.5);
  - reclassification for recycling (`Reclass`, §8.1);
  - hedge documentation and fair-value statements as documents.
- **FA lacks:** hedge designation, effectiveness assessment, fair-value measurement of derivatives, and recycling rules.
- The plan's `Instrument.kind = Derivative` is not in the implemented `InstrumentKind` enum.

### 5.12 Mining taxes and royalties

- **Verdict: D.** FA is explicitly "not ... a tax engine" (§0.3).
- **FA provides:**
  - `Accrue` entries;
  - a **tax book as an `AdjustmentLayer`** with reason-coded differences, the ASC 740 starting point (§12.2), though not yet surfaced;
  - the `TaxAuthority(jurisdiction, rule)` rounding regime (§6.2);
  - the `Tiered(brackets)` allocation law, which is **absent** in the registry, for sliding-scale royalties.
- **FrankenMining owns** the jurisdictional regimes: government royalties (ad valorem and profit-based), mining taxes treated as income taxes under IAS 12 where profit-based, NSR royalty computation, and royalty-holder statements.

### 5.13 Non-GAAP metrics: AISC, AIC, cash costs, C1, attributable versus consolidated

- **Verdict: G** for the data and certification substrate, **D** for the metric definitions.
- **FA provides:**
  - `StatementDefinition { kind: Custom, lines[{mapping: AccountSetExpr | Formula(lines)}] }` as journaled data, so a custom statement over certified registers is possible (§12.1, `custom_statement` verb);
  - **`Statistical` accounts** admitting single-sided legs for quantities such as ounces sold or tonnes milled, excluded from monetary footing (§4.2);
  - dimensions for site, metal and cost category;
  - exact `fa party ownership` stakes for attributable figures;
  - certificates FrankenMining can cite.
- **Missing:**
  - per-ounce and per-pound ratios mean dividing money by quantity, which statement formulas do not do (FA never emits a float, §1 constraint 4);
  - the WGC AISC/AIC component taxonomy (sustaining capex, sustaining exploration, operating-site reclamation accretion and amortization, G&A, by-product credits);
  - C1 (mining + processing + site G&A + TC/RC + freight − by-product credits, per payable pound);
  - only `HOUR` and `EACH` are single-sided units today.
- **FrankenMining owns** a metrics engine that consumes FA's certified TB and statement rows, by certificate, and publishes its own certified non-GAAP reconciliation.

### 5.14 Capital project accounting (CIP; sustaining versus growth)

- **Verdict: G** for posting, **D** for classification and project control.
- **FA:**
  - CIP accounts with a `project` dimension;
  - `Reclass` CIP to PP&E at commissioning;
  - `dimension_requirements` forcing `project` on CIP legs;
  - the Lantern `expense.capitalization` procedure, which detects operating-in-nature capitalizations (the WorldCom pattern, §11.7) and is valuable for AISC integrity.
- **FrankenMining owns:** the sustaining-versus-growth policy (a non-GAAP judgment), capitalized borrowing cost computation (IAS 23 / ASC 835-20), pre-production proceeds treatment (IAS 16 as amended), AFE and budget control, and commitments.
- FA's escrow `balance_limit` as a **budget cap** on a project's expense or CIP account (§5.5) could enforce AFE limits at posting time. The interim `LimitCell` is limited-account-only.
- **Interim encoding:** encode sustaining and growth in the **chart** (separate CIP leaves), because there is no free custom dimension (§7.3).

### 5.15 Reserve and resource disclosure linkage (S-K 1300, NI 43-101); non-financial quantities

- **Verdict: D** for the reserve model, **G** (limited) for linkage.
- **FA has the right primitives for linkage:**
  - **`Statistical` accounts** with single-sided units (§4.2; `account_types.toml`) could carry a reserve or resource roll-forward (opening, less depletion, plus or minus revisions, plus acquisitions, equals closing) through a `RollForward` statement definition with reason codes (§12.1);
  - the technical report summary or NI 43-101 report is a `Document` of kind `Disclosure`;
  - QP sign-off can be an `Attestation` (`Prepared` / `Reviewed{against}` / `Custom`) bound to the report digest (§9.4);
  - the same document can be cited as evidence by DD&A, impairment and stripping entries, giving a provable chain from QP-signed reserves to the depletion charge.
- **Missing:**
  - reserve-specific single-sided units (only `HOUR` and `EACH` exist; they are compiled in);
  - any notion of category (measured, indicated, inferred; proven, probable), grade or price assumptions;
  - this is FA "Event resources" and statistics at most, with no S-K 1300 semantics.
- **FrankenMining owns** the reserves and resources register and its compliance workflow.

### 5.16 Summary matrix

| # | Need | Verdict | Today in FA |
|---|---|---|---|
| 1a | Full consolidation + NCI | N | PARTIAL: single-book read; no tiered NCI, no multi-parent, no cross-book translation or CTA |
| 1b | Equity method; proportionate / IFRS 11 JO | D | Absent (relationship bases only) |
| 1c | JV operator billing | G | AR/AP subledgers, Allocate |
| 2a | Inventory cost carrying, lots, weighted average | G | PARTIAL (lots uncertified) |
| 2b | Metal quantities (oz, lb, t), conversions | G with upstream ask | **No metal units registered** |
| 2c | Multi-attribute quantities, heap leach, recovery, NRV | D | Absent |
| 3 | Production costing, by-products and co-products | G + D | Allocate laws partial; Canon cannot multiply variables |
| 4 | Deferred stripping | D + G | Multi-book layers not surfaced |
| 5 | UoP DD&A with prospective revisions | G + D | Depreciate family absent; no FixedAsset instrument |
| 6 | E&E (IFRS 6 vs US GAAP) | G + D | Needs two full books today |
| 7 | ARO / closure provisions | D + G | No PV or accretion law |
| 8 | Impairment (IAS 36 / ASC 360) | D + G | Scenarios available |
| 9 | Provisional pricing | D + G | Rate docs currency-only; difference kinds closed |
| 10 | Streams and royalties | D + G | Contract instrument available |
| 11 | Hedge accounting | D + G | OCI and AOCI types available; no Derivative kind |
| 12 | Mining taxes and royalties | D | Tax engine is an anti-goal; Tiered law absent |
| 13 | AISC, AIC, C1, attributable | G + D | Custom statements, Statistical accounts, ownership stakes |
| 14 | Capital projects (CIP, sustaining vs growth) | G + D | `project` dimension, `expense.capitalization` |
| 15 | Reserves and resources linkage | D (+ G linkage) | Statistical accounts, Disclosure documents, attestations |

---

## 6. Things FrankenMining can rely on from FA as-is (and should not rebuild)

- **The book of record:** immutable N-leg entries, balance law per unit and per entity, period law at commit, admissibility matrix, idempotent intents, reversal and replacement lineage, restatement cases, bitemporal selectors and `AS ORIGINALLY REPORTED` (V01, V02, V05 demonstrated; others partial).
- **Certified, replayable reports and offline proof bundles** (`fa replay`, `fa bundle produce`, `fa-verify`). These are how FrankenMining should hand accounting facts to auditors, regulators, JV partners, streamers and lenders.
- **Docket provenance:** ImportReceipts, provenance grades, attestations, balance assertions, bank-statement parsing (`camt.053/052`, BAI2, OFX, MT940, CSV).
- **Steward capabilities** for agent confinement, ceilings and access logs. The SoD binding to asserted names is a known gap (V19), so it is not yet SOX-grade evidence.
- **Lantern procedures** relevant to mining controls: JET suite, `expense.capitalization`, `cutoff.period_boundary` (concentrate shipments at period end), `match.three_way` (procurement), `numbers.threshold_splitting` (split purchases), `party.master_changes` (vendor bank changes), `graph.related_party_exposure` (host-government and related-party flows), `recon.subledger_control`, `sample.mus`.
- **Scenarios** for LOM and price-deck what-ifs, and **drafts** for agent-prepared month-end work.

---

## 7. Integration design proposal

### 7.1 Principles (each grounded in the plan)

1. **FA is the only book of record; FrankenMining never computes authoritative balances.** Balances are folds (A1, FA-INV-04). FrankenMining reads them through certified reads and cites the certificates.
2. **FrankenMining owns operational truth and estimates; FA owns the mechanical record** (§0.2). Every FrankenMining estimate that moves money (NRV, depletion rate, ARO PV, impairment, MTM, stream drawdown) is produced as a **certified FrankenMining computation document** and enters FA as evidence on the entry it supports (§9.2 "allocation driver tables" and "basis documents" are already in the provenance closure).
3. **No second posting universe.** Detailed physical and inventory ledgers live in FrankenMining and enter FA only as reviewed summary batches (§5.6 external form; FA-INV-12).
4. **One intent per accounting fact; idempotency keys derived from FrankenMining's own stable IDs** (for example `fm:<site>:<event-type>:<event-id>:<book>`) so retries are safe (§5.9, FA-INV-9).
5. **Agents prepare and humans authorize.** FrankenMining agents run under FA's `Agent` profile on drafts; controllers promote (§14.4, §15.6).

### 7.2 Book topology

Constraints: one functional currency per book; an entity with another functional currency needs its own book; reporting-currency amounts exist only as `Translation` outputs in a consolidation book (§0.4 decision 1, §4.1).

- **Primary-basis group book(s):** one FA book per *functional currency group* per primary basis. For example, all USD-functional mine-holding entities in one multi-entity book (`legal_entity` balancing dimension). Barrick-class issuers report IFRS; Newmont-class issuers report US GAAP. FrankenMining should support either as primary.
- **Secondary-basis books:** the other GAAP, local statutory and tax per jurisdiction. The target is `AdjustmentLayer(of: primary)` books holding only reason-coded differences (§12.2). **Interim:** layers are not surfaced (V27), so either run a second full book whose Canon activations differ, or defer.
- **Management book:** an `AdjustmentLayer` (or the primary book plus dimensions) for AISC and cost-category tagging.
- **Consolidation:** single-book `fa consolidate` today, for groups without tiered NCI. The cross-book `ConsolidationRun` with translation and CTA is pending (fa-76hy).
- **JV books:** when FrankenMining is the *operator*, it runs the JV entity's own book. Partners receive certified statements and `.fab` bundles verifiable with `fa-verify`. When FrankenMining is a *non-operating partner*, it imports the operator's certified statements or bundle as evidence and posts its proportionate share or equity pick-up (domain pack).

### 7.3 Chart and dimensions

- **Chart:** FrankenMining ships a **mining chart spec** (IFRS and US GAAP variants) applied through `OpenAccount` schema-control intents (`fa chart open <json>`), with no upstream dependency. An upstream `mining-ifrs` / `mining-usgaap` template is optional. Key leaf families:
  - ore stockpiles (current and non-current);
  - heap leach WIP, metal in circuit, doré/bullion, concentrate, cathode;
  - materials and supplies;
  - mineral properties (acquired, development, producing);
  - stripping activity assets per component; E&E (IFRS book);
  - CIP split by **sustaining/growth** leaves;
  - ARO asset and provision (operating and non-operating sites);
  - stream deferred revenue; provisional-pricing receivable and adjustments;
  - royalties and mining taxes payable; hedge assets and liabilities; CFH reserve (AOCI);
  - NCI equity and NCI income;
  - `Statistical` production and reserve accounts.
- **Dimensions:** FA offers 8 fixed kinds plus `legal_entity` and `party`. Proposed mapping:

  | FA dimension | FrankenMining meaning |
  |---|---|
  | `legal_entity` (balancing) | legal entity |
  | `party` | smelters and offtakers, refiners, JV partners, streamers, royalty holders, governments, suppliers |
  | `region` | operation hierarchy: country > complex > mine > pit/UG |
  | `segment` | IFRS 8 / ASC 280 reportable segment; CGU or asset group (as a hierarchy node) |
  | `cost_center` | process stage / department: mining, processing (mill/leach/flotation), refining, site G&A, corporate |
  | `project` | capital project / AFE, stripping component, exploration project |
  | `product` | metal / product: Au, Ag, Cu, Sn; doré, concentrate, cathode |
  | `department` | spare; optionally the WGC cost category |

  Avoid repurposing `fund` and `grant`: `fund` drives fund-balance closing behavior (§7.5). **Upstream ask:** `Custom` dimension kinds (plan §4.1) for capex class, AISC category and stockpile ID.

### 7.4 Units and conversion laws (upstream rows)

Upstream ask: add append-only rows to `units.toml`, or implement `fa unit add` as a journaled unit vertex as §4.1 and §15.3 intend.

- **Balanced commodity units** for metal and inventory flows. Suggested codes: `XAU_OZ`, `XAG_OZ`, `CU_LB` and/or `CU_T`, `SN_T`, `ORE_T` (dry metric tonnes) and `WMT`. Their scales are chosen once and are immutable (§4.4). These give a kernel-enforced metal balance.
- **Single-sided statistical units** for KPIs and reserves, for example `XAU_OZ_STAT` and `ORE_T_STAT`, with `balance_policy = single_sided`.
- **Conversion laws:**
  - `XAU_OZ↔GRAM` already exists (19439673/625000);
  - add `LB↔KG` (exact 45359237/100000000), `T↔KG`, and `T↔LB` (exact rational);
  - **never** a wet-to-dry conversion law, because moisture is measured per lot. FrankenMining computes dry tonnes and cites the moisture assay.

### 7.5 Mine event to FA mechanism map

| FrankenMining event | FA path | Class | Evidence (kind / grade) |
|---|---|---|---|
| Ore mined to ROM or stockpile (daily or monthly summary) | External-subledger summary `PostEntry`/`PostBatch`: Dr stockpile (USD cost; `ORE_T`, `XAU_OZ` contained) / Cr production cost absorbed (USD) / Cr in-situ metal clearing (commodity units) | Original | dispatch and survey `Workpaper` (Connector); grade-control assays |
| Ore processed (stockpile to mill or pad); pour (WIP to doré) | External-subledger summary or `Reclass`; lot relief at `WeightedAverage` when FA lots are used | Original / Reclassification | metallurgical balance `Workpaper`; pour slips |
| Stockpile or pad survey true-up; metal-accounting adjustment | `PostEntry` | Adjusting, reason `EstimateChange` | survey report, reconciliation `Workpaper`; reviewer `Attest` |
| NRV write-down or reversal | `PostEntry` per book | Adjusting | NRV computation `Workpaper`; long-term price `Rate` doc |
| Concentrate or doré shipment and provisional invoice | `RecordEvent(Shipment/Sale)` plus activated Canon rule, or `RecordInvoice`; party = smelter | Original | bill of lading, provisional invoice (`Invoice`), provisional assay |
| Month-end provisional-pricing MTM | `PostEntry` (until a commodity revaluation law exists) | Adjusting | forward-curve price documents (needs non-currency `Rate` docs), MTM `Workpaper` |
| Final settlement | `ApplyReceipt` plus explicit adjustment legs (until new difference kinds exist) | Original | final invoice, final or umpire assay, remittance, bank line |
| DD&A (UoP) | Canon `produces Adjusting` rule with amount input, or `PostEntry`; later `Depreciate` | Adjusting | depletion `Workpaper` citing the QP reserve `Disclosure` doc |
| Deferred stripping capitalization (IFRS book) | `Reclass` / `PostEntry` | Reclassification / Adjusting | stripping-ratio `Workpaper` |
| ARO accretion; revisions | `Accrue` / `PostEntry` | Adjusting (`EstimateChange` for revisions) | closure cost study, discount-rate `Rate` doc |
| Impairment / reversal | `PostEntry` per book; scenario rehearsal first | Adjusting | LOM plan, price deck, model output |
| Stream delivery; stream accretion; catch-up | `PostEntry` / `Accrue` | Original / Adjusting | stream agreement `Contract`, delivery notice |
| Royalty and mining-tax accruals | `Accrue` | Adjusting | royalty or tax computation `Workpaper` |
| Hedge MTM; recycling | `PostEntry` / `Reclass` | Adjusting / Reclassification | counterparty confirmations, valuation `Workpaper` |
| JV cash call; joint-interest billing | `RecordInvoice` / `RecordBill` to partner `party` | Original | JV agreement `Contract`, billing statement |
| Bank activity | `fa bank import` / `MatchBank` / `ConfirmMatch` / `AssertBalance` | — / attestation | `BankStatement` |
| Reserve statement update | `ImportDocument(kind Disclosure)` plus QP `Attest`; FrankenMining reprices DD&A prospectively from the effective date | — | Technical report summary / NI 43-101 report |

**REA event kinds:** the implemented enum has 8 kinds. Until `Custom(id)` lands upstream, FrankenMining maps mining events onto `Shipment`, `Sale`, `Accrual` and `Depreciation` and distinguishes them by resource labels. Other mining events use `PostEntry`/`PostBatch` directly. This is legitimate under §10.2 ("everything below could be expressed as it") but loses `DERIVED_FROM` event lineage.

### 7.6 Canon rule library: keep it thin

Current Canon has no variable × variable, no division by inputs, no tables, and only Original or Adjusting outputs. So FrankenMining's Canon rules should be **account determination only**. FrankenMining computes every amount and passes it as event resources or `Amount` inputs, and the rule maps them to accounts, dimensions and dates.

This keeps policy auditable: a rule version, activated with dual control, golden-tested and backtested (§10.5–§10.6). It also keeps valuation logic in FrankenMining's tested engines.

**Upstream asks** that would let more policy move into Canon, all already in plan §10.3 and §6.3–§6.5:

- multiplication of two quantities with a named rounding law ("a quantity times a price yields the price's unit");
- division;
- table inputs (`TaxTable@oid`, `AccountMap@oid`);
- folds over lots;
- `Allocation` and `Revaluation` output classes.

### 7.7 Subledgers

- **Metal and inventory subledger (FrankenMining-owned, externally fed).** FrankenMining posts summary entries naming the detail batch digest. The detail is retained as a canonical external-detail `Workpaper`, reviewed by an independent principal; batches whose detail does not fold to the summary are refused (§5.6, FA-INV-12; `fa::external_subledger`).
  - **Batch granularity:** per site per day, or per site per month at first.
  - **Note:** both FA-INV-12 clauses are still **dormant** (V13), and external summary-feed detail-digest validation remains open (`fa-03s`). FrankenMining should run its own fold-equals-summary check in addition.
- **Receivables and payables (native FA).** `fa subledger add` with a party-qualified control leaf for smelters and offtakers, JV partners and suppliers (implemented; §5.6).
- **Fixed assets / mineral properties.** FrankenMining-owned register posting summarized DD&A until FA has `FixedAsset` instruments and a schedules family.

### 7.8 Docket evidence from mine events

- FrankenMining ingests every source artifact through `ImportDocument` with `ImportReceipt.source = Connector{name: "franken_mining/<module>", version}`, which gives provenance grade `Connector(transcript)` (§9.3). Artifacts include assay certificates, survey reports, weighbridge tickets, bills of lading, provisional and final invoices, umpire results, LBMA/LME fixings, QP-signed reserve statements, LOM plans and closure studies.
- Lab certificates that carry signatures should be imported so they grade `ThirdPartySigned`.
- **Self-sourcing:** FrankenMining's own computed workpapers grade `Generated` (or `AgentUpload`). Entries citing **only** such evidence are self-sourced: an Agent may commit them only on a draft, and promotion requires a human `Reviewed{against}` attestation (§9.3). FrankenMining's design must therefore pair every computed workpaper with at least one third-party or connector source document (the assay, the survey, the invoice) or a human review step. This matches SOX review controls.
- **Document kinds:** FA's list has no `Assay`, `Survey`, `WeighbridgeTicket`, `BillOfLading`, `TechnicalReport` or `PriceFixing`. Use `Workpaper`, `Spreadsheet`, `Contract`, `Rate`, `Disclosure` or `Other` now, and request registry rows upstream.
- **Rate documents:** `{source, base_unit, quote_unit, rate, as_of, retrieved_at}`. They are currency-only today (§5.9).

### 7.9 Close orchestration

- FA's CloseRun DAG (§7.5) is **NOT_STARTED** (V07). FrankenMining should orchestrate the mining close itself, in this order:
  1. bank reconciliations;
  2. stockpile and pad surveys;
  3. metal-balance reconciliation;
  4. inventory valuation and NRV;
  5. provisional-pricing MTM;
  6. DD&A and stripping;
  7. ARO accretion;
  8. royalties and mining taxes;
  9. stream deliveries and accretion;
  10. hedge MTM;
  11. JV billing and partner reconciliation;
  12. intercompany matching;
  13. FX revaluation (`fa revalue`);
  14. consolidation (`fa consolidate --certify`);
  15. statements and `IssueStatements`;
  16. non-GAAP metrics.
- Use FA's period controls (`SoftClose` stops ordinary agent posting, §7.3), year-end `fa close`, `StatementIssuance`, and checkpoint witnessing.
- When FA's CloseRun lands, FrankenMining's task list becomes `CloseTask` vertices with `DEPENDS_ON` edges.

### 7.10 Governance

- Each FrankenMining service or agent is an FA `Principal` (`kind`, `program: "franken_mining/<module>"`, `model`, `session`).
- Capabilities are scoped with `entities ⊆ {site entities}`, `accounts ⊆ subtree(...)`, `dimensions ⊆ {region=<site>}`, `amount ≤` and `daily_total ≤` ceilings, `branch = <draft>`, and `requires_dry_run` (§14.2).
- Large estimates (impairment, NRV, ARO revisions) go above agent ceilings, so a human approves.
- Dangerous classes (restatement, reopen, rule activation) already carry dual control (§14.3).
- **Caveat:** today's SoD binds asserted names and approvals are unsigned (V19), so FrankenMining must not present FA SoD as audited control evidence until `fa-0hs4.4` / `fa-z46b` / `fa-yok` close.

### 7.11 Transport

- **Preferred contract:** `fad` hosting the book (writer lease), with FrankenMining calling the `fa` robot-mode verbs (App. D NDJSON envelope, exit codes, refusal objects) or the `fa-mcp` tools for agent workflows. The verb registry is the cross-surface parity contract (§15.1).
- **Embedded Rust** (`fa::Book` / `AsyncBook`) is viable only if FrankenMining adopts FA's toolchain and pins: nightly `2026-08-31`, asupersync `78b64636`, frankengraphdb `b323bd17`, and the closed dependency universe (§1 constraint 1, §18.4). The facade API is still moving, and differs from the plan's §15.2 sketch. Use it for simulation and tests (`Book::create_memory`), not as the production seam.

### 7.12 Scale

- FA is single-writer per book. Unbatched posts are bounded by `1 / (2 × fsync p50)` (§17), and §20.1 measured 118 ms rising to 439 ms per post across 300 posts in a debug build.
- Real-book operability (V38) is NOT_STARTED, and all SLO rows are inactive.
- FrankenMining must therefore post **summaries, not transactions**: shift, day or month per site for production, per invoice for sales. It should batch with `PostBatch` and shard books by functional-currency group and basis.
- Truck-level, assay-level and survey-level detail stays in FrankenMining, linked by evidence digests.

---

## 8. The FrankenMining-owned mining domain pack (must build)

1. **Metal accounting and inventory subledger.** Covers ROM, stockpiles (including long-term low-grade, current versus non-current), heap-leach WIP (recoverable ounces, recovery curves), metal in circuit, doré, concentrate and cathode finished goods, and materials and supplies. Handles multi-attribute quantities (wet and dry tonnes, moisture, grade, contained, recoverable and payable metal per element), average costing, survey true-ups, mine-to-market metal balance (AMIRA P754-style), and NRV tests with per-basis reversal policy. Posts reviewed summaries via the FA external-subledger form.
2. **Production cost engine.** Mine-to-mill-to-refinery process costing, cost pools and absorption, by-product versus co-product treatment, and relative-sales-value driver tables for `Allocate`.
3. **Deferred stripping engine.** IFRIC 20 components and production-measure allocation, the stripping-asset UoP schedule, and US GAAP variable-cost treatment.
4. **Mineral property, asset register and DD&A engine.** Units-of-production rates from reserves (and resources where policy allows), prospective revisions, and component and straight-line assets.
5. **E&E policy engine.** IFRS 6 capitalization, impairment triggers, transfer to development, and US GAAP expense policy.
6. **Closure and rehabilitation (ARO) engine.** Estimates, inflation, discount rates (IFRS remeasurement versus ASC 410 layers), accretion, revisions, settlement, and the operating versus non-operating split.
7. **Impairment engine.** CGU and asset-group registry, LOM and price-deck models, FVLCD/VIU, the ASC 360 two-step, impairment allocation and reversal policy, run as FA scenarios for sensitivities.
8. **Concentrate and bullion sales engine.** Payables, TC/RC, penalties, QP and forward-curve MTM, provisional and final invoicing, assay exchange and umpire, title-transfer cutoff, and refinery metal accounts.
9. **Streams and royalties engine.** Deposits, per-ounce drawdown, financing component, cumulative catch-ups, and NSR/NPI/gross royalty computation and statements.
10. **Hedge accounting engine.** Designation, effectiveness, valuation, and OCI reserve recycling.
11. **Mining tax and government royalty engine**, per jurisdiction.
12. **JV and joint-arrangement engine.** IFRS 11 classification, operator billing and overhead, proportionate consolidation and equity method, and partner reporting via FA bundles.
13. **Capital project accounting.** AFE and budget control, CIP, sustaining versus growth classification, capitalized borrowing costs, and pre-production proceeds.
14. **Non-GAAP metrics engine.** WGC AISC and AIC, cash costs, copper C1, per-ounce and per-pound unit costs, attributable versus consolidated (via `fa party ownership`), with certified GAAP reconciliations that cite FA certificates.
15. **Reserves and resources register.** S-K 1300, NI 43-101 and JORC categories, the QP attestation workflow, and the reserve reconciliation. It feeds DD&A, stripping, impairment, ARO and stream drawdown.
16. **Mining chart spec, dimension map, Canon account-determination rule library, and StatementDefinitions.** Covers IFRS and US GAAP mining presentation, segment notes, and AISC reconciliation schedules.
17. **Mining close orchestrator** (until FA CloseRun exists) and **mining control procedures**: metal-balance variance, F1/F2/F3 grade reconciliation, stockpile variance, and capitalization tests. These run over FA certified reads and file results as evidence.

---

## 9. Upstream asks to franken_accounting (proposed beads)

Ordered by blocking impact. Note FA's bridge freeze: no new verbs, MCP tools or Lantern procedures until truth, operability and authority hold (§20.1). Registry-row asks are lighter.

**P0, blocking any real mining book:**

1. Metal and mass units, balanced and single-sided, plus exact conversion laws. Alternatively, implement `fa unit add` as a journaled unit (§4.1, §15.3).
2. Tiered NCI, multi-parent ownership, and cross-book consolidation with translation and CTA (fa-76hy; V27).
3. Custom dimension kinds (§4.1 `Custom`).
4. Custom REA event kinds (§4.1 `Custom(id)`).

**P1:**

5. Canon expressiveness per §10.3: quantity × price, division, table inputs, folds over lots, and `Allocation`/`Revaluation` output classes (V21, fa-c34).
6. Non-currency `Rate` documents and a commodity remeasurement law (relax `fa/src/fx.rs` `currency_units` for registered commodity laws).
7. New application difference kinds: provisional pricing, assay, TC/RC, penalty.
8. Mining document kinds: assay certificate, survey, weighbridge ticket, bill of lading, technical report, price fixing.
9. `InstrumentKind::{FixedAsset, Derivative}` per §4.1; the `Depreciate`/`Amortize` schedules family (V37).
10. Surfaced `AdjustmentLayer` books and `recon.book_to_book`; `BookToBookReasonCode` moved from a closed enum to registry data.
11. Allocation laws `Tiered` and `ByUnitsThenValue` (§6.3).
12. CloseRun DAG (V07).
13. A decision on whether equity-method and proportionate-consolidation laws belong in Concordat or stay in the domain pack.

**Status FrankenMining depends on:** FA-INV-12 (both clauses) and `FA-INV-01.dimensions` activation (V13); SoD with authenticated principals (V19); operability at scale (V38); a full exact-tree verdict (V35).

---

## 10. Risks for FrankenMining

- **Maturity risk.** FA's own audit says "No genuine full verdict exists" and that operability at scale is NOT_STARTED. FrankenMining should treat FA as an evolving dependency: pin its rev, run its own end-to-end acceptance tests against `fad`, and track V27/V37/V38.
- **Upstream contention.** FrankenMining's asks compete with FA's bridge freeze. Plan the FrankenMining accounting adapter so interim encodings work today: chart-based classification, `PostEntry` for unsupported event kinds, FrankenMining-side computation.
- **Performance.** The single writer and per-post cost growth force summary-level posting; size the batching accordingly.
- **Judgment boundary.** FA will never decide GAAP treatment (§0.2). Every mining policy choice (E&E, stripping, NRV reversal, CGU definition, sustaining classification) needs a named FrankenMining policy object, versioned and cited.
- **Evidence grading.** FrankenMining-generated workpapers are `Generated`, so FrankenMining must design reviewer attestations into its workflows or face draft-only posting for agents.
- **Dimension scarcity.** Eight fixed slots force careful mapping. Misusing `fund` or `grant` could trigger fund-balance closing semantics.

---

## 11. Vocabulary cheat-sheet for the FrankenMining plan

Use these exact FA terms when FrankenMining's plan references accounting:

- book, branch, trunk, draft, scenario, `BookCoordinate`, known-at / effective, `CommitSeq`, `TimeEvidence`;
- `Entry` / `Leg` / `Register` (a fold), `EntryRef`, `idempotency_key`;
- entry class (`Original`, `Adjusting`, `Reclassification`, `Allocation`, `Revaluation`, `Elimination`, `Reversal`, `Replacement`, `Restatement`, `PriorPeriodAdjustment`, `Migration`), supersession (`SUPERSEDES {Reverses | Replaces | Reclassifies | Restates | Revises | Corrects}`), reason code (`EstimateChange`, `PolicyChange`, ...);
- period states `Future/Open/SoftClosed/Closed/Locked`; admissibility matrix; `RestatementCase`;
- intents (`PostEntry`, `PostBatch`, `RecordEvent`, `RecordInvoice`, `ApplyReceipt`, `Accrue`, `Defer`, `Reclass`, `Allocate`, `Revalue`/`Translate`, `Depreciate`, `ImportDocument`, `AttachEvidence`, `Attest`, `AssertBalance`, `ForkDraft`/`PromoteDraft`, `ForkScenario`, `IssueStatements`); dry-run plan and `plan_certificate`; `confirm_plan`;
- Canon `RuleVersion`, activated by `PolicyControl`; golden corpus; backtest;
- `Event` (REA) with `DERIVED_FROM`/`GENERATED_BY`;
- `Subledger` (native, party-qualified control leaf) versus externally fed subledger (summary entry naming the detail batch digest; FA-INV-12);
- `Document` / `ImportReceipt` (`Connector{name, version}`) / provenance grade / `EVIDENCED_BY {role}` / self-sourced / `Attestation` / balance assertion `RegisterEquals`;
- `Unit` (commodity, single-sided), `ConversionLaw`, `Lot` (`RELIEVES`), `AllocationLaw`, `FxLaw`, `Rate` document;
- `LegalEntity`, `OWNS {pct}`, `fa party ownership` (effective stake), `ConsolidationRun`, NCI, CTA, `AdjustmentLayer`, `recon.book_to_book`;
- `StatementDefinition` (`Custom`, `RollForward`), certificate, `fa replay`, `StatementIssuance`, `AS ORIGINALLY REPORTED`;
- `.fab` bundle, `fa-verify`, `TransparencyCheckpoint`, witness;
- capability, caveat, profile (`Agent`, `Controller`, `Auditor`, ...), SoD;
- Lantern procedure and finding (statistical, never invariant);
- the subsystem names Folio, Abacus, Almanac, Palimpsest, Docket, Canon, Lantern, Concordat, Notary, Steward, Teller;
- the surfaces `fa` CLI (robot mode), `fad`, `fa-mcp`, and the exit codes 0/2–8.
