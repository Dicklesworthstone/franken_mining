# R8: Numerics and Symbolic Stack for FrankenMining (frankensympy, franken_numpy, frankenpandas, frankenscipy, frankenjax)

Survey date: 2026-10-28. Read-only: nothing was built, run or written inside any surveyed repo.
This report extends R3 (`R3_numerics_ml_perception_survey.md`) and does not repeat it. R3 already
records crate lists, headline sizes and the broad capability matrix for fsci, fnp, fp, fj, ft and fnx.
R8 adds the first real survey of **frankensympy**, and goes deeper on the APIs FrankenMining will call.

Evidence labels:
- **FACT**: verified in source, with the path given.
- **HYPOTHESIS**: inferred from code or docs but not executed.
- **OPEN**: unresolved; needs a build, a benchmark or an owner answer.

Maturity grades (per need): **A** production-grade (published, oracle-differential, broad tests);
**B** usable with care; **C** prototype or narrow slice; **—** absent.
Decisions: **consume** (depend on the crate), **lift** (copy a kernel into `fm-*` under the suite
licence, with attribution and a parity test), **build** (write in `fm-*`).

## 0. Executive summary

1. **frankensympy is a design-time oracle, not a runtime dependency.**
   - It has exact rationals, exact linear algebra with certificates, `diff` (including `Piecewise`),
     symbolic and sparse Jacobians, lex-Gröbner polynomial solving, content-hashed terms and basic
     LaTeX.
   - It has no units, no piecewise or multiple integration (so no symbolic gamma-bar), and no
     `Max`/`Min` derivative.
   - Its "lambdify" is an f64 bytecode interpreter without `Piecewise`/`Min`/`Max`.
   - Its workspace references missing `../*-pin` paths, and it is self-declared pre-certification.
   - Consume it only through `fm-derive` (tests, golden formulas, LaTeX).
2. **There is no shared array type.** fsym, fnp, fp, fsci (linalg, sparse, ndimage, spatial each
   differ) and fj all use different representations. Build `fm-array` plus adapters.
3. **franken_numpy's `memmap` is not an mmap** (it reads into `Vec<u8>`), and `UFuncArray` stores every
   dtype as f64. Use fnp for RNG (bit-exact PCG64DXSM, Philox, `SeedSequence::spawn`) and `.npy`
   interchange only.
4. **frankenpandas** has `IntervalIndex`, but with linear-scan lookups and no overlap join. It has no
   decimal dtype, reads Parquet as a whole file, and its default features pull C SQLite. Consume it for
   I/O and telemetry; build drillhole interval algebra.
5. **frankenscipy** remains the backbone, but:
   - it has no sparse Cholesky and no preconditioned `cg`/`minres` (only GMRES and BiCGSTAB take a
     preconditioner; `spilu` is the only preconditioner);
   - its `linprog` "Highs" variants are a dense revised simplex or Mehrotra IPM, **not a HiGHS port**;
   - there is no QP and no censored MLE (`CensoredData` feeds only `logrank_censored`);
   - `RbfInterpolator` is capped at 4,096 points.
6. **frankenjax** traces Rust closures (`make_jaxpr`) and has grad, vmap, `hvp` and the special
   functions needed for log densities, but "jit" is interpretation, the GPU backend is a stub, and there
   is no WASM target. NUTS must be built (`fm-mcmc`, about 3–5K lines).
7. **Determinism.** Integer RNG streams are bitwise portable. Transcendentals go through the platform
   libm, so results are DeterministicPerIsa at best. FMA contraction is not a Rust hazard. Thread-count
   dependence is OPEN per kernel. Plan an `fm-libm` and per-ISA golden hashes.

---

## 1. Repo fact sheets

| Repo | HEAD | Toolchain | asupersync | Version / publish | Licence |
|---|---|---|---|---|---|
| frankensympy | `9d785d0 2026-10-27 Expose independently certified proper real integration in Python` | `nightly-2026-08-20`, target `wasm32-unknown-unknown` | path `../asupersync` (local checkout is 0.6.0 @ `523e86c00`); used only by `fsym-runtime`, `fsym-durable`, `fsym-python` | 0.1.0; root crate `publish = false`; no git tags; 1,006 commits since 2026-08-19 | MIT + OpenAI/Anthropic Rider |
| franken_numpy | `d32c4e04 2026-10-26 fix(linalg): solve coupled Schur blocks in eigenvector back-substitution` | `nightly-2026-08-31` | `=0.5.0`, `default-features = false`, optional (`fnp-runtime` feature) plus `fnp-conformance` | 0.5.0 | MIT + Rider |
| frankenpandas | `37bb94101 2026-10-26 merge(main): keep the rustfmt commit and the rust-deps bump` | `nightly-2026-08-31`, and `cargo-features = ["profile-rustflags"]` (nightly-only) | `0.5.0` optional (`fp-runtime` feature) | 0.6.0 | MIT + Rider |
| frankenscipy | `df0496783 2026-10-26 feat(sparse): add block and interval exponential actions` | `nightly-2026-08-31` | `0.5.0`, only in `fsci-conformance` | 0.4.0 (crates.io per R3) | MIT + Rider |
| frankenjax | `fa93fb77 2026-10-26 fix(api): keep soft-target gradients precise with masked teachers and offset logits (#1)` | see §6 | see §6 | see §6 | MIT + Rider |

FACT: every surveyed `LICENSE` opens "MIT License (with OpenAI/Anthropic Rider)". FrankenMining uses the
same suite licence, so there is no licence conflict inside the suite. Third-party crates pulled in by
frankenpandas (arrow and parquet 60, calamine, rusqlite with bundled C SQLite, optional hdf5-metno and
mysql) are permissive, but **some are C-backed**; see §7.4.

---

## 2. frankensympy (`fsym-*`): primary new target

### 2.1 Shape and maturity

- FACT: 29 crates under `crates/` totalling about **141.6K Rust lines and about 1,573 test markers**
  (`#[test]`/`proptest!`), plus 2 root integration tests and a 27-file Python test suite
  (`python/tests/`). The largest crates are `fsym-calculus` (18.1K lines, 177 tests), `fsym-core`
  (17.6K, 206), `fsym-runtime` (14.0K, 165), `fsym-polys` (11.1K, 101), `fsym-proof-kernel` (10.6K, 99),
  `fsym-python` (8.6K), `fsym-matrices` (7.6K, 97), `fsym-bigint` (7.4K, own NTT multiply) and
  `fsym-solvers` (7.3K, 87).
