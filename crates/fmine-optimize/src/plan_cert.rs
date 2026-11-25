//! Determinism: BitwisePortable (`i64` / `i128` integer arithmetic and SHA-256 only; no floating point or threads).
//!
//! Three-gap `PlanCertificate` builders for closure pits, whole-block and split schedules
//! (`BD-253`, `GOAL-010`, `NS-16`, plan §15.2).
//!
//! Every constructor independently re-verifies the primal feasibility, objective value, and dual/closure
//! upper bound before sealing an `FMT-PCERT-1` (`fmine_core::plan_cert::PlanCertificate`) frame suitable
//! for offline verification by `fmine-verify plancert`.

use fmine_core::cert::InputDigest;
use fmine_core::codec::Encoder;
use fmine_core::hash::{Digest32, sha256};
use fmine_core::plan_cert::{
    BoundKind, CertifiedBound, PlanCalibrationStatus, PlanCertError, PlanCertificate,
    PlanCertificateRequest, PlanHorizon, SaaGap, VssCertificate,
};
use fmine_evidence::{Refusal, RefusalCode};

use crate::pitflow::{self, ClosureProblem, PitSolution};
use crate::toposort::cpit::RATE_SCALE;
use crate::toposort::pcpsp::{self, PcpspModel};
use crate::toposort::search::{DISCOUNT_SCALE, discount_factors, discounted_value};
use crate::toposort::{ScheduleProblem, check, lp};

/// Caller-supplied plan metadata, sampling/VSS gaps, and calibration status for [`PlanCertificate`] sealing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlanCertMetadata {
    pub plan_id: String,
    pub horizon: PlanHorizon,
    pub code_revision: String,
    pub opt_set_id: String,
    pub currency_code: String,
    /// Scale denominator converting scaled integer objective units to `currency_code` (e.g. `1`, `100`).
    /// For [`certify_pcpsp_plan`], supply the parser's base value scale (10^value_decimals).
    /// The builder multiplies it by the schedule share and discount scales.
    pub value_scale_denominator: u128,
    pub saa: Option<SaaGap>,
    pub vss: Option<VssCertificate>,
    pub calibration: PlanCalibrationStatus,
    pub inputs: Vec<InputDigest>,
    pub extra_refusal_codes: Vec<String>,
}

