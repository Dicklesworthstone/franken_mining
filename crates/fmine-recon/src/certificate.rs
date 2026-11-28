//! Canonical `ReconciliationCertificate` (`BD-156`, plan §20.9, `FMT-CERT-1`, `PO-TOUCH-6`,
//! `G-TOUCH-4`, `INV-002`).
//! Determinism: DeterministicPerIsa (canonical binary framing `FMNE` + `RCRT` + CRC32C, self-verifying
//! SHA-256 payload digest, BLAKE3 replay digest, and `FMT-CERT-1` envelope conversion verified
//! offline by `fmine-verify`).

use fmine_core::anchor::LedgerAnchor;
use fmine_core::cert::{Certificate, DeterminismClass, InputDigest};
use fmine_core::codec::{Decoder, Encoder};
use fmine_core::hash::{Digest32, blake3_hash, sha256};

use crate::error::ReconError;
use crate::gross_error::CommitDisposition;

/// Format tag for canonical `ReconciliationCertificate` binary frames (`FMNE` + `RCRT`).
pub const TAG_RECON_CERT: [u8; 4] = *b"RCRT";

/// Current schema version for `ReconciliationCertificate`.
pub const FMT_RECON_CERT_V1: u16 = 1;

/// Subject kind recorded in the outer `FMT-CERT-1` certificate envelope.
pub const RECON_CERT_SUBJECT_KIND: &str = "touchstone_reconciliation";

/// Disposition of an R0 site-pack parity difference (§20.9 Parity row).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum ParityDifferenceStatus {
    /// Explained quantitatively (e.g. definition/rounding/basis delta).
    Explained = 1,
    /// Irreproducible / refused; filed as a `DiscrepancyRecord`.
    Refused = 2,
}

impl ParityDifferenceStatus {
    fn from_u8(tag: u8) -> Result<Self, ReconError> {
        match tag {
            1 => Ok(Self::Explained),
            2 => Ok(Self::Refused),
            other => Err(ReconError::CertificateTampered {
                detail: format!("invalid ParityDifferenceStatus tag {other}"),
            }),
        }
    }
}

/// One R0 parity note explaining or refusing a difference against the site's reconciliation pack.
#[derive(Clone, Debug, PartialEq)]
pub struct ParityExplanation {
    /// Metric or line item compared (e.g. `"F2_gold_oz"`).
    pub metric_id: String,
    /// Site pack published value.
    pub site_value: f64,
    /// FrankenMining recomputed value.
    pub recomputed_value: f64,
    /// Explained vs refused status.
    pub status: ParityDifferenceStatus,
    /// Concise provenance explanation or discrepancy reference.
    pub rationale: String,
}

/// Pre- and post-reconciliation balance summary for one interior node (§20.9 Balance row).
#[derive(Clone, Debug, PartialEq)]
pub struct NodeBalanceRecord {
    /// Constraint or node identifier.
    pub node_label: String,
    /// Pre-reconciliation residual `A y - b`.
    pub raw_residual: f64,
    /// Post-reconciliation residual `A x_hat - b`.
    pub reconciled_residual: f64,
}

/// Balance and gross-error summary embedded in the certificate (§20.9 Balance row).
#[derive(Clone, Debug, PartialEq)]
pub struct BalanceCertificateSummary {
    /// Per-node pre/post residuals.
    pub node_residuals: Vec<NodeBalanceRecord>,
    /// Global chi-square statistic.
    pub chi_square: f64,
    /// Degrees of freedom.
    pub degrees_of_freedom: u32,
    /// Upper-tail chi-square p-value.
    pub global_p_value: f64,
    /// Maximum relative closure error (`PO-TOUCH-1`).
    pub max_relative_closure_error: f64,
    /// Gross-error commit disposition (`Admit` vs `RefuseForInvestigation`).
    pub disposition: CommitDisposition,
    /// Top ranked suspect labels (empty if admitted cleanly).
    pub flagged_suspects: Vec<String>,
}

/// One certified F-factor posterior interval with mandatory definition version (`G-TOUCH-4`).
#[derive(Clone, Debug, PartialEq)]
pub struct CertifiedFactorInterval {
    /// Registry key (e.g. `"barrick:kibali:F2_gold_v2"`).
    pub registry_key: String,
    /// Non-empty definition version (`G-TOUCH-4` refuses empty version).
    pub definition_version: String,
    /// Posterior mean.
    pub mean: f64,
    /// Lower interval bound (`<= upper`).
    pub ci_lower: f64,
    /// Upper interval bound (`>= lower`).
    pub ci_upper: f64,
}

