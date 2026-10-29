# MOR_CASE — FrankenMining first-pass design (neutral case file)

RUN_ID: fm-2026-10-28-a. This is the case file for the Brenner/MOR hypothesis passes. It holds facts and open questions, not the lead agent's thesis.

## Decision contract

- **Primary question:** For FrankenMining, which architecture, epistemic model, sequencing and integration choices maximize decision value for a Tier-One integrated gold/copper miner? The system covers exploration → resource → plan → operate → process → tailings → account → allocate capital, and it must be built on the asupersync/frankensuite and frankensim.
- **Decision owner:** the project owner (the user). Barrick top management is the downstream audience. QPs/CPs own reserve classification. Controllers own GAAP judgment.
- **Horizon:** the design is for a 5–10 year program. The first credible demonstrator is needed within about 12 months.
- **Loss function, most costly errors first:**
  1. a false-certainty number reaching a reserve disclosure, a capital decision or a dam;
  2. an architecture that cannot scale to 10⁸–10⁹ blocks or 10⁵ drillholes;
  3. a wedge that management cannot see value in;
  4. duplicating suite substrate or forking upstream;
  5. missed upside from Bayesian regional reasoning.
- **Constraints:**
  - Rust; asupersync `Cx` everywhere; forbid unsafe.
  - Closed dependency universe; foreign tools are lab oracles only.
  - Consume, never fork, sibling repos.
  - Software never signs reserves or geotechnical sign-offs.

## Evidence (stable IDs; sources are planning/research/R1–R7)

### frankensim (R1)

- **E001** frankensim is unconverged. `cargo check` fails on 15 targets; it uses nightly-2026-07-06 and `generic_const_exprs` in fs-qty. Only `thermal.conduction-solve` is at L3.
- **E002** frankensim has no domain-extension protocol. FrankenTextiles consumes it by lifting files with `//! Provenance:` headers and golden-hash tests. A git dependency is likely broken by `../../../asupersync` path deps.
- **E003** Liftable crates that do not depend on asupersync: fs-qty, fs-math, fs-evidence, fs-ivl, fs-matdb, fs-thermochem, fs-tribo, fs-rand, fs-voi, fs-robust, fs-regime, fs-tropical, fs-eproc, fs-sparse, fs-package, fs-vvreg, fs-contract, fs-govern, fs-casebook, fs-mbd.
- **E004** All geometry is `Cx`-coupled: `fs_geom::Chart::eval(&self, Point3, &Cx)`.
- **E005** Geometry and representation crates:
  - fs-rep-sdf `VdbGrid<T>` (the 10⁹ target is unbenchmarked); fs-rep-voxel (MaterialField, PointCloud);
  - fs-rep-mesh (winding_exact, dual_contour, mesh_to_sdf); fs-rep-frep (CSG);
  - fs-mesh (delaunay, conservative_cell_remap); fs-geocon (volume_certified, min_thickness);
  - fs-asbuilt (registration from correspondences only; no ICP).
- **E006** Uncertainty and design crates:
  - fs-uq `KlExpansion` (dense, unconditional); fs-assimilate (EnSRF; dense Belief capped at 256);
  - fs-voi; fs-oed-e2e `run_campaign` (closest analogue to drillhole placement);
  - fs-bo (Matern GP); fs-robust (cvar); fs-dfo (CMA-ES, NSGA-III, dro);
  - fs-eproc (e-processes); fs-tropical (critical path).
- **E007** Physics crates:
  - fs-airflow (ventilation networks); fs-conduction (operator twin of confined Darcy seepage);
  - fs-lbm (no Bingham/Herschel-Bulkley); fs-solid (J2 only, no Mohr-Coulomb/Hoek-Brown); fs-tribo `ArchardLaw`;
  - fs-convection `CorrelationCard` pattern; fs-vvreg benchmark registry.
- **E008** fs-evidence: `Evidence<T>`, unforgeable `Certified<T>`, `ValidityDomain`, `ModelCard`, `Color::{Verified,Validated,Estimated}` with no laundering. Typed refusals: `NoClaimInDomain`, `Violation{code,what,fix}`.
- **E009** The matdb pipeline seals corpora; the pack schema is frozen at v7. The 183 bundles and 1,012 claims contain no rock, ore, slurry, tailings, liner or Bond Wi data. A sealed private corpus is possible.
- **E010** Gaps in frankensim: geostatistics; Mohr-Coulomb/Hoek-Brown/LEM/SSR; pit and stope optimization; drillholes; Richards/Biot; DEM; ICP; CRS/EPSG; DXF/OMF/LAS/E57/GeoTIFF; mining units and money; integer scheduling.

