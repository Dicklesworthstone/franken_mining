//! Determinism: BitwisePortable
//! 3D Open-Pit Au geological world generator: lithology domains, spatially correlated lognormal
//! grades, domain rock densities, and sealed truth statistics (`BD-128`, plan §33.3).

use fmine_core::codec::Encoder;
use fmine_core::det::{exp, ln, norm_ppf, sqrt};
use fmine_core::rand::{PhiloxStream, RngKey};

use crate::GenError;
use crate::seal::{SealKey, SealedTruth};

/// Geological alteration / weathering domains of the synthetic Open-Pit Au deposit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AuDomain {
    Oxide,
    Transition,
    Fresh,
}

/// Target statistical specification for one geological domain.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DomainSpec {
    pub domain: AuDomain,
    /// Target log-grade mean μ = E[ln g] (with g in g/t Au).
    pub target_log_grade_mean: f64,
    /// Target log-grade standard deviation σ = sd(ln g).
    pub target_log_grade_std: f64,
    /// Target dry rock bulk density in t/m³.
    pub mean_density_t_m3: f64,
    /// Dry rock bulk density standard deviation in t/m³.
    pub density_std_t_m3: f64,
}

/// 3D block-model specification for the synthetic Open-Pit Au geology world (`BD-128`).
#[derive(Debug, Clone, PartialEq)]
pub struct OpenPitAuGeologySpec {
    pub world_id: String,
    pub nx: usize,
    pub ny: usize,
    pub nz: usize,
    pub block_dx_m: f64,
    pub block_dy_m: f64,
    pub block_dz_m: f64,
    pub oxide: DomainSpec,
    pub transition: DomainSpec,
    pub fresh: DomainSpec,
}

impl OpenPitAuGeologySpec {
    /// Standard 24×24×12 (6,912-block) benchmark Open-Pit Au deposit specification.
    #[must_use]
    pub fn standard() -> Self {
        Self {
            world_id: "au-geology-3d-v1".to_string(),
            nx: 24,
            ny: 24,
            nz: 12,
            block_dx_m: 10.0,
            block_dy_m: 10.0,
            block_dz_m: 10.0,
            oxide: DomainSpec {
                domain: AuDomain::Oxide,
                target_log_grade_mean: 0.25,
                target_log_grade_std: 0.55,
                mean_density_t_m3: 2.35,
                density_std_t_m3: 0.08,
            },
            transition: DomainSpec {
                domain: AuDomain::Transition,
                target_log_grade_mean: 0.55,
                target_log_grade_std: 0.65,
                mean_density_t_m3: 2.60,
                density_std_t_m3: 0.08,
            },
            fresh: DomainSpec {
                domain: AuDomain::Fresh,
                target_log_grade_mean: 0.70,
                target_log_grade_std: 0.70,
                mean_density_t_m3: 2.78,
                density_std_t_m3: 0.07,
            },
        }
    }

    fn validate(&self) -> Result<(), GenError> {
        if self.world_id.is_empty() {
            return Err(GenError::InvalidConfig {
                field: "world_id",
                detail: "must be non-empty".to_string(),
            });
        }
        if self.nx == 0 || self.ny == 0 || self.nz == 0 || self.nx * self.ny * self.nz > 250_000 {
            return Err(GenError::InvalidConfig {
                field: "grid_dims",
                detail: format!("invalid grid {}x{}x{}", self.nx, self.ny, self.nz),
            });
        }
        for (f, v) in [
            ("block_dx_m", self.block_dx_m),
            ("block_dy_m", self.block_dy_m),
            ("block_dz_m", self.block_dz_m),
        ] {
            if !(v.is_finite() && v > 0.0) {
                return Err(GenError::InvalidConfig {
                    field: f,
                    detail: "must be finite and > 0".to_string(),
                });
            }
        }
        for spec in [&self.oxide, &self.transition, &self.fresh] {
            if !(spec.target_log_grade_mean.is_finite()
                && spec.target_log_grade_std.is_finite()
                && spec.target_log_grade_std > 0.0
                && spec.mean_density_t_m3.is_finite()
                && spec.mean_density_t_m3 > 1.0
                && spec.density_std_t_m3.is_finite()
                && spec.density_std_t_m3 >= 0.0)
            {
                return Err(GenError::InvalidConfig {
                    field: "domain_spec",
                    detail: format!("invalid domain spec for {:?}", spec.domain),
                });
            }
        }
        Ok(())
    }
}

/// One true 3D geological block inside the sealed Open-Pit Au deposit.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GeologyBlock {
    pub ix: usize,
    pub iy: usize,
    pub iz: usize,
    pub domain: AuDomain,
    pub density_t_m3: f64,
    pub dry_tonnes: f64,
    pub au_grade_g_per_t: f64,
    pub contained_au_grams: f64,
}

