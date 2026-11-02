#[cfg(test)]
mod tests {
    use super::*;

    /// The bd-hp2j.3 log line: kernel, input count, the folded Philox key and counter prefix, digest, host.
    fn digest_line(kernel: &str, key: RngKey, digest: &str) -> String {
        let (k, c) = key.to_philox_key_and_hi_ctr();
        format!(
            "{{\"kernel\":\"{kernel}\",\"n\":10000,\"rng_key\":\"{:08x}{:08x}{:08x}{:08x}\",\"digest\":\"{digest}\",\
             \"target\":\"{}-{}\"}}",
            k[0],
            k[1],
            c[0],
            c[1],
            std::env::consts::ARCH,
            std::env::consts::OS
        )
    }

    // Version 2 TARGETs, first observed on x86_64-linux after the deliberate BD-333.S1 corrections: relative
    // log-mean branch selection, and supplied-spend endpoints with separate printed gaps and measured residual.
    // Independent high-precision answers below establish the mathematics; these snapshots check regression
    // and host agreement. Never refresh them merely to accommodate a different host (bd-hp2j.3).
    const LOGARITHMIC_MEAN_10K: &str =
        "bf89571e2deb3f4510a7d1184f812376fa81e899443f9b25e9aab6d183f3e490";
    const DECOMPOSE_AISC_LMDI_10K: &str =
        "ae31929c9609d83a39953010f652a8dbb4a64bbb905f01c3552be834f92d8656";

    #[test]
    fn test_golden_digest_logarithmic_mean_10k() {
        let d = golden_digest_logarithmic_mean_10k();
        println!("{}", digest_line("logarithmic_mean", LMDI_DIGEST_KEY, &d));
        assert_eq!(d, LOGARITHMIC_MEAN_10K);
    }

    #[test]
    fn test_golden_digest_decompose_aisc_lmdi_10k() {
        let d = golden_digest_decompose_aisc_lmdi_10k();
        let key = RngKey {
            partition: 2,
            ..LMDI_DIGEST_KEY
        };
        println!("{}", digest_line("decompose_aisc_lmdi", key, &d));
        assert_eq!(d, DECOMPOSE_AISC_LMDI_10K);
    }

    #[test]
    fn test_digest_inputs_cover_series_branch_and_tails() {
        let pairs = logarithmic_mean_digest_inputs(LMDI_DIGEST_KEY);
        let rel = |(a, b): (f64, f64)| (a - b).abs() / (0.5 * (a + b));
        let ratio = |(a, b): (f64, f64)| a.max(b) / a.min(b);
        assert!(
            pairs.iter().any(|&p| rel(p) < 1e-7),
            "no pair on the old series branch"
        );
        assert!(
            pairs
                .iter()
                .any(|&p| rel(p) < SERIES_SWITCH && rel(p) > 1e-7)
        );
        assert!(pairs.iter().any(|&p| rel(p) >= SERIES_SWITCH));
        assert!(pairs.iter().any(|&p| ratio(p) > 1e100), "no large ratio");
        assert!(
            pairs.iter().any(|&(a, b)| a.min(b) < 1e-300),
            "nothing near the subnormals"
        );
        assert!(
            pairs
                .iter()
                .all(|&(a, b)| a > 0.0 && b > 0.0 && a.is_finite() && b.is_finite())
        );
    }

    /// One Philox key word changed changes both digests.
    #[test]
    fn test_digests_change_with_the_key() {
        let other = RngKey {
            seed_root: LMDI_DIGEST_KEY.seed_root + 1,
            ..LMDI_DIGEST_KEY
        };
        assert_ne!(
            digest_logarithmic_mean(other),
            golden_digest_logarithmic_mean_10k()
        );
        let other2 = RngKey {
            seed_root: LMDI_DIGEST_KEY.seed_root + 1,
            partition: 2,
            ..LMDI_DIGEST_KEY
        };
        assert_ne!(
            digest_decompose_aisc_lmdi(other2),
            golden_digest_decompose_aisc_lmdi_10k()
        );
    }