### Runtime and substrate (R2)

- **E011** The suite pins asupersync 0.5.0; main is 0.6.0. `[patch.crates-io]` is needed to avoid two `Cx` types.
- **E012** asupersync primitives:
  - `Cx`, `Scope`, `CapSet`, `Outcome{Ok,Err,Cancelled,Panicked}`, request→drain→finalize;
  - Budget, Obligations, `LabRuntime`, Spork supervisors, ATP;
  - franken-evidence `EvidenceLedger`, franken-decision `DecisionContract`/`LossMatrix`/`FallbackPolicy`.
- **E013** frankengraphdb is unreleased (~12k tests).
  - Live: time travel `FOR SYSTEM_TIME AS OF SEQ`, `execute_certified`, `CALL fnx.*`.
  - Not live: valid time, branches, MVs. There is no spatial index, and one was rejected for 1.0.
- **E014** frankensqlite v0.4.12 is mature, with R-tree, geopoly and FTS5.
- **E015** The suite has 35 idioms. They include: closed dependency universe; foreign tools as lab oracles only; canonical ledger plus derived projections; recorded vs valid time; robot-first single artifact; doctor plan/apply; evidence labels FACT/DESIGN/HYPOTHESIS/TARGET/OPEN; claim classes; stable ID prefixes; registries as data; certified replayable results; DecisionCards; kill gates; subset-never-substitute; consume-never-fork.
- **E016** Storage by shape of authority:
  - fgdb for temporal evidence graphs;
  - an owned delta ledger with an adapter ladder for high-rate telemetry;
  - content-addressed object graphs for block models, point clouds and shells;
  - fsqlite R-tree or an owned index for spatial data.

### Numerics (R3)

- **E017** frankenscipy fsci-opt linprog/milp handles dense, small-to-medium problems only.
  - fsci-sparse max-flow wraps at int32. fsci-spatial has KDTree and Delaunay. fsci-stats has 95+ distributions, CensoredData, KDE and Sobol/LHS.
  - franken_networkx uses string nodes and is not for million-block pits.
- **E018** No suite repo contains: kriging, variograms, MCMC/HMC, pseudoflow, GeoTIFF, EPSG, LAS, ICP, SfM, RF/GBT, NPV/IRR.
  - frankenjax has autodiff but no samplers.
  - Born-digital PDF extraction is missing; franken_ocr handles scanned documents only.

### Accounting (R4)

- **E019** franken_accounting (FA) is the book of record and mechanical layer only. Judgment stays outside it.
  - Single writer per book; 118→439 ms per post in a debug build; CloseRun not started; perf unproven; feature freeze.
  - No metal or mass units. FA-INV-12 (external subledger summary) is dormant.
- **E020** Integration posture:
  - typed intents via the `fa` robot CLI and `fad`, or fa-mcp;
  - thin Canon rules for account determination only;
  - summary entries carrying the detail Workpaper digest;
  - `ImportReceipt Connector{franken_mining/<module>}`;
  - FrankenMining-generated evidence is self-sourced, so it needs third-party docs or a human `Reviewed` attestation.
- **E021** FrankenMining must build a 17-part mining domain pack:
  - metal accounting subledger; production costing; IFRIC 20 stripping; UoP DD&A; E&E; ARO; impairment;
  - concentrate provisional pricing; streams and royalties; hedging; mining taxes; JV; capital projects;
  - AISC/AIC/C1; reserves register; chart/Canon/statements; close orchestrator and controls.

### Textiles patterns (R5)

- **E022** Four claims are never conflated. For mining these are Parity / Known-answers / Reconciliation / Performance.
  - Oracles are pinned and never regenerated from the candidate. One authority per number. Refusals, not clamps.
  - Textiles is 2.7× over its LOC ceiling, so a budget protocol is needed from day one. Its delivery log ballooned the plan (§25 grew to 82%).
- **E023** Flags software never sets true: `reserve_disclosure_claim`, `execution_authorized`, `geotechnical_signoff`, `regulatory_compliance_claim`.
  - Authorization refusals: unknown reactive ground → no ANFO; unknown PAG → no NAF dumping; unknown moisture vs TML → no shipping.
  - Drill results are material non-public information (MNPI).

### Industry (R6)

- **E024** Barrick Mining Corp:
  - CEO Mark Hill (permanent from February 2026); the North American Barrick IPO is targeted for end-2026.
  - Avathon is the strategic tech partner ("Computational Knowledge Graph" plus agents) and its lead director is Barrick's Chair.
  - Komatsu FrontRunner AHS runs at NGM; Barrick uses SAP S/4HANA and Pitram.
