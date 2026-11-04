//! Original FrankenMining code (not lifted). Upstream analogue: frankensim crates/fs-eproc/src/lib.rs @ 20463525d (not copied).
//! Anytime-valid betting martingale e-processes (`BD-022`, `GB-08`).
//! Determinism: DeterministicPerIsa (no cross-host digest test; plan §29.2, bd-hp2j.1).

use fmine_core::det;

pub mod coverage;

/// Why a [`BettingEProcess`] or [`ScoreBettingEProcess`] refused (bd-a482). Nothing is clamped into range.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EProcError {
    /// `null_mean` is not a finite value in `(0, 1)`.
    NullMeanOutOfDomain(f64),
    /// `alpha` is not a finite value in `(0, 1)`.
    AlphaOutOfDomain(f64),
    /// An observation is not a finite value in `[0, 1]`.
    ObservationOutOfDomain(f64),
    /// A score range that does not hold 0 strictly inside, or is not finite.
    ScoreRangeOutOfDomain { lo: f64, hi: f64 },
    /// A prior second moment that is not finite and positive.
    PriorSecondMomentOutOfDomain(f64),
    /// A stake truncation that is not a finite value in `(0, 1)`.
    TruncationOutOfDomain(f64),
    /// A score outside the declared range, or not finite.
    ScoreOutOfRange { score: f64, lo: f64, hi: f64 },
}

impl std::fmt::Display for EProcError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NullMeanOutOfDomain(m) => write!(f, "null mean must lie in (0, 1), got {m}"),
            Self::AlphaOutOfDomain(a) => write!(f, "alpha must lie in (0, 1), got {a}"),
            Self::ObservationOutOfDomain(u) => {
                write!(f, "an observation must lie in [0, 1], got {u}")
            }
            Self::ScoreRangeOutOfDomain { lo, hi } => {
                write!(
                    f,
                    "a score range must satisfy lo < 0 < hi, got [{lo}, {hi}]"
                )
            }
            Self::PriorSecondMomentOutOfDomain(q) => {
                write!(
                    f,
                    "the prior second moment must be finite and positive, got {q}"
                )
            }
            Self::TruncationOutOfDomain(c) => {
                write!(f, "the stake truncation must lie in (0, 1), got {c}")
            }
            Self::ScoreOutOfRange { score, lo, hi } => {
                write!(f, "a score must lie in [{lo}, {hi}], got {score}")
            }
        }
    }
}

impl std::error::Error for EProcError {}

/// Anytime-valid betting martingale e-process for testing $H_0: \mathbb{E}[U_t \mid U_{<t}] = \mu_0$
/// on bounded or PIT-transformed sequential observations $u_t \in [0, 1]$ (`GB-08`).
///
/// The bet fraction $\lambda_t$ is predictable (it reads only $u_{<t}$) and is held to $[-0.75, 0.75]$, so each
/// factor $1 + \lambda_t (u_t - \mu_0) \ge 0.25$ and $\mathbb{E}_0[E_t] = 1$. Its sign follows the running mean,
/// so the null is two-sided.
#[derive(Debug, Clone, PartialEq)]
pub struct BettingEProcess {
    pub null_mean: f64,
    pub alpha: f64,
    pub wealth: f64,
    pub max_wealth: f64,
    pub n_steps: u64,
    running_diff_sum: f64,
    running_var_sum: f64,
}

impl BettingEProcess {
    /// A `BettingEProcess` testing `E[U_t] = null_mean` at significance level `alpha`.
    ///
    /// # Errors
    /// [`EProcError::NullMeanOutOfDomain`] or [`EProcError::AlphaOutOfDomain`] for a value outside `(0, 1)` or
    /// not finite.
    pub fn new(null_mean: f64, alpha: f64) -> Result<Self, EProcError> {
        if !(null_mean.is_finite() && null_mean > 0.0 && null_mean < 1.0) {
            return Err(EProcError::NullMeanOutOfDomain(null_mean));
        }
        if !(alpha.is_finite() && alpha > 0.0 && alpha < 1.0) {
            return Err(EProcError::AlphaOutOfDomain(alpha));
        }
        Ok(Self {
            null_mean,
            alpha,
            wealth: 1.0,
            max_wealth: 1.0,
            n_steps: 0,
            running_diff_sum: 0.0,
            running_var_sum: 0.08333333333333333, // Uniform(0,1) prior variance 1/12
        })
    }

