//! Known-answer, refusal and rounding-enclosure checks for the typed AISC bridge.
//! Determinism: DeterministicPerIsa (inherits the tested kernel's cross-host qualification).

use super::*;
use fmine_core::rand::PhiloxStream;

fn r(s: &str) -> Rat {
    Rat::from_decimal_str(s).unwrap()
}

fn q(value: &str, half: &str, at: &str) -> PrintedQty {
    PrintedQty {
        value: r(value),
        half_step: r(half),
        at: at.into(),
    }
}

fn period(tag: &str, cost: &str, sold: &str, rate: &str) -> WaterfallPeriod {
    let cost = q(cost, "0.5", &format!("{tag}!C"));
    WaterfallPeriod {
        lines: vec![("total".into(), cost.clone())],
        cost,
        oz_sold: q(sold, "0.5", &format!("{tag}!Q")),
        printed_rate: q(rate, "0.5", &format!("{tag}!P")),
        tonnes: Factor::Value(q("1000", "0.5", &format!("{tag}!T"))),
        grade: Factor::Value(q("2", "0.005", &format!("{tag}!g"))),
        recovery: Factor::Value(q("0.9", "0.0005", &format!("{tag}!R"))),
        produced: Factor::Value(q("50", "0.5", &format!("{tag}!O"))),
        basis: BasisTag::Consolidated100,
    }
}

fn missing_recovery(p: &mut WaterfallPeriod) {
    let at = match &p.recovery {
        Factor::Value(q) => q.at.clone(),
        Factor::NotPublished { at, .. } => at.clone(),
    };
    p.recovery = Factor::NotPublished {
        token: "n/a".into(),
        at,
    };
}

fn bar_named<'a>(bridge: &'a Bridge, id: &str) -> &'a Bar {
    bridge.bars().iter().find(|b| b.id == id).unwrap()
}

fn sum(bridge: &Bridge) -> f64 {
    bridge.bars().iter().map(|b| b.point.as_f64()).sum()
}

#[test]
fn test_synthetic_total_1285_sales_timing_536_915() {
    // Invented inputs from the historical GB-26 test; these are not published Barrick physical states.
    let mut p0 = period("synthetic0", "60.959", "47", "1297");
    let mut p1 = period("synthetic1", "185.904", "72", "2582");
    p0.tonnes = Factor::Value(q("720", "0.5", "synthetic0!T"));
    p1.tonnes = Factor::Value(q("735", "0.5", "synthetic1!T"));
    p0.grade = Factor::Value(q("2.25", "0.005", "synthetic0!g"));
    p1.grade = Factor::Value(q("2.52", "0.005", "synthetic1!g"));
    p1.recovery = Factor::Value(q("0.905", "0.0005", "synthetic1!R"));
    p0.produced = Factor::Value(q("47", "0.5", "synthetic0!O"));
    p1.produced = Factor::Value(q("54", "0.5", "synthetic1!O"));
    let bridge = decompose_aisc_waterfall(&p0, &p1).unwrap();
    assert!(matches!(bridge, Bridge::Lmdi { .. }));
    assert_eq!(bridge.gap().aisc_0, r("1297"));
    assert_eq!(bridge.gap().aisc_1, r("2582"));
    assert_eq!(bridge.gap().total_delta, r("1285"));
    assert_eq!(bridge.gap().printed_rate_gap, ZERO);
    // Independent 60-digit Decimal result already established for the legacy API, not a kernel snapshot.
    assert!((bar_named(&bridge, "sales_timing").point.as_f64() + 536.9148672729303).abs() < 1e-9);
    assert!((sum(&bridge) - 1285.0).abs() < 1e-9);
    assert_eq!(
        bar_named(&bridge, "grade").inputs,
        [
            "synthetic0!C",
            "synthetic0!Q",
            "synthetic0!g",
            "synthetic1!C",
            "synthetic1!Q",
            "synthetic1!g"
        ]
    );
}

