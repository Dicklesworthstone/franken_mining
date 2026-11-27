//! Determinism: BitwisePortable
//!
//! Pre-transfer residency planner (`BD-107`, plan §31.2, `FMINE-REF-303`).
//!
//! Evaluates every proposed cross-border replication item against `registries/residency.toml`
//! BEFORE any bytes are scheduled onto the wire. Any disallowed item aborts the batch fail-closed
//! with `FMINE-REF-303` (`Residency303`, exit 7) and schedules zero bytes.

use std::collections::BTreeSet;

use fmine_authz::residency::{DataClass, ResidencyDecision, ResidencyRegistry, ResidencyRule};

use crate::SyncError;

/// One candidate replication item proposed for site-to-enterprise sync.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncTransferItem {
    pub cell_id: String,
    pub origin_country: String,
    pub destination_country: String,
    pub data_class: DataClass,
    pub is_aggregate: bool,
    pub payload_bytes: u64,
}

/// Approved transfer item bound to the active `residency_epoch` and effective statutory rule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannedTransfer {
    pub cell_id: String,
    pub origin_country: String,
    pub destination_country: String,
    pub data_class: DataClass,
    pub effective_rule: ResidencyRule,
    pub payload_bytes: u64,
}

/// Verified, residency-cleared replication plan (`BD-107`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResidencySyncPlan {
    pub residency_epoch: u64,
    pub transfers: Vec<PlannedTransfer>,
    pub total_planned_bytes: u64,
}

