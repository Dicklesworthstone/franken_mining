//! Twin holes (`BD-285`'s done-when: `WeakTwin` above 5 m; disclosure-basis refusal test). Separation comes
//! from the desurveyed traces, so a twin collared close by can still be weak at depth. Agreement statistics
//! have known answers. `G-TWIN` admits a twin-derived correction to a public or disclosure-basis estimate only
//! under recorded acceptances of every pair. The RC smearing monitor fires on a seeded synthetic campaign of
//! smeared holes and stays near its significance level on clean ones.

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::float_cmp
)]

use fmine_drill::desurvey::{Trace, desurvey};
use fmine_drill::join::Span;
use fmine_drill::records::{
    self, Collar, DrillError, DrillRecord, HoleType, Interval, Orientation, Survey, TwinAcceptance,
    TwinMatch, TwinPair,
};
use fmine_drill::store::{Change, StoreError, commit};
use fmine_drill::survey_error::{ErrorModel, propagate};
use fmine_drill::twin::{
    Admission, DownholeGrade, EstimateBasis, PairGrade, SmearingPolicy, SmearingState,
    TwinCorrection, TwinPolicy, TwinState, admit_twin_correction, assess_twin,
    separation_uncertainty, smearing_monitor,
};
use fmine_evidence::RefusalCode;
use fmine_ledger::l0::L0Ledger;

#[allow(dead_code)]
#[path = "common/fixtures.rs"]
mod fixtures;

use fixtures::{collar, interval, position, survey};

const E0: f64 = 421_000.0;

fn hole(hole_id: &str, easting_m: f64, hole_type: HoleType) -> Collar {
    Collar {
        hole_type,
        planned: None,
        actual: Some(position(easting_m)),
        ..collar(hole_id)
    }
}

fn station(hole_id: &str, depth_mm: u64, azimuth_deg: f64, dip_deg: f64) -> Survey {
    Survey {
        orientation: Some(Orientation {
            azimuth_deg,
            dip_deg,
            sigma_azimuth_deg: 0.5,
            sigma_dip_deg: 0.25,
        }),
        ..survey(&format!("{hole_id}-S{depth_mm}"), hole_id, depth_mm)
    }
}

/// A straight hole from `easting_m`, surveyed at the collar and at 300 m.
fn straight(hole_id: &str, easting_m: f64, azimuth_deg: f64, dip_deg: f64) -> (Collar, Trace) {
    let c = hole(hole_id, easting_m, HoleType::Diamond);
    let surveys = [
        station(hole_id, 0, azimuth_deg, dip_deg),
        station(hole_id, 300_000, azimuth_deg, dip_deg),
    ];
    let trace = desurvey(&c, &surveys).unwrap();
    (c, trace)
}

/// Two matched intervals per hole: 10–12 m and 100–102 m.
fn intervals(original: &str, twin: &str) -> Vec<Interval> {
    let mut out = Vec::new();
    for h in [original, twin] {
        out.push(interval(&format!("{h}-I1"), h, 10_000, 12_000));
        out.push(interval(&format!("{h}-I2"), h, 100_000, 102_000));
    }
    out
}

fn pair(twin_id: &str, original: &str, twin: &str) -> TwinPair {
    TwinPair {
        twin_id: twin_id.to_string(),
        original_hole_id: original.to_string(),
        twin_hole_id: twin.to_string(),
        matches: ["I1", "I2"]
            .into_iter()
            .map(|i| TwinMatch {
                original_interval_id: format!("{original}-{i}"),
                twin_interval_id: format!("{twin}-{i}"),
            })
            .collect(),
    }
}

const GRADES: [PairGrade; 2] = [
    PairGrade {
        original: 1.2,
        twin: 1.0,
    },
    PairGrade {
        original: 4.0,
        twin: 3.1,
    },
];

