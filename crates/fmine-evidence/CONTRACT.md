# CONTRACT.md — `fmine-evidence`

## 1. Purpose
The epistemic vocabulary every Claim is built from:
- plan §7.1's eight evidence classes, under NL-2's partial order with `meet`;
- §7.2's nine knowledge states, with NL-5's conjunction `and`;
- §8.1's `EpistemicStatus`;
- the three colours, with `compose` (NL-1) and the regime check;
- `project_color`, the last column of the §7.1 table;
- the compiled refusal catalogue (`FMINE-REF-nnn`) and the `Refusal` type.

It stores no claim payloads or provenance DAGs; `fmine-claim` holds those and applies these laws. It emits no Claims.

`src/color.rs` is lifted from frankensim `crates/fs-evidence/src/color.rs` at `20463525d9775a18c1b237d2676f364f367175ab`, under `LicenseRef-MIT-OpenAI-Anthropic-Rider` (`registries/lifts.toml`, guard test `color::tests::test_compose_never_promotes`). Its first line lists the changes and what was left out: typed axes and fs-qty among them, and its identity digests differ from upstream's. `src/lattice.rs` is original, an `[[analogue]]` row of `fs-evidence/src/lib.rs`. `src/refusal_catalog.rs` is generated from `registries/refusals.toml` by `cargo run -p xtask -- gen-refusals` and is never edited by hand.

- **Crate:** `fmine-evidence`
- **Layer:** S0
- **Subsystem:** Plumb
- **LOC budget:** 14,000 planned / 16,000 ceiling
- **Allowed Module Families:** `["color", "epistemic", "lattice", "refusal_catalog"]`

## 2. Inputs
- **Values of its own types.** Classes, states and statuses. `HypothesisSetId` wraps an `fmine_core::id::ObjectId`.
- **Colour payloads, as bare `f64`.** A `Verified` interval's bounds and an `Estimated` dispersion. The dispersion is a relative spread, and `∞` means "no spread claim". A `ValidityDomain` axis is named by a string and bounded in that axis's own unit. `check_regime` reads the current state as a `BTreeMap<String, f64>` keyed by axis.
- **Identity strings.** Dataset, axis and estimator.
- **`Refusal::new`.** A `RefusalCode` and a free-text detail.
- **No files.** The crate reads no files at run time. The catalogue is compiled in.

## 3. Outputs
None is a Claim.
- Classes, states and statuses: the results of `meet`, `and`, `no_stronger_than` and `partial_cmp`. Each has stable `tag()` bytes, which claim ids hash.
- `Color` values from `compose`, `check_regime` and `project_color`. `check_regime` also returns a `Demotion { dataset, axis, value }` when a regime is left, and `regime_demotion` returns it alone.
- `Result<(), ColorPayloadError>` from `validate_color_payload`.
- `Refusal { code, exit_code, what, fix }`. It implements `Display` as `[code] (exit n): what | Fix: fix`, and `std::error::Error`.
- `CATALOG: [RefusalSpec; 33]`, in code order.

## 4. Invariants
- **NL-2.** `meet` is the greatest lower bound of the §7.1 order, so a class never rises through `meet`:
  - `test_le_table_64_pairs` and `test_meet_table_64_pairs` check the order and `meet` against tables written by hand from the plan;
  - `test_meet_le_both_operands` and `test_meet_commutative_associative_idempotent` run over every class with every payload;
  - `test_meet_transferred_elicited_is_assumed` and `test_rank2_classes_incomparable` cover the incomparable middle classes;
  - `test_partial_ord_agrees_with_eq_and_le` checks `PartialOrd` against equality and `no_stronger_than`.