fn north_mara() -> (WaterfallPeriod, WaterfallPeriod) {
    // Hand-entered published cells, independently read by repro/barrick/waterfall_census.py before
    // either new Rust kernel/adapter was inspected. This pure test establishes arithmetic on those
    // inputs; source-hash verification, basis authentication and first-vintage selection belong to V.
    let mut p0 = period("2025", "64", "50", "1292");
    let mut p1 = period("2026", "93", "37", "2577");
    p0.cost = q("64", "6", "Barrick_Q2_2025_AISC.xlsx!'Q2 2025'!P18");
    p1.cost = q("93", "5.5", "Barrick_Q2_2026_AISC.xlsx!'Q2 2026'!M17");
    p0.oz_sold = q("50", "0.5", "Barrick_Q2_2025_AISC.xlsx!'Q2 2025'!P19");
    p1.oz_sold = q("37", "0.5", "Barrick_Q2_2026_AISC.xlsx!'Q2 2026'!M18");
    p0.printed_rate = q("1292", "0.5", "Barrick_Q2_2025_AISC.xlsx!'Q2 2025'!P23");
    p1.printed_rate = q("2577", "0.5", "Barrick_Q2_2026_AISC.xlsx!'Q2 2026'!M21");
    for (p, year, tonnes, grade, recovery, produced) in [
        (&mut p0, "2025", "698", "3.16", "0.88", "62"),
        (&mut p1, "2026", "688", "1.80", "0.85", "35"),
    ] {
        let file = format!("Barrick_Q2_{year}_Mine_Stats.xlsx!Minestats3");
        p.tonnes = Factor::Value(q(tonnes, "0.5", &format!("{file}!H12")));
        p.grade = Factor::Value(q(grade, "0.005", &format!("{file}!H18")));
        p.recovery = Factor::Value(q(recovery, "0.005", &format!("{file}!H19")));
        p.produced = Factor::Value(q(produced, "0.5", &format!("{file}!H20")));
        p.basis = BasisTag::Attributable { share: r("0.84") };
        p.lines.clear(); // LMDI uses supplied C, not a synthetic replacement set of component lines.
    }
    (p0, p1)
}

#[test]
fn test_north_mara_gap_1905_over_37_and_independent_bar_answers() {
    let (p0, p1) = north_mara();
    let bridge = decompose_aisc_waterfall(&p0, &p1).unwrap();
    assert_eq!(bridge.gap().aisc_0, r("1280"));
    assert_eq!(bridge.gap().aisc_1, Rat::new(93000, 37).unwrap());
    assert_eq!(bridge.gap().total_delta, Rat::new(45640, 37).unwrap());
    assert_eq!(bridge.gap().printed_rate_gap_0, r("12"));
    assert_eq!(bridge.gap().printed_rate_gap_1, Rat::new(2349, 37).unwrap());
    assert_eq!(bridge.gap().printed_rate_gap, Rat::new(1905, 37).unwrap());
    // Independent 80-digit Decimal log/mean calculation from the cited cells, not generated by Rust.
    for (id, expected) in [
        ("cost", 683.1202620861866),
        ("tonnes", 26.377236946656755),
        ("grade", 1028.7214432890034),
        ("recovery", 63.40210609531504),
        ("balance", -73.32638946573617),
        ("sales_timing", -494.7811454379121),
    ] {
        let bar = bar_named(&bridge, id);
        assert!((bar.point.as_f64() - expected).abs() < 1e-9, "{id}");
        assert!(bar.enclosure[0] <= expected && expected <= bar.enclosure[1]);
    }
    assert!((sum(&bridge) + bridge.gap().printed_rate_gap.to_f64() - 1285.0).abs() < 1e-9);
    assert_eq!(
        bridge
            .gap()
            .total_delta
            .checked_add(bridge.gap().printed_rate_gap)
            .unwrap(),
        r("1285")
    );
}

