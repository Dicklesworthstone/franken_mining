#![allow(
    clippy::expect_used,
    reason = "integration benchmark test verifies BD-310 performance and conservation"
)]

//! `BD-310` `FMT-COL-1` sweep-line compositing benchmark and multi-hole conservation test.
//!
//! - `test_fmt_col_1_100k_intervals_sweep_line_conservation` runs on every `cargo test` (~20 ms).
//! - `bench_fmt_col_1_10m_intervals_under_3s_single_core` (`#[ignore]`, run in `--release`) builds
//!   10,000,000 intervals across 100,000 holes, composites them on one thread with zero per-interval
//!   heap allocations into a pre-allocated slice, asserts mass/metal conservation and `< 3.0 s` wall time.

use std::time::Instant;

use fmine_array::AlignedF64Block;
use fmine_array::col::{ColumnarIntervalChunk, HoleComposite, IntervalRow, QualifierBits};

fn build_synthetic_chunk(
    holes: usize,
    intervals_per_hole: usize,
) -> (ColumnarIntervalChunk, f64, f64) {
    let total = holes * intervals_per_hole;
    let mut chunk = ColumnarIntervalChunk::with_capacity(total, holes);
    let mut raw_mass = 0.0f64;
    let mut raw_metal = 0.0f64;
    let mut hole_name = String::with_capacity(16);

    for h in 0..holes {
        hole_name.clear();
        use std::fmt::Write as _;
        let _ = write!(&mut hole_name, "DDH-{h:06}");
        let mut depth = 0i32;
        for k in 0..intervals_per_hole {
            let step_mm = 1000 + ((h.wrapping_add(k) % 5) as i32) * 250; // 1.00..2.00 m
            let next = depth + step_mm;
            let rho = 2.55 + ((k % 4) as f64) * 0.10;
            let grade = 0.40 + (((h * 7 + k * 13) % 40) as f64) * 0.15;
            let len_m = f64::from(step_mm) * 0.001;
            raw_mass += len_m * rho;
            raw_metal += len_m * rho * grade;
            chunk
                .push(IntervalRow {
                    hole_id: &hole_name,
                    from_mm: depth,
                    to_mm: next,
                    measurand_id: 1,
                    qualifiers: QualifierBits::NONE,
                    valid: true,
                    value: grade,
                    dry_density_t_m3: rho,
                    error_variance: 0.02,
                    xyz: None,
                })
                .expect("push row");
            depth = next;
        }
    }
    (chunk, raw_mass, raw_metal)
}

#[test]
fn test_fmt_col_1_100k_intervals_sweep_line_conservation() {
    assert_eq!(std::mem::align_of::<AlignedF64Block>(), 64);
    let (chunk, raw_mass, raw_metal) = build_synthetic_chunk(1_000, 100);
    assert_eq!(chunk.len(), 100_000);
    assert_eq!(chunk.hole_dict().len(), 1_000);

    // Pre-allocate caller output buffer once; composite_chunk_into allocates zero bytes.
    let mut out = vec![HoleComposite::default(); 100_000];
    let n = chunk
        .composite_chunk_into(2_000, &mut out)
        .expect("composite 100k");
    assert!(n > 50_000);

    let comp_mass: f64 = out[..n]
        .iter()
        .map(|c| c.length_m * c.mean_density_t_m3)
        .sum();
    let comp_metal: f64 = out[..n]
        .iter()
        .map(|c| c.length_m * c.mean_density_t_m3 * c.mass_weighted_value)
        .sum();
    assert!(((comp_mass - raw_mass) / raw_mass).abs() < 1e-11);
    assert!(((comp_metal - raw_metal) / raw_metal).abs() < 1e-11);
}

#[test]
#[ignore = "BD-310 10^7-interval release benchmark (~1.1 GiB RAM); run with --release -- --ignored --nocapture"]
fn bench_fmt_col_1_10m_intervals_under_3s_single_core() {
    let holes = 100_000usize;
    let intervals_per_hole = 100usize;
    let (chunk, raw_mass, raw_metal) = build_synthetic_chunk(holes, intervals_per_hole);
    assert_eq!(chunk.len(), 10_000_000);

    let mut out = vec![HoleComposite::default(); 8_000_000];
    let start = Instant::now();
    let n = chunk
        .composite_chunk_into(2_000, &mut out)
        .expect("composite 10^7 intervals");
    let elapsed = start.elapsed();

    let comp_mass: f64 = out[..n]
        .iter()
        .map(|c| c.length_m * c.mean_density_t_m3)
        .sum();
    let comp_metal: f64 = out[..n]
        .iter()
        .map(|c| c.length_m * c.mean_density_t_m3 * c.mass_weighted_value)
        .sum();
    let mass_rel = ((comp_mass - raw_mass) / raw_mass).abs();
    let metal_rel = ((comp_metal - raw_metal) / raw_metal).abs();

    println!(
        "{{\"bench\":\"BD-310\",\"intervals\":{},\"holes\":{},\"composites\":{},\"elapsed_ms\":{:.2},\"ns_per_interval\":{:.2},\"mass_rel_err\":{:.3e},\"metal_rel_err\":{:.3e}}}",
        chunk.len(),
        holes,
        n,
        elapsed.as_secs_f64() * 1_000.0,
        elapsed.as_nanos() as f64 / chunk.len() as f64,
        mass_rel,
        metal_rel
    );

    assert!(mass_rel < 1e-10, "mass conservation drift {mass_rel}");
    assert!(metal_rel < 1e-10, "metal conservation drift {metal_rel}");
    assert!(
        elapsed.as_secs_f64() < 3.0,
        "10^7 sweep-line composite took {:.3}s (target < 3.0s)",
        elapsed.as_secs_f64()
    );
}
