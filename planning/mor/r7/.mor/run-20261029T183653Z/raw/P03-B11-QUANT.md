RESULT_STATUS: MATERIAL_INCREMENT

# P03-B11-QUANT · PU-3 · Fermi estimation (B11), reference-class forecasting (B10), statistical reasoning (B2)

## WHAT I CHECKED

**Read.** `planning/mor/r7/PASS_BRIEF_COMMON.md`; `planning/mor/r7/MOR_CASE.md` (Decision Contract, E673–E690, PU-1…PU-5,
Source Access). Plan (`COMPREHENSIVE_PLAN_FOR_THE_DESIGN_OF_FRANKENMINING.md`, 8,902 lines, E682): §28 (6220–6428),
§29 (6430–6550), §33–§37 (6959–7558), Appendix D (8259–8340), Appendix E (8342–8390), Appendix H/H.5b (8493–8638),
plus the gate rows each M0–M2 gate points to: §10.2–§10.4 (886–1000), GATE-PLUMB table (1017–1026), GATE-CORE (1574–1575),
GATE-CRUC (3243–3245), §20.10 (4105–4124), §21.6–§21.9 (4408–4646), GATE-HELM (4765–4766), §24.9, §26.12–§26.13
(5944–5990), GATE-ROBOT (6678), GATE-WALL (6868). `registries/loc_budget.toml`; `crates/xtask/src/lib.rs` lines
704–713 (LOC = non-blank lines of `.rs` files). `planning/mor/audits/AUDIT_1` line 86 and `AUDIT_5` lines 119–124 (to
avoid re-verifying); `planning/research/W1` §5 (lines 230–282); `planning/research/W4` §3.3 (lines 140–215);
`planning/brenner/B4` card G1 (lines 622–645).

**Not redone.** Every card AUDIT_1 line 86 lists as reproduced (GB-01, 02, 04, 05, 07, 09–21, 24–27 synthetic parts,
29, 30, 32–34, 38 at p = 0, 0.5, 1); AUDIT_5's GB-08 variance-inflation check and GB-40–GB-43; H.5b item 7.

**Executions (read-only; no cargo; no repository file changed except this one).** Scripts and their outputs are in
`/private/tmp/claude-501/-Users-jemanuel-projects-franken-mining/2ba72fbb-c45d-4fd8-93cf-c08cb26d7a18/scratchpad/r7_repro/P03-B11-QUANT/`:
- `sibling_loc.py` (git ls-files and git log only) over 25 FrankenSuite siblings → `sib1.jsonl`, `sib2.jsonl`; `rg -c`
  of test annotations in five siblings; `stat` birth and modification times of `crates/*/src/*.rs`.
- `xlsx_grep.py` (stdlib zip/XML reader) on `corpus_of_input_docs/downloaded/mine_stats/Barrick_Q*_Mine_Stats.xlsx`;
  `pdftotext -layout Barrick_Q4_2025_MD-A.pdf`.
- `t8_scan.py`, `t3_scan.py`, `t3_scan_round.py`, `ceiling.py`, `ceiling_layers_excluded.py` over the ten Mine Stats
  workbooks (2024Q1–2026Q2) → `t8_out.txt`, `t3_out.txt`, `t3_round_out.txt`, `ceiling_out.txt`,
  `ceiling_layers_excluded_out.txt`.
- `calc.py` (card arithmetic) → `calc_out.txt`; `gates.py` (exact binomial pass probabilities) → `gates_out.txt`.
- Read `planning/mor/r7/.mor/run-20261029T183653Z/repro/assure_run1.json` (E676).

**Part (b) recomputations (14 items; none in AUDIT_1 line 86, AUDIT_5 or H.5b).** Results that fail are claims C1, C3,
C11, C12. Results that hold are listed in C13.