#[test]
fn test_north_mara_grade_control_is_non_identifying() {
    let (p0, mut p1) = north_mara();
    let original = decompose_aisc_waterfall(&p0, &p1).unwrap();
    let Factor::Value(grade) = &mut p1.grade else {
        unreachable!()
    };
    grade.value = r("1.98"); // Explicit counterfactual +10% input perturbation, not a source correction.
    let edited = decompose_aisc_waterfall(&p0, &p1).unwrap();
    let grade_shift =
        bar_named(&edited, "grade").point.as_f64() - bar_named(&original, "grade").point.as_f64();
    let balance_shift = bar_named(&edited, "balance").point.as_f64()
        - bar_named(&original, "balance").point.as_f64();
    // Independent 80-digit oracle: L(93000/37,1280)*ln(1.1), not the stale plan's approximate $177.
    assert!((grade_shift + 174.21850713853705).abs() < 1e-9);
    assert!((balance_shift - 174.21850713853705).abs() < 1e-9);
    assert_eq!(original.gap(), edited.gap());
    for id in ["cost", "tonnes", "recovery", "sales_timing"] {
        assert_eq!(bar_named(&original, id), bar_named(&edited, id));
    }
    assert!((sum(&original) - sum(&edited)).abs() < 1e-9);
}

#[test]
fn test_recovery_na_exact_signed_fallback_known_answer() {
    let mut p0 = period("p0", "100", "50", "2003");
    let mut p1 = period("p1", "115", "40", "2880");
    p0.lines = vec![
        ("operating".into(), q("100", "0.5", "p0!operating")),
        ("credit".into(), q("-10", "0.5", "p0!credit")),
        ("sustaining".into(), q("10", "0.5", "p0!sustaining")),
    ];
    p1.lines = vec![
        ("operating".into(), q("130", "0.5", "p1!operating")),
        ("credit".into(), q("-20", "0.5", "p1!credit")),
        ("sustaining".into(), q("5", "0.5", "p1!sustaining")),
    ];
    missing_recovery(&mut p0);
    let bridge = decompose_aisc_waterfall(&p0, &p1).unwrap();
    let Bridge::AdditiveFallback { reason, .. } = &bridge else {
        panic!("missing fallback")
    };
    assert!(reason.contains("period 0 recovery not published: `n/a` (p0!R)"));
    // Hand calculation: mean(1/Q)=9/400; mean(C)=215/2; Δ(1/Q)=1/200.
    for (id, expected) in [
        ("line:operating", "675"),
        ("line:credit", "-225"),
        ("line:sustaining", "-112.5"),
        ("volume", "537.5"),
    ] {
        assert_eq!(bar_named(&bridge, id).point, BarPoint::Exact(r(expected)));
    }
    assert_eq!(bridge.gap().total_delta, r("875"));
    assert_eq!(bridge.gap().printed_rate_gap_0, r("3"));
    assert_eq!(bridge.gap().printed_rate_gap_1, r("5"));
    assert_eq!(bridge.gap().printed_rate_gap, r("2"));
    assert_eq!(sum(&bridge) + bridge.gap().printed_rate_gap.to_f64(), 877.0);
    assert_eq!(
        bar_named(&bridge, "line:credit").inputs,
        ["p0!Q", "p0!credit", "p1!Q", "p1!credit"]
    );
}

#[test]
fn test_printed_gaps_never_change_bars_and_equal_gaps_survive() {
    let p0 = period("p0", "100", "50", "2003");
    let mut p1 = period("p1", "150", "50", "3003");
    let bridge = decompose_aisc_waterfall(&p0, &p1).unwrap();
    assert_eq!(bridge.gap().printed_rate_gap_0, r("3"));
    assert_eq!(bridge.gap().printed_rate_gap_1, r("3"));
    assert_eq!(bridge.gap().printed_rate_gap, ZERO);
    assert!((bar_named(&bridge, "cost").point.as_f64() - 1000.0).abs() < 1e-9);
    p1.printed_rate.value = r("2003");
    let edited = decompose_aisc_waterfall(&p0, &p1).unwrap();
    assert_eq!(bridge.bars(), edited.bars());
    assert_eq!(edited.gap().printed_rate_gap, r("-1000"));
    assert!((sum(&edited) + edited.gap().printed_rate_gap.to_f64()).abs() < 1e-9);
    // LMDI total C and the rounded component midpoints are independent observations. Only the signed
    // fallback requires the latter to close exactly; an LMDI result must not silently replace C by them.
    p1.lines[0].1.value = r("149");
    assert_eq!(
        decompose_aisc_waterfall(&p0, &p1).unwrap().bars(),
        edited.bars()
    );
    missing_recovery(&mut p1);
    let refusal = decompose_aisc_waterfall(&p0, &p1).unwrap_err();
    assert_eq!(refusal.code, "FMINE-REF-104");
    assert!(
        refusal
            .what
            .contains("sum to 149, supplied cost is 150; no plug")
    );
}