- **NL-3.** A payload is not a strength. Assertions from different sources compare equal in strength, and a transfer's `AoaStatus` only orders transfers: `test_payloads_never_change_the_order_of_assertions_or_promote_transfers`.
- **NL-5.** `and` never upgrades a state. `unknown ∧ known = unknown`, `redacted` absorbs every state, and `not_observable` absorbs all but `redacted`. `not_applicable` is the identity. Tests: `test_and_table_81_pairs`, `test_and_commutative_associative`, `test_not_applicable_is_and_identity`.
- **NL-2 and NL-5 over random DAGs.** `test_random_dags_nl2_nl5_100k`: 10⁵ seeded DAGs of 2 to 16 nodes, the deepest at least four levels. No node is stronger than any ancestor.
- **`EpistemicStatus::and` weakens.** It is commutative and associative, with `KnownKnown` as identity and `UnmodelledBudget` absorbing: `test_epistemic_and_is_a_commutative_associative_weakening`.
- **NL-1.** `compose` never ranks above its weaker (normalised) operand. `test_compose_never_promotes` covers 10⁴ seeded pairs, both orders and all three `IntervalOp`s, including malformed payloads. Other behaviours have their own tests:
  - `test_certified_discipline_composes`: a Verified chain stays Verified, an estimate poisons everything downstream, and an enclosure's width is charged;
  - `test_verified_add_encloses_exact_sum`: a Verified `Add` rounds outward one ulp each way, so it encloses the exact sum, and a `0 × ∞` product gives the whole line;
  - `test_disjoint_regimes_demote_and_overlapping_regimes_intersect`: disjoint regimes demote to Estimated with `∞` dispersion, and overlapping ones intersect;
  - `test_malformed_payloads_demote_and_long_identities_stay_bounded`: a malformed payload demotes, and a composed identity stays within 256 bytes.
- **Regime exit demotes.** A Validated colour becomes Estimated with `∞` dispersion when it is outside its regime, has an unreported axis, gets a non-finite reading, or has no axes. Verified and Estimated colours pass unchanged: `test_check_regime_demotes_validated_outside_domain`.
- **`project_color` never raises a colour.**
  - An `Assertion` has no colour: `test_assertion_is_refused_as_a_colour_leaf_whatever_its_basis`.
  - `Assumed` is Estimated with `∞` dispersion whatever the basis: `test_assumed_is_estimated_with_unbounded_dispersion`.
  - An unanchored class keeps only an Estimated basis, and any stronger basis is ignored: `test_unanchored_classes_never_rise_above_estimated`.
  - An anchored class with no basis is Estimated with `∞` dispersion: `test_anchored_classes_keep_a_validated_basis_and_need_one`.
- **Canonical tags are distinct.** There are 13 class tags, 9 state tags and 6 status tags: `test_tags_are_distinct`.
- **Every refusal carries a catalogue row.**
  - `test_catalog_is_the_enum_in_order_and_refusals_carry_its_rows`: the catalogue equals the enum in order, every code parses back, an unregistered spelling parses to `None`, and `Refusal::new` copies its row's code, exit and fix.
  - `Refusal` is `#[non_exhaustive]`, so outside the crate it is built only through `Refusal::new`. The two `compile_fail` doctests on `Refusal` (`src/lib.rs`) show that a string code and a struct literal do not compile.
  - `test_plan_meanings_on_known_codes` spot-checks the number, exit and status of four codes: 401 and 104 (active), 505 and 506 (proposed). It also checks that 105's fix does not say "clamp".

## 5. Refusals
This crate defines the codes; it raises none. `validate_color_payload` returns a typed `ColorPayloadError` (`InvalidIdentity`, `InvalidVerifiedInterval`, `InvalidValidatedRegime` or `InvalidEstimatedDispersion`), not a `Refusal`. `compose` does not refuse a malformed payload either: it demotes it to Estimated with `∞` dispersion. `project_color` returns `None` for an `Assertion`, and `fmine_claim::Claim::color` turns that into `FMINE-REF-205`.

