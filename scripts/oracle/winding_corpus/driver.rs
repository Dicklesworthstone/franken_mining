//! Driver for fmine-geom's winding differential corpus (bd-192). `assemble.py` puts it beside frankensim's
//! fs-rep-mesh `winding.rs` and `shapes.rs`, copied unmodified, and this prints `fixtures/fmine-geom/
//! winding_corpus.txt`: each soup, then for each query point upstream's `winding_exact` and its
//! `WindingOctree::winding` at beta 1, 2 and 4, every float as its 16 hex digits of IEEE-754 bits.

#[path = "upstream/winding.rs"]
#[allow(dead_code)]
mod winding;

#[path = "upstream/shapes.rs"]
#[allow(dead_code)]
mod shapes;

use fs_geom::{Point3, Vec3};
use winding::{Soup, WindingOctree, winding_exact};

const BETAS: [f64; 3] = [1.0, 2.0, 4.0];

/// The LCG of fs-rep-mesh's conformance tests.
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
}

fn bits(x: f64) -> String {
    format!("{:016x}", x.to_bits())
}

fn p3(x: f64, y: f64, z: f64) -> Point3 {
    Point3::new(x, y, z)
}

fn concat(a: Soup, b: Soup) -> Soup {
    let base = a.positions.len() as u32;
    let mut positions = a.positions;
    positions.extend(b.positions);
    let mut triangles = a.triangles;
    triangles.extend(b.triangles.iter().map(|t| [t[0] + base, t[1] + base, t[2] + base]));
    Soup { positions, triangles }
}

fn cross(a: Vec3, b: Vec3) -> Vec3 {
    Vec3::new(a.y * b.z - a.z * b.y, a.z * b.x - a.x * b.z, a.x * b.y - a.y * b.x)
}

fn soups() -> Vec<(&'static str, Soup)> {
    let o = p3(0.0, 0.0, 0.0);
    vec![
        ("cube", shapes::cube(o, 1.0)),
        ("cube_offset", shapes::cube(p3(0.3, -0.2, 0.1), 0.75)),
        // Every face flipped: the winding number is -1 inside.
        ("cube_flipped", shapes::corrupt(shapes::cube(o, 1.0), 0, 0, 0..12, None)),
        // One +z triangle removed: an open soup, so the winding number is fractional near the hole.
        ("cube_open", shapes::corrupt(shapes::cube(o, 1.0), 0, 0, 0..0, Some(2))),
        // Two overlapping cubes: 2 in the overlap.
        ("two_cubes", concat(shapes::cube(o, 1.0), shapes::cube(p3(1.0, 0.5, 0.25), 1.0))),
        ("far_cube", shapes::cube(p3(1.0e6, -2.0e6, 3.0e5), 10.0)),
        (
            "triangle",
            Soup {
                positions: vec![p3(0.0, 0.0, 0.0), p3(1.0, 0.0, 0.0), p3(0.0, 1.0, 0.0)],
                triangles: vec![[0, 1, 2]],
            },
        ),
        (
            "slivers",
            Soup {
                positions: vec![
                    p3(0.0, 0.0, 0.0),
                    p3(1.0, 0.0, 0.0),
                    p3(0.5, 1.0e-9, 0.0),
                    p3(0.5, 0.0, 1.0e-12),
                    p3(2.0, 0.0, 0.0),
                ],
                triangles: vec![[0, 1, 2], [0, 3, 1], [0, 1, 4], [2, 2, 2]],
            },
        ),
        ("icosphere1_offset", shapes::icosphere(p3(2.0, -1.0, 0.5), 0.5, 1)),
        ("icosphere2", shapes::icosphere(o, 1.0, 2)),
        // rmesh-003's nightmare soup: duplicates, degenerates, a flipped patch and a hole.
        ("nightmare", shapes::corrupt(shapes::icosphere(o, 1.0, 2), 6, 4, 40..48, Some(10))),
        // rmesh-004's soup.
        ("icosphere3", shapes::icosphere(o, 1.0, 3)),
        ("empty", Soup { positions: Vec::new(), triangles: Vec::new() }),
    ]
}

