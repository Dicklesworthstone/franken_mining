//! `FMT-PCERT-1` three-gap `PlanCertificate` wire format and offline invariants (`BD-253`, `GOAL-010`,
//! `NS-16`, plan §15.2).
//! Determinism: BitwisePortable
//!
//! Every published strategic, tactical, or short-term mine plan carries a self-verifying three-gap
//! certificate decomposing reported value against truth:
//! 1. **Optimization gap** (`z_model - z*_model`): certified by a verified dual/closure bound with
//!    witness digest, or explicitly `None` (unbounded heuristic, `G-PUSH-3`, `PlanColor::Estimated`).
//! 2. **Sampling gap** (`z*_model - z*_population`): SAA confidence interval and optional out-of-sample
//!    `VssCertificate` requiring strictly disjoint `eval_set_id != opt_set_id` (plan §15.2).
//! 3. **Calibration gap** (`z*_population - z_truth`): `Calibrated`, `Uncalibrated`, or `Miscalibrated`
//!    record from the reconciliation ledger.
//!
//! **Honesty guards (NL-1 / NL-4 / INV-002):**
//! - A certificate may carry `PlanColor::Validated` ONLY when a certified bound is present,
//!   calibration is `Calibrated`, and `refusal_codes` is empty. Otherwise `PlanColor::Estimated` is
//!   mandatory: a "0.1% from optimal" headline on an uncalibrated or miscalibrated model is never
//!   `Validated` (plan §15.2).
//! - All four `INV-002` authority flags (`reserve_disclosure_claim`, `execution_authorized`,
//!   `geotechnical_signoff`, `regulatory_compliance_claim`) are encoded on wire and MUST be `false`.
//!
//! Wire format: `FMNE` + `PCRT` + `version: u16` + canonical fields + `certificate_digest` + CRC32C.

use core::fmt;

use crate::cert::{InputDigest, MAX_CERT_INPUTS, MAX_CERT_STR_BYTES};
use crate::codec::{CodecError, Decoder, Encoder};
use crate::hash::{Digest32, sha256};

/// Format tag for `FMT-PCERT-1` three-gap plan certificates (`FMNE` + `PCRT`).
pub const TAG_PLAN_CERT: [u8; 4] = *b"PCRT";

/// Domain-separation tag for canonical `PlanCertificate` body hashing (`PCRB`).
const TAG_PLAN_CERT_BODY: [u8; 4] = *b"PCRB";

/// Current schema version for `FMT-PCERT-1`.
pub const FMT_PLANCERT_V1: u16 = 1;

/// Minimum supported schema version for `FMT-PCERT-1`.
pub const FMT_PLANCERT_MIN_VERSION: u16 = 1;

/// Maximum number of refusal codes attached to one plan certificate.
pub const MAX_PLAN_CERT_REFUSALS: usize = 64;

/// Planning horizon per plan §15.3.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum PlanHorizon {
    /// Life-of-mine / annual strategic shells, pushbacks, and LOM schedule.
    Strategic = 1,
    /// Budget / quarterly / monthly tactical schedule, stockpile, and blend policy.
    Tactical = 2,
    /// Weekly / shift receding-horizon dig sequence (advisory only, `INV-011`).
    ShortTerm = 3,
}

impl PlanHorizon {
    #[must_use]
    pub const fn tag(self) -> u8 {
        self as u8
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Strategic => "strategic",
            Self::Tactical => "tactical",
            Self::ShortTerm => "short_term",
        }
    }

    fn from_u8(tag: u8) -> Result<Self, PlanCertError> {
        match tag {
            1 => Ok(Self::Strategic),
            2 => Ok(Self::Tactical),
            3 => Ok(Self::ShortTerm),
            other => Err(PlanCertError::InvalidTag {
                field: "horizon",
                tag: other,
            }),
        }
    }
}

/// Method that produced the certified upper bound on the model objective (plan §15.2).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BoundKind {
    /// Exact maximum-weight closure verified independently (`pitflow::verify`, gap = 0).
    ClosureExact,
    /// Bienstock–Zuckerberg LP relaxation upper bound.
    BzLpRelaxation,
    /// Lagrangian relaxation dual bound at `iters` subgradient/bundle iterations.
    Lagrangian { iters: u32 },
    /// Restarted PDLP dual-repaired outer bound (`fmine-lp`).
    PdlpDualRepair,
    /// Statistical SAA upper bound at confidence `conf_bps` (in `1..=9999`) over `batches >= 2`.
    SaaStatistical { conf_bps: u16, batches: u16 },
}

impl BoundKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ClosureExact => "closure_exact",
            Self::BzLpRelaxation => "bz_lp_relaxation",
            Self::Lagrangian { .. } => "lagrangian",
            Self::PdlpDualRepair => "pdlp_dual_repair",
            Self::SaaStatistical { .. } => "saa_statistical",
        }
    }

    fn validate(self) -> Result<(), PlanCertError> {
        match self {
            Self::ClosureExact | Self::BzLpRelaxation | Self::PdlpDualRepair => Ok(()),
            Self::Lagrangian { iters } => {
                if iters == 0 {
                    return Err(PlanCertError::InvalidField {
                        field: "bound.kind.iters",
                        detail: "Lagrangian bound requires at least 1 iteration".to_string(),
                    });
                }
                Ok(())
            }
            Self::SaaStatistical { conf_bps, batches } => {
                if conf_bps == 0 || conf_bps >= 10_000 {
                    return Err(PlanCertError::InvalidField {
                        field: "bound.kind.conf_bps",
                        detail: format!("conf_bps {conf_bps} must lie in 1..=9999"),
                    });
                }
                if batches < 2 {
                    return Err(PlanCertError::InvalidField {
                        field: "bound.kind.batches",
                        detail: format!("batches {batches} must be >= 2"),
                    });
                }
                Ok(())
            }
        }
    }

    fn encode(self, enc: &mut Encoder) {
        match self {
            Self::ClosureExact => enc.put_u8(1),
            Self::BzLpRelaxation => enc.put_u8(2),
            Self::Lagrangian { iters } => {
                enc.put_u8(3);
                enc.put_u32(iters);
            }
            Self::PdlpDualRepair => enc.put_u8(4),
            Self::SaaStatistical { conf_bps, batches } => {
                enc.put_u8(5);
                enc.put_u16(conf_bps);
                enc.put_u16(batches);
            }
        }
    }

    fn decode(dec: &mut Decoder<'_>) -> Result<Self, PlanCertError> {
        match dec.get_u8()? {
            1 => Ok(Self::ClosureExact),
            2 => Ok(Self::BzLpRelaxation),
            3 => Ok(Self::Lagrangian {
                iters: dec.get_u32()?,
            }),
            4 => Ok(Self::PdlpDualRepair),
            5 => Ok(Self::SaaStatistical {
                conf_bps: dec.get_u16()?,
                batches: dec.get_u16()?,
            }),
            other => Err(PlanCertError::InvalidTag {
                field: "bound.kind",
                tag: other,
            }),
        }
    }
}

