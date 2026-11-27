//! Record checks and canonical bytes (`BD-163`).
//! Determinism: BitwisePortable (test code; integer and byte operations).

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use fmine_core::codec::{Encoder, crc32c};
use fmine_evidence::RefusalCode;

use super::*;
use crate::fixtures::{
    certificate, collar, crm, dispatch, duplicate, interval, linked_set, position, qc_material,
    result, sample, survey,
};

fn round_trip(record: &DrillRecord) {
    for correction in [None, Some("relogged after the 2026 audit")] {
        let bytes = encode_version(record, correction).expect("valid record");
        let back = decode_version(&bytes).expect("canonical bytes decode");
        assert_eq!(back.record, *record);
        assert_eq!(back.correction.as_deref(), correction);
        assert_eq!(
            encode_version(&back.record, correction).expect("valid"),
            bytes
        );
    }
}

#[test]
fn every_record_kind_and_variant_round_trips_with_one_encoding() {
    for r in linked_set() {
        round_trip(&r);
    }
    for &k in RecordKind::ALL {
        assert_eq!(RecordKind::from_code(k.code()), Some(k));
    }
    assert_eq!(RecordKind::from_code("collars"), None);

    for &t in HoleType::ALL {
        round_trip(&DrillRecord::Collar(Collar {
            hole_type: t,
            ..collar("H")
        }));
    }
    for &m in PositionMethod::ALL {
        round_trip(&DrillRecord::Collar(Collar {
            planned: None,
            actual: Some(Position {
                method: m,
                ..position(1.0)
            }),
            ..collar("H")
        }));
    }
    round_trip(&DrillRecord::Collar(Collar {
        actual: None,
        ..collar("H")
    }));

    for &i in SurveyInstrument::ALL {
        for &run in RunDirection::ALL {
            for magnetic_interference in [false, true] {
                let base = survey("S", "H", 12_000);
                round_trip(&DrillRecord::Survey(Survey {
                    instrument: i,
                    orientation: if i == SurveyInstrument::None {
                        None
                    } else {
                        base.orientation
                    },
                    magnetic_interference,
                    run,
                    ..base
                }));
            }
        }
    }

    for &m in RecoveryMethod::ALL {
        round_trip(&DrillRecord::Interval(Interval {
            recovery: Some(Recovery {
                percent: 104.0,
                method: m,
            }),
            ..interval("I", "H", 0, 1)
        }));
    }
    for logger in [
        Logger::Human {
            logger_id: "g".to_string(),
        },
        Logger::Scanner {
            instrument_id: "hyperspectral-2".to_string(),
        },
        Logger::Model {
            name: "litho-net".to_string(),
            version: "3.1.0".to_string(),
        },
    ] {
        round_trip(&DrillRecord::Interval(Interval {
            recovery: None,
            alteration: None,
            logger,
            ..interval("I", "H", 7, 9)
        }));
    }

    for &t in SampleType::ALL {
        let intervals: &[&str] = if t == SampleType::QcMaterial {
            &[]
        } else {
            &["I1", "I2", "I3"]
        };
        round_trip(&DrillRecord::Sample(Sample {
            sample_type: t,
            mass_g: None,
            custody: Vec::new(),
            ..sample("S", intervals)
        }));
    }
    round_trip(&DrillRecord::Sample(qc_material("CRM-SLOT-1")));
    round_trip(&DrillRecord::QcInsertion(crm("Q-CRM", "CRM-SLOT-1")));

    for &m in AssayMethod::ALL {
        for &regime in QaqcRegime::ALL {
            round_trip(&DrillRecord::AssayResult(AssayResult {
                method: m,
                qaqc_regime: regime,
                ..result("R", "S", "D", "LAB", None)
            }));
        }
    }
    for value in [
        AssayValue::Measured { mass_fraction: 0.0 },
        AssayValue::Measured { mass_fraction: 1.0 },
        AssayValue::BelowDetection { limit: 5e-9 },
        AssayValue::AboveUpper { limit: 0.1 },
    ] {
        round_trip(&DrillRecord::AssayResult(AssayResult {
            value,
            ..result("R", "S", "D", "LAB", Some("C"))
        }));
    }

    let mut kinds = vec![
        QcKind::Crm {
            crm_id: "OREAS 503d".to_string(),
            certified_mass_fraction: 6.95e-7,
            sigma_c: 2.0e-8,
            sigma_l: 1.1e-8,
            certificate_source: "OREAS certificate 2024".to_string(),
        },
        QcKind::Umpire {
            original_sample_id: "S0".to_string(),
            umpire_lab: "LAB-B".to_string(),
        },
    ];
    kinds.extend(BlankStage::ALL.iter().map(|&stage| QcKind::Blank { stage }));
    kinds.extend(DuplicateStage::ALL.iter().map(|&stage| QcKind::Duplicate {
        stage,
        original_sample_id: "S0".to_string(),
    }));
    for kind in kinds {
        round_trip(&DrillRecord::QcInsertion(QcInsertion {
            insertion_id: "Q".to_string(),
            sample_id: "S1".to_string(),
            kind,
        }));
    }
}

