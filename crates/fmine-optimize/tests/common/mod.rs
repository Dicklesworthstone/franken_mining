//! Test oracles for `pitflow`, sharing no code with it.
//!
//! The maximum closure is computed from a Dinic maximum flow on the usual s–t network: a source arc of capacity `w`
//! to each positive block, a sink arc of capacity `-w` from each negative block, and an arc of capacity larger than
//! every cut along each precedence. The best closure is worth the positive total minus the maximum flow.

#![allow(dead_code)]

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
}

struct Edge {
    to: usize,
    cap: i128,
}

/// Value of a maximum-weight closure, by Dinic's algorithm with an iterative depth-first search.
pub fn max_closure_value(weights: &[i64], requires: &[Vec<u32>]) -> i64 {
    let n = weights.len();
    let (s, t) = (n, n + 1);
    let mut adj: Vec<Vec<usize>> = vec![Vec::new(); n + 2];
    let mut edges: Vec<Edge> = Vec::new();
    let mut add = |u: usize, v: usize, cap: i128| {
        adj[u].push(edges.len());
        edges.push(Edge { to: v, cap });
        adj[v].push(edges.len());
        edges.push(Edge { to: u, cap: 0 });
    };
    let positive: i128 = weights
        .iter()
        .filter(|&&w| w > 0)
        .map(|&w| i128::from(w))
        .sum();
    let unbounded: i128 = weights.iter().map(|&w| i128::from(w).abs()).sum::<i128>() + 1;
    for (v, &w) in weights.iter().enumerate() {
        if w > 0 {
            add(s, v, i128::from(w));
        } else if w < 0 {
            add(v, t, -i128::from(w));
        }
    }
    for (u, r) in requires.iter().enumerate() {
        for &v in r {
            add(u, v as usize, unbounded);
        }
    }
    let mut flow: i128 = 0;
    loop {
        let mut level = vec![-1i64; n + 2];
        level[s] = 0;
        let mut queue = std::collections::VecDeque::from([s]);
        while let Some(u) = queue.pop_front() {
            for &e in &adj[u] {
                let v = edges[e].to;
                if edges[e].cap > 0 && level[v] < 0 {
                    level[v] = level[u] + 1;
                    queue.push_back(v);
                }
            }
        }
        if level[t] < 0 {
            break;
        }
        let mut next = vec![0usize; n + 2];
        loop {
            let mut path: Vec<usize> = Vec::new();
            let mut u = s;
            let reached = loop {
                if u == t {
                    break true;
                }
                let mut advanced = false;
                while next[u] < adj[u].len() {
                    let e = adj[u][next[u]];
                    let v = edges[e].to;
                    if edges[e].cap > 0 && level[v] == level[u] + 1 {
                        path.push(e);
                        u = v;
                        advanced = true;
                        break;
                    }
                    next[u] += 1;
                }
                if !advanced {
                    let Some(e) = path.pop() else {
                        break false;
                    };
                    level[u] = -1;
                    u = edges[e ^ 1].to;
                    next[u] += 1;
                }
            };
            if !reached {
                break;
            }
            let push = path.iter().map(|&e| edges[e].cap).min().unwrap_or(0);
            for &e in &path {
                edges[e].cap -= push;
                edges[e ^ 1].cap += push;
            }
            flow += push;
        }
    }
    i64::try_from(positive - flow).unwrap_or(i64::MIN)
}

/// A block model `nx × ny × nz` (`k = 0` at surface) where each block below the top requires the 3 × 3 blocks
/// above it that exist (45° slopes on a cubic grid). Values: waste costs `waste`; `bodies` ellipsoids add grade
/// value, integer only.
pub fn block_model(
    rng: &mut Rng,
    (nx, ny, nz): (usize, usize, usize),
    bodies: usize,
    waste: i64,
) -> (Vec<i64>, Vec<Vec<u32>>) {
    let idx = |i: usize, j: usize, k: usize| (k * ny + j) * nx + i;
    let mut weights = vec![-waste; nx * ny * nz];
    for _ in 0..bodies {
        let c = [
            rng.below(nx as u64) as i64,
            rng.below(ny as u64) as i64,
            (nz / 3 + rng.below((2 * nz / 3).max(1) as u64) as usize) as i64,
        ];
        let r = [
            2 + rng.below(4) as i64,
            2 + rng.below(4) as i64,
            1 + rng.below(3) as i64,
        ];
        let grade = 3 * waste + rng.below(8 * waste as u64 + 1) as i64;
        for k in 0..nz {
            for j in 0..ny {
                for i in 0..nx {
                    let d = [i as i64 - c[0], j as i64 - c[1], k as i64 - c[2]];
                    // Inside when Σ (dᵢ/rᵢ)² ≤ 1, in integers: Σ dᵢ² Π_{j≠i} rⱼ² ≤ Π rᵢ².
                    let r2 = [r[0] * r[0], r[1] * r[1], r[2] * r[2]];
                    let lhs = d[0] * d[0] * r2[1] * r2[2]
                        + d[1] * d[1] * r2[0] * r2[2]
                        + d[2] * d[2] * r2[0] * r2[1];
                    if lhs <= r2[0] * r2[1] * r2[2] {
                        weights[idx(i, j, k)] += grade;
                    }
                }
            }
        }
    }
    let mut requires = vec![Vec::new(); weights.len()];
    for k in 1..nz {
        for j in 0..ny {
            for i in 0..nx {
                for dj in -1i64..=1 {
                    for di in -1i64..=1 {
                        let (ii, jj) = (i as i64 + di, j as i64 + dj);
                        if (0..nx as i64).contains(&ii) && (0..ny as i64).contains(&jj) {
                            requires[idx(i, j, k)]
                                .push(idx(ii as usize, jj as usize, k - 1) as u32);
                        }
                    }
                }
            }
        }
    }
    (weights, requires)
}
