//! The budget table read against the plan's §26.7 table, the meet laws, and each region's exhaust behaviour.

use super::*;

const H: u64 = 3_600 * S;

/// Parameters chosen so no two of them are equal, and no product or sum of them equals a fixed limit.
const P: Params = Params {
    estimation_deadline_ns: 4 * H,
    estimation_blocks: 1_000,
    estimation_realizations: 50,
    peer_window_ns: 30 * S,
    peer_window_bytes: 8 * MIB,
    fa_build: FaBuild::Release,
};

fn table(p: &Params) -> [BudgetRow; 5] {
    budget_table(p).unwrap()
}

fn plan() -> String {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../COMPREHENSIVE_PLAN_FOR_THE_DESIGN_OF_FRANKENMINING.md"
    );
    std::fs::read_to_string(path).unwrap()
}

/// The cells of the first table after `marker` in `section`, header and rule rows dropped.
fn table_rows(section: &str, marker: &str) -> Vec<Vec<String>> {
    let rest = &section[section.find(marker).unwrap()..];
    rest.lines()
        .skip_while(|l| !l.starts_with('|'))
        .take_while(|l| l.starts_with('|'))
        .skip(2)
        .map(|l| {
            l.trim_matches('|')
                .split('|')
                .map(|c| c.trim().to_owned())
                .collect()
        })
        .collect()
}

/// Every `<number> <unit>` in `cell`, in nanoseconds or bytes.
fn quantities(cell: &str) -> Vec<u64> {
    let words: Vec<&str> = cell.split_whitespace().collect();
    words
        .windows(2)
        .filter_map(|w| {
            let n: u64 = w[0].parse().ok()?;
            let unit = match w[1].trim_end_matches([';', ')']) {
                "ms" => MS,
                "s" => S,
                "h" => H,
                "MiB" => MIB,
                _ => return None,
            };
            Some(n * unit)
        })
        .collect()
}

#[test]
fn the_table_is_the_plans_table() {
    let plan = plan();
    let exits = table_rows(&plan[plan.find("### 30.1").unwrap()..], "**Exit codes**");
    let indeterminate: u8 = exits
        .iter()
        .find(|r| r[1].starts_with("**Indeterminate."))
        .unwrap()[0]
        .trim_matches('*')
        .parse()
        .unwrap();
    let rows = table_rows(&plan[plan.find("### 26.7").unwrap()..], "**Budgets.**");
    let ours = table(&P);
    let debug = table(&Params {
        fa_build: FaBuild::Debug,
        ..P
    });
    assert_eq!(rows.len(), ours.len());
    for ((cells, row), debug_row) in rows.iter().zip(&ours).zip(&debug) {
        let (region, deadline, cost, priority, on_exhaust) =
            (&cells[0], &cells[1], &cells[2], &cells[3], &cells[4]);
        assert_eq!(
            region.split_whitespace().next().unwrap(),
            format!("{:?}", row.region)
        );
        let budget = row.budget;
        assert_eq!(budget.poll_quota, None, "§26.7 gives no poll quota");
        assert_eq!(budget.priority.to_string(), *priority);

        let times = quantities(deadline);
        if deadline.starts_with("per run") {
            assert_eq!(budget.deadline_ns, Some(P.estimation_deadline_ns));
        } else if deadline == "link window" {
            assert_eq!(budget.deadline_ns, Some(P.peer_window_ns));
        } else if times.len() == 2 {
            assert!(deadline.contains("debug FA") && deadline.contains("TARGET"));
            assert_eq!(debug_row.budget.deadline_ns, Some(times[0]));
            assert_eq!(budget.deadline_ns, Some(times[1]));
        } else {
            assert_eq!(budget.deadline_ns, Some(times[0]));
            assert_eq!(debug_row.budget, budget);
        }

        let expected_cost = match cost.as_str() {
            "—" => None,
            "blocks × realizations" => Some((
                P.estimation_blocks * P.estimation_realizations,
                CostUnit::BlockRealizations,
            )),
            "bytes per window" => Some((P.peer_window_bytes, CostUnit::Bytes)),
            _ => {
                assert!(cost.ends_with("MiB per segment"));
                Some((quantities(cost)[0], CostUnit::Bytes))
            }
        };
        assert_eq!(
            budget.cost_quota.zip(row.cost_unit),
            expected_cost,
            "{region}"
        );
        assert_eq!(budget.cost_quota.is_some(), row.cost_unit.is_some());

        let action = match on_exhaust.split_whitespace().next().unwrap() {
            "Refuse" => OnExhaust::RefuseBatch,
            "Seal" => OnExhaust::SealEarly,
            "Cancel;" => OnExhaust::Cancel,
            "Drain;" => OnExhaust::Drain,
            "Indeterminate" => OnExhaust::Reconcile,
            other => panic!("unread on-exhaust cell {other:?}"),
        };
        assert_eq!(row.on_exhaust, action, "{region}");
        let stated = on_exhaust
            .split("(exit ")
            .nth(1)
            .map(|s| s[..s.find(')').unwrap()].parse::<u8>().unwrap());
        let expected = stated.or((action == OnExhaust::Reconcile).then_some(indeterminate));
        assert_eq!(action.exit_code(), expected, "{region}");
    }
}