#[test]
fn negative_zero_is_written_as_zero() {
    let zero = |v: f64| {
        DrillRecord::AssayResult(AssayResult {
            value: AssayValue::Measured { mass_fraction: v },
            ..result("R", "S", "D", "LAB", None)
        })
    };
    let bytes = encode_version(&zero(-0.0), None).expect("valid");
    assert_eq!(bytes, encode_version(&zero(0.0), None).expect("valid"));
    let back = decode_version(&bytes).expect("decodes");
    let DrillRecord::AssayResult(r) = back.record else {
        panic!("kind");
    };
    let AssayValue::Measured { mass_fraction } = r.value else {
        panic!("value");
    };
    assert_eq!(mass_fraction.to_bits(), 0.0_f64.to_bits());
}

#[test]
fn only_a_magnetic_reading_with_interference_needs_a_gyro_check() {
    let s = |instrument, magnetic_interference| Survey {
        instrument,
        magnetic_interference,
        ..survey("S", "H", 0)
    };
    assert!(s(SurveyInstrument::MagneticMultishot, true).needs_gyro_check());
    assert!(s(SurveyInstrument::MagneticSingleShot, true).needs_gyro_check());
    assert!(!s(SurveyInstrument::MagneticMultishot, false).needs_gyro_check());
    assert!(!s(SurveyInstrument::Gyro, true).needs_gyro_check());
}

