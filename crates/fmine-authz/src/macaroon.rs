//! Determinism: BitwisePortable
//!
//! First-party HMAC-SHA256 macaroons with explicit mitigations for plan §31.4 defects S3(b) and S3(c)
//! (`BD-092`, `INT-AS-SEC-001 (b), (c)`):
//! - S3(b): [`FmKeyRing`] refuses any all-zero active or retired key, always installs a distinct
//!   non-zero retired key, and [`FmKeyRing::verify`] rejects an all-zero signature before evaluating HMAC.
//! - S3(c): First-party caveats only. Any attempt to attach or decode a third-party caveat is refused.

use std::hint::black_box;

use fmine_core::codec::{Decoder, Encoder};
use fmine_core::hash::sha256;

use crate::AuthzError;

/// Inner frame tag for canonical `FMT-MAC-1` macaroon serialization (`FMNE` + `MAC1` + CRC32C).
pub const MAC_FORMAT_MAGIC: [u8; 4] = *b"MAC1";
const MAC_FORMAT_VERSION: u16 = 1;
const MAX_CAVEATS: usize = 64;
const MAX_STR_LEN: usize = 256;
const CAVEAT_CLASS_FIRST_PARTY: u8 = 1;
const CAVEAT_CLASS_THIRD_PARTY: u8 = 2;

/// Identifier of a signing key inside [`FmKeyRing`].
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct KeyId(String);

impl KeyId {
    /// Creates a non-empty ASCII key identifier.
    pub fn new(id: impl Into<String>) -> Result<Self, AuthzError> {
        let s = id.into();
        if s.is_empty()
            || s.len() > MAX_STR_LEN
            || !s
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
        {
            return Err(AuthzError::WireFormat {
                detail: format!("invalid key id {s:?}"),
            });
        }
        Ok(Self(s))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// 32-byte HMAC-SHA256 root key guaranteed to be non-zero (`S3(b)`).
#[derive(Clone, PartialEq, Eq)]
pub struct SecretKey([u8; 32]);

impl SecretKey {
    /// Constructs a [`SecretKey`], refusing the all-zero 32-byte key (`S3(b)`).
    pub fn from_bytes(bytes: [u8; 32], role: &'static str) -> Result<Self, AuthzError> {
        if is_all_zero(&bytes) {
            return Err(AuthzError::ZeroKeyRejected { role });
        }
        Ok(Self(bytes))
    }

    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl std::fmt::Debug for SecretKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SecretKey([REDACTED])")
    }
}

/// Key ring always carrying both an active key and a distinct non-zero retired key (`S3(b)`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FmKeyRing {
    active_id: KeyId,
    active_key: SecretKey,
    retired_id: KeyId,
    retired_key: SecretKey,
}

impl FmKeyRing {
    /// Builds a key ring with explicit active and retired keys. Refuses colliding ids or colliding keys.
    pub fn new(
        active_id: KeyId,
        active_key: SecretKey,
        retired_id: KeyId,
        retired_key: SecretKey,
    ) -> Result<Self, AuthzError> {
        if active_id == retired_id {
            return Err(AuthzError::KeyRingCollision {
                detail: format!("active and retired key share id '{}'", active_id.as_str()),
            });
        }
        if ct_eq_32(active_key.as_bytes(), retired_key.as_bytes()) {
            return Err(AuthzError::KeyRingCollision {
                detail: "active and retired keys must be distinct".to_string(),
            });
        }
        Ok(Self {
            active_id,
            active_key,
            retired_id,
            retired_key,
        })
    }

    /// Builds a key ring with a fresh retired key derived via SHA-256 from `retired_entropy` (`S3(b)`).
    /// Even when a deployment has never rotated keys, the retired slot holds an unguessable non-zero key
    /// instead of an empty or all-zero default.
    pub fn with_derived_retired(
        active_id: KeyId,
        active_key: SecretKey,
        retired_entropy: &[u8],
    ) -> Result<Self, AuthzError> {
        if retired_entropy.is_empty() {
            return Err(AuthzError::ZeroKeyRejected {
                role: "retired_entropy",
            });
        }
        let mut material = Vec::with_capacity(64 + retired_entropy.len());
        material.extend_from_slice(b"FMINE-AUTHZ-RETIRED-KEY-V1:");
        material.extend_from_slice(active_key.as_bytes());
        material.extend_from_slice(retired_entropy);
        let mut derived = sha256(&material).0;
        if is_all_zero(&derived) || ct_eq_32(&derived, active_key.as_bytes()) {
            material.push(0xFF);
            derived = sha256(&material).0;
        }
        let retired_key = SecretKey::from_bytes(derived, "retired_key")?;
        let retired_id = KeyId::new(format!("{}-retired", active_id.as_str()))?;
        Self::new(active_id, active_key, retired_id, retired_key)
    }

