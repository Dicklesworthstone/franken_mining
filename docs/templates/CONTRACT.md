# CONTRACT.md — `<crate-name>`

> Per-crate contract template (§0.2, BD-002, BD-327). Every workspace crate carries a `CONTRACT.md` in this form. Sections 1–10 are §0.2's ten; 11 and 12 are BD-002's "plus API, POs". `cargo run -p xtask -- contracts` checks that the twelve level-2 headings below appear exactly as written, in this order, none empty, and that the Layer, Subsystem and LOC budget lines equal the crate's row in `registries/loc_budget.toml`. Write what the code does at the commit; anything planned but not done goes under Known gaps.

## 1. Purpose
What the crate is for, what is out of its scope, and any code lifted from `frankensim` (`registries/lifts.toml`). The Layer, Subsystem and LOC budget lines are written by `cargo run -p xtask -- contracts --write-budget` from the registry; never type them. A dev-only tool with no registry row (`xtask`) has the LOC budget line `- **LOC budget:** not budgeted (dev-only tool; no row in registries/loc_budget.toml)`.

- **Crate:** `<fmine-*>`
- **Layer:** <S0 | S1 | K0..K4 | E | V | U | T>
- **Subsystem:** <Bedrock | Plumb | Lodestar | Corebox | Strata | Highwall | Pushback | Headframe | Crucible | Freeboard | Muster | Touchstone | Bullion | Hallmark | Helm | Portal | Adit | Testpit>
- **LOC budget:** <planned> planned / <ceiling> ceiling
- **Allowed Module Families:** `[<mod_a>, <mod_b>, ...]`

## 2. Inputs
The types and sources the crate accepts, with units (`Qty<D, B, R>`) and bases, and the registered files it reads, if any.

## 3. Outputs
The types the crate emits, whether each is a Claim, and their schemas.

## 4. Invariants
Each `INV-*` and `NL-*` the crate upholds, with the test that shows it.

## 5. Refusals
Each refusal condition, its `FMINE-REF-nnn` code and its exit code. Every code must exist in `registries/refusals.toml`; a refusal is never a clamp.

## 6. Determinism class
The class of each module: Exact, BitwisePortable, DeterministicPerIsa or ThreadVariant (§29.2). Each must equal the module's `//! Determinism:` header.

## 7. Evidence class of outputs
The §7.1 evidence class and §7.2 knowledge state of what the crate emits, and how they are computed (NL-1, NL-2).

## 8. Validity domain
The inputs and ranges outside which outputs are refused or flagged.

## 9. Oracles
The pinned oracles and fixtures, never regenerated from the candidate (§33.2), with the tests that use them; or "none (gap)".

## 10. Known gaps
Everything that keeps the crate partial under §0.2, each with the bead that closes it; or "none" with the gate record that shows it.

## 11. Public API
The primary public types, traits and entry points. In the K and E layers no public signature takes a bare `f64` for a physical quantity (L-6, `xtask check-claims`).

## 12. Proof obligations
Each `PO-*` with the test that discharges it.
