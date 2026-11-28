# CONTRACT.md — `fmine-geostat`

## 1. Purpose
Strata variography (`BD-237`, plan §13, WP-16), the first slice of the geostatistics crate. Drill composites of one domain and one support become sample points on their desurveyed traces. Directional experimental semivariograms are computed over them with lag, angle and bandwidth tolerances. A model is a nugget and up to four nested spherical, exponential or Gaussian structures, each with geometric anisotropy under a GSLIB rotation. It is fitted to the experimental values by weighted least squares: exact non-negative least squares for the nugget and sills, Nelder–Mead over the logarithms of the free ranges. Kriging (`BD-238`), change of support and conditional simulation, the rest of the crate's charter, come in later slices.

- **Crate:** `fmine-geostat`
- **Layer:** K2
- **Subsystem:** Strata
- **LOC budget:** 40,000 planned / 46,000 ceiling
- **Allowed Module Families:** `["error", "experimental", "fit", "model", "samples"]`

## 2. Inputs
- Desurveyed traces (`fmine_drill::desurvey::Trace`) with each hole's composite set (`fmine_drill::composite::CompositeSet`), a domain code, the support to take (`fmine_claim::SpatialSupport`) and a rule for censored composites (`samples::Censored`: refuse, lower bound, upper bound or midpoint). Or a `samples::SampleSet` built directly: a CRS, one support, the hole names, and points with values.
- For experimental variograms: up to 64 `experimental::Direction`s (azimuth, dip, angle tolerance, horizontal and vertical bandwidths), `experimental::Lags` (count up to 1,000, width, tolerance above 0 and at most the width) and a `Pairs` scope (any pair, pairs within a hole, pairs across holes).
- For a fit: experimental variograms and a `fit::Spec`: whether there is a nugget, the structures (shape, and isotropic or anisotropic ranges each fixed or free from an initial value, under a fixed rotation), and the lag weights (`Pairs`: N, or `PairsOverDistanceSquared`: N / h²).
- Dependencies: `fmine-core` (`det` arithmetic), `fmine-evidence` (refusals), `fmine-claim` (`SpatialSupport`), `fmine-drill` (traces and composites) and `fmine-numerics` (`cholesky_solve`) (`K2 -> S0`, `K2 -> K0`, `K2 -> K1`). Zero external crates.

## 3. Outputs
- `samples::FromComposites`: the sample set (each kept composite at the trace point of its mid-depth, rounded down to the millimetre) and the count of composites left out because they had another support, no value, part of their core unsampled, or censored bounds (counted whether substituted or refused).
- `experimental::ExperimentalSet`: per direction, the lags that hold a pair, each with its index, pair count, mean distance, γ = Σ (z_head − z_tail)² / 2N and the head and tail means; the lag scheme and pair scope; and the number of coincident pairs, which count in no lag.
- `model::Model`: γ and the covariance at any lag vector, and the total sill. `SampleSet::variance` gives the population variance of the values, the usual reference for a total sill.
- `fit::Fit`: the fitted model, the weighted sum of squares it leaves (weights scaled to average 1), the number of lags fitted, the objective evaluations spent and the restarts of the range search.

