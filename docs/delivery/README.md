# FrankenMining Delivery & Gate Records (`docs/delivery/`)

This directory holds machine- and human-verifiable admission records (`INT-*`), milestone gate records (`GATE-*`), and budget amendments (`BUDGET-AMEND-*`). Per `§0.7` and `§28.4`, delivery logs live here rather than mutating the architecture specification.

## Admission records (none admitted)
- [`INT-AS-001.md`](./INT-AS-001.md) — `asupersync =0.5.0` and `franken-evidence` / `franken-decision` adapter boundary (`BD-257`) (not admitted).
- [`INT-NUM-001.md`](./INT-NUM-001.md) — Numerics foundation stack (`fsci =0.4.0`, `fp`, `fnp`, `fj`) closure & cross-ISA determinism scope (`BD-308`) (not admitted).

## Gate records
- FACT. The layering checks passed locally at `2f87dfd` and on both operating systems in dev and release at `eddeb7d`. [GATE-LAYERS-00](./GATE-LAYERS-00.md)
- FACT. The 18-family determinism record still awaits the complete cross-ISA CI matrix. [GATE-DET-01](./GATE-DET-01.md)
- FACT. No oracle crate is in the production closure (191 packages), and all 11 fixtures have capture records, measured locally at `e9400927`; check-all re-measures it in every CI job. [GATE-ORACLE-01](./GATE-ORACLE-01.md)
- [`GATE-BM-01.md`](./GATE-BM-01.md) — lifted `VdbGrid` at 10⁸ active cells (`BD-191`): PASS measured locally at `ef49fcf` (ratio 1.352 ≤ 2.5); self-verified; the test is ignored in CI.
- [`GATE-PUSH-PIT1.md`](./GATE-PUSH-PIT1.md) — MineLib UPIT exact with verified closure (`BD-250`): PASS measured locally at `ef49fcf` on all 11 instances; self-verified; MineLib's files are not committed, so CI cannot run it.

## Kill-gate records
- [`KILL-PUSH-PDLP.md`](./KILL-PUSH-PDLP.md) — PDLP to a 1e-4 certified gap on real-size non-closure LPs (`BD-251`). PASS, so the kill does not fire, measured locally at `9b9fe28`:
  - all 10 MineLib CPIT LP relaxations, up to 9.5×10⁷ nonzeros, at ε = 1e-6 within 30 minutes;
  - at the default 1e-4, 5 of 10 miss;
  - self-verified, and CI cannot run it.

## Decisions
- [`DECISION-BD-012-direct-push-blocking.md`](./DECISION-BD-012-direct-push-blocking.md) — what "CI blocks merges" (§28.4.4) means when every change is pushed straight to `main` (`BD-012`).
- [`DECISION-bd-lf4q-confidentiality-product-lattice.md`](./DECISION-bd-lf4q-confidentiality-product-lattice.md) — the owner's acceptance of confidentiality labels as a product of an internal flag, venture set and wall set, so that ventures and walls stay incomparable (§31.1, `bd-lf4q`).

## Budget amendments

FACT. Five approvals dated November 20 are recorded for the
[public](./BUDGET-AMEND-001-fmine-public.md), [CLI](./BUDGET-AMEND-002-fmine-cli.md),
[metrics](./BUDGET-AMEND-003-fmine-metrics.md), [calibration](./BUDGET-AMEND-004-fmine-calib.md) and
[capital](./BUDGET-AMEND-005-fmine-capital.md) crates.

## Project reality check, November 22, 2026

**INFERENCE.** A reviewer can reproduce Barrick workbook values, trace their source cells, and challenge
inconsistent identities today. The open M0 deliverable is an independently reviewable assurance report and an
externally time-proved FY2026 forecast record. M1–M6 have substantial individual components and no recorded
milestone acceptance. Use this snapshot to choose work for the M0 gate. Supersede it when the gate record lands.

**FACT.** The audited local source was `6b53adbcc27d8a7e4db6447fdbddc8c9e0dadbf7`. Public
`main` and `master` both pointed to `f87e84d55057192c5c0c8a94aa426abbc2923496`, with 48 local-only
and 22 public-only commits. GitHub listed no Releases. The corpus runs below used an existing arm64 macOS
executable with SHA-256 `4892d5b77ef1d423c92e72d59a8795d660fa0fd402d4e741602ddf1653a3e7a3`.
Its exact build revision was unverified. The October blind artifact has a separately verified identity.
The local workspace contains 35 members and 35 crate contracts. The README's 22,732 Mine Stats Claims describe
the Q2 epoch. Its default example now reads Q3. Plan §0.2 defines seven conditions for an implemented
capability, including the crate budget and protocol tests. The README's shorter checklist omits those details.

