//! Bilinear tonnes × grade metallurgical reconciliation over `fmine_flow::FlowGraph` (`BD-151`,
//! plan §20.2–§20.3, `G-TOUCH-1`).
//! Determinism: DeterministicPerIsa (delegates constrained Gauss–Newton SQP reconciliation to
//! `fmine_numerics::reconciliation::nonlinear::reconcile_nonlinear`).
//!
//! State coordinates are strictly **extensive** `[dry_mass, elemental_metal_mass]`, keeping every
//! mass and metal conservation equation `A x = 0` strictly linear (`G-TOUCH-1`). Raw weightometer
//! and assay grade observations enter through the exact nonlinear measurement operator
//! `h(M, m) = (M, m / M)` with analytic Jacobian and physical domain `M > 0`, `0 < m < M`.
//! Unmeasured stream dry masses (such as unweighed tailings in a concentrator) are eliminated
//! algebraically along dry-mass balance equations whenever uniquely observable, and refused with
//! `FMINE-REF-104` when unobservable.

use std::collections::{BTreeMap, BTreeSet};

use fmine_core::codec::Encoder;
use fmine_core::det;
use fmine_core::hash::blake3_hash;
use fmine_flow::graph::{FlowGraph, NodeClass};
use fmine_numerics::reconciliation::nonlinear::{NonlinearProblem, reconcile_nonlinear};

use crate::error::ReconError;
use crate::linear::PO_TOUCH_1_RELATIVE_TOLERANCE;

/// Typed elemental grade value converted explicitly to dimensionless mass fraction `(0, 1)`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum GradeValue {
    /// Dimensionless mass fraction (`0.01` = 1%, `1e-6` = 1 g/t = 1 ppm).
    MassFraction(f64),
    /// Weight percent (`%`, divided by `100.0` to obtain mass fraction).
    Percent(f64),
    /// Grams per dry metric tonne (`g/t` = ppm, divided by `1_000_000.0` to obtain mass fraction).
    GramsPerTonne(f64),
}

impl GradeValue {
    /// Converts this typed grade to dimensionless mass fraction.
    #[must_use]
    pub fn to_mass_fraction(self) -> f64 {
        match self {
            Self::MassFraction(x) => x,
            Self::Percent(pct) => pct * 1.0e-2,
            Self::GramsPerTonne(gt) => gt * 1.0e-6,
        }
    }
}

/// Measured positive extensive dry-mass observation with 1-sigma uncertainty.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ObservedScalar {
    /// Observed value (`> 0`).
    pub observed: f64,
    /// Measurement standard deviation (`> 0`).
    pub std_dev: f64,
}

/// Measured elemental assay grade observation with 1-sigma uncertainty.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ObservedGrade {
    /// Observed assay grade (converted to mass fraction in `(0, 1)`).
    pub grade: GradeValue,
    /// Assay 1-sigma uncertainty (converted to mass fraction `> 0`).
    pub std_dev: GradeValue,
}

/// Identifier of one raw observation inside a bilinear reconciliation problem (for specifying
/// off-diagonal measurement correlations).
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum BilinearObsRef {
    /// Dry-mass observation on `stream_id`.
    DryMass { stream_id: String },
    /// Elemental assay grade observation on `(stream_id, element)`.
    Grade { stream_id: String, element: String },
}

/// Symmetric off-diagonal correlation `rho in (-1, 1)` between two raw observations.
#[derive(Clone, Debug, PartialEq)]
pub struct BilinearObsCorrelation {
    /// First raw observation.
    pub a: BilinearObsRef,
    /// Second raw observation (distinct from `a`).
    pub b: BilinearObsRef,
    /// Correlation coefficient `rho in (-1, 1)`.
    pub correlation: f64,
}

/// Raw measurements attached to one stream of a metallurgical circuit.
#[derive(Clone, Debug, PartialEq)]
pub struct BilinearStreamObservation {
    /// Stream ID in [`FlowGraph`].
    pub stream_id: String,
    /// Optional dry-mass measurement (`None` if unweighed, e.g. tailings stream).
    pub dry_mass: Option<ObservedScalar>,
    /// Assay grade observations keyed by element symbol (e.g. `"Cu"`, `"Au"`).
    pub element_grades: BTreeMap<String, ObservedGrade>,
}

