# R3: Numerics, ML, Perception and Document-Ingestion Survey of the Franken Suite (for FrankenMining)

Survey date: 2026-10-28. Read-only survey: none of the surveyed repos were modified.
Method: README and AGENTS.md reads, workspace `Cargo.toml` member lists, `git log -1`, source greps to
confirm specific APIs, and two ripgrep term sweeps across 251 primary repos in `/Users/jemanuel/projects`.
The sweeps excluded worktree, release-wave and alias duplicates; `target/`, `node_modules/`, `.git/` and
venvs; and vendored upstream trees (`legacy_*`, `reference/`, `vendor/`, `third_party/`). They covered
code files only.

Size columns:
- **raw .rs lines** is `wc -l` over every `.rs` file outside `target/`, so it includes tests, fixtures and
  generated code.
- **test markers** is a grep count of `#[test]`, `#[tokio::test]` and `proptest!` lines.

Where a README states its own figures, those are quoted as well. Treat all counts as rough evidence of
size, not as quality claims.

---

## 0. Executive summary

1. **frankenscipy is the backbone.** It provides dense and sparse linear algebra, optimization (including
   LP and MILP), interpolation, spatial indexing, statistics, signal processing, ndimage and ODEs. It is
   the most mature numerics repo:
   - 19 crates, published on crates.io at 0.4.0.
   - 10,921 `#[test]` functions and 781 live-SciPy differential test files.
   - Census coverage of SciPy 1.17.1 symbols: 99.1% "real" and 57.9% "differentially compared".

   Its LP/MILP is **dense-row and small-to-medium scale**, and its max-flow uses **int32 capacities**. Both
   are unfit for production mine scheduling and block-model pit optimization.
2. **frankensim is the hidden gem**, although it was not in the original survey list. It has 164 native
   crates and about 2.85M raw .rs lines. It contains:
   - Matérn Gaussian processes, exact and sparse (Titsias ELBO), in `fs-bo`.
   - Karhunen–Loève random fields, multilevel Monte Carlo and QMC in `fs-uq`.
   - Linear-Gaussian data assimilation and Kalman updates in `fs-assimilate`.
   - Value of information (`fs-voi`) and optimal experimental design (`fs-oed-e2e`), which fit
     infill-drilling design directly.
   - CMA-ES in `fs-dfo` and CVaR/robust optimization in `fs-robust`.
   - Supernodal multifrontal sparse LDLᵀ with AMD ordering in `fs-sparse::direct`.
   - Point clouds and VDB voxels in `fs-rep-voxel`, octree SDFs in `fs-rep-sdf`, and exact-predicate
     Delaunay tet meshing in `fs-mesh`.
   - Isosurfaces in `fs-viz`, dual contouring in `fs-rep-mesh`, and 3-D Kabsch/Umeyama scan-to-design
     registration in `fs-asbuilt`.
   - PLY/STL/OBJ/STEP/glTF I/O in `fs-io`.

   Its self-declared maturity is low: 11 capabilities at L2, 3 at L1, 1 at L3, and nothing at L4 or L5.
3. **Bayesian inference has substrate but no samplers.** No repo implements MCMC, HMC, NUTS, SMC or
   general variational inference. `frankenjax` (grad, vmap, jit, hessian, scan/while_loop, ThreeFry RNG)
   is the natural substrate for writing them.
4. **There is no geostatistics anywhere.** No kriging, variogram or sequential Gaussian simulation turned
   up in any repo's code. The building blocks exist: KD-tree (`fsci-spatial::KDTree`), Cholesky and solves
   (`fsci-linalg`, `fs-la`), Matérn GP (`fs-bo`), Hermite polynomials (`fsci-special`, for Gaussian
   anamorphosis), and QMC/Philox RNG.
5. **Pit optimization needs its own kernel.** Max-flow exists in three places:
   - `fsci-sparse::maximum_flow` (Dinic or Edmonds–Karp on CSR, int32 capacities).
   - `fnx-algorithms` (`preflow_push` emulating NetworkX step for step including CPython set order, plus
     Edmonds–Karp on string-keyed attribute graphs).
   - `fss-graph-algorithms` and `fa-lantern` (internal Edmonds–Karp).

   None is a scalable Hochbaum pseudoflow or Lerchs–Grossmann. No repo has pseudoflow, LG or nested-pit
   parametric flow.
6. **GIS/CRS is a total gap.** No GeoTIFF, no TIFF raster I/O in the numerics suite, no PROJ/EPSG/WGS84/UTM
   transforms, and no shapefile or GeoJSON parsing in the suite. Point clouds are also a gap: no LAS/LAZ,
   no ICP, no ground filtering.
7. **Perception is early-stage.**
   - `franken_drone_geometry_reconstruction` (fdgr) stops at an "audited relative bundle problem": no
     bundle adjustment, no sparse or dense reconstruction.
   - `franken_surveillance_system` (fss, not on the original list) has a real Levenberg–Marquardt
     Schur-complement **bundle adjustment with surveyed control points (GCPs)** and covariance in
     `fss-geometry::bundle`. It also has Kalman tracking and H.264/H.265/MJPEG decoders.
8. **Document ingestion is usable but partial.**
   - `franken_ocr` (`focr`, v0.8.0 binary release) handles scanned PDFs (JPEG, CCITT G4 and Flate
     rasters) through Unlimited-OCR. It also offers GOT-OCR2 for structure and OneChart for chart-to-data,
     which suits grade-tonnage curves.
   - It **refuses born-digital vector/text PDF pages** and JPX/JBIG2 images, so modern NI 43-101 PDFs need
     a text-layer extractor that the suite does not have.
   - `franken_nlp` (Nanbeige4.2-3B, schema-constrained extraction) exposes only a "candidate" surface, and
     its build-proof authority is recorded as BLOCKED.
9. **Speech, diagrams and reports are covered.**
   - `franken_whisper` (`fw`) has a native large-v3-turbo engine, Sortformer diarization for up to 4
     speakers, and live `fw robot listen`. That covers shift-handover transcription.
   - `frankenmermaid` renders flowcharts (process flowsheets), Gantt charts (schedules) and Sankey
     diagrams (metal accounting).
   - `franken_markdown` (`fmd`) produces HTML, PDF and EPUB reports.
10. **Suite conventions FrankenMining must adopt to compose cleanly** (stated in every AGENTS.md checked):
    - Rust 2024 on nightly.
    - `#![forbid(unsafe_code)]`.
    - **No tokio**: asupersync is the only allowed async runtime.
    - "RULE NUMBER 1: NO FILE DELETION".
    - Heavy builds go through `rch exec -- cargo ...`.
    - Sibling path dependencies are common; only some crates are published on crates.io.

---

## 1. Per-repo fact sheets: core numerics, ML and perception

### 1.1 frankenscipy (`fsci-*`)
- **Path:** `/Users/jemanuel/projects/frankenscipy`
- **Last commit:** `df0496783 2026-10-26 feat(sparse): add block and interval exponential actions`
- **Size:** 1,827 .rs files, about 1.52M raw .rs lines, 11,125 test markers. The README states about
  722,500 lines under `crates/*/src`, 10,921 `#[test]` functions, and 846 conformance test files, of which
  781 are `diff_*` files that drive live SciPy.
- **Maturity evidence:**
  - All crates published on crates.io at `0.4.0` (Git tag `v0.2.0`), with `#![forbid(unsafe_code)]`.
  - Symbol census against SciPy 1.17.1: 1,525 of 1,539 applicable symbols "real" (99.1%), 891 compared
    differentially (57.9%).
  - A full CI fan-out passed in 2026-09.
  - Open V1.0 items: CASP outside linalg, CI streak stability, artifact topology.
- **Workspace (19 crates):** `fsci-linalg`, `fsci-sparse`, `fsci-opt`, `fsci-integrate`,
  `fsci-interpolate`, `fsci-fft`, `fsci-signal`, `fsci-spatial`, `fsci-special`, `fsci-stats`,
  `fsci-cluster`, `fsci-ndimage`, `fsci-io`, `fsci-constants`, `fsci-odr`, `fsci-datasets`, `fsci-runtime`
  (the CASP engine), `fsci-arrayapi`, `fsci-conformance`.