### Public workbook parity

| FACT measured on the registered sources | Result |
|---|---|
| File identity | All 20 files matched their registered lengths and SHA-256 hashes. The two Q3 files were retrieved from the registered issuer URLs. |
| Workbook extraction | All 20 per-file numeric digests and all six census fields matched the independent `fidelity.py` reader. It counted 112 visible sheets, 6 hidden sheets, 33,722 visible numeric cells, 4,411 placeholders, 1,092 formula errors and 7,197 text cells. |
| Semantic Mine Stats inventory | The separate openpyxl reader and CLI emitted the same 25,348 unique file/sheet/cell locations and reporting periods. All 20,932 source numeric tokens matched the original OOXML value text exactly, including seven zeros the Claim layer classified as disposed. All text placeholders and blanks matched. This population spans 11 Mine Stats workbooks. |
| Source citations | All 36,336 emitted source-cell Claims had a cell locator and the matching registered workbook hash. Tracing Carlin's Q1 2024 open-pit cost returned `Minestats1!B10`, the exact source hash, attributable share 123/200 and the printed 3.87 USD/t. Coverage of derived numbers in every verb remains a separate test. |
| AISC identities | 2,915 instances. States i–v counted 2,737 / 3 / 4 / 101 / 0, plus 70 not published. The ledger retained 108 exceptions, with zero unregistered headers or table refusals. |
| AISC derivation citations | All 29,954 operand references carried the registered file hash. All 10,411 distinct cited cells existed in the actual worksheet XML across 50 sheets. All 7,397 derived fields named a rule and version, and all 52,311 input references resolved to an operand of the same instance. |
| Mine Stats consistency | The report retained 52 discrepancies and 33 unresolved entries. Its cross-vintage projection counted 11 silently restated, 60 conflicting and 222 non-comparable states. These describe the registered evidence and its rules. |
| Reference cut-offs | Seven reference-input cases produced six reproductions and one agreement within input rounding. The local report set `source_verified=false` and `gate_eligible=false` for this section. The two-reader PDF inputs were absent here. |
| Determinism | Two full default Q3 outputs were byte-identical, SHA-256 `445c0b17c20bdd931eb1a562aa2e8b8e89c673d2f1eef61c1e752ea182e24018`. The Q3 certificate matched `a969738f…8ddf025a`. The same binary through Q2 matched `0effa0b4…d0e2f264`. |

FACT. The report's top-level `gate_eligible=true` records that the source lane is public. Individual
sections describe their own evidence, and milestone acceptance requires the delivery record.
[Eligibility predicate](../../crates/fmine-public/src/lib.rs)

FACT. `fmine assure challenges --issuer barrick --through 2026Q3` exported 601 distinct packets. Every
packet carried the Q3 root, cited operands and named a closing document. All 601 had an empty human check and
`external_use_allowed=false`. A reviewer can therefore start with a specific disagreement and the evidence
needed to resolve it. There is no recorded reviewer verdict in this packet set.

FACT. Reproduce with `fmine assure public --issuer barrick` and the same command with `--through 2026Q2`.
Run `python3 -I scripts/oracle/fidelity.py corpus_of_input_docs/downloaded/mine_stats/*.xlsx` for the
independent census. The independent semantic reader accepts a fresh output path followed by all Mine Stats files.
[Reader and provenance](../../scripts/oracle/mine_stats_extract3.py) · [Epoch pins](../../repro/barrick/expected.toml)

**FACT. Source integrity and arithmetic dependence have different tests.** Changing one byte in a scratch
copy of the Q3 AISC workbook preserved its length. The CLI reported the exact file and both hashes,
returned `FMINE-REF-505` (exit 8), and withheld the payload and certificate. The earlier Q2 dependency
exercise recorded 54/54 file checks, 56/56 label checks and 2,492 passing sampled leaves, with 103 skips.
The Q3 check above exercised one file. Coverage of every scored value is still open in `bd-r7lp`.
[Public crate contract](../../crates/fmine-public/CONTRACT.md)

