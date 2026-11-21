//! Determinism: BitwisePortable
//!
//! `HumanAuthorityToken` (`BD-095`, `PO-SEC-3`, plan §31.3):
//! - First-party macaroon minted ONLY by a named human principal (never by an AI agent or service).
//! - Hard TTL ceiling: `<= 15 min` (`900_000_000_000 ns`), enforced at BOTH mint and verify.
//! - Bound to one exact prepared effect SHA-256 `effect_digest` and one `site`.
//! - Single-use 128-bit nonce tracked and consumed atomically by [`HumanTokenVerifier`].
//! - Supports dual human attestation (two distinct humans for the same prepared effect, e.g. §17.6).

use std::collections::BTreeSet;

use crate::AuthzError;
use crate::macaroon::{Caveat, FmKeyRing, Macaroon, PrincipalKind, is_all_zero};

/// Hard maximum lifetime of a `HumanAuthorityToken` (`15 minutes` = `900 s` = `900_000_000_000 ns`, §31.3).
pub const MAX_HUMAN_TOKEN_TTL_NS: i64 = 15 * 60 * 1_000_000_000;

/// Inputs to mint a single-use `HumanAuthorityToken`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HumanAuthorityMintRequest {
    pub token_id: String,
    pub human_id: String,
    pub site: String,
    pub effect_digest: [u8; 32],
    pub issued_at_ns: i64,
    pub ttl_ns: i64,
    pub single_use_nonce: [u8; 16],
}

/// Verified single-use human grant returned after consuming a token.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedHumanGrant {
    pub human_id: String,
    pub site: String,
    pub effect_digest: [u8; 32],
    pub issued_at_ns: i64,
    pub expires_at_ns: i64,
    pub nonce: [u8; 16],
}

/// Verified dual-human attestation over one prepared effect (§17.6, §31.3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DualAttestationGrant {
    pub first: VerifiedHumanGrant,
    pub second: VerifiedHumanGrant,
}

/// Mints a first-party `HumanAuthorityToken` macaroon bound to one human, site, effect digest,
/// single-use nonce, and `ttl_ns <= 15 min`.
pub fn mint_human_authority_token(
    keyring: &FmKeyRing,
    req: &HumanAuthorityMintRequest,
) -> Result<Macaroon, AuthzError> {
    if req.human_id.trim().is_empty() {
        return Err(AuthzError::HumanAuthorityRefused {
            detail: "human_id must be non-empty".to_string(),
        });
    }
    if req.site.trim().is_empty() {
        return Err(AuthzError::HumanAuthorityRefused {
            detail: "site must be non-empty".to_string(),
        });
    }
    if req.ttl_ns <= 0 || req.ttl_ns > MAX_HUMAN_TOKEN_TTL_NS {
        return Err(AuthzError::HumanAuthorityRefused {
            detail: format!(
                "ttl_ns {} violates 0 < ttl <= {} (15 min)",
                req.ttl_ns, MAX_HUMAN_TOKEN_TTL_NS
            ),
        });
    }
    if is_all_zero(&req.effect_digest) {
        return Err(AuthzError::HumanAuthorityRefused {
            detail: "effect_digest must be non-zero".to_string(),
        });
    }
    if is_all_zero(&req.single_use_nonce) {
        return Err(AuthzError::HumanAuthorityRefused {
            detail: "single_use_nonce must be non-zero".to_string(),
        });
    }
    let expires_at_ns = req.issued_at_ns.checked_add(req.ttl_ns).ok_or_else(|| {
        AuthzError::HumanAuthorityRefused {
            detail: "issued_at_ns + ttl_ns overflowed i64".to_string(),
        }
    })?;
    keyring.mint(
        req.token_id.clone(),
        format!("fmine://human-authority/{}", req.site),
        vec![
            Caveat::Principal {
                kind: PrincipalKind::Human,
                id: req.human_id.clone(),
            },
            Caveat::Scope("release_effect".to_string()),
            Caveat::Site(req.site.clone()),
            Caveat::EffectDigest(req.effect_digest),
            Caveat::IssuedAtNs(req.issued_at_ns),
            Caveat::ExpiresAtNs(expires_at_ns),
            Caveat::SingleUseNonce(req.single_use_nonce),
        ],
    )
}

