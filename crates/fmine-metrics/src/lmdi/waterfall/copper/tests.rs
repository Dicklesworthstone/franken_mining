//! DESIGN: invented independent arithmetic controls for the explicit copper-unit kernel.
//! Determinism: DeterministicPerIsa
//! Points are exact and interval comparisons use outward floats.

use super::*;

fn r(numerator: i128, denominator: i128) -> Rat {
    Rat::new(numerator, denominator).unwrap()
}

fn quantity(value: Rat, half_step: Rat, at: &str) -> PrintedQty {
    PrintedQty {
        value,
        half_step,
        at: at.into(),
    }
}

fn period(later: bool) -> Period {
    let (cost, operating, sustaining, sold) = if later {
        (90, 75, 15, 60)
    } else {
        (60, 50, 10, 50)
    };
    let tag = if later { "second" } else { "first" };
    Period {
        metric: Metric::Aisc,
        definition: "invented.copper.aisc@1".into(),
        cost: quantity(r(cost, 1), r(0, 1), &format!("{tag}:cost")),
        lines: vec![
            (
                "operating".into(),
                quantity(r(operating, 1), r(0, 1), &format!("{tag}:operating")),
            ),
            (
                "sustaining".into(),
                quantity(r(sustaining, 1), r(0, 1), &format!("{tag}:sustaining")),
            ),
        ],
        sold: quantity(r(sold, 1), r(0, 1), &format!("{tag}:sold")),
        sold_unit: SalesUnit::MillionPounds,
        reported_rate: None,
        basis: BasisTag::Attributable { share: r(1, 2) },
    }
}

fn point(bridge: &Bridge, id: &str) -> Rat {
    match bridge.bars.iter().find(|bar| bar.id == id).unwrap().point {
        BarPoint::Exact(value) => value,
        BarPoint::Approx(_) => panic!("copper additive point must remain exact"),
    }
}

#[test]
fn exact_usd_per_pound_bridge_closes_with_separate_reported_gaps() {
    let (mut first, mut second) = (period(false), period(true));
    first.reported_rate = Some(quantity(r(121, 100), r(1, 200), "first:rate"));
    second.reported_rate = Some(quantity(r(151, 100), r(1, 200), "second:rate"));
    let result = decompose(&first, &second).unwrap();
    // 60M/50M = 6/5; 90M/60M = 3/2. Symmetric contributions are 11/24,
    // 11/120 and -1/4 USD/lb, with exact sum 3/10.
    assert_eq!(result.rates.rate_0, r(6, 5));
    assert_eq!(result.rates.rate_1, r(3, 2));
    assert_eq!(result.rates.total_delta, r(3, 10));
    assert_eq!(point(&result, "line:operating"), r(11, 24));
    assert_eq!(point(&result, "line:sustaining"), r(11, 120));
    assert_eq!(point(&result, "sold_volume"), r(-1, 4));
    assert_eq!(result.rates.reported_rate_gap_0, Some(r(1, 100)));
    assert_eq!(result.rates.reported_rate_gap_1, Some(r(1, 100)));
    assert_eq!(result.rates.reported_rate_gap, Some(r(0, 1)));
    second.reported_rate.as_mut().unwrap().value = r(-7, 1);
    let edited = decompose(&first, &second).unwrap();
    assert_eq!(edited.bars, result.bars);
    assert_eq!(edited.rates.rate_1, result.rates.rate_1);
    assert_ne!(
        edited.rates.reported_rate_gap,
        result.rates.reported_rate_gap
    );
    assert!(result.bars.iter().all(|bar| {
        bar.point.as_f64() >= bar.enclosure[0] && bar.point.as_f64() <= bar.enclosure[1]
    }));
}

#[test]
fn statutory_copper_mass_conversion_preserves_values_and_rounding_radii() {
    let (mut first, mut second) = (period(false), period(true));
    first.sold.half_step = r(1, 10);
    second.sold.half_step = r(1, 5);
    let reference = decompose(&first, &second).unwrap();
    // One Mlb is exactly 0.45359237 kt. No troy-ounce factor appears.
    let kt_per_mlb = r(45_359_237, 100_000_000);
    first.sold.value = first.sold.value.checked_mul(kt_per_mlb).unwrap();
    first.sold.half_step = first.sold.half_step.checked_mul(kt_per_mlb).unwrap();
    first.sold_unit = SalesUnit::Kilotonnes;
    second.sold.value = second.sold.value.checked_mul(r(1_000_000, 1)).unwrap();
    second.sold.half_step = second.sold.half_step.checked_mul(r(1_000_000, 1)).unwrap();
    second.sold_unit = SalesUnit::Pounds;
    assert_eq!(decompose(&first, &second).unwrap(), reference);
}

