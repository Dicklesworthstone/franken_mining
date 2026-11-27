//! Synthetic import-to-composite controls (BD-166, BD-188), independently hand-worked values.
//! Determinism: BitwisePortable after exact source projection.

#![allow(clippy::expect_used, clippy::panic)]

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use fmine_drill::composite::{Bounds, Domain, Residual};
use fmine_drill::desurvey::CollarFrom;
use fmine_drill::join::Span;
use fmine_drill::records::{HoleType, PositionMethod, RunDirection, SurveyInstrument};
use fmine_io::acquire::composite::{
    CompositeSpec, HoleDomains, MAX_COMPOSITE_PIECES, prepare_composites,
};
use fmine_io::acquire::drill::{BearingReference, HoleGeometry, PreparedHole, prepare_geometry};
use fmine_io::acquire::{
    AcquireBundleInput, AcquireTableInput, AssayReading, import_acquire_export_bundle,
};
use fmine_io::dialects::{ColumnBasis, IoDialectRegistry};
use fmine_object::store::CasStore;
use fmine_registry::RegistryCatalog;

const COLLARS: &[u8] =
    b"HOLEID,EAST,NORTH,RL,MAXDEPTH\nH1,500000,4500000,1500,6\nH2,500010,4500000,1500,4\n";
const SURVEYS: &[u8] =
    b"HOLEID,DEPTH,AZIMUTH,DIP\nH1,0,0,-90\nH1,6,0,-90\nH2,0,0,-90\nH2,4,0,-90\n";
const ASSAYS: &[u8] = b"HOLEID,SAMPLEID,FROM,TO,AU_GPT,CU_PCT,METHOD\nH1,A1,0,1,1,0.1,FA\nH1,A2,1,3,3,0.3,FA\nH1,A3,3,4,<2,NSS,FA\nH1,A4,4,5,NSS,0.4,FA\nH1,A5,5,6,5,0.5,FA\nH2,B1,0,2,NSS,NSS,FA\n";
static NEXT: AtomicU64 = AtomicU64::new(0);

fn prepared() -> Vec<PreparedHole> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("crates")
        .parent()
        .expect("workspace")
        .to_path_buf();
    let catalog = RegistryCatalog::load_from_root(&root).expect("registries");
    let units = catalog.unit_symbols.into_iter().collect();
    let crs: BTreeSet<String> = catalog.crs_codes.into_iter().collect();
    let dialects = IoDialectRegistry::load_from_root(&root).expect("dialects");
    let cas = CasStore::open(std::env::temp_dir().join(format!(
        "fmine-acquire-composite-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed),
    )))
    .expect("synthetic CAS");
    let bundle = import_acquire_export_bundle(
        &cas,
        &dialects,
        &AcquireBundleInput {
            collars: AcquireTableInput::Csv(COLLARS),
            surveys: AcquireTableInput::Csv(SURVEYS),
            assays: AcquireTableInput::Csv(ASSAYS),
            site_crs: None,
        },
        &units,
        &crs,
        None,
    )
    .expect("source import");
    assert!(bundle.rejected_rows.is_empty());
    let declarations = ["H1", "H2"].map(|id| HoleGeometry {
        hole_id: id.into(),
        program: "synthetic-composite".into(),
        hole_type: HoleType::Diamond,
        collar_from: CollarFrom::Actual,
        position_method: PositionMethod::DifferentialGnss,
        sigma_horizontal_m: 0.2,
        sigma_vertical_m: 0.3,
        instrument: SurveyInstrument::Gyro,
        run: RunDirection::In,
        sigma_azimuth_deg: 1.0,
        sigma_dip_deg: 1.0,
        magnetic_interference: false,
        azimuth_reference: BearingReference::Grid,
        survey_type_label: None,
    });
    prepare_geometry(&bundle, &declarations, &crs).expect("prepared geometry")
}

fn spec() -> CompositeSpec {
    CompositeSpec {
        column: "AU_GPT".into(),
        method: "FA".into(),
        unit: "g_per_t".into(),
        basis: ColumnBasis::Dry,
        length_mm: 2000,
        residual: Residual::Keep,
    }
}

fn domain(code: &str, from_mm: u64, to_mm: u64) -> Domain {
    Domain {
        code: code.into(),
        span: Span { from_mm, to_mm },
    }
}

fn domains() -> Vec<HoleDomains> {
    vec![
        HoleDomains {
            hole_id: "H1".into(),
            domains: vec![domain("oxide", 0, 3000), domain("fresh", 3000, 6000)],
        },
        HoleDomains {
            hole_id: "H2".into(),
            domains: vec![domain("fresh", 0, 4000)],
        },
    ]
}

fn whole_domains(holes: &[PreparedHole]) -> Vec<HoleDomains> {
    holes
        .iter()
        .map(|h| HoleDomains {
            hole_id: h.collar.hole_id.clone(),
            domains: vec![domain("whole", 0, h.max_depth_mm)],
        })
        .collect()
}

