# DECISION-BD-012: what "CI blocks merges" means without pull requests

- **Date:** 2026-11-01.
- **Decided by:** SwiftBasin (agent), under the owner's instruction of 2026-11-01 to use expert judgment on all
  decisions. The owner may revise it.
- **Bead:** BD-012 (`bd-012`), done-when "Synthetic overrun blocks a merge"; plan §28.4.3–28.4.4.

## Context

- §28.4.4 says that at the ceiling, "CI blocks merges to that crate".
- AGENTS.md RULE 2 forbids branches and pull requests, so every change is pushed straight to `main`.
- Required status checks on `main` would therefore refuse every push: the checks run after the push.
- On 2026-11-01, `main` has no branch protection (`GET branches/main/protection` answers 404) and no rulesets
  (an empty list).

## Decision

In this workflow, "blocks" means two things.

1. **Before a commit.** `cargo run -p xtask -- check-all` runs `check-amend` against the working tree. It fails a
   change that adds lines:
   - to a crate over its planned budget, unless an approved `BUDGET-AMEND-nnn` record covers it;
   - to a crate over its ceiling, record or not.

   A commit is not made while check-all fails (AGENTS.md, gates before commit).
2. **After a push.** A red "Budget amendments over the pushed range" step in CI stops the line. The next push to
   `main` fixes or reverts the overrun before any other change lands.

## Not claimed

The block is procedural. Nothing technically stops a push that skipped check-all. A ruleset or branch protection
would make it technical, but only together with pull requests, which RULE 2 forbids.

## Evidence

- `test:xtask::loc::tests::test_synthetic_overrun_blocks_a_merge`, with the other nine `loc::tests`, re-run
  2026-11-01 at `b776f61`: 10 passed.
- `ci:41267868225@3fdc08b`: the `ubuntu-latest (dev)` job ran "Budget amendments over the pushed range" and passed.

## Retire when

The owner adds a ruleset or moves to pull requests, which would also mean changing RULE 2.
