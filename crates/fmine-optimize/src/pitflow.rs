//! Determinism: BitwisePortable (`i64` and `i128` integer arithmetic only; no floating point, hashing or threads).
//!
//! The ultimate pit as a maximum-weight closure, by Hochbaum's pseudoflow algorithm (lowest-label variant with gap
//! termination) on an `i64` CSR precedence graph (`BD-250`, plan §15.4 row `fmine-optimize::pitflow`).
//!
//! A node is a block with an integer value. An arc `u → v` reads "taking `u` requires taking `v`": `v` lies in `u`'s
//! slope cone. A pit is a closed set, one that no arc leaves, and the ultimate pit is a closed set of maximum total
//! value.
//!
//! The answer carries its own certificate: the final pseudoflow on every arc. [`verify`] checks it in O(n + m),
//! independently of the solver:
//! - the pit is closed;
//! - no arc entering the pit carries flow;
//! - every node in the pit has non-negative excess and every node outside it non-positive excess, where a node's
//!   excess is its value plus the flow in minus the flow out.
//!
//! Together these make the pit the source side of a minimum cut of the usual s–t network (source arcs to positive
//! blocks, sink arcs from negative ones, precedence arcs uncapacitated), so no closed set is worth more. [`solve`]
//! runs [`verify`] before returning, and refuses rather than return an uncertified pit.

use fmine_evidence::{Refusal, RefusalCode};

const NONE: u32 = u32::MAX;

/// Plan §15.4's overflow guard: the absolute values must sum to less than 2⁶², or the problem is refused. Every flow
/// and excess is then bounded by the total positive value and fits an `i64` with room to spare.
pub const ABS_VALUE_SUM_LIMIT: i128 = 1 << 62;

fn refuse(detail: String) -> Refusal {
    Refusal::new(RefusalCode::NumericalCertificateFailed207, detail)
}

/// Node values and the precedence arcs in CSR form: the nodes `u` requires are
/// `targets[offsets[u]..offsets[u + 1]]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClosureProblem {
    weights: Vec<i64>,
    offsets: Vec<usize>,
    targets: Vec<u32>,
}

impl ClosureProblem {
    /// Checks the CSR shape and the overflow guard.
    ///
    /// # Errors
    /// `FMINE-REF-207` (proposed) when there are `u32::MAX` or more nodes or arcs, when `offsets` is not
    /// `n + 1` non-decreasing entries from 0 to `targets.len()`, when a target is not a node, or when the absolute
    /// values sum to 2⁶² or more.
    pub fn new(weights: Vec<i64>, offsets: Vec<usize>, targets: Vec<u32>) -> Result<Self, Refusal> {
        let n = weights.len();
        let limit = NONE as usize;
        if n >= limit || targets.len() >= limit {
            return Err(refuse(format!(
                "pitflow: {n} nodes and {} arcs; each must be below {limit}",
                targets.len()
            )));
        }
        if offsets.len() != n + 1
            || offsets.first() != Some(&0)
            || offsets.last() != Some(&targets.len())
            || offsets.windows(2).any(|w| w[0] > w[1])
        {
            return Err(refuse(format!(
                "pitflow: offsets must be {} non-decreasing entries from 0 to {}",
                n + 1,
                targets.len()
            )));
        }
        if let Some((a, &t)) = targets.iter().enumerate().find(|&(_, &t)| t as usize >= n) {
            return Err(refuse(format!(
                "pitflow: arc {a} requires node {t}, but there are {n} nodes"
            )));
        }
        let abs_sum: i128 = weights.iter().map(|&w| i128::from(w).abs()).sum();
        if abs_sum >= ABS_VALUE_SUM_LIMIT {
            return Err(refuse(format!(
                "pitflow: the absolute values sum to {abs_sum}, not below 2^62"
            )));
        }
        Ok(Self {
            weights,
            offsets,
            targets,
        })
    }

    /// Builds the CSR from one list of required nodes per node.
    ///
    /// # Errors
    /// As [`ClosureProblem::new`], and `FMINE-REF-207` when there is not one list per value.
    pub fn from_requirements(weights: Vec<i64>, requires: &[Vec<u32>]) -> Result<Self, Refusal> {
        if requires.len() != weights.len() {
            return Err(refuse(format!(
                "pitflow: {} requirement lists for {} values",
                requires.len(),
                weights.len()
            )));
        }
        let mut offsets = Vec::with_capacity(requires.len() + 1);
        offsets.push(0);
        let mut targets = Vec::with_capacity(requires.iter().map(Vec::len).sum());
        for r in requires {
            targets.extend_from_slice(r);
            offsets.push(targets.len());
        }
        Self::new(weights, offsets, targets)
    }