- **Mining-relevant capabilities, verified in source:**
  - **Optimization (`fsci-opt`)**
    - `linprog(c, a_ub, b_ub, a_eq, b_eq, bounds, LinprogOptions)`:
      - *Dense* `&[Vec<f64>]` constraint rows.
      - Bounded revised simplex plus a Mehrotra interior-point method, with marginals and sensitivity.
    - `milp(MilpProblem, MilpOptions{max_nodes, lp_maxiter})`:
      - Branch-and-bound over `linprog` relaxations.
      - The docstring says "small to medium problems".
      - No cuts, presolve, sparse matrices, warm-started dual simplex or MPS I/O.
    - `minimize` (14 methods, including SLSQP, trust-constr and COBYLA with constraints), `least_squares`
      (TRF, Dogbox, LM, robust losses), `curve_fit`, `nnls`, `isotonic_regression`.
    - Global optimizers: `differential_evolution`, `dual_annealing`, `basinhopping`, `shgo`, `pso`,
      `brute`, `direct`.
    - `linear_sum_assignment` (Jonker–Volgenant).
  - **Sparse (`fsci-sparse`)**
    - Formats: CSR, CSC, COO, BSR, DIA, DOK, LIL.
    - Direct solvers: `spsolve`, `splu`, `spilu`, `factorized`. There is **no sparse Cholesky**, matching
      SciPy.
    - Krylov solvers: CG, GMRES, LGMRES, BiCGSTAB, MINRES, LSQR, LSMR.
    - Eigensolvers: `eigs`, `eigsh`, `svds`.
    - csgraph: `dijkstra`, `bellman_ford`, `floyd_warshall`, `minimum_spanning_tree`,
      `connected_components`, `maximum_flow(csgraph, s, t, MaximumFlowMethod::{Dinic, EdmondsKarp})`,
      `maximum_bipartite_matching`, `min_weight_full_bipartite_matching`.
    - `maximum_flow` requires **integer capacities in int32 arithmetic, wrapping as SciPy's C does**.
  - **Linear algebra (`fsci-linalg`)**
    - LU, QR, Cholesky, SVD, LDL, eig/eigh, Schur, QZ, `expm`, Sylvester/Lyapunov/Riccati, banded solvers.
    - CASP-certified `solve_with_casp` and `lstsq_with_casp` return a `SolveCertificate` describing the
      conditioning-aware solver choice. This is useful for audit-grade kriging systems.
  - **Spatial (`fsci-spatial`)**
    - `KDTree` (`query`, `query_pairs`, `count_neighbors`); `pdist`, `cdist` (including Mahalanobis).
    - `ConvexHull`, `Delaunay`, `Voronoi` and `HalfspaceIntersection` are **N-dimensional** in source
      (a Qhull-option port: Delaunay up to 7-D, ConvexHull 2 ≤ ndim ≤ 8). The README still says "2-D", so
      the source is ahead of the docs.
    - `SphericalVoronoi`, `Rotation`, `RotationSpline`.
  - **Interpolation (`fsci-interpolate`)**
    - `griddata` (Nearest and Linear in N-D; Cubic Clough–Tocher in 2-D only).
    - `RbfInterpolator`, N-D, with kernels Linear, ThinPlateSpline, (Inverse)Multiquadric and Gaussian.
    - `RegularGridInterpolator`, `interpn`, `CubicSpline`, `BSpline`, `Akima`, `PCHIP`, `make_lsq_spline`.
  - **Statistics (`fsci-stats`)**
    - More than 95 continuous and more than 10 discrete distributions, with PDF/CDF/PPF, `fit` and moments.
    - `MultivariateNormal`, `Wishart`, `InvWishart`, `Dirichlet`, `DirichletMultinomial`.
    - `CensoredData`, useful for assays below detection limit and for right-censored equipment failures;
      `logrank_censored`.
    - `GaussianKde` and `GaussianKdeNd`, `bootstrap`, permutation tests, Box-Cox.
    - QMC: Sobol, Halton, Latin hypercube; `MultivariateNormalQmc`.
  - **Signal (`fsci-signal`), relevant to geophysical time series and profiles**
    - Filter design and application: `butter`, `cheby*`, `ellip`, `firwin`, `remez`, `filtfilt`, SOS.
    - Spectral analysis: `welch`, `spectrogram`, `stft`/`istft`, `lombscargle`.
    - Filtering and resampling: `detrend`, `savgol_filter`, `medfilt`, `wiener`, `decimate`, `resample`,
      `resample_poly`, `upfirdn`, `deconvolve`.
    - `hilbert2`, `find_peaks` (full SciPy condition set), CWT, `fftconvolve`, `convolve2d`.
  - **FFT (`fsci-fft`):** `rfft2`, `fftn`, `fftfreq`, `next_fast_len`, DCT/DST. These are the substrate for
    potential-field operators (upward continuation, reduction to pole, derivatives), which are not
    themselves present.
  - **Ndimage (`fsci-ndimage`)**
    - `gaussian_filter`, `sobel`, `label`, `find_objects`, `distance_transform_edt`.
    - Morphology, `zoom`, `map_coordinates`, `geometric_transform`.
    - This covers raster processing once the data is loaded.
  - **Integration (`fsci-integrate`):** `solve_ivp` (RK45, BDF, Radau, LSODA), `solve_bvp`, QUADPACK,
    `cubature`, `qmc_quad`. Relevant to leach, flotation and adsorption kinetics.
  - **Other**
    - `fsci-cluster`: KMeans, DBSCAN, hierarchical linkage, silhouette.
    - `fsci-odr`: orthogonal distance regression.
    - `fsci-io`: MAT, Matrix Market, WAV, a simplified NetCDF reader, IDL `.sav`.
- **Mining relevance: HIGH.** This is the default numerics provider. Its gaps for mining are scalable
  sparse LP/MILP, pseudoflow, geostatistics, geophysics-specific operators, and raster/CRS I/O.

### 1.2 franken_numpy (`fnp-*`)
- **Path:** `/Users/jemanuel/projects/franken_numpy`
- **Last commit:** `d32c4e04 2026-10-26 fix(linalg): solve coupled Schur blocks in eigenvector back-substitution`
- **Size:** 469 .rs files, about 662K raw .rs lines, 9,200 markers. The README states 9,086 `#[test]`.
- **Maturity evidence:**
  - All 499 names in `numpy.__all__` are reachable from `fnp_python`, and this is structurally locked.
  - Most families are marked "parity_green".
  - Crate version 0.5.0.
  - Caveats:
    - The Python layer delegates a number of paths, including most FFT entry points and 2-D LAPACK
      decompositions, to real NumPy.
    - The pure-Rust `UFuncArray` stores `Vec<f64>` internally.
    - The default build links no BLAS; large GEMM (above about 2000×2000) is slower than OpenBLAS.
- **Workspace (11 crates):** `fnp-dtype`, `fnp-ndarray`, `fnp-iter`, `fnp-ufunc`, `fnp-linalg`,
  `fnp-random-core`, `fnp-random`, `fnp-io`, `fnp-conformance`, `fnp-runtime`, `fnp-python`.
- **Mining relevance: MEDIUM.**
  - Uses: an N-D array substrate; bit-exact PCG64DXSM RNG (reproducible simulations);
    `.npy`/`.npz`/memmap I/O (large block-model arrays); histogram/binning; polynomials.
  - FrankenMining would more likely use `fsci-*` and `fs-*` types directly. fnp is useful for NumPy-format
    interchange and reproducible RNG streams.

### 1.3 frankenpandas (`fp-*`, facade `frankenpandas`)
- **Path:** `/Users/jemanuel/projects/frankenpandas`
- **Last commit:** `37bb94101 2026-10-26 merge(main): keep the rustfmt commit and the rust-deps bump`
- **Size:** 987 .rs files, about 1.18M raw .rs lines, 9,787 markers. The README states more than 8,700
  tests, 1,387 conformance packets against pandas 2.2.3, and 15 crates.
- **Workspace:** `fp-types`, `fp-columnar`, `fp-dot-kernel`, `fp-frame`, `fp-index`, `fp-expr`,
  `fp-groupby`, `fp-join`, `fp-io`, `fp-conformance`, `fp-bench`, `fp-runtime`, `fp-frankentui`,
  `fp-python`, `frankenpandas`.
- **Mining-relevant capabilities:**
  - I/O: CSV (full option matrix), fixed-width (`read_fwf`, for legacy assay and survey files), Excel
    (`.xlsx`, `.xls`, `.xlsb`, `.ods`), Parquet, Feather/Arrow IPC, SQL (`SqlConnection`, SQLite by default),
    JSON/JSONL, XML, HTML, Stata, SAS, and optional HDF5. ORC fails closed.
  - Time series: `rolling`, `expanding`, `ewm`, `resample`, `merge_asof` with tolerance and `by`, for fleet
    telemetry alignment.
  - `groupby` (three execution paths), `pivot_table`, `melt`, MultiIndex, Interval/Period types.
- **Mining relevance: HIGH** for drillhole, assay, telemetry and finance tables.
- **Gaps:** drillhole-specific interval operations (from/to overlap joins, length-weighted compositing,
  desurvey) must be built on top.

### 1.4 frankenjax (`fj-*`)
- **Path:** `/Users/jemanuel/projects/frankenjax`
- **Last commit:** `fa93fb77 2026-10-26 fix(api): keep soft-target gradients precise with masked teachers and offset logits (#1)`
- **Size:** 314 .rs files, about 494K raw .rs lines, 9,467 markers. The README states about 342K library
  lines, 17 crates, 861 oracle fixtures from JAX 0.9.2, and more than 9,400 tests.
