//! Coordinate reference systems (`G-CRS-1`), Snyder Transverse Mercator UTM projections (`BD-033`),
//! 7-parameter Helmert datum transforms, and local mine-grid control-point solver with convex-hull
//! extrapolation guard (`G-CRS-2`, `FMINE-REF-106`, `BD-034`).

use std::fmt;

/// Errors raised by CRS checks, projection bounds, or mine-grid convex-hull guards.
#[derive(Debug, Clone, PartialEq)]
pub enum CrsError {
    /// `FMINE-REF-101` (`G-CRS-1`): Attempted to combine points with different CRS identifiers.
    CrsMismatch { left: CrsId, right: CrsId },
    /// `FMINE-REF-106` (`G-CRS-2`): Point lies outside the control-point convex hull beyond buffer.
    ExtrapolationBeyondHull {
        distance_outside_m: f64,
        max_allowed_m: f64,
    },
    /// Fewer than 3 control monuments or collinear control geometry.
    IllConditionedControlNetwork { reason: &'static str },
    /// Latitude or longitude outside valid UTM domain.
    InvalidGeodeticCoordinate { lat_deg: f64, lon_deg: f64 },
}

impl CrsError {
    /// Returns the canonical `FMINE-REF-*` refusal code when mapped.
    #[must_use]
    pub const fn refusal_code(&self) -> Option<&'static str> {
        match self {
            Self::CrsMismatch { .. } => Some("FMINE-REF-101"),
            Self::ExtrapolationBeyondHull { .. } => Some("FMINE-REF-106"),
            _ => None,
        }
    }
}

impl fmt::Display for CrsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CrsMismatch { left, right } => {
                write!(f, "FMINE-REF-101: CRS mismatch ({left:?} vs {right:?})")
            }
            Self::ExtrapolationBeyondHull {
                distance_outside_m,
                max_allowed_m,
            } => write!(
                f,
                "FMINE-REF-106: point is {distance_outside_m:.2} m outside control hull (max {max_allowed_m:.2} m)"
            ),
            Self::IllConditionedControlNetwork { reason } => {
                write!(f, "ill-conditioned control monument network: {reason}")
            }
            Self::InvalidGeodeticCoordinate { lat_deg, lon_deg } => {
                write!(f, "invalid geodetic coordinate ({lat_deg}, {lon_deg})")
            }
        }
    }
}

impl std::error::Error for CrsError {}

/// Canonical Coordinate Reference System identifier (`BD-033`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CrsId {
    /// `EPSG:4326` WGS84 geographic (`lat`, `lon`, `ellipsoidal_h`).
    Epsg4326,
    /// WGS84 / UTM zone (`zone` `1..=60`, `north`: `true` for N, `false` for S).
    Wgs84Utm { zone: u8, north: bool },
    /// Named local mine grid (`id` hashed/indexed).
    MineGrid { grid_code: u32 },
}

impl CrsId {
    /// Reko Diq (`EPSG:32641`, WGS84 / UTM zone 41N).
    pub const REKO_DIQ_UTM41N: Self = Self::Wgs84Utm {
        zone: 41,
        north: true,
    };
    /// Nevada Gold Mines Carlin/Cortez (`EPSG:32611`, WGS84 / UTM zone 11N).
    pub const NGM_UTM11N: Self = Self::Wgs84Utm {
        zone: 11,
        north: true,
    };
    /// Veladero / Zaldívar (`EPSG:32719`, WGS84 / UTM zone 19S).
    pub const VELADERO_UTM19S: Self = Self::Wgs84Utm {
        zone: 19,
        north: false,
    };
    /// North Mara / Bulyanhulu (`EPSG:32736`, WGS84 / UTM zone 36S).
    pub const NORTH_MARA_UTM36S: Self = Self::Wgs84Utm {
        zone: 36,
        north: false,
    };
}

/// Geodetic coordinate on WGS84 (`lat_deg`, `lon_deg`, `height_m`, `sigma_m`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GeodeticPoint {
    pub lat_deg: f64,
    pub lon_deg: f64,
    pub height_m: f64,
    pub sigma_m: f64,
}

/// Projected 3D coordinate (`easting_m`, `northing_m`, `elevation_m`, `sigma_m`) tagged with `CrsId`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ProjectedPoint {
    pub crs: CrsId,
    pub easting_m: f64,
    pub northing_m: f64,
    pub elevation_m: f64,
    pub sigma_m: f64,
}

