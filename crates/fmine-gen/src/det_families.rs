//! Determinism: BitwisePortable
//! Cross-ISA `BitwisePortable` disclosure-grade fixture families (`BD-141`, plan §29.2 & §29.7,
//! `PO-NUM-1`, `GATE-DET-01`).
//!
//! Covers 18 disclosure-grade deterministic families across the entire workspace stack:
//! - S0/K0/E core numerical kernels (8 already CI-pinned dual-ISA golden digests from
//!   `fmine-core::det`, `fmine-numerics`, `fmine-calib`, `fmine-metrics`);
//! - K0/K1/K2/K3/S1/V/U/T mining engines (`fmine-geom`, `fmine-optimize`, `fmine-flow`,
//!   `fmine-econ`, `fmine-explore`, `fmine-plant`, `fmine-telemetry`, `fmine-view`, `fmine-io`,
//!   `fmine-gen`).
//!
//! Every family outputs a canonical 64-hex-char BLAKE3 digest (or its crate's committed BLAKE3/SHA-256
//! digest) plus a deterministic single-line JSON summary (`{"family":...,"crate":...,"digest":...}`)
//! suitable for cross-ISA (`x86_64-linux` vs `aarch64-darwin`) and cross-profile (`dev` vs `release`)
//! comparison in CI (`GATE-DET-01`).

use fmine_claim::SpatialSupport;
use fmine_core::hash::blake3_hash;
use fmine_core::rand::RngKey;
use fmine_crs::CrsId;
use fmine_econ::npv::{
    ConventionDiscoveryRequest, DiscountConvention, IrrScanRequest, TaxBasis,
    discover_discounting_convention, find_all_bracketed_irrs,
};
use fmine_explore::three_part::{
    GradeTonnageDeposit, UndiscoveredCountPmf, ks_two_sample_exact, simulate_three_part_endowment,
};
use fmine_flow::compensation::{
    AnnualFactorRow, WithinFactorInput, audit_multi_year_series, detect_within_factor_compensation,
};
use fmine_geom::vdb::VdbGrid;
use fmine_geom::volcert::ClosedMesh;
use fmine_geom::winding::{Soup, winding_exact};
use fmine_io::dxf::parse_ascii_dxf;
use fmine_io::las::{LasPoint, parse_las14, write_minimal_las14};
use fmine_optimize::pitflow::{self, ClosureProblem};
use fmine_plant::cards::InputUnit;
use fmine_plant::cards::cu_speciation::{
    CuSpeciationCard, CuSpeciationInput, ExactValidityRange, evaluate_cu_speciation_card,
};
use fmine_plant::cards::preg_rob::{CarbonType, PregRobCard, PregRobInput, evaluate_preg_rob_card};
use fmine_plant::custody::{DoreBarInput, DoreVaultLog};
use fmine_telemetry::backpressure::IngestPolicy;
use fmine_telemetry::endurance::run_synthetic_ingest_endurance;
use fmine_telemetry::segment::{DEFAULT_MAX_SEGMENT_BYTES, DEFAULT_SEAL_INTERVAL_NS};
use fmine_units::ExactQty;
use fmine_view::gltf::{GltfSceneInput, export_gltf_scene};
use fmine_view::voxel_mesh::{VoxelGridFrame, extract_voxel_surface_mesh};

use crate::review::{ReviewKey, SeedClass, clopper_pearson, commit};

/// One disclosure-grade `BitwisePortable` fixture family digest record (`GATE-DET-01`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FamilyDigest {
    pub family: &'static str,
    pub crate_name: &'static str,
    pub digest_hex: String,
}

impl FamilyDigest {
    /// Stable single-line JSON log record for cross-ISA CI summaries.
    #[must_use]
    pub fn to_json_line(&self) -> String {
        format!(
            "{{\"family\":\"{}\",\"crate\":\"{}\",\"digest\":\"{}\"}}",
            self.family, self.crate_name, self.digest_hex
        )
    }
}

fn push_qty(buf: &mut Vec<u8>, q: ExactQty) {
    buf.extend_from_slice(&q.num().to_le_bytes());
    buf.extend_from_slice(&q.den().to_le_bytes());
}