/// Recheck a whole-block schedule and full-model Lagrangian bound before sealing it.
/// `meta.value_scale_denominator` is the base value scale before discounting.
///
/// # Errors
/// Refuses inconsistent discount factors, an infeasible or misvalued schedule, unreplayable
/// prices or a mismatched bound, and invalid certificate metadata.
pub fn certify_whole_block_schedule(
    meta: &PlanCertMetadata,
    problem: &ScheduleProblem,
    discount: [i64; 2],
    factors: &[i64],
    schedule: &[Option<u32>],
    primal: i128,
    bound: &lp::Lagrangian,
) -> Result<PlanCertificate, Refusal> {
    let failed = |detail| Refusal::new(RefusalCode::NumericalCertificateFailed207, detail);
    if factors != discount_factors(discount[0], discount[1], problem.periods())? {
        return Err(failed(
            "plan_cert: discount factors disagree with the declared rate",
        ));
    }
    if !check(problem, schedule)?.feasible()
        || discounted_value(problem, schedule, factors)? != primal
    {
        return Err(failed(
            "plan_cert: whole-block schedule failed feasibility or value replay",
        ));
    }
    let upper = lp::lagrangian_value(problem, factors, &bound.prices)?.bound;
    if upper != bound.bound || upper < primal {
        return Err(failed(
            "plan_cert: whole-block Lagrangian bound failed replay",
        ));
    }
    let denominator = meta
        .value_scale_denominator
        .checked_mul(DISCOUNT_SCALE as u128)
        .ok_or_else(|| failed("plan_cert: value and discount scale product overflows u128"))?;

    let mut model = Encoder::new_frame(*b"WBPM", 1);
    model.put_bytes(closure_model_digest(problem.blocks()).as_bytes());
    model.put_u64(problem.periods() as u64);
    model.put_i64(discount[0]);
    model.put_i64(discount[1]);
    for &factor in factors {
        model.put_i64(factor);
    }
    model.put_u64(problem.resources() as u64);
    for resource in 0..problem.resources() {
        // DESIGN: ScheduleProblem fixes these lengths to blocks and periods at construction.
        for &qty in problem.usage(resource).unwrap_or(&[]) {
            model.put_i64(qty);
        }
        for &limit in problem.limits(resource).unwrap_or(&[]) {
            model.put_i64(limit);
        }
    }

    let mut witness = Encoder::new_frame(*b"WBLG", 1);
    witness.put_u64(schedule.len() as u64);
    for &period in schedule {
        witness.put_u32(period.unwrap_or(u32::MAX));
    }
    witness.put_i128(upper);
    witness.put_u64(bound.prices.len() as u64);
    for row in &bound.prices {
        witness.put_u64(row.len() as u64);
        for &price in row {
            witness.put_i64(price);
        }
    }

    PlanCertificate::build(PlanCertificateRequest {
        plan_id: meta.plan_id.clone(),
        horizon: meta.horizon,
        code_revision: meta.code_revision.clone(),
        model_digest: sha256(&model.finish()),
        opt_set_id: meta.opt_set_id.clone(),
        currency_code: meta.currency_code.clone(),
        value_scale_denominator: denominator,
        primal_scaled: primal,
        bound: Some(CertifiedBound {
            lower_scaled: upper,
            upper_scaled: upper,
            kind: BoundKind::Lagrangian {
                iters: u32::try_from(bound.history.len().max(1)).unwrap_or(u32::MAX),
            },
            witness_digest: sha256(&witness.finish()),
            verifier_id: "fmine-optimize::toposort::lp::lagrangian_value".into(),
        }),
        saa: meta.saa.clone(),
        vss: meta.vss.clone(),
        calibration: meta.calibration.clone(),
        inputs: meta.inputs.clone(),
        refusal_codes: meta.extra_refusal_codes.clone(),
    })
    .map_err(map_cert_error)
}

fn map_cert_error(err: PlanCertError) -> Refusal {
    let code = match err {
        PlanCertError::BoundBelowPrimal { .. } | PlanCertError::GapMismatch { .. } => {
            RefusalCode::NumericalCertificateFailed207
        }
        PlanCertError::DisjointSetViolation { .. } | PlanCertError::ColorLaundering { .. } => {
            RefusalCode::CalibrationRequired205
        }
        PlanCertError::AuthorityClaimForbidden { .. } => RefusalCode::MissingHumanToken304,
        PlanCertError::DigestMismatch { .. } => RefusalCode::DigestMismatch505,
        _ => RefusalCode::OutsidePhysicalDomain109,
    };
    Refusal::new(code, err.to_string())
}

/// Deterministic SHA-256 content digest of a [`ClosureProblem`] (weights and CSR precedence arcs).
#[must_use]
pub fn closure_model_digest(problem: &ClosureProblem) -> Digest32 {
    let mut enc = Encoder::new_frame(*b"CLPM", 1);
    enc.put_u64(problem.node_count() as u64);
    enc.put_u64(problem.arc_count() as u64);
    for &w in problem.weights() {
        enc.put_i64(w);
    }
    for u in 0..problem.node_count() {
        let reqs = problem.requires(u).unwrap_or(&[]);
        enc.put_u64(reqs.len() as u64);
        for &v in reqs {
            enc.put_u32(v);
        }
    }
    sha256(&enc.finish())
}

