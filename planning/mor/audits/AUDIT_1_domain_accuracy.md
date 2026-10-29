# Domain accuracy (mining, geostatistics, metallurgy, accounting, codes; arithmetic recomputed)

> Fresh-context, read-only audit of the plan draft, 2026-10-28. Model output, preserved verbatim as the
> provenance for plan Appendix H. Line numbers refer to the draft at audit time, not the current plan.
> Dispositions of every finding are in Appendix H.

I found 15 problems. The two most damaging are the LUC claim, which runs through the whole thesis, and a Newmont AISC comparison that mixes two different reporting periods. Line numbers refer to `/Users/jemanuel/projects/franken_mining/COMPREHENSIVE_PLAN_FOR_THE_DESIGN_OF_FRANKENMINING.md`. All arithmetic was recomputed with `python3 -I`.

## Findings (most severe first)

**1. LUC is misnamed and its one case study is presented as a general fact. Lines 203, 1437–1443, 1892–1895. Severity: high.**
- Quoted text: "the local-uncertainty-correction (LUC) slope against grade control can be ≈0.1"; "With the measured LUC-case slope b ≈ 0.1 … 1.1 g/t (−45%)"; "The third term dominates in mining".
- What is wrong:
  - LUC stands for Localised Uniform Conditioning (Abzalov 2006). R7 line 639 has it right.
  - The 0.1 slope comes from a single study (Assibey-Bonsu & Muller 2014). It shows that LUC's step of localizing estimates to SMUs is weak at local scale. It is not "kriging's conditional bias", and it is not a typical resource-model slope.
  - "Goes unreported" is also wrong. QPs routinely publish kriging efficiency and slope of regression, and the plan's own `ClassificationSupportPack` (line 1707) lists them.
  - GB-11's −45% therefore rests on a weak foundation. A Head of Mineral Resource Management would reject it on sight.
- Fix: spell out "Localised Uniform Conditioning". Cite the result as one case study of LUC. Use a range of measured slopes for ordinary kriging (OK) in GB-11 and keep 0.1 as a stress case. Drop the claim that bias "goes unreported".

**2. The Newmont by-product vs co-product AISC example mixes periods. Lines 3668–3669. Severity: high.**
- Quoted text: "Newmont's by-product AISC is about $1,520/oz and its co-product AISC $1,963/oz in Q3 2026".
- What is wrong: per R6 §2.3, $1,520 is the October 2026 full-year guidance. The Q3 2026 by-product actual is **$1,454/oz**. The plan sets a guidance figure against a quarterly actual, in the very passage arguing for basis discipline. A CFO would catch this immediately.
- Fix: say "Q3 2026: by-product $1,454/oz vs co-product $1,963/oz".

**3. The JORC 2026 reconciliation requirement is labelled FACT. Lines 3316 and 4276; compare 225 and 4285. Severity: medium.**
- Quoted text: "Provisional JORC 2026 adds mandatory reconciliation disclosure (FACT, E031)".
- What is wrong: R7 §4.2 says JORC 2012 is still in force, the reconciliation section is 2024 exposure-draft content, and the final wording is unverified. The plan's own §24.2 says the same, so the document contradicts itself.
- Fix: "The 2024 exposure draft proposes a mandatory reconciliation section (final wording and commencement unverified)."

**4. The IFRS 9 treatment of provisional pricing is wrong. Line 2651. Severity: medium.**
- Quoted text: "Embedded derivative MTM (IFRS 9)".
- What is wrong: under IFRS 9 (4.3.2), a provisionally priced receivable is measured at fair value through profit or loss (FVTPL) as a whole. There is no separated embedded derivative; that is how IAS 39 and US GAAP (ASC 815) treat it. Barrick reports under IFRS. R4 line 476 states this correctly.
- Fix: "Under IFRS 9 the receivable is FVTPL until final pricing; under US GAAP the embedded derivative is bifurcated."

**5. CIL is described as "the standard response to preg-robbing ores". Line 2594. Severity: medium.**
- What is wrong: this holds only for mildly preg-robbing ore. Barrick's own Carlin-trend carbonaceous, double-refractory ores are treated by roasting (Goldstrike, Gold Quarry), by pressure oxidation (POX) plus thiosulfate leach (Goldstrike's thiosulfate circuit), or with blanking agents. A Barrick metallurgist would notice.
- Fix: "CIL mitigates mild preg-robbing; strong preg-robbing needs roasting, POX/thiosulfate or blanking agents."

**6. The Kuz-Ram exponent is wrong. Line 2294. Severity: medium.**
- Quoted text: "(115/RWS)^(19/20)".
- What is wrong: the Cunningham form uses **19/30**. It follows directly from substituting TNT-equivalent charge: 0.8 − 1/6 = 19/30. R7 tagged the formula as unverified, and PO-CRUC-4 would fail on textbook examples.
- Fix: change the exponent to 19/30.

