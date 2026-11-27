//! Campaign data checks (`BD-286`'s done-when: each check fires on a seeded synthetic campaign). One seeded
//! campaign of three drilling programs carries one planted defect per check. Each check finds its own defect and
//! nothing else in the campaign. Across seeds, the two statistical checks hold their significance level on clean
//! groups and find their defects. The survey and collar checks have hand-worked answers, and the law refuses a
//! dispatch of material discarded after QC.

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::float_cmp
)]

use fmine_drill::checks::{
    CheckPolicy, CollarDtmState, ComparisonState, DependenceState, DrillIndex, DtmElevation,
    FlaggedEntry, IntervalGrade, Reassay, RegimeState, RunState, collar_dtm, compare_campaigns,
    qaqc_regimes, recovery_grade, select_for_estimate, survey_runs,
};
use fmine_drill::records::{
    self, AssayResult, AssayValue, BatchFlag, Collar, DrillRecord, HoleType, Interval, Orientation,
    QaqcRegime, Recovery, RecoveryMethod, RunDirection, Sample, SampleDiscard, Survey,
};
use fmine_drill::store::{Change, commit};
use fmine_evidence::RefusalCode;
use fmine_ledger::l0::L0Ledger;

#[allow(dead_code)]
#[path = "common/fixtures.rs"]
mod fixtures;

use fixtures::{collar, dispatch, interval, position, qc_material, result, sample, survey};

/// xorshift64* with Box–Muller normals, its state a splitmix64 hash of the seed: nearby seeds give unrelated
/// campaigns, and no seed gives the all-zero state.
struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        let mut z = seed.wrapping_add(0x9E37_79B9_7F4A_7C15);
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        Self((z ^ (z >> 31)).max(1))
    }

    fn uniform(&mut self) -> f64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        let x = self.0.wrapping_mul(0x2545_F491_4F6C_DD1D) >> 11;
        #[allow(clippy::cast_precision_loss)]
        let u = (x as f64 + 0.5) / (1_u64 << 53) as f64;
        u
    }

    fn normal(&mut self) -> f64 {
        let (u, v) = (self.uniform(), self.uniform());
        (-2.0 * u.ln()).sqrt() * (std::f64::consts::TAU * v).cos()
    }
}

const PROGRAMS: [(&str, HoleType, &str); 3] = [
    ("2019 RC", HoleType::ReverseCirculation, "R"),
    ("2024 DD", HoleType::Diamond, "D"),
    ("2025 DD", HoleType::Diamond, "E"),
];
const HOLES: usize = 12;
const METRES: u64 = 60;
const BATCH: usize = 40;
const DISCARDED_AT: i64 = 1_791_500_000_000_000_000;

struct Campaign {
    collars: Vec<Collar>,
    surveys: Vec<Survey>,
    intervals: Vec<Interval>,
    samples: Vec<Sample>,
    results: Vec<AssayResult>,
    grades: Vec<IntervalGrade>,
    dtm: Vec<DtmElevation>,
    flags: Vec<BatchFlag>,
    discards: Vec<SampleDiscard>,
}

fn station(
    hole_id: &str,
    run: RunDirection,
    depth_mm: u64,
    azimuth_deg: f64,
    dip_deg: f64,
) -> Survey {
    let tag = if run == RunDirection::In { "in" } else { "out" };
    Survey {
        run,
        orientation: Some(Orientation {
            azimuth_deg,
            dip_deg,
            sigma_azimuth_deg: 0.5,
            sigma_dip_deg: 0.25,
        }),
        ..survey(&format!("{hole_id}-{tag}-{depth_mm}"), hole_id, depth_mm)
    }
}

