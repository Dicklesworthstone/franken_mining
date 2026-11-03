//! Provenance: frankensim crates/fs-rep-mesh/src/winding.rs @ 20463525d9775a18c1b237d2676f364f367175ab; licence LicenseRef-MIT-OpenAI-Anthropic-Rider; Changes: `atan2` is fmine-core's `det::atan2`, not std's; fs-geom's `Point3` and `Vec3` (src/lib.rs at the same commit, without `offset`) are copied in as private types, with two conversions added, and the public API takes `[f64; 3]`; `Soup`'s fields are private and `Soup::new` refuses (FMINE-REF-109) a non-finite coordinate, a corner index past the positions or more than `u32::MAX` triangles, so the private `tri` cannot index out of range; `triangle_winding`, `winding_exact`, `WindingOctree::build`, `winding` and `inside` return `Result`, refusing a non-finite query point (FMINE-REF-109) or arithmetic that leaves the finite range (FMINE-REF-207); `build` refuses a `beta` below 1 or not finite instead of raising it to 1; `WindingOctree` borrows its soup, so no query can pass a different one; an empty soup's octree holds no NaN centroid; a test module is added (the differential corpus against upstream, upstream's conformance checks rmesh-003 and rmesh-004, refusals and a golden digest); Left out: nothing in this file (fs-rep-mesh's `shapes` generators, repair and contouring are not lifted)
//! Determinism: BitwisePortable (IEEE `+ − × ÷`, `sqrt` and `mul_add` in upstream's order; `atan2` is `det::atan2`).
//!
//! Generalized winding numbers (plan §7.2, Barill et al.): ROBUST
//! inside/outside classification on broken input — "the single most
//! effective modern trick in mesh robustness". Exact evaluation sums
//! per-triangle solid angles (van Oosterom–Strackee); the octree DIPOLE
//! approximation aggregates far triangles into area-weighted normal
//! moments so million-triangle soups classify fast, with the
//! approximation error MEASURED against exact on fixtures (rmesh-004).
//!
//! (Upstream's text above; its plan is frankensim's.) Points are `[f64; 3]` in one caller-chosen Cartesian frame,
//! and the winding number is unit-free.

use fmine_core::det::atan2;
use fmine_evidence::{Refusal, RefusalCode};

/// A point in ℝ³. Minimal geometry-local type (fs-la owns real linear
/// algebra; these exist so charts need no L1 dependency).
#[derive(Debug, Clone, Copy, PartialEq)]
struct Point3 {
    /// x coordinate.
    x: f64,
    /// y coordinate.
    y: f64,
    /// z coordinate.
    z: f64,
}

impl Point3 {
    /// Construct a point.
    #[must_use]
    const fn new(x: f64, y: f64, z: f64) -> Self {
        Point3 { x, y, z }
    }

    /// Difference vector `self - other`.
    #[must_use]
    fn delta_from(self, other: Point3) -> Vec3 {
        Vec3 {
            x: self.x - other.x,
            y: self.y - other.y,
            z: self.z - other.z,
        }
    }

    /// The point from the public form, if every coordinate is finite (`FMINE-REF-109`).
    fn finite(what: &str, [x, y, z]: [f64; 3]) -> Result<Self, Refusal> {
        if x.is_finite() && y.is_finite() && z.is_finite() {
            Ok(Point3::new(x, y, z))
        } else {
            Err(Refusal::new(
                RefusalCode::OutsidePhysicalDomain109,
                format!("{what} needs finite coordinates, got {:?}", [x, y, z]),
            ))
        }
    }

    fn array(self) -> [f64; 3] {
        [self.x, self.y, self.z]
    }
}

/// A vector in ℝ³ (see [`Point3`]'s scope note).
#[derive(Debug, Clone, Copy, PartialEq)]
struct Vec3 {
    /// x component.
    x: f64,
    /// y component.
    y: f64,
    /// z component.
    z: f64,
}

impl Vec3 {
    /// Construct a vector.
    #[must_use]
    const fn new(x: f64, y: f64, z: f64) -> Self {
        Vec3 { x, y, z }
    }

    /// Euclidean norm.
    #[must_use]
    fn norm(self) -> f64 {
        self.dot(self).sqrt()
    }

    /// Dot product.
    #[must_use]
    fn dot(self, o: Vec3) -> f64 {
        self.x * o.x + self.y * o.y + self.z * o.z
    }