/// One certified bias posterior or latent inventory entry (§20.9 Posteriors row).
#[derive(Clone, Debug, PartialEq)]
pub struct CertifiedQuantityInterval {
    /// Canonical quantity or bias ID.
    pub id: String,
    /// Posterior mean.
    pub mean: f64,
    /// Posterior standard deviation (`>= 0`).
    pub std_dev: f64,
    /// Lower central credible bound.
    pub ci_lower: f64,
    /// Upper central credible bound.
    pub ci_upper: f64,
}

/// Posterior quantities, F-factors, bias terms, and latent inventories (§20.9 Posteriors row).
#[derive(Clone, Debug, PartialEq)]
pub struct PosteriorCertificateSummary {
    /// Reconciled node/stream extensive quantities.
    pub node_quantities: Vec<CertifiedQuantityInterval>,
    /// Posterior F-factors carrying mandatory registry keys and definition versions (`G-TOUCH-4`).
    pub f_factors: Vec<CertifiedFactorInterval>,
    /// Node/domain bias posteriors (`BD-152`).
    pub bias_terms: Vec<CertifiedQuantityInterval>,
    /// Latent stockpile/GIC inventories.
    pub latent_inventories: Vec<CertifiedQuantityInterval>,
}

/// Pair refused individual attribution under `G-TOUCH-2`, paired with separating measurement & VOI.
#[derive(Clone, Debug, PartialEq)]
pub struct RefusedPairVoiRecord {
    /// First bias parameter.
    pub first: String,
    /// Second bias parameter.
    pub second: String,
    /// Posterior correlation (`|corr| > 0.80`).
    pub correlation: f64,
    /// Determined linear contrast formula.
    pub determined_contrast: String,
    /// Recommended separating measurement ID or description.
    pub separating_measurement: String,
    /// Expected net VOI in USD.
    pub net_voi_usd: f64,
}

/// Identifiability summary embedded in the certificate (§20.9 Identifiability row).
#[derive(Clone, Debug, PartialEq)]
pub struct IdentifiabilityCertificateSummary {
    /// Observation operator rank.
    pub observation_rank: u32,
    /// Total bias parameter count.
    pub parameter_count: u32,
    /// Pairs refused attribution with `FMINE-REF-104` and their separating measurement VOI.
    pub refused_pairs: Vec<RefusedPairVoiRecord>,
}

/// Score of a pre-registered forecast evaluated against the reconciliation posterior (§20.9).
#[derive(Clone, Debug, PartialEq)]
pub struct RegisteredForecastScore {
    /// Registered forecast ID (`INV-008`).
    pub forecast_id: String,
    /// Hex digest of the pre-period frozen forecast.
    pub forecast_digest_hex: String,
    /// Realized reconciliation posterior mean.
    pub realized_mean: f64,
    /// Whether the forecast credible interval covered the realized posterior mean.
    pub interval_covered: bool,
    /// Continuous Ranked Probability Score (`>= 0`).
    pub crps: f64,
}

/// Human attestation state (§20.9 Attestation row; FrankenMining never signs).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AttestationStatus {
    /// Draft certificate awaiting human QP/metallurgist review.
    DraftUnreviewed,
    /// Human reviewer attested `Reviewed{against}` against the named evidence/pack digest.
    ReviewedAgainst {
        /// Reviewer role/title (never personal PII).
        reviewer_role: String,
        /// Hex digest of the reviewed pack/evidence cut.
        against_digest_hex: String,
    },
}

/// Request to construct a canonical, self-verifying [`ReconciliationCertificate`] (`BD-156`).
#[derive(Clone, Debug, PartialEq)]
pub struct ReconciliationCertificateRequest {
    /// Period identifier (e.g. `"2026Q1"`).
    pub period_id: String,
    /// Site identifier (e.g. `"synthetic_cu_au_01"`).
    pub site_id: String,
    /// Evidence cut identifier.
    pub evidence_cut_id: String,
    /// Measurement error-model version string.
    pub error_model_version: String,
    /// Pinned git commit or build revision.
    pub code_revision: String,
    /// Determinism class (typically `DeterminismClass::DeterministicPerIsa`).
    pub determinism_class: DeterminismClass,
    /// Deterministic seed root (`0` when purely analytical).
    pub seed_root: u64,
    /// Optional `FMT-ANCHOR-1` authority ledger anchor.
    pub ledger_anchor: Option<LedgerAnchor>,
    /// Input artifact closure (`role`, `locator`, `sha256`).
    pub inputs: Vec<InputDigest>,
    /// R0 parity difference explanations/refusals.
    pub parity_notes: Vec<ParityExplanation>,
    /// Reconciled balance & gross-error summary.
    pub balance: BalanceCertificateSummary,
    /// Node quantities, F-factors, biases, and latent inventories.
    pub posteriors: PosteriorCertificateSummary,
    /// Identifiability & Measurement VOI summary.
    pub identifiability: IdentifiabilityCertificateSummary,
    /// Pre-registered forecast scores.
    pub forecast_scores: Vec<RegisteredForecastScore>,
    /// Human attestation state.
    pub attestation: AttestationStatus,
    /// When `true` (committed period close), refuses if `balance.disposition != Admit` (`INV-005`).
    pub require_admitted_balance: bool,
}

