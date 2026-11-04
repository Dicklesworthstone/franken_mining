# CONTRACT.md — `fmine-claim`

## 1. Purpose
The universal Claim and the checks around it:
- `Claim<T>` holds a value with its representation, support, additivity, evidence class, epistemic status, knowledge state, confidentiality label, UU terms, parents and anchors. Its id is a hash of a canonical body (§8.1, BD-040). Its colour is computed, never set.
- `Confidentiality` and `LabelRegistry` give the label lattice (§31.1, INV-007).
- `compose_binary` derives a claim from two parents without raising class, state or colour (NL rules, BD-042).
- `ClaimDag` is an in-memory provenance index. It propagates staleness (BD-046, INV-013) and checks G-DISC's evidence clauses.
- `dag::TemporalGraph` checks a root's temporal ancestry at an availability cut, and `dag::replay` replays a generator through a guarded read view (BD-344).
- `reliance` is the issued-output journal. It keeps the integrity of issued bytes apart from permission to reuse them now (BD-347).
- `forecast_journal` is the forecast-operation journal codec, independent of reliance's business ledger (BD-047).

It does no I/O. Storage, locking and key custody belong to the callers in `fmine-io` and `fmine-cli`.

- **Crate:** `fmine-claim`
- **Layer:** S0
- **Subsystem:** Plumb
- **LOC budget:** 16,000 planned / 18,000 ceiling
- **Allowed Module Families:** `["claim", "dag", "label", "uu", "reliance", "forecast_journal"]`

## 2. Inputs
- **`Claim::new`** takes 13 arguments: metric key, value, canonical value tag, representation, support, additivity, evidence class, epistemic status, knowledge state, label, UU terms, parents and anchors. It takes the evidence class and parents as given. Nothing checks that they agree. `stale`, `calibration_failed` and `supersedes` start false or `None`. They are `pub` fields that callers set afterwards.
- **What the id hashes.** `claim_id` is `clm_` and the first 16 bytes of BLAKE3(`clm_` ‖ body). The body is a `CLM1` frame, schema 2: metric key, value tag, evidence class, epistemic status, knowledge state, label, parents and anchors, in that order. Representation, support, additivity, UU terms, `supersedes`, `stale`, `calibration_failed` and the value itself are not hashed. Widening the body would move every claim id, and with them the public lane's state root.
- **Labels.** `LabelRegistry::{venture, wall}` hand out ids and refuse malformed, duplicate or unregistered ones (`LabelError`). Any caller can build a registry, so it is not a trust root. The deployment builds its own.
- **`compose_binary`.** Two claims, the derived value and its tag, a representation, a support, an additivity class and the `IntervalOp` the colour composes under.
- **`dag::TemporalGraph::new`.** `TemporalNode { id, kind, parents, availability, recorded_at }` values. `check` takes a root and a `ConeCut { decision_ns, anchor }`.
- **`dag::replay::verify_replay`.** A graph, root and cut, a `ReplayStore` of canonical bytes under their content ids, and a generator closure.
- **`reliance`.** Journal records as values or bytes, a `Header` (the owner policy), a `ReleaseCapability` proved from the release key, and a `TrustContext` holding an exact head pin and a `TrustedTime`. The runtime supplies the last two. They are never decoded from the journal.
- **No files.** It reads no files or registries.

## 3. Outputs
- **`Claim<T>`**, with `canonical_body()` bytes and `color() -> Result<Color, Refusal>`.
- **`compose_binary -> Claim<R>`.**
  - Class: the meet of the parents' classes.
  - Epistemic status and knowledge state: the `and` of the parents'.
  - Colour: the parents' colours composed under `op`, or none when either parent has none.
  - Label: the join of the parents' labels.
  - `stale` and `calibration_failed`: the OR of the parents'.
  - UU terms: the left list, plus the right's terms with a new `scenario_id`.
  - Anchors: the union of both parents' anchors.
  - Parents: `[left, right]`.