#[test]
fn test_a_twin_more_than_five_metres_away_on_its_trace_is_a_weak_twin() {
    let (_, o) = straight("O", E0, 0.0, -90.0);
    let policy = TwinPolicy::default();
    assert_eq!(policy.separation_limit_mm, 5_000);
    // Vertical twins: the separation is the collar offset at every depth.
    for (offset, state) in [
        (4.0, TwinState::Twin),
        (5.0, TwinState::Twin),
        (5.01, TwinState::WeakTwin),
        (12.0, TwinState::WeakTwin),
    ] {
        let (_, t) = straight("T", E0 + offset, 0.0, -90.0);
        let got = assess_twin(
            &pair("W", "O", "T"),
            &o,
            &t,
            &intervals("O", "T"),
            &GRADES,
            &policy,
        )
        .unwrap();
        assert_eq!(got.state, state, "offset {offset} m");
        assert!(
            (got.separation_m - offset).abs() < 1e-9,
            "{} vs {offset}",
            got.separation_m
        );
        assert_eq!(got.limit_m, 5.0);
        assert!(got.pairs.iter().all(|p| !p.extrapolated));
    }

    // Collared 2 m east and dipping 85° east: 2.9 m apart at 11 m, 10.7 m at 101 m. Weak, by the trace.
    let (_, t) = straight("T", E0 + 2.0, 90.0, -85.0);
    let got = assess_twin(
        &pair("W", "O", "T"),
        &o,
        &t,
        &intervals("O", "T"),
        &GRADES,
        &policy,
    )
    .unwrap();
    let cos85 = 5.0_f64.to_radians().sin();
    let expect = |depth: f64| {
        let (east, down) = (
            2.0 + depth * cos85,
            depth - depth * (1.0 - cos85 * cos85).sqrt(),
        );
        (east * east + down * down).sqrt()
    };
    assert!(
        (got.pairs[0].separation_m - expect(11.0)).abs() < 1e-9,
        "{:?}",
        got.pairs[0]
    );
    assert!(
        (got.pairs[1].separation_m - expect(101.0)).abs() < 1e-9,
        "{:?}",
        got.pairs[1]
    );
    assert!(got.pairs[0].separation_m < 5.0 && got.separation_m > 10.0);
    assert_eq!(got.state, TwinState::WeakTwin);
    // A wider policy limit makes the same pair a twin.
    let wide = TwinPolicy {
        separation_limit_mm: 11_000,
    };
    let got = assess_twin(
        &pair("W", "O", "T"),
        &o,
        &t,
        &intervals("O", "T"),
        &GRADES,
        &wide,
    )
    .unwrap();
    assert_eq!(got.state, TwinState::Twin);
}

fn assess(grades: &[PairGrade]) -> fmine_drill::twin::TwinAssessment {
    let (_, o) = straight("O", E0, 0.0, -90.0);
    let (_, t) = straight("T", E0 + 1.0, 0.0, -90.0);
    let n = grades.len();
    let mut ivs = Vec::new();
    let mut matches = Vec::new();
    for i in 0..n {
        let from = 1_000 * u64::try_from(i).unwrap();
        for h in ["O", "T"] {
            ivs.push(interval(&format!("{h}-{i:03}"), h, from, from + 1_000));
        }
        matches.push(TwinMatch {
            original_interval_id: format!("O-{i:03}"),
            twin_interval_id: format!("T-{i:03}"),
        });
    }
    let p = TwinPair {
        matches,
        ..pair("W", "O", "T")
    };
    assess_twin(&p, &o, &t, &ivs, grades, &TwinPolicy::default()).unwrap()
}

fn g(original: f64, twin: f64) -> PairGrade {
    PairGrade { original, twin }
}

