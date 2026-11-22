//! Determinism: BitwisePortable
//! Deterministic virtual-time `LabRuntime`, seeded schedule replay (`0..999`), and Dynamic
//! Partial-Order Reduction (DPOR) explorer (`BD-099`, plan §26.7 & §39 WP-05).

use std::collections::BTreeSet;

use fmine_core::Digest32;
use fmine_core::hash::blake3_hash;
use fmine_core::rand::{PhiloxStream, RngKey};
use fmine_runtime::{ClockError, WallClock};

use crate::LabError;

/// Deterministic monotonic virtual clock implementing `fmine_runtime::WallClock` without reading
/// any host clock (`BD-099`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VirtualClock {
    now_ns: i64,
}

impl VirtualClock {
    /// Construct a virtual clock starting at `initial_ns >= 0`.
    pub fn new(initial_ns: i64) -> Result<Self, LabError> {
        if initial_ns < 0 {
            return Err(LabError::InvalidConfig {
                field: "initial_ns",
                detail: "virtual clock initial_ns must be >= 0".to_string(),
            });
        }
        Ok(Self { now_ns: initial_ns })
    }

    /// Current virtual timestamp in nanoseconds.
    #[must_use]
    pub const fn current_ns(&self) -> i64 {
        self.now_ns
    }

    /// Advance the virtual clock monotonically to `target_ns`. Refuses backward jumps.
    pub fn advance_to(&mut self, target_ns: i64) -> Result<(), LabError> {
        if target_ns < self.now_ns {
            return Err(LabError::TimeWentBack {
                current_ns: self.now_ns,
                requested_ns: target_ns,
            });
        }
        self.now_ns = target_ns;
        Ok(())
    }

    /// Advance the virtual clock by `delta_ns >= 0`.
    pub fn advance_by(&mut self, delta_ns: i64) -> Result<i64, LabError> {
        if delta_ns < 0 {
            return Err(LabError::InvalidConfig {
                field: "delta_ns",
                detail: "virtual clock advance_by delta must be >= 0".to_string(),
            });
        }
        self.now_ns = self
            .now_ns
            .checked_add(delta_ns)
            .ok_or_else(|| LabError::InvalidConfig {
                field: "delta_ns",
                detail: "virtual clock nanosecond overflow".to_string(),
            })?;
        Ok(self.now_ns)
    }
}

impl WallClock for VirtualClock {
    fn now_ns(&self) -> Result<i64, ClockError> {
        if self.now_ns < 0 {
            Err(ClockError::BeforeUnixEpoch)
        } else {
            Ok(self.now_ns)
        }
    }
}

/// Diagnostic verbosity level tested for zero perturbation of authoritative state (`BD-337`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DiagnosticLevel {
    Quiet,
    Standard,
    Verbose,
}

impl DiagnosticLevel {
    pub const ALL: [Self; 3] = [Self::Quiet, Self::Standard, Self::Verbose];
}

/// Deterministic scheduler dispatch policy inside [`LabRuntime`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SchedulerPolicy {
    /// Strict virtual-timestamp then `event_id` FIFO ordering.
    Fifo,
    /// Highest-priority event ready at the current virtual timestamp first.
    PriorityFirst,
    /// Seeded `PhiloxStream` random selection among all events ready at the current virtual time.
    SeededRandom,
    /// Switches between priority-first and seeded-random under queue pressure (tests `BD-337` adaptation).
    AdaptiveBackpressure { pressure_threshold: usize },
}

/// One discrete event scheduled inside [`LabRuntime`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LabEvent {
    pub event_id: u64,
    pub deliver_at_ns: i64,
    pub priority: u8,
    pub actor_id: String,
    pub action_tag: String,
    pub payload_u64: u64,
}

/// One recorded execution trace entry produced by [`LabRuntime`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LabTraceEntry {
    pub step: u64,
    pub virtual_ns: i64,
    pub event_id: u64,
    pub actor_id: String,
    pub action_tag: String,
    pub payload_u64: u64,
}

/// Complete deterministic execution receipt from [`LabRuntime::run`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LabRunReport {
    pub seed: u64,
    pub diagnostic_level: DiagnosticLevel,
    pub policy: SchedulerPolicy,
    pub steps_executed: u64,
    pub final_virtual_ns: i64,
    pub trace_digest: Digest32,
    pub trace: Vec<LabTraceEntry>,
    pub diagnostics_emitted: usize,
}