- **`ClaimDag`.** `is_stale` answers. `invalidate_and_propagate` returns the sorted affected ids, including an absent root.
- **`CheckedCone`**, or `ConeError`. The cone holds the root, cut, nodes, availability times, anchor marks and a digest. Only `TemporalGraph::check` can build one.
- **`ReplayCertificate`**, or `ReplayError`. Its fields are private. It binds the root, cut, cone digest, output digest, and the full and guarded read and run receipts.
- **`RelianceLedger`.**
  - It gives the head, records, issuances and copies, and recall by revision or by claim.
  - `check` returns `RelianceCheck { status, current_reuse_allowed, as_of_ns, status_cut_ns }`, where `status` is `Current`, `Withdrawn`, `Superseded` or `Unknown`.
  - `gate_current_reuse` returns `Ok` or a refusal.
  - `append` returns the new `JournalRecord`.

## 4. Invariants
**Claim identity and colour.**
- The body bytes and ids of two claims equal known answers. The answers come from `scripts/oracle/claim_body_known_answers.py`, whose BLAKE3 port passes the official vectors first: `test_canonical_body_and_claim_id_golden`.
- A write to any hashed field is refused by `check_id`, and by `ClaimDag::insert`, which then stores nothing. A write to an unhashed field is not detected: `test_edited_claim_is_refused_and_unhashed_fields_are_not`.
- `downgraded` never raises a class. A changed claim gets a new id, and its `supersedes` names the old one: `test_downgraded_supersedes_and_never_raises`.
- A colour cannot be set. Its basis is private (the `compile_fail,E0616` doctest on `Claim`). `with_regime` counts only for a class that `project_color` lets stand on an anchor: `test_with_regime_keeps_a_validated_basis_only_for_a_class_that_may_stand_on_one`.
- An `Assertion` and a failed calibration have no colour, and the refusal passes to what they feed. A stale claim shows at most `Estimated`: `test_assertion_failed_calibration_and_staleness_never_show_a_strong_colour`.

**Composition.**
- One worked example covers class, state, status, label, UU terms and colour: `test_nl1_to_nl8_non_laundering_composition`.
- `test_random_claim_dags_nl1_nl2_nl5_100k` builds 100,000 random binary trees of 2 to 6 leaves. The leaves draw from nine classes and five states, and half carry a regime. No node outranks a parent in class or state, or its weakest leaf in colour. A node is refused exactly when a leaf under it is. The run must see both refused and `Validated` nodes.

**Labels.**
- Join and `leq` satisfy the semilattice laws, checked exhaustively: `test_join_semilattice_laws_exhaustive`.
- Distinct walls, and distinct ventures, are incomparable: `test_distinct_walls_and_ventures_are_incomparable`. `<` does not compile (the `compile_fail,E0369` doctest in `label.rs`).
- The registry admits only well-formed, unique ids: `test_registry_admits_only_well_formed_unique_ids`.
- Public and internal labels encode as the old one-byte tiers 0 and 1, so no older claim id moves. Any other label is tag 5, then its ventures and walls: `test_encoding_keeps_old_bytes_and_separates_sets`.
- The export check refuses a reader the label is not within: `test_confidentiality_lattice_and_export_guard` and `test_two_venture_composition_refuses_a_single_venture_reader`.

**Support and additivity.** `verify_support_compatible` refuses unequal supports, and `verify_unweighted_additive` refuses any class but `ExtensiveAdditive`: `test_support_and_additivity_guards`.

**Provenance index.**
- Staleness reaches every descendant: `test_claim_dag_staleness_propagation`.
- Invalidation survives arrival order and replay. The test covers all six arrival orders of a three-node chain, with the invalidation at each of four positions, then replays every body: `test_claim_dag_invalidation_is_monotone_across_arrival_orders_and_replay`.
- A stale parent that arrives late invalidates existing descendants: `test_claim_dag_late_stale_parent_invalidates_existing_descendants`.
- A new identity with `supersedes` invalidates the old one and its dependants, but is not stale itself: `test_claim_dag_supersession_invalidates_old_dependants_without_staling_new_version`.
- Self-supersession refuses before the graph changes: `test_claim_dag_rejects_self_supersession_without_mutation`.
- G-DISC's evidence clauses:
  - `test_g_disc_refuses_assertion_in_provenance`.
  - `test_g_disc_refuses_assumed_in_provenance`.
  - `test_g_disc_refuses_a_weak_class_and_a_parent_it_cannot_see`.
  - `test_g_disc_refuses_stale_claims_and_ancestors_before_root_insertion`.
  - The positive path, with an uninserted root: `test_g_disc_accepts_measured_derived_chain`.

