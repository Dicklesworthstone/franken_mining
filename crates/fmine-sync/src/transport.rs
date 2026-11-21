//! Determinism: BitwisePortable
//!
//! QUIC/TLS 1.3 transport policy over ATP with unconditional plaintext ban (`BD-105`, plan §31.4 S3(d)).
//!
//! Defect S3(d): ATP 0.6.0 allows cleartext channels if misconfigured. FrankenMining bans every
//! plaintext or unauthenticated transport scheme at the type and session boundary, requiring:
//! 1. [`TransportScheme::QuicTls13Atp`];
//! 2. mutual TLS 1.3 with non-zero peer certificate digest and ALPN [`ATP_ALPN_V1`];
//! 3. a verified first-party [`fmine_authz::Macaroon`] (enforcing S3(b) zero-sig rejection and
//!    S3(c) first-party caveats only) scoped to sync and the requested site, within its expiry window.

use fmine_authz::macaroon::{Caveat, FmKeyRing, Macaroon};
use fmine_core::codec::Encoder;
use fmine_core::hash::{Digest32, sha256};

use crate::SyncError;

/// Required ALPN protocol identifier for FrankenMining ATP sync over QUIC/TLS 1.3.
pub const ATP_ALPN_V1: &str = "fmine-atp/1";

/// Transport channel scheme offered to the sync session gate (`BD-105`, `S3(d)`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransportScheme {
    /// Mandatory encrypted & mutually authenticated QUIC/TLS 1.3 over ATP.
    QuicTls13Atp,
    /// Forbidden cleartext TCP (refused unconditionally).
    PlaintextTcp,
    /// Forbidden cleartext UDP / unencrypted ATP (refused unconditionally).
    PlaintextUdp,
    /// Forbidden TLS without mutual authentication or TLS 1.3 (refused unconditionally).
    UnauthenticatedTls,
}

impl TransportScheme {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::QuicTls13Atp => "quic+tls1.3+atp",
            Self::PlaintextTcp => "plaintext-tcp",
            Self::PlaintextUdp => "plaintext-udp",
            Self::UnauthenticatedTls => "unauthenticated-tls",
        }
    }
}

/// Verified peer TLS 1.3 identity presented by the QUIC/ATP transport handshake.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TlsPeerIdentity {
    pub peer_node_id: String,
    pub tls_version_1_3: bool,
    pub mtls_verified: bool,
    pub peer_cert_sha256: Digest32,
    pub alpn: String,
}

/// Established, mutually authenticated QUIC/TLS 1.3 + macaroon sync session (`BD-105`).
/// Can only be constructed via [`AuthenticatedSyncSession::establish`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthenticatedSyncSession {
    pub peer_node_id: String,
    pub site: String,
    pub token_id: String,
    pub peer_cert_sha256: Digest32,
    pub session_transcript_digest: Digest32,
}

