# GATE-HELM-02 — finite-model information value

Verdict: PASS (native synthetic known-answer run, 2026-11-25).

**FACT.** The paired Monte Carlo estimator met the [plan §22.7 criterion](../../COMPREHENSIVE_PLAN_FOR_THE_DESIGN_OF_FRANKENMINING.md).
The fixed test used 12 finite decision problems, 100 seeds per problem and 2,000 draws per seed.
The χ² statistic was 5.8724 on 12 degrees of freedom. Its upper-tail probability of 0.9224 fell
within the required two-sided acceptance interval [0.005, 0.995]. All three planted controls
failed as intended when the estimates were biased by half a standard error or the reported
standard errors were divided or multiplied by three.

**FACT.** All 18 VOI tests passed on macOS arm64 with the pinned Rust toolchain. The tests include
the GB-29 hand calculations and an independent exhaustive finite-policy oracle. A two-point sample
checked the mean and standard error at three payoff scales, including 2⁻⁹⁰⁰ and 2⁹⁰⁰. A constant sample
that misses a possible rare gain keeps its sign indeterminate. A constant gain across the declared
support retains its sign. An unrepresentable sampled net value returns an arithmetic refusal.
The tested `voi.rs` Git blob is `f6bfe1fde76f7e234934dd7842f4cbb6ac6117e2`.

```bash
rch exec -- env CARGO_TARGET_DIR="${RCH_TARGET_BASE:-${TMPDIR:-/tmp}}/rch_target_fmine" \
  cargo test --locked --offline --target aarch64-apple-darwin -j 4 \
  -p fmine-numerics --lib voi::tests -- --nocapture
```

**OPEN (bd-227).** Hosted confirmation is pending. The exercise covers the estimator's agreement
with finite enumeration on synthetic problems. Continuous nested simulation, site likelihood
calibration and observed testwork returns require separate evidence.
