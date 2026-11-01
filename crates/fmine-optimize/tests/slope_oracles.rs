//! Slope templates (`pitflow::slope`, `bd-t75z`) against a brute-force oracle written separately here.
//!
//! The oracle gives every block its whole K-level cone, found by testing every block of every layer up to K above it
//! with its own angle order and cone test. The tests check:
//! - on random grids, block sizes, slopes, sectors and domain maps, the reduced arcs give every block exactly the
//!   closure that the full cones give it, and every arc kept is a cone arc;
//! - with one slope everywhere, no block's closure leaves its true cone, at any height;
//! - the pit solved on the reduced arcs is worth the Dinic optimum on the full cones, and is closed under them.
//!
//! What green does not prove:
//! - speed at scale, since these grids hold at most a few thousand blocks;
//! - how far a pit above K layers strays from the true slope, which is not measured;
//! - anything about real geotechnical data, of which the workspace holds none.

mod common;

use common::{Rng, block_model, max_closure_value};
use fmine_evidence::Refusal;
use fmine_optimize::pitflow::slope::{
    BlockGrid, Sector, SlopeArcs, SlopeTable, SlopeTangent, slope_arcs,
};
use fmine_optimize::pitflow::{ClosureProblem, solve};
use std::cmp::Ordering;

/// One domain for the oracle: sector boundaries `(east, north)`, counterclockwise from the first, each with its slope
/// `(rise, run)`.
type Domain = Vec<([i64; 2], [i64; 2])>;

/// Counterclockwise order from east: the upper half-plane, east included, comes first.
fn before_from_east(a: [i64; 2], c: [i64; 2]) -> bool {
    let upper = |v: [i64; 2]| v[1] > 0 || (v[1] == 0 && v[0] > 0);
    match (upper(a), upper(c)) {
        (true, false) => true,
        (false, true) => false,
        _ => a[0] * c[1] - a[1] * c[0] > 0,
    }
}

/// Counterclockwise order from `b0`, built from the order from east: directions at or past `b0` come first.
fn before_from(b0: [i64; 2], a: [i64; 2], c: [i64; 2]) -> bool {
    let past = |v: [i64; 2]| !before_from_east(v, b0);
    match (past(a), past(c)) {
        (true, false) => true,
        (false, true) => false,
        _ => before_from_east(a, c),
    }
}

/// The slope for direction `v`: that of the last boundary not after `v`.
fn tangent_of(domain: &Domain, v: [i64; 2]) -> [i64; 2] {
    let b0 = domain[0].0;
    domain
        .iter()
        .rev()
        .find(|(b, _)| !before_from(b0, v, *b))
        .map_or(domain[0].1, |s| s.1)
}

/// Whether the block `o` away (`o[2]` layers up) lies in the true cone, at any height.
fn in_cone(domain: &Domain, size: [i64; 3], o: [i64; 3]) -> bool {
    let (x, y, z) = (o[0] * size[0], o[1] * size[1], o[2] * size[2]);
    if x == 0 && y == 0 {
        return z > 0;
    }
    let [rise, run] = tangent_of(domain, [x, y]);
    i128::from(x * x + y * y) * i128::from(rise * rise) <= i128::from(z * z) * i128::from(run * run)
}

struct Case {
    n: [i64; 3],
    size: [i64; 3],
    levels: i64,
    domains: Vec<Domain>,
    domain_of: Vec<u16>,
}

impl Case {
    fn blocks(&self) -> usize {
        (self.n[0] * self.n[1] * self.n[2]) as usize
    }

    fn block(&self, [i, j, k]: [i64; 3]) -> usize {
        ((k * self.n[1] + j) * self.n[0] + i) as usize
    }

    fn coords(&self, b: usize) -> [i64; 3] {
        let b = b as i64;
        [
            b % self.n[0],
            (b / self.n[0]) % self.n[1],
            b / (self.n[0] * self.n[1]),
        ]
    }

    /// Every block's K-level cone, in ascending block order.
    fn full_cones(&self) -> Vec<Vec<u32>> {
        (0..self.blocks())
            .map(|u| {
                let [i, j, k] = self.coords(u);
                let domain = &self.domains[usize::from(self.domain_of[u])];
                let mut out = Vec::new();
                for kk in (k - self.levels).max(0)..k {
                    for jj in 0..self.n[1] {
                        for ii in 0..self.n[0] {
                            if in_cone(domain, self.size, [ii - i, jj - j, k - kk]) {
                                out.push(self.block([ii, jj, kk]) as u32);
                            }
                        }
                    }
                }
                out
            })
            .collect()
    }

