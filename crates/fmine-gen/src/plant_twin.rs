//! Determinism: BitwisePortable
//! Lumwana plant-level synthetic twin family and public quarterly cell parity (`BD-356.S1`,
//! plan §37.3, `INV-006`, `INV-010`, `E551`, `E564–E566`, `E580–E584`).
//!
//! All twins reproduce cited public quarterly cells within `[v - step/2, v + step/2]` and vary
//! strictly along three registered unidentifiable axes (`sampler_ratio`, `intra_quarter_autocorr`,
//! `fresh_trans_kinetics`) without smoothing across registered plant segment boundaries.
//! Output is explicitly synthetic and model-derived ("from public filings; no affiliation implied").

use std::collections::BTreeSet;

use fmine_core::codec::Encoder;
use fmine_core::det::{norm_ppf, sqrt};
use fmine_core::hash::{Digest32, sha256};
use fmine_core::rand::{PhiloxStream, RngKey};
use fmine_evidence::{Refusal, RefusalCode};

/// Required provenance and non-affiliation disclaimer on all synthetic pilot twin outputs (`INV-006`).
pub const SYNTHETIC_PUBLIC_LABEL: &str =
    "synthetic twins; from public filings; no affiliation implied";

/// Three axes unidentifiable from public quarterly aggregates along which twins vary (`BD-356.S1`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TwinAxes {
    /// Ratio `σ_sampler / σ_process` splitting total variance between sampler noise and process noise.
    pub sampler_ratio: f64,
    /// Within-quarter AR(1) lag-1 autocorrelation `φ ∈ [0, 1)` (reset at segment boundaries).
    pub intra_quarter_autocorr: f64,
    /// Fresh-vs-transitional flotation kinetic ratio (`R_trans / R_fresh ∈ (0, 1]`).
    pub fresh_trans_kinetics: f64,
}

/// Registered `[min, max]` ranges for the three unidentifiable twin axes (`registries/pilot_twins.toml`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AxisRanges {
    pub sampler_ratio: (f64, f64),
    pub intra_quarter_autocorr: (f64, f64),
    pub fresh_trans_kinetics: (f64, f64),
}

impl AxisRanges {
    fn validate(&self) -> Result<(), Refusal> {
        let ok = |lo: f64, hi: f64, lb: f64, ub: f64| {
            lo.is_finite() && hi.is_finite() && lo >= lb && hi <= ub && lo <= hi
        };
        if !ok(self.sampler_ratio.0, self.sampler_ratio.1, 0.0, 10.0)
            || !ok(
                self.intra_quarter_autocorr.0,
                self.intra_quarter_autocorr.1,
                0.0,
                0.99,
            )
            || !ok(
                self.fresh_trans_kinetics.0,
                self.fresh_trans_kinetics.1,
                0.05,
                1.0,
            )
        {
            return Err(Refusal::new(
                RefusalCode::OutsidePhysicalDomain109,
                "invalid TwinAxes ranges in pilot_twins.toml",
            ));
        }
        Ok(())
    }

    /// Check that `axes` lies within registered bounds.
    pub fn contains(&self, axes: TwinAxes) -> bool {
        (self.sampler_ratio.0..=self.sampler_ratio.1).contains(&axes.sampler_ratio)
            && (self.intra_quarter_autocorr.0..=self.intra_quarter_autocorr.1)
                .contains(&axes.intra_quarter_autocorr)
            && (self.fresh_trans_kinetics.0..=self.fresh_trans_kinetics.1)
                .contains(&axes.fresh_trans_kinetics)
    }
}

/// Operational discontinuity boundary across which twins NEVER smooth (`E580–E584`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SegmentBoundary {
    pub site: String,
    pub quarter: String,
    pub event: String,
    pub source_e_id: String,
}

/// Candidate file extraction cost row from `registries/pilot_twins.toml` (TARGET/OPEN placeholder).
#[derive(Debug, Clone, PartialEq)]
pub struct CandidateFileCostRow {
    pub site: String,
    pub name: String,
    pub extraction_cost: f64,
    pub status: String,
    pub description: String,
}

/// Site twin-family specification parsed from `registries/pilot_twins.toml`.
#[derive(Debug, Clone, PartialEq)]
pub struct PilotTwinRegistrySpec {
    pub site: String,
    pub ownership_pct: u32,
    pub copper_unit: String,
    pub k: usize,
    pub status: String,
    pub axis_ranges: AxisRanges,
    pub power_bar_target: f64,
    pub segment_boundaries: Vec<SegmentBoundary>,
    pub candidate_files: Vec<CandidateFileCostRow>,
}

/// One cited public quarterly cell (`E564–E566`).
#[derive(Debug, Clone, PartialEq)]
pub struct CitedPoint {
    pub quarter: String,
    pub cell: String,
    pub source_sha256: String,
    pub printed_value: f64,
    /// Published rounding step (e.g. `1.0` for integer %, `0.01` for 2-decimal grade, `1.0` for kt).
    pub rounding_step: f64,
}

/// One cited public quarterly series (`recovery`, `head_grade`, `throughput`, `copper_kt`).
#[derive(Debug, Clone, PartialEq)]
pub struct CitedSeries {
    pub series_name: String,
    pub unit: String,
    pub points: Vec<CitedPoint>,
}

/// Quarterly aggregate output emitted by a [`PlantTwin`].
#[derive(Debug, Clone, PartialEq)]
pub struct QuarterAgg {
    pub quarter: String,
    pub throughput_kt: f64,
    pub head_grade_cu_pct: f64,
    pub recovery_pct: f64,
    pub copper_kt: f64,
    /// True iff this quarter sits on a registered segment boundary where AR(1) carryover reset.
    pub segment_boundary_reset: bool,
}

