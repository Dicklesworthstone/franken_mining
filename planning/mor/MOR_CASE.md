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

### Round 7: M0 code and execution evidence (2026-10-29)

The round-7 modes-of-reasoning run (`planning/mor/r7/`) added these rows. Full text, commands and logs are in
`planning/mor/r7/MOR_CASE.md` and `planning/mor/r7/.mor/run-20261029T183653Z/repro/`; ids E700–E814 (claim-level
evidence added during normalisation) are defined there only.

- **E673** Git: local commit `68e7091` (2026-10-29 13:42 -0400) adds 15 crates, `xtask`, registries, ADR-RNG-01, CI
  and `.beads/` (320 issues); its message says WP-00..WP-03 and WP-06 are implemented. Not pushed: `origin/main`
  and `origin/master` stay at `30ee8f5` (FACT).
- **E674** `cargo test --workspace` at `68e7091`: 59 unit tests pass over 16 binaries; doc-tests 0 (FACT, execution).
- **E675** `xtask check-all` prints "GATE-LAYERS-00 PASS: 60 crates registered, 8859 actual Rust LOC" (FACT,
  execution).
- **E676** `fmine assure public --issuer barrick` exits 0 and prints one envelope (1,108 bytes, byte-identical on
  rerun) with gold FY2025 AISC 1,638.10 vs "reported" 1,638, a North Mara LMDI labelled 2024 Q1→Q2 (1,297 →
  2,582) and reserve replacement 190%/504% (FACT, execution).
- **E677** None of the 18 workbook names compiled into `BARRICK_18_WORKBOOKS` exists in the corpus; the local
  Barrick workbooks are named `Barrick_Q{1–4}_{2024–2026}_{Mine_Stats|AISC}.xlsx` (FACT).
- **E678** `BarrickCorpus::load_from_workspace` substitutes an in-code workbook
  (`build_canonical_barrick_xlsx_bytes`) whenever a file read fails (FACT, code).
- **E679** `run_barrick_m0_assurance` builds the North Mara LMDI and reserve replacement from literals
  (`ReserveReplacementBridge::compute(3.91, 7.429, 12.277)`) (FACT, code).
- **E680** `.beads/issues.jsonl` at `68e7091` holds 320 issues, 87 closed, all with one shared close reason (FACT).
- **E681** Root `Cargo.toml` declares `license = "MIT"` beside the rider LICENSE; clippy lints do not deny
  `unwrap_used`, `expect_used` or `panic` (FACT).
- **E682** Plan file at `68e7091`: 8,902 lines, sha256 `e8429e48d43ca739b72d4e49d03c92981d2991ae2bc338146d3262d9e5451390`
  (FACT).
- **E683** CI (`.github/workflows/ci.yml`) runs `xtask check-all`, `cargo check` and `cargo test` on ubuntu-latest
  and macos-14; no fmt, clippy or cargo-deny step; it has never run, because the commit is unpushed (FACT).
- **E684** Registries at `68e7091`: calendars, crs, lifts, loc_budget, refusals, units, verbs, workspace_topology
  (FACT).
- **E685** The GATE-BULL-01 text audited in round 7 ("≥ 95% of site-quarter values reproduced to published
  rounding"), since replaced by the three-part gate (plan §21.6; FACT).
- **E690** Build host: 18.3 GB free on 2026-10-29, `sbh` status critical (FACT, execution).

### Round 7: B7 refutation and dueling-wizards checks of public files (2026-10-29)

Ids E691–E696 come from round 7's reserved block (E673–E699). Each was re-read from the bytes by the lead agent
unless marked otherwise. B7 pack ids refer to `planning/brenner/B7_assurance_chain.md`.

- **E691** `Barrick_Q4_2025_AISC.xlsx` `'Q4 2025'!J5:J21` (Veladero): cost of sales 67, total cash costs 39, AISC 84
  ($M), ounces sold 47 koz, but per-ounce rates 1,526 / 886 / 1,915, which imply 43.9–44.0 koz. Sheet `2025`
  column J: rates 1,286 / 785 / 1,450 imply 223.4–224.2 koz against 226 printed. `'Q3 2025'!J18` is 44 koz
  (FACT, execution). The cause is not public; "Q3's denominator reused" is a HYPOTHESIS (B7 EV2-P3-2).
- **E692** `Barrick_Q4_2024_MD-A.pdf` p.77, note 15: the 2024f row prints attributable net change GEO 6.7, the
  same value as 2023e, while the "2019 - 2024 Total" row prints 111 (4.2 + 8.5 + 13 + 6.7 + 79 = 111.4).
  `Barrick_Q4_2025_MD-A.pdf` p.71 prints 79 for 2024 (FACT). The first-filed vintage is wrong; see E147 for the
  deck question on 79.
- **E693** Row positions move between AISC workbooks: AISC, ounces sold and AISC per ounce sit in rows 17/18/21
  in `Barrick_Q4_2025_AISC.xlsx` `'Q4 2025'` and rows 18/19/23 in `Barrick_Q1_2025_AISC.xlsx` `'Q1 2025'`, where
  a "Non-recurring items" row is inserted at row 8 (FACT). B7 found the shift on 25 of 40 AISC sheets
  (EV-P3-2; INFERENCE, not re-counted).
- **E694** `Barrick_Q1_2025_AISC.xlsx` `'Q1 2025'!S18,S19,S23` (Africa & Middle East): 312 $M / 202 koz =
  1,544.6 $/oz against 1,602 printed (FACT). Whether the filed Q1 2025 MD&A explains the gap is disputed inside
  B7 (EV2-P2-3 vs EV2-P3-2: Loulo-Gounkoto sold no gold that quarter but carried about $10M of sustaining
  capital); open.
- **E695** Per-ounce identity AISC $M × 1,000 / koz sold = printed $/oz, on `Barrick_Q4_2025_AISC.xlsx` and
  `Barrick_Q1_2025_AISC.xlsx`, seven visible sheets: 104 eligible identities; 14 within ±$1 of the printed rate,
  87 inside the enclosure of a subtotal rounded once, 17 outside it (B7 EV2-P2-3; FACT, execution by a B7 pane,
  not re-run). Treating the subtotal as a sum of separately rounded component lines explains most of the rest
  (EV2-P3-2: 68 of 75 misses on its own sample; INFERENCE). No filing states which construction the issuer uses.
- **E696** A swapped-value mutation (two sites' values exchanged in memory, citations kept, both rows labelled
  "issuer discrepancy") leaves 0 unclassified and 0 uncited rows while extraction fidelity against an independent
  read falls from 2/2 to 0/2 (B7 EV2-P1-2, EV2-P2-3; FACT, execution in scratch). Classification completeness
  cannot stand in for extraction fidelity.
- **E697** The "General & administrative costs" row is zero in every column of all 40 visible AISC sheets in the
  eight `Barrick_Q*_AISC.xlsx` workbooks (FACT, execution). Site and region per-ounce AISC therefore carry no G&A,
  and corporate G&A sits wholly in the Total layer (L_corp; §21.6).
- **E698** 2025 gold reserve roll-forward in two filings of February 2026: `Fourmile_Feb2026.pdf` p.14 prints
  acquisitions and divestments (2.2), depletion (3.6), net change 1.7; `Barrick_Q4_2025_MD-A.pdf` p.71 prints
  depletion (3.7), net change 1.8. Both close 89 → 85 Moz to rounding (FACT). The pair fits one unrounded split near
  3.65 / 1.75 rounded by two conventions (INFERENCE); a known answer for rounding-convention discovery.
- **E699** LMDI on the public tables (B7 EV-P0-4, EV2-P3-3, EV2-P3-4, EV-P2-5; executions by B7 panes, not
  re-run; INFERENCE): 26 of 127 gold site-quarter year-on-year pairs have an `n/a` or non-positive factor
  (Veladero and Zaldívar publish no recovery); 72 of 606 factor effects change sign within rounding; by-product
  credits are zero or positive in 124 of 519 AISC columns; a +10% error in North Mara's 2026 grade
  (`Minestats3!H18`, 1.80 → 1.98 g/t) moves $177.39/oz from grade into B with closure unchanged.

**B7 decisive pilot (lead-agent checks of the B7 quickie pack, 2026-10-29).** Ids E815–E818 are new; E700–E814
belong to the round-7 run above.

- **E815** Footing census over all visible sheets of the eight `Barrick_Q*_AISC.xlsx` workbooks: 1,207 of 1,212
  printed TCC and AISC totals equal the sum of the displayed component lines above them (FACT, execution, label-
  anchored rows). The five exceptions are three cells, two of them in two vintages: Kibali Q2 2024 TCC 70 against
  72 (by-product credit printed +1 where −1 foots; `Barrick_Q3_2024_AISC.xlsx` `'Q2 2024'!O7`, and again in the
  Q2 2025 workbook); Porgera Q2 2024 AISC 13 against 12 (`'Q2 2024'!L18`, both vintages); North America Q4 2023
  TCC 541 against 540 (`Barrick_Q4_2024_AISC.xlsx` `'Q4 2023'`, column J). A total rounded once from unrounded
  inputs would foot exactly only about 56% of the time (B7 EVQ-P0-1 synthetic null; INFERENCE), so the tables
  follow the component-sum construction (INFERENCE).
- **E816** Africa & Middle East Q1 2025 (E694) closes from public cells: `Barrick_Q1_2025_MD-A.pdf` p.18 prints
  Loulo-Gounkoto cost of sales $5M and depreciation $5M; `Barrick_Q1_2025_Mine_Stats.xlsx` `Minestats3!B32:B34`
  print sustaining capital 10, projects 3, total 14, and `Barrick_Q2_2025_Mine_Stats.xlsx` `Minestats3!B32`, `B74`
  print 3 and 13 (FACT). The region's per-ounce numerators exceed its members by +10.8 $M AISC in Q1, +2.9 in Q2
  and +14.0 in H1, each within the member-weighted rounding bound of Loulo-Gounkoto's sustaining capital (B7
  EVQ-P0-1; INFERENCE, not re-run). No page states that the per-ounce rows include a site whose $M rows and
  ounces are shown as nil, so the item is explained by reconstruction, with the rule unstated.
