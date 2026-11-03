//! Determinism: Exact

use super::*;

fn r(num: i128, den: i128) -> Rat {
    Rat::new(num, den).unwrap()
}

fn plan(seed: u64, draws: usize) -> BootstrapPlan {
    BootstrapPlan {
        seed,
        replicates: MIN_REPLICATES,
        draws_per_replicate: draws,
    }
}

#[test]
fn known_fractional_tails_and_zero_count_states_keep_exact_units() {
    let law =
        PreparedPayoffs::new(&[r(-7, 3), r(1, 6), r(11, 4), Rat::from_integer(1_000)]).unwrap();
    let counts = [2, 1, 3, 0];
    assert_eq!(
        law.evaluate(&counts, None).unwrap(),
        ResampledStats {
            mean: r(5, 8),
            tail_loss: None
        }
    );
    for (mass, expected) in [
        (r(1, 2), r(3, 2)),
        (r(1, 4), r(7, 3)),
        (r(2, 5), r(23, 12)),
        (ONE, r(-5, 8)),
    ] {
        assert_eq!(
            law.evaluate(&counts, Some(mass)).unwrap(),
            ResampledStats {
                mean: r(5, 8),
                tail_loss: Some(expected)
            }
        );
    }
}

#[test]
fn repeated_values_permutation_and_signed_extremes_are_not_rounded() {
    let original = PreparedPayoffs::new(&[r(2, 7), r(-1, 3), r(2, 7)]).unwrap();
    let permuted = PreparedPayoffs::new(&[r(-1, 3), r(2, 7), r(2, 7)]).unwrap();
    for mass in [r(1, 100), r(3, 7), ONE] {
        assert_eq!(
            original.evaluate(&[3, 2, 5], Some(mass)),
            permuted.evaluate(&[2, 5, 3], Some(mass))
        );
    }
    let max = Rat::from_integer(i128::MAX);
    let huge = PreparedPayoffs::new(&[max, max]).unwrap();
    assert_eq!(
        huge.evaluate(&[2_048, 2_048], Some(ONE)).unwrap(),
        ResampledStats {
            mean: max,
            tail_loss: Some(max.checked_neg().unwrap())
        }
    );
    let min = Rat::from_integer(i128::MIN);
    let minimum = PreparedPayoffs::new(&[min]).unwrap();
    assert_eq!(minimum.evaluate(&[1], None).unwrap().mean, min);
    assert_eq!(
        minimum.evaluate(&[1], Some(ONE)).unwrap_err().code,
        "FMINE-REF-207"
    );
}

#[test]
fn reductions_match_independent_equal_mass_expansion() {
    let values = [r(-7, 3), r(4, 5), r(4, 5), r(11, 2)];
    let prepared = PreparedPayoffs::new(&values).unwrap();
    for code in 1..256_u32 {
        let counts: Vec<_> = (0..4).map(|i| (code >> (2 * i)) & 3).collect();
        let mut expanded = Vec::new();
        for (&value, &count) in values.iter().zip(&counts) {
            expanded.extend(std::iter::repeat_n(value, count as usize));
        }
        expanded.sort_unstable_by(|a, b| compare(*a, *b));
        let n = Rat::from_integer(expanded.len() as i128);
        let expected_mean = expanded
            .iter()
            .fold(ZERO, |sum, value| sum.checked_add(*value).unwrap())
            .checked_div(n)
            .unwrap();
        for mass in [r(1, 7), r(2, 5), ONE] {
            let atom_count = mass.checked_mul(n).unwrap();
            let whole = (atom_count.num() / atom_count.den()) as usize;
            let fraction = r(atom_count.num() % atom_count.den(), atom_count.den());
            let mut sum = expanded[..whole]
                .iter()
                .fold(ZERO, |sum, value| sum.checked_add(*value).unwrap());
            if fraction != ZERO {
                sum = sum
                    .checked_add(expanded[whole].checked_mul(fraction).unwrap())
                    .unwrap();
            }
            let expected_tail = sum.checked_div(atom_count).unwrap().checked_neg().unwrap();
            assert_eq!(
                prepared.evaluate(&counts, Some(mass)).unwrap(),
                ResampledStats {
                    mean: expected_mean,
                    tail_loss: Some(expected_tail)
                }
            );
        }
    }
}

#[test]
fn bounded_draw_rejects_biased_prefix_and_exhaustion_without_clamping() {
    let mut stream = [0, 5, 6, 9, 10].into_iter();
    let mut draw = || stream.next().unwrap();
    assert_eq!(bounded_u128(10, &mut draw).unwrap(), 6);
    assert_eq!(bounded_u128(10, &mut draw).unwrap(), 9);
    assert_eq!(bounded_u128(10, &mut draw).unwrap(), 0);
    assert_eq!(
        bounded_u128(1, &mut || panic!("degenerate draw must not consume RNG")).unwrap(),
        0
    );
    assert_eq!(
        bounded_u128(0, &mut || 0).unwrap_err().code,
        "FMINE-REF-109"
    );
    assert_eq!(
        bounded_u128(3, &mut || 0).unwrap_err().code,
        "FMINE-REF-207"
    );
}