/// One day-level synthetic micro-state inside a twin's quarterly realization.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DailyMicroPoint {
    pub quarter_index: usize,
    pub day_index: usize,
    pub ar1_process_state: f64,
    pub daily_throughput_kt: f64,
    pub daily_head_grade_pct: f64,
    pub daily_recovery_pct: f64,
    pub daily_sampler_noise_pct: f64,
}

/// One plant-level synthetic twin parameterized by its registered [`TwinAxes`] and `RngKey`.
#[derive(Debug, Clone, PartialEq)]
pub struct PlantTwin {
    pub k: usize,
    pub axes: TwinAxes,
    pub seed: RngKey,
    pub label: &'static str,
    quarters: Vec<QuarterAgg>,
    daily: Vec<DailyMicroPoint>,
    pub twin_digest: Digest32,
}

/// Complete family of `K` synthetic plant twins agreeing on cited public aggregates (`BD-356.S1`).
#[derive(Debug, Clone, PartialEq)]
pub struct TwinFamily {
    pub site: String,
    pub k: usize,
    pub axis_ranges: AxisRanges,
    pub segment_boundaries: Vec<SegmentBoundary>,
    pub twins: Vec<PlantTwin>,
    pub label: &'static str,
    pub family_digest: Digest32,
}

fn draw_normal(stream: &mut PhiloxStream) -> f64 {
    let u = ((stream.next_u64() >> 11) as f64) * (1.0 / ((1u64 << 53) as f64));
    norm_ppf(1e-12 + u * (1.0 - 2e-12))
}

fn van_der_corput(mut n: usize, base: usize) -> f64 {
    let mut out = 0.0_f64;
    let mut denom = 1.0_f64;
    while n > 0 {
        denom *= base as f64;
        out += ((n % base) as f64) / denom;
        n /= base;
    }
    out
}

/// Deterministic nested coordinates in `[0, 1]^3` independent of total family size `K`.
fn nested_axis_coords(idx: usize) -> (f64, f64, f64) {
    match idx {
        0 => (0.0, 0.0, 1.0),
        1 => (1.0, 1.0, 0.0),
        2 => (0.0, 1.0, 0.0),
        3 => (1.0, 0.0, 1.0),
        4 => (0.5, 0.5, 0.5),
        n => (
            van_der_corput(n - 4, 2),
            van_der_corput(n - 4, 3),
            van_der_corput(n - 4, 5),
        ),
    }
}