#[test]
fn zero_parameters_and_an_overflowing_run_size_are_refused() {
    type Set = fn(&mut Params);
    let cases: [(&str, Set); 5] = [
        ("estimation_deadline_ns", |p| p.estimation_deadline_ns = 0),
        ("estimation_blocks", |p| p.estimation_blocks = 0),
        ("estimation_realizations", |p| p.estimation_realizations = 0),
        ("peer_window_ns", |p| p.peer_window_ns = 0),
        ("peer_window_bytes", |p| p.peer_window_bytes = 0),
    ];
    for (name, set) in cases {
        let mut p = P;
        set(&mut p);
        assert_eq!(budget_table(&p), Err(BudgetError::Zero(name)));
    }
    let huge = Params {
        estimation_blocks: u64::MAX,
        estimation_realizations: 2,
        ..P
    };
    assert_eq!(budget_table(&huge), Err(BudgetError::Overflow));
    let edge = Params {
        estimation_blocks: u64::MAX,
        estimation_realizations: 1,
        ..P
    };
    assert_eq!(table(&edge)[2].budget.cost_quota, Some(u64::MAX));
}

/// Budgets over a small grid of limits, including no limit and a zero limit, and three priorities.
fn grid() -> Vec<Budget> {
    let limits = [None, Some(0), Some(7), Some(u64::MAX)];
    let mut out = Vec::new();
    for deadline_ns in limits {
        for poll_quota in limits {
            for cost_quota in limits {
                for priority in [0, 80, 200] {
                    out.push(Budget {
                        deadline_ns,
                        poll_quota,
                        cost_quota,
                        priority,
                    });
                }
            }
        }
    }
    out
}

#[test]
fn meet_is_a_semilattice_with_unlimited_as_its_identity() {
    let all = grid();
    for &a in &all {
        assert_eq!(a.meet(a), a);
        assert_eq!(a.meet(Budget::UNLIMITED), a);
        for &b in &all {
            let m = a.meet(b);
            assert_eq!(m, b.meet(a));
            for (x, y, z) in [
                (a.deadline_ns, b.deadline_ns, m.deadline_ns),
                (a.poll_quota, b.poll_quota, m.poll_quota),
                (a.cost_quota, b.cost_quota, m.cost_quota),
            ] {
                match (x, y) {
                    (None, None) => assert_eq!(z, None),
                    _ => {
                        let z = z.unwrap();
                        assert!(x.is_none_or(|x| z <= x) && y.is_none_or(|y| z <= y));
                        assert!(z == x.unwrap_or(z) || z == y.unwrap_or(z));
                    }
                }
            }
            assert_eq!(m.priority, a.priority.max(b.priority));
        }
    }
    let some: Vec<Budget> = all.iter().copied().step_by(7).collect();
    for &a in &some {
        for &b in &some {
            for &c in &some {
                assert_eq!(a.meet(b).meet(c), a.meet(b.meet(c)));
            }
        }
    }
}

fn exhausted(limit: Limit, on_exhaust: OnExhaust) -> Result<(), Stop> {
    Err(Stop::Exhausted(Exhausted { limit, on_exhaust }))
}

#[test]
fn the_ledger_writer_refuses_the_batch_at_250_ms_with_exit_6() {
    let row = table(&P)[0];
    let t0 = 1_000;
    let mut m = row.meter(Budget::UNLIMITED, t0);
    assert_eq!(m.poll(t0), Ok(()));
    assert_eq!(m.charge(u64::MAX, t0 + 250 * MS - 1), Ok(()));
    assert_eq!(
        m.poll(t0 + 250 * MS),
        exhausted(Limit::Deadline, OnExhaust::RefuseBatch)
    );
    assert_eq!(OnExhaust::RefuseBatch.exit_code(), Some(6));
    // Exhaustion is sticky: an earlier time or a free charge does not revive it.
    assert_eq!(
        m.charge(0, t0),
        exhausted(Limit::Deadline, OnExhaust::RefuseBatch)
    );
    assert_eq!(m.spent(), (1, u64::MAX));
}

#[test]
fn telemetry_seals_early_past_64_mib_and_keeps_the_refused_bytes_out() {
    let row = table(&P)[1];
    let mut m = row.meter(Budget::UNLIMITED, 0);
    assert_eq!(m.charge(64 * MIB - 1, 10), Ok(()));
    assert_eq!(m.charge(1, 20), Ok(()));
    assert_eq!(m.spent(), (0, 64 * MIB));
    assert_eq!(
        m.charge(1, 30),
        exhausted(Limit::Cost, OnExhaust::SealEarly)
    );
    assert_eq!(m.spent(), (0, 64 * MIB));
    assert_eq!(OnExhaust::SealEarly.exit_code(), None);

    let mut late = row.meter(Budget::UNLIMITED, 0);
    assert_eq!(late.charge(0, S - 1), Ok(()));
    assert_eq!(
        late.charge(0, S),
        exhausted(Limit::Deadline, OnExhaust::SealEarly)
    );
}