- **E817** Cost-of-sales per ounce has a one-cell numerator, so it fails or passes on any rounding construction:
  `Barrick_Q4_2025_AISC.xlsx` `'Q4 2025'` Kibali (column M) 123 $M at 1,557 $/oz implies 79.0 koz against 78
  printed, and Hemlo (column H) 44 $M at 1,738 $/oz implies 25.3 koz against 27 (FACT; both outside their
  half-unit intervals). B7 classes 35 of 46 out-of-interval per-ounce quantities as explained and 11 as having no
  public closure (Veladero ×6, Hemlo ×2, Latin America & Asia Pacific 2023 ×3; EVQ-P0-1; INFERENCE, not re-run).
- **E818** Loulo-Gounkoto: `Barrick_Q2_2025_MD-A.pdf` p.56 note j and `Barrick_Q3_2025_MD-A.pdf` p.55 both date
  the loss of control June 16, 2025, against September 16 in the Q3 2025 workbook footnotes (W1-DR-03); and
  `Barrick_Q1_2026_MD-A.pdf` p.42 states that "no operating data or per ounce data was provided for Q1 2025 to Q3
  2025" (FACT). The filed date resolves W1-DR-03, and the later filing supports NOT_PUBLISHED for those quarters
  while the Q1 2025 workbook's production cells stay a register item.
- **E819** `Barrick_Q2_2026_AISC.xlsx` `'Q2 2026'` North Mara (column M): cost of sales 117, TCC 78, AISC 93,
  ounces sold 37 koz attributable, and per-ounce rates COS 2,736, TCC 2,164, AISC 2,577 (FACT). The TCC and AISC
  rates imply 36.0 and 36.1 koz against 37 printed (FACT, arithmetic) but stay inside their component-sum intervals,
  so only the one-cell cost-of-sales test decides: with the 84% attributable share applied to cost of sales, it
  implies 35.8–36.1 koz, outside 37 ± 0.5 (INFERENCE, the share is assumed; B7 by_cc D2 and EV2-P1-4 agree). B7's
  pilot had classed this column explained and withdrew that, so its count becomes 34 of 46 explained and 12 without
  public closure (not re-run). The signature matches Veladero (E691): all three rates point to one ounce figure
  about 1 koz below the printed one.

**M0 machine run (lead agent, 2026-10-29).** Ids E820–E823 are new. An independent Python reader and census were
written first and their outputs pinned; the Rust implementation (`fmine-io`, `fmine-public`) was then written
against them and run on the 18 registered workbooks (BD-321, BD-322).

- **E820** Extraction fidelity (FACT, execution). The Rust reader and the independent reader agree exactly on every
  registered workbook: visible and hidden sheet counts, numeric, placeholder, formula-error and text cell counts,
  and the SHA-256 digest of every numeric cell (`"{sheet}\t{cell}\t{value}\n"`, visible sheets in workbook order,
  cells by row then column). Totals: 101 visible sheets, 6 hidden, 30,388 numeric cells, 3,952 placeholders, 1,092
  formula errors (all in `Barrick_Q2_2026_Mine_Stats.xlsx`), 6,537 text cells. Test:
  `barrick_real_files_match_the_independent_reader` (ignored by default; the files are not in git).