    #[must_use]
    pub fn active_id(&self) -> &KeyId {
        &self.active_id
    }

    #[must_use]
    pub fn retired_id(&self) -> &KeyId {
        &self.retired_id
    }

    /// Mints a new first-party macaroon under the active key with initial first-party caveats.
    pub fn mint(
        &self,
        token_id: impl Into<String>,
        location: impl Into<String>,
        caveats: Vec<Caveat>,
    ) -> Result<Macaroon, AuthzError> {
        let token_id = token_id.into();
        let location = location.into();
        validate_token_str("token_id", &token_id)?;
        validate_token_str("location", &location)?;
        if caveats.len() > MAX_CAVEATS {
            return Err(AuthzError::WireFormat {
                detail: format!("caveat count {} exceeds limit {MAX_CAVEATS}", caveats.len()),
            });
        }
        let mut sig = initial_tag(self.active_key.as_bytes(), &token_id, &location);
        for caveat in &caveats {
            validate_caveat(caveat)?;
            sig = chain_caveat(&sig, caveat);
        }
        if is_all_zero(&sig) {
            return Err(AuthzError::ZeroSignatureRejected);
        }
        Ok(Macaroon {
            key_id: self.active_id.clone(),
            token_id,
            location,
            caveats,
            signature: sig,
        })
    }

    /// Verifies a macaroon against the active or retired key.
    ///
    /// Enforces `S3(b)` (unconditional all-zero signature rejection before HMAC evaluation)
    /// and `S3(c)` (strict first-party caveat validation).
    pub fn verify(&self, token: &Macaroon) -> Result<(), AuthzError> {
        // S3(b): NEVER accept an all-zero signature regardless of key ring state.
        if is_all_zero(&token.signature) {
            return Err(AuthzError::ZeroSignatureRejected);
        }
        validate_token_str("token_id", &token.token_id)?;
        validate_token_str("location", &token.location)?;
        if token.caveats.len() > MAX_CAVEATS {
            return Err(AuthzError::WireFormat {
                detail: format!(
                    "caveat count {} exceeds limit {MAX_CAVEATS}",
                    token.caveats.len()
                ),
            });
        }
        let key = if token.key_id == self.active_id {
            &self.active_key
        } else if token.key_id == self.retired_id {
            &self.retired_key
        } else {
            return Err(AuthzError::SignatureInvalid {
                detail: format!("unknown key id '{}'", token.key_id.as_str()),
            });
        };
        let mut expected = initial_tag(key.as_bytes(), &token.token_id, &token.location);
        for caveat in &token.caveats {
            validate_caveat(caveat)?;
            expected = chain_caveat(&expected, caveat);
        }
        if !ct_eq_32(&expected, &token.signature) {
            return Err(AuthzError::SignatureInvalid {
                detail: "HMAC chain mismatch".to_string(),
            });
        }
        Ok(())
    }
}

/// Category of principal identified by a token caveat.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PrincipalKind {
    Human,
    Agent,
    Service,
}

impl PrincipalKind {
    const fn tag(self) -> u8 {
        match self {
            Self::Human => 1,
            Self::Agent => 2,
            Self::Service => 3,
        }
    }

    fn from_tag(tag: u8) -> Result<Self, AuthzError> {
        match tag {
            1 => Ok(Self::Human),
            2 => Ok(Self::Agent),
            3 => Ok(Self::Service),
            other => Err(AuthzError::WireFormat {
                detail: format!("unknown principal kind tag {other}"),
            }),
        }
    }
}

