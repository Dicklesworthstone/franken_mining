//! Frozen forecast ledger (`INV-008`), proper scoring rules (`CRPS`, log, Winkler),
//! `CalibrationFlag` auto-downgrade (`NL-8`), `REFCLASS-` $\varepsilon$-contamination (`GB-32`),
//! and Vecchia/Rosenblatt spatial PIT e-process monitoring (`GB-08`, `BD-311`).
//! Determinism: BitwisePortable (transcendentals via `fmine_core::det`; the file's three golden digests agree on
//! x86_64-linux and aarch64-macos in CI run 41237993223 at 84f32ac, bd-hp2j.3).

use fmine_claim::Claim;
use fmine_core::det;
use fmine_core::hash::blake3_hash;
use fmine_core::rand::{PhiloxStream, RngKey};
use fmine_evidence::{EvidenceClass, Refusal};
use fmine_numerics::cholesky_solve;
use fmine_numerics::eproc::BettingEProcess;
use fmine_numerics::norm_cdf;

pub mod ledger;
pub mod record;
pub use ledger::{
    Assessment, AssessmentPolicy, AssessmentSummary, Cohort, CohortSpec, CoverageSummary,
    EffectiveSampleSize, ForecastLedger, ForecastPayload, FrozenForecast, MemberAssessment,
    MemberOutcome, MissingReason, OutcomeCitation, OutcomeEvent, OutcomeKind, OutcomeSelector,
    ScoreBounds, ScoreSummary,
};

/// Calibration health classification (`BD-049`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CalibrationFlag {
    Calibrated,
    Overconfident,
    Biased,
    Unscored,
    /// Scores are descriptive: the declared effective sample is too small, membership is unresolved, or the
    /// forecast has no stated probability level. It cannot confer calibrated evidence (BD-343).
    Descriptive,
}

/// Summary report produced by `ForecastLedger::evaluate_model` (`BD-048`, `BD-049`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CalibrationReport {
    pub n_members: usize,
    pub n_scored: usize,
    pub n_unresolved: usize,
    /// A bound on final coverage, not a confidence interval; absent for an empty model.
    pub whole_cohort_coverage_80: Option<(f64, f64)>,
    pub mean_crps: f64,
    pub mean_winkler_80: f64,
    pub empirical_coverage_80: f64,
    pub mean_standardized_bias: f64,
    pub flag: CalibrationFlag,
}

/// Applies `CalibrationFlag` to a `Claim<T>`, enforcing `NL-8`: if `flag` is `Overconfident`
/// or `Biased`, marks `calibration_failed = true`. Either way, and for `Unscored` or `Descriptive`, a `ModelledCalibrated` claim
/// has lost its track record and becomes `ModelledUncalibrated` (`BD-049`, NL-4).
pub fn apply_calibration_gate<T>(claim: &mut Claim<T>, flag: CalibrationFlag) {
    if flag == CalibrationFlag::Calibrated {
        return;
    }
    claim.calibration_failed |= matches!(
        flag,
        CalibrationFlag::Overconfident | CalibrationFlag::Biased
    );
    if claim.evidence == EvidenceClass::ModelledCalibrated {
        claim.evidence = EvidenceClass::ModelledUncalibrated;
    }
}

// --- Proper Scoring Rules (BD-048) ---

/// Closed-form Gneiting & Raftery (2007) Continuous Ranked Probability Score (`CRPS`)
/// for $\mathcal{N}(\mu, \sigma^2)$ evaluated at realization $y$:
/// $\text{CRPS}(\mathcal{N}(\mu, \sigma^2), y) = \sigma \left[ z (2 \Phi(z) - 1) + 2 \phi(z) - \frac{1}{\sqrt{\pi}} \right]$.
pub fn gaussian_crps(mean: f64, std_dev: f64, y: f64) -> Result<f64, ScoreError> {
    check_gaussian(mean, std_dev, y)?;
    let z = (y - mean) / std_dev;
    let phi_cdf = norm_cdf(z);
    let phi_pdf = det::exp(-0.5 * z * z) / (2.0 * std::f64::consts::PI).sqrt();
    let inv_sqrt_pi = 1.0 / std::f64::consts::PI.sqrt();
    Ok(std_dev * (z * (2.0 * phi_cdf - 1.0) + 2.0 * phi_pdf - inv_sqrt_pi))
}

/// $\ln 2\pi$ as the literal `det::ln(2π)` returns (`test_ln_2pi_literal_matches_det` pins the bits). It is 0.525
/// ulp from the exact logarithm of the f64 2π, one ulp above the correctly rounded value (bd-hp2j.2).
const LN_2PI: f64 = 1.837_877_066_409_345_6;

/// Negative log predictive density (`NLL`) for $\mathcal{N}(\mu, \sigma^2)$ at $y$.
pub fn gaussian_log_score(mean: f64, std_dev: f64, y: f64) -> Result<f64, ScoreError> {
    check_gaussian(mean, std_dev, y)?;
    let z = (y - mean) / std_dev;
    Ok(0.5 * LN_2PI + det::ln(std_dev) + 0.5 * z * z)
}

/// The Philox key of the scoring-rule golden digests (bd-hp2j.3).
pub const SCORE_DIGEST_KEY: RngKey = RngKey {
    seed_root: 0x6361_6c69,
    partition: 1,
    domain: 0,
    var_group: 0,
    realization: 0,
    brick: 0,
};