**FACT. The same Q3 command under the macOS sandbox with network access denied produced the identical
full-output digest.** `xtask sbom --binary` generated a CycloneDX 1.5 bill with 23 components from the
checkout and recorded the exact Mac binary hash above. Independent security-pack review and a signed
release are still open. [M0 threat model](../security/THREAT-MODEL-M0.md)

### Two tests that change the M0 decision

1. **FACT. The pre-release importer refused the admitted Q3 registry.** GitHub artifact 13051824556 had exactly
   the recorded SHA-256 `58818c06c39df09de19eeba93d2a4b4b9c0fb72e24bb88afbc5fd5bf1b36bd82`.
   With the registry from its own commit `6a46b96`, it reproduced the 18-file control certificate
   `d37903cf…e66ef15c`. With the byte-identical Q3 admission registry from `8c3bbf8`, it returned exit 5,
   `FMINE-REF-506`, unknown table `[[subtotal_member]]` at line 451, and a null certificate. This November 22
   replay produced no blind-file extraction percentage. In a diagnostic control, appending only the two
   verbatim Q3 source rows to the frozen registry also returned exit 5. The old parser refused the new
   per-source `retrieved_utc` field. Both the table vocabulary and the source-row schema matter.
   Record the failed invocation, retain E859/E860 as
   known-file results, and adjudicate the blind clause and kill rule on that evidence. The prospective Q4
   rehearsal has its own task, `bd-s9j3.4`. [Frozen binary identity](../../repro/barrick/blind_record.json)
2. **FACT. A forecast candidate exists and the December deadline still matters.** The documented
   `forecast record build` invocation at `2026-11-10T23:59:59Z` returned 22 uncalibrated rows for 11 live
   sites and two targets, with Hemlo and Tongon excluded by name. The receipt had `issued=false`, no attestation
   and no timestamp proof. Both RFC 3161 **rules** receipts verified with `bash scripts/verify_rules_timestamp.sh`.
   The committed forecast directory contains the rules and their proofs. The actual forecast values, final
   manifest and record proof are outstanding. The pooled ratio model uses the persistence baseline's interval
   endpoints, so its median alone cannot improve the interval score. The rules require post-Q3 timing to remain
   visible, and set the proof deadline at December 31, 23:59:59 UTC. [Frozen rules](../../forecasts/fy2026/rules.toml)