/// Canonical `ReconciliationCertificate` (plan §20.9, `BD-156`, `PO-TOUCH-6`, `INV-002`).
///
/// By `INV-002`, all four non-claim flags (`reserve_disclosure_claim`, `execution_authorized`,
/// `geotechnical_signoff`, `regulatory_compliance_claim`) are unconditionally `false`; no method
/// or wire encoding can set them to `true`.
#[derive(Clone, Debug, PartialEq)]
pub struct ReconciliationCertificate {
    pub format_version: u16,
    pub period_id: String,
    pub site_id: String,
    pub evidence_cut_id: String,
    pub error_model_version: String,
    pub code_revision: String,
    pub determinism_class: DeterminismClass,
    pub seed_root: u64,
    pub ledger_anchor: Option<LedgerAnchor>,
    pub inputs: Vec<InputDigest>,
    pub parity_notes: Vec<ParityExplanation>,
    pub balance: BalanceCertificateSummary,
    pub posteriors: PosteriorCertificateSummary,
    pub identifiability: IdentifiabilityCertificateSummary,
    pub forecast_scores: Vec<RegisteredForecastScore>,
    pub attestation: AttestationStatus,
    /// Self-verifying SHA-256 digest of the canonical payload body (bound into `FMT-CERT-1`).
    pub payload_sha256: Digest32,
    /// Replay-stable BLAKE3 digest (`PO-TOUCH-6`).
    pub replay_digest_blake3: Digest32,
}

impl ReconciliationCertificate {
    /// `INV-002`: always `false`.
    #[must_use]
    pub const fn reserve_disclosure_claim(&self) -> bool {
        false
    }

    /// `INV-002`: always `false`.
    #[must_use]
    pub const fn execution_authorized(&self) -> bool {
        false
    }

    /// `INV-002`: always `false`.
    #[must_use]
    pub const fn geotechnical_signoff(&self) -> bool {
        false
    }

    /// `INV-002`: always `false`.
    #[must_use]
    pub const fn regulatory_compliance_claim(&self) -> bool {
        false
    }