impl PlantTwin {
    /// Construct one deterministic `PlantTwin` matching `targets` while generating daily micro-series
    /// controlled by `axes` and resetting AR(1) state at every `boundary_quarters` entry.
    pub fn build(
        k: usize,
        axes: TwinAxes,
        seed: RngKey,
        targets: &[QuarterAgg],
        boundary_quarters: &BTreeSet<String>,
        injected_recovery_shift_pct: f64,
    ) -> Result<Self, Refusal> {
        if targets.is_empty() {
            return Err(Refusal::new(
                RefusalCode::NoClaimInDomain201,
                "PlantTwin requires at least one target quarter",
            ));
        }
        if !(axes.sampler_ratio.is_finite()
            && (0.0..=10.0).contains(&axes.sampler_ratio)
            && axes.intra_quarter_autocorr.is_finite()
            && (0.0..=0.99).contains(&axes.intra_quarter_autocorr)
            && axes.fresh_trans_kinetics.is_finite()
            && (0.05..=1.0).contains(&axes.fresh_trans_kinetics)
            && injected_recovery_shift_pct.is_finite())
        {
            return Err(Refusal::new(
                RefusalCode::OutsidePhysicalDomain109,
                "PlantTwin axes or injected_recovery_shift_pct out of physical domain",
            ));
        }
        let mut seen_quarters = BTreeSet::new();
        for tgt in targets {
            if tgt.quarter.is_empty() || !seen_quarters.insert(tgt.quarter.as_str()) {
                return Err(Refusal::new(
                    RefusalCode::OutsidePhysicalDomain109,
                    format!("empty or duplicate target quarter {:?}", tgt.quarter),
                ));
            }
            if !(tgt.throughput_kt.is_finite()
                && tgt.throughput_kt > 0.0
                && tgt.head_grade_cu_pct.is_finite()
                && (0.0..=100.0).contains(&tgt.head_grade_cu_pct)
                && tgt.recovery_pct.is_finite()
                && (0.0..=100.0).contains(&tgt.recovery_pct))
            {
                return Err(Refusal::new(
                    RefusalCode::OutsidePhysicalDomain109,
                    format!(
                        "target quarter '{}' values out of physical domain",
                        tgt.quarter
                    ),
                ));
            }
        }
        const DAYS_PER_QUARTER: usize = 30;
        let mut stream = PhiloxStream::new(seed);
        let phi = axes.intra_quarter_autocorr;
        let innov_scale = sqrt((1.0 - phi * phi).max(1e-6));
        let total_sigma = 0.80_f64;
        let process_sigma = total_sigma / sqrt(1.0 + axes.sampler_ratio * axes.sampler_ratio);
        let sampler_sigma = process_sigma * axes.sampler_ratio;

        let mut prev_state = 0.0_f64;
        let mut quarters = Vec::with_capacity(targets.len());
        let mut daily = Vec::with_capacity(targets.len() * DAYS_PER_QUARTER);

        for (q_idx, tgt) in targets.iter().enumerate() {
            let is_boundary = boundary_quarters.contains(&tgt.quarter);
            if is_boundary {
                // Segment boundary law (E580-E584): never smooth across shutdowns/relines/tie-ins.
                prev_state = 0.0;
            }
            let mut raw_states = [0.0_f64; DAYS_PER_QUARTER];
            let mut raw_sampler = [0.0_f64; DAYS_PER_QUARTER];
            let mut sum_s = 0.0_f64;
            for d in 0..DAYS_PER_QUARTER {
                let eps = draw_normal(&mut stream);
                let samp = draw_normal(&mut stream);
                prev_state = phi * prev_state + innov_scale * eps;
                raw_states[d] = prev_state;
                raw_sampler[d] = sampler_sigma * samp;
                sum_s += prev_state;
            }
            // Zero-mean centering within the quarter preserves exact quarterly parity while letting
            // each twin have distinct daily autocorrelation, sampler noise, and fresh/trans kinetics.
            let mean_s = sum_s / (DAYS_PER_QUARTER as f64);
            let mut q_tp = 0.0_f64;
            let mut q_metal_in = 0.0_f64;
            let mut q_metal_rec = 0.0_f64;

            let daily_base_tp = tgt.throughput_kt / (DAYS_PER_QUARTER as f64);
            // Fresh vs. transitional kinetics spread scales daily recovery swing around quarterly target.
            let kinetic_spread = (1.0 - axes.fresh_trans_kinetics) * 4.0 + 0.4;

            for d in 0..DAYS_PER_QUARTER {
                let centered = raw_states[d] - mean_s;
                let d_tp = (daily_base_tp * (1.0 + 0.015 * centered)).max(0.1);
                let d_grade = tgt.head_grade_cu_pct;
                let d_rec = (tgt.recovery_pct
                    + injected_recovery_shift_pct
                    + process_sigma * kinetic_spread * centered)
                    .clamp(50.0, 99.5);
                q_tp += d_tp;
                let metal_in = d_tp * (d_grade / 100.0);
                q_metal_in += metal_in;
                q_metal_rec += metal_in * (d_rec / 100.0);

                daily.push(DailyMicroPoint {
                    quarter_index: q_idx,
                    day_index: d,
                    ar1_process_state: raw_states[d],
                    daily_throughput_kt: d_tp,
                    daily_head_grade_pct: d_grade,
                    daily_recovery_pct: d_rec,
                    daily_sampler_noise_pct: raw_sampler[d],
                });
            }

            let agg_grade = if q_tp > 0.0 {
                (q_metal_in / q_tp) * 100.0
            } else {
                0.0
            };
            let agg_rec = if q_metal_in > 0.0 {
                (q_metal_rec / q_metal_in) * 100.0
            } else {
                0.0
            };
            quarters.push(QuarterAgg {
                quarter: tgt.quarter.clone(),
                throughput_kt: q_tp,
                head_grade_cu_pct: agg_grade,
                recovery_pct: agg_rec,
                copper_kt: q_metal_rec,
                segment_boundary_reset: is_boundary,
            });
        }

        let mut enc = Encoder::new_frame(*b"PTWN", 1);
        enc.put_u32(k as u32);
        enc.put_u64(seed.seed_root);
        enc.put_u32(seed.partition);
        enc.put_u64(axes.sampler_ratio.to_bits());
        enc.put_u64(axes.intra_quarter_autocorr.to_bits());
        enc.put_u64(axes.fresh_trans_kinetics.to_bits());
        enc.put_u32(quarters.len() as u32);
        for q in &quarters {
            enc.put_str(&q.quarter);
            enc.put_u64(q.throughput_kt.to_bits());
            enc.put_u64(q.head_grade_cu_pct.to_bits());
            enc.put_u64(q.recovery_pct.to_bits());
            enc.put_u8(u8::from(q.segment_boundary_reset));
        }

        Ok(Self {
            k,
            axes,
            seed,
            label: SYNTHETIC_PUBLIC_LABEL,
            quarters,
            daily,
            twin_digest: sha256(&enc.finish()),
        })
    }

    /// Return the quarterly aggregates produced by this twin (`BD-356.S1`).
    #[must_use]
    pub fn quarterly_aggregates(&self) -> Vec<QuarterAgg> {
        self.quarters.clone()
    }

    /// Return the underlying day-level synthetic micro-series (for axis & boundary verification).
    #[must_use]
    pub fn daily_series(&self) -> &[DailyMicroPoint] {
        &self.daily
    }

