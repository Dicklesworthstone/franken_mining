# CONTRACT.md — `fmine-explore`

## 1. Purpose
Lodestar K1 exploration data model, effort accounting, five-class absence taxonomy, and cover-aware detection-thinning kernel (`BD-231`, plan §11.3–§11.4):
- `model`: CRS-bound `GeoCoord`, `DepositRecord` (with Good–Turing `k*` novel-type simplex `DepositTypeProbs`, censored `MarkRecord`, optional `DiscoveryEvent`, aggregation rule id, and `SourceAnchor` provenance), `EffortRecord` (method, geometry, valid interval, `MethodSpec` with line spacing / depth of investigation / detection limits, and `CoverageConfidence`), IDEA-protocol `ElicitationRecord` (`n90 <= n50 <= n10`), and versioned `TractMap`;
- `absence`: all five §11.3 absence classes (`A0NotLooked`, `A1LookedInsensitive`, `A2LookedSensitiveNothing`, `A3FoundSubEconomic`, `A4EffortSuspectedUnrecorded`), their exact `fmine_evidence::KnowledgeState` mapping, and cover-vs-DOI detection-thinning classification ensuring transported cover enters detection `q_j` and never existence `λ`.

- **Crate:** `fmine-explore`
- **Layer:** K1
- **Subsystem:** Lodestar
- **LOC budget:** 15,000 planned / 17,000 ceiling
- **Allowed Module Families:** `["model", "absence", "three_part"]`

## 2. Inputs
- Exploration deposit records, historical effort records, tract definitions, and elicitation records expressed strictly in `fmine_crs::CrsId`, `fmine_units::ExactQty`, and `fmine_core::{ObjectId, SourceAnchor}` (zero bare `f64` parameters in public K-layer signatures; L-6).
- Dependencies: `fmine-core`, `fmine-units`, `fmine-crs`, `fmine-evidence`, `fmine-claim` (`K1 → S0`, zero external crates, zero I/O/clocks/threads; L-3).

## 3. Outputs
- Validated `DepositRecord`, `EffortRecord`, `ElicitationRecord`, `TractMap`, canonical SHA-256 digests, and typed `AbsenceAssessment` carrying `AbsenceClass`, exact `KnowledgeState`, exact thinning hazard `H` or Manski bound `[0, π_era_max]`, and CRS-checked diagnostics.
- Structured `ExploreError` mapped to registered `FMINE-REF-*` refusal codes.

## 4. Invariants
- **CRS discipline (`G-CRS-1`, `BD-033`):** comparing or intersecting coordinates with differing `fmine_crs::CrsId` refuses with `FMINE-REF-106` (`CrsUnregisteredOrIncompatible106`).
- **Good–Turing novel-type slot (`§11.4`, `GB-09`):** `DepositTypeProbs` requires non-negative named-type probabilities plus an explicit `k_star_novel` slot summing exactly to `1`.
- **Absence of evidence is never zero (`§11.3`, `BD-231`):**
  - `A0NotLooked` → `KnowledgeState::NotObservable` (zero likelihood penalty);
  - `A1LookedInsensitive` (e.g. soil geochem over deep transported cover where `q_j <= q_min`) → `KnowledgeState::NotObservable` (cover attenuates detection `q_j`, never existence `λ`);
  - `A2LookedSensitiveNothing` (effective hazard `H > 0`) → `KnowledgeState::Estimated`;
  - `A3FoundSubEconomic` (occurrence with censored mark) → `KnowledgeState::Known`;
  - `A4EffortSuspectedUnrecorded` (`CoverageConfidence::Low | Unknown`) → `KnowledgeState::Unknown` with explicit Manski bound `π ∈ [0, π_era_max]`.
- **IDEA elicitation monotonicity (`§11.3`):** `ElicitationRecord` enforces `n90 <= n50 <= n10` and `PrefixKind::Calibration` on `seeds_scored`.

## 5. Refusals
- `ExploreError::CrsMismatch` → `FMINE-REF-106` (exit 3): attempting to combine or intersect points across incompatible `CrsId`s (`G-CRS-1`).
- `ExploreError::InvalidSimplex` / `ExploreError::InvalidRecord` / `ExploreError::Overflow` → `FMINE-REF-109` (exit 3): probabilities not summing to `1`, negative physical depths/grades/tonnages, non-monotone elicitation quantiles, or arithmetic overflow.
- `ExploreError::MissingProvenance` → `FMINE-REF-107` (exit 3): empty source locator or missing aggregation rule.

## 6. Determinism class
All files (`lib.rs`, `model.rs`, `absence.rs`, `three_part.rs`) declare `//! Determinism: BitwisePortable`. All coordinate, depth, detection, and simplex arithmetic uses exact `i128` rationals (`ExactQty`) over `BTreeMap` collections.

## 7. Evidence class of outputs
Data model and absence-class classifier preserve input provenance without upgrading evidence classes; `AbsenceClass` projects strictly to its §11.3 `KnowledgeState`.

## 8. Validity domain
- Exact rational exploration coordinates, cover depths, DOI thresholds, and simplex weights within `i128` `ExactQty` bounds.
- Planar distance comparisons require identical projected `CrsId` (`G-CRS-1`); geodetic datum transforms use `fmine-crs` upstream.

## 9. Oracles
- Data model construction, Good–Turing simplex closure, CRS mismatch refusal, IDEA elicitation checks, and all five §11.3 absence-class scenarios (`A0..A4`, including 60 m cover masking soil geochemistry and unrecorded 1987 RAB drilling producing Manski bounds) in `model::tests::test_explore_data_model_simplex_crs_and_elicitation` and `absence::tests::test_all_five_absence_classes_and_cover_thinning_law`.

## 10. Known gaps
- Thinned marked Cox process SPDE/GMRF inference (`BD-233`), USGS three-part MapMark4 parity (`BD-235`), and two-level Pad/Collar/Wedge drill portfolio optimization (`BD-315`) build on top of this crate in `fmine-infer` and `fmine-prospect`.

## 11. Public API
- `ExploreError`
- `model::{ExploreMethod, Ingredient, CoverageConfidence, GeoCoord, DepositTypeProbs, MarkRecord, DiscoveryEvent, DepositRecord, MethodSpec, EffortRecord, ElicitationRecord, Tract, TractMap}` (`BD-231`)
- `absence::{AbsenceClass, AbsenceAssessment, effective_detection_q, classify_site_absence}` (`BD-231`, plan §11.3)
- `three_part::{UndiscoveredCountPmf, GradeTonnageDeposit, ThreePartEndowmentSummary, simulate_three_part_endowment, ks_two_sample_exact}` (`BD-235`, N0 MapMark4-degenerate three-part endowment simulator, `PO-LODE-5`)

## 12. Proof obligations
- `BD-231` (deposits, effort, elicitation, tract map, CRS enforcement): `model::tests::test_explore_data_model_simplex_crs_and_elicitation`.
- `BD-231` / §11.3 (all five absence classes `A0..A4` and cover-vs-DOI detection thinning): `absence::tests::test_all_five_absence_classes_and_cover_thinning_law`.
- `BD-235` / `PO-LODE-5` (N0 three-part endowment Wald mean & exact two-sample KS < 0.02): `three_part::tests::test_three_part_n0_endowment_wald_mean_and_ks_stability`.