/// Three programs of twelve 60 m holes, 1 m intervals, one sample and one result each. Planted:
/// - recovery that follows grade in the 2024 DD fresh rock (`FR`), with that grade distribution untouched;
/// - 2025 DD oxide (`OX`) grades 1.6 times the others';
/// - hole D07's out-run 3° steeper than its in-run from 40 m;
/// - hole D03's collar 3.5 m above the DTM;
/// - 2019 RC holes R10–R12 assayed under no QA/QC regime, the rest of that program under a partial one;
/// - lab batches B-0002 (2019 RC, material discarded after QC) and B-0030 (2024 DD) committed flagged.
fn campaign(seed: u64) -> Campaign {
    let mut rng = Rng::new(seed);
    let mut c = Campaign {
        collars: Vec::new(),
        surveys: Vec::new(),
        intervals: Vec::new(),
        samples: Vec::new(),
        results: Vec::new(),
        grades: Vec::new(),
        dtm: Vec::new(),
        flags: Vec::new(),
        discards: Vec::new(),
    };
    for (program, hole_type, prefix) in PROGRAMS {
        for h in 1..=HOLES {
            let hole_id = format!("{prefix}{h:02}");
            #[allow(clippy::cast_precision_loss)]
            let easting = 421_000.0 + 50.0 * h as f64;
            c.collars.push(Collar {
                hole_type,
                program: program.to_string(),
                ..Collar {
                    actual: Some(position(easting)),
                    ..collar(&hole_id)
                }
            });
            let residual = if hole_id == "D03" {
                3.5
            } else {
                0.15 * rng.normal()
            };
            c.dtm.push(DtmElevation {
                elevation_m: 1_312.5 - residual,
                sigma_m: 0.3,
            });
            let runs: &[RunDirection] = if hole_type == HoleType::Diamond {
                &[RunDirection::In, RunDirection::Out]
            } else {
                &[RunDirection::In]
            };
            for &run in runs {
                for depth in [0, 20_000, 40_000, 60_000] {
                    let mut dip = -60.0 + 0.125 * rng.normal();
                    let azimuth = 90.0 + 0.25 * rng.normal();
                    if hole_id == "D07" && run == RunDirection::Out && depth >= 40_000 {
                        dip -= 3.0;
                    }
                    c.surveys.push(station(&hole_id, run, depth, azimuth, dip));
                }
            }
            for m in 0..METRES {
                let interval_id = format!("{hole_id}-{m:02}");
                let domain = if m < 30 { "OX" } else { "FR" };
                let mut grade = (2.0_f64.ln() + 0.6 * rng.normal()).exp();
                if program == "2025 DD" && domain == "OX" {
                    grade *= 1.6;
                }
                let (low, method) = if hole_type == HoleType::Diamond {
                    (60.0, RecoveryMethod::Length)
                } else {
                    (70.0, RecoveryMethod::Mass)
                };
                let follows = program == "2024 DD" && domain == "FR";
                let u = if follows {
                    let z = (grade.ln() - 2.0_f64.ln()) / 0.6;
                    0.7 / (1.0 + (-1.7 * z).exp()) + 0.3 * rng.uniform()
                } else {
                    rng.uniform()
                };
                let recovery = match m {
                    22 | 45 => None,
                    28 | 57 => Some(Recovery {
                        percent: 90.0,
                        method: RecoveryMethod::Unknown,
                    }),
                    _ => Some(Recovery {
                        percent: low + (100.0 - low) * u,
                        method,
                    }),
                };
                c.intervals.push(Interval {
                    recovery,
                    ..interval(&interval_id, &hole_id, m * 1_000, (m + 1) * 1_000)
                });
                c.grades.push(IntervalGrade {
                    interval_id: interval_id.clone(),
                    domain: domain.to_string(),
                    grade,
                });
                let sample_id = format!("S-{interval_id}");
                c.samples.push(sample(&sample_id, &[&interval_id]));
                let regime = match (program, h) {
                    ("2019 RC", 10..) => QaqcRegime::None,
                    ("2019 RC", _) => QaqcRegime::Partial,
                    _ => QaqcRegime::Current,
                };
                let n = c.results.len();
                c.results.push(AssayResult {
                    analyte: "Au".to_string(),
                    value: AssayValue::Measured {
                        mass_fraction: grade * 1e-6,
                    },
                    lab_batch: format!("B-{:04}", n / BATCH),
                    qaqc_regime: regime,
                    ..result(
                        &format!("R-{interval_id}"),
                        &sample_id,
                        "D-01",
                        "LAB-A",
                        None,
                    )
                });
                if program == "2019 RC" {
                    c.discards.push(SampleDiscard {
                        sample_id,
                        discarded_at_ns: DISCARDED_AT,
                        reason: "originals discarded after QC (program retention rule)".to_string(),
                    });
                }
            }
        }
    }
    for q in 1..=10 {
        let sample_id = format!("QC-{q:02}");
        c.samples.push(qc_material(&sample_id));
        let n = c.results.len();
        c.results.push(AssayResult {
            lab_batch: format!("B-{:04}", n / BATCH),
            ..result(&format!("R-{sample_id}"), &sample_id, "D-01", "LAB-A", None)
        });
    }
    for (flag_id, lab_batch) in [("F-1", "B-0002"), ("F-2", "B-0030")] {
        c.flags.push(BatchFlag {
            flag_id: flag_id.to_string(),
            lab: "LAB-A".to_string(),
            lab_batch: lab_batch.to_string(),
            finding: "CRM OREAS-235 failed at 3 SD (sigma_C); batch committed without re-assay"
                .to_string(),
            flagged_at_ns: 1_791_000_000_000_000_000,
        });
    }
    c
}