    /// Number of nodes (blocks).
    #[must_use]
    pub fn node_count(&self) -> usize {
        self.weights.len()
    }

    /// Number of precedence arcs.
    #[must_use]
    pub fn arc_count(&self) -> usize {
        self.targets.len()
    }

    /// The node values.
    #[must_use]
    pub fn weights(&self) -> &[i64] {
        &self.weights
    }

    /// The nodes `u` requires, in CSR order (arc ids `offsets[u]..`); `None` when `u` is not a node.
    #[must_use]
    pub fn requires(&self, u: usize) -> Option<&[u32]> {
        let start = *self.offsets.get(u)?;
        let end = *self.offsets.get(u + 1)?;
        self.targets.get(start..end)
    }

    /// `(arc id, tail, head)` for every arc, in CSR order.
    fn arcs(&self) -> impl Iterator<Item = (usize, usize, usize)> + '_ {
        self.offsets
            .windows(2)
            .enumerate()
            .flat_map(move |(u, w)| (w[0]..w[1]).map(move |a| (a, u, self.targets[a] as usize)))
    }
}

/// Work counters from one [`solve`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PitStats {
    /// Merger arcs used (a strong tree hung below a weak one).
    pub mergers: u64,
    /// Tree edges cut because they could not carry the excess pushed through them.
    pub splits: u64,
    /// Label increments.
    pub relabels: u64,
    /// The largest label any node reached.
    pub max_label: u32,
}

/// A certified ultimate pit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PitSolution {
    in_pit: Vec<bool>,
    value: i64,
    arc_flow: Vec<i64>,
    stats: PitStats,
}

impl PitSolution {
    /// One flag per node: `true` for the blocks in the pit.
    #[must_use]
    pub fn in_pit(&self) -> &[bool] {
        &self.in_pit
    }

    /// The pit's total value: the maximum over all closed sets.
    #[must_use]
    pub fn value(&self) -> i64 {
        self.value
    }

    /// The certificate: the final pseudoflow on each arc, in CSR order.
    #[must_use]
    pub fn arc_flow(&self) -> &[i64] {
        &self.arc_flow
    }

    /// Number of blocks in the pit.
    #[must_use]
    pub fn pit_size(&self) -> usize {
        self.in_pit.iter().filter(|&&b| b).count()
    }

    /// Work counters.
    #[must_use]
    pub fn stats(&self) -> PitStats {
        self.stats
    }
}

/// Checks a pit and its pseudoflow certificate, and returns the pit's value. It shares no code with the solver.
///
/// # Errors
/// `FMINE-REF-207` (proposed) when the lengths do not match the problem, a flow is negative, the pit is not closed,
/// an arc entering the pit carries flow, or an excess has the wrong sign.
pub fn verify(problem: &ClosureProblem, in_pit: &[bool], arc_flow: &[i64]) -> Result<i64, Refusal> {
    if in_pit.len() != problem.node_count() || arc_flow.len() != problem.arc_count() {
        return Err(refuse(format!(
            "pitflow certificate: {} pit flags and {} flows for {} nodes and {} arcs",
            in_pit.len(),
            arc_flow.len(),
            problem.node_count(),
            problem.arc_count()
        )));
    }
    let mut excess: Vec<i128> = problem.weights.iter().map(|&w| i128::from(w)).collect();
    for (a, u, v) in problem.arcs() {
        let f = arc_flow[a];
        if f < 0 {
            return Err(refuse(format!(
                "pitflow certificate: arc {a} ({u} -> {v}) carries negative flow {f}"
            )));
        }
        if in_pit[u] && !in_pit[v] {
            return Err(refuse(format!(
                "pitflow certificate: not closed; {u} is in the pit but requires {v}, which is not"
            )));
        }
        if !in_pit[u] && in_pit[v] && f > 0 {
            return Err(refuse(format!(
                "pitflow certificate: arc {a} ({u} -> {v}) enters the pit with flow {f}, so the cut is not minimal"
            )));
        }
        excess[u] -= i128::from(f);
        excess[v] += i128::from(f);
    }
    if let Some((v, e)) = excess
        .iter()
        .zip(in_pit)
        .enumerate()
        .find_map(|(v, (&e, &inside))| ((inside && e < 0) || (!inside && e > 0)).then_some((v, e)))
    {
        return Err(refuse(format!(
            "pitflow certificate: node {v} ({}) has excess {e}",
            if in_pit[v] {
                "in the pit"
            } else {
                "outside it"
            }
        )));
    }
    let value: i128 = problem
        .weights
        .iter()
        .zip(in_pit)
        .filter(|&(_, &inside)| inside)
        .map(|(&w, _)| i128::from(w))
        .sum();
    i64::try_from(value)
        .map_err(|_| refuse(format!("pitflow certificate: value {value} overflows i64")))
}