#[test]
fn test_signed_nonpositive_cost_is_not_forced_into_a_logarithm() {
    let mut p0 = period("p0", "-5", "10", "-500");
    let mut p1 = period("p1", "-2", "10", "-200");
    p0.lines = vec![
        ("credit".into(), q("-5", "0.5", "p0!credit")),
        ("other".into(), q("0", "0.5", "p0!other")),
    ];
    p1.lines = vec![
        ("credit".into(), q("-3", "0.5", "p1!credit")),
        ("other".into(), q("1", "0.5", "p1!other")),
    ];
    let bridge = decompose_aisc_waterfall(&p0, &p1).unwrap();
    let Bridge::AdditiveFallback { reason, .. } = &bridge else {
        panic!("missing fallback")
    };
    assert!(reason.contains("cost rounding interval"));
    assert_eq!(
        bar_named(&bridge, "line:credit").point,
        BarPoint::Exact(r("200"))
    );
    assert_eq!(
        bar_named(&bridge, "line:other").point,
        BarPoint::Exact(r("100"))
    );
    assert_eq!(bar_named(&bridge, "volume").point, BarPoint::Exact(ZERO));
    assert_eq!(bridge.gap().total_delta, r("300"));
}

#[test]
fn test_q_zero_and_unbounded_rounding_refuse_with_agreed_code() {
    let p0 = period("p0", "100", "50", "2000");
    for (sold, half) in [("0", "0"), ("-1", "0.5"), ("0.5", "0.5")] {
        let mut p1 = p0.clone();
        p1.oz_sold = q(sold, half, "p1!Q");
        let err = decompose_aisc_waterfall(&p0, &p1).unwrap_err();
        assert_eq!(err.code, "FMINE-REF-104");
        assert!(err.what.contains("p1!Q"));
    }
}

#[test]
fn test_basis_mismatch_undeclared_basis_and_invalid_share_refuse() {
    let p0 = period("p0", "100", "50", "2000");
    for (basis, code) in [
        (BasisTag::Attributable { share: ONE }, "FMINE-REF-105"),
        (BasisTag::Unprinted, "FMINE-REF-107"),
        (BasisTag::Attributable { share: r("1.1") }, "FMINE-REF-109"),
        (BasisTag::Attributable { share: ZERO }, "FMINE-REF-109"),
    ] {
        let p1 = WaterfallPeriod {
            basis,
            ..p0.clone()
        };
        assert_eq!(decompose_aisc_waterfall(&p0, &p1).unwrap_err().code, code);
    }
    let mut p0 = p0;
    p0.basis = BasisTag::Attributable { share: r("0.6") };
    let mut p1 = p0.clone();
    assert!(decompose_aisc_waterfall(&p0, &p1).is_ok());
    p1.basis = BasisTag::Attributable { share: r("0.5") };
    assert_eq!(
        decompose_aisc_waterfall(&p0, &p1).unwrap_err().code,
        "FMINE-REF-105"
    );
}