#[test]
fn test_each_check_fires_on_its_planted_defect_in_a_seeded_campaign() {
    let c = campaign(0x0286_5EED);
    let index = DrillIndex::new(&c.collars, &c.intervals, &c.samples).unwrap();
    let policy = CheckPolicy::default();

    // Recovery–grade: the planted group depends, far beyond the 5% level, and its intervals are Conflicted. A
    // clean group can fall below 5% by chance; the across-seeds test measures how often.
    let rg = recovery_grade(&index, &c.grades, &policy).unwrap();
    assert_eq!(rg.checks.len(), 6);
    for g in &rg.checks {
        println!(
            "recovery-grade {} {} {:?}: rho {:?} p {:?} {:?}",
            g.domain, g.campaign, g.hole_type, g.rho, g.p_two_sided, g.state
        );
    }
    let planted = rg
        .checks
        .iter()
        .find(|g| g.domain == "FR" && g.campaign == "2024 DD")
        .unwrap();
    assert_eq!(
        (planted.hole_type, planted.recovery_method, planted.state),
        (
            HoleType::Diamond,
            RecoveryMethod::Length,
            DependenceState::Dependent
        )
    );
    assert!(
        planted.rho.unwrap() > 0.5 && planted.p_two_sided.unwrap() < 1e-12,
        "{planted:?}"
    );
    // 30 fresh-rock metres a hole, less the two whose recovery is missing or of unknown method.
    assert_eq!(planted.interval_ids.len(), 12 * 28);
    assert!(rg.conflicted().filter(|id| id.starts_with('D')).count() >= 12 * 28);
    assert!(
        planted
            .interval_ids
            .iter()
            .all(|id| rg.conflicted().any(|c| c == id))
    );
    assert_eq!(
        rg.unmeasured.len(),
        36 * 4,
        "no recovery, or recovery of unknown method"
    );

    // Data generation: the biased campaign differs from both others in its domain, and only there.
    let cmp = compare_campaigns(&index, &c.grades, &policy).unwrap();
    assert_eq!(cmp.comparisons.len(), 6);
    for k in &cmp.comparisons {
        println!(
            "compare {} {} vs {}: means {:.3} {:.3} p {:?} {:?}",
            k.domain, k.campaign, k.other, k.mean.0, k.mean.1, k.p_two_sided, k.state
        );
        assert_eq!(k.qq.len(), 9);
    }
    assert_eq!(
        cmp.outliers(),
        vec![("OX".to_string(), "2025 DD".to_string())]
    );
    // On this seed two clean programs differ by chance in fresh rock; neither differs from every other program,
    // so neither is named.
    let chance = cmp
        .comparisons
        .iter()
        .find(|k| k.domain == "FR" && k.campaign == "2019 RC" && k.other == "2025 DD")
        .unwrap();
    assert_eq!(chance.state, ComparisonState::Differs);

    // Survey runs: D07 is inconsistent; the RC holes have no out-run.
    let mut states = Vec::new();
    for collar in &c.collars {
        let surveys: Vec<Survey> = c
            .surveys
            .iter()
            .filter(|s| s.hole_id == collar.hole_id)
            .cloned()
            .collect();
        let check = survey_runs(collar, &surveys, &policy).unwrap();
        states.push((collar.hole_id.clone(), check.state));
        if collar.hole_id == "D07" {
            let worst = check.pairs.iter().map(|p| p.ratio).fold(0.0, f64::max);
            println!(
                "D07: worst ratio {worst:.2}, runs {:.3} m apart at {:?} mm",
                check.separation_m.unwrap(),
                check.at_depth_mm
            );
            assert!(check.separation_m.unwrap() > 0.5);
        }
    }
    let inconsistent: Vec<&str> = states
        .iter()
        .filter(|(_, s)| *s == RunState::Inconsistent)
        .map(|(h, _)| h.as_str())
        .collect();
    assert_eq!(inconsistent, vec!["D07"]);
    assert_eq!(
        states
            .iter()
            .filter(|(_, s)| *s == RunState::NoPairedStations)
            .count(),
        HOLES
    );

    // Collar against DTM: D03 only.
    let off: Vec<&str> = c
        .collars
        .iter()
        .zip(&c.dtm)
        .filter(|(collar, dtm)| {
            collar_dtm(collar, dtm, &policy).unwrap().state == CollarDtmState::Disagrees
        })
        .map(|(collar, _)| collar.hole_id.as_str())
        .collect();
    assert_eq!(off, vec!["D03"]);

    // Regimes: the RC program mixes two; QC material belongs to no campaign.
    let regimes = qaqc_regimes(&index, &c.results).unwrap();
    let mixed: Vec<_> = regimes
        .checks
        .iter()
        .filter(|r| r.state == RegimeState::Mixed)
        .collect();
    assert_eq!(mixed.len(), 1);
    assert_eq!(mixed[0].campaign, "2019 RC");
    assert_eq!(
        mixed[0].counts,
        vec![(QaqcRegime::None, 3 * 60), (QaqcRegime::Partial, 9 * 60)]
    );
    assert_eq!(regimes.without_campaign, 10);

    // CommittedFlagged: both batches' results are flagged whichever way the estimate decides; B-0002's material
    // was discarded, so those cannot be re-assayed.
    let total = c.results.len();
    let out = select_for_estimate(&c.results, &c.flags, &c.discards, FlaggedEntry::Exclude);
    let into = select_for_estimate(&c.results, &c.flags, &c.discards, FlaggedEntry::Include);
    assert_eq!(
        (out.flagged.len(), into.flagged.len()),
        (2 * BATCH, 2 * BATCH)
    );
    assert_eq!(
        (out.included.len(), into.included.len()),
        (total - 2 * BATCH, total)
    );
    let impossible = out
        .flagged
        .iter()
        .filter(|f| matches!(f.reassay, Reassay::Impossible { discarded_at_ns } if discarded_at_ns == DISCARDED_AT))
        .count();
    assert_eq!(impossible, BATCH);
    assert!(
        out.flagged
            .iter()
            .filter(|f| f.flag_id == "F-2")
            .all(|f| f.reassay == Reassay::NoDiscardRecorded)
    );
}