/// Solves for a maximum-weight closure and certifies it with [`verify`].
///
/// One maximum closure is returned. When several closed sets tie, which one comes back is fixed by the input but is
/// not promised to be the smallest or the largest.
///
/// # Errors
/// `FMINE-REF-207` (proposed) when an intermediate flow would overflow `i64` or a label `u32`, or when the
/// certificate fails. The overflow guard in [`ClosureProblem::new`] makes the first two unreachable in practice.
pub fn solve(problem: &ClosureProblem) -> Result<PitSolution, Refusal> {
    let mut solver = Solver::new(problem);
    let end = solver.run()?;
    let in_pit: Vec<bool> = match end {
        End::Gap(g) => solver.label.iter().map(|&l| l >= g).collect(),
        End::NoStrongRoot => vec![false; problem.node_count()],
    };
    let value = verify(problem, &in_pit, &solver.flow)?;
    Ok(PitSolution {
        in_pit,
        value,
        arc_flow: solver.flow,
        stats: solver.stats,
    })
}

/// Why the main loop stopped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum End {
    /// The lowest strong label is `g` and no node has label `g - 1`: the nodes labelled `g` or more are the pit.
    Gap(u32),
    /// No strong node is left: the empty pit is optimal.
    NoStrongRoot,
}

/// The normalized forest. Every tree has a root, which alone may hold non-zero excess; a tree is strong when its
/// root's excess is positive and weak otherwise. Invariants (checked by the unit tests after every step):
/// - labels never decrease along a path from a root down its tree;
/// - every residual arc `(x, y)` has `label(x) <= label(y) + 1`;
/// - arcs outside the forest carry no flow, and non-root nodes have zero excess;
/// - every weak root is an original non-positive node that has never been strong, so its label is 0;
/// - every strong root except the one being processed is in the bucket of its label.
struct Solver<'a> {
    problem: &'a ClosureProblem,
    /// Tail of each arc (the CSR row it sits in).
    tail: Vec<u32>,
    flow: Vec<i64>,
    /// Arc ids incident to each node, both directions, self-loops left out.
    inc_off: Vec<usize>,
    inc: Vec<u32>,
    excess: Vec<i64>,
    label: Vec<u32>,
    parent: Vec<u32>,
    parent_arc: Vec<u32>,
    first_child: Vec<u32>,
    next_sib: Vec<u32>,
    prev_sib: Vec<u32>,
    next_scan: Vec<u32>,
    /// Strong roots by label, as singly linked stacks.
    bucket_head: Vec<u32>,
    bucket_next: Vec<u32>,
    label_count: Vec<u64>,
    lowest: usize,
    stats: PitStats,
}