#[test]
fn signed_c1_costs_and_absent_reported_rates_are_preserved() {
    let (mut first, mut second) = (period(false), period(true));
    for (p, cost) in [(&mut first, -10), (&mut second, 5)] {
        p.metric = Metric::C1;
        p.definition = "invented.c1@1".into();
        p.cost.value = r(cost, 1);
        p.lines = vec![("net_cost".into(), p.cost.clone())];
    }
    let result = decompose(&first, &second).unwrap();
    assert_eq!(result.metric, Metric::C1);
    assert_eq!(result.rates.rate_0, r(-1, 5));
    assert_eq!(result.rates.rate_1, r(1, 12));
    assert_eq!(result.rates.total_delta, r(17, 60));
    assert_eq!(point(&result, "line:net_cost"), r(11, 40));
    assert_eq!(point(&result, "sold_volume"), r(1, 120));
    assert_eq!(result.rates.reported_rate_0, None);
    assert_eq!(result.rates.reported_rate_1, None);
    assert_eq!(result.rates.reported_rate_gap, None);
    for p in [&mut first, &mut second] {
        p.cost.value = ZERO;
        p.lines[0].1.value = ZERO;
    }
    let zero = decompose(&first, &second).unwrap();
    assert_eq!(zero.rates.total_delta, ZERO);
    assert!(
        zero.bars
            .iter()
            .all(|bar| bar.point == BarPoint::Exact(ZERO))
    );
    assert!(zero.bars.iter().all(|bar| bar.sign_not_determined));
}

#[test]
fn negative_components_remain_separate_and_line_order_is_irrelevant() {
    let (mut first, mut second) = (period(false), period(true));
    for (p, gross, credit) in [(&mut first, 100, -40), (&mut second, 150, -60)] {
        p.lines[0].0 = "gross".into();
        p.lines[0].1.value = r(gross, 1);
        p.lines[1].0 = "credit".into();
        p.lines[1].1.value = r(credit, 1);
    }
    let result = decompose(&first, &second).unwrap();
    assert_eq!(point(&result, "line:gross"), r(11, 12));
    assert_eq!(point(&result, "line:credit"), r(-11, 30));
    assert_eq!(result.rates.total_delta, r(3, 10));
    first.lines.reverse();
    second.lines.reverse();
    assert_eq!(decompose(&first, &second).unwrap(), result);
    let reverse = decompose(&second, &first).unwrap();
    assert_eq!(reverse.rates.total_delta, r(-3, 10));
    for original in &result.bars {
        assert_eq!(
            point(&reverse, &original.id),
            ZERO.checked_sub(point(&result, &original.id)).unwrap()
        );
    }
}