- **E821** Six of the 40 sheets with a cost-of-sales row are hidden (FACT): `Barrick_Q4_2024_AISC.xlsx` `'Q4 2023'`,
  `Barrick_Q4_2025_AISC.xlsx` `'Q4 2024'`, and `'YTD Q3 2025'` and `'YTD Q3 2024'` in both 2026 AISC workbooks. The
  ledger reads the 34 visible tables only. This corrects E815, whose "all visible sheets" counted hidden ones: its
  North America Q4 2023 TCC exception (541 against 540) sits on a hidden sheet and is outside the M0 ledger.
- **E822** Identity ledger under the declared component-sum construction (FACT, execution; Rust equals the
  independent census in every count). 34 tables, 519 subject columns, 2,595 instances.
  - **Footing:** 1,004 consistent (i), 4 issuer discrepancies (iii), 6 refused (v), 24 not published.
  - **Per ounce:** 1,116 (i), 3 explained (ii), 17 undecidable (iv), 385 (v), 36 not published.
  - **Alternative construction** (subtotal rounded once; reported, not scored): it accepts 1,008 footing instances
    and 994 of the 1,116 consistent rates.
  - **The four (iii) instances are two cells, each in two vintages** (`Barrick_Q3_2024_AISC.xlsx` and
    `Barrick_Q2_2025_AISC.xlsx`, sheet `'Q2 2024'`):
    - Kibali TCC prints 70 (O11) against a component sum of 72. The run's sign-flip witness is O7, by-product
      credits printed +1: −1 closes the footing exactly. That the sign is the error is INFERENCE.
    - Porgera AISC prints 13 (L18) against 12, with no single-line witness.
  - **The three (ii) instances are E816's.** Each is re-verified on the run by the label checks on `Minestats3`:
    row label, Loulo-Gounkoto header, period and year.
  - **The 17 (iv) instances are all cost of sales per ounce:** Veladero ×12, Hemlo ×3, Kibali ×2.
  - **The (v) instances:**
    - 379 cost-of-sales rates in columns with a non-zero non-controlling-interest line ("attributable share
      needed");
    - 12 Loulo-Gounkoto instances (E823).
  - **Not published:** 12 all-zero columns.
  - State root `9283f89649b8e51b63e271532d8227b801ee0e6dce522d7a1b5e60c9ad389bb2`, anchor
    `anc_3ebc2f9029781b51cc242d95fc0df692` (2026-10-29). The earlier root `0aa62d52…` (543df53) printed the
    upper bound of undecidable rate ranges rounded down; the range text now rounds outward. Counts are unchanged.
- **E823** Loulo-Gounkoto, column K of `Barrick_Q1_2026_AISC.xlsx` `'Q1 2026'` and of `Barrick_Q2_2026_AISC.xlsx`
  `'Q2 2026'` and `'Q1 2026'`, stores unrounded values (FACT). In Q1 2026:
  - "Other" is −37.899643 (K8);
  - TCC is 132.100357 (K10);
  - AISC is 133.100357 (K17).

  At stored precision both footings close exactly: 241 − 38 − 37.899643 − 33 = 132.100357 (FACT, arithmetic). The
  ledger refuses these 12 instances because the declared construction needs the displayed values, and those
  depend on the cell number format, which the reader does not yet read. (Since E838 the ledger reads them,
  and all 12 are (i).)

**frankensim materials corpus, re-counted (lead agent, 2026-10-29).** Id E824 is new and corrects E009.

- **E824** The frankensim matdb seed corpus at commit `20463525d`, the commit §13.9 was verified against (FACT,
  execution: `git ls-tree` and `git show` of `data/matdb/seed-v1/*/properties.tsv` at that commit).
  - **Size.** 302 bundles holding 2,631 value claims (scalar, curve and surface rows).
  - **Uncertainty.** Of the uncertainty rows, 2,466 are `unstated` (94%), 163 absolute and 2 bounded. Unstated
    uncertainty becomes an explicit no-claim, never a verified value (`crates/fs-matdb/src/query.rs:1473-1480`).
  - **Schema.** Pack schemas run v1–v8 (`crates/fs-matdb/src/pack.rs:20-34`). v8, two-coordinate surfaces, landed
    on 2026-10-21 (`b323fed42`), so E009's "frozen at v7" is out of date.
  - **Coverage.** There are still no rock, ore, mineral, soil, slurry or tailings bundles.
  - **Working tree.** On the same day, frankensim's working tree held 264 bundles: a live session had 38 bundle
    deletions uncommitted, among them `gold-pure-nbs-c447`. Corpus counts are therefore quoted at a commit, never
    from a working tree.

**Mine Stats reader against W1's reader (lead agent, 2026-10-29).** Id E825 is new.

- **E825** `fmine_public::mine_stats` (bd-gwf1.1), the label-anchored Rust reader, reads the ten registered Mine Stats
  workbooks (2024Q1–2026Q2) exactly as W1's openpyxl reader did before the Rust code existed (FACT, execution:
  ignored test `barrick_mine_stats_match_the_w1_reader`; reference `scripts/oracle/mine_stats_extract3.py`, output
  `long_all3.json` sha256 `8992ae50…81dd`, lines from `scripts/oracle/mine_stats_lines.py`).
  - **Equality.** Same canonical-line digest on every file: sheet, cell, view, period, printed and clean site, label,
    section, kind, and the raw token (numbers as IEEE-754 bits). Q2 2026 and Q4 2024 were also compared byte for
    byte. There were no refusals and no hidden sheets.
  - **Counts.** 22,732 cells in 102 blocks: 18,775 numbers (7 of them numeric zeros), 1,173 `—` or `'— '`,
    2,778 `n/a`, 6 blank.

    | File | Cells | Numbers | Zeros | Dash | n/a | Blank |
    |---|---|---|---|---|---|---|
    | Q1 2024 | 1,380 | 1,082 | 0 | 108 | 190 | 0 |
    | Q2 2024 | 2,760 | 2,204 | 0 | 183 | 373 | 0 |
    | Q3 2024 | 2,760 | 2,210 | 0 | 177 | 373 | 0 |
    | Q4 2024 | 2,760 | 2,213 | 0 | 173 | 372 | 2 |
    | Q1 2025 | 1,292 | 1,095 | 0 | 52 | 145 | 0 |
    | Q2 2025 | 2,584 | 2,213 | 0 | 82 | 288 | 1 |
    | Q3 2025 | 2,584 | 2,220 | 0 | 82 | 281 | 1 |
    | Q4 2025 | 2,616 | 2,266 | 0 | 44 | 306 | 0 |
    | Q1 2026 | 1,308 | 1,066 | 3 | 90 | 152 | 0 |
    | Q2 2026 | 2,688 | 2,206 | 4 | 182 | 298 | 2 |
  - **Plan §21.6 / C1.** Q1 and Q2 2026 hold 3,996 cells. The seven numeric-zero placeholders (C1 DR-07) are numbers
    with stored text `0` at `Minestats1!N27`, `N28` and `Minestats3!F19` (both files) and `Minestats3!F64` (Q2 2026).
  - **W1 §4 M0 census.** It reproduces: 820 cells = 744 numeric, 33 `'— '`, 11 `'—'`, 28 `n/a`, 4 numeric zero.
    W1 gave the counts but not the selection. The selection that reproduces them was found in Python on W1's own
    read first: QTD cost of sales, total cash costs or C1, and AISC; three-month processed grade and site recovery;
    current year; aggregates excluded, the Nevada Gold Mines total among them.
  - **Change found on the way.** The xlsx reader used to trim placeholders, so `'— '` read as `'—'` and W1's census
    could not be reproduced. It now keeps the token as stored. The real-lane state root did not move.
  - **Not covered.** The metric-label registry (labels, units, region maps v0–v2, W1-DR-01 and W1-DR-02 as data) and
    the refusal of unregistered labels are not built. The read is not yet in the state root or the identity ledger.
