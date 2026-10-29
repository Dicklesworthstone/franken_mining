# R1: FrankenSim survey for FrankenMining

- Date: 2026-10-28
- Surveyed repo: `/Users/jemanuel/projects/frankensim`, HEAD `20463525d` (commit dated 2026-10-26)
- Method: read-only. Read docs, `CONTRACT.md` files, `Cargo.toml` files and source, and grepped for negatives. Nothing was built (a build would write `target/`) and no file in any surveyed repo was changed. Line and test counts come from `find`/`grep` over `crates/<name>/src` and `#[test]` attributes. They are size indicators, not quality measures.
- Companion reports in this folder: `R5_textiles_patterns.md` (the FrankenTextiles view of the same lifting pattern).

---

## 0. Executive summary

1. **FrankenSim is large, fast-moving and not converged.** It has about 10,060 commits, 3,007 of them in October 2026 alone. 3,769 paths are dirty in the shared working tree. There are 188 crate directories; the README claims 164 workspace crates and 184 contracts. The project's own `docs/REALITY_CHECK_2026-09-01.md` (refreshed 2026-09-24) says "Velocity is high; convergence is zero". `cargo check --workspace --all-targets` exits 101, with 15 targets that do not compile. Of the 15 tracked capabilities, exactly one (`thermal.conduction-solve`) is at maturity L3 and none is at L4 or L5. FrankenMining should treat FrankenSim as **a quarry of well-documented components and conventions, not a stable platform**.
2. **No protocol exists for external downstream domains.** The only internal extension charter is `COMPREHENSIVE_PLAN_TO_EXTEND_FRANKENSIM_TO_NEW_DOMAINS.md`, which covers machine elements. The precedents are:
   - FrankenTextiles **lifts files with provenance headers** and takes no Cargo dependency.
   - franken_accounting uses **git dependencies with exact rev pins** on other suite crates and does not use FrankenSim at all.
   - One hobby tool path-depends on FrankenSim through the sibling directory layout.
3. **A git dependency on FrankenSim will very likely fail without a spike-proven workaround.** Crates reference `asupersync` as `path = "../../../asupersync"`, which points outside the repo. Every geometry crate needs `fs_exec::Cx`, which pulls in asupersync. FrankenSim also needs nightly (`nightly-2026-07-06`) because `fs-qty` uses `#![feature(generic_const_exprs)]`.
4. **Mining-specific capability is close to zero, but the substrate is rich.** None of these exist anywhere in the repo: kriging, variograms, conditional simulation, drillholes, Mohr-Coulomb, Hoek-Brown or Drucker-Prager models, comminution and the Bond law, Bingham or Herschel-Bulkley rheology, Richards flow, discrete element method (DEM), ICP registration, CRS/EPSG, mining file formats, or mining units (tonne, troy ounce, g/t, ppm, day, year, currency). Strong reusable substrate does exist:
   - sparse hierarchical grids (`VdbGrid<T>`)
   - implicit and mesh geometry with robust winding numbers
   - sparse linear algebra with AMG
   - ensemble assimilation (EnSRF)
   - value-of-information ranking and a sensor-placement campaign
   - CVaR and distributionally robust optimization (DRO)
   - critical-path methods in max-plus algebra
   - fan and network airflow
   - Archard wear
   - a dimensioned-quantity type
   - a provenance-first evidence and claim system
   - an offline, licence-enforcing materials pack compiler.
5. **The materials DB has no geomaterials.** It contains 183 inventoried bundles and 1,012 claims, but no rock, soil, ore, slurry or tailings materials, no abrasion-resistant liner steels, and no Bond work index. The expansion plan says geomaterials "need stateful granular/porous constitutive support as well as data". The pack format (`frankensim.matdb-manifest.v1` plus `material-tsv-v1` sources, compiled by `cargo run -p xtask -- matdb-pack`) can carry scalar and tabulated mining properties now. It cannot carry rock-mass constitutive behaviour, because fs-material has no model to resolve it into.
6. **Recommended posture.** Lift small, asupersync-free leaf algorithms with provenance headers and golden-hash tests, as FrankenTextiles does. Adopt FrankenSim's evidence, refusal and Five-Explicits conventions as FrankenMining doctrine. Prove any direct dependency on the geometry stack with a spike in a separate clone first. Build the geostatistics, geomechanics, mine-planning and mining I/O layers new.

---

## 1. House conventions

### 1.1 The Decalogue (plan principles P1-P10)

| Principle | Content | Implication for FrankenMining |
|---|---|---|
| P1 Franken-only dependencies | Allowed: std, asupersync, FrankenSQLite, FrankenNumpy, FrankenTorch, FrankenScipy, FrankenPandas, FrankenNetworkx. No BLAS, C or FFI. | Lifted code arrives dependency-clean. FrankenMining must decide whether it adopts P1. (FrankenTextiles relaxed it to allow serde, spade and miniz_oxide.) |
| P2 Determinism | Bitwise reproducibility per declared class (see 1.5) | Needed for reproducible resource estimates and audit trails |
| P3 Differentiable or certifiable | Every numeric path either has an adjoint or carries a certificate | The "adjoint-or-excuse" doctrine (D8 in the new-domains plan) |
| P4 Budgets | Error Ledger and Time Ledger (`fs-plan` `ErrorLedger`, `TimeLedger`, `CostModel`) | Maps onto estimation error budgets |
| P7 Cancellation | `fs_exec::Cx` polled at tile boundaries | The source of the asupersync coupling |
| P9 Provenance | Content addressing (`fs-blake3`, `ProvenanceHash`) | Direct fit for regulatory traceability (JORC, NI 43-101, S-K 1300) |
| P10 Five Explicits | Units, seeds, budgets, versions, capabilities, plus structured errors that carry fixes | `fs_ledger::FiveExplicits{seed, versions, budget, capability}`. Units are carried by `Qty`. |

### 1.2 Layers

Each crate declares `[package.metadata.frankensim] layer = "UTIL" | "L0" ... "L6" | "TOOL"`. `cargo run -p xtask -- check-layers` enforces that dependencies point downward. L5 (visualisation) may not depend on L4.

Typical placement: L0 executor, allocator and math; L1 linear algebra, sparse, random numbers, interval arithmetic, matdb; L2 geometry and representations; L3 physics solvers and materials; L4 UQ, optimisation and assimilation; L6 product (CLI, ledger, session, IR, package).

### 1.3 Contracts and gates

- **One crate, one `CONTRACT.md`.** Each has 10 sections, per `docs/CONTRACT_TEMPLATE.md`:
  1. purpose and layer
  2. public types
  3. invariants
  4. error model
  5. determinism class
  6. cancellation
  7. unsafe
  8. feature flags
  9. conformance tests
  10. no-claim boundaries and golden couplings
- Each crate also has `tests/conformance.rs`.
- Gates run with `cargo run -p xtask -- check-all`. This covers check-contracts, check-layers, check-libm and others.
- Lints: workspace `deny(unsafe_code)`.

### 1.4 Ambition tags

Features are tagged `[S]` (standard), `[F]` (frontier, `frontier-*` feature) or `[M]` (moonshot, `moonshot-*` feature). Frontier and moonshot features are off by default. Example: `fs-dd` (BDDC domain decomposition) is `[M]`.

### 1.5 Determinism classes (`docs/DETERMINISM_CLASSES.md`)

- `Deterministic`: bitwise identical across ISAs. Transcendentals must go through `fs_math::det`, and xtask `check-libm` and `LIBM_DOCTRINE_CRATES` enforce this.
- `DeterministicPerIsa`: bitwise identical within one ISA. `fs-conduction` declares this class.
- `Fast`: no bitwise promise.
- The runtime mode is `fs_exec::ExecMode`.

FrankenTextiles lifted `fs-math`'s `det` module and guards it with a golden-hash test (`frankensim_core_golden_hash`). That is the template for keeping a lifted copy honest.

### 1.6 Evidence and claim integrity (`fs-evidence`, `docs/CLAIM_INTEGRITY.md`)

- **`Evidence<T>`** has these fields:
  - `value`
  - `qoi`
  - `numerical: NumericalCertificate{kind: NumericalKind::{Exact, Enclosure, Estimate, NoClaim}, lo, hi}`
  - `statistical: StatisticalCertificate::{None, EValue, HalfWidth}`
  - `model: ModelEvidence`
  - `sensitivity`
  - `provenance: ProvenanceHash`
  - `adjoint_ref`
- **`Certified<T>`** cannot be forged. The only way to obtain one is `Evidence::certified()`, and `into_evidence()` downgrades it. The type also provides `Evidence::combine`, `assess` and `escalation_advice`.
- **`ValidityDomain`** is the single validity type. It holds named axis boxes, and `with_quantity` attaches a `QuantitySpec`.
- **`ModelCard`** has fields `{name, version, ambition, assumptions, validity, known_failures, calibration, discrepancy_rel}`.
- **Colour algebra.** `Color::Verified{lo,hi}`, `Validated{regime, dataset}` and `Estimated{estimator, dispersion}`, together with `ColorRank`, `compose` and `check_regime`. A no-laundering rule applies: composition can never promote a colour.
- **`ClaimClass`** has 8 families.
- **Doctrine:**
  - "Verification is not validation."
  - Under-claiming is not a defect.
  - Refusals are typed, never silent defaults.
  - Claim-integrity defects are labelled `claim-integrity`, `severity:default-path|gated|doc-only` and `crate:<name>`.