**Temporal cone.**
- A derived node is available at the latest of its inputs' availability and its own `performed_at`. A later declaration may delay that. An earlier one refuses.
- A missing time refuses. No instant is substituted.
- Admission uses the cut's decision time and the anchor's per-cell sequence marks. Wall clocks never order cells.
- The cone digest binds the rules version (`temporal-cone/1`), root, cut, relevant marks and the checked metadata. Unrelated nodes do not change it.
- Each frontier violation has a shortest root-to-node path, with ties broken by id.
- Tests:
  - `clean_cone_computes_availability_and_retains_exact_cut`;
  - `known_answer_five_hops_catches_exactly_one_nanosecond_of_lookahead`;
  - `future_variogram_exclusion_and_price_deck_are_distinct_frontier_causes`;
  - `diamond_frontier_uses_shortest_path_with_id_tie_break_and_transitive_edges`;
  - `record_visibility_uses_per_cell_sequence_even_when_clocks_run_backwards`;
  - `missing_times_and_backdated_acts_fail_closed_without_substituted_instants`;
  - `missing_parents_duplicates_cycles_and_source_parents_refuse`;
  - `input_order_unrelated_bad_nodes_and_unrelated_anchor_marks_do_not_change_a_cone`;
  - `iterative_walk_handles_a_deep_chain_without_call_stack_growth`;
  - `bounded_dag_property_each_ancestor_leak_has_the_minimal_order_invariant_path`;
  - `claim_binding_refuses_a_mutated_body_or_omitted_canonical_parent`.

**Replay.**
- The store refuses any replacement, even with identical bytes, and a body whose content id differs.
- The generator runs against a full view and a guarded view. Both runs must give the same output bytes and the same ordered read receipts. Every read must lie in the root's declared ancestry. A denied read fails the replay even if the generator swallows the error.
- The guarded view reuses the root cone's successful admission in the same immutable graph and cut. Declared reads use a sorted membership index instead of traversing their ancestry again. Undeclared reads still receive their own temporal check and cannot pass the final ancestry check; ordered receipts and certificate bytes are unchanged.
- Tests:
  - `clean_replay_rebuilds_exact_output_and_binds_actual_payload_reads`;
  - `deep_shared_ancestry_replays_all_payloads_in_requested_order` (12,000 nodes, shared ancestors and repeated reads);
  - `shared_ancestor_admission_does_not_admit_an_undeclared_descendant`;
  - `store_rejects_wrong_body_and_both_identical_and_changed_replacements`;
  - `unrecorded_future_dependency_fails_even_when_graph_and_output_look_clean`;
  - `unrecorded_pre_cut_dependency_is_not_excused_by_matching_replay`;
  - `guarded_reads_check_anchor_sequence_and_missing_ancestor_metadata`;
  - `missing_body_and_bad_root_fail_before_a_certificate_can_exist`;
  - `unequal_outputs_and_unequal_read_traces_both_refuse`;
  - `unrelated_future_objects_do_not_change_successful_replay`.

**Reliance.**
- An issuance binds:
  - the BLAKE3 of the output bytes;
  - its claim ids, which must be sorted and unique;
  - the actual `LedgerAnchor`;
  - the code revision, binary digest and input-manifest root;
  - the lane, label and public taint;
  - the recipient role and purpose.
- Only a public label is accepted. A public-lane issuance must carry public taint.
- Withdrawal and supersession apply to the output digest across all its copies. Reissue cannot resurrect it. A replacement must already be issued and current, and cannot form a cycle.
- `check` returns `Unknown` when:
  - there is no trusted time;
  - there is no pin, or the pin is not the exact head;
  - the clock's uncertainty exceeds the header's skew bound;
  - the clock reads before the status cut;
  - the cut is older than `max_status_age_ns`, counting both the reader's and the header's uncertainty;
  - the as-of time is in the future.
- An as-of check never allows current reuse.
- `append` needs the capability for this header, and an entry time equal to the runtime's time within the skew bound.
- `replay` checks the previous-line hash, non-regressing times, the owner role and the size bounds.
- Tests:
  - `withdrawn_current_reuse_refuses_while_original_bytes_and_anchor_still_verify`;
  - `offline_stale_future_and_replayed_status_never_become_current`;
  - `replacement_must_exist_and_cannot_form_a_cycle_or_resurrect_either_output`;
  - `capabilities_clock_and_affected_claims_are_checked_before_mutation`;
  - `exact_recall_keeps_recipient_copies_and_distinct_builds_of_identical_bytes` (96 copies);
  - `bounded_codec_preserves_all_bindings_and_tampering_does_not_create_trust` (32,273 claim ids).