- **E025** Executive pay metrics: safety (including critical-control verifications), GEOs, AISC, relative TSR vs GDX, ROCE, reserve replacement, ESG. Published reserve-replacement and AISC figures are internally inconsistent across disclosures.
- **E026** Industry economics:
  - Cost per discovery is $218M (2011–20); discovery to production takes ~16–18 years.
  - Schedule compliance can be as low as 30% (EY). GISTM has 77 requirements.
  - Bertisen & Davis capex overrun +14%; Bond et al. 21%.
- **E027** OEMs are consolidating mining software (Cat-RPM, Weir-Micromine, Sandvik-Deswik, Komatsu-Modular, Hitachi-Wenco, IMDEX-Datarock, Maptek). OMF v2 has an MIT Rust library, `omf-rust`.

### Technical methods (R7)

- **E028** Prospectivity state of the art:
  - LGCP/SPDE intensity on mineral-system covariates; marks (tonnage, grade); discovery thinning by effort and detectability; negative evidence enters only via thinning; four absence classes;
  - Bayesian search theory; USGS three-part/MapMark4 as a degenerate parity mode; terrane partial pooling (pooled and local differ in 14/16 assessments);
  - validation by blocked/LOCO/temporal hindcast, PR-AUC, conformal and AOA.
- **E029** Geostatistics and reconciliation:
  - Kriging conditional bias; local-uncertainty-correction (LUC) slope ≈0.1; SGS/TB/MPS/SPDE simulation.
  - Reconciliation is joint state estimation over the material-flow graph. F-factors serve as hierarchical bias priors.
- **E030** Planning methods:
  - pseudoflow (MineFlow parity oracle), MineLib regression set, Lane cut-off, Bienstock-Zuckerberg (BZ);
  - COSMO stochastic, value of the stochastic solution.
- **E031** Reporting codes:
  - JORC 2012 is in force. Provisional JORC 2026 adds mandatory reconciliation and Risk = effect of uncertainty.
  - NI 43-101 replacement proposed (June 2025); S-K 1300.
  - CP/QP classification must not be automated.
- **E032** Epistemics:
  - KK (revocable); KU-a/KU-p/KU-s with calibration flags; UU (budget, detect, hedge, convert); unknown knowns.
  - Evidence classes: measured/derived/modelled-calibrated/modelled-uncalibrated/elicited/transferred/assumed/attested.
  - Forecast & calibration ledger; surprise monitor; synthetic-truth mine.

### FrankenSurveillance pattern (read directly)

- **E033** Three planes (authority, cognition, effect) plus a packet plane.
  - Knowledge states: known/estimated/unknown/conflicted/stale/not_observable/redacted/indeterminate.
  - Negative evidence requires observability preconditions.
  - SituationCapsule/WorldEnvelope; InvestigationCase with VOI; plan/commit; hydration H0–H4.

## Pivotal questions (assigned to passes)

- **PQ-1** Canonical subsurface representation, and how frankensim geometry participates.
- **PQ-2** Regional Bayesian prospectivity method, and how negative evidence and analog transfer enter.
- **PQ-3** Planning and optimization under geological, price and execution uncertainty, plus solver build vs boundary.
- **PQ-4** Storage authority, runtime region tree, and the frankensim, FA and fgdb consumption postures.
- **PQ-5** Native representation of KK/KU/UU, with probabilities that stay honest and calibratable.
- **PQ-6** Wedge and sequencing: what proves value to a Tier-One miner's top management first, and why this fails.
- **PQ-7** Executive capital allocation (new plays, junior equity, expansions) as one comparable decision object.

## Unknowns (recorded, not resolved here)

- Real Barrick data access, formats and confidentiality regime.
- Whether frankensim converges on the program's timeline.
- FA performance at real book scale.
- Whether a pure-Rust LP/MILP of adequate scale is attainable in the program's timeline.

## Additions after the audit rounds and the corpus pass (2026-10-28)

- **E034** Newmont served Barrick a notice of default on 3 Feb 2026, alleging NGM mismanagement and diversion of
  resources to the wholly owned Fourmile; it was settled on 10 Aug 2026 (B5 W4; FACT V-S, press sources). NGM
  operating data is JV-sensitive.
  - Corpus corroboration: Barrick "anticipates Fourmile will be incorporated into the Nevada Gold Mines joint
    venture, at fair market value, if certain criteria are met" (E139). Fourmile valuations are therefore
    transfer-price inputs and belong in a walled lane (plan §11.7, §37.3).