/// Center and half the largest extent of the soup's bounding box (the unit box for no positions).
fn frame(soup: &Soup) -> (Point3, f64) {
    let Some(first) = soup.positions.first() else {
        return (p3(0.0, 0.0, 0.0), 1.0);
    };
    let (mut lo, mut hi) = (*first, *first);
    for p in &soup.positions {
        lo = p3(lo.x.min(p.x), lo.y.min(p.y), lo.z.min(p.z));
        hi = p3(hi.x.max(p.x), hi.y.max(p.y), hi.z.max(p.z));
    }
    let c = p3(f64::midpoint(lo.x, hi.x), f64::midpoint(lo.y, hi.y), f64::midpoint(lo.z, hi.z));
    let half = 0.5 * (hi.x - lo.x).max(hi.y - lo.y).max(hi.z - lo.z);
    (c, half)
}

fn queries(soup: &Soup, seed: u64) -> Vec<Point3> {
    let (c, h) = frame(soup);
    let mut rng = Lcg(seed);
    let mut q = vec![c];
    for _ in 0..48 {
        let mut coord = |m: f64| m + (2.0 * rng.unit() - 1.0) * 1.5 * h;
        q.push(p3(coord(c.x), coord(c.y), coord(c.z)));
    }
    for _ in 0..8 {
        let d = Vec3::new(rng.unit() - 0.5, rng.unit() - 0.5, rng.unit() - 0.5);
        // Between 10 and 10^4 half-extents away; no libm call, so the points are the same bits on every host.
        let r = h * 10.0 * 999.0f64.mul_add(rng.unit(), 1.0) / d.norm().max(1e-9);
        q.push(c.offset(d.scale(r)));
    }
    q.extend(soup.positions.iter().take(16));
    for (k, t) in soup.triangles.iter().take(16).enumerate() {
        let [a, b, cc] = [t[0], t[1], t[2]].map(|i| soup.positions[i as usize]);
        q.push(p3((a.x + b.x + cc.x) / 3.0, (a.y + b.y + cc.y) / 3.0, (a.z + b.z + cc.z) / 3.0));
        if k < 8 {
            q.push(p3(f64::midpoint(a.x, b.x), f64::midpoint(a.y, b.y), f64::midpoint(a.z, b.z)));
            let n = cross(b.delta_from(a), cc.delta_from(a));
            let len = n.norm();
            if len > 0.0 {
                let centroid = p3((a.x + b.x + cc.x) / 3.0, (a.y + b.y + cc.y) / 3.0, (a.z + b.z + cc.z) / 3.0);
                for s in [1.0e-9, -1.0e-9] {
                    q.push(centroid.offset(n.scale(s * h / len)));
                }
            }
        }
    }
    q
}

fn main() {
    println!("# fmine-geom winding differential corpus (bd-192): see winding_corpus.txt.capture.toml.");
    println!("# soup <name> <positions> <triangles>, then one `p <x> <y> <z>` line per position and one");
    println!("# `t <a> <b> <c>` line per triangle; then `q <x> <y> <z> <exact> <beta1> <beta2> <beta4>` per query,");
    println!("# where exact is upstream winding_exact and betaN its WindingOctree (beta N) winding; then `end`.");
    println!("# Floats are the 16 hex digits of their IEEE-754 bits.");
    for (k, (name, soup)) in soups().into_iter().enumerate() {
        println!("soup {name} {} {}", soup.positions.len(), soup.triangles.len());
        for p in &soup.positions {
            println!("p {} {} {}", bits(p.x), bits(p.y), bits(p.z));
        }
        for t in &soup.triangles {
            println!("t {} {} {}", t[0], t[1], t[2]);
        }
        let octrees: Vec<WindingOctree> = BETAS.iter().map(|&b| WindingOctree::build(&soup, b)).collect();
        for p in queries(&soup, 0x0192_2026_1103_0000 + k as u64) {
            let exact = winding_exact(&soup, p);
            let approx: Vec<String> = octrees.iter().map(|o| bits(o.winding(&soup, p))).collect();
            println!("q {} {} {} {} {}", bits(p.x), bits(p.y), bits(p.z), bits(exact), approx.join(" "));
        }
        println!("end");
    }
}