**Mining fit.** The colour system lines up naturally with resource and reserve confidence language. It should not be conflated with Measured, Indicated and Inferred classification, which is a Competent Person judgement and not a numerical certificate. FrankenMining should adopt the evidence types (by lifting `fs-evidence`, whose closure is 3 crates and asupersync-free) and add its own mapping layer to the reporting codes.

### 1.7 Typed refusals and guidance

- Each crate defines error enums with named refusals, for example `MatDbError::NoClaimInDomain`, `AmbiguousSelection` and `OutsideKnotSpan`.
- Scenario validation produces `fs_scenario::Violation{code, what, fix}`.
- The session layer produces `fs_session::Guidance{code, diagnosis, fixes: Vec<RankedFix>}`.
- Agent-facing CLIs follow the pattern "diagnosis plus ranked fixes".

### 1.8 Maturity and frozen schemas

**Maturity.** `docs/MATURITY_LEVELS.md` defines levels L1 to L5. `capability-maturity.json` tracks 15 capabilities:

| Level | Capabilities |
|---|---|
| L3 | `thermal.conduction-solve` only |
| L2 | `evidence.colour-algebra`, `evidence.packaging`, `geometry.chart-conversion`, `geometry.topology-certificates`, `numerics.sparse-assembly`, `numerics.certified-arithmetic`, `solvers.krylov`, `gradients.adjoint-verification`, `campaign.quality-diversity`, `governance.*` |
| L1 | `geometry.import-formats`; `ledger.design-ledger` (4 GC tests red, caused by a FrankenSQLite cascade bug); `wasm.browser-flagships` |
| L4, L5 | none |

**Frozen schemas.** `schema-policy.json` freezes:
- `project.fsim` (v15)
- `package.format` (v9)
- `matdb.pack` (the FSMATPK family; `MATDB_STRESS_PACK_SCHEMA_VERSION` 7, in lockstep)
- `ledger.schema` (v20)
- `scenario.ir`, among others.

**Consequence.** New property kinds or axis types for mining in the frozen matdb pack schema are governed changes, not casual additions.

### 1.9 Shared-tree and process rules (`AGENTS.md`)

- No branches or worktrees inside the frankensim tree, and no file deletion.
- Coordinate through Agent Mail file reservations.
- Gates run through DSR (Doodlestein Self-Releaser, the local release and build tooling).
- Work is tracked in Beads.

**Consequence.** A FrankenMining spike against FrankenSim must use a separate clone (or `git archive` of a pinned commit), not a worktree in the shared tree.

### 1.10 Toolchain and licence

- Workspace version 0.0.1, edition 2024, unpublished.
- Toolchain pinned to `nightly-2026-07-06` (`rust-toolchain.toml`). The narrow nightly features are `generic_const_exprs` in `fs-qty` and portable SIMD behind `experimental-portable-simd` in `fs-simd`.
- FrankenTextiles pins `nightly-2026-08-31`, so the suite is already on mixed nightlies.
- Licence is `license-file`: MIT plus an OpenAI/Anthropic rider. FrankenMining needs a licence decision before redistributing lifted code.

### 1.11 Consolidation verdicts relevant here

`consolidation-review.json` records:
- `fs-rep-voxel`: KEEP, with zero dependents.
- `fs-dimine`: FREEZE.
- `fs-wasm`: REPAIR-OR-QUARANTINE.

**Name traps.** `fs-dimine` is "dimensional knowledge mining" (fitting power-law scaling laws in pi-space). It has nothing to do with the mining industry. `fs-frame` is a seismic building-frame flagship, not a coordinate-frame crate.

---

## 2. Domain-extension protocol and how FrankenTextiles consumes FrankenSim

### 2.1 What exists (internal only)

`COMPREHENSIVE_PLAN_TO_EXTEND_FRANKENSIM_TO_NEW_DOMAINS.md` is the internal machine-domain charter. Its domains are the Geneva drive, gears, the Wankel engine, PMSM motors and IC engines.

**Doctrine D1-D12.** The items that matter most for mining:
- D2: typed constitutive graph.
- D6: correlations are Validated or Estimated and are never promoted by verification.
- D8: adjoint-or-excuse.
- D9: verification is not validation.
- D12: one research bet per proof lane.

**Section 15 (operationalisation):**
- Beads for work tracking.
- Contracts come first (`check-contracts`).
- Layer and cycle admission through xtask.
- Oracles are dev-only (for example FrankenScipy as a dev-dependency).
- DSR lanes.
- A research-bet ledger.

**Section 5.10 (materials).** `MaterialCard`, `InterfaceSystemCard`, `PropertyUsageReceipt`, and an offline pack compiler. Appendix B sketches a card schema.

**Section 5.13 (porous media).** Darcy, Brinkman, Richards and Biot are **planned, not built**.

**Ratified verticals.** There is exactly one: thermal design assurance, "Cooling 0.1" (`fs_govern::ratification`, `fs-wedge`). Displacement rule `f85xj.16.3` deprioritises off-critical-path work. Mining-motivated upstream changes would be off the critical path and are likely to be displaced.

**Conclusion.** There is no supported way for an external domain to "plug into" FrankenSim. FrankenMining must define its own consumption contract.

### 2.2 How FrankenTextiles consumes FrankenSim: lifting with provenance

FrankenTextiles has **no Cargo dependency** on FrankenSim. Its rules:
- Plan section 11.5 and line 599: "frankensim code is lifted file by file with provenance, not depended on as a workspace".
- Its `AGENTS.md` requires "a provenance header on every lifted file naming the source path and commit".

Observed lifts:

| FrankenTextiles file | FrankenSim source | Guard |
|---|---|---|
| `ftx-core/src/det.rs` | `fs-math` `det` | Golden-hash test `frankensim_core_golden_hash` |
| `ftx-core/src/philox.rs` | `fs-rand`, commit `8254164a` | |
| `ftx-core/src/exact.rs`, `intersection.rs` | `fs-ivl`, `fs-topo`, commit `e76c9421` | |
| `response_curve.rs` | `fs-contact` | |
| `ftx-drape/src/sparse/direct.rs`, `sparse/precond.rs` | `fs-sparse`, commit `93fc9b26` | |
| `bvh.rs` | `fs-query` `features.rs` | |
| `eigen.rs` | `fs-la` `jacobi_eigh` | |
| `ad.rs` | `fs-ad` | |
| `render.rs` | (lifted) | |

`ftx-conformance` adds `propcheck` and `detaudit`. FrankenTextiles' `docs/material-catalog.md` treats `fs-matdb` as design inspiration only.

### 2.3 The reverse integration (FrankenTextiles into the FrankenSim matdb)

`docs/FRANKENTEXTILES_INTEGRATION.md` describes the opposite direction:
1. FrankenSim imports the FrankenTextiles catalogue with `scripts/import_franken_textiles.py` (offline, stdlib-only Python).
2. It compiles the result with `cargo run -p xtask -- matdb-pack --manifest ... --out ...`.
3. The data is consumed through `fs_material::sheet::QuadraticMembrane` and `ResolvedSheet`, `fs_couple::sheet_plate::SheetPlate`, and `SheetCaloric`.
4. The import requires `--citation`, `--license` and `--redistribution-terms`. Optional flags are `--allow-estimates` and `--interpolate-temperature`.

The doctrine is "no second material database". If FrankenMining wants its geomaterials consumable by FrankenSim later, it should emit `frankensim.matdb-manifest.v1` bundles rather than invent a format (see section 4).

### 2.4 Other consumption patterns found in the suite

- **franken_accounting.** Uses git dependencies with exact `rev` pins for asupersync (rev `78b6463...`), fgdb, fsym and fnx. A `[patch.crates-io]` entry unifies everything on one asupersync and one `Cx` type. It does not use FrankenSim.
- **`/Users/jemanuel/projects/jazz_chord_progression_editor_html/tools/piano-v2-soundboard-pack`.** Path-depends on `fs-material`, `fs-modal`, `fs-plate` and `fs-sparse` through `../../../frankensim/crates/...`. This works only because asupersync sits in the expected sibling location. It is the only direct Cargo consumer found.
- **`constellation.lock`.** Pins asupersync 0.5.0 (`7a0f2f55`), franken_networkx 0.2.0, franken_numpy, frankenpandas, frankenscipy 0.4.0, frankensqlite (`d5c68ea3`) and frankentorch.

### 2.5 Why a plain git dependency on FrankenSim is risky

1. Crates declare `asupersync = { path = "../../../asupersync" }` (confirmed in `crates/fs-exec/Cargo.toml` lines 26 and 29). The workspace root has `[patch.crates-io] asupersync = { path = "../asupersync" }`. In a git checkout, Cargo resolves path dependencies inside the fetched repo, so these out-of-repo paths would not resolve. This is inferred from Cargo semantics and **not yet tested by a spike**.
2. Sibling FrankenScipy, FrankenNetworkx, FrankenNumpy and FrankenTorch dependencies use the same `../../../<sibling>` form. Most of them are dev-dependencies or optional. The exceptions:
   - fsqlite is a normal dependency of `fs-ledger`, `fs-matdb-store` and `fs-vskeleton`.
   - `fs-rep-voxel` and `fs-truss` have normal dependencies on fnx-classes and fnx-runtime.