#[test]
fn test_the_statistical_checks_hold_their_level_and_find_their_defects_across_seeds() {
    const SEEDS: u64 = 60;
    let policy = CheckPolicy::default();
    let (mut clean_rg, mut clean_rg_fired, mut planted_rg_fired) = (0, 0, 0);
    let (mut clean_cmp, mut clean_cmp_fired, mut biased_cmp, mut biased_cmp_fired) = (0, 0, 0, 0);
    for seed in 0..SEEDS {
        let c = campaign(0xC0FF_EE00 + seed);
        let index = DrillIndex::new(&c.collars, &c.intervals, &c.samples).unwrap();
        for g in recovery_grade(&index, &c.grades, &policy).unwrap().checks {
            let fired = usize::from(g.state == DependenceState::Dependent);
            if g.domain == "FR" && g.campaign == "2024 DD" {
                planted_rg_fired += fired;
            } else {
                clean_rg += 1;
                clean_rg_fired += fired;
            }
        }
        for k in compare_campaigns(&index, &c.grades, &policy)
            .unwrap()
            .comparisons
        {
            let fired = usize::from(k.state == ComparisonState::Differs);
            if k.domain == "OX" && k.other == "2025 DD" {
                biased_cmp += 1;
                biased_cmp_fired += fired;
            } else {
                clean_cmp += 1;
                clean_cmp_fired += fired;
            }
        }
    }
    println!(
        "recovery-grade: planted {planted_rg_fired}/{SEEDS}, clean {clean_rg_fired}/{clean_rg}; \
         comparison: biased {biased_cmp_fired}/{biased_cmp}, clean {clean_cmp_fired}/{clean_cmp}"
    );
    let seeds = usize::try_from(SEEDS).unwrap();
    assert_eq!(planted_rg_fired, seeds);
    assert_eq!(biased_cmp_fired, biased_cmp);
    // A 5% test on clean groups: well under 10% and not never.
    assert!(
        clean_rg_fired * 10 < clean_rg && clean_rg_fired > 0,
        "{clean_rg_fired}/{clean_rg}"
    );
    assert!(
        clean_cmp_fired * 10 < clean_cmp && clean_cmp_fired > 0,
        "{clean_cmp_fired}/{clean_cmp}"
    );
}

