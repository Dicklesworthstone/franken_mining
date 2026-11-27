//! Lodestar Bayesian inference engine (`BD-232`, plan §27.3, §28.3, §39, `FD-FJ-00`, `PO-TOUCH-4`).
//! Determinism: BitwisePortable
//!
//! Provides:
//! - [`ad`]: `BitwisePortable` reverse-mode automatic differentiation (`Tape` / `Var`) and
//!   [`LogDensity`] trait verified against pinned `frankenjax@fa93fb77` (`fj-core`, `fj-lax`,
//!   `fj-ad`) gradient fixtures (`GATE-ORACLE-01`).
//! - [`bijector`]: smooth unconstrained-to-constrained parameter bijectors with exact
//!   log-abs-det-Jacobians.
//! - [`nuts`]: `BitwisePortable` No-U-Turn Sampler (Hoffman & Gelman 2014 + Betancourt 2017
//!   multinomial trajectory sampling) with dual-averaging step-size adaptation, Stan 3-stage
//!   windowed diagonal mass-matrix adaptation, and `Philox4x32-10` RNG streams (`BD-020`).
//! - [`diagnostics`]: Vehtari et al. (2021) rank-normalized split-$\hat{R}$ (bulk + folded),
//!   Bulk ESS, Tail ESS, E-BFMI, and fail-closed [`RhatEssCertificate`] (`PO-TOUCH-4`).

pub mod ad;
pub mod bijector;
pub mod diagnostics;
pub mod nuts;

use std::collections::BTreeSet;

use fmine_core::hash::sha256;
use fmine_evidence::{Refusal, RefusalCode};

pub use ad::{LogDensity, Real, Tape, TapeLogDensity, Var};
pub use bijector::Bijector;
pub use diagnostics::{
    ChainRuntimeStats, DiagnosticFailure, DiagnosticThresholds, DiagnosticVerdict,
    ParameterDiagnostics, RhatEssCertificate, certify_chains, compute_ebfmi,
    compute_parameter_diagnostics,
};
pub use nuts::{NutsConfig, NutsResult, sample_nuts};

/// Exact pinned `frankenjax` Git revision recorded in `franken-stack.lock` (`FD-FJ-00`).
pub const FJ_PIN_REV: &str = "fa93fb775e1cd73471cd9b2b3e3a014762d1066c";

/// Exact SHA-256 digest of `fixtures/fmine-infer/fj_ad_gradients.txt` captured from
/// `frankenjax@fa93fb77` (`fj-core`, `fj-lax`, `fj-ad`) on `nightly-2026-08-31` (`GATE-ORACLE-01`).
pub const FJ_ORACLE_FIXTURE_SHA256: &str =
    "866f0b911310d1046dbe56cfff96ff520de407d71e9879a40c47ea447db91b4b";

/// One parsed row from `fixtures/fmine-infer/fj_ad_gradients.txt`.
#[derive(Debug, Clone, PartialEq)]
pub struct FjGradOracleRow {
    pub case_id: String,
    pub model: String,
    pub point: Vec<f64>,
    pub log_density: f64,
    pub grad: Vec<f64>,
}

fn parse_csv_f64(raw: &str, line_no: usize, field: &str) -> Result<Vec<f64>, Refusal> {
    let mut out = Vec::new();
    for tok in raw.split(',') {
        let v: f64 = tok.parse().map_err(|_| {
            Refusal::new(
                RefusalCode::DigestMismatch505,
                format!("fj_ad_gradients.txt line {line_no}: invalid {field} token {tok:?}"),
            )
        })?;
        if !v.is_finite() {
            return Err(Refusal::new(
                RefusalCode::OutsidePhysicalDomain109,
                format!("fj_ad_gradients.txt line {line_no}: non-finite {field} value"),
            ));
        }
        out.push(if v == 0.0 { 0.0 } else { v });
    }
    if out.is_empty() {
        return Err(Refusal::new(
            RefusalCode::DigestMismatch505,
            format!("fj_ad_gradients.txt line {line_no}: empty {field} vector"),
        ));
    }
    Ok(out)
}

/// Parse `fixtures/fmine-infer/fj_ad_gradients.txt`, refusing malformed, empty, or duplicate rows.
pub fn parse_fj_oracle_fixture(text: &str) -> Result<Vec<FjGradOracleRow>, Refusal> {
    let mut rows = Vec::new();
    let mut seen = BTreeSet::new();
    for (idx, raw_line) in text.lines().enumerate() {
        let line = raw_line.trim_end_matches('\r');
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let cols: Vec<&str> = line.split('\t').collect();
        if cols.len() != 6 || cols[0] != "GRAD" {
            return Err(Refusal::new(
                RefusalCode::DigestMismatch505,
                format!(
                    "fj_ad_gradients.txt line {}: expected 6 tab-separated GRAD columns, got {}",
                    idx + 1,
                    cols.len()
                ),
            ));
        }
        if cols[1..6].iter().any(|c| c.trim().is_empty()) {
            return Err(Refusal::new(
                RefusalCode::DigestMismatch505,
                format!("fj_ad_gradients.txt line {}: empty field", idx + 1),
            ));
        }
        let point = parse_csv_f64(cols[3], idx + 1, "point")?;
        let log_density: f64 = cols[4].parse().map_err(|_| {
            Refusal::new(
                RefusalCode::DigestMismatch505,
                format!(
                    "fj_ad_gradients.txt line {}: invalid log_density {:?}",
                    idx + 1,
                    cols[4]
                ),
            )
        })?;
        if !log_density.is_finite() {
            return Err(Refusal::new(
                RefusalCode::OutsidePhysicalDomain109,
                format!(
                    "fj_ad_gradients.txt line {}: non-finite log_density",
                    idx + 1
                ),
            ));
        }
        let grad = parse_csv_f64(cols[5], idx + 1, "grad")?;
        if grad.len() != point.len() {
            return Err(Refusal::new(
                RefusalCode::DigestMismatch505,
                format!(
                    "fj_ad_gradients.txt line {}: point dim {} != grad dim {}",
                    idx + 1,
                    point.len(),
                    grad.len()
                ),
            ));
        }
        if !seen.insert(cols[1].to_string()) {
            return Err(Refusal::new(
                RefusalCode::DigestMismatch505,
                format!(
                    "fj_ad_gradients.txt line {}: duplicate case_id {:?}",
                    idx + 1,
                    cols[1]
                ),
            ));
        }
        rows.push(FjGradOracleRow {
            case_id: cols[1].to_string(),
            model: cols[2].to_string(),
            point,
            log_density,
            grad,
        });
    }
    if rows.is_empty() {
        return Err(Refusal::new(
            RefusalCode::DigestMismatch505,
            "fj_ad_gradients.txt contains no GRAD rows",
        ));
    }
    Ok(rows)
}

