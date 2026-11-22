//! Determinism: BitwisePortable
//! Chaos fault-injection profiles (link drop, reorder, network partition, crash at every boundary)
//! and self-contained reproducible `CrashPack` artifacts (`BD-100`, plan §26.7 & §39 WP-05).

use fmine_core::Digest32;
use fmine_core::hash::{blake3_hash, sha256};
use fmine_core::rand::{PhiloxStream, RngKey};

use crate::LabError;
use crate::runtime::{DiagnosticLevel, LabEvent, LabRuntime, SchedulerPolicy};

/// Binary format header for serialized `CrashPack` artifacts (`FMNE` + `CRSH` + version `1`).
pub const CRASH_PACK_MAGIC: &[u8; 8] = b"FMNECRSH";
pub const CRASH_PACK_VERSION: u16 = 1;

/// One virtual-time network partition isolating `isolated_actor` over `[start_ns, end_ns]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartitionWindow {
    pub isolated_actor: String,
    pub start_ns: i64,
    pub end_ns: i64,
}

/// One crash boundary injected into a named actor on a specific protocol step tag.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CrashBoundary {
    pub actor_id: String,
    pub boundary_tag: String,
}

/// Deterministic chaos fault-injection profile (`BD-100`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChaosProfile {
    pub profile_name: String,
    /// Probability in basis points (`0..=10_000`) of dropping an inter-actor message.
    pub link_drop_bps: u16,
    /// Probability in basis points (`0..=10_000`) of reordering/delaying a message.
    pub reorder_bps: u16,
    /// Maximum virtual-time reorder delay in nanoseconds when a message is reordered.
    pub max_reorder_delay_ns: i64,
    pub partitions: Vec<PartitionWindow>,
    pub crash_boundaries: Vec<CrashBoundary>,
}

impl ChaosProfile {
    /// Validate chaos profile parameter ranges (refuse > 10,000 bps or inverted partition windows).
    pub fn validate(&self) -> Result<(), LabError> {
        if self.profile_name.trim().is_empty() {
            return Err(LabError::InvalidConfig {
                field: "profile_name",
                detail: "chaos profile_name must be non-empty".to_string(),
            });
        }
        if self.link_drop_bps > 10_000 || self.reorder_bps > 10_000 {
            return Err(LabError::InvalidConfig {
                field: "fault_bps",
                detail: "link_drop_bps and reorder_bps must lie in 0..=10_000".to_string(),
            });
        }
        if self.max_reorder_delay_ns < 0 {
            return Err(LabError::InvalidConfig {
                field: "max_reorder_delay_ns",
                detail: "max_reorder_delay_ns must be >= 0".to_string(),
            });
        }
        for p in &self.partitions {
            if p.isolated_actor.trim().is_empty() || p.start_ns > p.end_ns {
                return Err(LabError::InvalidConfig {
                    field: "partitions",
                    detail: "partition requires non-empty actor and start_ns <= end_ns".to_string(),
                });
            }
        }
        for cb in &self.crash_boundaries {
            if cb.actor_id.trim().is_empty() || cb.boundary_tag.trim().is_empty() {
                return Err(LabError::InvalidConfig {
                    field: "crash_boundaries",
                    detail: "crash boundary requires non-empty actor_id and boundary_tag"
                        .to_string(),
                });
            }
        }
        Ok(())
    }

    fn is_partitioned(&self, actor: &str, now_ns: i64) -> bool {
        self.partitions
            .iter()
            .any(|p| p.isolated_actor == actor && now_ns >= p.start_ns && now_ns <= p.end_ns)
    }

    fn should_crash(&self, actor: &str, action_tag: &str) -> bool {
        self.crash_boundaries
            .iter()
            .any(|c| c.actor_id == actor && c.boundary_tag == action_tag)
    }
}

/// Outcome of executing one seeded chaos simulation run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChaosOutcome {
    pub seed: u64,
    pub trace_digest: Digest32,
    pub messages_dropped: u64,
    pub messages_reordered: u64,
    pub crashes_injected: u64,
    pub violation: Option<(u64, i64, String)>,
}

/// Self-contained reproducible failure artifact (`BD-100`: failing seed reproduces bit-for-bit).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CrashPack {
    pub seed: u64,
    pub profile: ChaosProfile,
    pub failing_step: u64,
    pub failing_virtual_ns: i64,
    pub trace_digest: Digest32,
    pub violation_detail: String,
}