    /// Rejection threshold $1 / \alpha$ guaranteed by Ville's inequality.
    #[must_use]
    pub fn threshold(&self) -> f64 {
        1.0 / self.alpha
    }

    /// Updates the e-process with an observation `u` and returns the updated wealth $E_t$.
    ///
    /// # Errors
    /// [`EProcError::ObservationOutOfDomain`] for `u` outside `[0, 1]` or not finite. The state is unchanged.
    pub fn observe(&mut self, u: f64) -> Result<f64, EProcError> {
        if !(0.0..=1.0).contains(&u) {
            return Err(EProcError::ObservationOutOfDomain(u));
        }
        let diff = u - self.null_mean;

        // Predictable bet fraction, from the history before `u`. The bound is part of the betting rule, not a
        // repair of an input: it keeps every factor at or above 0.25, since |diff| < 1.
        let lambda = (self.running_diff_sum / (self.running_var_sum + 1.0)).clamp(-0.75, 0.75);
        let factor = 1.0 + lambda * diff;

        self.wealth *= factor;
        if self.wealth > self.max_wealth {
            self.max_wealth = self.wealth;
        }
        self.n_steps += 1;
        self.running_diff_sum += diff;
        self.running_var_sum += diff * diff;
        Ok(self.wealth)
    }

    /// Returns `true` if the running maximum wealth has crossed Ville's threshold $1 / \alpha$.
    #[must_use]
    pub fn rejected_null(&self) -> bool {
        self.max_wealth >= self.threshold()
    }
}

/// Anytime-valid betting e-process for $H_0: \mathbb{E}[\psi_t \mid \psi_{<t}] = 0$ on a score bounded in
/// `[lo, hi]` with `lo < 0 < hi` (plan §10.4, `GB-08`; Waudby-Smith and Ramdas 2024).
///
/// Wealth is $M_t = \prod_{s \le t} (1 + \lambda_s \psi_s)$. The stake is aGRAPA-type: the running mean of the
/// scores over their running second moment, $\lambda_t = \sum_{s<t} \psi_s / (q_0 + \sum_{s<t} \psi_s^2)$, which
/// counts one prior pseudo-observation at the null (score 0, second moment `prior_second_moment`). It reads only
/// earlier scores, so it is predictable. It is held to $[-c/\mathrm{hi}, c/|\mathrm{lo}|]$ for the truncation
/// $c \in (0, 1)$, so each factor is at least $1 - c > 0$ and $M_t$ is a nonnegative martingale under the null:
/// $P_0(\sup_t M_t \ge 1/\alpha) \le \alpha$ (Ville). The stake's sign follows the running mean, so the test is
/// two-sided.
///
/// Wealth is kept as its logarithm, so a long null run cannot underflow it to 0.
#[derive(Debug, Clone, PartialEq)]
pub struct ScoreBettingEProcess {
    lo: f64,
    hi: f64,
    prior_second_moment: f64,
    truncation: f64,
    alpha: f64,
    log_wealth: f64,
    max_log_wealth: f64,
    n_steps: u64,
    sum: f64,
    sum_sq: f64,
}

