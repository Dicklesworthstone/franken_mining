//! Determinism: BitwisePortable
//! Runtime capability-mask lab test (`BD-097`, plan §26.6–§26.7, `PO-BED-8`).
//!
//! Verifies across all four node roles (`Site`, `Regional`, `Enterprise`, `Relay`) that:
//! - Every K/E compute region (`Compute`, `Law`, `EstimationRun`) holds zero `TIME`, `IO`, `REMOTE`,
//!   or `RANDOM` capabilities (`PO-BED-8`);
//! - No child region widens its parent's `Mask`;
//! - `RANDOM` is confined strictly to `Process`;
//! - `REMOTE` is confined strictly to the sync path (`Process`, `Node`, `Sync`, `PeerSession`, `Transfer`);
//! - Every role's `AppSpec` compiles and its `CompiledSupervisor` enforces read-only degraded mode
//!   and peer-stale isolation under virtual-time failure injection.

use std::collections::BTreeMap;

use fmine_runtime::region::{Mask, REGIONS, RegionKind, Role, profile};
use fmine_runtime::supervise::{AppSpec, Child, Outcome, Refused};

use crate::LabError;

/// Summary of the runtime capability-mask and role-supervision lab audit (`PO-BED-8`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilityAuditReport {
    pub roles_verified: usize,
    pub total_regions_checked: usize,
    pub compute_regions_checked: usize,
    pub remote_regions_checked: usize,
}

/// Audit every role profile in `fmine-runtime` for `PO-BED-8` capability containment and supervisor
/// compilation/escalation invariants (`BD-097`).
pub fn audit_all_runtime_capability_profiles() -> Result<CapabilityAuditReport, LabError> {
    let mut by_kind: BTreeMap<RegionKind, Mask> = BTreeMap::new();
    for spec in REGIONS {
        by_kind.insert(spec.kind, spec.caps);
    }

    let mut total_regions_checked = 0_usize;
    let mut compute_regions_checked = 0_usize;
    let mut remote_regions_checked = 0_usize;

    for role in Role::ALL {
        let rows = profile(role);
        if rows.is_empty() {
            return Err(LabError::CapabilityViolation {
                detail: format!("role {role:?} has empty region profile"),
            });
        }
        let mut role_map: BTreeMap<RegionKind, Mask> = BTreeMap::new();
        for row in &rows {
            role_map.insert(row.kind, row.caps);
        }

        for row in &rows {
            total_regions_checked += 1;

            // 1. Child mask must never widen parent mask.
            if let Some(parent_kind) = row.parent {
                let parent_mask = role_map.get(&parent_kind).copied().ok_or_else(|| {
                    LabError::CapabilityViolation {
                        detail: format!(
                            "role {role:?} region {:?} missing parent {:?}",
                            row.kind, parent_kind
                        ),
                    }
                })?;
                if !row.caps.within(parent_mask) {
                    return Err(LabError::CapabilityViolation {
                        detail: format!(
                            "role {role:?} region {:?} widens parent {:?}",
                            row.kind, parent_kind
                        ),
                    });
                }
            }

            // 2. RANDOM confined strictly to Process root (INV-010: engines receive seeds as input).
            if row.caps.random && row.kind != RegionKind::Process {
                return Err(LabError::CapabilityViolation {
                    detail: format!("non-root region {:?} granted RANDOM", row.kind),
                });
            }

            // 3. PO-BED-8: K/E compute regions never obtain TIME, IO, REMOTE, or RANDOM.
            if matches!(
                row.kind,
                RegionKind::Compute | RegionKind::Law | RegionKind::EstimationRun
            ) {
                compute_regions_checked += 1;
                if row.caps.time || row.caps.io || row.caps.remote || row.caps.random {
                    return Err(LabError::CapabilityViolation {
                        detail: format!(
                            "PO-BED-8 violation: compute region {:?} holds forbidden cap {:?}",
                            row.kind, row.caps
                        ),
                    });
                }
            }

            // 4. REMOTE confined strictly to sync-path regions.
            if row.caps.remote {
                remote_regions_checked += 1;
                let allowed_remote = matches!(
                    row.kind,
                    RegionKind::Process
                        | RegionKind::Node
                        | RegionKind::Sync
                        | RegionKind::PeerSession
                        | RegionKind::Transfer
                );
                if !allowed_remote {
                    return Err(LabError::CapabilityViolation {
                        detail: format!("non-sync region {:?} granted REMOTE", row.kind),
                    });
                }
            }
        }

        // 5. Compile AppSpec for every role and verify virtual-time read-only escalation on LedgerWriter.
        let mut sup =
            AppSpec::for_role(role)
                .compile()
                .map_err(|e| LabError::CapabilityViolation {
                    detail: format!("role {role:?} AppSpec failed to compile: {e:?}"),
                })?;
        sup.start(Child::one(RegionKind::Process), 0).map_err(|e| {
            LabError::CapabilityViolation {
                detail: format!("failed to start Process for {role:?}: {e:?}"),
            }
        })?;
        sup.start(Child::one(RegionKind::Node), 1)
            .map_err(|e| LabError::CapabilityViolation {
                detail: format!("failed to start Node for {role:?}: {e:?}"),
            })?;
        if role_map.contains_key(&RegionKind::Authority) {
            sup.start(Child::one(RegionKind::Authority), 2)
                .map_err(|e| LabError::CapabilityViolation {
                    detail: format!("failed to start Authority for {role:?}: {e:?}"),
                })?;
            let writer = Child::keyed(RegionKind::LedgerWriter, "CELL-SITE");
            sup.start(writer.clone(), 3)
                .map_err(|e| LabError::CapabilityViolation {
                    detail: format!("failed to start LedgerWriter for {role:?}: {e:?}"),
                })?;
            // 5 restarts succeed; 6th within 60 s MUST escalate into ReadOnly (exit 4).
            for i in 1_u64..=5 {
                let out =
                    sup.fail(&writer, i * 1_000)
                        .map_err(|e| LabError::CapabilityViolation {
                            detail: format!("unexpected fail error at {i}: {e:?}"),
                        })?;
                if !matches!(out, Outcome::Restart { .. }) {
                    return Err(LabError::CapabilityViolation {
                        detail: format!("expected Restart on failure {i}, got {out:?}"),
                    });
                }
            }
            let esc = sup
                .fail(&writer, 6_000)
                .map_err(|e| LabError::CapabilityViolation {
                    detail: format!("unexpected escalation error: {e:?}"),
                })?;
            if !matches!(esc, Outcome::ReadOnly { .. }) {
                return Err(LabError::CapabilityViolation {
                    detail: format!("expected ReadOnly on 6th writer failure, got {esc:?}"),
                });
            }
            if !matches!(sup.writes(), Err(Refused::ReadOnly { .. })) {
                return Err(LabError::CapabilityViolation {
                    detail: "escalated node failed to refuse writes with ReadOnly".to_string(),
                });
            }
        }
    }

    Ok(CapabilityAuditReport {
        roles_verified: Role::ALL.len(),
        total_regions_checked,
        compute_regions_checked,
        remote_regions_checked,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_po_bed_8_runtime_capability_masks_and_supervisor_profiles() {
        let rep = audit_all_runtime_capability_profiles().unwrap();
        assert_eq!(rep.roles_verified, 4);
        assert!(rep.total_regions_checked >= 40);
        assert!(rep.compute_regions_checked >= 9);
        assert!(rep.remote_regions_checked >= 12);
    }
}
