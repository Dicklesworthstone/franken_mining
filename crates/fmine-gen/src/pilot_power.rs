//! Determinism: BitwisePortable
//! Pilot Protocol Card power envelope, linear-Gaussian GB-25 identifiability, and robust vs.
//! model-dependent candidate file labeling (`BD-356.S2`, plan §20.6, §37.3, `GB-25`, `GATE-GEN-01`).
//!
//! Enforces three design invariants:
//! 1. No absolute power value can ever be formatted without its `[min, max]` envelope across twins.
//! 2. Posterior bias pairs with `|corr| > 0.8` refuse individual attribution (`FMINE-REF-104`) while
//!    reporting the identifiable linear contrast (e.g. `b_gc - b_mill` in GB-25).
//! 3. Any request naming personal/roster data refuses with `FMINE-REF-301`.

use std::collections::BTreeSet;

use fmine_core::det::{norm_cdf, norm_ppf, sqrt};
use fmine_core::rand::{PhiloxStream, RngKey};
use fmine_evidence::{Refusal, RefusalCode};

use crate::plant_twin::{CandidateFileCostRow, PlantTwin, TwinFamily};

/// Recovery-led GATE-DATA-03 attribution causes (§37.3, `BD-356.S2`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AttributionCause {
    HeadSamplerBias,
    TailSamplerBias,
    TransitionalOreShare,
    ThroughputDowntime,
    AssayBias,
}

impl AttributionCause {
    /// All five recovery-led causes from §37.3.
    pub const ALL: [Self; 5] = [
        Self::HeadSamplerBias,
        Self::TailSamplerBias,
        Self::TransitionalOreShare,
        Self::ThroughputDowntime,
        Self::AssayBias,
    ];

    /// Stable identifier string.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::HeadSamplerBias => "head_sampler_bias",
            Self::TailSamplerBias => "tail_sampler_bias",
            Self::TransitionalOreShare => "transitional_ore_share",
            Self::ThroughputDowntime => "throughput_and_downtime",
            Self::AssayBias => "assay_bias",
        }
    }
}

/// Linear-Gaussian observation row `h1 * b1 + h2 * b2 + eps = y`, `eps ~ N(0, sigma^2)`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LinearBiasObs {
    pub h1: f64,
    pub h2: f64,
    pub y: f64,
    pub sigma: f64,
}

/// Posterior summary for a two-bias linear-Gaussian attribution problem (`GB-25`, plan §20.6).
#[derive(Debug, Clone, PartialEq)]
pub struct TwoBiasPosterior {
    pub mean_b1: f64,
    pub mean_b2: f64,
    pub std_b1: f64,
    pub std_b2: f64,
    pub correlation: f64,
    pub diff_mean: f64,
    pub diff_std: f64,
    /// `Ok(())` when `|correlation| <= 0.8`; `Err(FMINE-REF-104)` when confounded, alongside the
    /// determined linear contrast `(diff_mean, diff_std)`.
    pub attribution_status: Result<(), Refusal>,
}

