# B7 disagreement register (Q-001), written before synthesis

Copied from the session workspace (`distillations/disagreement_register.md`); pack files stay outside the
repository.

Sources: `by_cod.md` §4 (Codex), `by_agy.md` §4 (Gemini), the Claude pilot pack
`EV_quickie_pane0_identity_explainability.md` (EVQ-P0-1) and `by_cc.md` §4 (Claude). Each entry names the
positions, the decisive premise, and how it was settled. "Lead check" means the operator re-read the bytes or
pages. Ids E815–E819 are in `planning/mor/MOR_CASE.md`.
A row is RESOLVED only when a lead check or a construction-free test decides it; otherwise it stays OPEN.

| DR | Topic | Positions (pack ids) | Decisive premise | Status and settlement | Plan effect |
|---|---|---|---|---|---|
| DR-1 | Africa & Middle East Q1 2025 AISC/oz (1,602 against 312/202 = 1,544.6) | Gemini: closed by Q1 2025 MD&A p.18 (by_agy §4.1). Codex: (312 + 10.5) × 1,000 / 201.8 = 1,598.1, not 1,602, so not a unique closure (by_cod §4.1). Claude: closes on the member-weighted test with Loulo-Gounkoto sustaining capital 10 from the Mine Stats workbook (EVQ-P0-1 §5) | Which public cells supply the excess, and whether any page states the inclusion rule | RESOLVED. Lead check verified p.18 (COS 5, depreciation 5) and `Minestats3!B32:B34` (10, 3, 14). The arithmetic closes; no page states the rule. Gemini is right that it closes, Codex that p.18 alone does not close it (E816) | §21.6 state (ii) now splits *stated* from *reconstructed* |
| DR-2 | Are 68 of 75 per-ounce misses "explained by rounding"? | Gemini: yes, under the component-sum construction (EV2-P3-2). Codex: only conditionally feasible, since the construction was undeclared (by_cod §4.1) | Which construction the tables follow | RESOLVED. Lead census: 1,207 of 1,212 printed TCC and AISC totals foot to their displayed lines (E815), so the tables declare the component-sum construction. Codex's objection held until that census; the count is now 35 of 46 quantities explained, 11 not (EVQ-P0-1, E817) | Construction declared per table family; COS/oz included |
| DR-3 | North Mara Q2 2026 per-ounce rows (84% share against M19 = 2,736) | Codex: 0.84 × (117 ± 0.5) × 1,000 / (37 ± 0.5) ≤ 2,704.11 refutes the witness (EV2-P1-4). Gemini: the witness rounds to every cell, and a flat 84% share misapplies to IFRS cost of sales (EV-P3-6; by_agy §4.3). Claude: COS-implied ounces [35.76, 36.08] are disjoint from 37 ± 0.5, and its own pilot item #23 is withdrawn (by_cc D2) | Whether the 16% non-controlling share applies to cost of sales | RESOLVED to state (iv) on two families' agreement and a lead cell check: TCC and AISC rates imply 36.0 and 36.1 koz against 37 (E819). The share remains an assumption | Added to the plan's open items (K.4); B7's count becomes 34 of 46 explained |
| DR-4 | Loulo-Gounkoto Q1 2025 production cells (22 koz on a 100% basis, 18 koz attributable) | Gemini: NOT_PUBLISHED exclusion is wrong. Plan: "NOT_PUBLISHED wins for scoring" (E122, E477, E478) | What the issuer later filed | RESOLVED. Lead check: Q1 2026 MD&A p.42 says "no operating data or per ounce data was provided for Q1 2025 to Q3 2025"; Q2 and Q3 2025 MD&As date loss of control June 16, 2025 (E818; W1-DR-03 closes) | NOT_PUBLISHED stands; workbook cells stay a register item |
| DR-5 | H-P0-6 copper deck revaluation of GEO reserves | Gemini: all four tests run, a revaluation term explains the gap (EV2-P3-6). Codex: 2025 overlaps without revaluation, [170.5, 170.75] Moz; the 2024 first-filed 6.7 still fails after the −2.04 term (EV2-P2-2) | Whether a single price-basis term explains both years | OPEN. The plan adopts neither: it keeps both vintages of the 2024 row (E692) and the price-basis mismatch (E147) as separate register items | §24.3 issuer reserve rows; GATE-HALL-01 fixture |
| DR-6 | H-P2-2 (bounded recipe) and H-P2-4 (draft interface) attacks | Gemini: fixed rows break on layout drift; registering a forecast is not obtaining a draft (EV-P3-2, EV-P3-4). Codex: both attacks change the target, since H-P2-2 keys rows by source and vintage and H-P2-4 routes drafts through an operator step (by_cod §4.3) | Whether the attacked mechanism is the one proposed | OPEN as an interpretation dispute. The row shift itself is FACT (E693), so the plan adopted label-anchored import (BD-322) either way | BD-322; agent-surface tests in §30 |
| DR-7 | Bennet against LMDI line effects | Claude: identical amounts on every pair (EV-P0-5). Codex: Phoenix Q3→Q4 2025 gives Bennet −735.12 against LMDI −721.69 $/oz (by_cod §4.4) | Fresh calculation on `F17`, `F18` | RESOLVED for Codex on its own execution (lead did not re-run). The falsifier that fired on H-P3-5 is unchanged; only the reason changes | The waterfall names its convention (§21.6) |
| DR-8 | Exhaustiveness of the "< 15 material findings" gate | Claude: five candidates classified (EV-P0-3). Codex: not exhaustive, partner join unrun; Kibali TR L1428 and Pueblo Viejo L818/L821 already flag the examples (by_cod §4.2) | Whether a census was run | RESOLVED for Codex: the count is not adopted | None |
| DR-10 | Hemlo Q4 2025 | Gemini: closed within [1,727, 2,058] (EV2-P3-2). Claude: COS 44, ounces 27, COS/oz 1,738; at most 44.5 / 26.5 = 1,679 is reachable (by_cc D1) | A one-cell numerator test | RESOLVED for Claude: lead check of `H5`, `H18`, `H19` (E817) | State (iv) |
| DR-11 | Zero lines in the two-part waterfall split | Gemini: it crashes on 23.9% of columns where a credit is zero (EV2-P3-4). Claude: the LMDI zero limit is finite, L(a,0)·ln(a/0) → a; the real defects are two sign changes and the sign of a displayed 0 (by_cc D4) | The limit of the log-mean weight | RESOLVED for Claude on the mathematics; the downgrade stands for its own reasons | None beyond §21.6's signed fallback |
| DR-9 | Locator errors in Gemini's packs | Codex and Claude independently: `AISC25Q4:W8 = 64` is not in the sheet, which has no column W (by_cod §4, by_cc D5); Codex: EV2-P3-3 labels column F Cortez where F4 is Phoenix | The cells themselves | RESOLVED for Codex on its reading. No Gemini count enters the plan unless the lead re-ran it (E697 was re-run; others were not) | Adoption rule: a pack's count enters only after a locator-verified rerun |

## Residual doubts (no counter-pack yet)

- Whether Veladero's per-ounce rows reuse a prior quarter's ounces (E691) or a separate ounce series (EVQ-P0-1
  §6.1). The cheapest decisive test is the issuer's own per-ounce denominator for Q4 2025, which only the issuer can
  give; until then it is state (iv).
- Kibali Q2 2024 by-product credit printed +1 where −1 foots, in two vintages (E815): a state-(iii) candidate with
  cited cells that no pack has yet attacked.
- North Mara Q2 2026 and Veladero share one signature (all three rates point to an ounce figure below the printed
  one). Cheapest test (by_cc §5.1): the share-scaled one-cell COS check on every site column of the eight AISC
  workbooks; if only these fail, the cause is site-specific.

## Hygiene

- EV2-P3-5 (Gemini) copies Qualified Person names from a filed MD&A into a workspace pack (flagged by Claude, by_cc
  §4). The pack stays outside the repository and nothing from it was carried into the plan. The plan and C1 have
  named the issuer's public QPs since `30ee8f5` (E671); that predates B7 and is left for the owner to decide.