    /// Scale by a scalar.
    #[must_use]
    fn scale(self, s: f64) -> Vec3 {
        Vec3 {
            x: self.x * s,
            y: self.y * s,
            z: self.z * s,
        }
    }
}

/// `FMINE-REF-207` unless every value is finite.
fn finite_arithmetic(what: &str, values: &[f64]) -> Result<(), Refusal> {
    if values.iter().all(|v| v.is_finite()) {
        Ok(())
    } else {
        Err(Refusal::new(
            RefusalCode::NumericalCertificateFailed207,
            format!("{what} arithmetic left the finite range: {values:?}"),
        ))
    }
}

/// A triangle soup (no connectivity assumed — that is the point).
#[derive(Debug, Clone, PartialEq)]
pub struct Soup {
    /// Vertex positions.
    positions: Vec<Point3>,
    /// Triangles as vertex index triples.
    triangles: Vec<[u32; 3]>,
}

impl Soup {
    /// A soup over `positions`, whose triangles index into them.
    ///
    /// # Errors
    /// `FMINE-REF-109` for a non-finite coordinate, a corner index not below `positions.len()`, or more than
    /// `u32::MAX` triangles (the octree names triangles by `u32`).
    pub fn new(positions: Vec<[f64; 3]>, triangles: Vec<[u32; 3]>) -> Result<Self, Refusal> {
        let positions = positions
            .into_iter()
            .enumerate()
            .map(|(i, p)| Point3::finite(&format!("soup position {i}"), p))
            .collect::<Result<Vec<_>, _>>()?;
        if u32::try_from(triangles.len()).is_err() {
            return Err(Refusal::new(
                RefusalCode::OutsidePhysicalDomain109,
                format!(
                    "a soup holds at most {} triangles, got {}",
                    u32::MAX,
                    triangles.len()
                ),
            ));
        }
        let n = positions.len();
        if let Some(t) = triangles
            .iter()
            .position(|tri| tri.iter().any(|&i| i as usize >= n))
        {
            return Err(Refusal::new(
                RefusalCode::OutsidePhysicalDomain109,
                format!(
                    "triangle {t} is {:?}, past the soup's {n} positions",
                    triangles[t]
                ),
            ));
        }
        Ok(Soup {
            positions,
            triangles,
        })
    }

    /// The number of vertex positions.
    #[must_use]
    pub fn position_count(&self) -> usize {
        self.positions.len()
    }

    /// Vertex position `i`, or `None` past the last one.
    #[must_use]
    pub fn position(&self, i: usize) -> Option<[f64; 3]> {
        self.positions.get(i).map(|p| p.array())
    }

    /// The triangles as corner-index triples.
    #[must_use]
    pub fn triangles(&self) -> &[[u32; 3]] {
        &self.triangles
    }

    /// The corner positions of triangle `t`, or `None` past the last triangle.
    #[must_use]
    pub fn corners(&self, t: usize) -> Option<[[f64; 3]; 3]> {
        (t < self.triangles.len()).then(|| self.tri(t).map(Point3::array))
    }

    /// Triangle corner positions. `Soup::new` checked every index, and callers pass `t` below the triangle count.
    fn tri(&self, t: usize) -> [Point3; 3] {
        let [a, b, c] = self.triangles[t];
        [
            self.positions[a as usize],
            self.positions[b as usize],
            self.positions[c as usize],
        ]
    }
}

/// Solid angle of triangle `[a, b, c]` seen from `p`, divided by 4π
/// (van Oosterom–Strackee — exact up to floating point).
fn solid_angle(p: Point3, a: Point3, b: Point3, c: Point3) -> Result<f64, Refusal> {
    let (va, vb, vc) = (a.delta_from(p), b.delta_from(p), c.delta_from(p));
    let (la, lb, lc) = (va.norm(), vb.norm(), vc.norm());
    let det = va.dot(cross(vb, vc));
    let denom = la * lb * lc + va.dot(vb) * lc + vb.dot(vc) * la + vc.dot(va) * lb;
    finite_arithmetic("triangle_winding", &[det, denom])?;
    let omega = 2.0 * atan2(det, denom);
    Ok(omega / (4.0 * core::f64::consts::PI))
}

