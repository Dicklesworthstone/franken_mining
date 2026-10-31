//! Typed runtime object identifiers and canonical prefixes (§0.3, BD-016).
//! Determinism: BitwisePortable

use core::fmt;
use std::str::FromStr;

use crate::hash::blake3_hash;

/// Canonical runtime object prefix (§0.3, BD-016).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PrefixKind {
    /// `clm_` — Universal epistemic `Claim` (§8.1).
    Claim,
    /// `anc_` — `LedgerAnchor` high-water mark (`FMT-ANCHOR-1`).
    Anchor,
    /// `evd_` — Evidence record in `CELL-EVD`.
    Evidence,
    /// `dec_` — `DecisionCard` or `CELL-DEC` entry (§22).
    Decision,
    /// `ref_` — Typed `Refusal` instance (§8.4).
    Refusal,
    /// `for_` — Frozen `ForecastRegistration` (§10.1).
    Forecast,
    /// `cal_` — `CalibrationRecord` (§10.3).
    Calibration,
    /// `dis_` — Public or site `DiscrepancyEntry` (§21.6).
    Discrepancy,
    /// `cell_` — Authority `Cell` identifier (§26.2).
    Cell,
    /// `mod_` — Registered `ModelCard` (§13.3).
    Model,
    /// `crd_` — Process `CorrelationCard` (§17.3).
    CorrelationCard,
    /// `ob_` — Durable `Obligation` (`OB-*`, §26.7).
    Obligation,
    /// `lot_` — Concentrate or doré `Lot` (§17.6).
    Lot,
    /// `hole_` — Drillhole `HoleId` (§12.1).
    Hole,
    /// `dom_` — Estimation or geometallurgical domain (§13.2).
    Domain,
    /// `scen_` — Shared stochastic scenario index (§23.2).
    Scenario,
}

impl PrefixKind {
    /// Canonical lowercase prefix string including trailing underscore.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Claim => "clm_",
            Self::Anchor => "anc_",
            Self::Evidence => "evd_",
            Self::Decision => "dec_",
            Self::Refusal => "ref_",
            Self::Forecast => "for_",
            Self::Calibration => "cal_",
            Self::Discrepancy => "dis_",
            Self::Cell => "cell_",
            Self::Model => "mod_",
            Self::CorrelationCard => "crd_",
            Self::Obligation => "ob_",
            Self::Lot => "lot_",
            Self::Hole => "hole_",
            Self::Domain => "dom_",
            Self::Scenario => "scen_",
        }
    }

    /// Parse a prefix from the start of an identifier string.
    #[must_use]
    pub fn split_prefix(s: &str) -> Option<(Self, &str)> {
        const ALL: [PrefixKind; 16] = [
            PrefixKind::Claim,
            PrefixKind::Anchor,
            PrefixKind::Evidence,
            PrefixKind::Decision,
            PrefixKind::Refusal,
            PrefixKind::Forecast,
            PrefixKind::Calibration,
            PrefixKind::Discrepancy,
            PrefixKind::Cell,
            PrefixKind::Model,
            PrefixKind::CorrelationCard,
            PrefixKind::Obligation,
            PrefixKind::Lot,
            PrefixKind::Hole,
            PrefixKind::Domain,
            PrefixKind::Scenario,
        ];
        for kind in ALL {
            if let Some(rest) = s.strip_prefix(kind.as_str()) {
                return Some((kind, rest));
            }
        }
        None
    }
}

/// Content-addressed or deterministic 128-bit runtime object ID with typed prefix (`BD-016`).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ObjectId {
    /// Object namespace prefix.
    pub kind: PrefixKind,
    /// First 16 bytes (128 bits, 32 hex chars) of the canonical BLAKE3 digest.
    pub bytes: [u8; 16],
}

/// Error returned when parsing an [`ObjectId`] from text (`BD-016`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum IdParseError {
    /// No known `PrefixKind` matched.
    UnknownPrefix,
    /// Hex payload was not exactly 32 lowercase hex characters.
    InvalidHexLength(usize),
    /// Invalid character in hex payload.
    InvalidHexChar(char),
}

impl fmt::Display for IdParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownPrefix => write!(f, "unknown object ID prefix"),
            Self::InvalidHexLength(n) => {
                write!(f, "expected 32 lowercase hex chars after prefix, got {n}")
            }
            Self::InvalidHexChar(c) => write!(f, "invalid lowercase hex character: {c:?}"),
        }
    }
}

impl std::error::Error for IdParseError {}

impl ObjectId {
    /// Derive a deterministic [`ObjectId`] from a [`PrefixKind`] and canonical payload bytes via BLAKE3.
    #[must_use]
    pub fn from_canonical_bytes(kind: PrefixKind, payload: &[u8]) -> Self {
        let mut buf = Vec::with_capacity(kind.as_str().len() + payload.len());
        buf.extend_from_slice(kind.as_str().as_bytes());
        buf.extend_from_slice(payload);
        let digest = blake3_hash(&buf);
        let mut bytes = [0u8; 16];
        bytes.copy_from_slice(&digest.as_bytes()[..16]);
        Self { kind, bytes }
    }