- **E035** The proofs the industry believes are single-asset operational uplifts, not assurance: Freeport "TROI" at
  Bagdad, about +10% throughput and about +1 point copper recovery with no new capital (B5 W8; consultant-reported).
  Twins stall on people (Lihir, B5 W9).
- **E031 update.** JORC 2012 remains in force (Oct 2026); the provisional JORC Code 2026 is still being finalized
  (R7 §4.2), so the plan says "exposure draft" or "provisional", never "in force". The NI 43-101 replacement
  proposal adds QP data-verification disclosure per technical-report item (B5 W7).
  - Corpus: Pueblo Viejo's 2026 report already publishes F1A/F1B/MCF tables, a rated risk table, a GISTM class and
    an IAS 37 closure split, while the 2012/2018 RPA reports (Porgera, Veladero, Zaldívar) publish none of them
    (C4 P11). Disclosure depth is a per-report attribute, not a per-code one.
- **E029 correction (append-only; the original row stands as written).** LUC is *Localised Uniform Conditioning*
  (Abzalov 2006; E352), not "local-uncertainty-correction". The slope ≈ 0.1 figure traces to Assibey-Bonsu &
  Muller (2014) through R7 and is FACT[U] until its page is pinned. The corpus supplies the mechanism (b ≈ ρ for
  any variance-restoring estimator; E355) and the originator's validity conditions (E353).

### Corpus evidence blocks (`planning/corpus/`)

Each block was produced by one read-only agent against `corpus_of_input_docs/` (quick-mode PDF conversion; every
value marked FACT was re-read from the PDF text layer or the xlsx cell, not from the markdown mirror). Page cites are
PDF page numbers; xlsx cites are `<file-period>:<Sheet>!<Cell>`.

| Block | Report | Source documents | Ids used | Strongest plan impacts (plan section) |
|---|---|---|---|---|
| E101–E149 | `C1_barrick_public_stats.md` | Barrick Q1 and Q2 2026 Mine Stats (xlsx); Fourmile, Lumwana and Tanzania investor decks | 49 | M0 coverage is 2025Q1–2026Q2 with no per-site GEO (§37.3); xlsx locators and the GATE-BULL-01 scoring rule (§21.6); aggregation layers L_corp/L_JV; GB-26 metal-balance and sold/produced factors; 190% vs 504% bridge (OPEN-BULL-4); Lumwana recovery-led variance (M2) |
| E151–E199 | `C2_nevada_technical_reports.md` | NI 43-101 reports: Carlin (2025), Cortez (2022), Turquoise Ridge (2024) | 49 | No public slope or kriging efficiency (GB-11 data gate); NGM MCF out and the "GC adjustment" plug (§20.4); preg-rob as a spike-test % (§17.3); Goldstrike CIL since Q1 2023; 71 lab-vs-plant pairs; cut-off fixed point; pseudoflow in production; closure bond vs estimate (§18.6) |
| E201–E249 | `C3_africa_technical_reports.md` | NI 43-101 reports: Kibali (Feb 2026), Loulo-Gounkoto (2023) | 49 | Site-specific F-factor registry (§20.4); irreproducible published cells; FoS-only acceptance criteria (GB-39); Kibali compensating errors 2016–2025; Loulo-Gounkoto 2025 host-state action (RISK-26); fiscal regime rows; hoist factor ≈ 0.81 |
| E251–E299 | `C4_americas_png_reports.md` | Pueblo Viejo (2026), Veladero, Zaldívar and Porgera technical reports | 49 | Heap book on a recoverable basis (GB-20, §25.2); stockpile degradation state (§20.8); per-row point of reference (§24.3); heap/VLF and riverine facilities, gate and valve state (§18.3); tenure (G-HALL-6); bitemporal ownership |
| E301–E349 | `C5_reko_diq_esia.md` | Reko Diq Early Works ESIA (Rev A, not approved) | 49 | Requirement sets beyond GISTM and the commitments register (§18.2, G-FREE-7…10, OB-COMMIT); chemical basis (§25.2); certificate-vs-report reconciliation; groundwater data gate; state-equity ownership; community-data redaction (§31.2) |
| E351–E399 | `C6_textbook_techniques.md` | Abzalov, *Applied Mining Geology*; Waltham, *Foundations of Engineering Geology*; Lacy; a mining-geology lecture note (reference-only) | 49 | LUC b ≈ ρ and the slope-plus-correlation flip (GB-11); CRM SD basis (§12.4); CV_AVR%, Gy calibration, twins, core recovery (§12); Highwall card rules and TARP triggers (§14) |