impl<'a> Solver<'a> {
    fn new(problem: &'a ClosureProblem) -> Self {
        let n = problem.node_count();
        let m = problem.arc_count();
        let mut tail = vec![0u32; m];
        let mut degree = vec![0usize; n];
        for (a, u, v) in problem.arcs() {
            tail[a] = u as u32;
            if u != v {
                degree[u] += 1;
                degree[v] += 1;
            }
        }
        let mut inc_off = Vec::with_capacity(n + 1);
        inc_off.push(0);
        for d in &degree {
            let last = inc_off.last().copied().unwrap_or(0);
            inc_off.push(last + d);
        }
        let mut fill: Vec<usize> = inc_off[..n].to_vec();
        let mut inc = vec![0u32; inc_off[n]];
        for (a, u, v) in problem.arcs() {
            if u != v {
                inc[fill[u]] = a as u32;
                fill[u] += 1;
                inc[fill[v]] = a as u32;
                fill[v] += 1;
            }
        }
        let mut s = Solver {
            problem,
            tail,
            flow: vec![0; m],
            inc_off,
            inc,
            excess: problem.weights.clone(),
            label: vec![0; n],
            parent: vec![NONE; n],
            parent_arc: vec![NONE; n],
            first_child: vec![NONE; n],
            next_sib: vec![NONE; n],
            prev_sib: vec![NONE; n],
            next_scan: vec![NONE; n],
            bucket_head: vec![NONE; 2],
            bucket_next: vec![NONE; n],
            label_count: vec![0; 2],
            lowest: 1,
            stats: PitStats::default(),
        };
        for v in 0..n {
            if s.excess[v] > 0 {
                s.label[v] = 1;
                s.label_count[1] += 1;
                s.push_strong(v as u32);
            } else {
                s.label_count[0] += 1;
            }
        }
        s.lowest = 1;
        s
    }

    fn run(&mut self) -> Result<End, Refusal> {
        loop {
            if let Some(end) = self.step()? {
                return Ok(end);
            }
        }
    }

    /// Processes the lowest-labelled strong root once: one merger, or one round of relabels.
    fn step(&mut self) -> Result<Option<End>, Refusal> {
        match self.lowest_strong_root()? {
            Next::Root(r) => {
                self.process_root(r)?;
                Ok(None)
            }
            Next::Stop(end) => Ok(Some(end)),
        }
    }

    fn head(&self, a: u32) -> u32 {
        self.problem.targets[a as usize]
    }

    fn push_strong(&mut self, v: u32) {
        let l = self.label[v as usize] as usize;
        self.bucket_next[v as usize] = self.bucket_head[l];
        self.bucket_head[l] = v;
        self.lowest = self.lowest.min(l);
    }

    fn pop_bucket(&mut self, l: usize) -> Option<u32> {
        let v = self.bucket_head[l];
        if v == NONE {
            return None;
        }
        self.bucket_head[l] = self.bucket_next[v as usize];
        self.bucket_next[v as usize] = NONE;
        Some(v)
    }

    /// `label(v) += 1`, growing the label tables as needed.
    fn relabel(&mut self, v: u32) -> Result<(), Refusal> {
        let l = self.label[v as usize];
        let next = l
            .checked_add(1)
            .filter(|&x| x < NONE)
            .ok_or_else(|| refuse(format!("pitflow: label of node {v} would overflow u32")))?;
        let next_us = next as usize;
        if self.label_count.len() <= next_us {
            self.label_count.resize(next_us + 1, 0);
            self.bucket_head.resize(next_us + 1, NONE);
        }
        self.label_count[l as usize] -= 1;
        self.label_count[next_us] += 1;
        self.label[v as usize] = next;
        self.stats.relabels += 1;
        self.stats.max_label = self.stats.max_label.max(next);
        Ok(())
    }

    fn lowest_strong_root(&mut self) -> Result<Next, Refusal> {
        if self.lowest == 0 {
            // A weak root (label 0) that became strong: lift it to label 1. Its children came from strong trees, so
            // they are labelled 1 or more, and label 1 is valid on every arc out of it.
            while let Some(r) = self.pop_bucket(0) {
                self.relabel(r)?;
                self.push_strong(r);
            }
            self.lowest = 1;
        }
        let mut i = self.lowest;
        while i < self.bucket_head.len() {
            if self.bucket_head[i] != NONE {
                self.lowest = i;
                if self.label_count[i - 1] == 0 {
                    return Ok(Next::Stop(End::Gap(i as u32)));
                }
                return Ok(self
                    .pop_bucket(i)
                    .map_or(Next::Stop(End::NoStrongRoot), Next::Root));
            }
            i += 1;
        }
        self.lowest = self.bucket_head.len();
        Ok(Next::Stop(End::NoStrongRoot))
    }

