RESULT_STATUS: MATERIAL_INCREMENT

# P04-I4-READERS — Perspective-taking (I4) with compliance reasoning (K6) on PU-4

Pivotal subquestion PU-4: would the actual readers of M0–M2 outputs be able to use and trust them as the plan
specifies them, given their documented needs and the disclosure rules that bind them?

Short answer: mostly yes for site GMs, JV partners, regulators and AI agents at the level of the plan text. There
are four reader-blocking gaps:

1. The one M0 output that exists today would give the CFO, IR and the compensation committee statements that
   contradict Barrick's filed documents, and the plan's gates as written would not catch this (C1–C3).
2. The plan is silent on the Canadian selective-disclosure regime that binds Barrick insiders. That regime governs
   who may review the M0 forecasts, and when (C4–C5).
3. The plan's own public-lane rule becomes undefined once the team joins Barrick's insider list (C6–C7).
4. The M2 deliverable is aimed at a statement in Barrick's continuous disclosure, but counsel review is scheduled
   after M2 (C8–C9).

## WHAT I CHECKED

- **Case and brief.** `planning/mor/r7/PASS_BRIEF_COMMON.md`; `planning/mor/r7/MOR_CASE.md` (Decision Contract,
  E673–E690, PU-4).
- **Plan sections** (`COMPREHENSIVE_PLAN_FOR_THE_DESIGN_OF_FRANKENMINING.md`, sha256 per E682):
  - §0.4 (lines 87–120), §2 BET-04/05 (296–297), §3 (306–324), §5 NS-01/02 (388–394);
  - §10.1 (867–886), §21.6–§21.8 (4408–4592), OPEN-BULL-1/4 (4633–4644);
  - §22 (4646–4787), §23.6–§23.7 (5031–5069), §24 (5190–5442);
  - §30 (6554–6680), §31.1–§31.2 (6703–6790), §34.4–§34.5 (7149–7186);
  - §35 F3/F4, RISK-22 (7199–7200, 7239), §36.2 (7262–7275), §37 (7317–7557);
  - §39 WP-03 beads BD-055…BD-076 and BD-271…BD-275 (7690–7727), BD-111 (7775), BD-200 (7916).
- **Brenner pass B5** (`planning/brenner/B5_wedge_sequencing_premortem.md`): the M0 kill gate (line 388) and F3
  (lines 502–510).
- **Evidence rows:**
  - standing register E023, E025 (`planning/mor/MOR_CASE.md` lines 96, 106, 243);
  - W5 E603–E607 (`planning/research/W5_figure_verification.md` lines 319–327);
  - C1 E107, E110, E113–E115, E137, E138, E146 (`planning/corpus/C1_barrick_public_stats.md` lines 744–783);
  - W4 E551–E599 (`planning/research/W4_m2_site_and_data_access.md` lines 594–642).
- **README.md** (lines 24–26, 34–40, 204, 215–229, 334–336) and **beads** in `.beads/issues.jsonl` (status and
  "Done when" for bd-040…bd-076, bd-111…bd-117, bd-123, bd-142, bd-225, bd-260).
- **M0 output** (E676): `.mor/run-20261029T183653Z/repro/assure_human.txt` (pretty-printed with
  `python3 -I -m json.tool`).
- **Code read:**
  - `crates/fmine-public/src/lib.rs` lines 420–520 (literal North Mara states and
    `ReserveReplacementBridge::compute(3.91, 7.429, 12.277)`);
  - `crates/fmine-metrics/src/lib.rs` lines 137–167 (the doc comment on the bridge);
  - `crates/fmine-io/src/zip_inflate.rs` lines 405–448 (the in-code workbook, cells `L218 = 1638`,
    `N218 = 1484`, `L228 = 3255`, `N228 = 3973`);
  - `crates/fmine-cli/src/lib.rs` lines 32–92 (envelope fields);
  - `grep -rn -E 'reserve_disclosure_claim|no_claim|next_actions|regulatory_compliance_claim' crates` (no match
    outside CONTRACT.md files).
