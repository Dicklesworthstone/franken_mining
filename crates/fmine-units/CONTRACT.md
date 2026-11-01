# CONTRACT.md — `fmine-units`

## 1. Purpose
Quantities for S0 (plan §25). It provides:
- `Qty<D, B, R>`: a dimension, a moisture basis and a metal stage carried in the type;
- the non-additive `Grade<R>`, averaged only by dry-mass weighting;
- `ExactQty`, an `i128` rational, with the exact troy-ounce, pound and `240/7` g/t-per-oz/st constants;
- `Money { ccy, minor }`, with no implicit FX;
- the chemical basis, as a run-time tag (`ChemConc`) and as a type (`Conc<B>`);
- `ImportedValue`, which keeps an undeclared unit under `UNREGISTERED_UNIT`;
- `HazardousByproduct`, which converts only to a storage liability (G-BYPROD-1).

Callers: fmine-metrics, fmine-numerics and fmine-public. It has no geostatistical supports (`fmine-geostat`) and reads no registry: `ImportedValue::import` takes the registered unit list from its caller.

- **Crate:** `fmine-units`
- **Layer:** S0
- **Subsystem:** Bedrock
- **LOC budget:** 10,000 planned / 12,000 ceiling
- **Allowed Module Families:** `["qty", "grade", "exact", "money", "chem", "hazard"]`

## 2. Inputs
- `f64` magnitudes for `Qty`, `Grade` (g/t dry), moisture and mass fractions, and concentrations (mg/L).
- `i128` numerators and denominators, or decimal strings, for `ExactQty`; `i128` minor units for `Money`.
- `u64` weights for `Money::allocate_pro_rata`, and an explicit `FxRate` for currency conversion.
- A declared unit string and the registered unit list, for `ImportedValue::import`.

## 3. Outputs
Typed quantities, grades, exact values, money and concentrations, plus `UnitsError`. None is a Claim.

## 4. Invariants
- INV-014, no averaging of non-additive attributes. `Grade` has no `Add`; its one average is `mass_weighted_mean` over dry mass. Splitting or merging lots leaves the blended grade and the metal unchanged: `test_po_unit_3_split_and_merge_preserve_grade_and_metal`.
- Basis, stage and currency are never mixed silently. These are compile-fail doctests, each checked against its error code by the pinned nightly:
  - adding Wet and Dry, or Contained and Recoverable, `Qty` (E0308);
  - `Grade + Grade` (E0369);
  - `metal_grams` on wet tonnes (E0308);
  - `Money + Money` (E0369);
  - a `Conc<AsN>` sample against a `Conc<AsNO3>` limit (E0308);
  - a conversion with no registered `StoichLaw` (E0277).
- `ExactQty` arithmetic is exact or refuses: every overflow, including negating `i128::MIN` in `new`, is `UnitsError::Overflow`, never a wrapped value. Tests: `test_exactqty_add_sub_neg_cmp_known_answers`, `test_exactqty_add_overflow_is_error`, `test_exactqty_from_decimal_str_exact`.
- `allocate_pro_rata` conserves the minor units exactly, by largest remainder with ties to the lower index: `test_money_pro_rata_conservation`.
- The two chemical-basis forms agree. `ChemConc::typed::<B>()` is the only way from the tag to the type, and the two give the same bits for the same law over 10,000 seeded values both ways: `test_typed_nitrate_matches_the_untyped_law_and_refuses_another_basis`.

## 5. Refusals
`UnitsError::refusal_code` (`src/lib.rs:38`) maps three codes. They are strings because the crate sits below `fmine-evidence`, and `xtask check-refusals` requires each to be a registry code. `test_refusal_codes_match_the_registry_rows` pins the 105 and 107 rows' keys, but not 109's.
- FMINE-REF-109 (outside physical domain, exit 3, status `proposed`) covers three variants. Each value is refused, never clamped:
  - `MoistureOutOfBounds`: outside `[0, 1)`, or not finite.
  - `FractionOutOfBounds`: outside `[0, 1]`, or not finite. Test: `test_fraction_outside_unit_interval_refuses_109_never_clamps`.
  - `ConcentrationOutOfDomain`: a `Conc<B>` that is negative or not finite, including a conversion that would overflow. Test: `test_concentration_outside_its_domain_refuses_109`.
- FMINE-REF-105 (basis mismatch, exit 3, active) covers three variants:
  - `ChemicalBasisMismatch` (G-ENV-1): a comparison across bases without the law, or typing a `ChemConc` as another basis. Test: `test_nitrate_basis_conversion`.
  - `CurrencyMismatch`: in `Money::checked_add` and `checked_sub`, and in `FxRate::convert`. Test: `test_money_checked_add_refuses_a_currency_mismatch`.
  - `UnitMismatch`: two different registered units. Test: `test_two_registered_units_refuse_and_one_unit_adds`.
- FMINE-REF-107 (undeclared unit, exit 3, active): `UnregisteredUnit`, raised when an `ImportedValue` held under `UNREGISTERED_UNIT` is combined. Test: `test_unregistered_unit_is_viewable_and_refused_in_a_sum`.

`SimplexSumInvalid`, `ZeroMassWeight` and `Overflow` carry no code.

## 6. Determinism class
Undeclared: `src/lib.rs` has no `//! Determinism:` header, which `xtask check-det` requires only in the K and E layers. `ExactQty` and `Money` are integer arithmetic. The `f64` paths use only `+ - * /`, and production code calls no transcendental.