    /// Verify cell-by-cell parity against `public`: every cell must lie in
    /// `[printed - step/2, printed + step/2]`, and copper must be in `kt` (never `Mlb`, `E565`).
    /// Any out-of-rounding cell refuses with `FMINE-REF-101` (never tuned silently).
    pub fn check_parity(&self, public: &[CitedSeries]) -> Result<(), Refusal> {
        if public.is_empty() {
            return Err(Refusal::new(
                RefusalCode::NoClaimInDomain201,
                "check_parity requires at least one public CitedSeries",
            ));
        }
        // Pre-pass: validate all series units and schema before numeric parity comparison.
        for series in public {
            if series.unit.eq_ignore_ascii_case("mlb") {
                return Err(Refusal::new(
                    RefusalCode::BasisMismatch105,
                    format!(
                        "series '{}' uses forbidden unit Mlb; copper must be in kt (E565)",
                        series.series_name
                    ),
                ));
            }
            if !matches!(
                series.series_name.as_str(),
                "recovery" | "head_grade" | "throughput" | "copper_kt"
            ) {
                return Err(Refusal::new(
                    RefusalCode::UndeclaredUnitBasisOrTz107,
                    format!("unknown public series '{}'", series.series_name),
                ));
            }
            if series.points.is_empty() {
                return Err(Refusal::new(
                    RefusalCode::NoClaimInDomain201,
                    format!("public series '{}' has zero points", series.series_name),
                ));
            }
        }
        for series in public {
            for pt in &series.points {
                if pt.cell.is_empty() || pt.source_sha256.is_empty() {
                    return Err(Refusal::new(
                        RefusalCode::UndeclaredUnitBasisOrTz107,
                        format!(
                            "public point in '{}' ({}) requires non-empty cell and source_sha256",
                            series.series_name, pt.quarter
                        ),
                    ));
                }
                if !pt.printed_value.is_finite()
                    || pt.rounding_step <= 0.0
                    || !pt.rounding_step.is_finite()
                {
                    return Err(Refusal::new(
                        RefusalCode::OutsidePhysicalDomain109,
                        format!(
                            "non-finite value or non-positive rounding_step on {}",
                            pt.cell
                        ),
                    ));
                }
                let Some(q) = self.quarters.iter().find(|q| q.quarter == pt.quarter) else {
                    return Err(Refusal::new(
                        RefusalCode::NoClaimInDomain201,
                        format!(
                            "twin {} missing public quarter {} ({})",
                            self.k, pt.quarter, pt.cell
                        ),
                    ));
                };
                let val = match series.series_name.as_str() {
                    "recovery" => q.recovery_pct,
                    "head_grade" => q.head_grade_cu_pct,
                    "throughput" => q.throughput_kt,
                    "copper_kt" => q.copper_kt,
                    other => {
                        return Err(Refusal::new(
                            RefusalCode::UndeclaredUnitBasisOrTz107,
                            format!("unknown public series '{other}'"),
                        ));
                    }
                };
                let half = pt.rounding_step * 0.5 + 1e-9;
                let lo = pt.printed_value - half;
                let hi = pt.printed_value + half;
                if !(lo..=hi).contains(&val) {
                    return Err(Refusal::new(
                        RefusalCode::Conservation101,
                        format!(
                            "twin {} parity failure at {} {} ({}): twin={val:.4} outside [{lo:.4}, {hi:.4}] (printed={}, step={})",
                            self.k,
                            pt.quarter,
                            series.series_name,
                            pt.cell,
                            pt.printed_value,
                            pt.rounding_step
                        ),
                    ));
                }
            }
        }
        Ok(())
    }
}

impl TwinFamily {
    /// Build a deterministic `K`-twin family spanning `spec.axis_ranges` and verify parity on all
    /// twins against `public`. Any failing twin refuses the entire family.
    pub fn build_and_verify(
        spec: &PilotTwinRegistrySpec,
        seed_root: u64,
        targets: &[QuarterAgg],
        public: &[CitedSeries],
    ) -> Result<Self, Refusal> {
        spec.axis_ranges.validate()?;
        if spec.k == 0 {
            return Err(Refusal::new(
                RefusalCode::OutsidePhysicalDomain109,
                "TwinFamily requires k >= 1",
            ));
        }
        if spec.copper_unit != "kt" {
            return Err(Refusal::new(
                RefusalCode::BasisMismatch105,
                format!(
                    "copper_unit must be 'kt' (E565), got '{}'",
                    spec.copper_unit
                ),
            ));
        }
        let boundaries: BTreeSet<String> = spec
            .segment_boundaries
            .iter()
            .filter(|b| b.site == spec.site)
            .map(|b| b.quarter.clone())
            .collect();

        let mut twins = Vec::with_capacity(spec.k);
        for idx in 0..spec.k {
            // Deterministic nested prefix coordinates in [0, 1]^3 independent of spec.k so
            // F_k1 is strictly a subset of F_k2 whenever k1 <= k2 (monotone power envelopes).
            let (u0, u1, u2) = nested_axis_coords(idx);
            let axes = TwinAxes {
                sampler_ratio: spec.axis_ranges.sampler_ratio.0
                    + u0 * (spec.axis_ranges.sampler_ratio.1 - spec.axis_ranges.sampler_ratio.0),
                intra_quarter_autocorr: spec.axis_ranges.intra_quarter_autocorr.0
                    + u1 * (spec.axis_ranges.intra_quarter_autocorr.1
                        - spec.axis_ranges.intra_quarter_autocorr.0),
                fresh_trans_kinetics: spec.axis_ranges.fresh_trans_kinetics.0
                    + u2 * (spec.axis_ranges.fresh_trans_kinetics.1
                        - spec.axis_ranges.fresh_trans_kinetics.0),
            };
            let seed = RngKey {
                seed_root,
                partition: idx as u32,
                domain: 356,
                var_group: 1,
                realization: 0,
                brick: 0,
            };
            let twin = PlantTwin::build(idx, axes, seed, targets, &boundaries, 0.0)?;
            twin.check_parity(public)?;
            twins.push(twin);
        }

        let mut enc = Encoder::new_frame(*b"TFAM", 1);
        enc.put_str(&spec.site);
        enc.put_u32(spec.k as u32);
        enc.put_u64(seed_root);
        for tw in &twins {
            enc.put_fixed(tw.twin_digest.as_bytes());
        }

        Ok(Self {
            site: spec.site.clone(),
            k: spec.k,
            axis_ranges: spec.axis_ranges,
            segment_boundaries: spec.segment_boundaries.clone(),
            twins,
            label: SYNTHETIC_PUBLIC_LABEL,
            family_digest: sha256(&enc.finish()),
        })
    }

    /// Check parity of every twin in this family against `public`.
    pub fn check_parity(&self, public: &[CitedSeries]) -> Result<(), Refusal> {
        for tw in &self.twins {
            tw.check_parity(public)?;
        }
        Ok(())
    }
}

