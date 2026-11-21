//! `FMT-CERT-1` deterministic execution certificates (`BD-096`, plan §26.5, §33.6, `PO-BED-9`).
//! Determinism: BitwisePortable
//!
//! Every `FMT-CERT-1` certificate binds:
//! - `subject_kind` (what produced the output, e.g. `"public_assurance"`, `"pit_solution"`);
//! - `code_revision` (pinned git commit or build revision);
//! - `determinism_class` (`BitwisePortable`, `DeterministicPerIsa`, `Exact`, `StatisticalSeeded`);
//! - `seed_root` (64-bit deterministic seed root);
//! - `inputs` (strictly ascending canonical closure of `(role, locator, sha256)`);
//! - `output_digest` (SHA-256 of the certified output artifact);
//! - `ledger_anchor` (optional `FMT-ANCHOR-1` authority high-water mark, v1+; omitted in N-1 v0);
//! - `certificate_digest` (self-verifying `SHA-256` of the canonical body).
//!
//! Wire format: `FMNE` + `CERT` + `version: u16` + canonical fields + `certificate_digest` + CRC32C.

use core::fmt;

use crate::anchor::LedgerAnchor;
use crate::codec::{CodecError, Decoder, Encoder};
use crate::hash::{Digest32, sha256};

/// Format tag for `FMT-CERT-1` certificates (`FMNE` + `CERT`).
pub const TAG_CERT: [u8; 4] = *b"CERT";

/// Internal domain-separation tag for canonical body hashing (`CRTB`).
const TAG_CERT_BODY: [u8; 4] = *b"CRTB";

/// Current schema version for `FMT-CERT-1`.
pub const FMT_CERT_V1: u16 = 1;

/// Minimum supported schema version (`N-1` compatibility: v0 omits `ledger_anchor`).
pub const FMT_CERT_MIN_VERSION: u16 = 0;

/// Maximum length of string metadata fields (`subject_kind`, `code_revision`, `role`, `locator`).
pub const MAX_CERT_STR_BYTES: usize = 512;

/// Maximum number of input closure entries per certificate.
pub const MAX_CERT_INPUTS: usize = 4096;

/// Maximum embedded `FMT-ANCHOR-1` frame length inside a certificate.
pub const MAX_CERT_ANCHOR_BYTES: usize = 4096;

/// Determinism class declared by a certified computation (plan §29.2, `PO-BED-9`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum DeterminismClass {
    /// Bit-identical across x86_64, aarch64, and wasm32 (`PO-NUM-2`).
    BitwisePortable = 1,
    /// Bit-identical on a pinned ISA / toolchain revision.
    DeterministicPerIsa = 2,
    /// Exact integer / rational arithmetic (bit-identical everywhere).
    Exact = 3,
    /// Seeded stochastic simulation (`Philox4x32-10` stream keyed by `seed_root`).
    StatisticalSeeded = 4,
}

impl DeterminismClass {
    /// Stable wire tag (`1..=4`).
    #[must_use]
    pub const fn tag(self) -> u8 {
        self as u8
    }

    /// Stable canonical string name matching plan §29.2.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::BitwisePortable => "BitwisePortable",
            Self::DeterministicPerIsa => "DeterministicPerIsa",
            Self::Exact => "Exact",
            Self::StatisticalSeeded => "StatisticalSeeded",
        }
    }

    fn from_u8(tag: u8) -> Result<Self, CertError> {
        match tag {
            1 => Ok(Self::BitwisePortable),
            2 => Ok(Self::DeterministicPerIsa),
            3 => Ok(Self::Exact),
            4 => Ok(Self::StatisticalSeeded),
            other => Err(CertError::InvalidDeterminismClass(other)),
        }
    }
}

/// One element of a certificate's content-addressed input closure (`role`, `locator`, `sha256`).
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct InputDigest {
    /// Semantic role of the input (e.g. `"workbook"`, `"registry"`, `"block_model"`).
    pub role: String,
    /// Relative path, URI, or canonical locator for the input artifact.
    pub locator: String,
    /// SHA-256 digest of the input bytes.
    pub sha256: Digest32,
}

impl InputDigest {
    /// Construct a new [`InputDigest`].
    #[must_use]
    pub fn new(role: impl Into<String>, locator: impl Into<String>, sha256: Digest32) -> Self {
        Self {
            role: role.into(),
            locator: locator.into(),
            sha256,
        }
    }

    fn validate(&self) -> Result<(), CertError> {
        if self.role.is_empty() || self.role.len() > MAX_CERT_STR_BYTES {
            return Err(CertError::InvalidField {
                field: "input.role",
                detail: format!(
                    "length {} outside 1..={MAX_CERT_STR_BYTES}",
                    self.role.len()
                ),
            });
        }
        if self.locator.is_empty() || self.locator.len() > MAX_CERT_STR_BYTES {
            return Err(CertError::InvalidField {
                field: "input.locator",
                detail: format!(
                    "length {} outside 1..={MAX_CERT_STR_BYTES}",
                    self.locator.len()
                ),
            });
        }
        Ok(())
    }
}

