# B6 — Operations Twin (Headframe), Processing & Metal Accounting (Crucible), Tailings & Closure (Freeboard), and Workforce & Critical Controls (Muster)

- RUN_ID: `fm-2026-10-29-b` · Pass: **B6** (closes open audit item **H-2**, plan §36.4, and bead **BD-246**)
- Pivotal questions: **B6-SQ1** (Headframe, §16), **B6-SQ2** (Crucible, §17), **B6-SQ3** (Freeboard, §18), **B6-SQ4** (Muster, §19)
- Inputs:
  - `planning/mor/MOR_CASE.md` (`E001–E035` + corpus blocks `E101–E399`);
  - `planning/research/R1_frankensim_survey.md`, `R2_runtime_and_substrate_survey.md`, `R3_numerics_ml_perception_survey.md`, `R4_accounting_fit.md`, `R5_textiles_patterns.md`, `R6_industry_landscape.md`, `R7_technical_methods_sota.md`, `R8_numerics_symbolic_stack.md`;
  - `planning/corpus/C1_barrick_public_stats.md` through `C6_textbook_techniques.md`;
  - `planning/brenner/B1_subsurface_representation.md` through `B5_wedge_sequencing_premortem.md`.
- Labels: **FACT** (verified in code or a cited source), **FACT[U]** (literature recall not yet pinned to a primary page), **INFERENCE** (derived by exact arithmetic from cited facts), **DESIGN** (proposed architecture), **HYPOTHESIS** (testable belief with a falsifier), **TARGET** (measurable gate threshold), **OPEN** (unresolved).

---

## 0. Verdict in one screen