/// Seeded deterministic virtual-time discrete-event lab runtime (`BD-099`).
///
/// Diagnostic instrumentation is strictly decoupled from the schedule `PhiloxStream` so changing
/// [`DiagnosticLevel`] can NEVER perturb event ordering or virtual timestamps.
#[derive(Debug, Clone)]
pub struct LabRuntime {
    seed: u64,
    clock: VirtualClock,
    policy: SchedulerPolicy,
    diagnostic_level: DiagnosticLevel,
    schedule_rng: PhiloxStream,
    next_event_id: u64,
    pending: Vec<LabEvent>,
    trace: Vec<LabTraceEntry>,
    diagnostics_emitted: usize,
}

impl LabRuntime {
    /// Construct a fresh deterministic lab runtime at `initial_ns` with the given `seed`.
    pub fn new(
        seed: u64,
        initial_ns: i64,
        policy: SchedulerPolicy,
        diagnostic_level: DiagnosticLevel,
    ) -> Result<Self, LabError> {
        let clock = VirtualClock::new(initial_ns)?;
        let schedule_rng = PhiloxStream::new(RngKey {
            seed_root: seed,
            partition: 0x0099,
            domain: 1,
            var_group: 0,
            realization: 0,
            brick: 0,
        });
        Ok(Self {
            seed,
            clock,
            policy,
            diagnostic_level,
            schedule_rng,
            next_event_id: 0,
            pending: Vec::new(),
            trace: Vec::new(),
            diagnostics_emitted: 0,
        })
    }

    /// Current virtual timestamp.
    #[must_use]
    pub const fn now_ns(&self) -> i64 {
        self.clock.current_ns()
    }

    /// Read-only view of the virtual clock.
    #[must_use]
    pub const fn clock(&self) -> &VirtualClock {
        &self.clock
    }

    /// Schedule one event at `deliver_at_ns >= now_ns()`. Returns its monotonic `event_id`.
    pub fn schedule(
        &mut self,
        deliver_at_ns: i64,
        priority: u8,
        actor_id: impl Into<String>,
        action_tag: impl Into<String>,
        payload_u64: u64,
    ) -> Result<u64, LabError> {
        if deliver_at_ns < self.clock.current_ns() {
            return Err(LabError::TimeWentBack {
                current_ns: self.clock.current_ns(),
                requested_ns: deliver_at_ns,
            });
        }
        let actor_id = actor_id.into();
        let action_tag = action_tag.into();
        if actor_id.trim().is_empty() || action_tag.trim().is_empty() {
            return Err(LabError::InvalidConfig {
                field: "event",
                detail: "actor_id and action_tag must be non-empty".to_string(),
            });
        }
        let id = self.next_event_id;
        self.next_event_id =
            self.next_event_id
                .checked_add(1)
                .ok_or_else(|| LabError::InvalidConfig {
                    field: "event_id",
                    detail: "event_id overflow".to_string(),
                })?;
        self.pending.push(LabEvent {
            event_id: id,
            deliver_at_ns,
            priority,
            actor_id,
            action_tag,
            payload_u64,
        });
        Ok(id)
    }

