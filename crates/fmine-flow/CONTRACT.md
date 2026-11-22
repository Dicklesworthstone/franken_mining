# CONTRACT.md — `fmine-flow`

## 1. Purpose
Touchstone K1 material-flow graph and exact extensive conservation kernel (`BD-146`, plan §20.1–§20.2, `INV-005`, `PO-BED-6`, `G-TOUCH-3`):
- `graph`: mine-to-market directed material-flow graph covering all eight §20.1 node classes (in-situ volumes, blasted/dug, transport, ROM pads/stockpiles/heaps, plant units, products/market, cross-complex/third-party sources, waste/tailings), explicit stockpile mixing models (`WellMixed`, `Fifo`, `Layered`), topology validation (no dangling endpoints, no self-loops, unique IDs), and `G-TOUCH-3` stockpile-decoupling detection;
- `balance`: exact rational (`ExactQty`) extensive conservation verification (`Σ in − Σ out − Δ inventory = 0` within exact registered tolerance) over dry mass, water, and per-element metal, refusing violations with `FMINE-REF-101` (never clamping or plugging) and computing derived intensive grades (`metal ÷ dry_tonnes`, refusing zero/negative mass rather than plugging).

- **Crate:** `fmine-flow`
- **Layer:** K1
- **Subsystem:** Touchstone
- **LOC budget:** 20,000 planned / 23,000 ceiling
- **Allowed Module Families:** `["graph", "balance", "ffactors", "r0_parity", "verbatim_table", "compensation"]`

## 2. Inputs
- Pure in-memory `FlowNode` and `FlowStream` declarations validated into a `FlowGraph`.
- Per-period extensive `PeriodBalanceInput` observations expressed strictly in `fmine_units::ExactQty` (zero bare `f64` parameters in public K-layer signatures, enforcing L-6).
- Dependencies: `fmine-core`, `fmine-units`, `fmine-evidence` (`K1 → S0`). Performs zero I/O, zero clocks, zero threads.

## 3. Outputs
- Validated `FlowGraph`, `G-TOUCH-3` decoupling verdicts, `BalanceReport` with exact per-node/per-element residuals (`ExactQty`), derived intensive grades, and `FlowError` mapped to registered `FMINE-REF-*` refusals.
- Emits no `Claim`s directly (upstream reconciliation engines wrap results in `Claim`s).

## 4. Invariants
- **Graph integrity (`BD-146`):** unique non-empty node/stream IDs, zero self-loops, zero dangling endpoints, every `StockpileOrPad` holds inventory with an explicit `StockpileMixingModel`, and non-inventory nodes reject stockpile mixing declarations.
- **G-TOUCH-3 (no naive ratio across stockpiles):** `check_direct_ratio_admissible` refuses with `FMINE-REF-102` whenever any path between a mining node and a processing node traverses an inventory-holding stockpile/pad.
- **INV-005 / PO-BED-6 (exact extensive conservation, never plug):** `verify_period_balance` evaluates `Σ in − Σ out − Δ inventory` in exact `ExactQty` rationals per node and per extensive channel (dry tonnes, water, elements) and refuses with `FMINE-REF-101` whenever any channel exceeds its exact tolerance.
- **Metamorphic split/merge invariance:** splitting or merging a stockpile node leaves whole-system extensive balances bit-identical (`test_stockpile_split_merge_metamorphic_conservation`).

## 5. Refusals
- `FlowError::ConservationViolation` → `FMINE-REF-101` (exit 3): exact extensive residual exceeded registered tolerance.
- `FlowError::StockpileDecouplingViolation` → `FMINE-REF-102` (exit 3): naive mined÷milled ratio attempted across an intervening stockpile (`G-TOUCH-3`).
- `FlowError::NegativeOrZeroDenominator` / `FlowError::InvalidQuantity` → `FMINE-REF-109` (exit 3): non-positive mass denominator or negative physical quantity.
- `FlowError::InvalidGraph` / `FlowError::DuplicateId` / `FlowError::UnknownNode` / `FlowError::UnknownStream` → `FMINE-REF-107` (exit 3).

## 6. Determinism class
All files (`lib.rs`, `graph.rs`, `balance.rs`, `ffactors.rs`, `r0_parity.rs`, `verbatim_table.rs`, `compensation.rs`) declare `//! Determinism: BitwisePortable`. Arithmetic is 100% exact `i128` rational (`ExactQty`) over `BTreeMap`/`BTreeSet` collections.