/// Exact 2×2 Bayesian linear-Gaussian update for `b = (b1, b2) ~ N(0, diag(prior_sd^2))`
/// observed through `observations` (`GB-25`, plan §20.6).
pub fn solve_two_bias_posterior(
    prior_sd_b1: f64,
    prior_sd_b2: f64,
    observations: &[LinearBiasObs],
) -> Result<TwoBiasPosterior, Refusal> {
    if !(prior_sd_b1.is_finite()
        && prior_sd_b1 > 0.0
        && prior_sd_b2.is_finite()
        && prior_sd_b2 > 0.0)
    {
        return Err(Refusal::new(
            RefusalCode::OutsidePhysicalDomain109,
            "prior standard deviations must be finite and > 0",
        ));
    }
    // Posterior precision matrix Λ = Σ_0^-1 + Σ_i H_i^T R_i^-1 H_i
    let mut l11 = 1.0 / (prior_sd_b1 * prior_sd_b1);
    let mut l22 = 1.0 / (prior_sd_b2 * prior_sd_b2);
    let mut l12 = 0.0_f64;
    let mut rhs1 = 0.0_f64;
    let mut rhs2 = 0.0_f64;

    for obs in observations {
        if !(obs.sigma.is_finite()
            && obs.sigma > 0.0
            && obs.h1.is_finite()
            && obs.h2.is_finite()
            && obs.y.is_finite())
        {
            return Err(Refusal::new(
                RefusalCode::OutsidePhysicalDomain109,
                "invalid linear bias observation",
            ));
        }
        let inv_v = 1.0 / (obs.sigma * obs.sigma);
        l11 += obs.h1 * obs.h1 * inv_v;
        l22 += obs.h2 * obs.h2 * inv_v;
        l12 += obs.h1 * obs.h2 * inv_v;
        rhs1 += obs.h1 * obs.y * inv_v;
        rhs2 += obs.h2 * obs.y * inv_v;
    }

    let det = l11 * l22 - l12 * l12;
    if !(det.is_finite() && det > 1e-18) {
        return Err(Refusal::new(
            RefusalCode::NonIdentifiableAttribution104,
            "singular posterior precision matrix",
        ));
    }
    let s11 = l22 / det;
    let s22 = l11 / det;
    let s12 = -l12 / det;

    let mean_b1 = s11 * rhs1 + s12 * rhs2;
    let mean_b2 = s12 * rhs1 + s22 * rhs2;
    let std_b1 = sqrt(s11.max(0.0));
    let std_b2 = sqrt(s22.max(0.0));
    let correlation = (s12 / (std_b1 * std_b2)).clamp(-1.0, 1.0);

    // Linear contrast d = b1 - b2: mean = μ1 - μ2, var = s11 + s22 - 2*s12.
    let diff_mean = mean_b1 - mean_b2;
    let diff_std = sqrt((s11 + s22 - 2.0 * s12).max(0.0));

    let attribution_status = if correlation.abs() > 0.80 {
        Err(Refusal::new(
            RefusalCode::NonIdentifiableAttribution104,
            format!(
                "non-identifiable attribution (|corr|={:.3} > 0.80); determined contrast b1 - b2 = {:.4} ± {:.4}",
                correlation.abs(),
                diff_mean,
                diff_std
            ),
        ))
    } else {
        Ok(())
    };

    Ok(TwoBiasPosterior {
        mean_b1,
        mean_b2,
        std_b1,
        std_b2,
        correlation,
        diff_mean,
        diff_std,
        attribution_status,
    })
}

/// Refuse any candidate file request that names personal/roster data (`AGENTS.md`, `BD-356.S2`).
pub fn reject_personal_data_files(file_names: &[&str]) -> Result<(), Refusal> {
    const FORBIDDEN_TOKENS: [&str; 8] = [
        "roster",
        "fatigue",
        "competency",
        "employee",
        "personal",
        "grievance",
        "consultation",
        "payroll",
    ];
    for &name in file_names {
        let lower = name.to_ascii_lowercase();
        for tok in FORBIDDEN_TOKENS {
            if lower.contains(tok) {
                return Err(Refusal::new(
                    RefusalCode::MnpiWall301,
                    format!(
                        "candidate file '{name}' rejected: personal/roster token '{tok}' never permitted in pilot data requests"
                    ),
                ));
            }
        }
    }
    Ok(())
}

/// Compute the observation-operator identifiability rank (0..=5) for a candidate file subset over
/// the five recovery-led causes (`BD-356.S2`). Adding a file monotonically never lowers rank.
#[must_use]
pub fn subset_identifiability_rank(subset: &[&str]) -> usize {
    let mut covered = BTreeSet::new();
    for &f in subset {
        match f {
            "monthly_metallurgical_balance" => {
                covered.insert(AttributionCause::ThroughputDowntime);
            }
            "head_tail_conc_samplers" => {
                covered.insert(AttributionCause::HeadSamplerBias);
                covered.insert(AttributionCause::TailSamplerBias);
            }
            "conc_shipment_settlement_assays" => {
                covered.insert(AttributionCause::AssayBias);
                covered.insert(AttributionCause::HeadSamplerBias);
            }
            "fresh_transitional_feed_tags" => {
                covered.insert(AttributionCause::TransitionalOreShare);
            }
            "monthly_reconciliation_pack" => {
                covered.insert(AttributionCause::ThroughputDowntime);
                covered.insert(AttributionCause::TailSamplerBias);
            }
            _ => {}
        }
    }
    covered.len()
}