/// The solid angle of triangle `[a, b, c]` seen from `p`, divided by 4π, in `[−½, ½]`. A point on the triangle's
/// plane outside it gets `±0`, one inside it `±½` by the sign of the zero determinant, and a point on a corner
/// `0` or `±½` by the signs of zero: upstream's rule, unchanged.
///
/// # Errors
/// `FMINE-REF-109` for a non-finite coordinate, and `FMINE-REF-207` when the determinant or the denominator
/// overflows (coordinate differences near `1e100` or more).
pub fn triangle_winding(
    p: [f64; 3],
    a: [f64; 3],
    b: [f64; 3],
    c: [f64; 3],
) -> Result<f64, Refusal> {
    solid_angle(
        Point3::finite("triangle_winding query point", p)?,
        Point3::finite("triangle_winding corner a", a)?,
        Point3::finite("triangle_winding corner b", b)?,
        Point3::finite("triangle_winding corner c", c)?,
    )
}

fn cross(a: Vec3, b: Vec3) -> Vec3 {
    Vec3::new(
        a.y * b.z - a.z * b.y,
        a.z * b.x - a.x * b.z,
        a.x * b.y - a.y * b.x,
    )
}

/// Exact generalized winding number of the whole soup at `p`: every triangle's [`triangle_winding`], summed in
/// triangle order from `−0` as `Iterator::sum` does, so an empty soup gives `−0`.
///
/// # Errors
/// `FMINE-REF-109` for a non-finite `p`, and `FMINE-REF-207` when a triangle's arithmetic overflows.
pub fn winding_exact(soup: &Soup, p: [f64; 3]) -> Result<f64, Refusal> {
    let p = Point3::finite("winding_exact query point", p)?;
    (0..soup.triangles.len())
        .map(|t| {
            let [a, b, c] = soup.tri(t);
            solid_angle(p, a, b, c)
        })
        .sum()
}

/// One octree node of the dipole hierarchy.
#[derive(Debug, Clone)]
struct DipoleNode {
    /// Aggregate area-weighted normal (the dipole moment).
    moment: Vec3,
    /// Area-weighted centroid.
    centroid: Point3,
    /// Node bounding radius around the centroid.
    radius: f64,
    /// Triangles owned directly (leaves).
    tris: Vec<u32>,
    /// Child nodes (empty at leaves).
    children: Vec<DipoleNode>,
}

/// The dipole-accelerated winding evaluator (Barill et al. §3): far nodes
/// contribute the dipole term; near nodes recurse; leaves are exact. The
/// octree stores triangle INDICES and borrows the soup it was built over.
#[derive(Debug, Clone)]
pub struct WindingOctree<'a> {
    soup: &'a Soup,
    root: DipoleNode,
    /// Accuracy knob β: nodes farther than `β·radius` use the dipole term.
    beta: f64,
}

impl<'a> WindingOctree<'a> {
    /// Build over a soup with accuracy parameter `beta` (≥ 1; larger =
    /// more accurate, slower; 2.0 is the paper's sweet spot).
    ///
    /// # Errors
    /// `FMINE-REF-109` for a `beta` below 1 or not finite (upstream raised it to 1), and `FMINE-REF-207` when a
    /// node's centroid, radius or moment overflows.
    pub fn build(soup: &'a Soup, beta: f64) -> Result<Self, Refusal> {
        if !(beta.is_finite() && beta >= 1.0) {
            return Err(Refusal::new(
                RefusalCode::OutsidePhysicalDomain109,
                format!("WindingOctree beta must be finite and at least 1, got {beta}"),
            ));
        }
        let root = if soup.triangles.is_empty() {
            // Upstream divides by a zero weight here and keeps a NaN centroid that no query reads.
            DipoleNode {
                moment: Vec3::new(0.0, 0.0, 0.0),
                centroid: Point3::new(0.0, 0.0, 0.0),
                radius: 0.0,
                tris: Vec::new(),
                children: Vec::new(),
            }
        } else {
            // `Soup::new` holds the count to `u32::MAX`.
            let tris: Vec<u32> = (0..soup.triangles.len() as u32).collect();
            Self::build_node(soup, tris, 0)?
        };
        Ok(WindingOctree { soup, root, beta })
    }