/// Parse and validate `registries/pilot_twins.toml` for `site`.
pub fn parse_pilot_twins_registry(
    text: &str,
    target_site: &str,
) -> Result<PilotTwinRegistrySpec, Refusal> {
    let mut saw_version = false;
    let mut section = "";
    let mut cur: Vec<(String, String)> = Vec::new();
    let mut site_rows: Vec<Vec<(String, String)>> = Vec::new();
    let mut boundary_rows: Vec<Vec<(String, String)>> = Vec::new();
    let mut file_rows: Vec<Vec<(String, String)>> = Vec::new();

    let push_cur = |sec: &str,
                    fields: &mut Vec<(String, String)>,
                    sites: &mut Vec<Vec<(String, String)>>,
                    bounds: &mut Vec<Vec<(String, String)>>,
                    files: &mut Vec<Vec<(String, String)>>| {
        if fields.is_empty() {
            return;
        }
        let taken = core::mem::take(fields);
        match sec {
            "site_twin" => sites.push(taken),
            "segment_boundary" => bounds.push(taken),
            "candidate_file" => files.push(taken),
            _ => {}
        }
    };

    for (idx, raw) in text.lines().enumerate() {
        let line = idx + 1;
        let s = raw.trim();
        if s.is_empty() || s.starts_with('#') {
            continue;
        }
        if s == "[[site_twin]]" || s == "[[segment_boundary]]" || s == "[[candidate_file]]" {
            if !saw_version {
                return Err(Refusal::new(
                    RefusalCode::UndeclaredUnitBasisOrTz107,
                    format!("pilot_twins.toml line {line}: schema_version = 1 required first"),
                ));
            }
            push_cur(
                section,
                &mut cur,
                &mut site_rows,
                &mut boundary_rows,
                &mut file_rows,
            );
            section = match s {
                "[[site_twin]]" => "site_twin",
                "[[segment_boundary]]" => "segment_boundary",
                _ => "candidate_file",
            };
            continue;
        }
        let Some((k, v)) = s.split_once('=') else {
            return Err(Refusal::new(
                RefusalCode::UndeclaredUnitBasisOrTz107,
                format!("pilot_twins.toml line {line}: expected key = value"),
            ));
        };
        let key = k.trim();
        let val = v.trim();
        if section.is_empty() {
            if !saw_version && key == "schema_version" && val == "1" {
                saw_version = true;
            } else {
                return Err(Refusal::new(
                    RefusalCode::UndeclaredUnitBasisOrTz107,
                    format!("pilot_twins.toml line {line}: expected single schema_version = 1"),
                ));
            }
        } else {
            if cur.iter().any(|(fk, _)| fk == key) {
                return Err(Refusal::new(
                    RefusalCode::UndeclaredUnitBasisOrTz107,
                    format!("pilot_twins.toml line {line}: duplicate key '{key}' in [[{section}]]"),
                ));
            }
            let parsed = val
                .strip_prefix('"')
                .and_then(|x| x.strip_suffix('"'))
                .map_or_else(|| val.to_string(), ToString::to_string);
            cur.push((key.to_string(), parsed));
        }
    }
    push_cur(
        section,
        &mut cur,
        &mut site_rows,
        &mut boundary_rows,
        &mut file_rows,
    );

    let get = |fields: &[(String, String)], k: &str| -> Result<String, Refusal> {
        fields
            .iter()
            .find(|(fk, _)| fk == k)
            .map(|(_, v)| v.clone())
            .ok_or_else(|| {
                Refusal::new(
                    RefusalCode::UndeclaredUnitBasisOrTz107,
                    format!("pilot_twins.toml missing field '{k}'"),
                )
            })
    };
    let get_f64 = |fields: &[(String, String)], k: &str| -> Result<f64, Refusal> {
        get(fields, k)?.parse::<f64>().map_err(|_| {
            Refusal::new(
                RefusalCode::OutsidePhysicalDomain109,
                format!("pilot_twins.toml invalid float for '{k}'"),
            )
        })
    };

    let matching_sites: Vec<&Vec<(String, String)>> = site_rows
        .iter()
        .filter(|f| f.iter().any(|(k, v)| k == "site" && v == target_site))
        .collect();
    if matching_sites.is_empty() {
        return Err(Refusal::new(
            RefusalCode::NoClaimInDomain201,
            format!("site '{target_site}' not found in pilot_twins.toml"),
        ));
    }
    if matching_sites.len() > 1 {
        return Err(Refusal::new(
            RefusalCode::UndeclaredUnitBasisOrTz107,
            format!("duplicate [[site_twin]] entry for site '{target_site}' in pilot_twins.toml"),
        ));
    }
    let sf = matching_sites[0];

    let axis_ranges = AxisRanges {
        sampler_ratio: (
            get_f64(sf, "sampler_ratio_min")?,
            get_f64(sf, "sampler_ratio_max")?,
        ),
        intra_quarter_autocorr: (
            get_f64(sf, "intra_quarter_autocorr_min")?,
            get_f64(sf, "intra_quarter_autocorr_max")?,
        ),
        fresh_trans_kinetics: (
            get_f64(sf, "fresh_trans_kinetics_min")?,
            get_f64(sf, "fresh_trans_kinetics_max")?,
        ),
    };
    axis_ranges.validate()?;

    let mut segment_boundaries = Vec::new();
    for bf in &boundary_rows {
        let site = get(bf, "site")?;
        if site == target_site {
            segment_boundaries.push(SegmentBoundary {
                site,
                quarter: get(bf, "quarter")?,
                event: get(bf, "event")?,
                source_e_id: get(bf, "source_e_id")?,
            });
        }
    }

    let mut candidate_files = Vec::new();
    let mut seen_file_names = BTreeSet::new();
    for ff in &file_rows {
        let site = get(ff, "site")?;
        if site == target_site {
            let name = get(ff, "name")?;
            if name.is_empty() || !seen_file_names.insert(name.clone()) {
                return Err(Refusal::new(
                    RefusalCode::UndeclaredUnitBasisOrTz107,
                    format!(
                        "empty or duplicate candidate_file name {name:?} for site '{target_site}'"
                    ),
                ));
            }
            let extraction_cost = get_f64(ff, "extraction_cost")?;
            if !(extraction_cost.is_finite() && extraction_cost > 0.0) {
                return Err(Refusal::new(
                    RefusalCode::OutsidePhysicalDomain109,
                    format!("candidate_file '{name}' requires finite extraction_cost > 0"),
                ));
            }
            candidate_files.push(CandidateFileCostRow {
                site,
                name,
                extraction_cost,
                status: get(ff, "status")?,
                description: get(ff, "description")?,
            });
        }
    }

    let ownership_pct: u32 = get(sf, "ownership_pct")?.parse().map_err(|_| {
        Refusal::new(
            RefusalCode::OutsidePhysicalDomain109,
            "invalid ownership_pct",
        )
    })?;
    if !(1..=100).contains(&ownership_pct) {
        return Err(Refusal::new(
            RefusalCode::OutsidePhysicalDomain109,
            format!("ownership_pct ({ownership_pct}) must lie in 1..=100"),
        ));
    }
    let k: usize = get(sf, "k")?
        .parse()
        .map_err(|_| Refusal::new(RefusalCode::OutsidePhysicalDomain109, "invalid k"))?;
    if k == 0 {
        return Err(Refusal::new(
            RefusalCode::OutsidePhysicalDomain109,
            "pilot_twins.toml requires k >= 1",
        ));
    }
    let power_bar_target = get_f64(sf, "power_bar_target")?;
    if !(power_bar_target.is_finite() && power_bar_target > 0.0 && power_bar_target <= 1.0) {
        return Err(Refusal::new(
            RefusalCode::OutsidePhysicalDomain109,
            format!("power_bar_target ({power_bar_target}) must lie in (0, 1]"),
        ));
    }

    Ok(PilotTwinRegistrySpec {
        site: target_site.to_string(),
        ownership_pct,
        copper_unit: get(sf, "copper_unit")?,
        k,
        status: get(sf, "status")?,
        axis_ranges,
        power_bar_target,
        segment_boundaries,
        candidate_files,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn load_lumwana_spec() -> PilotTwinRegistrySpec {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|p| p.parent())
            .expect("workspace root")
            .to_path_buf();
        let text = std::fs::read_to_string(root.join("registries/pilot_twins.toml"))
            .expect("read pilot_twins.toml");
        parse_pilot_twins_registry(&text, "lumwana").expect("parse lumwana spec")
    }

    /// Synthetic public series shaped like the 14 Lumwana quarters (used strictly in unit tests).
    fn synthetic_public_series() -> (Vec<QuarterAgg>, Vec<CitedSeries>) {
        let rows: [(&str, f64, f64, f64); 14] = [
            ("2023Q1", 6_500.0, 0.38, 87.0),
            ("2023Q2", 6_600.0, 0.50, 93.0),
            ("2023Q3", 6_600.0, 0.55, 91.0),
            ("2023Q4", 7_100.0, 0.54, 87.0),
            ("2024Q1", 6_000.0, 0.41, 88.0),
            ("2024Q2", 6_500.0, 0.45, 85.0),
            ("2024Q3", 6_400.0, 0.53, 91.0),
            ("2024Q4", 6_850.0, 0.71, 93.0),
            ("2025Q1", 5_250.0, 0.57, 91.0),
            ("2025Q2", 7_080.0, 0.67, 92.0),
            ("2025Q3", 6_390.0, 0.66, 92.0),
            ("2025Q4", 7_030.0, 0.65, 91.0),
            ("2026Q1", 5_910.0, 0.59, 91.0),
            ("2026Q2", 6_950.0, 0.64, 91.0),
        ];
        let mut targets = Vec::with_capacity(rows.len());
        let mut rec_pts = Vec::with_capacity(rows.len());
        let mut grd_pts = Vec::with_capacity(rows.len());
        let mut tp_pts = Vec::with_capacity(rows.len());
        for (q, tp, g, rec) in rows {
            targets.push(QuarterAgg {
                quarter: q.to_string(),
                throughput_kt: tp,
                head_grade_cu_pct: g,
                recovery_pct: rec,
                copper_kt: tp * (g / 100.0) * (rec / 100.0),
                segment_boundary_reset: false,
            });
            rec_pts.push(CitedPoint {
                quarter: q.to_string(),
                cell: format!("{q}:Minestats4!D14"),
                source_sha256: "synthetic-unit-test".to_string(),
                printed_value: rec,
                rounding_step: 1.0,
            });
            grd_pts.push(CitedPoint {
                quarter: q.to_string(),
                cell: format!("{q}:Minestats4!D13"),
                source_sha256: "synthetic-unit-test".to_string(),
                printed_value: g,
                rounding_step: 0.01,
            });
            tp_pts.push(CitedPoint {
                quarter: q.to_string(),
                cell: format!("{q}:Minestats4!D9"),
                source_sha256: "synthetic-unit-test".to_string(),
                printed_value: tp,
                rounding_step: 1.0,
            });
        }
        let series = vec![
            CitedSeries {
                series_name: "recovery".to_string(),
                unit: "pct".to_string(),
                points: rec_pts,
            },
            CitedSeries {
                series_name: "head_grade".to_string(),
                unit: "pct".to_string(),
                points: grd_pts,
            },
            CitedSeries {
                series_name: "throughput".to_string(),
                unit: "kt".to_string(),
                points: tp_pts,
            },
        ];
        (targets, series)
    }

    #[test]
    fn test_each_twin_reproduces_series_within_rounding() {
        let spec = load_lumwana_spec();
        let (targets, public) = synthetic_public_series();
        let family = TwinFamily::build_and_verify(&spec, 2_026_356, &targets, &public)
            .expect("all twins pass parity");
        assert_eq!(family.twins.len(), spec.k);
        assert_eq!(family.label, SYNTHETIC_PUBLIC_LABEL);
        family.check_parity(&public).expect("recheck parity");
    }

    #[test]
    fn test_family_varies_only_registered_axes() {
        let spec = load_lumwana_spec();
        let (targets, public) = synthetic_public_series();
        let family = TwinFamily::build_and_verify(&spec, 2_026_356, &targets, &public)
            .expect("build family");

        let mut distinct_axes = 0usize;
        for tw in &family.twins {
            assert!(spec.axis_ranges.contains(tw.axes));
            assert_eq!(tw.quarterly_aggregates().len(), targets.len());
            if tw.axes != family.twins[0].axes {
                distinct_axes += 1;
                assert_ne!(tw.daily_series(), family.twins[0].daily_series());
            }
            // All twins agree on quarterly aggregates within published rounding.
            for (a, b) in tw
                .quarterly_aggregates()
                .iter()
                .zip(family.twins[0].quarterly_aggregates().iter())
            {
                assert_eq!(a.quarter, b.quarter);
                assert_eq!(a.segment_boundary_reset, b.segment_boundary_reset);
                assert!((a.recovery_pct - b.recovery_pct).abs() < 0.25);
                assert!((a.throughput_kt - b.throughput_kt).abs() < 0.25);
                assert!((a.head_grade_cu_pct - b.head_grade_cu_pct).abs() < 1e-6);
            }
        }
        assert_eq!(distinct_axes, spec.k - 1);
    }

    #[test]
    fn test_segment_boundaries_respected() {
        let spec = load_lumwana_spec();
        let (targets, public) = synthetic_public_series();
        let family = TwinFamily::build_and_verify(&spec, 2_026_356, &targets, &public)
            .expect("build family");
        let tw = &family.twins[2];
        let aggs = tw.quarterly_aggregates();
        let boundary_set: BTreeSet<&str> = spec
            .segment_boundaries
            .iter()
            .map(|b| b.quarter.as_str())
            .collect();
        for q in &aggs {
            assert_eq!(
                q.segment_boundary_reset,
                boundary_set.contains(q.quarter.as_str()),
                "mismatch on quarter {}",
                q.quarter
            );
        }
        // Verify all five E580-E584 boundary quarters are flagged.
        for req_q in ["2024Q2", "2025Q1", "2025Q3", "2026Q1", "2026Q2"] {
            assert!(
                aggs.iter()
                    .any(|q| q.quarter == req_q && q.segment_boundary_reset),
                "missing boundary reset on {req_q}"
            );
        }
    }

    #[test]
    fn test_twin_disagreeing_with_aggregates_refused() {
        let spec = load_lumwana_spec();
        let (targets, public) = synthetic_public_series();
        let boundaries: BTreeSet<String> = spec
            .segment_boundaries
            .iter()
            .map(|b| b.quarter.clone())
            .collect();
        let seed = RngKey {
            seed_root: 99,
            partition: 0,
            domain: 356,
            var_group: 1,
            realization: 0,
            brick: 0,
        };
        // Inject a +2.0% recovery shift (exceeding ±0.5% rounding step) -> MUST refuse, never tune.
        let bad = PlantTwin::build(
            0,
            TwinAxes {
                sampler_ratio: 0.5,
                intra_quarter_autocorr: 0.3,
                fresh_trans_kinetics: 0.8,
            },
            seed,
            &targets,
            &boundaries,
            2.0,
        )
        .expect("build shifted twin");
        let err = bad.check_parity(&public).unwrap_err();
        assert_eq!(err.code, "FMINE-REF-101");

        // Mlb unit also refuses with FMINE-REF-105 (E565).
        let mut mlb_series = public;
        mlb_series[2].unit = "Mlb".to_string();
        let unit_err = bad.check_parity(&mlb_series).unwrap_err();
        assert_eq!(unit_err.code, "FMINE-REF-105");
    }

    #[test]
    fn test_twin_family_digest_deterministic() {
        let spec = load_lumwana_spec();
        let (targets, public) = synthetic_public_series();
        let f1 = TwinFamily::build_and_verify(&spec, 2_026_356, &targets, &public).expect("f1");
        let f2 = TwinFamily::build_and_verify(&spec, 2_026_356, &targets, &public).expect("f2");
        let f3 = TwinFamily::build_and_verify(&spec, 2_026_357, &targets, &public).expect("f3");
        assert_eq!(f1.family_digest, f2.family_digest);
        assert_ne!(f1.family_digest, f3.family_digest);
    }

    #[test]
    fn test_pilot_twins_registry_golden() {
        let spec = load_lumwana_spec();
        assert_eq!(spec.site, "lumwana");
        assert_eq!(spec.ownership_pct, 100);
        assert_eq!(spec.copper_unit, "kt");
        assert_eq!(spec.k, 5);
        assert_eq!(spec.status, "TARGET_OPEN_PLACEHOLDER");
        assert_eq!(spec.segment_boundaries.len(), 5);
        assert_eq!(spec.candidate_files.len(), 5);
    }

    #[test]
    #[ignore = "needs the registered Mine Stats workbooks under corpus_of_input_docs/"]
    fn test_lumwana_twins_parity_real_workbooks() {
        use fmine_io::xlsx::{CellValue, Workbook, parse_a1_ref};

        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|p| p.parent())
            .expect("workspace root")
            .to_path_buf();
        let dir = root.join("corpus_of_input_docs/downloaded/mine_stats");
        let spec = load_lumwana_spec();

        // Map of (quarter, workbook_filename, col_letter, expected E564 printed recovery %)
        let mapping: [(&str, &str, &str, f64); 14] = [
            ("2023Q1", "Barrick_Q1_2024_Mine_Stats.xlsx", "E", 87.0),
            ("2023Q2", "Barrick_Q2_2024_Mine_Stats.xlsx", "E", 93.0),
            ("2023Q3", "Barrick_Q3_2024_Mine_Stats.xlsx", "E", 91.0),
            ("2023Q4", "Barrick_Q4_2024_Mine_Stats.xlsx", "E", 87.0),
            ("2024Q1", "Barrick_Q1_2024_Mine_Stats.xlsx", "D", 88.0),
            ("2024Q2", "Barrick_Q2_2024_Mine_Stats.xlsx", "D", 85.0),
            ("2024Q3", "Barrick_Q3_2024_Mine_Stats.xlsx", "D", 91.0),
            ("2024Q4", "Barrick_Q4_2024_Mine_Stats.xlsx", "D", 93.0),
            ("2025Q1", "Barrick_Q1_2025_Mine_Stats.xlsx", "D", 91.0),
            ("2025Q2", "Barrick_Q2_2025_Mine_Stats.xlsx", "D", 92.0),
            ("2025Q3", "Barrick_Q3_2025_Mine_Stats.xlsx", "D", 92.0),
            ("2025Q4", "Barrick_Q4_2025_Mine_Stats.xlsx", "D", 91.0),
            ("2026Q1", "Barrick_Q1_2026_Mine_Stats.xlsx", "D", 91.0),
            ("2026Q2", "Barrick_Q2_2026_Mine_Stats.xlsx", "D", 91.0),
        ];

        let mut targets = Vec::new();
        let mut rec_pts = Vec::new();
        let mut grd_pts = Vec::new();
        let mut tp_pts = Vec::new();

        for (quarter, file, col, expected_rec_pct) in mapping {
            let path = dir.join(file);
            let wb = Workbook::from_file(&path).expect("read workbook");
            let sha_hex: String = wb.sha256.iter().map(|b| format!("{b:02x}")).collect();
            let sh = wb.sheet("Minestats4").expect("Minestats4 sheet");
            let read_num = |cell_ref: &str| -> f64 {
                let (r, c) = parse_a1_ref(cell_ref).expect("a1");
                match sh.cell(r, c).expect("cell").value {
                    CellValue::Number(v) => v,
                    ref other => panic!("expected numeric cell {cell_ref}, got {other:?}"),
                }
            };
            let c_tp = format!("{col}9");
            let c_grd = format!("{col}13");
            let c_rec = format!("{col}14");
            let tp_kt = read_num(&c_tp);
            let grd_pct = read_num(&c_grd) * 100.0;
            let rec_pct = read_num(&c_rec) * 100.0;
            assert!(
                (rec_pct - expected_rec_pct).abs() < 0.5,
                "{quarter} {file}!{c_rec}: got {rec_pct} vs E564 {expected_rec_pct}"
            );
            targets.push(QuarterAgg {
                quarter: quarter.to_string(),
                throughput_kt: tp_kt,
                head_grade_cu_pct: grd_pct,
                recovery_pct: rec_pct,
                copper_kt: tp_kt * (grd_pct / 100.0) * (rec_pct / 100.0),
                segment_boundary_reset: false,
            });
            rec_pts.push(CitedPoint {
                quarter: quarter.to_string(),
                cell: format!("{file}:Minestats4!{c_rec}"),
                source_sha256: sha_hex.clone(),
                printed_value: expected_rec_pct,
                rounding_step: 1.0,
            });
            grd_pts.push(CitedPoint {
                quarter: quarter.to_string(),
                cell: format!("{file}:Minestats4!{c_grd}"),
                source_sha256: sha_hex.clone(),
                printed_value: (grd_pct * 100.0).round() / 100.0,
                rounding_step: 0.01,
            });
            tp_pts.push(CitedPoint {
                quarter: quarter.to_string(),
                cell: format!("{file}:Minestats4!{c_tp}"),
                source_sha256: sha_hex,
                printed_value: tp_kt.round(),
                rounding_step: 1.0,
            });
        }

        let public = vec![
            CitedSeries {
                series_name: "recovery".to_string(),
                unit: "pct".to_string(),
                points: rec_pts,
            },
            CitedSeries {
                series_name: "head_grade".to_string(),
                unit: "pct".to_string(),
                points: grd_pts,
            },
            CitedSeries {
                series_name: "throughput".to_string(),
                unit: "kt".to_string(),
                points: tp_pts,
            },
        ];

        let family = TwinFamily::build_and_verify(&spec, 2_026_356, &targets, &public)
            .expect("all twins pass real workbook parity");
        println!(
            "{{\"event\":\"twin_parity_summary\",\"k\":{},\"cells\":{},\"failures\":0,\"family_digest\":\"{}\"}}",
            family.k,
            14 * 3 * family.k,
            family.family_digest.to_hex()
        );
    }
}