#[test]
fn test_invalid_precision_source_and_overflow_refuse() {
    let p0 = period("p0", "100", "50", "2000");
    let mut p1 = p0.clone();
    p1.cost.half_step = r("-0.5");
    assert_eq!(
        decompose_aisc_waterfall(&p0, &p1).unwrap_err().code,
        "FMINE-REF-109"
    );
    p1 = p0.clone();
    p1.cost.at.clear();
    assert_eq!(
        decompose_aisc_waterfall(&p0, &p1).unwrap_err().code,
        "FMINE-REF-201"
    );
    p1 = p0.clone();
    p1.cost.value = Rat::from_integer(i128::MAX);
    p1.cost.half_step = ONE;
    assert_eq!(
        decompose_aisc_waterfall(&p0, &p1).unwrap_err().code,
        "FMINE-REF-207"
    );
}

#[test]
fn test_fallback_never_invents_missing_lines_or_duplicate_identities() {
    let mut p0 = period("p0", "100", "50", "2000");
    missing_recovery(&mut p0);
    let mut p1 = p0.clone();
    p1.lines[0].0 = "different".into();
    let err = decompose_aisc_waterfall(&p0, &p1).unwrap_err();
    assert_eq!(err.code, "FMINE-REF-104");
    assert!(err.what.contains("absent line is not an observed zero"));
    p1 = p0.clone();
    p1.lines.push(p1.lines[0].clone());
    assert!(
        decompose_aisc_waterfall(&p0, &p1)
            .unwrap_err()
            .what
            .contains("repeated line id")
    );
    p1.lines.clear();
    assert!(
        decompose_aisc_waterfall(&p0, &p1)
            .unwrap_err()
            .what
            .contains("no plug")
    );
}

#[test]
fn test_straddling_bar_flagged_and_nonpositive_physical_box_falls_back() {
    let p0 = period("p0", "100", "50", "2000");
    let p1 = period("p1", "100.05", "50", "2001");
    let bridge = decompose_aisc_waterfall(&p0, &p1).unwrap();
    let cost = bar_named(&bridge, "cost");
    assert!(cost.point.as_f64() > 0.0);
    assert!(cost.enclosure[0] < 0.0 && cost.enclosure[1] > 0.0);
    assert!(cost.sign_not_determined);
    let p2 = period("p1", "200", "50", "4000");
    assert!(!bar_named(&decompose_aisc_waterfall(&p0, &p2).unwrap(), "cost").sign_not_determined);
    for (name, factor) in [
        ("tonnes", q("0", "0", "p1!T")),
        ("grade", q("0.005", "0.005", "p1!g")),
        ("recovery", q("1", "0.005", "p1!R")),
        ("produced", q("-1", "0.5", "p1!O")),
    ] {
        let mut bad = p1.clone();
        let target = match name {
            "tonnes" => &mut bad.tonnes,
            "grade" => &mut bad.grade,
            "recovery" => &mut bad.recovery,
            _ => &mut bad.produced,
        };
        *target = Factor::Value(factor);
        let Bridge::AdditiveFallback { reason, .. } = decompose_aisc_waterfall(&p0, &bad).unwrap()
        else {
            panic!("nonpositive/unphysical interval entered LMDI")
        };
        assert!(reason.contains(name));
    }
}

fn rng() -> PhiloxStream {
    PhiloxStream::new(super::super::LMDI_DIGEST_KEY)
}