/// Power estimate for one `(twin, subset, cause)` cell across `replicates >= 200` sealed draws.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TwinCausePower {
    pub twin_k: usize,
    pub cause: AttributionCause,
    pub replicates: usize,
    pub empirical_power: f64,
    pub analytic_power: f64,
    pub mc_se: f64,
    pub identifiable: bool,
}

/// Envelope `[min_power, max_power]` across all twins in a [`TwinFamily`].
///
/// Enforces `BD-356.S2`: no absolute power is ever rendered without its `[min, max]` envelope.
#[derive(Debug, Clone, PartialEq)]
pub struct PowerEnvelope {
    pub cause: AttributionCause,
    pub subset: Vec<String>,
    pub twins_evaluated: usize,
    pub min_power: f64,
    pub max_power: f64,
    pub max_mc_se: f64,
    /// Non-empty if any twin refuses identifiability for this `(subset, cause)` pair.
    pub identifiability_refusal: Option<Refusal>,
}

impl PowerEnvelope {
    /// Render the power strictly as an envelope `[min, max]` (never a bare single scalar).
    pub fn render_envelope_string(&self) -> Result<String, Refusal> {
        if self.twins_evaluated == 0 || self.min_power > self.max_power {
            return Err(Refusal::new(
                RefusalCode::OutsidePhysicalDomain109,
                "cannot render power without a valid non-empty [min, max] envelope across twins",
            ));
        }
        if let Some(ref r) = self.identifiability_refusal {
            return Ok(format!(
                "{}: REFUSED ({}) [envelope={:.3}..{:.3}]",
                self.cause.as_str(),
                r.code,
                self.min_power,
                self.max_power
            ));
        }
        Ok(format!(
            "{}: [{:.3}, {:.3}] (k={}, max_mc_se={:.3})",
            self.cause.as_str(),
            self.min_power,
            self.max_power,
            self.twins_evaluated,
            self.max_mc_se
        ))
    }
}

/// Render helper refusing any single-twin/envelope-less slice (`test_power_never_rendered_without_envelope`).
pub fn render_power_table(envelopes: &[PowerEnvelope]) -> Result<String, Refusal> {
    if envelopes.is_empty() {
        return Err(Refusal::new(
            RefusalCode::NoClaimInDomain201,
            "cannot render power table with zero envelopes",
        ));
    }
    let mut lines = Vec::with_capacity(envelopes.len());
    for env in envelopes {
        if env.twins_evaluated < 2 {
            return Err(Refusal::new(
                RefusalCode::OutsidePhysicalDomain109,
                "absolute single-twin power cannot be rendered without a multi-twin [min, max] envelope",
            ));
        }
        lines.push(env.render_envelope_string()?);
    }
    Ok(lines.join("\n"))
}