#[test]
fn test_cv_avr_rma_slope_and_the_paired_test_have_their_known_answers() {
    // CV_AVR% = 100·√((2/N)·Σ((a−b)/(a+b))²): terms 0.25, 0, 0.25 over N = 3; (0, 0) is left out.
    let got = assess(&[g(1.0, 3.0), g(2.0, 2.0), g(0.0, 0.0), g(5.0, 15.0)]);
    let p = got.precision.unwrap();
    assert!(
        (p.cv_avr_percent - 100.0 / 3.0_f64.sqrt()).abs() < 1e-12,
        "{p:?}"
    );
    assert_eq!((p.pairs_used, p.both_zero), (3, 1));
    assert_eq!(assess(&[g(0.0, 0.0)]).precision, None);

    // RMA slope of twin on original: exactly 2 for b = 2a + 1, −1 for b = 10 − a.
    let up = assess(&[g(1.0, 3.0), g(2.0, 5.0), g(3.0, 7.0), g(4.0, 9.0)]);
    assert_eq!(up.rma_slope, Some(2.0));
    let down = assess(&[g(1.0, 9.0), g(2.0, 8.0), g(3.0, 7.0), g(4.0, 6.0)]);
    assert_eq!(down.rma_slope, Some(-1.0));
    assert_eq!(
        assess(&[g(1.0, 2.0), g(1.0, 3.0)]).rma_slope,
        None,
        "original constant"
    );
    assert_eq!(assess(&[g(1.0, 2.0)]).rma_slope, None);

    // Differences 1..5: mean 3, sd √2.5, t = 3√2 on 4 df. Two-sided p from the closed-form t₄ CDF.
    let t = assess(&[
        g(1.0, 2.0),
        g(1.0, 3.0),
        g(1.0, 4.0),
        g(1.0, 5.0),
        g(1.0, 6.0),
    ])
    .paired
    .unwrap();
    assert_eq!((t.n, t.degrees_of_freedom), (5, 4));
    assert!((t.mean_difference - 3.0).abs() < 1e-15);
    assert!((t.sd_difference - 2.5_f64.sqrt()).abs() < 1e-15);
    assert!((t.t_statistic - 18.0_f64.sqrt()).abs() < 1e-12, "{t:?}");
    let x: f64 = 18.0;
    let u = 1.0 + x / 4.0;
    let cdf = 0.5 + 0.375 * (x.sqrt() / u.sqrt()) * (1.0 - x / (12.0 * u));
    assert!(
        (t.p_two_sided - 2.0 * (1.0 - cdf)).abs() < 1e-13,
        "{} vs {}",
        t.p_two_sided,
        2.0 * (1.0 - cdf)
    );
    // No difference at all, and a constant non-zero one.
    let same = assess(&[g(1.0, 1.0), g(2.0, 2.0)]).paired.unwrap();
    assert_eq!((same.t_statistic, same.p_two_sided), (0.0, 1.0));
    let shifted = assess(&[g(1.0, 1.5), g(2.0, 2.5)]).paired.unwrap();
    assert_eq!(
        (shifted.t_statistic, shifted.p_two_sided),
        (f64::INFINITY, 0.0)
    );
    assert_eq!(assess(&[g(1.0, 1.5)]).paired, None);
}

#[test]
fn test_an_assessment_refuses_traces_intervals_and_grades_that_do_not_belong() {
    let (_, o) = straight("O", E0, 0.0, -90.0);
    let (_, t) = straight("T", E0 + 3.0, 0.0, -90.0);
    let (_, x) = straight("X", E0 + 3.0, 0.0, -90.0);
    let ivs = intervals("O", "T");
    let policy = TwinPolicy::default();
    let field = |r: Result<_, DrillError>| match r {
        Err(DrillError::Invalid { field, .. }) => field,
        other => panic!("{other:?}"),
    };
    let p = pair("W", "O", "T");
    assert_eq!(
        field(assess_twin(&p, &o, &x, &ivs, &GRADES, &policy)),
        "twin.twin_hole_id"
    );
    assert_eq!(
        field(assess_twin(&p, &t, &o, &ivs, &GRADES, &policy)),
        "twin.original_hole_id"
    );
    assert_eq!(
        field(assess_twin(&p, &o, &t, &ivs, &GRADES[..1], &policy)),
        "twin.grades"
    );
    assert_eq!(
        field(assess_twin(&p, &o, &t, &ivs[..3], &GRADES, &policy)),
        "twin.matches"
    );
    let negative = [g(1.0, -0.1), g(1.0, 1.0)];
    assert_eq!(
        field(assess_twin(&p, &o, &t, &ivs, &negative, &policy)),
        "twin.grades"
    );
    let nan = [g(f64::NAN, 1.0), g(1.0, 1.0)];
    assert_eq!(
        field(assess_twin(&p, &o, &t, &ivs, &nan, &policy)),
        "twin.grades"
    );
    let selfie = pair("W", "O", "O");
    assert_eq!(
        field(assess_twin(&selfie, &o, &o, &ivs, &GRADES, &policy)),
        "twin.twin_hole_id"
    );
    // The twin's interval is in the original hole.
    let crossed = TwinPair {
        matches: vec![
            TwinMatch {
                original_interval_id: "O-I1".to_string(),
                twin_interval_id: "O-I2".to_string(),
            },
            TwinMatch {
                original_interval_id: "O-I2".to_string(),
                twin_interval_id: "T-I2".to_string(),
            },
        ],
        ..pair("W", "O", "T")
    };
    assert_eq!(
        field(assess_twin(&crossed, &o, &t, &ivs, &GRADES, &policy)),
        "twin.matches"
    );
}