- **E826** The stated share rule (BD-331.S3, bd-prmn.3) over the 379 cost-of-sales-per-ounce rates that E822 refused
  as "attributable share needed" (FACT, execution). Independent census: `python3 -I scripts/oracle/share_census.py .`,
  written 2026-10-30 by a separate agent that did not read `crates/` or `rate_census.py`; it ran before the Rust.
  - **Rule.** Each AISC sheet's footnote, found by text (A35 "h" in 2024Q3–2025Q1, A35 "g" in 2025Q2, A36 "h" in
    2025Q3, A32 "g" from 2025Q4), states the rate on an attributable basis using Barrick's ownership share. The rate
    is checked as 1000·s·N/Q on exact interval corners, at the registered share s: NGM and its four sites 61.5%
    (E457), Long Canyon 61.5% through 2023 (E827), Pueblo Viejo 60% (E672), Loulo-Gounkoto 80% (E670), North Mara
    and Bulyanhulu 84% (E135), Tongon 89.7% (E487). The same-release Mine Stats `QTD` label must print the same
    share; all 328 labels do.
  - **Result.** 328 site instances: 263 fit (i); 65 miss (iv). The 51 region instances stay (v): "regional
    numerator needs member weighting". None keeps "attributable share needed". The 65 misses are 39 distinct data
    points, because periods repeat across vintages: Carlin 15, NGM 14, North Mara 3, Pueblo Viejo 3, Cortez 2,
    Bulyanhulu 1, Turquoise Ridge 1.
  - **History.** The first run (commit 04684ff, root `be22f271…`) registered no Long Canyon share, so those 3
    instances were (v) "no registered share" and the result was 260 fit. Long Canyon was then registered on E827,
    by the same lead decision in both readers, and the census was re-run by its own author.
  - **Fit and miss by subject.** Carlin 9/25, NGM 11/23, Cortez 31/3, Turquoise Ridge 32/2, Phoenix 34/0, Long
    Canyon 3/0, Pueblo Viejo 29/5, North Mara 30/4, Bulyanhulu 31/3, Loulo-Gounkoto 23/0, Tongon 30/0. Long Canyon's
    operands are small, (6, 2, 1832) for example, so its fits allow shares as wide as [0.4227, 0.8330]: weak
    support. Every 61.5% miss interval ends
    below 0.615 (highest upper bound 0.6146, rounded up); every Pueblo Viejo miss interval ends below 0.600 (highest
    0.5998). INFERENCE: one-sided misses suggest a consistent basis difference, not rounding. North Mara Q2 2026,
    (n, q, r) = (117, 37, 2736), allows only s in [0.849751…, 0.880847…], which excludes 0.84.
  - **Rust equals the census** on all 379 instances: state, subject, footnote cell and label cell. On the 65 miss
    intervals, Rust rounds outward to four places and the census rounds to nearest, so each Rust bound lies within
    0.0001 of the census bound and contains it. Ignored test `barrick_share_rule_matches_census` pins the counts.
  - **Judgement call.** The 2026 footnotes say "Gold COS/oz is calculated as cost of sales…", not "Gold cost of
    sales per ounce…". Both readers accept this wording because the rate row there is labelled "COS/oz". It decides
    73 instances: 63 fit and 10 miss.
  - **State root** `0c6d01e3f3a8020e4cc6591559c04d10a171e4da0805c0b47f7784357cf03613` (2026-10-30), replacing
    `9283f896…` (E822). Totals: i 2383, ii 3, iii 4, iv 82, v 63, np 60.
- **E827** Long Canyon was in Nevada Gold Mines at Barrick's 61.5% until care and maintenance at the end of 2023
  (FACT). `QTD!A33` of `Barrick_Q1_2024_Mine_Stats.xlsx`, `Barrick_Q3_2024_Mine_Stats.xlsx` and
  `Barrick_Q4_2024_Mine_Stats.xlsx` reads: "These results represent our 61.5% interest in Carlin, Cortez, Turquoise
  Ridge, Phoenix and Long Canyon until it transitioned to care and maintenance at the end of 2023". The label at
  `QTD!A9`, "Long Canyon (61.5%)2", cites footnote 2 (A32, metrics ceased from Q1 2024; E489). Read 2026-10-30 by
  two readers. Only the 2023 comparative sheets of the Q3 2024 and Q4 2024 AISC files print Long Canyon rates.
- **E828** Hidden scope and state-type completion (BD-331.S2, bd-prmn.2; FACT, execution, 2026-10-30).
  - **Hidden scope.** The six hidden sheets of E821 hold 6 AISC tables and 435 instances. Footing: 169 (i), 1 (iii),
    4 not published. Per ounce: 235 (i), 17 (iv), 3 (v), 6 not published. Census:
    `python3 -I scripts/oracle/hidden_census.py .`. It is a wrapper with no ledger logic, written before the Rust
    ran on the corpus, by the agent that wrote the Rust. It runs census2.py and share_census.py unchanged, with their
    AISC sheet choice pointed at the hidden sheets. Rust equals it on all 435 instances (file, sheet, column,
    subject, family, state), compared outside the test. Ignored test `barrick_hidden_scope_matches_census` pins
    the counts.
  - **The (iii)** is North America Q4 2023 TCC, `Barrick_Q4_2024_AISC.xlsx` `'Q4 2023'` column J: 541 printed
    against a component sum of 540 (E815, E821). Visible 1,038 plus hidden 174 footing totals give E815's 1,212.
  - **Bead expectation not met.** The bead expected 204 scoreable hidden footings, 203 consistent (INFERENCE,
    E815 minus E822). The census finds 170 scoreable, 169 consistent, plus 4 not published. Only the census's
    numbers are pinned.
  - **Never counted.** Hidden instances appear in the report under `hidden_scope`, labelled "sheet not displayed
    to readers". They are bound into the state root in their own section (frame `M0AS` v3), and are outside the
    totals and `GATE-BULL-01`.
  - **Missing fact on every (iv).** Visible, 82: Veladero 12 "the ounce series behind the per-ounce rows" (E691);
    Kibali 2 and Hemlo 3 "the attributable ounces sold used for the rate" (E817); 65 stated-share misses "the
    attributable cost of sales or ounces the issuer used". Hidden, 17: 13 stated-share misses, Veladero 3, and
    one Long Canyon rate with the family default "the numerator or denominator the issuer used for this rate".
    No closing document is known for any of them.
  - **Basis on every (ii).** E816's three are `explained_reconstructed`; no Barrick (ii) is stated.
  - **State root** `fb8b38c2e3c6bf0c3f59e82ac88e00106b4048efb3c0463b06b8c0cf8a9c2601`, replacing `0c6d01e3…`
    (E826): frame v3, the hidden section, and the missing fact appended to each (iv) detail. No visible state
    changed; totals i 2383, ii 3, iii 4, iv 82, v 63, np 60.
