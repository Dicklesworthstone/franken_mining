# CONTRACT.md — `fmine-capital`

## 1. Charter & Layer

E-layer Helm capital-program comparison, within the planned 20k/23k LOC budget. This first BD-229 slice
enumerates declared static binary subsets and depends on the permitted `fmine-capital -> fmine-decide` edge.
- **Allowed Module Families:** `["model", "render"]`
The capital-program kernel lives in the crate root.

## 2. Mathematical & Domain Formulation

For each subset, spending is the exact sum of declared candidate costs. Joint scenario delta-NAV is the sum
of selected candidates' **net** payoffs at the same named state. Costs constrain spending and are not
deducted from those payoffs again. Prerequisites, exclusive groups and budget limits are applied before risk.
Every binary subset remains visible, including structurally rejected subsets with named rejection reasons.

Structurally feasible subsets use `fmine-decide::card` for expected value, inverse-CDF P10/P50/P90,
CVaR95 of signed loss `-delta-NAV`, and same-state regret against all structurally feasible subsets. A
configurable exact positive tail mass additionally integrates the lowest-payoff states, including fractional
boundary mass, and negates their mean. An optional signed tail-loss ceiling retains equality. Risk-rejected
subsets retain their numerical outcomes. All expected-value co-winners remain visible before and after risk;
the difference of their maximum means is the expected-value cost of the risk limit.

The empty subset has zero spending and payoffs. If it is the sole structurally feasible subset, its exact
zero statistics are returned. A negative loss limit may exclude it; no risk-feasible program is an explicit
result with empty constrained winners and no invented opportunity cost.

## 3. Authority, Epistemics & Representation

All imported values remain Assumed, Internal, Estimated and parametrically uncertain. A complete input
snapshot includes separate policy, shared-weight and candidate Claims, including every losing candidate.
The output meets evidence, epistemic and knowledge states, joins confidentiality, and retains all input
parents, source locators and consistent UU definitions. Generic Claim metadata omitted from its standard
identity (UU exact bits, computed colours, support, representation, supersession and current flags) is bound
into the program digest together with all typed values and constraints. The unanchored derived output never
acquires a stronger validated regime. Every authority flag remains false.

## 4. Public API Surface

- `ProgramPolicy`, `CapitalCandidate`, `ProgramInputs`: typed monetary declarations wrapped in Claims.
- `evaluate(&ProgramInputs) -> Result<EvaluatedProgram, Refusal>`: exact exhaustive comparison, bounded to
  1–8 candidates and 1–256 shared states (at most 256 subsets).
- `EvaluatedProgram::{inputs, summary, claim, digest, to_json, to_markdown}`: immutable input/result access
  and source-complete reports. Monetary output is rational major USD, without implicit rounding.
- `model::parse(text, SourceAnchor)`: verifies SHA-256 against actual bytes, then reads the strict
  `fmine.capital-program.v1` TOML subset. Top-level strings are `schema`, `model_key`, `question`, `currency`,
  `valuation_basis`, `budget`, `tail_mass`, `payoff_convention`, `reference_class_basis`, with optional
  `max_tail_loss`. Candidate rows require `id`, `label`, `cost`, `reference_class_basis` and optionally
  `exclusion_group`; separate `requires` rows name candidate/prerequisite pairs. State rows name id/weight;
  payoff rows name candidate/state/value; UU rows name id/description/trigger/downside_usd_m.
  Money uses quoted major USD decimals with at most four fractional digits, weights and tail mass exact
  positive `n/d`. The parser bounds text to 4 MiB, rows to 4096 and UU terms to 64. Core admission and
  parsing bound identifiers to 128 bytes and descriptions, bases, source paths and locators to 4096 bytes,
  preventing one large identifier from expanding into every enumerated subset's state statistics.

## 5. Guards & Typed Refusals

Every candidate is admitted before enumeration, including candidates that cannot be selected. Missing or
duplicate states, incomplete payoffs, foreign references, prerequisite cycles, duplicate prerequisites,
currency/basis mismatch, negative costs/budgets, nonpositive or non-unit-sum weights, and invalid tail mass
refuse. Stale, altered, unobservable, uncited, unsupported, calibration-failed or unbudgeted Claims refuse.
Conflicting UU definitions refuse. Exact arithmetic overflow returns `FMINE-REF-207`; no alternative is
dropped to obtain a plausible result. Input/source errors use the existing refusal catalog, including
`FMINE-REF-505` for source SHA mismatch and `FMINE-REF-507` for altered typed Claims.

## 6. Proof Obligations

- Exhaustive output contains exactly `2^n` subsets; constraint rejection never edits the input population.
- Joint risk uses the shared scenario index, never marginal risk sums.
- Every feasible winner satisfies exact budget, dependency, exclusion and optional tail-loss constraints.
- All ties survive, and arithmetic risk cost is nonnegative whenever constrained winners exist.
- Source, losing candidate, constraint, adjustment-basis and evidence metadata changes affect identity.

## 7. Failure Modes & Deterministic Fallbacks

Invalid input and arithmetic overflow refuse the complete comparison. Structural and risk infeasibility
are positive report states. No omitted candidate, state, cash flow, interaction or adjustment is inferred.
Internal temporary bundle Claims keep all input parents and one locator per source artifact to avoid
quadratic duplication of source metadata; the public result retains every original locator and input Claim.

## 8. Gates & Kill Criteria

Direct GB-30 known answers use the plan's explicitly illustrative, displayed inputs: A+D mean $241.18M
and risk cost $130.82M before adding streamed E, without treating rounded plan prose as an exact oracle.
Tests also cover fractional tail mass, dependence, exact boundaries, prerequisite and exclusion constraints,
do-nothing, infeasible risk, metadata and source identity, input permutation and overflow.
This bounded slice does not claim the full GATE-CAP-01/03/04 scale, selection-frequency or historical evidence.

## 9. Lifted Code & Foundation Dependencies

No new external dependencies. Reuses the exact units/rational arithmetic, strict registry TOML parser,
evidence Claim types, and `fmine-decide::card` statistics. No IO, spawning or time APIs occur in this crate.

## 10. Non-Goals & Scope Boundary

Static additive declared net-NAV comparison only. No staged recourse, interactions, price/fiscal model,
reference-class fitting, entity waterfalls, variable shares, portfolio safety, calibrated probabilities or
physical authorization is supplied. Reference-class bases must be explicitly declared, including an honest
unadjusted basis; the system does not apply an unnamed adjustment. Selection frequency is explicitly
unavailable. The broad BD-229 CapitalProgram and production-scale gates remain open.
