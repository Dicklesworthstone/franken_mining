# CONTRACT.md — `<crate-name>`

> Per-crate ten-section contract template (§0.2, §28.4, BD-002). Every workspace crate carries a `CONTRACT.md` derived from this template and checked by `xtask check-scope`.

## 1. Charter & Layer
- **Crate:** `<fmine-*>`
- **Layer:** `<S0 | S1 | K0..K4 | E | V | U | T>`
- **Subsystem:** `<Bedrock | Plumb | Portal | Corebox | Strata | Highwall | Pushback | Headframe | Crucible | Freeboard | Muster | Touchstone | Bullion | Helm | Hallmark | Adit | Testpit>`
- **LOC Budget:** `<planned_k>k planned / <ceiling_k>k ceiling` (`registries/loc_budget.toml`)
- **Allowed Module Families:** `[<mod_a>, <mod_b>, ...]`

## 2. Mathematical & Domain Formulation
State the governing equations, algebraic structures, units (`Qty<D, B, R>`), mass/chemical bases, and support conventions implemented by this crate.

## 3. Authority, Epistemics & Representation
Specify what authority objects or derived projections this crate reads/produces, which `EvidenceClass` and `KnowledgeState` transitions apply, and how `KK / KU / UU` (`UuTerm`) are preserved (`NL-1..NL-8`).

## 4. Public API Surface
List the primary public types, traits, and pure kernel entry points. In `K` and `E` layers, public signatures must never accept bare `f64` for physical quantities (`L-6`, `xtask check-claims`).

## 5. Guards & Typed Refusals
List the compile-time type guards and runtime `G-*` guards enforced by this crate, mapping each refusal condition to its `FMINE-REF-*` code, `what`, and `fix`.

## 6. Proof Obligations (`PO-*`)
Enumerate the `PO-*` invariants (conservation, round-trip exactness, metamorphic laws, determinism class `BitwisePortable` vs `TargetDeterministic`) and the tests that discharge them.

## 7. Failure Modes & Deterministic Fallbacks
Document how degraded inputs (`Missing`, `Nil`, `NotApplicable`, `Stale`, `Conflicted`, solver non-convergence, out-of-hull coordinates) are handled deterministically without silent clamping (`INV-006`).

## 8. Gates & Kill Criteria (`GATE-*` / `KILL-*`)
Name the milestone gates (`GATE-*`) and kill criteria (`KILL-*`) that depend on this crate.

## 9. Lifted Code & Foundation Dependencies
List any files lifted from `frankensim` (must match `registries/lifts.toml`) and any pinned foundations consumed (`franken-stack.lock`).

## 10. Non-Goals & Scope Boundary
State what is explicitly out of scope for this crate so domain creep is caught by `xtask check-scope`.