/// Typed errors returned while constructing, decoding, or verifying an `FMT-CERT-1` certificate.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CertError {
    /// Underlying `FMNE` framing or CRC32C failure.
    Codec(CodecError),
    /// Unknown wire tag for [`DeterminismClass`].
    InvalidDeterminismClass(u8),
    /// A required metadata field was empty, oversized, or incompatible with the format version.
    InvalidField { field: &'static str, detail: String },
    /// Input closure entries were not strictly ascending (out-of-order or duplicated).
    NonCanonicalInputs,
    /// Recomputed `SHA-256(canonical_body)` did not equal stored `certificate_digest`.
    DigestMismatch {
        expected: Digest32,
        actual: Digest32,
    },
}

impl From<CodecError> for CertError {
    fn from(value: CodecError) -> Self {
        Self::Codec(value)
    }
}

impl fmt::Display for CertError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Codec(e) => write!(f, "certificate codec error: {e}"),
            Self::InvalidDeterminismClass(tag) => {
                write!(f, "invalid DeterminismClass wire tag {tag}")
            }
            Self::InvalidField { field, detail } => {
                write!(f, "invalid certificate field {field}: {detail}")
            }
            Self::NonCanonicalInputs => {
                write!(
                    f,
                    "certificate inputs are not in strictly ascending canonical order"
                )
            }
            Self::DigestMismatch { expected, actual } => write!(
                f,
                "certificate digest mismatch: stored {}, recomputed {}",
                expected.to_hex(),
                actual.to_hex()
            ),
        }
    }
}

impl std::error::Error for CertError {}

/// Canonical `FMT-CERT-1` result certificate (`BD-096`, plan §26.5, `PO-BED-9`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Certificate {
    /// Format version (`1` for `FMT-CERT-1`; `0` for N-1 frames without `ledger_anchor`).
    pub format_version: u16,
    /// Kind of certified computation (e.g. `"public_assurance"`, `"pit_solution"`).
    pub subject_kind: String,
    /// Pinned git commit or build revision that produced the output.
    pub code_revision: String,
    /// Declared determinism class (plan §29.2).
    pub determinism_class: DeterminismClass,
    /// Explicit deterministic seed root (`0` when unseeded / exact).
    pub seed_root: u64,
    /// Strictly ascending canonical closure of input artifacts.
    pub inputs: Vec<InputDigest>,
    /// SHA-256 digest of the certified output payload.
    pub output_digest: Digest32,
    /// Optional authority high-water mark (`v1+` only; `None` in `v0`).
    pub ledger_anchor: Option<LedgerAnchor>,
    /// Self-verifying `SHA-256` digest of the canonical certificate body.
    pub certificate_digest: Digest32,
}

impl Certificate {
    /// Build a canonical `FMT-CERT-1` (`v1`) certificate, sorting `inputs` into strict ascending
    /// order and computing `certificate_digest`. Refuses empty metadata or duplicate inputs.
    pub fn build(
        subject_kind: impl Into<String>,
        code_revision: impl Into<String>,
        determinism_class: DeterminismClass,
        seed_root: u64,
        mut inputs: Vec<InputDigest>,
        output_digest: Digest32,
        ledger_anchor: Option<LedgerAnchor>,
    ) -> Result<Self, CertError> {
        inputs.sort();
        let mut cert = Self {
            format_version: FMT_CERT_V1,
            subject_kind: subject_kind.into(),
            code_revision: code_revision.into(),
            determinism_class,
            seed_root,
            inputs,
            output_digest,
            ledger_anchor,
            certificate_digest: Digest32([0u8; 32]),
        };
        cert.validate_fields()?;
        cert.certificate_digest = cert.compute_digest()?;
        Ok(cert)
    }