    fn arcs(&self) -> Result<SlopeArcs, Refusal> {
        let to = |v: i64| u32::try_from(v).unwrap_or(0);
        let grid = BlockGrid::new(self.n.map(to), self.size.map(to))?;
        let mut table = Vec::new();
        for d in &self.domains {
            let mut sectors = Vec::new();
            for &(b, t) in d {
                let tangent = SlopeTangent::new(to(t[0]), to(t[1]))?;
                sectors.push(Sector::new(b[0] as i32, b[1] as i32, tangent)?);
            }
            table.push(sectors);
        }
        slope_arcs(
            &grid,
            &SlopeTable::new(table)?,
            &self.domain_of,
            to(self.levels),
        )
    }
}

/// Each block's closure as a bitset. Arcs only go up, to lower indices, so every target's closure is ready first.
fn closures(n: usize, row: impl Fn(usize) -> Vec<usize>) -> Vec<Vec<u64>> {
    let mut out: Vec<Vec<u64>> = Vec::with_capacity(n);
    for u in 0..n {
        let mut set = vec![0u64; n.div_ceil(64)];
        set[u / 64] |= 1 << (u % 64);
        for t in row(u) {
            assert!(t < u, "arc {u} -> {t} does not go up");
            for (w, x) in set.iter_mut().zip(&out[t]) {
                *w |= x;
            }
        }
        out.push(set);
    }
    out
}

fn kept_row(arcs: &SlopeArcs, u: usize) -> Vec<usize> {
    arcs.targets()[arcs.offsets()[u]..arcs.offsets()[u + 1]]
        .iter()
        .map(|&t| t as usize)
        .collect()
}

/// Up to `sectors` boundaries from random directions, sorted and deduplicated, starting at a random one. Slopes are
/// `rise / run` with `rise` in `1..=6` and `run` in `1..=run_max`.
fn random_domain(rng: &mut Rng, sectors: u64, run_max: u64) -> Domain {
    let mut bounds: Vec<[i64; 2]> = (0..sectors)
        .map(|_| {
            let v = [rng.below(9) as i64 - 4, rng.below(9) as i64 - 4];
            if v == [0, 0] { [1, 0] } else { v }
        })
        .collect();
    let order = |a: &[i64; 2], c: &[i64; 2]| {
        if before_from_east(*a, *c) {
            Ordering::Less
        } else if before_from_east(*c, *a) {
            Ordering::Greater
        } else {
            Ordering::Equal
        }
    };
    bounds.sort_by(order);
    bounds.dedup_by(|a, c| order(a, c) == Ordering::Equal);
    let turn = rng.below(bounds.len() as u64) as usize;
    bounds.rotate_left(turn);
    bounds
        .into_iter()
        .map(|b| (b, [1 + rng.below(6) as i64, 1 + rng.below(run_max) as i64]))
        .collect()
}

/// A domain map: one domain, an east–west split, a top–bottom split, or noise.
fn random_map(rng: &mut Rng, n: [i64; 3], domains: u64) -> Vec<u16> {
    let style = rng.below(4);
    let (cut_i, cut_k) = (rng.below(n[0] as u64) as i64, rng.below(n[2] as u64) as i64);
    let second = u16::from(domains > 1);
    let mut out = Vec::new();
    for k in 0..n[2] {
        for _ in 0..n[1] {
            for i in 0..n[0] {
                out.push(match style {
                    0 => 0,
                    1 if i >= cut_i => second,
                    2 if k >= cut_k => second,
                    3 => rng.below(domains) as u16,
                    _ => 0,
                });
            }
        }
    }
    out
}

struct Checked {
    arcs: SlopeArcs,
    full: Vec<Vec<u32>>,
    got: Vec<Vec<u64>>,
}