3. Nightly is required transitively by anything that touches `fs-qty`.
4. The tree is dirty and HEAD moves about 100 commits per day. A pin must be a commit hash, and even pinned commits may not compile workspace-wide (exit 101).

### 2.6 Asupersync-free crates (transitive normal-dependency closure computed)

These crates can be lifted, or path-depended on, without dragging in asupersync or `Cx`:

`fs-qty`, `fs-blake3`, `fs-obs`, `fs-math`, `fs-evidence` (closure of 3), `fs-ivl`, `fs-matdb` (closure of 6), `fs-thermochem`, `fs-tribo`, `fs-rand`, `fs-ga`, `fs-rep-neural`, `fs-voi`, `fs-robust`, `fs-robustopt-e2e`, `fs-regime`, `fs-dimine`, `fs-tropical`, `fs-eproc`, `fs-schedule-e2e`, `fs-govern`, `fs-contract`, `fs-casebook`, `fs-archive`, `fs-mbd`, `fs-dd`, `fs-sparse`, `fs-package`, `fs-thermal`, `fs-toleralloc`, `fs-sos`, `fs-vvreg`.

**Not asupersync-free.**
- `fs-exec` depends on asupersync directly.
- Anything geometric is coupled through `fs_geom::Chart::eval(&self, x: Point3, cx: &Cx)`. That includes `fs-geom`, every `fs-rep-*` crate except `fs-rep-neural`, `fs-mesh` and `fs-query`.
- `fs-uq`, `fs-assimilate`, `fs-material`, `fs-solid`, `fs-conduction`, `fs-airflow` and `fs-lbm` are outside the asupersync-free set.
- Constructing a `Cx` requires `Cx::new(&CancelGate, &fs_alloc::Arena, StreamKey, Budget, ExecMode)`, plus optionally `with_time_source(&dyn asupersync::time::TimeSource)`.

### 2.7 Consumption options for FrankenMining

| Option | Mechanism | Pros | Cons | Use for |
|---|---|---|---|---|
| A. Lift with provenance (FrankenTextiles pattern) | Copy files and add a header naming the source path and commit. Add golden-hash and conformance tests. | No toolchain or asupersync coupling. Survives FrankenSim churn. | The copy drifts. Bug fixes must be re-lifted by hand. | Small leaf algorithms: `fs-math::det`, `fs-rand` Philox/Sobol, `fs-evidence` types, `fs-voi`, `fs-robust`, `fs-tropical`, `fs-eproc`, parts of `fs-sparse`, `VdbGrid` (after stripping `Cx`), winding numbers |
| B. Path dependency via sibling layout and a pinned commit | `../frankensim/crates/...` plus a lock file recording the FrankenSim commit (like `constellation.lock`) | Full fidelity. Picks up fixes. | Nightly and asupersync coupling. Breaks if the FrankenSim HEAD does not compile. Not reproducible without the lock. | Geometry stack, if needed whole |
| C. Git dependency with rev pin | `git = ..., rev = ...` | Reproducible | Very likely blocked by the out-of-repo `../../../asupersync` paths (2.5). Needs a spike. | Only if the spike passes |
| D. Upstream contribution | Beads and contracts inside FrankenSim | Shared maintenance | The displacement rule, frozen schemas and shared-tree governance make mining work low priority upstream | Only generic substrate fixes |

**Recommendation.**
- Default to A for asupersync-free leaves. Keep FrankenTextiles-style provenance headers and golden tests.
- Run one bounded spike on B or C for the geometry stack in a separate clone. Pick a pinned commit, build a scratch crate depending on `fs-rep-sdf` and `fs-rep-mesh`, and record the outcome. Do this before committing to depend on it.
- Adopt the conventions in section 1 as FrankenMining doctrine regardless of which option is chosen.

---

## 3. Per-crate fact table

Columns: layer | lines in `src` | count of `#[test]` attributes | lines in `CONTRACT.md` | key public surface | mining relevance. "AF" marks crates whose closure is asupersync-free (2.6).

### 3.1 Geometry and representations

| Crate | Layer | src LOC | #[test] | CONTRACT | Key surface | Mining relevance |
|---|---|---|---|---|---|---|
| fs-geom | L2 | 53,654 | 257 | 1,770 | `Region`, `RegionChart`, `Chart` trait (`eval(Point3, &Cx)`), `Convert`, `SampledSdf`, `Router`, sheaf; `Point3`, `Vec3`, `Aabb` | Common geometry spine. Brings in the `Cx` coupling. |
| fs-rep-voxel | L2 | 1,650 | 8 | 173 | `OccupancyField`, `MaterialField = VdbGrid<u16>`, `DensityField = VdbGrid<f32>`, `DistanceField`, `euclidean_dt`, `PointCloud`, `LatticeGraph`, `OccupancyChart`. Morphology is 6-connected. `euclidean_dt` densifies the bounding box. | Block-model rock-type codes and densities; point clouds. KEEP verdict, zero dependents. |
| fs-rep-sdf | L2 | 3,111 | 19 | 207 | `TiledSdf`, generic `VdbGrid<T: Copy>` (BTreeMap root, 32^3 internal nodes, 8^3 leaves), `NarrowBand`, `AdaptiveSdf`. Its 10^9-voxel target is unbenchmarked. | **Best substrate for sparse block models** and orebody signed-distance fields |
| fs-rep-mesh | L2 | 5,939 | 43 | 331 | `HalfEdgeMesh`, `TetComplex`, `HexComplex`, `TriComplex2`, `Soup`, `winding_exact`, `WindingOctree`, `MeshChart`, `repair`, `dual_contour`, `mesh_to_sdf`, `Bvh` | Wireframes (triangulations); robust inside/outside tests for dirty wireframes |
| fs-rep-nurbs | L2 | 29,635 | 252 | 1,178 | NURBS curves and surfaces | Little relevance (design surfaces) |
| fs-rep-frep | L2 | 5,407 | 65 | 251 | `Frep`, `FrepBuilder`, `BoolStyle::{Hard, Blend}`; primitives sphere, cylinder, box, torus, half-space | Constructive solids (CSG): pit and stope envelopes, fault half-spaces |
| fs-rep-neural | L2 | 839 | 20 | 119 | Neural implicit representation (AF) | Speculative |
| fs-mesh | L2 | 16,164 | 128 | 825 | `delaunay`, `refine`, `remesh`, `structured_hex`, `conservative_cell_remap`. PLC recovery is partial. | Meshing for slope and seepage FEM; regridding block attributes conservatively |
| fs-xform | L2 | 1,616 | 23 | 159 | FFD, RBF morphs (deformation, not scattered-data interpolation) | Not an implicit-modelling interpolator |
| fs-geocon | L4 | 1,341 | 13 | n/a | `volume_certified`, `volume_smooth`, `envelope_violation`, `min_thickness_soft`, `draft_violations`, `QuotientChart` | Certified volumes (tonnage reconciliation); minimum mining width; wall-angle checks (by analogy with draft angle) |
| fs-asbuilt | L2 | 10,802 | 120 | 659 | `register`, `register3`, `register3_datum`, `register3_similarity`, `as_built_diff(&Registration, &[Point2], &[Point2], tol, noise, calib, &Cx)`, `DeviationField`, `ThicknessField`. Needs known correspondences; no ICP; no free-form surfaces; output colour is Estimated only. | Survey versus design: the starting point for drone/lidar comparison |
| fs-topo | L2 | 1,976 | 27 | 269 | Manifold and self-intersection certificates; cubical Betti numbers | Validating wireframes (watertight, no self-intersections) |
| fs-query | L2 | 9,567 | 98 | n/a | `closest_point`, `raycast`, `separation`, `ClearanceField`, `thickness_at`, `min_thickness`, `minkowski_ball` (ball only), `convex_penetration_depth`, `geometric_moments` | Point-to-surface deviation; standoff to infrastructure; ore-body moments |
| fs-motion | L2 | 4,368 | 35 | 392 | `CertifiedMotorTube`, `SpacetimeChart`, `SweptChart` | Swept volumes (equipment clearance), low priority |
| fs-ga | L2 | 3,505 | 31 | 196 | Geometric algebra (AF) | Low |
| fs-scene | L3 | 807 | 12 | n/a | Static robot scene of boxes and half-spaces | Low |

### 3.2 Materials

