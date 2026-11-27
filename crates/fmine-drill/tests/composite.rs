//! Compositing tests (`BD-166`'s done-when: known answers; PO-CORE-2: Σ length × grade and Σ mass × grade
//! conserved to 1e-12 relative). Hand-worked composites use binary fractions, so they compare exactly. Seeded
//! random holes check conservation, exact tiling of every domain and each composite value against a brute-force
//! recomputation.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use fmine_claim::{AdditivityClass, SpatialSupport};
use fmine_drill::composite::{
    Bounds, CONSERVATION_TOLERANCE, Composite, CompositeSet, Domain, Input, Residual, Rule,
    Weighting, composite,
};
use fmine_drill::join::Span;
use fmine_drill::records::{AssayValue, DrillError};
use fmine_evidence::RefusalCode;

fn span(from_mm: u64, to_mm: u64) -> Span {
    Span { from_mm, to_mm }
}

fn exact(v: f64) -> Bounds {
    Bounds { lo: v, hi: v }
}

fn input(from_mm: u64, to_mm: u64, v: f64) -> Input {
    Input {
        span: span(from_mm, to_mm),
        value: exact(v),
        mass_kg: None,
    }
}

fn domain(code: &str, from_mm: u64, to_mm: u64) -> Domain {
    Domain {
        code: code.to_string(),
        span: span(from_mm, to_mm),
    }
}

fn rule(length_mm: u64, residual: Residual) -> Rule {
    Rule {
        length_mm,
        weighting: Weighting::Length,
        residual,
        additivity: AdditivityClass::IntensiveWeighted,
        input_support: SpatialSupport::PointCore,
    }
}

fn spans(cs: &[Composite]) -> Vec<(u64, u64)> {
    cs.iter().map(|c| (c.span.from_mm, c.span.to_mm)).collect()
}

fn values(cs: &[Composite]) -> Vec<Option<f64>> {
    cs.iter()
        .map(|c| {
            c.value.map(|b| {
                assert_eq!(b.lo, b.hi);
                b.lo
            })
        })
        .collect()
}

#[test]
fn test_length_weighted_composites_have_their_known_values_and_respect_domains() {
    let set = composite(
        &[domain("A", 0, 4000)],
        &[
            input(0, 1000, 0.25),
            input(1000, 3000, 0.5),
            input(3000, 4000, 1.0),
        ],
        &rule(2000, Residual::Keep),
    )
    .unwrap();
    assert_eq!(spans(&set.composites), [(0, 2000), (2000, 4000)]);
    assert_eq!(values(&set.composites), [Some(0.375), Some(0.75)]);
    assert_eq!(set.composites[0].inputs, [0, 1]);
    assert_eq!(
        set.composites[0].support,
        SpatialSupport::Composite { length_mm: 2000 }
    );
    assert_eq!(set.conservation.inputs, exact(2250.0));
    assert_eq!(set.conservation.relative_error, 0.0);

    // An input across a contact is shared between the two domains; no composite crosses it.
    let set = composite(
        &[domain("OX", 0, 3000), domain("FR", 3000, 6000)],
        &[
            input(0, 2000, 0.25),
            input(2000, 4000, 0.5),
            input(4000, 6000, 1.0),
        ],
        &rule(2000, Residual::Keep),
    )
    .unwrap();
    assert_eq!(
        spans(&set.composites),
        [(0, 2000), (2000, 3000), (3000, 5000), (5000, 6000)]
    );
    assert_eq!(
        values(&set.composites),
        [Some(0.25), Some(0.5), Some(0.75), Some(1.0)]
    );
    let codes: Vec<&str> = set.composites.iter().map(|c| c.domain.as_str()).collect();
    assert_eq!(codes, ["OX", "OX", "FR", "FR"]);
    assert_eq!(
        set.composites[1].support,
        SpatialSupport::Composite { length_mm: 1000 }
    );

    // Domains are taken by depth whatever order they come in; core outside every domain is not composited.
    let set = composite(
        &[domain("B", 5000, 6000), domain("A", 1000, 2000)],
        &[input(0, 6000, 0.5)],
        &rule(1000, Residual::Keep),
    )
    .unwrap();
    assert_eq!(spans(&set.composites), [(1000, 2000), (5000, 6000)]);
    assert_eq!(set.conservation.inputs, exact(1000.0));
}