/// Request to run bilinear tonnes × grade metallurgical reconciliation (`BD-151`).
#[derive(Clone, Debug, PartialEq)]
pub struct BilinearFlowRequest<'a> {
    /// Validated steady-state plant material-flow graph (interior stockpile nodes are refused under `G-TOUCH-3`).
    pub graph: &'a FlowGraph,
    /// Element symbols reconciled jointly with dry mass (e.g. `["Cu", "Au"]`).
    pub elements: Vec<String>,
    /// Per-stream raw mass and assay observations.
    pub stream_observations: Vec<BilinearStreamObservation>,
    /// Optional off-diagonal correlations among raw observations (identity correlation when empty).
    pub observation_correlations: Vec<BilinearObsCorrelation>,
    /// Maximum Gauss–Newton SQP iterations (`1..=128`, default `64`).
    pub max_iterations: usize,
    /// Relative closure tolerance on every mass & metal balance (default `1e-9`).
    pub relative_closure_tolerance: f64,
}

/// Post-reconciliation derived grade for one `(stream, element)` (`G-TOUCH-1`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ReconciledGrade {
    /// Reconciled contained elemental metal mass `m_hat` (same mass unit as `dry_mass`).
    pub metal_mass: f64,
    /// Local standard deviation of `m_hat`.
    pub metal_mass_sd: f64,
    /// Derived reconciled grade `g_hat = m_hat / M_hat` as dimensionless mass fraction.
    pub grade_mass_fraction: f64,
    /// Derived reconciled grade in `%` (`100 * g_hat`).
    pub grade_pct: f64,
    /// Derived reconciled grade in `g/t` (`1e6 * g_hat`).
    pub grade_g_per_t: f64,
    /// First-order delta-method local standard deviation of `g_hat` (mass fraction).
    pub grade_mass_fraction_sd: f64,
}

/// Reconciled extensive and derived intensive state of one stream.
#[derive(Clone, Debug, PartialEq)]
pub struct ReconciledStreamState {
    /// Stream ID.
    pub stream_id: String,
    /// Whether this stream's dry mass was directly measured (`false` if inferred from conservation).
    pub dry_mass_was_measured: bool,
    /// Reconciled dry solid mass `M_hat`.
    pub dry_mass: f64,
    /// Local standard deviation of `M_hat`.
    pub dry_mass_sd: f64,
    /// Reconciled elemental metal masses and post-reconciliation derived grades by element.
    pub elements: BTreeMap<String, ReconciledGrade>,
}

/// Reconciled mass pull and elemental recovery across an interior plant node (`outgoing / total_in`).
#[derive(Clone, Debug, PartialEq)]
pub struct NodeElementRecovery {
    /// Interior plant node ID.
    pub node_id: String,
    /// Outgoing stream ID (e.g. concentrate stream).
    pub stream_id: String,
    /// Reconciled dry mass pull `M_out / sum(M_in)`.
    pub mass_pull: f64,
    /// Reconciled elemental metal recoveries `m_{out,e} / sum(m_{in,e})` keyed by element.
    pub element_recoveries: BTreeMap<String, f64>,
}

/// Complete result of bilinear tonnes × grade metallurgical reconciliation (`BD-151`).
#[derive(Clone, Debug, PartialEq)]
pub struct BilinearReconResult {
    /// Reconciled per-stream extensive dry masses, metal masses, and derived grades.
    pub streams: BTreeMap<String, ReconciledStreamState>,
    /// Per-node outgoing mass pulls and elemental recoveries.
    pub recoveries: Vec<NodeElementRecovery>,
    /// Full canonical state labels (all stream dry masses followed by `(stream, element)` metal masses).
    pub state_labels: Vec<String>,
    /// Full reconciled extensive state vector matching `state_labels`.
    pub full_estimates: Vec<f64>,
    /// Full row-major local tangent covariance matching `state_labels`.
    pub full_local_covariance: Vec<f64>,
    /// Squared Mahalanobis measurement objective `r(x)^T R^-1 r(x)`.
    pub objective: f64,
    /// Number of independent balance equations reconciled (`dry_mass + elemental_metal` across interior nodes).
    pub total_balance_equations: usize,
    /// Net statistical redundancy (`raw_observations + total_balance_equations - total_extensive_states`).
    pub degrees_of_freedom: usize,
    /// Maximum relative closure error across all interior mass and metal balances (`<= tol`).
    pub max_relative_closure_error: f64,
    /// Gauss–Newton SQP iterations taken to converge.
    pub iterations: usize,
    /// Line-search backtracks taken.
    pub backtracks: usize,
    /// Replay-stable BLAKE3 hex digest (`PO-TOUCH-6`).
    pub digest_hex: String,
}