    #[test]
    fn test_synthetic_two_period_exact_lmdi() {
        // Synthetic site, invented numbers. (These values were once labelled as a Barrick North Mara case,
        // E137; they match no published cell, R7-C054.)
        // Period 0: AISC = $1,297/oz, oz_produced = 47 koz, oz_sold = 47 koz (S0 = 1.000)
        // Period 1: AISC = $2,582/oz, oz_produced = 54 koz, oz_sold = 72 koz (S1 = 72/54 = 1.3333)
        let q1 = LmdiState {
            aisc_per_oz: 1297.0,
            total_aisc_spend: 1297.0 * 47.0,
            tonnes_processed: 720.0,
            head_grade_g_t: 2.25,
            recovery_fraction: 0.90,
            oz_produced: 47.0,
            oz_sold: 47.0,
        };
        let q2 = LmdiState {
            aisc_per_oz: 2582.0,
            total_aisc_spend: 2582.0 * 72.0,
            tonnes_processed: 735.0,
            head_grade_g_t: 2.52,
            recovery_fraction: 0.905,
            oz_produced: 54.0,
            oz_sold: 72.0,
        };

        let wf = decompose_aisc_lmdi(&q1, &q2).expect("valid LMDI");
        assert!((wf.total_delta - 1285.0).abs() < 1e-12);
        // Zero-residual guarantee (GB-26): sum of factors == +$1,285.00/oz to < 1e-11.
        assert!(
            (wf.sum_of_factors() - wf.total_delta).abs() < 1e-11,
            "LMDI residual non-zero: {} vs {}",
            wf.sum_of_factors(),
            wf.total_delta
        );
        // Independent 60-place Decimal calculation: L = 1866.3480233476275208 and the sales effect is
        // -536.9148672729303024. These invented endpoints have no printed-rate discrepancy.
        assert!((wf.sales_timing_effect + 536.9148672729303).abs() < 1e-9);
        assert_eq!(
            (
                wf.printed_rate_gap_0,
                wf.printed_rate_gap_1,
                wf.printed_rate_gap
            ),
            (0.0, 0.0, 0.0)
        );
        assert_eq!(wf.closure_residual, wf.total_delta - wf.sum_of_factors());
        assert!(wf.spend_effect > 1800.0);

        // A non-positive state refuses with 109 (outside its physical domain), never 105 (basis mismatch).
        let zero_sold = LmdiState { oz_sold: 0.0, ..q2 };
        let err = decompose_aisc_lmdi(&q1, &zero_sold).expect_err("non-positive");
        assert_eq!((err.code, err.exit_code), ("FMINE-REF-109", 3));
        // NaN and infinity are outside the domain too; a `<= 0.0` test alone let NaN through.
        for bad in [f64::NAN, f64::INFINITY] {
            let st = LmdiState {
                head_grade_g_t: bad,
                ..q2
            };
            let err = decompose_aisc_lmdi(&q1, &st).expect_err("non-finite");
            assert_eq!((err.code, err.exit_code), ("FMINE-REF-109", 3), "{bad}");
        }
    }

    fn simple_state() -> LmdiState {
        LmdiState {
            aisc_per_oz: 100.0,
            total_aisc_spend: 1000.0,
            tonnes_processed: 10.0,
            head_grade_g_t: 2.0,
            recovery_fraction: 0.5,
            oz_produced: 10.0,
            oz_sold: 10.0,
        }
    }

    fn effects(w: &LmdiWaterfall) -> [f64; 6] {
        [
            w.spend_effect,
            w.throughput_effect,
            w.grade_effect,
            w.recovery_effect,
            w.sales_timing_effect,
            w.circuit_inventory_effect,
        ]
    }

    fn assert_reconciled(w: &LmdiWaterfall) {
        assert_eq!(w.closure_residual, w.total_delta - w.sum_of_factors());
        assert!(w.closure_residual.abs() <= 1e-9, "{w:?}");
        assert!(
            (w.sum_of_factors() + w.closure_residual + w.printed_rate_gap
                - (w.printed_aisc_1 - w.printed_aisc_0))
                .abs()
                < 1e-9,
            "{w:?}"
        );
    }

