//! PO-CRS-1 (bd-035): fmine-crs's UTM projection against PROJ over 10,000 points in each registered UTM CRS.
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use std::fmt::Write as _;

use fmine_core::hash::sha256;
use fmine_crs::{CrsId, GeodeticPoint, ProjectedPoint, geodetic_to_utm, utm_to_geodetic};

const CORPUS: &str = include_str!("../../../fixtures/fmine-crs/proj_utm_corpus.txt");
const RECORD: &str = include_str!("../../../fixtures/fmine-crs/proj_utm_corpus.txt.capture.toml");

/// The capture script's LCG (Knuth's MMIX constants), drawing whole microdegrees.
struct Lcg(u64);

impl Lcg {
    fn micro(&mut self, lo: i64, hi: i64) -> i64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        let span = u64::try_from(hi - lo + 1).unwrap();
        lo + i64::try_from((self.0 >> 11) % span).unwrap()
    }
}

/// The capture script's points for one CRS: `(lat, lon)` in degrees.
fn points(epsg: u64, zone: u8, north: bool, count: usize) -> Vec<(f64, f64)> {
    let mut rng = Lcg(epsg);
    let cm = (i64::from(zone) - 1) * 6 - 180 + 3;
    let (lat_lo, lat_hi) = if north {
        (0, 84_000_000)
    } else {
        (-80_000_000, 0)
    };
    let (lon_lo, lon_hi) = ((cm - 3) * 1_000_000, (cm + 3) * 1_000_000);
    (0..count)
        .map(|_| {
            let lat = rng.micro(lat_lo, lat_hi) as f64 / 1e6;
            let lon = rng.micro(lon_lo, lon_hi) as f64 / 1e6;
            (lat, lon)
        })
        .collect()
}

struct Block {
    epsg: u64,
    zone: u8,
    north: bool,
    points: Vec<(f64, f64)>,
    proj: Vec<(f64, f64)>,
}

fn corpus() -> Vec<Block> {
    let mut blocks: Vec<Block> = Vec::new();
    for line in CORPUS.lines().filter(|l| !l.starts_with('#')) {
        let f: Vec<&str> = line.split(' ').collect();
        if f[0] == "crs" {
            let epsg: u64 = f[1].strip_prefix("EPSG:").unwrap().parse().unwrap();
            let (zone, north, count) = (f[2].parse().unwrap(), f[3] == "N", f[4].parse().unwrap());
            blocks.push(Block {
                epsg,
                zone,
                north,
                points: points(epsg, zone, north, count),
                proj: Vec::with_capacity(count),
            });
        } else {
            let block = blocks.last_mut().expect("a crs line first");
            block
                .proj
                .push((f[0].parse().unwrap(), f[1].parse().unwrap()));
        }
    }
    blocks
}

/// The generated points are the ones the capture script sent through PROJ: the inputs digest of the capture
/// record, recomputed.
#[test]
fn generated_points_match_the_capture_inputs_digest() {
    let blocks = corpus();
    let mut text = String::new();
    for b in &blocks {
        assert_eq!(b.points.len(), b.proj.len(), "EPSG:{}", b.epsg);
        for &(lat, lon) in &b.points {
            writeln!(text, "{:016x} {:016x}", lat.to_bits(), lon.to_bits()).unwrap();
        }
    }
    let want = RECORD
        .lines()
        .find_map(|l| l.strip_prefix("inputs_digest = \"sha256:"))
        .and_then(|l| l.strip_suffix('"'))
        .expect("inputs_digest in the capture record");
    assert_eq!(sha256(text.as_bytes()).to_hex(), want);
    assert_eq!(blocks.len(), 9, "registered UTM CRSs");
    assert!(blocks.iter().all(|b| b.points.len() == 10_000));
}

/// The stated accuracy (CONTRACT §8): the horizontal distance between fmine-crs's easting and northing and PROJ's.
/// Measured worst 1.007 mm, near ±70° latitude in every zone; PROJ's output is rounded to 0.1 mm.
const FORWARD_TOLERANCE_M: f64 = 1.2e-3;
/// The stated accuracy of the inverse: fmine-crs's latitude and longitude from PROJ's easting and northing, against
/// the generated point, as `a·Δφ` and `a·cos φ·Δλ` on a sphere of the WGS 84 semi-major axis. Measured worst 0.44 mm,
/// at the strip's edge.
const INVERSE_TOLERANCE_M: f64 = 0.5e-3;

#[test]
fn forward_and_inverse_agree_with_proj_within_stated_accuracy() {
    let (mut points, mut worst_fwd_all, mut worst_inv_all) = (0usize, 0.0_f64, 0.0_f64);
    for b in corpus() {
        let mut worst_fwd = (0.0_f64, 0.0, 0.0);
        let mut worst_inv = (0.0_f64, 0.0, 0.0);
        let cm = f64::from((i32::from(b.zone) - 1) * 6 - 180 + 3);
        for (&(lat, lon), &(e, n)) in b.points.iter().zip(&b.proj) {
            let g = GeodeticPoint {
                lat_deg: lat,
                lon_deg: lon,
                height_m: 0.0,
                sigma_m: 0.0,
            };
            let p = geodetic_to_utm(g, b.zone, b.north).unwrap();
            let d = (p.easting_m - e).hypot(p.northing_m - n);
            if d > worst_fwd.0 {
                worst_fwd = (d, lat, lon - cm);
            }
            let q = utm_to_geodetic(ProjectedPoint {
                crs: CrsId::Wgs84Utm {
                    zone: b.zone,
                    north: b.north,
                },
                easting_m: e,
                northing_m: n,
                elevation_m: 0.0,
                sigma_m: 0.0,
            })
            .unwrap();
            let dlat = (q.lat_deg - lat).to_radians() * 6_378_137.0;
            let dlon = (q.lon_deg - lon).to_radians() * 6_378_137.0 * lat.to_radians().cos();
            let di = dlat.hypot(dlon);
            if di > worst_inv.0 {
                worst_inv = (di, lat, lon - cm);
            }
            assert!(
                d <= FORWARD_TOLERANCE_M && di <= INVERSE_TOLERANCE_M,
                "EPSG:{} at ({lat}, {lon}): forward {d:e} m, inverse {di:e} m",
                b.epsg
            );
            points += 1;
        }
        println!(
            "{{\"test\":\"proj_utm\",\"crs\":\"EPSG:{}\",\"forward_worst_m\":{:.6},\"forward_worst_lat\":{:.3},\
             \"forward_worst_dlon\":{:.3},\"inverse_worst_m\":{:.6},\"inverse_worst_lat\":{:.3},\
             \"inverse_worst_dlon\":{:.3}}}",
            b.epsg, worst_fwd.0, worst_fwd.1, worst_fwd.2, worst_inv.0, worst_inv.1, worst_inv.2
        );
        worst_fwd_all = worst_fwd_all.max(worst_fwd.0);
        worst_inv_all = worst_inv_all.max(worst_inv.0);
    }
    println!(
        "{{\"test\":\"proj_utm\",\"points\":{points},\"forward_worst_m\":{worst_fwd_all:.6},\
         \"inverse_worst_m\":{worst_inv_all:.6}}}"
    );
    assert_eq!(points, 9 * 10_000, "every corpus point compared");
}
