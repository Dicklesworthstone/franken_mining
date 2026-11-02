//! Test oracles for `pdlp`, sharing no code with it: vertex enumeration for tiny bounded LPs, and brute force over
//! all subsets for closure LPs.

#![allow(dead_code)]
// Test helpers outside `#[test]` functions: a malformed generated problem must fail the test at once.
#![allow(clippy::unwrap_used)]

use fmine_lp::pdlp::LpProblem;
use fmine_numerics::sparse::Coo;

/// SplitMix64.
pub struct Rng(pub u64);

impl Rng {
    pub fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    pub fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }

    /// Uniform in `[lo, hi)`.
    pub fn uniform(&mut self, lo: f64, hi: f64) -> f64 {
        lo + (hi - lo) * ((self.next() >> 11) as f64 / (1u64 << 53) as f64)
    }
}

/// A small LP held densely: `min c·x`, `lo ≤ A x ≤ hi`, `l ≤ x ≤ u`.
#[derive(Debug, Clone)]
pub struct Dense {
    pub c: Vec<f64>,
    pub a: Vec<Vec<f64>>,
    pub lo: Vec<f64>,
    pub hi: Vec<f64>,
    pub l: Vec<f64>,
    pub u: Vec<f64>,
}

impl Dense {
    pub fn problem(&self) -> LpProblem {
        let n = self.c.len();
        let mut coo = Coo::new(self.a.len(), n).unwrap();
        for (i, row) in self.a.iter().enumerate() {
            for (j, &v) in row.iter().enumerate() {
                if v != 0.0 {
                    coo.push(i, j, v).unwrap();
                }
            }
        }
        LpProblem::new(
            self.c.clone(),
            &coo.assemble(),
            self.lo.clone(),
            self.hi.clone(),
            self.l.clone(),
            self.u.clone(),
        )
        .unwrap()
    }

    fn feasible(&self, x: &[f64]) -> bool {
        let tol = |b: f64| 1e-9 * (1.0 + b.abs());
        for ((&v, &l), &u) in x.iter().zip(&self.l).zip(&self.u) {
            if v < l - tol(l) || v > u + tol(u) {
                return false;
            }
        }
        for ((row, &lo), &hi) in self.a.iter().zip(&self.lo).zip(&self.hi) {
            let ax: f64 = row.iter().zip(x).map(|(a, v)| a * v).sum();
            if (lo.is_finite() && ax < lo - tol(lo)) || (hi.is_finite() && ax > hi + tol(hi)) {
                return false;
            }
        }
        true
    }

    /// The minimum over all vertices: every choice of `n` tight constraints (row sides and finite column bounds)
    /// whose system is nonsingular and whose solution is feasible. `None` if no vertex is feasible. Needs every
    /// column bound finite, so that a feasible problem has a vertex optimum.
    pub fn vertex_optimum(&self) -> Option<f64> {
        let n = self.c.len();
        let mut planes: Vec<(Vec<f64>, f64)> = Vec::new();
        for ((row, &lo), &hi) in self.a.iter().zip(&self.lo).zip(&self.hi) {
            if lo.is_finite() {
                planes.push((row.clone(), lo));
            }
            if hi.is_finite() && hi != lo {
                planes.push((row.clone(), hi));
            }
        }
        for j in 0..n {
            let e: Vec<f64> = (0..n).map(|k| f64::from(u8::from(k == j))).collect();
            planes.push((e.clone(), self.l[j]));
            if self.u[j] != self.l[j] {
                planes.push((e, self.u[j]));
            }
        }
        let mut best: Option<f64> = None;
        let mut pick = Vec::with_capacity(n);
        self.choose(&planes, 0, &mut pick, &mut best);
        best
    }

