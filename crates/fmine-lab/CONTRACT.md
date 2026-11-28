# CONTRACT.md — `fmine-lab`

## 1. Purpose
Testpit `S1` deterministic virtual-time simulation harness, Dynamic Partial-Order Reduction (DPOR) schedule explorer, chaos fault-injection + `CrashPack` replayer, runtime capability-mask auditor (`PO-BED-8`), Bedrock FM protocol oracles (`PO-BED-1`, `2`, `3`, `4`, `10`), and scheduler/diagnostic invariance harness (`INT-AS-001` criterion 8) (`BD-099`, `BD-100`, `BD-101`, `BD-097`, `BD-337`, plan §26.6–§26.7 & §34.2).

- **Crate:** `fmine-lab`
- **Layer:** S1
- **Subsystem:** Testpit
- **LOC budget:** 10,000 planned / 12,000 ceiling
- **Allowed Module Families:** `["runtime", "chaos", "cap_audit", "invariance", "oracles"]`

## 2. Inputs
- Deterministic 64-bit seeds (`0..999` in CI, `0..10_000` nightly), `SchedulerPolicy`, `DiagnosticLevel`, virtual timestamps, and protocol actor scripts.
- `ChaosProfile` fault specifications (link drops, reorder delays, network partition windows, crash boundaries) and serialized `CrashPack` byte buffers.
- `fmine-runtime::region::{REGIONS, Role, profile}` and `fmine-runtime::supervise::AppSpec` for runtime capability-mask and escalation audits.
- Dependencies: `fmine-core`, `fmine-evidence`, `fmine-time` (`S1 -> S0`) and declared same-layer edge `fmine-lab -> fmine-runtime` (`S1 -> S1`, `registries/workspace_topology.toml`). Zero external crates.

## 3. Outputs
- `LabRunReport` with canonical BLAKE3 `trace_digest`, `authoritative_state_digest`, virtual timestamps, and event trace.
- `DporReport` summarizing Mazurkiewicz-inequivalent schedules explored, pruned transitions, distinct final states, and any discovered protocol invariant violations.
- Canonical binary `CrashPack` (`FMNE` + `CRSH` magic + SHA-256 digest) reproducing any failing seed deterministically.
- `CapabilityAuditReport` verifying `PO-BED-8` across all `Role::ALL` profiles and `InvarianceReport` verifying `INT-AS-001` criterion 8 across seeds `0..999` and 3 diagnostic levels.

## 4. Invariants
- **Seed determinism (`BD-099`):** identical `(seed, script, policy)` yields bit-identical `trace_digest` across runs (`test_lab_runtime_seeds_0_to_999_same_seed_same_trace_digest`).
- **Virtual time monotonicity:** `VirtualClock` never moves backward (`LabError::TimeWentBack`) and never reads `SystemTime::now`.
- **DPOR soundness (`BD-099`):** independent transitions on disjoint resources prune without missing conflicting race orders (`test_dpor_prunes_independent_actors_and_catches_seeded_lease_race`).
- **CrashPack reproducibility (`BD-100`):** encoding, decoding, and replaying a `CrashPack` reproduces the exact `failing_step`, `trace_digest`, and `violation_detail` (`test_chaos_profiles_and_crash_pack_reproduce_failing_seed`).
- **Runtime capability mask (`BD-097`, `PO-BED-8`):** K/E compute regions hold zero `TIME`/`IO`/`REMOTE`/`RANDOM` capabilities, no child widens its parent, and `AppSpec::compile` + `CompiledSupervisor` invariants hold for all four roles (`test_po_bed_8_runtime_capability_masks_and_supervisor_profiles`).
- **Scheduler/diagnostic invariance (`BD-337`, `INT-AS-001` criterion 8):** authoritative state digests are byte-identical across seeds `0..999`, all three diagnostic levels (`Quiet`, `Standard`, `Verbose`), and adaptive scheduler policies (`test_int_as_001_criterion_8_authoritative_invariance_1000_seeds`).