- **E829** Expected-roots file (BD-339.S1, bd-s4oy.1; FACT, execution, 2026-10-30). `repro/barrick/expected.toml`
  holds one append-only row for each change that moved the real-lane root. Each row was observed by building
  that commit from `git archive` in its own target directory and running `fmine assure public --issuer barrick`
  on the 18 registered files.
  - **E822's row resolved.** Commit 748da79 is the last commit before defab9d; no crate changed after 543df53.
    It prints root `0aa62d52…`, anchor `anc_a2d1090f25ec08128f0134f8559dcdb8` and totals 2120/3/4/17/391/60, as
    E822 recorded. defab9d prints `9283f896…` with anchor `anc_3ebc2f9029781b51cc242d95fc0df692`. A first
    defab9d run printed `0aa62d52…` because it reused 748da79's target directory and did not rebuild. That run
    is discarded.
  - **Two values a reader can recompute without this code.** registry_epoch is sha256 over the sorted
    `"{file}\t{sha256}\n"` lines of the 18 registry rows: `1b81a8aa…`. census_digest_of_digests is sha256 over
    `"{file}\t{numeric_digest}\n"` in registry order, with the digests from `scripts/oracle/fidelity.py`:
    `003ba318…`. Both were computed in Python before the Rust computed them; the Rust equals both.
  - **Planted flip.** With the last digit of the last row's root changed from 1 to 0, three tests failed:
    `barrick_real_files_match_the_independent_reader`, which named the field and both values;
    `test_cli_assure_public_barrick_real_files`; and the default-CI `test_expected_roots_file_is_well_formed`,
    because the anchor derived from the flipped root no longer matched. With the digit restored, all three pass.
- **E830** Clean-clone reproduction (BD-339.S2, bd-s4oy.2; FACT, execution, 2026-10-30, owner machine).
  - **Fetch.** `git clone` of commit 9c33dbb into a scratch directory, with no corpus, then
    `repro/barrick/fetch.sh`. It downloaded 18 files from the issuer's CDN, one GET each, 2 s apart, none
    redirected. Every file matched its registered size and sha256. The issuer still serves the bytes
    registered on 2026-10-29. Summary line: `{"stage":"fetch_summary","files":18,"ok":18,"failed":[]}`.
  - **Tests.** In the clone, `cargo test -p fmine-public -p fmine-cli -- --ignored` passed 6 of 6, including
    the root and anchor of `repro/barrick/expected.toml`'s last row (E829). A reader with a clean checkout and
    the issuer's own files obtains the same state root.
  - **Planted flip** in the clone's copy of `Barrick_Q3_2025_AISC.xlsx` (byte 100; the owner corpus was not
    touched). fetch.sh exited 1 with "present file does not match the registry: register a new vintage (do not
    edit the digest)" and left the file as it was. `fmine doctor` stayed exit 0, status ok, with 17 of 18
    verified and that file `present: true, bytes_ok: true, digest_ok: false`.
  - **Owner corpus.** `fmine doctor` reports 18 of 18 verified. fetch.sh verifies the 18 present files and
    downloads nothing.
  - **No `sources.tsv`.** The bead planned one for the case where the TOML registry had not landed. It had
    landed, so the registry is the one copy. `test_fetch_list_equals_registry` checks that fetch.sh's awk
    reading equals the Rust loader's. Planted once, two ways, and both failed it: a parser keeping `_` in byte
    counts, and a listing without the URL prefix.
- **E831** The eight AISC workbook URLs serve the registered bytes (BD-339.S3, bd-s4oy.3; FACT, execution,
  2026-10-30). E830's clean-clone fetch requested each URL once, under the prefix
  `https://s25.q4cdn.com/322814910/files/doc_financial/quarterly_results/`. The effective URL equalled the
  requested one, and the size and sha256 equalled the registered row:
  - `2024/q3/Barrick_Q3_2024_AISC.xlsx`, 40,977 bytes,
    `d063ea7c1cd667a6db9c9de1b3aa4cb0ff2dba4a8cb24f6ce889e56af2fd0ef7`;
  - `2024/q4/Barrick_Q4_2024_AISC.xlsx`, 42,141 bytes,
    `f842696915b7a44d00194e01a3bdf4a76d1e52d0e31c2610a91be70afb4b1b4d`;
  - `2025/01/AISC_Q1_2025.xlsx`, 32,587 bytes, `06951434c3a3e325553dbe25018544c7329bc1a8b6e01ab0d3fcdeba2bb4388e`;
  - `2025/q2/AISC_Q2_2025.xlsx`, 41,599 bytes, `1cbd6b739a276d64e4752ebbc67c369720ccb8eec16845a4858dccd04fd6d178`;
  - `2025/q3/AISC_Q3_2025.xlsx`, 41,020 bytes, `62901aaab21d64057422a950e29e0ad2ecebad525e0c2b8b9185e4843d9ecfd3`;
  - `2025/q4/AISC_Q4_2025.xlsx`, 40,336 bytes, `2ce7b8e46b36d745228487e683b221101c4db3019ba67acedea96967486d6755`;
  - `2026/q1/AISC_Q1_2026.xlsx`, 43,718 bytes, `dbe567e90730b4ef87330b1eaa539c7289a92781e452b949c24f7b705464ef28`;
  - `2026/q2/AISC_Q2_2026.xlsx`, 49,738 bytes, `66515a4ae4305de130b72fa1a3a78bf6aa51ac5a10350fc43732bb621c6d0e99`.

  Their `url_basis` changes from "W1 naming rule" (INFERENCE) to "fetched and verified 2026-10-30". The Q1 2025
  path `2025/01/` breaks the naming rule, yet it is the URL the registry already held, and it served the
  registered bytes. No URL needed correcting. The fetch ran on the owner machine, not in CI.
