# CONTRACT.md — `fmine-report`

## 1. Purpose
Adit U-layer deterministic report renderer (`BD-123`, `BD-124`, plan §30.5, §39 WP-06) exercising the explicitly authorized `FD-FMD-00` plain-Markdown fallback (`franken-stack.lock`: `pin = "OPEN"`, fallback `"Plain markdown rendering without diagrams."`, plan §39 BD-123):
- renders checked `Claim<T>` rows through `fmine-claim::render::build_fm_row`, preserving G-EXEC numeric suppression (`§7.2`), clickable `fmine:clm_...` links, evidence badges, KK/KU/UU tags, colours, calibration flags, flip margins, and UU terms;
- embeds canonicalized source anchors, execution/verification certificates, `NO_CLAIM_BANNER`, and the four always-`false` `INV-002` authority flags;
- binds every rendered report to deterministic `markdown_sha256`, `markdown_blake3`, and canonical `RPT1` `canonical_inputs_digest`, guaranteeing byte-identical reruns and tamper refusal (`FMINE-REF-505`).

It performs zero filesystem I/O, zero network I/O, zero clocks, and zero threads.

- **Crate:** `fmine-report`
- **Layer:** U
- **Subsystem:** Adit
- **LOC budget:** 12,000 planned / 14,000 ceiling
- **Allowed Module Families:** `["report"]`

## 2. Inputs
- `ReportSpec`: title, optional subtitle, lane label, optional issuer non-affiliation disclaimer, reader `Confidentiality` clearance, non-empty `ReportAnchorRef` list, non-empty `ReportCertificateRef` list, and ordered `ReportSection`s containing `ReportClaimEntry` rows built via `ReportClaimEntry::from_claim`.
- Dependencies: `fmine-core`, `fmine-evidence`, `fmine-claim` (`U → S0`, zero external crates).

## 3. Outputs
- `RenderedReport`: canonical LF-terminated UTF-8 Markdown (`FD-FMD-00` plain-Markdown fallback without diagrams), `markdown_sha256`, `markdown_blake3`, `canonical_inputs_digest`, `ReportAuthorityFlags` (all four `INV-002` flags unconditionally `false`), and summary counts.
- `render_report_metadata_json`: deterministic single-line JSON envelope carrying all digests, counts, renderer tag, `no_claim`, and the four `INV-002` `false` flags.
- Emits no new `Claim`s and holds zero authority or control write paths (`INV-002`, `INV-011`).

## 4. Invariants
- **INV-002 non-authority by type:** `ReportAuthorityFlags` exposes only `const fn` getters returning `false` for `reserve_disclosure_claim`, `execution_authorized`, `geotechnical_signoff`, and `regulatory_compliance_claim`; every rendered Markdown document embeds `NO_CLAIM_BANNER` and the four `false` flags (`test_report_renders_anchors_certificates_g_exec_and_no_claim_banner`).
- **Confidentiality & Claim integrity:** `ReportClaimEntry::from_claim` delegates to `fmine_claim::render::build_fm_row`, enforcing `verify_export_clearance` (`FMINE-REF-301`/`302`) and `check_id` (`FMINE-REF-507`), plus §7.2 G-EXEC numeric suppression (`test_report_refuses_mnpi_clearance_violation_tampered_claim_and_missing_anchors`).
- **Byte-identical determinism & tamper refusal (`BD-124`):** anchors, certificates, and within-section claim rows are canonicalized before rendering; rerenders and anchor/certificate/claim permutations produce bit-identical UTF-8 Markdown bytes and identical SHA-256 and BLAKE3 digests; any single-byte edit to `markdown` is refused by `verify_rendered_report` with `FMINE-REF-505` (`test_report_determinism_by_digest_byte_identical_rerender_and_order_invariance`).

## 5. Refusals
- `FMINE-REF-107` (exit 3): empty title, lane label, section heading, anchor/certificate field, or malformed 64-character lowercase hex digest.
- `FMINE-REF-201` (exit 3): missing anchors or missing certificates (every report must carry at least one anchor and one certificate).
- `FMINE-REF-301` / `FMINE-REF-302` (exit 7): claim confidentiality exceeds `reader` clearance (`INV-007`).
- `FMINE-REF-505` (exit 8): rendered report Markdown digest mismatch or stripped `NO_CLAIM_BANNER` detected by `verify_rendered_report`.
- `FMINE-REF-507` (exit 6): tampered `Claim` whose `claim_id` does not match its canonical body.

## 6. Determinism class
`src/lib.rs` declares `//! Determinism: BitwisePortable`. It uses zero floating-point arithmetic, zero transcendentals, zero hash-map iteration, and canonical LF line endings with BLAKE3 and SHA-256 digests from `fmine-core`.

## 7. Evidence class of outputs
Not applicable: presentation layer over caller-supplied `Claim<T>` values; never upgrades evidence class or knowledge state (`INV-006`).

## 8. Validity domain
- Plain-Markdown reports without Mermaid diagrams under the `FD-FMD-00` fallback (`docs/delivery/FD-FMD-00-FALLBACK.md`).
- Anchor and certificate digests must be 64-character lowercase hex strings.
- Does not verify external ledger signatures or open files on disk; callers verify anchors/certificates before supplying references.

## 9. Oracles
- Known-answer rendering, G-EXEC suppression, MNPI/tamper refusal, and byte-identical permutation/rerender digest tests in `src/lib.rs` (`test_report_renders_anchors_certificates_g_exec_and_no_claim_banner`, `test_report_determinism_by_digest_byte_identical_rerender_and_order_invariance`, `test_report_refuses_mnpi_clearance_violation_tampered_claim_and_missing_anchors`).

## 10. Known gaps
- `franken_markdown` and `frankenmermaid` remain `pin = "OPEN"` in `franken-stack.lock` (`FD-FMD-00`), so diagram rendering and `INT-FMD-001` / `INT-FMERM-001` rich Markdown extensions are deferred until upstream crates are pinned; the plain-Markdown fallback authorized by plan §39 (`BD-123`) is active (`docs/delivery/FD-FMD-00-FALLBACK.md`).
- Wiring `fmine-report` into `fmine-cli` for the M0 Barrick report belongs to `BD-072` and `BD-265`.

## 11. Public API
- `REPORT_FORMAT_VERSION`, `FD_FMD_00_FALLBACK_TAG`, `REPORT_NO_CLAIM_BANNER`
- `ReportAuthorityFlags` (`new`, `reserve_disclosure_claim`, `execution_authorized`, `geotechnical_signoff`, `regulatory_compliance_claim`)
- `ReportAnchorRef`, `ReportCertificateRef`, `ReportClaimEntry` (`from_claim`, `from_row`), `ReportSection`, `ReportSpec`
- `RenderedReport`
- `render_report`, `verify_rendered_report`, `render_report_metadata_json`

## 12. Proof obligations
- `BD-123` (`fmine-report` under `FD-FMD-00` plain-Markdown fallback with anchors, certificates, G-EXEC suppression, and `INV-002` no-claim lines): `tests::test_report_renders_anchors_certificates_g_exec_and_no_claim_banner`, `tests::test_report_refuses_mnpi_clearance_violation_tampered_claim_and_missing_anchors`.
- `BD-124` (report determinism by digest; byte-identical rerender and tamper refusal): `tests::test_report_determinism_by_digest_byte_identical_rerender_and_order_invariance`.