#[test]
fn imported_composites_respect_domains_and_keep_every_source_and_missing_span() {
    let holes = prepared();
    let original = holes.clone();
    let out = prepare_composites(&holes, &spec(), &domains()).expect("composites");
    assert_eq!(holes, original);
    assert_eq!(out.len(), 2);
    let h = &out[0];
    assert_eq!((h.sampled_mm, h.unsampled_mm), (5000, 1000));
    assert_eq!(
        h.inputs
            .iter()
            .map(|s| s.source.source.row_index)
            .collect::<Vec<_>>(),
        vec![2, 3, 4, 6]
    );
    assert_eq!(h.missing.len(), 1);
    assert_eq!(h.missing[0].source.source.sample_id.as_deref(), Some("A4"));
    assert!(
        matches!(&h.missing[0].source.source.assays["AU_GPT"], AssayReading::Placeholder { raw } if raw == "NSS")
    );
    for source in h.inputs.iter().chain(&h.missing) {
        assert!(!source.method_assumed);
    }
    let c = &h.set.composites;
    assert_eq!(c.len(), 4);
    for (c, code, from, to, sampled, lo, hi, indices) in [
        (&c[0], "oxide", 0, 2000, 2000, 2.0, 2.0, vec![0, 1]),
        (&c[1], "oxide", 2000, 3000, 1000, 3.0, 3.0, vec![1]),
        (&c[2], "fresh", 3000, 5000, 1000, 0.0, 2.0, vec![2]),
        (&c[3], "fresh", 5000, 6000, 1000, 5.0, 5.0, vec![3]),
    ] {
        assert_eq!(c.domain, code);
        assert_eq!(
            c.span,
            Span {
                from_mm: from,
                to_mm: to
            }
        );
        assert_eq!(c.sampled_mm, sampled);
        assert_eq!(c.value, Some(Bounds { lo, hi }));
        assert_eq!(c.inputs, indices);
    }
    assert_eq!(
        h.set.conservation.inputs,
        Bounds {
            lo: 12_000.0,
            hi: 14_000.0
        }
    );
    assert_eq!(h.set.conservation.composites, h.set.conservation.inputs);
    assert_eq!(h.set.conservation.relative_error, 0.0);
    assert_eq!((out[1].sampled_mm, out[1].unsampled_mm), (0, 4000));
    assert_eq!(out[1].missing.len(), 1);
    assert!(
        out[1]
            .set
            .composites
            .iter()
            .all(|c| c.value.is_none() && c.sampled_mm == 0)
    );

    let mut reordered = holes.clone();
    reordered.reverse();
    for h in &mut reordered {
        h.assays.reverse();
    }
    let mut runs = domains();
    runs.reverse();
    for h in &mut runs {
        h.domains.reverse();
    }
    assert_eq!(
        prepare_composites(&reordered, &spec(), &runs).expect("permuted"),
        out
    );
}

#[test]
fn censoring_is_bounded_missing_is_unweighted_and_residuals_still_conserve() {
    let holes = prepared();
    let domains = whole_domains(&holes);
    let out = prepare_composites(&holes, &spec(), &domains).expect("bounded composite");
    assert_eq!(
        out[0].set.composites[1].value,
        Some(Bounds { lo: 1.5, hi: 2.5 })
    );
    assert_eq!(
        out[0].set.composites[2].value,
        Some(Bounds { lo: 5.0, hi: 5.0 })
    );
    assert_eq!(
        out[0].set.composites[2].sampled_mm, 1000,
        "NSS is neither zero nor sampled support"
    );
    for (residual, kept, dropped) in [
        (Residual::Keep, vec![(0, 4000), (4000, 6000)], vec![]),
        (
            Residual::MergeBelow { min_mm: 3000 },
            vec![(0, 6000)],
            vec![],
        ),
        (
            Residual::DropBelow { min_mm: 3000 },
            vec![(0, 4000)],
            vec![(4000, 6000)],
        ),
        (Residual::Distribute, vec![(0, 3000), (3000, 6000)], vec![]),
    ] {
        let spec = CompositeSpec {
            length_mm: 4000,
            residual,
            ..spec()
        };
        let out = prepare_composites(&holes, &spec, &domains).expect("residual rule");
        let set = &out[0].set;
        assert_eq!(
            set.composites
                .iter()
                .map(|c| (c.span.from_mm, c.span.to_mm))
                .collect::<Vec<_>>(),
            kept
        );
        assert_eq!(
            set.dropped
                .iter()
                .map(|c| (c.span.from_mm, c.span.to_mm))
                .collect::<Vec<_>>(),
            dropped
        );
        assert_eq!(
            set.conservation.inputs,
            Bounds {
                lo: 12_000.0,
                hi: 14_000.0
            }
        );
        assert!(set.conservation.relative_error <= 1e-12);
    }
}

