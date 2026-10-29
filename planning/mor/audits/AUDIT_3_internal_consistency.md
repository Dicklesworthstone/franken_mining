# Internal consistency (ids, cross-references, schedule, crate phases, bead graph)

> Fresh-context, read-only audit of the plan draft, 2026-10-28. Model output, preserved verbatim as the
> provenance for plan Appendix H. Line numbers refer to the draft at audit time, not the current plan.
> Dispositions of every finding are in Appendix H.

I ran 15 numbered findings below, most severe first. Each was checked by script or by reading the text; nothing was edited.

## Findings

**1. Some month-1 and month-2 deliverables have no crate or bead in time, while two phase-A crates have no phase-A work.**
- DecisionCards are the delivery format from M0 (l.6141). GATE-HELM-01 and -02 are due at M1 (l.3949–3950, 5981, 6216). The M2 Site Assurance card is BD-211.
- But `fmine-decide` is phase B (M3–M4, l.5269), and WP-14 is set to M3–M4 (l.6339). The DecisionCard schema (BD-225), the retrospective cards (BD-226) and the value-of-information estimator (BD-227) all sit in WP-14's epics.
- The reverse problem: `fmine-metalacct` and `fmine-fa-bridge` are starred phase-A crates (l.5271, 5277), but their first beads are in WP-13 at M3 (BD-213–217). §37.4 also says "FA: None" for M0–M2.
- **Fix:** move `fmine-decide` (schema, retro cards, VOI) into phase A, and put BD-225–227 into WP-02/03. Either un-star metalacct and fa-bridge or add phase-A beads for them. Then recompute "36 crates / 640k".

**2. Pushback's start date contradicts itself.**
- §15.11 times every Pushback gate "relative to Pushback start, M4" (l.2208), and OPEN-PUSH-1's deadline is "Pushback start (M4)" (l.6095).
- §34.3 says Pushback's earliest gate is M6 (l.5974), WP-19 is M6 (l.6344), and the M6 acceptance uses GATE-PUSH-PIT1.
- If the start is M6 (month 30), KILL-PUSH-PDLP (+9 months) and GATE-PUSH-STO (+10) land after month 36, outside the plan. If the start is M4, the solver crates `fmine-optimize` and `fmine-lp` (phase C) don't exist yet, and `fmine-plan` is phase B but its work package is M6.
- **Fix:** pick one start month, fix it in all five places, and trim the +N schedule to fit inside month 36.

**3. The work-package dependency table in §38 (l.6325–6345) is contradicted by the bead graph, which forms cycles between work packages.**
- WP-02↔WP-04: BD-053 depends on BD-080, which depends on BD-040.
- WP-03↔WP-06: §38 says WP-03 depends on WP-06, but BD-125 depends on BD-071.
- WP-09↔WP-11: §38 says WP-09 depends on WP-11, but BD-188 depends on BD-163.
- WP-04↔WP-06 and WP-04↔WP-07: BD-098 depends on BD-121 and BD-131.
- WP-12 (M2) depends on WP-14 (M3–M4) through BD-211 → BD-225.
- WP-12 also depends on WP-03 (BD-199 → BD-075), which is on the critical path but undeclared.
- In total there are 23 undeclared cross-package edges.
- **Fix:** generate §38's "Depends on" column from the bead graph and break the cycles (for example, put the L0 reference ledger BD-080 in WP-02).

**4. KILL-1 is defined twice, differently.**
- §21.3 (l.3578): FA-ACC-09 fails at the FA rev pinned by **month 9**; fallback is monthly posting with ≤300 publications.
- §37.3 (l.6264): FA "lacks metal units or acceptable performance at **month 15**"; fallback is a local certified ledger with FA as a draft adapter.
- **Fix:** keep one trigger, date and fallback, and cross-reference it from the other place.