impl ProjectedPoint {
    /// Computes 3D Euclidean distance between two points, refusing if `CrsId` differs (`G-CRS-1`).
    pub fn distance_3d(self, other: Self) -> Result<f64, CrsError> {
        if self.crs != other.crs {
            return Err(CrsError::CrsMismatch {
                left: self.crs,
                right: other.crs,
            });
        }
        let dx = self.easting_m - other.easting_m;
        let dy = self.northing_m - other.northing_m;
        let dz = self.elevation_m - other.elevation_m;
        Ok((dx * dx + dy * dy + dz * dz).sqrt())
    }
}

// --- WGS84 Ellipsoid & Snyder Transverse Mercator UTM (BD-033) ---

const WGS84_A: f64 = 6_378_137.0;
const WGS84_F: f64 = 1.0 / 298.257_223_563;
const UTM_K0: f64 = 0.9996;
const UTM_FE: f64 = 500_000.0;
const UTM_FN_SOUTH: f64 = 10_000_000.0;

/// Projects a WGS84 `GeodeticPoint` to UTM (`zone`, `north`) using Snyder's equations.
pub fn geodetic_to_utm(
    pt: GeodeticPoint,
    zone: u8,
    north: bool,
) -> Result<ProjectedPoint, CrsError> {
    if !(1..=60).contains(&zone)
        || !(-80.0..=84.0).contains(&pt.lat_deg)
        || !(-180.0..=180.0).contains(&pt.lon_deg)
    {
        return Err(CrsError::InvalidGeodeticCoordinate {
            lat_deg: pt.lat_deg,
            lon_deg: pt.lon_deg,
        });
    }
    let e2 = 2.0 * WGS84_F - WGS84_F * WGS84_F;
    let ep2 = e2 / (1.0 - e2);
    let phi = pt.lat_deg.to_radians();
    let lam = pt.lon_deg.to_radians();
    let lam0 = ((i32::from(zone) - 1) * 6 - 180 + 3) as f64 * std::f64::consts::PI / 180.0;

    let sin_phi = phi.sin();
    let cos_phi = phi.cos();
    let tan_phi = phi.tan();

    let n = WGS84_A / (1.0 - e2 * sin_phi * sin_phi).sqrt();
    let t = tan_phi * tan_phi;
    let c = ep2 * cos_phi * cos_phi;
    let a = cos_phi * (lam - lam0);

    let e4 = e2 * e2;
    let e6 = e4 * e2;
    let m = WGS84_A
        * ((1.0 - e2 / 4.0 - 3.0 * e4 / 64.0 - 5.0 * e6 / 256.0) * phi
            - (3.0 * e2 / 8.0 + 3.0 * e4 / 32.0 + 45.0 * e6 / 1024.0) * (2.0 * phi).sin()
            + (15.0 * e4 / 256.0 + 45.0 * e6 / 1024.0) * (4.0 * phi).sin()
            - (35.0 * e6 / 3072.0) * (6.0 * phi).sin());

    let a2 = a * a;
    let a3 = a2 * a;
    let a4 = a2 * a2;
    let a5 = a4 * a;
    let a6 = a4 * a2;

    let easting = UTM_FE
        + UTM_K0
            * n
            * (a + (1.0 - t + c) * a3 / 6.0
                + (5.0 - 18.0 * t + t * t + 72.0 * c - 58.0 * ep2) * a5 / 120.0);

    let mut northing = UTM_K0
        * (m + n
            * tan_phi
            * (a2 / 2.0
                + (5.0 - t + 9.0 * c + 4.0 * c * c) * a4 / 24.0
                + (61.0 - 58.0 * t + t * t + 600.0 * c - 330.0 * ep2) * a6 / 720.0));
    if !north {
        northing += UTM_FN_SOUTH;
    }

    Ok(ProjectedPoint {
        crs: CrsId::Wgs84Utm { zone, north },
        easting_m: easting,
        northing_m: northing,
        elevation_m: pt.height_m,
        sigma_m: pt.sigma_m,
    })
}