Rights and privacy: textbook pages are cited, never shipped as fixtures; Reko Diq appendix personal data (phone
numbers, named individuals) is not reproduced anywhere in the case file or plan.

### Corrections appended after the corpus pass

- **E148 correction (append-only; 2026-10-28, appendix cross-check).** "2025Q1–2026Q2" in E148 and in the C1 index
  row above is shorthand for the workbooks' six period blocks, not a contiguous run of six quarters. The Q1 and Q2
  2026 workbooks each carry only their own period and the prior-year comparative (period headers re-read from the
  xlsx: years 2025 and 2026 only; "three months ended March 31", "three months ended June 30", "six months ended
  June 30"). Per-site quarters available: 2025Q1, 2025Q2, 2026Q1, 2026Q2, plus H1 2025 and H1 2026. **2025Q3 and
  2025Q4 are not in the corpus** and join 2024 and Q3 2026 in the M0 data gate (plan §21.6, §37.3).

### Research evidence blocks (`planning/research/W1–W5`, 2026-10-28/29)

Each block was produced by one agent working from public primary sources (SEC EDGAR, Barrick's IR host, official
gazettes and statute texts). The agents did not edit the plan or this file; the rows live in each report and are
indexed here. Like the corpus rows, they are proposed evidence until the independent claim audit (plan H-4) runs.

| Block | Report | Sources | Strongest plan impacts (plan section) |
|---|---|---|---|
| E401–E449 | `W2_host_state_events.md` | Barrick 40-F/6-K filings 2019–2026; Zijin HKEX notices; Mali JO (Loi n°2023-040); First Quantum releases and AIF; DRC JO (Law 18/001); B2Gold AIF | Mali 2023 Code applied retroactively, royalty ~18%; Porgera 2017 application, 24.5%/23.5%; Cobre Panamá timeline; DRC Art. 80 renewal +5% (§23.4, §21, RISK-26) |
| E451–E499 | `W1_barrick_mine_stats.md` | Ten Barrick Mine Stats workbooks Q1 2024–Q2 2026, MD&As, AISC workbooks; Q3 2026 release notice | M0 coverage Q1 2024–Q2 2026; Q3 2026 releases 9 Nov 2026; 17 discrepancy candidates; corporate AISC layer +$23.57 to +$88.66/oz (§21.6, §37.3, Appendix I) |
| E501–E549 | `W3_data_residency.md` | Statutes and regulations of PK, ZM, TZ, CD, ML, PG, DO, AR, CL, SA, US, CA | 72 residency rows; Zambia DPA s.70 localisation; Reko Diq under BMR 2002 r.56/r.71 (§26.14, §31.2) |
| E551–E599 | `W4_m2_site_and_data_access.md` | Barrick filings and site decks; Lumwana TR; Avathon release | Lumwana stays M2 primary on a grade-adjusted recovery gap; P(site data by month 7) = 0.3 (§34.4, §37.3) |
| E601–E649 | `W5_figure_verification.md` | Barrick 2026 Circular and releases; Newmont 10-K, proxy and releases; MinEx, McKinsey, FactSet, ICMM, vendor pages | 72 figures checked: Tanami 2 history and the 190%/504% bases corrected; pay GEO at fixed reserve prices (Part I, §21, §23) |

### Direct checks (2026-10-29)

- **E651** crates.io returns HTTP 404 for `fmine`, `fmined`, `fmine-core`, `fmine-cli`, `franken_mining`,
  `franken-mining` and `frankenmining`, and the crates.io search API returns 0 crates for "fmine" (FACT; API
  queried 2026-10-29 with a generic User-Agent). OPEN-B4-09 closes with `fmine`.
- **E652** GitHub repository search for "fmine" returns only unrelated `fminer` and `FMinecraft` projects in its
  top results; no `fmine` binary or crate was found (FACT[secondary]: search ranking, not an exhaustive census).
- **E653** crates.io lists asupersync `max_version` = `newest_version` = 0.5.0, published 2026-09-12; the previous
  releases are 0.4.11 (2026-09-10) and 0.4.10 (2026-09-01). No 0.6.x is published (FACT).
- **E654** The local asupersync repository's main branch declares `version = "0.6.0"` (HEAD 83175f51a, 2026-10-28);
  its newest tag is v0.5.0 (FACT; read-only `git log` and `git tag`).
- **E655** Sibling manifests under `/Users/jemanuel/projects` that take asupersync from crates.io pin `=0.5.0` or a
  caret `0.5.0` (which excludes 0.6); franken_networkx allows `>=0.3.4, <0.6`. The frankengraphdb,
  franken_accounting, frankensqlite and frankensearch lockfiles resolve 0.5.0. frankensympy (`fsym-runtime`,
  `fsym-durable`, `fsym-python`; lockfile 0.6.0) and frankensim (lockfile 0.5.0) depend on it by path.
  `fsym-core` has no asupersync dependency (FACT; read-only grep of `Cargo.toml` and `Cargo.lock`).

### Corrections appended after the research pass

- **E024 (append-only).** The North American Barrick IPO is officially targeted for end-2026 (Q2 2026 release);
  on 29 Sep 2026 the CEO said it may slip into January 2027 (FACT[U]; W5 E615, E616). Mark Hill has been permanent
  President and CEO since 4 Feb 2026 (W5 E614).
- **E025 (append-only).** Reserve replacement is published on two bases in the same Circular (gold ~190%, GEO 504%,
  both 2023–25 and ex-A&D), and gold AISC on two bases ($1,423 pay, $1,637 reported). The reserve-replacement bases
  are disclosed; the AISC pay adjustment is not (W5 E603–E608).
- **E026 (append-only).** Discovery to production averages 15.7 years (127 mines, 2002–23 starts) to 17.9 years
  (2020–23 starts) per S&P (FACT[U]); MinEx/CSIRO gives about 13 years. Bond et al. 2007: 21% of 412 interpreters
  found the correct setting (W5 E636, E639).
- **E027 (append-only).** OEMs own most mine-planning and fleet software (Cat–RPMGlobal 2026, Weir–Micromine 2025,
  Sandvik–Deswik 2022, Komatsu–Modular, Hitachi–Wenco); non-OEM firms are consolidating too (IMDEX–Datarock 2026,
  Maptek–PETRA 2026) (W5 E646, E647).
- **E035 (append-only).** Freeport's TROI figures are consultant-reported and seen only second-hand (FACT[U]); Lihir's
  twin claims only unquantified benefits (W5 E643, E644).
- **E148 (second append, 2026-10-29).** The data gate in the E148 correction above is closed except for Q3 2026:
  W1 retrieved every quarterly workbook from Q1 2024 to Q2 2026 (E451, E452). Q3 2026 releases on 9 Nov 2026 (E454).
- **E248 (append-only).** All three Loulo-Gounkoto dates hold (W2). The provisional administrator took day-to-day
  control on 23 Jun 2025; the Q3 2025 workbook's "September 16" is a footnote error (E476); the "no Q1–Q3 2025 data"
  footnote is wrong for Q1 2025, whose first publication carries production (E477, E478).
- **R6 §0 item 5 (noted here; R6 is not edited).** The Mali settlement payment is $253M (FCFA ~143bn, 28 Nov 2025)
  in Barrick's own filing, not the ~$430M press figure (W2 E408; W5). The Avathon release is dated 23 Sep 2026, not
  25 Sep. R6 §6.2's Tanami line needs the E631/E632 correction.

### Independent corpus claim audit (AUDIT_4), second-wave review (AUDIT_5), and Brenner pass B6 (2026-10-29)

- **Independent corpus claim audit (`planning/mor/audits/AUDIT_4_corpus_claim_verification.md`, closing H-4 for E101–E399 and the 18 workbooks).** Two independent second-reader subagents and a deterministic openpyxl workbook verifier audited all 294 corpus evidence rows (`E101–E399` across `C1–C6`) against the primary PDF text layers (`pdftotext -layout`) and `.xlsx` cells, plus all 44,328 populated cells across the 18 Barrick Mine Stats (10) and AISC (8) workbooks (`2024Q1–2026Q2`):
  - **273 / 294 rows (92.9%)** verified `EXACT` against the cited PDF page or `.xlsx` cell.
  - **18 / 294 rows (6.1%)** verified `CONFIRMED_WITH_NUANCE` (e.g., Carlin Table 13-8 vs Table 13-10 numbering, Carlin Table 13-12 exponential vs Cortez rational-cubic roaster curves, Pueblo Viejo Table 15-8 `$14.78/t` evaluation vs `$14.76/t` printed example, and two hidden external-link sheets `Sheet1` / `All Data_Output` in `Q1/Q2 2026 AISC.xlsx` holding `154` `#REF!` cells that must be ignored alongside the visible `YTD` `#REF!` region).
  - **3 / 294 rows (1.0%)** had minor PDF page-offset typos (`E124` p.5 → p.6; `E134` p.15 → p.9; `E239` p.301 → p.302), all corrected in place in `C1` and `C3`.
  - **0 / 294 rows (0.0%)** had substantive factual or numerical errors.
- **Second-wave review (`planning/mor/audits/AUDIT_5_second_wave_review.md`, advancing H-1 to Round 2).** Audited the post-`BD-309–BD-320` and post-`W1–W5` plan against the primary corpus, sibling crate conventions, and the 320-bead DAG. All 8 findings (Carlin/Cortez table and roaster curve attribution in `§15.6`, Pueblo Viejo `Qty<MassFraction, DryBasis>` scaling in `§15.6`, `G-CORE-1`/`G-CORE-2` and `G-STRATA-11` guard registry symmetry in `§12.6`/`§13.11`, `FMINE-REF-108` and `FMINE-REF-504` refusal-code parity in `§8.4`/`§25.6`/`§31.3`, `PO-LODE-15` naming consistency, `BD-161`/`BD-312` → `BD-309` DAG edges bringing the 320-bead graph to 533 edges with 0 cycles, and `fmine-time` `QuadTemporalStamp` description in `§28.3`) were folded into the plan.
- **Brenner pass B6 (`planning/brenner/B6_operations_plant_tailings_workforce.md`, closing H-2 / §36.4).** Executed a full 17-hypothesis Brenner tournament (`H-B6-1`–`H-B6-17`) over operational dispatch (`Headframe`, §16), plant metallurgy (`Crucible`, §17), tailings/environmental governance (`Freeboard`, §18), and workforce/fatigue/bootleg safety (`Muster`, §19), adding `G-CRUC-6`, `G-FREE-11`, `G-MUST-6`, and Gold-Box cards `GB-40`–`GB-43` to the plan and Appendix B/D.

### Second-reader checks of the concurrent revision (2026-10-29)

Two primary-source agents re-read every new FACT claim of the concurrent revision (BD-309–BD-320, B6, and the
AUDIT_4/AUDIT_5 fold-in) against the corpus PDFs and workbooks and the Barrick releases. The new mathematics was
checked numerically. The plan's Appendix H.5b lists the corrections. New evidence rows:

- **E656** Fourmile intercepts: FM24-200D 738.8–740.1 m, 1.3 m @ 16.25 g/t, labelled "Wenban 5/8 contact – Double
  Refractory Roaster feed"; FM24-209D 1.6 m @ 30.6 g/t within 16.3 m @ 47.07 g/t, labelled "Silicified Structural
  controlled – Single Refractory Roaster or Autoclave feed"; FM25-260DW1 3.4 m @ 37.16 g/t (Fourmile deck p.6, p.8;
  FACT).