#[test]
fn test_each_residual_rule_gives_its_known_pieces() {
    let whole = [input(0, 5000, 0.5)];
    let a = [domain("A", 0, 5000)];
    type Pieces = Vec<(u64, u64)>;
    let cases: Vec<(&str, Residual, Pieces, Pieces)> = vec![
        (
            "keep",
            Residual::Keep,
            vec![(0, 2000), (2000, 4000), (4000, 5000)],
            vec![],
        ),
        (
            "merge a short piece",
            Residual::MergeBelow { min_mm: 1500 },
            vec![(0, 2000), (2000, 5000)],
            vec![],
        ),
        (
            "keep a piece at the minimum",
            Residual::MergeBelow { min_mm: 1000 },
            vec![(0, 2000), (2000, 4000), (4000, 5000)],
            vec![],
        ),
        (
            "drop a short piece",
            Residual::DropBelow { min_mm: 1500 },
            vec![(0, 2000), (2000, 4000)],
            vec![(4000, 5000)],
        ),
        (
            "distribute",
            Residual::Distribute,
            vec![(0, 1667), (1667, 3333), (3333, 5000)],
            vec![],
        ),
    ];
    for (what, residual, kept, dropped) in cases {
        let set = composite(&a, &whole, &rule(2000, residual)).unwrap();
        assert_eq!(spans(&set.composites), kept, "{what}");
        assert_eq!(spans(&set.dropped), dropped, "{what}");
        assert!(
            set.composites
                .iter()
                .chain(&set.dropped)
                .all(|c| c.value == Some(exact(0.5))),
            "{what}"
        );
        assert_eq!(set.conservation.inputs, exact(2500.0), "{what}");
        assert_eq!(set.conservation.composites, exact(2500.0), "{what}");
    }
    // A run shorter than the minimum has nothing above it to merge into, and a short run is one piece.
    let short = [domain("A", 0, 900)];
    let merged = composite(
        &short,
        &whole,
        &rule(2000, Residual::MergeBelow { min_mm: 1500 }),
    )
    .unwrap();
    assert_eq!(spans(&merged.composites), [(0, 900)]);
    let spread = composite(&short, &whole, &rule(2000, Residual::Distribute)).unwrap();
    assert_eq!(spans(&spread.composites), [(0, 900)]);
    let gone = composite(
        &short,
        &whole,
        &rule(2000, Residual::DropBelow { min_mm: 1500 }),
    )
    .unwrap();
    assert!(gone.composites.is_empty());
    assert_eq!(spans(&gone.dropped), [(0, 900)]);
}