/// Inverse Snyder Transverse Mercator projection from WGS84 UTM to `GeodeticPoint`.
pub fn utm_to_geodetic(pt: ProjectedPoint) -> Result<GeodeticPoint, CrsError> {
    let CrsId::Wgs84Utm { zone, north } = pt.crs else {
        return Err(CrsError::CrsMismatch {
            left: pt.crs,
            right: CrsId::Epsg4326,
        });
    };
    let e2 = 2.0 * WGS84_F - WGS84_F * WGS84_F;
    let ep2 = e2 / (1.0 - e2);
    let e1 = (1.0 - (1.0 - e2).sqrt()) / (1.0 + (1.0 - e2).sqrt());

    let x = pt.easting_m - UTM_FE;
    let y = if north {
        pt.northing_m
    } else {
        pt.northing_m - UTM_FN_SOUTH
    };

    let m = y / UTM_K0;
    let e4 = e2 * e2;
    let e6 = e4 * e2;
    let mu = m / (WGS84_A * (1.0 - e2 / 4.0 - 3.0 * e4 / 64.0 - 5.0 * e6 / 256.0));

    let e1_2 = e1 * e1;
    let e1_3 = e1_2 * e1;
    let e1_4 = e1_2 * e1_2;
    let phi1 = mu
        + (3.0 * e1 / 2.0 - 27.0 * e1_3 / 32.0) * (2.0 * mu).sin()
        + (21.0 * e1_2 / 16.0 - 55.0 * e1_4 / 32.0) * (4.0 * mu).sin()
        + (151.0 * e1_3 / 96.0) * (6.0 * mu).sin()
        + (1097.0 * e1_4 / 512.0) * (8.0 * mu).sin();

    let sin1 = phi1.sin();
    let cos1 = phi1.cos();
    let tan1 = phi1.tan();
    let n1 = WGS84_A / (1.0 - e2 * sin1 * sin1).sqrt();
    let r1 = WGS84_A * (1.0 - e2) / (1.0 - e2 * sin1 * sin1).powf(1.5);
    let t1 = tan1 * tan1;
    let c1 = ep2 * cos1 * cos1;
    let d = x / (n1 * UTM_K0);
    let d2 = d * d;
    let d3 = d2 * d;
    let d4 = d2 * d2;
    let d5 = d4 * d;
    let d6 = d4 * d2;

    let phi = phi1
        - (n1 * tan1 / r1)
            * (d2 / 2.0 - (5.0 + 3.0 * t1 + 10.0 * c1 - 4.0 * c1 * c1 - 9.0 * ep2) * d4 / 24.0
                + (61.0 + 90.0 * t1 + 298.0 * c1 + 45.0 * t1 * t1 - 252.0 * ep2 - 3.0 * c1 * c1)
                    * d6
                    / 720.0);

    let lam0 = ((i32::from(zone) - 1) * 6 - 180 + 3) as f64 * std::f64::consts::PI / 180.0;
    let lam = lam0
        + (d - (1.0 + 2.0 * t1 + c1) * d3 / 6.0
            + (5.0 - 2.0 * c1 + 28.0 * t1 - 3.0 * c1 * c1 + 8.0 * ep2 + 24.0 * t1 * t1) * d5
                / 120.0)
            / cos1;

    Ok(GeodeticPoint {
        lat_deg: phi.to_degrees(),
        lon_deg: lam.to_degrees(),
        height_m: pt.elevation_m,
        sigma_m: pt.sigma_m,
    })
}

/// 7-parameter linearized Helmert transformation in ECEF/projected Cartesian coordinates (`BD-033`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Helmert7Params {
    pub tx_m: f64,
    pub ty_m: f64,
    pub tz_m: f64,
    pub rx_arcsec: f64,
    pub ry_arcsec: f64,
    pub rz_arcsec: f64,
    pub scale_ppm: f64,
    pub transform_sigma_m: f64,
}

impl Helmert7Params {
    /// Applies the 7-parameter Helmert transformation and propagates positional uncertainty in quadrature.
    #[must_use]
    pub fn apply(&self, x: f64, y: f64, z: f64, input_sigma_m: f64) -> (f64, f64, f64, f64) {
        let arcsec_to_rad = std::f64::consts::PI / (180.0 * 3600.0);
        let rx = self.rx_arcsec * arcsec_to_rad;
        let ry = self.ry_arcsec * arcsec_to_rad;
        let rz = self.rz_arcsec * arcsec_to_rad;
        let m = 1.0 + self.scale_ppm * 1e-6;

        let x_out = self.tx_m + m * (x - rz * y + ry * z);
        let y_out = self.ty_m + m * (rz * x + y - rx * z);
        let z_out = self.tz_m + m * (-ry * x + rx * y + z);
        let sigma_out = (input_sigma_m * input_sigma_m
            + self.transform_sigma_m * self.transform_sigma_m)
            .sqrt();
        (x_out, y_out, z_out, sigma_out)
    }
}