fn pit_witness_digest(in_pit: &[bool], arc_flow: &[i64]) -> Digest32 {
    let mut enc = Encoder::new_frame(*b"PITW", 1);
    enc.put_u64(in_pit.len() as u64);
    for &flag in in_pit {
        enc.put_u8(u8::from(flag));
    }
    enc.put_u64(arc_flow.len() as u64);
    for &f in arc_flow {
        enc.put_i64(f);
    }
    sha256(&enc.finish())
}

/// Deterministic SHA-256 content digest of a [`PcpspModel`] (blocks, periods, destinations,
/// resources, values, uses, limits, discount rate, and precedence arcs).
#[must_use]
pub fn pcpsp_model_digest(model: &PcpspModel) -> Digest32 {
    let mut enc = Encoder::new_frame(*b"PCPM", 2);
    let nb = model.blocks();
    let nd = model.destinations();
    let nr = model.resources();
    enc.put_u64(nb as u64);
    enc.put_u64(model.periods() as u64);
    enc.put_u64(nd as u64);
    enc.put_u64(nr as u64);
    enc.put_i64(model.discount_rate);
    for b in 0..nb {
        for d in 0..nd {
            enc.put_i64(model.value(b, d).unwrap_or(0));
        }
        let reqs = model.requires(b).unwrap_or(&[]);
        enc.put_u64(reqs.len() as u64);
        for &v in reqs {
            enc.put_u32(v);
        }
    }
    for r in 0..nr {
        for b in 0..nb {
            for d in 0..nd {
                enc.put_i64(model.uses(r, b, d).unwrap_or(0));
            }
        }
        let lims = model.limits(r).unwrap_or(&[]);
        enc.put_u64(lims.len() as u64);
        for &l in lims {
            enc.put_i64(l);
        }
    }
    sha256(&enc.finish())
}

fn pcpsp_witness_digest(plan: &pcpsp::planning::Plan) -> Digest32 {
    let mut enc = Encoder::new_frame(*b"LAGW", 1);
    enc.put_str(plan.origin);
    enc.put_u32(plan.schedule.share_decimals());
    enc.put_str(&pcpsp::candidate::write_solution(&plan.schedule));
    enc.put_u64(plan.factors.len() as u64);
    for &f in &plan.factors {
        enc.put_i64(f);
    }
    enc.put_i128(plan.bound.bound);
    enc.put_u64(plan.bound.prices.len() as u64);
    for row in &plan.bound.prices {
        enc.put_u64(row.len() as u64);
        for &p in row {
            enc.put_i64(p);
        }
    }
    sha256(&enc.finish())
}

/// Independently verify a [`PitSolution`] via [`pitflow::verify`] and emit a three-gap
/// [`PlanCertificate`] (`BoundKind::ClosureExact`, zero optimization gap).
///
/// # Errors
/// `FMINE-REF-207` if the pit closure or pseudoflow witness fails independent verification;
/// `FMINE-REF-109`/`205`/`304` if certificate metadata or three-gap invariants fail.
pub fn certify_ultimate_pit(
    meta: &PlanCertMetadata,
    problem: &ClosureProblem,
    solution: &PitSolution,
) -> Result<PlanCertificate, Refusal> {
    let verified_value = pitflow::verify(problem, solution.in_pit(), solution.arc_flow())?;
    if verified_value != solution.value() {
        return Err(Refusal::new(
            RefusalCode::NumericalCertificateFailed207,
            format!(
                "plan_cert: verified pit value ({verified_value}) != solution.value() ({})",
                solution.value()
            ),
        ));
    }
    let primal = i128::from(verified_value);
    let model_digest = closure_model_digest(problem);
    let witness_digest = pit_witness_digest(solution.in_pit(), solution.arc_flow());

    PlanCertificate::build(PlanCertificateRequest {
        plan_id: meta.plan_id.clone(),
        horizon: meta.horizon,
        code_revision: meta.code_revision.clone(),
        model_digest,
        opt_set_id: meta.opt_set_id.clone(),
        currency_code: meta.currency_code.clone(),
        value_scale_denominator: meta.value_scale_denominator,
        primal_scaled: primal,
        bound: Some(CertifiedBound {
            lower_scaled: primal,
            upper_scaled: primal,
            kind: BoundKind::ClosureExact,
            witness_digest,
            verifier_id: "fmine-optimize::pitflow::verify".to_string(),
        }),
        saa: meta.saa.clone(),
        vss: meta.vss.clone(),
        calibration: meta.calibration.clone(),
        inputs: meta.inputs.clone(),
        refusal_codes: meta.extra_refusal_codes.clone(),
    })
    .map_err(map_cert_error)
}