    /// Execute all scheduled events up to `max_steps`, invoking `handler(now_ns, &event, &mut newly_scheduled)`.
    pub fn run<F>(&mut self, max_steps: usize, mut handler: F) -> Result<LabRunReport, LabError>
    where
        F: FnMut(i64, &LabEvent, &mut Vec<LabEvent>) -> Result<(), LabError>,
    {
        let mut steps = 0_usize;
        while !self.pending.is_empty() {
            if steps >= max_steps {
                return Err(LabError::StepBudgetExceeded { max_steps });
            }
            let min_ts = self
                .pending
                .iter()
                .map(|e| e.deliver_at_ns)
                .min()
                .unwrap_or(self.clock.current_ns());
            self.clock.advance_to(min_ts)?;

            // Collect indices of all events ready at `min_ts` in deterministic `event_id` order.
            let mut ready: Vec<usize> = self
                .pending
                .iter()
                .enumerate()
                .filter_map(|(idx, e)| (e.deliver_at_ns == min_ts).then_some(idx))
                .collect();
            ready.sort_by_key(|&idx| self.pending[idx].event_id);

            let effective_policy = match self.policy {
                SchedulerPolicy::AdaptiveBackpressure { pressure_threshold } => {
                    if ready.len() >= pressure_threshold {
                        SchedulerPolicy::PriorityFirst
                    } else {
                        SchedulerPolicy::SeededRandom
                    }
                }
                other => other,
            };

            let chosen_pos = match effective_policy {
                SchedulerPolicy::Fifo => 0,
                SchedulerPolicy::PriorityFirst => ready
                    .iter()
                    .enumerate()
                    .max_by(|(_, a), (_, b)| {
                        let ea = &self.pending[**a];
                        let eb = &self.pending[**b];
                        ea.priority
                            .cmp(&eb.priority)
                            .then_with(|| eb.event_id.cmp(&ea.event_id))
                    })
                    .map_or(0, |(pos, _)| pos),
                SchedulerPolicy::SeededRandom | SchedulerPolicy::AdaptiveBackpressure { .. } => {
                    let n = ready.len() as u64;
                    if n <= 1 {
                        0
                    } else {
                        let reject_below = (0_u64.wrapping_sub(n)) % n;
                        loop {
                            let r = self.schedule_rng.next_u64();
                            if r >= reject_below {
                                break (r % n) as usize;
                            }
                        }
                    }
                }
            };

            let ev = self.pending.remove(ready[chosen_pos]);
            let now = self.clock.current_ns();

            // Diagnostics strictly observe without touching `schedule_rng` or authoritative data.
            self.diagnostics_emitted += match self.diagnostic_level {
                DiagnosticLevel::Quiet => 0,
                DiagnosticLevel::Standard => 1,
                DiagnosticLevel::Verbose => 3,
            };

            let mut spawned = Vec::new();
            handler(now, &ev, &mut spawned)?;
            for mut child in spawned {
                if child.deliver_at_ns < now {
                    return Err(LabError::TimeWentBack {
                        current_ns: now,
                        requested_ns: child.deliver_at_ns,
                    });
                }
                child.event_id = self.next_event_id;
                self.next_event_id += 1;
                self.pending.push(child);
            }

            self.trace.push(LabTraceEntry {
                step: steps as u64,
                virtual_ns: now,
                event_id: ev.event_id,
                actor_id: ev.actor_id,
                action_tag: ev.action_tag,
                payload_u64: ev.payload_u64,
            });
            steps += 1;
        }

        let trace_digest = compute_trace_digest(&self.trace);
        Ok(LabRunReport {
            seed: self.seed,
            diagnostic_level: self.diagnostic_level,
            policy: self.policy,
            steps_executed: steps as u64,
            final_virtual_ns: self.clock.current_ns(),
            trace_digest,
            trace: self.trace.clone(),
            diagnostics_emitted: self.diagnostics_emitted,
        })
    }
}

/// Canonical BLAKE3 digest over a [`LabTraceEntry`] slice (`BD-099`: same seed -> same digest).
#[must_use]
pub fn compute_trace_digest(trace: &[LabTraceEntry]) -> Digest32 {
    let mut buf = Vec::new();
    buf.extend_from_slice(b"fmine:lab-trace:v1\0");
    buf.extend_from_slice(&(trace.len() as u64).to_le_bytes());
    for entry in trace {
        buf.extend_from_slice(&entry.step.to_le_bytes());
        buf.extend_from_slice(&entry.virtual_ns.to_le_bytes());
        buf.extend_from_slice(&entry.event_id.to_le_bytes());
        buf.extend_from_slice(&(entry.actor_id.len() as u32).to_le_bytes());
        buf.extend_from_slice(entry.actor_id.as_bytes());
        buf.extend_from_slice(&(entry.action_tag.len() as u32).to_le_bytes());
        buf.extend_from_slice(entry.action_tag.as_bytes());
        buf.extend_from_slice(&entry.payload_u64.to_le_bytes());
    }
    blake3_hash(&buf)
}

// --- Dynamic Partial-Order Reduction (DPOR) Explorer (`BD-099`) ---

/// One resource read/write access declared by a concurrent actor transition for DPOR conflict checks.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ResourceAccess {
    pub resource_key: String,
    pub is_write: bool,
}

impl ResourceAccess {
    #[must_use]
    pub fn read(key: impl Into<String>) -> Self {
        Self {
            resource_key: key.into(),
            is_write: false,
        }
    }

    #[must_use]
    pub fn write(key: impl Into<String>) -> Self {
        Self {
            resource_key: key.into(),
            is_write: true,
        }
    }
}