    /// The soup this octree was built over.
    #[must_use]
    pub fn soup(&self) -> &'a Soup {
        self.soup
    }

    fn tri_centroid_area_normal(soup: &Soup, t: u32) -> (Point3, f64, Vec3) {
        let [a, b, c] = soup.tri(t as usize);
        let n2 = cross(b.delta_from(a), c.delta_from(a));
        let area = 0.5 * n2.norm();
        let centroid = Point3::new(
            (a.x + b.x + c.x) / 3.0,
            (a.y + b.y + c.y) / 3.0,
            (a.z + b.z + c.z) / 3.0,
        );
        (centroid, area, n2.scale(0.5))
    }

    fn build_node(soup: &Soup, tris: Vec<u32>, depth: u32) -> Result<DipoleNode, Refusal> {
        // Aggregate dipole data.
        let mut moment = Vec3::new(0.0, 0.0, 0.0);
        let mut wsum = 0.0f64;
        let mut csum = Vec3::new(0.0, 0.0, 0.0);
        for &t in &tris {
            let (c, area, n) = Self::tri_centroid_area_normal(soup, t);
            moment = Vec3::new(moment.x + n.x, moment.y + n.y, moment.z + n.z);
            // A weight floor, so a zero-area triangle still places the centroid; it stands in for no input.
            let w = area.max(1e-300);
            wsum += w;
            csum = Vec3::new(
                c.x.mul_add(w, csum.x),
                c.y.mul_add(w, csum.y),
                c.z.mul_add(w, csum.z),
            );
        }
        let centroid = Point3::new(csum.x / wsum, csum.y / wsum, csum.z / wsum);
        let mut radius = 0.0f64;
        for &t in &tris {
            let corners = soup.tri(t as usize);
            for p in corners {
                radius = radius.max(p.delta_from(centroid).norm());
            }
        }
        finite_arithmetic(
            "WindingOctree node",
            &[
                moment.x, moment.y, moment.z, centroid.x, centroid.y, centroid.z, radius,
            ],
        )?;
        if tris.len() <= 16 || depth >= 20 {
            return Ok(DipoleNode {
                moment,
                centroid,
                radius,
                tris,
                children: Vec::new(),
            });
        }
        // Octant split around the centroid (by triangle centroid).
        let mut buckets: [Vec<u32>; 8] = Default::default();
        for &t in &tris {
            let (c, _, _) = Self::tri_centroid_area_normal(soup, t);
            let idx = usize::from(c.x >= centroid.x)
                | (usize::from(c.y >= centroid.y) << 1)
                | (usize::from(c.z >= centroid.z) << 2);
            buckets[idx].push(t);
        }
        // Degenerate split (everything in one bucket): stay a leaf.
        if buckets.iter().filter(|b| !b.is_empty()).count() <= 1 {
            return Ok(DipoleNode {
                moment,
                centroid,
                radius,
                tris,
                children: Vec::new(),
            });
        }
        let children = buckets
            .into_iter()
            .filter(|b| !b.is_empty())
            .map(|b| Self::build_node(soup, b, depth + 1))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(DipoleNode {
            moment,
            centroid,
            radius,
            tris: Vec::new(),
            children,
        })
    }

    fn eval_node(&self, soup: &Soup, node: &DipoleNode, p: Point3) -> Result<f64, Refusal> {
        // Flux convention: w(p) = (1/4π) ∮ (x − p)·n dA / |x − p|³, so the
        // dipole term points FROM p TO the mass centroid.
        let d = node.centroid.delta_from(p);
        let dist = d.norm();
        if dist > self.beta * node.radius && dist > 0.0 {
            return Ok(node.moment.dot(d) / (4.0 * core::f64::consts::PI * dist * dist * dist));
        }
        if node.children.is_empty() {
            return node
                .tris
                .iter()
                .map(|&t| {
                    let [a, b, c] = soup.tri(t as usize);
                    solid_angle(p, a, b, c)
                })
                .sum();
        }
        node.children
            .iter()
            .map(|ch| self.eval_node(soup, ch, p))
            .sum()
    }

    /// Approximate generalized winding number at `p`. An empty soup gives `−0`, as [`winding_exact`] does.
    ///
    /// # Errors
    /// `FMINE-REF-109` for a non-finite `p`, and `FMINE-REF-207` when the arithmetic overflows.
    pub fn winding(&self, p: [f64; 3]) -> Result<f64, Refusal> {
        let p = Point3::finite("WindingOctree query point", p)?;
        if self.soup.triangles.is_empty() {
            return Ok(-0.0);
        }
        let w = self.eval_node(self.soup, &self.root, p)?;
        finite_arithmetic("WindingOctree", &[w])?;
        Ok(w)
    }

    /// Classify by the robust `w > 0.5` rule.
    ///
    /// # Errors
    /// As [`WindingOctree::winding`].
    pub fn inside(&self, p: [f64; 3]) -> Result<bool, Refusal> {
        Ok(self.winding(p)? > 0.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fmine_core::hash::sha256;

    /// The differential corpus: `scripts/oracle/winding_corpus/` run on upstream's code at the pinned commit.
    const CORPUS: &str = include_str!("../../../fixtures/fmine-geom/winding_corpus.txt");
    const DELTA: &str =
        include_str!("../../../fixtures/fmine-geom/winding_corpus_aarch64_darwin_delta.txt");
    const BETAS: [f64; 3] = [1.0, 2.0, 4.0];

    /// The largest difference between the lifted winding numbers (`det::atan2`) and upstream's (std `atan2` on an
    /// x86_64 Linux worker) anywhere in the corpus: measured 4.4e-16 over 5,056 values, 3,137 of them identical
    /// bits. The bound leaves room for another host's libm on upstream's side, not for a different rule; the
    /// digest test pins the lifted bits exactly.
    const TOLERANCE: f64 = 1e-13;

    /// A query and upstream's answers: exact, then the octree at each of `BETAS`.
    struct Query {
        p: [f64; 3],
        upstream: [f64; 4],
    }

    struct Case {
        name: String,
        soup: Soup,
        queries: Vec<Query>,
    }

    fn float(hex: &str) -> f64 {
        f64::from_bits(u64::from_str_radix(hex, 16).expect("16 hex digits"))
    }

    fn corpus() -> Vec<Case> {
        let mut cases = Vec::new();
        let mut lines = CORPUS.lines().filter(|l| !l.starts_with('#'));
        while let Some(head) = lines.next() {
            let f: Vec<&str> = head.split(' ').collect();
            assert_eq!(f[0], "soup", "{head}");
            let (np, nt): (usize, usize) =
                (f[2].parse().expect("count"), f[3].parse().expect("count"));
            let mut positions = Vec::with_capacity(np);
            let mut triangles = Vec::with_capacity(nt);
            for _ in 0..np {
                let g: Vec<&str> = lines.next().expect("p line").split(' ').collect();
                assert_eq!(g[0], "p");
                positions.push([float(g[1]), float(g[2]), float(g[3])]);
            }
            for _ in 0..nt {
                let g: Vec<&str> = lines.next().expect("t line").split(' ').collect();
                assert_eq!(g[0], "t");
                triangles.push([1, 2, 3].map(|k| g[k].parse::<u32>().expect("index")));
            }
            let mut queries = Vec::new();
            for line in lines.by_ref() {
                if line == "end" {
                    break;
                }
                let g: Vec<&str> = line.split(' ').collect();
                assert_eq!((g[0], g.len()), ("q", 8), "{line}");
                queries.push(Query {
                    p: [float(g[1]), float(g[2]), float(g[3])],
                    upstream: [float(g[4]), float(g[5]), float(g[6]), float(g[7])],
                });
            }
            let soup = Soup::new(positions, triangles).expect("corpus soups are valid");
            cases.push(Case {
                name: f[1].to_string(),
                soup,
                queries,
            });
        }
        cases
    }

    fn case<'c>(cases: &'c [Case], name: &str) -> &'c Case {
        cases
            .iter()
            .find(|c| c.name == name)
            .expect("soup in the corpus")
    }

    /// The lifted answers at `p`: exact, then the octree at each of `BETAS`.
    fn lifted(soup: &Soup, octrees: &[WindingOctree<'_>], p: [f64; 3]) -> [f64; 4] {
        [
            winding_exact(soup, p).expect("finite"),
            octrees[0].winding(p).expect("finite"),
            octrees[1].winding(p).expect("finite"),
            octrees[2].winding(p).expect("finite"),
        ]
    }

    fn octrees(soup: &Soup) -> Vec<WindingOctree<'_>> {
        BETAS
            .iter()
            .map(|&b| WindingOctree::build(soup, b).expect("beta >= 1"))
            .collect()
    }

    #[test]
    fn differential_corpus_matches_upstream() {
        let cases = corpus();
        assert_eq!(cases.len(), 13, "soups in the corpus");
        let (mut values, mut identical, mut worst, mut classified) = (0usize, 0usize, 0.0_f64, 0);
        for c in &cases {
            let octrees = octrees(&c.soup);
            for q in &c.queries {
                let ours = lifted(&c.soup, &octrees, q.p);
                for (k, (a, b)) in ours.iter().zip(q.upstream).enumerate() {
                    let err = (a - b).abs();
                    assert!(
                        err <= TOLERANCE,
                        "{} at {:?}, value {k}: lifted {a:e}, upstream {b:e}",
                        c.name,
                        q.p
                    );
                    worst = worst.max(err);
                    values += 1;
                    identical += usize::from(a.to_bits() == b.to_bits());
                    // Classification agrees wherever upstream is not within the tolerance of ½.
                    if (b - 0.5).abs() > TOLERANCE {
                        assert_eq!(*a > 0.5, b > 0.5, "{} at {:?}, value {k}", c.name, q.p);
                        classified += 1;
                    }
                }
            }
        }
        println!(
            "{{\"test\":\"winding_differential_corpus\",\"soups\":{},\"values\":{values},\
             \"bit_identical\":{identical},\"classified\":{classified},\"worst_abs_diff\":{worst:e}}}",
            cases.len()
        );
        assert_eq!(values, 4 * 1264, "every corpus query compared");
    }

    /// Upstream on an arm64 Mac (Apple's libm `atan2`) differs from the x86_64 corpus in 45 of its 1,264 query
    /// rows. The lifted code gives one answer, within `TOLERANCE` of both hosts' (the reason for the lift).
    #[test]
    fn upstream_answers_differ_by_host_and_the_lift_matches_both() {
        let cases = corpus();
        let (mut rows, mut differing, mut worst) = (0usize, 0usize, 0.0_f64);
        for line in DELTA.lines().filter(|l| !l.starts_with('#')) {
            let g: Vec<&str> = line.split(' ').collect();
            assert_eq!((g[0], g.len()), ("q", 7), "{line}");
            let c = case(&cases, g[1]);
            let q = &c.queries[g[2].parse::<usize>().expect("index")];
            let darwin = [3, 4, 5, 6].map(|k| float(g[k]));
            let ours = lifted(&c.soup, &octrees(&c.soup), q.p);
            let mut row_differs = false;
            for k in 0..4 {
                let differs = darwin[k].to_bits() != q.upstream[k].to_bits();
                row_differs |= differs;
                differing += usize::from(differs);
                let err = (ours[k] - darwin[k]).abs();
                assert!(err <= TOLERANCE, "{line}: lifted {:e}", ours[k]);
                worst = worst.max(err);
            }
            assert!(row_differs, "{line} repeats the x86_64 answers");
            rows += 1;
        }
        println!(
            "{{\"test\":\"winding_host_delta\",\"rows\":{rows},\"differing_values\":{differing},\
             \"worst_abs_diff_vs_darwin\":{worst:e}}}"
        );
        assert_eq!((rows, differing), (45, 107));
    }

    /// The bits of every lifted answer over the corpus, pinned. `det::atan2` and the fixed operation order make
    /// them host-independent (`BitwisePortable`), so any host must reproduce this digest.
    #[test]
    fn lifted_corpus_digest_is_pinned() {
        let mut bytes = Vec::new();
        for c in &corpus() {
            let octrees = octrees(&c.soup);
            for q in &c.queries {
                for w in lifted(&c.soup, &octrees, q.p) {
                    bytes.extend(w.to_bits().to_le_bytes());
                }
            }
        }
        assert_eq!(bytes.len(), 8 * 4 * 1264);
        // Pinned on an x86_64 Linux rch worker and reproduced on an arm64 Mac (aarch64-apple-darwin), 2026-11-03.
        assert_eq!(
            sha256(&bytes).to_hex(),
            "9c269daa3f8fab709b1d1eb92625412665632b5657b5022b306c9561a704ad1f"
        );
    }

    #[test]
    fn closed_soups_wind_by_their_orientation_and_overlap() {
        let cases = corpus();
        for (name, p, want) in [
            ("cube", [0.1, -0.2, 0.3], 1.0),
            ("cube", [3.0, 0.0, 0.0], 0.0),
            ("cube_flipped", [0.1, -0.2, 0.3], -1.0),
            ("two_cubes", [0.5, 0.25, 0.125], 2.0),
            ("two_cubes", [-0.5, -0.5, -0.5], 1.0),
            ("icosphere2", [0.2, 0.1, -0.3], 1.0),
            ("icosphere1_offset", [2.0, -1.0, 0.5], 1.0),
            ("far_cube", [1.0e6 + 1.0, -2.0e6, 3.0e5], 1.0),
        ] {
            let soup = &case(&cases, name).soup;
            let w = winding_exact(soup, p).expect("finite");
            assert!((w - want).abs() < 1e-9, "{name} at {p:?}: {w}");
            let octree = WindingOctree::build(soup, 2.0).expect("beta 2");
            assert!(
                (octree.winding(p).expect("finite") - want).abs() < 0.05,
                "{name}"
            );
        }
        // Scaling and translating the soup and the point together leaves the number alone.
        let unit = winding_exact(&cube([0.0; 3], 1.0), [0.25, 0.5, -0.75]).expect("finite");
        let moved = winding_exact(&cube([7.0, -3.0, 2.0], 8.0), [9.0, 1.0, -4.0]).expect("finite");
        assert!(
            (unit - 1.0).abs() < 1e-12 && (moved - 1.0).abs() < 1e-12,
            "{unit} {moved}"
        );
    }

    /// Upstream's rmesh-003: on the nightmare soup (duplicates, degenerates, a flipped patch, a hole), the octree
    /// classifies more than 99% of 2,000 points away from the surface correctly.
    #[test]
    fn rmesh_003_winding_classifies_nightmare_soup() {
        let cases = corpus();
        let soup = &case(&cases, "nightmare").soup;
        let octree = WindingOctree::build(soup, 2.0).expect("beta 2");
        let mut rng = Lcg(0x1003_2026_0706_50FA);
        let mut correct = 0u32;
        for _ in 0..2_000 {
            // Sample away from the surface and the defects' shadow (|r-1| > 0.2).
            let dir = Vec3::new(rng.unit() - 0.5, rng.unit() - 0.5, rng.unit() - 0.5);
            let n = dir.norm().max(1e-9);
            let r = if rng.below(2) == 0 {
                0.3 + rng.unit() * 0.45
            } else {
                1.25 + rng.unit() * 1.0
            };
            let p = Point3::new(dir.x / n * r, dir.y / n * r, dir.z / n * r);
            let truly_inside = r < 1.0;
            correct += u32::from(octree.inside(p.array()).expect("finite") == truly_inside);
        }
        println!("{{\"test\":\"rmesh_003\",\"correct\":{correct},\"total\":2000}}");
        assert!(correct > 1_980, "{correct}/2000");
    }

    /// Upstream's rmesh-004: the beta-2 octree stays within 0.05 of the exact number off the surface shell.
    #[test]
    fn rmesh_004_dipole_approximation_tracks_exact_within_declared_error() {
        let cases = corpus();
        let soup = &case(&cases, "icosphere3").soup;
        let octree = WindingOctree::build(soup, 2.0).expect("beta 2");
        let mut rng = Lcg(0x1004_2026_0706_D1B0);
        let (mut worst, mut used) = (0.0f64, 0u32);
        for _ in 0..400 {
            let p = Point3::new(
                (rng.unit() - 0.5) * 5.0,
                (rng.unit() - 0.5) * 5.0,
                (rng.unit() - 0.5) * 5.0,
            );
            // Skip the surface shell where winding is legitimately steep.
            let r = p.delta_from(Point3::new(0.0, 0.0, 0.0)).norm();
            if (r - 1.0).abs() < 0.1 {
                continue;
            }
            let exact = winding_exact(soup, p.array()).expect("finite");
            let approx = octree.winding(p.array()).expect("finite");
            worst = worst.max((approx - exact).abs());
            used += 1;
        }
        println!(
            "{{\"test\":\"rmesh_004\",\"worst_abs_error\":{worst:e},\"points\":{used},\"beta\":2}}"
        );
        assert!(used > 300 && worst < 0.05, "{worst} over {used}");
    }

    /// The LCG of upstream's conformance tests.
    struct Lcg(u64);

    impl Lcg {
        fn next(&mut self) -> u64 {
            self.0 = self
                .0
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            self.0
        }

        fn unit(&mut self) -> f64 {
            ((self.next() >> 11) as f64) / (1u64 << 53) as f64
        }

        fn below(&mut self, n: u64) -> u64 {
            (self.next() >> 32) % n
        }
    }

    /// Upstream's `shapes::cube`: 12 outward triangles.
    fn cube([x, y, z]: [f64; 3], h: f64) -> Soup {
        let positions = vec![
            [x - h, y - h, z - h],
            [x + h, y - h, z - h],
            [x + h, y + h, z - h],
            [x - h, y + h, z - h],
            [x - h, y - h, z + h],
            [x + h, y - h, z + h],
            [x + h, y + h, z + h],
            [x - h, y + h, z + h],
        ];
        let triangles = vec![
            [0, 2, 1],
            [0, 3, 2],
            [4, 5, 6],
            [4, 6, 7],
            [0, 1, 5],
            [0, 5, 4],
            [3, 7, 6],
            [3, 6, 2],
            [0, 4, 7],
            [0, 7, 3],
            [1, 2, 6],
            [1, 6, 5],
        ];
        Soup::new(positions, triangles).expect("valid cube")
    }

    fn code(r: Result<impl std::fmt::Debug, Refusal>) -> &'static str {
        r.expect_err("refused").code
    }

    #[test]
    fn bad_soups_queries_and_betas_are_refused() {
        let tri = vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]];
        for bad in [[0.0, f64::NAN, 0.0], [f64::INFINITY, 0.0, 0.0]] {
            assert_eq!(code(Soup::new(vec![bad], vec![])), "FMINE-REF-109");
        }
        assert_eq!(
            code(Soup::new(tri.clone(), vec![[0, 1, 3]])),
            "FMINE-REF-109"
        );
        let soup = Soup::new(tri, vec![[0, 1, 2]]).expect("valid");
        assert_eq!(soup.corners(1), None);
        assert_eq!(soup.corners(0).map(|t| t[1]), Some([1.0, 0.0, 0.0]));
        assert_eq!(
            (soup.position_count(), soup.position(2), soup.position(3)),
            (3, Some([0.0, 1.0, 0.0]), None)
        );
        let octree = WindingOctree::build(&soup, 2.0).expect("beta 2");
        for p in [[f64::NAN, 0.0, 0.0], [0.0, f64::NEG_INFINITY, 0.0]] {
            assert_eq!(code(winding_exact(&soup, p)), "FMINE-REF-109");
            let corner = [1.0, 0.0, 0.0];
            assert_eq!(
                code(triangle_winding([0.0; 3], p, corner, [0.0, 1.0, 0.0])),
                "FMINE-REF-109"
            );
            assert_eq!(
                code(triangle_winding(p, [0.0; 3], corner, [0.0, 1.0, 0.0])),
                "FMINE-REF-109"
            );
            assert_eq!(code(octree.winding(p)), "FMINE-REF-109");
            assert_eq!(code(octree.inside(p)), "FMINE-REF-109");
        }
        // Upstream raised these to 1.
        for beta in [0.999, 0.0, -2.0, f64::NAN, f64::INFINITY] {
            assert_eq!(
                code(WindingOctree::build(&soup, beta)),
                "FMINE-REF-109",
                "beta {beta}"
            );
        }
        assert!(WindingOctree::build(&soup, 1.0).is_ok());
    }

    #[test]
    fn overflowing_arithmetic_is_refused_207_not_answered() {
        // Corner distances near 1e105 overflow la·lb·lc; std's atan2 of the infinities gave upstream an answer.
        let huge = cube([0.0; 3], 1.0e105);
        assert_eq!(code(winding_exact(&huge, [0.0; 3])), "FMINE-REF-207");
        let (a, b, c) = ([1e105, 0.0, 0.0], [0.0, 1e105, 0.0], [0.0, 0.0, 1e105]);
        assert_eq!(code(triangle_winding([0.0; 3], a, b, c)), "FMINE-REF-207");
        // Corners at ±1.5e308 overflow the octree's centroid sums.
        let edge = Soup::new(
            vec![
                [1.5e308, 0.0, 0.0],
                [1.5e308, 1.0, 0.0],
                [1.5e308, 0.0, 1.0],
                [-1.5e308, 0.0, 0.0],
            ],
            (0..20)
                .map(|i| if i % 2 == 0 { [0, 1, 2] } else { [3, 1, 2] })
                .collect(),
        )
        .expect("finite");
        assert_eq!(code(WindingOctree::build(&edge, 2.0)), "FMINE-REF-207");
        // Below the overflow the same shape answers.
        let big = cube([0.0; 3], 1.0e100);
        assert!((winding_exact(&big, [0.0; 3]).expect("finite") - 1.0).abs() < 1e-12);
    }

    #[test]
    fn the_empty_soup_winds_zero() {
        let empty = Soup::new(Vec::new(), Vec::new()).expect("valid");
        let w = winding_exact(&empty, [0.5, 0.0, 0.0]).expect("finite");
        assert_eq!(w.to_bits(), (-0.0_f64).to_bits());
        let octree = WindingOctree::build(&empty, 2.0).expect("beta 2");
        assert_eq!(octree.winding([0.0; 3]).map(f64::to_bits), Ok(w.to_bits()));
        assert_eq!(octree.inside([0.0; 3]), Ok(false));
        assert!(std::ptr::eq(octree.soup(), &empty));
    }
}