    fn validate_fields(&self) -> Result<(), CertError> {
        if self.format_version > FMT_CERT_V1 {
            return Err(CertError::Codec(CodecError::UnsupportedVersion {
                actual: self.format_version,
                min_supported: FMT_CERT_MIN_VERSION,
                max_supported: FMT_CERT_V1,
            }));
        }
        if self.subject_kind.is_empty() || self.subject_kind.len() > MAX_CERT_STR_BYTES {
            return Err(CertError::InvalidField {
                field: "subject_kind",
                detail: format!(
                    "length {} outside 1..={MAX_CERT_STR_BYTES}",
                    self.subject_kind.len()
                ),
            });
        }
        if self.code_revision.is_empty() || self.code_revision.len() > MAX_CERT_STR_BYTES {
            return Err(CertError::InvalidField {
                field: "code_revision",
                detail: format!(
                    "length {} outside 1..={MAX_CERT_STR_BYTES}",
                    self.code_revision.len()
                ),
            });
        }
        if self.inputs.len() > MAX_CERT_INPUTS {
            return Err(CertError::Codec(CodecError::LengthOutOfBounds {
                len: self.inputs.len(),
                max: MAX_CERT_INPUTS,
            }));
        }
        for inp in &self.inputs {
            inp.validate()?;
        }
        if self.inputs.windows(2).any(|w| w[0] >= w[1]) {
            return Err(CertError::NonCanonicalInputs);
        }
        if self.format_version == 0 && self.ledger_anchor.is_some() {
            return Err(CertError::InvalidField {
                field: "ledger_anchor",
                detail: "FMT-CERT v0 does not carry a ledger_anchor".to_string(),
            });
        }
        Ok(())
    }

    fn encode_body_fields(&self, enc: &mut Encoder) -> Result<(), CertError> {
        enc.put_str(&self.subject_kind);
        enc.put_str(&self.code_revision);
        enc.put_u8(self.determinism_class.tag());
        enc.put_u64(self.seed_root);
        enc.put_canonical_seq(&self.inputs, |e, inp| {
            e.put_str(&inp.role);
            e.put_str(&inp.locator);
            e.put_fixed(inp.sha256.as_bytes());
        })?;
        enc.put_fixed(self.output_digest.as_bytes());
        if self.format_version >= 1 {
            match &self.ledger_anchor {
                None => enc.put_u8(0),
                Some(anchor) => {
                    let anchor_bytes = anchor.to_bytes();
                    if anchor_bytes.len() > MAX_CERT_ANCHOR_BYTES {
                        return Err(CertError::Codec(CodecError::LengthOutOfBounds {
                            len: anchor_bytes.len(),
                            max: MAX_CERT_ANCHOR_BYTES,
                        }));
                    }
                    enc.put_u8(1);
                    enc.put_bytes(&anchor_bytes);
                }
            }
        }
        Ok(())
    }

    /// Recompute the canonical `SHA-256` digest over all fields except `certificate_digest`.
    pub fn compute_digest(&self) -> Result<Digest32, CertError> {
        self.validate_fields()?;
        let mut enc = Encoder::new_frame(TAG_CERT_BODY, self.format_version);
        self.encode_body_fields(&mut enc)?;
        Ok(sha256(&enc.finish()))
    }

    /// Verify field invariants, canonical input ordering, and exact `certificate_digest` binding.
    pub fn verify(&self) -> Result<(), CertError> {
        let actual = self.compute_digest()?;
        if actual != self.certificate_digest {
            return Err(CertError::DigestMismatch {
                expected: self.certificate_digest,
                actual,
            });
        }
        Ok(())
    }

    /// Encode this verified certificate into a canonical `FMT-CERT-1` frame (`FMNE` + `CERT` + CRC32C).
    pub fn to_bytes(&self) -> Result<Vec<u8>, CertError> {
        self.verify()?;
        let mut enc = Encoder::new_frame(TAG_CERT, self.format_version);
        self.encode_body_fields(&mut enc)?;
        enc.put_fixed(self.certificate_digest.as_bytes());
        Ok(enc.finish())
    }

