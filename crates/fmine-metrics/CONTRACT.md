# CONTRACT.md — `fmine-metrics`

## 1. Charter & Layer
- **Crate:** `fmine-metrics`
- **Layer:** `E` (Enterprise & Decision)
- **Subsystem:** `Enterprise`
- **LOC Budget:** `16k planned / 18k ceiling` (`registries/loc_budget.toml`)
- **Allowed Module Families:** `["lmdi", "cutoff", "definitions"]`

## 2. Mathematical & Domain Formulation
Implements `SiteAiscRow` and `AiscConsolidationBridge` (`L_corp`, `L_JV`, `G-BULL-3`), versioned `PriceDeck` (Barrick reserve decks 2023–2025 as printed in the 2026 Circular, E606; no silver price is stated, so none is carried) and `compute_geo` (`GB-27`), `ReserveReplacementBridge` on one stated basis (the Circular's 190% and 504% differ by metal basis, E603–E604), a spend-based Logarithmic Mean Divisia Index (`lmdi`, `GB-26`, BD-333.S1), and fixed-point marginal cut-off grade solver (`solve_cutoff_grade`; synthetic known answer only, the published cut-offs E182–E184 are not yet reproduced).

The LMDI bridge recomputes each endpoint as `A = supplied total_aisc_spend / oz_sold`. Its six bars are
`L(A1,A0)·Δln(C)` and `−L(A1,A0)·Δln(X)` for `X` in tonnes, grade, recovery, metal balance
`B = produced/(tonnes·grade·recovery)` and sales timing `S = sold/produced`. The printed rates never set the
weight or a bar. Each printed-minus-recomputed endpoint gap is retained, including when the two gaps are
equal, and `printed_rate_gap = (P1−P0)−(A1−A0)` is a separate reconciliation item. The measured floating-point
closure residual is retained rather than forced to zero or allocated to a factor.

## 3. Authority, Epistemics & Representation
Pure `E`-layer metric kernel (`L-3`): enforces `G-BULL-3` (`FMINE-REF-203` when `L_corp` is omitted) and checks
the multiplicative-to-additive LMDI accounting identity (`GB-26`). Its public-data interpretation is descriptive;
arithmetic closure does not establish causation or identify an operational explanation (plan §21.6, E699).

## 4. Public API Surface
- `SiteAiscRow`, `AiscConsolidationBridge`
- `PriceDeck` (`barrick_reserve(year)`), `GeoBreakdown`, `compute_geo`
- `ReserveReplacementBridge`
- `solve_cutoff_grade` → `Result<CutoffSolution, CutoffError>`
- `definitions::{all_in_cost_per_oz, is_non_sustaining, bundled_registry, AllInCost, CostLine, MetricFailure, Project}`
  (BD-060): AISC and AIC from `registries/metrics.toml`. A line the definition neither counts nor excludes is refused,
  and every result names its definition and any successor.
- `cutoff::{parse_cases, reproduce, CutoffCase, CutoffReport, CutoffState, Printed, Recovery}` (BD-274): the
  published cut-offs in `registries/published_cutoffs.toml`, recomputed on exact rationals from the printed inputs.
  Each comes back `Reproduced`, `WithinInputRounding` or `Conflict`; `CutoffReport::to_json` prints exact results to
  six places, never through a float.
- `cutoff::{section, CutoffSection, CUTOFFS_REGISTRY}`: every row reproduced, as the JSON object an issuer run of
  `fmine assure public` carries as `published_cutoffs`: the registry's sha256, the count in each state and the rows in
  registry order, with the registry's `issuer`. A row that cannot be computed fails the whole section.
- `lmdi::{LmdiState, LmdiWaterfall, decompose_aisc_lmdi, logarithmic_mean}`
  - `LmdiState` keeps the existing floating-point API. Monetary spend and sold ounces must have compatible
    scales so their quotient is USD/oz: USD/oz or thousand USD/koz. `$M`/`koz` inputs require the explicit
    factor 1,000 on spend before this call. Each variable's unit and ownership basis must agree across periods;
    this legacy input does not encode or infer them.
  - `LmdiWaterfall::{aisc_0, aisc_1, total_delta}` now describe the recomputed rates. `printed_aisc_0/1` retain
    the observed endpoints; `printed_rate_gap_0/1` retain each discrepancy; `printed_rate_gap` reconciles the
    changes. `closure_residual = total_delta − sum_of_factors()` is measured and checked to at most 1e-9
    USD/oz. `sum_of_factors()` includes only the six factor bars.
  - Physical product/ratio changes are evaluated in log space. A temporal quotient that underflows,
    overflows or is subnormal uses the difference of portable logarithms; a rounded subnormal quotient can
    otherwise invent a material closure discrepancy. No input is clamped.
- `lmdi::{golden_digest_logarithmic_mean_10k, golden_digest_decompose_aisc_lmdi_10k}` (bd-hp2j.3): BLAKE3 over the
  output bits on 10,000 inputs from `LMDI_DIGEST_KEY` (series branch, both branches, ratios to e^300, magnitudes
  near the subnormals; seeded positive state pairs). The v2 decomposition draws independently supplied spend
  and binds all recomputed/printed endpoints, gaps, measured residual and six effects. The corrected
  logarithmic-mean branch is relative at every positive scale, so its tiny-wide-ratio digest changes too.
  These intentional mathematical/semantic changes invalidate the v1 snapshots; independent high-precision
  known answers establish correctness, while the v2 bit snapshots remain regression and host-agreement
  checks. `lmdi.rs` declares `BitwisePortable` only after ubuntu-latest and macos-15 agree in one CI run.

## 5. Guards & Typed Refusals
- `FMINE-REF-203` (`ConsolidationBridgeMissing`, `G-BULL-3`)
- `FMINE-REF-109` (outside physical domain, proposed; exit 3) on non-positive or non-finite LMDI state inputs,
  or recovery above one, naming the period and field.
- `FMINE-REF-207` (numerical certificate failed, proposed; exit 3) on an unrepresentable recomputed rate,
  a non-finite effect or reconciliation, or a measured closure residual above 1e-9 USD/oz. It supplies no
  forced closure. The bare logarithmic-mean helper returns NaN outside finite positive arguments; the
  decomposition checks its domain and result before returning a waterfall.
- `CutoffError` (non-positive cost, price or guess; recovery outside (0, 1]; no convergence in 100 iterations): no input is clamped (bd-8wrk).
- `cutoff::CutoffFailure`: a malformed registry row, a non-positive net value or recovery, or a float solver that
  disagrees with the exact fixed point. A printed figure beyond its inputs' rounding is a result (`Conflict`), not a
  failure.

## 6. Proof Obligations (`PO-*`)
- `PO-MET-1`: `LmdiWaterfall::sum_of_factors() == total_delta` to `< 1e-10` in the synthetic two-period case
  (`+$1,285/oz`; its numbers were once mislabelled as Barrick's, R7-C054). Its independently calculated sales
  timing bar is −536.9148672729303 USD/oz, and its printed gaps are zero because the supplied cost/quantity
  endpoints reconcile. Every accepted waterfall exposes its actual closure residual, at most 1e-9 USD/oz.
- `PO-MET-3`: `ReserveReplacementBridge::compute` returns `None` for non-positive or non-finite depletion; it never clamps the denominator.
- `PO-MET-2`: `AiscConsolidationBridge::verify_rollup(false)` refuses with `FMINE-REF-203` when `L_corp != 0`.
- `PO-MET-4` (bd-hp2j.2): `logarithmic_mean` takes its logarithm from `fmine_core::det`, never the platform libm. On
  10,000 Philox-seeded pairs each, it lies in [min, max], `L(a, b)` and `L(b, a)` are the same bits, and its series and
  logarithm branches agree to < 1e-12 relative at the switch (1.1e-14 measured; the old 1e-7 switch on `ln a − ln b`
  jumped 1.1e-8).
  - A relative near-equality test replaces the incorrect `mean.max(1e-15)` branch selector. Independent
    80-digit Decimal answers cover `L(1e-18,1e-21) = 1.4462006247378286e-19`, the same ratio at 1e-300,
    a subnormal result, and near-maximum positive endpoints. Scale invariance holds across 1e-300 to 1e300;
    equal smallest-subnormal and largest-finite inputs return themselves. An overflowing `a+b` uses a safe
    half-sum while ordinary finite-sum arithmetic retains its previous evaluation order.
- `PO-MET-5` (BD-333.S1 correctness slice, `lmdi::tests`):
  - Hand identities isolate a cost-only +100 USD/oz change and a throughput-only −50 change. Altering only
    printed rates changes no factor bits; changing supplied spend with unchanged printed rates changes the
    cost bar. Two nonzero endpoint gaps survive even when the gap change is zero.
  - The North Mara rate-reconciliation inputs quoted in BD-333.S1 (64 $M/50 koz/1,292 and
    93 $M/37 koz/2,577) give recomputed delta `45640/37` and printed gap `1905/37`, within 1e-12 in this
    floating-point API; bars plus gap give 1,285. That test uses explicitly invented physical factors and
    establishes only endpoint reconciliation, not a real-site operational attribution.
  - A grade-only change reallocates between grade and B without changing total cost per ounce. All seven
    input fields in both periods refuse zero, negatives, NaN and infinities by name; recovery above one,
    rate underflow/overflow and non-finite factor effects refuse. Log-space tests cover overflowing physical
    products and temporal subnormal ratios whose rounded logarithms would create a false residual.

## 7. Failure Modes & Deterministic Fallbacks
Non-positive physical inputs or missing consolidation adjustments fail closed with `Refusal`.
LMDI numerical failures also refuse; the printed-rate discrepancy is never used as a balancing factor.

## 8. Gates & Kill Criteria (`GATE-*` / `KILL-*`)
- `GATE-BULL-01`

## 9. Lifted Code & Foundation Dependencies
- Depends on `fmine-core`, `fmine-units`, `fmine-time`, `fmine-evidence`, `fmine-claim`, `fmine-numerics`.

## 10. Non-Goals & Scope Boundary
Pure metric kernel in layer `E`; workbook parsing lives in `fmine-io` / `fmine-public` (`V`).
BD-333.S1 remains partial: the exact-rational `PrintedQty`/basis API, rounding enclosures and sign flags,
signed additive fallback, and public pairing/citations are not supplied by this legacy floating-point fix.