The catalogue has 33 codes:
- 25 `active`, the codes plan §8.4 lists;
- 8 `proposed`, awaiting the owner's plan amendment (`docs/adr/ADR-REFUSALS-01.md`): 109, 110, 206, 207, 208, 505, 506 and 507.

Exit codes follow §30.1's family rule, which `xtask check-refusals` checks. 1xx and 2xx exit 3, 3xx and 4xx exit 7, and 5xx exit 8 or 6. The 6s are 502 (stale anchor) and 507 (frozen-record mutation). The listed exceptions are 504 (exit 7) and 506 (exit 5).

## 6. Determinism class
Undeclared: as an S0 crate it carries no `//! Determinism:` headers, which `xtask check-det` requires only in the K and E layers. In practice:
- `meet`, `and`, the orders and the tags are integer logic.
- `compose` and `check_regime` use IEEE `+`, `*`, `/`, `min`, `max`, `next_up` and `next_down`, and no transcendental function.
- Identities hash with `fmine_core`'s BLAKE3.

The random tests draw from Philox streams with fixed keys and print those keys in their JSON lines.

## 7. Evidence class of outputs
Not applicable: no Claims are emitted. This crate defines the classes and states. `fmine-claim` computes a Claim's class by `meet` (NL-2), its state by `and` (NL-5), and its colour by `compose` and `project_color` (NL-1).

## 8. Validity domain
- **Colour payloads.** A Verified interval with a NaN, an inverted pair, or a point at infinity is malformed. A half-infinite or whole-line interval is valid but says nothing. A Validated colour needs a valid dataset identity and at least one axis, and every axis must be validly named, finite and ordered. An Estimated dispersion must be neither NaN nor negative. `ValidityDomain::with` reorders reversed finite bounds, and a NaN bound leaves the domain empty.
- **`IntervalOp::Hull`** encloses the result only for operations whose value lies between its operands: min, max, selection and convex combination. `compose` cannot tell which operation the caller meant. `test_hull_is_not_used_for_difference` shows the hull missing a difference.
- **Identities.** At most 256 bytes, ASCII letters and digits plus `-_./:@+=`, not blank, with no surrounding space or control character, and no placeholder token (`tbd`, `n/a`, …). A composed identity that would be longer, or that has an invalid part, becomes a BLAKE3 digest. That digest differs from frankensim's for the same inputs.
- **`project_color` and a Verified basis.** An anchored class (`Measured`, `Derived`, `ModelledCalibrated` or `Transferred { aoa: Inside }`) keeps any basis, Verified included. §7.1 projects `ModelledCalibrated` and an in-AOA transfer to Validated, and reserves Verified for interval enclosures. No test gives an anchored class a Verified basis. In the tree today the basis comes only from `fmine_claim`: `with_regime` makes a Validated basis, and `compose_binary` composes the parents' colours. So no Verified basis reaches `project_color` (Known gaps).
- **Assertion sources.** When two assertions meet, the alphabetically first source survives (Known gaps).

## 9. Oracles
No external oracle or fixture.
- The plan's §7.1 order, NL-2's `meet` and NL-5's conjunction are written out by hand in the tests, apart from the code: `test_le_table_64_pairs`, `test_meet_table_64_pairs` and `test_and_table_81_pairs`. NL-5's full order is a working choice, not the plan's (Known gaps).
- Two upstream frankensim tests are ported to colours: `evd_008` as `test_verified_add_encloses_exact_sum`, and `evd_006` as `test_certified_discipline_composes`.
- `xtask check-refusals` checks the catalogue against plan §8.4 and the registry (`test_committed_registry_catalogue_and_plan_agree`).