/// Independently re-check a PCPSP [`pcpsp::planning::Plan`] (schedule feasibility, exact discounted
/// objective, and full-model Lagrangian dual replay) and emit a three-gap [`PlanCertificate`].
///
/// # Errors
/// `FMINE-REF-207` if the schedule fails independent feasibility/value verification or the
/// Lagrangian prices fail full-model replay; `FMINE-REF-109`/`205`/`304` on invalid metadata or
/// sampling/calibration invariants.
pub fn certify_pcpsp_plan(
    meta: &PlanCertMetadata,
    model: &PcpspModel,
    plan: &pcpsp::planning::Plan,
) -> Result<PlanCertificate, Refusal> {
    if plan.factors != discount_factors(model.discount_rate, RATE_SCALE, model.periods())? {
        return Err(Refusal::new(
            RefusalCode::NumericalCertificateFailed207,
            "plan_cert: schedule discount factors disagree with the model rate and periods",
        ));
    }
    let checked = pcpsp::candidate::check(model, &plan.schedule)?;
    if !checked.feasible() {
        return Err(Refusal::new(
            RefusalCode::NumericalCertificateFailed207,
            "plan_cert: PCPSP schedule is infeasible on independent check",
        ));
    }
    let recomputed_primal = checked.discounted_value(&plan.factors)?;
    if recomputed_primal != plan.discounted_numerator {
        return Err(Refusal::new(
            RefusalCode::NumericalCertificateFailed207,
            format!(
                "plan_cert: recomputed PCPSP value ({recomputed_primal}) != plan.discounted_numerator ({})",
                plan.discounted_numerator
            ),
        ));
    }
    let all_blocks: Vec<usize> = (0..model.blocks()).collect();
    let replay =
        pcpsp::lp::lagrangian_value(model, &all_blocks, &plan.factors, &plan.bound.prices)?;
    if replay.bound != plan.bound.bound {
        return Err(Refusal::new(
            RefusalCode::NumericalCertificateFailed207,
            format!(
                "plan_cert: replayed Lagrangian bound ({}) != reported bound ({})",
                replay.bound, plan.bound.bound
            ),
        ));
    }
    let share_scale = i128::from(plan.schedule.scale());
    let bound_scaled = replay.bound.checked_mul(share_scale).ok_or_else(|| {
        Refusal::new(
            RefusalCode::NumericalCertificateFailed207,
            "plan_cert: scaled Lagrangian bound overflows i128",
        )
    })?;
    let expected_gap = bound_scaled.checked_sub(recomputed_primal).ok_or_else(|| {
        Refusal::new(
            RefusalCode::NumericalCertificateFailed207,
            "plan_cert: PCPSP gap subtraction overflows i128",
        )
    })?;
    if expected_gap != plan.gap_numerator || expected_gap < 0 {
        return Err(Refusal::new(
            RefusalCode::NumericalCertificateFailed207,
            format!(
                "plan_cert: replayed gap ({expected_gap}) disagrees with plan.gap_numerator ({})",
                plan.gap_numerator
            ),
        ));
    }
    let denom = u128::try_from(plan.schedule.scale())
        .ok()
        .and_then(|s| s.checked_mul(u128::try_from(DISCOUNT_SCALE).ok()?))
        .and_then(|s| s.checked_mul(meta.value_scale_denominator))
        .ok_or_else(|| {
            Refusal::new(
                RefusalCode::NumericalCertificateFailed207,
                "plan_cert: value, share and discount scale product overflows u128",
            )
        })?;
    let iters = u32::try_from(plan.bound.history.len().max(1)).unwrap_or(u32::MAX);
    let model_digest = pcpsp_model_digest(model);
    let witness_digest = pcpsp_witness_digest(plan);

    PlanCertificate::build(PlanCertificateRequest {
        plan_id: meta.plan_id.clone(),
        horizon: meta.horizon,
        code_revision: meta.code_revision.clone(),
        model_digest,
        opt_set_id: meta.opt_set_id.clone(),
        currency_code: meta.currency_code.clone(),
        value_scale_denominator: denom,
        primal_scaled: recomputed_primal,
        bound: Some(CertifiedBound {
            lower_scaled: bound_scaled,
            upper_scaled: bound_scaled,
            kind: BoundKind::Lagrangian { iters },
            witness_digest,
            verifier_id: "fmine-optimize::toposort::pcpsp::lp::lagrangian_value".to_string(),
        }),
        saa: meta.saa.clone(),
        vss: meta.vss.clone(),
        calibration: meta.calibration.clone(),
        inputs: meta.inputs.clone(),
        refusal_codes: meta.extra_refusal_codes.clone(),
    })
    .map_err(map_cert_error)
}

