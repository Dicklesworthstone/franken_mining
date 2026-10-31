# CONTRACT.md — `fmine-numerics`

## 1. Charter & Layer
- **Crate:** `fmine-numerics`
- **Layer:** `K0` (Kernel Foundation)
- **Subsystem:** `Kernels`
- **LOC Budget:** `16k planned / 18k ceiling` (`registries/loc_budget.toml`)
- **Allowed Module Families:** `["eproc", "voi", "risk", "tropical", "exact_feas"]`

## 2. Mathematical & Domain Formulation
Implements Cholesky SPD factorization (`cholesky_solve`), Brent root-finding (`brent_root`), normal CDF/PPF (`norm_cdf`, `norm_inv`), anytime-valid betting e-processes (`BettingEProcess`), Value of Information (`VoiProblem`, `EVPI`, `EVSI`), coherent Rockafellar-Uryasev `CVaR` (`compute_cvar`), and `(max,+)`/`(min,+)` tropical critical-path algebra (`TropicalDag`, `GB-19`).

## 3. Authority, Epistemics & Representation
Pure mathematical kernels (`K0`) consumed by `K1..K4`, `E`, and `V`. Determinism class: `DeterministicPerIsa`, not yet `BitwisePortable`: `norm_cdf` and `norm_inv` delegate to `fmine_core::det` (bd-hp2j.2), and the class changes only with a cross-host golden digest (bd-hp2j.3).

## 4. Public API Surface
- `cholesky_solve`, `norm_cdf`, `norm_inv`. `norm_inv` clamps nothing: `p` outside [0, 1] or NaN gives NaN, and 0
  and 1 give −∞ and +∞; callers refuse those (bd-hp2j.2 removed a silent clamp to [1e-15, 1 − 1e-15]).
- `golden_digest_norm_cdf_inv_10k`, `golden_digest_cholesky_brent_10k` (bd-hp2j.3): BLAKE3 over the output bits on
  10,000 `NUMERICS_DIGEST_KEY` inputs (|z| up to 12, p from 1e-304 to 1 − 1e-16; seeded 3×3 SPD systems and cube
  roots). `cholesky_solve` and `brent_root` get their own digest so that one class can cover `lib.rs`. The values
  were first observed on aarch64-macos; the class flips only after ubuntu-latest and macos-15 agree in one CI run.
