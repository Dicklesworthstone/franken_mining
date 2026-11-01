# CONTRACT.md — `fmine-units`

## 1. Charter & Layer
- **Crate:** `fmine-units`
- **Layer:** `S0` (Substrate)
- **Subsystem:** `Bedrock`
- **LOC Budget:** `14k planned / 16k ceiling` (`registries/loc_budget.toml`)
- **Allowed Module Families:** `["qty", "grade", "exact", "money", "chem", "hazard"]`

## 2. Mathematical & Domain Formulation
Provides dimensional quantity algebra (`Qty<D, B, R>`), moisture-basis conversions (`Wet` vs `Dry`), non-additive `Grade<R>` (`INV-014`), exact rational metal accounting (`ExactQty`, `31.1034768 g/oz t`, `453.59237 g/lb`, `240/7 g/t per oz/st`), `Money { ccy, minor }` pro-rata allocation, chemical basis (`ChemBasis::AsN` vs `ChemBasis::AsNO3`), and hazardous byproduct tracking (`G-BYPROD-1`).

Chemical basis has two forms (`BD-277`, plan §25.2). `ChemConc` carries the basis as a run-time tag, as values arrive at
the import boundary, and refuses a mismatch with `FMINE-REF-105`. `Conc<B>` carries it as a type: a sample meets a
limit only in the same basis, and changes basis only through a registered `StoichLaw` (only `AsN` and `AsNO3` have
one, `× 62.0049 / 14.0067` and its inverse). `ChemConc::typed::<B>()` is the only way from the first form to the
second; both give the same bits for the same conversion.

## 3. Authority, Epistemics & Representation
Enforces compile-time and runtime guards against silent unit, moisture-basis, currency, or chemical-species conversion (`INV-006`, `INV-014`).

## 4. Public API Surface
- `Qty<D, B, R>`, `MoistureFraction`, `MassFraction`, `to_dry_mass`, `to_wet_mass`
- `Grade<R>`, `mass_weighted_mean`, `DryMassGrade<R>` (a dry mass with its grade)
- `ExactQty`: `new`, `from_integer`, `from_decimal_str` (sign, fraction, exponent, exact),
  `checked_{add, sub, neg, mul, div, cmp}`, `signum` (bd-4wvl.1). Every overflow, including negating
  `i128::MIN` in `new`, is `UnitsError::Overflow`, never a wrapped value.
- `Currency`, `Money`, `allocate_pro_rata`
- `ChemBasis`, `ChemConc`, `No3Loading`, `check_who_no3_limit`
- `Conc<B>`, `ChemBasisTag`, `StoichLaw<T>`, and the basis types `AsN`, `AsNO3`, `CnWad`, `CnFree`, `CnTotal`,
  `AsCaCO3`, `Dissolved`, `TotalRecoverable`
- `HazardousByproduct`, `StreamMassBalance`
- `ImportedValue`, `UNREGISTERED_UNIT` (BD-027): an undeclared unit is stored under that tag, stays viewable with the
  text the source declared, and refuses to combine (`FMINE-REF-107`). Two registered units refuse with `FMINE-REF-105`,
  and so does a currency mismatch.

## 5. Guards & Typed Refusals
`UnitsError::refusal_code` maps these variants. The crate sits below `fmine-evidence`, so each code is a string
that `xtask check-refusals` requires to be a registry code:
- `FMINE-REF-109` (outside physical domain, proposed): `MoistureOutOfBounds` (outside `[0.0, 1.0)`),
  `FractionOutOfBounds` (outside `[0.0, 1.0]`) and `ConcentrationOutOfDomain` (a `Conc<B>` that is negative or not
  finite, including a conversion that would overflow). The value is refused, never clamped.
- `FMINE-REF-105` (basis mismatch): `ChemicalBasisMismatch` (comparing concentrations on different species bases
  without the stoichiometric conversion, `G-ENV-1`, or typing a `ChemConc` as another basis), `CurrencyMismatch`
  and `UnitMismatch`.
- `FMINE-REF-107` (undeclared unit): `UnregisteredUnit`.

`SimplexSumInvalid`, `ZeroMassWeight` and `Overflow` are typed errors with no refusal code.

Compile-fail doctests (`compile_fail` with the error code, which the pinned nightly checks): Wet + Dry and Contained +
Recoverable `Qty` (E0308), `Grade + Grade` (E0369), `metal_grams` on wet tonnes (E0308), `Money + Money` (E0369), a
`Conc<AsN>` sample against a `Conc<AsNO3>` limit (E0308), and a conversion with no registered law (E0277).

Known gaps: dimension, moisture-basis and FX-missing refusals are compile-time markers or typed errors here, not
§8.4 codes. `G-BYPROD-1` is a type (`HazardousByproduct` converts only to a storage liability), not a refusal.

## 6. Proof Obligations (`PO-*`)
- `PO-UNIT-1`: $10^4$ randomized `ExactQty` round-trips (`g <-> oz_t`, `g/t <-> oz/st`) have zero residual.
- `PO-UNIT-2`: `79 mg/L as N = 349.72 mg/L as NO3 > 50 mg/L` WHO threshold; comparing without the conversion is
  refused with `FMINE-REF-105` (`test_nitrate_basis_conversion`). The typed form gives the same answer, and the
  same bits as the run-time form over 10,000 seeded values both ways
  (`test_typed_nitrate_matches_the_untyped_law_and_refuses_another_basis`).

## 7. Failure Modes & Deterministic Fallbacks
Invalid moisture fractions (`>= 1.0` or `< 0.0`) or mismatched currencies fail closed with typed `Refusal`.

## 8. Gates & Kill Criteria (`GATE-*` / `KILL-*`)
- `GATE-UNIT-01`
- `GATE-LAYERS-00` (`L-6` no bare `f64` quantities in K/E public signatures)

## 9. Lifted Code & Foundation Dependencies
- Depends on `fmine-core`.

## 10. Non-Goals & Scope Boundary
Does not implement `std::ops::Add` on `Grade<R>` (`INV-014`).
