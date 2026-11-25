//! `fmine-calib`'s `SpatialFrontPitMonitor` for the figures: conditional Gaussian prediction from drillholes, and
//! the sequential PIT e-process monitor.
//!
//! **Units.** The model is Gaussian in whatever the caller feeds it. Grades are skewed, so the figures work in
//! log-grade: values, priors, nugget and sill are all in log units, and the mean and sd come back in them.
//!
//! **Getting the prediction out.** The monitor's predictor, `predictive(x, y, z, prior) → (mean, sd)`, is private
//! (`fmine-calib` is not edited here). Its public `observe_sample` runs it, standardises the observation
//! `z* = (value − mean) / sd`, bets, appends the sample and returns the PIT `Φ(z*)`. The bridge reads the
//! predictor through that call alone:
//! - A probe calls `observe_sample` and then undoes it: the appended history entry is removed and both
//!   e-processes are put back as they were (`history`, `location_eproc` and `dispersion_eproc` are public
//!   fields). A probe therefore changes nothing.
//! - The PIT is inverted against the crate's own `norm_cdf` (Acklam's quantile, then Newton on `norm_cdf`
//!   itself), so the recovered `z*` is the crate's, whatever its `Φ`'s own error.
//! - **sd.** The neighbour choice and the conditional sd depend only on positions. A twin monitor holds the same
//!   positions with prior 0 and value 0, so its conditional mean is exactly 0. A probe there at prior 0 with
//!   value `δ` returns `Φ(δ / sd)`, so `sd = δ / z*`. `δ = −√nugget` puts `z*` in `[−1, 0)`, since
//!   `sd ≥ √nugget`; a second probe at `δ = −sd` follows when `|z*|` is small.
//! - **mean.** A probe on the real monitor at the query's prior with value `g` gives `z* = (g − mean) / sd`, so
//!   `mean = g − sd · z*`. `g` starts at the prior. `Φ` loses resolution in its tails (it saturates near
//!   `|z*| = 8.3`), so when `|z*| > 3` the probe is repeated at the estimated mean, and a saturated PIT moves `g`
//!   by a bracketing search.
//!
//! Typical cost: two probes per query. The `tests` module checks the recovered values against a direct
//! re-implementation of the covariance algebra.

#![forbid(unsafe_code)]

use crate::Output;
use fmine_calib::{SpatialFrontPitMonitor, SpatialSample};
use fmine_core::det;
use fmine_numerics::{norm_cdf, norm_inv};
use fmine_units::{Length, Qty};
use std::sync::Mutex;

/// The registered exponential-covariance model and the monitor's significance level.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Model {
    pub nugget: f64,
    pub sill: f64,
    pub range_m: f64,
    pub neighbors: usize,
    pub alpha: f64,
}

impl Model {
    fn monitor(&self) -> Result<SpatialFrontPitMonitor, String> {
        SpatialFrontPitMonitor::new(
            self.nugget,
            self.sill,
            Qty::<Length>::new(self.range_m),
            self.neighbors,
            self.alpha,
        )
        .map_err(|e| format!("spatial monitor refused its model: {e}"))
    }
}

/// A monitor and its zero-residual twin (same positions, prior 0, value 0).
#[derive(Debug, Clone)]
pub struct Pair {
    pub real: SpatialFrontPitMonitor,
    pub zero: SpatialFrontPitMonitor,
    model: Model,
}

impl Pair {
    pub fn new(model: Model) -> Result<Self, String> {
        let real = model.monitor()?;
        let zero = real.clone();
        Ok(Self { real, zero, model })
    }

    /// Adds conditioning samples to the history without betting on them.
    pub fn condition(&mut self, samples: impl IntoIterator<Item = SpatialSample>) {
        for s in samples {
            self.zero.history.push(SpatialSample {
                prior_mean: 0.0,
                observed_value: 0.0,
                ..s
            });
            self.real.history.push(s);
        }
    }

    /// The crate's predictive mean and sd at `(x, y, z)` for prior mean `prior`, read through probes.
    pub fn predict(&mut self, x: f64, y: f64, z: f64, prior: f64) -> Result<Prediction, String> {
        let mut probes = 0;
        let sd = recover_sd(&mut self.zero, x, y, z, &mut probes)?;
        let mean = recover_mean(&mut self.real, x, y, z, prior, sd, &mut probes)?;
        Ok(Prediction { mean, sd, probes })
    }
}