| Sub-question | Hypothesis | Verdict | Role in the final design |
|---|---|---|---|
| **B6-SQ1 · Headframe (§16)** | **H1-a** Real-time closed-loop OT control & FMS dispatch replacement | **KILL** | Violates `INV-011` / `NONGOAL-002` (no write conduit to ISA-95 Levels 0–2) and duplicates OEM FMS safety locks (Komatsu FrontRunner, Modular DISPATCH) |
| B6-SQ1 | **H1-b** Continuum/particle physics everywhere (3D DEM muckpiles, 3D CFD airways, 3D FEM paste walls per shift) | **KILL as shift engine** | `frankensim` lacks DEM and Mohr–Coulomb/Hoek–Brown (`E007`, `E010`); unidentifiable parameters manufacture false precision (Loss #1). Survives only as ingested `AttestedInterpretation` external studies |
| B6-SQ1 | **H1-c** Passive BI / historian OEE mirror (shift tonnage & availability dashboards) | **DOWNGRADE** | Kept as the `R0` parity baseline. Cannot price bottleneck duals, AHS geo-fence interaction delays, or dry-season power curtailment; fails pre-mortem `F6` |
| B6-SQ1 | **H1-d** Epoch-anchored event ledger + `LabRuntime` DES + validity-boxed `CorrelationCard`s + `EnergyEnvelope` | **SURVIVE, amended** | Core architecture (`fmine-ops`, `fmine-blast`), amended with mandatory analytic bounds (`fs-tropical` + finite-source queueing) beside every DES run |
| B6-SQ1 | **H1-e** *(Forced alternative)* Vendor-twin & OEM predictive-maintenance referee ledger | **SURVIVE, merged into H1-d** | Every OEM/AI alert (Avathon, MineStar Health, Modular Predict) is ingested as a frozen forecast and scored against SAP PM / downtime truth (`GATE-HEAD-03`) |
| **B6-SQ2 · Crucible (§17)** | **H2-a** Deterministic block-level recovery formulas $R(g)$ as plant authority | **KILL as authority** | Fails support rules (`G-CRUC-2`), ignores Jensen's inequality $\frac{1}{2}(gR)''\sigma_g^2$, ignores 16–36 h CIL/POX residence hold-up (`E170`), and ignores card versioning (`E181`). Survives as `DeclaredRecoveryCard` parity |
| B6-SQ2 | **H2-b** Full mechanistic multiphase CFD-DEM / population-balance plant twin (Geminex-class) | **KILL** | Lihir Geminex was technically deployed without operational commissioning (`B5 W9`); unmeasured slurry chemistry drift invalidates mechanistic constants weekly |
| B6-SQ2 | **H2-c** Four-layer validity-boxed `CorrelationCard`s + $N$-tank CSTR / 3D-heap RTD deconvolution + AMIRA P754 bilinear WLS/Bayesian reconciliation | **SURVIVE, adopted** | Core of `fmine-plant`: separates book recovery `R_card` (`Veladero` `E275`) from posterior `R_inf` (`GB-20`), deconvolves shift hold-up, and closes mine-to-market balances |
| B6-SQ2 | **H2-d** *(Forced alternative)* Check-in vs Check-out Divergence & Smelter/Refinery Outturn Arbitrage Engine | **SURVIVE, merged into H2-c** | Tracks `MCF_out - MCF_in` (`Kibali` +2.6% `E210`), detects shared-sensor tautologies (`Loulo` `C3 §3.12`), and optimizes concentrate lot blending & umpire splitting-limit decisions (Card **B6-G2**) |
| **B6-SQ3 · Freeboard (§18)** | **H3-a** Live "TSF Safety Score / Traffic-Light FoS Dashboard" | **KILL unconditionally** | Violates `INV-002` (`geotechnical_signoff = false`) and `G-FREE-1`. InSAR is blind to brittle static liquefaction (`E031`), and stale sensors / open manual gates (`Veladero` `E282`) make "GREEN/SAFE" lethal |
| B6-SQ3 | **H3-b** Static document repository (SharePoint for GISTM / ESIA PDFs) | **KILL** | Fails Jagersfontein (2019–2022) and the corpus ESIA checks (chemical-basis conversion `E322–E324`, certificate-to-table completeness `E325–E328`, unmapped stakeholder concerns `E331–E332`) |
| B6-SQ3 | **H3-c** Three-valued TARP obligation automaton + generalized `RequirementSet` / `EsCommitment` matrix + coupled $\bar{B}$ / slice-Jacobian & sub-aqueous PAG co-disposal twin | **SURVIVE, adopted** | Core of `fmine-tailings`: `{NotTriggered, Triggered, Indeterminate}` where `Indeterminate` escalates like `Triggered` (`PO-FREE-1`, `FMINE-REF-404`) |
| B6-SQ3 | **H3-d** *(Forced alternative)* Three-Measurand Closure & Perpetual-Liability Real-Option Engine | **SURVIVE, merged into H3-c** | Holds `RegulatorBond`, `IfrsProvision_IAS37`, and `StochasticClosureEstimate` side by side (`Carlin` 2.7× ratio `E188`), enforces hazardous-inventory completeness (`Veladero` Hg `E285`), and prices progressive rehab |
| **B6-SQ4 · Muster (§19)** | **H4-a** Individual operator leaderboards & biometric/camera disciplinary scoring | **KILL** | Violates privacy/works-council law (`G-MUST-1`, `G-MUST-5`) and confounds geology/haul/equipment variance with operator skill (`G-MUST-2`) |
| B6-SQ4 | **H4-b** Cost-minimizing roster LP that trades fatigue/overtime against mill-hour dollars | **KILL** | Violates `G-MUST-4` and `G-PUSH-1`: statutory hours, rest, and fatigue ceilings are hard constraints, never priced (`GB-23`) |
| B6-SQ4 | **H4-c** Hard competency/roster feasibility + covariate-adjusted hierarchical productivity + DiD intervention scoring | **SURVIVE, amended** | Core of `fmine-workforce`, amended by merging **H4-d** |
| B6-SQ4 | **H4-d** *(Forced alternative)* Goodhart-Resistant Critical Control Verification (CCV) Forensic Engine | **SURVIVE, merged into H4-c** | Models CCV field verifications (a published Barrick scorecard metric, `E025`) via Beta-Binomial capture-recapture and answer-entropy bounds so "300 checks, 0 non-conformances" triggers an audit (`OB-REVIEW`, Card **B6-G4**) |

---

## 1. B6-SQ1 — Headframe (§16): Operating Twin, Blasting, Haulage/AHS, Underground & Energy

### 1.1 Framing and loss-function alignment

An operating mine twin faces two opposite failure modes:
1. **The "control-room illusion" (Loss #1 + OT hazard):** pretending that an enterprise software layer can or should actuate autonomous haul trucks, ventilation regulators, or blast initiation, or that uncalibrated 3D continuum mechanics (DEM/CFD/FEM) run on sparse blasthole logs represent physical truth.
2. **The "passive historian" trap (Loss #3 + Pre-mortem F6):** re-plotting Pitram or DISPATCH shift tonnes on a new screen without connecting operational variance to subsurface beliefs (`Strata`), plant recovery/throughput (`Crucible`), or bottleneck shadow prices (`Pushback`).

**Corpus reality check (`C2`, `C3`, `C4`):**
- Barrick operates **23 Komatsu 930E-5 AHS trucks at Cortez** and **40 at Carlin** (`Carlin_Complex_TechnicalReport_14032025.pdf` p. 151, FACT).
- Underground paste backfill is **non-monotone in time**: Loulo paste at 3.5% binder drops from **2.65 MPa at 56 d to 1.10 MPa at 112 d**, and Kibali's legacy slag-cement fill degraded (`E233, E234`, FACT).
- Hoist nameplate formulas overstate capacity unless all utilization and fill factors are explicit: Kibali's $34\text{ skips/h} \times 14.8\text{ t} \times 8{,}760\text{ h} \times 0.90 = 3.97\text{ Mt/yr}$ vs stated **3.2 Mtpa** implies a further **0.806 effective hoist factor** (`E231, E232`, INFERENCE).
- At **Lumwana** (expanding 27 $\to$ 52 Mtpa) and **Kibali** (3 run-of-river hydro plants + solar + 2 BESS + 36 MW diesel peaking), **electrical active/reactive power (`MW` / `MVAr`) is a primary operational constraint** (`Lumwana_Site_Visit_25092025.pdf` pp. 29–30; `Kibali` p. 281, FACT).

### 1.2 Hypotheses (refuter-first)

#### H1-a — Real-time closed-loop OT control & FMS replacement
- **Mechanism.** Ingest sub-second telemetry from trucks, shovels, drills, and fans; solve real-time dispatch MILPs and ventilation set-points; write commands directly to FrontRunner, DISPATCH, and PLC/SCADA endpoints.
- **Falsifier.** Any deployment review by Barrick OT security or OEM safety engineering blocks go-live because external write paths into AHS safety envelopes or underground fan PLCs invalidate functional safety certifications (ISO 13849 / IEC 61508 / IEC 62443).
- **Verdict:** **KILL.** Enforced by `INV-011`, `NONGOAL-002`, `G-HEAD-1`, and `PO-SEC-4`.

#### H1-b — High-fidelity continuum/particle physics everywhere (3D DEM, 3D CFD, 3D FEM per shift)
- **Mechanism.** Simulate every blast muckpile with 3D Discrete Element Method (DEM), every underground heading with 3D turbulent CFD, and every paste-filled stope wall with 3D elasto-plastic FEM.
- **Falsifier.** On a real mine bench or stope, parameter uncertainty (joint spacing, local moisture, explosive velocity-of-detonation variance, paste segregation) dominates solver discretization error by $>10\times$, while 3D DEM/FEM violates shift compute budgets and lacks in-universe crates (`E007`, `E010`).
- **Verdict:** **KILL as the shift engine.** Keep external numerical analyses as `AttestedInterpretation` imports with declared validity domains.

#### H1-c — Passive BI / historian OEE mirror
- **Mechanism.** Mirror FMS and historian tables into standard GMG time-usage categories and OEE charts.
- **Falsifier.** Executive and site-GM reviews rate the output "nothing Pitram and PowerBI don't already show" (Pre-mortem `F6`).
- **Verdict:** **DOWNGRADE to R0 parity baseline.** Mandatory for data hygiene (`G-HEAD-2` enforces GMG calendar-time basis), insufficient as a product.

#### H1-d — Epoch-anchored event ledger + `LabRuntime` DES + validity-boxed `CorrelationCard`s + `EnergyEnvelope`
- **Mechanism.**
  1. **As-built depletion & survey epochs (`fmine-ops::survey`):** LiDAR/drone/CMS solids update epoch depletion with certified volume intervals (`fs-geocon` pattern) and conservative cell remap (`fs-mesh::conservative_cell_remap`) so block-to-dig-polygon mass is conserved to machine precision.
  2. **Blasthole MWD & blast movement (`fmine-blast`):** MWD specific energy feeds `Strata` hardness via rig-calibrated `CorrelationCard`s; Kuz-Ram ($19/30$ exponent, `H.2 #6`) + Swebrec predict PSD; blast-movement vectors + mixing kernel shift dig polygons *before* ore/waste boundary evaluation (`GB-18`).
  3. **Non-monotone backfill strength guard (`G-HEAD-7` / `FMINE-REF-504`):** `FillStrength(t)` tracks both early-age hydration cure and late-age sulfate/pyrrhotite degradation (`Loulo` `E233`, `Kibali` `E234`); adjacent stope ring-blasting is refused if evaluated strength falls below `UCS_design(wall_height, span, n_exposures)`.
  4. **AHS token & safety-bubble DES (`fmine-ops::haul`):** Runs on deterministic `asupersync::LabRuntime` virtual time, explicitly modelling intersection reservation tokens, single/double-side shovel spotting locks, and manned light-vehicle geo-fence slow/stop bubbles ($R_{\text{slow}}, R_{\text{stop}}$), cross-checked against finite-source queueing (`GB-17`) and `fs-tropical` max-plus cycle bounds.
  5. **Mine-to-Mill `EnergyEnvelope` (`fmine-ops::energy`):** Couples active power tiers (hydro/solar $\to$ wheeled grid $\to$ BESS $\to$ thermal diesel) and reactive stability (`STATCOM` / `MVAr`) to comminution hardness, POX oxygen demand, hoisting, and dewatering.
- **Falsifier.** Across 12 months of pilot shift data, `LabRuntime` DES with AHS token/bubble states fails to beat a static match-factor formula on shift-production 80% interval coverage (`GATE-HEAD-02`), or MWD-conditioned hardness adds zero skill to SAG throughput forecasts.
- **Discriminating test (`T6-1`).** Replay 100 shifts of mixed AHS + light-vehicle interaction on `LabRuntime` under two models: (i) classical M/M/1/N finite-source queue (`GB-17`) and (ii) token + geo-fence bubble DES. Measure shift tonnage P10/P50/P90 accuracy and attribution of delay hours.
- **Verdict:** **SURVIVE, amended** (every DES output must display the analytic queueing/max-plus bound alongside the simulated interval).

#### H1-e — *(Forced third alternative)* Vendor-twin & OEM predictive-maintenance referee ledger
- **Mechanism.** Treat every external predictive alert (Avathon, MineStar Health, Modular Predict, tire TKPH monitors) and every RL/heuristic dispatch recommendation as a **frozen forecast** (`Claim` in `CELL-QUAR`) with asset ID, failure mode, lead-time window, and probability. Score precision, recall, lead-time calibration, and avoided-downtime dollar value (priced at `Pushback`'s bottleneck dual) when SAP PM work orders and component tear-downs close (`PO-HEAD-5`, `GATE-HEAD-03`).
- **Why this sits alongside existing platforms (Pre-mortem F1):** FrankenMining does not compete with AI-platform or OEM analytics; it scores every alert source the same way, recording whether each alert preceded the failure it predicted or was a false alarm.
- **Verdict:** **SURVIVE, merged into H1-d.**

### 1.3 Galaxy-brain card B6-G1 · AHS Geo-Fence Bubble Contagion vs. Match Factor

- **Equation.**
  - Classical match factor: $\text{MF} = \frac{N_{\text{trucks}} \cdot t_{\text{load}}}{N_{\text{loaders}} \cdot t_{\text{cycle},0}}$.
  - With manned auxiliary-vehicle geo-fence intrusions at rate $\lambda_{\text{aux}}$ (intrusions/h) along a haul segment of density $\rho_{\text{trucks}}$ (trucks/km), each intrusion of duration $\tau_{\text{intr}}$ and bubble radius $R_{\text{stop}}$ stalls an expected $k = 2 R_{\text{stop}} \rho_{\text{trucks}}$ autonomous trucks and induces a synchronization bunching tail $\Delta t_{\text{bunch}} = \frac{1}{2} k \cdot t_{\text{load}}$ at the shovel queue:
    $$t_{\text{cycle,eff}} = t_{\text{cycle},0} + \lambda_{\text{aux}} p_{\text{hit}} \left(\tau_{\text{intr}} + \frac{\sigma_{\text{intr}}^2}{2\tau_{\text{intr}}} + \frac{k \cdot t_{\text{load}}}{2}\right)$$
- **Substituted (Carlin/Cortez-scale 20-truck AHS fleet, 2 shovels, $t_{\text{load}} = 2.4\text{ min}$, $t_{\text{cycle},0} = 24.0\text{ min}$, $\text{MF}_0 = 1.00$):**
  - Suppose road-maintenance graders / light vehicles enter the active AHS ramp $\lambda_{\text{aux}} = 3\text{ times/h}$ for $\tau_{\text{intr}} = 3.0\text{ min}$ ($\sigma_{\text{intr}} = 2.0\text{ min}$), affecting $p_{\text{hit}} = 0.25$ of cycles with $k = 2.2$ trucks caught per bubble.
  - Direct stop delay per truck cycle is only $3 \times 0.25 \times 3.0 = 2.25\text{ min}$, **but** releasing 2.2 bunched trucks simultaneously into the shovel spotting lock adds $\frac{2.2 \times 2.4}{2} = 2.64\text{ min}$ of spotting-token queue per affected wave, raising $t_{\text{cycle,eff}}$ from $24.0\text{ min}$ to **$28.2\text{ min}$ ($+17.5\%$)** and dropping effective shovel utilization from $0.84$ to **$0.73$ ($-13.1\%$ shift tonnes)**.
- **Intuition.** In autonomous haulage, safety-envelope interactions do not merely subtract the stopped minutes; they **synchronize truck arrivals** into platoons that contend for single/double-side shovel spotting tokens. Adding a 21st truck without scheduling grader windows in off-peak shovel swaps increases bunching rather than ore tonnes.
- **Flip.** If double-sided spotting tokens are active and grader maintenance is locked to blast-clearance windows ($\lambda_{\text{aux}} \to 0$ during production hours), the platoon term vanishes and classical finite-source queueing (`GB-17`) governs within $2\%$.

---

## 2. B6-SQ2 — Crucible (§17): Processing, Geometallurgical Transfer, RTD & Metal Accounting

### 2.1 Hypotheses (refuter-first)

#### H2-a — Deterministic block-level recovery equations $R(g)$ as plant authority
- **Mechanism.** Apply the technical report's regression curve $R(g)$ to each block's kriged grade, sum recovered ounces across blocks, and treat deviations from actual doré/cathode as an unattributed "Mill Call Factor".
- **Falsifier.** Demonstrated false in the Barrick corpus:
  1. **Concave/convex nonlinearity (`Jensen's inequality`):** For Carlin Goldstrike Roaster, tail grade is bivariate in Au and As ($g_{\text{tail}} = 1.821\times 10^{-5}\text{As} + 0.127\text{Au} - 4.890\times 10^{-4}$, `E172`); evaluating recovery on smoothed block grades without within-blend variance correction $\frac{1}{2}(gR)''\sigma_g^2$ systematically biases predicted ounces.
  2. **Dynamic inventory smearing (`E170`):** Carlin had to discard shifts below 90% availability and their neighbours when fitting roaster recovery because 16–36 h CIL/POX residence time smears head-grade spikes across 2–4 shifts.
  3. **Multiple concurrent card versions (`E181`):** Turquoise Ridge's reserve table uses three different intercepts of the same recovery formula (`100.79`, `97.14`, `101.79`) plus a `+3%` plant adjustment.
- **Verdict:** **KILL as authority.** Survives as `DeclaredRecoveryCard` for `GATE-BULL-01` / `GATE-HALL-01` reproduction parity.

#### H2-b — Mechanistic multiphase CFD-DEM / population-balance plant twin
- **Mechanism.** Fit a high-dimensional mechanistic simulator (breakage matrices, bubble-particle attachment efficiencies, autoclave mass-transfer coefficients) and run it as the online plant twin.
- **Falsifier.** Unmeasured feed mineralogy (clay swelling, soluble copper species, frother-active organics, Raman carbon maturity `E175`) shifts surface chemistry faster than lab breakage/flotation characterization can be re-run; Lihir's Geminex deployment produced zero published operational uplift (`B5 W9`).
- **Verdict:** **KILL.**

#### H2-c — Four-layer validity-boxed `CorrelationCard`s + $N$-tank CSTR / 3D-heap RTD deconvolution + AMIRA P754 bilinear reconciliation
- **Mechanism.**
  1. **Four-layer card architecture (`§17.3`):** (L1) test-work card with explicit unit AST (`input_units`, `output_scale`, `parse_check`, catching printed parenthesis/unit bugs like `E171`) and hyperrectangular + Mahalanobis validity box; (L2) attested stage operational factors (`Pueblo Viejo` `E256`); (L3) explicit caps and deleterious windows (`Zaldívar` 55% SCu cap `E290`; `NGM` `<40%` / `<60%` spike-test preg-rob caps `E173, E175`; `Carlin` autoclave S:CO3 windows); (L4) reserve-route vs operational-route tag (`E265`).
  2. **RTD deconvolution (`fmine-plant::rtd`):** Convolve shift feed with the $N$-tank CSTR cascade impulse response $h_{\text{CIL}}(\tau; \alpha_{\text{bypass}}, \beta_{\text{dead}}, Q_{\text{slurry}}(t))$ (`§17.3`) and 3D lift/cell heap percolation kernel $h_{\ell,c}(t)$ (`§20.8`) so transient Gold-in-Circuit ($\text{GIC}$) and heap solution hold-up are separated from metallurgical recovery performance.
  3. **AMIRA P754 bilinear WLS/Bayesian reconciliation (`§17.5`):** Reconcile dry mass, water, and per-element metal across the flowsheet graph with global $\chi^2$ and per-node `fs-eproc` drift monitors, separating primary (custody check-in/check-out) from secondary circuit accounting.
- **Falsifier.** Over 12 monthly closes at the pilot plant, RTD-deconvolved 4-layer cards fail to achieve 80% interval coverage in $[0.70, 0.90]$ (`GATE-CRUC-02`), or fail to improve out-of-sample shift/monthly recovery RMSE over naive unlagged shift ratios.
- **Discriminating test (`T6-2`).** On the 71 published Turquoise Ridge lab-vs-plant pairs (`E177`) plus synthetic CIL step-grade injections, compare (i) unlagged shift ratio, (ii) 90%-availability filtered regression (`E170`), and (iii) $N$-tank CSTR RTD state-space filter.
- **Verdict:** **SURVIVE, adopted.**

#### H2-d — *(Forced third alternative)* Check-in vs Check-out Divergence & Smelter/Refinery Outturn Arbitrage Engine
- **Mechanism.**
  1. **Dual-anchor plant MCF (`MCF_in` vs `MCF_out`):** Expose both `MCF_in` (reconstituted feed from tails + check-out) and `MCF_out` (bullion/concentrate check-out + $\Delta\text{GIC}$ over GC call). Their difference $\Delta_{\text{plant}} = \text{MCF}_{\text{out}} - \text{MCF}_{\text{in}}$ (`Kibali` 2025: $+2.6\%$ unaccounted gain, `E210`) isolates internal plant sampler/flowmeter bias from mine-to-mill `F2` variance. Automatically detect **tautological tonnage MCFs** where "mine delivered" and "mill milled" read the same weightometer/flowmeter (`Loulo` `C3 §3.12`) and mark tonnage bias `NotIdentifiable` (`FMINE-REF-104`).
  2. **Smelter/Refinery Outturn & Penalty Blending Engine (`fmine-plant::custody`):** Model concentrate lot blending and assay exchange (mine assay $y_M$, buyer assay $y_B$, splitting limit $\delta_{\text{split}}$, umpire cost $c_U$, umpire bias $\mu_U$, and tiered deleterious penalties for $\text{As}, \text{Sb}, \text{Bi}, \text{Hg}, \text{F}, \text{Cl}, \text{moisture}$) as an explicit Bayesian decision problem (Card **B6-G2**).
- **Verdict:** **SURVIVE, merged into H2-c.**

### 2.2 Galaxy-brain card B6-G2 · Concentrate Assay Exchange, Splitting Limits & Penalty Cliff Blending

- **Equation.**
  - Under a concentrate sales contract with splitting limit $\delta_S$ (e.g. $0.30\%\text{ Cu}$ or $1.0\text{ g/t Au}$), if $|y_M - y_B| \le \delta_S$, settlement uses the midpoint $\frac{1}{2}(y_M + y_B)$. If $|y_M - y_B| > \delta_S$ and a party calls umpire $y_U \sim \mathcal{N}(\mu_{\text{true}}, \sigma_U^2)$, settlement uses $y_U$ (or the party closer to $y_U$), and the party further from $y_U$ pays umpire fee $c_U$.
  - Expected net gain from calling umpire when $y_M - y_B = \Delta > \delta_S$, given posterior $y_{\text{true}} \sim \mathcal{N}(\hat{\mu}, \hat{\sigma}^2)$ from certified lot sampling (`Gy` FSE + duplicate precision, `§12.3`):
    $$\text{EV}_{\text{umpire}}(\Delta) = M_{\text{dry}}\, p_{\text{payable}}\left(\hat{\mu} - \frac{y_M + y_B}{2}\right) - c_U \cdot \Phi\!\left(\frac{\frac{1}{2}(y_M + y_B) - \hat{\mu}}{\sqrt{\hat{\sigma}^2 + \sigma_U^2}}\right)$$
  - Simultaneously, for a deleterious element $e$ (e.g. $\text{As}$ in Cu concentrate) with penalty threshold $\tau_{\text{pen}} = 0.20\%$ ($-\$25/\text{dmt}$ per $0.10\%$ above $0.20\%$) and outright rejection cliff $\tau_{\text{rej}} = 0.50\%$, blending two concentrate lots $A$ ($\text{As}_A = 0.38\%$) and $B$ ($\text{As}_B = 0.08\%$) in ratio $w : (1-w)$ to hit $\overline{\text{As}} = 0.19\% < \tau_{\text{pen}}$ eliminates the step penalty across both lots.
- **Substituted (10,000 dmt Cu concentrate shipment at $\$9{,}000/\text{t}$ payable Cu):**
  - Suppose $y_M = 28.40\%\text{ Cu}$, $y_B = 27.95\%\text{ Cu}$ ($\Delta = 0.45\% > \delta_S = 0.30\%$). Midpoint settlement loses $0.225\%\text{ Cu} = 22.5\text{ t Cu} \approx \$202{,}500$.
  - With P754-calibrated auto-sampler precision $\sigma_M = 0.08\%$ vs buyer grab-sample precision $\sigma_B = 0.22\%$, the inverse-variance posterior mean is $\hat{\mu} = 28.35\%\text{ Cu}$ ($\hat{\sigma} = 0.075\%$). Calling umpire ($c_U = \$4{,}000$, $\sigma_U = 0.09\%$) yields $\text{EV}_{\text{umpire}} = +\$155{,}000$ with $91\%$ win probability.
  - Conversely, if the mine's sampler had an uncalibrated $+0.35\%$ positive bias detected by `Touchstone` (`§20.5`), $\hat{\mu} = 28.02\%$, and calling umpire has **negative EV ($-\$48{,}000$)** — saving both umpire fees and commercial credibility.
- **Intuition.** P754 sampling precision and `Touchstone` bias posteriors pay for themselves directly at the concentrate settlement table: knowing whether $\Delta = y_M - y_B$ is buyer under-assay or mine sampler bias turns a blind commercial dispute into a positive-EV decision card.
- **Flip.** If mine sampler precision $\sigma_M > 0.20\%$ (uncalibrated manual spear sampling), $\hat{\mu}$ regresses toward the midpoint and the EV of calling umpire collapses toward zero.

---

## 3. B6-SQ3 — Freeboard (§18): Tailings, Water Balance, Geochemistry & Closure

### 3.1 Hypotheses (refuter-first)

#### H3-a — Live "TSF Safety Score / Traffic-Light FoS Dashboard"
- **Mechanism.** Combine piezometer levels, InSAR displacement rates, and limit-equilibrium FoS into a real-time green/amber/red "Safety Index" per facility.
- **Falsifier.**
  1. **Brittle undrained failure physics (`FACT, E031, Rana et al. 2024`):** Contractive saturated tailings can undergo static liquefaction with zero prior InSAR-detectable creep (Brumadinho 2019). A green InSAR badge on a contractive upstream raise is a textbook Loss #1 failure.
  2. **Stale/unmonitored paths (`FACT, E282`):** Veladero's 2015 solution release passed through an open diversion-channel gate when a valve failed; a dashboard that defaults unmonitored or stale manual gates to "normal" lies by omission.
  3. **Legal/regulatory doctrine (`INV-002`):** Only the Engineer of Record (EoR) and Accountable Executive sign geotechnical/GISTM assessments.
- **Verdict:** **KILL unconditionally.** Enforced by `INV-002`, `G-FREE-1` (the word "safe" never appears in system-authored text; document quotes like `"will safely store"` in `E267` render only as attributed `Assertion`s in quotes), and `G-FREE-2` (`FMINE-REF-404`).

#### H3-b — Passive document repository (SharePoint for GISTM / ESIA PDFs)
- **Mechanism.** Store PDF reports mapped to the 77 GISTM requirements.
- **Falsifier.** Shown on the held-back corpus ESIA notes (`C5`) and the Americas reports (`C4`): a passive repository performs none of these checks.
  - **Chemical-basis conversion (`E322–E324`):** a certificate value reported as $\text{NO}_3\text{-N}$ must be converted ($\times 62.0049 / 14.0067$) before it is compared with a limit stated as $\text{NO}_3$.
  - **Certificate-to-table completeness (`E325–E328`):** every sample on a lab certificate must appear in the summary table, or its absence must be explained.
  - **Unmapped stakeholder concerns (`E331–E332`) and matrix consistency (`E337`):** each recorded concern needs a commitment row, and each printed significance rating must match the report's own matrix.
- **Verdict:** **KILL.**

#### H3-c — Three-valued TARP obligation automaton + generalized `RequirementSet` / `EsCommitment` matrix + coupled $\bar{B}$ & sub-aqueous PAG twin
- **Mechanism.**
  1. **Three-valued TARP automaton (`PO-FREE-1`, shared with `PO-HIGH-1`):** Every sensor stream, manual gate/valve position (`E282`), and groundwater/pond limit (`Veladero` PSSA $\le 3{,}927\text{ MASL}$, LCRS $\le 3{,}914.7\text{ MASL}$ / $270\text{ m}^3/\text{d}$, `E280–E281`) evaluates to `{NotTriggered, Triggered, Indeterminate}`. Any `Indeterminate` state (stale past freshness window, missing piezometer baseline `E333`, or conflicting lab vs table value) escalates notification and opens an `OB-TARP` obligation identically to `Triggered`.
  2. **Generalized `RequirementSet` & `EsCommitment` engine (`§18.2`):** Covers `{GISTM, ESMP, PermitCondition, IFC_PS, EP4, LocalContentAgreement}` with `DocApproval` (`Draft` vs `Approved`, capping unapproved Rev A sources at `conflicted`/`stale` per `RISK-29`), chemical-basis typing (`§25.2`, `G-UNIT-5`), and certificate-to-table reconciliation (`§18.4`).
  3. **Coupled rate-of-rise $\bar{B}_k = \Delta u_k / \Delta \sigma_{v,k}$ & sub-aqueous PAG co-disposal (`§18.3`):** Links VWP pore-pressure response per raise lift to Highwall's analytical slice Jacobian $\partial \text{FoS}_s / \partial u_k$ (`§14.5`), and couples submerged PAG waste rock + tailings storage (`Pueblo Viejo` Naranjo TSF, `E266`) to `Pushback` shadow prices.
- **Falsifier.** In a 3-month shadow run (`GATE-FREE-02`), any silent `Indeterminate -> NotTriggered` mapping occurs, or the engine fails to flag every seeded certificate-vs-table omission and chemical-basis mismatch in the corpus fixtures.
- **Verdict:** **SURVIVE, adopted.**

#### H3-d — *(Forced third alternative)* Three-Measurand Closure & Perpetual-Liability Real-Option Engine
- **Mechanism.**
  1. **Three distinct closure measurands (`§18.6`):** Never merge `RegulatorBond` (financial assurance), `IfrsProvision_IAS37` (accounting provision for current disturbance), and `StochasticClosureEstimate` (full LOM + perpetual water treatment tail). Corpus proof: bond $\div$ operator estimate is **2.7× at Carlin**, **2.2× at Turquoise Ridge**, and **1.5× at Cortez** (`E188`), while Pueblo Viejo carries $\$569\text{M}$ LOM, $\$273\text{M}$ undiscounted provision ($\$126\text{M}$ discounted), and $\$250\text{M}$ assurance (`E271`).
  2. **Hazardous inventory completeness invariant:** Every `HazardousByproduct` inventory node in `Crucible` (e.g. `Veladero`'s stored flasked mercury, excluded from its closure plan, `E285`) must map to an explicit closure activity line or raise `G-FREE-11` (`UnmappedClosureInventory`).
  3. **Progressive rehabilitation option pricing:** Price progressive cover placement during mining (using waste haulage slack) against (i) reduced oxygen/water infiltration into PAG dumps (`§20.8`), (ii) early statutory bond release, and (iii) curtailment of the perpetual water-treatment PV tail (Card **B6-G3**).
- **Verdict:** **SURVIVE, merged into H3-c.**

### 3.2 Galaxy-brain card B6-G3 · The Perpetual Water-Treatment Tail & Progressive Cover Option

- **Equation.**
  - Present value of $T_{\text{treat}}$ years of post-closure ARD water treatment at annual real cost $c_{\text{treat}}$ under real discount rate $r$:
    $$\text{PV}(r, T_{\text{treat}}) = c_{\text{treat}} \frac{1 - (1 + r)^{-T_{\text{treat}}}}{r}, \qquad \lim_{T_{\text{treat}} \to \infty} \text{PV} = \frac{c_{\text{treat}}}{r}$$
  - Under IAS 37.36 / IFRIC 1, a $100\text{ bps}$ downward shift in the real risk-free discount rate from $r_0 = 3.0\%$ to $r_1 = 2.0\%$ on a perpetual treatment stream ($T_{\text{treat}} = 100\text{ y}$) multiplies the liability by:
    $$\frac{\text{PV}(0.02, 100)}{\text{PV}(0.03, 100)} = \frac{43.10 \cdot c_{\text{treat}}}{31.60 \cdot c_{\text{treat}}} = \mathbf{1.364\;\;(+36.4\%)}$$
  - If progressive concurrent encapsulation ($C_{\text{cover}}$ spent in year 0 using waste-truck backhaul slack) cuts net infiltration so $c_{\text{treat}}$ drops by fraction $\eta = 0.65$ and treatment duration shortens from $100\text{ y}$ to $25\text{ y}$, the avoided liability PV at $r = 2.5\%$ is:
    $$\Delta \text{PV}_{\text{avoided}} = c_{\text{treat}} \left[\frac{1 - 1.025^{-100}}{0.025} - (1 - \eta)\frac{1 - 1.025^{-25}}{0.025}\right] = c_{\text{treat}}(36.61 - 0.35 \times 18.42) = \mathbf{30.17 \cdot c_{\text{treat}}}$$
- **Substituted ($c_{\text{treat}} = \$6.0\text{M/y}$ lime/HDS plant, $C_{\text{cover}} = \$45\text{M}$ concurrent vs $\$68\text{M}$ post-closure remobilization):**
  - Perpetual 100-year treatment PV at $r = 2.5\%$ is **$\$219.7\text{M}$** (and jumps to **$\$258.6\text{M}$** at $r = 2.0\%$).
  - Concurrent progressive cover saves $\Delta \text{PV}_{\text{avoided}} = 30.17 \times \$6.0\text{M} = \mathbf{\$181.0\text{M}}$ in treatment PV plus $\$23\text{M}$ in avoided contractor remobilization, netting **+$159.0M** after the $\$45\text{M}$ cover cost.
- **Intuition.** Because mine planners discount closure at corporate WACC ($8\text{–}10\%$) while IAS 37 and surety bond regulators discount post-closure liabilities at real sovereign/credit-adjusted rates ($1.5\text{–}3.0\%$), traditional LOM optimization systematically defers dump encapsulation until the end of mine life — creating a massive balance-sheet ARO and bond-collateral shock when rates fall or PAG oxidation accelerates.
- **Flip.** If the waste dump is verified NAF ($P(\text{PAG}) < P^*$ via `GB-22`) or sits in a hyper-arid zero-net-percolation basin (`< 20 mm/y` rainfall with zero basal seepage), $c_{\text{treat}} \to 0$ and only earthworks/reshaping savings remain.

---

## 4. B6-SQ4 — Muster (§19): Workforce, Rosters, Critical Controls & Productivity Under Goodhart Pressure

### 4.1 Hypotheses (refuter-first)

#### H4-a — Individual operator leaderboards & wearable/camera disciplinary scoring
- **Mechanism.** Ingest per-operator FMS cycle speeds and in-cab fatigue/camera triggers; publish operator rankings and tie individual incentives to tonnes per hour.
- **Falsifier.**
  1. **Legal & works-council block:** Prohibited or subject to co-determination veto in major mining jurisdictions; violates `G-MUST-1` and `G-MUST-5`.
  2. **Statistical confounding (`§19.6`):** Bench rock hardness (`Strata`), shovel diggability, ramp grade, and truck mechanical derating explain more shift variance than operator skill; unadjusted rankings punish crews assigned to hard/wet pushbacks (`G-MUST-2`).
- **Verdict:** **KILL.**

#### H4-b — Cost-minimizing roster LP that trades fatigue/overtime against mill-hour dollars
- **Mechanism.** Treat maximum shift length, consecutive night shifts, and statutory appointments as soft penalty terms in the short-term production schedule.
- **Falsifier.** Violates statutory mine safety law and `G-MUST-4` / `G-PUSH-1`. As `GB-23` proved, an unconstrained cost optimizer always pushes toward longer swings (14/7 over 8/6) and more consecutive nights.
- **Verdict:** **KILL.**

#### H4-c — Hard competency/roster feasibility + covariate-adjusted hierarchical productivity + DiD attribution
- **Mechanism.**
  1. **Deterministic feasibility checker (`PO-MUST-1`):** Every scheduled shift checks seat coverage $\times$ unexpired `Competency` $\times$ `StatutoryAppointment` (including deputies) $\times$ `Agreement` hours-of-work envelope. Any deficit marks the plan `infeasible-labour` (`G-MUST-3`).
  2. **Denominator-typed local-content commitments (`§19.3`, `E348`):** Every local-employment row declares owner-vs-contractor scope, locality definition, and numeric target (so that an owner-only count with no stated target, such as `317/427`, never renders as "compliant").
  3. **Hierarchical covariate-adjusted productivity & staggered-rollout DiD (`§19.6`):** Regress out hardness, haul distance, weather, and fleet availability before reporting shrunk crew-panel intervals; register difference-in-differences (DiD) counterfactual specifications *before* AHS or roster rollouts (`Cortez`/`Carlin` `E024`).
- **Verdict:** **SURVIVE, amended** by merging **H4-d**.

#### H4-d — *(Forced third alternative)* Goodhart-Resistant Critical Control Verification (CCV) Forensic Engine
- **Mechanism.**
  - **The Goodhart problem (`FACT, E025, R6 §1.5`):** Critical Control Verification (CCV) completion is a weighted metric in Barrick's published annual scorecard (threshold 75%, target/max 85%, 2025 actual 84%). Any verification count used as a target carries the general risk that checklists are completed quickly ("tick-and-flick") with zero reported non-conformances, so the engine measures verification quality as well as count.
  - **The forensic model (`fmine-workforce::ccv`):** Instead of reporting raw completion %, model field verifications for Material Unwanted Event (MUE) control $c$ by verifier $i$ as a **hierarchical Beta-Binomial / capture-recapture detection process**:
    1. True latent control non-conformance rate on site-month $t$: $p_{c,t} \sim \text{Beta}(\alpha_c, \beta_c)$, anchored by blinded independent safety-audit re-verifications (capture-recapture overlap);
    2. Verifier detection sensitivity $s_i \in (0, 1)$: probability that verifier $i$ records a non-conformance given one exists;
    3. **Checklist entropy & duration telemetry:** compute Shannon answer entropy $H_i$, completion duration $d_i$ vs physical inspection minimum $d_{\min}(c)$, and photo/geolocation hash uniqueness.
  - When a site unit reports $N$ consecutive verifications with $k = 0$ non-conformances while independent audits or linked TARP/telemetry triggers (`Highwall` §14.4, `Freeboard` §18.3, `Headframe` gas/ventilation §16.6) show active control degradation $\hat{p}_{c,t} > 0$, `fmine-workforce::ccv` computes the posterior probability of **Tick-and-Flick Suppression ($s_i \ll s_{\text{audit}}$)** and opens an `OB-REVIEW` verification-quality audit (`G-MUST-6`).
- **Falsifier.** On synthetic and historical safety-audit datasets, the capture-recapture + entropy detector fails to separate genuinely well-controlled areas ($\hat{p}_{\text{audit}} \approx 0$) from tick-and-flick suppression ($\hat{p}_{\text{audit}} \ge 0.03,\; k_{\text{supervisor}} = 0$).
- **Verdict:** **SURVIVE, merged into H4-c.**

### 4.2 Galaxy-brain card B6-G4 · Goodhart on Safety Targets: When "300 Checks, 0 Defects" Proves Blindness, Not Safety

- **Equation.**
  - Suppose critical control $c$ has a baseline non-conformance prevalence $p \in (0,1)$ established by independent blinded safety audits ($n_{\text{audit}}$ checks finding $k_{\text{audit}}$ defects, posterior $p \sim \text{Beta}(1 + k_{\text{audit}}, 1 + n_{\text{audit}} - k_{\text{audit}})$).
  - A routine supervisor stream with relative detection sensitivity $s \in (0, 1]$ performs $N$ verifications and reports $k = 0$ non-conformances.
  - Under the null hypothesis of honest, attentive verification ($H_0: s = 1$), the predictive probability of observing $k = 0$ in $N$ checks given the audit posterior $\text{Beta}(a, b)$ is the Beta-Binomial zero-count probability:
    $$\mathbb{P}(k = 0 \mid N, H_0) = \frac{\text{B}(a, b + N)}{\text{B}(a, b)} = \prod_{j=0}^{a-1} \frac{b + j}{a + b + N - 1 - j}$$
  - Conversely, under the tick-and-flick hypothesis ($H_1: s = 0.05$, rubber-stamping 95% of defects), the likelihood of $k = 0$ remains near $1$, giving a Bayes factor in favour of suppression $B_{10} = \frac{\mathbb{P}(k=0 \mid H_1)}{\mathbb{P}(k=0 \mid H_0)}$.
- **Substituted ($N = 300$ routine supervisor CCVs with $k = 0$ non-conformances; independent quarterly audit found $k_{\text{audit}} = 3$ non-conformances in $n_{\text{audit}} = 50$ checks, so $a = 4, b = 48$, mean $\hat{p} = 7.7\%$):**
  - Under $H_0$ ($s = 1$, attentive verification):
    $$\mathbb{P}(k = 0 \mid N = 300, H_0) = \frac{48 \times 49 \times 50 \times 51}{348 \times 349 \times 350 \times 351} = \frac{5{,}997{,}600}{1.4918 \times 10^{10}} = \mathbf{0.000402\;\;(1\text{ in }2{,}487)}$$
  - Under $H_1$ ($s = 0.05$, tick-and-flick rubber-stamping, effective defect rate $s \cdot p \approx 0.38\%$): $\mathbb{P}(k = 0 \mid N = 300, H_1) = \mathbf{0.342}$.
  - Bayes factor for tick-and-flick suppression: **$B_{10} = 0.342 / 0.000402 = 851$** (decisive evidence of verification blindness).
- **Intuition.** When CCV completion is a scorecard target (`E025`), a 100% completion rate with zero found non-conformances reads well on a traditional KPI scorecard. Against even a modest 6–8% independent audit defect rate, $300$ zero-defect checks is a $4 \times 10^{-4}$ event under attentive inspection, which is strong evidence that verification sensitivity is low and warrants an audit.
- **Flip.** If the independent blinded audit also finds $k_{\text{audit}} = 0$ in $n_{\text{audit}} = 50$ checks ($a = 1, b = 51$) *and* linked automated sensors (e.g. gas/ventilation/proximity telemetry) record zero threshold excursions, $\mathbb{P}(k=0 \mid N=300, H_0) = 51 / 351 = 0.145$, so $B_{10} < 3$ and no suppression alarm fires.

---

## 5. Summary of Plan Additions from B6 (Fold-In Specification)

1. **`Appendix B` & `§36.4` / `Appendix H.6` (`H-2`):** Record `B6` as **COMPLETE** (`planning/brenner/B6_operations_plant_tailings_workforce.md`), closing open audit item `H-2` and adding 17 rows to the Brenner Verdict Register (`Appendix B`).
2. **`§16.11` (`Headframe`):** Add **GB-40 (`B6-G1`: AHS Geo-Fence Bubble Contagion vs. Match Factor)** quantifying platoon bunching at shovel spotting locks when auxiliary vehicles enter AHS zones.
3. **`§17.5–17.6` & `§17.10` (`Crucible`):**
   - Add guard **`G-CRUC-6` (`TautologicalTonnageMcf`)**: if `MCF_t` shares the same weightometer/flowmeter between mine-delivered and plant-milled (`Loulo` `C3 §3.12`), refuse to report tonnage bias as identified (`FMINE-REF-104`).
   - Add **GB-41 (`B6-G2`: Concentrate Assay Exchange, Splitting Limits & Penalty Cliff Blending)** to `§17.10`.
4. **`§18.6–18.8` (`Freeboard`):**
   - Add guard **`G-FREE-11` (`UnmappedClosureInventory`)**: any `HazardousByproduct` inventory node in `Crucible` (`Veladero` flasked mercury `E285`) without a mapped closure-activity line in `fmine-tailings::closure` refuses closure-pack sign-off preparation.
   - Add **GB-42 (`B6-G3`: The Perpetual Water-Treatment Tail & Progressive Cover Option)** to `§18.8`.
5. **`§19.5`, `§19.8` & `§19.9` (`Muster`):**
   - Add Beta-Binomial capture-recapture CCV verification-quality auditing and guard **`G-MUST-6` (`TickAndFlickSuppressionAlarm`)** when routine supervisor zero-defect runs diverge from blinded audit or sensor excursion baselines ($B_{10} \ge 20$).
   - Add **GB-43 (`B6-G4`: Goodhart on Safety Targets — When "300 Checks, 0 Defects" Proves Blindness, Not Safety)** to `§19.9`.