- **E832** The kit README's commands, followed on a fresh clone of 7658f55 (BD-339.S4, bd-s4oy.4; FACT,
  execution, 2026-10-30). The run was by the applying agent, not by the owner. The clone came from the local
  repository because GitHub did not yet hold the commit.
  - **Fetch.** `repro/barrick/fetch.sh`: `{"stage":"fetch_summary","files":18,"ok":18,"failed":[]}`, a second
    clean download of all 18 files that matched the registry.
  - **Tests.** `cargo test -p fmine-public -p fmine-cli -- --ignored`: 6 of 6 passed.
  - **Root.** `cargo run -q -p fmine-cli -- assure public --issuer barrick --through 2026Q2` printed certificate
    `fb8b38c2…` and anchor `anc_8874c61ae1fd136b9b64ed92deb763c9`, equal to `expected.toml`'s last row.
- **E833** Cell number formats on the 18 registered workbooks (NEW-cell.S1, bd-jg43.1; FACT, execution,
  2026-10-30).
  - **Oracle.** `scripts/oracle/formats.py` (zipfile and ElementTree, committed at 345e2ba before any Rust) found
    10 formats, 30,388 numeric cells (E820's total) and 6,256 non-integers. That equals the bead's table, which
    was written before either reader.
  - **Rust.** `Workbook::format_census` (7f68f82) equals the oracle on 18 of 18 files and on the summary, with
    lines compared whole (`test_format_census_matches_independent_reader`, run locally).
  - **Parity.** E820 parity holds: every census count and numeric digest is unchanged on the 18 files, and the
    synthetic census pinned at 345e2ba is unchanged.
  - **Planted negative.** Format codes left entity-encoded failed all 18 files; the change was reverted.
  - **Same origin.** The agent that wrote the Rust also wrote the oracle, though first. It counts once.
- **E834** Displayed values on the 18 registered workbooks (NEW-cell.S2, bd-jg43.2; FACT, execution,
  2026-10-30).
  - **Oracle.** `formats.py --displayed` was committed at 58d5c98, before the Rust. It shows 27,367 numbers and 2,873
    zero tokens, and refuses 148 cells (the General non-integers). These sum to E820's 30,388.
  - **Rust.** `Workbook::displayed_census` (5bfe46c) equals the oracle's digest and counts on 18 of 18 files.
  - **Loulo-Gounkoto.** In Q1 2026, K8 shows −38, K10 132 and K17 133. In Q2 2026's own sheet, K8 shows −8,
    K10 164 and K17 194. On displayed values the E823 footing closes in integers: 241 − 38 − 38 − 33 = 132.
  - **Planted negative.** Percentages left unscaled failed 10 of 18 files; the change was reverted.
  - **Hypotheses.** Section choice by stored sign, 15 significant digits first, and half away from zero are
    shared hypotheses of both readers. `scripts/oracle/as_shown_check.sh` (LibreOffice "as shown") decides
    them. It has not run: LibreOffice is on neither the owner machine nor an rch worker, and the `as-shown`
    CI job runs after a push. Until then, E834 equates two readers of one stated rule, not the rule with a
    renderer.
- **E835** Quarterly vintage registration, rehearsed on Q2 2026 (NEW-vintage.S2, bd-s9j3.2; FACT, execution,
  2026-10-30, owner machine, commit a211446).
  - **Oracle.** `fidelity.py` reproduces E820 on the 18 files: 101 visible and 6 hidden sheets; 30,388 numeric,
    3,952 placeholder, 1,092 error and 6,537 text cells. Run by the script's oracle step, `census2.py`
    reproduces E822: 2,595 instances, with i 2120, ii 3, iii 4, iv 17, v 391 and np 60.
  - **Dry run.** `scripts/register_vintage.sh --dry-run` was run on the two live Q2 2026 URLs. Its rows equal
    the registered rows field by field, except the Mine Stats `url_basis` ("fetched and verified 2026-10-30"
    against "W1 table").
  - **Run.** `--run 2026Q1` on that directory, with a scratch blind record of a local release binary
    (sha256 `c479c16e…`):
    - the seal was intact, and the oracle's time (06:11:41Z) preceded both fmine runs (06:12:04Z);
    - both Q2 2026 files' census equal the oracle's;
    - `root_all` is `fb8b38c2…`, and the `--through 2026Q1` root is `b6d5fa80…`;
    - Q2 2026 adds 320 instances (2,595 − 2,275), equal to the oracle's per-file count for
      `Barrick_Q2_2026_AISC.xlsx`.
  - **Not yet.** The real blind record (bd-s9j3.1) needs a push, which is the owner's decision. `--run` reads it
    from `repro/barrick/blind_record.json` (`binary_sha256`).
- **E836** Appendix K.4's cheapest common test (bd-prmn.8; FACT, execution, 2026-10-30, owner machine; oracle
  4acf3ce, Rust 3a73653).
  - **What ran.** The one-cell cost-of-sales-per-ounce check, 1000·s·N/Q against the printed rate on exact
    corners. It ran on every registered-subject column of the 34 visible AISC tables through 2026Q2. The basis
    is the registered, label-confirmed share where an NCI line is non-zero, and 100% otherwise.
    `scripts/oracle/one_cell_cos.py` was written and committed first. On all 519 columns,
    `fmine_public::aisc::one_cell_checks` equals it field by field (ignored test
    `barrick_one_cell_matches_independent_reader`). Both were written by the same agent, so this is one reader
    of one stated rule.
  - **Counts.** 374 pass, 82 fail and 63 refused (51 regions needing member weighting, 12 all-zero columns).
    The 82 failures are exactly the 82 cost-of-sales-per-ounce (iv).
  - **K.4 items.** 17 failing columns are K.4 items: Veladero 12 (all dated 2024 or 2025; it passes in 15 of 27
    columns, including every 2023 and 2026 column); Hemlo 3; North Mara 2 (Q2 2026 and YTD Q2 2026, failing at
    both 21/25 and 100%). The three 2023 Latin America & Asia Pacific region columns are refused for member
    weighting, and fail at 100%.
  - **Verdict: basis rule, not site-specific.** 65 failing columns are outside the K.4 items:
    - 63 are share-scaled NCI columns that fail at both their share and 100%: Carlin 25, Nevada Gold Mines 23,
      Pueblo Viejo 5, Cortez 3, Bulyanhulu 3, Turquoise Ridge 2, North Mara 2. With North Mara's two K.4
      columns, they are E826's 65 stated-share misses.
    - Kibali has 2 at 100%.
  - **A shared period.** The Q4 2025 and 2025 (FY) sheets fail for six subjects together: Carlin, Cortez,
    Nevada Gold Mines, Hemlo, Veladero and Kibali (17 of 42 columns on those sheets). Every Hemlo and Kibali
    failure sits there, so their (iv) is a period-wide cost or ounce basis, not a site fact. Their registered
    missing fact now says so (it was E817's "the attributable ounces sold used for the rate"), and Veladero's
    cites E836. States and totals are unchanged. The root moved from `fb8b38c2…` to `ca290d03…`
    (`repro/barrick/expected.toml`, row appended, observed from the binary of 3a73653).
  - **Next research hour.** The Q4 2025 release's basis change, read across the six subjects. Then the Nevada
    (61.5%) share rule, which fails for Carlin in 25 of 34 columns and for NGM in 23 of 34.
- **E837** The Q4 2025 one-cell failures have no single cause, and no issuer statement closes them (bd-10om;
  FACT, documents and arithmetic, 2026-10-30, owner machine; null result).
  - **What was read.**
    - `Barrick_Q4_2025_MD-A.pdf`, year-end 2025:
      - the per-segment reconciliation, three months to 12/31/25, p. 61, with footnotes g ("cost of sales …
        divided by ounces sold (both on an attributable basis using Barrick's ownership share)") and h ("may not
        calculate … due to rounding");
      - the consolidated reconciliation and its footnotes a–i;
      - "Other Mines – Gold", p. 37, footnote d: Hemlo was sold on 2025-11-26, with results up to closing;
      - Veladero's Q4 narrative.
    - `Barrick_Q4_2025_Mine_Stats.xlsx`: sales and cost of sales per ounce, `Minestats1!N36`/`N38` (Hemlo 27,
      1,738) and `Minestats2!B28`/`B30` (Veladero 47, 1,526).
    - Every one of these documents prints the AISC workbook's operands. `Barrick_Q1_2026_AISC.xlsx` reprints the
      Q4 2025 and 2025 sheets unchanged.
    - No note names an ounce or cost basis for the per-ounce rates other than footnote g. The one Q4 2025
      adjustment the notes do name is Loulo-Gounkoto's $283 million fair-value inventory removal, which concerns
      none of the six subjects.
  - **The failures split three ways.** The arithmetic is exact, with each printed figure x read as x ± ½.
    - **Hemlo and Veladero: the rates use fewer ounces than the "ounces sold" line.**
      - Veladero Q4 2025. Cost of sales 67, total cash costs 39 and AISC 84 against 1,526, 886 and 1,915 $/oz
        imply ounces in [43.564, 44.248], [43.429, 44.608] and [43.592, 44.137]. The common range
        [43.592, 44.137] misses the printed 47.
      - Hemlo Q4 2025. 44, 43 and 51 against 1,738, 1,707 and 1,976 imply [25.022, 25.612], [24.890, 25.491]
        and [25.550, 26.069]. That is just short of a common count, and far from the printed 27.
      - The rates themselves are not in doubt. The narrative's "COS/oz and TCC/oz in Q4 2025 were both 13% higher"
        than Q3 matches the printed rates (1,352 to 1,526, 787 to 886); the operands give 67/47 ≈ 1,426.
      - A Q4-only exclusion fits both the quarter and the year, under the ledger's posing (E836):
        - Veladero: Q4 gap [2.25, 3.94] koz, FY gap [1.07, 3.03], overlap [2.25, 3.03];
        - Hemlo: Q4 [0.89, 2.48], FY [1.06, 2.67], overlap [1.06, 2.48].
      - Ounces produced (Hemlo 26, Veladero 48; FY 123 and 230) close only Hemlo's Q4 cost of sales, so
        production is not the rule.
    - **Carlin, Cortez and Nevada Gold Mines: the Nevada share rule, not a Q4 item.** At 1000·s·N/Q, the six
      columns need s in:
      - Q4 2025: Carlin [0.61020, 0.61439], Cortez [0.60661, 0.61319], NGM [0.60981, 0.61192];
      - FY 2025: Carlin [0.61182, 0.61340], Cortez [0.61222, 0.61444], NGM [0.61273, 0.61362].
      None contains the registered 0.615. These columns are part of E826's and E836's Nevada misses, not of the
      period.
    - **Kibali Q4 2025 (printed twice): the opposite direction, at 100%.** 123 and 78 against 1,557 need a
      factor in [0.97675, 0.99807]: the printed ounces are 0.15–1.84 koz below what the rate implies.
  - **Verdict.** No single rule closes the 17 columns, and none is stated. The registered missing facts
    stand, and no state, registry row or root changes. E836's "a shared period" is a coincidence of three
    effects; only Hemlo's and Veladero's is specific to Q4 2025.
  - **What would decide it.** A Hemlo and Veladero ounce quantity, of about 1–2.5 koz and 2.25–3 koz, excluded
    from the per-ounce denominator in Q4 2025 only. Look for it in the Q4 2025 financial statements' inventory
    or revenue notes (for example, ounces delivered under a sale agreement or provisionally priced), or ask
    investor relations. Hemlo's sale on 2025-11-26 is one candidate for Hemlo. Nothing in the documents read
    suggests one for Veladero.
- **E838** The identity ledger on displayed values (NEW-cell.S3, bd-jg43.3, BD-331.S5; FACT, execution,
  2026-10-30, owner machine).
  - **Change.** The declared construction sums the numbers as printed, so each numeric operand now enters as its
    displayed value (E834's interpreter), never as the stored double. A whole displayed number enters as itself,
    and a zero section's dash as 0. An operand with no whole displayed number refuses its instance, naming the
    cell and the format code: a General non-integer, decimals or a percentage shown, or a refused format.
  - **Real files.** Exactly E823's 12 Loulo-Gounkoto instances move, all from (v) to (i).
    - They are column K of `Barrick_Q1_2026_AISC.xlsx` `'Q1 2026'` and of `Barrick_Q2_2026_AISC.xlsx`
      `'Q2 2026'` and `'Q1 2026'`, in the TCC and AISC footings and rates.
    - No other instance record changes, operands included. The hidden scope, the sources and the Mine Stats
      sections are equal.
    - Totals are i 2,395, ii 3, iii 4, iv 82, v 51 and np 60. The only (v) left are the 51 regional rates.
    - The alternative construction accepts 1,014 footings and 1,263 rates (1,008 and 1,257 before).
  - **Census.** `census2.py --displayed` reads each cell through `formats.py --displayed`, which was committed
    before the Rust interpreter.
    - Footing: i 1,010, iii 4, np 24.
    - Per ounce: i 1,122, ii 3, iv 17, v 379, np 36. The 379 are the stated-share census's input (E826).
    - Alternative construction: 1,014 and 1,000.
    - `test_e823_twelve_instances_leave_v` runs the census and finds all 2,216 instances it decides equal to
      the Rust states.
  - **Same origin.** The `--displayed` mode was written from the bead's stated rule, but after the Rust change
    had run on the corpus, by the agent that wrote both. It counts once with the Rust. The text it reads is
    E834's, whose hypotheses still await the LibreOffice cross-check.
  - **Planted negative.** Reading a zero token as 1 instead of 0 failed the census comparison and the
    zero-token unit test. The change was reverted.
  - **Root.** The `M0AS` frame v5 also binds each file's displayed digest and counts. The run's root is
    `e95d0b238954c56b21c5977b55de0b988517a500600bbe76d4a878d7d47393a6`; its `repro/barrick/expected.toml` row
    names the code commit.
  - **Synthetic.** Column G, 100.4 stored under `#,##0`, is scored on 100: its five (v) become (i). The new
    column N, "Site G", re-seeds the refusals. A General non-integer at N6 refuses the TCC footing, and
    date-format ounces at N13 refuse the three rates; each names the cell and the format code. The lane now has
    55 instances: i 40, ii 2, iii 1, iv 2, v 5, np 5.
- **E839** Latin America & Asia Pacific 2023 cost of sales per ounce against its members (bd-4wvl.6, BD-340.S6;
  FACT, execution, 2026-10-30, owner machine).
  - **Cells.** Two files: `Barrick_Q3_2024_AISC.xlsx` (sha256 `d063ea7c…`), sheets `'Q3 2023'` and
    `'YTD Sept 30 2023'`; and `Barrick_Q4_2024_AISC.xlsx` (`f8426969…`), sheet `'YTD Dec 31 2023'`. Rows: 19
    (attributable ounces sold) and 20 (cost of sales per ounce, footnotes h and i). Columns: K (Pueblo Viejo), L
    (Veladero) and M (the region), headers in row 4.
    - The three rates are E817's "Latin America & Asia Pacific 2023 ×3". The ledger's 2023 LAAP instances refused
      as `regional_numerator_needs_weighting` are exactly M20 on these sheets.
  - **Two readers before commit.**
    - Reader 1: `formats.py`.
    - Reader 2: a second agent's own zipfile/XML reader, given only the locators.
    - All 18 displayed values, both sha256 and the labels agree. The ignored test reads them a third time through
      `fmine-io`: 18 of 18.
  - **Result.**
    - The members' ounces foot exactly on all three sheets.
    - Each region rate is refuted by the McCormick relaxation of `rate·Σwᵢ = Σ rateᵢ·wᵢ`. Each Farkas
      certificate is checked over the envelope rows.
    - The lower end of the printed region rate lies about 13 to 17 $/oz above the highest weighted mean the
      members' rounding intervals allow. Example: Q3 2023 at 1,467.5 against at most about 1,454.6 (INFERENCE,
      hand arithmetic).
  - **Not claimed.**
    - Refuted means the printed cells contradict "the region is these two members, weighted by attributable
      ounces sold". It does not say which cell is wrong, and gives no corrected value: a relaxation carries no
      blame.
    - The region may carry a cost or a site the table does not show, as E816 found for Africa & Middle East
      (hypothesis, not tested).
    - Mapping the result to a ledger state is BD-331.S4's job.
  - **Tests.** `test:fmine-public::laap::test_laap_2023_fixture`, plus the ignored
    `test_laap_2023_fixture_cells_match_registered_bytes`.
  - **Planted negative.** Q3 2023 M20 set to 1,454 gives `NotRefuted`, and the bytes test names the cell. The
    value was reverted.
- **E840** Regional cost of sales per ounce decided against registered members (BD-331.S4, bd-prmn.4, the
  region map of bd-gwf1.1; FACT, execution, 2026-10-30, owner machine).
  - **Data.** The region map is registry data: 13 `[[region_member]]` rows in `registries/sources/barrick.toml`,
    keyed by the AISC file's release quarter (W1 §13 rule 2), with "South America & Asia Pacific" registered
    from 2025Q4 (Q4 2025 Mine Stats `QTD!A33`). The parser refuses an unknown region or member, a region as a
    member, a bad quarter, `last` before `first`, and one member over overlapping releases.
  - **Rule.** A region's cost-of-sales rate on a sheet with the attributable-basis footnote and a non-zero NCI
    line is weighed against its printed members by attributable ounces sold. The rule is region_census.py's
    rules 2 to 7 (57a5aa5), with one difference: the McCormick relaxation of `rate·Σwᵢ = Σ rateᵢ·wᵢ` replaces
    its exact vertex check. A checked certificate gives (iv) `region_outside_members`. No refutation gives (i)
    with `relaxation_only`: no conflict is shown, and no value is proved.
  - **Independent reader.** `scripts/oracle/region_census.py` was committed before any of this Rust. A subagent
    wrote it from a rule statement, without reading the crate. The rule statement and the Rust share one origin
    (this agent), so agreement checks the implementation, not the choice of rule.
  - **Result.**
    - The ignored test `barrick_regions_match_region_census` runs the census CLI. All 51 visible and 3 hidden
      instances equal it on verdict, reason, cell and the used, absent and excluded members.
    - Visible: 41 (i) and 10 (iv). Hidden: 2 (i) and 1 (iv). On this corpus the relaxation refutes exactly the
      set the exact check refutes.
    - The visible cost-of-sales-per-ounce counts move from 374 (i), 82 (iv), 51 (v) to 415 (i), 92 (iv), 0
      (v). No visible instance of any family is (v) any more.
    - E839's three LAAP 2023 rates are among the ten.
  - **Pattern (INFERENCE, from the census's `--values` run on the owner machine; values not printed).**
    - Nine of the eleven refuted regions have a member that is not weighed. In five, Loulo-Gounkoto (four Africa
      & Middle East cells, 2025) or Porgera (LAAP, Q1 2024) prints zero-section tokens. In four, Porgera is
      absent (LAAP 2023, one of them hidden). All nine print above every weighted mean the members allow.
    - Every region with a member not weighed is refuted, 9 of 9. Of the 45 with every member weighed, 2 are
      refuted: LAAP Q2 2024, the same period in the Q3 2024 and Q2 2025 files, both below.
  - **Not claimed.**
    - A refutation blames no cell and gives no corrected value.
    - Hypothesis, not tested: the region's cost of sales carries costs of a member that sold nothing or is not
      printed. The two Q2 2024 LAAP refutations are unexplained.
    - The 41 (i) are also consistent under the rounded-once alternative. That is the kernel's result; no
      independent reader computes it.
  - **Tests.** Synthetic, default suite:
    - `test_region_weighed_against_members`: not refuted at 2099 to 2101, refuted at 2098 and 2102, where the
      exact bound is [2099⅙, 2100⅚].
    - `test_region_excludes_and_names_absent` and `test_region_refusals`: each refusal names its cell.
    - Updated real-file tests: share census, hidden scope, one-cell, every (iv), and the totals.
  - **Planted negatives, each reverted.**
    - Weighing a member whose ounces print zero failed two synthetic tests. The real-file comparison passed: on
      this corpus every excluded member prints tokens, not zeros.
    - Not naming absent members failed a synthetic test and the real-file comparison.
  - **Root.** The state root moves to `f1f58188…`, observed from the release binary of 360cc39 (exit 0, empty
    stderr); `repro/barrick/expected.toml` has the row. The registry epoch hashes the file rows only, so it does
    not move (360cc39's commit message says it does; that was wrong).
  - **Verification.** Self-verified on this machine only; nothing has run in CI.