- **Workspace:** `fj-core`, `fj-trace`, `fj-interpreters`, `fj-dispatch`, `fj-lax`, `fj-cache`,
  `fj-runtime`, `fj-conformance`, `fj-ledger`, `fj-ad`, `fj-egraph`, `fj-test-utils`, `fj-api`,
  `fj-backend-cpu`, `fj-backend-gpu` (a CPU-emulated sharding layer, not a GPU backend), `fj-ffi`, `fj-py`.
- **Capabilities:**
  - `fj_api::{jit, grad, vmap, value_and_grad, jacfwd, jacrev, hessian}` over a Jaxpr IR.
  - 157 local primitives, all with VJP and JVP rules.
  - Linear algebra: Cholesky, QR, SVD, Eigh, TriangularSolve, LU, Solve, Det/Slogdet, all differentiable.
  - Control flow: `cond`, `scan`, `while_loop`, `fori_loop`, `switch`.
  - ThreeFry2x32 PRNG with 22 samplers.
  - An e-graph optimizer.
- **Limitations:** CPU only, no XLA (slower on large dense tensors), and no optimizer library (no
  optax-equivalent).
- **MCMC/HMC/NUTS/VI: absent.** The only grep hit for "Adam" was a code comment.
- **Mining relevance: MEDIUM-HIGH** as the substrate for:
  - Differentiable geostatistics, such as maximum-likelihood variogram and GP hyperparameter fitting via
    differentiable Cholesky.
  - HMC/NUTS for Bayesian prospectivity and resource-uncertainty models.
  - Gradient-based metallurgical model calibration.

  FrankenMining would have to write the samplers itself.

### 1.5 frankentorch (`ft-*`)
- **Path:** `/Users/jemanuel/projects/frankentorch`
- **Last commit:** `10f31700 2026-10-28 feat(metal): declare the command-buffer fault-injection feature`
- **Size:** 811 .rs files, about 551K raw .rs lines, 6,423 markers.
- **Workspace (13 crates):** `ft-core`, `ft-dispatch`, `ft-kernel-cpu`, `ft-kernel-metal`, `ft-autograd`,
  `ft-device`, `ft-serialize`, `ft-api`, `ft-conformance`, `ft-runtime`, `ft-nn`, `ft-optim`, `ft-data`.
- **Capabilities:**
  - `ft-nn` mirrors much of `torch.nn`, for example:
    - Layers: Linear, Conv1d/2d/3d, ConvTranspose*, LSTM/GRU, Transformer*, MultiheadAttention.
    - Normalization: BatchNorm, LayerNorm, GroupNorm, RMSNorm.
    - Losses: GaussianNLLLoss, PoissonNLLLoss, FocalLoss, CTCLoss.
    - Pooling and upsampling layers.
  - `ft-optim` provides SGD, Adam, AdamW, LBFGS, RAdam, NAdam, Muon and others, with full LR schedulers.
  - Some distribution sampling exists in `ft-api`; reparameterized autograd for Bernoulli and Poisson is
    tracked as TODO (`frankentorch-vugw`).
- **Maturity evidence:** the release-readiness scorecard shows batched small-matrix linalg gradients several
  times *faster* than PyTorch CPU. CNN and normalization training steps are 3–28× *slower* (for example,
  BatchNorm2d f32 is 28× slower). The project describes itself as a "CPU-first vertical slice" with an
  explicit Metal kernel crate.
- **Mining relevance: MEDIUM.**
  - Workable today: small and medium models (MLP prospectivity classifiers, LSTM telemetry models, small
    CNNs on raster patches).
  - Expect weakness: large-scale satellite or drone segmentation training.
- **No classical ML:** no random forest, gradient boosting or SVM exists anywhere in the suite, even though
  these are the standard models in mineral prospectivity mapping.

### 1.6 franken_networkx (`fnx-*`)
- **Path:** `/Users/jemanuel/projects/franken_networkx`
- **Last commit:** `de58cd218 2026-10-27 merge(main): keep fail-closed mean-geodesic receipts with upstream`
- **Size:** 123 .rs files, but large: about 305K raw .rs lines (the `fnx-algorithms/src/lib.rs` file alone
  is 3.3 MB). 2,274 markers.
- **Maturity evidence:**
  - Version 0.2.3, published as **PyPI wheels**; no crates.io claim.
  - 315 algorithms wired into the NetworkX dispatcher; 4,926 NetworkX 3.6.1 FeatureUniverse rows tracked.
  - 35 of 41 commonly used functions execute no NetworkX code.
- **Workspace (12 crates):** `fnx-classes`, `fnx-views`, `fnx-dispatch`, `fnx-convert`, `fnx-algorithms`,
  `fnx-generators`, `fnx-readwrite`, `fnx-durability`, `fnx-conformance`, `fnx-runtime`, `fnx-cgse`,
  `fnx-python`.
- **Capabilities:**
  - Flow: `maximum_flow`, `minimum_cut`, `preflow_push` (in `preflow_push.rs`), `max_flow_edmonds_karp`,
    `minimum_cut_edmonds_karp`, `min_cost_flow`, `network_simplex` (`network_simplex_int.rs`), `gomory_hu`,
    `stoer_wagner`.
  - Shortest paths: `dijkstra`, `astar`, `bidirectional_dijkstra`, `johnson`.
  - DAG tools: `dag_longest_path`, `topological_sort`, `transitive_closure`.
  - Matching, MST and TSP approximations (`christofides`).
- **Important design fact:** `Graph`/`DiGraph` are NetworkX-faithful. Nodes are *string-named* with
  per-node and per-edge `AttrMap`s, and APIs take `source: &str` plus `capacity_attr: &str`. `preflow_push`
  deliberately models **CPython's set iteration order** to reproduce NetworkX's exact flow.
- **Mining relevance: MEDIUM.**
  - Good for: haul-road network routing (A*, Dijkstra), dispatch assignment (`min_cost_flow`,
    `network_simplex`), and equipment and process graphs.
  - **Not appropriate** for 10⁷-block × 10⁸-arc pit-precedence max-flow. Use it as a correctness oracle for
    a purpose-built pseudoflow.

### 1.7 frankensympy (`fsym-*`)
- **Path:** `/Users/jemanuel/projects/frankensympy`
- **Last commit:** `9d785d0 2026-10-27 Expose independently certified proper real integration in Python`
- **Size:** 215 .rs files, about 146K raw .rs lines, 1,621 markers. 29 crates in `crates/`, including
  `fsym-core`, `fsym-polys`, `fsym-calculus`, `fsym-solvers`, `fsym-matrices`, `fsym-proof-kernel`,
  `fsym-geometry` and `fsym-python`.
- **Maturity:** the README states "implementation in progress, pre-certification ... no compatibility
  profile is certified, no performance win is demonstrated." A live SymPy 1.14.0 oracle exists.