/// Seal a heuristic plan that carries NO verified dual bound (`G-PUSH-3`, plan §15.4).
///
/// Always attaches `FMINE-REF-205` and forces `PlanColor::Estimated` so an unbounded heuristic can
/// never masquerade as certified-optimal.
pub fn certify_unbounded_heuristic(
    meta: &PlanCertMetadata,
    model_digest: Digest32,
    primal_scaled: i128,
) -> Result<PlanCertificate, Refusal> {
    let mut refusals = meta.extra_refusal_codes.clone();
    if !refusals.iter().any(|c| c == "FMINE-REF-205") {
        refusals.push("FMINE-REF-205".to_string());
    }
    PlanCertificate::build(PlanCertificateRequest {
        plan_id: meta.plan_id.clone(),
        horizon: meta.horizon,
        code_revision: meta.code_revision.clone(),
        model_digest,
        opt_set_id: meta.opt_set_id.clone(),
        currency_code: meta.currency_code.clone(),
        value_scale_denominator: meta.value_scale_denominator,
        primal_scaled,
        bound: None,
        saa: meta.saa.clone(),
        vss: meta.vss.clone(),
        calibration: meta.calibration.clone(),
        inputs: meta.inputs.clone(),
        refusal_codes: refusals,
    })
    .map_err(map_cert_error)
}

#[cfg(test)]
mod tests {
    use super::*;
    use fmine_core::plan_cert::PlanColor;

    fn base_meta(calibration: PlanCalibrationStatus) -> PlanCertMetadata {
        PlanCertMetadata {
            plan_id: "pit-cert-001".to_string(),
            horizon: PlanHorizon::Strategic,
            code_revision: "e3aadb3".to_string(),
            opt_set_id: "opt-set-1".to_string(),
            currency_code: "USD".to_string(),
            value_scale_denominator: 1,
            saa: Some(SaaGap {
                opt_set_id: "opt-set-1".to_string(),
                eval_set_id: "eval-set-2".to_string(),
                n_eval: 200,
                sample_mean_scaled: 14,
                sample_se_scaled: 2,
                conf_bps: 9500,
                ci_lo_scaled: 10,
                ci_hi_scaled: 18,
            }),
            vss: None,
            calibration,
            inputs: vec![InputDigest::new(
                "block_model",
                "models/pit.upit",
                sha256(b"pit-input"),
            )],
            extra_refusal_codes: vec![],
        }
    }