#[test]
fn test_unsampled_core_mass_weighting_censoring_and_extensive_quantities() {
    // Unsampled core is not zero grade.
    let set = composite(
        &[domain("A", 0, 6000)],
        &[input(0, 1000, 0.25), input(3000, 4000, 1.0)],
        &rule(2000, Residual::Keep),
    )
    .unwrap();
    assert_eq!(values(&set.composites), [Some(0.25), Some(1.0), None]);
    let sampled: Vec<u64> = set.composites.iter().map(|c| c.sampled_mm).collect();
    assert_eq!(sampled, [1000, 1000, 0]);

    // Mass weighting: (3 kg × 0.25 + 1 kg × 1.0) / 4 kg, where length weighting gives 0.625.
    let heavy = Input {
        mass_kg: Some(3.0),
        ..input(0, 1000, 0.25)
    };
    let light = Input {
        mass_kg: Some(1.0),
        ..input(1000, 2000, 1.0)
    };
    let mass = Rule {
        weighting: Weighting::Mass,
        ..rule(2000, Residual::Keep)
    };
    let set = composite(&[domain("A", 0, 2000)], &[heavy, light], &mass).unwrap();
    assert_eq!(values(&set.composites), [Some(0.4375)]);
    assert_eq!(set.composites[0].weight, 4.0);
    assert_eq!(set.conservation.inputs, exact(1.75));
    let set = composite(
        &[domain("A", 0, 2000)],
        &[heavy, light],
        &rule(2000, Residual::Keep),
    )
    .unwrap();
    assert_eq!(values(&set.composites), [Some(0.625)]);
    // An input split between composites lends each the mass of its share.
    let split = Input {
        mass_kg: Some(4.0),
        ..input(0, 2000, 0.5)
    };
    let set = composite(
        &[domain("A", 0, 2000)],
        &[split],
        &Rule {
            length_mm: 500,
            ..mass
        },
    )
    .unwrap();
    assert!(
        set.composites
            .iter()
            .all(|c| c.weight == 1.0 && c.value == Some(exact(0.5)))
    );

    // Censored inputs give bounds; nothing is substituted.
    assert_eq!(
        Bounds::of(&AssayValue::Measured { mass_fraction: 0.5 }),
        exact(0.5)
    );
    assert_eq!(
        Bounds::of(&AssayValue::BelowDetection { limit: 0.25 }),
        Bounds { lo: 0.0, hi: 0.25 }
    );
    assert_eq!(
        Bounds::of(&AssayValue::AboveUpper { limit: 0.75 }),
        Bounds { lo: 0.75, hi: 1.0 }
    );
    let set = composite(
        &[domain("A", 0, 4000)],
        &[
            input(0, 1000, 0.5),
            Input {
                value: Bounds::of(&AssayValue::BelowDetection { limit: 0.25 }),
                ..input(1000, 2000, 0.0)
            },
            Input {
                value: Bounds::of(&AssayValue::AboveUpper { limit: 0.75 }),
                ..input(2000, 4000, 0.0)
            },
        ],
        &rule(2000, Residual::Keep),
    )
    .unwrap();
    let bounds: Vec<Bounds> = set.composites.iter().map(|c| c.value.unwrap()).collect();
    assert_eq!(
        bounds,
        [
            Bounds {
                lo: 0.25,
                hi: 0.375
            },
            Bounds { lo: 0.75, hi: 1.0 }
        ]
    );
    assert_eq!(
        set.conservation.inputs,
        Bounds {
            lo: 2000.0,
            hi: 2750.0
        }
    );

    // An extensive quantity (contained metal, kg) is shared out by length and summed.
    let extensive = Rule {
        additivity: AdditivityClass::ExtensiveAdditive,
        ..rule(1000, Residual::Keep)
    };
    let set = composite(
        &[domain("A", 0, 3000)],
        &[input(0, 2000, 4.0), input(2000, 3000, 1.0)],
        &extensive,
    )
    .unwrap();
    assert_eq!(values(&set.composites), [Some(2.0), Some(2.0), Some(1.0)]);
    assert_eq!(set.conservation.inputs, exact(5.0));
    let set = composite(
        &[domain("A", 1000, 2000)],
        &[input(0, 4000, 4.0)],
        &extensive,
    )
    .unwrap();
    assert_eq!(values(&set.composites), [Some(1.0)]);
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

    fn unit(&mut self) -> f64 {
        (self.next() >> 11) as f64 / (1u64 << 53) as f64
    }
}

/// Consecutive spans from `0` to about `depth_mm`, each up to `max_mm` long, with about `gap_percent` of them
/// followed by a gap.
fn runs(rng: &mut Rng, depth_mm: u64, max_mm: u64, gap_percent: u64) -> Vec<Span> {
    let mut out = Vec::new();
    let mut at = rng.below(max_mm);
    while at < depth_mm {
        let to = at + 1 + rng.below(max_mm);
        out.push(span(at, to));
        at = to;
        if rng.below(100) < gap_percent {
            at += 1 + rng.below(max_mm);
        }
    }
    out
}

fn relative(a: f64, b: f64) -> f64 {
    let scale = a.abs().max(b.abs());
    if scale == 0.0 {
        0.0
    } else {
        (a - b).abs() / scale
    }
}

/// The value of one composite from every input, pair by pair.
fn brute(c: &Composite, inputs: &[Input], rule: &Rule) -> Option<Bounds> {
    let (mut w, mut lo, mut hi) = (0.0, 0.0, 0.0);
    let mut any = false;
    for i in inputs {
        let from = c.span.from_mm.max(i.span.from_mm);
        let to = c.span.to_mm.min(i.span.to_mm);
        if to <= from {
            continue;
        }
        any = true;
        let shared = (to - from) as f64;
        let fraction = shared / (i.span.to_mm - i.span.from_mm) as f64;
        match rule.additivity {
            AdditivityClass::IntensiveWeighted => {
                let wi = match rule.weighting {
                    Weighting::Length => shared,
                    Weighting::Mass => i.mass_kg.unwrap() * fraction,
                };
                w += wi;
                lo += wi * i.value.lo;
                hi += wi * i.value.hi;
            }
            _ => {
                lo += i.value.lo * fraction;
                hi += i.value.hi * fraction;
            }
        }
    }
    any.then(|| match rule.additivity {
        AdditivityClass::IntensiveWeighted => Bounds {
            lo: lo / w,
            hi: hi / w,
        },
        _ => Bounds { lo, hi },
    })
}

fn total(cs: &[Composite], rule: &Rule) -> f64 {
    cs.iter()
        .filter_map(|c| {
            c.value.map(|v| match rule.additivity {
                AdditivityClass::IntensiveWeighted => c.weight * v.lo,
                _ => v.lo,
            })
        })
        .sum()
}