- FACT: `Cargo.toml` declares `implementation_status = "implemented_uncertified"`. README: "**not** a
  certified SymPy replacement: no compatibility profile is certified, no performance win is
  demonstrated". In `registries/claims.toml`, every compatibility and release claim (`COMPAT-001..004`,
  `RELEASE-001`) is `planned`, as is `INTEGRATION-003` ("FrankenNumPy and FrankenSciPy can consume
  verified symbolic residual, Jacobian ... artifacts"). The math, runtime and agent claims are
  `implemented_uncertified`.
- FACT: the oracle is live SymPy 1.14.0 (pinned upstream `16fa855354...`):
  `crates/fsym-conformance/src/bin/capture_sympy_oracle.rs`, `tools/conformance-lab/oracle_runner.py`,
  `crates/fsym-functions/tests/oracle_differential.rs`, `python/tests/test_oracle_differentials.py`.
  Coverage is per-feature slices, not a census of the kind fsci has.
- FACT: `#![forbid(unsafe_code)]` is a workspace lint. There is no C/C++ CAS or bignum FFI: integers are
  the in-house `fsym-bigint`; rationals are `fsym-rational::BigRational`.
- **Build hazard (FACT, plus HYPOTHESIS on effect).** The workspace manifest pins
  `fnp-linalg = { path = "../franken_numpy-pin/..." }` and `fsci-opt = { path = "../frankenscipy-pin/..." }`
  (used by `fsym-numeric-consumer`). Neither `-pin` directory exists on this machine.
  `registries/dependencies.toml` records `franken_numpy @ d5d29c9d` in the worktree
  `/data/projects/franken_numpy-pin`. HYPOTHESIS: Cargo will refuse to load the frankensympy workspace
  here until those worktrees exist. FrankenMining must therefore depend on individual `fsym-*` crates
  through its own vendored path or git pin, not through the frankensympy workspace.
- FACT: the algebra crates are **asupersync-free and Cx-free**. The dependency lists of `fsym-core`,
  `fsym-calculus`, `fsym-matrices`, `fsym-solvers`, `fsym-simplify`, `fsym-polys`, `fsym-rational` and
  `fsym-printing` contain only `fsym-*`, `num-*`, `serde`, `thiserror` and `blake3`. Only the
  orchestration layers (`fsym-runtime`, `fsym-durable`, `fsym-python`) need asupersync.

### 2.2 Public API relevant to FrankenMining (all FACT unless marked)

| Capability | Entry point (path) | Notes |
|---|---|---|
| Expression type | `fsym_core::Expr` (`crates/fsym-core/src/lib.rs:122`): `Sym`, `Integer(BigInt)`, `Rational(BigRational)`, `Const`, `Add(Vec)`, `Mul(Vec)`, `Pow(Arc,Arc)`, `Function(String, Vec<Expr>)` | Plain enum tree. Functions are **stringly-named**, so `Piecewise`, `Max` and user functions are all `Function("…")`. `Expr::symbol`, `from_i64`, `rational`, `subs`, `free_symbols`, `evalf`, `evalf_ball(precision)` |
| Parse | `fsym_core::parse` (`fsym-core/src/parser.rs`) | String to `Expr`; used by `Session::construct` |
| Content identity | `fsym_core::TermDag::insert_expr`, `compute_term_id` (`fsym-core/src/dag.rs:842,1412`) | Hash-consed DAG with a blake3 digest per term (claim `SEMANTIC-001`). The foundation for **auditable canonical formula IDs** |
| Differentiation | `fsym_calculus::diff(&Expr,&Symbol) -> Expr`, `diff_n`, `diff_unsimplified` (`fsym-calculus/src/lib.rs:69,756,763`) | Rules for elementary functions, `erf`/`erfc`, `LambertW`, orthogonal polynomials, `Heaviside` (gives `DiracDelta`), **`Piecewise` (branch-wise, conditions kept, line 446)**, `Sum`/`Integral`/`Subs`. **`Max`/`Min` fall through** to an unevaluated `diff(Max(..), x)` (line 571) |
| Jacobian (dense, symbolic) | `fsym_matrices::Matrix::jacobian(exprs, vars) -> Matrix` (`fsym-matrices/src/lib.rs:1732`) | |
| Sparse symbolic Jacobian | `fsym_calculus::SparseJacobian::try_build(residuals, vars)`, `try_eval_sparse`, `try_eval_compressed`, `verify_sparse_jacobian_certificate` (`sparse_jacobian.rs:166,489`) | Sparsity pattern plus receipt |
| Numeric compilation ("lambdify") | `fsym_calculus::CompiledExpr::try_compile(expr, var_map)`, `try_eval(&[f64])`; `CompiledResidualSystem::try_compile(exprs, vars)`, `try_eval_residuals`, `try_eval_jacobian`, `check_with_finite_differences` (`compile.rs:332,512,643,694,714,777`); `VerifiedResidualSystem` with JSON artifact (`verified.rs:209`) | A **stack-bytecode interpreter**, not native code. Ops: const, var, add, mul, neg, pow, sin, cos, tan, sinh, cosh, tanh, exp, ln, sqrt, abs (`compile.rs:98`). **No comparison, `Piecewise`, `Min`/`Max`, `floor` or `erf`**. Limits: `MAX_COMPILE_OPS = 8192`, `MAX_COMPILE_DEPTH = 128`. Refuses exponents that are not exactly representable |
| Source code emitters | `fsym_printing::{to_rust_code, to_c_code, to_python_code}` (`fsym-printing/src/lib.rs:453,589,528`) | Text emitters; an alternative path to native kernels through `build.rs` codegen |
| LaTeX | `fsym_printing::latex(&Expr) -> Result<String>` (`lib.rs:254`), `pretty` | Basic: `\frac`, `\sqrt[n]`, `\operatorname{}`, 7 Greek names. No `cases` environment for `Piecewise`, no `\sum` or `\int`. The Python shell's `python/sympy/printing/latex.py` (40 KB) is richer but needs CPython |
| Simplify | `fsym_simplify::{simplify, expand, cancel, simplify_with_assumptions, verified_simplify}` (`fsym-simplify/src/lib.rs:331,734,903,845,623`) | `verified_simplify` returns an independently replayed derivation receipt (WS07) |
| Exact rationals | `fsym_rational::BigRational`, `fsym_bigint::BigInt` | Exact; canonical byte encoding in `fsym-core/src/canonical.rs` |
| Exact linear algebra | `Matrix::{solve, solve_least_squares, inverse, rref, nullspace, rank, det, lu, qr, ldl, eigenvalues_with_sturm_certificate}` plus `verify_*_certificate` (`fsym-matrices/src/lib.rs:1832–1963, 2036–2761`); `pseudoinverse.rs`, `sparse.rs` | Symbolic and exact over Q with certificates. Dense `Vec<Expr>`, so suited to **formula derivation, not data-scale solving** |
| Polynomial systems | `fsym_solvers::solve_polynomial_system(eqs, vars) -> Vec<Vec<Expr>>` (`polysys.rs:74`), using a lex Gröbner basis (`fsym-polys/src/groebner.rs`, `fglm.rs`) | Zero-dimensional systems; refuses on Gröbner failure |
| Scalar solve | `solve`, `solve_linear`, `solve_quadratic`, `solve_univariate`, `solve_real_equation` (complete real solution sets for rational expressions), `solve_univariate_inequality` | |
| Series and limits | `series_expansion(expr, var, x0, n)`, `taylor(expr, var, at, order)` (order ≤ 12 on the general route; up to 256 coefficients for rationals), `limit`, `gruntz::limit` | **Univariate only.** A multivariate delta-method expansion must be composed from `diff` |
| Integration | `integrate`, `integrate::antiderivative`, `integrate::definite`, `transforms::integrate_definite`, `laplace_transform`, `fourier_transform`; `rational_definite_integral`; **certified quadrature** `quadrature::certified_integral(expr, var, lo, hi, abs_tol, opts) -> IntegralEnclosureReceipt` (rational enclosure, ≤ 512 intervals by default) | Rule-based: elementary table, Hermite and partial fractions, parts, substitution. **No `Piecewise` integration and no multiple integrals.** Certified quadrature is 1-D only |
| Units and dimensions | none | — FACT: no units crate, and no `physics` directory in `python/sympy/` |
| Agent session | `frankensympy::Session::{construct, differentiate, construct_claim, verify_claim, export_claim, replay_claim}` (`src/session.rs`) | Shown in `examples/external_consumer.rs`. The facade re-exports only `Expr`/`Symbol` and the session, so mining code must depend on `fsym-*` crates directly |

### 2.3 Fit against FrankenMining's symbolic uses

| Use | Verdict | Grade | Decision |
|---|---|---|---|
| Derive and check two-product and n-product formulas (for example, recovery `R = c(f−t)/(f(c−t))`) | `solve`/`solve_linear` plus `Matrix::solve` over Q do this exactly. `verified_simplify` gives receipts for identity checks | B | **Consume** at design and test time: derive, then freeze as golden formulas in `fm-metbal` tests |
| Lagrange-multiplier (Bilmat/JKMBal) reconciliation: symbolic KKT system, symbolic Jacobian of constraints | `Matrix::jacobian`, `SparseJacobian`, `Matrix::nullspace` (conservation-constraint basis) and `Matrix::solve` (closed-form KKT for small flowsheets) all exist | B (derivation); — (data-scale solving) | **Consume** for derivation and Jacobian sparsity. **Build** the numeric weighted-least-squares solver in `fm-recon` on fsci (§5) |
| Exact rational arithmetic | `BigRational`/`BigInt`, in-house | B | Consume for formula verification. For money, use franken_accounting's money type (R3/R4), not rationals |
| Unit and dimension algebra | absent | — | **Build** `fm-units` (a typed dimension vector in Rust types: t, g/t, oz, lb, %, $, $/t). This is cheaper and safer than symbolic units |
| Canonical forms of royalty, tax and stream trees (NSR, NPI, sliding scale) | `Expr` with `Piecewise`/`Max`/`Min` evaluates (`fsym-core/src/elementary.rs:1446–1750`); `TermDag` gives content IDs; `simplify` canonicalizes `Add`/`Mul` ordering | C | **Build** a domain AST (`fm-fiscal`) that *lowers into* `fsym_core::Expr` for canonical hashing, LaTeX and derivative checks. Do not make `Expr` the system of record: `Function(String)` is untyped and `Max` has no derivative rule |
| Lambdify to fast kernels | Bytecode interpreter; no `Piecewise`/`Min`/`Max`; 8,192-op cap | C | **Build** `fm-kernels` codegen: lower the fiscal and metallurgical AST to Rust source at build time using `to_rust_code` as a seed, or write a tiny closure-compiler. Use `CompiledResidualSystem` only for small smooth residual systems |
| Symbolic sensitivities (tornado charts, elasticities `∂NPV/∂p · p/NPV`) | `diff` works on smooth closed forms and on `Piecewise`. Real NPV models are long cash-flow programs, so symbolic expressions swell | C for whole-model use; B for closed-form unit models | Use **frankenjax forward-mode or dual numbers** for model-scale sensitivities (§6). Use frankensympy only to cross-check closed-form pieces |
| Delta method and series | Univariate `taylor`/`series_expansion`; multivariate via repeated `diff` | C | Build the delta method in `fm-uq` (gradient times covariance times gradient) on autodiff; use fsym for golden checks |
| Polynomial system solving | lex Gröbner (`solve_polynomial_system`) | B (small systems) | Consume for small derivations only |
| Gamma-bar (point-to-block, block-to-block averaged variograms) | No `Piecewise` integration (spherical model), no multiple integrals; certified quadrature is 1-D | — | **Build** numerical gamma-bar in `fm-geostat` (Gauss–Legendre block discretization, as in GSLIB and Isatis). Optionally use fsym `certified_integral` to certify 1-D reference values in tests |
| Lane's cut-off grade (limiting and balancing cut-offs) | Limiting cut-offs are linear in grade, so `solve_linear` derives them. Balancing cut-offs need the grade-tonnage curve, which is numeric | B (derivation) | **Build** in `fm-cutoff` with fsci `brentq` for balancing roots; keep fsym-derived closed forms as test oracles |
| LaTeX for QP reports and galaxy-brain cards | Basic Rust `latex()` | C | **Consume** for simple formulas. **Upstream ask**: `Piecewise` to `cases`, `Sum`/`Integral`, a symbol-name map. Otherwise template LaTeX in `fm-report` |

**Verdict on frankensympy:** a well-engineered, proof-carrying CAS **kernel** that is honestly labelled
pre-certification. For FrankenMining it is a **design-time and test-time oracle**: derive formulas, check
identities, produce symbolic Jacobian sparsity, print simple LaTeX. It is **not a runtime dependency on
any critical path**. It has no units, no piecewise or multiple integration, no `Min`/`Max` derivative,
an interpreter-only numeric compiler, and a workspace that does not resolve on this machine. It does
raise R3's "LOW" rating to "LOW-MEDIUM" because of exact linear algebra with certificates and content-hashed terms.

---

## 3. franken_numpy (`fnp-*`): deepening R3

### 3.1 Array model and memory layout

- FACT: `fnp-ndarray` (5.3K lines) is **layout metadata only**: `NdLayout`, `MemoryOrder`,
  `broadcast_shape(s)`, `contiguous_strides`, `broadcast_strides`, `views_overlap`
  (`crates/fnp-ndarray/src/lib.rs`). It holds no element buffer.
- FACT: the concrete array is `fnp_ufunc::UFuncArray { shape: Vec<usize>, values: Vec<f64>, dtype:
  DType, integer_sidecar: Option<IntegerSidecar> }` (`crates/fnp-ufunc/src/lib.rs:2936`).
  **Every dtype is stored as `f64`**. i64 and u64 keep exactness through `IntegerSidecar::{I64(Vec<i64>),
  U64(Vec<u64>)}` (line 2727) alongside the f64 buffer. Strided views use `UFuncArrayView` with a
  shared buffer, offset and strides (line 2996).
  - Consequence (FACT-derived): a bool mask costs 8 B per cell, an f32 grid costs 8 B per cell, and an
    i64 array costs 16 B per cell. A 10⁹-cell block model of 5 f64 attributes is about 40 GB resident.
- FACT: `DType` has 18 variants, including `Bool`, `I64`, `F32`, `F64`, `Complex128`, `DateTime64` and
  `Structured` (`crates/fnp-dtype/src/lib.rs:27`), with an exhaustive 324-pair `const fn` promotion table
  (README).
- FACT: masked arrays exist: `fnp_ufunc::MaskedArray { data, mask: Option<UFuncArray>, fill_value,
  hard_mask }` (`lib.rs:38766`), with the mask stored as f64 0.0/1.0.
- FACT: `fnp-ufunc/src/lib.rs` is a single **3.3 MB** file (99.5K lines, 2,514 tests). Compile times
  for consumers will be material (HYPOTHESIS; OPEN to measure).

### 3.2 RNG for seeded geostatistical realizations

- FACT: five bit generators in `crates/fnp-random/src/lib.rs`: `Pcg64Rng` (870), `Pcg64DxsmRng` (1028),
  `PhiloxRng` (1266), `Sfc64Rng` (1449), `Mt19937Rng` (1523). There is also `SeedSequence` (1775) with
  `spawn(n_children)`/`spawn_key()` (1920, 1833), and `Generator`/`default_rng`.
- FACT: splittable and jumpable. `Pcg64*::advance(delta: u128)` (1006, 1156);
  `PhiloxRng::{jump_ahead(steps), jump_2_128(jumps), advance_like_numpy}` (1333–1361);
  `Mt19937Rng::jump_2_128` (1609). The README claims a **bit-exact PCG64DXSM core stream versus NumPy**
  for explicit seeds, and 40+ oracle-verified distributions.
- HYPOTHESIS: bit-exactness covers the *integer stream*. Distribution samplers (ziggurat normal tails,
  gamma, beta) call `exp`/`ln`. In pure Rust these go through the platform libm; `fnp-python` declares
  35 glibc libm symbols via `unsafe extern "C"` (README line 7). Cross-ISA bitwise equality of
  *normal/gamma samples* (x86-64 glibc vs aarch64 macOS) is therefore **not established** (OPEN).
- Decision: **consume `fnp-random` with `default-features = false`** (the generator plus `SeedSequence`
  core, per README line 580) for `fm-rng`. Key streams as `SeedSequence(root).spawn` by
  (realization, domain, node block). Alternatively use `fs-rand` Philox (R3). Either way, put an
  in-house `fm-rng` facade in front so the underlying stream can be pinned and golden-tested.

### 3.3 Linear algebra, FFT, einsum

- FACT: `fnp-linalg` works on **flat row-major `&[f64]` plus `n`**:
  - `cholesky_nxn(a, n)`, `cholesky_solve(l, b, n)`, `cholesky_solve_multi`;
  - `solve_nxn`, `solve_triangular`;
  - `qr_mxn`, `svd_mxn_full`, `eigh_nxn`, `eig_nxn_full`;
  - `lstsq_svd`, `lstsq_tsqr` (`crates/fnp-linalg/src/lib.rs:1044–8565`).

  It is dense only, safe Rust with rayon, and links no BLAS by default. The `ffi/` module is a
  quarantined, off-by-default `cblas-lapack` *shape-validation* surface (`ffi/mod.rs:1–10`).
- FACT: `UFuncArray::{dot, matmul, tensordot, lstsq, cholesky, eig, svd, fft, rfft, einsum(subscripts,
  operands)}` (`fnp-ufunc/src/lib.rs:14399–24455`). The Rust FFT is Cooley–Tukey plus Bluestein. In
  Python, 17 of 18 `numpy.fft` entry points delegate to real NumPy (README line 1376), so the
  **Python-side** parity evidence does not exercise the Rust FFT.
- Fit: many small kriging systems (n ≈ 20–200) are better served by `fs-la` batched Cholesky or
  `fsci-linalg::solve_with_casp` (R3). Use fnp-linalg as a third implementation for differential tests.

### 3.4 Large-array I/O

- FACT: `.npy` and `.npz` read and write: `save`, `load`, `load_auto`, `savez`, `savez_compressed`,
  `load_npz`, `read_npy_header`, structured and string variants (`crates/fnp-io/src/lib.rs:946–6751`).
- FACT: **"memmap" is not an OS memory map.** `MemmapArray` holds `buffer: Vec<u8>` ("In-memory buffer
  holding the array data", `lib.rs:5713–5725`). `memmap`/`open_memmap`/`memmap_npy` read the file region
  into RAM, and `to_f64_values()` decodes into a second `Vec<f64>`. The crate has no `memmap2`
  dependency (deps: `bytemuck`, `crc32fast`, `flate2`), consistent with `forbid(unsafe_code)`.
- Consequence: 10⁸ cells (0.8 GB) is fine. 10⁹ cells (8 GB per attribute, doubled during decode) is
  **not out-of-core**. FrankenMining needs its own chunked block-model store (`fm-blockstore`: tiled
  `.npy`-compatible chunks or Zarr-like layout, possibly on `fs-rep-voxel` VDB for sparse models).
  Use fnp-io only for interchange of modest arrays.

### 3.5 franken_numpy verdict

Consume `fnp-random` (core) and `fnp-io` (`.npy`/`.npz` interchange). Do not adopt `UFuncArray` as
FrankenMining's array type: it stores everything as f64, sits in a 3.3 MB single-file crate, and has
no real mmap. Use fnp-linalg as a differential-test oracle. Grade: RNG **A**, npy I/O **A−**,
linalg **B** (dense, no BLAS), arrays-as-substrate **C** for mining scale.

---

## 4. frankenpandas (`fp-*`): deepening R3

- FACT, intervals:
  - `fp_types::Interval` with `overlaps()` (`crates/fp-types/src/lib.rs:10142,10298`).
  - `fp_index::IntervalIndex { values: Vec<Interval>, closed, name }` (`crates/fp-index/src/lib.rs:21486`)
    with `from_breaks`, `from_tuples`, `from_arrays`, `overlaps`, `contains`, `get_loc`, `get_indexer`,
    `is_overlapping`, `is_non_overlapping_monotonic`.
  - **`get_loc` is a linear scan, so `get_indexer` costs O(n·m)** (lib.rs ≈ 21810–21850). There is no
    interval tree.
  - **No interval-interval join**: `fp-join` mentions intervals only as key labels. `merge_asof` and
    `merge_asof_with_options(left, right, on, direction, MergeAsofOptions{allow_exact_matches,
    tolerance, by})` (`crates/fp-join/src/lib.rs:11484–11501, 11013`) are point-based, and `on` "must be
    numeric and sorted".
  - R3 is confirmed: drillhole from/to compositing and overlap joins must be **built** (`fm-drillhole`:
    sweep-line overlap join per hole ID, length-weighted compositing, desurvey).
- FACT, binning: `cut`, `cut_bins`, `qcut`, `qcut_with_edges` (`crates/fp-frame/src/lib.rs:77196–77633`).
  These are useful for grade bins and grade-tonnage tables.
- FACT, time series: `rolling`, `rolling_offset`, `rolling_closed`, `rolling_center`, `expanding`, `ewm`
  (`fp-frame/src/lib.rs:36031–36217`); `resample(freq)`/`resample_ext` (36272); a group-by-resample on
  Series groupby (58076).
- FACT, column storage: `ColumnData::{Float64(Arc<[f64]>), Int64, Bool, Utf8(Vec<String>), Timedelta64,
  Datetime64, Period, Interval(Vec<Interval>)}` (`crates/fp-columnar/src/lib.rs:1146`). `DType` includes
  `Categorical` plus nullable variants (`fp-types/src/lib.rs:75`). `CategoricalDtype`, `CategoricalIndex`
  and `CategoricalAccessor` exist. Categorical *column storage* appears not to be a dedicated
  codes-plus-categories buffer (HYPOTHESIS; OPEN for lithology codes on large interval tables).
- FACT, **no decimal dtype**: only a `PandasDecimal` parse helper (`fp-types/src/lib.rs:1422`). Money in
  dataframes is f64, so ledger-grade amounts must stay in franken_accounting types.
- FACT, I/O entry points in `crates/fp-io/src/lib.rs`: `read_csv(_with_options)`, `read_fwf`,
  `read_parquet`, `read_feather`, `read_excel(_sheets)`, plus SQL with chunked `read_sql_chunks`.
  - `read_parquet` is **`std::fs::read` of the whole file** followed by an Arrow record-batch decode
    (line 12402), so there is no predicate or column pushdown and no streaming.
  - Excel is read via `calamine`. Vendor exports (acQuire, Datamine, Surpac, Leapfrog) are CSV/Excel or
    proprietary binary (Datamine `.dm`, Surpac `.str`/`.dtm`). The proprietary formats are **absent**;
    build them in `fm-io-vendor`.
- FACT, C-backed dependencies:
  - The default feature `sql-sqlite` pulls `rusqlite` with `bundled` (C SQLite).
  - `hdf5` (C) and `mysql` are optional.
  - `fp-dot-kernel` is compiled with `-Ctarget-feature=+avx2,+fma` in the `release` and `release-perf`
    profiles (root `Cargo.toml`).

  FrankenMining should use `default-features = false` and frankensqlite for SQL. See §7.3 for the
  determinism impact of FMA.
- FACT, performance: README line 934 reports 146 certified benchmark lanes against pandas 2.2.3, an
  overall geomean of **3.97×**, and 10 lanes still losing (worst `df_transpose_full_materialize` at
  0.004×). Small 100k-row lanes lose to a 350–420 µs per-call thread spawn (README line 2668).
- Verdict: **consume** frankenpandas (`default-features = false`) for ingest, telemetry resampling and
  report tables. Grade **A−** for CSV/Excel/Parquet and time-series work, **C** for interval algebra.
  Build drillhole interval algebra and vendor formats.

---

## 5. frankenscipy (`fsci-*`): deepening R3

R3 established the breadth: 19 crates, 0.4.0 on crates.io, 99.1% of SciPy 1.17.1 symbols "real".
R8 checks the specific entry points FrankenMining will call. Source sizes (`src/` only, FACT):
`fsci-stats` 142.9K lines (`lib.rs` alone is 4.1 MB), `fsci-sparse` 90.6K (`linalg.rs` is 1.9 MB),
`fsci-linalg` 74.4K, `fsci-opt` 68.5K, `fsci-integrate` 45.2K, `fsci-signal` 42.0K, `fsci-ndimage` 32.2K,
`fsci-interpolate` 28.9K, `fsci-spatial` 20.2K.

### 5.1 Sparse linear algebra (SPDE/GMRF geostatistics on 10⁶–10⁸ nodes)

- FACT: `CsrMatrix { data: Vec<f64>, indices: Vec<usize>, indptr: Vec<usize>, .. }`
  (`crates/fsci-sparse/src/formats.rs:601`). Indices are `usize`, so there is **no int32 limit**, at a
  cost of 8 B per index. R3's int32 caveat applies only to `maximum_flow` capacities.
- FACT: Krylov solvers in `crates/fsci-sparse/src/linalg.rs`:
  - `cg(a: &dyn LinearOperator, b, x0, IterativeSolveOptions{mode, check_finite, tol, max_iter})`
    (8595);
  - `gmres` (9612), `lgmres` (9999), `bicgstab` (10705), `minres` (11204), `lsqr` (11388), `lsmr` (11816).
- FACT: preconditioning exists only as **`gmres_preconditioned`** (9332) and
  **`bicgstab_preconditioned`** (9495), taking `M: Fn(&[f64]) -> SparseResult<Vec<f64>>`. **`cg` and
  `minres` have no preconditioner argument.** The only shipped preconditioner is `spilu` (6544).
  There is no AMG and no incomplete Cholesky.
- FACT: direct solvers are `spsolve` (5936, with an internal *banded* Cholesky fast path for n ≥ 256 and
  ≤ 8 nonzeros per row, lines 879–882), `splu` (COLAMD ordering, `PermutationOrdering`, line 62),
  `factorized` and `spsolve_triangular`. **There is no general sparse Cholesky, no supernodal
  factorization, no selected inversion, and no log-determinant**, matching SciPy.
- FACT: `eigsh` (35444); `expm_multiply` (47396), plus the new block/interval exponential actions at HEAD.
- Consequence for SPDE-Matérn (Lindgren–Rue) on 10⁶–10⁸ nodes, which needs:
  - sparse Cholesky of the precision Q, its log-determinant and sampling, or
  - preconditioned CG with AMG and stochastic log-determinant estimates.

  fsci provides **neither at scale**. R3's `fs-sparse::direct` (supernodal multifrontal LDLᵀ with AMD)
  and `fs-sparse` AMG are the substrate. Lift those into `fm-gmrf`, or consume them through a frankensim
  path dependency. Use fsci `cg` and `minres` as unpreconditioned reference oracles.
- Grade: Krylov **A** (SciPy parity); preconditioned SPD at 10⁷ scale **—**.
  Decision: **build `fm-gmrf`** on lifted `fs-sparse` kernels. **Upstream ask**: a preconditioned `cg`.

### 5.2 Optimization

- FACT, LP: `linprog(c, a_ub: &[Vec<f64>], b_ub, a_eq: &[Vec<f64>], b_eq, bounds, LinprogOptions)`
  (`crates/fsci-opt/src/linprog.rs:204`).
  - `LinprogMethod::{Highs, HighsDs, HighsIpm, ..}` (line 83): `Highs` and `HighsDs` run a **bounded
    revised primal simplex**; `HighsIpm` runs **Mehrotra predictor–corrector with crossover**.
  - This is **not a HiGHS port**. The variants are named for SciPy compatibility only (the doc comment
    says "fsci runs the bounded revised simplex").
  - Constraint rows are dense `Vec<f64>`.
- FACT, MILP: `milp(MilpProblem{c, integrality, a_ub, b_ub, a_eq, b_eq, bounds}, MilpOptions{max_nodes,
  lp_maxiter})` (`crates/fsci-opt/src/lib.rs:1260–1340`). It is branch-and-bound over dense LPs, as R3
  says.
- FACT, QP: there is **no dedicated QP solver**. Use `minimize` with SLSQP (`slsqp.rs`, Kraft TOMS 733)
  or `trust_constr`. Linear least squares: `nnls(a: &[Vec<f64>], b)` (lib.rs:3855) and
  `lsq_linear(a, b, lb, ub)` (4243).
- FACT, NLLS: `least_squares(residuals: Fn(&[f64]) -> Vec<f64> + Sync, x0, LeastSquaresOptions{method,
  gtol, xtol, ftol, max_nfev, diff_method, diff_step, ..})` (`curvefit.rs:339`), plus
  `least_squares_bounded` (1155), `least_squares_many` (1288) and `curve_fit` (1014).
  - The Jacobian is **finite-difference by default**. No analytic-Jacobian or Jacobian-sparsity
    parameter appears in this signature (HYPOTHESIS: other variants may accept one; OPEN).
- Data reconciliation fit:
  - *Linear* (total-flow) reconciliation has a closed form, x̂ = x − V Aᵀ(A V Aᵀ)⁻¹(A x), so it needs
    only dense `fsci-linalg::solve_with_casp`.
  - *Bilinear* (component) reconciliation suits `trust_constr` with equality constraints, given
    analytic Jacobians from fsym (§2) or frankenjax (§6).
  - **Build `fm-recon`**, with gross-error detection (global test, measurement test, MIMT) built on
    `fsci-stats` χ² and normal distributions.
- FACT, roots for IRR: `brentq(f, bracket, RootOptions)` (`root.rs:121`), `brenth`, `ridder`,
  `toms748`, `newton`, `root_scalar`, `fsolve`, `root`. **Consume** these; IRR needs sign-change
  bracketing and multiple-root reporting (build a thin wrapper).
- Grade: LP/MILP **A** for SciPy parity, **—** at mine-scheduling scale. NLLS and roots **A**.

### 5.3 Statistics

- FACT, univariate distributions in `crates/fsci-stats/src/lib.rs`: `Lognormal` (6361), `GammaDist`
  (5239), `BetaDist` (4680), `Weibull` (5881), `Normal` (1820). There are 108 `impl
  ContinuousDistribution for` blocks.
- FACT, multivariate distributions: `Dirichlet` (8795) and `MultivariateT` (9469), the latter useful for
  robust geochemistry and t-copula price shocks.
- FACT, KDE: `GaussianKde` (50102).
- FACT, QMC in `qmc.rs`: `SobolSampler` (314), `HaltonSampler` (44), `LatinHypercubeSampler` (2266),
  plus `SobolIndicesResult` (lib.rs:42293), i.e. Sobol sensitivity indices, which suit
  variance-based tornado and global sensitivity analysis.
- FACT, censoring: `CensoredData { uncensored, left, right, interval }` (`censored.rs:15`) is consumed
  only by `logrank_censored` (lib.rs:38184). **There is no censored MLE fit.**
  - `fit_mle(data: &[f64], parameters, make_distribution, MleOptions)` (`mle.rs:162`) takes plain
    `&[f64]`.
  - Assays below the detection limit (left-censored lognormal or Tobit, ROS, Kaplan–Meier for
    left-censored data) must be **built** in `fm-geochem` by writing the censored log-likelihood
    (logcdf for censored points) and maximizing it with `minimize`.
  - **Upstream ask**: `fit_mle_censored`.

### 5.4 Spatial, interpolation, ndimage, ODE, signal

- FACT: `KDTree::new(data: &[Vec<f64>])` builds a node-ordered contiguous coordinate slab
  (`crates/fsci-spatial/src/lib.rs:3108–3131`). Queries: `query`, `query_many`, `query_k`,
  `query_k_many`, `query_ball_point`, `query_ball_point_many` (3219–3501). The metric is Euclidean; for
  anisotropic search ellipsoids, rotate and scale coordinates first.
  - No KDTree benchmark figure was found in the README. **Performance at 10⁶–10⁷ samples is OPEN**;
    benchmark it in `fm-geostat` before committing.
  - The `&[Vec<f64>]` input costs one heap allocation per point at build time.
- FACT: `RbfInterpolator { points: Vec<Vec<f64>>, weights, .. }` is a global dense solve with
  **`MAX_RBF_POINTS = 4096`** (`crates/fsci-interpolate/src/lib.rs:6214–6227`). There is no
  `neighbors` local mode, so it **cannot support implicit modelling of 10⁵–10⁶ contact points**.
  Build a local or partition-of-unity RBF, or a FastRBF-class solver, in `fm-implicit`. `griddata`
  (3786) is unchanged from R3.
- FACT: morphology: `binary_dilation_with_structure` (`crates/fsci-ndimage/src/lib.rs:7330`), but
  `grey_dilation(input, size, mode, cval)` (11272) takes only a cubic `size` window, with `_axes` and
  `_with_origins` variants. There is no arbitrary grey footprint, which slope-cone dilation for pit
  precedence needs. Binary dilation with a cone structure works for a *binary* pit shell. Precedence
  arcs belong in `fm-pseudoflow` anyway.
- FACT: ndimage has **its own array type** `NdArray { data: Vec<f64>, shape, strides }` (ndimage
  lib.rs:546).
- FACT: `solve_ivp(fun: &mut FnMut(f64, &[f64]) -> Vec<f64>, &SolveIvpOptions)`
  (`crates/fsci-integrate/src/api.rs:1296`), plus an audited variant. It allocates once per right-hand
  side call, which is fine for leach, flotation and CIL kinetics (which have small state). **Consume.**
- FACT: `welch` (`crates/fsci-signal/src/lib.rs:12562`) and `find_peaks(x, FindPeaksOptions)` (4192),
  with the R3 list (STFT, CWT, filter design), cover vibration and condition monitoring (spectra,
  bearing peaks, envelope via `hilbert`). **Consume**; build order tracking and kurtogram on top.

### 5.5 frankenscipy verdict

**Consume as the default numerics dependency** (crates.io 0.4.0, pinned exactly). Its gaps for mining
are:
- preconditioned or Cholesky-class sparse SPD solvers at scale;
- sparse, scalable LP/MILP and QP;
- censored MLE;
- local RBF;
- grey footprints;
- a common array type.

---

## 6. frankenjax (`fj-*`): autodiff substrate for samplers

- FACT: `nightly-2026-08-31`; version 0.1.0. Cargo metadata says `license = "MIT"`, but the `LICENSE`
  file is MIT plus Rider. asupersync `0.5.0` is optional (`fj-runtime` feature `asupersync-integration`)
  and is used by `fj-conformance` (with `test-internals`).
- FACT: largest crates by `src/` lines: `fj-lax` 162K, `fj-interpreters` 38.5K, `fj-ad` 34.9K,
  `fj-dispatch` 24.2K, `fj-trace` 13.6K, `fj-core` 12.3K, `fj-api` 9.8K. `fj-backend-gpu` is only 231
  lines: "V1 reports unavailable ... CUDA and wgpu integration are deferred contracts"
  (`crates/fj-backend-gpu/src/lib.rs`). `fj-ffi` is "the ONLY crate ... permitted to use `unsafe`".
- FACT, tracing from Rust closures:
  - `fj_trace::make_jaxpr(f: FnOnce(&[TracerRef]) -> Vec<TracerRef>, in_avals: Vec<ShapedArray>)`
    (`crates/fj-trace/src/lib.rs:6501`);
  - `fj_api::trace_jaxpr_for_values(f, args: &[Value]) -> Jaxpr` (`crates/fj-api/src/transforms.rs:3073`);
  - control flow: `trace_cond`, `trace_switch`, `trace_while_loop`.
- FACT, transforms (`transforms.rs:329–647`): `jit`, `grad`, `vmap`, `value_and_grad`, `jacfwd`,
  `jacrev`, `hessian`, `jvp`, `vjp`, `linearize`, `checkpoint`, and `hvp(jaxpr, primals, tangents)`
  (3066). They operate on a `Jaxpr`. Values are `fj_core::Value::{Scalar(Literal), Tensor(TensorValue
  {dtype, shape, elements})}` (`crates/fj-core/src/lib.rs:941,3384`), with flat row-major storage.
- FACT: **"jit" means partial evaluation plus e-graph optimization, then direct interpretation; there is
  no code generation** (README line 563). Per-primitive interpretation overhead therefore dominates for
  scalar-heavy models. Vectorized log densities (one `Dot`/`Cholesky` per evaluation) amortize it
  (HYPOTHESIS; OPEN to benchmark).
- FACT: primitives needed for log densities exist (`crates/fj-core/src/lib.rs:208–306`): `Log1p`,
  `Expm1`, `Logistic`, `Erf`, `Erfc`, `ErfInv`, `Lgamma`, `Digamma`, `Igamma`, `BesselI0e`, `Cholesky`,
  `TriangularSolve`.
- FACT, PRNG: `random_key(seed)` and `random_split(key)` (`crates/fj-lax/src/threefry.rs:113,123`). The
  threaded gamma/beta rejection sampler "must be BIT-for-bit identical to the serial map" (line 1764),
  so this code path is thread-count invariant.
- FACT: no WASM target appears in any `Cargo.toml`. No MCMC, HMC, NUTS or VI exists (R3, confirmed:
  no `leapfrog`/`nuts`/`metropolis` in `fj-api` or `fj-lax`).

### 6.1 What a minimal NUTS (`fm-mcmc`) needs on top

1. **Model to Jaxpr.** The model author writes `logp(θ_unconstrained)` as a traced closure. Bijectors
   apply with log-Jacobian terms: log for positive, logit for (0,1), stick-breaking for simplex,
   LKJ-Cholesky for correlation. Then `value_and_grad` gives a `Jaxpr` cached once.
2. **Integrator.** Leapfrog in plain Rust over `Vec<f64>`, calling the cached gradient evaluation. This
   keeps the tree logic outside the traced program, so no `while_loop` tracing is needed.
3. **NUTS core.** Multinomial NUTS (Betancourt 2017) with iterative tree doubling, the generalized
   U-turn criterion, max depth 10, and a divergence flag when ΔH > 1000.
4. **Adaptation.** Stan-style warm-up: dual-averaging step size (target acceptance 0.8) and windowed
   Welford diagonal mass matrix (windows 75/25/50 plus doubling). Dense metric optional.
5. **Chains and RNG.** One stream per chain from `fm-rng` (`SeedSequence.spawn`). Run chains
   independently (scheduled on asupersync or rayon), with deterministic per-chain output independent of
   scheduling.
6. **Diagnostics.** Rank-normalized split R-hat; bulk and tail ESS (Vehtari et al. 2021); E-BFMI;
   divergence and treedepth counts; an MCSE estimate.
7. **Validation.** Simulation-based calibration plus posteriordb-style reference posteriors (Gaussian,
   eight schools, logistic regression), compared against stored CmdStan draws as an *offline* oracle.

Estimated size: 3–5K lines plus tests. A fallback that is also needed for 10⁴–10⁵-dimensional latent
GMRF fields is INLA-style Laplace approximation on `fm-gmrf`. Plain NUTS is not the tool for SPDE
latent fields at mining scale.

GPU and WASM: absent in frankenjax. Browser deployment of samplers would need `fm-mcmc` compiled to
`wasm32` with a non-fj gradient path (hand-written or forward-mode dual numbers). OPEN: whether `fj-*`
builds for `wasm32-unknown-unknown` at all (`fj-ffi` uses raw function pointers; it can probably be
feature-gated out).

Grade: autodiff, vmap and jit-as-interpretation **B**; samplers **—**. Decision: **consume**
`fj-api`/`fj-trace`/`fj-ad` (git pin) as the gradient engine for `fm-mcmc` and for ML variogram and GP
fitting. **Build** the samplers.

---

## 7. Cross-cutting findings

### 7.1 No shared ndarray type: FrankenMining needs adapters (FACT)

| Crate | Array representation |
|---|---|
| fsym | `Expr` tree; `fsym_matrices::Matrix` holds a dense `Vec<Expr>` |
| fnp | `UFuncArray { values: Vec<f64>, integer_sidecar }`; `fnp-linalg` takes a flat row-major `&[f64]` plus `n` |
| fp | `ColumnData::Float64(Arc<[f64]>)`, etc. |
| fsci-linalg / fsci-opt | `&[Vec<f64>]` (one Vec per row) |
| fsci-sparse | `CsrMatrix` (usize indices) and `&dyn LinearOperator` |
| fsci-ndimage | own `NdArray { data: Vec<f64>, shape, strides }` |
| fsci-spatial | `&[Vec<f64>]` input |
| fsci-arrayapi | `CoreArray` (used only by conformance) |
| fj | `Value::Tensor(TensorValue { shape, elements: LiteralBuffer })` |

Decision: **build `fm-array`**, a contiguous `Arc<[T]>` plus dims and strides in row-major order, typed
`T ∈ {f64, f32, i64, u8, bool-bitset}`. Add `fm-numerics-bridge` with:
- zero-copy views into `&[f64]` (fnp-linalg, fsci-sparse, fp `Arc<[f64]>`);
- copying adapters to `&[Vec<f64>]` (fsci-linalg/opt) and to `NdArray`/`TensorValue`.

Hot loops must not round-trip through `Vec<Vec<f64>>`. That is an upstream ask to fsci (§10).

### 7.2 asupersync and Cx (FACT)

No numeric kernel in any of the five repos needs asupersync or a `Cx`. It is optional or confined to
conformance in fnp (`=0.5.0`), fp (`0.5.0`), fsci (`0.5.0`, conformance only) and fj (`0.5.0`).
frankensympy's `fsym-runtime`, `fsym-durable` and `fsym-python` use a *path* dependency on
`../asupersync` (0.6.0 locally), while `fsym-durable`'s manifest notes that the pinned commit resolves
0.5.0 and that "a second asupersync build unit requires a written admission review".

Posture: FrankenMining owns concurrency (asupersync at the suite pin chosen in R2). Every numeric crate
is consumed with its asupersync feature **off**, and the fsym runtime and orchestration crates are not
consumed. That avoids a 0.5.0/0.6.0 split.

### 7.3 Determinism (the suite's DeterministicPerIsa class)

1. **Integer RNG streams are bitwise portable.** FACT: PCG64DXSM, Philox (fnp), ThreeFry (fj) and fs-rand
   Philox (R3) are integer arithmetic.
2. **FMA is not the main hazard in Rust.** rustc never contracts `a*b + c` into a fused multiply-add
   without an explicit `mul_add`, and `f64::mul_add` is always correctly rounded (software fallback),
   so results do not depend on the ISA (fsci-linalg has 194 `mul_add`/fma occurrences). The `+avx2,+fma`
   rustflags on `fp-dot-kernel` are a speed and portability concern (HYPOTHESIS: on aarch64 they produce
   an "unknown feature" warning, not an error), unless the lane width of hand-chunked SIMD sums changes
   with target features (OPEN).
3. **Transcendentals are the real hazard.** `f64::exp`, `ln`, `sin` and `powf` call the platform libm
   (glibc on Linux, Apple libm on macOS). Normal, gamma and beta samplers (fnp-random, fj-lax), fsci-stats
   CDFs and PPFs, and fsym's `CompiledExpr` will therefore be deterministic *per ISA and OS* at best, and
   not bitwise across x86_64-linux and aarch64-darwin. HYPOTHESIS, consistent with the fnp README scoping
   bit-exactness to the "core stream".
4. **Parallel reductions.** rayon appears in fnp-linalg GEMM, fnp-random fills (jump-ahead partitioned,
   so deterministic by construction per the fnp README) and fsci-sparse. fp uses a per-call scoped thread
   fan-out. Whether results depend on thread count is **OPEN per kernel**. fj states threaded and serial
   gamma/beta are bit-identical (FACT).
5. **fsym** results are exact (rational) and content-hashed, so they are bitwise reproducible by design.
   The replay claim `RUNTIME-003` is `implemented_uncertified`.

Recommendation:
- Classify every consumed API into one of four determinism classes: **Exact**, **BitwisePortable**,
  **DeterministicPerIsa** (single-threaded, platform libm), or **ThreadVariant**.
- Run `fm-*` kernels that must reproduce across ISAs (resource estimates, published realizations) on
  an **`fm-libm`** (a pure-Rust, fixed-algorithm or correctly-rounded exp/log/pow/erf port) and on
  fixed-chunk reductions.
- Gate them with per-ISA golden hashes for x86_64-linux and aarch64-darwin.

### 7.4 Licences, unsafe code and C dependencies

- All five repos are MIT + Rider. This is compatible with FrankenMining under the same licence (FACT).
- C or `unsafe` code inside dependencies:
  - frankenpandas default feature `sql-sqlite` (rusqlite, bundled C SQLite), optional `hdf5` (C);
  - `fnp-python` (67 audited `unsafe` items plus a glibc libm FFI; never consume it);
  - `fj-ffi` (raw function-pointer `unsafe`; exclude it).

  The numeric cores (fsym, fnp core, fsci, fj core) are `forbid(unsafe_code)` (FACT).

### 7.5 Toolchains

fsym pins `nightly-2026-08-20`; fnp, fp, fsci and fj pin `nightly-2026-08-31`. fp needs the nightly
`profile-rustflags` cargo feature in its *own* workspace only; consumers are unaffected (HYPOTHESIS).
FrankenMining should pin `nightly-2026-08-31` and confirm that the fsym crates compile on it (OPEN,
since this was not built here).

---

## 8. Capability matrix: FrankenMining need → provider → maturity → decision

| # | Need | Repo / API | Mat. | Decision |
|---|---|---|---|---|
| 1 | Derive two-product and n-product balance formulas | fsym `solve`, `Matrix::solve`/`nullspace`, `verified_simplify` | B | Consume (dev/test-time) |
| 2 | Symbolic Jacobian and sparsity of reconciliation constraints | fsym `Matrix::jacobian`, `SparseJacobian::try_build` | B | Consume (dev/test); runtime gradients from fj or hand-coded |
| 3 | Lagrange/WLS reconciliation solve | fsci-linalg `solve_with_casp`; fsci-opt `trust_constr`/`least_squares` | A (parts) | Build `fm-recon` |
| 4 | Exact rationals | fsym `BigRational` | B | Consume for verification only |
| 5 | Units and dimension algebra | — | — | Build `fm-units` |
| 6 | Canonical royalty, tax and stream formulas | fsym `Expr` plus `TermDag` digest; `Piecewise`/`Max` evaluate | C | Build `fm-fiscal` AST that lowers into fsym for hashing, LaTeX and checks |
| 7 | Lambdify to fast kernels | fsym `CompiledExpr` (interpreter, no `Piecewise`/`Max`), `to_rust_code` | C | Build `fm-kernels` (build-time codegen) |
| 8 | Sensitivities and elasticities (NPV, AISC) | fsym `diff` (closed forms); fj `jvp`/`grad` (programs) | C / B | Consume fj; fsym as oracle |
| 9 | Delta method and series | fsym univariate `taylor`; fj `hessian`/`hvp` | C / B | Build in `fm-uq` on fj |
| 10 | Polynomial systems | fsym `solve_polynomial_system` (lex Gröbner) | B (small) | Consume (dev) |
| 11 | Gamma-bar and change of support | — symbolic; fsym 1-D `certified_integral` for references | — | Build in `fm-geostat` (Gauss–Legendre discretization) |
| 12 | Lane's cut-offs | fsym `solve_linear` (limiting); fsci `brentq` (balancing) | B | Build `fm-cutoff` |
| 13 | LaTeX for reports | fsym `latex` (basic) | C | Consume plus upstream ask; templates in `fm-report` |
| 14 | ndarray substrate | fnp `UFuncArray` (all-f64) | C (for mining) | Build `fm-array` |
| 15 | dtypes i64, bool, masked | fnp `IntegerSidecar`, `MaskedArray` (f64-encoded) | B | Use in interchange only |
| 16 | Reproducible, splittable RNG | fnp `Pcg64DxsmRng`/`PhiloxRng`, `SeedSequence::spawn`, `advance`/`jump_2_128` | A | Consume behind `fm-rng` |
| 17 | Dense linalg (Cholesky, eig, svd, lstsq) | fnp-linalg flat `&[f64]`; fsci-linalg `&[Vec<f64>]` (CASP) | A / B | Consume fsci (audit); fnp as oracle |
| 18 | FFT and einsum | fsci-fft (R3); fnp `UFuncArray::einsum` | A / B | Consume fsci-fft |
| 19 | `.npy`/`.npz` interchange | fnp-io `save`/`load`/`savez`/`load_npz` | A | Consume |
| 20 | Out-of-core 10⁸–10⁹ cells | fnp-io `memmap` reads into a `Vec<u8>` (not mmap) | — | Build `fm-blockstore` |
| 21 | From/to interval joins and compositing | fp `IntervalIndex` (linear scan), point `merge_asof` | C | Build `fm-drillhole` |
| 22 | Telemetry groupby, rolling, resample | fp `rolling*`, `resample`, `ewm`, `merge_asof_with_options` | A− | Consume |
| 23 | Categorical (lithology, domain codes) | fp `CategoricalDtype`/`CategoricalIndex` | B | Consume; check memory at scale (OPEN) |
| 24 | CSV, Excel, Parquet, Arrow I/O | fp `read_csv*`, `read_excel*`, `read_parquet` (whole-file), `read_feather` | A− | Consume (`default-features = false`) |
| 25 | Vendor binaries (Datamine `.dm`, Surpac `.str`) | — | — | Build `fm-io-vendor` |
| 26 | Decimal money | — (no decimal dtype) | — | franken_accounting types (R4) |
| 27 | Sparse SPD solve at 10⁶–10⁸ (SPDE/GMRF) | fsci `cg`/`minres` (no preconditioner), `gmres_preconditioned`, `spilu`; no sparse Cholesky. fs-sparse LDLᵀ/AMG (R3) | B / — | Build `fm-gmrf` on lifted fs-sparse |
| 28 | LP/MILP at scheduling scale | fsci `linprog` (dense revised simplex or Mehrotra IPM; not HiGHS), `milp` (B&B) | — at scale | Build `fm-lp` (PDLP) plus decomposition |
| 29 | QP | fsci SLSQP / `trust_constr` | B | Consume for small; build in `fm-lp` |
| 30 | NLLS | fsci `least_squares` (FD Jacobian by default), `curve_fit` | A | Consume |
| 31 | IRR roots | fsci `brentq`, `toms748`, `newton` | A | Consume plus a wrapper |
| 32 | Lognormal, gamma, beta, Dirichlet, multivariate t | fsci-stats `Lognormal`, `GammaDist`, `BetaDist`, `Dirichlet`, `MultivariateT` | A | Consume |
| 33 | Censored assays (below detection limit) | fsci `CensoredData` (logrank only); `fit_mle` uncensored | C | Build in `fm-geochem` |
| 34 | KDE, Sobol, LHS, Sobol indices | fsci `GaussianKde`, `SobolSampler`, `LatinHypercubeSampler`, `SobolIndicesResult` | A | Consume |
| 35 | KDTree neighbourhood search | fsci `KDTree::query_k`/`query_ball_point(_many)` | A (correct); perf OPEN | Consume; benchmark |
| 36 | RBF implicit modelling | fsci `RbfInterpolator` (max 4,096 points, global) | C | Build `fm-implicit` |
| 37 | Pit-slope dilation | fsci `binary_dilation_with_structure`; `grey_dilation` (cubic only) | B | Use binary; precedence in `fm-pseudoflow` |
| 38 | Leach, flotation and CIL kinetics ODEs | fsci `solve_ivp` (RK45/BDF/Radau/LSODA) | A | Consume |
| 39 | Vibration and condition monitoring | fsci `welch`, `find_peaks`, STFT, `hilbert` | A | Consume |
| 40 | Autodiff, vmap, jit | fj `grad`, `value_and_grad`, `vmap`, `jit` (interpreted), `hvp` | B | Consume |
| 41 | HMC/NUTS | — | — | Build `fm-mcmc` on fj |
| 42 | GPU and WASM numerics | fj GPU is a stub; no WASM target | — | Out of scope for v1 |

---

## 9. Recommended numerics consumption posture

1. **Consume from crates.io with exact pins** (`=0.4.0`, default features off): `fsci-linalg`,
   `fsci-sparse`, `fsci-opt`, `fsci-stats`, `fsci-spatial`, `fsci-interpolate`, `fsci-integrate`,
   `fsci-signal`, `fsci-fft`, `fsci-special`, `fsci-ndimage`. This is the default numerics layer.
2. **Consume by git revision** (`default-features = false`):
   - `frankenpandas` @ `37bb94101`: no `sql-sqlite`, no hdf5. Use crates.io `0.6.0` instead if it is
     published (OPEN).
   - `fnp-random` and `fnp-io` @ `d32c4e04`: never `fnp-python`.
   - `fj-core`, `fj-trace`, `fj-ad`, `fj-api` @ `fa93fb77`: without `fj-ffi` or `fj-backend-gpu`.
3. **Dev-dependency or build-time only**: `fsym-core`, `fsym-calculus`, `fsym-matrices`, `fsym-solvers`,
   `fsym-simplify`, `fsym-printing` @ `9d785d0`, inside a single `fm-derive` crate used by tests,
   golden-formula generation and report LaTeX.
   - Vendor them or use a git dependency on individual crates, because the workspace root currently
     references missing `-pin` paths.
   - Never on a runtime path until an fsym compatibility profile is certified.
4. **Lift** (copy with attribution plus parity tests) from frankensim @ `20463525d` (R3):
   - `fs-sparse::direct` (supernodal LDLᵀ plus AMD) and the AMG preconditioner, into `fm-gmrf`;
   - `fs-la` batched Cholesky, into `fm-geostat`;
   - optionally `fs-rand` Philox keying.

   Lift rather than depend because frankensim is 0.0.1, unpublished, and carries
   `fs-evidence`/`fs-exec` weight (R3 §6).
5. **Single asupersync**: FrankenMining's own pin. All numeric crates have their asupersync features off.
6. **Oracles, never production kernels**:
   - `fsci-sparse::maximum_flow` and fnx `preflow_push` for `fm-pseudoflow`;
   - `fsci linprog`/`milp` for `fm-lp`;
   - fnp-linalg for `fm-geostat` solves;
   - fsym for closed-form formulas.

---

## 10. Upstream asks (proposed beads)

**frankensympy**
1. Make the workspace resolvable without `../franken_numpy-pin` and `../frankenscipy-pin`, for example
   by excluding `fsym-numeric-consumer` or feature-gating it.
2. Tag a release or publish `fsym-{core, calculus, matrices, solvers, simplify, printing}`.
3. `CompiledOp`: add comparisons, `Select`/`Piecewise`, `Min`/`Max`, `floor`, `erf`; make
   `MAX_COMPILE_OPS` configurable.
4. `diff` rules for `Max`/`Min`/`Abs` (to `Piecewise`/`sign`).
5. LaTeX: `Piecewise` to `cases`, `Sum`/`Integral`/`Matrix`, a subscript and symbol-name map.
6. A multivariate Taylor / delta-method helper.
7. Definite integration of piecewise polynomials, which enables exact 1-D spherical-variogram averages.

**franken_numpy**
1. A real memory map (feature-gated audited `unsafe` or `memmap2`), or `read_npy_slice(offset, count)`.
2. Typed storage (f32, u8, bitset) instead of all-f64 in `UFuncArray`.
3. A documented cross-ISA determinism scope for distribution samplers, and a pure-Rust libm option.

**frankenpandas**
1. An interval-tree `IntervalIndex::get_indexer` and an interval-overlap join (`by=` hole ID).
2. A `Decimal128` dtype.
3. A projected or row-group streaming `read_parquet`.
4. Make `sql-sqlite` non-default; scope the `fp-dot-kernel` `+avx2,+fma` rustflags to x86_64.

**frankenscipy**
1. A preconditioner argument for `cg` and `minres`; IC(0) and AMG preconditioners.
2. Sparse Cholesky with log-determinant, or bless fs-sparse as the provider.
3. CSR-input `linprog`, an MPS reader, and a warm-started dual simplex.
4. `fit_mle` over `CensoredData`.
5. `RbfInterpolator` neighbours mode; lift the 4,096-point cap.
6. `grey_dilation` with an arbitrary footprint.
7. `least_squares` with an analytic `jac` and `jac_sparsity`.
8. Flat `(&[f64], rows, cols)` overloads in fsci-linalg and fsci-opt.
9. A published KDTree benchmark at 10⁶–10⁷ points.

**frankenjax**
1. A `wasm32` build without `fj-ffi`.
2. A documented per-primitive interpretation overhead, and a fused or codegen path for scalar-heavy
   Jaxprs.
3. A home for samplers (`fj-mcmc`), or acceptance of an `fm-mcmc` upstreaming later.
4. A tagged release.

---

## 11. Prioritized "must build in fm-*" list (numerics)

| Pri | Crate | Scope | Builds on |
|---|---|---|---|
| P0 | `fm-array` + `fm-numerics-bridge` | Canonical typed grid; zero-copy and copy adapters | fnp-ndarray stride calculus (reference), fsci and fp slices |
| P0 | `fm-rng` | Keyed streams per realization, domain and node block; golden streams | `fnp-random` core (PCG64DXSM, Philox, `SeedSequence::spawn`) |
| P0 | `fm-units` | Typed dimensions (t, g/t, oz, lb, %, $/t) | none |
| P0 | `fm-drillhole` | Desurvey, sweep-line from/to overlap join, length-weighted compositing | frankenpandas I/O, `fm-array` |
| P0 | `fm-geostat` | Experimental and fitted variograms, SK/OK/block kriging, numeric gamma-bar, anisotropic search | fsci `KDTree`, `solve_with_casp`, `least_squares`, `fsci-special` Hermite; lifted fs-la batched Cholesky |
| P0 | `fm-pseudoflow` | HPF on i64 CSR, precedence templates, nested pits | Oracles: fsci `maximum_flow`, fnx `preflow_push` |
| P1 | `fm-recon` | WLS and bilinear reconciliation, gross-error tests | fsci-linalg, fsci-opt `trust_constr`, fsci-stats χ²; fsym-derived Jacobians (tests) |
| P1 | `fm-fiscal` + `fm-kernels` | NSR, NPI, sliding-scale royalty and tax AST; build-time codegen; canonical hash and LaTeX via fsym | fsym `Expr`, `TermDag`, `latex`, `to_rust_code` |
| P1 | `fm-lp` | PDLP first-order LP on CSR; QP; MPS I/O; later decomposition | fsci-sparse CSR; oracle fsci `linprog`/`milp` |
| P1 | `fm-mcmc` | NUTS with adaptation, bijectors, R-hat/ESS diagnostics | fj `trace_jaxpr_for_values`, `value_and_grad`; `fm-rng` |
| P1 | `fm-gmrf` | SPDE precision assembly, sparse LDLᵀ, log-determinant, selected inverse, AMG-PCG, sampling | Lifted fs-sparse direct and AMG; fsci `cg` as oracle |
| P1 | `fm-sgs` | Sequential Gaussian and turning-bands simulation, post-processing | `fm-geostat`, `fm-rng`, fsci `KDTree` |
| P2 | `fm-geochem` | Censored MLE (left-censored lognormal), ROS, CLR/ILR | fsci-stats distributions, fsci-opt `minimize` |
| P2 | `fm-implicit` | Partition-of-unity or local RBF at 10⁵–10⁶ points | fsci `RbfInterpolator` (small-case oracle), fs-rep-sdf |
| P2 | `fm-blockstore` | Chunked out-of-core block model, `.npy`-compatible tiles | fnp-io header codec |
| P2 | `fm-cutoff` | Lane's limiting and balancing cut-offs, DP over life of mine | fsci `brentq`; fsym-derived goldens |
| P2 | `fm-sensitivity` / `fm-uq` | Tornado charts, elasticities, delta method, Sobol indices | fj `jvp`/`hessian`, fsci `SobolSampler`/`SobolIndicesResult` |
| P2 | `fm-libm` | Fixed-algorithm transcendentals for cross-ISA reproducibility | none (port) |

---

## 12. Open questions

1. Do the fsym crates compile on `nightly-2026-08-31` when pulled individually (§2.1, §7.5)?
2. KDTree build and query throughput at 10⁶–10⁷ samples (§5.4)?
3. Which fsci and fnp kernels give thread-count-dependent results (§7.3)?
4. fj interpretation overhead per gradient evaluation for a 50–500-parameter hierarchical model (§6)?
5. Is frankenpandas 0.6.0 published on crates.io, and is categorical column storage code-based (§4)?
6. Can `fj-*` build for `wasm32` with `fj-ffi` excluded (§6)?
7. Is frankensim fs-sparse LDLᵀ suitable to lift at 10⁷ unknowns (memory, ordering quality)? R3 left
   this unbenchmarked.