**5. Gates on the early path that no work package or bead closes.** §39 says a bead closes only with "a gate record".
- GATE-WALL-01 is an M1 acceptance criterion (l.6220) but appears in no work package or bead.
- Also unowned: GATE-PLUMB-E3 (l.943), GATE-REPORT-01 (M3), GATE-CRUC-02/03, INT-AS-001 (asupersync admission, needed from commit 1), INT-FNX-001, INT-FSEARCH-001, KILL-PLUMB-5/6.
- **Fix:** add closing beads, for example GATE-WALL-01 under WP-04 next to BD-093, and INT-AS-001 under WP-00.

**6. Gate milestones conflict between sections.**
- GATE-SYNC-02 is M2 in §26.12 (l.4928) but listed under WP-17 at M5 (l.6342).
- GATE-HALL-02 is defined as equal to GATE-STRATA-G5, which is due at "M12" of Strata work (Strata starts M3, so roughly month 24). Yet WP-13 assigns it to M3 (l.6338).
- §13.12 (l.1739–1744) writes Strata-relative months as "M5…M12", which collide with milestone IDs M0–M6 (§0.3).
- **Fix:** move the two gates to consistent work packages, and write Strata months as "S+5" or "month 5 of Strata".

**7. Card GB-31's branch values don't match its own equation, and fixing them reverses its conclusion (l.4227–4232).**
- With the card's inputs, c_FI = 0.75 gives NAV = 0.85·(0.75 + 2.25·0.75)·300·0.65 = **$404M**, not $497M. c_FI = 0.29 gives **$232M**, not $197M.
- As a check, the probability-weighted mean of 404 and 232 equals the base NAV of $292M; the card's 497/197 do not.
- With $404M, the earn-in is worth 0.35·(0.51·404 − 110)/1.08 − 35 ≈ **−$3.9M**, so "buy information (the earn-in)" flips.
- **Fix:** correct the branch values, or state what else changes in the good branch.

**8. Identifier defects.**
- `G-` means Guard, but G-CAP-1…3 are gates (l.4192–4194). GATE-CAP-04/05 imply that 01–03 exist, and §34.3 omits them. Rename to GATE-CAP-01…03.
- GATE-PUSH-BZ is referenced (l.2204 and §34.3 l.5974) but only KILL-PUSH-BZ exists.
- KILL-PROS-1 (BET-09, l.284) is undefined; it should be KILL-LODE-1.
- BET-10 (l.285) cites OPEN-STRATA-3 for retained realizations; that topic is OPEN-STRATA-4.
- **GB-06 does not exist.** The cards are GB-01…GB-38 with one gap. Line 736 cites GB-06 for the capex mixture fit, which is GB-32.
- RISK-B4-10 (l.4650) and PO-HIGH (l.2937) are undefined. Highwall has no PO or guard tables at all.

**9. Cross-references that point to the wrong subsection.** All 744 internal § references resolve, but these hit the wrong target:
- l.1572, 1993, 2014: §18.4 → **§18.5** (acid rock drainage / PAG).
- l.2434: §18.3 → **§18.4** (site water balance).
- l.2376: §17.6 → **§17.7** (liners and wear).
- l.1664: §14.3 → **§14.2** (limit-equilibrium solvers).
- l.2910: §10.3 → **§9.3** (`REFCLASS-` records).
- l.3523: §0.5 → **§28.1** (crate-prefix decision).
- l.1333: §13.2 → **§13.4** (support).

**10. The layer rules contradict where two crates are placed.**
- §28.2 says V is "the only IO and REMOTE layer".
- But `fmine-sync` is in S1 with a QUIC/ATP adapter (l.5242), and `fmine-telemetry` is in S1 doing ingest plus an fsqlite catalog (l.5241, L-4).
- **Fix:** widen S1's "May" column explicitly to cover sync transport and telemetry IO, or move the adapters to V.

**11. The OPEN-item rule isn't met.**
- §36.1 says every OPEN item has an owner, a deadline, a fail-closed default and deciding evidence. Only the 10 items in §36.2 do; the roughly 65 section-level OPEN-* items have none.
- §36.3's index omits Highwall (OPEN-HIGH-1…3, l.1857) and OPEN-B4-07/09.
- **Fix:** add owner and deadline columns to §36.3 and add the missing rows.