- `IssuanceRecord::digest()` identifies the exact issuance, including its producer and recipient metadata; `RelianceLedger::issuance(id)` selects that record through an index rebuilt during replay. Selecting by output digest alone cannot identify which producing revision a forwarded copy belongs to. The index adds no durable fields and does not change any canonical frame or existing digest.

**Forecast journal (BD-047).**
- `forecast_journal` defines opaque forecast-operation records independently of reliance's business ledger. `FJR1` schema-1 FMNE frames bind a sequence, previous exact-line BLAKE3, observed Unix nanoseconds and either a genesis header or exact UTF-8 operation text. Genesis has sequence zero and a zero previous digest.
- The immutable header names the cell and its declared clock-skew bound. Canonical lowercase hex plus LF is the on-disk line; CRC32C and exact re-encoding detect malformed frames.
- Replay checks contiguous sequence, one genesis, nondecreasing observed time and checked earliest/latest clock bounds. Limits are 64 KiB per operation, 256 bytes per cell id, 100,000 records and 64 MiB of encoded journal lines. It does not parse the operation's business schema.
- Tests: the BD-047 direct codec tests retain exact multiline UTF-8 operation text, check frame field order and line-head hashing, mutate every byte, and reject noncanonical hex, truncated frames, bad chains and oversized inputs.

## 5. Refusals
- **`FMINE-REF-102`** (support mismatch; exit 3), from `verify_support_compatible`.
- **`FMINE-REF-103`** (non-additive average; exit 3), from `verify_unweighted_additive`.
- **`FMINE-REF-107`** (undeclared unit, basis or time zone; exit 3), from `reliance`, for malformed or out-of-bound input. The code is the nearest, not an exact fit.
- **`FMINE-REF-110`** (temporal cone violation; proposed, exit 3), from `ConeError::refusal` and `ReplayError::refusal`. It covers inadmissible or incomplete ancestry, backdated availability, a cycle, a broken claim binding, an undeclared or denied read, and unequal replays.
- **`FMINE-REF-201`** (no claim in domain; exit 3), from `reliance`: an unissued artifact, or an affected claim the artifact does not hold.
- **`FMINE-REF-202`** (not observable; exit 3):
  - from `ClaimDag::guard_disclosure_export`, when the claim or an ancestor is stale, including an invalidation recorded before the body arrived;
  - from `gate_current_reuse`, when current reuse is not allowed;
  - from `reliance`, for reissuing a withdrawn or superseded output, or naming a replacement that is not current.
- **`FMINE-REF-204`** (unbudgeted unknowns; exit 3), from `verify_uu_slot_for_decision`: a status other than `KnownKnown` with no UU term.
- **`FMINE-REF-205`** (calibration required; exit 3), from `Claim::color`: a failed calibration, or an unreviewed `Assertion`. §8.4 has no code for the second, so this is the nearest.
- **`FMINE-REF-301`** (MNPI wall; exit 7):
  - from `verify_export_clearance`, for a wall the reader lacks, or internal data going to a public reader (§8.4 has no code for the second);
  - from `reliance`, for an issuance whose label is not public, or a public-lane issuance without public taint.
- **`FMINE-REF-302`** (venture wall; exit 7), from `verify_export_clearance`: the reader covers every wall but not every venture.
- **`FMINE-REF-304`** (missing human token; exit 7):
  - from `guard_disclosure_export`, for a class outside {Measured, Derived, ModelledCalibrated}, an `Assumed` or `Assertion` in the provenance, or a parent the index does not hold. §8.4 has no G-DISC code.
  - from `reliance`, for a wrong release key or role, or an append without the runtime's time.
