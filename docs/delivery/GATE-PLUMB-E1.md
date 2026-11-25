# GATE-PLUMB-E1 — No-Laundering Algebra Property Gate (NL-1…NL-8)

- **Gate ID:** `GATE-PLUMB-E1` (plan §10.6, `PO-PLUMB-1`, `BD-042`)
- **Claim type:** Known answers / algebraic invariant property tests (NOT reconciliation or performance)
- **Date:** 2026-11-25

Verdict: PASS

The implementing agent verified the gate on RCH `ovh-a`. Independent GitHub Actions CI is pending.

## 1. Scope Verified

All eight non-laundering laws (`§7.3`, `NL-1..NL-8`) are enforced in [`fmine-claim`](../../crates/fmine-claim/src/lib.rs) and wired into a production K1 derivation path in [`fmine-explore`](../../crates/fmine-explore/src/three_part.rs):

| Law | Requirement (§7.3) | Enforcement | Proof Test |
|---|---|---|---|
| **NL-1** | Derived colour composes inputs, never exceeds weakest input | `compose_binary`, private `color_basis` (`compile_fail,E0616`) | `test_gate_plumb_e1_nl1_to_nl8_100k_dags`, `test_random_claim_dags_nl1_nl2_nl5_100k` |
| **NL-2** | Derived evidence class is greatest lower bound under partial order | `EvidenceClass::meet` in `compose_binary` | `test_gate_plumb_e1_nl1_to_nl8_100k_dags`, `test_downgraded_supersedes_and_never_raises` |
| **NL-3** | Attestation is orthogonal; signatures change `authority` only | `ClaimAuthority`, `Claim::attest` (`FMINE-REF-304` on empty/zero) | `test_nl3_attestation_is_strictly_orthogonal`, `test_gate_plumb_e1_nl1_to_nl8_100k_dags` |
| **NL-4** | Calibration is never inherited across derivation or support | `ModelledCalibrated → ModelledUncalibrated`, `Transferred { Inside } → NotAssessed`, `with_calibration_score` ignored on derived claims | `test_nl4_model_and_transfer_calibration_do_not_cross_derivation_or_support`, `test_gate_plumb_e1_nl1_to_nl8_100k_dags` |
| **NL-5** | Knowledge states never upgrade under conjunction (all 9 states) | `KnowledgeState::and`, `EpistemicStatus::and` | `test_gate_plumb_e1_nl1_to_nl8_100k_dags` |
| **NL-6** | Refusals are absorbing unless an active declared fallback exists | `compose_binary`, `compose_binary_with_fallback`, `DeclaredFallback` (`PO-PLUMB-15`) | `test_nl6_refusal_absorption_and_declared_fallback_bounds`, `test_gate_plumb_e1_nl1_to_nl8_100k_dags`, `fmine-explore::three_part::tests::test_three_part_claim_composition_dag_and_mapmark_fallback` |
| **NL-7** | Confidentiality is a monotone join (`MNPI ∨ public = MNPI`) | `Confidentiality::join`, `verify_export_clearance` (`FMINE-REF-301`/`302`) | `test_gate_plumb_e1_nl1_to_nl8_100k_dags`, `test_confidentiality_lattice_and_export_guard` |
| **NL-8** | `calibration_score` computed (never hand-entered), `0.0` when Uncalibrated; `fallback_active` mirrored | Private-field `CalibrationScore` (`compile_fail,E0451`), `CalibrationScore::compute`, `Claim::calibration_score`, `Claim::fallback_active` | `test_nl8_calibration_score_formula_and_refusals`, `test_gate_plumb_e1_nl1_to_nl8_100k_dags` |

## 2. Production Wiring

- `fmine-explore::three_part::compose_three_part_endowment_claim` composes elicited deposit counts and grade-tonnage inputs through `fmine_claim::compose_binary` / `compose_binary_with_fallback` (`mapmark4-degenerate-n0`), inserts parents and child into `ClaimDag`, and runs `ClaimDag::guard_disclosure_export`.

## 3. Honesty Disclosure & Remaining Non-Gate Gaps

- **Self-verification only:** Tests were authored and executed by the implementing agent on RCH remote Linux workers (`ovh-a`) and local `xtask check-all`. No independent human reviewer or CI pipeline run has re-verified this gate record yet.
- **Canonical body compatibility:** `authority`, `calibration_score`, and `fallback` remain outside `Claim::canonical_body()` so all pinned M0 public-lane claim IDs and Barrick state roots remain bit-identical. Widening `CLM1` schema remains an owner decision (`crates/fmine-claim/CONTRACT.md` §10).
- **`GATE-PLUMB-E4` remains separate:** `Claim` still carries no `RefClass`, so `G-DISC` remains necessary rather than sufficient for regulatory disclosure.