/// Evaluates all proposed `items` against `registry` BEFORE any bytes move.
/// Refuses atomically with `FMINE-REF-303` (exit 7) on the first disallowed transfer.
pub fn plan_residency_filtered_sync(
    registry: &ResidencyRegistry,
    items: &[SyncTransferItem],
    registered_instruments: &BTreeSet<String>,
) -> Result<ResidencySyncPlan, SyncError> {
    let mut transfers = Vec::with_capacity(items.len());
    let mut total_planned_bytes = 0u64;

    for item in items {
        if item.cell_id.trim().is_empty() {
            return Err(SyncError::InvalidField {
                field: "cell_id",
                detail: "transfer item cell_id must be non-empty".to_string(),
            });
        }
        match registry.evaluate_transfer(
            &item.origin_country,
            item.data_class,
            &item.destination_country,
            item.is_aggregate,
            registered_instruments,
        ) {
            ResidencyDecision::Allowed { effective_rule, .. } => {
                total_planned_bytes = total_planned_bytes
                    .checked_add(item.payload_bytes)
                    .ok_or_else(|| SyncError::InvalidField {
                        field: "payload_bytes",
                        detail: "total planned transfer bytes overflowed u64".to_string(),
                    })?;
                transfers.push(PlannedTransfer {
                    cell_id: item.cell_id.clone(),
                    origin_country: item.origin_country.clone(),
                    destination_country: item.destination_country.clone(),
                    data_class: item.data_class,
                    effective_rule,
                    payload_bytes: item.payload_bytes,
                });
            }
            ResidencyDecision::Refused(authz_err) => {
                return Err(SyncError::Authz(authz_err));
            }
        }
    }

    Ok(ResidencySyncPlan {
        residency_epoch: registry.residency_epoch,
        transfers,
        total_planned_bytes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn workspace_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|p| p.parent())
            .expect("workspace root")
            .to_path_buf()
    }

    /// BD-107 done-when (`FMINE-REF-303 tests`): verifies against the committed 72-row
    /// `registries/residency.toml` that Tanzania geoscience export, Zambia raw personal export,
    /// Pakistan geoscience without registered instrument, and unknown country pairs are all refused
    /// with `FMINE-REF-303` (exit 7) BEFORE any bytes move, while permitted rollups plan cleanly.
    #[test]
    fn test_residency_planner_refuses_disallowed_transfer_before_bytes_move_fmine_ref_303() {
        let reg = ResidencyRegistry::load_from_root(&workspace_root()).expect("residency.toml");
        let empty_instruments = BTreeSet::new();

        // 1. Tanzania geoscience (Mining Act s.35(6), in_country_only) to CA refuses FMINE-REF-303.
        let tz_geo = vec![SyncTransferItem {
            cell_id: "CELL-SITE(north_mara)".into(),
            origin_country: "TZ".into(),
            destination_country: "CA".into(),
            data_class: DataClass::Geoscience,
            is_aggregate: false,
            payload_bytes: 50_000_000,
        }];
        let err = plan_residency_filtered_sync(&reg, &tz_geo, &empty_instruments)
            .expect_err("TZ geoscience export must refuse");
        assert_eq!(err.to_refusal().code, "FMINE-REF-303");
        assert_eq!(err.to_refusal().exit_code, 7);

        // 2. Zambia raw personal data (DPA 2021 s.70) refuses FMINE-REF-303.
        let zm_personal = vec![SyncTransferItem {
            cell_id: "CELL-SITE(lumwana)".into(),
            origin_country: "ZM".into(),
            destination_country: "CA".into(),
            data_class: DataClass::Personal,
            is_aggregate: false,
            payload_bytes: 1_024,
        }];
        let err_zm = plan_residency_filtered_sync(&reg, &zm_personal, &empty_instruments)
            .expect_err("ZM personal export must refuse");
        assert_eq!(err_zm.to_refusal().code, "FMINE-REF-303");

        // 3. Unknown jurisdiction ("ZZ") fails closed to FMINE-REF-303.
        let unknown = vec![SyncTransferItem {
            cell_id: "CELL-SITE(unknown)".into(),
            origin_country: "ZZ".into(),
            destination_country: "US".into(),
            data_class: DataClass::Production,
            is_aggregate: true,
            payload_bytes: 4_096,
        }];
        let err_zz = plan_residency_filtered_sync(&reg, &unknown, &empty_instruments)
            .expect_err("unknown pair must fail closed");
        assert_eq!(err_zz.to_refusal().code, "FMINE-REF-303");

        // 4. Permitted US->CA aggregate + same-country ZM->ZM raw transfer plans cleanly.
        let ok_batch = vec![
            SyncTransferItem {
                cell_id: "CELL-SITE(ngm)".into(),
                origin_country: "US".into(),
                destination_country: "CA".into(),
                data_class: DataClass::Aggregate,
                is_aggregate: true,
                payload_bytes: 120_000,
            },
            SyncTransferItem {
                cell_id: "CELL-SITE(lumwana)".into(),
                origin_country: "ZM".into(),
                destination_country: "ZM".into(),
                data_class: DataClass::Personal,
                is_aggregate: false,
                payload_bytes: 8_000,
            },
        ];
        let plan =
            plan_residency_filtered_sync(&reg, &ok_batch, &empty_instruments).expect("plan ok");
        assert_eq!(plan.transfers.len(), 2);
        assert_eq!(plan.total_planned_bytes, 128_000);
        assert_eq!(plan.residency_epoch, reg.residency_epoch);

        // 5. Pakistan geoscience (BMR 2002 r.71(3)(i)) refuses aggregate export without registered
        //    legal instrument, allows aggregate export once registered, and still refuses raw export.
        let pk_geo_agg = vec![SyncTransferItem {
            cell_id: "CELL-SITE(reko_diq)".into(),
            origin_country: "PK".into(),
            destination_country: "CA".into(),
            data_class: DataClass::Geoscience,
            is_aggregate: true,
            payload_bytes: 16_384,
        }];
        let err_pk = plan_residency_filtered_sync(&reg, &pk_geo_agg, &empty_instruments)
            .expect_err("PK geoscience without instrument must fail closed");
        assert_eq!(err_pk.to_refusal().code, "FMINE-REF-303");

        let mut pk_instruments = BTreeSet::new();
        pk_instruments.insert(
            "Counsel reading of r.71(3)(i) + Reko Diq mineral agreement data clause".to_string(),
        );
        let pk_plan = plan_residency_filtered_sync(&reg, &pk_geo_agg, &pk_instruments)
            .expect("PK aggregate with instrument allowed");
        assert_eq!(pk_plan.total_planned_bytes, 16_384);

        let pk_geo_raw = vec![SyncTransferItem {
            is_aggregate: false,
            ..pk_geo_agg[0].clone()
        }];
        assert_eq!(
            plan_residency_filtered_sync(&reg, &pk_geo_raw, &pk_instruments)
                .expect_err("PK raw geoscience still refused")
                .to_refusal()
                .code,
            "FMINE-REF-303"
        );

        // 6. Mixed batch atomicity: a valid item followed by a disallowed item refuses the whole
        //    plan with FMINE-REF-303 so zero bytes are scheduled.
        let mixed = vec![ok_batch[0].clone(), tz_geo[0].clone()];
        assert_eq!(
            plan_residency_filtered_sync(&reg, &mixed, &empty_instruments)
                .expect_err("mixed batch with disallowed item must refuse atomically")
                .to_refusal()
                .code,
            "FMINE-REF-303"
        );

        // 7. Blank cell_id and u64 total byte overflow refuse cleanly (never clamped).
        let blank_cell = vec![SyncTransferItem {
            cell_id: "   ".into(),
            ..ok_batch[0].clone()
        }];
        assert!(matches!(
            plan_residency_filtered_sync(&reg, &blank_cell, &empty_instruments),
            Err(SyncError::InvalidField {
                field: "cell_id",
                ..
            })
        ));
        let overflow_batch = vec![
            SyncTransferItem {
                payload_bytes: u64::MAX,
                ..ok_batch[0].clone()
            },
            SyncTransferItem {
                payload_bytes: 1,
                ..ok_batch[1].clone()
            },
        ];
        assert!(matches!(
            plan_residency_filtered_sync(&reg, &overflow_batch, &empty_instruments),
            Err(SyncError::InvalidField {
                field: "payload_bytes",
                ..
            })
        ));
    }
}