    fn choose(
        &self,
        planes: &[(Vec<f64>, f64)],
        from: usize,
        pick: &mut Vec<usize>,
        best: &mut Option<f64>,
    ) {
        let n = self.c.len();
        if pick.len() == n {
            if let Some(x) = solve_dense(pick.iter().map(|&k| &planes[k]).collect())
                && self.feasible(&x)
            {
                let v: f64 = self.c.iter().zip(&x).map(|(c, v)| c * v).sum();
                *best = Some(best.map_or(v, |b: f64| b.min(v)));
            }
            return;
        }
        for k in from..planes.len() {
            pick.push(k);
            self.choose(planes, k + 1, pick, best);
            pick.pop();
        }
    }
}

/// Gaussian elimination with partial pivoting; `None` for a (near) singular system.
fn solve_dense(rows: Vec<&(Vec<f64>, f64)>) -> Option<Vec<f64>> {
    let n = rows.len();
    let mut m: Vec<Vec<f64>> = rows
        .iter()
        .map(|(a, b)| {
            let mut r = a.clone();
            r.push(*b);
            r
        })
        .collect();
    for col in 0..n {
        let piv = (col..n).max_by(|&p, &q| m[p][col].abs().total_cmp(&m[q][col].abs()))?;
        if m[piv][col].abs() < 1e-9 {
            return None;
        }
        m.swap(col, piv);
        for r in 0..n {
            if r != col {
                let f = m[r][col] / m[col][col];
                for k in col..=n {
                    let v = m[col][k];
                    m[r][k] -= f * v;
                }
            }
        }
    }
    Some((0..n).map(|i| m[i][n] / m[i][i]).collect())
}

/// A random feasible LP with `n ≤ 4` columns in finite boxes and `m ≤ 4` rows of mixed kinds. A random point of
/// the box fixes the row bounds, so the LP is feasible. With `open_tops`, some columns lose their upper bound.
pub fn random_dense(rng: &mut Rng, open_tops: bool) -> Dense {
    let n = 1 + rng.below(4) as usize;
    let m = rng.below(5) as usize;
    let mut l = Vec::new();
    let mut u = Vec::new();
    let mut x0 = Vec::new();
    for _ in 0..n {
        let lo = rng.uniform(-3.0, 1.0);
        let hi = lo + rng.uniform(0.5, 4.0);
        x0.push(rng.uniform(lo, hi));
        l.push(lo);
        u.push(if open_tops && rng.below(3) == 0 {
            f64::INFINITY
        } else {
            hi
        });
    }
    let c: Vec<f64> = (0..n).map(|_| rng.below(11) as f64 - 5.0).collect();
    let mut a = Vec::new();
    let (mut lo, mut hi) = (Vec::new(), Vec::new());
    for _ in 0..m {
        let row: Vec<f64> = (0..n)
            .map(|_| {
                if rng.below(4) == 0 {
                    0.0
                } else {
                    rng.below(11) as f64 - 5.0
                }
            })
            .collect();
        let v: f64 = row.iter().zip(&x0).map(|(a, x)| a * x).sum();
        match rng.below(5) {
            0 => {
                lo.push(v);
                hi.push(v);
            }
            1 => {
                lo.push(f64::NEG_INFINITY);
                hi.push(v + rng.uniform(0.0, 2.0));
            }
            2 => {
                lo.push(v - rng.uniform(0.0, 2.0));
                hi.push(f64::INFINITY);
            }
            _ => {
                lo.push(v - rng.uniform(0.0, 2.0));
                hi.push(v + rng.uniform(0.0, 2.0));
            }
        }
        a.push(row);
    }
    Dense { c, a, lo, hi, l, u }
}