## 7. Evidence class of outputs
Not applicable: pure structural graph and exact conservation law checker; callers wrap outputs in `Claim`s.

## 8. Validity domain
- Exact rational conservation over finite directed graphs up to `i128` `ExactQty` bounds (overflow returns `FlowError::Overflow`, never wraps).
- Extensive channels only (dry tonnes, water, elemental mass); grades are strictly derived after conservation checks.

## 9. Oracles
- Hand-worked exact mine-to-mill-to-tailings conservation tests, `G-TOUCH-3` stockpile decoupling refusal tests, and stockpile split/merge metamorphic tests in `graph.rs` and `balance.rs`.

## 10. Known gaps
- R1 weighted-least-squares linear reconciliation (`BD-148`), R2 hierarchical Bayesian state-space smoother (`BD-151`), and identifiability refusal probes (`BD-152`) live in subsequent WP-08 beads built on top of `FlowGraph`.

## 11. Public API
- `FlowError`
- `graph::{NodeClass, StockpileMixingModel, FlowNode, FlowStream, FlowGraph}`
- `balance::{ExtensiveQuantities, StreamFlow, InventoryDelta, BalanceTolerance, NodeResidual, BalanceReport, verify_period_balance, derive_grade}`
- `ffactors::{FFactorFamily, FFactorDefinition, ExactPosteriorTriplet, PosteriorFFactor, F3Decomposition, parse_ffactor_registry, compute_posterior_ffactor, decompose_f3, format_exact_pct_4dp, render_ffactor_g_touch_4}` (`BD-148`, `BD-278`, `G-TOUCH-4`, `registries/ffactors.toml`)
- `r0_parity::{SitePackRow, R0ParityRowVerdict, R0ParityReport, run_r0_site_parity}` (`BD-160`, R0 site-convention parity mode)
- `verbatim_table::{RowRole, DerivedGrade, DiscrepancyKind, DiscrepancyRecord, VerbatimTableRowInput, ImportedReconciliationRow, VerbatimRatioRowInput, VerbatimRatioAudit, grams_per_troy_oz, import_verbatim_table_row, audit_verbatim_ratio_row}` (`BD-279`, `G-TOUCH-6`, `E206–E209`)
- `compensation::{CompensationClass, WithinFactorInput, InvestigationTrigger, WithinFactorVerdict, AnnualFactorRow, SignedBiasPrior, McfCheckOutDivergence, MultiYearSeriesAudit, detect_within_factor_compensation, audit_mcf_checkout_vs_checkin, audit_multi_year_series}` (`BD-280`, §20.4–§20.5, `E154–E156`, `KA-TOUCH-KIBALI-2016-2025` `E210–E212`)

## 12. Proof obligations
- `PO-BED-6` / `INV-005` (exact extensive conservation at commit, no plugs): `balance::tests::test_exact_conservation_closure_and_refusal_no_plug`, `balance::tests::test_stockpile_split_merge_metamorphic_conservation`.
- `G-TOUCH-3` / `BD-146` (graph invariants & stockpile decoupling): `graph::tests::test_flow_graph_invariants_and_g_touch_3_decoupling`.
- `G-TOUCH-4` / `BD-278` (site-specific F-factor & MCF registry keys + versioned renders): `ffactors::tests::test_site_specific_ffactor_registry_and_g_touch_4_rendering`.
- `BD-160` (R0 parity mode reproducing synthetic site packs): `r0_parity::tests::test_r0_parity_reproduces_synthetic_site_packs_and_enforces_profile_isolation`.
- `G-TOUCH-6` / `BD-279` (verbatim reconciliation-table importer & E206–E209 discrepancy filing): `verbatim_table::tests::test_kibali_2024_mcf_loulo_rcf_and_g_touch_6_residual_discrepancies`.
- `BD-280` (within-factor compensation & `KA-TOUCH-KIBALI-2016-2025` signed bias prior): `compensation::tests::test_goldstrike_roaster_2024_within_factor_compensation_and_negative_controls`, `compensation::tests::test_ka_touch_kibali_2016_2025_series_compensation_and_signed_bias_prior`.