/// Stateful verifier enforcing single-use nonces, <=15 min TTL, human-only principal,
/// site match, and exact effect-digest binding (`PO-SEC-3`).
#[derive(Debug, Default, Clone)]
pub struct HumanTokenVerifier {
    consumed_nonces: BTreeSet<[u8; 16]>,
}

impl HumanTokenVerifier {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            consumed_nonces: BTreeSet::new(),
        }
    }

    #[must_use]
    pub fn consumed_count(&self) -> usize {
        self.consumed_nonces.len()
    }

    /// Validates a `HumanAuthorityToken` and consumes its single-use nonce atomically on success.
    pub fn verify_and_consume(
        &mut self,
        keyring: &FmKeyRing,
        token: &Macaroon,
        expected_site: &str,
        expected_effect_digest: &[u8; 32],
        now_ns: i64,
    ) -> Result<VerifiedHumanGrant, AuthzError> {
        let grant = self.validate_without_consuming(
            keyring,
            token,
            expected_site,
            expected_effect_digest,
            now_ns,
        )?;
        self.consumed_nonces.insert(grant.nonce);
        Ok(grant)
    }

    /// Validates two distinct humans' `HumanAuthorityToken`s over the same prepared effect and consumes
    /// both nonces atomically ONLY if both tokens pass and name distinct humans & distinct nonces.
    pub fn verify_dual_attestation(
        &mut self,
        keyring: &FmKeyRing,
        first_token: &Macaroon,
        second_token: &Macaroon,
        expected_site: &str,
        expected_effect_digest: &[u8; 32],
        now_ns: i64,
    ) -> Result<DualAttestationGrant, AuthzError> {
        let first = self.validate_without_consuming(
            keyring,
            first_token,
            expected_site,
            expected_effect_digest,
            now_ns,
        )?;
        let second = self.validate_without_consuming(
            keyring,
            second_token,
            expected_site,
            expected_effect_digest,
            now_ns,
        )?;
        if first.human_id == second.human_id {
            return Err(AuthzError::HumanAuthorityRefused {
                detail: format!(
                    "dual attestation requires two distinct humans, got '{}' twice",
                    first.human_id
                ),
            });
        }
        if first.nonce == second.nonce {
            return Err(AuthzError::HumanAuthorityRefused {
                detail: "dual attestation tokens must carry distinct single-use nonces".to_string(),
            });
        }
        self.consumed_nonces.insert(first.nonce);
        self.consumed_nonces.insert(second.nonce);
        Ok(DualAttestationGrant { first, second })
    }

    fn validate_without_consuming(
        &self,
        keyring: &FmKeyRing,
        token: &Macaroon,
        expected_site: &str,
        expected_effect_digest: &[u8; 32],
        now_ns: i64,
    ) -> Result<VerifiedHumanGrant, AuthzError> {
        if is_all_zero(expected_effect_digest) {
            return Err(AuthzError::HumanAuthorityRefused {
                detail: "expected_effect_digest must be non-zero".to_string(),
            });
        }
        keyring.verify(token)?;

        let mut human_id: Option<String> = None;
        let mut scope_ok = false;
        let mut site: Option<String> = None;
        let mut effect_digest: Option<[u8; 32]> = None;
        let mut issued_at_ns: Option<i64> = None;
        let mut expires_at_ns: Option<i64> = None;
        let mut nonce: Option<[u8; 16]> = None;

        for caveat in &token.caveats {
            match caveat {
                Caveat::Principal { kind, id } => {
                    if *kind != PrincipalKind::Human {
                        return Err(AuthzError::HumanAuthorityRefused {
                            detail: format!(
                                "non-human principal ({kind:?} '{id}') cannot authorize effect release"
                            ),
                        });
                    }
                    if human_id.replace(id.clone()).is_some() {
                        return Err(AuthzError::HumanAuthorityRefused {
                            detail: "multiple Principal caveats in HumanAuthorityToken".to_string(),
                        });
                    }
                }
                Caveat::Scope(s) if s == "release_effect" => scope_ok = true,
                Caveat::Site(s) => {
                    if let Some(prev) = &site {
                        if prev != s {
                            return Err(AuthzError::HumanAuthorityRefused {
                                detail: format!("conflicting Site caveats '{prev}' vs '{s}'"),
                            });
                        }
                    } else {
                        site = Some(s.clone());
                    }
                }
                Caveat::EffectDigest(d) => {
                    if let Some(prev) = effect_digest {
                        if prev != *d {
                            return Err(AuthzError::HumanAuthorityRefused {
                                detail: "conflicting EffectDigest caveats".to_string(),
                            });
                        }
                    } else {
                        effect_digest = Some(*d);
                    }
                }
                Caveat::IssuedAtNs(ns) => {
                    issued_at_ns = Some(issued_at_ns.map_or(*ns, |prev| prev.max(*ns)));
                }
                Caveat::ExpiresAtNs(ns) => {
                    // Attenuation can only tighten expiry (take minimum).
                    expires_at_ns = Some(expires_at_ns.map_or(*ns, |prev| prev.min(*ns)));
                }
                Caveat::SingleUseNonce(n) => match nonce {
                    Some(_) => {
                        return Err(AuthzError::HumanAuthorityRefused {
                            detail: "multiple SingleUseNonce caveats".to_string(),
                        });
                    }
                    None => nonce = Some(*n),
                },
                _ => {}
            }
        }

        let human_id = human_id.ok_or_else(|| AuthzError::HumanAuthorityRefused {
            detail: "missing Human Principal caveat".to_string(),
        })?;
        if !scope_ok {
            return Err(AuthzError::HumanAuthorityRefused {
                detail: "missing `release_effect` Scope caveat".to_string(),
            });
        }
        let site = site.ok_or_else(|| AuthzError::HumanAuthorityRefused {
            detail: "missing Site caveat".to_string(),
        })?;
        if site != expected_site {
            return Err(AuthzError::HumanAuthorityRefused {
                detail: format!("site mismatch: token '{site}' != expected '{expected_site}'"),
            });
        }
        let effect_digest = effect_digest.ok_or_else(|| AuthzError::HumanAuthorityRefused {
            detail: "missing EffectDigest caveat".to_string(),
        })?;
        if &effect_digest != expected_effect_digest {
            return Err(AuthzError::HumanAuthorityRefused {
                detail: "effect_digest does not match prepared effect".to_string(),
            });
        }
        let issued_at_ns = issued_at_ns.ok_or_else(|| AuthzError::HumanAuthorityRefused {
            detail: "missing IssuedAtNs caveat".to_string(),
        })?;
        let expires_at_ns = expires_at_ns.ok_or_else(|| AuthzError::HumanAuthorityRefused {
            detail: "missing ExpiresAtNs caveat".to_string(),
        })?;
        let ttl = expires_at_ns.checked_sub(issued_at_ns).ok_or_else(|| {
            AuthzError::HumanAuthorityRefused {
                detail: "TTL subtraction overflow".to_string(),
            }
        })?;
        if ttl <= 0 || ttl > MAX_HUMAN_TOKEN_TTL_NS {
            return Err(AuthzError::HumanAuthorityRefused {
                detail: format!("token TTL {ttl} ns violates 0 < ttl <= 15 min"),
            });
        }
        if now_ns < issued_at_ns {
            return Err(AuthzError::TokenTemporalInvalid {
                detail: format!("token not yet valid: now {now_ns} < issued {issued_at_ns}"),
            });
        }
        if now_ns > expires_at_ns {
            return Err(AuthzError::TokenTemporalInvalid {
                detail: format!("token expired: now {now_ns} > expires {expires_at_ns}"),
            });
        }
        let nonce = nonce.ok_or_else(|| AuthzError::HumanAuthorityRefused {
            detail: "missing SingleUseNonce caveat".to_string(),
        })?;
        if is_all_zero(&nonce) {
            return Err(AuthzError::HumanAuthorityRefused {
                detail: "single_use_nonce must be non-zero".to_string(),
            });
        }
        if self.consumed_nonces.contains(&nonce) {
            return Err(AuthzError::HumanAuthorityRefused {
                detail: "single-use token replay refused".to_string(),
            });
        }

        Ok(VerifiedHumanGrant {
            human_id,
            site,
            effect_digest,
            issued_at_ns,
            expires_at_ns,
            nonce,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::macaroon::{KeyId, SecretKey};

    fn test_ring() -> FmKeyRing {
        FmKeyRing::with_derived_retired(
            KeyId::new("k-human-1").expect("id"),
            SecretKey::from_bytes([0x33; 32], "active").expect("key"),
            b"human-token-test-entropy",
        )
        .expect("ring")
    }

    #[test]
    fn test_po_sec_3_human_token_single_use_15min_ttl_and_effect_digest_binding() {
        let ring = test_ring();
        let mut verifier = HumanTokenVerifier::new();
        let effect = [0xAB; 32];
        let other_effect = [0xCD; 32];
        let t0 = 1_760_000_000_000_000_000i64;

        // Minting > 15 min is refused.
        let over_ttl = HumanAuthorityMintRequest {
            token_id: "hat-over".into(),
            human_id: "qp-alice".into(),
            site: "lumwana".into(),
            effect_digest: effect,
            issued_at_ns: t0,
            ttl_ns: MAX_HUMAN_TOKEN_TTL_NS + 1,
            single_use_nonce: [1u8; 16],
        };
        let err = mint_human_authority_token(&ring, &over_ttl).expect_err(">15m refused");
        assert_eq!(
            (err.to_refusal().code, err.to_refusal().exit_code),
            ("FMINE-REF-304", 7)
        );

        // Even a raw macaroon signed directly with ttl > 15 min is refused at verify!
        let forged_long = ring
            .mint(
                "hat-raw-long",
                "fmine://human-authority/lumwana",
                vec![
                    Caveat::Principal {
                        kind: PrincipalKind::Human,
                        id: "qp-alice".into(),
                    },
                    Caveat::Scope("release_effect".into()),
                    Caveat::Site("lumwana".into()),
                    Caveat::EffectDigest(effect),
                    Caveat::IssuedAtNs(t0),
                    Caveat::ExpiresAtNs(t0 + MAX_HUMAN_TOKEN_TTL_NS + 1_000),
                    Caveat::SingleUseNonce([2u8; 16]),
                ],
            )
            .expect("raw mint");
        assert!(matches!(
            verifier.verify_and_consume(&ring, &forged_long, "lumwana", &effect, t0 + 100),
            Err(AuthzError::HumanAuthorityRefused { .. })
        ));

        // Agent principal is NEVER accepted as a HumanAuthorityToken.
        let agent_tok = ring
            .mint(
                "hat-agent",
                "fmine://human-authority/lumwana",
                vec![
                    Caveat::Principal {
                        kind: PrincipalKind::Agent,
                        id: "claude-01".into(),
                    },
                    Caveat::Scope("release_effect".into()),
                    Caveat::Site("lumwana".into()),
                    Caveat::EffectDigest(effect),
                    Caveat::IssuedAtNs(t0),
                    Caveat::ExpiresAtNs(t0 + 60_000_000_000),
                    Caveat::SingleUseNonce([3u8; 16]),
                ],
            )
            .expect("mint agent");
        assert!(matches!(
            verifier.verify_and_consume(&ring, &agent_tok, "lumwana", &effect, t0 + 100),
            Err(AuthzError::HumanAuthorityRefused { .. })
        ));

        // Valid 15-minute token: wrong effect digest & wrong site fail WITHOUT consuming nonce.
        let tok = mint_human_authority_token(
            &ring,
            &HumanAuthorityMintRequest {
                token_id: "hat-1".into(),
                human_id: "qp-alice".into(),
                site: "lumwana".into(),
                effect_digest: effect,
                issued_at_ns: t0,
                ttl_ns: MAX_HUMAN_TOKEN_TTL_NS,
                single_use_nonce: [4u8; 16],
            },
        )
        .expect("mint valid");
        assert!(
            verifier
                .verify_and_consume(&ring, &tok, "lumwana", &other_effect, t0 + 10)
                .is_err()
        );
        assert!(
            verifier
                .verify_and_consume(&ring, &tok, "reko_diq", &effect, t0 + 10)
                .is_err()
        );
        // Expired by 1 ns fails.
        assert!(matches!(
            verifier.verify_and_consume(
                &ring,
                &tok,
                "lumwana",
                &effect,
                t0 + MAX_HUMAN_TOKEN_TTL_NS + 1
            ),
            Err(AuthzError::TokenTemporalInvalid { .. })
        ));
        assert_eq!(verifier.consumed_count(), 0);

        // Exact expiry boundary succeeds ONCE, and immediate replay fails!
        let grant = verifier
            .verify_and_consume(&ring, &tok, "lumwana", &effect, t0 + MAX_HUMAN_TOKEN_TTL_NS)
            .expect("exact expiry boundary succeeds");
        assert_eq!(grant.human_id, "qp-alice");
        assert_eq!(verifier.consumed_count(), 1);
        assert!(matches!(
            verifier.verify_and_consume(&ring, &tok, "lumwana", &effect, t0 + 10),
            Err(AuthzError::HumanAuthorityRefused { .. })
        ));

        // Dual attestation: same human signing twice refused without consuming either nonce;
        // two distinct humans succeed and consume both nonces.
        let tok_a1 = mint_human_authority_token(
            &ring,
            &HumanAuthorityMintRequest {
                token_id: "hat-dual-a1".into(),
                human_id: "qp-alice".into(),
                site: "lumwana".into(),
                effect_digest: effect,
                issued_at_ns: t0,
                ttl_ns: 300_000_000_000,
                single_use_nonce: [5u8; 16],
            },
        )
        .expect("a1");
        let tok_a2 = mint_human_authority_token(
            &ring,
            &HumanAuthorityMintRequest {
                token_id: "hat-dual-a2".into(),
                human_id: "qp-alice".into(),
                site: "lumwana".into(),
                effect_digest: effect,
                issued_at_ns: t0,
                ttl_ns: 300_000_000_000,
                single_use_nonce: [6u8; 16],
            },
        )
        .expect("a2");
        let tok_b = mint_human_authority_token(
            &ring,
            &HumanAuthorityMintRequest {
                token_id: "hat-dual-b".into(),
                human_id: "met-bob".into(),
                site: "lumwana".into(),
                effect_digest: effect,
                issued_at_ns: t0,
                ttl_ns: 300_000_000_000,
                single_use_nonce: [7u8; 16],
            },
        )
        .expect("b");

        assert!(
            verifier
                .verify_dual_attestation(&ring, &tok_a1, &tok_a2, "lumwana", &effect, t0 + 10)
                .is_err()
        );
        // tok_a1 was not burned by failed dual check, so (a1, b) succeeds cleanly.
        let dual = verifier
            .verify_dual_attestation(&ring, &tok_a1, &tok_b, "lumwana", &effect, t0 + 10)
            .expect("distinct dual attestation");
        assert_eq!(dual.first.human_id, "qp-alice");
        assert_eq!(dual.second.human_id, "met-bob");
        assert_eq!(verifier.consumed_count(), 3);
    }
}
