//! Determinism: BitwisePortable
//! Open-pit Cu–Au porphyry flotation world generator (`BD-129`, plan §33.3).
//!
//! Generates a sealed multi-domain Cu–Au porphyry deposit (`SupergeneOxide`, `SecondaryEnriched`,
//! `PrimaryHypogene`) with correlated CuT (%), CuAS/CuT oxide speciation, Au (g/t), S (%), and dry
//! bulk density, coupled through a geometallurgical flotation transfer model and concentrate
//! smelter-custody payable balance satisfying exact `Feed = Concentrate + Tailings` conservation.

use fmine_core::codec::Encoder;
use fmine_core::det::{exp, ln, norm_ppf, sqrt};
use fmine_core::rand::{PhiloxStream, RngKey};

use crate::GenError;
use crate::seal::{SealKey, SealedTruth};

/// Alteration / mineralization domains of the synthetic Open-Pit Cu–Au flotation deposit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CuAuDomain {
    SupergeneOxide,
    SecondaryEnriched,
    PrimaryHypogene,
}

/// Target statistical parameters for one Cu–Au domain.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CuAuDomainSpec {
    pub domain: CuAuDomain,
    /// Target mean of `ln(CuT_pct)` where `CuT_pct` is total copper in weight percent.
    pub log_cu_pct_mean: f64,
    /// Target std of `ln(CuT_pct)`.
    pub log_cu_pct_std: f64,
    /// Mean acid-soluble Cu ratio `CuAS / CuT ∈ (0, 1)` (high in oxide, low in hypogene).
    pub mean_cu_as_ratio: f64,
    /// Target mean of `ln(Au_g_per_t)`.
    pub log_au_gt_mean: f64,
    /// Target std of `ln(Au_g_per_t)`.
    pub log_au_gt_std: f64,
    /// Target mean dry rock bulk density in t/m³.
    pub mean_density_t_m3: f64,
    /// Target mean total sulphur (%).
    pub mean_sulphur_pct: f64,
}

/// Smelter / concentrate custody parameters for the Cu–Au flotation world (`BD-129`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ConcentrateCustodyTerms {
    /// Target dry copper concentrate grade (% Cu, typically 25–30%).
    pub target_conc_cu_pct: f64,
    /// Target concentrate filter-cake moisture fraction (`0.08–0.11`).
    pub conc_moisture_fraction: f64,
    /// Payable Cu fraction (e.g., `0.965` after smelter unit deduction).
    pub cu_payability: f64,
    /// Payable Au fraction in Cu concentrate (e.g., `0.970`).
    pub au_payability: f64,
}

/// Complete specification for the synthetic Open-Pit Cu–Au flotation world (`BD-129`).
#[derive(Debug, Clone, PartialEq)]
pub struct CuAuFlotationSpec {
    pub world_id: String,
    pub blocks_per_domain: usize,
    pub block_volume_m3: f64,
    pub supergene_oxide: CuAuDomainSpec,
    pub secondary_enriched: CuAuDomainSpec,
    pub primary_hypogene: CuAuDomainSpec,
    pub custody: ConcentrateCustodyTerms,
}