    /// Construct directly from raw 16 bytes.
    #[must_use]
    pub const fn from_raw(kind: PrefixKind, bytes: [u8; 16]) -> Self {
        Self { kind, bytes }
    }
}

impl fmt::Display for ObjectId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.kind.as_str())?;
        for b in &self.bytes {
            write!(f, "{b:02x}")?;
        }
        Ok(())
    }
}

impl fmt::Debug for ObjectId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ObjectId({self})")
    }
}

impl FromStr for ObjectId {
    type Err = IdParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (kind, hex) = PrefixKind::split_prefix(s).ok_or(IdParseError::UnknownPrefix)?;
        if hex.len() != 32 {
            return Err(IdParseError::InvalidHexLength(hex.len()));
        }
        let mut bytes = [0u8; 16];
        let hex_bytes = hex.as_bytes();
        for (i, slot) in bytes.iter_mut().enumerate() {
            let hi = decode_lower_nibble(hex_bytes[2 * i] as char)?;
            let lo = decode_lower_nibble(hex_bytes[2 * i + 1] as char)?;
            *slot = (hi << 4) | lo;
        }
        Ok(Self { kind, bytes })
    }
}

fn decode_lower_nibble(c: char) -> Result<u8, IdParseError> {
    match c {
        '0'..='9' => Ok((c as u8) - b'0'),
        'a'..='f' => Ok((c as u8) - b'a' + 10),
        _ => Err(IdParseError::InvalidHexChar(c)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_prefixes_roundtrip_and_order() {
        let kinds = [
            PrefixKind::Claim,
            PrefixKind::Anchor,
            PrefixKind::Evidence,
            PrefixKind::Decision,
            PrefixKind::Refusal,
            PrefixKind::Forecast,
            PrefixKind::Calibration,
            PrefixKind::Discrepancy,
            PrefixKind::Cell,
            PrefixKind::Model,
            PrefixKind::CorrelationCard,
            PrefixKind::Obligation,
            PrefixKind::Lot,
            PrefixKind::Hole,
            PrefixKind::Domain,
            PrefixKind::Scenario,
        ];
        for (idx, kind) in kinds.iter().copied().enumerate() {
            let id = ObjectId::from_canonical_bytes(kind, &[idx as u8; 8]);
            let s = id.to_string();
            assert!(s.starts_with(kind.as_str()));
            let parsed = ObjectId::from_str(&s).unwrap();
            assert_eq!(id, parsed);
        }
    }

    /// BD-016: seeded, so a failure reproduces. Every kind and random bytes survive display then parse, and random
    /// text is refused with a typed error, never a panic.
    #[test]
    fn test_id_display_and_parse_property_seeded() {
        use crate::rand::{PhiloxStream, RngKey};
        let kinds = [
            PrefixKind::Claim,
            PrefixKind::Anchor,
            PrefixKind::Evidence,
            PrefixKind::Decision,
            PrefixKind::Refusal,
            PrefixKind::Forecast,
            PrefixKind::Calibration,
            PrefixKind::Discrepancy,
            PrefixKind::Cell,
            PrefixKind::Model,
            PrefixKind::CorrelationCard,
            PrefixKind::Obligation,
            PrefixKind::Lot,
            PrefixKind::Hole,
            PrefixKind::Domain,
            PrefixKind::Scenario,
        ];
        let mut rng = PhiloxStream::new(RngKey {
            seed_root: 16,
            partition: 0,
            domain: 0,
            var_group: 0,
            realization: 0,
            brick: 0,
        });
        let mut refused = 0;
        for _ in 0..10_000 {
            let kind = kinds[(rng.next_u64() % kinds.len() as u64) as usize];
            let len = (rng.next_u64() % 40) as usize;
            let bytes: Vec<u8> = (0..len).map(|_| rng.next_u64() as u8).collect();
            let id = ObjectId::from_canonical_bytes(kind, &bytes);
            assert_eq!(ObjectId::from_str(&id.to_string()), Ok(id));

            let text: String = (0..(rng.next_u64() % 45))
                .map(|_| char::from(b' ' + (rng.next_u64() % 95) as u8))
                .collect();
            if let Ok(parsed) = ObjectId::from_str(&text) {
                assert_eq!(parsed.to_string(), text);
            } else {
                refused += 1;
            }
        }
        assert_eq!(refused, 10_000, "random text should not name a real id");
    }

    #[test]
    fn test_invalid_ids_refused() {
        assert_eq!(
            ObjectId::from_str("bad_0123456789abcdef0123456789abcdef"),
            Err(IdParseError::UnknownPrefix)
        );
        assert_eq!(
            ObjectId::from_str("clm_0123"),
            Err(IdParseError::InvalidHexLength(4))
        );
        assert_eq!(
            ObjectId::from_str("clm_0123456789ABCDEF0123456789abcdef"),
            Err(IdParseError::InvalidHexChar('A'))
        );
    }
}