fn orient(azimuth_deg: f64, dip_deg: f64) -> Option<Orientation> {
    Some(Orientation {
        azimuth_deg,
        dip_deg,
        sigma_azimuth_deg: 0.5,
        sigma_dip_deg: 0.25,
    })
}

#[test]
fn test_the_survey_and_collar_checks_have_their_hand_worked_answers() {
    let policy = CheckPolicy::default();
    let hole = collar("H1");
    let pair = |dip_in: f64, dip_out: f64, az_in: f64, az_out: f64| {
        let surveys = [
            Survey {
                orientation: orient(az_in, dip_in),
                run: RunDirection::In,
                ..survey("in-50", "H1", 50_000)
            },
            Survey {
                orientation: orient(az_out, dip_out),
                run: RunDirection::Out,
                ..survey("out-50", "H1", 50_000)
            },
        ];
        survey_runs(&hole, &surveys, &policy).unwrap()
    };
    // Dip limit 3·√(0.25² + 0.25²) = 1.0607°.
    let ok = pair(-60.0, -59.0, 90.0, 90.0);
    assert!((ok.pairs[0].ratio - 1.0 / (3.0 * 0.125_f64.sqrt())).abs() < 1e-12);
    assert_eq!(ok.state, RunState::Consistent);
    assert_eq!(pair(-60.0, -58.9, 90.0, 90.0).state, RunState::Inconsistent);
    // Azimuth wraps through north: 359.5° to 0.3° is 0.8°; the limit is 3·√(0.5² + 0.5²) = 2.12°.
    let wrap = pair(-60.0, -60.0, 359.5, 0.3);
    assert!((wrap.pairs[0].azimuth_difference_deg.unwrap() - 0.8).abs() < 1e-9);
    assert_eq!(pair(-60.0, -60.0, 359.5, 2.5).state, RunState::Inconsistent);
    // Near vertical the azimuth says nothing and is not compared.
    let steep = pair(-88.0, -88.0, 10.0, 200.0);
    assert_eq!(
        (steep.pairs[0].azimuth_difference_deg, steep.state),
        (None, RunState::Consistent)
    );

    // Runs: vertical going in; coming out, 1° off vertical at 100 m. Minimum curvature from 0° to 1° over
    // 100 m: horizontal L·(1 − cos θ)/θ and vertical shortfall L·(1 − sin θ/θ).
    let surveys = [
        Survey {
            orientation: orient(0.0, -90.0),
            run: RunDirection::In,
            ..survey("i0", "H1", 0)
        },
        Survey {
            orientation: orient(0.0, -90.0),
            run: RunDirection::In,
            ..survey("i100", "H1", 100_000)
        },
        Survey {
            orientation: orient(0.0, -90.0),
            run: RunDirection::Out,
            ..survey("o0", "H1", 0)
        },
        Survey {
            orientation: orient(0.0, -89.0),
            run: RunDirection::Out,
            ..survey("o100", "H1", 100_000)
        },
    ];
    let runs = survey_runs(&hole, &surveys, &policy).unwrap();
    let theta = 1.0_f64.to_radians();
    let (h, v) = (
        100.0 * (1.0 - theta.cos()) / theta,
        100.0 * (1.0 - theta.sin() / theta),
    );
    assert_eq!(runs.at_depth_mm, Some(100_000));
    assert!(
        (runs.separation_m.unwrap() - h.hypot(v)).abs() < 1e-9,
        "{runs:?} vs {}",
        h.hypot(v)
    );
    assert_eq!(runs.state, RunState::Consistent, "1° against a 1.06° limit");

    // Collar against DTM: limit 3·√(0.1² + 0.3²) = 0.9487 m.
    let elevation = hole.actual.as_ref().unwrap().elevation_m;
    let at = |residual: f64| {
        collar_dtm(
            &hole,
            &DtmElevation {
                elevation_m: elevation - residual,
                sigma_m: 0.3,
            },
            &policy,
        )
        .unwrap()
    };
    let near = at(0.9);
    assert!((near.limit_m.unwrap() - 3.0 * 0.1_f64.hypot(0.3)).abs() < 1e-12);
    assert_eq!(
        (near.state, at(-1.0).state),
        (CollarDtmState::Agrees, CollarDtmState::Disagrees)
    );
    let planned_only = Collar {
        actual: None,
        ..collar("H2")
    };
    let dtm = DtmElevation {
        elevation_m: elevation,
        sigma_m: 0.3,
    };
    assert_eq!(
        collar_dtm(&planned_only, &dtm, &policy).unwrap().state,
        CollarDtmState::NoActualPosition
    );

    let field = |e: records::DrillError| match e {
        records::DrillError::Invalid { field, .. } => field,
        other => panic!("{other:?}"),
    };
    let bad_dtm = DtmElevation {
        sigma_m: -0.1,
        ..dtm
    };
    assert_eq!(
        field(collar_dtm(&hole, &bad_dtm, &policy).unwrap_err()),
        "checks.dtm"
    );
    assert_eq!(
        field(survey_runs(&collar("H2"), &surveys, &policy).unwrap_err()),
        "checks.surveys"
    );
    let bad = CheckPolicy {
        sigma_multiple: 0.0,
        ..policy
    };
    assert_eq!(
        field(collar_dtm(&hole, &dtm, &bad).unwrap_err()),
        "checks.policy"
    );
}