/// A recovered predictive distribution and the probes it took.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Prediction {
    pub mean: f64,
    pub sd: f64,
    pub probes: u32,
}

/// `observe_sample` on `sample`, then undone: the history entry it appended is removed and both e-processes are
/// restored. Returns the crate's PIT.
pub fn probe(mon: &mut SpatialFrontPitMonitor, sample: SpatialSample) -> Result<f64, String> {
    let location = mon.location_eproc.clone();
    let dispersion = mon.dispersion_eproc.clone();
    let len = mon.history.len();
    let result = mon.observe_sample(sample);
    mon.history.truncate(len);
    mon.location_eproc = location;
    mon.dispersion_eproc = dispersion;
    result.map(|(pit, _)| pit).map_err(|r| r.to_string())
}

const FRAC_1_SQRT_2PI: f64 = 0.398_942_280_401_432_7;

/// The `z` with the crate's `norm_cdf(z) = p`, or `None` for a saturated `p` (0, 1 or outside).
pub fn pit_to_z(p: f64) -> Option<f64> {
    if !(p > 0.0 && p < 1.0) {
        return None;
    }
    let mut z = norm_inv(p);
    if !z.is_finite() {
        return None;
    }
    let mut best = (f64::INFINITY, z);
    for _ in 0..8 {
        let f = norm_cdf(z) - p;
        if f.abs() < best.0 {
            best = (f.abs(), z);
        }
        if f == 0.0 {
            break;
        }
        let d = det::exp(-0.5 * z * z) * FRAC_1_SQRT_2PI;
        if d <= 0.0 || !d.is_finite() {
            break;
        }
        let step = f / d;
        z -= step;
        if !z.is_finite() || step.abs() <= 1e-17 * (1.0 + z.abs()) {
            break;
        }
    }
    if z.is_finite() {
        let f = (norm_cdf(z) - p).abs();
        if f < best.0 {
            best = (f, z);
        }
    }
    Some(best.1)
}

fn sample(x: f64, y: f64, z: f64, prior: f64, value: f64) -> SpatialSample {
    SpatialSample {
        x,
        y,
        z,
        prior_mean: prior,
        observed_value: value,
    }
}

/// sd from the zero-residual twin: a probe at value `δ < 0` returns `Φ(δ / sd)`.
fn recover_sd(
    zero: &mut SpatialFrontPitMonitor,
    x: f64,
    y: f64,
    z: f64,
    probes: &mut u32,
) -> Result<f64, String> {
    let mut delta = -zero.nugget_var.sqrt();
    let mut sd = f64::NAN;
    for _ in 0..4 {
        *probes += 1;
        let p = probe(zero, sample(x, y, z, 0.0, delta))?;
        let zs = pit_to_z(p).ok_or_else(|| format!("sd probe saturated (PIT {p})"))?;
        if zs.is_nan() || zs >= 0.0 {
            return Err(format!("sd probe gave z {zs} for a negative residual"));
        }
        sd = delta / zs;
        if zs.abs() >= 0.25 {
            return Ok(sd);
        }
        delta = -sd;
    }
    if sd.is_finite() && sd > 0.0 {
        Ok(sd)
    } else {
        Err("sd probe did not settle".to_string())
    }
}

/// The mean from the real monitor: a probe at value `g` returns `Φ((g − mean) / sd)`.
fn recover_mean(
    real: &mut SpatialFrontPitMonitor,
    x: f64,
    y: f64,
    z: f64,
    prior: f64,
    sd: f64,
    probes: &mut u32,
) -> Result<f64, String> {
    let (mut lo, mut hi) = (f64::NEG_INFINITY, f64::INFINITY);
    let mut g = prior;
    let mut step = 16.0 * sd;
    for _ in 0..400 {
        *probes += 1;
        let p = probe(real, sample(x, y, z, prior, g))?;
        match pit_to_z(p) {
            Some(zs) if zs.abs() <= 3.0 => return Ok(g - sd * zs),
            Some(zs) => {
                if zs > 0.0 {
                    hi = hi.min(g);
                } else {
                    lo = lo.max(g);
                }
                g -= sd * zs;
                continue;
            }
            None if p >= 1.0 => hi = hi.min(g),
            None => lo = lo.max(g),
        }
        g = if lo.is_finite() && hi.is_finite() {
            0.5 * (lo + hi)
        } else if hi.is_finite() {
            hi - step
        } else {
            lo + step
        };
        step *= 2.0;
        if !g.is_finite() {
            break;
        }
    }
    Err("mean probe did not settle".to_string())
}