    #[test]
    fn test_certify_ultimate_pit_exact_zero_gap_and_honest_color() {
        // Block 0 (waste -4), Block 1 (ore +10, requires 0), Block 2 (waste -20, requires 0)
        // Optimal closure: {0, 1} with value +6.
        let problem =
            ClosureProblem::from_requirements(vec![-4, 10, -20], &[vec![], vec![0], vec![0]])
                .expect("valid closure");
        let sol = pitflow::solve(&problem).expect("pit solves");
        assert_eq!(sol.value(), 6);

        let calibrated = certify_ultimate_pit(
            &base_meta(PlanCalibrationStatus::Calibrated {
                ledger_ref: "cal-2026q3".to_string(),
                n_resolved: 36,
                cov80_bps: 8056,
                bias_bps: -40,
            }),
            &problem,
            &sol,
        )
        .expect("calibrated pit cert");
        assert_eq!(calibrated.primal_scaled, 6);
        assert_eq!(calibrated.opt_gap_numerator, Some(0));
        assert_eq!(calibrated.opt_gap_bps(), Some(0));
        assert_eq!(calibrated.color, PlanColor::Validated);
        assert_eq!(
            PlanCertificate::from_bytes(&calibrated.to_bytes().expect("encode")).expect("decode"),
            calibrated
        );

        // Even with exact 0% optimization gap, Uncalibrated model forces Estimated color (NL-4, §15.2).
        let uncal = certify_ultimate_pit(
            &base_meta(PlanCalibrationStatus::Uncalibrated),
            &problem,
            &sol,
        )
        .expect("uncalibrated pit cert");
        assert_eq!(uncal.opt_gap_numerator, Some(0));
        assert_eq!(uncal.color, PlanColor::Estimated);
    }

    #[test]
    fn test_whole_block_certificate_refuses_changed_proof_inputs() {
        let closure = ClosureProblem::from_requirements(vec![-4, 10], &[vec![], vec![0]])
            .expect("valid closure");
        let problem = ScheduleProblem::new(closure, 2, vec![vec![1, 1]], vec![vec![1, 1]])
            .expect("valid schedule problem");
        let discount = [1, 10];
        let factors = discount_factors(discount[0], discount[1], 2).expect("valid rate");
        let schedule = [Some(0), Some(1)];
        let primal = -4 * i128::from(DISCOUNT_SCALE) + 10 * i128::from(factors[1]);
        let bound = lp::lagrangian_bound(&problem, &factors, primal, 8).expect("bound");
        let mut meta = base_meta(PlanCalibrationStatus::Uncalibrated);
        meta.saa = None;
        meta.value_scale_denominator = 100;
        let build = |discount, schedule: &[Option<u32>], primal, bound: &lp::Lagrangian| {
            certify_whole_block_schedule(
                &meta, &problem, discount, &factors, schedule, primal, bound,
            )
        };
        let cert = build(discount, &schedule, primal, &bound).expect("checked witness");
        assert_eq!(cert.primal_scaled, primal);
        assert_eq!(cert.value_scale_denominator, 100 * DISCOUNT_SCALE as u128);
        assert_eq!(cert.opt_gap_numerator, Some(bound.bound - primal));
        assert_eq!(cert.color, PlanColor::Estimated);
        let wrong_bound = lp::Lagrangian {
            bound: bound.bound + 1,
            ..bound.clone()
        };
        for attempt in [
            build([2, 10], &schedule, primal, &bound),
            build(discount, &[Some(0), Some(0)], primal, &bound),
            build(discount, &schedule, primal + 1, &bound),
            build(discount, &schedule, primal, &wrong_bound),
        ] {
            assert_eq!(attempt.expect_err("bad proof").code, "FMINE-REF-207");
        }
    }

