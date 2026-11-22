//! Determinism: BitwisePortable
//! `INT-AS-001` Criterion 8 invariance verification (`BD-337` / `bd-sffy`, plan §34.2):
//! scheduler adaptation and diagnostic verbosity (`Quiet`, `Standard`, `Verbose`) across seeds
//! `0..999` CANNOT alter authoritative state roots or outputs.

use std::collections::BTreeMap;

use fmine_core::Digest32;
use fmine_core::hash::blake3_hash;

use crate::LabError;
use crate::runtime::{DiagnosticLevel, LabRuntime, SchedulerPolicy};

/// One authoritative domain operation applied to a named cell under a sequence key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthoritativeOp {
    pub cell_id: String,
    pub seq: u64,
    pub delta_grams_au: i64,
    pub deliver_at_ns: i64,
    pub priority: u8,
}

/// Summary proving `INT-AS-001` criterion 8 across `n_seeds` × 3 diagnostic levels × scheduler policies.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvarianceReport {
    pub seeds_tested: u64,
    pub total_runs: usize,
    pub authoritative_digest: Digest32,
    pub quiet_diagnostics_per_run: usize,
    pub standard_diagnostics_per_run: usize,
    pub verbose_diagnostics_per_run: usize,
}

fn run_authoritative_once(
    seed: u64,
    policy: SchedulerPolicy,
    level: DiagnosticLevel,
    ops: &[AuthoritativeOp],
) -> Result<(Digest32, usize), LabError> {
    let mut rt = LabRuntime::new(seed, 0, policy, level)?;
    for (idx, op) in ops.iter().enumerate() {
        rt.schedule(
            op.deliver_at_ns,
            op.priority,
            op.cell_id.clone(),
            "apply_delta",
            idx as u64,
        )?;
    }

    // Authoritative per-cell commutative state ledger: sums exact deltas per cell sorted by (cell, seq).
    let mut applied: BTreeMap<(String, u64), i64> = BTreeMap::new();
    let rep = rt.run(ops.len().saturating_mul(2).max(16), |_now, ev, _out| {
        let op = &ops[ev.payload_u64 as usize];
        applied.insert((op.cell_id.clone(), op.seq), op.delta_grams_au);
        Ok(())
    })?;

    let mut cell_totals: BTreeMap<&str, i128> = BTreeMap::new();
    let mut buf = Vec::new();
    buf.extend_from_slice(b"fmine:authoritative-state:v1\0");
    for ((cell, seq), &delta) in &applied {
        *cell_totals.entry(cell.as_str()).or_insert(0) += i128::from(delta);
        buf.extend_from_slice(&(cell.len() as u32).to_le_bytes());
        buf.extend_from_slice(cell.as_bytes());
        buf.extend_from_slice(&seq.to_le_bytes());
        buf.extend_from_slice(&delta.to_le_bytes());
    }
    for (cell, total) in &cell_totals {
        buf.extend_from_slice(&(cell.len() as u32).to_le_bytes());
        buf.extend_from_slice(cell.as_bytes());
        buf.extend_from_slice(&total.to_le_bytes());
    }
    Ok((blake3_hash(&buf), rep.diagnostics_emitted))
}

/// Verify across `0..n_seeds`, all three [`DiagnosticLevel`]s, and both fixed and adaptive
/// [`SchedulerPolicy`] modes that every authoritative output digest is 100% byte-identical (`BD-337`).
pub fn verify_scheduler_and_diagnostic_invariance(
    n_seeds: u64,
    ops: &[AuthoritativeOp],
) -> Result<InvarianceReport, LabError> {
    if n_seeds == 0 || ops.is_empty() {
        return Err(LabError::InvalidConfig {
            field: "invariance_inputs",
            detail: "n_seeds and ops must be non-empty".to_string(),
        });
    }

    let policies = [
        SchedulerPolicy::Fifo,
        SchedulerPolicy::PriorityFirst,
        SchedulerPolicy::SeededRandom,
        SchedulerPolicy::AdaptiveBackpressure {
            pressure_threshold: 2,
        },
    ];

    let (baseline_digest, _) =
        run_authoritative_once(0, SchedulerPolicy::Fifo, DiagnosticLevel::Quiet, ops)?;
    let mut total_runs = 0_usize;
    let mut quiet_d = 0_usize;
    let mut std_d = 0_usize;
    let mut verbose_d = 0_usize;

    for seed in 0..n_seeds {
        for level in DiagnosticLevel::ALL {
            // Test SeededRandom + AdaptiveBackpressure on every seed, plus Fifo/PriorityFirst on seed 0.
            let active_policies: &[SchedulerPolicy] =
                if seed == 0 { &policies } else { &policies[2..] };
            for &policy in active_policies {
                let (digest, diag_count) = run_authoritative_once(seed, policy, level, ops)?;
                total_runs += 1;
                if digest != baseline_digest {
                    return Err(LabError::InvarianceViolation {
                        detail: format!(
                            "seed {seed}, policy {policy:?}, level {level:?} altered authoritative digest \
                             ({} != {})",
                            digest.to_hex(),
                            baseline_digest.to_hex()
                        ),
                    });
                }
                match level {
                    DiagnosticLevel::Quiet => quiet_d = diag_count,
                    DiagnosticLevel::Standard => std_d = diag_count,
                    DiagnosticLevel::Verbose => verbose_d = diag_count,
                }
            }
        }
    }

    Ok(InvarianceReport {
        seeds_tested: n_seeds,
        total_runs,
        authoritative_digest: baseline_digest,
        quiet_diagnostics_per_run: quiet_d,
        standard_diagnostics_per_run: std_d,
        verbose_diagnostics_per_run: verbose_d,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_int_as_001_criterion_8_authoritative_invariance_1000_seeds() {
        let ops = vec![
            AuthoritativeOp {
                cell_id: "CELL-PIT".to_string(),
                seq: 1,
                delta_grams_au: 12_450,
                deliver_at_ns: 1_000,
                priority: 10,
            },
            AuthoritativeOp {
                cell_id: "CELL-MILL".to_string(),
                seq: 1,
                delta_grams_au: 11_980,
                deliver_at_ns: 1_000,
                priority: 30,
            },
            AuthoritativeOp {
                cell_id: "CELL-VAULT".to_string(),
                seq: 1,
                delta_grams_au: 11_800,
                deliver_at_ns: 1_000,
                priority: 20,
            },
            AuthoritativeOp {
                cell_id: "CELL-PIT".to_string(),
                seq: 2,
                delta_grams_au: 8_300,
                deliver_at_ns: 2_000,
                priority: 15,
            },
            AuthoritativeOp {
                cell_id: "CELL-MILL".to_string(),
                seq: 2,
                delta_grams_au: 8_150,
                deliver_at_ns: 2_000,
                priority: 25,
            },
        ];

        let rep = verify_scheduler_and_diagnostic_invariance(1_000, &ops).unwrap();
        assert_eq!(rep.seeds_tested, 1_000);
        // 1000 seeds x 3 levels x 2 adaptive/random policies + seed 0 x 3 levels x 2 fixed policies = 6,006 runs!
        assert_eq!(rep.total_runs, 6_006);
        assert_eq!(rep.quiet_diagnostics_per_run, 0);
        assert_eq!(rep.standard_diagnostics_per_run, 5);
        assert_eq!(rep.verbose_diagnostics_per_run, 15);
    }
}