/// Verified dual or exact closure upper bound on the model objective (`[lower_scaled, upper_scaled]`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CertifiedBound {
    /// Lower endpoint of the enclosing bound interval (scaled by `value_scale_denominator`).
    pub lower_scaled: i128,
    /// Upper endpoint of the enclosing bound interval (scaled by `value_scale_denominator`).
    pub upper_scaled: i128,
    /// Method that certified this bound.
    pub kind: BoundKind,
    /// Non-zero SHA-256 / BLAKE3 content digest of the dual/closure witness artifact.
    pub witness_digest: Digest32,
    /// Identifier of the independent verifier that checked the witness.
    pub verifier_id: String,
}

impl CertifiedBound {
    fn validate(&self) -> Result<(), PlanCertError> {
        if self.lower_scaled > self.upper_scaled {
            return Err(PlanCertError::InvalidField {
                field: "bound",
                detail: format!(
                    "lower_scaled ({}) > upper_scaled ({})",
                    self.lower_scaled, self.upper_scaled
                ),
            });
        }
        self.kind.validate()?;
        if self.witness_digest == Digest32([0u8; 32]) {
            return Err(PlanCertError::InvalidField {
                field: "bound.witness_digest",
                detail: "witness_digest must not be all zeros".to_string(),
            });
        }
        check_str("bound.verifier_id", &self.verifier_id)?;
        Ok(())
    }
}

/// Sample Average Approximation (SAA) out-of-sample sampling gap (plan §15.2).
///
/// `eval_set_id` MUST differ from `opt_set_id` so in-sample optimism cannot pass as out-of-sample.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SaaGap {
    pub opt_set_id: String,
    pub eval_set_id: String,
    pub n_eval: u32,
    pub sample_mean_scaled: i128,
    pub sample_se_scaled: i128,
    pub conf_bps: u16,
    pub ci_lo_scaled: i128,
    pub ci_hi_scaled: i128,
}

impl SaaGap {
    fn validate(&self, cert_opt_set_id: &str) -> Result<(), PlanCertError> {
        check_str("saa.opt_set_id", &self.opt_set_id)?;
        check_str("saa.eval_set_id", &self.eval_set_id)?;
        if self.opt_set_id != cert_opt_set_id {
            return Err(PlanCertError::InvalidField {
                field: "saa.opt_set_id",
                detail: format!(
                    "saa.opt_set_id {:?} does not match certificate opt_set_id {:?}",
                    self.opt_set_id, cert_opt_set_id
                ),
            });
        }
        if self.eval_set_id == self.opt_set_id {
            return Err(PlanCertError::DisjointSetViolation {
                set_id: self.eval_set_id.clone(),
            });
        }
        if self.n_eval < 2 {
            return Err(PlanCertError::InvalidField {
                field: "saa.n_eval",
                detail: format!("n_eval {} must be >= 2 for a standard error", self.n_eval),
            });
        }
        if self.sample_se_scaled < 0 {
            return Err(PlanCertError::InvalidField {
                field: "saa.sample_se_scaled",
                detail: format!("sample_se_scaled {} must be >= 0", self.sample_se_scaled),
            });
        }
        if self.conf_bps == 0 || self.conf_bps >= 10_000 {
            return Err(PlanCertError::InvalidField {
                field: "saa.conf_bps",
                detail: format!("conf_bps {} must lie in 1..=9999", self.conf_bps),
            });
        }
        if !(self.ci_lo_scaled <= self.sample_mean_scaled
            && self.sample_mean_scaled <= self.ci_hi_scaled)
        {
            return Err(PlanCertError::InvalidField {
                field: "saa.ci",
                detail: format!(
                    "expected ci_lo ({}) <= mean ({}) <= ci_hi ({})",
                    self.ci_lo_scaled, self.sample_mean_scaled, self.ci_hi_scaled
                ),
            });
        }
        Ok(())
    }
}

/// Out-of-sample Value of the Stochastic Solution certificate (plan §15.2).
///
/// Refuses `eval_set_id == opt_set_id`, identical deterministic/stochastic plan ids, or inconsistent
/// optimism bounds.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VssCertificate {
    pub det_plan_id: String,
    pub sto_plan_id: String,
    pub opt_set_id: String,
    pub eval_set_id: String,
    pub n_eval: u32,
    pub vss_oos_lo_scaled: i128,
    pub vss_oos_hi_scaled: i128,
    pub vss_in_sample_scaled: i128,
    pub optimism_scaled: i128,
}