    /// Depth-first over the nodes of `r`'s tree that carry `r`'s label `l`: the first with a residual arc to a node
    /// labelled `l - 1` (necessarily weak, since `l` is the lowest strong label) gives a merger. A node is relabelled
    /// once none of its remaining children shares its label. If no merger is found, every node labelled `l`,
    /// `r` included, ends at `l + 1` and `r` goes back to its bucket.
    fn process_root(&mut self, r: u32) -> Result<(), Refusal> {
        let l = self.label[r as usize];
        let ru = r as usize;
        self.next_scan[ru] = self.first_child[ru];
        if let Some((a, y)) = self.find_weak(r, l) {
            return self.merge_and_push(r, r, y, a);
        }
        self.check_children(r)?;
        let mut node = r;
        loop {
            while self.next_scan[node as usize] != NONE {
                let child = self.next_scan[node as usize];
                self.next_scan[node as usize] = self.next_sib[child as usize];
                node = child;
                self.next_scan[node as usize] = self.first_child[node as usize];
                if let Some((a, y)) = self.find_weak(node, l) {
                    return self.merge_and_push(r, node, y, a);
                }
                self.check_children(node)?;
            }
            if node == r {
                break;
            }
            node = self.parent[node as usize];
            self.check_children(node)?;
        }
        self.push_strong(r);
        Ok(())
    }

    /// Moves `next_scan(v)` to the next child sharing `v`'s label, or relabels `v` when there is none.
    fn check_children(&mut self, v: u32) -> Result<(), Refusal> {
        let l = self.label[v as usize];
        let mut c = self.next_scan[v as usize];
        while c != NONE {
            if self.label[c as usize] == l {
                self.next_scan[v as usize] = c;
                return Ok(());
            }
            c = self.next_sib[c as usize];
        }
        self.next_scan[v as usize] = NONE;
        self.relabel(v)
    }

    /// A residual arc from `x` to a node labelled `l - 1`, as `(arc, other end)`.
    fn find_weak(&self, x: u32, l: u32) -> Option<(u32, u32)> {
        let target = l.checked_sub(1)?;
        let xu = x as usize;
        self.inc[self.inc_off[xu]..self.inc_off[xu + 1]]
            .iter()
            .find_map(|&a| {
                let forward = self.tail[a as usize] == x;
                let y = if forward {
                    self.head(a)
                } else {
                    self.tail[a as usize]
                };
                // Forward along `x -> y` the arc is uncapacitated; backward it can return the flow it carries.
                (self.label[y as usize] == target && (forward || self.flow[a as usize] > 0))
                    .then_some((a, y))
            })
    }

    fn add_child(&mut self, p: u32, c: u32) {
        let h = self.first_child[p as usize];
        self.next_sib[c as usize] = h;
        self.prev_sib[c as usize] = NONE;
        if h != NONE {
            self.prev_sib[h as usize] = c;
        }
        self.first_child[p as usize] = c;
    }

    fn remove_child(&mut self, p: u32, c: u32) {
        let (prev, next) = (self.prev_sib[c as usize], self.next_sib[c as usize]);
        if prev == NONE {
            self.first_child[p as usize] = next;
        } else {
            self.next_sib[prev as usize] = next;
        }
        if next != NONE {
            self.prev_sib[next as usize] = prev;
        }
        self.prev_sib[c as usize] = NONE;
        self.next_sib[c as usize] = NONE;
    }

    /// Re-roots `r`'s tree at `x`, hangs `x` below the weak node `y` through arc `a`, then pushes `r`'s excess up to
    /// the merged tree's root, splitting off every edge that cannot carry what arrives at it.
    fn merge_and_push(&mut self, r: u32, x: u32, y: u32, a: u32) -> Result<(), Refusal> {
        self.stats.mergers += 1;
        let (mut cur, mut new_parent, mut new_arc) = (x, y, a);
        loop {
            let old_parent = self.parent[cur as usize];
            let old_arc = self.parent_arc[cur as usize];
            if old_parent != NONE {
                self.remove_child(old_parent, cur);
            }
            self.parent[cur as usize] = new_parent;
            self.parent_arc[cur as usize] = new_arc;
            self.add_child(new_parent, cur);
            if old_parent == NONE {
                break;
            }
            (new_parent, new_arc, cur) = (cur, old_arc, old_parent);
        }

        let mut cur = r;
        loop {
            let p = self.parent[cur as usize];
            let amount = self.excess[cur as usize];
            if p == NONE {
                if amount > 0 {
                    self.push_strong(cur);
                }
                return Ok(());
            }
            if amount == 0 {
                return Ok(());
            }
            let arc = self.parent_arc[cur as usize];
            let forward = self.tail[arc as usize] == cur;
            let residual = if forward {
                i64::MAX
            } else {
                self.flow[arc as usize]
            };
            let moved = amount.min(residual);
            let au = arc as usize;
            self.flow[au] = if forward {
                self.flow[au].checked_add(moved)
            } else {
                self.flow[au].checked_sub(moved)
            }
            .ok_or_else(|| refuse(format!("pitflow: flow on arc {arc} would overflow i64")))?;
            self.excess[p as usize] = self.excess[p as usize]
                .checked_add(moved)
                .ok_or_else(|| refuse(format!("pitflow: excess at node {p} would overflow i64")))?;
            self.excess[cur as usize] = amount - moved;
            if moved < amount {
                self.remove_child(p, cur);
                self.parent[cur as usize] = NONE;
                self.parent_arc[cur as usize] = NONE;
                self.stats.splits += 1;
                self.push_strong(cur);
            }
            cur = p;
        }
    }
}