const CELL: &str = "CELL-SITE(twin-test)";

struct Cell {
    ledger: L0Ledger,
    lease: fmine_ledger::cell::CellLease,
}

impl Cell {
    fn new() -> Self {
        let mut ledger = L0Ledger::new();
        let lease = ledger.create_cell(CELL, "node-1").unwrap();
        Self { ledger, lease }
    }

    fn write(&mut self, changes: &[Change]) -> Result<(), StoreError> {
        commit(&mut self.ledger, &self.lease, changes).map(|_| ())
    }
}

fn put(record: DrillRecord) -> Change {
    Change::Put(record)
}

fn acceptance(acceptance_id: &str, twin_ids: &[&str]) -> TwinAcceptance {
    TwinAcceptance {
        acceptance_id: acceptance_id.to_string(),
        twin_ids: twin_ids.iter().map(|s| (*s).to_string()).collect(),
        accepted_by_role: "QP (resource estimate)".to_string(),
        accepted_at_ns: 1_792_000_000_000_000_000,
        statement: "RC bias correction from the 2026 twin study enters the 2026 reported resource"
            .to_string(),
    }
}

/// Holes O1, O2 (RC) and their diamond twins T1, T2, with twin pairs W-1 and W-2.
fn twin_cell() -> Cell {
    let mut cell = Cell::new();
    let mut changes = Vec::new();
    for (h, kind) in [
        ("O1", HoleType::ReverseCirculation),
        ("O2", HoleType::ReverseCirculation),
        ("T1", HoleType::Diamond),
        ("T2", HoleType::Diamond),
    ] {
        changes.push(put(DrillRecord::Collar(hole(h, E0, kind))));
    }
    for (o, t) in [("O1", "T1"), ("O2", "T2")] {
        changes.extend(
            intervals(o, t)
                .into_iter()
                .map(|i| put(DrillRecord::Interval(i))),
        );
    }
    cell.write(&changes).unwrap();
    cell.write(&[
        put(DrillRecord::TwinPair(pair("W-1", "O1", "T1"))),
        put(DrillRecord::TwinPair(pair("W-2", "O2", "T2"))),
    ])
    .unwrap();
    cell
}

fn ids(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| (*s).to_string()).collect()
}

