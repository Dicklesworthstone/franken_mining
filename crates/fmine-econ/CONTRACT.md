# CONTRACT.md — `fmine-econ`

## 1. Purpose
Helm K1 economic valuation and fiscal-regime kernel (`BD-228`, `BD-296`, plan §15.4, §23.4, `INV-013`):
- `price_deck`: versioned commodity price decks (`Au`, `Cu`, `Ag`, `Sn`) with explicit currency/unit basis, canonical SHA-256 digests, `INV-013` staleness refusal when superseded (`FMINE-REF-502`), and royalty deck-mixing audit;
- `fiscal`: exact-`Money` and exact-`ExactQty` Net Smelter Return (NSR), flat ad-valorem and price-stepped royalties (e.g. Mali Loi n°2023-040), corporate income tax with minimum turnover floors, price-contingent super-profits tax (e.g. DRC/Kibali Art. 251 bis), and exact discounted cash-flow NPV;
- `regime`: `CountryRegime` registry parser and evaluator (`registries/country_regimes.toml`) carrying statutory instrument, as-of date, supersession date, `Restricted{Reagent|Water|StockpileOnly}` and `NotPublished` operational states, price-contingent fiscal transforms, and the Kibali `$85.5/oz` deck-mix detector (`FACT E244`).

- **Crate:** `fmine-econ`
- **Layer:** K1
- **Subsystem:** Helm
- **LOC budget:** 20,000 planned / 23,000 ceiling
- **Allowed Module Families:** `["price_deck", "fiscal", "regime"]`

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
All files (`lib.rs`, `price_deck.rs`, `fiscal.rs`, `regime.rs`) declare `//! Determinism: BitwisePortable`. All arithmetic is exact `i128` rational/fixed-point (`ExactQty` / `Money`) over `BTreeMap` collections.

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

## 12. Proof obligations
- `BD-228` (exact versioned price decks, NSR, royalties, tax known answers): `fiscal::tests::test_versioned_price_decks_nsr_royalties_and_tax_known_answers`.
- `BD-296` (Kibali & Loulo rows, `Restricted` / `NOT_PUBLISHED`, `$85.5/oz` deck mix flag): `regime::tests::test_country_regimes_kibali_loulo_restricted_not_published_and_deck_mix_flag`.