fn push_len_str(buf: &mut Vec<u8>, s: &str) -> Result<(), LabError> {
    let len = u32::try_from(s.len()).map_err(|_| LabError::CorruptCrashPack {
        detail: "string exceeds u32::MAX".to_string(),
    })?;
    buf.extend_from_slice(&len.to_le_bytes());
    buf.extend_from_slice(s.as_bytes());
    Ok(())
}

fn read_bytes<'a>(bytes: &'a [u8], cur: &mut usize, n: usize) -> Result<&'a [u8], LabError> {
    let end = cur
        .checked_add(n)
        .ok_or_else(|| LabError::CorruptCrashPack {
            detail: "cursor overflow".to_string(),
        })?;
    let sl = bytes
        .get(*cur..end)
        .ok_or_else(|| LabError::CorruptCrashPack {
            detail: "unexpected EOF decoding CrashPack".to_string(),
        })?;
    *cur = end;
    Ok(sl)
}

fn read_u16(bytes: &[u8], cur: &mut usize) -> Result<u16, LabError> {
    let b = read_bytes(bytes, cur, 2)?;
    Ok(u16::from_le_bytes([b[0], b[1]]))
}

fn read_u32(bytes: &[u8], cur: &mut usize) -> Result<u32, LabError> {
    let b = read_bytes(bytes, cur, 4)?;
    Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
}

fn read_u64(bytes: &[u8], cur: &mut usize) -> Result<u64, LabError> {
    let b = read_bytes(bytes, cur, 8)?;
    Ok(u64::from_le_bytes([
        b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7],
    ]))
}

fn read_i64(bytes: &[u8], cur: &mut usize) -> Result<i64, LabError> {
    let b = read_bytes(bytes, cur, 8)?;
    Ok(i64::from_le_bytes([
        b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7],
    ]))
}

fn read_len_str(bytes: &[u8], cur: &mut usize) -> Result<String, LabError> {
    let len = read_u32(bytes, cur)? as usize;
    let raw = read_bytes(bytes, cur, len)?;
    core::str::from_utf8(raw)
        .map(str::to_string)
        .map_err(|_| LabError::CorruptCrashPack {
            detail: "invalid UTF-8 string in CrashPack".to_string(),
        })
}

impl CrashPack {
    /// Encode to canonical binary format with trailing SHA-256 integrity tag.
    pub fn encode_bytes(&self) -> Result<Vec<u8>, LabError> {
        self.profile.validate()?;
        let mut buf = Vec::new();
        buf.extend_from_slice(CRASH_PACK_MAGIC);
        buf.extend_from_slice(&CRASH_PACK_VERSION.to_le_bytes());
        buf.extend_from_slice(&self.seed.to_le_bytes());
        push_len_str(&mut buf, &self.profile.profile_name)?;
        buf.extend_from_slice(&self.profile.link_drop_bps.to_le_bytes());
        buf.extend_from_slice(&self.profile.reorder_bps.to_le_bytes());
        buf.extend_from_slice(&self.profile.max_reorder_delay_ns.to_le_bytes());
        let p_len = u32::try_from(self.profile.partitions.len()).map_err(|_| {
            LabError::CorruptCrashPack {
                detail: "too many partitions".to_string(),
            }
        })?;
        buf.extend_from_slice(&p_len.to_le_bytes());
        for p in &self.profile.partitions {
            push_len_str(&mut buf, &p.isolated_actor)?;
            buf.extend_from_slice(&p.start_ns.to_le_bytes());
            buf.extend_from_slice(&p.end_ns.to_le_bytes());
        }
        let c_len = u32::try_from(self.profile.crash_boundaries.len()).map_err(|_| {
            LabError::CorruptCrashPack {
                detail: "too many crash boundaries".to_string(),
            }
        })?;
        buf.extend_from_slice(&c_len.to_le_bytes());
        for c in &self.profile.crash_boundaries {
            push_len_str(&mut buf, &c.actor_id)?;
            push_len_str(&mut buf, &c.boundary_tag)?;
        }
        buf.extend_from_slice(&self.failing_step.to_le_bytes());
        buf.extend_from_slice(&self.failing_virtual_ns.to_le_bytes());
        buf.extend_from_slice(self.trace_digest.as_bytes());
        push_len_str(&mut buf, &self.violation_detail)?;
        let checksum = sha256(&buf);
        buf.extend_from_slice(checksum.as_bytes());
        Ok(buf)
    }