/// Predictions for `count` queries `(x, y, z, prior)` against a pair's history.
///
/// Output: `f64s` = `[mean × count, sd × count, refused, total probes]`, a refused query being NaN in both. The first
/// refusal's message is not lost: when every query is refused the call itself refuses with it.
pub fn predict_many(pair: &mut Pair, queries: &[f64], count: usize) -> Result<Output, String> {
    if queries.len() != 4 * count {
        return Err(format!(
            "{} numbers for {count} queries of 4",
            queries.len()
        ));
    }
    let mut mean = Vec::with_capacity(count);
    let mut sd = Vec::with_capacity(count);
    let (mut refused, mut probes, mut first_error) = (0_u32, 0_u64, None);
    for &[x, y, z, prior] in queries.as_chunks::<4>().0 {
        match pair.predict(x, y, z, prior) {
            Ok(p) => {
                mean.push(p.mean);
                sd.push(p.sd);
                probes += u64::from(p.probes);
            }
            Err(e) => {
                mean.push(f64::NAN);
                sd.push(f64::NAN);
                refused += 1;
                first_error.get_or_insert(e);
            }
        }
    }
    if count > 0 && refused as usize == count {
        return Err(first_error.unwrap_or_default());
    }
    let mut f64s = mean;
    f64s.extend(sd);
    f64s.extend([f64::from(refused), probes as f64]);
    Ok(Output {
        f64s,
        i32s: Vec::new(),
        u8s: Vec::new(),
    })
}

fn samples_from(input: &[f64], count: usize) -> Result<Vec<SpatialSample>, String> {
    if input.len() < 5 * count {
        return Err(format!("{} numbers for {count} samples of 5", input.len()));
    }
    Ok(input[..5 * count]
        .as_chunks::<5>()
        .0
        .iter()
        .map(|&[x, y, z, prior, value]| sample(x, y, z, prior, value))
        .collect())
}

/// Conditional prediction from drill samples. Input: `samples` × (x, y, z, prior, value), then `queries` ×
/// (x, y, z, prior). Output: as [`predict_many`].
pub fn estimate(
    model: &Model,
    input: &[f64],
    samples: usize,
    queries: usize,
) -> Result<Output, String> {
    if input.len() != 5 * samples + 4 * queries {
        return Err(format!(
            "{} numbers for {samples} samples of 5 and {queries} queries of 4",
            input.len()
        ));
    }
    let mut pair = Pair::new(*model)?;
    pair.condition(samples_from(input, samples)?);
    predict_many(&mut pair, &input[5 * samples..], queries)
}

// --- Monitor handles ---

static MONITORS: Mutex<Vec<Option<Pair>>> = Mutex::new(Vec::new());

fn with_monitor<T>(h: usize, f: impl FnOnce(&mut Pair) -> Result<T, String>) -> Result<T, String> {
    let mut all = crate::lock(&MONITORS);
    let pair = all
        .get_mut(h)
        .and_then(Option::as_mut)
        .ok_or_else(|| format!("no monitor {h}"))?;
    f(pair)
}

pub fn monitor_new(model: Model) -> Result<i32, String> {
    let pair = Pair::new(model)?;
    let mut all = crate::lock(&MONITORS);
    let slot = match all.iter().position(Option::is_none) {
        Some(i) => {
            all[i] = Some(pair);
            i
        }
        None => {
            all.push(Some(pair));
            all.len() - 1
        }
    };
    i32::try_from(slot).map_err(|_| "too many monitors".to_string())
}