impl CuAuFlotationSpec {
    /// Standard benchmark Open-Pit Cu–Au porphyry flotation world (`3 × 1,200 = 3,600` blocks).
    #[must_use]
    pub fn standard() -> Self {
        Self {
            world_id: "cu-au-flotation-v1".to_string(),
            blocks_per_domain: 1_200,
            block_volume_m3: 1_000.0,
            supergene_oxide: CuAuDomainSpec {
                domain: CuAuDomain::SupergeneOxide,
                log_cu_pct_mean: -0.70, // median ~0.50% Cu
                log_cu_pct_std: 0.35,
                mean_cu_as_ratio: 0.55, // high acid-soluble oxide -> lower flotation recovery
                log_au_gt_mean: -1.10,  // median ~0.33 g/t Au
                log_au_gt_std: 0.40,
                mean_density_t_m3: 2.52,
                mean_sulphur_pct: 0.45,
            },
            secondary_enriched: CuAuDomainSpec {
                domain: CuAuDomain::SecondaryEnriched,
                log_cu_pct_mean: -0.10, // median ~0.90% Cu (chalcocite blanket)
                log_cu_pct_std: 0.30,
                mean_cu_as_ratio: 0.12,
                log_au_gt_mean: -0.65, // median ~0.52 g/t Au
                log_au_gt_std: 0.35,
                mean_density_t_m3: 2.68,
                mean_sulphur_pct: 1.85,
            },
            primary_hypogene: CuAuDomainSpec {
                domain: CuAuDomain::PrimaryHypogene,
                log_cu_pct_mean: -0.45, // median ~0.64% Cu (chalcopyrite)
                log_cu_pct_std: 0.28,
                mean_cu_as_ratio: 0.03,
                log_au_gt_mean: -0.85, // median ~0.43 g/t Au
                log_au_gt_std: 0.35,
                mean_density_t_m3: 2.76,
                mean_sulphur_pct: 2.30,
            },
            custody: ConcentrateCustodyTerms {
                target_conc_cu_pct: 27.5,
                conc_moisture_fraction: 0.095,
                cu_payability: 0.965,
                au_payability: 0.970,
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
        if self.blocks_per_domain == 0 || self.blocks_per_domain > 50_000 {
            return Err(GenError::InvalidConfig {
                field: "blocks_per_domain",
                detail: "must lie in 1..=50000".to_string(),
            });
        }
        if !(self.block_volume_m3.is_finite() && self.block_volume_m3 > 0.0) {
            return Err(GenError::InvalidConfig {
                field: "block_volume_m3",
                detail: "must be finite and > 0".to_string(),
            });
        }
        if !(10.0..=50.0).contains(&self.custody.target_conc_cu_pct)
            || !(0.0..0.30).contains(&self.custody.conc_moisture_fraction)
            || !(0.5..=1.0).contains(&self.custody.cu_payability)
            || !(0.5..=1.0).contains(&self.custody.au_payability)
        {
            return Err(GenError::InvalidConfig {
                field: "custody",
                detail: "concentrate custody parameters out of physical domain".to_string(),
            });
        }
        let check_domain = |spec: &CuAuDomainSpec, expected: CuAuDomain| -> Result<(), GenError> {
            if spec.domain != expected {
                return Err(GenError::InvalidConfig {
                    field: "domain",
                    detail: format!("domain slot expected {expected:?}, got {:?}", spec.domain),
                });
            }
            if !spec.log_cu_pct_mean.is_finite()
                || !(spec.log_cu_pct_std.is_finite() && spec.log_cu_pct_std >= 0.0)
                || !(spec.mean_cu_as_ratio.is_finite()
                    && (0.0..=1.0).contains(&spec.mean_cu_as_ratio))
                || !spec.log_au_gt_mean.is_finite()
                || !(spec.log_au_gt_std.is_finite() && spec.log_au_gt_std >= 0.0)
                || !(spec.mean_density_t_m3.is_finite()
                    && (1.0..=10.0).contains(&spec.mean_density_t_m3))
                || !(spec.mean_sulphur_pct.is_finite()
                    && (0.0..=60.0).contains(&spec.mean_sulphur_pct))
            {
                return Err(GenError::InvalidConfig {
                    field: "domain_spec",
                    detail: format!("domain {expected:?} parameters out of physical domain"),
                });
            }
            Ok(())
        };
        check_domain(&self.supergene_oxide, CuAuDomain::SupergeneOxide)?;
        check_domain(&self.secondary_enriched, CuAuDomain::SecondaryEnriched)?;
        check_domain(&self.primary_hypogene, CuAuDomain::PrimaryHypogene)?;
        Ok(())
    }
}

/// One sealed block with true geology, flotation transfer, and exact Feed = Conc + Tail balance.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CuAuBlockTruth {
    pub block_index: usize,
    pub domain: CuAuDomain,
    pub dry_tonnes: f64,
    pub density_t_m3: f64,
    pub cu_total_pct: f64,
    pub cu_as_ratio: f64,
    pub au_grade_g_per_t: f64,
    pub sulphur_pct: f64,
    pub cu_recovery: f64,
    pub au_recovery: f64,
    pub conc_dry_tonnes: f64,
    pub conc_wet_tonnes: f64,
    pub conc_cu_pct: f64,
    pub conc_au_g_per_t: f64,
    pub tail_dry_tonnes: f64,
    pub tail_cu_pct: f64,
    pub tail_au_g_per_t: f64,
    pub payable_cu_tonnes: f64,
    pub payable_au_grams: f64,
}

/// Realized summary statistics for one Cu–Au domain.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CuAuDomainStats {
    pub domain: CuAuDomain,
    pub block_count: usize,
    pub total_feed_dry_tonnes: f64,
    pub total_conc_dry_tonnes: f64,
    pub total_tail_dry_tonnes: f64,
    pub realized_log_cu_mean: f64,
    pub realized_log_cu_std: f64,
    pub realized_mean_cu_as_ratio: f64,
    pub realized_log_au_mean: f64,
    pub realized_mean_density: f64,
    pub mean_cu_recovery: f64,
    pub mean_au_recovery: f64,
    pub total_feed_cu_tonnes: f64,
    pub total_conc_cu_tonnes: f64,
    pub total_tail_cu_tonnes: f64,
    pub total_payable_cu_tonnes: f64,
    pub total_payable_au_grams: f64,
}

