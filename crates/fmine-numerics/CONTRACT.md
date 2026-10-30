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
Pure mathematical kernels (`K0`) consumed by `K1..K4`, `E`, and `V`. Determinism class: `DeterministicPerIsa`, not `BitwisePortable`: `norm_cdf` and `norm_inv` call std `exp`, `ln` and `sqrt` (porting them is NEW-det-portable-kernels, bd-hp2j).

## 4. Public API Surface
- `cholesky_solve`, `norm_cdf`, `norm_inv`
- `brent_root` → `Result<Root, RootError>`: `NoSignChange`, `NonFinite` (endpoint or iterate) and `NotConverged` refuse; an unconverged iterate is never returned (bd-t1dk)
- `eproc::{BettingEProcess, EProcStatus}`
- `voi::{VoiProblem, VoiSummary}`
- `risk::{TailRiskSummary, compute_cvar, max_drawdown}`
- `tropical::{TropicalEdge, TropicalDag, TropicalScheduleResult}`
- `exact_feas` (bd-4wvl.1, BD-340.S1): exact rounding-interval feasibility over `Rat = fmine_units::ExactQty`.
  - `Cell`, `Convention::{Closed, HalfUp, HalfDown, HalfEven, Truncate}` and `Convention::interval`;
    `Identity { id, family, closure, kind: Sum | Equal | Scale }` and `Identity::row`.
  - `solve` and `solve_with_caps` give one `Component` per connected group of cells, with
    `Outcome::{Feasible { witness }, Infeasible { certificate }, NotRefuted, Indeterminate { reason }}`.
    `NotRefuted` is reserved for relaxations (S6).
  - `check_witness` and `check_certificate` are independent of the simplex. Every reported outcome passed
    one; a rejected one is `Indeterminate`.
  - `certificate_digest`: hex BLAKE3 of an `XFCT` v1 frame, independent of input order.
  - Input is refused with `XfRefusal`: a non-positive step, an off-grid half-even value, a duplicate cell
    or identity, an unknown cell, bad signs, or a degenerate row.
  - Caps: 256 cells per component (`MAX_VARIABLES`) and 10⁴ pivots (`MAX_PIVOTS`). A cap or an overflow
    gives `Indeterminate`.

## 5. Guards & Typed Refusals
- `FMINE-REF-301` (`NonSpdCovarianceMatrix`) when Cholesky diagonal pivot `<= 1e-12` or cycle detected in `TropicalDag`.

## 6. Proof Obligations (`PO-*`)
- `PO-NUM-1`: `0 <= EVSI <= EVPI` across all prior covariance and action sets (`GATE-HELM-02`).
- `PO-NUM-3`: Ville's inequality $\mathbb{P}_{H_0}(\sup_t E_t \ge 1/\alpha) \le \alpha$ holds empirically and `CVaR` satisfies coherent subadditivity $\text{CVaR}_\alpha(X+Y) \le \text{CVaR}_\alpha(X) + \text{CVaR}_\alpha(Y)$.
- `PO-NUM-4`: Tropical `(max,+)` and `(min,+)` critical paths satisfy zero-slack duality.

## 7. Failure Modes & Deterministic Fallbacks
Indefinite matrices or cyclic Precedence graphs fail closed with `FMINE-REF-301`.

## 8. Gates & Kill Criteria (`GATE-*` / `KILL-*`)
- `GATE-HELM-02`
- `GATE-FSIM-DRIFT`

## 9. Lifted Code & Foundation Dependencies
- `crates/fs-eproc/src/lib.rs` @ `20463525d9775a18c1b237d2676f364f367175ab` -> `src/eproc.rs`
- `crates/fs-voi/src/lib.rs` @ `20463525d9775a18c1b237d2676f364f367175ab` -> `src/voi.rs`
- `crates/fs-robust/src/lib.rs` @ `20463525d9775a18c1b237d2676f364f367175ab` -> `src/risk.rs`

## 10. Non-Goals & Scope Boundary
Zero I/O (`std::fs`, `std::net`) or domain-specific accounting rules (`L-3`).