/// One atomic step of a concurrent protocol actor explored by [`DporExplorer`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActorTransition {
    pub actor_id: usize,
    pub step_index: usize,
    pub label: String,
    pub accesses: Vec<ResourceAccess>,
}

impl ActorTransition {
    /// Two transitions conflict iff they belong to the same actor OR access the same resource with
    /// at least one write (standard Mazurkiewicz dependence relation).
    #[must_use]
    pub fn conflicts_with(&self, other: &Self) -> bool {
        if self.actor_id == other.actor_id {
            return true;
        }
        for a in &self.accesses {
            for b in &other.accesses {
                if a.resource_key == b.resource_key && (a.is_write || b.is_write) {
                    return true;
                }
            }
        }
        false
    }
}

/// Violation discovered by [`DporExplorer`] along with the exact minimal actor schedule that
/// triggered it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DporViolation {
    pub schedule: Vec<(usize, usize)>,
    pub detail: String,
}

/// Summary of a bounded Dynamic Partial-Order Reduction exploration (`BD-099`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DporReport {
    pub schedules_explored: usize,
    pub pruned_branches: usize,
    pub distinct_final_digests: BTreeSet<Digest32>,
    pub violations: Vec<DporViolation>,
}

/// Dynamic Partial-Order Reduction (Flanagan–Godefroid backtrack + sleep-set DPOR) explorer over
/// finite per-actor transition sequences (`BD-099`).
pub struct DporExplorer {
    actors: Vec<Vec<ActorTransition>>,
    max_schedules: usize,
}

impl DporExplorer {
    /// Construct a DPOR explorer over per-actor transition lists (`actors[a]` is actor `a`'s steps).
    pub fn new(actors: Vec<Vec<ActorTransition>>, max_schedules: usize) -> Result<Self, LabError> {
        if actors.is_empty() || max_schedules == 0 {
            return Err(LabError::InvalidConfig {
                field: "dpor",
                detail: "DPOR requires at least one actor and max_schedules >= 1".to_string(),
            });
        }
        Ok(Self {
            actors,
            max_schedules,
        })
    }

