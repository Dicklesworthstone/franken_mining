# CONTRACT.md — `fmine-decide`

## 1. Charter & Layer
- **Crate:** `fmine-decide`
- **Layer:** `E` (Enterprise & Decision)
- **Subsystem:** `Enterprise`
- **LOC Budget:** `16k planned / 18k ceiling` (`registries/loc_budget.toml`)
- **Allowed Module Families:** `["card", "voi_rank", "one_pager"]`

## 2. Mathematical & Domain Formulation
Implements `DecisionCard` (`FMT-CARD-1`), `FlipMargin`, `rank_voi_campaigns` (`GATE-HELM-02`), and the
CEO/CFO/Mine-GM 1-Page Executive Decision Card Markdown renderer (`render_one_pager_markdown`, `§9.6`).

`DecisionCard::build_as_of` consumes an ordinary card and returns an immutable `RetrospectiveDecisionCard`
(`BD-344`, §22.6). It checks the expected-NPV Claim's canonical identity, graph node kind and immediate parents,
then checks every declared ancestor against a decision timestamp and per-cell record anchor. A successful
`fmine_claim::dag::replay::ReplayCertificate` must bind the same root, cut, checked cone and card output. The replay
generator returns `DecisionCard::temporal_replay_bytes()`: rendered Markdown, the NPV Claim body and exact bits of
displayed numeric values and VOI calculation totals. Thus changing a value beneath the displayed decimal
precision cannot reuse a proof. Unrendered Claim metadata is subject to the existing Claim canonical-body limits.
The projection is an internal versioned replay input, not a new durable `FMT-CARD-1` encoding.

## 3. Authority, Epistemics & Representation
Enforces `INV-002` (`authoritative: false`, `may_actuate: false`, `is_safe_certification: false`, `is_signing_basis: false`) and `INV-015` (non-empty `uu_register`, `FMINE-REF-204`) on every `DecisionCard`.

## 4. Public API Surface
- `FlipMargin`, `DecisionCard` (`build`, `build_as_of`, `temporal_replay_bytes`)
- `RetrospectiveDecisionCard`: immutable `card`, `cone` and `replay` accessors; scoped Markdown renderer
- `VoiCampaign`, `VoiRankedItem`, `rank_voi_campaigns`
- `render_one_pager_markdown`

## 5. Guards & Typed Refusals
`DecisionCard::build` relays the claim's refusals:
- `FMINE-REF-204` (unbudgeted unknowns, `G-UU`), exit 3: a non-`KnownKnown` claim with no `UuTerm`.
- `FMINE-REF-205` (calibration required, `NL-4`), exit 3: an `expected_npv_usd_m` with no colour to show, because
  calibration failed or it is an unreviewed `Assertion`.

`build_as_of` repeats the UU and colour guards, rejects a changed stored colour, and additionally refuses:
- `FMINE-REF-507`, exit 6: the NPV Claim's current hashed body does not match its identity.
- `FMINE-REF-110`, exit 3: missing or mismatched Claim root/parents, incomplete or post-cut temporal ancestry,
  or a replay certificate for a different root, timestamp, anchor, cone or output. Missing node times are never
  inferred. Record admissibility uses the per-cell anchor, not wall-clock ordering between cells.

## 6. Proof Obligations (`PO-*`)
- `PO-DEC-1`: `DecisionCard::build` refuses a card whose claim has an empty `uu_terms` (`FMINE-REF-204`) or no colour
  (`FMINE-REF-205`).
- `PO-DEC-2`: `rank_voi_campaigns` guarantees `0 <= EVSI <= EVPI` (`GATE-HELM-02`).
- `PO-DEC-3`: retrospective construction binds the actual NPV Claim and the replayed output; neither an unrelated
  clean root nor an older certificate for edited output is accepted. The checked snapshot has no mutable accessor.

## 7. Failure Modes & Deterministic Fallbacks
Invalid or unanchored decision inputs fail closed with typed `Refusal`.

## 8. Gates & Kill Criteria (`GATE-*` / `KILL-*`)
- `GATE-HELM-02`
- `GATE-HELM-03`

## 9. Lifted Code & Foundation Dependencies
- Depends on `fmine-core`, `fmine-units`, `fmine-time`, `fmine-evidence`, `fmine-claim`, `fmine-numerics`, `fmine-calib`.

## 10. Non-Goals & Scope Boundary
Never actuates physical equipment (`INV-002`) or performs file/network I/O (`L-3`).

The retrospective proof covers the **declared expected-NPV Claim ancestry**. The existing action text, CVaR,
flip margins and VOI fields have no individual Claim provenance. Their bytes are replayed, but this does not prove
complete historical sourcing for those fields or certify the whole decision. The renderer states this boundary.
The replay contract requires all evidence reads to use the supplied store view; captured values or external I/O
cannot be detected by a pure in-process checker. This slice supplies the guard and synthetic controls, not the
two real retrospective cards required by GATE-HELM-01. No cross-host replay result is asserted;
`DeterministicPerIsa` remains the crate's class. Ordinary `build` and its output are unchanged.