## 4. Invariants
- **One support, one CRS (`INV-014`):** a sample set holds one declared support and one CRS by construction. `from_composites` takes only composites of the requested support and counts the rest; traces in two CRSs are refused.
- **Nothing is dropped silently:** every composite of the domain is either a sample or counted as other support, unsampled, partly sampled or censored. A censored composite is refused unless the caller chose a substitution (`experimental::test_composites_of_two_holes_give_the_variograms_worked_by_hand`).
- **The grid search is the pair scan:** the neighbour grid gives the same lags, bit for bit, as a double loop over every pair in index order with every lag tested. Checked on four seeded sets of 1,502 points spread over at least four grid cells per axis, with coincident points within and across holes, under three lag schemes (one with overlapping classes), three pair scopes and four directions including omnidirectional and vertical (`experimental::test_the_grid_search_gives_the_brute_force_lags_bit_for_bit`).
- **Head and tail follow the direction:** the head is the sample ahead along the direction's unit vector. Down two vertical holes and across them, γ, head and tail means and pair counts equal values worked by hand. A cross-hole pair 11.3° off horizontal is outside a 10° cone, and counts when omnidirectional.
- **Exact recovery:** a model's own γ at its lags is fitted back to that model to 10⁻⁷ relative in nugget, sills and ranges. Shown for a nugget with nested spherical and exponential structures under both weightings, and for a spherical structure rotated 35°/−20°/15° with ranges 240, 120 and 40 m from five directions (`fit::test_a_nested_isotropic_model_is_recovered_from_its_own_variogram`, `fit::test_a_rotated_anisotropic_model_is_recovered_from_five_directions`).
- **The sills are the non-negative least-squares optimum:** for the ranges found, the nugget and sills are the best of the unconstrained solutions over every subset of the coefficients that come out positive, and all zero. A variogram whose unconstrained nugget is negative is fitted with the nugget at 0; there the residuals are orthogonal to the sill's column and their weighted sum is negative, the KKT conditions (`fit::test_a_negative_nugget_is_held_at_zero_where_the_kkt_conditions_hold`).
- **A noisy fit is the oracle's optimum:** on 40 lags of a spherical model with ±0.04 noise, under both weightings, the objective, nugget and sill equal an independent closed-form two-coefficient solve at the fitted range to 10⁻⁹. No range on a grid from 1 m to 1,000 m at 0.05% steps fits better, and a 10⁻⁶ relative step either way does not improve it (`fit::test_a_noisy_fit_is_the_global_least_squares_optimum_an_oracle_finds`).
- **What the lags cannot determine is refused:** a free range of an anisotropic structure that the directions do not separate from its other free ranges (the columns (e_a·u)² of the free axes must have full rank), fewer lags than parameters, and a fitted range at the edge of the search on a structure with a positive sill (`fit::test_a_minor_range_no_direction_sees_is_refused_and_fits_once_fixed`, `fit::test_fits_the_lags_cannot_support_are_refused`).
- **Models are valid:** sills and nugget are finite and non-negative, ranges positive and finite, angles finite. A nested anisotropic model's covariance matrix over scattered points is positive definite (`model::tests::test_a_nested_anisotropic_covariance_is_positive_definite`).
- **GSLIB conventions:** spherical 1.5h − 0.5h³, exponential 1 − exp(−3h) and Gaussian 1 − exp(−3h²) by practical range; rotations follow `setrot` (`model::tests::test_structures_take_their_textbook_values`, `model::tests::test_rotations_follow_gslib_setrot`, `model::tests::test_each_range_lies_on_its_axis_for_any_rotation`).

## 5. Refusals
- `GeostatError::Invalid` → `FMINE-REF-109`: an empty CRS or domain, no holes, a hole named twice, a sample of a hole not named, a non-finite coordinate or value, more samples than `MAX_SAMPLES`; lags or directions outside their domains, no directions or more than 64, points spread over more than 10⁹ times the farthest lag; a model parameter outside its domain or more than four structures; a fit with no nugget and no structures, a fixed or initial range that is not positive and finite, an initial free range outside the search, a lag with no pairs or a non-finite or negative value, or an anisotropic structure fitted to an omnidirectional variogram.
- `GeostatError::CrsMismatch` → `FMINE-REF-106`: traces in more than one CRS.
- `GeostatError::NotObservable` → `FMINE-REF-202`: a censored composite under `Censored::Refuse`; a fit with fewer lags than parameters, a free range the directions do not determine, or a range that runs to the edge of the search.
- `GeostatError::NoConvergence` → `FMINE-REF-207`: the range search spent `MAX_EVALUATIONS` (200,000) objective evaluations, or was still improving after 20 restarts (`error::tests::test_each_error_names_its_registered_refusal`).

## 6. Determinism class
`BitwisePortable` (plan §29.2): IEEE-754 `+ − × ÷`, `sqrt` and `floor`, with `exp`, `ln` and `sin_cos` through `fmine_core::det`. Pair sums run in (first sample, second sample) index order, the order of a scan over every pair; the neighbour grid sorts its candidates into that order. The fit's subsets, Nelder–Mead steps and restarts run in a fixed order, and its sorts are stable under `total_cmp`.