#[test]
fn an_estimation_run_cancels_past_blocks_times_realizations_or_its_deadline() {
    let row = table(&P)[2];
    let mut m = row.meter(Budget::UNLIMITED, 0);
    for k in 0..P.estimation_realizations {
        assert_eq!(m.charge(P.estimation_blocks, k), Ok(()));
    }
    assert_eq!(
        m.charge(1, P.estimation_realizations),
        exhausted(Limit::Cost, OnExhaust::Cancel)
    );
    assert_eq!(m.spent().1, P.estimation_blocks * P.estimation_realizations);

    // Under a compute budget of one hour, the run's four-hour deadline gives way to the parent's.
    let parent = Budget {
        deadline_ns: Some(H),
        ..Budget::UNLIMITED
    };
    let mut nested = row.meter(parent, 5);
    assert_eq!(nested.poll(5 + H - 1), Ok(()));
    assert_eq!(
        nested.poll(5 + H),
        exhausted(Limit::Deadline, OnExhaust::Cancel)
    );
    let mut alone = row.meter(Budget::UNLIMITED, 5);
    assert_eq!(alone.poll(5 + H), Ok(()));
    assert_eq!(
        alone.poll(5 + 4 * H),
        exhausted(Limit::Deadline, OnExhaust::Cancel)
    );
}

#[test]
fn a_peer_session_drains_at_its_window_and_the_next_window_starts_fresh() {
    let row = table(&P)[3];
    let mut window = row.meter(Budget::UNLIMITED, 0);
    assert_eq!(window.charge(P.peer_window_bytes, 1), Ok(()));
    assert_eq!(
        window.charge(1, 2),
        exhausted(Limit::Cost, OnExhaust::Drain)
    );
    let mut next = row.meter(Budget::UNLIMITED, P.peer_window_ns);
    assert_eq!(
        next.charge(P.peer_window_bytes, P.peer_window_ns + 1),
        Ok(())
    );
    assert_eq!(
        next.poll(2 * P.peer_window_ns),
        exhausted(Limit::Deadline, OnExhaust::Drain)
    );
}

#[test]
fn a_finance_post_past_its_deadline_is_indeterminate_and_reconciles() {
    for (build, deadline) in [(FaBuild::Debug, 120 * S), (FaBuild::Release, 10 * S)] {
        let row = table(&Params {
            fa_build: build,
            ..P
        })[4];
        let mut m = row.meter(Budget::UNLIMITED, 0);
        assert_eq!(m.poll(deadline - 1), Ok(()));
        assert_eq!(
            m.poll(deadline),
            exhausted(Limit::Deadline, OnExhaust::Reconcile)
        );
    }
    assert_eq!(OnExhaust::Reconcile.exit_code(), Some(9));
}

#[test]
fn the_poll_quota_counts_polls_and_a_zero_limit_allows_nothing_but_free_work() {
    let budget = Budget {
        poll_quota: Some(3),
        cost_quota: Some(0),
        ..Budget::UNLIMITED
    };
    let mut m = Meter::start(budget, OnExhaust::Cancel, 0);
    assert_eq!(m.charge(0, 0), Ok(()));
    for t in 0..3 {
        assert_eq!(m.poll(t), Ok(()));
    }
    assert_eq!(m.poll(3), exhausted(Limit::Polls, OnExhaust::Cancel));
    assert_eq!(m.spent(), (3, 0));
    let mut free = Meter::start(budget, OnExhaust::Cancel, 0);
    assert_eq!(free.charge(1, 0), exhausted(Limit::Cost, OnExhaust::Cancel));
    let instant = Budget {
        deadline_ns: Some(0),
        ..Budget::UNLIMITED
    };
    assert_eq!(
        Meter::start(instant, OnExhaust::Drain, 9).poll(9),
        exhausted(Limit::Deadline, OnExhaust::Drain)
    );
}

#[test]
fn time_going_backwards_and_a_cost_past_u64_are_refused_without_spending_or_exhausting() {
    let mut m = Meter::start(Budget::UNLIMITED, OnExhaust::Drain, 100);
    assert_eq!(m.poll(99), Err(Stop::Invalid(BudgetError::BeforeStart)));
    assert_eq!(
        m.charge(1, 99),
        Err(Stop::Invalid(BudgetError::BeforeStart))
    );
    assert_eq!(m.charge(1, 100), Ok(()));
    assert_eq!(
        m.charge(u64::MAX, 101),
        Err(Stop::Invalid(BudgetError::Overflow))
    );
    assert_eq!(m.spent(), (0, 1));
    assert_eq!(m.poll(u64::MAX), Ok(()));
    assert_eq!(m.spent(), (1, 1));
}