    /// Explore all Mazurkiewicz-inequivalent interleavings, running `evaluate_schedule` at each
    /// complete schedule to obtain `(final_state_digest, Option<violation_detail>)`.
    pub fn explore<F>(&self, mut evaluate_schedule: F) -> Result<DporReport, LabError>
    where
        F: FnMut(&[(usize, usize)]) -> (Digest32, Option<String>),
    {
        let total_steps: usize = self.actors.iter().map(Vec::len).sum();
        let n_actors = self.actors.len();

        let mut report = DporReport {
            schedules_explored: 0,
            pruned_branches: 0,
            distinct_final_digests: BTreeSet::new(),
            violations: Vec::new(),
        };

        // Backtrack sets indexed by search depth `0..=total_steps`.
        let mut backtrack: Vec<BTreeSet<usize>> = vec![BTreeSet::new(); total_steps + 1];
        let mut pc = vec![0_usize; n_actors];
        let mut trace_trans: Vec<ActorTransition> = Vec::with_capacity(total_steps);
        let mut trace_pairs: Vec<(usize, usize)> = Vec::with_capacity(total_steps);

        #[allow(clippy::too_many_arguments)]
        fn dfs<F>(
            actors: &[Vec<ActorTransition>],
            total_steps: usize,
            max_schedules: usize,
            depth: usize,
            pc: &mut [usize],
            trace_trans: &mut Vec<ActorTransition>,
            trace_pairs: &mut Vec<(usize, usize)>,
            backtrack: &mut [BTreeSet<usize>],
            sleep: &BTreeSet<usize>,
            report: &mut DporReport,
            eval: &mut F,
        ) -> Result<(), LabError>
        where
            F: FnMut(&[(usize, usize)]) -> (Digest32, Option<String>),
        {
            if report.schedules_explored >= max_schedules {
                return Ok(());
            }

            // Dynamic conflict detection: for every currently enabled transition `next_t`, find the
            // most recent conflicting executed transition at depth `j < depth` and add its actor
            // (or an enabled actor that can lead to it) into `backtrack[j]`.
            for (aid, steps) in actors.iter().enumerate() {
                if pc[aid] < steps.len() {
                    let next_t = &steps[pc[aid]];
                    for j in (0..depth).rev() {
                        if trace_trans[j].actor_id != aid && trace_trans[j].conflicts_with(next_t) {
                            backtrack[j].insert(aid);
                            break;
                        }
                    }
                }
            }

            if depth == total_steps {
                report.schedules_explored += 1;
                let (digest, maybe_err) = eval(trace_pairs);
                report.distinct_final_digests.insert(digest);
                if let Some(detail) = maybe_err {
                    report.violations.push(DporViolation {
                        schedule: trace_pairs.clone(),
                        detail,
                    });
                }
                return Ok(());
            }

            // Pick the lowest-id enabled actor not in the sleep set as the initial branch.
            let enabled: Vec<usize> = (0..actors.len())
                .filter(|&a| pc[a] < actors[a].len())
                .collect();
            let first = enabled.iter().copied().find(|a| !sleep.contains(a));
            let Some(initial_actor) = first else {
                report.pruned_branches += 1;
                return Ok(());
            };

            backtrack[depth].clear();
            backtrack[depth].insert(initial_actor);

            let mut done: BTreeSet<usize> = BTreeSet::new();
            let mut cur_sleep = sleep.clone();

            while let Some(&chosen) = backtrack[depth]
                .iter()
                .find(|a| !done.contains(a) && pc[**a] < actors[**a].len())
            {
                if report.schedules_explored >= max_schedules {
                    break;
                }
                done.insert(chosen);
                let step_idx = pc[chosen];
                let trans = actors[chosen][step_idx].clone();

                // Sleep set for child: keep slept actors whose next transition is independent of `trans`.
                let mut next_sleep = BTreeSet::new();
                for &s_actor in &cur_sleep {
                    if s_actor != chosen && pc[s_actor] < actors[s_actor].len() {
                        let s_trans = &actors[s_actor][pc[s_actor]];
                        if !s_trans.conflicts_with(&trans) {
                            next_sleep.insert(s_actor);
                        }
                    }
                }

                pc[chosen] += 1;
                trace_trans.push(trans.clone());
                trace_pairs.push((chosen, step_idx));

                dfs(
                    actors,
                    total_steps,
                    max_schedules,
                    depth + 1,
                    pc,
                    trace_trans,
                    trace_pairs,
                    backtrack,
                    &next_sleep,
                    report,
                    eval,
                )?;

                trace_pairs.pop();
                trace_trans.pop();
                pc[chosen] -= 1;

                // Record `chosen` as slept for any subsequent sibling branches at this depth.
                cur_sleep.insert(chosen);
            }

            let unchosen_enabled = enabled.len().saturating_sub(done.len());
            report.pruned_branches += unchosen_enabled;
            Ok(())
        }

        let initial_sleep = BTreeSet::new();
        dfs(
            &self.actors,
            total_steps,
            self.max_schedules,
            0,
            &mut pc,
            &mut trace_trans,
            &mut trace_pairs,
            &mut backtrack,
            &initial_sleep,
            &mut report,
            &mut evaluate_schedule,
        )?;

        Ok(report)
    }
}