#[test]
fn test_a_twin_correction_enters_a_disclosure_basis_estimate_only_under_recorded_acceptances() {
    let mut cell = twin_cell();
    let both = ids(&["W-1", "W-2"]);
    let admit = |cell: &Cell, basis, acceptance_ids: &[String]| {
        admit_twin_correction(
            &cell.ledger,
            CELL,
            &TwinCorrection {
                basis,
                twin_ids: &both,
                acceptance_ids,
            },
        )
    };
    let refused_304 = |r: Result<Admission, StoreError>, why: &str| {
        let err = r.unwrap_err();
        assert!(
            matches!(&err, StoreError::Record(DrillError::MissingAcceptance { detail }) if detail.contains(why)),
            "{err}"
        );
        assert_eq!(
            err.to_refusal().code,
            RefusalCode::MissingHumanToken304.spec().code
        );
    };

    // A Full-basis internal projection takes it as it is.
    assert_eq!(
        admit(&cell, EstimateBasis::Full, &[]).unwrap(),
        Admission::Internal
    );
    // Public and disclosure bases need recorded acceptances of every pair.
    for basis in [EstimateBasis::Public, EstimateBasis::Disclosure] {
        refused_304(admit(&cell, basis, &[]), "none is cited");
        refused_304(
            admit(&cell, basis, &ids(&["A-9"])),
            "acceptance 'A-9' is not recorded",
        );
    }
    cell.write(&[put(DrillRecord::TwinAcceptance(acceptance(
        "A-1",
        &["W-1"],
    )))])
    .unwrap();
    refused_304(
        admit(&cell, EstimateBasis::Disclosure, &ids(&["A-1"])),
        "not accepted: W-2",
    );
    cell.write(&[put(DrillRecord::TwinAcceptance(acceptance(
        "A-2",
        &["W-2"],
    )))])
    .unwrap();
    let cited = ids(&["A-1", "A-2"]);
    for basis in [EstimateBasis::Public, EstimateBasis::Disclosure] {
        assert_eq!(
            admit(&cell, basis, &cited).unwrap(),
            Admission::Accepted {
                acceptance_ids: cited.clone()
            }
        );
    }

    // A correction from an unrecorded pair is refused on every basis.
    let stray = ids(&["W-1", "W-7"]);
    for basis in [EstimateBasis::Full, EstimateBasis::Disclosure] {
        let err = admit_twin_correction(
            &cell.ledger,
            CELL,
            &TwinCorrection {
                basis,
                twin_ids: &stray,
                acceptance_ids: &cited,
            },
        )
        .unwrap_err();
        assert!(
            matches!(&err, StoreError::Record(DrillError::Invalid { field, .. }) if field == "correction.twin_ids")
        );
        assert_eq!(
            err.to_refusal().code,
            RefusalCode::OutsidePhysicalDomain109.spec().code
        );
    }
}

#[test]
fn test_the_law_refuses_twins_of_unrecorded_or_wrong_records_and_moved_holes() {
    let mut cell = twin_cell();
    let refused = |cell: &mut Cell, changes: &[Change], why: &str| {
        let before = cell.ledger.committed_count(CELL).unwrap();
        let err = cell.write(changes).unwrap_err();
        assert!(err.to_string().contains(why), "{err}");
        assert_eq!(
            err.to_refusal().code,
            RefusalCode::Conservation101.spec().code
        );
        assert_eq!(
            cell.ledger.committed_count(CELL).unwrap(),
            before,
            "nothing written"
        );
    };
    refused(
        &mut cell,
        &[put(DrillRecord::TwinAcceptance(acceptance(
            "A-1",
            &["W-1", "W-9"],
        )))],
        "twin 'W-9', which is not recorded",
    );
    refused(
        &mut cell,
        &[put(DrillRecord::TwinPair(pair("W-3", "O1", "X")))],
        "collar 'X'",
    );
    // The twin side names an interval of the original hole.
    let crossed = TwinPair {
        matches: vec![TwinMatch {
            original_interval_id: "O1-I1".to_string(),
            twin_interval_id: "O2-I1".to_string(),
        }],
        ..pair("W-3", "O1", "T1")
    };
    refused(
        &mut cell,
        &[put(DrillRecord::TwinPair(crossed))],
        "interval 'O2-I1' is in hole 'O2', not 'T1'",
    );
    refused(
        &mut cell,
        &[Change::Supersede {
            record: DrillRecord::TwinPair(pair("W-1", "O1", "T2")),
            reason: "wrong twin".to_string(),
        }],
        "cannot change the holes it compares",
    );
    // A correction of the matches alone is a correction.
    let fewer = TwinPair {
        matches: pair("W-1", "O1", "T1").matches[..1].to_vec(),
        ..pair("W-1", "O1", "T1")
    };
    cell.write(&[Change::Supersede {
        record: DrillRecord::TwinPair(fewer),
        reason: "the 100 m interval was not twinned".to_string(),
    }])
    .unwrap();

    // The records round-trip through the codec.
    for r in [
        DrillRecord::TwinPair(pair("W-1", "O1", "T1")),
        DrillRecord::TwinAcceptance(acceptance("A-1", &["W-1", "W-2"])),
    ] {
        let frame = records::encode_version(&r, None).unwrap();
        assert_eq!(records::decode_version(&frame).unwrap().record, r);
    }
    let unsorted = TwinAcceptance {
        twin_ids: ids(&["W-2", "W-1"]),
        ..acceptance("A-3", &[])
    };
    assert!(DrillRecord::TwinAcceptance(unsorted).validate().is_err());
}

