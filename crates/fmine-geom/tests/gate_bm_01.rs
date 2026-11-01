//! GATE-BM-01 (plan §26 gate table, `BD-191`): the lifted `VdbGrid` holds 10⁸ active cells with an 8-byte payload
//! in resident memory at most 2.5× the payload, and its `iter_active` digest is deterministic.
//!
//! Ignored by default (about 1 GB and a minute in release). Run it with
//! `cargo test --release -p fmine-geom --test gate_bm_01 -- --ignored --nocapture`.
//!
//! - Resident memory is the process RSS from `ps -o rss=`, read before and after the build. The workspace forbids
//!   `unsafe`, so no counting allocator is possible; RSS is the honest measure, and it includes allocator slack.
//! - Determinism: the grid is built twice, in opposite insertion orders, and both `iter_active` digests must equal
//!   each other and the pinned value.
//!
//! What green does not prove: anything about sparse occupancy. The cells form one dense box, the block-model shape;
//! a scattered pattern with one active voxel per leaf costs about 4 KiB per voxel (CONTRACT.md §8).

use fmine_core::hash::Blake3;
use fmine_geom::vdb::VdbGrid;
use std::process::Command;

/// 465³ = 100,544,625 ≥ 10⁸. The box starts off the leaf grid, so its boundary leaves are partial.
const SIDE: i32 = 465;
const ORIGIN: [i32; 3] = [-229, -3, 1_000_003];

fn value(c: [i32; 3]) -> u64 {
    let (x, y, z) = (c[0] as u64, c[1] as u64, c[2] as u64);
    (x.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ y.wrapping_mul(0xC2B2_AE3D_27D4_EB4F))
        .wrapping_add(z.wrapping_mul(0x1656_67B1_9E37_79F9))
}

/// `None` when `ps` cannot run or prints something other than a KiB count; the test then fails rather than guess.
fn rss_bytes() -> Option<u64> {
    let out = Command::new("ps")
        .args(["-o", "rss=", "-p", &std::process::id().to_string()])
        .output()
        .ok()?;
    let kib: u64 = String::from_utf8_lossy(&out.stdout).trim().parse().ok()?;
    Some(kib * 1024)
}

fn build(reverse: bool) -> VdbGrid<u64> {
    let mut grid = VdbGrid::new(0u64);
    for k in 0..SIDE {
        for j in 0..SIDE {
            for i in 0..SIDE {
                let (i, j, k) = if reverse {
                    (SIDE - 1 - i, SIDE - 1 - j, SIDE - 1 - k)
                } else {
                    (i, j, k)
                };
                let c = [ORIGIN[0] + i, ORIGIN[1] + j, ORIGIN[2] + k];
                grid.set(c, value(c));
            }
        }
    }
    grid
}

/// BLAKE3 over (x, y, z, value), little-endian, in `iter_active` order. (The strict-order check needs the private
/// key split, so it lives in the unit test `golden_iter_active_digest`.)
fn digest(grid: &VdbGrid<u64>) -> (u64, String) {
    let mut h = Blake3::new();
    let mut n = 0u64;
    let mut buf = Vec::with_capacity(1 << 20);
    for (c, v) in grid.iter_active() {
        for x in c {
            buf.extend_from_slice(&x.to_le_bytes());
        }
        buf.extend_from_slice(&v.to_le_bytes());
        if buf.len() >= (1 << 20) - 20 {
            h.update(&buf);
            buf.clear();
        }
        n += 1;
    }
    h.update(&buf);
    (n, h.finalize().to_hex())
}

#[test]
#[ignore = "GATE-BM-01: 10^8 cells, about 1 GB; run in release"]
fn gate_bm_01_1e8_cells_within_2_5x_payload_with_deterministic_digest() {
    let rss_before = rss_bytes().expect("ps -o rss= reports this process's RSS in KiB");
    let t0 = std::time::Instant::now();
    let grid = build(false);
    let build_s = t0.elapsed().as_secs_f64();
    let rss_after = rss_bytes().expect("ps -o rss= reports this process's RSS in KiB");
    let active = grid.active_count();
    assert_eq!(active, (SIDE as u64).pow(3));
    assert!(
        active >= 100_000_000,
        "fewer than 10^8 active cells: {active}"
    );
    let payload = active * 8;
    let resident = rss_after.saturating_sub(rss_before);
    let ratio = resident as f64 / payload as f64;
    let stats = grid.memory_stats();
    let t1 = std::time::Instant::now();
    let (n1, d1) = digest(&grid);
    let digest_s = t1.elapsed().as_secs_f64();
    assert_eq!(n1, active);
    drop(grid);
    let (n2, d2) = digest(&build(true));
    println!(
        "{{\"gate\":\"GATE-BM-01\",\"active\":{active},\"payload_bytes\":{payload},\"rss_delta_bytes\":{resident},\
         \"ratio\":{ratio:.3},\"layout_estimate\":{},\"build_s\":{build_s:.1},\"digest_s\":{digest_s:.1},\
         \"digest\":\"{d1}\"}}",
        stats.to_json()
    );
    assert!(
        ratio <= 2.5,
        "resident {resident} B is {ratio:.3}x the {payload} B payload (limit 2.5x)"
    );
    assert_eq!(
        (n2, &d2),
        (n1, &d1),
        "insertion order changed the iter_active digest"
    );
    // Pinned from the first run (2026-11-01, aarch64-macos); the reverse-order build matched it in that run.
    assert_eq!(
        d1, "495a719a42a27ad18415b24a590e8a9291d777ad052d84c81d0dda6346a64de5",
        "pinned GATE-BM-01 digest"
    );
}
