//! Synthetic source-to-geometry controls (BD-188, bd-zx57). These do not assert acQuire vendor parity.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use fmine_drill::desurvey::{CollarFrom, desurvey};
use fmine_drill::records::{HoleType, PositionMethod, RunDirection, SurveyInstrument};
use fmine_io::acquire::drill::{BearingReference, HoleGeometry, prepare_geometry};
use fmine_io::acquire::{
    AcquireBundleInput, AcquireExportBundle, AcquireTableInput, AssayReading,
    import_acquire_export_bundle,
};
use fmine_io::dialects::{ColumnBasis, ColumnSupport, IoDialectRegistry, ResolvedUnit};
use fmine_object::store::CasStore;
use fmine_registry::RegistryCatalog;
use fmine_units::ExactQty;

const COLLARS: &[u8] = b"HOLEID,EAST,NORTH,RL,MAXDEPTH,PROJECTCODE\nH2,500020,4100000,100,100,SOURCE-PROJECT\nH1,500000,4100000,100,100,SOURCE-PROJECT\n";
const SURVEYS: &[u8] = b"HOLEID,DEPTH,AZIMUTH,DIP,SURVEYTYPE\nH1,100,0,-90,GYRO\nH2,0,90,0,GYRO\nH1,0,0,-90,GYRO\nH2,100,90,0,GYRO\n";
const ASSAYS: &[u8] = b"HOLEID,FROM,TO,SAMPLEID,AU_GPT,CU_PCT,METHOD\nH1,50,100,S2,<0.005,NS,FA\nH2,0,100,S3,2,0.1,FA\nH1,0,50,S1,1.5,0.2,FA\n";

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}