    /// Validates all §20.9, `G-TOUCH-4`, and `INV-005` invariants, canonicalizes inputs, and builds
    /// a self-verifying [`ReconciliationCertificate`].
    pub fn build(mut req: ReconciliationCertificateRequest) -> Result<Self, ReconError> {
        if req.period_id.trim().is_empty()
            || req.site_id.trim().is_empty()
            || req.evidence_cut_id.trim().is_empty()
            || req.error_model_version.trim().is_empty()
            || req.code_revision.trim().is_empty()
        {
            return Err(ReconError::invalid(
                "certificate.metadata",
                "period_id, site_id, evidence_cut_id, error_model_version, and code_revision must be non-empty",
            ));
        }
        if req.require_admitted_balance && req.balance.disposition != CommitDisposition::Admit {
            return Err(ReconError::ConservationGrossError {
                detail: format!(
                    "cannot seal committed ReconciliationCertificate for '{}'/'{}': balance disposition is RefuseForInvestigation",
                    req.site_id, req.period_id
                ),
            });
        }
        if !(req.balance.chi_square.is_finite()
            && req.balance.chi_square >= 0.0
            && req.balance.global_p_value.is_finite()
            && (0.0..=1.0).contains(&req.balance.global_p_value)
            && req.balance.max_relative_closure_error.is_finite()
            && req.balance.max_relative_closure_error >= 0.0)
        {
            return Err(ReconError::invalid(
                "certificate.balance",
                "balance statistics must be finite and within valid statistical bounds",
            ));
        }
        // Guard G-TOUCH-4: every F-factor must carry a non-empty registry key, non-empty
        // definition version, and valid finite interval.
        for f in &req.posteriors.f_factors {
            if f.registry_key.trim().is_empty() || f.definition_version.trim().is_empty() {
                return Err(ReconError::MissingDefinitionVersion {
                    detail: format!(
                        "F-factor '{}' must declare non-empty registry_key and definition_version",
                        f.registry_key
                    ),
                });
            }
            if !(f.mean.is_finite()
                && f.ci_lower.is_finite()
                && f.ci_upper.is_finite()
                && f.ci_lower <= f.ci_upper)
            {
                return Err(ReconError::MissingDefinitionVersion {
                    detail: format!(
                        "F-factor '{}' must carry a valid finite interval [ci_lower, ci_upper]",
                        f.registry_key
                    ),
                });
            }
        }
        for q in req
            .posteriors
            .node_quantities
            .iter()
            .chain(&req.posteriors.bias_terms)
            .chain(&req.posteriors.latent_inventories)
        {
            if q.id.trim().is_empty()
                || !(q.mean.is_finite()
                    && q.std_dev.is_finite()
                    && q.std_dev >= 0.0
                    && q.ci_lower.is_finite()
                    && q.ci_upper.is_finite()
                    && q.ci_lower <= q.ci_upper)
            {
                return Err(ReconError::invalid(
                    "certificate.posteriors",
                    format!("quantity '{}' has invalid finite summary or interval", q.id),
                ));
            }
        }
        for fs in &req.forecast_scores {
            if fs.forecast_id.trim().is_empty()
                || fs.forecast_digest_hex.trim().is_empty()
                || !(fs.realized_mean.is_finite() && fs.crps.is_finite() && fs.crps >= 0.0)
            {
                return Err(ReconError::invalid(
                    "certificate.forecast_scores",
                    format!("forecast score '{}' has invalid fields", fs.forecast_id),
                ));
            }
        }
        if let AttestationStatus::ReviewedAgainst {
            reviewer_role,
            against_digest_hex,
        } = &req.attestation
            && (reviewer_role.trim().is_empty() || against_digest_hex.trim().is_empty())
        {
            return Err(ReconError::invalid(
                "certificate.attestation",
                "ReviewedAgainst requires non-empty reviewer_role and against_digest_hex",
            ));
        }

        req.inputs.sort();
        if req.inputs.windows(2).any(|w| w[0] >= w[1]) {
            return Err(ReconError::invalid(
                "certificate.inputs",
                "duplicate input closure entries",
            ));
        }

        let mut cert = Self {
            format_version: FMT_RECON_CERT_V1,
            period_id: req.period_id,
            site_id: req.site_id,
            evidence_cut_id: req.evidence_cut_id,
            error_model_version: req.error_model_version,
            code_revision: req.code_revision,
            determinism_class: req.determinism_class,
            seed_root: req.seed_root,
            ledger_anchor: req.ledger_anchor,
            inputs: req.inputs,
            parity_notes: req.parity_notes,
            balance: req.balance,
            posteriors: req.posteriors,
            identifiability: req.identifiability,
            forecast_scores: req.forecast_scores,
            attestation: req.attestation,
            payload_sha256: Digest32([0u8; 32]),
            replay_digest_blake3: Digest32([0u8; 32]),
        };
        let body = cert.encode_body_bytes();
        cert.payload_sha256 = sha256(&body);
        cert.replay_digest_blake3 = blake3_hash(&body);
        Ok(cert)
    }