impl VssCertificate {
    fn validate(&self, cert_opt_set_id: &str) -> Result<(), PlanCertError> {
        check_str("vss.det_plan_id", &self.det_plan_id)?;
        check_str("vss.sto_plan_id", &self.sto_plan_id)?;
        check_str("vss.opt_set_id", &self.opt_set_id)?;
        check_str("vss.eval_set_id", &self.eval_set_id)?;
        if self.det_plan_id == self.sto_plan_id {
            return Err(PlanCertError::InvalidField {
                field: "vss.sto_plan_id",
                detail: "det_plan_id and sto_plan_id must be distinct".to_string(),
            });
        }
        if self.opt_set_id != cert_opt_set_id {
            return Err(PlanCertError::InvalidField {
                field: "vss.opt_set_id",
                detail: format!(
                    "vss.opt_set_id {:?} does not match certificate opt_set_id {:?}",
                    self.opt_set_id, cert_opt_set_id
                ),
            });
        }
        if self.eval_set_id == self.opt_set_id {
            return Err(PlanCertError::DisjointSetViolation {
                set_id: self.eval_set_id.clone(),
            });
        }
        if self.n_eval < 2 {
            return Err(PlanCertError::InvalidField {
                field: "vss.n_eval",
                detail: format!("n_eval {} must be >= 2", self.n_eval),
            });
        }
        if self.vss_oos_lo_scaled > self.vss_oos_hi_scaled {
            return Err(PlanCertError::InvalidField {
                field: "vss.vss_oos",
                detail: format!(
                    "vss_oos_lo ({}) > vss_oos_hi ({})",
                    self.vss_oos_lo_scaled, self.vss_oos_hi_scaled
                ),
            });
        }
        let min_opt = self
            .vss_in_sample_scaled
            .checked_sub(self.vss_oos_hi_scaled)
            .ok_or_else(|| PlanCertError::InvalidField {
                field: "vss.optimism_scaled",
                detail: "optimism lower bound overflowed i128".to_string(),
            })?;
        let max_opt = self
            .vss_in_sample_scaled
            .checked_sub(self.vss_oos_lo_scaled)
            .ok_or_else(|| PlanCertError::InvalidField {
                field: "vss.optimism_scaled",
                detail: "optimism upper bound overflowed i128".to_string(),
            })?;
        if self.optimism_scaled < min_opt || self.optimism_scaled > max_opt {
            return Err(PlanCertError::InvalidField {
                field: "vss.optimism_scaled",
                detail: format!(
                    "optimism {} lies outside [{min_opt}, {max_opt}] implied by in-sample minus OOS interval",
                    self.optimism_scaled
                ),
            });
        }
        Ok(())
    }
}

/// Calibration-gap record linking the plan to historical reconciliation (plan §15.2).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PlanCalibrationStatus {
    /// Scored against resolved historical outcomes in `ledger_ref`.
    Calibrated {
        ledger_ref: String,
        n_resolved: u32,
        /// Empirical 80% coverage in basis points (`0..=10_000`, target `8_000`).
        cov80_bps: u16,
        /// Signed mean bias in basis points (e.g. `-600` for -6.00%).
        bias_bps: i32,
    },
    /// No resolved calibration history at this support; forces `PlanColor::Estimated`.
    Uncalibrated,
    /// Known systematic bias or under-dispersion recorded in `ledger_ref`; forces `PlanColor::Estimated`.
    Miscalibrated {
        ledger_ref: String,
        bias_bps: i32,
        /// Variance/spread inflation factor in basis points (`10_000` = 1.00×; must be `> 0`).
        inflation_bps: u32,
    },
}

impl PlanCalibrationStatus {
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Calibrated { .. } => "calibrated",
            Self::Uncalibrated => "uncalibrated",
            Self::Miscalibrated { .. } => "miscalibrated",
        }
    }

    fn validate(&self) -> Result<(), PlanCertError> {
        match self {
            Self::Calibrated {
                ledger_ref,
                n_resolved,
                cov80_bps,
                ..
            } => {
                check_str("calibration.ledger_ref", ledger_ref)?;
                if *n_resolved == 0 {
                    return Err(PlanCertError::InvalidField {
                        field: "calibration.n_resolved",
                        detail: "calibrated status requires n_resolved > 0".to_string(),
                    });
                }
                if *cov80_bps > 10_000 {
                    return Err(PlanCertError::InvalidField {
                        field: "calibration.cov80_bps",
                        detail: format!("cov80_bps {cov80_bps} exceeds 10000"),
                    });
                }
                Ok(())
            }
            Self::Uncalibrated => Ok(()),
            Self::Miscalibrated {
                ledger_ref,
                inflation_bps,
                ..
            } => {
                check_str("calibration.ledger_ref", ledger_ref)?;
                if *inflation_bps == 0 {
                    return Err(PlanCertError::InvalidField {
                        field: "calibration.inflation_bps",
                        detail: "inflation_bps must be > 0".to_string(),
                    });
                }
                Ok(())
            }
        }
    }

    fn encode(&self, enc: &mut Encoder) {
        match self {
            Self::Calibrated {
                ledger_ref,
                n_resolved,
                cov80_bps,
                bias_bps,
            } => {
                enc.put_u8(1);
                enc.put_str(ledger_ref);
                enc.put_u32(*n_resolved);
                enc.put_u16(*cov80_bps);
                enc.put_i64(i64::from(*bias_bps));
            }
            Self::Uncalibrated => enc.put_u8(2),
            Self::Miscalibrated {
                ledger_ref,
                bias_bps,
                inflation_bps,
            } => {
                enc.put_u8(3);
                enc.put_str(ledger_ref);
                enc.put_i64(i64::from(*bias_bps));
                enc.put_u32(*inflation_bps);
            }
        }
    }

    fn decode(dec: &mut Decoder<'_>) -> Result<Self, PlanCertError> {
        match dec.get_u8()? {
            1 => {
                let ledger_ref = dec.get_str(MAX_CERT_STR_BYTES)?.to_string();
                let n_resolved = dec.get_u32()?;
                let cov80_bps = dec.get_u16()?;
                let bias_raw = dec.get_i64()?;
                let bias_bps =
                    i32::try_from(bias_raw).map_err(|_| PlanCertError::InvalidField {
                        field: "calibration.bias_bps",
                        detail: format!("bias_bps {bias_raw} out of i32 range"),
                    })?;
                Ok(Self::Calibrated {
                    ledger_ref,
                    n_resolved,
                    cov80_bps,
                    bias_bps,
                })
            }
            2 => Ok(Self::Uncalibrated),
            3 => {
                let ledger_ref = dec.get_str(MAX_CERT_STR_BYTES)?.to_string();
                let bias_raw = dec.get_i64()?;
                let bias_bps =
                    i32::try_from(bias_raw).map_err(|_| PlanCertError::InvalidField {
                        field: "calibration.bias_bps",
                        detail: format!("bias_bps {bias_raw} out of i32 range"),
                    })?;
                let inflation_bps = dec.get_u32()?;
                Ok(Self::Miscalibrated {
                    ledger_ref,
                    bias_bps,
                    inflation_bps,
                })
            }
            other => Err(PlanCertError::InvalidTag {
                field: "calibration",
                tag: other,
            }),
        }
    }
}