impl ScoreBettingEProcess {
    /// An e-process on scores in `[lo, hi]` at level `alpha`.
    ///
    /// # Errors
    /// [`EProcError::ScoreRangeOutOfDomain`] unless `lo < 0 < hi`, both finite;
    /// [`EProcError::PriorSecondMomentOutOfDomain`] unless `prior_second_moment` is finite and positive;
    /// [`EProcError::TruncationOutOfDomain`] or [`EProcError::AlphaOutOfDomain`] for a value outside `(0, 1)`.
    pub fn new(
        lo: f64,
        hi: f64,
        prior_second_moment: f64,
        truncation: f64,
        alpha: f64,
    ) -> Result<Self, EProcError> {
        if !(lo.is_finite() && hi.is_finite() && lo < 0.0 && hi > 0.0) {
            return Err(EProcError::ScoreRangeOutOfDomain { lo, hi });
        }
        if !(prior_second_moment.is_finite() && prior_second_moment > 0.0) {
            return Err(EProcError::PriorSecondMomentOutOfDomain(
                prior_second_moment,
            ));
        }
        if !(truncation.is_finite() && truncation > 0.0 && truncation < 1.0) {
            return Err(EProcError::TruncationOutOfDomain(truncation));
        }
        if !(alpha.is_finite() && alpha > 0.0 && alpha < 1.0) {
            return Err(EProcError::AlphaOutOfDomain(alpha));
        }
        Ok(Self {
            lo,
            hi,
            prior_second_moment,
            truncation,
            alpha,
            log_wealth: 0.0,
            max_log_wealth: 0.0,
            n_steps: 0,
            sum: 0.0,
            sum_sq: 0.0,
        })
    }

    /// The stake $\lambda$ the next score will be bet at. It depends only on the scores already observed.
    #[must_use]
    pub fn stake(&self) -> f64 {
        let raw = self.sum / (self.prior_second_moment + self.sum_sq);
        // The bound is part of the betting rule, not a repair of an input: it keeps every factor at or above
        // 1 - truncation.
        raw.clamp(-self.truncation / self.hi, self.truncation / -self.lo)
    }

    /// Bets on `score` and returns the new log wealth.
    ///
    /// # Errors
    /// [`EProcError::ScoreOutOfRange`] for a score outside `[lo, hi]` or not finite. The state is unchanged.
    pub fn observe(&mut self, score: f64) -> Result<f64, EProcError> {
        if !(score >= self.lo && score <= self.hi) {
            return Err(EProcError::ScoreOutOfRange {
                score,
                lo: self.lo,
                hi: self.hi,
            });
        }
        self.log_wealth += det::ln(1.0 + self.stake() * score);
        if self.log_wealth > self.max_log_wealth {
            self.max_log_wealth = self.log_wealth;
        }
        self.n_steps += 1;
        self.sum += score;
        self.sum_sq += score * score;
        Ok(self.log_wealth)
    }

    /// The natural logarithm of the current wealth.
    #[must_use]
    pub fn log_wealth(&self) -> f64 {
        self.log_wealth
    }

    /// The largest log wealth reached so far (0 before any score).
    #[must_use]
    pub fn max_log_wealth(&self) -> f64 {
        self.max_log_wealth
    }

    /// The number of scores observed.
    #[must_use]
    pub fn n_steps(&self) -> u64 {
        self.n_steps
    }

    /// Ville's threshold $1 / \alpha$ on the log scale, $-\ln \alpha$.
    #[must_use]
    pub fn log_threshold(&self) -> f64 {
        -det::ln(self.alpha)
    }