- `brent_root` → `Result<Root, RootError>`: `NoSignChange`, `NonFinite` (endpoint or iterate) and `NotConverged` refuse; an unconverged iterate is never returned (bd-t1dk)
- `eproc::{BettingEProcess, EProcStatus}`
- `voi::{VoiProblem, VoiSummary}`
- `risk::{TailRiskSummary, compute_cvar, max_drawdown}`
- `tropical::{TropicalEdge, TropicalDag, TropicalScheduleResult}`
- `exact_feas` (bd-4wvl.1 BD-340.S1, bd-4wvl.2 BD-340.S2): exact rounding-interval feasibility over
  `Rat = fmine_units::ExactQty`.
  - `Cell`, `Convention::{Closed, HalfUp, HalfDown, HalfEven, Truncate}` and `Convention::interval`; `End`
    and `Bound` (ranges with open or infinite ends). A zero step marks an exact value, a closed point under
    every convention (BD-331.S6: the 0 a placeholder stands for).
  - `Identity { id, family, closure, kind: Sum | Equal | Scale | Ratio | WeightedMean }` and
    `Identity::row` (linear kinds). `Ratio { rate, num, den, factor }` is linearised when its rate cell is in
    no other identity (otherwise `Indeterminate`, "bilinear") and its denominator keeps one sign.
  - `WeightedMean { rate, parts }` (bd-4wvl.6, BD-340.S6), `rate·Σwᵢ = Σ rateᵢ·wᵢ`, is relaxed: `W = Σwᵢ`
    and the four McCormick rows per product over its factors' closed hulls (every factor needs a finite range,
    else `Indeterminate`). An infeasible relaxation is `Infeasible` with a checked Farkas certificate over the
    envelope rows; a feasible one is `NotRefuted`, never `Feasible`; neither `implied_interval` nor `blame`
    reads one. `check_witness` checks the bilinear identity exactly. No part or a repeated cell is refused.
  - `region_against_members(id, family, closure, factor, region, members)` with `RateCells { rate, weight,
    numerator }`: the weights' `Sum`; then, when the region and every member print a numerator, the linear
    form (the numerators' `Sum` and one unshared `Ratio` per rate), else one `WeightedMean`.
  - `Rules { convention, constructions }`. `Construction::{ComponentSum, RoundedOnce}` is declared per
    family before a run; a `Sum` whose family has none is refused. `ComponentSum` requires the printed
    footing (else `Certificate::Residual`) and drops the total's own box.
  - `solve` and `solve_with_caps` give one `Component` per connected group of cells, with
    `Outcome::{Feasible { witness }, Infeasible { certificate }, NotRefuted, Indeterminate { reason }}`.
    `Certificate` is `Farkas { multipliers }` by row id, or `Residual { identity, residual }`.
    `NotRefuted` is a feasible relaxation (S6).
  - `check_witness` (identities checked exactly, ratios as ratios) and `check_certificate` (residuals
    recomputed; Farkas ranges over the linearised rows) are independent of the simplex. Every reported
    outcome passed one; a rejected one is `Indeterminate`. The certificate checker shares the linearisation:
    a linearisation fault is caught by the `aisc.rs` oracle tests, not by the checker.
  - `implied_interval(cells, identities, rules, var, free_own_box)`: the exact interval of a cell, with
    open ends, each end re-decided by checked runs; `ImpliedError::{Refused, Infeasible, Unbounded,
    Indeterminate}`. A rate gets its ratio's exact range (Charnes–Cooper over the closure) cut by its own
    box; a denominator without a finite range of one sign gives `Indeterminate`.
  - `blame` and `blame_with_caps` (bd-4wvl.3, BD-340.S3), for each component `solve` finds infeasible:
    - `ConflictSet`s: inclusion-minimal infeasible sets of printed cells, each with a certificate that
      `check_conflict_set` re-checks;
    - `CorrectionSet`s: cells whose printed values, disregarded together, restore feasibility, each with its
      implied range (never a corrected value);
    - the label is `Minimum` only after an exhaustive search over at most `MAX_EXHAUSTIVE_CELLS` (20) cells
      within `MAX_BLAME_RUNS`; otherwise `InclusionMinimalOnly`;
    - disregarding a cell drops its box and any component-sum footing over it.
  - `presence_conflicts`: an `Equal` identity between a number and a `Placeholder` is a conflict located on
    the number. Other cells printing the same value are `Signature`s labelled `Hypothesis`.
  - `certificate_digest`: hex BLAKE3 of an `XFCT` v2 frame (v2 binds the constructions), independent of
    input order; a weighted mean is kind 4 (no earlier digest moves).
  - Input is refused with `XfRefusal`:
    - a negative step, or an off-grid half-even value;
    - a duplicate cell or identity, or an unknown cell or variable;
    - bad signs, a degenerate row, or a ratio row requested;
    - an undeclared construction, or a denominator interval that contains zero;
    - a placeholder in an identity other than `Equal` (`presence_conflicts`).
  - Caps: 256 cells per component (`MAX_VARIABLES`) and 10⁴ pivots per run (`MAX_PIVOTS`). A cap or an
    overflow gives `Indeterminate`.
- `exact_feas::discover` (bd-4wvl.4, BD-340.S4 part 1; `DeterministicPerIsa`, the rest of `exact_feas` is
  `Exact`): which construction one family's decided footings fit. A report; it decides no state.
  - `p0(k)`: P(|Σeᵢ| < ½) for k i.i.d. U(−½, ½) errors, an exact `Rat` from the Irwin–Hall polynomial;
    `None` past `MAX_K` (20).
  - `discover_construction(&[Footing { k, residual }]) -> Result<ConstructionReport, UnitsError>`: the counts
    each construction accepts, Σp₀ exactly, and the log tails, in `f64` through `fmine_core::det`. These are
    the exact count's two tails under `RoundedOnce` (Poisson-binomial) and each construction's rejections
    under Binomial(n, `MAX_ERROR_RATE` = 1/10).
  - `Discovered::{ComponentSum, RoundedOnce, Ambiguous}`: a construction is ruled out when a tail is below
    `ALPHA` (10⁻⁶); exactly one left is discovered; both or neither is `Ambiguous`, which carries
    `NON_IDENTIFIABLE` (FMINE-REF-104). Past `MAX_K` a footing is `indeterminate` and not scored.
  - The tails take footings as independent: the caller passes each distinct footing once.
  - `discover_tie_conventions(&[Printed { document, quantity, printed, step }]) -> Result<TieDiscovery,
    TieRefusal>` (part 2): one convention per document, never per cell. Every assignment of
    `STRICT_CONVENTIONS` (half-up, half-down, half-even, truncate) to 2 to `MAX_DOCUMENTS` (4) documents gets
    each quantity's meet of intervals and a `Fit::{Empty, Points, Proper}`; `closed` is every document
    `Closed`. `TieFinding::{Identified, Ambiguous (FMINE-REF-104), NoProperFit}` per document is read from the
    proper fits only. `conflicts` names each quantity whose meet is empty under `closed` and under every
    assignment (bd-plx1): no convention reconciles its prints, and it makes every assignment `Empty`.
    `Closed` alone does not decide, since a truncation interval [v, v + 2h) can reach past it when the steps
    differ (bd-4wvl.7). Refused: a step of zero or below, a quantity printed twice by one document, a document count
    outside 2 to 4, a value off its step's grid (half-even), an overflow.
  - `share_interval(c, numerators, denominators, convention) -> Result<Bound, ShareRefusal>` (part 3): the
    exact range of s = c·Π numerators / Π denominators over each `Factor { printed, step }`'s rounding box
    (a zero step is exact), with each end open when a factor end it comes from is open. Every factor must be a
    different cell with a wholly positive range; `ShareRefusal::{NotPositive, NegativeStep, Interval}`.
  - `discover_unit(pairs, candidates, convention) -> Result<UnitDiscovery, ShareRefusal>` (part 5): a
    candidate `(name, factor)` fits a `UnitPair { observed, reference }` when factor × the observed range meets
    the reference range. `UnitFinding::{Identified, Ambiguous (FMINE-REF-104), NoFit}` counts only candidates
    that fit every pair.
  - `discover_deck(&[GeoRelation { gold, copper, geo }], &[Deck { name, gold, copper }]) -> Result<DeckDiscovery,
    ShareRefusal>` (part 4): geo = gold + copper · (copper price / gold price), gold and GEO in Moz, copper in
    Mlb. Each `STRICT_CONVENTIONS` member, one for all of a row's relations, against each deck gives a `DeckFit`
    (the worst `Fit` over the relations). A one-point meet is `Points` unless exact inputs make the converted
    sum that point. `DeckFinding::{Found, Ambiguous (FMINE-REF-104), NoFit}` reads the proper fits only, and
    `conventions` lists the conventions with one. Found is not registered: the row's stated deck is the
    caller's. Refused: a price of zero or below (`NotPositive`), a negative step, an overflow.
- `exact_feas::generator` (bd-4wvl.7, BD-340.S7; `Exact`): clean random tables for the soundness, metamorphic
  and power tests. (`gen` is a reserved word in edition 2024, so the module is `generator`.)
  - `generate(key, &GenSpec) -> Result<Generated, GenError>` and `generate_vintage(key, spec, vintage)`: one
    `Block::{Sum (depth ≤ 3, negative lines), HalfYear, Breakdowns (COD-02's shape), Share (SHARES), Ratio,
    Reprint, WeightedMean}` per table, exact true values (ties by chance, or every value on a half step in a
    tie table), printed under one drawn construction and one drawn rule. The shape comes from the key's Philox
    stream from counter 0, the values from counters set by the vintage, so vintages share the shape.
    `Breakdowns` is always `RoundedOnce`. S5's roll-forward has no block yet.
  - `round(x, step, rule)`: half-up, half-down, half-even or truncate; `Closed` is refused (`NotARoundingRule`).
  - `Generated::{rules, bracket, witness}`: `bracket` is `Closed` for a round-to-nearest table and `Truncate`
    for a truncated one, since `Closed` (v ± h) does not contain a truncation interval [v, v + 2h).
- `exact_feas::power` (bd-4wvl.7, BD-340.S7; `DeterministicPerIsa`, the Clopper–Pearson bounds are `f64`
  through `fmine_core::det`):
  - `Injection::{SignFlip, Transposition, Neighbour, WrongShare, StaleVintage, Units}`; `inject` and
    `inject_at` change one printed value and give its error, slack (Σ half-steps over the smallest identity
    naming the cell) and bin in `BINS` ([0, ½), [½, 1), [1, 2), [2, 4), ≥ 4; DESIGN).
  - `joint_detects` (some component `Infeasible`), `baseline_detects` (some identity alone over its own cells,
    through the same kernel), `joint_only`, `blame_accuracy` (hit and exact) and `size1_blame`.
  - `clopper_pearson(k, n, alpha) -> Option<(f64, f64)>` by bisection on the exact binomial tails; `None` for
    `k > n` or `alpha` outside (0, 1).
  - `power_study(&PowerSpec, Detector) -> Result<PowerTable, PowerError>`: per kind and bin the counts with
    their `n`; a clean table the joint solver finds infeasible stops it (`CleanConflict`). Tables are decided
    under their bracket with their own construction. The baseline's alarms on clean tables are counted apart.
  - `KillReport { joint_only_detections, size1, discoveries, implied_consumers }`, plan K.5 row 3: `fired()`
    is computed (all four empty; a `None` entry counts as empty), never stored.

## 5. Guards & Typed Refusals
- `FMINE-REF-207` (numerical certificate failed, proposed; exit 3) from Cholesky when the matrix is not square or
  a diagonal pivot is `<= 1e-12` (`test_cholesky_non_spd_refuses_207`).
- `TropicalDag::earliest_starts` returns `None` on a positive precedence cycle; it is not a refusal code.
- `NON_IDENTIFIABLE` (`FMINE-REF-104`, non-identifiable attribution) marks an ambiguous construction finding.

## 6. Proof Obligations (`PO-*`)
- `PO-NUM-1`: `0 <= EVSI <= EVPI` across all prior covariance and action sets (`GATE-HELM-02`).
- `PO-NUM-3`: Ville's inequality $\mathbb{P}_{H_0}(\sup_t E_t \ge 1/\alpha) \le \alpha$ holds empirically and `CVaR` satisfies coherent subadditivity $\text{CVaR}_\alpha(X+Y) \le \text{CVaR}_\alpha(X) + \text{CVaR}_\alpha(Y)$.
- `PO-NUM-4`: Tropical `(max,+)` and `(min,+)` critical paths satisfy zero-slack duality.

## 7. Failure Modes & Deterministic Fallbacks
Indefinite matrices fail closed with `FMINE-REF-207`; cyclic precedence graphs give `None`, never a schedule.

## 8. Gates & Kill Criteria (`GATE-*` / `KILL-*`)
- `GATE-HELM-02`
- `GATE-FSIM-DRIFT`

## 9. Lifted Code & Foundation Dependencies
No lifted code. `src/eproc.rs`, `src/voi.rs`, `src/risk.rs` and `src/tropical.rs` are original, listed as
`[[analogue]]` rows in `registries/lifts.toml` so GATE-FSIM-DRIFT still watches their upstream counterparts. A real
lift of any of them is a per-module decision under BD-022.

## 10. Non-Goals & Scope Boundary
Zero I/O (`std::fs`, `std::net`) or domain-specific accounting rules (`L-3`).