/// Strictly first-party caveats carried by FrankenMining macaroons (§30.4, §31.1–§31.3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Caveat {
    /// Named human, AI agent, or service identity.
    Principal { kind: PrincipalKind, id: String },
    /// Allowed operation scope (e.g. `"read"`, `"prepare_effect"`, `"release_effect"`).
    Scope(String),
    /// Site scope (e.g. `"lumwana"`, `"reko_diq"`).
    Site(String),
    /// Joint-venture wall clearance (`venture=...`).
    Venture(String),
    /// MNPI wall clearance (`mnpi_wall=W-...`).
    MnpiWall(String),
    /// Residency jurisdiction ISO-2 country code.
    Residency(String),
    /// Token issuance instant (UTC ns).
    IssuedAtNs(i64),
    /// Hard expiry instant (UTC ns, inclusive).
    ExpiresAtNs(i64),
    /// Single-use 128-bit nonce (mandatory for `HumanAuthorityToken`).
    SingleUseNonce([u8; 16]),
    /// SHA-256 digest of the exact prepared effect authorized for release.
    EffectDigest([u8; 32]),
    /// Explicit human-held MNPI release capability (§31.1).
    MnpiRelease,
    /// Explicit cross-venture read capability (§31.2).
    CrossVenture,
}

/// Verified or unverified first-party macaroon token.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Macaroon {
    pub key_id: KeyId,
    pub token_id: String,
    pub location: String,
    pub caveats: Vec<Caveat>,
    pub signature: [u8; 32],
}

impl Macaroon {
    /// Attenuates this macaroon by appending a first-party caveat and chaining its HMAC signature.
    pub fn attenuate(&self, caveat: Caveat) -> Result<Self, AuthzError> {
        if is_all_zero(&self.signature) {
            return Err(AuthzError::ZeroSignatureRejected);
        }
        if self.caveats.len() >= MAX_CAVEATS {
            return Err(AuthzError::WireFormat {
                detail: format!("cannot exceed {MAX_CAVEATS} caveats"),
            });
        }
        validate_caveat(&caveat)?;
        let next_sig = chain_caveat(&self.signature, &caveat);
        if is_all_zero(&next_sig) {
            return Err(AuthzError::ZeroSignatureRejected);
        }
        let mut caveats = self.caveats.clone();
        caveats.push(caveat);
        Ok(Self {
            key_id: self.key_id.clone(),
            token_id: self.token_id.clone(),
            location: self.location.clone(),
            caveats,
            signature: next_sig,
        })
    }

    /// Permanently refused per plan §31.4 S3(c) (`INT-AS-SEC-001 (c)`): legacy third-party caveats
    /// can be stripped by the third party and are never permitted in FrankenMining tokens.
    pub fn add_third_party_caveat(
        &self,
        _third_party_location: &str,
        _caveat_id: &[u8],
    ) -> Result<Self, AuthzError> {
        Err(AuthzError::ThirdPartyCaveatForbidden)
    }
}

/// Computes RFC 2104 HMAC-SHA256 using `fmine_core::hash::sha256` (zero external crypto crates).
#[must_use]
pub fn hmac_sha256(key: &[u8], message: &[u8]) -> [u8; 32] {
    const BLOCK_LEN: usize = 64;
    let mut key_block = [0u8; BLOCK_LEN];
    if key.len() > BLOCK_LEN {
        let hashed = sha256(key).0;
        key_block[..32].copy_from_slice(&hashed);
    } else {
        key_block[..key.len()].copy_from_slice(key);
    }

    let mut ipad_buf = Vec::with_capacity(BLOCK_LEN + message.len());
    for &b in &key_block {
        ipad_buf.push(b ^ 0x36);
    }
    ipad_buf.extend_from_slice(message);
    let inner_hash = sha256(&ipad_buf).0;

    let mut opad_buf = [0u8; BLOCK_LEN + 32];
    for i in 0..BLOCK_LEN {
        opad_buf[i] = key_block[i] ^ 0x5c;
    }
    opad_buf[BLOCK_LEN..].copy_from_slice(&inner_hash);
    sha256(&opad_buf).0
}