#[test]
fn records_that_cannot_be_true_are_refused() {
    let bad_orientation = |azimuth_deg: f64, dip_deg: f64| {
        DrillRecord::Survey(Survey {
            orientation: Some(Orientation {
                azimuth_deg,
                dip_deg,
                sigma_azimuth_deg: 0.5,
                sigma_dip_deg: 0.5,
            }),
            ..survey("S", "H", 0)
        })
    };
    let mut other_crs = position(1.0);
    other_crs.crs = "EPSG:4326".to_string();
    let cases: Vec<(DrillRecord, &str, RefusalCode)> = vec![
        (
            DrillRecord::Interval(interval("I", "H", 1_500, 1_500)),
            "interval.to_mm",
            RefusalCode::OutsidePhysicalDomain109,
        ),
        (
            DrillRecord::Interval(interval("I", "H", 2_000, 1_500)),
            "interval.to_mm",
            RefusalCode::OutsidePhysicalDomain109,
        ),
        (
            bad_orientation(360.0, -60.0),
            "survey.azimuth_deg",
            RefusalCode::OutsidePhysicalDomain109,
        ),
        (
            bad_orientation(-0.5, -60.0),
            "survey.azimuth_deg",
            RefusalCode::OutsidePhysicalDomain109,
        ),
        (
            bad_orientation(10.0, -90.5),
            "survey.dip_deg",
            RefusalCode::OutsidePhysicalDomain109,
        ),
        (
            bad_orientation(f64::NAN, -60.0),
            "survey.azimuth_deg",
            RefusalCode::OutsidePhysicalDomain109,
        ),
        (
            DrillRecord::Survey(Survey {
                instrument: SurveyInstrument::None,
                ..survey("S", "H", 0)
            }),
            "survey.orientation",
            RefusalCode::OutsidePhysicalDomain109,
        ),
        (
            DrillRecord::Survey(Survey {
                orientation: None,
                ..survey("S", "H", 0)
            }),
            "survey.orientation",
            RefusalCode::OutsidePhysicalDomain109,
        ),
        (
            DrillRecord::Collar(Collar {
                planned: None,
                actual: None,
                ..collar("H")
            }),
            "collar.planned",
            RefusalCode::OutsidePhysicalDomain109,
        ),
        (
            DrillRecord::Collar(Collar {
                actual: Some(other_crs),
                ..collar("H")
            }),
            "",
            RefusalCode::CrsUnregisteredOrIncompatible106,
        ),
        (
            DrillRecord::Collar(Collar {
                actual: Some(Position {
                    sigma_horizontal_m: -0.01,
                    ..position(1.0)
                }),
                ..collar("H")
            }),
            "collar.actual.sigma_horizontal_m",
            RefusalCode::OutsidePhysicalDomain109,
        ),
        (
            DrillRecord::Collar(Collar {
                actual: Some(Position {
                    northing_m: f64::INFINITY,
                    ..position(1.0)
                }),
                ..collar("H")
            }),
            "collar.actual.northing_m",
            RefusalCode::OutsidePhysicalDomain109,
        ),
        (
            DrillRecord::Collar(collar("H/1")),
            "collar.hole_id",
            RefusalCode::OutsidePhysicalDomain109,
        ),
        (
            DrillRecord::Collar(collar("")),
            "collar.hole_id",
            RefusalCode::OutsidePhysicalDomain109,
        ),
        (
            DrillRecord::Collar(collar("H 1")),
            "collar.hole_id",
            RefusalCode::OutsidePhysicalDomain109,
        ),
        (
            DrillRecord::Collar(collar(&"H".repeat(MAX_ID_BYTES + 1))),
            "collar.hole_id",
            RefusalCode::OutsidePhysicalDomain109,
        ),
        (
            DrillRecord::Interval(Interval {
                recovery: Some(Recovery {
                    percent: -1.0,
                    method: RecoveryMethod::Mass,
                }),
                ..interval("I", "H", 0, 1)
            }),
            "interval.recovery.percent",
            RefusalCode::OutsidePhysicalDomain109,
        ),
        (
            DrillRecord::Interval(Interval {
                lithology: "x".repeat(MAX_TEXT_BYTES + 1),
                ..interval("I", "H", 0, 1)
            }),
            "interval.lithology",
            RefusalCode::OutsidePhysicalDomain109,
        ),
        (
            DrillRecord::Sample(sample("S", &["I2", "I1"])),
            "sample.interval_ids",
            RefusalCode::OutsidePhysicalDomain109,
        ),
        (
            DrillRecord::Sample(sample("S", &["I1", "I1"])),
            "sample.interval_ids",
            RefusalCode::OutsidePhysicalDomain109,
        ),
        (
            DrillRecord::Sample(sample("S", &[])),
            "sample.interval_ids",
            RefusalCode::OutsidePhysicalDomain109,
        ),
        (
            DrillRecord::Sample(Sample {
                interval_ids: vec!["I1".to_string()],
                ..qc_material("S")
            }),
            "sample.interval_ids",
            RefusalCode::OutsidePhysicalDomain109,
        ),
        (
            DrillRecord::Sample(Sample {
                mass_g: Some(0.0),
                ..sample("S", &["I1"])
            }),
            "sample.mass_g",
            RefusalCode::OutsidePhysicalDomain109,
        ),
        (
            DrillRecord::Sample(Sample {
                custody: sample("S", &["I1"]).custody.into_iter().rev().collect(),
                ..sample("S", &["I1"])
            }),
            "sample.custody",
            RefusalCode::OutsidePhysicalDomain109,
        ),
        (
            DrillRecord::Dispatch(dispatch("D", " ", &["S1"])),
            "dispatch.lab",
            RefusalCode::OutsidePhysicalDomain109,
        ),
        (
            DrillRecord::AssayResult(AssayResult {
                // Porgera's database held S > 100% (plan §12.1).
                value: AssayValue::Measured {
                    mass_fraction: 1.02,
                },
                ..result("R", "S", "D", "LAB", None)
            }),
            "result.value",
            RefusalCode::OutsidePhysicalDomain109,
        ),
        (
            DrillRecord::AssayResult(AssayResult {
                value: AssayValue::BelowDetection { limit: 0.0 },
                ..result("R", "S", "D", "LAB", None)
            }),
            "result.value.limit",
            RefusalCode::OutsidePhysicalDomain109,
        ),
        (
            DrillRecord::QcInsertion(duplicate("Q", "S1", "S1")),
            "qc.original_sample_id",
            RefusalCode::OutsidePhysicalDomain109,
        ),
        (
            DrillRecord::QcInsertion(QcInsertion {
                kind: QcKind::Crm {
                    crm_id: "OREAS 503d".to_string(),
                    certified_mass_fraction: 6.95e-7,
                    sigma_c: -1e-9,
                    sigma_l: 1e-9,
                    certificate_source: "x".to_string(),
                },
                ..duplicate("Q", "S1", "S0")
            }),
            "qc.sigma_c",
            RefusalCode::OutsidePhysicalDomain109,
        ),
        (
            DrillRecord::LabCertificate(LabCertificate {
                media_type: String::new(),
                ..certificate("C", "LAB")
            }),
            "certificate.media_type",
            RefusalCode::OutsidePhysicalDomain109,
        ),
    ];
    for (record, field, code) in cases {
        let err = record.validate().expect_err("refused");
        assert_eq!(err.to_refusal().code, code.spec().code, "{record:?}: {err}");
        if let DrillError::Invalid { field: f, .. } = &err {
            assert_eq!(f, field, "{record:?}");
        }
        assert_eq!(encode_version(&record, None), Err(err));
    }
    for reason in ["", "   "] {
        let err = encode_version(&DrillRecord::Collar(collar("H")), Some(reason))
            .expect_err("a correction says why");
        assert!(matches!(&err, DrillError::Invalid { field, .. } if field == "correction"));
    }
}