/// Builds the arcs and checks them against the oracle: the full-cone count, every arc a cone arc, and every block's
/// closure equal to the full cones' closure.
fn check_case(case: &Case, label: &str) -> Result<Checked, Refusal> {
    let arcs = case.arcs()?;
    let full = case.full_cones();
    let blocks = case.blocks();
    let full_count: u64 = full.iter().map(|r| r.len() as u64).sum();
    assert_eq!(arcs.full_cone_arcs(), full_count, "{label}");
    assert_eq!(arcs.offsets().len(), blocks + 1, "{label}");
    for (u, cone) in full.iter().enumerate() {
        for t in kept_row(&arcs, u) {
            assert!(
                cone.binary_search(&(t as u32)).is_ok(),
                "{label}: arc {u} -> {t} is not in the cone"
            );
        }
    }
    let got = closures(blocks, |u| kept_row(&arcs, u));
    let want = closures(blocks, |u| full[u].iter().map(|&t| t as usize).collect());
    for u in 0..blocks {
        assert_eq!(
            got[u],
            want[u],
            "{label}: block {u} at {:?}",
            case.coords(u)
        );
    }
    Ok(Checked { arcs, full, got })
}

#[test]
fn reduced_arcs_have_the_closure_of_the_full_cones() {
    let mut rng = Rng(0x05_10BE);
    let (mut kept, mut all) = (0usize, 0u64);
    let (mut boundary, mut interior, mut interior_multi) = (0usize, 0usize, 0usize);
    let (mut within, mut extra) = (0usize, 0usize);
    for round in 0..80 {
        let n = [
            3 + rng.below(7) as i64,
            3 + rng.below(7) as i64,
            2 + rng.below(5) as i64,
        ];
        let size = [
            5 + rng.below(11) as i64,
            5 + rng.below(11) as i64,
            5 + rng.below(11) as i64,
        ];
        let levels = 1 + rng.below(4) as i64;
        let count = 1 + rng.below(3);
        let domains: Vec<Domain> = (0..count)
            .map(|_| {
                let s = 1 + rng.below(4);
                random_domain(&mut rng, s, 6)
            })
            .collect();
        let domain_of = random_map(&mut rng, n, count);
        let case = Case {
            n,
            size,
            levels,
            domains,
            domain_of,
        };
        let Checked { arcs, full, got } = check_case(&case, &format!("round {round}")).unwrap();
        let blocks = case.blocks();
        let full_count = arcs.full_cone_arcs();
        // Chains through other sectors or domains can put blocks outside a block's own cone, within K layers.
        for (u, cone) in full.iter().enumerate() {
            let k = case.coords(u)[2];
            for v in (0..u).filter(|&v| got[u][v / 64] >> (v % 64) & 1 == 1) {
                if k - case.coords(v)[2] <= levels {
                    within += 1;
                    extra += usize::from(cone.binary_search(&(v as u32)).is_err());
                }
            }
        }
        kept += arcs.arc_count();
        all += full_count;
        boundary += arcs.boundary_blocks();
        interior += blocks - arcs.boundary_blocks();
        if count > 1 {
            interior_multi += blocks - arcs.boundary_blocks();
        }
    }
    eprintln!(
        "slope_oracles: {kept} arcs kept of {all}; {boundary} boundary blocks, {interior} interior \
         ({interior_multi} with more than one domain); {extra} of {within} closure blocks within K layers lie \
         outside the block's own cone"
    );
    assert!((kept as u64) < all, "no arc was dropped");
    assert!(boundary > 0 && interior > 0 && interior_multi > 0);
}