fn push_f64_canon(buf: &mut Vec<u8>, v: f64) {
    let bits = if v == 0.0 {
        0.0_f64.to_bits()
    } else {
        v.to_bits()
    };
    buf.extend_from_slice(&bits.to_le_bytes());
}

fn digest_geom_volcert_and_winding() -> Result<String, String> {
    // 10m cube outward-oriented closed mesh: exact divergence volume = 1000.0 + exact winding at interior/exterior.
    let positions = vec![
        [0.0, 0.0, 0.0],
        [10.0, 0.0, 0.0],
        [10.0, 10.0, 0.0],
        [0.0, 10.0, 0.0],
        [0.0, 0.0, 10.0],
        [10.0, 0.0, 10.0],
        [10.0, 10.0, 10.0],
        [0.0, 10.0, 10.0],
    ];
    let tris = vec![
        [0, 3, 2],
        [0, 2, 1],
        [4, 5, 6],
        [4, 6, 7],
        [0, 1, 5],
        [0, 5, 4],
        [2, 3, 7],
        [2, 7, 6],
        [3, 0, 4],
        [3, 4, 7],
        [1, 2, 6],
        [1, 6, 5],
    ];
    let mesh = ClosedMesh::certify(positions.clone(), tris.clone()).map_err(|e| e.to_string())?;
    let soup = Soup::new(positions, tris).map_err(|e| e.to_string())?;
    let w_in = winding_exact(&soup, [5.0, 5.0, 5.0]).map_err(|e| e.to_string())?;
    let w_out = winding_exact(&soup, [20.0, 5.0, 5.0]).map_err(|e| e.to_string())?;

    let mut buf = Vec::new();
    buf.extend_from_slice(b"fmine:gate-det-01:geom:v1\0");
    push_f64_canon(&mut buf, mesh.volume().lo());
    push_f64_canon(&mut buf, mesh.volume().hi());
    push_f64_canon(&mut buf, w_in);
    push_f64_canon(&mut buf, w_out);
    Ok(blake3_hash(&buf).to_hex())
}

fn digest_optimize_pitflow_closure() -> Result<String, String> {
    // 6-block 2-level open-pit precedence graph (top overburden -4,-3,-4; deep ore +15 requires all 3 top blocks).
    let weights = vec![-4_i64, -3, -4, 15, -2, 1];
    let offsets = vec![0_usize, 0, 0, 0, 3, 4, 4];
    let targets = vec![0_u32, 1, 2, 2];
    let problem = ClosureProblem::new(weights, offsets, targets).map_err(|e| e.to_string())?;
    let sol = pitflow::solve(&problem).map_err(|e| e.to_string())?;
    let verified_val =
        pitflow::verify(&problem, sol.in_pit(), sol.arc_flow()).map_err(|e| e.to_string())?;

    let mut buf = Vec::new();
    buf.extend_from_slice(b"fmine:gate-det-01:pitflow:v1\0");
    buf.extend_from_slice(&verified_val.to_le_bytes());
    for &b in sol.in_pit() {
        buf.push(u8::from(b));
    }
    for &f in sol.arc_flow() {
        buf.extend_from_slice(&f.to_le_bytes());
    }
    Ok(blake3_hash(&buf).to_hex())
}