fn check_tiling(set: &CompositeSet, domains: &[Domain], rule: &Rule, what: &str) {
    let mut pieces: Vec<&Composite> = set.composites.iter().chain(&set.dropped).collect();
    pieces.sort_by_key(|c| c.span);
    let mut sorted = domains.to_vec();
    sorted.sort_by_key(|d| d.span);
    let mut it = pieces.into_iter().peekable();
    for d in &sorted {
        let mut at = d.span.from_mm;
        let mut lengths = Vec::new();
        while let Some(c) = it.next_if(|c| c.span.from_mm < d.span.to_mm) {
            assert_eq!(c.domain, d.code, "{what}");
            assert_eq!(c.span.from_mm, at, "{what}: a gap or overlap in {}", d.code);
            assert!(
                c.span.to_mm <= d.span.to_mm,
                "{what}: a composite crosses out of {}",
                d.code
            );
            lengths.push(c.span.to_mm - c.span.from_mm);
            at = c.span.to_mm;
        }
        assert_eq!(
            at, d.span.to_mm,
            "{what}: {} not covered to its end",
            d.code
        );
        let step = rule.length_mm;
        match rule.residual {
            Residual::Distribute => {
                let (lo, hi) = (lengths.iter().min().unwrap(), lengths.iter().max().unwrap());
                assert!(hi - lo <= 1, "{what}: equal pieces differ by {}", hi - lo);
            }
            Residual::Keep | Residual::DropBelow { .. } => {
                assert!(
                    lengths[..lengths.len() - 1].iter().all(|&l| l == step),
                    "{what}"
                );
            }
            Residual::MergeBelow { min_mm } => {
                assert!(
                    lengths[..lengths.len() - 1].iter().all(|&l| l == step),
                    "{what}"
                );
                assert!(*lengths.last().unwrap() < step + min_mm, "{what}");
            }
        }
    }
    assert!(
        it.next().is_none(),
        "{what}: a composite outside every domain"
    );
}

#[test]
fn test_seeded_random_holes_conserve_tile_their_domains_and_match_brute_force() {
    let mut rng = Rng(0x0166_C0DE);
    let mut worst = 0.0_f64;
    let mut worst_value = 0.0_f64;
    let mut control = 0.0_f64;
    let mut sets = 0;
    let residuals = [
        Residual::Keep,
        Residual::MergeBelow { min_mm: 1_000 },
        Residual::DropBelow { min_mm: 1_000 },
        Residual::Distribute,
    ];
    for hole in 0..200 {
        let depth = 20_000 + rng.below(400_000);
        let domain_spans = runs(&mut rng, depth, 60_000, 30);
        let domains: Vec<Domain> = domain_spans
            .iter()
            .enumerate()
            .map(|(i, &s)| Domain {
                code: format!("D{}", i % 4),
                span: s,
            })
            .collect();
        let inputs: Vec<Input> = runs(&mut rng, depth, 3_000, 10)
            .into_iter()
            .map(|s| {
                let g = rng.unit() * 0.05;
                let value = match rng.below(10) {
                    0 => Bounds { lo: 0.0, hi: 0.001 },
                    1 => Bounds {
                        lo: g,
                        hi: (g * 3.0).min(1.0),
                    },
                    _ => exact(g),
                };
                Input {
                    span: s,
                    value,
                    mass_kg: Some(0.5 + rng.unit() * 6.0),
                }
            })
            .collect();
        for residual in residuals {
            for weighting in [Weighting::Length, Weighting::Mass] {
                for additivity in [
                    AdditivityClass::IntensiveWeighted,
                    AdditivityClass::ExtensiveAdditive,
                ] {
                    let rule = Rule {
                        length_mm: [1_000, 2_000, 2_500, 5_000][hole % 4],
                        weighting,
                        residual,
                        additivity,
                        input_support: SpatialSupport::PointCore,
                    };
                    let what = format!("hole {hole}, {residual:?}, {weighting:?}, {additivity:?}");
                    let set = composite(&domains, &inputs, &rule)
                        .unwrap_or_else(|e| panic!("{what}: {e}"));
                    sets += 1;
                    check_tiling(&set, &domains, &rule, &what);
                    for c in set.composites.iter().chain(&set.dropped) {
                        let expected = brute(c, &inputs, &rule);
                        match (c.value, expected) {
                            (None, None) => assert_eq!(c.sampled_mm, 0, "{what}"),
                            (Some(v), Some(b)) => {
                                worst_value = worst_value
                                    .max(relative(v.lo, b.lo))
                                    .max(relative(v.hi, b.hi));
                            }
                            (v, b) => panic!("{what}: {v:?} against brute force {b:?}"),
                        }
                    }
                    let c = set.conservation;
                    assert!(c.relative_error <= CONSERVATION_TOLERANCE, "{what}: {c:?}");
                    worst = worst.max(c.relative_error);
                    // The totals the check compares are what they say: recomputed here from the composites.
                    let all: Vec<Composite> =
                        set.composites.iter().chain(&set.dropped).cloned().collect();
                    assert!(
                        relative(total(&all, &rule), c.composites.lo) < 1e-13,
                        "{what}"
                    );
                    // Control: leaving the dropped pieces out of the total loses core the check would catch.
                    if set
                        .dropped
                        .iter()
                        .any(|d| d.value.is_some_and(|v| v.lo > 0.0))
                    {
                        control = control.max(relative(total(&set.composites, &rule), c.inputs.lo));
                    }
                }
            }
        }
    }
    eprintln!(
        "{sets} sets: worst conservation {worst:e}, worst value against brute force {worst_value:e}, \
         dropped-pieces control {control:e}"
    );
    assert!(worst_value < 1e-12, "{worst_value:e}");
    assert!(
        control > CONSERVATION_TOLERANCE,
        "the control should exceed the tolerance: {control:e}"
    );
}