**12. The §28.3 ceiling totals don't add up.** The K-layer ceilings sum to **514**, not 513 (l.5296), so the grand total is **1,298**, not 1,297 (l.5301). The plan-LOC column and the phase subtotals are correct.

**13. Two brief items are thin.**
- *Equipment and techniques:* §16.3 and §16.4 design open-pit bench blasting and truck/shovel haulage well. But there is no design for underground development rounds (drill–charge–fire–muck–support), longhole/ring blast design, LHD loading, hoisting, or dozing/ripping/scraping. "Demolition" appears only as a closure-cost line item.
- *Highwall* (§14) has no proof obligations, guards or galaxy-brain card, unlike every other subsystem.
- **Fix:** add an underground subsection to Headframe (records, cycle model, scored productivity) and a PO/guard table to §14.

**14. Places an implementer can't act on without an outside document.**
- INT-AS-001 depends on "FSS-DD §2.9 criteria" and INT-FSQL-001 on "FSS-DD ten criteria" (l.5951, 5955); neither list is given.
- G-PUSH-1 and G-HELM-1 fire when "an E023 flag would be set" (l.2104, 3932). E023 is an evidence row in MOR_CASE.md, and l.1181 uses it to mean MNPI. Restate the condition as "any INV-002 flag".
- The FD- dependency rows (FD-FA-*, FD-FSIM-03) and every "upstream ask" live only in Appendix F (l.3553, 4993, 1841).
- FA-internal IDs (FA-INV-12, V07, V19, V27, fa-76hy) are used without a gloss.
- **Fix:** inline the criteria and give each external ID a one-line meaning.

**15. Minor contradictions and label collisions.**
- §0.6 says evidence rows run E001–E033, but E034 and E035 are cited (l.6038, 2548).
- §0.7 says FTX ran "2.7× over its **ceiling**"; §28.3 and RISK-25 say it overran its **plan** 2.7×.
- §0.2 requires a ten-section CONTRACT.md, but the template in §28.6 and BD-002 has five sections.
- Label collisions: the reconciliation levels R0–R3 (§20.3) clash with research reports R1–R8; failure modes F1–F12 (§26.11) clash with pre-mortem stories F1–F16; `FMINE-REF-NoClaimInDomain` (l.411) duplicates code 201.

## Checks that came back clean
- **Cross-references:** all 744 internal § references resolve to real headings, including the bold §21.3.1–21.3.7 subheads. External references (R#, B#, FSS) were excluded.
- **§39 bead graph:** the stated facts reproduce exactly — 256 beads numbered BD-001…256 with no gaps, 415 edges, 0 cycles, 0 unknown dependencies, one root (BD-001), and a 28-bead longest chain identical to the one listed.
- **§28.3 crate map** (apart from finding 12): 59 crates plus 1 gated; the per-layer crate counts and plan-LOC column total 1,128; starred crates match phase A one-to-one; phases have 36/15/8 crates and 640/306/182k; every ceiling is 1.15× plan, rounded. The §0.5 crate lists agree with §28.3, and the K-order lists in §28.5 cover all 20 K crates.
- **Definitions:** INV-001…015, RISK-01…25, all FMT-, OB- and INT- IDs, and the FMINE-REF families are defined. No ID is defined twice with different meanings.
- **Arithmetic:** cards GB-01, 02, 04, 05, 07, 09, 24, 25, 28, 30, 32, 33 and 38 check out.
- **Exit codes** agree across §8.3, §21.3, §26 and §30.1.
- **Milestone calendar** (M0–M6 months and dates) and the data-acquisition deadlines agree across §34.4, §36.2 and §37.3.
- **Brief coverage:** exploration and sampling (§11–12), frankensim primitives and the materials database (§13.9–13.10), on-site refining (§17.6), tailings (§18), labour, budgets and productivity (§19, §21.6), FA accounting (§21), ore mix and capital (§15.6, §23), the evidence ledger with known/unknown terms (§6–10), and the asupersync pattern (§26–27) are all actually designed, not just mentioned.
