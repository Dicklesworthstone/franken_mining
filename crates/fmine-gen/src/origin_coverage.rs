//! Determinism: BitwisePortable
//! Sealed-truth empirical coverage proof and dense Cholesky cross-check for `BD-352`
//! (`fmine_claim::origin`, batch-bias variance `σ_b² + σ_ε²/n`, reprint non-narrowing).
//!
//! Runs `>= 200` (default `500`) seeded Philox replicates of a synthetic Corebox assay batch
//! campaign with batch biases `b_g ~ N(0, σ_b²)`, independent analytical errors `ε_i ~ N(0, σ_ε²)`,
//! and reprinted publication copies, verifying:
//! 1. `fmine_claim::origin::evaluate_origin_batch_model` agrees with dense `Σ = σ_ε²·I + σ_b²·Z·Zᵀ`
//!    solved via `fmine_numerics::cholesky_solve` to `< 1e-12` even on unbalanced batch sizes.
//! 2. Empirical coverage of the nominal 95% interval lies within exact Clopper–Pearson Monte Carlo
//!    error of `0.95` across `>= 200` sealed-truth replicates.
//! 3. A naive iid estimator that ignores batch bias and counts reprints as independent severely
//!    under-covers, demonstrating the exact statistical defect `BD-352` prevents.

use fmine_claim::origin::{
    BatchBiasModel, BatchGroupId, OriginId, OriginReading, OriginRelation, RelationStatus,
    evaluate_origin_batch_model,
};
use fmine_core::det::{norm_ppf, sqrt};
use fmine_core::rand::{PhiloxStream, RngKey};
use fmine_evidence::{Refusal, RefusalCode};
use fmine_numerics::cholesky_solve;

/// Summary of a sealed-truth `BD-352` coverage experiment (`>= 200` replicates).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OriginCoverageReport {
    pub replicates: usize,
    pub nominal_level: f64,
    pub origin_model_covered: usize,
    pub origin_model_coverage: f64,
    pub naive_iid_covered: usize,
    pub naive_iid_coverage: f64,
    pub max_cholesky_var_diff: f64,
    pub max_cholesky_mean_diff: f64,
}

fn draw_std_normal(stream: &mut PhiloxStream) -> f64 {
    let u = ((stream.next_u64() >> 11) as f64) * (1.0 / ((1u64 << 53) as f64));
    norm_ppf(1e-12 + u * (1.0 - 2e-12))
}

/// Dense GLS reference `(μ̂, Var(μ̂))` computed directly via `fmine_numerics::cholesky_solve` on
/// `Σ = σ_ε²·I + σ_b²·Z·Zᵀ` for cross-checking `fmine_claim::origin`.
pub fn dense_cholesky_batch_gls(
    values: &[f64],
    batch_indices: &[usize],
    sigma_b: f64,
    sigma_eps: f64,
) -> Result<(f64, f64), Refusal> {
    let n = values.len();
    if n == 0 || n != batch_indices.len() {
        return Err(Refusal::new(
            RefusalCode::OutsidePhysicalDomain109,
            "values and batch_indices must be non-empty and equal length",
        ));
    }
    let var_b = sigma_b * sigma_b;
    let var_eps = sigma_eps * sigma_eps;
    let mut sigma = vec![0.0_f64; n * n];
    for i in 0..n {
        for j in 0..n {
            let mut entry = if batch_indices[i] == batch_indices[j] {
                var_b
            } else {
                0.0
            };
            if i == j {
                entry += var_eps;
            }
            sigma[i * n + j] = entry;
        }
    }
    let ones = vec![1.0_f64; n];
    let inv_ones = cholesky_solve(&sigma, &ones, n)?;
    let inv_y = cholesky_solve(&sigma, values, n)?;
    let denom: f64 = inv_ones.iter().sum();
    let numer: f64 = inv_y.iter().sum();
    let var_mu = 1.0 / denom;
    let mu_hat = numer * var_mu;
    Ok((mu_hat, var_mu))
}