#[test]
fn test_property_fallback_exact_random_lines_with_negatives() {
    let mut rng = rng();
    for case in 0..10_000 {
        let count = 1 + (rng.next_u64() % 6) as usize;
        let q0 = 1 + i128::from(rng.next_u64() % 200);
        let q1 = 1 + i128::from(rng.next_u64() % 200);
        let mut p0 = period("p0", "1", "1", "1000");
        let mut p1 = period("p1", "1", "1", "1000");
        p0.oz_sold.value = Rat::from_integer(q0);
        p1.oz_sold.value = Rat::from_integer(q1);
        missing_recovery(&mut p0);
        p0.lines.clear();
        p1.lines.clear();
        let (mut c0, mut c1) = (0_i128, 0_i128);
        let mut expected = BTreeMap::new();
        for j in 0..count {
            let v0 = i128::from(rng.next_u64() % 401) - 200;
            let v1 = i128::from(rng.next_u64() % 401) - 200;
            c0 += v0;
            c1 += v1;
            let id = format!("line{j}");
            p0.lines
                .push((id.clone(), q(&v0.to_string(), "0.5", &format!("p0!{id}"))));
            p1.lines
                .push((id.clone(), q(&v1.to_string(), "0.5", &format!("p1!{id}"))));
            // Closed-form rational derived separately from the kernel's reciprocal arithmetic.
            expected.insert(
                format!("line:{id}"),
                Rat::new(1000 * (v1 - v0) * (q0 + q1), 2 * q0 * q1).unwrap(),
            );
        }
        expected.insert(
            "volume".into(),
            Rat::new(1000 * (c0 + c1) * (q0 - q1), 2 * q0 * q1).unwrap(),
        );
        p0.cost.value = Rat::from_integer(c0);
        p1.cost.value = Rat::from_integer(c1);
        let bridge = decompose_aisc_waterfall(&p0, &p1).unwrap();
        let expected_delta = Rat::new(1000 * (c1 * q0 - c0 * q1), q0 * q1).unwrap();
        assert_eq!(bridge.gap().total_delta, expected_delta);
        let mut exact_sum = ZERO;
        for b in bridge.bars() {
            let BarPoint::Exact(point) = b.point else {
                panic!("approximate fallback")
            };
            assert_eq!(point, expected[&b.id]);
            exact_sum = exact_sum.checked_add(point).unwrap();
            assert!(b.enclosure[0] <= point.to_f64() && point.to_f64() <= b.enclosure[1]);
        }
        assert_eq!(exact_sum, expected_delta);
        if case < 100 {
            p0.lines.reverse();
            p1.lines.rotate_left(count / 2);
            assert_eq!(decompose_aisc_waterfall(&p0, &p1).unwrap(), bridge);
        }
    }
}

#[test]
fn test_property_lmdi_order_independent_and_enclosure_contains_point() {
    let mut rng = rng();
    for _ in 0..256 {
        let mut p0 = period("p0", "100", "50", "2000");
        let mut p1 = period("p1", "100", "50", "2000");
        for p in [&mut p0, &mut p1] {
            p.cost.value = Rat::from_integer(10 + i128::from(rng.next_u64() % 200));
            p.oz_sold.value = Rat::from_integer(50 + i128::from(rng.next_u64() % 150));
            for f in [&mut p.tonnes, &mut p.grade, &mut p.produced] {
                let Factor::Value(q) = f else { unreachable!() };
                q.value = q
                    .value
                    .checked_mul(Rat::new(50 + i128::from(rng.next_u64() % 151), 100).unwrap())
                    .unwrap();
            }
        }
        let bridge = decompose_aisc_waterfall(&p0, &p1).unwrap();
        let Bridge::Lmdi {
            closure_residual, ..
        } = bridge
        else {
            panic!("positive input refused LMDI")
        };
        assert_eq!(
            closure_residual,
            bridge.gap().total_delta.to_f64() - sum(&bridge)
        );
        let mut bars = bridge.bars().to_vec();
        for b in &bars {
            let point = b.point.as_f64();
            assert!(b.enclosure[0] <= point && point <= b.enclosure[1]);
            assert_eq!(
                b.sign_not_determined,
                b.enclosure[0] <= 0.0 && b.enclosure[1] >= 0.0
            );
        }
        for _ in 0..8 {
            for i in (1..bars.len()).rev() {
                bars.swap(i, (rng.next_u64() % (i as u64 + 1)) as usize);
            }
            let reordered: f64 = bars.iter().map(|b| b.point.as_f64()).sum();
            assert!((reordered - bridge.gap().total_delta.to_f64()).abs() < 1e-9);
        }
    }
}

fn qty_mut(p: &mut WaterfallPeriod, index: usize) -> &mut PrintedQty {
    if index == 0 {
        return &mut p.cost;
    }
    if index == 1 {
        return &mut p.oz_sold;
    }
    let f = match index {
        2 => &mut p.tonnes,
        3 => &mut p.grade,
        4 => &mut p.recovery,
        _ => &mut p.produced,
    };
    let Factor::Value(q) = f else { unreachable!() };
    q
}