    /// Decode a binary `CrashPack` after verifying magic, version, exact length, and SHA-256 digest.
    pub fn decode_bytes(bytes: &[u8]) -> Result<Self, LabError> {
        if bytes.len() < CRASH_PACK_MAGIC.len() + 2 + 32 {
            return Err(LabError::CorruptCrashPack {
                detail: "CrashPack too short".to_string(),
            });
        }
        let payload_len = bytes.len() - 32;
        let payload = &bytes[..payload_len];
        let expected_tag = &bytes[payload_len..];
        let computed = sha256(payload);
        let mut diff = 0_u8;
        for (&a, &b) in computed.as_bytes().iter().zip(expected_tag.iter()) {
            diff |= a ^ b;
        }
        if diff != 0 {
            return Err(LabError::CorruptCrashPack {
                detail: "CrashPack SHA-256 checksum mismatch".to_string(),
            });
        }

        let mut cur = 0_usize;
        let magic = read_bytes(payload, &mut cur, 8)?;
        if magic != CRASH_PACK_MAGIC {
            return Err(LabError::CorruptCrashPack {
                detail: "invalid CrashPack magic".to_string(),
            });
        }
        let ver = read_u16(payload, &mut cur)?;
        if ver != CRASH_PACK_VERSION {
            return Err(LabError::CorruptCrashPack {
                detail: format!("unsupported CrashPack version {ver}"),
            });
        }
        let seed = read_u64(payload, &mut cur)?;
        let profile_name = read_len_str(payload, &mut cur)?;
        let link_drop_bps = read_u16(payload, &mut cur)?;
        let reorder_bps = read_u16(payload, &mut cur)?;
        let max_reorder_delay_ns = read_i64(payload, &mut cur)?;
        let p_len = read_u32(payload, &mut cur)? as usize;
        if p_len > 1_024 {
            return Err(LabError::CorruptCrashPack {
                detail: "partition list exceeds cap".to_string(),
            });
        }
        let mut partitions = Vec::with_capacity(p_len);
        for _ in 0..p_len {
            let isolated_actor = read_len_str(payload, &mut cur)?;
            let start_ns = read_i64(payload, &mut cur)?;
            let end_ns = read_i64(payload, &mut cur)?;
            partitions.push(PartitionWindow {
                isolated_actor,
                start_ns,
                end_ns,
            });
        }
        let c_len = read_u32(payload, &mut cur)? as usize;
        if c_len > 1_024 {
            return Err(LabError::CorruptCrashPack {
                detail: "crash boundary list exceeds cap".to_string(),
            });
        }
        let mut crash_boundaries = Vec::with_capacity(c_len);
        for _ in 0..c_len {
            let actor_id = read_len_str(payload, &mut cur)?;
            let boundary_tag = read_len_str(payload, &mut cur)?;
            crash_boundaries.push(CrashBoundary {
                actor_id,
                boundary_tag,
            });
        }
        let failing_step = read_u64(payload, &mut cur)?;
        let failing_virtual_ns = read_i64(payload, &mut cur)?;
        let digest_raw = read_bytes(payload, &mut cur, 32)?;
        let mut d = [0u8; 32];
        d.copy_from_slice(digest_raw);
        let trace_digest = Digest32(d);
        let violation_detail = read_len_str(payload, &mut cur)?;
        if cur != payload.len() {
            return Err(LabError::CorruptCrashPack {
                detail: "trailing bytes inside CrashPack payload".to_string(),
            });
        }
        let profile = ChaosProfile {
            profile_name,
            link_drop_bps,
            reorder_bps,
            max_reorder_delay_ns,
            partitions,
            crash_boundaries,
        };
        profile.validate()?;
        Ok(Self {
            seed,
            profile,
            failing_step,
            failing_virtual_ns,
            trace_digest,
            violation_detail,
        })
    }
}