enum Next {
    Root(u32),
    Stop(End),
}

#[cfg(test)]
mod tests {
    use super::*;

    /// SplitMix64, enough for test instances.
    struct Rng(u64);

    impl Rng {
        fn next(&mut self) -> u64 {
            self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
            let mut z = self.0;
            z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
            z ^ (z >> 31)
        }

        fn below(&mut self, n: u64) -> u64 {
            self.next() % n
        }

        fn value(&mut self, span: i64) -> i64 {
            (self.below(2 * span as u64 + 1) as i64) - span
        }
    }

    fn problem(weights: &[i64], requires: &[&[u32]]) -> ClosureProblem {
        let lists: Vec<Vec<u32>> = requires.iter().map(|r| r.to_vec()).collect();
        ClosureProblem::from_requirements(weights.to_vec(), &lists).unwrap()
    }

    /// Every subset, kept when closed; the best value.
    fn brute_force(p: &ClosureProblem) -> i64 {
        let n = p.node_count();
        assert!(n <= 16);
        (0u32..(1 << n))
            .filter(|mask| {
                p.arcs()
                    .all(|(_, u, v)| mask & (1 << u) == 0 || mask & (1 << v) != 0)
            })
            .map(|mask| {
                (0..n)
                    .filter(|&v| mask & (1 << v) != 0)
                    .map(|v| p.weights[v])
                    .sum()
            })
            .max()
            .unwrap()
    }

    fn random_problem(rng: &mut Rng, n: usize, arcs: usize, span: i64) -> ClosureProblem {
        let weights: Vec<i64> = (0..n).map(|_| rng.value(span)).collect();
        let mut lists = vec![Vec::new(); n];
        for _ in 0..arcs {
            let u = rng.below(n as u64) as usize;
            let v = rng.below(n as u64) as u32;
            lists[u].push(v);
        }
        ClosureProblem::from_requirements(weights, &lists).unwrap()
    }