- **`FMINE-REF-502`** (stale anchor; exit 6), from `reliance`: a broken chain, a codec failure, or a time that regresses.
- **`FMINE-REF-505`** (digest mismatch; proposed, exit 8), from `RelianceLedger::verify_artifact`: the bytes do not hash to the issued digest.
- **`FMINE-REF-507`** (frozen record mutation; proposed, exit 6):
  - from `Claim::check_id`, `ClaimDag::insert` and `guard_disclosure_export`, when the id does not name the current body;
  - from `insert`, for a self-supersession;
  - from `ReplayStore::insert`, for a replacement or a body whose content id differs;
  - from `reliance`, for a forbidden replacement or issuance change.
- Forecast journals use `107` for invalid bounded fields/time arithmetic and `502` for noncanonical bytes, malformed chains, incomplete tails and regressing observed time. Forecast semantics retain their own refusals.

`docs/adr/ADR-REFUSALS-01.md` proposes 110, 505 and 507. It asks the owner to confirm 301 for internal data, and leaves G-DISC's code open (`bd-306n.3`). It does not list this crate's other nearest-code uses: 107 in `reliance`, 205 for an `Assertion`, and 304 for G-DISC.

## 6. Determinism class
- **`src/reliance.rs`** states BitwisePortable in its first doc line. It is not in the `//! Determinism: <class>` form that `xtask check-det` parses, so the check does not see it. The file uses no floating point.
- **`src/lib.rs`, `src/label.rs`, `src/dag.rs` and `src/dag/replay.rs`** have no header. An S0 crate needs none: `check-det` requires one only in K and E layers.
- **Ids and digests** are canonical codec bytes under BLAKE3. Times are `i64` nanoseconds. Ordered maps and sorted vectors fix every traversal and diagnostic. `dag.rs` uses hash maps only as lookup tables.
- **Floats** appear only in `UuTerm::downside_exposure_usd_m` and the infinite dispersion of a stale colour. Neither is hashed.

## 7. Evidence class of outputs
- **The crate assigns no class from data.** `Claim::new` takes the caller's class. `compose_binary` gives the meet of the parents' classes, and `downgraded` can only lower one.
- **Colour** comes from `fmine_evidence::color::project_color`, over the class and the anchor basis, capped at `Estimated` when stale.
- **The other outputs carry no evidence class.**
  - A `CheckedCone` says the supplied metadata was admissible at the cut, not that it is complete or authentic.
  - A `ReplayCertificate` says two mediated runs matched.
  - A reliance `Current` says the journal, under an exact pin and a trusted time, records no withdrawal or supersession. It says nothing about whether the output is right.

## 8. Validity domain
- **`ClaimDag` is in memory only.** It has no durable format and does not authenticate the unhashed `supersedes`. `insert` does not check for cycles. A cycle would need a claim to name a descendant's id among its hashed parents. Visited sets stop every walk.
- **G-DISC passing is necessary, not sufficient.** Its RefClass and attestation clauses wait on fields `Claim` does not have.
- **Support compatibility is exact equality**, including composite length and SMU dimensions. There is no change-of-support model.
- **The temporal check covers supplied evidence.** It does not authenticate clocks or prove that the parent lists are complete.
- **Replay proves only reads through `ReplayView`.** Captured values, external I/O and cached results are out of its reach. Content-id checks cover the encoded bytes only. A `Claim` body commits its value tag, not its value.
- **Reliance bounds.**
  - 4 MiB a frame and 64 MiB in all.
  - 100,000 records, and 100,000 claim ids an issuance.
  - 256 bytes a text field.
- **A self-consistent chain is not authority.** A consumer needs an independently obtained pin of the exact current head.
- **Recall covers only the known issued population.** It sends nothing, and cannot find detached copies.
- **Roles are a closed vocabulary:** `release-owner`, `sponsor`, `reviewer-pool-<n>` and `agent-session:<32 hex>`. There is no name, email or contact field.

## 9. Oracles
- **`scripts/oracle/claim_body_known_answers.py`** gives the claim body bytes and ids. It checks its BLAKE3 port against the official vectors before printing any answer.
- **Hand-built temporal known answers**, such as five hops with one nanosecond of lookahead: `known_answer_five_hops_catches_exactly_one_nanosecond_of_lookahead`.
- **Seeded generated populations.**
  - The random claim trees use a Philox stream.
  - The reliance tests generate 96 recipient and build copies and 32,273 claim ids.
- **No fixture** under `fixtures/`.

