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
- A mine-grid point farther outside the control hull than its buffer is refused (G-CRS-2): `test_mine_grid_solver_and_convex_hull_guard_g_crs_2`.
- The UTM forward projection followed by the inverse returns the input to 1e-8 degrees for Reko Diq (41N) and Veladero (19S): `test_utm_roundtrip_reko_diq_and_carlin`. Despite its name, the test has no Carlin point.

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
- `utm_to_geodetic` does not check easting or northing ranges.
- Mine grid: within `max_extrapolation_m` of the control hull, with at least 3 non-collinear monuments.

## 9. Oracles
None (gap). The UTM test is a round trip through this crate's own forward and inverse, not a comparison with PROJ, which §33.2 names as the oracle for CRS (PO-CRS-1, GATE-PORT-01).

## 10. Known gaps
- BD-033 (bd-033) and BD-034 (bd-034): the code and tests exist, but they stay open behind bd-023 (`Qty<D, B, R>`).
- No PROJ comparison for the projection (PO-CRS-1, GATE-PORT-01); no bead covers it yet.
- `Helmert7Params::apply` has no test and no input checks; no "registered Helmert transforms" registry exists (BD-033's title).
- The UTM zone is not checked against the longitude.
- The determinism class is undeclared.

## 11. Public API
- `CrsError` (`refusal_code`), `CrsId` (`REKO_DIQ_UTM41N`, `NGM_UTM11N`, `VELADERO_UTM19S`, `NORTH_MARA_UTM36S`)
- `GeodeticPoint`, `ProjectedPoint` (`distance_3d`)
- `geodetic_to_utm`, `utm_to_geodetic`
- `Helmert7Params` (`apply`)
- `ControlMonument`, `MineGridTransform` (`fit`, `transform`)

## 12. Proof obligations
- PO-CRS-2 (out-of-hull points refused with FMINE-REF-106): `test_mine_grid_solver_and_convex_hull_guard_g_crs_2`.
- PO-CRS-1 (accuracy against PROJ) is not discharged: there is a round-trip test only.