/// Plan evidence colour enforced by NL-1 / NL-4 / plan §15.2.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum PlanColor {
    /// Certified dual bound present, calibration status is `Calibrated`, and zero refusals.
    Validated = 1,
    /// Unbounded heuristic, uncalibrated model, miscalibrated model, or active refusal.
    Estimated = 2,
}

impl PlanColor {
    #[must_use]
    pub const fn tag(self) -> u8 {
        self as u8
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Validated => "Validated",
            Self::Estimated => "Estimated",
        }
    }

    fn from_u8(tag: u8) -> Result<Self, PlanCertError> {
        match tag {
            1 => Ok(Self::Validated),
            2 => Ok(Self::Estimated),
            other => Err(PlanCertError::InvalidTag {
                field: "color",
                tag: other,
            }),
        }
    }
}

/// Typed errors raised while constructing, decoding, or offline-verifying `FMT-PCERT-1`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PlanCertError {
    Codec(CodecError),
    InvalidTag {
        field: &'static str,
        tag: u8,
    },
    InvalidField {
        field: &'static str,
        detail: String,
    },
    NonCanonicalInputs,
    NonCanonicalRefusals,
    BoundBelowPrimal {
        primal: i128,
        bound_upper: i128,
    },
    GapMismatch {
        stored: Option<i128>,
        expected: Option<i128>,
    },
    DisjointSetViolation {
        set_id: String,
    },
    ColorLaundering {
        detail: String,
    },
    AuthorityClaimForbidden {
        flag: &'static str,
    },
    DigestMismatch {
        expected: Digest32,
        actual: Digest32,
    },
}

impl From<CodecError> for PlanCertError {
    fn from(value: CodecError) -> Self {
        Self::Codec(value)
    }
}

impl fmt::Display for PlanCertError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Codec(e) => write!(f, "plan certificate codec error: {e}"),
            Self::InvalidTag { field, tag } => {
                write!(f, "invalid plan certificate wire tag {tag} for {field}")
            }
            Self::InvalidField { field, detail } => {
                write!(f, "invalid plan certificate field {field}: {detail}")
            }
            Self::NonCanonicalInputs => {
                write!(f, "plan certificate inputs are not strictly ascending")
            }
            Self::NonCanonicalRefusals => {
                write!(
                    f,
                    "plan certificate refusal codes are not strictly ascending"
                )
            }
            Self::BoundBelowPrimal {
                primal,
                bound_upper,
            } => write!(
                f,
                "invalid optimization bound: primal ({primal}) exceeds certified upper bound ({bound_upper})"
            ),
            Self::GapMismatch { stored, expected } => write!(
                f,
                "optimization gap mismatch: stored {stored:?}, expected {expected:?}"
            ),
            Self::DisjointSetViolation { set_id } => write!(
                f,
                "sampling/VSS evaluation set {set_id:?} must be strictly disjoint from optimization set"
            ),
            Self::ColorLaundering { detail } => {
                write!(f, "NL-1/NL-4 plan colour laundering refused: {detail}")
            }
            Self::AuthorityClaimForbidden { flag } => {
                write!(
                    f,
                    "INV-002 forbids setting {flag}=true on a plan certificate"
                )
            }
            Self::DigestMismatch { expected, actual } => write!(
                f,
                "plan certificate digest mismatch: stored {}, recomputed {}",
                expected.to_hex(),
                actual.to_hex()
            ),
        }
    }
}

impl std::error::Error for PlanCertError {}

fn check_str(field: &'static str, s: &str) -> Result<(), PlanCertError> {
    if s.is_empty() || s.len() > MAX_CERT_STR_BYTES {
        return Err(PlanCertError::InvalidField {
            field,
            detail: format!("length {} outside 1..={MAX_CERT_STR_BYTES}", s.len()),
        });
    }
    Ok(())
}

/// Canonical three-gap `PlanCertificate` (`FMT-PCERT-1`, `BD-253`, plan §15.2).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlanCertificate {
    pub format_version: u16,
    pub plan_id: String,
    pub horizon: PlanHorizon,
    pub code_revision: String,
    pub model_digest: Digest32,
    pub opt_set_id: String,
    pub currency_code: String,
    pub value_scale_denominator: u128,
    pub primal_scaled: i128,
    pub bound: Option<CertifiedBound>,
    /// Exact `bound.upper_scaled - primal_scaled` (`>= 0`) when `bound.is_some()`, else `None`.
    pub opt_gap_numerator: Option<i128>,
    pub saa: Option<SaaGap>,
    pub vss: Option<VssCertificate>,
    pub calibration: PlanCalibrationStatus,
    pub color: PlanColor,
    pub inputs: Vec<InputDigest>,
    pub refusal_codes: Vec<String>,
    /// Always `false` (`INV-002`).
    pub reserve_disclosure_claim: bool,
    /// Always `false` (`INV-002`).
    pub execution_authorized: bool,
    /// Always `false` (`INV-002`).
    pub geotechnical_signoff: bool,
    /// Always `false` (`INV-002`).
    pub regulatory_compliance_claim: bool,
    pub certificate_digest: Digest32,
}

