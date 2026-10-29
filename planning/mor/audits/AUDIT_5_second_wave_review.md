# Second-wave architectural, mathematical, and primary-source consistency audit (AUDIT_5)

> Fresh-pass, script-verified second-wave review of `COMPREHENSIVE_PLAN_FOR_THE_DESIGN_OF_FRANKENMINING.md`
> (8,262 lines; 320 beads `BD-001..BD-320`) and `planning/brenner/B6_operations_plant_tailings_workforce.md`,
> 2026-10-29. Closes open audit item **H-1 (round 2)** in Appendix H.6.

---

## 1. Scope and Verification Methodology

This second-wave audit (`AUDIT_5`) was executed after the first three audits (`AUDIT_1` domain accuracy, `AUDIT_2` FrankenSuite fit, `AUDIT_3` internal consistency), the corpus extraction pass (`C1–C6`, Appendix I), the 12 deep-upgrade additions (`BD-309..BD-320`), and the round-2 Brenner pass `B6` (`planning/brenner/B6_operations_plant_tailings_workforce.md`).

We ran automated Python verification scripts across the 8,262-line plan and cross-checked every newly added equation and table citation directly against the primary-source NI 43-101 markdown extractions in `corpus_of_input_docs/markdown_extractions/`.

---

## 2. Numbered Findings (Most Severe First)

### Finding 1 · Primary-source table numbers and Four Corners $(\text{Au}, \text{As})$ recovery coefficients in Carlin (`§15.6` L2226–2231, `§17.3` L2836)
- **What we found by checking `Carlin_Complex_TechnicalReport_14032025.md` (pp. 142–147) directly:**
  1. **Table numbering (`§15.6` L2227–2230):**
     - `Table 13-7` (p. 142) is *Goldstrike Roaster Feed Parameters* ($\text{SS} \in [0.8\%, 1.8\%]$ opt $1.6\%$; $\text{CO}_3 > 14\%$; $\text{TCM} \in [0.8\%, 1.2\%]$ opt $0.95\%$; $\text{As} < 1{,}800\text{ ppm}$ opt $< 1{,}200\text{ ppm}$).
     - `Table 13-8` (p. 142) is *Gold Quarry Roaster Feed Parameters* ($\text{SS} \in [1.4\%, 2.5\%]$ opt $2.1\%$; $\text{CO}_3 \in [10\%, 18\%]$ opt $12\%$; $\text{TCM} \in [0.4\%, 0.75\%]$ opt $0.6\%$; $\text{As} < 1{,}800\text{ ppm}$ opt $< 1{,}200\text{ ppm}$; $\text{Hg} < 25\text{ ppm}$). Line 2227 cites `Table 13-10` for Gold Quarry Roaster instead of `Table 13-8`.
     - `Table 13-9` (p. 142) is *Goldstrike Autoclave Alkaline POX Feed Parameters* ($\text{CO}_3 > 8\%$, preg-rob $< 40\%$–$60\%$) and `Table 13-10` (p. 142) is *Goldstrike Autoclave Acidic POX Feed Parameters* ($\text{SS} < 2.8\%$ opt $2.8\%$, $\text{CO}_3 < 8\%$).
     - `Table 13-13` and `Table 13-14` (p. 145) contain the piecewise cubic recovery polynomials in $\text{Au}_{\text{opt}} = 0.029167\,\text{Au}_{\text{g/t}}$ for Alkaline POX and Acid POX ($\text{Au} \le 9.6\text{ g/t}$, $9.6 \le \text{Au} \le 45\text{ g/t}$, $\text{Au} > 45\text{ g/t}$).
  2. **Four Corners bivariate $(\text{Au}, \text{As})$ Gold Quarry Roaster recovery (`Table 13-16`, p. 147; `E156`):**
     - The verbatim equation in `Table 13-16` of `Carlin_Complex_TechnicalReport_14032025.pdf` is:
       $$R_{\text{FourCorners}}(\text{Au}, \text{As}) = \frac{\text{Au}_{\text{opt}} - \left(1.821\times 10^{-5}\,\text{As}_{\text{ppm}} + 0.127\,\text{Au}_{\text{opt}} - 4.890\times 10^{-4}\right)}{\text{Au}_{\text{opt}}}$$
       where $\text{Au}$ is in $\text{g/t}$, $\text{Au}_{\text{opt}} = 0.029167\,\text{Au}$ is in $\text{oz/st}$, and $\text{As}_{\text{ppm}}$ is in $\text{ppm}$.
     - Multiplying by $\text{Au}_{\text{opt}}$ yields the exact affine recovered-metal law in $\text{oz/st}$:
       $$\text{Au}_{\text{rec,opt}} = 0.873\,\text{Au}_{\text{opt}} - 1.821\times 10^{-5}\,\text{As}_{\text{ppm}} + 4.890\times 10^{-4}$$
       or equivalently in $\text{g/t}$ (dividing by $0.029167$):
       $$\text{Au}_{\text{rec,g/t}} = 0.873\,\text{Au}_{\text{g/t}} - 6.243\times 10^{-4}\,\text{As}_{\text{ppm}} + 0.01677$$
     - In `§15.6` (L2228) and `§17.3` (L2836), the coefficients were written with `0.007335 As` and `0.0230`. Those should be replaced with the exact `Table 13-16` coefficients above.
  3. **Carlin 2025 `Table 13-12` vs. Cortez 2022 Goldstrike Roaster curve (`§15.6` L2231):**
     - In `Carlin_Complex_TechnicalReport_14032025.pdf` `Table 13-12` (p. 143; `E154`), the updated Goldstrike Roaster recovery curve is exponential: $R(\%) = 92.03 - 37.36\,e^{-12.94(0.029167\,\text{Au}_{\text{g/t}})}$ (`West Leeville / All Others`) and $87.53 - 37.36\,e^{-12.94(0.029167\,\text{Au}_{\text{g/t}})}$ (`Turf / Four Corners`).
     - The rational cubic $R(g) = 1.1536 - 0.0046/g - 2.0942g + 3.4316g^2 - 1.9228g^3$ (with $g$ in $\text{oz/st}$ over its valid low-grade domain) is the older Goldstrike Roaster formula cited in the 2022 Cortez report (`E181`). Both are concave in head grade and benefit from the 2nd-order Jensen variance correction, but L2231 should cite both curves with their exact report provenance (`Carlin 2025 Table 13-12, E154` vs. `Cortez 2022, E181`).

