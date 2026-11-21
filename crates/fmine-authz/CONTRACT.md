# CONTRACT.md — `fmine-authz`

## 1. Purpose
Authorization, confidentiality enforcement, fail-closed cross-border data residency, and short-lived single-use human effect-release tokens for FrankenMining Bedrock (plan §31, `BD-092`–`BD-095`).

- **Crate:** `fmine-authz`
- **Layer:** S1
- **Subsystem:** Bedrock
- **LOC budget:** 14,000 planned / 16,000 ceiling
- **Allowed Module Families:** `["macaroon", "residency", "human_token", "wall"]`

## 2. Inputs
- Primary and non-zero retired 32-byte HMAC-SHA256 signing keys (`FmKeyRing`), first-party `Caveat` chains, and `FMT-MAC-1` (`FMNE` + `MAC1` + CRC32C) wire bytes (`macaroon.rs`).
- Data residency registries (`registries/residency.toml`, schema `fmine.residency/1`), cross-border transfer queries `(origin_country, DataClass, destination_country, is_aggregate, registered_instruments)` (`residency.rs`).
- Human-minted effect release tokens (`effect_digest`, `site`, `issued_at_ns`, `expires_at_ns`, `single_use_nonce`) and current verification clock (`human_token.rs`).
- `fmine_claim::LabelRegistry`, `fmine_claim::Confidentiality` labels, optional `DisclosureState`, and caveated principal tokens (`wall.rs`).

## 3. Outputs
- Verified first-party `Macaroon` tokens and canonical `FMT-MAC-1` bytes (`macaroon.rs`).
- Fail-closed `ResidencyDecision` (`Allowed` or `Refused` carrying `FMINE-REF-303`) and effective `ResidencyRule` (`residency.rs`).
- `VerifiedHumanGrant` and `DualAttestationGrant` with consumed single-use nonces (`human_token.rs`).
- `CompiledClearance`, wall-scoped `WallQueryResult<T>` (strictly indistinguishable counts/payloads across inaccessible walls), and hash-chained insider-list-ready `AccessLog` of `AccessEvent` entries (`wall.rs`).

## 4. Invariants
- **S3(b) zero-signature & retired-key guard (`BD-092`):** `FmKeyRing` refuses any all-zero active or retired key and always installs a distinct non-zero retired key; `FmKeyRing::verify` unconditionally rejects an all-zero signature before evaluating HMAC (`test_int_as_sec_001_b_random_retired_key_and_zero_signature_rejected`).
- **S3(c) first-party caveats only (`BD-092`):** third-party caveats are refused both at mint (`add_third_party_caveat`) and on wire decode (`test_int_as_sec_001_c_third_party_caveat_refused_at_mint_and_decode`).
- **Fail-closed residency (`BD-094`):** any unregistered `(country, data_class)` pair defaults to `in_country_only`, and any registry attempting a non-`in_country_only` `default_rule` is refused on load; rows with an unregistered `requires` instrument hold `in_country_only` (`test_committed_residency_registry_has_72_rows_and_fail_closed_unknown_pair`).
- **PO-SEC-3 (`BD-095`):** every `HumanAuthorityToken` is minted only for `PrincipalKind::Human`, has `TTL <= 15 min` (`900_000_000_000 ns`), binds one non-zero `effect_digest` and `site`, and its `single_use_nonce` is consumed atomically on first use so replays fail (`test_po_sec_3_human_token_single_use_15min_ttl_and_effect_digest_binding`).
- **PO-SEC-1 & PO-SEC-2 (`BD-093`, `INV-007`):** wall-scoped queries return indistinguishable outputs (counts and absence included) with or without inaccessible wall/venture records, and no embargoed or MNPI-tainted object crosses public export without a human-held `MnpiRelease` capability (`test_po_sec_1_wall_differential_hides_data_counts_and_absence_across_walls`, `test_po_sec_2_embargoed_and_mnpi_derived_bytes_refused_on_public_export`).
- Zero `unsafe` code (`unsafe_code = "forbid"`), zero `unwrap`/`expect`/`panic` in library paths, constant-time 32-byte tag comparison.

## 5. Refusals
- `FMINE-REF-301` (`MnpiWall301`, exit 7): embargoed/MNPI read or export without required wall clearance or human `MnpiRelease` capability (`src/wall.rs`).
- `FMINE-REF-302` (`VentureWall302`, exit 7): cross-venture read/export without matching venture clearance or `CrossVenture` capability (`src/wall.rs`).
- `FMINE-REF-303` (`Residency303`, exit 7): cross-border transfer forbidden by `registries/residency.toml`, `deny_destinations`, unregistered `requires` instrument, or fail-closed default for unknown `(country, data_class)` (`src/residency.rs`).
- `FMINE-REF-304` (`MissingHumanToken304`, exit 7): missing, expired, >15 min TTL, replayed, agent-minted, wrong-site, wrong-effect-digest, or non-distinct dual `HumanAuthorityToken` (`src/human_token.rs`).
- `FMINE-REF-505` (`DigestMismatch505`, exit 8): invalid/all-zero macaroon signature, tampered wire frame/CRC32C, or forbidden third-party caveat (`src/macaroon.rs`).