/// A horizontal domain contact a few layers up. An arc can be implied only through a block two or more layers up,
/// in the other domain. For example, on 12 × 10 × 12 blocks at tan 2 over four layers, (0, −2, 4) is implied only
/// by way of (0, −1, 2). The interior check has to see the contact or the arc is wrongly dropped. The random cases
/// above rarely build this.
#[test]
fn layered_contacts_keep_the_closure() {
    let layered = |n: [i64; 3], cut: i64| -> Vec<u16> {
        (0..n[0] * n[1] * n[2])
            .map(|b| u16::from(b / (n[0] * n[1]) >= cut))
            .collect()
    };
    let steep: Domain = vec![([1, 0], [100, 1])];
    let n = [7, 7, 6];
    let hand = Case {
        n,
        size: [12, 10, 12],
        levels: 4,
        domains: vec![steep.clone(), vec![([1, 0], [4, 2])]],
        domain_of: layered(n, 3),
    };
    check_case(&hand, "hand case").unwrap();
    let mut rng = Rng(0x05_10C1);
    let mut interior = 0;
    for round in 0..60 {
        let n = [
            5 + rng.below(5) as i64,
            5 + rng.below(5) as i64,
            6 + rng.below(3) as i64,
        ];
        let size = [
            5 + rng.below(11) as i64,
            5 + rng.below(11) as i64,
            5 + rng.below(11) as i64,
        ];
        let s = 1 + rng.below(4);
        let mut domains = vec![steep.clone(), random_domain(&mut rng, s, 2)];
        if rng.below(2) == 1 {
            domains.swap(0, 1);
        }
        let cut = 1 + rng.below(n[2] as u64 - 1) as i64;
        let case = Case {
            n,
            size,
            levels: 3 + rng.below(3) as i64,
            domains,
            domain_of: layered(n, cut),
        };
        let checked = check_case(&case, &format!("layered round {round}")).unwrap();
        interior += case.blocks() - checked.arcs.boundary_blocks();
    }
    eprintln!("slope_oracles: {interior} interior blocks in layered rounds");
    assert!(interior > 0);
}

#[test]
fn one_slope_never_reaches_outside_its_true_cone() {
    let mut rng = Rng(0x05_10BF);
    let mut beyond = 0usize;
    for round in 0..40 {
        let n = [
            3 + rng.below(6) as i64,
            3 + rng.below(6) as i64,
            4 + rng.below(6) as i64,
        ];
        let size = [
            5 + rng.below(11) as i64,
            5 + rng.below(11) as i64,
            5 + rng.below(11) as i64,
        ];
        let levels = 1 + rng.below(3) as i64;
        let s = 1 + rng.below(4);
        let mut domain = random_domain(&mut rng, s, 6);
        let slope = domain[0].1;
        for sector in &mut domain {
            sector.1 = slope;
        }
        let case = Case {
            n,
            size,
            levels,
            domain_of: vec![0; (n[0] * n[1] * n[2]) as usize],
            domains: vec![domain],
        };
        let arcs = case.arcs().unwrap();
        let got = closures(case.blocks(), |u| kept_row(&arcs, u));
        for (u, set) in got.iter().enumerate() {
            let [i, j, k] = case.coords(u);
            for v in (0..u).filter(|&v| set[v / 64] >> (v % 64) & 1 == 1) {
                let [vi, vj, vk] = case.coords(v);
                assert!(
                    in_cone(&case.domains[0], size, [vi - i, vj - j, k - vk]),
                    "round {round}: block {u} requires {v}, outside its cone"
                );
                beyond += usize::from(k - vk > levels);
            }
        }
    }
    assert!(beyond > 0, "no closure reached past K layers");
}

#[test]
fn pit_on_slope_arcs_matches_dinic_on_full_cones() {
    let mut rng = Rng(0x05_10C0);
    for (dims, levels) in [((12usize, 12usize, 6usize), 2i64), ((16, 10, 8), 3)] {
        let (weights, _) = block_model(&mut rng, dims, 4, 10);
        let n = [dims.0 as i64, dims.1 as i64, dims.2 as i64];
        let domains = vec![random_domain(&mut rng, 3, 2), random_domain(&mut rng, 2, 2)];
        let domain_of: Vec<u16> = (0..weights.len())
            .map(|b| u16::from(b % dims.0 >= dims.0 / 2))
            .collect();
        let case = Case {
            n,
            size: [10, 10, 10],
            levels,
            domains,
            domain_of,
        };
        let full = case.full_cones();
        let (offsets, targets) = case.arcs().unwrap().into_csr();
        assert!(targets.len() < full.iter().map(Vec::len).sum::<usize>());
        let pit = solve(&ClosureProblem::new(weights.clone(), offsets, targets).unwrap()).unwrap();
        assert_eq!(
            pit.value(),
            max_closure_value(&weights, &full),
            "{dims:?}: pit value against Dinic on the full cones"
        );
        assert!(pit.pit_size() > 0, "{dims:?}: empty pit");
        let in_pit = pit.in_pit();
        for (u, cone) in full.iter().enumerate() {
            assert!(
                !in_pit[u] || cone.iter().all(|&t| in_pit[t as usize]),
                "{dims:?}: block {u} is mined without its cone"
            );
        }
    }
}