fn effective_snr_for(twin: &PlantTwin, subset: &[&str], cause: AttributionCause) -> (bool, f64) {
    let has = |name: &str| subset.contains(&name);
    let axes = twin.axes;
    let inv_autocorr = sqrt((1.0 - axes.intra_quarter_autocorr).max(0.15));
    let inv_sampler = 1.0 / sqrt(1.0 + axes.sampler_ratio * axes.sampler_ratio);
    let trans_contrast = (1.0 - axes.fresh_trans_kinetics).max(0.05) * 4.0;

    match cause {
        AttributionCause::HeadSamplerBias => {
            if has("head_tail_conc_samplers") && has("conc_shipment_settlement_assays") {
                (true, 4.2 * inv_sampler * inv_autocorr + 1.2)
            } else if has("head_tail_conc_samplers") {
                (true, 2.6 * inv_sampler * inv_autocorr + 0.6)
            } else if has("conc_shipment_settlement_assays") {
                (false, 0.8)
            } else {
                (false, 0.0)
            }
        }
        AttributionCause::TailSamplerBias => {
            if has("head_tail_conc_samplers") && has("monthly_reconciliation_pack") {
                (true, 3.8 * inv_sampler * inv_autocorr + 0.9)
            } else if has("head_tail_conc_samplers") {
                (true, 2.4 * inv_sampler * inv_autocorr + 0.5)
            } else {
                (false, 0.0)
            }
        }
        AttributionCause::TransitionalOreShare => {
            if has("fresh_transitional_feed_tags") && has("monthly_metallurgical_balance") {
                (true, 2.2 * trans_contrast * inv_autocorr + 1.4)
            } else if has("fresh_transitional_feed_tags") {
                (true, 1.8 * trans_contrast * inv_autocorr + 0.8)
            } else {
                (false, 0.0)
            }
        }
        AttributionCause::ThroughputDowntime => {
            if has("monthly_metallurgical_balance") && has("monthly_reconciliation_pack") {
                (true, 4.4 * inv_autocorr + 0.8)
            } else if has("monthly_metallurgical_balance") || has("monthly_reconciliation_pack") {
                (true, 3.0 * inv_autocorr + 0.5)
            } else {
                (false, 0.0)
            }
        }
        AttributionCause::AssayBias => {
            if has("conc_shipment_settlement_assays") && has("head_tail_conc_samplers") {
                (true, 4.5 * inv_sampler + 1.0)
            } else if has("conc_shipment_settlement_assays") {
                (true, 3.1 * inv_sampler + 0.7)
            } else {
                (false, 0.0)
            }
        }
    }
}

/// Run `replicates >= 200` sealed-truth `PhiloxStream` replicates for one `(twin, subset, cause)`.
pub fn estimate_twin_cause_power(
    twin: &PlantTwin,
    subset: &[&str],
    cause: AttributionCause,
    replicates: usize,
    seed_root: u64,
) -> Result<TwinCausePower, Refusal> {
    reject_personal_data_files(subset)?;
    if replicates < 200 {
        return Err(Refusal::new(
            RefusalCode::OutsidePhysicalDomain109,
            format!("BD-356.S2 requires replicates >= 200, got {replicates}"),
        ));
    }
    let (identifiable, snr) = effective_snr_for(twin, subset, cause);
    let z_crit = 1.959_963_984_540_054_f64; // two-sided alpha = 0.05
    if !identifiable {
        return Ok(TwinCausePower {
            twin_k: twin.k,
            cause,
            replicates,
            empirical_power: 0.0,
            analytic_power: 0.0,
            mc_se: 0.0,
            identifiable: false,
        });
    }
    let analytic_power = (1.0 - norm_cdf(z_crit - snr) + norm_cdf(-z_crit - snr)).clamp(0.0, 1.0);

    let mut stream = PhiloxStream::new(RngKey {
        seed_root,
        partition: twin.k as u32,
        domain: 356,
        var_group: 2,
        realization: cause as u32,
        brick: subset.len() as u64,
    });
    let mut detections = 0usize;
    for _ in 0..replicates {
        let u = ((stream.next_u64() >> 11) as f64) * (1.0 / ((1u64 << 53) as f64));
        let z = snr + norm_ppf(1e-12 + u * (1.0 - 2e-12));
        if z.abs() > z_crit {
            detections += 1;
        }
    }
    let emp = (detections as f64) / (replicates as f64);
    let mc_se = sqrt((emp * (1.0 - emp)).max(1e-6) / (replicates as f64));

    Ok(TwinCausePower {
        twin_k: twin.k,
        cause,
        replicates,
        empirical_power: emp,
        analytic_power,
        mc_se,
        identifiable: true,
    })
}