fn digest_flow_compensation() -> Result<String, String> {
    let q = |n: i128, d: i128| ExactQty::new(n, d).map_err(|e| e.to_string());
    let goldstrike = detect_within_factor_compensation(&WithinFactorInput {
        site_id: "ngm_goldstrike_roaster".to_string(),
        period: "2024".to_string(),
        factor_key: "MCF_out.ngm@v1".to_string(),
        ownership_basis: "100% JV; Barrick 61.5% / Newmont 38.5%".to_string(),
        tonnes_ratio: q(109, 100)?,
        grade_ratio: q(94, 100)?,
        metal_ratio: q(102, 100)?,
        tol_bps: 500,
        source_e_ids: vec!["E154".to_string()],
    })
    .map_err(|e| e.to_string())?;

    let kibali_rows: Vec<AnnualFactorRow> = [
        (2016_u16, 104, 92, 95),
        (2017, 101, 93, 94),
        (2018, 99, 94, 93),
        (2019, 102, 91, 93),
        (2020, 100, 94, 94),
        (2021, 98, 95, 93),
        (2022, 103, 91, 94),
        (2023, 101, 92, 93),
        (2024, 105, 89, 93),
        (2025, 102, 91, 93),
    ]
    .into_iter()
    .map(|(year, t, g, m)| {
        Ok::<AnnualFactorRow, String>(AnnualFactorRow {
            year,
            tonnes_ratio: q(t, 100)?,
            grade_ratio: q(g, 100)?,
            metal_ratio: q(m, 100)?,
        })
    })
    .collect::<Result<Vec<_>, _>>()?;

    let series = audit_multi_year_series(
        "kibali",
        "F1_grade.kibali@v1",
        "100% project; Barrick 45%",
        &kibali_rows,
        500,
        &["E210".to_string(), "E211".to_string(), "E212".to_string()],
    )
    .map_err(|e| e.to_string())?;

    let mut buf = Vec::new();
    buf.extend_from_slice(b"fmine:gate-det-01:flow-comp:v1\0");
    buf.push(u8::from(goldstrike.compensation_detected));
    buf.push(u8::from(goldstrike.fully_masked_metal));
    push_qty(&mut buf, goldstrike.product_minus_metal);
    push_qty(&mut buf, series.bias_prior.mean_metal_bias);
    if let Some(var) = series.bias_prior.sample_variance_metal {
        push_qty(&mut buf, var);
    }
    buf.extend_from_slice(&(series.flagged_years.len() as u64).to_le_bytes());
    Ok(blake3_hash(&buf).to_hex())
}

fn digest_econ_npv_and_irr() -> Result<String, String> {
    let q = |n: i128, d: i128| ExactQty::new(n, d).map_err(|e| e.to_string());
    let flows = vec![
        ExactQty::from_integer(-100),
        ExactQty::from_integer(30),
        ExactQty::from_integer(40),
        ExactQty::from_integer(50),
        ExactQty::from_integer(20),
    ];
    let disc = discover_discounting_convention(&ConventionDiscoveryRequest {
        report_id: "gate-det-01-synthetic",
        report_sha256: "0000000000000000",
        flows: &flows,
        flow_rounding_step: ExactQty::from_integer(0),
        discount_rate: q(5, 100)?,
        printed_npv: q(251, 10)?,
        printed_step: q(1, 10)?,
        tax_basis: &TaxBasis::PreTax,
    })
    .map_err(|e| e.to_string())?;

    let two_root_flows = vec![
        ExactQty::from_integer(-100),
        ExactQty::from_integer(230),
        ExactQty::from_integer(-132),
    ];
    let irrs = find_all_bracketed_irrs(&IrrScanRequest {
        flows: &two_root_flows,
        convention: DiscountConvention::EndYear,
        tax_basis: &TaxBasis::PreTax,
        scan_lo: q(0, 1)?,
        scan_hi: q(50, 100)?,
        subdivisions: 10,
        tol: q(1, 1_000_000_000)?,
        max_iter: 100,
    })
    .map_err(|e| e.to_string())?;

    let mut buf = Vec::new();
    buf.extend_from_slice(b"fmine:gate-det-01:econ:v1\0");
    buf.extend_from_slice(disc.discovered.as_str().as_bytes());
    for row in &disc.evaluations {
        push_qty(&mut buf, row.npv_interval.lo);
        push_qty(&mut buf, row.npv_interval.hi);
        buf.push(u8::from(row.matches_printed));
    }
    for root in &irrs {
        push_qty(&mut buf, root.rate);
    }
    Ok(blake3_hash(&buf).to_hex())
}

