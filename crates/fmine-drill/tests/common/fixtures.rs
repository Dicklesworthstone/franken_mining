//! Records shared by the unit tests: one valid record of each kind, linked so that each can be written after
//! the ones it names.

use fmine_core::hash::sha256;

use super::records::{
    Alteration, AssayMethod, AssayResult, AssayValue, CertificateRef, Collar, CustodyEvent,
    Dispatch, DrillRecord, DuplicateStage, HoleType, Interval, LabCertificate, Logger, Orientation,
    Position, PositionMethod, QaqcRegime, QcInsertion, QcKind, Recovery, RecoveryMethod,
    RunDirection, Sample, SampleType, Survey, SurveyInstrument,
};

pub fn position(easting_m: f64) -> Position {
    Position {
        crs: "EPSG:32735".to_string(),
        easting_m,
        northing_m: 8_652_100.25,
        elevation_m: 1_312.5,
        method: PositionMethod::DifferentialGnss,
        sigma_horizontal_m: 0.05,
        sigma_vertical_m: 0.1,
    }
}

pub fn collar(hole_id: &str) -> Collar {
    Collar {
        hole_id: hole_id.to_string(),
        hole_type: HoleType::Diamond,
        program: "2026 infill".to_string(),
        planned: Some(Position {
            method: PositionMethod::Design,
            sigma_horizontal_m: 1.0,
            sigma_vertical_m: 1.0,
            ..position(421_000.0)
        }),
        actual: Some(position(421_000.4)),
    }
}

pub fn survey(survey_id: &str, hole_id: &str, depth_mm: u64) -> Survey {
    Survey {
        survey_id: survey_id.to_string(),
        hole_id: hole_id.to_string(),
        depth_mm,
        instrument: SurveyInstrument::Gyro,
        orientation: Some(Orientation {
            azimuth_deg: 87.5,
            dip_deg: -62.0,
            sigma_azimuth_deg: 0.5,
            sigma_dip_deg: 0.25,
        }),
        magnetic_interference: false,
        run: RunDirection::Out,
    }
}

pub fn interval(interval_id: &str, hole_id: &str, from_mm: u64, to_mm: u64) -> Interval {
    Interval {
        interval_id: interval_id.to_string(),
        hole_id: hole_id.to_string(),
        from_mm,
        to_mm,
        recovery: Some(Recovery {
            percent: 97.5,
            method: RecoveryMethod::Length,
        }),
        lithology: "SCHIST".to_string(),
        alteration: Some(Alteration {
            kind: "sericite".to_string(),
            intensity: "moderate".to_string(),
        }),
        mineralization: "chalcopyrite 1%".to_string(),
        weathering: "fresh".to_string(),
        logger: Logger::Human {
            logger_id: "geo-07".to_string(),
        },
    }
}

pub fn sample(sample_id: &str, interval_ids: &[&str]) -> Sample {
    Sample {
        sample_id: sample_id.to_string(),
        interval_ids: interval_ids.iter().map(|s| (*s).to_string()).collect(),
        mass_g: Some(3_250.0),
        sample_type: SampleType::HalfCore,
        splitter: "core saw".to_string(),
        custody: vec![
            CustodyEvent {
                at_ns: 1_790_000_000_000_000_000,
                holder: "core shed".to_string(),
            },
            CustodyEvent {
                at_ns: 1_790_000_360_000_000_000,
                holder: "courier".to_string(),
            },
        ],
    }
}

pub fn dispatch(dispatch_id: &str, lab: &str, sample_ids: &[&str]) -> Dispatch {
    Dispatch {
        dispatch_id: dispatch_id.to_string(),
        lab: lab.to_string(),
        sample_ids: sample_ids.iter().map(|s| (*s).to_string()).collect(),
        dispatched_at_ns: 1_790_000_400_000_000_000,
    }
}

pub fn certificate(certificate_id: &str, lab: &str) -> LabCertificate {
    LabCertificate {
        certificate_id: certificate_id.to_string(),
        lab: lab.to_string(),
        original_sha256: sha256(certificate_id.as_bytes()),
        media_type: "application/pdf".to_string(),
        issued_at_ns: 1_790_600_000_000_000_000,
    }
}

pub fn result(
    result_id: &str,
    sample_id: &str,
    dispatch_id: &str,
    lab: &str,
    certificate_id: Option<&str>,
) -> AssayResult {
    AssayResult {
        result_id: result_id.to_string(),
        sample_id: sample_id.to_string(),
        dispatch_id: dispatch_id.to_string(),
        analyte: "Cu".to_string(),
        method: AssayMethod::FourAcidIcpOes,
        value: AssayValue::Measured {
            mass_fraction: 0.0062,
        },
        lab: lab.to_string(),
        lab_batch: "LB-2211".to_string(),
        certificate: certificate_id.map_or(CertificateRef::LimsDirectNoCertificate, |id| {
            CertificateRef::Certificate {
                certificate_id: id.to_string(),
            }
        }),
        qaqc_regime: QaqcRegime::Current,
    }
}

pub fn duplicate(insertion_id: &str, sample_id: &str, original: &str) -> QcInsertion {
    QcInsertion {
        insertion_id: insertion_id.to_string(),
        sample_id: sample_id.to_string(),
        kind: QcKind::Duplicate {
            stage: DuplicateStage::Field,
            original_sample_id: original.to_string(),
        },
    }
}

/// A CRM or blank pulp inserted into the sample stream: no interval.
pub fn qc_material(sample_id: &str) -> Sample {
    Sample {
        sample_id: sample_id.to_string(),
        interval_ids: Vec::new(),
        mass_g: Some(60.0),
        sample_type: SampleType::QcMaterial,
        splitter: String::new(),
        custody: Vec::new(),
    }
}

pub fn crm(insertion_id: &str, sample_id: &str) -> QcInsertion {
    QcInsertion {
        insertion_id: insertion_id.to_string(),
        sample_id: sample_id.to_string(),
        kind: QcKind::Crm {
            crm_id: "CRM-CU-1".to_string(),
            certified_mass_fraction: 0.0048,
            sigma_c: 0.000_12,
            sigma_l: 0.000_09,
            certificate_source: "supplier certificate 2025".to_string(),
        },
    }
}

/// One record of every kind, in an order in which each names only records before it.
pub fn linked_set() -> Vec<DrillRecord> {
    vec![
        DrillRecord::Collar(collar("H1")),
        DrillRecord::Survey(survey("H1-S1", "H1", 30_000)),
        DrillRecord::Interval(interval("H1-I1", "H1", 0, 1_500)),
        DrillRecord::Interval(interval("H1-I2", "H1", 1_500, 3_000)),
        DrillRecord::Sample(sample("S-0001", &["H1-I1", "H1-I2"])),
        DrillRecord::Sample(sample("S-0002", &["H1-I2"])),
        DrillRecord::Dispatch(dispatch("D-01", "LAB-A", &["S-0001", "S-0002"])),
        DrillRecord::LabCertificate(certificate("C-77", "LAB-A")),
        DrillRecord::AssayResult(result("R-1", "S-0001", "D-01", "LAB-A", Some("C-77"))),
        DrillRecord::QcInsertion(duplicate("Q-1", "S-0002", "S-0001")),
    ]
}