| Crate | Layer | src LOC | #[test] | CONTRACT | Key surface | Mining relevance |
|---|---|---|---|---|---|---|
| fs-matdb | L1 | 13,621 | 122 | 1,292 | `PropertyClaim`, `ClaimSet::query`, `query_typed`, `query_pinned_typed`, `MaterialAnswer`, `PropertyUsageReceipt`, cards (AF, closure of 6) | Host format for geomaterial property packs |
| fs-matdb-store | L6 | 2,001 | 23 | n/a | `MaterialStore` (FrankenSQLite), `seal_corpus`, `StoreError::CorpusChanged`, `DiscoveryRequest` | Sealed project corpora (for example, client-confidential rock tests) |
| fs-material | L3 | 62,128 | 687 | 1,743 | `IsotropicElastic`, `OrthotropicElastic`, `J2Plasticity`, `Hyperelastic`, `ManderConcrete`, `MenegottoPintoSteel`, `porous` (JCA acoustic absorber), `state_point::{ScalarPropertyRequirement, ResolvedMaterialStatePoint, resolve_material_state_point}` | **No Mohr-Coulomb, Hoek-Brown or Drucker-Prager models.** `porous` is acoustics, not geomechanics. |
| fs-thermochem | L1 | 8,274 | 45 | n/a | NASA-9 polynomials, ideal-gas mixtures, Wilke mixing, single-reaction equilibrium (AF) | Mine-air thermodynamics; simple leach and gas equilibria |
| fs-tribo | L3 | 9,133 | 70 | n/a | `ArchardLaw`, `FrictionLaw`, Hertz contact, flash temperature, `DryInterfaceSystemCard` (AF) | Liner, GET and chute wear by Archard's law |

### 3.3 Uncertainty, inference, decision and optimisation

| Crate | Layer | src LOC | #[test] | CONTRACT | Key surface | Mining relevance |
|---|---|---|---|---|---|---|
| fs-evidence | UTIL | 37,120 | 326 | 1,642 | Section 1.6 (AF) | Adopt as the claim system |
| fs-uq | L4 | 11,128 | 275 | n/a | `KlExpansion::build(points, &CovarianceKind::{Exponential, SquaredExponential}, sigma2, ell, variance_target)`: dense O(n^2) covariance plus Jacobi O(n^3) eigensolve, isotropic, unconditional. Also PCE, QMC, MLMC, LHS, Shapley and Sobol indices, `chance_constrained_min`, Gaussian copula, Kanai-Tajimi seismic model, `ida_fragility`. | Gaussian random fields only at small n; general UQ toolkit; fragility pattern for slope probability of failure |
| fs-assimilate | L4 | 10,707 | 168 | n/a | Dense `Belief` (`MAX_DENSE_STATE_DIM` = 256, `MAX_DENSE_OBSERVATIONS` = 4096), `point_sensor`, `scan_observation`, `assimilate_colored`. Feature `ensemble`: serial EnSRF (Whitaker-Hamill, O(n x members) per observation), localisation, `Ensemble`, `EnsembleSmoother`. | **Bayesian updating** of grade or geotech fields from drilling, mapping and monitoring (ensemble path) |
| fs-voi | L4 | 1,052 | 32 | n/a | `ranking_flip_probability`, `expected_opportunity_loss`, `top_two_evpi_surrogate`, `action_value`, `recommend`, `KnowledgeGradient` (AF) | VOI core for drilling decisions |
| fs-oed-e2e | L4 | 6,019 | 99 | 319 | SensorForge `run_campaign`: Kalman update, greedy VOI placement, stop when EVPI is exhausted | **Closest analogue to drillhole infill placement** |
| fs-surrogate | L4 | 6,663 | 46 | 359 | `Pod`, `dmd`, `edmd`, `deim`, `conformal_band`, `certify_or_escalate` | Fast emulators for slope, seepage and leach models |
| fs-robust | L4 | 570 | 19 | 130 | `cvar`, `empirical_cvar`, `ColoredObjective`, `robust_optimum`, `fragility_curve` (AF) | Risk-averse pit, schedule and design choices |
| fs-robustopt-e2e | L4 | 261 | 3 | 61 | Robust-optimisation demo (AF) | Pattern only |
| fs-bo | L4 | 8,178 | 149 | 290 | Matern GP with ARD, sparse inducing-point GP, TuRBO, multi-fidelity, q-EI | GP surrogates; a GP is also simple kriging when the kernel is supplied |
| fs-opt | L4 | 17,583 | 214 | 726 | `Problem` IR (continuous variables only, no integers) | Continuous design optimisation, not scheduling |
| fs-dfo | L4 | 17,604 | 271 | 800 | CMA-ES, BIPOP, Nelder-Mead, DE, DIRECT, NSGA-III multi-objective, `dro`, `ot` | Black-box and robust optimisation, DRO |
| fs-constraint | L4 | 4,303 | 57 | 321 | Typed constraint kinds, minimal unsat cores, ranked repairs | Explaining infeasible plans |
| fs-eproc | L4 | 1,399 | 31 | 234 | Betting e-processes, mixture confidence sequences, e-BH (AF) | Anytime-valid reconciliation tests (mine-to-mill, QA/QC drift) |
| fs-plan | L6 | 12,882 | 97 | n/a | `CostModel`, `ErrorLedger`, `TimeLedger` (uses fsqlite) | Error budgeting |
| fs-regime | L3 | 2,277 | 34 | n/a | Buckingham-pi analysis, `admit` regime admission (AF) | Validity envelopes for empirical correlations |
| fs-dimine | L4 | 278 | 9 | 88 | Fits pi-space power laws; Estimated colour; FROZEN (AF) | Fitting comminution and wear scaling laws (name is a false friend) |
| fs-tropical | L1 | 721 | 13 | n/a | `TaskDag::new(latency)`, `critical_path`, `slack`, `max_cycle_mean` (Karp), `critical_circuit`; no dependencies; no resource constraints | Critical path for development and construction schedules |
| fs-schedule-e2e | L6 | 480 | 6 | 69 | Scheduling demo (AF) | Pattern |
| fs-archive | L4 | 447 | 12 | 92 | `MapElites`, `CvtArchive` (AF) | Quality-diversity exploration of design alternatives |
| fs-sos | L4 | 4,253 | 38 | 170 | Sum-of-squares optimisation with an in-house SDP solver; certified Lasserre bounds (AF) | Low |
| fs-truss | L4 | 5,105 | 25 | 205 | Ground-structure truss optimisation | Low (headframes, conveyor structures) |
| fs-toleralloc | L4 | 6,500 | 58 | 464 | Tolerance allocation (AF) | Low |

### 3.4 Physics solvers

