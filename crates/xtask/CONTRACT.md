# CONTRACT.md — `xtask`

## 1. Charter & Layer
- **Crate:** `xtask`
- **Layer:** `T` (Build/Verification tooling)
- **Subsystem:** `Testpit`
- **LOC Budget:** Dev-only verification binary (`GATE-LAYERS-00`)
- **Allowed Module Families:** `["loc", "layers", "deps", "caps", "lifts", "scope", "claims", "det", "oracle"]`

## 2. Mathematical & Domain Formulation
Enforces the structural invariants of the workspace (`L-1`..`L-7`, `PO-NUM-2`, `GATE-ORACLE-01`, `§28.4` LOC budget ceilings) over the repository source tree and `registries/*.toml`.

## 3. Authority, Epistemics & Representation
Pure static verifier over repository source files and TOML registries; produces deterministic pass/fail diagnostics (`GATE-LAYERS-00`).

## 4. Public API Surface
- `check_loc_budget(root: &Path) -> Result<LocReport, Vec<String>>`
- `check_layers(root: &Path) -> Result<(), Vec<String>>`
- `check_deps(root: &Path) -> Result<(), Vec<String>>`
- `check_caps(root: &Path) -> Result<(), Vec<String>>`
- `check_lifts(root: &Path) -> Result<(), Vec<String>>`
- `check_scope(root: &Path) -> Result<(), Vec<String>>`
- `check_claims(root: &Path) -> Result<(), Vec<String>>`
- `check_det(root: &Path) -> Result<(), Vec<String>>`
- `check_oracle(root: &Path) -> Result<(), Vec<String>>`
- `check_all(root: &Path) -> Result<LocReport, Vec<String>>`

## 5. Guards & Typed Refusals
Fails CI (`exit 2`) on any upward layer edge (`L-1`), banned dependency (`L-5`), `open_child_region` escalation outside `fmine-runtime` (`L-3`), lift provenance mismatch (`L-7`), undeclared module family (`§28.4`), bare `f64` physical quantity in `K`/`E` public signatures (`L-6`), `std` transcendental in `BitwisePortable` modules (`PO-NUM-2`), LOC ceiling overrun (`§28.4`), or oracle crate in a production closure (`GATE-ORACLE-01`).

## 6. Proof Obligations (`PO-*`)
Each check has a positive workspace test and a seeded negative violation unit test (`BD-003`..`BD-015`).

## 7. Failure Modes & Deterministic Fallbacks
Deterministic error collection across all files in lexicographical path order.

## 8. Gates & Kill Criteria (`GATE-*` / `KILL-*`)
- `GATE-LAYERS-00` (`BD-015`)
- `GATE-ORACLE-01` (`BD-013`)

## 9. Lifted Code & Foundation Dependencies
`std`-only.

## 10. Non-Goals & Scope Boundary
Never linked by any production crate.
