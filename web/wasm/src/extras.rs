//! Smaller kernels for the figures, each the crate's own function:
//! - `fmine-plant::grinding::evaluate`: Bond specific energy `E = 10 Wi (1/√P80 − 1/√F80)` and power-limited
//!   throughput over a hardness distribution, with moments taken after the model, never at averaged hardness;
//! - `fmine-numerics::voi`: EVPI and EVSI of a discrete decision;
//! - `fmine-numerics::risk::compute_cvar`: empirical VaR and CVaR.

#![forbid(unsafe_code)]

use crate::Output;
use fmine_claim::{AdditivityClass, Claim, Confidentiality, Representation, SpatialSupport};
use fmine_core::anchor::SourceAnchor;
use fmine_core::hash::sha256;
use fmine_evidence::{EpistemicStatus, EvidenceClass, KnowledgeState};
use fmine_numerics::risk::compute_cvar;
use fmine_numerics::voi::VoiProblem;
use fmine_plant::grinding::{
    BondModel, Fraction, GrindingReceipt, GrindingScenario, GrindingScope, INPUT_METRIC, Kilowatts,
    KwhPerDryTonne, MODEL_METRIC, Micrometres, evaluate,
};
use fmine_time::ValidTime;
use fmine_units::ExactQty;

/// Scenario probabilities are exact fractions over this denominator unless all are equal (then exactly `1/n`).
const WEIGHT_DEN: i128 = 1_000_000_000;

fn scope() -> GrindingScope {
    GrindingScope {
        boundary: "frankenmining.com illustrative circuit".into(),
        circuit: "ball mill".into(),
        blend: "page-supplied hardness distribution".into(),
        interval: ValidTime {
            start_ns: 0,
            end_ns: 3_600_000_000_000,
        },
    }
}

/// The page's inputs as a source claim: stated, not measured (`EvidenceClass::Assumed`), anchored to a digest of
/// the inputs themselves.
fn source<T>(metric: &str, value: T, tag: String, inputs: &[f64]) -> Claim<T> {
    let bytes: Vec<u8> = inputs.iter().flat_map(|v| v.to_le_bytes()).collect();
    Claim::new(
        metric,
        value,
        &tag,
        Representation::Deterministic,
        SpatialSupport::PeriodAggregate,
        AdditivityClass::NonAdditive,
        EvidenceClass::Assumed,
        EpistemicStatus::KnownKnown,
        KnowledgeState::Known,
        Confidentiality::public(),
        vec![],
        vec![],
        vec![SourceAnchor::new(
            "frankenmining.com/figure-inputs",
            sha256(&bytes).0,
            "illustrative inputs supplied by the page",
        )],
    )
}

fn model(inputs: &[f64]) -> Claim<BondModel> {
    let m = BondModel {
        scope: scope(),
        id: "frankenmining-com-bond".into(),
        version: "1".into(),
        work_index_test_basis:
            "illustrative metric Bond ball-mill work index; declared prepared feed".into(),
        validity_basis: "illustrative validity box for the website figure".into(),
        complete_joint_population: true,
        work_index_domain: [KwhPerDryTonne::new(1.0), KwhPerDryTonne::new(60.0)],
        feed_size_domain: [Micrometres::new(50.0), Micrometres::new(50_000.0)],
        product_size_domain: [Micrometres::new(5.0), Micrometres::new(5_000.0)],
        power_domain: [Kilowatts::new(0.0), Kilowatts::new(100_000.0)],
        utilization_domain: [Fraction::new(0.0), Fraction::new(1.0)],
    };
    let tag = m.canonical_tag();
    source(MODEL_METRIC, m, tag, inputs)
}

fn scenario(
    id: String,
    wi: f64,
    weight: ExactQty,
    sizes: [f64; 4],
    inputs: &[f64],
) -> Claim<GrindingScenario> {
    let [f80, p80, power, utilization] = sizes;
    let s = GrindingScenario {
        id,
        scope: scope(),
        weight,
        work_index: KwhPerDryTonne::new(wi),
        feed_size: Micrometres::new(f80),
        product_size: Micrometres::new(p80),
        installed_power: Kilowatts::new(power),
        utilization: Fraction::new(utilization),
    };
    let tag = s.canonical_tag();
    source(INPUT_METRIC, s, tag, inputs)
}

