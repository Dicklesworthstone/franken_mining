# GATE-PLUMB-E4 — Reference-Class Registry & ε-Contamination Gate

- **Gate ID:** `GATE-PLUMB-E4` (plan §9.3, §10.6, `BD-050`)
- **Claim type:** Known answers / algebraic & governance invariants (NOT reconciliation or performance)
- **Date:** 2026-11-25

Verdict: PASS

The implementing agent verified the gate on RCH. Independent GitHub Actions CI is pending.

## 1. Gate Criteria Verified

1. **Zero ε without a `RefClass`:**
   - [`EpsilonContamination`](../../crates/fmine-claim/src/refclass.rs) hides its fields (`compile_fail,E0451`) and refuses any `epsilon_bps > 0` without a registered `RefClassRecord` (`FMINE-REF-205`).
   - `Claim::with_checked_epsilon` and `Claim::with_ref_class` enforce the same check on `Claim<T>` without mutating `CLM1` schema-2 `canonical_body()` (preserving pinned M0 Barrick claim IDs).
2. **Zero unverified reference classes on disclosure-facing claims:**
   - [`registries/refclass.toml`](../../registries/refclass.toml) registers verified reference classes (including `REFCLASS-CAPEX-GREENFIELD` reproducing GB-32's Bertisen & Davis moment-matched Huber mixture) alongside explicitly `unverified` rows (`REFCLASS-TAILINGS-PRELIMINARY`).
   - `ClaimDag::guard_disclosure_export` (`G-DISC`) refuses (`FMINE-REF-304`) both direct disclosure claims and descendants carrying any `RefClassStatus::Unverified` ancestor or unbacked `epsilon_bps > 0`.
   - `compose_binary` propagates `Unverified` reference-class taint so composition with a verified claim never launders an unverified reference class.
3. **Registry enforcement in `xtask check-all`:**
   - [`xtask::refclass::check_refclass`](../../crates/xtask/src/refclass.rs) validates `registries/refclass.toml` through `fmine_claim::refclass::RefClassRegistry::parse` on every `xtask check-refclass` and `xtask check-all`.

## 2. Honesty Disclosure

- **Self-verification only:** Tests were authored and run by the implementing agent via RCH remote Linux workers (`x86_64`) and local checks. No independent human reviewer or CI workflow run has re-verified this gate yet.
- **Canonical body boundary:** `epsilon_contamination` sits outside `CLM1` schema-2 `canonical_body()` so pinned M0 public-lane claim IDs remain unchanged; widening the canonical claim body remains an owner decision.