/// The 10,000 `(mean, std_dev, y)` triples of the scoring-rule digests: mean uniform on [−100, 100], `std_dev`
/// log-uniform on [1e-3, 1e3] (through `det`), and `y = mean + z·std_dev` with `z` uniform on [−12, 12], so
/// |z| > 8 is a third of them.
#[must_use]
pub fn score_digest_inputs(key: RngKey) -> Vec<(f64, f64, f64)> {
    let mut rng = PhiloxStream::new(key);
    let (lo, hi) = (det::ln(1e-3), det::ln(1e3));
    (0..10_000)
        .map(|_| {
            let mean = 200.0 * rng.next_f64_open01() - 100.0;
            let std_dev = det::exp(lo + rng.next_f64_open01() * (hi - lo));
            let z = 24.0 * rng.next_f64_open01() - 12.0;
            (mean, std_dev, mean + z * std_dev)
        })
        .collect()
}

/// BLAKE3 over each score's little-endian bits over [`score_digest_inputs`]; a refusal hashes as `u64::MAX`.
fn digest_scores(key: RngKey, score: fn(f64, f64, f64) -> Result<f64, ScoreError>) -> String {
    let bytes: Vec<u8> = score_digest_inputs(key)
        .into_iter()
        .map(|(m, s, y)| score(m, s, y).map_or(u64::MAX, f64::to_bits))
        .flat_map(u64::to_le_bytes)
        .collect();
    blake3_hash(&bytes).to_hex()
}

/// The digest of [`gaussian_crps`] over [`score_digest_inputs`].
#[must_use]
pub fn digest_gaussian_crps(key: RngKey) -> String {
    digest_scores(key, gaussian_crps)
}

/// The digest of [`gaussian_log_score`] over [`score_digest_inputs`].
#[must_use]
pub fn digest_gaussian_log_score(key: RngKey) -> String {
    digest_scores(key, gaussian_log_score)
}

/// The values behind [`digest_calib_rest`]; a refusal is `u64::MAX`.
fn calib_rest_bits(key: RngKey) -> Vec<u64> {
    let inputs = score_digest_inputs(key);
    let mut bits = Vec::with_capacity(34_104);
    let prior = ReferenceClassPrior::GREENFIELD_PORPHYRY_CAPEX;
    for &(mean, std_dev, y) in &inputs {
        let (lo, hi) = (mean - 1.28 * std_dev, mean + 1.28 * std_dev);
        bits.push(winkler_interval_score(lo, hi, 0.2, y).map_or(u64::MAX, f64::to_bits));
        match prior.blend(mean, std_dev) {
            Ok((m, s)) => bits.extend([m.to_bits(), s.to_bits()]),
            Err(_) => bits.extend([u64::MAX; 2]),
        }
    }
    let mut ledger = ForecastLedger::new();
    for (i, &(mean, std_dev, y)) in inputs.iter().step_by(100).enumerate() {
        let t = i as i64;
        let scored = ledger
            .register("digest", t, t, mean, std_dev)
            .and_then(|id| ledger.record_outcome(id, t + 1, y));
        bits.push(scored.map_or(u64::MAX, |()| 0));
    }
    match ledger.evaluate_model("digest") {
        Ok(r) => bits.extend(
            [
                r.mean_crps,
                r.mean_winkler_80,
                r.empirical_coverage_80,
                r.mean_standardized_bias,
            ]
            .map(f64::to_bits),
        ),
        Err(_) => bits.extend([u64::MAX; 4]),
    }
    let mut rng = PhiloxStream::new(key);
    let mut unit = || rng.next_f64_open01();
    match SpatialFrontPitMonitor::new(0.05, 1.0, 50.0, 8, 0.05) {
        Ok(mut monitor) => {
            for i in 0..1_000 {
                // A bias that flips at sample 200 takes the e-processes to both betting limits and the alarm.
                let bias = if i < 200 { 3.0 } else { -3.0 };
                let sample = SpatialSample {
                    x: 500.0 * unit(),
                    y: 500.0 * unit(),
                    z: 100.0 * unit(),
                    prior_mean: 5.0,
                    observed_value: 3.0 + bias + 4.0 * unit(),
                };
                match monitor.observe_sample(sample) {
                    Ok((u, alarm)) => bits.extend([
                        u.to_bits(),
                        u64::from(alarm),
                        monitor.mean_eproc.wealth.to_bits(),
                        monitor.spread_eproc.wealth.to_bits(),
                    ]),
                    Err(_) => bits.extend([u64::MAX; 4]),
                }
            }
        }
        Err(_) => bits.extend([u64::MAX; 4_000]),
    }
    bits
}

/// The digest of the rest of this file, so that one class covers all of it (bd-hp2j.3). On
/// [`score_digest_inputs`]: each Winkler score and reference-class blend, then a ledger of every hundredth input
/// and its report. Then 1,000 samples through [`SpatialFrontPitMonitor`], which also runs `BettingEProcess`.
#[must_use]
pub fn digest_calib_rest(key: RngKey) -> String {
    let bytes: Vec<u8> = calib_rest_bits(key)
        .into_iter()
        .flat_map(u64::to_le_bytes)
        .collect();
    blake3_hash(&bytes).to_hex()
}

/// [`digest_calib_rest`] under [`SCORE_DIGEST_KEY`] (partition 2).
#[must_use]
pub fn golden_digest_calib_rest_10k() -> String {
    digest_calib_rest(RngKey {
        partition: 2,
        ..SCORE_DIGEST_KEY
    })
}

/// [`digest_gaussian_crps`] under [`SCORE_DIGEST_KEY`].
#[must_use]
pub fn golden_digest_gaussian_crps_10k() -> String {
    digest_gaussian_crps(SCORE_DIGEST_KEY)
}

/// [`digest_gaussian_log_score`] under [`SCORE_DIGEST_KEY`].
#[must_use]
pub fn golden_digest_gaussian_log_score_10k() -> String {
    digest_gaussian_log_score(SCORE_DIGEST_KEY)
}

