# CONTRACT.md — `fmine-econ`

## 1. Purpose
Helm K1 economic valuation and fiscal-regime kernel (`BD-228`, `BD-296`, plan §15.4, §23.4, `INV-013`):
- `price_deck`: versioned commodity price decks (`Au`, `Cu`, `Ag`, `Sn`) with explicit currency/unit basis, canonical SHA-256 digests, `INV-013` staleness refusal when superseded (`FMINE-REF-502`), and royalty deck-mixing audit;
- `fiscal`: exact-`Money` and exact-`ExactQty` Net Smelter Return (NSR), flat ad-valorem and price-stepped royalties (e.g. Mali Loi n°2023-040), corporate income tax with minimum turnover floors, price-contingent super-profits tax (e.g. DRC/Kibali Art. 251 bis), and exact discounted cash-flow NPV;
- `regime`: `CountryRegime` registry parser and evaluator (`registries/country_regimes.toml`) carrying statutory instrument, as-of date, supersession date, `Restricted{Reagent|Water|StockpileOnly}` and `NotPublished` operational states, price-contingent fiscal transforms, and the Kibali `$85.5/oz` deck-mix detector (`FACT E244`).
- `entity_graph` (`BD-314`, plan §23.2–§23.4): `CapitalEntityGraph` DAG validation, canonical digest, multi-hop effective equity fractions, Barrick Parent vs. `NA-NewCo` IPO carve-out dual-denominator `ΔNAV/share`/AISC/FCF-yield evaluation, NGM (61.5%/38.5%) & Pueblo Viejo (60%/40%) capital call/dilution, Fourmile vend-in vs. inter-entity tolling option (marked walled under `INV-007`), Kibali (45/45/10) repatriation gates integrated with `CountryRegime`, Twiga (84/16 + 50/50) economic benefits sharing annual true-up, Reko Diq (50/25/15/10) DSCR ring-fence & free-carry recovery, and Loulo-Gounkoto H1 2026 reset & Tongon divestiture fixtures.

- **Crate:** `fmine-econ`
- **Layer:** K1
- **Subsystem:** Helm
- **LOC budget:** 20,000 planned / 23,000 ceiling
- **Allowed Module Families:** `["price_deck", "fiscal", "regime", "npv", "entity_graph"]`

## 2. Inputs
- Versioned `PriceDeck` records carrying `fmine_units::Money` prices and explicit `PriceUnit`.
- Ore metal grades, recoveries, payabilities, TC/RC/refining deductions, and fiscal parameters expressed strictly in `fmine_units::Money` and `fmine_units::ExactQty` (zero bare `f64` parameters in public K-layer signatures; L-6).
- `registries/country_regimes.toml` text passed by caller (this K1 crate performs zero I/O, zero clocks, zero threads; L-3).
- Dependencies: `fmine-core`, `fmine-units`, `fmine-evidence` (`K1 → S0`, zero external crates).

## 3. Outputs
- Exact `NsrBreakdown`, `FiscalBreakdown`, `DeckMixAudit`, `RegimeFiscalEvaluation`, and exact `Money` NPV results, plus structured `EconError` mapped to registered `FMINE-REF-*` refusal codes.
- Emits no `Claim`s directly (upstream Helm engines wrap outputs in `Claim`s with provenance).

## 4. Invariants
- **Exact Money arithmetic (`BD-026`, `BD-228`):** all monetary amounts stay in exact `fmine_units::Money` (`10^-4` major units) and `ExactQty` rationals; currency mismatches refuse with `FMINE-REF-105` (no implicit FX).
- **INV-013 (versioned price-deck & regime staleness):** evaluating a superseded `PriceDeck` or superseded `CountryRegimeRow` in live mode refuses with `FMINE-REF-502`.
- **No silent fill for `NotPublished` (`RISK-26`, `BD-296`):** a `CountryRegimeRow` in `RegimeState::NotPublished` refuses valuation with `FMINE-REF-201` rather than fabricating zero or carrying forward prior periods.
- **Deck-mix detection (`FACT E244`, `BD-296`):** `audit_royalty_deck_mix` flags any quoted royalty-per-ounce that disagrees with `stated_rate × stated_price`, recovering the implied mixed deck price (e.g., Kibali `$85.50/oz` at `$2,000/oz` -> implied `$1,500/oz` deck).

## 5. Refusals
- `EconError::StaleVersion` → `FMINE-REF-502` (exit 6): superseded price deck or fiscal regime evaluated in live mode (`INV-013`).
- `EconError::CurrencyMismatch` / `EconError::UnitMismatch` → `FMINE-REF-105` (exit 3): cross-currency or mismatched-unit combination without explicit FX/conversion.
- `EconError::MissingPrice` / `EconError::NotPublished` → `FMINE-REF-201` (exit 3): missing commodity price in deck or `NOT_PUBLISHED` site-quarter.
- `EconError::InvalidDomain` / `EconError::Overflow` → `FMINE-REF-109` (exit 3): negative prices, out-of-range rates, or exact arithmetic overflow.
- `EconError::RegistryParse` → `FMINE-REF-107` (exit 3): malformed `country_regimes.toml` schema or duplicate row id.

## 6. Determinism class
All files (`lib.rs`, `price_deck.rs`, `fiscal.rs`, `regime.rs`, `npv.rs`, `entity_graph.rs`) declare `//! Determinism: BitwisePortable`. All arithmetic is exact `i128` rational/fixed-point (`ExactQty` / `Money`) over `BTreeMap` collections.