// --- Local Mine-Grid Control-Monument Solver & Convex-Hull Guard (BD-034, G-CRS-2) ---

/// Survey control monument pairing local mine-grid coordinates with target UTM coordinates.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ControlMonument {
    pub local_e: f64,
    pub local_n: f64,
    pub local_z: f64,
    pub target_e: f64,
    pub target_n: f64,
    pub target_z: f64,
}

/// Fitted 2D similarity + vertical shift transform from local mine grid to target UTM (`BD-034`).
///
/// `E_target = t_e + a * E_local - b * N_local`
/// `N_target = t_n + b * E_local + a * N_local`
/// `Z_target = t_z + Z_local`
#[derive(Debug, Clone, PartialEq)]
pub struct MineGridTransform {
    pub source_crs: CrsId,
    pub target_crs: CrsId,
    pub a: f64,
    pub b: f64,
    pub t_e: f64,
    pub t_n: f64,
    pub t_z: f64,
    pub rms_residual_m: f64,
    pub max_extrapolation_m: f64,
    /// Counter-clockwise 2D convex hull of control monuments in local `(E, N)` coordinates.
    pub hull_local: Vec<(f64, f64)>,
}

impl MineGridTransform {
    /// Fits a similarity transform from `>= 3` non-collinear control monuments and computes
    /// the 2D convex hull for `G-CRS-2` (`FMINE-REF-106`).
    pub fn fit(
        source_crs: CrsId,
        target_crs: CrsId,
        monuments: &[ControlMonument],
        max_extrapolation_m: f64,
    ) -> Result<Self, CrsError> {
        if monuments.len() < 3 {
            return Err(CrsError::IllConditionedControlNetwork {
                reason: "at least 3 control monuments are required",
            });
        }
        let n = monuments.len() as f64;
        let mean_le = monuments.iter().map(|m| m.local_e).sum::<f64>() / n;
        let mean_ln = monuments.iter().map(|m| m.local_n).sum::<f64>() / n;
        let mean_lz = monuments.iter().map(|m| m.local_z).sum::<f64>() / n;
        let mean_te = monuments.iter().map(|m| m.target_e).sum::<f64>() / n;
        let mean_tn = monuments.iter().map(|m| m.target_n).sum::<f64>() / n;
        let mean_tz = monuments.iter().map(|m| m.target_z).sum::<f64>() / n;

        let mut s_xx_yy = 0.0_f64;
        let mut num_a = 0.0_f64;
        let mut num_b = 0.0_f64;
        for m in monuments {
            let x = m.local_e - mean_le;
            let y = m.local_n - mean_ln;
            let u = m.target_e - mean_te;
            let v = m.target_n - mean_tn;
            s_xx_yy += x * x + y * y;
            num_a += x * u + y * v;
            num_b += x * v - y * u;
        }
        if s_xx_yy < 1e-6 {
            return Err(CrsError::IllConditionedControlNetwork {
                reason: "degenerate control monument spread",
            });
        }

        let a = num_a / s_xx_yy;
        let b = num_b / s_xx_yy;
        let t_e = mean_te - (a * mean_le - b * mean_ln);
        let t_n = mean_tn - (b * mean_le + a * mean_ln);
        let t_z = mean_tz - mean_lz;

        let pts: Vec<(f64, f64)> = monuments.iter().map(|m| (m.local_e, m.local_n)).collect();
        let hull_local = convex_hull_2d(&pts);
        if hull_local.len() < 3 || polygon_area(&hull_local) < 1e-3 {
            return Err(CrsError::IllConditionedControlNetwork {
                reason: "control monuments are collinear",
            });
        }

        let mut sq_err = 0.0_f64;
        for m in monuments {
            let pe = t_e + a * m.local_e - b * m.local_n;
            let pn = t_n + b * m.local_e + a * m.local_n;
            let pz = t_z + m.local_z;
            let de = pe - m.target_e;
            let dn = pn - m.target_n;
            let dz = pz - m.target_z;
            sq_err += de * de + dn * dn + dz * dz;
        }
        let rms_residual_m = (sq_err / n).sqrt();

        Ok(Self {
            source_crs,
            target_crs,
            a,
            b,
            t_e,
            t_n,
            t_z,
            rms_residual_m,
            max_extrapolation_m,
            hull_local,
        })
    }