## 7. Evidence class of outputs
Not applicable: no Claims are emitted, so no `EvidenceClass` or `KnowledgeState` is assigned. A fitted model is a derived description of the samples given; it is not a resource estimate and carries no QP acceptance.

## 8. Validity domain
- Coordinates in metres in a projected CRS. The CRS is carried and compared, not converted; samples in a geographic CRS would give distances in degrees.
- One domain at a time, under the intrinsic hypothesis: γ depends on the lag alone within the domain. Nothing tests stationarity, detrends or declusters.
- Samples sit at composite mid-depths; a composite's value is taken at a point. Censored values are substituted by the caller's chosen rule or refused; no censored-data likelihood is used.
- The fit treats each direction's lags as lying exactly along its unit vector, at the lag's mean distance; the angle cone and bandwidths are not modelled. Rotations are fixed by the caller, not fitted.
- Free ranges are searched between a thousandth of the shortest lag and a thousand times the longest.

## 9. Oracles
- Experimental variograms: a brute-force double loop over every pair, written in the test with every lag tested and no candidate pruning; two vertical holes composited by `fmine_drill::composite` and desurveyed by `fmine_drill::desurvey`, with γ, head and tail means and pair counts worked by hand.
- Fit: models evaluated at their own lags (exact recovery); a closed-form solve of the four candidate supports of a nugget and one sill, written in the test, scanned over the range; the KKT conditions computed in the test from the fitted model.
- Models: textbook values of each shape, GSLIB `setrot` matrices, ranges reached on each axis, and a Cholesky factorization of a nested covariance matrix.
- Planted mutants: dropping the positivity test in the NNLS, ignoring the 1/h² weights, skipping the identifiability check and skipping the edge check each fail a fit test; skipping the cells below in the neighbour search, not sorting the neighbours, swapping head and tail and narrowing the candidate lags each fail an experimental test. Removing Nelder–Mead's expansion step passes: it slows the search and leaves the answers.

## 10. Known gaps
- No kriging (`BD-238`, `DeclaredOk` parity), change of support or conditional simulation yet: the rest of the crate's charter (plan §13).
- No declustering, trend removal, normal-score transform, indicator or cross variograms, or robust estimators (Cressie–Hawkins, madogram).
- The range search is local, restarted from its own best point. On the noisy oracle case it finds the global optimum; nothing guarantees that for multi-structure models with several local optima. Fitted ranges carry no uncertainty.
- Shapes are spherical, exponential and Gaussian only: no power, hole-effect or cubic models, and no zonal anisotropy.
- The fit weights are N and N/h²; Cressie's weights, which use the model's own γ, are not offered.

## 11. Public API
- `GeostatError` (`error::GeostatError`)
- `samples::{MAX_SAMPLES, Sample, SampleSet, Censored, FromComposites, from_composites}`
- `experimental::{MAX_LAGS, MAX_DIRECTIONS, Direction, Lags, Pairs, Lag, Experimental, ExperimentalSet, experimental}`
- `model::{MAX_STRUCTURES, Shape, Rotation, Ranges, Structure, Model}`
- `fit::{MAX_EVALUATIONS, RANGE_REACH, Weights, Range, RangeSpec, StructureSpec, Spec, Fit, fit}`

## 12. Proof obligations
- `BD-237` (variography, `tests/experimental.rs`, `tests/fit.rs`, `src/model.rs`):
  - `experimental::test_the_grid_search_gives_the_brute_force_lags_bit_for_bit`
  - `experimental::test_composites_of_two_holes_give_the_variograms_worked_by_hand`
  - `fit::test_a_nested_isotropic_model_is_recovered_from_its_own_variogram`
  - `fit::test_a_rotated_anisotropic_model_is_recovered_from_five_directions`
  - `fit::test_a_minor_range_no_direction_sees_is_refused_and_fits_once_fixed`
  - `fit::test_a_noisy_fit_is_the_global_least_squares_optimum_an_oracle_finds`
  - `fit::test_a_negative_nugget_is_held_at_zero_where_the_kkt_conditions_hold`
  - `fit::test_fits_the_lags_cannot_support_are_refused`
  - `model::tests::test_a_nested_anisotropic_covariance_is_positive_definite`