- **New evidence from local filed documents** (read-only, `python3 -I`, own zip/XML reader at
  `/private/tmp/claude-501/-Users-jemanuel-projects-franken-mining/2ba72fbb-c45d-4fd8-93cf-c08cb26d7a18/scratchpad/r7_repro/P04-I4-READERS/xl.py`):
  - `corpus_of_input_docs/downloaded/mine_stats/Barrick_Q4_2025_MD-A.pdf`, converted with
    `pdftotext -layout … q4_2025_mda.txt`:
    - the summary table (pdftotext page 6, extraction lines 352–358) gives FY2025 gold produced 3,255 koz, sold
      3,318 koz and AISC $1,637/oz; FY2024 produced 3,911 koz, sold 3,798 koz and AISC $1,484/oz;
    - the non-GAAP reconciliation (page 47, lines 4060–4067) gives AISC $5,428M (FY2025) and $5,636M (FY2024).
  - `Barrick_Q2_2026_Mine_Stats.xlsx`: `QTD!A19` "North Mara (84%)", `QTD!G19 = 2577` (Q2 2026 AISC),
    `QTD!N19 = 1292` (Q2 2025 AISC).
  - `Barrick_Q2_2024_Mine_Stats.xlsx`: `QTD!G20 = 1491` (Q2 2024 AISC).
  - `Barrick_Q4_2025_AISC.xlsx`: sheets "2025" and "2024" carry site columns C–P only (no Total Gold column).