/// Helper running a standard 3-actor protocol workload on `LabRuntime` for a given `seed`.
pub fn run_seeded_protocol_workload(
    seed: u64,
    policy: SchedulerPolicy,
    diagnostic_level: DiagnosticLevel,
) -> Result<LabRunReport, LabError> {
    let mut rt = LabRuntime::new(seed, 1_000_000, policy, diagnostic_level)?;
    rt.schedule(1_000_000, 10, "writer_a", "propose", 101)?;
    rt.schedule(1_000_000, 20, "writer_b", "propose", 202)?;
    rt.schedule(1_000_000, 15, "sync_peer", "gossip", 303)?;

    rt.run(200, |now, ev, out| {
        if ev.action_tag == "propose" {
            out.push(LabEvent {
                event_id: 0,
                deliver_at_ns: now + 50_000,
                priority: ev.priority,
                actor_id: ev.actor_id.clone(),
                action_tag: "commit".to_string(),
                payload_u64: ev.payload_u64 + 1,
            });
        } else if ev.action_tag == "gossip" {
            out.push(LabEvent {
                event_id: 0,
                deliver_at_ns: now + 50_000,
                priority: 5,
                actor_id: "sync_peer".to_string(),
                action_tag: "ack".to_string(),
                payload_u64: ev.payload_u64 + 1,
            });
        }
        Ok(())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    #[test]
    fn test_lab_runtime_seeds_0_to_999_same_seed_same_trace_digest() {
        let mut distinct_digests: BTreeSet<Digest32> = BTreeSet::new();
        for seed in 0_u64..1_000 {
            let r1 = run_seeded_protocol_workload(
                seed,
                SchedulerPolicy::SeededRandom,
                DiagnosticLevel::Standard,
            )
            .unwrap();
            let r2 = run_seeded_protocol_workload(
                seed,
                SchedulerPolicy::SeededRandom,
                DiagnosticLevel::Standard,
            )
            .unwrap();
            assert_eq!(
                r1.trace_digest, r2.trace_digest,
                "seed {seed} must replay to identical trace digest"
            );
            assert_eq!(r1.steps_executed, 6);
            assert_eq!(r1.final_virtual_ns, 1_050_000);
            distinct_digests.insert(r1.trace_digest);
        }
        // 3 actors at t=1_000_000 (3! = 6) x 3 followups at t=1_050_000 (3! = 6) = 36 possible traces.
        assert!(
            distinct_digests.len() >= 20,
            "1,000 seeds must exercise diverse interleavings, saw {}",
            distinct_digests.len()
        );
    }

    #[test]
    fn test_dpor_prunes_independent_actors_and_catches_seeded_lease_race() {
        // 1. 4 completely independent actors touching 4 disjoint cells -> 4! = 24 naive schedules,
        //    but DPOR MUST collapse them into exactly 1 Mazurkiewicz equivalence class!
        let disjoint: Vec<Vec<ActorTransition>> = (0..4)
            .map(|a| {
                vec![ActorTransition {
                    actor_id: a,
                    step_index: 0,
                    label: format!("write_cell_{a}"),
                    accesses: vec![ResourceAccess::write(format!("cell:{a}"))],
                }]
            })
            .collect();
        let dpor_indep = DporExplorer::new(disjoint, 1_000).unwrap();
        let rep_indep = dpor_indep
            .explore(|sched| {
                let mut map: BTreeMap<usize, usize> = BTreeMap::new();
                for &(a, s) in sched {
                    map.insert(a, s);
                }
                let bytes: Vec<u8> = map.keys().map(|&k| k as u8).collect();
                (blake3_hash(&bytes), None)
            })
            .unwrap();
        assert_eq!(
            rep_indep.schedules_explored, 1,
            "independent disjoint-cell actors must prune to 1 schedule, got {}",
            rep_indep.schedules_explored
        );
        assert!(rep_indep.pruned_branches >= 3);

        // 2. Two conflicting writers racing on the SAME cell lease (write-write conflict):
        //    DPOR MUST explore BOTH orders and detect a seeded race bug present ONLY when actor 1 runs first!
        let conflicting = vec![
            vec![ActorTransition {
                actor_id: 0,
                step_index: 0,
                label: "acquire_epoch_2".to_string(),
                accesses: vec![ResourceAccess::write("lease:CELL-PIT")],
            }],
            vec![ActorTransition {
                actor_id: 1,
                step_index: 0,
                label: "stale_write_epoch_1".to_string(),
                accesses: vec![ResourceAccess::write("lease:CELL-PIT")],
            }],
        ];
        let dpor_race = DporExplorer::new(conflicting, 100).unwrap();
        let rep_race = dpor_race
            .explore(|sched| {
                // Model a buggy non-fenced receiver: if epoch 2 commits first then stale epoch 1
                // overwrites it, flag a fencing violation!
                let mut current_epoch = 0_u8;
                let mut violation = None;
                for &(actor, _) in sched {
                    let next_epoch = if actor == 0 { 2 } else { 1 };
                    if next_epoch < current_epoch {
                        violation = Some("stale epoch 1 overwrote epoch 2".to_string());
                    }
                    current_epoch = next_epoch;
                }
                let digest = blake3_hash(&[current_epoch]);
                (digest, violation)
            })
            .unwrap();
        assert_eq!(
            rep_race.schedules_explored, 2,
            "conflicting lease writers must explore both race orders"
        );
        assert_eq!(rep_race.distinct_final_digests.len(), 2);
        assert_eq!(rep_race.violations.len(), 1);
        assert_eq!(rep_race.violations[0].schedule, vec![(0, 0), (1, 0)]);
    }
}