    /// Hand identity: C doubles, Q and every physical factor stay fixed. Only the cost bar can move, by 100.
    #[test]
    fn test_supplied_cost_drives_factors_and_printed_rate_only_drives_gaps() {
        let p0 = LmdiState {
            aisc_per_oz: 105.0,
            ..simple_state()
        };
        let p1 = LmdiState {
            aisc_per_oz: 209.0,
            total_aisc_spend: 2000.0,
            ..simple_state()
        };
        let w = decompose_aisc_lmdi(&p0, &p1).unwrap();
        assert_eq!((w.aisc_0, w.aisc_1, w.total_delta), (100.0, 200.0, 100.0));
        assert_eq!(
            (
                w.printed_rate_gap_0,
                w.printed_rate_gap_1,
                w.printed_rate_gap
            ),
            (5.0, 9.0, 4.0)
        );
        assert!((w.spend_effect - 100.0).abs() < 1e-12);
        assert!(effects(&w)[1..].iter().all(|v| *v == 0.0));
        assert_reconciled(&w);

        let different_print = LmdiState {
            aisc_per_oz: 409.0,
            ..p1
        };
        let changed = decompose_aisc_lmdi(&p0, &different_print).unwrap();
        assert_eq!(
            effects(&w).map(f64::to_bits),
            effects(&changed).map(f64::to_bits)
        );
        assert_eq!(changed.total_delta, 100.0);
        assert_eq!(changed.printed_rate_gap, 204.0);
        assert_reconciled(&changed);

        // Conversely, unchanged printed rates must not hide a supplied-cost change.
        let unchanged_print = LmdiState {
            aisc_per_oz: 105.0,
            ..p1
        };
        let changed = decompose_aisc_lmdi(&p0, &unchanged_print).unwrap();
        assert!((changed.spend_effect - 100.0).abs() < 1e-12);
        assert_eq!(changed.printed_rate_gap, -100.0);
        assert_reconciled(&changed);
    }

    #[test]
    fn test_equal_endpoint_gaps_remain_visible_when_gap_change_is_zero() {
        let p0 = LmdiState {
            aisc_per_oz: 105.0,
            ..simple_state()
        };
        let p1 = LmdiState {
            aisc_per_oz: 205.0,
            total_aisc_spend: 2000.0,
            ..simple_state()
        };
        let w = decompose_aisc_lmdi(&p0, &p1).unwrap();
        assert_eq!((w.printed_rate_gap_0, w.printed_rate_gap_1), (5.0, 5.0));
        assert_eq!(w.printed_rate_gap, 0.0);
        assert_reconciled(&w);
    }

    /// Hand identity: fixed spend, twice the throughput/produced/sold quantity. Only throughput moves, -50.
    #[test]
    fn test_volume_change_closes_on_recomputed_rate() {
        let p0 = simple_state();
        let p1 = LmdiState {
            aisc_per_oz: 55.0,
            tonnes_processed: 20.0,
            oz_produced: 20.0,
            oz_sold: 20.0,
            ..p0
        };
        let w = decompose_aisc_lmdi(&p0, &p1).unwrap();
        assert_eq!((w.aisc_0, w.aisc_1, w.total_delta), (100.0, 50.0, -50.0));
        assert!((w.throughput_effect + 50.0).abs() < 1e-12);
        assert_eq!(w.spend_effect, 0.0);
        assert_eq!(w.printed_rate_gap, 5.0);
        assert_reconciled(&w);
    }

    /// Only C, Q and printed AISC are the bead's published North Mara inputs (BD-333.S1, E819):
    /// Q2 2025 AISC P18/P19/P23 = 64 $M / 50 koz / 1292; Q2 2026 M17/M18 = 93 $M / 37 koz, rate 2577.
    /// Physical factors below are invented to isolate reconciliation; this is not a site-driver attribution.
    #[test]
    fn test_north_mara_rate_gap_with_synthetic_physical_factors() {
        let p0 = LmdiState {
            aisc_per_oz: 1292.0,
            total_aisc_spend: 64_000.0,
            oz_sold: 50.0,
            ..simple_state()
        };
        let p1 = LmdiState {
            aisc_per_oz: 2577.0,
            total_aisc_spend: 93_000.0,
            oz_sold: 37.0,
            ..simple_state()
        };
        let w = decompose_aisc_lmdi(&p0, &p1).unwrap();
        assert_eq!(w.aisc_0, 1280.0);
        assert!((w.aisc_1 - 93_000.0 / 37.0).abs() < 1e-12);
        assert!((w.total_delta - 45_640.0 / 37.0).abs() < 1e-12);
        assert_eq!(w.printed_rate_gap_0, 12.0);
        assert!((w.printed_rate_gap - 1905.0 / 37.0).abs() < 1e-12);
        assert!((w.sum_of_factors() + w.printed_rate_gap - 1285.0).abs() < 1e-9);
        assert_reconciled(&w);
    }

    #[test]
    fn test_grade_change_reallocates_into_balance_without_identifying_a_cause() {
        let p0 = simple_state();
        let p1 = LmdiState {
            head_grade_g_t: 2.2,
            ..p0
        };
        let w = decompose_aisc_lmdi(&p0, &p1).unwrap();
        assert_eq!(w.total_delta, 0.0);
        assert!(w.grade_effect < 0.0 && w.circuit_inventory_effect > 0.0);
        assert!((w.grade_effect + w.circuit_inventory_effect).abs() < 1e-12);
        assert_reconciled(&w);
    }