/// Winkler interval score for the central $(1 - \alpha)$ interval $[l, u]$ at realization $y$.
///
/// Refuses `alpha` outside `(0, 1)`, an inverted interval and non-finite inputs; nothing is clamped.
pub fn winkler_interval_score(lo: f64, hi: f64, alpha: f64, y: f64) -> Result<f64, ScoreError> {
    if !(lo.is_finite() && hi.is_finite() && alpha.is_finite() && y.is_finite()) {
        return Err(ScoreError::NonFinite);
    }
    if !(alpha > 0.0 && alpha < 1.0) {
        return Err(ScoreError::AlphaOutOfDomain(alpha));
    }
    if lo > hi {
        return Err(ScoreError::InvertedInterval { lo, hi });
    }
    let width = hi - lo;
    Ok(if y < lo {
        width + (2.0 / alpha) * (lo - y)
    } else if y > hi {
        width + (2.0 / alpha) * (y - hi)
    } else {
        width
    })
}

/// Why a proper score refused (`bd-8wrk`): out-of-domain inputs are never floored or clamped (AGENTS rule 5).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ScoreError {
    /// A mean, bound, level or realization is NaN or infinite.
    NonFinite,
    /// The predictive standard deviation is not positive.
    NonPositiveStdDev(f64),
    /// The interval level `alpha` is outside `(0, 1)`.
    AlphaOutOfDomain(f64),
    /// The interval's lower bound exceeds its upper bound.
    InvertedInterval { lo: f64, hi: f64 },
    /// A model parameter is outside its documented domain.
    InvalidParameter { name: &'static str, value: f64 },
}

impl std::fmt::Display for ScoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NonFinite => write!(f, "score inputs must be finite"),
            Self::NonPositiveStdDev(s) => write!(f, "predictive std_dev must be positive, got {s}"),
            Self::AlphaOutOfDomain(a) => {
                write!(f, "interval level alpha must lie in (0, 1), got {a}")
            }
            Self::InvertedInterval { lo, hi } => {
                write!(f, "interval is inverted: lo {lo} > hi {hi}")
            }
            Self::InvalidParameter { name, value } => {
                write!(f, "parameter {name} = {value} is outside its domain")
            }
        }
    }
}

impl std::error::Error for ScoreError {}

fn check_gaussian(mean: f64, std_dev: f64, y: f64) -> Result<(), ScoreError> {
    if !(mean.is_finite() && std_dev.is_finite() && y.is_finite()) {
        return Err(ScoreError::NonFinite);
    }
    if std_dev <= 0.0 {
        return Err(ScoreError::NonPositiveStdDev(std_dev));
    }
    Ok(())
}

// --- Reference-Class Epsilon-Contamination Prior (BD-050, GB-32) ---

/// Huber $\varepsilon$-contaminated reference-class prior blending an internal engineering estimate
/// $\mathcal{N}(\mu_{\text{int}}, \sigma_{\text{int}}^2)$ with an industry outside-view reference class
/// $\mathcal{N}(\mu_{\text{ref}}, \sigma_{\text{ref}}^2)$ (`GB-32`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ReferenceClassPrior {
    pub class_id: &'static str,
    pub ref_mean_multiplier: f64,
    pub ref_std_multiplier: f64,
    pub epsilon: f64,
}

impl ReferenceClassPrior {
    /// Outside-view greenfield porphyry Cu-Au project capex overrun prior (`REFCLASS-CAPEX-GREENFIELD`).
    pub const GREENFIELD_PORPHYRY_CAPEX: Self = Self {
        class_id: "REFCLASS-CAPEX-GREENFIELD",
        ref_mean_multiplier: 1.28,
        ref_std_multiplier: 0.35,
        epsilon: 0.25,
    };

    /// Computes the moment-matched blended mean and standard deviation for an internal baseline `base`.
    ///
    /// Refuses an `epsilon` outside `[0, 1]`, a negative `base_std` and non-finite inputs (bd-8wrk: `epsilon` was
    /// clamped).
    pub fn blend(&self, base_mean: f64, base_std: f64) -> Result<(f64, f64), ScoreError> {
        let eps = self.epsilon;
        if !(eps.is_finite() && (0.0..=1.0).contains(&eps)) {
            return Err(ScoreError::InvalidParameter {
                name: "epsilon",
                value: eps,
            });
        }
        if !(base_mean.is_finite() && base_std.is_finite()) {
            return Err(ScoreError::NonFinite);
        }
        if base_std < 0.0 {
            return Err(ScoreError::NonPositiveStdDev(base_std));
        }
        let ref_mean = base_mean * self.ref_mean_multiplier;
        let ref_std = base_mean.abs() * self.ref_std_multiplier;

        let blended_mean = (1.0 - eps) * base_mean + eps * ref_mean;
        let m2_int = base_std * base_std + base_mean * base_mean;
        let m2_ref = ref_std * ref_std + ref_mean * ref_mean;
        // A mixture's variance is non-negative; `max(0.0)` only removes a rounding residue of order ε·m2 when the
        // two components coincide. It is not a domain clamp.
        let blended_var =
            ((1.0 - eps) * m2_int + eps * m2_ref - blended_mean * blended_mean).max(0.0);
        Ok((blended_mean, blended_var.sqrt()))
    }
}

// --- Vecchia / Rosenblatt Spatial Front PIT Monitor (BD-051, BD-311, GB-08) ---

/// 3D spatial sample observed at an advancing mining face or blast-hole pattern (`BD-311`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpatialSample {
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub prior_mean: f64,
    pub observed_value: f64,
}