/// Encodes a macaroon into canonical `FMT-MAC-1` (`FMNE` + `MAC1` + CRC32C) bytes.
pub fn encode_macaroon(token: &Macaroon) -> Result<Vec<u8>, AuthzError> {
    if is_all_zero(&token.signature) {
        return Err(AuthzError::ZeroSignatureRejected);
    }
    validate_token_str("token_id", &token.token_id)?;
    validate_token_str("location", &token.location)?;
    if token.caveats.len() > MAX_CAVEATS {
        return Err(AuthzError::WireFormat {
            detail: format!("caveat count {} exceeds limit", token.caveats.len()),
        });
    }
    let mut enc = Encoder::new_frame(MAC_FORMAT_MAGIC, MAC_FORMAT_VERSION);
    enc.put_str(token.key_id.as_str());
    enc.put_str(&token.token_id);
    enc.put_str(&token.location);
    enc.put_u32(token.caveats.len() as u32);
    for caveat in &token.caveats {
        validate_caveat(caveat)?;
        enc.put_u8(CAVEAT_CLASS_FIRST_PARTY);
        encode_caveat_body(&mut enc, caveat);
    }
    enc.put_fixed(&token.signature);
    Ok(enc.finish())
}

/// Decodes a canonical `FMT-MAC-1` macaroon frame, refusing CRC32C errors, zero signatures (`S3(b)`),
/// and any third-party caveat wire tag (`S3(c)`).
pub fn decode_macaroon(bytes: &[u8]) -> Result<Macaroon, AuthzError> {
    let mut dec =
        Decoder::open_frame(bytes, MAC_FORMAT_MAGIC, 0, MAC_FORMAT_VERSION).map_err(|e| {
            AuthzError::WireFormat {
                detail: e.to_string(),
            }
        })?;
    let key_id = KeyId::new(dec.get_str(MAX_STR_LEN).map_err(wire_err)?.to_string())?;
    let token_id = dec.get_str(MAX_STR_LEN).map_err(wire_err)?.to_string();
    let location = dec.get_str(MAX_STR_LEN).map_err(wire_err)?.to_string();
    validate_token_str("token_id", &token_id)?;
    validate_token_str("location", &location)?;
    let count = dec.get_u32().map_err(wire_err)? as usize;
    if count > MAX_CAVEATS {
        return Err(AuthzError::WireFormat {
            detail: format!("caveat count {count} exceeds limit {MAX_CAVEATS}"),
        });
    }
    let mut caveats = Vec::with_capacity(count);
    for _ in 0..count {
        let class_tag = dec.get_u8().map_err(wire_err)?;
        match class_tag {
            CAVEAT_CLASS_FIRST_PARTY => {
                let caveat = decode_caveat_body(&mut dec)?;
                validate_caveat(&caveat)?;
                caveats.push(caveat);
            }
            CAVEAT_CLASS_THIRD_PARTY => return Err(AuthzError::ThirdPartyCaveatForbidden),
            other => {
                return Err(AuthzError::WireFormat {
                    detail: format!("unknown caveat class tag {other}"),
                });
            }
        }
    }
    let signature = dec.get_fixed::<32>().map_err(wire_err)?;
    dec.finish().map_err(wire_err)?;
    if is_all_zero(&signature) {
        return Err(AuthzError::ZeroSignatureRejected);
    }
    Ok(Macaroon {
        key_id,
        token_id,
        location,
        caveats,
        signature,
    })
}

fn wire_err(e: impl std::fmt::Display) -> AuthzError {
    AuthzError::WireFormat {
        detail: e.to_string(),
    }
}

fn initial_tag(key: &[u8; 32], token_id: &str, location: &str) -> [u8; 32] {
    let mut enc = Encoder::new_frame(*b"MINI", 1);
    enc.put_str(token_id);
    enc.put_str(location);
    hmac_sha256(key, &enc.finish())
}

fn chain_caveat(prev_sig: &[u8; 32], caveat: &Caveat) -> [u8; 32] {
    let mut enc = Encoder::new_frame(*b"MCAV", 1);
    enc.put_u8(CAVEAT_CLASS_FIRST_PARTY);
    encode_caveat_body(&mut enc, caveat);
    hmac_sha256(prev_sig, &enc.finish())
}