/// Complete sealed Cu–Au flotation world truth (`BD-129`).
#[derive(Debug, Clone, PartialEq)]
pub struct CuAuFlotationTruth {
    pub world_id: String,
    pub seed_root: u64,
    pub blocks: Vec<CuAuBlockTruth>,
    pub domain_stats: [CuAuDomainStats; 3],
}

impl CuAuFlotationTruth {
    pub(crate) fn canonical_bytes(&self) -> Vec<u8> {
        let mut enc = Encoder::new_frame(*b"CUAU", 1);
        enc.put_str(&self.world_id);
        enc.put_u64(self.seed_root);
        enc.put_u32(self.blocks.len() as u32);
        for b in &self.blocks {
            enc.put_u8(b.domain as u8);
            enc.put_u64(b.dry_tonnes.to_bits());
            enc.put_u64(b.cu_total_pct.to_bits());
            enc.put_u64(b.au_grade_g_per_t.to_bits());
            enc.put_u64(b.conc_dry_tonnes.to_bits());
        }
        enc.finish()
    }
}

fn draw_normal(stream: &mut PhiloxStream) -> f64 {
    let u = ((stream.next_u64() >> 11) as f64) * (1.0 / ((1u64 << 53) as f64));
    norm_ppf(1e-12 + u * (1.0 - 2e-12))
}