### Finding 2 · Unit-scale trap ($S$ as mass fraction vs. weight percent) and stream cost ordering in Pueblo Viejo (`§15.6` L2232–2234, `§17.3` L2837)
- **What we found by checking `Barrick_PuebloViejo_NI43-101_TechnicalReport_February2026.md` (`§13.3` pp. 134–137 & `§15.2.6–15.2.14` pp. 215–218):**
  1. **Unit-scale verification (`Table 15-4` & `Table 15-8`):**
     - In Pueblo Viejo `Table 15-4` (p. 215; `E271`), the quartic variable processing cost $c_{\text{proc}}(S) = -1911.2 S^4 + 857.27 S^3 - 128.24 S^2 + 11.236 S + 6.8302\text{ \$/t}$ and power cost $c_{\text{pwr}}(S, P) = (794.9833 S + 22.3323)P$ define **$S$ as a dimensionless mass fraction** ($S \in [0, 0.12]$, where $9.04\%\text{ S}_{\text{tot}} \implies S = 0.0904$) and $P = \$0.07912/\text{kWh}$.
     - Evaluating at $S = 0.0904$ and $P = 0.07912$ yields $c_{\text{proc}} = \$7.3036/\text{t}$ and $c_{\text{pwr}} = \$7.4530/\text{t}$, summing to **$\$14.7566/\text{t}$** — which reproduces `Table 15-8`'s reported `Variable Ore Processing Cost = $14.76/t` (p. 218) to 4 significant digits!
     - Conversely, if an engineer passes $S = 9.04$ (weight percent, as used two lines earlier for Carlin's $\text{SS} \le 2.8\%$), the quartic term $-1911.2 S^4$ evaluates to $-\$10.1\text{M}/\text{t}$.
     - **Fix:** Explicitly annotate $S \in [0, 0.12]$ as `MassFraction` (alongside $P = \$0.07912/\text{kWh}$ and the `Table 15-8` $\$14.76/\text{t}$ check value) in `§15.6` L2234 and `§17.3` L2837, and cite this as a concrete regression fixture for `G-UNIT-4` (`BD-287`).
  2. **Stream cost ordering and table citations (`§15.2.12`, p. 217; `E272`):**
     - `§15.2.12` states that Whittle assumed *"the more expensive stream of flotation and POX for costs and recoveries rather than using the most profitable of the two possible processing streams"* (with stage-wise POX-CIL recovery $R_{\text{POX}} = \min(0.9 g_{\text{Au}} + 87, 90)\%$ in `§13.3.1` / `Table 13-4` and flotation recovery $R_{\text{FLT}} = a[\text{S}^{2-}] + b[\text{S}^{2-}]/g_{\text{Au}} + c/g_{\text{Au}} + d$ in `§13.3.2` / `Tables 13-5 & 13-6`).
     - In `§15.6` L2232, the parenthetical described `Flotation + POX` as having lower variable cost and cited `Tables 15-2 & 15-3` for recoveries. **Fix:** update L2232 to state that `Flotation + POX` is the more expensive bounding stream conservatively assumed in Whittle (`§15.2.12`), while `Tables 13-4..13-6` (`§13.3`) give the stage-wise `Direct POX` vs. `Flotation + POX` recovery laws and `Tables 15-2..15-8` give the cost cards.

### Finding 3 · Guard ID collision (`G-STRATA-6`) and unindexed `Corebox` guards (`G-CORE-1`, `G-CORE-2`)
- **What we found by script:**
  1. **`G-STRATA-6` collision (`§13.4` L1605–1609 vs. `§13.11` L1831):**
     - In the `§13.11` guard table (L1831), `G-STRATA-6` is already defined as *"QA/QC invalidation or new evidence in the region after the cut → Mark `Stale`; block use in disclosure without re-run"*.
     - In `§13.4` (L1605–1609), `BD-316` (L7490), and `Appendix D.3` (L7866), `G-STRATA-6` is also used for *"Support-Consistent Sub-Cell Accumulation Regularization before `ILR`/`PPMT`/normal-score back-transforms (`FMINE-REF-103`)"*.
     - **Fix:** Renumber the sub-cell non-linear transform regularization guard to **`G-STRATA-11`** in `§13.4`, `BD-316`, and `Appendix D.3`, and add `G-STRATA-11` as row 11 of the `§13.11` guard table.
  2. **Unindexed `Corebox` guards (`G-CORE-1`, `G-CORE-2`):**
     - `GB-08` (L921) cites `G-CORE-2` for an uncharacterized grade-control observation error $\sigma^2_\varepsilon$, while `§12.4` (L1463) defines inline guard `G-TWIN` (twin-hole bias corrections cannot enter `Public`/disclosure projections without QP acceptance). Unlike `§13–§25`, `§12.6` has a `PO-CORE-1..5` table and a `GATE-CORE-01..02` table, but no `G-CORE-*` table.
     - **Fix:** Add a two-row guard table to `§12.6` defining **`G-CORE-1` (`G-TWIN`)** and **`G-CORE-2` (`UncharacterizedObservationError`)**.

### Finding 4 · Refusal-code registry completeness (`FMINE-REF-108` and `FMINE-REF-504`)
- **`FMINE-REF-108` (`AssayLagLookahead`):**
  - Defined in `§25.6` (`G-TIME-2`, L5223) and `BD-309` (L7164) to block operational hindcasts (`as_of_decision`) when an assay's `availability_time > t_dec`.
  - However, `§8.4`'s summary table (L674) still lists only `101..107` in the `FMINE-REF-1xx` row, and `§25.6`'s prose intro (L5206–5210) still states *"The refusal catalogue (§8.4) gains three domain-law refusals"* (`105, 106, 107`) instead of four (`105, 106, 107, 108`).
  - **Fix:** Add `108 assay-lag lookahead in operational hindcast (§5.1, G-TIME-2)` to `§8.4` (L674) and `§25.6` (L5210).
- **`FMINE-REF-504` (`AdjacentFillUncured`):**
  - Properly listed in `§8.4` (L678), `§16.4` (L2550), `§16.10` (`G-HEAD-7`, L2749), and `BD-320` (L7539).
  - **Fix:** Also mention `FMINE-REF-504` in `§31.3` (L6382) alongside `FMINE-REF-401..404` so the physical-safety refusal list in `§31.3` cross-references underground stope-sequencing backfill cure refusal.

### Finding 5 · Proof-obligation citation typo in `BD-315` (`PO-LODE-5` vs. `PO-LODE-13`)
- In `§39` (`WP-15`, L7473), `BD-315` writes:
  *"Two-level Pad/Collar/Wedge Tree-Matroid submodular drill portfolio optimization (`PortfolioCertificate` with Lagrangian pad-activation dual bound, PO-LODE-5)"*.
- In `§11.9` (L1322 vs. L1325), `PO-LODE-5` is MapMark4 endowment parity, whereas **`PO-LODE-13`** is the greedy submodular portfolio ratio guarantee.
- **Fix:** Change `PO-LODE-5` to `PO-LODE-13` in `BD-315` (L7473).

### Finding 6 · Round-2 Brenner pass `B6` is now complete (`H-2` ready to close)
- `planning/brenner/B6_operations_plant_tailings_workforce.md` has been written in full (covering `Headframe` §16, `Crucible` §17, `Freeboard` §18, and `Muster` §19; 17 hypothesis verdicts including 4 forced third alternatives; discriminating experiments `T6-1` and `T6-2`; galaxy-brain cards `GB-40`–`GB-43`; and guards `G-CRUC-6`, `G-FREE-11`, `G-MUST-6`).
- **Locations in `COMPREHENSIVE_PLAN_FOR_THE_DESIGN_OF_FRANKENMINING.md` to update when folding in `B6`:**
  1. `§0.6` item 3 (L162): update `"Five refuter-first Brenner passes (B1–B5)"` → `"Six refuter-first Brenner passes (B1–B6)"`.
  2. `§16.11` (after L2808): add **GB-40 · AHS Geo-Fence Bubble Contagion vs. Match Factor (`B6-G1`)**.
  3. `§17` banner (L2811), `§17.10` guard table (L3029), and `§17.11` (after L3097): update banner to cite `B6-SQ2`, add guard **`G-CRUC-6`** (`TautologicalTonnageMcf`), and add **GB-41 · Concentrate Assay Exchange, Splitting Limits & Penalty Cliff Blending (`B6-G2`)**.
  4. `§18` banner (L3101), `§18.8` guard table (L3335), and `§18.10` (after L3396): update banner to cite `B6-SQ3`, add guard **`G-FREE-11`** (`UnmappedClosureInventory`), and add **GB-42 · The Perpetual Water-Treatment Tail & Progressive Cover Option (`B6-G3`)**.
  5. `§19` banner (L3400), `§19.5` (L3502), `§19.8` guard table (L3560), and `§19.9` (after L3603): update banner to cite `B6-SQ4`, add Beta-Binomial capture-recapture CCV verification-quality scoring, add guard **`G-MUST-6`** (`TickAndFlickSuppressionAlarm`), and add **GB-43 · Goodhart on Safety Pay — When "300 Checks, 0 Defects" Proves Blindness, Not Safety (`B6-G4`)**.
  6. `§28.6` (L6029): update `(B1–B5)` → `(B1–B6)`.
  7. `§36.4` (L6846) and `Appendix H.6` (`H-2`, L8052): mark `B6` / `H-2` **Fixed / Complete** (`planning/brenner/B6_operations_plant_tailings_workforce.md`).
  8. `Appendix B` (L7625–7670): add the 17 `B6` verdict rows (`B6 · SQ-1` through `B6 · SQ-4`).
  9. `Appendix D.2` (L7818): add index rows for `GB-40` (`§16.11`), `GB-41` (`§17.11`), `GB-42` (`§18.10`), and `GB-43` (`§19.9`).

### Finding 7 · Explicit bead-level dependency edges on `BD-309` (`QuadTemporalStamp`)
- `BD-309` (`WP-01`) implements `QuadTemporalStamp` and guard `G-TIME-2` (`FMINE-REF-108: AssayLagLookahead`).
- In `WP-08`, `BD-161` (*Rolling-origin hindcast harness*) and `BD-312` (*3D Stockpile Kinematic & Geochemical Residence-Time Twin and 3D Heap-Leach Percolation Convolution*) both rely on `QuadTemporalStamp` to separate `collection_time`, `availability_time`, and `record_time`.
- Although `WP-08` already transitively depends on `WP-01` (`WP-08 → WP-07 → WP-06 → WP-04 → WP-02 → WP-01`), adding `BD-309` explicitly to the `Depends on` column of `BD-161` and `BD-312` guarantees at the bead-scheduler level that neither hindcast nor heap-leach attribution runs before `G-TIME-2` is live, with zero change to the work-package DAG or the 29-bead critical path.

### Finding 8 · `fmine-time` description in `§28.3` (L5873) and crate LOC budget check
- In `§28.3` (L5873), `fmine-time`'s one-line responsibility still reads `"Bitemporal types, TimeEvidence, site and fiscal calendars, rosters"`.
- **Fix:** Update L5873 to `"Quad-temporal types (QuadTemporalStamp), TimeEvidence, site and fiscal calendars, rosters"`.
- **LOC budget audit:** We checked whether the 12 `BD-309..BD-320` additions strain any crate's plan/ceiling LOC in `§28.3`:
  - `fmine-time` (`8k` plan / `9k` ceiling): `QuadTemporalStamp` + `G-TIME-2` is `< 0.5k` LOC; fits within `8k`.
  - `fmine-array` (`6k` plan / `7k` ceiling): `FMT-COL-1` Arrow-compatible SIMD interval chunks + bitmask pushdown is `~1.0k` LOC; fits within `6k`.
  - `fmine-calib` (`12k` plan / `14k` ceiling): `Monitor::SpatialFrontPit` Vecchia/Rosenblatt whitening (`k ≤ 16` Cholesky) is `~0.8k` LOC; fits within `12k`.
  - All 59 crate budgets and layer/phase totals (`1,128k` plan / `1,298k` ceiling) remain intact.

### Finding 9 · Newly downloaded 2024–2025 Barrick Mine Stats & AISC workbooks close the M0 data gate
- `corpus_of_input_docs/downloaded/mine_stats/` now contains all 8 quarters of `2024` and `2025` (`Barrick_Q1_2024` through `Barrick_Q4_2025` `Mine_Stats.xlsx`, `AISC.xlsx`, and `MD-A.pdf` — 30 files in total), alongside the existing `Q1_2026` and `Q2_2026` workbooks.
- This closes the M0 multi-quarter public-workbook acquisition gate noted in `§21.6`, `§37.3` (`M0`), `Appendix I.5`, and `Appendix I.7 #1`.

---

## 3. Checks That Came Back 100% Clean

1. **Internal `§` Cross-References:** All 800+ internal `§` section and subsection references across all 8,262 lines resolve to existing headings or bold subheadings (`§21.3.1–21.3.7`). The only two regex flags (`§15.2.12` on L2232 and `§1.5` on L3499) are external citations to the Pueblo Viejo NI 43-101 report and research report `R6`, respectively.
2. **Bead Graph (`§39`) & Work-Package DAG (`§38.1`):**
   - Verified by script: **320 beads (`BD-001..BD-320`)** with zero gaps or duplicates, **531 dependency edges**, **0 cycles**, **0 unknown dependencies**, **1 root (`BD-001`)**, and a **29-bead longest chain** (`BD-001 → ... → BD-301`) identical to the one stated in `§39` (L7103).
   - Every cross-package bead dependency across all 320 beads lies inside the transitive closure of `§38.1`'s `Depends on` column.
3. **Crate Map (`§28.3`) & Layer Rules (`§28.2`):**
   - 59 crates + 1 gated crate (`fmine-simbridge`).
   - Per-layer crate counts and LOC totals match `§28.3` exactly: `S0 (8 / 89k / 102k)`, `S1 (7 / 107k / 123k)`, `K (20 / 446k / 514k)`, `E (10 / 232k / 267k)`, `V (5 / 108k / 124k)`, `U (5 / 78k / 90k)`, `T (4 / 68k / 78k)` → **Total: 59 crates, 1,128k plan, 1,298k ceiling**.
   - Per-phase counts and LOC totals match `§28.3` exactly: `Phase A (35 ★ / 615k)`, `Phase B (18 / 392k)`, `Phase C (6 / 121k)`.
4. **Mathematical Verification of New Cards and Equations (`BD-309..BD-320` & `B6`):**
   - **GB-08 (`§10.4`):** Under $\phi = 0.65$ exponential spatial correlation ($d = 10\text{ m}, r = 25\text{ m}$), unwhitened variance inflation is $1 + 2\sum_{h=1}^{19}(1 - h/20)0.65^h = 4.35\times$, inflating nominal $\alpha = 0.05$ Ville alarm rate to $\approx 0.41$; Vecchia/Rosenblatt conditional PIT whitening restores $\text{Var}(\bar{z}_t) = 1/m$ and Ville validity $\le 0.052$. Verified.
   - **Pueblo Viejo `Table 15-4` & `Table 15-8` (`§15.6`, `§17.3`):** At $S = 0.0904$ and $P = \$0.07912/\text{kWh}$, $c_{\text{proc}}(0.0904) + c_{\text{pwr}}(0.0904, 0.07912) = \$7.3036 + \$7.4530 = \$14.7566/\text{t}$, matching `Table 15-8` (`$14.76/t`). Verified.
   - **GB-40 (`B6-G1`):** 3 light-vehicle geo-fence intrusions/hr into an 11-truck / 2-shovel AHS loop ($36.2\text{ min}$ base cycle) add $1.8 + 0.9 + 3.63 = 6.33\text{ min}$ ($+17.49\%$ cycle time), reducing shift output from $45{,}750\text{ t}$ to $39{,}750\text{ t}$ ($-13.1\%$ against the $45{,}750\text{ t}$ AHS baseline). Verified.
   - **GB-41 (`B6-G2`):** Lot mass $10{,}000\text{ dmt}$, $\Delta g = 1.1\text{ g/t}$, $\sigma_{\text{lot}} = 0.35\text{ g/t}$, $z = 1.65 \implies \mathbb{P}(\text{win}) = 0.9505$, EV of calling umpire $= +\$478\text{k}$ under `Touchstone` bias detection vs. $-\$19\text{k}$ under blind splitting. Penalty blending across $0.25\%$ and $0.14\%$ As lots eliminates $\$300\text{k}$ step-cliff penalty. Verified.
   - **GB-42 (`B6-G3`):** 100-year $\$10\text{M/yr}$ real ARD water-treatment annuity jumps from $\$276.55\text{M}$ at $r = 3.5\%$ to $\$366.14\text{M}$ at $r = 2.5\%$ ($+32.4\%$ for 100 yr; $+36.4\%$ on a perpetual/150-year horizon: $\$285.7\text{M} \to \$390.2\text{M}$), and progressive cover ($c_{\text{prog}} = \$18\text{M}$ PV) reducing steady-state flow by $65\%$ yields $+\$159\text{M}$ net PV at $3.5\%$ ($+\$220\text{M}$ at $2.5\%$). Verified.
   - **GB-43 (`B6-G4`):** Beta-Binomial zero-defect probability for $N = 300$ routine supervisor CCVs against an independent blinded audit posterior $\text{Beta}(4, 48)$ ($3$ defects in $50$ audits, $\hat{p} = 7.7\%$) is $\frac{48\cdot 49\cdot 50\cdot 51}{348\cdot 349\cdot 350\cdot 351} = 4.02 \times 10^{-4}$ ($1\text{ in }2{,}487$), yielding Bayes factor $B_{10} = 0.342 / 0.000402 = 851$ for tick-and-flick suppression. Verified.