/// Run one seeded two-phase posting/sync protocol under `profile`.
///
/// When `allow_unprotected_retry` is `true`, a crash injected at `boundary_tag == "after_prepare"`
/// followed by a dropped/reordered ack causes a duplicate non-idempotent commit (simulating a real
/// protocol bug that `CrashPack` captures and reproduces deterministically).
pub fn run_chaos_once(
    seed: u64,
    profile: &ChaosProfile,
    allow_unprotected_retry: bool,
) -> Result<ChaosOutcome, LabError> {
    profile.validate()?;
    let mut rt = LabRuntime::new(
        seed,
        10_000,
        SchedulerPolicy::SeededRandom,
        DiagnosticLevel::Standard,
    )?;
    let mut fault_rng = PhiloxStream::new(RngKey {
        seed_root: seed,
        partition: 0x0100,
        domain: 2,
        var_group: 0,
        realization: 0,
        brick: 0,
    });

    rt.schedule(10_000, 10, "coordinator", "prepare", 1)?;
    rt.schedule(10_000, 10, "peer_b", "heartbeat", 1)?;

    let mut messages_dropped = 0_u64;
    let mut messages_reordered = 0_u64;
    let mut crashes_injected = 0_u64;
    let mut committed_slots: Vec<u64> = Vec::new();
    let mut violation: Option<(u64, i64, String)> = None;
    let mut step_counter = 0_u64;

    let rep = rt.run(200, |now, ev, out| {
        let cur_step = step_counter;
        step_counter += 1;

        if profile.is_partitioned(&ev.actor_id, now) {
            messages_dropped += 1;
            return Ok(());
        }

        if profile.should_crash(&ev.actor_id, &ev.action_tag) {
            crashes_injected += 1;
            // After crash at prepare boundary, coordinator restarts and retries slot `ev.payload_u64`.
            out.push(LabEvent {
                event_id: 0,
                deliver_at_ns: now + 5_000,
                priority: 10,
                actor_id: "coordinator".to_string(),
                action_tag: "retry_commit".to_string(),
                payload_u64: ev.payload_u64,
            });
            if allow_unprotected_retry {
                // In-flight commit also lands unless dropped by link drop.
                let roll = (fault_rng.next_u64() % 10_000) as u16;
                if roll >= profile.link_drop_bps {
                    let reorder_roll = (fault_rng.next_u64() % 10_000) as u16;
                    let delay = if reorder_roll < profile.reorder_bps
                        && profile.max_reorder_delay_ns > 0
                    {
                        messages_reordered += 1;
                        1 + (fault_rng.next_u64() % (profile.max_reorder_delay_ns as u64)) as i64
                    } else {
                        2_000
                    };
                    out.push(LabEvent {
                        event_id: 0,
                        deliver_at_ns: now + delay,
                        priority: 10,
                        actor_id: "ledger".to_string(),
                        action_tag: "commit_slot".to_string(),
                        payload_u64: ev.payload_u64,
                    });
                } else {
                    messages_dropped += 1;
                }
            }
            return Ok(());
        }

        match ev.action_tag.as_str() {
            "prepare" => {
                out.push(LabEvent {
                    event_id: 0,
                    deliver_at_ns: now + 1_000,
                    priority: 10,
                    actor_id: "coordinator".to_string(),
                    action_tag: "after_prepare".to_string(),
                    payload_u64: ev.payload_u64,
                });
            }
            "after_prepare" | "retry_commit" => {
                let roll = (fault_rng.next_u64() % 10_000) as u16;
                if roll < profile.link_drop_bps {
                    messages_dropped += 1;
                } else {
                    let reorder_roll = (fault_rng.next_u64() % 10_000) as u16;
                    let extra = if reorder_roll < profile.reorder_bps
                        && profile.max_reorder_delay_ns > 0
                    {
                        messages_reordered += 1;
                        (fault_rng.next_u64() % (profile.max_reorder_delay_ns as u64 + 1)) as i64
                    } else {
                        0
                    };
                    out.push(LabEvent {
                        event_id: 0,
                        deliver_at_ns: now + 1_000 + extra,
                        priority: 10,
                        actor_id: "ledger".to_string(),
                        action_tag: "commit_slot".to_string(),
                        payload_u64: ev.payload_u64,
                    });
                }
            }
            "commit_slot" => {
                if committed_slots.contains(&ev.payload_u64) && violation.is_none() {
                    violation = Some((
                        cur_step,
                        now,
                        format!("duplicate commit of posting slot {}", ev.payload_u64),
                    ));
                }
                committed_slots.push(ev.payload_u64);
            }
            _ => {}
        }
        Ok(())
    })?;

    // Fold fault counters into canonical trace digest so fault replay is strictly verified.
    let mut dig_buf = Vec::new();
    dig_buf.extend_from_slice(rep.trace_digest.as_bytes());
    dig_buf.extend_from_slice(&messages_dropped.to_le_bytes());
    dig_buf.extend_from_slice(&messages_reordered.to_le_bytes());
    dig_buf.extend_from_slice(&crashes_injected.to_le_bytes());
    let trace_digest = blake3_hash(&dig_buf);

    Ok(ChaosOutcome {
        seed,
        trace_digest,
        messages_dropped,
        messages_reordered,
        crashes_injected,
        violation,
    })
}