/// Bets on one observation with the crate's `observe_sample`; the mean and sd it was judged against are read
/// first, by probes.
///
/// Output `f64s` = `[pit, alarm, mean, sd, z, log wealth (location), log wealth (dispersion), max log wealth
/// (location), max log wealth (dispersion), log threshold, steps, stake (location), stake (dispersion)]`. The
/// alarm fires when either wealth has reached `2 / alpha` (log threshold `−ln(alpha / 2)`).
pub fn monitor_observe(
    h: usize,
    x: f64,
    y: f64,
    z: f64,
    prior: f64,
    value: f64,
) -> Result<Output, String> {
    with_monitor(h, |pair| {
        let p = pair.predict(x, y, z, prior)?;
        let (pit, alarm) = pair
            .real
            .observe_sample(sample(x, y, z, prior, value))
            .map_err(|r| r.to_string())?;
        pair.zero.history.push(sample(x, y, z, 0.0, 0.0));
        let zs = pit_to_z(pit).unwrap_or((value - p.mean) / p.sd);
        let (loc, disp) = (&pair.real.location_eproc, &pair.real.dispersion_eproc);
        Ok(Output {
            f64s: vec![
                pit,
                f64::from(u8::from(alarm)),
                p.mean,
                p.sd,
                zs,
                loc.log_wealth(),
                disp.log_wealth(),
                loc.max_log_wealth(),
                disp.max_log_wealth(),
                loc.log_threshold(),
                loc.n_steps() as f64,
                loc.stake(),
                disp.stake(),
            ],
            i32s: Vec::new(),
            u8s: Vec::new(),
        })
    })
}

pub fn monitor_condition(h: usize, input: &[f64], count: usize) -> Result<Output, String> {
    if input.len() != 5 * count {
        return Err(format!("{} numbers for {count} samples of 5", input.len()));
    }
    let samples = samples_from(input, count)?;
    if let Some(s) = samples.iter().find(|s| {
        ![s.x, s.y, s.z, s.prior_mean, s.observed_value]
            .iter()
            .all(|v| v.is_finite())
    }) {
        return Err(format!("conditioning sample {s:?} is not finite"));
    }
    with_monitor(h, |pair| {
        pair.condition(samples);
        Ok(Output::default())
    })
}

pub fn monitor_estimate(h: usize, input: &[f64], count: usize) -> Result<Output, String> {
    with_monitor(h, |pair| predict_many(pair, input, count))
}

pub fn monitor_reset(h: usize) -> Result<Output, String> {
    with_monitor(h, |pair| {
        *pair = Pair::new(pair.model)?;
        Ok(Output::default())
    })
}