fn imported(surveys: &[u8]) -> (AcquireExportBundle, BTreeSet<String>) {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let catalog = RegistryCatalog::load_from_root(&root()).unwrap();
    let units = catalog.unit_symbols.into_iter().collect();
    let crs: BTreeSet<String> = catalog.crs_codes.into_iter().collect();
    let dialects = IoDialectRegistry::load_from_root(&root()).unwrap();
    let store = CasStore::open(std::env::temp_dir().join(format!(
        "fmine-acquire-drill-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    )))
    .unwrap();
    let bundle = import_acquire_export_bundle(
        &store,
        &dialects,
        &AcquireBundleInput {
            collars: AcquireTableInput::Csv(COLLARS),
            surveys: AcquireTableInput::Csv(surveys),
            assays: AcquireTableInput::Csv(ASSAYS),
            site_crs: None,
        },
        &units,
        &crs,
        None,
    )
    .unwrap();
    (bundle, crs)
}

fn declarations() -> Vec<HoleGeometry> {
    ["H1", "H2"]
        .into_iter()
        .map(|hole| HoleGeometry {
            hole_id: hole.into(),
            program: "explicit geometry program".into(),
            hole_type: HoleType::Diamond,
            collar_from: CollarFrom::Actual,
            position_method: PositionMethod::DifferentialGnss,
            sigma_horizontal_m: 0.2,
            sigma_vertical_m: 0.5,
            instrument: SurveyInstrument::Gyro,
            run: RunDirection::In,
            sigma_azimuth_deg: 0.1,
            sigma_dip_deg: 0.2,
            magnetic_interference: false,
            azimuth_reference: BearingReference::Grid,
            survey_type_label: None,
        })
        .collect()
}

#[test]
fn imported_geometry_drives_the_existing_trace_and_preserves_exact_assay_support() {
    let (bundle, crs) = imported(SURVEYS);
    assert!(
        bundle.rejected_rows.is_empty(),
        "{:?}",
        bundle.rejected_rows
    );
    let original = bundle.clone();
    let prepared = prepare_geometry(&bundle, &declarations(), &crs).unwrap();
    assert_eq!(prepared.len(), 2);
    let hole = &prepared[0];
    assert_eq!(hole.collar.hole_id, "H1");
    assert_eq!(hole.collar_row_index, 3);
    assert_eq!(hole.max_depth_mm, 100_000);
    assert_eq!(
        hole.surveys
            .iter()
            .map(|s| (s.row_index, s.record.depth_mm))
            .collect::<Vec<_>>(),
        vec![(4, 0), (2, 100_000)]
    );
    let records = hole
        .surveys
        .iter()
        .map(|s| s.record.clone())
        .collect::<Vec<_>>();
    let trace = desurvey(&hole.collar, &records).unwrap();
    let at = trace.at(50_000);
    assert!((at.point.easting_m - 500_000.0).abs() < 1e-8);
    assert!((at.point.northing_m - 4_100_000.0).abs() < 1e-8);
    assert!((at.point.elevation_m - 50.0).abs() < 1e-8);
    assert!(!at.extrapolated);
    assert_eq!(hole.collar.actual.as_ref().unwrap().sigma_horizontal_m, 0.2);
    assert_eq!(records[0].orientation.unwrap().sigma_dip_deg, 0.2);
    assert!(
        records[0]
            .survey_id
            .contains(&bundle.survey_report.source_sha256)
    );

    assert_eq!((hole.assays[0].from_mm, hole.assays[0].to_mm), (0, 50_000));
    assert_eq!(hole.assays[0].source.row_index, 4);
    assert_eq!(
        (hole.assays[1].from_mm, hole.assays[1].to_mm),
        (50_000, 100_000)
    );
    assert_eq!(hole.assays[1].source.row_index, 2);
    assert!(matches!(&hole.assays[1].source.assays["AU_GPT"],
        AssayReading::BelowDetection { detection_limit, raw, .. }
        if *detection_limit == ExactQty::from_decimal_str("0.005").unwrap() && raw == "<0.005"));
    assert!(matches!(&hole.assays[1].source.assays["CU_PCT"],
        AssayReading::Placeholder { raw } if raw == "NS"));
    assert_eq!(bundle, original);

    let mut reordered = bundle.clone();
    reordered.collars.reverse();
    reordered.surveys.reverse();
    reordered.assays.reverse();
    let mut metadata = declarations();
    metadata.reverse();
    assert_eq!(
        prepare_geometry(&reordered, &metadata, &crs).unwrap(),
        prepared
    );
}

#[test]
fn every_hole_needs_explicit_metadata_and_no_quarantined_station_can_disappear() {
    let (bundle, crs) = imported(SURVEYS);
    let metadata = declarations();
    assert!(prepare_geometry(&bundle, &metadata[..1], &crs).is_err());
    let mut duplicate = metadata.clone();
    duplicate.push(metadata[0].clone());
    assert!(prepare_geometry(&bundle, &duplicate, &crs).is_err());
    duplicate[2].hole_id = "UNIMPORTED".into();
    assert!(prepare_geometry(&bundle, &duplicate, &crs).is_err());
    let mut missing = bundle.clone();
    missing.surveys.retain(|s| s.hole_id != "H2");
    assert!(prepare_geometry(&missing, &metadata, &crs).is_err());

    let mut bad = SURVEYS.to_vec();
    bad.extend_from_slice(b"H1,50,0,100,GYRO\n");
    let (quarantined, crs) = imported(&bad);
    assert_eq!(quarantined.rejected_rows.len(), 1);
    assert_eq!(quarantined.surveys.len(), bundle.surveys.len());
    let rejection = prepare_geometry(&quarantined, &metadata, &crs).unwrap_err();
    assert_eq!(rejection.code, "FMINE-REF-208");
    assert!(rejection.what.contains("quarantined"));
    assert_eq!(quarantined.rejected_rows[0].row_index, 6);
}

#[test]
fn metric_unit_basis_and_frame_checks_precede_any_geometry_projection() {
    let (bundle, crs) = imported(SURVEYS);
    let metadata = declarations();
    for column in [
        "EAST", "NORTH", "RL", "MAXDEPTH", "DEPTH", "AZIMUTH", "DIP", "FROM", "TO",
    ] {
        let mut changed = bundle.clone();
        let report = match column {
            "EAST" | "NORTH" | "RL" | "MAXDEPTH" => &mut changed.collar_report,
            "DEPTH" | "AZIMUTH" | "DIP" => &mut changed.survey_report,
            _ => &mut changed.assay_report,
        };
        report
            .resolved_columns
            .iter_mut()
            .find(|c| c.column_name == column)
            .unwrap()
            .unit = ResolvedUnit::Registered("ft".into());
        assert_eq!(
            prepare_geometry(&changed, &metadata, &crs)
                .unwrap_err()
                .code,
            "FMINE-REF-107"
        );
    }
    let mut changed = bundle.clone();
    changed
        .collar_report
        .resolved_columns
        .iter_mut()
        .find(|c| c.column_name == "EAST")
        .unwrap()
        .basis = Some(ColumnBasis::Wet);
    assert_eq!(
        prepare_geometry(&changed, &metadata, &crs)
            .unwrap_err()
            .code,
        "FMINE-REF-107"
    );
    changed = bundle.clone();
    changed
        .survey_report
        .resolved_columns
        .iter_mut()
        .find(|c| c.column_name == "DEPTH")
        .unwrap()
        .support = Some(ColumnSupport::Composite);
    assert_eq!(
        prepare_geometry(&changed, &metadata, &crs)
            .unwrap_err()
            .code,
        "FMINE-REF-107"
    );
    changed = bundle.clone();
    changed.crs = "EPSG:4326".into();
    for collar in &mut changed.collars {
        collar.crs = changed.crs.clone();
    }
    assert_eq!(
        prepare_geometry(&changed, &metadata, &crs)
            .unwrap_err()
            .code,
        "FMINE-REF-106"
    );
    assert_eq!(
        prepare_geometry(&bundle, &metadata, &BTreeSet::new())
            .unwrap_err()
            .code,
        "FMINE-REF-106"
    );
    for bearing in [BearingReference::True, BearingReference::Magnetic] {
        let mut changed = metadata.clone();
        changed[0].azimuth_reference = bearing;
        assert_eq!(
            prepare_geometry(&bundle, &changed, &crs).unwrap_err().code,
            "FMINE-REF-106"
        );
    }
}

#[test]
fn exact_depths_and_declared_accuracy_never_round_or_fill_missing_metadata() {
    let (bundle, crs) = imported(SURVEYS);
    let metadata = declarations();
    let mut changed = bundle.clone();
    changed.surveys[0].depth_m = ExactQty::from_decimal_str("0.0005").unwrap();
    assert_eq!(
        prepare_geometry(&changed, &metadata, &crs)
            .unwrap_err()
            .code,
        "FMINE-REF-109"
    );
    changed = bundle.clone();
    changed.assays[0].to_m = ExactQty::from_decimal_str("50.0001").unwrap();
    assert_eq!(
        prepare_geometry(&changed, &metadata, &crs)
            .unwrap_err()
            .code,
        "FMINE-REF-109"
    );
    changed = bundle.clone();
    changed.collars[0].max_depth_m = ExactQty::from_integer(1_i128 << 53);
    assert_eq!(
        prepare_geometry(&changed, &metadata, &crs)
            .unwrap_err()
            .code,
        "FMINE-REF-109"
    );
    for bad in [-1.0, f64::NAN, f64::INFINITY] {
        let mut changed = metadata.clone();
        changed[0].sigma_horizontal_m = bad;
        assert_eq!(
            prepare_geometry(&bundle, &changed, &crs).unwrap_err().code,
            "FMINE-REF-109"
        );
        changed = metadata.clone();
        changed[0].sigma_dip_deg = bad;
        assert_eq!(
            prepare_geometry(&bundle, &changed, &crs).unwrap_err().code,
            "FMINE-REF-109"
        );
    }
    let mut planned = metadata.clone();
    planned[0].collar_from = CollarFrom::Planned;
    let prepared = prepare_geometry(&bundle, &planned, &crs).unwrap();
    assert!(prepared[0].collar.actual.is_none());
    assert!(prepared[0].collar.planned.is_some());
}

#[test]
fn survey_labels_and_redacted_identities_cannot_be_silently_reinterpreted() {
    let (bundle, crs) = imported(SURVEYS);
    let metadata = declarations();
    let mut unknown = metadata.clone();
    unknown[0].instrument = SurveyInstrument::None;
    assert!(prepare_geometry(&bundle, &unknown, &crs).is_err());
    unknown[0].instrument = SurveyInstrument::MagneticMultishot;
    assert!(prepare_geometry(&bundle, &unknown, &crs).is_err());
    let mut labelled = bundle.clone();
    for s in &mut labelled.surveys {
        s.survey_type = Some("Declared gyro instrument".into());
    }
    assert!(prepare_geometry(&labelled, &metadata, &crs).is_err());
    let mut explicit = metadata.clone();
    for m in &mut explicit {
        m.survey_type_label = Some("Declared gyro instrument".into());
    }
    assert!(prepare_geometry(&labelled, &explicit, &crs).is_ok());
    let mut scrubbed = bundle.clone();
    scrubbed.assays[0].sample_id = Some("[REDACTED:PII]".into());
    assert_eq!(
        prepare_geometry(&scrubbed, &metadata, &crs)
            .unwrap_err()
            .code,
        "FMINE-REF-208"
    );
    scrubbed = bundle.clone();
    scrubbed.collars[0].hole_id = "[REDACTED:PII]".into();
    assert_eq!(
        prepare_geometry(&scrubbed, &metadata, &crs)
            .unwrap_err()
            .code,
        "FMINE-REF-208"
    );
}