## 5. Refusals
- `LabError::TimeWentBack` → `FMINE-REF-107` (monotonic virtual-clock violation).
- `LabError::InvalidConfig` / `LabError::StepBudgetExceeded` → `FMINE-REF-109` (invalid fault basis points, empty actors, or runaway step budget).
- `LabError::CorruptCrashPack` → `FMINE-REF-505` (magic, version, truncation, or SHA-256 mismatch on `CrashPack::decode_bytes`).
- `LabError::CapabilityViolation` → `FMINE-REF-301` (`PO-BED-8` runtime capability widening or compute IO/TIME/REMOTE grant).

## 6. Determinism class
`BitwisePortable` (plan §29.2): integer arithmetic, keyed `PhiloxStream`, canonical BLAKE3/SHA-256 digests, and `BTreeMap`/`BTreeSet` ordering only.

## 7. Evidence class of outputs
Verification/lab evidence only (Testpit); never production operational measurements (`INV-006`).

## 8. Validity domain
- Models protocol schedules, virtual clocks, fault injection, and `fmine-runtime` capability/supervision state machines deterministically.
- `asupersync =0.6.2` is in `Cargo.lock` since `021a58e6`, through `fsqlite` in `fmine-projection` only (`INT-AS-001a = PASS`). This crate does not link it: `INT-AS-001b` (`BD-257`) and `BD-337` must re-run criterion 8 against the live `asupersync` executor.

## 9. Oracles
- Seeded known-answer concurrent lease-fencing race (caught deterministically by `DporExplorer` and by `ChaosProfile` crash-at-boundary campaign).
- `fmine-runtime::region::REGIONS` (itself verified against plan §26.6) for `PO-BED-8`.

## 10. Known gaps
- The `asupersync` native `LabRuntime` adapter is not written. `asupersync =0.6.2` is admitted (`INT-AS-001a` / `BD-325`), so it waits only on `INT-AS-001b` (`BD-257`).
- 1,000-seed partition and crash chaos campaign wiring all FM oracles against live `fmine-sync` replicas belongs to `GATE-SYNC-01` (`BD-109`).

## 11. Public API
- `LabError`
- `runtime::{VirtualClock, DiagnosticLevel, SchedulerPolicy, LabEvent, LabTraceEntry, LabRunReport, LabRuntime, ResourceAccess, ActorTransition, DporViolation, DporReport, DporExplorer}`
- `chaos::{PartitionWindow, CrashBoundary, ChaosProfile, ChaosOutcome, CrashPack, run_chaos_campaign, replay_crash_pack}`
- `cap_audit::{CapabilityAuditReport, audit_all_runtime_capability_profiles}`
- `invariance::{AuthoritativeOp, InvarianceReport, verify_scheduler_and_diagnostic_invariance}`
- `oracles::{ProofObligation, OracleViolation, LeaseEvent, CasTraceEvent, PostingEvent, BridgeIntent, BridgeReplayState, check_po_bed_1_single_writer, check_po_bed_2_prefix_consistency, check_po_bed_3_root_last_cas, check_po_bed_4_exactly_once_posting, check_po_bed_10_bridge_idempotency}` (`BD-101`)

## 12. Proof obligations
- `BD-099` (`Same seed → same trace digest` + DPOR): `runtime::tests::test_lab_runtime_seeds_0_to_999_same_seed_same_trace_digest`, `runtime::tests::test_dpor_prunes_independent_actors_and_catches_seeded_lease_race`.
- `BD-100` (`Failing seed reproduces` via `CrashPack`): `chaos::tests::test_chaos_profiles_and_crash_pack_reproduce_failing_seed`.
- `BD-097` (`PO-BED-8` runtime capability-mask lab test): `cap_audit::tests::test_po_bed_8_runtime_capability_masks_and_supervisor_profiles`.
- `BD-101` (`FM oracles PO-BED-1, 2, 3, 4, 10` firing on seeded bugs): `oracles::tests::test_all_five_fm_oracles_pass_clean_and_fire_on_seeded_bugs`.
- `BD-337` (`INT-AS-001` criterion 8 over seeds `0..999` × 3 diagnostic levels): `invariance::tests::test_int_as_001_criterion_8_authoritative_invariance_1000_seeds`.