**7. GB-22's flip condition does not follow from its numbers. Line 2983; inputs at 2973–2977. Severity: medium.**
- Quoted text: "ANC sd falls to 2 kg/t after testing → Block B's P(PAG) falls below P* → NAF".
- Recomputation: with S sd = 0.25% (the value that produces the card's 0.13), sulphur alone contributes 30.6 × 0.25 = 7.65 kg/t of sd. Reaching P* = 0.048 needs a total sd of 6.49 or less. With ANC sd = 2, P(PAG) = **0.086**, still above P*. No amount of ANC testing can flip Block B.
- Fix: make the flip about sulphur precision, e.g. "S sd ≤ ~0.15% plus ANC sd ≤ 4", and adjust the intuition line about ANC VOI.

**8. GB-31's branch NAVs do not follow from its stated inputs. Lines 2229–2232. Severity: low–medium.**
- What is wrong: with P_dev = 0.65, the formula gives $404M and $232M, not $497M and $197M. On those numbers the bad branch would not walk away (0.51 × 232 − 110 = +8.6), and the earn-in is worth about +$1.3M, not +$11.5M. The source (B3 line 802) also revises P_dev to 0.80 and 0.55 in the two branches, and the plan dropped that step.
- Fix: restore the per-branch P_dev values.

**9. GB-23's $55M/y saving is mostly an artefact of the salary assumption. Lines 3170–3173. Severity: low–medium.**
- What is wrong: the 14/7 roster works 17% more hours (2,920 vs 2,503) for the same $150k salary. That assumption accounts for $85k of the $139k per-seat difference. At equal hourly pay the saving is about **$53k/seat, or ~$21M/y**.
- Fix: show the equal-hourly-rate case, or relabel the headline.

**10. The M2 pilot-site preference contradicts its own alternate. Lines 6227–6228. Severity: low.**
- What is wrong: the stated preference is "100% owned, outside the JV perimeters", but the alternate, North Mara/Bulyanhulu, is 84% Barrick with 16% held by the Government of Tanzania through Twiga (R6 line 168).
- Fix: drop the alternate or state the Twiga data-governance implications.

**11. NRV "at the long-term price" is applied to all inventory. Lines 3330, 3621, 3641. Severity: low.**
- What is wrong: IAS 2 uses the expected selling price. Long-term prices are defensible for long-term stockpiles, but not for doré, concentrate or metal in circuit.
- Fix: make the price basis a per-inventory-class policy object. Verify the wording against Barrick's accounting policy note.

**12. GB-16 counts deferred metal as lost value. Line 2194. Severity: low.**
- Quoted text: "≈ $36M at $4,500/oz".
- What is wrong: unmined planned blocks are deferred (sequence debt), not lost. The value at stake is the NPV of the deferral plus any sterilization.
- Fix: label it "production variance vs plan" and value it separately.

**13. The cause of the Q1 2026 industry AISC rise is overstated. Line 2036. Severity: low.**
- Quoted text: "Royalties drove most of the +16% industry AISC rise".
- What is wrong: R6 §4.2 says it was "driven by royalties and corporate G&A".
- Fix: match the source wording.

**14. A sweeping claim about reconciliation failures is unsupported. Line 4455. Severity: low.**
- Quoted text: "Most material reconciliation failures in mining are not modelling failures. They are basis failures".
- What is wrong: there is no evidence for it, and it will antagonize resource geologists.
- Fix: label it HYPOTHESIS or say "many".

**15. Two technical identities are stated too strongly. Severity: low.**
- (a) Lines 2051 and 2134: the claim that the LP mill-hour dual equals (f+F)/H holds only if mine life, and hence the time-based f and F, is endogenous. In a fixed-horizon period LP, f is sunk.
- (b) Lines 1138–1143: the (1−1/e) and (1−1/e)/2 greedy guarantees apply to the monotone coverage term under a budget, not to F(S) − cost(S), which is non-monotone.

## Areas checked and found sound

- **Galaxy-brain arithmetic reproduces:** GB-01, 02, 04, 05, 07, 09, 10, 11 (arithmetic only), 12, 13 (including the 27% flips), 14, 15 (binomial 0.073 and 0.008), 16 (arithmetic), 17 (queue U = 0.832/0.917), 18, 19 (74.5% break-even), 20, 21, 24, 25 (posteriors −7.1 ± 2.2 and 1.8 ± 1.8), 26 (LMDI effects sum to +$144.98; sequential +$66.67 and +$69.80), 27, 29 (73% and ~0.79 flips), 30 (including the regret table), 32, 33, 34 and 38. Singer σ ≈ 1.8–2.4 also checks.
- **Formulas:** Lane's three cut-offs, Bond, Swebrec, Faraday, Atkinson, square-root scaled-distance PPV, Gy's fundamental sampling error, MPA = 30.6·S, NPR thresholds, the Koopman search update, Krige's relation, nested-dissection fill n^(4/3), and the troy-ounce, short-ton, 240/7 g/t and survey-foot constants.
- **Planning algorithms:** pseudoflow, BZ, MineLib, TopoSort, Mak–Morton–Wood bounds, and the F1/F2/F3 and mine call factor definitions.
- **Tailings and safety:** GISTM facts (6 topics, 15 principles, 77 requirements, consequence classes, ITRB for Very High/Extreme, 2023/2025 deadlines), TML under the IMSBC Code, ICMM critical control management, TARP logic, and Mathews/Potvin.
- **Accounting:** WGC 2013/2018 dates, IAS 2.33 vs ASC 330, IFRIC 20, ASC 360's two-step test, and IAS 37 vs ASC 410-20.
- **Disclosure codes:** S-K 1300 and NI 43-101 status.
- **Barrick and industry figures** match R6/R7: CCV 5%/75%/85%/84%, GEO 25%, $1,637 vs $1,423, 190% vs 504%, 64 TSFs with 18 Extreme/Very High, MinEx base rates, Bertisen & Davis, and Boddington's 41 trucks. These are post-2025 single-source items. Re-verify them against primary filings before presenting to Barrick executives.
