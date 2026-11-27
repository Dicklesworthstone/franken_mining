# BUDGET-AMEND-001: `fmine-public`

SwiftBasin drafted this record on 2026-10-31 at 22,000 / 25,000. The 24,000 / 28,000 revision was approved on
2026-11-20 under the owner's explicit session grant of expert judgment on all reality-check decisions.

- **Crate:** `fmine-public`
- **Planned / ceiling before:** 18000 / 21000 (`registries/loc_budget.toml`)
- **Actual at revision:** 22615 (`cargo run -p xtask -- loc` at `05ac300`)
- **New planned:** 26100
- **New ceiling:** 28000 (retains the registered ceiling)
- **New scope:** full public-lane assurance spine shipped through `05ac300`: source-bound citation validation
  (`bd-sjrh.2`), cross-vintage and Mine Stats identity consistency, public LMDI waterfalls (`BD-333`), guidance
  completion and cluster track-record binders (`BD-341`, `BD-342`), copper/gold recovery and reserve-replacement
  adapters, HTML table projection, JV mirror, and disclosure lint, plus `rustfmt` normalization.
- Includes Markdown soft-wrap context in disclosure lint (BD-345) under the owner's standing line-count waiver.
- **Gate served:** `GATE-BULL-01` (M0 public mirror) and `GATE-LAYERS-00` (`xtask check-amend`).
- **Alternatives:** moving public-lane parsers into `fmine-cli` violates layer separation (`V` → `U`); stripping
  unit tests into `xtask/testdata/` solely to hide LOC is prohibited ceremony.
- **Consequences:** `registries/loc_budget.toml` updates `fmine-public` to `24000 / 28000` (+6,000 planned, +7,000
  ceiling); combined with `BUDGET-AMEND-002` through `005`, workspace totals move from `1,128,000 / 1,298,000` to
  `1,171,000 / 1,347,000`.
- **Approval:** approved by owner (expert-judgment grant 2026-11-20; owner may revise), 2026-11-20

**DESIGN (2026-11-25, bd-sjrh.2).** Extend the existing 24,000 allowance by 500 lines under the owner's
standing direction against blocking useful changes on LoC budgets. The 24,092-line measured version adds
source validation for hidden-sheet exceptions and checks the rendered footing certificate against its
operands. The allowance applies to `fmine-public` through this record. The registered workspace budget
continues to provide the planned baseline used by `xtask loc`.

**DESIGN (2026-11-25, bd-058/bd-059).** Extend the allowance to 25,600 under the same owner direction.
The 25,530-line measured version supports explicitly cited closing parenthesis cells in registered HTML
Claims. Keep the registered budget row and ceiling for change measurement.

**DESIGN (2026-11-26, bd-kftl.2).** Extend the allowance to 25,700 under the same owner direction to cover
refusal of salted internal review packets on public assurance paths. Keep the registered baseline and ceiling.

**DESIGN (2026-11-26, bd-kftl.2).** Extend the same allowance to 25,800 for manifest-integrity checks
on the refusal path at `5eb8997`. Keep the registered baseline and ceiling.

**DESIGN (2026-11-26).** Extend the allowance to 26,000 under the same owner direction to cover the
registered forecast source binding and cited capital expenditure verification integrated on canonical
`main`. Keep the registered baseline and ceiling.