- **Web primary sources** (accessed 2026-10-29):
  - [17 CFR 243.101(b) (Reg FD "issuer")](https://www.law.cornell.edu/cfr/text/17/243.101): excludes "any
    foreign government or foreign private issuer".
  - [EDGAR filing index, CIK 756894](https://www.sec.gov/cgi-bin/browse-edgar?action=getcompany&CIK=0000756894&type=40-F&dateb=&owner=include&count=10):
    Barrick Mining Corp files Form 40-F annually (latest 2026-02-27).
  - [National Policy 51-201 Disclosure Standards, BCSC copy, amended 31 May 2013](https://www.bcsc.bc.ca/Securities_Law/Policies/Policy5/PDF/51-201__NP__May_31__2013).
    I converted it locally with pdftotext and read s.3.1, s.3.2, s.5.2, s.6.8, s.6.9 and s.6.10.
  - NI 43-101 s.2.1, read in a secondary copy only ([Cassels](https://cassels.com/insights/national-instrument-43-101-who-is-a-qualified-person-and-what-are-their-responsibilities/);
    the [CSA June 2025 draft amendments](https://fcaa.gov.sk.ca/public/CKeditorUpload/Securities/CSA_Consultation_Papers/2025/43_101_et_al/Annex_A_43-101_NI_Amendments_June_12,_2025.pdf)).
  - [SEC Release 33-10570 (S-K 1300)](https://www.govinfo.gov/content/pkg/FR-2018-12-26/html/2018-26337.htm): the
    first 100k characters amend Forms 20-F and 1-A. I did not find an explicit MJDS/40-F statement, so I make no
    claim on it.
- **Not done.** I did not read other passes' raw outputs. I ran no cargo builds and did not re-run the binary;
  E676 bytes were sufficient. I did not open C5.

### Reader trace (summary; detail in the claims)

| Reader (plan §3) | M0–M2 outputs they would receive | Usable as specified? |
|---|---|---|
| CFO / controller | M0 metric reproduction, consolidated AISC bridge, NS-01 waterfall; M2 AISC waterfall (GATE-BULL-03) | **No at M0 as emitted** (C1). The plan's waterfall scope exceeds public data, and "controllable" is undefined (C10). |
| Compensation / remuneration committee (not in §3) | M0 reserve-replacement bases and AISC variants (E025) | **No as emitted** (C1). Not named as a reader (C9). |
| IR / disclosure committee (not in §3) | M0 discrepancy register over IR-host workbooks | **Unspecified** (C9). NP 51-201 s.6.8 permits issuer engagement on public-information inaccuracies. |
| EC / board, sponsor | M0 report and FY2026 forecasts; M1 retrospective cards; M2 Site Assurance card | Historical reproduction yes. **Forecasts not usable two-way before release** (C4). |
| QPs / Head of MRM | M0 cut-off reproduction, GATE-HALL-01; M2 attestation | M0 fits. **M2 attestation lacks routing and counsel timing** (C8). |
| Site GM / metallurgist (M2) | Parity pack, posterior, attribution, waterfall | Fits (null), except the cost-data gap for the waterfall (C10). |
| Counsel | Four flags and `no_claim` on every output | Plan text fits. **The emitted M0 envelope has neither** (C1). |
| JV partners | Attributable public-lane figures; partner bundles | Fits at M0–M2 because Lumwana is 100% owned (null). |
| Regulators | None at M0–M2 | Null. §3 does not list them, and the case framing overstates §3 (C9). |
| AI agents | Robot envelope, Claims, refusals; GATE-ROBOT-01 at M1 | Plan text fits. The emitted envelope lacks Claims and `next_actions` (C1 context; PU-1 territory). |

## CANDIDATE CLAIMS

### P04-I4-READERS-C1

**Type: observation**

**Statement.** The M0 output as emitted at HEAD (E676) would put at least six statements in front of Barrick's CFO,
IR and compensation-committee readers that contradict Barrick's filed documents or the plan's own resolved
evidence. Each is checkable by those readers in minutes.

1. **`reserve_replacement_2024: {organic_pct: 190, headline_pct: 504}`** is the wrong window and the wrong basis.
   Both figures are three-year 2023–25 and both exclude acquisitions and divestments. 190% is gold ounces and 504%
   is gold-equivalent ounces (87 Moz ÷ 17.2 Moz) (E603, E604; plan §21.6 OPEN-BULL-4, lines 4639–4644; BD-063
   "Done when", line 7706). The code produces them from literals back-solved to the targets:
   - 7.429 ÷ 3.91 = 190.0%;
   - (7.429 + 12.277) ÷ 3.91 = 504.0%.

   The doc comment attributes the difference to "greenfield feasibility conversions (Reko Diq +13.0 Moz + Lumwana
   Super Pit +1.7 Moz = 12.28 Moz)". 13.0 + 1.7 = 14.7, not 12.28. As printed, this states
   that the published 504% figure rests on a non-organic basis, which the Circular does not say.
2. **`gold_fy2025.reported_aisc_per_oz: 1638`** comes from an in-code literal (`zip_inflate.rs` line 447,
   `L218`). Barrick's filed FY2025 gold AISC is $1,637/oz (E607; Q4 2025 MD&A, pdftotext page 6 line 358 and
   page 47 line 4067; new evidence).
3. **`gold_fy2025.consolidated_sales_koz: 3255`** equals FY2025 gold *production*. Filed sales were 3,318 koz
   (MD&A line 352–353; the plan states unit costs are per attributable ounce sold, §21.6 line 4444, E110).
   **`gold_fy2024`** uses 5,882 / 3,973. The filed values are $5,636M / 3,798 koz (MD&A lines 4064–4065; new
   evidence). The resulting $3.51/oz gap is labelled `rounding_bridge`, although it is 3.5× the plan's ±1-digit
   tolerance (§21.6 line 4521–4524).
4. **`north_mara_lmdi_2024q1_q2: aisc_q1 1297 → aisc_q2 2582`** has the wrong period and the wrong values. The
   filed +$1,285 pair is Q2 2025 $1,292 → Q2 2026 $2,577 (`Q2_2026:QTD!N19`, `G19`; E137; BD-064 "Done when",
   line 7707), and North Mara's Q2 2024 AISC was $1,491 (`Q2_2024:QTD!G20`; new evidence). The decomposition
   gives only `spend_effect` and `sales_timing_effect`, not the grade, recovery, tonnes, B, S and cost effects
   specified in §21.6 (lines 4465–4472) and E137.
5. **`workbooks_verified: 18`** is reported, although none of the 18 named files exists and an in-code workbook
   is substituted (E677, E678).
6. **The envelope** has no `flags` object, no `certificate`, no `warnings` and no `next_actions`, and no
   `no_claim` line. `grep` finds no `reserve_disclosure_claim` in any crate source, while BD-111 "FMT-ENV-1
   envelope with the four flags false" is closed. This leaves counsel's §3 question ("does it ever make a
   disclosure claim?", line 321) unanswerable from the artifact.

**Scope.** The bytes of E676 at HEAD `68e7091`, checked against the local copies of filed Barrick documents
(W1 retrieval).

**Evidence.** E603, E604, E607, E110, E137, E676–E679, E680. New evidence: Q4 2025 MD&A pdftotext lines 352–358
and 4060–4067; Mine Stats cells above; `grep` result.

**Inference.** Direct comparison of output fields with filed values and labels.

**Assumptions.** The local MD&A and workbooks are the filed versions (W1 states the downloads are byte-identical
where compared; E451, E452).

**Alternative/defeater check.**
- (a) "M0 is not yet shown to readers." True. But BD-063, BD-064 and BD-071 are closed as implemented (E673,
  E680). The repository is public once pushed (E673). The Decision Contract ranks a false statement reaching
  Barrick management as the costliest loss.
- (b) "With the real workbooks present the values would differ." Items 1 and 4 are code literals (E679) and do
  not depend on files. Items 2–3 come from in-code cells, but the real Q4 2025 AISC workbook has no Total Gold
  column at the coded cell addresses (new evidence). The real-file path is therefore unproven, not exculpatory.
- (c) "1,638 might be a Mine Stats figure distinct from the MD&A." The MD&A (filed) is the document the plan
  names as the reconciling source (E107, §21.6 line 4507–4509). The output cites none.

NONE_IDENTIFIED_AFTER_BOUNDED_CHECK for an alternative that makes these labels correct.

**Resolution / reversal.** Reversed if a rerun on the real workbooks emits the filed values and labels with
`<file-period>:<Sheet>!<Cell>` + sha256 + filed-page citations, and if the reserve-replacement bases are rendered
as "3-yr 2023–25, gold oz, ex-A&D, ~190%" and "3-yr 2023–25, GEO at reserve prices, ex-A&D, 504%".

**Materiality.** Highest. It is the Decision Contract's costliest loss. It also defeats the CFO's §3 question
("can I replay every published non-GAAP figure from source?") and the wedge rationale (§37.1, E025) on first
contact.

**Uncertainty basis.** Low for items 1–4 and 6 (direct reads). Medium for item 5's reader impact, which depends on
how the output is presented.

### P04-I4-READERS-C2

**Type: deductive**

**Statement.** The plan's M0 acceptance would not catch C1, because the reader-salient outputs fall outside the
tested denominator.

- GATE-BULL-01 scores site-quarter values only (§21.8 line 4590; §37.3 lines 7397–7402; BD-272).
- The company-level derived outputs that most reach the EC, CFO and compensation committee (consolidated AISC,
  reserve-replacement bases, the waterfall) are accepted only by bead-level "known answer" or "bridge reproduced"
  tests (BD-063, BD-064, BD-273). Those tests check numbers, not period, basis and denominator labels, and not the
  filed page.
- G-BULL-6 ("a metric rendered without `rules_version` and basis → refuse", line 4576) is not attached to any M0
  gate. Nor is PO-ADIT-1 (four flags in every envelope).
- BD-072 (the `no_claim` line) is open while BD-071 and BD-111 are closed.

**Scope.** Plan §21.8, §30.6 and §37.3 M0 acceptance; §39 WP-03.

**Evidence.** Plan lines cited; bead statuses (E680; `.beads/issues.jsonl`).

**Inference.** A gate whose denominator excludes an output cannot fail on that output.

**Assumptions.** None beyond the plan text.

**Alternative/defeater check.** GATE-HALL-01 (M0–M1, line 5405) covers the published reserve roll-forward. That
partly covers reserve replacement, but it tests the roll-forward and discrepancy items, not the rendering of the
190%/504% labels. "Zero uncited numbers" would fail C1. But that criterion is evaluated at the gate, and BD-063
and BD-064 were closed before it ran. NONE_IDENTIFIED_AFTER_BOUNDED_CHECK for an existing gate that tests derived
labels.

**Resolution / reversal.** Reversed if a plan section already requires each company-level M0 figure to match the
filed document's value **and label** with a filed-page citation. I found none.

**Materiality.** High: it is the mechanism by which C1 can recur after a fix.

**Uncertainty basis.** Low.

### P04-I4-READERS-C3

**Type: recommendation**

**Supporting claims.** C1, C2.

**Statement.** Add a reader-facing check to §21.8 (GATE-BULL-01 row) and §37.3 M0 acceptance. The corrected text
must assert:

- Every company-level or derived figure in the M0 report must do the following. This covers consolidated AISC per
  metal, each reserve-replacement basis, GEO, each waterfall bar, and every "reported" value.
  - Cite the filed document and page that states it (MD&A or Circular page). E107 already gives the pattern for
    site cells.
  - Reproduce that document's **label**: period or window, basis, metal coverage, denominator (sold or produced),
    and ownership basis.
  - Carry `rules_version` (G-BULL-6).
- A value that matches but whose label differs counts as a failure, not a reproduction.
- The envelope carries the four flags (PO-ADIT-1), and the report carries the `no_claim` line.
- Before any M0 output is shown to an issuer reader, a person who did not build it checks every company-level
  figure against the filed page, and the check is recorded in CELL-DEC (a sibling of BD-075).

**Cost.** Small. It adds one scorer category to BD-272, plus one review pass.

**Reversibility.** Fully reversible plan text.

**Acceptance test.** Running `fmine assure public --issuer barrick` on the real workbooks yields a report in
which:
- the reserve-replacement row reads three-year 2023–25 for both bases, with gold oz versus GEO at reserve prices;
- FY2025 gold AISC reads $1,637 with the MD&A page cited;
- the North Mara known answer is labelled Q2 2025 → Q2 2026, with the six E137 effects;
- a scripted mutation of any label (for example "2024" for "2023–25") makes the scorer fail.

**Materiality.** High. **Uncertainty.** Low.

### P04-I4-READERS-C4

**Type: deductive** (compliance)

**Statement.** The plan gives the M0 forecasts and the M0 kill review to Barrick insiders without saying when,
and the rules that bind those insiders make the specified use partly unavailable.

1. **The governing rule.** Barrick files Form 40-F (EDGAR; new evidence) and 6-Ks (E603). It is a foreign private
   issuer, and Regulation FD's definition of "issuer" excludes foreign private issuers (17 CFR 243.101(b)). The
   binding standard for Barrick's officers is therefore the Canadian tipping prohibition, with its guidance in
   NP 51-201:
   - s.3.1 prohibits informing anyone of an undisclosed material fact;
   - s.5.2 warns that a company "takes on a high degree of risk" if it confirms an analyst's estimate is "on
     target", "too high" or "too low";
   - s.6.8 says there is "a serious risk of violating the tipping prohibition if you express comfort with or
     provide guidance on an analyst's report, earnings model or earnings estimates", and limits review to
     "pointing out inaccuracies or omissions with reference to publicly available information";
   - s.6.9 recommends a quiet period "between the end of the quarter and the release".
2. **The timing.** M0 runs Nov 2026–Jan 2027 (§37.3 line 7378). Its artifact includes FY2026 per-site forecasts
   (line 7393). Its kill criterion is "two finance reviewers rate it 'nothing new'" (line 7403; BD-075). FY2026
   ends 31 Dec 2026, and the release is in Feb 2027 (E685). A Barrick finance reviewer at the M0 gate would
   therefore be inside the FY2026 quiet period, reviewing an outside model of results they already know.
3. **What the plan dropped from B5.** B5 named the reviewers as "Barrick or an ex-major advisor" (B5 line 388),
   and F3 required "counsel reviews any external use of public-lane forecasts (default: share with the sponsor
   only)" (B5 lines 507–509). The plan dropped both (§21.8 line 4590; §35.1 F3 line 7199; §31.1 lines
   6719–6725).
4. **The consequence.** Under NP 51-201 s.6.8, a Barrick reviewer can legitimately review the historical
   reproduction and the discrepancy register, because both reference public information. They cannot use or
   comment on the forecast set before release. The plan neither says this nor fixes the reviewer pool, so the M0
   kill criterion's meaning depends on an unstated choice.

**Scope.** M0 and the M0→M1 boundary (forecast hashing, BD-068; scoring, BD-076).

**Evidence.** NP 51-201 ss.3.1, 3.2, 5.2, 6.8, 6.9 (BCSC copy, accessed 2026-10-29); 17 CFR 243.101(b); EDGAR
40-F index; plan lines cited; B5 lines 388 and 507–509; E603, E685.

**Inference.** The rule applies to Barrick's insiders as recipients and commenters. It does not apply to
FrankenMining as author while FrankenMining has no special relationship (but see C6).

**Assumptions.**
- The forecasts are material enough to Barrick that comment on them would engage s.5.2. Per-site full-year
  production and AISC for a senior producer plausibly are, but materiality is the issuer's judgment.
- Barrick's own quiet-period dates follow NP 51-201 s.6.9. Its policy may differ; s.6.9 notes periods "vary by
  company".

**Alternative/defeater check.**
- (a) If both reviewers are ex-major advisors with no Barrick relationship, NP 51-201 does not bind them, and C4
  reduces to the sponsor-sharing question.
- (b) Showing a public-data model to an insider is not itself prohibited. The constraint is on the insider's
  response, so the claim is about usability, not illegality.
- (c) Scoring after release (BD-076, "Scores published to the sponsor") is unaffected.

These narrow the claim. They do not remove it.

**Resolution / reversal.** Reversed if the plan, or a counsel note, already fixes the reviewer pool and the
pre-release handling of public-lane forecasts. I found none in the plan, `planning/` (outside r7), README or
AGENTS (grep for "quiet period", "51-201", "Reg FD", "tipping", "selective disclosure": no hits).

**Materiality.** High. The plan rates F3 (MNPI breach or lane contamination) as "fatal impact" (line 7199), and
the M0 kill criterion is a program gate input.

**Uncertainty basis.** Medium. The rule texts are verified; their application depends on materiality and
Barrick's policy dates.

### P04-I4-READERS-C5

**Type: recommendation**

**Supporting claims.** C4.

**Statement.** Edit §21.8 (GATE-BULL-01 kill), §37.3 M0 *Kill*, §31.1 (public lane) and BD-075/BD-068/BD-069 so
that they assert:

1. **The reviewer pool is named.** For example, "one issuer finance reviewer and one independent ex-major
   reviewer". An issuer reviewer receives only the historical reproduction and the discrepancy register before the
   release (NP 51-201 s.6.8 scope). The forecast set is shown to issuer insiders only after the release, as scored
   output (BD-076).
2. **The forecast hash is registered early.** It is registered before the first presentation of any M0 content to
   an issuer insider, and in any case before quarter end (31 Dec 2026). The registration carries a signed author
   attestation of no non-public issuer information, extending §10.1's `author` field. A later revision is a new
   registration flagged "post-contact".
3. **B5's line is restored.** "Counsel reviews any external use of public-lane forecasts; default: share with the
   sponsor only, after release."
4. **The plan names the regime.** For Barrick (a 40-F filer), NP 51-201 and the provincial tipping provisions
   apply, not Regulation FD.

**Cost.** Low. It is text plus one attestation field.

**Reversibility.** Full.

**Acceptance test.**
- BD-075's CELL-DEC record lists the reviewers' affiliations and the artifact subset each received.
- BD-068's timestamp precedes both 31 Dec 2026 and the first logged issuer presentation.
- BD-069's attestation is present in the registration record.

**Materiality.** High. **Uncertainty.** Medium (see C4).

### P04-I4-READERS-C6

**Type: deductive**

**Statement.** The plan's public-lane rule becomes undefined, and in one branch contradicted, once GATE-DATA-01
passes.

- §31.1 and §4.3 say "No public-lane author holds private access" (lines 376, 6723).
- F3's countermeasure puts "insider list and blackout for the whole team" in place (line 7199). GATE-DATA-01
  requires that list by month 4 (line 7156; BD-200).
- Under NP 51-201 s.3.2(c), persons "engaging in professional or business activities for or on behalf of the
  company" are in a special relationship.

Consequences:

1. From GATE-DATA-01, the owner and the agent team can no longer author Barrick public-lane outputs without
   breaching §31.1 as written. The plan does not say what happens to the public lane then.
2. The plan does not define "private access": an insider-list entry, receipt of any non-public information, or
   data access.
3. In the branch where GATE-DATA-01 passes but GATE-DATA-03 fails, M2′ runs. M2′ is "parent-level metric and
   reserve assurance on public data" (lines 7158, 7486–7487) and is the plan's modal path (≈ 0.7, line 7176). The
   public lane would then be authored by insider-listed people. Using E599's factors (0.6 × 0.8 that the agreement
   is in force; 0.4 that data then fails by month 7), this branch is ≈ 0.19.
4. README's "Not affiliated … not built for Barrick" (lines 36–39) becomes inaccurate under a signed data
   agreement.

**Scope.** Month 4 onward, including M2′.

**Evidence.** Plan lines cited; NP 51-201 s.3.2(c); E599; README lines 36–39.

**Inference.** Composition of the plan's own rules with the binding definition of special relationship.

**Assumptions.** The data agreement makes the team persons acting for Barrick (the usual case for a vendor under
NDA).

**Alternative/defeater check.** If "private access" is read narrowly as possession of site data, the M2′ branch
is not a breach. But the plan does not say so, and the agreement negotiation itself can convey non-public
information. Public-lane *reproduction* of already-published numbers carries little tipping risk. The plan's rule
is nonetheless written for all public-lane authorship, not only forecasts. NONE_IDENTIFIED_AFTER_BOUNDED_CHECK
for plan text that defines the transition.

**Resolution / reversal.** Reversed if §31.1 or §34.4 defines "private access" and states the lane's status after
GATE-DATA-01.

**Materiality.** Medium–high. The M2′ branch is the modal path, and the plan rates F3's impact "fatal".

**Uncertainty basis.** Medium. The branch probability rests on E599's elicited factors.

### P04-I4-READERS-C7

**Type: recommendation**

**Supporting claims.** C6.

**Statement.** In §31.1, §34.4 (GATE-DATA-01 row) and §37.3 (M2′), assert:

- **The definition.** "Private access" means any of: being on an issuer insider list, being party to a data
  agreement, or having received non-public issuer information.
- **The transition.** On GATE-DATA-01, the issuer's public lane is frozen at its last registered state.
  Thereafter it is either retired for that issuer or authored only by a walled author set with signed
  no-access attestations.
- **M2′ scope.** M2′'s public-lane content is limited to reproduction of already-published values plus the
  pre-registered FY2026 scoring, unless counsel clears more.
- **README.** The "Not affiliated" statement is updated at GATE-DATA-01.
- **BET-02.** State where its "four scored quarters" (line 294) come from if the public series stops: the
  in-tenancy M2 forward quarters and FY2026.

**Cost.** Low (text). It may reduce the public track record available at M3.

**Reversibility.** Full.

**Acceptance test.** BD-200's completion record includes the lane-state change. BD-069's attestation list
contains no insider-listed author for any public-lane registration after that date.

**Materiality.** Medium–high. **Uncertainty.** Medium.

### P04-I4-READERS-C8

**Type: deductive** (compliance; reader = QP, metallurgist, counsel, disclosure controls)

**Statement.** The M2 deliverable is aimed at a statement in Barrick's continuous disclosure, but the plan has no
routing for an attested finding on it, and its counsel review is scheduled after M2.

- **What M2 scores.** The pre-registered M2 recovery card is "scored against the feasibility study's 92.7%
  fresh-ore recovery" (§37.3 line 7436–7437). That figure is disclosed in Barrick's AIF as the Super Pit
  life-of-mine basis (E570, AIF p.102). The AIF also states that "no material difference is expected in recovery …
  between currently processed mineralization and expected mineralization" (E571).
- **What M2 requires.** Acceptance item 2 requires a finding attested `Reviewed{against}` by the site QP or
  metallurgist as "new and plausible", with "material" pre-registered at ≥ 1% of contained metal (lines
  7479–7481).
- **The binding rule.** NI 43-101 s.2.1 requires issuer disclosure of scientific or technical information on a
  material property to be based on information prepared by, or approved by, a QP (secondary copy, accessed
  2026-10-29).
- **What follows.** An attested internal record that a disclosed recovery basis or statement may not hold is
  something the QP who supports Lumwana's disclosure and Barrick's disclosure controls must at least evaluate.
- **The plan's gaps.**
  - The plan has no step routing such a finding to the disclosure QP and the disclosure committee.
  - It schedules counsel's QP-liability review (OPEN-HALL-4) "before M3" (line 7273), after M2's attestation.
    Only the insider-list format (OPEN-B4-07) is due "before M2" (line 7272).
  - The plan's "material finding" wording borrows the securities-law term without saying it is not a disclosure
    materiality determination. This bears on counsel's §3 question about discoverable records (line 321) and on
    F4 (line 7200).

**Scope.** M2 at Lumwana, and the alternates if they inherit the same acceptance.

**Evidence.** E569–E571, E566–E567; plan lines cited; NI 43-101 s.2.1 (secondary).

**Inference.** The plan deliberately aims a scored card at a disclosed assumption. The attestation therefore
creates exactly the kind of record whose handling the binding rules govern.

**Assumptions.** Lumwana is a material property for Barrick. INFERENCE only: the AIF carries a property-level
description at pp.101–102 (E551, E569–E571), which is consistent with, but does not prove, material-property
status.

**Alternative/defeater check.**
- (a) E570's 92.7% is for Super Pit fresh ore, while the current circuit treats fresh plus transitional ore at a
  "reasonable" 91.15% (E569). A current-circuit shortfall may therefore not contradict the AIF. This narrows the
  claim but does not remove it, because the plan itself scores the card against 92.7%.
- (b) Likely materiality at Barrick scale: E567 gives $30–57M/yr, small against group revenue. The issuer may
  judge the finding immaterial. The claim is about routing and timing, not a violation.
- (c) The attestor may be the metallurgist rather than the QP. That lowers QP-liability exposure but not the
  routing need.

**Resolution / reversal.** Reversed if the plan or GATE-DATA-01's agreement already specifies routing of findings
that bear on disclosed statements, and counsel review before the first attestation. I found neither.

**Materiality.** Medium. It does not block M2 technically, but it can make the QP or metallurgist decline to
attest (F4), and attestation is M2's acceptance.

**Uncertainty basis.** Medium. NI 43-101 s.2.1 was read in a secondary copy, and material-property status is
inferred.

### P04-I4-READERS-C9

**Type: observation + recommendation**

**Supporting claims.** C1, C4, C8.

**Statement.** §3 (lines 306–324) omits three readers who consume M0–M2 outputs or are their stated beneficiaries.

1. **IR / disclosure committee.** The M0 discrepancy register audits workbooks published on Barrick's IR host
   (E451; §21.6 line 4501). NP 51-201 s.6.8 shows the issuer may legitimately engage on "inaccuracies or
   omissions with reference to publicly available information".
2. **Compensation / remuneration committee.** The wedge is justified by measurement integrity behind the
   published scorecard metrics (§37.1 lines 7336–7337; E025, E605).
3. **Regulators.** The case file says §3 lists IR and regulators (`MOR_CASE.md` line 48). It lists neither. No
   M0–M2 output is addressed to a regulator, so for them this is a framing correction, not a gap.

**Recommendation.** Add §3 rows:

- **IR / disclosure committee.** Question: "Which of our published figures does an outside system find
  inconsistent, with what citation?" Answered by the §37.3 discrepancy register. Items are worded as
  "unexplained difference between cited cells" and reach the issuer privately before any public push. The
  private-first preference is HYPOTHESIS: I found no document stating Barrick IR's preference. The basis is the
  Decision Contract's loss function plus E673 (push = publish).
- **Compensation committee.** Question: "Is each scorecard figure reproducible, on which basis?" The M0
  report shows the scorecard figure beside the reported figure, for example gold AISC $1,423 (API actual) beside
  $1,637 (non-GAAP) (E605, E607). The scorecard figure carries a typed refusal "basis not located (OPEN-BULL-1)"
  rather than being omitted, and OPEN-BULL-1 gets an owner and a deadline in §36.2.

**Cost.** Low. **Reversibility.** Full.

**Acceptance test.** §3 has the two rows. The M0 report has a scorecard-metric section listing each E605 metric with a
reproduction state (reproduced, explained, or refused with code).

**Evidence.** Plan lines cited; E025, E451, E605, E607; NP 51-201 s.6.8; MOR_CASE line 48.

**Alternative/defeater check.** The audit committee row (line 320) partly covers the disclosure function. It asks
about reproducibility and attestation, not about issuer-facing handling of discrepancies.

**Materiality.** Medium. **Uncertainty.** Low for the omission. HYPOTHESIS for the IR preference.

### P04-I4-READERS-C10

**Type: observation + recommendation** (reader = CFO, site GM)

**Statement.** The CFO's waterfall question cannot be answered at M0 as NS-01 promises, and M2's minimum data
set would not support it either.

- **M0 scope.** NS-01 is tagged [M0/M2] and promises bars for "price-linked royalty, fuel, FX, sustaining capex"
  (lines 388–391). The public lane has no per-site fuel, FX, G&A or by-product split. The plan itself says
  TCC → AISC is only a residual, and the component split "stays OPEN" (§21.6 lines 4525–4528). §37.3's M0
  artifact list does not include a waterfall at all (lines 7379–7396).
- **"Controllable."** The CFO's §3 question asks "which parts are controllable" (line 311). The word appears
  nowhere else in the plan's metric design (the only other hit, line 5039, is about rTSR), so no output answers
  it.
- **M2 data.** M2's AISC waterfall (GATE-BULL-03, line 4592; §37.3 line 7473) needs cost-centre extracts.
  GATE-DATA-03 lists them (line 7158), but §37.3's "recovery-capable" minimum set omits them (lines 7446–7448).
  Lumwana is a copper mine, so the waterfall is per pound (C1 or AISC per lb), not per ounce.

**Recommendation.**
- Restate NS-01's M0 scope as tonnes, grade, recovery, B and S plus the published cost lines (CoS, royalties/oz,
  depreciation/oz, sustaining capex) and a named "other" residual. Fuel and FX bars arrive at M2 only.
- Either define controllability as named layers (price-linked, FX, ownership and ratio-basis layers as
  non-controllable) or drop the word from §3.
- State in §37.3 M2 that GATE-BULL-03 requires the GATE-DATA-03 cost-centre extracts, at $/lb for copper.

**Cost.** Low. **Reversibility.** Full.

**Acceptance test.**
- NS-01 text matches §37.3.
- The M0 report's North Mara known answer shows the six E137 effects summing to +$1,285.
- The M2 data request lists the cost extracts.

**Evidence.** Plan lines cited; E137; W4 E551–E573 (Lumwana copper).

**Alternative/defeater check.** M2 could deliver the waterfall from published site cost lines only. That would
not be "from tonnes, grade and recovery to unit cost reconciled to the site's reported figures" with cost drivers
(GATE-BULL-03). NONE_IDENTIFIED_AFTER_BOUNDED_CHECK.

**Materiality.** Medium: the gap is in the CFO's headline scenario. **Uncertainty.** Low.

## NULL OR BLOCKED RESULT (checked nulls)

- **JV partners.** At M0, partner-relevant figures are attributable public values with typed ownership (§21.6;
  §24.3 OwnershipRef). The plan already catches the Porgera 47.5% label on 24.5% values (E468, E469). M2 is at a
  100%-owned site (E551, E552) chosen to avoid partner consent (F13). The alternates name the consent instruments
  (Twiga paper, AngloGold consent). FA `.fab` bundles are unavailable in M0–M2 (§37.4), but §31.2's
  `fmine-verify` bundles cover verification. No gap found.
- **Regulators.** No M0–M2 output is addressed to a regulator. Regulator-facing renders (§24.6) start at M3, and
  M2's residency handling (Zambia DPA s.70, E514) is specified. No gap beyond C9's framing correction.
- **QPs at M0.** Cut-off reproduction (BD-274) and GATE-HALL-01 consume published technical-report and
  reserve-table content. G-HALL-1 forbids class labels, and nothing in M0 speaks for a QP. This fits the QP's
  documented question in §3 (line 314).
- **EC at M1.** Retrospective cards built as known at the decision date on public evidence (GATE-HELM-01) touch
  no disclosure rule I could identify.
- **AI agents (plan text).** §30.1 and §30.4 and GATE-ROBOT-01 (M1) fit the agent's §3 question. The emitted
  envelope does not (C1 item 6). That is a code-versus-plan gap, left to PU-1/PU-2.
- **Site GM / metallurgist at M2.** Parity-first acceptance, pack-time reduction or co-sign, and a pre-registered
  recovery attribution fit the GM's §3 question for a copper site. The only gap found is the cost-data one (C10).