    fn encode_body_bytes(&self) -> Vec<u8> {
        let mut enc = Encoder::new_frame(*b"RCRB", self.format_version);
        enc.put_str(&self.period_id);
        enc.put_str(&self.site_id);
        enc.put_str(&self.evidence_cut_id);
        enc.put_str(&self.error_model_version);
        enc.put_str(&self.code_revision);
        enc.put_u8(self.determinism_class.tag());
        enc.put_u64(self.seed_root);
        // INV-002 non-claim flags: hard-coded zero bytes.
        enc.put_u8(0);
        enc.put_u8(0);
        enc.put_u8(0);
        enc.put_u8(0);
        match &self.ledger_anchor {
            Some(anchor) => {
                enc.put_u8(1);
                let ab = anchor.to_bytes();
                enc.put_bytes(&ab);
            }
            None => enc.put_u8(0),
        }
        enc.put_u32(self.inputs.len() as u32);
        for inp in &self.inputs {
            enc.put_str(&inp.role);
            enc.put_str(&inp.locator);
            enc.put_fixed(inp.sha256.as_bytes());
        }
        enc.put_u32(self.parity_notes.len() as u32);
        for p in &self.parity_notes {
            enc.put_str(&p.metric_id);
            enc.put_u64(p.site_value.to_bits());
            enc.put_u64(p.recomputed_value.to_bits());
            enc.put_u8(p.status as u8);
            enc.put_str(&p.rationale);
        }
        enc.put_u32(self.balance.node_residuals.len() as u32);
        for nr in &self.balance.node_residuals {
            enc.put_str(&nr.node_label);
            enc.put_u64(nr.raw_residual.to_bits());
            enc.put_u64(nr.reconciled_residual.to_bits());
        }
        enc.put_u64(self.balance.chi_square.to_bits());
        enc.put_u32(self.balance.degrees_of_freedom);
        enc.put_u64(self.balance.global_p_value.to_bits());
        enc.put_u64(self.balance.max_relative_closure_error.to_bits());
        enc.put_u8(match self.balance.disposition {
            CommitDisposition::Admit => 1,
            CommitDisposition::RefuseForInvestigation => 2,
        });
        enc.put_u32(self.balance.flagged_suspects.len() as u32);
        for s in &self.balance.flagged_suspects {
            enc.put_str(s);
        }
        encode_qty_list(&mut enc, &self.posteriors.node_quantities);
        enc.put_u32(self.posteriors.f_factors.len() as u32);
        for f in &self.posteriors.f_factors {
            enc.put_str(&f.registry_key);
            enc.put_str(&f.definition_version);
            enc.put_u64(f.mean.to_bits());
            enc.put_u64(f.ci_lower.to_bits());
            enc.put_u64(f.ci_upper.to_bits());
        }
        encode_qty_list(&mut enc, &self.posteriors.bias_terms);
        encode_qty_list(&mut enc, &self.posteriors.latent_inventories);
        enc.put_u32(self.identifiability.observation_rank);
        enc.put_u32(self.identifiability.parameter_count);
        enc.put_u32(self.identifiability.refused_pairs.len() as u32);
        for rp in &self.identifiability.refused_pairs {
            enc.put_str(&rp.first);
            enc.put_str(&rp.second);
            enc.put_u64(rp.correlation.to_bits());
            enc.put_str(&rp.determined_contrast);
            enc.put_str(&rp.separating_measurement);
            enc.put_u64(rp.net_voi_usd.to_bits());
        }
        enc.put_u32(self.forecast_scores.len() as u32);
        for fs in &self.forecast_scores {
            enc.put_str(&fs.forecast_id);
            enc.put_str(&fs.forecast_digest_hex);
            enc.put_u64(fs.realized_mean.to_bits());
            enc.put_u8(u8::from(fs.interval_covered));
            enc.put_u64(fs.crps.to_bits());
        }
        match &self.attestation {
            AttestationStatus::DraftUnreviewed => enc.put_u8(0),
            AttestationStatus::ReviewedAgainst {
                reviewer_role,
                against_digest_hex,
            } => {
                enc.put_u8(1);
                enc.put_str(reviewer_role);
                enc.put_str(against_digest_hex);
            }
        }
        enc.finish()
    }

    /// Encodes this certificate into a canonical `FMNE` + `RCRT` + CRC32C frame.
    #[must_use]
    pub fn to_bytes(&self) -> Vec<u8> {
        let body = self.encode_body_bytes();
        let mut enc = Encoder::new_frame(TAG_RECON_CERT, self.format_version);
        enc.put_bytes(&body);
        enc.put_fixed(self.payload_sha256.as_bytes());
        enc.put_fixed(self.replay_digest_blake3.as_bytes());
        enc.finish()
    }

    /// Decodes and cryptographically verifies an `FMNE` + `RCRT` certificate frame, refusing any
    /// CRC32C error, digest mismatch, or non-zero `INV-002` flag with `FMINE-REF-505`.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, ReconError> {
        let mut outer =
            Decoder::open_frame(bytes, TAG_RECON_CERT, 1, FMT_RECON_CERT_V1).map_err(|e| {
                ReconError::CertificateTampered {
                    detail: format!("outer RCRT frame decode failed: {e}"),
                }
            })?;
        let version = outer.version();
        let body = outer
            .get_bytes(16 * 1024 * 1024)
            .map_err(|e| ReconError::CertificateTampered {
                detail: format!("RCRT body read failed: {e}"),
            })?
            .to_vec();
        let stored_sha =
            Digest32(
                outer
                    .get_fixed::<32>()
                    .map_err(|e| ReconError::CertificateTampered {
                        detail: format!("RCRT payload_sha256 read failed: {e}"),
                    })?,
            );
        let stored_blake3 =
            Digest32(
                outer
                    .get_fixed::<32>()
                    .map_err(|e| ReconError::CertificateTampered {
                        detail: format!("RCRT replay_digest_blake3 read failed: {e}"),
                    })?,
            );
        outer
            .finish()
            .map_err(|e| ReconError::CertificateTampered {
                detail: format!("trailing bytes in RCRT frame: {e}"),
            })?;