**FACT. Public CI has a reproducible vintage-selection problem.** Its Ubuntu real-files job verified all
20 downloads. The historical corpus test target then reported 11 passed, 3 failed and 1 filtered. The Rust
population was bounded to Q2 while the oracle calls enumerated the directory containing Q3. Mine Stats compared
9,102 with 10,228, the COS reader compared 519 with 583, and the E823 test compared 2,915 with 2,595.
The scheduled cross-host root check also names Q2 explicitly. Preserve its control and compare the current
epoch separately. On the same public revision, the forecast-and-production job passed, rustfmt failed and all
four layer-check jobs failed. The Ubuntu dev log reports 18 unresolved test references and two unrecorded gate
claims. [Real files](https://github.com/Dicklesworthstone/franken_mining/actions/runs/45037117855) ·
[CI](https://github.com/Dicklesworthstone/franken_mining/actions/runs/45037120367)

### Vision checklist and route to acceptance

The evidence column describes observed code or records at the named dates. The final column names the next
acceptance test and its owner in the tracker. The [milestone ladder](../../COMPREHENSIVE_PLAN_FOR_THE_DESIGN_OF_FRANKENMINING.md#373-the-ladder-about-36-months-from-november-2026)
sets the full criteria.

| # | Deliverable | FACT about present evidence | TARGET and tracker |
|---|---|---|---|
| 1 | M0 Portal and Bullion | Known-file workbook parity, five-state ledger, cross-vintage comparisons and cited challenges run. The two-reader PDF and recovery sections were absent locally. | Same-epoch independent readers, live-leaf proof, filed-page human check, zero uncited output and clean reproducible run. `bd-s4oy.3`, `bd-r7lp`, `bd-i7d6.3`, `bd-sjrh`, `bd-272`, `bd-504e`, `bd-074`. |
| 2 | M0–M1 Plumb forecasts | Real-source draft, rules proof, forecast journal, cohort scoring and correction code. A completed issuer-year score awaits original outcomes. | Issue and time-prove the record, then score the original and both baselines with missing outcomes visible. `bd-067`, `bd-0545.3`, `bd-068`, `bd-076`. |
| 3 | Bedrock and Adit | CLI, Claims, authority ledger, obligations, lab replay, sync primitives and an offline verifier exist. Cargo.lock has no asupersync package. The daemon, MCP, TUI and projection crates are absent. | Admit the runtime, exercise cancellation/crash/partition and serve the same certified read from the intended client. `bd-257`, `bd-109`, `bd-119`, `bd-121`, `bd-244`, `bd-245`. |
| 4 | M1 Testpit and Touchstone | Synthetic worlds, balances, F-factors and reconciliation kernels exist. The 18-family determinism record is pending. | Recover seeded biases with coverage and false-alarm controls, report all replicates, then reproduce the ISA/profile matrix. `bd-157`, `bd-141`, `bd-dm2z`. |
| 5 | M2 Corebox, Crucible and site assurance | Plant mass balance, RTD inversion, grinding and production commands run on supplied worksheets. Drill/QC authority and a site reconciliation history need their own proof. | Signed access, authorized exports, known-answer plant gate, a shadow card and subsequent monthly forecasts. Use the public M2′ alternative on the plan's data trigger. `bd-163`, `bd-184`, `bd-200`, `bd-204`, `bd-211`, `bd-263`. |
| 6 | M3 Bullion and Hallmark | Cost and reserve arithmetic, source lint and JV comparisons exist. FA conformance, posting detail, and QP acceptance remain open. | Replay the pinned accounting subprocess and produce an index that a pilot QP accepts. `bd-213`, `bd-221`, `bd-224`, `bd-xzvb`. |
| 7 | M3–M4 Helm | DecisionCards and capital bundles compute conditional results on declared scenarios, including downside, information value and entity funding. | Fit the historical reference class, hindcast at least five approvals at their original information cuts, and retain actual decision use. `bd-225`, `bd-229`, `bd-230`, `bd-2jow`, `bd-212`. |
| 8 | M4 Lodestar and Strata | Exploration effort/absence types, geometry, lifted grids and OMF are present. District inference, calibrated hindcast and subsurface belief updates remain partial. | Three-part parity, a time-cut district hindcast with positive counts and intervals, and incremental block updates at the stated scale. `bd-233`, `bd-235`, `bd-237`, `bd-240`, `bd-315`. |
| 9 | M5 Freeboard, Highwall and Muster | The workspace has no corresponding operational crates. Synthetic and planning evidence describes future site use. | Shadow instrument triggers with staleness, prospective pond and bench outcomes, privacy approval and site-confirmed competency coverage. `bd-247`, `bd-248`, `bd-249`, `bd-0pnt`, `bd-g8t5`. |
| 10 | M6 Pushback and Headframe | MineLib ultimate-pit parity is recorded on 11/11 instances. Scheduling, telemetry primitives and a viewer exist. A site-calibrated stochastic plan and live twin remain open. | A three-gap plan certificate, actual shift calibration, fleet join coverage and an external consumer scoring predictions. `bd-253`, `bd-255`, `bd-256`, `bd-sw2g`. |

**FACT. Performance evidence has named workloads.** In addition to MineLib, the recorded 10-million-interval
compositing benchmark took 156.28–213.16 ms across three runs on hz3. The PDLP record reports 10/10 within
the certified gap at solver epsilon 1e-6 and 5/10 misses at the default 1e-4, with the rule change disclosed.
Cross-ISA proof, sustained site ingest and end-to-end site latency have separate gates.
[Compositing benchmark](./bench-bd-310-fmt-col-1.md) · [PDLP record](./KILL-PUSH-PDLP.md)

**FACT. README smoke tests.** Of 182 documented shell invocations identified, a read-only subset of 81
distinct CLI examples used checked-in inputs or built-in synthetic data. Using the existing debug binary,
all 81 exited zero. The 40 JSON envelopes reported success and four false authority flags. The other 41
produced the requested nonempty Markdown. The sample included 35 decision, 18 plant, seven metrics, six
forecast, five JV mirror, four scheduling, three guidance track-record, two recovery-gap and one table-check
invocations. A missing plant worksheet produced exit 5 and
`FMINE-REF-506`. These checks establish CLI reachability. Numerical parity and site performance have their
own denominators above.

**FACT. The GB-30 capital example has an independently checkable result.** Re-enumerating all eight
subsets from the worksheet's five equally weighted scenarios reproduced spend, budget status, and the mean
and worst-fifth loss for every affordable bundle. A+E had the largest mean at USD 372M. Applying the
USD 150M loss limit selected A+D at USD 241.18M mean and USD 130.6M tail loss. This is a known answer on
invented payoffs. The real-approval hindcasts and sponsor decisions in row 7 remain outstanding.
[Scenario inputs](../../tests/capital/gb30.toml)

**FACT. Checkout verification.** `cargo fmt --check` exited zero. The freshly linked Linux `xtask check-all`
exited zero at the audited revision, including 522 Beads, 204 distinct closes, nine oracle fixtures and
zero oracle packages in the product closure. Its source archive had no Git metadata, so the amendment
check could not measure the pushed range. The corpus checks above used the separately identified Mac
executable. No Rust files changed in the audit and Clippy was not rerun. To repeat the Linux checks:

```sh
revision=6b53adbcc27d8a7e4db6447fdbddc8c9e0dadbf7
target_dir="${RCH_TARGET_BASE:-${TMPDIR:-/tmp}}/rch_target_fmine"
rch exec --require-remote --clean-overlay --base "$revision" --no-overlay -- \
  env CARGO_TARGET_DIR="$target_dir" cargo test --workspace --locked
rch exec --require-remote --clean-overlay --base "$revision" --no-overlay -- \
  env CARGO_TARGET_DIR="$target_dir" cargo run --locked -p xtask -- check-all
```

FACT. The full workspace invocation on `trj` ended with exit 137 during compilation when the RCH
free-space monitor timed out. The `-j2` retry on `vmi1153651` ended with exit 120 at disk admission.
Neither produced a workspace test verdict. The separately scheduled Cargo wrapper for `xtask` also
failed at source handoff (recovered exit 1). The check-all result above came from running the newly linked
binary directly on the exact source while it was still available.

FACT. Seven library test executables from that Linux build completed individually. Passing tests by crate
were public 62, metrics 247, calibration 249, capital 160, plant 192, claim 60 and decide 112. Across the
seven, **1,082 passed, zero failed and two were ignored**. Two public tests initially failed after RCH
removed the source archive. Both passed with the pinned Barrick registry and expected roots restored as
read-only fixtures, and the full public library rerun passed 62 with two ignored. These results cover seven
libraries. The remaining workspace targets and doc tests have no complete current-run result.

**INFERENCE. Completing the original queue would still leave holes.** All 357 numbered §39 rows map to
tracker entries. The initial 522 Beads were 268 open, 50 in progress and 204 closed. Checking actual acceptance
text exposed missing ownership for per-kernel thread-count proof, the capital prior, pond-level coverage,
roster confirmation and pilot fleet/vendor scoring. The Q4 fallback was mentioned but lacked its own executable
work package. Six new tasks and six refined tasks brought the inventory to 528 (274 open, 50 in progress,
204 closed), with zero dependency cycles. Existing tasks carry the observed Q3 refusal, historical CI
denominators, outstanding forecast, and the QP, shadow TARP and capital checks. The review leaves every
pre-existing status intact. A signed site agreement, elapsed outcomes and the sponsor's meeting minute or
written equivalent still require external evidence. The month-12 decision controls M3 spending.

**DESIGN. Work order.** Complete the FY2026 record by December 31 and before any M0 content reaches an
issuer insider, in parallel with the Q2/Q3 CI repair. The forecast task currently depends on disposition
of the Q3 blind run (`bd-s9j3.3`). Record the Q3 result promptly so the February Q4 contingency cannot
consume the December issuance window. Have the independent ex-major finance reviewer assess
the historical reproduction and cited discrepancies. Any issuer review follows the plan's disclosure sequence
and counsel clearance. Score the time-proved forecast against the original release in February 2027. In the
next blind run, rehearse the exact artifact against the registry it must
accept before release. Admit the runtime and run M1 synthetic gates on their own evidence. Activate M2 onward
when the relevant data, review and adoption gates are met.
