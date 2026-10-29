# CONTRACT.md — `fmine-crs`

## 1. Charter & Layer
- **Crate:** `fmine-crs`
- **Layer:** `S0` (Substrate)
- **Subsystem:** `Bedrock`
- **LOC Budget:** `10k planned / 12k ceiling` (`registries/loc_budget.toml`)
- **Allowed Module Families:** `["crs", "utm", "helmert", "hull"]`

## 2. Mathematical & Domain Formulation
Implements `CrsId`, `Point3D`, `Ellipsoid` (`WGS84`, `GRS80`), 6th-order Snyder Transverse Mercator (`geodetic_to_utm`, `utm_to_geodetic`), 7-parameter Helmert datum shifts (`Helmert7Params`), and `MineGridTransform` with 2D Andrew monotone-chain convex-hull control-point domain enforcement (`G-CRS-1`, `G-CRS-2`).

## 3. Authority, Epistemics & Representation
Prevents silent coordinate datum mixing (`FMINE-REF-105`) and out-of-hull local mine-grid extrapolation (`FMINE-REF-106`).

## 4. Public API Surface
- `CrsId`, `Point3D`, `Ellipsoid`, `UtmCoord`
- `geodetic_to_utm`, `utm_to_geodetic`
- `Helmert7Params`, `MineGridTransform`

## 5. Guards & Typed Refusals
- `FMINE-REF-105` (`CrsDatumMismatch`, `G-CRS-1`)
- `FMINE-REF-106` (`MineGridHullExtrapolation`, `G-CRS-2`)

## 6. Proof Obligations (`PO-*`)
- `PO-CRS-1`: Sub-millimeter round-trip accuracy (`geodetic -> UTM -> geodetic` `< 1e-4 m`) across Reko Diq (`UTM 41N`), Lumwana (`UTM 35S`), and Carlin (`UTM 11N`).
- `PO-CRS-2`: Points outside the control-point convex hull are refused with `FMINE-REF-106`.

## 7. Failure Modes & Deterministic Fallbacks
Degenerate control hulls (`< 3` non-collinear points) or datum mismatches fail closed with typed `Refusal`.

## 8. Gates & Kill Criteria (`GATE-*` / `KILL-*`)
- `GATE-CRS-01`

## 9. Lifted Code & Foundation Dependencies
- Depends on `fmine-core`, `fmine-units`.

## 10. Non-Goals & Scope Boundary
Does not perform geostatistical interpolation or block modeling (`K1`).