#[test]
fn explicit_methods_units_and_bases_cannot_merge_and_missing_columns_stay_visible() {
    let holes = prepared();
    for changed in [
        CompositeSpec {
            unit: "wt_pct".into(),
            ..spec()
        },
        CompositeSpec {
            basis: ColumnBasis::Wet,
            ..spec()
        },
    ] {
        assert_eq!(
            prepare_composites(&holes, &changed, &domains())
                .expect_err("different meaning")
                .code,
            "FMINE-REF-107"
        );
    }
    for changed in [
        CompositeSpec {
            method: "".into(),
            ..spec()
        },
        CompositeSpec {
            column: "AU_TYPO".into(),
            ..spec()
        },
    ] {
        assert!(prepare_composites(&holes, &changed, &domains()).is_err());
    }
    let mut changed = holes.clone();
    changed[0].assays[3].source.method = Some("OTHER".into());
    assert_eq!(
        prepare_composites(&changed, &spec(), &domains())
            .expect_err("placeholder method still matters")
            .code,
        "FMINE-REF-208"
    );
    changed = holes.clone();
    changed[0].assays[0].source.method = None;
    changed[0].assays[4].source.assays.remove("AU_GPT");
    changed[1].assays.clear();
    let out = prepare_composites(&changed, &spec(), &domains()).expect("declared missing evidence");
    assert!(out[0].inputs[0].method_assumed);
    assert_eq!(out[0].missing.len(), 2);
    assert_eq!((out[0].sampled_mm, out[0].unsampled_mm), (4000, 2000));
    assert_eq!(out[1].unsampled_mm, 4000);
    assert!(
        out[1].missing.is_empty(),
        "a gap is not a fabricated source row"
    );
    let mut placeholder_only = holes;
    for h in &mut placeholder_only {
        for a in &mut h.assays {
            a.source.assays.insert(
                "AU_GPT".into(),
                AssayReading::Placeholder { raw: "NSS".into() },
            );
        }
    }
    assert!(
        prepare_composites(&placeholder_only, &spec(), &domains())
            .expect("column exists despite missing values")
            .iter()
            .all(|h| h.sampled_mm == 0)
    );
}

#[test]
fn domain_declarations_must_cover_exactly_the_source_holes_without_gaps_or_overlap() {
    let holes = prepared();
    let mut missing = domains();
    missing.pop();
    let mut extra = domains();
    extra.push(HoleDomains {
        hole_id: "UNKNOWN".into(),
        domains: vec![],
    });
    let mut duplicate = domains();
    duplicate.push(duplicate[0].clone());
    for bad in [missing, extra, duplicate] {
        assert!(prepare_composites(&holes, &spec(), &bad).is_err());
    }
    for bad in [
        vec![domain("gap", 0, 2000), domain("gap", 3000, 6000)],
        vec![domain("overlap", 0, 4000), domain("overlap", 3000, 6000)],
        vec![domain("short", 0, 5000)],
        vec![],
        vec![domain("too-deep", 0, 6001)],
        vec![domain("", 0, 6000)],
    ] {
        let mut changed = domains();
        changed[0].domains = bad;
        assert!(prepare_composites(&holes, &spec(), &changed).is_err());
    }
    let mut changed = holes.clone();
    changed.push(changed[0].clone());
    assert!(prepare_composites(&changed, &spec(), &domains()).is_err());
    changed = holes;
    changed[0].assays[0].to_mm += 1;
    assert!(
        prepare_composites(&changed, &spec(), &domains()).is_err(),
        "source endpoint edits do not stay source-bound"
    );
}

#[test]
fn all_pieces_are_bounded_before_kernel_allocation_even_for_extreme_domain_lengths() {
    let mut holes = prepared();
    let fine = CompositeSpec {
        length_mm: 1,
        ..spec()
    };
    for h in &mut holes {
        h.max_depth_mm = MAX_COMPOSITE_PIECES as u64 / 2;
    }
    let out =
        prepare_composites(&holes, &fine, &whole_domains(&holes)).expect("exactly the piece limit");
    assert_eq!(
        out.iter().map(|h| h.set.composites.len()).sum::<usize>(),
        MAX_COMPOSITE_PIECES
    );
    holes[0].max_depth_mm += 1;
    assert!(
        prepare_composites(&holes, &fine, &whole_domains(&holes))
            .expect_err("one piece too many")
            .what
            .contains("pieces")
    );
    holes[0].max_depth_mm = 1_u64 << 53;
    assert!(prepare_composites(&holes, &fine, &whole_domains(&holes)).is_err());
    assert!(
        prepare_composites(
            &prepared(),
            &CompositeSpec {
                length_mm: 0,
                ..spec()
            },
            &domains()
        )
        .is_err()
    );
}