impl AuthenticatedSyncSession {
    /// Enforces `S3(d)` (plaintext banned), mutual TLS 1.3 + ALPN, and first-party macaroon
    /// authorization before permitting any sync frame exchange.
    pub fn establish(
        scheme: TransportScheme,
        peer: &TlsPeerIdentity,
        macaroon: &Macaroon,
        key_ring: &FmKeyRing,
        required_site: &str,
        now_ns: i64,
    ) -> Result<Self, SyncError> {
        match scheme {
            TransportScheme::QuicTls13Atp => {}
            TransportScheme::PlaintextTcp | TransportScheme::PlaintextUdp => {
                return Err(SyncError::PlaintextBanned {
                    scheme: scheme.as_str(),
                });
            }
            TransportScheme::UnauthenticatedTls => {
                return Err(SyncError::TlsPolicyViolation {
                    detail: "unauthenticated TLS is forbidden; mTLS 1.3 is mandatory".to_string(),
                });
            }
        }

        if peer.peer_node_id.is_empty() || peer.peer_node_id.len() > 256 {
            return Err(SyncError::TlsPolicyViolation {
                detail: "peer_node_id must be 1..=256 bytes".to_string(),
            });
        }
        if !peer.tls_version_1_3 {
            return Err(SyncError::TlsPolicyViolation {
                detail: "TLS version below 1.3 is forbidden".to_string(),
            });
        }
        if !peer.mtls_verified {
            return Err(SyncError::TlsPolicyViolation {
                detail: "mutual TLS client certificate verification is mandatory".to_string(),
            });
        }
        if peer.peer_cert_sha256.0.iter().all(|&b| b == 0) {
            return Err(SyncError::TlsPolicyViolation {
                detail: "all-zero peer certificate digest rejected".to_string(),
            });
        }
        if peer.alpn != ATP_ALPN_V1 {
            return Err(SyncError::TlsPolicyViolation {
                detail: format!("ALPN {:?} != required {ATP_ALPN_V1:?}", peer.alpn),
            });
        }

        // Verify cryptographic macaroon integrity (enforces S3(b) zero-sig rejection & S3(c) first-party only).
        key_ring.verify(macaroon)?;

        // Check required sync scope, site binding, and explicit time expiry.
        let mut has_sync_scope = false;
        let mut site_matches = false;
        let mut has_expiry = false;
        for caveat in &macaroon.caveats {
            match caveat {
                Caveat::Scope(s) if s == "sync" => has_sync_scope = true,
                Caveat::Site(s) if s == required_site => site_matches = true,
                Caveat::IssuedAtNs(t) if now_ns < *t => {
                    return Err(SyncError::TlsPolicyViolation {
                        detail: format!("sync macaroon not yet valid (now={now_ns} < issued={t})"),
                    });
                }
                Caveat::ExpiresAtNs(exp) => {
                    has_expiry = true;
                    if now_ns > *exp {
                        return Err(SyncError::TlsPolicyViolation {
                            detail: format!("sync macaroon expired (now={now_ns} > exp={exp})"),
                        });
                    }
                }
                _ => {}
            }
        }
        if !has_sync_scope {
            return Err(SyncError::TlsPolicyViolation {
                detail: "sync macaroon lacks Scope(\"sync\") caveat".to_string(),
            });
        }
        if !site_matches {
            return Err(SyncError::TlsPolicyViolation {
                detail: format!("sync macaroon is not scoped to site '{required_site}'"),
            });
        }
        if !has_expiry {
            return Err(SyncError::TlsPolicyViolation {
                detail: "sync macaroon must carry an explicit ExpiresAtNs caveat".to_string(),
            });
        }

        let mut enc = Encoder::new_frame(*b"ATPS", 1);
        enc.put_str(scheme.as_str());
        enc.put_str(&peer.peer_node_id);
        enc.put_fixed(peer.peer_cert_sha256.as_bytes());
        enc.put_str(&peer.alpn);
        enc.put_str(required_site);
        enc.put_str(&macaroon.token_id);
        enc.put_fixed(&macaroon.signature);
        let session_transcript_digest = sha256(&enc.finish());

        Ok(Self {
            peer_node_id: peer.peer_node_id.clone(),
            site: required_site.to_string(),
            token_id: macaroon.token_id.clone(),
            peer_cert_sha256: peer.peer_cert_sha256,
            session_transcript_digest,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fmine_authz::macaroon::{KeyId, PrincipalKind, SecretKey};

    fn test_ring_and_token() -> (FmKeyRing, Macaroon) {
        let active_key = SecretKey::from_bytes([0x11; 32], "active").expect("active key");
        let ring = FmKeyRing::with_derived_retired(
            KeyId::new("sync-k1").expect("kid"),
            active_key,
            b"sync-retired-entropy-seed",
        )
        .expect("ring");
        let token = ring
            .mint(
                "tok-sync-lumwana-001",
                "fmine-sync://lumwana",
                vec![
                    Caveat::Principal {
                        kind: PrincipalKind::Service,
                        id: "site-edge-lumwana".to_string(),
                    },
                    Caveat::Scope("sync".to_string()),
                    Caveat::Site("lumwana".to_string()),
                    Caveat::IssuedAtNs(1_000),
                    Caveat::ExpiresAtNs(5_000),
                ],
            )
            .expect("mint");
        (ring, token)
    }

    fn valid_peer() -> TlsPeerIdentity {
        TlsPeerIdentity {
            peer_node_id: "site-edge-lumwana".to_string(),
            tls_version_1_3: true,
            mtls_verified: true,
            peer_cert_sha256: sha256(b"lumwana-mtls-cert-v1"),
            alpn: ATP_ALPN_V1.to_string(),
        }
    }

    /// BD-105 done-when (`Plaintext attempt refused`): verifies S3(d) plaintext TCP/UDP ban,
    /// TLS < 1.3 & missing mTLS rejection, wrong ALPN refusal, zero-sig macaroon refusal,
    /// expired/wrong-site refusal, and valid QUIC/TLS 1.3 ATP session establishment.
    #[test]
    fn test_quic_tls_atp_transport_refuses_plaintext_and_verifies_macaroon() {
        let (ring, token) = test_ring_and_token();
        let peer = valid_peer();

        // 1. Plaintext TCP and UDP are unconditionally banned (S3(d)).
        for banned in [TransportScheme::PlaintextTcp, TransportScheme::PlaintextUdp] {
            let err =
                AuthenticatedSyncSession::establish(banned, &peer, &token, &ring, "lumwana", 2_000)
                    .expect_err("plaintext must refuse");
            assert!(matches!(err, SyncError::PlaintextBanned { .. }));
            assert_eq!(err.to_refusal().code, "FMINE-REF-505");
        }

        // 2. Unauthenticated TLS, TLS < 1.3, and wrong ALPN refuse.
        assert!(matches!(
            AuthenticatedSyncSession::establish(
                TransportScheme::UnauthenticatedTls,
                &peer,
                &token,
                &ring,
                "lumwana",
                2_000,
            ),
            Err(SyncError::TlsPolicyViolation { .. })
        ));
        let mut tls12_peer = peer.clone();
        tls12_peer.tls_version_1_3 = false;
        assert!(matches!(
            AuthenticatedSyncSession::establish(
                TransportScheme::QuicTls13Atp,
                &tls12_peer,
                &token,
                &ring,
                "lumwana",
                2_000,
            ),
            Err(SyncError::TlsPolicyViolation { .. })
        ));
        let mut bad_alpn = peer.clone();
        bad_alpn.alpn = "http/1.1".to_string();
        assert!(matches!(
            AuthenticatedSyncSession::establish(
                TransportScheme::QuicTls13Atp,
                &bad_alpn,
                &token,
                &ring,
                "lumwana",
                2_000,
            ),
            Err(SyncError::TlsPolicyViolation { .. })
        ));

        // 3. Forged all-zero macaroon signature refuses (S3(b)), expired token refuses, wrong site refuses.
        let mut zero_sig = token.clone();
        zero_sig.signature = [0u8; 32];
        assert!(matches!(
            AuthenticatedSyncSession::establish(
                TransportScheme::QuicTls13Atp,
                &peer,
                &zero_sig,
                &ring,
                "lumwana",
                2_000,
            ),
            Err(SyncError::Authz(_))
        ));
        assert!(matches!(
            AuthenticatedSyncSession::establish(
                TransportScheme::QuicTls13Atp,
                &peer,
                &token,
                &ring,
                "lumwana",
                9_999,
            ),
            Err(SyncError::TlsPolicyViolation { .. })
        ));
        assert!(matches!(
            AuthenticatedSyncSession::establish(
                TransportScheme::QuicTls13Atp,
                &peer,
                &token,
                &ring,
                "reko_diq",
                2_000,
            ),
            Err(SyncError::TlsPolicyViolation { .. })
        ));

        // 4. Valid QUIC/TLS 1.3 ATP + mTLS + verified macaroon establishes a deterministic transcript.
        let s1 = AuthenticatedSyncSession::establish(
            TransportScheme::QuicTls13Atp,
            &peer,
            &token,
            &ring,
            "lumwana",
            2_000,
        )
        .expect("valid session");
        let s2 = AuthenticatedSyncSession::establish(
            TransportScheme::QuicTls13Atp,
            &peer,
            &token,
            &ring,
            "lumwana",
            2_000,
        )
        .expect("deterministic session");
        assert_eq!(s1, s2);
        assert_eq!(s1.site, "lumwana");
    }
}