## 6. Determinism class
- `src/lib.rs`: `BitwisePortable`
- `src/macaroon.rs`: `BitwisePortable`
- `src/residency.rs`: `BitwisePortable`
- `src/human_token.rs`: `BitwisePortable`
- `src/wall.rs`: `BitwisePortable`

## 7. Evidence class of outputs
Policy authorization verdicts and audit events (`AccessEvent`, `VerifiedHumanGrant`, `ResidencyDecision`) are deterministic security controls, not physical measurements; they carry no `EvidenceClass` upgrade and never fabricate or alter `Claim` evidence classes (`INV-006`).

## 8. Validity domain
- Macaroon HMAC-SHA256 chains over 32-byte non-zero keys with up to 64 first-party caveats per token and canonical `FMT-MAC-1` framing.
- `registries/residency.toml` covers the 12 researched mining jurisdictions × 6 data classes (72 rows, W3 `E501–E549`) as of `2026-10-29`; research, not legal advice, with fail-closed `in_country_only` fallback for every unlisted pair.
- `HumanAuthorityToken` enforces wall-clock comparison against caller-supplied explicit `now_ns` (no ambient clock reads in S1 library code).

## 9. Oracles
- `test_hmac_sha256_rfc4231_known_answers`: RFC 4231 HMAC-SHA256 test vectors 1–3 (`src/macaroon.rs`).
- `test_int_as_sec_001_b_random_retired_key_and_zero_signature_rejected`: S3(b) zero-signature & retired-key regression (`src/macaroon.rs`).
- `test_int_as_sec_001_c_third_party_caveat_refused_at_mint_and_decode`: S3(c) third-party caveat refusal (`src/macaroon.rs`).
- `test_committed_residency_registry_has_72_rows_and_fail_closed_unknown_pair`: committed 72-row `registries/residency.toml` and fail-closed unknown pair (`src/residency.rs`).
- `test_po_sec_3_human_token_single_use_15min_ttl_and_effect_digest_binding`: PO-SEC-3 single-use, <=15 min, effect-digest binding, dual attestation (`src/human_token.rs`).
- `test_po_sec_1_wall_differential_hides_data_counts_and_absence_across_walls`: PO-SEC-1 differential wall invisibility (`src/wall.rs`).
- `test_po_sec_2_embargoed_and_mnpi_derived_bytes_refused_on_public_export`: PO-SEC-2 embargo/taint export refusal (`src/wall.rs`).

## 10. Known gaps
- `asupersync` 0.5.0 is not yet linked into `fmine-runtime` (`BD-077` / `INT-AS-001` posture), so `FmKeyRing` implements the standalone pure-Rust HMAC-SHA256 first-party macaroon layer with the exact S3(b)/S3(c) mitigations; once `asupersync` is admitted, a bridge test will wrap `asupersync::security::MacaroonKeyRing` under `INT-AS-SEC-001` (`bd-092`, `bd-099`).
- `HumanTokenVerifier`'s consumed-nonce set is currently in-memory; durable nonce persistence across restarts will attach to `fmine-ledger` / `fmined` (`bd-119`, `bd-180`).
- Full 10^4 generated cross-wall query-pair sweep belongs to `GATE-WALL-01` (`bd-258`, blocked on `bd-093` and `bd-135`).
- Counsel sign-off on W3 §7 residency clauses (`OPEN-B4-01`) and listing-venue insider-list export schema (`OPEN-B4-07`) remain human/legal open decisions.

## 11. Public API
- `AuthzError` (`to_refusal`)
- `macaroon::{KeyId, SecretKey, FmKeyRing, PrincipalKind, Caveat, Macaroon, hmac_sha256, encode_macaroon, decode_macaroon, MAC_FORMAT_MAGIC}`
- `residency::{DataClass, ResidencyRule, ResidencyStatus, ResidencyRow, ResidencyRegistry, ResidencyDecision}`
- `human_token::{MAX_HUMAN_TOKEN_TTL_NS, HumanAuthorityMintRequest, HumanTokenVerifier, VerifiedHumanGrant, DualAttestationGrant, mint_human_authority_token}`
- `wall::{DisclosureState, CompiledClearance, AccessAction, AccessDecision, AccessEvent, AccessLog, WallRecord, WallQueryResult, WallScopedCatalog, compile_clearance}`

## 12. Proof obligations
- `INT-AS-SEC-001 (b)`: `test_int_as_sec_001_b_random_retired_key_and_zero_signature_rejected` (`src/macaroon.rs`).
- `INT-AS-SEC-001 (c)`: `test_int_as_sec_001_c_third_party_caveat_refused_at_mint_and_decode` (`src/macaroon.rs`).
- `BD-094` unknown-pair fail-closed: `test_committed_residency_registry_has_72_rows_and_fail_closed_unknown_pair` (`src/residency.rs`).
- `PO-SEC-3`: `test_po_sec_3_human_token_single_use_15min_ttl_and_effect_digest_binding` (`src/human_token.rs`).
- `PO-SEC-1`: `test_po_sec_1_wall_differential_hides_data_counts_and_absence_across_walls` (`src/wall.rs`).
- `PO-SEC-2`: `test_po_sec_2_embargoed_and_mnpi_derived_bytes_refused_on_public_export` (`src/wall.rs`).