## 7. Evidence class of outputs
Not applicable: deterministic K1 economic and fiscal arithmetic kernel; callers wrap outputs in `Claim`s carrying source provenance.

## 8. Validity domain
- Exact rational and `Money` (`i128` × `10^-4`) arithmetic within `i128` overflow bounds (overflow refuses with `FMINE-REF-109`).
- Country-regime rows reflect documented statutory/contractual instruments as of their declared `as_of_date` (`plan §23.4`, `FACT E202`, `E242–E246`, `E278`, `E281`, `E410–E449`).

## 9. Oracles
- Hand-worked exact NSR, flat/stepped royalty, CIT, super-profits tax, and NPV tests (`fiscal::tests::test_versioned_price_decks_nsr_royalties_and_tax_known_answers`).
- Documented Kibali (`5.7%` royalty, `50%` super-profits tax > `$2,000/oz`, `$85.5/oz` deck-mix flag) and Loulo-Gounkoto (1991 `6%` superseded vs 2023 Loi n°2023-040 `17.85%` at `$4,500/oz`) reproduction tests (`regime::tests::test_country_regimes_kibali_loulo_restricted_not_published_and_deck_mix_flag`).

## 10. Known gaps
- Stochastic `CountryRegime` Markov transition matrices, scenario-index common-random-numbers sampling, and `CapitalProgram` MILP optimization live in `fmine-capital` (`BD-229`, `BD-314`).

## 11. Public API
- `EconError`
- `price_deck::{Commodity, PriceUnit, PricePoint, PriceDeck, DeckMixAudit, audit_royalty_deck_mix}` (`BD-228`, `BD-296`)
- `fiscal::{MetalStreamSpec, RoyaltySchedule, CorporateTaxTerms, SuperProfitsBase, SuperProfitsTax, NsrBreakdown, FiscalBreakdown, compute_metal_nsr, compute_royalty_rate, compute_royalty_amount, compute_fiscal_breakdown, exact_npv}` (`BD-228`)
- `regime::{RestrictionCause, RegimeState, CountryRegimeRow, RegimeFiscalEvaluation, parse_country_regime_registry, evaluate_regime_fiscal_terms}` (`BD-296`, `registries/country_regimes.toml`)
- `npv::{DiscountConvention, TaxBasis, ExactInterval, ConventionEvalRow, ConventionDiscovery, ConventionDiscoveryRequest, IrrRoot, IrrScanRequest, sqrt_enclosure, npv_interval, discover_discounting_convention, count_cash_flow_sign_changes, solve_irr_bracketed, find_all_bracketed_irrs}` (`BD-350.S4`)
- `npv::evaluate_discounting_conventions` also exposes zero or multiple overlaps to a caller
  assessing competing valuation dates. It uses the same exact enclosures and numerical refusals.
- `entity_graph::{EntityNodeId, EntityKind, EntityNode, WaterfallEdgeKind, WaterfallEdge, CapitalEntityGraph, EntityDenominatorInput, EntityDenominatorResult, DualEntityEvaluation, JvCapitalCallInput, JvCapitalCallOutcome, FourmilePreferredPath, FourmileOptionInput, FourmileOptionComparison, KibaliWaterfallInput, KibaliWaterfallResult, TwigaWaterfallInput, TwigaWaterfallResult, RekoDiqWaterfallInput, RekoDiqWaterfallResult, LouloGounkotoResetInput, LouloGounkotoResetResult, DivestiturePeriodOutcome, evaluate_dual_entity_delta_nav, evaluate_jv_capital_call, compare_fourmile_vend_in_vs_tolling, evaluate_kibali_waterfall, evaluate_twiga_waterfall, evaluate_reko_diq_waterfall, evaluate_loulo_gounkoto_reset, evaluate_divested_asset_period, barrick_2026_portfolio_graph}` (`BD-314`)

## 12. Proof obligations
- `BD-228` (exact versioned price decks, NSR, royalties, tax known answers): `fiscal::tests::test_versioned_price_decks_nsr_royalties_and_tax_known_answers`.
- `BD-296` (Kibali & Loulo rows, `Restricted` / `NOT_PUBLISHED`, `$85.5/oz` deck mix flag): `regime::tests::test_country_regimes_kibali_loulo_restricted_not_published_and_deck_mix_flag`.
- `BD-350.S4` (exact rational half-year enclosures, discounting convention discovery, refusing bracketed IRR): `npv::tests::test_npv_end_year_24_50`, `npv::tests::test_npv_mid_year_all_25_10`, `npv::tests::test_npv_year0_undiscounted_27_57`, `npv::tests::test_printed_25_1_identifies_mid_year_all`, `npv::tests::test_irr_two_sign_changes_never_picks_one`, `npv::tests::test_irr_refuses_not_converged`.
- `BD-314` (`CapitalEntityGraph` dual-entity `ΔNAV/share`, Fourmile vend-in vs. tolling, NGM/PV capital call dilution, Kibali/Twiga/Reko Diq waterfalls, Loulo-Gounkoto H1 2026 reset & Tongon divestiture): `entity_graph::tests::test_barrick_portfolio_graph_topology_effective_equity_and_digest_invariance`, `entity_graph::tests::test_dual_entity_delta_nav_and_fourmile_vend_in_vs_tolling`, `entity_graph::tests::test_ngm_and_pv_capital_call_dilution_conservation`, `entity_graph::tests::test_kibali_twiga_and_reko_diq_jv_waterfalls_exact_conservation`, `entity_graph::tests::test_loulo_gounkoto_h1_2026_reset_and_tongon_divestiture_fixtures`.