/// A small open-pit scheduling LP shaped like MineLib's CPIT relaxations, in "by" variables `w[b][t]` (index
/// `t·n + b`, in `[0, 1]`): a vertical section `width` blocks wide and `depth` deep, where each block below the top
/// needs the three above it, `periods` periods, and one capacity row per period on the blocks mined in it. Values
/// are random ore or waste, discounted 10% a period. Returned as a minimisation (negated values).
pub fn random_cpit(rng: &mut Rng, width: usize, depth: usize, periods: usize) -> LpProblem {
    let n = width * depth;
    let value: Vec<f64> = (0..n)
        .map(|_| {
            if rng.below(3) == 0 {
                rng.uniform(5.0, 40.0)
            } else {
                -rng.uniform(1.0, 4.0)
            }
        })
        .collect();
    let cap = (n as f64 / periods as f64 * rng.uniform(0.3, 0.8)).max(1.0);
    let col = |b: usize, t: usize| t * n + b;
    let mut rows: Vec<Vec<(usize, f64)>> = Vec::new();
    let (mut lo, mut hi) = (Vec::new(), Vec::new());
    for t in 0..periods {
        for k in 1..depth {
            for i in 0..width {
                let b = k * width + i;
                for di in [-1i64, 0, 1] {
                    let j = i as i64 + di;
                    if j >= 0 && (j as usize) < width {
                        let q = (k - 1) * width + j as usize;
                        rows.push(vec![(col(b, t), 1.0), (col(q, t), -1.0)]);
                        lo.push(f64::NEG_INFINITY);
                        hi.push(0.0);
                    }
                }
            }
        }
        if t > 0 {
            for b in 0..n {
                rows.push(vec![(col(b, t - 1), 1.0), (col(b, t), -1.0)]);
                lo.push(f64::NEG_INFINITY);
                hi.push(0.0);
            }
        }
        let mut cap_row: Vec<(usize, f64)> = (0..n).map(|b| (col(b, t), 1.0)).collect();
        if t > 0 {
            cap_row.extend((0..n).map(|b| (col(b, t - 1), -1.0)));
        }
        rows.push(cap_row);
        lo.push(f64::NEG_INFINITY);
        hi.push(cap);
    }
    let mut c = vec![0.0f64; n * periods];
    for t in 0..periods {
        let disc = 1.1f64.powi(-(t as i32));
        for b in 0..n {
            // Mining b in period t is w[b][t] − w[b][t−1]: its value enters w[b][t] with +disc_t and w[b][t−1]
            // with −disc_t, negated for the minimisation.
            c[col(b, t)] -= value[b] * disc;
            if t > 0 {
                c[col(b, t - 1)] += value[b] * disc;
            }
        }
    }
    let mut coo = Coo::new(rows.len(), n * periods).unwrap();
    for (r, row) in rows.iter().enumerate() {
        for &(j, v) in row {
            coo.push(r, j, v).unwrap();
        }
    }
    LpProblem::new(
        c,
        &coo.assemble(),
        lo,
        hi,
        vec![0.0; n * periods],
        vec![1.0; n * periods],
    )
    .unwrap()
}

/// A random closure LP on `n ≤ 12` blocks: minimise `−w·x` subject to `x_u ≤ x_v` for each arc and `0 ≤ x ≤ 1`.
/// Returns it with the best closure's value (a maximum), found over all subsets.
pub fn random_closure(rng: &mut Rng) -> (Dense, f64) {
    let n = 2 + rng.below(11) as usize;
    let w: Vec<f64> = (0..n).map(|_| rng.below(21) as f64 - 10.0).collect();
    let mut arcs = Vec::new();
    for v in 0..n {
        for t in 0..n {
            if v != t && rng.below(5) == 0 {
                arcs.push((v, t));
            }
        }
    }
    let mut best = 0.0f64;
    for set in 0u32..(1u32 << n) {
        let has = |b: usize| set >> b & 1 == 1;
        if arcs.iter().all(|&(v, t)| !has(v) || has(t)) {
            let val: f64 = (0..n).filter(|&b| has(b)).map(|b| w[b]).sum();
            best = best.max(val);
        }
    }
    let a = arcs
        .iter()
        .map(|&(v, t)| {
            (0..n)
                .map(|k| {
                    if k == v {
                        1.0
                    } else if k == t {
                        -1.0
                    } else {
                        0.0
                    }
                })
                .collect()
        })
        .collect();
    let dense = Dense {
        c: w.iter().map(|v| -v).collect(),
        a,
        lo: vec![f64::NEG_INFINITY; arcs.len()],
        hi: vec![0.0; arcs.len()],
        l: vec![0.0; n],
        u: vec![1.0; n],
    };
    (dense, best)
}