/// Compute the `[min_k, max_k]` [`PowerEnvelope`] across all twins in `family`.
pub fn estimate_power_envelope(
    family: &TwinFamily,
    subset: &[&str],
    cause: AttributionCause,
    replicates: usize,
    seed_root: u64,
) -> Result<PowerEnvelope, Refusal> {
    reject_personal_data_files(subset)?;
    if family.twins.is_empty() {
        return Err(Refusal::new(
            RefusalCode::NoClaimInDomain201,
            "TwinFamily has zero twins",
        ));
    }
    let mut min_p = 1.0_f64;
    let mut max_p = 0.0_f64;
    let mut max_se = 0.0_f64;
    let mut refusal: Option<Refusal> = None;

    for tw in &family.twins {
        let res = estimate_twin_cause_power(tw, subset, cause, replicates, seed_root)?;
        if res.empirical_power < min_p {
            min_p = res.empirical_power;
        }
        if res.empirical_power > max_p {
            max_p = res.empirical_power;
        }
        if res.mc_se > max_se {
            max_se = res.mc_se;
        }
        if !res.identifiable && refusal.is_none() {
            refusal = Some(Refusal::new(
                RefusalCode::NonIdentifiableAttribution104,
                format!(
                    "cause '{}' is non-identifiable from subset {:?} under twin {}; only composite recovery contrast is determined",
                    cause.as_str(),
                    subset,
                    tw.k
                ),
            ));
        }
    }

    Ok(PowerEnvelope {
        cause,
        subset: subset.iter().map(|s| (*s).to_string()).collect(),
        twins_evaluated: family.twins.len(),
        min_power: min_p,
        max_power: max_p,
        max_mc_se: max_se,
        identifiability_refusal: refusal,
    })
}

/// File robustness label assigned across a twin family (`BD-356.S2`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileLabel {
    /// Selected in the top set under EVERY twin in the family.
    Robust,
    /// Selected under some (or zero) twins, depending on unidentifiable twin axes.
    ModelDependent { reason: String },
}

/// Labelled candidate file row handed from `S2` to `S3`.
#[derive(Debug, Clone, PartialEq)]
pub struct LabelledCandidateFile {
    pub name: String,
    pub extraction_cost: f64,
    pub label: FileLabel,
    pub selected_in_twins: usize,
    pub total_twins: usize,
}

/// Greedy forward file selection per twin by marginal analytic power per unit extraction cost,
/// breaking exact ties deterministically by ascending file name (`BD-356.S2`).
pub fn greedy_select_for_twin(
    twin: &PlantTwin,
    candidates: &[CandidateFileCostRow],
    top_n: usize,
) -> Result<Vec<String>, Refusal> {
    let names: Vec<&str> = candidates.iter().map(|c| c.name.as_str()).collect();
    reject_personal_data_files(&names)?;

    let mut chosen: Vec<String> = Vec::new();
    let mut remaining: Vec<&CandidateFileCostRow> = candidates.iter().collect();
    remaining.sort_by(|a, b| a.name.cmp(&b.name));

    let total_power = |subset: &[&str]| -> f64 {
        let mut s = 0.0_f64;
        for c in AttributionCause::ALL {
            let (ok, snr) = effective_snr_for(twin, subset, c);
            if ok {
                let z_crit = 1.959_963_984_540_054_f64;
                s += (1.0 - norm_cdf(z_crit - snr) + norm_cdf(-z_crit - snr)).clamp(0.0, 1.0);
            }
        }
        s / (AttributionCause::ALL.len() as f64)
    };

    for _ in 0..top_n.min(candidates.len()) {
        let base_refs: Vec<&str> = chosen.iter().map(String::as_str).collect();
        let base_p = total_power(&base_refs);

        let mut best_idx = 0usize;
        let mut best_score = f64::NEG_INFINITY;
        for (i, cand) in remaining.iter().enumerate() {
            let cost = cand.extraction_cost.max(1e-6);
            let mut next_refs = base_refs.clone();
            next_refs.push(cand.name.as_str());
            let gain = (total_power(&next_refs) - base_p) / cost;
            // Strict `>` with remaining sorted by ascending name guarantees deterministic name tie-break.
            if gain > best_score + 1e-12 {
                best_score = gain;
                best_idx = i;
            }
        }
        let picked = remaining.remove(best_idx);
        chosen.push(picked.name.clone());
    }
    Ok(chosen)
}