    /// Transforms a `ProjectedPoint` from `self.source_crs` into `self.target_crs`, enforcing
    /// both `G-CRS-1` (`FMINE-REF-101`) and the convex-hull extrapolation guard `G-CRS-2` (`FMINE-REF-106`).
    pub fn transform(&self, pt: ProjectedPoint) -> Result<ProjectedPoint, CrsError> {
        if pt.crs != self.source_crs {
            return Err(CrsError::CrsMismatch {
                left: pt.crs,
                right: self.source_crs,
            });
        }
        let dist_outside =
            distance_outside_convex_hull((pt.easting_m, pt.northing_m), &self.hull_local);
        if dist_outside > self.max_extrapolation_m {
            return Err(CrsError::ExtrapolationBeyondHull {
                distance_outside_m: dist_outside,
                max_allowed_m: self.max_extrapolation_m,
            });
        }
        let e = self.t_e + self.a * pt.easting_m - self.b * pt.northing_m;
        let n = self.t_n + self.b * pt.easting_m + self.a * pt.northing_m;
        let z = self.t_z + pt.elevation_m;
        let sigma = (pt.sigma_m * pt.sigma_m + self.rms_residual_m * self.rms_residual_m).sqrt();
        Ok(ProjectedPoint {
            crs: self.target_crs,
            easting_m: e,
            northing_m: n,
            elevation_m: z,
            sigma_m: sigma,
        })
    }
}

fn cross_2d(o: (f64, f64), a: (f64, f64), b: (f64, f64)) -> f64 {
    (a.0 - o.0) * (b.1 - o.1) - (a.1 - o.1) * (b.0 - o.0)
}

fn convex_hull_2d(pts: &[(f64, f64)]) -> Vec<(f64, f64)> {
    let mut sorted = pts.to_vec();
    sorted.sort_by(|a, b| a.0.total_cmp(&b.0).then_with(|| a.1.total_cmp(&b.1)));
    sorted.dedup_by(|a, b| (a.0 - b.0).abs() < 1e-9 && (a.1 - b.1).abs() < 1e-9);
    if sorted.len() <= 2 {
        return sorted;
    }
    let mut lower = Vec::new();
    for &p in &sorted {
        while lower.len() >= 2 && cross_2d(lower[lower.len() - 2], lower[lower.len() - 1], p) <= 0.0
        {
            lower.pop();
        }
        lower.push(p);
    }
    let mut upper = Vec::new();
    for &p in sorted.iter().rev() {
        while upper.len() >= 2 && cross_2d(upper[upper.len() - 2], upper[upper.len() - 1], p) <= 0.0
        {
            upper.pop();
        }
        upper.push(p);
    }
    lower.pop();
    upper.pop();
    lower.extend(upper);
    lower
}

fn polygon_area(hull: &[(f64, f64)]) -> f64 {
    let mut area2 = 0.0_f64;
    for i in 0..hull.len() {
        let j = (i + 1) % hull.len();
        area2 += hull[i].0 * hull[j].1 - hull[j].0 * hull[i].1;
    }
    0.5 * area2.abs()
}