- **E657** Fourmile timeline: 16 rigs in Q4 2025, 20 surface rigs in 2026, Bullion Hill decline development in 2026,
  underground drilling from the Goldrush MPD in 2027 (Fourmile deck p.4; FACT).
- **E658** IPO perimeter: a NewCo anchored by the NGM and Pueblo Viejo JV interests plus the wholly owned Fourmile,
  with a small minority IPO (Barrick release, 1 Dec 2025,
  https://www.barrick.com/English/news/news-details/2025/barrick-announces-evaluation-of-an-IPO-of-its-north-american-gold-assets/default.aspx).
  Barrick and Newmont agreed on 9 Aug 2026 to vend Fourmile (Barrick) and Mike/Fiberline (Newmont) into NGM (Barrick
  release; Q2 2026 MD&A; FACT).
- **E659** Goldstrike autoclave feed windows: alkaline CO3 > 8% (Table 13-9); acid SS < 2.8% and CO3 < 8% (Table
  13-10). Recovery (Tables 13-13, 13-14): the same piecewise cubic in grade (opt) for both modes, acid about +8.09
  points (Carlin TR PDF p.143, p.146; FACT).
- **E660** Goldstrike roaster recovery R% = 92.03 − 37.36·exp(−12.94·g), g in opt (West Leeville, All Others); 87.53 −
  … for Turf and Four Corners; Pete Bajo uses a log form; no validity range printed (Fig 13-11 spans about 3–13 g/t)
  (Carlin TR Table 13-12, PDF p.144; FACT).
- **E661** Four Corners roaster: R = (g − T)/g with T = 1.821×10⁻⁵·As + 0.127·g − 4.890×10⁻⁴ (g in opt, As in ppm)
  (Carlin TR Table 13-16, PDF p.147; FACT).
- **E662** Carlin mining fleet: 40 Komatsu 930E, 15 CAT 793, 3 CAT 789, 2 CAT 785, 4 CAT 777; the TR does not mention
  autonomy (Carlin TR §16, PDF p.254–255; FACT).
- **E663** Goldrush ore goes to the Goldstrike and Gold Quarry roasters (PDF p.169); Cortez refractory ore is trucked
  about 115 km at about 2.3 Mt/a in 36 t loads (PDF p.35); Cortez Pipeline CIL has 8 tanks with 36–48 h retention (PDF
  p.266). Turquoise Ridge ore goes to Sage (Turquoise Ridge TR PDF p.146) (Cortez TR; Turquoise Ridge TR; FACT).
- **E664** Pueblo Viejo flotation + POX variable cost (Table 15-4): plant −1911.2S⁴ + 857.27S³ − 128.24S² + 11.236S +
  6.8302 US$/t, S as a fraction; power (794.9833S + 22.3323) kWh/t × US$0.07912/kWh. Table 15-8 gives 14.76 US$/t
  at S = 0.0904. Only flotation + POX costs are published (PV TR PDF p.217; FACT).
- **E665** Pueblo Viejo mining cost: ore 3.20, waste 3.68, incremental ore −0.48 US$/t (Table 15-2). TSF sustaining
  capital: ore 5.96, waste 2.25, difference +3.71 US$/t (Table 15-5). The stated reason is temporary WRSF rehandle or
  TSF co-disposal (PV TR §15; FACT).
- **E666** Pueblo Viejo 2025 MCF reconciliation: plant cone opening 281.7 kt @ 2.41 g/t, closing 352.1 kt @ 2.44 g/t
  (PV TR Table 15-10, PDF p.222; FACT).
- **E667** Pueblo Viejo tailings: El Llagal until 2030; Naranjo from mid-2030 to 2048. PAG waste goes into a dump
  inside the Naranjo impoundment from 2028 to 2045 and is submerged at the end of mining under at least 10 m of
  tailings; temporary WRSF before then (PV TR §18; FACT).
- **E668** Lumwana: 52 Mtpa run rate during 2028, peak design 54 (p.18, p.23); shovel electrification (p.21); "for
  every MW generated more than a third is lost as unusable reactive power" (Zutari 2024, p.30); 330 kV lines
  complement STATCOMs at neighbouring mines (p.30); 100% BESS backup, 50% emergency generation, 45% imported within
  the SAPP, emergency capacity to 31.5 MW by Q4 2025 (p.29) (Lumwana site-visit deck; FACT).
- **E669** Kibali power: Nzoro II about 22 MW, Ambarau 10.6 MW, Azambi 10.2 MW (42.8 MW peak); 32 MW high-speed
  diesel; a 7 MW BESS (2020) cut the spinning reserve from nine generators to four; 16 MW solar and 15 MW BESS (2025);
  about 85% renewable energy (Kibali TR PDF p.28, p.294; FACT).
- **E670** Loulo-Gounkoto (80% basis): Q2 2026 914 kt processed at 3.25 g/t with 92% recovery; production 88 koz in
  Q2, 64 koz in Q1 and 152 koz in H1; royalties (US$/oz) Q1 684, Q2 585, H1 629; TCC Q2 1,897, H1 1,906; AISC Q2
  2,240, H1 2,104 (`Barrick_Q2_2026_Mine_Stats.xlsx` and `Barrick_Q1_2026_Mine_Stats.xlsx`, sheet `Minestats3`; FACT).
- **E671** Qualified persons named in the Q2 2026 release endnote: Richard Peattie (Chief Technical Officer), Sam
  Baldwin (VP Geology, MAIG), Joel Holliday (EVP Exploration), Jesse Clark (VP Geology, SME RM) (Q2 2026 workbook,
  sheet `Endnote`, cell A2; FACT).
- **E672** Pueblo Viejo ownership: 60% Barrick, 40% Newmont (PV TR PDF p.15; FACT).

**Corrections to the 2026-10-29 AUDIT_4/AUDIT_5 block above (append-only).**
- AUDIT_4's three page-locator fixes are **E146** (FM p.15 → p.14), **E211** (K p222 → p221) and **E212** (K p222 →
  p220), as AUDIT_4 §2.1 states and as the corpus files now read. The block above lists E124, E134 and E239 by
  mistake.
- The nuance "Carlin Table 13-12 exponential vs Cortez rational-cubic roaster curves" is wrong. Neither the
  0.8926·[1 − exp(−22.2995·(Au − 0.032))] law nor the rational cubic 1.1536 − 0.0046/g − … appears in any corpus
  document. Table 13-12 is E660 above; E154 is roaster MCF and E181 is Turquoise Ridge intercepts.
- The Pueblo Viejo Table 15-8 check works at S = 0.0904 (14.76 US$/t), not S = 0.0368.