/// Exact probabilities summing to exactly one: `1/n` each when the weights are all equal (or all zero), otherwise
/// each rounded to `1/10⁹` (at least one unit), the remainder going to the heaviest.
fn exact_weights(weights: &[f64]) -> Result<Vec<ExactQty>, String> {
    let n = weights.len();
    let units = |e: fmine_units::UnitsError| e.to_string();
    if weights.iter().any(|w| !(w.is_finite() && *w >= 0.0)) {
        return Err("weights must be finite and non-negative".to_string());
    }
    let first = weights.first().copied().unwrap_or(0.0);
    if weights.iter().all(|&w| w == first) {
        let one = ExactQty::new(1, i128::try_from(n).map_err(|e| e.to_string())?).map_err(units)?;
        return Ok(vec![one; n]);
    }
    let total: f64 = weights.iter().sum();
    let mut nums: Vec<i128> = weights
        .iter()
        .map(|w| ((w / total * WEIGHT_DEN as f64).round() as i128).max(1))
        .collect();
    let heaviest = (0..n)
        .max_by(|&a, &b| weights[a].total_cmp(&weights[b]))
        .ok_or("no weights")?;
    let sum: i128 = nums.iter().sum();
    nums[heaviest] += WEIGHT_DEN - sum;
    if nums[heaviest] < 1 {
        return Err("too many scenarios for the exact weights".to_string());
    }
    nums.into_iter()
        .map(|k| ExactQty::new(k, WEIGHT_DEN).map_err(units))
        .collect()
}

fn summary_numbers(r: &GrindingReceipt) -> [f64; 13] {
    let s = r.summary();
    let (t, e) = (&s.throughput, &s.specific_energy);
    [
        t.mean.value(),
        t.minimum.value(),
        t.p10.value(),
        t.p50.value(),
        t.p90.value(),
        t.maximum.value(),
        e.mean.value(),
        e.minimum.value(),
        e.p10.value(),
        e.p50.value(),
        e.p90.value(),
        e.maximum.value(),
        s.shutdown_probability.to_f64(),
    ]
}

/// Bond throughput over a hardness distribution. Input: `n` work indices (kWh/t), then `n` weights (or none).
///
/// Output `f64s` = `[t mean, t min, t p10, t p50, t p90, t max, E mean, E min, E p10, E p50, E p90, E max,
/// shutdown probability, t at the mean Wi, E at the mean Wi, mean Wi]`, then each scenario's throughput (t/h)
/// and specific energy (kWh/t) in input order. "At the mean Wi" is the same `evaluate` on one scenario at the
/// weighted-mean hardness: the averaging the crate refuses to do for you.
pub fn grind(
    input: &[f64],
    n: usize,
    f80_um: f64,
    p80_um: f64,
    power_kw: f64,
    utilization: f64,
) -> Result<Output, String> {
    if n == 0 || (input.len() != n && input.len() != 2 * n) {
        return Err(format!(
            "{} numbers for {n} work indices and weights",
            input.len()
        ));
    }
    let wi = &input[..n];
    let weights = if input.len() == 2 * n {
        &input[n..]
    } else {
        &input[n..n]
    };
    let exact = if weights.is_empty() {
        exact_weights(&vec![1.0; n])?
    } else {
        exact_weights(weights)?
    };
    let sizes = [f80_um, p80_um, power_kw, utilization];
    let mut digest_inputs = input.to_vec();
    digest_inputs.extend(sizes);
    let model = model(&digest_inputs);
    let scenarios: Vec<_> = wi
        .iter()
        .zip(&exact)
        .enumerate()
        .map(|(i, (&w, &p))| scenario(format!("wi-{i:06}"), w, p, sizes, &digest_inputs))
        .collect();
    let receipt = evaluate(&model, &scenarios).map_err(|r| r.to_string())?;
    let mean_wi: f64 = wi.iter().zip(&exact).map(|(w, p)| w * p.to_f64()).sum();
    let at_mean = evaluate(
        &model,
        &[scenario(
            "mean-wi".into(),
            mean_wi,
            ExactQty::from_integer(1),
            sizes,
            &digest_inputs,
        )],
    )
    .map_err(|r| r.to_string())?;
    let mut f64s = summary_numbers(&receipt).to_vec();
    let point = at_mean.points().first().ok_or("no point at the mean")?;
    f64s.extend([
        point.throughput.value(),
        point.specific_energy.value(),
        mean_wi,
    ]);
    // Points come back sorted by id, which the zero-padded ids make input order.
    f64s.extend(receipt.points().iter().map(|p| p.throughput.value()));
    f64s.extend(receipt.points().iter().map(|p| p.specific_energy.value()));
    Ok(Output {
        f64s,
        i32s: Vec::new(),
        u8s: Vec::new(),
    })
}