/// Seal `body` (a frame without its checksum) the way `Encoder::finish` does.
fn reseal(mut body: Vec<u8>) -> Vec<u8> {
    let crc = crc32c(&body);
    body.extend_from_slice(&crc.to_le_bytes());
    body
}

#[test]
fn decoding_refuses_torn_tampered_and_non_canonical_bytes() {
    let record = DrillRecord::AssayResult(result("R-1", "S-0001", "D-01", "LAB-A", Some("C-77")));
    let bytes = encode_version(&record, Some("lab reissued the certificate")).expect("valid");
    for len in 0..bytes.len() {
        assert!(
            matches!(
                decode_version(&bytes[..len]),
                Err(DrillError::Corrupt { .. })
            ),
            "prefix of {len} bytes"
        );
    }
    for i in 0..bytes.len() {
        let mut flipped = bytes.clone();
        flipped[i] ^= 0x01;
        assert!(
            matches!(decode_version(&flipped), Err(DrillError::Corrupt { .. })),
            "byte {i} flipped"
        );
    }

    // A whole frame with a byte after the record.
    let mut enc = Encoder::new_frame(RECORD_TAG, RECORD_VERSION);
    put_record(&mut enc, &record).expect("writes");
    put_opt(&mut enc, None::<&String>, |e, s| e.put_str(s));
    enc.put_u8(0);
    assert!(matches!(
        decode_version(&enc.finish()),
        Err(DrillError::Corrupt { .. })
    ));

    // A whole frame whose record the checks refuse.
    let mut enc = Encoder::new_frame(RECORD_TAG, RECORD_VERSION);
    put_record(&mut enc, &DrillRecord::Interval(interval("I", "H", 9, 9))).expect("writes");
    put_opt(&mut enc, None::<&String>, |e, s| e.put_str(s));
    let err = decode_version(&enc.finish()).expect_err("refused");
    assert!(err.to_string().contains("interval.to_mm"), "{err}");
    assert_eq!(
        err.to_refusal().code,
        RefusalCode::DigestMismatch505.spec().code
    );

    // A whole frame holding -0.0, which encode_version writes as 0.0.
    let marker = 0.006_25_f64;
    let canonical = encode_version(
        &DrillRecord::AssayResult(AssayResult {
            value: AssayValue::Measured {
                mass_fraction: marker,
            },
            ..result("R", "S", "D", "LAB", None)
        }),
        None,
    )
    .expect("valid");
    let mut body = canonical[..canonical.len() - 4].to_vec();
    let at = body
        .windows(8)
        .position(|w| w == marker.to_bits().to_le_bytes())
        .expect("the value's bytes");
    body[at..at + 8].copy_from_slice(&(-0.0_f64).to_bits().to_le_bytes());
    let err = decode_version(&reseal(body)).expect_err("non-canonical");
    assert!(err.to_string().contains("canonical encoding"), "{err}");

    // Another frame tag.
    let mut enc = Encoder::new_frame(*b"OBEV", RECORD_VERSION);
    put_record(&mut enc, &record).expect("writes");
    put_opt(&mut enc, None::<&String>, |e, s| e.put_str(s));
    assert!(decode_version(&enc.finish()).is_err());
}
