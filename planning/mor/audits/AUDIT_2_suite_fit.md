# Suite fit (claims about sibling repos checked against their code at pinned revisions)

> Fresh-context, read-only audit of the plan draft, 2026-10-28. Model output, preserved verbatim as the
> provenance for plan Appendix H. Line numbers refer to the draft at audit time, not the current plan.
> Dispositions of every finding are in Appendix H.

I checked 30 claims against the code. 22 are confirmed, 5 are wrong and 3 are stale. Beyond the individual claims, three things in Part IV don't fit how the siblings actually work:

  - **The chrono/time ban can never pass:** fsqlite, fastmcp, frankenpandas and asupersync's TLS feature all pull these crates in unconditionally, so `check-deps` fails on every build.
  - **Two surface pins bring a second asupersync version into the build:** fastmcp `=0.5.0` brings asupersync 0.4.5, and ftui `0.7.x` brings 0.4.9.
  - **The franken-evidence confinement breaks the layer rules:** S0 forbids foundation crates and serde, but `fmine-evidence` (S0) depends on franken-evidence, which is an asupersync-workspace crate built on serde/serde_json.

  Everything was read-only: no builds, no edits.

  ## (1) Claims checked

  | # | Claim | Plan line | Verdict | Evidence |
  |---|---|---|---|---|
  | 1 | asupersync HEAD is 0.6.0; the suite pins 0.5.0 | 4760, 5145 | CONFIRMED | `asupersync/Cargo.toml:24`; tag v0.5.0 = `78b64636e` |
  | 2 | "fgdb rev `78b64636`" | 5145 | WRONG | No such commit in fgdb. `78b64636` is asupersync's v0.5.0 tag, which fgdb pins (`fgdb-codec/Cargo.toml:12`) and FA pins (`franken_accounting/Cargo.toml:64`) |
  | 3 | `open_child_region`, `MacaroonKeyRing`, `CapSet<SPAWN,TIME,RANDOM,IO,REMOTE>`, `LabRuntime`, `CompiledSupervisor`, `AppSpec`, `Budget{poll_quota,cost_quota}` exist at 0.5.0 | 4762–4829 | CONFIRMED | v0.5.0: `src/cx/cx.rs:4086`, `cx/macaroon.rs:564`, `cx/cap.rs:85-102`, `lab/runtime.rs:2275`, `supervision.rs:1103`, `app.rs:1681`, `types/budget.rs:173` |
  | 4 | S3(a): `Cx<None>` → `Cx<All>` via child region | 4786, 5724 | CONFIRMED (type level) | CHANGELOG v0.6.0 "child region keeps its opener's capability set". At 0.5.0 the runtime mask is still checked (`cx.rs:4093`) |
  | 5 | S3(b): all-zero signature accepted | 5725 | CONFIRMED | `CHANGELOG.md` v0.6.0 Security |
  | 6 | S3(c) is "fixed only in 0.6.0" | 5719, 5726 | WRONG | `CHANGELOG.md:141`: "unchanged in 0.6.0". 0.6.0 only adds `add_protected_third_party_caveat` |
  | 7 | S3(d): plaintext ATP refused off loopback; S3(e): durable journaling is new in 0.6.0 | 5727–5728 | CONFIRMED | `CHANGELOG.md` "Breaking CLI change" and "Durable ATP resume and journaling" |
  | 8 | OPEN-SUITE-1: do franken-evidence/-decision/-kernel exist at 0.5.0? | 5177 | Resolvable: YES | v0.5.0 `franken_evidence/src/lib.rs:62` (`EvidenceLedger`), `franken_decision/src/lib.rs:247/679/731` (`LossMatrix`, `DecisionContract`, `DecisionAuditEntry`), `render.rs:106/568/730` |
  | 9 | `VdbGrid`: 310 lines, std-only, `Leaf{[u64;8],[T;512]}` | 1612–1614, 5021 | CONFIRMED | `fs-rep-sdf/src/vdb.rs:11-23`; `CONTRACT.md:203` |
  | 10 | `winding_exact` is 214 lines and liftable, needing only the `atan2` swap | 1627 | Partly wrong | `winding.rs:41` uses std `atan2`, but `:9` also imports `fs_geom::{Point3,Vec3}` |
  | 11 | `volume_certified` needs an exact-distance chart and caps at 256³ | 1619–1622 | CONFIRMED | `fs-geocon/src/primitives.rs:789-793`, `:398` |
  | 12 | fs-qty needs nightly (`generic_const_exprs`); frankensim is on `nightly-2026-07-06`; the suite is on `-08-31` | 1625, 5002 | CONFIRMED | `fs-qty/src/lib.rs:20`; toolchain files |
  | 13 | frankensim uses a path dependency on asupersync, and some crates point at `../../../asupersync` | 5000–5004 | CONFIRMED | `frankensim/Cargo.toml:194`; `fs-topo/Cargo.toml:21`. Its lockfile already holds asupersync 0.4.11 (via fsqlite) and 0.5.0 |
  | 14 | `fs_exec::Cx` is a separate type | 1631, 5152 | CONFIRMED | `fs-exec/src/cx.rs:579`; `fs-geom/src/diff.rs:15` |
  | 15 | FA limits 128 / 32 / 8192 and detail schema `fa:external-subledger-detail:v1` | 3519, 4753 | CONFIRMED | `fa-journal/src/external_subledger.rs:38-41` |
  | 16 | `fad` takes the writer lease; a second daemon gets FA-REF-018, exit 6 | 3486, 4910 | CONFIRMED | `fad/src/main.rs:4`; `fa-policy/src/denial.rs:39` |
  | 17 | FA exit codes are 0 and 2–8 | 3487 | CONFIRMED | `registries/exit_codes.toml`; `verbs.toml:178` |
  | 18 | FA `units.toml` has no metals | 3475, 4528 | CONFIRMED | 8 currencies, BTC/ETH, SHARE/HOUR/EACH. A `commodity` kind exists, so the upstream ask is feasible |
  | 19 | FA is about 793K LOC, rev `53c6e953` | 3473, 5063 | STALE | About 796K now; HEAD `41363acb` is 5 commits on |
  | 20 | fgdb: an index on a `Point` property fails closed | 4640, 5137 | CONFIRMED (overstated) | `docs/THREAT_AND_TRUST_MODEL.md:289`. It is a 1.0 scope rejection with a recorded post-1.0 Z-order route, not a constitutional rule |
  | 21 | fsqlite v0.4.12; time travel only on `:memory:`; R-tree and geopoly exist | 4641, 5132 | CONFIRMED | tag v0.4.12; `fsqlite-core/src/connection.rs:14479`; `fsqlite-ext-rtree/src/geopoly.rs` |
  | 22 | frankensearch facade is 0.7.1 and keeps fastembed off by default | 5134 | CONFIRMED | `frankensearch/frankensearch/Cargo.toml:3`, `default=["hash"]` |
  | 23 | fnx "`0.3.1`, at fgdb-prism's revision" | 5135 | WRONG | `fgdb-prism/Cargo.toml:16-18` pins fnx `9d710b1c`, where fnx-classes is 0.2.0. 0.3.1 is fnx HEAD |
  | 24 | fastmcp `=0.5.0` (`nightly-2026-08-25`) | 5147 | WRONG | fastmcp HEAD is 0.13.0 (`Cargo.toml:18`). Its v0.5.0 tag pins asupersync `=0.4.5` and `nightly-2026-07-11`. Versions from v0.11.0 on pin asupersync `=0.5.0` |
  | 25 | ftui `0.7.x`, asupersync lane | 5148 | STALE | v0.7.0 `ftui-runtime` pins asupersync 0.4.9. v0.8.0 and v0.9.0 (HEAD) pin `=0.5.0` |
  | 26 | frankenmermaid uses the `fm-*` prefix and HiGHS is optional; `fmd` is franken_markdown's binary; `/usr/bin/fm`; `fmn` | 5150, 5195–5199 | CONFIRMED | `fm-layout/Cargo.toml:14-18`; `franken_markdown/Cargo.toml:72-74`; `/usr/bin/fm` |
  | 27 | fsci: `linprog` is not HiGHS; RBF capped at 4,096; no censored MLE | 5076–5078 | CONFIRMED | `fsci-opt/src/linprog.rs:6-20`; `fsci-interpolate/src/lib.rs:6227`; `fsci-stats/src/censored.rs` (data type only) |
  | 28 | fsci has "no preconditioned CG/MINRES" | 5075 | WRONG | `fsci-sparse/src/linalg.rs:9111` (ILU-preconditioned `pcg`), `:11204` (`minres`), plus `gcrotmk` with a preconditioner. All present at v0.4.0 |
  | 29 | fnp `memmap` is not an mmap; `UFuncArray` stores f64; fp `fp-dot-kernel` is built with `+avx2,+fma` | 5072, 5413 | CONFIRMED (nuance) | `fnp-io/src/lib.rs:5707-5713`; `fnp-ufunc/src/lib.rs:2936-2940` (has an i64/u64 sidecar); `fp-dot-kernel/Cargo.toml:3` |
  | 30 | fj has `grad`/`vmap`/`hvp` and an interpreted "jit"; fsym has `Expr`, `TermDag`, `latex`, `to_rust_code` and is on nightly-2026-08-20 | 5079, 5109 | CONFIRMED | `fj-api/src/transforms.rs:329-361,2764`; `fsym-printing/src/lib.rs:254,453`; `fsym-core/src/dag.rs:868` |

  ## (2) Real problems, most severe first

  1. **The L-5 ban on `chrono` and `time` can never pass** (5351–5359; §31.5 checks the full dependency tree). These foundations pull them in unconditionally:
     - fsqlite: `fsqlite-func/Cargo.toml:23-24` (chrono on native), reached through `fsqlite/Cargo.toml:67`.
     - fastmcp: `fastmcp-server/Cargo.toml:70` (chrono).
     - frankenpandas: `fp-types/Cargo.toml:15-16` (chrono, chrono-tz), so every fp crate the plan lists.
     - asupersync: QUIC/TLS needs the `tls` feature (v0.5.0 `src/net/quic_native/mod.rs:13`), which pulls `x509-parser` and then `time`.

     **Fix:** apply the ban to `fmine-*` direct dependencies and public APIs, and keep a per-foundation allow-list for transitive entries in `deny.toml`. Alternatively, drop fp to Oracle status.

  2. **The fastmcp `=0.5.0` pin breaks the one-asupersync rule.** That release links asupersync 0.4.5, a second package next to 0.5.0. GATE-PIN-06 and L-5 would fail. The plan appears to have copied fastmcp's asupersync pin as fastmcp's own version. **Fix:** pin `fastmcp =0.13.0` (anything from 0.11.0 on), which matches the cited `nightly-2026-08-25`.

  3. **The ftui `0.7.x` pin has the same problem:** it brings asupersync 0.4.9. **Fix:** pin ftui 0.9.0 (or at least 0.8.0).

  4. **The fnx pin contradicts itself and hides a gap.** At fgdb-prism's revision, fnx-classes is 0.2.0 and `preflow_push` doesn't exist; it appears only at HEAD (`fnx-algorithms/src/preflow_push.rs:503`). Under the one-package-id rule for fnx-classes, FrankenMining gets either 0.2.0 without the `preflow_push` oracle, or two fnx-classes packages. **Fix:** pin fnx at `9d710b1c` while fgdb is in the graph. Either run the `preflow_push` oracle in a separate dev workspace, or ask fgdb to bump.

  5. **"frankenjax without `fj-ffi`" is unsatisfiable.** `fj-api/Cargo.toml:16`, `fj-dispatch:12` and `fj-interpreters:8` all depend on `fj-ffi` unconditionally. **Fix:** consume only `fj-core`, `fj-lax` and `fj-ad` (`fj-ad` exposes `compile_value_and_grad_jaxpr_for_repeated_eval` and `vjp`). Or accept `fj-ffi`, which is pure Rust with its unsafe code confined, and record that.

  6. **The franken-evidence and franken-decision confinement breaks the layering rules.**
     - §27.5 (line 5146) confines them to `fmine-evidence`, which §28.3 (line 5233) places in S0. S0 is "std plus lifted, no foundation" (5209, 5342).
     - franken-evidence depends on serde and serde_json, but serde is allowed only in FMT-ENV-1 (4755, 5517).
     - §30.5 (line 5598) renders through `franken_evidence::render` in `fmine-report`, which is outside the confinement.
     - CELL-DEC (line 4666) types durable rows with franken-decision structs. Those version with asupersync, which contradicts "formats never depend on a runtime version" (4740).

     **Fix:** S0 owns schema-compatible types. Conversion and rendering go through a single E- or V-layer adapter, and serde is allowed only in that adapter.

  7. **Three open questions can be closed now.**
     - OPEN-SUITE-1: yes, all four APIs exist at v0.5.0.
     - OPEN-SUITE-2: no. fnp uses Philox4x64-10 (`fnp-random/src/lib.rs:1261`) and frankensim uses Philox4x32-10 (`fs-rand/src/philox.rs:1`), so the cross-check has to use golden vectors.
     - OPEN-SUITE-4: yes, and it's worse than asked. `fp-io` depends on arrow (`Cargo.toml:15`), parquet (`:28`) and chrono (`:19`) unconditionally, and `read_parquet` reads the whole file and decodes it as one Arrow batch (`lib.rs:12402-12404`, `:12276-12286`).

     **Fix:** record these answers and take the KILL-6 path for `fp-io` now.

  8. **The pin label is wrong.** **Fix:** change "fgdb rev `78b64636`" to "asupersync rev `78b64636e` (tag v0.5.0, as fgdb and FA pin it)". Also name an fgdb revision; FA uses `b323bd17`.

  9. **S3(c) is described as fixed in 0.6.0, but it isn't.** **Fix:** say it is unchanged in 0.6.0, which only adds a new protected API. GATE-PIN-06 must keep the first-party-only rule after the bump. S3(a) should be described as a type-level escalation; at 0.5.0 the runtime mask still refuses SPAWN, so the lab mask test is the real control.

  10. **The fsci gap list overstates one gap.** **Fix:** change "no preconditioned CG/MINRES" to "CG is ILU-preconditioned only (`pcg`), with no AMG or generic preconditioner hook; MINRES is unpreconditioned". The design doesn't change.

  11. **Minor corrections:**
      - The winding lift needs `Point3`/`Vec3` substitutes; it isn't only an `atan2` swap.
      - fnp `UFuncArray` keeps exact integers in a sidecar, so it doesn't store every dtype purely as f64.
      - The fgdb Point-index ban applies to 1.0, not constitutionally.
      - Refresh the FA HEAD to `41363acb`.