#[test]
fn test_the_index_and_grades_refuse_what_does_not_belong() {
    let c = campaign(7);
    let field = |e: records::DrillError| match e {
        records::DrillError::Invalid { field, .. } => field,
        other => panic!("{other:?}"),
    };
    let mut twice = c.collars.clone();
    twice.push(c.collars[0].clone());
    assert_eq!(
        field(DrillIndex::new(&twice, &c.intervals, &c.samples).unwrap_err()),
        "checks.collars"
    );
    assert_eq!(
        field(DrillIndex::new(&c.collars[1..], &c.intervals, &c.samples).unwrap_err()),
        "checks.intervals"
    );
    let mut across = c.samples.clone();
    across[0].interval_ids = vec!["D01-00".to_string(), "R01-01".to_string()];
    assert_eq!(
        field(DrillIndex::new(&c.collars, &c.intervals, &across).unwrap_err()),
        "checks.samples"
    );

    let index = DrillIndex::new(&c.collars, &c.intervals, &c.samples).unwrap();
    let policy = CheckPolicy::default();
    let mut grades = c.grades.clone();
    grades.push(grades[0].clone());
    assert_eq!(
        field(recovery_grade(&index, &grades, &policy).unwrap_err()),
        "checks.grades"
    );
    let mut grades = c.grades.clone();
    grades[5].grade = f64::NAN;
    assert_eq!(
        field(compare_campaigns(&index, &grades, &policy).unwrap_err()),
        "checks.grades"
    );
    let mut grades = c.grades.clone();
    grades[5].interval_id = "X99-00".to_string();
    assert_eq!(
        field(recovery_grade(&index, &grades, &policy).unwrap_err()),
        "checks.grades"
    );
    let mut stray = c.results.clone();
    stray[0].sample_id = "S-X99".to_string();
    assert_eq!(
        field(qaqc_regimes(&index, &stray).unwrap_err()),
        "checks.results"
    );
}

const CELL: &str = "CELL-SITE(retention-test)";