/// Inputs to [`PlanCertificate::build`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlanCertificateRequest {
    pub plan_id: String,
    pub horizon: PlanHorizon,
    pub code_revision: String,
    pub model_digest: Digest32,
    pub opt_set_id: String,
    pub currency_code: String,
    pub value_scale_denominator: u128,
    pub primal_scaled: i128,
    pub bound: Option<CertifiedBound>,
    pub saa: Option<SaaGap>,
    pub vss: Option<VssCertificate>,
    pub calibration: PlanCalibrationStatus,
    pub inputs: Vec<InputDigest>,
    pub refusal_codes: Vec<String>,
}

impl PlanCertificate {
    /// Construct and cryptographically seal a three-gap `FMT-PCERT-1` certificate.
    ///
    /// Computes `opt_gap_numerator` and honest `PlanColor` automatically and forces all four
    /// `INV-002` authority flags to `false`.
    pub fn build(mut req: PlanCertificateRequest) -> Result<Self, PlanCertError> {
        req.inputs.sort();
        req.refusal_codes.sort();
        req.refusal_codes.dedup();
        if req.bound.is_none() && !req.refusal_codes.iter().any(|c| c == "FMINE-REF-205") {
            req.refusal_codes.push("FMINE-REF-205".to_string());
            req.refusal_codes.sort();
            req.refusal_codes.dedup();
        }
        let opt_gap_numerator = match &req.bound {
            None => None,
            Some(b) => {
                let gap = b
                    .upper_scaled
                    .checked_sub(req.primal_scaled)
                    .ok_or_else(|| PlanCertError::InvalidField {
                        field: "opt_gap_numerator",
                        detail: "bound.upper_scaled - primal_scaled overflowed i128".to_string(),
                    })?;
                Some(gap)
            }
        };
        let color = expected_color(req.bound.is_some(), &req.calibration, &req.refusal_codes);
        let mut cert = Self {
            format_version: FMT_PLANCERT_V1,
            plan_id: req.plan_id,
            horizon: req.horizon,
            code_revision: req.code_revision,
            model_digest: req.model_digest,
            opt_set_id: req.opt_set_id,
            currency_code: req.currency_code,
            value_scale_denominator: req.value_scale_denominator,
            primal_scaled: req.primal_scaled,
            bound: req.bound,
            opt_gap_numerator,
            saa: req.saa,
            vss: req.vss,
            calibration: req.calibration,
            color,
            inputs: req.inputs,
            refusal_codes: req.refusal_codes,
            reserve_disclosure_claim: false,
            execution_authorized: false,
            geotechnical_signoff: false,
            regulatory_compliance_claim: false,
            certificate_digest: Digest32([0u8; 32]),
        };
        cert.validate_invariants()?;
        cert.certificate_digest = cert.compute_digest()?;
        Ok(cert)
    }

    /// Relative optimization gap in integer basis points (`ceil(gap * 10_000 / |bound_upper|)`),
    /// returning `Some(0)` when `gap == 0`, and `None` when unbounded or `bound_upper == 0` with `gap > 0`.
    #[must_use]
    pub fn opt_gap_bps(&self) -> Option<i128> {
        let bound = self.bound.as_ref()?;
        let gap = self.opt_gap_numerator?;
        if gap == 0 {
            return Some(0);
        }
        let denom = bound.upper_scaled.checked_abs()?;
        if denom == 0 {
            return None;
        }
        let scaled = gap.checked_mul(10_000)?;
        Some((scaled + denom - 1) / denom)
    }

    /// Validate all structural, mathematical three-gap, NL-1/NL-4, and `INV-002` invariants.
    pub fn validate_invariants(&self) -> Result<(), PlanCertError> {
        if self.format_version < FMT_PLANCERT_MIN_VERSION || self.format_version > FMT_PLANCERT_V1 {
            return Err(PlanCertError::Codec(CodecError::UnsupportedVersion {
                actual: self.format_version,
                min_supported: FMT_PLANCERT_MIN_VERSION,
                max_supported: FMT_PLANCERT_V1,
            }));
        }
        if self.reserve_disclosure_claim {
            return Err(PlanCertError::AuthorityClaimForbidden {
                flag: "reserve_disclosure_claim",
            });
        }
        if self.execution_authorized {
            return Err(PlanCertError::AuthorityClaimForbidden {
                flag: "execution_authorized",
            });
        }
        if self.geotechnical_signoff {
            return Err(PlanCertError::AuthorityClaimForbidden {
                flag: "geotechnical_signoff",
            });
        }
        if self.regulatory_compliance_claim {
            return Err(PlanCertError::AuthorityClaimForbidden {
                flag: "regulatory_compliance_claim",
            });
        }

        check_str("plan_id", &self.plan_id)?;
        check_str("code_revision", &self.code_revision)?;
        check_str("opt_set_id", &self.opt_set_id)?;
        check_str("currency_code", &self.currency_code)?;
        if self.value_scale_denominator == 0 {
            return Err(PlanCertError::InvalidField {
                field: "value_scale_denominator",
                detail: "value_scale_denominator must be >= 1".to_string(),
            });
        }
        if self.model_digest == Digest32([0u8; 32]) {
            return Err(PlanCertError::InvalidField {
                field: "model_digest",
                detail: "model_digest must not be all zeros".to_string(),
            });
        }

        match &self.bound {
            Some(b) => {
                b.validate()?;
                if self.primal_scaled > b.upper_scaled {
                    return Err(PlanCertError::BoundBelowPrimal {
                        primal: self.primal_scaled,
                        bound_upper: b.upper_scaled,
                    });
                }
                let expected = b
                    .upper_scaled
                    .checked_sub(self.primal_scaled)
                    .ok_or_else(|| PlanCertError::InvalidField {
                        field: "opt_gap_numerator",
                        detail: "bound.upper_scaled - primal_scaled overflowed i128".to_string(),
                    })?;
                if self.opt_gap_numerator != Some(expected) {
                    return Err(PlanCertError::GapMismatch {
                        stored: self.opt_gap_numerator,
                        expected: Some(expected),
                    });
                }
            }
            None => {
                if self.opt_gap_numerator.is_some() {
                    return Err(PlanCertError::GapMismatch {
                        stored: self.opt_gap_numerator,
                        expected: None,
                    });
                }
                if self.refusal_codes.is_empty() {
                    return Err(PlanCertError::ColorLaundering {
                        detail: "unbounded plan certificate must carry an explicit refusal code (G-PUSH-3)"
                            .to_string(),
                    });
                }
            }
        }

        if let Some(saa) = &self.saa {
            saa.validate(&self.opt_set_id)?;
        }
        if let Some(vss) = &self.vss {
            vss.validate(&self.opt_set_id)?;
        }
        self.calibration.validate()?;

        if self.inputs.len() > MAX_CERT_INPUTS {
            return Err(PlanCertError::Codec(CodecError::LengthOutOfBounds {
                len: self.inputs.len(),
                max: MAX_CERT_INPUTS,
            }));
        }
        for inp in &self.inputs {
            check_str("input.role", &inp.role)?;
            check_str("input.locator", &inp.locator)?;
        }
        if self.inputs.windows(2).any(|w| w[0] >= w[1]) {
            return Err(PlanCertError::NonCanonicalInputs);
        }

        if self.refusal_codes.len() > MAX_PLAN_CERT_REFUSALS {
            return Err(PlanCertError::Codec(CodecError::LengthOutOfBounds {
                len: self.refusal_codes.len(),
                max: MAX_PLAN_CERT_REFUSALS,
            }));
        }
        for code in &self.refusal_codes {
            if !code.starts_with("FMINE-REF-") || code.len() > 32 {
                return Err(PlanCertError::InvalidField {
                    field: "refusal_codes",
                    detail: format!("invalid refusal code {code:?}"),
                });
            }
        }
        if self.refusal_codes.windows(2).any(|w| w[0] >= w[1]) {
            return Err(PlanCertError::NonCanonicalRefusals);
        }

        let required_color =
            expected_color(self.bound.is_some(), &self.calibration, &self.refusal_codes);
        if self.color == PlanColor::Validated && required_color != PlanColor::Validated {
            return Err(PlanCertError::ColorLaundering {
                detail: format!(
                    "Validated colour is forbidden when bound_present={}, calibration={}, refusals={}",
                    self.bound.is_some(),
                    self.calibration.as_str(),
                    self.refusal_codes.len()
                ),
            });
        }
        Ok(())
    }

