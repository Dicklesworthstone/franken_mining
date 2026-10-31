# CONTRACT.md — `fmine-metrics`

## 1. Charter & Layer
- **Crate:** `fmine-metrics`
- **Layer:** `E` (Enterprise & Decision)
- **Subsystem:** `Enterprise`
- **LOC Budget:** `16k planned / 18k ceiling` (`registries/loc_budget.toml`)
- **Allowed Module Families:** `["lmdi", "cutoff"]`

## 2. Mathematical & Domain Formulation
Implements `SiteAiscRow` and `AiscConsolidationBridge` (`L_corp`, `L_JV`, `G-BULL-3`), versioned `PriceDeck` (Barrick reserve decks 2023–2025 as printed in the 2026 Circular, E606; no silver price is stated, so none is carried) and `compute_geo` (`GB-27`), `ReserveReplacementBridge` on one stated basis (the Circular's 190% and 504% differ by metal basis, E603–E604), exact 5-factor Logarithmic Mean Divisia Index (`lmdi`, `GB-26`), and fixed-point marginal cut-off grade solver (`solve_cutoff_grade`; synthetic known answer only, the published cut-offs E182–E184 are not yet reproduced).

## 3. Authority, Epistemics & Representation
Pure `E`-layer metric kernel (`L-3`): enforces `G-BULL-3` (`FMINE-REF-203` when `L_corp` is omitted) and zero-residual multiplicative-to-additive LMDI attribution (`GB-26`).

## 4. Public API Surface
- `SiteAiscRow`, `AiscConsolidationBridge`
- `PriceDeck` (`barrick_reserve(year)`), `GeoBreakdown`, `compute_geo`
- `ReserveReplacementBridge`
- `solve_cutoff_grade` → `Result<CutoffSolution, CutoffError>`
- `cutoff::{parse_cases, reproduce, CutoffCase, CutoffReport, CutoffState, Printed, Recovery}` (BD-274): the
  published cut-offs in `registries/published_cutoffs.toml`, recomputed on exact rationals from the printed inputs.
  Each comes back `Reproduced`, `WithinInputRounding` or `Conflict`; `CutoffReport::to_json` prints exact results to
  six places, never through a float.
- `lmdi::{LmdiState, LmdiWaterfall, decompose_aisc_lmdi, logarithmic_mean}`
- `lmdi::{golden_digest_logarithmic_mean_10k, golden_digest_decompose_aisc_lmdi_10k}` (bd-hp2j.3): BLAKE3 over the
  output bits on 10,000 inputs from `LMDI_DIGEST_KEY` (series branch, both branches, ratios to e^300, magnitudes
  near the subnormals; seeded positive state pairs). The committed values were first observed on aarch64-macos;
  `lmdi.rs` declares `BitwisePortable` only after ubuntu-latest and macos-15 agree in one CI run.

## 5. Guards & Typed Refusals
- `FMINE-REF-203` (`ConsolidationBridgeMissing`, `G-BULL-3`)
- `FMINE-REF-109` (outside physical domain, proposed; exit 3) on non-positive or non-finite LMDI state inputs.
- `CutoffError` (non-positive cost, price or guess; recovery outside (0, 1]; no convergence in 100 iterations): no input is clamped (bd-8wrk).
- `cutoff::CutoffFailure`: a malformed registry row, a non-positive net value or recovery, or a float solver that
  disagrees with the exact fixed point. A printed figure beyond its inputs' rounding is a result (`Conflict`), not a
  failure.

## 6. Proof Obligations (`PO-*`)
- `PO-MET-1`: `LmdiWaterfall::sum_of_factors() == total_delta` to `< 1e-10` (synthetic two-period case, `+$1,285/oz`; its numbers were once mislabelled as Barrick's, R7-C054).
- `PO-MET-3`: `ReserveReplacementBridge::compute` returns `None` for non-positive or non-finite depletion; it never clamps the denominator.
- `PO-MET-2`: `AiscConsolidationBridge::verify_rollup(false)` refuses with `FMINE-REF-203` when `L_corp != 0`.
- `PO-MET-4` (bd-hp2j.2): `logarithmic_mean` takes its logarithm from `fmine_core::det`, never the platform libm. On
  10,000 Philox-seeded pairs each, it lies in [min, max], `L(a, b)` and `L(b, a)` are the same bits, and its series and
  logarithm branches agree to < 1e-12 relative at the switch (1.1e-14 measured; the old 1e-7 switch on `ln a − ln b`
  jumped 1.1e-8).

## 7. Failure Modes & Deterministic Fallbacks
Non-positive physical inputs or missing consolidation adjustments fail closed with `Refusal`.

## 8. Gates & Kill Criteria (`GATE-*` / `KILL-*`)
- `GATE-BULL-01`

## 9. Lifted Code & Foundation Dependencies
- Depends on `fmine-core`, `fmine-units`, `fmine-time`, `fmine-evidence`, `fmine-claim`, `fmine-numerics`.

## 10. Non-Goals & Scope Boundary
Pure metric kernel in layer `E`; workbook parsing lives in `fmine-io` / `fmine-public` (`V`).