/// EVPI, and EVSI when there are signals. Input: `states` priors, `states × actions` payoffs, `states × signals`
/// likelihoods `P(signal | state)`.
///
/// Output `f64s` = `[prior best action, prior expected value, perfect-information expected value, EVPI, EVSI,
/// net VOI]`; EVSI and net VOI are NaN with no signals.
pub fn voi(
    input: &[f64],
    states: usize,
    actions: usize,
    signals: usize,
    cost: f64,
) -> Result<Output, String> {
    if input.len() != states * (1 + actions + signals) {
        return Err(format!(
            "{} numbers for {states} states × (1 prior + {actions} payoffs + {signals} likelihoods)",
            input.len()
        ));
    }
    let (probs, rest) = input.split_at(states);
    let (pay, like) = rest.split_at(states * actions);
    let rows = |flat: &[f64], width: usize| -> Vec<Vec<f64>> {
        if width == 0 {
            vec![Vec::new(); states]
        } else {
            flat.chunks_exact(width).map(<[f64]>::to_vec).collect()
        }
    };
    let problem = VoiProblem {
        state_probs: probs.to_vec(),
        payoffs: rows(pay, actions),
    };
    let f64s = if signals == 0 {
        let (action, value, evpi) = problem.compute_evpi().map_err(|e| e.to_string())?;
        let perfect = value + evpi;
        vec![action as f64, value, perfect, evpi, f64::NAN, f64::NAN]
    } else {
        let s = problem
            .compute_evsi(&rows(like, signals), cost)
            .map_err(|e| e.to_string())?;
        vec![
            s.prior_best_action as f64,
            s.prior_expected_value,
            s.perfect_info_expected_value,
            s.evpi,
            s.evsi,
            s.net_voi,
        ]
    };
    Ok(Output {
        f64s,
        i32s: Vec::new(),
        u8s: Vec::new(),
    })
}

/// Empirical tail risk of `losses` (larger is worse). Output `f64s` = `[alpha, mean loss, VaR, CVaR]`.
pub fn cvar(losses: &[f64], alpha: f64) -> Result<Output, String> {
    let s = compute_cvar(losses, alpha).map_err(|e| e.to_string())?;
    Ok(Output {
        f64s: vec![s.alpha, s.mean_loss, s.var_alpha, s.cvar_alpha],
        i32s: Vec::new(),
        u8s: Vec::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The crate README's example: Wi 5 and 20 at equal weight average 200 t/h; Wi 12.5 gives 128 t/h.
    #[test]
    fn grinding_averaging_trap() {
        let out = grind(&[5.0, 20.0], 2, 400.0, 100.0, 1000.0, 0.8).unwrap();
        assert!((out.f64s[0] - 200.0).abs() < 1e-9, "{:?}", out.f64s);
        assert!((out.f64s[13] - 128.0).abs() < 1e-9, "{:?}", out.f64s);
        assert!((out.f64s[16] - 320.0).abs() < 1e-9 && (out.f64s[17] - 80.0).abs() < 1e-9);
    }

    #[test]
    fn evpi_of_a_coin() {
        // Two equally likely states; action 0 pays 1 in state 0, action 1 pays 1 in state 1.
        let out = voi(&[0.5, 0.5, 1.0, 0.0, 0.0, 1.0], 2, 2, 0, 0.0).unwrap();
        assert!((out.f64s[3] - 0.5).abs() < 1e-12, "{:?}", out.f64s);
    }
}