| # | Claim (plan line) | Recomputed | Verdict |
|---|---|---|---|
| 1 | GB-03 (856–865) | P(≥1) = 1 − 0.625^0.5 = 0.2094; plug-in 0.2592 (+23.8%); P(≥2) 0.0612 vs 0.0369; plug-in error < 3% needs a > 4.3 | Holds (flip "a > 5" is sufficient) |
| 2 | GB-08 monthly e-values (962–964) | B(k+1, n−k+1)/(0.8^k 0.2^(n−k)) = 1.447, 4.529, 83.5 | Holds |
| 3 | §10.4 "3–5 years to alarm at 58%" (934) | Exact first passage of e ≥ 20 at p = 0.58: median 26 months | Overstated (C12) |
| 4 | GB-28 (4553–4566) | Writer: 4.85×10⁵ s, ρ = 5.61; review ρ as printed = 37.5, not 7.5 | Unreproducible as printed (C11) |
| 5 | GB-35 (5960–5973) | S = 26.5 ms, ρ = 0.106, W_q = 0.79 ms (needs C_s² = 0.5, unstated); fgdb ρ = 4 at 10³/s | Holds |
| 6 | GB-36 (5975–5989) | 1.19 GB, 31.7 min; 9,504 B; Merkle diff 943,104 B vs 65,536 B manifest | Holds (10⁸×20×4 B is 7.45 GiB, 1,907 chunks; conclusions unchanged) |
| 7 | GB-37 (6529–6549) | k = ln(10¹³)/ln 1.01 = 3,008 (2,928 with δ₀ = 2⁻⁵²) | Holds |
| 8 | GB-38 p = 0.3 row (7360) | H-f 4.7; H-a, H-e, H-d 2.3; H-b 0.6; H-c −1.4 | Holds |
| 9 | §34.4 P(site data by month 7) (7163–7179) | 0.6×0.8×0.6 = 0.288; odds ×2 → 0.46, ×1.5 → 0.39, ×3 → 0.56, ×0.5 → 0.18 | Holds (see C13 note on alternates) |
| 10 | §37.3 Lumwana gap value and "one point ≈ 2.6 kt" (7429–7437) | 2023–2025 shortfall $31.7–57.0M/yr; 240/0.927×0.01 = 2.59 kt | Holds |
| 11 | GB-26 public case (4615–4618) | Q2_2026:QTD!N19 = 1,292 → G19 = 2,577: +1,285; effects sum to 1,285 | Holds |
| 12 | §21.6 corporate layer (4433–4439) | FY2025 +$61.0/oz (+$202M), FY2024 +$32.6/oz (+$124M) above sold-weighted regions | Holds (inside the stated +23.57 to +88.66 range) |
| 13 | GATE-BULL-01 justification "94.4% … so the 95% bar" (4524, 7401–7402, 8673–8675) | Reproduction ceiling for a perfect extractor 90–92% (literal rule) or 96–97% (layers bridged) | Fails (C1) |
| 14 | M0 envelope fields (E676) vs the workbooks they stand for | FY2025 AISC, sold ounces, L_corp sign, North Mara endpoints and period, reserve-replacement period | Fail (C3) |

## CANDIDATE CLAIMS

### P03-B11-QUANT-C1 · GATE-BULL-01's 95% bar sits above or barely below what a perfect extractor can reach

- **Type.** deductive (with a measured input).
- **Statement.** Under the plan's own definition of "reproduced" (value equals the cell **and** every applicable
  identity holds to ±1 unit of the last published digit, otherwise a register item that counts as "explained, not
  reproduced"), a perfect extractor working on the ten known workbooks reproduces at most **90.5–92.4%** of numeric
  site cells. That is below the 95% threshold, so the convergence gate cannot be passed as written. If the NGM and
  corporate aggregation layers are treated as bridged and reproduced, the ceiling is **96.8–97.4%**. The 95% bar then
  leaves 1.2–2.4 points of slack, about 230–450 cells, so it cannot tell a correct importer from one that misreads
  about 2% of clean cells. The plan's justification also mixes units. "84/89 (94.4%)" is the pass rate of one
  identity family (attributable = ownership × 100%) over two files. The gate counts cells.
- **Scope and definitions.** Denominator: 18,860 numeric data cells in labelled rows of the visible QTD, YTD and
  Minestats sheets, all vintages 2024Q1–2026Q2 (comparative columns included). Identity families tested in-file:
  attributable (T1), Σ components for capex (T8), and sold-weighted roll-up of CoS, TCC/C1 and AISC (T3). Q1 + Q2 = H1
  (T5) is cross-file and is added as W1's bound of ≤ 120 cells. Horizon: M0 (GATE-BULL-01, plan lines 4590,
  7397–7402).