    #[test]
    fn test_invalid_inputs_name_the_period_and_field() {
        let names = [
            "aisc_per_oz",
            "total_aisc_spend",
            "tonnes_processed",
            "head_grade_g_t",
            "recovery_fraction",
            "oz_produced",
            "oz_sold",
        ];
        for period in 0..2 {
            for (field, name) in names.into_iter().enumerate() {
                for bad in [0.0, -1.0, f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
                    let mut pair = [simple_state(); 2];
                    let st = &mut pair[period];
                    match field {
                        0 => st.aisc_per_oz = bad,
                        1 => st.total_aisc_spend = bad,
                        2 => st.tonnes_processed = bad,
                        3 => st.head_grade_g_t = bad,
                        4 => st.recovery_fraction = bad,
                        5 => st.oz_produced = bad,
                        _ => st.oz_sold = bad,
                    }
                    let err = decompose_aisc_lmdi(&pair[0], &pair[1]).unwrap_err();
                    assert_eq!((err.code, err.exit_code), ("FMINE-REF-109", 3));
                    assert!(
                        err.what.contains(&format!("period {period} {name}")),
                        "{err:?}"
                    );
                }
            }
        }
        let bad = LmdiState {
            recovery_fraction: 1.01,
            ..simple_state()
        };
        let err = decompose_aisc_lmdi(&simple_state(), &bad).unwrap_err();
        assert_eq!(err.code, "FMINE-REF-109");
        assert!(err.what.contains("period 1 recovery_fraction"));
        let one = LmdiState {
            recovery_fraction: 1.0,
            ..simple_state()
        };
        assert_reconciled(&decompose_aisc_lmdi(&one, &one).unwrap());
    }

    #[test]
    fn test_unrepresentable_rates_refuse_instead_of_returning_nan() {
        for (cost, sold) in [(f64::MAX, f64::MIN_POSITIVE), (f64::MIN_POSITIVE, f64::MAX)] {
            let bad = LmdiState {
                total_aisc_spend: cost,
                oz_sold: sold,
                ..simple_state()
            };
            let err = decompose_aisc_lmdi(&simple_state(), &bad).unwrap_err();
            assert_eq!((err.code, err.exit_code), ("FMINE-REF-207", 3));
            assert!(err.what.contains("period 1 recomputed AISC"));
        }
        let large = LmdiState {
            total_aisc_spend: 1e308,
            oz_sold: 1.0,
            ..simple_state()
        };
        let too_large_effect = LmdiState {
            tonnes_processed: 1e308,
            ..large
        };
        assert_eq!(
            decompose_aisc_lmdi(&large, &too_large_effect)
                .unwrap_err()
                .code,
            "FMINE-REF-207"
        );
    }

    #[test]
    fn test_log_space_avoids_product_overflow_and_subnormal_ratio_rounding() {
        let large = LmdiState {
            tonnes_processed: 1e200,
            head_grade_g_t: 1e200,
            ..simple_state()
        };
        let unchanged = decompose_aisc_lmdi(&large, &large).unwrap();
        assert!(effects(&unchanged).iter().all(|v| *v == 0.0));
        assert_reconciled(&unchanged);

        // Independent review case: temporal cost and ounce ratios are positive subnormals, while both
        // endpoint rates are ordinary finite numbers. Logging those rounded subnormals breaks closure.
        let p0 = LmdiState {
            total_aisc_spend: 1e300,
            oz_sold: 1e297,
            oz_produced: 1e297,
            ..simple_state()
        };
        let p1 = LmdiState {
            total_aisc_spend: 1e-20,
            oz_sold: 1.1e-23,
            oz_produced: 1.1e-23,
            ..simple_state()
        };
        assert_reconciled(&decompose_aisc_lmdi(&p0, &p1).unwrap());
    }

