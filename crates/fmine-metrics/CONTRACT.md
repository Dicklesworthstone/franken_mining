# CONTRACT.md — `fmine-metrics`

## 1. Charter & Layer
- **Crate:** `fmine-metrics`
- **Layer:** `E` (Enterprise & Decision)
- **Subsystem:** `Enterprise`
- **LOC Budget:** `16k planned / 18k ceiling` (`registries/loc_budget.toml`)
- **Allowed Module Families:** `["lmdi"]`

## 2. Mathematical & Domain Formulation
Implements `SiteAiscRow` and `AiscConsolidationBridge` (`L_corp`, `L_JV`, `G-BULL-3`), versioned `PriceDeck` (Barrick reserve decks 2023–2025 as printed in the 2026 Circular, E606; no silver price is stated, so none is carried) and `compute_geo` (`GB-27`), `ReserveReplacementBridge` on one stated basis (the Circular's 190% and 504% differ by metal basis, E603–E604), exact 5-factor Logarithmic Mean Divisia Index (`lmdi`, `GB-26`), and fixed-point marginal cut-off grade solver (`solve_cutoff_grade`, `E182–E184`).

## 3. Authority, Epistemics & Representation
Pure `E`-layer metric kernel (`L-3`): enforces `G-BULL-3` (`FMINE-REF-203` when `L_corp` is omitted) and zero-residual multiplicative-to-additive LMDI attribution (`GB-26`).

## 4. Public API Surface
- `SiteAiscRow`, `AiscConsolidationBridge`
- `PriceDeck` (`barrick_reserve(year)`), `GeoBreakdown`, `compute_geo`
- `ReserveReplacementBridge`
- `solve_cutoff_grade`
- `lmdi::{LmdiState, LmdiWaterfall, decompose_aisc_lmdi, logarithmic_mean}`

## 5. Guards & Typed Refusals
- `FMINE-REF-203` (`ConsolidationBridgeMissing`, `G-BULL-3`)
- `FMINE-REF-301` on non-positive LMDI state inputs.

## 6. Proof Obligations (`PO-*`)
- `PO-MET-1`: `LmdiWaterfall::sum_of_factors() == total_delta` to `< 1e-10` (synthetic two-period case, `+$1,285/oz`; its numbers were once mislabelled as Barrick's, R7-C054).
- `PO-MET-3`: `ReserveReplacementBridge::compute` returns `None` for non-positive or non-finite depletion; it never clamps the denominator.
- `PO-MET-2`: `AiscConsolidationBridge::verify_rollup(false)` refuses with `FMINE-REF-203` when `L_corp != 0`.

## 7. Failure Modes & Deterministic Fallbacks
Non-positive physical inputs or missing consolidation adjustments fail closed with `Refusal`.

## 8. Gates & Kill Criteria (`GATE-*` / `KILL-*`)
- `GATE-BULL-01`

## 9. Lifted Code & Foundation Dependencies
- Depends on `fmine-core`, `fmine-units`, `fmine-time`, `fmine-evidence`, `fmine-claim`, `fmine-numerics`.

## 10. Non-Goals & Scope Boundary
Pure metric kernel in layer `E`; workbook parsing lives in `fmine-io` / `fmine-public` (`V`).