fn encode_caveat_body(enc: &mut Encoder, caveat: &Caveat) {
    match caveat {
        Caveat::Principal { kind, id } => {
            enc.put_u8(1);
            enc.put_u8(kind.tag());
            enc.put_str(id);
        }
        Caveat::Scope(s) => {
            enc.put_u8(2);
            enc.put_str(s);
        }
        Caveat::Site(s) => {
            enc.put_u8(3);
            enc.put_str(s);
        }
        Caveat::Venture(s) => {
            enc.put_u8(4);
            enc.put_str(s);
        }
        Caveat::MnpiWall(s) => {
            enc.put_u8(5);
            enc.put_str(s);
        }
        Caveat::Residency(s) => {
            enc.put_u8(6);
            enc.put_str(s);
        }
        Caveat::IssuedAtNs(ns) => {
            enc.put_u8(7);
            enc.put_i64(*ns);
        }
        Caveat::ExpiresAtNs(ns) => {
            enc.put_u8(8);
            enc.put_i64(*ns);
        }
        Caveat::SingleUseNonce(nonce) => {
            enc.put_u8(9);
            enc.put_fixed(nonce);
        }
        Caveat::EffectDigest(digest) => {
            enc.put_u8(10);
            enc.put_fixed(digest);
        }
        Caveat::MnpiRelease => enc.put_u8(11),
        Caveat::CrossVenture => enc.put_u8(12),
    }
}

fn decode_caveat_body(dec: &mut Decoder<'_>) -> Result<Caveat, AuthzError> {
    let tag = dec.get_u8().map_err(wire_err)?;
    match tag {
        1 => {
            let kind = PrincipalKind::from_tag(dec.get_u8().map_err(wire_err)?)?;
            let id = dec.get_str(MAX_STR_LEN).map_err(wire_err)?.to_string();
            Ok(Caveat::Principal { kind, id })
        }
        2 => Ok(Caveat::Scope(
            dec.get_str(MAX_STR_LEN).map_err(wire_err)?.to_string(),
        )),
        3 => Ok(Caveat::Site(
            dec.get_str(MAX_STR_LEN).map_err(wire_err)?.to_string(),
        )),
        4 => Ok(Caveat::Venture(
            dec.get_str(MAX_STR_LEN).map_err(wire_err)?.to_string(),
        )),
        5 => Ok(Caveat::MnpiWall(
            dec.get_str(MAX_STR_LEN).map_err(wire_err)?.to_string(),
        )),
        6 => Ok(Caveat::Residency(
            dec.get_str(MAX_STR_LEN).map_err(wire_err)?.to_string(),
        )),
        7 => Ok(Caveat::IssuedAtNs(dec.get_i64().map_err(wire_err)?)),
        8 => Ok(Caveat::ExpiresAtNs(dec.get_i64().map_err(wire_err)?)),
        9 => Ok(Caveat::SingleUseNonce(
            dec.get_fixed::<16>().map_err(wire_err)?,
        )),
        10 => Ok(Caveat::EffectDigest(
            dec.get_fixed::<32>().map_err(wire_err)?,
        )),
        11 => Ok(Caveat::MnpiRelease),
        12 => Ok(Caveat::CrossVenture),
        other => Err(AuthzError::WireFormat {
            detail: format!("unknown caveat variant tag {other}"),
        }),
    }
}

fn validate_token_str(field: &'static str, value: &str) -> Result<(), AuthzError> {
    if value.is_empty() || value.len() > MAX_STR_LEN {
        return Err(AuthzError::WireFormat {
            detail: format!("invalid {field} length {}", value.len()),
        });
    }
    Ok(())
}

fn validate_caveat(caveat: &Caveat) -> Result<(), AuthzError> {
    match caveat {
        Caveat::Principal { id, .. } => validate_token_str("principal_id", id),
        Caveat::Scope(s) => validate_token_str("scope", s),
        Caveat::Site(s) => validate_token_str("site", s),
        Caveat::Venture(s) => validate_token_str("venture", s),
        Caveat::MnpiWall(s) => {
            validate_token_str("mnpi_wall", s)?;
            if !s.starts_with("W-") || s.len() == 2 {
                return Err(AuthzError::WireFormat {
                    detail: format!("mnpi_wall caveat must begin with W-: {s:?}"),
                });
            }
            Ok(())
        }
        Caveat::Residency(s) => {
            if s.len() != 2 || !s.bytes().all(|b| b.is_ascii_uppercase()) {
                return Err(AuthzError::WireFormat {
                    detail: format!("residency caveat must be ISO-2 uppercase country: {s:?}"),
                });
            }
            Ok(())
        }
        Caveat::IssuedAtNs(_)
        | Caveat::ExpiresAtNs(_)
        | Caveat::SingleUseNonce(_)
        | Caveat::EffectDigest(_)
        | Caveat::MnpiRelease
        | Caveat::CrossVenture => Ok(()),
    }
}