    fn pairs(stream: u32) -> (fmine_core::rand::RngKey, Vec<(f64, f64)>) {
        let key = fmine_core::rand::RngKey {
            seed_root: 2_242,
            partition: stream,
            domain: 0,
            var_group: 0,
            realization: 0,
            brick: 0,
        };
        let mut rng = fmine_core::rand::PhiloxStream::new(key);
        let out = (0..10_000)
            .map(|_| {
                // a in (1e-3, 1e6); b at a relative distance 10^-e, e in [0, 14), either side. The inputs come
                // from det too, so every host tests the same pairs.
                let ln10 = std::f64::consts::LN_10;
                let a = 1e-3 * det::exp(9.0 * ln10 * rng.next_f64_open01());
                let r = det::exp(-14.0 * ln10 * rng.next_f64_open01());
                let b = if rng.next_f64_open01() < 0.5 {
                    a * (1.0 + r)
                } else {
                    a / (1.0 + r)
                };
                (a, b)
            })
            .collect();
        (key, out)
    }

    #[test]
    fn test_logarithmic_mean_high_precision_tiny_and_large_known_answers() {
        // Independent Python Decimal (80 decimal digits), (a-b)/(ln(a)-ln(b)), on the decimal inputs.
        // These are mathematical expected values, not output snapshots from the candidate kernel.
        for (a, b, expected) in [
            (1.0, 0.001, 0.14462006247378287),
            (1e-18, 1e-21, 1.4462006247378286e-19),
            (1e-300, 1e-303, 1.4462006247378286e-301),
            (1e-308, 1e-310, 2.1497576854211e-309),
            (1.6e308, 1.5e308, 1.549462216322538e308),
            (1.6e308, 1.599e308, 1.5994999479003842e308),
        ] {
            let actual = logarithmic_mean(a, b);
            assert!(
                (actual / expected - 1.0).abs() < 2e-14,
                "L({a},{b})={actual}, expected {expected}"
            );
            assert_eq!(actual.to_bits(), logarithmic_mean(b, a).to_bits());
        }
        for x in [f64::from_bits(1), 1e-300, 1.0, f64::MAX] {
            assert_eq!(logarithmic_mean(x, x), x);
        }
        for bad in [0.0, -1.0, f64::NAN, f64::INFINITY] {
            assert!(logarithmic_mean(1.0, bad).is_nan());
            assert!(logarithmic_mean(bad, 1.0).is_nan());
        }
    }

    #[test]
    fn test_logarithmic_mean_scale_invariance_for_widely_separated_rates() {
        let unscaled = logarithmic_mean(1.0, 0.001);
        for scale in [1e-300, 1e-18, 1e-10, 1.0, 1e100, 1e300] {
            let actual = logarithmic_mean(scale, 0.001 * scale);
            assert!(
                (actual / (unscaled * scale) - 1.0).abs() < 2e-14,
                "scale={scale}"
            );
        }
    }

    #[test]
    fn test_logarithmic_mean_between_min_and_max() {
        let (key, pairs) = pairs(1);
        for &(a, b) in &pairs {
            let l = logarithmic_mean(a, b);
            assert!(a.min(b) <= l && l <= a.max(b), "L({a:e}, {b:e}) = {l:e}");
        }
        println!(
            "{{\"test\":\"between\",\"cases\":{},\"key\":\"{key:?}\"}}",
            pairs.len()
        );
    }

    #[test]
    fn test_logarithmic_mean_symmetric() {
        let (key, pairs) = pairs(2);
        for &(a, b) in &pairs {
            assert_eq!(
                logarithmic_mean(a, b).to_bits(),
                logarithmic_mean(b, a).to_bits()
            );
        }
        println!(
            "{{\"test\":\"symmetric\",\"cases\":{},\"key\":\"{key:?}\"}}",
            pairs.len()
        );
    }

    /// The jump across the switch is the two branches' difference there: both are evaluated at the same points,
    /// a relative distance of SERIES_SWITCH · (1 ± 1e-3) apart, on either side.
    #[test]
    fn test_logarithmic_mean_continuous_at_series_switch() {
        let (key, pairs) = pairs(3);
        let mut max_rel_jump = 0.0_f64;
        for (i, &(a, _)) in pairs.iter().enumerate() {
            let r = SERIES_SWITCH * (1.0 + 1e-3 * ((i % 21) as f64 - 10.0) / 10.0);
            let b = if i % 2 == 0 {
                a * (1.0 + r)
            } else {
                a / (1.0 + r)
            };
            let (s, l) = (series_branch(a, b), log_branch(a, b));
            max_rel_jump = max_rel_jump.max((s - l).abs() / l);
        }
        println!(
            "{{\"test\":\"continuous_at_switch\",\"cases\":{},\"key\":\"{key:?}\",\"max_rel_jump\":{max_rel_jump:e}}}",
            pairs.len()
        );
        assert!(max_rel_jump < 1e-12, "max relative jump {max_rel_jump:e}");
    }
}
