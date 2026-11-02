//! Frozen forecast ledger (`INV-008`), proper scoring rules (`CRPS`, log, Winkler),
//! `CalibrationFlag` auto-downgrade (`NL-8`), `REFCLASS-` $\varepsilon$-contamination (`GB-32`),
//! and Vecchia/Rosenblatt spatial PIT e-process monitoring (`GB-08`, `BD-311`).
//! Determinism: BitwisePortable (transcendentals via `fmine_core::det`; the file's three golden digests agreed on
//! x86_64-linux and aarch64-macos in CI run 41237993223 at 84f32ac, bd-hp2j.3). The calib-rest digest was
//! re-pinned on aarch64-macos when the spatial monitor's scores changed (bd-311), and an x86_64-linux rch worker
//! gave the same value; no CI run of it exists yet.

use fmine_claim::Claim;
use fmine_core::det;
use fmine_core::hash::blake3_hash;
use fmine_core::rand::{PhiloxStream, RngKey};
use fmine_evidence::{EvidenceClass, Refusal, RefusalCode};
use fmine_numerics::cholesky_solve;
use fmine_numerics::eproc::ScoreBettingEProcess;
use fmine_numerics::norm_cdf;
use fmine_units::{Length, Qty};

pub mod guidance_scores;
pub mod ledger;
pub mod record;
pub mod scoring;
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
///
/// The class is hashed into the claim's id, so the lowered claim is a new one that supersedes the old
/// (`Claim::downgraded`, §8.1); `calibration_failed` is outside the id and is set in place.
pub fn apply_calibration_gate<T: Clone>(claim: &mut Claim<T>, flag: CalibrationFlag) {
    if flag == CalibrationFlag::Calibrated {
        return;
    }
    claim.calibration_failed |= matches!(
        flag,
        CalibrationFlag::Overconfident | CalibrationFlag::Biased
    );
    if claim.evidence == EvidenceClass::ModelledCalibrated {
        *claim = claim.downgraded(EvidenceClass::ModelledUncalibrated);
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
    match SpatialFrontPitMonitor::new(0.05, 1.0, Qty::new(50.0), 8, 0.05) {
        Ok(mut monitor) => {
            for i in 0..1_000 {
                // A bias that flips at sample 200 takes the location stake to both of its limits and the pair to
                // the alarm.
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
                        monitor.location_eproc.log_wealth().to_bits(),
                        monitor.dispersion_eproc.log_wealth().to_bits(),
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
    /// Valid inputs produce a score component that cannot be represented reliably.
    Arithmetic { quantity: &'static str },
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
            Self::Arithmetic { quantity } => {
                write!(f, "score arithmetic cannot represent {quantity}")
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

/// $c_0 = \mathbb{E}_0[\tanh((Z^2 - 1)/2)]$ for $Z \sim N(0, 1)$, the centre of the dispersion score (plan §10.4).
/// Uncentred, that score has null mean $c_0 \approx -0.075$, a negative stake earns on it, and Ville's bound fails.
/// From `scripts/oracle/front_pit_constants.py` (Simpson on CPython's `tanh` and `exp`, summed exactly).
const DISPERSION_SCORE_CENTRE: f64 = -0.074_848_061_759_276_64;
/// $\mathbb{E}_0[\tanh(Z/2)^2]$: the location stake's prior second moment (same script).
const LOCATION_SCORE_SECOND_MOMENT: f64 = 0.173_516_143_432_371_87;
/// $\mathbb{E}_0[(\tanh((Z^2 - 1)/2) - c_0)^2]$: the dispersion stake's prior second moment (same script).
const DISPERSION_SCORE_SECOND_MOMENT: f64 = 0.188_861_986_137_620_65;
/// The stake truncation $c$: each wealth factor stays at or above $1 - c$. The plan's simulated detection times
/// (§10.4) are reproduced at 0.5.
const STAKE_TRUNCATION: f64 = 0.5;
/// Bounds on the dispersion score: its infimum is $\tanh(-1/2) - c_0 \approx -0.38727$ (at $z = 0$), its supremum
/// $1 - c_0$. The declared range is slightly wider, so a last-place rounding never refuses a score.
const DISPERSION_SCORE_RANGE: (f64, f64) = (-0.3873, 1.0749);

/// The location score $\psi_{loc}(z) = \tanh(z/2)$: in $(-1, 1)$, mean 0 under $N(0, 1)$ by symmetry.
fn location_score(z: f64) -> f64 {
    det::tanh(z / 2.0)
}

/// The centred dispersion score $\psi_{disp}(z) = \tanh((z^2 - 1)/2) - c_0$: mean 0 under $N(0, 1)$, positive on
/// average when the model's spread is too small.
fn dispersion_score(z: f64) -> f64 {
    det::tanh((z * z - 1.0) / 2.0) - DISPERSION_SCORE_CENTRE
}

/// Vecchia-conditioned Rosenblatt PIT sequential monitor (`Monitor::SpatialFrontPit`, `GB-08`, `BD-311`).
///
/// Each polygon or ring $y_k$, taken in availability order, is conditioned on up to `max_neighbors` nearest
/// earlier observations under an isotropic exponential covariance
/// $C(h) = \sigma_0^2 \mathbb{I}(h=0) + \sigma_1^2 \exp(-h / r)$ (the registered Vecchia joint model). The
/// standardized conditional residual $z_k = (y_k - \mu_{k|\mathcal{N}(k)}) / \sigma_{k|\mathcal{N}(k)}$ is i.i.d.
/// $N(0, 1)$ when that model is the true joint law, however correlated the polygons are (Rosenblatt 1952). Two
/// [`ScoreBettingEProcess`]es bet against that null, each at half of `alpha`: one on the location score
/// $\tanh(z/2)$, one on the centred dispersion score $\tanh((z^2 - 1)/2) - c_0$ (plan §10.4). The pair alarms when either wealth reaches $2/\alpha$, so the false-alarm
/// probability over any horizon is at most `alpha`.
#[derive(Debug, Clone)]
pub struct SpatialFrontPitMonitor {
    pub nugget_var: f64,
    pub sill_var: f64,
    pub range_m: Qty<Length>,
    pub max_neighbors: usize,
    pub history: Vec<SpatialSample>,
    pub location_eproc: ScoreBettingEProcess,
    pub dispersion_eproc: ScoreBettingEProcess,
}

impl SpatialFrontPitMonitor {
    /// Creates a new `SpatialFrontPitMonitor` with exponential variogram parameters and significance `alpha`.
    /// The nugget and sill are variances of the observed quantity, in its unit squared; the range is a length.
    ///
    /// Refuses (rather than floors or clamps, bd-8wrk) a nugget or sill below 1e-6, a range below 1 m, a
    /// neighbourhood outside 1..=16 and `alpha` outside (0, 1).
    pub fn new(
        nugget_var: f64,
        sill_var: f64,
        range_m: Qty<Length>,
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
        check("range_m", range_m.value(), range_m.value() >= 1.0)?;
        check(
            "max_neighbors",
            max_neighbors as f64,
            (1..=16).contains(&max_neighbors),
        )?;
        if !(alpha.is_finite() && alpha > 0.0 && alpha < 1.0) {
            return Err(ScoreError::AlphaOutOfDomain(alpha));
        }
        // Each monitor gets half of `alpha`. Only a subnormal `alpha` whose half rounds to 0 is refused here.
        let eproc = |(lo, hi), second_moment| {
            ScoreBettingEProcess::new(lo, hi, second_moment, STAKE_TRUNCATION, alpha * 0.5)
                .map_err(|_| ScoreError::AlphaOutOfDomain(alpha))
        };
        Ok(Self {
            nugget_var,
            sill_var,
            range_m,
            max_neighbors,
            history: Vec::new(),
            location_eproc: eproc((-1.0, 1.0), LOCATION_SCORE_SECOND_MOMENT)?,
            dispersion_eproc: eproc(DISPERSION_SCORE_RANGE, DISPERSION_SCORE_SECOND_MOMENT)?,
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
            self.sill_var * det::exp(-h / self.range_m.value())
        }
    }

    /// The registered model's predictive distribution for an observation at `(x, y, z)` with prior mean
    /// `prior_mean`, given the history: the Gaussian conditional mean and standard deviation on the nearest
    /// `max_neighbors` earlier observations. Its 80% interval is `mean ± 1.2816 sd`. Nothing is recorded.
    ///
    /// Refuses with `FMINE-REF-109` a NaN or infinite coordinate or prior mean, and with `FMINE-REF-207` a
    /// neighbour covariance that is not positive definite.
    fn predictive(&self, x: f64, y: f64, z: f64, prior_mean: f64) -> Result<(f64, f64), Refusal> {
        let fields = [("x", x), ("y", y), ("z", z), ("prior_mean", prior_mean)];
        if let Some((name, value)) = fields.into_iter().find(|(_, v)| !v.is_finite()) {
            return Err(Refusal::new(
                RefusalCode::OutsidePhysicalDomain109,
                format!("spatial sample {name} is {value}"),
            ));
        }
        let sample = SpatialSample {
            x,
            y,
            z,
            prior_mean,
            observed_value: prior_mean,
        };
        let total_var = self.nugget_var + self.sill_var;
        if self.history.is_empty() {
            Ok((sample.prior_mean, det::sqrt(total_var)))
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
            // Distance, then history index: a total order, so the k nearest and their order are those of a full
            // sort. Selecting them first keeps each step linear in the history.
            let order = |a: &(f64, usize), b: &(f64, usize)| {
                a.0.total_cmp(&b.0).then_with(|| a.1.cmp(&b.1))
            };
            let k = indexed_dists.len().min(self.max_neighbors);
            if k < indexed_dists.len() {
                indexed_dists.select_nth_unstable_by(k, order);
            }
            indexed_dists[..k].sort_by(order);
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
            Ok((mu, det::sqrt(c_var)))
        }
    }

    /// Conditions `sample` on its nearest Vecchia predecessors (the private `predictive`), bets both scores on the
    /// standardized residual, and appends `sample` to the conditioning history (`BD-311`). Returns the PIT
    /// $u = \Phi(z)$ and whether the pair has alarmed.
    ///
    /// Refuses with `FMINE-REF-109` a sample with a NaN or infinite coordinate, prior mean or value, and changes
    /// nothing (bd-a482). An infinite value used to give a PIT of 1 and join the history, so every later sample
    /// conditioned on it. A refusal from either e-process also changes nothing.
    pub fn observe_sample(&mut self, sample: SpatialSample) -> Result<(f64, bool), Refusal> {
        if !sample.observed_value.is_finite() {
            return Err(Refusal::new(
                RefusalCode::OutsidePhysicalDomain109,
                format!("spatial sample observed_value is {}", sample.observed_value),
            ));
        }
        let (cond_mean, cond_std) =
            self.predictive(sample.x, sample.y, sample.z, sample.prior_mean)?;
        let z_star = (sample.observed_value - cond_mean) / cond_std;
        // Finite inputs can still overflow the conditional mean and give a NaN residual. Refuse it before either
        // e-process moves.
        if z_star.is_nan() {
            return Err(Refusal::new(
                RefusalCode::NumericalCertificateFailed207,
                format!(
                    "spatial residual is NaN (observed {}, prior mean {})",
                    sample.observed_value, sample.prior_mean
                ),
            ));
        }
        let refuse = |e: fmine_numerics::eproc::EProcError| {
            Refusal::new(RefusalCode::NumericalCertificateFailed207, e.to_string())
        };
        // Both bets succeed or neither is kept.
        let mut location = self.location_eproc.clone();
        location.observe(location_score(z_star)).map_err(refuse)?;
        let mut dispersion = self.dispersion_eproc.clone();
        dispersion
            .observe(dispersion_score(z_star))
            .map_err(refuse)?;
        self.location_eproc = location;
        self.dispersion_eproc = dispersion;
        self.history.push(sample);

        let alarm = self.location_eproc.rejected_null() || self.dispersion_eproc.rejected_null();
        Ok((norm_cdf(z_star), alarm))
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
    use fmine_numerics::eproc::BettingEProcess;

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
    // bd-311 replaced the spatial monitor's scores (PIT and |PIT - 1/2|) with the plan's tanh scores and aGRAPA
    // stakes. The probe's PIT bits are unchanged; its wealth and alarm bits are not. Re-pinned on aarch64-macos;
    // the value before was 0652013fb792cbe11d0da7eb5b378ca8f2af427cdd42447f3a9fd792348af9c5.
    const CALIB_REST_10K: &str = "9b06b492532266132b0df763ea0937e90f56bde07ef230789678c7a36134aedd";

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
        // The lowered class is a new claim that supersedes the old one (§8.1), and its id names its body.
        assert_eq!(claim.supersedes, Some(unscored.claim_id));
        assert_ne!(claim.claim_id, unscored.claim_id);
        assert_eq!(claim.check_id(), Ok(()));
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
            SpatialFrontPitMonitor::new(0.0, 0.75, Qty::new(30.0), 6, 0.02).map(|_| ()),
            p("nugget_var", 0.0)
        );
        assert_eq!(
            SpatialFrontPitMonitor::new(0.25, -1.0, Qty::new(30.0), 6, 0.02).map(|_| ()),
            p("sill_var", -1.0)
        );
        assert_eq!(
            SpatialFrontPitMonitor::new(0.25, 0.75, Qty::new(0.5), 6, 0.02).map(|_| ()),
            p("range_m", 0.5)
        );
        assert_eq!(
            SpatialFrontPitMonitor::new(0.25, 0.75, Qty::new(30.0), 0, 0.02).map(|_| ()),
            p("max_neighbors", 0.0)
        );
        assert_eq!(
            SpatialFrontPitMonitor::new(0.25, 0.75, Qty::new(30.0), 17, 0.02).map(|_| ()),
            p("max_neighbors", 17.0)
        );
        assert_eq!(
            SpatialFrontPitMonitor::new(0.25, 0.75, Qty::new(30.0), 6, 1.0).map(|_| ()),
            Err(ScoreError::AlphaOutOfDomain(1.0))
        );
        // bd-a482: half of the smallest subnormal rounds to 0, which the e-processes refuse instead of clamping.
        assert_eq!(
            SpatialFrontPitMonitor::new(0.25, 0.75, Qty::new(30.0), 6, f64::from_bits(1))
                .map(|_| ()),
            Err(ScoreError::AlphaOutOfDomain(f64::from_bits(1)))
        );
    }

    /// bd-a482: a NaN value used to reach the e-processes as a NaN PIT and turn their wealth to NaN, and an
    /// infinite one gave a PIT of 1 and joined the history.
    #[test]
    fn test_spatial_monitor_refuses_non_finite_sample_and_changes_nothing() {
        let mut monitor = SpatialFrontPitMonitor::new(0.25, 0.75, Qty::new(30.0), 6, 0.05)
            .expect("valid parameters");
        let sample = |x, observed_value| SpatialSample {
            x,
            y: 0.0,
            z: 0.0,
            prior_mean: 1.0,
            observed_value,
        };
        monitor
            .observe_sample(sample(0.0, 1.4))
            .expect("finite sample");
        let eprocs = (
            monitor.location_eproc.clone(),
            monitor.dispersion_eproc.clone(),
        );
        for bad in [
            sample(0.0, f64::NAN),
            sample(0.0, f64::INFINITY),
            sample(f64::NAN, 1.0),
        ] {
            let refusal = monitor.observe_sample(bad).expect_err("refused");
            assert_eq!(refusal.code, RefusalCode::OutsidePhysicalDomain109.as_str());
            assert_eq!(
                (
                    monitor.location_eproc.clone(),
                    monitor.dispersion_eproc.clone()
                ),
                eprocs
            );
            assert_eq!(monitor.history.len(), 1);
            assert_eq!(monitor.location_eproc.n_steps(), 1);
        }
        monitor
            .observe_sample(sample(5.0, 0.6))
            .expect("finite sample");
        assert_eq!(monitor.history.len(), 2);
    }

    /// Simpson on [-12, 12] with compensated summation, on `fmine_core::det`.
    fn normal_expectation(f: impl Fn(f64) -> f64) -> f64 {
        let (a, n) = (-12.0_f64, 240_000_u32);
        let h = 24.0 / f64::from(n);
        let (mut sum, mut carry) = (0.0_f64, 0.0_f64);
        for i in 0..=n {
            let z = a + f64::from(i) * h;
            let weight = if i == 0 || i == n {
                1.0
            } else if i % 2 == 1 {
                4.0
            } else {
                2.0
            };
            let term = weight * f(z) * det::exp(-0.5 * z * z);
            let next = sum + term;
            carry += if sum.abs() >= term.abs() {
                (sum - next) + term
            } else {
                (term - next) + sum
            };
            sum = next;
        }
        (sum + carry) * h / 3.0 / det::sqrt(2.0 * std::f64::consts::PI)
    }

    /// The scores' constants against `scripts/oracle/front_pit_constants.py`, and that both scores have null
    /// mean 0 once centred.
    #[test]
    fn test_front_pit_score_constants_known_answers() {
        let c0 = normal_expectation(|z| det::tanh((z * z - 1.0) / 2.0));
        let q_loc = normal_expectation(|z| location_score(z) * location_score(z));
        let q_disp = normal_expectation(|z| dispersion_score(z) * dispersion_score(z));
        for (name, got, pinned) in [
            ("c0", c0, DISPERSION_SCORE_CENTRE),
            ("q_loc", q_loc, LOCATION_SCORE_SECOND_MOMENT),
            ("q_disp", q_disp, DISPERSION_SCORE_SECOND_MOMENT),
        ] {
            assert!((got - pinned).abs() <= 1e-14, "{name}: {got} vs {pinned}");
        }
        assert!(normal_expectation(location_score).abs() <= 1e-15);
        assert!(normal_expectation(dispersion_score).abs() <= 1e-14);
        // The declared dispersion range holds the score's infimum (z = 0) and supremum.
        let (lo, hi) = DISPERSION_SCORE_RANGE;
        assert!(lo < dispersion_score(0.0) && dispersion_score(1e300) < hi);
    }

    /// The acceptance test below draws its truth from `predictive`, so it cannot see a wrong predictive. These are
    /// the Gaussian conditionals worked by hand: with total variance t = nugget + sill and C(h) = sill e^(-h/r),
    /// one neighbour gives mean prior + C(h) r1 / t and variance t - C(h)^2 / t; two give the 2 x 2 solve.
    #[test]
    fn test_front_pit_predictive_known_answers() {
        let (nugget, sill, range) = (0.2, 0.8, 40.0);
        let total = nugget + sill;
        let c = |h: f64| sill * (-h / range).exp();
        let mut monitor =
            SpatialFrontPitMonitor::new(nugget, sill, Qty::new(range), 1, 0.05).expect("valid");
        assert_eq!(
            monitor.predictive(3.0, 4.0, 0.0, 2.5).expect("finite"),
            (2.5, total.sqrt())
        );
        let at = |x, y, prior_mean, observed_value| SpatialSample {
            x,
            y,
            z: 0.0,
            prior_mean,
            observed_value,
        };
        monitor
            .observe_sample(at(0.0, 0.0, 1.0, 1.6))
            .expect("finite");
        // The farther point, 50 m away, must not be used with one neighbour.
        monitor
            .observe_sample(at(60.0, 0.0, 1.0, -3.0))
            .expect("finite");
        let (mean, sd) = monitor.predictive(6.0, 8.0, 0.0, 2.0).expect("finite");
        let ch = c(10.0);
        assert!((mean - (2.0 + ch * 0.6 / total)).abs() <= 1e-14, "{mean}");
        assert!(
            (sd - (total - ch * ch / total).sqrt()).abs() <= 1e-14,
            "{sd}"
        );

        let mut two =
            SpatialFrontPitMonitor::new(nugget, sill, Qty::new(range), 2, 0.05).expect("valid");
        two.observe_sample(at(0.0, 0.0, 1.0, 1.6)).expect("finite");
        two.observe_sample(at(20.0, 0.0, 1.0, 0.7)).expect("finite");
        let (mean, sd) = two.predictive(10.0, 0.0, 0.0, 1.0).expect("finite");
        // Symmetric case: both neighbours 10 m away, 20 m apart, so the weights are equal: w = C(10) / (t + C(20)).
        let w = c(10.0) / (total + c(20.0));
        assert!((mean - (1.0 + w * (0.6 - 0.3))).abs() <= 1e-14, "{mean}");
        assert!(
            (sd - (total - 2.0 * w * c(10.0)).sqrt()).abs() <= 1e-14,
            "{sd}"
        );

        let refusal = two
            .predictive(f64::NAN, 0.0, 0.0, 1.0)
            .expect_err("refused");
        assert_eq!(refusal.code, RefusalCode::OutsidePhysicalDomain109.as_str());
    }

    /// Seed of the bd-311 acceptance runs, fixed before the first run.
    const FRONT_PIT_SEED: u64 = 0x0042_4433_3131;
    /// From `scripts/oracle/front_pit_constants.py`: the location shift and the spread factor at which the
    /// model's 80% interval covers 58%.
    const SHIFT_58: f64 = 1.054_676_155_906_398_2;
    const SPREAD_58: f64 = 1.589_183_779_920_438_2;

    fn front_key(partition: u32, run: u32) -> RngKey {
        RngKey {
            seed_root: FRONT_PIT_SEED,
            partition,
            domain: 1,
            var_group: 1,
            realization: run,
            brick: 0,
        }
    }

    /// Polygon `k` of a bench mined row by row: 20 polygons a row at 10 m, rows 10 m apart, on a prior-mean trend.
    fn polygon(k: usize) -> (f64, f64, f64) {
        let (x, y) = ((k % 20) as f64 * 10.0, (k / 20) as f64 * 10.0);
        (x, y, 2.0 + 0.002 * (x + y))
    }

    /// One run of the monitor on synthetic truth: each polygon is drawn as `mean + sd * z` from the monitor's own
    /// predictive, so with `z ~ N(0, 1)` the run is an exact draw from the registered Vecchia joint law. Returns
    /// the residuals and the first alarm (1-based). Checks at every polygon that the PIT is `Phi(z)`.
    fn front_run(
        draw: &mut dyn FnMut() -> f64,
        horizon: usize,
        stop_at_alarm: bool,
    ) -> (Vec<f64>, Option<usize>) {
        let mut monitor = SpatialFrontPitMonitor::new(0.2, 0.8, Qty::new(40.0), 8, 0.05)
            .expect("valid parameters");
        let mut zs = Vec::with_capacity(horizon);
        let mut first = None;
        for k in 0..horizon {
            let (x, y, prior_mean) = polygon(k);
            let (mean, sd) = monitor.predictive(x, y, 0.0, prior_mean).expect("finite");
            let z = draw();
            let (u, alarm) = monitor
                .observe_sample(SpatialSample {
                    x,
                    y,
                    z: 0.0,
                    prior_mean,
                    observed_value: mean + sd * z,
                })
                .expect("finite");
            assert!(
                (u - norm_cdf(z)).abs() <= 1e-12,
                "polygon {k}: PIT {u}, Phi(z) {}",
                norm_cdf(z)
            );
            zs.push(z);
            if alarm && first.is_none() {
                first = Some(k + 1);
                if stop_at_alarm {
                    break;
                }
            }
        }
        (zs, first)
    }

    /// First alarm (1-based) of a pair of e-processes fed `scores(z)` for each residual.
    fn pair_alarm<A, B>(
        zs: &[f64],
        mut a: A,
        mut b: B,
        step: impl Fn(&mut A, &mut B, f64) -> bool,
    ) -> Option<usize> {
        zs.iter()
            .position(|&z| step(&mut a, &mut b, z))
            .map(|i| i + 1)
    }

    fn median(times: &[Option<usize>], censor: usize) -> usize {
        let mut t: Vec<usize> = times.iter().map(|x| x.unwrap_or(censor + 1)).collect();
        t.sort_unstable();
        t[t.len() / 2]
    }

    /// bd-311 acceptance (plan §39 BD-311: median alarm ≤ 25 polygons for the 58%-coverage location shift and
    /// ≤ 40 for the dispersion shift; null false-alarm rate ≤ α over 600 polygons). Declared before the first run:
    /// - end to end, 200 null runs of 600 polygons through the monitor: the PIT is `Phi(z)` at every polygon, and
    ///   each run's first alarm is the one the pair of score e-processes gives on its residuals;
    /// - those e-processes, configured as the monitor configures them, on 20,000 null runs of 600 i.i.d. N(0, 1)
    ///   residuals: false-alarm rate ≤ α = 0.05;
    /// - end to end, 200 runs each: every run alarms within 300 polygons; median ≤ 25 under z ~ N(1.0547, 1) and
    ///   ≤ 40 under z ~ N(0, 1.5892²), the two shifts that take 80% coverage to 58%.
    ///
    /// Added after the first run (694 of 20,000 alarmed): the one-sided 95% Clopper–Pearson upper bound on that
    /// false-alarm rate is ≤ α too, the statistic GATE-PLUMB-E5 asks of a correctly specified control.
    ///
    /// Planted negatives on the same residuals: the dispersion score left uncentred breaks the false-alarm bound,
    /// and the scores this monitor used before bd-311 (the PIT and |PIT - 1/2| on `BettingEProcess`) miss the
    /// dispersion criterion.
    #[test]
    fn test_spatial_front_pit_detects_58pct_coverage_shift_and_keeps_ville_bound() {
        let q90 = 1.281_551_565_544_600_4;
        let cover = |lo: f64, hi: f64| norm_cdf(hi) - norm_cdf(lo);
        assert!((cover(-q90 - SHIFT_58, q90 - SHIFT_58) - 0.58).abs() < 1e-9);
        assert!((cover(-q90 / SPREAD_58, q90 / SPREAD_58) - 0.58).abs() < 1e-9);

        let fresh = SpatialFrontPitMonitor::new(0.2, 0.8, Qty::new(40.0), 8, 0.05)
            .expect("valid parameters");
        let pair = |zs: &[f64], centre: f64| {
            let disp_range = (det::tanh(-0.5) - centre, 1.0 - centre);
            let disp = ScoreBettingEProcess::new(
                disp_range.0,
                disp_range.1,
                DISPERSION_SCORE_SECOND_MOMENT,
                STAKE_TRUNCATION,
                0.025,
            )
            .expect("valid");
            pair_alarm(zs, fresh.location_eproc.clone(), disp, |loc, disp, z| {
                loc.observe(location_score(z)).expect("in range");
                disp.observe(det::tanh((z * z - 1.0) / 2.0) - centre)
                    .expect("in range");
                loc.rejected_null() || disp.rejected_null()
            })
        };
        let monitor_pair = |zs: &[f64]| {
            pair_alarm(
                zs,
                fresh.location_eproc.clone(),
                fresh.dispersion_eproc.clone(),
                |loc, disp, z| {
                    loc.observe(location_score(z)).expect("in range");
                    disp.observe(dispersion_score(z)).expect("in range");
                    loc.rejected_null() || disp.rejected_null()
                },
            )
        };
        let before_bd311 = |zs: &[f64]| {
            let eproc = |m| BettingEProcess::new(m, 0.025).expect("valid");
            pair_alarm(zs, eproc(0.5), eproc(0.25), |mean, spread, z| {
                let u = norm_cdf(z);
                mean.observe(u).expect("in [0, 1]");
                spread.observe((u - 0.5).abs()).expect("in [0, 1]");
                mean.rejected_null() || spread.rejected_null()
            })
        };

        // End to end under the null.
        let mut e2e_alarms = 0;
        for run in 0..200 {
            let mut rng = PhiloxStream::new(front_key(1, run));
            let (zs, first) = front_run(&mut || rng.next_normal(), 600, false);
            assert_eq!(first, monitor_pair(&zs), "null run {run}");
            e2e_alarms += usize::from(first.is_some());
        }

        // The false-alarm rate, on i.i.d. residuals.
        let (runs, mut alarms, mut uncentred) = (20_000_u32, 0_u32, 0_u32);
        for run in 0..runs {
            let mut rng = PhiloxStream::new(front_key(2, run));
            let zs: Vec<f64> = (0..600).map(|_| rng.next_normal()).collect();
            alarms += u32::from(monitor_pair(&zs).is_some());
            uncentred += u32::from(pair(&zs, 0.0).is_some());
        }
        let fpr = f64::from(alarms) / f64::from(runs);
        let (_, fpr_upper) = fmine_numerics::exact_feas::power::clopper_pearson(
            u64::from(alarms),
            u64::from(runs),
            0.10,
        )
        .expect("alarms <= runs");
        let fpr_uncentred = f64::from(uncentred) / f64::from(runs);

        // End to end under the two 58% alternatives.
        let shifted = |partition: u32, shift: f64, spread: f64| {
            let mut now = Vec::new();
            let mut old = Vec::new();
            for run in 0..200 {
                let mut rng = PhiloxStream::new(front_key(partition, run));
                let (_, first) = front_run(&mut || shift + spread * rng.next_normal(), 300, true);
                now.push(first);
                let mut rng = PhiloxStream::new(front_key(partition, run));
                let zs: Vec<f64> = (0..300)
                    .map(|_| shift + spread * rng.next_normal())
                    .collect();
                old.push(before_bd311(&zs));
            }
            (now, old)
        };
        let (location, location_old) = shifted(3, SHIFT_58, 1.0);
        let (dispersion, dispersion_old) = shifted(4, 0.0, SPREAD_58);
        let (m_loc, m_disp) = (median(&location, 300), median(&dispersion, 300));
        let (o_loc, o_disp) = (median(&location_old, 300), median(&dispersion_old, 300));
        println!(
            "bd-311: null end to end {e2e_alarms}/200 alarmed; false-alarm rate {fpr:.4} over {runs} runs of 600, \
             one-sided 95% upper bound {fpr_upper:.4} (uncentred dispersion score {fpr_uncentred:.4}); median \
             alarm {m_loc} polygons (location), {m_disp} (dispersion); before bd-311 {o_loc} and {o_disp}"
        );
        assert!(fpr <= 0.05, "false-alarm rate {fpr}");
        assert!(fpr_upper <= 0.05, "false-alarm upper bound {fpr_upper}");
        assert!(location.iter().chain(&dispersion).all(Option::is_some));
        assert!(m_loc <= 25, "location median {m_loc}");
        assert!(m_disp <= 40, "dispersion median {m_disp}");
        assert!(fpr_uncentred > 0.05, "uncentred {fpr_uncentred}");
        assert!(o_disp > 40, "the scores before bd-311 gave {o_disp}");
    }

    #[test]
    fn test_vecchia_spatial_front_pit_eprocess_gb08() {
        // 1. Under well-specified spatial innovations, monitor does not trip false alarm.
        let mut monitor_ok = SpatialFrontPitMonitor::new(0.25, 0.75, Qty::new(30.0), 6, 0.02)
            .expect("valid parameters");
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
        let mut monitor_shift = SpatialFrontPitMonitor::new(0.25, 0.75, Qty::new(30.0), 6, 0.05)
            .expect("valid parameters");
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