    #[test]
    fn test_certify_pcpsp_plan_replays_lagrangian_bound() {
        let pcpsp_text = "\
NAME: split-blend-example
TYPE: PCPSP
NBLOCKS: 3
NPERIODS: 2
NDESTINATIONS: 2
NRESOURCE_SIDE_CONSTRAINTS: 2
NGENERAL_SIDE_CONSTRAINTS: 0
DISCOUNT_RATE: 0
RESOURCE_CONSTRAINT_LIMITS:
0 0 L 2
0 1 L 2
1 0 L 0
1 1 L 0
OBJECTIVE_FUNCTION:
0 -4 12
1 -2 8
2 -3 21
RESOURCE_CONSTRAINT_COEFFICIENTS:
0 0 0 1
0 1 0 1
0 1 1 2
1 0 0 1
1 1 0 1
1 1 1 -1
2 0 0 1
2 1 0 1
EOF
";
        let prec_text = "0 0\n1 0\n2 1 0\n";
        let model = pcpsp::parse(
            pcpsp_text,
            prec_text,
            3,
            pitflow::minelib::Rounding::Refuse,
            0,
        )
        .expect("parse tiny pcpsp");
        let plan = pcpsp::planning::solve(&model, pcpsp::planning::Options::default(), None)
            .expect("solve tiny pcpsp");

        let meta = PlanCertMetadata {
            plan_id: "pcpsp-tiny-001".to_string(),
            horizon: PlanHorizon::Tactical,
            code_revision: "e3aadb3".to_string(),
            opt_set_id: "opt-set-1".to_string(),
            currency_code: "USD".to_string(),
            value_scale_denominator: 1_000,
            saa: None,
            vss: None,
            calibration: PlanCalibrationStatus::Miscalibrated {
                ledger_ref: "cal-2026q3".to_string(),
                bias_bps: 600,
                inflation_bps: 12_000,
            },
            inputs: vec![],
            extra_refusal_codes: vec![],
        };
        let cert = certify_pcpsp_plan(&meta, &model, &plan).expect("certify pcpsp");

        assert_eq!(cert.primal_scaled, plan.discounted_numerator);
        assert_eq!(
            cert.value_scale_denominator,
            1_000 * plan.schedule.scale() as u128 * DISCOUNT_SCALE as u128
        );
        assert_eq!(cert.opt_gap_numerator, Some(plan.gap_numerator));
        assert!(matches!(
            cert.bound.as_ref().map(|b| b.kind),
            Some(BoundKind::Lagrangian { .. })
        ));
        assert_eq!(cert.color, PlanColor::Estimated);
        assert_eq!(
            PlanCertificate::from_bytes(&cert.to_bytes().expect("encode")).expect("decode"),
            cert
        );
        let changed_rate = pcpsp::parse(
            &pcpsp_text.replace("DISCOUNT_RATE: 0", "DISCOUNT_RATE: 1"),
            prec_text,
            3,
            pitflow::minelib::Rounding::Refuse,
            0,
        )
        .expect("model with a new discount rate");
        assert_ne!(
            pcpsp_model_digest(&model),
            pcpsp_model_digest(&changed_rate)
        );
        let refusal =
            certify_pcpsp_plan(&meta, &changed_rate, &plan).expect_err("old discount factors");
        assert_eq!(refusal.code, "FMINE-REF-207");
        assert!(refusal.what.contains("discount factors disagree"));
    }

    #[test]
    fn test_certify_unbounded_heuristic_forces_estimated_and_refusal() {
        let cert = certify_unbounded_heuristic(
            &base_meta(PlanCalibrationStatus::Calibrated {
                ledger_ref: "cal-2026q3".to_string(),
                n_resolved: 40,
                cov80_bps: 8000,
                bias_bps: 0,
            }),
            sha256(b"stope-model"),
            42_000,
        )
        .expect("heuristic cert");
        assert!(cert.bound.is_none());
        assert!(cert.opt_gap_numerator.is_none());
        assert_eq!(cert.color, PlanColor::Estimated);
        assert!(cert.refusal_codes.iter().any(|c| c == "FMINE-REF-205"));
    }
}