fn digest_explore_three_part() -> Result<String, String> {
    let q = |n: i128, d: i128| ExactQty::new(n, d).map_err(|e| e.to_string());
    let pmf =
        UndiscoveredCountPmf::from_usgs_mark3b_quantiles(1, 3, 6).map_err(|e| e.to_string())?;
    let gt = vec![
        GradeTonnageDeposit {
            deposit_id: "dep-a".to_string(),
            tonnage_kt: ExactQty::from_integer(100_000),
            grade_fraction: q(5, 1000)?,
        },
        GradeTonnageDeposit {
            deposit_id: "dep-b".to_string(),
            tonnage_kt: ExactQty::from_integer(200_000),
            grade_fraction: q(6, 1000)?,
        },
        GradeTonnageDeposit {
            deposit_id: "dep-c".to_string(),
            tonnage_kt: ExactQty::from_integer(50_000),
            grade_fraction: q(8, 1000)?,
        },
    ];
    let k1 = RngKey {
        seed_root: 0x0141_2350,
        partition: 1,
        domain: 1,
        var_group: 1,
        realization: 0,
        brick: 0,
    };
    let k2 = RngKey {
        realization: 1,
        ..k1
    };
    let s1 = simulate_three_part_endowment("det-tract-1", &pmf, &gt, 2_000, k1)
        .map_err(|e| e.to_string())?;
    let s2 = simulate_three_part_endowment("det-tract-1", &pmf, &gt, 2_000, k2)
        .map_err(|e| e.to_string())?;
    let ks = ks_two_sample_exact(&s1.sorted_metal_draws_kt, &s2.sorted_metal_draws_kt)
        .map_err(|e| e.to_string())?;

    let mut buf = Vec::new();
    buf.extend_from_slice(b"fmine:gate-det-01:explore:v1\0");
    push_qty(&mut buf, s1.p10_metal_kt);
    push_qty(&mut buf, s1.p50_metal_kt);
    push_qty(&mut buf, s1.p90_metal_kt);
    push_qty(&mut buf, s1.empirical_mean_metal_kt);
    push_qty(&mut buf, s1.analytical_mean_metal_kt);
    push_qty(&mut buf, s1.empirical_zero_fraction);
    push_qty(&mut buf, ks);
    Ok(blake3_hash(&buf).to_hex())
}