    /// `true` once the running maximum wealth has reached $1 / \alpha$.
    #[must_use]
    pub fn rejected_null(&self) -> bool {
        self.max_log_wealth >= self.log_threshold()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fmine_core::rand::{PhiloxStream, RngKey};

    #[test]
    fn test_ville_inequality_type1_error_bound() {
        let alpha = 0.05;
        let n_trials = 500;
        let horizon = 200;
        let mut false_positives = 0;

        for trial in 0..n_trials {
            let mut rng = PhiloxStream::new(RngKey {
                seed_root: 1001,
                partition: 1,
                domain: 1,
                var_group: 1,
                realization: trial as u32,
                brick: 0,
            });
            let mut ep = BettingEProcess::new(0.5, alpha).expect("valid parameters");
            for _ in 0..horizon {
                let u = rng.next_f64_open01();
                ep.observe(u).expect("u in (0, 1)");
            }
            if ep.rejected_null() {
                false_positives += 1;
            }
        }
        let empirical_fpr = (false_positives as f64) / (n_trials as f64);
        assert!(
            empirical_fpr <= alpha + 0.02,
            "Ville Type-I error bound violated: empirical FPR {empirical_fpr} > {alpha}"
        );

        // Under alternative E[U] ~ 0.75, e-process must reject decisively.
        let mut ep_alt = BettingEProcess::new(0.5, 0.01).expect("valid parameters");
        let mut rng_alt = PhiloxStream::new(RngKey {
            seed_root: 42,
            partition: 2,
            domain: 1,
            var_group: 1,
            realization: 0,
            brick: 0,
        });
        for _ in 0..150 {
            let u = 0.5 + 0.5 * rng_alt.next_f64_open01();
            ep_alt.observe(u).expect("u in (0.5, 1)");
        }
        assert!(
            ep_alt.rejected_null(),
            "must reject under shifted mean 0.75"
        );
    }

    #[test]
    fn test_eproc_refuses_out_of_domain_parameters_and_observations() {
        for m in [0.0, 1.0, -0.1, 1.5, f64::NAN, f64::INFINITY] {
            assert!(
                matches!(
                    BettingEProcess::new(m, 0.05),
                    Err(EProcError::NullMeanOutOfDomain(_))
                ),
                "null mean {m}"
            );
        }
        for a in [0.0, 1.0, -0.01, 2.0, f64::NAN, f64::NEG_INFINITY] {
            assert!(
                matches!(
                    BettingEProcess::new(0.5, a),
                    Err(EProcError::AlphaOutOfDomain(_))
                ),
                "alpha {a}"
            );
        }
        // Before bd-a482, alpha 0.9 became 0.5 and a null mean of 0.995 became 0.99; both now stand as given.
        let ep = BettingEProcess::new(0.995, 0.9).expect("valid parameters");
        assert_eq!((ep.null_mean, ep.alpha), (0.995, 0.9));

        let mut ep = BettingEProcess::new(0.5, 0.05).expect("valid parameters");
        ep.observe(0.9).expect("in range");
        let before = ep.clone();
        for u in [1.3, -0.1, f64::NAN, f64::INFINITY] {
            match ep.observe(u) {
                Err(EProcError::ObservationOutOfDomain(v)) => {
                    assert!(v == u || (v.is_nan() && u.is_nan()), "{v} vs {u}");
                }
                other => panic!("observation {u} must refuse, got {other:?}"),
            }
            assert_eq!(ep, before, "a refused observation must change nothing");
        }
        // The endpoints are observations, not errors.
        ep.observe(0.0).expect("0 is in [0, 1]");
        ep.observe(1.0).expect("1 is in [0, 1]");
    }

    fn score_key(partition: u32, realization: u32) -> RngKey {
        RngKey {
            seed_root: 0x0053_434f_5245,
            partition,
            domain: 1,
            var_group: 1,
            realization,
            brick: 0,
        }
    }

    #[test]
    fn test_score_eproc_refuses_out_of_domain_parameters_and_scores() {
        let new = ScoreBettingEProcess::new;
        for (lo, hi) in [
            (0.0, 1.0),
            (-1.0, 0.0),
            (1.0, 2.0),
            (f64::NAN, 1.0),
            (-1.0, f64::INFINITY),
        ] {
            assert!(
                matches!(
                    new(lo, hi, 0.2, 0.5, 0.05),
                    Err(EProcError::ScoreRangeOutOfDomain { .. })
                ),
                "[{lo}, {hi}]"
            );
        }
        for q in [0.0, -0.1, f64::NAN, f64::INFINITY] {
            assert!(matches!(
                new(-1.0, 1.0, q, 0.5, 0.05),
                Err(EProcError::PriorSecondMomentOutOfDomain(_))
            ));
        }
        for c in [0.0, 1.0, -0.5, 1.5, f64::NAN] {
            assert!(matches!(
                new(-1.0, 1.0, 0.2, c, 0.05),
                Err(EProcError::TruncationOutOfDomain(_))
            ));
        }
        for a in [0.0, 1.0, f64::NAN] {
            assert!(matches!(
                new(-1.0, 1.0, 0.2, 0.5, a),
                Err(EProcError::AlphaOutOfDomain(_))
            ));
        }
        let mut ep = new(-0.4, 1.1, 0.2, 0.5, 0.05).expect("valid parameters");
        ep.observe(0.7).expect("in range");
        let before = ep.clone();
        for s in [-0.41, 1.2, f64::NAN, f64::NEG_INFINITY] {
            assert!(matches!(
                ep.observe(s),
                Err(EProcError::ScoreOutOfRange { .. })
            ));
            assert_eq!(ep, before, "a refused score must change nothing");
        }
        ep.observe(-0.4).expect("lo is in range");
        ep.observe(1.1).expect("hi is in range");
        assert_eq!(ep.n_steps(), 3);
    }

    /// The stake is fixed before the score is seen, and held so that every factor is at least 1 - c.
    #[test]
    fn test_score_eproc_stake_is_predictable_and_bounded() {
        let mut ep =
            ScoreBettingEProcess::new(-0.4, 1.1, 0.2, 0.5, 0.05).expect("valid parameters");
        assert_eq!(
            ep.stake(),
            0.0,
            "the prior pseudo-observation sits at the null"
        );
        for s in [0.3, -0.1, 0.9, 0.2] {
            ep.observe(s).expect("in range");
        }
        let stake = ep.stake();
        for s in [-0.4, 0.0, 1.1] {
            let mut next = ep.clone();
            let before = next.log_wealth();
            next.observe(s).expect("in range");
            assert_eq!(next.log_wealth(), before + det::ln(1.0 + stake * s));
        }
        // A constant score s has Kelly stake 1/s: 0.3 asks for about 3.3, over the cap c/|lo| = 1.25.
        let mut up = ep.clone();
        for _ in 0..50 {
            up.observe(0.3).expect("in range");
        }
        assert_eq!(up.stake(), 0.5 / 0.4);
        let mut down = ep;
        for _ in 0..50 {
            down.observe(-0.4).expect("in range");
        }
        assert_eq!(down.stake(), -0.5 / 1.1);
        for s in [-0.4, 1.1] {
            assert!(1.0 + up.stake() * s >= 0.5 && 1.0 + down.stake() * s >= 0.5);
        }
    }

    /// An alarm is the running maximum crossing 1/alpha, so wealth that falls back does not withdraw it.
    #[test]
    fn test_score_eproc_alarm_persists() {
        let mut ep =
            ScoreBettingEProcess::new(-1.0, 1.0, 0.17, 0.5, 0.05).expect("valid parameters");
        // Bounded, so a broken e-process fails here instead of looping: a constant 0.5 alarms within 20 scores.
        for _ in 0..100 {
            if ep.rejected_null() {
                break;
            }
            ep.observe(0.5).expect("in range");
        }
        assert!(ep.rejected_null(), "no alarm in 100 scores of 0.5");
        let peak = ep.max_log_wealth();
        assert!(peak >= det::ln(20.0));
        for _ in 0..20 {
            ep.observe(-1.0).expect("in range");
        }
        assert!(ep.log_wealth() < ep.log_threshold() && ep.rejected_null());
        assert_eq!(ep.max_log_wealth(), peak);
    }

    /// Ville on the plan's location score tanh(z/2), whose null mean is 0 by symmetry: 4,000 null runs of 600
    /// scores. The planted negative is the plan's dispersion score left uncentred, tanh((z^2 - 1)/2), whose null
    /// mean is about -0.075: a negative stake earns on it, and the bound fails (plan §10.4).
    #[test]
    fn test_score_eproc_ville_bound_and_uncentred_negative() {
        let (alpha, runs, horizon) = (0.025, 4_000, 600);
        let mut centred = 0;
        let mut uncentred = 0;
        for run in 0..runs {
            let mut rng = PhiloxStream::new(score_key(1, run));
            let mut loc = ScoreBettingEProcess::new(-1.0, 1.0, 0.17, 0.5, alpha).expect("valid");
            let mut raw =
                ScoreBettingEProcess::new(det::tanh(-0.5), 1.0, 0.17, 0.5, alpha).expect("valid");
            for _ in 0..horizon {
                let z = rng.next_normal();
                loc.observe(det::tanh(z / 2.0)).expect("in (-1, 1)");
                raw.observe(det::tanh((z * z - 1.0) / 2.0))
                    .expect("in [tanh(-1/2), 1]");
            }
            centred += usize::from(loc.rejected_null());
            uncentred += usize::from(raw.rejected_null());
        }
        let rate = |k: usize| k as f64 / f64::from(runs);
        println!(
            "score e-process, {runs} null runs of {horizon} at alpha {alpha}: location score {}, uncentred \
             dispersion score {}",
            rate(centred),
            rate(uncentred)
        );
        assert!(rate(centred) <= alpha, "Ville: {} > {alpha}", rate(centred));
        assert!(rate(uncentred) > 0.5, "uncentred: {}", rate(uncentred));

        // A score with mean 0.2 alarms.
        let mut rng = PhiloxStream::new(score_key(2, 0));
        let mut ep = ScoreBettingEProcess::new(-1.0, 1.0, 0.17, 0.5, alpha).expect("valid");
        let mut steps = 0;
        while !ep.rejected_null() && steps < 600 {
            ep.observe(det::tanh(rng.next_normal() / 2.0) * 0.5 + 0.2)
                .expect("in (-0.3, 0.7)");
            steps += 1;
        }
        assert!(ep.rejected_null(), "no alarm in {steps} shifted scores");
    }

    /// The wealth after each observation, as printed by `python3 -I scripts/oracle/risk_eproc_known_answers.py`,
    /// which follows the stated betting rule in exact fractions (bd-022).
    #[test]
    fn test_betting_eprocess_known_answers() {
        let cases: [(f64, &[f64], &[f64]); 2] = [
            (
                0.5,
                &[1.0, 1.0, 0.75, 0.25, 1.0, 0.0, 0.875],
                &[
                    1.0,
                    1.1875,
                    1.375,
                    1.1171875,
                    1.444169207317073,
                    0.9026057545731707,
                    1.0558784298780488,
                ],
            ),
            // The stake reaches its bound of 0.75 from the fourth step.
            (
                0.125,
                &[1.0; 5],
                &[
                    1.0,
                    1.4140845070422534,
                    2.2422535211267607,
                    3.7137323943661973,
                    6.150869278169014,
                ],
            ),
        ];
        for (null_mean, us, want) in cases {
            let mut ep = BettingEProcess::new(null_mean, 0.05).expect("valid");
            for (t, (&u, &w)) in us.iter().zip(want).enumerate() {
                let got = ep.observe(u).expect("in [0, 1]");
                assert!(
                    (got - w).abs() <= 1e-14 * w,
                    "null {null_mean}, step {t}: {got} against {w}"
                );
            }
            let peak = want.iter().copied().fold(1.0, f64::max);
            assert!((ep.max_wealth - peak).abs() <= 1e-14 * peak);
            assert_eq!(ep.rejected_null(), peak >= 20.0);
        }
    }

    /// The log wealth after each score, from the same script. Stakes and factors are exact there; only the
    /// logarithm is CPython's, not `det::ln`.
    #[test]
    fn test_score_eprocess_known_answers() {
        let cases: [(f64, &[f64], &[f64]); 2] = [
            (
                0.5,
                &[1.5, -0.5, 2.0, 0.25, -1.0, 1.0],
                &[
                    0.0,
                    -0.262364264467491,
                    0.1896208592755662,
                    0.28493103907989115,
                    -0.2767530019195906,
                    -0.04344233324993668,
                ],
            ),
            // A truncation of 0.2 holds every stake after the first at 0.2 / hi = 0.2.
            (
                0.2,
                &[2.0, 2.0, 2.0, -1.0],
                &[
                    0.0,
                    0.3364722366212129,
                    0.6729444732424258,
                    0.4498009219282161,
                ],
            ),
        ];
        for (truncation, scores, want) in cases {
            let mut ep = ScoreBettingEProcess::new(-1.0, 2.0, 1.0, truncation, 0.1).expect("valid");
            for (t, (&x, &w)) in scores.iter().zip(want).enumerate() {
                let got = ep.observe(x).expect("in [-1, 2]");
                assert!(
                    (got - w).abs() <= 1e-14,
                    "truncation {truncation}, step {t}: {got} against {w}"
                );
            }
            assert_eq!(ep.n_steps(), scores.len() as u64);
        }
    }
}