/// xorshift64*: deterministic noise in [−1, 1).
struct Noise(u64);

impl Noise {
    fn next(&mut self) -> f64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        let x = self.0.wrapping_mul(0x2545_F491_4F6C_DD1D) >> 11;
        #[allow(clippy::cast_precision_loss)]
        let u = x as f64 / (1_u64 << 53) as f64;
        2.0 * u - 1.0
    }
}

/// 1 m samples from 40 m to 100 m; the mineralized interval is 50–80 m.
fn rc_hole(grade_at: impl Fn(usize) -> f64) -> Vec<DownholeGrade> {
    (40..100_u64)
        .map(|m| DownholeGrade {
            span: Span {
                from_mm: m * 1_000,
                to_mm: (m + 1) * 1_000,
            },
            value: grade_at(usize::try_from(m).unwrap()),
        })
        .collect()
}

const ORE: Span = Span {
    from_mm: 50_000,
    to_mm: 80_000,
};

#[test]
fn test_the_smearing_monitor_fires_on_a_seeded_smeared_campaign_and_rarely_on_a_clean_one() {
    let rc = hole("RC-1", E0, HoleType::ReverseCirculation);
    let policy = SmearingPolicy::default();
    let mut noise = Noise(0x5EED_0285);
    let (mut fired_smeared, mut fired_clean) = (0, 0);
    const HOLES: usize = 400;
    for _ in 0..HOLES {
        // Smeared: a contact at 50 m, then a decaying tail carried down the hole.
        let tail: Vec<f64> = (0..100).map(|_| noise.next()).collect();
        let smeared = rc_hole(|m| {
            if (50..80).contains(&m) {
                let k = f64::from(u32::try_from(m - 50).unwrap());
                (6.0 * (-0.08 * k).exp() + 0.4) * (1.0 + 0.25 * tail[m])
            } else {
                0.05
            }
        });
        let report = smearing_monitor(&rc, &[ORE], &smeared, &policy).unwrap();
        assert_eq!(report.checks[0].n, 30);
        fired_smeared += usize::from(report.checks[0].state == SmearingState::Decline);

        let flat: Vec<f64> = (0..100).map(|_| noise.next()).collect();
        let clean = rc_hole(|m| {
            if (50..80).contains(&m) {
                2.0 * (1.0 + 0.5 * flat[m])
            } else {
                0.05
            }
        });
        let report = smearing_monitor(&rc, &[ORE], &clean, &policy).unwrap();
        fired_clean += usize::from(report.proposals().count() == 1);
    }
    // Measured on this seed: 400 of 400 smeared holes fire, 15 of 400 clean ones (3.75% at alpha 5%).
    assert!(
        fired_smeared >= HOLES * 99 / 100,
        "smeared: {fired_smeared}/{HOLES}"
    );
    assert!(
        fired_clean <= HOLES * 8 / 100,
        "clean: {fired_clean}/{HOLES}"
    );
    assert!(
        fired_clean > 0,
        "a 5% test that never fires on 400 holes is not testing"
    );
    println!("smearing monitor: smeared {fired_smeared}/{HOLES}, clean {fired_clean}/{HOLES}");
}