/// Sweep `seed_start..seed_end` under `profile` and return the first failing [`CrashPack`] found.
pub fn run_chaos_campaign(
    seed_start: u64,
    seed_end: u64,
    profile: &ChaosProfile,
    allow_unprotected_retry: bool,
) -> Result<Option<CrashPack>, LabError> {
    profile.validate()?;
    for seed in seed_start..seed_end {
        let outcome = run_chaos_once(seed, profile, allow_unprotected_retry)?;
        if let Some((failing_step, failing_virtual_ns, violation_detail)) = outcome.violation {
            return Ok(Some(CrashPack {
                seed,
                profile: profile.clone(),
                failing_step,
                failing_virtual_ns,
                trace_digest: outcome.trace_digest,
                violation_detail,
            }));
        }
    }
    Ok(None)
}

/// Replay a [`CrashPack`] and verify it reproduces the EXACT `failing_step`, `failing_virtual_ns`,
/// `trace_digest`, and `violation_detail` (`BD-100`).
pub fn replay_crash_pack(
    pack: &CrashPack,
    allow_unprotected_retry: bool,
) -> Result<ChaosOutcome, LabError> {
    let outcome = run_chaos_once(pack.seed, &pack.profile, allow_unprotected_retry)?;
    if outcome.trace_digest != pack.trace_digest {
        return Err(LabError::CorruptCrashPack {
            detail: format!(
                "CrashPack replay digest mismatch: expected {}, got {}",
                pack.trace_digest.to_hex(),
                outcome.trace_digest.to_hex()
            ),
        });
    }
    match &outcome.violation {
        Some((step, ts, detail))
            if *step == pack.failing_step
                && *ts == pack.failing_virtual_ns
                && *detail == pack.violation_detail =>
        {
            Ok(outcome)
        }
        other => Err(LabError::CorruptCrashPack {
            detail: format!("CrashPack replay failed to reproduce violation: got {other:?}"),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fmine_evidence::RefusalCode;

    #[test]
    fn test_chaos_profiles_and_crash_pack_reproduce_failing_seed() {
        let profile = ChaosProfile {
            profile_name: "link_drop_reorder_crash_prepare".to_string(),
            link_drop_bps: 2_500, // 25% link drop
            reorder_bps: 4_000,   // 40% message reorder
            max_reorder_delay_ns: 8_000,
            partitions: vec![PartitionWindow {
                isolated_actor: "peer_b".to_string(),
                start_ns: 10_000,
                end_ns: 20_000,
            }],
            crash_boundaries: vec![CrashBoundary {
                actor_id: "coordinator".to_string(),
                boundary_tag: "after_prepare".to_string(),
            }],
        };

        // 1. Protected protocol (no unprotected duplicate retry) survives all 200 seeds cleanly.
        assert!(
            run_chaos_campaign(0, 200, &profile, false)
                .unwrap()
                .is_none()
        );

        // 2. Unprotected retry under crash-at-boundary + link drop triggers a failing seed -> CrashPack!
        let pack = run_chaos_campaign(0, 200, &profile, true)
            .unwrap()
            .expect("chaos campaign must capture a failing seed as CrashPack");
        let encoded = pack.encode_bytes().unwrap();
        let decoded = CrashPack::decode_bytes(&encoded).unwrap();
        assert_eq!(pack, decoded);

        // 3. Replaying the decoded CrashPack reproduces the EXACT failing step, time, digest, and violation.
        let replayed = replay_crash_pack(&decoded, true).unwrap();
        assert_eq!(replayed.trace_digest, pack.trace_digest);
        assert!(replayed.crashes_injected >= 1);

        // 4. Tampering with a single byte of the CrashPack fails decode with FMINE-REF-505.
        let mut tampered = encoded;
        tampered[12] ^= 0x01;
        assert_eq!(
            CrashPack::decode_bytes(&tampered)
                .unwrap_err()
                .to_refusal()
                .code,
            RefusalCode::DigestMismatch505.spec().code
        );
    }
}