#[test]
fn component_and_sales_rounding_enclosures_contain_all_exact_corners() {
    let (mut first, mut second) = (period(false), period(true));
    for p in [&mut first, &mut second] {
        p.cost.half_step = r(1, 1);
        p.sold.half_step = r(1, 2);
        for (_, amount) in &mut p.lines {
            amount.half_step = r(1, 2);
        }
    }
    let report = decompose(&first, &second).unwrap();
    // Two signed line amounts at each endpoint, and two independent sold quantities: 64 corners.
    for mask in 0..64 {
        let (mut a, mut b) = (first.clone(), second.clone());
        for (bit, qty) in [
            &mut a.lines[0].1,
            &mut b.lines[0].1,
            &mut a.sold,
            &mut b.sold,
        ]
        .into_iter()
        .enumerate()
        {
            let offset = if mask & (1 << bit) == 0 {
                r(-1, 2)
            } else {
                r(1, 2)
            };
            qty.value = qty.value.checked_add(offset).unwrap();
            qty.half_step = ZERO;
        }
        for (bit, qty) in [(4, &mut a.lines[1].1), (5, &mut b.lines[1].1)] {
            qty.value = qty
                .value
                .checked_add(if mask & (1 << bit) == 0 {
                    r(-1, 2)
                } else {
                    r(1, 2)
                })
                .unwrap();
            qty.half_step = ZERO;
        }
        for p in [&mut a, &mut b] {
            p.cost.value = p.lines[0].1.value.checked_add(p.lines[1].1.value).unwrap();
            p.cost.half_step = ZERO;
        }
        // Independently evaluate the average of both change orders: first change cost then volume,
        // and first volume then cost. The two allocations coincide only after averaging.
        for (index, id) in ["line:operating", "line:sustaining"]
            .into_iter()
            .enumerate()
        {
            let delta = b.lines[index]
                .1
                .value
                .checked_sub(a.lines[index].1.value)
                .unwrap();
            let expected = delta
                .checked_div(a.sold.value)
                .unwrap()
                .checked_add(delta.checked_div(b.sold.value).unwrap())
                .unwrap()
                .checked_div(TWO)
                .unwrap();
            let bar = report.bars.iter().find(|bar| bar.id == id).unwrap();
            assert!(bar.enclosure[0] <= expected.to_f64());
            assert!(expected.to_f64() <= bar.enclosure[1]);
        }
        let expected_volume = a
            .cost
            .value
            .checked_div(b.sold.value)
            .unwrap()
            .checked_sub(a.cost.value.checked_div(a.sold.value).unwrap())
            .unwrap()
            .checked_add(
                b.cost
                    .value
                    .checked_div(b.sold.value)
                    .unwrap()
                    .checked_sub(b.cost.value.checked_div(a.sold.value).unwrap())
                    .unwrap(),
            )
            .unwrap()
            .checked_div(TWO)
            .unwrap();
        let volume = report
            .bars
            .iter()
            .find(|bar| bar.id == "sold_volume")
            .unwrap();
        assert!(volume.enclosure[0] <= expected_volume.to_f64());
        assert!(expected_volume.to_f64() <= volume.enclosure[1]);
    }
}

#[test]
fn incompatible_definitions_populations_and_denominators_refuse_without_plugs() {
    let first = period(false);
    let mut second = period(true);
    second.metric = Metric::C1;
    assert_eq!(
        decompose(&first, &second).unwrap_err().code,
        "FMINE-REF-105"
    );
    second = period(true);
    second.definition.push_str("-different");
    assert_eq!(
        decompose(&first, &second).unwrap_err().code,
        "FMINE-REF-105"
    );
    second = period(true);
    second.basis = BasisTag::Unprinted;
    assert_eq!(
        decompose(&first, &second).unwrap_err().code,
        "FMINE-REF-107"
    );
    second = period(true);
    second.cost.value = r(91, 1);
    assert_eq!(
        decompose(&first, &second).unwrap_err().code,
        "FMINE-REF-104"
    );
    second = period(true);
    second.lines[1].0 = "missing-counterpart".into();
    assert_eq!(
        decompose(&first, &second).unwrap_err().code,
        "FMINE-REF-104"
    );
    second = period(true);
    second.lines.push(second.lines[0].clone());
    assert_eq!(
        decompose(&first, &second).unwrap_err().code,
        "FMINE-REF-104"
    );
    second = period(true);
    second.sold.half_step = second.sold.value;
    assert_eq!(
        decompose(&first, &second).unwrap_err().code,
        "FMINE-REF-104"
    );
    second = period(true);
    second.sold.half_step = r(-1, 2);
    assert_eq!(
        decompose(&first, &second).unwrap_err().code,
        "FMINE-REF-109"
    );
    second = period(true);
    second.sold.at.clear();
    assert_eq!(
        decompose(&first, &second).unwrap_err().code,
        "FMINE-REF-201"
    );
}

#[test]
fn exact_conversion_overflow_is_a_refusal() {
    let first = period(false);
    let mut second = period(true);
    second.sold.value = r(i128::MAX, 1);
    second.sold_unit = SalesUnit::Kilotonnes;
    assert_eq!(
        decompose(&first, &second).unwrap_err().code,
        "FMINE-REF-207"
    );
}