#[test]
fn test_what_cannot_be_composited_is_refused() {
    let a = [domain("A", 0, 4000)];
    let ok = [input(0, 4000, 0.5)];
    let code = |e: &DrillError| e.to_refusal().code;
    let nonadditive = Rule {
        additivity: AdditivityClass::NonAdditive,
        ..rule(2000, Residual::Keep)
    };
    let e = composite(&a, &ok, &nonadditive).unwrap_err();
    assert!(matches!(e, DrillError::NonAdditive { .. }), "{e:?}");
    assert_eq!(code(&e), RefusalCode::NonAdditiveAverage103.spec().code);
    let blocks = Rule {
        input_support: SpatialSupport::SmuBlock {
            dx_cm: 500,
            dy_cm: 500,
            dz_cm: 500,
        },
        ..rule(2000, Residual::Keep)
    };
    let e = composite(&a, &ok, &blocks).unwrap_err();
    assert!(matches!(e, DrillError::SupportMismatch { .. }), "{e:?}");
    assert_eq!(code(&e), RefusalCode::SupportMismatch102.spec().code);
    let finer = Rule {
        input_support: SpatialSupport::Composite { length_mm: 1000 },
        ..rule(2000, Residual::Keep)
    };
    assert!(composite(&a, &ok, &finer).is_ok());

    let invalid: Vec<(&str, Vec<Domain>, Vec<Input>, Rule)> = vec![
        (
            "zero length",
            a.to_vec(),
            ok.to_vec(),
            rule(0, Residual::Keep),
        ),
        (
            "overlapping inputs",
            a.to_vec(),
            vec![input(0, 2000, 0.5), input(1000, 3000, 0.5)],
            rule(1000, Residual::Keep),
        ),
        (
            "overlapping domains",
            vec![domain("A", 0, 2000), domain("B", 1500, 3000)],
            ok.to_vec(),
            rule(1000, Residual::Keep),
        ),
        (
            "an empty domain",
            vec![domain("A", 10, 10)],
            ok.to_vec(),
            rule(1000, Residual::Keep),
        ),
        (
            "NaN",
            a.to_vec(),
            vec![input(0, 4000, f64::NAN)],
            rule(1000, Residual::Keep),
        ),
        (
            "bounds out of order",
            a.to_vec(),
            vec![Input {
                value: Bounds { lo: 0.5, hi: 0.25 },
                ..input(0, 4000, 0.0)
            }],
            rule(1000, Residual::Keep),
        ),
        (
            "no mass",
            a.to_vec(),
            ok.to_vec(),
            Rule {
                weighting: Weighting::Mass,
                ..rule(1000, Residual::Keep)
            },
        ),
    ];
    for (what, domains, inputs, r) in invalid {
        let e = composite(&domains, &inputs, &r).unwrap_err();
        assert!(matches!(e, DrillError::Invalid { .. }), "{what}: {e:?}");
        assert_eq!(
            code(&e),
            RefusalCode::OutsidePhysicalDomain109.spec().code,
            "{what}"
        );
    }
}