    /// The solver's invariants, checked from scratch.
    fn check_invariants(s: &Solver<'_>) {
        let n = s.problem.node_count();
        // Excess from flows: zero at non-roots, the stored value at roots.
        let mut e: Vec<i128> = s.problem.weights.iter().map(|&w| i128::from(w)).collect();
        let mut in_tree = vec![false; s.flow.len()];
        for v in 0..n {
            if s.parent[v] != NONE {
                in_tree[s.parent_arc[v] as usize] = true;
                let p = s.parent[v] as usize;
                assert!(
                    s.label[p] <= s.label[v],
                    "labels decrease from {p} to its child {v}"
                );
                let a = s.parent_arc[v] as usize;
                let (t, h) = (s.tail[a] as usize, s.problem.targets[a] as usize);
                assert!(
                    (t, h) == (v, p) || (t, h) == (p, v),
                    "parent arc {a} does not join {v} and {p}"
                );
            }
        }
        for (a, u, v) in s.problem.arcs() {
            let f = s.flow[a];
            assert!(f >= 0, "negative flow on arc {a}");
            if !in_tree[a] {
                assert_eq!(f, 0, "arc {a} outside the forest carries flow");
            }
            e[u] -= i128::from(f);
            e[v] += i128::from(f);
            // Validity: u -> v is always residual; v -> u when it carries flow.
            assert!(
                s.label[u] <= s.label[v] + 1,
                "invalid labels on residual arc {u} -> {v}"
            );
            if f > 0 {
                assert!(
                    s.label[v] <= s.label[u] + 1,
                    "invalid labels on residual arc {v} -> {u}"
                );
            }
        }
        let mut bucketed = vec![false; n];
        for (l, &head) in s.bucket_head.iter().enumerate() {
            let mut v = head;
            while v != NONE {
                assert_eq!(s.label[v as usize] as usize, l);
                bucketed[v as usize] = true;
                v = s.bucket_next[v as usize];
            }
        }
        for v in 0..n {
            if s.parent[v] == NONE {
                assert_eq!(e[v], i128::from(s.excess[v]), "root {v} excess");
                if s.excess[v] > 0 {
                    assert!(bucketed[v], "strong root {v} is not bucketed");
                } else {
                    assert_eq!(s.label[v], 0, "weak root {v} has a label");
                    assert!(!bucketed[v], "weak root {v} is bucketed");
                }
            } else {
                assert_eq!(e[v], 0, "non-root {v} has excess");
            }
        }
        let mut counts = vec![0u64; s.label_count.len()];
        for &l in &s.label {
            counts[l as usize] += 1;
        }
        assert_eq!(counts, s.label_count);
    }

    #[test]
    fn two_blocks_by_hand() {
        // Ore worth 5 below waste costing 3: take both, value 2.
        let p = problem(&[5, -3], &[&[1], &[]]);
        let s = solve(&p).unwrap();
        assert_eq!((s.value(), s.in_pit()), (2, &[true, true][..]));
        // Ore worth 2 below waste costing 3: take nothing.
        let p = problem(&[2, -3], &[&[1], &[]]);
        let s = solve(&p).unwrap();
        assert_eq!((s.value(), s.in_pit()), (0, &[false, false][..]));
        // Nothing at all.
        let s = solve(&problem(&[], &[])).unwrap();
        assert_eq!((s.value(), s.pit_size()), (0, 0));
    }

    /// One ore block at depth `k` of a vertical section under 45° slopes needs its cone of `(k + 1)²` blocks.
    #[test]
    fn single_ore_block_cone_threshold() {
        let (w, depth) = (15i64, 7usize);
        let idx = |i: usize, k: usize| k * w as usize + i;
        for k in [0usize, 1, 3, 6] {
            let cone_waste = ((k + 1) * (k + 1) - 1) as i64;
            for (ore, expect) in [(cone_waste, 0), (cone_waste + 1, 1)] {
                let mut weights = vec![-1i64; w as usize * depth];
                weights[idx(7, k)] = ore;
                let mut lists = vec![Vec::new(); weights.len()];
                for kk in 1..depth {
                    for i in 0..w as usize {
                        for di in [-1i64, 0, 1] {
                            let ii = i as i64 + di;
                            if (0..w).contains(&ii) {
                                lists[idx(i, kk)].push(idx(ii as usize, kk - 1) as u32);
                            }
                        }
                    }
                }
                let p = ClosureProblem::from_requirements(weights, &lists).unwrap();
                let s = solve(&p).unwrap();
                assert_eq!(s.value(), expect, "ore {ore} at depth {k}");
                if expect > 0 {
                    assert_eq!(s.pit_size(), (k + 1) * (k + 1));
                }
            }
        }
    }

    #[test]
    fn random_small_instances_match_brute_force_and_keep_invariants() {
        let mut rng = Rng(0x5EED_0250);
        let mut cases = 0;
        let mut nonempty = 0;
        for round in 0..3_000 {
            let n = 1 + (round % 12);
            let arcs = rng.below(3 * n as u64 + 1) as usize;
            let p = random_problem(&mut rng, n, arcs, 9);
            let mut s = Solver::new(&p);
            check_invariants(&s);
            let end = loop {
                if let Some(end) = s.step().unwrap() {
                    break end;
                }
                check_invariants(&s);
            };
            let in_pit: Vec<bool> = match end {
                End::Gap(g) => s.label.iter().map(|&l| l >= g).collect(),
                End::NoStrongRoot => vec![false; n],
            };
            let value = verify(&p, &in_pit, &s.flow).unwrap();
            assert_eq!(value, brute_force(&p), "round {round}");
            assert_eq!(solve(&p).unwrap().value(), value);
            cases += 1;
            nonempty += usize::from(value > 0);
        }
        assert_eq!(cases, 3_000);
        assert!(
            nonempty > 1_000,
            "only {nonempty} instances had a non-empty pit"
        );
    }