| Crate | Layer | src LOC | #[test] | CONTRACT | Key surface | Mining relevance |
|---|---|---|---|---|---|---|
| fs-solid | L3 | 30,375 | 408 | 1,016 | `linear`, `linear3`, `hyper3`, `plastic3` (J2), `frame2d`, `cohesive`, feature-gated IPC contact | Slope and pillar FEM **only after a geomaterial constitutive model is added**. Cohesive elements could model joints. |
| fs-contact | L3 | 8,171 | 85 | 358 | Contact response curves | Low |
| fs-dcontact | L3 | 1,355 | 34 | 188 | Instrumented penalty contact | Low |
| fs-lbm | L3 | 39,721 | 581 | 1,597 | D2Q9, D3Q19, D3Q27 lattices; `rheology::Rheology::{Newtonian, PowerLaw, Carreau}` (no Bingham or Herschel-Bulkley); `freesurface` VOF; `terrain_wind` (DEM here means digital elevation model); conjugate Darcy heat | Slurry and tailings flow once yield-stress rheology exists; dam-break runout; dust and wind over pits |
| fs-flux | L3 | 5,398 | 61 | 300 | 2D BDM Navier-Stokes | Low |
| fs-convection | L3 | 1,729 | 35 | 225 | `CorrelationCard` pattern | **Pattern to copy** for Bond-law, slurry-friction and wear correlation cards |
| fs-conduction | L3 | 40,721 | 706 | 1,675 | Steady -div(k grad T) = f on P1 tets; anisotropic k(T) from fs-matdb with receipts; `DeterministicPerIsa`; `transient` and `enthalpy` modules; the only L3 capability. Lib tests do not currently compile. | **Operator twin of confined steady Darcy seepage** (-div(K grad h) = 0). Transient form for consolidation-like diffusion. |
| fs-airflow | L3 | 19,878 | 397 | n/a | Fan curves plus quadratic-loss networks; `solve_operating_point` (interval Newton); `graph` module for arbitrary topologies (nominal, uncertified); `solve_with_fan` | **Mine ventilation networks** (Atkinson's p = R Q^2 is a quadratic loss) |
| fs-mbd | L3 | 9,996 | 98 | 473 | Featherstone articulated-body dynamics (AF) | Equipment dynamics, low |
| fs-scenario | L3 | 31,434 | 288 | n/a | Boundary conditions, `FrameTree`, ensembles, RANS Darcy-Forchheimer momentum sink, `Violation{code, what, fix}` | Scenario IR pattern; the porous sink is a CFD drag term, not groundwater |
| fs-dd | L3 | 1,618 | 17 | 133 | BDDC 2D, `[M]` (AF) | Low |
| fs-sparse | L1 | 6,507 | 101 | n/a | Coo, Csr, Bsr, Sell formats; `SaAmg`; LDL^T direct; AMD and nested-dissection ordering; `pcg`; `ilu0` (AF) | Seepage and FEM linear algebra; GMRF and SPDE random fields |
| fs-la | L1 | 12,399 | 191 | 740 | Dense linear algebra, `jacobi_eigh` | Small dense problems |
| fs-solver | L3 | 8,993 | 137 | 439 | CG, MINRES, GMRES, Newton, LSQR, p-multigrid | Nonlinear seepage and plasticity |
| fs-modal | L1 | 5,739 | 54 | 312 | Modal analysis | Low (mill and screen vibration) |
| fs-cutfem | L3 | 9,770 | 138 | 409 | 2D quadtree CutFEM, 3D `elastic3` | Analysis on implicit geometry without meshing (pit walls) |
| fs-fft | L1 | 2,130 | 38 | 250 | `FftNd` (power-of-two dimensions) | Circulant-embedding Gaussian random fields on block grids |
| fs-feec | L3 | 21,407 | 174 | 1,059 | Finite element exterior calculus | Mixed Darcy formulations (mass-conservative seepage) |
| fs-couple | L3 | 170,766 | 3,799 | 2,562 | Multiphysics coupling hub; some lib tests do not compile | Large; avoid direct dependency |
| fs-thermal | L3 | 3,670 | 57 | none | Shared enthalpy timelines (AF) | Heap thermal balance, low |
| fs-ladder | L3 | 3,725 | 43 | 261 | Fidelity graphs; Eca-Hoekstra grid-convergence uncertainty | Verification-ladder pattern |

### 3.5 Infrastructure, product and I/O

| Crate | Layer | src LOC | #[test] | CONTRACT | Key surface | Mining relevance |
|---|---|---|---|---|---|---|
| fs-exec | L0 | 33,866 | 270 | n/a | `Cx`, `ExecMode`, two-lane executor on asupersync | Cancellation and budget contract |
| fs-alloc | L0 | 8,826 | 99 | 262 | Arenas | Needed by `Cx` |
| fs-math | L0 | 3,711 | 72 | 230 | `det` deterministic transcendentals (AF) | Lift for determinism |
| fs-ivl | L1 | 5,460 | 126 | 384 | Interval arithmetic (AF) | Certified volume and tonnage bounds |
| fs-rand | L1 | 7,356 | 125 | n/a | Philox keyed by logical identity; Sobol/Owen scrambling (AF) | Reproducible realisations per block and per realisation index |
| fs-ad | L1 | 2,917 | 40 | 178 | Forward/reverse AD; optional `torch-bridge` | Gradients for calibration |
| fs-qty | UTIL | 8,889 | 95 | n/a | `Qty<M, KG, S, K, A, MOL>` with aliases (Length ... Density, Stress, DynViscosity, MassFlowRate, VolumetricFlowRate), `QtyAny`, `parse_qty`; units m, g, s, K, A, mol, N, Pa, J, W, V, C, Wb, H, Ohm, S, F, T, Hz, L, min, h, rad, deg, %, affine degC, SI prefixes; nightly `generic_const_exprs` (AF) | Units. **No tonne, troy ounce, ppm, g/t, day, year or currency.** Money "belongs to fs-ir". |
| fs-obs | UTIL | 9,495 | 73 | n/a | Observability (AF) | Logging |
| fs-blake3 | UTIL | n/a | n/a | n/a | Content hashing (AF) | Provenance |
| fs-time | L3 | 13,048 | 179 | n/a | Time integration | Transient solvers |
| fs-io | L2 | 22,115 | 145 | n/a | Quarantine-on-import; STL, OBJ, PLY, faceted STEP, gmsh; catalogue CSV/JSON | **No DXF, OMF, LAS/LAZ, E57, GeoTIFF or block-model CSV conventions. No CRS or EPSG support anywhere in the repo.** |
| fs-ledger | L6 | 38,145 | 310 | 1,460 | `Ledger`, `FiveExplicits`, `travel`, `explain`; maturity L1 | Design-ledger pattern |
| fs-session | L6 | 24,638 | 168 | n/a | `Governor`, `CapabilityToken`, `Guidance`, `estimate` | Agent and session governance pattern |
| fs-govern | UTIL | 25,007 | 261 | n/a | Governance and ratification (AF) | Pattern |
| fs-contract | L3 | 1,546 | 19 | n/a | `Contract`, `Envelope`, `compose` (AF) | Assume-guarantee composition |
| fs-casebook | UTIL | 1,113 | 13 | n/a | Case records (AF) | Pattern |
| fs-vvreg | UTIL | 20,243 | 183 | 912 | V&V benchmark registry with pinned editions, licences, deck hashes and acceptance envelopes (AF) | **Pattern for a mining benchmark registry** (for example slope-stability benchmarks or a Walker Lake-type geostatistics benchmark) |
| fs-checker | L6 | 3,851 | 52 | 446 | Standalone evidence-package verifier | Independent audit of study packages |
| fs-package | L6 | 10,080 | 102 | 574 | Evidence package format v9 (AF) | Deliverable packaging |
| fs-project | L6 | 15,407 | 152 | 650 | `.fsim` schema v15 | Project-file pattern |
| fs-report | L6 | 2,154 | 26 | n/a | `LabNotebook`, `semantic_diff`, `EngineeringReport` | Report generation |
| fs-viz | L5 | 6,745 | 59 | n/a | Streamlines, isosurface, VTU and XDMF export | Visual QA; ParaView export |
| fs-ir | L6 | 53,651 | 320 | 1,877 | FrankenScript IR (typed AST, s-expression and JSON forms) | Owns money semantics per fs-qty |
| fs-cli | L6 | 88,576 | 1,275 | 2,266 | `validate`, `import`, `solve`, `run`, `report`, `package`, `compare`, `discover` | CLI design reference |
| fs-wasm | L6 | 9,863 | 70 | 306 | Browser builds; REPAIR-OR-QUARANTINE verdict | Low |
| fs-substrate | L0 | 2,661 | 42 | 173 | Hardware probes and fingerprints | Low |
| fs-wedge | UTIL | 6,054 | 43 | 428 | Go-to-market wedge scoring | Shows that mining is **not** a named vertical |

---

## 4. Material DB

### 4.1 Pipeline

1. **Source bundles** live in `data/matdb/seed-v1/<bundle>/`. Each has a `manifest.tsv` plus one or more source files:
   - `properties.tsv` in `material-tsv-v1`
   - `interface-tsv-v1`
   - species files
   - `contact.tsv`
2. **Compile** with `cargo run -p xtask -- matdb-pack --manifest <M> --out <P>`. The compiler is `xtask/src/matdb_pack.rs`, about 8,843 lines.
   - Units are parsed with `fs_qty::parse_qty` and converted with `unit_transform`.
   - Manifest dialects v1 to v9 add property and axis mapping, typed axes, sample rows and `surface2d`.
   - `constitutive-v1` and `kinetics-v1` sources and NASA-9 data compile to `FSMODPK` model packs.
   - Binary packs are not committed.
3. **Store.** `fs-matdb-store` (FrankenSQLite) provides `MaterialStore` and `seal_corpus`. If sealed content changes after sealing, it raises `StoreError::CorpusChanged`.
4. **Query.** `ClaimSet::query(property, &QueryPoint, SelectionPolicy)` returns a `MaterialAnswer`; `query_typed` and `query_pinned_typed` are typed variants. A `MaterialAnswer` carries:
   - `PropertySample{value, dims, quantity, uncertainty}`
   - `PropertyUsageReceipt{schema_version, property, query_point, axis_quantities, considered, in_domain, selected, policy, decision: EvaluationDecision, observation_backed, evaluator_version, source_hashes}`
5. **Resolve.** `fs_material::state_point::resolve_material_state_point` resolves `ScalarPropertyRequirement` values into a `ResolvedMaterialStatePoint` for solvers. `fs-conduction` is the reference consumer, and attaches receipts to its results.

### 4.2 Source grammar (as used in seed bundles)

Manifest:
```
frankensim.matdb-manifest.v1
pack_id <id>
redistribution permitted <terms>
citation <text>
license <spdx-or-text>
source primary properties.tsv material-tsv-v1
```
Source rows:
```
frankensim.matdb-source.v1
observation <id> <specimen> <method> <process>
scalar <id> <obs> <property> <value> <unit> constant
uncertainty <id> unstated|<half-width forms>
validity <id> temperature <lo> <hi> degC
(curve / frame / joint rows for tabulated data)
```

### 4.3 Core types (`fs-matdb`)

- **`PropertyClaim`** has fields:
  - `key: PropertyKey`
  - `value: PropertyValue::{Scalar, Curve, Surface2D}`
  - `validity: ValidityDomain`
  - `uncertainty: UncertaintyModel::{Unstated, HalfWidth, RelativeHalfWidth}`
  - `interpolation: InterpolationPolicy::{LinearInside, ConstantWithinValidity, TabulatedOnly}`
  - `observations`
  - `provenance: Provenance{source, license, artifact}`. An empty licence is refused.
- **`ObservationDataset`** has fields `{specimen, method, artifact, caveats, provenance}`.
- **Cards:** `MaterialCard`, `ConstitutiveModelCard`, `InterfaceSystemCard`, `ContactLawCard`.
- **`SelectionPolicy`:** `SingleClaimOnly` or `PreferObservationBacked`.
- **No extrapolation.** A query outside every claim's validity box returns `MatDbError::NoClaimInDomain`. Multiple equally eligible claims return `AmbiguousSelection`.

### 4.4 Inventory and classes

Per `docs/MATERIAL_DATA_INVENTORY.md` (2026-09-10): 183 bundles (165 bulk, 8 interface, 7 species, 3 contact), 1,012 claims, about 0.88 MB, median of 4 properties per bundle. There are now 267 bundle directories under seed-v1.

Material classes present:
- **Steels:** AISI 1045, 4140, 52100, 9310, 4130, 4340.
- **Stainless steels:** 301, 304, 316, 347, 17-4PH, 1.4401.
- **Other metals:** gray cast iron; aluminium alloys; copper (pure, OFHC, C11000, annealed); pure tin, lead, zinc, silver, nickel, iron, titanium and tungsten; brasses and bronzes.
- **Non-metals:** woods, glasses, polymers.
- **CFAST construction materials:** normal and lightweight concrete, `concrete-nsc-mix-iv-nistir6475`, cement mortar, clay and common brick, gypsum, calcium silicate, insulations.
- **Fluids:** water (IAPWS, 10-80 degC at 0.1 MPa), air and other gases, oils and coolants.
- **Specialist:** magnets and M19 electrical steel, biological tissues, musical strings.

**Absent:** rock, minerals, ore, soil, sand, gravel, slurry, tailings, abrasion-resistant liner steels (Hardox, AR400/AR500, Hadfield manganese steel, Ni-hard and high-chrome white irons), rubber liner compounds, and ore hardness or comminution indices (Bond Wi, Ai, JK A x b, SMC DWi).

The expansion plan says: "Asphalt/bitumen, sand, gravel, clay/soil, granite and limestone - No dedicated bulk bundles ... need stateful granular/porous constitutive support as well as data." `docs/MATERIAL_PROPERTY_TAXONOMY.md` has no geomechanics section. The policy is "populate on demand, named refusal otherwise".

### 4.5 What it takes to add a pack ("what counts as delivery", from the expansion plan)

1. Sources land in `seed-v1` with citation, licence and redistribution terms.
2. The pack compiles and ingests.
3. Discovery and resolution work, including the refusal path.
4. An existing consumer runs against it.
5. The inventory is updated.

### 4.6 Mining packs: what fits now and what does not

| Pack | Example properties and axes | Fits the current format? | Blocking gaps |
|---|---|---|---|
| **Intact rock and rock mass** | Density, UCS (Pa), Young's modulus, Poisson's ratio, tensile strength, Hoek-Brown `mi`, GSI, disturbance D, `mb`/`s`/`a`, Mohr-Coulomb c' and phi', joint stiffness Kn/Ks, Barton JRC/JCS | Scalars yes, as `scalar` rows. Validity axes such as confining stress or depth need typed-axis support for those quantities (manifest v5+ typed axes; must confirm which axis quantities are admitted). | There is no Mohr-Coulomb or Hoek-Brown model in `fs-material` to resolve these into, so the data has no consumer and fails delivery step 4. There is no `PropertyKey` vocabulary for rock-mass indices (a governed taxonomy addition). GSI, RMR and Q are classifications with heavy judgement and should be Estimated colour. |
| **Bond work index and comminution** | Bond ball-mill Wi, rod-mill Wi, crushing Wi, abrasion index Ai, JK A x b, SMC DWi, SG | Values yes. Unit: kWh/t is energy per mass, which `fs-qty` can represent as J/kg, but "t" is not a parsed unit, so values must be converted or the unit table extended. Wi depends on the closing screen size (P100), which needs an axis. | Wi is a property of an **ore sample, not a material grade**. Spatially varying ore properties belong in the block model or geometallurgical model, not in a global material DB. The DB fits only for reference ores and test standards. Bond's law itself is a correlation (a `CorrelationCard` pattern), not a property. |
| **Slurry rheology** | Yield stress tau_y (Pa), Bingham plastic viscosity (Pa s), Herschel-Bulkley K and n, settling velocity, solids SG, as functions of solids concentration (Cw or Cv) and temperature | Curves and `Surface2D` over concentration and temperature are representable **if a concentration axis is admitted**. That needs a typed-axis check, and possibly a frozen-schema change under `schema-policy.json`. | `fs-lbm::rheology::Rheology` has no Bingham or Herschel-Bulkley variant. Rheology is strongly site- and grind-specific (PSD, clay mineralogy, pH), so validity domains must capture PSD and mineralogy or refuse. |
| **Liner and wear steels** | Hardness (HB), UTS, yield, toughness (CVN), Archard wear coefficient k against specific abrasives, manganese-steel work hardening | Bulk properties fit like the existing AISI steels. Wear coefficients fit as `InterfaceSystemCard` or `contact.tsv` (pairwise, abrasive-specific), consumed by `fs-tribo::ArchardLaw`. | Proprietary grades (Hardox and similar) bring licensing and redistribution constraints. Wear coefficients depend strongly on the test (ASTM G65 dry sand/rubber wheel, impact-abrasion, wet milling), so `method` and `process` rows matter. Mill-liner wear needs charge dynamics (DEM), which is absent. |
| **Soils and tailings** | Gradation, Atterberg limits, permeability K (m/s), void ratio, Cc/Cv, critical-state parameters, unsaturated SWCC (van Genuchten) | Scalars and SWCC curves are representable | No porous or critical-state constitutive model and no Richards solver. Plan section 5.13 is "planned, not built". |

### 4.7 Recommendation for materials

- Keep FrankenMining geomaterial data **in the FrankenSim pack format** (`frankensim.matdb-manifest.v1` with `material-tsv-v1` sources). This honours "no second material database" and leaves an upstream path open.
- Store client-confidential test data in **sealed project corpora** (the `fs-matdb-store` `seal_corpus` pattern), never in public seed bundles.
- Separate the three kinds of knowledge:
  1. Material properties go in the DB.
  2. Spatially variable ore and rock properties go in the block or geometallurgical model, as random fields with evidence colour.
  3. Empirical laws go in correlation cards (Bond, Hoek-Brown, Archard, Durand/Wasp slurry friction, Atkinson), with a `ValidityDomain` and Validated or Estimated colour, following `fs-convection::CorrelationCard` and doctrine D6.

---

## 5. Mapping mining needs to FrankenSim building blocks and gaps

Legend:
- **Route:** Lift (copy with provenance), Dep (dependency after the spike), Pattern (copy the design, not the code), Build (new in FrankenMining).
- **Gap size:** S (weeks), M (1-3 months), L (quarter or more), for a focused team. These are rough planning numbers, not estimates with evidence.

| # | Mining need | FrankenSim building blocks | What is missing | Route | Gap |
|---|---|---|---|---|---|
| 1 | **Block models and sparse grids** | `fs-rep-sdf::VdbGrid<T: Copy>` (sparse BTreeMap root, 32^3 internal nodes, 8^3 leaves; T can be a Copy struct of attributes); `fs-rep-voxel::MaterialField` (u16 domain codes) and `DensityField`; `fs-geocon::volume_certified`; `fs-mesh::conservative_cell_remap` | Sub-blocking and octree parent/child blocks; rotated and offset grid frames; multi-variable attribute storage (columnar, f64, strings); regularisation and re-blocking with tonnage-grade conservation; the 10^9-voxel target is unbenchmarked; `euclidean_dt` densifies the bounding box (memory risk); Datamine, Vulcan, Surpac and CSV block-model I/O | Lift `VdbGrid` (strip `Cx`) or Dep, then Build the block-model layer | M |
| 2 | **Orebody wireframes and implicit surfaces** | `fs-rep-mesh` (`HalfEdgeMesh`, `winding_exact`, `WindingOctree` for robust inside tests on non-watertight wireframes, `repair`, `mesh_to_sdf`, `dual_contour` for extraction); `fs-rep-frep` (CSG with hard or blended booleans, half-spaces for faults); `fs-rep-sdf::AdaptiveSdf`; `fs-topo` (manifold and self-intersection certificates); `fs-cutfem` (analysis on implicit geometry) | Implicit interpolation from contact points and orientations (Hermite RBF or dual kriging, potential-field/co-kriging geology modelling); fault-network stratigraphic sequencing; vein and lode modelling; uncertainty of contacts; DXF, OMF and string-file I/O. `fs-xform` RBF is morphing, not interpolation. | Dep or Lift for geometry; Build the implicit modeller | L |
| 3 | **Pit shells and stopes** | `fs-geocon` (`min_thickness_soft` for minimum mining width, `draft_violations` as an analogue for slope-angle constraints, certified volume); `fs-rep-frep` envelopes; `fs-dfo` and `fs-bo` for shape-parameter search; FrankenNetworkx (fnx) may provide max-flow and min-cut (not verified in this survey) | Lerchs-Grossmann or pseudoflow ultimate pit; precedence-arc generation with variable slopes by sector; nested shells and revenue factors; stope optimisation (MSO-like); cut-off grade optimisation (Lane); economic block values (needs currency) | Build (maybe on fnx max-flow) | L |
| 4 | **Drillhole traces** | `fs-geom::Point3`, `Vec3`; `fs-query::raycast` and `closest_point` for hole-surface intersections; `fs-assimilate::point_sensor` as an observation operator | Collar, survey, assay and lithology tables; desurveying (minimum curvature, tangent); downhole compositing and sample support; QA/QC (standards, blanks, duplicates, possibly with `fs-eproc` sequential tests); intersection with wireframes; CSV/DB import | Build (small) | S |
| 5 | **As-built drone or lidar versus design** | `fs-asbuilt` (`register3`, `register3_similarity`, `register3_datum`, `DeviationField`, `ThicknessField`; Estimated colour; known correspondences only; `as_built_diff` works on `Point2` profiles); `fs-query::closest_point` (point-to-mesh deviation); `fs-rep-voxel::PointCloud`; `fs-geocon::volume_certified` for cut/fill and stockpile volumes; `fs-ivl` for enclosures | ICP or robust correspondence-free registration; LAS/LAZ, E57 and GeoTIFF readers; **CRS and EPSG transforms (absent repo-wide)** and local mine grids; DTM triangulation from dense clouds with outlier handling; noise models per sensor; reconciliation reporting (overbreak, underbreak, dilution) | Pattern and Lift (registration math); Build I/O, CRS and ICP | M |
| 6 | **Gaussian random fields and conditional simulation** | `fs-uq::KlExpansion` (dense, isotropic, unconditional, O(n^3), so only a few thousand points); `fs-fft::FftNd` (power-of-two dimensions, the base for circulant embedding); `fs-sparse` `SaAmg` and LDL^T (base for an SPDE/GMRF approach); `fs-rand` Philox keyed by logical identity (reproducible per-realisation and per-block streams); `fs-bo` GP (Matern ARD) | Experimental variograms and model fitting; anisotropy (rotated ranges); kriging (OK, SK, UK, co-kriging) with neighbourhood search; SGS and turning bands; normal-score and Gaussian anamorphosis back-transforms; indicator and multiple-point simulation (for categorical domains); change of support; scale to about 10^7 nodes. **This whole layer is absent.** | Build (Lift `fs-rand` and parts of `fs-sparse` and `fs-fft`) | L |
| 7 | **Bayesian updating** | `fs-assimilate` with feature `ensemble` (serial EnSRF O(n x members) per observation, localisation, `EnsembleSmoother`); dense `Belief` capped at 256 states and 4,096 observations; `assimilate_colored` keeps evidence colour; `fs-eproc` | Observation operators for assays over intervals (support averaging), grade-control blasthole data and production reconciliation; non-Gaussian handling (log-normal grades, categorical domains); localisation tuned to variogram ranges; scale tests at block-model size | Dep or Lift for EnSRF; Build operators | M |
| 8 | **VOI and drillhole placement** | `fs-voi` (`ranking_flip_probability`, `expected_opportunity_loss`, `top_two_evpi_surrogate`, `KnowledgeGradient`, `recommend`); `fs-oed-e2e` SensorForge `run_campaign` (greedy VOI placement plus an EVPI stopping rule) | Candidate hole geometry (collar, azimuth, dip, length) as a design variable; line-support observation model along the trace; per-metre cost and access constraints; the decision model linking information to economics (pit or stope value, classification upgrade) | Lift `fs-voi` (AF); Pattern from `fs-oed-e2e`; Build the mining decision model | M |
| 9 | **Slope stability** | `fs-solid` (`plastic3` J2 only; `cohesive` for joints; `frame2d`); `fs-mesh` meshing; `fs-cutfem`; `fs-uq` fragility (`ida_fragility`) and `fs-robust::fragility_curve` (pattern for probability of failure); `fs-surrogate` emulators | Mohr-Coulomb and Hoek-Brown constitutive models (non-associated, with tension cut-off); shear strength reduction (SSR) factor-of-safety driver; limit-equilibrium methods (Bishop, Spencer, Morgenstern-Price) as a fast, independent second method; kinematic analysis of discontinuities (planar, wedge, toppling); pore-pressure coupling from seepage; 3D benches | Build (constitutive models and SSR may justify upstream contribution to `fs-material` and `fs-solid`) | L |
| 10 | **Comminution energy and liner wear** | `fs-convection::CorrelationCard` pattern; `fs-dimine` (pi-space power-law fitting, Estimated colour, frozen); `fs-regime` (Buckingham-pi admission); `fs-tribo::ArchardLaw` and `DryInterfaceSystemCard`; `fs-qty` energy per mass | Bond's law card, E = 10 Wi (1/sqrt(P80) - 1/sqrt(F80)) with microns and kWh/t; JK and SMC models; population-balance models; mill power models (Morrell, Hogg-Fuerstenau); DEM for charge motion and liner wear (**no DEM crate exists**); wet impact-abrasion wear laws | Pattern and Build | M (Bond and power cards), L (DEM and liner wear) |
| 11 | **Slurry and tailings flow and seepage** | Flow: `fs-lbm` (`freesurface` VOF; `rheology` Newtonian, power-law, Carreau). Seepage: `fs-conduction` (steady -div(k grad T) operator is the same as confined Darcy -div(K grad h); anisotropic k; transient module); `fs-feec` (mixed forms); `fs-sparse`, `fs-solver` | Bingham and Herschel-Bulkley rheology (regularised Papanastasiou); pipe-flow friction models (Durand, Wasp, settling and deposition velocity); tailings beach and dam-break runout validation; **unconfined seepage with a phreatic surface (free boundary)**; Richards unsaturated flow; consolidation (Biot), all "planned, not built" in plan section 5.13; filter and drain boundary conditions | Pattern from `fs-conduction` (lift or reimplement the P1 operator); Build rheology and Richards; upstream Bingham/HB in `fs-lbm` is plausible | M (confined seepage, slurry cards), L (Richards, Biot, runout) |
| 12 | **Heap leach** | `fs-thermochem` (single-reaction equilibrium, NASA-9); `fs-matdb` `kinetics-v1` model packs; `fs-lbm` conjugate Darcy heat; `fs-conduction` `transient` and `enthalpy`; `fs-thermal` | Variably saturated flow in heaps (Richards); preferential flow and channelling; shrinking-core or particle-scale leach kinetics; reactive transport (multi-species advection-diffusion-reaction); heat from sulfide oxidation; solution management | Build | L |
| 13 | **Ventilation** | `fs-airflow` (fan curves; quadratic-loss networks equivalent to Atkinson p = R Q^2; `solve_operating_point` by interval Newton; `graph` module for arbitrary topologies, nominal only; `solve_with_fan`); `fs-thermochem` (mixtures, Wilke) | Hardy Cross or Newton network solves at 10^3-10^4 branches (scale unverified); natural ventilation pressure (density and depth); regulators and doors; heat and humidity (psychrometrics, wet-bulb) and strata heat; gas and dust (CH4, diesel particulates) transport; ventilation-on-demand optimisation; certification of the graph solver | Dep or Lift `fs-airflow` network core (check closure); Build the mine-specific physics | M |
| 14 | **Robust optimisation** | `fs-robust` (`cvar`, `empirical_cvar`, `robust_optimum`, `ColoredObjective`, AF); `fs-dfo` (`dro`, NSGA-III, CMA-ES); `fs-uq::chance_constrained_min`; `fs-bo` (expensive objectives, multi-fidelity); `fs-archive` (MAP-Elites) | Integration with mine-planning formulations (scenario-based stochastic MILP over geological realisations); risk measures for NPV with metal-price and grade uncertainty | Lift (AF crates); Build formulations | M |
| 15 | **Scheduling and critical path** | `fs-tropical` (`TaskDag`, `critical_path`, `slack`, Karp `max_cycle_mean`, no dependencies, AF); `fs-schedule-e2e`; FrankenScipy `fsci-opt` `linprog` and `milp`; `fs-constraint` (unsat cores and ranked repairs for infeasible schedules) | Resource-constrained scheduling (RCPSP: fleets, crews, mill capacity); block and bench precedence; long-term MILP at mining scale (millions of variables, needing aggregation, Lagrangian or decomposition; FrankenScipy MILP scale unverified); short-term fleet dispatch; discounting (needs currency and time units). `fs-opt` has no integer variables. | Lift `fs-tropical`; Build the scheduler (evaluate `fsci-opt::milp`) | L |
| 16 | **Units** | `fs-qty` (`Qty<M,KG,S,K,A,MOL>` const-generic dimensions; `parse_qty`; SI prefixes; affine degC) | tonne, kt and Mt; troy ounce; grade units (g/t, ppm, ppb, %, oz/t, lb/t); day, week and year (and Julian versus calendar); kWh/t; currency and price units (USD/t, USD/oz, USD/lb) are deliberately not in `fs-qty` ("money belongs to fs-ir"); bulk versus in-situ density and swell factor; "wet versus dry tonnes" as a semantic, not dimensional, distinction | Lift `fs-qty` (forces nightly) or Build a stable-Rust units layer modelled on it; add a separate typed money module | S-M |
| 17 | Cross-cutting: evidence, provenance and reporting | `fs-evidence`, `fs-blake3`, `fs-package`, `fs-checker`, `fs-report`, `fs-ledger::FiveExplicits`, `fs-vvreg` | Mapping to JORC, NI 43-101 and S-K 1300 language (Competent or Qualified Person sign-off, classification categories, modifying factors); a mining benchmark registry | Lift and Pattern; Build the reporting mapping | M |
| 18 | Cross-cutting: I/O and coordinates | `fs-io` (quarantine-on-import pattern; STL, OBJ, PLY, STEP, gmsh; catalogue CSV and JSON); `fs-viz` (VTU and XDMF export) | DXF, OMF (Open Mining Format), LAS/LAZ, E57, GeoTIFF and DEM rasters, Datamine, Vulcan, Surpac and Leapfrog exchange, drillhole CSV conventions; **CRS and EPSG, local grid transforms, vertical datums** | Pattern (quarantine); Build everything else | M |

---

## 6. Risks and gaps

### 6.1 Platform risks

1. **Non-convergence and churn.**
   - The workspace does not fully compile (`cargo check --workspace --all-targets` exits 101; 15 targets fail, including `fs-conduction`, `fs-couple` lib tests and `fs-cli` cooling tests).
   - The tree is dirty (3,769 paths) and receives about 100 commits per day.
   - The reality check reports Journey A as indeterminate (8 of 8 NO-DATA), new-domain flagships at 0 of 66 and theorem lanes at 0 of 79 closed.
   - **Mitigation:** pin by commit hash. Lift instead of depend. Re-verify lifted code with golden tests.
2. **Low maturity.** Only one capability is at L3, and most of what mining needs is L1-L2 or untracked. Anything FrankenMining builds on these blocks must claim Estimated or Validated colour at most until it has its own validation. "Verification is not validation" applies with full force: a converged FoS is not a correct FoS.
3. **Toolchain.** `nightly-2026-07-06` plus `generic_const_exprs` in `fs-qty` risks compiler ICEs and blocks stable Rust. The suite already runs mixed nightlies (FrankenTextiles uses `nightly-2026-08-31`).
4. **asupersync and `Cx` coupling.** All geometry requires `Cx`. A git dependency is likely broken by the out-of-repo `../../../asupersync` paths. A path dependency requires the exact sibling layout and a compatible asupersync. franken_accounting shows how to unify asupersync with `[patch.crates-io]`, but that has not been tried with FrankenSim.
5. **Scale is unproven where mining needs it most.**
   - The `VdbGrid` 10^9-voxel target is unbenchmarked.
   - `euclidean_dt` densifies.
   - `KlExpansion` is O(n^3) dense.
   - The dense `Belief` is capped at 256 states.
   - Ventilation-graph and MILP scale are unverified.
   - Block models (10^6-10^8 blocks times 100+ realisations) are well beyond any tested size.
6. **Governance friction for upstream work.** Displacement rule `f85xj.16.3`, frozen `schema-policy.json` schemas (matdb pack v7 lockstep; new axis quantities or property keys are governed), one ratified vertical (Cooling 0.1), and shared-tree rules (no branches or worktrees, no deletions, file reservations). Mining-driven changes upstream will be slow.
7. **Licence.** MIT plus an OpenAI/Anthropic rider. Confirm compatibility with FrankenMining's licence and with any client redistribution before lifting.

### 6.2 Capability gaps (consolidated)

| Area | Present? | Notes |
|---|---|---|
| Geostatistics (variograms, kriging, SGS, MPS) | Absent | Largest single gap; core of any mining product |
| Geomechanical constitutive models (Mohr-Coulomb, Hoek-Brown, Drucker-Prager, critical state) | Absent | Blocks slope, pillar and tailings analysis |
| Limit equilibrium, SSR, kinematic analysis | Absent | |
| Pit optimisation (LG, pseudoflow), stope optimisation, cut-off grade | Absent | |
| Drillhole data model and desurvey | Absent | Small to build |
| Unsaturated or unconfined seepage, Richards, Biot | Absent | Planned only (plan section 5.13) |
| Yield-stress rheology | Absent | `fs-lbm` has Newtonian, power-law and Carreau only |
| DEM (particles) | Absent | Needed for liner wear and charge motion |
| Comminution models | Absent | Bond law fits the card pattern |
| ICP, correspondence-free registration | Absent | `fs-asbuilt` needs known correspondences |
| CRS, EPSG, mining file formats (DXF, OMF, LAS, E57, GeoTIFF) | Absent | |
| Mining units and currency | Absent | `fs-qty` deliberately excludes money |
| Integer and resource-constrained scheduling | Absent in FrankenSim | `fsci-opt::milp` exists in FrankenScipy (scale unverified) |
| Geomaterial data | Absent | Packs would have no consumer until constitutive models exist |

### 6.3 Doctrine traps

- **False-friend crate names.** `fs-dimine` is dimensional knowledge mining and is frozen. `fs-frame` is a seismic frame flagship.
- **Colour versus classification.** Evidence colour (Verified, Validated, Estimated) is a numerical and model provenance claim. Resource classification (Measured, Indicated, Inferred) is a regulated Competent Person judgement. They must be distinct types, linked only by documented mapping rules.
- **Lift drift.** Every lifted file needs a provenance header (source path and commit) and a golden or conformance test, as in FrankenTextiles' `det.rs` with `frankensim_core_golden_hash`. Without them, lifted copies will silently diverge from bug fixes upstream.
- **Data sovereignty.** The `fs-matdb` licence-required rule (an empty licence is refused) is right for public packs. Mining test data is usually client-confidential, so FrankenMining needs sealed private corpora and must never write confidential data into shared seed bundles.

### 6.4 Suggested first actions for FrankenMining planning

1. **Spike in a separate clone** (not a worktree in the FrankenSim tree). Pin a FrankenSim commit, build a scratch crate depending on `fs-rep-sdf`, `fs-rep-mesh` and `fs-sparse`, and record whether the path and git dependency routes work and what `Cx` setup costs.
2. **Adopt the conventions** in section 1 for FrankenMining: contract per crate, typed refusals with ranked fixes, the Five Explicits, determinism classes, and evidence colour with no laundering.
3. **First lift set** (all asupersync-free): `fs-math::det`, `fs-rand` (Philox), `fs-evidence` core types, `fs-voi`, `fs-robust`, `fs-tropical`, `fs-eproc`. Decide separately on `fs-qty` (nightly) versus a stable-Rust units layer.
4. **Build-new backlog, ordered by dependency:** units and money → drillholes → block model on `VdbGrid` → variograms and kriging → conditional simulation (FFT, SPDE) → EnSRF observation operators → VOI drill planning → pit optimisation → scheduling. The geomechanics track (Mohr-Coulomb and Hoek-Brown, SSR and LEM) runs in parallel with the seepage track (confined Darcy via the `fs-conduction` operator pattern, then Richards).

---

## Appendix A: Verification commands (read-only)

```
# HEAD and churn
git -C /Users/jemanuel/projects/frankensim log -1 --format='%h %cd'
git -C /Users/jemanuel/projects/frankensim status --porcelain | wc -l

# asupersync path coupling
grep -n asupersync /Users/jemanuel/projects/frankensim/crates/fs-exec/Cargo.toml
grep -n -A2 'patch.crates-io' /Users/jemanuel/projects/frankensim/Cargo.toml

# Nightly feature
grep -n generic_const_exprs /Users/jemanuel/projects/frankensim/crates/fs-qty/src/lib.rs

# Negatives (expect only false positives such as Eca-Hoekstra and Herschel-Quincke)
rg -il 'kriging|variogram|mohr.?coulomb|hoek.?brown|drucker|bingham|herschel.?bulkley|richards|discrete[- ]element|epsg' /Users/jemanuel/projects/frankensim/crates

# Materials inventory
ls /Users/jemanuel/projects/frankensim/data/matdb/seed-v1 | wc -l
```

Key FrankenSim docs consulted:
- `/Users/jemanuel/projects/frankensim/AGENTS.md`
- `/Users/jemanuel/projects/frankensim/docs/CONVENTIONS.md`
- `/Users/jemanuel/projects/frankensim/docs/CLAIM_INTEGRITY.md`
- `/Users/jemanuel/projects/frankensim/docs/DETERMINISM_CLASSES.md`
- `/Users/jemanuel/projects/frankensim/docs/CONTRACT_TEMPLATE.md`
- `/Users/jemanuel/projects/frankensim/docs/MATURITY_LEVELS.md`
- `/Users/jemanuel/projects/frankensim/docs/REALITY_CHECK_2026-09-01.md`
- `/Users/jemanuel/projects/frankensim/docs/MATERIAL_DATA_INVENTORY.md`
- `/Users/jemanuel/projects/frankensim/docs/MATERIAL_PROPERTY_TAXONOMY.md`
- `/Users/jemanuel/projects/frankensim/docs/FRANKENTEXTILES_INTEGRATION.md`
- `/Users/jemanuel/projects/frankensim/COMPREHENSIVE_PLAN_TO_EXTEND_FRANKENSIM_TO_NEW_DOMAINS.md`
- `/Users/jemanuel/projects/frankensim/xtask/src/matdb_pack.rs`
- `capability-maturity.json`, `schema-policy.json` and `consolidation-review.json` at the root of the FrankenSim repo