#[test]
fn test_the_law_refuses_a_dispatch_of_material_discarded_after_qc() {
    let mut ledger = L0Ledger::new();
    let lease = ledger.create_cell(CELL, "node-1").unwrap();
    let put = |r: DrillRecord| Change::Put(r);
    let sent = 1_790_000_400_000_000_000;
    let discard = |sample_id: &str, at: i64| SampleDiscard {
        sample_id: sample_id.to_string(),
        discarded_at_ns: at,
        reason: "originals discarded after QC".to_string(),
    };
    commit(
        &mut ledger,
        &lease,
        &[
            put(DrillRecord::Collar(collar("H1"))),
            put(DrillRecord::Interval(interval("H1-I1", "H1", 0, 1_500))),
            put(DrillRecord::Sample(sample("S-1", &["H1-I1"]))),
            put(DrillRecord::Sample(sample("S-2", &["H1-I1"]))),
            put(DrillRecord::Sample(sample("S-3", &["H1-I1"]))),
            put(DrillRecord::Dispatch(dispatch("D-01", "LAB-A", &["S-1"]))),
        ],
    )
    .unwrap();
    commit(
        &mut ledger,
        &lease,
        &[put(DrillRecord::SampleDiscard(discard(
            "S-1",
            sent + 1_000,
        )))],
    )
    .unwrap();
    let refused = |ledger: &mut L0Ledger, changes: &[Change]| {
        let before = ledger.committed_count(CELL).unwrap();
        let err = commit(ledger, &lease, changes).unwrap_err();
        assert!(err.to_string().contains("was discarded after QC"), "{err}");
        assert_eq!(
            err.to_refusal().code,
            RefusalCode::Conservation101.spec().code
        );
        assert_eq!(ledger.committed_count(CELL).unwrap(), before);
    };
    // A later dispatch of the discarded sample.
    let later = |id: &str, samples: &[&str]| records::Dispatch {
        dispatched_at_ns: sent + 2_000,
        ..dispatch(id, "LAB-A", samples)
    };
    refused(
        &mut ledger,
        &[put(DrillRecord::Dispatch(later("D-02", &["S-1", "S-2"])))],
    );
    // A discard and a dispatch after it in one batch.
    refused(
        &mut ledger,
        &[
            put(DrillRecord::SampleDiscard(discard("S-3", sent))),
            put(DrillRecord::Dispatch(later("D-03", &["S-3"]))),
        ],
    );
    // The dispatch that sent it before the discard can be corrected: it keeps the sample it already listed.
    commit(
        &mut ledger,
        &lease,
        &[Change::Supersede {
            record: DrillRecord::Dispatch(dispatch("D-01", "LAB-A", &["S-1", "S-2"])),
            reason: "S-2 was in the same box".to_string(),
        }],
    )
    .unwrap();
    // A dispatch dated before the discard, recorded late, is admitted.
    commit(
        &mut ledger,
        &lease,
        &[put(DrillRecord::Dispatch(dispatch(
            "D-04",
            "LAB-A",
            &["S-1"],
        )))],
    )
    .unwrap();
    commit(
        &mut ledger,
        &lease,
        &[put(DrillRecord::Dispatch(later("D-05", &["S-2"])))],
    )
    .unwrap();

    // A discard names a recorded sample; the new records round-trip and refuse empty text.
    let err = commit(
        &mut ledger,
        &lease,
        &[put(DrillRecord::SampleDiscard(discard("S-9", sent)))],
    )
    .unwrap_err();
    assert!(
        err.to_string()
            .contains("sample 'S-9', which is not recorded"),
        "{err}"
    );
    let flag = BatchFlag {
        flag_id: "F-1".to_string(),
        lab: "LAB-A".to_string(),
        lab_batch: "B-0002".to_string(),
        finding: "CRM failed at 3 SD".to_string(),
        flagged_at_ns: sent,
    };
    for r in [
        DrillRecord::BatchFlag(flag.clone()),
        DrillRecord::SampleDiscard(discard("S-1", sent)),
    ] {
        let frame = records::encode_version(&r, None).unwrap();
        assert_eq!(records::decode_version(&frame).unwrap().record, r);
    }
    let blank = BatchFlag {
        finding: String::new(),
        ..flag
    };
    assert!(DrillRecord::BatchFlag(blank).validate().is_err());
    assert!(
        DrillRecord::SampleDiscard(SampleDiscard {
            reason: String::new(),
            ..discard("S-1", sent)
        })
        .validate()
        .is_err()
    );
}
