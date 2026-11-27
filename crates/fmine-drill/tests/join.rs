//! Overlap-join tests (`BD-165`'s done-when: matches brute force). Every pair is compared with the pair-by-pair
//! answer on seeded random spans: dense and sparse, nested, identical, touching, and many sharing one start.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use fmine_drill::join::{HoleOverlap, Overlap, Span, join_intervals, overlaps, self_overlaps};
use fmine_drill::records::{self, DrillError};
use fmine_evidence::RefusalCode;

#[allow(dead_code)]
#[path = "common/fixtures.rs"]
mod fixtures;

fn span(from_mm: u64, to_mm: u64) -> Span {
    Span { from_mm, to_mm }
}

fn shared(a: Span, b: Span) -> Option<Span> {
    let from_mm = a.from_mm.max(b.from_mm);
    let to_mm = a.to_mm.min(b.to_mm);
    (to_mm > from_mm).then_some(Span { from_mm, to_mm })
}

fn brute(left: &[Span], right: &[Span]) -> Vec<Overlap> {
    let mut out = Vec::new();
    for (i, &a) in left.iter().enumerate() {
        for (j, &b) in right.iter().enumerate() {
            if let Some(span) = shared(a, b) {
                out.push(Overlap {
                    left: i,
                    right: j,
                    span,
                });
            }
        }
    }
    out
}

fn brute_self(spans: &[Span]) -> Vec<Overlap> {
    let mut out = Vec::new();
    for i in 0..spans.len() {
        for j in i + 1..spans.len() {
            if let Some(span) = shared(spans[i], spans[j]) {
                out.push(Overlap {
                    left: i,
                    right: j,
                    span,
                });
            }
        }
    }
    out
}

/// splitmix64.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
}

/// `n` spans on a hole of `depth_mm`, each at most `max_len_mm` long. With `grid_mm` above 1 the ends fall on a
/// grid, so that touching, shared starts and identical spans are common.
fn spans(rng: &mut Rng, n: usize, depth_mm: u64, max_len_mm: u64, grid_mm: u64) -> Vec<Span> {
    (0..n)
        .map(|_| {
            let from = rng.below(depth_mm / grid_mm) * grid_mm;
            let len = (1 + rng.below(max_len_mm / grid_mm)) * grid_mm;
            span(from, from + len)
        })
        .collect()
}

#[test]
fn test_the_sweep_matches_brute_force_on_seeded_random_spans() {
    let mut rng = Rng(0x0000_0165_5EED);
    let mut pairs = 0;
    let mut cases = 0;
    // (left count, right count, hole depth, longest span, grid)
    let shapes: [(usize, usize, u64, u64, u64); 8] = [
        (0, 40, 100_000, 5_000, 1),
        (1, 1, 10, 10, 1),
        (50, 50, 300_000, 3_000, 1),
        (200, 30, 300_000, 60_000, 1_000),
        (120, 120, 20_000, 2_000, 500),
        (300, 300, 1_000_000, 1_500, 250),
        (80, 400, 50_000, 50_000, 5_000),
        (25, 25, 10_000, 1_000, 1_000),
    ];
    for round in 0..40 {
        for &(n, m, depth, longest, grid) in &shapes {
            let left = spans(&mut rng, n, depth, longest, grid);
            let right = spans(&mut rng, m, depth, longest, grid);
            let got = overlaps(&left, &right).unwrap();
            assert_eq!(got, brute(&left, &right), "round {round}, shape {n}x{m}");
            pairs += got.len();
            assert_eq!(
                self_overlaps(&left).unwrap(),
                brute_self(&left),
                "round {round}, self {n}"
            );
            cases += 1;
        }
    }
    assert!(
        pairs > 100_000,
        "the cases should overlap often: {pairs} pairs"
    );
    eprintln!("{cases} cases, {pairs} overlapping pairs, all equal to brute force");
}