#[test]
fn test_the_smearing_statistic_has_its_known_answers_and_refuses_what_it_cannot_test() {
    let rc = hole("RC-1", E0, HoleType::ReverseCirculation);
    let policy = SmearingPolicy::default();
    // Strictly declining through ten samples in 50–60 m: S = −45, Var = 10·9·25/18 = 125, z = −44/√125.
    let falling = rc_hole(|m| {
        if (50..60).contains(&m) {
            f64::from(u32::try_from(70 - m).unwrap())
        } else {
            0.0
        }
    });
    let decade = Span {
        from_mm: 50_000,
        to_mm: 60_000,
    };
    let report = smearing_monitor(&rc, &[decade], &falling, &policy).unwrap();
    let c = report.checks[0];
    assert_eq!(
        (c.n, c.kendall_s, c.state),
        (10, -45, SmearingState::Decline)
    );
    assert!(
        (c.z.unwrap() + 44.0 / 125.0_f64.sqrt()).abs() < 1e-12,
        "{c:?}"
    );
    // Constant: S = 0, no variance left after the ties, no decline.
    let flat = rc_hole(|_| 1.5);
    let c = smearing_monitor(&rc, &[decade], &flat, &policy)
        .unwrap()
        .checks[0];
    assert_eq!(
        (c.kendall_s, c.z, c.p_decline, c.state),
        (0, Some(0.0), Some(0.5), SmearingState::NoDecline)
    );
    // Ties: 3, 1, 2, 2 gives S = −1, Var = (4·3·13 − 2·1·9)/18; the continuity correction makes z = 0.
    let four = Span {
        from_mm: 50_000,
        to_mm: 54_000,
    };
    let tied = rc_hole(|m| {
        [3.0, 1.0, 2.0, 2.0]
            .get(m.wrapping_sub(50))
            .copied()
            .unwrap_or(0.0)
    });
    let small = SmearingPolicy {
        min_samples: 3,
        ..policy
    };
    let c = smearing_monitor(&rc, &[four], &tied, &small)
        .unwrap()
        .checks[0];
    assert_eq!((c.n, c.kendall_s, c.z), (4, -1, Some(0.0)));
    // Too few with the default policy; samples crossing the interval's edge are not inside it.
    let c = smearing_monitor(&rc, &[four], &tied, &policy)
        .unwrap()
        .checks[0];
    assert_eq!((c.n, c.z, c.state), (4, None, SmearingState::TooFewSamples));
    let straddle = Span {
        from_mm: 50_500,
        to_mm: 54_500,
    };
    assert_eq!(
        smearing_monitor(&rc, &[straddle], &tied, &policy)
            .unwrap()
            .checks[0]
            .n,
        3
    );

    let field = |r: Result<_, DrillError>| match r {
        Err(DrillError::Invalid { field, .. }) => field,
        other => panic!("{other:?}"),
    };
    let dd = hole("DD-1", E0, HoleType::Diamond);
    assert_eq!(
        field(smearing_monitor(&dd, &[ORE], &flat, &policy)),
        "smearing.hole_type"
    );
    for bad in [
        SmearingPolicy {
            alpha: 0.0,
            ..policy
        },
        SmearingPolicy {
            alpha: 0.6,
            ..policy
        },
        SmearingPolicy {
            min_samples: 2,
            ..policy
        },
    ] {
        assert_eq!(
            field(smearing_monitor(&rc, &[ORE], &flat, &bad)),
            "smearing.policy"
        );
    }
    let mut overlapping = flat.clone();
    overlapping[1].span.from_mm -= 500;
    assert_eq!(
        field(smearing_monitor(&rc, &[ORE], &overlapping, &policy)),
        "smearing.grades"
    );
    assert_eq!(
        field(smearing_monitor(&rc, &[ORE, decade], &flat, &policy)),
        "smearing.mineralized"
    );
    let mut negative = flat;
    negative[3].value = -1.0;
    assert_eq!(
        field(smearing_monitor(&rc, &[ORE], &negative, &policy)),
        "smearing.grades"
    );
}

