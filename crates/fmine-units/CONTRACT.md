# CONTRACT.md — `fmine-units`

## 1. Charter & Layer
- **Crate:** `fmine-units`
- **Layer:** `S0` (Substrate)
- **Subsystem:** `Bedrock`
- **LOC Budget:** `14k planned / 16k ceiling` (`registries/loc_budget.toml`)
- **Allowed Module Families:** `["qty", "grade", "exact", "money", "chem", "hazard"]`

## 2. Mathematical & Domain Formulation
Provides dimensional quantity algebra (`Qty<D, B, R>`), moisture-basis conversions (`Wet` vs `Dry`), non-additive `Grade<R>` (`INV-014`), exact rational metal accounting (`ExactQty`, `31.1034768 g/oz t`, `453.59237 g/lb`, `240/7 g/t per oz/st`), `Money { ccy, minor }` pro-rata allocation, chemical basis (`ChemBasis::AsN` vs `ChemBasis::AsNO3`), and hazardous byproduct tracking (`G-BYPROD-1`).

## 3. Authority, Epistemics & Representation
Enforces compile-time and runtime guards against silent unit, moisture-basis, currency, or chemical-species conversion (`INV-006`, `INV-014`).

## 4. Public API Surface
- `Qty<D, B, R>`, `MoistureFraction`, `MassFraction`, `to_dry_mass`, `to_wet_mass`
- `Grade<R>`, `mass_weighted_mean`, `DryMassGrade<R>` (a dry mass with its grade)
- `ExactQty`: `new`, `from_integer`, `from_decimal_str` (sign, fraction, exponent, exact),
  `checked_{add, sub, neg, mul, div, cmp}`, `signum` (bd-4wvl.1). Every overflow, including negating
  `i128::MIN` in `new`, is `UnitsError::Overflow`, never a wrapped value.
- `Currency`, `Money`, `allocate_pro_rata`
- `ChemBasis`, `No3Loading`, `check_who_no3_limit`
- `HazardousByproduct`, `StreamMassBalance`

## 5. Guards & Typed Refusals
- `FMINE-REF-101` (`UnitDimensionMismatch`)
- `FMINE-REF-102` (`MoistureBasisMissing`)
- `FMINE-REF-103` (`CurrencyFxMissing`)
- `FMINE-REF-107` (`ChemicalBasisMismatch`, `G-ENV-1`)
- `FMINE-REF-206` (`HazardousByproductOmitted`, `G-BYPROD-1`)

## 6. Proof Obligations (`PO-*`)
- `PO-UNIT-1`: $10^4$ randomized `ExactQty` round-trips (`g <-> oz_t`, `g/t <-> oz/st`) have zero residual.
- `PO-UNIT-2`: `79 mg/L as N = 349.72 mg/L as NO3 > 50 mg/L` WHO threshold (`FMINE-REF-107`).

## 7. Failure Modes & Deterministic Fallbacks
Invalid moisture fractions (`>= 1.0` or `< 0.0`) or mismatched currencies fail closed with typed `Refusal`.

## 8. Gates & Kill Criteria (`GATE-*` / `KILL-*`)
- `GATE-UNIT-01`
- `GATE-LAYERS-00` (`L-6` no bare `f64` quantities in K/E public signatures)

## 9. Lifted Code & Foundation Dependencies
- Depends on `fmine-core`.

## 10. Non-Goals & Scope Boundary
Does not implement `std::ops::Add` on `Grade<R>` (`INV-014`).