#[test]
fn test_touching_nested_identical_and_shared_start_spans() {
    let left = [span(0, 10), span(10, 20), span(5, 15), span(0, 100)];
    let right = [span(10, 20), span(20, 30), span(0, 10), span(12, 13)];
    let got = overlaps(&left, &right).unwrap();
    let expect = vec![
        Overlap {
            left: 0,
            right: 2,
            span: span(0, 10),
        },
        Overlap {
            left: 1,
            right: 0,
            span: span(10, 20),
        },
        Overlap {
            left: 1,
            right: 3,
            span: span(12, 13),
        },
        Overlap {
            left: 2,
            right: 0,
            span: span(10, 15),
        },
        Overlap {
            left: 2,
            right: 2,
            span: span(5, 10),
        },
        Overlap {
            left: 2,
            right: 3,
            span: span(12, 13),
        },
        Overlap {
            left: 3,
            right: 0,
            span: span(10, 20),
        },
        Overlap {
            left: 3,
            right: 1,
            span: span(20, 30),
        },
        Overlap {
            left: 3,
            right: 2,
            span: span(0, 10),
        },
        Overlap {
            left: 3,
            right: 3,
            span: span(12, 13),
        },
    ];
    assert_eq!(got, expect);
    assert_eq!(got, brute(&left, &right));
    // Swapping the sides swaps every pair.
    let mut swapped: Vec<Overlap> = overlaps(&right, &left)
        .unwrap()
        .into_iter()
        .map(|o| Overlap {
            left: o.right,
            right: o.left,
            span: o.span,
        })
        .collect();
    swapped.sort_unstable();
    assert_eq!(swapped, expect);
    // A logger's own intervals: touching is fine, overlap is reported.
    assert_eq!(
        self_overlaps(&[span(0, 10), span(10, 20), span(20, 30)]).unwrap(),
        vec![]
    );
    assert_eq!(
        self_overlaps(&[span(20, 30), span(0, 10), span(5, 25)]).unwrap(),
        vec![
            Overlap {
                left: 0,
                right: 2,
                span: span(20, 25)
            },
            Overlap {
                left: 1,
                right: 2,
                span: span(5, 10)
            },
        ]
    );
    assert_eq!(overlaps(&[], &[]).unwrap(), vec![]);
}

#[test]
fn test_records_join_hole_by_hole() {
    let i = |id: &str, hole: &str, from_mm: u64, to_mm: u64| {
        fixtures::interval(id, hole, from_mm, to_mm)
    };
    let samples = [
        i("S1", "H2", 0, 1_000),
        i("S2", "H1", 0, 1_000),
        i("S3", "H1", 1_000, 2_000),
        i("S4", "H3", 0, 5_000),
    ];
    let lithology = [
        i("L1", "H1", 500, 1_500),
        i("L2", "H2", 900, 3_000),
        i("L3", "H1", 1_500, 4_000),
        i("L4", "H4", 0, 5_000),
    ];
    let got = join_intervals(&samples, &lithology).unwrap();
    let pair = |hole: &str, left, right, from_mm, to_mm| HoleOverlap {
        hole_id: hole.to_string(),
        overlap: Overlap {
            left,
            right,
            span: span(from_mm, to_mm),
        },
    };
    assert_eq!(
        got,
        vec![
            pair("H1", 1, 0, 500, 1_000),
            pair("H1", 2, 0, 1_000, 1_500),
            pair("H1", 2, 2, 1_500, 2_000),
            pair("H2", 0, 1, 900, 1_000),
        ]
    );
    // The same answer from brute force over every pair on a shared hole.
    let mut brute_records = Vec::new();
    for (a, l) in samples.iter().enumerate() {
        for (b, r) in lithology.iter().enumerate() {
            if l.hole_id == r.hole_id
                && let Some(s) = shared(Span::of(l), Span::of(r))
            {
                brute_records.push(pair(&l.hole_id, a, b, s.from_mm, s.to_mm));
            }
        }
    }
    brute_records.sort();
    assert_eq!(got, brute_records);
}

#[test]
fn test_a_span_that_does_not_end_past_its_start_is_refused() {
    for (what, err) in [
        ("left", overlaps(&[span(5, 5)], &[span(0, 10)]).unwrap_err()),
        (
            "right",
            overlaps(&[span(0, 10)], &[span(0, 10), span(9, 3)]).unwrap_err(),
        ),
        ("self", self_overlaps(&[span(7, 7)]).unwrap_err()),
        (
            "record",
            join_intervals(
                &[records::Interval {
                    to_mm: 0,
                    ..fixtures::interval("S1", "H1", 0, 1)
                }],
                &[fixtures::interval("L1", "H1", 0, 1)],
            )
            .unwrap_err(),
        ),
    ] {
        assert!(matches!(err, DrillError::Invalid { .. }), "{what}: {err:?}");
        assert_eq!(
            err.to_refusal().code,
            RefusalCode::OutsidePhysicalDomain109.spec().code,
            "{what}"
        );
    }
}