#[test]
fn test_lmdi_rounding_enclosures_contain_all_4096_input_corners() {
    let p0 = period("p0", "100", "50", "2000");
    let p1 = period("p1", "115", "40", "2880");
    let outer = decompose_aisc_waterfall(&p0, &p1).unwrap();
    for mask in 0..4096 {
        let (mut c0, mut c1) = (p0.clone(), p1.clone());
        for (period, p) in [&mut c0, &mut c1].into_iter().enumerate() {
            for field in 0..6 {
                let q = qty_mut(p, field);
                q.value = if mask & (1 << (6 * period + field)) == 0 {
                    q.value.checked_sub(q.half_step).unwrap()
                } else {
                    q.value.checked_add(q.half_step).unwrap()
                };
                q.half_step = ZERO;
            }
        }
        let corner = decompose_aisc_waterfall(&c0, &c1).unwrap();
        for (inside, bound) in corner.bars().iter().zip(outer.bars()) {
            let v = inside.point.as_f64();
            assert!(
                bound.enclosure[0] <= v && v <= bound.enclosure[1],
                "mask={mask}, bar={}",
                bound.id
            );
        }
    }
}

#[test]
fn test_additive_rounding_enclosures_contain_all_64_input_corners() {
    let mut p0 = period("p0", "100", "50", "2000");
    let mut p1 = period("p1", "115", "40", "2875");
    p0.lines = vec![
        ("operating".into(), q("110", "0.5", "p0!operating")),
        ("credit".into(), q("-10", "0.5", "p0!credit")),
    ];
    p1.lines = vec![
        ("operating".into(), q("130", "0.5", "p1!operating")),
        ("credit".into(), q("-15", "0.5", "p1!credit")),
    ];
    p0.cost.half_step = ONE;
    p1.cost.half_step = ONE;
    missing_recovery(&mut p0);
    let outer = decompose_aisc_waterfall(&p0, &p1).unwrap();
    for mask in 0..64 {
        let (mut c0, mut c1) = (p0.clone(), p1.clone());
        for (period, p) in [&mut c0, &mut c1].into_iter().enumerate() {
            for (field, q) in p
                .lines
                .iter_mut()
                .map(|(_, q)| q)
                .chain([&mut p.oz_sold])
                .enumerate()
            {
                q.value = if mask & (1 << (3 * period + field)) == 0 {
                    q.value.checked_sub(q.half_step).unwrap()
                } else {
                    q.value.checked_add(q.half_step).unwrap()
                };
                q.half_step = ZERO;
            }
            p.cost.value = p.lines[0].1.value.checked_add(p.lines[1].1.value).unwrap();
            p.cost.half_step = ZERO;
        }
        let corner = decompose_aisc_waterfall(&c0, &c1).unwrap();
        for (inside, bound) in corner.bars().iter().zip(outer.bars()) {
            let v = inside.point.as_f64();
            assert!(
                bound.enclosure[0] <= v && v <= bound.enclosure[1],
                "mask={mask}, bar={}",
                bound.id
            );
        }
    }
}

#[test]
fn test_logarithmic_mean_enclosure_known_answers_near_equality_and_tiny_scale() {
    // Independent 80-place Decimal values, also used to audit the earlier point-kernel correction.
    for (a, b, expected) in [
        (2582.0, 1297.0, 1866.3480233476275),
        (1e-18, 1e-21, 1.4462006247378286e-19),
        (1e-300, 1e-303, 1.4462006247378286e-301),
        (1.0, 1.0_f64.next_up(), 1.0),
    ] {
        let enclosure = mean_at(a, b).unwrap();
        assert!(enclosure.lo <= expected && expected <= enclosure.hi);
        assert!(a.min(b) <= enclosure.lo && enclosure.hi <= a.max(b));
    }
}