#[test]
fn exact_weighted_sampler_preserves_replication_and_draw_prefixes() {
    let weights = [r(1, 6), r(1, 3), r(1, 7), r(5, 14)];
    let original = sample_counts(&weights, plan(229, 64)).unwrap();
    assert_eq!(sample_counts(&weights, plan(229, 64)).unwrap(), original);
    let mut longer = plan(229, 64);
    longer.replicates += 1;
    assert_eq!(
        &sample_counts(&weights, longer).unwrap()[..MIN_REPLICATES],
        original
    );
    longer.draws_per_replicate += 1;
    let extended = sample_counts(&weights, longer).unwrap();
    for (old, new) in original.iter().zip(&extended) {
        assert_eq!(old.iter().sum::<u32>(), 64);
        assert_eq!(new.iter().sum::<u32>(), 65);
        assert!(old.iter().zip(new).all(|(a, b)| a <= b));
    }
    assert_ne!(sample_counts(&weights, plan(230, 64)).unwrap(), original);
    let mut totals = [0_i128; 4];
    for row in original {
        for (total, count) in totals.iter_mut().zip(row) {
            *total += i128::from(count);
        }
    }
    for (total, weight) in totals.into_iter().zip(weights) {
        let error = r(total, (MIN_REPLICATES * 64) as i128)
            .checked_sub(weight)
            .unwrap();
        assert!(compare(error, r(-1, 50)).is_gt() && compare(error, r(1, 50)).is_lt());
    }
    assert_eq!(
        sample_counts(&[ONE], plan(0, 3)).unwrap(),
        vec![vec![3]; MIN_REPLICATES]
    );
}

#[test]
fn conditional_tree_needs_no_global_common_denominator() {
    // DESIGN known answer: p*q exceeds u128; adjacent pairs each total exactly one half.
    let p = (1_i128 << 70) + 1;
    let q = (1_i128 << 70) + 3;
    assert!(p.checked_mul(q).is_none());
    let weights = [r(1, 2 * p), r(p - 1, 2 * p), r(1, 2 * q), r(q - 1, 2 * q)];
    let counts = sample_counts(&weights, plan(u64::MAX, 7)).unwrap();
    assert!(
        counts
            .iter()
            .all(|row| row.len() == 4 && row.iter().sum::<u32>() == 7)
    );
    let very_large = i128::MAX;
    let counts = sample_counts(
        &[r(1, very_large), r(very_large - 1, very_large)],
        plan(17, 1),
    )
    .unwrap();
    assert!(counts.iter().all(|row| row.iter().sum::<u32>() == 1));
}

#[test]
fn invalid_laws_counts_and_work_refuse_before_a_result() {
    for weights in [
        vec![],
        vec![ZERO, ONE],
        vec![r(-1, 2), r(3, 2)],
        vec![r(1, 3), r(1, 3)],
        vec![ONE; MAX_STATES + 1],
    ] {
        assert!(sample_counts(&weights, plan(0, 1)).is_err());
    }
    for (replicates, draws) in [
        (199, 1),
        (2_001, 1),
        (200, 0),
        (200, 4_097),
        (2_000, 4_096),
        (usize::MAX, usize::MAX),
    ] {
        assert_eq!(
            sample_counts(
                &[ONE],
                BootstrapPlan {
                    seed: 0,
                    replicates,
                    draws_per_replicate: draws
                }
            )
            .unwrap_err()
            .code,
            "FMINE-REF-201"
        );
    }
    assert!(PreparedPayoffs::new(&[]).is_err());
    assert!(PreparedPayoffs::new(&vec![ZERO; MAX_STATES + 1]).is_err());
    let law = PreparedPayoffs::new(&[ONE, ZERO]).unwrap();
    for counts in [
        vec![],
        vec![1],
        vec![0, 0],
        vec![4_096, 1],
        vec![u32::MAX, u32::MAX],
    ] {
        assert!(law.evaluate(&counts, None).is_err());
    }
    for mass in [ZERO, r(-1, 2), r(3, 2)] {
        assert!(law.evaluate(&[1, 1], Some(mass)).is_err());
    }
    let overflow = PreparedPayoffs::new(&[r(i128::MAX, 2), r(i128::MAX - 2, 3)]).unwrap();
    assert_eq!(
        overflow.evaluate(&[1, 1], None).unwrap_err().code,
        "FMINE-REF-207"
    );
}