/// Realized summary statistics for one domain.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DomainStats {
    pub domain: AuDomain,
    pub block_count: usize,
    pub total_tonnes: f64,
    pub total_au_grams: f64,
    pub mean_grade_g_per_t: f64,
    pub realized_log_grade_mean: f64,
    pub realized_log_grade_std: f64,
    pub realized_mean_density: f64,
}

/// One point on the exact sealed-truth grade-tonnage curve.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GradeTonnagePoint {
    pub cutoff_g_per_t: f64,
    pub tonnes: f64,
    pub mean_grade_g_per_t: f64,
    pub contained_au_grams: f64,
}

/// Complete sealed 3D geological ground truth for `BD-128`.
#[derive(Debug, Clone, PartialEq)]
pub struct OpenPitAuGeologyTruth {
    pub world_id: String,
    pub seed_root: u64,
    pub blocks: Vec<GeologyBlock>,
    pub domain_stats: [DomainStats; 3],
    pub grade_tonnage: Vec<GradeTonnagePoint>,
}

impl OpenPitAuGeologyTruth {
    pub(crate) fn canonical_bytes(&self) -> Vec<u8> {
        let mut enc = Encoder::new_frame(*b"GEO3", 1);
        enc.put_str(&self.world_id);
        enc.put_u64(self.seed_root);
        enc.put_u32(self.blocks.len() as u32);
        for b in &self.blocks {
            enc.put_u8(b.domain as u8);
            enc.put_u64(b.density_t_m3.to_bits());
            enc.put_u64(b.au_grade_g_per_t.to_bits());
        }
        enc.finish()
    }
}

fn draw_normal(stream: &mut PhiloxStream) -> f64 {
    let u_raw = ((stream.next_u64() >> 11) as f64) * (1.0 / ((1u64 << 53) as f64));
    norm_ppf(1e-12 + u_raw * (1.0 - 2e-12))
}

/// Generate a spatially correlated 3D Open-Pit Au geological model and seal its ground truth.
pub(crate) fn generate_geology_truth(
    seed_root: u64,
    spec: &OpenPitAuGeologySpec,
    key: &SealKey,
) -> Result<SealedTruth<OpenPitAuGeologyTruth>, GenError> {
    spec.validate()?;
    let n_total = spec.nx * spec.ny * spec.nz;
    let mut g_stream = PhiloxStream::new(RngKey {
        seed_root,
        partition: 0x4745_4F33, // "GEO3"
        domain: 1,
        var_group: 1,
        realization: 0,
        brick: 0,
    });
    let mut d_stream = PhiloxStream::new(RngKey {
        seed_root,
        partition: 0x4745_4F33,
        domain: 1,
        var_group: 2,
        realization: 0,
        brick: 0,
    });

    // Draw independent standard normals on the 3D grid, then apply a 7-point 3D moving-average
    // spatial correlation stencil normalized to exact unit variance (1 / sqrt(7) on interior).
    let mut raw_z = Vec::with_capacity(n_total);
    for _ in 0..n_total {
        raw_z.push(draw_normal(&mut g_stream));
    }

    let idx = |x: usize, y: usize, z: usize| (z * spec.ny + y) * spec.nx + x;
    let vol = spec.block_dx_m * spec.block_dy_m * spec.block_dz_m;
    let mut blocks = Vec::with_capacity(n_total);

    for iz in 0..spec.nz {
        let dom_spec = if iz < spec.nz / 3 {
            spec.oxide
        } else if iz < (2 * spec.nz) / 3 {
            spec.transition
        } else {
            spec.fresh
        };
        for iy in 0..spec.ny {
            for ix in 0..spec.nx {
                let mut sum = raw_z[idx(ix, iy, iz)];
                let mut cnt = 1.0_f64;
                if ix > 0 {
                    sum += raw_z[idx(ix - 1, iy, iz)];
                    cnt += 1.0;
                }
                if ix + 1 < spec.nx {
                    sum += raw_z[idx(ix + 1, iy, iz)];
                    cnt += 1.0;
                }
                if iy > 0 {
                    sum += raw_z[idx(ix, iy - 1, iz)];
                    cnt += 1.0;
                }
                if iy + 1 < spec.ny {
                    sum += raw_z[idx(ix, iy + 1, iz)];
                    cnt += 1.0;
                }
                if iz > 0 {
                    sum += raw_z[idx(ix, iy, iz - 1)];
                    cnt += 1.0;
                }
                if iz + 1 < spec.nz {
                    sum += raw_z[idx(ix, iy, iz + 1)];
                    cnt += 1.0;
                }
                // Exact unit-variance correlated Gaussian field value.
                let z_corr = sum / sqrt(cnt);
                let au_grade_g_per_t =
                    exp(dom_spec.target_log_grade_mean + dom_spec.target_log_grade_std * z_corr);
                let density_raw = dom_spec.mean_density_t_m3
                    + dom_spec.density_std_t_m3 * draw_normal(&mut d_stream);
                let density_t_m3 = density_raw.max(1.2);
                let dry_tonnes = vol * density_t_m3;
                let contained_au_grams = dry_tonnes * au_grade_g_per_t;
                blocks.push(GeologyBlock {
                    ix,
                    iy,
                    iz,
                    domain: dom_spec.domain,
                    density_t_m3,
                    dry_tonnes,
                    au_grade_g_per_t,
                    contained_au_grams,
                });
            }
        }
    }

    let domain_stats = [
        compute_domain_stats(AuDomain::Oxide, &blocks),
        compute_domain_stats(AuDomain::Transition, &blocks),
        compute_domain_stats(AuDomain::Fresh, &blocks),
    ];

    let cutoffs = [0.0, 0.5, 1.0, 2.0, 4.0];
    let mut grade_tonnage = Vec::with_capacity(cutoffs.len());
    for c in cutoffs {
        let mut t = 0.0_f64;
        let mut m = 0.0_f64;
        for b in &blocks {
            if b.au_grade_g_per_t >= c {
                t += b.dry_tonnes;
                m += b.contained_au_grams;
            }
        }
        let g = if t > 0.0 { m / t } else { 0.0 };
        grade_tonnage.push(GradeTonnagePoint {
            cutoff_g_per_t: c,
            tonnes: t,
            mean_grade_g_per_t: g,
            contained_au_grams: m,
        });
    }

    let truth = OpenPitAuGeologyTruth {
        world_id: spec.world_id.clone(),
        seed_root,
        blocks,
        domain_stats,
        grade_tonnage,
    };
    let bytes = truth.canonical_bytes();
    Ok(SealedTruth::seal(
        &spec.world_id,
        seed_root,
        &bytes,
        key,
        truth,
    ))
}

