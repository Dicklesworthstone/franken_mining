# CONTRACT.md — `xtask`

## 1. Charter & Layer
- **Crate:** `xtask`
- **Layer:** `T` (Build/Verification tooling)
- **Subsystem:** `Testpit`
- **LOC Budget:** Dev-only verification binary (`GATE-LAYERS-00`)
- **Allowed Module Families:** `["loc", "layers", "deps", "caps", "lifts", "scope", "claims", "det", "oracle", "forecast"]`

## 2. Mathematical & Domain Formulation
Enforces the structural invariants of the workspace (`L-1`..`L-7`, `PO-NUM-2`, `GATE-ORACLE-01`, `§28.4` LOC budget ceilings) over the repository source tree and `registries/*.toml`.

## 3. Authority, Epistemics & Representation
Pure static verifier over repository source files and TOML registries; produces deterministic pass/fail diagnostics (`GATE-LAYERS-00`).
The `forecast-*` commands are the exception: they write a record's digest files and run `openssl ts` and `git`
(BD-334.S1, S2).

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
- `check_sources_registry(root: &Path) -> Result<Vec<String>, Vec<String>>` (`registry-check`; on success one
  `{"stage":"registry",…}` JSON log line per `registries/sources/*.toml`)
- `check_all(root: &Path) -> Result<LocReport, Vec<String>>`
- `forecast::{rules_hash, record_hash, record_verify}` (BD-334.S1; commands `forecast-rules-hash <rules.toml>`,
  `forecast-record-hash <dir>`, `forecast-record-verify <dir>`): the IO around the pure `fmine_calib::record`.
  They write `rules.sha256` and `record.sha256`, and replace a different digest only while no proof (`.tsr`,
  `.ots`) stands beside it. `record_verify`
  takes the record's time only from `record.tsr`, after `openssl ts -verify` against `tsa_chain.pem` passes
  (`forecast::proof_times_from_text` reads the reply's time stamp and accuracy; an accuracy that overflows the
  interval refuses). `rules_hash` refuses rules that do not parse or still carry an `OPEN` marker; the record is
  validated against the dates its rules state, and its `forecasts.csv` against their universe and targets.
- `forecast::persistence_baselines` (command `forecast-baseline`): each `run_rate_persistence` baseline of the
  FY2026 rules from the registered Mine Stats workbooks (read through `fmine-public` under the registered
  vocabulary; a workbook with a refused cell refuses): the pool's size, the pairs left out, the two ratio
  quantiles and, once the point quarter's workbook is registered, each live site's exact interval. It needs the
  corpus, so it never runs in CI (RISK-31).
  `scripts/forecast_record_e2e.sh` runs all three on a synthetic record with a throwaway local TSA (`test_tsa`,
  never a record's proof); `scripts/verify_rules_timestamp.sh` checks the rules' proofs with openssl/ots alone.
- `forecast::rules_order` (BD-334.S2; command `forecast-rules-order`): the commit adding
  `forecasts/fy2026/rules.toml` is a strict ancestor of every commit touching `forecasts/fy2026/forecasts.csv`;
  refuses a shallow clone, whose history cannot show the order.

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
`fmine-registry` (the source registries and TOML subset), `fmine-calib` (the pure forecast record) and
`fmine-public` (the label-anchored Mine Stats reader, for `forecast-baseline`); `openssl` on `PATH` for
`forecast-record-verify` and `git` for `forecast-rules-order` only.

## 10. Non-Goals & Scope Boundary
Never linked by any production crate.