- **Mining relevance: LOW.** It could derive closed-form sensitivities, for example of cutoff-grade
  formulas (Lane's method), recovery curves or royalty schedules, but it is not on a critical path.

### 1.8 franken_drone_geometry_reconstruction (`fdgr-*`)
- **Path:** `/Users/jemanuel/projects/franken_drone_geometry_reconstruction`
- **Last commit:** `db45703 2026-10-26 docs: refresh implementation boundary to 30 crates and calibrated reprojection-evidence eva`
- **Size:** 88 .rs files, about 27K raw .rs lines, 123 markers.
- **Crates (30):** `fdgr-core`, `fdgr-media*`, `fdgr-clock`, `fdgr-calibration`, `fdgr-scale`,
  `fdgr-keyframe`, `fdgr-correspondence`, `fdgr-epipolar`, `fdgr-relative-pose`, `fdgr-graph`,
  `fdgr-global-pose`, `fdgr-pose-graph`, `fdgr-pose-refinement`, `fdgr-bundle-problem`,
  `fdgr-bundle-admission`, `fdgr-reprojection`, `fdgr-projection`, `fdgr-evidence`, `fdgr-ledger`,
  `fdgr-cli`, and others.
- **Status (README, 2026-10-26):** the pipeline runs from media custody through calibrated epipolar and
  relative-pose adjudication, translation-only refinement, and a structural bundle problem plus audit.
  It explicitly does **not** yet claim:
  - live DJI acquisition;
  - native feature generation;
  - joint pose-landmark bundle adjustment;
  - metric camera pose;
  - sparse or dense reconstruction.
- **Mining relevance:** HIGH in intent (pit, stockpile and highwall surveys) but **LOW in usable
  capability today**. Its evidence-ledger architecture (custody → calibration → scale witness) is a good
  pattern for survey-grade provenance.

### 1.9 franken_ocr (`focr`)
- **Path:** `/Users/jemanuel/projects/franken_ocr`
- **Last commit:** `ceb71bc 2026-10-14 fix(install): read the latest tag from minified GitHub API JSON [skip ci]`
- **Size:** 130 .rs files, about 137K raw .rs lines, 1,599 markers. Crate version 0.9.1; binary release
  v0.8.0 for macOS, Linux and Windows on x86 and ARM; usable as a library from crates.io.
- **Models:**
  - Baidu Unlimited-OCR (default): documents to Markdown, including tables, LaTeX and reading order.
  - GOT-OCR2: structured formats.
  - SmolVLM2: image description and VQA.
  - OneChart: chart to data.
  - TrOMR: sheet music.

  All run CPU-only on model-specific kernels built on `frankentorch-kernel-cpu`.
- **PDF support:** in-process rasterization of scanned PDFs (JPEG `DCTDecode`, CCITT G4, Flate/LZW).
  **JPX, JBIG2 and born-digital vector/text pages are rejected with an error.**
- **Quality evidence:** aggregate normalized CER 0.193 on a 20-page corpus, with one known hard page at
  CER 0.616.
- **Mining relevance: HIGH for legacy scanned drill logs, assay certificates and old reports.** OneChart
  can digitize grade-tonnage curves and section plots.
- **Gap:** text-layer extraction from modern born-digital NI 43-101 and JORC PDFs. It also does not
  understand handwritten logging sheets specifically; Unlimited-OCR is a print-document model.

### 1.10 franken_nlp (`fnlp`)
- **Path:** `/Users/jemanuel/projects/franken_nlp`
- **Last commit:** `99b166a 2026-10-27 feat(cli): expose verified local release reconstruction`
- **Size:** 997 .rs files in a single crate (`src/`), about 229K raw .rs lines, 4,244 markers.
- **Design:** one model, Nanbeige4.2-3B, with INT4 kernels. The target 1.0 contract includes:
  - `extract --schema` (JSON Schema → valid JSON by construction, with byte-exact `verbatim` source spans);
  - `ner`, `resolve` (entity resolution), `classify`, `judge`, `redact`, `summarize --cited`, `answer`
    (with calibrated abstention), `keyphrases`.
- **Status:** the README is written in future tense. Only `candidate source-int4 ...` routes are
  executable. WIRING.md records build-proof authority as **BLOCKED** pending a DSR receipt.
- **Mining relevance: HIGH in intent** (extracting resource tables, drillhole intercepts, QP names,
  cut-offs and recoveries from reports and logs) but **pre-qualification**. Plan for a fallback.

### 1.11 franken_whisper (`fw`)
- **Path:** `/Users/jemanuel/projects/franken_whisper`
- **Last commit:** `1d4d9328 2026-10-23 fix(check): build the merged tree against current sibling crates`
- **Size:** 262 .rs files, about 386K raw .rs lines, 5,845 markers. Version 0.11.0, with prebuilt binaries
  and crates.io packaging.
- **Capabilities:**
  - A native pure-Rust Whisper engine on FrankenTorch CPU kernels, with automatic Metal on macOS. The
    README claims 2.99× faster than whisper.cpp on large-v3-turbo.
  - Sortformer diarization for up to 4 speakers.
  - DTW word timestamps.
  - Live mic and pipe streaming (`fw robot listen`) with NDJSON events.
  - SQLite run history; native decoding of MP3, AAC, FLAC, WAV, OGG and ALAC.
- **Limitations:** single-machine; diarization is "development-uncertified".
- **Mining relevance: MEDIUM.** Shift-handover and toolbox-talk transcription, and radio logs. A four-speaker
  cap and noisy pit radio channels are risks.

### 1.12 franken_lean (`fln`)
- **Path:** `/Users/jemanuel/projects/franken_lean`
- **Last commit:** `534df5968 2026-10-28 feat(ir): execute UInt64 and USize with full-width native carriers`
- **Size:** 2,126 .rs files, about 1.31M raw .rs lines, 13,901 markers, 36 Cargo manifests.
- **Design:** a native-Rust Lean 4 toolchain. The README is in future tense. Commands live today in bounded
  form include `fln check-olean [--receipts]`, `fln run`, `olean inspect/diff/verify-rebuild`,
  `audit --tcb`, `why-trusts` and `goals`.
- **Mining relevance: LOW-MEDIUM.** It could carry machine-checked proofs of core invariants:
  - closure of pit-precedence constraints;
  - mass and metal balance conservation in reconciliation;
  - double-entry accounting identities;
  - schedule-feasibility certificates.

  This is a long-horizon assurance play, not a dependency for v1.

### 1.13 frankenmermaid (`fm-*`)
- **Path:** `/Users/jemanuel/projects/frankenmermaid`
- **Last commit:** `6880dc8e 2026-10-27 refactor(render): unify authored font metrics across backends`
- **Size:** 378 .rs files, about 333K raw .rs lines, 4,879 markers.
- **Crates:** `fm-core`, `fm-parser`, `fm-layout` (18 layout algorithms plus Auto), `fm-render-svg`,
  `fm-render-canvas`, `fm-render-term`, `fm-wasm`, `fm-cli`.
- **Diagram types:** flowchart, Gantt, Sankey, architecture, C4, quadrant, xychart, timeline, block-beta,
  kanban, treemap, mindmap.
- **Scale:** renders 2,000–10,000-node graphs that mermaid-js fails on.
- **Mining relevance: MEDIUM.**
  - Process flowsheets (flowchart or block-beta).
  - Life-of-mine and maintenance Gantt charts.
  - **Sankey diagrams for metal accounting and mass balance.**
  - Equipment hierarchies.

### 1.14 franken_manim (`fmn`)
- **Path:** `/Users/jemanuel/projects/franken_manim`
- **Last commit:** `3a4cc1ff 2026-10-28 fix(python): retain exact camera geometry through placement and playback`
- **Size:** 757 .rs files, about 481K raw .rs lines, 5,003 markers, 32 manifests (`fmn-*`).
- **Capabilities:** deterministic, bit-identical 4K renders with native TeX math (fmd-math). The README is
  in target-state tense.
- **Mining relevance: LOW.** Explainer videos for training, board and investor communication (for example,
  how pit optimization works, or safety procedures).

---

## 2. Per-repo fact sheets: brief, mostly low relevance

| Repo | Last commit | Size (rs files / raw lines / markers) | Purpose (one line) | Mining relevance |
|---|---|---|---|---|
| franken_snowflake | `ca5bdc3 2026-10-19` | 96 / 95K / 808 | Clean-room Snowflake SQL API connector (PAT, key-pair JWT, OAuth), no ODBC/JDBC/tokio | MEDIUM-LOW: pull ERP, finance or corporate warehouse data if the company uses Snowflake |
| franken_trends | `333d209 2026-09-04` | 10 / 9K / 82 | Pure-Rust Google Trends client (pytrends port) | NEGLIGIBLE: commodity interest signal at most |
| franken_overlap | `7952434 2026-10-14` | 125 / 58K / 173 | Explainable textual provenance, edited-passage retrieval, BM25 and hybrid search | MEDIUM-LOW: trace resource statements and copied passages across successive technical reports; report search |
| franken_alignment | `582334e9 2026-10-25` | 1,857 / 411K / 6,590 | Evidence-carrying control and governance for untrusted agents | MEDIUM-LOW: gate agent-proposed operational actions (blast schedules, dispatch overrides) |
| franken_remote | `de5b359d 2026-10-27` | 1,241 / 419K / 4,879 | Tailnet-native remote desktop (HEVC); early, Linux X11 only | LOW: remote operations-centre access to site machines |
| FrankenCA | `3f6469b 2026-10-18` | 0 .rs (JS and Playwright) | "Smeared Life" continuous cellular-automata web experiments | NONE |
| franken_engine | `55b66ae24 2026-10-28` | 13,792 / 16.8M / 147,575 | Native Rust JS/TS runtime for adversarial extension workloads | LOW: sandboxed user scripting and plugins |
| franken_node | `31d45dff0 2026-10-28` | 1,312 / 1.36M / 28,407 | Trust-native JS/TS runtime platform | LOW: same as franken_engine |
| frankenlibc | `edd6fba82 2026-10-27` | 2,925 / 2.46M / 20,005 | Clean-room Rust glibc with safety membrane | NONE |
| frankengit | `fcef8e5d 2026-10-21` | 2,086 / 901K / 9,185 | Pure-Rust Git-compatible forge (pre-release) | LOW: versioning of block models, plans and configs |
| franken_markdown (`fmd`) | `4f2bcc95 2026-10-28` | 1,214 / 1.22M / 5,262 | Markdown → HTML, tagged PDF, EPUB, WASM; crates.io 0.6.0 | MEDIUM: generate shift, reconciliation and technical-report documents |
| franken_macos | `586be8a 2026-09-22` | 24 / 11K / 105 | FrankenCodeBrowser macOS app (Metal code atlas) | NONE |

### 2.1 Additional suite repos discovered by the sweep (not in the original list, but relevant)

| Repo | Last commit | Size | Relevant contents | Mining relevance |
|---|---|---|---|---|
| **frankensim** (`fs-*`) | `20463525d 2026-10-26` | 5,391 .rs / 2.85M lines / 21,918 tests; 164 native crates plus 20 standalone WASM workspaces; version 0.0.1 | See §2.2 | **HIGH** (geostatistics-adjacent UQ, GP, VOI/OED, geometry, sparse direct, robust optimization) |
| **franken_surveillance_system** (`fss-*`) | `8d810de7 2026-10-28` | 2,198 .rs / 1.21M lines; 22 manifests | `fss-geometry::bundle`: LM bundle adjustment over a Schur complement (intrinsics, k1/k2, SO(3) extrinsics, landmarks), gauge fixed by a reference camera or **≥3 surveyed control points**, Gauss–Newton covariance. Also `fss-twin` joint refinement; `fss-codec-h264`/`-h265`/`-mjpeg` decoders; YOLOX-Nano detector; Kalman tracking (`fss-reference`); `fss-graph-algorithms` (max-flow/min-cut, Gomory–Hu). Self-described "unqualified reference implementation" | **MEDIUM-HIGH**: the only real BA with GCPs in the suite; camera ingestion for pit and stockpile monitoring; vehicle and person detection |
| **franken_accounting** (`fa-*`) | `53c6e953 2026-10-28` | 1,028 .rs; 34 manifests | Double-entry, bitemporal, evidence-native ledger on FrankenGraphDB; exact money and calendar types; reasoned reversals; MCP tools | **HIGH for the finance module**: system of record (status: implementation in progress) |
| frankengraphdb | `3bb235b76 2026-10-28` | 2,383 .rs | MVCC property-graph DB with time travel and branches; GQL | MEDIUM: asset, equipment and process hierarchies; lineage of block-model versions |
| frankensqlite | `9090854e1 2026-10-28` | 1,878 .rs | Rust SQLite reimplementation with MVCC concurrent writers | MEDIUM-HIGH: embedded drillhole, assay and telemetry store |
| frankensearch | `b2c09398 2026-10-28` | 723 .rs | Two-tier hybrid local search (`fsfs`) | MEDIUM: technical report and log search |
| asupersync | `523e86c00 2026-10-28` | 6,097 .rs | Mandatory async runtime for the suite (no tokio) | REQUIRED infrastructure |
| fast_cmaes / wasm_cmaes | `a4bd47e 2026-09-04` / `c480f9c 2026-09-21` | 6 and 2 .rs | SIMD and rayon CMA-ES (Python-first) and a WASM build | LOW-MEDIUM: prefer `fs-dfo` for in-suite use |
| franken_textiles | `4a11a6b 2026-10-28` | 711 .rs | Pattern geometry, DXF I/O | LOW: has a DXF writer (CAD interchange precedent) |
| frankentts | `6e791312 2026-10-17` | 146 .rs | Pure-Rust Qwen3-TTS | LOW: voice alerts |
| hnswlib-rs | `dbbb8dd 2026-10-12` | 21 .rs | HNSW approximate nearest neighbours | LOW: embedding search |
| frankenredis | n/a | n/a | Redis GEO commands (haversine and geohash hits) | LOW: the only geodesic code found in the suite |

### 2.2 frankensim crates most relevant to mining (descriptions from each crate's `Cargo.toml`, APIs spot-verified)

| Crate | What it provides | Mining use |
|---|---|---|
| `fs-bo` | `gp::Gp` (exact GP, **Matérn ½, 3⁄2, 5⁄2 with ARD lengthscales**, fs-la Cholesky, log marginal likelihood, L-BFGS with QMC multistart hyperparameter fitting); `sparse::SparseGp` (inducing points, DTC/SoR, **Titsias ELBO**); `mf::MfGp` (multi-fidelity); EI and q-EI Bayesian optimization | Kriging-equivalent estimator (a GP with Matérn kernel is simple kriging with a Matérn covariance). Lacks rotated anisotropy, nested structures, spherical model, block support and ordinary-kriging unbiasedness |
| `fs-uq` | Karhunen–Loève random fields with captured-variance evidence, polynomial chaos, QMC propagation, multilevel Monte Carlo | Unconditional Gaussian random fields; resource and price uncertainty propagation |
| `fs-assimilate` | Checked linear-Gaussian (Kalman-type) belief updates from instruments | Updating grade and geotechnical models with production and sensor data; reconciliation |
| `fs-voi`, `fs-oed-e2e` | Value of information; greedy OED placing Kalman sensor updates by EVPI | **Infill drilling and sampling design**, piezometer placement |
| `fs-robust`, `fs-robustopt-e2e`, `fs-sos` | CVaR and robust optima; SOS/SDP (in-house primal-dual SDP solver) certified global optima | Risk-aware planning (CVaR of NPV) |
| `fs-dfo` | CMA-ES (IGO form), BIPOP restarts, Nelder–Mead | Cutoff-grade and policy optimization, black-box calibration |
| `fs-sparse` | CSR/BSR/SELL-C-σ, SpMV/SpMM, AMG; `direct.rs` **supernodal multifrontal LDLᵀ with AMD ordering**, 2×2 pivoting, inertia | Large SPD and indefinite systems (FEM tailings, large reconciliation); sparse-Cholesky-class capability |
| `fs-la` | Dense GEMM, batched small dense, `factor::cholesky`, `batched::batch_cholesky` | Many small kriging systems in parallel |
| `fs-rand` | Counter-based Philox streams keyed by logical identity; Sobol/Owen QMC | Reproducible stochastic simulation and realizations |
| `fs-eproc` | Anytime-valid e-processes, confidence sequences, e-BH | Sequential monitoring (grade-control reconciliation drift, sensor anomalies) with valid error control |
| `fs-rep-voxel` | VDB sparse occupancy and multi-material voxels, exact EDT, **point clouds with normals** | Block-model and voxel storage; survey point clouds |
| `fs-rep-sdf` | Dense and VDB SDFs, adaptive octree SDF, narrow-band evolution | Implicit geological modelling (orebody surfaces as SDFs) |
| `fs-mesh` | BRIO incremental Delaunay tet meshing on exact predicates, quality refinement | Tetrahedral geomechanics meshes; TIN surfaces |
| `fs-rep-mesh` | Half-edge surfaces, tet/hex complexes, generalized-winding-number repair, `contour::dual_contour` | Wireframe repair; grade-shell extraction |
| `fs-viz` | `isosurface`, isocontours, RK4 streamlines, Morse critical points | Grade shells; groundwater and airflow visualization |
| `fs-asbuilt` | Closed-form 2-D rigid fit; **3-D Kabsch rigid and Umeyama similarity registration**; 3-2-1 datum registration; calibrated 6-dof pose covariance | Scan-to-design (as-built pit vs design, stockpile surveys) given correspondences. Not ICP |
| `fs-io` | Hardened PLY, STL, OBJ, STEP, glTF, VTK interchange | Mesh exchange with CAD and mine-planning packages (DXF and GeoTIFF absent) |
| `fs-solid`, `fs-cutfem`, `fs-solver` | Small and finite-strain elasticity, CutFEM, Newton–Krylov, p-multigrid | Tailings and pit-slope geomechanics substrate (no soil plasticity or limit equilibrium) |
| `fs-conduction` | Steady conduction (the only L3 capability) | Same PDE as steady Darcy seepage, reusable for tailings phreatic analysis |
| `fs-tropical`, `fs-schedule-e2e` | Max-plus CPM, makespan, Karp max-cycle-mean throughput | Truck-shovel cycle and throughput bottlenecks; maintenance critical path |
| `fs-surrogate` | POD, DMD and EDMD reduced-order models inside certified validity bands | Fast surrogates of process-plant or geotechnical simulators |
| `fs-thermochem`, `fs-material` | Thermochemical data and standard-state evaluation; constitutive laws | Hydrometallurgy and pyrometallurgy property evaluation (no Gibbs minimizer found) |
| `fs-lbm` | D2Q9 lattice Boltzmann (2-D) | Toy-scale flow only |

The registry in `capability-maturity.json` puts every recorded capability at L1–L3. Its note says "Nothing
here is L4 or L5, because the external V&V corpus ... do[es] not exist yet." Treat frankensim components as
**high-quality reference kernels with strong contracts, not production-qualified solvers.**

---

## 3. Cross-project ripgrep sweep results

Code files only, across 251 primary repos. The regexes produced false positives; these were checked
manually and are marked.

| Term(s) | Where found (verified) | Verdict |
|---|---|---|
| kriging, variogram, semivariogram | nowhere | **Absent** |
| gaussian_process / GaussianProcess | nowhere by name; real GPs are in `frankensim/crates/fs-bo` (`Gp`, `SparseGp`, `MfGp`) | Present under another name |
| karhunen / turning_bands / sgsim / sequential_gaussian | `frankensim/crates/fs-uq` (Karhunen–Loève only) | Partial (unconditional KL only) |
| mcmc, metropolis_hastings, hamiltonian/HMC, NUTS | nowhere (the "nuts" hits are English words in web projects) | **Absent** |
| variational / ELBO | `fs-bo::sparse` (Titsias ELBO for sparse GPs only) | No general VI |
| milp, mixed_integer | `frankenscipy/crates/fsci-opt` (`milp`) | Present, small scale |
| branch_and_bound | only unrelated hits (pi_agent_rust, frankensqlite, dwarf_fortress_mcp) | Only inside fsci-opt `milp` |
| simplex (LP) | `fsci-opt` (revised simplex), `fnx-algorithms/network_simplex_int.rs` | Present |
| interior point / Mehrotra / PDHG | `fsci-opt` (Mehrotra IPM), `frankensim` (`fs-truss` PDHG, `fs-sos` SDP) | Present |
| max_flow | `fsci-sparse::maximum_flow` (Dinic, int32); `fnx-algorithms` (`preflow_push`, Edmonds–Karp); `fss-graph-algorithms/src/flow.rs`; `franken_accounting/crates/fa-lantern/src/graph/maxflow.rs` (Edmonds–Karp). The `frankensim/fs-airflow` hits mean airflow rate, a false positive | Present, not pit-scale |
| pseudoflow, Lerchs–Grossmann, ultimate pit | nowhere | **Absent** |
| geotiff, TIFF | none in the numerics suite (hits are in giil, document tools and image-link utilities) | **Absent** |
| proj4, EPSG, WGS84, ECEF, UTM, transverse_mercator | none in the suite (only retail_lease_manager_portal, a web app, and a frankenredis GEO helper) | **Absent** |
| haversine, geodesic | frankenredis (Redis GEO), frankenlibc, web apps | Effectively absent |
| shapefile, GeoJSON | web apps only | **Absent** in Rust suite |
| LAS, LAZ, lidar | no real hits (the ".las" and "laz" matches were substrings such as "alias") | **Absent** |
| point_cloud | `frankensim/crates/fs-rep-voxel` (point clouds with normals), `fs-fmm` (point sets) | Basic container only |
| delaunay | `frankenscipy/fsci-spatial` (N-D Qhull port), `frankensim/fs-mesh` (3-D BRIO, exact predicates) | Present |
| marching_cubes | only `frankensim/crates/fs-wasm` demos; production isosurfacing in `fs-viz::isosurface`, `fs-rep-mesh::contour::dual_contour` | Present (isosurface and dual contouring) |
| sparse Cholesky, supernodal, CHOLMOD | `frankensim/crates/fs-sparse/src/direct.rs` (supernodal multifrontal LDLᵀ plus AMD) | Present (in frankensim, not fsci) |
| mesh boolean, CSG | `frankensim` `fs-rep-frep` and `fs-rep-nurbs` (F-rep and NURBS-level only) | No robust triangle-mesh booleans |
| ICP | no real implementation (the `fs-govern` hit is a string label) | **Absent** |
| RANSAC | none (hits were "tRANSACtion") | **Absent** |
| bundle adjustment | `franken_surveillance_system/crates/fss-geometry/src/bundle.rs` (+ `bundle/solve.rs`), `fss-twin/src/joint_refinement.rs` | Present (reference grade) |
| SfM, MVS, photogrammetry | none; fdgr has partial SfM front-end stages only | **Absent** (dense) |
| Kalman | `frankensim/fs-assimilate`, `fs-oed-e2e`; `franken_surveillance_system/fss-reference`, `fss-twin` | Present |
| particle filter, SMC | nowhere | **Absent** |
| octree | frankensim (`fs-rep-sdf` and others) | Present |
| CMA-ES | `frankensim/fs-dfo`, `fast_cmaes`, `wasm_cmaes` | Present |
| CVaR, robust optimization | `frankensim/fs-robust`, `fs-robustopt-e2e` | Present |
| PLY, STL, OBJ, STEP, glTF | `frankensim/fs-io` | Present |
| DXF | `franken_textiles` (12 files), `frankensim` (1) | Partial (textiles domain) |
| HDF5, NetCDF | `frankenpandas` (optional HDF5 feature), `fsci-io` (simplified NetCDF reader) | Partial |
| random forest, GBM, decision tree, SVM | none in the ML suite | **Absent** |
| block_model, orebody | no real hits (noise) | **Absent** |
| NPV, IRR, DCF | none in the suite's Rust numerics (web and finance tools only) | **Absent** (trivial to build) |

---

## 4. Capability matrix: mining need → suite provider → maturity → gap

Maturity key:
- **A** = mature (published, differential-tested against an upstream oracle).
- **B** = substantial implementation with tests, but self-declared pre-qualification or reference grade.
- **C** = early or partial.
- **—** = absent.

### 4.1 Exploration and Bayesian prospectivity
| Need | Provider | Maturity | Gap |
|---|---|---|---|
| Grid scattered geophysical and geochemical data | `fsci-interpolate::{griddata, RbfInterpolator, RegularGridInterpolator, interpn}` | A | Minimum curvature gridding; kriging-based gridding; anisotropic RBF |
| Potential-field filtering (upward continuation, RTP, derivatives, tilt, analytic signal) | `fsci-fft::{rfft2, fftn, fftfreq}`, `fsci-ndimage::{gaussian_filter, sobel}`, `fsci-signal::hilbert2` | A (primitives) | **Geophysics operators themselves**; Euler deconvolution; gridded wavenumber filters |
| 1-D profile and time-series filtering (EM, IP, radiometrics) | `fsci-signal` (Butterworth, `filtfilt`, Welch, STFT, CWT, `savgol`, `find_peaks`, `decimate`) | A | Domain corrections (levelling, microlevelling, radiometric stripping) |
| Geophysical inversion (mag, grav, IP, EM) | Building blocks: `fsci-sparse` Krylov (LSQR, LSMR, CG), `fs-sparse` AMG, `frankenjax` gradients, `fs-fmm`, `fs-bem` | B (blocks) | **Forward operators, sensitivity matrices, regularized inversion: build** |
| Geochemical statistics (lognormal, censored below-detection-limit values, compositional data) | `fsci-stats` (distributions, `CensoredData`, Box-Cox, `bootstrap`, `GaussianKde`, `Dirichlet`), `fsci-cluster` (KMeans, DBSCAN, hierarchical) | A | CLR/ILR log-ratio transforms; robust PCA; anomaly thresholds (simple to build) |
| Prospectivity ML (weights of evidence, logistic regression, RF/GBM, NN) | `frankentorch` (`ft-nn` MLP/CNN, `ft-optim`), `fsci-opt` (logistic regression via `minimize`) | B | **RF/GBM/XGBoost-class trees absent**; weights-of-evidence and fuzzy logic (simple) |
| Bayesian inference (MCMC, HMC, NUTS, VI, SMC) | Substrate: `frankenjax` (grad, vmap, jit, scan, while_loop, ThreeFry), `fsci-stats` logpdfs, `fs-bo` GP, `fs-assimilate` | B (substrate) | **All samplers absent: build HMC/NUTS on frankenjax** |
| Remote sensing (multispectral and hyperspectral rasters, band ratios, alteration mapping, segmentation) | `fsci-ndimage` ops; `frankentorch` Conv2d | A / B | **GeoTIFF/COG reader, CRS, reprojection, resampling to grid: build**; spectral unmixing |
| Survey and drilling design via value of information | `fs-voi`, `fs-oed-e2e`, `fs-bo` | C–B | Adapt to drillhole geometry and geostatistical posterior |

### 4.2 Drillhole and assay data
| Need | Provider | Maturity | Gap |
|---|---|---|---|
| Tabular ingest (CSV, Excel, fixed-width, SQL, Parquet) | `frankenpandas` (`fp-io`) | A–B | Vendor formats (acQuire, Datamine, Surpac, Leapfrog exports) |
| Embedded database | `frankensqlite`, `frankengraphdb` | B | Drillhole schema (collar, survey, assay, lithology, QA/QC) |
| Desurvey (minimum curvature, tangent), interval compositing, from/to overlap joins | none (`frankenpandas` `merge_asof` is point-based) | — | **Build** |
| QA/QC (CRMs, blanks, duplicates; Thompson–Howarth, HARD, Shewhart) | `fsci-stats` (regression, tests), `fs-eproc` (sequential monitoring) | A / B | Domain-specific charts and rules (build) |
| Legacy logs and certificates (scanned) | `franken_ocr` (Unlimited-OCR, GOT-OCR2) | B | Handwriting; born-digital text-layer PDFs |

### 4.3 Geostatistics and conditional simulation
| Need | Provider | Maturity | Gap |
|---|---|---|---|
| Neighbourhood search | `fsci-spatial::KDTree` (query, `query_pairs`, `count_neighbors`) | A | Anisotropic search ellipsoids and octant search (build on KDTree with transformed coordinates) |
| Experimental variograms (directional, anisotropic), model fitting (nested spherical, exponential, Gaussian) | Fitting via `fsci-opt::least_squares` / `curve_fit`; differentiable fitting via `frankenjax` | A (fitting) | **Variogram engine: build** |
| Kriging (SK, OK, UK, IK, co-kriging, block kriging) | Solvers: `fsci-linalg` (`solve_with_casp`, `cholesky`), `fs-la` batched Cholesky; estimator analogue `fs-bo::Gp` (Matérn ARD) | A (solvers) / B (GP) | **Kriging systems, support change, kriging variance and slope of regression: build** |
| Gaussian anamorphosis and normal scores | `fsci-special` (Hermite polynomials), `fsci-stats` (`norm.ppf`, ECDF) | A | Hermite-expansion anamorphosis and DGM change of support (build) |
| Conditional simulation (SGS, turning bands, LU, PFS, multiple-point) | Unconditional KL: `fs-uq`; RNG: `fs-rand` (Philox, Sobol/Owen), `fnp-random` (PCG64DXSM) | B (KL) | **SGS, turning bands, conditioning, post-processing (e-type, probability maps): build** |
| Declustering | `fsci-spatial::Voronoi` (polygonal), simple cell declustering | A (Voronoi) | Cell declustering optimizer (trivial) |
| Block model storage and queries | `fs-rep-voxel` (VDB sparse voxels), `fnp-io` (`.npy`/memmap), `fsci-ndimage` (labels, morphology) | B / A | Sub-blocked and rotated block models; standard model export formats |
| Implicit geological modelling | `fs-rep-sdf` (octree SDF), `fsci-interpolate::RbfInterpolator`, `fs-viz::isosurface`, `fs-rep-mesh::dual_contour` | B | Structural constraints (contacts, orientations, faults); FastRBF-scale solvers |

### 4.4 Mine planning optimization
| Need | Provider | Maturity | Gap |
|---|---|---|---|
| Ultimate pit (LG or pseudoflow) on 10⁶–10⁸ blocks | `fsci-sparse::maximum_flow` (Dinic, **int32**); `fnx-algorithms` (`preflow_push`, Edmonds–Karp on string graphs) | A for correctness, but **not scalable** | **Hochbaum HPF pseudoflow on i64 CSR with implicit precedence generation, parametric nested pits (revenue factors), slope-angle precedence templates: build.** Use fsci and fnx as test oracles |
| Pushback and phase design | none | — | Build (parametric pseudoflow plus smoothing) |
| Production scheduling MILP (block, aggregate, multi-destination, stockpiles) | `fsci-opt::{linprog, milp}` (dense rows; B&B over linprog; "small to medium") | A for SciPy parity but **not industrial scale** | **Sparse LP/MILP (presolve, dual simplex warm start, cuts) or decomposition algorithms (Bienstock–Zuckerberg, TopoSort, Lagrangian), MPS/LP I/O: build or choose.** `fs-truss` PDHG suggests a PDLP-style first-order path |
| Cutoff-grade and policy optimization | `fs-dfo` (CMA-ES, BIPOP), `fsci-opt` (DE, dual annealing, SHGO, `direct`), `fs-bo` (BO with EI/qEI) | B / A | Lane's method and dynamic programming (simple) |
| Stochastic and robust planning (grade and price uncertainty) | `fs-robust` (CVaR), `fs-uq` (MLMC, QMC), `fsci-stats` QMC | B | Stochastic integer programming (two-stage, scenario trees) and SIP heuristics (build) |
| Haul routing | `fnx-algorithms` (`dijkstra`, `astar`, `bidirectional_dijkstra`), `fsci-sparse::dijkstra` | A–B | Grade- and speed-dependent travel-time model (build) |
| Truck dispatch and shovel assignment | `fsci-opt::linear_sum_assignment`; `fnx` `min_cost_flow` and `network_simplex` | A / B | Real-time dispatch policy (DISPATCH-style LP plus assignment) |
| Cycle-time and throughput bottlenecks | `fs-tropical` (max-plus CPM, Karp max-cycle-mean) | C–B | Discrete-event simulation of the haulage system (build) |

### 4.5 Operations digital twin (drone and lidar surveys, fleet telemetry)
| Need | Provider | Maturity | Gap |
|---|---|---|---|
| Image ingestion and custody | `fdgr-media*`, `fss-codec-{h264,h265,mjpeg}`, `fss-container` | B | DJI acquisition (documented as unbuilt) |
| Feature matching, relative pose | `fdgr-correspondence`, `fdgr-epipolar`, `fdgr-relative-pose` | C | **Native feature extraction (SIFT/ORB-class) is not claimed** |
| Bundle adjustment with ground control points | `fss-geometry::bundle` (LM plus Schur complement, control points, covariance) | B (reference grade, synthetic fixtures only) | Scale-out to thousands of images; robust losses and outlier rejection; RTK and PPK priors |
| Dense reconstruction (MVS), orthomosaic, DSM | none | — | **Build (largest perception gap)** |
| LAS/LAZ point-cloud I/O | none | — | **Build** |
| Point-cloud processing (ICP, ground filtering, decimation, normals) | `fs-rep-voxel` (point clouds with normals), `fsci-spatial::KDTree`, `fs-asbuilt` (Kabsch/Umeyama with known correspondences) | B | **ICP, RANSAC, CSF/PMF ground filtering, outlier removal: build** |
| Surface (TIN/DEM) creation and stockpile volumes (cut/fill) | `fsci-spatial::Delaunay` (N-D), `fsci-interpolate::griddata` (linear on TIN), `fs-mesh`, `fs-geocon` (certified volume) | A / B | Surface-to-surface volume, breaklines, contouring of DTMs (build) |
| As-built vs design compliance | `fs-asbuilt` (rigid and similarity registration with calibrated covariance) | B | Design-surface deviation heatmaps (build) |
| CRS and geodesy (projected grids, mine grid, WGS84, geoid) | none | — | **Build (or vendor PROJ math in pure Rust)** |
| Fleet telemetry analytics | `frankenpandas` (resample, rolling, ewm, `merge_asof`), `fsci-signal`, `fs-assimilate` (Kalman), `fs-eproc` (anytime-valid change detection) | A / B | GNSS/IMU fusion; payload and fuel models (build) |
| Object detection (vehicles, people, PPE) | `fss` (YOLOX-Nano, COCO-80) | B | Mining-specific classes and training data |

### 4.6 Process plant modelling
| Need | Provider | Maturity | Gap |
|---|---|---|---|
| Kinetics (leach, flotation, CIL adsorption), stiff ODEs | `fsci-integrate::solve_ivp` (BDF, Radau, LSODA), `solve_bvp` | A | Unit-operation model library (build) |
| Parameter fitting and calibration | `fsci-opt::{least_squares, curve_fit}`, `fsci-odr`, `frankenjax` gradients | A / B | — |
| Mass-balance data reconciliation | `fsci-opt` (SLSQP, trust-constr with linear constraints), `fsci-linalg::lstsq_with_casp`, `fs-sparse` LDLᵀ | A / B | Reconciliation formulation and gross-error detection (build) |
| Comminution and classification (Bond, JK, population balance, hydrocyclones) | none | — | **Build** |
| Thermochemistry (roasting, smelting, Pourbaix) | `fs-thermochem` (standard-state data) | C | Gibbs-energy minimization (build) |
| Surrogates and reduced-order models of plant | `fs-surrogate` (POD, DMD, EDMD), `fs-bo`, `frankentorch` | B | — |
| Metal-accounting visuals | `frankenmermaid` (Sankey, flowchart) | B | — |

### 4.7 Tailings and geotechnics
| Need | Provider | Maturity | Gap |
|---|---|---|---|
| FEM (stress, deformation) | `fs-solid`, `fs-cutfem`, `fs-solver`, `fs-sparse` AMG, `fs-mesh` | B | Mohr–Coulomb and critical-state plasticity; consolidation |
| Seepage (Darcy) | `fs-conduction` (same Laplace/Poisson operator; L3) | B | Unconfined phreatic surface; transient flow |
| Slope stability (limit equilibrium: Bishop, Spencer, Morgenstern–Price) | none | — | **Build** |
| Instrumentation monitoring (piezometers, InSAR, prisms) | `fs-eproc`, `fs-assimilate`, `frankenpandas` | B | Trigger action response plan (TARP) logic (build) |

### 4.8 Labour, maintenance and HSE
| Need | Provider | Maturity | Gap |
|---|---|---|---|
| Reliability (Weibull with censoring, MTBF) | `fsci-stats` (`weibull_min`/`exponweib`, `CensoredData`, `logrank_censored`) | A | Reliability-centred maintenance (RCM) and optimal replacement policies (build) |
| Crew rostering and assignment | `fsci-opt::linear_sum_assignment`, `milp` (small) | A | Scalable rostering (build or decompose) |
| Maintenance critical path | `fs-tropical`, `fnx` `dag_longest_path` | B | — |
| Shift handover and radio transcription | `franken_whisper` (`fw`, diarization for up to 4 speakers) | B | Noisy radio channels; more than 4 speakers |
| Work-order and incident text extraction | `franken_nlp` (schema extraction, NER; *candidate*) | C | Qualification |
| Agent action governance | `franken_alignment` | B | — |

### 4.9 Finance
| Need | Provider | Maturity | Gap |
|---|---|---|---|
| Ledger of record | `franken_accounting` (double-entry, bitemporal, FrankenGraphDB) | B–C | Mining cost centres, inventory (stockpile valuation), royalties |
| DCF, NPV, IRR | `fsci-opt::brentq` (for IRR), `frankenpandas` | A | Trivial helpers (build) |
| Commodity price models (GBM, mean reversion, Schwartz–Smith), real options (LSM) | `fsci-stats` (distributions, QMC), `fs-uq` (MLMC), `fsci-linalg::lstsq` | A / B | **Build** |
| Warehouse connectivity | `franken_snowflake` | B | — |

### 4.10 Documents, reports, visualization and assurance
| Need | Provider | Maturity | Gap |
|---|---|---|---|
| Scanned PDF and image OCR | `franken_ocr` (Unlimited-OCR, GOT-OCR2) | B (released) | JPX, JBIG2, handwriting |
| Born-digital PDF text and table extraction (NI 43-101, JORC, S-K 1300) | none | — | **Build a pure-Rust text-layer extractor (focr already has partial PDF object parsing for images)** |
| Chart to data (grade-tonnage curves) | `franken_ocr` OneChart | B | — |
| Structured extraction, entity resolution | `franken_nlp` | C | Qualification |
| Provenance and search over reports | `franken_overlap`, `frankensearch` | B | — |
| Report generation | `franken_markdown` (`fmd`; HTML, PDF, EPUB) | A–B | Report templates |
| Diagrams | `frankenmermaid` (flowchart, Gantt, Sankey) | B | — |
| Explainer videos | `franken_manim` | B | — |
| 3-D interactive viewing (block models, pits, drillholes, point clouds) | `fs-viz`, `fs-render` (path tracing), `fs-img` (PNG/EXR) | B (offline) | **Interactive WebGPU/WASM 3-D viewer: build** |
| Formal invariants | `franken_lean` (`fln`) | C | Long-horizon |
| Symbolic derivations | `frankensympy` | C | — |

---

## 5. Capabilities FrankenMining would have to build itself

Ordered by criticality to the mining OS.

1. **Geostatistics engine.**
   - Experimental variograms (omnidirectional, directional, downhole) and anisotropy ellipsoids with three
     rotation angles.
   - Nested variogram models (nugget, spherical, exponential, Gaussian, Matérn) with fitting.
   - Kriging (simple, ordinary, universal, indicator, co-kriging, block and discretized), with kriging
     variance, slope of regression and kriging efficiency.
   - Search ellipsoids with octant and sector rules.
   - Gaussian anamorphosis (Hermite) and discrete Gaussian model change of support.
   - Conditional simulation (SGS, turning bands, LU decomposition) and post-processing.
   - Reuse `fsci-spatial::KDTree`, `fsci-linalg`/`fs-la` Cholesky, `fsci-special` Hermite, `fs-rand`,
     `fs-bo` (as a cross-check), and `frankenjax` (for maximum-likelihood fitting).
2. **Pit optimization kernel.**
   - Hochbaum pseudoflow (HPF), or Lerchs–Grossmann, on i64/i128 CSR arrays with on-the-fly precedence
     arcs from slope templates.
   - Parametric revenue-factor nested pits and pushback selection.
   - Validate against `fsci-sparse::maximum_flow` and `fnx-algorithms::preflow_push` on small instances.
3. **Scalable LP/MILP for scheduling.** Choose one or more:
   - a sparse simplex/IPM plus branch-and-cut;
   - a PDLP-style first-order LP (precedent: `fs-truss` PDHG);
   - mining-specific decompositions (Bienstock–Zuckerberg, TopoSort, Lagrangian, rolling-horizon).

   Add MPS/LP I/O, stochastic scenarios (two-stage, CVaR via `fs-robust`), and stockpile and blending
   constraints.
4. **Bayesian samplers.** HMC/NUTS (and possibly SMC or ADVI) built on `frankenjax` autodiff, with
   diagnostics (R-hat, effective sample size). Used for prospectivity models, resource uncertainty and
   metallurgical calibration.
5. **Drillhole data model.**
   - Collar, survey, interval and sample schema.
   - Desurvey (minimum curvature), compositing (length, bench, domain), interval overlap and merge
     algebra.
   - QA/QC rules and charts.
   - Importers for common vendor CSV exports.
6. **GIS and geodesy.**
   - CRS definitions (EPSG subset, local mine grids, Helmert, 3-D similarity transforms).
   - Projections (UTM/TM, Lambert), WGS84 and ECEF, geoid handling.
   - GeoTIFF/COG raster read and write.
   - Vector formats (GeoJSON, shapefile, DXF; reuse the `franken_textiles` DXF precedent).
   - Resampling and reprojection.
7. **Point-cloud and survey pipeline.**
   - LAS/LAZ I/O.
   - ICP (point-to-plane), RANSAC, statistical outlier removal, voxel decimation.
   - Ground filtering (cloth simulation or progressive morphological filter).
   - DTM/DSM rasterization, surface-to-surface volumes, breaklines and contours.
   - Build on `fs-rep-voxel`, `fsci-spatial`, `fs-asbuilt` and `fs-mesh`.
8. **Photogrammetry completion.**
   - Feature detection and description, matching, incremental or global SfM.
   - Scaling `fss-geometry::bundle` (sparse Schur, robust losses, GCPs plus RTK priors).
   - Dense MVS, orthomosaic and DSM.

   Alternatively, accept external photogrammetry outputs (LAS/LAZ plus GeoTIFF) as v1 inputs. That is a
   strong scoping recommendation.
9. **Geophysics toolkit.**
   - Gridded wavenumber-domain operators (upward and downward continuation, RTP/RTE, vertical and
     horizontal derivatives, tilt, analytic signal).
   - Levelling and microlevelling; Euler deconvolution.
   - Forward models and regularized inversion for magnetics, gravity, IP and EM.
   - Build on `fsci-fft`, `fsci-sparse` Krylov and `frankenjax` gradients.
10. **Classical ML for prospectivity.** Gradient-boosted trees and random forests, weights of evidence,
    calibrated logistic regression, spatial cross-validation. `frankentorch` covers neural nets only.
11. **Process-plant unit models.** Comminution (Bond, JK, population balance), classification, flotation
    kinetics, leach, CIL/CIP, SX-EW, thickening; mass-balance reconciliation with gross-error detection;
    a Gibbs-energy minimizer.
12. **Geotechnics.** Limit-equilibrium slope stability (Bishop, Spencer, Morgenstern–Price), soil and rock
    plasticity for `fs-solid`, phreatic seepage built on `fs-conduction`, and TARP trigger logic.
13. **Mining finance.** DCF/NPV/IRR helpers; commodity price stochastic models and scenarios; real options
    (Longstaff–Schwartz); royalty and tax regimes; stockpile inventory valuation in `franken_accounting`.
14. **Born-digital PDF extraction.** Text-layer and table extraction for technical reports; JPX and JBIG2
    decoding (a known `focr` gap).
15. **Interactive 3-D viewer.** WebGPU/WASM viewer for block models, pits, drillholes, point clouds and
    meshes. The suite only has offline rendering (`fs-render`, `fs-viz`) and 2-D diagrams
    (`frankenmermaid`).
16. **Mine-specific discrete-event simulation.** Haulage, crushers, stockpiles and plant, building on
    `fs-tropical` analytics.

---

## 6. Integration notes for FrankenMining

- **Recommended dependency core:**
  - `fsci-*` from crates.io 0.4.0: stable, SciPy-differential.
  - `frankenpandas`.
  - `frankensqlite`.
  - `asupersync`, which is mandatory: every surveyed AGENTS.md forbids tokio.
- **Selectively from frankensim**, via sibling path dependencies (the crate is version 0.0.1 and not
  published):
  - `fs-bo`, `fs-uq`, `fs-rand`, `fs-la`, `fs-sparse`;
  - `fs-rep-voxel`, `fs-rep-sdf`, `fs-mesh`, `fs-asbuilt`, `fs-io`;
  - `fs-voi`, `fs-eproc`, `fs-robust`, `fs-dfo`, `fs-tropical`.
- **frankensim dependency weight:** its contracts and receipts pull in `fs-evidence`, `fs-exec` (its own
  two-lane executor on asupersync) and `fs-blake3`. Budget for that, or vendor and adapt specific kernels
  under the suite license.
- **Use as test oracles, not production kernels:** `franken_networkx` (string-keyed, NetworkX-faithful),
  and `fsci-sparse::maximum_flow` and `fsci-opt::milp` for pit and schedule kernels.
- **Licensing:** most repos use "MIT plus OpenAI/Anthropic Rider" (`LicenseRef-MIT-OpenAI-Anthropic-Rider`).
- **Conventions to inherit:**
  - Rust 2024 nightly (several repos pin `nightly-2026-08-31`).
  - `#![forbid(unsafe_code)]`.
  - Fail-closed strict and hardened modes.
  - RaptorQ-protected evidence artifacts.
  - Beads (`br`/`bv`) for tasks.
  - `rch exec -- cargo ...` for builds.
  - No file deletion.
- **Honest-maturity reading:** several READMEs (franken_lean, franken_manim, franken_nlp) are deliberately
  written in "target-state present tense". Their "current status" or IMPLEMENTATION_STATUS sections are
  authoritative. The frankenscipy README understates spatial dimensionality: Delaunay and ConvexHull are
  N-D in source.