/// Vecchia-conditioned Rosenblatt PIT sequential monitor (`Monitor::SpatialFrontPit`, `GB-08`, `BD-311`).
///
/// Decorrelates each new spatial observation $y_i$ against up to `max_neighbors` nearest prior
/// observations under an isotropic exponential covariance $C(h) = \sigma_0^2 \mathbb{I}(h=0) + \sigma_1^2 \exp(-h / r)$,
/// maps the standardized conditional residual $z_i^* = (y_i - \mu_{i|\mathcal{N}(i)}) / \sigma_{i|\mathcal{N}(i)}$
/// to $u_i = \Phi(z_i^*) \in (0, 1)$, and updates a pair of anytime-valid `BettingEProcess` monitors
/// (one for mean bias, one for variance/spread error).
#[derive(Debug, Clone)]
pub struct SpatialFrontPitMonitor {
    pub nugget_var: f64,
    pub sill_var: f64,
    pub range_m: f64,
    pub max_neighbors: usize,
    pub history: Vec<SpatialSample>,
    pub mean_eproc: BettingEProcess,
    pub spread_eproc: BettingEProcess,
}

impl SpatialFrontPitMonitor {
    /// Creates a new `SpatialFrontPitMonitor` with exponential variogram parameters and significance `alpha`.
    ///
    /// Refuses (rather than floors or clamps, bd-8wrk) a nugget or sill below 1e-6, a range below 1 m, a
    /// neighbourhood outside 1..=16 and `alpha` outside (0, 1).
    pub fn new(
        nugget_var: f64,
        sill_var: f64,
        range_m: f64,
        max_neighbors: usize,
        alpha: f64,
    ) -> Result<Self, ScoreError> {
        let check = |name: &'static str, value: f64, ok: bool| {
            if value.is_finite() && ok {
                Ok(())
            } else {
                Err(ScoreError::InvalidParameter { name, value })
            }
        };
        check("nugget_var", nugget_var, nugget_var >= 1e-6)?;
        check("sill_var", sill_var, sill_var >= 1e-6)?;
        check("range_m", range_m, range_m >= 1.0)?;
        check(
            "max_neighbors",
            max_neighbors as f64,
            (1..=16).contains(&max_neighbors),
        )?;
        if !(alpha.is_finite() && alpha > 0.0 && alpha < 1.0) {
            return Err(ScoreError::AlphaOutOfDomain(alpha));
        }
        Ok(Self {
            nugget_var,
            sill_var,
            range_m,
            max_neighbors,
            history: Vec::new(),
            mean_eproc: BettingEProcess::new(0.5, alpha * 0.5),
            spread_eproc: BettingEProcess::new(0.25, alpha * 0.5),
        })
    }

    fn covariance(&self, a: &SpatialSample, b: &SpatialSample) -> f64 {
        let dx = a.x - b.x;
        let dy = a.y - b.y;
        let dz = a.z - b.z;
        let h = (dx * dx + dy * dy + dz * dz).sqrt();
        if h < 1e-9 {
            self.nugget_var + self.sill_var
        } else {
            self.sill_var * det::exp(-h / self.range_m)
        }
    }

    /// Conditions `sample` on its nearest Vecchia predecessors, computes the whitened Rosenblatt PIT $u_i$,
    /// updates the e-processes, and appends `sample` to the conditioning history (`BD-311`).
    pub fn observe_sample(&mut self, sample: SpatialSample) -> Result<(f64, bool), Refusal> {
        let total_var = self.nugget_var + self.sill_var;
        let (cond_mean, cond_std) = if self.history.is_empty() {
            (sample.prior_mean, total_var.sqrt())
        } else {
            let mut indexed_dists: Vec<(f64, usize)> = self
                .history
                .iter()
                .enumerate()
                .map(|(idx, prev)| {
                    let dx = sample.x - prev.x;
                    let dy = sample.y - prev.y;
                    let dz = sample.z - prev.z;
                    (dx * dx + dy * dy + dz * dz, idx)
                })
                .collect();
            indexed_dists.sort_by(|a, b| a.0.total_cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
            let k = indexed_dists.len().min(self.max_neighbors);
            let neighbors: Vec<SpatialSample> = indexed_dists[..k]
                .iter()
                .map(|&(_, idx)| self.history[idx])
                .collect();

            let mut k_mat = vec![0.0_f64; k * k];
            let mut k_vec = vec![0.0_f64; k];
            for i in 0..k {
                k_vec[i] = self.covariance(&sample, &neighbors[i]);
                for j in 0..k {
                    k_mat[i * k + j] = if i == j {
                        total_var
                    } else {
                        self.covariance(&neighbors[i], &neighbors[j])
                    };
                }
            }

            let weights = cholesky_solve(&k_mat, &k_vec, k)?;
            let mut mu = sample.prior_mean;
            let mut var_red = 0.0_f64;
            for i in 0..k {
                mu += weights[i] * (neighbors[i].observed_value - neighbors[i].prior_mean);
                var_red += weights[i] * k_vec[i];
            }
            let c_var = (total_var - var_red).max(self.nugget_var);
            (mu, c_var.sqrt())
        };

        let z_star = (sample.observed_value - cond_mean) / cond_std;
        let u_pit = norm_cdf(z_star);
        self.mean_eproc.observe(u_pit);
        //Under Uniform(0,1), E[(u - 0.5)^2] = 1/12, and E[|u - 0.5|] = 0.25.
        self.spread_eproc.observe((u_pit - 0.5).abs());
        self.history.push(sample);

        let alarm = self.mean_eproc.rejected_null() || self.spread_eproc.rejected_null();
        Ok((u_pit, alarm))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fmine_claim::{AdditivityClass, Confidentiality, Representation, SpatialSupport};
    use fmine_core::anchor::SourceAnchor;
    use fmine_core::id::{ObjectId, PrefixKind};
    use fmine_core::rand::{PhiloxStream, RngKey};
    use fmine_evidence::{EpistemicStatus, KnowledgeState, KuKind};

    /// bd-hp2j.2: the literal is the bits `det::ln(2π)` returns, so the log score depends on no platform libm.
    #[test]
    fn test_ln_2pi_literal_matches_det() {
        let from_det = det::ln(2.0 * std::f64::consts::PI);
        assert_eq!(
            LN_2PI.to_bits(),
            from_det.to_bits(),
            "det::ln(2π) = {from_det:?}"
        );
    }

    /// The bd-hp2j.3 log line: kernel, input count, the folded Philox key and counter prefix, digest, host.
    fn digest_line(kernel: &str, key: RngKey, digest: &str) -> String {
        let (k, c) = key.to_philox_key_and_hi_ctr();
        format!(
            "{{\"kernel\":\"{kernel}\",\"n\":10000,\"rng_key\":\"{:08x}{:08x}{:08x}{:08x}\",\"digest\":\"{digest}\",\
             \"target\":\"{}-{}\"}}",
            k[0],
            k[1],
            c[0],
            c[1],
            std::env::consts::ARCH,
            std::env::consts::OS
        )
    }

    // TARGETs, first observed on aarch64-macos (bd-hp2j.3). Never regenerated on another host: a difference
    // there is the failure this test exists to catch.
    const GAUSSIAN_CRPS_10K: &str =
        "d3a20c177f23259db387f4451373c8662dbd4d3178ec2b24e6b0e05d05b75f65";
    const GAUSSIAN_LOG_SCORE_10K: &str =
        "b59d447a25e88d6a669ac00492a8bb883be9a28ac4db057865be3cf659a8b1a8";
    const CALIB_REST_10K: &str = "0652013fb792cbe11d0da7eb5b378ca8f2af427cdd42447f3a9fd792348af9c5";

    #[test]
    fn test_golden_digest_calib_rest_10k() {
        let d = golden_digest_calib_rest_10k();
        let key = RngKey {
            partition: 2,
            ..SCORE_DIGEST_KEY
        };
        println!("{}", digest_line("calib_rest", key, &d));
        assert_eq!(d, CALIB_REST_10K);
    }

    /// A digest of refusals would agree across hosts and prove nothing, so every part must produce values.
    #[test]
    fn test_calib_rest_digest_runs_every_part_and_changes_with_key() {
        let key = RngKey {
            partition: 2,
            ..SCORE_DIGEST_KEY
        };
        let bits = calib_rest_bits(key);
        assert_eq!(bits.len(), 30_000 + 100 + 4 + 4_000);
        assert!(!bits.contains(&u64::MAX));
        assert!(bits[30_104..].chunks(4).any(|sample| sample[1] == 1));
        let other = RngKey {
            seed_root: key.seed_root + 1,
            ..key
        };
        assert_ne!(digest_calib_rest(other), digest_calib_rest(key));
    }

    #[test]
    fn test_golden_digest_gaussian_crps_10k() {
        let d = golden_digest_gaussian_crps_10k();
        println!("{}", digest_line("gaussian_crps", SCORE_DIGEST_KEY, &d));
        assert_eq!(d, GAUSSIAN_CRPS_10K);
    }

    #[test]
    fn test_golden_digest_gaussian_log_score_10k() {
        let d = golden_digest_gaussian_log_score_10k();
        println!(
            "{}",
            digest_line("gaussian_log_score", SCORE_DIGEST_KEY, &d)
        );
        assert_eq!(d, GAUSSIAN_LOG_SCORE_10K);
    }

    #[test]
    fn test_score_digest_inputs_cover_tails_and_change_with_key() {
        let inputs = score_digest_inputs(SCORE_DIGEST_KEY);
        let z = |&(m, s, y): &(f64, f64, f64)| (y - m) / s;
        assert!(inputs.iter().any(|t| z(t) > 8.0) && inputs.iter().any(|t| z(t) < -8.0));
        assert!(inputs.iter().all(|t| gaussian_crps(t.0, t.1, t.2).is_ok()));
        let other = RngKey {
            seed_root: SCORE_DIGEST_KEY.seed_root + 1,
            ..SCORE_DIGEST_KEY
        };
        assert_ne!(
            digest_gaussian_crps(other),
            golden_digest_gaussian_crps_10k()
        );
        assert_ne!(
            digest_gaussian_log_score(other),
            golden_digest_gaussian_log_score_10k()
        );
    }

    #[test]
    fn test_frozen_forecast_ledger_immutability() {
        let mut ledger = ForecastLedger::new();
        let fid = ledger
            .register("grade_control_carlin", 1_000, 2_000, 3.5, 0.4)
            .expect("valid pre-freeze registration");

        // Registering after freeze_time must fail with FMINE-REF-206 (late registration, INV-008).
        let err_late = ledger
            .register("grade_control_carlin", 2_500, 2_000, 3.5, 0.4)
            .expect_err("must reject post-freeze registration");
        assert_eq!((err_late.code, err_late.exit_code), ("FMINE-REF-206", 3));

        // Recording outcome after freeze_time succeeds once, then refuses mutation.
        ledger
            .record_outcome(fid, 3_000, 3.6)
            .expect("first outcome");
        let err_mut = ledger
            .record_outcome(fid, 3_500, 4.0)
            .expect_err("must reject outcome mutation");
        assert_eq!((err_mut.code, err_mut.exit_code), ("FMINE-REF-507", 6));
    }

    /// BD-328.S3: each ledger refusal carries the code whose plan meaning matches it; none is 403 (unknown
    /// moisture vs TML, a shipment-safety code).
    #[test]
    fn test_ledger_refusals_by_condition() {
        let mut ledger = ForecastLedger::new();
        let code = |r: Refusal| (r.code, r.exit_code);
        // test_late_forecast_refuses_206
        let late = ledger.register("m", 3, 2, 1.0, 1.0).expect_err("late");
        assert_eq!(code(late), ("FMINE-REF-206", 3));
        // test_nonpositive_std_dev_refuses_109 (the other half of register's old combined check)
        for (mean, sd) in [
            (1.0, 0.0),
            (1.0, -1.0),
            (f64::NAN, 1.0),
            (1.0, f64::INFINITY),
        ] {
            let bad = ledger.register("m", 1, 2, mean, sd).expect_err("domain");
            assert_eq!(code(bad), ("FMINE-REF-109", 3), "{mean} {sd}");
        }
        // Registering the same record twice is a frozen-record mutation.
        let fid = ledger.register("m", 1, 2, 1.0, 1.0).expect("first");
        let dup = ledger.register("m", 1, 2, 1.0, 1.0).expect_err("dup");
        assert_eq!(code(dup), ("FMINE-REF-507", 6));
        // test_outcome_before_freeze_refuses_206 (the other half of record_outcome's old combined check)
        let early = ledger.record_outcome(fid, 2, 1.0).expect_err("early");
        assert_eq!(code(early), ("FMINE-REF-206", 3));
        let nan = ledger.record_outcome(fid, 3, f64::NAN).expect_err("nan");
        assert_eq!(code(nan), ("FMINE-REF-109", 3));
        // An unknown id is no claim in the domain.
        let other = ObjectId::from_canonical_bytes(PrefixKind::Forecast, b"absent");
        let unknown = ledger.record_outcome(other, 3, 1.0).expect_err("unknown");
        assert_eq!(code(unknown), ("FMINE-REF-201", 3));
        // test_outcome_overwrite_refuses_507
        ledger.record_outcome(fid, 3, 1.0).expect("first outcome");
        let again = ledger.record_outcome(fid, 4, 2.0).expect_err("overwrite");
        assert_eq!(code(again), ("FMINE-REF-507", 6));
    }

    #[test]
    fn test_proper_scoring_rules_crps_and_winkler() {
        // Perfect mean has lower CRPS than biased mean.
        let c_good = gaussian_crps(2.0, 0.5, 2.0).expect("valid");
        let c_bad = gaussian_crps(4.0, 0.5, 2.0).expect("valid");
        assert!(c_good < c_bad);

        // Winkler score penalizes observations outside [lo, hi]: width 2 plus (2 / 0.2) * 1 = 12.
        let w_in = winkler_interval_score(1.0, 3.0, 0.20, 2.0).expect("valid");
        let w_out = winkler_interval_score(1.0, 3.0, 0.20, 4.0).expect("valid");
        assert!((w_in - 2.0).abs() < 1e-12);
        assert!((w_out - 12.0).abs() < 1e-12);
    }

    /// BD-048 known answers, from `scripts/oracle/score_known_answers.py`: CRPS integrated from its definition
    /// (Simpson, CPython `erfc`), not from the closed form used here, and the log score from CPython's `log`. Rows are
    /// `(mean, std_dev, y, crps, log_score)`; N(0, 1) at 0 gives (√2 − 1)/√π and ln(2π)/2.
    #[test]
    fn test_crps_and_log_score_known_answers() {
        let rows = [
            (
                0.0,
                1.0,
                0.0,
                0.233_694_977_255_099_83,
                0.918_938_533_204_672_7,
            ),
            (
                0.0,
                1.0,
                1.0,
                0.602_441_357_627_615_1,
                1.418_938_533_204_672_7,
            ),
            (
                0.0,
                1.0,
                -2.5,
                1.939_818_690_810_345_6,
                4.043_938_533_204_672,
            ),
            (
                2.0,
                0.5,
                2.0,
                0.116_847_488_627_549_93,
                0.225_791_352_644_727_38,
            ),
            (4.0, 0.5, 2.0, 1.717_912_353_484_474, 8.225_791_352_644_727),
            (
                100.0,
                25.0,
                160.0,
                46.031_282_615_096_73,
                7.017_814_358_072_873,
            ),
            (
                -3.0,
                0.01,
                -2.97,
                0.024_365_747_250_863_2,
                0.813_768_347_216_523_1,
            ),
            (1.0, 3.0, 9.0, 6.314_521_076_418_091, 5.573_106_377_428_338),
        ];
        let close = |got: f64, want: f64| (got - want).abs() <= 1e-12 * want.abs();
        for (mean, sd, y, crps, log) in rows {
            let got = gaussian_crps(mean, sd, y).expect("valid");
            assert!(
                close(got, crps),
                "CRPS N({mean}, {sd}) at {y}: {got} vs {crps}"
            );
            let got = gaussian_log_score(mean, sd, y).expect("valid");
            assert!(
                close(got, log),
                "log score N({mean}, {sd}) at {y}: {got} vs {log}"
            );
        }
    }

    /// Regression for the 2026-10-29 finding (bd-8wrk): alpha used to be clamped to [1e-6, 1 − 1e-6], an inverted
    /// interval became width 0, and a non-positive std_dev was floored to 1e-12. Each now refuses.
    #[test]
    fn test_scores_refuse_out_of_domain_inputs() {
        assert_eq!(
            winkler_interval_score(1.0, 3.0, 0.0, 2.0),
            Err(ScoreError::AlphaOutOfDomain(0.0))
        );
        assert_eq!(
            winkler_interval_score(1.0, 3.0, 1.0, 2.0),
            Err(ScoreError::AlphaOutOfDomain(1.0))
        );
        assert_eq!(
            winkler_interval_score(3.0, 1.0, 0.2, 2.0),
            Err(ScoreError::InvertedInterval { lo: 3.0, hi: 1.0 })
        );
        assert_eq!(
            winkler_interval_score(1.0, 3.0, 0.2, f64::NAN),
            Err(ScoreError::NonFinite)
        );
        assert_eq!(
            gaussian_crps(2.0, 0.0, 2.0),
            Err(ScoreError::NonPositiveStdDev(0.0))
        );
        assert_eq!(
            gaussian_log_score(2.0, -1.0, 2.0),
            Err(ScoreError::NonPositiveStdDev(-1.0))
        );
        assert_eq!(
            gaussian_log_score(f64::INFINITY, 1.0, 2.0),
            Err(ScoreError::NonFinite)
        );
        // A degenerate interval (lo == hi) is valid and scores its miss penalty only.
        assert_eq!(winkler_interval_score(2.0, 2.0, 0.5, 3.0), Ok(4.0));
    }

    /// `evaluate_model` had no test. Known answer over three resolved forecasts, plus the entry refusals that keep
    /// unscorable records out of the ledger (so no record is ever skipped during evaluation).
    #[test]
    fn test_evaluate_model_known_answer_and_entry_refusals() {
        let mut ledger = ForecastLedger::new();
        let mut ids = Vec::new();
        for (i, mean) in [10.0, 20.0, 30.0].into_iter().enumerate() {
            let t = 1_000 * (i as i64 + 1);
            ids.push(
                ledger
                    .register("m", t, t + 100, mean, 1.0)
                    .expect("register"),
            );
        }
        // Outcomes: exactly on the mean, inside the 80% band, far outside it.
        for (fid, y) in ids.iter().zip([10.0, 20.5, 35.0]) {
            ledger.record_outcome(*fid, 10_000, y).expect("outcome");
        }
        let report = ledger.evaluate_model("m").expect("scorable");
        assert_eq!(report.n_scored, 3);
        assert!((report.empirical_coverage_80 - 2.0 / 3.0).abs() < 1e-12);
        assert!((report.mean_standardized_bias - 5.5 / 3.0).abs() < 1e-12);
        assert_eq!(report.flag, CalibrationFlag::Descriptive);
        let z80 = 1.281_551_565_544_600_4_f64;
        let width = 2.0 * z80;
        let expected_winkler = (3.0 * width + (2.0 / 0.2) * (35.0 - (30.0 + z80))) / 3.0;
        assert!((report.mean_winkler_80 - expected_winkler).abs() < 1e-9);
        assert_eq!(
            ledger.evaluate_model("absent").expect("empty").flag,
            CalibrationFlag::Unscored
        );

        assert!(
            ledger.register("m", 1, 2, 1.0, 0.0).is_err(),
            "zero std_dev refused"
        );
        assert!(
            ledger.register("m", 1, 2, 1.0, f64::NAN).is_err(),
            "NaN std_dev refused"
        );
        assert!(
            ledger.register("m", 1, 2, f64::NAN, 1.0).is_err(),
            "NaN mean refused"
        );
        // The ledger has already recorded outcomes at 10_000; this independent record must advance time.
        let fid = ledger
            .register("n", 11_000, 12_000, 1.0, 1.0)
            .expect("register");
        assert!(
            ledger.record_outcome(fid, 13_000, f64::NAN).is_err(),
            "NaN outcome refused"
        );
        assert!(
            ledger.record_outcome(fid, 13_000, 1.0).is_ok(),
            "the refused write left the record open"
        );
    }

    #[test]
    fn test_reference_class_epsilon_contamination_gb32() {
        // Reko Diq Phase 1 internal estimate $5.6B blended with greenfield reference class.
        let (mean, std) = ReferenceClassPrior::GREENFIELD_PORPHYRY_CAPEX
            .blend(5_600.0, 400.0)
            .expect("valid");
        let bad = ReferenceClassPrior {
            epsilon: 1.5,
            ..ReferenceClassPrior::GREENFIELD_PORPHYRY_CAPEX
        };
        assert_eq!(
            bad.blend(5_600.0, 400.0),
            Err(ScoreError::InvalidParameter {
                name: "epsilon",
                value: 1.5
            })
        );
        assert!(
            ReferenceClassPrior::GREENFIELD_PORPHYRY_CAPEX
                .blend(5_600.0, -1.0)
                .is_err()
        );
        assert!(mean > 5_600.0, "reference class must lift expected capex");
        assert!(std > 400.0, "reference class must widen capex dispersion");

        // Verify NL-8 auto-downgrade on biased calibration flag.
        let mut claim = Claim::new(
            "reko_diq_capex",
            mean,
            "5992.0",
            Representation::Gaussian { mean, std_dev: std },
            SpatialSupport::PeriodAggregate,
            AdditivityClass::ExtensiveAdditive,
            EvidenceClass::ModelledCalibrated,
            EpistemicStatus::KnownUnknown(KuKind::Parametric),
            KnowledgeState::Estimated,
            Confidentiality::public(),
            vec![],
            vec![],
            vec![SourceAnchor::new("esia.pdf", [0; 32], "p.1")],
        );
        let unscored = claim.clone();
        apply_calibration_gate(&mut claim, CalibrationFlag::Biased);
        assert!(claim.calibration_failed);
        assert_eq!(claim.evidence, EvidenceClass::ModelledUncalibrated);
        assert_eq!(
            claim.color().map(|_| ()).map_err(|e| e.code),
            Err("FMINE-REF-205")
        );

        // Unscored loses the class but has not failed.
        let mut unscored = unscored;
        apply_calibration_gate(&mut unscored, CalibrationFlag::Unscored);
        assert!(!unscored.calibration_failed);
        assert_eq!(unscored.evidence, EvidenceClass::ModelledUncalibrated);

        // A passing flag leaves the class alone, and no flag raises one.
        let mut assumed = unscored.clone();
        assumed.evidence = EvidenceClass::Assumed;
        apply_calibration_gate(&mut assumed, CalibrationFlag::Calibrated);
        assert_eq!(assumed.evidence, EvidenceClass::Assumed);
    }

    /// BD-049: what each of the five flags does to a calibrated Claim, and that a run of flags only ever lowers it.
    /// A later Calibrated flag neither clears a failure nor gives the class back (NL-4, NL-8).
    #[test]
    fn test_calibration_gate_per_flag_and_over_a_run_of_flags() {
        let calibrated = Claim::new(
            "grade_forecast",
            1.2,
            "1.2",
            Representation::Gaussian {
                mean: 1.2,
                std_dev: 0.1,
            },
            SpatialSupport::PeriodAggregate,
            AdditivityClass::IntensiveWeighted,
            EvidenceClass::ModelledCalibrated,
            EpistemicStatus::KnownUnknown(KuKind::Parametric),
            KnowledgeState::Estimated,
            Confidentiality::public(),
            vec![],
            vec![],
            vec![SourceAnchor::new("model_card.md", [0; 32], "s.1")],
        );
        use CalibrationFlag::*;
        for (flag, failed, evidence) in [
            (Calibrated, false, EvidenceClass::ModelledCalibrated),
            (Overconfident, true, EvidenceClass::ModelledUncalibrated),
            (Biased, true, EvidenceClass::ModelledUncalibrated),
            (Unscored, false, EvidenceClass::ModelledUncalibrated),
            (Descriptive, false, EvidenceClass::ModelledUncalibrated),
        ] {
            let mut claim = calibrated.clone();
            apply_calibration_gate(&mut claim, flag);
            assert_eq!(
                (claim.calibration_failed, claim.evidence),
                (failed, evidence),
                "{flag:?}"
            );
            let mut again = claim.clone();
            apply_calibration_gate(&mut again, flag);
            assert_eq!(again, claim, "{flag:?} applied twice");
        }

        let mut claim = calibrated.clone();
        let mut trace = Vec::new();
        for flag in [
            Calibrated,
            Descriptive,
            Calibrated,
            Overconfident,
            Unscored,
            Calibrated,
            Biased,
            Descriptive,
        ] {
            apply_calibration_gate(&mut claim, flag);
            trace.push((claim.calibration_failed, claim.evidence));
        }
        let (ok, lost, failed) = (
            (false, EvidenceClass::ModelledCalibrated),
            (false, EvidenceClass::ModelledUncalibrated),
            (true, EvidenceClass::ModelledUncalibrated),
        );
        assert_eq!(
            trace,
            [ok, lost, lost, failed, failed, failed, failed, failed]
        );
    }

    /// bd-8wrk: the monitor's constructor used to floor nugget and sill at 1e-6, range at 1 m, and clamp the
    /// neighbourhood to 1..=16. Out-of-domain parameters now refuse.
    #[test]
    fn test_spatial_monitor_refuses_out_of_domain_parameters() {
        let p = |name: &'static str, value: f64| Err(ScoreError::InvalidParameter { name, value });
        assert_eq!(
            SpatialFrontPitMonitor::new(0.0, 0.75, 30.0, 6, 0.02).map(|_| ()),
            p("nugget_var", 0.0)
        );
        assert_eq!(
            SpatialFrontPitMonitor::new(0.25, -1.0, 30.0, 6, 0.02).map(|_| ()),
            p("sill_var", -1.0)
        );
        assert_eq!(
            SpatialFrontPitMonitor::new(0.25, 0.75, 0.5, 6, 0.02).map(|_| ()),
            p("range_m", 0.5)
        );
        assert_eq!(
            SpatialFrontPitMonitor::new(0.25, 0.75, 30.0, 0, 0.02).map(|_| ()),
            p("max_neighbors", 0.0)
        );
        assert_eq!(
            SpatialFrontPitMonitor::new(0.25, 0.75, 30.0, 17, 0.02).map(|_| ()),
            p("max_neighbors", 17.0)
        );
        assert_eq!(
            SpatialFrontPitMonitor::new(0.25, 0.75, 30.0, 6, 1.0).map(|_| ()),
            Err(ScoreError::AlphaOutOfDomain(1.0))
        );
    }

    #[test]
    fn test_vecchia_spatial_front_pit_eprocess_gb08() {
        // 1. Under well-specified spatial innovations, monitor does not trip false alarm.
        let mut monitor_ok =
            SpatialFrontPitMonitor::new(0.25, 0.75, 30.0, 6, 0.02).expect("valid parameters");
        let mut rng = PhiloxStream::new(RngKey {
            seed_root: 777,
            partition: 1,
            domain: 1,
            var_group: 1,
            realization: 0,
            brick: 0,
        });
        for i in 0..80 {
            let x = (i as f64) * 50.0; // Wide spacing so marginal innovation ~ N(0, 1)
            let y = rng.next_normal();
            let (_, alarm) = monitor_ok
                .observe_sample(SpatialSample {
                    x,
                    y: 0.0,
                    z: 0.0,
                    prior_mean: 0.0,
                    observed_value: y,
                })
                .unwrap();
            assert!(!alarm, "unexpected false alarm under null at step {i}");
        }

        // 2. Under systematic +1.5g/t unmodeled high-grade shear zone, monitor trips alarm.
        let mut monitor_shift =
            SpatialFrontPitMonitor::new(0.25, 0.75, 30.0, 6, 0.05).expect("valid parameters");
        let mut tripped = false;
        for i in 0..60 {
            let x = (i as f64) * 10.0;
            let (_, alarm) = monitor_shift
                .observe_sample(SpatialSample {
                    x,
                    y: 0.0,
                    z: 0.0,
                    prior_mean: 1.0,
                    observed_value: 2.8,
                })
                .unwrap();
            if alarm {
                tripped = true;
                break;
            }
        }
        assert!(
            tripped,
            "Vecchia spatial PIT e-process must detect shifted bench grade"
        );
    }
}