fn digest_plant_custody_and_cards() -> Result<String, String> {
    let q = |n: i128, d: i128| ExactQty::new(n, d).map_err(|e| e.to_string());
    let mut vault = DoreVaultLog::new("barrick_kibali").map_err(|e| e.to_string())?;
    let bar = vault
        .append_bar(&DoreBarInput {
            site_id: "barrick_kibali".to_string(),
            bar_id: "KIB-2026-0001".to_string(),
            seal_number: "SEAL-9001".to_string(),
            gross_mass_g: ExactQty::from_integer(25_000),
            top_pin_au_fraction: q(875, 1000)?,
            bottom_pin_au_fraction: q(877, 1000)?,
            ag_fraction: q(85, 1000)?,
            max_pin_spread_fraction: q(5, 1000)?,
        })
        .map_err(|e| e.to_string())?;

    let cu_card = CuSpeciationCard::zaldivar_reference(
        ExactValidityRange::new(q(180, 1)?, q(540, 1)?).map_err(|e| e.to_string())?,
        ExactValidityRange::new(q(25, 100)?, q(120, 100)?).map_err(|e| e.to_string())?,
        ExactValidityRange::new(q(10, 100)?, q(85, 100)?).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    let cu_v = evaluate_cu_speciation_card(
        &cu_card,
        &CuSpeciationInput {
            support: SpatialSupport::PeriodAggregate,
            as_cu_ratio: q(50, 100)?,
            cn_cu_ratio: q(30, 100)?,
            s_cu_ratio: q(20, 100)?,
            total_cu_pct: q(55, 100)?,
            residence_time_days: q(360, 1)?,
        },
    )
    .map_err(|e| e.to_string())?;

    let pr_card = PregRobCard::ngm_40_pct_cap("ngm_sage_feed").map_err(|e| e.to_string())?;
    let pr_v = evaluate_preg_rob_card(
        &pr_card,
        &PregRobInput {
            unit: InputUnit::SpikeTestPercent,
            spike_test_pct: ExactQty::from_integer(35),
            carbon_type: Some(CarbonType::RamanLowActivity),
            annual_feed_tonnes: Some(ExactQty::from_integer(100_000)),
        },
    )
    .map_err(|e| e.to_string())?;

    let mut buf = Vec::new();
    buf.extend_from_slice(b"fmine:gate-det-01:plant:v1\0");
    buf.extend_from_slice(bar.entry_hash.as_bytes());
    push_qty(&mut buf, cu_v.tcu_recovery_fraction);
    push_qty(&mut buf, pr_v.recovery_fraction);
    Ok(blake3_hash(&buf).to_hex())
}

fn digest_telemetry_endurance() -> Result<String, String> {
    let audit = run_synthetic_ingest_endurance(
        IngestPolicy {
            site_id: "barrick_kibali".to_string(),
            seal_interval_ns: DEFAULT_SEAL_INTERVAL_NS,
            max_segment_samples: 25_000,
            max_segment_bytes: DEFAULT_MAX_SEGMENT_BYTES,
            max_batch_admit_samples: 2_000,
            downsample_stride: 2,
            ring_capacity_samples: 5_000,
        },
        15,
        1_000,
        25,
        10_000,
        ExactQty::from_integer(150_000),
    )
    .map_err(|e| e.to_string())?;

    let mut buf = Vec::new();
    buf.extend_from_slice(b"fmine:gate-det-01:telemetry:v1\0");
    push_qty(&mut buf, audit.utilization_rho);
    push_qty(&mut buf, audit.mean_wait_s);
    push_qty(&mut buf, audit.littles_law_mean_queue);
    buf.extend_from_slice(&audit.p99_ingest_to_seal_ns.to_le_bytes());
    buf.extend_from_slice(&audit.total_sealed.to_le_bytes());
    buf.push(u8::from(audit.meets_po_bed_7));
    Ok(blake3_hash(&buf).to_hex())
}

fn digest_view_voxel_glb() -> Result<String, String> {
    let mut grid = VdbGrid::new(0u8);
    for x in 0..3 {
        for y in 0..3 {
            grid.set([x, y, 0], 1);
        }
    }
    grid.set([1, 1, 1], 2);
    let frame = VoxelGridFrame {
        origin_m: [1000.0, 2000.0, 500.0],
        voxel_size_m: [10.0, 10.0, 10.0],
        crs_id: CrsId::REKO_DIQ_UTM41N,
    };
    let bench =
        extract_voxel_surface_mesh("bench", &grid, &frame, [0.55, 0.55, 0.58, 1.0], Some(1))
            .map_err(|e| e.to_string())?;
    let ore = extract_voxel_surface_mesh("ore", &grid, &frame, [0.90, 0.72, 0.10, 1.0], Some(2))
        .map_err(|e| e.to_string())?;
    let export = export_gltf_scene(&GltfSceneInput {
        scene_name: "gate_det_01_scene".to_string(),
        meshes: vec![bench, ore],
    })
    .map_err(|e| e.to_string())?;
    Ok(blake3_hash(&export.glb_bytes).to_hex())
}

fn digest_io_dxf_and_las14() -> Result<String, String> {
    let pface = "\
0\nSECTION\n2\nHEADER\n9\n$INSUNITS\n70\n6\n0\nENDSEC\n\
0\nSECTION\n2\nENTITIES\n\
0\nPOLYLINE\n8\nSTOPE\n70\n64\n\
0\nVERTEX\n70\n192\n10\n0\n20\n0\n30\n0\n\
0\nVERTEX\n70\n192\n10\n6\n20\n0\n30\n0\n\
0\nVERTEX\n70\n192\n10\n0\n20\n6\n30\n0\n\
0\nVERTEX\n70\n192\n10\n0\n20\n0\n30\n6\n\
0\nVERTEX\n70\n128\n71\n1\n72\n3\n73\n2\n\
0\nVERTEX\n70\n128\n71\n1\n72\n2\n73\n4\n\
0\nVERTEX\n70\n128\n71\n1\n72\n4\n73\n3\n\
0\nVERTEX\n70\n128\n71\n2\n72\n-3\n73\n4\n\
0\nSEQEND\n0\nENDSEC\n0\nEOF\n";
    let dxf =
        parse_ascii_dxf(pface.as_bytes(), CrsId::REKO_DIQ_UTM41N).map_err(|e| e.to_string())?;
    let mesh = dxf
        .certify_closed_mesh(Some("STOPE"))
        .map_err(|e| e.to_string())?;

    let las_bytes = write_minimal_las14(
        &[LasPoint {
            x: 512_345.25,
            y: 3_210_987.50,
            z: 1_142.125,
            intensity: 4200,
            return_number: 1,
            number_of_returns: 1,
            classification: 2,
            scan_angle_deg: -6.0,
            user_data: 3,
            point_source_id: 9,
            gps_time: Some(98_765.5),
            rgb: Some([100, 200, 300]),
            nir: None,
        }],
        [0.001, 0.001, 0.001],
        [500_000.0, 3_200_000.0, 1_000.0],
        7,
        Some(r#"PROJCS["WGS 84 / UTM zone 41N",AUTHORITY["EPSG","32641"]]"#),
    )
    .map_err(|e| e.to_string())?;
    let cloud = parse_las14(&las_bytes).map_err(|e| e.to_string())?;

    let mut buf = Vec::new();
    buf.extend_from_slice(b"fmine:gate-det-01:io-dxf-las:v1\0");
    buf.extend_from_slice(dxf.source_sha256.as_bytes());
    push_f64_canon(&mut buf, mesh.volume().lo());
    push_f64_canon(&mut buf, mesh.volume().hi());
    buf.extend_from_slice(cloud.source_sha256.as_bytes());
    push_f64_canon(&mut buf, cloud.points[0].x);
    push_f64_canon(&mut buf, cloud.points[0].y);
    push_f64_canon(&mut buf, cloud.points[0].z);
    Ok(blake3_hash(&buf).to_hex())
}

fn digest_gen_review_clopper_pearson() -> Result<String, String> {
    let mut buf = Vec::new();
    buf.extend_from_slice(b"fmine:gate-det-01:review:v1\0");
    for (x, n) in [(0_u32, 10_u32), (3, 10), (8, 10), (10, 10), (24, 50)] {
        let (lo, hi) = clopper_pearson(x, n, 0.05).map_err(|e| e.to_string())?;
        push_f64_canon(&mut buf, lo);
        push_f64_canon(&mut buf, hi);
    }
    let key = ReviewKey {
        round_id: "gate-det-01-round".to_string(),
        items: vec![
            (
                "bead:bd-141".to_string(),
                SeedClass::S1CloseCitesNonExercisingTest,
            ),
            ("cell:A1".to_string(), SeedClass::S3MisCitedCell),
        ],
        nonce: [0x5A; 32],
    };
    let digest = commit(&key).map_err(|e| e.to_string())?;
    buf.extend_from_slice(digest.as_bytes());
    Ok(blake3_hash(&buf).to_hex())
}

/// Evaluate all 18 disclosure-grade `BitwisePortable` fixture families in canonical order (`BD-141`,
/// `GATE-DET-01`).
pub fn evaluate_all_bitwise_portable_families() -> Result<Vec<FamilyDigest>, String> {
    Ok(vec![
        FamilyDigest {
            family: "core_det_10k",
            crate_name: "fmine-core",
            digest_hex: fmine_core::det::golden_digest_10k(),
        },
        FamilyDigest {
            family: "numerics_norm_cdf_inv_10k",
            crate_name: "fmine-numerics",
            digest_hex: fmine_numerics::golden_digest_norm_cdf_inv_10k(),
        },
        FamilyDigest {
            family: "numerics_cholesky_brent_10k",
            crate_name: "fmine-numerics",
            digest_hex: fmine_numerics::golden_digest_cholesky_brent_10k(),
        },
        FamilyDigest {
            family: "calib_gaussian_crps_10k",
            crate_name: "fmine-calib",
            digest_hex: fmine_calib::golden_digest_gaussian_crps_10k(),
        },
        FamilyDigest {
            family: "calib_gaussian_log_score_10k",
            crate_name: "fmine-calib",
            digest_hex: fmine_calib::golden_digest_gaussian_log_score_10k(),
        },
        FamilyDigest {
            family: "calib_rest_10k",
            crate_name: "fmine-calib",
            digest_hex: fmine_calib::golden_digest_calib_rest_10k(),
        },
        FamilyDigest {
            family: "metrics_log_mean_10k",
            crate_name: "fmine-metrics",
            digest_hex: fmine_metrics::lmdi::golden_digest_logarithmic_mean_10k(),
        },
        FamilyDigest {
            family: "metrics_aisc_lmdi_10k",
            crate_name: "fmine-metrics",
            digest_hex: fmine_metrics::lmdi::golden_digest_decompose_aisc_lmdi_10k(),
        },
        FamilyDigest {
            family: "geom_volcert_and_winding",
            crate_name: "fmine-geom",
            digest_hex: digest_geom_volcert_and_winding()?,
        },
        FamilyDigest {
            family: "optimize_pitflow_closure",
            crate_name: "fmine-optimize",
            digest_hex: digest_optimize_pitflow_closure()?,
        },
        FamilyDigest {
            family: "flow_compensation_kibali_goldstrike",
            crate_name: "fmine-flow",
            digest_hex: digest_flow_compensation()?,
        },
        FamilyDigest {
            family: "econ_npv_conventions_and_irr",
            crate_name: "fmine-econ",
            digest_hex: digest_econ_npv_and_irr()?,
        },
        FamilyDigest {
            family: "explore_three_part_n0_endowment",
            crate_name: "fmine-explore",
            digest_hex: digest_explore_three_part()?,
        },
        FamilyDigest {
            family: "plant_custody_and_cards",
            crate_name: "fmine-plant",
            digest_hex: digest_plant_custody_and_cards()?,
        },
        FamilyDigest {
            family: "telemetry_segments_and_continuity",
            crate_name: "fmine-telemetry",
            digest_hex: digest_telemetry_endurance()?,
        },
        FamilyDigest {
            family: "view_voxel_mesh_and_glb",
            crate_name: "fmine-view",
            digest_hex: digest_view_voxel_glb()?,
        },
        FamilyDigest {
            family: "io_dxf_and_las14",
            crate_name: "fmine-io",
            digest_hex: digest_io_dxf_and_las14()?,
        },
        FamilyDigest {
            family: "gen_review_clopper_pearson",
            crate_name: "fmine-gen",
            digest_hex: digest_gen_review_clopper_pearson()?,
        },
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    const PINNED_MANIFEST: &str =
        include_str!("../../../fixtures/det/bitwise_portable_families.txt");

    #[test]
    fn test_gate_det_01_all_18_bitwise_portable_families_match_pinned_manifest() {
        let run_a = evaluate_all_bitwise_portable_families().unwrap();
        let run_b = evaluate_all_bitwise_portable_families().unwrap();
        assert_eq!(run_a.len(), 18);
        assert_eq!(
            run_a, run_b,
            "every BitwisePortable family must be repeat-identical"
        );

        let expected: Vec<(&str, &str, &str)> = PINNED_MANIFEST
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty() && !l.starts_with('#'))
            .map(|line| {
                let parts: Vec<&str> = line.split_whitespace().collect();
                assert_eq!(parts.len(), 3, "malformed manifest line: {line}");
                (parts[0], parts[1], parts[2])
            })
            .collect();
        assert_eq!(
            expected.len(),
            run_a.len(),
            "pinned manifest family count must equal evaluated family count"
        );

        for (actual, (exp_family, exp_crate, exp_digest)) in run_a.iter().zip(expected.iter()) {
            assert_eq!(actual.family, *exp_family);
            assert_eq!(actual.crate_name, *exp_crate);
            assert_eq!(
                actual.digest_hex, *exp_digest,
                "GATE-DET-01 digest drift on family '{}' ({})",
                actual.family, actual.crate_name
            );
            println!("{}", actual.to_json_line());
        }
    }
}