    /// Decode and verify an `FMT-CERT-1` frame (supporting both v1 and N-1 v0 frames).
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, CertError> {
        let mut dec = Decoder::open_frame(bytes, TAG_CERT, FMT_CERT_MIN_VERSION, FMT_CERT_V1)?;
        let format_version = dec.version();
        let subject_kind = dec.get_str(MAX_CERT_STR_BYTES)?.to_string();
        let code_revision = dec.get_str(MAX_CERT_STR_BYTES)?.to_string();
        let determinism_class = DeterminismClass::from_u8(dec.get_u8()?)?;
        let seed_root = dec.get_u64()?;
        let input_count = dec.get_u32()? as usize;
        if input_count > MAX_CERT_INPUTS {
            return Err(CertError::Codec(CodecError::LengthOutOfBounds {
                len: input_count,
                max: MAX_CERT_INPUTS,
            }));
        }
        let mut inputs = Vec::with_capacity(input_count.min(64));
        for _ in 0..input_count {
            let role = dec.get_str(MAX_CERT_STR_BYTES)?.to_string();
            let locator = dec.get_str(MAX_CERT_STR_BYTES)?.to_string();
            let digest = Digest32(dec.get_fixed::<32>()?);
            let item = InputDigest {
                role,
                locator,
                sha256: digest,
            };
            if let Some(prev) = inputs.last()
                && prev >= &item
            {
                return Err(CertError::NonCanonicalInputs);
            }
            inputs.push(item);
        }
        let output_digest = Digest32(dec.get_fixed::<32>()?);
        let ledger_anchor = if format_version >= 1 {
            match dec.get_u8()? {
                0 => None,
                1 => {
                    let raw = dec.get_bytes(MAX_CERT_ANCHOR_BYTES)?;
                    Some(LedgerAnchor::from_bytes(raw)?)
                }
                other => {
                    return Err(CertError::InvalidField {
                        field: "ledger_anchor_present",
                        detail: format!("expected 0 or 1, got {other}"),
                    });
                }
            }
        } else {
            None
        };
        let certificate_digest = Digest32(dec.get_fixed::<32>()?);
        dec.finish()?;

        let cert = Self {
            format_version,
            subject_kind,
            code_revision,
            determinism_class,
            seed_root,
            inputs,
            output_digest,
            ledger_anchor,
            certificate_digest,
        };
        cert.verify()?;
        Ok(cert)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::anchor::FMT_ANCHOR_V1;
    use crate::hash::blake3_hash;

    fn sample_anchor() -> LedgerAnchor {
        LedgerAnchor {
            format_version: FMT_ANCHOR_V1,
            cell_id: "CELL-PUBLIC(barrick)".to_string(),
            ledger_epoch: 1,
            commit_sequence: 7,
            adapter_registry_epoch: 1,
            schema_epoch: 1,
            policy_epoch: 2,
            privacy_epoch: 1,
            residency_epoch: 1,
            state_root: blake3_hash(b"state-root-7"),
        }
    }

    #[test]
    fn test_fmt_cert_1_roundtrip_n_minus_1_and_tamper_refusal() {
        let inputs = vec![
            InputDigest::new("workbook", "barrick/2026Q2.xlsx", sha256(b"wb-q2")),
            InputDigest::new(
                "registry",
                "registries/sources/barrick.toml",
                sha256(b"reg"),
            ),
        ];
        let cert = Certificate::build(
            "public_assurance",
            "3274dc2",
            DeterminismClass::BitwisePortable,
            42,
            inputs,
            sha256(b"assurance-report-payload"),
            Some(sample_anchor()),
        )
        .expect("build cert");

        // Inputs were canonically sorted ascending by (role, locator, sha256).
        assert_eq!(cert.inputs[0].role, "registry");
        assert_eq!(cert.inputs[1].role, "workbook");

        // Roundtrip byte-for-byte.
        let bytes = cert.to_bytes().expect("encode v1");
        let decoded = Certificate::from_bytes(&bytes).expect("decode v1");
        assert_eq!(cert, decoded);
        assert_eq!(decoded.to_bytes().expect("re-encode"), bytes);

        // Single-bit flip in EVERY byte of the serialized frame must be refused.
        for pos in 0..bytes.len() {
            let mut tampered = bytes.clone();
            tampered[pos] ^= 0x01;
            assert!(
                Certificate::from_bytes(&tampered).is_err(),
                "bit flip at byte {pos} should have been refused"
            );
        }

        // Mutating an in-memory field invalidates self-verification.
        let mut mutated = cert.clone();
        mutated.seed_root = 43;
        assert!(matches!(
            mutated.verify(),
            Err(CertError::DigestMismatch { .. })
        ));

        // Duplicate inputs are refused at build time.
        let dup_err = Certificate::build(
            "public_assurance",
            "3274dc2",
            DeterminismClass::Exact,
            0,
            vec![
                InputDigest::new("registry", "a.toml", sha256(b"same")),
                InputDigest::new("registry", "a.toml", sha256(b"same")),
            ],
            sha256(b"out"),
            None,
        )
        .unwrap_err();
        assert_eq!(dup_err, CertError::NonCanonicalInputs);

        // N-1 reader compatibility: v0 frame without ledger_anchor round-trips cleanly.
        let mut v0 = Certificate {
            format_version: 0,
            subject_kind: "pit_solution".to_string(),
            code_revision: "3274dc2".to_string(),
            determinism_class: DeterminismClass::Exact,
            seed_root: 0,
            inputs: vec![InputDigest::new("upit", "pit.upit", sha256(b"pit"))],
            output_digest: sha256(b"pit-out"),
            ledger_anchor: None,
            certificate_digest: Digest32([0u8; 32]),
        };
        v0.certificate_digest = v0.compute_digest().expect("v0 digest");
        let v0_bytes = v0.to_bytes().expect("encode v0");
        let v0_decoded = Certificate::from_bytes(&v0_bytes).expect("decode v0");
        assert_eq!(v0, v0_decoded);
        assert_eq!(v0_decoded.format_version, 0);
        assert!(v0_decoded.ledger_anchor.is_none());
    }
}