/// Generate a sealed Open-Pit Cu–Au flotation world (`BD-129`).
pub(crate) fn generate_cu_au_flotation_truth(
    seed_root: u64,
    spec: &CuAuFlotationSpec,
    key: &SealKey,
) -> Result<SealedTruth<CuAuFlotationTruth>, GenError> {
    spec.validate()?;
    let domain_specs = [
        spec.supergene_oxide,
        spec.secondary_enriched,
        spec.primary_hypogene,
    ];
    let mut blocks = Vec::with_capacity(spec.blocks_per_domain * 3);
    let mut block_index = 0usize;

    for (d_idx, dspec) in domain_specs.iter().enumerate() {
        let mut stream = PhiloxStream::new(RngKey {
            seed_root,
            partition: 0x4355_4155, // "CUAU"
            domain: (d_idx as u32) + 1,
            var_group: 1,
            realization: 0,
            brick: 0,
        });
        for _ in 0..spec.blocks_per_domain {
            // Correlated Cu-Au grades (rho = 0.65 porphyry correlation).
            let z_cu = draw_normal(&mut stream);
            let z_au_ind = draw_normal(&mut stream);
            let z_au = 0.65 * z_cu + sqrt(1.0 - 0.65 * 0.65) * z_au_ind;
            let z_dens = draw_normal(&mut stream);
            let z_as = draw_normal(&mut stream);
            let z_s = draw_normal(&mut stream);

            let cu_total_pct = exp(dspec.log_cu_pct_mean + dspec.log_cu_pct_std * z_cu);
            let au_grade_g_per_t = exp(dspec.log_au_gt_mean + dspec.log_au_gt_std * z_au);
            let density_t_m3 = (dspec.mean_density_t_m3 + 0.06 * z_dens).max(1.5);
            let cu_as_ratio = (dspec.mean_cu_as_ratio + 0.03 * z_as).clamp(0.005, 0.92);
            let sulphur_pct = (dspec.mean_sulphur_pct + 0.15 * z_s).max(0.05);

            // Geometallurgical flotation recovery transfer (plan §17.3 / §33.3):
            // sulphide Cu floats at ~93%, acid-soluble oxide Cu floats poorly (~22%).
            let cu_sulphide_frac = 1.0 - cu_as_ratio;
            let cu_recovery = (0.93 * cu_sulphide_frac + 0.22 * cu_as_ratio).clamp(0.15, 0.96);
            // Au floats preferentially with sulphide minerals (chalcopyrite/pyrite association).
            let s_factor = (sulphur_pct / (sulphur_pct + 0.40)).clamp(0.35, 0.95);
            let au_recovery = (0.86 * s_factor * (1.0 - 0.35 * cu_as_ratio)).clamp(0.25, 0.90);

            let dry_tonnes = spec.block_volume_m3 * density_t_m3;
            let feed_cu_tonnes = dry_tonnes * (cu_total_pct / 100.0);
            let feed_au_grams = dry_tonnes * au_grade_g_per_t;

            // Concentrate grade must strictly exceed feed Cu grade so mass pull < 1.
            let conc_cu_pct = spec.custody.target_conc_cu_pct.max(cu_total_pct * 3.0);
            let conc_cu_tonnes = feed_cu_tonnes * cu_recovery;
            let conc_dry_tonnes = conc_cu_tonnes / (conc_cu_pct / 100.0);
            let conc_wet_tonnes = conc_dry_tonnes / (1.0 - spec.custody.conc_moisture_fraction);
            let conc_au_grams = feed_au_grams * au_recovery;
            let conc_au_g_per_t = conc_au_grams / conc_dry_tonnes;

            // Exact extensive conservation: Tail = Feed - Concentrate.
            let tail_dry_tonnes = dry_tonnes - conc_dry_tonnes;
            let tail_cu_tonnes = feed_cu_tonnes - conc_cu_tonnes;
            let tail_au_grams = feed_au_grams - conc_au_grams;
            let tail_cu_pct = (tail_cu_tonnes / tail_dry_tonnes) * 100.0;
            let tail_au_g_per_t = tail_au_grams / tail_dry_tonnes;

            blocks.push(CuAuBlockTruth {
                block_index,
                domain: dspec.domain,
                dry_tonnes,
                density_t_m3,
                cu_total_pct,
                cu_as_ratio,
                au_grade_g_per_t,
                sulphur_pct,
                cu_recovery,
                au_recovery,
                conc_dry_tonnes,
                conc_wet_tonnes,
                conc_cu_pct,
                conc_au_g_per_t,
                tail_dry_tonnes,
                tail_cu_pct,
                tail_au_g_per_t,
                payable_cu_tonnes: conc_cu_tonnes * spec.custody.cu_payability,
                payable_au_grams: conc_au_grams * spec.custody.au_payability,
            });
            block_index += 1;
        }
    }

    let domain_stats = [
        summarize_domain(CuAuDomain::SupergeneOxide, &blocks),
        summarize_domain(CuAuDomain::SecondaryEnriched, &blocks),
        summarize_domain(CuAuDomain::PrimaryHypogene, &blocks),
    ];

    let truth = CuAuFlotationTruth {
        world_id: spec.world_id.clone(),
        seed_root,
        blocks,
        domain_stats,
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

fn summarize_domain(domain: CuAuDomain, blocks: &[CuAuBlockTruth]) -> CuAuDomainStats {
    let mut n = 0usize;
    let mut feed_t = 0.0_f64;
    let mut conc_t = 0.0_f64;
    let mut tail_t = 0.0_f64;
    let mut sum_log_cu = 0.0_f64;
    let mut sum_as = 0.0_f64;
    let mut sum_log_au = 0.0_f64;
    let mut sum_dens = 0.0_f64;
    let mut feed_cu = 0.0_f64;
    let mut conc_cu = 0.0_f64;
    let mut tail_cu = 0.0_f64;
    let mut feed_au = 0.0_f64;
    let mut conc_au = 0.0_f64;
    let mut pay_cu = 0.0_f64;
    let mut pay_au = 0.0_f64;

    for b in blocks {
        if b.domain == domain {
            n += 1;
            feed_t += b.dry_tonnes;
            conc_t += b.conc_dry_tonnes;
            tail_t += b.tail_dry_tonnes;
            sum_log_cu += ln(b.cu_total_pct);
            sum_as += b.cu_as_ratio;
            sum_log_au += ln(b.au_grade_g_per_t);
            sum_dens += b.density_t_m3;
            feed_cu += b.dry_tonnes * (b.cu_total_pct / 100.0);
            conc_cu += b.conc_dry_tonnes * (b.conc_cu_pct / 100.0);
            tail_cu += b.tail_dry_tonnes * (b.tail_cu_pct / 100.0);
            feed_au += b.dry_tonnes * b.au_grade_g_per_t;
            conc_au += b.conc_dry_tonnes * b.conc_au_g_per_t;
            pay_cu += b.payable_cu_tonnes;
            pay_au += b.payable_au_grams;
        }
    }
    let nf = n.max(1) as f64;
    let mean_log_cu = sum_log_cu / nf;
    let mut var_log_cu = 0.0_f64;
    for b in blocks {
        if b.domain == domain {
            let d = ln(b.cu_total_pct) - mean_log_cu;
            var_log_cu += d * d;
        }
    }
    CuAuDomainStats {
        domain,
        block_count: n,
        total_feed_dry_tonnes: feed_t,
        total_conc_dry_tonnes: conc_t,
        total_tail_dry_tonnes: tail_t,
        realized_log_cu_mean: mean_log_cu,
        realized_log_cu_std: sqrt(var_log_cu / nf),
        realized_mean_cu_as_ratio: sum_as / nf,
        realized_log_au_mean: sum_log_au / nf,
        realized_mean_density: sum_dens / nf,
        mean_cu_recovery: if feed_cu > 0.0 {
            conc_cu / feed_cu
        } else {
            0.0
        },
        mean_au_recovery: if feed_au > 0.0 {
            conc_au / feed_au
        } else {
            0.0
        },
        total_feed_cu_tonnes: feed_cu,
        total_conc_cu_tonnes: conc_cu,
        total_tail_cu_tonnes: tail_cu,
        total_payable_cu_tonnes: pay_cu,
        total_payable_au_grams: pay_au,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::harness::ScoringHarness;

    #[test]
    fn test_cu_au_flotation_world_truth_statistics_and_conservation_match_spec() {
        let spec = CuAuFlotationSpec::standard();
        let harness = ScoringHarness::new(2_026, "cu-au-test");
        let s1 = harness
            .generate_cu_au_flotation(2_026_129, &spec)
            .expect("gen 1");
        let s2 = harness
            .generate_cu_au_flotation(2_026_129, &spec)
            .expect("gen 2");
        assert_eq!(s1.truth_commitment(), s2.truth_commitment());

        let truth = harness.open_cu_au_truth(&s1).expect("open truth");
        assert_eq!(truth.blocks.len(), 3_600);

        let targets = [
            spec.supergene_oxide,
            spec.secondary_enriched,
            spec.primary_hypogene,
        ];
        for (st, tgt) in truth.domain_stats.iter().zip(targets.iter()) {
            assert_eq!(st.domain, tgt.domain);
            assert_eq!(st.block_count, 1_200);
            assert!(
                (st.realized_log_cu_mean - tgt.log_cu_pct_mean).abs() < 0.04,
                "{:?}: log Cu mean {} vs {}",
                tgt.domain,
                st.realized_log_cu_mean,
                tgt.log_cu_pct_mean
            );
            assert!(
                (st.realized_log_cu_std - tgt.log_cu_pct_std).abs() < 0.04,
                "{:?}: log Cu std {} vs {}",
                tgt.domain,
                st.realized_log_cu_std,
                tgt.log_cu_pct_std
            );
            assert!(
                (st.realized_mean_cu_as_ratio - tgt.mean_cu_as_ratio).abs() < 0.01,
                "{:?}: CuAS/CuT {} vs {}",
                tgt.domain,
                st.realized_mean_cu_as_ratio,
                tgt.mean_cu_as_ratio
            );
            assert!(
                (st.realized_log_au_mean - tgt.log_au_gt_mean).abs() < 0.04,
                "{:?}: log Au mean {} vs {}",
                tgt.domain,
                st.realized_log_au_mean,
                tgt.log_au_gt_mean
            );
            assert!(
                (st.realized_mean_density - tgt.mean_density_t_m3).abs() < 0.015,
                "{:?}: density {} vs {}",
                tgt.domain,
                st.realized_mean_density,
                tgt.mean_density_t_m3
            );
            // Exact Feed = Concentrate + Tailings mass and Cu metal conservation.
            let mass_err = (st.total_feed_dry_tonnes
                - (st.total_conc_dry_tonnes + st.total_tail_dry_tonnes))
                .abs()
                / st.total_feed_dry_tonnes;
            let cu_err = (st.total_feed_cu_tonnes
                - (st.total_conc_cu_tonnes + st.total_tail_cu_tonnes))
                .abs()
                / st.total_feed_cu_tonnes;
            assert!(mass_err < 1e-12, "mass conservation error {mass_err}");
            assert!(cu_err < 1e-12, "Cu conservation error {cu_err}");
        }

        // Geometallurgical physics: hypogene sulphide Cu recovery strictly exceeds supergene oxide.
        assert!(
            truth.domain_stats[2].mean_cu_recovery > truth.domain_stats[0].mean_cu_recovery + 0.30
        );
    }
}