    fn encode_body_fields(&self, enc: &mut Encoder) -> Result<(), PlanCertError> {
        enc.put_str(&self.plan_id);
        enc.put_u8(self.horizon.tag());
        enc.put_str(&self.code_revision);
        enc.put_fixed(self.model_digest.as_bytes());
        enc.put_str(&self.opt_set_id);
        enc.put_str(&self.currency_code);
        enc.put_fixed(&self.value_scale_denominator.to_le_bytes());
        enc.put_i128(self.primal_scaled);
        match &self.bound {
            None => enc.put_u8(0),
            Some(b) => {
                enc.put_u8(1);
                enc.put_i128(b.lower_scaled);
                enc.put_i128(b.upper_scaled);
                b.kind.encode(enc);
                enc.put_fixed(b.witness_digest.as_bytes());
                enc.put_str(&b.verifier_id);
            }
        }
        match self.opt_gap_numerator {
            None => enc.put_u8(0),
            Some(gap) => {
                enc.put_u8(1);
                enc.put_i128(gap);
            }
        }
        match &self.saa {
            None => enc.put_u8(0),
            Some(s) => {
                enc.put_u8(1);
                enc.put_str(&s.opt_set_id);
                enc.put_str(&s.eval_set_id);
                enc.put_u32(s.n_eval);
                enc.put_i128(s.sample_mean_scaled);
                enc.put_i128(s.sample_se_scaled);
                enc.put_u16(s.conf_bps);
                enc.put_i128(s.ci_lo_scaled);
                enc.put_i128(s.ci_hi_scaled);
            }
        }
        match &self.vss {
            None => enc.put_u8(0),
            Some(v) => {
                enc.put_u8(1);
                enc.put_str(&v.det_plan_id);
                enc.put_str(&v.sto_plan_id);
                enc.put_str(&v.opt_set_id);
                enc.put_str(&v.eval_set_id);
                enc.put_u32(v.n_eval);
                enc.put_i128(v.vss_oos_lo_scaled);
                enc.put_i128(v.vss_oos_hi_scaled);
                enc.put_i128(v.vss_in_sample_scaled);
                enc.put_i128(v.optimism_scaled);
            }
        }
        self.calibration.encode(enc);
        enc.put_u8(self.color.tag());
        enc.put_canonical_seq(&self.inputs, |e, inp| {
            e.put_str(&inp.role);
            e.put_str(&inp.locator);
            e.put_fixed(inp.sha256.as_bytes());
        })?;
        enc.put_canonical_seq(&self.refusal_codes, |e, code| {
            e.put_str(code);
        })?;
        enc.put_u8(u8::from(self.reserve_disclosure_claim));
        enc.put_u8(u8::from(self.execution_authorized));
        enc.put_u8(u8::from(self.geotechnical_signoff));
        enc.put_u8(u8::from(self.regulatory_compliance_claim));
        Ok(())
    }

    /// Recompute `SHA-256` over the canonical `PCRB` body frame.
    pub fn compute_digest(&self) -> Result<Digest32, PlanCertError> {
        let mut enc = Encoder::new_frame(TAG_PLAN_CERT_BODY, self.format_version);
        self.encode_body_fields(&mut enc)?;
        Ok(sha256(&enc.finish()))
    }

    /// Independently re-verify every three-gap, NL-1/NL-4, INV-002, and cryptographic binding.
    pub fn verify(&self) -> Result<(), PlanCertError> {
        self.validate_invariants()?;
        let actual = self.compute_digest()?;
        if actual != self.certificate_digest {
            return Err(PlanCertError::DigestMismatch {
                expected: self.certificate_digest,
                actual,
            });
        }
        Ok(())
    }