/// Constant-time check for an all-zero 32-byte slice (zero `unsafe`).
#[must_use]
pub fn is_all_zero(bytes: &[u8]) -> bool {
    let mut acc = 0u8;
    for &b in bytes {
        acc |= black_box(b);
    }
    black_box(acc) == 0
}

/// Constant-time equality over two 32-byte tags (zero `unsafe`).
#[must_use]
pub fn ct_eq_32(a: &[u8; 32], b: &[u8; 32]) -> bool {
    let mut diff = 0u8;
    for i in 0..32 {
        diff |= black_box(a[i] ^ b[i]);
    }
    black_box(diff) == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex_bytes<const N: usize>(hex: &str) -> [u8; N] {
        assert_eq!(hex.len(), N * 2);
        let mut out = [0u8; N];
        for i in 0..N {
            out[i] = u8::from_str_radix(&hex[2 * i..2 * i + 2], 16).expect("hex byte");
        }
        out
    }

    #[test]
    fn test_hmac_sha256_rfc4231_known_answers() {
        // RFC 4231 Test Case 1
        let k1 = [0x0bu8; 20];
        let d1 = b"Hi There";
        let want1: [u8; 32] =
            hex_bytes("b0344c61d8db38535ca8afceaf0bf12b881dc200c9833da726e9376c2e32cff7");
        assert_eq!(hmac_sha256(&k1, d1), want1);

        // RFC 4231 Test Case 2
        let k2 = b"Jefe";
        let d2 = b"what do ya want for nothing?";
        let want2: [u8; 32] =
            hex_bytes("5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec3843");
        assert_eq!(hmac_sha256(k2, d2), want2);

        // RFC 4231 Test Case 3
        let k3 = [0xaau8; 20];
        let d3 = [0xddu8; 50];
        let want3: [u8; 32] =
            hex_bytes("773ea91e36800e46854db8ebd09181a72959098b3ef8c122d9635514ced565fe");
        assert_eq!(hmac_sha256(&k3, &d3), want3);

        // RFC 4231 Test Case 6 (key longer than 64-byte block)
        let k6 = [0xaau8; 131];
        let d6 = b"Test Using Larger Than Block-Size Key - Hash Key First";
        let want6: [u8; 32] =
            hex_bytes("60e431591ee0b67f0d8a26aacbf5b77f8e0bc6213728c5140546040f0ee37f54");
        assert_eq!(hmac_sha256(&k6, d6), want6);
    }

    /// INT-AS-SEC-001 (b): zero keys refused, retired key always populated and non-zero,
    /// and all-zero signatures unconditionally refused before HMAC evaluation.
    #[test]
    fn test_int_as_sec_001_b_random_retired_key_and_zero_signature_rejected() {
        assert!(matches!(
            SecretKey::from_bytes([0u8; 32], "active"),
            Err(AuthzError::ZeroKeyRejected { role: "active" })
        ));

        let k_active = SecretKey::from_bytes([0x11; 32], "active").expect("active key");
        let k_retired = SecretKey::from_bytes([0x22; 32], "retired").expect("retired key");
        let id_active = KeyId::new("k-2026-11").expect("id");
        let id_retired = KeyId::new("k-2026-10").expect("id");

        // Same key or same id refused.
        assert!(matches!(
            FmKeyRing::new(
                id_active.clone(),
                k_active.clone(),
                id_active.clone(),
                k_retired.clone()
            ),
            Err(AuthzError::KeyRingCollision { .. })
        ));
        assert!(matches!(
            FmKeyRing::new(
                id_active.clone(),
                k_active.clone(),
                id_retired.clone(),
                k_active.clone()
            ),
            Err(AuthzError::KeyRingCollision { .. })
        ));

        // Derived retired key is non-zero and distinct from active key.
        let ring = FmKeyRing::with_derived_retired(
            id_active.clone(),
            k_active.clone(),
            b"boot-entropy-2026-11-20",
        )
        .expect("derived ring");
        assert_ne!(ring.active_id(), ring.retired_id());

        let token = ring
            .mint(
                "tok-001",
                "fmine://lumwana",
                vec![Caveat::Scope("read".to_string())],
            )
            .expect("mint");
        ring.verify(&token).expect("valid token verifies");

        // All-zero signature MUST be rejected on active and retired key ids alike.
        let mut zero_sig = token.clone();
        zero_sig.signature = [0u8; 32];
        assert_eq!(
            ring.verify(&zero_sig),
            Err(AuthzError::ZeroSignatureRejected)
        );
        zero_sig.key_id = ring.retired_id().clone();
        assert_eq!(
            ring.verify(&zero_sig),
            Err(AuthzError::ZeroSignatureRejected)
        );
        assert_eq!(
            encode_macaroon(&zero_sig),
            Err(AuthzError::ZeroSignatureRejected)
        );

        // Key rotation: token minted with old key still verifies when moved to retired slot.
        let rotated = FmKeyRing::new(id_active, k_active, id_retired.clone(), k_retired.clone())
            .expect("ring");
        let old_ring =
            FmKeyRing::with_derived_retired(id_retired, k_retired, b"old-seed").expect("old");
        let old_tok = old_ring
            .mint(
                "tok-old",
                "fmine://lumwana",
                vec![Caveat::Site("lumwana".into())],
            )
            .expect("mint old");
        rotated
            .verify(&old_tok)
            .expect("retired key verifies old token");

        // One-bit flip in signature fails.
        let mut forged = old_tok;
        forged.signature[0] ^= 0x01;
        assert!(matches!(
            rotated.verify(&forged),
            Err(AuthzError::SignatureInvalid { .. })
        ));
    }

    /// INT-AS-SEC-001 (c): third-party caveats are refused at mint and on wire decode;
    /// first-party attenuation chains verify and resist stripping/reordering.
    #[test]
    fn test_int_as_sec_001_c_third_party_caveat_refused_at_mint_and_decode() {
        let ring = FmKeyRing::with_derived_retired(
            KeyId::new("k-1").expect("id"),
            SecretKey::from_bytes([0x5A; 32], "active").expect("key"),
            b"entropy-s3c",
        )
        .expect("ring");

        let base = ring
            .mint(
                "tok-s3c",
                "fmine://reko-diq",
                vec![Caveat::Principal {
                    kind: PrincipalKind::Agent,
                    id: "agent-01".into(),
                }],
            )
            .expect("mint");

        // Minting a third-party caveat is permanently refused.
        assert_eq!(
            base.add_third_party_caveat("https://third-party.invalid", b"cid"),
            Err(AuthzError::ThirdPartyCaveatForbidden)
        );

        // Attenuation chains verify and round-trip through FMT-MAC-1.
        let narrowed = base
            .attenuate(Caveat::Venture("reko_diq".into()))
            .expect("attenuate 1")
            .attenuate(Caveat::Site("reko_diq".into()))
            .expect("attenuate 2");
        ring.verify(&narrowed).expect("narrowed token verifies");

        let wire = encode_macaroon(&narrowed).expect("encode");
        let decoded = decode_macaroon(&wire).expect("decode");
        assert_eq!(decoded, narrowed);
        ring.verify(&decoded).expect("decoded verifies");

        // Stripping the last caveat without re-signing fails verification.
        let mut stripped = narrowed.clone();
        stripped.caveats.pop();
        assert!(matches!(
            ring.verify(&stripped),
            Err(AuthzError::SignatureInvalid { .. })
        ));

        // Hand-crafted wire frame carrying CAVEAT_CLASS_THIRD_PARTY (2) is refused at decode.
        let mut enc = Encoder::new_frame(MAC_FORMAT_MAGIC, MAC_FORMAT_VERSION);
        enc.put_str(narrowed.key_id.as_str());
        enc.put_str(&narrowed.token_id);
        enc.put_str(&narrowed.location);
        enc.put_u32(1);
        enc.put_u8(CAVEAT_CLASS_THIRD_PARTY);
        enc.put_u8(2);
        enc.put_str("read");
        enc.put_fixed(&narrowed.signature);
        assert_eq!(
            decode_macaroon(&enc.finish()),
            Err(AuthzError::ThirdPartyCaveatForbidden)
        );
    }
}