fn compute_domain_stats(domain: AuDomain, blocks: &[GeologyBlock]) -> DomainStats {
    let mut n = 0usize;
    let mut total_tonnes = 0.0_f64;
    let mut total_au_grams = 0.0_f64;
    let mut sum_log_g = 0.0_f64;
    let mut sum_dens = 0.0_f64;
    for b in blocks {
        if b.domain == domain {
            n += 1;
            total_tonnes += b.dry_tonnes;
            total_au_grams += b.contained_au_grams;
            sum_log_g += ln(b.au_grade_g_per_t);
            sum_dens += b.density_t_m3;
        }
    }
    let nf = n.max(1) as f64;
    let mean_log = sum_log_g / nf;
    let mut var_log = 0.0_f64;
    for b in blocks {
        if b.domain == domain {
            let d = ln(b.au_grade_g_per_t) - mean_log;
            var_log += d * d;
        }
    }
    DomainStats {
        domain,
        block_count: n,
        total_tonnes,
        total_au_grams,
        mean_grade_g_per_t: if total_tonnes > 0.0 {
            total_au_grams / total_tonnes
        } else {
            0.0
        },
        realized_log_grade_mean: mean_log,
        realized_log_grade_std: sqrt(var_log / nf),
        realized_mean_density: sum_dens / nf,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_open_pit_au_geology_truth_statistics_match_spec() {
        let spec = OpenPitAuGeologySpec::standard();
        let key = SealKey::mint(128, "geology-test");
        let sealed1 = generate_geology_truth(2_026_128, &spec, &key).expect("gen 1");
        let sealed2 = generate_geology_truth(2_026_128, &spec, &key).expect("gen 2");
        assert_eq!(sealed1.truth_commitment(), sealed2.truth_commitment());

        let truth = sealed1.open(&key).expect("open with harness key");
        assert_eq!(truth.blocks.len(), 24 * 24 * 12);

        // Verify realized domain log-grade mean, log-grade std, and rock density match spec.
        let targets = [spec.oxide, spec.transition, spec.fresh];
        for (realized, target) in truth.domain_stats.iter().zip(targets.iter()) {
            assert_eq!(realized.domain, target.domain);
            assert_eq!(realized.block_count, 24 * 24 * 4);
            assert!(
                (realized.realized_log_grade_mean - target.target_log_grade_mean).abs() < 0.08,
                "{:?}: log mean {} vs target {}",
                target.domain,
                realized.realized_log_grade_mean,
                target.target_log_grade_mean
            );
            assert!(
                (realized.realized_log_grade_std - target.target_log_grade_std).abs() < 0.08,
                "{:?}: log std {} vs target {}",
                target.domain,
                realized.realized_log_grade_std,
                target.target_log_grade_std
            );
            assert!(
                (realized.realized_mean_density - target.mean_density_t_m3).abs() < 0.02,
                "{:?}: density {} vs target {}",
                target.domain,
                realized.realized_mean_density,
                target.mean_density_t_m3
            );
        }

        // Grade-tonnage curve invariants: tonnes strictly non-increasing, mean grade non-decreasing.
        for w in truth.grade_tonnage.windows(2) {
            assert!(w[1].tonnes <= w[0].tonnes);
            assert!(w[1].contained_au_grams <= w[0].contained_au_grams);
            assert!(w[1].mean_grade_g_per_t >= w[0].mean_grade_g_per_t);
        }
    }
}