    /// Encode this verified certificate into a canonical `FMT-PCERT-1` binary frame (`FMNE` + `PCRT` + CRC32C).
    pub fn to_bytes(&self) -> Result<Vec<u8>, PlanCertError> {
        self.verify()?;
        let mut enc = Encoder::new_frame(TAG_PLAN_CERT, self.format_version);
        self.encode_body_fields(&mut enc)?;
        enc.put_fixed(self.certificate_digest.as_bytes());
        Ok(enc.finish())
    }

    /// Decode and independently verify an `FMT-PCERT-1` frame.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, PlanCertError> {
        let mut dec = Decoder::open_frame(
            bytes,
            TAG_PLAN_CERT,
            FMT_PLANCERT_MIN_VERSION,
            FMT_PLANCERT_V1,
        )?;
        let format_version = dec.version();
        let plan_id = dec.get_str(MAX_CERT_STR_BYTES)?.to_string();
        let horizon = PlanHorizon::from_u8(dec.get_u8()?)?;
        let code_revision = dec.get_str(MAX_CERT_STR_BYTES)?.to_string();
        let model_digest = Digest32(dec.get_fixed::<32>()?);
        let opt_set_id = dec.get_str(MAX_CERT_STR_BYTES)?.to_string();
        let currency_code = dec.get_str(MAX_CERT_STR_BYTES)?.to_string();
        let value_scale_denominator = u128::from_le_bytes(dec.get_fixed::<16>()?);
        let primal_scaled = dec.get_i128()?;
        let bound = match dec.get_u8()? {
            0 => None,
            1 => {
                let lower_scaled = dec.get_i128()?;
                let upper_scaled = dec.get_i128()?;
                let kind = BoundKind::decode(&mut dec)?;
                let witness_digest = Digest32(dec.get_fixed::<32>()?);
                let verifier_id = dec.get_str(MAX_CERT_STR_BYTES)?.to_string();
                Some(CertifiedBound {
                    lower_scaled,
                    upper_scaled,
                    kind,
                    witness_digest,
                    verifier_id,
                })
            }
            other => {
                return Err(PlanCertError::InvalidTag {
                    field: "bound_present",
                    tag: other,
                });
            }
        };
        let opt_gap_numerator = match dec.get_u8()? {
            0 => None,
            1 => Some(dec.get_i128()?),
            other => {
                return Err(PlanCertError::InvalidTag {
                    field: "opt_gap_present",
                    tag: other,
                });
            }
        };
        let saa = match dec.get_u8()? {
            0 => None,
            1 => Some(SaaGap {
                opt_set_id: dec.get_str(MAX_CERT_STR_BYTES)?.to_string(),
                eval_set_id: dec.get_str(MAX_CERT_STR_BYTES)?.to_string(),
                n_eval: dec.get_u32()?,
                sample_mean_scaled: dec.get_i128()?,
                sample_se_scaled: dec.get_i128()?,
                conf_bps: dec.get_u16()?,
                ci_lo_scaled: dec.get_i128()?,
                ci_hi_scaled: dec.get_i128()?,
            }),
            other => {
                return Err(PlanCertError::InvalidTag {
                    field: "saa_present",
                    tag: other,
                });
            }
        };
        let vss = match dec.get_u8()? {
            0 => None,
            1 => Some(VssCertificate {
                det_plan_id: dec.get_str(MAX_CERT_STR_BYTES)?.to_string(),
                sto_plan_id: dec.get_str(MAX_CERT_STR_BYTES)?.to_string(),
                opt_set_id: dec.get_str(MAX_CERT_STR_BYTES)?.to_string(),
                eval_set_id: dec.get_str(MAX_CERT_STR_BYTES)?.to_string(),
                n_eval: dec.get_u32()?,
                vss_oos_lo_scaled: dec.get_i128()?,
                vss_oos_hi_scaled: dec.get_i128()?,
                vss_in_sample_scaled: dec.get_i128()?,
                optimism_scaled: dec.get_i128()?,
            }),
            other => {
                return Err(PlanCertError::InvalidTag {
                    field: "vss_present",
                    tag: other,
                });
            }
        };
        let calibration = PlanCalibrationStatus::decode(&mut dec)?;
        let color = PlanColor::from_u8(dec.get_u8()?)?;
        let inputs = dec.get_canonical_seq(MAX_CERT_INPUTS, |d| {
            let role = d.get_str(MAX_CERT_STR_BYTES)?.to_string();
            let locator = d.get_str(MAX_CERT_STR_BYTES)?.to_string();
            let sha256 = Digest32(d.get_fixed::<32>()?);
            Ok(InputDigest {
                role,
                locator,
                sha256,
            })
        })?;
        let refusal_codes =
            dec.get_canonical_seq(MAX_PLAN_CERT_REFUSALS, |d| Ok(d.get_str(32)?.to_string()))?;
        let reserve_disclosure_claim = decode_bool("reserve_disclosure_claim", dec.get_u8()?)?;
        let execution_authorized = decode_bool("execution_authorized", dec.get_u8()?)?;
        let geotechnical_signoff = decode_bool("geotechnical_signoff", dec.get_u8()?)?;
        let regulatory_compliance_claim =
            decode_bool("regulatory_compliance_claim", dec.get_u8()?)?;
        let certificate_digest = Digest32(dec.get_fixed::<32>()?);
        dec.finish()?;

        let cert = Self {
            format_version,
            plan_id,
            horizon,
            code_revision,
            model_digest,
            opt_set_id,
            currency_code,
            value_scale_denominator,
            primal_scaled,
            bound,
            opt_gap_numerator,
            saa,
            vss,
            calibration,
            color,
            inputs,
            refusal_codes,
            reserve_disclosure_claim,
            execution_authorized,
            geotechnical_signoff,
            regulatory_compliance_claim,
            certificate_digest,
        };
        cert.verify()?;
        Ok(cert)
    }
}

fn decode_bool(field: &'static str, b: u8) -> Result<bool, PlanCertError> {
    match b {
        0 => Ok(false),
        1 => Ok(true),
        other => Err(PlanCertError::InvalidTag { field, tag: other }),
    }
}

