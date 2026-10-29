# CONTRACT.md — `fmine-decide`

## 1. Charter & Layer
- **Crate:** `fmine-decide`
- **Layer:** `E` (Enterprise & Decision)
- **Subsystem:** `Enterprise`
- **LOC Budget:** `16k planned / 18k ceiling` (`registries/loc_budget.toml`)
- **Allowed Module Families:** `["card", "voi_rank", "one_pager"]`

## 2. Mathematical & Domain Formulation
Implements `DecisionCard` (`FMT-CARD-1`), `DecisionAction`, `FlipMargin`, `rank_voi_campaigns` (`GATE-HELM-02`), and the CEO/CFO/Mine-GM 1-Page Executive Decision Card Markdown renderer (`render_one_pager_markdown`, `§9.6`).

## 3. Authority, Epistemics & Representation
Enforces `INV-002` (`authoritative: false`, `may_actuate: false`, `is_safe_certification: false`, `is_signing_basis: false`) and `INV-015` (non-empty `uu_register`, `FMINE-REF-404`) on every `DecisionCard`.

## 4. Public API Surface
- `DecisionAction`, `FlipMargin`, `DecisionCard`
- `CampaignCandidate`, `RankedCampaign`, `rank_voi_campaigns`
- `render_one_pager_markdown`

## 5. Guards & Typed Refusals
- `FMINE-REF-401` (`UnanchoredClaimError`)
- `FMINE-REF-404` (`MissingUuRegisterError`, `G-UU`)

## 6. Proof Obligations (`PO-*`)
- `PO-DEC-1`: `DecisionCard::new` refuses any card with empty `uu_register` (`FMINE-REF-404`) or empty `action_set` (`FMINE-REF-401`).
- `PO-DEC-2`: `rank_voi_campaigns` guarantees `0 <= EVSI <= EVPI` (`GATE-HELM-02`).

## 7. Failure Modes & Deterministic Fallbacks
Invalid or unanchored decision inputs fail closed with typed `Refusal`.

## 8. Gates & Kill Criteria (`GATE-*` / `KILL-*`)
- `GATE-HELM-02`
- `GATE-HELM-03`

## 9. Lifted Code & Foundation Dependencies
- Depends on `fmine-core`, `fmine-units`, `fmine-time`, `fmine-evidence`, `fmine-claim`, `fmine-numerics`, `fmine-calib`.

## 10. Non-Goals & Scope Boundary
Never actuates physical equipment (`INV-002`) or performs file/network I/O (`L-3`).