        let actual_sha = sha256(&body);
        if actual_sha != stored_sha {
            return Err(ReconError::CertificateTampered {
                detail: format!(
                    "payload SHA-256 mismatch: stored {}, actual {}",
                    stored_sha.to_hex(),
                    actual_sha.to_hex()
                ),
            });
        }
        let actual_blake3 = blake3_hash(&body);
        if actual_blake3 != stored_blake3 {
            return Err(ReconError::CertificateTampered {
                detail: format!(
                    "replay BLAKE3 mismatch: stored {}, actual {}",
                    stored_blake3.to_hex(),
                    actual_blake3.to_hex()
                ),
            });
        }

        let mut dec = Decoder::open_frame(&body, *b"RCRB", 1, FMT_RECON_CERT_V1).map_err(|e| {
            ReconError::CertificateTampered {
                detail: format!("inner RCRB body decode failed: {e}"),
            }
        })?;
        let get_s = |d: &mut Decoder<'_>| -> Result<String, ReconError> {
            d.get_str(4096)
                .map(ToString::to_string)
                .map_err(|e| ReconError::CertificateTampered {
                    detail: format!("string decode failed: {e}"),
                })
        };
        let get_u8 = |d: &mut Decoder<'_>| -> Result<u8, ReconError> {
            d.get_u8().map_err(|e| ReconError::CertificateTampered {
                detail: format!("u8 decode failed: {e}"),
            })
        };
        let get_u32 = |d: &mut Decoder<'_>| -> Result<u32, ReconError> {
            d.get_u32().map_err(|e| ReconError::CertificateTampered {
                detail: format!("u32 decode failed: {e}"),
            })
        };
        let get_u64 = |d: &mut Decoder<'_>| -> Result<u64, ReconError> {
            d.get_u64().map_err(|e| ReconError::CertificateTampered {
                detail: format!("u64 decode failed: {e}"),
            })
        };

        let period_id = get_s(&mut dec)?;
        let site_id = get_s(&mut dec)?;
        let evidence_cut_id = get_s(&mut dec)?;
        let error_model_version = get_s(&mut dec)?;
        let code_revision = get_s(&mut dec)?;
        let det_tag = get_u8(&mut dec)?;
        let determinism_class = match det_tag {
            1 => DeterminismClass::BitwisePortable,
            2 => DeterminismClass::DeterministicPerIsa,
            3 => DeterminismClass::Exact,
            4 => DeterminismClass::StatisticalSeeded,
            other => {
                return Err(ReconError::CertificateTampered {
                    detail: format!("invalid DeterminismClass tag {other}"),
                });
            }
        };
        let seed_root = get_u64(&mut dec)?;
        // Verify INV-002 four non-claim flags are strictly 0.
        for flag_idx in 0..4 {
            let b = get_u8(&mut dec)?;
            if b != 0 {
                return Err(ReconError::CertificateTampered {
                    detail: format!("INV-002 violation: non-claim flag #{flag_idx} was {b} != 0"),
                });
            }
        }
        let has_anchor = get_u8(&mut dec)?;
        let ledger_anchor = match has_anchor {
            0 => None,
            1 => {
                let ab = dec
                    .get_bytes(4096)
                    .map_err(|e| ReconError::CertificateTampered {
                        detail: format!("ledger_anchor bytes decode failed: {e}"),
                    })?;
                Some(
                    LedgerAnchor::from_bytes(ab).map_err(|e| ReconError::CertificateTampered {
                        detail: format!("ledger_anchor parse failed: {e}"),
                    })?,
                )
            }
            other => {
                return Err(ReconError::CertificateTampered {
                    detail: format!("invalid ledger_anchor presence tag {other}"),
                });
            }
        };

        let n_inputs = get_u32(&mut dec)? as usize;
        let mut inputs = Vec::with_capacity(n_inputs.min(64));
        for _ in 0..n_inputs {
            let role = get_s(&mut dec)?;
            let locator = get_s(&mut dec)?;
            let digest =
                Digest32(
                    dec.get_fixed::<32>()
                        .map_err(|e| ReconError::CertificateTampered {
                            detail: format!("input digest decode failed: {e}"),
                        })?,
                );
            inputs.push(InputDigest::new(role, locator, digest));
        }

        let n_parity = get_u32(&mut dec)? as usize;
        let mut parity_notes = Vec::with_capacity(n_parity.min(64));
        for _ in 0..n_parity {
            let metric_id = get_s(&mut dec)?;
            let site_value = f64::from_bits(get_u64(&mut dec)?);
            let recomputed_value = f64::from_bits(get_u64(&mut dec)?);
            let status = ParityDifferenceStatus::from_u8(get_u8(&mut dec)?)?;
            let rationale = get_s(&mut dec)?;
            parity_notes.push(ParityExplanation {
                metric_id,
                site_value,
                recomputed_value,
                status,
                rationale,
            });
        }

        let n_res = get_u32(&mut dec)? as usize;
        let mut node_residuals = Vec::with_capacity(n_res.min(64));
        for _ in 0..n_res {
            let node_label = get_s(&mut dec)?;
            let raw_residual = f64::from_bits(get_u64(&mut dec)?);
            let reconciled_residual = f64::from_bits(get_u64(&mut dec)?);
            node_residuals.push(NodeBalanceRecord {
                node_label,
                raw_residual,
                reconciled_residual,
            });
        }
        let chi_square = f64::from_bits(get_u64(&mut dec)?);
        let degrees_of_freedom = get_u32(&mut dec)?;
        let global_p_value = f64::from_bits(get_u64(&mut dec)?);
        let max_relative_closure_error = f64::from_bits(get_u64(&mut dec)?);
        let disposition = match get_u8(&mut dec)? {
            1 => CommitDisposition::Admit,
            2 => CommitDisposition::RefuseForInvestigation,
            other => {
                return Err(ReconError::CertificateTampered {
                    detail: format!("invalid CommitDisposition tag {other}"),
                });
            }
        };
        let n_suspects = get_u32(&mut dec)? as usize;
        let mut flagged_suspects = Vec::with_capacity(n_suspects.min(64));
        for _ in 0..n_suspects {
            flagged_suspects.push(get_s(&mut dec)?);
        }
        let balance = BalanceCertificateSummary {
            node_residuals,
            chi_square,
            degrees_of_freedom,
            global_p_value,
            max_relative_closure_error,
            disposition,
            flagged_suspects,
        };

        let node_quantities = decode_qty_list(&mut dec)?;
        let n_ff = get_u32(&mut dec)? as usize;
        let mut f_factors = Vec::with_capacity(n_ff.min(64));
        for _ in 0..n_ff {
            let registry_key = get_s(&mut dec)?;
            let definition_version = get_s(&mut dec)?;
            let mean = f64::from_bits(get_u64(&mut dec)?);
            let ci_lower = f64::from_bits(get_u64(&mut dec)?);
            let ci_upper = f64::from_bits(get_u64(&mut dec)?);
            f_factors.push(CertifiedFactorInterval {
                registry_key,
                definition_version,
                mean,
                ci_lower,
                ci_upper,
            });
        }
        let bias_terms = decode_qty_list(&mut dec)?;
        let latent_inventories = decode_qty_list(&mut dec)?;
        let posteriors = PosteriorCertificateSummary {
            node_quantities,
            f_factors,
            bias_terms,
            latent_inventories,
        };

        let observation_rank = get_u32(&mut dec)?;
        let parameter_count = get_u32(&mut dec)?;
        let n_rp = get_u32(&mut dec)? as usize;
        let mut refused_pairs = Vec::with_capacity(n_rp.min(64));
        for _ in 0..n_rp {
            let first = get_s(&mut dec)?;
            let second = get_s(&mut dec)?;
            let correlation = f64::from_bits(get_u64(&mut dec)?);
            let determined_contrast = get_s(&mut dec)?;
            let separating_measurement = get_s(&mut dec)?;
            let net_voi_usd = f64::from_bits(get_u64(&mut dec)?);
            refused_pairs.push(RefusedPairVoiRecord {
                first,
                second,
                correlation,
                determined_contrast,
                separating_measurement,
                net_voi_usd,
            });
        }
        let identifiability = IdentifiabilityCertificateSummary {
            observation_rank,
            parameter_count,
            refused_pairs,
        };

        let n_fs = get_u32(&mut dec)? as usize;
        let mut forecast_scores = Vec::with_capacity(n_fs.min(64));
        for _ in 0..n_fs {
            let forecast_id = get_s(&mut dec)?;
            let forecast_digest_hex = get_s(&mut dec)?;
            let realized_mean = f64::from_bits(get_u64(&mut dec)?);
            let interval_covered = match get_u8(&mut dec)? {
                0 => false,
                1 => true,
                other => {
                    return Err(ReconError::CertificateTampered {
                        detail: format!("invalid boolean tag {other}"),
                    });
                }
            };
            let crps = f64::from_bits(get_u64(&mut dec)?);
            forecast_scores.push(RegisteredForecastScore {
                forecast_id,
                forecast_digest_hex,
                realized_mean,
                interval_covered,
                crps,
            });
        }

        let attestation = match get_u8(&mut dec)? {
            0 => AttestationStatus::DraftUnreviewed,
            1 => AttestationStatus::ReviewedAgainst {
                reviewer_role: get_s(&mut dec)?,
                against_digest_hex: get_s(&mut dec)?,
            },
            other => {
                return Err(ReconError::CertificateTampered {
                    detail: format!("invalid AttestationStatus tag {other}"),
                });
            }
        };
        dec.finish().map_err(|e| ReconError::CertificateTampered {
            detail: format!("trailing bytes in RCRB body: {e}"),
        })?;

        Ok(Self {
            format_version: version,
            period_id,
            site_id,
            evidence_cut_id,
            error_model_version,
            code_revision,
            determinism_class,
            seed_root,
            ledger_anchor,
            inputs,
            parity_notes,
            balance,
            posteriors,
            identifiability,
            forecast_scores,
            attestation,
            payload_sha256: stored_sha,
            replay_digest_blake3: stored_blake3,
        })
    }

    /// Wraps this certificate's verified `payload_sha256` and input closure inside a canonical
    /// `FMT-CERT-1` [`Certificate`] (`BD-096`) verifiable offline by `fmine-verify cert` (`BD-156`).
    pub fn to_fmt_cert_1(&self) -> Result<Certificate, ReconError> {
        Certificate::build(
            RECON_CERT_SUBJECT_KIND,
            self.code_revision.clone(),
            self.determinism_class,
            self.seed_root,
            self.inputs.clone(),
            self.payload_sha256,
            self.ledger_anchor.clone(),
        )
        .map_err(|e| ReconError::invalid("certificate.fmt_cert_1", e.to_string()))
    }
}