fn expected_color(
    has_bound: bool,
    calibration: &PlanCalibrationStatus,
    refusal_codes: &[String],
) -> PlanColor {
    if has_bound
        && matches!(calibration, PlanCalibrationStatus::Calibrated { .. })
        && refusal_codes.is_empty()
    {
        PlanColor::Validated
    } else {
        PlanColor::Estimated
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_request(calibration: PlanCalibrationStatus) -> PlanCertificateRequest {
        PlanCertificateRequest {
            plan_id: "plan-lom-2026-rev1".to_string(),
            horizon: PlanHorizon::Strategic,
            code_revision: "e3aadb3".to_string(),
            model_digest: sha256(b"block-model-v1"),
            opt_set_id: "rset-opt-200".to_string(),
            currency_code: "USD".to_string(),
            value_scale_denominator: 100,
            primal_scaled: 98_200_000,
            bound: Some(CertifiedBound {
                lower_scaled: 100_000_000,
                upper_scaled: 100_000_000,
                kind: BoundKind::Lagrangian { iters: 64 },
                witness_digest: sha256(b"lagrangian-witness"),
                verifier_id: "fmine-optimize::toposort::pcpsp::planning".to_string(),
            }),
            saa: Some(SaaGap {
                opt_set_id: "rset-opt-200".to_string(),
                eval_set_id: "rset-eval-1000".to_string(),
                n_eval: 1000,
                sample_mean_scaled: 97_900_000,
                sample_se_scaled: 900_000,
                conf_bps: 9500,
                ci_lo_scaled: 96_136_000,
                ci_hi_scaled: 99_664_000,
            }),
            vss: Some(VssCertificate {
                det_plan_id: "plan-det-2026".to_string(),
                sto_plan_id: "plan-lom-2026-rev1".to_string(),
                opt_set_id: "rset-opt-200".to_string(),
                eval_set_id: "rset-eval-1000".to_string(),
                n_eval: 1000,
                vss_oos_lo_scaled: 1_200_000,
                vss_oos_hi_scaled: 2_400_000,
                vss_in_sample_scaled: 2_600_000,
                optimism_scaled: 800_000,
            }),
            calibration,
            inputs: vec![InputDigest::new(
                "pcpsp_model",
                "models/pit.pcpsp",
                sha256(b"pcpsp-bytes"),
            )],
            refusal_codes: vec![],
        }
    }

    #[test]
    fn test_plan_cert_roundtrip_and_three_gap_honesty() {
        // Calibrated + bounded -> Validated
        let cert = PlanCertificate::build(sample_request(PlanCalibrationStatus::Calibrated {
            ledger_ref: "cal-2026q3".to_string(),
            n_resolved: 48,
            cov80_bps: 7917,
            bias_bps: -120,
        }))
        .expect("calibrated cert builds");
        assert_eq!(cert.color, PlanColor::Validated);
        assert_eq!(cert.opt_gap_numerator, Some(1_800_000));
        assert_eq!(cert.opt_gap_bps(), Some(180));
        assert!(!cert.reserve_disclosure_claim);
        assert!(!cert.execution_authorized);
        assert!(!cert.geotechnical_signoff);
        assert!(!cert.regulatory_compliance_claim);

        let bytes = cert.to_bytes().expect("encode");
        let decoded = PlanCertificate::from_bytes(&bytes).expect("decode");
        assert_eq!(decoded, cert);

        // Uncalibrated or Miscalibrated -> forced Estimated even with tight opt gap!
        let uncal = PlanCertificate::build(sample_request(PlanCalibrationStatus::Uncalibrated))
            .expect("uncalibrated cert");
        assert_eq!(uncal.color, PlanColor::Estimated);
        let miscal = PlanCertificate::build(sample_request(PlanCalibrationStatus::Miscalibrated {
            ledger_ref: "cal-2026q3".to_string(),
            bias_bps: 600,
            inflation_bps: 13_500,
        }))
        .expect("miscalibrated cert");
        assert_eq!(miscal.color, PlanColor::Estimated);
    }

    #[test]
    fn test_plan_cert_refuses_forged_color_bound_disjoint_and_authority() {
        // 1. Bound below primal refused
        let mut bad_bound = sample_request(PlanCalibrationStatus::Uncalibrated);
        bad_bound.bound.as_mut().unwrap().upper_scaled = 50_000_000;
        bad_bound.bound.as_mut().unwrap().lower_scaled = 50_000_000;
        assert!(matches!(
            PlanCertificate::build(bad_bound),
            Err(PlanCertError::BoundBelowPrimal { .. })
        ));

        // 2. Non-disjoint SAA eval_set == opt_set refused
        let mut same_set = sample_request(PlanCalibrationStatus::Uncalibrated);
        same_set.saa.as_mut().unwrap().eval_set_id = "rset-opt-200".to_string();
        assert!(matches!(
            PlanCertificate::build(same_set),
            Err(PlanCertError::DisjointSetViolation { .. })
        ));

        // 3. Forged Validated color on Uncalibrated cert re-sealed with fresh digest MUST fail verify
        let mut forged =
            PlanCertificate::build(sample_request(PlanCalibrationStatus::Uncalibrated)).unwrap();
        forged.color = PlanColor::Validated;
        forged.certificate_digest = forged.compute_digest().unwrap();
        assert!(matches!(
            forged.verify(),
            Err(PlanCertError::ColorLaundering { .. })
        ));

        // 4. Forged INV-002 authority flag re-sealed with fresh digest MUST fail verify
        let mut forged_auth =
            PlanCertificate::build(sample_request(PlanCalibrationStatus::Uncalibrated)).unwrap();
        forged_auth.reserve_disclosure_claim = true;
        forged_auth.certificate_digest = forged_auth.compute_digest().unwrap();
        assert!(matches!(
            forged_auth.verify(),
            Err(PlanCertError::AuthorityClaimForbidden {
                flag: "reserve_disclosure_claim"
            })
        ));
    }
}