## 7. Evidence class of outputs
Not applicable: no Claims are emitted, so no `EvidenceClass` or `KnowledgeState` is assigned.

## 8. Validity domain
- `Qty::new` and `Grade::from_g_per_t` accept any `f64`, including negative and NaN values. Only `MoistureFraction`, `MassFraction`, `SimplexVector` (components in `[0, 1]`, sum `1 ± 1e-9`) and `Conc<B>` check their domain.
- `mass_weighted_mean` refuses a negative or non-finite mass, a non-finite grade, and a zero total mass. All of these come back as `ZeroMassWeight`, and a negative grade is accepted.
- `ExactQty` holds an `i128` numerator and denominator, and refuses anything past them. It reports a zero denominator in `new` as `Overflow`.
- `Money` holds `i128` minor units at `SCALE = 10,000` per major unit. `from_millions_f64` casts with `as i128`: NaN becomes 0 and infinities saturate, without an error. `FxRate::convert` truncates toward zero. It reports a zero `rate_den` as `Overflow`, and it accepts negative rates.

## 9. Oracles
- The exact constants are the definitions: 1 troy oz = 31.1034768 g and 1 lb = 453.59237 g, so 1 oz/st = 240/7 g/t. `test_exact_statutory_conversions_10k_roundtrip` round-trips 10,000 values (i/100 oz, i/50 lb, i/10 oz/st, a sweep, not random draws) with zero residual.
- Nitrate: `62.0049 / 14.0067`, from the molar masses of NO3 and N, with the 50 mg/L WHO guideline for NO3. `test_nitrate_basis_conversion` checks that 79 mg/L as N is above that guideline. No external table is pinned.

## 10. Known gaps
- BD-023 (bd-023), BD-024 (bd-024), BD-025 (bd-025), BD-026 (bd-026), BD-027 (bd-027) and BD-277 (bd-277): the code and tests exist, but each stays open behind the GATE-LAYERS-00 chain. bd-023 waits on bd-016; the others wait on bd-023, bd-024 or bd-025.
- Unchecked constructors, `ZeroMassWeight` standing for several distinct errors, and the `Money` and `FxRate` behaviour above (see Validity domain). Nothing in the workspace outside this crate calls `from_millions_f64` or `FxRate`, and no bead covers them yet.
- The dimension and moisture-basis guards are compile-time types or typed errors, not §8.4 codes. G-BYPROD-1 is a type, not a refusal: `HazardousByproduct` converts only to a `StorageLiabilitySchedule` (`test_hazardous_byproduct_cannot_monetize`).
- The doc comment on `Grade` cites INV-012 for non-additivity; the plan's invariant is INV-014.

## 11. Public API
- `UnitsError` (`refusal_code`)
- The marker types:
  - dimensions `Mass`, `Length`, `Volume`, `Energy`;
  - bases `Wet`, `Dry`, `InSitu`, `Neutral`;
  - stages `Contained`, `Recoverable`, `Produced`, `Payable`, `Sold`.
- `Qty<D, B, R>`, `MoistureFraction`, `MassFraction`, `wet_to_dry`, `metal_grams`. Test: `test_wet_to_dry_and_contained_metal`.
- `Grade<R>` (`from_g_per_t`, `g_per_t`, `apply_recovery`), `DryMassGrade<R>`, `mass_weighted_mean`
- `ExactQty`:
  - constructors `new`, `from_integer`, `from_decimal_str`;
  - arithmetic `checked_{add, sub, neg, mul, div, cmp}`, `signum`;
  - conversions `troy_oz_to_grams`, `grams_to_troy_oz`, `avdp_lb_to_grams`, `grams_to_avdp_lb`, `oz_per_st_to_g_per_t`, `g_per_t_to_oz_per_st`, `to_f64`;
  - the `*_NUM` and `*_DEN` constants.
- `Currency`, `Money` (`from_minor`, `from_major`, `from_millions_f64`, `checked_add`, `checked_sub`, `to_major_f64`, `allocate_pro_rata`), `FxRate` (`convert`)
- `UNREGISTERED_UNIT`, `ImportedValue` (`import`, `value`, `unit`, `declared`, `checked_add`)
- `ChemBasis`, `NO3_TO_N_STOICHIOMETRIC_RATIO`, `ChemConc` (`convert_basis`, `exceeds_strict`, `typed`)
- `ChemBasisTag`, `StoichLaw<T>` (only between `AsN` and `AsNO3`), `Conc<B>` (`new`, `exceeds`, `convert`, `untyped`)
- The basis types `AsN`, `AsNO3`, `CnWad`, `CnFree`, `CnTotal`, `AsCaCO3`, `Dissolved`, `TotalRecoverable`.
- `SimplexVector<N>`, `HazardousByproduct` (`into_storage_liability`), `StorageLiabilitySchedule`

## 12. Proof obligations
- PO-UNIT-1 (exact conversions round-trip): `test_exact_statutory_conversions_10k_roundtrip`. It is a 10,000-value sweep, where the plan names property tests over random quantities.
- PO-UNIT-2 (no K or E public function takes a bare `f64` for a physical quantity) is not discharged here: it is `xtask check-claims` (L-6), with `test_seeded_bare_f64_quantity_fails`. The old contract gave this number to the nitrate check, which belongs to BD-277.
- PO-UNIT-3 (mass-weighted means are invariant to splitting and merging lots): `test_po_unit_3_split_and_merge_preserve_grade_and_metal`, seeded, with a relative tolerance of 1e-9.