fn encode_qty_list(enc: &mut Encoder, items: &[CertifiedQuantityInterval]) {
    enc.put_u32(items.len() as u32);
    for q in items {
        enc.put_str(&q.id);
        enc.put_u64(q.mean.to_bits());
        enc.put_u64(q.std_dev.to_bits());
        enc.put_u64(q.ci_lower.to_bits());
        enc.put_u64(q.ci_upper.to_bits());
    }
}

fn decode_qty_list(dec: &mut Decoder<'_>) -> Result<Vec<CertifiedQuantityInterval>, ReconError> {
    let n = dec.get_u32().map_err(|e| ReconError::CertificateTampered {
        detail: format!("quantity count decode failed: {e}"),
    })? as usize;
    let mut out = Vec::with_capacity(n.min(64));
    for _ in 0..n {
        let id = dec.get_str(4096).map(ToString::to_string).map_err(|e| {
            ReconError::CertificateTampered {
                detail: format!("quantity id decode failed: {e}"),
            }
        })?;
        let mean = f64::from_bits(dec.get_u64().map_err(|e| ReconError::CertificateTampered {
            detail: format!("quantity mean decode failed: {e}"),
        })?);
        let std_dev =
            f64::from_bits(dec.get_u64().map_err(|e| ReconError::CertificateTampered {
                detail: format!("quantity std_dev decode failed: {e}"),
            })?);
        let ci_lower =
            f64::from_bits(dec.get_u64().map_err(|e| ReconError::CertificateTampered {
                detail: format!("quantity ci_lower decode failed: {e}"),
            })?);
        let ci_upper =
            f64::from_bits(dec.get_u64().map_err(|e| ReconError::CertificateTampered {
                detail: format!("quantity ci_upper decode failed: {e}"),
            })?);
        out.push(CertifiedQuantityInterval {
            id,
            mean,
            std_dev,
            ci_lower,
            ci_upper,
        });
    }
    Ok(out)
}