fn distance_outside_convex_hull(p: (f64, f64), hull: &[(f64, f64)]) -> f64 {
    let mut inside = true;
    let mut min_dist_sq = f64::INFINITY;
    for i in 0..hull.len() {
        let a = hull[i];
        let b = hull[(i + 1) % hull.len()];
        if cross_2d(a, b, p) < -1e-9 {
            inside = false;
        }
        let vx = b.0 - a.0;
        let vy = b.1 - a.1;
        let len_sq = vx * vx + vy * vy;
        let t = if len_sq > 0.0 {
            (((p.0 - a.0) * vx + (p.1 - a.1) * vy) / len_sq).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let proj_x = a.0 + t * vx;
        let proj_y = a.1 + t * vy;
        let dx = p.0 - proj_x;
        let dy = p.1 - proj_y;
        let d2 = dx * dx + dy * dy;
        if d2 < min_dist_sq {
            min_dist_sq = d2;
        }
    }
    if inside { 0.0 } else { min_dist_sq.sqrt() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_utm_roundtrip_reko_diq_and_carlin() {
        // Reko Diq Western Porphyries approximate coordinates (29.15 N, 62.08 E, UTM Zone 41N).
        let reko = GeodeticPoint {
            lat_deg: 29.152_400,
            lon_deg: 62.081_900,
            height_m: 980.0,
            sigma_m: 0.05,
        };
        let utm = geodetic_to_utm(reko, 41, true).expect("Reko Diq UTM41N");
        assert_eq!(utm.crs, CrsId::REKO_DIQ_UTM41N);
        let back = utm_to_geodetic(utm).expect("inverse UTM41N");
        assert!((back.lat_deg - reko.lat_deg).abs() < 1e-8);
        assert!((back.lon_deg - reko.lon_deg).abs() < 1e-8);

        // Veladero southern hemisphere (29.37 S, 69.95 W, UTM Zone 19S).
        let veladero = GeodeticPoint {
            lat_deg: -29.371_200,
            lon_deg: -69.954_100,
            height_m: 4_100.0,
            sigma_m: 0.10,
        };
        let utm_s = geodetic_to_utm(veladero, 19, false).expect("Veladero UTM19S");
        let back_s = utm_to_geodetic(utm_s).expect("inverse UTM19S");
        assert!((back_s.lat_deg - veladero.lat_deg).abs() < 1e-8);
        assert!((back_s.lon_deg - veladero.lon_deg).abs() < 1e-8);
    }

    #[test]
    fn test_mine_grid_solver_and_convex_hull_guard_g_crs_2() {
        let local_crs = CrsId::MineGrid { grid_code: 101 };
        let target_crs = CrsId::REKO_DIQ_UTM41N;
        let monuments = [
            ControlMonument {
                local_e: 0.0,
                local_n: 0.0,
                local_z: 100.0,
                target_e: 410_000.0,
                target_n: 3_225_000.0,
                target_z: 980.0,
            },
            ControlMonument {
                local_e: 1_000.0,
                local_n: 0.0,
                local_z: 100.0,
                target_e: 411_000.0,
                target_n: 3_225_000.0,
                target_z: 980.0,
            },
            ControlMonument {
                local_e: 1_000.0,
                local_n: 1_000.0,
                local_z: 100.0,
                target_e: 411_000.0,
                target_n: 3_226_000.0,
                target_z: 980.0,
            },
            ControlMonument {
                local_e: 0.0,
                local_n: 1_000.0,
                local_z: 100.0,
                target_e: 410_000.0,
                target_n: 3_226_000.0,
                target_z: 980.0,
            },
        ];

        let tf = MineGridTransform::fit(local_crs, target_crs, &monuments, 250.0)
            .expect("valid control network");
        assert!(tf.rms_residual_m < 1e-6);

        // Inside convex hull succeeds.
        let inside_pt = ProjectedPoint {
            crs: local_crs,
            easting_m: 500.0,
            northing_m: 500.0,
            elevation_m: 120.0,
            sigma_m: 0.02,
        };
        let out = tf.transform(inside_pt).expect("inside hull");
        assert!((out.easting_m - 410_500.0).abs() < 1e-6);
        assert!((out.northing_m - 3_225_500.0).abs() < 1e-6);
        assert!((out.elevation_m - 1_000.0).abs() < 1e-6);

        // Far outside convex hull (> 250m) triggers FMINE-REF-106 (G-CRS-2).
        let far_pt = ProjectedPoint {
            crs: local_crs,
            easting_m: 2_000.0,
            northing_m: 500.0,
            elevation_m: 120.0,
            sigma_m: 0.02,
        };
        let err = tf.transform(far_pt).expect_err("must reject extrapolation");
        assert_eq!(err.refusal_code(), Some("FMINE-REF-106"));
    }

    #[test]
    fn test_crs_mismatch_refusal_g_crs_1() {
        let p1 = ProjectedPoint {
            crs: CrsId::REKO_DIQ_UTM41N,
            easting_m: 410_000.0,
            northing_m: 3_225_000.0,
            elevation_m: 980.0,
            sigma_m: 0.05,
        };
        let p2 = ProjectedPoint {
            crs: CrsId::NGM_UTM11N,
            easting_m: 550_000.0,
            northing_m: 4_500_000.0,
            elevation_m: 1_600.0,
            sigma_m: 0.05,
        };
        let err = p1.distance_3d(p2).expect_err("must refuse CRS mismatch");
        assert_eq!(err.refusal_code(), Some("FMINE-REF-101"));
    }
}
