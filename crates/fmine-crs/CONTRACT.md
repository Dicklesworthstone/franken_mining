# CONTRACT.md — `fmine-crs`

## 1. Purpose
Coordinate reference systems for S0: a CRS identifier on every point, WGS84 UTM forward and inverse projection (Snyder's transverse Mercator series, BD-033), a seven-parameter Helmert shift, and a local mine-grid transform fitted to control monuments that refuses points outside their convex hull (BD-034). Nothing in the workspace calls it yet. It does no geostatistics or block modelling, and it uses no PROJ database.

- **Crate:** `fmine-crs`
- **Layer:** S0
- **Subsystem:** Portal
- **LOC budget:** 15,000 planned / 17,000 ceiling
- **Allowed Module Families:** `["crs", "utm", "helmert", "hull"]`

## 2. Inputs
- `GeodeticPoint { lat_deg, lon_deg, height_m, sigma_m }` with a UTM zone and hemisphere, for `geodetic_to_utm`.
- `ProjectedPoint { crs, easting_m, northing_m, elevation_m, sigma_m }`.
- `ControlMonument` pairs (local and target E, N, Z) and an extrapolation buffer in metres, for `MineGridTransform::fit`.
- Helmert parameters (metres, arc-seconds, ppm).

All values are bare `f64` (S0).

## 3. Outputs
`ProjectedPoint` and `GeodeticPoint` values carrying their `CrsId` and a propagated `sigma_m`; a fitted `MineGridTransform` with its RMS residual and hull; `(x, y, z, sigma)` from `Helmert7Params::apply`. None is a Claim.

## 4. Invariants
- Points on different CRSs are never combined: `distance_3d` and `transform` refuse a mismatch (G-CRS-1); `test_crs_mismatch_refusal_g_crs_1`.
- A mine-grid point farther outside the control hull than its buffer is refused (G-CRS-2): `test_mine_grid_solver_and_convex_hull_guard_g_crs_2`. The plan's G-CRS-2 instead marks such a use `Estimated` with a UU term and refuses only a decision-grade use; the code refuses every such point.
- The UTM forward projection followed by the inverse returns the input to 1e-8 degrees for Reko Diq (41N) and Veladero (19S): `test_utm_roundtrip_reko_diq_and_carlin`. Despite its name, the test has no Carlin point.
- PO-CRS-1: in each of the nine UTM CRSs of `registries/crs.toml`, over 10,000 points of the zone's area of use, the forward projection is within 1.2 mm of PROJ's easting and northing (measured worst 1.007 mm, near ±70° latitude in every zone), and the inverse of PROJ's coordinates is within 0.5 mm of the point (measured worst 0.44 mm, at the strip's edge): `tests/proj_corpus.rs::forward_and_inverse_agree_with_proj_within_stated_accuracy`. A scale factor of 0.99961 for 0.9996, or one coefficient of the inverse's fifth-order longitude term changed, fails it; the round-trip test passes both.

## 5. Refusals
- `CrsError::CrsMismatch` (G-CRS-1) → FMINE-REF-106 (unregistered or incompatible CRS), exit 3 (`src/lib.rs:28-31`).
- `CrsError::ExtrapolationBeyondHull` (G-CRS-2) → FMINE-REF-106, exit 3.
- `CrsError::IllConditionedControlNetwork` (fewer than 3 monuments, or collinear) and `CrsError::InvalidGeodeticCoordinate` (zone outside 1–60, latitude outside [-80, 84], longitude outside [-180, 180]) carry no code: `refusal_code()` returns `None`.

The code is a string, because the crate sits below `fmine-evidence`.

## 6. Determinism class
Undeclared: `src/lib.rs` has no `//! Determinism:` header, which `xtask check-det` requires only in the K and E layers. The projection calls std `sin`, `cos`, `tan` and `sqrt`, so the class would be DeterministicPerIsa, not BitwisePortable.

## 7. Evidence class of outputs
Not applicable: no Claims are emitted. Uncertainty travels only as `sigma_m`, combined in quadrature.

## 8. Validity domain
- UTM: latitudes -80 to 84 and zones 1–60. The zone is not checked against the longitude, so a point can be projected into a distant zone without refusal, and the series loses accuracy there.
- Stated accuracy (PO-CRS-1), against PROJ 9.8.1: 1.2 mm forward and 0.5 mm inverse (inverse error as `a·Δφ` and `a·cos φ·Δλ` with `a` the WGS 84 semi-major axis), inside a registered zone's 6° strip at latitudes 0 to 84 (north) or -80 to 0 (south). Outside the strip, nothing is measured or claimed.
- `utm_to_geodetic` does not check easting or northing ranges.
- Mine grid: within `max_extrapolation_m` of the control hull, with at least 3 non-collinear monuments.

## 9. Oracles
PROJ, which §33.2 names as the oracle for CRS: `fixtures/fmine-crs/proj_utm_corpus.txt` holds PROJ 9.8.1's easting and northing (pyproj 3.8.0, to 0.1 mm) for 10,000 generated points per registered UTM CRS, with its capture record, made by `scripts/oracle/proj_utm_corpus.py`. The points are drawn again by the test from the same integer generator, which checks the record's inputs digest (`generated_points_match_the_capture_inputs_digest`). The Helmert shift and the mine grid have no oracle.

## 10. Known gaps
- The PROJ comparison covers the UTM projection only, inside each registered zone's strip, and ran on one host (arm64-darwin). GATE-PORT-01's other formats are not covered.
- `Helmert7Params::apply` has no test and no input checks; no "registered Helmert transforms" registry exists (BD-033's title), so PO-CRS-1 has no Helmert row to check.
- The UTM zone is not checked against the longitude.
- The determinism class is undeclared.

## 11. Public API
- `CrsError` (`refusal_code`), `CrsId` (`REKO_DIQ_UTM41N`, `NGM_UTM11N`, `VELADERO_UTM19S`, `NORTH_MARA_UTM36S`)
- `GeodeticPoint`, `ProjectedPoint` (`distance_3d`)
- `geodetic_to_utm`, `utm_to_geodetic`
- `Helmert7Params` (`apply`)
- `ControlMonument`, `MineGridTransform` (`fit`, `transform`)

## 12. Proof obligations
- PO-CRS-2 (out-of-hull points refused with FMINE-REF-106; the old contract's label, not a row of the plan's PO table): `test_mine_grid_solver_and_convex_hull_guard_g_crs_2`.
- PO-CRS-1 (registered transforms agree with PROJ within stated accuracy over a 10⁴-point corpus per site CRS): discharged for the nine registered UTM CRSs by `tests/proj_corpus.rs::forward_and_inverse_agree_with_proj_within_stated_accuracy` (bd-035). No Helmert transform is registered.