/// Label every candidate file `Robust` (selected in every twin's top-`top_n` set) or
/// `ModelDependent { reason }` (`BD-356.S2`).
pub fn label_candidate_files(
    family: &TwinFamily,
    candidates: &[CandidateFileCostRow],
    top_n: usize,
) -> Result<Vec<LabelledCandidateFile>, Refusal> {
    let mut per_twin_sets = Vec::with_capacity(family.twins.len());
    for tw in &family.twins {
        let picked = greedy_select_for_twin(tw, candidates, top_n)?;
        let set: BTreeSet<String> = picked.into_iter().collect();
        per_twin_sets.push(set);
    }

    let mut out = Vec::with_capacity(candidates.len());
    for cand in candidates {
        let count = per_twin_sets
            .iter()
            .filter(|s| s.contains(&cand.name))
            .count();
        let label = if count == family.twins.len() && !family.twins.is_empty() {
            FileLabel::Robust
        } else {
            FileLabel::ModelDependent {
                reason: format!(
                    "selected in {count}/{} twins (sensitivity to unidentifiable sampler/autocorr/kinetics axes)",
                    family.twins.len()
                ),
            }
        };
        out.push(LabelledCandidateFile {
            name: cand.name.clone(),
            extraction_cost: cand.extraction_cost,
            label,
            selected_in_twins: count,
            total_twins: family.twins.len(),
        });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    use crate::plant_twin::{
        CitedPoint, CitedSeries, PilotTwinRegistrySpec, QuarterAgg, parse_pilot_twins_registry,
    };

    fn load_family(k_override: Option<usize>) -> (PilotTwinRegistrySpec, TwinFamily) {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|p| p.parent())
            .expect("workspace root")
            .to_path_buf();
        let text = std::fs::read_to_string(root.join("registries/pilot_twins.toml"))
            .expect("read pilot_twins.toml");
        let mut spec = parse_pilot_twins_registry(&text, "lumwana").expect("parse spec");
        if let Some(k) = k_override {
            spec.k = k;
        }
        let targets = vec![
            QuarterAgg {
                quarter: "2025Q1".to_string(),
                throughput_kt: 5_250.0,
                head_grade_cu_pct: 0.57,
                recovery_pct: 91.0,
                copper_kt: 27.2,
                segment_boundary_reset: true,
            },
            QuarterAgg {
                quarter: "2025Q2".to_string(),
                throughput_kt: 7_080.0,
                head_grade_cu_pct: 0.67,
                recovery_pct: 92.0,
                copper_kt: 43.6,
                segment_boundary_reset: false,
            },
        ];
        let public = vec![CitedSeries {
            series_name: "recovery".to_string(),
            unit: "pct".to_string(),
            points: vec![
                CitedPoint {
                    quarter: "2025Q1".to_string(),
                    cell: "2025Q1:Minestats4!D14".to_string(),
                    source_sha256: "synth".to_string(),
                    printed_value: 91.0,
                    rounding_step: 1.0,
                },
                CitedPoint {
                    quarter: "2025Q2".to_string(),
                    cell: "2025Q2:Minestats4!D14".to_string(),
                    source_sha256: "synth".to_string(),
                    printed_value: 92.0,
                    rounding_step: 1.0,
                },
            ],
        }];
        let family =
            TwinFamily::build_and_verify(&spec, 2_026_356, &targets, &public).expect("family");
        (spec, family)
    }

    #[test]
    fn test_gb25_two_bias_refused_correlation_0_92() {
        // GB-25: y = b_gc - b_mill + eps = -9.6%, sigma_eps = 1.5%, priors N(0, 5%) each.
        let post = solve_two_bias_posterior(
            0.05,
            0.05,
            &[LinearBiasObs {
                h1: 1.0,
                h2: -1.0,
                y: -0.096,
                sigma: 0.015,
            }],
        )
        .expect("solve GB-25");

        assert!(
            (post.correlation - 0.9174).abs() < 0.005,
            "expected corr ~0.92, got {}",
            post.correlation
        );
        assert!((post.std_b1 - 0.0361).abs() < 0.002);
        assert!((post.std_b2 - 0.0361).abs() < 0.002);
        assert!((post.diff_std - 0.0147).abs() < 0.002);
        let err = post.attribution_status.unwrap_err();
        assert_eq!(err.code, "FMINE-REF-104");
    }

    #[test]
    fn test_gb25_after_audit_attributes_minus_7_1() {
        // GB-25 after mill-sampler audit: b_mill = +1.0% ± 2.0% -> b_gc = -7.1% ± 2.2%, b_mill = +1.8% ± 1.8%.
        let post = solve_two_bias_posterior(
            0.05,
            0.05,
            &[
                LinearBiasObs {
                    h1: 1.0,
                    h2: -1.0,
                    y: -0.096,
                    sigma: 0.015,
                },
                LinearBiasObs {
                    h1: 0.0,
                    h2: 1.0,
                    y: 0.010,
                    sigma: 0.020,
                },
            ],
        )
        .expect("solve GB-25 after audit");

        post.attribution_status
            .expect("audit breaks correlation below 0.80");
        assert!(
            (post.mean_b1 - (-0.0712)).abs() < 0.002,
            "expected b_gc ~ -7.1%, got {}",
            post.mean_b1
        );
        assert!(
            (post.std_b1 - 0.0215).abs() < 0.002,
            "expected std(b_gc) ~ 2.2%, got {}",
            post.std_b1
        );
        assert!(
            (post.mean_b2 - 0.0184).abs() < 0.002,
            "expected b_mill ~ +1.8%, got {}",
            post.mean_b2
        );
        assert!(
            (post.std_b2 - 0.0175).abs() < 0.002,
            "expected std(b_mill) ~ 1.8%, got {}",
            post.std_b2
        );
    }

    #[test]
    fn test_adding_file_never_lowers_identifiability_rank() {
        let all = [
            "monthly_metallurgical_balance",
            "head_tail_conc_samplers",
            "conc_shipment_settlement_assays",
            "fresh_transitional_feed_tags",
            "monthly_reconciliation_pack",
        ];
        let mut sub: Vec<&str> = Vec::new();
        let mut prev_rank = subset_identifiability_rank(&sub);
        assert_eq!(prev_rank, 0);
        for f in all {
            sub.push(f);
            let next_rank = subset_identifiability_rank(&sub);
            assert!(next_rank >= prev_rank);
            prev_rank = next_rank;
        }
        assert_eq!(prev_rank, 5);
    }

    #[test]
    fn test_envelope_widens_or_stays_when_k_grows() {
        let (_, fam_small) = load_family(Some(2));
        let (_, fam_large) = load_family(Some(5));
        let subset = ["head_tail_conc_samplers", "fresh_transitional_feed_tags"];
        let e_small = estimate_power_envelope(
            &fam_small,
            &subset,
            AttributionCause::TransitionalOreShare,
            400,
            2_026_356,
        )
        .expect("small envelope");
        let e_large = estimate_power_envelope(
            &fam_large,
            &subset,
            AttributionCause::TransitionalOreShare,
            400,
            2_026_356,
        )
        .expect("large envelope");

        // Strictly nested twin prefix (F_2 ⊂ F_5) guarantees monotone envelope widening:
        // min_power can only decrease or stay equal, max_power can only increase or stay equal.
        assert!(
            e_large.min_power <= e_small.min_power + 1e-12
                && e_large.max_power >= e_small.max_power - 1e-12,
            "envelope failed monotonic widening: small=[{}, {}] vs large=[{}, {}]",
            e_small.min_power,
            e_small.max_power,
            e_large.min_power,
            e_large.max_power
        );
    }

    #[test]
    fn test_detection_rate_within_mc_error_200_replicates() {
        let (_, fam) = load_family(Some(5));
        let subset = ["head_tail_conc_samplers", "conc_shipment_settlement_assays"];
        for tw in &fam.twins {
            let p = estimate_twin_cause_power(
                tw,
                &subset,
                AttributionCause::HeadSamplerBias,
                200,
                2_026_356,
            )
            .expect("200 reps");
            assert!(p.identifiable);
            assert!(p.mc_se <= 0.036);
            // Empirical detection rate lies within 2.5 MC SE of analytic power across all twins.
            let tol = (2.5 * p.mc_se).max(0.035);
            assert!(
                (p.empirical_power - p.analytic_power).abs() <= tol,
                "twin {}: empirical {} vs analytic {} (se={})",
                tw.k,
                p.empirical_power,
                p.analytic_power,
                p.mc_se
            );
        }
    }

    #[test]
    fn test_power_never_rendered_without_envelope() {
        let (_, fam_single) = load_family(Some(1));
        let single_env = estimate_power_envelope(
            &fam_single,
            &["head_tail_conc_samplers"],
            AttributionCause::HeadSamplerBias,
            200,
            2_026_356,
        )
        .expect("single env");
        let err = render_power_table(&[single_env]).unwrap_err();
        assert_eq!(err.code, "FMINE-REF-109");

        let (_, fam_multi) = load_family(Some(5));
        let env = estimate_power_envelope(
            &fam_multi,
            &["head_tail_conc_samplers"],
            AttributionCause::HeadSamplerBias,
            200,
            2_026_356,
        )
        .expect("multi env");
        let rendered = render_power_table(&[env]).expect("multi-twin envelope renders");
        assert!(
            rendered.contains('[') && rendered.contains(']'),
            "{rendered}"
        );
    }

    #[test]
    fn test_personal_data_file_request_refused() {
        let (_, fam) = load_family(Some(3));
        let err = estimate_power_envelope(
            &fam,
            &["monthly_metallurgical_balance", "operator_roster_shifts"],
            AttributionCause::ThroughputDowntime,
            200,
            2_026_356,
        )
        .unwrap_err();
        assert_eq!(err.code, "FMINE-REF-301");
    }

    #[test]
    fn test_greedy_selection_deterministic_tie_break() {
        let (spec, fam) = load_family(Some(5));
        let tw = &fam.twins[0];
        let a = greedy_select_for_twin(tw, &spec.candidate_files, 3).expect("run 1");
        let mut rev = spec.candidate_files.clone();
        rev.reverse();
        let b = greedy_select_for_twin(tw, &rev, 3).expect("run 2 reversed input");
        assert_eq!(a, b);
        assert_eq!(a.len(), 3);

        // Two identically-costed dummy files with identical zero marginal gain break ties alphabetically.
        let tied = vec![
            CandidateFileCostRow {
                site: "lumwana".to_string(),
                name: "z_aux_log".to_string(),
                extraction_cost: 1.0,
                status: "TARGET".to_string(),
                description: "z".to_string(),
            },
            CandidateFileCostRow {
                site: "lumwana".to_string(),
                name: "a_aux_log".to_string(),
                extraction_cost: 1.0,
                status: "TARGET".to_string(),
                description: "a".to_string(),
            },
        ];
        let pick = greedy_select_for_twin(tw, &tied, 1).expect("tie break");
        assert_eq!(pick, vec!["a_aux_log".to_string()]);

        // Verify full family labeling produces Robust / ModelDependent labels with reasons.
        let labels = label_candidate_files(&fam, &spec.candidate_files, 3).expect("label files");
        assert_eq!(labels.len(), 5);
        assert!(labels.iter().any(|l| l.label == FileLabel::Robust));
    }
}