## 10. Known gaps
- **The old contract had the wrong registry lines.** It said Bedrock and 14k/16k. The registry says Plumb and 16,000/18,000.
- **The old contract listed a dependency on `fmine-units`.** The crate depends on `fmine-core`, `fmine-evidence` and `fmine-time` only. `workspace_topology.toml` allows the `fmine-units` edge, but it is unused.
- **The old contract named GATE-CLAIM-01 and PO-CLAIM-1 to PO-CLAIM-3.** The plan has none of them. The plan's rows are in Proof obligations.
- **The old contract's "fails closed" claim for composition is false.**
  - It said adding `IntensiveMassWeighted` claims without mass weights, or combining mismatched supports, fails closed with 103 or 102.
  - No `IntensiveMassWeighted` exists. The variant is `IntensiveWeighted`.
  - `compose_binary` calls neither guard.
  - `verify_support_compatible` and `verify_unweighted_additive` have no caller outside this crate's tests.
- **The old contract said "every claim enforces Confidentiality isolation".** It holds only where a caller runs `verify_export_clearance`. The callers are `fmine-public`'s `key/inspect.rs`, `fmine-cli`'s `reliance_binding.rs` and `fmine-metrics`'s recovery tests.
- **Composition and the provenance index have no production caller.**
  - `compose_binary` is called only from tests here and in `xtask`.
  - `ClaimDag`, including `guard_disclosure_export`, has no caller outside this crate.
  - The callers outside this crate are: `downgraded` (`fmine-calib`), `TemporalGraph` (`fmine-cli` forecast provenance and `fmine-decide`), replay certificates (`fmine-decide`), and `RelianceLedger` (`fmine-io`, `fmine-cli`).
- **PO-PLUMB-1 is only partly discharged.**
  - The random test checks colour, class and state over trees. It has no `Stale` state and no shared parents.
  - It is a seeded loop, not a proptest.
  - Only one example covers the label join and UU terms.
  - NL-3 (attestation is orthogonal) and NL-4 (calibration is not inherited) have no test.
- **NL-4 may be unmet.** A claim composed from two `ModelledCalibrated` parents stays `ModelledCalibrated`, and its colour can be `Validated`. The owner should confirm whether the plan's NL-4 forbids that.
- **The code's NL labels differ from the plan's.** The code calls staleness NL-7, but the plan's NL-7 is the confidentiality join. The code calls `calibration_failed` NL-8, but the plan's NL-8 is a computed `calibration_score`, which does not exist here.
- **`Claim::new` trusts the caller's class and parents.** A claim can state `Derived` over an issuer figure. G-DISC catches that only when the issuer figure is a listed parent. A `Derived` claim with no parents passes its walk.
- **The id leaves fields unhashed.** Representation, support, additivity, UU terms, `supersedes` and the value itself are outside the body, and every field stays `pub`. Widening the body moves every claim id. That is an owner decision.
- **§31.1 writes the labels as a chain.** The product lattice refines it, so two ventures or two walls never collapse into one tier. The owner should confirm the refinement.
- **G-DISC is incomplete (GATE-PLUMB-E4).** `Claim` has no RefClass or attestation field. FrankenMining's own public report is not a disclosure and does not call the guard.
- **Three codes are proposed, not active:** 110, 505 and 507.
- **Four nearest-code uses are unconfirmed.** ADR-REFUSALS-01 covers 301 for internal data and leaves G-DISC's code open. It does not list 107 for malformed reliance input, 205 for an unreviewed `Assertion`, or 304 for G-DISC.
- **No `//! Determinism:` header** in four files, and the one in `reliance.rs` is not in the form `check-det` parses.
- **No `claim` or `uu` module.** Those families are the code in `src/lib.rs`.
- **The forecast journal is an integrity format**, not a forecast evaluator, authority service or external timestamp proof. An internally valid prefix cannot prove that no suffix was removed; rollback detection needs an independently retained final-head pin. Operation validity is enforced by the consuming calibration replay.
- **A code comment points at the old layout.** It sits on `canonical_body` in `src/lib.rs` and cites "CONTRACT §2". The hashed-body rule is now in Inputs.

## 11. Public API
- **Claim.** `ClaimId`, `Representation`, `SpatialSupport`, `AdditivityClass` and `UuTerm`. `Claim<T>`, with:
  - `new`, `value_tag`, `canonical_body` and `check_id`;
  - `downgraded` and `with_regime`;
  - `color`, `verify_export_clearance` and `verify_uu_slot_for_decision`.