#[test]
fn test_a_twin_separation_carries_the_two_holes_survey_error() {
    // Parallel holes dipping 60° to grid north, the twin offset east. A dip error moves a point north and up; an
    // azimuth error moves it east by cos 60° per radian, times the station weights at 41 m of stations every
    // 25 m: 12.5, 12.5 + f·h·(2 − f)/2 and f²·h/2 with h = 25 and f = 0.64.
    let depths = [0, 25_000, 50_000, 75_000, 100_000];
    let surveys = |hole_id: &str, dip: f64| -> Vec<Survey> {
        depths
            .iter()
            .map(|&d| station(hole_id, d, 0.0, dip))
            .collect()
    };
    let (h, f) = (25.0_f64, 0.64_f64);
    let w = [12.5, 12.5 + f * h * (2.0 - f) / 2.0, f * f * h / 2.0];
    let w2: f64 = w.iter().map(|x| x * x).sum();
    let sigma_az = 0.5_f64.to_radians();
    let per_hole = 0.05_f64.powi(2) + sigma_az * sigma_az * 0.25 * w2;
    let want_sigma = (2.0 * per_hole).sqrt();

    let original = hole("O", E0, HoleType::Diamond);
    let original_surveys = surveys("O", -60.0);
    let o = propagate(&original, &original_surveys, &ErrorModel::random()).unwrap();
    for offset in [2.0, 4.9, 5.2] {
        let twin = hole("T", E0 + offset, HoleType::Diamond);
        let twin_surveys = surveys("T", -60.0);
        let ivs = vec![
            interval("O-I1", "O", 40_000, 42_000),
            interval("T-I1", "T", 40_000, 42_000),
        ];
        let record = TwinPair {
            twin_id: "TW-1".to_string(),
            original_hole_id: "O".to_string(),
            twin_hole_id: "T".to_string(),
            matches: vec![TwinMatch {
                original_interval_id: "O-I1".to_string(),
                twin_interval_id: "T-I1".to_string(),
            }],
        };
        let assessment = assess_twin(
            &record,
            &desurvey(&original, &original_surveys).unwrap(),
            &desurvey(&twin, &twin_surveys).unwrap(),
            &ivs,
            &[PairGrade {
                original: 1.0,
                twin: 1.1,
            }],
            &TwinPolicy::default(),
        )
        .unwrap();
        let t = propagate(&twin, &twin_surveys, &ErrorModel::random()).unwrap();
        let u = separation_uncertainty(&assessment, &o, &t, &ivs).unwrap();
        assert_eq!(u.len(), 1);
        let got = &u[0];
        assert!((got.separation_m - offset).abs() < 1e-9, "{got:?}");
        assert!(
            (got.sigma_m - want_sigma).abs() < 1e-9 * want_sigma,
            "{} vs {want_sigma}",
            got.sigma_m
        );
        let z = (got.separation_m - 5.0) / got.sigma_m;
        if z > -3.0 {
            assert_eq!(got.p_beyond_limit, fmine_core::det::norm_cdf(z));
        } else {
            assert!(
                got.p_beyond_limit > 0.0 && got.p_beyond_limit < 1e-50,
                "{got:?}"
            );
        }
        println!(
            "offset {offset} m: {:?}, sigma {:.4} m, P(beyond 5 m) {:.4e}",
            assessment.state, got.sigma_m, got.p_beyond_limit
        );
        if offset == 4.9 {
            // A twin by the limit, with even odds of being weak: 4.9 m is 0.55 sigma inside it.
            assert_eq!(assessment.state, TwinState::Twin);
            assert!(got.p_beyond_limit > 0.25 && got.p_beyond_limit < 0.35);
        }

        // An assessment made from other surveys, or intervals of another hole, are refused.
        let steeper = propagate(&twin, &surveys("T", -61.0), &ErrorModel::random()).unwrap();
        let field = |e: DrillError| match e {
            DrillError::Invalid { field, .. } => field,
            other => panic!("{other:?}"),
        };
        assert_eq!(
            field(separation_uncertainty(&assessment, &o, &steeper, &ivs).unwrap_err()),
            "twin.separation"
        );
        assert_eq!(
            field(separation_uncertainty(&assessment, &t, &o, &ivs).unwrap_err()),
            "twin.matches"
        );
    }
}