    #[test]
    fn rerun_is_identical() {
        let mut rng = Rng(42);
        let p = random_problem(&mut rng, 400, 1_600, 50);
        assert_eq!(solve(&p).unwrap(), solve(&p).unwrap());
    }

    #[test]
    fn verify_refuses_planted_certificates() {
        let mut rng = Rng(7);
        let mut checked = 0;
        for _ in 0..200 {
            let p = random_problem(&mut rng, 30, 60, 20);
            let s = solve(&p).unwrap();
            let (pit, flow) = (s.in_pit().to_vec(), s.arc_flow().to_vec());
            assert_eq!(verify(&p, &pit, &flow).unwrap(), s.value());
            for (a, u, v) in p.arcs() {
                if u == v {
                    continue;
                }
                // Open pit: drop a required node.
                if pit[u] && pit[v] {
                    let mut open = pit.clone();
                    open[v] = false;
                    let r = verify(&p, &open, &flow).unwrap_err();
                    assert_eq!(r.code, "FMINE-REF-207", "{r:?}");
                    checked += 1;
                }
                // Negative flow.
                let mut neg = flow.clone();
                neg[a] = -1;
                assert!(verify(&p, &pit, &neg).is_err());
                // Flow into the pit along an arc from outside.
                if !pit[u] && pit[v] {
                    let mut into = flow.clone();
                    into[a] += 1;
                    assert!(verify(&p, &pit, &into).is_err());
                    checked += 1;
                }
            }
            // A worse closed set with the same flows fails on an excess sign: the empty pit, when the best is not empty.
            if s.value() > 0 {
                assert!(verify(&p, &vec![false; p.node_count()], &flow).is_err());
                checked += 1;
            }
            // Wrong lengths.
            assert!(verify(&p, &pit[1..], &flow).is_err());
        }
        assert!(checked > 500, "only {checked} planted certificates checked");
    }

    #[test]
    fn malformed_input_and_overflow_are_refused() {
        let code = |r: Result<ClosureProblem, Refusal>| r.unwrap_err().code;
        let c207 = "FMINE-REF-207";
        assert_eq!(
            code(ClosureProblem::new(vec![1, 2], vec![0, 1], vec![1])),
            c207
        );
        assert_eq!(
            code(ClosureProblem::new(vec![1, 2], vec![1, 1, 1], vec![1])),
            c207
        );
        assert_eq!(
            code(ClosureProblem::new(vec![1, 2], vec![0, 1, 0], vec![1])),
            c207
        );
        assert_eq!(
            code(ClosureProblem::new(vec![1, 2], vec![0, 1, 1], vec![2])),
            c207
        );
        assert_eq!(code(ClosureProblem::from_requirements(vec![1], &[])), c207);
        let big = 1i64 << 61;
        assert_eq!(
            code(ClosureProblem::new(vec![big, -big], vec![0, 0, 0], vec![])),
            c207
        );
        assert_eq!(
            code(ClosureProblem::new(vec![i64::MIN], vec![0, 0], vec![])),
            c207
        );
        assert!(ClosureProblem::new(vec![big - 1, -big], vec![0, 0, 0], vec![]).is_ok());
        let p = ClosureProblem::new(vec![3, -1, 2], vec![0, 1, 1, 3], vec![1, 0, 2]).unwrap();
        assert_eq!(p.requires(2), Some(&[0u32, 2][..]));
        assert_eq!(p.requires(3), None);
    }

    /// Values near the guard: the largest legal positive mass still solves exactly.
    #[test]
    fn values_near_the_overflow_guard_solve_exactly() {
        let big = (1i64 << 61) - 1;
        // {0, 1} is worth 5 and {0, 1, 2} is worth 9; anything without 0 is worth less than nothing.
        let p = problem(&[big, -(big - 5), 4], &[&[1], &[], &[1]]);
        let s = solve(&p).unwrap();
        assert_eq!(s.value(), 9);
        assert_eq!(s.in_pit(), &[true, true, true]);
    }
}