- **Labels.** `Confidentiality`, with `public`, `internal`, `venture`, `mnpi`, `join`, `leq`, `is_public`, `ventures`, `walls` and `refusal_over`. `LabelRegistry`, with `new`, `venture` and `wall`. `VentureId`, `WallId` and `LabelError`.
- **Free functions.** `verify_support_compatible`, `verify_unweighted_additive` and `compose_binary`.
- **Provenance index.** `DagNode`, and `ClaimDag`, with `new`, `insert`, `guard_disclosure_export`, `invalidate_and_propagate` and `is_stale`.
- **`dag`.** `RULES_VERSION`, `NodeKind`, `Availability`, `TemporalNode`, `ConeCut`, `CheckedCone`, `FrontierViolation`, `ConeViolation`, `TimeField` and `ConeError` (with `refusal`). `TemporalGraph`, with `new`, `node`, `nodes`, `verify_claim` and `check`.
- **`dag::replay`.** `ReplayStore`, `ReadReceipt`, `RunReceipt`, `ReplayError`, `ReplayView`, `ReplayCertificate` and `verify_replay`.
- **`reliance`.**
  - Bounds: `MAX_RECORD_BYTES`, `MAX_JOURNAL_BYTES`, `MAX_RECORDS`, `MAX_CLAIMS` and `MAX_TEXT`.
  - Records: `RoleId`, `Header`, `IssuanceLane`, `IssuanceRecord`, `DefectClass`, `Withdrawal`, `Supersession`, `StatusRefresh`, `Entry` and `JournalRecord`.
  - Trust: `ReleaseCapability`, `TrustedTime` and `TrustContext`.
  - Status: `UnknownReason`, `RelianceStatus` and `RelianceCheck`.
  - `RelianceLedger`, with:
    - `from_records`, `head`, `header`, `records` and `status_cut_ns`;
    - `issuances`, `issuance`, `copies`, `recall_revision` and `recall_claim`;
    - `verify_artifact`, `replay`, `append`, `check` and `gate_current_reuse`.
  - `issuance(id)` resolves an exact canonical `IssuanceRecord::digest()`, distinct from `copies(output_digest)`.
- **`forecast_journal`.** `Header`, `Entry`, `Record`, `validate_chain` and `decode_journal`. `Record` exposes bounded binary and line codecs, `chain_head` and `time_bounds`; `Header::time_bounds` returns checked earliest/latest instants. The pure codec never obtains a clock or performs filesystem I/O; live timestamps belong to the locked adapter.

## 12. Proof obligations
The plan defines no `PO-CLAIM-*`. The old contract's PO-CLAIM-1 to PO-CLAIM-3 were local labels. Their properties are tested here, under Invariants. The plan's rows that name this crate's work:
- **PO-PLUMB-1** (NL-1 to NL-8 over 10⁵ random provenance DAGs): partly discharged.
  - Colour, class and state: `test_random_claim_dags_nl1_nl2_nl5_100k`.
  - The rest is example-only or untested (Known gaps).
  - The plan's "fs-evidence `compose`" is `fmine_evidence::color::compose` here.
- **PO-PLUMB-2** (taint monotone; no Full-basis bytes in Public exports): not discharged here.
  - The label join is monotone: `test_join_semilattice_laws_exhaustive`.
  - A reliance issuance accepts only a public label.
  - The Full-basis taint and the export fuzz are not in this crate.
- **PO-PLUMB-8** (append-only, hash-chained ledger; bit-identical as-of replay): partly discharged.
  - The reliance journal is hash-chained and append-only, and gives as-of status.
  - `dag::replay` requires bit-identical output.
  - The forecast-scoring clause is outside this crate.
- **PO-PLUMB-15** (`Refused` never becomes a number without a declared fallback): partly discharged. A colour refusal survives composition (the random test, and `test_assertion_failed_calibration_and_staleness_never_show_a_strong_colour`). No declared-fallback mechanism exists.
- **GATE-PLUMB-E1** (no-laundering proptests 100%): the random test asserts zero violations, within PO-PLUMB-1's limits.
- **GATE-PLUMB-E4** (no ε without a RefClass; no unverified reference class on disclosure-facing claims): not met. `Claim` has no RefClass.