/// Verify that `fixture_text` matches [`FJ_ORACLE_FIXTURE_SHA256`] and parses cleanly.
pub fn verify_fj_oracle_fixture(fixture_text: &str) -> Result<Vec<FjGradOracleRow>, Refusal> {
    let actual = sha256(fixture_text.as_bytes()).to_hex();
    if actual != FJ_ORACLE_FIXTURE_SHA256 {
        return Err(Refusal::new(
            RefusalCode::DigestMismatch505,
            format!(
                "fj-ad oracle fixture sha256 mismatch: expected {FJ_ORACLE_FIXTURE_SHA256}, got {actual}"
            ),
        ));
    }
    parse_fj_oracle_fixture(fixture_text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use fmine_core::det;
    use fmine_core::rand::{PhiloxStream, RngKey};

    const FJ_ORACLE_TEXT: &str = include_str!("../../../fixtures/fmine-infer/fj_ad_gradients.txt");

    fn build_normal_2d_corr_model()
    -> TapeLogDensity<impl Fn(&mut Tape, &[Var]) -> Result<Var, Refusal>> {
        TapeLogDensity::new(vec!["x0".to_string(), "x1".to_string()], |t, v| {
            let mu0 = t.constant(Real(1.5))?;
            let mu1 = t.constant(Real(-0.5))?;
            let inv_s0 = t.constant(Real(0.5))?;
            let inv_s1 = t.constant(Real(1.25))?;
            let dx0 = t.sub(v[0], mu0)?;
            let dx1 = t.sub(v[1], mu1)?;
            let z0 = t.mul(dx0, inv_s0)?;
            let z1 = t.mul(dx1, inv_s1)?;
            let z0_sq = t.square(z0)?;
            let z1_sq = t.square(z1)?;
            let z0_z1 = t.mul(z0, z1)?;
            let two_rho = t.constant(Real(1.4))?;
            let cross = t.mul(two_rho, z0_z1)?;
            let sum_sq = t.add(z0_sq, z1_sq)?;
            let quad = t.sub(sum_sq, cross)?;
            let coeff = t.constant(Real(-0.5 / 0.51))?;
            t.mul(coeff, quad)
        })
        .expect("normal_2d_corr")
    }

    fn build_beta_logit_model() -> TapeLogDensity<impl Fn(&mut Tape, &[Var]) -> Result<Var, Refusal>>
    {
        // Beta(alpha=12, beta=5) in unconstrained logit space z with Bijector::UnitInterval.
        TapeLogDensity::new(vec!["logit_p".to_string()], |t, v| {
            let (_p, ladj) = Bijector::UnitInterval.forward_var(t, v[0])?;
            // (alpha - 1) * ln(p) + (beta - 1) * ln(1 - p) + ladj
            let ls = t.log_sigmoid(v[0])?;
            let l1ms = t.log_one_minus_sigmoid(v[0])?;
            let a_m1 = t.constant(Real(11.0))?;
            let b_m1 = t.constant(Real(4.0))?;
            let term1 = t.mul(a_m1, ls)?;
            let term2 = t.mul(b_m1, l1ms)?;
            let sum = t.add(term1, term2)?;
            t.add(sum, ladj)
        })
        .expect("beta_logit")
    }

    fn build_gamma_pos_model() -> TapeLogDensity<impl Fn(&mut Tape, &[Var]) -> Result<Var, Refusal>>
    {
        // Gamma(k=4.5, lambda=1.2) in log space z = ln(theta) with Bijector::Positive.
        TapeLogDensity::new(vec!["log_theta".to_string()], |t, v| {
            let (theta, ladj) = Bijector::Positive.forward_var(t, v[0])?;
            let k_m1 = t.constant(Real(3.5))?;
            let lam = t.constant(Real(1.2))?;
            let term1 = t.mul(k_m1, v[0])?;
            let term2 = t.mul(lam, theta)?;
            let log_lik = t.sub(term1, term2)?;
            t.add(log_lik, ladj)
        })
        .expect("gamma_pos")
    }

    fn build_hierarchical_ncp_model()
    -> TapeLogDensity<impl Fn(&mut Tape, &[Var]) -> Result<Var, Refusal>> {
        TapeLogDensity::new(
            vec![
                "mu".to_string(),
                "log_tau".to_string(),
                "eta0".to_string(),
                "eta1".to_string(),
            ],
            |t, v| {
                let mu = v[0];
                let log_tau = v[1];
                let eta0 = v[2];
                let eta1 = v[3];

                let tau = t.exp_op(log_tau)?;
                let sh0 = t.mul(tau, eta0)?;
                let sh1 = t.mul(tau, eta1)?;
                let th0 = t.add(mu, sh0)?;
                let th1 = t.add(mu, sh1)?;

                let y0 = t.constant(Real(1.8))?;
                let y1 = t.constant(Real(0.9))?;
                let r0 = t.sub(y0, th0)?;
                let r1 = t.sub(y1, th1)?;
                let r0_sq = t.square(r0)?;
                let r1_sq = t.square(r1)?;
                let r_sum = t.add(r0_sq, r1_sq)?;
                let lik_scale = t.constant(Real(-2.0))?;
                let lik = t.mul(lik_scale, r_sum)?;

                let mu_sq = t.square(mu)?;
                let mu_scale = t.constant(Real(-0.125))?;
                let p_mu = t.mul(mu_scale, mu_sq)?;

                let lt_sq = t.square(log_tau)?;
                let e0_sq = t.square(eta0)?;
                let e1_sq = t.square(eta1)?;
                let s1 = t.add(lt_sq, e0_sq)?;
                let s2 = t.add(s1, e1_sq)?;
                let half = t.constant(Real(-0.5))?;
                let p_std = t.mul(half, s2)?;

                let p_tot = t.add(p_mu, p_std)?;
                t.add(lik, p_tot)
            },
        )
        .expect("hier_ncp")
    }

    fn build_student_t_model() -> TapeLogDensity<impl Fn(&mut Tape, &[Var]) -> Result<Var, Refusal>>
    {
        TapeLogDensity::new(vec!["x".to_string()], |t, v| {
            let loc = t.constant(Real(1.5))?;
            let dx = t.sub(v[0], loc)?;
            let dx2 = t.square(dx)?;
            let inv_scale = t.constant(Real(1.0 / 1.44))?;
            let scaled = t.mul(dx2, inv_scale)?;
            let one = t.constant(Real(1.0))?;
            let inner = t.add(one, scaled)?;
            let l = t.ln_op(inner)?;
            let coeff = t.constant(Real(-2.5))?;
            t.mul(coeff, l)
        })
        .expect("student_t")
    }

    #[test]
    fn test_fj_ad_oracle_fixture_parity_and_tamper_refusal() {
        let rows = verify_fj_oracle_fixture(FJ_ORACLE_TEXT)
            .expect("pinned frankenjax@fa93fb77 fixture must verify");
        assert_eq!(rows.len(), 10);

        let m_norm = build_normal_2d_corr_model();
        let m_beta = build_beta_logit_model();
        let m_gamma = build_gamma_pos_model();
        let m_hier = build_hierarchical_ncp_model();
        let m_st = build_student_t_model();

        for row in &rows {
            let (val, grad) = match row.model.as_str() {
                "normal_2d_corr" => m_norm.log_density_and_grad(&row.point).expect("norm"),
                "beta_logit_transformed" => m_beta.log_density_and_grad(&row.point).expect("beta"),
                "gamma_positive_transformed" => {
                    m_gamma.log_density_and_grad(&row.point).expect("gamma")
                }
                "hierarchical_ncp_4d" => m_hier.log_density_and_grad(&row.point).expect("hier"),
                "student_t_robust" => m_st.log_density_and_grad(&row.point).expect("st"),
                other => panic!("unexpected model in oracle fixture: {other}"),
            };

            let v_tol = 1e-13 * row.log_density.abs().max(1.0);
            assert!(
                (val - row.log_density).abs() <= v_tol,
                "{}: log_density got {val:.16e}, fj-ad oracle {:.16e}",
                row.case_id,
                row.log_density
            );
            assert_eq!(grad.len(), row.grad.len());
            for (i, (&got, &want)) in grad.iter().zip(row.grad.iter()).enumerate() {
                let g_tol = 1e-13 * want.abs().max(1.0);
                assert!(
                    (got - want).abs() <= g_tol,
                    "{} grad[{i}]: got {got:.16e}, fj-ad oracle {want:.16e}",
                    row.case_id
                );
            }
        }

        // Planted negative: tampering one digit refuses with FMINE-REF-505.
        let tampered = FJ_ORACLE_TEXT.replacen("student_t_p1", "student_t_x1", 1);
        assert_eq!(
            verify_fj_oracle_fixture(&tampered).unwrap_err().code,
            RefusalCode::DigestMismatch505.as_str()
        );
    }

    #[test]
    fn test_tape_ad_matches_central_finite_differences() {
        let model = TapeLogDensity::new(vec!["a".to_string(), "b".to_string()], |t, v| {
            let ea = t.exp_op(v[0])?;
            let sqb = t.square(v[1])?;
            let one = t.constant(Real(1.0))?;
            let pos = t.add(one, sqb)?;
            let rt = t.sqrt(pos)?;
            let ratio = t.div(ea, rt)?;
            let th = t.tanh_op(v[1])?;
            let sp = t.softplus(v[0])?;
            let neg_sp = t.neg(sp)?;
            let s1 = t.add(ratio, th)?;
            t.add(s1, neg_sp)
        })
        .expect("composite");

        let pt = [0.65, -0.45];
        let (_val, grad) = model.log_density_and_grad(&pt).expect("ad");
        let h = 1e-6;
        for i in 0..2 {
            let mut p_plus = pt;
            let mut p_minus = pt;
            p_plus[i] += h;
            p_minus[i] -= h;
            let (vp, _) = model.log_density_and_grad(&p_plus).expect("p+");
            let (vm, _) = model.log_density_and_grad(&p_minus).expect("p-");
            let fd = (vp - vm) / (2.0 * h);
            assert!(
                (grad[i] - fd).abs() <= 1e-7 * grad[i].abs().max(1.0),
                "dim {i}: ad {} vs fd {fd}",
                grad[i]
            );
        }

        // Domain refusals fail closed.
        let mut t = Tape::default();
        let zero = t.input(Real(0.0)).expect("0");
        let one = t.input(Real(1.0)).expect("1");
        assert_eq!(
            t.div(one, zero).unwrap_err().code,
            RefusalCode::OutsidePhysicalDomain109.as_str()
        );
        assert_eq!(
            t.ln_op(zero).unwrap_err().code,
            RefusalCode::OutsidePhysicalDomain109.as_str()
        );
        assert_eq!(
            t.sqrt(zero).unwrap_err().code,
            RefusalCode::OutsidePhysicalDomain109.as_str()
        );
    }

    #[test]
    fn test_bijectors_roundtrip_jacobian_and_refusals() {
        let bijectors = [
            Bijector::Identity,
            Bijector::Positive,
            Bijector::UnitInterval,
            Bijector::LowerBounded { lower: Real(-2.5) },
            Bijector::Bounded {
                lower: Real(0.1),
                upper: Real(3.5),
            },
        ];
        let h = 1e-6;
        for b in bijectors {
            for z in [-2.5, -0.3, 0.0, 0.85, 2.2] {
                let x = b.forward(Real(z)).expect("fwd");
                let z_back = b.inverse(x).expect("inv");
                assert!(
                    (z_back.get() - z).abs() < 1e-12,
                    "{b:?} roundtrip failed at z={z}: got {}",
                    z_back.get()
                );

                // Check log|dx/dz| against finite difference.
                let xp = b.forward(Real(z + h)).expect("fwd+").get();
                let xm = b.forward(Real(z - h)).expect("fwd-").get();
                let fd_ladj = det::ln(((xp - xm) / (2.0 * h)).abs());
                let analytic_ladj = b.log_det_jacobian(Real(z)).expect("ladj").get();
                assert!(
                    (analytic_ladj - fd_ladj).abs() < 1e-7,
                    "{b:?} ladj at z={z}: analytic {analytic_ladj} vs fd {fd_ladj}"
                );

                // Check Tape forward_var matches scalar forward and log_det_jacobian.
                let mut tape = Tape::default();
                let zv = tape.input(Real(z)).expect("zv");
                let (xv, lv) = b.forward_var(&mut tape, zv).expect("fwd_var");
                assert!((tape.value(xv) - x.get()).abs() < 1e-14);
                assert!((tape.value(lv) - analytic_ladj).abs() < 1e-14);
            }
        }

        // Extreme logit values stay finite (no NaN/Inf overflow).
        let extreme = Bijector::UnitInterval
            .log_det_jacobian(Real(50.0))
            .expect("extreme ladj");
        assert!(extreme.get().is_finite() && (extreme.get() + 50.0).abs() < 1e-12);

        // Invalid bounds and out-of-domain inverses refuse with FMINE-REF-109.
        assert_eq!(
            Bijector::Bounded {
                lower: Real(2.0),
                upper: Real(2.0),
            }
            .validate()
            .unwrap_err()
            .code,
            RefusalCode::OutsidePhysicalDomain109.as_str()
        );
        assert_eq!(
            Bijector::Positive.inverse(Real(0.0)).unwrap_err().code,
            RefusalCode::OutsidePhysicalDomain109.as_str()
        );
        assert_eq!(
            Bijector::UnitInterval.inverse(Real(1.0)).unwrap_err().code,
            RefusalCode::OutsidePhysicalDomain109.as_str()
        );
    }

    #[test]
    fn test_diagnostics_ar1_known_answer_and_iid_certification() {
        // 1. AR(1) known answer: x_{t} = phi * x_{t-1} + sqrt(1 - phi^2) * eps_t with phi = 0.5
        // has exact asymptotic integrated autocorrelation time tau = (1 + phi)/(1 - phi) = 3.0,
        // so ESS_raw over 4 chains * 1,000 draws = 4,000 should be ~1,333.
        let phi = 0.5_f64;
        let noise_sd = det::sqrt(1.0 - phi * phi);
        let mut ar_chains = Vec::new();
        for c in 0..4u32 {
            let mut rng = PhiloxStream::new(RngKey {
                seed_root: 0xAA11_2233,
                partition: 1,
                domain: 0,
                var_group: 0,
                realization: c,
                brick: 0,
            });
            let mut x = rng.next_normal();
            let mut chain = Vec::with_capacity(1000);
            for _ in 0..1000 {
                x = phi * x + noise_sd * rng.next_normal();
                chain.push(x);
            }
            ar_chains.push(chain);
        }
        let diag = compute_parameter_diagnostics("ar1", &ar_chains);
        let rhat = diag.split_rhat.expect("ar1 rhat");
        assert!(rhat < 1.015, "ar1 split_rhat={rhat}");
        // Expected ESS ~ 4000 / 3 = 1333; allow generous finite-sample margin [900, 1900].
        assert!(
            diag.raw_ess > 900.0 && diag.raw_ess < 1900.0,
            "AR(1) phi=0.5 raw_ess={} outside expected ~1333",
            diag.raw_ess
        );

        // 2. Pure i.i.d. N(0, 1) draws across 4 chains * 500 draws -> R-hat < 1.01, ESS ~ 2000.
        let mut iid_chains = Vec::new();
        let mut stats = Vec::new();
        for c in 0..4u32 {
            let mut rng = PhiloxStream::new(RngKey {
                seed_root: 0xBB44_5566,
                partition: 2,
                domain: 0,
                var_group: 0,
                realization: c,
                brick: 0,
            });
            let mut chain = Vec::with_capacity(500);
            let mut energies = Vec::with_capacity(500);
            for _ in 0..500 {
                let x = rng.next_normal();
                let p = rng.next_normal();
                chain.push(vec![x]);
                energies.push(0.5 * x * x + 0.5 * p * p);
            }
            iid_chains.push(chain);
            stats.push(ChainRuntimeStats {
                chain_id: c as usize,
                adapted_step_size: 1.0,
                post_warmup_divergences: 0,
                max_treedepth_hits: 0,
                mean_accept_prob: 0.9,
                post_warmup_energies: energies,
            });
        }
        let cert = certify_chains(
            &iid_chains,
            &["z".to_string()],
            &stats,
            &DiagnosticThresholds::default(),
        )
        .expect("cert");
        cert.require_certified()
            .expect("iid chains must certify cleanly");
        assert!(cert.parameters[0].bulk_ess > 1400.0);
        assert!(cert.parameters[0].tail_ess > 1200.0);
    }

    #[test]
    fn test_nuts_conjugate_multivariate_normal_certified() {
        // 4D target with heterogeneous scales (0.3, 1.5, 4.0, 0.8) and (x0, x1) correlation rho=0.7.
        // Tests mass-matrix adaptation across a >13x scale spread + PO-TOUCH-4 certification.
        struct HeteroNormal4D;
        impl LogDensity for HeteroNormal4D {
            fn dim(&self) -> usize {
                4
            }
            fn param_names(&self) -> Vec<String> {
                vec![
                    "x0".to_string(),
                    "x1".to_string(),
                    "x2".to_string(),
                    "x3".to_string(),
                ]
            }
            fn log_density_and_grad(&self, z: &[f64]) -> Result<(f64, Vec<f64>), Refusal> {
                let mu = [1.2, -0.8, 3.5, 0.4];
                let sd = [0.3, 1.5, 4.0, 0.8];
                let rho = 0.7_f64;
                let omr2 = 1.0 - rho * rho;

                let u0 = (z[0] - mu[0]) / sd[0];
                let u1 = (z[1] - mu[1]) / sd[1];
                let u2 = (z[2] - mu[2]) / sd[2];
                let u3 = (z[3] - mu[3]) / sd[3];

                let lp01 = -0.5 / omr2 * (u0 * u0 - 2.0 * rho * u0 * u1 + u1 * u1);
                let lp23 = -0.5 * (u2 * u2 + u3 * u3);
                let g0 = -(u0 - rho * u1) / (omr2 * sd[0]);
                let g1 = -(u1 - rho * u0) / (omr2 * sd[1]);
                let g2 = -u2 / sd[2];
                let g3 = -u3 / sd[3];
                Ok((lp01 + lp23, vec![g0, g1, g2, g3]))
            }
        }

        let inits = vec![
            vec![0.0, 0.0, 0.0, 0.0],
            vec![2.0, -2.0, 6.0, -1.0],
            vec![0.5, 1.0, -1.0, 2.0],
            vec![1.8, -1.5, 5.0, 0.0],
        ];
        let cfg = NutsConfig {
            seed_root: 0x2026_0232,
            num_warmup: 400,
            num_draws: 500,
            ..NutsConfig::default()
        };
        let res = sample_nuts(&HeteroNormal4D, &inits, &cfg).expect("nuts 4d");
        res.certificate
            .require_certified()
            .expect("4D heterogeneous Gaussian must pass PO-TOUCH-4 R-hat/ESS certificate");

        let exact_means = [1.2, -0.8, 3.5, 0.4];
        let exact_sds = [0.3, 1.5, 4.0, 0.8];
        for (i, p) in res.certificate.parameters.iter().enumerate() {
            let mcse = p.mcse_mean.expect("mcse");
            assert!(
                (p.mean - exact_means[i]).abs() <= 4.5 * mcse,
                "param {}: mean={} vs exact={}, mcse={mcse}",
                p.name,
                p.mean,
                exact_means[i]
            );
            assert!(
                (p.sd - exact_sds[i]).abs() <= 0.12 * exact_sds[i],
                "param {}: sd={} vs exact={}",
                p.name,
                p.sd,
                exact_sds[i]
            );
        }
    }

    #[test]
    fn test_nuts_conjugate_beta_and_gamma_via_bijectors() {
        // 1. Beta(12, 5) via UnitInterval bijector: E[p] = 12/17, SD[p] = sqrt(60 / (289*18)).
        let beta_model = build_beta_logit_model();
        let cfg = NutsConfig {
            seed_root: 0xBE7A_0001,
            num_warmup: 350,
            num_draws: 450,
            ..NutsConfig::default()
        };
        let res = sample_nuts(
            &beta_model,
            &[vec![-1.0], vec![0.0], vec![1.0], vec![2.0]],
            &cfg,
        )
        .expect("nuts beta");
        res.certificate
            .require_certified()
            .expect("Beta-Binomial must certify");

        let p_draws: Vec<f64> = res
            .chains
            .iter()
            .flat_map(|c| c.iter())
            .map(|d| Bijector::UnitInterval.forward(Real(d[0])).unwrap().get())
            .collect();
        let n = p_draws.len() as f64;
        let mean_p = p_draws.iter().sum::<f64>() / n;
        let sd_p = det::sqrt(
            p_draws
                .iter()
                .map(|&x| (x - mean_p) * (x - mean_p))
                .sum::<f64>()
                / (n - 1.0),
        );
        let exact_mean = 12.0 / 17.0;
        let exact_sd = det::sqrt((12.0 * 5.0) / (17.0 * 17.0 * 18.0));
        assert!(
            (mean_p - exact_mean).abs() < 0.015,
            "Beta mean {mean_p} vs exact {exact_mean}"
        );
        assert!(
            (sd_p - exact_sd).abs() < 0.015,
            "Beta sd {sd_p} vs exact {exact_sd}"
        );

        // 2. Gamma(k=4.5, lambda=1.2) via Positive bijector: E[theta] = 3.75, SD = sqrt(4.5)/1.2.
        let gamma_model = build_gamma_pos_model();
        let g_res = sample_nuts(
            &gamma_model,
            &[vec![-0.5], vec![0.5], vec![1.2], vec![2.0]],
            &cfg,
        )
        .expect("nuts gamma");
        g_res
            .certificate
            .require_certified()
            .expect("Gamma must certify");

        let th_draws: Vec<f64> = g_res
            .chains
            .iter()
            .flat_map(|c| c.iter())
            .map(|d| Bijector::Positive.forward(Real(d[0])).unwrap().get())
            .collect();
        let gn = th_draws.len() as f64;
        let mean_th = th_draws.iter().sum::<f64>() / gn;
        let exact_th = 4.5 / 1.2;
        assert!(
            (mean_th - exact_th).abs() < 0.25,
            "Gamma mean {mean_th} vs exact {exact_th}"
        );
    }

    #[test]
    fn test_nuts_hierarchical_mining_grade_bias_model_certified() {
        // Non-centered 4-domain mining grade-bias model with weakly-informative domain-spread prior
        // log_tau ~ N(-1.0, 0.5^2) matched to assay noise sigma_y = 0.5 (avoids the inverted NCP
        // funnel at tau >> sigma_y while testing 6-parameter hierarchical posterior geometry).
        let model = TapeLogDensity::new(
            vec![
                "mu".to_string(),
                "log_tau".to_string(),
                "eta0".to_string(),
                "eta1".to_string(),
                "eta2".to_string(),
                "eta3".to_string(),
            ],
            |t, v| {
                let mu = v[0];
                let log_tau = v[1];
                let tau = t.exp_op(log_tau)?;

                let obs = [1.8, 0.9, 1.5, 1.2];
                let mut lik_sq_sum = t.constant(Real(0.0))?;
                let mut eta_sq_sum = t.constant(Real(0.0))?;
                for (i, &y_val) in obs.iter().enumerate() {
                    let eta = v[2 + i];
                    let sh = t.mul(tau, eta)?;
                    let th = t.add(mu, sh)?;
                    let y = t.constant(Real(y_val))?;
                    let resid = t.sub(y, th)?;
                    let r2 = t.square(resid)?;
                    lik_sq_sum = t.add(lik_sq_sum, r2)?;
                    let e2 = t.square(eta)?;
                    eta_sq_sum = t.add(eta_sq_sum, e2)?;
                }
                let lik_scale = t.constant(Real(-2.0))?; // -1 / (2 * 0.5^2)
                let lik = t.mul(lik_scale, lik_sq_sum)?;

                let mu_center = t.constant(Real(1.0))?;
                let dmu = t.sub(mu, mu_center)?;
                let mu_sq = t.square(dmu)?;
                let mu_scale = t.constant(Real(-1.0 / 4.5))?; // prior N(1.0, 1.5^2)
                let p_mu = t.mul(mu_scale, mu_sq)?;

                let lt_center = t.constant(Real(-1.0))?;
                let dlt = t.sub(log_tau, lt_center)?;
                let lt_sq = t.square(dlt)?;
                let lt_scale = t.constant(Real(-2.0))?; // prior N(-1.0, 0.5^2)
                let p_lt = t.mul(lt_scale, lt_sq)?;

                let half = t.constant(Real(-0.5))?;
                let p_eta = t.mul(half, eta_sq_sum)?;

                let p1 = t.add(p_mu, p_lt)?;
                let p_tot = t.add(p1, p_eta)?;
                t.add(lik, p_tot)
            },
        )
        .expect("hierarchical_4domain");

        let cfg = NutsConfig {
            seed_root: 0x4849_4552,
            num_warmup: 500,
            num_draws: 600,
            target_accept: 0.90,
            ..NutsConfig::default()
        };
        let inits = vec![
            vec![1.0, -1.0, 0.2, -0.2, 0.1, -0.1],
            vec![0.6, -0.6, -0.3, 0.3, -0.2, 0.2],
            vec![1.6, -1.4, 0.4, -0.4, 0.3, -0.3],
            vec![1.2, -0.8, 0.0, 0.1, -0.1, 0.0],
        ];
        let res = sample_nuts(&model, &inits, &cfg).expect("hierarchical ncp");
        res.certificate.require_certified().expect(
            "non-centered 4-domain hierarchical grade-bias model must certify with 0 divergences",
        );
        assert_eq!(res.certificate.total_divergences, 0);
        // Posterior mean of mu should sit near the pooled domain mean ~1.35.
        let mu_mean = res.certificate.parameters[0].mean;
        assert!(
            mu_mean > 1.0 && mu_mean < 1.7,
            "unexpected hierarchical mu_mean={mu_mean}"
        );
    }

    #[test]
    fn test_planted_negative_multimodal_and_divergent_chains_refused() {
        // 1. Strongly separated bimodal mixture at +-10 with fixed small step size traps chains in
        // opposite modes -> R-hat >> 1.01 -> require_certified() MUST refuse with FMINE-REF-207.
        struct SeparatedBimodal;
        impl LogDensity for SeparatedBimodal {
            fn dim(&self) -> usize {
                1
            }
            fn param_names(&self) -> Vec<String> {
                vec!["theta".to_string()]
            }
            fn log_density_and_grad(&self, z: &[f64]) -> Result<(f64, Vec<f64>), Refusal> {
                let x = z[0];
                let l1 = -0.5 * (x - 10.0) * (x - 10.0);
                let l2 = -0.5 * (x + 10.0) * (x + 10.0);
                let lse = ad::log_add_exp(l1, l2);
                let w1 = det::exp(l1 - lse);
                let w2 = det::exp(l2 - lse);
                let g = -w1 * (x - 10.0) - w2 * (x + 10.0);
                Ok((lse, vec![g]))
            }
        }
        let bimodal_cfg = NutsConfig {
            seed_root: 0xBAD0_0001,
            num_warmup: 100,
            num_draws: 200,
            fixed_step_size: Some(0.25),
            adapt_mass_matrix: false,
            ..NutsConfig::default()
        };
        let bimodal_res = sample_nuts(&SeparatedBimodal, &[vec![-10.0], vec![10.0]], &bimodal_cfg)
            .expect("sampling completes and emits refused certificate");
        let err = bimodal_res
            .certificate
            .require_certified()
            .expect_err("separated bimodal chains must refuse");
        assert_eq!(
            err.code,
            RefusalCode::NumericalCertificateFailed207.as_str()
        );
        assert!(
            bimodal_res.certificate.parameters[0]
                .split_rhat
                .expect("rhat")
                > 1.5
        );

        // 2. Deliberately unstable fixed step size -> divergences + refusal with FMINE-REF-207.
        let model = build_normal_2d_corr_model();
        let div_cfg = NutsConfig {
            seed_root: 0xBAD0_0002,
            num_warmup: 20,
            num_draws: 60,
            fixed_step_size: Some(50.0),
            adapt_mass_matrix: false,
            ..NutsConfig::default()
        };
        let div_res = sample_nuts(&model, &[vec![0.0, 0.0], vec![1.0, 1.0]], &div_cfg)
            .expect("divergent sampling returns refused certificate");
        assert!(div_res.certificate.total_divergences > 0);
        assert_eq!(
            div_res.certificate.require_certified().unwrap_err().code,
            RefusalCode::NumericalCertificateFailed207.as_str()
        );
    }

    #[test]
    fn test_simulation_based_calibration_conjugate_normal() {
        // SBC (Talts et al. 2018): draw theta* ~ N(0, 1), y | theta* ~ N(theta*, 0.5^2),
        // sample posterior via NUTS, verify posterior ranks of theta* are uniform and 80% CI covers.
        struct NormalConjugate {
            y: f64,
        }
        impl LogDensity for NormalConjugate {
            fn dim(&self) -> usize {
                1
            }
            fn param_names(&self) -> Vec<String> {
                vec!["theta".to_string()]
            }
            fn log_density_and_grad(&self, z: &[f64]) -> Result<(f64, Vec<f64>), Refusal> {
                let th = z[0];
                let r = self.y - th;
                // Prior N(0, 1): -0.5 * th^2; Lik N(th, 0.5^2): -2 * (y - th)^2
                let lp = -0.5 * th * th - 2.0 * r * r;
                let g = -th + 4.0 * r;
                Ok((lp, vec![g]))
            }
        }

        let mut gen_rng = PhiloxStream::new(RngKey {
            seed_root: 0x5BC0_2026,
            partition: 7,
            domain: 0,
            var_group: 0,
            realization: 0,
            brick: 0,
        });
        let reps = 32usize;
        let mut rank_fracs = Vec::with_capacity(reps);
        let mut covered_80 = 0usize;

        for rep in 0..reps {
            let theta_true = gen_rng.next_normal();
            let y_obs = theta_true + 0.5 * gen_rng.next_normal();
            let cfg = NutsConfig {
                seed_root: 0x5BC1_0000 + rep as u64,
                num_warmup: 120,
                num_draws: 150,
                thresholds: DiagnosticThresholds {
                    min_bulk_ess: 80.0,
                    min_tail_ess: 60.0,
                    max_rhat: 1.05,
                    ..DiagnosticThresholds::default()
                },
                ..NutsConfig::default()
            };
            let res = sample_nuts(
                &NormalConjugate { y: y_obs },
                &[vec![0.0], vec![y_obs]],
                &cfg,
            )
            .expect("sbc nuts");
            let mut draws: Vec<f64> = res
                .chains
                .iter()
                .flat_map(|c| c.iter().map(|d| d[0]))
                .collect();
            draws.sort_by(|a, b| a.total_cmp(b));
            let rank = draws.iter().filter(|&&d| d < theta_true).count();
            let u = (rank as f64 + 0.5) / (draws.len() as f64 + 1.0);
            rank_fracs.push(u);
            let lo = draws[draws.len() / 10];
            let hi = draws[(draws.len() * 9) / 10];
            if theta_true >= lo && theta_true <= hi {
                covered_80 += 1;
            }
        }

        let mean_u = rank_fracs.iter().sum::<f64>() / (reps as f64);
        let cov = (covered_80 as f64) / (reps as f64);
        assert!(
            mean_u > 0.35 && mean_u < 0.65,
            "SBC mean rank {mean_u} outside expected uniform band [0.35, 0.65]"
        );
        assert!(
            (0.625..=0.97).contains(&cov),
            "SBC 80% empirical coverage {cov} outside expected binomial band [0.625, 0.97]"
        );
    }

    #[test]
    fn test_nuts_bitwise_deterministic_and_input_validation() {
        let model = build_normal_2d_corr_model();
        let inits = vec![vec![0.0, 0.0], vec![2.0, -1.0]];
        let cfg = NutsConfig {
            seed_root: 0xDE7E_1234,
            num_warmup: 80,
            num_draws: 80,
            thresholds: DiagnosticThresholds {
                min_bulk_ess: 40.0,
                min_tail_ess: 40.0,
                max_rhat: 1.05,
                ..DiagnosticThresholds::default()
            },
            ..NutsConfig::default()
        };
        let r1 = sample_nuts(&model, &inits, &cfg).expect("run 1");
        let r2 = sample_nuts(&model, &inits, &cfg).expect("run 2");
        assert_eq!(r1.chains.len(), r2.chains.len());
        for (c1, c2) in r1.chains.iter().zip(r2.chains.iter()) {
            for (d1, d2) in c1.iter().zip(c2.iter()) {
                for (&x1, &x2) in d1.iter().zip(d2.iter()) {
                    assert_eq!(x1.to_bits(), x2.to_bits());
                }
            }
        }
        assert_eq!(r1.certificate, r2.certificate);

        // Input validation refusals fail closed with FMINE-REF-109.
        assert_eq!(
            sample_nuts(&model, &[vec![0.0, 0.0]], &cfg)
                .unwrap_err()
                .code,
            RefusalCode::OutsidePhysicalDomain109.as_str()
        );
        assert_eq!(
            sample_nuts(&model, &[vec![f64::NAN, 0.0], vec![0.0, 0.0]], &cfg)
                .unwrap_err()
                .code,
            RefusalCode::OutsidePhysicalDomain109.as_str()
        );
        let bad_cfg = NutsConfig {
            num_draws: 2,
            ..cfg
        };
        assert_eq!(
            sample_nuts(&model, &inits, &bad_cfg).unwrap_err().code,
            RefusalCode::OutsidePhysicalDomain109.as_str()
        );
    }

    #[test]
    fn test_fresh_eyes_hardening_guards_and_refusals() {
        // 1. Custom LogDensity returning wrong gradient length must refuse (never panic).
        struct ShortGradModel;
        impl LogDensity for ShortGradModel {
            fn dim(&self) -> usize {
                2
            }
            fn param_names(&self) -> Vec<String> {
                vec!["a".to_string(), "b".to_string()]
            }
            fn log_density_and_grad(&self, _z: &[f64]) -> Result<(f64, Vec<f64>), Refusal> {
                Ok((0.0, vec![0.0])) // length 1 != dim 2
            }
        }
        let cfg = NutsConfig {
            num_warmup: 4,
            num_draws: 4,
            ..NutsConfig::default()
        };
        assert_eq!(
            sample_nuts(&ShortGradModel, &[vec![0.0, 0.0], vec![1.0, 1.0]], &cfg)
                .unwrap_err()
                .code,
            RefusalCode::OutsidePhysicalDomain109.as_str()
        );

        // 2. Duplicate or blank parameter names refuse in TapeLogDensity and certify_chains.
        assert_eq!(
            TapeLogDensity::new(vec!["x".to_string(), "x".to_string()], |_t, v| Ok(v[0]))
                .err()
                .expect("dup names")
                .code,
            RefusalCode::OutsidePhysicalDomain109.as_str()
        );
        assert_eq!(
            TapeLogDensity::new(vec!["   ".to_string()], |_t, v| Ok(v[0]))
                .err()
                .expect("blank name")
                .code,
            RefusalCode::OutsidePhysicalDomain109.as_str()
        );

        // 3. Cross-tape Var misuse refuses instead of panicking.
        let mut t1 = Tape::default();
        let _v0 = t1.input(Real(1.0)).expect("v0");
        let v1 = t1.input(Real(2.0)).expect("v1");
        let mut t2 = Tape::default();
        let a = t2.input(Real(3.0)).expect("a");
        assert_eq!(
            t2.add(a, v1).unwrap_err().code,
            RefusalCode::OutsidePhysicalDomain109.as_str()
        );

        // 4. log_add_exp(+inf, +inf) stays +inf (never NaN).
        assert_eq!(ad::log_add_exp(f64::INFINITY, f64::INFINITY), f64::INFINITY);
        assert_eq!(ad::log_add_exp(f64::NEG_INFINITY, 2.5), 2.5);

        // 5. Direct compute_parameter_diagnostics on ragged or non-finite chains never panics.
        let ragged = compute_parameter_diagnostics(
            "ragged",
            &[vec![1.0, 2.0, 3.0, 4.0], vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0]],
        );
        assert!(ragged.split_rhat.is_none());
        assert_eq!(ragged.bulk_ess, 0.0);

        let nan_diag = compute_parameter_diagnostics("nan", &[vec![f64::NAN; 4], vec![1.0; 4]]);
        assert!(nan_diag.split_rhat.is_none());
        assert!(nan_diag.mean.is_finite());

        // 6. Missing post_warmup_energies must fail-closed on E-BFMI when min_ebfmi > 0.
        let chains = vec![
            vec![vec![0.1], vec![0.2], vec![-0.1], vec![-0.2]],
            vec![vec![-0.1], vec![0.15], vec![0.05], vec![-0.15]],
        ];
        let stats = vec![
            ChainRuntimeStats {
                chain_id: 0,
                adapted_step_size: 0.5,
                post_warmup_divergences: 0,
                max_treedepth_hits: 0,
                mean_accept_prob: 0.8,
                post_warmup_energies: Vec::new(),
            },
            ChainRuntimeStats {
                chain_id: 1,
                adapted_step_size: 0.5,
                post_warmup_divergences: 0,
                max_treedepth_hits: 0,
                mean_accept_prob: 0.8,
                post_warmup_energies: Vec::new(),
            },
        ];
        let cert = certify_chains(
            &chains,
            &["z".to_string()],
            &stats,
            &DiagnosticThresholds {
                min_bulk_ess: 1.0,
                min_tail_ess: 1.0,
                max_rhat: 2.0,
                min_ebfmi: 0.3,
                ..DiagnosticThresholds::default()
            },
        )
        .expect("cert");
        assert!(matches!(cert.verdict, DiagnosticVerdict::Refused(_)));
        assert_eq!(
            cert.require_certified().unwrap_err().code,
            RefusalCode::NumericalCertificateFailed207.as_str()
        );
    }
}