/// Execute `replicates >= 200` sealed-truth Philox draws comparing `evaluate_origin_batch_model`
/// (with `PublicationCopy` reprints) against dense `cholesky_solve` and naive iid aggregation.
pub fn run_origin_coverage_experiment(
    seed_root: u64,
    replicates: usize,
    batch_sizes: &[usize],
    reprints_per_assay: usize,
    true_mean: f64,
    model: BatchBiasModel,
) -> Result<OriginCoverageReport, Refusal> {
    model.validate()?;
    if replicates < 200 {
        return Err(Refusal::new(
            RefusalCode::OutsidePhysicalDomain109,
            format!("BD-352 coverage proof requires replicates >= 200, got {replicates}"),
        ));
    }
    if batch_sizes.is_empty() || batch_sizes.contains(&0) {
        return Err(Refusal::new(
            RefusalCode::OutsidePhysicalDomain109,
            "batch_sizes must be non-empty with every batch size >= 1",
        ));
    }

    let z_crit = norm_ppf(0.5 + 0.5 * model.nominal_level);
    let mut stream = PhiloxStream::new(RngKey {
        seed_root,
        partition: 352,
        domain: 1,
        var_group: 0,
        realization: 0,
        brick: 0,
    });

    let mut origin_covered = 0usize;
    let mut naive_covered = 0usize;
    let mut max_var_diff = 0.0_f64;
    let mut max_mean_diff = 0.0_f64;

    let reviewer = RelationStatus::Reviewed {
        reviewer_role: "qaqc-reviewer".to_string(),
    };

    for rep in 0..replicates {
        let mut readings = Vec::new();
        let mut active_vals = Vec::new();
        let mut active_batches = Vec::new();
        let mut all_row_vals = Vec::new();

        for (g_idx, &n_g) in batch_sizes.iter().enumerate() {
            let b_g = model.sigma_b * draw_std_normal(&mut stream);
            let group_id = BatchGroupId::parse(&format!("batch-{g_idx}"))?;
            for j in 0..n_g {
                let eps = model.sigma_eps * draw_std_normal(&mut stream);
                let y = true_mean + b_g + eps;
                let oid = OriginId::parse(&format!("rep{rep}-b{g_idx}-a{j}"))?;
                readings.push(OriginReading {
                    reading_id: format!("r-{rep}-{g_idx}-{j}"),
                    origin_id: oid.clone(),
                    value: y,
                    rounding_interval: None,
                    relation: OriginRelation::SharedSystematicError {
                        group_id: group_id.clone(),
                    },
                    status: reviewer.clone(),
                    vintage_rank: 1,
                });
                active_vals.push(y);
                active_batches.push(g_idx);
                all_row_vals.push(y);

                for c in 0..reprints_per_assay {
                    readings.push(OriginReading {
                        reading_id: format!("c-{rep}-{g_idx}-{j}-{c}"),
                        origin_id: OriginId::parse(&format!("rep{rep}-b{g_idx}-a{j}-copy{c}"))?,
                        value: y,
                        rounding_interval: None,
                        relation: OriginRelation::PublicationCopy {
                            of_origin: oid.clone(),
                        },
                        status: reviewer.clone(),
                        vintage_rank: 1,
                    });
                    all_row_vals.push(y);
                }
            }
        }

        let res = evaluate_origin_batch_model(&readings, model)?;
        let (lo, hi) = res.measurement_interval.ok_or_else(|| {
            Refusal::new(
                RefusalCode::NumericalCertificateFailed207,
                "unexpected missing interval on non-conflicted batch replicate",
            )
        })?;
        if true_mean >= lo && true_mean <= hi {
            origin_covered += 1;
        }

        // Cross-check the first 10 replicates against full dense Cholesky GLS.
        if rep < 10 {
            let (chol_mu, chol_var) = dense_cholesky_batch_gls(
                &active_vals,
                &active_batches,
                model.sigma_b,
                model.sigma_eps,
            )?;
            let mu_diff = (res.estimate.unwrap_or(0.0) - chol_mu).abs();
            let var_diff = (res.measurement_var.unwrap_or(0.0) - chol_var).abs();
            if mu_diff > max_mean_diff {
                max_mean_diff = mu_diff;
            }
            if var_diff > max_var_diff {
                max_var_diff = var_diff;
            }
        }

        // Naive iid estimator: ignores batch bias σ_b AND treats reprinted copies as independent.
        let n_naive = all_row_vals.len() as f64;
        let naive_mean = all_row_vals.iter().sum::<f64>() / n_naive;
        let naive_sd = model.sigma_eps / sqrt(n_naive);
        let naive_half = z_crit * naive_sd;
        if true_mean >= naive_mean - naive_half && true_mean <= naive_mean + naive_half {
            naive_covered += 1;
        }
    }

    Ok(OriginCoverageReport {
        replicates,
        nominal_level: model.nominal_level,
        origin_model_covered: origin_covered,
        origin_model_coverage: (origin_covered as f64) / (replicates as f64),
        naive_iid_covered: naive_covered,
        naive_iid_coverage: (naive_covered as f64) / (replicates as f64),
        max_cholesky_var_diff: max_var_diff,
        max_cholesky_mean_diff: max_mean_diff,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sbc::clopper_pearson_interval;

    #[test]
    fn test_bd352_sealed_truth_coverage_500_replicates_and_cholesky_parity() {
        let model = BatchBiasModel {
            sigma_b: 0.05,
            sigma_eps: 0.20,
            nominal_level: 0.95,
        };
        // Unbalanced batches (3 and 5) with 2 reprints per assay to exercise non-trivial GLS
        // weights AND reprint collapse simultaneously across 500 sealed-truth replicates.
        let report = run_origin_coverage_experiment(2_026_352, 500, &[3, 5], 2, 2.50, model)
            .expect("run 500 replicates");

        // Exact parity with dense `fmine_numerics::cholesky_solve` on unbalanced batches.
        assert!(
            report.max_cholesky_mean_diff < 1e-12,
            "mean diff vs cholesky_solve: {}",
            report.max_cholesky_mean_diff
        );
        assert!(
            report.max_cholesky_var_diff < 1e-13,
            "var diff vs cholesky_solve: {}",
            report.max_cholesky_var_diff
        );

        // Nominal 0.95 must lie inside the exact 99% Clopper-Pearson interval of empirical coverage.
        let (ci_lo, ci_hi) =
            clopper_pearson_interval(report.origin_model_covered, report.replicates, 0.01)
                .expect("clopper-pearson");
        assert!(
            (ci_lo..=ci_hi).contains(&0.95),
            "origin model empirical coverage {:.4} ({}/{}) outside 99% CP CI [{:.4}, {:.4}]",
            report.origin_model_coverage,
            report.origin_model_covered,
            report.replicates,
            ci_lo,
            ci_hi
        );

        // Naive iid ignoring batch bias & counting reprints severely under-covers (negative control).
        assert!(
            report.naive_iid_coverage < 0.80,
            "expected naive iid with reprints to severely under-cover, got {:.4}",
            report.naive_iid_coverage
        );

        // Bitwise determinism across reruns.
        let rerun =
            run_origin_coverage_experiment(2_026_352, 500, &[3, 5], 2, 2.50, model).unwrap();
        assert_eq!(report, rerun);
    }
}