/// Solves joint bilinear tonnes × grade metallurgical reconciliation over `request.graph`
/// (`BD-151`, `G-TOUCH-1`, wired to `fmine_numerics::reconciliation::nonlinear::reconcile_nonlinear`).
pub fn reconcile_bilinear_flow(
    request: &BilinearFlowRequest<'_>,
) -> Result<BilinearReconResult, ReconError> {
    if request.elements.is_empty() {
        return Err(ReconError::invalid(
            "bilinear.elements",
            "at least one element must be specified",
        ));
    }
    if !(1..=128).contains(&request.max_iterations) {
        return Err(ReconError::invalid(
            "bilinear.max_iterations",
            format!("must lie in 1..=128, got {}", request.max_iterations),
        ));
    }
    if !(request.relative_closure_tolerance.is_finite()
        && request.relative_closure_tolerance > 0.0
        && request.relative_closure_tolerance <= PO_TOUCH_1_RELATIVE_TOLERANCE)
    {
        return Err(ReconError::invalid(
            "bilinear.relative_closure_tolerance",
            format!(
                "must lie in (0, {:.1e}] (PO-TOUCH-1), got {}",
                PO_TOUCH_1_RELATIVE_TOLERANCE, request.relative_closure_tolerance
            ),
        ));
    }

    let mut elements_set = BTreeSet::new();
    for el in &request.elements {
        if el.trim().is_empty() || !elements_set.insert(el.clone()) {
            return Err(ReconError::invalid(
                "bilinear.elements",
                format!("empty or duplicate element '{el}'"),
            ));
        }
    }
    let elements: Vec<String> = elements_set.into_iter().collect();

    let interior_nodes = request.graph.interior_nodes();
    if interior_nodes.is_empty() {
        return Err(ReconError::InsufficientRedundancy {
            context: "bilinear.flow_graph".to_string(),
            detail: "flow graph has no interior conservation nodes".to_string(),
        });
    }
    for node in &interior_nodes {
        if node.class == NodeClass::StockpileOrPad || node.holds_inventory {
            return Err(ReconError::StockpileDecouplingRefused {
                detail: format!(
                    "interior node '{}' holds inventory; steady-state bilinear balance refuses inventory-buffering nodes under G-TOUCH-3",
                    node.id
                ),
            });
        }
    }

    // Collect participating streams (all streams incident to at least one interior node).
    let mut participating_set = BTreeSet::new();
    for node in &interior_nodes {
        for s in request.graph.incoming_streams(&node.id) {
            participating_set.insert(s.id.clone());
        }
        for s in request.graph.outgoing_streams(&node.id) {
            participating_set.insert(s.id.clone());
        }
    }
    let stream_ids: Vec<String> = participating_set.into_iter().collect();
    let n_streams = stream_ids.len();
    let stream_idx: BTreeMap<&str, usize> = stream_ids
        .iter()
        .enumerate()
        .map(|(i, s)| (s.as_str(), i))
        .collect();

    // Index and validate supplied per-stream observations.
    let mut obs_by_stream: BTreeMap<&str, &BilinearStreamObservation> = BTreeMap::new();
    for obs in &request.stream_observations {
        if request.graph.stream(&obs.stream_id).is_none() {
            return Err(ReconError::invalid(
                "bilinear.stream_observations",
                format!("unknown stream '{}'", obs.stream_id),
            ));
        }
        if obs_by_stream.insert(obs.stream_id.as_str(), obs).is_some() {
            return Err(ReconError::invalid(
                "bilinear.stream_observations",
                format!("duplicate observation for stream '{}'", obs.stream_id),
            ));
        }
    }

    let mut measured_mass_streams: Vec<usize> = Vec::new();
    let mut mass_obs_vals: Vec<(f64, f64)> = Vec::new();
    let mut stream_to_meas_mass_slot: Vec<Option<usize>> = vec![None; n_streams];

    // Metal variable table: (stream_index, element_name, grade_obs_mf, grade_sd_mf).
    let mut metal_vars: Vec<(usize, String, f64, f64)> = Vec::new();

    for (s_i, sid) in stream_ids.iter().enumerate() {
        let Some(obs) = obs_by_stream.get(sid.as_str()) else {
            return Err(ReconError::invalid(
                "bilinear.stream_observations",
                format!("missing BilinearStreamObservation for participating stream '{sid}'"),
            ));
        };
        if let Some(dm) = obs.dry_mass {
            if !(dm.observed.is_finite()
                && dm.observed > 0.0
                && dm.std_dev.is_finite()
                && dm.std_dev > 0.0)
            {
                return Err(ReconError::invalid(
                    "bilinear.dry_mass",
                    format!("stream '{sid}' requires finite dry_mass > 0 and std_dev > 0"),
                ));
            }
            let slot = measured_mass_streams.len();
            measured_mass_streams.push(s_i);
            mass_obs_vals.push((dm.observed, dm.std_dev));
            stream_to_meas_mass_slot[s_i] = Some(slot);
        }
    }

    if measured_mass_streams.is_empty() {
        return Err(ReconError::NonIdentifiable {
            detail: "at least one stream must have a measured dry mass to pin circuit scale"
                .to_string(),
        });
    }

    let mut stream_grade_sum = vec![0.0_f64; n_streams];
    for el in &elements {
        let mut carried_count = 0usize;
        for (s_i, sid) in stream_ids.iter().enumerate() {
            let Some(stream) = request.graph.stream(sid) else {
                continue;
            };
            if !stream.elements.contains(el) {
                continue;
            }
            carried_count += 1;
            let Some(obs) = obs_by_stream.get(sid.as_str()) else {
                continue;
            };
            let Some(og) = obs.element_grades.get(el) else {
                return Err(ReconError::invalid(
                    "bilinear.element_grades",
                    format!("missing '{el}' assay grade observation on stream '{sid}'"),
                ));
            };
            let g = og.grade.to_mass_fraction();
            let sd = og.std_dev.to_mass_fraction();
            if !(g.is_finite() && g > 0.0 && g < 1.0 && sd.is_finite() && sd > 0.0) {
                return Err(ReconError::invalid(
                    "bilinear.element_grades",
                    format!(
                        "stream '{sid}' element '{el}': grade mass fraction must lie in (0, 1) and std_dev > 0, got g={g} sd={sd}"
                    ),
                ));
            }
            stream_grade_sum[s_i] += g;
            if stream_grade_sum[s_i] >= 1.0 {
                return Err(ReconError::invalid(
                    "bilinear.element_grades",
                    format!(
                        "stream '{sid}': sum of elemental mass fractions ({}) must be < 1.0",
                        stream_grade_sum[s_i]
                    ),
                ));
            }
            metal_vars.push((s_i, el.clone(), g, sd));
        }
        if carried_count == 0 {
            return Err(ReconError::invalid(
                "bilinear.elements",
                format!("requested element '{el}' is not declared on any participating stream"),
            ));
        }
    }

    let n_meas_mass = measured_mass_streams.len();
    let n_metal = metal_vars.len();

    // Algebraically eliminate any unmeasured stream dry masses using interior dry-mass balances:
    // M_s = sum_{k=0..n_meas_mass} mass_expr[s][k] * M_{meas,k}.
    let mut mass_expr: Vec<Vec<f64>> = vec![vec![0.0_f64; n_meas_mass]; n_streams];
    let mut resolved = vec![false; n_streams];
    for (slot, &s_i) in measured_mass_streams.iter().enumerate() {
        mass_expr[s_i][slot] = 1.0;
        resolved[s_i] = true;
    }

    let mut used_mass_node = vec![false; interior_nodes.len()];
    loop {
        let mut progress = false;
        for (node_idx, node) in interior_nodes.iter().enumerate() {
            if used_mass_node[node_idx] {
                continue;
            }
            let mut incident: Vec<(usize, f64)> = Vec::new();
            for s in request.graph.incoming_streams(&node.id) {
                if let Some(&idx) = stream_idx.get(s.id.as_str()) {
                    incident.push((idx, 1.0));
                }
            }
            for s in request.graph.outgoing_streams(&node.id) {
                if let Some(&idx) = stream_idx.get(s.id.as_str()) {
                    incident.push((idx, -1.0));
                }
            }
            let unresolved: Vec<(usize, f64)> = incident
                .iter()
                .copied()
                .filter(|&(idx, _)| !resolved[idx])
                .collect();
            if unresolved.len() == 1 {
                let (u_idx, sgn_u) = unresolved[0];
                let mut expr = vec![0.0_f64; n_meas_mass];
                for &(other_idx, sgn_o) in &incident {
                    if other_idx == u_idx {
                        continue;
                    }
                    let coeff = -sgn_u * sgn_o;
                    for k in 0..n_meas_mass {
                        expr[k] += coeff * mass_expr[other_idx][k];
                    }
                }
                mass_expr[u_idx] = expr;
                resolved[u_idx] = true;
                used_mass_node[node_idx] = true;
                progress = true;
            }
        }
        if !progress {
            break;
        }
    }

    for (s_i, &is_res) in resolved.iter().enumerate() {
        if !is_res {
            return Err(ReconError::NonIdentifiable {
                detail: format!(
                    "unmeasured dry mass on stream '{}' is not uniquely observable from interior dry-mass balances",
                    stream_ids[s_i]
                ),
            });
        }
    }

    // Reduced extensive state: [M_meas (0..n_meas_mass), m_{s,e} (n_meas_mass..n_red)].
    let n_red = n_meas_mass + n_metal;
    let p_obs = n_red;

    // Initial state from raw mass observations and M_s_init * y_{g,s,e}.
    let mut initial = vec![0.0_f64; n_red];
    for (slot, &(obs_m, _)) in mass_obs_vals.iter().enumerate() {
        initial[slot] = obs_m;
    }
    let mut init_stream_mass = vec![0.0_f64; n_streams];
    for s_i in 0..n_streams {
        let mut m_s = 0.0_f64;
        for k in 0..n_meas_mass {
            m_s += mass_expr[s_i][k] * initial[k];
        }
        if !(m_s.is_finite() && m_s > 0.0) {
            return Err(ReconError::invalid(
                "bilinear.initial_state",
                format!(
                    "stream '{}' implied initial dry mass {m_s} is not strictly positive; check measured stream masses",
                    stream_ids[s_i]
                ),
            ));
        }
        init_stream_mass[s_i] = m_s;
    }
    for (m_idx, &(s_i, _, g_mf, _)) in metal_vars.iter().enumerate() {
        initial[n_meas_mass + m_idx] = init_stream_mass[s_i] * g_mf;
    }

    // Assemble remaining linear conservation rows A_red x_red = 0.
    let metal_slot: BTreeMap<(usize, &str), usize> = metal_vars
        .iter()
        .enumerate()
        .map(|(m_idx, (s_i, el, _, _))| ((*s_i, el.as_str()), n_meas_mass + m_idx))
        .collect();

    let mut constraint_rows: Vec<Vec<f64>> = Vec::new();
    let mut tolerances: Vec<f64> = Vec::new();
    let mut total_balance_equations = used_mass_node.iter().filter(|&&u| u).count();

    for (node_idx, node) in interior_nodes.iter().enumerate() {
        if !used_mass_node[node_idx] {
            let mut row = vec![0.0_f64; n_red];
            let mut row_scale = 0.0_f64;
            for s in request.graph.incoming_streams(&node.id) {
                if let Some(&s_i) = stream_idx.get(s.id.as_str()) {
                    for k in 0..n_meas_mass {
                        row[k] += mass_expr[s_i][k];
                    }
                    row_scale += init_stream_mass[s_i].abs();
                }
            }
            for s in request.graph.outgoing_streams(&node.id) {
                if let Some(&s_i) = stream_idx.get(s.id.as_str()) {
                    for k in 0..n_meas_mass {
                        row[k] -= mass_expr[s_i][k];
                    }
                    row_scale += init_stream_mass[s_i].abs();
                }
            }
            if row.iter().any(|&v| v.abs() > 1e-14) {
                // Scale each linear balance row by its initial physical magnitude so dry mass,
                // base metals (% Cu), and trace metals (g/t Au) have O(1) residuals and O(1)
                // Lagrange multipliers inside the SQP l1 merit function.
                let inv_scale = 1.0 / row_scale.max(1e-30);
                for v in &mut row {
                    *v *= inv_scale;
                }
                constraint_rows.push(row);
                tolerances.push((0.5 * request.relative_closure_tolerance).max(1e-14));
                total_balance_equations += 1;
            }
        }
        for el in &elements {
            let mut row = vec![0.0_f64; n_red];
            let mut row_scale = 0.0_f64;
            let mut any = false;
            for s in request.graph.incoming_streams(&node.id) {
                if let Some(&s_i) = stream_idx.get(s.id.as_str())
                    && let Some(&slot) = metal_slot.get(&(s_i, el.as_str()))
                {
                    row[slot] += 1.0;
                    row_scale += initial[slot].abs();
                    any = true;
                }
            }
            for s in request.graph.outgoing_streams(&node.id) {
                if let Some(&s_i) = stream_idx.get(s.id.as_str())
                    && let Some(&slot) = metal_slot.get(&(s_i, el.as_str()))
                {
                    row[slot] -= 1.0;
                    row_scale += initial[slot].abs();
                    any = true;
                }
            }
            if any {
                let inv_scale = 1.0 / row_scale.max(1e-30);
                for v in &mut row {
                    *v *= inv_scale;
                }
                constraint_rows.push(row);
                tolerances.push((0.5 * request.relative_closure_tolerance).max(1e-14));
                total_balance_equations += 1;
            }
        }
    }

    let m_red = constraint_rows.len();
    if m_red == 0 {
        return Err(ReconError::InsufficientRedundancy {
            context: "bilinear.constraints".to_string(),
            detail: "no redundant conservation equations remain after mass elimination".to_string(),
        });
    }
    let mut constraints_flat = Vec::with_capacity(m_red * n_red);
    for row in &constraint_rows {
        constraints_flat.extend_from_slice(row);
    }
    let targets = vec![0.0_f64; m_red];

    // Build raw observation correlation matrix R (p_obs x p_obs, unit diagonal).
    let mut obs_index: BTreeMap<BilinearObsRef, usize> = BTreeMap::new();
    for (slot, &s_i) in measured_mass_streams.iter().enumerate() {
        obs_index.insert(
            BilinearObsRef::DryMass {
                stream_id: stream_ids[s_i].clone(),
            },
            slot,
        );
    }
    for (m_idx, (s_i, el, _, _)) in metal_vars.iter().enumerate() {
        obs_index.insert(
            BilinearObsRef::Grade {
                stream_id: stream_ids[*s_i].clone(),
                element: el.clone(),
            },
            n_meas_mass + m_idx,
        );
    }

    let mut correlation = vec![0.0_f64; p_obs * p_obs];
    for i in 0..p_obs {
        correlation[i * p_obs + i] = 1.0;
    }
    let mut seen_corr = BTreeSet::new();
    for corr in &request.observation_correlations {
        if !(corr.correlation.is_finite() && corr.correlation > -1.0 && corr.correlation < 1.0) {
            return Err(ReconError::invalid(
                "bilinear.observation_correlations",
                format!("correlation {} must lie in (-1, 1)", corr.correlation),
            ));
        }
        let Some(&ia) = obs_index.get(&corr.a) else {
            return Err(ReconError::invalid(
                "bilinear.observation_correlations",
                format!("unknown observation reference {:?}", corr.a),
            ));
        };
        let Some(&ib) = obs_index.get(&corr.b) else {
            return Err(ReconError::invalid(
                "bilinear.observation_correlations",
                format!("unknown observation reference {:?}", corr.b),
            ));
        };
        if ia == ib {
            return Err(ReconError::invalid(
                "bilinear.observation_correlations",
                "self-correlation entry is not allowed",
            ));
        }
        let pair = (ia.min(ib), ia.max(ib));
        if !seen_corr.insert(pair) {
            return Err(ReconError::invalid(
                "bilinear.observation_correlations",
                "duplicate observation correlation entry",
            ));
        }
        correlation[ia * p_obs + ib] = corr.correlation;
        correlation[ib * p_obs + ia] = corr.correlation;
    }

    // Solve constrained nonlinear WLS via fmine-numerics::reconciliation::nonlinear.
    let nl = reconcile_nonlinear(
        NonlinearProblem {
            initial: &initial,
            correlation: &correlation,
            observation_count: p_obs,
            constraints: &constraints_flat,
            targets: &targets,
            tolerances: &tolerances,
            max_iterations: request.max_iterations,
        },
        |x, r_out, j_out| {
            j_out.fill(0.0);
            // Dry-mass measurement residuals & Jacobian.
            for (slot, &(obs_m, sd_m)) in mass_obs_vals.iter().enumerate() {
                r_out[slot] = (x[slot] - obs_m) / sd_m;
                j_out[slot * n_red + slot] = 1.0 / sd_m;
            }
            // Precompute all stream dry masses M_s(x).
            let mut m_s_vec = vec![0.0_f64; n_streams];
            for s_i in 0..n_streams {
                let mut m_s = 0.0_f64;
                for k in 0..n_meas_mass {
                    m_s += mass_expr[s_i][k] * x[k];
                }
                m_s_vec[s_i] = m_s;
            }
            // Assay grade measurement residuals h(x) = m_{s,e} / M_s and exact analytic Jacobian.
            for (m_idx, &(s_i, _, obs_g, sd_g)) in metal_vars.iter().enumerate() {
                let row = n_meas_mass + m_idx;
                let m_s = m_s_vec[s_i];
                let metal = x[row];
                let g_hat = metal / m_s;
                r_out[row] = (g_hat - obs_g) / sd_g;
                let dg_dm_stream = -(metal / (m_s * m_s)) / sd_g;
                for k in 0..n_meas_mass {
                    let c = mass_expr[s_i][k];
                    if c != 0.0 {
                        j_out[row * n_red + k] = dg_dm_stream * c;
                    }
                }
                j_out[row * n_red + row] = (1.0 / m_s) / sd_g;
            }
            Ok(())
        },
        |x| {
            let mut m_s_vec = vec![0.0_f64; n_streams];
            for s_i in 0..n_streams {
                let mut m_s = 0.0_f64;
                for k in 0..n_meas_mass {
                    m_s += mass_expr[s_i][k] * x[k];
                }
                if !(m_s.is_finite() && m_s > 0.0) {
                    return false;
                }
                m_s_vec[s_i] = m_s;
            }
            let mut metal_sum = vec![0.0_f64; n_streams];
            for (m_idx, &(s_i, _, _, _)) in metal_vars.iter().enumerate() {
                let metal = x[n_meas_mass + m_idx];
                if !(metal.is_finite() && metal > 0.0 && metal < m_s_vec[s_i]) {
                    return false;
                }
                metal_sum[s_i] += metal;
                if metal_sum[s_i] >= m_s_vec[s_i] {
                    return false;
                }
            }
            true
        },
    )?;

    // Lift reduced state and local covariance back to the full extensive state:
    // full = [M_0 .. M_{S-1}, m_0 .. m_{n_metal-1}] = L * x_red.
    let n_full = n_streams + n_metal;
    let mut lift = vec![0.0_f64; n_full * n_red];
    for s_i in 0..n_streams {
        for k in 0..n_meas_mass {
            lift[s_i * n_red + k] = mass_expr[s_i][k];
        }
    }
    for m_idx in 0..n_metal {
        lift[(n_streams + m_idx) * n_red + (n_meas_mass + m_idx)] = 1.0;
    }

    let mut full_estimates = vec![0.0_f64; n_full];
    for i in 0..n_full {
        let mut sum = 0.0_f64;
        for k in 0..n_red {
            sum += lift[i * n_red + k] * nl.estimates[k];
        }
        full_estimates[i] = sum;
    }

    // Propagate constrained local covariance: cov_full = L * cov_red * L^T.
    let mut tmp = vec![0.0_f64; n_full * n_red];
    for i in 0..n_full {
        for j in 0..n_red {
            let mut sum = 0.0_f64;
            for k in 0..n_red {
                sum += lift[i * n_red + k] * nl.local_covariance[k * n_red + j];
            }
            tmp[i * n_red + j] = sum;
        }
    }
    let mut full_local_covariance = vec![0.0_f64; n_full * n_full];
    for i in 0..n_full {
        for j in i..n_full {
            let mut sum = 0.0_f64;
            for k in 0..n_red {
                sum += tmp[i * n_red + k] * lift[j * n_red + k];
            }
            full_local_covariance[i * n_full + j] = sum;
            full_local_covariance[j * n_full + i] = sum;
        }
    }

    // Audit exact closure across ALL interior nodes on dry mass and every element (PO-TOUCH-1).
    let full_metal_slot: BTreeMap<(usize, &str), usize> = metal_vars
        .iter()
        .enumerate()
        .map(|(m_idx, (s_i, el, _, _))| ((*s_i, el.as_str()), n_streams + m_idx))
        .collect();

    let mut max_rel_err = 0.0_f64;
    for node in &interior_nodes {
        let mut in_m = 0.0_f64;
        let mut out_m = 0.0_f64;
        for s in request.graph.incoming_streams(&node.id) {
            if let Some(&s_i) = stream_idx.get(s.id.as_str()) {
                in_m += full_estimates[s_i];
            }
        }
        for s in request.graph.outgoing_streams(&node.id) {
            if let Some(&s_i) = stream_idx.get(s.id.as_str()) {
                out_m += full_estimates[s_i];
            }
        }
        let denom_m = in_m.abs() + out_m.abs();
        let rel_m = if denom_m > 0.0 {
            (in_m - out_m).abs() / denom_m
        } else {
            (in_m - out_m).abs()
        };
        max_rel_err = max_rel_err.max(rel_m);

        for el in &elements {
            let mut in_e = 0.0_f64;
            let mut out_e = 0.0_f64;
            let mut any = false;
            for s in request.graph.incoming_streams(&node.id) {
                if let Some(&s_i) = stream_idx.get(s.id.as_str())
                    && let Some(&slot) = full_metal_slot.get(&(s_i, el.as_str()))
                {
                    in_e += full_estimates[slot];
                    any = true;
                }
            }
            for s in request.graph.outgoing_streams(&node.id) {
                if let Some(&s_i) = stream_idx.get(s.id.as_str())
                    && let Some(&slot) = full_metal_slot.get(&(s_i, el.as_str()))
                {
                    out_e += full_estimates[slot];
                    any = true;
                }
            }
            if any {
                let denom_e = in_e.abs() + out_e.abs();
                let rel_e = if denom_e > 0.0 {
                    (in_e - out_e).abs() / denom_e
                } else {
                    (in_e - out_e).abs()
                };
                max_rel_err = max_rel_err.max(rel_e);
            }
        }
    }
    if !max_rel_err.is_finite() || max_rel_err > request.relative_closure_tolerance {
        return Err(ReconError::NumericalFailure {
            detail: format!(
                "bilinear conservation audit failed: max relative closure error {max_rel_err:.3e} > {:.1e}",
                request.relative_closure_tolerance
            ),
        });
    }

    // Build per-stream outputs with post-reconciliation derived grades and delta-method SDs.
    let mut streams = BTreeMap::new();
    for (s_i, sid) in stream_ids.iter().enumerate() {
        let dry_mass = full_estimates[s_i];
        let dry_mass_var = full_local_covariance[s_i * n_full + s_i].max(0.0);
        let dry_mass_sd = det::sqrt(dry_mass_var);
        let mut elem_map = BTreeMap::new();
        for el in &elements {
            if let Some(&slot) = full_metal_slot.get(&(s_i, el.as_str())) {
                let metal_mass = full_estimates[slot];
                let metal_var = full_local_covariance[slot * n_full + slot].max(0.0);
                let cov_m_metal = full_local_covariance[s_i * n_full + slot];
                let g_mf = metal_mass / dry_mass;
                // First-order delta-method variance for g = m / M:
                // J = [-m / M^2, 1 / M].
                let j_m = -metal_mass / (dry_mass * dry_mass);
                let j_e = 1.0 / dry_mass;
                let g_var = (j_m * j_m * dry_mass_var
                    + 2.0 * j_m * j_e * cov_m_metal
                    + j_e * j_e * metal_var)
                    .max(0.0);
                elem_map.insert(
                    el.clone(),
                    ReconciledGrade {
                        metal_mass,
                        metal_mass_sd: det::sqrt(metal_var),
                        grade_mass_fraction: g_mf,
                        grade_pct: g_mf * 100.0,
                        grade_g_per_t: g_mf * 1.0e6,
                        grade_mass_fraction_sd: det::sqrt(g_var),
                    },
                );
            }
        }
        streams.insert(
            sid.clone(),
            ReconciledStreamState {
                stream_id: sid.clone(),
                dry_mass_was_measured: stream_to_meas_mass_slot[s_i].is_some(),
                dry_mass,
                dry_mass_sd,
                elements: elem_map,
            },
        );
    }

    // Compute per-node outgoing mass pulls and elemental recoveries.
    let mut recoveries = Vec::new();
    for node in &interior_nodes {
        let total_in_mass: f64 = request
            .graph
            .incoming_streams(&node.id)
            .iter()
            .filter_map(|s| stream_idx.get(s.id.as_str()).map(|&i| full_estimates[i]))
            .sum();
        let mut total_in_elem: BTreeMap<&str, f64> = BTreeMap::new();
        for el in &elements {
            let sum_e: f64 = request
                .graph
                .incoming_streams(&node.id)
                .iter()
                .filter_map(|s| {
                    stream_idx
                        .get(s.id.as_str())
                        .and_then(|&s_i| full_metal_slot.get(&(s_i, el.as_str())))
                        .map(|&slot| full_estimates[slot])
                })
                .sum();
            total_in_elem.insert(el.as_str(), sum_e);
        }
        for s in request.graph.outgoing_streams(&node.id) {
            let Some(&s_i) = stream_idx.get(s.id.as_str()) else {
                continue;
            };
            let mass_pull = if total_in_mass > 0.0 {
                full_estimates[s_i] / total_in_mass
            } else {
                0.0
            };
            let mut elem_recs = BTreeMap::new();
            for el in &elements {
                if let Some(&slot) = full_metal_slot.get(&(s_i, el.as_str()))
                    && let Some(&in_e) = total_in_elem.get(el.as_str())
                    && in_e > 0.0
                {
                    elem_recs.insert(el.clone(), full_estimates[slot] / in_e);
                }
            }
            recoveries.push(NodeElementRecovery {
                node_id: node.id.clone(),
                stream_id: s.id.clone(),
                mass_pull,
                element_recoveries: elem_recs,
            });
        }
    }

    let mut state_labels = Vec::with_capacity(n_full);
    for sid in &stream_ids {
        state_labels.push(format!("stream:{sid}:dry_mass"));
    }
    for (s_i, el, _, _) in &metal_vars {
        state_labels.push(format!("stream:{}:element:{el}", stream_ids[*s_i]));
    }

    let mut enc = Encoder::new_frame(*b"RBIL", 1);
    enc.put_u32(state_labels.len() as u32);
    for lbl in &state_labels {
        enc.put_str(lbl);
    }
    for &v in &full_estimates {
        enc.put_u64(v.to_bits());
    }
    for &c in &full_local_covariance {
        enc.put_u64(c.to_bits());
    }
    enc.put_u64(nl.objective.to_bits());
    let digest_hex = blake3_hash(&enc.finish()).to_hex();

    let degrees_of_freedom = (p_obs + total_balance_equations).saturating_sub(n_full);

    Ok(BilinearReconResult {
        streams,
        recoveries,
        state_labels,
        full_estimates,
        full_local_covariance,
        objective: nl.objective,
        total_balance_equations,
        degrees_of_freedom,
        max_relative_closure_error: max_rel_err,
        iterations: nl.iterations,
        backtracks: nl.backtracks,
        digest_hex,
    })
}