- **Evidence.** Plan lines 4520–4524 (definition and 94.4%), 7397–7402, and 8673–8675 (Appendix I.2 item 2). Appendix
  I.2 item 3 (8676) says company AISC is not the sold-weighted sum of its sites. §21.6 item 4 (4433–4439) names L_corp
  and L_JV as layers. E113–E115; W1 §5 table (lines 248–265; research rows with no second reader, H-4). New evidence
  (`t3_out.txt`, `t3_round_out.txt`, `t8_out.txt`, `ceiling_out.txt`, `ceiling_layers_excluded_out.txt`):
  - Sold-weighted roll-up, 612 instances. 94 fail at W1's 1% tolerance, which is exactly W1's tally. 270 fail at
    ±1 unit (44%). 191 fail with rounding-propagated tolerance (31%): NGM level 51/102, regions 44/306, totals 96/204.
  - Capex Σ components, 593 instances. 69 fail, exactly W1's T8 tally. All 69 have total > sustaining + projects, by
    +2 to +25 $M. That fits the footnoted "Includes capitalized interest", a third component that is not published
    per site (e.g. `Q2_2026:Minestats1!C49/C50/C51` = 78 + 18 vs 98).
  - Attributable, 504 instances: 17 fail (tolerance ±1) or 32 fail (W1's tolerance).
  - Distinct cells in at least one failing identity: 1,438–1,801 (7.6–9.5%), so the ceiling is 90.5–92.4%, or
    89.8–91.7% with the T5 bound. With the NGM and total roll-ups treated as bridged: 493–598 cells, so the ceiling
    is 96.8–97.4%, or 96.2–96.7% with T5.
- **Inference.** Reproduced share ≤ 1 − (identity-implicated cells ÷ denominator) for any extractor. Identity failures
  are mostly structural (named layers, unpublished capitalized interest), not keying errors. Software quality cannot
  remove them; only the definition can.
- **Assumptions.** "Every applicable identity" makes every participating cell non-reproduced (the literal reading of
  4520–4523). Group membership for roll-ups follows sheet order (validated: my independent implementation hits W1's
  612/94 and 593/69 exactly). The gate's denominator, if deduplicated to first publication, scales numerator and
  denominator alike.
- **Alternative or defeater check.** (i) An implementer reads "applicable" to exclude layered aggregates. That is the
  bridged case, which I computed, and the slack argument still holds. (ii) The tolerance is rounding-aware rather than
  ±1 unit. Computed, and the ceiling is still < 95% under the literal identity set. (iii) Different row grouping by
  the real importer. This could shift the counts by tens of cells, not by the 600–1,300 cells needed to reverse the
  conclusion. NONE_IDENTIFIED_AFTER_BOUNDED_CHECK beyond these.
- **Resolution or reversal.** Run the M0 importer on the ten real files and publish, per identity family, the
  instances, failures and implicated cells. The claim reverses if the ceiling under the adopted rule is ≥ 99%.
- **Materiality.** High. This is the M0 gate and the first number shown to Barrick finance. The loss function ranks
  "a gate that cannot be passed as written" and "a gate that passes vacuously" among the costliest outcomes.
- **Uncertainty basis.** Measured on all ten workbooks with two independent tallies agreeing; residual uncertainty
  is in the definition, not the data.

### P03-B11-QUANT-C2 · Recommendation: split GATE-BULL-01 into extraction fidelity and identity accounting

- **Type.** recommendation. **Supports:** C1.
- **Plan edits.**
  - §21.6 lines 4520–4524, §21.8 line 4590, §37.3 lines 7397–7402 and Appendix I.2 item 2 (8673–8675) must assert:
    1. **Extraction fidelity.** 100% of numeric cells in the denominator are extracted equal to the published value,
       each with its locator. Every miss is a defect and is listed. This is measured separately on the ten known
       files and on Q3 2026.
    2. **Identity accounting.** Every identity instance is evaluated with rounding-propagated tolerance. Every
       failure is a register item with a named cause class (aggregation layer L_JV or L_corp; unpublished component
       such as capitalized interest; keying error; restatement) and closure arithmetic. Unexplained failures: 0.
    3. **Issuer consistency** per identity family is reported as a finding, with no threshold.
  - Delete "so the 95% bar is set against the issuer's own consistency". Keep the kill (< 80%) on fidelity.
- **Cost.** Low (text, plus counts the importer must compute anyway). **Reversibility.** Full.
- **Acceptance test.** The gate record lists the denominator N, extraction misses (target 0), and identity instances
  and failures by family with cause classes. Rerunning `ceiling.py` on the same files reproduces the counts.

### P03-B11-QUANT-C3 · The M0 envelope prints Barrick numbers and labels that do not match the cited workbooks

- **Type.** observation and deductive.
- **Statement.** Fields of `fmine assure public --issuer barrick` (E676) differ from the public cells they stand for:
  - **FY2025 AISC.** `reported_aisc_per_oz` = 1,638. Q4_2025:YTD!G22 and the Q4 2025 MD&A both say $1,637/oz (the
    plan says 1,637 at line 4430).
  - **FY2025 ounces.** `consolidated_sales_koz` = 3,255 is attributable **production** (C22). Sold is 3,318 (D22), and
    the plan says public unit costs are per ounce sold (E110).
  - **FY2024 ounces.** `consolidated_sales_koz` = 3,973 matches no cell. Produced is 3,911 and sold is 3,798
    (Q4_2024:YTD!C23, D23).
  - **Corporate layer.** `l_corp_adj_usd_m` = −49 (FY2024) and −8 (FY2025). The cells give a **positive** layer:
    1,637 − 1,576.0 = +$61.0/oz (+$202M) for FY2025 and 1,484 − 1,451.4 = +$32.6/oz (+$124M) for FY2024, which matches
    the plan's always-positive layer (E496).
  - **Rounding bridge.** `rounding_bridge_per_oz` = 3.51. One $M term rounding moves AISC by at most 0.126 $/oz on
    3,973 koz, so 3.51 needs ≥ 28 terms all rounding the same way. It is not rounding.
  - **North Mara.** `north_mara_lmdi_2024q1_q2` = 1,297 → 2,582. The 2024 cells are 1,753 → 1,491
    (Q1_2024:QTD!G20, Q2_2024:QTD!G20). The +1,285 delta is the plan's Q2 2025 → Q2 2026 case, whose cells are
    1,292 → 2,577 (Q2_2026:QTD!N19, G19). So the period label is wrong and both endpoints are off by +$5.
  - **Reserve replacement.** `reserve_replacement_2024` "organic 190 / headline 504" equals 7.429/3.91 and
    (7.429 + 12.277)/3.91 (E679). The plan, from primary sources (E603, E604; lines 4639–4645), says both are
    three-year 2023–25 figures on gold-ounce and GEO bases, not one year and not organic vs headline.
- **Scope.** M0 output at HEAD `68e7091`, built from the in-code substitute workbook (E677, E678).
- **Evidence.** E676, E678, E679; new evidence: `xlsx_grep.py` rows quoted above; `pdftotext` of Q4 2025 MD&A
  (lines 352–358: produced 3,255, sold 3,318, AISC 1,637); `calc_out.txt` (b10).
- **Inference.** The demonstrator "reproduces" headline values that were built into its inputs, and it mislabels
  period and basis. The arithmetic is not what fails; the inputs and labels are.
- **Alternative or defeater check.** A later vintage could restate FY2025 AISC to 1,638. I checked the Q4 2025 MD&A
  and the Q4 2025 workbook (both 1,637); the 2026 workbooks carry no FY2025 column. The substitute workbook could be
  a placeholder, removed once the real files load. That would not stop the current binary printing these values with
  exit 0. Overlaps PU-1; my contribution is the cell-level arithmetic.
- **Resolution.** Rerun on the real files at the registered paths and diff each payload field against its cited cell.
  The claim reverses if every field matches.
- **Materiality.** High: a false Barrick number with a reversed sign is the loss function's costliest case.
- **Uncertainty basis.** Direct cell reads, cross-checked against the MD&A.

### P03-B11-QUANT-C4 · Code volume will not bind M0–M2 dates; the LOC envelope and human gates will

- **Type.** forecast (reference class) with deductive arithmetic.
- **Statement.** The plan implies about **1,030–1,070 non-blank Rust LOC per calendar day**: a 390k phase-A slice in
  12 months (§28.3 line 6340), and 1,128k in 36 months. That equals the **slowest** of 25 FrankenSuite siblings
  (franken_networkx, 1,106/day). The median sibling ran 3.3× faster and the fastest 36× faster. Across the 25
  siblings, lifetime non-blank Rust LOC per day is:
  - minimum 1,106; Q1 1,872; median 3,539; Q3 11,217; maximum 38,541;
  - franken_accounting: 770,820 LOC in 20 days;
  - franken_textiles: 313,337 in 17 days.

  At sibling rates the 1,298k program ceiling is reached in about 12 months at the median, 3.8 months at Q3 and 1.1
  months at the maximum. So either FrankenMining runs at or below the slowest sibling, because something other than
  code binds, or the §28.4 80/100/115% triggers fire within months and BUDGET-AMEND becomes routine. Tests are not a
  constraint either. M0 has 6.7 tests per kLOC (59/8.859k, E674) against 6.3–9.5 in five siblings, which implies about
  2,500–3,700 tests for the phase-A slice. What binds M0–M2 dates is external and human:
  - the Q3 2026 release (9 Nov 2026) and the FY2026 release (Feb 2027);
  - two named finance reviewers (M0 kill);
  - the executive panel (GATE-HELM-01, KILL-PLUMB-5);
  - GATE-DATA-01…03 by months 4–7 at p ≈ 0.3;
  - GATE-SEC-01's independent penetration test;
  - QP or metallurgist attestation (GATE-TOUCH-04).

  None of these is sized in the plan.
- **Scope and definitions.** LOC = non-blank `.rs` lines tracked at HEAD (same rule as `xtask`, lib.rs 704–713).
  Rate = LOC at HEAD ÷ days from first to last commit (idle days included). Horizon: months 0–12.
- **Evidence.** E675 (8,859 LOC; planned 1,128,000; ceiling 1,298,000); E674; new evidence `sib1.jsonl`, `sib2.jsonl`
  (25 repos, read-only git); test-annotation counts (FA 4,818; FTX 2,790; FSS 11,063; frankensim 22,802; fgdb 12,952).
  `crates/*/src/*.rs` birth times run 12:51–13:17 and modification times run to 13:39 on 2026-10-29 (the authoring
  duration is HYPOTHESIS; files may have been copied in). Plan §28.3 lines 6338–6349; §28.4; RISK-25 line 7242; F10
  line 7206.
- **Inference.** Reference-class (same owner, same agent workflow, same language) arithmetic.
- **Assumptions.** Sibling HEAD LOC counts tests and generated code in the same proportion as FrankenMining will.
  Lifetime averages understate burst rates.
- **Alternative or defeater check.** If sibling LOC were inflated ≥ 3× by generated code relative to FrankenMining's
  count, the median would fall to the plan's rate and LOC could bind. No sign of that at the scale required:
  test-density ratios are similar, 6.3–9.5 per kLOC against M0's 6.7. Domain verification (oracles, real data) may slow
  FrankenMining. That supports the claim that non-code steps bind.
- **Resolution.** FrankenMining's own non-blank LOC per day and gate records per month over its first 30–60 days. The
  claim reverses if LOC per day < 1,100 while no human or external gate is pending.
- **Materiality.** Medium. It changes how §28 and RISK-25 are framed and what counts as progress.
- **Uncertainty basis.** n = 25 siblings, wide spread (35×); quartiles reported, not a point.

### P03-B11-QUANT-C5 · Recommendation: treat LOC as a scope cap, measure progress in gate records, and calendar the human gates

- **Type.** recommendation. **Supports:** C4 (and C3 for why LOC and bead closure mislead).
- **Plan edits.**
  - **§28.3 lines 6344–6349.** Add the velocity reference class (quartiles above). State that the envelope is a scope
    cap, not an effort estimate.
  - **§28.4.** Add an early-warning rule: program LOC above 2× the pro-rata phase-A trajectory triggers a scope review.
  - **RISK-25 (line 7242).** Raise likelihood from Medium to High. Use that rule as the leading indicator.
  - **§37.3 milestones.** Add, per milestone, a dependency calendar of named people and external dates: finance
    reviewers by M0 week 4; executive panel by M1; site sponsor by month 2; pen-tester engaged by month 4.
  - **§34.1 or §37.** State that M0–M2 progress is reported as gate records (measured value, anchor, certificate), not
    LOC or closed beads. E680 shows 87 closed beads while AGENTS.md says nothing is implemented in the §0.2 sense.
- **Cost.** Low. **Reversibility.** Full.
- **Acceptance test.** §28.3 cites the reference class. `fmine doctor` or `xtask` prints LOC per day and the
  gate-record count. Each milestone lists its human dependencies with names and dates.

### P03-B11-QUANT-C6 · By month 12 no real-world calibration claim is statistically supportable

- **Type.** deductive (forecast of available sample size).
- **Statement.** The scorable real-world outcomes that exist by the month-12 program gate cannot tell 80%-calibrated
  intervals from 60% ones. Separating 80% from 60% coverage (one-sided α = 0.05, power 0.8) needs ≥ 35 independent
  outcomes (exact binomial; reject if ≤ 23 covered). Separating 80% from 70% needs 116. What will exist:
  - **FY2026 per-site public forecasts (M0, BD-067/BD-076).** There are 14 reporting sites (11 gold, 3 copper;
    Q2_2026:QTD rows 5–28) and one period. With 42–56 site-metric outcomes and within-period correlation
    ρ = 0.1–0.5, n_eff ≈ 2–9. If frozen after the 9 Nov 2026 Q3 release, about three quarters of the outcome is
    already known: Q4 was 26.8% of FY2025 attributable production (871/3,255) and 28.9% of ounces sold (960/3,318).
    The 80% FY interval therefore tests about one quarter.
  - **GATE-TOUCH-05 hindcast.** 8–12 quarters, rolling origin, so after a ≥ 4-quarter burn-in 4–8 are scored.
    P(≤ 4 of 8 covered | calibrated 0.8) = 0.056, so F7's "missed 4 of 8" is itself borderline evidence.
  - **Two forward quarters.** P(both covered | calibrated) = 0.64.

  The plan's `uncalibrated` default and "small-sample caveat" are consistent with this. GB-38's assumption "a
  pre-registered scored forecast is persuasive" (7365) rests on n_eff of a few.
- **Evidence.** Plan 7393–7394, 7412, 4123, 7203, 7365; new evidence `gates_out.txt` (small-n section),
  `xlsx_grep.py` site rows; Q4 2025 MD&A lines 352–353.
- **Assumptions.** ρ range is an assumption (common gold-price and cost shocks across sites in one quarter). The
  burn-in is ≥ 4 quarters.
- **Alternative or defeater check.** Scoring CRPS instead of coverage uses more information per outcome, but it still
  cannot certify calibration at n_eff < 10. Synthetic-truth coverage (GATE-TOUCH-02) is not real-world calibration
  (§33.1). NONE_IDENTIFIED_AFTER_BOUNDED_CHECK.
- **Resolution.** Count independent scored outcomes at month 12. The claim reverses if ≥ 35 exist.
- **Materiality.** Medium–high. It bounds what may be said to the CFO at month 12 (F7, RISK-07).
- **Uncertainty basis.** Exact binomial; n_eff depends on the assumed ρ.

### P03-B11-QUANT-C7 · Recommendation: pre-register per-site quarterly public forecasts at every release from M0

- **Type.** recommendation. **Supports:** C6.
- **Plan edits.**
  - **§37.3 lines 7393–7394 and 7412; §21.6 line 4495; BD-067 (7710) and BD-076 (7719).** Add hashed per-site
    forecasts with 80% intervals and an as-of date before each Barrick release inside the window: Q4/FY2026 (Feb 2027),
    then Q1 2027 and Q2 2027 (expected about May and Aug 2027; exact dates are HYPOTHESIS until announced).
  - **FY2026 forecast.** State its as-of date and score the unknown remainder (Q4) separately.
  - **Every scoring report.** Give n, an n_eff estimate and the Clopper–Pearson interval for coverage.
- **Effect.** Three periods × 14 sites = 42 site-periods by month 12 against 14 now, roughly tripling n_eff. All on
  public data, so it serves M2′, the modal path at p ≈ 0.7.
- **Cost.** Low (same engine, public inputs). **Reversibility.** Full.
- **Acceptance test.** Three or more forecast sets, each timestamped before its release. Each score report states
  n, n_eff and the coverage CI.

### P03-B11-QUANT-C8 · GATE-PLUMB-E3 and GATE-CRUC-02 are small-n band tests that cannot separate calibrated from miscalibrated

- **Type.** deductive.
- **Statement.**
  - **GATE-PLUMB-E3 (line 1019; BD-259)**, "restores 80% coverage to ±5 points within 12 resolutions", gives no
    replicate count. Read as one run (n = 12), only 9/12 or 10/12 pass: P(pass | true 0.8) = 0.52, P(pass | 0.7) =
    0.41, P(pass | 0.6) = 0.21. Pooled over R replicate worlds: R = 10 gives 0.86 vs 0.14; R = 50 gives 0.998 vs
    0.004.
  - **GATE-CRUC-02 (line 3244; BD-263)**, "80% interval coverage in [0.70, 0.90]" over 12 monthly forecasts, passes
    only k ∈ {9, 10}: P(pass | 0.8) = 0.52, P(pass | 0.6) = 0.21, P(pass | 0.95) = 0.12. Pooling recovery and
    throughput (n = 24) gives 0.80 vs 0.19, before allowing for month-to-month autocorrelation. With plant data no
    earlier than GATE-DATA-03 (month 7), the twelfth monthly outcome resolves at month ≥ 19. WP-10 (line 7577) says
    "through month 18".
- **Evidence.** Plan lines 1019, 3244, 7577, 7158; new evidence `gates_out.txt`.
- **Plan edits implied.**
  - **E3:** "over R ≥ 50 replicate misspecified worlds, pooled coverage of 80% intervals over resolutions 1–12 after
    the alarm lies in [75%, 85%]".
  - **CRUC-02:** replace the two-sided band with a miscalibration test. Fail if the 90% Clopper–Pearson interval for
    pooled coverage excludes 0.80, and require the interval score to be no worse than the reference-class baseline.
    Alternatively, relabel it a reporting gate. Correct the month to ≥ 19.
- **Alternative or defeater check.** E3 may have been meant across replicates. The text and BD-259 do not say so,
  hence the edit. CRUC-02's fallback is benign ("cards stay Estimated"), which lowers stakes but does not make the test
  informative.
- **Resolution.** BD-259 and BD-263 specify R and the test statistic.
- **Materiality.** Medium.
- **Uncertainty basis.** Exact binomial; independence assumed, so real power is lower.

### P03-B11-QUANT-C9 · GATE-HELM-02's "within 2·SE across 100 seeds" fails an unbiased estimator under its literal reading

- **Type.** deductive.
- **Statement.** If every seed must fall within 2·SE, an unbiased estimator passes with probability
  0.9545¹⁰⁰ = 0.0095. If the mean of 100 seeds must fall within 2·SE of the mean, it passes with 0.954. If ≥ 90 of 100
  seeds must fall within 2·SE, it passes with 0.994. The text (line 4766) does not say which. The M0 code computes EVSI
  in closed form with no seeds (`crates/fmine-decide/src/lib.rs` line 73 calls `compute_evsi`), while E673 reports
  BD-227 closed. The Monte Carlo the gate tests is not yet exercised.
- **Plan edit.** Line 4766 and BD-227: "for each of K test problems, |mean of 100 seeds − closed form| ≤ 2·SE_mean,
  and ≥ 90 of 100 seeds within 2·SE_seed".
- **Evidence.** Line 4766; `gates_out.txt`; E673; fmine-decide source.
- **Alternative or defeater check.** None. Every reasonable reading is one of the three computed.
- **Resolution.** The text is fixed. **Materiality.** Low–medium (M1 gate). **Uncertainty.** None in the arithmetic.

### P03-B11-QUANT-C10 · The false-alarm gates name no sample size, and KILL-PLUMB-6 has no per-monitor budget

- **Type.** deductive.
- **Statement.**
  - **N and the test are unstated.** GATE-PLUMB-E5 (line 1021, "false alarms ≤ α") and GATE-CORE-02 (line 1575) give
    neither the number of null runs nor the test. With the plan's own simulated false-alarm rate of 3.8% (§10.4),
    a one-sided 95% Clopper–Pearson bound ≤ 0.05 needs about **1,000** null runs: N = 600 gives 0.054 and N = 200
    gives 0.071. Tested as a point estimate instead, a monitor at exactly α passes about half the time.
  - **KILL-PLUMB-6 (line 1026)** caps false alarms at 1 per site per quarter summed over M monitor streams. That leaves
    each stream 1/M per quarter. At about 30 polygons a quarter, a CUSUM stream needs ARL₀ ≥ 30·M polygons (600 at
    M = 20). The plan gives no M, no CUSUM h and no ARL₀.
- **Plan edits.** Lines 1021, 1026 and 1575: state N ≥ 1,000 null runs, the bound used, M per site, and each CUSUM's
  ARL₀.
- **Evidence.** Lines 935–951, 1021, 1026, 1575; `gates_out.txt`.
- **Alternative or defeater check.** The anytime-valid monitors' own guarantee (P₀ ≤ α over the whole horizon) makes
  per-quarter false alarms rare. CUSUM has no such guarantee, which is why the budget matters.
- **Resolution.** Gate texts carry N, M and ARL₀. **Materiality.** Medium–low (M1 gates; one kill gate).
  **Uncertainty.** The M range is assumed.

### P03-B11-QUANT-C11 · GB-28 cannot be reproduced as printed, and monthly review may be infeasible once close steps count

- **Type.** deductive.
- **Statement.**
  - **Review load as printed.** "60 independent reviews per book per day … ρ_review ≈ 7.5" (line 4561): 60/day × 30
    × 10 min ÷ 480 min = **37.5**. The 7.5 needs B4's unstated assumptions: 15 sites in one book; each controller
    reviews their own site's 4 families × 3 books per site (B4 lines 626, 641).
  - **Writer time.** "≈ 29 min/day in year 1" is the end-of-year value (t(n) ≈ 29.3 s at n ≈ 27,300), not the year
    average.
  - **Monthly review.** "ρ_review ≈ 0.25" counts only the 4 families. B4's own demand line has 16 close steps per
    site per month as well. If each needs the independent human review G-BULL-2 requires, ρ = (4 + 16) × 3 × 10 ÷ 480
    = **1.25**, which is infeasible at 8 h a month, or 0.42 with one book per site.
- **Evidence.** Plan lines 4553–4566, G-BULL-2 (4572); B4 lines 622–645; `calc_out.txt` (b3).
- **Plan edit.** Card GB-28 states its three assumptions (15 sites per book, 3 books per site, 8 h a month). It also
  says whether close-step publications need FA-INV-12 review. If they do, it states the controller hours needed
  (≥ 10 h a month at 3 books) or the book layout that keeps ρ < 1. This feeds GATE-POST-01 (M3).
- **Alternative or defeater check.** Close-step entries may be reviewed in one sitting as a batch, which would cut
  the count. The card would then have to say so.
- **Resolution.** FA's review rule per summary entry is confirmed. **Materiality.** Medium for M3, low for M0–M2.
  **Uncertainty.** Depends on the review-unit definition.

### P03-B11-QUANT-C12 · §10.4 overstates how slow the monthly Coverage monitor is

- **Type.** deductive.
- **Statement.** "3–5 years to alarm when 80% coverage slips to about 58%" (line 934). The exact first passage of the
  uniform-prior Bernoulli mixture to e ≥ 20 at p = 0.58 gives:
  - median 26 months;
  - P(alarm ≤ 12 months) = 0.27, ≤ 36 months = 0.63, ≤ 60 months = 0.85;
  - 90th percentile 71 months.

  Under the null (p = 0.8), P(any alarm within 240 months) = 0.031 ≤ α, which holds.
- **Plan edit.** Line 934 should read "median about 2 years; about 1 run in 7 takes more than 5 years".
- **Evidence.** `calc_out.txt` (b2). **Materiality.** Low (the pairing decision stands). **Uncertainty.** Exact
  computation.

### P03-B11-QUANT-C13 · Checked nulls: these quantitative commitments hold

- **Type.** observation.
- **Statement.** Recomputed and holding (part b table rows 1, 2, 5–12):
  - GB-03; the GB-08 e-values; GB-35; GB-36 (GiB nit only); GB-37; the GB-38 p = 0.3 row.
  - §34.4's 0.288 and its odds updates. Note: pursuing alternates in parallel lifts P(any site by month 7) to at most
    0.49 if independent, so "M2′ ≈ 0.7" is an upper bound (0.51–0.7).
  - Lumwana's $31.7–57.0M/yr and "1 point ≈ 2.59 kt". "1.7–2.7 points" corresponds to 1.9–3.0% of output; the
    2.4–3.4% figure comes from the tonnage models, a minor mix.
  - GB-26's public case against cells N19/G19; the corporate layer's sign and range.
  - W1's T3 (612/94) and T8 (593/69) tallies, reproduced exactly by an independent implementation. This is a partial
    second read for H-4.

  Statistical gates that are sound:
  - **GATE-TOUCH-02** (n = 200 per bias). P(pass | 0.90) = 0.987 per bias, 0.924 for 6 biases and 0.854 for 12; the
    family-wise false fail costs only a fix cycle. P(kill band | calibrated) < 10⁻³. It catches gross miscalibration
    (true 0.80 passes 4%) but passes a single bias at 0.85 about half the time.
  - **GATE-ROBOT-01:** P(pass | 0.95) = 0.925, P(pass | 0.80) = 0.21.
  - **GATE-WALL-01 and GATE-SYNC-01** (zero failures in 10⁴ and 10³ runs): 95% upper bounds 3×10⁻⁴ and 3×10⁻³.
  - **GATE-BULL-01's 80% kill** is reachable under either reading of C1.

  Fermi checks of performance targets:
  - §29.4's 10⁷ intervals in < 3 s is 300 ns per interval and about 400 MB streamed: feasible.
  - GATE-STORE-01 needs ≥ 11k rows/s into the fsqlite projection (which has an R-tree extension), about 10–100× under
    typical SQLite-class batch rates. Unmeasured on fsqlite.
- **Evidence.** `calc_out.txt`, `gates_out.txt`, `t3_out.txt`, `t8_out.txt`; frankensqlite `crates/fsqlite-ext-rtree`.
- **Materiality.** Keeps correct numbers out of churn. **Uncertainty.** Low.

## NULL OR BLOCKED RESULT

Not blocked. Part (a) is answered in C4 and C5. The plan sizes phases only, not M0, M1 and M2 separately. M0's crates
carry 256k of full budget, and 8.9k (3.5%) exists at M0 start.