## 10. Known gaps
- **`project_color` does not cap anchored classes at Validated** (see Validity domain): bd-wiaq.
- **Owner questions.** Each has its working choice in force. They are recorded here and in the doc comments, with no bead:
  - **Plan erratum.** NL-2 says "the weakest input", but `Transferred` and `Elicited` are incomparable, so there may be no weakest. `meet` gives their lower bound, `Assumed`. It never promotes, and it demotes two mid-tier inputs.
  - **`AoaStatus`.** The plan names `Transferred{aoa}` without defining `aoa`. The working values are `Outside`, `NotAssessed` and `Inside`, weakest first.
  - **Assertion sources.** When two meet, the alphabetically first survives. The rule is arbitrary but total, and both stay reachable through provenance.
  - **NL-5's order.** The plan gives only `unknown ∧ known = unknown` and that `redacted` and `not_observable` persist. The working order, weakest knowledge last, is `NotApplicable`, `Known`, `Estimated`, `Stale`, `Unknown`, `Indeterminate`, `Conflicted`, `NotObservable`, `Redacted`.
  - **Structural hypothesis sets.** Two different `Structural` sets conjoin to the greater id.
- **Proposed codes.** Eight codes await the plan amendment: bd-h0q1.1.
- **BD-037 (bd-037) is open.** The plan's lift is fs-evidence's "closure of 3" modules, types only, with a stable `QuantitySpec` shim. Only `color.rs` is lifted, without typed axes or fs-qty, and there is no `QuantitySpec`. bd-037 waits on bd-023.
- **Not in the crate.** The registry charter also names `EvidenceRow`/`DecisionRow` schema types, which do not exist yet. NL-1's `weakest_color` for headlines is not here either. No bead covers either.
- **PO-PLUMB-1 is only partly here.** NL-2 and NL-5 are checked over 10⁵ DAGs. NL-1 is checked over 10⁴ random pairs, not over DAGs. NL-3, NL-4 and NL-6 to NL-8 belong to other crates (Proof obligations).

## 11. Public API
- `epistemic::{EvidenceClass, AoaStatus, AssertionSource, KnowledgeState, EpistemicStatus, KuKind, HypothesisSetId}`. All are re-exported at the root.
  - `EvidenceClass::{ISSUER, rank, no_stronger_than, meet, tag}` and `PartialOrd`. There is no `Ord`, because the three middle classes are incomparable.
  - `KnowledgeState::{and, tag}`.
  - `EpistemicStatus::{and, tag}`.
- `color::{Color, ColorRank, ColorPayloadError, ValidityDomain, IntervalOp, Demotion, MAX_COLOR_IDENTITY_BYTES}`:
  - functions `compose`, `check_regime`, `regime_demotion`, `intersect_domains`, `validate_color_payload`, `color_identity_reason` and `demotion_estimator_identity`;
  - `Color::{rank, name}`;
  - `ValidityDomain::{unconstrained, with, bounds, intersect, is_empty}`.
- `lattice::project_color(evidence: EvidenceClass, basis: Option<&Color>) -> Option<Color>`, re-exported at the root.
- `Refusal` (`#[non_exhaustive]`) with `Refusal::new(code: RefusalCode, detail: impl Into<String>)`. Its `what` is the code's summary, then `: ` and the detail when there is one.
- `RefusalCode` has one variant per registry row (`Conservation101`, …), with `ALL`, `spec`, `as_str`, `parse(&str) -> Option<Self>` and `Display`.
- `CATALOG`, `RefusalSpec { code, key, exit_code, status, summary, fix }` and `RefusalStatus::{Active, Proposed, Reserved}` with `as_str`.

## 12. Proof obligations
- **PO-PLUMB-1** (NL-1 to NL-8 hold over 10⁵ random provenance DAGs) is in part:
  - NL-2 and NL-5 by `test_random_dags_nl2_nl5_100k`;
  - NL-1 by `test_compose_never_promotes`, over 10⁴ pairs rather than DAGs.
  The other rules are not checked here (Known gaps).
- The old contract's PO-EVID-1, PO-EVID-2 and PO-EVID-3 and its GATE-CLAIM-01 are not in the plan. The properties they named are under Invariants.