pub fn monitor_free(h: usize) -> Result<Output, String> {
    let mut all = crate::lock(&MONITORS);
    match all.get_mut(h) {
        Some(slot @ Some(_)) => {
            *slot = None;
            Ok(Output::default())
        }
        _ => Err(format!("no monitor {h}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A direct re-implementation of the registered model's conditional Gaussian on the nearest `k` samples:
    /// exponential covariance with the nugget at zero distance, Gaussian elimination with partial pivoting.
    fn direct(m: &Model, hist: &[SpatialSample], q: [f64; 4]) -> (f64, f64) {
        let total = m.nugget + m.sill;
        let cov = |a: [f64; 3], b: [f64; 3]| {
            let h = ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt();
            if h < 1e-9 {
                total
            } else {
                m.sill * (-h / m.range_m).exp()
            }
        };
        if hist.is_empty() {
            return (q[3], total.sqrt());
        }
        let mut idx: Vec<(f64, usize)> = hist
            .iter()
            .enumerate()
            .map(|(i, s)| {
                let d = (s.x - q[0]).powi(2) + (s.y - q[1]).powi(2) + (s.z - q[2]).powi(2);
                (d, i)
            })
            .collect();
        idx.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
        let k = idx.len().min(m.neighbors);
        let nb: Vec<&SpatialSample> = idx[..k].iter().map(|&(_, i)| &hist[i]).collect();
        let pos = |s: &SpatialSample| [s.x, s.y, s.z];
        let qp = [q[0], q[1], q[2]];
        // Augmented system [K | k].
        let mut a: Vec<Vec<f64>> = (0..k)
            .map(|i| {
                let mut row: Vec<f64> = (0..k)
                    .map(|j| {
                        if i == j {
                            total
                        } else {
                            cov(pos(nb[i]), pos(nb[j]))
                        }
                    })
                    .collect();
                row.push(cov(qp, pos(nb[i])));
                row
            })
            .collect();
        for c in 0..k {
            let p = (c..k)
                .max_by(|&i, &j| a[i][c].abs().total_cmp(&a[j][c].abs()))
                .unwrap();
            a.swap(c, p);
            for r in 0..k {
                if r != c {
                    let f = a[r][c] / a[c][c];
                    for cc in c..=k {
                        a[r][cc] -= f * a[c][cc];
                    }
                }
            }
        }
        let w: Vec<f64> = (0..k).map(|i| a[i][k] / a[i][i]).collect();
        let mut mu = q[3];
        let mut red = 0.0;
        for i in 0..k {
            mu += w[i] * (nb[i].observed_value - nb[i].prior_mean);
            red += w[i] * cov(qp, pos(nb[i]));
        }
        (mu, (total - red).max(m.nugget).sqrt())
    }

    /// A small deterministic generator (SplitMix64) for test layouts.
    struct Gen(u64);
    impl Gen {
        fn next(&mut self) -> f64 {
            self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
            let mut z = self.0;
            z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
            ((z ^ (z >> 31)) >> 11) as f64 / (1u64 << 53) as f64
        }
    }

    fn layout(seed: u64, n: usize, spread: f64) -> Vec<SpatialSample> {
        let mut g = Gen(seed);
        (0..n)
            .map(|_| {
                let prior = 0.3 * (g.next() - 0.5);
                sample(
                    spread * g.next(),
                    spread * g.next(),
                    0.3 * spread * g.next(),
                    prior,
                    prior + 2.0 * (g.next() - 0.5) + (g.next() - 0.5),
                )
            })
            .collect()
    }

    fn check(model: Model, seed: u64, n: usize, queries: usize, tol: f64) -> f64 {
        let hist = layout(seed, n, 400.0);
        let mut pair = Pair::new(model).unwrap();
        pair.condition(hist.iter().copied());
        let mut g = Gen(seed ^ 0xABCD);
        let mut worst = 0.0_f64;
        for qi in 0..queries {
            let q = if qi % 7 == 0 {
                // Exactly at a sample.
                let s = hist[qi % hist.len()];
                [s.x, s.y, s.z, s.prior_mean]
            } else {
                [
                    600.0 * g.next() - 100.0,
                    600.0 * g.next() - 100.0,
                    150.0 * g.next(),
                    g.next() - 0.5,
                ]
            };
            let got = pair.predict(q[0], q[1], q[2], q[3]).unwrap();
            let (mu, sd) = direct(&model, &hist, q);
            let em = (got.mean - mu).abs() / sd;
            let es = (got.sd - sd).abs() / sd;
            worst = worst.max(em).max(es);
            assert!(
                em < tol && es < tol,
                "query {q:?}: recovered ({}, {}) vs direct ({mu}, {sd})",
                got.mean,
                got.sd
            );
        }
        // Probing changed nothing.
        assert_eq!(pair.real.history.len(), n);
        assert_eq!(pair.real.location_eproc.n_steps(), 0);
        worst
    }

    #[test]
    fn recovered_prediction_matches_direct_algebra() {
        let base = Model {
            nugget: 0.05,
            sill: 0.6,
            range_m: 80.0,
            neighbors: 12,
            alpha: 0.05,
        };
        let w1 = check(base, 1, 300, 400, 1e-9);
        let w2 = check(
            Model {
                nugget: 1e-6,
                sill: 1.0,
                neighbors: 16,
                ..base
            },
            2,
            200,
            200,
            1e-9,
        );
        let w3 = check(
            Model {
                nugget: 0.3,
                sill: 0.01,
                range_m: 5.0,
                neighbors: 1,
                ..base
            },
            3,
            50,
            100,
            1e-9,
        );
        eprintln!("worst relative error (in sd units): {w1:e} {w2:e} {w3:e}");
    }

    #[test]
    fn empty_history_gives_prior() {
        let mut pair = Pair::new(Model {
            nugget: 0.1,
            sill: 0.9,
            range_m: 50.0,
            neighbors: 8,
            alpha: 0.05,
        })
        .unwrap();
        let p = pair.predict(1.0, 2.0, 3.0, 0.7).unwrap();
        assert!(
            (p.mean - 0.7).abs() < 1e-12 && (p.sd - 1.0).abs() < 1e-12,
            "{p:?}"
        );
    }

    #[test]
    fn pit_inversion_returns_crate_z() {
        for i in -800..=800 {
            let z = f64::from(i) / 100.0;
            let p = norm_cdf(z);
            let back = pit_to_z(p).unwrap();
            let tol = 1e-15 / (det::exp(-0.5 * z * z) * FRAC_1_SQRT_2PI) + 1e-15;
            assert!((back - z).abs() <= 8.0 * tol, "z {z}: back {back}");
        }
    }
}
